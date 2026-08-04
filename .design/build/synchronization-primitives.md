# Waiting and synchronization primitives

<!--
tier: 3-component
status: partial
decision: Thermite ships explicit bounded-wait and ticket-lock state mechanics in .th plus frozen pause, blocking-wait, and terminal-halt declarations; actual atomic and machine implementations remain consumer-refined boundaries
governs:
  - stdlib/kernel-primitives/synchronization.thpkg.json
  - stdlib/kernel-primitives/synchronization/ticket_lock.th
  - stdlib/kernel-primitives/synchronization/wait.th
  - forge/tests/synchronization_primitives.rs
audited-content-sha256: 31cccdc383df7500e8ec3ecd23fd6e7a67f1d502fad0ccf762a89e2590aa1677
extends:
  - .design/build/kernel-primitives.md
  - .design/build/sealed-atomics.md
  - .design/build/l3-verified-artifact.md
thesis-refs:
  - thermite-design.md §3
  - thermite-design.md §5.3
  - thermite-design.md §6
  - thermite-design.md §9
-->

## Decision

Thermite provides policy-free waiting and synchronization mechanics for later
consumer kernels. This repository does not choose which threads block, how a
scheduler parks them, what protected data means, or which architecture executes
pause and halt instructions.

`stdlib/kernel-primitives/synchronization.thpkg.json` is a canonical two-root
package containing only Thermite source. It has no Rust or assembly runtime
implementation and no kernel policy.

## Waiting surface

The frozen machine-facing declarations are:

- `thermite::cpu::pause`, a platform-CPU pause/yield hint;
- `thermite::wait::block`, which consumes a sealed wait permit and returns a
  sealed wake token for the same waiter and key at a nondecreasing epoch; and
- `thermite::cpu::halt_terminal`, an explicitly divergent platform-CPU
  operation.

They are declarations, not implementations. A consumer must provide the exact
machine bodies and direct refinements through a registry version capable of
expressing their concurrency and progress semantics. The blocking declaration
does not itself prove fairness.

`bounded_wait_scan` is the total, allocation-free in-language foundation. It
examines at most a caller-supplied budget from a 64-observation trace and returns
either the first observed change with its poll count or an explicit timeout.
The budget is bounded by trace length, every loop has an ordinary decreasing
measure, and no infinite spin is represented as verified progress.

## Ticket-lock mechanics

`TicketLockState64` models monotonically increasing `next` and `serving`
counters without wraparound. Ticket issue fails closed at `u64::MAX` rather than
reusing a live number. Entry distinguishes entered, waiting, stale, and unknown
tickets. Release consumes a matching guard and advances `serving`; a mismatched
or idle release returns both the unchanged state and guard explicitly.

Source probes prove FIFO admission for two tickets, stale-ticket rejection after
release, and exhaustion without wraparound. This module specifies the reusable
ticket-state algorithm. A concurrent consumer still has to realize the
transitions with the sealed atomic operations and directly refined machine
implementations; this increment does not claim mutual exclusion from a
sequential pure model alone.

## Assurance and remaining work

`forge check --level l3` proves all 31 in-language items at L3. The three frozen
declarations remain L1 boundaries, so the package contains 34 source items in
total. Executable contracts kill 62 of 65 generated mutants.

`forge/tests/synchronization_primitives.rs` additionally:

- pins the exact L3/L1 split and all three boundary targets;
- requires the halt declaration to retain its explicit `diverge` effect;
- pins wait mutation at 21/22 and ticket mutation at 41/43;
- rejects a false claim that an unchanged trace reports a change;
- rejects a false claim that the second ticket may bypass the first;
- builds and replays `ticket_lock_can_issue` as a strict freestanding scalar
  export while binding both original modules into the receipt; and
- tampers with the bound wait source and requires validation to fail.

The strict export is scalar because current body TV cannot independently frame
the full named-aggregate/ADT lifecycle. The complete package source is still
receipt-bound, and every in-language aggregate transition has its individual
L3 certificate.

Remaining synchronization work includes atomic integration, named progress and
fairness assumptions in the registry, once cells, barriers, reference counts,
seqlocks, bounded concurrent queues, and work-stealing deque mechanics.

## Auditable metrics

| Metric | Value |
|---|---:|
| Physical Thermite LOC | 488 |
| Nonblank Thermite LOC | 455 |
| Thermite functions | 23 (17 executable, 3 specification, 3 frozen declarations) |
| In-language L3 items | 31 |
| Frozen boundary declarations | 3 at L1 |
| Executable mutants killed | 62/65 |
| Bodyful Rust/assembly synchronization implementations | 0 |
| Ordinary Rust kernel-policy/algorithm LOC | 0 |
| Direct-Verus TPL LOC shipped by this package | 0 |

The Rust acceptance test is proof, replay, and tamper harness code; it is not
linked into the artifact.
