/-
  Thermite/Exec/Loop.lean — the v1 `while`-LOOP extension of the exec-body
  state-transformer `S_B`: the fuel-indexed iteration semantics `loopDenote` + the
  PARTIAL-CORRECTNESS WHILE-RULE (`while_rule`) + its translation-validation meta-
  theorem `tv_meta_loop` (increment 2.2.2-ii, #163; epic #169). This is the LOOP
  brick of the proven spine — the universal Lean piece the per-run Z3 obligations
  (the Rust increment (i), `thermite-tv`) compose with to certify a loop's after-state.

  Governing design: `.design/verified/loop-tv.md` REQ-3 (the Lean iteration semantics
  + the WHILE-RULE — the EXACT rule statement + premises) + REQ-4 (the trust
  accounting: PARTIAL correctness only — termination is the per-run Verus `decreases`
  residual, deliberately NOT a Lean premise). GROUNDED in the SHIPPED Rust increment
  (i) (`thermite-tv/src/exec_stmt_encode.rs::loop_ref_obligations` + the three
  obligations `loop_{entry,preservation,exit}_obligation` in `obligation.rs`,
  commit `21b84c5f` — critic-clean): the Lean here is FAITHFUL to those exact shapes.

  ════════════════════════════════════════════════════════════════════════════════
  THE MODELING DECISION — a SEPARATE `WhileLoop` AROUND the SHIPPED `blockThread`,
  NOT a new `Stmt`/`Block` case. (The critic diffs this against the Rust.)
  ════════════════════════════════════════════════════════════════════════════════

  The Rust `loop_ref_obligations` treats a loop as a SEPARATE recognized form (the
  LAST statement before the tail, `recognize_v1_loop`), NOT a new `body_ref_state`
  statement arm: the loop body's SINGLE ITERATION is encoded by REUSING the SHIPPED
  straight-line `body_ref_state` (the loop body IS a straight-line `Block`), and the
  THREE loop obligations are built AROUND that reused step (entry / preservation /
  exit) — `body_ref_state` itself is UNCHANGED, with `Stmt::Loop` still `Unsupported`
  inside it. The FAITHFUL Lean mirror is therefore a SEPARATE `WhileLoop` + a
  `loopDenote` that ITERATES the SHIPPED `blockThread` (the Lean side of
  `body_ref_state`), with `Exec/Stmt.lean`'s `Stmt`/`Block`/`blockThread`/`State`
  UNCHANGED (NO new loop `Stmt` arm). Integrating `whileS` INTO the `Stmt` inductive
  would force reproving `body_ref_sound` over a changed type (a big perturbation the
  design does NOT require). The correspondence is EXACT:

    Rust (increment i)                        ↔  Lean (this module)
    ─────────────────────────────────────────────────────────────────────────────
    `recognize_v1_loop` (separate form)       ↔  `WhileLoop` (a separate structure)
    single iteration = `body_ref_state`        ↔  one step = `blockThread L.body`
    `loopDenote`-style genuine iteration       ↔  `loopDenote` (fuel-indexed iterate)
    ENTRY  `inv[cells:=entry]` ⊨ inv           ↔  `h_entry : I st₀`
    PRESERVATION `requires inv∧cond` one step   ↔  `h_pres` (one `blockThread` keeps I)
      `ensures inv[cells:=stepped]`               (under `I ∧ condHolds`)
    EXIT `requires inv ∧ !(cond)` ⊨ after       ↔  the rule's CONCLUSION
      after-loop = `inv ∧ ¬cond`                   `I stf ∧ condBool stf = some false`
    the `decreases` (Verus per-run residual)   ↔  the `h_run : loopDenote .. = some stf`
                                                  hypothesis (the loop EXITS — NOT proved)

  ════════════════════════════════════════════════════════════════════════════════
  PARTIAL CORRECTNESS — the termination residual (REQ-4, R-HONEST-3).
  ════════════════════════════════════════════════════════════════════════════════

  `while_rule` proves PARTIAL CORRECTNESS only: *after-loop holds IF the loop EXITS*
  (the `h_run : loopDenote cond body fuel st = some stf` hypothesis carries "exits at
  this fuel"). It deliberately does NOT prove termination. Termination is the Verus-
  checked `decreases` measure per run (`lower_loop` emits `decreases <dec>`, Verus
  discharges it; a non-decreasing measure → L0) — a NAMED per-run residual, exactly
  as Verus's own loop-termination check. `loopDenote`'s fuel-`0` `none` is NOT a
  fixpoint claim and NOT a vacuity dodge: `none` means "fuel exhausted, no result"
  (faithful to nontermination-as-obligation), and `while_rule` is ∀-fuel — it fires
  for EVERY fuel at which the loop actually exits. The fuel is the iteration COUNT, a
  genuine well-founded measure for the induction (NOT a cap on what the rule certifies).

  DEPENDENCIES: Lean 4 CORE ONLY. Reuses 2b's core-only `Stmt`/`Block`/`blockThread`/
  `State` (`Exec/Stmt.lean`) + 2a's `ExecExpr`/`execDenote`/`asBool` (`Exec`). The
  iteration is `Option`-monad composition; the rule is induction on `Nat` fuel
  discharged by `simp`/`omega`/`decide` — NO Mathlib, NO Lean-SMT, NO `sorry`/`admit`/
  `native_decide`. Mirrors the spine's core-only discipline.
-/
import Thermite.Exec.Stmt

namespace Thermite.Exec

/-! ## The `while` form (a SEPARATE recognized form — NOT a `Stmt` arm)

  A `WhileLoop` is the v1 frozen-subset loop: a condition `ExecExpr` (denoted by 2a's
  `execDenote`/`asBool`), a STRAIGHT-LINE `Block` body (the SHIPPED `blockThread`
  steps it once), and an invariant `I : State → Prop`.

  **The invariant modeling decision (JUSTIFIED).** The design (REQ-3) leaves the
  invariant modeling open ("an ExecExpr-denoted invariant OR a `State → Prop`
  predicate — decide the cleanest faithful modeling and justify"). We model `I` as a
  `State → Prop` predicate over the cells, NOT an `ExecExpr`, because:
    (1) The Rust obligations treat the invariant as a Verus PREDICATE over the cells
        (`encode_inv_clauses` emits a bool-valued Verus expression that the entry /
        preservation `requires`/`ensures` and the exit `requires` use as a PROP — Z3
        reasons about it as a logical predicate, not as a bounded exec VALUE). The
        WHILE-RULE's content is the LOGICAL relation entry→preservation→exit; a
        `State → Prop` is the faithful denotation of that Verus predicate (the same
        choice `S_C` makes for contract clauses — a `Prop`, not an exec value).
    (2) It keeps the rule GENERAL: `while_rule` holds for ANY invariant predicate
        (the `forall_*`-shaped corpus invariants included), exactly as the per-run
        Z3 check discharges whatever predicate the production emitted. An `ExecExpr`
        invariant would needlessly restrict to bool-decidable bounded forms.
    (3) The non-vacuity witnesses (`b_loop_iterates`, the negatives) instantiate `I`
        to a CONCRETE predicate (`lo ≤ n`) over the state's cells via `execDenote` /
        the `BVal.value`, so the modeling is not vacuous — a real invariant bites. -/
structure WhileLoop where
  /-- The loop CONDITION (`while <cond>`) — a 2a `ExecExpr`, denoted via `execDenote`
      + `asBool` (the exit is at the head: `¬cond`). -/
  cond : ExecExpr
  /-- The STRAIGHT-LINE loop BODY (`LoopNode.body`) — the SHIPPED `blockThread` steps
      it once per iteration (the loop body IS a straight-line `Block`, AC-5 reuse). -/
  body : Block
  /-- The loop INVARIANT as a predicate over the cells (the Verus `inv` denotation —
      see the modeling decision above). -/
  inv : State → Prop

/-- The loop condition's BOUNDED BOOL value at a state (`asBool ∘ execDenote cond` over
    the state's valuation): `some true` (continue), `some false` (exit), or `none` (a
    non-bool condition — an exec type error, the `asBool` partiality). This is EXACTLY
    the value `loopDenote`'s head test reads (`do let c ← asBool (← execDenote cond
    st.env)`), factored out so the rule's premises/conclusion name it directly. -/
def condBool (cond : ExecExpr) (st : State) : Option Bool :=
  asBool =<< execDenote cond st.env

/-- Project an `ExecVal` to its underlying mathematical (bounded) integer value (a bool
    projects to `0` — the invariant predicates here range over the SCALAR cells, which
    are integers; a bool cell never enters these `≤`/`<` facts). Used to state the
    concrete `l1Inv` (`lo ≤ n`) over the state's bounded cell values. -/
def execIntValue : ExecVal → Int
  | .int b => b.value
  | .bool _ => 0

/-! ## `loopDenote` — the GENUINE iteration semantics (fuel-indexed iteration of the
    SHIPPED `blockThread`)

  The big-step meaning of the loop: iterate the SHIPPED single-step body transformer
  `blockThread body` until the condition is false, fuel-bounded on the iteration
  COUNT (the `Denote.lean` #181 fuel pattern — NOT a vacuity dodge; `while_rule` is
  ∀-fuel). `none` at fuel `0` = fuel exhausted (no result — NOT a fixpoint claim);
  `none` from `blockThread` = the body's obligation failed (overflow / out-of-range,
  PROPAGATED faithfully); `none` from `condBool` = a non-bool condition. FAITHFUL to
  the design's REQ-3 `loopDenote` (the exact pseudo-Lean there). -/
def loopDenote (cond : ExecExpr) (body : Block) : Nat → State → Option State
  | 0,        _  => none                       -- fuel exhausted: NO result (not a fixpoint)
  | fuel + 1, st => do
      let c ← condBool cond st                  -- the head test (the SHIPPED `asBool ∘ execDenote`)
      if c then
        let st' ← blockThread body st           -- ONE iteration = the SHIPPED transformer
        loopDenote cond body fuel st'            -- consume one fuel, iterate
      else
        some st                                  -- exit: the after-loop state (`¬cond`)

/-! ## (T2-loop) — the PARTIAL-CORRECTNESS WHILE-RULE (the universal piece)

  The universally-proven Lean rule the per-run Z3 obligations compose with. The
  premises MATCH the Rust obligations' semantics (the critic diffs this):

    `h_entry`  ↔ ENTRY  (REQ-2.1): the invariant holds at the loop-entry state.
    `h_pres`   ↔ PRESERVATION (REQ-2.2): assuming `I ∧ cond` at the loop head, ONE
                 iteration of the SHIPPED `blockThread body` re-establishes `I` (the
                 havoc-cells + frame shape: Verus havocs the mutated cells, assumes
                 `I ∧ cond`, runs the body, must re-prove `I` — here the body step is
                 `blockThread body` over an ARBITRARY state satisfying `I ∧ cond`).
    conclusion ↔ EXIT (REQ-2.3): the after-loop state satisfies `I ∧ ¬cond`.

  PARTIAL correctness: the `h_run : loopDenote .. fuel st = some stf` hypothesis is
  the "loop EXITS at this fuel" premise (the Verus `decreases` residual, REQ-4).

  Proof: induction on `fuel`. `fuel = 0`: `loopDenote .. 0 st = none ≠ some stf`,
  vacuous. `fuel + 1`: case on `condBool cond st`:
    - `some false` (exit): `loopDenote` returns `some st`, so `stf = st`; the
      conclusion is `h_entry` + the false-condition fact directly.
    - `some true` (continue): the body steps to some `st'` (else `loopDenote = none ≠
      some stf`), `h_pres` re-establishes `I st'`, and the IH on `fuel` (with entry
      `I st'`) gives `I stf ∧ ¬cond stf`.
    - `none`: `loopDenote = none ≠ some stf`, vacuous. -/
