# Kernel byte-slice proof model

<!--
tier: 3-component
status: shipped
audited-content-sha256: d7560e1aa86bedb8a47fcb8e95e7cb47f2c983ce0bc0f68bb4a289c9e198b6f3 (re-pinned 2026-08-05 after registry-v3 selected the full vstd dependency only for machine adapters; byte-slice behavior remains regression-covered)
decision: explicit pinned vstd slice/fixed-array proof-model import plus deterministic no_std erased link metadata
issue: github:dollspace-gay/Thermite#108
governs:
  - forge/src/verified_build.rs
  - forge/src/verified_build/composition.rs
  - forge/src/kernel_vstd_link.rs
  - forge/tests/kernel_byte_slice.rs
  - thermite-lower/src/lower.rs
  - thermite-lower/tests/fixed_array.rs
  - forge/tests/verified_build.rs
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
`libvstd.rlib` containing only the Rust metadata skeleton for `Seq`, `View`,
slice indexing, and native fixed arrays. The imported VIR is the semantic
authority. The small rlib has no allocator or hosted container implementation;
checked bodies still execute Rust's native slice/array indexing and assignment.

The same receipt-bound Rust ABI mechanism also preserves `&mut` for mutable
primitive/array-element slices and records it as an exclusive borrow. That is a
generic finite-storage export primitive; this document's imported `vstd` model
continues to describe the finite view and does not add a collection adapter.

The fixed-array extension imports `ArrayAdditionalSpecFns` and the native array
`View` implementation. Thermite repeat initializers lower to vstd's pinned
`array_fill_for_copy_types` compiler helper because Verus rejects array literal
syntax whenever `--no-vstd` is present, even with an explicit model import. The
no-std link skeleton implements that exact erased helper as native `[value; N]`;
its source, object, imported VIR, and complete vstd source closure are receipt-
bound. This is irreducible compiler construction support, not a bodyful
application primitive. All collection/slab algorithms using it remain Thermite
L3.

## Why the builtins-only profile is insufficient

Under `--no-vstd`, `verus_builtin` provides the verifier language but not the
standard-library model for Rust slices or arrays. An executable
`bytes[offset]` or `slots[index]` lowers to core indexing, while the corresponding
specification needs `View`, `Seq`, the slice/array additional spec traits, and
the length/index/update laws. Without them Verus reports missing `view` or
`spec_index` methods or an undeclared vstd AIR symbol.

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
paths. Kernel lowering and same-crate direct-Verus shells explicitly import
`vstd::prelude::*` from that closed dependency. Registry-v2 scalar primitive
crates use only Verus builtins and therefore add no second `vstd` runtime
dependency. The ghost finite-view vocabulary survives only in proof position,
while executable length, indexing, and updates remain native allocation-free
operations.

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
