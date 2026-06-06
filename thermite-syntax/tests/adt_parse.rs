//! Basis Stage 1a — ADT SURFACE parse oracle (`.design/basis/01-adts.md`
//! REQ-1/REQ-2/REQ-3/REQ-4 + REQ-6 SURFACE part). These tests assert the EXACT
//! AST shape the hand-derived parse oracle pins
//! (`conformance/parse/{bank_account,shape,list_sum}.facts.json`) — `struct`
//! items + the `inv` clause, `enum` items with tuple/struct/unit variants, the
//! recursive `Box<List>` self-reference (`Type::Box`), `match` over enum/struct
//! patterns with payload binding, the `Expr::StructLit` construction, the
//! `Expr::Is` variant-discriminator, and the boxed-tail deref `*t`
//! (`Expr::Deref`). Expected shapes are hand-derived from the design + facts
//! (R-CHAR-3), NEVER copied from the parser's output. SURFACE only — the
//! VALIDATOR (1b) and Verus LOWERING (1c) are not exercised here.

use std::path::PathBuf;
use thermite_syntax::{
    parse, Clause, EffectRow, Expr, Item, Pattern, PrimType, Type, VariantShape,
};

fn corpus(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("conformance")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

// ---- bank_account.th: struct + inv + StructLit + field access -------------

#[test]
fn bank_account_parses_struct_with_inv_and_struct_lit() {
    let r = parse(&corpus("bank_account.th"));
    assert!(
        r.is_clean(),
        "bank_account.th must parse clean, got {:?}",
        r.errors
    );
    assert_eq!(r.program.items.len(), 2, "two items: Account + deposit");

    // item[0] — `struct Account { balance: u64 } inv balance <= 1_000_000`.
    let s = match &r.program.items[0] {
        Item::Struct(s) => s,
        other => panic!("item[0] must be Item::Struct, got {other:?}"),
    };
    assert_eq!(s.name, "Account");
    assert_eq!(s.fields.len(), 1, "one field");
    assert_eq!(s.fields[0].name, "balance");
    assert_eq!(s.fields[0].ty, Type::Prim(PrimType::U64), "balance: u64");
    // The `inv` type-invariant clause is present with the verbatim text.
    let inv: &Clause = s
        .inv
        .as_ref()
        .expect("Account carries inv: Some(_) (REQ-1)");
    assert_eq!(
        inv.text, "balance <= 1_000_000",
        "inv text is the verbatim clause source"
    );

    // item[1] — `fn deposit` whose body constructs an Account StructLit and
    // field-accesses `a.balance`; req_count 1, ens_count 1, fx pure.
    let f = match &r.program.items[1] {
        Item::Fn(f) => f,
        other => panic!("item[1] must be Item::Fn, got {other:?}"),
    };
    assert_eq!(f.name, "deposit");
    assert_eq!(f.params.len(), 2, "params a, amount");
    assert_eq!(f.params[0].name, "a");
    assert_eq!(f.params[1].name, "amount");
    assert_eq!(f.contract.ens.len(), 1, "ens_count 1");
    assert_eq!(f.contract.fx, EffectRow::Pure, "fx pure");
    // `req` is a single non-empty clause (req_count 1 in the facts).
    assert_eq!(
        f.contract.req.text, "a.balance + amount <= 1_000_000",
        "the single req clause"
    );

    // The body tail constructs `Account { balance: a.balance + amount }`.
    let body = f.body.as_ref().expect("deposit has a body");
    let tail = body.tail.as_ref().expect("body has a tail expr");
    let (path, fields) = match tail.as_ref() {
        Expr::StructLit { path, fields } => (path, fields),
        other => panic!("body tail must be Expr::StructLit, got {other:?}"),
    };
    assert_eq!(path, &vec!["Account".to_string()], "constructs Account");
    assert_eq!(fields.len(), 1, "one initializer");
    assert_eq!(fields[0].0, "balance", "the `balance` initializer");
    // The initializer field-accesses `a.balance` (an Expr::Field).
    assert!(
        contains_field_access(&fields[0].1, "balance"),
        "the StructLit value field-accesses a.balance, got {:?}",
        fields[0].1
    );
}

/// True if `e` contains an `Expr::Field { name }` anywhere (the `a.balance`
/// access the facts pin).
fn contains_field_access(e: &Expr, field: &str) -> bool {
    match e {
        Expr::Field { receiver, name } => name == field || contains_field_access(receiver, field),
        Expr::Binary { lhs, rhs, .. } => {
            contains_field_access(lhs, field) || contains_field_access(rhs, field)
        }
        Expr::Deref(inner) => contains_field_access(inner, field),
        Expr::Is { scrutinee, .. } => contains_field_access(scrutinee, field),
        Expr::Cast { expr, .. } => contains_field_access(expr, field),
        _ => false,
    }
}

// ---- shape.th: enum (tuple + struct variants), is-expr, match binding ------

#[test]
fn shape_parses_enum_is_expr_and_match_arms() {
    let r = parse(&corpus("shape.th"));
    assert!(
        r.is_clean(),
        "shape.th must parse clean, got {:?}",
        r.errors
    );
    assert_eq!(r.program.items.len(), 2, "Shape + is_circle");

    // item[0] — enum Shape { Circle(u64), Rect { w: u64, h: u64 } }.
    let e = match &r.program.items[0] {
        Item::Enum(e) => e,
        other => panic!("item[0] must be Item::Enum, got {other:?}"),
    };
    assert_eq!(e.name, "Shape");
    assert_eq!(e.variants.len(), 2);
    assert_eq!(e.variants[0].name, "Circle");
    match &e.variants[0].shape {
        VariantShape::Tuple(tys) => {
            assert_eq!(tys, &vec![Type::Prim(PrimType::U64)], "Circle(u64)");
        }
        other => panic!("Circle must be a Tuple variant, got {other:?}"),
    }
    assert_eq!(e.variants[1].name, "Rect");
    match &e.variants[1].shape {
        VariantShape::Struct(fields) => {
            assert_eq!(fields.len(), 2, "Rect {{ w, h }}");
            assert_eq!(fields[0].name, "w");
            assert_eq!(fields[1].name, "h");
        }
        other => panic!("Rect must be a Struct variant, got {other:?}"),
    }

    // item[1] — fn is_circle: ens contains `s is Circle`; body match s has 2
    // arms binding `r` (Circle) and `w, h` (Rect).
    let f = match &r.program.items[1] {
        Item::Fn(f) => f,
        other => panic!("item[1] must be Item::Fn, got {other:?}"),
    };
    assert_eq!(f.name, "is_circle");
    assert_eq!(f.contract.ens.len(), 1, "ens_count 1");

    // The ens `result == (s is Circle)` contains an Expr::Is { scrutinee: s,
    // variant: Circle }.
    let is = find_is(&f.contract.ens[0].expr).expect("ens contains an Expr::Is (REQ-6 surface)");
    match is {
        Expr::Is { scrutinee, variant } => {
            assert_eq!(
                scrutinee.as_ref(),
                &Expr::Path(vec!["s".to_string()]),
                "is scrutinee is `s`"
            );
            assert_eq!(variant, &vec!["Circle".to_string()], "is variant is Circle");
        }
        _ => unreachable!(),
    }

    // The body match s has 2 arms; arm 0 binds `r` (Circle(r)), arm 1 binds
    // `w, h` (Rect { w, h }).
    let body = f.body.as_ref().expect("is_circle has a body");
    let arms = match body.tail.as_deref() {
        Some(Expr::Match { scrutinee, arms }) => {
            assert_eq!(
                scrutinee.as_ref(),
                &Expr::Path(vec!["s".to_string()]),
                "match scrutinee is `s`"
            );
            arms
        }
        other => panic!("body tail must be Expr::Match, got {other:?}"),
    };
    assert_eq!(arms.len(), 2, "two match arms (exhaustive over Shape)");
    // Arm 0: Circle(r) — an enum tuple pattern binding `r`.
    match &arms[0].pattern {
        Pattern::Enum { path, fields } => {
            assert_eq!(path, &vec!["Circle".to_string()]);
            assert_eq!(fields.len(), 1, "Circle(r) binds one");
            assert_eq!(fields[0], Pattern::Binding("r".to_string()));
        }
        other => panic!("arm[0] must be Pattern::Enum Circle(r), got {other:?}"),
    }
    // Arm 1: Rect { w, h } — a struct pattern binding `w` and `h`.
    match &arms[1].pattern {
        Pattern::Struct { path, fields, rest } => {
            assert_eq!(path, &vec!["Rect".to_string()]);
            assert!(!rest, "Rect {{ w, h }} has no `..` rest");
            assert_eq!(fields.len(), 2, "binds w and h");
            assert_eq!(fields[0].0, "w");
            assert_eq!(fields[0].1, Pattern::Binding("w".to_string()));
            assert_eq!(fields[1].0, "h");
            assert_eq!(fields[1].1, Pattern::Binding("h".to_string()));
        }
        other => panic!("arm[1] must be Pattern::Struct Rect {{ w, h }}, got {other:?}"),
    }
}

/// Find the first `Expr::Is` subexpression of `e` (the `s is Circle` inside
/// `result == (s is Circle)`).
fn find_is(e: &Expr) -> Option<&Expr> {
    match e {
        Expr::Is { .. } => Some(e),
        Expr::Binary { lhs, rhs, .. } => find_is(lhs).or_else(|| find_is(rhs)),
        Expr::Cast { expr, .. } | Expr::Deref(expr) => find_is(expr),
        _ => None,
    }
}

// ---- list_sum.th: recursive Box<List>, spec fn, dec, match + deref ---------

#[test]
fn list_sum_parses_recursive_box_enum_and_deref() {
    let r = parse(&corpus("list_sum.th"));
    assert!(
        r.is_clean(),
        "list_sum.th must parse clean, got {:?}",
        r.errors
    );
    assert_eq!(r.program.items.len(), 2, "List + sum_list");

    // item[0] — enum List { Nil, Cons(u64, Box<List>) }.
    let e = match &r.program.items[0] {
        Item::Enum(e) => e,
        other => panic!("item[0] must be Item::Enum, got {other:?}"),
    };
    assert_eq!(e.name, "List");
    assert_eq!(e.variants.len(), 2);
    assert_eq!(e.variants[0].name, "Nil");
    assert_eq!(
        e.variants[0].shape,
        VariantShape::Unit,
        "Nil is a unit variant"
    );
    assert_eq!(e.variants[1].name, "Cons");
    match &e.variants[1].shape {
        VariantShape::Tuple(tys) => {
            assert_eq!(tys.len(), 2, "Cons(u64, Box<List>)");
            assert_eq!(tys[0], Type::Prim(PrimType::U64), "first field u64");
            // The SELF-REFERENCE: Box<List> is a Type::Box wrapping the named
            // generic `List` (parsed as a Generic with no args -> here a bare
            // user type; the recursive occurrence is the Box node).
            match &tys[1] {
                Type::Box(inner) => {
                    // The recursive SELF-REFERENCE: Box<List> wraps the bare
                    // user type `List` (Type::Named), the recursive occurrence.
                    assert_eq!(
                        inner.as_ref(),
                        &Type::Named("List".to_string()),
                        "Cons's second field is Box<List> (self-ref)"
                    );
                }
                other => panic!("Cons's second field must be Type::Box, got {other:?}"),
            }
        }
        other => panic!("Cons must be a Tuple variant, got {other:?}"),
    }

    // item[1] — spec fn sum_list(l: List) -> u64 dec l, no contract clauses, a
    // 2-arm match l binding h,t, and the recursive call derefs t (`*t`).
    let sf = match &r.program.items[1] {
        Item::SpecFn(sf) => sf,
        other => panic!("item[1] must be Item::SpecFn, got {other:?}"),
    };
    assert_eq!(sf.name, "sum_list");
    assert_eq!(sf.params.len(), 1);
    assert_eq!(sf.params[0].name, "l");
    assert_eq!(sf.dec.text, "l", "dec l (the datatype value)");

    let arms = match sf.body.tail.as_deref() {
        Some(Expr::Match { scrutinee, arms }) => {
            assert_eq!(
                scrutinee.as_ref(),
                &Expr::Path(vec!["l".to_string()]),
                "match scrutinee is `l`"
            );
            arms
        }
        other => panic!("sum_list body tail must be Expr::Match, got {other:?}"),
    };
    assert_eq!(arms.len(), 2, "two arms: Nil, Cons(h, t)");
    // Arm 0: Nil — a zero-field enum pattern.
    match &arms[0].pattern {
        Pattern::Enum { path, fields } => {
            assert_eq!(path, &vec!["Nil".to_string()]);
            assert!(fields.is_empty(), "Nil binds nothing");
        }
        other => panic!("arm[0] must be Pattern::Enum Nil, got {other:?}"),
    }
    // Arm 1: Cons(h, t) — binds h and t.
    match &arms[1].pattern {
        Pattern::Enum { path, fields } => {
            assert_eq!(path, &vec!["Cons".to_string()]);
            assert_eq!(fields.len(), 2, "Cons(h, t)");
            assert_eq!(fields[0], Pattern::Binding("h".to_string()));
            assert_eq!(fields[1], Pattern::Binding("t".to_string()));
        }
        other => panic!("arm[1] must be Pattern::Enum Cons(h, t), got {other:?}"),
    }
    // The Cons arm body `h as u64 + sum_list(*t)` derefs the boxed tail: it
    // contains a `Call` whose arg is an `Expr::Deref(Path(["t"]))`.
    assert!(
        contains_deref_of(&arms[1].body, "t"),
        "the Cons arm's recursive call must deref `*t`, got {:?}",
        arms[1].body
    );
}

/// True if `e` contains an `Expr::Deref` of the path `name` (the `*t` deref of
/// the boxed tail in the recursive call).
fn contains_deref_of(e: &Expr, name: &str) -> bool {
    match e {
        Expr::Deref(inner) => {
            matches!(inner.as_ref(), Expr::Path(p) if p == &vec![name.to_string()])
                || contains_deref_of(inner, name)
        }
        Expr::Binary { lhs, rhs, .. } => {
            contains_deref_of(lhs, name) || contains_deref_of(rhs, name)
        }
        Expr::Call { callee, args } => {
            contains_deref_of(callee, name) || args.iter().any(|a| contains_deref_of(a, name))
        }
        Expr::Cast { expr, .. } => contains_deref_of(expr, name),
        Expr::Field { receiver, .. } => contains_deref_of(receiver, name),
        _ => false,
    }
}

// ---- regression: the existing corpus parses unchanged ----------------------

#[test]
fn existing_corpus_still_parses_clean() {
    for name in ["sum", "binary_search"] {
        let r = parse(&corpus(&format!("{name}.th")));
        assert!(
            r.is_clean(),
            "existing corpus `{name}.th` must still parse clean, got {:?}",
            r.errors
        );
        // None of the existing corpus items is a struct/enum (additive only).
        for item in &r.program.items {
            assert!(
                matches!(item, Item::Fn(_) | Item::SpecFn(_)),
                "existing `{name}` items stay Fn/SpecFn, got {item:?}"
            );
        }
    }
}
