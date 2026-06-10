//! Regression pin for #196 — the var*var overflow discharge
//! (`.design/lower/verus-lowering.md` REQ-7, the req-bounded-mul template).
//!
//! Verus's default linear solver fails ANY product of two non-literal operands'
//! overflow obligation — even `n * n` under `req n <= 30` (probed live against
//! verus 0.2026.05.24). The lowerer emits, at the start of the BLOCK that
//! contains the product, one
//! `assert((EXPR) <= BOUND) by(nonlinear_arith) requires <req conjuncts>;`
//! whose `requires` are EXACTLY the req conjuncts the bound depends on (no
//! invented bound) and whose `BOUND` is the syntactic product of those conjuncts'
//! constants. The aid can only FAIL, never prove a false thing (R-DEFER-9).
//!
//! Expected values are HAND-DERIVED from the design contract + the user's
//! `/goal` spec (R-CHAR-3): for `n * n` under `req n <= 30`, the hand-derived
//! bound is `30 * 30 == 900` and the hypothesis is the verbatim req conjunct
//! `n <= 30` — NOT copied from the lowerer's output. The NO-AID assertions pin
//! the honest-skip cases (an unbounded factor; a non-mul body; a mutated-local
//! product) where fabricating an assert would be a proof cheat.

use thermite_syntax::ast::{BinOp, Expr, Item, Stmt};

fn lower(src: &str) -> String {
    let parsed = thermite_syntax::parse(src);
    assert!(parsed.is_clean(), "fixture must parse clean: {parsed:?}");
    thermite_lower::lower(&parsed.program).expect("L3 lowering")
}

/// Non-vacuity helper: the parsed body genuinely contains a `Binary{Mul}` of
/// two non-literal operands (so the test is exercising the real shape).
fn body_has_nonliteral_mul(body: &thermite_syntax::ast::Block) -> bool {
    fn ex(e: &Expr) -> bool {
        match e {
            Expr::Binary {
                op: BinOp::Mul,
                lhs,
                rhs,
            } => {
                (!matches!(lhs.as_ref(), Expr::IntLit { .. })
                    && !matches!(rhs.as_ref(), Expr::IntLit { .. }))
                    || ex(lhs)
                    || ex(rhs)
            }
            Expr::Binary { lhs, rhs, .. } => ex(lhs) || ex(rhs),
            Expr::Cast { expr, .. } | Expr::Ref { expr, .. } | Expr::Unary { expr, .. } => ex(expr),
            _ => false,
        }
    }
    fn st(s: &Stmt) -> bool {
        match s {
            Stmt::Let { init, .. } => ex(init),
            Stmt::Assign { value, .. } => ex(value),
            Stmt::Return(Some(e)) | Stmt::Expr(e) => ex(e),
            Stmt::Loop(l) => l.body.stmts.iter().any(st),
            _ => false,
        }
    }
    body.stmts.iter().any(st) || body.tail.as_deref().map(ex).unwrap_or(false)
}

/// The user's exact `/goal` case: `n * n` under `req n <= 30`. The aid is one
/// `by(nonlinear_arith)` assert whose bound is the hand-derived `30 * 30 == 900`
/// and whose `requires` is the verbatim req conjunct `n <= 30` (#196, REQ-7).
#[test]
fn sq_emits_req_bounded_mul_aid_with_hand_derived_bound() {
    let src = "fn sq(n: u64) -> u64 req n <= 30 ens result == n * n fx pure { n * n }";
    let parsed = thermite_syntax::parse(src);
    assert!(parsed.is_clean(), "fixture must parse clean: {parsed:?}");
    let Item::Fn(f) = &parsed.program.items[0] else {
        panic!("expected a fn item");
    };
    assert!(
        body_has_nonliteral_mul(f.body.as_ref().expect("sq has a body")),
        "fixture must contain a non-literal product (else the test is vacuous)"
    );

    let l3 = thermite_lower::lower(&parsed.program).expect("L3 lowering");
    // HAND-DERIVED: bound = 30 * 30 = 900; hypothesis = the req conjunct `n <= 30`.
    assert!(
        l3.contains("assert((n * n) <= 900) by(nonlinear_arith) requires n <= 30;"),
        "sq must emit the hand-derived var*var aid `assert((n * n) <= 900) \
         by(nonlinear_arith) requires n <= 30;` (#196):\n{l3}"
    );
    // The aid sits in a proof block at the fn body's start.
    assert!(
        l3.contains("proof {"),
        "the aid must be wrapped in a `proof {{ .. }}` block (#196):\n{l3}"
    );
    // R-DEFER-9: NO proof cheats.
    for forbidden in ["assume(false)", "external", "#[slag]"] {
        assert!(
            !l3.contains(forbidden),
            "the aid must not introduce a proof cheat `{forbidden}` (R-DEFER-9):\n{l3}"
        );
    }
}

