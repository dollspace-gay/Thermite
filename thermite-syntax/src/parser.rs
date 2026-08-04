//! Thermite parser — hand-written recursive descent over the lexer's token
//! stream, producing the AST. The executable form of `surface-grammar.md`.
//!
//! Governing design: `.design/syntax/parser.md`. Two design-mandated properties
//! dominate: (a) per-item error recovery, where a syntax error inside one item
//! does not cascade into the next (§4.3, REQ-3): the top-level loop resyncs to the
//! next item-boundary token (`fn`/`spec`/`#[`/EOF) and keeps parsing; and
//! (b) mandatory-clause enforcement, where a `fn` missing `req`/`ens`/`fx`, or a
//! `loop`/`while` missing `inv`/`dec`, is a `SyntaxError`, never a default
//! (§4.1, REQ-2). It is registry-free: combinator calls parse as generic
//! `Expr::Call`s; it never consults thermite-spec. Returns a
//! diagnostics-bearing `ParseResult` and never panics (REQ-4).
//!
//! This module owns the crate's `SyntaxError` type — the first fallible code in
//! the toolchain introduces its own error enum (`.design/scaffold/workspace.md`
//! REQ-3).
//!
//! ## REQ status
//!
//! <!-- generated:reqs view=thermite-syntax-parser-core-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-SYNTAX-PARSER-ADDRESSING | shipped | `thermite-syntax/src/parser.rs` | Parser addressing substrate |  |
//! | REQ-SYNTAX-PARSER-CALL-SYNTAX | shipped | `thermite-syntax/src/parser.rs` | One call-syntax parser |  |
//! | REQ-SYNTAX-PARSER-MANDATORY-CLAUSES | shipped | `thermite-syntax/src/parser.rs` | Mandatory clause enforcement |  |
//! | REQ-SYNTAX-PARSER-OPERATOR-TIERS | shipped | `thermite-syntax/src/parser.rs` | Operator precedence parser tiers |  |
//! | REQ-SYNTAX-PARSER-PARTIALITY | shipped | `thermite-syntax/src/parser.rs` | Partial operators stay parse-only |  |
//! | REQ-SYNTAX-PARSER-RECOVERY | shipped | `thermite-syntax/src/parser.rs` | Per-item parse recovery |  |
//! | REQ-SYNTAX-PARSER-RECURSIVE-DESCENT | shipped | `thermite-syntax/src/parser.rs` | Recursive-descent parser structure |  |
//! | REQ-SYNTAX-PARSER-RESULT | shipped | `thermite-syntax/src/parser.rs` | Diagnostics-bearing parser result |  |
//! | REQ-SYNTAX-PARSER-ROUNDTRIP | shipped | `thermite-syntax/src/parser.rs` | Corpus round-trip parse fidelity |  |
//! <!-- /generated:reqs -->
//!
//! ## #193 body-position holes (`.design/forge/goal-repl.md` REQ-4)
//!
//! <!-- generated:reqs view=thermite-syntax-parser-goal-repl-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-SYNTAX-PARSER-HOLE-SCOPE | shipped | `thermite-syntax/src/parser.rs` | Goal REPL hole parser scope |  |
//! <!-- /generated:reqs -->
//!
//! ## Cluster C7 — Option/Result type parsing (`.design/basis/09-option-result.md`, #95)
//!
//! <!-- generated:reqs view=thermite-syntax-parser-option-result-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-SYNTAX-OPTRES-OPTION | shipped | `thermite-syntax/src/ast.rs` | Option type surface AST |  |
//! | REQ-SYNTAX-OPTRES-RESULT | shipped | `thermite-syntax/src/ast.rs` | Result type surface AST |  |
//! <!-- /generated:reqs -->
//!
//! ## Cluster C9-A — plain-`fn` recursion `dec` clause (`.design/basis/10-recursion-tuples.md`, #108)
//!
//! <!-- generated:reqs view=thermite-syntax-parser-recursion-tuples-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-SYNTAX-RECURSION-DEC | shipped | `thermite-syntax/src/ast.rs` | Plain function decreases clause AST |  |
//! <!-- /generated:reqs -->
//!
//! ## Cluster C10 — binding/control-flow ergonomics parse (`.design/basis/11-ergonomics.md`, #112)
//!
//! <!-- generated:reqs view=thermite-syntax-parser-ergonomics-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-SYNTAX-ERGONOMICS-FOR | shipped | `thermite-syntax/src/parser.rs` | For-loop desugar |  |
//! | REQ-SYNTAX-ERGONOMICS-IF-WHILE-LET | shipped | `thermite-syntax/src/parser.rs` | If-let and while-let desugars |  |
//! | REQ-SYNTAX-ERGONOMICS-MATCH-GUARD | shipped | `thermite-syntax/src/ast.rs` | Match guard surface AST |  |
//! | REQ-SYNTAX-ERGONOMICS-OR-PATTERN | shipped | `thermite-syntax/src/ast.rs` | Or-pattern surface AST |  |
//! | REQ-SYNTAX-ERGONOMICS-TUPLE-DESTRUCTURE | shipped | `thermite-syntax/src/parser.rs` | Tuple destructuring desugar |  |
//! <!-- /generated:reqs -->
//!
//! ## #16 boundary-fn parser extension (`.design/boundary/ffi-boundary.md`)
//!
//! <!-- generated:reqs view=thermite-syntax-parser-ffi-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-SYNTAX-PARSER-BOUNDARY-FN | shipped | `thermite-syntax/src/parser.rs` | Boundary function parser extension |  |
//! <!-- /generated:reqs -->
//!
//! ## Basis Stage 4 — bounded-collection type parse (`.design/basis/04-collections.md`)
//!
//! <!-- generated:reqs view=thermite-syntax-parser-collections-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-SYNTAX-COLLECTIONS-VEC | shipped | `thermite-syntax/src/ast.rs` | Vec type surface AST |  |
//! | REQ-SYNTAX-MAP-METHODS | shipped | `thermite-syntax/src/ast.rs` | Map operations as method calls |  |
//! | REQ-SYNTAX-MAP-TYPE | shipped | `thermite-syntax/src/ast.rs` | Map type surface AST |  |
//! <!-- /generated:reqs -->

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
    /// A `break;`/`continue;` statement parsed outside any `loop`/`while` body
    /// (parser.md REQ-10, #93). A structural rule (like the mandatory-clause
    /// rule): break/continue are loop-control statements and have no meaning at
    /// a function-body top level; `keyword` is `"break"` or `"continue"`.
    BreakContinueOutsideLoop { keyword: String, span: Span },
    /// A body-position hole `?N` parsed outside an exec-fn body
    /// (`.design/forge/goal-repl.md` REQ-4, #193). The v1 scope pin: holes are
    /// exec-fn-body statement position only; a `?N` in a `spec fn` body, a clause,
    /// an expression, or a signature is a structural parse error (`number` is the
    /// verbatim hole number written).
    HoleOutsideFnBody { number: u32, span: Span },
    /// A proof hole `?pN` parsed outside a proof block
    /// (`.design/stage1-forge-tier.md` REQ-3, AC-7). The forge-tier scope pin:
    /// proof holes are valid only inside a `lemma`/`proof`-item proof block; a
    /// `?pN` in fn-body statement position, a clause, an expression, or a signature
    /// is a structural parse error (`number` is the verbatim hole number written).
    ProofHoleOutsideProofBlock { number: u32, span: Span },
    /// A body hole `?N` parsed inside a proof block
    /// (`.design/stage1-forge-tier.md` REQ-3). A proof block admits only proof
    /// holes `?pN`; a body hole `?N` there is the mirror error of
    /// `ProofHoleOutsideProofBlock` (`number` is the verbatim hole number written).
    BodyHoleInProofBlock { number: u32, span: Span },
    /// A `@bv` machine-semantics clause tag appeared in a build without the
    /// shadow-flag plumbing compiled in (`.design/stage3-bv-reconstruction.md`
    /// REQ-1, AC-1). This is the structural lock (R-BV-1): the feature cannot
    /// exist without its visibility machinery, so a build that lacks the
    /// `bv` cargo feature treats the tag as a parse error rather than
    /// silently accepting (and then under-tracking) wraparound semantics. The span
    /// points at the `@`.
    BvTagWithoutShadowPlumbing { span: Span },
    /// A `@bv` tag was attached to a precondition. Preconditions are interpreted
    /// at the width of each tagged conclusion and do not define a width by
    /// themselves.
    BvTagOnPrecondition { span: Span },
    /// A `@bv` tag whose width is not one of the four committed widths
    /// `bv8`/`bv16`/`bv32`/`bv64` (`.design/stage3-bv-reconstruction.md` REQ-1).
    /// `found` is the verbatim token that followed `@` (e.g. `bv7`, `bv`, `foo`).
    /// Only reachable in a `bv` build (else `BvTagWithoutShadowPlumbing`
    /// fires first).
    BvWidthInvalid { found: String, span: Span },
}

