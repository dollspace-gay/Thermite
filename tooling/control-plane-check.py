#!/usr/bin/env python3
"""
control-plane gate — the gate that guards the gates.

`tooling/spec-discipline.py` and `tooling/anti-pattern-gate.py` are agent-facing
PreToolUse/PostToolUse hooks. They only ever fire because `.claude/settings.json`
WIRES them. That wiring is a single tracked JSON file that `crosslink init`
regenerates from a generic template — so a routine `crosslink init` (or a
`--force` re-init) silently drops the project-specific entries and leaves the
crosslink-generic ones. That is exactly what commit 5581b65f did on 2026-06-21:
both gates were dormant for the whole Stage-3 arc while README.md:172, goal.md
§Spec-discipline, and all four `.claude/agents/acto-*.md` files kept asserting
"they enforce automatically — no setup" (crosslink #93).

Nothing could catch it: `doc-drift.py` only checks files reachable from
`tooling/spec-routes.toml`, and no route covered the control plane. The design
layer governed everything except the file that decides whether the governance
runs.

This gate closes that loop. It asserts — as a CI-enforced, deterministic check —
that every hook this project's documentation CLAIMS is live is actually wired in
the tracked `settings.json`, and that the script each entry names exists on disk.
An "asserted enforcement that isn't" becomes a red build instead of a quiet
regression (the R-HONEST-3 move: a gate that fails open is a silent pass).

The rule, precisely (governed by .design/tooling/control-plane.md):

  1. Load the tracked `.claude/settings.json` (REQ-1). It must exist and parse:
     Claude Code loads NO hooks from a malformed settings file, so unparseable
     is gate-dead, i.e. a FINDING (exit 1), never INCONCLUSIVE.
  2. For each required wiring (event, tools, script) in REQUIRED_HOOKS below,
     find a hook entry under that event whose `matcher` COVERS every required
     tool and one of whose commands names `script` (REQ-2). Matcher coverage is
     alternative-set containment (`Write|Edit|Bash` covers a `Write|Edit`
     requirement), so a superset matcher passes and a reordering does not
     false-fail — but a matcher that drops a required tool is a MISSING-WIRING.
  3. Assert each required script exists at its repo-relative path (REQ-3). A
     wired-but-absent hook is the same dead gate as an unwired one: every
     command is `if [ -f "$HOOK" ]`-guarded, so an absent script degrades to a
     silent no-op exit 0.
  4. Report (REQ-4) deterministically in REQUIRED_HOOKS order, and exit per the
     REQ-5 contract:
         0 = every required hook wired and present;
         1 = at least one MISSING-WIRING / MISSING-SCRIPT / UNPARSEABLE;
         3 = the gate could not determine the answer (no git / not a repo) —
             the audit's INCONCLUSIVE precedent (scripts/audit.sh, doc-drift.py
             REQ-9). An environment failure is never collapsed to "all wired".

A finding prints the exact JSON entry to restore, so the fix is a paste, not an
archaeology dig through `git show 5581b65f`.

NOT a Claude-Code hook: invoked by CI and by `make control-plane`. Deliberately
NOT part of `make audit` — hook wiring is a development-discipline invariant,
not a link in the proof-trust chain (the doc-drift decision-5 precedent).

Usage:  python3 tooling/control-plane-check.py [--root <repo-toplevel>]

  --root  the repo to check (default: the git toplevel of the cwd). The
          production invocation is flagless; --root keeps the fixture tests
          hermetic.

See:
  .design/tooling/control-plane.md  (the governing doc — REQ-1..REQ-5)
  goal.md                            (authority chain; R-CODE-4/5, R-HONEST-3)
  tooling/doc-drift.py               (the sibling gate; exit-3 precedent)
  tooling/spec-routes.toml           (routes the control plane under doc-drift)

PROJECT CUSTOMIZATION:
  Edit SETTINGS_RELPATH, REQUIRED_HOOKS below.
"""

import json
import re
import subprocess
import sys
from pathlib import Path


# =====================================================================
# PROJECT CUSTOMIZATION — edit these constants for your project
# =====================================================================

# Repo-relative path to the tracked Claude Code settings file (the subject).
SETTINGS_RELPATH = ".claude/settings.json"

