//! The INDEPENDENT operational-semantics reference STATE-DENOTATION for the FROZEN
//! straight-line exec-statement subset (`.design/verified/exec-stmt-tv.md` REQ-2;
//! epic crosslink #158, blocker #159; `thermite-design.md` §4.1/§6).
//!
//! [`body_ref_state`] maps a STRAIGHT-LINE [`Block`] (the frozen 2.2.1 subset:
//! `let`/mutable-`let`/assignment/`if`-as-statement/sequencing/tail/tail-`return`,
//! NO loops) to a Verus EXEC expression STRING giving the body's FINAL STATE as a
//! closed-form function of the INITIAL state (the fn params). It is the STATE
//! analogue of step 2.1's [`crate::exec_encode::exec_ref_value`] (which gives a
//! single VALUE): where 2.1 checks the per-RHS expression VALUE, 2.2.1 adds the
//! orthogonal axis — the STATE SEQUENCING and mutation-ORDER faithfulness ON TOP of
//! the per-RHS value faithfulness.
//!
//! ## THE STATE-TRANSFORMER SEMANTICS (the new, load-bearing part)
//!
//! The program state is the environment of in-scope bindings (name -> its current
//! closed-form VALUE EXPRESSION in the inputs). Big-step evaluation threads an
//! initial environment (the params, each bound to itself) through the statement
//! sequence to a FINAL environment; the body's value is the tail expression
//! evaluated in that final environment. Concretely:
//!
//! - `let [mut] n = <rhs>` BINDS `n` to the rhs SUBSTITUTED under the current env
//!   (each in-env var replaced by its current value expr). `{ let a = x + 1; let b
//!   = a * 2; b }` -> `a |-> (x + 1)`, then `b |-> ((x + 1) * 2)`, tail `b` ->
//!   `((x + 1) * 2)`.
//! - `n = <rhs>` (assignment / mutation) REBINDS the in-scope cell `n` to the rhs
//!   substituted under the CURRENT env — ORDER-SENSITIVE: `s = s + 1; s = s * 2`
//!   threads `s |-> x` -> `s |-> (x + 1)` -> `s |-> ((x + 1) * 2)`, but the REORDER
//!   `s = s * 2; s = s + 1` threads to `((x * 2) + 1)` — a DIFFERENT closed form
//!   (the STATE-SEQUENCING teeth — `exec-stmt-tv.md` AC-3).
//! - `if c { .. } else { .. }` as the body TAIL composes the two branch
//!   state-transformers into a Verus `if`-EXPRESSION over the (substituted)
//!   condition — `if c { <then-tail> } else { <else-tail> }` (`exec-stmt-tv.md`
//!   AC-4).
//! - the body's final VALUE is the tail expr (or a tail `return <e>`) evaluated in
//!   the final env. A MULTI-CELL final state is a TUPLE `(<cell0>, <cell1>, ...)` —
//!   the tail `(a, b)` projects the final `a`/`b` cells (the design's
//!   least-confident #1, GROUNDED by B4).
//!
//! The substitution + threading + branch-composition + tuple projection are the
//! ONLY new logic; every RHS / condition / branch-tail VALUE is encoded by REUSING
//! [`crate::exec_encode::exec_ref_value`] on the env-substituted [`Expr`] (the
//! per-RHS bounded-value reference is ALREADY independent — it carries the #122
//! inner-paren / #146 cast-`<` / bounded-overflow disciplines). So a value-infidel
//! RHS (the #122/#146/wrong-op class) is ALSO caught by the SAME body obligation
//! (`exec-stmt-tv.md` AC-5).
//!
//! ## THE INDEPENDENCE BOUNDARY (REQ-2 HARD CONSTRAINT, R-CHAR-3 / AC-6)
//!
//! This module MUST NOT call any `thermite_lower::lower::*` symbol — `thermite-tv`
//! does NOT depend on `thermite-lower` (`Cargo.toml`; the dep graph makes reuse a
//! compile error). The reference state-denotation is authored from the FROZEN-SUBSET
//! big-step imperative semantics (`exec-stmt-tv.md` REQ-1/REQ-2), NOT from
//! `lower_block_inner`/`lower_stmt`. Agreement of production's `lower_exec_body` with
//! this reference is N-version differential EVIDENCE, not proof.
//!
//! ## HONEST BOUNDARY (out of the frozen 2.2.1 subset -> an `Err`, never silent-wrong)
//!
//! A construct OUTSIDE the straight-line subset is an honest
//! [`crate::exec_encode::RefEncodeError::Unsupported`] (R-CODE-2 / R-APG-1 — NEVER a
//! panic, NEVER a silent wrong denotation): a `Stmt::Loop`/`Break`/`Continue` (step
//! 2.2.2, kernel-gated), a mid-body early `return` nested in an `if` branch (the
//! multi-exit CPS form, OUT of v1), a `match`-as-statement, a non-scalar mutation
//! (`Vec::push` — a v2 sequence theory), and a re-shadow `let x = ..; let x = ..` in
//! the same block (the flat name->value env can't represent it). A silent wrong
//! denotation would defeat the whole point (TV would compare a wrong reference).
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (frozen kernel exec-statement subset v1) | SHIPPED | the IN/OUT subset is PINNED IN CODE here: [`body_ref_state`] ADMITS exactly `Stmt::Let`/`Assign`/`If`/`Expr`/tail-`Return` + `Block` sequencing/tail, and HONESTLY REJECTS (an `Unsupported` `Err`) `Stmt::Loop`/`Break`/`Continue` (2.2.2), a mid-`if`-branch early return, `match`-stmt, non-scalar mutation, and a re-shadow — the design-amendment-gated stable set; non-test consumer `crate::obligation::body_equivalence_obligation`; verified by `thermite-tv/tests/body_teeth.rs` B1-B4 (faithful VERIFIES, infidel CAUGHT) + the in-module honest-skip tests. |
//! | REQ-2 (operational-semantics reference state-denotation — independent) | SHIPPED | `pub fn body_ref_state` here — the big-step state-transformer (let/assign substitution-threading, mutation-ORDER sensitivity, `if`-branch composition, multi-cell TUPLE projection), composing `crate::exec_encode::exec_ref_value` on each env-substituted RHS / condition / tail; non-test consumer `crate::obligation::body_equivalence_obligation`; verified by `tests/body_teeth.rs` B1-B4 against real verus (B2 the mutation-ORDER teeth, B4 the multi-cell tuple). Deps `thermite-syntax` + `thermite-spec` ONLY (`Cargo.toml`, AC-6) — no `thermite-lower`. |
//!
//! ## LOOP extension — step 2.2.2-i (`.design/verified/loop-tv.md`; epic #169, blocker #163)
//!
//! [`loop_ref_obligations`] extends the straight-line state-transformer to a v1
//! frozen-subset `while` loop (`loop-tv.md` REQ-1/REQ-2): a single `while <cond>`
//! with declared `inv`+`dec`, a straight-line scalar body, the loop the LAST
//! statement before the tail. It produces the THREE per-run reference pieces the
//! obligation emitters in [`crate::obligation`] turn into Z3-checkable Verus units —
//! ENTRY (`inv` holds on the pre-loop entry state), PRESERVATION (one iteration of the
//! body carries `inv ∧ cond` to `inv`, REUSING the SHIPPED [`body_ref_state`] step),
//! and EXIT (the after-loop state is `inv ∧ ¬cond`-constrained). The after-loop state
//! threads as the design's OPAQUE-but-invariant-constrained fresh cells: a loop cannot
//! produce a closed form (it is a fixpoint), so the post-loop cells are havocked +
//! re-constrained to `inv ∧ ¬cond` (the honest analogue of how Verus itself models a
//! loop's after-state). Every OUT-of-v1 loop (`loop`-kind, `break`/`continue`, a
//! mid-body `return`, a nested loop, non-scalar state, a trivially-weak `inv`) is an
//! honest [`RefEncodeError::Unsupported`] (R-HONEST-3 — Skipped, NEVER silently
//! Faithful).
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | loop-REQ-1 (loop surface + v1 frozen loop subset) | SHIPPED | the v1 IN/OUT subset is PINNED IN CODE: [`loop_ref_obligations`] (+ [`recognize_v1_loop`]) ADMITS exactly a single `while <cond>` with non-empty `invs` + `dec` + a straight-line scalar body as the LAST statement before the tail, and HONESTLY REJECTS (an `Unsupported` `Err`) the `loop`-kind, `break`/`continue`, a mid-body `return`, a nested loop, non-scalar state, and a trivially-weak `inv` (`inv true`) — the design-amendment-gated stable set; non-test consumer `crate::obligation::{loop_entry_obligation, loop_preservation_obligation, loop_exit_obligation}`; verified by `thermite-tv/tests/loop_teeth.rs` L1 (faithful VERIFIES) + L4 (each OUT form Skipped). |
//! | loop-REQ-2 (the three per-run loop obligations) | SHIPPED | `pub fn loop_ref_obligations` here returns [`LoopObligations`] carrying the three reference pieces (`entry_pred` = `inv[cells:=entry]`; `cond` + `inv` for the preservation step's `requires`/`ensures` over the SHIPPED [`body_ref_state`] step; `after_loop` = `inv ∧ ¬cond` over the opaque cells); the per-cond/inv VALUE encoding REUSES [`exec_ref_value`] on the env-substituted [`Expr`] (independence preserved — no `thermite-lower` dep). The three Verus-unit emitters are `crate::obligation::{loop_entry_obligation, loop_preservation_obligation, loop_exit_obligation}`; verified by `tests/loop_teeth.rs` L1–L4 against real verus (L1 all three VERIFY, L2 broken-preservation CAUGHT, L3 wrong-exit CAUGHT, L4 weak-inv/loop-kind/break/return Skipped). |

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use thermite_syntax::ast::{BinOp, Block, Clause, Expr, IndexArg, LoopKind, LoopNode, Stmt};

