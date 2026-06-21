//! Stage-2 REQ-8 — production quantifier emission in `thermite-lower`
//! (`.design/stage2-stratified-cage.md` REQ-8). The raw `forall`/`exists` surface binder
//! (REQ-0's `Expr::Quantifier`) now lowers to the Verus MBQI index-grammar surface
//! instead of refusing (`LowerError::Unsupported`, the pre-REQ-8 behaviour).

use thermite_lower::lower_contract_expr;
use thermite_syntax::ast::{BinOp, Expr, Quant};

fn path(name: &str) -> Expr {
    Expr::Path(vec![name.to_string()])
}

fn int(value: u128) -> Expr {
    Expr::IntLit {
        value,
        raw: value.to_string(),
    }
}

#[test]
fn forall_lowers_to_bounded_int_quantifier() {
    // `forall (i : usize) in xs. i < n`
    let phi = Expr::Quantifier {
        quant: Quant::Forall,
        var: "i".to_string(),
        sort: "usize".to_string(),
        domain: Box::new(path("xs")),
        body: Box::new(Expr::Binary {
            op: BinOp::Lt,
            lhs: Box::new(path("i")),
            rhs: Box::new(path("n")),
        }),
    };
    let out = lower_contract_expr(&phi, &[], &[], &[], &[], &[], &[]).expect("lowers");
    // The bounded index form: `forall|i: int| 0 <= i < xs.len() ==> (i < n)`.
    assert!(out.starts_with("forall|i: int|"), "{out}");
    assert!(out.contains("0 <= i < xs.len()"), "membership guard: {out}");
    assert!(out.contains("==>"), "forall guards with ==>: {out}");
    assert!(out.contains("(i < n)"), "body lowered in scope: {out}");
    // Trigger-free MBQI surface (T1-S `strat_ref_wf`): no `#[trigger]`.
    assert!(!out.contains("#[trigger]"), "trigger-free surface: {out}");
}

#[test]
fn exists_lowers_to_bounded_conjunction() {
    // `exists (i : usize) in xs. i == 0`
    let phi = Expr::Quantifier {
        quant: Quant::Exists,
        var: "i".to_string(),
        sort: "usize".to_string(),
        domain: Box::new(path("xs")),
        body: Box::new(Expr::Binary {
            op: BinOp::Eq,
            lhs: Box::new(path("i")),
            rhs: Box::new(int(0)),
        }),
    };
    let out = lower_contract_expr(&phi, &[], &[], &[], &[], &[], &[]).expect("lowers");
    // The bounded existential form: `exists|i: int| 0 <= i < xs.len() && (i == 0)`.
    assert!(out.starts_with("exists|i: int|"), "{out}");
    assert!(out.contains("0 <= i < xs.len()"), "membership guard: {out}");
    assert!(out.contains("&&"), "exists guards with &&: {out}");
    assert!(out.contains("(i == 0)"), "body lowered in scope: {out}");
}
