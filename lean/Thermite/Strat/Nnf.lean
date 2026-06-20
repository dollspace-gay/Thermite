/-
  Thermite/Strat/Nnf.lean — the classifier surface syntax (the stratified fragment
  S₂.0 of the stage-2 metatheory §1.2), its structural denotation, and the two
  load-bearing normaliser passes: negation-normal form (`nnf`) and prenex (`prenex`),
  each with a denotation-preservation lemma (`nnf_sound` / `prenex_sound`).

  Governing design: `.design/stage2-stratified-cage.md` REQ-3 / AC-3 (child of
  `.design/thermite2-program.md`; spec of record: the stage-2 metatheory sketch,
  GH issue #2, §1 "the fragment S₂", §2 "semantics", §3.1 "the graph is computed
  from a formula in NNF — `¬∀ = ∃¬`, so NNF is load-bearing, and the NNF transform
  itself gets a denotation-preservation lemma `nnf_sound`").

  WHY A SEPARATE SYNTAX FROM REQ-1's `Strat/Syntax.lean`.  REQ-1 (#323) shipped the
  *semantic* spine: a deliberately minimal de Bruijn `Frm` (a single opaque carrier,
  unsorted `all`/`ex`, `Tm.var` only, `qf` atoms opaque) tuned for denotation, the
  SubstKit, and the encoder/soundness track.  The *classifier* needs strictly more:
  the metatheory §1.2 `Frm` carries SORTS on binders and the array-property term
  vocabulary (`Read`/`Len`/`Cast`/`IdxOp`/spec-fns), because the admission traps it
  must reject — `a[a[i]]`, the cast cycle (§3.2) — are properties of that sort/term
  structure, which REQ-1's minimal `Frm` cannot express.  So this module defines the
  classifier's own sorted surface (faithful to §1.2); REQ-1's minimal `Frm` stays the
  semantic spine, and the encoder (REQ-5) is the bridge between the two.  Both are
  `Thermite.Strat`; the classifier types live here under fresh names (`Sort₂`, the
  rich `Tm`/`Atom`/`Frm`) so they never collide with REQ-1's `Tm`/`Atom`/`Frm`
  — those stay in the `Thermite.Strat.Syntax` *open* but are not imported here.

  THE DENOTATION (§2, the finite-carrier design — structural form).  The metatheory's
  full per-sort `sdenote` (a `carrier : Sort₂ → Type` family with `Fintype` per sort)
  is the encoder increment's concern (REQ-5).  Here `nnf`/`prenex` rearrange only the
  propositional + quantifier SKELETON, so the soundness lemmas are proved against a
  STRUCTURAL denotation that erases sorts: the value domain is a finite `List` of
  closed terms (`dom`), variables are resolved by a closing substitution `ρ : Nat → Tm`,
  and atoms are read by an oracle `q : Atom → Bool` on the resulting closed atom.  This
  is honest and in fact STRONGER than a single fixed model: `nnf`/`prenex` preserve
  meaning for EVERY domain and EVERY atom interpretation, so they preserve it under the
  eventual per-sort `sdenote` too (that is one instance).  `nnf_sound` is
  unconditional; `prenex_sound` carries the standard non-empty-carrier side condition
  (`dom ≠ []`) — true of S₂ carriers (machine/opaque sorts are inhabited).

  Core-Lean-only: the only import is REQ-1's `Strat/Carrier.lean` (for the generic de
  Bruijn `cons`) and `Thermite.Ast` (the v1 `Expr` the `qf` atom embeds), both
  Mathlib-free.  No `Fintype`, no Mathlib.
-/
import Thermite.Ast
import Thermite.Strat.Carrier

namespace Thermite.Strat.Cls

/-! ## Sorts (metatheory §1.1) -/

/-- Machine sorts — finite by definition. -/
inductive Mach where
  | u8 | u16 | u32 | u64 | usize | bool
  deriving DecidableEq, Repr

/-- The stratified sort language: machine sorts, sequences, and user-declared opaque
    nominal sorts (`Key`, `Value`, … identified by a `Nat`). `seq` is never itself a
    quantifier sort (you quantify over indices and elements). -/
