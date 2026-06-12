#!/usr/bin/env python3
"""
Critic divergence tests for tooling/doc-drift.py (acto-critic, crosslink #258).

Each test pins a DIVERGENCE between the gate's behavior and its authority,
.design/tooling/doc-drift-tripwire.md (REQ-9) + goal.md R-HONEST-3. Expected
values are taken from the design doc's REQ-9 exit-code contract, never from
the tool's own output (R-CHAR-3). These tests FAIL against the current
implementation by construction; they are the audit artifact for the builder's
commit bde2089f.

Divergence inventory:

  C-1  Empty-but-valid route table -> the gate exits 0 having checked ZERO
       docs. Authority: REQ-9 "0 = every routed doc pinned and current ...
       The tool never exits 0 without having checked all 48 docs" +
       R-HONEST-3 (a gate that fails open is a silent pass). A truncated /
       emptied spec-routes.toml silently turns the gate green. Expected: 3
       (INCONCLUSIVE — the enumeration source yielded nothing to check).
  C-2  spec-routes.toml that PARSES as TOML but has the wrong shape
       (`route = 5`, or `route = ["a"]`) -> unhandled Python traceback with
       exit code 1. Authority: REQ-9 "3 = the gate could not determine the
       answer (... spec-routes.toml unreadable)" and AC-5's never-a-traceback
       discipline. Exit 1 is the DRIFT-FOUND class, so an environment defect
       is misreported as a drift finding; the traceback violates the
       "never a traceback" contract the tool's own docstring restates.
  C-3  A [[route]] entry whose `crate_pattern` key is MISSING (a required
       field per the spec-routes.toml schema header), alongside valid
       routes -> the entry is silently dropped, its `design`-field doc
       leaves the checked set, and the gate exits 0. Authority: REQ-5
       ("every doc referenced by a [[route]].design field") + REQ-1 ("the
       set of checked docs is exactly the deduplicated design fields") +
       REQ-8 (no grandfathering) + REQ-9 ("never exits 0 without having
       checked all 48 docs"). Skip-gated; tracking crosslink #261.

Run with:  python3 -m unittest discover -s tooling/tests -v
(both C-1 (#259) and C-2 (#260) are now FIXED and UNGATED — permanent
 regression coverage, no env gate.)
"""

import os
import subprocess
import sys
import unittest
from pathlib import Path

# Reuse the builder's hermetic fixture (same directory under discover).
from test_doc_drift import Fixture

GATE = Path(__file__).resolve().parents[1] / "doc-drift.py"

# REQ-9 contract constants, transcribed from the design doc (the authority),
# NOT imported from the tool under test (R-CHAR-3).
REQ9_EXIT_INCONCLUSIVE = 3


