# Primitive-only tracked-source gate

<!--
tier: 3-component
status: shipped
governs: tooling/primitive-only-gate.py,
         tooling/tests/test_primitive_only_gate.py, the `make primitive-only`
         target, and the primitive-only CI step
audited-content-sha256: 55b373a541513580b91e5abe1676b133862c4a0c457f3d867bdc29838f7db445
goal-refs:
  - kernel-primitives goal item 6 (permanent no concrete kernel/firmware/image gate)
  - user directive (primitives only; no bundled kernel)
  - .design/build/kernel-primitives.md REQ-KPRIM-10
-->

## Decision

Thermite is a reusable language, proof, and freestanding-primitive repository.
It must not silently become the home of a concrete kernel, firmware program,
boot image, release archive, or generated build tree.  The Git index is the
canonical source allowlist: every tracked path is enumerated with
`git ls-files --stage -z`, while untracked developer output is outside the
claim and is deliberately ignored.

`tooling/primitive-only-gate.py` is a fail-closed development-discipline gate.
It runs in CI, through `make primitive-only`, and in `make gauntlet`.  It is not
a proof-trust-chain step and therefore does not run from `scripts/audit.sh`.

This policy is about artifacts and implementation, not vocabulary.  Exact
components such as `kernel`, `firmware`, `boot`, `uefi`, `arch`, and `image` are
forbidden, while `stdlib/kernel-primitives`, documents discussing kernels, and
tests named for kernel-target lowering remain allowed.  New top-level roots or
new source-bearing roots require an explicit policy review rather than silently
expanding the allowlist.

## Canonical tracked set

The gate reads index mode, object identity, stage, and path from the NUL-delimited
`git ls-files --stage -z` output.  It rejects:

1. unmerged index entries, symlinks, gitlinks, non-regular modes, missing
   worktree files, and invalid/non-canonical paths;
2. top-level files or directories absent from the explicit allowlist;
3. source extensions outside the explicit production and conformance roots;
4. tracked binary content (a NUL byte), because this repository's canonical
   source closure is text-only;
5. incidental/generated directory components including `target`, `dist`,
   cache directories, virtual environments, coverage output, and
   `node_modules`;
6. concrete implementation directory components including `kernel`, `boot`,
   `firmware`, `uefi`, `arch`, and `image`, matched as whole components so
   `kernel-primitives` is not a false positive; and
7. boot/release artifact suffixes including EFI, disk/ISO, archive, object,
   library, ROM, and virtual-machine image formats.

Forbidden components and artifact suffixes are compared case-insensitively
after POSIX-component parsing; canonical allowlist spellings remain exact.
Findings are sorted by class, path, and detail, so the report is deterministic.

## Freestanding-entry detection and the two exceptions

A concrete boot program can be text-only and live below an otherwise legitimate
source root.  Source files therefore also fail when they contain a Rust
`#![no_main]` exported C entry, a named `_start`/`efi_main`/`kernel_main` entry,
an assembly global entry label, a linker `ENTRY(...)`, or a UEFI-style entry
attribute.

Two pre-existing conformance files are not boot images.  They are tiny
compile/link-only consumers used to prove that generated primitive libraries
can be consumed without `std`; no emulator or firmware protocol executes them.
They are exceptions only at these exact identities:

| Path | SHA-256 |
|---|---|
| `conformance/verified-build/kernel_consumer.rs` | `33c80731f3c68c6784e9c50aa324ddd885251f363b75c43ffd34a250fc02b511` |
| `conformance/verified-composition/kernel_bytes_freestanding.rs` | `0dc072e81e8bed5094ac1b6fd11c1a8ffd42e1e888b0d5533e1dcf3e270e6978` |

If either tracked file changes, `FIXTURE-DRIFT` fails before entry-marker
inspection.  A new freestanding fixture must be reviewed and added with an
exact digest; a path-based blanket exception is forbidden.

## Findings and exit contract

The stable finding tokens are `INVALID-PATH`, `UNMERGED-INDEX`,
`TRACKED-SYMLINK`, `TRACKED-GITLINK`, `TRACKED-MODE`, `MISSING-TRACKED`,
`UNCLASSIFIED-TOPLEVEL`, `UNCLASSIFIED-SOURCE`, `FORBIDDEN-PATH`,
`FORBIDDEN-ARTIFACT`, `BINARY-TRACKED`, `FIXTURE-DRIFT`, and
`FREESTANDING-ENTRY`.

- exit 0: the complete tracked set satisfies the primitive-only policy;
- exit 1: at least one deterministic finding exists; and
- exit 3: Git or the repository cannot be inspected reliably.

Exit 3 never degrades to success.  Untracked files are ignored by construction,
so a developer's local `dist/` or `target/` does not falsify a statement about
committed source.

## Adversarial acceptance

The fixture-oracle suite constructs temporary Git repositories and pins these
facts independently of the gate implementation:

1. a primitive library and ordinary conformance sources pass;
2. an untracked `dist/` archive is ignored;
3. the same archive tracked in Git fails;
4. concrete kernel, firmware, boot, architecture, and image directories fail;
5. release/archive/object/image suffixes fail;
6. a hidden Rust, assembly, or linker entry below an allowed source root fails;
7. each exact freestanding fixture passes, while a one-byte drift fails;
8. a tracked symlink and a missing tracked worktree file fail;
9. a new top-level root or misplaced source fails;
10. repeated runs are byte-identical, and a non-Git environment exits 3.

The repository itself is then the positive fixture: CI runs the gate over the
entire real index.  This closes the no-concrete-kernel part of REQ-KPRIM-10; the
remaining package, receipt-tamper, machine-refinement, and bundle matrix stays
tracked by the parent kernel-primitives design.

## Assurance boundary

This gate prevents a prohibited artifact from being committed under the known
forms above.  It does not prove semantic non-kernelhood of arbitrary text, and
it does not promote platform declarations to L3/L4.  Bodyful Thermite
application/library primitives retain their ordinary L3/L4 receipt gates;
irreducible consumer-owned platform declarations remain explicitly lower
assurance until exact machine refinement is supplied by the consumer.
