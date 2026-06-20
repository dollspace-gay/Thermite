/-
  Thermite/Strat/Syntax.lean — the stratified-FOL spine: the de Bruijn term /
  atom / formula language (`Tm` / `Atom` / `Frm`) with `lift`/`subst`.

  Governing design: `.design/stage2-stratified-cage.md` REQ-1 / AC-1 (child of
  `.design/thermite2-program.md`; spec of record: the stage-2 metatheory sketch,
  GH issue #2). This is the `Strat/` sibling namespace to the v1 spine
  (`Thermite.Ast`/`Thermite.Denote`), built against the REQ-0 surface
  quantifiers (`Expr::Quantifier`, merged at 60fd029e).

  The de Bruijn conventions are inherited VERBATIM from the SPIKE-1 deliverable
  — the surviving conventions note `.design/strat/substkit-conventions.md`
  (proven end to end on the toy `lean/Thermite/Spike/SubstKit.lean`, which this
  increment retires). The statement shapes below are the note's §1–§3:

  * `bumpIdx` / `liftTm` / `substTm` are the note's §1 leaf rules verbatim.
  * de Bruijn index 0 = the most-recently-bound (innermost) variable (§2).
  * `lift c` (weakening) shifts every free index `≥ c` up by one; under a binder
    the cutoff increments `c → c+1` (§3). The off-by-one neighbour (cutoff left
    unchanged under the binder) is refuted by SPIKE-1's `PinBrokenLift`; the
    stratified micro-pin lands with the SubstKit (REQ-2).
  * `subst j s` substitutes `s` for index `j`, de-indexing variables above `j`;
    under a binder both the index and the substituted term shift
    (`j → j+1`, `s → liftTm 0 s`) (§3).

  Stratification (the key structural choice): the carrier-sort variables — the
  things the `∀`/`∃` binders range over — live ONLY in the `Atom.eq` term
  equalities (de Bruijn `Tm`s). The `Atom.qf` leaf embeds a v1 quantifier-free
  `Thermite.Expr`, which is CLOSED with respect to the carrier binders (its free
  names are the v1 `Env`'s param/`result`/`old(x)` names, not carrier indices).
  So `lift`/`subst` shift the de Bruijn structure of the `eq` atoms and leave the
  `qf` atoms untouched — the foundation of "QFree atoms defer to the v1
  denotation" (`Strat/Denote.lean`; design Architecture §"Lean").

  Core-Lean-only on this path: the only import is `Thermite.Ast` (the v1 `Expr`
  the `qf` atom embeds), which is itself Mathlib-free. No `Fintype`, no Mathlib.
-/
import Thermite.Ast

namespace Thermite.Strat

/-! ## The de Bruijn term language

    A single constructor, a variable index into the carrier-sort environment.
    `lift`/`subst` act here at the leaves; the binder traversal lives in `Frm`.
    (SPIKE-1 `Tm`, verbatim.) -/
inductive Tm where
  | var (i : Nat) : Tm
  deriving DecidableEq, Repr

/-! ## Atoms — the quantifier-free leaves

    Two kinds, realising the stratification:
    * `eq t u` — equality of two carrier-sort terms (the part the `∀`/`∃`
      binders constrain; denoted via the carrier's `DecidableEq`).
    * `qf e` — an embedded v1 quantifier-free `Thermite.Expr`, denoted by
      DEFERRING to the existing `Thermite.denote` (the v1 arithmetic / cast /
      byte-view layers are consumed, not re-proven; `Strat/Denote.lean`). It is
      closed with respect to the carrier binders, so `lift`/`subst` pass it
      through unchanged. -/
inductive Atom where
  | eq (t u : Tm) : Atom
  | qf (e : Thermite.Expr) : Atom
  deriving Repr

/-! ## The stratified formula language

    The atoms, the propositional connectives (`neg`/`conj`/`disj` — enough for
    the NNF/prenex normaliser REQ-3 to chew on), and the two carrier-sort
    quantifiers (`all`/`ex`). -/
inductive Frm where
  | atom (a : Atom) : Frm
  | neg  (φ : Frm) : Frm
  | conj (φ ψ : Frm) : Frm
  | disj (φ ψ : Frm) : Frm
  | all  (φ : Frm) : Frm
  | ex   (φ : Frm) : Frm
  deriving Repr

/-! ## `lift` and `subst` (SPIKE-1 §1/§3, verbatim shapes) -/

/-- The cutoff bump on a single index: indices `< c` are untouched, indices
    `≥ c` shift up by one. (SPIKE-1 §1.) -/
def bumpIdx (c i : Nat) : Nat := if i < c then i else i + 1

/-- `lift` on terms. -/
def liftTm (c : Nat) : Tm → Tm
  | .var i => .var (bumpIdx c i)

/-- `subst` on terms: replace index `j` by `s`, de-index variables above `j`. -/
def substTm (j : Nat) (s : Tm) : Tm → Tm
  | .var i => if i = j then s else if i < j then .var i else .var (i - 1)

/-- `lift` on atoms: shift the `eq` terms; leave the carrier-closed `qf` atom. -/
def liftAtom (c : Nat) : Atom → Atom
  | .eq t u => .eq (liftTm c t) (liftTm c u)
  | .qf e   => .qf e

/-- `subst` on atoms: substitute in the `eq` terms; leave the `qf` atom. -/
def substAtom (j : Nat) (s : Tm) : Atom → Atom
  | .eq t u => .eq (substTm j s t) (substTm j s u)
  | .qf e   => .qf e

/-- `lift` on formulas. Under a binder (`all`/`ex`) the cutoff increments
    (SPIKE-1 §3 — the convention `PinBrokenLift` pins). -/
def liftFrm (c : Nat) : Frm → Frm
  | .atom a   => .atom (liftAtom c a)
  | .neg φ    => .neg (liftFrm c φ)
  | .conj φ ψ => .conj (liftFrm c φ) (liftFrm c ψ)
  | .disj φ ψ => .disj (liftFrm c φ) (liftFrm c ψ)
  | .all φ    => .all (liftFrm (c + 1) φ)
  | .ex φ     => .ex (liftFrm (c + 1) φ)

/-- `subst` on formulas. Under a binder the index and the substituted term both
    shift (`j → j+1`, `s → liftTm 0 s`; SPIKE-1 §3). -/
def substFrm (j : Nat) (s : Tm) : Frm → Frm
  | .atom a   => .atom (substAtom j s a)
  | .neg φ    => .neg (substFrm j s φ)
  | .conj φ ψ => .conj (substFrm j s φ) (substFrm j s ψ)
  | .disj φ ψ => .disj (substFrm j s φ) (substFrm j s ψ)
  | .all φ    => .all (substFrm (j + 1) (liftTm 0 s) φ)
  | .ex φ     => .ex (substFrm (j + 1) (liftTm 0 s) φ)

end Thermite.Strat