class DocDriftCriticTest(unittest.TestCase):
    def setUp(self):
        import tempfile

        self._tmp = tempfile.TemporaryDirectory()
        self.tmp = Path(self._tmp.name)

    def tearDown(self):
        self._tmp.cleanup()

    # --- C-1: zero-route table is a vacuous pass (fail-open) ----------------
    # UNGATED (crosslink #259 fixed): an empty route table now exits 3
    # (INCONCLUSIVE) per REQ-9 / R-HONEST-3. This is permanent regression
    # coverage. (test_c2 below stays skip-gated for #260.)
    def test_c1_empty_route_table_must_not_exit_0(self):
        """REQ-9: 'The tool never exits 0 without having checked all 48 docs.'

        A spec-routes.toml that is valid TOML but contains zero [[route]]
        entries gives the gate NOTHING to check; exiting 0 asserts 'every
        routed doc pinned and current' vacuously — exactly the fail-open
        silent pass R-HONEST-3 forbids. Expected: exit 3 (INCONCLUSIVE: the
        enumeration source is empty/unusable). Actual today: exit 0, empty
        report.
        """
        fx = Fixture(self.tmp / "c1")
        fx.write("tooling/spec-routes.toml", "# valid TOML, zero routes\n")
        fx.commit("src/a.rs", "v1\n", "A: a v1")

        res = fx.run_gate()
        self.assertEqual(
            res.returncode,
            REQ9_EXIT_INCONCLUSIVE,
            "zero routed docs checked must be INCONCLUSIVE (3), never a "
            f"green 0 — got {res.returncode}; stdout={res.stdout!r} "
            f"stderr={res.stderr!r}",
        )

    # --- C-2: TOML-valid but wrong-shaped route table -> traceback, exit 1 --
    # UNGATED (crosslink #260 fixed): a TOML-valid but wrong-shaped route table
    # (`route = 5`, `route = ["a"]`) now exits 3 (INCONCLUSIVE) with no
    # traceback per REQ-9 / R-HONEST-3. This is permanent regression coverage.
    def test_c2_wrong_shape_route_table_is_exit_3_not_traceback(self):
        """REQ-9: '3 = ... spec-routes.toml unreadable', never a traceback.

        `route = 5` parses as TOML, then `for route in data.get("route", [])`
        raises TypeError -> unhandled traceback, Python exits 1. Exit 1 is
        REQ-9's DRIFT/MISSING-PIN/INVALID-PIN class, so a broken route table
        is misreported as a drift FINDING, and the traceback breaks the
        never-a-traceback contract (AC-5 discipline; the tool's docstring:
        'never traceback, never fail-open'). Expected: exit 3, no Traceback.
        """
        for bad_table in ("route = 5\n", 'route = ["a"]\n'):
            with self.subTest(table=bad_table):
                fx = Fixture(self.tmp / f"c2-{abs(hash(bad_table))}")
                fx.write("tooling/spec-routes.toml", bad_table)
                fx.commit("src/a.rs", "v1\n", "A: a v1")

                res = fx.run_gate()
                self.assertEqual(
                    res.returncode,
                    REQ9_EXIT_INCONCLUSIVE,
                    "wrong-shaped route table is 'spec-routes.toml "
                    f"unreadable' (exit 3) — got {res.returncode}; "
                    f"stderr={res.stderr!r}",
                )
                self.assertNotIn(
                    "Traceback",
                    res.stderr,
                    "the gate must never surface an unhandled traceback",
                )

    # --- C-3: missing-required-field entry silently shrinks coverage --------
    # SKIP-GATED (crosslink #261 open): a [[route]] entry with `design` present
    # but `crate_pattern` MISSING (both are "# required" per the
    # spec-routes.toml schema header) falls through the #260 validator (it
    # checks "if present, must be a string" — None is not present) into the
    # builder-era `if not design or not pattern: continue`, so the entry is
    # silently dropped. When it is the ONLY entry the #259 zero-routes guard
    # fires (exit 3, correct); when valid routes exist ALONGSIDE it, the
    # design-field doc silently leaves the checked set and the gate exits 0.
    @unittest.skipUnless(
        os.environ.get("DOC_DRIFT_DIVERGENCE"),
        "divergence: a [[route]] entry with design but no crate_pattern is "
        "silently dropped alongside valid routes — its doc leaves the checked "
        "set and the gate exits 0; REQ-1/REQ-8/REQ-9; tracking crosslink #261",
    )
    def test_c3_design_only_route_entry_must_not_vanish_into_exit_0(self):
        """REQ-5/REQ-1/REQ-8/REQ-9: a design-field doc never silently leaves
        the checked set.

        Authority: REQ-5 defines the routed-doc set as "every doc referenced
        by a [[route]].design field"; REQ-1 says the checked set is "exactly
        the deduplicated design fields"; REQ-8 (no grandfathering) makes a
        routed doc without an audited-sha line a MISSING-PIN FAIL naming the
        doc (exit 1 per REQ-9). Alternatively, since the spec-routes.toml
        schema header marks crate_pattern "# required", the entry is legally
        treatable as wrong-shaped -> the #260 ENVIRONMENT class (exit 3).
        EITHER reading forbids the observed behavior: exit 0 with
        .design/orphan.md invisible — REQ-9's "the tool never exits 0
        without having checked all 48 docs" + R-HONEST-3, scoped to one doc
        instead of the whole table. Realistic trigger: a typo'd field name
        (`crate_patern =`) in one entry silently shrinks coverage while the
        gate stays green.
        """
        fx = Fixture(self.tmp / "c3")
        fx.write(
            "tooling/spec-routes.toml",
            # Entry #0: design present, crate_pattern MISSING (schema-invalid).
            '[[route]]\ndesign = ".design/orphan.md"\n\n'
            # Entry #1: a fully valid, CURRENT route so the #259 zero-routes
            # guard does not mask the per-entry hole.
            '[[route]]\ncrate_pattern = "src/a.rs"\ndesign = ".design/good.md"\n',
        )
        fx.commit("src/a.rs", "v1\n", "A: a v1")
        fx.write_doc(".design/good.md", fx.head())
        # .design/orphan.md does not exist and has no pin: REQ-8's plain
        # reading demands MISSING-PIN for it (it IS a design field).

        res = fx.run_gate()
        self.assertIn(
            res.returncode,
            (1, REQ9_EXIT_INCONCLUSIVE),
            "a routed (design-field) doc dropped by a missing required "
            "crate_pattern must be MISSING-PIN (exit 1, REQ-8) or a "
            "wrong-shaped-table INCONCLUSIVE (exit 3, REQ-9) — never a green "
            f"0; got {res.returncode}; stdout={res.stdout!r} "
            f"stderr={res.stderr!r}",
        )
        if res.returncode == 1:
            # The REQ-8 path must name the doc and the defect class.
            self.assertIn("MISSING-PIN", res.stdout)
            self.assertIn(".design/orphan.md", res.stdout)
        self.assertNotIn(
            "Traceback",
            res.stderr,
            "the gate must never surface an unhandled traceback",
        )


if __name__ == "__main__":
    unittest.main()
