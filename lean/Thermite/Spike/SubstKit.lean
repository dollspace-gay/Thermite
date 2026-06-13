/-
  Thermite/Spike/SubstKit.lean — SPIKE-1: the de Bruijn SubstKit toy.

  Governing design: `.design/m0-spikes.md` (SPIKE-1, REQ-1/REQ-2), child of
  `.design/thermite2-program.md`. This file de-risks the stage-2 binder
  metatheory (risk row 1 / fallback F-A of the metatheory sketch) by proving
  the two load-bearing de Bruijn lemmas — `sdenote_push_lift` (weakening is
  denotation-invariant) and `sdenote_subst` (the substitution lemma) — end to
  end on a 3-constructor toy formula language, before `Strat/SubstKit.lean` is
  scheduled. The surviving artifact is the conventions note
  (`.design/strat/substkit-conventions.md`); this whole `Spike/` directory is
  deletable scaffolding (it is removed in the same change that lands
  `lean/Thermite/Strat/Syntax.lean` inheriting these conventions verbatim).

  Core-Lean-only discipline (REQ-1). No Mathlib import — not the umbrella
  module, not any Mathlib-namespaced module: the same hot-path discipline as
  `lean/Thermite/Denote.lean`. The lakefile pulls Mathlib transitively via the
  `smt` require, so a Fintype-bearing import would compile; the discipline is
  therefore enforced by intent here, not by
  the build failing. The carrier's finiteness is a hand-rolled witness: an
  enumeration `List` + a completeness proof (`∀ x, x ∈ enum`) + a `deriving`d
  `DecidableEq`, rather than Mathlib's `Fintype`. SPIKE-1 exists in part to determine
  whether finite-carrier `Bool`-denotation can stay core-Lean-only with this
  witness; the carrier verdict (see the conventions note) is a direct input
  to stage-2 `Strat/Carrier.lean`.

  Conventions (proven below; mirrored in the conventions note):
  * de Bruijn index 0 = the most-recently-bound (innermost) variable.
  * Environments are total functions `Env C := Nat → C` (a valuation). This
    choice keeps the index lemmas unconditional: there is
    no out-of-range default / `getD` partiality to carry as a side condition,
    the plumbing the spike is probing for friction.
  * `cons v ρ` pushes `v` as the new index-0 (binder-introduction); higher
    indices shift up by one. A binder `∀` denotes `enum.all (fun x => ⟦φ⟧ (cons x ρ))`.
  * `lift c` (weakening) shifts every free index `≥ c` up by one; under a
    binder the cutoff increments (`c → c+1`).
  * `subst j s` substitutes the term `s` for index `j`, de-indexing the
    variables above `j` (the binder at `j` is consumed); under a binder both
    the index and the substituted term shift (`j → j+1`, `s → lift 0 s`).
-/
namespace Thermite.Spike

/-! ## The toy term + formula language (de Bruijn) -/

/-- The de Bruijn term language: a single constructor, a variable index.
    `lift`/`subst` act here at the leaves; the binder traversal lives in `Frm`. -/
inductive Tm where
  | var (i : Nat) : Tm
  deriving DecidableEq, Repr

/-- The toy formula language — three constructors (REQ-1):
    one atom (`atom`, term equality), one connective (`conj`), one binder
    (`all`, ∀ over the carrier sort). It denotes into `Bool`. -/
inductive Frm where
  | atom (t u : Tm) : Frm
  | conj (φ ψ : Frm) : Frm
  | all (φ : Frm) : Frm
  deriving Repr

/-! ## The carrier — a `CarrierAssign`-lite with a hand-rolled finiteness witness

    REQ-1's correction to the metatheory sketch's `CarrierAssign`
    (which wrote `Fintype`, a Mathlib type). Here the opaque sort `C`
    carries its finiteness as plain data: `enum` (the enumeration) + `complete`
    (`∀ x, x ∈ enum`) + `deq` (`DecidableEq`, core Lean, `deriving`d on the
    concrete sort). No Fintype, no Mathlib. -/
