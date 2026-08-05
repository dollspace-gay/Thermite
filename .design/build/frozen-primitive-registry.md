# Frozen primitive registry and exact boundary refinement

<!--
tier: 3-component
status: partial
audited-content-sha256: a4ae1941d08ddc45d63acb7a2aa6742894cd7f65d5f90734443e3c5122824ae5 (re-pinned 2026-08-05 after making registry-wide Rust-ABI and borrowed-return diagnostics version-neutral; semantics unchanged)
decision: consumer-owned registry entries close reachable Thermite boundaries through non-exempt same-crate or separately compiled/imported direct-Verus calls
governs:
  - thermite-lower/src/lower.rs
  - thermite-lower/src/lib.rs
  - thermite-lower/tests/l3_library.rs
  - forge/src/cli.rs
  - forge/src/verified_build.rs
  - forge/src/verified_build/composition.rs
  - forge/src/verified_build/primitive_registry.rs
  - forge/tests/verified_composition.rs
  - conformance/verified-composition/frozen_primitive.th
  - conformance/verified-composition/frozen_primitive_shell.rs
  - conformance/verified-composition/frozen_primitive_registry.json
  - conformance/verified-composition/separate_primitive_impl.rs
  - conformance/verified-composition/separate_primitive_shell.rs
  - conformance/verified-composition/separate_primitive_registry.json
  - conformance/verified-composition/machine_atomic.th
  - conformance/verified-composition/machine_atomic_impl.rs
  - conformance/verified-composition/machine_atomic_shell.rs
  - conformance/verified-composition/machine_atomic_registry.json
  - conformance/verified-composition/machine_atomic_consumer.rs
  - stdlib/kernel-primitives/platform.thpkg.json
  - stdlib/kernel-primitives/platform/api.th
  - forge/tests/platform_primitives.rs
extends:
  - .design/build/kernel-primitives.md
  - .design/build/l3-rich-composition.md
  - .design/lower/boundary-composition.md
thesis-refs:
  - thermite-design.md §5.3
  - thermite-design.md §6
  - thermite-design.md §8
  - thermite-design.md §9
-->

## Scope

Forge accepts an optional consumer-owned frozen primitive registry on a strict
L3 composition build:

```text
forge build package.thpkg.json --level l3 \
  --compose-export kernel_entry \
  --compose-shell platform_shell.rs \
  --primitive-registry platform.registry.json \
  --target kernel
```

The registry is a closure and refinement input, not an operation catalog.
Thermite contains no built-in x86, ARM, RISC-V, firmware, scheduler, allocator,
or device profile. A consumer declares only the platform operations its source
contains. Forge resolves those declarations against the source-reachable
boundary closure selected by the link and composition roots.

Thermite now ships 74 architecture-neutral frozen declarations in
`platform.thpkg.json`, plus the separate 50-operation atomic surface and three
wait operations. Those declarations define reusable semantic contracts; they
are not registry entries and contain no implementation. A consumer registry
still closes only its reachable subset, with consumer-owned target and object
evidence.

Registry v1 supports exact same-crate Rust-ABI functions authored in a
direct-Verus shell with `sequential` concurrency semantics. Registry v2 adds a
`separate_verus_crate` linkage for the safe sequential subset. Forge compiles
that authored source in its own strict Verus crate, exports the checked proof
interface, links the exact emitted rlib into the generated caller, and records
the hashes of every emitted object member. Canonical non-empty target-feature
sets are bound into both proof/codegen invocations and their replay.

Registry v3 adds the first machine-aware vertical slice: one canonical
`PAtomicU64` create-and-SeqCst-load adapter. The adapter itself is an ordinary
bodyful direct-Verus function proved at L3, while the exact pinned vstd atomic
source, full codegen rlib, adapter source/interface/rlib, and every emitted
adapter object are receipt-bound and replayed. Verus marks its own atomic
implementation `external_body`, so the hardware crossing remains an explicit
L1 residual assumption. A v3 bundle is consequently reported as
`L1/to_machine_boundary`; it is never laundered into an end-to-end L3 artifact.

