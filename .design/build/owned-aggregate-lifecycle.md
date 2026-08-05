# Owned aggregate lifecycle and pure-call composition

<!--
tier: 3-component
status: shipped
decision: a typed mutable local of a finite non-sealed record may be updated and returned only when the independent body semantics reconstruct every field exactly; pure value calls compose through independently derived specifications, while the separate mutable-call extension composes exact direct finite-record effects
governs:
  - thermite-tv/src/exec_encode.rs
  - thermite-tv/src/exec_stmt_encode.rs
  - thermite-tv/src/lib.rs
  - thermite-tv/src/obligation.rs
  - thermite-tv/tests/owned_aggregate_lifecycle_tv.rs
  - forge/src/body_tv.rs
  - forge/src/exec_tv.rs
  - forge/tests/body_tv.rs
  - forge/tests/verified_build.rs
  - conformance/verified-build/owned_aggregate_lifecycle.th
audited-content-sha256: 4c6fda546cd9e377318b543a9b94ca4f7e301940d6397f53d912d1106b5800d1 (re-pinned 2026-08-04 after atomic-storage acceptance extended the shared verified-build suite; owned aggregate semantics remain regression-covered)
extends:
  - .design/build/kernel-primitives.md
  - .design/build/named-record-lifecycle.md
  - .design/build/fixed-collections.md
  - .design/verified/exec-tv.md
  - .design/verified/exec-stmt-tv.md
-->

## Purpose and boundary

A later Thermite-authored kernel must be able to express state transitions as
ordinary value transformations: construct a finite record, keep it in a typed
local, replace one field, return the new value, and feed that value through other
verified Thermite functions. This is the allocation-free ownership pattern used
by capability tables, allocator metadata, queues, and service state without
placing any of those policies in this repository.

Thermite already validates and lowers direct field assignment on a typed mutable
local. The remaining gap is strict independent body translation validation. A
body with `let mut next: State = state; next.value = value; next` currently
becomes `Skipped`, as do callers whose independently derived callee specification
returns such an aggregate. This increment closes that proof/build gap. It adds no
static global, kernel state machine, allocator, capability policy, scheduler,
firmware, platform implementation, or boot artifact.

## Frozen source surface

No new syntax is required:

```thermite
struct State {
  generation: u64,
  occupied: bool,
}

fn replace_generation(state: State, next: u64) -> State
  req next > state.generation
  ens result.generation == next
  ens result.occupied == state.occupied
  fx pure
{
  let mut updated: State = state;
  updated.generation = next;
  updated
}

fn open_then_replace(next: u64) -> State
  req next > 0
  ens result.generation == next
  ens result.occupied
  fx pure
{
  let initial: State = State { generation: 0, occupied: true };
  replace_generation(initial, next)
}
```

The admitted local must have an explicit `Type::Named` annotation, and that type
must be an ordinary or defining-module opaque struct in the same recursively
finite non-sealed closure used by direct borrowed-record mutation. This original
increment froze the direct `local.field` case. The shipped extension in
`.design/build/nested-aggregate-lifecycle.md` now admits exact recursive fields
and one terminal fixed-array index. Enum payloads, heap-backed fields,
references, recursive records, sealed records, untyped/inferred mutable record
locals, index-then-field projections, and aliasing remain outside the combined
subset.

A fixed array may therefore be replaced as a whole, updated through a separately
typed local and reinstalled, or updated directly as the final projection of a
typed record root. The hosted proof profile observes the complete array view;
the freestanding scalar-nested fixture remains separate until no-vstd array
views are available.

## Independent state denotation

The body reference context carries the exact ordered field inventory of every
reachable finite record. On

```text
let mut local: Name = initial;
local.changed = rhs;
```

the reference state becomes a nominal reconstruction:

```text
Name {
  changed: rhs[environment],
  untouched_0: initial[environment].untouched_0,
  ...
}
```

Every initializer, receiver, field value, shared borrow, dereference, tuple
projection, call argument, and final result is recursively substituted through
the current environment. A later read therefore observes the most recent
reconstruction. Assignment order remains observable, and an `if` statement
composes the complete record value from the two branch states.

For a named-record result, the obligation does not rely on ambient whole-record
equality. It compares every declared field independently. Native fixed-array
fields compare their complete finite sequence views; other admitted fields use
typed value equality. A dropped write, wrong field, wrong value, reordered
dependent write, collateral change, or missing untouched-field frame must fail a
real Verus obligation.