/// A 3-var product chain `a * b * c` (= `(a * b) * c`) under per-var bounds
/// emits TWO asserts — the inner sub-product `a * b` AND the full product —
/// each with its own hand-derived bound and the exact req conjuncts it depends
/// on (#196). `a*b <= 100` (10*10), `a*b*c <= 1000` (10*10*10).
#[test]
fn three_var_product_chain_emits_one_aid_per_subproduct() {
    let src = "fn p3(a: u64, b: u64, c: u64) -> u64 \
               req a <= 10 && b <= 10 && c <= 10 ens result == a * b * c fx pure { a * b * c }";
    let l3 = lower(src);
    assert!(
        l3.contains("assert((a * b) <= 100) by(nonlinear_arith) requires a <= 10, b <= 10;"),
        "inner sub-product aid `a * b <= 100` (hand-derived 10*10) missing (#196):\n{l3}"
    );
    assert!(
        l3.contains(
            "assert((a * b * c) <= 1000) by(nonlinear_arith) requires a <= 10, b <= 10, c <= 10;"
        ),
        "full product aid `a * b * c <= 1000` (hand-derived 10*10*10) missing (#196):\n{l3}"
    );
}

/// HONEST SKIP — an UNBOUNDED factor: `n * m` where `m` carries no req bound.
/// The aid is NOT emitted (its bound is not req-derivable), so the overflow
/// obligation stands honestly — no fabricated assert (#196, R-DEFER-9).
#[test]
fn unbounded_factor_emits_no_aid() {
    let src = "fn nm(n: u64, m: u64) -> u64 req n <= 30 ens result == n * m fx pure { n * m }";
    let l3 = lower(src);
    assert!(
        !l3.contains("by(nonlinear_arith)"),
        "an unbounded factor must NOT get a fabricated aid — the obligation \
         stands honestly (#196, R-DEFER-9):\n{l3}"
    );
}

/// HONEST SKIP — a NON-mul body: `lo + (hi - lo) / 2` (the binary-search
/// midpoint shape). No product, so no aid is emitted and the lowering is
/// unchanged (#196). Pins that the template fires ONLY on a real product.
#[test]
fn non_mul_body_emits_no_aid_and_is_unchanged() {
    let src = "fn mid(lo: u64, hi: u64) -> u64 \
               req lo <= hi && hi <= 100 ens result >= lo fx pure { lo + (hi - lo) / 2 }";
    let l3 = lower(src);
    assert!(
        !l3.contains("by(nonlinear_arith)") && !l3.contains("proof {"),
        "a non-mul body must emit NO proof aid (#196):\n{l3}"
    );
    // The body lowers verbatim — the midpoint expression survives unchanged.
    assert!(
        l3.contains("lo + (hi - lo) / 2"),
        "the non-mul body must lower unchanged (#196):\n{l3}"
    );
}

/// HONEST SKIP — a product of a MUTATED LOCAL: `acc = acc * n` in a loop. `acc`
/// is not a req-bounded param (it is a `let mut` rebound each iteration), so
/// `req_expr_upper_bound` returns `None` and NO aid is emitted — the walker
/// must skip non-param operands (#196 soundness: the obligation honestly
/// stands; verus reports the real overflow).
#[test]
fn product_of_mutated_local_is_skipped() {
    let src = "fn acc_mul(n: u64) -> u64 req n <= 10 ens result >= 0 fx pure \
               { let mut acc: u64 = 1; let mut i: usize = 0; \
                 while i < 3 inv i <= 3 dec 3 - i { acc = acc * n; i = i + 1; } acc }";
    let l3 = lower(src);
    assert!(
        !l3.contains("acc * n) <=") && !l3.contains("assert((acc"),
        "a product of a MUTATED LOCAL `acc * n` must NOT get an aid — `acc` is \
         not a req-bounded param (#196 soundness):\n{l3}"
    );
}

/// A product DIRECTLY in a LOOP BODY (`n * n` inside a `while`) gets its aid in
/// a proof block at the LOOP BODY's start — a body-start fact does not flow
/// past the loop head, so fn-body placement would be inert (#196). Pins that
/// the aid is INSIDE the loop braces, after the `decreases` line.
#[test]
fn product_in_loop_body_aid_is_placed_inside_the_loop() {
    let src = "fn lm(n: u64) -> u64 req n <= 30 ens result >= 0 fx pure \
               { let mut s: u64 = 0; let mut i: usize = 0; \
                 while i < 2 inv i <= 2 dec 2 - i { let p: u64 = n * n; i = i + 1; } 0 }";
    let l3 = lower(src);
    assert!(
        l3.contains("assert((n * n) <= 900) by(nonlinear_arith) requires n <= 30;"),
        "in-loop product must still get the hand-derived aid (#196):\n{l3}"
    );
    // The aid must sit AFTER the `decreases` (i.e. inside the loop body), not at
    // fn-body start where it would be inert.
    let dec_pos = l3.find("decreases").expect("loop has a decreases");
    let aid_pos = l3.find("by(nonlinear_arith)").expect("aid present");
    assert!(
        aid_pos > dec_pos,
        "the in-loop product aid must be placed INSIDE the loop body (after the \
         `decreases`), not at fn-body start where it cannot reach the loop (#196):\n{l3}"
    );
}