Foreign ABIs, consumer unsafe Rust, assembly, volatile access, privileged
instructions, persistent sealed-cell ABIs, and general concurrent execution
remain unsupported. Separate-crate byte closure cannot be repurposed to claim
those classes. They continue to fail closed until their exact implementation
and honest residual/refinement evidence are represented by a later v3
operation or successor schema.

## Schema v1

The top-level schema is `thermite.frozen-primitive-registry.v1` and contains an
exact target triple, a strictly sorted unique list of canonical bare target
features, and entries strictly sorted by semantic name. Unknown JSON fields are
errors. Forge renders a non-empty set exactly once as the canonical
`-C target-feature=+f1,+f2` argument; the argument is part of the artifact plan,
whole-crate result, receipt root, validation, and replay. Registry values cannot
carry their own `+`/`-` prefixes or inject an additional compiler option. Every
requested name must also occur in the pinned codegen rustc's
`--print target-features` inventory. That sorted inventory is itself recorded
in `CodegenRustcEvidence`, included in the path-independent toolchain identity,
validated from the receipt, and reconstructed during replay; an unknown feature
cannot degrade to a compiler warning and silently disappear.

Each entry declares:

- a canonical versioned semantic name and whether it must be reachable;
- the exact Thermite function, `#[boundary]` target, normalized signature,
  contract digest, effect digest, and effect row;
- parameter and result ownership (`by_value`, shared/exclusive borrow,
  sealed consume, or sealed mint) derived independently from the AST;
- the shell module, checked function item, exact shell-source digest, Rust ABI,
  same-crate symbol, and power-of-two alignment;
- `thermite_contract` as the v1 model and
  `same_crate_verus_checked_wrapper` as the refinement mechanism;
- the mandatory `contract_refinement`, `exact_implementation_call`, and
  `whole_crate_no_cheating` obligations;
- the `sequential` concurrency class, an empty memory-ordering list, and failure
  behavior. The parser recognizes the future `atomic`, `volatile`, and
  `privileged` vocabulary only to return a precise unsupported-assurance error;
  registry v1 never accepts those classes into a plan.

The registry cannot weaken or replace source facts. Forge independently derives
the function signature, contract/effects digests, effects, and ownership, then
rejects any mismatch. It independently inventories the shell and rejects a
missing, private, declaration-only, digest-drifted, duplicate, or non-function
implementation.

## Schema v2: separate safe-Rust crate closure

`thermite.frozen-primitive-registry.v2` retains every v1 declaration and makes
`implementation.linkage` mandatory. `same_crate` has the v1 meaning.
`separate_verus_crate` selects a whole source module as a separate crate; a
module cannot mix linkage modes. Its exact symbol remains
`crate_name::checked_item`, so the generated Thermite wrapper calls the imported
crate directly rather than a parallel same-crate implementation.

For each separate crate, the frozen plan binds:

- the authored module path, length, digest, public-item inventory, and generated
  complete crate-source digest;
- the exact target and feature set;
- mandatory `contract_refinement`, `exact_implementation_call`,
  `exported_verus_interface`, `imported_call_refinement`,
  `separate_source_identity`, `separate_object_identity`, and
  `whole_crate_no_cheating` obligations.

Forge first runs `verus --no-cheating --compile --export` on the generated
crate. The kernel form is `#![no_std]` and uses only Verus builtins, so the
emitted implementation has no hosted or `vstd` runtime dependency. Forge then
imports the emitted `.vir` interface and passes the exact emitted rlib as the
Rust dependency of the canonical Thermite caller proof/codegen. The caller
source contains `extern crate <name>` and its generated wrapper calls
`<name>::<item>`.

The receipt independently binds the authored and generated sources, separate
Verus result, exported interface, rlib, and every `.o` archive member's name,
length, and SHA-256. Forge parses the rlib archive itself; an external `ar` tool
does not define the inventory. Validation rejects a changed source, interface,
rlib, archive member, proof result, plan, or receipt.

