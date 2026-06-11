/-
  CRITIC PIN — the EXEC-BODY OVERFLOW-VACUITY ESCAPE (#253, proof-backends.md
  §4.1.5/§4.1.6). The third of the FOUR bridge-divergence pins, and the
  CERTIFICATE-LEVEL CONJUNCTION regression oracle (the Pin B shape). A body that ALWAYS
  overflows under the precondition (`bodyDenote = none`) makes the HYPOTHESIZE CONTRACT
  obligation VACUOUSLY kernel-accept WITH A FALSE `ens` — AND the pin proves the
  conjoined OVERFLOW obligation REFUTED at the same state. The vacuous CONTRACT discharge
  must stay UNREACHABLE as a certificate, blocked by the failing OVERFLOW class (the §4.1
  conjunction rule: an item certifies only when BOTH the CONTRACT and the OVERFLOW class
  discharge).

  THE WITNESS. The body `{ let a = m + m; a }` with `m := 2^64 − 1` (the
  `body_overflow_rhs_has_no_result` shape, `Exec/Stmt.lean`): the `let` RHS `m + m`
  OVERFLOWS `u64`, so `bodyDenote = none`.

    - THE HYPOTHESIZE CONTRACT obligation `∀ r, bodyConverges body st (.int r) →
      ensStable(bindResult …)` is VACUOUSLY provable EVEN FOR A FALSE `ens` (here
      `ens := result < result`, never true): the antecedent `bodyConverges = (none = some …)`
      is FALSE for every `r`, so the implication holds. This is the vacuity the HYPOTHESIZE
      form would let through IF the CONTRACT class stood alone.
    - THE CONJOINED OVERFLOW obligation `(bodyDenote body st).isSome` is FALSE — so the
      item does NOT certify (the conjunction rule rejects it). The vacuous CONTRACT can
      never reach a certificate.

  Both halves are pinned: the vacuous CONTRACT discharge AND the conjoined OVERFLOW
  refutation. This is why the HYPOTHESIZE form is SOUND (§4.1's #212(b) resolution).
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

/-- The ALWAYS-OVERFLOWING body `{ let a = m + m; a }` (the `body_overflow_rhs_has_no_result`
    shape): the `let` RHS `m + m` overflows `u64` when `m` is at the rim. -/
def ovfBody : Block :=
  .mk [ .letS "a" (.arith .add (.var "m") (.var "m")) ] (some (.var "a"))

/-- The state seeding `m := 2^64 − 1` (max `u64`) — the precondition forces `m` at the rim. -/
def maxState : State :=
  { env := { vars := fun s => if s = "m" then .int ⟨.u64, (2 : Int) ^ 64 - 1⟩
                              else .int ⟨.u64, 0⟩
             slices := fun _ => [] }
    scope := fun _ => false }

/-- The body has NO result at the overflow (`bodyDenote = none`) — the no-overflow
    OBLIGATION fails (the `m + m` RHS leaves `[0, 2^64)`). -/
theorem ovfBody_no_result : bodyDenote ovfBody maxState = none := by
  simp only [ovfBody, bodyDenote, blockThread, stmtDenote, maxState, State.setVar, State.bind,
    Block.blkTail, execDenote, asInt, evalArith, rawArith, IntTy.bound, IntTy.width]
  decide

/-- A FALSE `ens` clause: `result < result` (never true). The vacuity escape would
    "discharge" THIS through the false antecedent. -/
def ensFalse : Expr := Expr.cmp CmpOp.lt (Expr.var "result") (Expr.var "result")

/-- The HYPOTHESIZE CONTRACT obligation for this item, with the FALSE `ens`. The
    antecedent binds the body's result `r` via `bodyConverges`, the consequent denotes
    the (false) `ens` at the value bridge — exactly the §4.1.5 form. -/
def contractObligation : Prop :=
  ∀ r : BVal, bodyConverges ovfBody maxState (.int r) → denote 0 ensFalse (bindResult baseEnv (.int r))

/-- THE CONJOINED OVERFLOW obligation (§4.1.5): under the precondition, the body produces
    a value. EXPORTED ALONGSIDE the CONTRACT obligation. -/
def overflowObligation : Prop := (bodyDenote ovfBody maxState).isSome

/-- **Half 1 — the CONTRACT obligation is VACUOUSLY provable even with the FALSE `ens`.**
    `bodyConverges ovfBody maxState (.int r)` is `none = some (.int r)`, FALSE for every
    `r` (`ovfBody_no_result`), so the implication holds regardless of the (false)
    consequent. This is the vacuity the HYPOTHESIZE form admits IF the CONTRACT class
    stood alone. -/
theorem contract_vacuously_holds : contractObligation := by
  intro r hconv
  rw [bodyConverges, ovfBody_no_result] at hconv
  exact absurd hconv (by simp)

/-- **Half 2 — the conjoined OVERFLOW obligation is REFUTED.** `(bodyDenote ovfBody
    maxState).isSome` is FALSE (`bodyDenote = none`), so the OVERFLOW class FAILS — and by
    the §4.1 conjunction rule the item does NOT certify, the vacuous CONTRACT discharge
    notwithstanding. -/
theorem overflow_obligation_fails : ¬ overflowObligation := by
  simp only [overflowObligation, ovfBody_no_result, Option.isSome]
  decide

/-- The certificate-level conjunction: the CONTRACT class is vacuously discharged BUT the
    OVERFLOW class fails, so the conjoined item obligation is NOT met. The vacuous CONTRACT
    can never reach an L3 certificate — the failing OVERFLOW class blocks it. This is the
    SOUNDNESS condition that makes the HYPOTHESIZE form safe (the #212(b) resolution). -/
theorem conjunction_blocks_vacuous_cert :
    contractObligation ∧ ¬ overflowObligation :=
  ⟨contract_vacuously_holds, overflow_obligation_fails⟩

end Thermite.PinExecOverflowVacuity
