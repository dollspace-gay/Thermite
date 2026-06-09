# Exec-Statement (Body) Translation Validation — step 2.2 / kernel groundwork

<!--
tier: 3-component
status: draft
governs: thermite-tv/src/exec_stmt_encode.rs, thermite-tv/src/obligation.rs, thermite-lower/src/lower.rs, forge/src/body_tv.rs
thesis-refs:
  - thermite-design.md §1 (trust relocated: code → spec → spec-intent)
  - thermite-design.md §4.1 (contract-first functions — the exec BODY they guard: let/assign/while/inv/dec)
  - thermite-design.md §6 (the verification ladder; L3 = Verus-derived SMT proof; L1 runtime checks)
  - thermite-design.md §5.1 (counterexamples, not adjectives)
  - thermite-design.md §13 (v0.1 kernel scope; the forward-looking verified-microkernel convergence)
epic: crosslink #158
step-2.1-sibling: .design/verified/exec-tv.md (crosslink #151 — exec-EXPRESSION TV, shipped + total on the corpus)
step-1-sibling: .design/verified/contract-tv.md (crosslink #139 — CONTRACT-position TV, shipped + total on the corpus)
-->

## Summary

Step 2.1 (`.design/verified/exec-tv.md`, SHIPPED) certifies that a pure exec-position EXPRESSION's
lowered VALUE matches an independent EXEC-semantics reference — arithmetic/cast/comparison/call/index,
NO state. It does NOT cover exec STATEMENTS / BODIES: `let`-binding, assignment/mutation, statement
SEQUENCING, `if`/`else` as statements, and the through-body STATE TRANSFORMATION of a whole fn. The
question 2.2 answers is whether the lowerer emits a BODY whose STATE TRANSFORMATION (not just a single
value) matches the source — a dropped statement, a reordered mutation, or a swapped `if`-branch all
change the final state while every individual sub-expression may be faithful.

This is "kernel-gated" for two reasons that converge: (a) a mechanized operational semantics is futile
against a MOVING target, so the exec-statement subset must be FROZEN first; (b) the frozen subset + the
mechanized operational semantics ARE the verified-microkernel's exec-language foundation. This
component delivers the groundwork:

1. **FREEZE the kernel exec-statement subset v1** (REQ-1) — the enumerated `Stmt`/`Block`/`LoopNode`
   construct set, IN vs OUT, pinned from what the toolchain has TODAY.
2. **Design the operational-semantics reference denotation** (REQ-2) — a statement sequence as a STATE
   TRANSFORMER, independent of production (the N-version reference, like `exec_ref_value` but over
   STATE).
3. **Design + GROUND step 2.2.1 straight-line body-refinement TV** (REQ-3) — the concrete first slice,
   grounded end-to-end against real verus below.
4. **Frame step 2.2.2 loops as the harder horizon** (REQ-4) — the candidate approaches, why
   kernel-gated, NOT designed.

**Scope split (the crux — pin it precisely):**
- **Step 2.2.1 (DESIGNED HERE + GROUNDED, tractable NOW):** STATE-refinement TV for STRAIGHT-LINE
  bodies — `let`/assignment/mutation/`if`/sequencing, **NO loops**. The body's final state is a total
  function of the inputs; the obligation compares production's lowered body's final state to the
  reference state-transformation. GROUNDED below (faithful → VERIFIED; dropped statement / reordered
  mutation / swapped branch → counterexample).
- **Step 2.2.2 (FRAMED ONLY, kernel-gated):** LOOPS — the state after a `while` needs the loop
  invariant / bounded unrolling / a fixpoint argument. The hard, kernel-gated piece. NOT designed here
  (see "Step 2.2.2 horizon").
- **Sibling groundwork (FRAMED, NOT designed here):** the `no_std` freestanding target
  (`forge build --target kernel`) — the verified-microkernel enablement. It is a SEPARATE deliverable,
  NOT a step-2.2 prerequisite (see "Kernel convergence").

## Trust model (same as step 1 / 2.1 — N-version differential validation, not proof)

Body-TV checks `production-body-lowering ≡ independent-operational-reference` over STATE. Agreement is
EVIDENCE, not PROOF (both could share a wrong assumption). What makes it meaningful is the same
asymmetry of auditability: the reference state-denotation is a small total recursion over the FROZEN
exec-statement subset, authored against `thermite-design.md` §4.1/§6 + standard imperative semantics,
independently of `lower_block_inner in lower.rs` / `lower_stmt in lower.rs`. A human certifies the
reference by inspection; the production body lowering (the ~5000-line shape-keyed `lower.rs`) cannot.
The honesty boundary is HARD (REQ-2): the reference MUST NOT call any production lowering symbol; the
`thermite-tv` crate keeps NO `thermite-lower` dependency (the step-1/2.1 invariant, `cargo tree -p
thermite-tv` = syntax + spec only).

**Where step 2.1 plugs in.** Each statement's RHS is an exec EXPRESSION — `let a = x + 1`'s `x + 1`,
the `if` condition, an assignment's value. Step 2.1's `exec_ref_value` ALREADY checks those expression
denotations per-value. 2.2.1 adds the orthogonal axis: the STATE SEQUENCING and mutation-ORDER
faithfulness ON TOP of the per-RHS value faithfulness. The state-denotation (REQ-2) composes
`exec_ref_value` over each statement's RHS — it does not re-author expression encoding.

## REQ-1 — the FROZEN kernel exec-statement subset v1 (the key deliverable)

Enumerated from the EXISTING surface (`enum Stmt`, `struct Block`, `struct LoopNode`, `enum LoopKind`
in `thermite-syntax/src/ast.rs`; lowered by `lower_stmt`/`lower_block_inner`/`lower_loop in lower.rs`).
This set is DECLARED STABLE: no new exec-statement construct is admitted into the kernel exec language
or the operational semantics without a design amendment (the moving-target problem `exec-tv.md` "Step
2.2 horizon" named). It is BOTH the operational semantics' fixed target AND the verified-microkernel's
exec language v1.

**IN (the frozen v1 construct set):**

| Construct | AST node | Production lowering | State role |
|---|---|---|---|
| immutable let-binding | `Stmt::Let { mutable: false, .. }` | `let <name>[: T] = <init>;` (`lower_stmt`) | introduces a fresh value binding |
| mutable let-binding | `Stmt::Let { mutable: true, .. }` | `let mut <name>[: T] = <init>;` | introduces a mutable state cell |
| assignment / mutation | `Stmt::Assign { target, value }` | `<target> = <value>;` | updates an in-scope mutable cell |
| sequencing | `Block.stmts` ordered + `Block.tail` | `lower_block_inner` (statements in order, then tail) | threads state left-to-right |
| `if` / `if-else` statement | `Stmt::If { cond, then, else_ }` | `if <cond> { .. } [else { .. }]` (`lower_stmt`) | branch on a `bool` exec expr |
| block tail value | `Block.tail: Option<Box<Expr>>` | trailing expr = the block's value (`lower_block_inner`) | the body's RESULT (final-state projection) |
| expression statement | `Stmt::Expr(e)` | `<e>;` | a side-effecting exec expr (a non-tail call) |
| `return` | `Stmt::Return(Option<Expr>)` | `return [<e>];` | early exit with a value (straight-line: TAIL position only — see OUT) |
| `while` with `inv`/`dec` | `Stmt::Loop(LoopNode { kind: While(..), invs, dec, .. })` | `lower_loop` | **step 2.2.2** — framed, not in 2.2.1 |
| `loop` with `inv`/`dec` | `Stmt::Loop(LoopNode { kind: Loop, invs, dec, .. })` | `lower_loop` | **step 2.2.2** — framed |
| `break` | `Stmt::Break` | `break;` (`lower_stmt`) | **step 2.2.2** — loop-control only |
| `continue` | `Stmt::Continue` | `continue;` (`lower_stmt`) | **step 2.2.2** — loop-control only |

The RHS expression sublanguage of every IN statement is the step-2.1 pure-exec subset
(`exec-tv.md` Architecture): arithmetic/cast/comparison/call/index at the bounded `u64`/`u32`/`usize`
type. The state values are those bounded scalar types (the v1 kernel-exec state is scalar; see OUT).

**OUT (explicitly NOT in kernel exec subset v1 — honest boundary):**

- **Loops in step 2.2.1.** `Stmt::Loop` (both `While`/`Loop` `kind`s), `break`, `continue` are IN the
  FROZEN SUBSET (the kernel exec language has loops) but are step **2.2.2**, NOT the 2.2.1 straight-line
  slice. A 2.2.1 body containing a `Stmt::Loop` is HONESTLY SKIPPED (never silently passed).
- **Early `return` inside an `if`/`while` branch (mid-body control flow).** v1 admits `return` only in
  TAIL position of a straight-line body (equivalent to the block tail). An early return nested inside an
  `if`-branch splits the state-denotation into a multi-exit CPS form; OUT of v1 (a 2.2.2-adjacent edge,
  framed not designed). The state-denotation (REQ-2) is the SINGLE-EXIT final-state function.
- **`match` as a statement / `match`-bound state.** Exec `match` (C7 Option/Result payload) is exec-
  expression territory (`exec-tv.md`); a `match` that MUTATES per-arm is OUT of v1.
- **Non-scalar mutable state (Vec/Map/String mutation).** v1 state is bounded SCALAR cells
  (`u64`/`u32`/`usize`/`bool`). A `Vec::push`/`Map::insert`-mutating body is OUT (the state-denotation
  would need a sequence/map theory; a v2 extension). Such a body is HONESTLY SKIPPED.
- **Recursion-as-statement / nested-fn definitions.** No fn definitions inside a body; a recursive call
  is an exec-EXPRESSION (step 2.1 checks its value), not a statement form.
- **Shadowing edge cases.** v1 assumes each `let` introduces a distinct name (no `let x = ..; let x =
  ..` re-shadow in the same block); a re-shadow is OUT of v1 (the denotation's environment is a flat
  name→value map). HONESTLY SKIPPED if detected.

This frozen list is the design-pinned contract REQ-2's reference is authored against and REQ-4's loop
horizon extends. Derived from `thermite-design.md` §4.1 (the exec body the contract guards) + the
existing `Stmt`/`LoopNode` surface.

## REQ-2 — the operational-semantics reference denotation (state-transformer, independent)

The reference DENOTATION for the frozen straight-line subset: a statement sequence is a STATE
TRANSFORMER. The **program state** is the environment of in-scope mutable + immutable bindings
(name → bounded-scalar value). For a straight-line body the denotation is a **big-step** evaluation
threading an initial environment (the fn params) through the statement sequence to a FINAL environment,
and the body's value is the tail expression evaluated in that final environment.

`thermite_tv::exec_stmt_encode::body_ref_state(block: &Block, &BodyRefCtx) -> Result<String>` maps a
straight-line `Block` to a Verus **spec-fn state-denotation**: the final state (and hence the tail
value) as a FUNCTION of the initial state (the inputs). Mechanizability dictates big-step over
small-step for the straight-line case: the final state is a closed-form expression in the inputs, so
it lowers to a single Verus `spec fn` returning the tail-value (the projection of the final state the
body returns), e.g. for `{ let a = x + 1; let b = a * 2; b }` the denotation is
`spec fn body_ref(x) -> nat { ((x as nat) + 1) * 2 }` — each `let`/assignment SUBSTITUTED in order
(the threading), the tail returned. Mutation (`s = s + 1; s = s * 2`) is the same SUBSTITUTION at the
mutated cell, ORDER-SENSITIVE (a reorder changes the substitution chain → a different closed form). An
`if` denotes a Verus `if`-expression over the two branch state-transformers.

**Independence (HARD, R-CHAR-3 / trust model):** `body_ref_state` MUST NOT call any
`thermite_lower::lower::*` symbol. It composes step 2.1's `exec_ref_value` (`exec_encode.rs`) on each
statement's RHS / the `if` condition / the tail — the per-RHS expression VALUE is already an
independent reference — and adds ONLY the state-threading / mutation-substitution / branch-composition
logic (the new, small, auditable part). The `thermite-tv` crate keeps NO `thermite-lower` dependency.

Derived from `thermite-design.md` §4.1/§6 + standard big-step imperative semantics.

## REQ-3 — step-2.2.1 straight-line body-refinement TV (the concrete first slice, GROUNDED)

The body-refinement obligation wraps production's lowered straight-line body as the BODY of an exec
`fn`, with `ensures <final-state projection> == <body_ref_state denotation>`, discharged through the
existing `forge::check::run_verus`. This is the STATE analogue of step 2.1's `exec_equivalence_obligation`
(`obligation.rs`): 2.1 compares a single value; 2.2.1 compares the FINAL STATE (the tail value, which
is the projection of the final environment a single-exit body returns).

```verus
use vstd::prelude::*;
verus! {
    spec fn body_ref(<inputs>) -> <state-proj-ty> { <body_ref_state denotation> }

    fn tv_body_wrap(<inputs>) -> (result: <ret>)
        requires <enclosing req / well-formedness frame>,
        ensures result as <state-proj-ty> == body_ref(<inputs>),
    {
        <production lowering of the straight-line body — the artifact under test>
    }
}
fn main() {}
```

VERIFIED (`success: true, verified: 1, errors: 0`) ⟺ production's lowered body produces the reference
FINAL STATE for ALL inputs (Z3) ⟺ faithful. A `postcondition not satisfied` counterexample ⟺ a
state-transformation infidelity (a dropped statement, a reordered mutation, a swapped `if`-branch — any
of which changes the final state while each sub-expression stays value-faithful). The production body
is an EXEC `fn` (not `proof`/`spec`), so the always-active runtime overflow checks (`thermite-design.md`
§6, L1) are LIVE — the same structural reason as 2.1's exec-fn obligation.

**The home (reuse the step-2.1 architecture; extend, do not fork):**
- `thermite-tv/src/exec_stmt_encode.rs` — the NEW reference state-denotation encoder (REQ-2), sibling
  to `exec_encode.rs` (which stays exec-EXPRESSION-only). Composes `exec_ref_value`.
- `thermite-tv/src/obligation.rs` — a NEW `body_equivalence_obligation` + `BodyObligationFrame`
  (REQ-3), sibling to `exec_equivalence_obligation`. Emits the body-wrapped `ensures` form above.
- `thermite-lower/src/lower.rs` — a `lower_exec_body` per-body EXEC-context entry (the production side,
  blocker #161 — the analogue of the `lower_exec_expr` step-2.1 prerequisite; `lower_block_inner` is
  private and fn-context-bound, so a thermite-tv-driven obligation needs a reachable per-body exec
  entry).
- `forge/src/body_tv.rs` — the NEW forge check phase (REQ-5, blocker #162), sibling to `exec_tv.rs`.
  Four-way `Faithful` / `Divergent` / `Unverifiable` / `Skipped` (a loop body / non-scalar state / an
  early-return-in-branch is SKIPPED honestly — step 2.2.2 / OUT), discharged through
  `forge::check::run_verus` + the `ScratchDir` (#53) cleanup, exactly as `exec_tv::discharge`.

## Requirements

- **REQ-1 (the FROZEN kernel exec-statement subset v1)** — the enumerated IN/OUT construct set above
  (`Stmt::Let`/`Assign`/`If`/`Expr`/`Return`-tail + `Block` sequencing/tail for 2.2.1; `Stmt::Loop`/
  `Break`/`Continue` IN the frozen set but step 2.2.2; non-scalar state / mid-body early return /
  `match`-state / recursion-as-stmt / re-shadow OUT). Declared STABLE — no new construct without a
  design amendment. This is the operational semantics' fixed target AND the kernel exec language v1.
  Derived from `thermite-design.md` §4.1 + the `Stmt`/`LoopNode in ast.rs` surface. **Blocker #158
  (epic).**
- **REQ-2 (operational-semantics reference state-denotation — independent)** —
  `thermite_tv::exec_stmt_encode::body_ref_state(block: &Block, &BodyRefCtx) -> Result<String>` maps a
  straight-line `Block` (the frozen 2.2.1 subset) to a Verus spec-fn STATE-TRANSFORMER: the final state
  (tail value) as a closed-form function of the inputs, big-step, threading each `let`/assignment in
  ORDER (mutation = order-sensitive substitution), an `if` as a branch-composed Verus `if`-expression.
  Composes step-2.1's `exec_ref_value` on each RHS / condition / tail. **HARD CONSTRAINT (R-CHAR-3):**
  MUST NOT call `thermite_lower::lower::*`; `thermite-tv` keeps NO `thermite-lower` dep. Derived from
  `thermite-design.md` §4.1/§6. **Blocker #159.**
- **REQ-3 (step-2.2.1 straight-line body state-refinement obligation + discharge)** —
  `thermite_tv::obligation::body_equivalence_obligation(body: &Block, p_production: &str, frame:
  &BodyObligationFrame) -> Result<String>` emits the self-contained `fn tv_body_wrap(<inputs>) requires
  <req>, ensures result == body_ref(<inputs>), { <p_production> }` Verus unit (the STATE analogue of
  `exec_equivalence_obligation`, over the final state, not a single value). Discharged through the
  EXISTING `forge::check::run_verus`. VERIFIED ⟺ faithful body STATE TRANSFORMATION; a `postcondition
  not satisfied` ⟺ a state-transformation infidelity (dropped stmt / reordered mutation / swapped
  branch). The production side is the per-body exec lowering (`lower_exec_body`, blocker #161); the
  reference is REQ-2. GROUNDED below. **Blocker #160 (obligation) + #161 (production entry).**
- **REQ-4 (step-2.2.2 loops — the harder horizon)** — the after-loop state needs the loop invariant /
  bounded unrolling / a fixpoint; the candidate approaches + why kernel-gated are framed in "Step 2.2.2
  horizon" below. **NOW DESIGNED** in the dedicated `.design/verified/loop-tv.md` (#163): the chosen
  architecture is a variant of (a) — the three per-run loop obligations (entry / preservation / exit =
  `inv ∧ ¬cond`) reusing the SHIPPED single-iteration `body_ref_state` step, + a Lean PARTIAL-CORRECTNESS
  WHILE-RULE extending the `Faithfulness.lean` `h_tv` capstone (termination stays the per-run Verus
  `decreases` residual). **Blocker #163.**
- **REQ-5 (forge plug-in point)** — a new `forge::body_tv` check phase runs the straight-line body
  state-refinement TV over each checked item's exec body, exposed as `forge body-tv <file>` (non-test
  consumer `cli::run_body_tv`). Four-way `Faithful`/`Divergent`/`Unverifiable`/`Skipped` reported
  DISTINCTLY (a loop / non-scalar-state / mid-body early-return body is Skipped HONESTLY — never
  masking an infidelity, R-HONEST-3). Derived from `thermite-design.md` §6. **Blocker #162.**

## Acceptance criteria

- **AC-1 (faithful straight-line body → verified)** — the body obligation for `{ let a = x + 1; let b
  = a * 2; b }` (`x: u64`, `req x <= 1000`, reference `body_ref(x) = ((x as nat) + 1) * 2`, production
  the faithful `let a = x + 1; let b = a * 2; b`) discharges as `success: true, verified: 1, errors:
  0`. GROUNDED below.
- **AC-2 (dropped-statement infidelity → counterexample)** — the SAME obligation with production
  dropping `let b = a * 2` and returning `a` fails verus with `postcondition not satisfied`. GROUNDED
  below.
- **AC-3 (reordered-mutation infidelity → counterexample)** — for the mutation body `{ let mut s = x;
  s = s + 1; s = s * 2; s }` (reference `(x + 1) * 2`), a production that REORDERS the mutations
  (`s = s * 2; s = s + 1` → `x * 2 + 1`) fails with `postcondition not satisfied`; the correctly-ordered
  production VERIFIES. GROUNDED below.
- **AC-4 (`if`-statement state-transformer + swapped-branch infidelity → counterexample)** — for `{ let
  mut r = x; if x < 10 { r = r + 1; } else { r = r + 2; } r }` (reference `if x < 10 { x+1 } else
  { x+2 }`), the faithful production VERIFIES; a production that SWAPS the branches fails with
  `postcondition not satisfied`. GROUNDED below.
- **AC-5 (step 2.1 plugs in — per-RHS value faithfulness composes)** — the reference state-denotation
  composes `exec_ref_value` on each statement RHS; a body whose RHS carries a value infidelity (the
  #122/#146 class) is caught by the SAME obligation (the RHS value faithfulness is necessary for the
  state faithfulness). (Verification: a body with a #146 cast-`<` RHS fails the obligation — inherited
  from `exec-tv.md` AC-3.)
- **AC-6 (independence is structural)** — `thermite-tv` keeps NO `thermite-lower` dependency;
  `exec_stmt_encode.rs` references no `lower_stmt`/`lower_block`/`lower_expr` symbol (`cargo tree -p
  thermite-tv` = syntax + spec only).
- **AC-7 (loops / out-of-scope bodies skipped HONESTLY)** — a body containing a `Stmt::Loop`, a
  non-scalar mutation, or a mid-body early return reaches the `forge::body_tv` phase as `Skipped` (with
  a reason), NEVER as `Faithful` — the honest 2.2.1-vs-2.2.2 boundary in the certificate.

## Verification

GROUNDED end-to-end against the real `verus` binary (`Verus 0.2026.05.24.ecee80a`) during authoring,
exactly as 2.1. The conformance test (a future `thermite-tv/tests/body_teeth.rs` + `forge/tests/
body_tv_conformance.rs`, blocker #160/#162) replays these through `forge::check::run_verus`.

**AC-1 (faithful straight-line `{ let a = x + 1; let b = a * 2; b }` → verified).**
Obligation: `spec fn body_ref(x: u64) -> nat { ((x as nat) + 1) * 2 }` +
`fn tv_body_wrap(x: u64) -> (result: u64) requires x <= 1000, ensures result as nat == body_ref(x), {
let a: u64 = x + 1; let b: u64 = a * 2; b }`:

```
"verification-results": { "success": true, "verified": 1, "errors": 0 }
```

**AC-2 (dropped statement → counterexample).** SAME obligation, production drops `let b = a * 2` and
returns `a`:

```
error: postcondition not satisfied
 --> drop.rs:6:13
  |             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^ failed this postcondition
9 |     a
error: aborting due to 1 previous error
"verification-results": { "success": false, "verified": 0, "errors": 1 }
```

**AC-3 (reordered mutation → counterexample).** Reference `((x as nat) + 1) * 2`; faithful production
`let mut s = x; s = s + 1; s = s * 2; s`:

```
"verification-results": { "success": true, "verified": 1, "errors": 0 }
```

REORDERED production `let mut s = x; s = s * 2; s = s + 1; s` (final state `x*2+1 ≠ (x+1)*2`):

```
error: postcondition not satisfied
  --> reorder.rs:6:13
   |             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^ failed this postcondition
11 |     s
error: aborting due to 1 previous error
"verification-results": { "success": false, "verified": 0, "errors": 1 }
```

This is the STATE-SEQUENCING teeth: each statement's RHS (`s + 1`, `s * 2`) is value-faithful in
isolation (step 2.1 would pass each), but the MUTATION ORDER changes the final state — exactly what
2.2.1 adds on top of 2.1.

**AC-4 (`if`-statement state-transformer + swapped branch).** Reference `if x < 10 { (x as nat) + 1 }
else { (x as nat) + 2 }`; faithful production `let mut r = x; if x < 10 { r = r + 1; } else { r = r +
2; } r`:

```
"verification-results": { "success": true, "verified": 1, "errors": 0 }
```

SWAPPED-branch production (`if x < 10 { r = r + 2; } else { r = r + 1; }`):

```
error: postcondition not satisfied
"verification-results": { "success": false, "verified": 0, "errors": 1 }
```

These confirm the load-bearing feasibility questions for 2.2.1: (1) Verus DOES discharge `ensures
result == <reference state-denotation>` for a production-lowered STRAIGHT-LINE body wrapped as an exec
fn, threading `let`/mutation/`if` state — the big-step closed-form reference is Z3-comparable
(AC-1/3/4 verify); (2) the state-transformation teeth bite on exactly the body-lowering infidelity
classes the per-expression 2.1 TV cannot see — a dropped statement (AC-2), a reordered mutation
(AC-3), a swapped branch (AC-4), each a `postcondition not satisfied` counterexample, none a silent
pass. **Straight-line body state-refinement (2.2.1) is genuinely tractable NOW.**

**Crate gauntlet (when built):** `cargo test -p thermite-tv`, `cargo test -p forge` (`body_tv`
conformance), `cargo clippy -p thermite-tv -p forge --all-targets -- -D warnings`, `cargo fmt
--check`. Scratch/verus temp cleaned per the `ScratchDir` Drop guard (blocker #53).

## Step 2.2.2 horizon — LOOPS (kernel-gated, FRAMED not designed)

**The object.** Step 2.2.1 covers STRAIGHT-LINE bodies (a closed-form final state). It does NOT cover
loop bodies: the state AFTER a `while`/`loop` (`Stmt::Loop`, `break`, `continue` — IN the frozen subset
but deferred to 2.2.2). A loop's final state is NOT a closed-form substitution of the inputs — it is a
FIXPOINT of the loop-body state-step, reached after an input-dependent number of iterations.

**Why it is harder (the kernel-gated piece).** A straight-line body's denotation is finite
substitution; a loop's denotation is a recurrence whose closed form may not exist (or requires
induction). Verus cannot unfold an unbounded loop into a single comparable expression. The candidate
approaches:

- **(a) Single-iteration state-step TV + rely on the production invariant.** TV the loop BODY as a
  straight-line state-transformer (the 2.2.1 machinery: the per-iteration `(state, i) → (state', i+1)`
  step is straight-line), then argue the whole-loop refinement from the production loop INVARIANT
  (which `lower_loop` already emits + Verus checks). The body-step TV catches a per-iteration
  state-lowering infidelity; the invariant + `decreases` (already L3-checked) carry the induction. This
  is the most tractable candidate — it reuses 2.2.1 for the step and leans on the EXISTING verified
  invariant rather than re-deriving the loop's closed form.
- **(b) Bounded unrolling for a bound.** For a loop with a known small bound, unroll N iterations into a
  straight-line body and apply 2.2.1 directly — sound only up to the bound (a bounded-model-checking
  flavour, the L2/Kani spirit of `thermite-design.md` §13 v0.2), NOT a full refinement.

**WHY it stays FRAMED (the frozen-subset prerequisite).** A loop's operational semantics is a
definition of meaning for the loop-step + the fixpoint/invariant interaction. Authoring it against a
GROWING exec-statement set chases a moving target (the `exec-tv.md` step-2.2 argument). REQ-1's frozen
subset is the prerequisite: once the exec-statement construct set is pinned (this doc) and the v0.1
kernel's exec-body set is mechanically complete (`goal.md` stopping condition), 2.2.2's loop reference
+ refinement is authored against the frozen list. Until then, 2.2.2 stays framed; a loop body is
`Skipped` HONESTLY by `forge::body_tv` (AC-7). **Blocker #163.**

## Kernel convergence — the frozen subset IS the kernel exec language (note, not designed here)

The REQ-1 frozen exec-statement subset + the REQ-2 operational semantics are the verified-microkernel's
EXEC-LANGUAGE foundation: a kernel written in Thermite needs (a) a pinned, semantics-bearing exec
statement language (REQ-1/REQ-2 deliver exactly this — let/assign/mutation/seq/if/while with a
mechanized state-transformer meaning) and (b) a freestanding `no_std` lowering target. (b) is a
SEPARATE sibling groundwork item — `forge build --target kernel` emitting `no_std` freestanding
Verus-Rust (no `vstd::prelude::*` std assumptions, a freestanding allocator/panic discipline). It is
**NOT a step-2.2 prerequisite** (2.2.1/2.2.2 are TV over the SAME lowering target as the rest of the
toolchain). It is FRAMED here as a sibling and tracked as blocker #164; it is NOT designed in this doc
(it would be its own `.design/verified/kernel-target.md` when scheduled). The convergence point #158
names is: the exec-statement subset this doc freezes serves BOTH the TV arc (2.2.1/2.2.2) AND the
kernel's exec language — one frozen set, two consumers.

## The honest boundary (state in the certificate)

- **2.2.1 (straight-line, NOW, GROUNDED):** `let`/assignment/mutation/`if`/sequencing — the final state
  is a closed-form function of the inputs; the obligation discharges in verus (grounded above). Body
  state-refinement certifies the body's STATE TRANSFORMATION matches the source, on top of step 2.1's
  per-RHS value faithfulness.
- **2.2.2 (loops, KERNEL-GATED):** `while`/`loop`/`break`/`continue` — the after-loop state needs the
  invariant / unrolling / a fixpoint. Framed, not designed; a loop body is Skipped honestly.
- **no_std kernel target (SIBLING groundwork):** `forge build --target kernel` — separate deliverable,
  not a step-2.2 prerequisite, framed not designed (blocker #164).

A reader must NOT read straight-line body-TV (2.2.1) as whole-body (loop-inclusive) faithfulness, nor
as the kernel target.

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (frozen kernel exec-statement subset v1) | SHIPPED | the IN/OUT construct set is PINNED IN CODE: `thermite_tv::exec_stmt_encode::body_ref_state` (and its `thread_stmt`/`encode_value`) ADMIT exactly `Stmt::Let`/`Assign`/`If`/`Expr`/tail-`Return` + `Block` sequencing/tail, and HONESTLY REJECT (an `Unsupported` `Err`) `Stmt::Loop`/`Break`/`Continue` (2.2.2), a mid-`if`-branch early return, `match`-stmt, non-scalar mutation, and a re-shadow — the design-amendment-gated stable set; mirrored on the production side by `thermite_lower::lower_exec_body` (a loop body → `LowerError::Unsupported`, the `exec_body_tests::loop_body_is_err_not_silent` pin). Verified by `thermite-tv/tests/body_teeth.rs` B1–B4 + `exec_stmt_encode::tests` (the loop/re-shadow honest-skip tests). |
| REQ-2 (operational-semantics reference state-denotation) | SHIPPED | `pub fn body_ref_state` (+ `body_ref_state_ensures`, `BodyRefCtx`) in `thermite-tv/src/exec_stmt_encode.rs` — the big-step state-transformer (let/assign substitution-threading, mutation-ORDER sensitivity, `if`-branch composition, multi-cell TUPLE projection), composing step-2.1's `exec_ref_value` on each env-substituted RHS / condition / tail. Non-test consumer: `thermite_tv::obligation::body_equivalence_obligation`. Independence is STRUCTURAL: deps `thermite-syntax` + `thermite-spec` ONLY (`cargo tree -p thermite-tv` — no `thermite-lower`, AC-6). Verified by `tests/body_teeth.rs` B1–B4 against real verus (B2 mutation-ORDER, B4 multi-cell tuple) + `exec_stmt_encode::tests` (the closed-form pins, incl. the reorder ≠ ordered form). |
| REQ-3 (step-2.2.1 straight-line body state-refinement obligation + discharge) | SHIPPED | `thermite_tv::obligation::body_equivalence_obligation` + `BodyObligationFrame`/`BodyParamDecl` (`obligation.rs`) — emits the self-contained `fn tv_body_wrap(<inputs>) requires <req>, ensures <result-state == body_ref_state>, { <p_production> }` STATE form (single-cell: `result == <ref>`; multi-cell: `result.0 == <c0> && result.1 == <c1>`). The production side is the per-body exec entry `thermite_lower::lower_exec_body` (#161 — `lower_block_inner(block, Ctx::exec(), 0, zero_span())`, the minimal standalone-body frame, pinned by `lower.rs::exec_body_tests` B1–B4 as the cross-crate faithful bridge). GROUNDED end-to-end against real verus (`Verus 0.2026.05.24`, `tests/body_teeth.rs`): all FOUR faithful bodies VERIFY (`verified: 1, errors: 0`); the dropped-statement (B1) / reordered-mutation (B2) / swapped-branch (B3) / wrong-cell (B4) infidelities each fail `postcondition not satisfied` (`errors: 1`). The forge `body_tv` phase consumer is REQ-5 (#162, next dispatch — the `lower_exec_expr`→`forge::exec_tv` precedent). |
| REQ-4 (step-2.2.2 loops — harder horizon) | NOT-STARTED | open prereq blocker #163. Kernel-gated (REQ-1 frozen-subset prerequisite). NOW DESIGNED in `.design/verified/loop-tv.md`: a variant of (a) — three per-run loop obligations (entry/preservation/exit `inv ∧ ¬cond`) reusing the SHIPPED `body_ref_state` single-step + a Lean partial-correctness WHILE-RULE (`Exec/Loop.lean`, new) extending the `Faithfulness.lean` `h_tv` capstone; termination is the per-run Verus `decreases` residual. Bounded unrolling (b) DROPPED for v1 (the future v0.2 L2 fallback for invariant-free loops). Unbuilt. |
| REQ-5 (forge `body_tv` plug-in point) | NOT-STARTED | open prereq blocker #162. No `forge::body_tv` module (`forge/src/` has `exec_tv.rs` for EXPRESSIONS; `grep` for `body_tv`/`run_body_tv` in `forge/src/` is empty). The four-way `Faithful`/`Divergent`/`Unverifiable`/`Skipped` plug-in (sibling to `forge::exec_tv::run_generated`) is designed above; unbuilt. Depends on REQ-2/REQ-3. |
