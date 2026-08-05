# Exact named-record borrow and mutation primitives

<!--
tier: 3-component
status: shipped
audited-content-sha256: d26dc5ea904c1ee4926c1d3eb98c20796735f3ee1bab87a7e3cab43c9aa9027b (re-pinned 2026-08-05 after exact body-TV entry-state grounding; named-record semantics remain regression-covered)
decision: direct mutation through an exclusive borrow of finite non-sealed named record state is admitted only when validator, L3, independent contract/exec/body TV, strict ABI, receipt replay, and representation ownership all describe the same field-exact transition
governs:
  - thermite-spec/src/validator.rs
  - thermite-spec/tests/named_record_mutation_validate.rs
  - thermite-lower/src/lower.rs
  - thermite-lower/src/l1.rs
  - thermite-lower/tests/named_record_lifecycle.rs
  - thermite-tv/src/exec_encode.rs
  - thermite-tv/src/exec_stmt_encode.rs
  - thermite-tv/src/obligation.rs
  - thermite-tv/src/ref_encode.rs
  - forge/src/contract_tv.rs
  - forge/src/exec_tv.rs
  - forge/src/body_tv.rs
  - forge/src/verified_build.rs
  - forge/tests/contract_tv_conformance.rs
  - forge/tests/exec_tv_conformance.rs
  - forge/tests/body_tv.rs
  - forge/tests/verified_build.rs
  - conformance/verified-build/named_record_lifecycle.th
extends:
  - .design/build/kernel-primitives.md
  - .design/build/opaque-library-state.md
  - .design/build/generation-ownership.md
  - .design/verified/exec-tv.md
  - .design/verified/exec-stmt-tv.md
-->

## Purpose and boundary

A later Thermite-authored kernel must be able to keep allocator, capability,
queue, synchronization, and service state in named records and update that state
through an exclusive borrow. Requiring every transition to consume and return a
whole record is usable for early libraries but does not provide the in-place,
statically allocated state surface a real freestanding consumer needs.

This increment is reusable language, proof, build, and receipt infrastructure.
It defines no allocator, capability policy, scheduler, IPC protocol, device
state machine, firmware entry point, architecture runtime, or bootable image.

## Frozen source surface

The existing expression and assignment grammar is sufficient:

```thermite
#[opaque] struct State {
  generation: u64,
  occupied: bool,
}

spec fn generation_of(state: State) -> u64
  dec 0
{ state.generation }

spec fn occupied_of(state: State) -> bool
  dec 0
{ state.occupied }

fn advance(state: &mut State, next: u64) -> bool
  req next > generation_of(*old(state))
  ens result == occupied_of(*old(state))
  ens generation_of(*final(state)) == next
  ens occupied_of(*final(state)) == occupied_of(*old(state))
  fx pure
{
  let previous: bool = state.occupied;
  state.generation = next;
  previous
}
```

`state.field` is a direct field read. `state.field = value` is a direct field
write. `old(state).field`/`final(state).field` use implicit snapshot
dereferencing for plain records; `*old(state)`/`*final(state)` explicitly pass
an opaque snapshot to a public closed specification. No new update
operator, implicit heap allocation, interior mutability, or Rust escape hatch is
introduced.

The first frozen mutation closure admits a root parameter of `&mut Name` where
`Name` is a non-sealed struct and every field is a finite plain value composed
from machine scalars, unit, fixed arrays, tuples, and ordinary acyclic records.
An opaque root is admitted in its defining package module because opaque state
exists specifically so that module can implement verified transitions. A sealed
root is rejected: platform-minted authority must be transformed through an
explicit registered operation, not by ambient field writes.

Direct one-level fields are the mutation target established by this increment.
The exact recursive extension in `.design/build/nested-aggregate-lifecycle.md`
now also admits `state.inner.count = value` and one terminal fixed-array index.
Index-then-field projections, enum-variant fields, references, `Box`, `Vec`,
`String`, `Map`, `Option`, `Result`, recursive records, and generic/heap-backed
fields remain outside the admitted closure until each has an exact independent
state and alias model.

## Validation and representation ownership

Field assignment is accepted before code generation only when all of these hold:

1. the target is `root.field`, or belongs to the separately frozen exact nested
   field/terminal-array extension;
2. `root` is a named parameter or typed local whose source type is known;
3. the root is writable because it is an `&mut Name` parameter or a `let mut`
   owned value;
4. `field` belongs to that exact `Name`, rather than merely sharing a spelling
   with a field on some unrelated struct;
5. `Name` belongs to the finite non-sealed mutation closure; and
6. package code that reads or writes the representation of an opaque record is
   in the record's defining module.

Shared borrows, immutable owned parameters, immutable locals, unknown roots,
unknown fields, sealed roots, unmodeled nested/aliased targets, recursive state,
and unsupported field types receive structured validator/package diagnostics. They must not be
left for Rust or Verus type errors. Ordinary bare-cell and indexed slice/array
assignment keep their existing rules.

Opaque representation ownership applies to field reads as well as writes and
struct literals. A foreign Thermite module may mention the opaque type in a
signature and call its public verified constructor, observer, or transition, but
may not inspect or update a field directly. This makes the package promise match
the generated `pub(crate)` representation boundary instead of relying on source
convention inside the generated crate.

