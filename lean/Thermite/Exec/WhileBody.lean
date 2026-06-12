/-
  Thermite/Exec/WhileBody.lean — the (v-a) WHILE-BODY COMPOSITION LAYER: the first
  `S_B`×`S_Loop` composition artifact (increment (v), blocker #264;
  `.design/verified/proof-backends.md` §4.2.2 / REQ-11.1/REQ-11.2/REQ-11.3). This is
  the SPINE PREREQUISITE the (v-b) exporter targets, EXACTLY parallel to increment
  (iv)'s `Exec/Stmt.lean` bridge layer (#253 iv-a) and increment (ii)'s `Stabilize.lean`
  layer (#240): it lands FIRST, kernel-green with the standard axiom set
  `{propext, Classical.choice, Quot.sound}` (NO `sorryAx`, NO new axiom), every existing
  theorem + shipped pin still green.

  ════════════════════════════════════════════════════════════════════════════════
  WHAT THIS LAYER IS — and what it is NOT.
  ════════════════════════════════════════════════════════════════════════════════

  Increment (iv) (`Exec/Stmt.lean`) stopped at STRAIGHT-LINE bodies; `bodyDenote` has NO
  loop arm. The LOOP brick of the spine (`Exec/Loop.lean`: `loopDenote` + the
  PARTIAL-CORRECTNESS `while_rule`) is ALREADY kernel-proven, but `loopDenote` yields a
  `State`, not the body's `ExecVal` result, and is a SEPARATE artifact never composed into
  `bodyDenote`. (v-a) supplies the ONE genuinely missing piece — a COMPOSED whole-body
  denotation `whileBodyDenote` (prefix `blockThread` → the SHIPPED `loopDenote` → tail
  `execDenote`) — plus the termination story §4.2.3 decides (`loopDenote_exits_of_dec`).

  `Exec/Stmt.lean` and `Exec/Loop.lean` are NOT modified — this layer composes AROUND
  them (preserving `Exec/Loop.lean`'s `WhileLoop`-not-a-`Stmt`-arm modeling decision; NO
  re-proof of `body_ref_sound`/`while_rule`). The composed denotation is the `Option`-monad
  composition of the three SHIPPED transformers; a prefix / iteration / tail failure OR
  FUEL EXHAUSTION is `none` (GENUINE — never a forged value, the §4.1.5 no-NB-layer
  argument verbatim).

  ════════════════════════════════════════════════════════════════════════════════
  THE SIX NAMED PIECES (§4.2.2, statement shapes pinned by the design):
  ════════════════════════════════════════════════════════════════════════════════

    1. `whileBodyDenote`             — the composed whole-body denotation (prefix → loop → tail).
    2. `whileBodyConverges`          — the ∃-fuel convergence relation (result bound THROUGH it).
    3. `loopDenote_fuel_mono`        — surplus fuel after exit is unconsumed (induction on fuel).
    4. `whileBodyConverges_unique`   — overlap-at-max via fuel-mono + functional determinism.
    5. `while_compose`               — the loop-exit-to-ens composition (`while_rule` lifted).
    6. `loopDenote_exits_of_dec`     — the TERMINATION bridge (strong induction on `(μ st).toNat`).

  TERMINATION HONESTY (§4.2.3): `loopDenote_exits_of_dec` is the `converges_imp_stabilizes`
  mirror one domain over — dec-VALIDITY (strict bounded-below descent of the denoted
  measure across each GENUINE `blockThread` step) + PROGRESS ⟹ the loop EXITS at some fuel.
  The exit witness `while_rule` HYPOTHESIZES is shown to EXIST; dec-validity is the
  discharge METHOD, the exit witness is the semantic CONTENT (the REQ-1.2 pattern). The
  SPINE is UNCHANGED: `while_rule` stays partial correctness (`loop-tv.md` REQ-4 stands);
  totality lives in this EXPORTED bridge.

  DEPENDENCIES: Lean 4 CORE ONLY. Reuses `Exec/Loop.lean`'s `loopDenote`/`condBool`/
  `while_rule` + `Exec/Stmt.lean`'s `blockThread`/`State` + `Exec.lean`'s `execDenote`/
  `ExecVal`. NO Mathlib, NO Lean-SMT, NO `sorry`/`admit`/`native_decide`. The proofs are
  induction on `Nat` fuel / strong induction on `(μ st).toNat` + `simp`/`omega`/`cases`.
  Mirrors the spine's core-only discipline.
-/
import Thermite.Exec.Loop

namespace Thermite.Exec

/-! ## (REQ-11.1) `whileBodyDenote` — the composed whole-body denotation

  The FIRST `S_B`×`S_Loop` artifact: the `Option`-monad composition of the three SHIPPED
  transformers — the straight-line PREFIX (`blockThread prefixB`, a tail-less `Block`), the
  SHIPPED fuel-indexed iteration (`loopDenote cond lbody fuel`, `none` propagates), and the
  tail value at the exit state (`execDenote tail`, in the loop's exit `env`). A prefix /
  iteration / tail failure OR fuel exhaustion is `none`. `Exec/Stmt.lean` / `Exec/Loop.lean`
  are UNCHANGED — this composes AROUND them. (The design names the first argument `prefix`;
  `prefix` is a Lean keyword, so the binder is `prefixB` — a SYNTAX adaptation only, the
  semantics are the §4.2.2 sketch exactly.) -/
def whileBodyDenote (prefixB : Block) (cond : ExecExpr) (lbody : Block)
    (tail : ExecExpr) (fuel : Nat) (st : State) : Option ExecVal := do
  let st₁ ← blockThread prefixB st          -- the straight-line PREFIX (a tail-less Block)
  let stf ← loopDenote cond lbody fuel st₁  -- the SHIPPED iteration (`none` propagates)
  execDenote tail stf.env                   -- the tail at the exit state

/-! ## (REQ-11.1) `whileBodyConverges` — the ∃-fuel convergence relation

  The result is bound THROUGH the relation (the #214 discipline). The ∃ is FORCED (the
  #213 lesson): the iteration count is ENV-DEPENDENT (the L1 fixture exits at fuel `n+1`
  with `n` ∀-quantified), so NO export-time fuel exists — the relation quantifies the index
  away. NO NB layer (the §4.1.5 argument verbatim): `whileBodyDenote`'s `none` is GENUINE (a
  failure or fuel exhaustion), never a forged value, because every constituent
  (`blockThread`/`loopDenote`/`execDenote`) is bottom-distinguishing via `Option`. -/
abbrev whileBodyConverges (prefixB : Block) (cond : ExecExpr) (lbody : Block)
    (tail : ExecExpr) (st : State) (r : ExecVal) : Prop :=
  ∃ fuel, whileBodyDenote prefixB cond lbody tail fuel st = some r

/-! ## (REQ-11.1) `loopDenote_fuel_mono` — surplus fuel after exit is unconsumed

  Once the loop EXITS at fuel `f` (`loopDenote .. f st = some stf`), ANY larger fuel `g ≥ f`
  yields the SAME exit state. The extra fuel is simply never consumed (the exit branch
  returns `some st` without recursing). Proof: induction on `f`. This is the lever
  `whileBodyConverges_unique` overlaps two converging fuels at their max. -/
theorem loopDenote_fuel_mono (cond : ExecExpr) (body : Block) :
    ∀ (f : Nat) (st stf : State), loopDenote cond body f st = some stf →
      ∀ g, f ≤ g → loopDenote cond body g st = some stf := by
  intro f
  induction f with
  | zero =>
      -- fuel `0`: `loopDenote .. 0 st = none ≠ some stf`, vacuous.
      intro st stf h_run
      simp only [loopDenote] at h_run
      exact absurd h_run (by simp)
  | succ f ih =>
      intro st stf h_run g hg
      -- `g ≥ f + 1`, so `g = g' + 1` for some `g' ≥ f`.
      obtain ⟨g', rfl⟩ : ∃ g', g = g' + 1 := ⟨g - 1, by omega⟩
      have hg' : f ≤ g' := by omega
      cases hc : condBool cond st with
      | none =>
          -- a non-bool condition: both sides `none`, contradicting `some stf`.
          simp only [loopDenote, hc, bind, Option.bind] at h_run
          exact absurd h_run (by simp)
      | some c =>
          -- reduce both fuels' iteration with the CONCRETE head value `some c`.
          simp only [loopDenote, hc, bind, Option.bind] at h_run ⊢
          cases c with
          | false =>
              -- EXIT branch: both fuels return `some st` (no recursion — surplus unconsumed).
              simp only [Bool.false_eq_true, reduceIte, Option.some.injEq] at h_run ⊢
              exact h_run
          | true =>
              -- CONTINUE branch: the body steps to some `st'`, the IH lifts the smaller-fuel
              -- recursion `loopDenote cond body f st' = some stf` to `g' ≥ f`.
              simp only [if_true] at h_run ⊢
              cases hb : blockThread body st with
              | none =>
                  simp only [hb] at h_run
                  exact absurd h_run (by simp)
              | some st' =>
                  simp only [hb] at h_run ⊢
                  exact ih st' stf h_run g' hg'

/-! ## (REQ-11.1) `whileBodyConverges_unique` — the `stabilizes_unique` mirror

  Two converging fuels overlap at their MAX (via `loopDenote_fuel_mono`), where the whole
  composition is functional (`blockThread`/`loopDenote`/`execDenote` are functions), so the
  results are equal by `Option.some.injEq`. Binding `r` through the ∃-fuel relation is
  thereby WELL-DEFINED — the exporter computes NO value; the relation forces the true one. -/
theorem whileBodyConverges_unique (prefixB : Block) (cond : ExecExpr) (lbody : Block)
    (tail : ExecExpr) (st : State) (r₁ r₂ : ExecVal)
    (h₁ : whileBodyConverges prefixB cond lbody tail st r₁)
    (h₂ : whileBodyConverges prefixB cond lbody tail st r₂) :
    r₁ = r₂ := by
  obtain ⟨f₁, h₁⟩ := h₁
  obtain ⟨f₂, h₂⟩ := h₂
  -- Overlap the two fuels at their max `max f₁ f₂`. Both `whileBodyDenote`s agree there.
  -- Pull the prefix/loop/tail apart at each fuel.
  simp only [whileBodyDenote, bind, Option.bind] at h₁ h₂
  cases hpre : blockThread prefixB st with
  | none =>
      rw [hpre] at h₁
      exact absurd h₁ (by simp)
  | some st₁ =>
      rw [hpre] at h₁ h₂
      simp only at h₁ h₂
      -- decode each loop result, lift to `m` by fuel-mono, then the tail is a function.
      cases hl₁ : loopDenote cond lbody f₁ st₁ with
      | none => rw [hl₁] at h₁; exact absurd h₁ (by simp)
      | some stf₁ =>
          cases hl₂ : loopDenote cond lbody f₂ st₁ with
          | none => rw [hl₂] at h₂; exact absurd h₂ (by simp)
          | some stf₂ =>
              rw [hl₁] at h₁; rw [hl₂] at h₂
              simp only at h₁ h₂
              -- Lift both loop results to fuel `max f₁ f₂`; fuel-mono forces ONE exit state.
              have hm₁ : loopDenote cond lbody (max f₁ f₂) st₁ = some stf₁ :=
                loopDenote_fuel_mono cond lbody f₁ st₁ stf₁ hl₁ (max f₁ f₂) (Nat.le_max_left f₁ f₂)
              have hm₂ : loopDenote cond lbody (max f₁ f₂) st₁ = some stf₂ :=
                loopDenote_fuel_mono cond lbody f₂ st₁ stf₂ hl₂ (max f₁ f₂) (Nat.le_max_right f₁ f₂)
              have hstf : stf₁ = stf₂ := by
                rw [hm₁] at hm₂; exact (Option.some.injEq _ _).mp hm₂
              subst hstf
              -- The tail `execDenote tail stf₁.env` is the SAME function in both, so r₁ = r₂.
              rw [h₁] at h₂
              exact (Option.some.injEq _ _).mp h₂

/-! ## (REQ-11.2) `while_compose` — the loop-exit-to-ens composition lemma

  The bridge wrapping the straight-line prefix/tail segments AROUND `while_rule`: ANY
  converged whole-body result is the tail's value at SOME exit state satisfying `I ∧ ¬cond`.
  Proof shape (§4.2.2): unfold `whileBodyDenote`; the prefix step is a FUNCTION, so the
  loop-entry state is DETERMINED; apply the SHIPPED `while_rule` to the middle segment; the
  tail value rides out in the `∃`. The prefix-entry invariant is HYPOTHESIZED (`I` at the
  loop-entry state for every prefix outcome) — the (v-b) `_entry` obligation discharges it. -/
theorem while_compose (prefixB lbody : Block) (cond tail : ExecExpr) (I : State → Prop)
    (h_pres : ∀ st, I st → condBool cond st = some true →
                ∀ st', blockThread lbody st = some st' → I st') :
    ∀ st₀ fuel r,
      whileBodyDenote prefixB cond lbody tail fuel st₀ = some r →
      (∀ st₁, blockThread prefixB st₀ = some st₁ → I st₁) →
      ∃ stf, I stf ∧ condBool cond stf = some false ∧ execDenote tail stf.env = some r := by
  intro st₀ fuel r h_run h_entry
  -- Unfold the composition. The prefix step is a function; decode it.
  simp only [whileBodyDenote, bind, Option.bind] at h_run
  cases hpre : blockThread prefixB st₀ with
  | none =>
      rw [hpre] at h_run
      exact absurd h_run (by simp)
  | some st₁ =>
      rw [hpre] at h_run
      simp only at h_run
      -- `I` holds at the loop-entry state (the prefix outcome).
      have hI₁ : I st₁ := h_entry st₁ hpre
      -- Decode the loop result.
      cases hl : loopDenote cond lbody fuel st₁ with
      | none =>
          rw [hl] at h_run
          exact absurd h_run (by simp)
      | some stf =>
          rw [hl] at h_run
          simp only at h_run
          -- Apply the SHIPPED partial-correctness `while_rule` to the middle segment.
          have hexit : I stf ∧ condBool cond stf = some false :=
            while_rule cond lbody I h_pres fuel st₁ stf hI₁ hl
          exact ⟨stf, hexit.1, hexit.2, h_run⟩

/-! ## (REQ-11.3) `loopDenote_exits_of_dec` — the TERMINATION bridge

  The `converges_imp_stabilizes` mirror, one domain over (§4.2.3's currency). dec-VALIDITY
  (strict, bounded-below descent of the denoted measure `μ` across each GENUINE `blockThread`
  step — `h_dec`) + PROGRESS (the condition denotes a bool — `h_cond_total`; the body steps at
  every invariant state where the condition holds — `h_progress`) ⟹ the loop EXITS at some
  fuel. This is the `h_run` witness `while_rule` HYPOTHESIZES, shown to EXIST.

  Proof shape (§4.2.2): strong induction on `(μ st).toNat`. The measure is a NON-NEGATIVE
  bounded-below `Int` (`0 ≤ μ st` from `h_dec`), so `(μ st).toNat` is a genuine well-founded
  `Nat` measure. At each `I`-state: the condition is total (`h_cond_total`). If it is `false`,
  fuel `1` exits (`some st`). If `true`, the body progresses to `st'` (`h_progress`),
  preservation keeps `I st'`, descent gives `μ st' < μ st ∧ 0 ≤ μ st`, so `(μ st').toNat <
  (μ st).toNat` — the IH supplies an exit fuel for `st'`, and ONE MORE fuel exits from `st`.

  REQ-1.2 pattern EXACTLY: dec-validity is the discharge METHOD, the exit witness is the
  semantic CONTENT, the bridge lemma carries one to the other — never assumed (the §4.2.2
  soundness asymmetry: a mis-denoted `μ` is a COMPLETENESS bug, not a soundness seam, because
  `h_dec` is stated against the REAL step semantics, not a `μ`-dependent program denotation). -/
theorem loopDenote_exits_of_dec (cond : ExecExpr) (lbody : Block)
    (I : State → Prop) (μ : State → Int)
    (h_pres : ∀ st, I st → condBool cond st = some true →
                ∀ st', blockThread lbody st = some st' → I st')
    (h_cond_total : ∀ st, I st → (condBool cond st).isSome)
    (h_progress   : ∀ st, I st → condBool cond st = some true →
                      (blockThread lbody st).isSome)
    (h_dec        : ∀ st st', I st → condBool cond st = some true →
                      blockThread lbody st = some st' → μ st' < μ st ∧ 0 ≤ μ st) :
    ∀ st, I st → ∃ fuel stf, loopDenote cond lbody fuel st = some stf := by
  -- Strong induction on the NON-NEGATIVE measure `(μ st).toNat`. Generalize over `st`
  -- via a well-founded recursion keyed on `n = (μ st).toNat`.
  intro st hI
  -- The induction variable is `n`; we recover `st` from `(μ st).toNat = n`.
  suffices H : ∀ n : Nat, ∀ st, I st → (μ st).toNat = n →
      ∃ fuel stf, loopDenote cond lbody fuel st = some stf by
    exact H (μ st).toNat st hI rfl
  intro n
  induction n using Nat.strongRecOn with
  | ind n ih =>
      intro st hI hn
      -- The condition is total at `st` (`I st`): case on its bool value.
      have hct := h_cond_total st hI
      cases hc : condBool cond st with
      | none => rw [hc] at hct; exact absurd hct (by simp)
      | some c =>
          cases c with
          | false =>
              -- EXIT: fuel `1` returns `some st` (the false-condition branch, no recursion).
              refine ⟨1, st, ?_⟩
              simp only [loopDenote, bind, Option.bind, hc, Bool.false_eq_true, if_false]
          | true =>
              -- CONTINUE: the body progresses to `st'`; descent shrinks the measure; the IH
              -- supplies an exit fuel for `st'`, one more fuel exits from `st`.
              have hprog := h_progress st hI hc
              cases hb : blockThread lbody st with
              | none => rw [hb] at hprog; exact absurd hprog (by simp)
              | some st' =>
                  have hI' : I st' := h_pres st hI hc st' hb
                  obtain ⟨hlt, hge⟩ := h_dec st st' hI hc hb
                  -- The descent gives `μ st' < μ st` and `0 ≤ μ st`. Two cases on `μ st`:
                  by_cases hpos : 0 < μ st
                  · -- `μ st ≥ 1`: `(μ st').toNat < (μ st).toNat = n` even if `μ st' < 0`
                    -- (then `(μ st').toNat = 0 < 1 ≤ μ st`). `omega` knows `Int.toNat`.
                    have hmono : (μ st').toNat < n := by omega
                    -- the IH gives an exit fuel for `st'`; one more fuel exits from `st`.
                    obtain ⟨fuel', stf, hrun'⟩ := ih (μ st').toNat hmono st' hI' rfl
                    refine ⟨fuel' + 1, stf, ?_⟩
                    simp only [loopDenote, bind, Option.bind, hc, if_true, hb]
                    exact hrun'
                  · -- `μ st = 0` (`0 ≤ μ st`, `¬ 0 < μ st`): the step gives `μ st' < 0`. At
                    -- `st'` the condition CANNOT be `true` — `h_dec` there would force
                    -- `0 ≤ μ st'`, contradicting `μ st' < 0`. So the loop exits next iteration
                    -- (fuel `2`): step to `st'`, then the false condition returns `some st'`.
                    have hmust_exit : condBool cond st' = some false := by
                      have hct' := h_cond_total st' hI'
                      cases hc' : condBool cond st' with
                      | none => rw [hc'] at hct'; exact absurd hct' (by simp)
                      | some c' =>
                          cases c' with
                          | false => rfl
                          | true =>
                              -- cond true at `st'` ⟹ the body progresses, `h_dec` gives
                              -- `0 ≤ μ st'`, contradicting `μ st' < 0` (from `μ st = 0`).
                              exfalso
                              have hprog' := h_progress st' hI' hc'
                              cases hb' : blockThread lbody st' with
                              | none => rw [hb'] at hprog'; exact absurd hprog' (by simp)
                              | some st'' =>
                                  obtain ⟨_, hge'⟩ := h_dec st' st'' hI' hc' hb'
                                  omega
                    refine ⟨2, st', ?_⟩
                    simp only [loopDenote, bind, Option.bind, hc, if_true, hb, hmust_exit,
                      Bool.false_eq_true, reduceIte]

end Thermite.Exec
