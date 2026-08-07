#!/usr/bin/env python3
"""
Oracle fixture tests for tooling/anti-pattern-gate.py.

Same convention as test_control_plane.py: each test builds a throwaway repo in
a tmpdir (a .crosslink/ marker plus one gated source file), pipes a Claude Code
hook payload to the gate on stdin, and asserts against HAND-AUTHORED oracle
facts drawn from goal.md — never the tool's own output (R-CHAR-3).

The authority is goal.md:

  R-APG-1  blocks todo!()/unimplemented!()/unreachable!(), .unwrap()/.expect()/
           panic!() outside #[cfg(test)], module-root #![allow], and the
           Arc<Mutex<T>>/Rc<RefCell<T>> escape hatches.
  R-APG-2  "#[cfg(test)] blocks exempt; production is not."
  R-CODE-2 names the same production rule the gate enforces.

O-5 and O-6 are the load-bearing pair. O-5 pins the R-APG-2 exemption for an
Edit whose anchor sits inside an existing #[cfg(test)] block, which is the
behaviour a REQ-KPRIM-3 builder was blocked by. O-6 uses the SAME fixture file
and the SAME replacement text, moving only the anchor into production code, so
a gate that satisfied O-5 by allowing every Edit fails O-6. Neither test can
pass by accident.

O-8 pins the fail-closed direction: an anchor the gate cannot locate keeps the
narrow exemption. A gate that treated an unreadable or unmatched anchor as
"probably a test" would pass O-5 and fail O-8.

Runnable as:  python3 -m unittest discover -s tooling/tests

The oracle (goal.md's expected values, not the tool's):

  O-1  (R-APG-1 teeth):      Write with .unwrap() in production -> exit 2,
                             report names the file and the offending line.
  O-2  (R-APG-2 Write):      Write with .unwrap() inside #[cfg(test)] mod
                             tests -> exit 0, no output.
  O-3  (R-APG-1 Edit teeth): Edit replacing production code with .unwrap()
                             -> exit 2.
  O-4  (R-APG-2 Edit, self-contained): Edit whose replacement carries its own
                             #[cfg(test)] mod block -> exit 0.
  O-5  (R-APG-2 Edit, on-disk context): Edit whose anchor sits inside an
                             existing #[cfg(test)] block -> exit 0.
  O-6  (O-5's discriminator): same file, same replacement, anchor in
                             production -> exit 2.
  O-7  (is_gated_path):      a path with no src/ component is not gated
                             -> exit 0 even with .unwrap() in production.
  O-8  (fail closed):        Edit whose anchor appears nowhere in the file
                             -> exit 2; an unlocatable anchor never widens
                             the exemption.
  O-9  (R-APG-1 breadth):    todo!() in production -> exit 2, so the fix to
                             the test exemption did not disarm other rules.
"""

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
GATE = REPO_ROOT / "tooling" / "anti-pattern-gate.py"

# A gated path per is_gated_path: a forge/ or thermite-* crate dir plus a src/
# component plus a .rs extension.
GATED = "forge/src/sample.rs"
UNGATED = "forge/tests/sample.rs"

# The hand-authored fixture. Production code first, then a test module whose
# body uses .unwrap() legitimately under R-APG-2.
FIXTURE = """\
pub fn parse(raw: &str) -> Result<u64, String> {
    let trimmed = raw.trim();
    trimmed.parse::<u64>().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_number() {
        let value = parse("7").unwrap();
        assert_eq!(value, 7);
    }
}
"""

# The anchor inside the test module, and the anchor in production. Both are
# verbatim slices of FIXTURE, so a fixture edit that invalidates them fails
# loudly rather than silently weakening a test.
TEST_ANCHOR = '        let value = parse("7").unwrap();'
PRODUCTION_ANCHOR = "    let trimmed = raw.trim();"

# One replacement text, reused by O-5 and O-6 so the anchor is the only
# variable between them.
REPLACEMENT_WITH_UNWRAP = '        let value = parse("11").unwrap();'


def run_gate(payload, root):
    """Pipe a hook payload to the gate from `root` and return (code, stdout)."""
    completed = subprocess.run(
        [sys.executable, str(GATE)],
        input=json.dumps(payload),
        capture_output=True,
        text=True,
        cwd=str(root),
    )
    return completed.returncode, completed.stdout, completed.stderr


