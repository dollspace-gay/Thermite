//! Tests for the forge-tier `dec` measure forms `dec lex(...)` and `dec wf <rel>`
//! (`.design/stage1-forge-tier.md` REQ-3, Q-DECWF, increment 2a).
//!
//! Both are registry-free (like the `forall_in`/`sorted` combinators): `dec
//! lex(...)` is already an ordinary `Expr::Call`, and `dec wf <rel>` normalizes to
//! the call `wf(<rel>)` — the lexer stays ASCII-only (no Unicode `⟨⟩` operator).
//! The v1 plain `dec <expr>` is unchanged. R-CHAR-3: shapes hand-derived from the
//! grammar; `tests/` is ungated.

use thermite_syntax::{parse, Expr, Item};

/// Parse `src` cleanly and return the single spec fn's `dec` clause.
fn spec_fn_dec(src: &str) -> thermite_syntax::Clause {
    let result = parse(src);
    assert!(
        result.is_clean(),
        "expected a clean parse of {src:?}, got: {:?}",
        result.errors
    );
    match result.program.items.into_iter().next().unwrap() {
        Item::SpecFn(s) => s.dec,
        other => panic!("expected a spec fn, got {other:?}"),
    }
}

/// The callee name + arg count of an `Expr::Call` whose callee is a single-segment
/// path.
fn call_shape(expr: &Expr) -> (String, usize) {
    match expr {
        Expr::Call { callee, args } => match callee.as_ref() {
            Expr::Path(segs) if segs.len() == 1 => (segs[0].clone(), args.len()),
            other => panic!("expected a single-segment path callee, got {other:?}"),
        },
        other => panic!("expected an Expr::Call, got {other:?}"),
    }
}

#[test]
fn dec_lex_parses_as_a_plain_call_registry_free() {
    // `dec lex(n, n)` — a lexicographic tuple is an ordinary `Expr::Call` (no
    // special parse; the consumer keys on the `lex` callee).
    let dec = spec_fn_dec("spec fn f(n: u64) -> u64 dec lex(n, n) { n }");
    assert_eq!(call_shape(&dec.expr), ("lex".to_string(), 2));
    assert_eq!(dec.text, "lex(n, n)");
}

#[test]
fn dec_wf_normalizes_to_a_wf_call() {
    // `dec wf lt` (ASCII spelling, Q-DECWF) — normalized to the registry-free call
    // `wf(lt)` so downstream sees an ordinary `Expr::Call`.
    let dec = spec_fn_dec("spec fn f(n: u64) -> u64 dec wf lt { n }");
    assert_eq!(call_shape(&dec.expr), ("wf".to_string(), 1));
    // The verbatim text preserves the surface `wf <rel>` spelling.
    assert_eq!(dec.text, "wf lt");
}

#[test]
fn dec_wf_relation_can_be_a_compound_expression() {
    let dec = spec_fn_dec("spec fn f(n: u64) -> u64 dec wf less_than(n) { n }");
    assert_eq!(call_shape(&dec.expr), ("wf".to_string(), 1));
}

#[test]
fn plain_dec_measure_is_unchanged() {
    // The v1 plain measure `dec n` still parses to the bare expression (no `wf`/
    // `lex` wrapping) — byte-stable.
    let dec = spec_fn_dec("spec fn f(n: u64) -> u64 dec n { n }");
    match &dec.expr {
        Expr::Path(segs) if segs == &vec!["n".to_string()] => {}
        other => panic!("expected the bare measure `n`, got {other:?}"),
    }
    assert_eq!(dec.text, "n");
}

#[test]
fn dec_wf_in_fn_position_parses() {
    // A recursive exec `fn` may carry `dec wf <rel>` (optional dec slot, after fx).
    let src = "fn f(n: u64) -> u64 req true ens result == n fx pure dec wf lt { n }";
    let result = parse(src);
    assert!(result.is_clean(), "parse errors: {:?}", result.errors);
    let Item::Fn(f) = &result.program.items[0] else {
        panic!("expected a fn");
    };
    let dec = f.dec.as_ref().expect("fn should carry a dec");
    assert_eq!(call_shape(&dec.expr), ("wf".to_string(), 1));
}
