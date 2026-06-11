//! Pinning regression for crosslink #237 (gap 1 — the int-literal return-typing
//! gap, `thermite-lower/src/lower.rs`). A recursive `spec fn` over a sized integer
//! return whose body's RESULT position is integer-literal arithmetic
//! (`if n == 0 { 0 } else { 1 + count(n - 1) }`) lowered the else-arm as
//! `1 + count((n - 1) as u64)`, which Verus types as the UNBOUNDED `int` against
//! the declared `u64` return → E0308 (`expected u64, found int`) → L0 on legitimate
//! frozen-subset source.
//!
//! THE AUTHORITY (R-CHAR-3): Verus spec arithmetic is the unbounded `int`, so
//! `<int-literal> + <u64>` evaluates to `int` and the body must narrow back to the
//! declared sized-int return (`.design/lower/verus-lowering.md` REQ-5, "the cast
//! `i as int` is mandatory — Verus spec indices are `int`"; same fidelity class as
//! the #225/#229 narrowing casts — identity on the spec domain for in-range
//! values). The match-form `spec_sum`/ADT folds take the `nat`-return path (casts
//! coerce `as nat` uniformly) and certify; the if-form/int-literal shape is the
//! gap. The expected `(... ) as u64`/`as u32`/`as usize` substrings below are
//! HAND-DERIVED from each fixture's DECLARED return type, NOT copied from the
//! lowerer.
//!
//! REGRESSION GUARD: `spec_line_start` (a `-> u64` if-form spec fn whose result
//! arms are `acc` / recursive calls — NO result-position arithmetic) must NOT gain
//! a narrowing cast (it is already `u64`-typed); byte-stability is pinned below.

use thermite_syntax::ast::{Expr, Item};

fn lower(src: &str) -> String {
    let parsed = thermite_syntax::parse(src);
    assert!(parsed.is_clean(), "fixture must parse clean: {parsed:?}");
    thermite_lower::lower(&parsed.program).expect("L3 lowering")
}

/// Non-vacuity helper: the parsed program genuinely contains a `spec fn` whose
/// RESULT position is integer-literal arithmetic (the `1 + f(n - 1)` shape the
/// narrowing fires on — else the test is vacuous).
fn has_int_literal_arith_result(src: &str) -> bool {
    fn result_arith(e: &Expr) -> bool {
        match e {
            Expr::If { then, else_, .. } => {
                then.tail.as_deref().map(result_arith).unwrap_or(false)
                    || else_.tail.as_deref().map(result_arith).unwrap_or(false)
            }
            // `1 + f(n - 1)` — an Add over an int-literal operand at a result leaf.
            Expr::Binary { lhs, rhs, .. } => mentions_lit(lhs) || mentions_lit(rhs),
            _ => false,
        }
    }
    fn mentions_lit(e: &Expr) -> bool {
        match e {
            Expr::IntLit { .. } => true,
            Expr::Binary { lhs, rhs, .. } => mentions_lit(lhs) || mentions_lit(rhs),
            Expr::Unary { expr, .. } | Expr::Cast { expr, .. } => mentions_lit(expr),
            Expr::Call { args, .. } => args.iter().any(mentions_lit),
            _ => false,
        }
    }
    let parsed = thermite_syntax::parse(src);
    parsed.program.items.iter().any(|i| match i {
        Item::SpecFn(s) => s.body.tail.as_deref().map(result_arith).unwrap_or(false),
        _ => false,
    })
}

const COUNT_U64: &str = "\
spec fn count(n: u64) -> u64
  dec n
{
  if n == 0 {
    0
  } else {
    1 + count(n - 1)
  }
}
";

const COUNT_U32: &str = "\
spec fn count(n: u32) -> u32
  dec n
{
  if n == 0 {
    0
  } else {
    1 + count(n - 1)
  }
}
";

const COUNT_USIZE: &str = "\
spec fn count(n: usize) -> usize
  dec n
{
  if n == 0 {
    0
  } else {
    1 + count(n - 1)
  }
}
";

// A `-> u64` if-form spec fn whose result arms are a bare path (`acc`) and
// recursive calls — NO result-position arithmetic. It is ALREADY `u64`-typed and
// must NOT gain a narrowing cast (byte-stability — the `spec_line_start` shape).
const NO_ARITH_RESULT: &str = "\
spec fn pick(n: u64, acc: u64) -> u64
  dec n
{
  if n == 0 {
    acc
  } else {
    pick(n - 1, acc)
  }
}
";

#[test]
fn u64_int_literal_arith_result_narrows_as_u64() {
    assert!(
        has_int_literal_arith_result(COUNT_U64),
        "fixture must have an int-literal arith result (else vacuous)"
    );
    let out = lower(COUNT_U64);
    // The body's `int`-typed result `1 + count((n - 1) as u64)` narrows back to the
    // declared `u64` return — `(1 + count((n - 1) as u64)) as u64`.
    assert!(
        out.contains(") as u64"),
        "a u64-return int-literal-arith spec fn must narrow its result `as u64`:\n{out}"
    );
    assert!(
        out.contains("1 + count((n - 1) as u64)"),
        "the recursive arith arg keeps the #225 param-type cast under the narrow:\n{out}"
    );
}

#[test]
fn u32_int_literal_arith_result_narrows_as_u32() {
    assert!(has_int_literal_arith_result(COUNT_U32));
    let out = lower(COUNT_U32);
    assert!(
        out.contains(") as u32"),
        "a u32-return int-literal-arith spec fn must narrow its result `as u32`:\n{out}"
    );
    assert!(
        !out.contains(") as u64"),
        "a u32 return must NOT narrow to u64:\n{out}"
    );
}

#[test]
fn usize_int_literal_arith_result_narrows_as_usize() {
    assert!(has_int_literal_arith_result(COUNT_USIZE));
    let out = lower(COUNT_USIZE);
    assert!(
        out.contains(") as usize"),
        "a usize-return int-literal-arith spec fn must narrow its result `as usize`:\n{out}"
    );
}

#[test]
fn no_arith_result_spec_fn_is_not_narrowed() {
    // Non-vacuity: this fixture must NOT have a result-position arithmetic leaf.
    assert!(
        !has_int_literal_arith_result(NO_ARITH_RESULT),
        "the guard fixture must have NO int-literal arith result (else vacuous)"
    );
    let out = lower(NO_ARITH_RESULT);
    // No result-position arithmetic → already `u64`-typed → NO result-narrowing
    // wrap added (the `spec_line_start` byte-stability guard). The result-narrow is
    // a `(<body-if>) as u64` WRAP of the whole `if` — distinct from the inner #225
    // param-type arg cast `pick((n - 1) as u64, acc)` (which IS present + correct).
    // Pin on the WRAP shape, not the substring `) as u64` (which the inner arg cast
    // also contains).
    assert!(
        !out.contains("(if "),
        "a spec fn whose result is a path / recursive call must NOT gain a `(if ...) as` result-narrow:\n{out}"
    );
}
