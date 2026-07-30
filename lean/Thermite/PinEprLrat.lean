/-
  Small, environment-independent fixture for the EPR LRAT boundary.

  Keeping the executable Boolean replay here avoids making Lean's symbolic
  evaluator traverse the much larger EPR import environment. The full EPR pin
  separately replays the same certificate into an unsatisfiability theorem.
-/
import Thermite.PropReconstruct

namespace Thermite.EprLratPin

open Std.Tactic.BVDecide

def certificate : Array LRAT.IntAction :=
  #[LRAT.Action.addRup 26 #[-6] #[3, 4],
    LRAT.Action.addRup 27 #[-10, 2] #[26, 8],
    LRAT.Action.addRup 28 #[-22, 25] #[26, 14],
    LRAT.Action.addRup 29 #[30, -22] #[3, 18],
    LRAT.Action.addRup 30 #[1] #[25, 22],
    LRAT.Action.addRup 31 #[37] #[25, 23],
    LRAT.Action.addRup 32 #[-2] #[30, 1],
    LRAT.Action.addRup 33 #[10] #[31, 19],
    LRAT.Action.addRup 34 #[30] #[31, 20],
    LRAT.Action.addEmpty 35 #[33, 32, 27]]

def problem : BoolExpr Nat :=
  .gate .and
    (.not (.literal 0))
    (.gate .and
      (.gate .or (.not (.const true)) (.literal 0))
      (.gate .and
        (.gate .or (.not (.const true)) (.literal 1))
        (.const true)))

#guard Thermite.PropReconstruct.verifyActions problem certificate
#guard !Thermite.PropReconstruct.verifyActions problem #[]

theorem proof_refutes_problem : BoolExpr.Unsat problem := by
  kernel_lrat_unsat "Thermite.EprLratPin.problem"
    with "Thermite.EprLratPin.certificate"

theorem pinned_proof_refutes_problem :
    Thermite.PropReconstruct.UnsatPin problem := by
  kernel_lrat_unsat "Thermite.EprLratPin.problem"
    with "Thermite.EprLratPin.certificate"

#print axioms proof_refutes_problem
#print axioms pinned_proof_refutes_problem

end Thermite.EprLratPin
