//! Final holistic re-audit of `thermite-syntax` (issue #3), after the #28/#30/
//! #29/#31/#32 fix sequence. This file pins ONE residual divergence the
//! representation-agnostic conformance oracle does not catch.
//!
//! DIVERGENCE — the #30 if-tail refactor promotes a VALUE-LESS trailing
//! `if/else` to a tail `Expr::If` (the expression form) purely on a positional
//! rule (has-`else` + nothing follows), ignoring the design's discriminator.
//!
//! Authority — `.design/syntax/surface-grammar.md` "Key design decisions" #2
//! ("`if` is both a statement and an expression"): "The expression form
//! requires an `else` (it must have a value); the statement form does not.";
//! and OQ-3: "REQ-5 makes the *expression* form of `if` require an `else` (it
//! must produce a value) ... The corpus only uses the statement form." The
//! discriminator the design names is VALUE-NESS, not source position.
//!
//! The corpus `conformance/binary_search.th` ends its `loop` body with
//!   `if haystack[mid] < needle { lo = mid + 1; } else { hi = mid; }`
//! whose branches are assignment statements (`Block { tail: None }`) — it
//! produces NO value, so by the design it is the STATEMENT form. The parser
//! (`parse_block` tail-promotion via `parse_if_parts`, src #30) encodes it as
//! the loop body's tail `Expr::If` (the value/expression form). That is an
//! AST-shape divergence on a verbatim corpus construct, masked because
//! `conformance/parse/binary_search.facts.json` is "representation-agnostic"
//! and pins no stmt-vs-tail fact for the trailing if.
//!
//! Tracking: filed as a `-l blocker` via crosslink (see report).

use std::fs;
use thermite_syntax::{parse, Block, Item, LoopKind, Stmt};

fn corpus(rel: &str) -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf();
    fs::read_to_string(root.join(rel)).expect("read corpus file")
}

/// True if a block produces a value (has a tail expression).
fn produces_value(b: &Block) -> bool {
    b.tail.is_some()
}

#[test]
fn divergence_value_less_trailing_if_is_statement_not_tail_expr() {
    let r = parse(&corpus("conformance/binary_search.th"));
    assert!(
        r.is_clean(),
        "binary_search must parse clean: {:?}",
        r.errors
    );
    let Item::Fn(f) = &r.program.items[0] else {
        panic!("binary_search is the first item and is a `fn`");
    };
    let Some(body) = &f.body else {
        panic!("binary_search is an in-language fn with a body (not a boundary fn)");
    };

    // Locate the bare `loop` (binary_search.loop#1).
    let lp = body
        .stmts
        .iter()
        .find_map(|s| match s {
            Stmt::Loop(lp) => Some(lp),
            _ => None,
        })
        .expect("binary_search has a top-level `loop`");
    assert!(
        matches!(lp.kind, LoopKind::Loop),
        "binary_search.loop#1 is a bare `loop`"
    );

    // The loop body's final construct is the corpus's value-less `if/else`:
    //   `if haystack[mid] < needle { lo = mid + 1; } else { hi = mid; }`
    // Both branches are assignment statements -> neither branch produces a
    // value. By surface-grammar.md decision #2 ("the expression form ... must
    // have a value; the statement form does not") + OQ-3 ("the corpus only uses
    // the statement form"), this is the STATEMENT form: it must be the LAST
    // `Stmt::If` in the body, and the loop body must have NO tail expr.
    //
    // EXPECTED (design authority): loop body tail is None; last stmt is Stmt::If
    //   whose else-branch produces no value.
    // ACTUAL (current parser, #30 tail-promotion): the if/else is hoisted to a
    //   tail `Expr::If`, so `lp.body.tail` is Some(..) and the construct is NOT
    //   a Stmt::If. This assertion therefore FAILS, pinning the divergence.
    assert!(
        lp.body.tail.is_none(),
        "design: a value-less trailing `if/else` (assignment branches) is the \
         statement form, so the loop body must have no tail expr — but the \
         parser promoted it to a tail Expr::If: {:?}",
        lp.body.tail
    );

    // And it must be present as the final statement-form if, with value-less
    // branches (consistent with the design's value-ness discriminator).
    match lp.body.stmts.last() {
        Some(Stmt::If {
            else_: Some(else_b),
            then,
            ..
        }) => {
            assert!(
                !produces_value(then) && !produces_value(else_b),
                "the corpus trailing if/else has value-less (assignment) branches"
            );
        }
        other => panic!(
            "design: the loop body's final construct is a statement-form \
             `if/else`, got {other:?}"
        ),
    }
}
