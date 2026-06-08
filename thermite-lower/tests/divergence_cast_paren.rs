//! Regression pin for #122 — the cast lowering MUST parenthesize a binary/unary
//! inner operand so a `(binary) as T` lowers correctly.
//!
//! `as` binds TIGHTER than the binary operators in both Verus and Rust. So
//! `(n - 1) as nat` MUST emit the inner in parens (`(n - 1) as nat`); without
//! them `n - 1 as nat` parses as `n - (1 as nat)` — an `int`/`nat` (or
//! `u64`/`usize`) type mismatch → L0. Surfaced twice on `main` before this fix:
//! a mutual-recursion spec twin `s_odd((n - 1) as nat)` (the L3 check path in
//! `lower.rs`) and the editor's compound string index `slice(i - 1, …)` /
//! `byte_at(i - 1)` (the L1 build path in `l1.rs`, which had been worked around
//! with forward scans).
//!
//! Expected values are HAND-DERIVED from the design authority — the golden
//! reference `tests/golden/lower/parse_u64.verus.rs` writes exactly
//! `pow10((k - 1) as nat)` and `s.subrange(0, (s.len() - 1) as int)` (R-CHAR-3:
//! the paren'd form is the design's, not copied from the lowerer's output). The
//! NEGATIVE assertion (the bare `k - 1 as nat` form must NOT appear) is the
//! divergence the bug produced.

use thermite_syntax::ast::{Expr, Item};

/// L3 (`lower.rs` `Expr::Cast` arm): a `(k - 1) as nat` in a recursive spec-fn
/// body lowers to the parenthesized `(k - 1) as nat`, never the mis-binding
/// `k - 1 as nat` (= `k - (1 as nat)`). The fixture's source genuinely contains
/// a binary inner under the cast, so the test is non-vacuous.
#[test]
fn cast_over_binary_inner_is_parenthesized_in_spec_position_l3() {
    let src =
        "spec fn pow(k: nat) -> nat dec k { if k == 0 { 1 } else { 10 * pow((k - 1) as nat) } }";
    let parsed = thermite_syntax::parse(src);
    assert!(parsed.is_clean(), "fixture must parse clean: {parsed:?}");

    // Non-vacuity: the parsed body really carries a `Cast` over a `Binary`.
    let Item::SpecFn(sf) = &parsed.program.items[0] else {
        panic!("expected a spec fn item");
    };
    assert!(
        block_has_binary_cast(&sf.body),
        "fixture must contain a Cast over a Binary inner (else the test is vacuous)"
    );

    let l3 = thermite_lower::lower(&parsed.program).expect("L3 lowering");
    assert!(
        l3.contains("(k - 1) as nat"),
        "L3 must parenthesize the binary cast inner — `(k - 1) as nat` (#122):\n{l3}"
    );
    // The mis-binding form must NOT appear: `k - 1 as nat` (no parens) is
    // `k - (1 as nat)`, the int/nat mismatch the bug produced.
    assert!(
        !l3.contains("k - 1 as nat"),
        "L3 must NOT emit the unparenthesized `k - 1 as nat` (= `k - (1 as nat)`, #122):\n{l3}"
    );
}

/// L3 (`lower.rs` byte_at/slice index coercion): a compound string index
/// `byte_at(i - 1)` in a CONTRACT coerces the index `as usize` with the inner
/// parenthesized — `(i - 1) as usize`, never `i - 1 as usize` (= `i - (1 as
/// usize)`, a `u64 - usize` mismatch, E0277). This is the editor's case.
#[test]
fn compound_string_index_in_contract_is_parenthesized_l3() {
    let src = "fn last(s: String, i: u64) -> u64 \
               req i >= 1 && i <= s.len() \
               ens result == s.byte_at(i - 1) \
               fx pure { s.byte_at(i - 1) }";
    let parsed = thermite_syntax::parse(src);
    assert!(parsed.is_clean(), "fixture must parse clean: {parsed:?}");

    let l3 = thermite_lower::lower(&parsed.program).expect("L3 lowering");
    assert!(
        l3.contains("(i - 1) as usize"),
        "L3 must parenthesize the compound index coercion — `(i - 1) as usize` (#122):\n{l3}"
    );
    assert!(
        !l3.contains("i - 1 as usize"),
        "L3 must NOT emit the unparenthesized `i - 1 as usize` (= `i - (1 as usize)`, #122):\n{l3}"
    );
}

/// L1 (`l1.rs` byte_at/slice index coercion — the build path): the editor's
/// `slice(i - 1, j)` / `byte_at(i - 1)` lowers with the compound index
/// parenthesized so the runnable Rust compiles (no `u64 - usize` E0277).
#[test]
fn compound_string_index_in_build_path_is_parenthesized_l1() {
    let src = "fn last(s: String, i: u64) -> u64 \
               req i >= 1 && i <= s.len() \
               ens result == s.byte_at(i - 1) \
               fx pure { s.byte_at(i - 1) }";
    let parsed = thermite_syntax::parse(src);
    assert!(parsed.is_clean(), "fixture must parse clean: {parsed:?}");

    let l1 = thermite_lower::lower_l1(&parsed.program).expect("L1 lowering");
    assert!(
        l1.contains("(i - 1) as usize"),
        "L1 must parenthesize the compound index coercion — `(i - 1) as usize` (#122):\n{l1}"
    );
    assert!(
        !l1.contains("i - 1 as usize"),
        "L1 must NOT emit the unparenthesized `i - 1 as usize` (#122):\n{l1}"
    );
}

/// No regression: a SIMPLE cast inner (`i as usize`, a bare path) never
/// mis-binds, so the paren is NOT added — the corpus/golden simple casts stay
/// byte-identical. Authority: `tests/golden/lower/sum.verus.rs` writes `i as
/// int`, `acc as nat` WITHOUT parens (the simple-inner form, R-CHAR-3).
#[test]
fn simple_cast_inner_is_not_parenthesized_no_regression() {
    let src =
        "spec fn pow(k: nat) -> nat dec k { if k == 0 { 1 } else { 10 * pow((k - 1) as nat) } }";
    let parsed = thermite_syntax::parse(src);
    assert!(parsed.is_clean(), "fixture must parse clean: {parsed:?}");
    let l3 = thermite_lower::lower(&parsed.program).expect("L3 lowering");
    // The simple comparison `k == 0` is emitted plain (no spurious parens), and
    // there is no `(k) as` artifact from over-parenthesizing a non-binary inner.
    assert!(
        !l3.contains("(k) as"),
        "a non-binary cast inner must NOT be parenthesized (`(k) as ...` is a regression):\n{l3}"
    );
}

/// Walk a tail expression looking for a `Cast` whose inner is a `Binary` — the
/// non-vacuity check for the L3 spec-position test.
fn expr_has_binary_cast(e: &Expr) -> bool {
    match e {
        Expr::Cast { expr, .. } => matches!(expr.as_ref(), Expr::Binary { .. }),
        Expr::If { then, else_, .. } => block_has_binary_cast(then) || block_has_binary_cast(else_),
        Expr::Binary { lhs, rhs, .. } => expr_has_binary_cast(lhs) || expr_has_binary_cast(rhs),
        Expr::Call { args, .. } => args.iter().any(expr_has_binary_cast),
        _ => false,
    }
}

fn block_has_binary_cast(b: &thermite_syntax::ast::Block) -> bool {
    b.tail.as_deref().map(expr_has_binary_cast).unwrap_or(false)
}
