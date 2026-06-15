#!/usr/bin/env python3
"""
REQ-status inventory + contradiction lint for source-level `//!` status rows.

The pinned-SHA doc-drift gate catches routed design docs that are stale relative
to governed files. It does not catch semantic contradictions inside the long
source comments themselves, such as one module saying a REQ is NOT-STARTED while
the owning forge module says the exact same REQ is SHIPPED.

This gate parses markdown table rows of the form:

    //! | REQ-... | SHIPPED|NOT-STARTED | evidence |

and applies three deliberately mechanical checks:

  * exact requirement labels must not carry conflicting statuses;
  * NOT-STARTED rows must cite a blocker or explicit future/deferred scope;
  * SHIPPED rows must cite at least one backtick-quoted file path or symbol that
    resolves in the current tree.

Usage:

    python3 tooling/req-status.py [--root <repo>] [--inventory] [--json]
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import asdict, dataclass
from pathlib import Path


REQ_ROW_RE = re.compile(
    r"^\s*//!\s*\|\s*(?P<req>[^|]+?)\s*\|\s*"
    r"(?P<status>SHIPPED|NOT-STARTED)\s*\|\s*"
    r"(?P<evidence>.*?)\s*\|\s*$"
)

FUTURE_SCOPE_RE = re.compile(
    r"#\d+|blocker|prereq|future|follow-up|deferred|out of scope|"
    r"next dispatch|not implemented|unbuilt|v1\.1|stage|epic|lands when|scheduled",
    re.IGNORECASE,
)

PATHLIKE_RE = re.compile(r"[/\\]|\.([A-Za-z0-9]+)$")

SOURCE_SUFFIXES = {
    ".rs",
    ".lean",
    ".py",
    ".sh",
    ".md",
    ".toml",
    ".json",
    ".th",
    ".yml",
    ".yaml",
}

SKIP_DIRS = {
    ".git",
    ".pytest_cache",
    "target",
    ".lake",
    "__pycache__",
}

KEYWORDS = {
    "AC",
    "REQ",
    "SHIPPED",
    "NOT",
    "STARTED",
    "Some",
    "None",
    "Option",
    "Result",
    "Vec",
    "String",
    "true",
    "false",
    "pub",
    "fn",
    "struct",
    "enum",
    "mod",
    "impl",
    "crate",
    "self",
    "super",
}


@dataclass(frozen=True)
class Row:
    path: str
    line: int
    req: str
    status: str
    evidence: str


@dataclass(frozen=True)
class Issue:
    kind: str
    path: str
    line: int
    req: str
    detail: str


def iter_rs_files(root: Path):
    for p in sorted(root.rglob("*.rs")):
        rel_parts = p.relative_to(root).parts
        if any(part in SKIP_DIRS for part in rel_parts):
            continue
        yield p


def crate_root_for(row_path: Path) -> Path:
    """Return the likely crate root for a row source path."""
    if row_path.parent.name == "src":
        return row_path.parent.parent
    return row_path.parent


def load_rows(root: Path) -> list[Row]:
    rows: list[Row] = []
    for p in iter_rs_files(root):
        rel = p.relative_to(root).as_posix()
        try:
            text = p.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            text = p.read_text(encoding="utf-8", errors="ignore")
        for line_no, line in enumerate(text.splitlines(), 1):
            m = REQ_ROW_RE.match(line)
            if not m:
                continue
            rows.append(
                Row(
                    path=rel,
                    line=line_no,
                    req=m.group("req").strip(),
                    status=m.group("status"),
                    evidence=m.group("evidence").strip(),
                )
            )
    return rows


def searchable_text(root: Path) -> str:
    """Repo text with REQ rows stripped, so a fake citation cannot satisfy itself."""
    chunks: list[str] = []
    for p in sorted(root.rglob("*")):
        if not p.is_file() or p.suffix not in SOURCE_SUFFIXES:
            continue
        rel_parts = p.relative_to(root).parts
        if any(part in SKIP_DIRS for part in rel_parts):
            continue
        try:
            lines = p.read_text(encoding="utf-8").splitlines()
        except UnicodeDecodeError:
            lines = p.read_text(encoding="utf-8", errors="ignore").splitlines()
        chunks.extend(line for line in lines if not REQ_ROW_RE.match(line))
    return "\n".join(chunks)


def backtick_citations(evidence: str) -> list[str]:
    return [c.strip() for c in re.findall(r"`([^`]+)`", evidence) if c.strip()]


def candidate_paths(citation: str, root: Path, row: Row) -> list[Path]:
    token = citation.strip().strip(".,;:()[]{}")
    if not token or " " in token:
        return []
    if token.startswith("./"):
        token = token[2:]
    if "#" in token:
        token = token.split("#", 1)[0]
    if "::" in token and "/" not in token:
        return []
    if not PATHLIKE_RE.search(token):
        return []

    row_path = root / row.path
    crate_root = crate_root_for(row_path)
    return [
        root / token,
        crate_root / token,
        row_path.parent / token,
    ]


def citation_resolves_path(citation: str, root: Path, row: Row) -> bool:
    return any(p.exists() for p in candidate_paths(citation, root, row))


def citation_resolves_symbol(citation: str, haystack: str) -> bool:
    identifiers = re.findall(r"[A-Za-z_][A-Za-z0-9_]*", citation)
    for ident in reversed(identifiers):
        if len(ident) < 3 or ident in KEYWORDS:
            continue
        if re.search(rf"(?<![A-Za-z0-9_]){re.escape(ident)}(?![A-Za-z0-9_])", haystack):
            return True
    return False


def has_resolving_evidence(row: Row, root: Path, haystack: str) -> bool:
    citations = backtick_citations(row.evidence)
    return any(
        citation_resolves_path(citation, root, row)
        or citation_resolves_symbol(citation, haystack)
        for citation in citations
    )


def lint_rows(root: Path, rows: list[Row]) -> list[Issue]:
    issues: list[Issue] = []
    haystack = searchable_text(root)

    by_req: dict[str, list[Row]] = {}
    for row in rows:
        by_req.setdefault(row.req, []).append(row)

    for req, grouped in sorted(by_req.items()):
        statuses = sorted({row.status for row in grouped})
        if len(statuses) <= 1:
            continue
        locations = ", ".join(
            f"{row.path}:{row.line}={row.status}" for row in grouped
        )
        for row in grouped:
            issues.append(
                Issue(
                    "STATUS-CONFLICT",
                    row.path,
                    row.line,
                    row.req,
                    f"exact requirement label has conflicting statuses: {locations}",
                )
            )

    for row in rows:
        if row.status == "NOT-STARTED" and not FUTURE_SCOPE_RE.search(row.evidence):
            issues.append(
                Issue(
                    "NOT-STARTED-SCOPE",
                    row.path,
                    row.line,
                    row.req,
                    "NOT-STARTED evidence must cite a blocker or explicit future/deferred scope",
                )
            )
        if row.status == "SHIPPED" and not has_resolving_evidence(row, root, haystack):
            issues.append(
                Issue(
                    "UNRESOLVED-SHIPPED-EVIDENCE",
                    row.path,
                    row.line,
                    row.req,
                    "SHIPPED evidence must cite at least one resolving backtick-quoted file path or symbol",
                )
            )

    return sorted(issues, key=lambda i: (i.path, i.line, i.kind))


def render_inventory(rows: list[Row]) -> str:
    out = []
    for row in sorted(rows, key=lambda r: (r.path, r.line)):
        out.append(f"{row.status}  {row.path}:{row.line}  {row.req}")
    return "\n".join(out)


def render_issues(issues: list[Issue]) -> str:
    out = []
    for issue in issues:
        out.append(
            f"{issue.kind}  {issue.path}:{issue.line}  {issue.req}\n"
            f"  {issue.detail}"
        )
    return "\n".join(out)


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default=".", help="repo root to scan")
    parser.add_argument(
        "--inventory",
        action="store_true",
        help="print the normalized row inventory instead of only lint findings",
    )
    parser.add_argument("--json", action="store_true", help="emit JSON")
    args = parser.parse_args(argv)

    root = Path(args.root).resolve()
    rows = load_rows(root)
    issues = lint_rows(root, rows)

    if args.json:
        print(
            json.dumps(
                {
                    "rows": [asdict(row) for row in rows],
                    "issues": [asdict(issue) for issue in issues],
                },
                indent=2,
                sort_keys=True,
            )
        )
    elif args.inventory:
        inventory = render_inventory(rows)
        if inventory:
            print(inventory)
        if issues:
            print("\nREQ status lint failed:\n" + render_issues(issues), file=sys.stderr)
    elif issues:
        print("REQ status lint failed:\n" + render_issues(issues))
    else:
        print(f"REQ status lint clean: {len(rows)} row(s) checked")

    return 1 if issues else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
