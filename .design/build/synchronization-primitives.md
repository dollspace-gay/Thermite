# Waiting and synchronization primitives

<!--
tier: 3-component
status: partial
decision: Thermite ships explicit bounded-wait, ticket-lock, barrier, once, reference-count, and seqlock mechanics in .th plus frozen pause, blocking-wait, and terminal-halt declarations; actual atomic and machine implementations remain consumer-refined boundaries
governs:
  - stdlib/kernel-primitives/synchronization.thpkg.json
  - stdlib/kernel-primitives/synchronization/barrier.th
  - stdlib/kernel-primitives/synchronization/once.th
  - stdlib/kernel-primitives/synchronization/refcount.th
  - stdlib/kernel-primitives/synchronization/seqlock.th
  - stdlib/kernel-primitives/synchronization/ticket_lock.th
  - stdlib/kernel-primitives/synchronization/wait.th
  - forge/tests/synchronization_primitives.rs
audited-content-sha256: 5d68e1a8bfb52985712a453289083faef992e21606e955412e2162c591c156c5 (re-pinned 2026-08-04 after the participant-aware barrier primitive checkpoint)
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

`stdlib/kernel-primitives/synchronization.thpkg.json` is a canonical six-root
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

## Participant-aware barriers

`BarrierState64` uses `u64` membership and arrival masks plus a nonwrapping
generation. A normal participant is one bit; a caller may deliberately use a
disjoint nonempty cohort mask as one arrival unit. Registration and
unregistration are accepted only at a clean round boundary, so membership
cannot change after the first arrival. Arrival distinguishes stale generation,
inactive membership, duplicate arrival, exhausted generation, waiting, and the
last arrival that advances the round and clears the arrival mask.

The exact state transitions, classification priority, stale-generation probe,
membership-freeze probe, and generation exhaustion mechanics prove at L3. The
`barrier_wf` inspector states the arrived-subset-of-registered mask invariant.
Thermite structs are not yet opaque, so arbitrary literal construction is not
excluded by the type system; consumers should construct through the supplied
operations and explicitly check externally supplied state. Atomic realization,
parking, wakeup, and barrier participation policy remain consumer obligations.

## Once initialization

`OnceState64` separates uninitialized, running, complete, and poisoned phases.
Beginning initialization rotates a nonwrapping generation and returns its token;
a second begin reports busy. Completion and poisoning accept only the live token,
so a stale initializer cannot publish or poison a later generation. Complete and
poisoned states are terminal, and generation exhaustion fails closed.

The probes establish one-winner completion, stale-token rejection followed by a
valid completion, poison persistence, and exhaustion without wraparound. The
module does not choose initialization contents or recovery policy.

## Reference counting

`RefCountState64` starts with one live reference. Acquire increments unless the
count is retired or exhausted. Release distinguishes an ordinary decrement from
the last reference, which atomically enters a retired zero-count state at the
model level. No acquire can resurrect that state, and another release reports
retirement rather than underflowing.

The lifecycle probe composes acquire, ordinary release, last release, and a
rejected resurrection. A separate probe proves saturation at `u64::MAX`.
Destruction, reclamation epochs, and what a reference owns remain consumer
policy.

## Seqlock versioning

`SeqLockState64` ties writer activity to odd sequence numbers. An inactive even
state can begin a writer only when two increments remain; writer begin returns
the odd token, readers retry while it is active, and matching finish advances to
an inactive even version. Read validation accepts only an unchanged inactive
stamp. Mismatched writer tokens and both begin/finish exhaustion paths fail
closed.

The probes show that a reader captured before a write becomes stale, a fresh
post-write read validates, and the counter never wraps. The module governs
version mechanics only; copying protected payload data and the atomic load/store
realization remain consumer composition obligations.

## Assurance and remaining work

`forge check --level l3` proves all 96 in-language items at L3. The three frozen
declarations remain L1 boundaries, so the package contains 99 source items in
total. Executable contracts kill 349 of 368 generated mutants.

`forge/tests/synchronization_primitives.rs` additionally:

- pins the exact L3/L1 split and all three boundary targets;
- requires the halt declaration to retain its explicit `diverge` effect;
- pins wait mutation at 21/22, barrier mutation at 141/149, ticket mutation at
  41/43, once mutation at 65/68, reference-count mutation at 32/34, and seqlock
  mutation at 49/52;
- rejects a false claim that an unchanged trace reports a change;
- rejects a false claim that the second ticket may bypass the first;
- rejects a false claim that a duplicate barrier arrival advances the round;
- rejects false second-once-winner, retired-reference-resurrection, and stale
  seqlock-read claims;
- builds and replays `ticket_lock_can_issue` as a strict freestanding scalar
  export while binding all six original modules into the receipt; and
- tampers with the bound wait source and requires validation to fail.

The strict export is scalar because current body TV cannot independently frame
the full named-aggregate/ADT lifecycle. The complete package source is still
receipt-bound, and every in-language aggregate transition has its individual
L3 certificate.

Remaining synchronization work includes atomic integration, named progress and
fairness assumptions in the registry, bounded concurrent queues, work-stealing
deque mechanics, and richer reader/writer coordination.

## Auditable metrics

| Metric | Value |
|---|---:|
| Physical Thermite LOC | 1,837 |
| Nonblank Thermite LOC | 1,743 |
| Thermite functions | 73 (64 executable, 6 specification, 3 frozen declarations) |
| In-language L3 items | 96 |
| Frozen boundary declarations | 3 at L1 |
| Executable mutants killed | 349/368 |
| Bodyful Rust/assembly synchronization implementations | 0 |
| Ordinary Rust kernel-policy/algorithm LOC | 0 |
| Direct-Verus TPL LOC shipped by this package | 0 |

The Rust acceptance test is proof, replay, and tamper harness code; it is not
linked into the artifact.
