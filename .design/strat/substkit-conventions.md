# SubstKit conventions (the SPIKE-1 deliverable)

*Source: SPIKE-1 of `.design/m0-spikes.md` (REQ-4). Proven artifact:
`lean/Thermite/Spike/SubstKit.lean` (+ micro-pin
`lean/Thermite/Spike/PinBrokenLift.lean`), built on the pinned toolchain
`lean/lean-toolchain` = `leanprover/lean4:v4.29.0`. This one-page note is the
SURVIVING artifact when `Spike/` is deleted; `lean/Thermite/Strat/Syntax.lean`
inherits the statement shapes below VERBATIM.*

The spike de-risks the stage-2 binder metatheory (risk row 1 / fallback F-A of
the metatheory sketch) by proving the two load-bearing de Bruijn lemmas —
`sdenote_push_lift` (weakening is denotation-invariant) and `sdenote_subst`
(the substitution lemma) — end to end on a 3-constructor toy formula language
(`Frm` = `atom` / `conj` / `all`) over a de Bruijn term language (`Tm` =
`var`), denoting into `Bool`.

---

## 1. Lift direction

`lift c` is **weakening**: it shifts every free index `≥ c` **up by one**,
opening a hole at cutoff `c` for a freshly-introduced binding. Indices `< c`
are untouched. The leaf rule on a single index is

```lean
def bumpIdx (c i : Nat) : Nat := if i < c then i else i + 1
```

so `liftTm c (var i) = var (bumpIdx c i)`. There is no downward lift: removal
of a binding is the job of `subst`, not `lift`.

## 2. Environment push order

Environments are **total valuations** `Env C := Nat → C`. This is the deliberate
representation choice (see §6): de Bruijn index `0` is the **most-recently-bound
(innermost)** variable; higher indices are further out. Pushing a value puts it
at index `0` and shifts the rest up:

```lean
def cons (v : C) (ρ : Env C) : Env C := fun i => match i with | 0 => v | i+1 => ρ i
```

The general "insert at depth `c`" used by the weakening lemma is the structural
generalization, with `insert 0 = cons` definitionally:

```lean
def insert : Nat → C → Env C → Env C
  | 0,     v, ρ => cons v ρ
  | c + 1, v, ρ => cons (ρ 0) (insert c v (fun i => ρ (i + 1)))
```

The single algebraic fact that makes the binder cases go through is the
`cons`/`insert` commutation (near-definitional with this `insert`):

```lean
theorem cons_insert (c : Nat) (v x : C) (ρ : Env C) :
    cons x (insert c v ρ) = insert (c + 1) v (cons x ρ)
```

## 3. Binder-traversal convention

A binder denotes by folding the carrier enumeration with `List.all`:

```lean
| .all φ, ρ => 𝓒.enum.all (fun x => sdenote 𝓒 φ (cons x ρ))
```

Traversing **into** a binder bumps the de Bruijn context by one. Concretely:

* `lift` under a binder **increments the cutoff**: `liftFrm c (all φ) = all (liftFrm (c+1) φ)`.
* `subst` under a binder **increments the index AND lifts the substituted term**:
  `substFrm j s (all φ) = all (substFrm (j+1) (liftTm 0 s) φ)`.

The off-by-one neighbor — leaving the cutoff unchanged under the binder —
is **refuted** by `PinBrokenLift.lean`: its `liftBadFrm` differs from the correct
`liftFrm` only in that one arithmetic step, and `sdenote_push_lift` is shown
**false** for it on the concrete 2-element carrier, discharged by `decide`.

## 4. Exact lemma statement shapes (verbatim-inheritable)

Both lemmas are stated with the formula as the induction target and the cutoff /
index / value / environment universally quantified after it (so the binder case's
`c → c+1` / `j → j+1`, `ρ → cons x ρ` re-instantiation type-checks):

```lean
-- weakening is denotation-invariant
theorem sdenote_push_lift (𝓒 : Carrier) (φ : Frm) :
    ∀ (c : Nat) (v : 𝓒.C) (ρ : Env 𝓒.C),
      sdenote 𝓒 (liftFrm c φ) (insert c v ρ) = sdenote 𝓒 φ ρ

-- the substitution lemma
theorem sdenote_subst (𝓒 : Carrier) (φ : Frm) :
    ∀ (j : Nat) (s : Tm) (ρ : Env 𝓒.C),
      sdenote 𝓒 (substFrm j s φ) ρ = sdenote 𝓒 φ (insert j (tdenote 𝓒 s ρ) ρ)
```

Their term-level companions (the leaf facts the `atom` case rewrites with):

