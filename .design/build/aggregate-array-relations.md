# Structural fixed-array relations for plain aggregates

<!--
tier: 3-component
status: shipped
decision: array_eq and array_same_except derive exact structural equality for finite plain record elements without granting equality to sealed or opaque authority
governs:
  - thermite-spec/src/lib.rs
  - thermite-spec/src/validator.rs
  - thermite-spec/tests/fixed_array_validate.rs
  - thermite-lower/src/lower.rs
  - thermite-lower/src/l1.rs
  - thermite-lower/src/lib.rs
  - thermite-lower/tests/fixed_array.rs
  - thermite-lower/tests/aggregate_array_relations.rs
  - thermite-tv/src/exec_encode.rs
  - thermite-tv/src/ref_encode.rs
  - thermite-tv/tests/fixed_array_tv.rs
  - forge/src/exec_tv.rs
  - forge/src/body_tv.rs
  - forge/src/verified_build.rs
  - forge/tests/body_tv.rs
  - forge/tests/contract_tv_conformance.rs
  - forge/tests/exec_tv_conformance.rs
  - forge/tests/verified_build.rs
  - conformance/verified-build/aggregate_array_relations.th
audited-content-sha256: c5b9fa54c34547025374763e5c67f0d0331fa0a98172f3c3e4160ed6650aa34b
extends:
  - .design/build/kernel-primitives.md
  - .design/verified/exec-tv.md
  - .design/verified/exec-stmt-tv.md
  - .design/lower/l1-runtime-checks.md
-->

## Purpose and scope

Kernel data structures routinely contain fixed tables of records rather than
only tables of integers. Thermite's native fixed arrays already provide exact
initialization, indexing, mutation, scalar extensional equality, and the
`array_same_except` frame relation. This increment extends the two relations to
plain, finite record elements so a consuming project can verify descriptor,
slot, queue-entry, and capability-record tables without replacing them with
parallel scalar arrays or writing one equality scan per record type.

This is a reusable language and proof primitive. It does not define a concrete
table, allocator, scheduler, capability policy, IPC format, or kernel.

## Surface and meaning

The existing surface is unchanged:

```thermite
struct Slot {
  generation: u64,
  occupied: bool,
}

fn equal(left: [Slot; 64], right: [Slot; 64]) -> bool
req true
ens result == left.array_eq(right)
fx pure
{
  left.array_eq(right)
}

fn framed(left: [Slot; 64], right: [Slot; 64], changed: usize) -> bool
req true
ens result == left.array_same_except(right, changed)
fx pure
{
  left.array_same_except(right, changed)
}
```

`array_eq` returns true exactly when every element agrees structurally.
`array_same_except` returns true exactly when every in-bounds element other
than the supplied index agrees structurally. An out-of-bounds exception means
full equality, preserving the existing scalar semantics.

The first aggregate derivation admits a finite structural closure built from:

- `u8`, `u16`, `u32`, `u64`, `usize`, `bool`, and unit;
- fixed arrays whose elements are themselves admitted;
- tuples whose components are admitted; and
- ordinary, non-recursive Thermite structs whose fields are admitted.

Nested records, tuples, and fixed arrays are therefore included. Recursive
records, enums, references, `Box`, `Vec`, `String`, `Map`, `Option`, `Result`,
and generic/heap-backed values are rejected by the validator in this
increment. Later increments may add finite enum equality with an equally direct
proof; they must not silently inherit Rust trait behavior.

## Authority and abstraction barrier

`#[sealed]` and `#[opaque]` structs are deliberately excluded. Sealed values are
platform-minted authority, and opacity promises that representation-dependent
operations are introduced explicitly by the declaring module. A compiler-
derived structural comparator would create an ambient observation channel and
would weaken both promises. A library may instead expose an explicitly verified
identity or equivalence operation appropriate to that authority.

This primitive is ordinary equality, not ownership. It does not make a record
`Copy`, affine, linear, unique, or safe to duplicate.

## Validation

Before lowering, the validator computes the least finite set of structurally
comparable struct declarations. A struct enters the set only when it is neither
sealed nor opaque and all of its fields are already comparable. The monotone
fixed point admits declaration-order-independent nesting and rejects recursive
cycles.

Both relation operands must still be named arrays (or direct references/derefs
of them) with exactly the same element type and capacity. The array element
must be in the structural-comparison closure. Invalid arity, scalar receivers,
capacity mismatch, hidden authority, recursive records, and unsupported fields
fail before code generation.

## L3 implementation and proof

Verus does not verify Rust's native array `PartialEq`, so L3 continues to use a
generated allocation-free linear scan. For every comparable aggregate array
shape declared in a program that uses a relation, lowering emits an exact
element comparator with

```text
ensures result <==> *left == *right
```

and then emits the existing const-generic array-relation implementation for
that element. Struct comparators conjoin comparisons of every field. Nested
array fields call the already verified finite-array scan and explicitly bridge
finite-view extensional equality to value equality. Tuples and nested structs
compose their corresponding exact comparators. There is no `external_body`,
assumed lemma, native `PartialEq` proof shortcut, or trusted implementation.

The array scan retains its exact contracts:

- `result <==> self@ =~= right@`; and
- `result <==> forall j, 0 <= j < N && j != except ==> self@[j] == right@[j]`.

The generated helper closure is deterministic and appears only when a program
uses one of the fixed-array relations. Emitting the finite declaration closure,
rather than performing call-graph reachability, keeps validation, L1 trait
derivation, L3 proof support, and TV frames on one source-stable type inventory.

## Executable and independent-validation paths

L1 derives Rust `PartialEq`/`Eq` only for plain structs in that same declared
aggregate-array closure, then uses bounded native/explicit scans. It never
derives those traits for sealed or opaque structs. L2's general ADT harness
remains outside the existing bounded-checking rung and is not upgraded by this
increment; such programs prove at L3 and remain runnable at L1.

Strict public exports admit these same finite plain values by value, and admit
their elements inside borrowed slices/fixed arrays. Each export ABI fingerprint
contains the resolved capacity values and complete transitive record field
layout/order in addition to the pinned compiler and target; changing a record
definition or a named capacity therefore changes the ABI identity directly.
General named-record borrows and borrowed returns remain excluded.

Contract and executable translation validation continue to derive the finite
view meaning independently of the production helper generator. Their frames
must carry the exact required struct declarations and native aggregate-array
types. A production comparator that drops a field, swaps a field, skips an
element, or mishandles the exception index must fail the corresponding real-
Verus obligation.

## Evidence and residual work

Completion evidence for this increment must include:

1. validator accept tests for nested plain records and reject tests for hidden,
   recursive, mismatched, and unsupported record shapes;
2. exact emitted-helper tests and a real Verus proof over a representative
   nested record array;
3. independent exec/contract/body translation-validation proofs plus a
   dropped-field generated-comparator mutant that fails its direct Verus
   contract;
4. strict L3 build, receipt replay, and bound-source tamper rejection for a
   policy-free aggregate fixture, including ABI-layout/capacity fingerprint
   sensitivity and a downstream codegen-pinned Rust consumer that constructs
   the public records and executes the generated comparator; and
5. workspace formatting, lint, requirement-registry, and documentation-drift
   gates.

This increment does not complete static ownership, aggregate mutation through
named borrows, enum equality, full aggregate lifecycle body TV, affine
authority, atomic integration, or machine-operation refinement.
