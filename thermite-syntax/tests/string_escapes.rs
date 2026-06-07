//! Lexer string-escape tests — crosslink #91 cluster 1 (the LITERAL LAYER), gap 1
//! (string escapes). The v0.1 `lex_string` escape table was only `\n`/`\t`/`\"`/
//! `\\`; its catch-all `other => other as char` SILENTLY swallowed `\x`/`\r`/`\0`
//! (e.g. `\x1b` decoded to the char `x` then literal `1b`), so `"\x1b".byte_at(0)`
//! could never equal 27. This stage extends the escape table to `\r`/`\0`/`\xNN`
//! and makes an UNKNOWN/malformed escape a STRUCTURED diagnostic, never a swallow
//! and never a panic (`.design/syntax/lexer.md` REQ-4 / REQ-8,
//! `.design/basis/07-strings.md` REQ-6).
//!
//! R-CHAR-3: the expected BYTES are the ANSI/ASCII control-code constants (ESC ==
//! 27, CR == 13, NUL == 0, the literal `'A'` == 65), NOT values copied from the
//! lexer's own output. `tests/` is not gated, so `unwrap`/`expect` are fine here.

use thermite_syntax::{tokenize, TokKind};

/// Lex a single `"…"` source and return the decoded string content, asserting
/// the lexer produced exactly one `Str` token with zero diagnostics.
fn decode(src: &str) -> Vec<u8> {
    let (tokens, errors) = tokenize(src);
    assert!(
        errors.is_empty(),
        "expected clean lex of {src:?}, got diagnostics: {errors:?}"
    );
    // tokens: [Str(..), Eof]
    match &tokens[0].kind {
        TokKind::Str(s) => s.as_bytes().to_vec(),
        other => panic!("expected a Str token, got {other:?}"),
    }
}

#[test]
fn escape_x1b_decodes_to_esc_byte_27() {
    // ANSI escape introducer ESC == 0x1b == 27 — the editor's headline unblock.
    assert_eq!(decode(r#""\x1b""#), vec![27]);
}

#[test]
fn escape_cr_decodes_to_byte_13() {
    // Carriage return CR == 13.
    assert_eq!(decode(r#""\r""#), vec![13]);
}

#[test]
fn escape_nul_decodes_to_byte_0() {
    // NUL == 0.
    assert_eq!(decode(r#""\0""#), vec![0]);
}

#[test]
fn existing_escapes_unchanged_no_regression() {
    // The v0.1 escapes still decode to their control/literal bytes: LF == 10,
    // TAB == 9, `"` == 34, `\` == 92.
    assert_eq!(decode(r#""\n""#), vec![10]);
    assert_eq!(decode(r#""\t""#), vec![9]);
    assert_eq!(decode(r#""\"""#), vec![34]);
    assert_eq!(decode(r#""\\""#), vec![92]);
}

#[test]
fn escape_x_nn_full_ascii_range_is_byte_faithful() {
    // Every `\xNN` in 0x00..=0x7F materializes to exactly byte NN (single UTF-8
    // byte). A representative sweep across the range.
    assert_eq!(decode(r#""\x00""#), vec![0x00]);
    assert_eq!(decode(r#""\x09""#), vec![0x09]);
    assert_eq!(decode(r#""\x41""#), vec![0x41]); // 'A'
    assert_eq!(decode(r#""\x7f""#), vec![0x7f]);
}

#[test]
fn escapes_compose_in_a_mixed_literal() {
    // `"a\x1b[0m\n"` — a realistic ANSI reset sequence: 'a', ESC, '[', '0', 'm', LF.
    assert_eq!(
        decode(r#""a\x1b[0m\n""#),
        vec![b'a', 27, b'[', b'0', b'm', 10]
    );
}

#[test]
fn malformed_hex_escape_is_structured_diagnostic_not_panic() {
    // `\xZZ` — `Z` is not a hex digit: a structured StrayChar diagnostic, never a
    // panic and never a silent swallow (lexer.md REQ-8).
    let (_tokens, errors) = tokenize(r#""\xZZ""#);
    assert!(
        !errors.is_empty(),
        "a malformed `\\xZZ` escape must produce a diagnostic"
    );
}

#[test]
fn high_byte_hex_escape_is_rejected_in_v1_byte_model() {
    // `\x80`..=`\xFF` is NOT a single UTF-8 byte in a Rust `String`, so v1 (the
    // byte char model, 07-strings.md REQ-2) rejects it structurally rather than
    // mis-materialize it to two bytes (it awaits the Vec<u8> string-content
    // reshape). A structured diagnostic, never a panic.
    let (_tokens, errors) = tokenize(r#""\xC3""#);
    assert!(
        !errors.is_empty(),
        "a high-byte `\\xC3` (>= 0x80) must be a structured diagnostic in v1"
    );
}

#[test]
fn unknown_escape_is_structured_diagnostic_not_silent_swallow() {
    // `\z` is an unknown escape: a structured diagnostic, NOT the v0.1
    // `other as char` swallow (which would have decoded `\z` to `z`).
    let (_tokens, errors) = tokenize(r#""\z""#);
    assert!(
        !errors.is_empty(),
        "an unknown `\\z` escape must produce a diagnostic, not silently swallow"
    );
}
