//! Pinning regression for crosslink #225 — the `plain_user_spec_call` arm in
//! `thermite-lower/src/lower.rs` HARDCODED `as u64` on an arithmetic / unary
//! argument to a user `spec fn`, regardless of the callee's DECLARED parameter
//! type. A recursive `spec fn s_dec(n: u32) -> u32 { … s_dec(n - 1) }` therefore
//! emitted `s_dec((n - 1) as u64)` — ill-typed Verus (`expected u32, found u64`),
//! so the item died at L0 with an opaque obligation failure though the Thermite
//! source is fine.
//!
//! THE AUTHORITY (R-CHAR-3): the narrowing cast IS legitimate — Verus spec
//! arithmetic is the unbounded `int`, so a `u32`-typed `n - 1` evaluates to `int`
//! and must narrow back to the callee's EXEC param type (`.design/lower/`
//! `verus-lowering.md` REQ-5, "the cast `i as int` is mandatory — Verus spec
//! indices are `int`"; the surface integer set is `u32`/`u64`/`usize`). Only the
//! TARGET of the cast was wrong: it must be the callee's DECLARED param type for
//! that argument position, not a hardcoded `u64`. The expected substrings below
//! are HAND-DERIVED from the param types in each fixture's `spec fn` signature,
//! NOT copied from the lowerer's output.
//!
//! Three cast cases pinned: a `u32` param → `as u32` (and NOT `as u64`); a `u64`
//! param → `as u64` (the no-regression case); a `usize` param → `as usize`.

use thermite_syntax::ast::{Expr, Item, UnaryOp};

fn lower(src: &str) -> String {
    let parsed = thermite_syntax::parse(src);
    assert!(parsed.is_clean(), "fixture must parse clean: {parsed:?}");
    thermite_lower::lower(&parsed.program).expect("L3 lowering")
}

/// Non-vacuity helper: the parsed program genuinely contains a recursive spec-fn
/// CALL whose argument is an arithmetic (`Binary`)/unary expression — the shape
/// the `plain_user_spec_call` cast arm fires on (else the test is vacuous).
fn has_arith_spec_call_arg(src: &str) -> bool {
    fn ex(e: &Expr) -> bool {
        match e {
            Expr::Call { args, .. } => args
                .iter()
                .any(|a| matches!(a, Expr::Binary { .. } | Expr::Unary { .. }) || ex(a)),
            Expr::Binary { lhs, rhs, .. } => ex(lhs) || ex(rhs),
            Expr::Unary { expr, .. } | Expr::Cast { expr, .. } => ex(expr),
            Expr::If { cond, then, else_ } => {
                ex(cond)
                    || then.tail.as_deref().map(ex).unwrap_or(false)
                    || else_.tail.as_deref().map(ex).unwrap_or(false)
            }
            _ => false,
        }
    }
    let parsed = thermite_syntax::parse(src);
    parsed.program.items.iter().any(|i| match i {
        Item::SpecFn(s) => s.body.tail.as_deref().map(ex).unwrap_or(false),
        _ => false,
    })
}

const U32_PROGRAM: &str = "\
spec fn s_dec(n: u32) -> u32
  dec n
{
  if n == 0 {
    0
  } else {
    s_dec(n - 1)
  }
}
";

const U64_PROGRAM: &str = "\
spec fn s_dec(n: u64) -> u64
  dec n
{
  if n == 0 {
    0
  } else {
    s_dec(n - 1)
  }
}
";

const USIZE_PROGRAM: &str = "\
spec fn s_dec(n: usize) -> usize
  dec n
{
  if n == 0 {
    0
  } else {
    s_dec(n - 1)
  }
}
";

#[test]
fn u32_param_spec_call_arith_arg_casts_as_u32_not_u64() {
    assert!(
        has_arith_spec_call_arg(U32_PROGRAM),
        "fixture must contain an arithmetic spec-fn-call arg (else vacuous)"
    );
    let out = lower(U32_PROGRAM);
    // The callee `s_dec` declares a `u32` param, so the spec-arithmetic `n - 1`
    // (which is `int` in Verus spec position) narrows back to the DECLARED `u32`.
    assert!(
        out.contains("s_dec((n - 1) as u32)"),
        "a u32-param spec fn's recursive arith arg must cast `as u32`:\n{out}"
    );
    // The divergence: it must NOT emit the hardcoded `as u64` (ill-typed —
    // `expected u32, found u64`).
    assert!(
        !out.contains("s_dec((n - 1) as u64)"),
        "the hardcoded `as u64` cast is the #225 bug — must be gone:\n{out}"
    );
}

#[test]
fn u64_param_spec_call_arith_arg_still_casts_as_u64() {
    assert!(has_arith_spec_call_arg(U64_PROGRAM));
    let out = lower(U64_PROGRAM);
    // No regression: a genuinely `u64`-param callee still narrows `as u64`.
    assert!(
        out.contains("s_dec((n - 1) as u64)"),
        "a u64-param spec fn's recursive arith arg must still cast `as u64` (no regression):\n{out}"
    );
}

#[test]
fn usize_param_spec_call_arith_arg_casts_as_usize() {
    assert!(has_arith_spec_call_arg(USIZE_PROGRAM));
    let out = lower(USIZE_PROGRAM);
    // A `usize`-param callee narrows `as usize`.
    assert!(
        out.contains("s_dec((n - 1) as usize)"),
        "a usize-param spec fn's recursive arith arg must cast `as usize`:\n{out}"
    );
    assert!(
        !out.contains("s_dec((n - 1) as u64)"),
        "a usize param must NOT get the hardcoded `as u64`:\n{out}"
    );
}

// Keep `UnaryOp` import load-bearing — a unary-negation arg would also hit the
// `plain_user_spec_call` arm (the surface has no negative literals, but a future
// unary arg must take the same param-type-directed cast). Referenced so the
// import is not dead.
#[allow(dead_code)]
const _UNARY_OP_REF: Option<UnaryOp> = None;