/// The maximum recursive-descent nesting depth the parser will follow before
/// returning an `ExpressionTooDeep` diagnostic. Bounding the recursion keeps
/// external input from overflowing the native stack and aborting the process
/// (parser.md AC-4). The limit is a fixed constant (determinism, goal.md
/// R-CODE-5), comfortably above any human-authored nesting yet well below the
/// stack budget for a debug build.
///
/// This single bound guards every recursive-descent family, including the
/// expression ladder: nested expressions (`parse_expr`), nested types
/// (`parse_type` on `Option<Option<...>>`), nested patterns (`parse_pattern`
/// covering both the slice `[[...]]` and enum `Some(Some(...))` cycles), and the
/// tail-position `if/else` cycle (`parse_block`/`parse_if_parts`). Each family
/// re-enters its recursion through a guarded entry point, so a single shared
/// counter caps them all (a divergence the #29 expr-only guard missed).
///
/// The value must sit below the native-stack overflow point: each nesting level
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
            | SyntaxError::BreakContinueOutsideLoop { span, .. }
            | SyntaxError::HoleOutsideFnBody { span, .. }
            | SyntaxError::ProofHoleOutsideProofBlock { span, .. }
            | SyntaxError::BodyHoleInProofBlock { span, .. }
            | SyntaxError::BvTagWithoutShadowPlumbing { span }
            | SyntaxError::BvTagOnPrecondition { span }
            | SyntaxError::BvWidthInvalid { span, .. } => *span,
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
            SyntaxError::HoleOutsideFnBody { number, span } => write!(
                f,
                "hole `?{number}` outside an exec-fn body at byte {} (a `?N` hole is \
                 valid only in `fn`-body statement position, not in a `spec fn`, a \
                 clause, or an expression)",
                span.start
            ),
            SyntaxError::ProofHoleOutsideProofBlock { number, span } => write!(
                f,
                "proof hole `?p{number}` outside a proof block at byte {} (a `?pN` hole \
                 is valid only inside a `lemma`/`proof`-item proof block, not in a \
                 fn body, a clause, or an expression)",
                span.start
            ),
            SyntaxError::BodyHoleInProofBlock { number, span } => write!(
                f,
                "body hole `?{number}` inside a proof block at byte {} (a proof block \
                 admits only proof holes `?pN`)",
                span.start
            ),
            SyntaxError::BvTagWithoutShadowPlumbing { span } => write!(
                f,
                "the `@bv` machine-semantics clause tag at byte {} requires the \
                 shadow-flag plumbing, which is not compiled into this build (build \
                 `thermite-syntax` with the `bv` feature to enable it)",
                span.start
            ),
            SyntaxError::BvTagOnPrecondition { span } => write!(
                f,
                "`@bv` cannot annotate a precondition at byte {}; put the tag on \
                 each `ens` or `inv` whose fixed-width meaning you want",
                span.start
            ),
            SyntaxError::BvWidthInvalid { found, span } => write!(
                f,
                "`@{found}` at byte {} is not a valid `@bv` width (expected one of \
                 `bv8`/`bv16`/`bv32`/`bv64`)",
                span.start
            ),
        }
    }
}

impl std::error::Error for SyntaxError {}

