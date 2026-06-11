/-
  CRITIC PIN — the EXEC-BODY BOOL-RESULT BIND (#253, proof-backends.md §4.1.2/§4.1.6).
  The second of the FOUR bridge-divergence pins. A kernel-checked DIVERGENCE ORACLE: a
  mis-bind that DROPS the bool bind (the consequent reads the DEFAULTED `Env.bools`
  `false` regardless of the body's `.bool true` result) certifies a NEGATED contract,
  while the faithful `bindBool` bridge REFUTES it. Extends the SHIPPED Pin H
  (`PinExportBoolResult.lean`'s `true_false_indistinguishable_in_intVal`) from "why the
  Int-0/1 route is REFUSED" to "why the bind must be GENUINE" (the §4.1.2 `bindBool`).

  THE WITNESS. A straight-line body converging to `.bool true` (`bodyConverges` over a
  tail `boolLit true`). The contract `ens` is `!result` (the negated contract — it claims
  the result is FALSE).

    - FAITHFUL (`bindResult`, the §4.1.2 `bindBool`): binds `result := true`, so
      `ens : !result` denotes `¬True` = FALSE — the negated contract is REFUTED.
    - DROPPED BIND (the bug `bindResult` must NOT commit): leaves `result` at the
      DEFAULTED `Env.bools` `false`, so `ens : !result` denotes `¬False` = TRUE — the
      negated contract CERTIFIES against a body that genuinely produces `true`.

  The Int-0/1 alternative is ALSO walled off: `PinExportBoolResult.lean` proves `intVal`
  bottoms BOTH `true` and `false` to `0`, so an Int-encoding cannot distinguish them —
  which is exactly why §4.1.2 lands a GENUINE bool sort (`Env.bools` + `bindBool`).
-/
import Thermite.Stabilize
import Thermite.Exec.Stmt

namespace Thermite.PinExecBoolBind

open Thermite Thermite.Exec

/-- A contract env with nothing bound — every bool name is the DEFAULTED `false`. The
    `ens` reads ONLY `result`, bound (or not) by the bridge. -/
def baseEnv : Env :=
  { ints := fun _ => 0
    seqs := fun _ => []
    optres := fun _ => OptResVal.none_
    specs := fun _ => none }

/-- The exec body `{ true }` (tail is the bool literal `true`). -/
def trueBody : Block := .mk [] (some (.boolLit true))

/-- The empty starting state (every var defaults; nothing relevant — the tail is a literal). -/
def st0 : State :=
  { env := { vars := fun _ => .int ⟨.u64, 0⟩, slices := fun _ => [] }
    scope := fun _ => false }

/-- The body GENUINELY converges to `.bool true` (`bodyDenote = some (.bool true)` — a real
    value, not a bottom). -/
theorem trueBody_converges :
    bodyConverges trueBody st0 (.bool true) := by
  simp [bodyConverges, trueBody, bodyDenote, blockThread, Block.blkTail, st0, execDenote]

/-- The negated contract `ens : !result` (claims the bool result is FALSE). -/
def ensNotResult : Expr := Expr.neg (Expr.boolVar "result")

/-- THE DROPPED-BIND MIS-BRIDGE (the bug): ignore the body's `.bool` result, leaving
    `result` at the DEFAULTED `Env.bools` `false`. -/
def bindResultDropped (env : Env) (_b : Bool) : Env := env

/-- **Teeth (the dropped bind CERTIFIES the negated contract).** With the bind dropped,
    `result` reads the defaulted `false`, so `ens : !result` = `¬False` denotes TRUE — the
    negated contract certifies against a body producing `true`. -/
theorem dropped_bind_certifies_negated_contract :
    denote 0 ensNotResult (bindResultDropped baseEnv true) := by
  simp only [ensNotResult, bindResultDropped, denote, baseEnv]
  decide

/-- **The faithful `bindBool` bridge REFUTES the negated contract.** `bindResult` binds
    `result := true`, so `ens : !result` = `¬True` denotes FALSE — the negated contract
    does NOT certify. (The §4.1.2 GENUINE bool bind, not the dropped/Int-0/1 routes.) -/
theorem faithful_bind_refutes_negated_contract :
    ¬ denote 0 ensNotResult (bindResult baseEnv (.bool true)) := by
  simp only [ensNotResult, bindResult, denote, Env.bindBool, baseEnv]
  decide

/-- The divergence in one statement: at the SAME witness (a body producing `.bool true`)
    the dropped bind and the faithful `bindBool` DISAGREE on the negated contract `!result`
    — the dropped bind certifies, the faithful refutes. A bool bind that DROPS (or routes
    through the rejected Int-0/1 encoding, `PinExportBoolResult.lean`) is therefore UNSOUND;
    the genuine `Env.bindBool` is what keeps the bool-result obligation faithful. -/
theorem dropped_bind_diverges_from_faithful :
    denote 0 ensNotResult (bindResultDropped baseEnv true)
      ≠ denote 0 ensNotResult (bindResult baseEnv (.bool true)) := by
  rw [propext (iff_true_intro dropped_bind_certifies_negated_contract)]
  rw [eq_false faithful_bind_refutes_negated_contract]
  simp

end Thermite.PinExecBoolBind
