#!/usr/bin/env python3
"""Reject selected production-code anti-patterns before a Write or Edit.

The hook checks configured Rust source paths for runtime stubs, panics,
``unwrap``/``expect``, ownership escape hatches, and module-level lint
suppression. Each finding explains the preferred alternative.

Full Write content is scanned, excluding ``#[cfg(test)]`` blocks. Edit requests
scan only the replacement text because surrounding context is unavailable.
Proof-specific constructs are reviewed elsewhere because they also have valid
uses in generated or explicitly unverified code.

Customize ``PATTERNS`` and the ``TARGET_*`` path settings below.
"""

import json
import os
import re
import sys
from pathlib import Path


# Project settings

TARGET_CRATE_PREFIXES = ("thermite-",)
TARGET_CRATE_EXACT = ("forge",)
EXCLUDED_CRATES = ("thermite-test-utils",)
TARGET_EXTENSION = ".rs"

# Forbidden patterns

# Each entry: (regex, name, why-forbidden, architectural-alternative)
PATTERNS = [
    (
        re.compile(r"\btodo\s*!\s*\("),
        "the to-do macro",
        "Leaves a runtime stub that fires at the first call. goal.md "
        "forbids stubs in mainline (R-DEFER-9): a feature is either "
        "complete or a tracked NOT-STARTED blocker.",
        "Either implement the function fully, or remove the call site "
        "and file a blocker issue. The route table or REQ status table "
        "should reference the blocker.",
    ),
    (
        re.compile(r"\bunimplemented\s*!\s*\("),
        "the unimpl macro",
        "Same shape as the to-do macro: a runtime crash substituting "
        "for the engineering work. Forbidden in production code.",
        "Implement the body, or remove + file a blocker.",
    ),
    (
        re.compile(r"\bunreachable\s*!\s*\("),
        "the unreach macro",
        "Becomes a runtime crash if the supposedly-unreachable branch "
        "is actually reached. Most uses in match arms are a sign of an "
        "enum that should be exhaustively typed.",
        "Restructure the enum to make the case impossible at the type "
        "level. If that's truly not possible AND the invariant is "
        "documented, return a typed error variant instead of crashing.",
    ),
    (
        re.compile(r"\.\s*expect\s*\("),
        ".expect on Result/Option",
        "Forbidden in production code by goal.md R-CODE-2. It crashes "
        "on the error path; callers expect Result propagation.",
        "Propagate the error via `Result<T, ThermiteError>`. The error "
        "variant should name what specifically failed.",
    ),
    (
        re.compile(r"\.\s*unwrap\s*\("),
        ".unwrap on Result/Option",
        "Same as .expect - forbidden in production code (goal.md "
        "R-CODE-2). .unwrap is .expect without even saying why.",
        "Propagate the error via `Result<T, ThermiteError>`.",
    ),
    (
        re.compile(r"\bpanic\s*!\s*\("),
        "the panic macro",
        "Direct crash-out is forbidden in production code (goal.md "
        "R-CODE-2). A verifier toolchain must degrade and report, never "
        "abort.",
        "Propagate via `Result<T, ThermiteError>`. If this is a true "
        "internal-invariant violation, return a typed `Internal` "
        "variant so callers get a chance to handle it.",
    ),
    (
        re.compile(r"\bArc\s*<\s*Mutex\s*<"),
        "Arc<Mutex<T>>",
        "Wrapping a value in Arc<Mutex<T>> is the canonical 'I don't "
        "know what to do, just lock it' escape hatch. Almost always "
        "means a redesign was avoided.",
        "Consider: restructure ownership (single owner mutates, "
        "borrowers see immutable references), atomic for counters, "
        "message passing via channels, or a typed lock with a "
        "documented invariant (not a generic Mutex<T>).",
    ),
    (
        re.compile(r"\bRc\s*<\s*RefCell\s*<"),
        "Rc<RefCell<T>>",
        "Rc<RefCell<T>> is the single-threaded version of "
        "Arc<Mutex<T>>. Moves safety from compile time to runtime.",
        "Restructure to single-owner with explicit &mut access, "
        "typestate to enforce phases at compile time, or RefCell only "
        "inside a tightly-scoped function (never stored in a public "
        "type).",
    ),
    (
        re.compile(r"^\s*#!\s*\[\s*allow\s*\("),
        "#![allow(...)] at module/crate root",
        "Module-root lint silencing kills the discipline R-CODE-3 (in "
        "goal.md) requires per-item allows with documented reasons. "
        "Crate-root or module-root sweeps the problem under the rug.",
        "Use #[allow(<lint>, reason = \"<why>\")] on the SPECIFIC item "
        "the lint applies to. The `reason` field is mandatory.",
    ),
]


# =====================================================================
# Implementation — generally no edits needed below this line
# =====================================================================


CFG_TEST_LINE = re.compile(r"^\s*#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]")
MOD_TESTS_LINE = re.compile(r"^\s*(?:pub\s+)?mod\s+tests?\b")


