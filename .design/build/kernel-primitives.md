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
  - thermite-spec/tests/named_record_mutation_validate.rs
  - thermite-spec/tests/atomic_ordering_validate.rs
  - thermite-spec/tests/u64_bit_methods_validate.rs
  - thermite-lower/src/effects.rs
  - thermite-lower/src/l1.rs
  - thermite-lower/src/lib.rs
  - thermite-lower/src/lower.rs
  - thermite-lower/tests/fixed_array.rs
  - thermite-lower/tests/aggregate_array_relations.rs
  - thermite-lower/tests/named_record_lifecycle.rs
  - thermite-lower/tests/nested_aggregate_lifecycle.rs
  - thermite-lower/tests/kernel_mutable_slice.rs
  - thermite-lower/tests/effects_verified.rs
  - thermite-lower/tests/u64_bit_methods.rs
  - thermite-verified/src/lib.rs
  - thermite-tv/src/exec_stmt_encode.rs
  - thermite-tv/src/exec_encode.rs
  - thermite-tv/src/obligation.rs
  - thermite-tv/src/ref_encode.rs
  - thermite-tv/tests/fixed_array_tv.rs
  - thermite-tv/tests/named_record_lifecycle_tv.rs
  - thermite-tv/tests/owned_aggregate_lifecycle_tv.rs
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
  - forge/tests/fixed_freelist.rs
  - forge/tests/fixed_intrusive.rs
  - forge/tests/fixed_slab.rs
  - forge/tests/ownership_primitives.rs
  - forge/tests/static_storage_primitives.rs
  - forge/tests/synchronization_primitives.rs
  - forge/tests/platform_primitives.rs
  - stdlib/kernel-primitives/collections.thpkg.json
  - stdlib/kernel-primitives/collections/bitmap.th
  - stdlib/kernel-primitives/collections/direct_map.th
  - stdlib/kernel-primitives/collections/freelist.th
  - stdlib/kernel-primitives/collections/intrusive.th
  - stdlib/kernel-primitives/collections/open_map.th
  - stdlib/kernel-primitives/collections/ring.th
  - stdlib/kernel-primitives/collections/slab.th
  - stdlib/kernel-primitives/collections/vector.th
  - stdlib/kernel-primitives/freelist.thpkg.json
  - stdlib/kernel-primitives/intrusive.thpkg.json
  - stdlib/kernel-primitives/slab.thpkg.json
  - stdlib/kernel-primitives/ownership.thpkg.json
  - stdlib/kernel-primitives/ownership/generation.th
  - stdlib/kernel-primitives/static-storage.thpkg.json
  - stdlib/kernel-primitives/storage/static_storage.th
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
  - stdlib/kernel-primitives/platform.thpkg.json
  - stdlib/kernel-primitives/platform/api.th
  - conformance/kernel_primitives.th
  - conformance/verified-build/aggregate_storage.th
  - conformance/verified-build/aggregate_array_relations.th
  - conformance/verified-build/named_record_lifecycle.th
  - conformance/verified-build/owned_aggregate_lifecycle.th
  - conformance/verified-build/nested_aggregate_lifecycle.th
  - conformance/verified-build/projected_indexed_call_effect.th
  - conformance/verified-build/record_after_indexed_call_effect.th
  - conformance/verified-composition/frozen_primitive.th
  - conformance/verified-composition/frozen_primitive_shell.rs
  - conformance/verified-composition/frozen_primitive_registry.json
audited-content-sha256: aa738c1e9d917193cd49dc1575af186e6fe4225e6719201efa75b6be96de5a52 (re-pinned 2026-08-05 after leafwise record-call composition over projected indexed state; no kernel policy or implementation was added)
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

Every Thermite-authored language semantic, model, and reusable algorithm has an
L3-or-L4 assurance floor. L3 means an all-input machine proof; L4 is accepted
only for an admitted decidable route with checked reconstruction. L2, L1, L0,
an unrun proof, or a skipped translation-validation row is not a completed
primitive.

The sole sub-L3 exception class is a bodyless frozen declaration for an
irreducible machine operation whose implementation is deliberately supplied by
a consuming platform, or a hardware/concurrency fact that the current formal
semantics literally cannot express. These declarations must remain visibly
incomplete platform obligations. They acquire end-to-end L3/L4 assurance only
when a consumer binds the exact emitted Rust/assembly/object implementation and
discharges its direct refinement; a source contract or L1 wrapper alone cannot
upgrade them.

