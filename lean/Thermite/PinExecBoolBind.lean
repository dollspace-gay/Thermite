/-
  Critic pin — the exec-body bool-result bind (#253, proof-backends.md §4.1.2/§4.1.6).
  The second of the four bridge-divergence pins. A kernel-checked divergence oracle: a
  mis-bind that drops the bool bind (the consequent reads the defaulted `Env.bools`
  `false` regardless of the body's `.bool true` result) certifies a negated contract,
  while the faithful `bindBool` bridge refutes it. Extends the shipped Pin H
  (`PinExportBoolResult.lean`'s `true_false_indistinguishable_in_intVal`) from "why the
  Int-0/1 route is refused" to "why the bind must be genuine" (the §4.1.2 `bindBool`).

  The witness. A straight-line body converging to `.bool true` (`bodyConverges` over a
  tail `boolLit true`). The contract `ens` is `!result`, the negated contract; it claims
  the result is false.

    - Faithful (`bindResult`, the §4.1.2 `bindBool`): binds `result := true`, so
      `ens : !result` denotes `¬True` = false; the negated contract is refuted.
    - Dropped bind (the bug `bindResult` must not commit): leaves `result` at the
      defaulted `Env.bools` `false`, so `ens : !result` denotes `¬False` = true; the
      negated contract certifies against a body that produces `true`.

  The Int-0/1 alternative is also walled off: `PinExportBoolResult.lean` proves `intVal`
  bottoms both `true` and `false` to `0`, so an Int-encoding cannot distinguish them,
  which is why §4.1.2 lands a genuine bool sort (`Env.bools` + `bindBool`).
-/
import Thermite.Stabilize
import Thermite.Exec.Stmt

namespace Thermite.PinExecBoolBind

open Thermite Thermite.Exec

/-- A contract env with nothing bound — every bool name is the defaulted `false`. The
    `ens` reads only `result`, bound (or not) by the bridge. -/
def baseEnv : Env :=
  { ints := fun _ => 0
    seqs := fun _ => []
    optres := fun _ => OptResVal.none_
    specs := fun _ => none }

/-- The exec body `{ true }` (tail is the bool literal `true`). -/
def trueBody : Block := .mk [] (some (.boolLit true))

/-- The empty starting state (every var defaults; the tail is a literal). -/
def st0 : State :=
  { env := { vars := fun _ => .int ⟨.u64, 0⟩, slices := fun _ => [] }
    scope := fun _ => false }

/-- The body converges to `.bool true` (`bodyDenote = some (.bool true)`, a value rather
    than a bottom). -/
theorem trueBody_converges :
    bodyConverges trueBody st0 (.bool true) := by
  simp [bodyConverges, trueBody, bodyDenote, blockThread, Block.blkTail, st0, execDenote]

/-- The negated contract `ens : !result` (claims the bool result is false). -/
def ensNotResult : Expr := Expr.neg (Expr.boolVar "result")

/-- The dropped-bind mis-bridge (the bug): ignore the body's `.bool` result, leaving
    `result` at the defaulted `Env.bools` `false`. -/
def bindResultDropped (env : Env) (_b : Bool) : Env := env

/-- The dropped bind certifies the negated contract. With the bind dropped,
    `result` reads the defaulted `false`, so `ens : !result` = `¬False` denotes true; the
    negated contract certifies against a body producing `true`. -/
theorem dropped_bind_certifies_negated_contract :
    denote 0 ensNotResult (bindResultDropped baseEnv true) := by
  simp only [ensNotResult, bindResultDropped, denote, baseEnv]
  decide

/-- The faithful `bindBool` bridge refutes the negated contract. `bindResult` binds
    `result := true`, so `ens : !result` = `¬True` denotes false; the negated contract
    does not certify. (The §4.1.2 genuine bool bind, not the dropped/Int-0/1 routes.) -/
theorem faithful_bind_refutes_negated_contract :
    ¬ denote 0 ensNotResult (bindResult baseEnv (.bool true)) := by
  simp only [ensNotResult, bindResult, denote, Env.bindBool, baseEnv]
  decide

/-- The divergence in one statement: at the same witness (a body producing `.bool true`)
    the dropped bind and the faithful `bindBool` disagree on the negated contract `!result`;
    the dropped bind certifies, the faithful refutes. A bool bind that drops (or routes
    through the rejected Int-0/1 encoding, `PinExportBoolResult.lean`) is therefore unsound;
    the genuine `Env.bindBool` is what keeps the bool-result obligation faithful. -/
theorem dropped_bind_diverges_from_faithful :
    denote 0 ensNotResult (bindResultDropped baseEnv true)
      ≠ denote 0 ensNotResult (bindResult baseEnv (.bool true)) := by
  rw [propext (iff_true_intro dropped_bind_certifies_negated_contract)]
  rw [eq_false faithful_bind_refutes_negated_contract]
  simp

end Thermite.PinExecBoolBind