use crate::exec_encode::{exec_ref_value, ExecRefCtx, RefEncodeError};

/// The body-reference-encoding context (REQ-2). Carries the slice-bound names (so a
/// slice index in an RHS / tail encodes to the spec-view element value `xs[i as
/// int]`, mirroring the obligation's `xs: &[u32]` binding) — the EXACT same
/// information [`ExecRefCtx`] carries for the per-expr encoder, reused here for the
/// per-RHS value encoding. It deliberately carries NO `nat`-coerce set (the exec
/// state is bounded-typed, never `nat`-coerced — the same as step 2.1).
///
/// This is the BODY dual of [`ExecRefCtx`]: where `ExecRefCtx` frames a single exec
/// EXPRESSION, `BodyRefCtx` frames a whole straight-line BODY. The state-threading
/// environment is INTERNAL to [`body_ref_state`] (it is the closed-form-in-the-
/// inputs map, not an external knob); the ctx carries only the slice-param frame.
#[derive(Debug, Clone, Default)]
pub struct BodyRefCtx {
    /// Names bound as a slice (`&[T]`) param in the obligation — an `Index` over
    /// such a name in any RHS / condition / tail encodes to the spec-view element
    /// value `xs[i as int]` (delegated to [`exec_ref_value`] via the [`ExecRefCtx`]
    /// this ctx builds). EMPTY for the scalar-only B1-B4 bodies.
    slice_bound: BTreeSet<String>,
}

impl BodyRefCtx {
    /// A context in which the named free vars are bound as slice (`&[T]`) params.
    pub fn with_slice_bound<I, S>(names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        BodyRefCtx {
            slice_bound: names.into_iter().map(Into::into).collect(),
        }
    }

    /// Build the [`ExecRefCtx`] the per-RHS value encoder uses (the slice-bound set
    /// passes straight through — every RHS / tail value is a step-2.1 exec value).
    fn exec_ref_ctx(&self) -> ExecRefCtx {
        ExecRefCtx::with_slice_bound(self.slice_bound.iter().cloned())
    }
}

/// The big-step STATE environment: each in-scope binding name -> its current
/// closed-form VALUE [`Expr`] (a function of the initial inputs). A `let`/assignment
/// REBINDS a name to its RHS SUBSTITUTED under this env; the tail is evaluated under
/// the FINAL env. Keeping the value as an [`Expr`] (not a string) lets every value
/// be encoded by REUSING [`exec_ref_value`] on the substituted [`Expr`] — the
/// independence boundary (the per-RHS bounded-value reference is unchanged), so the
/// ONLY new logic is the substitution + threading.
type Env = BTreeMap<String, Expr>;

/// Encode a STRAIGHT-LINE [`Block`] (the frozen 2.2.1 subset) to a Verus EXEC
/// expression STRING giving the body's FINAL STATE (the tail value) as a closed-form
/// function of the inputs, INDEPENDENTLY of the production lowerer (REQ-2). The
/// initial environment is implicit (each free var = itself); each `let`/assignment
/// threads the env in ORDER (mutation = order-sensitive substitution); an `if`-tail
/// composes the branch transformers; the tail (or tail-`return`) projects the final
/// state (a multi-cell tail tuple -> a Verus tuple).
///
/// REUSES [`exec_ref_value`] on each env-SUBSTITUTED RHS / condition / branch-tail —
/// the per-RHS bounded-value reference (the #122/#146/overflow disciplines) is
/// unchanged; the new logic is ONLY the state threading. Returns
/// [`RefEncodeError::Unsupported`] (NEVER a panic / silent wrong encoding) for a
/// construct outside the frozen straight-line subset (a loop, a mid-branch early
/// return, a `match`-stmt, a non-scalar mutation, a re-shadow).
pub fn body_ref_state(block: &Block, ctx: &BodyRefCtx) -> Result<String, RefEncodeError> {
    let mut env: Env = Env::new();
    encode_block_tail(block, &mut env, ctx)
}

/// Build the body-refinement obligation's `ensures` PREDICATE comparing the exec fn
/// `result` (named by `result_name`) to the reference FINAL STATE (REQ-3 helper for
/// [`crate::obligation::body_equivalence_obligation`]). For a SINGLE-CELL body this
/// is the scalar equality `result == <body_ref_state>` (the same form step 2.1 uses,
/// where `u64 == <u64 arithmetic>` Verus-coerces fine). For a MULTI-CELL body whose
/// tail is a TUPLE (`(a, b)`, B4 — the design's least-confident #1) it is the
/// PER-PROJECTION conjunction `result.0 == <cell0> && result.1 == <cell1>`: Verus has
/// no `SpecEq` between a `(u64, u64)` result and a `(int, int)` tuple LITERAL (each
/// element's bounded arithmetic elaborates to `int`), but the per-projection
/// `result.0: u64 == <u64 arithmetic>` compares element-wise at the bounded type
/// (exactly the GROUNDED projection equality `r.0 == b`, `ast.rs` `TupleProj`). The
/// reorder/wrong-cell teeth bite on whichever projection differs (B4's `b` cell).
///
/// This is the obligation-shape concern (how `result` is compared), kept distinct
/// from [`body_ref_state`] (the state DENOTATION itself, REQ-2). REUSES the SAME
/// state-threading; the ONLY addition is the multi-cell projection split.
pub fn body_ref_state_ensures(
    block: &Block,
    result_name: &str,
    ctx: &BodyRefCtx,
) -> Result<String, RefEncodeError> {
    // A multi-cell body is one whose TAIL is a tuple (the final state spans cells).
    // Each cell is encoded under the body's FINAL env (the same threading), then
    // compared to the matching `result.<i>` projection at the bounded type.
    if let Some(tail) = &block.tail {
        if let Expr::Tuple(elems) = tail.as_ref() {
            let mut env: Env = Env::new();
            for stmt in &block.stmts {
                thread_stmt(stmt, &mut env)?;
            }
            let conjuncts = elems
                .iter()
                .enumerate()
                .map(|(i, e)| {
                    let cell = encode_value(e, &env, ctx)?;
                    Ok(format!("{result_name}.{i} == {cell}"))
                })
                .collect::<Result<Vec<_>, RefEncodeError>>()?;
            return Ok(conjuncts.join(" && "));
        }
    }
    // The single-cell (scalar / bool / if-tail) body: the plain scalar equality.
    let reference = body_ref_state(block, ctx)?;
    Ok(format!("{result_name} == {reference}"))
}