inductive Sort₂ where
  | mach   (m : Mach) : Sort₂
  | seq    (s : Sort₂) : Sort₂
  | opaque (k : Nat) : Sort₂
  deriving DecidableEq, Repr

/-- The `usize` index sort, the workhorse of array-property formulas. -/
abbrev usizeS : Sort₂ := .mach .usize

/-! ## Terms and atoms (metatheory §1.2)

    Terms carry their sort annotations explicitly (the classifier reads sorts off the
    syntax rather than re-running a typechecker — the ops-half Rust classifier, REQ-4,
    mirrors exactly this). The array-property vocabulary is named (`read`/`len`/`cast`/
    `idxOp`) so (R2) and the sort graph can pattern-match it; `mul` is the
    representative non-linear operation (R2 forbids a quantified index var under it);
    `app1` is a declared unary spec fn (uninterpreted in S₂; an E2 edge `arg → res`). -/
inductive Tm where
  | var   (s : Sort₂) (i : Nat) : Tm            -- de Bruijn variable, carries its sort
  | lit   (s : Sort₂) : Tm                       -- a machine literal (value irrelevant here)
  | read  (elem : Sort₂) (sq ix : Tm) : Tm      -- sq[ix] : Read SeqS elem × usize → elem
  | len   (sq : Tm) : Tm                          -- sq.len() : SeqS _ → usize
  | cast  (to : Sort₂) (t : Tm) : Tm            -- (t as `to`)
  | idxOp (t : Tm) (k : Int) : Tm                -- t ± literal k  (R2-admissible offset)
  | mul   (t u : Tm) : Tm                         -- a non-linear op (R2-forbidden over idx vars)
  | app1  (arg res : Sort₂) (f : Nat) (a : Tm) : Tm  -- declared unary spec fn
  deriving DecidableEq, Repr

/-- The sort of a term, read straight off its annotations. -/
def Tm.sortOf : Tm → Sort₂
  | .var s _      => s
  | .lit s        => s
  | .read elem _ _ => elem
  | .len _        => usizeS
  | .cast to _    => to
  | .idxOp t _    => t.sortOf
  | .mul t _      => t.sortOf
  | .app1 _ res _ _ => res

/-- Relations on machine / opaque sorts. NNF needs them only to keep `¬` on literals;
    the structural denotation reads them through the atom oracle. -/
inductive Rel where
  | eq | ne | lt | le | gt | ge
  deriving DecidableEq, Repr

/-- Atoms: a relation between two terms, or a whole v1 quantifier-free formula embedded
    (opaque to the classifier — it contributes no sorts and no graph edges, exactly as
    the metatheory §1.2 `QFree φ₀` leaf). -/
inductive Atom where
  | rel   (ρ : Rel) (t u : Tm) : Atom
  | qfree (e : Thermite.Expr) : Atom
  deriving Repr

/-- The stratified formula language with SORTED binders and `⇒` (eliminated by NNF). -/
inductive Frm where
  | atom (a : Atom) : Frm
  | neg  (φ : Frm) : Frm
  | conj (φ ψ : Frm) : Frm
  | disj (φ ψ : Frm) : Frm
  | imp  (φ ψ : Frm) : Frm
  | all  (s : Sort₂) (φ : Frm) : Frm
  | ex   (s : Sort₂) (φ : Frm) : Frm
  deriving Repr

/-! ## de Bruijn `lift` (for prenex) and closing substitution (for the denotation) -/

/-- Cutoff bump on a single index (SPIKE-1 §1, the same shape REQ-1 uses). -/
def bumpIdx (c i : Nat) : Nat := if i < c then i else i + 1

/-- `lift` on terms: shift free indices `≥ c` up by one. No binders inside terms, so the
    cutoff is constant across the term recursion. -/
def liftTm (c : Nat) : Tm → Tm
  | .var s i      => .var s (bumpIdx c i)
  | .lit s        => .lit s
  | .read e sq ix => .read e (liftTm c sq) (liftTm c ix)
  | .len sq       => .len (liftTm c sq)
  | .cast to t    => .cast to (liftTm c t)
  | .idxOp t k    => .idxOp (liftTm c t) k
  | .mul t u      => .mul (liftTm c t) (liftTm c u)
  | .app1 a r f t => .app1 a r f (liftTm c t)