```lean
theorem tdenote_liftTm (𝓒 : Carrier) (c : Nat) (v : 𝓒.C) (ρ : Env 𝓒.C) (t : Tm) :
    tdenote 𝓒 (liftTm c t) (insert c v ρ) = tdenote 𝓒 t ρ

theorem tdenote_substTm (𝓒 : Carrier) (j : Nat) (s : Tm) (ρ : Env 𝓒.C) (t : Tm) :
    tdenote 𝓒 (substTm j s t) ρ = tdenote 𝓒 t (insert j (tdenote 𝓒 s ρ) ρ)
```

**Axiom discipline (REQ-2 / AC-1), measured via spike-local `#print axioms`:**

```
'Thermite.Spike.sdenote_push_lift' depends on axioms: [propext, Quot.sound]
'Thermite.Spike.sdenote_subst'     depends on axioms: [propext, Quot.sound]
```

Both ⊆ `{propext, Classical.choice, Quot.sound}` — in fact only two of the three
(`Classical.choice` is never pulled in; `propext`/`Quot.sound` enter through the
standard `simp`/`Nat`/`List` machinery). Zero `sorry`. The probe is run as
`#print axioms` lines inside `SubstKit.lean` itself — NOT via `make audit`, whose
theorem list is fixed and must not be perturbed by Spike files.

## 5. Final lemma count

**11** named `theorem`s in `SubstKit.lean`:

| # | lemma | role |
|---|-------|------|
| 1 | `Carrier.beq_self` | `decide`-eq reflexivity (atom denotation) |
| 2 | `Carrier.beq_eq_true` | `decide`-eq ↔ propositional eq |
| 3 | `all_congr` | `List.all` respects a pointwise-equal predicate |
| 4 | `insert_apply` | closed form of `insert j w ρ i` (the master lookup lemma) |
| 5 | `insert_bumpIdx` | `insert c v ρ (bumpIdx c i) = ρ i` (weakening lookup) |
| 6 | `cons_insert` | `cons`/`insert` commutation |
| 7 | `tdenote_liftTm` | term-level weakening |
| 8 | `tdenote_substTm` | term-level substitution |
| 9 | **`sdenote_push_lift`** | **load-bearing lemma A** |
| 10 | **`sdenote_subst`** | **load-bearing lemma B** |
| 11 | `sdenote_all_iff` | binder fold ↔ genuine `∀` (consumes the finiteness witness) |

The real stage-2 `Strat/SubstKit.lean` is scoped at ~25 lemmas; the toy proving
its two hardest with 11 supporting lemmas is consistent with that estimate and
well under the 40-lemma failure threshold.

## 6. Carrier verdict

> **The hand-rolled finiteness witness kept the denotation core-Lean-only. No
> Mathlib, no `Fintype`, no `DecidableEq`/universe plumbing fight.**

The carrier is a `CarrierAssign`-lite structure bundling the opaque sort with its
finiteness as plain data — the deliberate correction to the metatheory sketch's
`CarrierAssign` (which wrote `Fintype`, a Mathlib type, contradicting the
sketch's own §4 core-Lean-only-hot-path claim, the §2/§4 tension this spike
resolves):

```lean
structure Carrier where
  C : Type
  deq : DecidableEq C          -- core Lean, `deriving`d on the concrete sort
  enum : List C                -- the enumeration
  complete : ∀ x : C, x ∈ enum -- the completeness witness (replaces `Fintype.complete`)
```

Findings, for the stage-2 `Strat/Carrier.lean` input:

* **`Fintype` is NOT needed.** The two load-bearing lemmas never touch
  finiteness at all (they hold for any carrier); the witness is consumed only by
  `sdenote_all_iff`, which upgrades the computational `enum.all` fold to the
  genuine `∀ x : C` using `complete` + core `List.all_eq_true`. Discharged with
  the hand-rolled witness, zero Mathlib.
* **One mild plumbing observation (did NOT fight back).** Because the sort `C`
  is a *field* of a bundled value `𝓒`, instance synthesis cannot find
  `DecidableEq 𝓒.C` from `𝓒` alone. The fix is to carry decidability as data
  (`deq`) and route Boolean equality through it explicitly
  (`Carrier.beq a b := @decide (a = b) (𝓒.deq a b)`), rather than relying on a
  `[DecidableEq]` instance. Stage-2 recommendation: either keep this
  carry-as-data style, or make the carrier a `class`/section-`variable` bundle
  so the `DecidableEq` field is an instance in scope. Either is core-Lean; the
  choice is ergonomic, not foundational.
* **Universes:** bundling `C : Type` puts `Carrier : Type 1`. No universe
  trouble arose — denotation and all lemmas are monomorphic at `Type`.

## Failure-signal verdict

**No trigger fired.** 11 lemmas (≤ 40), instance plumbing did not fight back
(one ergonomic carry-as-data note, §6), and the denotation stayed core-Lean-only
with the hand-rolled finiteness witness. Plain de Bruijn is **confirmed** for
stage-2 REQ-2; no fallback F-A (locally nameless / single-prefix S₂⁻) review is
required before `Strat/SubstKit.lean` is scheduled.
