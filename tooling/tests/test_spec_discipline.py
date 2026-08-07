#!/usr/bin/env python3
"""
Oracle fixture tests for tooling/spec-discipline.py.

Same convention as test_control_plane.py: each test builds a throwaway repo in
a tmpdir (a .crosslink/ marker, a goal.md, a spec-routes.toml, the design docs
and reference files those routes name), drives the gate by subprocess with real
Claude Code hook payloads, and asserts against HAND-AUTHORED oracle facts drawn
from goal.md — never the tool's own output (R-CHAR-3).

The authority is goal.md:

  R-XLATE-1  every Edit/Write to a routed thermite-*/src or forge/src .rs file
             requires a Read this session of goal.md + the route's design doc +
             (if the route declares one) at least one route reference.
  R-XLATE-2  a routed file with no route table entry BLOCKS.
  R-XLATE-3  a route whose design doc does not exist BLOCKS until
             acto-doc-author authors it.

S-2 and S-3 are the load-bearing pair. The route table declares 48 references
under `stdlib/`, and `REFERENCE_PREFIXES` decides which Reads get recorded at
all. A prefix missing from that tuple silently drops the Read, so the route's
reference requirement can never be satisfied and the gate blocks the file
forever. S-2 pins that a declared stdlib reference satisfies its own route.
S-3 uses the SAME route and reads a DIFFERENT stdlib file, so a gate that
satisfied S-2 by accepting any stdlib read fails S-3. Neither can pass by
accident.

S-8 pins the fail-closed direction: two of the three required classes read is
still a block, and the report names only the missing class.

Runnable as:  python3 -m unittest discover -s tooling/tests

The oracle (goal.md's expected values, not the tool's):

  S-1  (R-XLATE-1 teeth):    edit a routed file with no reads -> exit 2; the
                             report names goal.md, the design doc, and the
                             reference.
  S-2  (R-XLATE-1 stdlib):   goal.md + design + the route's declared stdlib
                             reference -> exit 0.
  S-3  (S-2's discriminator): same route, a stdlib file the route does not
                             declare -> exit 2; the report names the
                             reference class.
  S-4  (R-XLATE-1 corpus):   the conformance reference path still satisfies
                             its own route -> exit 0.
  S-5  (R-XLATE-2):          a gated file absent from the route table
                             -> exit 2.
  S-6  (R-XLATE-3):          a route whose design doc is absent -> exit 2 and
                             the report names acto-doc-author.
  S-7  (is_gated_path):      a path with no src/ component is not gated
                             -> exit 0 with no reads recorded.
  S-8  (partial reads):      goal.md + design but no reference -> exit 2, and
                             the report names the reference class alone.
"""

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
GATE = REPO_ROOT / "tooling" / "spec-discipline.py"

ROUTES = """\
[[route]]
crate_pattern = "forge/src/alpha.rs"
design = ".design/build/alpha.md"
reference = ["stdlib/pkg/api.th"]
conformance_ops = []

[[route]]
crate_pattern = "forge/src/beta.rs"
design = ".design/build/beta.md"
reference = ["conformance/beta.th"]
conformance_ops = []

[[route]]
crate_pattern = "forge/src/orphan.rs"
design = ".design/build/absent.md"
reference = []
conformance_ops = []
"""

FILES = {
    "goal.md": "# fixture goal\n",
    "tooling/spec-routes.toml": ROUTES,
    ".design/build/alpha.md": "# alpha\n",
    ".design/build/beta.md": "# beta\n",
    "stdlib/pkg/api.th": "fn declared() -> u64 req true ens result == 1 fx pure { 1 }\n",
    "stdlib/pkg/other.th": "fn undeclared() -> u64 req true ens result == 2 fx pure { 2 }\n",
    "conformance/beta.th": "fn corpus() -> u64 req true ens result == 3 fx pure { 3 }\n",
    "forge/src/alpha.rs": "pub fn alpha() {}\n",
    "forge/src/beta.rs": "pub fn beta() {}\n",
    "forge/src/orphan.rs": "pub fn orphan() {}\n",
    "forge/src/unrouted.rs": "pub fn unrouted() {}\n",
    "forge/tests/helper.rs": "pub fn helper() {}\n",
}


