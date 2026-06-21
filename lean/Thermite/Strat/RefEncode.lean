/-
  Thermite/Strat/RefEncode.lean — `sencode`, the stratified reference encoder: an
  admitted classifier-surface formula (`Cls.Frm`) → the trigger-free MBQI SMT token
  surface (`Tok`, `Strat/TokDenote.lean`), with the fresh-name discipline and the
  encoder's well-formedness checks.

  Governing design: `.design/stage2-stratified-cage.md` REQ-5 / AC-5. See
  `Strat/TokDenote.lean` for the two-syntax bridge decision (option B: encode
  `Cls.Frm` directly against the structural denotation `fdenote`).

  THE FRESH-NAME DISCIPLINE (de Bruijn LEVELS). `sencode` names each binder by its
  de Bruijn LEVEL — the current binder depth `d` — and rewrites a body occurrence
  of de Bruijn index `i` (at depth `d`) to the NAME of the binder it refers to,
  `encName d i = d - 1 - i` (index 0 ↦ the innermost binder, name `d-1`). Because
  the depth strictly increases under each binder, the names assigned strictly
  increase down every root-to-leaf path, so they are pairwise distinct on any path:
  no capture. `tokWf` checks exactly this invariant (`d ≤ name` and the body is
  `tokWf (name+1)`), plus the `triggerFree` (MBQI) flag; `strat_ref_wf`
  (`Strat/Soundness.lean`) proves `sencode` satisfies it, and `PinStratCapture`
  exhibits the unsound name-reusing neighbour.

  The terms / atoms are otherwise carried through unchanged (the SMT function
  symbols are the Cls term constructors `read`/`len`/`cast`/…); only the VARIABLE
  references are relabelled index → level. So the entire soundness content lives in
  the binder bookkeeping — the fresh-name discipline T1-S certifies.

  Core-Lean-only: imports only `Strat/TokDenote.lean` (transitively `Strat/Nnf`,
  `Strat/Carrier`, the Mathlib-free `Thermite.Ast`). No Mathlib.
-/
import Thermite.Strat.TokDenote

namespace Thermite.Strat

open Thermite.Strat.Cls

/-! ## The fresh-name map (de Bruijn index → de Bruijn LEVEL) -/

/-- The SMT NAME (de Bruijn level) that index `i` denotes at binder depth `d`:
    `encName d i = d - 1 - i`. At depth `d` the binders in scope were introduced
    with names `0, 1, …, d-1` (each named by the depth at its introduction), so
    index `0` (the innermost) maps to name `d-1` and index `i` to name `d-1-i`. -/
def encName (d i : Nat) : Nat := d - 1 - i

/-! ## The encoder over terms / atoms / formulas

    Terms and atoms relabel variable references index → level (no binders inside
    terms, so the depth is constant across the term recursion). Formulas name each
    binder by the current depth `d` and recurse the body at `d+1`, flagging every
    quantifier `triggerFree := true` (the MBQI surface). -/

def encTm (d : Nat) : Tm → Tm
  | .var s i      => .var s (encName d i)
  | .lit s        => .lit s
  | .read e sq ix => .read e (encTm d sq) (encTm d ix)
  | .len sq       => .len (encTm d sq)
  | .cast to t    => .cast to (encTm d t)
  | .idxOp t k    => .idxOp (encTm d t) k
  | .mul t u      => .mul (encTm d t) (encTm d u)
  | .app1 a r f t => .app1 a r f (encTm d t)

def encAtom (d : Nat) : Atom → Atom
  | .rel ρ t u => .rel ρ (encTm d t) (encTm d u)
  | .qfree e   => .qfree e

/-- The reference encoder, depth-indexed. Each binder is named by the current
    depth (its de Bruijn level — the fresh-name discipline) and flagged trigger-free
    (the MBQI surface). -/
def sencodeAt (d : Nat) : Frm → Tok
  | .atom a   => .atom (encAtom d a)
  | .neg φ    => .neg (sencodeAt d φ)
  | .conj φ ψ => .conj (sencodeAt d φ) (sencodeAt d ψ)
  | .disj φ ψ => .disj (sencodeAt d φ) (sencodeAt d ψ)
  | .imp φ ψ  => .imp (sencodeAt d φ) (sencodeAt d ψ)
  | .all s φ  => .all s d true (sencodeAt (d + 1) φ)
  | .ex s φ   => .ex s d true (sencodeAt (d + 1) φ)

/-- The reference encoder at top level (depth 0). -/
def sencode (φ : Frm) : Tok := sencodeAt 0 φ

/-! ## Well-scopedness of the source (the de Bruijn closedness the bridge relies on)

    `wfFrm d φ` holds when every free de Bruijn index in `φ` is `< d` — i.e. `φ` is
    well-scoped under `d` enclosing binders. A top-level admitted clause is a
    SENTENCE (`wfFrm 0`): the stratification keeps carrier variables only in
    bound positions, so a clause has no free carrier index. T1-S is stated under
    this hypothesis. -/

def wfTm (d : Nat) : Tm → Bool
  | .var _ i      => decide (i < d)
  | .lit _        => true
  | .read _ sq ix => wfTm d sq && wfTm d ix
  | .len sq       => wfTm d sq
  | .cast _ t     => wfTm d t
  | .idxOp t _    => wfTm d t
  | .mul t u      => wfTm d t && wfTm d u
  | .app1 _ _ _ t => wfTm d t

def wfAtom (d : Nat) : Atom → Bool
  | .rel _ t u => wfTm d t && wfTm d u
  | .qfree _   => true

def wfFrm (d : Nat) : Frm → Bool
  | .atom a   => wfAtom d a
  | .neg φ    => wfFrm d φ
  | .conj φ ψ => wfFrm d φ && wfFrm d ψ
  | .disj φ ψ => wfFrm d φ && wfFrm d ψ
  | .imp φ ψ  => wfFrm d φ && wfFrm d ψ
  | .all _ φ  => wfFrm (d + 1) φ
  | .ex _ φ   => wfFrm (d + 1) φ

/-! ## Well-formedness of the ENCODER OUTPUT (the fresh-name + MBQI discipline)

    `tokWf d Φ` checks that every binder name is `≥ d` and the body is `tokWf
    (name+1)` — so binder names strictly increase down every path (hence pairwise
    distinct: no capture) — AND that every quantifier is `triggerFree` (the MBQI
    surface). `strat_ref_wf` (`Strat/Soundness.lean`) proves `tokWf 0 (sencode φ)`;
    `PinStratCapture` exhibits the name-reusing neighbour this rejects. -/
def tokWf (d : Nat) : Tok → Bool
  | .atom _       => true
  | .neg φ        => tokWf d φ
  | .conj φ ψ     => tokWf d φ && tokWf d ψ
  | .disj φ ψ     => tokWf d φ && tokWf d ψ
  | .imp φ ψ      => tokWf d φ && tokWf d ψ
  | .all _ n tf φ => decide (d ≤ n) && tf && tokWf (n + 1) φ
  | .ex _ n tf φ  => decide (d ≤ n) && tf && tokWf (n + 1) φ

end Thermite.Strat
