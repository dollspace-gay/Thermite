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

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use thermite_syntax::ast::{Block, Expr, IndexArg, Stmt};

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
        Stmt::If { .. } => Err(RefEncodeError::Unsupported(
            "`if` as a non-tail STATEMENT (a mid-body branch mutating the state per \
             arm is OUT of v1 — the state-denotation composes an `if` only in TAIL \
             position; a per-arm-mutating `if` is a 2.2.2-adjacent multi-exit form)"
                .to_string(),
        )),
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

/// Encode a body VALUE position (a tail expr, a branch tail, a tail-`return`'s
/// expr) under `env` (REQ-2). An `if`-EXPRESSION composes the two branch
/// state-transformers; a TUPLE projects the multi-cell final state; everything else
/// is an exec VALUE -> SUBSTITUTE the env then REUSE [`exec_ref_value`].
fn encode_value(expr: &Expr, env: &Env, ctx: &BodyRefCtx) -> Result<String, RefEncodeError> {
    match expr {
        // The `if` state-transformer (`exec-stmt-tv.md` AC-4): compose the two
        // branch transformers into a Verus `if`-expression over the env-substituted
        // condition. Each branch is its OWN straight-line block (a fresh env CLONE
        // — a branch-local `let` does not leak past the branch), and the branch
        // VALUE is the branch's tail.
        Expr::If { cond, then, else_ } => {
            let c = encode_value(cond, env, ctx)?;
            let mut then_env = env.clone();
            let t = encode_block_tail(then, &mut then_env, ctx)?;
            let mut else_env = env.clone();
            let e = encode_block_tail(else_, &mut else_env, ctx)?;
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
        // step-2.1 exec VALUE: substitute the env into it, then REUSE the
        // independent per-RHS encoder (the #122/#146/overflow disciplines unchanged).
        other => {
            let substituted = substitute(other, env)?;
            exec_ref_value(&substituted, &ctx.exec_ref_ctx())
        }
    }
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
