# Thermite kernel-authoring primitives

<!--
tier: 3-component
status: partial
decision: Thermite ships reusable verified language, library, boundary, and build primitives; kernels and platform implementations live in consumer projects
governs:
  - THERMITE.skill.md
  - thermite-syntax/src/ast.rs
  - thermite-syntax/src/parser.rs
  - thermite-syntax/src/package.rs
  - thermite-syntax/src/lib.rs
  - thermite-syntax/tests/package.rs
  - thermite-syntax/tests/kernel_surface.rs
  - thermite-syntax/tests/conformance.rs
  - thermite-spec/src/validator.rs
  - thermite-spec/tests/fixed_array_validate.rs
  - thermite-spec/tests/atomic_ordering_validate.rs
  - thermite-spec/tests/u64_bit_methods_validate.rs
  - thermite-lower/src/effects.rs
  - thermite-lower/src/l1.rs
  - thermite-lower/src/lib.rs
  - thermite-lower/src/lower.rs
  - thermite-lower/tests/fixed_array.rs
  - thermite-lower/tests/kernel_mutable_slice.rs
  - thermite-lower/tests/effects_verified.rs
  - thermite-lower/tests/u64_bit_methods.rs
  - thermite-verified/src/lib.rs
  - thermite-tv/src/exec_stmt_encode.rs
  - thermite-tv/src/exec_encode.rs
  - thermite-tv/src/obligation.rs
  - thermite-tv/src/ref_encode.rs
  - thermite-tv/tests/fixed_array_tv.rs
  - lean/Thermite/Ast.lean
  - lean/Thermite/Denote.lean
  - lean/Thermite/RefEncode.lean
  - forge/src/build.rs
  - forge/src/body_tv.rs
  - forge/src/contract_tv.rs
  - forge/src/exec_tv.rs
  - forge/tests/body_tv.rs
  - forge/tests/contract_tv_conformance.rs
  - forge/tests/exec_tv_conformance.rs
  - forge/src/verified_build.rs
  - forge/src/verified_build/composition.rs
  - forge/src/verified_build/primitive_registry.rs
  - forge/src/thermite_package.rs
  - forge/tests/verified_build.rs
  - forge/tests/fixed_collections.rs
  - forge/tests/ownership_primitives.rs
  - forge/tests/synchronization_primitives.rs
  - stdlib/kernel-primitives/collections.thpkg.json
  - stdlib/kernel-primitives/collections/bitmap.th
  - stdlib/kernel-primitives/collections/direct_map.th
  - stdlib/kernel-primitives/collections/ring.th
  - stdlib/kernel-primitives/collections/vector.th
  - stdlib/kernel-primitives/ownership.thpkg.json
  - stdlib/kernel-primitives/ownership/generation.th
  - stdlib/kernel-primitives/synchronization.thpkg.json
  - stdlib/kernel-primitives/synchronization/barrier.th
  - stdlib/kernel-primitives/synchronization/epoch_ack.th
  - stdlib/kernel-primitives/synchronization/mpsc_queue.th
  - stdlib/kernel-primitives/synchronization/once.th
  - stdlib/kernel-primitives/synchronization/refcount.th
  - stdlib/kernel-primitives/synchronization/seqlock.th
  - stdlib/kernel-primitives/synchronization/ticket_lock.th
  - stdlib/kernel-primitives/synchronization/wait.th
  - stdlib/kernel-primitives/synchronization/work_deque.th
  - stdlib/kernel-primitives/atomics.thpkg.json
  - stdlib/kernel-primitives/src/api.th
  - stdlib/kernel-primitives/src/model.th
  - conformance/kernel_primitives.th
  - conformance/verified-build/aggregate_storage.th
  - conformance/verified-composition/frozen_primitive.th
  - conformance/verified-composition/frozen_primitive_shell.rs
  - conformance/verified-composition/frozen_primitive_registry.json
audited-content-sha256: a5934d84aa255b6c9add2a234d131098ff921d90cf355af12023f7f0e7076b84 (re-pinned 2026-08-04 after adding reusable opaque library-state construction; no kernel policy or implementation was added)
extends:
  - .design/build/kernel-target.md
  - .design/build/l3-rich-composition.md
  - .design/build/kernel-byte-slice.md
