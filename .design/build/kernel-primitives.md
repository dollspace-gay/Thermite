# Thermite kernel-authoring primitives

<!--
tier: 3-component
status: partial
decision: Thermite ships reusable verified language, library, boundary, and build primitives; kernels and platform implementations live in consumer projects
governs:
  - THERMITE.skill.md
  - thermite-syntax/src/ast.rs
  - thermite-syntax/src/parser.rs
  - thermite-syntax/src/lib.rs
  - thermite-syntax/tests/kernel_surface.rs
  - thermite-syntax/tests/conformance.rs
  - thermite-spec/src/validator.rs
  - thermite-lower/src/effects.rs
  - thermite-lower/src/lower.rs
  - thermite-lower/tests/kernel_mutable_slice.rs
  - thermite-lower/tests/effects_verified.rs
  - thermite-verified/src/lib.rs
  - lean/Thermite/Ast.lean
  - lean/Thermite/Denote.lean
  - lean/Thermite/RefEncode.lean
  - forge/src/build.rs
  - forge/src/verified_build.rs
  - forge/src/verified_build/composition.rs
  - conformance/kernel_primitives.th
audited-content-sha256: fca7272907810874994ece528708fec2b2879a60ec61b8c82db7fe2db1ef6af1
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
sealed structs, mutable byte-slice assignment, and a `final(slice)` proof view.
It does not yet have a first-class fixed-array/static-storage surface or the
allocation-free collection library.

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

Ticket locks, spin locks, once cells, barriers, reference counts, seqlocks,
bounded MPSC queues, and work-stealing deque mechanics are verified `.th`
libraries built from the atomic and waiting primitives. They are not Rust
kernel implementations and not privileged boundaries.

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
- Prove representative reusable libraries: a fixed bitmap, fixed ring,
  ticket lock, once cell, bounded MPSC queue, generation ledger, and
  epoch-acknowledgement set.
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
| REQ-KPRIM-2 | partial | `.design/build/kernel-primitives.md` | Exact mutable and fixed storage | Mutable byte-slice assignment with final(slice) is shipped. First-class fixed arrays, static storage, mutable aggregate slices, and verified fixed-capacity vector/map/bitmap/ring libraries remain. |
| REQ-KPRIM-3 | not_started | `.design/build/kernel-primitives.md` | Receipt-bound packages and modules | Implement multi-file parsing/name resolution, deterministic diagnostics, canonical allowlists, closure receipts, validation, and replay. |
| REQ-KPRIM-4 | partial | `.design/build/kernel-primitives.md` | Sealed authority and ownership | The sealed-construction barrier is shipped. Add affine-style consumption or a verified generation discipline that rejects stale copies, double release, and rights escalation. |
| REQ-KPRIM-5 | not_started | `.design/build/kernel-primitives.md` | Sealed atomics and ordering model | Add the Thermite surface, legality validation, happens-before model, direct-refinement interface, and adversarial tests without adding a kernel implementation. |
| REQ-KPRIM-6 | not_started | `.design/build/kernel-primitives.md` | Verified waiting and synchronization | Add the wait/liveness surface and implement ticket locks, once cells, barriers, bounded queues, reference counts, seqlocks, and deque mechanics in .th. |
| REQ-KPRIM-7 | not_started | `.design/build/kernel-primitives.md` | Generic frozen boundary registry | Implement a target-independent registry schema and fail-closed closure validator; do not hard-code an architecture profile or operation table. |
| REQ-KPRIM-8 | shipped | `.design/build/kernel-primitives.md` | Generic freestanding verified library build |  |
| REQ-KPRIM-9 | partial | `.design/build/kernel-primitives.md` | Exact platform refinement composition | Rich-state composition is shipped, but generic registry-to-implementation one-to-one binding and exact refinement evidence for every reachable operation remain. |
<!-- /generated:reqs -->
