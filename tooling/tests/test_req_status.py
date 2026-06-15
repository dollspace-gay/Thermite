#!/usr/bin/env python3
"""Oracle tests for tooling/req-status.py."""

import json
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path


GATE = Path(__file__).resolve().parents[1] / "req-status.py"


class Fixture:
    def __init__(self, root: Path):
        self.root = root

    def write(self, relpath: str, content: str):
        p = self.root / relpath
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(textwrap.dedent(content).lstrip(), encoding="utf-8")
        return p

    def run(self, *args):
        return subprocess.run(
            [sys.executable, str(GATE), "--root", str(self.root), *args],
            capture_output=True,
            text=True,
        )


class ReqStatusOracleTest(unittest.TestCase):
    def setUp(self):
        self.tmpdir = tempfile.TemporaryDirectory()
        self.root = Path(self.tmpdir.name)
        self.fx = Fixture(self.root)

    def tearDown(self):
        self.tmpdir.cleanup()

    def test_exact_req_label_conflict_fails(self):
        self.fx.write(
            "a/src/lib.rs",
            """
            //! | REQ-5 (forge plug-in point) | SHIPPED | `ship_symbol` is implemented. |
            pub fn ship_symbol() {}
            """,
        )
        self.fx.write(
            "b/src/lib.rs",
            """
            //! | REQ-5 (forge plug-in point) | NOT-STARTED | future stage #144 owns this. |
            """,
        )

        res = self.fx.run()

        self.assertEqual(res.returncode, 1, res.stdout + res.stderr)
        self.assertIn("STATUS-CONFLICT", res.stdout)
        self.assertIn("REQ-5 (forge plug-in point)", res.stdout)

    def test_not_started_requires_future_scope(self):
        self.fx.write(
            "a/src/lib.rs",
            """
            //! | REQ-9 (deferred thing) | NOT-STARTED | no implementation yet. |
            """,
        )

        res = self.fx.run()

        self.assertEqual(res.returncode, 1, res.stdout + res.stderr)
        self.assertIn("NOT-STARTED-SCOPE", res.stdout)

    def test_shipped_requires_resolving_evidence(self):
        self.fx.write(
            "a/src/lib.rs",
            """
            //! | REQ-1 (fake) | SHIPPED | `phantom_symbol` proves it. |
            pub fn real_symbol() {}
            """,
        )

        res = self.fx.run()

        self.assertEqual(res.returncode, 1, res.stdout + res.stderr)
        self.assertIn("UNRESOLVED-SHIPPED-EVIDENCE", res.stdout)

    def test_shipped_accepts_crate_relative_test_path(self):
        self.fx.write(
            "a/src/lib.rs",
            """
            //! | REQ-1 (tested) | SHIPPED | verified by `tests/thing.rs`. |
            pub fn real_symbol() {}
            """,
        )
        self.fx.write("a/tests/thing.rs", "use a::real_symbol;\n")

        res = self.fx.run()

        self.assertEqual(res.returncode, 0, res.stdout + res.stderr)
        self.assertIn("REQ status lint clean: 1 row(s) checked", res.stdout)

    def test_shipped_accepts_symbol_citation(self):
        self.fx.write(
            "a/src/lib.rs",
            """
            //! | REQ-1 (symbol) | SHIPPED | `real_symbol` is the consumer. |
            pub fn real_symbol() {}
            """,
        )

        res = self.fx.run()

        self.assertEqual(res.returncode, 0, res.stdout + res.stderr)

    def test_json_inventory_is_normalized(self):
        self.fx.write(
            "a/src/lib.rs",
            """
            //! | REQ-1 (symbol) | SHIPPED | `real_symbol` is the consumer. |
            pub fn real_symbol() {}
            """,
        )

        res = self.fx.run("--json")

        self.assertEqual(res.returncode, 0, res.stdout + res.stderr)
        payload = json.loads(res.stdout)
        self.assertEqual(payload["rows"][0]["path"], "a/src/lib.rs")
        self.assertEqual(payload["rows"][0]["status"], "SHIPPED")
        self.assertEqual(payload["issues"], [])


if __name__ == "__main__":
    unittest.main()
