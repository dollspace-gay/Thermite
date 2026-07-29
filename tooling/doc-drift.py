#!/usr/bin/env python3
"""Check whether routed design documents are current with their source files.

Routes come from ``tooling/spec-routes.toml``. Each routed document must contain
either an ``audited-content-sha256`` pin or a legacy ``audited-sha`` commit pin.
Content pins are compared with a digest of the governed files. Legacy pins are
checked against commits that touched those files after the pinned commit.

Results are sorted by document and file:

* 0: every document is pinned and current
* 1: drift, a missing pin, or an invalid pin was found
* 3: the check could not run reliably

Usage: ``python3 tooling/doc-drift.py [--root <repo-toplevel>]``

The detailed rules are in ``.design/tooling/doc-drift-tripwire.md``.
This is a standalone/CI check, not a Claude Code hook or part of ``make audit``.
Customize ``ROUTES_RELPATH`` and the pin expressions below when adapting it.
"""

import hashlib
import re
import subprocess
import sys
from pathlib import Path

try:
    import tomllib  # Python 3.11+
except ImportError:  # pragma: no cover - exercised via the env-failure path
    tomllib = None


# Project settings

# Repo-relative path to the route table (the enumeration source, REQ-1).
ROUTES_RELPATH = "tooling/spec-routes.toml"

# Preferred content pin: a deterministic aggregate SHA-256 over the doc's
# governed file set. This makes drift a data-consistency check, independent of
# merge topology.
CONTENT_PIN_FIELD_RE = re.compile(
    r"^audited-content-sha256:\s*([0-9a-f]{64})\b",
    re.MULTILINE,
)
CONTENT_PIN_ANY_RE = re.compile(r"^audited-content-sha256:\s*(\S+)", re.MULTILINE)

# Legacy commit pin, per REQ-5: the first matching line in a doc's header.
# Use all 40 hex digits to avoid ambiguity as the repository grows.
PIN_FIELD_RE = re.compile(r"^audited-sha:\s*([0-9a-f]{40})\b", re.MULTILINE)

# Implementation

# Defect classes (REQ-7/REQ-8). The literal tokens the report emits and the
# oracle asserts.
DRIFT = "DRIFT"
MISSING_PIN = "MISSING-PIN"
INVALID_PIN = "INVALID-PIN"
CURRENT = "CURRENT"

# Exit codes (REQ-9).
EXIT_OK = 0
EXIT_FAIL = 1
EXIT_INCONCLUSIVE = 3


class EnvironmentError3(Exception):
    """The check could not determine the answer; maps to exit 3 (REQ-9).

    This covers missing git, an invalid repository, missing tomllib, and an
    unreadable route table.
    """


# --- git helpers (every subprocess exit status is inspected, R-CODE-4) ------

def _run_git(root, args):
    """Run `git <args>` in `root`; return (returncode, stdout, stderr).

    Raise EnvironmentError3 when git cannot be invoked. Otherwise return git's
    status to the caller for interpretation.
    """
    try:
        proc = subprocess.run(
            ["git", "-C", str(root), *args],
            capture_output=True,
            text=True,
        )
    except (FileNotFoundError, PermissionError, OSError) as exc:
        raise EnvironmentError3(f"git could not be invoked: {exc}") from exc
    return proc.returncode, proc.stdout, proc.stderr


def _git_toplevel(start):
    """Resolve the git worktree toplevel containing `start`, or raise exit-3."""
    rc, out, _ = _run_git(start, ["rev-parse", "--show-toplevel"])
    if rc != 0:
        raise EnvironmentError3(f"not inside a git repository: {start}")
    top = out.strip()
    if not top:
        raise EnvironmentError3(f"could not resolve git toplevel for: {start}")
    return Path(top)


def _resolve_head(root):
    """The HEAD commit SHA, or exit-3 (e.g. an unborn branch / not a repo)."""
    rc, out, _ = _run_git(root, ["rev-parse", "--verify", "HEAD^{commit}"])
    if rc != 0:
        raise EnvironmentError3("could not resolve HEAD (no commits / not a repo)")
    return out.strip()


def _pin_resolves(root, pin):
    """True iff `pin` resolves to a commit object (REQ-6d, first clause)."""
    rc, _, _ = _run_git(root, ["rev-parse", "--verify", f"{pin}^{{commit}}"])
    return rc == 0


def _pin_is_ancestor(root, pin):
    """True iff `pin` is an ancestor of HEAD (REQ-6d, second clause).

    `git merge-base --is-ancestor` exits 0 (yes) / 1 (no) / other (error).
    An error status is an environment failure (exit 3), never silently "no".
    """
    rc, _, _ = _run_git(root, ["merge-base", "--is-ancestor", pin, "HEAD"])
    if rc == 0:
        return True
    if rc == 1:
        return False
    raise EnvironmentError3(
        f"git merge-base --is-ancestor exited {rc} for pin {pin}"
    )


