/-
  Ground-universe pins: both source constants and the complete Cartesian
  function instance set must be present. The broken neighbor omits one Boolean
  argument and fails `checkUniverse`.
-/
import Thermite.Strat.Grounding

namespace Thermite

open Thermite.Strat.Cls

private abbrev boolS : Sort₂ := .mach .bool
private abbrev outS : Sort₂ := .opaque 0

private def falseTerm : GroundTerm :=
  .constant boolS (.literal (.bool false))

private def trueTerm : GroundTerm :=
  .constant boolS (.literal (.bool true))

private def negFn : GroundFunction :=
  { kind := .source 0, arguments := [boolS], result := outS }

private def completeTerms : GroundUniverse :=
  [falseTerm, trueTerm, .appList negFn [falseTerm], .appList negFn [trueTerm]]

private def omittedTerms : GroundUniverse :=
  [falseTerm, trueTerm, .appList negFn [falseTerm]]

theorem complete_ground_universe_is_accepted :
    checkUniverse [negFn] [boolS, outS] [falseTerm, trueTerm] completeTerms = true := by
  decide

theorem omitted_quantifier_instance_is_rejected :
    checkUniverse [negFn] [boolS, outS] [falseTerm, trueTerm] omittedTerms = false := by
  decide

#print axioms complete_ground_universe_is_accepted
#print axioms omitted_quantifier_instance_is_rejected

end Thermite
