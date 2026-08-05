# Sealed atomic kernel-authoring primitives

<!--
tier: 3-component
status: partial
decision: Thermite ships a receipt-bound, policy-free atomic declaration and proof-model package; consumer platforms own machine implementations and exact refinements
governs:
  - stdlib/kernel-primitives/atomics.thpkg.json
  - stdlib/kernel-primitives/src/model.th
  - stdlib/kernel-primitives/src/init.th
  - stdlib/kernel-primitives/src/machine.th
  - stdlib/kernel-primitives/src/api.th
  - stdlib/kernel-primitives/src/atomic_storage.th
  - stdlib/kernel-primitives/storage/static_storage.th
  - thermite-spec/src/validator.rs
  - thermite-spec/tests/atomic_ordering_validate.rs
  - forge/src/verified_build/primitive_registry.rs
  - forge/tests/verified_build.rs
audited-content-sha256: bdc2139289828b77b68a34b166ad03b3e7241e19c1845a4f1fbc8fd5694105f5 (re-pinned 2026-08-05 after mixed-borrow strict-build coverage and the no_std vstd-prelude assertion repair in the shared harness; atomic semantics unchanged)
extends:
  - .design/build/kernel-primitives.md
  - .design/build/frozen-primitive-registry.md
  - .design/build/kernel-target.md
thesis-refs:
  - thermite-design.md §5.3
  - thermite-design.md §6
  - thermite-design.md §9
-->

## Decision

Thermite provides a reusable atomic package for later kernels. It defines the
source types, legal operations, contracts, ordering rules, finite proof model,
and frozen boundary names. It does not implement atomics in Rust or assembly,
choose a scheduler or synchronization policy, boot a machine, or claim that a
host proof refines a target instruction.

The package is `stdlib/kernel-primitives/atomics.thpkg.json`. Its `api` and
`atomic_storage` roots bind six modules: the atomic model, typed initialization
policy, static-storage ownership protocol, scalar/tuple machine declarations,
application API, and lifecycle composition. Every consumer therefore receives
the same source identities and transitive closure rather than copying
declarations into a private kernel tree.

## Source surface

The source enum is closed:

```thermite
enum AtomicOrdering {
  Relaxed,
  Acquire,
  Release,
  AcqRel,
  SeqCst,
}
```

There are opaque initialization-slot and sealed cell-handle types for `bool`,
`u32`, `u64`, and `usize`. Ordinary foreign Thermite code cannot construct a
slot or cell. Each cell uses `#[sealed("atomic_*_init")]`: exactly its named,
bodyful L3 initialization function may construct the representation, while the
bare `#[sealed]` form remains boundary-only. An L3 converter consumes a
generation-bound committed `StaticStorageRegion` to produce a typed slot, and
initialization consumes that slot by value to return the corresponding cell.
Duplicate use, a second construction function, invalid factory declarations,
and foreign slot construction are pinned negative tests. The exact protocol is
specified in `.design/build/atomic-storage-initialization.md`.

The 50 application operations are:

- four initialization operations;
- four loads, four stores, and four swaps;
- four strong and four weak compare-exchanges;
- boolean fetch-and/or/xor;
- `u32`, `u64`, and `usize` wrapping fetch-add/sub;
- `u32`, `u64`, and `usize` fetch-and/or/xor/min/max; and
- compiler and hardware fences.

Every application operation is a bodyful Thermite function proved at L3. The
application functions validate ordering, consume or borrow the sealed handle,
call one irreducible machine door, and construct the observation, transition,
write, or fence receipt in Thermite. A separate `machine.th` module contains
exactly 50 bodyless L1 declarations with `fx platform(atomic)` and exact
`thermite::atomic::*` targets. Those doors expose only scalar/tuple ABI values:
initialization returns a handle while echoing authority/slot/generation; cell
operations echo the handle; stores echo the written value; CAS returns its
observed value and success flag; and fences acknowledge completion. Each door
requires the exact legal ordering code. The package intentionally contains no
parallel Rust or assembly implementation.

