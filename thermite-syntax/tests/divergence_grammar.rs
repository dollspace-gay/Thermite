//! Adversarial divergence tests for `thermite-syntax` (issue #3 audit).
//!
//! Each test pins a divergence between the parser and its authority
//! (`.design/syntax/surface-grammar.md`, `parser.md`, `ast.md`,
//! `thermite-design.md` §4). Expected behaviour traces to the design docs /
//! the grammar EBNF, NEVER copied from the parser's own output (goal.md
//! R-CHAR-3). These probe constructs the conformance corpus does NOT exercise.
//!
//! Each test is `#[ignore]`d with its tracking blocker: the divergence is
//! tracked, and the fixer un-`#[ignore]`s + greens the test (goal.md R-DEFER-3).
//!
//! `tests/` is not gated, so `unwrap`/`expect` are fine here.

use thermite_syntax::ast::{Block, Expr, Item};
use thermite_syntax::parse;

/// D1 — Unit return type `()` is rejected.
///
/// Authority: `.design/syntax/surface-grammar.md` "Key design decisions" #4 —
/// "`()` return type is written explicitly (§4.4 'All conversions explicit'
/// register); the grammar requires `-> Type`. No implicit unit return in a
/// signature." The EBNF `RetType ::= '->' Type ; '()' written explicitly if
/// unit` makes `-> ()` a legal signature. REQ-8 (type grammar) governs `Type`.
///
/// Per §4.4 ("one way to do everything") the ONLY way to spell a unit-returning
/// function is `-> ()`, so the parser MUST accept it. The corpus functions all
/// return non-unit, so this construct is unexercised by the oracle.
///
/// Tracking: #28
#[test]
fn divergence_unit_return_type_accepted() {
    let src = "fn f(x: u32) -> () req true ens result == result fx pure { }";
    let r = parse(src);
    assert!(
        r.is_clean(),
        "design: `-> ()` is the explicit unit return spelling (surface-grammar.md \
         decision 4); parser rejected it with {:?}",
        r.errors
    );
    assert_eq!(
        r.program.items.len(),
        1,
        "the unit-returning fn should parse to exactly one Item"
    );
}

/// D2 — Deeply nested expressions abort the process with a stack overflow.
///
/// Authority: `.design/syntax/parser.md` REQ-4 ("No `unwrap`/`expect`/`panic!`
/// in production ... the parser ... never panics") and AC-4 ("No input —
/// including the negative and recovery fixtures — causes a panic; all failures
/// surface as `SyntaxError` diagnostics in the returned structure"). Also
/// `goal.md` R-CODE-2.
///
/// A hand-written recursive-descent parser with no recursion-depth guard
/// overflows its stack on deeply nested input. `parse` MUST return — either
/// accepting the (well-formed) deeply nested expression or surfacing a
/// `SyntaxError` — but it MUST NOT abort the process (SIGABRT). This test simply
/// requires `parse` to RETURN. If the parser overflows, the test process aborts
/// and the test is recorded as failed.
///
/// NOTE: while failing, this test aborts the whole `divergence_grammar` binary
/// (SIGABRT), which is itself the demonstration of the no-panic violation.
///
/// Tracking: #29
#[test]
#[ignore = "divergence: deep-nesting stack overflow; tracking #29 (depth guard at parse_expr entry is bypassed by the paren-grouping re-entry path)"]
fn divergence_deep_nesting_no_panic() {
    // 1500 balanced parens around `x`: a deeply nested but otherwise well-formed
    // grouping expression. Far below any token-count limit; the failure mode is
    // pure C-stack recursion depth in the precedence ladder.
    let depth = 1500;
    let src = format!(
        "fn f(x: u32) -> u32 req true ens result == result fx pure {{ {}x{} }}",
        "(".repeat(depth),
        ")".repeat(depth)
    );
    // The only assertion that matters: control returns here. Reaching this line
    // means no panic/abort occurred (parser.md AC-4).
    let r = parse(&src);
    // Whatever the verdict (accept or structured error), it must be observable.
    assert!(
        r.is_clean() || !r.errors.is_empty(),
        "parse must yield a defined result, never abort (parser.md AC-4)"
    );
}

/// D3 — An `if/else` in block-tail (value) position is dropped to a `Stmt::If`,
/// leaving the block with NO tail value.
///
/// Authority: `.design/syntax/surface-grammar.md` `Block ::= '{' Stmt* TailExpr?
/// '}'` with `TailExpr ::= Expr` and `IfExpr` as a `Primary`/`Expr` (decision 2:
/// "`if` is both a statement and an expression ... The expression form requires
/// an `else` (it must have a value)"). `.design/syntax/ast.md` REQ-6 lists
/// `If { cond, then, else_ }` as an EXPRESSION node distinct from the REQ-4
/// statement-form `If`. Therefore, when an `if EXPR { } else { }` is the sole
/// value of a `-> T` function body (no trailing `;`), it is the block's TAIL
/// EXPRESSION: `Block.tail == Some(Expr::If { .. })`, and `Block.stmts` is empty.
///
/// The corpus only uses `if` in statement position (`if lo == hi { return None;
/// }`), so this value-position case is unexercised by the oracle.
///
/// Tracking: #30
#[test]
fn divergence_if_else_in_tail_position_is_expr() {
    let src = "fn f(x: u32) -> u32 req true ens result == x fx pure { if x == 0 { 1 } else { 2 } }";
    let r = parse(src);
    assert!(
        r.is_clean(),
        "the fn should parse cleanly, got {:?}",
        r.errors
    );
    let Item::Fn(f) = &r.program.items[0] else {
        panic!("expected a Fn item");
    };
    let Block { stmts, tail } = &f.body;
    // The if/else is the block's VALUE: it must be the tail Expr::If, not a
    // statement (surface-grammar.md Block grammar + ast.md REQ-6).
    assert!(
        matches!(tail.as_deref(), Some(Expr::If { .. })),
        "if/else in value position must be the block tail `Expr::If`; \
         got tail={tail:?}, stmts={stmts:?}"
    );
    assert!(
        stmts.is_empty(),
        "the if/else is the value, so there should be no statements; got {stmts:?}"
    );
}