thesis-refs:
  - thermite-design.md §2
  - thermite-design.md §3
  - thermite-design.md §5.3
  - thermite-design.md §6
  - thermite-design.md §9
  - thermite-design.md §13
-->

## Scope

Thermite provides the reusable substrate needed to author kernels in Thermite.
It does not contain a kernel, a scheduler policy, an allocator policy, a device
driver, a firmware implementation, or a bootable machine image. Those belong
to a consuming kernel repository.

The intended split is:

| Thermite repository | Consumer kernel repository |
|---|---|
| scalar, aggregate, mutation, control-flow, and proof surface | capability ledger contents and ownership policy |
| fixed-capacity collections and synchronization libraries written in `.th` | frame allocator and virtual-memory policy |
| sealed platform types and frozen boundary declarations | scheduler, IPC, services, drivers, AP and shootdown state machines |
| generic boundary-registry validation and exact refinement composition | architecture-specific Rust/assembly implementations |
| freestanding verified library build and receipt closure | firmware entry, linker script, image packaging, and boot tests |

The design maximizes Thermite authorship by keeping only irreducible machine
operations behind frozen boundaries. A platform adapter may contain foreign ABI,
pointer/provenance operations, volatile access, privileged instructions,
atomics, entry/return assembly, and compiler-runtime necessities. Algorithms
over those operations are ordinary verified Thermite libraries.

### Explicit non-goals

- No bundled UEFI, BIOS, device-tree, Multiboot, or architecture boot runtime.
- No concrete x86, ARM, RISC-V, or simulator platform profile.
- No scheduler, allocator, IPC, page-table, AP, DMA, service, or driver policy.
- No QEMU serial protocol, boot markers, disk image, or release archive.
- No claim that Thermite itself is a kernel.

A small compiler conformance program may exercise a primitive in isolation. It
must not grow into a parallel demonstration kernel.

## Completion rule

A primitive is complete only when all applicable layers exist:

1. surface syntax and a fail-closed validator;
2. executable lowering for both hosted and freestanding targets;
3. specification lowering and proof obligations;
4. independent translation validation where the construct affects semantics;
5. positive and adversarial conformance tests;
6. receipt binding for every source, declaration, proof, generated input, and
   implementation input used by the primitive; and
7. documentation usable by a different repository without importing kernel
   policy from Thermite.

A Rust reference model alone is not a completed Thermite primitive. A boundary
declaration without an exact implementation/refinement binding is only a
declaration. A boot test does not substitute for a proof.

## Primitive inventory

### Language and storage basis

The kernel profile needs:

- unsigned machine integers `u8`, `u16`, `u32`, `u64`, and `usize`;
- checked arithmetic, bitwise operations, shifts, comparisons, and explicit
  conversions with bit-vector proof routing where requested;
- structs, enums, tuples, `Option`, `Result`, exhaustive matches, loops,
  recursion with decreases, and closed effects;
- immutable and mutable slices for all admitted scalar and plain aggregate
  element types, with `old(...)` and `final(...)` content views;
- fixed arrays `[T; N]`, constants usable as capacities, and allocation-free
  initialization, indexing, iteration, and mutation;
- fixed-capacity vector, map, bitmap, deque/ring, intrusive-list metadata, and
  slab/freelist components implemented as verified `.th` libraries;
- content-preserving specifications for copy, move, fill, split, subrange, and
  equality; and
- static-storage declarations that can be initialized without a heap and
  exposed only through sealed ownership.

The current implementation has the integer widths, core ADTs/control flow,
sealed structs, mutable borrowed-slice and borrowed-array assignment (including
slices whose elements are nested primitive arrays), exact `old(...)`/`final(...)`
content views, and the first-class fixed-array syntax, validation, native
L3/L2/L1 lowering, and independent exact initialization/read/indexed-write/length
translation validation described below. The strict L3 fixtures bind and replay
those contract, executable-guard, body-state, and wrapper proof rows, and record
shared/exclusive borrow ownership in the public ABI receipt.
Repeat initialization rejects non-copy element types before lowering.
`.array_eq(other)` now provides allocation-free extensional equality for every
primitive-scalar array, while `.array_same_except(other, index)` proves exact
frame preservation around one updated index. Both use const-generic scans whose
exact generated bodies are verified by Verus and independently
translation-validated. Total `u64.bit_test`, `.bit_set`, and `.bit_clear`
operations bridge finite direct bit-vector proofs into ordinary compositional
L3 contracts. `.bit_set_preserves_other(changed, observed)` and
`.bit_clear_preserves_other(changed, observed)` additionally prove exact
same-word framing for any two distinct in-range indices. All five methods are
independently checked by contract, expression, and body translation validation.
Static ownership,
borrows of general named aggregates, and aggregate-element equality remain. The
allocation-free collection package now supplies a packed 256-bit `[u64; 4]` bitmap,
64-entry `u64` vector, FIFO ring, and collision-explicit `usize`-to-`u64` direct
map in Thermite. Richer collision-resolving maps, slabs/freelists,
intrusive-list metadata and generic library capacities remain.

