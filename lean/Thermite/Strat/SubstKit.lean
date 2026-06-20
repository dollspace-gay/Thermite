/-
  Thermite/Strat/SubstKit.lean — the de Bruijn binder kit: weakening / substitution
  denotation-invariance for the stratified spine, with the syntactic lift/subst
  inverse and the binder-introduction corollaries.

  Governing design: `.design/stage2-stratified-cage.md` REQ-2 / AC-2 (child of
  `.design/thermite2-program.md`; spec of record: the stage-2 metatheory sketch,
  GH issue #2; gate G2). Builds on REQ-1's `Strat/{Syntax,Carrier,Denote}.lean`.

  THE LEMMA LIST IS FIXED FROM THE SPIKE-1 CONVENTIONS NOTE
  (`.design/strat/substkit-conventions.md`, sha256
  9acf36855e824ddeb022ad61cc601a4dbb6f217b47a99894bf8027f64bd35ba5). The two
  load-bearing lemmas (`sdenote_push_lift`, `sdenote_subst`) and their term-level
  companions inherit the note's §4 statement shapes VERBATIM — the formula as the
  induction target, with the cutoff / index / value / environment universally
  quantified AFTER it, so the binder case's `c → c+1` / `j → j+1`, `ρ → cons x ρ`
  re-instantiation type-checks. The QFree oracle `q` is threaded as FIXED context
  before the formula (the note's §denote convention: it leaves the binder algebra
  unaffected). The `cons`/`insert` commutation (`cons_insert`) and the
  carry-as-data decidability routing (`CarrierAssign.beq`, NOT a `[DecidableEq]`
  instance — note §6) are likewise inherited. SPIKE-1 proved this end to end on a
  3-constructor toy in 11 supporting lemmas (≤ the 40-lemma F-A trigger): plain de
  Bruijn is confirmed, no fallback review required.

  Count, honestly (note §5 scopes the real kit at ~25): REQ-1 already shipped four
  of the kit's pieces — `CarrierAssign.beq_self` / `CarrierAssign.beq_eq_true`
  (`Strat/Carrier.lean`) and `sdenote_all_iff` / `sdenote_ex_iff`
  (`Strat/Denote.lean`). This file carries the remaining binder lemmas. The toy's
  3-constructor language grows to six (`neg`/`disj`/`ex` added) over two atom kinds
  (`eq`/`qf`), so the toy's single binder case becomes the `all` AND `ex` cases,
  the atom case factors through `adenote_liftAtom` / `adenote_substAtom`, and the
  syntactic lift/subst inverse (`substTm_liftTm` → `substAtom_liftAtom` →
  `substFrm_liftFrm`) underwrites the freshness corollary the encoder (REQ-5)
  consumes — none of it padding.

  Core-Lean-only (AC-2): imports only `Strat/Denote` (transitively `Strat/Syntax`,
  `Strat/Carrier`, the Mathlib-free `Thermite.Denote`). No Mathlib, no `Fintype`.
  The micro-pin refuting the off-by-one `lift` lives in `Thermite/PinBrokenLift.lean`
  (the flat `Pin*.lean` battery placement REQ-1 used for `PinFiniteEscape`).

  Axiom discipline: the `#print axioms` lines at the end probe the two load-bearing
  lemmas in-file (note §4's instruction — NOT via `make audit`, whose THEOREM list
  is fixed and must not be perturbed). Each must show a subset of
  `{propext, Classical.choice, Quot.sound}`; zero `sorry`.
-/
import Thermite.Strat.Denote

namespace Thermite.Strat

/-! ## The environment-algebra lemmas (the "instance plumbing", note §2/§3) -/

/-- `List.all` respects a pointwise-equal predicate. Proven by induction on the
    list (no `funext`) to keep the axiom footprint minimal. (Note §5 #3.) -/
theorem all_congr {α : Type} (l : List α) (f g : α → Bool)
    (h : ∀ x, f x = g x) : l.all f = l.all g := by
  induction l with
  | nil => rfl
  | cons a t ih => simp only [List.all_cons, h a, ih]

/-- `List.any` respects a pointwise-equal predicate — the `ex`-binder companion of
    `all_congr` (the richer spine's `ex` constructor needs it where the toy, with
    only `all`, did not). -/
theorem any_congr {α : Type} (l : List α) (f g : α → Bool)
    (h : ∀ x, f x = g x) : l.any f = l.any g := by
  induction l with
  | nil => rfl
  | cons a t ih => simp only [List.any_cons, h a, ih]

/-- The master lookup lemma: a closed form for `insert j w ρ i`. Everything else
    about `insert` is a corollary. Structural induction on the cutoff.
    (Note §5 #4.) -/
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
          congr 1
          omega

/-- `insert` at the cutoff bump recovers the original lookup (the term-level
    weakening fact). (Note §5 #5.) -/
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
    then pushing. Near-definitional with the structural `insert`. (Note §2/§5 #6.) -/
theorem cons_insert {C : Type} (c : Nat) (v x : C) (ρ : Env C) :
    cons x (insert c v ρ) = insert (c + 1) v (cons x ρ) := by
  simp [insert, cons]

/-! ## Term-level weakening / substitution (note §4, verbatim shapes) -/

/-- Term-level weakening: lifting a term and denoting it under an inserted value
    equals denoting the original. (Note §4 / §5 #7.) -/
theorem tdenote_liftTm (𝓒 : CarrierAssign) (c : Nat) (v : 𝓒.C) (ρ : Env 𝓒.C) (t : Tm) :
    tdenote 𝓒 (liftTm c t) (insert c v ρ) = tdenote 𝓒 t ρ := by
  cases t with
  | var i => simp only [liftTm, tdenote]; exact insert_bumpIdx c v ρ i

/-- Term-level substitution: substituting a term and denoting equals denoting the
    original under an environment with the term's value inserted at `j`.
    (Note §4 / §5 #8.) -/
theorem tdenote_substTm (𝓒 : CarrierAssign) (j : Nat) (s : Tm) (ρ : Env 𝓒.C) (t : Tm) :
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

/-- The syntactic term-level lift/subst inverse: substituting at the very cutoff a
    term was lifted past is the identity (the freshly-introduced index does not
    occur). Underwrites the freshness corollary `substFrm_liftFrm` below. -/
theorem substTm_liftTm (c : Nat) (s t : Tm) : substTm c s (liftTm c t) = t := by
  cases t with
  | var i =>
    simp only [liftTm, bumpIdx]
    by_cases h : i < c
    · simp only [substTm, if_pos h, if_neg (show ¬ i = c by omega)]
    · simp only [substTm, if_neg h, if_neg (show ¬ i + 1 = c by omega),
        if_neg (show ¬ i + 1 < c by omega), Nat.add_sub_cancel]

/-! ## Atom-level weakening / substitution (the `eq`/`qf` stratification split)

    The richer spine's atoms split into a carrier-sort equality `eq` (which `lift`/
    `subst` act on) and a carrier-closed `qf` embedding (which they pass through
    unchanged). These two lemmas factor the formula `atom` case. -/

/-- Atom-level weakening. The `eq` atom rewrites through `tdenote_liftTm` twice;
    the `qf` atom is carrier-closed, so both sides read the oracle at the same
    expression. -/
theorem adenote_liftAtom (𝓒 : CarrierAssign) (q : QOracle) (c : Nat) (v : 𝓒.C)
    (ρ : Env 𝓒.C) (a : Atom) :
    adenote 𝓒 q (liftAtom c a) (insert c v ρ) = adenote 𝓒 q a ρ := by
  cases a with
  | eq t u => simp only [liftAtom, adenote, tdenote_liftTm]
  | qf e => rfl

/-- Atom-level substitution. The `eq` atom rewrites through `tdenote_substTm`
    twice; the `qf` atom is carrier-closed and untouched. -/
theorem adenote_substAtom (𝓒 : CarrierAssign) (q : QOracle) (j : Nat) (s : Tm)
    (ρ : Env 𝓒.C) (a : Atom) :
    adenote 𝓒 q (substAtom j s a) ρ = adenote 𝓒 q a (insert j (tdenote 𝓒 s ρ) ρ) := by
  cases a with
  | eq t u => simp only [substAtom, adenote, tdenote_substTm]
  | qf e => rfl

/-- The syntactic atom-level lift/subst inverse (lifts `substTm_liftTm` to atoms). -/
theorem substAtom_liftAtom (c : Nat) (s : Tm) (a : Atom) :
    substAtom c s (liftAtom c a) = a := by
  cases a with
  | eq t u => simp only [liftAtom, substAtom, substTm_liftTm]
  | qf e => rfl

/-! ## The two load-bearing lemmas (note §4, verbatim shapes)

    `q` is fixed context; the formula `φ` is the induction target; the cutoff /
    index / value / environment are quantified after it. The `atom` case factors
    through the atom lemmas above; the `all`/`ex` binder cases use `cons_insert`
    to line up the two environments and `all_congr`/`any_congr` to rewrite under
    the fold. -/

/-- `sdenote_push_lift` (REQ-2): weakening is denotation-invariant. Denoting a
    `c`-lifted formula under an environment with a fresh value inserted at cutoff
    `c` equals denoting the original. (Note §4 / §5 #9.) -/
theorem sdenote_push_lift (𝓒 : CarrierAssign) (q : QOracle) (φ : Frm) :
    ∀ (c : Nat) (v : 𝓒.C) (ρ : Env 𝓒.C),
      sdenote 𝓒 q (liftFrm c φ) (insert c v ρ) = sdenote 𝓒 q φ ρ := by
  induction φ with
  | atom a =>
    intro c v ρ
    simp only [liftFrm, sdenote, adenote_liftAtom]
  | neg φ ih =>
    intro c v ρ
    simp only [liftFrm, sdenote, ih]
  | conj φ ψ ihφ ihψ =>
    intro c v ρ
    simp only [liftFrm, sdenote, ihφ, ihψ]
  | disj φ ψ ihφ ihψ =>
    intro c v ρ
    simp only [liftFrm, sdenote, ihφ, ihψ]
  | all φ ih =>
    intro c v ρ
    simp only [liftFrm, sdenote]
    apply all_congr
    intro x
    rw [cons_insert]
    exact ih (c + 1) v (cons x ρ)
  | ex φ ih =>
    intro c v ρ
    simp only [liftFrm, sdenote]
    apply any_congr
    intro x
    rw [cons_insert]
    exact ih (c + 1) v (cons x ρ)

/-- `sdenote_subst` (REQ-2): the substitution lemma. Denoting a formula after
    substituting term `s` for index `j` equals denoting the original under an
    environment with `⟦s⟧` inserted at `j`. (Note §4 / §5 #10.) -/
theorem sdenote_subst (𝓒 : CarrierAssign) (q : QOracle) (φ : Frm) :
    ∀ (j : Nat) (s : Tm) (ρ : Env 𝓒.C),
      sdenote 𝓒 q (substFrm j s φ) ρ = sdenote 𝓒 q φ (insert j (tdenote 𝓒 s ρ) ρ) := by
  induction φ with
  | atom a =>
    intro j s ρ
    simp only [substFrm, sdenote, adenote_substAtom]
  | neg φ ih =>
    intro j s ρ
    simp only [substFrm, sdenote, ih]
  | conj φ ψ ihφ ihψ =>
    intro j s ρ
    simp only [substFrm, sdenote, ihφ, ihψ]
  | disj φ ψ ihφ ihψ =>
    intro j s ρ
    simp only [substFrm, sdenote, ihφ, ihψ]
  | all φ ih =>
    intro j s ρ
    simp only [substFrm, sdenote]
    apply all_congr
    intro x
    rw [ih (j + 1) (liftTm 0 s) (cons x ρ)]
    have hs : tdenote 𝓒 (liftTm 0 s) (cons x ρ) = tdenote 𝓒 s ρ :=
      tdenote_liftTm 𝓒 0 x ρ s
    rw [hs, ← cons_insert]
  | ex φ ih =>
    intro j s ρ
    simp only [substFrm, sdenote]
    apply any_congr
    intro x
    rw [ih (j + 1) (liftTm 0 s) (cons x ρ)]
    have hs : tdenote 𝓒 (liftTm 0 s) (cons x ρ) = tdenote 𝓒 s ρ :=
      tdenote_liftTm 𝓒 0 x ρ s
    rw [hs, ← cons_insert]

/-! ## The syntactic lift/subst inverse, lifted to formulas (freshness) -/

/-- The syntactic formula-level lift/subst inverse: substituting at the cutoff a
    formula was lifted past returns the original. The de Bruijn statement of
    freshness — the introduced index does not occur, so substituting for it is the
    identity (the substance of REQ-5's fresh-name discipline). -/
theorem substFrm_liftFrm (φ : Frm) :
    ∀ (c : Nat) (s : Tm), substFrm c s (liftFrm c φ) = φ := by
  induction φ with
  | atom a => intro c s; simp only [liftFrm, substFrm, substAtom_liftAtom]
  | neg φ ih => intro c s; simp only [liftFrm, substFrm, ih]
  | conj φ ψ ihφ ihψ => intro c s; simp only [liftFrm, substFrm, ihφ, ihψ]
  | disj φ ψ ihφ ihψ => intro c s; simp only [liftFrm, substFrm, ihφ, ihψ]
  | all φ ih => intro c s; simp only [liftFrm, substFrm, ih]
  | ex φ ih => intro c s; simp only [liftFrm, substFrm, ih]

/-! ## Binder-introduction corollaries (the `c = 0` / `j = 0` instances)

    These are the forms the consumers actually cite: `insert 0 = cons`
    definitionally, so the cutoff-`0` push-lift is binder-introduction weakening
    and the index-`0` subst is instantiation (β). -/

/-- Binder-introduction weakening: a `0`-lifted formula under a freshly `cons`'d
    value equals denoting the original. (`sdenote_push_lift` at `c = 0`.) -/
theorem sdenote_lift0 (𝓒 : CarrierAssign) (q : QOracle) (φ : Frm) (v : 𝓒.C)
    (ρ : Env 𝓒.C) :
    sdenote 𝓒 q (liftFrm 0 φ) (cons v ρ) = sdenote 𝓒 q φ ρ := by
  have h := sdenote_push_lift 𝓒 q φ 0 v ρ
  simpa only [insert] using h

/-- Instantiation (β): substituting term `s` for index `0` equals denoting the
    body under `⟦s⟧` `cons`'d on. (`sdenote_subst` at `j = 0`.) -/
theorem sdenote_subst0 (𝓒 : CarrierAssign) (q : QOracle) (φ : Frm) (s : Tm)
    (ρ : Env 𝓒.C) :
    sdenote 𝓒 q (substFrm 0 s φ) ρ = sdenote 𝓒 q φ (cons (tdenote 𝓒 s ρ) ρ) := by
  have h := sdenote_subst 𝓒 q φ 0 s ρ
  simpa only [insert] using h

/-- Semantic freshness: substituting at the cutoff a formula was lifted past is
    denotation-invariant (the corollary of `substFrm_liftFrm` the encoder cites). -/
theorem sdenote_substFrm_liftFrm (𝓒 : CarrierAssign) (q : QOracle) (φ : Frm)
    (c : Nat) (s : Tm) (ρ : Env 𝓒.C) :
    sdenote 𝓒 q (substFrm c s (liftFrm c φ)) ρ = sdenote 𝓒 q φ ρ := by
  rw [substFrm_liftFrm]

/-! ## In-file axiom probe (note §4)

    Run as part of `lake build` output, NOT via `make audit` (whose THEOREM list
    is fixed and must not be perturbed by stage-2 build targets). Each must show a
    subset of `{propext, Classical.choice, Quot.sound}` — zero `sorry`. -/
#print axioms sdenote_push_lift
#print axioms sdenote_subst

end Thermite.Strat