structure Carrier where
  /-- The opaque carrier sort. -/
  C : Type
  /-- Core-Lean `DecidableEq` on the sort (carried as data so the `decide`-based
      `Bool` denotation never relies on instance synthesis over a value). -/
  deq : DecidableEq C
  /-- The finiteness enumeration. -/
  enum : List C
  /-- The finiteness completeness witness — the hand-rolled replacement for
      `Fintype.complete`. -/
  complete : ∀ x : C, x ∈ enum

/-- Boolean equality on a carrier, routed through the carried `DecidableEq`
    (no instance synthesis over the value `𝓒`). -/
def Carrier.beq (𝓒 : Carrier) (a b : 𝓒.C) : Bool := @decide (a = b) (𝓒.deq a b)

@[simp] theorem Carrier.beq_self (𝓒 : Carrier) (a : 𝓒.C) : 𝓒.beq a a = true := by
  simp [Carrier.beq]

theorem Carrier.beq_eq_true {𝓒 : Carrier} {a b : 𝓒.C} : 𝓒.beq a b = true ↔ a = b := by
  simp [Carrier.beq]

/-! ## Environments: total valuations `Nat → C` -/

/-- A semantic environment / valuation: total, so index lookups are total and
    the index lemmas stay unconditional. -/
abbrev Env (C : Type) := Nat → C

/-- Push `v` as the new index-0; shift everything else up. -/
def cons {C : Type} (v : C) (ρ : Env C) : Env C :=
  fun i => match i with
    | 0 => v
    | i + 1 => ρ i

/-- Insert `v` at de Bruijn position `c`, shifting indices `≥ c` up by one.
    Defined by structural recursion on the cutoff so the `cons`/`insert`
    interplay is (almost) definitional. `insert 0 = cons`. -/
def insert {C : Type} : Nat → C → Env C → Env C
  | 0,     v, ρ => cons v ρ
  | c + 1, v, ρ => cons (ρ 0) (insert c v (fun i => ρ (i + 1)))

/-! ## `lift` and `subst` on terms and formulas -/

/-- The cutoff bump on a single index: indices `< c` are untouched, indices
    `≥ c` shift up by one. -/
def bumpIdx (c i : Nat) : Nat := if i < c then i else i + 1

/-- `lift` on terms. -/
def liftTm (c : Nat) : Tm → Tm
  | .var i => .var (bumpIdx c i)

/-- `lift` on formulas. Under a binder the cutoff increments. -/
def liftFrm (c : Nat) : Frm → Frm
  | .atom t u => .atom (liftTm c t) (liftTm c u)
  | .conj φ ψ => .conj (liftFrm c φ) (liftFrm c ψ)
  | .all φ    => .all (liftFrm (c + 1) φ)

/-- `subst` on terms: replace index `j` by `s`, de-index variables above `j`. -/
def substTm (j : Nat) (s : Tm) : Tm → Tm
  | .var i => if i = j then s else if i < j then .var i else .var (i - 1)

/-- `subst` on formulas. Under a binder the index and the substituted term
    both shift (`j → j+1`, `s → liftTm 0 s`). -/
def substFrm (j : Nat) (s : Tm) : Frm → Frm
  | .atom t u => .atom (substTm j s t) (substTm j s u)
  | .conj φ ψ => .conj (substFrm j s φ) (substFrm j s ψ)
  | .all φ    => .all (substFrm (j + 1) (liftTm 0 s) φ)

/-! ## Denotation into `Bool` -/

/-- Term denotation: a variable reads the environment. -/
def tdenote (𝓒 : Carrier) : Tm → Env 𝓒.C → 𝓒.C
  | .var i, ρ => ρ i

/-- Formula denotation into `Bool`. The binder folds the hand-rolled
    enumeration with `List.all`, the only place the finiteness
    witness is consumed by the computation. -/
def sdenote (𝓒 : Carrier) : Frm → Env 𝓒.C → Bool
  | .atom t u, ρ => 𝓒.beq (tdenote 𝓒 t ρ) (tdenote 𝓒 u ρ)
  | .conj φ ψ, ρ => sdenote 𝓒 φ ρ && sdenote 𝓒 ψ ρ
  | .all φ,    ρ => 𝓒.enum.all (fun x => sdenote 𝓒 φ (cons x ρ))