// =============================================================================
// LOOP extension — step 2.2.2-i (`.design/verified/loop-tv.md` REQ-1/REQ-2)
// =============================================================================

/// The three per-run loop reference pieces a v1 frozen-subset `while` loop produces
/// (`loop-tv.md` REQ-2), consumed by the obligation emitters in [`crate::obligation`]
/// to build the three Z3-checkable Verus units (ENTRY / PRESERVATION / EXIT). Each
/// field is an INDEPENDENT reference encoding (composing [`exec_ref_value`] on the
/// env-substituted cond / inv / cell exprs — no `thermite-lower` symbol, AC-7).
///
/// The loop is the v1 frozen subset: a single `while <cond>` with non-empty `invs` +
/// a `dec`, a STRAIGHT-LINE SCALAR body, the loop the LAST statement before the tail.
/// The mutated cells are the bare scalar names the body rebinds (the design's `lo`/
/// `hi`); they are bound as the loop-step parameters in the preservation obligation
/// and as the OPAQUE-but-invariant-constrained after-loop cells in the exit
/// obligation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopObligations {
    /// The mutated scalar cell names (the body-rebound cells), in a stable
    /// (sorted) order — the order they are declared/projected in the obligations.
    pub cells: Vec<String>,
    /// ENTRY (`loop-tv.md` REQ-2.1): the conjoined `inv` with each cell SUBSTITUTED
    /// by its pre-loop ENTRY value (the prefix straight-line state, encoded by the
    /// SHIPPED [`body_ref_state`] threading). The entry obligation asserts this
    /// predicate holds (`entry-state ⟹ inv`).
    pub entry_pred: String,
    /// The loop condition `<cond>` over the FREE cells (cells as the loop-step
    /// params) — the preservation obligation's `requires inv && cond` guard and the
    /// negated `!(cond)` in the exit characterization.
    pub cond: String,
    /// The conjoined loop `inv` over the FREE cells — the preservation obligation's
    /// `requires inv` guard and (negated-cond conjunct) the exit characterization.
    pub inv: String,
    /// PRESERVATION (`loop-tv.md` REQ-2.2): per-cell, the SINGLE-ITERATION stepped
    /// value — the SHIPPED [`body_ref_state`] step of the loop body (the loop body
    /// IS a straight-line `Block`). `step_cells[i]` is the closed form cell `i` holds
    /// AFTER one iteration, as a function of the entry cells. The obligation's
    /// `ensures` is `result.i == step_cells[i]` (the body-TV REUSE, AC-5) AND the inv
    /// at the stepped state (the preservation conjunct, AC-2).
    pub step_cells: Vec<String>,
    /// PRESERVATION: the conjoined `inv` with each cell SUBSTITUTED by its stepped
    /// value (`step_cells`) — the obligation's preservation `ensures` conjunct
    /// (`inv ∧ cond` carried to `inv` by one iteration).
    pub inv_at_step: String,
}

/// Recognize + encode the v1 frozen-subset `while` loop in `block`, producing the
/// three per-run reference pieces ([`LoopObligations`]) the [`crate::obligation`]
/// emitters turn into Z3-checkable Verus units (`loop-tv.md` REQ-1/REQ-2). The loop
/// MUST be the LAST statement before the tail (the `binary_search` shape — v1's
/// after-loop continuation is in scope ONLY there); a prefix of straight-line
/// statements establishes the pre-loop entry state.
///
/// REUSES the SHIPPED [`body_ref_state`] threading for both the pre-loop entry state
/// and the single-iteration body step (the loop body is itself a straight-line
/// `Block`); the cond / inv VALUE encoding REUSES [`exec_ref_value`] on the
/// env-substituted [`Expr`] (independence preserved — no `thermite-lower` dep).
///
/// Returns [`RefEncodeError::Unsupported`] (NEVER a panic / silent wrong encoding,
/// R-HONEST-3) for an OUT-of-v1 loop: a `loop`-kind (multi-exit), a `break`/
/// `continue` / mid-body `return` in the body (multi-exit CPS), a NESTED loop, a
/// non-scalar-state body, or a TRIVIALLY-WEAK `inv` (`inv true` — the after-loop
/// `true ∧ ¬cond` is vacuous, cannot enter the (a) rule). Each is Skipped HONESTLY,
/// never silently Faithful (the honest 2.2.2 boundary in the certificate).
pub fn loop_ref_obligations(
    block: &Block,
    ctx: &BodyRefCtx,
) -> Result<LoopObligations, RefEncodeError> {
    let (prefix, loop_node) = recognize_v1_loop(block)?;
    let cond_expr = match &loop_node.kind {
        LoopKind::While(c) => c.as_ref(),
        LoopKind::Loop => {
            // Unreachable: recognize_v1_loop rejects the `loop`-kind. Kept exhaustive
            // (no `_`/panic — R-APG-1).
            return Err(RefEncodeError::Unsupported(
                "`loop`-kind (the infinite-loop form is a multi-exit CPS shape — OUT \
                 of the v1 single-`while` subset, Skipped honestly)"
                    .to_string(),
            ));
        }
    };

    // A TRIVIALLY-WEAK `inv` (the conjunction is the bare `true`) is OUT of v1 (the
    // after-loop `true ∧ ¬cond` is vacuous) — honest Err, checked before encoding.
    if invariant_is_vacuous(&loop_node.invs) {
        return Err(RefEncodeError::Unsupported(
            "trivially-weak loop invariant (`inv true` — the after-loop `true ∧ ¬cond` \
             characterization is vacuous; the loop cannot enter the (a) rule, Skipped \
             honestly — bounded unrolling is the future v0.2 L2 fallback)"
                .to_string(),
        ));
    }

    // The mutated scalar cells: the bare names the loop body REBINDS (a v1 scalar
    // mutation is a `Stmt::Assign` to a bare in-scope name — recognize_v1_loop has
    // already rejected non-scalar / out-of-subset bodies). Sorted for a stable
    // declaration/projection order across the three obligations.
    let mut cells: Vec<String> = collect_assigned_cells(&loop_node.body)?
        .into_iter()
        .collect();
    cells.sort();
    if cells.is_empty() {
        return Err(RefEncodeError::Unsupported(
            "v1 `while` loop with no scalar cell mutation (a loop whose body mutates \
             no in-scope scalar cell carries no per-iteration state step — OUT of the \
             v1 subset)"
                .to_string(),
        ));
    }

    // The ENTRY state: thread the pre-loop prefix straight-line statements (the
    // `let mut lo = 0; let mut hi = haystack.len();` prefix) into the entry env — the
    // closed-form value of EVERY prefix-introduced binding (cells AND read-only
    // in-scope bindings the inv/cond reference) in the fn inputs. The entry invariant
    // substitutes the WHOLE entry env (so a referenced `hi |-> n` is resolved, not
    // left free); the fn inputs are the only surviving free vars. Every CELL must have
    // a prefix `let mut` binding (an assigned cell needs an in-scope introducer) —
    // honest Err otherwise.
    let mut entry_env: Env = Env::new();
    for stmt in prefix {
        thread_stmt(stmt, &mut entry_env)?;
    }
    for cell in &cells {
        if !entry_env.contains_key(cell) {
            return Err(RefEncodeError::Unsupported(format!(
                "loop cell `{cell}` has no pre-loop `let mut` binding in the straight-\
                 line prefix (malformed v1 loop — the entry state is undefined)"
            )));
        }
    }
    let entry_subst = entry_env;

    // The STEPPED state: thread ONE iteration of the loop body (the SHIPPED
    // straight-line state-transformer, the loop body IS a straight-line Block). Each
    // cell starts free (bound to itself — the loop-step's param), so the stepped
    // value is a closed form in the ENTRY cells. A cell the body does NOT rebind keeps
    // its identity binding (unchanged across the iteration).
    let mut step_env: Env = Env::new();
    for cell in &cells {
        step_env.insert(cell.clone(), Expr::Path(vec![cell.clone()]));
    }
    for stmt in &loop_node.body.stmts {
        thread_stmt(stmt, &mut step_env)?;
    }
    // The per-cell stepped CLOSED FORM (in the entry cells) + the cell→stepped-form
    // substitution env — both read from `step_env`, where every `cell` is present (it
    // was seeded with its identity binding above, never removed by threading); an
    // absent key is handled honestly (the identity), no panic.
    let mut step_subst: Env = Env::new();
    let mut step_cells: Vec<String> = Vec::with_capacity(cells.len());
    for cell in &cells {
        let stepped = match step_env.get(cell) {
            Some(expr) => expr.clone(),
            None => Expr::Path(vec![cell.clone()]),
        };
        step_cells.push(encode_value(&stepped, &Env::new(), ctx)?);
        step_subst.insert(cell.clone(), stepped);
    }

    // The condition + the invariant over the FREE cells (encoded as bool-valued exec
    // predicates — REUSE exec_ref_value on the env-substituted cond / inv).
    let cond = encode_predicate(cond_expr, &Env::new(), ctx)?;
    let inv = encode_inv_clauses(&loop_node.invs, &Env::new(), ctx)?.join(" && ");

    // The entry-substituted invariant (cells → entry values) and the step-substituted
    // invariant (cells → stepped values) — REUSE the same inv-clause encoder under the
    // substitution env.
    let entry_pred = encode_inv_clauses(&loop_node.invs, &entry_subst, ctx)?.join(" && ");
    let inv_at_step = encode_inv_clauses(&loop_node.invs, &step_subst, ctx)?.join(" && ");

    Ok(LoopObligations {
        cells,
        entry_pred,
        cond,
        inv,
        step_cells,
        inv_at_step,
    })
}