#### Fixed-array surface lock

The fixed-storage language form is native and capacity-generic; it is not a
family of privileged names such as `FixedArray8<T>`:

```thermite
const SLOT_COUNT: usize = 64;

fn replace(slots: [u64; SLOT_COUNT], at: usize, value: u64)
    -> [u64; SLOT_COUNT]
  req at < SLOT_COUNT
  ens result[at] == value
  fx pure
{
  let mut updated: [u64; SLOT_COUNT] = slots;
  updated[at] = value;
  updated
}
```

`const NAME: usize = INTEGER;` introduces an immutable package declaration.
Array lengths are either a non-negative integer literal or one such constant;
there is no ambient constant evaluation or target-dependent lookup. Duplicate,
unknown, cyclic, non-`usize`, and cross-module-without-import capacity names are
errors before lowering. Forge bounds an individual capacity and the recursively
expanded element count to 1,048,576 elements to prevent source-sized denial of
service; the bound is a toolchain constant, not a target ABI fact.

`[T; N]` is an owned, allocation-free value. `[value; N]` is repeat
initialization for copy-safe scalar/plain values, while `[a, b, ...]` is exact
element initialization and must contain precisely `N` values in an annotated
context. Existing indexing and indexed assignment are the only access/mutation
forms; bounded `for i in 0..N` supplies iteration without an allocator or hidden
iterator state. `.len()` is the constant `N`. For arrays whose element is one of
`u8`, `u16`, `u32`, `u64`, `usize`, or `bool`, `.array_eq(other)` compares every
element and returns exactly finite-view extensional equality.
`.array_same_except(other, index)` returns true exactly when every in-bounds
element other than `index` agrees; an out-of-bounds exception therefore means
full equality. This combines with an exact indexed postcondition to frame an
owned array update compositionally. Arrays nest, may
appear in plain structs/enums/tuples, and may be borrowed as immutable or mutable
arrays.

The executable representation is the target's native `[T; N]`, not `Vec<T>` and
not a generated per-capacity Rust policy type. Specification lowering exposes a
finite sequence view with length `N`; indexed assignment proves an exact update
and preservation of every other index. `old(...)` and `final(...)` retain their
existing state-transition meaning for mutable array borrows. Independent
translation validation compares initialization, reads, writes, equality, and
the finite view; changing a capacity, index, assigned value, or pre/post-state
selection must be detected. Contract TV reifies `old(...)` and `final(...)` as
independent symbolic snapshots, so postconditions are checked over arbitrary
transitions instead of a synthetic no-op state. Body TV observes the complete
final sequence and proves the exact chained update, including every unchanged
index.

#### Fixed-collection package

`stdlib/kernel-primitives/collections.thpkg.json` contains four policy-free root
modules. `FixedBitmap256` supplies bounded membership and owned insert/remove/
set-to transitions over packed `[u64; 4]` storage, with exact target-word and
other-word frames. `FixedRing64` supplies explicit full and
empty result variants, FIFO push/pop, modulo wraparound, and returned ownership
over `[u64; 64]`. `FixedVec64` adds bounded random access, replacement, and
owned LIFO push/pop. `FixedDirectMap64` adds deterministic direct-slot lookup,
insert, replacement, and removal while reporting key collisions and invalid
counts as explicit result variants. All 76 source items prove at L3 with no
boundary or runtime implementation; the executable contracts kill 196/218
generated mutants.

The package builds and replays as a strict freestanding receipt rooted at the
scalar ring-index transition, binding all four original modules and rejecting
receipt-source tampering. Full aggregate lifecycle export remains gated by
named-aggregate/ADT body TV. The packed bitmap's finite dynamic-bit bridge is
directly proved and independently translation-validated. Exact claims and
residual work are in `.design/build/fixed-collections.md`.

