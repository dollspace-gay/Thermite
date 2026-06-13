/-
  Thermite/Spike/PinBrokenLift.lean — SPIKE-1 micro-pin (REQ-3).

  The `Pin*.lean`-style refute-a-plausibly-wrong-neighbor artifact for the
  SubstKit toy: a broken `lift` (an off-by-one cutoff shift) is
  shown to break `sdenote_push_lift` on a concrete small carrier, discharged by
  `decide`. This pins down why the cutoff must increment under a binder, the
  convention the conventions note records and `Strat/Syntax.lean` will
  inherit.

  Authority: `lean/Thermite/Spike/SubstKit.lean` (the correct `liftFrm`, which
  bumps the cutoff `c → c+1` under `Frm.all`). The broken variant below differs
  only in that one arithmetic step (it leaves the cutoff unchanged under the
  binder), and that single difference falsifies weakening-invariance.

  Style model: the in-tree `lean/Thermite/Pin*.lean` battery (e.g.
  `PinIntBottom.lean`) — a small concrete registry/env + `decide`/`simp`-checked
  theorems that pin the spine's behavior. Like those, this file must keep
  compiling.
-/
import Thermite.Spike.SubstKit

namespace Thermite.Spike.PinBrokenLift

open Thermite.Spike

/-- The broken `lift`: identical to `SubstKit.liftFrm` except the `Frm.all`
    case fails to increment the cutoff (`c` instead of `c + 1`). This is the
    canonical de Bruijn off-by-one: the body of a binder is lifted as if the
    binder had not introduced a new index-0. -/
def liftBadFrm (c : Nat) : Frm → Frm
  | .atom t u => .atom (liftTm c t) (liftTm c u)
  | .conj φ ψ => .conj (liftBadFrm c φ) (liftBadFrm c ψ)
  | .all φ    => .all (liftBadFrm c φ)   -- BUG: should be `liftBadFrm (c + 1) φ`

/-- The witness formula: `∀ x. (var 0 = var 1)` — under the binder, `var 0` is
    the bound `x` and `var 1` reads index-0 of the ambient environment. -/
def phi : Frm := Frm.all (Frm.atom (Tm.var 0) (Tm.var 1))

/-- The ambient environment: constantly `t0`. -/
def rho : Env twoCarrier.C := fun _ => Two.t0

/-! ## The pin

    With the correct `liftFrm`, weakening is denotation-invariant
    (`sdenote_push_lift`), so on this instance both sides are `false`. With the
    broken `liftBadFrm`, the off-by-one makes the lifted body read the inserted
    value instead of the ambient one, flipping the result to `true`: the
    push/lift equation fails. All discharged by `decide` on the concrete
    2-element carrier. -/

/-- The concrete counterexample data: the broken-lifted formula denotes `true`
    while the original denotes `false`, so they are not equal. The push/lift
    equation `⟦liftBad c φ⟧ (insert c v ρ) = ⟦φ⟧ ρ` is violated at `c = 0`,
    `v = t0`. -/
theorem brokenLift_counterexample :
    sdenote twoCarrier (liftBadFrm 0 phi) (insert 0 Two.t0 rho) = true
      ∧ sdenote twoCarrier phi rho = false := by
  decide

/-- The pin: the push/lift lemma `sdenote_push_lift` is false for `liftBadFrm`.
    There is an instance on a concrete carrier where it fails. (Contrast
    `SubstKit.sdenote_push_lift`, which holds for every carrier.) -/
theorem brokenLift_breaks_push_lift :
    ¬ ∀ (φ : Frm) (c : Nat) (v : twoCarrier.C) (ρ : Env twoCarrier.C),
        sdenote twoCarrier (liftBadFrm c φ) (insert c v ρ) = sdenote twoCarrier φ ρ := by
  intro h
  have hbad := h phi 0 Two.t0 rho
  revert hbad
  decide

/-- For contrast: the correct `liftFrm` satisfies the same push/lift
    instance (both sides `false`), confirming the divergence is the
    off-by-one cutoff. -/
theorem correctLift_satisfies_push_lift :
    sdenote twoCarrier (liftFrm 0 phi) (insert 0 Two.t0 rho) = sdenote twoCarrier phi rho := by
  decide

end Thermite.Spike.PinBrokenLift
