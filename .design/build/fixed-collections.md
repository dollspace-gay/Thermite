# Allocation-free fixed collections

<!--
tier: 3-component
status: partial
decision: Thermite ships policy-free packed bitmap, vector, FIFO-ring, and collision-explicit direct-map mechanics in .th; generic capacities, richer maps, slabs, and complete aggregate receipt TV remain
governs:
  - stdlib/kernel-primitives/collections.thpkg.json
  - stdlib/kernel-primitives/collections/bitmap.th
  - stdlib/kernel-primitives/collections/direct_map.th
  - stdlib/kernel-primitives/collections/ring.th
  - stdlib/kernel-primitives/collections/vector.th
  - forge/tests/fixed_collections.rs
audited-content-sha256: 9f3a5e88e97f0bb58fb4c53ed894d07e16fcced7ddc44015aa5ed6f45d47e44c
extends:
  - .design/build/kernel-primitives.md
  - .design/build/l3-verified-artifact.md
thesis-refs:
  - thermite-design.md §3
  - thermite-design.md §5.3
  - thermite-design.md §6
  - thermite-design.md §9
-->

## Decision

Thermite provides allocation-free collection mechanics that a separate kernel
can reuse. It does not decide what a bit or queue entry means, which subsystem
owns a collection, how producers are scheduled, or whether an operation should
block. Those are consumer policies.

`stdlib/kernel-primitives/collections.thpkg.json` is a canonical collection
package. It binds its four manifest-declared Thermite modules without a
Rust runtime implementation, platform boundary, heap dependency, or hosted
effect. The modules use native fixed arrays and ordinary verified Thermite
control flow.

## Fixed bitset

`FixedBitmap256` is a packed 256-bit bitmap backed by `[u64; 4]`, with:

- an allocation-free empty constructor;
- capacity and representation-validity queries;
- bounded membership lookup; and
- owned insert, remove, and set-to transitions.

Every transition preserves the fixed capacity, pins the requested bit's final
value, specifies the target word's exact update, and proves that every other
word is unchanged. Bits 0–63 occupy word 0, 64–127 word 1, and so on; a boundary
probe composes inserts at bits 63 and 64 and observes both words.

The language surface now provides total `u64.bit_test(index)`,
`u64.bit_set(index)`, and `u64.bit_clear(index)` methods, plus
`u64.bit_set_preserves_other(changed, observed)` and
`u64.bit_clear_preserves_other(changed, observed)` composition witnesses. For
an index at least
64, observation is false and updates return the original word. L3 lowering
emits a finite 64-mask helper only when these methods are reachable. Each
constant-mask update is discharged directly with Verus bit-vector reasoning,
then exported through an ordinary L3 postcondition that collection callers can
compose. There is no trusted mask axiom and no runtime helper boundary.

Contract, expression, and body translation validation derive an independent
64-mask reference table rather than importing the production generator. The
body reference state machine also substitutes bit-method receivers and
arguments through local bindings, closing the multi-statement validation gap
found by this increment.

For in-range distinct indices, the preservation methods directly prove that
setting or clearing `changed` leaves membership at `observed` unchanged. Their
generated implementations bridge the finite mask table to a dynamic-shift
bit-vector proof, and independent contract, expression, and body encoders derive
the equality separately. This closes the former same-word framing residual
without weakening the exact word-update contracts.

Popcount, first-set search, range scans, bulk union/intersection, and a fully
quantified all-indices collection contract remain future operations.

## Fixed FIFO ring

`FixedRing64` stores 64 `u64` entries plus a head and live length. Its
well-formedness condition is `head < 64 && len <= 64`. Tail and successor
indices use bounded modulo arithmetic, so wraparound is explicit and cannot
overflow the storage bound.

Push and pop return closed result enums:

- push either returns `Pushed64 { ring }` or `RingFull64 { ring, value }`;
- pop either returns `Popped64 { ring, value }` or `RingEmpty64 { ring }`;
- successful push writes exactly at the logical tail and increments length;
- successful pop returns the head value, clears that slot, advances the head,
  and decrements length; and
- rejected operations return the owned ring and, for push, the unconsumed
  value.

Contracts expose the preservation facts needed to compose two pushes and two
pops into a FIFO proof. Source probes prove first-in/first-out behavior,
wraparound from slot 63 to slot 0, and full-capacity rejection. The module is
single-threaded mechanics; synchronization and memory ordering are layered on
top rather than hidden inside the queue.