### Modules, packages, and receipts

A consumer kernel must be a multi-file project. The reusable package primitive
therefore provides:

- one canonical package manifest with package identity and explicit roots;
- relative module imports with no ambient search path;
- deterministic name resolution and cycle diagnostics;
- a complete transitive `.th` source closure;
- explicit composition shells and platform registries as separate inputs;
- a canonical allowlist that rejects `target`, `dist`, `__pycache__`,
  symlinks, path escapes, unsorted inputs, and undeclared generated files; and
- validation/replay that recomputes the same closure and all digests.

Single-file builds remain supported. Package support must not be implemented by
concatenating files without preserving source identity and diagnostic spans.

The current package layer independently parses manifest-declared modules,
preserves module/path/local-span origins for every item, rejects duplicate
declarations with both locations, validates a canonical rooted import graph,
enforces direct imports for cross-module calls and signature types, and restricts
public build roots to manifest root modules. It rejects cycles, unreachable
modules, symlinks, path escapes, and incidental generated-directory components.
L3 library and rich-composition builds consume the package AST, bind the
canonical manifest, every original module, the backend projection, and its
host-independent source map, and reconstruct and re-resolve all of them during
validation and replay. Other source-oriented Forge commands remain single-file
and are the remaining package integration work.

### Sealed authority and platform effects

`#[sealed]` types are the unforgeable source-level handles for capabilities,
interrupt-state tokens, atomic cells, physical/virtual regions, device ranges,
contexts, and other platform-owned resources. Ordinary Thermite cannot
construct a sealed value; a registered boundary may mint or transform one.

The closed effect family is:

`platform(boot)`, `platform(memory)`, `platform(mmio)`,
`platform(pio)`, `platform(irq)`, `platform(cpu)`,
`platform(atomic)`, `platform(smp)`, `platform(dma)`,
`platform(clock)`, `platform(entropy)`, and `platform(power)`.

These atoms participate in ordinary transitive effect subsumption. Hosted
`read`, `write`, `net`, `time`, `rand`, and `term` effects remain
invalid for the freestanding target.

The language must additionally support affine-style ownership transitions for
sealed values, or a verified generation/consumption discipline strong enough
to reject copies, stale generations, double release, and rights escalation.
That ownership facility is a language/library primitive; the policy governing
which capabilities a kernel grants is not.

The first policy-free generation discipline is now shipped as the receipt-bound
`stdlib/kernel-primitives/ownership.thpkg.json` package. A sealed authority is
moved into an opaque 64-slot fixed ledger; acquisition and renewal rotate
generations, release retires a handle, and rights may only remain equal or
narrow. Both ledger and handle construction are restricted to their declaring
module, while public closed specifications provide an abstract Verus contract
surface. All 23 in-language items prove at L3 and the executable surface kills
90/90 mutants. Probes prove double-release rejection, stale-handle rejection
after slot reuse, and rights-escalation rejection. The package contains no
kernel ledger policy and no Rust/assembly implementation.

This does not yet complete affine ownership. Code outside the defining module
is now blocked from constructing the opaque ledger/handle, including through
external safe Rust, but opacity alone does not prove linearity or reject all
aliases. Strict body TV also cannot yet frame the full named-aggregate lifecycle,
and the authority-mint boundary has no package-owned implementation/refinement.
The exact construction semantics are in
`.design/build/opaque-library-state.md`; the ownership claim and residual trust
are in `.design/build/generation-ownership.md`.

### Frozen boundary registry

Thermite defines a generic, versioned registry schema. Consumer projects supply
the entries and implementations. Each entry binds:

- canonical semantic name and schema version;
- exact Thermite signature, contract digest, and platform-effect row;
- sealed input/output types and ownership transition;
- target architecture, feature set, ABI, symbol, and alignment;
- implementation source and object identity;
- pure model or state-transition relation;
- concurrency and memory-ordering semantics;
- failure behavior; and
- direct proof and test evidence.

Forge computes the source-reachable boundary closure from package roots. It
rejects unknown, duplicate, unreachable-required, signature-drifted,
contract-drifted, effect-drifted, ABI-drifted, or implementation-drifted
entries. Registries are data owned by consumer platforms, not a hard-coded
104-operation x86 table in Thermite.