/-- `lift` on atoms (the `qfree` leaf is carrier-closed, passed through). -/
def liftAtom (c : Nat) : Atom → Atom
  | .rel ρ t u => .rel ρ (liftTm c t) (liftTm c u)
  | .qfree e   => .qfree e

/-- `lift` on formulas; the cutoff increments under each binder. -/
def liftFrm (c : Nat) : Frm → Frm
  | .atom a   => .atom (liftAtom c a)
  | .neg φ    => .neg (liftFrm c φ)
  | .conj φ ψ => .conj (liftFrm c φ) (liftFrm c ψ)
  | .disj φ ψ => .disj (liftFrm c φ) (liftFrm c ψ)
  | .imp φ ψ  => .imp (liftFrm c φ) (liftFrm c ψ)
  | .all s φ  => .all s (liftFrm (c + 1) φ)
  | .ex s φ   => .ex s (liftFrm (c + 1) φ)

/-- A closing substitution: a total map from de Bruijn indices to closed terms. The
    denotation resolves every variable through one of these. -/
abbrev Subst := Nat → Tm

/-- Apply a closing substitution to a term. -/
def substTm (ρ : Subst) : Tm → Tm
  | .var _ i      => ρ i
  | .lit s        => .lit s
  | .read e sq ix => .read e (substTm ρ sq) (substTm ρ ix)
  | .len sq       => .len (substTm ρ sq)
  | .cast to t    => .cast to (substTm ρ t)
  | .idxOp t k    => .idxOp (substTm ρ t) k
  | .mul t u      => .mul (substTm ρ t) (substTm ρ u)
  | .app1 a r f t => .app1 a r f (substTm ρ t)

/-- Apply a closing substitution to an atom. -/
def substAtom (ρ : Subst) : Atom → Atom
  | .rel r t u => .rel r (substTm ρ t) (substTm ρ u)
  | .qfree e   => .qfree e

/-! ## The structural denotation (§2)

    `fdenote q dom φ ρ` is the `Bool` truth of `φ` in the closing environment `ρ`, with
    binders ranging over the finite domain `dom : List Tm` (sorts erased) and closed
    atoms read by the oracle `q`. Computable; the negative pins `decide` through it. -/
def fdenote (q : Atom → Bool) (dom : List Tm) : Frm → Subst → Bool
  | .atom a,   ρ => q (substAtom ρ a)
  | .neg φ,    ρ => !fdenote q dom φ ρ
  | .conj φ ψ, ρ => fdenote q dom φ ρ && fdenote q dom ψ ρ
  | .disj φ ψ, ρ => fdenote q dom φ ρ || fdenote q dom ψ ρ
  | .imp φ ψ,  ρ => !fdenote q dom φ ρ || fdenote q dom ψ ρ
  | .all _ φ,  ρ => dom.all (fun v => fdenote q dom φ (cons v ρ))
  | .ex _ φ,   ρ => dom.any (fun v => fdenote q dom φ (cons v ρ))

/-! ## The lift / closing-substitution cancellation (the de Bruijn lemmas)

    Resolving a `liftTm c`-lifted term under a substitution is the same as resolving the
    unlifted term under the substitution with index `c` skipped (`fun i => ρ (bumpIdx c i)`).
    The formula version, generalised over the cutoff, is the foundation of
    `prenex_sound`'s quantifier-pull steps. -/

/-- Term-level: lifting at cutoff `c` is undone by skipping index `c` in the substitution. -/
theorem substTm_liftTm (c : Nat) (ρ : Subst) (t : Tm) :
    substTm ρ (liftTm c t) = substTm (fun i => ρ (bumpIdx c i)) t := by
  induction t with
  | var s i => rfl
  | lit s => rfl
  | read e sq ix ihsq ihix => simp [liftTm, substTm, ihsq, ihix]
  | len sq ih => simp [liftTm, substTm, ih]
  | cast to t ih => simp [liftTm, substTm, ih]
  | idxOp t k ih => simp [liftTm, substTm, ih]
  | mul t u iht ihu => simp [liftTm, substTm, iht, ihu]
  | app1 a r f t ih => simp [liftTm, substTm, ih]

