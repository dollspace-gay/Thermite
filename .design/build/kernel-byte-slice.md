# Kernel byte-slice proof model

<!--
tier: 3-component
status: shipped
audited-content-sha256: 7ab8f31cbabf2d46c0fc9cfa9ae079307ed97d8819f6d9b5020d5a853f73c912
decision: explicit pinned vstd proof-model import plus deterministic no_std erased link metadata
issue: github:dollspace-gay/Thermite#108
governs:
  - forge/src/verified_build.rs
  - forge/src/verified_build/composition.rs
  - forge/src/kernel_vstd_link.rs
  - forge/tests/kernel_byte_slice.rs
thesis-refs:
  - thermite-design.md §3
  - thermite-design.md §5.3
  - thermite-design.md §6
  - thermite-design.md §9
-->

## Summary

Kernel composition supports executable, allocation-free reads from `&[u8]`
with contracts over the slice's exact length and element contents. A shell can
therefore prove a little-endian decoder over the same borrowed bytes that its
compiled body reads. The resulting crate remains `#![no_std]`, is compiled
with Verus `--no-vstd --no-cheating`, and links through the existing
freestanding gates without a hosted runtime.

The model is explicit rather than ambient. Forge imports the `vstd.vir` shipped
with the pinned Verus installation and supplies a deterministic erased
`libvstd.rlib` containing only the Rust metadata skeleton for `Seq`, `View`, and
slice indexing. The imported VIR is the semantic authority. The small rlib has
no allocator or hosted implementation and contributes no executable slice
adapter: the checked body still executes Rust's native `&[u8]::len` and
indexing directly.

The same receipt-bound Rust ABI mechanism also preserves `&mut` for mutable
primitive/array-element slices and records it as an exclusive borrow. That is a
generic finite-storage export primitive; this document's imported `vstd` model
continues to describe the slice view and does not add an executable adapter.

## Why the builtins-only profile is insufficient

Under `--no-vstd`, `verus_builtin` provides the verifier language but not the
standard-library model for Rust slices. An executable `bytes[offset]` lowers to
core slice indexing, while the corresponding specification expression needs
`View`, `Seq`, `SliceAdditionalSpecFns::spec_index`, and the specifications for
slice length/index. Without those declarations Verus either reports a missing
`spec_index` method or an undeclared `vstd::slice::spec_slice_len` AIR symbol.

A local wrapper cannot repair that soundly. Any constructor or accessor that
claims its ghost bytes equal an arbitrary input slice must assume the very
core-slice relation that is missing. Adding `assume_specification`,
`external_body`, `assume`, or `admit` to the combined source would create an
unchecked seam and is rejected by both Forge policy and `--no-cheating`.

Using the distributed `libvstd.rlib` directly is also unsuitable: that erased
artifact is built with vstd's default `std` feature. The kernel target instead
needs vstd's already-verified semantic metadata paired with a `no_std` erased
Rust metadata crate.

## Selected architecture

For a kernel build Forge performs two distinct operations:

1. It resolves `vstd.vir` and the complete `vstd/` source tree next to the
   pinned Verus binary. The VIR digest and a canonical file-by-file source-tree
   digest are captured in `KernelVstdModelEvidence`.
2. It writes the embedded `forge/src/kernel_vstd_link.rs` source into a private
   scratch directory and invokes the same pinned Verus/rustc with `--is-vstd
   --no-verify --compile --crate-type=rlib`. This step creates erased Rust
   metadata; it does not create or replace proof semantics. Its source, exact
   normalized arguments, and resulting rlib digest are bound.

The final whole-crate command remains strict and records the portable argument
shape:

```text
--no-vstd
--import vstd=<KERNEL_VSTD_VIR>
--extern vstd=<KERNEL_VSTD_RLIB>
--no-cheating --compile ...
```

At execution time Forge substitutes the exact pinned VIR and generated rlib
paths. A direct-Verus shell that uses slice specifications explicitly imports
`vstd::prelude::*`; ordinary kernel lowering remains builtins-only. The ghost
slice vocabulary survives only in proof position, while executable length and
indexing remain native, allocation-free slice operations.

The link skeleton deliberately mirrors the pinned vstd definition paths and
impl order for the admitted subset. Verus metadata keys external impls by those
paths. Expanding this subset is therefore a reviewed toolchain-model change,
not an implicit glob of executable vstd functionality.

## Binding, validation, and replay

`ToolchainEvidence.kernel_vstd_model` records:

- the exact `vstd.vir` path and SHA-256;
- the full pinned `vstd/` source root, file count, byte count, and canonical
  source-tree SHA-256;
- the erased link source filename and SHA-256;
- its normalized build arguments; and
- the generated no-std `libvstd.rlib` SHA-256.

The bundle contains `evidence/kernel-vstd-link.rs` and the generated rlib at
`artifact/deps/libvstd.rlib`; the ordinary receipt file inventory binds both.
Validation requires the model only for a kernel target, checks that all model
and dependency digests agree, and rejects a missing, duplicated, hosted, or
malformed substitution.

Replay re-resolves the pinned Verus installation, re-hashes its VIR and full
source tree, rebuilds the erased link crate from the current Forge-embedded
source, compares the complete model evidence, and only then reruns the strict
whole-crate proof/codegen. A model, source, stub, compiler, or generated-rmeta
change therefore fails before an artifact can be accepted.

## Exact-content API and proof obligations

The conformance shell defines open specification functions for little-endian
`u32` and `u64` values from `bytes@[offset + n]`. Its executable functions use
the corresponding native `bytes[offset + n]` expressions and require
`offset + width <= bytes.len()`. Verus proves both every bounds obligation and
the exact result equality; there is no copied buffer, wrapper constructor, raw
pointer, allocation, or unverified conversion seam.

The negative matrix is load-bearing:

- a little-endian body with a big-endian content postcondition fails;
- a caller whose slice is shorter than the read width fails the callee's bounds
  precondition;
- the exact combined source is scanned for proof escapes; and
- publication remains absent on either failure.

A host consumer decodes known bytes and checks the observed values. Separate
low (no-std rlib) and high (no-entry-runtime final link) freestanding consumers
link the same artifact and bundled dependencies. Two independent builds must
produce byte-identical receipts, combined source, target rlib, and no-std vstd
link rlib, and receipt replay must reproduce the artifact digest.

## Requirements

<!-- generated:reqs view=forge-kernel-byte-slice-status -->
Source: `.design/reqs/registry.toml`

| ID | Status | Owner | Title | Follow-up |
|---|---|---|---|---|
| REQ-KERNELBYTES-1 | shipped | `.design/build/kernel-byte-slice.md` | Pinned no-std kernel slice proof model |  |
| REQ-KERNELBYTES-2 | shipped | `.design/build/kernel-byte-slice.md` | Exact executable byte-slice content contracts |  |
| REQ-KERNELBYTES-3 | shipped | `.design/build/kernel-byte-slice.md` | Receipt-bound model source and replay |  |
| REQ-KERNELBYTES-4 | shipped | `.design/build/kernel-byte-slice.md` | Bounds and content negatives reject publication |  |
| REQ-KERNELBYTES-5 | shipped | `.design/build/kernel-byte-slice.md` | Reproducible hosted and freestanding consumption |  |
<!-- /generated:reqs -->