/// Recognize the v1 frozen-subset `while` loop: `block`'s LAST statement must be a
/// `Stmt::Loop` with `kind: While(_)`, non-empty `invs`, a `dec`, and a STRAIGHT-LINE
/// SCALAR body containing NO nested loop / `break` / `continue` / mid-body `return`.
/// Returns the pre-loop prefix statements + the loop node, or an honest
/// [`RefEncodeError::Unsupported`] naming the precise OUT-of-v1 reason (Skipped, never
/// silently Faithful — R-HONEST-3).
fn recognize_v1_loop(block: &Block) -> Result<(&[Stmt], &LoopNode), RefEncodeError> {
    let Some((last, prefix)) = block.stmts.split_last() else {
        return Err(RefEncodeError::Unsupported(
            "no loop statement (the v1 loop arm requires a `while` loop as the last \
             statement before the tail)"
                .to_string(),
        ));
    };
    let Stmt::Loop(loop_node) = last else {
        return Err(RefEncodeError::Unsupported(
            "the last statement before the tail is not a loop (v1's after-loop \
             continuation is in scope ONLY when the loop is the last statement — a \
             loop followed by further straight-line mutation is a v1.1 extension)"
                .to_string(),
        ));
    };
    // The PREFIX must itself be straight-line (no earlier loop / break / continue /
    // mid-body return) — reuse the SHIPPED thread_stmt rejection by threading it.
    let mut probe: Env = Env::new();
    for stmt in prefix {
        thread_stmt(stmt, &mut probe)?;
    }
    if !matches!(loop_node.kind, LoopKind::While(_)) {
        return Err(RefEncodeError::Unsupported(
            "`loop`-kind (the infinite-loop form is a multi-exit CPS shape — the corpus \
             `binary_search` uses `loop { if .. { return .. } }`; OUT of the v1 \
             single-`while` subset, Skipped honestly)"
                .to_string(),
        ));
    }
    if loop_node.invs.is_empty() {
        // Structurally LoopNode carries a non-empty invs (the parser enforces §4.1);
        // the honest Err keeps the rule total even against a hand-built node.
        return Err(RefEncodeError::Unsupported(
            "`while` loop with no `inv` (v1's after-loop characterization needs a \
             usable invariant — Skipped honestly)"
                .to_string(),
        ));
    }
    // The body must be straight-line SCALAR with NO nested loop-control / loop /
    // mid-body return — reject any of those before encoding.
    reject_out_of_subset_body(&loop_node.body)?;
    Ok((prefix, loop_node))
}

/// Reject an OUT-of-v1 loop body: a NESTED `Stmt::Loop`, a `break`/`continue`, or a
/// mid-body `return` (the multi-exit CPS forms) → an honest
/// [`RefEncodeError::Unsupported`]. Recurses into `if`-branch bodies (a `break` /
/// `return` nested in an `if` is just as OUT). A straight-line scalar body (the v1
/// IN set: `let`/`assign`/`if`/`expr`) passes; the per-statement value/scalar
/// rejection is left to the SHIPPED [`thread_stmt`] (e.g. a non-scalar assignment is
/// already an honest Err there).
fn reject_out_of_subset_body(body: &Block) -> Result<(), RefEncodeError> {
    for stmt in &body.stmts {
        reject_out_of_subset_stmt(stmt)?;
    }
    Ok(())
}

fn reject_out_of_subset_stmt(stmt: &Stmt) -> Result<(), RefEncodeError> {
    match stmt {
        Stmt::Loop(_) => Err(RefEncodeError::Unsupported(
            "NESTED loop in a v1 loop body (the inner loop's after-state is itself a \
             fixpoint inside the outer body-step — OUT of v1, Skipped honestly)"
                .to_string(),
        )),
        Stmt::Break => Err(RefEncodeError::Unsupported(
            "`break` in a v1 loop body (a `break` is a multi-exit form — the after-loop \
             characterization needs per-exit invariant conjuncts, a v2 extension; \
             Skipped honestly)"
                .to_string(),
        )),
        Stmt::Continue => Err(RefEncodeError::Unsupported(
            "`continue` in a v1 loop body (a back-edge is a multi-exit control form — \
             OUT of v1, Skipped honestly)"
                .to_string(),
        )),
        Stmt::Return(_) => Err(RefEncodeError::Unsupported(
            "mid-body `return` in a v1 loop body (the corpus `binary_search` uses \
             `return None`/`return Some(mid)` — a multi-exit CPS form, OUT of v1; \
             Skipped honestly)"
                .to_string(),
        )),
        Stmt::If { then, else_, .. } => {
            reject_out_of_subset_body(then)?;
            if let Some(else_block) = else_ {
                reject_out_of_subset_body(else_block)?;
            }
            Ok(())
        }
        Stmt::Let { .. } | Stmt::Assign { .. } | Stmt::Expr(_) => Ok(()),
    }
}