The reference encoder independently emits record construction, field
projection, immutable references, and dereference. It does not reuse production
lowering. Unsupported shapes return `Unsupported` and therefore become
`Skipped`, never `Faithful` by omission.

## Pure value-call composition

Body TV already derives the executable dependency closure and annotates each
callee with an independently generated `when_used_as_spec` definition. This
increment permits those reference definitions to accept and return admitted
finite records and to contain owned local-record updates. A caller then composes
through the callee's independently derived value specification rather than
inlining production code or trusting the source contract alone.

This increment's original composition is deliberately limited to value effects.
`.design/build/mutable-call-effects.md` now supplies the separate exact frame for
statement-position bodyful calls over pairwise-distinct direct finite-record
roots. Mixed shared/mutable formals, mutable slices/arrays, projected actual
roots, returned-value consumption, bodyless boundary functions, platform
effects, allocation, unresolved calls, recursive effect cycles, and other
non-admitted effects remain fail-closed.

## Strict build, receipt, and execution

A policy-free freestanding conformance fixture constructs a scalar-field record
inside an owned pipeline and exports the pipeline, owned record transitions,
and observers. Fixed-array record fields are exercised in a separate real-Verus
body-equivalence battery because the current `--target kernel` no-vstd profile
literally lacks the array `View` operation needed by their proof statements;
that target split remains incomplete rather than being relabeled. Strict L3
must require faithful contract, executable-expression, body-state, and wrapper
rows for the exported scalar-record path and its reachable definitions. The
artifact receipt binds the Thermite source, exact ordered record layout,
generated Verus/Rust, translation-validation evidence, toolchain identity, and
replay inputs.

A codegen-pinned downstream Rust consumer constructs and transforms the value
only through generated Thermite functions and observes the compiled result.
Changing the Thermite assignment or callee must alter runtime behavior or fail a
proof/build. The runtime check is execution evidence, not a substitute for the
independent obligations.

## Assurance floor

This is an L3 primitive increment. Production semantics, source contracts,
independent contract/expression/body translation-validation obligations, and
the selected strict wrapper must all discharge for all inputs through Verus.
No L2, L1, skipped, assumed, or boundary-only row satisfies acceptance. An L4
route may replace an individual decidable clause only when checked
reconstruction establishes the same claim; L4 is not required for stateful
whole-body equivalence outside an admitted decidable fragment.

More generally, reusable Thermite-authored kernel algorithms and language
semantics have an L3-or-L4 completion floor. The only legitimate sub-L3 items
are bodyless declarations for operations whose exact machine implementation is
absent from this repository or whose hardware/concurrency semantics cannot be
expressed by the current proof backend. Such a declaration is an incomplete
platform obligation, not a completed primitive implementation, and a consumer
may not upgrade it without an exact receipt-bound implementation refinement.

## Acceptance and adversarial evidence

This increment is shipped only when all of the following hold:

1. a typed mutable finite record local with a direct field write and returned
   value is body-TV faithful under real Verus at L3 (or an applicable
   reconstruction-checked L4 route);
2. record results are checked field-by-field, including complete fixed-array
   views and untouched fields;
3. reads after writes and branch-composed record updates observe the exact
   current state;
4. a caller composes through an independently generated pure value-callee
   specification;
5. dropped, wrong-field, wrong-value, reordered, collateral, stale-read, and
   wrong-callee production mutants fail an independent obligation;
6. exact direct finite-record mutable callees compose through the separate
   source-derived effect frame, while wider mutable/alias forms remain rejected;
7. a strict freestanding L3/L4-only build, receipt replay, ABI/source tamper
   checks, and a downstream compiled execution test all pass, with no skipped or
   sub-L3 proof row; and
8. workspace formatting, lint, focused/broad tests, requirement registry,
   documentation drift, canonical-source, and primitive-only scope audits remain
   green.

## Residual work

The exact typed nested-field and terminal fixed-array projection subset is now
shipped by `.design/build/nested-aggregate-lifecycle.md`; exact record-state
loops are supplied by `.design/build/record-state-loops.md`. This increment still
does not claim mutable enum payloads, index-then-field aliasing, mixed
shared/mutable, projected-root, or mutable slice/array call effects, returned
mutable-call values, static global ownership, affine uniqueness, concurrent
record access, atomic object/machine refinement, or Rust/assembly TPL refinement.
It does not add or package a kernel.