# The wirings this project's docs claim are live. Each entry is:
#   event   — the settings.json hook event key
#   tools   — the tool names the matcher MUST cover
#   script  — the repo-relative hook script the entry must invoke
#   claim   — the doc line asserting this hook enforces (named in the report,
#             so a finding points at the prose that would go false)
REQUIRED_HOOKS = (
    {
        "event": "PostToolUse",
        "tools": ("Read",),
        "script": "tooling/spec-discipline.py",
        "claim": "spec-discipline.py:16 'PostToolUse on Read -> records the Read'",
    },
    {
        "event": "PreToolUse",
        "tools": ("Write", "Edit"),
        "script": "tooling/spec-discipline.py",
        "claim": "goal.md R-XLATE-1/2/3 'enforced by tooling/spec-discipline.py'",
    },
    {
        "event": "PreToolUse",
        "tools": ("Write", "Edit"),
        "script": "tooling/anti-pattern-gate.py",
        "claim": "goal.md R-APG 'enforced by tooling/anti-pattern-gate.py'",
    },
)

# =====================================================================
# Implementation — generally no edits needed below this line
# =====================================================================

# Defect classes (REQ-4). The literal tokens the report emits and the oracle
# asserts.
MISSING_WIRING = "MISSING-WIRING"
MISSING_SCRIPT = "MISSING-SCRIPT"
UNPARSEABLE = "UNPARSEABLE"
WIRED = "WIRED"

# Exit codes (REQ-5), matching tooling/doc-drift.py.
EXIT_OK = 0
EXIT_FAIL = 1
EXIT_INCONCLUSIVE = 3

# A matcher is a regex alternation of tool names (`Write|Edit|Bash`). Split on
# `|` and strip regex-grouping punctuation so `(Write|Edit)` reads the same as
# `Write|Edit`. An absent matcher means "every tool" in Claude Code, which
# trivially covers any requirement.
_MATCHER_STRIP_RE = re.compile(r"[()\s^$]")


class EnvironmentError3(Exception):
    """The gate could not determine the answer — maps to exit 3 (REQ-5).

    Raised (never a traceback to the user) for: git absent / not a repo. A CI
    gate that fails open is a silent pass, so these are INCONCLUSIVE, never
    "every hook is wired".
    """


def _git_toplevel(start):
    """Resolve the git worktree toplevel containing `start`, or raise exit-3."""
    try:
        proc = subprocess.run(
            ["git", "-C", str(start), "rev-parse", "--show-toplevel"],
            capture_output=True,
            text=True,
        )
    except (FileNotFoundError, PermissionError, OSError) as exc:
        raise EnvironmentError3(f"git could not be invoked: {exc}") from exc
    if proc.returncode != 0:
        raise EnvironmentError3(f"not inside a git repository: {start}")
    top = proc.stdout.strip()
    if not top:
        raise EnvironmentError3(f"could not resolve git toplevel for: {start}")
    return Path(top)


def _matcher_covers(matcher, tools):
    """True iff `matcher` fires for every tool in `tools`.

    An absent/empty matcher matches every tool in Claude Code, so it covers any
    requirement. Otherwise this is alternative-set containment: a superset
    matcher (`Write|Edit|Bash`) covers `Write|Edit`, and order is irrelevant.
    """
    if matcher is None or not str(matcher).strip():
        return True
    alternatives = {
        _MATCHER_STRIP_RE.sub("", alt)
        for alt in str(matcher).split("|")
    }
    return all(tool in alternatives for tool in tools)


def _entry_commands(entry):
    """Every command string in a settings.json hook entry, shape-tolerantly.

    A wrong-shaped entry yields no commands rather than raising: the wiring it
    was supposed to provide then reads as MISSING (a loud finding), which is the
    honest answer for a settings file Claude Code could not act on either.
    """
    if not isinstance(entry, dict):
        return []
    hooks = entry.get("hooks", [])
    if not isinstance(hooks, list):
        return []
    commands = []
    for hook in hooks:
        if isinstance(hook, dict) and isinstance(hook.get("command"), str):
            commands.append(hook["command"])
    return commands


def _restore_snippet(required):
    """The JSON entry to paste back when a wiring is missing (REQ-4)."""
    guard = (
        'HOOK="$(git rev-parse --show-toplevel 2>/dev/null)/'
        + required["script"]
        + '"; if [ -f "$HOOK" ]; then python3 "$HOOK"; else exit 0; fi'
    )
    entry = {
        "hooks": [{"command": guard, "timeout": 5, "type": "command"}],
        "matcher": "|".join(required["tools"]),
    }
    body = json.dumps(entry, indent=2, sort_keys=True)
    return "\n".join(f"           {line}" for line in body.splitlines())