The first registry increment is shipped for exact same-crate direct-Verus
functions. `--primitive-registry` binds a strict versioned JSON document to an
L3 composition build; Forge independently reconstructs signature, contract,
effects, ownership, shell inventory, source digest, symbol, ABI, alignment,
concurrency/failure declarations, and one-to-one reachability. Registered
boundaries lower to real checked wrapper calls, never `external_body`, and the
single `--no-cheating` proof must establish their Thermite contracts. Receipts
and replay bind the registry bytes, resolved plan, reachable count, and proof
obligation count. Signature/effect/contract/source/ownership drift, missing or
extra reachable bindings, post-plan mutation, receipt tampering, and a lying
implementation all reject without publication.

This v1 path intentionally accepts only the Rust ABI, an empty target-feature
set, safe direct-Verus shell bodies, and `sequential` concurrency. It rejects
otherwise well-formed `atomic`, `volatile`, and `privileged` entries because
the same-crate checked-wrapper proof does not model their object or machine
semantics. Exact separate Rust/assembly object closure for irreducible machine
instructions remains; it cannot be claimed as directly refined through this
schema. The precise contract and limitations are in
`.design/build/frozen-primitive-registry.md`.

### Atomics and memory ordering

The sealed atomic surface is monomorphic over `bool`, `u32`, `u64`, and
`usize`. It includes:

- load, store, and swap;
- strong and weak compare-exchange;
- fetch add, subtract, and, or, xor, min, and max;
- compiler fence and hardware fence; and
- initialization from static storage or a uniquely owned region.

The frozen ordering enum is `Relaxed | Acquire | Release | AcqRel | SeqCst`.
The validator rejects illegal load/store/fence orders and illegal
compare-exchange failure orders before code generation.

The first primitive-only atomic package is now present at
`stdlib/kernel-primitives/atomics.thpkg.json`. It contains 50 bodyless frozen
boundary declarations, sealed initialization-slot and cell-handle types,
explicit observation/transition witnesses, exact wrapping fetch arithmetic,
the complete 45-case ordering table, and a bounded 256-event history model.
The validator recognizes atomic operations only from their exact
`#[boundary("thermite::atomic::...")]` target and requires literal ordering
variants; dynamic, malformed, wrong-arity, and illegal pairs fail before
lowering, and atomic boundary aliases are rejected. An unrelated function with
an atomic-looking source name is not special.

The executable ordering matrix builds and replays at strict L3 for the generic
freestanding target. The fixed-array history relations build and replay at
strict L3 for the hosted target because the current kernel-target Verus path is
deliberately `--no-vstd`, while array-view relation proofs use vstd's finite
array model. These are two proof surfaces over the same receipt-bound package,
not a claim that hosted evidence proves a machine atomic implementation. The
consumer still owes exact object/machine semantics and direct refinement for
every reachable atomic boundary. Registry v1 refuses to overstate that missing
assurance. Slot single-use also remains dependent on the unfinished affine or
generation discipline; sealing alone prevents construction, not stale copies.

The detailed contract, ordering matrix, verification split, and residual trust
statement are in `.design/build/sealed-atomics.md`.

The proof interface exposes modification order, reads-from, happens-before,
release sequences, and sequentially consistent order at the abstraction level
needed by library algorithms. Each registered target operation needs a direct
refinement proof tied to its exact emitted implementation. A safe Rust model or
an L1 boundary contract is not enough.

### Waiting and synchronization

Thermite is total by default, while real kernels sometimes wait until another
CPU changes state. The primitive layer must provide an explicit verified
waiting abstraction:

- bounded spin with a returned timeout;
- architecture pause/yield as a platform action;
- blocking wait with a wake token and stated fairness assumption; and
- terminal halt as an explicit divergent/terminal effect.

The verifier distinguishes safety (proved for every step) from liveness
(proved under named fairness/progress assumptions). An unannotated infinite
loop is not accepted as a lock implementation.

Ticket locks, spin locks, once cells, barriers, epoch-acknowledgement sets,
reference counts, seqlocks, bounded MPSC queues, and bounded work-stealing deque
mechanics are verified `.th` libraries built from the atomic and waiting
primitives. They are not Rust kernel implementations and not privileged
boundaries.

