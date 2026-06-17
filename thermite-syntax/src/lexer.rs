//! Thermite lexer — a single-pass, hand-written scanner over the source `&str`
//! producing a flat `Vec<Token>` for the recursive-descent parser.
//!
//! Governing design: `.design/syntax/lexer.md`. Thermite has no significant
//! whitespace (§4.3); whitespace and `//` comments are insignificant separators
//! (REQ-5). The keyword set is exactly the surface-grammar terminals (REQ-2);
//! effect-row names (`read`, `write`, ...) and `slag`/`reason`/`owner`/`review`
//! are lexed as identifiers (contextual keywords, OQ-1). The scanner is
//! registry-free: `forall_in`, `sorted`, `len` are plain identifiers. Maximal
//! munch (REQ-6) picks the longest operator at each position. Errors are
//! `SyntaxError` values; the lexer never panics (REQ-8).
//!
//! ## REQ status
//!
//! <!-- generated:reqs view=thermite-syntax-lexer-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-SYNTAX-LEXER-CHAR-BYTE | shipped | `thermite-syntax/src/lexer.rs` | Lexer char literals as byte ints |  |
//! | REQ-SYNTAX-LEXER-INT-RADIX | shipped | `thermite-syntax/src/lexer.rs` | Lexer hexadecimal and binary integer spellings |  |
//! | REQ-SYNTAX-LEXER-INT-RAW | shipped | `thermite-syntax/src/lexer.rs` | Lexer integer literal raw spelling |  |
//! | REQ-SYNTAX-LEXER-INT-VALUE | shipped | `thermite-syntax/src/lexer.rs` | Lexer integer literal value |  |
//! | REQ-SYNTAX-LEXER-KEYWORDS | shipped | `thermite-syntax/src/lexer.rs` | Lexer reserved keyword set |  |
//! | REQ-SYNTAX-LEXER-MAXIMAL-MUNCH | shipped | `thermite-syntax/src/lexer.rs` | Lexer maximal-munch punctuation |  |
//! | REQ-SYNTAX-LEXER-RESULT-DISCIPLINE | shipped | `thermite-syntax/src/lexer.rs` | Lexer structured diagnostics |  |
//! | REQ-SYNTAX-LEXER-SLAG-STRINGS | shipped | `thermite-syntax/src/lexer.rs` | Lexer slag and string tokenization |  |
//! | REQ-SYNTAX-LEXER-SPANS | shipped | `thermite-syntax/src/lexer.rs` | Lexer source spans |  |
//! | REQ-SYNTAX-LEXER-TOKEN-SET | shipped | `thermite-syntax/src/lexer.rs` | Lexer token set |  |
//! | REQ-SYNTAX-LEXER-TRIVIA | shipped | `thermite-syntax/src/lexer.rs` | Lexer trivia skipping |  |
//! <!-- /generated:reqs -->
//!
//! ## `?N` hole token (`.design/forge/goal-repl.md` REQ-4, #193)
//!
//! The body-position structural hole `?N` lexes to a single `TokKind::Hole(N)`
//! token: the `'?'` branch in `tokenize` → `lex_hole` reads `?` + a run of ASCII
//! digits into the verbatim hole number (`?0` → `Hole(0)`). A bare `?` with no
//! following digit is a stray-char `SyntaxError` (REQ-8; Thermite has no
//! `?`-operator, §2.3), never a partial token, never a panic. The lexer lexes
//! `?N` anywhere the scanner sees it; the parser (`parser.md` REQ-11) restricts a
//! hole to fn-body statement position (a `?N` in expression / clause / signature
//! position is a parse error). Consumer: `parse_block`'s statement dispatch in
//! `parser.rs`.

use crate::parser::SyntaxError;

/// A source span: a byte offset and byte length into the original source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub len: usize,
}

impl Span {
    /// Construct a span from start/len byte offsets.
    pub fn new(start: usize, len: usize) -> Self {
        Span { start, len }
    }

    /// The byte offset one past the end of this span.
    pub fn end(&self) -> usize {
        self.start + self.len
    }