def evaluate(root):
    """Run the gate over `root`; return (exit_code, report_lines).

    Never raises for a defect in the SUBJECT (that is a finding); raises
    EnvironmentError3 only for an environment failure the caller maps to exit 3.
    """
    lines = []
    settings_path = root / SETTINGS_RELPATH

    if not settings_path.is_file():
        lines.append(
            f"{UNPARSEABLE}  {SETTINGS_RELPATH}  file not found — no hook is "
            f"wired, so every gate below is dormant"
        )
        return EXIT_FAIL, lines

    try:
        with open(settings_path, "rb") as f:
            settings = json.load(f)
    except (OSError, json.JSONDecodeError) as exc:
        # Claude Code loads NO hooks from a settings file it cannot parse, so
        # this is a dead gate (a finding), not an environment failure.
        lines.append(
            f"{UNPARSEABLE}  {SETTINGS_RELPATH}  {exc} — Claude Code loads no "
            f"hooks from an unparseable settings file; every gate is dormant"
        )
        return EXIT_FAIL, lines

    hooks_by_event = settings.get("hooks", {}) if isinstance(settings, dict) else {}
    if not isinstance(hooks_by_event, dict):
        lines.append(
            f"{UNPARSEABLE}  {SETTINGS_RELPATH}  `hooks` must be an object, got "
            f"{type(hooks_by_event).__name__} — every gate is dormant"
        )
        return EXIT_FAIL, lines

    failed = False
    for required in REQUIRED_HOOKS:
        event = required["event"]
        script = required["script"]
        tools = required["tools"]
        label = f"{event}/{'|'.join(tools)} -> {script}"

        entries = hooks_by_event.get(event, [])
        if not isinstance(entries, list):
            entries = []

        wired = any(
            _matcher_covers(entry.get("matcher") if isinstance(entry, dict) else None, tools)
            and any(script in cmd for cmd in _entry_commands(entry))
            for entry in entries
        )

        if not wired:
            failed = True
            lines.append(
                f"{MISSING_WIRING}  {label}\n"
                f"           the docs claim this fires: {required['claim']}\n"
                f"           restore this entry under hooks.{event} in "
                f"{SETTINGS_RELPATH}:\n{_restore_snippet(required)}"
            )
            continue

        if not (root / script).is_file():
            failed = True
            lines.append(
                f"{MISSING_SCRIPT}  {label}\n"
                f"           wired in {SETTINGS_RELPATH} but {script} is absent; "
                f"the `if [ -f \"$HOOK\" ]` guard degrades it to a silent no-op"
            )
            continue

        lines.append(f"{WIRED}  {label}")

    return (EXIT_FAIL if failed else EXIT_OK), lines


def _parse_args(argv):
    root = None
    rest = list(argv)
    while rest:
        arg = rest.pop(0)
        if arg == "--root":
            if not rest:
                raise SystemExit("--root requires a path argument")
            root = rest.pop(0)
        elif arg in ("-h", "--help"):
            print(__doc__.strip())
            raise SystemExit(EXIT_OK)
        else:
            raise SystemExit(f"unrecognized argument: {arg}")
    return root


def main(argv=None):
    argv = sys.argv[1:] if argv is None else argv
    try:
        root_arg = _parse_args(argv)
        root = Path(root_arg).resolve() if root_arg else _git_toplevel(Path.cwd())
        code, lines = evaluate(root)
    except EnvironmentError3 as exc:
        print(f"control-plane: INCONCLUSIVE — {exc}", file=sys.stderr)
        return EXIT_INCONCLUSIVE

    for line in lines:
        print(line)

    if code == EXIT_OK:
        print(
            f"control-plane: all {len(REQUIRED_HOOKS)} required hooks wired in "
            f"{SETTINGS_RELPATH}"
        )
    else:
        print(
            "control-plane: FAIL — a hook the docs claim enforces automatically "
            "is not wired. This is how crosslink #93 happened: `crosslink init` "
            "regenerates .claude/settings.json from a generic template and drops "
            "the project-specific entries. Re-add them (above) and re-run.",
            file=sys.stderr,
        )
    return code


if __name__ == "__main__":
    sys.exit(main())