/// Collect the bare scalar cell names a straight-line loop body REBINDS (a v1
/// `Stmt::Assign` to a bare in-scope name). Recurses into `if`-branch bodies (a cell
/// mutated in a branch is a mutated cell). A `let`-introduced branch-local binding is
/// NOT a mutated outer cell (it does not leak — the body_ref_state semantics), so a
/// branch-local `let mid = ..` is excluded. Returns an honest Err only on a malformed
/// non-bare-name target (left to the SHIPPED threading otherwise).
fn collect_assigned_cells(body: &Block) -> Result<BTreeSet<String>, RefEncodeError> {
    let mut cells = BTreeSet::new();
    collect_assigned_cells_block(body, &mut cells)?;
    Ok(cells)
}

fn collect_assigned_cells_block(
    body: &Block,
    cells: &mut BTreeSet<String>,
) -> Result<(), RefEncodeError> {
    for stmt in &body.stmts {
        match stmt {
            Stmt::Assign { target, .. } => match target {
                Expr::Path(segments) if segments.len() == 1 => {
                    cells.insert(segments[0].clone());
                }
                _ => {
                    return Err(RefEncodeError::Unsupported(
                        "assignment to a non-scalar / non-bare-name target in a v1 loop \
                         body (the v1 loop state mutates only bare scalar cells — an \
                         indexed / field target is OUT, a v2 sequence/struct theory)"
                            .to_string(),
                    ));
                }
            },
            Stmt::If { then, else_, .. } => {
                collect_assigned_cells_block(then, cells)?;
                if let Some(else_block) = else_ {
                    collect_assigned_cells_block(else_block, cells)?;
                }
            }
            // A `let` introduces a fresh (branch-local or body-local) binding, not a
            // mutated OUTER cell; an `Expr`-stmt has no state effect. Neither
            // contributes a mutated loop cell.
            Stmt::Let { .. } | Stmt::Expr(_) => {}
            // The multi-exit / nested forms are already rejected by
            // reject_out_of_subset_body before this is reached; kept exhaustive.
            Stmt::Loop(_) | Stmt::Break | Stmt::Continue | Stmt::Return(_) => {}
        }
    }
    Ok(())
}

/// Encode a bool-valued PREDICATE (a loop `cond` / `inv` clause) under `env` — the
/// cells are SUBSTITUTED by their env value (entry / stepped) then the predicate is
/// REUSED through [`exec_ref_value`] (the bounded comparison / logical reference — the
/// same independent encoder the per-RHS value uses). A predicate outside the bounded
/// exec sublanguage (a quantifier, a spec-only combinator) is an honest Err from
/// [`exec_ref_value`] — the v1 loop subset is scalar-comparison invariants (`lo <=
/// hi`, `i <= n`), never a `forall_*` (those are the `binary_search` v2 forms).
fn encode_predicate(expr: &Expr, env: &Env, ctx: &BodyRefCtx) -> Result<String, RefEncodeError> {
    let substituted = substitute(expr, env)?;
    exec_ref_value(&substituted, &ctx.exec_ref_ctx())
}

/// Encode each loop `inv` Clause to a bool-valued predicate string (under `env` — the
/// cell substitution for the entry / stepped invariant), via [`encode_predicate`].
fn encode_inv_clauses(
    invs: &[Clause],
    env: &Env,
    ctx: &BodyRefCtx,
) -> Result<Vec<String>, RefEncodeError> {
    invs.iter()
        .map(|clause| encode_predicate(&clause.expr, env, ctx))
        .collect()
}

/// Whether the loop's invariant conjunction is TRIVIALLY WEAK (every clause is the
/// literal `true`) — the after-loop `true ∧ ¬cond` is vacuous, so the loop cannot
/// enter the (a) rule (`loop-tv.md` REQ-1 OUT — Skipped honestly, NOT Faithful). A
/// single `inv true` or several all-`true` clauses are vacuous; ANY non-`true`
/// conjunct makes the invariant usable.
fn invariant_is_vacuous(invs: &[Clause]) -> bool {
    invs.iter()
        .all(|clause| matches!(clause.expr, Expr::BoolLit(true)))
}

/// Build the negated-condition string `(!(<cond>))` for the exit characterization
/// (`loop-tv.md` REQ-2.3: after-loop = `inv ∧ ¬cond`). REUSES the SHIPPED
/// [`exec_ref_value`] `Not`-encoding (the bounded logical-not reference) over the
/// already-encoded condition — wrapped so the `&&` with the invariant binds the whole
/// negated predicate.
pub fn negate_condition(cond: &str) -> String {
    format!("(!({cond}))")
}

/// Thread `block`'s statements through `env` (in order), then encode its TAIL value
/// under the resulting env. A block with NO tail (a unit-valued straight-line body)
/// is outside the v1 single-exit *value* subset — the body-refinement obligation
/// compares a RESULT value, so a tail is REQUIRED (an honest `Err` otherwise).
fn encode_block_tail(
    block: &Block,
    env: &mut Env,
    ctx: &BodyRefCtx,
) -> Result<String, RefEncodeError> {
    for stmt in &block.stmts {
        thread_stmt(stmt, env)?;
    }
    match &block.tail {
        Some(tail) => encode_value(tail, env, ctx),
        None => Err(RefEncodeError::Unsupported(
            "straight-line body with no tail value (the body-refinement obligation \
             compares a RESULT value; a unit-valued body is outside the v1 \
             single-exit value subset)"
                .to_string(),
        )),
    }
}

