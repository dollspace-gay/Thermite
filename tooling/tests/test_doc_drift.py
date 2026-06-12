#!/usr/bin/env python3
"""
Oracle fixture tests for tooling/doc-drift.py.

tooling/ had NO test convention before this gate (its two shipped hooks are
untested); this introduces the first one, per
.design/tooling/doc-drift-tripwire.md "Verification". Each test builds a
throwaway git repo in a tmpdir (git init + a hermetic user.name/email + a mini
spec-routes.toml + docs + governed files), then runs the gate via subprocess
with `--root <fixture>` and asserts against HAND-AUTHORED oracle facts — never
the tool's own output (R-CHAR-3).

Runnable as:  python3 -m unittest discover -s tooling/tests

The oracle (the spec's expected values, not the tool's):

  O-1  (AC-1 DRIFT):      doc pinned at A; file modified+committed in B>A
                          -> exit 1; output names the doc, the file, the
                          literal token DRIFT, and B's full SHA.
  O-2  (AC-2 CURRENT):    doc pinned at HEAD, file last touched before pin
                          -> exit 0; one CURRENT line per doc.
  O-3  (AC-3 MISSING):    doc with no audited-sha line -> exit 1; MISSING-PIN
                          + the doc path.
  O-4a (AC-4 INVALID):    pin = non-resolving 40-hex -> exit 1; INVALID-PIN +
                          doc path; DRIFT absent for that doc.
  O-4b (AC-4 INVALID):    pin resolves but is not an ancestor of HEAD
                          -> exit 1; INVALID-PIN.
  O-5  (AC-5 INCONCLUSIVE): --root at a non-git tmpdir -> exit 3, no Traceback
                          on stderr.
  O-6  (AC-6 unbuilt):    route whose crate_pattern was never committed
                          -> that doc is CURRENT (exit 0 when otherwise clean).
  O-7  (AC-8 determinism): two runs on the unchanged fixture -> byte-identical
                          stdout.
  O-8  (multi-file doc):  one doc governing TWO files, only one drifted
                          -> exit 1; the drifted file named, the current file
                          NOT named in a DRIFT line.
"""

import os
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

# The gate under test: tooling/doc-drift.py, two levels up from this file.
GATE = Path(__file__).resolve().parents[1] / "doc-drift.py"


def _git(repo, *args, env=None, check=True):
    """Run a git command in `repo`, returning stdout (stripped)."""
    proc = subprocess.run(
        ["git", "-C", str(repo), *args],
        capture_output=True,
        text=True,
        env=env,
    )
    if check and proc.returncode != 0:
        raise AssertionError(
            f"git {' '.join(args)} failed ({proc.returncode}): {proc.stderr}"
        )
    return proc.stdout.strip()


class Fixture:
    """A throwaway git repo with a controllable route table + docs + files."""

    def __init__(self, path):
        self.path = Path(path)
        self.path.mkdir(parents=True, exist_ok=True)
        # Hermetic identity + a deterministic default branch so HEAD always
        # resolves regardless of the host git's init.defaultBranch.
        env = dict(os.environ)
        env["GIT_AUTHOR_NAME"] = "Fixture"
        env["GIT_AUTHOR_EMAIL"] = "fixture@example.com"
        env["GIT_COMMITTER_NAME"] = "Fixture"
        env["GIT_COMMITTER_EMAIL"] = "fixture@example.com"
        self.env = env
        _git(self.path, "init", "-q", env=env)
        _git(self.path, "checkout", "-q", "-b", "main", env=env)
        _git(self.path, "config", "user.name", "Fixture", env=env)
        _git(self.path, "config", "user.email", "fixture@example.com", env=env)
        (self.path / "tooling").mkdir(parents=True, exist_ok=True)

    def write(self, relpath, content):
        p = self.path / relpath
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(content, encoding="utf-8")
        return p

    def write_routes(self, routes):
        """routes: list of (crate_pattern, design)."""
        blocks = []
        for pattern, design in routes:
            blocks.append(
                "[[route]]\n"
                f'crate_pattern = "{pattern}"\n'
                f'design = "{design}"\n'
                "reference = []\n"
                "conformance_ops = []\n"
            )
        self.write("tooling/spec-routes.toml", "\n".join(blocks))

    def write_doc(self, relpath, pin):
        """Write a routed doc with an HTML-comment header. pin=None -> no pin."""
        pin_line = f"audited-sha: {pin}\n" if pin is not None else ""
        self.write(
            relpath,
            "# Fixture doc\n\n<!--\ntier: 3-component\nstatus: draft\n"
            f"{pin_line}-->\n\nbody\n",
        )

    def commit(self, relpath, content, message):
        """Write `content` to `relpath`, stage it, commit; return full SHA."""
        self.write(relpath, content)
        _git(self.path, "add", relpath, env=self.env)
        _git(self.path, "commit", "-q", "-m", message, env=self.env)
        return _git(self.path, "rev-parse", "HEAD", env=self.env)

    def head(self):
        return _git(self.path, "rev-parse", "HEAD", env=self.env)

    def run_gate(self, env=None):
        """Run the gate with --root at this fixture; return CompletedProcess."""
        return subprocess.run(
            [sys.executable, str(GATE), "--root", str(self.path)],
            capture_output=True,
            text=True,
            env=env if env is not None else self.env,
        )


