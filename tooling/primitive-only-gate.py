#!/usr/bin/env python3
"""Fail if the tracked source closure contains a concrete kernel artifact.

The Git index is the canonical allowlist. Untracked files are intentionally
ignored. See ``.design/tooling/primitive-only-gate.md`` for the policy.

Exit codes:

* 0: every tracked path satisfies the primitive-only policy
* 1: one or more deterministic findings were found
* 3: Git or the repository could not be inspected reliably
"""

import argparse
import hashlib
import re
import subprocess
import sys
from pathlib import Path, PurePosixPath


EXIT_OK = 0
EXIT_FAIL = 1
EXIT_INCONCLUSIVE = 3

ALLOWED_TOP_LEVEL_DIRS = frozenset(
    {
        ".claude",
        ".crosslink",
        ".design",
        ".github",
        "conformance",
        "docs",
        "examples",
        "forge",
        "lean",
        "scripts",
        "stdlib",
        "tests",
        "thermite-lower",
        "thermite-skill",
        "thermite-spec",
        "thermite-syntax",
        "thermite-tv",
        "thermite-verified",
        "tooling",
    }
)

ALLOWED_TOP_LEVEL_FILES = frozenset(
    {
        ".gitignore",
        ".mcp.json",
        "CHANGELOG.md",
        "Cargo.lock",
        "Cargo.toml",
        "LICENSE",
        "Makefile",
        "RATIONALE.md",
        "README.md",
        "THERMITE.skill.md",
        "goal.md",
        "rust-toolchain.toml",
        "thermite-design.md",
        "thermite2-semantics.md",
    }
)

# Source-bearing roots are narrower than the top-level metadata/document roots.
# Adding a new compiler crate or source root must update this policy explicitly.
ALLOWED_SOURCE_ROOTS = frozenset(
    {
        "conformance",
        "examples",
        "forge",
        "lean",
        "scripts",
        "stdlib",
        "tests",
        "thermite-lower",
        "thermite-skill",
        "thermite-spec",
        "thermite-syntax",
        "thermite-tv",
        "thermite-verified",
        "tooling",
    }
)

SOURCE_SUFFIXES = frozenset(
    {
        ".asm",
        ".c",
        ".cc",
        ".cpp",
        ".h",
        ".hpp",
        ".ld",
        ".lds",
        ".lean",
        ".py",
        ".rs",
        ".s",
        ".sh",
        ".th",
        ".x",
    }
)

INCIDENTAL_COMPONENTS = frozenset(
    {
        ".cache",
        ".coverage",
        ".mypy_cache",
        ".pytest_cache",
        ".ruff_cache",
        ".tox",
        ".venv",
        "__pycache__",
        "coverage",
        "dist",
        "node_modules",
        "target",
        "venv",
    }
)

CONCRETE_COMPONENTS = frozenset(
    {
        "arch",
        "architecture",
        "boot",
        "bootloader",
        "efi",
        "firmware",
        "image",
        "images",
        "kernel",
        "kernels",
        "uefi",
    }
)

FORBIDDEN_ARTIFACT_SUFFIXES = (
    ".tar.gz",
    ".tar.bz2",
    ".tar.xz",
    ".qcow2",
    ".vhdx",
    ".dylib",
    ".rlib",
    ".tar.zst",
    ".tar",
    ".tgz",
    ".tbz2",
    ".txz",
    ".efi",
    ".img",
    ".iso",
    ".zip",
    ".gz",
    ".bz2",
    ".xz",
    ".zst",
    ".bin",
    ".elf",
    ".obj",
    ".o",
    ".a",
    ".so",
    ".dll",
    ".exe",
    ".fd",
    ".rom",
    ".vhd",
    ".vmdk",
    ".deb",
    ".rpm",
)

PINNED_FREESTANDING_FIXTURES = {
    "conformance/verified-build/kernel_consumer.rs": (
        "33c80731f3c68c6784e9c50aa324ddd885251f363b75c43ffd34a250fc02b511"
    ),
    "conformance/verified-composition/kernel_bytes_freestanding.rs": (
        "0dc072e81e8bed5094ac1b6fd11c1a8ffd42e1e888b0d5533e1dcf3e270e6978"
    ),
}

