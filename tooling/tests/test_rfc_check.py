#!/usr/bin/env python3
"""Oracle tests for tooling/rfc-check.py.

Each test asserts on the reported message rather than only the exit code. A gate
that fails for the wrong reason is a gate that sends the next reader to the wrong
place, and an exit-code-only test cannot tell the two apart.
"""

import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path


GATE = Path(__file__).resolve().parents[1] / "rfc-check.py"


class Fixture:
    def __init__(self, root: Path):
        self.root = root
        (root / ".design" / "rfcs").mkdir(parents=True, exist_ok=True)
        (root / ".design" / "reqs").mkdir(parents=True, exist_ok=True)
        (root / ".design" / "reqs" / "registry.toml").write_text(
            'id = "REQ-KNOWN"\n', encoding="utf-8"
        )

    def rfc(self, name: str, body: str):
        p = self.root / ".design" / "rfcs" / name
        p.write_text(textwrap.dedent(body).lstrip(), encoding="utf-8")
        return p

    def run(self, *args):
        return subprocess.run(
            [sys.executable, str(GATE), "--root", str(self.root), *args],
            capture_output=True,
            text=True,
        )


def front(rfc: int, status: str = "draft", title: str = "A title", extra: str = "") -> str:
    return f"---\nrfc: {rfc}\ntitle: {title}\nstatus: {status}\n{extra}---\n\nbody\n"


class RfcCheckTests(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.fx = Fixture(Path(self._tmp.name))

    def tearDown(self):
        self._tmp.cleanup()

    def test_well_formed_rfc_passes(self):
        self.fx.rfc("0001-a-thing.md", front(1, "accepted"))
        r = self.fx.run()
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn("ok", r.stdout)

    def test_missing_front_matter_is_named(self):
        self.fx.rfc("0001-a-thing.md", "# no front matter\n")
        r = self.fx.run()
        self.assertEqual(r.returncode, 1)
        self.assertIn("no front matter", r.stderr)

    def test_unclosed_front_matter_is_named(self):
        self.fx.rfc("0001-a-thing.md", "---\nrfc: 1\ntitle: T\nstatus: draft\n")
        r = self.fx.run()
        self.assertEqual(r.returncode, 1)
        self.assertIn("not closed", r.stderr)

    def test_missing_required_field_is_named(self):
        self.fx.rfc("0001-a-thing.md", "---\nrfc: 1\nstatus: draft\n---\n\nbody\n")
        r = self.fx.run()
        self.assertEqual(r.returncode, 1)
        self.assertIn("missing `title`", r.stderr)

    def test_implemented_is_not_a_status(self):
        """Implementation is derived from the REQ registry, never declared."""
        self.fx.rfc("0001-a-thing.md", front(1, "implemented"))
        r = self.fx.run()
        self.assertEqual(r.returncode, 1)
        self.assertIn("derived from the REQ registry", r.stderr)

    def test_number_must_match_filename(self):
        self.fx.rfc("0007-a-thing.md", front(9))
        r = self.fx.run()
        self.assertEqual(r.returncode, 1)
        self.assertIn("disagrees with the filename prefix", r.stderr)

    def test_filename_shape_is_enforced(self):
        self.fx.rfc("not-an-rfc.md", front(1))
        r = self.fx.run()
        self.assertEqual(r.returncode, 1)
        self.assertIn("NNNN-slug.md", r.stderr)

    def test_unknown_introduced_req_is_rejected(self):
        self.fx.rfc("0001-a-thing.md", front(1, extra="introduces: [REQ-NOPE]\n"))
        r = self.fx.run()
        self.assertEqual(r.returncode, 1)
        self.assertIn("REQ-NOPE", r.stderr)

    def test_known_introduced_req_is_accepted(self):
        self.fx.rfc("0001-a-thing.md", front(1, extra="introduces: [REQ-KNOWN]\n"))
        self.assertEqual(self.fx.run().returncode, 0)

    def test_supersedes_must_exist(self):
        self.fx.rfc("0002-b.md", front(2, extra="supersedes: [1]\n"))
        r = self.fx.run()
        self.assertEqual(r.returncode, 1)
        self.assertIn("supersedes RFC-1", r.stderr)

    # --- the provisional-number rule -------------------------------------

    def test_two_drafts_may_share_a_number(self):
        """A draft's number is provisional; the collision resolves at merge."""
        self.fx.rfc("0004-one.md", front(4, "draft"))
        self.fx.rfc("0004-two.md", front(4, "draft", title="Rival"))
        r = self.fx.run()
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn("provisional until merge", r.stderr)

    def test_draft_may_not_take_a_canonical_number(self):
        self.fx.rfc("0004-one.md", front(4, "accepted"))
        self.fx.rfc("0004-two.md", front(4, "draft", title="Rival"))
        r = self.fx.run()
        self.assertEqual(r.returncode, 1)
        self.assertIn("already used by", r.stderr)

    def test_two_canonical_rfcs_may_not_share(self):
        self.fx.rfc("0004-one.md", front(4, "accepted"))
        self.fx.rfc("0004-two.md", front(4, "rejected", title="Rival"))
        r = self.fx.run()
        self.assertEqual(r.returncode, 1)
        self.assertIn("already used by", r.stderr)

    # --- the derived index ------------------------------------------------

    def test_index_reports_unversioned_outside_git(self):
        self.fx.rfc("0001-a-thing.md", front(1, "accepted"))
        r = self.fx.run("--index")
        self.assertEqual(r.returncode, 0)
        self.assertIn("unversioned", r.stdout)

    def test_index_derives_the_revision_from_git(self):
        """r<n> is the count of commits touching the file, not a declared field."""
        root = self.fx.root
        env = {"GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@e",
               "GIT_COMMITTER_NAME": "t", "GIT_COMMITTER_EMAIL": "t@e"}
        def git(*a):
            subprocess.run(["git", *a], cwd=root, check=True,
                           capture_output=True, env={**dict(**env), "PATH": "/usr/bin:/bin:/usr/local/bin"})
        git("init", "-q")
        self.fx.rfc("0001-a-thing.md", front(1, "accepted"))
        git("add", "-A"); git("commit", "-q", "-m", "one")
        self.fx.rfc("0001-a-thing.md", front(1, "accepted", title="Retitled"))
        git("add", "-A"); git("commit", "-q", "-m", "two")

        r = self.fx.run("--index", "--json")
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn('"revision": 2', r.stdout)


if __name__ == "__main__":
    unittest.main()
