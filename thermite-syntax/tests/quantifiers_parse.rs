//! Tests for the Stage-2 raw quantifier binder productions `forall (x : S) in
//! <dom>. φ` / `exists (x : S) in <dom>. φ` (`.design/stage2-stratified-cage.md`
//! REQ-0, AC-0): the surface binder grammar over a named sorted carrier the (R2)
//! index grammar admits, plus the binder/scope corner-case pins.
//!
//! REQ-0 is the foundation increment blocking REQ-1 (the Lean `Strat/Syntax`
//! denote path) and REQ-4 (the Rust classifier). It is distinct from
//! the registry-free `forall_in`/`forall_below`/`forall_from`/`sorted` COMBINATOR
//! calls, which stay ordinary `Expr::Call` nodes — the combinator registry is
//! untouched (the registry-unchanged pins below assert this). R-CHAR-3:
//! shapes hand-derived from the grammar; `tests/` is ungated.

use thermite_syntax::{parse, BinOp, Expr, ForgeItem, Item, Quant};

/// Parse `src` and return the body tail `Expr` of the single `prop fn`
/// (its `{ <expr> }` body is a bool-valued expression — a convenient quantifier
/// carrier needing no contract scaffolding).
fn prop_body(src: &str) -> Expr {
    let result = parse(src);
    assert!(
        result.is_clean(),
        "expected a clean parse of {src:?}, got: {:?}",
        result.errors
    );
    match result.program.items.into_iter().next().unwrap() {
        Item::Forge(ForgeItem::PropFn(p)) => {
            *p.body.tail.expect("prop fn body has a tail expression")
        }
        other => panic!("expected a prop fn, got {other:?}"),
    }
}

/// Parse `src` and return the `req` clause of the single `fn` (both its
/// parsed `expr` and its verbatim `text`, for the round-trip assertion).
fn fn_req(src: &str) -> (Expr, String) {
    let result = parse(src);
    assert!(
        result.is_clean(),
        "expected a clean parse of {src:?}, got: {:?}",
        result.errors
    );
    match result.program.items.into_iter().next().unwrap() {
        Item::Fn(f) => (f.contract.req.expr, f.contract.req.text),
        other => panic!("expected a fn, got {other:?}"),
    }
}

/// Wrap a contract expression `e` (bool-valued) as the `req` of a minimal exec
/// `fn` (the `fn f(...) -> u64 req <e> ens result == 0 fx pure { 0 }` scaffold).
fn fn_with_req(req: &str) -> String {
    format!("fn f(xs: Vec<u64>) -> u64 req {req} ens result == 0 fx pure {{ 0 }}")
}

/// Wrap a bool expression `e` as the body of a minimal `prop fn`.
fn prop_with_body(body: &str) -> String {
    format!("prop fn p(xs: Vec<u64>) -> bool {{ {body} }}")
}

// ---- the AST node: forall / exists parse into Expr::Quantifier --------------

#[test]
fn forall_parses_into_the_quantifier_node() {
    let e = prop_body(&prop_with_body("forall (i : Idx) in xs. xs[i] != needle"));
    match e {
        Expr::Quantifier {
            quant,
            var,
            sort,
            domain,
            body,
        } => {
            assert_eq!(quant, Quant::Forall);
            assert_eq!(var, "i");
            assert_eq!(sort, "Idx");
            // The domain is the bare carrier `xs`.
            assert!(matches!(domain.as_ref(), Expr::Path(segs) if segs == &vec!["xs".to_string()]));
            // The body is the comparison `xs[i] != needle`.
            assert!(matches!(body.as_ref(), Expr::Binary { op: BinOp::Ne, .. }));
        }
        other => panic!("expected Expr::Quantifier, got {other:?}"),
    }
}

#[test]
fn exists_parses_into_the_quantifier_node() {
    let e = prop_body(&prop_with_body("exists (j : Jdx) in ys. ys[j] == needle"));
    match e {
        Expr::Quantifier {
            quant, var, sort, ..
        } => {
            assert_eq!(quant, Quant::Exists);
            assert_eq!(var, "j");
            assert_eq!(sort, "Jdx");
        }
        other => panic!("expected Expr::Quantifier, got {other:?}"),
    }
}