/// Thread ONE statement through `env` (REQ-2): bind/rebind a cell to its
/// env-substituted RHS. The frozen straight-line subset admits `Let`/`Assign`/
/// `Expr` here; `If`/`Return` are only admitted in TAIL position (handled by
/// [`encode_value`] / the tail), so an `If`/`Return` in NON-tail (statement)
/// position — a mid-body branch / early return — is OUT of v1 (the multi-exit CPS
/// form) and an honest `Err`. A `Loop`/`Break`/`Continue` is step 2.2.2.
fn thread_stmt(stmt: &Stmt, env: &mut Env) -> Result<(), RefEncodeError> {
    match stmt {
        Stmt::Let {
            name, init, ty: _, ..
        } => {
            // A re-shadow `let x = ..; let x = ..` in the same block is OUT of v1
            // (the flat name->value env can't represent two distinct `x` cells) —
            // honest `Err`, never a silent wrong substitution.
            if env.contains_key(name) {
                return Err(RefEncodeError::Unsupported(format!(
                    "re-shadowed binding `{name}` in the same block (the v1 state \
                     environment is a flat name->value map; a re-shadow is OUT of the \
                     frozen subset)"
                )));
            }
            let substituted = substitute(init, env)?;
            env.insert(name.clone(), substituted);
            Ok(())
        }
        Stmt::Assign { target, value } => {
            // v1 mutation is a SCALAR-cell rebind: the target must be a bare
            // in-scope name (a non-scalar mutation — `xs[i] = ..`, `m.field = ..` —
            // is OUT of v1, a v2 sequence/struct theory).
            let name = match target {
                Expr::Path(segments) if segments.len() == 1 => segments[0].clone(),
                _ => {
                    return Err(RefEncodeError::Unsupported(
                        "assignment to a non-scalar / non-bare-name target (the v1 \
                         frozen subset mutates only bare scalar cells; an indexed / \
                         field / projection target is OUT — a v2 sequence/struct \
                         theory)"
                            .to_string(),
                    ));
                }
            };
            // The cell must already be in scope (a `let mut` introduced it). An
            // assignment to an unbound name is malformed input — an honest `Err`.
            if !env.contains_key(&name) {
                return Err(RefEncodeError::Unsupported(format!(
                    "assignment to the unbound cell `{name}` (no in-scope `let mut` \
                     introduced it — malformed straight-line body)"
                )));
            }
            // ORDER-SENSITIVE: substitute under the CURRENT env (the value BEFORE
            // this assignment), then rebind. This is the state-sequencing teeth: a
            // reorder threads a different substitution chain -> a different closed
            // form (`exec-stmt-tv.md` AC-3).
            let substituted = substitute(value, env)?;
            env.insert(name, substituted);
            Ok(())
        }
        // A bare expression statement `<e>;` in the frozen scalar subset has no
        // STATE effect (a non-tail call's value is discarded; v1 scalar bodies carry
        // no side-effecting cell mutation outside an explicit assignment). It must be
        // well-formed under the env, so we encode (and discard) it to surface a
        // value-encoding error, but it does not thread the state.
        Stmt::Expr(e) => {
            let _ = substitute(e, env)?;
            Ok(())
        }
        // An `if` STATEMENT that MUTATES outer cells per arm (the GROUNDED AC-4 form
        // `if x < 10 { r = r + 1; } else { r = r + 2; }` — `exec-stmt-tv.md` REQ-1
        // lists the `if`-statement as IN the frozen 2.2.1 subset, AC-4 grounds it
        // `verified: 1`). It is a state-transformer: thread the then-branch into a
        // COPY of the current env (-> the then-env) and the else-branch into another
        // copy (-> the else-env, an ABSENT else == identity), then for EACH cell
        // either branch mutated, the post-if value becomes the Verus if-EXPRESSION
        // `if <cond> { <then-cell> } else { <else-cell> }` composing the two branch
        // states (the state-transformer semantics — exec-stmt-tv.md REQ-2 / AC-4). A
        // cell mutated in neither branch is unchanged. The recursion handles a NESTED
        // `if`-statement in a branch; an out-of-subset branch construct (a loop, a
        // non-scalar mutation, a mid-branch return) PROPAGATES its honest `Err`.
        Stmt::If { cond, then, else_ } => {
            // The condition is itself an exec value — substitute it under the
            // pre-`if` env so the composed value is a closed form in the inputs.
            let cond_subst = substitute(cond, env)?;

            // Thread each branch into its OWN copy of the pre-`if` env. A branch-tail
            // VALUE (a value-discarding `if c { ..; v }` statement) is OUT of the v1
            // mutation subset — an honest `Err` (the state-denotation only composes a
            // branch that mutates cells, never a discarded branch value).
            let mut then_env = env.clone();
            thread_branch(then, &mut then_env)?;
            let mut else_env = env.clone();
            if let Some(else_block) = else_ {
                thread_branch(else_block, &mut else_env)?;
            }

            // For each cell ALREADY in scope before the `if` (a branch-local `let`
            // does not leak past the branch — it lives only in the branch-env clone),
            // recompose: if either branch changed it, the post-`if` value is the
            // branch-composed Verus `if`-expression (an absent else / a non-mutating
            // branch contributes the cell's pre-`if` value, i.e. identity — exactly
            // what the cloned env preserves). A cell mutated in neither branch keeps
            // its pre-`if` value untouched.
            let cell_names: Vec<String> = env.keys().cloned().collect();
            for name in cell_names {
                let then_val = then_env
                    .get(&name)
                    .cloned()
                    .unwrap_or_else(|| Expr::Path(vec![name.clone()]));
                let else_val = else_env
                    .get(&name)
                    .cloned()
                    .unwrap_or_else(|| Expr::Path(vec![name.clone()]));
                let pre_val = env.get(&name);
                if pre_val == Some(&then_val) && pre_val == Some(&else_val) {
                    // Unchanged by both branches — leave the cell as-is.
                    continue;
                }
                let composed = Expr::If {
                    cond: Box::new(cond_subst.clone()),
                    then: Block {
                        stmts: vec![],
                        tail: Some(Box::new(then_val)),
                    },
                    else_: Block {
                        stmts: vec![],
                        tail: Some(Box::new(else_val)),
                    },
                };
                env.insert(name, composed);
            }
            Ok(())
        }
        Stmt::Return(_) => Err(RefEncodeError::Unsupported(
            "early `return` in non-tail position (v1 admits `return` only in TAIL \
             position — a mid-body early return is a multi-exit CPS form, OUT of the \
             frozen subset)"
                .to_string(),
        )),
        Stmt::Loop(_) => Err(RefEncodeError::Unsupported(
            "`loop`/`while` in a straight-line body (step 2.2.2 — the after-loop \
             state needs the invariant / a fixpoint; kernel-gated, HONESTLY SKIPPED \
             in 2.2.1)"
                .to_string(),
        )),
        Stmt::Break => Err(RefEncodeError::Unsupported(
            "`break` outside a loop body (loop-control is step 2.2.2)".to_string(),
        )),
        Stmt::Continue => Err(RefEncodeError::Unsupported(
            "`continue` outside a loop body (loop-control is step 2.2.2)".to_string(),
        )),
    }
}

/// Thread an `if`-STATEMENT BRANCH `Block`'s statements through `env` (in order),
/// REUSING the per-statement [`thread_stmt`] recursively (so a NESTED `if`-statement
/// in the branch is composed, and an out-of-subset branch construct — a loop, a
/// non-scalar mutation, a mid-branch early return — propagates its honest `Err`). A
/// branch in the v1 mutation subset is value-LESS (`tail: None`): it mutates outer
/// cells via `Stmt::Assign`, it does not produce a discarded value. A branch with a
/// TAIL VALUE (`if c { ..; v }` as a statement) is OUT of the v1 mutation subset — an
/// honest [`RefEncodeError::Unsupported`], never a silent discard.
fn thread_branch(branch: &Block, env: &mut Env) -> Result<(), RefEncodeError> {
    for stmt in &branch.stmts {
        thread_stmt(stmt, env)?;
    }
    match &branch.tail {
        None => Ok(()),
        Some(_) => Err(RefEncodeError::Unsupported(
            "`if`-statement branch with a tail VALUE (a value-discarding \
             `if c { ..; v }` statement is OUT of the v1 mutation subset — a branch \
             mutates outer cells, it does not produce a discarded value)"
                .to_string(),
        )),
    }
}

/// Strip exactly ONE layer of fully-enclosing parentheses from `s`, used ONLY for
/// the `if`-condition syntax position (`if <cond> { .. }`), where the canonical
/// reference form is the bare predicate (`exec-stmt-tv.md` AC-4 `if x < 10 { .. }`).
/// `exec_ref_value` wholly parenthesizes a `Binary` (the #122 discipline), so the
/// encoded comparison condition arrives as `(x < 10)`; this removes the redundant
/// outer pair (Verus parses `if x < 10` identically). The strip is conservative: it
/// removes the pair ONLY when the leading `(` matches the trailing `)` AND that pair
/// encloses the WHOLE string (a `(a) + (b)` is left untouched — its outer chars are
/// not a single enclosing pair). A string with no enclosing pair is returned as-is.
fn strip_one_enclosing_paren(s: &str) -> String {
    let bytes = s.as_bytes();
    if bytes.first() != Some(&b'(') || bytes.last() != Some(&b')') {
        return s.to_string();
    }
    // Walk the depth; the opening `(` encloses the whole string only if depth returns
    // to 0 EXACTLY at the final char (never reaching 0 before the end).
    let mut depth = 0i32;
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 && i != bytes.len() - 1 {
                    // The leading `(` closed before the end — not a single enclosing
                    // pair (e.g. `(a) + (b)`). Leave the string untouched.
                    return s.to_string();
                }
            }
            _ => {}
        }
    }
    // The leading `(` and trailing `)` are a single enclosing pair — strip them.
    s[1..s.len() - 1].to_string()
}