A Rust reference model alone is not a completed Thermite primitive. A boundary
declaration without an exact implementation/refinement binding is only a
declaration. A boot test does not substitute for a proof.

The primitive package acceptance gates enumerate complete certificate
inventories, not only representative exports: all collection rows, all
ownership rows except the exact mint declaration, all synchronization rows
except the exact three wait/machine declarations, and all bodyful atomic model
and API rows must be L3. The atomic machine declarations are the exact 50-item
bodyless L1 set. The generic platform package has exactly 74 additional
bodyless L1 machine doors; its other 55 type/model/helper rows are L3. Any
additional sub-L3 row fails the applicable package gate.

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
Exact field writes through `&mut Name` and typed owned locals are admitted for
finite non-sealed records, including `root.field(.field)*` and an optional final
fixed-array index. Every receiver field and target type is resolved before
lowering; index-then-field aliasing, sealed, recursive, heap/reference-bearing,
unknown, computed, dereferenced, and immutable targets fail closed. Defining
modules may use the same primitive for opaque roots, while foreign package
modules may neither construct nor read/write their representation. Contract TV
gives `old(...)` and `final(...)` independent typed snapshots, exec TV covers
structural construction/projection, and body TV recursively reconstructs every
enclosing record plus the exact array update while framing untouched siblings
and dependent write order. Strict opaque direct-field and scalar-nested fixtures
are receipt-replayed and executed from codegen-pinned consumers; hosted real
Verus covers the terminal fixed-array view until that view exists in the
freestanding backend.
Repeat initialization rejects non-copy element types before lowering.
`.array_eq(other)` now provides allocation-free extensional equality for arrays
of scalars and recursively finite plain records, tuples, and fixed arrays, while
`.array_same_except(other, index)` proves exact frame preservation around one
updated index, and `.array_same_except_two(other, first, second)` preserves every
element outside two explicitly named indices. Sealed, opaque, recursive,
reference-bearing, enum, and heap-backed elements fail closed. All three
relations use program-shaped
const-generic scans whose exact generated bodies are verified by Verus and
independently translation-validated through contract, expression, and body TV.
Total `u64.bit_test`, `.bit_set`, and `.bit_clear`
operations bridge finite direct bit-vector proofs into ordinary compositional
L3 contracts. `.bit_set_preserves_other(changed, observed)` and
`.bit_clear_preserves_other(changed, observed)` additionally prove exact
same-word framing for any two distinct in-range indices. All five methods are
independently checked by contract, expression, and body translation validation.
Typed owned finite-record locals now have exact recursive field-by-field body and
aggregate-result expression TV, including terminal fixed-array updates, pure
value-call composition, user-enum match/results with exact payload scope,
record-state loops with recursively exact leaf preservation and full generated
post-state obligations, exact statement-position and direct typed let-bound
result calls over structurally disjoint direct or explicitly borrowed projected
finite-record actuals plus pairwise-distinct direct mutable-slice and
mutable-fixed-array roots, and strict freestanding L3 receipt/runtime fixtures.
Indexed calls thread complete sequence state with exact element/capacity types.
Mixed shared/mutable record and indexed calls additionally snapshot direct,
nominally or structurally exact, nonoverlapping shared roots, including a
separately mutable peer's current state. Explicit `&mut root.field(.field)*` and
`&root.field(.field)*` calls now receive exact nominal resolution, program-point
copy-in, recursive copy-back, sibling framing, and prefix-based alias rejection
for finite-record and fixed-array pointees. Projected indexed copy-back uses an
exact sequence overlay plus recursive leaf equations, so nested array siblings
compose without an unsound `Seq<T>`-to-`[T; N]` conversion; its 51-row strict
freestanding fixture replays and executes at L3. A later direct/projected
mutable record call or shared record snapshot now rebases those overlays
leafwise through the callee and back to the caller; its 59-row strict fixture
replays and executes the generated sequence/record/sequence pipeline at L3.
Static ownership, index-then-field aliasing, array-element-root calls, general
whole-record value/result materialization after a descendant sequence overlay, nested or
otherwise general mutable-call result expressions, enum-payload lvalue mutation, and complete aggregate
transition composition through those remaining forms persist. The
allocation-free collection packages now supply a packed 256-bit `[u64; 4]`
bitmap, 64-entry `u64` vector, FIFO ring, collision-explicit direct map, opaque
linear-probing map, generation-safe slab, duplicate-safe freelist, and opaque
intrusive doubly linked metadata with arbitrary-node unlink and tail relink in
Thermite. Arbitrary-position insertion/relink, workload-driven chained maps,
quantified aggregate-state framing, and generic library capacities/types remain.
See `.design/build/named-record-lifecycle.md` for the exact admitted record
subset and adversarial proof boundary.

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
iterator state. `.len()` is the constant `N`. For arrays whose element belongs
to the finite structural closure—scalars, unit, nested fixed arrays, tuples, and
ordinary acyclic structs composed from those forms—`.array_eq(other)` compares
every element and returns exactly finite-view extensional equality. Sealed and
opaque authority never receive an ambient derived comparator.
`.array_same_except(other, index)` returns true exactly when every in-bounds
element other than `index` agrees; an out-of-bounds exception therefore means
full equality. `.array_same_except_two(other, first, second)` uses the same
allocation-free, quantified semantics while excluding two positions; either
exception may be out of bounds or equal to the other without weakening the
remaining-index equality. These relations combine with exact indexed
postconditions to frame one- and two-write owned array updates compositionally.
Arrays nest, may appear in plain structs/enums/tuples, and may be borrowed as
immutable or mutable arrays.

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

