//! Divergence pin (re-audit of caa38848/#231, the #225 cast-chain) — a
//! BOOL-typed `Binary`/`Unary` argument to a user `spec fn` whose declared
//! param is `bool` gets a bogus `as u64` narrowing cast, killing the item at
//! L0 (E0308 `expected bool, found u64` — verus-confirmed live on the emitted
//! unit).
//!
//! MECHANISM: `Ctx::spec_call_param_cast` documents `PrimType::Bool => None`
//! as "a no-cast pass-through", but `None` is ALSO the not-in-map/out-of-range
//! fallback — and every consumer site (the `lower_expr` Call-arm arithmetic
//! branch and the `lower_inv_expr` Call arm) collapses both meanings with
//! `.unwrap_or("u64")` and CASTS. The #225 chain comment claims "the bug site
//! never produces a bool-targeted arithmetic arg" — FALSE: the surface unary
//! set is exactly `!` (`UnaryOp::Not`, REQ-10 #92), so EVERY `Expr::Unary`
//! argument is bool-typed, and a comparison `x < y` is a bool-typed
//! `Expr::Binary`. `req s_b(x, x < y)` emits `s_b(x, (x < y) as u64)` → E0308.
//!
//! THE AUTHORITY (R-CHAR-3): `.design/lower/verus-lowering.md` REQ-5 — the
//! narrowing exists because Verus spec INTEGER arithmetic is the unbounded
//! `int` ("the surface integer set is `u32`/`u64`/`usize`"); a bool-typed
//! expression is NOT integer arithmetic, already has the callee's declared
//! `bool` param type, and per thermite-design.md §4.4 ("All conversions
//! explicit") must flow through UNCAST. Expected substrings are HAND-DERIVED
//! from the fixtures' `spec fn` signatures, never copied from lowerer output.
//!
//! Tracking: crosslink #233.

fn lower(src: &str) -> String {
    let parsed = thermite_syntax::parse(src);
    assert!(parsed.is_clean(), "fixture must parse clean: {parsed:?}");
    thermite_lower::lower(&parsed.program).expect("lowering must succeed")
}

/// A `bool`-param user spec fn called in a fn `req` with a bool-typed
/// comparison (`Expr::Binary`) argument at the bool position.
const BOOL_BINARY_PROGRAM: &str = "\
spec fn s_b(n: u32, b: bool) -> bool
  dec n
{
  if n == 0 { b } else { s_b(n - 1, b) }
}

fn f(x: u32, y: u32) -> u32
  req s_b(x, x < y)
  ens result == 0
  fx  pure
{
  0
}
";

#[test]
#[ignore = "divergence: bool-typed Binary spec-call arg cast `as u64` (spec_call_param_cast bool=>None collapses into the u64 fallback) -> E0308 -> L0; tracking #233"]
fn spec_call_bool_binary_arg_takes_no_cast() {
    let out = lower(BOOL_BINARY_PROGRAM);
    // REQ-5/§4.4: `x < y` is bool, matching `b: bool` exactly — no cast.
    assert!(
        out.contains("s_b(x, x < y)"),
        "a bool-typed Binary arg to a bool param must flow UNCAST \
         (verus rejects `(x < y) as u64` with E0308: expected bool, found u64):\n{out}"
    );
}

/// The unary twin: the surface unary set is exactly `!` (UnaryOp::Not), so
/// every `Expr::Unary` arg is bool-typed — yet the arithmetic-cast branch
/// matches `Expr::Unary` and casts it.
const BOOL_UNARY_PROGRAM: &str = "\
spec fn s_b(n: u32, b: bool) -> bool
  dec n
{
  if n == 0 { b } else { s_b(n - 1, b) }
}

fn f(flag: bool) -> u32
  req s_b(1, !flag)
  ens result == 0
  fx  pure
{
  0
}
";

#[test]
#[ignore = "divergence: bool-typed Unary (`!`) spec-call arg cast `as u64` -> E0308 -> L0; tracking #233"]
fn spec_call_bool_unary_arg_takes_no_cast() {
    let out = lower(BOOL_UNARY_PROGRAM);
    assert!(
        out.contains("s_b(1, !flag)"),
        "a `!`-typed Unary arg to a bool param must flow UNCAST \
         (verus rejects `(!flag) as u64` with E0308: expected bool, found u64):\n{out}"
    );
}