def _intervening_commits(root, pin, pathspec):
    """The commits in <pin>..HEAD touching `pathspec`, newest-first.

    Returns a list of (sha, subject). Empty => CURRENT (REQ-6 unbuilt-file
    rule: a never-committed file yields an empty list and is CURRENT). A
    non-zero git status here is an environment failure (exit 3), never
    collapsed to "no drift".
    """
    rc, out, err = _run_git(
        root,
        ["log", "--full-history", "--format=%H %s", f"{pin}..HEAD", "--", pathspec],
    )
    if rc != 0:
        raise EnvironmentError3(
            f"git log --full-history {pin}..HEAD -- {pathspec} exited {rc}: "
            f"{err.strip()}"
        )
    commits = []
    for line in out.splitlines():
        line = line.rstrip("\n")
        if not line:
            continue
        sha, _, subject = line.partition(" ")
        commits.append((sha, subject))
    return commits


# --- route table ------------------------------------------------------------

def load_doc_files(root):
    """Invert the route table to doc -> sorted(set(governed file patterns)).

    Raise EnvironmentError3 when tomllib is unavailable or the route table
    cannot be read and validated (REQ-1, REQ-6b, REQ-9).
    """
    if tomllib is None:
        raise EnvironmentError3("tomllib is unavailable (Python < 3.11)")
    p = root / ROUTES_RELPATH
    if not p.is_file():
        raise EnvironmentError3(f"route table not found: {ROUTES_RELPATH}")
    try:
        with open(p, "rb") as f:
            data = tomllib.load(f)
    except (OSError, tomllib.TOMLDecodeError) as exc:
        raise EnvironmentError3(
            f"route table unreadable ({ROUTES_RELPATH}): {exc}"
        ) from exc

    # TOML syntax alone does not validate the route-table shape. Check it here
    # so malformed input is reported as inconclusive instead of raising while
    # the entries are traversed (REQ-9).
    routes = data.get("route", [])
    if not isinstance(routes, list):
        raise EnvironmentError3(
            f"route table {ROUTES_RELPATH} is wrong-shaped: `route` must be a "
            f"list of [[route]] tables, got {type(routes).__name__}"
        )

    doc_files = {}
    for i, route in enumerate(routes):
        if not isinstance(route, dict):
            raise EnvironmentError3(
                f"route table {ROUTES_RELPATH} is wrong-shaped: route entry "
                f"#{i} must be a [[route]] table, got {type(route).__name__}"
            )
        design = route.get("design")
        pattern = route.get("crate_pattern")
        # Both fields are required by the spec-routes.toml schema. Reject absent,
        # empty, or non-string values so a malformed route cannot reduce coverage
        # while the check still succeeds (#261).
        for field, value in (("design", design), ("crate_pattern", pattern)):
            if value is not None and not isinstance(value, str):
                raise EnvironmentError3(
                    f"route table {ROUTES_RELPATH} is wrong-shaped: route entry "
                    f"#{i} `{field}` must be a string, got "
                    f"{type(value).__name__}"
                )
            if not value:
                what = "is missing" if value is None else "is empty"
                raise EnvironmentError3(
                    f"route table {ROUTES_RELPATH} is wrong-shaped: route entry "
                    f"#{i} required field `{field}` {what} (both `crate_pattern` "
                    f"and `design` are required per the schema header)"
                )
        doc_files.setdefault(design, set()).add(pattern)
    if not doc_files:
        # With no routes, the check cannot establish that routed docs are current.
        # Treat that case as inconclusive (REQ-9).
        raise EnvironmentError3(
            f"route table {ROUTES_RELPATH} yielded zero routed docs — nothing "
            f"to check; an empty enumeration source is INCONCLUSIVE, not a pass"
        )
    return {doc: sorted(files) for doc, files in doc_files.items()}


def extract_pins(root, doc_relpath):
    """Pins in `doc_relpath`: (content_pin, bad_content_pin, commit_pin).

    A doc that does not exist on disk has no pins -> MISSING-PIN; the doc is
    named, never a traceback. A malformed content pin is distinct from an absent
    one so a typo cannot silently fall back to a legacy commit pin.
    """
    p = root / doc_relpath
    try:
        text = p.read_text(encoding="utf-8", errors="replace")
    except (OSError, FileNotFoundError):
        return None, None, None
    content_m = CONTENT_PIN_FIELD_RE.search(text)
    bad_content_m = CONTENT_PIN_ANY_RE.search(text) if content_m is None else None
    commit_m = PIN_FIELD_RE.search(text)
    return (
        content_m.group(1) if content_m else None,
        bad_content_m.group(1) if bad_content_m else None,
        commit_m.group(1) if commit_m else None,
    )


def _pathspec_for(pattern):
    """REQ-6e: literal path -> `<f>`; any glob -> `:(glob)<f>`."""
    if "*" in pattern or "?" in pattern or "[" in pattern:
        return f":(glob){pattern}"
    return pattern


def _is_glob(pattern):
    return "*" in pattern or "?" in pattern or "[" in pattern


def _content_paths(root, pattern):
    """Repo-relative file paths represented by `pattern`, sorted."""
    if _is_glob(pattern):
        try:
            matches = sorted(
                p.relative_to(root).as_posix()
                for p in root.glob(pattern)
                if p.is_file()
            )
        except (OSError, ValueError) as exc:
            raise EnvironmentError3(
                f"could not expand content-hash glob {pattern}: {exc}"
            ) from exc
        return matches

    p = root / pattern
    return [pattern] if p.is_file() else []


