//! Lexer + parser tests for the `?pN` proof-hole sigil (the Stage-1 forge tier,
//! `.design/stage1-forge-tier.md` REQ-3 / AC-7, increment 2a).
//!
//! The proof hole `?pN` rides the same `TokKind::Hole` machinery as the body hole
//! `?N` (#193), distinguished only by the `proof` discriminant. The parser pins
//! the two to their positions: a body hole `?N` to exec-fn-body statement
//! position, a proof hole `?pN` to a proof block — a `?pN` anywhere else is a
//! structured `SyntaxError::ProofHoleOutsideProofBlock` (AC-7). R-CHAR-3: the
//! expected token shapes are hand-derived from the grammar, not copied from the
//! lexer's output; `tests/` is ungated, so `unwrap`/`panic` are fine.

use thermite_syntax::{parse, tokenize, SyntaxError, TokKind};

/// The single `Hole` token a one-hole source lexes to, asserting zero
/// diagnostics. Returns `(number, proof)`.
fn lex_hole(src: &str) -> (u32, bool) {
    let (tokens, errors) = tokenize(src);
    assert!(
        errors.is_empty(),
        "expected clean lex of {src:?}, got diagnostics: {errors:?}"
    );
    match &tokens[0].kind {
        TokKind::Hole { number, proof } => (*number, *proof),
        other => panic!("expected a Hole token for {src:?}, got {other:?}"),
    }
}

#[test]
fn proof_hole_lexes_with_proof_discriminant_set() {
    // `?p3` → Hole { number: 3, proof: true } — the forge-tier proof sigil.
    assert_eq!(lex_hole("?p3"), (3, true));
    assert_eq!(lex_hole("?p0"), (0, true));
    assert_eq!(lex_hole("?p42"), (42, true));
}

#[test]
fn body_hole_lexes_with_proof_discriminant_clear() {
    // `?N` is unchanged (#193): the body sigil, proof = false.
    assert_eq!(lex_hole("?3"), (3, false));
    assert_eq!(lex_hole("?0"), (0, false));
}

#[test]
fn bare_proof_sigil_without_digits_is_a_stray_char() {
    // `?p` with no following digit is not a partial token — it is a stray `?`
    // diagnostic (REQ-8), then `p` lexes as an ordinary identifier.
    let (tokens, errors) = tokenize("?p");
    assert!(
        !errors.is_empty(),
        "expected a stray-char diagnostic for `?p`, got none; tokens: {tokens:?}"
    );
    assert!(matches!(errors[0], SyntaxError::StrayChar { .. }));
    // The `p` survives as an identifier (recovery past the stray `?`).
    assert!(
        tokens
            .iter()
            .any(|t| matches!(&t.kind, TokKind::Ident(s) if s == "p")),
        "expected `p` to lex as an identifier after the stray `?`; tokens: {tokens:?}"
    );
}

#[test]
fn digit_then_p_is_a_body_hole_then_ident() {
    // `?0p` is a body hole `?0` (proof = false) followed by the identifier `p`:
    // the `p` sigil is only recognized immediately after the `?`.
    let (tokens, errors) = tokenize("?0p");
    assert!(errors.is_empty(), "unexpected diagnostics: {errors:?}");
    assert!(matches!(
        tokens[0].kind,
        TokKind::Hole {
            number: 0,
            proof: false
        }
    ));
    assert!(matches!(&tokens[1].kind, TokKind::Ident(s) if s == "p"));
}

#[test]
fn proof_hole_in_fn_body_is_structured_error_not_a_body_hole() {
    // A `?pN` in fn-body statement position is rejected: proof holes live only in
    // proof blocks (AC-7). It is not silently reclassified as a body hole.
    let src = "fn f(x: u64) -> u64 req true ens result == x fx pure { ?p0 }";
    let result = parse(src);
    assert!(
        result
            .errors
            .iter()
            .any(|e| matches!(e, SyntaxError::ProofHoleOutsideProofBlock { number: 0, .. })),
        "expected ProofHoleOutsideProofBlock, got: {:?}",
        result.errors
    );
}

#[test]
fn body_hole_in_fn_body_still_parses_clean() {
    // Regression (#193): a body hole `?N` in fn-body statement position is still
    // accepted and recorded on the fn — the proof discriminant did not disturb it.
    let src = "fn f(x: u64) -> u64 req true ens result == x fx pure { ?0 }";
    let result = parse(src);
    assert!(
        result.is_clean(),
        "expected a clean parse of a body hole, got: {:?}",
        result.errors
    );
    let item = &result.program.items[0];
    let thermite_syntax::Item::Fn(f) = item else {
        panic!("expected a fn item, got {item:?}");
    };
    assert_eq!(f.holes.len(), 1);
    assert_eq!(f.holes[0].number, 0);
    assert_eq!(f.holes[0].context, thermite_syntax::HoleContext::Body);
}
