/-
  Thermite/Exec/Stmt.lean — the exec-body sublanguage `S_B` (a big-step state
  transformer over straight-line blocks) plus the (T1) soundness theorem for the exec
  body reference encoder `body_ref_state` (increment 2b, #172; epic #169). This
  builds on increment 2a (`Thermite.Exec`): the RHS of a `let`/`assign`, an
  `if` condition, and a tail value are all exec expressions, denoted by 2a's
  `execDenote` / `ExecVal` / `ExecEnv`. `S_B` adds the state-threading,
  scalar-mutation rebind, fixed mutable-slice indexed update, branch composition,
  and tail projection on top of 2a's per-RHS bounded-value denotation.

  Governing design: `.design/verified/thermite-semantics.md` Architecture §"S_B — the
  exec-body sublanguage (state transformer; unified from exec-stmt-tv.md)",
  REQ-1/REQ-2/REQ-4 (increment (c), #172: "exec-statement `S_B` + prove
  `body_ref_state` sound"). The body state-transformer is grounded in
  `thermite-tv/src/exec_stmt_encode.rs::body_ref_state` (the big-step state denotation:
  the let/assign substitution-threading, the scalar-cell mutation order sensitivity,
  the `if`-statement branch composition, the tail / multi-cell-tuple projection) plus
  `exec-stmt-tv.md` REQ-1/REQ-2 (the frozen straight-line subset and the big-step rules).

  ════════════════════════════════════════════════════════════════════════════════
  The model: a big-step state transformer `S_B` over straight-line blocks.
  ════════════════════════════════════════════════════════════════════════════════

  A state is the mutable bindings, an `ExecEnv` from 2a (var → `ExecVal`, slices →
  element sequences). `S_B` is a big-step evaluation `⟨B, σ⟩ ⇓ ⟨σ', v⟩` threading an
  initial state through a `Block` to a final state plus the tail value. The transformer
  is partial (`Option`): it propagates `none` whenever a sub-expression's obligation
  fails (2a's overflow / div-zero / shift-zero / out-of-range index) or a binding is
  missing / out of the frozen subset. The straight-line statement forms (faithful to
  `body_ref_state`, `exec-stmt-tv.md` REQ-1 IN set):

    - `let x = e`   — bind `x` to `execDenote e` in the current state, extend the env.
    - `assign x = e`— update an existing scalar binding `x` to `execDenote e` (the
                      order-sensitive mutation: the rhs reads the state before this
                      assign; `s = s+1; s = s*2` ≠ `s = s*2; s = s+1`).
    - `sliceAssign xs i e` — evaluate `i` and `e` in the pre-write state, require
                      `0 ≤ i < xs.length`, and replace exactly cell `i`. This is the
                      top-level fixed mutable-slice effect admitted by the Rust
                      reference encoder; nested branch slice effects remain outside
                      that encoder's initial subset.
    - `ifElse c B₁ B₂` — branch on `execDenote c`: run `B₁` (then) or `B₂` (else) as a
                      sub-block state-transformer over the same state, recompose.
    - sequencing (`Block.stmts` in order) — compose the per-statement transformers.
    - `ret e` / tail `e` — the body result value, `execDenote e` in the final state.

  ════════════════════════════════════════════════════════════════════════════════
  Faithfulness to `body_ref_state` (the critic diffs this; read carefully).
  ════════════════════════════════════════════════════════════════════════════════

  The Rust `body_ref_state` is a symbolic encoder: its env maps name → a closed-form
  value `Expr` in the inputs, and `let`/`assign` substitute the rhs under the current
  env then rebind. That symbolic substitution is a big-step state transformer
  whose state is the valuation (`exec_stmt_encode.rs` module doc: "Big-step evaluation
  threads an initial environment through the statement sequence to a final
  environment; the body's value is the tail expression evaluated in that final
  environment"). `bodyDenote` (`S_B`) is that big-step transformer over values;
  `bodyRefState` models the encoder's threading (its operational order: thread each
  stmt left-to-right into the state, then evaluate the tail in the final state, the
  loop in `encode_block_tail` + `thread_stmt`), independently of `bodyDenote`.
  `body_ref_sound` proves them equal construct-by-construct. The match, arm
  by arm with `thread_stmt`:

    - `Stmt::Let { name, init }`  →  `Stmt.letS` : bind a fresh name (the encoder
      rejects a re-shadow `let x=..; let x=..`; modelled: `bodyRefState` is `none`
      when `name` is already bound, matching `env.contains_key(name)` → `Err`).
    - `Stmt::Assign { Path[x], value }` → `Stmt.assign` : rebind an existing bare
      scalar `x` (the encoder rejects an unbound target; modelled: `bodyRefState` is
      `none` when `x` is not already bound).
    - `Stmt::Assign { Index(slice, i), value }` → `Stmt.sliceAssign` for a bare,
      frame-declared mutable-slice name: evaluate `i`/`value` in the pre-write state,
      bounds-check, and replace exactly one sequence cell. The Rust recognizer is
      narrower than the operational form: top-level writes only, with no mutable-slice
      read in the index/value; other non-bare targets remain `Unsupported`.
    - `Stmt::If { cond, then, else_ }` (statement position) → `Stmt.ifElse` : run each
      branch over the state, recompose. (The Rust composes the per-cell branch values
      into a Verus `if`-expression; under any concrete state that `if`-expression
      evaluates to the taken branch's value, matching `bodyDenote`'s branch.)
    - `Stmt::Expr(e)` → `Stmt.exprS` : a bare expr stmt with no state effect, but it
      must be well-formed under the state (the encoder `substitute`s it to surface a
      value error). Modelled: `none` when `execDenote e` is `none`, else the state is
      unchanged (faithful to `thread_stmt`'s `Stmt::Expr` arm: encode-and-discard).
    - the tail (`Block.tail`) → `Block.tail` : the body value `execDenote tail` in the
      final state; a tail `if`-expression / a tuple tail are the `Expr`-level branch /
      multi-cell forms, modelled here at the value level via `execDenote` (the 2a
      `ExecExpr` has neither an `if`-expr nor a tuple node: the body `ifElse`
      is the statement form, which is what `body_ref_state` threads; the tail value is
      a scalar exec expr, faithful to the scalar B1/B2 bodies). The Rust tuple-tail /
      if-expression-tail are value-shape projections of the same final state; they are
      out of the 2a scalar `ExecExpr` and documented out here (not faked).

  Loops are out (increment 2c, #163, kernel-gated): a `Stmt::Loop`/`Break`/
  `Continue` is `Unsupported` in `body_ref_state` (the after-loop state needs the
  invariant / a fixpoint), so there is no loop `Stmt` form here; straight-line blocks
  only. Documented as 2c #163, not embedded-then-`sorry`. Likewise a mid-body early
  `return` (a multi-exit CPS form) is out of v1; the `ret` form here is the tail
  result only (the single-exit body value).

  Dependencies: Lean 4 core only (reuses 2a's core-only `ExecExpr`/`execDenote`/
  `ExecVal`/`ExecEnv`; the state-threading is `Option`-monad composition; the proofs
  are `cases`/`simp`/`rfl`/`omega`; no Mathlib, no Lean-SMT). Mirrors 2a and the
  contract side's core-only discipline.
-/
import Thermite.Exec
import Thermite.Denote

namespace Thermite.Exec

/-! ## The exec-body AST (`body_ref_state`'s straight-line statement subset)

  The straight-line forms `body_ref_state` / `thread_stmt` handle. Loops (`Stmt::Loop`/
  `Break`/`Continue`) are absent (2c #163, kernel-gated). A single-index assignment
  to a frame-declared mutable slice is represented by `sliceAssign`; field writes,
  range writes, slice writes nested under control flow, and a mid-body early `return`
  remain absent (`Unsupported` in the encoder; documented out, not
  embed-then-`sorry`). -/

/-! A straight-line exec statement (the frozen 2.2.1 subset `body_ref_state` admits).
    The RHS / condition are 2a `ExecExpr`s (denoted by `execDenote`), so `S_B` reuses
    2a's bounded-value and overflow-obligation semantics for every value position; the
    new content is the state threading. `Stmt` is mutually recursive with `Block` for
    the `ifElse` sub-blocks. -/
mutual

inductive Stmt where
  /-- `let x = e` — bind a fresh name `x` to the bounded value of `e` in the current
      state (`Stmt::Let`; the encoder rejects a re-shadow, modelled by the
      already-bound guard). `x` must not already be in scope. -/
  | letS (name : String) (init : ExecExpr)
  /-- `x = e` — update an existing bare scalar binding `x` to the bounded value of `e`
      in the current state (`Stmt::Assign` over a `Path[x]` target). Order-sensitive:
      `e` reads the state before this assign. `x` must already be in scope (the encoder
      `Err`s on an unbound target). -/
  | assign (name : String) (value : ExecExpr)
  /-- `slice[index] = value` for a frame-declared mutable slice. Both expressions
      are evaluated in the pre-write state; the index must be in range, then exactly
      that list cell is replaced. The Rust reference emits the corresponding full
      sequence post-state `final(slice)@ == old(slice)@.update(index, value)`. -/
  | sliceAssign (slice : String) (index value : ExecExpr)
  /-- a bare expression statement `e;` (`Stmt::Expr`): no state effect, but `e` must
      be well-formed (the encoder encodes-and-discards to surface a value error). -/
  | exprS (e : ExecExpr)
  /-- `if c { B₁ } else { B₂ }` in statement position (`Stmt::If`): branch on `c`,
      run the taken branch as a sub-block over the current state, recompose. -/
  | ifElse (cond : ExecExpr) (thenB : Block) (elseB : Block)

/-- A straight-line `Block` (`thermite-syntax::ast::Block`): a sequence of statements
    plus an optional tail value `Expr` (`blkTail`). The body's result is the tail
    evaluated in the final state. A tail-less block (the encoder `Err`s; the
    body-refinement obligation compares a result value) yields no result value. -/
inductive Block where
  | mk (stmts : List Stmt) (tail : Option ExecExpr)

end

/-- The statements of a block. -/
def Block.blkStmts : Block → List Stmt
  | .mk ss _ => ss

/-- The optional tail value of a block. -/
def Block.blkTail : Block → Option ExecExpr
  | .mk _ t => t

/-! ## The state and the `S_B` big-step state transformer (the source denotation)

  A state is an `ExecEnv` from 2a (var → `ExecVal`, slices → element sequences)
  together with the set of names currently in scope (so `let` can reject a re-shadow
  and `assign` can reject an unbound target, the `env.contains_key` guards in
  `thread_stmt`). 2a's `ExecEnv.vars` is a total function (every name has a default
  value), so the in-scope set is the extra structure the body subset needs (the
  encoder's env is a `BTreeMap` whose key set is the in-scope set). -/

/-- The body state: 2a's `ExecEnv` (the valuation: scalar vars → bounded values,
    slices → element sequences) plus the set of in-scope binding names (the encoder's
    `BTreeMap` key set, needed for the re-shadow / unbound-target guards). A name not
    in `scope` is a free input (a param) or unbound; a name in `scope` is a `let`-bound
    cell that `assign` may update. -/
structure State where
  /-- The bounded valuation (reuses 2a's `ExecEnv`). -/
  env : ExecEnv
  /-- The in-scope `let`-bound cell names (the encoder's env key set). -/
  scope : String → Bool

/-- Update one scalar var to a new value in the state's valuation. -/
def State.setVar (st : State) (name : String) (v : ExecVal) : State :=
  { st with env := { st.env with vars := fun s => if s = name then v else st.env.vars s } }

/-- Replace one named slice sequence in the state's valuation. -/
def State.setSlice (st : State) (name : String) (xs : List BVal) : State :=
  { st with env := { st.env with slices := fun s => if s = name then xs else st.env.slices s } }

/-- Mark a name as in-scope (a `let`-bound cell). -/
def State.bind (st : State) (name : String) : State :=
  { st with scope := fun s => if s = name then true else st.scope s }

/-- Project a branch's resulting state `branch` back onto the pre-`if` cell/scope set
    of `pre` (the encoder's `env.keys()` recomposition in `thread_stmt`'s `Stmt::If`
    arm). Keeps the pre-`if` cells' (possibly branch-mutated) values: a branch's
    `assign` to an in-scope cell persists, as the encoder composes the
    branch-cell value into the post-`if` env, but discards any branch-local `let`
    (a cell not in the pre-`if` scope set, which lived only in the branch-env clone).
    The post-`if` scope set is the pre-`if` scope set; an in-scope cell takes the
    branch's value, a non-pre-`if` name keeps the pre-`if` valuation (the branch-local
    `let` does not leak). This mirrors `Stmt::If`'s `then_env = env.clone()` discipline
    (`thermite-tv/src/exec_stmt_encode.rs`; `.design/verified/exec-stmt-tv.md`
    REQ-2). -/
def State.restoreScope (pre branch : State) : State :=
  { env := { vars := fun s => if pre.scope s then branch.env.vars s else pre.env.vars s
             slices := branch.env.slices
             variants := pre.env.variants }
    scope := pre.scope }

/-! The source `S_B` denotation — the big-step state transformer over a straight-line
    `Block` (`stmtDenote` per statement; `blockThread` for the statement sequence).
    Threads the state through the statement sequence (left to right), then `bodyDenote`
    evaluates the tail in the final state. Partial (`Option`): `none` when any RHS /
    condition / tail obligation fails (2a's overflow / div-zero / out-of-range), a
    `let` re-shadows an in-scope name, or an `assign` targets an unbound cell, the
    `body_ref_state` `Err` sites, surfaced as partiality. -/
mutual
  /-- Thread one statement through the state (`thread_stmt`). -/
  def stmtDenote : Stmt → State → Option State
    | .letS name init, st =>
        -- A re-shadow `let x = ..; let x = ..` is out of v1 (`thread_stmt` `Err`s when
        -- `env.contains_key(name)`): `none` when `name` already in scope.
        if st.scope name then none
        else do
          let v ← execDenote init st.env
          some ((st.setVar name v).bind name)
    | .assign name value, st =>
        -- The cell must already be in scope (a `let mut` introduced it); the encoder
        -- `Err`s on an unbound target. Order-sensitive: evaluate `value` in the state
        -- before this assign, then update.
        if st.scope name then do
          let v ← execDenote value st.env
          some (st.setVar name v)
        else none
    | .sliceAssign slice index value, st => do
        let iv ← asInt (← execDenote index st.env)
        let vv ← asInt (← execDenote value st.env)
        let xs := st.env.slices slice
        if 0 ≤ iv.value ∧ iv.value < (xs.length : Int) then
          some (st.setSlice slice (xs.set iv.value.toNat vv))
        else none
    | .exprS e, st => do
        -- No state effect, but `e` must be well-formed (encode-and-discard). `none`
        -- propagates a value error; else the state is unchanged.
        let _ ← execDenote e st.env
        some st
    | .ifElse cond thenB elseB, st => do
        -- Branch on the condition's bounded bool value (the encoder composes the two
        -- branch state-transformers into a Verus `if`-expression; under a concrete
        -- state that `if`-expression evaluates to the taken branch's state). A
        -- non-bool condition is an exec type error → `none` (the `asBool` partiality).
        -- The taken branch threads over the pre-`if` state; its resulting state is then
        -- projected back onto the pre-`if` cell/scope set (`State.restoreScope`); a
        -- branch-local `let` does not leak past the `if` (the encoder's `Stmt::If`
        -- `then_env = env.clone()` + `env.keys()` recomposition discipline). The
        -- branch's `assign` effects on pre-`if` cells persist; the branch-local `let`s
        -- are discarded (so a post-`if` `let` of a branch-local name is a fresh bind).
        let c ← asBool (← execDenote cond st.env)
        let branch ← (if c then blockThread thenB st else blockThread elseB st)
        some (st.restoreScope branch)

  /-- Thread a `Block`'s statements through the state (the sequencing, left to right),
      yielding the final state (not yet the tail value). Composes the per-statement
      transformers (`encode_block_tail`'s loop over `block.stmts`). -/
  def blockThread : Block → State → Option State
    | .mk [] _, st => some st
    | .mk (s :: rest) tl, st => do
        let st' ← stmtDenote s st
        blockThread (.mk rest tl) st'
end

/-- The full `S_B` body result: thread the block's statements to the final state, then
    evaluate the tail value in that final state. `none` when the threading fails, the
    tail value's obligation fails, or the block is tail-less (the body-refinement
    obligation compares a result value; `encode_block_tail` `Err`s on `None` tail). -/
def bodyDenote (b : Block) (st : State) : Option ExecVal := do
  let stf ← blockThread b st
  match b.blkTail with
  | some t => execDenote t stf.env
  | none => none

/-! ## `bodyRefState` — the model of `exec_stmt_encode.rs::body_ref_state`'s output

  This models what the Rust `body_ref_state` produces, the operational threading it
  performs (`encode_block_tail`: thread each stmt left-to-right via `thread_stmt`, then
  encode the tail in the final env), as a state transformer, independently of
  `bodyDenote`. The encoder's per-RHS / condition / tail value is delegated to
  `exec_ref_value` (2a), modelled here by `execRefValue`; the new logic (the threading
  / the re-shadow + unbound guards / the branch composition / the tail projection) is
  re-stated as its own recursion `refThread`, so `body_ref_sound` is non-vacuous. -/

mutual
  /-- Model `thread_stmt` for one statement: the encoder's per-statement threading,
      delegating the rhs value to `execRefValue` (the 2a encoder model). Independent of
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
    | .sliceAssign slice index value, st => do
        let iv ← asInt (← execRefValue index st.env)
        let vv ← asInt (← execRefValue value st.env)
        let xs := st.env.slices slice
        if 0 ≤ iv.value ∧ iv.value < (xs.length : Int) then
          some (st.setSlice slice (xs.set iv.value.toNat vv))
        else none
    | .exprS e, st => do
        let _ ← execRefValue e st.env
        some st
    | .ifElse cond thenB elseB, st => do
        -- Same branch-env-clone discipline as `stmtDenote.ifElse`: thread the taken
        -- branch over the pre-`if` state, then project back onto the pre-`if`
        -- cell/scope set (`State.restoreScope`); a branch-local `let` is discarded
        -- (the encoder's `then_env = env.clone()` + `env.keys()` recomposition). Both
        -- sides discard identically so `body_ref_sound` still holds.
        let c ← asBool (← execRefValue cond st.env)
        let branch ← (if c then refBlockThread thenB st else refBlockThread elseB st)
        some (st.restoreScope branch)

  /-- Model `encode_block_tail`'s statement loop: thread the block's statements
      left-to-right through the env (the encoder's sequencing). -/
  def refBlockThread : Block → State → Option State
    | .mk [] _, st => some st
    | .mk (s :: rest) tl, st => do
        let st' ← refStmt s st
        refBlockThread (.mk rest tl) st'
end

/-- Model `body_ref_state` (the full encoder): thread the statements, then encode the
    tail value in the final env via `execRefValue` (the encoder's tail encoding). A
    tail-less block is the encoder's `Err` (no result value) → `none`. -/
def bodyRefState (b : Block) (st : State) : Option ExecVal := do
  let stf ← refBlockThread b st
  match b.blkTail with
  | some t => execRefValue t stf.env
  | none => none

/-! ## (T1) — `body_ref_state` is sound against `S_B`

  The per-RHS / condition / tail value agreement is 2a's `exec_ref_sound`
  (`execRefValue e env = execDenote e env`); the body soundness lifts it through the
  state threading. The threading itself is structurally identical on both sides (the
  only difference is `execRefValue` vs `execDenote` at each value position), so the
  lift is a structural induction over `Stmt`/`Block` discharged by `exec_ref_sound`. -/

/-- The 2a per-value agreement lifted to a function equality (`execRefValue =
    execDenote`): from `exec_ref_sound` by `funext`. This is the single fact the body
    threading depends on; every value position in the body (RHS / condition / tail)
    is one of these, so once the functions are equal the threadings are
    definitionally equal. -/
theorem execRefValue_eq_execDenote : execRefValue = execDenote := by
  funext e env; exact exec_ref_sound e env

/-! The per-statement threading agrees (`refStmt = stmtDenote`): the threading is
    identical, the only value positions are `execRefValue`/`execDenote`, equal as
    functions by `execRefValue_eq_execDenote`. `refStmt_eq_stmtDenote` is mutually
    recursive with the block-threading agreement `refBlockThread_eq_blockThread`. -/
mutual
  theorem refStmt_eq_stmtDenote : ∀ (s : Stmt) (st : State),
      refStmt s st = stmtDenote s st
    | .letS name init, st => by
        simp only [refStmt, stmtDenote, execRefValue_eq_execDenote]
    | .assign name value, st => by
        simp only [refStmt, stmtDenote, execRefValue_eq_execDenote]
    | .sliceAssign slice index value, st => by
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
  (T1) — verified-validator soundness for the exec-body fragment (`S_B`).

  For every straight-line `Block` `b` and every state `st`, the meaning of the
  reference encoder's output (`bodyRefState`, the encoder's threading composing
  `exec_ref_value` per RHS / condition / tail) equals the source big-step
  state-transformer denotation (`bodyDenote`, `S_B`):

  `∀ straight-line Block P, ⟦body_ref_state(P)⟧ = ⟦P⟧_{S_B}`.

  Proved by lifting 2a's `exec_ref_sound` through the state threading (the block
  threading agrees by `refBlockThread_eq_blockThread`; the tail value agrees by
  `exec_ref_sound`). Non-vacuous: the threading is a state transformer
  (`assign` updates the cell, sequencing composes, `ifElse` branches), and the
  obligation-`none` of 2a (overflow / div-zero / out-of-range) propagates through the
  body (a body whose RHS overflows has no result, not a blanket `none`). The negative
  lemmas below witness that the transformer is real (wrong-var assign, dropped
  sequencing, dropped mutation each break soundness). Loops are out (2c #163). -/
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

/-! ## Aggregate-valued body results

  A struct-returning straight-line body uses the same statement transformer and an
  aggregate tail relation rather than forcing a struct through the scalar `ExecVal`
  result-binding bridge. This matches `body_ref_state_ensures`: thread statements,
  independently encode each named initializer, then compare every result field.
-/

/-- Source meaning of a straight-line statement sequence with a named-aggregate
    result expression. -/
def structBodyDenote (stmts : List Stmt) (fields : StructExpr) (st : State) :
    Option StructVal := do
  let stf ← blockThread (.mk stmts none) st
  structDenote fields stf.env

/-- Independent body-reference meaning for that aggregate-valued result. -/
def structBodyRefState (stmts : List Stmt) (fields : StructExpr) (st : State) :
    Option StructVal := do
  let stf ← refBlockThread (.mk stmts none) st
  structRefValue fields stf.env

/-- Aggregate body refinement composes the already-proved statement transformer
    with exact field construction. No aggregate result is dropped at `bindResult`;
    it is discharged here by per-field equality before the contract bridge. -/
theorem struct_body_ref_sound (stmts : List Stmt) (fields : StructExpr) (st : State) :
    structBodyRefState stmts fields st = structBodyDenote stmts fields st := by
  unfold structBodyRefState structBodyDenote
  rw [refBlockThread_eq_blockThread (.mk stmts none) st]
  cases blockThread (.mk stmts none) st with
  | none => rfl
  | some stf => exact struct_ref_sound fields stf.env

/-! ## The state transformer is non-vacuous — positive witnesses (B1/B2/B3 grounded)

  These witness that `S_B` is a real big-step state transformer (not a vacuous
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

/-- B1 — the let-chain threads. `{ let a = x + 1;
    let b = a * 2; b }` with `x := 5` yields the body value `(5+1)*2 = 12`: the env
    threaded `a ↦ 6`, then `b ↦ 12`, tail `b`. (A constant/vacuous transformer could
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

/-- B2 — the mutation order is required. `{ let mut s = x; s = s + 1;
    s = s * 2; s }` threads `s ↦ 5 → 6 → 12` = `12`, while the reorder `s = s * 2;
    s = s + 1` threads `s ↦ 5 → 10 → 11` = `11`, a different result. The mutation
    updates the state, so the order matters. -/
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

/-- B3 — the branch is taken (`ifElse` composition). `{ let mut r = x;
    if (r < 10) { r = r + 1 } else { r = r + 2 }; r }` with `x := 5` (so `r < 10` is
    true) takes the then branch: `r ↦ 5 → 6` = `6`. (A dropped / wrong branch would
    give a different value; see the negative lemmas.) -/
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

/-! ## Mutable-slice state effects are exact (the service-write vertical slice) -/

/-- A concrete mutable-slice call state: `data = [1,2,3]`, `at = 1`, `value = 7`.
    The index is a bounded `usize`; the stored value and slice elements are `u8`. -/
def sliceWriteState : State :=
  { env :=
      { vars := fun s =>
          if s = "at" then .int ⟨.usize, 1⟩
          else if s = "value" then .int ⟨.u8, 7⟩
          else .int ⟨.u64, 0⟩
        slices := fun s =>
          if s = "data" then [⟨.u8, 1⟩, ⟨.u8, 2⟩, ⟨.u8, 3⟩] else [] }
    scope := fun _ => false }

def correctSliceWrite : Block :=
  .mk [.sliceAssign "data" (.var "at") (.var "value")] (some (.var "value"))

/-- The source transformer replaces exactly `data[1]` and preserves both other
    cells. This is the list meaning of the Rust/Verus
    `old(data)@.update(at as int, value)` post-state. -/
theorem slice_write_updates_exact_cell :
    Option.map (fun st => st.env.slices "data")
      (blockThread correctSliceWrite sliceWriteState)
      = some [⟨.u8, 1⟩, ⟨.u8, 7⟩, ⟨.u8, 3⟩] := by
  simp [correctSliceWrite, blockThread, stmtDenote, sliceWriteState, State.setSlice,
        execDenote, asInt]

/-- The independent reference-state thread has the same complete final slice, by
    the general statement-refinement theorem (not merely the same scalar return). -/
theorem slice_write_reference_state_is_sound :
    refBlockThread correctSliceWrite sliceWriteState
      = blockThread correctSliceWrite sliceWriteState :=
  refBlockThread_eq_blockThread _ _

/-- Wrong-index tooth: changing the generated write target to cell zero changes the
    complete final sequence even though the scalar return remains `value`. -/
theorem wrong_slice_index_breaks_state_refinement :
    Option.map (fun st => st.env.slices "data")
      (blockThread
        (.mk [.sliceAssign "data" (.intLit .usize 0) (.var "value")]
             (some (.var "value")))
        sliceWriteState)
    ≠ Option.map (fun st => st.env.slices "data")
      (blockThread correctSliceWrite sliceWriteState) := by
  simp [correctSliceWrite, blockThread, stmtDenote, sliceWriteState, State.setSlice,
        execDenote, asInt, IntTy.bound, IntTy.width]

/-- Wrong-value tooth: storing zero instead of the source value changes the final
    sequence even though the generated body can still return the expected scalar. -/
theorem wrong_slice_value_breaks_state_refinement :
    Option.map (fun st => st.env.slices "data")
      (blockThread
        (.mk [.sliceAssign "data" (.var "at") (.intLit .u8 0)]
             (some (.var "value")))
        sliceWriteState)
    ≠ Option.map (fun st => st.env.slices "data")
      (blockThread correctSliceWrite sliceWriteState) := by
  simp [correctSliceWrite, blockThread, stmtDenote, sliceWriteState, State.setSlice,
        execDenote, asInt, IntTy.bound, IntTy.width]

/-! ## The obligation-none propagates through the body (not blanket vacuity)

  The body partiality is the 2a obligation (overflow / div-zero / out-of-range)
  threaded through the state, not a blanket `none`. A body whose RHS overflows has no
  result; the same body with an in-range RHS has a result. This witnesses the
  partiality is the obligation, not vacuous failure. -/

/-- A state seeding `m := 2^64 - 1` (max `u64`) in scope-free input position. -/
def maxState : State :=
  { env := { vars := fun s => if s = "m" then .int ⟨.u64, (2:Int)^64 - 1⟩
                              else .int ⟨.u64, 0⟩
             slices := fun _ => [] }
    scope := fun _ => false }

/-- Obligation propagates — overflow in a body RHS kills the body result. `{ let
    a = m + m; a }` with `m := 2^64 - 1` overflows in the `let`'s RHS, so the body has no
    result (`bodyDenote = none`). The obligation is threaded, not swallowed. -/
theorem body_overflow_rhs_has_no_result :
    bodyDenote
      (.mk [ .letS "a" (.arith .add (.var "m") (.var "m")) ] (some (.var "a")))
      maxState
      = none := by
  simp only [bodyDenote, blockThread, stmtDenote, maxState, State.setVar, State.bind,
        execDenote, asInt, evalArith, rawArith, IntTy.bound, IntTy.width]
  decide

/-- Contrast — the same body shape with an in-range RHS has a result. `{ let a =
    x + x; a }` with `x := 5` gives `10`, so the `none` above is the overflow
    obligation, not a blanket failure of the `let`-form. -/
theorem body_in_range_rhs_has_result :
    bodyDenote
      (.mk [ .letS "a" (.arith .add (.var "x") (.var "x")) ] (some (.var "a")))
      inputState
      = some (.int ⟨.u64, 10⟩) := by
  simp only [bodyDenote, blockThread, stmtDenote, inputState, State.setVar, State.bind,
        execDenote, asInt, evalArith, rawArith, IntTy.bound, IntTy.width]
  decide

/-! ## Negative lemmas — the state-transformer infidelities bite (mirror 2a / S_C)

  Three faithfulness bugs `body_ref_state` must not commit, each proven to disagree
  with the faithful `bodyDenote` at a concrete state:
    (a) a wrong-var assign (assigns `y` where the source assigns `x`);
    (b) a sequencing bug (the two assigns composed in the wrong order);
    (c) a mutation not applied (the `assign` dropped, leaving the cell unchanged).
  Each builds a buggy encoder model and shows it ≠ `bodyDenote` at a witness, so a
  body encoder committing that bug would fail `body_ref_sound` exactly there. -/

/-- (a) wrong-var assign breaks soundness. The source body `{ let mut s = x;
    s = s + 1; s }` (with `x := 5`) yields `s ↦ 6`. A buggy encoder that assigned the
    wrong cell, modelled as a body that assigns to a different name `t` (so the read
    cell `s` is never updated), yields `s ↦ 5` (the original). They disagree
    (`some 5 ≠ some 6`). A wrong-var-target encoder does not satisfy `body_ref_sound`. -/
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

/-- (b) sequencing bug breaks soundness. The source `{ let mut s = x; s = s + 1;
    s = s * 2; s }` yields `12` (`(5+1)*2`); the reordered composition `s = s * 2;
    s = s + 1` yields `11` (`(5*2)+1`). They disagree, so an encoder that composed the
    sequence in the wrong order (dropped the ordering) would fail `body_ref_sound`.
    The sequencing composes (order is observable in `S_B`). -/
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

/-- (c) mutation not applied breaks soundness. The source `{ let mut s = x;
    s = s + 1; s }` updates `s ↦ 6`. A buggy encoder that dropped the `assign` (left
    the cell unchanged), modelled as the body without the `assign` stmt, yields the
    original `s ↦ 5`. They disagree (`some 5 ≠ some 6`). A mutation-dropping encoder
    does not satisfy `body_ref_sound`: the mutation updates the state. -/
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

/-- With the faithful threading, the body encoder is sound:
    `bodyRefState = bodyDenote` for the correct
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

/-! ## The exec-body bridge — `bodyConverges` + the `S_E→S_C` value bridge (#253, §4.1)

  The increment-(iv) artifacts that tie `S_B` (the exec body) to `S_C` (the contract). The
  design decision (§4.1.5) is that the exec side needs no bottom-distinguishing NB layer:
  `bodyDenote : Block → State → Option ExecVal` is fuel-free; `ExecExpr` has no `specCall`
  constructor, so there is no registry, no fuel index, and no default-value bottom anywhere on
  the exec side. Its `none` arises only at failure sites (the `evalArith` overflow /
  div-or-shift-by-zero, the out-of-range `index`, the `asInt`/`asBool` sort mismatch, the `letS`
  re-shadow, the unbound `assign`, a tail-less block), and `some v` means a value: the
  contrasting lemmas `body_overflow_rhs_has_no_result` (`= none`) and `body_in_range_rhs_has_result`
  (`= some (.int ⟨.u64, 10⟩)`). The `Option` is the bottom-distinguishing layer; the #213/#241
  trap (a total denotation that forges a value at the bottom) does not exist in `S_B`. So
  `bodyConverges` is a definitional abbreviation over `bodyDenote`, not a new denotation. -/

/-- The exec-body convergence relation (§4.1.5): the body `b` run from state `st` produces the
    result `r`. A one-line abbrev over the fuel-free, Option-bottom-distinguishing `bodyDenote`
    ("converges", not "stabilizes"; there is no fuel to stabilize over). Uniqueness is free:
    `bodyDenote` is a function, so `some`-results are unique by `Option.some.injEq` (no analogue
    of `stabilizes_unique` is needed, in contrast with the §4 pure-contract `stabilizes`
    result-binding, which does need uniqueness). The hypothesize obligation antecedent is
    `bodyConverges body_block (stateOf v) r`; the overflow class is exported alongside as
    `(bodyDenote body_block (stateOf v)).isSome` per the §4.1 conjunction rule. -/
abbrev bodyConverges (b : Block) (st : State) (r : ExecVal) : Prop :=
  bodyDenote b st = some r

/-- The `S_E→S_C` value bridge (§4.1.1/§4.1.2): bind an exec body result `r : ExecVal` into the
    contract environment `env` at the name `"result"`. The bridge is the identity on the
    mathematical value:
      - an int-sorted `r = .int b` binds `Thermite.Env.bindInt env "result" b.value`; `BVal.value`
        is the mathematical unsigned value (`evalArith` yields "the mathematical result given no
        overflow, never a wrap, never a nat-coercion", `Exec.lean`). Nothing else: no `Int.toNat`
        clamp, no `% bound` re-wrap, no signed reinterpretation (the exec domain is the unsigned
        `[0, ty.bound)`; the four `PinExec*` pins kernel-check that a mis-bridge breaks soundness).
        The width `b.ty` is not carried into `Env` (`S_C` compares mathematical
        values).
      - a bool-sorted `r = .bool b` binds `Thermite.Env.bindBool env "result" b` (the §4.1.2
        spine prerequisite: a bool sort rather than an Int-0/1 encoding, which
        `PinExportBoolResult.lean`'s `true_false_indistinguishable_in_intVal` proves unsound).
    The contract `ens` reads `result` as `Expr.var "result"` (int) / `Expr.boolVar "result"`
    (bool); the soundness pins witness both bind positions. -/
def bindResult (env : Thermite.Env) (r : ExecVal) : Thermite.Env :=
  match r with
  | .int b  => Thermite.Env.bindInt env "result" b.value
  | .bool b => Thermite.Env.bindBool env "result" b

end Thermite.Exec
