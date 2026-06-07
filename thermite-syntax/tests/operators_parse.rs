//! Lexer + parser tests for the OPERATOR + LITERAL layer — crosslink #92
//! (clusters 1-remainder + 2 of the primitive-completeness buildout).
//!
//! Covers the lexer additions (char/hex/binary literals → the SAME `Int` token,
//! `lexer.md` REQ-3/REQ-9 / AC-3/AC-6/AC-7/AC-8) and the parser additions (the
//! integer operators `% << >> & | ^` and the prefix `!`, with the pinned
//! standard-Rust precedence — `parser.md` REQ-8/REQ-9 / AC-5/AC-6/AC-7,
//! `surface-grammar.md` REQ-10).
//!
//! R-CHAR-3: the expected token VALUES are the design's symbolic constants (`'A'`
//! == the ASCII code 65, `0x1b` == 27, `0b101` == 5 — `lexer.md` AC-7/AC-8) and
//! the expected AST SHAPES are hand-derived from the grammar EBNF, NEVER copied
//! from the lexer/parser's own output. `tests/` is not gated, so `unwrap`/`expect`
//! are fine here.

use thermite_syntax::ast::{BinOp, Expr, Item, UnaryOp};
use thermite_syntax::{parse, tokenize, TokKind};

// ---------------------------------------------------------------------------
// Lexer: char / hex / binary literals → the SAME Int token (lexer.md REQ-3/REQ-9).
// ---------------------------------------------------------------------------

/// The single `Int` token a one-literal source lexes to, asserting zero
/// diagnostics. Returns `(value, raw)`.
fn lex_int_literal(src: &str) -> (u128, String) {
    let (tokens, errors) = tokenize(src);
    assert!(
        errors.is_empty(),
        "expected clean lex of {src:?}, got diagnostics: {errors:?}"
    );
    match &tokens[0].kind {
        TokKind::Int { value, raw } => (*value, raw.clone()),
        other => panic!("expected an Int token for {src:?}, got {other:?}"),
    }
}

#[test]
fn char_literal_lexes_to_byte_value_int_token() {
    // AC-8: `'A'` is the byte value 65 (the u8 char model), carried by the SAME
    // `Int` token as a numeric literal; raw is the verbatim `"'A'"`.
    assert_eq!(lex_int_literal("'A'"), (65, "'A'".to_string()));
    // `'\n'` == 10, `'\x1b'` == 27 (the shared escape table).
    assert_eq!(lex_int_literal(r"'\n'"), (10, r"'\n'".to_string()));
    assert_eq!(lex_int_literal(r"'\x1b'"), (27, r"'\x1b'".to_string()));
}

#[test]
fn hex_and_binary_literals_lex_to_decimal_value() {
    // AC-7: a hex/binary literal carries the SAME integer value as the decimal,
    // with the verbatim raw (prefix preserved, #37).
    assert_eq!(lex_int_literal("0x1b"), (27, "0x1b".to_string()));
    assert_eq!(lex_int_literal("0b101"), (5, "0b101".to_string()));
    assert_eq!(lex_int_literal("0xFF_FF"), (65535, "0xFF_FF".to_string()));
}

#[test]
fn shift_operators_are_one_token_each_maximal_munch() {
    // AC-3: `<<`/`>>` lex as ONE token each, NOT split into `<` `<` / `>` `>`.
    let (lt, _) = tokenize("<<");
    assert_eq!(lt[0].kind, TokKind::Shl, "`<<` must be one Shl token");
    assert_eq!(lt.len(), 2, "`<<` is one token + Eof, got {lt:?}");
    let (gt, _) = tokenize(">>");
    assert_eq!(gt[0].kind, TokKind::Shr, "`>>` must be one Shr token");
    // `>=` stays distinct (the second byte differs from `>>`).
    let (ge, _) = tokenize(">=");
    assert_eq!(ge[0].kind, TokKind::Ge, "`>=` must stay one Ge token");
}

#[test]
fn percent_and_caret_lex_as_single_char_tokens() {
    // REQ-6 (#92): `%`/`^` are new single-char operator tokens.
    let (p, _) = tokenize("%");
    assert_eq!(p[0].kind, TokKind::Percent);
    let (c, _) = tokenize("^");
    assert_eq!(c[0].kind, TokKind::Caret);
}

#[test]
fn malformed_literals_are_structured_diagnostics_not_panic() {
    // AC-6: `''` (empty), `'AB'` (multi-char), `0x` (no hex digit), `0b2` (no
    // binary digit), and a non-ASCII `'é'` each yield a diagnostic, never a panic.
    for bad in ["''", "'AB'", "0x", "0b2", "'é'"] {
        let (_tokens, errors) = tokenize(bad);
        assert!(
            !errors.is_empty(),
            "malformed literal {bad:?} must produce a SyntaxError diagnostic (AC-6)"
        );
    }
}

// ---------------------------------------------------------------------------
// Parser: operator shapes + the pinned precedence (parser.md REQ-8, AC-5/AC-6).
// ---------------------------------------------------------------------------

