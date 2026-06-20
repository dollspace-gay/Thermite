/-
  Thermite/PinBrokenLift.lean — the broken-lift micro-pin (stage-2 pin battery,
  `.design/stage2-stratified-cage.md` REQ-2 / AC-2 / REQ-10).

  What it guards: `Strat/SubstKit.lean`'s `sdenote_push_lift` (weakening is
  denotation-invariant) holds BECAUSE `Strat/Syntax.lean`'s `liftFrm` increments
  the cutoff `c → c+1` under a binder (`all`/`ex`; SPIKE-1 §3). This pin exhibits
  the off-by-one neighbour — a `lift` that leaves the cutoff UNCHANGED under the
  binder — and shows it FALSIFIES the push/lift equation on a concrete 2-element
  carrier, discharged by `decide`. That single arithmetic step is the whole
  content of the convention; the pin is the SPIKE-1 `PinBrokenLift` ported to the
  richer stratified spine (the toy `Spike/PinBrokenLift.lean` was retired in
  REQ-1).

  Authority: `Strat/SubstKit.lean` (the correct `liftFrm`, which bumps the cutoff
  under `Frm.all`). `liftBadFrm` below differs ONLY in that one step (and its
  `ex` twin), and that single difference flips a sound `false` to an unsound
  `true`.

  Style model: the in-tree `lean/Thermite/Pin*.lean` battery (e.g.
  `PinFiniteEscape.lean`) — a small concrete carrier + `decide`-checked theorems.
  Like those, this file must keep compiling. -/
import Thermite.Strat.SubstKit

namespace Thermite.PinBrokenLift

open Thermite.Strat

/-- A 2-element opaque sort, `DecidableEq` via core-Lean `deriving` (no Fintype). -/
inductive Two where
  | t0 : Two
  | t1 : Two
  deriving DecidableEq, Repr

/-- The demonstrator carrier with its hand-rolled finiteness witness. -/
def twoCarrier : CarrierAssign where
  C := Two
  deq := inferInstance
  enum := [Two.t0, Two.t1]
  complete := by intro x; cases x <;> decide

/-- A trivial QFree oracle — this pin uses only carrier-`eq` atoms, so the oracle
    is never consulted. -/
def qTrivial : QOracle := fun _ => true

/-- The broken `lift`: identical to `SubstKit`'s `liftFrm` except the `all`/`ex`
    binder cases fail to increment the cutoff (`c` instead of `c + 1`). This is the
    canonical de Bruijn off-by-one: the body of a binder is lifted as if the binder
    had not introduced a new index-0. -/
def liftBadFrm (c : Nat) : Frm → Frm
  | .atom a   => .atom (liftAtom c a)
  | .neg φ    => .neg (liftBadFrm c φ)
  | .conj φ ψ => .conj (liftBadFrm c φ) (liftBadFrm c ψ)
  | .disj φ ψ => .disj (liftBadFrm c φ) (liftBadFrm c ψ)
  | .all φ    => .all (liftBadFrm c φ)   -- BUG: should be `liftBadFrm (c + 1) φ`
  | .ex φ     => .ex (liftBadFrm c φ)    -- BUG: should be `liftBadFrm (c + 1) φ`

/-- The witness formula: `∀ x. (var 0 = var 1)` — under the binder, `var 0` is the
    bound `x` and `var 1` reads index-0 of the ambient environment. -/
def phi : Frm := Frm.all (Frm.atom (Atom.eq (Tm.var 0) (Tm.var 1)))

/-- The ambient environment: constantly `t0`. (Qualified `Strat.Env`: bare `Env`
    would resolve to the v1 `Thermite.Env` structure in the enclosing namespace.) -/
def rho : Strat.Env twoCarrier.C := fun _ => Two.t0

/-! ## The pin

    With the correct `liftFrm`, weakening is denotation-invariant
    (`sdenote_push_lift`), so on this instance both sides are `false`. With the
    broken `liftBadFrm`, the off-by-one makes the lifted body read the inserted
    value instead of the ambient one, flipping the result to `true`: the push/lift
    equation fails. All discharged by `decide` on the concrete 2-element carrier. -/

/-- The concrete counterexample data: the broken-lifted formula denotes `true`
    while the original denotes `false`, so they are not equal. The push/lift
    equation `⟦liftBad c φ⟧ (insert c v ρ) = ⟦φ⟧ ρ` is violated at `c = 0`,
    `v = t0`. -/
theorem brokenLift_counterexample :
    sdenote twoCarrier qTrivial (liftBadFrm 0 phi) (insert 0 Two.t0 rho) = true
      ∧ sdenote twoCarrier qTrivial phi rho = false := by
  decide

/-- The pin: the push/lift lemma `sdenote_push_lift` is FALSE for `liftBadFrm` —
    there is an instance on a concrete carrier where it fails. (Contrast
    `SubstKit.sdenote_push_lift`, which holds for every carrier.) -/
theorem brokenLift_breaks_push_lift :
    ¬ ∀ (φ : Frm) (c : Nat) (v : twoCarrier.C) (ρ : Strat.Env twoCarrier.C),
        sdenote twoCarrier qTrivial (liftBadFrm c φ) (insert c v ρ)
          = sdenote twoCarrier qTrivial φ ρ := by
  intro h
  have hbad := h phi 0 Two.t0 rho
  revert hbad
  decide

/-- For contrast: the correct `liftFrm` satisfies the same push/lift instance
    (both sides `false`), confirming the divergence is exactly the off-by-one
    cutoff and nothing else. -/
theorem correctLift_satisfies_push_lift :
    sdenote twoCarrier qTrivial (liftFrm 0 phi) (insert 0 Two.t0 rho)
      = sdenote twoCarrier qTrivial phi rho := by
  decide

end Thermite.PinBrokenLift
