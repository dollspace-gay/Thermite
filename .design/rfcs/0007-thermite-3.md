---
rfc: 7
title: Thermite 3 — a surface generation for a language written by agents
status: draft
supersedes: []
introduces: []
---

# RFC-7: Thermite 3 — a surface generation for a language written by agents

| | |
|---|---|
| **Status** | Design horizon. One step is filed; the rest are sequenced, not proposed |
| **Supersedes** | — |
| **Baseline** | `dollspace-gay/Thermite @ 84d276e7` |
| **Anchor** | [RFC-6](0006-full-words.md), filed as PR #128 |
| **Follow-up** | Six capability RFCs, in the order given in §14 |

This document is the **horizon rather than a delta**. It writes out the whole
proposed surface in one place so that each following RFC can be read as one step
of a stated plan rather than as an isolated request.

**Nothing here is being asked for except the anchor.** RFC-6 is the only live
proposal, and it changes no expressive power at all. Everything below it is shown
so a reviewer can see what the naming decisions in RFC-6 are serving, and can
reject the direction now rather than one clause at a time.

**Contents:** [§0 What this asks](#0-what-this-asks) ·
[§1 Why](#1-why-a-surface-generation) · [§2 The throughline](#2-the-throughline) ·
[§3 Functions](#3-functions) · [§4 Clauses](#4-clauses) ·
[§5 The effect row](#5-the-effect-row) · [§6 Specification vocabulary](#6-specification-vocabulary) ·
[§7 Data and resources](#7-data-and-resources) · [§8 Shared state and locks](#8-shared-state-and-locks) ·
[§9 Interference and interrupts](#9-interference-and-interrupts) ·
[§10 Protocols](#10-protocols) · [§11 Loops and durability](#11-loops-and-durability) ·
[§12 Where this is going](#12-where-this-is-going) · [§13 The anchor](#13-the-anchor) ·
[§14 The sequence](#14-the-sequence) · [§15 The whole surface as one subsystem](#15-the-whole-surface-as-one-subsystem)

---

## 0. What this asks

A read, and a direction check. Concretely:

1. **Is a surface generation wanted at all?** If the answer is no, RFC-6 should
   be closed and the six documents behind this one are never written up. That is
   a cheap outcome and it is the reason this exists.
2. **Does the organising rule hold?** Every clause is a third-person-singular
   verb whose subject is the item. It decided every name below, and it is the one
   thing worth arguing about before any of it is built.
3. **Is the ordering right?** Six capability proposals, each depending on the one
   before, listed in §14 with what each costs.

No requirement is introduced by this RFC. It is a plan.

## 1. Why a surface generation

### The name is borrowed, not invented

Thermite already numbers generations: [RFC-1](0001-thermite-2.md), "Thermite 2 —
a dependent-type tier, a stratified cage, and new ladder boundaries." There is a
convention for a generational proposal and this follows it.

**Thermite 3 is a surface generation, not a semantics one.** Thermite 2 changed
what the language can prove. This changes what it reads like, and then adds
capability on top of the settled surface. That distinction is what makes the
sequence work: the expensive-to-review part and the cheap-to-review part are
separable, and the cheap one goes first.

### The motivation is Thermite's own stated purpose

The language is designed to be written principally by agents, and a surface built
for that has requirements a human-authored one does not. A keyword's cost is not
the tokens it spends but the prior it activates, and a clause read wrongly yields
a vacuous proof rather than an error. `fx`, `dec` and `inv` are not merely terse —
they point at effects, declarations and inverses. That is a gap between what the
language is for and what it currently reads like, and closing it is the whole of
the anchor.

### Where this comes from

This comes from outside the project. It was written while porting a kernel to
Thermite — a workload chosen to press on the language hard enough to find where
the surface and the capability run out. A kernel needs structural predicates,
shared state, ownership, interference and protocols, and it needs them in a form
an agent can write correctly.

Every gap behind these proposals was found by attempting something rather than by
reading the reference, and several contradicted the documentation in both
directions. Three are already filed as defects with reproductions: #124, #125,
#126.

What that buys is evidence, not standing. The porting project has no verified
subsystem yet, and a proposal here earns its way on the design and the
reproduction attached to it.

## 2. The throughline

One principle, and every section inherits it:

> Thermite is written principally by language models, so **semantic overlap with
> pretraining is worth more than token economy** — and abbreviations do not
> merely fail to help, they misdirect. In a language where a misread clause
> yields a vacuous proof rather than a compile error, that is a safety property.

Three rules follow, and between them they decided every name below:

1. **Full words**, except where a symbol *is* the concept.
2. **Every clause is a third-person-singular verb whose subject is the item**, so
   a clause reads as a sentence with the subject elided.
3. **A thing that is not a claim about behaviour is not a clause.** The effect
   row belongs to the arrow; modifiers are adjectives on an item.

Status marks below say what a construct costs to adopt:

| mark | meaning |
|---|---|
| **anchor** | in [RFC-6](0006-full-words.md), the syntax-only change. No new expressive power |
| **later** | a capability, named with the RFC that would carry it |
| **research** | reachable surface, open discharge |

---

## 3. Functions

```thermite
fn allocate(pages: u64) -> Result<Grant, u64>
  ! write(heap), cost(12 * pages + 40)
  requires {
    pages > 0;
    pages <= 1024;
  }
  ensures match result {
    Ok(g)  => g.len == pages * 4096 && plan_ok(current_plan()),
    Err(e) => true,
  } by unfold(plan_ok)
```

The signature, then the effect row, then the clauses. Ordering is mandatory:

```
1. the effect row      — it is part of the type
2. bare clauses        — requires, ensures, survives
3. blocks              — interleaves { }
4. measures            — last, as Verus places `decreases`
```

Nothing semantic has to be known to check that order. **anchor**

## 4. Clauses

| clause | on | says | status |
|---|---|---|---|
| `requires P` | fn | holds at entry | **anchor** (from `req`) |
| `ensures P` | fn | holds at normal exit | **anchor** (from `ens`) |
| `keeps P` | struct, enum, loop | always true of this | **anchor** (from `inv`) |
| `measures E` | spec fn, loop, recursive fn | a well-founded quantity that decreases | **anchor** (from `dec`) |
| `ensures P` | spec fn | the interface of a sealed predicate | later — spec surface |
| `survives P` | fn | holds if execution stops mid-step | later — crash clause |
| `asks R` / `promises G` | fn, inside `interleaves` | what others may do to me / what I do to them | later — interference |

A clause body may be a bare expression or a **block of conjuncts**, which is what
makes a proof hint attach per obligation rather than per clause:

```thermite
requires {
  cpu < 64;
  (s.expected >> cpu) & 1 == 1;
}
```

**anchor.** The trivial precondition is `requires nothing`. A trivial
postcondition is refused by the assurance gate — `ens true` is `EnsIsTrivial`
§7.1(a) today — so `ensures nothing` parses, says what it means, and does not
certify.

### Why `keeps` and `measures`

`requires` and `ensures` are Verus's and need no defence. The other two do:

```
f          requires  n < 100
f          ensures   result == n * 2
f          measures  p.count
the loop   keeps     acked & !expected == 0
Grant      keeps     base + len <= MAX_PHYS
```

`inv` is a **noun in a verb slot**, which is why it never sat right beside
`req` and `ens`. `dec` names the expression's property rather than the clause's
purpose.

`measures` also fixes a mechanical problem. A clause keyword is a
semantic-address segment, and `validate_segments` matches a fixed allowlist after
splitting on `.`, so a clause keyword must be **one word**. That is what ruled
out `terminates by`, which is rejected as malformed before any lookup. The
near-miss worth recording is `variant` — the dual of a loop invariant, and
Eiffel's word — which dies on enum variants.

## 5. The effect row

```thermite
  ! write(heap), cost(12 * pages + 40)
  ! pure
  ! blocks
```

Type-level, not a predicate: `() ! pure` and `() ! write(shootdown)` are
different types. The line between the row and the clauses:

> **An effect propagates up the call graph by construction. A clause is proved at
> the item.**

Two families, and the shape is the discriminator:

| family | shape | atoms | composition |
|---|---|---|---|
| state | names a region | `read(r)` `write(r)` `owns(r)` `forgets(r)` | union |
| control | describes the arrow | `panic` `diverge` `blocks` `cost(E)` `random` | or; sum for `cost` |

**The rule for which name a row carries:** a `shared` thing is named, a
`resource` thing is not, because ownership already gives exclusivity. So global
state needs `write(shootdown)` and a channel endpoint needs only `blocks`.

`alloc`, `time` and `rand` all touch ambient mutable state; declaring that state
removes them as special atoms and lets one conflict rule cover them uniformly.
That has a consequence worth stating: making the heap a region means every
allocating function conflicts with every other, so allocation serialises. That is
correct for a single global allocator and currently invisible — and it makes the
fix expressible, since per-CPU heaps are separate regions.

### An effect is an algebraic theory

Each label owes three things, and the third is what pays off:

| | what it decides |
|---|---|
| **theory** | which equations the prover may use when reasoning through the effect |
| **composition** | what a caller inherits — union, or, sum |
| **commutation** | whether two of them may run concurrently |

Commutation makes the data-race-freedom table a **theorem rather than an axiom**:

```
read(r) ∥ read(r)    accept    get commutes with get
read(r) ∥ write(r)   reject    get does not commute with put
write(a) ∥ write(b)  accept    independent instances are independent theories
random  ∥ random     accept    independent samples commute
```

The admissibility criterion is that **a primitive effect is admissible when it
generates a frame condition expressible in the prover's logic.** That is a real
filter: it moved `exception` out of the row and into the type, on the checkable
ground that Thermite has no unwinding — no `catch`, `try`, `throw` or `raise`
anywhere in the surface, and `panic` is abort. A recoverable failure is decided
at the call site, so it belongs in `Result<T, M>`.

It also kept `random` differently. A PRNG is not probability: `next(seed)` is
deterministic and satisfies the state equations, so the pseudorandom case is
`state(entropy)`. What is not state is the modelling assumption that a value is
drawn from a distribution. Forcing that stipulation makes an incompatibility
visible that is otherwise invisible — an argument assuming true randomness cannot
be discharged against an implementation declared `state(entropy)`.

User-declared effects, as combinations of a fixed basis:

```thermite
effect platform(d) = state(d)
effect journal(d)  = state(d) + exception
```

The prover only ever sees primitives it knows how to encode, and the conflict
rule, composition law and frame conditions are generated rather than hardcoded.
Today `platform(memory)` gets no conflict checking at all, because nothing knows
what it is. **later — effect algebra, then verified effect rows**

## 6. Specification vocabulary

```thermite
opaque spec fn plan_ok(p: Plan) -> bool
  ensures   !result || p.count > 0     // the interface, visible while sealed
  measures  p.count
{ ...expensive, quantifies over p.regions... }
```

`opaque` seals the body; consumers reason from `ensures`. Establishing a sealed
predicate costs an unfold, declared where a reader sees it:

```thermite
  ensures match result { Ok(p) => plan_ok(p), Err(e) => true }
          by unfold(plan_ok)
```

**The asymmetry is the design**: assuming an opaque predicate is free,
establishing one costs an unfold. `by` is per-obligation and extensible — other
hints join without new syntax, and it has strong priors from Isabelle and Lean.
This is also the only viable granularity: Thermite has no `assert` and no proof
blocks, so there is no way to interleave proof steps in a body.
**later — spec surface**

## 7. Data and resources

```thermite
struct Shoot { epoch: u64, acked: u64, expected: u64 }
  keeps acked & !expected == 0

enum Walk { Ready { addr: u64 }, Pending { level: u32, index: u64 } }

resource struct Grant { base: u64, len: u64, generation: u64 }
  keeps base + len <= MAX_PHYS
```

**anchor** for `keeps`. Structural predicates over recursive data and quantified
predicates over collections are already filed as defects rather than requested as
features — #124, #125, #126 — because the language already intends to support
them.

A `resource` binding is consumed on every path **that returns**, scoped exactly
as `ensures` is. Contagion is declared and checked: a struct or enum reachable to
a resource is itself a resource. Abandonment is an operation rather than a hole:

```thermite
fn teardown(g: Grant) -> ()
  ! forgets(heap)
  requires  nothing
  ensures   nothing
{ forget(g) }
```

`resource` names what the value **is** rather than what happens to it, which is
why it beat `linear` (names the mathematics), `once` and `used` (adverb and
participle in an adjective slot), and `undroppable` (a negation naming the
prohibition). Both halves follow from the kind: a copy would be a second claim on
the same thing, and one you fail to release is a **resource leak**, which is the
named failure in every systems vocabulary there is. **later — resource types**

## 8. Shared state and locks

```thermite
shared heap:       Heap
shared scheduler:  SchedState
shared interrupts: MaskState

lock sched_lock guards SchedState;
lock frame_lock guards FrameTable after sched_lock;

fn enqueue(t: TaskId) -> ()
  ! owns(sched_lock), write(scheduler)
  requires  t < MAX_TASKS
  ensures   nothing
{ holding sched_lock { … } }
```

`shared` rather than `region`: the jargon names the mechanism, the word names the
safety-relevant property. Declaring it is what makes the row checkable.

The row declares **which** lock; the block declares **where**. Neither is
inferred, so an over-declared lock is an error rather than invisible. A function
holds at most one lock unless the locks are ordered with `after`.
**later — verified effect rows, then shared-state invariants**

## 9. Interference and interrupts

```thermite
fn ack(s: &mut Shoot, cpu: u64) -> ()
  ! write(shootdown)
  requires  cpu < 64
  ensures   (final(s).acked >> cpu) & 1 == 1
  interleaves {
    asks      final(s).acked | s.acked == final(s).acked;
    promises  final(s).acked == s.acked | (1 << cpu);
  }
```

For shared state read without a lock, where monotonicity is what makes the read
mean something. `asks`/`promises` outside the block is a parse error, because
`requires`/`ensures` is *also* ask-and-promise — to the caller rather than the
environment — and only the block names the party. Resolving that ambiguity the
wrong way proves something about the wrong thing, which is what earns a parse
error rather than a lint. The mechanism is the absence of a production: the words
are reserved and the only production consuming them is `interleaves { }`.

Interrupts are **not an effect**. An interrupt is preemption without parallelism,
and it decomposes into three things that already exist: the handler is the
environment, so the obligation is the ordinary rely-guarantee one; masking is a
lock, `owns(interrupts)`; and handler atomicity is one platform assumption that
belongs in the trusted base, written where a reader meets it.

```thermite
handlers { ipi_shootdown at 2, timer_isr at 1 }
```

The declaration carries a **preemption order**, so the pairwise obligation is
generated only in the direction that can happen. Nested interrupts fall out.
**later — interference clauses**

## 10. Protocols

```thermite
protocol PageRequest {
  User     { op: u32, count: u64 },
  Provider { status: u32, base: u64 },
  end
}

fn pager(c: PageRequest::Provider) -> () ! blocks
fn app(c: PageRequest::User)       -> () ! blocks
```

A sequence of turns, each labelled with whose turn it is. Roles are names rather
than keywords, so a protocol names its own; an endpoint is a path to a role, and
it is a `resource`. Termination is load-bearing: running the protocol to
completion is what consumes the endpoint. Conditional repetition is a branch,
`repeat | end`, and a branch costs a discriminant on the wire.

`dual` is a mathematician's word; `client`/`server` bakes in a topology
assumption; `Channel<send T>` misreads as a message type rather than a role. An
endpoint type generalises to any number of roles, where two verbs cannot name
three parties. **later — protocol types, which depends on resource types**

## 11. Loops and durability

```thermite
while i > 0
  keeps     i <= n
  measures  i
{
  i = i - 1;
}
```

Clauses sit between the head and the body, as a function's sit between its
signature and its body. Inside the braces they would read as statements and blend
into the first real one. **anchor** for the renames.

```thermite
fn commit(j: &mut Journal, b: Block) -> ()
  ! write(disk), panic
  requires  j.open
  ensures   final(j).committed == j.committed + 1
  survives {
    final(j).recoverable;
    final(j).committed == j.committed
      || final(j).committed == j.committed + 1;
  }
```

`survives` is the crash-time analogue of `ensures`. The logic is settled — Crash
Hoare Logic, FSCQ — and the work is the **crash model**, which is a per-device
trusted assumption. A first model: synchronous and sector-atomic, which is enough
for the journalling obligation and is **not** true of a real disk, since devices
buffer and reorder. Saying so is the point.

This is not hypothetical for the existing corpus: `examples/editor/editor.th` is
a verified editor carrying `write(output)`, and an editor that dies mid-save
guarantees nothing about the file. **later — crash clause, unscheduled**

## 12. Where this is going

Reachable surface, open discharge. Shown so the direction is legible, not
proposed.

```thermite
fn hash(seed: u64) -> u64
  ! random(Uniform)                    // distributional effects
  requires  nothing
  ensures   result is Uniform

fn dispatch(t: TaskId) -> ()
  ! write(scheduler), cost(180)        // cost discharged, not declared
  requires  t < MAX_TASKS
  ensures   nothing
```

`cost(E)` is already in the row and already composes; what is open is the
analysis that discharges it. `random` is already an atom and is designed to
accept a parameter later, so gaining `random(D)` is not a breaking change — and
that is what a cryptographic argument needs: stipulate a distribution on an
input, certify the output's.

Also out: noninterference, which is a hyperproperty about *sets* of executions;
an algebra for composing certificates; user-supplied effect equations; and effect
handlers in the Koka sense.

---

## 13. The anchor

The first proposal changes **no expressive power whatsoever**, and it is the only
thing being asked for. [RFC-6](0006-full-words.md) is the document to read.

Five renames — `req` `ens` `inv` `dec` `fx` — plus the row moving to the front,
the fixed clause order, conjunct blocks, and `requires nothing`. 547 clause sites
across 67 files, a deterministic rewrite, certificates unaffected, and the
migrated corpus certifies identically.

**One deliberate exception.** `alloc` and `rand` are abbreviations that rule 1
would rename. The anchor leaves them, because verified effect rows turns them
into `write(heap)` and `write(entropy)` anyway and renaming a token twice is
churn.

## 14. The sequence

```
1. defects                    FILED — #124, #125, #126
2. the RFC process            FILED — PR #127
3. the anchor                 FILED — PR #128, RFC-6
4. the effect algebra         what a row is; underpins 5
5. verified effect rows       the multiplier
6. shared-state invariants    locks and masking
7. resource types             the honest goal
8. interference clauses       rely-guarantee where locks are too strong
9. protocol types             server decomposition, end to end
```

Defects go first because they are the dependency, not the diplomacy: #124 blocks
every structural predicate over an ADT.

The RFC process goes second because Thermite's only RFC namespace was its issue
tracker, which models reports and cannot version a proposal.

### The dependency tree

```
                    RFC-6 full words
                            │
                    effect algebra
                            │
                    verified effect rows ──────┐
                            │                  │
                    shared-state invariants    │
                            │                  │
                    interference clauses ──────┤
                                               │
resource types ────────────────────────────────┴──→ protocol types

crash clause             independent; unscheduled
spec surface (#124-126)  independent; defects, filed first
```

Protocols need resources, because an endpoint that can be dropped abandons its
peer mid-session. Interference needs both the row and the lock discipline,
because its claim is that it is the cheaper option where those two are too
strong.

### Rung 5 is the honest goal

Everything below resource types produces a system where a real OS runs *beside*
the verified part. Reusable allocation is what `fork`, `exec`, demand paging and
copy-on-write all need, so it is the step where a verified kernel becomes
conceivable rather than a verified box around an unverified one.

Verified effect rows is a **multiplier rather than an adder**: it takes everything
already proven and makes it valid on multiple CPUs, via
data-race-freedom-implies-sequential-consistency. It is also the only step whose
check is purely syntactic — no solver, no new proof obligations.

### The cost of each step

Read in order, the work is small and gets larger:

1. Three defect reports with reproductions and located fixes. **Filed.**
2. A process proposal that migrates the existing RFCs out of issues and adds a
   front-matter field feeding the REQ registry. **Filed** as PR #127.
3. A rename with a migration tool and a corpus that still certifies. **Filed** as
   PR #128, with the spike, the counts and the certification table attached.
4. Onwards, each adds an obligation, a type, or a check, and each is a separate
   RFC that assumes the ones before it.

Steps 4 onwards are **not written up as RFCs yet, on purpose.** Each has a design
document behind it, and each stays unfiled until the direction here is answered.

## 15. The whole surface as one subsystem

Every construct above, written out as one coherent paging subsystem so it can be
read as a language rather than as a pile of decisions. **This does not certify**,
and it is not meant to: the syntax is proposed, not shipped.

```thermite
// The effect basis, spoken in the language.
effect state(r) {
  operation get() -> R
  operation put(v: R) -> ()

  law  get(); get()     == get()
  law  put(a); put(b)   == put(b)
  law  put(a); get()    == put(a); a
}

effect accrues(M) { operation add(v: M) -> () }
effect panic   { }
effect diverge { }

effect cost(n)     = accrues(nat, +, 0)
effect platform(d) = state(d)

// A `shared` thing is named in the row; a `resource` thing is not.
shared heap:       Heap
shared scheduler:  SchedState
shared shootdown:  Shoot
shared clock:      Instant
shared entropy:    Prng          // a PRNG is state — deterministic, reproducible
shared interrupts: MaskState     // the hardware enable bit

lock sched_lock guards SchedState;
lock frame_lock guards FrameTable after sched_lock;

resource struct Grant { base: u64, len: u64, generation: u64 }
  keeps base + len <= MAX_PHYS

struct Shoot { epoch: u64, acked: u64, expected: u64 }
  keeps acked & !expected == 0

enum Walk { Ready { addr: u64 }, Pending { level: u32, index: u64 } }

opaque spec fn plan_ok(p: Plan) -> bool
  ensures   !result || p.count > 0
  measures  p.count

// A resource is created here and consumed exactly once on every path that returns.
fn allocate(pages: u64) -> Result<Grant, u64>
  ! write(heap), cost(12 * pages + 40)
  requires {
    pages > 0;
    pages <= 1024;
  }
  ensures match result {
    Ok(g)  => g.len == pages * 4096 && plan_ok(current_plan()),
    Err(e) => true,
  } by unfold(plan_ok)

// Abandonment is an operation, not a hole: counted in the row.
fn teardown(g: Grant) -> ()
  ! forgets(heap)
  requires  nothing
  ensures   nothing
{ forget(g) }

// Lock-free, monotone, shared. The rely is what makes an unlocked read mean
// something: bits are only ever set, so an observation stays a lower bound.
fn ack(s: &mut Shoot, cpu: u64) -> ()
  ! write(shootdown)
  requires {
    cpu < 64;
    (s.expected >> cpu) & 1 == 1;
  }
  ensures  (final(s).acked >> cpu) & 1 == 1
  interleaves {
    asks {
      final(s).epoch == s.epoch;
      final(s).acked | s.acked == final(s).acked;
    }
    promises {
      final(s).epoch == s.epoch;
      final(s).acked == s.acked | (1 << cpu);
    }
  }

// Serialised, under a lock. The invariant may be false inside the section and
// must hold at its edges.
fn enqueue(t: TaskId) -> ()
  ! owns(interrupts), owns(sched_lock), write(scheduler)
  requires  t < MAX_TASKS
  ensures   nothing
{
  holding interrupts {
    holding sched_lock {
      // SchedState's `keeps` may be false here
    }
  }
}

// A handler is the environment, and its atomicity is a platform assumption.
fn timer_isr() -> ()
  ! owns(sched_lock), write(scheduler), read(clock)
  requires  nothing
  ensures   nothing
  interleaves {
    asks      final(scheduler) == scheduler;   // atomic — PLATFORM ASSUMPTION
    promises  final(scheduler).ticks == scheduler.ticks + 1;
  }
{ holding sched_lock { } }

handlers { timer_isr at 1 }

protocol PageRequest {
  User     { op: u32, count: u64 },
  Provider { status: u32, base: u64 },
  end
}

fn pager(c: PageRequest::Provider) -> ()
  ! blocks, write(heap)
  requires  nothing
  ensures   nothing

// An operation that does not finish in one step. The continuation is data the
// caller holds, and the postcondition carries a measure across calls.
fn advance(s: WalkState) -> Walk
  ! pure
  requires  s.level <= 4
  ensures match result {
    Walk::Ready { addr }           => s.level == 0,
    Walk::Pending { level, index } => level < s.level,
  }

// `survives` is the crash-time analogue of `ensures`.
fn commit(j: &mut Journal, b: Block) -> ()
  ! write(disk), panic
  requires {
    j.open;
    b.len <= MAX_BLOCK;
  }
  ensures   final(j).committed == j.committed + 1
  survives {
    final(j).recoverable;
    final(j).committed == j.committed
      || final(j).committed == j.committed + 1;
  }

fn drain(n: u64) -> u64
  ! pure
  requires  n <= 64
  ensures   result == 0
{
  let mut i: u64 = n;
  while i > 0
    keeps     i <= n
    measures  i
  {
    i = i - 1;
  }
  i
}

// `state(entropy)` is reproducible; `random` is the claim that nothing is known
// about the value, which is sound, weaker, and not reproducible.
fn next_token() -> u64
  ! write(entropy)
  requires  nothing
  ensures   nothing

fn sample_nonce() -> u64
  ! random
  requires  nothing
  ensures   nothing
```

---

## Appendix: what the probes established

Every verdict is a `forge check` result at `84d276e7` against Verus
`0.2026.05.24.ecee80a`. These are the facts the design was built on, and several
contradicted the documentation.

| probe | verdict |
|---|---|
| `once` `spent` `consumed` `exhausts` `terminates` `by` as function names | all L3 — every candidate keyword is a free identifier today |
| `ens` before `req`; `fx` first; a second `req` after `ens` | all three: `clause 'req' is out of order in 'f'` |
| `fx write(a_region_that_does_not_exist)` on a body touching nothing | **L3** — the row is unchecked in both directions |
| `spec fn` with no measure, recursive or not | `missing the mandatory 'dec' clause` |
| recursive `fn` with no measure and no `diverge` | rejected, naming the `fx diverge` exemption |
| the same function with `fx diverge` | accepted at **L1** — divergence is priced, not free |
| a loop with `fx diverge` on the enclosing fn and no measure | `missing the mandatory 'dec' clause` |
| `req true` | certifies at **L3** |
| `ens true` | **rejected** — `EnsIsTrivial`, §7.1(a) |
| address `double.ens`, `small.dec` | *no such address* |
| address `double.terminates by` | *malformed address* |

The `fx diverge` loop exemption gap is worth a defect report on its own: an
intentionally infinite loop — a scheduler idle loop, the most ordinary construct
in a kernel — is only writable by supplying a measure that cannot decrease, and
the false measure then sits in the source where a later reader will believe it.