class DocDriftOracleTest(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.tmp = Path(self._tmp.name)

    def tearDown(self):
        self._tmp.cleanup()

    # --- O-1: DRIFT (AC-1) --------------------------------------------------
    def test_o1_drift_names_doc_file_token_and_sha(self):
        fx = Fixture(self.tmp / "o1")
        fx.write_routes([("src/widget.rs", ".design/widget.md")])
        # Commit A: the file as the doc last saw it.
        sha_a = fx.commit("src/widget.rs", "v1\n", "A: widget v1")
        # Pin the doc at A (doc itself need not be committed — it just exists).
        fx.write_doc(".design/widget.md", sha_a)
        # Commit B (> A): the file changes after the pin.
        sha_b = fx.commit("src/widget.rs", "v2\n", "B: widget v2")

        res = fx.run_gate()
        self.assertEqual(res.returncode, 1, res.stdout + res.stderr)
        self.assertIn("DRIFT", res.stdout)
        self.assertIn(".design/widget.md", res.stdout)
        self.assertIn("src/widget.rs", res.stdout)
        self.assertIn(sha_b, res.stdout)

    # --- O-2: CURRENT (AC-2) ------------------------------------------------
    def test_o2_current_one_line_per_doc(self):
        fx = Fixture(self.tmp / "o2")
        fx.write_routes([("src/a.rs", ".design/a.md")])
        fx.commit("src/a.rs", "v1\n", "A: a v1")
        # Pin at HEAD: the file's last touch is at-or-before the pin.
        fx.write_doc(".design/a.md", fx.head())

        res = fx.run_gate()
        self.assertEqual(res.returncode, 0, res.stdout + res.stderr)
        current_lines = [ln for ln in res.stdout.splitlines() if "CURRENT" in ln]
        self.assertEqual(len(current_lines), 1)
        self.assertIn(".design/a.md", current_lines[0])

    # --- O-3: MISSING-PIN (AC-3) --------------------------------------------
    def test_o3_missing_pin(self):
        fx = Fixture(self.tmp / "o3")
        fx.write_routes([("src/a.rs", ".design/a.md")])
        fx.commit("src/a.rs", "v1\n", "A: a v1")
        fx.write_doc(".design/a.md", None)  # no audited-sha line

        res = fx.run_gate()
        self.assertEqual(res.returncode, 1, res.stdout + res.stderr)
        self.assertIn("MISSING-PIN", res.stdout)
        self.assertIn(".design/a.md", res.stdout)

    # --- O-4a: INVALID-PIN, non-resolving 40-hex (AC-4) ---------------------
    def test_o4a_invalid_pin_nonresolving(self):
        fx = Fixture(self.tmp / "o4a")
        fx.write_routes([("src/a.rs", ".design/a.md")])
        fx.commit("src/a.rs", "v1\n", "A: a v1")
        bogus = "0123456789abcdef" * 2 + "01234567"  # 40 hex, won't resolve
        self.assertEqual(len(bogus), 40)
        fx.write_doc(".design/a.md", bogus)

        res = fx.run_gate()
        self.assertEqual(res.returncode, 1, res.stdout + res.stderr)
        self.assertIn("INVALID-PIN", res.stdout)
        self.assertIn(".design/a.md", res.stdout)
        # Textually distinct from DRIFT for that doc.
        doc_lines = [
            ln for ln in res.stdout.splitlines() if ".design/a.md" in ln
        ]
        self.assertTrue(doc_lines)
        for ln in doc_lines:
            self.assertNotIn("DRIFT", ln)

    # --- O-4b: INVALID-PIN, resolves but not an ancestor (AC-4) -------------
    def test_o4b_invalid_pin_non_ancestor(self):
        fx = Fixture(self.tmp / "o4b")
        fx.write_routes([("src/a.rs", ".design/a.md")])
        base = fx.commit("src/a.rs", "v1\n", "A: a v1")
        # A side-branch commit, NOT reachable from main's HEAD.
        _git(fx.path, "checkout", "-q", "-b", "side", env=fx.env)
        side = fx.commit("src/sidefile.rs", "s\n", "side commit")
        _git(fx.path, "checkout", "-q", "main", env=fx.env)
        # Advance main so `side` is genuinely off the HEAD line.
        fx.commit("src/a.rs", "v2\n", "B: a v2")
        self.assertNotEqual(side, base)
        fx.write_doc(".design/a.md", side)

        res = fx.run_gate()
        self.assertEqual(res.returncode, 1, res.stdout + res.stderr)
        self.assertIn("INVALID-PIN", res.stdout)
        self.assertIn(".design/a.md", res.stdout)

    # --- O-5: INCONCLUSIVE outside a git repo (AC-5) ------------------------
    def test_o5_non_git_root_exits_3_no_traceback(self):
        non_git = self.tmp / "not-a-repo"
        non_git.mkdir()
        res = subprocess.run(
            [sys.executable, str(GATE), "--root", str(non_git)],
            capture_output=True,
            text=True,
        )
        self.assertEqual(res.returncode, 3, res.stdout + res.stderr)
        self.assertNotIn("Traceback", res.stderr)

    def test_o5b_git_shadowed_off_path_exits_3(self):
        """PATH stripped of git -> exit 3, no traceback (AC-5 alt form)."""
        fx = Fixture(self.tmp / "o5b")
        fx.write_routes([("src/a.rs", ".design/a.md")])
        fx.commit("src/a.rs", "v1\n", "A: a v1")
        fx.write_doc(".design/a.md", fx.head())
        env = dict(fx.env)
        env["PATH"] = ""  # no git resolvable
        res = fx.run_gate(env=env)
        self.assertEqual(res.returncode, 3, res.stdout + res.stderr)
        self.assertNotIn("Traceback", res.stderr)

    # --- O-6: never-committed route -> CURRENT (AC-6) -----------------------
    def test_o6_uncommitted_file_is_current(self):
        fx = Fixture(self.tmp / "o6")
        fx.write_routes([("src/never.rs", ".design/a.md")])
        # Commit something unrelated so HEAD exists, but never src/never.rs.
        fx.commit("src/other.rs", "v1\n", "A: other v1")
        fx.write_doc(".design/a.md", fx.head())

        res = fx.run_gate()
        self.assertEqual(res.returncode, 0, res.stdout + res.stderr)
        self.assertIn("CURRENT", res.stdout)
        self.assertNotIn("DRIFT", res.stdout)

    # --- O-7: determinism (AC-8) --------------------------------------------
    def test_o7_two_runs_byte_identical(self):
        fx = Fixture(self.tmp / "o7")
        fx.write_routes(
            [
                ("src/a.rs", ".design/a.md"),
                ("src/b.rs", ".design/b.md"),
            ]
        )
        sha_a = fx.commit("src/a.rs", "v1\n", "A")
        fx.commit("src/b.rs", "v1\n", "B")
        fx.write_doc(".design/a.md", sha_a)  # a.rs drifted (b committed after)
        fx.write_doc(".design/b.md", fx.head())

        first = fx.run_gate()
        second = fx.run_gate()
        self.assertEqual(first.stdout, second.stdout)
        self.assertEqual(first.returncode, second.returncode)

    # --- O-8: multi-file doc, only one file drifted -------------------------
    def test_o8_multi_file_doc_only_drifted_named(self):
        fx = Fixture(self.tmp / "o8")
        fx.write_routes(
            [
                ("src/stable.rs", ".design/shared.md"),
                ("src/moving.rs", ".design/shared.md"),
            ]
        )
        fx.commit("src/stable.rs", "v1\n", "stable v1")
        pin = fx.commit("src/moving.rs", "v1\n", "moving v1")
        # Pin AFTER both files' first commit; then move only moving.rs.
        fx.write_doc(".design/shared.md", pin)
        fx.commit("src/moving.rs", "v2\n", "moving v2")

        res = fx.run_gate()
        self.assertEqual(res.returncode, 1, res.stdout + res.stderr)
        drift_lines = [ln for ln in res.stdout.splitlines() if "DRIFT" in ln]
        self.assertTrue(drift_lines)
        joined = "\n".join(drift_lines)
        self.assertIn("src/moving.rs", joined)
        # The stable file must not appear in any DRIFT line.
        self.assertNotIn("src/stable.rs", joined)


if __name__ == "__main__":
    unittest.main()