Verus's serialized `.vir` includes nondeterministic internal metadata, so replay
does not falsely require byte reproduction of a fresh export. It re-verifies the
same generated source, requires byte-identical rlib and object members, imports
the freshly checked interface into the caller, and requires the final caller
rlib to reproduce exactly. The original interface bytes remain individually
bound and tamper-evident in the receipt.

## Schema v3: machine-aware atomic pilot

`thermite.frozen-primitive-registry.v3` retains the safe v1/v2 linkages and adds
`separate_verus_machine_crate`. The initial admitted operation is deliberately
narrow and canonical: `p_atomic_u64_seq_cst_roundtrip` accepts one `u64`, creates
a `vstd::atomic::PAtomicU64`, and returns its SeqCst load under the matching
tracked permission. Its authored adapter bytes must match the canonical checked
body exactly. Returning the input directly, moving the atomic call into dead
code, changing the type or ordering, or substituting a digest-updated safe model
is rejected during planning.

The entry must declare:

- model `pinned_vstd_atomic_permission`, refinement
  `separate_crate_verus_machine_import`, concurrency `atomic`, and exactly the
  `seq_cst` ordering;
- ten sorted discharged obligations covering the Thermite contract, exact
  adapter call, permission and ordering refinement, exported/imported Verus
  interface, source/object identity, pinned model, and both no-cheating layers;
- the three sorted residual assumptions `hardware_atomic_semantics`,
  `pinned_vstd_external_body`, and `rust_core_atomic_codegen`.

Forge compiles the adapter against the full pinned no-std vstd rlib rather than
the proof-only slice/array link shim. The final Thermite caller uses that same
crate identity, preventing a different metadata crate from satisfying the
machine adapter dependency. The receipt carries the full vstd rlib and the
exact `vstd/atomic.rs` source in addition to the aggregate vstd source-tree and
`vstd.vir` identities already present in toolchain evidence. Replay reconstructs
both Verus invocations and requires the same adapter rlib and object members.

This is a proof of the bodyful adapter relative to the pinned Verus atomic
permission model, plus exact evidence for what machine code was emitted. It is
not a proof that Verus's `external_body`, Rust/LLVM atomic codegen, or the target
hardware memory model is correct. Those three facts are counted separately as
residual assumptions, not discharged obligations. The boundary member remains
`L1-residual-machine-assumption`; a distinct checked-wrapper member reports
`L3-relative-to-pinned-machine-model`, and ordinary Thermite callers retain
their own L3 certificate.

## Reachability closure

Forge computes the union closure of every selected link and composition root.
Every reachable `#[boundary]` needs exactly one registry entry. An entry marked
`required` must be reachable. Unknown Thermite functions, non-boundary
functions, duplicate semantic names, duplicate Thermite functions, duplicate
boundary targets, and duplicate implementation functions are errors.

Optional unreachable entries may describe boundary declarations present in the
same canonical package, but they are not lowered, counted, or claimed as
discharged. A supplied registry with no reachable boundary is rejected so an
irrelevant registry cannot create an assurance label.

## Exact checked-wrapper refinement

Ordinary `forge check` and unregistered composition preserve the established
`external_body` boundary semantics and `to_boundary` scope. Registry composition
uses a different, stricter lowering for each resolved boundary:

```text
Thermite caller
  -> generated boundary function with the exact Thermite req/ens signature
  -> shell_module::implementation(original parameters)
```

The generated boundary function has a real body. It carries no
`external_body`, `assume`, `admit`, `unsafe`, or `unimplemented` exemption. For
`same_crate`, the exact shell bytes are inserted into the same canonical
`verus!` crate, and one `verus --no-cheating --compile` invocation must prove
the wrapper call and compile those same bytes. For `separate_verus_crate`, the
first strict invocation verifies and emits the dependency, and the second
strict invocation imports that checked interface and proves the generated
wrapper's exact cross-crate call. Registry v3 uses the second shape but compiles
the canonical adapter against its pinned machine model. A body returning the
wrong value therefore fails before publication in every mode.

The ordinary per-function certificate for the source boundary remains an honest
L1 boundary declaration. The composition assurance aggregate upgrades a safe
member to `L3-direct-refinement` only after registry closure and the
whole-crate checked-wrapper proof succeed. Callers whose local certificates are
`L3/to_boundary` become end-to-end members of the composed artifact because the
named crossing is closed by that exact refinement. Unregistered crossings never
receive this completion.

