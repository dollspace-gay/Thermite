/-
  A negative pin for de Bruijn capture.

  Substituting outer variable 0 by variable 0 beneath a binder must shift the
  replacement to variable 1. The deliberately broken neighbor captures it.
-/
import Thermite.Strat.Substitution

namespace Thermite

open Thermite.Strat.Cls

private abbrev boolS : Sort₂ := .mach .bool

private def source : Frm :=
  .all boolS <|
    .atom (.rel .eq (.var boolS 1) (.var boolS 0))

private def replacement : TypedSubstitution
  | _, _ => .var boolS 0

private def expected : Frm :=
  .all boolS <|
    .atom (.rel .eq (.var boolS 1) (.var boolS 0))

private def captured : Frm :=
  .all boolS <|
    .atom (.rel .eq (.var boolS 0) (.var boolS 0))

private def observedLeftIndex : Frm → Nat
  | .all _ (.atom (.rel _ (.var _ index) _)) => index
  | _ => 0

theorem substitution_avoids_capture :
    applySubstFrm replacement source = expected := by
  rfl

theorem captured_neighbor_is_different :
    applySubstFrm replacement source ≠ captured := by
  intro equality
  have indexEquality := congrArg observedLeftIndex equality
  simp [source, captured, replacement, observedLeftIndex, applySubstFrm,
    applySubstAtom, applySubstTm, liftSubstitution, liftTm, bumpIdx] at indexEquality

#print axioms substitution_avoids_capture
#print axioms captured_neighbor_is_different

end Thermite
