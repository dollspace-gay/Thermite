/-
  Thermite/PinStratCapture.lean — the variable-capture encoder pin (stage-2 pin
  battery, `.design/stage2-stratified-cage.md` REQ-5 / AC-5 / REQ-10).

  What it guards: `Strat/Soundness.lean`'s `strat_ref_sound` (T1-S) holds because
  `Strat/RefEncode.lean`'s `sencode` obeys the FRESH-NAME discipline — each binder
  is named by its de Bruijn LEVEL, so nested binder names are distinct and a body
  variable resolves to the binder it actually refers to. This pin exhibits the
  broken neighbour that REUSES the name `0` for every binder (and relabels every
  variable to name `0`): an inner binder then SHADOWS an outer one at name `0`, so
  an outer-variable occurrence is CAPTURED by the inner binder. It shows this
  falsifies encoder soundness on a concrete 2-element domain, discharged by `decide`.

  The witness is the sentence `∀x. ∃y. x = c0`, whose truth depends only on the
  OUTER `x` — false over `{c0, c1}` (`c1 ≠ c0`). Under the capturing encoder the
  occurrence of `x` is captured by the inner `∃y`, so it reads `y` instead and the
  sentence collapses to `∀x. ∃y. y = c0`, which is true (take `y = c0`). So the
  capturing token denotes `true` where the source denotes `false`. The correct
  `sencode` agrees with the source on the same witness (via the proven
  `strat_ref_sound`), confirming the divergence is solely the lost freshness.

  Style model: the in-tree `lean/Thermite/Pin*.lean` battery (`PinFiniteEscape`,
  `PinBrokenLift`) — a small concrete carrier + `decide`-checked theorems. Like
  those, this file must keep compiling; `decide` (kernel), never `native_decide`.
-/
import Thermite.Strat.Soundness

namespace Thermite.PinStratCapture

open Thermite.Strat
open Thermite.Strat.Cls

/-! ## A concrete 2-element domain + an equality oracle -/

/-- Two distinct closed carrier terms (`c0 ≠ c1`: different constructors). -/
def c0 : Tm := .lit usizeS
def c1 : Tm := .idxOp (.lit usizeS) 1
/-- The finite quantifier domain. -/
def dom : List Tm := [c0, c1]

/-- The oracle interpreting `rel eq` as structural term equality. -/
def qEq : Atom → Bool
  | .rel .eq t u => decide (t = u)
  | _            => false

/-- An ambient substitution (unused — the witness is a closed sentence). -/
def σ0 : Subst := fun _ => c0

/-! ## The capturing encoder (the broken neighbour) -/

/-- The capture-broken term encoder: every variable is relabelled to name `0`
    (instead of its level), so it cannot distinguish nested binders. -/
def encTmCap : Tm → Tm
  | .var s _      => .var s 0
  | .lit s        => .lit s
  | .read e sq ix => .read e (encTmCap sq) (encTmCap ix)
  | .len sq       => .len (encTmCap sq)
  | .cast to t    => .cast to (encTmCap t)
  | .idxOp t k    => .idxOp (encTmCap t) k
  | .mul t u      => .mul (encTmCap t) (encTmCap u)
  | .app1 a r f t => .app1 a r f (encTmCap t)

def encAtomCap : Atom → Atom
  | .rel ρ t u => .rel ρ (encTmCap t) (encTmCap u)
  | .qfree e   => .qfree e

/-- The capture-broken formula encoder: every binder reuses the name `0` (no
    fresh-name discipline), so a nested binder shadows its parent. -/
def sencodeCap : Frm → Tok
  | .atom a   => .atom (encAtomCap a)
  | .neg φ    => .neg (sencodeCap φ)
  | .conj φ ψ => .conj (sencodeCap φ) (sencodeCap ψ)
  | .disj φ ψ => .disj (sencodeCap φ) (sencodeCap ψ)
  | .imp φ ψ  => .imp (sencodeCap φ) (sencodeCap ψ)
  | .all s φ  => .all s 0 true (sencodeCap φ)   -- BUG: name 0, not the fresh level
  | .ex s φ   => .ex s 0 true (sencodeCap φ)

/-- The witness sentence `∀x:usize. ∃y:usize. x = c0` — depends only on the outer
    `x` (de Bruijn index 1 under the two binders); FALSE over `dom` (`c1 ≠ c0`). -/
def phiCap : Frm := .all usizeS (.ex usizeS (.atom (.rel .eq (.var usizeS 1) c0)))

/-! ## The pin -/

/-- The concrete counterexample: the capturing token denotes `true` (the outer `x`
    is captured by the inner `∃y`) while the source denotes `false`. -/
theorem capture_counterexample :
    tokDenote qEq dom (sencodeCap phiCap) σ0 = true
      ∧ fdenote qEq dom phiCap σ0 = false := by decide

/-- The pin: encoder soundness is false for the capturing encoder — there is a
    formula on a concrete domain where its token disagrees with the source. -/
theorem capture_breaks_soundness :
    ¬ ∀ (φ : Frm) (σ ρ : Subst),
        tokDenote qEq dom (sencodeCap φ) σ = fdenote qEq dom φ ρ := by
  intro h
  have hc := h phiCap σ0 σ0
  rw [capture_counterexample.1, capture_counterexample.2] at hc
  exact absurd hc (by decide)

/-- For contrast: the CORRECT `sencode` agrees with the source on the same witness,
    via the proven `strat_ref_sound` (`phiCap` is a well-scoped sentence). The
    divergence is solely the lost fresh-name discipline. -/
theorem correct_sound_on_phiCap (ρ σ : Subst) :
    tokDenote qEq dom (sencode phiCap) σ = fdenote qEq dom phiCap ρ :=
  strat_ref_sound qEq dom phiCap 0 ρ σ (fun i hi => absurd hi (Nat.not_lt_zero i)) (by decide)

end Thermite.PinStratCapture