Operations return explicit observation, transition, write, or fence values.
These values expose the observed/next value, cell identity, success state, and
ordering codes needed by later Thermite algorithms. They are runtime values,
not erased ghost proof objects. A consumer machine adapter must produce values
that satisfy the exact contract of its declaration.

## Ordering legality

The source and validator use the same legality table:

| Operation | Legal ordering |
|---|---|
| load | `Relaxed`, `Acquire`, `SeqCst` |
| store | `Relaxed`, `Release`, `SeqCst` |
| swap/fetch RMW | all five variants |
| compiler/hardware fence | `Acquire`, `Release`, `AcqRel`, `SeqCst` |

Compare-exchange is the exact success/failure relation below:

| Success | Legal failure |
|---|---|
| `Relaxed` | `Relaxed` |
| `Acquire` | `Relaxed`, `Acquire` |
| `Release` | `Relaxed` |
| `AcqRel` | `Relaxed`, `Acquire` |
| `SeqCst` | `Relaxed`, `Acquire`, `SeqCst` |

The validator derives each canonical application name from the machine
declaration's exact boundary target, not from a source-name prefix. Application
calls therefore retain the pre-codegen enum-ordering gate, while the internal
machine door receives the L3-proved `u8` code. This keeps the rule inert for
unrelated user functions. For a recognized application call it rejects, before
lowering:

- a nonliteral or dynamically selected order;
- a path other than an exact `AtomicOrdering::Variant`;
- an alias or function-value reference to an atomic boundary, which would hide
  the checked ordering positions;
- wrong arity that would leave an ordering position unchecked;
- load/store/fence orders outside the table; and
- every compare-exchange pair outside the nine legal pairs.

Literal-only ordering is an intentional fail-closed first surface. A future
proposal may admit a dynamic order only if its finite proof obligation is
carried into lowering and directly bound to the machine operation. Silently
passing a dynamic enum to backend Rust is not that proof.

`atomic_ordering_matrix_probe` packages the five load, five store, five fence,
25 compare-exchange, and five RMW cases into one 45-case executable relation.
The executable functions are proved equal to independently named specifications;
mutating a case changes the proof obligation rather than merely a QEMU marker.

## Finite concurrency model

The proof model has a fixed capacity of 256 events. `AtomicEvent` records:

- event and CPU-local sequence identities;
- cell, operation kind, and ordering code;
- observed and written values;
- reads-from, modification-order, and release-head links; and
- sequential-consistency order.

Named relations cover write/read classification, acquire/release classification,
modification order, reads-from, release-sequence membership, synchronizes-with,
happens-before reachability, release-sequence reachability, and SC precedence.
The recursive reachability functions are fuel-bounded and total. Parent arrays
make the finite relation allocation-free and suitable for later verified
libraries.

This is an algorithm-facing proof vocabulary, not a complete axiomatization of
the C++/Rust memory model and not an operational implementation. In particular,
a consumer still must prove that its exact atomic instruction or intrinsic
produces histories satisfying the declared transition and relation invariants.
No theorem in this package turns an unmodeled machine operation into a refined
boundary.

## Verification and target split

The assurance-floor gate checks the dependency-first six-module projection as
one program. It requires all 202 non-boundary certificate rows to be L3 and
requires exactly 52 boundary rows, all at L1: the 50 atomic machine doors plus
the two static-storage machine doors. It also pairs every `atomic_machine_*`
door with the corresponding bodyful `atomic_*` L3 application operation. The
standalone model has 60 certificate rows and every one must remain non-boundary
L3. This is a whole-source assertion: adding an unproved bodyful helper fails
the gate even if the public receipt probes still pass.

The package currently has three strict receipt proof surfaces:

1. `atomic_ordering_matrix_probe` builds for `--target kernel --level l3`.
   This proves the executable ordering algorithms and their generated export
   wrapper in the generic freestanding, no-vstd configuration.
2. `atomic_history_model_probe` builds for `--target std --level l3`. This
   proves the borrowed-fixed-array happens-before and release-sequence relations
   using the vstd finite-array view available to hosted Verus.
3. `atomic_storage_capacity_probe` builds for `--target kernel --level l3`.
   This proves and executes the typed storage-capacity policy without reaching
   a machine boundary.