/-- Atom-level companion. -/
theorem substAtom_liftAtom (c : Nat) (ρ : Subst) (a : Atom) :
    substAtom ρ (liftAtom c a) = substAtom (fun i => ρ (bumpIdx c i)) a := by
  cases a with
  | rel r t u => simp [liftAtom, substAtom, substTm_liftTm]
  | qfree e => rfl

/-- The environment commutation behind the binder case: pushing `v` then skipping index
    `c+1` equals skipping index `c` then pushing `v`. -/
theorem cons_bumpIdx_succ (c : Nat) (v : Tm) (ρ : Subst) :
    (fun i => (cons v ρ) (bumpIdx (c + 1) i)) = cons v (fun i => ρ (bumpIdx c i)) := by
  funext i
  cases i with
  | zero => rfl
  | succ j =>
      simp only [cons, bumpIdx]
      by_cases h : j < c
      · have h1 : j + 1 < c + 1 := Nat.succ_lt_succ h
        simp [h, h1]
      · have h1 : ¬ (j + 1 < c + 1) := by omega
        simp [h, h1]

/-- Formula-level cancellation, generalised over the cutoff. -/
theorem fdenote_liftFrm (q : Atom → Bool) (dom : List Tm) :
    ∀ (φ : Frm) (c : Nat) (ρ : Subst),
      fdenote q dom (liftFrm c φ) ρ = fdenote q dom φ (fun i => ρ (bumpIdx c i)) := by
  intro φ
  induction φ with
  | atom a => intro c ρ; simp [liftFrm, fdenote, substAtom_liftAtom]
  | neg φ ih => intro c ρ; simp [liftFrm, fdenote, ih]
  | conj φ ψ ihφ ihψ => intro c ρ; simp [liftFrm, fdenote, ihφ, ihψ]
  | disj φ ψ ihφ ihψ => intro c ρ; simp [liftFrm, fdenote, ihφ, ihψ]
  | imp φ ψ ihφ ihψ => intro c ρ; simp [liftFrm, fdenote, ihφ, ihψ]
  | all s φ ih =>
      intro c ρ
      simp only [liftFrm, fdenote]
      apply congrArg (List.all dom)
      funext v
      rw [ih (c + 1) (cons v ρ), cons_bumpIdx_succ]
  | ex s φ ih =>
      intro c ρ
      simp only [liftFrm, fdenote]
      apply congrArg (List.any dom)
      funext v
      rw [ih (c + 1) (cons v ρ), cons_bumpIdx_succ]

/-- The cutoff-0 corollary used by `prenex_sound`: resolving `liftFrm 0 ψ` under
    `cons v ρ` is the same as resolving `ψ` under `ρ`. -/
theorem fdenote_cons_liftFrm (q : Atom → Bool) (dom : List Tm) (v : Tm) (ψ : Frm)
    (ρ : Subst) :
    fdenote q dom (liftFrm 0 ψ) (cons v ρ) = fdenote q dom ψ ρ := by
  rw [fdenote_liftFrm]
  have : (fun i => (cons v ρ) (bumpIdx 0 i)) = ρ := by
    funext i; simp [bumpIdx, cons]
  rw [this]

/-! ## De Morgan over the finite folds

    `¬∀ = ∃¬` and `¬∃ = ∀¬` at the `List.all`/`List.any` level — the load-bearing
    identities behind NNF's quantifier-duality clauses (§3.1 "NNF is load-bearing"). -/

theorem not_listAll (dom : List Tm) (P : Tm → Bool) :
    (!dom.all P) = dom.any (fun v => !P v) := by
  induction dom with
  | nil => rfl
  | cons a l ih => simp [List.all_cons, List.any_cons, Bool.not_and, ih]

theorem not_listAny (dom : List Tm) (P : Tm → Bool) :
    (!dom.any P) = dom.all (fun v => !P v) := by
  induction dom with
  | nil => rfl
  | cons a l ih => simp [List.all_cons, List.any_cons, Bool.not_or, ih]

/-! ## Negation-normal form (`nnf`) and its denotation-preservation lemma

    `nnf` pushes every negation inward to the atoms (and eliminates `⇒`), so that in the
    output every `all`/`ex` carries its true polarity syntactically — exactly what the
    sort graph (`Strat/Graph.lean`) reads. `nnfNeg φ` computes the NNF of `¬φ`. -/