## L3 and L1 execution

Production L3 remains the direct Verus/Rust operation
`root.field = lowered_value;`. L1 remains the corresponding safe Rust field
assignment. These are generated language semantics, not TPL primitives and not a
parallel implementation. Verus checks mutability, value typing, arithmetic
safety, record invariants, and every source postcondition on the exact emitted
body.

No `external_body`, `assume`, native callback, generated expected marker, or
runtime policy shim is permitted. Changing the source field, assignment value,
assignment order, pre/post-state selector, or an untouched-field frame must
change an independently checked obligation or make the build fail.

## Independent translation validation

Contract TV adds named-record and named-record-reference frames. Record
definitions are included in the proof preamble. Every source `old(root)` and
`final(root)` becomes a distinct arbitrary shared snapshot reference, preserving
both implicit field projection and explicit dereference while checking arbitrary
transitions rather than a fabricated no-op state. The independent reference
encoder defines field projection and dereference without calling the production
lowerer.

Exec-expression TV admits structural field reads and record construction,
independently emitting `receiver.field` and a field-ordered nominal literal. Its
free-variable walk includes every receiver and initializer, and its frame
contains the reachable record definition. This checks constructors, typed
initializers, and return expressions; a production projection or initializer of
the wrong field must diverge.

Body TV independently threads every direct field as a state cell. The initial
cell is `old(root).field`. A field read observes the cell's value at that source
program point; a direct write replaces exactly that cell. The nested extension
reconstructs the changed direct cell recursively while preserving all nested
siblings. Scalar locals preserve snapshot semantics, and branch composition uses
an exact `if` expression for every changed field. The final obligation contains:

- the exact independently computed result value; and
- one equality for every declared root field between `final(root).field` and
  its modeled final cell.

Consequently an untouched field is explicitly framed to its old value. A
dropped write, wrong field, wrong value, reordered dependent write, collateral
write, stale read, or swapped branch changes at least one obligation. Fixed-array
fields compare their complete finite sequence views. Index-then-field writes,
loops over record state, mutable aliases, and mixtures the state encoder cannot
compose are reported as `Skipped` and are forbidden from a strict verified
export; they never become `Faithful` by omission.

Unit-returning mutators are part of the frozen lifecycle surface. Body TV treats
a tail-less unit block as result `()` while still observing every final field.

## Strict build, ABI, and receipt

A strict public export may accept `&mut Name` only for a record in the admitted
mutation closure. Its ABI entry records exclusive ownership and recursively
binds the ordered field names, types, resolved fixed capacities, opacity/sealing
markers, and codegen/toolchain identity. Reordering, adding, removing, or changing
a field changes the ABI fingerprint.

The build must require faithful contract, executable-expression, body-state, and
wrapper rows for the exported transition. The canonical source closure binds the
Thermite module, package manifest, generated Verus/Rust, proof evidence, ABI
record, and replay inputs. It must continue rejecting `dist`, `target`,
`__pycache__`, symlinks, path escapes, and undeclared artifacts.

A downstream codegen-pinned Rust consumer obtains an opaque value only from the
public generated constructor, passes an exclusive borrow to the public
transition, and observes both fields through public generated observers. A
negative consumer that selects a field must fail compilation. The runtime check
demonstrates that the compiled generated Thermite transition actually changes
the value; it is not formal evidence and does not replace any proof row.

## Acceptance and adversarial evidence

This increment is shipped only when all of the following are true:

1. validator tests accept direct mutation of finite plain and defining-module
   opaque records, while the nested extension accepts exact field chains and a
   terminal array index; shared/immutable, sealed, index-then-field, computed,
   dereferenced, recursive, heap-backed, unknown-root, and wrong-field targets
   fail before lowering;
2. package tests reject foreign opaque field reads and writes across the complete
   receipt-bound module graph;
3. emitted L3 and L1 bodies contain the exact field operation and a representative
   lifecycle verifies with real Verus;
4. contract, exec, and body TV are faithful for field reads, `old`/`final`
   clauses, straight-line writes, dependent reads, untouched fields, branches,
   and unit results;
5. production mutants that drop the write, select another field, alter the value,
   reorder dependent writes, add a collateral write, or swap `old`/`final` fail a
   real independent obligation;
6. a strict package build, receipt replay, source/field/attribute tamper checks,
   ABI-layout sensitivity checks, and downstream compiled consumer all pass; and
7. workspace tests, formatting, lint, requirement-registry, documentation drift,
   canonical-source, and no-concrete-kernel audits remain green.

## Residual work

The follow-on owned-aggregate increment now supplies strict body TV for a typed
mutable local record returned as an aggregate and pure value-call composition.
Nested mutable projections and record-state loops are now supplied by
`.design/build/nested-aggregate-lifecycle.md` and
`.design/build/record-state-loops.md`. Mutable enum payloads, heap-backed record
fields, an affine type system, static global ownership, concurrent record access,
atomic-object machine refinement, mixed shared/mutable, mutable slice/array, or
projected-root call effects, and separate Rust/assembly TPL refinement remain
explicit later primitive increments. Exact statement-position calls over
pairwise-distinct direct finite-record roots are supplied by
`.design/build/mutable-call-effects.md`. It also does not add any kernel policy
or kernel artifact.
