/-
  CRITIC PIN — the (v-a) `h_dec` STATEMENT-SHAPE DRIFT (#264, the critic audit of
  commit 99c7f304; `.design/verified/proof-backends.md` §4.2.2 / REQ-11.3).

  THE DIVERGENCE. The design §4.2.2 pins `loopDenote_exits_of_dec`'s statement shape:

      h_dec : ∀ st st', I st → condBool cond st = some true →
                blockThread lbody st = some st' → μ st' < μ st ∧ 0 ≤ μ st'
                                                                       ^^^^^
  (the POST-state bound — the measure stays non-negative AFTER each genuine step).
  The SHIPPED theorem (`Exec/WhileBody.lean`, `loopDenote_exits_of_dec`) instead takes

      h_dec : … → μ st' < μ st ∧ 0 ≤ μ st
                                       ^^
  (the PRE-state bound). The REQ-11.3 SHIPPED row claims "STATEMENT SHAPE is the
  §4.2.2 sketch verbatim (`h_dec : … → μ st' < μ st ∧ 0 ≤ μ st`)" — quoting a shape
  that is NOT the §4.2.2 sketch. The builder declared exactly ONE adaptation
  (`prefix` → `prefixB`, a Lean-keyword rename); the moved prime is a SECOND,
  UNDECLARED adaptation, and it is SEMANTIC, not syntactic.

  THIS PIN proves, kernel-checked:

  1. `shipped_hdec_is_not_the_pinned_shape` — a CONCRETE instance (the SHIPPED L1
     loop machinery: `cond := lo < n`, `lbody := lo = lo + 1`, `I := (· = s0)` with
     `s0 : lo = 0, n = 1`, `μ := -lo`) on which the SHIPPED `h_dec` premise HOLDS
     while the DESIGN-pinned `h_dec` premise is REFUTED (the step lands at `lo = 1`,
     so `μ st' = -1 < 0` — the post-state bound fails, the pre-state bound holds).
     The two hypothesis shapes are therefore NOT equivalent: "verbatim" is FALSE,
     and the shipped theorem is a genuinely DIFFERENT statement from the pinned one.

  2. `design_hdec_implies_shipped_hdec` + `loopDenote_exits_of_dec_design_shape` —
     the DIRECTION-SAFETY adjudication: the design-pinned shape IMPLIES the shipped
     shape pointwise (`μ st' < μ st ∧ 0 ≤ μ st'` forces `0 < μ st`), so the design's
     verbatim theorem is a one-line corollary of the shipped one. The drift WEAKENS
     the hypothesis (strengthens the theorem): it is a RECORD-FIDELITY divergence,
     not a soundness hole. The fix (re-pin §4.2.2 to the shipped shape with the
     adaptation declared, or restate the theorem to the pinned shape) is the
     GENERATOR's, not the critic's — this file only pins that the divergence is real.

  NOTE this file imports `Thermite.Exec.WhileBody` and applies
  `loopDenote_exits_of_dec` — its compilation is ALSO a kernel-checked refutation of
  the REQ-status-table row (added in the SAME commit 99c7f304) asserting
  "`lean/Thermite/` has NO `whileBodyDenote`/…/`loopDenote_exits_of_dec`".

  Like the other pins it must keep compiling (kernel-checked, standard axioms only,
  NO `sorry`/`admit`/`native_decide`).
-/
import Thermite.Exec.WhileBody

namespace Thermite.PinWhileDecShape

open Thermite.Exec

/-- The SHIPPED `h_dec` hypothesis shape (`Exec/WhileBody.lean`,
    `loopDenote_exits_of_dec`): descent + the PRE-state bound `0 ≤ μ st`. -/
abbrev shippedHdec (cond : ExecExpr) (lbody : Block) (I : State → Prop)
    (μ : State → Int) : Prop :=
  ∀ st st', I st → condBool cond st = some true →
    blockThread lbody st = some st' → μ st' < μ st ∧ 0 ≤ μ st

/-- The DESIGN-pinned `h_dec` hypothesis shape (proof-backends.md §4.2.2, quoted
    verbatim): descent + the POST-state bound `0 ≤ μ st'`. -/
abbrev designHdec (cond : ExecExpr) (lbody : Block) (I : State → Prop)
    (μ : State → Int) : Prop :=
  ∀ st st', I st → condBool cond st = some true →
    blockThread lbody st = some st' → μ st' < μ st ∧ 0 ≤ μ st'

/-! ## The separating instance — the SHIPPED L1 machinery with `μ := -lo`

  `cond := lo < n`, `lbody := lo = lo + 1` (the SHIPPED `l1Cond`/`l1Body`,
  `Exec/Loop.lean`), the single `I`-state `s0 : lo = 0, n = 1`, and the measure
  `μ := -lo`. The one genuine step goes `lo : 0 → 1`, so `μ : 0 → -1`:
  strict descent ✓, pre-state bound `0 ≤ 0` ✓, post-state bound `0 ≤ -1` ✗. -/

/-- The separating state: `lo = 0`, `n = 1` (both `usize`), `lo` in scope. -/
def s0 : State :=
  { env := { vars := fun s => if s = "n" then .int ⟨.usize, 1⟩ else .int ⟨.usize, 0⟩
             slices := fun _ => [] }
    scope := fun s => s = "lo" }

/-- The separating measure `μ := -lo` (negated cell read — strictly decreasing
    across the `lo = lo + 1` step, non-negative BEFORE it, negative AFTER it). -/
def negLo (st : State) : Int := - execIntValue (st.env.vars "lo")

