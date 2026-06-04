//! Second re-audit of `thermite-syntax` deep-recursion coverage (issue #3,
//! post-#31). The #31 fix (commit f0ceb12) generalised the #29 expr-only guard
//! into one shared `guard_recursion`/`MAX_RECURSION_DEPTH` counter and routed
//! FOUR recursive families through it: `parse_expr`, `parse_type`,
//! `parse_pattern`, and the `parse_if_parts` if-tail cycle.
//!
//! This file probes the recursive cycles the #31 fix did NOT route through the
//! guard. The suspect is the `loop`/`while` body cycle:
//!
//!     parse_block  --(Loop/While arm)-->  parse_loop  --(body)-->  parse_block
//!
//! NEITHER `parse_block` NOR `parse_loop` calls `guard_recursion`, so a deeply
//! nested `loop { loop { loop { ... } } }` (each loop carries the mandatory
//! `inv true dec 0` so the body is well-formed v0.1 grammar) drives unbounded
//! native recursion `parse_block`<->`parse_loop` and overflows the C stack
//! (SIGABRT) — exactly the AC-4 failure mode #29/#31 claimed to close, on a
//! cycle neither fix instrumented.
//!
//! Authority: `.design/syntax/parser.md` AC-4 ("No input ... causes a panic;
//! all failures surface as `SyntaxError` diagnostics in the returned
//! structure") + REQ-4 (no panic); `surface-grammar.md` REQ-3 / EBNF
//! `LoopExpr ::= 'loop' InvClause+ DecClause Block` and `Stmt ::= ... | Loop`
//! — a `loop`/`while` body is a `Block`, and a `Block` may contain a nested
//! `loop`/`while` statement, so arbitrarily nested loops are WELL-FORMED v0.1
//! grammar. `goal.md` R-CODE-2 (no panic in production).
//!
//! Method (R-CHAR-3): expected behaviour is "control returns; no process
//! abort", traced to parser.md AC-4 — never copied from parser output. Each
//! probe runs `parse` on a 2 MiB stack (the Rust test-thread default the #29/#31
//! fixes were calibrated against — commits 2a8e3e3, f0ceb12) in a child thread.
//! A clean return (the guard fires -> `SyntaxError::ExpressionTooDeep`, or the
//! input parses) joins `Ok`; a native stack overflow aborts the process and
//! joins `Err`. That abort IS the AC-4 violation.
//!
//! `tests/` is not gated, so `unwrap`/`expect` are fine here.

use thermite_syntax::parse;

/// Depth used by every probe. 1500 matches the depth the #29/#31 oracle
/// (`divergence_reaudit.rs`) uses for the paths those fixes DID cap; these
/// loop/while-body paths must cap too.
const DEPTH: usize = 1500;

/// Run `parse` on a 2 MiB stack in a child thread and report whether control
/// returned (matches `divergence_reaudit.rs::parse_returns_on_bounded_stack`).
/// A return (`Ok`) satisfies parser.md AC-4 regardless of accept/reject; a
/// native stack overflow aborts the thread/process and joins as `Err`.
fn parse_returns_on_bounded_stack(src: String) -> bool {
    let handle = std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(move || {
            let r = parse(&src);
            r.is_clean() || !r.errors.is_empty()
        })
        .expect("spawn probe thread");
    handle.join().is_ok()
}

/// Wrap a function body around `inner`. The function itself is well-formed:
/// `fn f() -> u32 req true ens true fx pure { <inner> }`.
fn in_fn(inner: &str) -> String {
    format!("fn f() -> u32 req true ens true fx pure {{ {inner} }}")
}

/// D2-R1 — Deeply nested `loop { ... }` bodies overflow the native stack.
///
/// `parse_block` dispatches a `loop`/`while` statement to `parse_loop`, whose
/// body is parsed by `parse_block` again — a `parse_block`<->`parse_loop` cycle
/// that routes through NEITHER `guard_recursion` entry point (the #31 fix
/// guarded expr/type/pattern/if-tail, not this loop-body cycle). DEPTH nested
/// `loop`s (each `loop inv true dec 0 { ... }`) therefore recurse unbounded.
///
/// Authority: `surface-grammar.md` REQ-3 / EBNF `LoopExpr ::= 'loop'
/// InvClause+ DecClause Block` (legal v0.1) + parser.md AC-4 (no input aborts
/// the process).
///
/// Tracking: #32
#[test]
fn divergence_deep_nested_loop_no_panic() {
    // Build `loop inv true dec 0 { loop inv true dec 0 { ... 0 } }`.
    let mut body = String::from("0");
    for _ in 0..DEPTH {
        body = format!("loop inv true dec 0 {{ {body} }}");
    }
    let src = in_fn(&body);
    assert!(
        parse_returns_on_bounded_stack(src),
        "parser must return (accept or SyntaxError), never abort, on deeply \
         nested `loop` bodies (parser.md AC-4)"
    );
}

/// D2-R2 — Deeply nested `while c { ... }` bodies overflow the native stack.
///
/// Same `parse_block`<->`parse_loop` cycle as D2-R1, via the `while` arm. The
/// `while` condition routes through the (guarded) `parse_expr`, but the
/// BODY-cycle re-entry (`parse_loop` -> `parse_block` -> `parse_loop`) does not,
/// so the guard caps the condition expression, never the loop nesting.
///
/// Authority: `surface-grammar.md` REQ-3 / EBNF `WhileExpr ::= 'while' Expr
/// InvClause+ DecClause Block` + parser.md AC-4.
///
/// Tracking: #32
#[test]
fn divergence_deep_nested_while_no_panic() {
    let mut body = String::from("0");
    for _ in 0..DEPTH {
        body = format!("while true inv true dec 0 {{ {body} }}");
    }
    let src = in_fn(&body);
    assert!(
        parse_returns_on_bounded_stack(src),
        "parser must return (accept or SyntaxError), never abort, on deeply \
         nested `while` bodies (parser.md AC-4)"
    );
}

/// CONTROL — Deeply nested STATEMENT-form `if` is already bounded.
///
/// Statement `if a { if a { ... } }` re-enters `parse_block` -> `parse_if_parts`
/// (GUARDED by the #31 fix) -> `parse_block`, so this path SHOULD return a
/// `SyntaxError::ExpressionTooDeep` rather than abort. This test is expected to
/// PASS on f0ceb12 (it confirms the statement-if path the task asked to verify
/// is covered, distinct from the unguarded loop path above). It is NOT
/// `#[ignore]`d: if it ever fails, the if-statement path regressed.
///
/// Authority: `surface-grammar.md` EBNF `IfStmt ::= 'if' Expr Block
/// ('else' Block)?` + parser.md AC-4. Expected: control returns (guard fires).
#[test]
fn control_deep_nested_if_stmt_returns() {
    // `if true { if true { ... { 0 } } }` — each inner `if` is the only stmt of
    // its parent's then-block, with a trailing `0` tail so the body is valid.
    let mut body = String::from("0");
    for _ in 0..DEPTH {
        body = format!("if true {{ {body} }} 0");
    }
    let src = in_fn(&body);
    assert!(
        parse_returns_on_bounded_stack(src),
        "statement-`if` nesting routes through the guarded parse_if_parts and \
         must return (parser.md AC-4)"
    );
}
