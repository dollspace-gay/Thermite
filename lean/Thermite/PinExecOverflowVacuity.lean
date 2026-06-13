/-
  Critic pin — the exec-body overflow-vacuity escape (#253, proof-backends.md
  §4.1.5/§4.1.6). The third of the four bridge-divergence pins, and the
  certificate-level conjunction regression oracle (the Pin B shape). A body that always
  overflows under the precondition (`bodyDenote = none`) makes the hypothesize-contract
  obligation vacuously kernel-accept with a false `ens`, and the pin proves the
  conjoined overflow obligation refuted at the same state. The vacuous contract discharge
  must stay unreachable as a certificate, blocked by the failing overflow class (the §4.1
  conjunction rule: an item certifies only when both the contract and the overflow class
  discharge).

  The witness. The body `{ let a = m + m; a }` with `m := 2^64 − 1` (the
  `body_overflow_rhs_has_no_result` shape, `Exec/Stmt.lean`): the `let` RHS `m + m`
  overflows `u64`, so `bodyDenote = none`.

    - The hypothesize-contract obligation `∀ r, bodyConverges body st (.int r) →
      ensStable(bindResult …)` is vacuously provable even for a false `ens` (here
      `ens := result < result`, never true): the antecedent `bodyConverges = (none = some …)`
      is false for every `r`, so the implication holds. This is the vacuity the hypothesize
      form would let through if the contract class stood alone.
    - The conjoined overflow obligation `(bodyDenote body st).isSome` is false, so the
      item does not certify (the conjunction rule rejects it). The vacuous contract can
      never reach a certificate.

  Both halves are pinned: the vacuous contract discharge and the conjoined overflow
  refutation. This is why the hypothesize form is sound (§4.1's #212(b) resolution).
-/
import Thermite.Stabilize
import Thermite.Exec.Stmt

namespace Thermite.PinExecOverflowVacuity

open Thermite Thermite.Exec

/-- A contract env with nothing bound (the `ens` reads only `result`, bound by the bridge). -/
def baseEnv : Env :=
  { ints := fun _ => 0
    seqs := fun _ => []
    optres := fun _ => OptResVal.none_
    specs := fun _ => none }

/-- The always-overflowing body `{ let a = m + m; a }` (the `body_overflow_rhs_has_no_result`
    shape): the `let` RHS `m + m` overflows `u64` when `m` is at the rim. -/
def ovfBody : Block :=
  .mk [ .letS "a" (.arith .add (.var "m") (.var "m")) ] (some (.var "a"))

/-- The state seeding `m := 2^64 − 1` (max `u64`) — the precondition forces `m` at the rim. -/
def maxState : State :=
  { env := { vars := fun s => if s = "m" then .int ⟨.u64, (2 : Int) ^ 64 - 1⟩
                              else .int ⟨.u64, 0⟩
             slices := fun _ => [] }
    scope := fun _ => false }

/-- The body has no result at the overflow (`bodyDenote = none`): the no-overflow
    obligation fails (the `m + m` RHS leaves `[0, 2^64)`). -/
theorem ovfBody_no_result : bodyDenote ovfBody maxState = none := by
  simp only [ovfBody, bodyDenote, blockThread, stmtDenote, maxState, State.setVar, State.bind,
    Block.blkTail, execDenote, asInt, evalArith, rawArith, IntTy.bound, IntTy.width]
  decide

/-- A false `ens` clause: `result < result` (never true). The vacuity escape would
    "discharge" this through the false antecedent. -/
def ensFalse : Expr := Expr.cmp CmpOp.lt (Expr.var "result") (Expr.var "result")

/-- The hypothesize-contract obligation for this item, with the false `ens`. The
    antecedent binds the body's result `r` via `bodyConverges`, the consequent denotes
    the (false) `ens` at the value bridge — the §4.1.5 form. -/
def contractObligation : Prop :=
  ∀ r : BVal, bodyConverges ovfBody maxState (.int r) → denote 0 ensFalse (bindResult baseEnv (.int r))

/-- The conjoined overflow obligation (§4.1.5): under the precondition, the body produces
    a value. Exported alongside the contract obligation. -/
def overflowObligation : Prop := (bodyDenote ovfBody maxState).isSome

/-- Half 1 — the contract obligation is vacuously provable even with the false `ens`.
    `bodyConverges ovfBody maxState (.int r)` is `none = some (.int r)`, false for every
    `r` (`ovfBody_no_result`), so the implication holds regardless of the (false)
    consequent. This is the vacuity the hypothesize form admits if the contract class
    stood alone. -/
theorem contract_vacuously_holds : contractObligation := by
  intro r hconv
  rw [bodyConverges, ovfBody_no_result] at hconv
  exact absurd hconv (by simp)

/-- Half 2 — the conjoined overflow obligation is refuted. `(bodyDenote ovfBody
    maxState).isSome` is false (`bodyDenote = none`), so the overflow class fails, and by
    the §4.1 conjunction rule the item does not certify, the vacuous contract discharge
    notwithstanding. -/
theorem overflow_obligation_fails : ¬ overflowObligation := by
  simp only [overflowObligation, ovfBody_no_result, Option.isSome]
  decide

/-- The certificate-level conjunction: the contract class is vacuously discharged but the
    overflow class fails, so the conjoined item obligation is not met. The vacuous contract
    can never reach an L3 certificate; the failing overflow class blocks it. This is the
    soundness condition that makes the hypothesize form safe (the #212(b) resolution). -/
theorem conjunction_blocks_vacuous_cert :
    contractObligation ∧ ¬ overflowObligation :=
  ⟨contract_vacuously_holds, overflow_obligation_fails⟩

end Thermite.PinExecOverflowVacuity
