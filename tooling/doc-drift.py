#!/usr/bin/env python3
"""
doc-drift tripwire — pinned-SHA freshness for every routed design doc.

The .design/ docs are the per-component contracts (goal.md authority chain),
and spec-discipline.py guarantees they EXIST and are READ before a routed
edit — but nothing checks their CONTENT is still true of the code. They drift
silently. This gate converts that staleness from a silent failure into a loud,
gated one (the same move #[slag] makes for unverified code, thermite-design.md
§8): every routed design doc pins an `audited-sha:` commit, and this gate FAILS
whenever any file a doc governs has been committed since the doc's pin.

The rule, precisely (governed by .design/tooling/doc-drift-tripwire.md):

  1. Enumerate the routed docs: the deduplicated `design` fields of every
     [[route]] in tooling/spec-routes.toml, each inverted to its governed file
     set = the union of that doc's routes' crate_patterns (REQ-1, REQ-6b).
  2. Extract each doc's pin: the first line matching
     `^audited-sha:\\s*([0-9a-f]{40})\\b` in the doc's HTML-comment header
     (REQ-5).
  3. Validate the pin: it must resolve to a commit
     (`git rev-parse --verify <P>^{commit}`) AND be an ancestor of HEAD
     (`git merge-base --is-ancestor <P> HEAD`) — else INVALID-PIN (REQ-6d, 8).
  4. Drift predicate (commit-set, never commit-date — decision 2): a governed
     file f has drifted iff `git log --format=%H <P>..HEAD -- <pathspec>` is
     non-empty. Literal paths use pathspec `<f>`; glob patterns use `:(glob)<f>`
     (REQ-6e). A file with no commits in <P>..HEAD — including a file never
     committed at all — is CURRENT, not drift (REQ-6 unbuilt-file rule).
  5. Report (REQ-7) deterministically sorted by doc path, then file path
     (R-CODE-5), and exit per the REQ-9 contract:
         0 = every routed doc pinned and current;
         1 = at least one DRIFT / MISSING-PIN / INVALID-PIN;
         3 = the gate could not determine the answer (no git / not a repo /
             tomllib absent / spec-routes.toml unreadable) — the audit's
             INCONCLUSIVE precedent (scripts/audit.sh, REQ-3/REQ-9). A CI gate
             that fails open is a silent pass, so an environment failure is
             never collapsed to "no drift" (R-HONEST-3, R-CODE-4).

NOT a Claude-Code hook (decision 5): invoked via `make doc-drift` and runnable
standalone. NOT part of `make audit` — doc freshness is a development-discipline
invariant, not a link in the proof-trust chain. scripts/audit.sh is untouched
by this component (AC-7).

Usage:  python3 tooling/doc-drift.py [--root <repo-toplevel>]

  --root  the repo to check (default: the git toplevel of the cwd). The
          production invocation is flagless; --root keeps the fixture tests
          hermetic.

See:
  .design/tooling/doc-drift-tripwire.md  (the governing doc — REQ-5..REQ-11)
  goal.md                                 (authority chain; R-CODE-4/5, R-HONEST-3)
  tooling/spec-routes.toml                (the route table — single source of truth)
  scripts/audit.sh                        (the exit-3 INCONCLUSIVE precedent)

PROJECT CUSTOMIZATION:
  Edit ROUTES_RELPATH, PIN_FIELD_RE below.
"""

import re
import subprocess
import sys
from pathlib import Path

try:
    import tomllib  # Python 3.11+
except ImportError:  # pragma: no cover - exercised via the env-failure path
    tomllib = None


# =====================================================================
# PROJECT CUSTOMIZATION — edit these constants for your project
# =====================================================================

# Repo-relative path to the route table (the enumeration source, REQ-1).
ROUTES_RELPATH = "tooling/spec-routes.toml"

# The pin field, per REQ-5: the FIRST line matching this in a doc's header.
# Full 40-hex (never the 8-hex short form) so a pin can never go ambiguous as
# the repo grows.
PIN_FIELD_RE = re.compile(r"^audited-sha:\s*([0-9a-f]{40})\b", re.MULTILINE)

