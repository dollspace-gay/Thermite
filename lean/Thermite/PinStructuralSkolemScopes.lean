/-
  Independent quantified branches must not acquire dependencies on one another.

  Pulling both branches into one global prenex prefix makes the existential
  introduced by negating the right-hand universal appear to depend on the
  left-hand universal. With one opaque sort this creates a false self-cycle in
  the Skolem sort graph. Structural instantiation keeps the two lexical scopes
  separate and accepts the formula.
-/
import Thermite.Strat.StructuralInstantiation

namespace Thermite

open Thermite.Strat.Cls

private def sort : Sort₂ := .opaque 811

private def reflexive : Frm :=
  .all sort (.atom (.rel .eq (.var sort 0) (.var sort 0)))

private def independentBranches : Frm :=
  .conj reflexive (.neg reflexive)

private def structural : InstantiationCertificate :=
  buildStructuralInstantiation independentBranches

#guard admitted independentBranches
#guard verifyStructuralInstantiation independentBranches structural

-- This is the regression: global prenexing invents a same-sort dependency and
-- the grounding builder correctly refuses that artificial cycle.
#guard !verifyInstantiation independentBranches
  (buildInstantiation independentBranches)

end Thermite
