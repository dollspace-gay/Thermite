# Allocation-free fixed collections

<!--
tier: 3-component
status: partial
decision: Thermite ships policy-free packed bitmap, vector, FIFO-ring, direct-map, open-addressed map, generation-safe slab, and duplicate-safe freelist mechanics in .th; generic capacities, intrusive metadata, and quantified aggregate framing remain
governs:
  - stdlib/kernel-primitives/collections.thpkg.json
  - stdlib/kernel-primitives/collections/bitmap.th
  - stdlib/kernel-primitives/collections/direct_map.th
  - stdlib/kernel-primitives/collections/open_map.th
  - stdlib/kernel-primitives/collections/freelist.th
  - stdlib/kernel-primitives/freelist.thpkg.json
  - stdlib/kernel-primitives/collections/slab.th
  - stdlib/kernel-primitives/slab.thpkg.json
  - stdlib/kernel-primitives/collections/ring.th
  - stdlib/kernel-primitives/collections/vector.th
  - forge/tests/fixed_collections.rs
  - forge/tests/fixed_freelist.rs
  - forge/tests/fixed_slab.rs
audited-content-sha256: 653fdde9a839d495afd1e5c1744f0e22297876f452b2005ff53b766b2f2c648b (re-pinned 2026-08-05 after the strict L3 duplicate-safe freelist increment)
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

`stdlib/kernel-primitives/collections.thpkg.json` is the canonical five-module
collection package. The generation-safe slab and duplicate-safe freelist also
have focused `stdlib/kernel-primitives/{slab,freelist}.thpkg.json` receipt roots
so their aggregate public transitions can be built, replayed, and attacked
independently. None of these packages contains a Rust runtime implementation,
platform boundary, heap dependency, or hosted effect. The modules use native
fixed arrays and ordinary verified Thermite control flow.

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

## Generation-safe slab

`FixedSlab64` owns 64 occupancy bits, generation counters, and `u64` values.
Allocation finds the first free slot, advances that slot's nonzero generation,
stores the value, and returns an opaque handle containing the exact slot and
generation. A full slab and a slot whose generation is exhausted are explicit
result variants that return the unconsumed value. Lookup succeeds only for a
currently occupied slot whose generation matches the handle.

Release consumes both the slab and handle. A live handle returns the stored
value, clears exactly its occupancy/value slot, and preserves the generation so
the handle becomes stale immediately. Invalid slot, vacant slot, stale
generation, and zero generation are rejected with explicit reasons and return
the original owned values. The opaque slab and handle cannot be cloned, forged,
or inspected outside their defining module.

Contracts pin all three fixed arrays through exact equality or same-except
framing. Source probes compose allocation with lookup and release, prove the
returned value, and prove that the released handle is no longer live. The
focused package exports the aggregate allocation/lookup probe through a strict
kernel-target L3 receipt; all reachable contract, expression, body, and wrapper
TV rows are faithful.

## Duplicate-safe freelist

`FixedFreelist64` is an opaque, allocation-free stack of 64 storage indices.
Its fixed node array supplies LIFO order, while an exact presence bitmap rejects
duplicate insertion without scanning or allocating. Push distinguishes success,
duplicate membership, out-of-range input, and capacity exhaustion. Pop
distinguishes success, emptiness, and fail-closed corrupt metadata; no unchecked
index is used on a rejection path.

Successful push and pop contracts pin the length transition, written/cleared
stack slot, changed membership bit, and complete same-except frames for both
arrays. Every rejection returns the unchanged owned state and an explicit node
or reason. Source probes prove two-element LIFO behavior, duplicate rejection,
and release/reuse of an index. The focused package exports the LIFO probe as a
strict kernel-target L3 receipt with faithful contract, expression, body, and
wrapper TV. The state is not clonable, and there is no parallel Rust freelist.

## Assurance and adversarial evidence

`forge check --level l3` proves all 195 source items across the collection, slab,
and freelist modules at L3. There are no boundaries. Executable contract
mutation kills 461 of 498 generated mutants; the surviving mutants remain counted and the
per-function scores stay above the configured floor.

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

`forge/tests/fixed_slab.rs` separately:

- requires all 36 slab items to be boundary-free L3 and pins its executable
  mutation score at 70/73;
- rejects stale-handle-live claims, post-release-live claims, and handle cloning;
- builds and replays the aggregate slab probe under the kernel target;
- requires every reachable translation-validation row to be faithful; and
- removes the bound opacity marker and requires replay to fail.

`forge/tests/fixed_freelist.rs` separately:

- requires all 23 freelist items to be boundary-free L3 and pins its executable
  mutation score at 49/53;
- rejects false duplicate-acceptance, out-of-range-membership, and state-cloning
  claims;
- builds and replays the LIFO probe under the kernel target;
- requires every reachable translation-validation row to be faithful; and
- removes the bound opacity marker and requires replay to fail.

The canonical five-root package retains a scalar ring export, while the focused
slab and freelist packages supply strict aggregate receipt fixtures. Body TV frames
direct and nested finite-record mutation, user-ADT match/results, exact
statement-position mutable calls over direct finite-record roots, and the slab's
and freelist's fixed-array state. Quantified all-index aggregate framing remains
open, so these increments do not generalize the focused results into a claim that
every collection lifecycle is already a strict public export.

## Remaining collection closure

This is a substantial REQ-KPRIM-2 increment, not completion. Remaining work is:

1. intrusive-list metadata;
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
| Physical Thermite LOC | 3,799 |
| Nonblank Thermite LOC | 3,605 |
| Thermite functions | 169 (105 executable, 64 specification) |
| In-language L3 items | 195 |
| Frozen boundary declarations | 0 |
| Executable mutants killed | 461/498 |
| Bodyful Rust/assembly collection implementations | 0 |
| Ordinary Rust kernel-policy/algorithm LOC | 0 |
| Direct-Verus TPL LOC shipped by this package | 0 |

The Rust integration test is proof, replay, and tamper harness code; it is not
linked into the collection artifact.
