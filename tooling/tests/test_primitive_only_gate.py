#!/usr/bin/env python3
"""Hand-authored adversarial oracles for the primitive-only tracked-source gate."""

import hashlib
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


GATE = Path(__file__).resolve().parents[1] / "primitive-only-gate.py"
REPO_ROOT = GATE.parents[1]

PINNED_FIXTURE_HASHES = {
    "conformance/verified-build/kernel_consumer.rs": (
        "33c80731f3c68c6784e9c50aa324ddd885251f363b75c43ffd34a250fc02b511"
    ),
    "conformance/verified-build/closed_result_enum_consumer.rs": (
        "bf1459e9e86312e714cee2c400a3ca741daf1588729de55ab40f676f517175bd"
    ),
    "conformance/verified-composition/kernel_bytes_freestanding.rs": (
        "0dc072e81e8bed5094ac1b6fd11c1a8ffd42e1e888b0d5533e1dcf3e270e6978"
    ),
}


def _git(root, *args):
    return subprocess.run(
        ["git", "-C", str(root), *args],
        check=True,
        capture_output=True,
        text=True,
    )


def _repo(tmp):
    root = Path(tmp)
    _git(root, "init", "--quiet")
    return root


def _write(root, relpath, data, *, track=True):
    path = root / relpath
    path.parent.mkdir(parents=True, exist_ok=True)
    if isinstance(data, bytes):
        path.write_bytes(data)
    else:
        path.write_text(data, encoding="utf-8")
    if track:
        _git(root, "add", "--", relpath)
    return path


def _run(root=None, *, cwd=None):
    argv = [sys.executable, str(GATE)]
    if root is not None:
        argv += ["--root", str(root)]
    proc = subprocess.run(argv, cwd=cwd, capture_output=True, text=True)
    return proc.returncode, proc.stdout, proc.stderr


