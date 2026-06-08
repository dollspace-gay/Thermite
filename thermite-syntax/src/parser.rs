//! Thermite parser — hand-written recursive descent over the lexer's token
//! stream, producing the AST. The executable form of `surface-grammar.md`.
//!
//! Governing design: `.design/syntax/parser.md`. Two design-mandated properties
//! dominate: (a) PER-ITEM error recovery — a syntax error inside one item must
//! not cascade into the next (§4.3, REQ-3): the top-level loop resyncs to the
//! next item-boundary token (`fn`/`spec`/`#[`/EOF) and keeps parsing; and
//! (b) MANDATORY-CLAUSE enforcement — a `fn` missing `req`/`ens`/`fx`, or a
//! `loop`/`while` missing `inv`/`dec`, is a `SyntaxError`, never a default
//! (§4.1, REQ-2). It is REGISTRY-FREE: combinator calls parse as generic
//! `Expr::Call`s; it never consults thermite-spec. Returns a
//! diagnostics-bearing `ParseResult` and never panics (REQ-4).
//!
//! This module owns the crate's `SyntaxError` type — the first fallible code in
//! the toolchain introduces its own error enum (`.design/scaffold/workspace.md`
//! REQ-3).
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (recursive descent) | SHIPPED | one fn per grammar family (`parse_item`/`parse_contract`/`parse_block`/`parse_expr_bp`/...); accepts both corpus programs (tests). |
//! | REQ-2 (mandatory-clause enforcement) | SHIPPED | `parse_contract` requires `req`->`ens`+->`fx` in order; `parse_loop` requires `inv`+->one `dec`; absence/misorder -> `SyntaxError`. |
//! | REQ-3 (per-item recovery) | SHIPPED | `parse_program` resyncs via `resync_to_item_boundary`; `recover_per_item` fixture passes. |
//! | REQ-4 (Result / no panic) | SHIPPED | `ParseResult { program, errors }`; `enum SyntaxError`; no panicking constructs in this file. |
//! | REQ-5 (round-trip fidelity) | SHIPPED | `tests/conformance.rs` asserts `sum`/`binary_search` facts with 0 diagnostics. |
//! | REQ-6 (one call syntax) | SHIPPED | postfix `.` -> `MethodCall`/`Field`, free `f(args)` -> `Call`, `::` -> `Path` (never method dispatch). |
//! | REQ-7 (addressing substrate) | SHIPPED | loops/`inv`s kept in source order in the AST; numbered by `address.rs`. |
//! | REQ-8 (operator tiers `% << >> & \| ^ !`, #92) | SHIPPED | `parse_mul`+`%`→`Rem`; new tiers `parse_shift`/`parse_bitand`/`parse_bitxor`/`parse_bitor` (threaded `parse_is`→`parse_bitor`→…→`parse_add`, `is` above the bitwise tiers); `parse_unary` builds the prefix `!`→`Unary { Not }`. Binary `&`/`\|` vs prefix ref/closure disambiguated by position. Tests `tests/operators_parse.rs`. |
//! | REQ-9 (partiality not a parse concern, #92) | SHIPPED | `parse_mul`/`parse_shift` build the `Binary` node UNCONDITIONALLY — no `req` injection; the div-by-zero / shift-bound obligation is a §7 proof obligation (`ast.md` REQ-11), GROUNDED L0-without/L3-with in `forge/tests/operators_conformance.rs`. |
//!
//! ## Cluster C7 — Option/Result type parsing (`.design/basis/09-option-result.md`, #95)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (`Option<T>` parse) | SHIPPED | `parse_type`'s `"Option"` contextual-ident arm builds `Type::Option(Box<Type>)` (mirroring the `Box`/`Vec` arms) — `Option` STOPS being a string-named `Generic`. Verified: `forge/tests/option_result_conformance.rs::ac1_...`. |
//! | REQ-2 (`Result<T, E>` two-arg parse) | SHIPPED | `parse_type`'s `"Result"` arm parses `<T, E>` — the FIRST two-type-argument type in the grammar (a `Comma` + a second `parse_type` + `Gt`), building `Type::Result(Box<Type>, Box<Type>)`. The single-arg `Generic` could not (it died at the comma). Verified: `forge/tests/option_result_conformance.rs::ac2_...` (`Result<u64, ParseErr>` parses). |
//!
//! ## Cluster C9-A — plain-`fn` recursion `dec` clause (`.design/basis/10-recursion-tuples.md`, #108)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (`fn` `dec` clause parse) | SHIPPED | `parse_fn` parses an OPTIONAL trailing `dec <expr>` clause AFTER the contract (`req`/`ens`/`fx`) and BEFORE the body — the OQ-4 byte-stable slot mirroring the loop order (`inv`s then `dec`). Reuses `parse_clause(&TokKind::Dec)` (the same `dec` parse `parse_spec_fn`/`parse_loop` use). Absent → `FnItem.dec = None` (the `req`/`ens`/`fx` parse is UNCHANGED for every non-recursive fn). Consumer: `thermite-lower::lower::lower_fn`. Verified: `forge/tests/recursion_conformance.rs` (a recursive `count_down` with `dec n` parses + certifies L3). |
//!
//! ## Cluster C10 — binding/control-flow ergonomics parse (`.design/basis/11-ergonomics.md`, #112)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (tuple destructure parse) | SHIPPED | `parse_let` returns `Vec<Stmt>`; a `(` after `let [mut]` routes to `parse_let_tuple_destructure`, desugaring `let (x, y) = e;` to `let __td<n> = e;` + per-element `let x = __td<n>.0;` (reusing `Expr::TupleProj`). `_` drops an element. Consumer: `lower_stmt` (the projection `let`s lower today). |
//! | REQ-2 (`for i in 0..n` parse) | SHIPPED | `parse_for` (dispatched on the contextual `for` ident at statement head) desugars `for i in lo..hi inv … { B }` to `let mut i = lo;` + `LoopNode { While(i < hi), invs, dec: hi - i, body: B ++ [i = i + 1;] }`. `for`/`in` are NOT reserved (matched by name). The user `inv` is mandatory; the `dec` is AUTO-synthesized (`hi - i`), and a user `dec` on a `for` is rejected. Consumer: `lower_loop`. |
//! | REQ-3 (match guard parse) | SHIPPED | `parse_match` parses an optional `if <cond>` (a no-struct-literal head) before `=>` into `MatchArm.guard`. Consumer: `lower_match`/`check_match_exhaustiveness`. |
//! | REQ-4 (or-pattern parse) | SHIPPED | `parse_pattern` parses a `\|`-joined alternation into a flat `Pattern::Or(Vec<Pattern>)` (a single alternative stays the bare pattern — byte-stable). Consumer: `lower_pattern`/`check_match_exhaustiveness`. |
//! | REQ-5 (`if let`/`while let` parse) | SHIPPED | `parse_if_let` (dispatched on `if` followed by `let` via `peek_nth`) desugars to `Expr::Match { e, [P => T, _ => E] }` (value form, mandatory `else`); `parse_while_let` desugars `while let Variant(_) = e inv … dec … { B }` to a `LoopNode { While(e is Variant), … }` (the canonical `while (cond)` form). Consumer: `lower_match`/`lower_loop`. |
//!
//! ## #16 boundary-fn parser extension (`.design/boundary/ffi-boundary.md`)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! ## Basis Stage 4 — bounded-collection type parse (`.design/basis/04-collections.md`)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (`Vec<T>` type spelling) | SHIPPED | `parse_type` matches the contextual `Vec` ident (mirroring `Box`): `Vec<u64>` → `Type::Vec(Box::new(Type::Prim(U64)))` (`conformance/vec_demo.th`, asserted by `thermite-lower/tests/collections_conformance.rs`). The `push`/`pop`/`get`/`len` operations are ordinary postfix `.m(args)` `MethodCall`s already parsed by `parse_postfix` (REQ-6) — no new surface. |
//!
//! | ffi REQ-1/REQ-3 | SHIPPED | `parse_attribute` generalizes `parse_slag` (dispatch on the `#[` attribute name: `slag` -> `SlagAttr`, `boundary` -> `BoundaryAttr` reading one positional `(` STRING `)`); `parse_item` routes a `boundary` attribute into `parse_fn` (and rejects `#[boundary]` on a `spec fn`). `parse_fn` gains a `Semi`-terminated bodyless path GATED on `boundary.is_some()` (OQ-2): `#[boundary]` REQUIRES the `;` body, a fn with NO `#[boundary]` REQUIRES the `{ }` body — a bodyless non-`#[boundary]` fn is a clear `SyntaxError`, never silently a boundary fn. |

use crate::ast::*;
use crate::lexer::{tokenize, Span, TokKind, Token};

/// The crate's error type — the first fallible code in the toolchain owns it
/// (`.design/scaffold/workspace.md` REQ-3). Every variant carries a span so
/// diagnostics are crisp (pillar 4) and per-item recovery can resync.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyntaxError {
    /// An unrecognized character in the source (lexer.md REQ-8).
    StrayChar { ch: String, span: Span },
    /// A `"`-string with no closing quote.
    UnterminatedString { span: Span },
    /// A token of a different kind than the grammar required here.
    Unexpected {
        expected: String,
        found: String,
        span: Span,
    },
    /// A mandatory contract/loop clause was absent (§4.1).
    MissingClause {
        item: String,
        clause: String,
        span: Span,
    },
    /// A mandatory clause appeared out of the grammar's fixed order (§4.1).
    ClauseOrder {
        item: String,
        clause: String,
        span: Span,
    },
    /// Unexpected end of input while a production was still open.
    UnexpectedEof { expected: String, span: Span },
    /// An expression nested past the parser's recursion-depth limit. Surfaced
    /// as a structured diagnostic so external input can never overflow the
    /// C stack and abort the process (parser.md AC-4 / REQ-4; goal.md R-CODE-2).
    ExpressionTooDeep { limit: usize, span: Span },
    /// A `break;`/`continue;` statement parsed OUTSIDE any `loop`/`while` body
    /// (parser.md REQ-10, #93). A structural rule (like the mandatory-clause
    /// rule) — break/continue are loop-control statements and have no meaning at
    /// a function-body top level; `keyword` is `"break"` or `"continue"`.
    BreakContinueOutsideLoop { keyword: String, span: Span },
}

/// The maximum recursive-descent nesting depth the parser will follow before
/// returning an `ExpressionTooDeep` diagnostic. Bounding the recursion keeps
/// external input from overflowing the native stack and aborting the process
/// (parser.md AC-4). The limit is a fixed constant (determinism, goal.md
/// R-CODE-5), comfortably above any human-authored nesting yet well below the
/// stack budget for a debug build.
///
/// This single bound guards EVERY recursive-descent family, not just the
/// expression ladder: nested expressions (`parse_expr`), nested types
/// (`parse_type` on `Option<Option<...>>`), nested patterns (`parse_pattern`
/// covering both the slice `[[...]]` and enum `Some(Some(...))` cycles), and the
/// tail-position `if/else` cycle (`parse_block`/`parse_if_parts`). Each family
/// re-enters its recursion through a guarded entry point, so a single shared
/// counter caps them all (a divergence the #29 expr-only guard missed).
///
/// The value MUST sit below the native-stack overflow point: each nesting level
/// descends a chain of frames (the full ladder `parse_expr`->...->`parse_primary`
/// plus paren re-entry for expressions, ~10 frames/level; fewer for types and
/// patterns), so deep nesting overflows the C stack long before a large count
/// would. Empirically, on a 2 MiB thread (the Rust test-thread default) a debug
/// build overflows between ~135 and ~140 levels; 64 leaves a ~2x margin to cover
/// debug/release and platform variance while staying far above any plausible
/// hand-authored nesting (the re-audit confirmed depth-63 parses, depth-70
/// errors, and depth-40 reasonable nesting still parses).
const MAX_RECURSION_DEPTH: usize = 64;

impl SyntaxError {
    /// Construct a stray-character diagnostic (used by the lexer).
    pub fn stray_char(ch: String, span: Span) -> Self {
        SyntaxError::StrayChar { ch, span }
    }

    /// Construct an unterminated-string diagnostic (used by the lexer).
    pub fn unterminated_string(span: Span) -> Self {
        SyntaxError::UnterminatedString { span }
    }

    /// The source span this diagnostic points at.
    pub fn span(&self) -> Span {
        match self {
            SyntaxError::StrayChar { span, .. }
            | SyntaxError::UnterminatedString { span }
            | SyntaxError::Unexpected { span, .. }
            | SyntaxError::MissingClause { span, .. }
            | SyntaxError::ClauseOrder { span, .. }
            | SyntaxError::UnexpectedEof { span, .. }
            | SyntaxError::ExpressionTooDeep { span, .. }
            | SyntaxError::BreakContinueOutsideLoop { span, .. } => *span,
        }
    }
}

impl std::fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyntaxError::StrayChar { ch, span } => {
                write!(f, "stray character {:?} at byte {}", ch, span.start)
            }
            SyntaxError::UnterminatedString { span } => {
                write!(f, "unterminated string literal at byte {}", span.start)
            }
            SyntaxError::Unexpected {
                expected,
                found,
                span,
            } => write!(
                f,
                "expected {expected}, found {found} at byte {}",
                span.start
            ),
            SyntaxError::MissingClause { item, clause, span } => write!(
                f,
                "function `{item}` is missing the mandatory `{clause}` clause (byte {})",
                span.start
            ),
            SyntaxError::ClauseOrder { item, clause, span } => write!(
                f,
                "clause `{clause}` is out of order in `{item}` (byte {})",
                span.start
            ),
            SyntaxError::UnexpectedEof { expected, span } => write!(
                f,
                "expected {expected}, found end of input at byte {}",
                span.start
            ),
            SyntaxError::ExpressionTooDeep { limit, span } => write!(
                f,
                "expression nested deeper than the limit of {limit} at byte {}",
                span.start
            ),
            SyntaxError::BreakContinueOutsideLoop { keyword, span } => write!(
                f,
                "`{keyword}` outside of a loop body at byte {}",
                span.start
            ),
        }
    }
}

