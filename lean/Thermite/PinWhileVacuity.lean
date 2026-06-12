/-
  CRITIC PIN — the WHILE-BODY TERMINATION-VACUITY ESCAPE (#264, proof-backends.md
  §4.2.3/§4.2.6 REQ-11.6). The first of the TWO (v) bridge-divergence pins, and the
  CERTIFICATE-LEVEL CONJUNCTION regression oracle for the LOOP path — the
  `PinExecOverflowVacuity` shape ONE VACUITY POSITION OVER (fuel-exhaustion `none`
  instead of overflow `none`).

  THE FORCING GROUND (§4.2.3). `loopDenote` is `none` at exhausted fuel, so for a
  NON-EXITING loop (a `while true`-shaped body) `whileBodyConverges` is FALSE at EVERY
  `r` — the HYPOTHESIZE CONTRACT obligation is then VACUOUSLY provable with a FALSE
  `ens` (the termination twin of the §4.1.5 overflow vacuity). Something conjoined MUST
  fail for that item, or the Lean-only path silently certifies a non-terminating body.

  THE DECISION (§4.2.3). Increment (v) exports a conjoined CONVERGENCE theorem: under
  `InRangeParams` + `req`, the whole body CONVERGES (`∃ r, whileBodyConverges …`). This
  single obligation JOINTLY discharges the OVERFLOW + TERMINATION classes. The pin proves
  BOTH halves at the same env: the vacuous CONTRACT discharge AND the conjoined
  CONVERGENCE obligation REFUTED (`¬ ∃ r, whileBodyConverges …`).

  THE WITNESS. The non-exiting loop `while true { }` (the condition `condBool` is
  CONSTANTLY `some true`, the body a no-op `.mk [] none` that re-arrives at the same
  state): `loopDenote` NEVER hits the false-exit branch, so it is `none` at EVERY fuel —
  hence `whileBodyDenote` is `none` at every fuel and `whileBodyConverges` is FALSE at
  every `r`.

    - THE HYPOTHESIZE CONTRACT obligation `∀ r, whileBodyConverges … (.int r) →
      ensStable(bindResult …)` is VACUOUSLY provable EVEN FOR A FALSE `ens` (here
      `ens := result < result`, never true): the antecedent is FALSE for every `r`, so
      the implication holds. This is the vacuity the HYPOTHESIZE form would let through
      IF the CONTRACT class stood alone.
    - THE CONJOINED CONVERGENCE obligation `∃ r, whileBodyConverges …` is REFUTED — so
      the item does NOT certify (the §4.2.3 conjunction rejects it). The vacuous CONTRACT
      can never reach a certificate.

  The rule-level teeth already EXIST on the spine and are NOT re-pinned here
  (`l2_no_preservation_premise_for_buggy_body`, `l3_exit_overclaim_refuted` —
  `Exec/Loop.lean`). This file is the audit artifact; like the other pins it must keep
  compiling (kernel-checked, standard axioms only).
-/
import Thermite.Stabilize
import Thermite.Exec.WhileBody

namespace Thermite.PinWhileVacuity

open Thermite Thermite.Exec

/-- A contract env with nothing bound (the `ens` reads only `result`, bound by the bridge). -/
def baseEnv : Env :=
  { ints := fun _ => 0
    seqs := fun _ => []
    optres := fun _ => OptResVal.none_
    specs := fun _ => none }

/-- The empty no-op prefix (nothing happens before the loop). -/
def noPrefix : Block := .mk [] none

/-- The CONSTANTLY-TRUE loop condition `while true` — the head test never goes false. -/
def trueCond : ExecExpr := .boolLit true

/-- The no-op loop body `{ }` (`blockThread (.mk [] none) st = some st`) — the loop
    re-arrives at the SAME state every iteration, so `loopDenote` never exits. -/
def noopBody : Block := .mk [] none