mutual
def nnf : Frm → Frm
  | .atom a   => .atom a
  | .neg φ    => nnfNeg φ
  | .conj φ ψ => .conj (nnf φ) (nnf ψ)
  | .disj φ ψ => .disj (nnf φ) (nnf ψ)
  | .imp φ ψ  => .disj (nnfNeg φ) (nnf ψ)
  | .all s φ  => .all s (nnf φ)
  | .ex s φ   => .ex s (nnf φ)
def nnfNeg : Frm → Frm
  | .atom a   => .neg (.atom a)
  | .neg φ    => nnf φ
  | .conj φ ψ => .disj (nnfNeg φ) (nnfNeg ψ)
  | .disj φ ψ => .conj (nnfNeg φ) (nnfNeg ψ)
  | .imp φ ψ  => .conj (nnf φ) (nnfNeg ψ)
  | .all s φ  => .ex s (nnfNeg φ)
  | .ex s φ   => .all s (nnfNeg φ)
end

mutual
/-- `nnf` is denotation-preserving (unconditional). -/
theorem nnf_sound (q : Atom → Bool) (dom : List Tm) :
    ∀ (φ : Frm) (ρ : Subst), fdenote q dom (nnf φ) ρ = fdenote q dom φ ρ
  | .atom a, ρ => by simp [nnf, fdenote]
  | .neg φ, ρ => by simp only [nnf, fdenote, nnfNeg_sound q dom φ ρ]
  | .conj φ ψ, ρ => by simp only [nnf, fdenote, nnf_sound q dom φ ρ, nnf_sound q dom ψ ρ]
  | .disj φ ψ, ρ => by simp only [nnf, fdenote, nnf_sound q dom φ ρ, nnf_sound q dom ψ ρ]
  | .imp φ ψ, ρ => by simp only [nnf, fdenote, nnfNeg_sound q dom φ ρ, nnf_sound q dom ψ ρ]
  | .all s φ, ρ => by
      simp only [nnf, fdenote]
      apply congrArg (List.all dom); funext v; exact nnf_sound q dom φ (cons v ρ)
  | .ex s φ, ρ => by
      simp only [nnf, fdenote]
      apply congrArg (List.any dom); funext v; exact nnf_sound q dom φ (cons v ρ)
/-- `nnfNeg φ` denotes the negation of `φ`. -/
theorem nnfNeg_sound (q : Atom → Bool) (dom : List Tm) :
    ∀ (φ : Frm) (ρ : Subst), fdenote q dom (nnfNeg φ) ρ = !fdenote q dom φ ρ
  | .atom a, ρ => by simp [nnfNeg, fdenote]
  | .neg φ, ρ => by simp only [nnfNeg, fdenote, nnf_sound q dom φ ρ, Bool.not_not]
  | .conj φ ψ, ρ => by
      simp only [nnfNeg, fdenote, nnfNeg_sound q dom φ ρ, nnfNeg_sound q dom ψ ρ, Bool.not_and]
  | .disj φ ψ, ρ => by
      simp only [nnfNeg, fdenote, nnfNeg_sound q dom φ ρ, nnfNeg_sound q dom ψ ρ, Bool.not_or]
  | .imp φ ψ, ρ => by
      simp only [nnfNeg, fdenote, nnf_sound q dom φ ρ, nnfNeg_sound q dom ψ ρ, Bool.not_or,
        Bool.not_not]
  | .all s φ, ρ => by
      simp only [nnfNeg, fdenote, not_listAll]
      apply congrArg (List.any dom); funext v; exact nnfNeg_sound q dom φ (cons v ρ)
  | .ex s φ, ρ => by
      simp only [nnfNeg, fdenote, not_listAny]
      apply congrArg (List.all dom); funext v; exact nnfNeg_sound q dom φ (cons v ρ)
end

/-! ## Prenex form (`prenex`) and its denotation-preservation lemma

    `prenex` pulls every quantifier to the front (the second §8.2 layer-1 pass). The
    binder-merge helpers `mergeConj`/`mergeDisj` pull one quantifier at a time, lifting
    the other conjunct/disjunct under the new binder. The structural measure `fsize`
    (constructor count, lift-invariant) carries termination. -/

