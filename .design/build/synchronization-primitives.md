# Waiting and synchronization primitives

<!--
tier: 3-component
status: partial
decision: Thermite ships explicit bounded-wait, ticket-lock, barrier, epoch-acknowledgement, once, reference-count, seqlock, bounded MPSC queue, and bounded work-stealing deque mechanics in .th plus frozen pause, blocking-wait, and terminal-halt declarations; actual atomic and machine implementations remain consumer-refined boundaries
governs:
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
  - forge/tests/synchronization_primitives.rs
audited-content-sha256: ac71262f18e20c8acc8ada370887f5ed9a00b4d1ad322ec03725df52f2bd3c72 (re-pinned 2026-08-04 after complete-certificate L3-floor enforcement)
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

`stdlib/kernel-primitives/synchronization.thpkg.json` is a canonical nine-root
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

## Epoch acknowledgement sets

`EpochAckState64` provides a bounded 64-participant epoch mechanism for such
uses as cross-CPU acknowledgements, grace periods, or caller-defined rendezvous.
Registration issues a per-slot generation token and is frozen while a round is
open. Beginning a round increments the nonwrapping epoch and snapshots the
active membership mask into a pending mask. A matching participant may either
acknowledge or be explicitly withdrawn; both operations clear exactly that
pending bit. Closing succeeds only after the pending mask reaches zero.

No-round, stale-epoch, future-epoch, stale-participant, duplicate/not-pending,
pending-close, epoch-exhaustion, and participant-generation-exhaustion outcomes
are distinct. The packed-bit preservation bridge proves that clearing one
participant leaves every specifically observed distinct participant unchanged.
Source probes cover acknowledgement and withdrawal framing, duplicate rejection,
stale/future tokens, frozen membership, membership snapshot, both exhaustion
paths, and complete versus pending close.

The primitive does not choose participants, withdrawal policy, timeouts, retry,
or what an epoch means. It is a pure state machine; a consumer must map its
transitions to sealed atomics and directly refined machine operations. Its plain
state and token structs remain forgeable until opaque construction or a complete
affine rule lands, so consumers must preserve token ownership and construct
states through the supplied operations.

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

## Bounded MPSC queue mechanics

`MpscQueue64` separates producer reservation from publication. A successful
reservation returns a ticket/slot token without making the value visible. Any
producer may publish its own live reservation out of order, while the single
consumer may pop only the lowest outstanding ticket. If that ticket has not yet
been published, pop returns `MpscPending64` instead of bypassing it. The model is
allocation-free and uses 64 fixed slots, per-slot ready bits, exact tickets, and
`u64` payloads.
The empty constructor pins all 64 elements of all three storage arrays through
extensional equality; it does not infer initialization from sentinel slots.

Publication rejects slot-mismatched, stale, unknown, duplicate, and conflicting
tokens explicitly. Reservation reports full capacity, counter exhaustion, and an
unexpected busy slot without wrapping or overwriting live storage. Pop reports
empty, pending, and conflicting slot identity separately. The contracts use the
native fixed-array `array_same_except` relation to frame every untouched element;
that relation has runnable L1/L3 scans and independent contract, expression, and
body translation validation.

Source probes establish out-of-order producer publication with FIFO consumer
visibility, duplicate-publication rejection, stale-token rejection after
consumption, and fail-closed slot conflict. The pure transitions are reusable
queue mechanics, not a claim that sequential state alone implements concurrency.
A consumer must linearize reservation/publication/pop through sealed atomics and
directly refined machine operations, and must choose any blocking, retry,
backpressure, payload-ownership, and scheduling policy.
Thermite structs are not yet opaque, so a consumer can still spell a live-looking
reservation literal; until module-private construction or affine tokens land, the
consumer must preserve reservation ownership and treat the supplied range/slot/
generation checks as misuse detection rather than unforgeability.

## Bounded work-stealing deque mechanics

`WorkDeque64` is a fixed-capacity, allocation-free Chase–Lev-style state machine:
one owner pushes and pops at the bottom while any number of thieves snapshot and
attempt to commit steals at the top. Slots carry exact logical tickets alongside
ready bits and `u64` values. Push reports owner-busy, full, counter-exhausted, and
unexpected occupied-slot states instead of overwriting storage or wrapping an
index.