/-- A tail value `result := 0` (irrelevant — the body never converges; chosen concrete
    so `whileBodyDenote`'s tail position is well-typed). -/
def tail0 : ExecExpr := .intLit .u64 0

/-- The starting state (`inputState`-shaped; nothing in scope). -/
def st0 : State :=
  { env := { vars := fun _ => .int ⟨.u64, 0⟩, slices := fun _ => [] }
    scope := fun _ => false }

/-- The NO-OP body keeps the state (`blockThread (.mk [] none) st = some st`). -/
theorem noopBody_keeps_state (st : State) : blockThread noopBody st = some st := by
  simp only [noopBody, blockThread]

/-- The condition is CONSTANTLY `some true` (the `while true` head test). -/
theorem trueCond_is_true (st : State) : condBool trueCond st = some true := by
  show asBool =<< execDenote trueCond st.env = some true
  simp only [trueCond, execDenote]
  rfl

/-- **The loop NEVER EXITS — `loopDenote` is `none` at every fuel.** With a constantly-true
    condition and a state-preserving body, each `fuel+1` step re-arrives at the SAME state
    with ONE LESS fuel; fuel `0` is `none`. So no fuel reaches the false-exit branch. -/
theorem loop_never_exits : ∀ fuel, loopDenote trueCond noopBody fuel st0 = none := by
  intro fuel
  induction fuel with
  | zero => simp only [loopDenote]
  | succ f ih =>
      simp only [loopDenote, trueCond_is_true, noopBody_keeps_state, if_true, bind, Option.bind]
      exact ih

/-- **The whole composed body NEVER CONVERGES — `whileBodyDenote` is `none` at every fuel.**
    The loop segment is `none` at every fuel (`loop_never_exits`), so the `Option`-monad
    composition (prefix → loop → tail) is `none` — fuel exhaustion PROPAGATES, the GENUINE
    `none` of §4.1.5 (never a forged value). -/
theorem body_never_converges :
    ∀ fuel, whileBodyDenote noPrefix trueCond noopBody tail0 fuel st0 = none := by
  intro fuel
  simp only [whileBodyDenote, noPrefix, blockThread, loop_never_exits, bind, Option.bind]

/-- A FALSE `ens` clause: `result < result` (never true). The vacuity escape would
    "discharge" THIS through the false antecedent. -/
def ensFalse : Expr := Expr.cmp CmpOp.lt (Expr.var "result") (Expr.var "result")

/-- The HYPOTHESIZE CONTRACT obligation for this item, with the FALSE `ens`. The
    antecedent binds the body's result `r` via `whileBodyConverges`, the consequent
    denotes the (false) `ens` at the value bridge — the §4.2.4 `<thm>` form. -/
def contractObligation : Prop :=
  ∀ r : BVal, whileBodyConverges noPrefix trueCond noopBody tail0 st0 (.int r) →
    denote 0 ensFalse (bindResult baseEnv (.int r))

/-- THE CONJOINED CONVERGENCE obligation (§4.2.3): under the precondition, the body
    CONVERGES. EXPORTED ALONGSIDE the CONTRACT obligation (the `<thm>_converges`). -/
def convergenceObligation : Prop :=
  ∃ r : ExecVal, whileBodyConverges noPrefix trueCond noopBody tail0 st0 r

/-- **Half 1 — the CONTRACT obligation is VACUOUSLY provable even with the FALSE `ens`.**
    `whileBodyConverges … (.int r)` is `∃ fuel, none = some (.int r)`, FALSE for every `r`
    (`body_never_converges`), so the implication holds regardless of the (false)
    consequent. This is the vacuity the HYPOTHESIZE form admits IF the CONTRACT class
    stood alone — the termination twin of `PinExecOverflowVacuity`'s overflow vacuity. -/
theorem contract_vacuously_holds : contractObligation := by
  intro r hconv
  obtain ⟨fuel, hfuel⟩ := hconv
  rw [body_never_converges fuel] at hfuel
  exact absurd hfuel (by simp)

/-- **Half 2 — the conjoined CONVERGENCE obligation is REFUTED.** No `r` and no fuel make
    `whileBodyDenote … = some r` (`body_never_converges`), so `∃ r, whileBodyConverges …`
    is FALSE — the CONVERGENCE class FAILS, and by §4.2.3 the item does NOT certify, the
    vacuous CONTRACT discharge notwithstanding. -/
theorem convergence_obligation_fails : ¬ convergenceObligation := by
  rintro ⟨r, fuel, hfuel⟩
  rw [body_never_converges fuel] at hfuel
  exact absurd hfuel (by simp)

/-- The certificate-level conjunction: the CONTRACT class is vacuously discharged BUT the
    CONVERGENCE class fails, so the conjoined item obligation is NOT met. The vacuous
    CONTRACT can never reach an L3 certificate — the failing CONVERGENCE class (the
    OVERFLOW + TERMINATION joint discharge, §4.2.3) blocks it. This is the SOUNDNESS
    condition that makes the loop HYPOTHESIZE form safe. -/
theorem conjunction_blocks_vacuous_cert :
    contractObligation ∧ ¬ convergenceObligation :=
  ⟨contract_vacuously_holds, convergence_obligation_fails⟩

end Thermite.PinWhileVacuity