/// The result of parsing: the recovered program and every diagnostic, so even a
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
    let mut program = Program {
        items: parser.items,
    };
    // Post-parse desugaring pass (`.design/stage1-forge-tier.md` REQ-3): fold the
    // refinement-type sugar (`x: T{P}` / `-> T{P}`) into the v1 `req`/`ens` clause
    // shapes so downstream stages see only v1 contracts plus the new item kinds.
    crate::desugar::desugar(&mut program);
    ParseResult {
        program,
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
/// `Sealed` onto a `StructItem`. A module-private dispatch type: the AST carries
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
    /// When true, a path primary does not consume a following `{ … }` as a
    /// struct-literal (`.design/basis/01-adts.md` REQ-2): set for the
    /// `match`/`if`/`while` head positions so `match s { … }` reads `{` as the
    /// arm block, not `s { … }` as a struct lit (the Rust no-struct-literal
    /// context). Saved/restored around each head so nested call/index/paren
    /// args re-enable struct literals.
    no_struct_literal: bool,
    /// When true, a postfix `.` is treated as a TERMINATOR rather than a
    /// field/method access — set only while parsing a quantifier's `<dom>`
    /// (`.design/stage2-stratified-cage.md` REQ-0). The binder grammar
    /// `forall (x : S) in <dom>. φ` separates the domain from the body with a `.`,
    /// which collides with the postfix field-access `.` (`xs.len()`). Suppressing
    /// the postfix `.` at the top level of the domain makes the first `.` after the
    /// domain unambiguously the body separator. A nested call/index/paren-group
    /// re-enables the postfix `.` via `with_struct_literal` (so `(a.b)` / `xs[a.b]`
    /// inside a domain still read their dots), mirroring the `no_struct_literal`
    /// re-enabling convention exactly.
    no_dot: bool,
    /// Current loop-nesting depth (parser.md REQ-10, #93). Incremented in
    /// `parse_loop_inner` around the loop body parse, decremented after. A
    /// `break;`/`continue;` parsed at depth 0 (outside any `loop`/`while` body)
    /// is a structural `SyntaxError`, analogous to the mandatory-clause rule
    /// (REQ-2): the parser owns presence/position; Verus owns the invariant/
    /// decreases semantics (`verus-lowering.md` REQ-12).
    loop_depth: usize,
    /// Current exec-fn-body nesting depth (`.design/forge/goal-repl.md` REQ-4,
    /// #193). Incremented around the body parse of an `Item::Fn` (`parse_fn`),
    /// decremented after; a nested `loop`/`if`/`while` block keeps it > 0 (a hole
    /// in a nested block within a fn body is still in "fn-body statement position").
    /// A `?N` parsed at depth 0 (a `spec fn` body, which parses at depth 0, or
    /// any non-fn-body context) is a structural `SyntaxError::HoleOutsideFnBody`
    /// (the v1 scope pin: holes are exec-fn-body statement position only). A `spec
    /// fn` body is parsed without incrementing this, so its holes are rejected.
    fn_body_depth: usize,
    /// The open holes (`?N`) accumulated while parsing the current exec-fn body, in
    /// document order (`.design/forge/goal-repl.md` REQ-4, #193). `parse_fn` saves
    /// then clears this around the body parse and pulls the collected holes into the
    /// `FnItem.holes` field, so holes from a nested fn (none in v0.1) / sibling fn
    /// never leak. A `?N` statement-dispatch arm in `parse_block` pushes here.
    pending_holes: Vec<Hole>,
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
            no_dot: false,
            loop_depth: 0,
            fn_body_depth: 0,
            pending_holes: Vec::new(),
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
        // The same bracketed sub-contexts that re-enable struct literals also
        // re-enable the postfix `.` inside a quantifier `<dom>` (REQ-0): a
        // parenthesised `(a.b)` or an index `xs[a.b]` reads its dots normally even
        // when the enclosing domain suppresses the top-level body-separator `.`.
        let saved_dot = self.no_dot;
        self.no_struct_literal = false;
        self.no_dot = false;
        let result = inner(self);
        self.no_struct_literal = saved;
        self.no_dot = saved_dot;
        result
    }

    /// Run `inner` with the postfix `.` suppressed as a terminator — used only for
    /// a quantifier's `<dom>`, so the `.` separating the domain from the body is
    /// unambiguous (`.design/stage2-stratified-cage.md` REQ-0). Bracketed
    /// sub-expressions inside the domain re-enable the postfix `.` via
    /// `with_struct_literal`, so `(a.b)` / `xs[a.b]` domains still parse.
    fn with_no_dot<T>(&mut self, inner: impl FnOnce(&mut Self) -> PResult<T>) -> PResult<T> {
        let saved = self.no_dot;
        self.no_dot = true;
        let result = inner(self);
        self.no_dot = saved;
        result
    }

    // ---- recursion-depth guard (parser.md AC-4) ----------------------------

    /// Bound the recursive-descent nesting depth: run `inner` one level deeper,
    /// returning a structured `ExpressionTooDeep` diagnostic (never a stack
    /// overflow / process abort) once the shared counter hits
    /// `MAX_RECURSION_DEPTH` (parser.md AC-4 / REQ-4; goal.md R-CODE-2).
    ///
    /// A single shared counter caps every recursive family: expressions
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
            ) || matches!(self.peek(), TokKind::Ident(word) if word == "const")
            {
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

        // Fixed-storage capacity declaration. `const` is contextual so it does
        // not become a reserved identifier in expressions.
        if matches!(self.peek(), TokKind::Ident(word) if word == "const") {
            if attr.is_some() {
                return Err(self.unexpected("`const` declarations take no attribute"));
            }
            return self.parse_const(start_span);
        }

        // A `struct` item (`.design/basis/01-adts.md` REQ-1) accepts the
        // `#[sealed]` abstraction-barrier attribute (REQ-8) and no other; an
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

        // Stage-1 forge-tier items (`.design/stage1-forge-tier.md` REQ-3), led by
        // contextual identifiers (not reserved keywords — like `for`/`Box`/`Vec`,
        // so they never collide with an existing program identifier): `prop fn`,
        // `lemma`, `proof for`, `witness`. The match peeks (and looks one ahead for
        // `prop fn`) without consuming, so a plain identifier named e.g. `proof`
        // used elsewhere is unaffected. None takes a leading attribute.
        let forge_kind: Option<&'static str> = match self.peek() {
            TokKind::Ident(w) if w == "prop" && matches!(self.peek_nth(1), TokKind::Fn) => {
                Some("prop")
            }
            TokKind::Ident(w) if w == "lemma" => Some("lemma"),
            TokKind::Ident(w) if w == "proof" => Some("proof"),
            TokKind::Ident(w) if w == "witness" => Some("witness"),
            _ => None,
        };
        if let Some(kind) = forge_kind {
            if attr.is_some() {
                return Err(self.unexpected(
                    "a forge-tier item (`prop fn`/`lemma`/`proof`/`witness`) takes no attribute",
                ));
            }
            let forge = match kind {
                "prop" => ForgeItem::PropFn(self.parse_prop_fn(start_span)?),
                "lemma" => ForgeItem::Lemma(self.parse_lemma(start_span)?),
                "proof" => ForgeItem::Proof(self.parse_proof_item(start_span)?),
                _ => ForgeItem::Witness(self.parse_witness(start_span)?),
            };
            return Ok(Item::Forge(forge));
        }

        if self.check(&TokKind::Spec) {
            // Neither `#[slag]` nor `#[boundary]` attaches to a `spec fn`
            // (surface-grammar Item; ffi-boundary.md "#[boundary] is not valid on
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

    fn parse_const(&mut self, start_span: Span) -> PResult<Item> {
        let keyword = self.take_ident("`const`")?;
        debug_assert_eq!(keyword, "const");
        let name = self.take_ident("a constant name")?;
        self.consume(&TokKind::Colon, "`:` after the constant name")?;
        let ty = self.take_ident("`usize` as the capacity constant type")?;
        if ty != "usize" {
            return Err(self.unexpected("`usize` as the capacity constant type"));
        }
        self.consume(&TokKind::Eq, "`=` in the constant declaration")?;
        let (value, raw) = match self.peek().clone() {
            TokKind::Int { value, raw } => {
                self.bump();
                (value, raw)
            }
            _ => return Err(self.unexpected("an integer capacity value")),
        };
        self.consume(&TokKind::Semi, "`;` after the constant declaration")?;
        Ok(Item::Const(ConstItem {
            name,
            value,
            raw,
            span: start_span.to(self.prev_span()),
        }))
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
                    // The lexer/parser do not validate field names; that is a
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
        // Refinement-type sugar (`.design/stage1-forge-tier.md` REQ-3): a refined
        // parameter `x: T{P}` / return `-> T{P}` records its predicate here; the
        // post-parse `desugar` pass folds them into `req`/`ens`. Only a `fn`
        // collects refinements (it carries the v1 contract); `spec fn`/`prop fn`/
        // `lemma` params parse without (a `{` after their param type is an error).
        let mut refinements: Vec<Refinement> = Vec::new();
        let params = self.parse_params_inner(Some(&mut refinements))?;
        self.consume(&TokKind::Arrow, "`->`")?;
        let ret = self.parse_type()?;
        // A return refinement `-> T{P}` (REQ-3): the `{` here is unambiguously a
        // refinement, not the body — the mandatory contract (`req`…) parses next,
        // and the body `{` only follows `fx`.
        if self.check(&TokKind::LBrace) {
            let pred = self.parse_refinement_clause()?;
            refinements.push(Refinement {
                target: RefinementTarget::Result,
                pred,
            });
        }
        let contract = self.parse_contract(&name)?;
        // The optional `dec <measure>` termination clause of a recursive exec `fn`
        // (`.design/basis/10-recursion-tuples.md` REQ-1, C9-A). It parses after the
        // contract (`req`/`ens`/`fx`) and before the body, the OQ-4 byte-stable
        // slot mirroring the loop order (`inv`s then `dec`). Absent → `None` (a
        // non-recursive `fn`); the `req`/`ens`/`fx` parse is unchanged for every
        // existing non-recursive fn. A self-calling fn lacking this clause (and not
        // `fx diverge`) is a validator error (REQ-2), not a parse error: the
        // grammar admits it; the cage rejects it.
        let dec = if self.check(&TokKind::Dec) {
            Some(self.parse_dec_clause()?)
        } else {
            None
        };
        // Body fork (ffi-boundary.md REQ-3, OQ-2): a `#[boundary]` fn is bodyless,
        // terminated by `;` (the foreign body lives in the foreign crate); a
        // non-`#[boundary]` fn requires a `{ }` body (the §4.1 body-second rule).
        // The `;` body is valid only when `boundary.is_some()`: a bodyless fn
        // without `#[boundary]` is a clear parse error, never silently a boundary
        // fn (a normal fn missing its body must not be mistaken for a foreign one).
        // The open holes (`?N`) the body carries (`.design/forge/goal-repl.md`
        // REQ-4); empty for a boundary fn (no Thermite body) and for a hole-free fn.
        let mut holes: Vec<Hole> = Vec::new();
        let body = if boundary.is_some() {
            // A foreign fn must be bodyless: `;`, not `{ }`. A `{ }` body on a
            // `#[boundary]` fn is an error; there is no Thermite body to prove.
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
            // A non-boundary fn must have a `{ }` body. A `;` here is the OQ-2
            // case, a bodyless fn without `#[boundary]`: a clear, distinct error,
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
            // Parse the exec fn body inside a fn-body scope so a `?N` hole is
            // accepted in statement position (`.design/forge/goal-repl.md` REQ-4):
            // save + clear the hole accumulator, mark the fn-body depth, parse, then
            // pull the holes back. Holes from a sibling/prior fn never leak in.
            let saved_holes = std::mem::take(&mut self.pending_holes);
            self.fn_body_depth += 1;
            let body_result = self.parse_block();
            self.fn_body_depth -= 1;
            holes = std::mem::replace(&mut self.pending_holes, saved_holes);
            Some(body_result?)
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
            holes,
            refinements,
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
        let dec = self.parse_dec_clause()?;
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

    /// Parse a `[#[sealed]] struct NAME { field: type, … } [inv <expr>]` item
    /// (`.design/basis/01-adts.md` REQ-1; the seal is
    /// `.design/basis/06-provenance-and-sinks.md` REQ-8). The optional `inv`
    /// type-invariant clause follows the closing brace and reuses the existing
    /// `Clause` (verbatim text + parsed expr). `sealed` is the `#[sealed]`
    /// abstraction-barrier flag the caller already parsed from the leading
    /// attribute (REQ-8). The validator rules (field well-formedness; the
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

    /// Parse a `{ field: type, … }` field-definition block, shared by `struct`
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

    /// Parse an `enum NAME { Variant, Variant(type, …), Variant { field: type, … }
    /// }` item (`.design/basis/01-adts.md` REQ-2). A variant is `Unit` (bare
    /// name), `Tuple` (`(type, …)`), or `Struct` (`{ field: type, … }`). A
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

    // ---- Stage-1 forge-tier items (`.design/stage1-forge-tier.md` REQ-3) -------

    /// Parse a `prop fn NAME(params) -> type [dec <measure>] { body }` proposition
    /// definition (REQ-3). Mirrors `parse_spec_fn` (it is a forge-tier definition,
    /// not an exec fn): the body is parsed without entering an exec-fn-body scope,
    /// so a `?N` body hole in a prop fn body is rejected (`HoleOutsideFnBody`), the
    /// same as a `spec fn` body. The optional `dec` accepts the same measure surface
    /// the other `dec` positions do (`dec <expr>` / `dec lex(…)` / `dec wf <rel>`).
    fn parse_prop_fn(&mut self, start_span: Span) -> PResult<PropFnItem> {
        self.expect_contextual("prop")?;
        self.consume(&TokKind::Fn, "`fn`")?;
        let name = self.take_ident("a prop fn name")?;
        let params = self.parse_params()?;
        self.consume(&TokKind::Arrow, "`->`")?;
        let ret = self.parse_type()?;
        let dec = if self.check(&TokKind::Dec) {
            Some(self.parse_dec_clause()?)
        } else {
            None
        };
        let body = self.parse_block()?;
        let span = start_span.to(self.prev_span());
        Ok(PropFnItem {
            name,
            params,
            ret,
            dec,
            body,
            span,
        })
    }

    /// Parse a `lemma NAME(params) req clause ens clause+ proof { … }` item
    /// (REQ-3). The `req`/`ens` cardinality mirrors a `fn` contract (exactly one
    /// `req`, one-or-more `ens`) but a lemma carries no `fx` row — it is pure proof.
    /// The `proof { … }` block is captured verbatim (its tactic content is the
    /// frozen battery, increment 2c) with its `?pN` proof holes extracted.
    fn parse_lemma(&mut self, start_span: Span) -> PResult<LemmaItem> {
        self.expect_contextual("lemma")?;
        let name = self.take_ident("a lemma name")?;
        let params = self.parse_params()?;
        // `req` — exactly one, first (the lemma's hypothesis).
        if !self.check(&TokKind::Req) {
            return Err(SyntaxError::MissingClause {
                item: name.clone(),
                clause: "req".to_string(),
                span: self.peek_span(),
            });
        }
        let req = self.parse_clause(&TokKind::Req)?;
        // `ens` — one or more (the lemma's conclusions).
        if !self.check(&TokKind::Ens) {
            return Err(SyntaxError::MissingClause {
                item: name.clone(),
                clause: "ens".to_string(),
                span: self.peek_span(),
            });
        }
        let mut ens = Vec::new();
        while self.check(&TokKind::Ens) {
            ens.push(self.parse_clause(&TokKind::Ens)?);
        }
        // `proof { … }` — mandatory.
        if !self.eat_contextual("proof") {
            return Err(self.unexpected("`proof { … }` to discharge the lemma"));
        }
        let proof = self.scan_proof_block()?;
        let span = start_span.to(self.prev_span());
        Ok(LemmaItem {
            name,
            params,
            req,
            ens,
            proof,
            span,
        })
    }

    /// Parse a `proof for f { clause by { … } … }` item (REQ-3): a proof discharging
    /// specific contract clauses (`ens#k`) of an existing function `f`. Each
    /// obligation is a [`ClauseSelector`] + a `by { … }` proof block (with `?pN`
    /// proof holes). The clauses are resolved against `f` by the proof view
    /// (increment 2e); here they are parsed + addressed (`f.proof.ens#k`).
    fn parse_proof_item(&mut self, start_span: Span) -> PResult<ProofItem> {
        self.expect_contextual("proof")?;
        self.expect_contextual("for")?;
        let target = self.take_ident("the target function name of `proof for`")?;
        self.consume(&TokKind::LBrace, "`{`")?;
        let mut obligations = Vec::new();
        while !self.check(&TokKind::RBrace) && !self.at_eof() {
            let ob_start = self.peek_span();
            let clause = self.parse_clause_selector()?;
            if !self.eat_contextual("by") {
                return Err(self.unexpected("`by { … }` to discharge the clause"));
            }
            let proof = self.scan_proof_block()?;
            let ob_span = ob_start.to(self.prev_span());
            obligations.push(ProofObligation {
                clause,
                proof,
                span: ob_span,
            });
        }
        self.consume(&TokKind::RBrace, "`}`")?;
        let span = start_span.to(self.prev_span());
        Ok(ProofItem {
            target,
            obligations,
            span,
        })
    }

    /// Parse a clause selector `ens#k` / `req` / `inv#k` (REQ-3): the clause family
    /// keyword (a reserved keyword, accepted by kind) plus an optional `#k` ordinal.
    fn parse_clause_selector(&mut self) -> PResult<ClauseSelector> {
        let keyword = if self.eat(&TokKind::Ens) {
            "ens".to_string()
        } else if self.eat(&TokKind::Req) {
            "req".to_string()
        } else if self.eat(&TokKind::Inv) {
            "inv".to_string()
        } else {
            return Err(self.unexpected("a clause family (`ens`/`req`/`inv`)"));
        };
        let index = if self.eat(&TokKind::Hash) {
            match self.peek().clone() {
                TokKind::Int { value, .. } => {
                    self.bump();
                    Some(value as u32)
                }
                _ => return Err(self.unexpected("a clause ordinal `#k` after `#`")),
            }
        } else {
            None
        };
        Ok(ClauseSelector { keyword, index })
    }

    /// Parse a `witness { inhabit (…); falsify N; }` covenant witness block
    /// (REQ-3/REQ-4). Surface only: `inhabit` records an author-stated witness tuple
    /// of expressions, `falsify` records the generator budget. The covenant logic
    /// (type-check + execute against `req`, run the generator) is increment 2b.
    fn parse_witness(&mut self, start_span: Span) -> PResult<WitnessBlock> {
        self.expect_contextual("witness")?;
        self.consume(&TokKind::LBrace, "`{`")?;
        let mut inhabits = Vec::new();
        let mut falsifies = Vec::new();
        while !self.check(&TokKind::RBrace) && !self.at_eof() {
            let dir_start = self.peek_span();
            if self.eat_contextual("inhabit") {
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
                self.consume(&TokKind::Semi, "`;` to end the `inhabit` directive")?;
                let span = dir_start.to(self.prev_span());
                inhabits.push(Inhabit { args, span });
            } else if self.eat_contextual("falsify") {
                let budget = match self.peek().clone() {
                    TokKind::Int { value, .. } => {
                        self.bump();
                        value as u64
                    }
                    _ => return Err(self.unexpected("a `falsify` budget integer")),
                };
                self.consume(&TokKind::Semi, "`;` to end the `falsify` directive")?;
                let span = dir_start.to(self.prev_span());
                falsifies.push(Falsify { budget, span });
            } else {
                return Err(self.unexpected("`inhabit (…);` or `falsify N;` in a `witness` block"));
            }
        }
        self.consume(&TokKind::RBrace, "`}`")?;
        let span = start_span.to(self.prev_span());
        Ok(WitnessBlock {
            inhabits,
            falsifies,
            span,
        })
    }

    /// Scan a forge-tier proof block `{ … }` (REQ-3) without structurally parsing
    /// its tactic content (the frozen tactic battery is increment 2c, REQ-5).
    /// Captures the verbatim inner source `text` plus the open proof holes (`?pN`,
    /// [`HoleContext::Proof`]) in document order, tracking brace depth so nested
    /// `{ … }` (e.g. a `by { … }` or `calc` sub-block) are spanned correctly. A body
    /// hole `?N` inside a proof block is a structured `SyntaxError::BodyHoleInProofBlock`
    /// (a proof block admits only proof holes). An unterminated block is a structured
    /// error, never a panic (REQ-4 Result discipline).
    fn scan_proof_block(&mut self) -> PResult<ProofBlock> {
        let open = self.consume(&TokKind::LBrace, "`{` to open the proof block")?;
        let inner_start = open.span.end();
        let mut depth = 1usize;
        let mut holes = Vec::new();
        loop {
            if self.at_eof() {
                return Err(self.unexpected("`}` to close the proof block"));
            }
            // Clone the token kind so the borrow is released before `bump` (proof
            // blocks are small; the clone is not on a hot path).
            match self.peek().clone() {
                TokKind::LBrace => {
                    self.bump();
                    depth += 1;
                }
                TokKind::RBrace => {
                    let close = self.bump();
                    depth -= 1;
                    if depth == 0 {
                        let inner = Span::new(inner_start, close.span.start - inner_start);
                        let text = self.span_text(inner);
                        let span = Span::new(open.span.start, close.span.end() - open.span.start);
                        return Ok(ProofBlock { text, holes, span });
                    }
                }
                TokKind::Hole { number, proof } => {
                    let tok = self.bump();
                    if proof {
                        holes.push(Hole {
                            number,
                            span: tok.span,
                            context: HoleContext::Proof,
                        });
                    } else {
                        return Err(SyntaxError::BodyHoleInProofBlock {
                            number,
                            span: tok.span,
                        });
                    }
                }
                _ => {
                    self.bump();
                }
            }
        }
    }

    /// Consume a contextual-keyword identifier (a non-reserved word matched by
    /// name, like `for`/`Box`/`Vec`), returning whether it was present.
    fn eat_contextual(&mut self, word: &str) -> bool {
        if matches!(self.peek(), TokKind::Ident(w) if w == word) {
            self.bump();
            true
        } else {
            false
        }
    }

    /// Consume a required contextual-keyword identifier or produce a `SyntaxError`.
    fn expect_contextual(&mut self, word: &str) -> PResult<()> {
        if self.eat_contextual(word) {
            Ok(())
        } else {
            Err(self.unexpected(&format!("`{word}`")))
        }
    }

    /// Parse a `(name: Type, …)` parameter list (no refinements). Used by `spec
    /// fn`/`prop fn`/`lemma`, where a refinement `{P}` after a param type is not
    /// accepted (a stray `{` there surfaces as the normal `,`/`)` parse error).
    fn parse_params(&mut self) -> PResult<Vec<Param>> {
        self.parse_params_inner(None)
    }

    /// Parse a `(name: Type[{P}], …)` parameter list. When `refinements` is
    /// `Some`, a refinement-type sugar `{P}` after a parameter's type
    /// (`.design/stage1-forge-tier.md` REQ-3) is parsed and recorded against that
    /// parameter (folded into `req` by the post-parse desugar pass); when `None`,
    /// no refinement is consumed (a `{` then is a normal unexpected token). Only a
    /// `fn` passes `Some` (it carries the v1 contract the predicate folds into).
    fn parse_params_inner(
        &mut self,
        mut refinements: Option<&mut Vec<Refinement>>,
    ) -> PResult<Vec<Param>> {
        self.consume(&TokKind::LParen, "`(`")?;
        let mut params = Vec::new();
        if !self.check(&TokKind::RParen) {
            loop {
                let name = self.take_ident("a parameter name")?;
                self.consume(&TokKind::Colon, "`:`")?;
                let ty = self.parse_type()?;
                // A refinement `x: T{P}` (only when the caller collects them).
                if let Some(refs) = refinements.as_deref_mut() {
                    if self.check(&TokKind::LBrace) {
                        let pred = self.parse_refinement_clause()?;
                        refs.push(Refinement {
                            target: RefinementTarget::Param(name.clone()),
                            pred,
                        });
                    }
                }
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

    /// Parse a refinement predicate `{ P }` (`.design/stage1-forge-tier.md` REQ-3):
    /// the brace-delimited contract-position expression of an `x: T{P}` / `-> T{P}`
    /// refinement. Captured as a [`Clause`] (parsed expr + verbatim text + span) so
    /// the desugar pass can fold it into `req`/`ens` with faithful text. The
    /// predicate is a no-struct-literal head (like a clause), so a trailing
    /// `Name { … }` is not mis-read as a struct literal.
    fn parse_refinement_clause(&mut self) -> PResult<Clause> {
        self.consume(&TokKind::LBrace, "`{` to open a refinement predicate")?;
        let start = self.peek_span();
        let expr = self.with_no_struct_literal(Self::parse_expr)?;
        let end = self.prev_span();
        self.consume(&TokKind::RBrace, "`}` to close the refinement predicate")?;
        let span = start.to(end);
        let text = self.span_text(span);
        // Refinement sugar has no `@bv` tag.
        Ok(Clause {
            expr,
            text,
            span,
            bv: None,
        })
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
        // Parse the tag before recording the expression span so `Clause.text`
        // remains the expression alone.
        let bv = self.parse_bv_tag()?;
        if matches!(keyword, TokKind::Req) {
            if let Some(tag) = bv {
                return Err(SyntaxError::BvTagOnPrecondition { span: tag.span });
            }
        }
        let start = self.peek_span();
        // A clause expression is a no-struct-literal head: a clause is followed
        // by another clause keyword or a block `{` (a loop body, a spec-fn body),
        // so a trailing `Name { … }` must not be read as a struct literal
        // (`.design/basis/01-adts.md` REQ-2; e.g. `dec xs.len() - i { … }` —
        // the `{` is the body). Struct literals inside call args / parens still
        // parse (those re-enable the context).
        let expr = self.with_no_struct_literal(Self::parse_expr)?;
        let end = self.prev_span();
        let span = start.to(end);
        let text = self.span_text(span);
        Ok(Clause {
            expr,
            text,
            span,
            bv,
        })
    }

    /// Parse an optional `@bvN` / `@bvN(nowrap)` machine-semantics clause tag
    /// (`.design/stage3-bv-reconstruction.md` REQ-1), the first clause-level
    /// annotation in `thermite-syntax`. Returns `Ok(None)` when no `@` follows the
    /// clause keyword (every v1/v2 clause). When a `@` IS present, the tag parses
    /// only if the shadow-flag plumbing is compiled in (the `bv` cargo
    /// feature) — this is the build-flag gate, REQ-1's structural lock R-BV-1:
    ///
    /// - without the feature, the `@`-handling code path below is `#[cfg]`-removed,
    ///   so the tag is a structured parse error (`BvTagWithoutShadowPlumbing`) and
    ///   the feature cannot exist in the build (AC-1's negative half).
    /// - with the feature, `@bvN` for N ∈ {8, 16, 32, 64} parses, plus the optional
    ///   `(nowrap)` modifier (`@bvN(nowrap)`, REQ-5's surface). A bad width
    ///   (`@bv7`, `@bv`) is `BvWidthInvalid`; a malformed modifier is the generic
    ///   unexpected-token error.
    ///
    /// Postconditions, invariants, and lemma conclusions accept the tag.
    /// Preconditions are rejected because their width comes from the tagged
    /// conclusion they guard. `dec` and `fx` carry no tag.
    fn parse_bv_tag(&mut self) -> PResult<Option<BvTag>> {
        if !self.check(&TokKind::At) {
            return Ok(None);
        }
        let at_span = self.peek_span();
        #[cfg(not(feature = "bv"))]
        {
            // The shadow-flag plumbing is absent: the tag cannot parse (R-BV-1).
            Err(SyntaxError::BvTagWithoutShadowPlumbing { span: at_span })
        }
        #[cfg(feature = "bv")]
        {
            self.bump(); // consume `@`
            let width = self.parse_bv_width()?;
            // An optional `(nowrap)` modifier (REQ-5's surface form).
            let mut nowrap = false;
            let mut end = self.prev_span();
            if self.eat(&TokKind::LParen) {
                let marker = self.take_ident("`nowrap`")?;
                if marker != "nowrap" {
                    return Err(SyntaxError::Unexpected {
                        expected: "`nowrap`".to_string(),
                        found: format!("identifier `{marker}`"),
                        span: self.prev_span(),
                    });
                }
                self.consume(&TokKind::RParen, "`)` to close `(nowrap)`")?;
                nowrap = true;
                end = self.prev_span();
            }
            Ok(Some(BvTag {
                width,
                nowrap,
                span: at_span.to(end),
            }))
        }
    }

    /// Parse the `bvN` width token of a `@bv` tag (`.design/stage3-bv-reconstruction.md`
    /// REQ-1). The `bvN` spelling lexes as a single identifier (`bv64`), so this
    /// matches the whole ident against the four committed widths; anything else is
    /// `BvWidthInvalid`. Only compiled in a `bv` build.
    #[cfg(feature = "bv")]
    fn parse_bv_width(&mut self) -> PResult<BvWidth> {
        let span = self.peek_span();
        let ident = match self.peek().clone() {
            TokKind::Ident(w) => w,
            other => {
                return Err(SyntaxError::BvWidthInvalid {
                    found: token_text(&other).to_string(),
                    span,
                });
            }
        };
        let width = match ident.as_str() {
            "bv8" => BvWidth::W8,
            "bv16" => BvWidth::W16,
            "bv32" => BvWidth::W32,
            "bv64" => BvWidth::W64,
            _ => return Err(SyntaxError::BvWidthInvalid { found: ident, span }),
        };
        self.bump(); // consume the width ident
        Ok(width)
    }

    /// Parse a `dec <measure>` clause, supporting the forge-tier measure forms
    /// (`.design/stage1-forge-tier.md` REQ-3, Q-DECWF). Consumes the `dec` keyword,
    /// then:
    /// - `dec wf <rel>` — a WELL-FOUNDED relation (ASCII spelling per Q-DECWF, not
    ///   the Unicode `⟨⟩` — the lexer stays ASCII-only). Since `wf <rel>` is two
    ///   tokens (not one expression), it is normalized to the registry-free call
    ///   `wf(<rel>)` so a downstream consumer keys on the `wf` callee. A bare `wf`
    ///   not followed by an expression (a `{` body, a clause keyword, EOF) is an
    ///   ordinary measure named `wf` (the v1 reading), handled by the plain path.
    /// - `dec lex(<e>, …)` — a LEXICOGRAPHIC tuple. `lex` is a contextual ident, so
    ///   `lex(...)` is ALREADY an ordinary `Expr::Call` (registry-free, like the
    ///   `forall_in`/`sorted` combinators) — no special parse; the plain path
    ///   captures it and a downstream consumer keys on the `lex` callee.
    /// - `dec <expr>` — the v1 plain measure (`dec n`, `dec hi - i`), unchanged.
    fn parse_dec_clause(&mut self) -> PResult<Clause> {
        self.consume(&TokKind::Dec, "`dec`")?;
        let start = self.peek_span();
        // `dec wf <rel>`: the `wf` marker followed by a relation expression.
        if matches!(self.peek(), TokKind::Ident(w) if w == "wf") && self.nth_starts_expr(1) {
            self.bump(); // consume `wf`
            let rel = self.with_no_struct_literal(Self::parse_expr)?;
            let end = self.prev_span();
            let span = start.to(end);
            let text = self.span_text(span);
            // Normalize to the registry-free `wf(<rel>)` call (REQ-3) so the v1
            // clause shape downstream is an ordinary `Expr::Call`.
            let expr = Expr::Call {
                callee: Box::new(Expr::Path(vec!["wf".to_string()])),
                args: vec![rel],
            };
            return Ok(Clause {
                expr,
                text,
                span,
                bv: None,
            });
        }
        // `dec <expr>` (incl. `dec lex(...)` as an ordinary call) — the same
        // no-struct-literal head as `parse_clause` (a trailing `{` is the body).
        let expr = self.with_no_struct_literal(Self::parse_expr)?;
        let end = self.prev_span();
        let span = start.to(end);
        let text = self.span_text(span);
        // A `dec` measure carries no `@bv` tag.
        Ok(Clause {
            expr,
            text,
            span,
            bv: None,
        })
    }

    /// True if the token `n` ahead of the cursor can begin an expression — used to
    /// distinguish `dec wf <rel>` (the well-founded form) from a bare measure named
    /// `wf` (`dec wf` followed by a `{` body / clause keyword / end). A negative
    /// check: the listed tokens never start an expression in `dec` position.
    fn nth_starts_expr(&self, n: usize) -> bool {
        !matches!(
            self.peek_nth(n),
            TokKind::LBrace
                | TokKind::RBrace
                | TokKind::RParen
                | TokKind::Semi
                | TokKind::Comma
                | TokKind::Eof
                | TokKind::Req
                | TokKind::Ens
                | TokKind::Fx
                | TokKind::Inv
                | TokKind::Dec
        )
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
            "platform" => {
                self.consume(&TokKind::LParen, "`(` after `platform`")?;
                let span = self.peek_span();
                let domain = self.take_ident("a platform domain")?;
                self.consume(&TokKind::RParen, "`)` after the platform domain")?;
                let Some(domain) = PlatformDomain::from_surface(&domain) else {
                    return Err(SyntaxError::Unexpected {
                        expected: "a platform domain (boot/memory/mmio/pio/irq/cpu/atomic/smp/dma/clock/entropy/power)"
                            .to_string(),
                        found: format!("identifier `{domain}`"),
                        span,
                    });
                };
                Ok(Effect::Platform(domain))
            }
            _ => Err(SyntaxError::Unexpected {
                expected: "an effect (read/write/net/alloc/time/rand/panic/diverge/term/platform)"
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
                // ergonomic (REQ-2). `for`/`in` are contextual identifiers (not
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
                // A body-position structural hole `?N` (`.design/forge/goal-repl.md`
                // REQ-4, #193). Valid only in exec-fn-body statement position
                // (`self.fn_body_depth > 0`): record it on the fn's hole list (the
                // parser's accumulator, document order) and emit nothing into the
                // statement stream. A hole is not a `Stmt` (it never lowers; the
                // holed item short-circuits at `forge check`). A `?N` in a `spec fn`
                // body (depth 0) or any non-fn-body context is a structural
                // `SyntaxError::HoleOutsideFnBody` (the v1 scope pin: holes are
                // exec-fn-body statement position only). A `?N` in expression /
                // clause / signature position is unreachable here; those are parsed
                // by `parse_primary`/`parse_clause`, where `TokKind::Hole` is not a
                // primary, so it surfaces as a normal "unexpected token" parse error.
                // A proof hole `?pN` in fn-body position is rejected by `parse_hole`
                // (ProofHoleOutsideProofBlock — proof holes live only in proof blocks).
                TokKind::Hole { .. } => self.parse_hole()?,
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
                    // value-ness, not source position: "the expression form ...
                    // must have a value; the statement form does not." (OQ-3:
                    // "the corpus only uses the statement form".) It is the
                    // block's tail value only when it (a) has an `else`, (b)
                    // produces a value (its then-branch block has a tail expr,
                    // `then.tail.is_some()`), and (c) nothing follows it before
                    // the closing `}` (ast.md REQ-6 `Expr::If`). A value-less
                    // trailing `if/else` (both branches statement-only, e.g.
                    // corpus `if .. { lo = mid + 1; } else { hi = mid; }`) is the
                    // statement form and leaves the block `tail: None`.
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
    /// `let y = __td<n>.1;` per element, reusing the shipped `Expr::TupleProj`
    /// (C9-B). Pure desugar: no new AST node, the projection lowers + verifies
    /// today. v0.1 admits only flat binding/`_` sub-patterns in a tuple `let`
    /// (a nested `let (Some(x), y) = …` is out of scope, §2.3 one-way).
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
    /// (`.design/basis/11-ergonomics.md` REQ-2). `for`/`in` are contextual
    /// identifiers (not reserved keywords), so the caller dispatched on
    /// `Ident("for")`. Pure desugar to the shipped `while`+`inv`/`dec` core:
    ///   `let mut i = lo;`
    ///   `while i < hi inv <user invs> dec hi - i { B; i = i + 1; }`
    /// The user supplies the `inv` (mandatory, §4.1, at least one); the `dec` is
    /// automatic (`hi - i`, the canonical monotone measure of a bounded range:
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
        // inv clauses opens the body, never a struct literal, mirrors `while`).
        let (lo, hi) = self.with_no_struct_literal(|p| {
            let lo = p.parse_expr()?;
            p.consume(
                &TokKind::DotDot,
                "`..` in the `for` range `lo..hi` (only an exclusive integer range is admitted)",
            )?;
            let hi = p.parse_expr()?;
            Ok((lo, hi))
        })?;
        // `inv` — one or more (mandatory; the for-loop is a loop, §4.1). No `dec`;
        // it is synthesized below (REQ-2).
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
        // A `dec` on a `for` is an error; the `dec` is automatic (REQ-2).
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
            bv: None,
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
    /// (`.design/basis/11-ergonomics.md` REQ-5). Pure desugar to the shipped
    /// `Expr::Match { e, [P => T, _ => E] }`. v0.1 admits the value form (both
    /// branches reduce to a tail expression) with a mandatory `else`; the
    /// statement-`if`-without-`else` `_ => ()` form needs a unit expr the grammar
    /// does not surface (OQ-4). Returns the `Expr::Match` and whether it is in
    /// value (tail) position (always true here, the value form). The caller
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
    /// value-producing block whose body is its tail expression (`{ v }`); a
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
    /// (`.design/basis/11-ergonomics.md` REQ-5). Pinned (GROUNDED): desugar to the
    /// canonical `while (e is Variant)` form, not `loop { match … None => break }`
    /// (the loop+break shape fails to carry the post-exit fact, L0). v0.1 admits
    /// a payload-free pattern (`Variant`, `Variant(_)`, `Variant { .. }`); the
    /// condition is `e is Variant` (the shipped `Expr::Is`), no payload rebind.
    /// The user supplies the loop `inv`/`dec` as for a `while` (mandatory,
    /// §4.1). Returns the `LoopNode`.
    fn parse_while_let(&mut self) -> PResult<LoopNode> {
        let start = self.peek_span();
        self.consume(&TokKind::While, "`while`")?;
        self.consume(&TokKind::Let, "`let`")?;
        let pattern = self.parse_pattern()?;
        // Extract the variant head of the payload-free pattern (the shipped
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
        let dec = self.parse_dec_clause()?;
        self.loop_depth += 1;
        let body_result = self.parse_block();
        self.loop_depth -= 1;
        let body = body_result?;
        // The condition `e is Variant` (the shipped `Expr::Is`).
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

    /// Parse a body-position structural hole `?N` (`.design/forge/goal-repl.md`
    /// REQ-4, #193). Records the hole (number + span) on the parser's accumulator
    /// (`pending_holes`, document order — pulled into `FnItem.holes` by `parse_fn`)
    /// and consumes the token, emitting no statement (a hole is not a `Stmt`). A
    /// hole is value-less + payload-less and takes no trailing `;` (the §5.1
    /// `body = ?0` shape). It is valid only in exec-fn-body statement position
    /// (`self.fn_body_depth > 0`); a `?N` in a `spec fn` body (depth 0) is a
    /// structural `SyntaxError::HoleOutsideFnBody` (the v1 scope pin). Returns
    /// `Ok(())` — `parse_block` pushes nothing into `stmts`.
    fn parse_hole(&mut self) -> PResult<()> {
        let span = self.peek_span();
        let (number, proof) = match self.peek() {
            TokKind::Hole { number, proof } => (*number, *proof),
            // Unreachable: the caller dispatches here only on a `TokKind::Hole`.
            // A structured error (no panic, R-CODE-2) keeps the parser total.
            other => {
                return Err(SyntaxError::Unexpected {
                    expected: "a hole `?N`".to_string(),
                    found: describe(other),
                    span,
                });
            }
        };
        // Consume the `?N`/`?pN` token. A proof hole `?pN` is never valid in
        // fn-body statement position: proof holes live only inside a proof block
        // (`.design/stage1-forge-tier.md` REQ-3, AC-7). The proof-block scanner
        // (`scan_proof_block`) collects them there; reaching one here means it sits
        // outside any proof block.
        self.bump();
        if proof {
            return Err(SyntaxError::ProofHoleOutsideProofBlock { number, span });
        }
        if self.fn_body_depth == 0 {
            return Err(SyntaxError::HoleOutsideFnBody { number, span });
        }
        self.pending_holes.push(Hole {
            number,
            span,
            context: HoleContext::Body,
        });
        Ok(())
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
        // #31, the construct the #30 fix introduced).
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
        // is a cycle that, like the if-tail cycle (#31), never saw the #29
        // expr-only guard. Guarding this re-entry caps deep loop nesting to a
        // structured diagnostic instead of a native stack overflow (parser.md
        // AC-4; #32, the last unguarded block-nesting vector).
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
        let dec = self.parse_dec_clause()?;
        if self.check(&TokKind::Dec) {
            // A second `dec` violates the exactly-one cardinality.
            return Err(SyntaxError::ClauseOrder {
                item: "loop".to_string(),
                clause: "dec".to_string(),
                span: self.peek_span(),
            });
        }

        // Enter the loop body at depth+1 so a `break;`/`continue;` anywhere
        // inside it (including nested `if` blocks, depth stays > 0) is accepted
        // (parser.md REQ-10, #93). A nested loop bumps the depth again; the
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

    /// Comparison is non-associative (surface-grammar.md): at most one CmpOp.
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
    /// comparison so `s is Circle` reads as one operand. The validator rule
    /// (accept only a declared variant of the scrutinee's enum) is stage 1b.
    fn parse_is(&mut self) -> PResult<Expr> {
        // OQ-3 (parser.md): `is` sits just below comparison and above the #92
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
    /// joins two operands here; a `|` that opens a closure is recognized only in
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
    /// prefix reference `&`/`&mut` is parsed in `parse_ref` (one operand),
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

    /// Tier 3 `<<` `>>` — shifts (#92), below `+ -`. Partial: an unbounded shift
    /// amount fails the §7 shift-bound obligation at L3 (ast.md REQ-11), but the
    /// parser builds the `Binary` node unconditionally (parser.md REQ-9).
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
                // tier 1). Partial: a zero divisor fails the §7 obligation at L3
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
    /// unambiguously the unary operator; `!=` is the distinct maximal-munch
    /// `TokKind::Ne` token (parser.md REQ-8). The one `UnaryOp::Not` is built
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
            // `Expr::Deref` unary (no existing node fits; `Ref` is its inverse).
            // Semantics are stage 1c; surface-only here.
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
                // A postfix `.` is suppressed (treated as a terminator) while
                // parsing a quantifier's `<dom>` so the domain/body separator `.`
                // is unambiguous (REQ-0). Brackets re-enable it (`with_struct_literal`).
                TokKind::Dot if self.no_dot => break,
                TokKind::Dot => {
                    self.bump();
                    // A numeric projection `e.0`/`e.1`/…
                    // (`.design/basis/10-recursion-tuples.md` REQ-5/REQ-8, OQ-1
                    // resolved to a dedicated `Expr::TupleProj { receiver, index }`,
                    // not an overloaded `Expr::Field` with a string `"0"` name: a
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

    /// Parse a raw quantified formula `forall (x : S) in <dom>. φ` / `exists (x :
    /// S) in <dom>. φ` (`.design/stage2-stratified-cage.md` REQ-0): the surface
    /// binder production over a named sorted carrier the (R2) index grammar admits.
    /// The keyword (`forall`/`exists`) is at the cursor.
    ///
    /// Grammar: `QUANT '(' IDENT ':' IDENT ')' 'in' <dom> '.' <expr>`. `in` is a
    /// contextual identifier (mirroring the C10 `for … in` loop precedent,
    /// `parse_for`), not a reserved keyword. The `<dom>` is parsed with the postfix
    /// `.` suppressed (`with_no_dot`) so the `.` introducing the body is
    /// unambiguous; the body is a full greedy `parse_expr` (lowest precedence). The
    /// parser builds the node unconditionally — well-sortedness of `S`/`<dom>` and
    /// fragment admission are the stratified classifier's job (REQ-4), not the
    /// parser's (registry-free, parse-only, like the `forall_in` combinator path).
    fn parse_quantifier(&mut self) -> PResult<Expr> {
        let quant = match self.peek() {
            TokKind::Forall => Quant::Forall,
            TokKind::Exists => Quant::Exists,
            // `parse_primary` only dispatches here on `Forall`/`Exists`.
            _ => return Err(self.unexpected("`forall` or `exists`")),
        };
        self.bump(); // consume the quantifier keyword
        self.consume(&TokKind::LParen, "`(` to open the quantifier binder")?;
        let var = self.take_ident("a bound variable name")?;
        self.consume(
            &TokKind::Colon,
            "`:` between the bound variable and its sort",
        )?;
        let sort = self.take_ident("a sort name")?;
        self.consume(&TokKind::RParen, "`)` to close the quantifier binder")?;
        // `in` — a contextual identifier (the for-loop precedent), not a keyword.
        let in_kw = self.take_ident("`in` after the quantifier binder")?;
        if in_kw != "in" {
            return Err(self.unexpected("`in` after the quantifier binder"));
        }
        // The domain ranges to the body-separating `.`; suppress the postfix `.`
        // so that separator is unambiguous (bracketed sub-exprs re-enable it).
        let domain = self.with_no_dot(Self::parse_expr)?;
        self.consume(
            &TokKind::Dot,
            "`.` separating the quantifier domain from its body",
        )?;
        let body = self.parse_expr()?;
        Ok(Expr::Quantifier {
            quant,
            var,
            sort,
            domain: Box::new(domain),
            body: Box::new(body),
        })
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
            TokKind::LBracket => self.parse_array_expr(),
            // A string literal `"hello"` as a primary expression
            // (`.design/basis/07-strings.md` REQ-1). The literal lexes today
            // (`TokKind::Str(String)`); this arm accepts it as an `Expr::StrLit`,
            // mirroring the `IntLit`/`BoolLit` value-carrying literal precedent.
            // The token's existing `parse_slag`/`parse_attribute` consumers (the
            // `#[slag(reason = "…")]` / `#[boundary("…")]` field values) are
            // unchanged: those read the token directly via `take_string`, never
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
            // A raw quantified formula `forall (x : S) in <dom>. φ` / `exists …`
            // (`.design/stage2-stratified-cage.md` REQ-0). Recognized as a primary
            // (like `if`/`match`) so a quantifier may appear in any operand position;
            // its body extends greedily to the right (lowest precedence).
            TokKind::Forall | TokKind::Exists => self.parse_quantifier(),
            TokKind::LParen => {
                self.bump();
                // A parenthesised group re-enables struct literals (REQ-2):
                // `(s is Circle)` / `(A { x: 1 })`. The same `(` opens an n-tuple
                // construction `(a, b, …)`
                // (`.design/basis/10-recursion-tuples.md` REQ-5/REQ-7): the parser
                // distinguishes by the comma: `()` → unit (the empty group; no
                // tuple expr, mirroring `Type::Unit`), `(e)` → grouping (the inner
                // expr, arity 1), `(a, b, …)` → `Expr::Tuple` (arity ≥ 2).
                self.with_struct_literal(|p| {
                    if p.check(&TokKind::RParen) {
                        // Arity 0: the empty group `()`, the unit value. There is
                        // no `Expr::Unit` node; v1 surfaces unit only as a return
                        // type (`Type::Unit`), so a literal `()` value is not a
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

    /// Parse `[]`, `[a, b, ...]`, or the allocation-free repeat form
    /// `[value; N]`. Array patterns use their separate pattern parser.
    fn parse_array_expr(&mut self) -> PResult<Expr> {
        self.consume(&TokKind::LBracket, "`[` to open an array expression")?;
        if self.eat(&TokKind::RBracket) {
            return Ok(Expr::Array(Vec::new()));
        }
        self.with_struct_literal(|parser| {
            let first = parser.parse_expr()?;
            if parser.eat(&TokKind::Semi) {
                let len = parser.parse_array_len()?;
                parser.consume(&TokKind::RBracket, "`]` to close `[value; N]`")?;
                return Ok(Expr::ArrayRepeat {
                    value: Box::new(first),
                    len,
                });
            }
            let mut values = vec![first];
            while parser.eat(&TokKind::Comma) {
                if parser.check(&TokKind::RBracket) {
                    break;
                }
                values.push(parser.parse_expr()?);
            }
            parser.consume(&TokKind::RBracket, "`]` to close the array literal")?;
            Ok(Expr::Array(values))
        })
    }

    /// Parse a path expression `Ident (:: Ident)*` (`lo`, `u32::MAX`, `Some`),
    /// or a struct-literal `Path { field: val, … }` when a `{` follows and the
    /// struct-literal context is enabled (`.design/basis/01-adts.md` REQ-2).
    /// `::` is a path separator, never method dispatch (REQ-6).
    fn parse_path_expr(&mut self) -> PResult<Expr> {
        let mut segments = vec![self.take_ident("a path")?];
        while self.eat(&TokKind::ColonCol) {
            segments.push(self.take_ident("a path segment")?);
        }
        // A `Path { … }` is a struct / struct-variant construction (REQ-2),
        // except in a no-struct-literal head (`match s { … }`), where the `{`
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
            // mirroring the `if`/`while`/`match`-head rule. A guarded arm does not
            // complete a match (the validator's exhaustiveness check, REQ-3).
            let guard = if self.eat(&TokKind::If) {
                Some(self.with_no_struct_literal(Self::parse_expr)?)
            } else {
                None
            };
            self.consume(&TokKind::FatArrow, "`=>`")?;
            // An arm body is in value position, so a struct-literal construction
            // (`Point { x: 1 }`) must parse here even when the `match` sits under
            // an enclosing no-struct-literal head (a contract clause / `match`
            // scrutinee). Re-enable struct literals as `parse_call_args`
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
        // flat `Pattern::Or` (a single alternative stays the bare pattern; no
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
            TokKind::LBracket => {
                self.bump();
                let elem = self.parse_type()?;
                self.consume(
                    &TokKind::Semi,
                    "`;` between array element type and capacity",
                )?;
                let len = self.parse_array_len()?;
                self.consume(&TokKind::RBracket, "`]` to close the array type")?;
                Ok(Type::Array {
                    elem: Box::new(elem),
                    len,
                })
            }
            // `()` is the one sanctioned unit-type spelling (surface-grammar.md
            // decision 4 / REQ-8): written explicitly in a return position. The
            // same `(` opens an n-tuple type `(T, U, …)`
            // (`.design/basis/10-recursion-tuples.md` REQ-5/REQ-7): the parser
            // distinguishes by the comma: `()` → `Type::Unit` (arity 0), `(T)`
            // → grouping (the inner type, arity 1), `(T, U, …)` → `Type::Tuple`
            // (arity ≥ 2).
            TokKind::LParen => {
                self.bump();
                if self.check(&TokKind::RParen) {
                    // Arity 0: `()` is the unit type (unchanged).
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
                    "u8" => Ok(Type::Prim(PrimType::U8)),
                    "u16" => Ok(Type::Prim(PrimType::U16)),
                    "u32" => Ok(Type::Prim(PrimType::U32)),
                    "u64" => Ok(Type::Prim(PrimType::U64)),
                    "usize" => Ok(Type::Prim(PrimType::Usize)),
                    "bool" => Ok(Type::Prim(PrimType::Bool)),
                    // The heap-indirection primitive `Box<T>`
                    // (`.design/basis/01-adts.md` REQ-3, OQ-1 resolved: a
                    // dedicated `Type::Box` node). `Box` is a contextual
                    // identifier (not a reserved keyword), matched here by name.
                    "Box" => {
                        self.consume(&TokKind::Lt, "`<` after `Box`")?;
                        let inner = self.parse_type()?;
                        self.consume(&TokKind::Gt, "`>` to close `Box<…>`")?;
                        Ok(Type::Box(Box::new(inner)))
                    }
                    // The bounded growable-collection primitive `Vec<T>`
                    // (`.design/basis/04-collections.md` REQ-1, OQ-2 resolved: a
                    // dedicated `Type::Vec` node, mirroring `Box<T>`). `Vec` is a
                    // contextual identifier (not a reserved keyword), matched here
                    // by name as `Box` is. The element type `T` parses
                    // recursively; `Vec<u64>` (`conformance/vec_demo.th`) yields
                    // `Type::Vec(Box::new(Type::Prim(U64)))`. Its `push`/`pop`/
                    // `get`/`len` operations are ordinary `MethodCall`s parsed by
                    // the existing postfix `.` form (REQ-6); no new surface here.
                    "Vec" => {
                        self.consume(&TokKind::Lt, "`<` after `Vec`")?;
                        let inner = self.parse_type()?;
                        self.consume(&TokKind::Gt, "`>` to close `Vec<…>`")?;
                        Ok(Type::Vec(Box::new(inner)))
                    }
                    // The bounded owned-text primitive `String`
                    // (`.design/basis/07-strings.md` REQ-2, OQ-3 resolved: a
                    // dedicated nullary `Type::String` node with no `<T>` argument,
                    // unlike `Vec<T>`, because the element type is fixed to `u8`
                    // (the char model is bytes for v1). `String` is a contextual
                    // identifier (not a reserved keyword), matched here by name
                    // as `Box`/`Vec` are. The borrowed `str`-view is
                    // `&String` (`Ref { inner: String }`), parsed by the `&` arm
                    // above. `String`'s `len`/`byte_at`/`slice`/`concat` ops are
                    // ordinary `MethodCall`s (the existing postfix `.` form); no
                    // new surface; `==`/`+` are the existing `Binary` ops.
                    "String" => Ok(Type::String),
                    // The built-in optional primitive `Option<T>`
                    // (`.design/basis/09-option-result.md` REQ-1, OQ-1 resolved: a
                    // dedicated `Type::Option` node, mirroring `Box<T>`/`Vec<T>`).
                    // `Option` stops being a string-named `Generic` so the
                    // lowerer/validator key on the node kind. `Option` is a
                    // contextual ident (not a reserved keyword), matched here by
                    // name as `Box`/`Vec` are. `Some(v)`/`None`/`match`/`is`
                    // reuse the existing `Call`/`Path`/`Match`/`Is` nodes.
                    "Option" => {
                        self.consume(&TokKind::Lt, "`<` after `Option`")?;
                        let inner = self.parse_type()?;
                        self.consume(&TokKind::Gt, "`>` to close `Option<…>`")?;
                        Ok(Type::Option(Box::new(inner)))
                    }
                    // The built-in fallible primitive `Result<T, E>`
                    // (`.design/basis/09-option-result.md` REQ-2, OQ-1 resolved: a
                    // dedicated two-type-argument node, the first two-arg type in
                    // the grammar, the parser change of C7). The
                    // single-arg `Generic { name, arg }` dies at the comma; this arm
                    // parses `<T, E>` (a comma + a second type + `>`). `Result` is a
                    // contextual ident matched by name as `Box`/`Vec`/
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
                    // (`.design/basis/13-map.md` REQ-1, C12: the second
                    // two-type-argument node, mirroring `Result<T, E>`:
                    // the single-arg `Generic { name, arg }` cannot carry a key and
                    // a value (it dies at the comma, the C7 finding). `Map` is
                    // a contextual ident matched by name as `Box`/`Vec`/
                    // `Option`/`Result`. The key `K` and value `V` parse recursively;
                    // `Map<u64, u64>` yields `Type::Map(Box::new(u64), Box::new(u64))`.
                    // Its `insert`/`get`/`contains_key`/`len` ops are ordinary
                    // `MethodCall`s (the existing postfix `.` form); no new surface.
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

    fn parse_array_len(&mut self) -> PResult<ArrayLen> {
        match self.peek().clone() {
            TokKind::Int { value, raw } => {
                self.bump();
                Ok(ArrayLen::Literal { value, raw })
            }
            TokKind::Ident(name) => {
                self.bump();
                Ok(ArrayLen::Const(name))
            }
            _ => Err(self.unexpected("an integer literal or capacity constant name")),
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
        TokKind::Hole { number, proof } => {
            if *proof {
                format!("proof hole `?p{number}`")
            } else {
                format!("hole `?{number}`")
            }
        }
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
        TokKind::Forall => "forall",
        TokKind::Exists => "exists",
        TokKind::HashBracket => "#[",
        TokKind::Hash => "#",
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
        TokKind::At => "@",
        TokKind::Ident(_)
        | TokKind::Int { .. }
        | TokKind::Bool(_)
        | TokKind::Str(_)
        // A `?N`/`?pN` hole has no fixed surface text (the number varies);
        // `describe` formats it dynamically (#193 / forge-tier REQ-3). It is listed
        // here only to keep this match exhaustive without a `_` wildcard (R-APG-1).
        | TokKind::Hole { .. }
        | TokKind::Eof => "<token>",
    }
}
