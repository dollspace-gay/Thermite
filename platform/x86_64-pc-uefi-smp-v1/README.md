# x86_64-pc-uefi-smp-v1

This frozen profile builds a freestanding PE/COFF UEFI platform/conformance
demonstration and packages it
at `EFI/BOOT/BOOTX64.EFI` in a deterministic FAT image. Its reviewed runtime
boundary uses UEFI MP Services only to discover the enabled processor/APIC-ID
inventory, then calls `ExitBootServices` and continues as a self-contained kernel. The post-firmware
path installs its own page tables, GDT, TSS, IDT, local APIC state, and low-memory
INIT/SIPI trampoline. Every AP runs on a kernel-owned stack and participates in
the scheduler, interrupt, timer, and TLB-shootdown gates.

Before the one-way firmware handoff, the boot adapter reserves a 64-page extent
below 4 GiB. After the handoff, a page-granular global allocator owns that
extent, zeroes each allocation, and generation-safely reclaims it. The runtime
exercises the actual Rust allocation bridge and rejects overlap, stale contents,
or a failed reclaim.

Build and run the receipt-bound acceptance matrix from the workspace root:

```sh
cargo run -p forge -- build conformance/thermite-kernel.thpkg.json \
  --level l3 --target kernel-image \
  --platform x86_64-pc-uefi-smp-v1 \
  --compose-export kernel_acceptance_slice \
  --compose-export ipc_worker_dispatch \
  --compose-export service_write_user_byte \
  --compose-export ap_expected_mask \
  --compose-export apic_profile_supported \
  --compose-export apic_physical_base \
  --compose-export allocator_claim_first \
  --compose-shell platform/x86_64-pc-uefi-smp-v1/verified/kernel_policy_ingress.rs \
  --compose-shell platform/x86_64-pc-uefi-smp-v1/verified/tpl/atomic.rs \
  --out dist/thermite-kernel.img
cargo run -p forge -- verify-build dist/thermite-kernel.img --replay --json
```

The runtime validates a bounded firmware memory-map handoff. It also copies the
loaded-image, command-line, firmware-table, and ACPI RSDP
metadata into a pointer-free normalized handoff with checked exact bounds;
optional framebuffer and initrd regions are explicitly absent in the frozen
headless conformance profile.

The runtime performs a second block read after firmware exit through a directly
driven legacy virtio-blk PCI
queue, and transfers the DMA buffer from CPU to device and back before advancing
its generation. It also completes epoch-correlated APIC IPIs, replaces a live
mapping and remotely invalidates it, exercises a shared work queue plus ticket
lock and once invariants, delivers per-CPU TSC-deadline/APIC timer events, fills
an exact entropy buffer with checked `RDRAND`, and enters a real ring-3 program.
That program invokes the architectural `SYSCALL` entry configured through
`STAR`, `LSTAR`, and `SFMASK`, returns with `SYSRET`, takes a user page fault, resumes, and
returns through the TSS-owned kernel stack. The final power action calls the
UEFI runtime reset service, which remains valid after Boot Services exit.

The runtime's raw firmware pointers, UEFI ABI declarations, assembly trampoline,
page-table writes, descriptor loads, APIC, PCI/PIO, and DMA operations are
target-platform-layer code. They do not extend the ordinary
Thermite language with unsafe operations. `profile.toml` freezes the build and
virtual hardware, while `registry.toml` binds the executable profile to the
canonical registry implemented in `thermite-kernel`. The generated receipt
therefore uses the honest `platform_conformance_to_boundary` assurance scope,
sets `migration_complete=false`, and calls the artifact a
`platform_conformance_demonstration`.

The image links and executes the exact UEFI-targeted Verus rlib generated from
the Thermite capability-ledger, scheduler, and IPC/event acceptance slice. Its
CPU-dependent result controls the base task identifier of the real
post-firmware multicore work queue. Generated Thermite also performs the live
allocator's bounded first-fit/CAS claim state machine and decides allocator ownership,
page-table entry flags, AP-set, scheduler-completion,
shootdown, DMA-completion, and user-service verdicts. The AP-set path now folds
the complete bounded APIC-ID inventory in generated Thermite, rejecting
out-of-range or duplicate IDs, and generated Thermite also interprets the xAPIC
enable/x2APIC bits and extracts the physical LAPIC base from the architectural
MSR. The four source-reachable
atomic load/store/fetch/strong-compare-exchange boundaries map to exact checked
Verus implementations and retained final PE symbols. The live per-CPU IPC worker
dispatch now executes generated Thermite for payload validation, stale-message
accounting, and delivery-mask publication. Rust-managed scheduler, AP, and
shootdown state uses those sealed atomics; the allocator retains only
pointer/provenance and zeroing operations in Rust. Interrupt assembly still has
raw atomic operations awaiting exact refinement. QEMU checks
execution with 1, 2, 4, and 8 CPUs. The runtime no longer links the parallel
safe-Rust kernel model. Page-table traversal and
writes, synchronization orchestration, AP/shootdown execution, DMA setup,
device, and protocol logic remain in runtime Rust and must still migrate before
this artifact can be called a Thermite-authored formally verified kernel.

`source-allowlist.txt` is the canonical source closure. Forge rejects unsorted,
unsafe, symlinked, or incidental `target`, `dist`, and `__pycache__` entries and
binds every listed source into the final receipt. The receipt also publishes
conservative authorship metrics; the ordinary-Rust policy target is zero and
is intentionally reported as unmet during migration.