    /// The smallest span covering both `self` and `other`.
    pub fn to(&self, other: Span) -> Span {
        let start = self.start.min(other.start);
        let end = self.end().max(other.end());
        Span::new(start, end - start)
    }
}

/// A lexical token: a kind plus the source span it covers (lexer.md REQ-7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokKind,
    pub span: Span,
}

/// The kinds of token the lexer produces (lexer.md REQ-1). The closed reserved
/// keyword set is REQ-2; punctuation/operators are maximal-munch (REQ-6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokKind {
    // Keywords (reserved closed set — REQ-2).
    Fn,
    Spec,
    Req,
    Ens,
    Fx,
    Inv,
    Dec,
    Pure,
    Let,
    Mut,
    Return,
    Break,    // break (#93)
    Continue, // continue (#93)
    If,
    Else,
    Loop,
    While,
    Match,
    As,
    Struct,
    Enum,
    Is,

    // Literals / names.
    Ident(String),
    /// An integer literal carrying both the numeric `value` (with `_`
    /// separators stripped, lexer.md REQ-3 VALUE, unchanged) and the verbatim
    /// source `raw` (separators included, lexer.md REQ-3 RAW, #37). E.g.
    /// `1_000_000` lexes to `{ value: 1000000, raw: "1_000_000" }`.
    Int {
        value: u128,
        raw: String,
    },
    Bool(bool),
    Str(String),

    // Attribute introducer `#[`.
    HashBracket,

    // Multi-char operators (maximal munch — REQ-6).
    Arrow,    // ->
    FatArrow, // =>
    EqEq,     // ==
    Ne,       // !=
    Le,       // <=
    Ge,       // >=
    AndAnd,   // &&
    OrOr,     // ||
    ColonCol, // ::
    DotDot,   // ..
    Shl,      // <<  (#92)
    Shr,      // >>  (#92)

    // Single-char punctuation.
    LBrace,
    RBrace,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Comma,
    Semi,
    Colon,
    Dot,
    Eq,
    Lt,
    Gt,
    Plus,
    Minus,
    Star,
    Slash,
    Percent, // %  (#92)
    Caret,   // ^  (#92)
    Amp,
    Pipe,
    Bang,

    /// A body-position structural HOLE `?N` (`.design/forge/goal-repl.md` REQ-4,
    /// #193). The `u32` is the verbatim hole number as written (`?0` → `0`); it is
    /// the surface ordinal the agent typed, not a document-order index (the parser
    /// records holes in document order for `<fn>.?N` addressing, `parser.md` /
    /// `semantic-addressing.md`). The token lexes
    /// everywhere the scanner sees `?<digits>`, and the parser restricts it to
    /// fn-body statement position (a `?N` elsewhere is a parse error, `parser.md`).
    Hole(u32),

    Eof,
}

/// Map a word to its reserved-keyword kind, or `None` if it is an identifier.
/// Effect-row names and slag field names are not reserved (REQ-2,
/// OQ-1); they fall through to `Ident`.
fn keyword_kind(word: &str) -> Option<TokKind> {
    Some(match word {
        "fn" => TokKind::Fn,
        "spec" => TokKind::Spec,
        "req" => TokKind::Req,
        "ens" => TokKind::Ens,
        "fx" => TokKind::Fx,
        "inv" => TokKind::Inv,
        "dec" => TokKind::Dec,
        "pure" => TokKind::Pure,
        "let" => TokKind::Let,
        "mut" => TokKind::Mut,
        "return" => TokKind::Return,
        "break" => TokKind::Break,
        "continue" => TokKind::Continue,
        "if" => TokKind::If,
        "else" => TokKind::Else,
        "loop" => TokKind::Loop,
        "while" => TokKind::While,
        "match" => TokKind::Match,
        "as" => TokKind::As,
        "struct" => TokKind::Struct,
        "enum" => TokKind::Enum,
        "is" => TokKind::Is,
        "true" => TokKind::Bool(true),
        "false" => TokKind::Bool(false),
        _ => return None,
    })
}

