//! Thermite lexer — a single-pass, hand-written scanner over the source `&str`
//! producing a flat `Vec<Token>` for the recursive-descent parser.
//!
//! Governing design: `.design/syntax/lexer.md`. Thermite has NO significant
//! whitespace (§4.3); whitespace and `//` comments are insignificant separators
//! (REQ-5). The keyword set is exactly the surface-grammar terminals (REQ-2);
//! effect-row names (`read`, `write`, ...) and `slag`/`reason`/`owner`/`review`
//! are lexed as IDENTIFIERS (contextual keywords, OQ-1). The scanner is
//! REGISTRY-FREE: `forall_in`, `sorted`, `len` are plain identifiers. Maximal
//! munch (REQ-6) picks the longest operator at each position. Errors are
//! `SyntaxError` values; the lexer never panics (REQ-8).
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (token set) | SHIPPED | `enum TokKind` enumerates exactly keywords/ident/int/bool/punct/slag/eof; consumed by `parser.rs`. |
//! | REQ-2 (keywords closed set) | SHIPPED | `keyword_kind` maps the 17 reserved words; effect/slag names fall through to `Ident`. |
//! | REQ-3 (int literals with `_`) | SHIPPED | `lex_int` strips `_` and accumulates value; `1_000_000` → `1000000` (test in `tests/conformance.rs`). |
//! | REQ-4 (`#[slag]` tokenization) | SHIPPED | `#[` token + string literals via `lex_string`; consumed by `parse_slag`. |
//! | REQ-5 (comments + whitespace insignificant) | SHIPPED | `skip_trivia` consumes `[ \t\r\n]+` and `//`-to-EOL, emitting nothing. |
//! | REQ-6 (maximal munch operators) | SHIPPED | `lex_punct` tries 2-char operators before 1-char (`<=`, `==`, `->`, `::`, `..`, `#[`). |
//! | REQ-7 (spans) | SHIPPED | every `Token` carries a `Span { start, len }`; used by parser diagnostics + addressing. |
//! | REQ-8 (Result discipline) | SHIPPED | `tokenize` returns `(Vec<Token>, Vec<SyntaxError>)`; stray chars become diagnostics, no panic. |

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
    If,
    Else,
    Loop,
    While,
    Match,
    As,

    // Literals / names.
    Ident(String),
    Int(u128),
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
    Amp,
    Pipe,
    Bang,

    Eof,
}

/// Map a word to its reserved-keyword kind, or `None` if it is an identifier.
/// Effect-row names and slag field names are deliberately NOT reserved (REQ-2,
/// OQ-1) — they fall through to `Ident`.
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
        "if" => TokKind::If,
        "else" => TokKind::Else,
        "loop" => TokKind::Loop,
        "while" => TokKind::While,
        "match" => TokKind::Match,
        "as" => TokKind::As,
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
            let (tok, next) = lex_int(bytes, i);
            tokens.push(tok);
            i = next;
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
/// `_` are stripped while accumulating; a trailing `_` is not consumed.
fn lex_int(bytes: &[u8], i: usize) -> (Token, usize) {
    let n = bytes.len();
    let mut j = i;
    let mut value: u128 = 0;
    let mut last_digit = i;
    while j < n {
        let c = bytes[j];
        if c.is_ascii_digit() {
            value = value.saturating_mul(10).saturating_add((c - b'0') as u128);
            j += 1;
            last_digit = j;
        } else if c == b'_' {
            j += 1;
        } else {
            break;
        }
    }
    // A trailing `_` (e.g. `1_`) is not part of the literal: end at last digit.
    (
        Token {
            kind: TokKind::Int(value),
            span: Span::new(i, last_digit - i),
        },
        last_digit,
    )
}

/// A string-lex failure carrying the diagnostic and where to resume.
struct StringLexError {
    error: SyntaxError,
    recover_to: usize,
}

/// Lex a double-quoted string literal (only used as a `#[slag]` field value —
/// lexer.md REQ-4). Returns the token + next index, or an unterminated-string
/// diagnostic.
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
            content.push(match esc {
                b'n' => '\n',
                b't' => '\t',
                b'"' => '"',
                b'\\' => '\\',
                other => other as char,
            });
            j += 2;
            continue;
        }
        content.push(c as char);
        j += 1;
    }
    Err(StringLexError {
        error: SyntaxError::unterminated_string(Span::new(i, n - i)),
        recover_to: n,
    })
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