/-- The loop-head guard holds at `s0` (`0 < 1`). -/
theorem cond_true_at_s0 : condBool l1Cond s0 = some true := by
  simp only [condBool, l1Cond, s0, execDenote, asInt, cmpVal, bind, Option.bind]
  decide

/-- The genuine `lo = lo + 1` step from `s0` lands at a state whose `lo` cell is `1`
    (the concrete decode, the `Exec/Loop.lean` L2-witness pattern). -/
theorem step_lo_cell :
    (blockThread l1Body s0).map (fun s => s.env.vars "lo") = some (.int ⟨.usize, 1⟩) := by
  simp only [l1Body, s0, blockThread, stmtDenote, execDenote, asInt, evalArith,
        rawArith, IntTy.bound, IntTy.width, State.setVar, bind, Option.bind, Option.map]
  decide

/-- **Half 1 — the SHIPPED `h_dec` premise HOLDS on the separating instance.** The
    only `I`-state is `s0` (`μ = 0`); the genuine step lands at `lo = 1` (`μ = -1`):
    `-1 < 0` (descent) and `0 ≤ 0` (the PRE-state bound). -/
theorem shipped_hdec_holds : shippedHdec l1Cond l1Body (· = s0) negLo := by
  intro st st' hst _ hb
  subst hst
  -- decode the step result's `lo` cell via the functional `blockThread`.
  have hmap := step_lo_cell
  rw [Option.map_eq_some_iff] at hmap
  obtain ⟨st'', hb'', hlo''⟩ := hmap
  rw [hb] at hb''
  obtain rfl : st' = st'' := (Option.some.injEq _ _).mp hb''
  have h1 : negLo st' = -1 := by simp only [negLo, hlo'']; rfl
  have h0 : negLo s0 = 0 := by
    simp only [negLo, s0, if_neg (by decide : ¬ ("lo" = "n"))]; rfl
  exact ⟨by omega, by omega⟩

/-- **Half 2 — the DESIGN-pinned `h_dec` premise is REFUTED on the SAME instance.**
    The step's post-state has `μ st' = -1`, so the §4.2.2 conjunct `0 ≤ μ st'` is
    FALSE. The shipped and pinned hypothesis shapes are NOT the same statement. -/
theorem design_hdec_fails : ¬ designHdec l1Cond l1Body (· = s0) negLo := by
  intro h
  have hmap := step_lo_cell
  rw [Option.map_eq_some_iff] at hmap
  obtain ⟨st', hb, hlo⟩ := hmap
  obtain ⟨_, hge⟩ := h s0 st' rfl cond_true_at_s0 hb
  have h1 : negLo st' = -1 := by simp only [negLo, hlo]; rfl
  omega

/-- **THE PIN — the two `h_dec` shapes are NOT equivalent (the "verbatim" claim is
    refuted).** One concrete instance satisfies the SHIPPED hypothesis and refutes
    the DESIGN-pinned one, so `loopDenote_exits_of_dec` as shipped is a genuinely
    DIFFERENT theorem from the §4.2.2-pinned statement — an UNDECLARED semantic
    adaptation (the builder declared only the `prefix` → `prefixB` rename). -/
theorem shipped_hdec_is_not_the_pinned_shape :
    shippedHdec l1Cond l1Body (· = s0) negLo
    ∧ ¬ designHdec l1Cond l1Body (· = s0) negLo :=
  ⟨shipped_hdec_holds, design_hdec_fails⟩

/-! ## Direction safety — the drift weakens the HYPOTHESIS, never the conclusion -/

/-- The DESIGN-pinned shape IMPLIES the shipped shape pointwise: `μ st' < μ st`
    together with `0 ≤ μ st'` forces `0 < μ st`, hence `0 ≤ μ st`. -/
theorem design_hdec_implies_shipped_hdec
    (cond : ExecExpr) (lbody : Block) (I : State → Prop) (μ : State → Int)
    (h : designHdec cond lbody I μ) : shippedHdec cond lbody I μ := by
  intro st st' hI hc hb
  obtain ⟨hlt, hge'⟩ := h st st' hI hc hb
  exact ⟨hlt, by omega⟩

/-- The design's VERBATIM theorem (§4.2.2's `0 ≤ μ st'` shape) is a one-line
    corollary of the shipped `loopDenote_exits_of_dec`: the shipped theorem is
    STRICTLY MORE GENERAL, so the drift is a RECORD-FIDELITY divergence (the REQ-11.3
    "verbatim" claim is false), NOT a soundness hole — the (v-b) exporter can target
    either shape. Recorded so the adjudication is kernel-checked in both directions. -/
theorem loopDenote_exits_of_dec_design_shape (cond : ExecExpr) (lbody : Block)
    (I : State → Prop) (μ : State → Int)
    (h_pres : ∀ st, I st → condBool cond st = some true →
                ∀ st', blockThread lbody st = some st' → I st')
    (h_cond_total : ∀ st, I st → (condBool cond st).isSome)
    (h_progress   : ∀ st, I st → condBool cond st = some true →
                      (blockThread lbody st).isSome)
    (h_dec        : ∀ st st', I st → condBool cond st = some true →
                      blockThread lbody st = some st' → μ st' < μ st ∧ 0 ≤ μ st') :
    ∀ st, I st → ∃ fuel stf, loopDenote cond lbody fuel st = some stf :=
  loopDenote_exits_of_dec cond lbody I μ h_pres h_cond_total h_progress
    (design_hdec_implies_shipped_hdec cond lbody I μ h_dec)

end Thermite.PinWhileDecShape
