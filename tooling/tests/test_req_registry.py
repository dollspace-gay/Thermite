#!/usr/bin/env python3
"""Oracle tests for tooling/req-registry.py."""

import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path


GATE = Path(__file__).resolve().parents[1] / "req-registry.py"


class Fixture:
    def __init__(self, root: Path):
        self.root = root

    def write(self, relpath: str, content: str):
        p = self.root / relpath
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(textwrap.dedent(content).lstrip(), encoding="utf-8")
        return p

    def registry(self, body: str):
        return self.write(".design/reqs/registry.toml", body)

    def valid_registry(self, extra: str = ""):
        self.write("src/lib.rs", "pub fn real_symbol() {}\n")
        self.write("tests/thing.rs", "# test fixture\n")
        self.registry(
            f"""
            schema_version = 1

            [[view]]
            name = "status"
            path = ".design/reqs/status.md"
            kind = "full_inventory"
            title = "Test Status"

            [[requirement]]
            id = "REQ-TEST-1"
            title = "Valid requirement"
            owner = "src/lib.rs"
            status = "shipped"
            scope = "tooling"
            generated_to = ["status"]

            [[requirement.evidence]]
            kind = "symbol"
            target = "real_symbol"

            {extra}
            """
        )

    def run(self, *args):
        return subprocess.run(
            [sys.executable, str(GATE), "--root", str(self.root), *args],
            capture_output=True,
            text=True,
        )


class ReqRegistryOracleTest(unittest.TestCase):
    def setUp(self):
        self.tmpdir = tempfile.TemporaryDirectory()
        self.root = Path(self.tmpdir.name)
        self.fx = Fixture(self.root)

    def tearDown(self):
        self.tmpdir.cleanup()

    def test_valid_registry_writes_and_checks_generated_view(self):
        self.fx.valid_registry()

        write_res = self.fx.run("--write")
        self.assertEqual(write_res.returncode, 0, write_res.stdout + write_res.stderr)
        self.assertTrue((self.root / ".design/reqs/status.md").is_file())

        check_res = self.fx.run("--check")
        self.assertEqual(check_res.returncode, 0, check_res.stdout + check_res.stderr)
        self.assertIn("REQ registry clean: 1 requirement(s), 1 view(s)", check_res.stdout)

    def test_rejects_duplicate_requirement_id(self):
        self.fx.valid_registry(
            """
            [[requirement]]
            id = "REQ-TEST-1"
            title = "Duplicate"
            owner = "src/lib.rs"
            status = "shipped"
            scope = "tooling"
            generated_to = ["status"]

            [[requirement.evidence]]
            kind = "file"
            target = "src/lib.rs"
            """
        )

        res = self.fx.run()

        self.assertEqual(res.returncode, 1, res.stdout + res.stderr)
        self.assertIn("DUPLICATE-REQ-ID", res.stdout)

    def test_rejects_unknown_status(self):
        self.fx.valid_registry()
        text = (self.root / ".design/reqs/registry.toml").read_text(encoding="utf-8")
        text = text.replace('status = "shipped"', 'status = "done"')
        (self.root / ".design/reqs/registry.toml").write_text(text, encoding="utf-8")

        res = self.fx.run()

        self.assertEqual(res.returncode, 1, res.stdout + res.stderr)
        self.assertIn("BAD-STATUS", res.stdout)

    def test_shipped_requires_proof_evidence_kind(self):
        self.fx.registry(
            """
            schema_version = 1

            [[view]]
            name = "status"
            path = ".design/reqs/status.md"
            kind = "full_inventory"

            [[requirement]]
            id = "REQ-TEST-1"
            title = "Weak shipped row"
            owner = "src/lib.rs"
            status = "shipped"
            scope = "tooling"
            generated_to = ["status"]

            [[requirement.evidence]]
            kind = "issue"
            target = "#17"
            """
        )

        res = self.fx.run()

        self.assertEqual(res.returncode, 1, res.stdout + res.stderr)
        self.assertIn("WEAK-SHIPPED-EVIDENCE", res.stdout)

    def test_rejects_unresolved_file_evidence(self):
        self.fx.valid_registry(
            """
            [[requirement]]
            id = "REQ-TEST-2"
            title = "Missing file"
            owner = "src/lib.rs"
            status = "partial"
            scope = "tooling"
            remaining_scope = "Finish it."
            generated_to = ["status"]

            [[requirement.evidence]]
            kind = "file"
            target = "missing.rs"
            """
        )

        res = self.fx.run()

        self.assertEqual(res.returncode, 1, res.stdout + res.stderr)
        self.assertIn("UNRESOLVED-EVIDENCE", res.stdout)

    def test_blocked_requires_issue_blocker(self):
        self.fx.valid_registry(
            """
            [[requirement]]
            id = "REQ-TEST-2"
            title = "Blocked row"
            owner = "src/lib.rs"
            status = "blocked"
            scope = "tooling"
            blockers = ["not-an-issue"]
            generated_to = ["status"]
            """
        )

        res = self.fx.run()

        self.assertEqual(res.returncode, 1, res.stdout + res.stderr)
        self.assertIn("BAD-BLOCKER", res.stdout)

    def test_check_detects_stale_generated_view(self):
        self.fx.valid_registry()
        self.fx.write(".design/reqs/status.md", "stale\n")

        res = self.fx.run("--check")

        self.assertEqual(res.returncode, 1, res.stdout + res.stderr)
        self.assertIn("STALE-GENERATED", res.stdout)


if __name__ == "__main__":
    unittest.main()