`stdlib/kernel-primitives/collections.thpkg.json` contains five policy-free root
modules. `FixedBitmap256` supplies bounded membership and owned insert/remove/
set-to transitions over packed `[u64; 4]` storage, with exact target-word and
other-word frames. `FixedRing64` supplies explicit full and
empty result variants, FIFO push/pop, modulo wraparound, and returned ownership
over `[u64; 64]`. `FixedVec64` adds bounded random access, replacement, and
owned LIFO push/pop. `FixedDirectMap64` adds deterministic direct-slot lookup,
insert, replacement, and removal while reporting key collisions and invalid
counts as explicit result variants. `FixedOpenMap64` adds opaque linear probing,
collision traversal, and tombstone reuse. Focused sibling packages add an opaque
generation-tagged 64-slot slab, a duplicate-safe freelist, and doubly linked
intrusive metadata with arbitrary-live-node unlink. All 234 collection source
items prove at L3 with no boundary or runtime implementation; their executable
contracts kill 732/779 generated mutants.

The canonical package builds and replays as a strict freestanding receipt rooted
at the scalar ring-index transition, binding all five roots and rejecting
receipt-source tampering. The slab, freelist, and intrusive packages separately
build and replay aggregate kernel-target exports; the intrusive fixture also
executes the generated middle-unlink logic from a downstream consumer and binds
72 faithful reachable translation-validation rows. Typed mutable local records,
pure value calls, exact direct/projected finite-record mutable calls with typed let-bound
results, and user-ADT
match/results now have strict L3 composition. Full collection exports remain
gated by quantified aggregate-state framing and dedicated aggregate
receipt/runtime coverage. The packed
bitmap's finite dynamic-bit bridge is
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
aliases. Strict body TV now frames user-ADT results and matches, direct opaque
constructor/observer/`&mut` record transitions, and exact direct finite-record
mutable-call chains. The generation package uses owned ADT transitions; its
authority-mint boundary still has no package-owned
implementation/refinement.
The exact construction semantics are in
`.design/build/opaque-library-state.md`; the ownership claim and residual trust
are in `.design/build/generation-ownership.md`.

The receipt-bound `stdlib/kernel-primitives/static-storage.thpkg.json` package
now supplies the allocation-free static-storage protocol. Its opaque 64-slot
ledger claims a nonzero-capacity slot under a sealed authority, rotates a
nonwrapping generation, issues an uninitialized lease, consumes a sealed fill
witness, commits an initialized opaque region, and permits release only before
commit. Reuse invalidates stale leases, zero capacity and exhausted generations
fail closed, and foreign modules cannot construct the ledger, lease, or region.
All 31 in-language rows are L3 and all 84 executable mutants are killed. The
only two L1 rows are the bodyless authority and physical byte-fill declarations;
their exact static object and emitted implementation remain consumer-owned
direct-refinement obligations. The complete protocol and assurance split are in
`.design/build/static-storage.md`.

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