All three bundles bind and replay the package manifest, source map, six-module
source closure, generated Verus source, translation-validation inventory,
toolchain, and artifact. The integration test requires assurance L3 for every
reachable receipt member and requires every recorded TV row to be `faithful`.

The split is explicit because the generic kernel target invokes Verus with
`--no-vstd`; its built-in-only environment does not provide the array `View`
model used by the finite-history proof. Hosted L3 history evidence must never be
reported as kernel-target evidence. Closing this split requires a no-vstd array
model/refinement or a kernel-compatible verified finite-history representation,
not relabeling the existing bundle.

The reproducible commands are:

```sh
forge build stdlib/kernel-primitives/atomics.thpkg.json \
  --level l3 --export atomic_ordering_matrix_probe --target kernel \
  --out /tmp/atomic-ordering.verified
forge verify-build /tmp/atomic-ordering.verified --replay

forge build stdlib/kernel-primitives/atomics.thpkg.json \
  --level l3 --export atomic_history_model_probe --target std \
  --out /tmp/atomic-history.verified
forge verify-build /tmp/atomic-history.verified --replay
```

## Registry and machine refinement

Frozen registry v1 proves exact same-crate safe-Rust checked wrappers, and v2
adds exact separately emitted safe sequential Rust source/rlib/object closure.
Those paths are sufficient for sequential pure adapters; neither is a
machine-concurrency proof. Both therefore reject every entry whose concurrency
is `atomic`, `volatile`, or `privileged`, even when its memory-ordering strings
are otherwise well formed.

Registry v3 now admits one canonical machine-aware pilot: a `PAtomicU64`
create-and-SeqCst-load adapter. Its bodyful adapter and Thermite caller prove at
L3 relative to the pinned vstd permission model. The receipt binds the exact
vstd atomic source/full rlib, adapter source/interface/rlib/object, target
features, and replay of both proof layers. It also retains three explicit
machine assumptions and therefore reports the complete artifact as
`L1/to_machine_boundary`, not end-to-end L3.

A broader registry operation may be admitted only when it binds all of:

- exact Rust/assembly and generated source closure;
- exact object bytes, ABI, symbol, features, and target;
- an object or instruction semantics model;
- the operation's transition and memory-ordering relation;
- a direct Verus/refinement proof tied to that exact emitted implementation;
- whole-closure no-cheating evidence; and
- replay that recomputes every binding.

Consumers may import the declarations and prove pure algorithms over the model,
but a selected build that reaches a real sealed atomic boundary cannot claim
full end-to-end assurance merely from the v3 scalar pilot.

## Auditable metrics

The checked-in package currently has:

| Metric | Value |
|---|---:|
| Thermite physical LOC | 3,413 |
| Thermite nonblank LOC | 3,152 |
| Thermite functions | 237 |
| executable functions | 178 |
| specification functions | 59 |
| bodyful executable functions | 126 |
| in-language L3 certificate rows | 202 |
| bodyful L3 atomic application operations | 50 |
| frozen boundary declarations | 52 (50 atomic, 2 static storage) |
| ordinary Rust kernel-policy LOC | 0 |
| bundled Rust/assembly atomic implementations | 0 |
| reachable boundaries in either pure proof bundle | 0 |

These metrics describe the reusable primitive package only. They are not kernel
metrics and do not imply that any machine boundary is implemented. Function and
boundary counts are mechanically recoverable with anchored searches over the
six package modules; LOC is the ordinary line count of those exact
receipt-bound files.

## Remaining work

REQ-KPRIM-5 remains partial. Completion still requires:

- a kernel-target proof surface for the finite history model;
- exact object/model refinement of the persistent scalar/tuple machine ABI
  across the complete consumer-supplied atomic operation and ordering matrix;
- positive composition evidence from the sealed atomic lifecycle through that
  expanded machine-aware registry; and
- verified synchronization libraries that consume these operations without
  introducing Rust policy.

Those gaps are named assurance boundaries. They do not justify adding a kernel
or a Rust reference implementation to this repository.
