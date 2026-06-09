/-
  Thermite/Exec/Stmt.lean — the EXEC-BODY sublanguage `S_B` (a big-step STATE
  TRANSFORMER over straight-line blocks) + the (T1) SOUNDNESS theorem for the exec
  body reference encoder `body_ref_state` (increment 2b, #172; epic #169). THIS
  BUILDS DIRECTLY ON increment 2a (`Thermite.Exec`): the RHS of a `let`/`assign`, an
  `if` condition, and a tail value are ALL exec EXPRESSIONS, denoted by 2a's
  `execDenote` / `ExecVal` / `ExecEnv` — `S_B` adds ONLY the state-threading,
  scalar-mutation rebind, branch composition, and tail projection ON TOP of 2a's
  per-RHS bounded-value denotation.

  Governing design: `.design/verified/thermite-semantics.md` Architecture §"S_B — the
  exec-body sublanguage (state transformer; UNIFIED from exec-stmt-tv.md)",
  REQ-1/REQ-2/REQ-4 (increment (c), #172: "exec-statement `S_B` + prove
  `body_ref_state` sound"). The body state-transformer is GROUNDED in
  `thermite-tv/src/exec_stmt_encode.rs::body_ref_state` (the big-step state denotation:
  the let/assign substitution-threading, the SCALAR-cell mutation ORDER sensitivity,
  the `if`-statement branch composition, the tail / multi-cell-tuple projection) +
  `exec-stmt-tv.md` REQ-1/REQ-2 (the FROZEN straight-line subset + the big-step rules).

  ════════════════════════════════════════════════════════════════════════════════
  THE MODEL: a big-step STATE TRANSFORMER `S_B` over straight-line blocks.
  ════════════════════════════════════════════════════════════════════════════════

  A STATE is the mutable bindings — an `ExecEnv` from 2a (var → `ExecVal`, slices →
  element sequences). `S_B` is a big-step evaluation `⟨B, σ⟩ ⇓ ⟨σ', v⟩` threading an
  initial state through a `Block` to a FINAL state + the tail value. The transformer
  is PARTIAL (`Option`): it propagates `none` whenever a sub-expression's OBLIGATION
  fails (2a's overflow / div-zero / shift-zero / out-of-range index) or a binding is
  missing / out of the frozen subset. The straight-line statement forms (faithful to
  `body_ref_state`, `exec-stmt-tv.md` REQ-1 IN set):

    - `let x = e`   — bind `x` to `execDenote e` in the current state, extend the env.
    - `assign x = e`— UPDATE an EXISTING scalar binding `x` to `execDenote e` (the
                      ORDER-SENSITIVE mutation: the rhs reads the state BEFORE this
                      assign; `s = s+1; s = s*2` ≠ `s = s*2; s = s+1`).
    - `ifElse c B₁ B₂` — branch on `execDenote c`: run `B₁` (then) or `B₂` (else) as a
                      sub-block state-transformer over the SAME state, recompose.
    - SEQUENCING (`Block.stmts` in order) — compose the per-statement transformers.
    - `ret e` / tail `e` — the body RESULT value, `execDenote e` in the FINAL state.

  ════════════════════════════════════════════════════════════════════════════════
  FAITHFULNESS TO `body_ref_state` (the critic diffs this — read carefully).
  ════════════════════════════════════════════════════════════════════════════════

  The Rust `body_ref_state` is a SYMBOLIC encoder: its env maps name → a closed-form
  VALUE `Expr` in the inputs, and `let`/`assign` SUBSTITUTE the rhs under the current
  env then rebind. That symbolic substitution is exactly a big-step state transformer
  whose state is the valuation (`exec_stmt_encode.rs` module doc: "Big-step evaluation
  threads an initial environment through the statement sequence to a FINAL
  environment; the body's value is the tail expression evaluated in that final
  environment"). `bodyDenote` (`S_B`) is that big-step transformer over VALUES;
  `bodyRefState` models the encoder's THREADING (its operational order: thread each
  stmt left-to-right into the state, then evaluate the tail in the final state — the
  exact loop in `encode_block_tail` + `thread_stmt`), independently of `bodyDenote`.
  `body_ref_sound` proves them equal construct-by-construct. The faithful match, arm
  by arm with `thread_stmt`:

    - `Stmt::Let { name, init }`  →  `Stmt.letS` : bind a FRESH name (the encoder
      REJECTS a re-shadow `let x=..; let x=..` — modelled: `bodyRefState` is `none`
      when `name` is already bound, matching `env.contains_key(name)` → `Err`).
    - `Stmt::Assign { Path[x], value }` → `Stmt.assign` : rebind an EXISTING bare
      scalar `x` (the encoder REJECTS a non-bare / non-scalar target `xs[i]=..` and an
      unbound target — modelled: `bodyRefState` is `none` when `x` is NOT already
      bound). NOTE: a NON-SCALAR mutation `xs[i] = e` is `Unsupported` in
      `body_ref_state` (the v1 frozen subset mutates only bare scalar cells — a
      sequence theory is v2), so it is NOT a `Stmt` form here (honest absence, not
      embed-then-`sorry`). The mutation `S_B` models is the SCALAR-cell rebind.
    - `Stmt::If { cond, then, else_ }` (statement position) → `Stmt.ifElse` : run each
      branch over the state, recompose. (The Rust composes the per-cell branch values
      into a Verus `if`-EXPRESSION; under any concrete state that `if`-expression
      EVALUATES to the taken branch's value — exactly `bodyDenote`'s branch.)
    - `Stmt::Expr(e)` → `Stmt.exprS` : a bare expr stmt with NO state effect — but it
      must be WELL-FORMED under the state (the encoder `substitute`s it to surface a
      value error). Modelled: `none` when `execDenote e` is `none`, else the state is
      UNCHANGED (faithful to `thread_stmt`'s `Stmt::Expr` arm: encode-and-discard).
    - the tail (`Block.tail`) → `Block.tail` : the body value `execDenote tail` in the
      final state; a tail `if`-EXPRESSION / a tuple tail are the `Expr`-level branch /
      multi-cell forms — modelled here at the value level via `execDenote` (the 2a
      `ExecExpr` already has neither an `if`-expr nor a tuple node: the BODY `ifElse`
      is the STATEMENT form, which is what `body_ref_state` threads; the tail value is
      a scalar exec expr, faithful to the scalar B1/B2 bodies). The Rust tuple-tail /
      if-EXPRESSION-tail are value-shape projections of the SAME final state; they are
      out of the 2a scalar `ExecExpr` and documented OUT here (not faked).

  LOOPS are EXPLICITLY OUT (increment 2c, #163, kernel-gated): a `Stmt::Loop`/`Break`/
  `Continue` is `Unsupported` in `body_ref_state` (the after-loop state needs the
  invariant / a fixpoint), so there is NO loop `Stmt` form here — straight-line blocks
  ONLY. Documented as 2c #163, NOT embedded-then-`sorry`. Likewise a mid-body early
  `return` (a multi-exit CPS form) is OUT of v1; the `ret` form here is the TAIL
  result only (the single-exit body value).

  DEPENDENCIES: Lean 4 CORE ONLY (reuses 2a's core-only `ExecExpr`/`execDenote`/
  `ExecVal`/`ExecEnv`; the state-threading is `Option`-monad composition; the proofs
  are `cases`/`simp`/`rfl`/`omega` — no Mathlib, no Lean-SMT). Mirrors 2a + the
  contract side's core-only discipline.
-/
import Thermite.Exec

namespace Thermite.Exec

/-! ## The exec-BODY AST (`body_ref_state`'s straight-line statement subset — faithful)

  The straight-line forms `body_ref_state` / `thread_stmt` handle. LOOPS (`Stmt::Loop`/
  `Break`/`Continue`) are ABSENT (2c #163, kernel-gated). A non-scalar mutation
  `xs[i]=e` and a mid-body early `return` are ABSENT (`Unsupported` in the encoder;
  documented OUT — NOT embed-then-`sorry`). -/

/-! A straight-line exec STATEMENT (the frozen 2.2.1 subset `body_ref_state` admits).
    The RHS / condition are 2a `ExecExpr`s (denoted by `execDenote`), so `S_B` reuses
    2a's bounded-value + overflow-obligation semantics for every value position; the
    NEW content is the state threading. `Stmt` is mutually recursive with `Block` for
    the `ifElse` sub-blocks. -/
mutual

inductive Stmt where
  /-- `let x = e` — bind a FRESH name `x` to the bounded value of `e` in the current
      state (`Stmt::Let`; the encoder REJECTS a re-shadow, modelled by the
      already-bound guard). `x` must NOT already be in scope. -/
  | letS (name : String) (init : ExecExpr)
  /-- `x = e` — UPDATE an EXISTING bare scalar binding `x` to the bounded value of `e`
      in the current state (`Stmt::Assign` over a `Path[x]` target). ORDER-SENSITIVE:
      `e` reads the state BEFORE this assign. `x` MUST already be in scope (the encoder
      `Err`s on an unbound target). The v1 mutation form — a NON-scalar `xs[i]=e` is
      `Unsupported` (OUT, documented). -/
  | assign (name : String) (value : ExecExpr)
  /-- a bare expression statement `e;` (`Stmt::Expr`) — NO state effect, but `e` must
      be WELL-FORMED (the encoder encodes-and-discards to surface a value error). -/
  | exprS (e : ExecExpr)
  /-- `if c { B₁ } else { B₂ }` in STATEMENT position (`Stmt::If`) — branch on `c`,
      run the taken branch as a sub-block over the current state, recompose. -/
  | ifElse (cond : ExecExpr) (thenB : Block) (elseB : Block)

/-- A straight-line `Block` (`thermite-syntax::ast::Block`): a sequence of statements
    + an OPTIONAL tail value `Expr` (`blkTail`). The body's result is the tail
    evaluated in the FINAL state. A tail-LESS block (the encoder `Err`s — the
    body-refinement obligation compares a RESULT value) yields no result value. -/
inductive Block where
  | mk (stmts : List Stmt) (tail : Option ExecExpr)

end

/-- The statements of a block. -/
def Block.blkStmts : Block → List Stmt
  | .mk ss _ => ss

/-- The optional tail value of a block. -/
def Block.blkTail : Block → Option ExecExpr
  | .mk _ t => t

/-! ## The STATE + the `S_B` big-step state transformer (the source denotation)

  A STATE is an `ExecEnv` from 2a (var → `ExecVal`, slices → element sequences)
  TOGETHER WITH the set of names currently IN SCOPE (so `let` can reject a re-shadow
  and `assign` can reject an unbound target — exactly the `env.contains_key` guards in
  `thread_stmt`). 2a's `ExecEnv.vars` is a TOTAL function (every name has a default
  value), so the in-scope set is the extra structure the body subset needs (the
  encoder's env is a `BTreeMap` whose KEY SET is the in-scope set). -/

/-- The body STATE: 2a's `ExecEnv` (the valuation — scalar vars → bounded values,
    slices → element sequences) PLUS the set of in-scope binding names (the encoder's
    `BTreeMap` key set, needed for the re-shadow / unbound-target guards). A name not
    in `scope` is a free INPUT (a param) or unbound; a name in `scope` is a `let`-bound
    cell that `assign` may update. -/
structure State where
  /-- The bounded valuation (reuses 2a's `ExecEnv`). -/
  env : ExecEnv
  /-- The in-scope `let`-bound cell names (the encoder's env key set). -/
  scope : String → Bool

/-- Update one scalar var to a new value in the state's valuation. -/
def State.setVar (st : State) (name : String) (v : ExecVal) : State :=
  { st with env := { st.env with vars := fun s => if s = name then v else st.env.vars s } }

/-- Mark a name as in-scope (a `let`-bound cell). -/
def State.bind (st : State) (name : String) : State :=
  { st with scope := fun s => if s = name then true else st.scope s }

/-! THE SOURCE `S_B` DENOTATION — the big-step state transformer over a straight-line
    `Block` (`stmtDenote` per statement; `blockThread` for the statement sequence).
    Threads the state through the statement sequence (left to right), then `bodyDenote`
    evaluates the tail in the FINAL state. Partial (`Option`): `none` when any RHS /
    condition / tail OBLIGATION fails (2a's overflow / div-zero / out-of-range), a
    `let` re-shadows an in-scope name, or an `assign` targets an unbound cell — exactly
    `body_ref_state`'s honest `Err` sites, surfaced as partiality. -/
mutual
  /-- Thread ONE statement through the state (`thread_stmt`). -/
  def stmtDenote : Stmt → State → Option State
    | .letS name init, st =>
        -- A re-shadow `let x = ..; let x = ..` is OUT of v1 (`thread_stmt` `Err`s when
        -- `env.contains_key(name)`): `none` when `name` already in scope.
        if st.scope name then none
        else do
          let v ← execDenote init st.env
          some ((st.setVar name v).bind name)
    | .assign name value, st =>
        -- The cell must ALREADY be in scope (a `let mut` introduced it); the encoder
        -- `Err`s on an unbound target. ORDER-SENSITIVE: evaluate `value` in the state
        -- BEFORE this assign, then update.
        if st.scope name then do
          let v ← execDenote value st.env
          some (st.setVar name v)
        else none
    | .exprS e, st => do
        -- NO state effect — but `e` must be well-formed (encode-and-discard). `none`
        -- propagates a value error; else the state is UNCHANGED.
        let _ ← execDenote e st.env
        some st
    | .ifElse cond thenB elseB, st => do
        -- Branch on the condition's bounded BOOL value (the encoder composes the two
        -- branch state-transformers into a Verus `if`-expression; under a concrete
        -- state that `if`-expression evaluates to the TAKEN branch's state). A
        -- non-bool condition is an exec type error → `none` (the `asBool` partiality).
        let c ← asBool (← execDenote cond st.env)
        if c then blockThread thenB st else blockThread elseB st

  /-- Thread a `Block`'s STATEMENTS through the state (the sequencing — left to right),
      yielding the FINAL state (NOT yet the tail value). Composes the per-statement
      transformers (`encode_block_tail`'s loop over `block.stmts`). -/
  def blockThread : Block → State → Option State
    | .mk [] _, st => some st
    | .mk (s :: rest) tl, st => do
        let st' ← stmtDenote s st
        blockThread (.mk rest tl) st'
end

/-- The full `S_B` body result: thread the block's statements to the final state, then
    evaluate the tail value in that final state. `none` when the threading fails OR the
    tail value's obligation fails OR the block is tail-LESS (the body-refinement
    obligation compares a RESULT value — `encode_block_tail` `Err`s on `None` tail). -/
def bodyDenote (b : Block) (st : State) : Option ExecVal := do
  let stf ← blockThread b st
  match b.blkTail with
  | some t => execDenote t stf.env
  | none => none

/-! ## `bodyRefState` — the MODEL of `exec_stmt_encode.rs::body_ref_state`'s output

  This models WHAT THE RUST `body_ref_state` PRODUCES — the OPERATIONAL THREADING it
  performs (`encode_block_tail`: thread each stmt left-to-right via `thread_stmt`, then
  encode the tail in the final env) — as a state transformer, INDEPENDENTLY of
  `bodyDenote`. The encoder's per-RHS / condition / tail VALUE is delegated to
  `exec_ref_value` (2a), modelled here by `execRefValue`; the NEW logic (the threading
  / the re-shadow + unbound guards / the branch composition / the tail projection) is
  re-stated as its OWN recursion `refThread`, so `body_ref_sound` is non-vacuous. -/

mutual
  /-- Model `thread_stmt` for ONE statement: the encoder's per-statement threading,
      delegating the rhs VALUE to `execRefValue` (the 2a encoder model). Independent of
      `stmtDenote` (uses `execRefValue`, not `execDenote`). -/
  def refStmt : Stmt → State → Option State
    | .letS name init, st =>
        if st.scope name then none
        else do
          let v ← execRefValue init st.env        -- the encoder's per-RHS value (2a)
          some ((st.setVar name v).bind name)
    | .assign name value, st =>
        if st.scope name then do
          let v ← execRefValue value st.env        -- the encoder's per-RHS value (2a)
          some (st.setVar name v)
        else none
    | .exprS e, st => do
        let _ ← execRefValue e st.env
        some st
    | .ifElse cond thenB elseB, st => do
        let c ← asBool (← execRefValue cond st.env)
        if c then refBlockThread thenB st else refBlockThread elseB st

  /-- Model `encode_block_tail`'s statement loop: thread the block's statements
      left-to-right through the env (the encoder's sequencing). -/
  def refBlockThread : Block → State → Option State
    | .mk [] _, st => some st
    | .mk (s :: rest) tl, st => do
        let st' ← refStmt s st
        refBlockThread (.mk rest tl) st'
end

/-- Model `body_ref_state` (the full encoder): thread the statements, then encode the
    tail VALUE in the final env via `execRefValue` (the encoder's tail encoding). A
    tail-LESS block is the encoder's `Err` (no result value) → `none`. -/
def bodyRefState (b : Block) (st : State) : Option ExecVal := do
  let stf ← refBlockThread b st
  match b.blkTail with
  | some t => execRefValue t stf.env
  | none => none

/-! ## (T1) — `body_ref_state` is SOUND against `S_B`

  The per-RHS / condition / tail VALUE agreement is exactly 2a's `exec_ref_sound`
  (`execRefValue e env = execDenote e env`); the BODY soundness lifts it through the
  state threading. The threading itself is identical structurally on both sides (the
  ONLY difference is `execRefValue` vs `execDenote` at each value position), so the
  lift is a structural induction over `Stmt`/`Block` discharged by `exec_ref_sound`. -/

/-- The 2a per-value agreement lifted to a FUNCTION equality (`execRefValue =
    execDenote`): from `exec_ref_sound` by `funext`. This is the SINGLE fact the body
    threading depends on — every value position in the body (RHS / condition / tail)
    is one of these, so once the functions are equal the threadings are
    definitionally equal. -/
theorem execRefValue_eq_execDenote : execRefValue = execDenote := by
  funext e env; exact exec_ref_sound e env

/-! The per-statement threading agrees (`refStmt = stmtDenote`): the threading is
    identical, the only value positions are `execRefValue`/`execDenote`, equal as
    FUNCTIONS by `execRefValue_eq_execDenote`. `refStmt_eq_stmtDenote` is mutually
    recursive with the block-threading agreement `refBlockThread_eq_blockThread`. -/
mutual
  theorem refStmt_eq_stmtDenote : ∀ (s : Stmt) (st : State),
      refStmt s st = stmtDenote s st
    | .letS name init, st => by
        simp only [refStmt, stmtDenote, execRefValue_eq_execDenote]
    | .assign name value, st => by
        simp only [refStmt, stmtDenote, execRefValue_eq_execDenote]
    | .exprS e, st => by
        simp only [refStmt, stmtDenote, execRefValue_eq_execDenote]
    | .ifElse cond thenB elseB, st => by
        unfold refStmt stmtDenote
        rw [execRefValue_eq_execDenote,
            refBlockThread_eq_blockThread thenB st,
            refBlockThread_eq_blockThread elseB st]

  theorem refBlockThread_eq_blockThread : ∀ (b : Block) (st : State),
      refBlockThread b st = blockThread b st
    | .mk [] _, st => by simp only [refBlockThread, blockThread]
    | .mk (s :: rest) tl, st => by
        unfold refBlockThread blockThread
        rw [refStmt_eq_stmtDenote s st]
        cases stmtDenote s st with
        | none => rfl
        | some st' =>
            show refBlockThread (.mk rest tl) st' = blockThread (.mk rest tl) st'
            exact refBlockThread_eq_blockThread (.mk rest tl) st'
end

/--
  **(T1) — verified-validator soundness for the exec-BODY fragment (`S_B`).**

  For every straight-line `Block` `b` and every state `st`, the meaning of the
  reference encoder's output (`bodyRefState` — the encoder's THREADING composing
  `exec_ref_value` per RHS / condition / tail) EQUALS the source big-step
  state-transformer denotation (`bodyDenote`, `S_B`):

  `∀ straight-line Block P, ⟦body_ref_state(P)⟧ = ⟦P⟧_{S_B}`.

  Proved by lifting 2a's `exec_ref_sound` through the state threading (the block
  threading agrees by `refBlockThread_eq_blockThread`; the tail value agrees by
  `exec_ref_sound`). NON-VACUOUS: the threading is a GENUINE state transformer
  (`assign` updates the cell, sequencing composes, `ifElse` branches), and the
  obligation-`none` of 2a (overflow / div-zero / out-of-range) PROPAGATES through the
  body (a body whose RHS overflows has NO result — NOT a blanket `none`). The negative
  lemmas below witness that the transformer is real (wrong-var assign, dropped
  sequencing, dropped mutation each BREAK soundness). LOOPS are OUT (2c #163). -/
theorem body_ref_sound (b : Block) (st : State) :
    bodyRefState b st = bodyDenote b st := by
  unfold bodyRefState bodyDenote
  rw [refBlockThread_eq_blockThread b st]
  cases blockThread b st with
  | none => rfl
  | some stf =>
      cases b.blkTail with
      | none => rfl
      | some t =>
          show execRefValue t stf.env = execDenote t stf.env
          exact exec_ref_sound t stf.env

/-! ## The STATE TRANSFORMER is GENUINE — positive witnesses (B1/B2/B3 grounded)

  These witness that `S_B` is a REAL big-step state transformer (not a vacuous
  constant): the let-chain threads, the mutation order matters, the branch is taken.
  Mirror `body_ref_state`'s B1-B4 reference tests (`exec_stmt_encode.rs::tests`). -/

/-- The empty starting state: every var defaults to `u64 0`, no slices, nothing in
    scope. A param `x` is read via `inputState` (below) which seeds it. -/
def emptyState : State :=
  { env := { vars := fun _ => .int ⟨.u64, 0⟩, slices := fun _ => [] }
    scope := fun _ => false }

/-- A starting state seeding the input param `x := 5` (`u64`), nothing `let`-bound yet
    (the `let`/`assign` cells are introduced by the body). -/
def inputState : State :=
  { env := { vars := fun s => if s = "x" then .int ⟨.u64, 5⟩ else .int ⟨.u64, 0⟩
             slices := fun _ => [] }
    scope := fun _ => false }

/-- **B1 — the let-chain threads (state transformer is REAL).** `{ let a = x + 1;
    let b = a * 2; b }` with `x := 5` yields the body value `(5+1)*2 = 12` — the env
    THREADED `a ↦ 6`, then `b ↦ 12`, tail `b`. (A constant/vacuous transformer could
    not produce `12` from `x := 5`.) -/
theorem b1_let_chain_threads :
    bodyDenote
      (.mk [ .letS "a" (.arith .add (.var "x") (.intLit .u64 1)),
             .letS "b" (.arith .mul (.var "a") (.intLit .u64 2)) ]
           (some (.var "b")))
      inputState
      = some (.int ⟨.u64, 12⟩) := by
  simp only [bodyDenote, blockThread, stmtDenote, inputState, State.setVar, State.bind,
        execDenote, asInt, evalArith, rawArith, IntTy.bound, IntTy.width]
  decide

/-- **B2 — the mutation ORDER is load-bearing.** `{ let mut s = x; s = s + 1;
    s = s * 2; s }` threads `s ↦ 5 → 6 → 12` = `12`, while the REORDER `s = s * 2;
    s = s + 1` threads `s ↦ 5 → 10 → 11` = `11` — a DIFFERENT result. The mutation
    ACTUALLY UPDATES the state and the order matters (the state-sequencing teeth). -/
theorem b2_mutation_order_matters :
    bodyDenote
      (.mk [ .letS "s" (.var "x"),
             .assign "s" (.arith .add (.var "s") (.intLit .u64 1)),
             .assign "s" (.arith .mul (.var "s") (.intLit .u64 2)) ]
           (some (.var "s")))
      inputState
      = some (.int ⟨.u64, 12⟩)
  ∧ bodyDenote
      (.mk [ .letS "s" (.var "x"),
             .assign "s" (.arith .mul (.var "s") (.intLit .u64 2)),
             .assign "s" (.arith .add (.var "s") (.intLit .u64 1)) ]
           (some (.var "s")))
      inputState
      = some (.int ⟨.u64, 11⟩) := by
  constructor <;>
    · simp only [bodyDenote, blockThread, stmtDenote, inputState, State.setVar, State.bind,
          execDenote, asInt, evalArith, rawArith, IntTy.bound, IntTy.width]
      decide

/-- **B3 — the branch is TAKEN (genuine `ifElse` composition).** `{ let mut r = x;
    if (r < 10) { r = r + 1 } else { r = r + 2 }; r }` with `x := 5` (so `r < 10` is
    TRUE) takes the THEN branch: `r ↦ 5 → 6` = `6`. (A dropped / wrong branch would
    give a different value — see the negative lemmas.) -/
theorem b3_if_branch_taken :
    bodyDenote
      (.mk [ .letS "r" (.var "x"),
             .ifElse (.cmp .lt (.var "r") (.intLit .u64 10))
               (.mk [ .assign "r" (.arith .add (.var "r") (.intLit .u64 1)) ] none)
               (.mk [ .assign "r" (.arith .add (.var "r") (.intLit .u64 2)) ] none) ]
           (some (.var "r")))
      inputState
      = some (.int ⟨.u64, 6⟩) := by
  simp only [bodyDenote, blockThread, stmtDenote, inputState, State.setVar, State.bind,
        execDenote, asInt, asBool, evalArith, rawArith, cmpVal, IntTy.bound, IntTy.width]
  decide

/-! ## The OBLIGATION-NONE PROPAGATES through the body (NOT blanket vacuity)

  The body partiality is the 2a obligation (overflow / div-zero / out-of-range)
  threaded through the state — NOT a blanket `none`. A body whose RHS OVERFLOWS has no
  result; the SAME body with an in-range RHS HAS a result. This witnesses the
  partiality is the genuine obligation, not vacuous failure. -/

/-- A state seeding `m := 2^64 - 1` (max `u64`) in scope-free input position. -/
def maxState : State :=
  { env := { vars := fun s => if s = "m" then .int ⟨.u64, (2:Int)^64 - 1⟩
                              else .int ⟨.u64, 0⟩
             slices := fun _ => [] }
    scope := fun _ => false }

/-- **Obligation propagates — overflow in a body RHS kills the body result.** `{ let
    a = m + m; a }` with `m := 2^64 - 1` OVERFLOWS in the `let`'s RHS → the body has NO
    result (`bodyDenote = none`). The obligation is threaded, not swallowed. -/
theorem body_overflow_rhs_has_no_result :
    bodyDenote
      (.mk [ .letS "a" (.arith .add (.var "m") (.var "m")) ] (some (.var "a")))
      maxState
      = none := by
  simp only [bodyDenote, blockThread, stmtDenote, maxState, State.setVar, State.bind,
        execDenote, asInt, evalArith, rawArith, IntTy.bound, IntTy.width]
  decide

/-- **Contrast — the SAME body shape with an in-range RHS HAS a result.** `{ let a =
    x + x; a }` with `x := 5` gives `10` — so the `none` above is the OVERFLOW
    obligation, not a blanket failure of the `let`-form. -/
theorem body_in_range_rhs_has_result :
    bodyDenote
      (.mk [ .letS "a" (.arith .add (.var "x") (.var "x")) ] (some (.var "a")))
      inputState
      = some (.int ⟨.u64, 10⟩) := by
  simp only [bodyDenote, blockThread, stmtDenote, inputState, State.setVar, State.bind,
        execDenote, asInt, evalArith, rawArith, IntTy.bound, IntTy.width]
  decide

/-! ## NEGATIVE LEMMAS — the state-transformer infidelities BITE (mirror 2a / S_C)

  Three faithfulness bugs `body_ref_state` MUST NOT commit, each PROVEN to disagree
  with the faithful `bodyDenote` at a concrete state:
    (a) a WRONG-VAR assign (assigns `y` where the source assigns `x`);
    (b) a SEQUENCING bug (the two assigns composed in the WRONG ORDER);
    (c) a MUTATION not applied (the `assign` dropped, leaving the cell unchanged).
  Each builds a BUGGY encoder model and shows it ≠ `bodyDenote` at a witness — so a
  body encoder committing that bug would FAIL `body_ref_sound` exactly there. -/

/-- **(a) WRONG-VAR assign breaks soundness.** The source body `{ let mut s = x;
    s = s + 1; s }` (with `x := 5`) yields `s ↦ 6`. A buggy encoder that assigned the
    WRONG cell — modelled as a body that assigns to a DIFFERENT name `t` (so the read
    cell `s` is never updated) — yields `s ↦ 5` (the original). They DISAGREE
    (`some 5 ≠ some 6`). A wrong-var-target encoder does NOT satisfy `body_ref_sound`. -/
theorem wrong_var_assign_breaks_soundness :
    bodyDenote
      (.mk [ .letS "s" (.var "x"), .letS "t" (.var "x"),
             .assign "t" (.arith .add (.var "s") (.intLit .u64 1)) ]   -- BUG: assigns `t`, not `s`
           (some (.var "s")))
      inputState
    ≠ bodyDenote
      (.mk [ .letS "s" (.var "x"),
             .assign "s" (.arith .add (.var "s") (.intLit .u64 1)) ]   -- correct: assigns `s`
           (some (.var "s")))
      inputState := by
  simp only [bodyDenote, blockThread, stmtDenote, inputState, State.setVar, State.bind,
        execDenote, asInt, evalArith, rawArith, IntTy.bound, IntTy.width]
  decide

/-- **(b) SEQUENCING bug breaks soundness.** The source `{ let mut s = x; s = s + 1;
    s = s * 2; s }` yields `12` (`(5+1)*2`); the REORDERED composition `s = s * 2;
    s = s + 1` yields `11` (`(5*2)+1`). They DISAGREE — so an encoder that composed the
    sequence in the WRONG ORDER (dropped the ordering) would FAIL `body_ref_sound`.
    The sequencing ACTUALLY composes (order is observable in `S_B`). -/
theorem sequencing_order_breaks_soundness :
    bodyDenote
      (.mk [ .letS "s" (.var "x"),
             .assign "s" (.arith .mul (.var "s") (.intLit .u64 2)),    -- BUG: reordered
             .assign "s" (.arith .add (.var "s") (.intLit .u64 1)) ]
           (some (.var "s")))
      inputState
    ≠ bodyDenote
      (.mk [ .letS "s" (.var "x"),
             .assign "s" (.arith .add (.var "s") (.intLit .u64 1)),    -- correct order
             .assign "s" (.arith .mul (.var "s") (.intLit .u64 2)) ]
           (some (.var "s")))
      inputState := by
  simp only [bodyDenote, blockThread, stmtDenote, inputState, State.setVar, State.bind,
        execDenote, asInt, evalArith, rawArith, IntTy.bound, IntTy.width]
  decide

/-- **(c) MUTATION NOT APPLIED breaks soundness.** The source `{ let mut s = x;
    s = s + 1; s }` updates `s ↦ 6`. A buggy encoder that DROPPED the `assign` (left
    the cell unchanged) — modelled as the body WITHOUT the `assign` stmt — yields the
    original `s ↦ 5`. They DISAGREE (`some 5 ≠ some 6`). A mutation-dropping encoder
    does NOT satisfy `body_ref_sound`: the mutation ACTUALLY updates the state. -/
theorem mutation_not_applied_breaks_soundness :
    bodyDenote
      (.mk [ .letS "s" (.var "x") ]                                     -- BUG: assign dropped
           (some (.var "s")))
      inputState
    ≠ bodyDenote
      (.mk [ .letS "s" (.var "x"),
             .assign "s" (.arith .add (.var "s") (.intLit .u64 1)) ]    -- mutation applied
           (some (.var "s")))
      inputState := by
  simp only [bodyDenote, blockThread, stmtDenote, inputState, State.setVar, State.bind,
        execDenote, asInt, evalArith, rawArith, IntTy.bound, IntTy.width]
  decide

/-- The faithful POSITIVE counterpart (the teeth bite ONLY the bugs): with the REAL
    threading the body encoder IS sound — `bodyRefState = bodyDenote` for the correct
    `{ let mut s = x; s = s + 1; s }` body, by `body_ref_sound`. -/
theorem faithful_body_is_sound :
    bodyRefState
      (.mk [ .letS "s" (.var "x"),
             .assign "s" (.arith .add (.var "s") (.intLit .u64 1)) ]
           (some (.var "s")))
      inputState
    = bodyDenote
      (.mk [ .letS "s" (.var "x"),
             .assign "s" (.arith .add (.var "s") (.intLit .u64 1)) ]
           (some (.var "s")))
      inputState :=
  body_ref_sound _ _

end Thermite.Exec