Owner pop is deliberately two-phase. Begin decrements the visible bottom, marks
an owner operation pending, and issues a generation-bound token. This hides the
candidate from new thieves while permitting a thief that already observed the
old top to race for the last item. Commit either pops the bottom candidate,
reports that the thief won, or returns an explicit stale/vacant/conflicting
state. Cancellation restores the visible bottom without consuming the item.
The nonwrapping owner generation prevents a stale token from matching a later
pending operation.

Steal is also two-phase. Begin snapshots the top ticket, visible bottom, slot,
and value. Commit succeeds only while the top snapshot remains current and the
exact slot identity/value still agrees; otherwise it returns retry, malformed,
vacant, or conflicting evidence without changing the state. A successful steal
advances top and clears exactly its slot. The pending-state invariant admits the
single transient `top == bottom + 1` state when a pre-existing thief wins the
last-item race, which owner cleanup restores to the ordinary empty state.

Source probes prove owner LIFO behavior, thief FIFO behavior, both possible
last-item winners with exactly one successful consumer, opposite-end progress
on two items, cancellation, and both index and owner-generation exhaustion.
These are reusable linearization mechanics, not a machine-concurrency claim.
A consumer must map begin/commit to the sealed atomic loads, stores, fences, and
compare-exchanges with the required Chase–Lev orderings and direct refinements.
Scheduling, retry, parking, victim selection, fairness, and task ownership remain
consumer policy. Plain structs remain forgeable until the language gains opaque
construction or complete affine tokens, so consumers must preserve token
ownership in the interim.

## Assurance and remaining work

`forge check --level l3` proves all 222 in-language items at L3. The three frozen
declarations remain L1 boundaries, so the package contains 225 source items in
total. Executable contracts kill 756 of 834 generated mutants.

`forge/tests/synchronization_primitives.rs` additionally:

- iterates every certificate row, pins every bodyful item at L3, and pins the
  exact three-item L1 exception set and all three boundary targets;
- requires the halt declaration to retain its explicit `diverge` effect;
- pins wait mutation at 21/22, barrier mutation at 141/149, ticket mutation at
  41/43, once mutation at 65/68, reference-count mutation at 32/34, seqlock
  mutation at 49/52, MPSC queue mutation at 100/119, and work-deque mutation at
  211/247, plus epoch-ack mutation at 96/100;
- rejects a false claim that an unchanged trace reports a change;
- rejects a false claim that the second ticket may bypass the first;
- rejects a false claim that a duplicate barrier arrival advances the round;
- rejects a false claim that acknowledging one epoch participant clears a
  distinct pending participant;
- rejects a false claim that a published later queue ticket may bypass the
  unpublished FIFO head;
- rejects a false claim that the owner can also consume the last item after a
  thief has won its top compare-exchange;
- rejects false second-once-winner, retired-reference-resurrection, and stale
  seqlock-read claims;
- builds and replays `ticket_lock_can_issue` as a strict freestanding scalar
  export while binding all nine original modules into the receipt; and
- tampers with the bound wait source and requires validation to fail.

The strict export remains scalar. Body TV now independently frames user-ADT
match/results, exact record-state loops, and direct finite-record mutable-call
effects including typed let-bound results. These synchronization modules use owned pure state transitions; their
remaining end-to-end gap is atomic composition and machine concurrency proof,
not an unmodelled Rust implementation. The complete package source is
receipt-bound, and every in-language aggregate transition has its individual L3
certificate.

Remaining synchronization work includes atomic integration, named progress and
fairness assumptions in the registry, and richer reader/writer coordination.

## Auditable metrics

| Metric | Value |
|---|---:|
| Physical Thermite LOC | 5,587 |
| Nonblank Thermite LOC | 5,367 |
| Thermite functions | 176 (135 executable, 38 specification, 3 frozen declarations) |
| In-language L3 items | 222 |
| Frozen boundary declarations | 3 at L1 |
| Executable mutants killed | 756/834 |
| Bodyful Rust/assembly synchronization implementations | 0 |
| Ordinary Rust kernel-policy/algorithm LOC | 0 |
| Direct-Verus TPL LOC shipped by this package | 0 |

The Rust acceptance test is proof, replay, and tamper harness code; it is not
linked into the artifact.