The receipt-bound synchronization package now ships a total bounded-wait trace
scan, frozen pause/block/terminal-halt declarations, and fail-closed ticket-lock,
participant-aware barrier, once, reference-count, seqlock, and bounded MPSC queue
mechanics, a generation-tagged 64-participant epoch-acknowledgement set, plus
two-phase owner/thief work-deque mechanics. Two hundred twenty-two
in-language items prove at L3; the three
machine-facing declarations remain honest L1 boundaries, and executable
contracts kill 756/834 mutants.
Probes cover FIFO handoff, frozen barrier membership, stale generations,
stale tickets and once tokens, poison, last-reference retirement without
resurrection, stale seqlock reads, out-of-order MPSC publication with FIFO
visibility, duplicate/stale queue tokens, owner/thief LIFO/FIFO ends, both
last-item race winners, epoch snapshot/acknowledgement/withdrawal and stale
epoch/participant rejection, and nonwrapping exhaustion. These are not yet machine
concurrency proofs: consumer code must connect the state mechanics to
sealed atomics and directly refined wait/atomic implementations. Exact claims
and residual work are in
`.design/build/synchronization-primitives.md`.

### Irreducible platform-operation families

The generic registry schema must be able to declare these families. Thermite
does not ship their architecture-specific bodies.

| Family | Minimal frozen operation |
|---|---|
| boot/runtime | normalized handoff borrow, entry transfer, panic/contract/allocation failure, compiler intrinsics |
| provenance | capability-backed raw address conversion, bounded copy/fill, volatile access |
| memory | page-table entry read/write, address-space activate, local invalidation |
| MMIO/PIO | aligned volatile reads/writes by width and device barriers |
| CPU | ID/features, control/MSR access, per-CPU base, pause/halt |
| IRQ/trap | interrupt state token, route/mask/EOI, checked context entry/return |
| SMP | AP start transport, IPI send, online snapshot/acknowledgement transport |
| DMA | device-visible mapping/synchronization mechanics |
| services | monotonic counter/deadline, entropy fill, reboot/poweroff |
| atomics | the sealed operations and fences listed above |

Frame allocation, virtual layout, page-table traversal, shootdown epochs, AP
lifecycle, interrupt policy, DMA protocols, drivers, schedulers, IPC, and
services are consumer-authored Thermite code over these operations.

### Generic freestanding composition

`forge build --target kernel` remains a generic `no_std + alloc` verified
library build. It produces an rlib and a receipt; it does not select firmware,
link a kernel, manufacture a disk image, or run QEMU.

For a consumer project, Forge must be able to:

- compile a receipt-bound multi-file Thermite package;
- compose selected generated exports with consumer-supplied direct-Verus
  platform shells;
- bind the exact compiler, Verus model, target, flags, source closure,
  generated source, shells, registry, linker inputs, and resulting rlib;
- validate and replay the bundle independently; and
- report which boundaries are end-to-end proved, directly refined, or remain
  assumptions.

Final executable and image construction is deliberately consumer-owned.

## Acceptance matrix

The primitive suite is target-independent wherever possible.

- Parse, validate, lower, prove, compile, and replay every primitive in small
  `.th` conformance packages.
- Mutate each operation, contract, ordering, capacity, source file, registry
  entry, shell, and receipt field; the appropriate proof or closure gate fails.
- Prove representative reusable libraries: the fixed bitset, fixed ring,
  generation ledger, bounded-wait scan, ticket lock, participant-aware barrier,
  once, reference-count, seqlock, bounded MPSC queue, and bounded work-stealing
  deque, epoch-acknowledgement, and packed-bitmap mechanics are present.
- Compile those libraries for the generic freestanding target with no hosted
  effects and no concrete platform dependency.
- Compose a synthetic test platform whose bodies are tiny direct-Verus
  adapters. This exercises the registry/refinement machinery without booting
  or implementing a kernel.
- Confirm that the repository contains no bundled kernel policy, firmware
  runtime, architecture boot assembly, or bootable image.

## Implementation order

1. Remove the accidentally bundled kernel and profile-specific image builder.
2. Land receipt-bound packages/modules and canonical transitive closure.
3. Land fixed arrays, static storage, mutable aggregate slices, and fixed
   collection libraries.
4. Land sealed atomic declarations, legal orderings, and direct-refinement
   composition for a synthetic platform.
5. Land explicit waiting/liveness primitives and verified synchronization
   libraries.
