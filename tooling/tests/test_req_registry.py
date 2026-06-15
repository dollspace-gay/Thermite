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

            [[status]]
            name = "shipped"
            final = true
            required_evidence_any = ["file", "symbol", "test"]

            [[status]]
            name = "partial"
            final = false
            required_evidence_any = ["file", "symbol", "test"]
            requires_remaining_scope = true

            [[status]]
            name = "blocked"
            final = false
            requires_blocker = true
            requires_remaining_scope = true

            [[view]]
            name = "status"
            path = ".design/reqs/status.md"
            kind = "full_inventory"
            mode = "region"
            region = "status"
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

            [[status]]
            name = "shipped"
            final = true
            required_evidence_any = ["file", "symbol", "test"]

            [[view]]
            name = "status"
            path = ".design/reqs/status.md"
            kind = "full_inventory"
            mode = "region"
            region = "status"

            [[requirement]]
            id = "REQ-TEST-1"
            title = "Weak shipped row"
            owner = "src/lib.rs"
            status = "shipped"
            scope = "tooling"
            generated_to = ["status"]

            [[requirement.evidence]]
            kind = "issue"
            target = "github:dollspace-gay/Thermite#17"
            """
        )

        res = self.fx.run()

        self.assertEqual(res.returncode, 1, res.stdout + res.stderr)
        self.assertIn("WEAK-STATUS-EVIDENCE", res.stdout)

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
            remaining_scope = "Waiting on tracker work."
            generated_to = ["status"]
            """
        )

        res = self.fx.run()

        self.assertEqual(res.returncode, 1, res.stdout + res.stderr)
        self.assertIn("BAD-BLOCKER", res.stdout)

    def test_issue_evidence_rejects_bare_tracker_number(self):
        self.fx.valid_registry(
            """
            [[requirement]]
            id = "REQ-TEST-2"
            title = "Bare issue ref"
            owner = "src/lib.rs"
            status = "partial"
            scope = "tooling"
            remaining_scope = "Waiting on tracker work."
            generated_to = ["status"]

            [[requirement.evidence]]
            kind = "file"
            target = "src/lib.rs"

            [[requirement.evidence]]
            kind = "issue"
            target = "#17"
            """
        )

        res = self.fx.run()

        self.assertEqual(res.returncode, 1, res.stdout + res.stderr)
        self.assertIn("BAD-EVIDENCE-TARGET", res.stdout)

    def test_req_blocker_resolves_to_registry_id(self):
        self.fx.valid_registry(
            """
            [[requirement]]
            id = "REQ-TEST-2"
            title = "Blocked by requirement"
            owner = "src/lib.rs"
            status = "blocked"
            scope = "tooling"
            blockers = ["req:REQ-TEST-1"]
            remaining_scope = "Waiting on the prerequisite requirement."
            generated_to = ["status"]
            """
        )

        res = self.fx.run()

        self.assertEqual(res.returncode, 0, res.stdout + res.stderr)

    def test_check_detects_stale_generated_view(self):
        self.fx.valid_registry()
        self.fx.write(
            ".design/reqs/status.md",
            """
            # Test Status

            <!-- generated:reqs view=status -->
            stale
            <!-- /generated:reqs -->
            """,
        )

        res = self.fx.run("--check")

        self.assertEqual(res.returncode, 1, res.stdout + res.stderr)
        self.assertIn("STALE-GENERATED", res.stdout)

    def test_reference_list_writes_rust_doc_comment_region(self):
        self.fx.write(
            "src/lib.rs",
            """
            pub fn real_symbol() {}
            //! <!-- generated:reqs view=source-status -->
            //! stale
            //! <!-- /generated:reqs -->
            """,
        )
        self.fx.registry(
            """
            schema_version = 1

            [[status]]
            name = "shipped"
            final = true
            required_evidence_any = ["file", "symbol", "test"]

            [[view]]
            name = "source-status"
            path = "src/lib.rs"
            kind = "reference_list"
            mode = "region"
            region = "source-status"
            comment_prefix = "//! "

            [[requirement]]
            id = "REQ-TEST-1"
            title = "Valid requirement"
            owner = "src/lib.rs"
            status = "shipped"
            scope = "tooling"
            generated_to = ["source-status"]

            [[requirement.evidence]]
            kind = "symbol"
            target = "real_symbol"
            """
        )

        write_res = self.fx.run("--write")
        self.assertEqual(write_res.returncode, 0, write_res.stdout + write_res.stderr)

        source = (self.root / "src/lib.rs").read_text(encoding="utf-8")
        self.assertIn("//! Source: `.design/reqs/registry.toml`", source)
        self.assertIn(
            "//! | REQ-TEST-1 | shipped | `src/lib.rs` | Valid requirement |  |",
            source,
        )

        check_res = self.fx.run("--check")
        self.assertEqual(check_res.returncode, 0, check_res.stdout + check_res.stderr)

    def test_legacy_mapping_rejects_unknown_target_id(self):
        self.fx.valid_registry(
            """
            [[view]]
            name = "source-status"
            path = "src/lib.rs"
            kind = "reference_list"
            mode = "region"
            region = "source-status"
            comment_prefix = "//! "

            [[legacy_mapping]]
            path = "src/lib.rs"
            label = "REQ-OLD"
            id = "REQ-MISSING"
            replacement_view = "source-status"
            """
        )
        (self.root / "src/lib.rs").write_text(
            "pub fn real_symbol() {}\n//! | REQ-OLD | SHIPPED | `real_symbol` |\n",
            encoding="utf-8",
        )

        res = self.fx.run()

        self.assertEqual(res.returncode, 1, res.stdout + res.stderr)
        self.assertIn("UNKNOWN-LEGACY-ID", res.stdout)

    def test_legacy_mapping_accepts_generated_replacement_region(self):
        self.fx.valid_registry(
            """
            [[view]]
            name = "source-status"
            path = "src/lib.rs"
            kind = "reference_list"
            mode = "region"
            region = "source-status"
            comment_prefix = "//! "

            [[legacy_mapping]]
            path = "src/lib.rs"
            label = "REQ-OLD"
            id = "REQ-TEST-1"
            replacement_view = "source-status"
            """
        )
        (self.root / "src/lib.rs").write_text(
            "pub fn real_symbol() {}\n"
            "//! <!-- generated:reqs view=source-status -->\n"
            "//! <!-- /generated:reqs -->\n",
            encoding="utf-8",
        )

        res = self.fx.run()

        self.assertEqual(res.returncode, 0, res.stdout + res.stderr)


if __name__ == "__main__":
    unittest.main()