impl std::error::Error for SyntaxError {}

/// The result of parsing: the recovered program AND every diagnostic, so even a
/// partial failure yields the surviving items for tooling (parser.md REQ-4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseResult {
    pub program: Program,
    pub errors: Vec<SyntaxError>,
}

impl ParseResult {
    /// True if parsing produced no diagnostics at all.
    pub fn is_clean(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Parse `src` into a `ParseResult`, recovering per-item on error (parser.md).
/// Never panics (REQ-4).
pub fn parse(src: &str) -> ParseResult {
    let (tokens, lex_errors) = tokenize(src);
    let mut parser = Parser::new(src, tokens, lex_errors);
    parser.parse_program();
    ParseResult {
        program: Program {
            items: parser.items,
        },
        errors: parser.errors,
    }
}

/// A parse error local to one item — carries enough to record + resync.
type PResult<T> = Result<T, SyntaxError>;

/// A parsed leading `#[...]` attribute: the `#[slag(...)]` field list or the
/// `#[boundary("...")]` foreign-target string (ffi-boundary.md REQ-3), or the
/// `#[sealed]` abstraction-barrier marker on a `struct`
/// (`.design/basis/06-provenance-and-sinks.md` REQ-8). `parse_attribute`
/// produces this; `parse_item` routes `Slag`/`Boundary` onto a `FnItem` and
/// `Sealed` onto a `StructItem`. A module-private dispatch type — the AST carries
/// the fn attributes as separate `Option`s and the struct seal as a `bool`, not
/// this union.
enum ParsedAttr {
    Slag(SlagAttr),
    Boundary(BoundaryAttr),
    /// `#[sealed]` on a `struct` (REQ-8): a bare marker (no body). Sets
    /// `StructItem.sealed`; the struct's own `span` covers the attribute, so the
    /// marker needs no payload.
    Sealed,
}

struct Parser<'a> {
    src: &'a str,
    tokens: Vec<Token>,
    pos: usize,
    items: Vec<Item>,
    errors: Vec<SyntaxError>,
    /// Current recursive-descent nesting depth (guards every recursive family —
    /// expressions, types, patterns, and the if-tail cycle — against stack
    /// overflow on deeply nested input — parser.md AC-4).
    recursion_depth: usize,
    /// When true, a path primary does NOT consume a following `{ … }` as a
    /// struct-literal (`.design/basis/01-adts.md` REQ-2): set for the
    /// `match`/`if`/`while` head positions so `match s { … }` reads `{` as the
    /// arm block, not `s { … }` as a struct lit (the Rust no-struct-literal
    /// context). Saved/restored around each head so nested call/index/paren
    /// args re-enable struct literals.
    no_struct_literal: bool,
    /// Current loop-nesting depth (parser.md REQ-10, #93). Incremented in
    /// `parse_loop_inner` around the loop body parse, decremented after. A
    /// `break;`/`continue;` parsed at depth 0 (outside any `loop`/`while` body)
    /// is a structural `SyntaxError` — analogous to the mandatory-clause rule
    /// (REQ-2): the parser owns presence/position; Verus owns the invariant/
    /// decreases semantics (`verus-lowering.md` REQ-12).
    loop_depth: usize,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str, tokens: Vec<Token>, lex_errors: Vec<SyntaxError>) -> Self {
        Parser {
            src,
            tokens,
            pos: 0,
            items: Vec::new(),
            errors: lex_errors,
            recursion_depth: 0,
            no_struct_literal: false,
            loop_depth: 0,
        }
    }

    /// Run `inner` with struct-literal parsing suppressed (the `match`/`if`/
    /// `while` head context), restoring the prior flag afterward
    /// (`.design/basis/01-adts.md` REQ-2). A nested call/index/paren-group
    /// re-enables struct literals via `with_struct_literal`.
    fn with_no_struct_literal<T>(
        &mut self,
        inner: impl FnOnce(&mut Self) -> PResult<T>,
    ) -> PResult<T> {
        let saved = self.no_struct_literal;
        self.no_struct_literal = true;
        let result = inner(self);
        self.no_struct_literal = saved;
        result
    }

    /// Run `inner` with struct-literal parsing RE-ENABLED inside a bracketed
    /// subexpression (call args, index, paren group) of a no-struct-literal head
    /// — `match f(A { x: 1 }) { … }` constructs `A { … }` even though the match
    /// scrutinee itself forbids a bare struct literal (Rust semantics).
    fn with_struct_literal<T>(
        &mut self,
        inner: impl FnOnce(&mut Self) -> PResult<T>,
    ) -> PResult<T> {
        let saved = self.no_struct_literal;
        self.no_struct_literal = false;
        let result = inner(self);
        self.no_struct_literal = saved;
        result
    }

    // ---- recursion-depth guard (parser.md AC-4) ----------------------------

    /// Bound the recursive-descent nesting depth: run `inner` one level deeper,
    /// returning a structured `ExpressionTooDeep` diagnostic (never a stack
    /// overflow / process abort) once the shared counter hits
    /// `MAX_RECURSION_DEPTH` (parser.md AC-4 / REQ-4; goal.md R-CODE-2).
    ///
    /// A SINGLE shared counter caps EVERY recursive family — expressions
    /// (`parse_expr`), types (`parse_type`), patterns (`parse_pattern`), and the
    /// `parse_block`/`parse_if_parts` if-tail cycle. The #29 fix incremented the
    /// counter only inside `parse_expr`, so the type/pattern/if-tail recursions
    /// bypassed it and still overflowed the C stack on deep input (#31); routing
    /// each family's recursive entry through this guard closes that gap. The
    /// counter is decremented on every exit path, so siblings (e.g. successive
    /// type arguments) do not accumulate depth.
    fn guard_recursion<T>(&mut self, inner: impl FnOnce(&mut Self) -> PResult<T>) -> PResult<T> {
        if self.recursion_depth >= MAX_RECURSION_DEPTH {
            return Err(SyntaxError::ExpressionTooDeep {
                limit: MAX_RECURSION_DEPTH,
                span: self.peek_span(),
            });
        }
        self.recursion_depth += 1;
        let result = inner(self);
        self.recursion_depth -= 1;
        result
    }

    // ---- cursor primitives -------------------------------------------------

    fn peek(&self) -> &TokKind {
        // The token stream always ends with `Eof`; `pos` never exceeds it.
        &self.tokens[self.pos.min(self.tokens.len() - 1)].kind
    }

    fn peek_span(&self) -> Span {
        self.tokens[self.pos.min(self.tokens.len() - 1)].span
    }

    /// Look ahead `n` tokens past the cursor without consuming (clamped to the
    /// trailing `Eof`). Used by the C10 ergonomics to distinguish `if`/`while`
    /// from `if let`/`while let` (`.design/basis/11-ergonomics.md` REQ-5) — the
    /// only lookahead in the parser beyond a single `peek`.
    fn peek_nth(&self, n: usize) -> &TokKind {
        &self.tokens[(self.pos + n).min(self.tokens.len() - 1)].kind
    }

    fn at_eof(&self) -> bool {
        matches!(self.peek(), TokKind::Eof)
    }

    fn bump(&mut self) -> Token {
        let tok = self.tokens[self.pos.min(self.tokens.len() - 1)].clone();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        tok
    }

    fn check(&self, kind: &TokKind) -> bool {
        self.peek() == kind
    }

    fn eat(&mut self, kind: &TokKind) -> bool {
        if self.check(kind) {
            self.bump();
            true
        } else {
            false
        }
    }

    /// Consume a token of the required kind or produce a `SyntaxError`.
    fn consume(&mut self, kind: &TokKind, what: &str) -> PResult<Token> {
        if self.check(kind) {
            Ok(self.bump())
        } else {
            Err(self.unexpected(what))
        }
    }

    fn unexpected(&self, expected: &str) -> SyntaxError {
        let span = self.peek_span();
        if self.at_eof() {
            SyntaxError::UnexpectedEof {
                expected: expected.to_string(),
                span,
            }
        } else {
            SyntaxError::Unexpected {
                expected: expected.to_string(),
                found: describe(self.peek()),
                span,
            }
        }
    }

    /// The verbatim source text covered by `span` (used for clause `text`).
    fn span_text(&self, span: Span) -> String {
        self.src
            .get(span.start..span.end())
            .unwrap_or("")
            .trim()
            .to_string()
    }

    // ---- top level: per-item recovery (REQ-3) ------------------------------

    fn parse_program(&mut self) {
        while !self.at_eof() {
            let start = self.pos;
            match self.parse_item() {
                Ok(item) => self.items.push(item),
                Err(err) => {
                    self.errors.push(err);
                    // Resync to the next item boundary so the broken item's
                    // tokens never bleed into the next (REQ-3).
                    self.resync_to_item_boundary(start);
                }
            }
        }
    }

    /// Discard tokens up to the next top-level item-start token (`fn`/`spec`/
    /// `#[`) or EOF. `min_start` guards against an infinite loop: we always make
    /// progress past where the failed item began.
    fn resync_to_item_boundary(&mut self, min_start: usize) {
        if self.pos == min_start && !self.at_eof() {
            self.bump();
        }
        while !self.at_eof() {
            if matches!(
                self.peek(),
                TokKind::Fn
                    | TokKind::Spec
                    | TokKind::HashBracket
                    | TokKind::Struct
                    | TokKind::Enum
            ) {
                break;
            }
            self.bump();
        }
    }

    // ---- items -------------------------------------------------------------

    fn parse_item(&mut self) -> PResult<Item> {
        let start_span = self.peek_span();
        // An optional leading `#[...]` attribute (`#[slag(...)]` or
        // `#[boundary("...")]`; ffi-boundary.md REQ-3). `parse_attribute`
        // dispatches on the name; `parse_item` routes the result to the fn.
        let attr = if self.check(&TokKind::HashBracket) {
            Some(self.parse_attribute()?)
        } else {
            None
        };

        // A `struct` item (`.design/basis/01-adts.md` REQ-1) accepts the
        // `#[sealed]` abstraction-barrier attribute (REQ-8) and NO other; an
        // `enum` (REQ-2) carries no attribute (only `fn`/sealed-`struct` do).
        if self.check(&TokKind::Struct) {
            let sealed = match &attr {
                Some(ParsedAttr::Sealed) => true,
                Some(ParsedAttr::Slag(_)) => {
                    return Err(self.unexpected("`fn` after `#[slag(...)]`"));
                }
                Some(ParsedAttr::Boundary(_)) => {
                    return Err(self.unexpected("`fn` after `#[boundary(\"...\")]`"));
                }
                None => false,
            };
            return self.parse_struct(start_span, sealed);
        }
        if self.check(&TokKind::Enum) {
            match &attr {
                Some(ParsedAttr::Slag(_)) => {
                    return Err(self.unexpected("`fn` after `#[slag(...)]`"));
                }
                Some(ParsedAttr::Boundary(_)) => {
                    return Err(self.unexpected("`fn` after `#[boundary(\"...\")]`"));
                }
                // `#[sealed]` is an abstraction barrier for a struct clean type
                // (REQ-8); it does not attach to an `enum`.
                Some(ParsedAttr::Sealed) => {
                    return Err(self.unexpected("`struct` after `#[sealed]`"));
                }
                None => {}
            }
            return self.parse_enum(start_span);
        }

        if self.check(&TokKind::Spec) {
            // Neither `#[slag]` nor `#[boundary]` attaches to a `spec fn`
            // (surface-grammar Item; ffi-boundary.md "#[boundary] is NOT valid on
            // a spec fn"); `#[sealed]` is a `struct`-only barrier (REQ-8).
            match &attr {
                Some(ParsedAttr::Slag(_)) => {
                    return Err(self.unexpected("`fn` after `#[slag(...)]`"));
                }
                Some(ParsedAttr::Boundary(_)) => {
                    return Err(self.unexpected("`fn` after `#[boundary(\"...\")]`"));
                }
                Some(ParsedAttr::Sealed) => {
                    return Err(self.unexpected("`struct` after `#[sealed]`"));
                }
                None => {}
            }
            self.parse_spec_fn(start_span)
        } else if self.check(&TokKind::Fn) {
            let (slag, boundary) = match attr {
                Some(ParsedAttr::Slag(s)) => (Some(s), None),
                Some(ParsedAttr::Boundary(b)) => (None, Some(b)),
                // `#[sealed]` is a `struct`-only abstraction barrier (REQ-8); a
                // door is a `#[boundary]` fn, never `#[sealed]`.
                Some(ParsedAttr::Sealed) => {
                    return Err(self.unexpected("`struct` after `#[sealed]`"));
                }
                None => (None, None),
            };
            self.parse_fn(slag, boundary, start_span)
        } else {
            Err(self.unexpected(
                "`fn`, `spec fn`, `#[slag(...)]`, `#[boundary(\"...\")]`, or `#[sealed] struct`",
            ))
        }
    }

