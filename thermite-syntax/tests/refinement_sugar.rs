//! Tests for the refinement-type sugar `x: T{P}` / `-> T{P}` and its post-parse
//! desugar pass (`.design/stage1-forge-tier.md` REQ-3, increment 2a).
//!
//! The sugar desugars in a new post-parse pass (`thermite_syntax::desugar`) so
//! downstream stages see only the v1 `req`/`ens` clause shapes: a parameter
//! refinement folds into `req` (and so becomes a Verus-checked call-site
//! obligation), a return refinement folds into `ens`. After parsing, the transient
//! `FnItem.refinements` store is empty. R-CHAR-3: expected shapes are hand-derived
//! from the grammar; `tests/` is ungated.

use thermite_syntax::{parse, BinOp, Expr, Item};

/// Parse `src` and return its single `fn` item.
fn single_fn(src: &str) -> thermite_syntax::FnItem {
    let result = parse(src);
    assert!(
        result.is_clean(),
        "expected a clean parse of {src:?}, got: {:?}",
        result.errors
    );
    match result.program.items.into_iter().next().unwrap() {
        Item::Fn(f) => f,
        other => panic!("expected a fn, got {other:?}"),
    }
}

#[test]
fn param_refinement_folds_into_req_and_clears_the_transient_store() {
    let src = "fn f(x: u64{x > 0}) -> u64 req true ens result == x fx pure { x }";
    let f = single_fn(src);
    // The transient refinement store is empty post-parse: downstream sees v1 shapes.
    assert!(
        f.refinements.is_empty(),
        "refinements must be folded + cleared, got: {:?}",
        f.refinements
    );
    // `req` is now `(true) && (x > 0)` — the refinement is a precondition conjunct.
    assert_eq!(f.contract.req.text, "(true) && (x > 0)");
    match &f.contract.req.expr {
        Expr::Binary { op: BinOp::And, .. } => {}
        other => panic!("expected req to be an `&&` conjunction, got {other:?}"),
    }
    // `ens` is unchanged (no return refinement here).
    assert_eq!(f.contract.ens.len(), 1);
}

#[test]
fn return_refinement_folds_into_ens() {
    let src = "fn f(x: u64) -> u64{result > 0} req true ens result == x fx pure { x }";
    let f = single_fn(src);
    assert!(f.refinements.is_empty());
    // The return refinement is appended as a new `ens` clause.
    assert_eq!(f.contract.ens.len(), 2, "ens: {:?}", f.contract.ens);
    assert!(
        f.contract.ens.iter().any(|c| c.text == "result > 0"),
        "expected the return refinement among ens clauses: {:?}",
        f.contract.ens
    );
    // `req` is untouched (no parameter refinement).
    assert_eq!(f.contract.req.text, "true");
}

#[test]
fn multiple_param_refinements_chain_into_req() {
    let src = "fn f(x: u64{x > 0}, y: u64{y < 100}) -> u64 req true ens result == x fx pure { x }";
    let f = single_fn(src);
    assert!(f.refinements.is_empty());
    // Both refinements fold in, in source order: ((true) && (x > 0)) && (y < 100).
    assert_eq!(f.contract.req.text, "((true) && (x > 0)) && (y < 100)");
}

#[test]
fn both_param_and_return_refinements_desugar() {
    let src = "fn f(x: u64{x > 0}) -> u64{result >= x} req true ens result == x fx pure { x }";
    let f = single_fn(src);
    assert!(f.refinements.is_empty());
    assert_eq!(f.contract.req.text, "(true) && (x > 0)");
    assert_eq!(f.contract.ens.len(), 2);
    assert!(f.contract.ens.iter().any(|c| c.text == "result >= x"));
}

#[test]
fn an_unrefined_fn_is_byte_stable() {
    // A fn with no refinement is unchanged by the pass (no spurious conjuncts).
    let src = "fn f(x: u64) -> u64 req x > 0 ens result == x fx pure { x }";
    let f = single_fn(src);
    assert!(f.refinements.is_empty());
    assert_eq!(f.contract.req.text, "x > 0");
    assert_eq!(f.contract.ens.len(), 1);
}

#[test]
fn refinement_predicate_is_a_parsed_expression() {
    // The predicate is a contract-position expression, not opaque text.
    let src = "fn f(x: u64{x > 0 && x < 10}) -> u64 req true ens result == x fx pure { x }";
    let f = single_fn(src);
    // The folded req's rhs conjunct is the (parsed) predicate `x > 0 && x < 10`.
    match &f.contract.req.expr {
        Expr::Binary {
            op: BinOp::And,
            rhs,
            ..
        } => match rhs.as_ref() {
            Expr::Binary { op: BinOp::And, .. } => {}
            other => panic!("expected the predicate `x > 0 && x < 10`, got {other:?}"),
        },
        other => panic!("expected an `&&` req, got {other:?}"),
    }
}