/// Tokenize `src` into a token stream plus any lexical diagnostics. Never
/// panics (lexer.md REQ-8): an unrecognized character produces a `SyntaxError`
/// and the scan continues past it. The stream always ends with an `Eof` token.
pub fn tokenize(src: &str) -> (Vec<Token>, Vec<SyntaxError>) {
    let bytes = src.as_bytes();
    let n = bytes.len();
    let mut tokens = Vec::new();
    let mut errors = Vec::new();
    let mut i = 0usize;

    while i < n {
        i = skip_trivia(bytes, i);
        if i >= n {
            break;
        }
        let c = bytes[i];
        if c == b'#' && i + 1 < n && bytes[i + 1] == b'[' {
            tokens.push(Token {
                kind: TokKind::HashBracket,
                span: Span::new(i, 2),
            });
            i += 2;
        } else if c == b'"' {
            match lex_string(bytes, i) {
                Ok((tok, next)) => {
                    tokens.push(tok);
                    i = next;
                }
                Err(err) => {
                    let next = err.recover_to;
                    errors.push(err.error);
                    i = next;
                }
            }
        } else if c.is_ascii_digit() {
            match lex_int(bytes, i) {
                Ok((tok, next)) => {
                    tokens.push(tok);
                    i = next;
                }
                Err(err) => {
                    // A malformed radix literal (`0x` with no hex digit, `0b2`):
                    // structured diagnostic, resume past the scanned bytes (REQ-8).
                    let next = err.span().end().max(i + 1).min(n);
                    errors.push(err);
                    i = next;
                }
            }
        } else if c == b'\'' {
            // A char literal `'A'` (lexer.md REQ-9, #91/#92) lexes into the same
            // integer-literal token (no new token kind / Expr variant). A
            // malformed char (`''`, `'AB'`, non-ASCII, bad escape) is a structured
            // diagnostic that resyncs past the literal, never a panic.
            match lex_char(bytes, i) {
                Ok((tok, next)) => {
                    tokens.push(tok);
                    i = next;
                }
                Err(err) => {
                    let next = err.recover_to;
                    errors.push(err.error);
                    i = next;
                }
            }
        } else if c == b'?' {
            // A body-position structural hole `?N` (lexer.md / goal-repl.md REQ-4,
            // #193). `?` followed by one-or-more ASCII digits lexes to a single
            // `Hole(N)` token carrying the verbatim hole number. A `?` with no
            // following digit is an unrecognized character (a structured
            // diagnostic, never a panic, REQ-8): Thermite has no `?`-operator
            // (no try/Result-propagation surface, §2.3), so a bare `?` is a stray
            // char, not a partial token.
            match lex_hole(bytes, i) {
                Some((kind, len)) => {
                    tokens.push(Token {
                        kind,
                        span: Span::new(i, len),
                    });
                    i += len;
                }
                None => {
                    errors.push(SyntaxError::stray_char(
                        src[i..(i + 1).min(n)].to_string(),
                        Span::new(i, 1),
                    ));
                    i += 1;
                }
            }
        } else if is_ident_start(c) {
            let (tok, next) = lex_word(bytes, i);
            tokens.push(tok);
            i = next;
        } else if let Some((kind, len)) = lex_punct(bytes, i) {
            tokens.push(Token {
                kind,
                span: Span::new(i, len),
            });
            i += len;
        } else {
            // Unrecognized character (e.g. a stray `@`): diagnostic, continue.
            let ch_len = utf8_char_len(c);
            errors.push(SyntaxError::stray_char(
                src[i..(i + ch_len).min(n)].to_string(),
                Span::new(i, ch_len),
            ));
            i += ch_len;
        }
    }

    tokens.push(Token {
        kind: TokKind::Eof,
        span: Span::new(n, 0),
    });
    (tokens, errors)
}

/// Skip insignificant whitespace and `//`-to-EOL comments (lexer.md REQ-5),
/// returning the next byte index that begins a token.
fn skip_trivia(bytes: &[u8], mut i: usize) -> usize {
    let n = bytes.len();
    loop {
        // whitespace
        while i < n && matches!(bytes[i], b' ' | b'\t' | b'\r' | b'\n') {
            i += 1;
        }
        // line comment
        if i + 1 < n && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            i += 2;
            while i < n && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        break;
    }
    i
}