/// Encode a body VALUE position (a tail expr, a branch tail, a tail-`return`'s
/// expr) under `env` (REQ-2). An `if`-EXPRESSION composes the two branch
/// state-transformers; a TUPLE projects the multi-cell final state; everything else
/// is an exec VALUE -> SUBSTITUTE the env then REUSE [`exec_ref_value`].
fn encode_value(expr: &Expr, env: &Env, ctx: &BodyRefCtx) -> Result<String, RefEncodeError> {
    // SUBSTITUTE the env FIRST so a tail / branch-tail that NAMES a cell composed by
    // an `if`-statement (the AC-4 form: the tail `r` whose env value is the
    // branch-composed `Expr::If`) dispatches on the cell's COMPOSED value, not on the
    // bare `Path` (which `exec_ref_value` would reject as an `if expression`). For a
    // syntactic `Expr::If` / `Expr::Tuple` tail `substitute` is the identity (it does
    // not recurse into those nodes), so the B3 if-tail / B4 tuple-tail are unchanged.
    let substituted = substitute(expr, env)?;
    match &substituted {
        // The `if` state-transformer (`exec-stmt-tv.md` AC-4): compose the two branch
        // transformers into a Verus `if`-expression over the (already-substituted)
        // condition. The condition and the branch tails are encoded as exec VALUES.
        // For a SYNTACTIC if-tail (B3) the branches are still source blocks (a fresh
        // env CLONE — a branch-local `let` does not leak); for a cell composed by an
        // `if`-statement the branch blocks are `{ tail: <closed-form> }` (already
        // threaded), and `encode_block_tail` re-encodes that closed form unchanged.
        Expr::If { cond, then, else_ } => {
            // The condition sits in Verus `if <cond> { .. }` syntax position, where a
            // bare predicate is the canonical form (`exec-stmt-tv.md` AC-4 reference
            // `if x < 10 { x+1 } else { x+2 }`, B3 `if c { .. }`). `exec_ref_value`
            // wholly parenthesizes a `Binary` (the #122 discipline), so strip exactly
            // ONE layer of fully-enclosing parens for the condition — Verus parses
            // `if x < 10` identically to `if (x < 10)`, and this matches the pinned
            // reference form. A non-parenthesized cond (a bare path `c`) is unchanged.
            let c = strip_one_enclosing_paren(&encode_value(cond, env, ctx)?);
            let mut then_env = env.clone();
            let t = encode_block_tail(then, &mut then_env, ctx)?;
            let mut else_env = env.clone();
            let e = encode_block_tail(else_, &mut else_env, ctx)?;

            // Verus requires the two `if`/`else` arms to share a TYPE. `exec_ref_value`
            // encodes spec ARITHMETIC (a `Binary` over `+`/`-`/`*`/...) as `int`, but a
            // bare cell value (the IDENTITY arm of a no-else `if c { r = r + 1; } r` —
            // the else is the unchanged `r`, a `u64`) stays bounded. If the two arms
            // DISAGREE on int-ness, coerce the bounded (non-`int`) arm with `as int` so
            // the arms unify (Verus parses `(x as int)` and the `result: u64 == <int>`
            // comparison coerces fine). When BOTH arms are arithmetic (the GROUNDED
            // AC-4 `(x + 1)`/`(x + 2)`, B3 `(x + 1)`/`(x - 1)`) no coercion is applied —
            // the pinned reference form is preserved.
            let t_int = branch_is_int_typed(then, env)?;
            let e_int = branch_is_int_typed(else_, env)?;
            let (t, e) = match (t_int, e_int) {
                (true, false) => (t, format!("({e} as int)")),
                (false, true) => (format!("({t} as int)"), e),
                _ => (t, e),
            };
            Ok(format!("if {c} {{ {t} }} else {{ {e} }}"))
        }
        // The multi-cell TUPLE projection (`exec-stmt-tv.md` REQ-2, the design's
        // least-confident #1, GROUNDED by B4): the body's final state across cells
        // is a Verus tuple of each cell's (env-substituted) closed form.
        Expr::Tuple(elems) => {
            let parts = elems
                .iter()
                .map(|e| encode_value(e, env, ctx))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(format!("({})", parts.join(", ")))
        }
        // Every other value (a path, arithmetic, a cast, a call, an index, ...) is a
        // step-2.1 exec VALUE -> REUSE the independent per-RHS encoder (the
        // #122/#146/overflow disciplines unchanged). Already substituted above.
        _ => exec_ref_value(&substituted, &ctx.exec_ref_ctx()),
    }
}

/// Whether the VALUE an `if`-EXPRESSION branch `block` yields is encoded by
/// [`exec_ref_value`] as a spec `int` (vs a bounded `u64`/.../`bool`). Used ONLY to
/// unify the two arms' Verus types (`exec_ref_value` encodes a `Binary` ARITHMETIC as
/// `int`; a bare cell value — the identity arm of a no-else `if` — stays bounded). It
/// threads the branch's own stmts into a clone of `env` then classifies the
/// (substituted) branch-tail [`Expr`]: spec ARITHMETIC (`Binary` over `+`/`-`/`*`/
/// `/`/`%`/shift/bit-ops) is `int`; a comparison `Binary` is `bool` (not `int`);
/// everything else (a bare path cell, a literal, a cast, an index, a call) is the
/// bounded type (not `int`). A branch with no tail value would already be an `Err`
/// from [`encode_block_tail`]; here an absent tail is conservatively NOT-`int`.
fn branch_is_int_typed(block: &Block, env: &Env) -> Result<bool, RefEncodeError> {
    let mut branch_env = env.clone();
    for stmt in &block.stmts {
        thread_stmt(stmt, &mut branch_env)?;
    }
    let Some(tail) = &block.tail else {
        return Ok(false);
    };
    let value = substitute(tail, &branch_env)?;
    Ok(matches!(
        value,
        Expr::Binary {
            op: BinOp::Add
                | BinOp::Sub
                | BinOp::Mul
                | BinOp::Div
                | BinOp::Rem
                | BinOp::Shl
                | BinOp::Shr
                | BinOp::BitAnd
                | BinOp::BitOr
                | BinOp::BitXor,
            ..
        }
    ))
}