## Fixed vector

`FixedVec64` owns 64 `u64` slots and a live length. It provides bounded length,
empty/full, and random-access queries; an in-range replacement transition; and
owned push/pop transitions with explicit full and empty result variants.
Successful pop is LIFO, clears the released slot, and returns both the shortened
vector and removed value. Rejected push returns the vector and unconsumed value.

The contracts expose the inserted/replaced slot and length transitions needed
for consumer proofs. Source probes compose two pushes with random reads, compose
two pops into a LIFO proof, and prove replacement at index zero. Element type
and capacity are deliberately concrete library choices; language-level fixed
arrays remain capacity-generic.

## Collision-explicit direct map

`FixedDirectMap64` owns 64 occupancy bits, `usize` keys, `u64` values, and a live
count. A key selects `key % 64`. Lookup, insert, replacement, and removal are
allocation-free. Every operation reports a colliding stored key explicitly;
the library does not silently discard it or smuggle in an unproved probing,
eviction, or hashing policy. Insert and remove also expose count-invalid results
because ordinary named structs are not yet opaque and a consumer can presently
construct representation-inconsistent values.

Source probes prove insert-then-lookup, replacement with the prior value,
collision preservation for keys 0 and 64, and remove-then-vacancy. This is a
useful deterministic slot-map mechanic, not a claim of general collision
resolution. A later consumer can select a collision policy in Thermite, while a
future library can add open addressing once quantified aggregate framing and
opaque representations are available.

## Assurance and adversarial evidence

`forge check --level l3` proves all 76 source items across the four modules at
L3. There are no boundaries. Executable contract mutation kills 196 of 218
generated mutants; the surviving mutants remain counted and the per-function
scores stay above the configured floor.

`forge/tests/fixed_collections.rs` additionally:

- requires every source row to be L3 and boundary-free;
- pins the bitmap score at 33/40, ring score at 64/71, vector score at 45/49,
  and direct-map score at 54/58;
- rejects a hostile function claiming an inserted bit is absent;
- rejects a hostile function claiming the FIFO is LIFO;
- rejects a hostile function claiming vector pop is FIFO;
- rejects a hostile function claiming an inserted map entry is missing;
- builds `fixed_ring_advance` as a strict freestanding L3 export;
- replays every strict translation-validation row;
- requires all four original package modules and the source map in the receipt;
  and
- tampers with the bound direct-map source and requires validation to fail.

The strict export is intentionally scalar. Body TV now frames direct and nested
finite-record mutation plus user-ADT match/results, but complete collection
transitions still traverse mutable-reference callee chains and quantified
aggregate frames. Those call-effect/quantified forms remain outside the strict
body denotation, so this increment does not claim that the whole ring lifecycle
is a strict public receipt export. The complete package source is bound by the
scalar receipt, while aggregate operations retain their individual L3
certificates and the generic fixed-array TV evidence.

## Remaining collection closure

This is a substantial REQ-KPRIM-2 increment, not completion. Remaining work is:

1. popcount, set-bit search, and bulk bitmap operations;
2. open-addressed or chained maps, slabs/freelists, and intrusive-list metadata;
3. capacity/type parameterization that does not rely on privileged generated
   policy types;
4. quantified framing and equality for aggregate collection states;
5. mutable-reference callee-effect and quantified aggregate body TV so complete
   transitions can be strict exports;
6. static-storage ownership and initialization; and
7. atomic integration for concurrent containers; pure bounded MPSC and
   work-stealing deque state mechanics are supplied by the synchronization
   package.

## Auditable metrics

At this increment:

| Metric | Value |
|---|---:|
| Physical Thermite LOC | 1,287 |
| Nonblank Thermite LOC | 1,211 |
| Thermite functions | 65 (53 executable, 12 specification) |
| In-language L3 items | 76 |
| Frozen boundary declarations | 0 |
| Executable mutants killed | 196/218 |
| Bodyful Rust/assembly collection implementations | 0 |
| Ordinary Rust kernel-policy/algorithm LOC | 0 |
| Direct-Verus TPL LOC shipped by this package | 0 |

The Rust integration test is proof, replay, and tamper harness code; it is not
linked into the collection artifact.
