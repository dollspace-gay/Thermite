# Exact-source rich-state L3 composition builds

<!--
tier: 3-component
status: shipped
audited-content-sha256: 1be93c92981d8a8101e5c03c5cc98f93d7fbb23c4b32925d6d36aa3406fe7994
decision: one canonical Verus crate with crate-visible rich Thermite roots and public shell exports
issue: github:dollspace-gay/Thermite#104
governs:
  - forge/src/verified_build.rs
  - forge/src/verified_build/composition.rs
  - forge/src/cli.rs
  - thermite-lower/src/lower.rs
  - thermite-lower/src/lib.rs
thesis-refs:
  - thermite-design.md §3
  - thermite-design.md §5.3
  - thermite-design.md §6
  - thermite-design.md §9
-->

## Summary

Forge can build one L3 library from canonical Thermite lowering and authored
direct-Verus shell modules without exposing rich Rust types as a cross-crate
ABI. The selected Thermite functions may accept and return ADTs, references,
tuples, and bounded collections. They lower as `pub(crate)` and are callable by
the shell only because both sources are verified and compiled in the same
crate. Only explicitly selected link exports and public shell items cross the
crate boundary.

The combined source is not a second lowering. Forge takes the unchanged,
target-specific result of `lower_l3_library`, removes only its final `verus!`
delimiter, inserts each exact shell source in a deterministic module frame, and
restores that delimiter. One `verus --no-cheating --compile` invocation proves
and compiles those exact bytes. A distinct
`VerifiedCompositionReceiptV1`-schema receipt binds the Thermite input,
canonical lowering, exact shell files, complete item/type inventory, combined
source, proof evidence, artifact-codegen closure, and output rlib.

## User surface

The additive command is:

```text
forge build model.th --level l3 \
  --compose-export transition \
  --compose-shell platform_shell.rs \
  [--export primitive_link_export] \
  [--crate-name model_platform] \
  [--target std|kernel] \
  [--out dist/model.verified]
```

`--compose-export` and `--compose-shell` are repeatable and must occur together.
At least one composition export is required for this mode. `--export` retains
the issue-#101 primitive/unit public ABI and may be combined with composition
exports, but the same Thermite function cannot occupy both tiers. L1 behavior
and ordinary L3 build behavior are unchanged.

`forge verify-build <bundle> [--replay]` recognizes the receipt schema and
independently reconstructs the combined plan. Replay requires the recorded
Verus and artifact-codegen identities and reproduces the rlib digest.

## Visibility and closure

There are two distinct roots:

| Root kind | Lowered visibility | Admitted signature | Consumer |
|---|---|---|---|
| link export | `pub` or total public wrapper | primitive scalars/unit | another crate |
| composition export | `pub(crate)` | full checked Thermite type language | direct-Verus shell in the same crate |

Every root participates in one union closure. Forge resolves and binds every
reachable executable function, specification function, ADT, generated bounded
type/helper, wrapper, caller edge, body, contract, and effect row. Slag,
boundaries, holes, unresolved calls, divergence, panic, hosted effects in a
kernel target, or any sub-L3 certificate reject the whole build.

For each composition export the plan records its semantic address, exact
signature, parameter ownership (`by_value`, `shared_borrow`, or
`exclusive_borrow`), return type, recursively closed type definitions, and
`crate` visibility. The composition inventory separately records Thermite,
generated, shell-module, and shell-item origins and visibility.

## Direct-Verus source policy

A shell file is a closed authored module body, not a crate or include tree.
Forge hashes its exact UTF-8 bytes and inventories each top-level function,
struct, enum, type, const, static, or trait. Module names and evidence paths are
canonicalized and sorted.

The shell policy rejects attributes, nested `verus!` invocations, nested or
external modules, declarations without checked bodies, `external_body`,
`assume`, `admit`, axioms, unsafe code, unchecked decreases, include macros,
unimplemented/todo markers, erasure bypasses, and macro definitions. Comments
and literals do not create false escape-hatch matches. This policy is checked
when planning and checked again from the bound files during receipt validation.

## Exact-source construction

The assembly algorithm is deterministic:

1. Parse, spec-check, effect-check, close, and canonically lower the frozen
   Thermite input for the selected target and visibility map.
2. Require the lowering to contain exactly one outer `verus!` block and no
   escape hatch.
3. Strip its final `}\n` only.
4. Append each shell as `pub mod <canonical_name> { use super::*; <exact bytes> }`.
5. Restore the final `}\n`, hash the combined source, and freeze the plan.
6. Re-read every input and repeat the complete assembly. Any byte difference
   rejects before proof.
7. Prove and compile the exact combined source once with
   `--no-cheating --compile`; source digests before and after Verus must equal
   the plan.

There is no shell-only proof, second Rust reconstruction, post-proof source
rewrite, or unbound artifact compilation.

## Translation validation and proof completion

Ordinary contract, executable-expression, body, loop, and wrapper-guard TV
rows remain mandatory over the complete union closure. A divergent,
unverifiable, skipped, missing, duplicate, or injected non-pass row prevents
publication.

