/-
  Permanent probe for proof-producing BitVec reconstruction.
  Its axiom report is checked by scripts/lean-axiom-probe.sh.
-/
import Thermite.Reconstruct

namespace Thermite

set_option maxRecDepth 1000000
set_option maxHeartbeats 2000000 in
theorem bv_reconstruction_lrat_probe (a : BitVec 64) :
    (a &&& 0#64) = 0#64 := by
  bv_reconstruct (timeout := 30)

#print axioms bv_reconstruction_lrat_probe

end Thermite
