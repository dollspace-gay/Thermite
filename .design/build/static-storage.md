# Allocation-free static-storage ownership

<!--
tier: 3-component
status: partial
decision: Thermite owns the verified allocation-free claim/fill/commit/release protocol; a consumer owns the concrete static object and directly refines the two irreducible authority/memory declarations
governs:
  - stdlib/kernel-primitives/static-storage.thpkg.json
  - stdlib/kernel-primitives/storage/static_storage.th
  - thermite-tv/src/exec_stmt_encode.rs
  - forge/tests/static_storage_primitives.rs
audited-content-sha256: 083faef6dfc91788e0536c44feb5a7bec924f1a3b8069d84eab2350f13aea55e (re-pinned 2026-08-05 after logical-record value observers joined the aggregate lifecycle engine; static-storage semantics remain regression-covered)
extends:
  - .design/build/kernel-primitives.md
  - .design/build/generation-ownership.md
  - .design/build/atomic-storage-initialization.md
  - .design/build/frozen-primitive-registry.md
  - .design/build/kernel-target.md
thesis-refs:
  - thermite-design.md §3
  - thermite-design.md §5.3
  - thermite-design.md §6
  - thermite-design.md §9
-->

## Decision

Thermite ships the reusable ownership and initialization protocol for static
storage. It does not declare a kernel global, choose an address, lay out a page,
provide an allocator, or implement a raw memory operation. Those are properties
of a consuming platform and must remain outside this platform-free repository.

The package is
`stdlib/kernel-primitives/static-storage.thpkg.json`. Its single module,
`storage/static_storage.th`, contains all storage state, validation, generation,
claim, commit, release, and rejection algorithms in Thermite. The canonical
manifest, original source, module map, generated source, proof evidence,
translation-validation rows, toolchain, and freestanding rlib are bound into a
strict receipt.

The defining module also exposes exact L3 observers for lease and committed
region authority, slot, generation, capacity, and fill metadata. Those
observers preserve opaque representation ownership while allowing the atomic
package to consume a committed region without copying its implementation.

There is no kernel, firmware runtime, architecture profile, Rust storage
algorithm, assembly implementation, linker script, or image builder in this
increment.

## Storage model

The policy-free ledger has 64 slots. Each slot records:

- whether an uninitialized lease is reserved;
- whether initialization has been committed;
- a nonwrapping `u64` generation; and
- a nonzero byte capacity.

`StaticStorageLedger`, `StaticStorageLease`, and `StaticStorageRegion` are
opaque. Only their defining Thermite module can construct or inspect their
representations. The ledger owns one sealed `StaticStorageAuthority`, so moving
the ledger moves the authority; ordinary Thermite cannot call a transition
twice with the same ledger or clone it. A lease carries the authority identity,
slot, generation, and capacity needed to validate it against the current
ledger. A committed region additionally records the fill byte witnessed by the
platform operation.

The protocol is:

```text
sealed authority
      |
      v
 empty fixed ledger --claim--> live uninitialized lease
                                  |             |
                                  | release     | fill boundary
                                  v             v
                            reusable slot   sealed fill witness
                                                  |
                                                  v
                                           commit in Thermite
                                                  |
                                                  v
                                     initialized opaque region
```

Claim rejects zero capacity, a reserved slot, an initialized slot, and
generation exhaustion. Release is permitted only for a live uninitialized
lease. Reclaiming a released slot increments its generation, so the old lease
cannot become live again. Commit consumes both the lease and the sealed fill
witness; it succeeds only when identity, slot, generation, and capacity match.
An initialized slot remains reserved and has no release transition in this
primitive, preventing the abstraction from silently reusing memory that a
consumer may still expose.

## Platform boundary

Exactly two source declarations are bodyless:

- `thermite::storage::static_authority` binds a consumer-selected static-object
  identity to a sealed Thermite authority; and
- `thermite::storage::fill_bytes` performs the concrete memory initialization
  for a live lease and returns a sealed witness carrying the exact identity,
  slot, generation, capacity, and fill byte.

Both declarations are honestly L1 in this repository. Thermite has no
platform-independent way to name a writable global object, preserve raw-pointer
provenance, or prove a concrete volatile or bytewise machine operation against
an emitted Rust/assembly object. Marking either declaration L3 here would be a
false proof claim.

A consumer upgrades a reachable instance only by binding the exact static
object, target features, implementation source, emitted object, memory model,
and direct refinement in the machine-aware registry. A contract-only wrapper,
safe-Rust model test, or boot marker is insufficient. Everything on either side
of those literal machine actions—the slot protocol, generation discipline,
matching checks, state transitions, and rejection behavior—is bodyful Thermite
and must remain L3 or L4.

## Translation validation

The strict executable surface is `static_storage_claim_reason`. It is a scalar,
freestanding decision operation reused by the aggregate claim transition. Its
five outcomes execute from the generated Thermite rlib in the acceptance test;
there is no parallel Rust implementation.

That strict build exposed an independent body-TV elaboration defect: an
annotated `let reason: u8 = if ...` lost its bounded result context after state
substitution, leaving branch literals polymorphic in the Verus `ensures`
expression. The body reference now propagates an annotated bounded integer type
through every `if` and `match` result arm and casts bare literals at the leaves.
This preserves the source type rather than borrowing production lowering. A
unit test pins the exact nested-branch reference term, and the package build
requires the resulting body-TV row to be faithful.

## Acceptance evidence

`forge/tests/static_storage_primitives.rs` enforces the complete package claim:

- all 31 in-language type, specification, and bodyful operation rows are L3;
- the exact two-item boundary set is L1 with exact semantic targets;
- every important bodyful transition or probe has a nonzero mutation score;
- moving one ledger twice and cloning a lease fail L3 certification;
- a foreign package module cannot construct an opaque lease;
- the strict freestanding package receipt replays with only faithful TV rows;
- a compiled consumer executes all five generated claim-reason branches; and
- removing the receipt-bound opaque barrier makes replay fail.

The source lifecycle probes additionally prove successful claim/fill/commit to
the sealed boundary contract, zero-capacity rejection, mismatched-witness
rejection, release invalidation, and stale-lease rejection after slot reuse.
These are proofs over the generated Thermite state machine, not expected QEMU
markers.

## Assurance and remaining composition

At this increment:

| Metric | Value |
|---|---:|
| Physical Thermite LOC | 852 |
| Nonblank Thermite LOC | 801 |
| Thermite functions | 43 (27 executable, 16 specification) |
| Bodyful executable Thermite functions | 25 |
| In-language L3 items | 49 |
| Frozen boundary declarations | 2 at L1 |
| Executable mutants killed | 93/93 |
| Bodyful Rust/assembly primitive implementations | 0 |
| Ordinary Rust kernel-policy/algorithm LOC | 0 |
| Direct-Verus TPL LOC shipped by this package | 0 |

The generic static-storage state machine is shipped. End-to-end machine storage
assurance remains a consumer composition obligation until the generic registry
can bind and directly refine the exact authority and fill implementations. The
generation-bound lease/region protocol is now connected to typed opaque atomic
initialization slots by `.design/build/atomic-storage-initialization.md`. Exact
atomic object refinement remains machine-aware registry work. That task must
not introduce a concrete kernel or a parallel Rust storage algorithm here.