# =====================================================================
# Implementation — generally no edits needed below this line
# =====================================================================

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
    """The gate could not determine the answer — maps to exit 3 (REQ-9).

    Raised (never a traceback to the user) for: git absent / not a repo /
    tomllib absent / spec-routes.toml unreadable. A CI gate that fails open is
    a silent pass, so these are INCONCLUSIVE, never "no drift".
    """


# --- git helpers (every subprocess exit status is inspected, R-CODE-4) ------

def _run_git(root, args):
    """Run `git <args>` in `root`; return (returncode, stdout, stderr).

    Raises EnvironmentError3 iff git itself cannot be invoked (absent from
    PATH / not executable) — an environment failure, exit 3. A non-zero
    returncode from git that DOES run is returned to the caller to interpret;
    it is never an environment failure on its own.
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
        ["log", "--format=%H %s", f"{pin}..HEAD", "--", pathspec],
    )
    if rc != 0:
        raise EnvironmentError3(
            f"git log {pin}..HEAD -- {pathspec} exited {rc}: {err.strip()}"
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

    REQ-1 / REQ-6b. Raises EnvironmentError3 on an unreadable / unparseable
    route table or absent tomllib (a gate that fails open is a silent pass).
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

    # Shape validation (REQ-9): a table that PARSES as TOML can still be the
    # wrong SHAPE (e.g. `route = 5`, or `route = ["a"]`). Iterating such a value
    # would raise a bare TypeError/AttributeError -> unhandled traceback, exit 1
    # (the DRIFT class). A malformed enumeration source is an ENVIRONMENT failure
    # (the "spec-routes.toml unreadable" case), so it is INCONCLUSIVE (exit 3),
    # never a drift finding and never a traceback (R-HONEST-3: a gate that fails
    # open is a silent pass). Validate before the loop; name the defect loudly.
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
        for field, value in (("design", design), ("crate_pattern", pattern)):
            if value is not None and not isinstance(value, str):
                raise EnvironmentError3(
                    f"route table {ROUTES_RELPATH} is wrong-shaped: route entry "
                    f"#{i} `{field}` must be a string, got "
                    f"{type(value).__name__}"
                )
        if not design or not pattern:
            continue
        doc_files.setdefault(design, set()).add(pattern)
    if not doc_files:
        # REQ-9 / R-HONEST-3: the route table is the enumeration source, and an
        # empty one means there is NOTHING to check — not "every routed doc is
        # current". Exiting 0 here would be a vacuous green (fail-open silent
        # pass), so an empty/usable-but-route-less table is INCONCLUSIVE (3),
        # exactly like an unreadable one. ("The tool never exits 0 without
        # having checked all 48 docs.")
        raise EnvironmentError3(
            f"route table {ROUTES_RELPATH} yielded zero routed docs — nothing "
            f"to check; an empty enumeration source is INCONCLUSIVE, not a pass"
        )
    return {doc: sorted(files) for doc, files in doc_files.items()}


def extract_pin(root, doc_relpath):
    """The first REQ-5 pin in `doc_relpath`, or None if the doc has no pin.

    A doc that does not exist on disk has no pin (None) -> MISSING-PIN; the
    doc is named, never a traceback.
    """
    p = root / doc_relpath
    try:
        text = p.read_text(encoding="utf-8", errors="replace")
    except (OSError, FileNotFoundError):
        return None
    m = PIN_FIELD_RE.search(text)
    return m.group(1) if m else None


def _pathspec_for(pattern):
    """REQ-6e: literal path -> `<f>`; any glob -> `:(glob)<f>`."""
    if "*" in pattern or "?" in pattern or "[" in pattern:
        return f":(glob){pattern}"
    return pattern


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
        pin = extract_pin(root, doc)

        if pin is None:
            lines.append(f"{MISSING_PIN}  {doc}  (no audited-sha: line)")
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
