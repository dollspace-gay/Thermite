//! Adversarial audit of #37 — Expr-level verbatim int-literal preservation
//! (lexer + AST round-trip half; the lowering "emits value, not raw" probe
//! lives in `thermite-lower/tests/divergence_intlit_lower.rs` since
//! thermite-syntax is a leaf crate and cannot depend on thermite-lower).
//!
//! The invariant under audit (commit a2c0f73): reshaping `TokKind::Int` /
//! `Expr::IntLit` to struct variants `{ value: u128, raw: String }` is PURELY
//! ADDITIVE — `value` is the `_`-stripped number (the original, UNCHANGED
//! semantics) and `raw` is the verbatim source slice.
//!
//! These tests pin the authority's expected round-trip
//! (`.design/syntax/lexer.md` REQ-3 / AC-2b, `.design/syntax/ast.md` REQ-6 /
//! AC-1b). Expected values are hand-derived from the cited REQ/AC text and the
//! design constant `1_000_000` (Appendix A), NEVER copied from the toolchain's
//! own output (goal.md R-CHAR-3).
//!
//! NOTE FROM THE CRITIC: every assertion below is expected to PASS under
//! a2c0f73 — they document that NO divergence exists across the edge cases the
//! spec names but the pre-existing `tests/conformance.rs` battery did not cover
//! (`0`, multi-underscore `1_2_3`, multi-`_`-with-trailing). If any FAILS, the
//! additive invariant is broken.

use thermite_syntax::{tokenize, TokKind};

/// Extract the first `Int` token's `(value, raw)` from a source string.
fn first_int(src: &str) -> Option<(u128, String)> {
    let (tokens, errors) = tokenize(src);
    assert!(
        errors.is_empty(),
        "lexing {src:?} produced errors: {errors:?}"
    );
    tokens.iter().find_map(|t| match &t.kind {
        TokKind::Int { value, raw } => Some((*value, raw.clone())),
        _ => None,
    })
}

/// lexer.md AC-2b: `0` lexes to value `0` AND raw `"0"` (a separator-free
/// literal; the spec names `0` explicitly). This edge case is NOT covered by
/// `tests/conformance.rs::int_literal_preserves_raw` (which probes `1_000_000`,
/// `42`, `1_`). Expected hand-derived from AC-2b ("A literal with no separators
/// (`0`) has `raw == "0"`").
#[test]
fn divergence_intlit_zero_value_and_raw() {
    assert_eq!(first_int("0"), Some((0u128, "0".to_string())));
}

/// lexer.md REQ-3: a multi-underscore literal `1_2_3` has value `123` (all
/// interior `_` stripped) AND raw `"1_2_3"` (every interior `_` preserved
/// verbatim). Expected hand-derived from REQ-3 ("the `_` separators are
/// removed" for value; "the exact source substring … separators included" for
/// raw).
#[test]
fn divergence_intlit_multi_underscore_value_and_raw() {
    assert_eq!(first_int("1_2_3"), Some((123u128, "1_2_3".to_string())));
}

/// lexer.md REQ-3 / AC-2b: a trailing `_` adjacent to a multi-`_` run is in
/// NEITHER value nor raw — both end at the last digit. `1_000_` lexes to value
/// `1000` and raw `"1_000"` (the trailing `_` is dropped from raw; interior
/// `_` kept). Expected hand-derived from REQ-3 ("A trailing/leading `_` …
/// excluded from BOTH the value and the raw (the raw ends at the last digit)").
#[test]
fn divergence_intlit_trailing_underscore_excluded_from_raw() {
    assert_eq!(first_int("1_000_"), Some((1_000u128, "1_000".to_string())));
}

/// lexer.md REQ-3: `raw` equals the span's source slice by construction
/// (`source[span.start .. span.start + span.len]`). Pin that `raw` and the
/// recorded span stay consistent for a separator-bearing literal — a divergence
/// here would mean `raw` and `span` describe different substrings. Expected
/// derived from REQ-3 ("The raw is exactly the span's source slice").
#[test]
fn divergence_intlit_raw_matches_span_slice() {
    let src = "1_000_000";
    let (tokens, _) = tokenize(src);
    let tok = tokens
        .iter()
        .find(|t| matches!(t.kind, TokKind::Int { .. }))
        .expect("expected an Int token");
    let TokKind::Int { raw, .. } = &tok.kind else {
        unreachable!()
    };
    let span_slice = &src[tok.span.start..tok.span.start + tok.span.len];
    assert_eq!(raw, span_slice, "raw must equal the span's source slice");
    assert_eq!(raw, "1_000_000");
}

/// REQ-3 determinism (goal.md R-CODE-5): `raw` is captured from the source
/// slice, so lexing the same input twice yields identical tokens. Pin that
/// `tokenize` is a pure function of its input (no wall-clock / RNG leak into
/// the `raw` capture).
#[test]
fn divergence_intlit_raw_capture_deterministic() {
    let a = first_int("1_000_000");
    let b = first_int("1_000_000");
    assert_eq!(a, b);
    assert_eq!(a, Some((1_000_000u128, "1_000_000".to_string())));
}