A machine member is intentionally different. Its checked adapter is L3 relative
to the pinned model, but the bodyless Thermite boundary remains L1 and caps the
complete artifact at `L1/to_machine_boundary`. This split is encoded in the
receipt aggregate rather than left to prose.

Executable/body translation validation may initially report that a dependency
has no in-language body. Forge completes only that exact refusal when the named
dependency is in the validated registry. It replaces the coarse skipped row by
the expected statement rows and records that the canonical boundary call is
covered by the bound wrapper and no-cheating whole-crate proof. Other skipped,
unsupported, divergent, missing, or unverifiable rows remain fatal.

## Receipt and replay

The composition plan records the complete resolved registry, including which
entries are reachable. The bundle stores the exact input at
`evidence/frozen-primitive-registry.json`. The ordinary bound-file inventory,
artifact plan digest, and receipt root cover those bytes. The composition
binding additionally records the registry digest, reachable primitive count,
discharged refinement-obligation count, and residual-machine-assumption count.
For a machine composition the exact pinned atomic model source and full vstd
codegen rlib are ordinary bound files and cannot be replaced independently of
the receipt.

Validation re-parses the strict schema, re-resolves the source closure, rechecks
every declaration/shell/digest/ownership fact, regenerates the bound wrappers
and separate crate sources, and requires the reconstructed plan and combined
source to be identical. Replay re-runs both proof/codegen layers and reproduces
the separate rlib/object set and final rlib digest. Registry byte tampering,
semantic re-authoring, post-plan mutation, shell drift, dependency artifact
drift, or receipt drift rejects.

## Acceptance

- The synthetic identity primitive builds as a freestanding composition,
  validates, replays, and reports one `L3-direct-refinement` boundary.
- The combined source calls the exact inventoried shell implementation and
  contains no `external_body` or synthetic unimplemented body.
- Signature, contract, effect, target, ABI, ownership, shell-source, proof-list,
  and schema drift fail closed.
- A non-empty canonical target-feature set is present in the plan and in the
  exact whole-crate proof/codegen arguments; validation and replay reconstruct
  the same argument, while duplicate, unsorted, malformed, or post-plan feature
  changes reject. A syntactically valid feature absent from the pinned rustc
  inventory rejects before proof/codegen.
- Registry tampering and a registry change after plan freeze publish nothing.
- A digest-updated shell whose body violates the Thermite contract reaches the
  real whole-crate proof, fails there, and publishes nothing.
- Otherwise well-formed machine classes reject through the safe v1/v2 linkages;
  v3 admits only the exact canonical atomic roundtrip operation.
- A registry-v2 synthetic primitive verifies and compiles as a separate
  freestanding crate, is called by the generated Thermite wrapper, links and
  executes from a downstream consumer, replays both proof layers, inventories
  at least one exact object member, and rejects source/interface/rlib tampering.
- The v3 atomic pilot proves its Thermite caller and checked adapter, executes
  from a downstream consumer, replays both proof layers, binds the exact pinned
  atomic model source/full vstd rlib/adapter object, rejects all three forms of
  tampering, and reports ten discharged obligations plus three visible residual
  assumptions under an L1 machine cap.
- Builds without a registry retain the previous strict policy and continue to
  reject reachable boundaries.

## Remaining work

Registry v1 plus v2 cover same-crate and separately emitted safe sequential
Rust with exact source, proof-interface, rlib, and object identity. Registry v3
adds one honest atomic object/model vertical slice without upgrading its three
literal machine assumptions. The sealed
atomic declaration and finite proof-model package exists, and the generic
platform package now supplies all requested non-atomic declaration families,
but the real sealed atomic ABI is not yet wired through v3. Completion still
requires persistent shared ABI types, the remaining atomic operation/order
matrix, assembly and unsafe/irreducible Rust source/object closure, volatile and
privileged models, and concurrent/liveness composition. None of those broader
claims are made by the pilot.
