/-
  Negative pins for typed normalization and Skolemization.

  The first formula needs its existential witness to depend on the preceding
  universal value. Replacing that witness by one constant loses the `true`
  branch. The second formula catches the classic `¬∀` polarity error.
-/
import Thermite.Strat.Skolem
import Thermite.Strat.TestModel

namespace Thermite

open Thermite.Strat.Cls

private abbrev boolS : Sort₂ := .mach .bool

private def dependsFormula : Frm :=
  .all boolS <| .ex boolS <|
    .atom (.rel .eq (.var boolS 0) (.var boolS 1))

private def missingDependency : Frm :=
  .all boolS <|
    .atom (.rel .eq
      (.lit boolS (.bool false))
      (.var boolS 0))

theorem skolem_dependency_is_required :
    evalFrm boolModel dependsFormula emptyBoolValuation = true
      ∧ evalFrm boolModel missingDependency emptyBoolValuation = false := by
  decide

private def polarityFormula : Frm :=
  .neg <| .all boolS <|
    .atom (.rel .eq
      (.var boolS 0)
      (.lit boolS (.bool true)))

/-- The broken neighbor keeps `all` under negation instead of flipping it to
    `ex`. It disagrees on the two-element carrier. -/
private def brokenPolarity : Frm :=
  .all boolS <| .neg <|
    .atom (.rel .eq
      (.var boolS 0)
      (.lit boolS (.bool true)))

theorem nnf_quantifier_polarity_is_required :
    evalFrm boolModel (nnf polarityFormula) emptyBoolValuation = true
      ∧ evalFrm boolModel brokenPolarity emptyBoolValuation = false := by
  decide

theorem no_empty_carrier_escape (sort : Sort₂) :
    boolModel.enum sort ≠ [] :=
  boolModel.enum_ne_nil sort

#print axioms skolem_dependency_is_required
#print axioms nnf_quantifier_polarity_is_required

end Thermite
