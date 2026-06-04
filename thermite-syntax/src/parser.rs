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
            | SyntaxError::ExpressionTooDeep { span, .. } => *span,
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
        }
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
                TokKind::Fn | TokKind::Spec | TokKind::HashBracket
            ) {
                break;
            }
            self.bump();
        }
    }

    // ---- items -------------------------------------------------------------

    fn parse_item(&mut self) -> PResult<Item> {
        let start_span = self.peek_span();
        let slag = if self.check(&TokKind::HashBracket) {
            Some(self.parse_slag()?)
        } else {
            None
        };

        if self.check(&TokKind::Spec) {
            if slag.is_some() {
                // `#[slag]` only attaches to `fn` (surface-grammar Item).
                return Err(self.unexpected("`fn` after `#[slag(...)]`"));
            }
            self.parse_spec_fn(start_span)
        } else if self.check(&TokKind::Fn) {
            self.parse_fn(slag, start_span)
        } else {
            Err(self.unexpected("`fn`, `spec fn`, or `#[slag(...)]`"))
        }
    }

    fn parse_slag(&mut self) -> PResult<SlagAttr> {
        let start = self.peek_span();
        self.consume(&TokKind::HashBracket, "`#[`")?;
        let name = self.take_ident("`slag`")?;
        if name != "slag" {
            return Err(SyntaxError::Unexpected {
                expected: "`slag`".to_string(),
                found: format!("identifier `{name}`"),
                span: start,
            });
        }
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

    fn parse_fn(&mut self, slag: Option<SlagAttr>, start_span: Span) -> PResult<Item> {
        self.consume(&TokKind::Fn, "`fn`")?;
        let name = self.take_ident("a function name")?;
        let params = self.parse_params()?;
        self.consume(&TokKind::Arrow, "`->`")?;
        let ret = self.parse_type()?;
        let contract = self.parse_contract(&name)?;
        let body = self.parse_block()?;
        let span = start_span.to(self.prev_span());
        Ok(Item::Fn(FnItem {
            slag,
            name,
            params,
            ret,
            contract,
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
        let expr = self.parse_expr()?;
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
            _ => Err(SyntaxError::Unexpected {
                expected: "an effect (read/write/net/alloc/time/rand/panic/diverge)".to_string(),
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
                TokKind::Let => stmts.push(self.parse_let()?),
                TokKind::Return => stmts.push(self.parse_return()?),
                TokKind::Loop | TokKind::While => {
                    stmts.push(Stmt::Loop(self.parse_loop()?));
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

    fn parse_let(&mut self) -> PResult<Stmt> {
        self.consume(&TokKind::Let, "`let`")?;
        let mutable = self.eat(&TokKind::Mut);
        let name = self.take_ident("a binding name")?;
        let ty = if self.eat(&TokKind::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };
        self.consume(&TokKind::Eq, "`=`")?;
        let init = self.parse_expr()?;
        self.consume(&TokKind::Semi, "`;`")?;
        Ok(Stmt::Let {
            mutable,
            name,
            ty,
            init,
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
        let cond = self.parse_expr()?;
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
            let cond = self.parse_expr()?;
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

        let body = self.parse_block()?;
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
    fn parse_cmp(&mut self) -> PResult<Expr> {
        let lhs = self.parse_add()?;
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
            let rhs = self.parse_add()?;
            Ok(Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            })
        } else {
            Ok(lhs)
        }
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
        let mut expr = self.parse_ref()?;
        while self.eat(&TokKind::As) {
            let ty = self.parse_type()?;
            expr = Expr::Cast {
                expr: Box::new(expr),
                ty,
            };
        }
        Ok(expr)
    }

    fn parse_ref(&mut self) -> PResult<Expr> {
        if self.eat(&TokKind::Amp) {
            let mutable = self.eat(&TokKind::Mut);
            let expr = self.parse_ref()?;
            Ok(Expr::Ref {
                mutable,
                expr: Box::new(expr),
            })
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
            loop {
                args.push(self.parse_expr()?);
                if !self.eat(&TokKind::Comma) {
                    break;
                }
                if self.check(&TokKind::RParen) {
                    break;
                }
            }
        }
        self.consume(&TokKind::RParen, "`)`")?;
        Ok(args)
    }

    /// Parse an index argument: `i`, `..i`, `i..`, `i..j` (surface-grammar.md).
    fn parse_index_arg(&mut self) -> PResult<IndexArg> {
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
            TokKind::Int(v) => {
                self.bump();
                Ok(Expr::IntLit(v))
            }
            TokKind::Bool(b) => {
                self.bump();
                Ok(Expr::BoolLit(b))
            }
            TokKind::Ident(_) => self.parse_path_expr(),
            TokKind::Pipe | TokKind::OrOr => self.parse_closure(),
            TokKind::Match => self.parse_match(),
            TokKind::If => self.parse_if_expr(),
            TokKind::LParen => {
                self.bump();
                let inner = self.parse_expr()?;
                self.consume(&TokKind::RParen, "`)`")?;
                Ok(inner)
            }
            _ => Err(self.unexpected("an expression")),
        }
    }

    /// Parse a path expression `Ident (:: Ident)*` (`lo`, `u32::MAX`, `Some`).
    /// `::` is a PATH separator, never method dispatch (REQ-6).
    fn parse_path_expr(&mut self) -> PResult<Expr> {
        let mut segments = vec![self.take_ident("a path")?];
        while self.eat(&TokKind::ColonCol) {
            segments.push(self.take_ident("a path segment")?);
        }
        Ok(Expr::Path(segments))
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
        let scrutinee = self.parse_expr()?;
        self.consume(&TokKind::LBrace, "`{`")?;
        let mut arms = Vec::new();
        while !self.check(&TokKind::RBrace) && !self.at_eof() {
            let pattern = self.parse_pattern()?;
            self.consume(&TokKind::FatArrow, "`=>`")?;
            let body = self.parse_expr()?;
            arms.push(MatchArm { pattern, body });
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
        let cond = self.parse_expr()?;
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
        self.guard_recursion(Self::parse_pattern_inner)
    }

    fn parse_pattern_inner(&mut self) -> PResult<Pattern> {
        match self.peek().clone() {
            TokKind::Ident(name) if name == "_" => {
                self.bump();
                Ok(Pattern::Wildcard)
            }
            TokKind::Int(v) => {
                self.bump();
                Ok(Pattern::Literal(Expr::IntLit(v)))
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
            // decision 4 / REQ-8): written explicitly in a return position.
            TokKind::LParen => {
                self.bump();
                self.consume(&TokKind::RParen, "`)` to close the unit type `()`")?;
                Ok(Type::Unit)
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
                    _ => {
                        // A generic application `NAME<T>` (e.g. `Option<usize>`).
                        if self.eat(&TokKind::Lt) {
                            let arg = self.parse_type()?;
                            self.consume(&TokKind::Gt, "`>`")?;
                            Ok(Type::Generic {
                                name,
                                arg: Box::new(arg),
                            })
                        } else {
                            Err(SyntaxError::Unexpected {
                                expected: "a type (primitive, &T, &[T], or Name<T>)".to_string(),
                                found: format!("identifier `{name}`"),
                                span: self.prev_span(),
                            })
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
        TokKind::Int(v) => format!("integer `{v}`"),
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
        TokKind::If => "if",
        TokKind::Else => "else",
        TokKind::Loop => "loop",
        TokKind::While => "while",
        TokKind::Match => "match",
        TokKind::As => "as",
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
        TokKind::Amp => "&",
        TokKind::Pipe => "|",
        TokKind::Bang => "!",
        TokKind::Ident(_) | TokKind::Int(_) | TokKind::Bool(_) | TokKind::Str(_) | TokKind::Eof => {
            "<token>"
        }
    }
}
