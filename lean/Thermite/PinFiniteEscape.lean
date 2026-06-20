/-
  Thermite/PinFiniteEscape.lean — the (R1) finiteness pin (stage-2 pin battery,
  `.design/stage2-stratified-cage.md` REQ-1 / REQ-10).

  What it guards: `Strat/Denote.lean`'s `sdenote_all_iff` upgrades the `List.all`
  binder fold to a genuine `∀ x : C` — and that upgrade CONSUMES the carrier's
  `complete : ∀ x, x ∈ enum` witness. This pin exhibits the soundness escape that
  dropping completeness would permit: a fold over an INCOMPLETE enumeration
  reports `true` (it only checks the elements it lists) while the genuine `∀` is
  false. That is precisely why (R1) finite — and provably exhaustively enumerated
  — carriers are load-bearing: the admission classifier (REQ-3/REQ-4) rejects
  infinite-carrier quantifiers because no such `complete` witness exists, and
  without it the `Bool` fold cannot be trusted as a `∀`.

  Authority: `Strat/Carrier.lean` (the `complete` field) + `Strat/Denote.lean`
  (`sdenote_all_iff`, which uses it). The `CarrierAssign` structure makes an
  incomplete enumeration UNREPRESENTABLE (the `complete` field demands the
  witness), so the escape is exhibited at the raw `List.all` fold level and then
  tied back to `sdenote` on the genuine carrier.

  Style model: the in-tree `lean/Thermite/Pin*.lean` battery — a small concrete
  carrier + `decide`-checked theorems. Like those, this file must keep compiling.
-/
import Thermite.Strat.Denote

namespace Thermite.PinFiniteEscape

open Thermite.Strat

/-- A 2-element opaque sort, `DecidableEq` via core-Lean `deriving` (no Fintype). -/
inductive Two where
  | t0 : Two
  | t1 : Two
  deriving DecidableEq, Repr

/-- The genuine carrier: its `enum` lists BOTH elements, and `complete` is
    discharged by `decide` — the hand-rolled finiteness witness. -/
def twoCarrier : CarrierAssign where
  C := Two
  deq := inferInstance
  enum := [Two.t0, Two.t1]
  complete := by intro x; cases x <;> decide

/-- A trivial QFree oracle — this pin uses only carrier-`eq` atoms, so the oracle
    is never consulted. -/
def qTrivial : QOracle := fun _ => true

/-- The fold predicate "`= t0`", false at `t1`. -/
def isT0 (x : Two) : Bool := twoCarrier.beq x Two.t0

/-! ## The escape, at the fold level

    The genuine, complete enumeration correctly refutes the false `∀ x, x = t0`;
    a strict-subset ("incomplete") enumeration LIES, reporting `true`. The single
    difference is the `complete` witness — the (R1) datum. -/

/-- An incomplete enumeration: it omits `t1`. (Not constructible as a
    `CarrierAssign`, which would demand a `complete` proof this list cannot
    provide — that unrepresentability is the point.) -/
def incompleteEnum : List Two := [Two.t0]

/-- The pin: folding over the incomplete enumeration reports `true`, while the
    genuine complete enumeration reports `false`. Dropping completeness flips a
    sound `false` to an unsound `true` — the finiteness escape that the
    `complete` witness in `sdenote_all_iff` blocks. -/
theorem finiteEscape_pinned :
    incompleteEnum.all isT0 = true ∧ twoCarrier.enum.all isT0 = false := by
  decide

/-- The escape, stated against the genuine `∀`: the incomplete fold certifies
    `true` even though `∀ x, isT0 x` is FALSE (it fails at `t1`). -/
theorem incompleteEnum_escapes :
    incompleteEnum.all isT0 = true ∧ ¬ (∀ x : Two, isT0 x = true) := by
  refine ⟨by decide, ?_⟩
  intro h; have := h Two.t1; revert this; decide

/-! ## Tied back to `sdenote` on the genuine carrier

    On the real (complete) carrier, the `all` fold and the genuine `∀` agree —
    both `false` for the false claim `∀ x, x = ρ(0)`. `sdenote_all_iff` is the
    lemma making them agree, and it is exactly the consumer of `complete`. -/

/-- `∀ x. (var 0 = var 1)`: under the binder, `var 0` is the bound `x` and
    `var 1` reads index-0 of the ambient environment. -/
def phiEq : Frm := Frm.all (Frm.atom (Atom.eq (Tm.var 0) (Tm.var 1)))

/-- The ambient environment: constantly `t0`, so `var 1` reads `t0`. (Qualified
    `Strat.Env`: bare `Env` would resolve to the v1 `Thermite.Env` structure in
    the enclosing namespace.) -/
def rho0 : Strat.Env twoCarrier.C := fun _ => Two.t0

/-- Honest: `sdenote` of the false universal is `false` on the complete carrier
    (the fold checks `t1` too, where `t1 = t0` is false). -/
theorem sdenote_all_honest :
    sdenote twoCarrier qTrivial phiEq rho0 = false := by decide

/-- The upgrade in action: `sdenote_all_iff` ties the `sdenote` fold to the
    genuine `∀ x, isT0 x` — the very statement the incomplete fold (above) would
    wrongly certify. Here both sides are honestly `false`. -/
theorem sdenote_all_iff_instance :
    sdenote twoCarrier qTrivial phiEq rho0 = true ↔ ∀ x : Two, isT0 x = true := by
  have h := sdenote_all_iff twoCarrier qTrivial
    (Frm.atom (Atom.eq (Tm.var 0) (Tm.var 1))) rho0
  simpa [phiEq, sdenote, adenote, tdenote, cons, isT0, rho0] using h

end Thermite.PinFiniteEscape
