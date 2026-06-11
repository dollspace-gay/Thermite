//! Divergence pin (crosslink #231) — the #225 spec-call declared-param-type
//! narrowing is SKIPPED when the argument's lowered text happens to END with
//! `as <cast>`, even though the WHOLE argument is still the unbounded spec
//! `int`. The dedupe heuristic at every `spec_call_param_cast` consumer
//! (`lower_expr` `Call` arm, twice, and the c116360c `lower_inv_expr` `Call`
//! arm) is textual:
//!
//!     if lowered.ends_with(&format!("as {cast}")) { skip the cast }
//!
//! For `req s_dec(k + j as u32)` the arg lowers to `k + j as u32`; `as` binds
//! tighter than `+`, so this is `k + (j as u32)` — in Verus spec position
//! `u32 + u32` widens to `int`, the suffix check matches `as u32`, the
//! narrowing is skipped, and the emitted `s_dec(k + j as u32)` is ill-typed:
//! E0308 `expected u32, found int` → the whole item dies at L0 though the
//! Thermite source is correct (LIVE-CONFIRMED via `forge check`: `s_dec` L3,
//! `f` L0, E0308 on the requires line). The struct-inv twin (the c116360c
//! `Expr::Call` arm in `lower_inv_expr`) shares the heuristic verbatim and
//! emits `s_a(self.x + self.y as u32)` — same E0308 (verus-confirmed by hand).
//!
//! THE AUTHORITY (R-CHAR-3): `.design/lower/verus-lowering.md` REQ-5 — in spec
//! position integer arithmetic is the unbounded `int`; an ARITHMETIC argument
//! to a user spec fn must narrow to the callee's DECLARED param type, inner
//! parenthesized because `as` binds tighter than `+`/`-` (#122). The expected
//! emission `(k + j as u32) as u32` is HAND-DERIVED from `s_dec(n: u32)` —
//! never copied from the lowerer's output.

fn lower(src: &str) -> String {
    let parsed = thermite_syntax::parse(src);
    assert!(parsed.is_clean(), "fixture must parse clean: {parsed:?}");
    thermite_lower::lower(&parsed.program).expect("lowering must succeed")
}

/// A `u32`-param spec fn named in a fn `req` with an arithmetic arg whose
/// lowering textually ENDS in `as u32` (`k + j as u32` = `k + (j as u32)`,
/// still `int`-typed as a whole in Verus spec position).
const DEDUPE_REQ_PROGRAM: &str = "\
spec fn s_dec(n: u32) -> u32
  dec n
{
  if n == 0 {
    0
  } else {
    s_dec(n - 1)
  }
}

fn f(k: u32, j: u32) -> u32
  req s_dec(k + j as u32) == 0
  ens result == 0
  fx  pure
{
  0
}
";

#[test]
fn spec_call_arith_arg_ending_in_cast_text_still_narrows_to_declared_param_type() {
    let out = lower(DEDUPE_REQ_PROGRAM);
    // REQ-5: the Binary arg `k + j as u32` is `int` in spec position, so it
    // must STILL narrow to `s_dec`'s declared `u32` — `(k + j as u32) as u32`
    // — regardless of the textual suffix of its lowering.
    assert!(
        out.contains("s_dec((k + j as u32) as u32)"),
        "the dedupe suffix heuristic skipped the declared-param-type narrowing \
         (emitted arg stays `int`; E0308: expected u32, found int -> L0):\n{out}"
    );
}

/// The struct-inv twin: the same suffix-skip in the c116360c `lower_inv_expr`
/// `Expr::Call` arm.
const DEDUPE_INV_PROGRAM: &str = "\
spec fn s_a(n: u32) -> u32
  dec n
{
  if n == 0 {
    0
  } else {
    s_a(n - 1)
  }
}

struct Nest {
  x: u32,
  y: u32,
} inv s_a(x + y as u32) == 0
";

#[test]
fn struct_inv_spec_call_arith_arg_ending_in_cast_text_still_narrows() {
    let out = lower(DEDUPE_INV_PROGRAM);
    assert!(
        out.contains("s_a((self.x + self.y as u32) as u32)"),
        "the struct-inv Call arm's dedupe suffix heuristic skipped the \
         declared-param-type narrowing (verus rejects `s_a(self.x + self.y as \
         u32)` with E0308: expected u32, found int):\n{out}"
    );
}
