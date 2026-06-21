/-
  Thermite/PinRestratDropSide.lean — the restratify drop-Side pin (stage-2 pin
  battery, `.design/stage2-stratified-cage.md` REQ-7 / AC-7 / REQ-10).

  What it guards: `Strat/Restratify.lean`'s `restrat_conservative` (T4-R) certifies the
  original φ from the rewritten φ' = `A ∧ p` ONLY when the side obligation
  `Side(φ', φ) = p ⇒ B` is ALSO discharged (R-SIDE-1). This pin exhibits the
  mis-certification that DROPPING `Side` would permit: a model in which φ' holds (the
  fresh abstraction `p` is trivially `true`) while the ORIGINAL φ is FALSE (its excised
  conjunct `B` is false). So a φ'-only certificate, with `Side` dropped, would attest a
  FALSE φ — exactly the soundness hole R-SIDE-1 closes.

  The contrast: under the SAME model the real `Side` is FALSE (`p ⇒ B` = `true ⇒ false`
  = false), so the genuine `restrat_conservative` does NOT fire (its `Side` hypothesis is
  unmet) — confirming the gap is solely the dropped obligation, and the discipline is
  exactly what blocks the unsound inference.

  Style model: the in-tree `lean/Thermite/Pin*.lean` battery (`PinStratFlip`,
  `PinFiniteEscape`) — a small concrete model + `decide`-checked theorems; `decide`
  (kernel), never `native_decide`.
-/
import Thermite.Strat.Restratify

namespace Thermite.PinRestratDropSide

open Thermite.Strat.Cls

/-! ## A concrete model: a boolean-literal oracle reading `qfree` leaves -/

/-- The oracle interprets each `qfree` leaf by the truth of the `boolLit` it carries
    (every other atom shape is `false` — unused here). This lets the model set the fresh
    abstraction `p` to `true` independently of the excised conjunct `B`. -/
def qBool : Atom → Bool
  | .qfree (.boolLit b) => b
  | _                   => false

/-- The (irrelevant) finite domain and environment — the formulas below are closed
    propositional combinations of `qfree` leaves, so neither is inspected. -/
def dom : List Tm := [.lit usizeS]
def ρ0 : Subst := fun _ => .lit usizeS

/-! ## The witness formulas

    A simple `conj A B` with a TRUE left conjunct and a FALSE right conjunct, both modelled
    as opaque `qfree` leaves — the minimal shape exhibiting the drop-Side hole. -/

/-- `A` — a true sub-formula (`boolLit true`, read as `true` by `qBool`). -/
def aT : Frm := .atom (.qfree (.boolLit true))
/-- `B` — the cycle-closing conjunct, FALSE in this model (`boolLit false`). -/
def bF : Frm := .atom (.qfree (.boolLit false))
/-- The fresh restratify abstraction token `p` — a DISTINCT leaf, `true` in this model. -/
def pAbs : Thermite.Expr := .boolLit true

/-- The original (would-be-restratified) formula `φ = A ∧ B`: TRUE ∧ FALSE = FALSE. -/
def phi : Frm := .conj aT bF

/-! ## The pin -/

/-- The concrete counterexample: the rewritten φ' = `A ∧ p` is `true` (both leaves
    `true`), while the original φ = `A ∧ B` is `false` (`B` false). A φ'-only certificate
    would therefore mis-certify a FALSE φ. -/
theorem dropSide_counterexample :
    fdenote qBool dom (restrat pAbs phi) ρ0 = true
      ∧ fdenote qBool dom phi ρ0 = false := by decide

/-- The pin: with `Side` DROPPED, "φ' certified ⇒ φ certified" is UNSOUND — there is a
    model where φ' holds but φ does not. -/
theorem dropSide_breaks_certification :
    ¬ ∀ (A B : Frm),
        fdenote qBool dom (restrat pAbs (.conj A B)) ρ0 = true →
        fdenote qBool dom (.conj A B) ρ0 = true := by
  intro h
  have hc := h aT bF dropSide_counterexample.1
  -- hc : fdenote … (aT.conj bF) … = true ; counterexample.2 : the same is false (`phi` defeq)
  have hf : fdenote qBool dom (aT.conj bF) ρ0 = false := dropSide_counterexample.2
  exact absurd (hc.symm.trans hf) (by decide)

/-- The contrast: under the SAME model the genuine `Side(φ', φ) = p ⇒ B` is FALSE
    (`true ⇒ false`), so `restrat_conservative`'s `Side` hypothesis is UNMET — the real
    discipline does not fire, and the unsound step is blocked exactly by R-SIDE-1. -/
theorem side_is_false_here :
    fdenote qBool dom (Side pAbs phi) ρ0 = false := by decide

/-- And WITH the discharged `Side`, `restrat_conservative` would correctly require it —
    here `Side` is false, so the (sound) certificate of φ is correctly WITHHELD. The pin
    therefore shows the gap is solely the dropped obligation. -/
theorem conservative_withholds_without_side :
    fdenote qBool dom (restrat pAbs phi) ρ0 = true
      ∧ fdenote qBool dom (Side pAbs phi) ρ0 = false
      ∧ fdenote qBool dom phi ρ0 = false := by decide

end Thermite.PinRestratDropSide