    /// Parse a leading `#[...]` attribute, dispatching on its name (ffi-boundary.md
    /// REQ-3): `slag` -> the `SlagAttr` field-list path, `boundary` -> a single
    /// positional `("crate::path")` string -> a `BoundaryAttr`. Generalizes the
    /// former name-hardcoded `parse_slag`.
    fn parse_attribute(&mut self) -> PResult<ParsedAttr> {
        let start = self.peek_span();
        self.consume(&TokKind::HashBracket, "`#[`")?;
        let name = self.take_ident("`slag` or `boundary`")?;
        match name.as_str() {
            "slag" => Ok(ParsedAttr::Slag(self.parse_slag_body(start)?)),
            "boundary" => Ok(ParsedAttr::Boundary(self.parse_boundary_body(start)?)),
            // `#[sealed]` (`.design/basis/06-provenance-and-sinks.md` REQ-8): a
            // bare marker on a `struct`, no body — just the closing `]`. Mirrors
            // the `slag`/`boundary` dispatch but reads no parenthesized body.
            "sealed" => {
                self.consume(&TokKind::RBracket, "`]`")?;
                Ok(ParsedAttr::Sealed)
            }
            _ => Err(SyntaxError::Unexpected {
                expected: "`slag`, `boundary`, or `sealed`".to_string(),
                found: format!("identifier `{name}`"),
                span: start,
            }),
        }
    }

    /// Parse a `#[boundary("crate::path")]` attribute body: a single positional
    /// string literal naming the foreign target (ffi-boundary.md REQ-1/OQ-1).
    /// `start` is the span of the opening `#[` (for the attribute span).
    fn parse_boundary_body(&mut self, start: Span) -> PResult<BoundaryAttr> {
        self.consume(&TokKind::LParen, "`(`")?;
        let target = self.take_string("a foreign-target string `\"crate::path\"`")?;
        let end = self.peek_span();
        self.consume(&TokKind::RParen, "`)`")?;
        self.consume(&TokKind::RBracket, "`]`")?;
        Ok(BoundaryAttr {
            target,
            span: start.to(end),
        })
    }

    /// Parse a `#[slag(...)]` attribute body (the `key = "value"` field list).
    /// `start` is the span of the opening `#[`. The `#[` and the `slag` name are
    /// already consumed by `parse_attribute`.
    fn parse_slag_body(&mut self, start: Span) -> PResult<SlagAttr> {
        self.consume(&TokKind::LParen, "`(`")?;
        let mut reason = None;
        let mut owner = None;
        let mut review = None;
        if !self.check(&TokKind::RParen) {
            loop {
                let field = self.take_ident("a slag field name")?;
                self.consume(&TokKind::Eq, "`=`")?;
                let value = self.take_string("a string value")?;
                match field.as_str() {
                    "reason" => reason = Some(value),
                    "owner" => owner = Some(value),
                    "review" => review = Some(value),
                    // The lexer/parser do not validate field names — that is a
                    // downstream (§8/forge) check. Keep unknown fields out of
                    // the structured node but do not error.
                    _ => {}
                }
                if !self.eat(&TokKind::Comma) {
                    break;
                }
                if self.check(&TokKind::RParen) {
                    break;
                }
            }
        }
        let end = self.peek_span();
        self.consume(&TokKind::RParen, "`)`")?;
        self.consume(&TokKind::RBracket, "`]`")?;
        Ok(SlagAttr {
            reason,
            owner,
            review,
            span: start.to(end),
        })
    }

    fn parse_fn(
        &mut self,
        slag: Option<SlagAttr>,
        boundary: Option<BoundaryAttr>,
        start_span: Span,
    ) -> PResult<Item> {
        self.consume(&TokKind::Fn, "`fn`")?;
        let name = self.take_ident("a function name")?;
        let params = self.parse_params()?;
        self.consume(&TokKind::Arrow, "`->`")?;
        let ret = self.parse_type()?;
        let contract = self.parse_contract(&name)?;
        // The OPTIONAL `dec <measure>` termination clause of a RECURSIVE exec `fn`
        // (`.design/basis/10-recursion-tuples.md` REQ-1, C9-A). It parses AFTER the
        // contract (`req`/`ens`/`fx`) and BEFORE the body — the OQ-4 byte-stable
        // slot mirroring the loop order (`inv`s then `dec`). Absent → `None` (a
        // non-recursive `fn`); the `req`/`ens`/`fx` parse is UNCHANGED for every
        // existing non-recursive fn. A self-calling fn LACKING this clause (and not
        // `fx diverge`) is a validator error (REQ-2), not a parse error — the
        // grammar admits it; the cage rejects it.
        let dec = if self.check(&TokKind::Dec) {
            Some(self.parse_clause(&TokKind::Dec)?)
        } else {
            None
        };
        // Body fork (ffi-boundary.md REQ-3, OQ-2): a `#[boundary]` fn is bodyless
        // — terminated by `;` (the foreign body lives in the foreign crate); a
        // non-`#[boundary]` fn REQUIRES a `{ }` body (the §4.1 body-second rule).
        // The `;` body is VALID ONLY when `boundary.is_some()`: a bodyless fn
        // WITHOUT `#[boundary]` is a clear parse error, never silently a boundary
        // fn (a normal fn missing its body must not be mistaken for a foreign one).
        let body = if boundary.is_some() {
            // A foreign fn MUST be bodyless: `;`, not `{ }`. A `{ }` body on a
            // `#[boundary]` fn is an error — there is no Thermite body to prove.
            if self.check(&TokKind::LBrace) {
                return Err(SyntaxError::Unexpected {
                    expected: "`;` (a `#[boundary]` fn is bodyless — its body is foreign)"
                        .to_string(),
                    found: describe(self.peek()),
                    span: self.peek_span(),
                });
            }
            self.consume(
                &TokKind::Semi,
                "`;` to end the bodyless `#[boundary]` fn (its body is foreign)",
            )?;
            None
        } else {
            // A non-boundary fn MUST have a `{ }` body. A `;` here is the OQ-2
            // case — a bodyless fn WITHOUT `#[boundary]`: a clear, distinct error,
            // not a silent boundary fn.
            if self.check(&TokKind::Semi) {
                return Err(SyntaxError::Unexpected {
                    expected: "`{` (a non-`#[boundary]` fn requires a `{ }` body; \
                               only a `#[boundary(\"...\")]` fn is bodyless)"
                        .to_string(),
                    found: describe(self.peek()),
                    span: self.peek_span(),
                });
            }
            Some(self.parse_block()?)
        };
        let span = start_span.to(self.prev_span());
        Ok(Item::Fn(FnItem {
            slag,
            boundary,
            name,
            params,
            ret,
            contract,
            dec,
            body,
            span,
        }))
    }

    fn parse_spec_fn(&mut self, start_span: Span) -> PResult<Item> {
        self.consume(&TokKind::Spec, "`spec`")?;
        self.consume(&TokKind::Fn, "`fn`")?;
        let name = self.take_ident("a function name")?;
        let params = self.parse_params()?;
        self.consume(&TokKind::Arrow, "`->`")?;
        let ret = self.parse_type()?;
        // A spec fn carries exactly one `dec` measure, no req/ens/fx (§4.2).
        if !self.check(&TokKind::Dec) {
            return Err(SyntaxError::MissingClause {
                item: name,
                clause: "dec".to_string(),
                span: self.peek_span(),
            });
        }
        let dec = self.parse_clause(&TokKind::Dec)?;
        let body = self.parse_block()?;
        let span = start_span.to(self.prev_span());
        Ok(Item::SpecFn(SpecFnItem {
            name,
            params,
            ret,
            dec,
            body,
            span,
        }))
    }

    /// Parse a `[#[sealed]] struct NAME { field: TYPE, … } [inv <expr>]` item
    /// (`.design/basis/01-adts.md` REQ-1; the seal is
    /// `.design/basis/06-provenance-and-sinks.md` REQ-8). The optional `inv`
    /// type-invariant clause follows the closing brace and reuses the existing
    /// `Clause` (verbatim text + parsed expr). `sealed` is the `#[sealed]`
    /// abstraction-barrier flag the caller already parsed from the leading
    /// attribute (REQ-8). The VALIDATOR rules (field well-formedness; the
    /// sealed-construction reject) are stage 1b / Stage 6; here we only parse the
    /// surface into the right AST.
    fn parse_struct(&mut self, start_span: Span, sealed: bool) -> PResult<Item> {
        self.consume(&TokKind::Struct, "`struct`")?;
        let name = self.take_ident("a struct name")?;
        let fields = self.parse_field_defs()?;
        // The optional `inv <expr>` type-invariant clause (REQ-1) follows the
        // field block. Absent -> `None` (a struct may declare no invariant).
        let inv = if self.check(&TokKind::Inv) {
            Some(self.parse_clause(&TokKind::Inv)?)
        } else {
            None
        };
        let span = start_span.to(self.prev_span());
        Ok(Item::Struct(StructItem {
            name,
            fields,
            inv,
            sealed,
            span,
        }))
    }

    /// Parse a `{ field: TYPE, … }` field-definition block, shared by `struct`
    /// items and struct-shaped enum variants (`.design/basis/01-adts.md`
    /// REQ-1/REQ-2). A trailing comma is permitted.
    fn parse_field_defs(&mut self) -> PResult<Vec<FieldDef>> {
        self.consume(&TokKind::LBrace, "`{`")?;
        let mut fields = Vec::new();
        if !self.check(&TokKind::RBrace) {
            loop {
                let name = self.take_ident("a field name")?;
                self.consume(&TokKind::Colon, "`:`")?;
                let ty = self.parse_type()?;
                fields.push(FieldDef { name, ty });
                if !self.eat(&TokKind::Comma) {
                    break;
                }
                if self.check(&TokKind::RBrace) {
                    break;
                }
            }
        }
        self.consume(&TokKind::RBrace, "`}`")?;
        Ok(fields)
    }

    /// Parse an `enum NAME { Variant, Variant(TYPE, …), Variant { field: TYPE, … }
    /// }` item (`.design/basis/01-adts.md` REQ-2). A variant is `Unit` (bare
    /// name), `Tuple` (`(TYPE, …)`), or `Struct` (`{ field: TYPE, … }`). A
    /// trailing comma is permitted. Recursive `Box<List>` self-refs parse via
    /// `parse_type` (REQ-3).
    fn parse_enum(&mut self, start_span: Span) -> PResult<Item> {
        self.consume(&TokKind::Enum, "`enum`")?;
        let name = self.take_ident("an enum name")?;
        self.consume(&TokKind::LBrace, "`{`")?;
        let mut variants = Vec::new();
        if !self.check(&TokKind::RBrace) {
            loop {
                let vname = self.take_ident("a variant name")?;
                let shape = if self.check(&TokKind::LParen) {
                    // Tuple variant `Circle(u64)` / `Cons(u64, Box<List>)`.
                    self.bump();
                    let mut tys = Vec::new();
                    if !self.check(&TokKind::RParen) {
                        loop {
                            tys.push(self.parse_type()?);
                            if !self.eat(&TokKind::Comma) {
                                break;
                            }
                            if self.check(&TokKind::RParen) {
                                break;
                            }
                        }
                    }
                    self.consume(&TokKind::RParen, "`)`")?;
                    VariantShape::Tuple(tys)
                } else if self.check(&TokKind::LBrace) {
                    // Struct variant `Rect { w: u64, h: u64 }`.
                    VariantShape::Struct(self.parse_field_defs()?)
                } else {
                    // Unit variant `Nil`.
                    VariantShape::Unit
                };
                variants.push(VariantDef { name: vname, shape });
                if !self.eat(&TokKind::Comma) {
                    break;
                }
                if self.check(&TokKind::RBrace) {
                    break;
                }
            }
        }
        self.consume(&TokKind::RBrace, "`}`")?;
        let span = start_span.to(self.prev_span());
        Ok(Item::Enum(EnumItem {
            name,
            variants,
            span,
        }))
    }

    fn parse_params(&mut self) -> PResult<Vec<Param>> {
        self.consume(&TokKind::LParen, "`(`")?;
        let mut params = Vec::new();
        if !self.check(&TokKind::RParen) {
            loop {
                let name = self.take_ident("a parameter name")?;
                self.consume(&TokKind::Colon, "`:`")?;
                let ty = self.parse_type()?;
                params.push(Param { name, ty });
                if !self.eat(&TokKind::Comma) {
                    break;
                }
                if self.check(&TokKind::RParen) {
                    break;
                }
            }
        }
        self.consume(&TokKind::RParen, "`)`")?;
        Ok(params)
    }

    /// Parse the mandatory contract `req` then `ens`+ then `fx`, in that exact
    /// order (parser.md REQ-2). Absence or misorder is a `SyntaxError`.
    fn parse_contract(&mut self, fn_name: &str) -> PResult<Contract> {
        // `req` — exactly one, first.
        if !self.check(&TokKind::Req) {
            // If `ens`/`fx` appear first, that is an order error; otherwise the
            // clause is simply absent.
            if matches!(self.peek(), TokKind::Ens | TokKind::Fx) {
                return Err(SyntaxError::ClauseOrder {
                    item: fn_name.to_string(),
                    clause: "req".to_string(),
                    span: self.peek_span(),
                });
            }
            return Err(SyntaxError::MissingClause {
                item: fn_name.to_string(),
                clause: "req".to_string(),
                span: self.peek_span(),
            });
        }
        let req = self.parse_clause(&TokKind::Req)?;

        // `ens` — one or more.
        if !self.check(&TokKind::Ens) {
            return Err(SyntaxError::MissingClause {
                item: fn_name.to_string(),
                clause: "ens".to_string(),
                span: self.peek_span(),
            });
        }
        let mut ens = Vec::new();
        while self.check(&TokKind::Ens) {
            ens.push(self.parse_clause(&TokKind::Ens)?);
        }

        // A stray `req` after `ens` is an order error (req must be first).
        if self.check(&TokKind::Req) {
            return Err(SyntaxError::ClauseOrder {
                item: fn_name.to_string(),
                clause: "req".to_string(),
                span: self.peek_span(),
            });
        }

        // `fx` — exactly one, last.
        if !self.check(&TokKind::Fx) {
            return Err(SyntaxError::MissingClause {
                item: fn_name.to_string(),
                clause: "fx".to_string(),
                span: self.peek_span(),
            });
        }
        let fx = self.parse_effect_row()?;

        Ok(Contract { req, ens, fx })
    }

