# Generation-bound atomic storage initialization

<!--
tier: 3-component
status: partial
decision: committed static-storage regions are consumed by L3 Thermite functions into typed opaque single-use atomic initialization slots; only consumer machine atomics remain bodyless boundaries
governs:
  - stdlib/kernel-primitives/atomics.thpkg.json
  - stdlib/kernel-primitives/src/model.th
  - stdlib/kernel-primitives/src/init.th
  - stdlib/kernel-primitives/src/machine.th
  - stdlib/kernel-primitives/src/api.th
  - stdlib/kernel-primitives/src/atomic_storage.th
  - stdlib/kernel-primitives/storage/static_storage.th
  - forge/tests/verified_build.rs
audited-content-sha256: 35bace9cfe0cadf18e7293cc4bb9619e7ab92ef6432b1ad688580b9d4c864967
extends:
  - .design/build/static-storage.md
  - .design/build/generation-ownership.md
  - .design/build/sealed-atomics.md
  - .design/build/kernel-primitives.md
-->

## Decision

The atomic package consumes the receipt-bound static-storage module directly.
A committed `StaticStorageRegion` may be converted exactly once into one of four
opaque initialization slots:

- `AtomicBoolSlot`;
- `AtomicU32Slot`;
- `AtomicU64Slot`; or
- `AtomicUsizeSlot`.

The conversion, capacity policy, generation identity transfer, initialization
factory, and lifecycle orchestration are ordinary bodyful Thermite and certify
at L3. The application initialization function consumes the slot, calls the
scalar/tuple machine initialization door, checks its echoed identity through
the door contract, and constructs the sealed cell as its exact named verified
factory. No Rust storage policy, Rust slot ledger, or parallel atomic
implementation is present in this repository.

## Receipt-bound package graph

`stdlib/kernel-primitives/atomics.thpkg.json` binds six modules:

```text
atomic_storage -> api -> init -> static_storage
       |           |      `-> model
       |           |-> machine -> model
       |           `-----------> model
       `-----------------------> model
```

The manifest has `api` and `atomic_storage` roots. Direct imports are explicit,
the complete graph is source-mapped into every receipt, and replay revalidates
all six exact files. The standalone static-storage package remains usable by a
consumer that does not need atomics.

## Identity and capacity

`AtomicIdentity` is a platform-independent triple:

```thermite
struct AtomicIdentity {
  authority: usize,
  slot: usize,
  generation: u64,
}
```

It replaces the previous scalar cell identity. Atomic observations,
transitions, writes, and finite-history events carry this full identity, so two
storage slots under one authority cannot collapse into the same proof-model
cell. Equality is an explicit L3 relation over all three fields.

The storage-to-slot converters preserve the committed region's authority, slot,
generation, and capacity exactly. Capacity policy is also Thermite logic:

| Slot | Required bytes |
|---|---:|
| `bool` | 1 |
| `u32` | 4 |
| `u64` | 8 |
| `usize` | 8 |

The `usize` row is bound to the current 64-bit generic-kernel artifact target by
the receipt's target and pointer-width fields. A future non-64-bit target must
provide a separately proved target-width policy rather than silently reusing
this constant.

## Single-use protocol

The usable path is:

```text
claim region
  -> generation-bound StaticStorageLease
  -> fill_bytes witness
  -> commit consumes lease + witness
  -> committed StaticStorageRegion
  -> typed slot conversion consumes region
  -> atomic init consumes opaque slot
  -> sealed Atomic* cell
```

The following properties are enforced at the L3 surface:

1. a foreign package module cannot construct or inspect an `Atomic*Slot`;
2. a committed region is moved into exactly one typed conversion;
3. the resulting slot is moved into exactly one initialization call;
4. attempts to use the same slot for two initializations do not certify at L3;
5. authority, slot, and generation are preserved into the sealed cell;
6. the minimum capacity is proved before conversion; and
7. the complete claim/fill/commit/convert/init orchestration is generated from
   `atomic_storage.th`, not duplicated in Rust.

This is an enforceable single-use discipline for the atomic initialization
slots. It does not claim that Thermite now has a general affine type qualifier
for every user-defined type. The opaque construction barrier, non-copying L3
lowering, move checking, and generation identity together are the deliberately
narrow mechanism used here.

## Assurance split

All bodyful application/library functions in this package are L3, including the
50 atomic application operations. The only L1 rows are the matching 50 atomic
machine doors and the two static-storage machine doors. Those 52 declarations
are bodyless because their implementations depend on consumer-selected memory
and machine atomic operations.

`atomic_storage_capacity_probe` is a strict kernel-target L3 export with no
reachable boundary. Its receipt compiles and executes the generated Thermite
capacity logic. The lifecycle functions that reach `fill_bytes` and atomic
initialization certify individually at L3, but an end-to-end publication that
reaches those declarations remains correctly refused until a machine-aware
registry binds and directly refines the exact consumer objects.

## Acceptance evidence

`atomic_primitive_package_keeps_every_in_language_item_at_l3` requires:

- all 202 non-boundary certificates in the six-module projection to be L3;
- exactly 52 named bodyless L1 boundaries;
- every one of the 50 atomic machine doors to have a matching bodyful L3
  application operation;
- nonzero mutation teeth for every new identity, conversion, and lifecycle
  function;
- duplicate slot use to fail L3 certification;
- foreign opaque slot construction to fail at package validation;
- strict kernel ordering and storage receipts plus the hosted finite-history
  receipt to build and replay;
- every receipt to bind all six package sources, including `machine.th`;
- every reachable translation-validation row to be faithful;
- generated storage-capacity logic to compile and execute in a separate Rust
  consumer; and
- removing the bound `#[opaque]` slot barrier to make replay fail.

## Auditable metrics

The complete atomic package source closure now has:

| Metric | Value |
|---|---:|
| Physical Thermite LOC | 3,413 |
| Nonblank Thermite LOC | 3,152 |
| Thermite functions | 237 (178 executable, 59 specification) |
| Bodyful executable Thermite functions | 126 |
| In-language L3 certificate rows | 202 |
| Bodyful L3 atomic application operations | 50 |
| Frozen boundary declarations | 52 at L1 |
| Ordinary Rust kernel-policy/algorithm LOC | 0 |
| Bundled Rust/assembly machine implementations | 0 |

The Rust acceptance test is proof, replay, tamper, and generated-artifact
harness code. It is not a primitive implementation.

## Remaining work

Single-use initialization ownership and static-storage integration are shipped.
REQ-KPRIM-5 remains partial because completion still requires:

- a kernel-target finite-history proof surface;
- exact atomic object/instruction semantics and direct refinement tied to the
  consumer's source, object, ABI, target, and feature set;
- positive end-to-end lifecycle composition through that machine-aware
  registry; and
- synchronization consumers connected to the exact atomic transitions and
  concurrency model.

Those remaining tasks may add bodyless declarations and consumer refinement
evidence. They must not add a concrete kernel or an ordinary Rust policy
implementation here.
