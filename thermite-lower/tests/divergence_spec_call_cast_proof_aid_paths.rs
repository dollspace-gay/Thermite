//! Divergence pins (crosslink #229) — the #225/#227 type-directed spec-call
//! narrowing cast (`Ctx::spec_call_param_cast` fed by `spec_fn_param_type_map`)
//! is STILL not threaded to every spec-context lowering site. #227 (commit
//! `1c59e4fd`) claims "narrowing target = callee's declared param type at ALL
//! six spec-context entry points", but the PROOF-AID emission paths re-lower
//! contract clauses with a bare un-threaded `Ctx::spec_seq()`:
//!
//!   1. `nonlinear_overflow_assert` (the §sum overflow discharge,
//!      verus-lowering.md REQ-7) re-lowers the fn's `req` clause as a
//!      `by(nonlinear_arith) requires` hypothesis via `Ctx::spec_seq()` with NO
//!      `.with_spec_fn_param_types(..)` — a `u32`-param spec fn named in the
//!      `req` with an arithmetic arg emits the hardcoded `as u64` fallback in
//!      the emitted assert (`s_dec((k + 0) as u64)`), ill-typed Verus
//!      (`expected u32, found u64`, E0308) → the whole item dies at L0 though
//!      the Thermite source is correct AND the same `req` lowers correctly
//!      (`as u32`) in the signature `requires` AND the lifted loop invariant
//!      two lines above (LIVE-CONFIRMED via `forge check`: `s_dec` L3,
//!      `sum_b` L0, E0308 on the requires hypothesis). Tracking #229.
//!
//!   2. `lower_inv_expr` (the struct-invariant / `well_formed()` predicate
//!      lowering, verus-lowering.md REQ-8) routes a non-method `Expr::Call`
//!      through its catch-all `lower_expr(expr, Ctx::spec_seq(), ..)` — same
//!      un-threaded `as u64` fallback for a spec-call arithmetic arg in a
//!      struct `inv`, AND the catch-all drops the REQ-8 `self.<field>` rewrite
//!      for the call's args (the emitted `well_formed` body references a bare
//!      `x` — unresolvable). Tracking #229.
//!
//! THE AUTHORITY (R-CHAR-3): `.design/lower/verus-lowering.md` REQ-5 — spec
//! position is unbounded `int`; the narrowing target is the callee's DECLARED
//! param type over the surface integer set `u32`/`u64`/`usize` (the #225 rule,
//! re-affirmed by #227's own commit message) — and REQ-8 (struct-inv field
//! rewrite). Expected substrings are HAND-DERIVED from the fixtures' declared
//! param types (`s_dec(n: u32)` → `as u32`), never copied from the lowerer's
//! output.
//!
//! POST-FIX NOTE (c116360c re-audit): the fixture carries the `xs.len() <=
//! 1_000_000` length bound so the EXACT pinned program is live-L3 end-to-end —
//! `forge check`: `s_dec` L3 (1 obligation), `sum_b` L3 (4 obligations
//! discharged). Without the bound the `ens` overflow obligation `result <=
//! xs.len() * u32::MAX` genuinely fails for an unbounded `xs` (a fixture
//! authoring gap, not a toolchain divergence — the c116360c fixer's
//! escalation (1), critic-confirmed).

fn lower(src: &str) -> String {
    let parsed = thermite_syntax::parse(src);
    assert!(parsed.is_clean(), "fixture must parse clean: {parsed:?}");
    thermite_lower::lower(&parsed.program).expect("lowering must succeed")
}

/// A `u32`-param recursive spec fn + a sum-shaped exec fn (accumulator growth
/// `acc = acc + xs[i] as u64` + product-bound invariant — the EXACT shape that
/// fires `nonlinear_overflow_assert`) whose `req` names the spec fn with an
/// arithmetic argument. The `xs.len()` bound makes the whole program live-L3
/// (see POST-FIX NOTE above).
const NONLINEAR_REQ_PROGRAM: &str = "\
spec fn s_dec(n: u32) -> u32
  dec n
{
  if n == 0 {
    0
  } else {
    s_dec(n - 1)
  }
}

fn sum_b(xs: &[u32], k: u32) -> u64
  req s_dec(k + 0) == 0 && xs.len() <= 1_000_000
  ens result <= xs.len() as u64 * u32::MAX as u64
  fx  pure
{
  let mut acc: u64 = 0;
  let mut i: usize = 0;
  while i < xs.len()
    inv i <= xs.len()
    inv acc <= i as u64 * u32::MAX as u64
    dec xs.len() - i
  {
    acc = acc + xs[i] as u64;
    i = i + 1;
  }
  acc
}
";

#[test]
fn nonlinear_overflow_assert_req_hypothesis_casts_to_declared_param_type() {
    let out = lower(NONLINEAR_REQ_PROGRAM);
    // Precondition of the pin: the overflow proof-aid actually fired (the
    // fixture matches the accumulator-growth + product-bound shape).
    assert!(
        out.contains("by(nonlinear_arith)"),
        "fixture must fire the nonlinear overflow assert:\n{out}"
    );
    // The callee `s_dec` declares a `u32` param, so EVERY lowering of the req's
    // `s_dec(k + 0)` must narrow `as u32` (verus-lowering.md REQ-5 / #225 rule).
    // The signature `requires` already does; the re-lowered hypothesis inside
    // the `by(nonlinear_arith) requires` must match or the item is ill-typed.
    assert!(
        !out.contains("s_dec((k + 0) as u64)"),
        "the nonlinear_overflow_assert req-hypothesis path still hardcodes the \
         #225 `as u64` fallback (E0308: expected u32, found u64 → L0):\n{out}"
    );
}

/// A `u32`-param spec fn named (with an arithmetic arg over a field) in a
/// STRUCT invariant — the `lower_inv_expr` / `well_formed()` path.
const STRUCT_INV_PROGRAM: &str = "\
spec fn s_dec(n: u32) -> u32
  dec n
{
  if n == 0 {
    0
  } else {
    s_dec(n - 1)
  }
}

struct Counter {
  x: u32,
} inv s_dec(x + 0) == 0
";

#[test]
fn struct_invariant_spec_call_arith_arg_casts_to_declared_param_type() {
    let out = lower(STRUCT_INV_PROGRAM);
    assert!(
        !out.contains("s_dec((x + 0) as u64)") && !out.contains("s_dec((self.x + 0) as u64)"),
        "the struct-invariant (lower_inv_expr) path still hardcodes the #225 \
         `as u64` fallback:\n{out}"
    );
    // The struct-inv field rewrite must also survive the call: `x` is a FIELD,
    // so the well_formed body must reference `self.x` (REQ-8), narrowed to the
    // declared `u32`.
    assert!(
        out.contains("s_dec((self.x + 0) as u32)"),
        "a u32-param spec fn's struct-inv arith arg must be `(self.x + 0) as u32` \
         (REQ-8 field rewrite + REQ-5 declared-param-type cast):\n{out}"
    );
}