    /// Parse one `KEYWORD EXPR` clause, capturing the verbatim source text of
    /// the expression for addressing (`Clause.text`).
    fn parse_clause(&mut self, keyword: &TokKind) -> PResult<Clause> {
        self.consume(keyword, "a clause keyword")?;
        let start = self.peek_span();
        // A clause expression is a no-struct-literal head: a clause is followed
        // by another clause keyword or a block `{` (a loop body, a spec-fn body),
        // so a trailing `Name { … }` must NOT be read as a struct literal
        // (`.design/basis/01-adts.md` REQ-2; e.g. `dec xs.len() - i { … }` —
        // the `{` is the body). Struct literals inside call args / parens still
        // parse (those re-enable the context).
        let expr = self.with_no_struct_literal(Self::parse_expr)?;
        let end = self.prev_span();
        let span = start.to(end);
        let text = self.span_text(span);
        Ok(Clause { expr, text, span })
    }

    fn parse_effect_row(&mut self) -> PResult<EffectRow> {
        self.consume(&TokKind::Fx, "`fx`")?;
        if self.eat(&TokKind::Pure) {
            return Ok(EffectRow::Pure);
        }
        let mut effects = Vec::new();
        loop {
            effects.push(self.parse_effect()?);
            if !self.eat(&TokKind::Comma) {
                break;
            }
        }
        Ok(EffectRow::Set(effects))
    }

    fn parse_effect(&mut self) -> PResult<Effect> {
        let name = self.take_ident("an effect name")?;
        match name.as_str() {
            "read" | "write" | "net" => {
                self.consume(&TokKind::LParen, "`(`")?;
                let arg = self.take_ident("an effect path argument")?;
                self.consume(&TokKind::RParen, "`)`")?;
                Ok(match name.as_str() {
                    "read" => Effect::Read(arg),
                    "write" => Effect::Write(arg),
                    _ => Effect::Net(arg),
                })
            }
            "alloc" => Ok(Effect::Alloc),
            "time" => Ok(Effect::Time),
            "rand" => Ok(Effect::Rand),
            "panic" => Ok(Effect::Panic),
            "diverge" => Ok(Effect::Diverge),
            "term" => Ok(Effect::Term),
            _ => Err(SyntaxError::Unexpected {
                expected: "an effect (read/write/net/alloc/time/rand/panic/diverge/term)"
                    .to_string(),
                found: format!("identifier `{name}`"),
                span: self.prev_span(),
            }),
        }
    }

    // ---- blocks + statements ----------------------------------------------

    fn parse_block(&mut self) -> PResult<Block> {
        self.consume(&TokKind::LBrace, "`{`")?;
        let mut stmts = Vec::new();
        let mut tail = None;
        while !self.check(&TokKind::RBrace) && !self.at_eof() {
            // Statement keywords that are not expression-starting.
            match self.peek() {
                // `let` (incl. the C10 tuple-destructure `let (x, y) = e;`, which
                // desugars to a temp + N projection `let`s — REQ-1). `parse_let`
                // returns 1+ statements; extend the block with all of them.
                TokKind::Let => {
                    let lets = self.parse_let()?;
                    stmts.extend(lets);
                }
                TokKind::Return => stmts.push(self.parse_return()?),
                TokKind::Loop => {
                    stmts.push(Stmt::Loop(self.parse_loop()?));
                }
                // `while` is the bare loop OR the C10 `while let P = e inv … { B }`
                // ergonomic (REQ-5), distinguished by a `let` after `while`. The
                // `while let` form desugars to a `while (e is Variant)` loop.
                TokKind::While => {
                    if matches!(self.peek_nth(1), TokKind::Let) {
                        stmts.push(Stmt::Loop(self.parse_while_let()?));
                    } else {
                        stmts.push(Stmt::Loop(self.parse_loop()?));
                    }
                }
                // `for i in lo..hi inv … { B }` — the C10 bounded-range loop
                // ergonomic (REQ-2). `for`/`in` are contextual identifiers (NOT
                // reserved keywords, matched by name like `Box`/`Vec`), so the
                // token here is `Ident("for")`. The desugar produces a `let mut i`
                // statement + a `while` loop, so it extends the block.
                TokKind::Ident(name) if name == "for" => {
                    let stmts_for = self.parse_for()?;
                    stmts.extend(stmts_for);
                }
                // `break;` / `continue;` (parser.md REQ-10, #93). Loop-control
                // statements: payload-less, value-less, require a trailing `;`,
                // and are valid only inside a loop body (the in-loop structural
                // rule — `self.loop_depth > 0`).
                TokKind::Break => stmts.push(self.parse_break_continue(true)?),
                TokKind::Continue => stmts.push(self.parse_break_continue(false)?),
                // `if let P = e { T } else { E }` — the C10 ergonomic (REQ-5),
                // distinguished by a `let` after `if`. It desugars to the SHIPPED
                // `Expr::Match { e, [P => T, _ => E] }`. In tail position (an `else`
                // + a value-producing then-tail + nothing after) it is the block
                // tail; otherwise a `Stmt::Expr` (the `_ => ()` arm when no `else`).
                TokKind::If if matches!(self.peek_nth(1), TokKind::Let) => {
                    let (match_expr, value_tail) = self.parse_if_let()?;
                    if value_tail && self.check(&TokKind::RBrace) {
                        tail = Some(Box::new(match_expr));
                        break;
                    }
                    stmts.push(Stmt::Expr(match_expr));
                }
                TokKind::If => {
                    // `if` is both a statement and an expression
                    // (surface-grammar.md decision 2). The discriminator is
                    // VALUE-NESS, not source position: "the expression form ...
                    // must have a value; the statement form does not." (OQ-3:
                    // "the corpus only uses the statement form".) It is the
                    // block's TAIL VALUE only when it (a) has an `else`, (b)
                    // produces a value — its then-branch block has a tail expr,
                    // `then.tail.is_some()` — AND (c) nothing follows it before
                    // the closing `}` (ast.md REQ-6 `Expr::If`). A value-LESS
                    // trailing `if/else` (both branches statement-only, e.g.
                    // corpus `if .. { lo = mid + 1; } else { hi = mid; }`) is the
                    // STATEMENT form and leaves the block `tail: None`.
                    let (cond, then, else_) = self.parse_if_parts()?;
                    if let Some(else_block) = else_ {
                        if self.check(&TokKind::RBrace) && then.tail.is_some() {
                            // Value position: the if/else is the block tail.
                            tail = Some(Box::new(Expr::If {
                                cond: Box::new(cond),
                                then,
                                else_: else_block,
                            }));
                            break;
                        }
                        stmts.push(Stmt::If {
                            cond,
                            then,
                            else_: Some(else_block),
                        });
                    } else {
                        stmts.push(Stmt::If {
                            cond,
                            then,
                            else_: None,
                        });
                    }
                }
                _ => {
                    // Expression statement, assignment, or trailing tail expr.
                    let expr = self.parse_expr()?;
                    if self.eat(&TokKind::Eq) {
                        // Assignment: LVALUE = EXPR ;
                        let value = self.parse_expr()?;
                        self.consume(&TokKind::Semi, "`;`")?;
                        stmts.push(Stmt::Assign {
                            target: expr,
                            value,
                        });
                    } else if self.eat(&TokKind::Semi) {
                        stmts.push(Stmt::Expr(expr));
                    } else {
                        // No `;` and no `=`: this is the block's tail value.
                        tail = Some(Box::new(expr));
                        break;
                    }
                }
            }
        }
        self.consume(&TokKind::RBrace, "`}`")?;
        Ok(Block { stmts, tail })
    }

    /// Parse a `let` binding. Returns 1+ statements: a scalar `let x = e;` is one
    /// `Stmt::Let`; the C10 tuple-destructure `let (x, y) = e;`
    /// (`.design/basis/11-ergonomics.md` REQ-1) DESUGARS, in the parser, to a
    /// fresh temp `let __td<n> = e;` plus one `let x = __td<n>.0;` /
    /// `let y = __td<n>.1;` per element — reusing the SHIPPED `Expr::TupleProj`
    /// (C9-B). PURE-DESUGAR: no new AST node, the projection lowers + verifies
    /// today. v0.1 admits only flat binding/`_` sub-patterns in a tuple `let`
    /// (a nested `let (Some(x), y) = …` is out of scope — §2.3 one-way).
    fn parse_let(&mut self) -> PResult<Vec<Stmt>> {
        self.consume(&TokKind::Let, "`let`")?;
        let mutable = self.eat(&TokKind::Mut);
        // A `(` here opens a tuple-destructuring pattern `let (x, y) = e;` (REQ-1).
        if self.check(&TokKind::LParen) {
            return self.parse_let_tuple_destructure(mutable);
        }
        let name = self.take_ident("a binding name")?;
        let ty = if self.eat(&TokKind::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };
        self.consume(&TokKind::Eq, "`=`")?;
        let init = self.parse_expr()?;
        self.consume(&TokKind::Semi, "`;`")?;
        Ok(vec![Stmt::Let {
            mutable,
            name,
            ty,
            init,
        }])
    }

    /// Desugar a tuple-destructuring `let (x, y, …) = e;` to a temp + per-element
    /// projection `let`s (`.design/basis/11-ergonomics.md` REQ-1). The element
    /// sub-patterns are flat: a `Binding` name (`x`) becomes
    /// `let [mut] x = __td<n>.<i>;`, a `Wildcard` (`_`) drops that element (no
    /// `let`). The temp `__td<n>` uses the let's start byte as a unique suffix so
    /// nested/sibling destructures never collide. The temp init re-enables struct
    /// literals (it is a value-position initializer).
    fn parse_let_tuple_destructure(&mut self, mutable: bool) -> PResult<Vec<Stmt>> {
        let start = self.peek_span();
        self.consume(&TokKind::LParen, "`(` to open a tuple-destructuring `let`")?;
        // Collect the flat element sub-patterns: a binding name or `_`.
        let mut elems: Vec<Option<Ident>> = Vec::new();
        if !self.check(&TokKind::RParen) {
            loop {
                if let TokKind::Ident(name) = self.peek().clone() {
                    self.bump();
                    if name == "_" {
                        elems.push(None);
                    } else {
                        elems.push(Some(name));
                    }
                } else {
                    return Err(self.unexpected(
                        "a binding name or `_` in a tuple-destructuring `let` \
                         (v0.1 admits only flat names — a nested pattern is out of scope)",
                    ));
                }
                if !self.eat(&TokKind::Comma) {
                    break;
                }
                if self.check(&TokKind::RParen) {
                    break;
                }
            }
        }
        self.consume(
            &TokKind::RParen,
            "`)` to close the tuple-destructuring `let`",
        )?;
        self.consume(&TokKind::Eq, "`=` after a tuple-destructuring `let`")?;
        let init = self.parse_expr()?;
        self.consume(&TokKind::Semi, "`;`")?;
        // A fresh, collision-free temp name keyed on the byte offset (deterministic
        // — goal.md R-CODE-5).
        let temp = format!("__td{}", start.start);
        let mut out = Vec::with_capacity(elems.len() + 1);
        out.push(Stmt::Let {
            mutable: false,
            name: temp.clone(),
            ty: None,
            init,
        });
        for (i, elem) in elems.into_iter().enumerate() {
            if let Some(name) = elem {
                out.push(Stmt::Let {
                    mutable,
                    name,
                    ty: None,
                    init: Expr::TupleProj {
                        receiver: Box::new(Expr::Path(vec![temp.clone()])),
                        index: i,
                    },
                });
            }
        }
        Ok(out)
    }