The scalar contract/exec/body TV obligation frames intentionally cannot spell a
rich ADT or tuple signature. For exactly those signature/frame refusals—and no
other refusal—the composition path completes the row from the conjunction of:

- the normalized Thermite AST and complete closure bound by the plan;
- the canonical target lowering bound byte-for-byte;
- the L3 function certificate for that same lowering; and
- the single whole-crate no-cheating proof and compilation.

The evidence row identifies this rich-state completion explicitly. It does not
convert an actual counterexample, timeout, unsupported body construct, missing
row, or non-rich skip into success. Fault-injected non-pass rows still reject.

## Kernel bounded-state representation

The kernel profile retains `#![no_std]`, `--no-vstd`, and a separate no-std
final-link check. Because the pinned vstd rlib depends on `std`, it cannot be
used to justify a kernel artifact. A kernel composition may transport a bounded
`Vec<T>` through rich state and reason about its bounded length using an
allocation-free length representation. Element-observing or mutating methods
are deliberately absent in that target and therefore fail whole-crate
verification. The hosted target retains the full vstd-backed collection
lowering. This is an observable-subset refinement, not an unchecked allocator
shim.

The acceptance probe uses `ProbeState { owner, generation, payload: Vec<u64> }`,
a typed `ProbeEvent`, and a `(ProbeState, ProbeAction)` result. Its shell proves
the transition precondition, model/platform representation, action
authorization, and next-state simulation, then exports only a primitive
`boot_observation`. A host consumer observes the result, a separate `no_std`
consumer final-links the kernel rlib, and an external rich-function consumer is
rejected as private.

## Receipt, validation, and publication

The bundle uses:

- plan schema `thermite.combined-artifact-plan.v1`;
- receipt schema `thermite.verified-composition-receipt.v1`;
- `evidence/lowered-thermite.verus.rs` for the canonical lowering;
- `evidence/direct-verus/*.rs` for exact authored shell bytes; and
- `evidence/source.verus.rs` for the exact combined compiler input.

The receipt composition binding commits to the lowering digest, canonical
shell-set digest, canonical inventory digest, and combined-source digest. The
ordinary source, parsed program, TV/certificate/Verus/toolchain evidence,
artifact and complete file inventory remain bound by the issue-#101 receipt
root. Validation rejects schema mixing, path traversal, missing/extra files,
semantic-plan drift, source-policy drift, visibility drift, toolchain drift,
and artifact drift.

Publication reuses the staging/fsync/self-validation/atomic-rename protocol.
No destination appears on any planning, policy, certificate, TV, proof,
codegen, binding, validation, or injected-fault failure.

## Requirements

<!-- generated:reqs view=forge-l3-rich-composition-status -->
Source: `.design/reqs/registry.toml`

| ID | Status | Owner | Title | Follow-up |
|---|---|---|---|---|
| REQ-L3COMPOSE-1 | shipped | `.design/build/l3-rich-composition.md` | Explicit rich-state composition surface |  |
| REQ-L3COMPOSE-10 | shipped | `.design/build/l3-rich-composition.md` | Authoritative artifact-codegen binding |  |
| REQ-L3COMPOSE-2 | shipped | `.design/build/l3-rich-composition.md` | Separated link and composition visibility |  |
| REQ-L3COMPOSE-3 | shipped | `.design/build/l3-rich-composition.md` | Complete rich closure and type inventory |  |
| REQ-L3COMPOSE-4 | shipped | `.design/build/l3-rich-composition.md` | Fail-closed direct-Verus policy |  |
| REQ-L3COMPOSE-5 | shipped | `.design/build/l3-rich-composition.md` | Single exact combined Verus source |  |
| REQ-L3COMPOSE-6 | shipped | `.design/build/l3-rich-composition.md` | Strict rich proof and TV completion |  |
| REQ-L3COMPOSE-7 | shipped | `.design/build/l3-rich-composition.md` | Versioned independently validated composition receipt |  |
| REQ-L3COMPOSE-8 | shipped | `.design/build/l3-rich-composition.md` | Atomic composition publication |  |
| REQ-L3COMPOSE-9 | shipped | `.design/build/l3-rich-composition.md` | Freestanding rich-state kernel observation |  |
<!-- /generated:reqs -->

## Acceptance and adversarial matrix

- The ProbeState composition builds, validates, replays, and reproduces.
- The exact combined source contains one `verus!`, a `pub(crate)` rich root,
  and the exact planned shell bytes.
- A declared consumer observes the public shell result; a no-std consumer
  final-links; a consumer naming the rich root fails privacy checking.
- Every receipt digest, inventory, source, shell, toolchain, and artifact is
  tamper-evident.
- Direct-Verus cheating tokens and declaration-only functions are rejected.
- Post-plan Thermite/lowering/shell/combined-source mutation is rejected.
- Missing, L0/L1/L2, timeout, counterexample, rejected, or failed-obligation
  certificates reject.
- Divergent, skipped, or unverifiable TV evidence rejects.
- Slag, boundary, hole, ignored executable, unsafe, axiom, external body, or
  unsupported kernel collection operation rejects without publication.
