//! Divergence tests for compile-time effect-row subsumption
//! (`thermite-lower/src/effects.rs`, commit 5b0967f).
//!
//! Authority: `thermite-design.md` §4.1 — "Effect rows compose: a caller's row
//! must subsume EVERY callee's row, checked at compile time"; and §9 (trust
//! invariant under composition). Governing contract:
//! `.design/lower/effect-subsumption.md` REQ-2/REQ-3 + OQ-2 (direct per-call-site
//! checking composes to transitive correctness ONLY IF every reachable call site
//! is walked).
//!
//! Each test is HAND-DERIVED from §4.1 (R-CHAR-3) — expected values never copied
//! from the checker's own output. `unwrap`/`panic` are fine: `tests/` is not gated.
//!
//! Tracking: crosslink #38 (release-blocker; the failing test IS the block,
//! left un-`#[ignore]`d per goal.md R-DEFER-3).

use thermite_lower::{check_effects, LowerError};
use thermite_syntax::ast::{
    Block, Clause, Contract, Effect, EffectRow, Expr, FnItem, Item, LoopKind, LoopNode, Program,
    Stmt, Type,
};
use thermite_syntax::lexer::Span;

fn span() -> Span {
    Span::new(0, 1)
}

fn true_clause() -> Clause {
    Clause {
        expr: Expr::BoolLit(true),
        text: "true".to_string(),
        span: span(),
    }
}

/// A bare `Expr::Call` to `callee` with no args.
fn call(callee: &str) -> Expr {
    Expr::Call {
        callee: Box::new(Expr::Path(vec![callee.to_string()])),
        args: vec![],
    }
}

/// A `fn` with the given effect row and an explicit body block.
fn fn_with_body(name: &str, fx: EffectRow, body: Block) -> Item {
    Item::Fn(FnItem {
        slag: None,
        boundary: None,
        name: name.to_string(),
        params: vec![],
        ret: Type::Unit,
        contract: Contract {
            req: true_clause(),
            ens: vec![true_clause()],
            fx,
        },
        body: Some(body),
        span: span(),
    })
}

// ---------------------------------------------------------------------------
// DIVERGENCE 1 (crosslink #38): a callee invoked in a `while` LOOP CONDITION
// escapes the subsumption check.
//
// §4.1: "a caller's row must subsume EVERY callee's row". A `while <cond> { }`
// evaluates `<cond>` at runtime before each iteration; a `Call` inside it is a
// reachable callee. `ast.rs` models this as `LoopKind::While(Box<Expr>)`, and
// `lower.rs` itself lowers the condition (`LoopKind::While(c)` arm). But
// `effects.rs`'s `Stmt::Loop(l)` arm walks ONLY `&l.body` and NEVER inspects
// `l.kind`, so the condition's call site is silently skipped.
//
// A `fx pure` caller running `while effectful() { }` (where `effectful` has
// `fx {alloc}`) MUST be rejected with `missing: [Alloc]` (hand-derived from
// §4.1). The current checker returns Ok(()) — a false ACCEPT.
// ---------------------------------------------------------------------------
#[test]
fn divergence_while_condition_callee_is_checked() {
    // caller: fx pure, body = `while effectful() { }`
    let while_loop = Stmt::Loop(LoopNode {
        kind: LoopKind::While(Box::new(call("effectful"))),
        invs: vec![true_clause()],
        dec: true_clause(),
        body: Block {
            stmts: vec![],
            tail: None,
        },
        span: span(),
    });
    let caller = fn_with_body(
        "caller",
        EffectRow::Pure,
        Block {
            stmts: vec![while_loop],
            tail: None,
        },
    );
    let callee = fn_with_body(
        "effectful",
        EffectRow::Set(vec![Effect::Alloc]),
        Block {
            stmts: vec![],
            tail: None,
        },
    );
    let prog = Program {
        items: vec![caller, callee],
    };

    // §4.1: the `while`-condition call is a real callee; a pure caller calling an
    // {alloc} callee must be rejected with missing == [Alloc].
    match check_effects(&prog) {
        Err(errs) => {
            assert!(
                errs.iter().any(|e| matches!(
                    e,
                    LowerError::EffectNotSubsumed { callee, missing, .. }
                        if callee == "effectful" && *missing == vec![Effect::Alloc]
                )),
                "while-condition callee must be checked; expected EffectNotSubsumed \
                 {{callee: effectful, missing: [Alloc]}}, got {errs:?}"
            );
        }
        Ok(()) => panic!(
            "DIVERGENCE: `pure` caller running `while effectful() {{}}` with effectful \
             fx {{alloc}} was ACCEPTED — the while-condition call site escaped the \
             subsumption check (effects.rs Stmt::Loop walks only l.body, not l.kind). \
             §4.1 requires a caller subsume EVERY callee's row."
        ),
    }
}