/-- Connective/binder count — invariant under `liftFrm` (unlike `sizeOf`, which counts
    the de Bruijn indices `liftFrm` shifts). -/
def fsize : Frm → Nat
  | .atom _   => 0
  | .neg φ    => fsize φ + 1
  | .conj φ ψ => fsize φ + fsize ψ + 1
  | .disj φ ψ => fsize φ + fsize ψ + 1
  | .imp φ ψ  => fsize φ + fsize ψ + 1
  | .all _ φ  => fsize φ + 1
  | .ex _ φ   => fsize φ + 1

theorem fsize_liftFrm (c : Nat) (φ : Frm) : fsize (liftFrm c φ) = fsize φ := by
  induction φ generalizing c with
  | atom a => rfl
  | neg φ ih => simp [liftFrm, fsize, ih]
  | conj φ ψ ihφ ihψ => simp [liftFrm, fsize, ihφ, ihψ]
  | disj φ ψ ihφ ihψ => simp [liftFrm, fsize, ihφ, ihψ]
  | imp φ ψ ihφ ihψ => simp [liftFrm, fsize, ihφ, ihψ]
  | all s φ ih => simp [liftFrm, fsize, ih]
  | ex s φ ih => simp [liftFrm, fsize, ih]

/-- Merge two formulas under `∧`, pulling their leading quantifiers to the front. -/
def mergeConj : Frm → Frm → Frm
  | .all s φ, ψ => .all s (mergeConj φ (liftFrm 0 ψ))
  | .ex s φ, ψ  => .ex s (mergeConj φ (liftFrm 0 ψ))
  | φ, .all s ψ => .all s (mergeConj (liftFrm 0 φ) ψ)
  | φ, .ex s ψ  => .ex s (mergeConj (liftFrm 0 φ) ψ)
  | φ, ψ        => .conj φ ψ
termination_by φ ψ => fsize φ + fsize ψ
decreasing_by
  all_goals (simp_wf; simp only [fsize, fsize_liftFrm]; omega)

/-- Merge two formulas under `∨`, pulling their leading quantifiers to the front. -/
def mergeDisj : Frm → Frm → Frm
  | .all s φ, ψ => .all s (mergeDisj φ (liftFrm 0 ψ))
  | .ex s φ, ψ  => .ex s (mergeDisj φ (liftFrm 0 ψ))
  | φ, .all s ψ => .all s (mergeDisj (liftFrm 0 φ) ψ)
  | φ, .ex s ψ  => .ex s (mergeDisj (liftFrm 0 φ) ψ)
  | φ, ψ        => .disj φ ψ
termination_by φ ψ => fsize φ + fsize ψ
decreasing_by
  all_goals (simp_wf; simp only [fsize, fsize_liftFrm]; omega)

def prenex : Frm → Frm
  | .atom a   => .atom a
  | .neg φ    => .neg (prenex φ)
  | .conj φ ψ => mergeConj (prenex φ) (prenex ψ)
  | .disj φ ψ => mergeDisj (prenex φ) (prenex ψ)
  | .imp φ ψ  => .imp (prenex φ) (prenex ψ)
  | .all s φ  => .all s (prenex φ)
  | .ex s φ   => .ex s (prenex φ)

/-! ### Soundness of the merges and of prenex (non-empty-carrier side condition)

    The quantifier-pull `(∀x.φ) ∧ ψ ≡ ∀x.(φ ∧ ψ↑)` requires the carrier to be
    inhabited (`dom ≠ []`) — the standard side condition, true of every S₂ sort. -/

/-- A `∃`-fold of the constant `false` is `false`. -/
theorem any_const_false (dom : List Tm) : dom.any (fun _ => false) = false := by
  induction dom with
  | nil => rfl
  | cons a l ih => simp [List.any_cons, ih]

/-- A `∀`-fold of the constant `true` is `true`. -/
theorem all_const_true (dom : List Tm) : dom.all (fun _ => true) = true := by
  induction dom with
  | nil => rfl
  | cons a l ih => simp [List.all_cons, ih]

/-- `∀`-fold distributes over a constant `&&` when the domain is inhabited (needed: with
    an empty domain the `∀`-fold is `true` regardless of `b`). -/
