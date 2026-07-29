/-
  Thermite/PinRelaxRefute.lean — the relax-route converse pin (stage-2 pin battery,
  `.design/stage2-stratified-cage.md` REQ-10; relax spine `Thermite/Relax.lean`,
  `.design/stage1-forge-tier.md` REQ-8 / Q-NLSAT).

  What it guards: `Strat/`'s relax-route soundness `Thermite.Relax.r_relax_sound` is the
  ONE-WAY implication "if the real relaxation `∀x:ℝ, 0 ≤ e(x)` holds, then the integer
  clause `∀a:ℤ, 0 ≤ e(a)` holds". The route's discharge uses exactly that direction. The
  CONVERSE is FALSE, and that is not a cosmetic detail — it is why the relax route must
  perform the integrality check and (on a non-integral real countermodel) ESCALATE to the
  forge via `RealWitness` rather than emit a `Counterexample`. This pin exhibits the
  broken neighbour that would use the converse — "the real relaxation FAILED, so report
  the clause false / emit a Counterexample" — and shows it is UNSOUND: there is a clause
  valid over ℤ whose real relaxation fails.

  The witness is `e(x) = x·x + (−1)·x = x² − x = x(x−1)`. Over ℤ it is non-negative for
  every integer (a product of consecutive integers), so the integer clause HOLDS. Over ℝ
  it dips below zero at `x = 1/2` (value `−1/4`), so the real relaxation FAILS. A route
  reading the failed relaxation as an integer counterexample would wrongly reject a valid
  clause — precisely the `RealWitness` escalation `r_relax_sound`'s one-wayness mandates.

  This is a Mathlib-importing pin (it reasons over ℝ/ℤ), an island exactly like
  `Thermite/Relax.lean`, whose `r_relax_sound` it guards — Mathlib is already in the build
  graph there, so this adds no dependency to the core Mathlib-free denotation path. It is
  axiom-probed like the other pins; its footprint stays ⊆ {propext, Classical.choice,
  Quot.sound} (no `sorry`, no `native_decide`).
-/
import Thermite.Relax
import Mathlib.Tactic.Ring
import Mathlib.Tactic.NormNum

namespace Thermite.PinRelaxRefute

open Thermite.Relax

/-- The single-variable polynomial atom `e(x) = x·x + (−1)·x = x² − x` (the relax
    fragment's syntax over one variable, `ν := Unit`). -/
def ePoly : PExpr Unit :=
  .add (.mul (.var ()) (.var ())) (.mul (.lit (-1)) (.var ()))

/-- The integer clause HOLDS: `x² − x = x(x−1) ≥ 0` for every integer `x` (a product of
    two consecutive integers). The `decide`-free integer argument splits on `x ≤ 0`
    versus `x ≥ 1`; both factors share a sign. -/
theorem int_clause_holds (a : Unit → ℤ) : 0 ≤ ePoly.eval a := by
  simp only [ePoly, PExpr.eval, Int.cast_id]
  have hfac : a () * a () + (-1) * a () = a () * (a () - 1) := by ring
  rw [hfac]
  by_cases h : a () ≤ 0
  · -- both factors ≤ 0: rewrite as a product of their negations, then `mul_nonneg`.
    have hneg : a () * (a () - 1) = (-a ()) * (-(a () - 1)) := by ring
    rw [hneg]
    exact mul_nonneg (by omega) (by omega)
  · exact mul_nonneg (by omega) (by omega)

/-- The real relaxation fails: at `x = 1/2`, `e = (1/2)² − 1/2 = −1/4 < 0`. -/
theorem real_relax_fails : ¬ ∀ x : Unit → ℝ, 0 ≤ ePoly.eval x := by
  intro h
  have hx := h (fun _ => (1 : ℝ) / 2)
  simp only [ePoly, PExpr.eval] at hx
  norm_num at hx

/-- The pin: the CONVERSE of `r_relax_sound` is false. A failed real relaxation does NOT
    imply a failed integer clause — `ePoly` is valid over ℤ yet its relaxation fails over
    ℝ. So a relax route that read "real relaxation failed" as "clause false / emit a
    Counterexample" would be unsound; the integrality check + `RealWitness` escalation
    `r_relax_sound`'s one-wayness mandates is exactly what blocks it. -/
theorem relax_converse_unsound :
    ¬ ∀ (e : PExpr Unit),
        (¬ ∀ x : Unit → ℝ, 0 ≤ e.eval x) → (¬ ∀ a : Unit → ℤ, 0 ≤ e.eval a) := by
  intro h
  exact h ePoly real_relax_fails int_clause_holds

/-- For contrast: the sound direction `r_relax_sound` holds vacuously here — `ePoly`'s
    relaxation does not hold, so its hypothesis is unmet, confirming the gap is solely the
    illegitimate converse. (Stated as the instantiated implication.) -/
theorem r_relax_sound_on_ePoly (a : Unit → ℤ) :
    (∀ x : Unit → ℝ, 0 ≤ ePoly.eval x) → 0 ≤ ePoly.eval a :=
  fun hrel => r_relax_sound ePoly hrel a

end Thermite.PinRelaxRefute
