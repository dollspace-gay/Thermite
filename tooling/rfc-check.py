#!/usr/bin/env python3
# /// script
# requires-python = ">=3.9"
# dependencies = []
# ///
"""
RFC front-matter gate for `.design/rfcs/`.

RFCs are files with front matter (RFC-4). This gate keeps the directory
mechanically readable, so the generated index and the registry link cannot drift
from the documents:

  * every RFC has front matter with the required fields;
  * `status` is one of draft/accepted/rejected/superseded — there is no
    `implemented`, because implementation is derived from the REQ registry;
  * `rfc:` agrees with the filename prefix, and is unique — except that two
    *drafts* may share a number, since a draft's number is provisional until
    merge and the collision is resolved there;
  * every REQ named in `introduces:` exists in `.design/reqs/registry.toml`;
  * every RFC named in `supersedes:` exists.

Usage:

    python3 tooling/rfc-check.py [--root <repo>] [--json] [--index]
    uv run  tooling/rfc-check.py [--root <repo>] [--json] [--index]

The PEP 723 header above pins the interpreter this needs, so `uv run` fetches a
matching one rather than inheriting whatever `python3` happens to be. This gate
is stdlib-only and works on 3.9, so the header is a statement rather than a
requirement — but a gate that skips silently on the wrong interpreter is worse
than one that fails, and declaring the floor is how that is avoided.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

REQUIRED = ("rfc", "title", "status")
STATUSES = ("draft", "accepted", "rejected", "superseded")
FILENAME = re.compile(r"^(\d{4})-[a-z0-9-]+\.md$")


def parse_front_matter(text: str) -> tuple[dict, str | None]:
    """Return (fields, error). Deliberately small: a flat `key: value` block plus
    `[a, b]` and `- item` lists. A full YAML parser is not a dependency worth
    taking for five fields."""
    if not text.startswith("---\n"):
        return {}, "no front matter (the file must open with `---`)"
    end = text.find("\n---\n", 3)
    if end == -1:
        return {}, "front matter is not closed with `---`"

    fields: dict[str, object] = {}
    key = None
    for raw in text[4:end].splitlines():
        line = raw.rstrip()
        if not line.strip():
            continue
        if line.startswith("  - ") and key:
            fields.setdefault(key, [])
            if isinstance(fields[key], list):
                fields[key].append(line[4:].strip())
            continue
        if ":" not in line:
            return {}, f"malformed front-matter line: {line!r}"
        key, _, value = line.partition(":")
        key, value = key.strip(), value.strip()
        if value.startswith("[") and value.endswith("]"):
            inner = value[1:-1].strip()
            fields[key] = [v.strip() for v in inner.split(",") if v.strip()]
        elif value:
            fields[key] = value
        else:
            fields[key] = []
    return fields, None


def known_reqs(root: Path) -> set[str]:
    registry = root / ".design" / "reqs" / "registry.toml"
    if not registry.is_file():
        return set()
    return set(re.findall(r"\bREQ-[A-Z0-9-]+\b", registry.read_text(encoding="utf-8")))


def revision(root: Path, path: Path) -> tuple[int, str]:
    """(revision number, short hash) for an RFC file — the count of commits that
    touched it, and the newest. Derived rather than declared, so it cannot drift
    from the file it describes. Returns (0, "") outside a git tree."""
    try:
        log = subprocess.run(
            ["git", "log", "--format=%h", "--", str(path.relative_to(root))],
            cwd=root, capture_output=True, text=True, check=True).stdout.split()
    except (subprocess.CalledProcessError, OSError, ValueError):
        return 0, ""
    return len(log), (log[0] if log else "")


def index(root: Path) -> list[dict]:
    """The generated RFC index: number, title, status, and derived version."""
    rows = []
    for path in sorted((root / ".design" / "rfcs").glob("*.md")):
        fields, err = parse_front_matter(path.read_text(encoding="utf-8"))
        if err:
            continue
        rev, sha = revision(root, path)
        rows.append({
            "rfc": fields.get("rfc"),
            "title": fields.get("title"),
            "status": fields.get("status"),
            "revision": rev,
            "commit": sha,
            "file": path.name,
        })
    return rows


def check(root: Path) -> list[str]:
    rfc_dir = root / ".design" / "rfcs"
    if not rfc_dir.is_dir():
        return [f"{rfc_dir} does not exist"]

    problems: list[str] = []
    notes: list[str] = []
    reqs = known_reqs(root)
    seen: dict[int, tuple[str, str]] = {}   # number -> (filename, status)
    titles: dict[str, str] = {}

    for path in sorted(rfc_dir.glob("*.md")):
        rel = path.relative_to(root)
        name_match = FILENAME.match(path.name)
        if not name_match:
            problems.append(f"{rel}: filename must be NNNN-slug.md")
            continue

        fields, err = parse_front_matter(path.read_text(encoding="utf-8"))
        if err:
            problems.append(f"{rel}: {err}")
            continue

        for field in REQUIRED:
            if field not in fields:
                problems.append(f"{rel}: front matter is missing `{field}`")

        status = fields.get("status")
        if status is not None and status not in STATUSES:
            problems.append(
                f"{rel}: status {status!r} is not one of {'/'.join(STATUSES)}"
                " (implementation is derived from the REQ registry, not declared)"
            )

        try:
            number = int(str(fields.get("rfc", "")).strip())
        except ValueError:
            problems.append(f"{rel}: `rfc:` must be an integer")
            continue

        if number != int(name_match.group(1)):
            problems.append(
                f"{rel}: `rfc: {number}` disagrees with the filename prefix"
                f" {name_match.group(1)}"
            )
        if number in seen:
            other_name, other_status = seen[number]
            if status == "draft" and other_status == "draft":
                notes.append(
                    f"{rel}: rfc {number} is also used by {other_name};"
                    " both are drafts, so the number is provisional until merge"
                )
            else:
                problems.append(
                    f"{rel}: rfc {number} is already used by {other_name}"
                    f" (status {other_status})"
                )
        seen[number] = (path.name, str(status))
        titles[path.name] = str(fields.get("title", ""))

        for req in fields.get("introduces", []) or []:
            if reqs and req not in reqs:
                problems.append(
                    f"{rel}: introduces {req}, which is not in registry.toml"
                )

        for target in fields.get("supersedes", []) or []:
            try:
                target_n = int(str(target).strip())
            except ValueError:
                problems.append(f"{rel}: supersedes {target!r} is not an RFC number")
                continue
            if not any(p.name.startswith(f"{target_n:04d}-") for p in rfc_dir.glob("*.md")):
                problems.append(f"{rel}: supersedes RFC-{target_n}, which does not exist")

    for note in notes:
        print(f"rfc-check: note: {note}", file=sys.stderr)
    return problems


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default=".", help="repository root")
    parser.add_argument("--json", action="store_true", help="machine-readable output")
    parser.add_argument("--index", action="store_true", help="print the RFC index")
    args = parser.parse_args(argv)

    root = Path(args.root).resolve()

    if args.index:
        rows = index(root)
        if args.json:
            print(json.dumps(rows, indent=2))
        else:
            print("| RFC | title | status | version |")
            print("|---|---|---|---|")
            for r in rows:
                v = f"r{r['revision']} @ {r['commit']}" if r["commit"] else "unversioned"
                print(f"| {r['rfc']} | {r['title']} | {r['status']} | {v} |")
        return 0

    problems = check(root)

    if args.json:
        print(json.dumps({"ok": not problems, "problems": problems}, indent=2))
    elif problems:
        for p in problems:
            print(f"rfc-check: {p}", file=sys.stderr)
    else:
        print("rfc-check: ok")

    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