    /// Parse + desugar a C10 `for i in lo..hi inv … { B }` bounded-range loop
    /// (`.design/basis/11-ergonomics.md` REQ-2). `for`/`in` are CONTEXTUAL
    /// identifiers (not reserved keywords), so the caller dispatched on
    /// `Ident("for")`. PURE-DESUGAR to the SHIPPED `while`+`inv`/`dec` core:
    ///   `let mut i = lo;`
    ///   `while i < hi inv <user invs> dec hi - i { B; i = i + 1; }`
    /// The user supplies the `inv` (mandatory, §4.1 — at least one); the `dec` is
    /// AUTOMATIC (`hi - i`, the canonical monotone measure of a bounded range —
    /// strictly decreases on each `i = i + 1`, floored at 0). Returns the `let mut
    /// i` + the `while` loop as two statements.
    fn parse_for(&mut self) -> PResult<Vec<Stmt>> {
        let start = self.peek_span();
        // `for` (contextual ident).
        let kw = self.take_ident("`for`")?;
        if kw != "for" {
            return Err(self.unexpected("`for`"));
        }
        let var = self.take_ident("a `for` loop variable")?;
        // `in` (contextual ident).
        let in_kw = self.take_ident("`in` after the `for` loop variable")?;
        if in_kw != "in" {
            return Err(self.unexpected("`in` after the `for` loop variable"));
        }
        // The range `lo..hi` is a no-struct-literal head (the `{` after `hi`/the
        // inv clauses opens the body, never a struct literal — mirrors `while`).
        let (lo, hi) = self.with_no_struct_literal(|p| {
            let lo = p.parse_expr()?;
            p.consume(
                &TokKind::DotDot,
                "`..` in the `for` range `lo..hi` (only an exclusive integer range is admitted)",
            )?;
            let hi = p.parse_expr()?;
            Ok((lo, hi))
        })?;
        // `inv` — one or more (mandatory; the for-loop is a loop, §4.1). NO `dec`
        // — it is synthesized below (REQ-2).
        if !self.check(&TokKind::Inv) {
            return Err(SyntaxError::MissingClause {
                item: "for".to_string(),
                clause: "inv".to_string(),
                span: self.peek_span(),
            });
        }
        let mut invs = Vec::new();
        while self.check(&TokKind::Inv) {
            invs.push(self.parse_clause(&TokKind::Inv)?);
        }
        // A `dec` on a `for` is an error — the `dec` is automatic (REQ-2).
        if self.check(&TokKind::Dec) {
            return Err(SyntaxError::Unexpected {
                expected: "the loop body `{` (a `for` loop's `dec` is automatic — \
                           `dec hi - i` — so the user writes no `dec`)"
                    .to_string(),
                found: describe(self.peek()),
                span: self.peek_span(),
            });
        }
        // Parse the body at loop depth +1 (break/continue are valid inside).
        self.loop_depth += 1;
        let body_result = self.parse_block();
        self.loop_depth -= 1;
        let mut body = body_result?;
        // Append the auto-step `i = i + 1;` to the body.
        body.stmts.push(Stmt::Assign {
            target: Expr::Path(vec![var.clone()]),
            value: Expr::Binary {
                op: BinOp::Add,
                lhs: Box::new(Expr::Path(vec![var.clone()])),
                rhs: Box::new(Expr::IntLit {
                    value: 1,
                    raw: "1".to_string(),
                }),
            },
        });
        // The auto `dec hi - i` clause: a single `Clause` whose expr is `hi - i`.
        let dec_expr = Expr::Binary {
            op: BinOp::Sub,
            lhs: Box::new(hi.clone()),
            rhs: Box::new(Expr::Path(vec![var.clone()])),
        };
        let dec = Clause {
            expr: dec_expr,
            text: "hi - i".to_string(),
            span: start,
        };
        // The loop condition `i < hi`.
        let cond = Expr::Binary {
            op: BinOp::Lt,
            lhs: Box::new(Expr::Path(vec![var.clone()])),
            rhs: Box::new(hi),
        };
        let span = start.to(self.prev_span());
        Ok(vec![
            Stmt::Let {
                mutable: true,
                name: var,
                ty: None,
                init: lo,
            },
            Stmt::Loop(LoopNode {
                kind: LoopKind::While(Box::new(cond)),
                invs,
                dec,
                body,
                span,
            }),
        ])
    }

    /// Parse + desugar a C10 `if let P = e { T } else { E }`
    /// (`.design/basis/11-ergonomics.md` REQ-5). PURE-DESUGAR to the SHIPPED
    /// `Expr::Match { e, [P => T, _ => E] }`. v0.1 admits the VALUE form (both
    /// branches reduce to a tail expression) with a mandatory `else` — the
    /// statement-`if`-without-`else` `_ => ()` form needs a unit expr the grammar
    /// does not surface (OQ-4). Returns the `Expr::Match` and whether it is in
    /// value (tail) position (always true here — the value form). The caller
    /// places it as the block tail or a `Stmt::Expr`.
    fn parse_if_let(&mut self) -> PResult<(Expr, bool)> {
        self.consume(&TokKind::If, "`if`")?;
        self.consume(&TokKind::Let, "`let`")?;
        let pattern = self.parse_pattern()?;
        self.consume(&TokKind::Eq, "`=` in `if let P = e`")?;
        // The scrutinee is a no-struct-literal head (the `{` opens the then-block).
        let scrutinee = self.with_no_struct_literal(Self::parse_expr)?;
        let then = self.parse_block()?;
        self.consume(
            &TokKind::Else,
            "`else` (a v0.1 `if let` requires an `else` — its branches must produce a value)",
        )?;
        let else_ = self.parse_block()?;
        let then_body = self.block_into_arm_body(then, "the `if let` then-branch")?;
        let else_body = self.block_into_arm_body(else_, "the `if let` else-branch")?;
        let match_expr = Expr::Match {
            scrutinee: Box::new(scrutinee),
            arms: vec![
                MatchArm {
                    pattern,
                    guard: None,
                    body: then_body,
                },
                MatchArm {
                    pattern: Pattern::Wildcard,
                    guard: None,
                    body: else_body,
                },
            ],
        };
        Ok((match_expr, true))
    }

    /// Reduce a single-tail-expression `Block` to its arm-body `Expr`
    /// (`.design/basis/11-ergonomics.md` REQ-5). A v0.1 `if let` branch is a
    /// value-producing block whose body IS its tail expression (`{ v }`); a
    /// statement-bearing branch is out of scope (the desugar target is a `match`
    /// arm body, an `Expr`, not a block). A branch with no tail (or with leading
    /// statements) is a structured `SyntaxError`, never silently dropped.
    fn block_into_arm_body(&self, block: Block, what: &str) -> PResult<Expr> {
        if !block.stmts.is_empty() {
            return Err(SyntaxError::Unexpected {
                expected: format!(
                    "a single value expression in {what} \
                     (a v0.1 `if let` branch is `{{ value }}` — no leading statements)"
                ),
                found: "a statement".to_string(),
                span: self.peek_span(),
            });
        }
        match block.tail {
            Some(tail) => Ok(*tail),
            None => Err(SyntaxError::Unexpected {
                expected: format!("a value expression in {what} (its branch must produce a value)"),
                found: "an empty/value-less block".to_string(),
                span: self.peek_span(),
            }),
        }
    }

    /// Parse + desugar a C10 `while let Variant(_) = e inv … dec … { B }`
    /// (`.design/basis/11-ergonomics.md` REQ-5). PINNED (GROUNDED): desugar to the
    /// canonical `while (e is Variant)` form (NOT `loop { match … None => break }`
    /// — the loop+break shape fails to carry the post-exit fact, L0). v0.1 admits
    /// a PAYLOAD-FREE pattern (`Variant`, `Variant(_)`, `Variant { .. }`) — the
    /// condition is `e is Variant` (the SHIPPED `Expr::Is`), no payload rebind.
    /// The user supplies the loop `inv`/`dec` exactly as for a `while` (mandatory,
    /// §4.1). Returns the `LoopNode`.
    fn parse_while_let(&mut self) -> PResult<LoopNode> {
        let start = self.peek_span();
        self.consume(&TokKind::While, "`while`")?;
        self.consume(&TokKind::Let, "`let`")?;
        let pattern = self.parse_pattern()?;
        // Extract the variant head of the payload-free pattern (the SHIPPED
        // `Expr::Is` discriminant). A binding/wildcard pattern is rejected: a
        // `while let` must discriminate a variant (`e is Variant`).
        let variant = match &pattern {
            Pattern::Enum { path, .. } | Pattern::Struct { path, .. } => path.clone(),
            _ => {
                return Err(self.unexpected(
                    "a variant pattern after `while let` (e.g. `Some(_)` — v0.1 admits a \
                     payload-free variant; the loop runs while `e is Variant`)",
                ));
            }
        };
        self.consume(&TokKind::Eq, "`=` in `while let P = e`")?;
        // The scrutinee is a no-struct-literal head.
        let scrutinee = self.with_no_struct_literal(Self::parse_expr)?;
        // `inv` — one or more (mandatory, §4.1).
        if !self.check(&TokKind::Inv) {
            return Err(SyntaxError::MissingClause {
                item: "while".to_string(),
                clause: "inv".to_string(),
                span: self.peek_span(),
            });
        }
        let mut invs = Vec::new();
        while self.check(&TokKind::Inv) {
            invs.push(self.parse_clause(&TokKind::Inv)?);
        }
        // `dec` — exactly one (mandatory, §4.1; a `while let` is a `while`).
        if !self.check(&TokKind::Dec) {
            return Err(SyntaxError::MissingClause {
                item: "while".to_string(),
                clause: "dec".to_string(),
                span: self.peek_span(),
            });
        }
        let dec = self.parse_clause(&TokKind::Dec)?;
        self.loop_depth += 1;
        let body_result = self.parse_block();
        self.loop_depth -= 1;
        let body = body_result?;
        // The condition `e is Variant` (the SHIPPED `Expr::Is`).
        let cond = Expr::Is {
            scrutinee: Box::new(scrutinee),
            variant,
        };
        Ok(LoopNode {
            kind: LoopKind::While(Box::new(cond)),
            invs,
            dec,
            body,
            span: start.to(self.prev_span()),
        })
    }

    /// Parse `break;` / `continue;` (parser.md REQ-10, #93). `is_break` selects
    /// the keyword/variant. Enforces the in-loop structural rule: a
    /// break/continue at `loop_depth == 0` (outside any loop body) is a
    /// `BreakContinueOutsideLoop` diagnostic. Payload-less, value-less, with a
    /// mandatory trailing `;` (presence/cardinality, like every statement).
    fn parse_break_continue(&mut self, is_break: bool) -> PResult<Stmt> {
        let (tok, keyword) = if is_break {
            (TokKind::Break, "break")
        } else {
            (TokKind::Continue, "continue")
        };
        let span = self.peek_span();
        self.consume(&tok, if is_break { "`break`" } else { "`continue`" })?;
        if self.loop_depth == 0 {
            return Err(SyntaxError::BreakContinueOutsideLoop {
                keyword: keyword.to_string(),
                span,
            });
        }
        self.consume(&TokKind::Semi, "`;`")?;
        Ok(if is_break {
            Stmt::Break
        } else {
            Stmt::Continue
        })
    }

    fn parse_return(&mut self) -> PResult<Stmt> {
        self.consume(&TokKind::Return, "`return`")?;
        if self.eat(&TokKind::Semi) {
            return Ok(Stmt::Return(None));
        }
        let expr = self.parse_expr()?;
        self.consume(&TokKind::Semi, "`;`")?;
        Ok(Stmt::Return(Some(expr)))
    }

    /// Parse the shared shape `if EXPR Block ('else' Block)?`, returning its
    /// parts. The caller (`parse_block`) decides whether this is the statement
    /// form (`Stmt::If`) or — when it has an `else` and sits in tail position —
    /// the expression form (`Expr::If`), per surface-grammar.md decision 2.
    fn parse_if_parts(&mut self) -> PResult<(Expr, Block, Option<Block>)> {
        // Bound recursion: a tail-position `if x { <nest> } else { 0 }` re-enters
        // `parse_block` -> this fn -> `parse_block` ..., a cycle the #29
        // expr-only guard never saw (only the condition routes through
        // `parse_expr`). Guarding the cycle's re-entry point caps it so deep
        // tail-`if` nesting returns a diagnostic, never aborts (parser.md AC-4;
        // #31 — the construct the #30 fix introduced).
        self.guard_recursion(Self::parse_if_parts_inner)
    }

    fn parse_if_parts_inner(&mut self) -> PResult<(Expr, Block, Option<Block>)> {
        self.consume(&TokKind::If, "`if`")?;
        // The condition is a no-struct-literal head (REQ-2): `if c { … }` reads
        // `{` as the then-block, not a struct literal.
        let cond = self.with_no_struct_literal(Self::parse_expr)?;
        let then = self.parse_block()?;
        let else_ = if self.eat(&TokKind::Else) {
            Some(self.parse_block()?)
        } else {
            None
        };
        Ok((cond, then, else_))
    }

    /// Parse a `loop`/`while` with mandatory `inv`+ then exactly one `dec`
    /// (parser.md REQ-2; §4.1).
    fn parse_loop(&mut self) -> PResult<LoopNode> {
        // Bound recursion: a `loop`/`while` body is a `Block`
        // (surface-grammar.md REQ-3), and a `Block` may contain a nested
        // `loop`/`while` statement, so `parse_block` -> this fn -> `parse_block`
        // is a cycle that — like the if-tail cycle (#31) — never saw the #29
        // expr-only guard. Guarding this re-entry caps deep loop nesting to a
        // structured diagnostic instead of a native stack overflow (parser.md
        // AC-4; #32 — the last unguarded block-nesting vector).
        self.guard_recursion(Self::parse_loop_inner)
    }