The first two registry increments are shipped for exact same-crate and
separately emitted safe sequential direct-Verus functions.
`--primitive-registry` binds a strict versioned JSON document to an
L3 composition build; Forge independently reconstructs signature, contract,
effects, ownership, shell inventory, source digest, symbol, ABI, alignment,
concurrency/failure declarations, and one-to-one reachability. Registered
boundaries lower to real checked wrapper calls, never `external_body`, and the
final `--no-cheating` caller proof must establish their Thermite contracts.
Registry v2 additionally binds a separate authored/generated source, exported
proof interface, rlib, every object-member digest, and its own no-cheating
proof/codegen. Receipts and replay bind both layers. Signature/effect/contract/
source/ownership/object drift, missing or extra reachable bindings, post-plan
mutation, receipt tampering, and a lying implementation all reject without
publication.

The v1/v2 paths intentionally accept only the Rust ABI, safe direct-Verus
bodies, and `sequential` concurrency. Sorted canonical target features are
bound into the frozen plan and supplied to the exact proof/codegen and replay
commands. It rejects otherwise well-formed `atomic`, `volatile`, and
`privileged` entries because safe-Rust source/object closure does not model
their machine semantics. Exact unsafe Rust/assembly object closure and direct
machine refinement for irreducible instructions remain; they cannot be claimed
through these schemas. The precise contract and limitations are in
`.design/build/frozen-primitive-registry.md`.

Registry v3 adds one honest machine-aware atomic vertical slice: a canonical
`PAtomicU64` SeqCst create/load adapter whose bodyful wrapper verifies at L3
relative to the exact pinned vstd permission model. The receipt binds the vstd
atomic source/full rlib, adapter source/interface/rlib/object, target features,
and both proof layers. It separately reports three residual machine assumptions
and caps the aggregate at `L1/to_machine_boundary`; it does not claim that
Verus's trusted atomic body, Rust/LLVM codegen, or hardware memory model was
proved. Persistent sealed-cell ABI composition and the remaining operation
matrix are still open.

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
`stdlib/kernel-primitives/atomics.thpkg.json`. It contains 50 bodyful L3
application operations over sealed initialization-slot and cell-handle types,
plus exactly 50 bodyless scalar/tuple machine doors in `machine.th`. The L3
layer constructs explicit observation/transition witnesses, proves exact
wrapping fetch arithmetic, and preserves the machine handle and compound
authority/slot/generation identity. The machine layer is the literal L1
exception and contains no platform implementation. The package also contains
the complete 45-case ordering table and a bounded 256-event history model. The
validator derives the canonical application operations from exact
`#[boundary("thermite::atomic::...")]` targets and requires literal application
ordering variants; dynamic, malformed, wrong-arity, and illegal pairs fail
before lowering, while the internal machine call receives the proved `u8`
code. An unrelated function with an atomic-looking source name is not special.

The executable ordering matrix builds and replays at strict L3 for the generic
freestanding target. The fixed-array history relations build and replay at
strict L3 for the hosted target because the ordinary kernel-target path uses a
minimal vstd slice/array model. Registry v3 separately binds the full pinned
vstd model only for its machine-aware adapter. These are distinct proof
surfaces over the same receipt-bound package,
not a claim that hosted evidence proves a machine atomic implementation. The
consumer still owes exact object/machine semantics and direct refinement for
every real atomic machine door. Safe v1/v2 linkages refuse to overstate that
missing assurance, while the v3 pilot remains explicitly residual. Slot
single-use is enforced for the shipped typed slot path by move semantics and
the generation-bound storage protocol; a stronger general affine rule remains
open for other authority types.

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

The receipt-bound `stdlib/kernel-primitives/platform.thpkg.json` package now
ships the policy-free semantic declarations for these families. Its 74 exact
machine-facing rows are bodyless L1 boundaries because Thermite does not ship
their architecture-specific bodies. All 55 sealed-type, observation-type,
specification, and executable legality rows prove at L3; the width/range/
alignment helpers kill 38/38 generated mutants. A strict freestanding receipt
binds and replays the complete package module while exporting only an L3 helper,
and a false width claim is rejected. Atomics remain in their dedicated package,
and pause/block/halt remain in the synchronization package rather than being
duplicated here.

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

The exact inventory and consumer-refinement rule are in
`.design/build/platform-primitives.md`.

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
- Prove and independently translation-validate equality and same-except framing
  over nested plain-record arrays, while rejecting sealed, opaque, recursive,
  enum, reference-bearing, and heap-backed element shapes.