class AntiPatternGateOracle(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)
        (self.root / ".crosslink").mkdir()
        for rel in (GATED, UNGATED):
            target = self.root / rel
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(FIXTURE, encoding="utf-8")
        self.addCleanup(self._tmp.cleanup)

    def write_payload(self, rel, content):
        return {
            "tool_name": "Write",
            "tool_input": {
                "file_path": str(self.root / rel),
                "content": content,
            },
        }

    def edit_payload(self, rel, old_string, new_string):
        return {
            "tool_name": "Edit",
            "tool_input": {
                "file_path": str(self.root / rel),
                "old_string": old_string,
                "new_string": new_string,
            },
        }

    # O-1
    def test_write_blocks_production_unwrap(self):
        body = "pub fn go(v: Option<u64>) -> u64 {\n    v.unwrap()\n}\n"
        code, out, _ = run_gate(self.write_payload(GATED, body), self.root)
        self.assertEqual(code, 2, f"R-APG-1: production unwrap must block\n{out}")
        self.assertIn(GATED, out)
        self.assertIn("v.unwrap()", out)

    # O-2
    def test_write_exempts_cfg_test_block(self):
        code, out, _ = run_gate(self.write_payload(GATED, FIXTURE), self.root)
        self.assertEqual(code, 0, f"R-APG-2: #[cfg(test)] is exempt\n{out}")
        self.assertEqual(out, "")

    # O-3
    def test_edit_blocks_production_unwrap(self):
        payload = self.edit_payload(
            GATED, PRODUCTION_ANCHOR, "    let trimmed = raw.trim().unwrap();"
        )
        code, out, _ = run_gate(payload, self.root)
        self.assertEqual(code, 2, f"R-APG-1 binds on Edit as well as Write\n{out}")

    # O-4
    def test_edit_exempts_a_self_contained_test_module(self):
        replacement = (
            "#[cfg(test)]\n"
            "mod extra {\n"
            "    #[test]\n"
            "    fn checks() {\n"
            '        let v: Option<u64> = Some(3);\n'
            "        assert_eq!(v.unwrap(), 3);\n"
            "    }\n"
            "}\n"
        )
        payload = self.edit_payload(GATED, PRODUCTION_ANCHOR, replacement)
        code, out, _ = run_gate(payload, self.root)
        self.assertEqual(
            code,
            0,
            "R-APG-2: a replacement carrying its own #[cfg(test)] block is "
            f"exempt inside that block\n{out}",
        )

    # O-5 — the behaviour a builder was blocked by.
    def test_edit_exempts_an_anchor_inside_an_existing_test_block(self):
        payload = self.edit_payload(GATED, TEST_ANCHOR, REPLACEMENT_WITH_UNWRAP)
        code, out, _ = run_gate(payload, self.root)
        self.assertEqual(
            code,
            0,
            "R-APG-2: an Edit landing inside an existing #[cfg(test)] block "
            f"is test code and is exempt\n{out}",
        )

    # O-6 — the discriminator for O-5. Same file, same replacement text.
    def test_edit_blocks_the_same_replacement_anchored_in_production(self):
        payload = self.edit_payload(
            GATED, PRODUCTION_ANCHOR, REPLACEMENT_WITH_UNWRAP
        )
        code, out, _ = run_gate(payload, self.root)
        self.assertEqual(
            code,
            2,
            "R-APG-2 exempts test blocks only; the identical replacement "
            f"anchored in production must still block\n{out}",
        )

    # O-7
    def test_ungated_path_is_not_scanned(self):
        body = "pub fn go(v: Option<u64>) -> u64 {\n    v.unwrap()\n}\n"
        code, out, _ = run_gate(self.write_payload(UNGATED, body), self.root)
        self.assertEqual(
            code, 0, f"is_gated_path requires a src/ component\n{out}"
        )

    # O-8 — fail closed.
    def test_edit_with_an_unlocatable_anchor_stays_blocked(self):
        payload = self.edit_payload(
            GATED,
            "    let absent = never_present_in_the_fixture();",
            REPLACEMENT_WITH_UNWRAP,
        )
        code, out, _ = run_gate(payload, self.root)
        self.assertEqual(
            code,
            2,
            "an anchor the gate cannot locate must keep the narrow exemption; "
            f"an unknown anchor never widens what the gate allows\n{out}",
        )

    # O-9
    def test_other_rules_survive_the_test_exemption(self):
        body = "pub fn go() -> u64 {\n    todo!()\n}\n"
        code, out, _ = run_gate(self.write_payload(GATED, body), self.root)
        self.assertEqual(code, 2, f"R-APG-1 covers todo!() too\n{out}")

    def test_fixture_anchors_are_real_slices_of_the_fixture(self):
        # Guards the suite itself: an anchor that drifts out of FIXTURE would
        # make O-5 and O-6 test nothing.
        self.assertIn(TEST_ANCHOR, FIXTURE)
        self.assertIn(PRODUCTION_ANCHOR, FIXTURE)


if __name__ == "__main__":
    unittest.main()
