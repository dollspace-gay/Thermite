# Allocation-free fixed collections

<!--
tier: 3-component
status: partial
decision: Thermite ships policy-free packed bitmap, vector, FIFO-ring, direct-map, and open-addressed map mechanics in .th; generic capacities, slabs, intrusive metadata, and complete aggregate receipt TV remain
governs:
  - stdlib/kernel-primitives/collections.thpkg.json
  - stdlib/kernel-primitives/collections/bitmap.th
  - stdlib/kernel-primitives/collections/direct_map.th
  - stdlib/kernel-primitives/collections/open_map.th
  - stdlib/kernel-primitives/collections/ring.th
  - stdlib/kernel-primitives/collections/vector.th
  - forge/tests/fixed_collections.rs
audited-content-sha256: a7abf12ec73130a5ba4d86ac82d5ee6caa1c306b0637a8eda52cc8ae9a09c9d5
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
package. It binds its five manifest-declared Thermite modules without a
Rust runtime implementation, platform boundary, heap dependency, or hosted
effect. The modules use native fixed arrays and ordinary verified Thermite
control flow.

## Fixed bitset

`FixedBitmap256` is a packed 256-bit bitmap backed by `[u64; 4]`, with:

- an allocation-free empty constructor;
- capacity and representation-validity queries;
- bounded membership lookup;
- exact population count and first-set search from any bounded offset;
- bulk union, intersection, and difference; and
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

Population count is specified by an exact recursive prefix count. First-set
search proves both that the returned bit is present and that the preceding
range is clear; absence proves the complete requested range is clear. Bulk
union, intersection, and difference pin all four result words through exact
fixed-array equality. Generic capacities and a quantified all-indices public
contract remain future work.

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
resolution. Consumers that need explicit collision reporting can use it; the
sibling open-addressed map supplies allocation-free linear probing.

## Open-addressed map

`FixedOpenMap64` is an opaque 64-slot `usize -> u64` map with empty, occupied,
and deleted slot states. Keys start at `key % 64` and advance with bounded
linear probing. Lookup terminates at the first empty slot. Insert remembers the
first tombstone, replaces an existing key, reuses the earliest deleted slot, or
returns the owned map/key/value unchanged when all 64 slots are occupied.
Removal writes a tombstone rather than an empty marker, preserving lookup for
keys later in the probe chain.

The recursive find/search specifications exactly describe wraparound,
termination, collision traversal, and tombstone selection. Insert, replacement,
and removal contracts use fixed-array equality and same-except relations to pin
the complete unchanged state and the one modified slot. Source probes prove
insert-then-lookup, a collision between keys 0 and 64 selecting slot 1, and
delete-then-search reusing tombstone slot 0. The representation is opaque and
its field names are module-unique, so the package resolver preserves the
foreign-construction/read/write barrier across the five-root closure.

## Assurance and adversarial evidence

`forge check --level l3` proves all 136 source items across the five modules at
L3. There are no boundaries. Executable contract mutation kills 342 of 372
generated mutants; the surviving mutants remain counted and the per-function
scores stay above the configured floor.

`forge/tests/fixed_collections.rs` additionally:

- requires every source row to be L3 and boundary-free;
- pins the bitmap score at 107/114, ring score at 64/71, vector score at 45/49,
  direct-map score at 54/58, and open-map score at 72/80;
- rejects hostile functions claiming an inserted bit is absent, population
  count exceeds capacity, or union drops a present bit;
- rejects a hostile function claiming the FIFO is LIFO;
- rejects a hostile function claiming vector pop is FIFO;
- rejects a hostile function claiming an inserted map entry is missing;
- rejects a hostile function claiming a linear-probe collision selects the
  occupied home slot;
- builds `fixed_ring_advance` as a strict freestanding L3 export;
- replays every strict translation-validation row;
- requires all five package modules and the source map in the receipt;
  and
- tampers with the bound direct-map source and requires validation to fail.

The strict export is intentionally scalar. Body TV now frames direct and nested
finite-record mutation, user-ADT match/results, and exact statement-position
mutable calls over direct finite-record roots. The collection package itself uses
owned state transitions; complete collection exports remain gated by quantified
aggregate framing and a dedicated strict aggregate receipt/runtime fixture. This
increment therefore does not yet claim that the whole ring lifecycle is a strict
public receipt export. The complete package source is bound by the scalar receipt,
while aggregate operations retain their individual L3 certificates and the
generic fixed-array TV evidence.

## Remaining collection closure

This is a substantial REQ-KPRIM-2 increment, not completion. Remaining work is:

1. slabs/freelists and intrusive-list metadata;
2. a chained-map variant where consumer workloads require it;
3. capacity/type parameterization that does not rely on privileged generated
   policy types;
4. quantified framing and equality for aggregate collection states;
5. quantified aggregate body TV and strict aggregate receipt/runtime fixtures;
6. atomic integration for concurrent containers; pure bounded MPSC and
   work-stealing deque state mechanics are supplied by the synchronization
   package.

Allocation-free static-storage ownership and initialization are now supplied by
the sibling `stdlib/kernel-primitives/static-storage.thpkg.json` package; see
`.design/build/static-storage.md`. Collection-to-storage composition remains a
consumer of that primitive rather than a second storage implementation here.

## Auditable metrics

At this increment:

| Metric | Value |
|---|---:|
| Physical Thermite LOC | 2,568 |
| Nonblank Thermite LOC | 2,432 |
| Thermite functions | 119 (76 executable, 43 specification) |
| In-language L3 items | 136 |
| Frozen boundary declarations | 0 |
| Executable mutants killed | 342/372 |
| Bodyful Rust/assembly collection implementations | 0 |
| Ordinary Rust kernel-policy/algorithm LOC | 0 |
| Direct-Verus TPL LOC shipped by this package | 0 |

The Rust integration test is proof, replay, and tamper harness code; it is not
linked into the collection artifact.
