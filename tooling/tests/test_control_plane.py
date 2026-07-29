#!/usr/bin/env python3
"""
Oracle fixture tests for tooling/control-plane-check.py.

Same convention as test_doc_drift.py: each test builds a throwaway control plane
in a tmpdir (a .claude/settings.json + the tooling/ scripts it names), runs the
gate via subprocess with `--root <fixture>`, and asserts against HAND-AUTHORED
oracle facts — never the tool's own output (R-CHAR-3).

O-2 is the load-bearing one: its fixture is the VERBATIM de-wired settings.json
that commit 5581b65f left on main, so the suite fails if the gate ever stops
catching the exact regression it was built for (crosslink #93).

Runnable as:  python3 -m unittest discover -s tooling/tests

The oracle (the spec's expected values, not the tool's):

  O-1  (REQ-2 wired):     all three required hooks present + scripts on disk
                          -> exit 0; one WIRED line each; no MISSING token.
  O-2  (REQ-2 the #93 regression): the verbatim post-5581b65f settings.json
                          (crosslink-generic hooks only) -> exit 1; three
                          MISSING-WIRING lines naming spec-discipline.py twice
                          and anti-pattern-gate.py once.
  O-3  (REQ-3 dead hook): wiring present but the named script absent from disk
                          -> exit 1; MISSING-SCRIPT (NOT MISSING-WIRING), since
                          the `if [ -f "$HOOK" ]` guard makes it a silent no-op.
  O-4  (REQ-1 malformed): settings.json that is not valid JSON -> exit 1;
                          UNPARSEABLE; no Traceback (Claude Code loads no hooks
                          from it, so it is a dead gate, not an env failure).
  O-5  (REQ-1 absent):    no settings.json at all -> exit 1; UNPARSEABLE.
  O-6  (REQ-2 superset):  matcher "Write|Edit|Bash" satisfies a Write|Edit
                          requirement -> exit 0 (coverage, not equality).
  O-7  (REQ-2 narrowed):  matcher "Write" alone does NOT satisfy Write|Edit
                          -> exit 1; MISSING-WIRING.
  O-8  (REQ-4 determinism): two runs on the unchanged fixture -> byte-identical
                          stdout.
  O-9  (REQ-5 inconclusive): no --root, cwd a non-git tmpdir -> exit 3, no
                          Traceback on stderr, and NOT exit 0 (never fail open).
"""

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

# The gate under test: tooling/control-plane-check.py, two levels up.
GATE = Path(__file__).resolve().parents[1] / "control-plane-check.py"

# The two hook scripts the required wirings name.
SPEC_DISCIPLINE = "tooling/spec-discipline.py"
ANTI_PATTERN = "tooling/anti-pattern-gate.py"


def _guard(script):
    """The `if [ -f "$HOOK" ]`-guarded command form used throughout settings."""
    return (
        f'HOOK="$(git rev-parse --show-toplevel 2>/dev/null)/{script}"; '
        f'if [ -f "$HOOK" ]; then python3 "$HOOK"; else exit 0; fi'
    )


def _entry(matcher, *scripts):
    """A settings.json hook entry invoking `scripts` under `matcher`."""
    hooks = [
        {"command": _guard(s), "timeout": 5, "type": "command"} for s in scripts
    ]
    entry = {"hooks": hooks}
    if matcher is not None:
        entry["matcher"] = matcher
    return entry


# The crosslink-generic hooks that SURVIVED the 5581b65f clobber. Present in
# every fixture so a test can never pass merely because the file is empty.
def _crosslink_generic():
    return {
        "PostToolUse": [
            _entry("Write|Edit", ".claude/hooks/post-edit-check.py"),
            _entry(None, ".claude/hooks/heartbeat.py"),
        ],
        "PreToolUse": [
            _entry("WebFetch|WebSearch", ".claude/hooks/pre-web-check.py"),
            _entry("Write|Edit|Bash", ".claude/hooks/work-check.py"),
        ],
    }


def _fully_wired():
    """The pre-clobber (and now restored) hook set."""
    hooks = _crosslink_generic()
    hooks["PostToolUse"].insert(1, _entry("Read", SPEC_DISCIPLINE))
    hooks["PreToolUse"].append(_entry("Write|Edit", SPEC_DISCIPLINE, ANTI_PATTERN))
    return hooks


def _make_fixture(tmp, hooks, *, scripts_present=True, raw_settings=None,
                  write_settings=True):
    """Build a control-plane fixture rooted at `tmp`; return that root."""
    root = Path(tmp)
    (root / ".claude").mkdir(parents=True, exist_ok=True)
    (root / "tooling").mkdir(parents=True, exist_ok=True)

    if scripts_present:
        for script in (SPEC_DISCIPLINE, ANTI_PATTERN):
            (root / script).write_text("#!/usr/bin/env python3\n", encoding="utf-8")

    if write_settings:
        settings_path = root / ".claude" / "settings.json"
        if raw_settings is not None:
            settings_path.write_text(raw_settings, encoding="utf-8")
        else:
            settings_path.write_text(
                json.dumps({"hooks": hooks}, indent=2), encoding="utf-8"
            )
    return root