- Compile those libraries for the generic freestanding target with no hosted
  effects and no concrete platform dependency.
- Require every non-boundary platform-package row at L3 and pin the 74-row
  bodyless machine exception exactly by family, effect, and target.
- Compose a synthetic test platform whose bodies are tiny direct-Verus
  adapters. This exercises the registry/refinement machinery without booting
  or implementing a kernel.
- Confirm that the repository contains no bundled kernel policy, firmware
  runtime, architecture boot assembly, or bootable image. The permanent
  `tooling/primitive-only-gate.py` check now enumerates the canonical Git-index
  source set, rejects those concrete paths and artifacts, pins the only two
  compile-only freestanding fixtures by digest, and runs in CI. Untracked local
  output is deliberately outside the committed-source claim.

## Implementation order

1. Remove the accidentally bundled kernel and profile-specific image builder.
2. Land receipt-bound packages/modules and canonical transitive closure.
3. Land fixed arrays, static storage, mutable aggregate slices, and fixed
   collection libraries.
4. Land sealed atomic declarations, legal orderings, and direct-refinement
   composition for a synthetic platform.
5. Land explicit waiting/liveness primitives and verified synchronization
   libraries.
6. Land the generic platform-operation declarations, then complete their
   unsafe-Rust/assembly/object and machine-refinement registry path.
7. Consolidate the now-enforced no-concrete-kernel gate and existing primitive
   adversarial suites into one matrix, then publish a reusable freestanding
   bundle.

## Requirements

<!-- generated:reqs view=forge-kernel-primitives-status -->
Source: `.design/reqs/registry.toml`

