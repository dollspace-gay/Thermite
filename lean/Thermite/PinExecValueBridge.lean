/-
  CRITIC PIN — the EXEC-BODY VALUE BRIDGE (#253, proof-backends.md §4.1.1/§4.1.6).
  The first of the FOUR bridge-divergence pins the §4.1 build authors. It is a
  kernel-checked DIVERGENCE ORACLE: a mis-bridge binding the SIGNED reinterpretation
  (or a NARROWER-width re-wrap) in place of the faithful `BVal.value` lets a WRONG
  contract certify at a witness, while the faithful identity-on-value bridge REFUTES
  the same contract. Both directions are pinned (the poisoned discharge AND the
  faithful refutation), and the file must keep COMPILING as the regression oracle.

  THE WITNESS. A straight-line body converging to `.int ⟨.u64, 2^64 − 1⟩` (the `u64`
  rim — `bodyConverges` over a tail reading a param `m := 2^64 − 1`). The contract
  `ens` is `result < 0`. The exec value is the UNSIGNED `2^64 − 1` (`BVal.value` is the
  mathematical unsigned value, `Exec.lean`); `S_C` compares mathematical values.

    - FAITHFUL (`bindResult`, the identity on `BVal.value`): binds `result := 2^64 − 1`,
      so `ens : result < 0` is FALSE — the wrong contract is REFUTED.
    - SIGNED MIS-READ (the bug `bindResult` must NOT commit): reinterprets the top bit,
      binding `result := −1`, so `ens : result < 0` discharges TRUE — a wrong contract
      CERTIFIES.

  `2^64 − 1 ↦ −1` is the exact signed reinterpretation the §4.1.1 discipline forbids
  (the exec domain is the UNSIGNED `[0, ty.bound)`). This is the analogue of
  `nat_coercion_underflow_breaks_soundness` (`Exec.lean`) on the BIND side.
-/
import Thermite.Stabilize
import Thermite.Exec.Stmt

namespace Thermite.PinExecValueBridge

open Thermite Thermite.Exec

/-- The `u64` rim value `2^64 − 1` (the max `u64`) — the body's converged result. -/
def rimVal : Int := (2 : Int) ^ 64 - 1

/-- A contract env with no bound names (every int `0`, every bool `false`). The `ens`
    reads ONLY `result`, bound by the bridge. -/
def baseEnv : Env :=
  { ints := fun _ => 0
    seqs := fun _ => []
    optres := fun _ => OptResVal.none_
    specs := fun _ => none }

/-- The exec body `{ m }` (tail reads the input cell `m`); its state seeds `m := 2^64 − 1`. -/
def rimBody : Block := .mk [] (some (.var "m"))

/-- The exec state seeding `m := 2^64 − 1` (`u64`), nothing `let`-bound. -/
def rimState : State :=
  { env := { vars := fun s => if s = "m" then .int ⟨.u64, rimVal⟩ else .int ⟨.u64, 0⟩
             slices := fun _ => [] }
    scope := fun _ => false }

/-- The body GENUINELY converges to the `u64`-rim value `2^64 − 1` (the witness is real —
    `bodyDenote = some (.int ⟨.u64, 2^64 − 1⟩)`, not a bottom). -/
theorem rimBody_converges :
    bodyConverges rimBody rimState (.int ⟨.u64, rimVal⟩) := by
  simp [bodyConverges, rimBody, bodyDenote, blockThread, Block.blkTail, rimState,
    execDenote, rimVal]

/-- The wrong contract `ens : result < 0`. -/
def ensResultNeg : Expr := Expr.cmp CmpOp.lt (Expr.var "result") (Expr.intLit 0)

/-- THE SIGNED MIS-BRIDGE (the bug): reinterpret the unsigned `value` as signed
    (`v ≥ 2^63 → v − 2^64`). At the rim `2^64 − 1` this binds `result := −1`. -/
def bindResultSigned (env : Env) (b : BVal) : Env :=
  Env.bindInt env "result"
    (if b.value ≥ (2 : Int) ^ 63 then b.value - (2 : Int) ^ 64 else b.value)

/-- **Teeth (the signed mis-bridge CERTIFIES a wrong contract).** The signed mis-read
    binds `result := −1`, so `ens : result < 0` denotes TRUE — a contract no faithful
    exec value satisfies (the body's value is the UNSIGNED `2^64 − 1 ≥ 0`). -/
theorem signed_misbridge_certifies_wrong_contract :
    denote 0 ensResultNeg (bindResultSigned baseEnv ⟨.u64, rimVal⟩) := by
  simp only [ensResultNeg, bindResultSigned, denote, intVal, Env.bindInt, baseEnv, rimVal]
  decide

/-- **The faithful bridge REFUTES the same contract.** `bindResult` binds the identity
    `result := 2^64 − 1`, so `ens : result < 0` denotes FALSE — the wrong contract does
    NOT certify. (The bridge is the identity on `BVal.value`, §4.1.1.) -/
theorem faithful_bridge_refutes_wrong_contract :
    ¬ denote 0 ensResultNeg (bindResult baseEnv (.int ⟨.u64, rimVal⟩)) := by
  simp only [ensResultNeg, bindResult, denote, intVal, Env.bindInt, baseEnv, rimVal]
  decide

/-- The divergence in one statement: at the SAME witness the signed mis-bridge and the
    faithful bridge DISAGREE on the wrong contract `result < 0` (the mis-bridge certifies,
    the faithful refutes). A value bridge that signed-reinterprets is therefore UNSOUND;
    the identity-on-`BVal.value` bridge is what keeps the exec-body obligation faithful. -/
theorem signed_misbridge_diverges_from_faithful :
    denote 0 ensResultNeg (bindResultSigned baseEnv ⟨.u64, rimVal⟩)
      ≠ denote 0 ensResultNeg (bindResult baseEnv (.int ⟨.u64, rimVal⟩)) := by
  rw [propext (iff_true_intro signed_misbridge_certifies_wrong_contract)]
  rw [eq_false faithful_bridge_refutes_wrong_contract]
  simp

end Thermite.PinExecValueBridge
