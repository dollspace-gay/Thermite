/-
  Negative pin for quantifier-instance omission.

  The source is `∀x:S. x = x`. Its checked universe contains the binder-sort
  inhabitant and instantiation produces that one required atom. Replacing the
  ground formula by `true` must fail even though `true` is propositionally
  equivalent for this particular example: replay binds the exact instance set,
  not merely a convenient equisatisfiable neighbor.
-/
import Thermite.Strat.Instantiation

namespace Thermite

open Thermite.Strat.Cls

private def sort : Sort₂ := .opaque 700

private def source : Frm :=
  .all sort (.atom (.rel .eq (.var sort 0) (.var sort 0)))

private def inhabitant : GroundTerm :=
  .constant sort .inhabitant

private def ground : GroundUniverse := [inhabitant]

private def grounding : GroundingCertificate :=
  { order := [sort], ground }

private def expected : GroundFrm :=
  .atom (.rel .eq inhabitant inhabitant)

private def complete : InstantiationCertificate :=
  { grounding, formula := expected }

private def omitted : InstantiationCertificate :=
  { grounding, formula := .const true }

theorem complete_instantiation_is_accepted :
    verifyInstantiation source complete = true := by decide

theorem omitted_instantiation_is_rejected :
    verifyInstantiation source omitted = false := by decide

#print axioms complete_instantiation_is_accepted
#print axioms omitted_instantiation_is_rejected

end Thermite