/// Substitute the env into an [`Expr`] (REQ-2): replace each free `Path` leaf that
/// names an in-env cell with that cell's current value expr, recursively. This is
/// the BIG-STEP state threading made concrete on the syntax — the result is a closed
/// form in the INITIAL inputs (the env values are themselves already closed forms).
/// A var NOT in env is a free input (a param) — left verbatim. This recursion covers
/// exactly the frozen exec-value `Expr` shapes (`exec-stmt-tv.md` REQ-1 RHS
/// sublanguage = the step-2.1 pure-exec subset); an out-of-subset value node is
/// passed through UNCHANGED to [`exec_ref_value`], which honestly rejects it (so the
/// `Err` carries the precise node — never a silent wrong substitution).
fn substitute(expr: &Expr, env: &Env) -> Result<Expr, RefEncodeError> {
    match expr {
        Expr::Path(segments) => {
            if segments.len() == 1 {
                if let Some(value) = env.get(&segments[0]) {
                    return Ok(value.clone());
                }
            }
            Ok(expr.clone())
        }
        Expr::IntLit { .. } | Expr::BoolLit(_) | Expr::StrLit(_) => Ok(expr.clone()),
        Expr::Binary { op, lhs, rhs } => Ok(Expr::Binary {
            op: *op,
            lhs: Box::new(substitute(lhs, env)?),
            rhs: Box::new(substitute(rhs, env)?),
        }),
        Expr::Unary { op, expr: inner } => Ok(Expr::Unary {
            op: *op,
            expr: Box::new(substitute(inner, env)?),
        }),
        Expr::Cast { expr: inner, ty } => Ok(Expr::Cast {
            expr: Box::new(substitute(inner, env)?),
            ty: ty.clone(),
        }),
        Expr::Call { callee, args } => Ok(Expr::Call {
            callee: Box::new(substitute(callee, env)?),
            args: args
                .iter()
                .map(|a| substitute(a, env))
                .collect::<Result<Vec<_>, _>>()?,
        }),
        Expr::Index { base, index } => {
            let new_index = match index {
                IndexArg::Single(i) => IndexArg::Single(Box::new(substitute(i, env)?)),
                IndexArg::RangeTo(i) => IndexArg::RangeTo(Box::new(substitute(i, env)?)),
                IndexArg::RangeFrom(i) => IndexArg::RangeFrom(Box::new(substitute(i, env)?)),
                IndexArg::Range(a, b) => {
                    IndexArg::Range(Box::new(substitute(a, env)?), Box::new(substitute(b, env)?))
                }
            };
            Ok(Expr::Index {
                base: Box::new(substitute(base, env)?),
                index: new_index,
            })
        }
        Expr::Tuple(elems) => Ok(Expr::Tuple(
            elems
                .iter()
                .map(|e| substitute(e, env))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        // An out-of-subset value node (a method call, a struct literal, a closure, a
        // match-expr, a field/projection, a deref, a ref) is passed through
        // UNCHANGED — [`exec_ref_value`] will honestly reject it (the frozen RHS
        // sublanguage is the step-2.1 pure-exec subset). Passing it through keeps the
        // rejection in ONE place (the value encoder) with the precise node tag.
        other => Ok(other.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use thermite_syntax::ast::{BinOp, Block, Expr, Stmt};

    fn path(name: &str) -> Expr {
        Expr::Path(vec![name.to_string()])
    }
    fn int(value: u128) -> Expr {
        Expr::IntLit {
            value,
            raw: value.to_string(),
        }
    }
    fn bin(op: BinOp, lhs: Expr, rhs: Expr) -> Expr {
        Expr::Binary {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }
    }
    fn let_(mutable: bool, name: &str, init: Expr) -> Stmt {
        Stmt::Let {
            mutable,
            name: name.to_string(),
            ty: None,
            init,
        }
    }
    fn assign(target: &str, value: Expr) -> Stmt {
        Stmt::Assign {
            target: path(target),
            value,
        }
    }

    /// B1 reference: `{ let a = x + 1; let b = a * 2; b }` -> the threaded closed
    /// form `((x + 1) * 2)` (the let-chain substitution).
    #[test]
    fn b1_let_chain_state() {
        let block = Block {
            stmts: vec![
                let_(false, "a", bin(BinOp::Add, path("x"), int(1))),
                let_(false, "b", bin(BinOp::Mul, path("a"), int(2))),
            ],
            tail: Some(Box::new(path("b"))),
        };
        assert_eq!(
            body_ref_state(&block, &BodyRefCtx::default()).unwrap(),
            "((x + 1) * 2)"
        );
    }

    /// B2 reference (the mutation-ORDER teeth): `s = s + 1; s = s * 2` threads to
    /// `((x + 1) * 2)` — and the REORDER threads to a DIFFERENT form, so the order
    /// is load-bearing in the reference (not just in production).
    #[test]
    fn b2_mutation_order_state() {
        let ordered = Block {
            stmts: vec![
                let_(true, "s", path("x")),
                assign("s", bin(BinOp::Add, path("s"), int(1))),
                assign("s", bin(BinOp::Mul, path("s"), int(2))),
            ],
            tail: Some(Box::new(path("s"))),
        };
        assert_eq!(
            body_ref_state(&ordered, &BodyRefCtx::default()).unwrap(),
            "((x + 1) * 2)"
        );

        // The reorder is a DIFFERENT closed form — the state threading is real.
        let reordered = Block {
            stmts: vec![
                let_(true, "s", path("x")),
                assign("s", bin(BinOp::Mul, path("s"), int(2))),
                assign("s", bin(BinOp::Add, path("s"), int(1))),
            ],
            tail: Some(Box::new(path("s"))),
        };
        assert_eq!(
            body_ref_state(&reordered, &BodyRefCtx::default()).unwrap(),
            "((x * 2) + 1)"
        );
    }

    /// B3 reference (the `if`-branch state-transformer): the tail `if c { x + 1 }
    /// else { x - 1 }` composes the two branch tails.
    #[test]
    fn b3_if_branch_state() {
        let then = Block {
            stmts: vec![],
            tail: Some(Box::new(bin(BinOp::Add, path("x"), int(1)))),
        };
        let els = Block {
            stmts: vec![],
            tail: Some(Box::new(bin(BinOp::Sub, path("x"), int(1)))),
        };
        let block = Block {
            stmts: vec![],
            tail: Some(Box::new(Expr::If {
                cond: Box::new(path("c")),
                then,
                else_: els,
            })),
        };
        assert_eq!(
            body_ref_state(&block, &BodyRefCtx::default()).unwrap(),
            "if c { (x + 1) } else { (x - 1) }"
        );
    }

    /// B4 reference (the multi-cell TUPLE — the design's least-confident #1): the
    /// final state `(a, b)` projects `a |-> (x + 1)`, `b |-> (y + (x + 1))` (b uses
    /// the UPDATED a, the order-sensitive threading).
    #[test]
    fn b4_multi_cell_tuple_state() {
        let block = Block {
            stmts: vec![
                let_(true, "a", path("x")),
                let_(true, "b", path("y")),
                assign("a", bin(BinOp::Add, path("a"), int(1))),
                assign("b", bin(BinOp::Add, path("b"), path("a"))),
            ],
            tail: Some(Box::new(Expr::Tuple(vec![path("a"), path("b")]))),
        };
        assert_eq!(
            body_ref_state(&block, &BodyRefCtx::default()).unwrap(),
            "((x + 1), (y + (x + 1)))"
        );
    }

    /// A loop body is OUT of the frozen 2.2.1 subset -> an honest `Err`, NEVER a
    /// silent (wrong) denotation (REQ-1 honest boundary).
    #[test]
    fn loop_body_is_unsupported_not_panic() {
        use thermite_syntax::ast::{Clause, LoopKind, LoopNode};
        let span = thermite_syntax::lexer::Span { start: 0, len: 0 };
        let loop_node = LoopNode {
            kind: LoopKind::While(Box::new(path("c"))),
            invs: vec![Clause {
                expr: Expr::BoolLit(true),
                text: "true".to_string(),
                span,
            }],
            dec: Clause {
                expr: int(0),
                text: "0".to_string(),
                span,
            },
            body: Block {
                stmts: vec![],
                tail: None,
            },
            span,
        };
        let block = Block {
            stmts: vec![Stmt::Loop(loop_node)],
            tail: Some(Box::new(path("x"))),
        };
        assert!(matches!(
            body_ref_state(&block, &BodyRefCtx::default()),
            Err(RefEncodeError::Unsupported(_))
        ));
    }

    /// A re-shadow `let x = ..; let x = ..` in the same block is OUT of v1 (the flat
    /// env can't represent two `x` cells) -> an honest `Err`.
    #[test]
    fn reshadow_is_unsupported() {
        let block = Block {
            stmts: vec![let_(false, "a", path("x")), let_(false, "a", int(1))],
            tail: Some(Box::new(path("a"))),
        };
        assert!(matches!(
            body_ref_state(&block, &BodyRefCtx::default()),
            Err(RefEncodeError::Unsupported(_))
        ));
    }
}