class SpecDisciplineOracle(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)
        (self.root / ".crosslink").mkdir()
        for rel, body in FILES.items():
            target = self.root / rel
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(body, encoding="utf-8")
        self.addCleanup(self._tmp.cleanup)

    def run_gate(self, payload):
        completed = subprocess.run(
            [sys.executable, str(GATE)],
            input=json.dumps(payload),
            capture_output=True,
            text=True,
            cwd=str(self.root),
        )
        return completed.returncode, completed.stdout

    def record_read(self, rel):
        code, _ = self.run_gate(
            {
                "hook_event_name": "PostToolUse",
                "tool_name": "Read",
                "tool_input": {"file_path": str(self.root / rel)},
            }
        )
        self.assertEqual(code, 0, "recording a Read never blocks")

    def attempt_edit(self, rel):
        return self.run_gate(
            {
                "hook_event_name": "PreToolUse",
                "tool_name": "Edit",
                "tool_input": {
                    "file_path": str(self.root / rel),
                    "old_string": "pub fn",
                    "new_string": "pub fn",
                },
            }
        )

    # S-1
    def test_edit_without_any_read_blocks(self):
        code, out = self.attempt_edit("forge/src/alpha.rs")
        self.assertEqual(code, 2, f"R-XLATE-1: an unread route blocks\n{out}")
        self.assertIn("goal.md", out)
        self.assertIn(".design/build/alpha.md", out)
        self.assertIn("stdlib/pkg/api.th", out)

    # S-2 — the stdlib reference prefix.
    def test_declared_stdlib_reference_satisfies_its_route(self):
        self.record_read("goal.md")
        self.record_read(".design/build/alpha.md")
        self.record_read("stdlib/pkg/api.th")
        code, out = self.attempt_edit("forge/src/alpha.rs")
        self.assertEqual(
            code,
            0,
            "R-XLATE-1: a route declaring a stdlib reference must be "
            f"satisfiable by reading that file\n{out}",
        )

    # S-3 — the discriminator for S-2. Same route, undeclared stdlib file.
    def test_undeclared_stdlib_read_does_not_satisfy_the_route(self):
        self.record_read("goal.md")
        self.record_read(".design/build/alpha.md")
        self.record_read("stdlib/pkg/other.th")
        code, out = self.attempt_edit("forge/src/alpha.rs")
        self.assertEqual(
            code,
            2,
            "a reference read satisfies only the route that declares it; "
            f"any stdlib read must not stand in for the declared one\n{out}",
        )
        self.assertIn("[reference]", out)

    # S-4 — regression guard on the original prefix.
    def test_conformance_reference_still_satisfies_its_route(self):
        self.record_read("goal.md")
        self.record_read(".design/build/beta.md")
        self.record_read("conformance/beta.th")
        code, out = self.attempt_edit("forge/src/beta.rs")
        self.assertEqual(code, 0, f"the corpus prefix must keep working\n{out}")

    # S-5
    def test_gated_file_with_no_route_blocks(self):
        self.record_read("goal.md")
        code, out = self.attempt_edit("forge/src/unrouted.rs")
        self.assertEqual(code, 2, f"R-XLATE-2: no route entry blocks\n{out}")

    # S-6
    def test_route_with_absent_design_doc_blocks(self):
        self.record_read("goal.md")
        code, out = self.attempt_edit("forge/src/orphan.rs")
        self.assertEqual(code, 2, f"R-XLATE-3: an absent design blocks\n{out}")
        self.assertIn("acto-doc-author", out)

    # S-7
    def test_ungated_path_is_not_gated(self):
        code, out = self.attempt_edit("forge/tests/helper.rs")
        self.assertEqual(
            code, 0, f"is_gated_path requires a src/ component\n{out}"
        )

    # S-8 — fail closed on a partial read set.
    def test_missing_reference_alone_still_blocks(self):
        self.record_read("goal.md")
        self.record_read(".design/build/alpha.md")
        code, out = self.attempt_edit("forge/src/alpha.rs")
        self.assertEqual(
            code, 2, f"R-XLATE-1 requires every declared class\n{out}"
        )
        self.assertIn("[reference]", out)
        self.assertNotIn("[design]", out)

    def test_every_route_reference_prefix_is_recordable(self):
        # Guards the real route table, not the fixture: a prefix any route
        # declares but is_tracked_read drops makes that route permanently
        # unsatisfiable. This is the defect S-2 was written for.
        import tomllib

        with open(REPO_ROOT / "tooling" / "spec-routes.toml", "rb") as f:
            routes = tomllib.load(f)["route"]
        declared = {
            ref.split("/")[0] + "/"
            for route in routes
            for ref in route.get("reference", [])
        }
        source = (REPO_ROOT / "tooling" / "spec-discipline.py").read_text()
        for line in source.splitlines():
            if line.startswith("REFERENCE_PREFIXES"):
                recorded = {
                    part.strip().strip('"').strip("'")
                    for part in line.split("(", 1)[1].rstrip(")").split(",")
                    if part.strip()
                }
                break
        else:
            self.fail("REFERENCE_PREFIXES not found in spec-discipline.py")
        for prefix in sorted(declared):
            self.assertTrue(
                any(prefix.startswith(r) or r.startswith(prefix) for r in recorded),
                f"routes declare references under '{prefix}' but "
                f"REFERENCE_PREFIXES records {sorted(recorded)}; those routes "
                "can never satisfy their own gate",
            )


if __name__ == "__main__":
    unittest.main()