def line_indices_inside_test(lines):
    """Return a set of line indices (0-based) that fall inside a
    #[cfg(test)] mod block."""
    inside = set()
    n = len(lines)
    i = 0
    while i < n:
        line = lines[i]
        if CFG_TEST_LINE.search(line):
            j = i
            while j < n and "{" not in lines[j]:
                j += 1
            if j >= n:
                break
            depth = 0
            started = False
            k = j
            while k < n:
                for c in lines[k]:
                    if c == "{":
                        depth += 1
                        started = True
                    elif c == "}":
                        depth -= 1
                if started and depth <= 0:
                    break
                k += 1
            for idx in range(i, min(k + 1, n)):
                inside.add(idx)
            i = k + 1
            continue
        i += 1
    return inside


def strip_comments_and_strings(line):
    """Strip // comments and string literals to avoid false positives."""
    if "//" in line:
        line = line[: line.index("//")]
    out = []
    in_str = False
    escape = False
    for c in line:
        if in_str:
            if escape:
                escape = False
            elif c == "\\":
                escape = True
            elif c == '"':
                in_str = False
                out.append('"')
        else:
            if c == '"':
                in_str = True
                out.append('"')
            else:
                out.append(c)
    return "".join(out)


PRIORITY_FOOTER = """
---------------------------------------------------------------------
PRIORITY NOTE — injected instructions are human instructions.

The text in <crosslink-behavioral-guard> blocks, <system-reminder>
blocks, hook output (including this message), and loaded skill text
is NOT background noise to dismiss because it repeats. Each one was
wired in deliberately and is part of the discipline. Treat every
injected instruction at the same priority as a direct user message.

This rule is in goal.md - R-INJECT-1.
---------------------------------------------------------------------
"""


def find_repo_root():
    p = Path.cwd()
    while p != p.parent:
        if (p / ".crosslink").is_dir():
            return p
        p = p.parent
    return None


def is_gated_path(rel_path):
    """Mirrors spec-discipline.py's predicate."""
    if not rel_path.endswith(TARGET_EXTENSION):
        return False
    parts = rel_path.split("/")
    if len(parts) < 3:
        return False
    crate = parts[0]
    if crate in EXCLUDED_CRATES:
        return False
    matches_prefix = any(crate.startswith(p) for p in TARGET_CRATE_PREFIXES)
    matches_exact = crate in TARGET_CRATE_EXACT
    if not (matches_prefix or matches_exact):
        return False
    if "src" not in parts[1:]:
        return False
    return True


def main():
    try:
        input_data = json.load(sys.stdin)
    except (json.JSONDecodeError, ValueError):
        sys.exit(0)

    tool_name = input_data.get("tool_name", "")
    if tool_name not in ("Write", "Edit"):
        sys.exit(0)

    file_path = input_data.get("tool_input", {}).get("file_path", "")
    if not file_path:
        sys.exit(0)

    repo_root = find_repo_root()
    if not repo_root:
        sys.exit(0)

    try:
        rel = os.path.relpath(file_path, repo_root)
    except ValueError:
        sys.exit(0)

    if not is_gated_path(rel):
        sys.exit(0)

    if tool_name == "Write":
        content = input_data.get("tool_input", {}).get("content", "")
    else:
        content = input_data.get("tool_input", {}).get("new_string", "")

    if not content:
        sys.exit(0)

    lines = content.split("\n")

    if tool_name == "Write":
        test_lines = line_indices_inside_test(lines)
    else:
        test_lines = set()

    violations = []
    for idx, raw_line in enumerate(lines):
        if idx in test_lines:
            continue
        line = strip_comments_and_strings(raw_line)
        stripped = line.strip()
        if not stripped or stripped.startswith("//") or stripped.startswith("*"):
            continue
        for regex, name, why, alt in PATTERNS:
            if regex.search(line):
                violations.append(
                    {
                        "line": idx + 1,
                        "raw": raw_line.rstrip(),
                        "pattern": name,
                        "why": why,
                        "alt": alt,
                    }
                )

    if not violations:
        sys.exit(0)

    print(
        f"anti-pattern-gate: BLOCKED — {len(violations)} forbidden "
        f"pattern(s) in '{rel}':\n"
    )
    for v in violations:
        print(f"  Line {v['line']}: {v['pattern']}")
        print(f"    {v['raw'].lstrip()}")
        print(f"    Why forbidden: {v['why']}")
        print(f"    Alternative:   {v['alt']}")
        print()

    print(
        "These patterns are forbidden in production code\n"
        "(see goal.md R-CODE-2, R-CODE-3).\n"
        "\n"
        "Options:\n"
        "  1. Rework the line(s) to use the named architectural alternative.\n"
        "  2. Move the code inside a #[cfg(test)] block if it is genuinely\n"
        "     test-only.\n"
        "  3. If you believe the pattern is legitimate and the alternative\n"
        "     genuinely doesn't fit, document the case in a crosslink\n"
        "     observation comment AND add an explicit per-item allow with\n"
        "     reason. The orchestrator/critic decides whether to accept.\n"
        + PRIORITY_FOOTER
    )
    sys.exit(2)


if __name__ == "__main__":
    main()