NO_MAIN_RE = re.compile(rb"#!\s*\[\s*no_main\s*\]")
EXPORTED_ENTRY_RE = re.compile(
    rb"#\s*\[\s*(?:no_mangle|unsafe\s*\(\s*no_mangle\s*\))\s*\]"
    rb"[\s\S]{0,240}?\bextern\s*(?:\"[^\"]+\")?\s*fn\s+[A-Za-z_][A-Za-z0-9_]*\s*\("
)
NAMED_ENTRY_RE = re.compile(
    rb"\bfn\s+(?:_start|efi_main|kernel_main|kmain|boot_entry)\s*\("
)
C_ENTRY_RE = re.compile(
    rb"(?m)^\s*(?:void|int|unsigned|long|__attribute__\s*\([^\n]+\)\s*)+\s*"
    rb"(?:_start|efi_main|kernel_main|kmain|boot_entry)\s*\("
)
ASM_GLOBAL_ENTRY_RE = re.compile(
    rb"(?mi)^\s*\.(?:global|globl)\s+(?:_start|efi_main|kernel_main|kmain|boot_entry)\b"
)
ASM_ENTRY_LABEL_RE = re.compile(
    rb"(?mi)^\s*(?:_start|efi_main|kernel_main|kmain|boot_entry)\s*:"
)
LINKER_ENTRY_RE = re.compile(rb"(?mi)\bENTRY\s*\(")
UEFI_ENTRY_ATTRIBUTE_RE = re.compile(rb"#\s*\[\s*(?:entry|uefi::entry)\s*\]")


class EnvironmentError3(Exception):
    """The gate could not inspect the repository reliably."""