/-! ## The environment-algebra lemmas (the "instance plumbing") -/

/-- `List.all` respects a pointwise-equal predicate. Proven by induction on the
    list (no `funext`) to keep the axiom footprint minimal. -/
theorem all_congr {α : Type} (l : List α) (f g : α → Bool)
    (h : ∀ x, f x = g x) : l.all f = l.all g := by
  induction l with
  | nil => rfl
  | cons a t ih => simp only [List.all_cons, h a, ih]

/-- The master lookup lemma: a closed form for `insert j w ρ i`. Everything
    else about `insert` is a corollary. Structural induction on the cutoff. -/
theorem insert_apply {C : Type} (j : Nat) (w : C) (ρ : Env C) (i : Nat) :
    insert j w ρ i = if i < j then ρ i else if i = j then w else ρ (i - 1) := by
  induction j generalizing i ρ with
  | zero =>
    cases i with
    | zero => simp [insert, cons]
    | succ k => simp [insert, cons]
  | succ j ih =>
    cases i with
    | zero => simp [insert, cons]
    | succ k =>
      rw [insert]
      simp only [cons]
      rw [ih]
      by_cases h : k < j
      · have h1 : k + 1 < j + 1 := by omega
        simp [h, h1]
      · by_cases h2 : k = j
        · subst h2
          simp
        · have h1 : ¬ (k + 1 < j + 1) := by omega
          have h3 : ¬ (k + 1 = j + 1) := by omega
          simp only [h, h2, h1, h3, if_false]
          -- target: `ρ (k - 1 + 1) = ρ (k + 1 - 1)` (simp already beta-reduced)
          congr 1
          omega

/-- `insert` at the cutoff bump recovers the original lookup (the term-level
    weakening fact). -/
theorem insert_bumpIdx {C : Type} (c : Nat) (v : C) (ρ : Env C) (i : Nat) :
    insert c v ρ (bumpIdx c i) = ρ i := by
  rw [insert_apply]
  unfold bumpIdx
  by_cases h : i < c
  · simp [h]
  · have h1 : ¬ (i + 1 < c) := by omega
    have h2 : ¬ (i + 1 = c) := by omega
    simp [h, h1, h2]

/-- `cons`/`insert` commute: pushing then inserting one deeper equals inserting
    then pushing. Near-definitional with the structural `insert`. -/
theorem cons_insert {C : Type} (c : Nat) (v x : C) (ρ : Env C) :
    cons x (insert c v ρ) = insert (c + 1) v (cons x ρ) := by
  simp [insert, cons]

/-! ## The two load-bearing lemmas (REQ-2) -/

/-- Term-level weakening: lifting a term and denoting it under an inserted
    value equals denoting the original. -/
theorem tdenote_liftTm (𝓒 : Carrier) (c : Nat) (v : 𝓒.C) (ρ : Env 𝓒.C) (t : Tm) :
    tdenote 𝓒 (liftTm c t) (insert c v ρ) = tdenote 𝓒 t ρ := by
  cases t with
  | var i => simp only [liftTm, tdenote]; exact insert_bumpIdx c v ρ i

/-- Term-level substitution: substituting a term and denoting equals denoting
    the original under an environment with the term's value inserted at `j`. -/
theorem tdenote_substTm (𝓒 : Carrier) (j : Nat) (s : Tm) (ρ : Env 𝓒.C) (t : Tm) :
    tdenote 𝓒 (substTm j s t) ρ = tdenote 𝓒 t (insert j (tdenote 𝓒 s ρ) ρ) := by
  cases t with
  | var i =>
    simp only [substTm, tdenote]
    rw [insert_apply]
    by_cases h1 : i = j
    · subst h1; simp
    · by_cases h2 : i < j
      · simp [h1, h2]
      · simp [h1, h2]

/-- `sdenote_push_lift` (REQ-2): weakening is denotation-invariant.
    Denoting a `c`-lifted formula under an environment with a fresh value
    inserted at cutoff `c` equals denoting the original. Specialized at `c = 0`
    this is the binder-introduction fact `⟦lift 0 φ⟧ (cons v ρ) = ⟦φ⟧ ρ`. -/