// ---- round-trip: the verbatim clause text is preserved ----------------------

#[test]
fn quantifier_req_clause_preserves_verbatim_text() {
    // The `req` clause captures the exact surface span of the quantifier — the
    // round-trip oracle string `address.rs` resolves against (AC-0).
    let src = fn_with_req("forall (i : Idx) in xs. xs[i] != 0");
    let (expr, text) = fn_req(&src);
    assert!(matches!(
        expr,
        Expr::Quantifier {
            quant: Quant::Forall,
            ..
        }
    ));
    assert_eq!(text, "forall (i : Idx) in xs. xs[i] != 0");
}

// ---- binder/scope pins ------------------------------------------------------

#[test]
fn body_is_greedy_lowest_precedence() {
    // `forall …. a && b` reads the whole `a && b` as the body, not just `a`
    // (the binder body extends greedily to the right).
    let e = prop_body(&prop_with_body(
        "forall (i : Idx) in xs. xs[i] != 0 && xs[i] != 1",
    ));
    match e {
        Expr::Quantifier { body, .. } => assert!(
            matches!(body.as_ref(), Expr::Binary { op: BinOp::And, .. }),
            "the body should be the full `&&` conjunction, got {body:?}"
        ),
        other => panic!("expected Expr::Quantifier, got {other:?}"),
    }
}

#[test]
fn parentheses_bound_the_body() {
    // `(forall …. a) && b` — the parens bound the quantifier body so the top node
    // is the `&&`, whose LHS is the quantifier.
    let e = prop_body(&prop_with_body(
        "(forall (i : Idx) in xs. xs[i] != 0) && true",
    ));
    match e {
        Expr::Binary {
            op: BinOp::And,
            lhs,
            ..
        } => assert!(
            matches!(lhs.as_ref(), Expr::Quantifier { .. }),
            "the `&&` LHS should be the bounded quantifier, got {lhs:?}"
        ),
        other => panic!("expected a top-level `&&`, got {other:?}"),
    }
}

#[test]
fn quantifier_is_an_operand_in_any_position() {
    // A quantifier is a primary (like `if`/`match`), so it parses as the RHS of a
    // binary operator too: `p || forall …. q`.
    let e = prop_body(&prop_with_body(
        "false || forall (i : Idx) in xs. xs[i] != 0",
    ));
    match e {
        Expr::Binary {
            op: BinOp::Or, rhs, ..
        } => assert!(
            matches!(
                rhs.as_ref(),
                Expr::Quantifier {
                    quant: Quant::Forall,
                    ..
                }
            ),
            "the `||` RHS should be the quantifier, got {rhs:?}"
        ),
        other => panic!("expected a top-level `||`, got {other:?}"),
    }
}

#[test]
fn quantifiers_nest_in_the_body() {
    // `forall …. exists …. φ` — the outer body is the inner quantifier.
    let e = prop_body(&prop_with_body(
        "forall (i : Idx) in xs. exists (j : Idx) in xs. xs[i] == xs[j]",
    ));
    match e {
        Expr::Quantifier {
            quant: Quant::Forall,
            body,
            ..
        } => assert!(
            matches!(
                body.as_ref(),
                Expr::Quantifier {
                    quant: Quant::Exists,
                    ..
                }
            ),
            "the outer body should be the inner `exists`, got {body:?}"
        ),
        other => panic!("expected an outer `forall`, got {other:?}"),
    }
}

#[test]
fn domain_may_be_an_indexed_slice_before_the_dot_separator() {
    // `in xs[..n].` — the domain is an index expression; the `.` after the `]`
    // (not a field access) separates the domain from the body. The `[..n]` slice
    // uses `..` (DotDot), distinct from the body-separator `.` (Dot).
    let e = prop_body(&prop_with_body("forall (i : Idx) in xs[..3]. xs[i] != 0"));
    match e {
        Expr::Quantifier { domain, body, .. } => {
            assert!(
                matches!(domain.as_ref(), Expr::Index { .. }),
                "the domain should be the indexed slice, got {domain:?}"
            );
            assert!(matches!(body.as_ref(), Expr::Binary { op: BinOp::Ne, .. }));
        }
        other => panic!("expected Expr::Quantifier, got {other:?}"),
    }
}

