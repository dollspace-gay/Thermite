# Allocation-free fixed collections

<!--
tier: 3-component
status: partial
decision: Thermite ships policy-free fixed bitset and FIFO mechanics in .th; packed representations, generic capacities, maps, vectors, slabs, and complete aggregate receipt TV remain
governs:
  - stdlib/kernel-primitives/collections.thpkg.json
  - stdlib/kernel-primitives/collections/bitmap.th
  - stdlib/kernel-primitives/collections/ring.th
  - forge/tests/fixed_collections.rs
audited-content-sha256: 604d4609d33a23ee98b01954bfb8d3d7bbbd5dda4c0a6765ba2b9cb2b8edd3e5
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

`stdlib/kernel-primitives/collections.thpkg.json` is a canonical two-root
package. It binds `collections/bitmap.th` and `collections/ring.th` without a
Rust runtime implementation, platform boundary, heap dependency, or hosted
effect. Both modules use native fixed arrays and ordinary verified Thermite
control flow.

## Fixed bitset

`FixedBitmap256` is a 256-entry boolean bitset with:

- an allocation-free empty constructor;
- capacity and representation-validity queries;
- bounded membership lookup; and
- owned insert, remove, and set-to transitions.

Every transition preserves the fixed capacity and pins the requested bit's
final value. The generated code is a native `[bool; 256]` indexed update. The
existing fixed-array verifier and translation-validation machinery establish
the language operation's exact final-array update; this library adds the
collection-level contracts and composition probes.

This increment deliberately does not call the representation a packed bitmap.
The first attempted `[u64; 4]` formulation exposed that a compositional ordinary
L3 contract cannot yet reuse a dynamic fixed-width shift proof. `@bv64` clauses
can prove a standalone shift at L4, but they do not currently supply the
ordinary array/struct contract needed by callers. Shipping the boolean bitset
keeps the executable and proof claim identical. A packed representation needs
an explicit bitvector-to-ordinary-contract refinement bridge, not a trusted
mask helper.

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

## Assurance and adversarial evidence

`forge check --level l3` proves all 30 source items across the two modules at
L3. There are no boundaries. Executable contract mutation kills 78 of 87
generated mutants; the surviving mutants remain counted and the per-function
scores stay above the configured floor.

`forge/tests/fixed_collections.rs` additionally:

- requires every source row to be L3 and boundary-free;
- pins the bitmap score at 14/16 and ring score at 64/71;
- rejects a hostile function claiming an inserted bit is absent;
- rejects a hostile function claiming the FIFO is LIFO;
- builds `fixed_ring_advance` as a strict freestanding L3 export;
- replays every strict translation-validation row;
- requires both original package modules and the source map in the receipt; and
- tampers with the bound ring source and requires validation to fail.

The strict export is intentionally scalar. Current body TV cannot frame a
complete named-aggregate/ADT push-pop closure, so this increment does not claim
that the whole ring lifecycle is a strict public receipt export. The complete
package source is bound by the scalar receipt, while aggregate operations retain
their individual L3 certificates and the generic fixed-array TV evidence.

## Remaining collection closure

This is a substantial REQ-KPRIM-2 increment, not completion. Remaining work is:

1. a packed bitmap with a proved bitvector-to-L3 refinement bridge;
2. fixed-capacity vectors, key/value maps, slabs/freelists, intrusive-list
   metadata, and deque mechanics;
3. capacity/type parameterization that does not rely on privileged generated
   policy types;
4. quantified framing and equality for aggregate collection states;
5. named-aggregate/ADT body TV so complete transitions can be strict exports;
6. static-storage ownership and initialization; and
7. atomic/waiting consumers such as MPSC rings and work-stealing deques.

## Auditable metrics

At this increment:

| Metric | Value |
|---|---:|
| Physical Thermite LOC | 400 |
| Nonblank Thermite LOC | 370 |
| Thermite functions | 26 (22 executable, 4 specification) |
| In-language L3 items | 30 |
| Frozen boundary declarations | 0 |
| Executable mutants killed | 78/87 |
| Bodyful Rust/assembly collection implementations | 0 |
| Ordinary Rust kernel-policy/algorithm LOC | 0 |
| Direct-Verus TPL LOC shipped by this package | 0 |

The Rust integration test is proof, replay, and tamper harness code; it is not
linked into the collection artifact.