    fn parse_loop_inner(&mut self) -> PResult<LoopNode> {
        let start = self.peek_span();
        let kind = if self.eat(&TokKind::Loop) {
            LoopKind::Loop
        } else {
            self.consume(&TokKind::While, "`loop` or `while`")?;
            // The condition is a no-struct-literal head (REQ-2).
            let cond = self.with_no_struct_literal(Self::parse_expr)?;
            LoopKind::While(Box::new(cond))
        };

        // `inv` — one or more.
        if !self.check(&TokKind::Inv) {
            return Err(SyntaxError::MissingClause {
                item: "loop".to_string(),
                clause: "inv".to_string(),
                span: self.peek_span(),
            });
        }
        let mut invs = Vec::new();
        while self.check(&TokKind::Inv) {
            invs.push(self.parse_clause(&TokKind::Inv)?);
        }

        // `dec` — exactly one.
        if !self.check(&TokKind::Dec) {
            return Err(SyntaxError::MissingClause {
                item: "loop".to_string(),
                clause: "dec".to_string(),
                span: self.peek_span(),
            });
        }
        let dec = self.parse_clause(&TokKind::Dec)?;
        if self.check(&TokKind::Dec) {
            // A second `dec` violates the exactly-one cardinality.
            return Err(SyntaxError::ClauseOrder {
                item: "loop".to_string(),
                clause: "dec".to_string(),
                span: self.peek_span(),
            });
        }

        // Enter the loop body at depth+1 so a `break;`/`continue;` anywhere
        // inside it (including nested `if` blocks — depth stays > 0) is accepted
        // (parser.md REQ-10, #93). A NESTED loop bumps the depth again; the
        // decrement is symmetric on every exit path (the `?` on `parse_block`
        // would skip a manual decrement, so guard around it).
        self.loop_depth += 1;
        let body_result = self.parse_block();
        self.loop_depth -= 1;
        let body = body_result?;
        Ok(LoopNode {
            kind,
            invs,
            dec,
            body,
            span: start.to(self.prev_span()),
        })
    }

    // ---- expressions (precedence ladder, surface-grammar.md) ---------------

    fn parse_expr(&mut self) -> PResult<Expr> {
        // Bound recursion depth so deeply nested input surfaces a structured
        // diagnostic instead of overflowing the native stack (parser.md AC-4).
        // Every nested expression re-enters the ladder through `parse_expr`
        // (parenthesised grouping, call args, closure bodies, match arms), so
        // this guard caps the whole precedence ladder via the shared counter.
        self.guard_recursion(Self::parse_or)
    }

