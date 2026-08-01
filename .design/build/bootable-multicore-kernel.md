# Bootable multicore kernel platform

<!--
tier: 3-component
status: shipped
decision: frozen registry-backed platform boundaries with a receipt-bound bootable SMP image
governs:
  - thermite-syntax/src/ast.rs
  - thermite-syntax/src/parser.rs
  - thermite-syntax/src/lib.rs
  - thermite-syntax/tests/kernel_surface.rs
  - thermite-lower/src/effects.rs
  - thermite-lower/src/lower.rs
  - thermite-lower/tests/kernel_mutable_slice.rs
  - thermite-verified/src/lib.rs
  - thermite-kernel/src/lib.rs
  - thermite-kernel/src/registry.rs
  - thermite-kernel/tests/model.rs
  - forge/src/kernel_image.rs
  - forge/src/cli.rs
  - forge/src/main.rs
  - conformance/bootable_kernel.th
  - conformance/kernel_primitives.th
  - platform/x86_64-pc-uefi-smp-v1/profile.toml
  - platform/x86_64-pc-uefi-smp-v1/registry.toml
  - platform/x86_64-pc-uefi-smp-v1/kernel_shell.rs
  - platform/x86_64-pc-uefi-smp-v1/build-image.sh
  - platform/x86_64-pc-uefi-smp-v1/test-qemu.py
  - platform/x86_64-pc-uefi-smp-v1/runtime/src/main.rs
  - platform/x86_64-pc-uefi-smp-v1/runtime/src/post_firmware.rs
  - platform/x86_64-pc-uefi-smp-v1/runtime/src/ap_trampoline.S
  - .github/workflows/ci.yml
audited-content-sha256: b58cd04ccd9ea1fe627f5ea9779c5a6a65f328c781362b22268c0c2d553c633a
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

Thermite's complete kernel product is a bootable, reproducible multicore image.
The current `--target kernel` rlib remains the verified freestanding library
stage. A new `kernel-image` target closes that library over one frozen target
platform layer, a boot adapter, target identity and link policy, compiler
runtime, and image packager. Forge publishes the image only with a receipt that
binds every member of that closure.

The first conformance profile is `x86_64-pc-uefi-smp-v1`. It produces a UEFI
disk image that can boot on a PC-class machine and under QEMU/OVMF. Its release
gate boots with 1, 2, 4, and 8 logical CPUs. The 4-CPU run is the normative SMP
acceptance case. Every discovered application processor must reach a named
online or failed state, and useful kernel work must execute concurrently on
more than one online CPU.

Ordinary Thermite code retains the existing surface. It has no raw pointers,
inline assembly, arbitrary `extern` declarations, or general unsafe escape.
Privileged operations are frozen functions written as
`#[boundary("kernel::...")]`; authority values are `#[sealed]` types. A final
image accepts only boundary declarations that exactly match its platform
registry. The privileged Rust and assembly bodies are part of the explicit
target platform layer (TPL) and trust boundary.

## Product and assurance boundary

The image has four layers:

| Layer | Contents | Assurance |
|---|---|---|
| verified kernel core | resource policy, parsers, allocators, schedulers, drivers, protocols, filesystems, syscalls | L3 where the current exact-source rules admit the code |
| verified concurrency runtime | per-CPU state machines, queues, locks, ownership transfer, IPI and shootdown protocols | L3 against the frozen atomic and interrupt models |
| target platform layer | entry stubs, context/trap assembly, privileged instructions, atomics, MMIO/PIO, page-table and CPU control adapters | registered L1 boundary contracts plus target-specific review or external proof evidence |
| image closure | compiler runtime, linker, boot adapter, firmware ABI, image packager | digest-bound build evidence and runtime acceptance |

The final receipt reports the artifact scope as `to_platform_boundary`. It may
also report the L3 result for the verified core. It cannot describe the whole
image as end-to-end L3 while registered platform calls remain boundary
assumptions. A TPL implementation can carry stronger evidence, but that
evidence is recorded separately and does not change the Thermite assurance
ladder.

The hardware, firmware, compiler backend, linker, and frozen TPL bodies remain
in the trusted computing base. This limitation is stated in the receipt and in
`forge audit` output.

### Paired proof and implementation closures

`kernel-image` preserves the strict L3 artifact policy. The current strict
`VerifiedCompositionReceiptV1` path continues to reject every boundary and to
mean end-to-end L3 for its complete rlib closure.

The image builder constructs a separate pair of closures from the same roots:

1. The proof closure uses the shipped boundary-composition rule. Forge emits a
   generated `external_body` signature only for a declared boundary, and Verus
   proves each caller against that boundary's exact contract. The caller can
   achieve L3 while its orthogonal assurance scope is `to_boundary`.
2. The implementation closure resolves that declaration to exactly one frozen
   registry entry and links the bound Rust or assembly body. The registry
   signature, contract, model, effects, ABI, object, and symbol must agree with
   the proof closure.

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
forge build kernel.th --level l3 \
  --target kernel-image \
  --platform x86_64-pc-uefi-smp-v1 \
  --compose-export kernel_step \
  --compose-shell platform_shell.rs \
  --out dist/thermite-kernel.img
