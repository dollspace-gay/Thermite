# Opaque reusable library-state primitive

<!--
tier: 3-component
status: shipped
decision: Thermite provides an opt-in module-local construction barrier for reusable state types; it does not claim affine ownership or ship kernel policy
governs:
  - thermite-syntax/src/ast.rs
  - thermite-syntax/src/parser.rs
  - thermite-syntax/tests/opaque_parse.rs
  - thermite-lower/src/lower.rs
  - thermite-lower/tests/opaque_state.rs
  - forge/src/verified_build.rs
  - forge/tests/ownership_primitives.rs
  - stdlib/kernel-primitives/ownership/generation.th
audited-content-sha256: c1a78a71c0ac40ac86cad05c2caa4e7f5dd72217c937176bcb0c39dcee95ad50 (re-pinned 2026-08-05 after orthogonal registry-v2 separate-crate closure; opaque semantics remain regression-covered)
extends:
  - .design/build/kernel-primitives.md
  - .design/build/generation-ownership.md
  - .design/build/l3-rich-composition.md
thesis-refs:
  - thermite-design.md §3
  - thermite-design.md §5.3
  - thermite-design.md §6
-->

## Decision

`#[opaque] struct State { ... }` is a reusable library-state construction
barrier. Thermite code in the package module that declares `State` may use a
`State { ... }` literal. Code in every other package module must obtain the
value by calling a verified function provided by the declaring module. A
single-file program is one defining module and may construct its own opaque
types.

Representation ownership covers projection and assignment as well as literals.
Foreign Thermite modules may carry an opaque value, call its public closed
specifications, observers, and exclusive transitions, but may not directly read
or write a field.

This is language and proof infrastructure for a later kernel project. It adds
no kernel, firmware image, allocator, scheduler, IPC policy, device policy,
Rust reference implementation, or assembly.

## Three distinct struct surfaces

| Surface | Defining Thermite module may construct | Foreign Thermite module may construct | Intended use |
|---|---:|---:|---|
| plain `struct` | yes | yes, with a declared import | public data |
| `#[opaque] struct` | yes | no | verified library-owned state |
| `#[sealed] struct` | no | no | values minted only by a declared boundary |

`#[opaque]` and `#[sealed]` are mutually exclusive parser attributes. Opaque
state is not a weaker spelling of sealed authority: the library itself needs to
initialize and transition opaque values using ordinary proved Thermite bodies,
whereas sealed values enter only through a platform door.

## Package semantic gate

The package resolver records the declaring module for every opaque struct and
walks the complete expression tree of every package item. It rejects a foreign
module's opaque literal, field read, or field assignment in contracts,
decreases clauses, executable and specification bodies, and nested control
flow. Receiver types are derived from signatures, typed and inferred locals,
constructor returns, state snapshots, structural projections, and branch
results. If a receiver cannot be resolved and its selected field belongs to a
foreign opaque type, the package fails closed rather than leaving privacy to the
backend. Resolved plain records may reuse the same field spelling. The walk
covers the complete receipt-bound package, not only the requested export
closure, so an unreachable sibling item cannot hide a representation access.

Calling a verified constructor imported from the defining module remains
legal. Direct import and root-export rules continue to apply independently.
Proof blocks contain tactic text rather than executable Thermite expressions
and therefore have no struct-literal expression node to inspect.

## Verus and Rust representation

L3 lowering emits an opaque Thermite type as a public Rust/Verus type whose
fields are `pub(crate)`. Generated code in the receipt-bound crate can implement
the verified transitions, but an external safe-Rust crate cannot spell the
struct literal or read its representation. Any consumer that bypasses this
with raw pointers, transmutation, or assembly has introduced a TPL operation
that needs its own exact model and refinement; opacity does not bless such a
bypass.

Verus forbids a `pub open spec fn` from unfolding through a crate-visible
field. The lowerer therefore computes both:

- the transitive named-type closure whose representation reaches an opaque
  struct; and
- the specification-function closure whose signature, construction, or calls
  reach that representation.

Those specification functions lower as `pub closed spec fn`. Their symbols are
usable in public abstract contracts, their bodies remain available to proofs in
the defining generated module, and external clients cannot unfold the private
representation. Unrelated scalar specifications remain `pub open`.

The surface grammar requires a `dec` on every specification function. For a
nonrecursive opaque observer, lowering omits that operationally irrelevant
clause: publishing a measure such as `state.field` would itself expose an
ill-formed private-field expression. A directly recursive opaque specification
retains its decreases clause and must use a measure that is valid on its public
abstract surface.

L1 already emits user structs and fields module-private, so no additional L1
visibility widening is needed. L2 continues to reject algebraic data types
rather than inventing a lower-assurance opacity claim.

## Ownership claim boundary

Opacity controls construction, not value multiplicity. It does not by itself:

- make a type affine or linear;
- reject every move duplication or prove unique aliases;
- add `Copy` or `Clone` restrictions beyond the type's existing fields;
- validate stale handles or rights; or
- refine a platform machine operation.

Libraries still need sealed roots, generation checks, consumption rules, or a
future affine type discipline for those properties. The generation package
combines a sealed authority, non-`Clone` ledger state, stale-generation laws,
and opaque construction; each claim remains separately auditable.

## Receipt and adversarial evidence

The attribute is part of the canonical package source and therefore of the
transitive source closure bound into a strict receipt. Replay rejects evidence
whose `#[opaque]` marker is removed or changed.

Acceptance evidence covers:

- parser distinction among plain, opaque, and sealed structs;
- rejection of attributes on non-struct items;
- package rejection of a foreign opaque literal and acceptance through the
  defining module's constructor;
- package rejection of typed, inferred, chained, and unresolved foreign opaque
  field reads and assignments, without rejecting type-resolved plain fields;
- generated `pub(crate)` fields, transitive closed specifications, and unchanged
  plain-struct/open-spec behavior;
- a real Verus proof of exported opaque construction and observation through a
  public closed specification;
- rejection of a foreign module that forges `GenerationHandle`; and
- strict ownership-package replay followed by a negative attribute-tamper
  replay.

The direct named-record lifecycle fixture additionally exports an opaque
constructor, two observers, and an exclusive transition from one strict receipt.
A codegen-pinned consumer executes the generated transition, while a separate
consumer that selects a field fails Rust compilation because the representation
remains crate-private.

## Auditable metrics

This increment adds one struct attribute and no boundary declaration. The
checked-in ownership package remains entirely Thermite-authored except for its
test/proof harness:

| Metric | Value |
|---|---:|
| New kernel-policy or algorithm LOC | 0 |
| New Rust/assembly primitive implementation LOC | 0 |
| New frozen boundaries | 0 |
| Opaque ownership-package types | 2 |
| External safe-Rust constructible opaque fields | 0 |

These are primitive metrics, not kernel metrics.
