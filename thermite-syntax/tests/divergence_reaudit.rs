//! Re-audit divergence tests for `thermite-syntax` (issue #3, post-#28/#29/#30).
//!
//! The prior audit pinned #28 (unit return), #29 (deep-nesting overflow), and
//! #30 (if-expr tail) in `divergence_grammar.rs`. Those fixes landed (5bb910e,
//! 2a8e3e3). This file pins divergences the #29 fix did NOT cover (tracking #31).
//!
//! `#29` bounded recursion with a single `expr_depth` counter incremented in
//! `parse_expr` (the precedence-ladder entry). But several grammar productions
//! recurse WITHOUT routing through `parse_expr`, so the guard never sees them
//! and deeply nested input still overflows the native C stack and aborts the
//! process (SIGABRT) — exactly the failure mode #29 claimed to close.
//!
//! Authority: `.design/syntax/parser.md` REQ-4 ("No `unwrap`/`expect`/`panic!`
//! in production ... the parser ... never panics") and AC-4 ("No input ...
//! causes a panic; all failures surface as `SyntaxError` diagnostics in the
//! returned structure"); `goal.md` R-CODE-2. The grammar productions probed
//! here (`Type ::= Ident '<' Type '>'`, `Pattern ::= '[' SlicePat* ']'` and
//! `Path '(' Pattern* ')'`, and `IfExpr` as a block tail per
//! `surface-grammar.md` decision 2) are all WELL-FORMED v0.1 grammar: the
//! parser must either accept or return a `SyntaxError`, never abort.
//!
//! Method (R-CHAR-3): expected behaviour is "control returns; no process
//! abort", traced to parser.md AC-4 — never copied from parser output. Each
//! probe runs on a 2 MiB stack (the Rust test-thread default the #29 fix was
//! tuned against; commit 2a8e3e3 message) in a child thread; under the current
//! parser the overflow aborts the process (Rust routes stack overflow to
//! `abort()`, so `join` cannot recover it) — that abort IS the AC-4 violation.
//! Each test is `#[ignore]`d (tracked #31) so the abort does not break CI; the
//! fixer un-`#[ignore]`s and greens them (goal.md R-DEFER-3).
//!
//! `tests/` is not gated, so `unwrap`/`expect` are fine here.

use thermite_syntax::parse;

/// Depth used by every probe. 1500 is the same depth the #29 oracle
/// (`divergence_grammar.rs::divergence_deep_nesting_no_panic`) uses for the
/// paren path, which the fix DID cap. These non-`parse_expr` paths must cap too.
const DEPTH: usize = 1500;

/// Run `parse` on a 2 MiB stack in a child thread and report whether control
/// returned. The 2 MiB stack matches the Rust default test-thread budget the
/// #29 fix calibrated `MAX_EXPR_DEPTH = 64` against (commit 2a8e3e3). A return
/// (`Ok`) satisfies parser.md AC-4 regardless of accept/reject; an overflow
/// aborts the thread (and, in Rust, the process) and joins as `Err`.
fn parse_returns_on_bounded_stack(src: String) -> bool {
    let handle = std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(move || {
            let r = parse(&src);
            // Force the result to be observed; the verdict (clean vs errors) is
            // irrelevant to AC-4 — only that control reached here.
            r.is_clean() || !r.errors.is_empty()
        })
        .expect("spawn probe thread");
    handle.join().is_ok()
}

/// D-R1 — Deeply nested GENERIC TYPE (`Option<Option<...<u32>...>>`) overflows.
///
/// `parse_type` recurses on itself for `Name<T>` (the `Generic` arm) with NO
/// `expr_depth` guard — the #29 counter lives only in `parse_expr`. A 1500-deep
/// `Option<...>` type therefore drives unbounded native recursion and SIGABRTs.
///
/// Authority: `surface-grammar.md` REQ-8 / EBNF `Type ::= Ident '<' Type '>'`
/// (a legal v0.1 type form) + parser.md AC-4 (no input aborts the process).
///
/// Tracking: #31
#[test]
fn divergence_deep_generic_type_no_panic() {
    let mut g = String::from("u32");
    for _ in 0..DEPTH {
        g = format!("Option<{g}>");
    }
    let src = format!(
        "fn f(x: u32) -> u32 req true ens result == result fx pure {{ let y: {g} = x; x }}"
    );
    assert!(
        parse_returns_on_bounded_stack(src),
        "deeply nested generic type must return a result, never abort the \
         process (parser.md AC-4); parse_type recursion is unguarded by the \
         #29 depth bound"
    );
}