#[test]
fn a_parenthesized_domain_reenables_field_dots() {
    // Inside a parenthesised domain the postfix `.` is RE-ENABLED, so `(a.b)` is a
    // field access and the first `.` after the `)` is the body separator.
    let e = prop_body(&prop_with_body("forall (i : Idx) in (xs.foo). xs[i] != 0"));
    match e {
        Expr::Quantifier { domain, body, .. } => {
            assert!(
                matches!(domain.as_ref(), Expr::Field { name, .. } if name == "foo"),
                "the parenthesised domain should be the field access `xs.foo`, got {domain:?}"
            );
            assert!(matches!(body.as_ref(), Expr::Binary { op: BinOp::Ne, .. }));
        }
        other => panic!("expected Expr::Quantifier, got {other:?}"),
    }
}

// ---- registry-unchanged pins: the combinator idents stay Expr::Call ---------

#[test]
fn forall_in_combinator_stays_a_plain_call_not_a_binder() {
    // `forall_in` is a distinct identifier from the `forall` keyword (the lexer
    // keys on the full word), so the registry-free combinator call is unchanged:
    // it parses as an ordinary `Expr::Call`, not an `Expr::Quantifier`.
    let e = prop_body(&prop_with_body("forall_in(xs, |x| x != 0)"));
    match e {
        Expr::Call { callee, args } => {
            assert!(
                matches!(callee.as_ref(), Expr::Path(segs) if segs == &vec!["forall_in".to_string()])
            );
            assert_eq!(args.len(), 2);
        }
        other => panic!("expected forall_in to stay an Expr::Call, got {other:?}"),
    }
}

#[test]
fn sorted_and_forall_below_combinators_are_unchanged() {
    // `sorted` is still usable as a plain combinator/fn ident (it even names a
    // `prop fn` elsewhere in the corpus), and `forall_below` stays a call.
    assert!(matches!(
        prop_body(&prop_with_body("sorted(xs)")),
        Expr::Call { .. }
    ));
    assert!(matches!(
        prop_body(&prop_with_body("forall_below(xs, 2, |x| x != 0)")),
        Expr::Call { .. }
    ));
}

// ---- error pins: malformed binders are structured errors, never panics ------

#[test]
fn missing_binder_parens_is_a_structured_error() {
    // `forall i in xs. …` (no `( : )` binder) — a structured parse error, not a
    // panic; the parser never panics (parser.md REQ-4).
    let result = parse(&prop_with_body("forall i in xs. xs[i] != 0"));
    assert!(
        !result.is_clean(),
        "a quantifier missing its `( x : S )` binder must not parse cleanly"
    );
}

#[test]
fn missing_sort_annotation_is_a_structured_error() {
    // `forall (i) in xs. …` — the `: S` sort annotation is mandatory.
    let result = parse(&prop_with_body("forall (i) in xs. xs[i] != 0"));
    assert!(
        !result.is_clean(),
        "a binder missing its `: S` sort must not parse cleanly"
    );
}

#[test]
fn missing_in_keyword_is_a_structured_error() {
    // `forall (i : Idx) xs. …` — the contextual `in` is mandatory.
    let result = parse(&prop_with_body("forall (i : Idx) xs. xs[i] != 0"));
    assert!(
        !result.is_clean(),
        "a binder missing `in` must not parse cleanly"
    );
}

#[test]
fn missing_body_separator_dot_is_a_structured_error() {
    // `forall (i : Idx) in xs xs[i] != 0` — the `.` separating domain from body is
    // mandatory (the domain `xs xs[i]` then has no separator).
    let result = parse(&prop_with_body("forall (i : Idx) in xs xs[i] != 0"));
    assert!(
        !result.is_clean(),
        "a quantifier missing the `.` body separator must not parse cleanly"
    );
}