/// True if `c` may start an identifier (ASCII letter or `_`).
fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}

/// True if `c` may continue an identifier (letter, digit, or `_`).
fn is_ident_continue(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// Lex an identifier or keyword starting at `i`.
fn lex_word(bytes: &[u8], i: usize) -> (Token, usize) {
    let n = bytes.len();
    let mut j = i;
    while j < n && is_ident_continue(bytes[j]) {
        j += 1;
    }
    // The identifier bytes are ASCII (is_ident_* gate ASCII only), so this slice
    // is valid UTF-8.
    let word: String = bytes[i..j].iter().map(|&b| b as char).collect();
    let kind = keyword_kind(&word).unwrap_or(TokKind::Ident(word));
    (
        Token {
            kind,
            span: Span::new(i, j - i),
        },
        j,
    )
}

/// Lex an integer literal with optional `_` separators (lexer.md REQ-3). The
/// `_` are stripped while accumulating the numeric `value` (VALUE); the verbatim
/// source slice (separators + any `0x`/`0b` prefix included) is captured as `raw`
/// (RAW, #37).
///
/// The radix is chosen by the prefix at the start of a digit run (lexer.md REQ-3,
/// #92): `0x`/`0X` → hexadecimal, `0b`/`0B` → binary, otherwise decimal. A hex /
/// binary literal carries the same integer `value` as the equivalent decimal
/// (`0x1b` → 27, `0b101` → 5); the radix is a surface spelling only, never a
/// distinct token kind. A `0x`/`0b` prefix requires at least one radix digit; a
/// bare prefix with no following digit is an `Err(SyntaxError)` (lexer.md REQ-8),
/// not a `0` followed by an `x`/`b` identifier. A trailing/leading `_` adjacent to
/// the digit run is in neither value nor raw (both end at the last radix digit).
fn lex_int(bytes: &[u8], i: usize) -> Result<(Token, usize), SyntaxError> {
    let n = bytes.len();
    // Radix prefix dispatch (#92). `0x`/`0X` → 16, `0b`/`0B` → 2, else 10. The
    // prefix is two bytes; the digit scan begins after it.
    let (radix, digits_start): (u32, usize) = if i + 1 < n && bytes[i] == b'0' {
        match bytes[i + 1] {
            b'x' | b'X' => (16, i + 2),
            b'b' | b'B' => (2, i + 2),
            _ => (10, i),
        }
    } else {
        (10, i)
    };

    let mut j = digits_start;
    let mut value: u128 = 0;
    let mut last_digit = digits_start;
    let mut saw_digit = false;
    while j < n {
        let c = bytes[j];
        if let Some(d) = radix_digit(c, radix) {
            value = value
                .saturating_mul(radix as u128)
                .saturating_add(d as u128);
            j += 1;
            last_digit = j;
            saw_digit = true;
        } else if c == b'_' {
            j += 1;
        } else {
            break;
        }
    }

    // A `0x`/`0b` prefix with no radix digit (`0x`, `0b2`) is a malformed literal,
    // not `0` + ident `x`/`b` (lexer.md REQ-3/REQ-8). Span the prefix + any bytes
    // scanned; recovery resumes past it (the caller advances to the returned end).
    if radix != 10 && !saw_digit {
        let bad_end = (digits_start).max(j).min(n);
        let bad: String = bytes[i..bad_end].iter().map(|&b| b as char).collect();
        return Err(SyntaxError::stray_char(bad, Span::new(i, bad_end - i)));
    }

    // The literal raw is `source[i..last_digit]` (prefix + interior `_` included,
    // trailing `_` excluded). The bytes are ASCII (digits/`_`/`0x`/`0b`), valid UTF-8.
    let raw: String = bytes[i..last_digit].iter().map(|&b| b as char).collect();
    Ok((
        Token {
            kind: TokKind::Int { value, raw },
            span: Span::new(i, last_digit - i),
        },
        last_digit,
    ))
}

/// Lex a body-position structural hole `?N` (lexer.md / `.design/forge/goal-repl.md`
/// REQ-4, #193). `bytes[i]` is the `?`; the hole NUMBER is the run of ASCII digits
/// immediately following. Returns the `Hole(N)` token kind + its byte length, or
/// `None` if no digit follows the `?` (a bare `?` is a stray char, REQ-8). The
/// number is parsed deterministically (R-CODE-5); an over-long digit run that
/// overflows `u32` saturates (a hole number is a small surface ordinal: there is
/// no semantic difference between `?4000000000` and `?u32::MAX`, both name a hole
/// the agent must address, and saturation keeps the lexer total + panic-free, REQ-8).
fn lex_hole(bytes: &[u8], i: usize) -> Option<(TokKind, usize)> {
    let n = bytes.len();
    let mut j = i + 1; // past the `?`
    let mut value: u32 = 0;
    let mut saw_digit = false;
    while j < n && bytes[j].is_ascii_digit() {
        let d = (bytes[j] - b'0') as u32;
        value = value.saturating_mul(10).saturating_add(d);
        saw_digit = true;
        j += 1;
    }
    if !saw_digit {
        return None;
    }
    Some((TokKind::Hole(value), j - i))
}

/// Map an ASCII digit to its value `0..radix`, or `None` if it is not a digit of
/// that radix. Supports radix 2 (binary), 10 (decimal), and 16 (hexadecimal) —
/// the three integer-literal spellings (lexer.md REQ-3, #92).
fn radix_digit(c: u8, radix: u32) -> Option<u32> {
    let d = match c {
        b'0'..=b'9' => (c - b'0') as u32,
        b'a'..=b'f' => (c - b'a') as u32 + 10,
        b'A'..=b'F' => (c - b'A') as u32 + 10,
        _ => return None,
    };
    if d < radix {
        Some(d)
    } else {
        None
    }
}

/// A char-lex failure carrying the diagnostic and where to resume (mirrors
/// [`StringLexError`]).
struct CharLexError {
    error: SyntaxError,
    recover_to: usize,
}

/// Lex a char literal `'A'` (lexer.md REQ-9, #91/#92) into the same
/// `TokKind::Int { value, raw }` token as a numeric literal: no new token kind,
/// no new Expr variant. `value` is the byte value of the character (`'A'` → 65,
/// `'\n'` → 10, `'\x1b'` → 27); `raw` is the verbatim source including the quotes
/// (`"'A'"`). The char model is byte-level `u8` (consistent with the 07-strings
/// byte model). A `\`-escape is decoded by the same escape table the string lexer
/// uses (`\n`/`\t`/`\r`/`\0`/`\\`/`\'` + `\xNN` with `NN <= 0x7F`).
///
/// A char literal that is multi-byte / non-ASCII (a codepoint `>= 0x80`), empty
/// (`''`), unterminated, or whose `\xNN >= 0x80` is a structured `SyntaxError`
/// (lexer.md REQ-8/REQ-9): never a silent mis-lex, never a panic. (§4.4 removes
/// lifetimes, so `'` always begins a char literal; there is no `'a` lifetime to
/// disambiguate against.)
fn lex_char(bytes: &[u8], i: usize) -> Result<(Token, usize), CharLexError> {
    let n = bytes.len();
    // Helper: the verbatim raw from `i` to `end` (exclusive), ASCII-faithful.
    let raw_of =
        |end: usize| -> String { bytes[i..end.min(n)].iter().map(|&b| b as char).collect() };
    // An empty/unterminated literal at EOF: `'` with nothing after.
    if i + 1 >= n {
        return Err(CharLexError {
            error: SyntaxError::stray_char(raw_of(n), Span::new(i, n - i)),
            recover_to: n,
        });
    }
    let (byte, content_end): (u8, usize) = if bytes[i + 1] == b'\\' {
        // An escape `'\n'`, `'\xNN'`, etc.: decode via the shared table.
        if i + 2 >= n {
            return Err(CharLexError {
                error: SyntaxError::stray_char(raw_of(n), Span::new(i, n - i)),
                recover_to: n,
            });
        }
        let esc = bytes[i + 2];
        let single: Option<u8> = match esc {
            b'n' => Some(10),
            b't' => Some(9),
            b'r' => Some(13),
            b'0' => Some(0),
            b'\\' => Some(b'\\'),
            b'\'' => Some(b'\''),
            _ => None,
        };
        if let Some(b) = single {
            (b, i + 3)
        } else if esc == b'x' {
            // `\xNN` — exactly two hex digits; ASCII range `0x00..=0x7F` only (the
            // byte model, mirroring `lex_string`). A high byte (`>= 0x80`) or a
            // malformed escape is a structured error.
            match parse_hex_escape(bytes, i + 3) {
                Some(b) if b < 0x80 => (b, i + 5), // `'` `\` `x` + two hex digits
                Some(_) | None => {
                    let bad_end = (i + 5).min(n);
                    return Err(CharLexError {
                        error: SyntaxError::stray_char(raw_of(bad_end), Span::new(i, bad_end - i)),
                        recover_to: resume_past_char(bytes, bad_end),
                    });
                }
            }
        } else {
            // An unknown escape (`'\z'`): structured error, never a silent swallow.
            let bad_end = (i + 3).min(n);
            return Err(CharLexError {
                error: SyntaxError::stray_char(raw_of(bad_end), Span::new(i, bad_end - i)),
                recover_to: resume_past_char(bytes, bad_end),
            });
        }
    } else {
        let c = bytes[i + 1];
        // A non-ASCII / multi-byte char (`'é'`, codepoint >= 0x80) is not a single
        // `u8` in v1: a structured error (it awaits the `Vec<u8>` reshape that
        // defers high-byte string escapes). An ASCII char is its own byte value.
        if c >= 0x80 {
            let bad_end = resume_past_char(bytes, i + 1);
            return Err(CharLexError {
                error: SyntaxError::stray_char(raw_of(bad_end), Span::new(i, bad_end - i)),
                recover_to: bad_end,
            });
        }
        // An empty literal `''`: the char position is immediately the close quote.
        if c == b'\'' {
            return Err(CharLexError {
                error: SyntaxError::stray_char(raw_of(i + 2), Span::new(i, 2)),
                recover_to: i + 2,
            });
        }
        (c, i + 2)
    };

    // The closing quote must follow the single char/escape; anything else (a
    // multi-char literal `'AB'`, a missing close) is a structured error.
    if content_end < n && bytes[content_end] == b'\'' {
        let end = content_end + 1;
        Ok((
            Token {
                kind: TokKind::Int {
                    value: byte as u128,
                    raw: raw_of(end),
                },
                span: Span::new(i, end - i),
            },
            end,
        ))
    } else {
        let bad_end = resume_past_char(bytes, content_end);
        Err(CharLexError {
            error: SyntaxError::stray_char(raw_of(bad_end), Span::new(i, bad_end - i)),
            recover_to: bad_end,
        })
    }
}

/// After a malformed char literal, resume scanning past the next `'` (if any) so
/// per-item recovery resyncs at a token boundary (mirrors [`resume_past_string`]).
/// Bounded by a small window so a stray `'` does not swallow the rest of the file.
fn resume_past_char(bytes: &[u8], from: usize) -> usize {
    let n = bytes.len();
    let mut k = from;
    // Scan a bounded window for a closing quote; cap so an unmatched `'` resyncs
    // promptly rather than consuming the program (per-item recovery, REQ-8).
    let limit = (from + 4).min(n);
    while k < limit {
        if bytes[k] == b'\'' {
            return k + 1;
        }
        k += 1;
    }
    from
}

/// A string-lex failure carrying the diagnostic and where to resume.
struct StringLexError {
    error: SyntaxError,
    recover_to: usize,
}

/// Lex a double-quoted string literal (lexer.md REQ-4). Returns the token + next
/// index, or a structured diagnostic. v1 stores decoded content in a Rust
/// `String`, so literal bytes must be ASCII single-byte values; raw high bytes
/// await the future byte-buffer string representation.
fn lex_string(bytes: &[u8], i: usize) -> Result<(Token, usize), StringLexError> {
    let n = bytes.len();
    let mut j = i + 1; // skip opening quote
    let mut content = String::new();
    while j < n {
        let c = bytes[j];
        if c == b'"' {
            return Ok((
                Token {
                    kind: TokKind::Str(content),
                    span: Span::new(i, j + 1 - i),
                },
                j + 1,
            ));
        }
        if c == b'\\' && j + 1 < n {
            let esc = bytes[j + 1];
            // Single-char escapes materialize to a control/literal byte
            // (`.design/basis/07-strings.md` REQ-6, the escape table). The byte
            // model (REQ-2: a string is a run of `u8`) means each escape decodes
            // to one byte; `\n`/`\t`/`\r`/`\0` are control bytes, `\"`/`\\` are
            // the quote/backslash literals. Codepoints < 0x80 are a single UTF-8
            // byte, so `String::push` of the corresponding `char` materializes
            // the intended byte exactly (`"\x1b".as_bytes()[0] == 27`).
            let single: Option<char> = match esc {
                b'n' => Some('\n'), // 10
                b't' => Some('\t'), // 9
                b'r' => Some('\r'), // 13
                b'0' => Some('\0'), // 0
                b'"' => Some('"'),  // 34
                b'\\' => Some('\\'),
                _ => None,
            };
            if let Some(ch) = single {
                content.push(ch);
                j += 2;
                continue;
            }
            if esc == b'x' {
                // `\xNN` — exactly two hex digits, materializing to the byte
                // value (`\x1b` -> 27). The byte model (REQ-2/REQ-6) admits the
                // ASCII range `\x00`..=`\x7F` (a single UTF-8 byte); a value
                // >= 0x80 is not a single byte in a UTF-8 `String` (it would
                // UTF-8-encode to two bytes), so it is a structured lex error in
                // v1, not silently mis-materialized; faithful byte indexing is
                // the string claim (REQ-2). A high-byte `\xNN`
                // awaits the `Vec<u8>` string-content reshape (a future stage).
                match parse_hex_escape(bytes, j + 2) {
                    Some(byte) if byte < 0x80 => {
                        // codepoint == byte < 0x80 -> exactly one UTF-8 byte.
                        content.push(byte as char);
                        j += 4; // `\x` + two hex digits
                        continue;
                    }
                    Some(_) | None => {
                        // Malformed (`\xZZ`, truncated) OR a high byte (>= 0x80,
                        // not single-byte representable in v1): a structured
                        // diagnostic over the `\x..` span; resume after `"`.
                        let bad_end = (j + 4).min(n);
                        let bad: String = bytes[j..bad_end].iter().map(|&b| b as char).collect();
                        return Err(StringLexError {
                            error: SyntaxError::stray_char(bad, Span::new(j, bad_end - j)),
                            recover_to: resume_past_string(bytes, bad_end),
                        });
                    }
                }
            }
            // Any other escape (`\z`) is unknown: a structured diagnostic, never
            // a silent `other as char` swallow (the v0.1 bug this stage closes).
            let bad: String = bytes[j..(j + 2).min(n)]
                .iter()
                .map(|&b| b as char)
                .collect();
            return Err(StringLexError {
                error: SyntaxError::stray_char(bad, Span::new(j, (j + 2).min(n) - j)),
                recover_to: resume_past_string(bytes, j + 2),
            });
        }
        if c >= 0x80 {
            let bad_end = (j + utf8_char_len(c)).min(n);
            let bad = String::from_utf8_lossy(&bytes[j..bad_end]).into_owned();
            return Err(StringLexError {
                error: SyntaxError::stray_char(bad, Span::new(j, bad_end - j)),
                recover_to: resume_past_string(bytes, bad_end),
            });
        }
        content.push(c as char);
        j += 1;
    }
    Err(StringLexError {
        error: SyntaxError::unterminated_string(Span::new(i, n - i)),
        recover_to: n,
    })
}

/// Parse exactly two hex digits at `bytes[at..at+2]` into a byte value, or
/// `None` if fewer than two hex digits are present (a malformed `\xZ`/`\x`).
/// Deterministic, total — no panic (lexer.md REQ-8).
fn parse_hex_escape(bytes: &[u8], at: usize) -> Option<u8> {
    let n = bytes.len();
    if at + 1 >= n {
        return None;
    }
    let hi = hex_digit(bytes[at])?;
    let lo = hex_digit(bytes[at + 1])?;
    Some(hi * 16 + lo)
}

/// Map an ASCII hex digit (`0-9`/`a-f`/`A-F`) to its value `0..=15`, or `None`.
fn hex_digit(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// After a malformed escape inside a string literal, resume scanning past the
/// string's closing `"` (if any) so per-item recovery resyncs at a token
/// boundary, mirroring the unterminated-string recovery. Starts at `from`.
fn resume_past_string(bytes: &[u8], from: usize) -> usize {
    let n = bytes.len();
    let mut k = from;
    while k < n {
        // A `\"` inside the literal is an escaped quote, not the terminator.
        if bytes[k] == b'\\' && k + 1 < n {
            k += 2;
            continue;
        }
        if bytes[k] == b'"' {
            return k + 1;
        }
        k += 1;
    }
    n
}

/// Lex a punctuation/operator token by maximal munch (lexer.md REQ-6): try the
/// two-character operators before falling back to single characters. Returns
/// the kind and its byte length, or `None` if `bytes[i]` is not punctuation.
fn lex_punct(bytes: &[u8], i: usize) -> Option<(TokKind, usize)> {
    let n = bytes.len();
    let c = bytes[i];
    let d = if i + 1 < n { Some(bytes[i + 1]) } else { None };

    // Two-char operators first (maximal munch).
    if let Some(next) = d {
        let two = match (c, next) {
            (b'-', b'>') => Some(TokKind::Arrow),
            (b'=', b'>') => Some(TokKind::FatArrow),
            (b'=', b'=') => Some(TokKind::EqEq),
            (b'!', b'=') => Some(TokKind::Ne),
            (b'<', b'=') => Some(TokKind::Le),
            (b'>', b'=') => Some(TokKind::Ge),
            (b'&', b'&') => Some(TokKind::AndAnd),
            (b'|', b'|') => Some(TokKind::OrOr),
            (b':', b':') => Some(TokKind::ColonCol),
            (b'.', b'.') => Some(TokKind::DotDot),
            // Shift operators (#92): `<<`/`>>` win over single `<`/`>` by maximal
            // munch (REQ-6). `<=`/`>=` are distinct pairs (the second byte differs),
            // so the order among these two-char arms is irrelevant: `>>` is `>` `>`,
            // `>=` is `>` `=`; both are matched here before any single-char fallback.
            (b'<', b'<') => Some(TokKind::Shl),
            (b'>', b'>') => Some(TokKind::Shr),
            _ => None,
        };
        if let Some(kind) = two {
            return Some((kind, 2));
        }
    }

    let one = match c {
        b'{' => TokKind::LBrace,
        b'}' => TokKind::RBrace,
        b'(' => TokKind::LParen,
        b')' => TokKind::RParen,
        b'[' => TokKind::LBracket,
        b']' => TokKind::RBracket,
        b',' => TokKind::Comma,
        b';' => TokKind::Semi,
        b':' => TokKind::Colon,
        b'.' => TokKind::Dot,
        b'=' => TokKind::Eq,
        b'<' => TokKind::Lt,
        b'>' => TokKind::Gt,
        b'+' => TokKind::Plus,
        b'-' => TokKind::Minus,
        b'*' => TokKind::Star,
        b'/' => TokKind::Slash,
        b'%' => TokKind::Percent, // #92
        b'^' => TokKind::Caret,   // #92
        b'&' => TokKind::Amp,
        b'|' => TokKind::Pipe,
        b'!' => TokKind::Bang,
        _ => return None,
    };
    Some((one, 1))
}

/// Byte length of the UTF-8 character whose leading byte is `c` (for span width
/// on a stray non-ASCII character).
fn utf8_char_len(c: u8) -> usize {
    if c < 0x80 {
        1
    } else if c >> 5 == 0b110 {
        2
    } else if c >> 4 == 0b1110 {
        3
    } else if c >> 3 == 0b11110 {
        4
    } else {
        1
    }
}
