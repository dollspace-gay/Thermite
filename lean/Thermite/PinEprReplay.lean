/-
  End-to-end pin for structural grounding, direct theory CNF, and LRAT replay.

  The source is the false formula `¬(c = c)`. Lean recomputes its finite
  instance and reflexivity clause; the LRAT proof refutes that exact CNF.
  Neighboring source, theory, and certificate values are rejected.
-/
import Thermite.Strat.EprReplay

namespace Thermite

open Thermite.Strat.Cls
open Std.Tactic.BVDecide

set_option maxHeartbeats 8000000
set_option maxRecDepth 100000

private def sort : Sort₂ := .opaque 702
private def sourceTerm : Tm := .const sort 0
private def source : Frm :=
  .neg (.atom (.rel .eq sourceTerm sourceTerm))
private def skeleton : EprReplayCertificate :=
  buildEprSkeleton source

def eprPinLrat : Array LRAT.IntAction :=
  #[LRAT.Action.addEmpty 7 #[3, 4, 1]]

private def certificate : EprReplayCertificate :=
  { instantiation := skeleton.instantiation
    theory := skeleton.theory
    lrat := eprPinLrat }

def eprPinCnf : Std.Sat.CNF Nat := eprCnf certificate

private theorem instantiation_is_bound :
    verifyStructuralBinding source certificate.instantiation = true := by
  kernel_bool_check

private theorem theory_is_checked :
    verifyTheory (eprGround certificate)
      certificate.theory = true := by
  simpa only [certificate, skeleton, eprGround] using
    verifyTheory_buildEprSkeleton source

private theorem cnf_is_unsat : eprPinCnf.Unsat := by
  kernel_lrat_cnf_unsat "Thermite.eprPinCnf"
    with "Thermite.eprPinLrat"

theorem checked_source_is_false (M : Model) (ρ : Valuation M) :
    evalFrm M source ρ = false := by
  apply source_false_of_verifiedStructuralBinding
    instantiation_is_bound
  intro candidate interpretation
  exact ground_false_of_epr_cnf_unsat theory_is_checked
    cnf_is_unsat candidate interpretation

private def changedSource : Frm :=
  .neg (.atom (.rel .eq (.const sort 1) (.const sort 1)))

private def invalidTheory : EprReplayCertificate :=
  { certificate with
    theory :=
      [.functionCongruence
        { kind := .source 4, arguments := [sort], result := sort }
        [] []] }

private def invalidLrat : EprReplayCertificate :=
  { certificate with lrat := #[] }

#guard verifyStructuralEprReplay source certificate
#guard !verifyStructuralEprReplay changedSource certificate
#guard !verifyStructuralEprReplay source invalidTheory
#guard !verifyStructuralEprReplay source invalidLrat

#print axioms checked_source_is_false

end Thermite