theorem all_and_distrib (dom : List Tm) (hdom : dom ≠ []) (f : Tm → Bool) (b : Bool) :
    dom.all (fun v => f v && b) = (dom.all f && b) := by
  cases b with
  | true => simp [Bool.and_true]
  | false =>
      cases dom with
      | nil => exact absurd rfl hdom
      | cons a l => simp [List.all_cons, Bool.and_false]

/-- `∃`-fold distributes over a constant `||` when the domain is inhabited. -/
theorem any_or_distrib (dom : List Tm) (hdom : dom ≠ []) (f : Tm → Bool) (b : Bool) :
    dom.any (fun v => f v || b) = (dom.any f || b) := by
  cases b with
  | false => simp [Bool.or_false]
  | true =>
      cases dom with
      | nil => exact absurd rfl hdom
      | cons a l => simp [List.any_cons, Bool.or_true]

/-- `∃`-fold distributes over a constant `&&` — unconditional (pulling `∃` out of a
    conjunction needs no inhabitation). -/
theorem any_and_distrib (dom : List Tm) (f : Tm → Bool) (b : Bool) :
    dom.any (fun v => f v && b) = (dom.any f && b) := by
  cases b with
  | true => simp [Bool.and_true]
  | false => simp only [Bool.and_false, any_const_false]

/-- `∀`-fold distributes over a constant `||` — unconditional. -/
theorem all_or_distrib (dom : List Tm) (f : Tm → Bool) (b : Bool) :
    dom.all (fun v => f v || b) = (dom.all f || b) := by
  cases b with
  | false => simp [Bool.or_false]
  | true => simp only [Bool.or_true, all_const_true]

set_option linter.unusedVariables false in
theorem mergeConj_sound (q : Atom → Bool) (dom : List Tm) (hdom : dom ≠ []) :
    ∀ (φ ψ : Frm) (ρ : Subst),
      fdenote q dom (mergeConj φ ψ) ρ = (fdenote q dom φ ρ && fdenote q dom ψ ρ) := by
  intro φ ψ
  induction φ, ψ using mergeConj.induct with
  | case1 s φ ψ ih =>
      intro ρ
      simp only [mergeConj, fdenote]
      have hbody : (fun v => fdenote q dom (mergeConj φ (liftFrm 0 ψ)) (cons v ρ))
                 = (fun v => fdenote q dom φ (cons v ρ) && fdenote q dom ψ ρ) :=
        funext fun v => by rw [ih (cons v ρ), fdenote_cons_liftFrm]
      rw [hbody, all_and_distrib dom hdom]
  | case2 s φ ψ ih =>
      intro ρ
      simp only [mergeConj, fdenote]
      have hbody : (fun v => fdenote q dom (mergeConj φ (liftFrm 0 ψ)) (cons v ρ))
                 = (fun v => fdenote q dom φ (cons v ρ) && fdenote q dom ψ ρ) :=
        funext fun v => by rw [ih (cons v ρ), fdenote_cons_liftFrm]
      rw [hbody, any_and_distrib dom]
  | case3 φ s ψ hne1 hne2 ih =>
      intro ρ
      simp only [mergeConj, fdenote]
      have hbody : (fun v => fdenote q dom (mergeConj (liftFrm 0 φ) ψ) (cons v ρ))
                 = (fun v => fdenote q dom ψ (cons v ρ) && fdenote q dom φ ρ) :=
        funext fun v => by rw [ih (cons v ρ), fdenote_cons_liftFrm, Bool.and_comm]
      rw [hbody, all_and_distrib dom hdom, Bool.and_comm]
  | case4 φ s ψ hne1 hne2 ih =>
      intro ρ
      simp only [mergeConj, fdenote]
      have hbody : (fun v => fdenote q dom (mergeConj (liftFrm 0 φ) ψ) (cons v ρ))
                 = (fun v => fdenote q dom ψ (cons v ρ) && fdenote q dom φ ρ) :=
        funext fun v => by rw [ih (cons v ρ), fdenote_cons_liftFrm, Bool.and_comm]
      rw [hbody, any_and_distrib dom, Bool.and_comm]
  | case5 φ ψ hne1 hne2 hne3 hne4 =>
      intro ρ
      simp only [mergeConj, fdenote]