    fn parse_or(&mut self) -> PResult<Expr> {
        let mut lhs = self.parse_and()?;
        while self.check(&TokKind::OrOr) {
            self.bump();
            let rhs = self.parse_and()?;
            lhs = Expr::Binary {
                op: BinOp::Or,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }
        Ok(lhs)
    }

    fn parse_and(&mut self) -> PResult<Expr> {
        let mut lhs = self.parse_cmp()?;
        while self.check(&TokKind::AndAnd) {
            self.bump();
            let rhs = self.parse_cmp()?;
            lhs = Expr::Binary {
                op: BinOp::And,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }
        Ok(lhs)
    }

    /// Comparison is NON-associative (surface-grammar.md): at most one CmpOp.
    /// Its operands are `is`-level (so `s is Circle` is a valid comparison
    /// operand, e.g. `result == (s is Circle)`).
    fn parse_cmp(&mut self) -> PResult<Expr> {
        let lhs = self.parse_is()?;
        let op = match self.peek() {
            TokKind::EqEq => Some(BinOp::Eq),
            TokKind::Ne => Some(BinOp::Ne),
            TokKind::Lt => Some(BinOp::Lt),
            TokKind::Le => Some(BinOp::Le),
            TokKind::Gt => Some(BinOp::Gt),
            TokKind::Ge => Some(BinOp::Ge),
            _ => None,
        };
        if let Some(op) = op {
            self.bump();
            let rhs = self.parse_is()?;
            Ok(Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            })
        } else {
            Ok(lhs)
        }
    }

    /// Parse the variant-discrimination operator `SCRUTINEE is Variant`
    /// (`.design/basis/01-adts.md` REQ-6): a `bool`-valued postfix operator
    /// producing `Expr::Is`. The variant is a (possibly `::`-segmented) path.
    /// Non-associative (a discrimination is not chained), sitting just below
    /// comparison so `s is Circle` reads as one operand. The VALIDATOR rule
    /// (accept only a declared variant of the scrutinee's enum) is stage 1b.
    fn parse_is(&mut self) -> PResult<Expr> {
        // OQ-3 (parser.md): `is` sits just below comparison and ABOVE the #92
        // bitwise/shift tiers, so `a & b is Variant` reads as `(a & b) is Variant`
        // (its scrutinee is a full bitwise-or expression). The ladder below `is`
        // is `parse_bitor`→`parse_bitxor`→`parse_bitand`→`parse_shift`→`parse_add`.
        let scrutinee = self.parse_bitor()?;
        if self.eat(&TokKind::Is) {
            let mut variant = vec![self.take_ident("a variant name after `is`")?];
            while self.eat(&TokKind::ColonCol) {
                variant.push(self.take_ident("a variant path segment")?);
            }
            Ok(Expr::Is {
                scrutinee: Box::new(scrutinee),
                variant,
            })
        } else {
            Ok(scrutinee)
        }
    }

    /// Tier 6 `|` — bitwise or (#92, `surface-grammar.md` REQ-10). A binary `|`
    /// joins two operands here; a `|` that OPENS a closure is recognized only in
    /// `parse_primary` (`Closure`), so the two `|` roles are disambiguated by
    /// position (parser.md REQ-8 / AC-6): an operator `|` is seen at the start of
    /// an iteration of this loop (after a left operand), never at expression head.
    fn parse_bitor(&mut self) -> PResult<Expr> {
        let mut lhs = self.parse_bitxor()?;
        while self.check(&TokKind::Pipe) {
            self.bump();
            let rhs = self.parse_bitxor()?;
            lhs = Expr::Binary {
                op: BinOp::BitOr,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }
        Ok(lhs)
    }

    /// Tier 5 `^` — bitwise xor (#92).
    fn parse_bitxor(&mut self) -> PResult<Expr> {
        let mut lhs = self.parse_bitand()?;
        while self.check(&TokKind::Caret) {
            self.bump();
            let rhs = self.parse_bitand()?;
            lhs = Expr::Binary {
                op: BinOp::BitXor,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }
        Ok(lhs)
    }

    /// Tier 4 `&` — bitwise and (#92). The binary `&` joins two operands here; the
    /// PREFIX reference `&`/`&mut` is parsed in `parse_ref` (one operand) —
    /// disambiguated by position (parser.md REQ-8 / AC-6): a prefix `&` is seen at
    /// expression head, a binary `&` after a left operand at this loop's start.
    fn parse_bitand(&mut self) -> PResult<Expr> {
        let mut lhs = self.parse_shift()?;
        while self.check(&TokKind::Amp) {
            self.bump();
            let rhs = self.parse_shift()?;
            lhs = Expr::Binary {
                op: BinOp::BitAnd,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }
        Ok(lhs)
    }

    /// Tier 3 `<<` `>>` — shifts (#92), below `+ -`. PARTIAL: an unbounded shift
    /// amount fails the §7 shift-bound obligation at L3 (ast.md REQ-11), but the
    /// PARSER builds the `Binary` node unconditionally (parser.md REQ-9).
    fn parse_shift(&mut self) -> PResult<Expr> {
        let mut lhs = self.parse_add()?;
        loop {
            let op = match self.peek() {
                TokKind::Shl => BinOp::Shl,
                TokKind::Shr => BinOp::Shr,
                _ => break,
            };
            self.bump();
            let rhs = self.parse_add()?;
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }
        Ok(lhs)
    }

    fn parse_add(&mut self) -> PResult<Expr> {
        let mut lhs = self.parse_mul()?;
        loop {
            let op = match self.peek() {
                TokKind::Plus => BinOp::Add,
                TokKind::Minus => BinOp::Sub,
                _ => break,
            };
            self.bump();
            let rhs = self.parse_mul()?;
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }
        Ok(lhs)
    }

    fn parse_mul(&mut self) -> PResult<Expr> {
        let mut lhs = self.parse_cast()?;
        loop {
            let op = match self.peek() {
                TokKind::Star => BinOp::Mul,
                TokKind::Slash => BinOp::Div,
                // `%` folds into the `MulExpr` tier alongside `*`/`/` (#92,
                // tier 1). PARTIAL: a zero divisor fails the §7 obligation at L3
                // (ast.md REQ-11); the parser builds the node unconditionally.
                TokKind::Percent => BinOp::Rem,
                _ => break,
            };
            self.bump();
            let rhs = self.parse_cast()?;
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }
        Ok(lhs)
    }

    fn parse_cast(&mut self) -> PResult<Expr> {
        let mut expr = self.parse_unary()?;
        while self.eat(&TokKind::As) {
            let ty = self.parse_type()?;
            expr = Expr::Cast {
                expr: Box::new(expr),
                ty,
            };
        }
        Ok(expr)
    }

    /// The prefix `!` tier (#92, `surface-grammar.md` REQ-10 `UnaryExpr`): prefix
    /// `!` binds tighter than every binary operator (so `!a & b` is `(!a) & b`)
    /// and sits between `parse_cast` and `parse_ref`. A standalone `!` is
    /// unambiguously the unary operator — `!=` is the distinct maximal-munch
    /// `TokKind::Ne` token (parser.md REQ-8). The ONE `UnaryOp::Not` is built
    /// regardless of operand type; its bitwise-vs-logical meaning is resolved
    /// downstream by Verus's type-directed `!` (§2.3, ast.md OQ-4). `!` is
    /// right-recursive (`!!a` is `!(!a)`).
    fn parse_unary(&mut self) -> PResult<Expr> {
        if self.eat(&TokKind::Bang) {
            let expr = self.parse_unary()?;
            Ok(Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(expr),
            })
        } else {
            self.parse_ref()
        }
    }

    fn parse_ref(&mut self) -> PResult<Expr> {
        if self.eat(&TokKind::Amp) {
            let mutable = self.eat(&TokKind::Mut);
            let expr = self.parse_ref()?;
            Ok(Expr::Ref {
                mutable,
                expr: Box::new(expr),
            })
        } else if self.eat(&TokKind::Star) {
            // Prefix dereference `*EXPR` (`.design/basis/01-adts.md` REQ-3): the
            // recursive call `sum_list(*t)` derefs the boxed tail. A new
            // `Expr::Deref` unary — no existing node fits (`Ref` is its inverse).
            // SEMANTICS are stage 1c; surface-only here.
            let expr = self.parse_ref()?;
            Ok(Expr::Deref(Box::new(expr)))
        } else {
            self.parse_postfix()
        }
    }

    fn parse_postfix(&mut self) -> PResult<Expr> {
        let mut expr = self.parse_primary()?;
        loop {
            match self.peek() {
                TokKind::Dot => {
                    self.bump();
                    // A numeric projection `e.0`/`e.1`/…
                    // (`.design/basis/10-recursion-tuples.md` REQ-5/REQ-8, OQ-1
                    // RESOLVED → a dedicated `Expr::TupleProj { receiver, index }`,
                    // NOT an overloaded `Expr::Field` with a string `"0"` name: a
                    // tuple index is a `usize`, and a dedicated node keeps the
                    // projection lowering (`<recv>.<index>`) distinct from a
                    // struct/method `.field`). A tuple index lexes as a
                    // `TokKind::Int` after the `.` (e.g. `r.0` is `r` `.` `Int{0}`).
                    if let TokKind::Int { value, .. } = self.peek().clone() {
                        self.bump();
                        let index = usize::try_from(value).map_err(|_| {
                            self.unexpected("a tuple projection index within `usize`")
                        })?;
                        expr = Expr::TupleProj {
                            receiver: Box::new(expr),
                            index,
                        };
                        continue;
                    }
                    let name = self.take_ident("a field or method name")?;
                    if self.check(&TokKind::LParen) {
                        let args = self.parse_call_args()?;
                        expr = Expr::MethodCall {
                            receiver: Box::new(expr),
                            name,
                            args,
                        };
                    } else {
                        expr = Expr::Field {
                            receiver: Box::new(expr),
                            name,
                        };
                    }
                }
                TokKind::LBracket => {
                    self.bump();
                    let index = self.parse_index_arg()?;
                    self.consume(&TokKind::RBracket, "`]`")?;
                    expr = Expr::Index {
                        base: Box::new(expr),
                        index,
                    };
                }
                TokKind::LParen => {
                    let args = self.parse_call_args()?;
                    expr = Expr::Call {
                        callee: Box::new(expr),
                        args,
                    };
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_call_args(&mut self) -> PResult<Vec<Expr>> {
        self.consume(&TokKind::LParen, "`(`")?;
        let mut args = Vec::new();
        if !self.check(&TokKind::RParen) {
            // Inside the `( … )` a struct literal is unambiguous again (REQ-2).
            self.with_struct_literal(|p| {
                loop {
                    args.push(p.parse_expr()?);
                    if !p.eat(&TokKind::Comma) {
                        break;
                    }
                    if p.check(&TokKind::RParen) {
                        break;
                    }
                }
                Ok(())
            })?;
        }
        self.consume(&TokKind::RParen, "`)`")?;
        Ok(args)
    }

    /// Parse an index argument: `i`, `..i`, `i..`, `i..j` (surface-grammar.md).
    /// Inside the `[ … ]` a struct literal is unambiguous again (REQ-2).
    fn parse_index_arg(&mut self) -> PResult<IndexArg> {
        self.with_struct_literal(Self::parse_index_arg_inner)
    }

    fn parse_index_arg_inner(&mut self) -> PResult<IndexArg> {
        if self.eat(&TokKind::DotDot) {
            // `..i`
            let hi = self.parse_expr()?;
            return Ok(IndexArg::RangeTo(Box::new(hi)));
        }
        let lo = self.parse_expr()?;
        if self.eat(&TokKind::DotDot) {
            if self.check(&TokKind::RBracket) {
                Ok(IndexArg::RangeFrom(Box::new(lo)))
            } else {
                let hi = self.parse_expr()?;
                Ok(IndexArg::Range(Box::new(lo), Box::new(hi)))
            }
        } else {
            Ok(IndexArg::Single(Box::new(lo)))
        }
    }

    fn parse_primary(&mut self) -> PResult<Expr> {
        match self.peek().clone() {
            TokKind::Int { value, raw } => {
                self.bump();
                Ok(Expr::IntLit { value, raw })
            }
            TokKind::Bool(b) => {
                self.bump();
                Ok(Expr::BoolLit(b))
            }
            // A string literal `"hello"` as a primary expression
            // (`.design/basis/07-strings.md` REQ-1). The literal LEXES today
            // (`TokKind::Str(String)`); this arm accepts it as an `Expr::StrLit`,
            // mirroring the `IntLit`/`BoolLit` value-carrying literal precedent.
            // The token's EXISTING `parse_slag`/`parse_attribute` consumers (the
            // `#[slag(reason = "…")]` / `#[boundary("…")]` field values) are
            // UNCHANGED — those read the token directly via `take_string`, never
            // through `parse_primary`, so a field value is still a token-level
            // string, not an `Expr` (REQ-1; no regression to sealed/boundary parse).
            TokKind::Str(s) => {
                self.bump();
                Ok(Expr::StrLit(s))
            }
            TokKind::Ident(_) => self.parse_path_expr(),
            TokKind::Pipe | TokKind::OrOr => self.parse_closure(),
            TokKind::Match => self.parse_match(),
            TokKind::If => self.parse_if_expr(),
            TokKind::LParen => {
                self.bump();
                // A parenthesised group re-enables struct literals (REQ-2):
                // `(s is Circle)` / `(A { x: 1 })`. The SAME `(` opens an n-tuple
                // construction `(a, b, …)`
                // (`.design/basis/10-recursion-tuples.md` REQ-5/REQ-7): the parser
                // distinguishes by the comma — `()` → unit (the empty group; no
                // tuple expr, mirroring `Type::Unit`), `(e)` → grouping (the inner
                // expr, arity 1), `(a, b, …)` → `Expr::Tuple` (arity ≥ 2).
                self.with_struct_literal(|p| {
                    if p.check(&TokKind::RParen) {
                        // Arity 0: the empty group `()` — the unit value. There is
                        // no `Expr::Unit` node; v1 surfaces unit only as a return
                        // TYPE (`Type::Unit`), so a literal `()` value is not a
                        // grammar form. Reject it explicitly rather than silently.
                        return Err(
                            p.unexpected("an expression (an empty `()` is not a value form)")
                        );
                    }
                    let first = p.parse_expr()?;
                    if !p.check(&TokKind::Comma) {
                        // Arity 1: `(e)` is a parenthesised grouping — the inner
                        // expression.
                        p.consume(&TokKind::RParen, "`)`")?;
                        return Ok(first);
                    }
                    // Arity ≥ 2: an n-tuple construction `(a, b, …)`.
                    let mut elems = vec![first];
                    while p.eat(&TokKind::Comma) {
                        if p.check(&TokKind::RParen) {
                            // A trailing comma `(a, b,)` — stop collecting.
                            break;
                        }
                        elems.push(p.parse_expr()?);
                    }
                    p.consume(&TokKind::RParen, "`)` to close the tuple `(a, b, …)`")?;
                    Ok(Expr::Tuple(elems))
                })
            }
            _ => Err(self.unexpected("an expression")),
        }
    }

    /// Parse a path expression `Ident (:: Ident)*` (`lo`, `u32::MAX`, `Some`),
    /// or a struct-literal `Path { field: val, … }` when a `{` follows and the
    /// struct-literal context is enabled (`.design/basis/01-adts.md` REQ-2).
    /// `::` is a PATH separator, never method dispatch (REQ-6).
    fn parse_path_expr(&mut self) -> PResult<Expr> {
        let mut segments = vec![self.take_ident("a path")?];
        while self.eat(&TokKind::ColonCol) {
            segments.push(self.take_ident("a path segment")?);
        }
        // A `Path { … }` is a struct / struct-variant construction (REQ-2),
        // EXCEPT in a no-struct-literal head (`match s { … }`), where the `{`
        // opens the arm/then/loop block, not a struct literal.
        if !self.no_struct_literal && self.check(&TokKind::LBrace) {
            return self.parse_struct_lit(segments);
        }
        Ok(Expr::Path(segments))
    }

    /// Parse the `{ field: val, … }` tail of a struct / struct-variant
    /// construction `Path { … }` (`.design/basis/01-adts.md` REQ-2), building an
    /// `Expr::StructLit`. The field initializers re-enable struct literals
    /// (a nested `A { b: B { … } }`); a trailing comma is permitted.
    fn parse_struct_lit(&mut self, path: Vec<Ident>) -> PResult<Expr> {
        self.consume(&TokKind::LBrace, "`{`")?;
        let mut fields = Vec::new();
        if !self.check(&TokKind::RBrace) {
            self.with_struct_literal(|p| {
                loop {
                    let name = p.take_ident("a field name")?;
                    p.consume(&TokKind::Colon, "`:`")?;
                    let value = p.parse_expr()?;
                    fields.push((name, value));
                    if !p.eat(&TokKind::Comma) {
                        break;
                    }
                    if p.check(&TokKind::RBrace) {
                        break;
                    }
                }
                Ok(())
            })?;
        }
        self.consume(&TokKind::RBrace, "`}`")?;
        Ok(Expr::StructLit { path, fields })
    }

    fn parse_closure(&mut self) -> PResult<Expr> {
        let mut params = Vec::new();
        if self.eat(&TokKind::OrOr) {
            // `||` is an empty parameter list.
        } else {
            self.consume(&TokKind::Pipe, "`|`")?;
            if !self.check(&TokKind::Pipe) {
                loop {
                    params.push(self.take_ident("a closure parameter")?);
                    if !self.eat(&TokKind::Comma) {
                        break;
                    }
                }
            }
            self.consume(&TokKind::Pipe, "`|`")?;
        }
        let body = self.parse_expr()?;
        Ok(Expr::Closure {
            params,
            body: Box::new(body),
        })
    }

    fn parse_match(&mut self) -> PResult<Expr> {
        self.consume(&TokKind::Match, "`match`")?;
        // The scrutinee is a no-struct-literal head: `match s { … }` reads the
        // `{` as the arm block, not `s { … }` as a struct literal (REQ-2).
        let scrutinee = self.with_no_struct_literal(Self::parse_expr)?;
        self.consume(&TokKind::LBrace, "`{`")?;
        let mut arms = Vec::new();
        while !self.check(&TokKind::RBrace) && !self.at_eof() {
            let pattern = self.parse_pattern()?;
            // An optional match guard `pat if <cond> =>`
            // (`.design/basis/11-ergonomics.md` REQ-3): a `bool`-valued condition
            // evaluated in the arm's binding scope. The guard is a no-struct-literal
            // head (the `=>` follows; a trailing `Name { … }` would be ambiguous),
            // mirroring the `if`/`while`/`match`-head rule. A guarded arm does NOT
            // complete a match (the validator's exhaustiveness check, REQ-3).
            let guard = if self.eat(&TokKind::If) {
                Some(self.with_no_struct_literal(Self::parse_expr)?)
            } else {
                None
            };
            self.consume(&TokKind::FatArrow, "`=>`")?;
            // An arm body is in VALUE position, so a struct-literal construction
            // (`Point { x: 1 }`) MUST parse here even when the `match` sits under
            // an enclosing no-struct-literal head (a contract clause / `match`
            // scrutinee). Re-enable struct literals exactly as `parse_call_args`
            // does inside `( … )` (REQ-2/REQ-4); the scrutinee above stays under
            // the no-struct-literal context, and `with_struct_literal` restores
            // the prior context on exit so no leak escapes the body.
            let body = self.with_struct_literal(Self::parse_expr)?;
            arms.push(MatchArm {
                pattern,
                guard,
                body,
            });
            if !self.eat(&TokKind::Comma) {
                break;
            }
        }
        self.consume(&TokKind::RBrace, "`}`")?;
        Ok(Expr::Match {
            scrutinee: Box::new(scrutinee),
            arms,
        })
    }

    /// The expression form of `if` requires an `else` (it must have a value).
    fn parse_if_expr(&mut self) -> PResult<Expr> {
        self.consume(&TokKind::If, "`if`")?;
        let cond = self.with_no_struct_literal(Self::parse_expr)?;
        let then = self.parse_block()?;
        self.consume(&TokKind::Else, "`else` (an `if` expression must have one)")?;
        let else_ = self.parse_block()?;
        Ok(Expr::If {
            cond: Box::new(cond),
            then,
            else_,
        })
    }

    // ---- patterns ----------------------------------------------------------

    fn parse_pattern(&mut self) -> PResult<Pattern> {
        // Bound recursion: slice patterns (`[[...]]` via `parse_slice_pattern`)
        // and enum/tuple-struct patterns (`Some(Some(...))` via
        // `parse_path_pattern`) both re-enter `parse_pattern`, so a single guard
        // here caps both cycles (parser.md AC-4; #31 — the #29 expr-only guard
        // never saw the pattern path).
        //
        // An or-pattern `p0 | p1 | …` (`.design/basis/11-ergonomics.md` REQ-4):
        // parse one alternative, then while a `|` follows collect more, building a
        // flat `Pattern::Or` (a single alternative stays the bare pattern — no
        // spurious `Or` wrapper, byte-stable for the pre-C10 corpus). The `|`
        // here is unambiguously the pattern alternator: a pattern position never
        // starts a bitwise/closure `|` (those are expression-tier).
        let first = self.guard_recursion(Self::parse_pattern_inner)?;
        if !self.check(&TokKind::Pipe) {
            return Ok(first);
        }
        let mut alts = vec![first];
        while self.eat(&TokKind::Pipe) {
            alts.push(self.guard_recursion(Self::parse_pattern_inner)?);
        }
        Ok(Pattern::Or(alts))
    }

    fn parse_pattern_inner(&mut self) -> PResult<Pattern> {
        match self.peek().clone() {
            TokKind::Ident(name) if name == "_" => {
                self.bump();
                Ok(Pattern::Wildcard)
            }
            TokKind::Int { value, raw } => {
                self.bump();
                Ok(Pattern::Literal(Expr::IntLit { value, raw }))
            }
            TokKind::Bool(b) => {
                self.bump();
                Ok(Pattern::Literal(Expr::BoolLit(b)))
            }
            TokKind::LBracket => self.parse_slice_pattern(),
            TokKind::Ident(_) => self.parse_path_pattern(),
            _ => Err(self.unexpected("a pattern")),
        }
    }

    fn parse_slice_pattern(&mut self) -> PResult<Pattern> {
        self.consume(&TokKind::LBracket, "`[`")?;
        let mut elems = Vec::new();
        if !self.check(&TokKind::RBracket) {
            loop {
                if self.eat(&TokKind::DotDot) {
                    let name = self.take_ident("a rest binding name")?;
                    elems.push(SlicePat::Rest(name));
                } else {
                    elems.push(SlicePat::Pat(self.parse_pattern()?));
                }
                if !self.eat(&TokKind::Comma) {
                    break;
                }
                if self.check(&TokKind::RBracket) {
                    break;
                }
            }
        }
        self.consume(&TokKind::RBracket, "`]`")?;
        Ok(Pattern::Slice(elems))
    }

    /// A path pattern: a bare binding (`i`, `head`) or an enum/tuple-struct
    /// pattern (`Some(i)`, `None`).
    fn parse_path_pattern(&mut self) -> PResult<Pattern> {
        let mut path = vec![self.take_ident("a pattern path")?];
        while self.eat(&TokKind::ColonCol) {
            path.push(self.take_ident("a path segment")?);
        }
        if self.check(&TokKind::LParen) {
            self.bump();
            let mut fields = Vec::new();
            if !self.check(&TokKind::RParen) {
                loop {
                    fields.push(self.parse_pattern()?);
                    if !self.eat(&TokKind::Comma) {
                        break;
                    }
                }
            }
            self.consume(&TokKind::RParen, "`)`")?;
            Ok(Pattern::Enum { path, fields })
        } else if self.check(&TokKind::LBrace) {
            // A struct / struct-variant destructuring pattern `Path { field: pat,
            // … }` or `Path { .. }` (`.design/basis/01-adts.md` REQ-4). A bare
            // field name `Rect { w, h }` is shorthand for `w: w` (a binding).
            self.parse_struct_pattern(path)
        } else if path.len() == 1 {
            // A single lowercase name is a binding; an uppercase-initial single
            // segment (`None`) is a zero-field enum pattern.
            let name = &path[0];
            if name.chars().next().is_some_and(|c| c.is_uppercase()) {
                Ok(Pattern::Enum {
                    path,
                    fields: Vec::new(),
                })
            } else {
                Ok(Pattern::Binding(name.clone()))
            }
        } else {
            Ok(Pattern::Enum {
                path,
                fields: Vec::new(),
            })
        }
    }

    /// Parse a struct / struct-variant destructuring pattern `Path { field:
    /// pat, … }` / `Path { .. }` (`.design/basis/01-adts.md` REQ-4). Each field
    /// is `name: pat` or the shorthand `name` (expanded to `name:
    /// Pattern::Binding(name)`). A leading/trailing `..` sets `rest`. Building
    /// the binding shorthand keeps `match`-arm binding ergonomic (`Rect { w, h }`
    /// binds `w` and `h`).
    fn parse_struct_pattern(&mut self, path: Vec<Ident>) -> PResult<Pattern> {
        self.consume(&TokKind::LBrace, "`{`")?;
        let mut fields = Vec::new();
        let mut rest = false;
        if !self.check(&TokKind::RBrace) {
            loop {
                if self.eat(&TokKind::DotDot) {
                    rest = true;
                    break;
                }
                let name = self.take_ident("a field name")?;
                let pat = if self.eat(&TokKind::Colon) {
                    self.parse_pattern()?
                } else {
                    // Field shorthand `Rect { w, h }`: bind the field to its name.
                    Pattern::Binding(name.clone())
                };
                fields.push((name, pat));
                if !self.eat(&TokKind::Comma) {
                    break;
                }
                if self.check(&TokKind::RBrace) {
                    break;
                }
            }
        }
        self.consume(&TokKind::RBrace, "`}`")?;
        Ok(Pattern::Struct { path, fields, rest })
    }

    // ---- types -------------------------------------------------------------

    fn parse_type(&mut self) -> PResult<Type> {
        // Bound recursion: `Name<T>` and `&[T]`/`&T` re-enter `parse_type`, so a
        // deeply nested `Option<Option<...>>` would overflow the native stack
        // without this guard (parser.md AC-4; #31 — the #29 expr-only guard
        // never saw the type path).
        self.guard_recursion(Self::parse_type_inner)
    }

    fn parse_type_inner(&mut self) -> PResult<Type> {
        match self.peek().clone() {
            // `()` is the ONE sanctioned unit-type spelling (surface-grammar.md
            // decision 4 / REQ-8): written explicitly in a return position. The
            // SAME `(` opens an n-tuple type `(T, U, …)`
            // (`.design/basis/10-recursion-tuples.md` REQ-5/REQ-7): the parser
            // distinguishes by the comma — `()` → `Type::Unit` (arity 0), `(T)`
            // → grouping (the inner type, arity 1), `(T, U, …)` → `Type::Tuple`
            // (arity ≥ 2).
            TokKind::LParen => {
                self.bump();
                if self.check(&TokKind::RParen) {
                    // Arity 0: `()` is the unit type (UNCHANGED).
                    self.bump();
                    return Ok(Type::Unit);
                }
                let first = self.parse_type()?;
                if !self.check(&TokKind::Comma) {
                    // Arity 1: `(T)` is a parenthesised grouping — the inner type.
                    self.consume(
                        &TokKind::RParen,
                        "`)` to close the parenthesised type `(T)`",
                    )?;
                    return Ok(first);
                }
                // Arity ≥ 2: an n-tuple type `(T, U, …)`.
                let mut elems = vec![first];
                while self.eat(&TokKind::Comma) {
                    if self.check(&TokKind::RParen) {
                        // A trailing comma `(T, U,)` — stop collecting.
                        break;
                    }
                    elems.push(self.parse_type()?);
                }
                self.consume(&TokKind::RParen, "`)` to close the tuple type `(T, U, …)`")?;
                Ok(Type::Tuple(elems))
            }
            TokKind::Amp => {
                self.bump();
                let mutable = self.eat(&TokKind::Mut);
                if self.check(&TokKind::LBracket) {
                    self.bump();
                    let inner = self.parse_type()?;
                    self.consume(&TokKind::RBracket, "`]`")?;
                    Ok(Type::Ref {
                        mutable,
                        inner: Box::new(Type::Slice(Box::new(inner))),
                    })
                } else {
                    let inner = self.parse_type()?;
                    Ok(Type::Ref {
                        mutable,
                        inner: Box::new(inner),
                    })
                }
            }
            TokKind::Ident(name) => {
                self.bump();
                match name.as_str() {
                    "u32" => Ok(Type::Prim(PrimType::U32)),
                    "u64" => Ok(Type::Prim(PrimType::U64)),
                    "usize" => Ok(Type::Prim(PrimType::Usize)),
                    "bool" => Ok(Type::Prim(PrimType::Bool)),
                    // The heap-indirection primitive `Box<T>`
                    // (`.design/basis/01-adts.md` REQ-3, OQ-1 RESOLVED: a
                    // dedicated `Type::Box` node). `Box` is a contextual
                    // identifier (NOT a reserved keyword), matched here by name.
                    "Box" => {
                        self.consume(&TokKind::Lt, "`<` after `Box`")?;
                        let inner = self.parse_type()?;
                        self.consume(&TokKind::Gt, "`>` to close `Box<…>`")?;
                        Ok(Type::Box(Box::new(inner)))
                    }
                    // The bounded growable-collection primitive `Vec<T>`
                    // (`.design/basis/04-collections.md` REQ-1, OQ-2 RESOLVED: a
                    // dedicated `Type::Vec` node, mirroring `Box<T>`). `Vec` is a
                    // contextual identifier (NOT a reserved keyword), matched here
                    // by name exactly as `Box` is. The element type `T` parses
                    // recursively; `Vec<u64>` (`conformance/vec_demo.th`) yields
                    // `Type::Vec(Box::new(Type::Prim(U64)))`. Its `push`/`pop`/
                    // `get`/`len` operations are ordinary `MethodCall`s parsed by
                    // the existing postfix `.` form (REQ-6) — no new surface here.
                    "Vec" => {
                        self.consume(&TokKind::Lt, "`<` after `Vec`")?;
                        let inner = self.parse_type()?;
                        self.consume(&TokKind::Gt, "`>` to close `Vec<…>`")?;
                        Ok(Type::Vec(Box::new(inner)))
                    }
                    // The bounded owned-text primitive `String`
                    // (`.design/basis/07-strings.md` REQ-2, OQ-3 RESOLVED: a
                    // dedicated NULLARY `Type::String` node — no `<T>` argument,
                    // unlike `Vec<T>`, because the element type is FIXED to `u8`
                    // (the char model is bytes for v1). `String` is a contextual
                    // identifier (NOT a reserved keyword), matched here by name
                    // exactly as `Box`/`Vec` are. The borrowed `str`-view is
                    // `&String` (`Ref { inner: String }`), parsed by the `&` arm
                    // above. `String`'s `len`/`byte_at`/`slice`/`concat` ops are
                    // ordinary `MethodCall`s (the existing postfix `.` form) — no
                    // new surface; `==`/`+` are the existing `Binary` ops.
                    "String" => Ok(Type::String),
                    // The built-in optional primitive `Option<T>`
                    // (`.design/basis/09-option-result.md` REQ-1, OQ-1 RESOLVED: a
                    // dedicated `Type::Option` node, mirroring `Box<T>`/`Vec<T>`).
                    // `Option` STOPS being a string-named `Generic` so the
                    // lowerer/validator key on the node kind. `Option` is a
                    // contextual ident (NOT a reserved keyword), matched here by
                    // name exactly as `Box`/`Vec` are. `Some(v)`/`None`/`match`/`is`
                    // reuse the existing `Call`/`Path`/`Match`/`Is` nodes.
                    "Option" => {
                        self.consume(&TokKind::Lt, "`<` after `Option`")?;
                        let inner = self.parse_type()?;
                        self.consume(&TokKind::Gt, "`>` to close `Option<…>`")?;
                        Ok(Type::Option(Box::new(inner)))
                    }
                    // The built-in fallible primitive `Result<T, E>`
                    // (`.design/basis/09-option-result.md` REQ-2, OQ-1 RESOLVED: a
                    // dedicated TWO-type-argument node — the FIRST two-arg type in
                    // the grammar, the load-bearing parser change of C7). The
                    // single-arg `Generic { name, arg }` dies at the comma; this arm
                    // parses `<T, E>` (a comma + a second type + `>`). `Result` is a
                    // contextual ident matched by name exactly as `Box`/`Vec`/
                    // `Option`. `Ok(v)`/`Err(e)`/`match`/`is` reuse the existing
                    // `Call`/`Match`/`Is` nodes.
                    "Result" => {
                        self.consume(&TokKind::Lt, "`<` after `Result`")?;
                        let ok_ty = self.parse_type()?;
                        self.consume(&TokKind::Comma, "`,` between `Result<T, E>` args")?;
                        let err_ty = self.parse_type()?;
                        self.consume(&TokKind::Gt, "`>` to close `Result<…, …>`")?;
                        Ok(Type::Result(Box::new(ok_ty), Box::new(err_ty)))
                    }
                    // The bounded verified key-value primitive `Map<K, V>`
                    // (`.design/basis/13-map.md` REQ-1, C12: the SECOND
                    // two-type-argument node, mirroring `Result<T, E>` VERBATIM —
                    // the single-arg `Generic { name, arg }` cannot carry a key AND
                    // a value (it dies at the comma, the exact C7 finding). `Map` is
                    // a contextual ident matched by name exactly as `Box`/`Vec`/
                    // `Option`/`Result`. The key `K` and value `V` parse recursively;
                    // `Map<u64, u64>` yields `Type::Map(Box::new(u64), Box::new(u64))`.
                    // Its `insert`/`get`/`contains_key`/`len` ops are ordinary
                    // `MethodCall`s (the existing postfix `.` form) — no new surface.
                    "Map" => {
                        self.consume(&TokKind::Lt, "`<` after `Map`")?;
                        let key_ty = self.parse_type()?;
                        self.consume(&TokKind::Comma, "`,` between `Map<K, V>` args")?;
                        let val_ty = self.parse_type()?;
                        self.consume(&TokKind::Gt, "`>` to close `Map<…, …>`")?;
                        Ok(Type::Map(Box::new(key_ty), Box::new(val_ty)))
                    }
                    _ => {
                        // A generic application `NAME<T>` (e.g. `Option<usize>`),
                        // or a bare user-defined type name `Account`/`Shape`/
                        // `List` (`.design/basis/01-adts.md` REQ-1/REQ-2 ->
                        // `Type::Named`). A bare lowercase/uppercase ident with no
                        // `<` is a named type (the type-side of a `struct`/`enum`
                        // declaration) rather than a parse error.
                        if self.eat(&TokKind::Lt) {
                            let arg = self.parse_type()?;
                            self.consume(&TokKind::Gt, "`>`")?;
                            Ok(Type::Generic {
                                name,
                                arg: Box::new(arg),
                            })
                        } else {
                            Ok(Type::Named(name))
                        }
                    }
                }
            }
            _ => Err(self.unexpected("a type")),
        }
    }

    // ---- small helpers -----------------------------------------------------

    fn take_ident(&mut self, what: &str) -> PResult<Ident> {
        match self.peek().clone() {
            TokKind::Ident(name) => {
                self.bump();
                Ok(name)
            }
            _ => Err(self.unexpected(what)),
        }
    }

    fn take_string(&mut self, what: &str) -> PResult<String> {
        match self.peek().clone() {
            TokKind::Str(s) => {
                self.bump();
                Ok(s)
            }
            _ => Err(self.unexpected(what)),
        }
    }

    /// The span of the token most recently consumed (for end-of-node spans).
    fn prev_span(&self) -> Span {
        if self.pos == 0 {
            self.tokens[0].span
        } else {
            self.tokens[self.pos - 1].span
        }
    }
}

/// A short human description of a token kind, for diagnostics.
fn describe(kind: &TokKind) -> String {
    match kind {
        TokKind::Ident(s) => format!("identifier `{s}`"),
        TokKind::Int { value, .. } => format!("integer `{value}`"),
        TokKind::Bool(b) => format!("`{b}`"),
        TokKind::Str(s) => format!("string {s:?}"),
        TokKind::Eof => "end of input".to_string(),
        other => format!("`{}`", token_text(other)),
    }
}

/// The canonical surface spelling of a fixed-text token kind.
fn token_text(kind: &TokKind) -> &'static str {
    match kind {
        TokKind::Fn => "fn",
        TokKind::Spec => "spec",
        TokKind::Req => "req",
        TokKind::Ens => "ens",
        TokKind::Fx => "fx",
        TokKind::Inv => "inv",
        TokKind::Dec => "dec",
        TokKind::Pure => "pure",
        TokKind::Let => "let",
        TokKind::Mut => "mut",
        TokKind::Return => "return",
        TokKind::Break => "break",
        TokKind::Continue => "continue",
        TokKind::If => "if",
        TokKind::Else => "else",
        TokKind::Loop => "loop",
        TokKind::While => "while",
        TokKind::Match => "match",
        TokKind::As => "as",
        TokKind::Struct => "struct",
        TokKind::Enum => "enum",
        TokKind::Is => "is",
        TokKind::HashBracket => "#[",
        TokKind::Arrow => "->",
        TokKind::FatArrow => "=>",
        TokKind::EqEq => "==",
        TokKind::Ne => "!=",
        TokKind::Le => "<=",
        TokKind::Ge => ">=",
        TokKind::AndAnd => "&&",
        TokKind::OrOr => "||",
        TokKind::ColonCol => "::",
        TokKind::DotDot => "..",
        TokKind::Shl => "<<",
        TokKind::Shr => ">>",
        TokKind::LBrace => "{",
        TokKind::RBrace => "}",
        TokKind::LParen => "(",
        TokKind::RParen => ")",
        TokKind::LBracket => "[",
        TokKind::RBracket => "]",
        TokKind::Comma => ",",
        TokKind::Semi => ";",
        TokKind::Colon => ":",
        TokKind::Dot => ".",
        TokKind::Eq => "=",
        TokKind::Lt => "<",
        TokKind::Gt => ">",
        TokKind::Plus => "+",
        TokKind::Minus => "-",
        TokKind::Star => "*",
        TokKind::Slash => "/",
        TokKind::Percent => "%",
        TokKind::Caret => "^",
        TokKind::Amp => "&",
        TokKind::Pipe => "|",
        TokKind::Bang => "!",
        TokKind::Ident(_)
        | TokKind::Int { .. }
        | TokKind::Bool(_)
        | TokKind::Str(_)
        | TokKind::Eof => "<token>",
    }
}
