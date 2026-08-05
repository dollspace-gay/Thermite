# Frozen primitive registry and exact boundary refinement

<!--
tier: 3-component
status: partial
audited-content-sha256: 1fc7305aed00fb09e0f760b2e37844d0b4379ffcca08ebdf26763bd28c1998b3 (re-pinned 2026-08-05 after exact target-feature inventory, proof/codegen, validation, replay, and lint-clean command binding)
decision: consumer-owned registry entries close reachable Thermite boundaries through non-exempt same-crate direct-Verus calls
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

Registry v1 deliberately supports exact same-crate Rust-ABI functions authored
in a direct-Verus shell with `sequential` concurrency semantics. Canonical
non-empty target-feature sets are bound into the frozen plan and supplied to
the exact Verus/rustc proof-codegen and replay invocations. Foreign ABIs,
separate objects, unsafe Rust, assembly, atomics, volatile access, and
privileged instructions remain unsupported. They must fail closed rather than
being mislabeled as directly refined. A later schema must bind their exact
separate-source/object identities and direct-Verus object or machine model
before those implementations can receive this assurance label.

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
`external_body`, `assume`, `admit`, `unsafe`, or `unimplemented` exemption. The
exact shell bytes are inserted into the same canonical `verus!` crate, and one
`verus --no-cheating --compile` invocation must prove the wrapper call satisfies
the Thermite contract and compile those same bytes. A shell body returning the
wrong value therefore fails whole-crate verification and publishes nothing.

The ordinary per-function certificate for the source boundary remains an honest
L1 boundary declaration. The composition assurance aggregate upgrades that
specific member to `L3-direct-refinement` only after registry closure and the
whole-crate checked-wrapper proof succeed. Callers whose local certificates are
`L3/to_boundary` become end-to-end members of the composed artifact because the
named crossing is closed by that exact refinement. Unregistered crossings never
receive this completion.

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
and discharged refinement-obligation count.

Validation re-parses the strict schema, re-resolves the source closure, rechecks
every declaration/shell/digest/ownership fact, regenerates the bound wrappers,
and requires the reconstructed plan and combined source to be identical. Replay
re-runs the exact proof/codegen and reproduces the rlib digest. Registry byte
tampering, semantic re-authoring, post-plan mutation, shell drift, or receipt
drift rejects.

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
- Otherwise well-formed `atomic`, `volatile`, and `privileged` entries reject
  because this proof path has no object/machine semantics.
- Builds without a registry retain the previous strict policy and continue to
  reject reachable boundaries.

## Remaining work

Registry v1 is the exact safe/direct-Verus substrate needed by later platform
declaration work. The sealed atomic declaration and finite proof-model package
now exists, but it deliberately cannot use this schema to claim machine
refinement. Completion of the larger primitive goal still requires affine
sealed ownership, separate object/source closure for irreducible Rust/assembly
machine operations, an atomic object/machine refinement model,
waiting/liveness primitives, and verified
synchronization libraries. None of those are claimed here.