set_option linter.unusedVariables false in
theorem mergeDisj_sound (q : Atom → Bool) (dom : List Tm) (hdom : dom ≠ []) :
    ∀ (φ ψ : Frm) (ρ : Subst),
      fdenote q dom (mergeDisj φ ψ) ρ = (fdenote q dom φ ρ || fdenote q dom ψ ρ) := by
  intro φ ψ
  induction φ, ψ using mergeDisj.induct with
  | case1 s φ ψ ih =>
      intro ρ
      simp only [mergeDisj, fdenote]
      have hbody : (fun v => fdenote q dom (mergeDisj φ (liftFrm 0 ψ)) (cons v ρ))
                 = (fun v => fdenote q dom φ (cons v ρ) || fdenote q dom ψ ρ) :=
        funext fun v => by rw [ih (cons v ρ), fdenote_cons_liftFrm]
      rw [hbody, all_or_distrib dom]
  | case2 s φ ψ ih =>
      intro ρ
      simp only [mergeDisj, fdenote]
      have hbody : (fun v => fdenote q dom (mergeDisj φ (liftFrm 0 ψ)) (cons v ρ))
                 = (fun v => fdenote q dom φ (cons v ρ) || fdenote q dom ψ ρ) :=
        funext fun v => by rw [ih (cons v ρ), fdenote_cons_liftFrm]
      rw [hbody, any_or_distrib dom hdom]
  | case3 φ s ψ hne1 hne2 ih =>
      intro ρ
      simp only [mergeDisj, fdenote]
      have hbody : (fun v => fdenote q dom (mergeDisj (liftFrm 0 φ) ψ) (cons v ρ))
                 = (fun v => fdenote q dom ψ (cons v ρ) || fdenote q dom φ ρ) :=
        funext fun v => by rw [ih (cons v ρ), fdenote_cons_liftFrm, Bool.or_comm]
      rw [hbody, all_or_distrib dom, Bool.or_comm]
  | case4 φ s ψ hne1 hne2 ih =>
      intro ρ
      simp only [mergeDisj, fdenote]
      have hbody : (fun v => fdenote q dom (mergeDisj (liftFrm 0 φ) ψ) (cons v ρ))
                 = (fun v => fdenote q dom ψ (cons v ρ) || fdenote q dom φ ρ) :=
        funext fun v => by rw [ih (cons v ρ), fdenote_cons_liftFrm, Bool.or_comm]
      rw [hbody, any_or_distrib dom hdom, Bool.or_comm]
  | case5 φ ψ hne1 hne2 hne3 hne4 =>
      intro ρ
      simp only [mergeDisj, fdenote]

/-- `prenex` is denotation-preserving (non-empty carrier). -/
theorem prenex_sound (q : Atom → Bool) (dom : List Tm) (hdom : dom ≠ []) :
    ∀ (φ : Frm) (ρ : Subst), fdenote q dom (prenex φ) ρ = fdenote q dom φ ρ
  | .atom a, ρ => by simp [prenex]
  | .neg φ, ρ => by simp only [prenex, fdenote, prenex_sound q dom hdom φ ρ]
  | .conj φ ψ, ρ => by
      simp only [prenex, fdenote]
      rw [mergeConj_sound q dom hdom, prenex_sound q dom hdom φ ρ, prenex_sound q dom hdom ψ ρ]
  | .disj φ ψ, ρ => by
      simp only [prenex, fdenote]
      rw [mergeDisj_sound q dom hdom, prenex_sound q dom hdom φ ρ, prenex_sound q dom hdom ψ ρ]
  | .imp φ ψ, ρ => by
      simp only [prenex, fdenote, prenex_sound q dom hdom φ ρ, prenex_sound q dom hdom ψ ρ]
  | .all s φ, ρ => by
      simp only [prenex, fdenote]
      apply congrArg (List.all dom); funext v; exact prenex_sound q dom hdom φ (cons v ρ)
  | .ex s φ, ρ => by
      simp only [prenex, fdenote]
      apply congrArg (List.any dom); funext v; exact prenex_sound q dom hdom φ (cons v ρ)

end Thermite.Strat.Cls
