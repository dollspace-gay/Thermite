/-
  Capture-free simultaneous substitution for the typed S₂.0 syntax.

  A substitution is indexed by the sort written on each variable occurrence.
  Lifting a substitution below a binder leaves the new variable alone and
  shifts every substituted outer term. This is the standard de Bruijn
  construction; `eval_applySubstFrm` proves that it commutes with the typed
  semantics.
-/
import Thermite.Strat.Normalize

namespace Thermite.Strat.Cls

/-- A simultaneous substitution. The sort argument is the annotation carried
    by the variable occurrence being replaced. -/
abbrev TypedSubstitution := Sort₂ → Nat → Tm

def applySubstTm (σ : TypedSubstitution) : Tm → Tm
  | .var sort index => σ sort index
  | .const sort id => .const sort id
  | .lit sort value => .lit sort value
  | .read elem sq index =>
      .read elem (applySubstTm σ sq) (applySubstTm σ index)
  | .len sq => .len (applySubstTm σ sq)
  | .cast target term => .cast target (applySubstTm σ term)
  | .idxOp term offset => .idxOp (applySubstTm σ term) offset
  | .mul left right => .mul (applySubstTm σ left) (applySubstTm σ right)
  | .app1 arg result fn term =>
      .app1 arg result fn (applySubstTm σ term)

def applySubstAtom (σ : TypedSubstitution) : Atom → Atom
  | .rel relation left right =>
      .rel relation (applySubstTm σ left) (applySubstTm σ right)
  | .qfree id expr => .qfree id expr

/-- Lift a substitution through one binder. A replacement for an outer
    variable is shifted once, so its free variables cannot be captured. -/
def liftSubstitution (σ : TypedSubstitution) : TypedSubstitution
  | sort, 0 => .var sort 0
  | sort, index + 1 => liftTm 0 (σ sort index)

def applySubstFrm (σ : TypedSubstitution) : Frm → Frm
  | .atom atom => .atom (applySubstAtom σ atom)
  | .neg formula => .neg (applySubstFrm σ formula)
  | .conj left right =>
      .conj (applySubstFrm σ left) (applySubstFrm σ right)
  | .disj left right =>
      .disj (applySubstFrm σ left) (applySubstFrm σ right)
  | .imp left right =>
      .imp (applySubstFrm σ left) (applySubstFrm σ right)
  | .all sort body => .all sort (applySubstFrm (liftSubstitution σ) body)
  | .ex sort body => .ex sort (applySubstFrm (liftSubstitution σ) body)

/-- The semantic relation required of a substitution: evaluating a replacement
    under `target` gives the same tagged value as reading the corresponding
    variable under `source`. This statement also makes malformed sort
    annotations explicit instead of silently assuming them away. -/
def EvalSubstitution (M : Model) (target : Valuation M)
    (σ : TypedSubstitution) (source : Valuation M) : Prop :=
  ∀ sort index,
    evalTm M target (σ sort index) =
      ⟨sort, M.valueAt source sort index⟩

theorem eval_applySubstTm (M : Model) (target source : Valuation M)
    (σ : TypedSubstitution) (related : EvalSubstitution M target σ source) :
    ∀ term,
      evalTm M target (applySubstTm σ term) = evalTm M source term := by
  intro term
  induction term with
  | var sort index => exact related sort index
  | const sort id => rfl
  | lit sort value => rfl
  | read elem sq index sqIH indexIH =>
      simp only [applySubstTm, evalTm]
      rw [sqIH, indexIH]
  | len sq ih =>
      simp only [applySubstTm, evalTm]
      rw [ih]
  | cast targetSort term ih =>
      simp only [applySubstTm, evalTm]
      rw [ih]
  | idxOp term offset ih =>
      simp only [applySubstTm, evalTm]
      rw [ih]
  | mul left right leftIH rightIH =>
      simp only [applySubstTm, evalTm]
      rw [leftIH, rightIH]
  | app1 arg result fn term ih =>
      simp only [applySubstTm, evalTm]
      rw [ih]