/// D-R2 — Deeply nested SLICE PATTERN (`[[[ ... ]]]`) overflows.
///
/// `parse_slice_pattern` → `parse_pattern` → `parse_slice_pattern` recurses with
/// NO depth guard (the #29 counter is in `parse_expr`, never reached on the
/// pattern path). A 1500-deep slice pattern SIGABRTs.
///
/// Authority: `surface-grammar.md` REQ-7 / EBNF `Pattern ::= '[' (SlicePat ...)?
/// ']'` (a legal v0.1 pattern) + parser.md AC-4.
///
/// Tracking: #31
#[test]
fn divergence_deep_slice_pattern_no_panic() {
    let mut p = String::from("_");
    for _ in 0..DEPTH {
        p = format!("[{p}]");
    }
    let src = format!(
        "fn f(x: u32) -> u32 req true ens result == result fx pure {{ match x {{ {p} => 0 }} }}"
    );
    assert!(
        parse_returns_on_bounded_stack(src),
        "deeply nested slice pattern must return a result, never abort the \
         process (parser.md AC-4); parse_pattern/parse_slice_pattern recursion \
         is unguarded by the #29 depth bound"
    );
}

/// D-R3 — Deeply nested ENUM/TUPLE-STRUCT PATTERN (`Some(Some(...))`) overflows.
///
/// `parse_path_pattern` → `parse_pattern` → `parse_path_pattern` recurses with
/// NO depth guard. A 1500-deep `Some(...)` pattern SIGABRTs.
///
/// Authority: `surface-grammar.md` REQ-7 / EBNF `Pattern ::= Path '(' Pattern
/// (',' Pattern)* ')'` (the `Some(i)` form, a legal v0.1 pattern) + parser.md
/// AC-4.
///
/// Tracking: #31
#[test]
fn divergence_deep_enum_pattern_no_panic() {
    let mut p = String::from("x");
    for _ in 0..DEPTH {
        p = format!("Some({p})");
    }
    let src = format!(
        "fn f(x: u32) -> u32 req true ens result == result fx pure {{ match x {{ {p} => 0 }} }}"
    );
    assert!(
        parse_returns_on_bounded_stack(src),
        "deeply nested enum/tuple-struct pattern must return a result, never \
         abort the process (parser.md AC-4); parse_path_pattern recursion is \
         unguarded by the #29 depth bound"
    );
}

/// D-R4 — Deeply nested IF/ELSE in block-TAIL position overflows.
///
/// The #30 fix routes an `if/else` block tail through `parse_if_parts` (in
/// `parse_block`), which calls `parse_block` for the branches; their tails
/// re-enter the `If`-token arm of `parse_block` → `parse_if_parts` again. This
/// `parse_block`/`parse_if_parts` cycle never increments `expr_depth` (only the
/// `if` CONDITION goes through `parse_expr`), so a 1500-deep nest of
/// `if x == 0 { <nest> } else { 0 }` as the function-body tail drives unbounded
/// native recursion and SIGABRTs.
///
/// Authority: `surface-grammar.md` decision 2 (`if` is a value/tail expression
/// with `else`) + ast.md REQ-6 (`Expr::If`) — a legal v0.1 construct — and
/// parser.md AC-4 (no input aborts the process). This is the very tail-position
/// `if` the #30 fix introduced support for.
///
/// Tracking: #31
#[test]
fn divergence_deep_if_tail_no_panic() {
    let mut s = String::from("x");
    for _ in 0..DEPTH {
        s = format!("if x == 0 {{ {s} }} else {{ 0 }}");
    }
    let src = format!("fn f(x: u32) -> u32 req true ens result == result fx pure {{ {s} }}");
    assert!(
        parse_returns_on_bounded_stack(src),
        "deeply nested tail-position if/else must return a result, never abort \
         the process (parser.md AC-4); the parse_block/parse_if_parts cycle is \
         unguarded by the #29 depth bound"
    );
}