6. Complete the generic frozen registry and remaining platform-operation
   declarations.
7. Run the primitive-only adversarial matrix and publish a reusable
   freestanding bundle.

## Requirements

<!-- generated:reqs view=forge-kernel-primitives-status -->
Source: `.design/reqs/registry.toml`

| ID | Status | Owner | Title | Follow-up |
|---|---|---|---|---|
| REQ-KPRIM-1 | shipped | `.design/build/kernel-primitives.md` | Kernel scalar and effect surface |  |
| REQ-KPRIM-10 | not_started | `.design/build/kernel-primitives.md` | Primitive-only adversarial suite | Add package, fixed-storage, atomic, waiting, registry, refinement, receipt-tamper, freestanding-consumer, and no-concrete-kernel gates. |
| REQ-KPRIM-2 | partial | `.design/build/kernel-primitives.md` | Exact mutable and fixed storage | Mutable borrowed slices/arrays, arbitrary old/final snapshot framing, native fixed arrays, scalar extensional equality and same-except update framing, strict public-borrow receipts, exact array TV, total directly proved u64 bit methods including distinct-bit set/clear framing, and a receipt-bound packed bitmap/u64 vector/u64 FIFO-ring/collision-explicit direct-map package are shipped. Add static storage, general named-aggregate borrows, aggregate-element equality, richer collision-resolving maps, slabs/freelists, generic library capacities, and complete aggregate lifecycle TV. |
| REQ-KPRIM-3 | partial | `.design/build/kernel-primitives.md` | Receipt-bound packages and modules | Independent parsing, module-local identity, direct-import/root-export enforcement, rooted graph validation, source allowlisting, L3 build/composition, complete receipt binding, validation, and replay are shipped. Extend the remaining source-oriented Forge commands (check, audit, TV, goal/edit/fill) to operate on packages without losing module-local diagnostics. |
| REQ-KPRIM-4 | partial | `.design/build/kernel-primitives.md` | Sealed authority and ownership | The sealed-construction barrier plus an opaque receipt-bound 64-slot generation ledger now prove acquisition/renewal/release, stale-handle-after-reuse rejection, double-release rejection, monotonic rights, L3 move/clone refusal, foreign-module construction rejection, and a strict replayed scalar surface. Add a complete affine rule if stronger uniqueness is required, named-aggregate/ADT body TV for strict lifecycle replay, exact authority-mint refinement, and atomic-slot integration. |
| REQ-KPRIM-5 | partial | `.design/build/kernel-primitives.md` | Sealed atomics and ordering model | The receipt-bound package, 50 sealed boundary declarations, exact ordering matrix, pre-codegen legality gate, bounded history relations, strict kernel ordering proof, strict hosted history proof, replay, and adversarial tests are present. Add enforceable single-use slot ownership, a kernel-target finite-history proof surface, exact atomic object/machine refinement, and verified synchronization consumers. |
| REQ-KPRIM-6 | partial | `.design/build/kernel-primitives.md` | Verified waiting and synchronization | A receipt-bound bounded-wait trace scan, frozen pause/block/terminal-halt declarations, and fail-closed ticket-lock/barrier/epoch-ack/once/reference-count/seqlock/bounded-MPSC/bounded-work-deque mechanics are shipped with L3 proofs, adversarial claims, strict replay, and tamper rejection. Add registry-level fairness/progress semantics, directly refined wait bodies, atomic integration and machine concurrency composition, then richer reader/writer coordination in .th. |
| REQ-KPRIM-7 | partial | `.design/build/kernel-primitives.md` | Generic frozen boundary registry | Same-crate safe direct-Verus Rust-ABI entries now close reachable boundaries exactly. Add non-empty codegen-feature binding and exact separate Rust/assembly source, object, machine-model, and refinement closure for irreducible operations without adding an architecture operation table. |
| REQ-KPRIM-8 | shipped | `.design/build/kernel-primitives.md` | Generic freestanding verified library build |  |
| REQ-KPRIM-9 | partial | `.design/build/kernel-primitives.md` | Exact platform refinement composition | Safe same-crate direct-Verus operations now receive exact one-to-one checked-wrapper refinement. Add direct machine-operation refinement tied to separate Rust/assembly objects and the atomic/concurrency model before every irreducible platform family is covered. |
<!-- /generated:reqs -->
