# Frozen platform-operation primitives

<!--
tier: 3-component
status: partial
decision: Thermite ships the policy-free semantic platform surface while exact architecture implementations remain consumer-owned machine obligations
governs:
  - stdlib/kernel-primitives/platform.thpkg.json
  - stdlib/kernel-primitives/platform/api.th
  - forge/tests/platform_primitives.rs
audited-content-sha256: 1febc7b7bde0d9c458c6cc0bf6c326ff43a1f61994582d617cec9c076fb0384a (initial pin 2026-08-05 after the exact L3/type/model and bodyless machine-boundary audit)
extends:
  - .design/build/kernel-primitives.md
  - .design/build/frozen-primitive-registry.md
-->

## Scope

`stdlib/kernel-primitives/platform.thpkg.json` is the reusable declaration
surface for operations that cannot be implemented by a platform-independent
Thermite library. It contains no architecture selection, boot policy, page-table
walk, allocator, scheduler, interrupt policy, driver, DMA protocol, image, Rust
implementation, or assembly implementation.

The package is deliberately split by assurance rather than by source language:

- 55 sealed-type, observation-type, specification, and executable legality rows
  prove at L3;
- 74 exact bodyless declarations remain L1 machine boundaries; and
- there are no other sub-L3 rows.

The L1 count is not unfinished application code. Every row is an operation whose
meaning ultimately depends on firmware ABI, pointer provenance, volatile or
privileged machine behavior, concurrent hardware, terminal transfer, or a
consumer-owned platform resource. Consumer registries must bind only the
reachable subset to exact source, objects, target features, machine models, and
direct refinement evidence. A declaration alone never upgrades a caller to
end-to-end L3.

## Frozen families

| Boundary family | Count | Generic responsibility |
|---|---:|---|
| boot | 2 | normalize a handoff and transfer terminal control |
| runtime | 6 | terminal failures and compiler-required byte transfer/fill |
| memory/provenance | 18 | capability-backed addresses, raw/volatile access, page-table words, activation, local invalidation, cache maintenance |
| MMIO | 10 | capability-backed device addresses, width-specific volatile access, device fence |
| PIO | 7 | width-specific port access and device fence |
| CPU | 8 | CPU identity/features, control/register access, per-CPU base |
| IRQ | 7 | state-token acquisition, disable/restore, routing, mask/unmask, EOI |
| trap | 2 | checked context entry and terminal return |
| SMP | 4 | AP transport, IPI transport, online snapshot and acknowledgement |
| DMA | 4 | mapping, unmapping, and ownership-direction synchronization |
| clock | 3 | monotonic observation and deadline arm/cancel |
| entropy | 1 | capability-backed fill |
| power | 2 | terminal reboot and poweroff |

Atomics are intentionally not duplicated here. Their 50 declarations and
bounded ordering/history model live in `atomics.thpkg.json`. CPU pause, blocking
wait, and terminal halt remain the three machine declarations in
`synchronization.thpkg.json`. A later reusable umbrella bundle composes these
packages without inventing another operation table.

## L3 application-facing basis

The package supplies sealed authorities and handles for boot handoffs, memory
regions and addresses, address spaces, device regions, IRQ state, trap context,
CPU control, SMP transport, DMA domains/mappings, entropy, and power. Ordinary
Thermite code cannot construct those values.

The following bodyful helpers are ordinary Thermite and therefore prove at L3:

- `platform_width_legal` accepts only widths 1, 2, 4, and 8;
- `raw_range_legal` and `mmio_range_legal` use subtraction-after-bound checking
  so their range test cannot wrap; and
- `raw_aligned` and `mmio_aligned` combine width, range, and alignment legality.

Their executable contracts kill 38/38 generated mutants. The strict
freestanding package receipt exports `platform_width_legal`, binds the complete
authored module even though the machine doors are unreachable, and replays to
the same artifact. A false claim that width 3 is legal fails verification.

## Consumer refinement rule

The declarations are ABI-neutral Thermite contracts, not implementations. A
consumer selects the reachable operations and supplies registry rows. Registry
v3 now demonstrates this split for one canonical `PAtomicU64` SeqCst
create/load adapter: its wrapper is L3 relative to the pinned vstd model, while
three machine facts remain visible and cap the artifact at L1. The real sealed
atomic ABI, volatile, privileged, terminal, unsafe-Rust, and assembly
implementations remain unresolved. The safe sequential registry-v2 path must
reject them rather than laundering their contracts through a safe Rust model.

For each reachable machine row the eventual receipt must bind:

1. exact Thermite declaration and contract/effect/ownership digests;
2. exact consumer Rust/assembly/link inputs and every emitted object member;
3. target triple, pointer width, endianness, and target features;
4. an operation-specific machine or concurrency model;
5. the direct Verus refinement layer and every explicit residual assumption;
6. the final caller proof and link artifact; and
7. validation/replay of both proof layers without substituting a safe reference
   implementation.

That remaining refinement work is tracked by the frozen-registry and exact
platform-refinement requirements. It is the literal machine exception to the
L3/L4 application-primitive floor, not permission to put kernel algorithms in
Rust or assembly.
