//! Adversarial audit of #37 — the CRUX regression probe: lowering must emit the
//! numeric `value` in the EXECUTABLE expression, NOT the verbatim `raw`.
//!
//! ast.md REQ-6 CRITICAL note: "the thermite-lower lowering … continues to
//! emit the numeric `value` (e.g. `1000000`), NOT the raw — so the
//! `tests/golden/lower/*.verus.rs` files do NOT change". This test pins that
//! invariant directly on a FRESH `_`-bearing literal.
//!
//! IMPORTANT (critic note on assertion shape): the L1 lowering ALSO embeds the
//! verbatim clause SOURCE text inside the human-legible diagnostic label of
//! `thermite_check!("req", "x <= 1_000", x <= 1000)` — that `1_000` in the
//! string literal is the original `Clause.text` and is CORRECT (the legible
//! diagnostic, not the executable form). The invariant is about the LOWERED
//! EXPRESSION (`x <= 1000`), so we assert on that form, not a blanket substring.
//!
//! Expected values hand-derived from the cited REQ text — the value `1000` is
//! the `_`-stripped form of the source `1_000` (R-CHAR-3: not copied from the
//! lowerer's output). Expected to PASS under a2c0f73 (documents NO divergence).

use thermite_syntax::ast::{BinOp, Expr, Item};

/// ast.md REQ-6 / AC-1b: a contract with a `_`-bearing literal (`req x <=
/// 1_000`) lowers so the EXECUTABLE comparison is `x <= 1000` (the value), and
/// no executable expression `x <= 1_000` (the raw) appears. A `_` in the
/// emitted Verus/L1 *expression* would itself be a behavior change.
#[test]
fn divergence_lowering_emits_value_not_raw() {
    let src = "fn f(x: u32) -> u32 req x <= 1_000 ens result == 0 fx pure { 0 }";
    let parsed = thermite_syntax::parse(src);
    assert!(parsed.is_clean(), "fixture must parse clean: {parsed:?}");

    // Confirm the parsed node carries the verbatim raw, so the test is not
    // vacuous — the raw genuinely contains a `_` that lowering must drop from
    // the executable form.
    let Item::Fn(f) = &parsed.program.items[0] else {
        panic!("expected a fn item");
    };
    let Expr::Binary {
        rhs, op: BinOp::Le, ..
    } = &f.contract.req.expr
    else {
        panic!("expected a `<=` req, got {:?}", f.contract.req.expr);
    };
    match rhs.as_ref() {
        Expr::IntLit { value, raw } => {
            assert_eq!(*value, 1_000u128);
            assert_eq!(raw, "1_000", "raw must preserve the separator");
        }
        other => panic!("expected an IntLit rhs, got {other:?}"),
    }

    // L3 lowering: the executable comparison is `x <= 1000` (value), and the
    // raw form `x <= 1_000` never appears anywhere in the L3 output.
    let l3 = thermite_lower::lower(&parsed.program).expect("L3 lowering");
    assert!(
        l3.contains("x <= 1000"),
        "L3 must lower the comparison to the value `x <= 1000`:\n{l3}"
    );
    assert!(
        !l3.contains("x <= 1_000"),
        "L3 must NOT lower the comparison to the raw `x <= 1_000`:\n{l3}"
    );

    // L1 lowering: the executable check expression is `x <= 1000` (value). The
    // raw `1_000` is permitted ONLY inside the diagnostic-label string literal
    // ("x <= 1_000"), never as the executable comparison `x <= 1_000`.
    let l1 = thermite_lower::lower_l1(&parsed.program).expect("L1 lowering");
    assert!(
        l1.contains("x <= 1000"),
        "L1 must lower the check expression to the value `x <= 1000`:\n{l1}"
    );
    assert!(
        !l1.contains("x <= 1_000)"),
        "L1 must NOT use the raw `x <= 1_000` as the executable check expr:\n{l1}"
    );
}