def _content_digest(root, patterns):
    """Deterministic aggregate digest for a doc's governed file set."""
    digest = hashlib.sha256()
    digest.update(b"doc-drift-content-v1\0")
    for pattern in patterns:
        digest.update(b"pattern\0")
        digest.update(pattern.encode("utf-8", errors="surrogateescape"))
        digest.update(b"\0")
        paths = _content_paths(root, pattern)
        if not paths:
            digest.update(b"missing\0")
            continue
        for relpath in paths:
            digest.update(b"file\0")
            digest.update(relpath.encode("utf-8", errors="surrogateescape"))
            digest.update(b"\0")
            try:
                data = (root / relpath).read_bytes()
            except OSError as exc:
                raise EnvironmentError3(
                    f"could not read governed file for content hash {relpath}: {exc}"
                ) from exc
            digest.update(hashlib.sha256(data).hexdigest().encode("ascii"))
            digest.update(b"\0")
    return digest.hexdigest()


# --- the check --------------------------------------------------------------

def evaluate(root):
    """Run the gate over `root`; return (exit_code, report_lines).

    Deterministic: docs sorted by path, files sorted by path within each doc
    (R-CODE-5 / AC-8). Never raises for a doc-level defect — only an
    environment failure escapes as EnvironmentError3 (exit 3).
    """
    doc_files = load_doc_files(root)
    _resolve_head(root)  # exit-3 early if HEAD is unresolvable

    lines = []
    failed = False

    for doc in sorted(doc_files):
        files = doc_files[doc]
        content_pin, bad_content_pin, pin = extract_pins(root, doc)

        if bad_content_pin is not None:
            lines.append(
                f"{INVALID_PIN}  {doc}  audited-content-sha256 "
                f"{bad_content_pin} is not a 64-hex SHA-256 digest"
            )
            failed = True
            continue

        if content_pin is not None:
            current = _content_digest(root, files)
            if current == content_pin:
                lines.append(f"{CURRENT}  {doc}  (content-sha256 {content_pin})")
                continue
            lines.append(
                f"{DRIFT}  {doc}  content-sha256 {content_pin}  "
                f"current {current}  governed file pattern(s):"
            )
            for pattern in files:
                lines.append(f"    {pattern}")
            failed = True
            continue

        if pin is None:
            lines.append(
                f"{MISSING_PIN}  {doc}  "
                "(no audited-content-sha256: or audited-sha: line)"
            )
            failed = True
            continue

        if not _pin_resolves(root, pin):
            lines.append(
                f"{INVALID_PIN}  {doc}  pin {pin} does not resolve to a commit"
            )
            failed = True
            continue

        if not _pin_is_ancestor(root, pin):
            lines.append(
                f"{INVALID_PIN}  {doc}  pin {pin} is not an ancestor of HEAD"
            )
            failed = True
            continue

        drifted = []
        for pattern in files:
            commits = _intervening_commits(root, pin, _pathspec_for(pattern))
            if commits:
                drifted.append((pattern, commits))

        if not drifted:
            lines.append(f"{CURRENT}  {doc}  (pin {pin})")
            continue

        failed = True
        for pattern, commits in drifted:
            lines.append(
                f"{DRIFT}  {doc}  pin {pin}  governed file {pattern} "
                f"has {len(commits)} intervening commit(s):"
            )
            for sha, subject in commits:
                lines.append(f"    {sha} {subject}")

    return (EXIT_FAIL if failed else EXIT_OK), lines


# --- main -------------------------------------------------------------------

def _parse_args(argv):
    root_arg = None
    i = 0
    while i < len(argv):
        a = argv[i]
        if a == "--root":
            if i + 1 >= len(argv):
                raise EnvironmentError3("--root requires a path argument")
            root_arg = argv[i + 1]
            i += 2
        elif a.startswith("--root="):
            root_arg = a[len("--root="):]
            i += 1
        elif a in ("-h", "--help"):
            print(__doc__)
            sys.exit(EXIT_OK)
        else:
            raise EnvironmentError3(f"unknown argument: {a}")
        i += 1
    return root_arg


def main(argv=None):
    argv = list(sys.argv[1:] if argv is None else argv)
    try:
        root_arg = _parse_args(argv)
        start = Path(root_arg) if root_arg else Path.cwd()
        if not start.exists():
            raise EnvironmentError3(f"--root path does not exist: {start}")
        root = _git_toplevel(start)
        exit_code, lines = evaluate(root)
    except EnvironmentError3 as exc:
        # REQ-9: environment failure is exit 3, never a traceback, never
        # fail-open. Diagnostics go to stderr; stdout stays report-only so
        # AC-8 byte-identity is unaffected.
        print(f"doc-drift: INCONCLUSIVE (exit 3): {exc}", file=sys.stderr)
        sys.exit(EXIT_INCONCLUSIVE)

    for line in lines:
        print(line)
    sys.exit(exit_code)


if __name__ == "__main__":
    main()