class TestPrimitiveOnlyGate(unittest.TestCase):
    def test_o1_canonical_primitive_sources_pass(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = _repo(tmp)
            _write(root, "README.md", "primitive repository\n")
            _write(
                root,
                "stdlib/kernel-primitives/array.th",
                "fn get(x: u64) -> u64 { x }\n",
            )
            _write(root, "forge/tests/primitive.rs", "#[test]\nfn primitive() {}\n")
            code, out, err = _run(root)
            self.assertEqual(code, 0, out + err)
            self.assertIn("PRIMITIVE-ONLY tracked=3 inspected=3", out)
            self.assertNotIn("FORBIDDEN", out)

    def test_o2_untracked_dist_is_ignored(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = _repo(tmp)
            _write(root, "README.md", "tracked\n")
            _write(root, "dist/release.zip", b"not a real archive", track=False)
            code, out, err = _run(root)
            self.assertEqual(code, 0, out + err)
            self.assertIn("tracked=1 inspected=1", out)

    def test_o3_tracked_dist_archive_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = _repo(tmp)
            _write(root, "dist/release.zip", b"tracked artifact")
            code, out, err = _run(root)
            self.assertEqual(code, 1, out + err)
            self.assertIn("FORBIDDEN-PATH  dist/release.zip", out)
            self.assertIn("FORBIDDEN-ARTIFACT  dist/release.zip", out)

    def test_o4_concrete_implementation_components_fail_exactly(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = _repo(tmp)
            for relpath in (
                "kernel/src/main.rs",
                "firmware/main.rs",
                "boot/start.S",
                "arch/x86_64/interrupts.rs",
                "images/README.md",
            ):
                _write(root, relpath, "placeholder\n")
            # The similarly named primitive library is deliberately valid.
            _write(
                root,
                "stdlib/kernel-primitives/capability.th",
                "fn cap() -> u64 { 1 }\n",
            )
            code, out, err = _run(root)
            self.assertEqual(code, 1, out + err)
            self.assertEqual(out.count("FORBIDDEN-PATH"), 5, out)
            self.assertNotIn("FORBIDDEN-PATH  stdlib/kernel-primitives", out)

    def test_o5_every_release_artifact_family_fails(self):
        suffixes = (
            "efi",
            "img",
            "iso",
            "zip",
            "tar.gz",
            "elf",
            "o",
            "rlib",
            "rom",
            "qcow2",
        )
        with tempfile.TemporaryDirectory() as tmp:
            root = _repo(tmp)
            for suffix in suffixes:
                _write(root, f"tests/artifact.{suffix}", b"text fixture")
            code, out, err = _run(root)
            self.assertEqual(code, 1, out + err)
            self.assertEqual(out.count("FORBIDDEN-ARTIFACT"), len(suffixes), out)

    def test_o6_hidden_rust_assembly_and_linker_entries_fail(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = _repo(tmp)
            _write(
                root,
                "forge/src/hidden_entry.rs",
                '#![no_std]\n#![no_main]\n#[no_mangle]\npub extern "C" fn thermite_entry() -> ! { loop {} }\n',
            )
            _write(root, "scripts/entry.S", ".global _start\n_start:\n  hlt\n")
            _write(root, "tests/consumer.ld", "ENTRY(runtime_entry)\nSECTIONS {}\n")
            code, out, err = _run(root)
            self.assertEqual(code, 1, out + err)
            self.assertEqual(out.count("FREESTANDING-ENTRY"), 3, out)
            self.assertIn("no_main exported C entry", out)
            self.assertIn("assembly global entry", out)
            self.assertIn("linker ENTRY directive", out)

    def test_o7_exact_pinned_fixtures_pass_and_one_byte_drift_fails(self):
        for relpath, expected_hash in PINNED_FIXTURE_HASHES.items():
            with self.subTest(relpath=relpath), tempfile.TemporaryDirectory() as tmp:
                source = (REPO_ROOT / relpath).read_bytes()
                self.assertEqual(hashlib.sha256(source).hexdigest(), expected_hash)
                root = _repo(tmp)
                fixture = _write(root, relpath, source)
                code, out, err = _run(root)
                self.assertEqual(code, 0, out + err)
                self.assertIn("pinned_fixtures=1", out)

                fixture.write_bytes(source + b"\n")
                code, out, err = _run(root)
                self.assertEqual(code, 1, out + err)
                self.assertIn(f"FIXTURE-DRIFT  {relpath}", out)

    @unittest.skipUnless(hasattr(os, "symlink"), "platform has no symlink support")
    def test_o8_symlink_and_missing_tracked_file_fail(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = _repo(tmp)
            _write(root, "README.md", "target\n")
            link = root / "scripts" / "alias"
            link.parent.mkdir(parents=True)
            os.symlink("../README.md", link)
            _git(root, "add", "--", "scripts/alias")
            missing = _write(root, "tests/missing.rs", "fn missing() {}\n")
            missing.unlink()
            code, out, err = _run(root)
            self.assertEqual(code, 1, out + err)
            self.assertIn("TRACKED-SYMLINK  scripts/alias", out)
            self.assertIn("MISSING-TRACKED  tests/missing.rs", out)

    def test_o9_new_top_level_and_misplaced_source_fail(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = _repo(tmp)
            _write(root, "payload/notes.md", "new unreviewed root\n")
            _write(root, "docs/executable.rs", "fn helper() {}\n")
            code, out, err = _run(root)
            self.assertEqual(code, 1, out + err)
            self.assertIn("UNCLASSIFIED-TOPLEVEL  payload/notes.md", out)
            self.assertIn("UNCLASSIFIED-SOURCE  docs/executable.rs", out)

    def test_o10_output_is_deterministic_and_non_git_is_inconclusive(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = _repo(tmp)
            _write(root, "kernel/main.rs", "fn kernel_main() {}\n")
            first = _run(root)
            second = _run(root)
            self.assertEqual(first, second)

        with tempfile.TemporaryDirectory() as tmp:
            code, out, err = _run(cwd=tmp)
            self.assertEqual(code, 3, out + err)
            self.assertIn("INCONCLUSIVE", err)
            self.assertNotIn("Traceback", err)


if __name__ == "__main__":
    unittest.main()