def _run(root=None, cwd=None):
    """Invoke the gate; return (returncode, stdout, stderr)."""
    argv = [sys.executable, str(GATE)]
    if root is not None:
        argv += ["--root", str(root)]
    proc = subprocess.run(argv, capture_output=True, text=True, cwd=cwd)
    return proc.returncode, proc.stdout, proc.stderr


class TestControlPlaneGate(unittest.TestCase):

    def test_o1_fully_wired_passes(self):
        """O-1: all three required hooks wired + present -> exit 0."""
        with tempfile.TemporaryDirectory() as tmp:
            root = _make_fixture(tmp, _fully_wired())
            code, out, _ = _run(root)
            self.assertEqual(code, 0, out)
            self.assertEqual(out.count("WIRED "), 3, out)
            self.assertNotIn("MISSING", out)

    def test_o2_the_5581b65f_regression_is_caught(self):
        """O-2: the verbatim post-clobber settings.json -> exit 1, 3 findings.

        This is the crosslink #93 regression. If this test ever passes with
        exit 0, the gate has stopped guarding the thing it exists to guard.
        """
        with tempfile.TemporaryDirectory() as tmp:
            root = _make_fixture(tmp, _crosslink_generic())
            code, out, err = _run(root)
            self.assertEqual(code, 1, out + err)
            self.assertEqual(out.count("MISSING-WIRING "), 3, out)
            # spec-discipline is required twice (PostToolUse/Read +
            # PreToolUse/Write|Edit), anti-pattern-gate once.
            self.assertEqual(out.count(f"-> {SPEC_DISCIPLINE}"), 2, out)
            self.assertEqual(out.count(f"-> {ANTI_PATTERN}"), 1, out)
            self.assertNotIn("Traceback", err)

    def test_o3_wired_but_script_absent_is_missing_script(self):
        """O-3: the guard degrades an absent script to a no-op -> MISSING-SCRIPT."""
        with tempfile.TemporaryDirectory() as tmp:
            root = _make_fixture(tmp, _fully_wired(), scripts_present=False)
            code, out, _ = _run(root)
            self.assertEqual(code, 1, out)
            self.assertIn("MISSING-SCRIPT", out)
            self.assertNotIn("MISSING-WIRING", out)

    def test_o4_malformed_json_is_a_finding_not_a_traceback(self):
        """O-4: unparseable settings -> exit 1 UNPARSEABLE, no traceback."""
        with tempfile.TemporaryDirectory() as tmp:
            root = _make_fixture(tmp, None, raw_settings='{"hooks": {,,,}')
            code, out, err = _run(root)
            self.assertEqual(code, 1, out + err)
            self.assertIn("UNPARSEABLE", out)
            self.assertNotIn("Traceback", err)

    def test_o5_absent_settings_is_a_finding(self):
        """O-5: no settings.json -> exit 1 UNPARSEABLE."""
        with tempfile.TemporaryDirectory() as tmp:
            root = _make_fixture(tmp, None, write_settings=False)
            code, out, _ = _run(root)
            self.assertEqual(code, 1, out)
            self.assertIn("UNPARSEABLE", out)

    def test_o6_superset_matcher_covers_requirement(self):
        """O-6: `Write|Edit|Bash` satisfies a Write|Edit requirement."""
        with tempfile.TemporaryDirectory() as tmp:
            hooks = _crosslink_generic()
            hooks["PostToolUse"].insert(1, _entry("Read", SPEC_DISCIPLINE))
            hooks["PreToolUse"].append(
                _entry("Write|Edit|Bash", SPEC_DISCIPLINE, ANTI_PATTERN)
            )
            root = _make_fixture(tmp, hooks)
            code, out, _ = _run(root)
            self.assertEqual(code, 0, out)
            self.assertNotIn("MISSING", out)

    def test_o7_narrowed_matcher_does_not_cover_requirement(self):
        """O-7: a matcher that drops `Edit` is a MISSING-WIRING, not a pass."""
        with tempfile.TemporaryDirectory() as tmp:
            hooks = _crosslink_generic()
            hooks["PostToolUse"].insert(1, _entry("Read", SPEC_DISCIPLINE))
            hooks["PreToolUse"].append(
                _entry("Write", SPEC_DISCIPLINE, ANTI_PATTERN)
            )
            root = _make_fixture(tmp, hooks)
            code, out, _ = _run(root)
            self.assertEqual(code, 1, out)
            self.assertEqual(out.count("MISSING-WIRING "), 2, out)

    def test_o8_output_is_deterministic(self):
        """O-8: two runs on an unchanged fixture -> byte-identical stdout."""
        with tempfile.TemporaryDirectory() as tmp:
            root = _make_fixture(tmp, _crosslink_generic())
            _, first, _ = _run(root)
            _, second, _ = _run(root)
            self.assertEqual(first, second)

    def test_o9_non_git_cwd_is_inconclusive_never_a_pass(self):
        """O-9: no --root outside a git repo -> exit 3, never a silent exit 0."""
        with tempfile.TemporaryDirectory() as tmp:
            code, out, err = _run(root=None, cwd=tmp)
            self.assertEqual(code, 3, out + err)
            self.assertNotEqual(code, 0)
            self.assertNotIn("Traceback", err)


if __name__ == "__main__":
    unittest.main()