| ID | Status | Owner | Title | Follow-up |
|---|---|---|---|---|
| REQ-KPRIM-1 | shipped | `.design/build/kernel-primitives.md` | Kernel scalar and effect surface |  |
| REQ-KPRIM-10 | partial | `.design/build/kernel-primitives.md` | Primitive-only adversarial suite | The canonical tracked-source gate now rejects concrete kernel/firmware/boot/image directories, release and machine artifacts, generated trees, binary or nonlocal source closure, and hidden freestanding entries; exact compile-only freestanding fixtures are digest-pinned, CI-enforced, and adversarially tested. The platform suite also pins its exact 74-row bodyless machine exception and 55-row L3 floor. Consolidate the existing package, fixed-storage, atomic, waiting, platform, registry, refinement, receipt-tamper, and freestanding-consumer suites into one primitive-only matrix, then publish the reusable bundle. |
| REQ-KPRIM-2 | partial | `.design/build/kernel-primitives.md` | Exact mutable and fixed storage | Mutable borrowed slices/arrays, arbitrary old/final snapshot framing, native fixed arrays, scalar and recursively finite plain-aggregate equality plus exact one- and two-index quantified frames, defining-module opaque state transitions, exact typed root.field(.field)* mutation with an optional final fixed-array index, owned/value-call composition, exact user-ADT result/match contract and body TV with arm scoping, exact record-state loop entry/leaf-preservation/exit/full-result TV, statement-position and direct typed let-bound result mutable-call composition over structurally disjoint direct or explicitly borrowed projected finite-record and indexed-storage actuals with nonoverlapping nominally and structurally exact shared snapshots, leafwise record-formal rebasing after descendant sequence overlays, strict freestanding L3 record/rich-state receipts, digest-bound freestanding fixed-array views and repeat construction, total directly proved u64 bit methods, a receipt-bound packed bitmap with population count/set-bit search/bulk set operations, u64 vector, u64 FIFO ring, collision-explicit direct map, opaque collision-resolving open-addressed map, a generation-safe opaque slab, a duplicate-safe freelist, opaque intrusive doubly linked metadata with arbitrary-live-node unlink and tail relink, and a receipt-bound generation-owned static-storage claim/fill/commit/release protocol are shipped. Add index-then-field aliasing if required, array-element-root calls, general whole-record value/result materialization after descendant sequence overlays, nested/general mutable-call result expressions, enum-payload lvalue mutation, arbitrary-position intrusive insertion/relink if required, chained maps where required, quantified aggregate loops/state framing, and generic library capacities/types. |
| REQ-KPRIM-3 | partial | `.design/build/kernel-primitives.md` | Receipt-bound packages and modules | Independent parsing, module-local identity, direct-import/root-export enforcement, rooted graph validation, opaque construction/read/write ownership, source allowlisting, L3 build/composition, complete receipt binding, validation, and replay are shipped. Extend the remaining source-oriented Forge commands (check, audit, TV, goal/edit/fill) to operate on packages without losing module-local diagnostics. |
| REQ-KPRIM-4 | partial | `.design/build/kernel-primitives.md` | Sealed authority and ownership | The sealed-construction barrier now includes an explicit bodyful verified-factory form while bare seals remain boundary-only. Direct and nested opaque lifecycle receipts, typed owned-record local/value-call L3 receipts, exact user-ADT result/match TV, exact direct and projected finite-record mutable-call effects with structural alias rejection, an opaque receipt-bound 64-slot generation ledger, a generation-bound 64-slot static-storage lease/region protocol, and typed opaque single-use atomic-init slots prove acquisition/renewal/release, stale-token-after-reuse rejection, double-release rejection, monotonic rights, L3 move/clone refusal, initialization-witness matching, committed-region consumption, exact authority/slot/generation transfer, duplicate-init refusal, and foreign-module construction/read/write rejection. Add a complete affine rule if stronger general-purpose uniqueness is required, strictly compose the full owned-ADT lifecycles through exactly refined authority/memory/atomic doors, and add concurrent synchronization consumers that rotate generations through exact atomic transitions. |
| REQ-KPRIM-5 | partial | `.design/build/kernel-primitives.md` | Sealed atomics and ordering model | The six-module receipt-bound package, typed generation-bound storage-to-slot conversion, enforceable single-use initialization, compound atomic identity, 50 bodyful L3 application operations, 50 non-vacuous scalar/tuple L1 machine doors, exact ordering-code preconditions, persistent handle/identity echoes, exact ordering matrix, pre-codegen legality gate, bounded history relations, strict kernel ordering/storage proofs, strict hosted history proof, replay, runtime generated-logic execution, and adversarial tests are present. Registry v3 additionally proves and executes a canonical PAtomicU64 SeqCst roundtrip adapter while retaining three explicit L1 machine assumptions. Add a kernel-target finite-history proof surface, exact object/model refinement of the full persistent machine ABI and ordering matrix, and verified synchronization consumers. |
| REQ-KPRIM-6 | partial | `.design/build/kernel-primitives.md` | Verified waiting and synchronization | A receipt-bound bounded-wait trace scan, frozen pause/block/terminal-halt declarations, and fail-closed ticket-lock/barrier/epoch-ack/once/reference-count/seqlock/bounded-MPSC/bounded-work-deque mechanics are shipped with L3 proofs, adversarial claims, strict replay, and tamper rejection. Add registry-level fairness/progress semantics, directly refined wait bodies, atomic integration and machine concurrency composition, then richer reader/writer coordination in .th. |
| REQ-KPRIM-7 | partial | `.design/build/kernel-primitives.md` | Generic frozen boundary registry | Same-crate and separately emitted safe sequential direct-Verus Rust-ABI entries now close reachable boundaries exactly. The v2 path binds authored and generated sources, exported proof interface, rlib, every object-member digest, target features, two no-cheating proof/codegen layers, validation, runtime linking, and replay. Registry v3 adds an honestly capped atomic machine-model pilot with ten discharged wrapper obligations and three explicit residual assumptions. A policy-free package supplies 74 frozen declarations across boot/runtime, memory/provenance, MMIO/PIO, CPU, IRQ/trap, SMP, DMA, clock, entropy, and power while keeping all 55 non-machine rows at L3. Extend v3 to shared sealed ABIs, the full atomic matrix, assembly and unsafe/irreducible Rust source/object closure, volatile, privileged, and concurrent models without adding an architecture operation table. |
| REQ-KPRIM-8 | shipped | `.design/build/kernel-primitives.md` | Generic freestanding verified library build |  |
| REQ-KPRIM-9 | partial | `.design/build/kernel-primitives.md` | Exact platform refinement composition | Safe same-crate and separately emitted sequential direct-Verus operations receive exact checked-wrapper/import refinement through their emitted Rust objects. The registry-v3 atomic pilot proves an exact checked adapter relative to pinned vstd and binds the emitted object while exposing the literal hardware/codegen trust cap. Extend this honest split to persistent sealed atomic ABIs, unsafe Rust/assembly objects, volatile and privileged operations, and concurrency models before every irreducible platform family is covered. |
<!-- /generated:reqs -->