theorem while_rule
    (cond : ExecExpr) (body : Block) (I : State → Prop)
    (h_pres : ∀ st, I st → condBool cond st = some true →
                ∀ st', blockThread body st = some st' → I st')
    (fuel : Nat) :
    ∀ st stf, I st → loopDenote cond body fuel st = some stf →
      I stf ∧ condBool cond stf = some false := by
  induction fuel with
  | zero =>
      intro st stf _ h_run
      -- fuel `0`: `loopDenote .. 0 st = none`, contradicting `= some stf`.
      simp only [loopDenote] at h_run
      exact absurd h_run (by simp)
  | succ fuel ih =>
      intro st stf h_entry h_run
      simp only [loopDenote, bind, Option.bind] at h_run
      -- Case on the head condition test `condBool cond st`.
      cases hc : condBool cond st with
      | none =>
          -- A non-bool condition: `loopDenote = none`, contradicting `= some stf`.
          rw [hc] at h_run
          exact absurd h_run (by simp)
      | some c =>
          rw [hc] at h_run
          cases c with
          | false =>
              -- EXIT branch: `loopDenote` returns `some st`, so `stf = st`. The
              -- conclusion is `I st` (= `h_entry`) ∧ the false-condition fact.
              simp only [Bool.false_eq_true, if_false, Option.some.injEq] at h_run
              subst h_run
              exact ⟨h_entry, hc⟩
          | true =>
              -- CONTINUE branch: the body steps to some `st'` (else `loopDenote =
              -- none`), `h_pres` re-establishes `I st'`, IH on `fuel` finishes.
              simp only [if_true] at h_run
              cases hb : blockThread body st with
              | none =>
                  rw [hb] at h_run
                  exact absurd h_run (by simp)
              | some st' =>
                  rw [hb] at h_run
                  simp only at h_run
                  have hI' : I st' := h_pres st h_entry hc st' hb
                  exact ih st' stf hI' h_run

/-! ## `tv_meta_loop` — the loop translation-validation meta-theorem (the capstone
    composition, sibling to `Faithfulness.lean`'s `tv_meta_body`)

  The capstone-pattern composition for the loop: the THREE per-run Z3 premises (the
  REQ-2 obligations, Z3-discharged) + `while_rule` ⟹ the after-loop state the TV
  certifies IS the true one. This is the loop sibling of `tv_meta_body`; the actual
  bundling into `FnTvWitness` lives in `Faithfulness.lean` (which imports this), but
  the COMPOSITION lemma is stated here against the genuine `loopDenote`. -/

/--
  **(T2-loop) — the loop TV meta-theorem.** For a v1 `WhileLoop` `L`, if the per-run
  Z3 obligations attested ENTRY (`h_entry : L.inv st`) and PRESERVATION (`h_pres`: one
  SHIPPED `blockThread L.body` step keeps `L.inv` under `L.inv ∧ cond`), then for EVERY
  fuel at which the loop EXITS (`h_run`, the Verus `decreases` residual), the TV-
  certified after-loop characterization `L.inv stf ∧ ¬cond stf` (the EXIT obligation's
  claim) is the TRUE meaning under the genuine iteration semantics `loopDenote`.

  This is `while_rule` packaged on the `WhileLoop` surface — the loop analogue of
  `tv_meta_body`'s `h_tv.trans (body_ref_sound ..)`: the three Z3-discharged premises
  compose with the kernel-proven `while_rule` to yield the universal after-state
  guarantee. The `∀ fuel/st/stf` is REAL; the premises are the genuine Z3 obligations
  (a wrong-invariant / wrong-after-state production does NOT satisfy them — see the
  negatives). RELATIVE to {Z3 (the three obligations), S = the intended meaning, the
  Lean kernel}; termination is the per-run Verus residual (PARTIAL correctness). -/
theorem tv_meta_loop
    (L : WhileLoop)
    (h_pres : ∀ st, L.inv st → condBool L.cond st = some true →
                ∀ st', blockThread L.body st = some st' → L.inv st')
    (fuel : Nat) (st stf : State)
    (h_entry : L.inv st)
    (h_run : loopDenote L.cond L.body fuel st = some stf) :
    L.inv stf ∧ condBool L.cond stf = some false :=
  while_rule L.cond L.body L.inv h_pres fuel st stf h_entry h_run

/-! ## NON-VACUITY — the L1 witness: a concrete faithful loop GENUINELY iterates

  Mirrors the Rust L1 fixture (`thermite-tv/tests/loop_teeth.rs`):

      let mut lo: usize = 0;
      while lo < n  inv lo <= n  dec n - lo  { lo = lo + 1; }
      lo

  Here `n := 3`. The loop GENUINELY iterates (fuel consumed, the real per-iteration
  `lo ↦ lo+1` step run by the SHIPPED `blockThread`) to the real exit state `lo = 3`,
  and `while_rule` certifies `inv (lo ≤ 3) ∧ ¬(lo < 3)` — hence `lo = 3` — exactly the
  L1 after-loop fact `lo == n`. This proves `loopDenote` is REAL (not vacuous `none`)
  and `while_rule` FIRES on a genuine loop (not vacuously unusable). -/

/-- The L1 loop body: the single straight-line statement `lo = lo + 1` (the cell `lo`
    rebound — the SHIPPED `assign` form, AC-5 reuse), no tail (the iteration steps the
    state; the loop's tail value is the after-loop `lo`). -/
def l1Body : Block :=
  .mk [ .assign "lo" (.arith .add (.var "lo") (.intLit .usize 1)) ] none

/-- The L1 condition `lo < n` (`while lo < n`). -/
def l1Cond : ExecExpr := .cmp .lt (.var "lo") (.var "n")

/-- The L1 invariant `lo ≤ n` as a `State → Prop` over the cell `lo`'s and input `n`'s
    bounded values (faithful to the Verus `inv lo <= n` predicate). It ALSO carries the
    `usize` TYPE-RANGE facts for the cells (`0 ≤ lo, n < 2^64`) — these are NOT an extra
    assumption: in Verus the cells are typed `usize`, so their in-range-ness is an
    IMPLICIT part of every loop invariant (the type system guarantees it). Modeling them
    explicitly here is faithful (the Lean `State.env.vars` is an untyped total map, so the
    type-range fact the Verus obligation gets for free must be stated). -/
def l1Inv (st : State) : Prop :=
  execIntValue (st.env.vars "lo") ≤ execIntValue (st.env.vars "n")
  ∧ 0 ≤ execIntValue (st.env.vars "lo") ∧ execIntValue (st.env.vars "lo") < (2 : Int) ^ 64
  ∧ 0 ≤ execIntValue (st.env.vars "n") ∧ execIntValue (st.env.vars "n") < (2 : Int) ^ 64

/-- The L1 `WhileLoop` (the faithful v1 fixture). -/
def l1Loop : WhileLoop := { cond := l1Cond, body := l1Body, inv := l1Inv }

/-- A starting state with `lo := 0`, `n := 3` (both `usize`), `lo` in scope (a
    `let mut lo` cell). The other vars default to `usize 0`. -/
def l1State : State :=
  { env := { vars := fun s => if s = "n" then .int ⟨.usize, 3⟩ else .int ⟨.usize, 0⟩
             slices := fun _ => [] }
    scope := fun s => s = "lo" }

/-- **L1 (non-vacuity) — the loop GENUINELY iterates to the real exit state.** From
    `lo = 0`, `n = 3`, `loopDenote` runs THREE real iterations (`lo ↦ 1 ↦ 2 ↦ 3`, each
    the SHIPPED `blockThread` step) and EXITS at `lo = 3` (where `¬(3 < 3)`). At fuel
    `4` (3 iterations + the exit test) the result is `some` a state with `lo = 3`. This
    is the REAL iteration (fuel consumed, the genuine per-iteration mutation) — NOT a
    vacuous `none`. -/
theorem b_loop_iterates :
    (loopDenote l1Cond l1Body 4 l1State).map (fun st => (st.env.vars "lo")) = some (.int ⟨.usize, 3⟩) := by
  simp only [loopDenote, l1Cond, l1Body, l1State, condBool, execDenote,
        blockThread, stmtDenote, asInt, cmpVal, evalArith, rawArith,
        State.setVar, IntTy.bound, IntTy.width, bind, Option.bind, Option.map]
  decide

/-- `0 < 2^64` (the `usize` bound is positive) — core Lean (`Int.pow_pos`). Used to
    discharge the `usize` type-range facts in `l1Inv` for the concrete L1 states. -/
theorem two_pow_64_pos : (0 : Int) < (2 : Int) ^ 64 := Int.pow_pos (by decide)

/-- The concrete value of the `usize` bound `2^64`. Lets `omega` discharge the small
    in-range facts (`2 < 2^64`, `4 < 2^64`, …) for the concrete L1/L2 fixture states
    without re-evaluating the power each time. -/
theorem two_pow_64_val : (2 : Int) ^ 64 = 18446744073709551616 := by decide

/-- The L1 invariant HOLDS on entry (`lo = 0 ≤ 3 = n`) — the ENTRY obligation
    (REQ-2.1), `h_entry` for the rule. -/
theorem l1_entry_holds : l1Inv l1State := by
  have hp := two_pow_64_val
  have hlo : execIntValue (l1State.env.vars "lo") = 0 := by simp only [l1State, execIntValue]; rfl
  have hn : execIntValue (l1State.env.vars "n") = 3 := by simp only [l1State, execIntValue]; rfl
  simp only [l1Inv, hlo, hn]
  refine ⟨by omega, by omega, by omega, by omega, by omega⟩

/-- The bounded value of the body step `lo + 1` when `lo` holds `.int blo`: it is
    `.int ⟨blo.ty, blo.value + 1⟩`, and the step SUCCEEDING (`= some v`) means the
    `lo + 1` add did NOT overflow (`blo.value + 1 < 2^blo.ty.width`) and stays
    non-negative. Used by `l1_preservation` to decode the SHIPPED `blockThread` step
    over an ARBITRARY state (the cell's bounded type is whatever it is — the step's
    success is the in-range witness). -/
theorem l1_step_value (env : ExecEnv) (blo : BVal) (hlo : env.vars "lo" = .int blo)
    (v : ExecVal)
    (hv : execDenote (.arith .add (.var "lo") (.intLit .usize 1)) env = some v) :
    v = .int ⟨blo.ty, blo.value + 1⟩
    ∧ blo.value + 1 < (2 : Int) ^ blo.ty.width ∧ 0 ≤ blo.value + 1 := by
  simp only [execDenote, hlo, asInt, evalArith, rawArith, IntTy.bound,
        Option.bind_eq_bind, Option.bind_some] at hv
  rw [if_pos (show (0:Int) ≤ 1 ∧ (1:Int) < 2 ^ (IntTy.usize).width from
        ⟨by omega, by simp only [IntTy.width]; omega⟩)] at hv
  dsimp only [Option.bind] at hv
  by_cases hcond : 0 ≤ blo.value + (1:Int) ∧ blo.value + (1:Int) < 2 ^ blo.ty.width
  · rw [if_pos hcond] at hv
    simp only [Option.some.injEq] at hv; subst hv
    exact ⟨rfl, hcond.2, hcond.1⟩
  · rw [if_neg hcond] at hv; simp at hv

/-- The L1 invariant is PRESERVED by one iteration (`lo ≤ n ∧ lo < n ⟹ lo+1 ≤ n`) — the
    PRESERVATION obligation (REQ-2.2), `h_pres` for the rule. This is the GENUINE
    per-iteration step (the SHIPPED `blockThread` of `lo = lo + 1`); it bites only when
    `lo < n` (the loop-head guard). The `usize` step is total here because `lo < n`
    keeps `lo + 1 ≤ n < 2^64` (no overflow). -/
theorem l1_preservation :
    ∀ st, l1Inv st → condBool l1Cond st = some true →
      ∀ st', blockThread l1Body st = some st' → l1Inv st' := by
  intro st hI hc st' hstep
  -- The loop-head guard `lo < n` decoded to a bounded `<` (the cmp `lt` is decided true).
  have hlt : execIntValue (st.env.vars "lo") < execIntValue (st.env.vars "n") := by
    -- `hc` reduces to a `decide (lo.value < n.value) = true` after projecting the cells.
    rw [condBool, l1Cond] at hc
    cases hlo : st.env.vars "lo" with
    | bool b =>
        simp only [execDenote, hlo, asInt, bind, Option.bind] at hc
        exact absurd hc (by decide)
    | int blo =>
        cases hn : st.env.vars "n" with
        | bool b =>
            simp only [execDenote, hlo, hn, asInt, bind, Option.bind] at hc
            exact absurd hc (by decide)
        | int bn =>
            simp only [execDenote, hlo, hn, asInt, bind, Option.bind, cmpVal] at hc
            simp only [execIntValue]
            exact decide_eq_true_eq.mp (Option.some.injEq .. ▸ hc)
  -- The single `assign "lo" (lo+1)` step: `blockThread l1Body st` is exactly
  -- `stmtDenote (assign "lo" (lo+1)) st` (the trailing `blockThread (.mk [] none)` is the
  -- identity `some`). Rewrite hstep to that single step, then decode.
  have hbt : blockThread l1Body st = stmtDenote (.assign "lo" (.arith .add (.var "lo")
      (.intLit .usize 1))) st := by
    simp only [l1Body, blockThread]
    cases stmtDenote (.assign "lo" (.arith .add (.var "lo") (.intLit .usize 1))) st <;>
      simp only [bind, Option.bind]
  rw [hbt] at hstep
  -- Extract the inv facts (the `lo ≤ n` AND the `usize` type-range bounds).
  obtain ⟨hle, hlo_lo, hlo_hi, hn_lo, hn_hi⟩ := hI
  simp only [l1Inv, execIntValue] at hle hlo_lo hlo_hi hn_lo hn_hi ⊢
  cases hlo : st.env.vars "lo" with
  | bool b =>
      -- A bool `lo` can't step (`asInt (.bool _) = none` in the rhs `lo + 1`): hstep is false.
      simp only [stmtDenote, execDenote, hlo, asInt, bind, Option.bind] at hstep
      split at hstep <;> nomatch hstep
  | int blo =>
      cases hn : st.env.vars "n" with
      | bool b => simp only [execIntValue, hlo, hn] at hlt hlo_lo; omega
      | int bn =>
          simp only [execIntValue, hlo, hn] at hlt hle hlo_lo hlo_hi hn_lo hn_hi ⊢
          -- The assign step: `stmtDenote (.assign "lo" (lo+1)) st = some st'`. So `lo` is in
          -- scope (the `if scope` is taken) and the rhs `lo + 1` evaluates to some `v`;
          -- `l1_step_value` then gives `v = ⟨blo.ty, blo.value + 1⟩` (+ in-range).
          unfold stmtDenote at hstep
          split at hstep
          · -- `lo` in scope: hstep = `(execDenote (lo+1) st.env) >>= fun v => some (setVar v)`.
            rw [Option.bind_eq_bind, Option.bind_eq_some_iff] at hstep
            obtain ⟨v, hv, hsetv⟩ := hstep
            obtain ⟨hvval, _, _⟩ := l1_step_value st.env blo hlo v hv
            subst hvval
            simp only [Option.some.injEq] at hsetv
            subst hsetv
            -- `st' = st.setVar "lo" ⟨blo.ty, blo.value + 1⟩`; decode its `lo`/`n` cells
            -- (the `lo` lookup takes the new value; the `n` lookup is unchanged).
            have hlo' : (State.setVar st "lo" (.int ⟨blo.ty, blo.value + 1⟩)).env.vars "lo"
                = .int ⟨blo.ty, blo.value + 1⟩ := by
              simp only [State.setVar, if_pos]
            have hn' : (State.setVar st "lo" (.int ⟨blo.ty, blo.value + 1⟩)).env.vars "n"
                = .int bn := by
              simp only [State.setVar, if_neg (by decide : ¬ ("n" = "lo"))]; exact hn
            rw [hlo', hn']
            show blo.value + 1 ≤ bn.value ∧ 0 ≤ blo.value + 1 ∧ blo.value + 1 < (2:Int)^64
              ∧ 0 ≤ bn.value ∧ bn.value < (2:Int)^64
            refine ⟨by omega, by omega, by omega, by omega, by omega⟩
          · -- `lo` NOT in scope: hstep : `none = some st'`, impossible.
            simp at hstep

/-- **The L1 loop is CERTIFIED by `while_rule`.** Composing the entry obligation
    (`l1_entry_holds`), the preservation obligation (`l1_preservation`), and the
    genuine exit (`b_loop_iterates` shows the loop exits at fuel `4` with `some` a
    state `stf`), `while_rule` certifies the after-loop characterization
    `l1Inv stf ∧ ¬(lo < n) stf`. From `lo ≤ 3 ∧ ¬(lo < 3)` the L1 after-loop fact
    `lo = 3` (= `lo == n`) FOLLOWS — exactly the Rust L1 exit obligation's claim
    (`lo == n`). The rule FIRES on a genuine, terminating loop (NOT vacuous). -/
theorem l1_while_rule_certifies_exit :
    ∃ stf, loopDenote l1Cond l1Body 4 l1State = some stf
      ∧ l1Inv stf ∧ condBool l1Cond stf = some false := by
  -- `b_loop_iterates` shows the loop EXITS at fuel 4 (the `.map` of the result is `some`,
  -- so the loop denotation is `some stf` for the real exit state `stf`). Extract that.
  have hmap := b_loop_iterates
  rw [Option.map_eq_some_iff] at hmap
  obtain ⟨stf, hrun, _⟩ := hmap
  -- `while_rule` then certifies the after-loop characterization at `stf`.
  exact ⟨stf, hrun, while_rule l1Cond l1Body l1Inv l1_preservation 4 l1State stf l1_entry_holds hrun⟩

/-! ## NEGATIVE LEMMAS — the WHILE-RULE's premises are LOAD-BEARING (the teeth, the
    L2/L3 mirrors)

  Two faithfulness bugs the loop TV MUST NOT commit, each PROVEN to break the rule —
  pinning that `while_rule` genuinely CONSUMES `h_pres` (L2) and that the after-loop
  conclusion is exactly `inv ∧ ¬cond`, NOT a stronger over-claim (L3). -/

/-! ### L2 — a NON-PRESERVED invariant (the `lo + 2` shape): `h_pres` is load-bearing.

  Mirrors the Rust L2 (`loop_teeth.rs::l2_broken_preservation_caught`): a body that
  steps `lo` by 2 (the production infidelity for source `lo + 1`) does NOT preserve
  `inv lo ≤ n` — one iteration from a state with `lo = n - 1 < n` (so the loop-head
  guard holds, AND `inv` holds) lands at `lo = n + 1 > n`, BREAKING `inv`. We exhibit
  that concrete witness: the preservation premise `h_pres` is FALSE for this body, so
  the rule cannot be (mis)applied to certify it — `h_pres` genuinely gates. -/

/-- The L2 BUGGY body: `lo = lo + 2` (the production infidelity; source is `lo + 1`). -/
def l2Body : Block :=
  .mk [ .assign "lo" (.arith .add (.var "lo") (.intLit .usize 2)) ] none

/-- A witness state for L2: `lo = 2`, `n = 3` (so `inv lo ≤ n` HOLDS — `2 ≤ 3` — AND
    the loop-head guard `lo < n` HOLDS — `2 < 3`). One `lo + 2` step lands at `lo = 4`,
    where `inv` is FALSE (`4 > 3`). -/
def l2State : State :=
  { env := { vars := fun s => if s = "n" then .int ⟨.usize, 3⟩
                              else if s = "lo" then .int ⟨.usize, 2⟩
                              else .int ⟨.usize, 0⟩
             slices := fun _ => [] }
    scope := fun s => s = "lo" }

/-- **L2 — the `lo + 2` body does NOT preserve the invariant (the premise BITES).** At
    `l2State` (`lo = 2`, `n = 3`): `l1Inv` HOLDS (`2 ≤ 3`) and the loop-head guard
    `condBool l1Cond` is `some true` (`2 < 3`), so the PRESERVATION premise's antecedent
    is satisfied — yet the buggy `lo + 2` step lands at `lo = 4` where `l1Inv` is FALSE
    (`4 > 3`). So the preservation premise (instantiated for `l2Body`) is FALSE: there
    is NO valid `h_pres` for this buggy body. This PROVES `while_rule` genuinely
    CONSUMES `h_pres` — a vacuous rule (one that dropped `h_pres`) would WRONGLY certify
    this loop's after-state. The loop analogue of `mutation_not_applied_breaks_soundness`. -/
theorem l2_non_preserved_invariant_admits_bad_step :
    l1Inv l2State
    ∧ condBool l1Cond l2State = some true
    ∧ (∃ st', blockThread l2Body l2State = some st' ∧ ¬ l1Inv st') := by
  refine ⟨?_, ?_, ?_⟩
  · -- `l1Inv l2State` : `2 ≤ 3` (+ the `usize` range facts for `lo := 2`, `n := 3`).
    have hp := two_pow_64_val
    simp only [l1Inv, l2State, execIntValue, if_neg (by decide : ¬ ("lo" = "n")), reduceIte]
    refine ⟨by omega, by omega, by omega, by omega, by omega⟩
  · -- the loop-head guard `2 < 3` holds.
    simp only [condBool, l1Cond, l2State, execDenote, asInt, cmpVal, bind, Option.bind]
    decide
  · -- the `lo + 2` step lands at `lo = 4`, breaking `inv` (`4 > 3`). The step's `lo` value
    -- is decidably `.int ⟨usize, 4⟩` (the genuine `lo + 2` mutation, l2State `lo := 2`).
    have hmap : (blockThread l2Body l2State).map (fun s => s.env.vars "lo")
        = some (.int ⟨.usize, 4⟩) := by
      simp only [l2Body, l2State, blockThread, stmtDenote, execDenote, asInt, evalArith,
            rawArith, IntTy.bound, IntTy.width, State.setVar, bind, Option.bind, Option.map]
      decide
    rw [Option.map_eq_some_iff] at hmap
    obtain ⟨st', hb, hlo4⟩ := hmap
    refine ⟨st', hb, ?_⟩
    -- at `lo = 4`, `n = 3` (n unchanged): `l1Inv st'` (`4 ≤ n`) — but n = 3, so FALSE.
    intro hbad
    obtain ⟨hle, _⟩ := hbad
    -- `st'`'s `lo` is `.int ⟨usize, 4⟩` (hlo4); its `n` is unchanged from l2State (`3`).
    have hn4 : st'.env.vars "n" = .int ⟨.usize, 3⟩ := by
      have hmapn : (blockThread l2Body l2State).map (fun s => s.env.vars "n")
          = some (.int ⟨.usize, 3⟩) := by
        simp only [l2Body, l2State, blockThread, stmtDenote, execDenote, asInt, evalArith,
              rawArith, IntTy.bound, IntTy.width, State.setVar, bind, Option.bind, Option.map]
        decide
      rw [Option.map_eq_some_iff] at hmapn
      obtain ⟨st'', hb'', hn3⟩ := hmapn
      rw [hb] at hb''
      have : st' = st'' := (Option.some.injEq _ _).mp hb''
      rw [this]; exact hn3
    simp only [execIntValue, hlo4, hn4] at hle
    omega

/-- **L2 contrast — `while_rule` REFUSES the buggy loop (the teeth bite the bug).** If
    one could supply a preservation premise `h_pres` for the buggy `l2Body`, the rule
    would certify its after-state; but L2 above shows no such `h_pres` exists (the
    premise is FALSE at `l2State`). Concretely: a (hypothetical) `h_pres` for `l2Body`
    applied at `l2State` would force `l1Inv` of the `lo = 4` step state, which is FALSE.
    So `while_rule` cannot be (soundly) instantiated for the buggy body — the premise
    is genuinely load-bearing. -/
theorem l2_no_preservation_premise_for_buggy_body :
    ¬ (∀ st, l1Inv st → condBool l1Cond st = some true →
         ∀ st', blockThread l2Body st = some st' → l1Inv st') := by
  intro h_pres
  obtain ⟨hI, hc, st', hstep, hbad⟩ := l2_non_preserved_invariant_admits_bad_step
  exact hbad (h_pres l2State hI hc st' hstep)

/-! ### L3 — the after-loop OVER-CLAIM (the `lo > n` shape): the conclusion is exactly
    `inv ∧ ¬cond`, no stronger.

  Mirrors the Rust L3 (`loop_teeth.rs::l3_wrong_exit_characterization_caught`): the
  after-loop characterization is `inv ∧ ¬cond` = `lo ≤ n ∧ ¬(lo < n)`, from which only
  `lo = n` follows. A production that OVER-CLAIMS `lo > n` is WRONG: the countermodel is
  the genuine exit state `lo = n` (`lo = 3 = n`), where `inv ∧ ¬cond` holds but `lo > n`
  is FALSE. We exhibit that countermodel from the real `while_rule` conclusion. -/

/-- **L3 — `inv ∧ ¬cond` does NOT imply the over-claim `lo > n` (the exit countermodel).**
    The genuine `while_rule` conclusion at the L1 loop's exit is `l1Inv stf ∧ ¬cond stf`
    = `lo ≤ n ∧ ¬(lo < n)`, satisfied by the REAL exit state `lo = 3 = n`. The
    over-claim `lo > n` is FALSE there (`3 > 3` is false). So a production claiming the
    STRONGER `lo > n` would be refuted at the genuine `loopDenote` exit — the conclusion
    is EXACTLY `inv ∧ ¬cond`, never the over-claim. The loop analogue of the wrong-cell /
    swapped-branch teeth. (`exit_lo_gt_n` is the over-claim predicate; the witness is
    `l1_while_rule_certifies_exit`'s `stf`.) -/
theorem l3_exit_overclaim_refuted :
    ∃ stf, loopDenote l1Cond l1Body 4 l1State = some stf
      ∧ l1Inv stf ∧ condBool l1Cond stf = some false
      ∧ ¬ (execIntValue (stf.env.vars "lo") > execIntValue (stf.env.vars "n")) := by
  obtain ⟨stf, hrun, hI, hcond⟩ := l1_while_rule_certifies_exit
  refine ⟨stf, hrun, hI, hcond, ?_⟩
  -- The countermodel is GENERIC (`lo = n`, NOT the specific `lo = 3`): from the genuine
  -- `while_rule` conclusion `inv ∧ ¬cond` = `lo ≤ n ∧ ¬(lo < n)` we get `lo ≥ n` AND
  -- `lo ≤ n`, hence `lo = n` — so the over-claim `lo > n` is FALSE. This is precisely
  -- the Rust L3 fact (`lo == n` follows from `inv ∧ ¬cond`, but the over-claim `lo > n`
  -- does NOT). No explicit exit state needed.
  intro hgt
  -- `hI` gives `lo ≤ n`; the over-claim `hgt : lo > n` directly contradicts it. (The
  -- exit also gives `¬(lo < n)` hence `lo = n` — the genuine `lo == n` countermodel —
  -- but `lo ≤ n` ∧ `lo > n` already refutes the over-claim.)
  obtain ⟨hle, _⟩ := hI
  omega

end Thermite.Exec
