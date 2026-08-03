# Bootable multicore kernel platform

<!--
tier: 3-component
status: migration-in-progress
decision: a platform/conformance demonstration is retained while kernel policy migrates from Rust into receipt-bound Thermite
governs:
  - Cargo.toml
  - Cargo.lock
  - THERMITE.skill.md
  - thermite-syntax/src/ast.rs
  - thermite-syntax/src/parser.rs
  - thermite-syntax/src/lib.rs
  - thermite-syntax/tests/kernel_surface.rs
  - thermite-syntax/tests/conformance.rs
  - thermite-lower/src/effects.rs
  - thermite-lower/src/lower.rs
  - thermite-lower/tests/kernel_mutable_slice.rs
  - thermite-lower/tests/effects_verified.rs
  - thermite-verified/src/lib.rs
  - lean/Thermite/Ast.lean
  - lean/Thermite/Denote.lean
  - lean/Thermite/RefEncode.lean
  - thermite-kernel/Cargo.toml
  - thermite-kernel/src/*.rs
  - thermite-kernel/tests/*.rs
  - forge/Cargo.toml
  - forge/src/kernel_image.rs
  - forge/src/cli.rs
  - forge/src/main.rs
  - conformance/bootable_kernel.th
  - conformance/kernel_primitives.th
  - conformance/thermite-kernel.thpkg.json
  - platform/x86_64-pc-uefi-smp-v1/*
  - platform/x86_64-pc-uefi-smp-v1/runtime/.cargo/config.toml
  - platform/x86_64-pc-uefi-smp-v1/runtime/Cargo.*
  - platform/x86_64-pc-uefi-smp-v1/runtime/src/*
  - .github/workflows/ci.yml
audited-content-sha256: 9ceeb3590bc4a0105562c306cb09316fed5af0343539759eb1c1866a4a509881
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

## Summary

The current artifact is a bootable, reproducible multicore
**platform/conformance demonstration**. It is not yet the complete
Thermite-authored formally verified kernel described by this document. The
image proves that generated Thermite logic can be verified for the exact UEFI
target, linked into the boot image, and executed across the SMP matrix. Much of
the remaining kernel behavior is still ordinary Rust and assembly.

The first conformance profile is `x86_64-pc-uefi-smp-v1`. It produces a UEFI
disk image that can boot on a PC-class machine and under QEMU/OVMF. Its release
gate boots with 1, 2, 4, and 8 logical CPUs. The 4-CPU run is the normative SMP
acceptance case. Every discovered application processor must reach a named
online or failed state, and useful kernel work must execute concurrently on
more than one online CPU.

The image publisher checks the staged boot binary for the stable PE32+, EFI
application, and x86-64 fields reported by `file`; it does not depend on the
distribution-specific prose used by a particular libmagic release.

The QEMU gate accepts the paired legacy 2 MiB and current 4 MiB OVMF package
layouts. Code and variable firmware names are selected as a matched pair; the
harness never combines incompatible firmware sizes.

Ordinary Thermite code retains the existing surface. It has no raw pointers,
inline assembly, arbitrary `extern` declarations, or general unsafe escape.
Privileged operations are frozen functions written as
`#[boundary("kernel::...")]`; authority values are `#[sealed]` types. A final
image accepts only boundary declarations that exactly match its platform
registry. The privileged Rust and assembly bodies are part of the explicit
target platform layer (TPL) and trust boundary.

## Product and assurance boundary

The target architecture has four layers. Only the first vertical slice of the
first layer is implemented in Thermite today:

| Layer | Contents | Assurance |
|---|---|---|
| generated kernel slice | capability ledger, scheduler/IPC/event policy, plus live allocator first-fit/CAS claim orchestration, page-entry flags, exact bounded APIC-ID set validation, xAPIC MSR interpretation, scheduler-completion, shootdown, DMA, and service verdicts | exact-source L3 Thermite/direct-Verus composition, compiled for `x86_64-unknown-uefi` |
| remaining kernel logic | allocator reclaim checks, page-table traversal/writes, scheduler orchestration, synchronization, AP/shootdown execution, DMA setup, drivers, and protocols | ordinary Rust today; must migrate to Thermite; allocation claim and page-entry flag policy are generated Thermite |
| target platform layer | entry stubs, context/trap assembly, privileged instructions, atomics, MMIO/PIO, page-table and CPU control adapters | reachable atomic load, store, fetch, and strong compare-exchange carry exact direct-Verus refinement; raw interrupt-assembly atomics and other reachable machine operations remain source-bound/runtime-tested and must migrate to the same proof rule |
| image closure | compiler runtime, linker, boot adapter, firmware ABI, image packager | digest-bound build evidence and runtime acceptance |

The final receipt reports the artifact class as
`platform_conformance_demonstration`, `migration_complete=false`, and the scope
as `platform_conformance_to_boundary`. It may
also report the L3 result for the verified core. It cannot describe the whole
image as end-to-end L3 while registered platform calls remain boundary
assumptions. A TPL implementation can carry stronger evidence, but that
evidence is recorded separately and does not change the Thermite assurance
ladder.

The hardware, firmware, compiler backend, linker, and not-yet-directly-proved
TPL bodies remain
in the trusted computing base. This limitation is stated in the receipt and in
`forge audit` output.

### Paired proof and implementation closures

`kernel-image` preserves the strict L3 artifact policy. The current strict
`VerifiedCompositionReceiptV1` path continues to reject every boundary and to
mean end-to-end L3 for its complete rlib closure.

The image builder constructs a separate pair of closures from the same roots:

1. A standalone source check records a declared frozen boundary at L1 and its
   callers at L3-to-boundary. In a kernel composition, however, the lowerer
   emits an ordinary checked call to the exact TPL module rather than an
   `external_body`. The whole combined crate proves that Thermite caller and
   emitted implementation together.
2. The implementation closure resolves that declaration to exactly one frozen
   registry entry and one checked public symbol in an `exact_tpl_v1` shell.
   The registry signature, contract, model, effects, ABI, object, and final PE
   symbol must agree with the proof closure. Ordinary checked shells cannot
   claim direct refinement.

The receipt binds both inventories and their one-to-one correspondence. Slag
is excluded from an image closure. A declared boundary without a frozen entry,
or a source-reachable registry entry without a reachable declaration, is a
build error. Implementation-only entries are closed through the bound safe
model and TPL source inventories and are not fabricated as Thermite
declarations. This keeps the existing honest L3-to-boundary semantics while
adding a concrete, auditable implementation for every platform assumption.

## Selected architecture

Kernel policy is expressed as deterministic state transitions. Each CPU owns a
`CpuShard`, receives a `PlatformEvent`, and produces a new shard plus a bounded
sequence of `PlatformAction` values. Global objects are reached through
ownership transfer, verified atomic structures, or verified locks. The design
does not place all kernel state behind one global scheduler lock.

The TPL has three duties:

1. The ingress side normalizes boot, interrupt, trap, timer, IPI, DMA, and
   syscall inputs into typed events.
2. The executor validates capabilities and performs actions such as mapping,
   interrupt acknowledgement, AP startup, IPI delivery, device access, DMA,
   timer programming, context entry, and power control.
3. The concurrency side supplies the small synchronous operations that cannot
   be represented as delayed actions: atomics, fences, CPU-local interrupt
   state, CPU-local storage selection, pause/halt, and trap/context return.

Drivers, allocators, schedulers, synchronization algorithms, ACPI and device
table parsers, syscall policy, and recovery logic stay in Thermite. The TPL
contains only operations that intrinsically require a privileged instruction,
volatile or physical access, a foreign ABI, entry/return assembly, or a
compiler runtime hook.

## Current migration slice and non-claims

`conformance/thermite-kernel.thpkg.json` is the receipt-bound multi-file package
for the migration. Its executable functions implement capability
mint/transfer/authorization, bounded scheduler transitions, correlated
IPC/event dispatch, and live acceptance predicates for allocator ownership,
AP failure and online sets, scheduler completion, shootdowns, DMA completion,
and user-service completion. The complete one-to-eight-entry APIC-ID fold now
executes in generated Thermite, with an exact recursive-free bounded
specification that rejects IDs above 63 and duplicates. Thermite also decides
whether the architectural APIC-base MSR selects supported xAPIC mode and
extracts the physical LAPIC base. Forge lowers
that closure together with the directly verified ingress module, verifies the
exact combined source without cheating, and compiles the same source for
`x86_64-unknown-uefi`. The UEFI runtime links that rlib and emits a semantic
signature derived from the online CPU count. The generated scheduler decision
is decoded as the base task identifier for the real post-firmware multicore
work queue. The QEMU gate checks both the distinct 1, 2, 4, and 8 CPU
signatures and the resulting task sum, so changing the Thermite operation
changes runtime behavior rather than only changing a marker.

The earlier Rust capability/scheduler/event model probe and its runtime crate
dependency are removed. The booted runtime now calls the generated Thermite
verdicts on observed machine state; changing one changes acceptance rather than
only a serial marker. The remaining post-firmware Rust still contains
substantial page-table traversal and writes, scheduling/synchronization orchestration,
AP/shootdown execution, DMA setup, device, and protocol behavior. Those are
conformance implementations, not proof that the complete kernel is
Thermite-authored.

The receipt publishes conservative, reproducible authorship metrics. The
ordinary-Rust kernel-logic target is zero; it remains explicitly unmet. Runtime
Rust and Rust/assembly TPL metrics are overlapping upper bounds until every
remaining function is mechanically classified as either policy or an
irreducible platform operation.

The latest canonical receipt for this slice reports 1,717 Thermite LOC across
117 functions, 792 direct composition-Verus LOC across 74 discharged function
obligations, 232 exact-TPL Verus LOC across 16 discharged function obligations,
a 3,049 LOC Rust/assembly TPL upper bound, and a 2,950 LOC ordinary-Rust
kernel-logic upper bound. It declares nine platform boundaries, reaches four,
and records all four as exact L3 direct-Verus refinements. The overlapping Rust
upper bounds are intentionally conservative; the ordinary-Rust target remains
zero and remains unmet.

## Frozen platform registry

Each platform profile is a canonical registry plus an exact set of source
files. A registry entry contains:

- semantic name and registry schema version;
- target profile and architecture feature set;
- exact Thermite signature and contract digest;
- closed platform-effect domain;
- required capability kinds, rights, alignment, and ownership transition;
- executable Rust symbol, calling convention, and assembly source when used;
- pure Verus model or state-transition relation used by callers;
- concurrency semantics, legal atomic orderings, and interrupt context rules;
- implementation, target identity, compiler, and toolchain digests;
- review, test, proof, and architecture-manual evidence references; and
- failure behavior, including whether failure is returned, terminal, or a
  platform fault.

Registry names use the `kernel::<domain>::<operation>@v1` form. A source-level
boundary path is complete only when name, version, signature, contract, effect,
and capability transition all match one entry. Unknown names, duplicate names,
weaker contracts, extra effects, ABI mismatches, implementation drift, or
unbound assembly reject the build before final linking.

Forge constructs one transitive boundary inventory from every image root. It
rejects an arbitrary `#[boundary]` even when that function could compile and
link. This image rule is stricter than the general L1 library rule and leaves
ordinary non-kernel boundary use unchanged.

## Capabilities and effects

All physical authority crosses the platform boundary as a sealed capability.
The initial vocabulary is:

| Capability | Authority represented |
|---|---|
| `BootInfo` | one normalized firmware handoff and its immutable byte regions |
| `CpuCap` / `CpuSetCap` | one logical CPU or a bounded set of discovered CPUs |
| `CpuLocalCap` | one CPU's local storage and stack domain |
| `PhysRegionCap` / `FrameCap` | aligned physical storage with explicit ownership |
| `VirtRegionCap` / `AddressSpaceCap` | virtual ranges and page-table roots |
| `MmioCap` / `IoPortCap` | width, range, ordering, and device access rights |
| `IrqCap` / `IrqStateCap` | interrupt route and CPU-local mask state |
| `TrapFrameCap` / `UserContextCap` | architecture register state at a privilege transition |
| `DmaCap` / `IommuDomainCap` | pinned device-visible memory and translation domain |
| `ClockCap`, `EntropyCap`, `PowerCap` | explicit clock, entropy, and machine control authority |

`#[sealed]` prevents construction in ordinary Thermite. The TPL also maintains
a capability ledger keyed by kind, slot, owner, rights, and generation. Reads
may preserve a shared capability. An ownership-changing operation consumes the
current generation and returns the next generation. A copied stale value,
foreign owner, rights escalation, range overflow, or double release returns a
typed platform error and performs no operation. This ledger supplies dynamic
uniqueness until the language has affine types.

The effect system gains one parameterized family:

```text
platform(boot)    platform(memory)  platform(mmio)
platform(pio)     platform(irq)     platform(cpu)
platform(atomic)  platform(smp)     platform(dma)
platform(clock)   platform(entropy) platform(power)
```

These are closed effect atoms and participate in the existing caller
subsumption rule. A registry entry declares exactly one primary domain and any
secondary domains. The image manifest records the transitive set for every
root. Existing hosted `read`, `write`, `net`, `time`, `rand`, and `term` effects
remain invalid for a kernel target.

## Required frozen primitives

The entries below form the minimum complete registry. Names show semantic
families; the registry contains monomorphic width- and architecture-specific
entries where the implementation ABI needs them.

### Image, boot, and compiler runtime

| Family | Required operation and contract |
|---|---|
| entry | BSP `_start`, AP trampoline, stack selection, zeroed BSS, and one call into the checked ingress ABI |
| handoff | borrow normalized memory-map, ACPI RSDP, framebuffer, command-line, initrd, image, and firmware-table bytes with exact bounds |
| runtime | panic, contract-violation, allocation-failure, stack-probe, and terminal-fault handlers |
| memory intrinsics | exact, overlap-aware `memcpy`, `memmove`, `memset`, and integer helper symbols selected by compiler codegen |
| image | frozen target identity, linker and section policy, relocation policy, boot metadata, and deterministic disk-image packaging |

Boot code mints the root capabilities once. Firmware pointers are converted to
bounded immutable bytes or sealed regions before the verified core sees them.
The TPL does not expose a general pointer-to-integer or integer-to-pointer
conversion.

### Physical and virtual memory

| Family | Required operation and contract |
|---|---|
| frames | split, join, zero, lend, reclaim, and query aligned physical frame runs |
| temporary map | map a capability-backed physical run into a bounded kernel window and revoke it by generation |
| address spaces | create, map, unmap, protect, inspect, activate, and destroy a page-table root |
| translation | architecture-level page-size, permission, cache-policy, and canonical-address validation |
| TLB | local invalidate, address-space invalidate, and epoch-based remote shootdown |
| user copy | bounded copy to/from a user address space with a typed fault result and no partial-success ambiguity |

Frame allocation, virtual layout, replacement policy, and page-fault policy are
verified Thermite code. The TPL performs the target page-table writes and
invalidations described by those policies.

### Volatile device access

| Family | Required operation and contract |
|---|---|
| MMIO | aligned volatile read/write for 8, 16, 32, and 64 bits within an `MmioCap` |
| PIO | x86 port read/write for 8, 16, and 32 bits within an `IoPortCap` |
| ordering | compiler fence, device read barrier, device write barrier, and full device barrier |
| discovery | capability-bounded PCI configuration access and feature-query records |

Device register protocols, PCI enumeration, virtio queues, console, block, and
network drivers are Thermite programs over these calls. Each device profile
states its endianness, alignment, volatility, and DMA coherence rules.

### CPU, interrupts, traps, and user mode

| Family | Required operation and contract |
|---|---|
| CPU query | logical ID, feature bits, control-register state, model-specific register access, and supported page/atomic widths |
| descriptors | install GDT, IDT, TSS, syscall entry, and per-CPU bases from checked descriptors |
| IRQ state | CPU-local save-and-disable, restore with matching CPU/generation, route, mask, unmask, and end-of-interrupt |
| execution | `pause`, interrupt-aware `halt`, terminal stop, and bounded architecture rendezvous |
| traps | checked trap-frame construction, interrupt/syscall ingress, context switch, return to kernel, and return to user mode |

The verified scheduler chooses a runnable context and proves its ownership
transition. The frozen context operation changes registers, stack, address
space, and privilege level exactly as that transition requests.

### Atomics and the SMP memory model

The atomic surface is monomorphic over `bool`, `u32`, `u64`, and `usize`. It
provides `load`, `store`, `swap`, strong and weak `compare_exchange`, and the
integer `fetch_add`, `fetch_sub`, `fetch_and`, `fetch_or`, and `fetch_xor`
operations. Atomic objects are aligned sealed cells created from static storage
or a uniquely owned region.

The ordering enum is frozen as `Relaxed`, `Acquire`, `Release`, `AcqRel`, and
`SeqCst`. The validator enforces operation-specific legality, including the
failure ordering of compare-exchange. The proof model records modification
order, reads-from, happens-before, and sequentially consistent order. Compiler
and hardware fences are separate registered operations.

Shared mutable kernel state follows three admitted patterns:

1. one CPU owns and mutates a CPU-local shard;
2. ownership moves through a verified atomic queue or generation transition;
3. a verified lock protects the state and its invariant.

Spinlocks, ticket locks, once cells, reference counts, barriers, queues, and
seqlocks are verified Thermite library code built from the atomic primitives.
Interrupt masking is CPU-local and does not substitute for synchronization
between CPUs. Non-atomic concurrent mutation cannot enter an L3 image closure.

### Multicore lifecycle and coordination

| Family | Required operation and contract |
|---|---|
| discovery | enumerate stable CPU IDs and topology into a `CpuSetCap` |
| AP startup | reserve stack/per-CPU storage, install the trampoline, send startup, and report `Online` or a named `Failed` reason |
| per-CPU state | install and query the current CPU's local base under its `CpuLocalCap` |
| IPI | send one vector to one CPU or capability-bounded set and correlate delivery with an epoch |
| rendezvous | barrier, stop request, online-mask snapshot, and acknowledgement bitmap |
| shootdown | publish a mapping epoch, send IPIs, invalidate locally, and complete after every online target acknowledges |

The AP state machine is
`Discovered -> Prepared -> Starting -> Online | Failed`. Every transition is
monotonic and has one owner. A CPU enters the scheduler only after its local
stack, descriptor state, interrupt controller, and idle task are installed.
The BSP can continue with a policy-approved subset after named failures; it
cannot count an unacknowledged CPU as online.

TLB shootdown uses a monotonically increasing epoch. The initiator publishes
the mapping change with release ordering, snapshots the online target set,
sends the IPI, and waits for acquire-observed acknowledgements for the same
epoch. CPU removal and failure update the online set through the rendezvous
protocol, so an old acknowledgement cannot complete a new shootdown.

### DMA, time, entropy, and power

| Family | Required operation and contract |
|---|---|
| DMA | pin, map into an IOMMU domain when present, obtain device-visible segments, synchronize ownership, unmap, and unpin |
| clock | read a monotonic counter with scale/error metadata and arm or cancel a per-CPU deadline |
| entropy | fill an exact bounded region and report source/health failure explicitly |
| power | reboot, power off, and terminal halt through a `PowerCap` |

DMA capabilities bind device, region, direction, coherence, and generation. A
device cannot receive an arbitrary physical address. Drivers own descriptor
and buffer protocols; the TPL owns cache maintenance and translation mechanics
required by the target profile.

## Kernel language and proof basis

The whole kernel cannot be expressed with the present scalar and no-vstd
collection subset. The kernel profile therefore adds:

- `u8` and `u16`, with the same checked arithmetic and explicit-conversion
  rules as `u32` and `u64`;
- executable and specified mutable slice reads/writes for the admitted element
  types;
- fixed-capacity storage views that can be backed by boot-reserved memory;
- content-preserving kernel `Vec`, `Map`, bitmap, and ring-buffer models;
- sealed atomic cell types and the frozen ordering enum;
- explicit address, page-size, alignment, CPU-mask, and generation newtypes; and
- loop, recursion, and collection lemmas needed by allocators, queues,
  schedulers, parsers, and drivers.

Fixed-capacity bitmaps, intrusive lists, slab metadata, queues, locks, and
runqueues are verified library components. They are not privileged primitives.
The early kernel uses boot-reserved fixed storage. It installs the verified
frame allocator and heap bridge before allocation-backed collections become
reachable. The allocator bridge is a frozen compiler-runtime hook whose body
delegates to the verified allocator state and whose receipt records that
binding.

## Event and action ABI

The ingress ABI has a closed event enum:

```text
Boot, CpuOnline, CpuStartFailed, Irq, Timer, Ipi, DmaComplete,
Syscall, UserFault, DeviceFault, ActionComplete, ShutdownRequest
```

Every event carries a CPU ID, monotonic event ID, capability-bounded payload,
and source-specific data. Asynchronous actions carry a correlation ID. The
initial action enum includes:

```text
StartCpu, SendIpi, TlbShootdown, AckIrq, MaskIrq, UnmaskIrq,
Map, Unmap, Protect, MmioRead, MmioWrite, PioRead, PioWrite,
SubmitDma, ArmTimer, CancelTimer, EnqueueTask, EnterContext,
Reboot, PowerOff, Halt
```

Event conversion validates lengths, CPU identity, vector ownership, trap
origin, and correlation before calling verified code. Action execution checks
the current capability ledger and action precondition. A failed action returns
a typed completion event or performs a declared terminal transition. It does
not silently mutate the modeled state.

## Build surface and final-image receipt

The additive command is:

```text
forge build conformance/thermite-kernel.thpkg.json --level l3 \
  --target kernel-image \
  --platform x86_64-pc-uefi-smp-v1 \
  --compose-export kernel_acceptance_slice \
  --compose-export service_write_user_byte \
  --compose-export ap_expected_mask \
  --compose-export apic_profile_supported \
  --compose-export apic_physical_base \
  --compose-export allocator_claim_first \
  --compose-shell platform/x86_64-pc-uefi-smp-v1/verified/kernel_policy_ingress.rs \
  --compose-shell platform/x86_64-pc-uefi-smp-v1/verified/tpl/atomic.rs \
  --out dist/thermite-kernel.img
```

`--target kernel` continues to produce the existing rlib. `kernel-image`
requires one platform, one ingress export, and the exact registered shell set.
It has no hosted fallback and no ambient toolchain discovery. Tool paths,
target features, CPU baseline, code model, relocation model, red-zone policy,
linker arguments, boot files, and image layout come from the frozen profile.

`ThermitePlatformConformanceReceiptV2` binds:

- all Thermite sources, roots, closure inventories, proof certificates,
  translation-validation evidence, and generated rlibs;
- the complete boundary inventory and registry entry for each reachable call;
- the canonical allowlisted transitive source closure, rejecting incidental
  `target`, `dist`, `__pycache__`, symlinks, unsorted entries, and path escapes;
- every TPL Rust and assembly source plus the linked PE/PDB, section, and
  public-symbol inventories;
- kernel vstd model, erased metadata, compiler runtime, allocator bridge, panic
  path, entry stubs, AP trampoline, and context/trap sources;
- platform manifest, architecture features, target identity, linker and
  section policy, compiler, linker, image packager, and boot adapter;
- final linked PE/COFF executable, debug-symbol artifact, UEFI executable,
  disk image, and canonical file inventory;
- replay environment, deterministic timestamp inputs, normalized commands, and
  every intermediate digest; and
- the acceptance matrix, virtual hardware configuration, CPU count, serial
  transcript digest, test result digest, and final success marker.

Validation recomputes the closure, signatures, contracts, models, source
digests, symbol ownership, section and public-symbol inventories, section
policy, and image files.
Replay rebuilds from the frozen profile and deterministic inputs, compares the
complete artifact set, and reruns the boot tests. Publication uses one scratch
directory and treats the final image rename as the atomic publication sentinel
after every proof, link, reproducibility, and boot gate succeeds.

## Landed implementation

The `x86_64-pc-uefi-smp-v1` conformance profile has a large frozen registry,
but declarations and model tests alone are not direct refinement proofs. The
current Thermite closure reaches atomic load, store, fetch, and strong
compare-exchange. Each maps in the canonical composition plan to its
`tpl_atomic_*` implementation in `verified/tpl/atomic.rs`; Verus checks those
exact bodies, the linker retains them, and Forge requires all four public
symbols in the final PE/PDB inventory. The receipt reports those four
boundaries separately at `L3_direct_refinement` and does not promote
unreachable registry entries. Rust-managed scheduler, allocator, IPC, AP, and
shootdown state now uses this sealed surface. Atomic instructions inside the
interrupt assembly remain a declared proof gap rather than being counted as
directly refined.

The authored package spans `conformance/kernel/*.th`. Its exact generated rlib
is linked into the UEFI image and supplies both the first capability/
scheduler/IPC vertical slice and the live subsystem behavior described above.
The allocator now invokes a generated, recursively specified 64-page first-fit
search inside a bounded generated strong-CAS claim state machine. Rust retains
raw pointer/provenance arithmetic and zeroing but no longer implements the
free-run traversal or allocation-claim orchestration policy.
The old linked safe-Rust model crate is absent from the runtime dependency
closure.

The freestanding image performs real post-firmware paging, descriptor and APIC
setup, INIT/SIPI AP startup, per-CPU stacks and GS bases, scheduler work,
release/acquire message passing, IPIs, TLB invalidation, timers, allocator
traffic, PCI/virtio DMA, RDRAND, ring-3 entry, architectural SYSCALL/SYSRET, a
user page fault, reboot, and poweroff. The release harness boots the same image
at 1, 2, 4, and 8 CPUs and also runs named AP-start-failure and reboot cases.

Forge publishes the image, PE/COFF executable, PDB, section and public-symbol
inventories, platform receipt, boot transcripts, and
`ThermitePlatformConformanceReceiptV2`. Validation recomputes every bound closure;
replay rebuilds byte-identical artifacts and reruns all six QEMU scenarios.
Receipt-field, transcript-marker, boundary-name, effect, signature, and exact
contract-digest mutations are permanent negative tests, and CI runs the
release-shaped image gate.

## Acceptance and adversarial matrix

The release gate covers the complete image rather than a host-linked model.

### Boot and useful work

- Boot the same UEFI image with 1, 2, 4, and 8 CPUs. Every CPU ID is unique and
  each discovered CPU reaches exactly one terminal bring-up state.
- On 4 CPUs, execute scheduler work on at least two APs as well as the BSP,
  deliver per-CPU timer interrupts, and complete a cross-CPU wakeup.
- Start an initial user task, enter user mode, service a syscall, handle a user
  page fault as a typed result, and resume or terminate according to policy.
- Enumerate the conformance device profile, write console output, read a known
  block through DMA, and verify its content in Thermite code.
- Reboot and power-off paths end in their named terminal states.

### SMP and memory ordering

- Concurrent atomic increments produce one exact total with no duplicate work
  IDs. A release/acquire message-passing test never observes the ready flag
  with stale payload.
- Compare-exchange rejects illegal success/failure ordering combinations at
  validation. Misaligned or non-capability-backed atomic cells reject before
  execution.
- Verified lock, bounded MPSC queue, work-stealing, and once-initialization
  stress tests preserve their invariants across all four CPU-count profiles.
- IPI unicast and broadcast acknowledge the correct vector, CPU mask, and
  epoch. A stale, duplicate, foreign-CPU, or missing acknowledgement cannot
  complete the request.
- Concurrent map/unmap stress completes TLB shootdown only after every online
  target acknowledges the current epoch. Stale translations are detected by
  a controlled probe.
- CPU-local interrupt save/restore rejects a token from another CPU or an old
  generation. One CPU's mask operation does not change another CPU's state.
- Partial AP startup failure yields a named failed CPU and a correct smaller
  online set. The scheduler and shootdown protocol continue with that set.

### Boundary and image negatives

- Unknown registry name, signature drift, weaker contract, undeclared platform
  effect, fabricated sealed type, stale capability, range overflow, wrong
  width, misalignment, and unauthorized rights all reject with no image.
- Modified TPL source, assembly, target identity, linker policy, compiler
  runtime, boot file, PE/PDB, section or symbol inventory, or image byte
  invalidates the receipt.
- A reachable arbitrary boundary, `unsafe`, `extern`, raw pointer, inline
  assembly, proof escape, hosted effect, panic, divergence, or sub-L3 core item
  rejects according to its named closure rule.
- MMIO/PIO outside the capability range, DMA after unpin, cross-device DMA,
  double frame release, non-canonical mapping, and user-copy overflow return
  their specified failure without unauthorized access.
- A failed proof, link, receipt validation, replay, boot, SMP test, syscall test,
  or device test leaves the publication path absent.
- Two clean builds are byte-identical for canonical sources, receipts, PE/COFF,
  PDB, UEFI executable, and disk image. Replay reproduces them before acceptance.

QEMU/OVMF supplies the automated release environment. The image format and
boot ABI remain suitable for a PC-class UEFI machine. Hardware smoke evidence
may be attached to a release receipt, but simulator success does not claim
verification of a particular physical platform.

## Migration order

1. **Landed:** exact-source UEFI codegen and the capability/scheduler/IPC event
   slice, canonical closure binding, authorship metrics, and the 1/2/4/8 gate.
2. **Landed:** receipt-bound multi-file Thermite packages, fixed-capacity kernel
   storage, sealed atomics/orderings, mutable slices with `final(...)`, and
   usable frozen-registry declarations.
3. **In progress:** allocator request/mask/claim, exact bounded first-fit and CAS
   orchestration, and page-entry policy are generated; migrate the remaining
   reclaim policy, page-table policy, synchronization, and full scheduler
   orchestration.
4. **In progress:** AP lifecycle models, exact APIC-ID set construction, xAPIC
   MSR interpretation, and shootdown verdicts are generated; migrate the
   remaining AP startup, IPI/rendezvous, interrupt acknowledgement, and
   shootdown execution state machines.
5. **In progress:** DMA queue/device policy and service/user-code construction
   are generated; migrate the remaining driver, DMA execution, syscall, and
   service state machines.
6. Reduce Rust/assembly to firmware ABI, raw pointer/provenance and volatile
   access, privileged instructions, atomics, entry/return assembly, and
   compiler-runtime necessities.
7. Supply a direct-Verus model/refinement proof for every reachable TPL
   operation, tied to the exact emitted implementation. L1 contracts, source
   review, model tests, and QEMU remain useful but are not substitutes.
8. Declare migration complete only when ordinary Rust policy/algorithm LOC is
   zero and the receipt independently reconstructs that result.

## Requirements

<!-- generated:reqs view=forge-bootable-multicore-kernel-status -->
Source: `.design/reqs/registry.toml`

| ID | Status | Owner | Title | Follow-up |
|---|---|---|---|---|
| REQ-MKERNEL-1 | partial | `.design/build/bootable-multicore-kernel.md` | Bootable kernel-image build target | Migrate the remaining kernel policy and algorithms into Thermite before treating kernel-image as a complete kernel product. |
| REQ-MKERNEL-10 | partial | `.design/build/bootable-multicore-kernel.md` | Application-processor lifecycle, IPIs, and shootdown | Migrate the remaining AP startup, IPI rendezvous, interrupt acknowledgement, and TLB shootdown execution state machines after the generated AP-set/APIC-policy slice. |
| REQ-MKERNEL-11 | partial | `.design/build/bootable-multicore-kernel.md` | Concurrent scheduler, contexts, and user-mode syscall path | Expand the acceptance scheduler slice into the complete multicore scheduler and context ownership policy. |
| REQ-MKERNEL-12 | partial | `.design/build/bootable-multicore-kernel.md` | Generation-safe DMA and IOMMU mediation | Migrate the remaining DMA execution, driver, and IOMMU ownership state machines into Thermite and directly prove raw device adapters. |
| REQ-MKERNEL-13 | partial | `.design/build/bootable-multicore-kernel.md` | Explicit clock, entropy, and power authority | Move service state machines into Thermite and directly prove clock, entropy, and power implementations. |
| REQ-MKERNEL-14 | partial | `.design/build/bootable-multicore-kernel.md` | Typed per-CPU event and action ABI | Generalize the acceptance event slice to the complete typed per-CPU event/action ABI. |
| REQ-MKERNEL-15 | partial | `.design/build/bootable-multicore-kernel.md` | Final-image receipt, validation, replay, and honest assurance | Finish exact TPL proof binding and reach zero ordinary-Rust policy LOC while preserving canonical source closure. |
| REQ-MKERNEL-16 | partial | `.design/build/bootable-multicore-kernel.md` | Bootable SMP release and adversarial gate | Run the same boot and adversarial matrix after every remaining policy subsystem is generated from Thermite. |
| REQ-MKERNEL-2 | partial | `.design/build/bootable-multicore-kernel.md` | Frozen platform registry and exact boundary closure | Tie every source-reachable TPL registry operation to a direct-Verus refinement proof of its exact implementation. |
| REQ-MKERNEL-3 | partial | `.design/build/bootable-multicore-kernel.md` | Sealed capability ledger and platform effects | Replace the remaining Rust capability policy with fixed-storage Thermite ledger state and generated execution. |
| REQ-MKERNEL-4 | partial | `.design/build/bootable-multicore-kernel.md` | Kernel scalar, mutable-storage, and collection basis | Add receipt-bound modules, fixed-capacity Thermite storage, and sealed atomics/orderings needed by the full kernel. |
| REQ-MKERNEL-5 | partial | `.design/build/bootable-multicore-kernel.md` | Boot entry and freestanding compiler runtime | Reduce the runtime to irreducible machine operations and directly prove each reachable adapter. |
| REQ-MKERNEL-6 | partial | `.design/build/bootable-multicore-kernel.md` | Capability-bounded physical and virtual memory | Migrate the remaining allocator traversal, page-table ownership transitions, and virtual-memory mapping algorithms into generated Thermite code. |
| REQ-MKERNEL-7 | partial | `.design/build/bootable-multicore-kernel.md` | Volatile MMIO, PIO, and device ordering | Directly refine the exact volatile MMIO, PIO, and ordering implementations in Verus. |
| REQ-MKERNEL-8 | partial | `.design/build/bootable-multicore-kernel.md` | CPU, interrupt, trap, and privilege transitions | Move CPU, interrupt, trap, and context policy to Thermite and prove the remaining assembly adapters. |
| REQ-MKERNEL-9 | partial | `.design/build/bootable-multicore-kernel.md` | Verified atomic memory model and synchronization basis | Extend the sealed atomic declarations and exact load/store/fetch/strong-CAS refinements to the remaining operations, then migrate all synchronization algorithms and raw interrupt atomics. |
<!-- /generated:reqs -->