def _run_git(root, args):
    try:
        proc = subprocess.run(
            ["git", "-C", str(root), *args],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    except (FileNotFoundError, PermissionError, OSError) as exc:
        raise EnvironmentError3(f"git could not be invoked: {exc}") from exc
    return proc.returncode, proc.stdout, proc.stderr


def _git_toplevel(start):
    rc, out, _ = _run_git(start, ["rev-parse", "--show-toplevel"])
    if rc != 0:
        raise EnvironmentError3(f"not inside a git repository: {start}")
    try:
        top = out.rstrip(b"\n").decode("utf-8", "strict")
    except UnicodeDecodeError as exc:
        raise EnvironmentError3("git toplevel is not valid UTF-8") from exc
    if not top:
        raise EnvironmentError3("git returned an empty repository toplevel")
    return Path(top)


def _display(path):
    return path.encode("unicode_escape", "backslashreplace").decode("ascii")


def _tracked_entries(root):
    rc, out, err = _run_git(root, ["ls-files", "--stage", "-z"])
    if rc != 0:
        detail = err.decode("utf-8", "replace").strip()
        raise EnvironmentError3(f"git ls-files --stage failed: {detail}")

    entries = []
    for raw in out.split(b"\0"):
        if not raw:
            continue
        try:
            metadata, raw_path = raw.split(b"\t", 1)
            mode, object_id, stage = metadata.split(b" ", 2)
            path = raw_path.decode("utf-8", "strict")
            mode_text = mode.decode("ascii", "strict")
            object_text = object_id.decode("ascii", "strict")
            stage_number = int(stage.decode("ascii", "strict"))
        except (ValueError, UnicodeDecodeError) as exc:
            raise EnvironmentError3(
                "git ls-files --stage produced an unparseable entry"
            ) from exc
        entries.append((path, mode_text, object_text, stage_number))
    return entries


def _entry_marker(data, suffix):
    if suffix == ".rs" and NO_MAIN_RE.search(data) and EXPORTED_ENTRY_RE.search(data):
        return "no_main exported C entry"
    patterns = []
    if suffix in {".rs", ".th"}:
        patterns.append((NAMED_ENTRY_RE, "named Rust/Thermite entry"))
    if suffix in {".c", ".cc", ".cpp", ".h", ".hpp"}:
        patterns.append((C_ENTRY_RE, "named C entry"))
    if suffix in {".s", ".asm"}:
        patterns.extend(
            (
                (ASM_GLOBAL_ENTRY_RE, "assembly global entry"),
                (ASM_ENTRY_LABEL_RE, "assembly entry label"),
            )
        )
    if suffix in {".ld", ".lds", ".x"}:
        patterns.append((LINKER_ENTRY_RE, "linker ENTRY directive"))
    if suffix == ".rs":
        patterns.append((UEFI_ENTRY_ATTRIBUTE_RE, "UEFI entry attribute"))
    for pattern, detail in patterns:
        if pattern.search(data):
            return detail
    return None


def evaluate(root):
    """Return ``(exit_code, report_lines)`` for the complete tracked set."""

    findings = []
    entries = _tracked_entries(root)
    inspected = 0
    pinned = 0

    for path, mode, _object_id, stage in entries:
        shown = _display(path)
        pure = PurePosixPath(path)
        parts = pure.parts

        if (
            not parts
            or pure.is_absolute()
            or any(part in {"", ".", ".."} for part in parts)
        ):
            findings.append(
                ("INVALID-PATH", shown, "path is not canonical and relative")
            )
            continue
        if stage != 0:
            findings.append(("UNMERGED-INDEX", shown, f"index stage is {stage}, not 0"))
            continue
        if mode == "120000":
            findings.append(
                ("TRACKED-SYMLINK", shown, "canonical source must be a regular file")
            )
            continue
        if mode == "160000":
            findings.append(("TRACKED-GITLINK", shown, "submodule source closure is not local"))
            continue
        if mode not in {"100644", "100755"}:
            findings.append(("TRACKED-MODE", shown, f"unsupported index mode {mode}"))
            continue

        lowered_parts = tuple(part.casefold() for part in parts)
        forbidden = sorted(
            set(lowered_parts).intersection(
                INCIDENTAL_COMPONENTS | CONCRETE_COMPONENTS
            )
        )
        for component in forbidden:
            findings.append(
                ("FORBIDDEN-PATH", shown, f"forbidden component {component!r}")
            )

        lower_path = path.casefold()
        artifact = next(
            (
                suffix
                for suffix in FORBIDDEN_ARTIFACT_SUFFIXES
                if lower_path.endswith(suffix)
            ),
            None,
        )
        if artifact is not None:
            findings.append(
                ("FORBIDDEN-ARTIFACT", shown, f"forbidden suffix {artifact!r}")
            )

        if len(parts) == 1:
            if path not in ALLOWED_TOP_LEVEL_FILES:
                findings.append(
                    ("UNCLASSIFIED-TOPLEVEL", shown, "top-level file is not allowlisted")
                )
        elif parts[0] not in ALLOWED_TOP_LEVEL_DIRS:
            findings.append(
                (
                    "UNCLASSIFIED-TOPLEVEL",
                    shown,
                    f"top-level root {parts[0]!r} is not allowlisted",
                )
            )

        suffix = pure.suffix.casefold()
        if suffix in SOURCE_SUFFIXES and (
            len(parts) == 1 or parts[0] not in ALLOWED_SOURCE_ROOTS
        ):
            findings.append(
                (
                    "UNCLASSIFIED-SOURCE",
                    shown,
                    "source file is outside an allowlisted source root",
                )
            )

        disk_path = root.joinpath(*parts)
        if not disk_path.is_file():
            findings.append(
                ("MISSING-TRACKED", shown, "tracked worktree file is missing")
            )
            continue
        try:
            data = disk_path.read_bytes()
        except OSError as exc:
            findings.append(
                ("MISSING-TRACKED", shown, f"tracked file is unreadable: {exc}")
            )
            continue

        inspected += 1
        if b"\0" in data:
            findings.append(
                (
                    "BINARY-TRACKED",
                    shown,
                    "tracked source closure must be text-only",
                )
            )
            continue

        expected_digest = PINNED_FREESTANDING_FIXTURES.get(path)
        if expected_digest is not None:
            actual_digest = hashlib.sha256(data).hexdigest()
            if actual_digest != expected_digest:
                findings.append(
                    (
                        "FIXTURE-DRIFT",
                        shown,
                        f"expected sha256 {expected_digest}, found {actual_digest}",
                    )
                )
            else:
                pinned += 1
            continue

        if suffix in SOURCE_SUFFIXES:
            marker = _entry_marker(data, suffix)
            if marker is not None:
                findings.append(("FREESTANDING-ENTRY", shown, marker))

    if findings:
        lines = [
            f"{kind}  {path}  {detail}"
            for kind, path, detail in sorted(set(findings))
        ]
        return EXIT_FAIL, lines

    return EXIT_OK, [
        f"PRIMITIVE-ONLY tracked={len(entries)} inspected={inspected} "
        f"pinned_fixtures={pinned}"
    ]


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root",
        type=Path,
        default=None,
        help="repository root or a path inside it (default: current directory)",
    )
    args = parser.parse_args(argv)

    try:
        root = _git_toplevel(args.root if args.root is not None else Path.cwd())
        code, lines = evaluate(root)
    except EnvironmentError3 as exc:
        print(f"INCONCLUSIVE  {exc}", file=sys.stderr)
        return EXIT_INCONCLUSIVE

    for line in lines:
        print(line)
    return code


if __name__ == "__main__":
    sys.exit(main())