```

`--target kernel` continues to produce the existing rlib. `kernel-image`
requires one platform, one ingress export, and the exact registered shell set.
It has no hosted fallback and no ambient toolchain discovery. Tool paths,
target features, CPU baseline, code model, relocation model, red-zone policy,
linker arguments, boot files, and image layout come from the frozen profile.

`ThermiteBootableKernelReceiptV1` binds:

- all Thermite sources, roots, closure inventories, proof certificates,
  translation-validation evidence, and generated rlibs;
- the complete boundary inventory and registry entry for each reachable call;
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

The `x86_64-pc-uefi-smp-v1` profile is implemented as a 104-operation frozen
registry. The source-reachable `kernel::clock::read@v1` boundary is pinned by
name, exact Thermite signature, one `platform(clock)` effect, capability and
rights metadata, C ABI symbol, and the exact source-contract digest. The same
`kernel_shell.rs` bound by Forge is compiled and exercised by the UEFI runtime.
Implementation-only operations are supplied by the linked `thermite-kernel`
models and the digest-bound Rust/assembly TPL.

The safe core implements generation-tracked capabilities, frame and address
space state, temporary mappings, all-or-nothing user copies, volatile device
models, interrupt tokens, atomic traces, AP/IPI/shootdown state, concurrent
scheduling and synchronization, DMA/IOMMU ownership, services, typed ingress,
and the capability-checking action executor. Executable mirrors of the critical
transition predicates are proved by `verus --no-cheating` and exercised against
those production models.

The freestanding image performs real post-firmware paging, descriptor and APIC
setup, INIT/SIPI AP startup, per-CPU stacks and GS bases, scheduler work,
release/acquire message passing, IPIs, TLB invalidation, timers, allocator
traffic, PCI/virtio DMA, RDRAND, ring-3 entry, architectural SYSCALL/SYSRET, a
user page fault, reboot, and poweroff. The release harness boots the same image
at 1, 2, 4, and 8 CPUs and also runs named AP-start-failure and reboot cases.

Forge publishes the image, PE/COFF executable, PDB, section and public-symbol
inventories, platform receipt, boot transcripts, and
`ThermiteBootableKernelReceiptV1`. Validation recomputes every bound closure;
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

## Implementation order

The implementation is one release program with the following dependency
order. SMP is part of its completion gate.

1. Add `u8`/`u16`, kernel mutable-storage models, platform effects, sealed
   atomic types, and ordering validation.
2. Define the registry schema, boundary matcher, capability ledger model, and
   image-specific closure rule.
3. Add Verus models and lowering for storage, atomics, capabilities, per-CPU
   ownership, queues, and locks.
4. Add the frozen x86_64 target identity, linker/image pipeline, runtime
   symbols, receipt, validation, and replay.
5. Bring up BSP boot, console, normalized firmware bytes, physical memory,
   paging, the verified heap, interrupts, and timers.
6. Bring up APs, per-CPU state, atomics, IPIs, rendezvous, TLB shootdown, and
   concurrent scheduler execution.
7. Add contexts, user mode, syscall/trap paths, DMA, the conformance block
   device, and terminal power actions.
8. Run the complete negative matrix and 1/2/4/8-CPU boot gate, then publish the
   first receipt-bound image.

## Requirements

<!-- generated:reqs view=forge-bootable-multicore-kernel-status -->
Source: `.design/reqs/registry.toml`

| ID | Status | Owner | Title | Follow-up |
|---|---|---|---|---|
| REQ-MKERNEL-1 | shipped | `.design/build/bootable-multicore-kernel.md` | Bootable kernel-image build target |  |
| REQ-MKERNEL-10 | shipped | `.design/build/bootable-multicore-kernel.md` | Application-processor lifecycle, IPIs, and shootdown |  |
| REQ-MKERNEL-11 | shipped | `.design/build/bootable-multicore-kernel.md` | Concurrent scheduler, contexts, and user-mode syscall path |  |
| REQ-MKERNEL-12 | shipped | `.design/build/bootable-multicore-kernel.md` | Generation-safe DMA and IOMMU mediation |  |
| REQ-MKERNEL-13 | shipped | `.design/build/bootable-multicore-kernel.md` | Explicit clock, entropy, and power authority |  |
| REQ-MKERNEL-14 | shipped | `.design/build/bootable-multicore-kernel.md` | Typed per-CPU event and action ABI |  |
| REQ-MKERNEL-15 | shipped | `.design/build/bootable-multicore-kernel.md` | Final-image receipt, validation, replay, and honest assurance |  |
| REQ-MKERNEL-16 | shipped | `.design/build/bootable-multicore-kernel.md` | Bootable SMP release and adversarial gate |  |
| REQ-MKERNEL-2 | shipped | `.design/build/bootable-multicore-kernel.md` | Frozen platform registry and exact boundary closure |  |
| REQ-MKERNEL-3 | shipped | `.design/build/bootable-multicore-kernel.md` | Sealed capability ledger and platform effects |  |
| REQ-MKERNEL-4 | shipped | `.design/build/bootable-multicore-kernel.md` | Kernel scalar, mutable-storage, and collection basis |  |
| REQ-MKERNEL-5 | shipped | `.design/build/bootable-multicore-kernel.md` | Boot entry and freestanding compiler runtime |  |
| REQ-MKERNEL-6 | shipped | `.design/build/bootable-multicore-kernel.md` | Capability-bounded physical and virtual memory |  |
| REQ-MKERNEL-7 | shipped | `.design/build/bootable-multicore-kernel.md` | Volatile MMIO, PIO, and device ordering |  |
| REQ-MKERNEL-8 | shipped | `.design/build/bootable-multicore-kernel.md` | CPU, interrupt, trap, and privilege transitions |  |
| REQ-MKERNEL-9 | shipped | `.design/build/bootable-multicore-kernel.md` | Verified atomic memory model and synchronization basis |  |
<!-- /generated:reqs -->