/// Parse a `fn` whose single `ens` is `result == <EXPR>` and return the rhs
/// `<EXPR>` of that top-level `==`. The harness pins operator/precedence shapes
/// without a standalone expression entry point.
fn ens_rhs(expr_src: &str) -> Expr {
    let src = format!(
        "fn f(a: u64, b: u64, c: u64) -> u64 req true ens result == {expr_src} fx pure {{ a }}"
    );
    let r = parse(&src);
    assert!(
        r.is_clean(),
        "expected clean parse of {src:?}, got {:?}",
        r.errors
    );
    let Item::Fn(f) = &r.program.items[0] else {
        panic!("expected a fn item");
    };
    let ens = &f.contract.ens[0].expr;
    match ens {
        Expr::Binary {
            op: BinOp::Eq, rhs, ..
        } => (**rhs).clone(),
        other => panic!("expected `result == <expr>`, got {other:?}"),
    }
}

fn binop_of(e: &Expr) -> BinOp {
    match e {
        Expr::Binary { op, .. } => *op,
        other => panic!("expected a Binary, got {other:?}"),
    }
}

#[test]
fn each_new_operator_parses_to_its_binop_node() {
    // AC-5: each operator parses to the expected `Binary`/`Unary` node.
    assert_eq!(binop_of(&ens_rhs("a % b")), BinOp::Rem);
    assert_eq!(binop_of(&ens_rhs("a << b")), BinOp::Shl);
    assert_eq!(binop_of(&ens_rhs("a >> b")), BinOp::Shr);
    assert_eq!(binop_of(&ens_rhs("a & b")), BinOp::BitAnd);
    assert_eq!(binop_of(&ens_rhs("a | b")), BinOp::BitOr);
    assert_eq!(binop_of(&ens_rhs("a ^ b")), BinOp::BitXor);
    // `!a` → Unary { Not, .. }.
    match ens_rhs("!a") {
        Expr::Unary {
            op: UnaryOp::Not, ..
        } => {}
        other => panic!("expected `Unary {{ Not }}`, got {other:?}"),
    }
}

#[test]
fn modulo_binds_tighter_than_add() {
    // AC-6: `a % b + 1` groups as `(a % b) + 1` — the top op is `Add`, its lhs is
    // the `Rem` (the `%` is tighter than `+`).
    let e = ens_rhs("a % b + 1");
    let Expr::Binary {
        op: BinOp::Add,
        lhs,
        ..
    } = &e
    else {
        panic!("expected top-level Add for `a % b + 1`, got {e:?}");
    };
    assert_eq!(binop_of(lhs), BinOp::Rem, "`%` must bind tighter than `+`");
}

#[test]
fn shift_binds_looser_than_add() {
    // AC-6: `a + b << c` groups as `(a + b) << c` — shifts are below `+ -`. The
    // top op is `Shl`, its lhs is the `Add`.
    let e = ens_rhs("a + b << c");
    let Expr::Binary {
        op: BinOp::Shl,
        lhs,
        ..
    } = &e
    else {
        panic!("expected top-level Shl for `a + b << c`, got {e:?}");
    };
    assert_eq!(binop_of(lhs), BinOp::Add, "shift must bind looser than `+`");
}

#[test]
fn not_binds_tighter_than_bitand() {
    // AC-6: `!a & b` groups as `(!a) & b` — prefix `!` is tighter than every
    // binary. The top op is `BitAnd`, its lhs is the `Unary`.
    let e = ens_rhs("!a & b");
    let Expr::Binary {
        op: BinOp::BitAnd,
        lhs,
        ..
    } = &e
    else {
        panic!("expected top-level BitAnd for `!a & b`, got {e:?}");
    };
    match lhs.as_ref() {
        Expr::Unary {
            op: UnaryOp::Not, ..
        } => {}
        other => panic!("expected `(!a)` as the BitAnd lhs, got {other:?}"),
    }
}

#[test]
fn bitand_binds_tighter_than_bitor() {
    // AC-6: `a & b | c` groups as `(a & b) | c` — `&` (tier 4) tighter than `|`
    // (tier 6). The top op is `BitOr`, its lhs is the `BitAnd`.
    let e = ens_rhs("a & b | c");
    let Expr::Binary {
        op: BinOp::BitOr,
        lhs,
        ..
    } = &e
    else {
        panic!("expected top-level BitOr for `a & b | c`, got {e:?}");
    };
    assert_eq!(
        binop_of(lhs),
        BinOp::BitAnd,
        "`&` must bind tighter than `|`"
    );
}

#[test]
fn binary_pipe_distinct_from_closure_pipe() {
    // AC-6: `a | b` parses as `BitOr` (operator position), while `|x| x` opens a
    // closure (in Primary) — disambiguated by position.
    assert_eq!(binop_of(&ens_rhs("a | b")), BinOp::BitOr);
    match ens_rhs("forall_in(a, |x| x)") {
        Expr::Call { .. } => {} // the closure parses as a call arg, no error
        other => panic!("expected a Call with a closure arg, got {other:?}"),
    }
}

#[test]
fn char_hex_binary_parse_to_intlit_no_new_variant() {
    // AC-7 / ast.md AC-1c: `'A'`/`0x1b`/`0b101` each parse to `Expr::IntLit` (the
    // SAME node as a decimal), carrying the byte/radix value + verbatim raw.
    for (src, value, raw) in [
        ("'A'", 65u128, "'A'"),
        ("0x1b", 27, "0x1b"),
        ("0b101", 5, "0b101"),
    ] {
        match ens_rhs(src) {
            Expr::IntLit { value: v, raw: r } => {
                assert_eq!(v, value, "{src} value");
                assert_eq!(r, raw, "{src} raw (#37 verbatim)");
            }
            other => panic!("expected IntLit for {src}, got {other:?}"),
        }
    }
}