theorem sdenote_push_lift (𝓒 : Carrier) (φ : Frm) :
    ∀ (c : Nat) (v : 𝓒.C) (ρ : Env 𝓒.C),
      sdenote 𝓒 (liftFrm c φ) (insert c v ρ) = sdenote 𝓒 φ ρ := by
  induction φ with
  | atom t u =>
    intro c v ρ
    simp only [liftFrm, sdenote, tdenote_liftTm]
  | conj φ ψ ihφ ihψ =>
    intro c v ρ
    simp only [liftFrm, sdenote, ihφ, ihψ]
  | all φ ih =>
    intro c v ρ
    simp only [liftFrm, sdenote]
    apply all_congr
    intro x
    rw [cons_insert]
    exact ih (c + 1) v (cons x ρ)

/-- `sdenote_subst` (REQ-2): the substitution lemma. Denoting a formula
    after substituting term `s` for index `j` equals denoting the original
    under an environment with `⟦s⟧` inserted at `j`. -/
theorem sdenote_subst (𝓒 : Carrier) (φ : Frm) :
    ∀ (j : Nat) (s : Tm) (ρ : Env 𝓒.C),
      sdenote 𝓒 (substFrm j s φ) ρ = sdenote 𝓒 φ (insert j (tdenote 𝓒 s ρ) ρ) := by
  induction φ with
  | atom t u =>
    intro j s ρ
    simp only [substFrm, sdenote, tdenote_substTm]
  | conj φ ψ ihφ ihψ =>
    intro j s ρ
    simp only [substFrm, sdenote, ihφ, ihψ]
  | all φ ih =>
    intro j s ρ
    simp only [substFrm, sdenote]
    apply all_congr
    intro x
    rw [ih (j + 1) (liftTm 0 s) (cons x ρ)]
    -- `liftTm 0 s` denoted under `cons x ρ` (= `insert 0 x ρ`) recovers `⟦s⟧ ρ`,
    -- then `cons`/`insert` commute to line up the two environments.
    have hs : tdenote 𝓒 (liftTm 0 s) (cons x ρ) = tdenote 𝓒 s ρ :=
      tdenote_liftTm 𝓒 0 x ρ s
    rw [hs, ← cons_insert]

/-! ## The finiteness witness carries the binder's meaning

    `sdenote (all φ)` computes `enum.all`; the `complete` witness is what
    upgrades that finite fold to `∀ x : C`. This lemma
    would fail if the carrier were not finite, and it is discharged with
    the hand-rolled witness, no `Fintype`. (Recorded in the carrier verdict.) -/
theorem sdenote_all_iff (𝓒 : Carrier) (φ : Frm) (ρ : Env 𝓒.C) :
    sdenote 𝓒 (Frm.all φ) ρ = true ↔ ∀ x : 𝓒.C, sdenote 𝓒 φ (cons x ρ) = true := by
  simp only [sdenote, List.all_eq_true]
  constructor
  · intro h x; exact h x (𝓒.complete x)
  · intro h x _; exact h x

/-! ## A demonstrator carrier (a concrete 2-element finite sort)

    Used by the bonus lemma above and by `PinBrokenLift.lean`. The finiteness
    witness is hand-rolled: `enum`, `complete` (discharged by `decide`), and a
    `deriving`d `DecidableEq` — no `Fintype`. -/

/-- A 2-element opaque sort. `DecidableEq` via core-Lean `deriving`. -/
inductive Two where
  | t0 : Two
  | t1 : Two
  deriving DecidableEq, Repr

/-- The demonstrator carrier with its hand-rolled finiteness witness. -/
def twoCarrier : Carrier where
  C := Two
  deq := inferInstance
  enum := [Two.t0, Two.t1]
  complete := by intro x; cases x <;> decide

/-! ## Spike-local axiom probe (AC-1)

    Run as part of `lake build` output, not via `make audit` (whose theorem
    list is fixed and must not be perturbed by Spike files). Each must show a
    subset of `{propext, Classical.choice, Quot.sound}`. -/
#print axioms sdenote_push_lift
#print axioms sdenote_subst

end Thermite.Spike
