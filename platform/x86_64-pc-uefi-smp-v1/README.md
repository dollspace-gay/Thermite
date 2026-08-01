# x86_64-pc-uefi-smp-v1

This frozen profile builds a freestanding PE/COFF UEFI image and packages it
at `EFI/BOOT/BOOTX64.EFI` in a deterministic FAT image. Its reviewed runtime
boundary first uses UEFI MP Services as a firmware diagnostic, then calls
`ExitBootServices` and continues as a self-contained kernel. The post-firmware
path installs its own page tables, GDT, TSS, IDT, local APIC state, and low-memory
INIT/SIPI trampoline. Every AP runs on a kernel-owned stack and participates in
the scheduler, interrupt, timer, and TLB-shootdown gates.

Before the one-way firmware handoff, the boot adapter reserves a 64-page extent
below 4 GiB. After the handoff, a page-granular global allocator owns that
extent, zeroes each allocation, and generation-safely reclaims it. The runtime
exercises the actual Rust allocation bridge and rejects overlap, stale contents,
or a failed reclaim.

Build and run the complete acceptance matrix from the workspace root:

```sh
platform/x86_64-pc-uefi-smp-v1/build-image.sh dist/thermite-kernel.img
platform/x86_64-pc-uefi-smp-v1/test-qemu.py dist/thermite-kernel.img
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
therefore uses the honest `to_platform_boundary` assurance scope.

The freestanding executable links `thermite-kernel` itself. After enabling the
kernel allocator it runs the safe event/action policy, scheduler, atomic
reads-from/happens-before model, generation-safe frame lifecycle, and a
present-IOMMU DMA transition before accepting the corresponding TPL behavior.