theorem eval_applySubstAtom (M : Model) (target source : Valuation M)
    (σ : TypedSubstitution) (related : EvalSubstitution M target σ source)
    (atom : Atom) :
    evalAtom M target (applySubstAtom σ atom) = evalAtom M source atom := by
  cases atom with
  | qfree expr => rfl
  | rel relation left right =>
      cases relation <;>
        simp only [applySubstAtom, evalAtom,
          eval_applySubstTm M target source σ related left,
          eval_applySubstTm M target source σ related right]

theorem valueAt_cons_zero (M : Model) (binderSort requested : Sort₂)
    (value : M.Carrier binderSort) (ρ : Valuation M) :
    M.valueAt (Valuation.cons M binderSort value ρ) requested 0 =
      M.valueAt (Valuation.cons M binderSort value ρ) requested 0 :=
  rfl

theorem valueAt_cons_succ (M : Model) (binderSort requested : Sort₂)
    (value : M.Carrier binderSort) (ρ : Valuation M) (index : Nat) :
    M.valueAt (Valuation.cons M binderSort value ρ) requested (index + 1) =
      M.valueAt ρ requested index := by
  simp only [Model.valueAt, Valuation.cons]

theorem liftSubstitution_related (M : Model) (target source : Valuation M)
    (σ : TypedSubstitution) (related : EvalSubstitution M target σ source)
    (binderSort : Sort₂) (value : M.Carrier binderSort) :
    EvalSubstitution M (Valuation.cons M binderSort value target)
      (liftSubstitution σ) (Valuation.cons M binderSort value source) := by
  intro requested index
  cases index with
  | zero => rfl
  | succ index =>
      simp only [liftSubstitution]
      rw [eval_liftTm]
      have skip :
          (fun i =>
            Valuation.cons M binderSort value target (bumpIdx 0 i)) =
          target := by
        funext i
        simp [bumpIdx, Valuation.cons]
      rw [skip, related requested index]
      simp only [Model.valueAt, Valuation.cons]

/-- Capture-free substitution preserves the meaning of every formula whenever
    its replacements denote the source environment. The binder cases use
    `liftSubstitution_related`, which is the capture-avoidance step. -/
theorem eval_applySubstFrm (M : Model) :
    ∀ (formula : Frm) (target source : Valuation M)
      (σ : TypedSubstitution),
      EvalSubstitution M target σ source →
      evalFrm M (applySubstFrm σ formula) target =
        evalFrm M formula source := by
  intro formula
  induction formula with
  | atom atom =>
      intro target source σ related
      exact eval_applySubstAtom M target source σ related atom
  | neg formula ih =>
      intro target source σ related
      simp only [applySubstFrm, evalFrm, ih target source σ related]
  | conj left right leftIH rightIH =>
      intro target source σ related
      simp only [applySubstFrm, evalFrm,
        leftIH target source σ related, rightIH target source σ related]
  | disj left right leftIH rightIH =>
      intro target source σ related
      simp only [applySubstFrm, evalFrm,
        leftIH target source σ related, rightIH target source σ related]
  | imp left right leftIH rightIH =>
      intro target source σ related
      simp only [applySubstFrm, evalFrm,
        leftIH target source σ related, rightIH target source σ related]
  | all sort body ih =>
      intro target source σ related
      simp only [applySubstFrm, evalFrm]
      apply congrArg (List.all (M.enum sort))
      funext value
      exact ih _ _ _ (liftSubstitution_related M target source σ related sort value)
  | ex sort body ih =>
      intro target source σ related
      simp only [applySubstFrm, evalFrm]
      apply congrArg (List.any (M.enum sort))
      funext value
      exact ih _ _ _ (liftSubstitution_related M target source σ related sort value)

#print axioms eval_applySubstFrm

end Thermite.Strat.Cls
