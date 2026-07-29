#!/usr/bin/env python3
"""Verify the hook wiring declared by this repository.

The check reads ``.claude/settings.json`` and confirms that every entry in
``REQUIRED_HOOKS`` has a covering matcher and an existing script. A missing or
malformed setting is a finding because Claude Code cannot run that hook.

Exit codes:

* 0: every required hook is wired and present
* 1: missing wiring, a missing script, or invalid settings
* 3: the repository could not be inspected

Usage: ``python3 tooling/control-plane-check.py [--root <repo-toplevel>]``

See ``.design/tooling/control-plane.md`` for the requirements. The check runs in
CI and through ``make control-plane``; it is not itself a hook or part of
``make audit``. Customize ``SETTINGS_RELPATH`` and ``REQUIRED_HOOKS`` below.
"""

import json
import re
import subprocess
import sys
from pathlib import Path


# Project settings

# Repo-relative path to the tracked Claude Code settings file (the subject).
SETTINGS_RELPATH = ".claude/settings.json"

# The wirings this project's docs claim are live. Each entry is:
#   event   — the settings.json hook event key
#   tools   — the tool names the matcher must cover
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

# Implementation

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
    """The check could not determine the answer; maps to exit 3 (REQ-5).

    This covers a missing git executable or an invalid repository.
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
    """Return the command strings in a settings.json hook entry.

    A malformed entry yields no commands and is therefore reported as missing
    wiring.
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
