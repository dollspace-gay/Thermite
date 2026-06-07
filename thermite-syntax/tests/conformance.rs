//! Conformance tests for `thermite-syntax` against the hand-authored, read-only
//! oracle fixtures under `conformance/` (R-CHAR-3 — the fixtures are the truth;
//! this crate is the artifact under test and must MATCH them, never the
//! reverse).
//!
//! Covers the parser oracle (`conformance/parse/*.facts.json`,
//! `.design/syntax/parser.md` AC-1..AC-4) and the address oracle
//! (`conformance/address/*.addresses.json`,
//! `.design/syntax/semantic-addressing.md` AC-1..AC-4). `tests/` is not gated,
//! so `unwrap`/`expect` are fine here.

use serde::Deserialize;
use std::path::PathBuf;

use thermite_syntax::ast::{Item, Param, Stmt, Type};
use thermite_syntax::{addresses_of, parse, resolve, AddressError, EffectRow, PrimType};

// ---------------------------------------------------------------------------
// Fixture loading helpers
// ---------------------------------------------------------------------------

fn conformance_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR == <workspace>/thermite-syntax; the corpus is a sibling.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .join("conformance")
}

fn read(rel: &str) -> String {
    let path = conformance_dir().join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn read_json<T: for<'de> Deserialize<'de>>(rel: &str) -> T {
    let text = read(rel);
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse json {rel}: {e}"))
}

// ---------------------------------------------------------------------------
// Parse-facts oracle (conformance/parse/<name>.facts.json)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ParamFact {
    name: String,
    #[serde(rename = "type")]
    ty: String,
}

#[derive(Deserialize)]
struct LoopFact {
    addr: String,
    surface_keyword: String,
    inv_count: usize,
    has_dec: bool,
}

#[derive(Deserialize)]
struct ItemFact {
    name: String,
    kind: String,
    params: Vec<ParamFact>,
    ret: String,
    #[serde(default)]
    req_count: Option<usize>,
    #[serde(default)]
    ens_count: Option<usize>,
    #[serde(default)]
    fx: Option<String>,
    #[serde(default)]
    has_dec: Option<bool>,
    #[serde(default)]
    loops: Vec<LoopFact>,
}

#[derive(Deserialize)]
struct ParseFacts {
    source: String,
    parses_ok: bool,
    error_count: usize,
    items: Vec<ItemFact>,
}

/// Render a `Type` to the exact surface spelling the fixtures use.
fn render_type(ty: &Type) -> String {
    match ty {
        Type::Prim(PrimType::U32) => "u32".to_string(),
        Type::Prim(PrimType::U64) => "u64".to_string(),
        Type::Prim(PrimType::Usize) => "usize".to_string(),
        Type::Prim(PrimType::Bool) => "bool".to_string(),
        Type::Ref { mutable, inner } => {
            let m = if *mutable { "mut " } else { "" };
            format!("&{m}{}", render_type(inner))
        }
        Type::Slice(inner) => format!("[{}]", render_type(inner)),
        Type::Generic { name, arg } => format!("{name}<{}>", render_type(arg)),
        Type::Unit => "()".to_string(),
        // Basis Stage 1a ADT type nodes (`.design/basis/01-adts.md` REQ-1/REQ-2/
        // REQ-3): a bare user type name and the `Box<T>` recursive-occurrence
        // primitive. Additive arms so this existing test helper compiles; the
        // existing `sum`/`binary_search` fixtures never exercise them.
        Type::Named(name) => name.clone(),
        Type::Box(inner) => format!("Box<{}>", render_type(inner)),
        // Basis Stage 4 bounded-collection type node
        // (`.design/basis/04-collections.md` REQ-1): the `Vec<T>` surface
        // rendering. Additive arm so this existing test helper compiles; the
        // sum/binary_search fixtures never exercise it.
        Type::Vec(inner) => format!("Vec<{}>", render_type(inner)),
        // Basis Stage 7 bounded owned-text type node
        // (`.design/basis/07-strings.md` REQ-2): the `String` surface rendering.
        // Additive arm so this existing test helper compiles; the
        // sum/binary_search fixtures never exercise it.
        Type::String => "String".to_string(),
        // Cluster C7 built-in Option/Result type nodes
        // (`.design/basis/09-option-result.md` REQ-1/REQ-2): the surface rendering.
        // Additive arms so this existing test helper compiles; the
        // sum/binary_search fixtures never exercise them.
        Type::Option(inner) => format!("Option<{}>", render_type(inner)),
        Type::Result(ok, err) => {
            format!("Result<{}, {}>", render_type(ok), render_type(err))
        }
        // Cluster C9-B n-tuple type node (`.design/basis/10-recursion-tuples.md`
        // REQ-5/REQ-7): the `(T, U, …)` surface rendering. Additive arm so this
        // existing test helper compiles; the sum/binary_search fixtures never
        // exercise it.
        Type::Tuple(tys) => {
            let parts: Vec<String> = tys.iter().map(render_type).collect();
            format!("({})", parts.join(", "))
        }
    }
}

fn check_params(actual: &[Param], expected: &[ParamFact]) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "param count mismatch (got {}, want {})",
        actual.len(),
        expected.len()
    );
    for (a, e) in actual.iter().zip(expected) {
        assert_eq!(a.name, e.name, "param name mismatch");
        assert_eq!(
            render_type(&a.ty),
            e.ty,
            "param type mismatch for {}",
            e.name
        );
    }
}

/// Collect every loop statement in a block (recursing into if-branches), in
/// source order — the structural order addressing relies on.
fn collect_loops<'a>(
    block: &'a thermite_syntax::Block,
    out: &mut Vec<&'a thermite_syntax::LoopNode>,
) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Loop(lp) => {
                out.push(lp);
                collect_loops(&lp.body, out);
            }
            Stmt::If { then, else_, .. } => {
                collect_loops(then, out);
                if let Some(eb) = else_ {
                    collect_loops(eb, out);
                }
            }
            _ => {}
        }
    }
}

fn check_parse_facts(facts_file: &str) {
    let facts: ParseFacts = read_json(facts_file);
    // `source` is repo-relative (e.g. "conformance/sum.th"); strip the prefix.
    let rel = facts
        .source
        .strip_prefix("conformance/")
        .unwrap_or(&facts.source);
    let src = read(rel);
    let result = parse(&src);

    assert_eq!(
        result.is_clean(),
        facts.parses_ok,
        "{}: parses_ok mismatch (errors: {:?})",
        facts.source,
        result.errors
    );
    assert_eq!(
        result.errors.len(),
        facts.error_count,
        "{}: error_count mismatch (errors: {:?})",
        facts.source,
        result.errors
    );
    assert_eq!(
        result.program.items.len(),
        facts.items.len(),
        "{}: item count mismatch",
        facts.source
    );

    for (item, fact) in result.program.items.iter().zip(&facts.items) {
        assert_eq!(item.name(), fact.name, "item name mismatch");
        match item {
            Item::Fn(f) => {
                assert_eq!(fact.kind, "fn", "{} expected `fn`", fact.name);
                check_params(&f.params, &fact.params);
                assert_eq!(render_type(&f.ret), fact.ret, "{} ret mismatch", fact.name);
                // Mandatory-clause counts (ast.md REQ-2): req is always exactly 1.
                assert_eq!(Some(1), fact.req_count, "{} req_count", fact.name);
                assert_eq!(
                    Some(f.contract.ens.len()),
                    fact.ens_count,
                    "{} ens_count",
                    fact.name
                );
                let fx_str = match f.contract.fx {
                    EffectRow::Pure => "pure".to_string(),
                    EffectRow::Set(_) => "set".to_string(),
                };
                assert_eq!(
                    fact.fx.as_deref(),
                    Some(fx_str.as_str()),
                    "{} fx",
                    fact.name
                );

                // Loop facts. The corpus fns are in-language (bodied); a boundary
                // fn (#16) would carry `body: None` and no loops.
                let mut loops = Vec::new();
                if let Some(body) = &f.body {
                    collect_loops(body, &mut loops);
                }
                assert_eq!(
                    loops.len(),
                    fact.loops.len(),
                    "{} loop count mismatch",
                    fact.name
                );
                for (lp, lf) in loops.iter().zip(&fact.loops) {
                    assert_eq!(lp.kind.surface_keyword(), lf.surface_keyword, "loop kw");
                    assert_eq!(lp.invs.len(), lf.inv_count, "inv_count");
                    assert!(lf.has_dec, "fixture {} loop should have dec", lf.addr);
                    // The addr column is a structural fact we validate via the
                    // address oracle; here we just confirm it is `loop#N`.
                    assert!(lf.addr.starts_with("loop#"), "loop addr shape");
                }
            }
            Item::SpecFn(s) => {
                assert_eq!(fact.kind, "spec fn", "{} expected `spec fn`", fact.name);
                check_params(&s.params, &fact.params);
                assert_eq!(render_type(&s.ret), fact.ret, "{} ret mismatch", fact.name);
                assert_eq!(Some(true), fact.has_dec, "{} has_dec", fact.name);
            }
            // The existing `sum`/`binary_search` fixtures contain only `fn`/
            // `spec fn`; the basis ADT item kinds (`.design/basis/01-adts.md`)
            // never appear here. Additive arm so this exhaustive `match`
            // compiles; ADT items are asserted by `tests/adt_parse.rs`.
            Item::Struct(_) | Item::Enum(_) => {
                panic!(
                    "{}: unexpected ADT item in the non-ADT corpus fixture",
                    fact.name
                )
            }
        }
    }
}

#[test]
fn sum_parse_facts() {
    check_parse_facts("parse/sum.facts.json");
}

#[test]
fn binary_search_parse_facts() {
    check_parse_facts("parse/binary_search.facts.json");
}

// ---------------------------------------------------------------------------
// Per-item recovery oracle (conformance/parse/recover_per_item.facts.json)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ErrorFact {
    item: String,
    kind: String,
    clause: String,
}

#[derive(Deserialize)]
struct RecoverFacts {
    parses_ok: bool,
    min_error_count: usize,
    errors: Vec<ErrorFact>,
    recovered_items: Vec<String>,
    recovered_item_facts: Vec<ItemFact>,
}

#[test]
fn recover_per_item() {
    let facts: RecoverFacts = read_json("parse/recover_per_item.facts.json");
    let src = read("parse/recover_per_item.th");
    let result = parse(&src);

    // The first item `broken` omits the mandatory `ens` clause -> parse error.
    assert_eq!(result.is_clean(), facts.parses_ok, "should NOT be clean");
    assert!(
        result.errors.len() >= facts.min_error_count,
        "expected >= {} errors, got {}: {:?}",
        facts.min_error_count,
        result.errors.len(),
        result.errors
    );

    // The missing-`ens` error must be reported.
    let want = &facts.errors[0];
    assert_eq!(want.kind, "missing-mandatory-clause");
    assert_eq!(want.clause, "ens");
    let found_missing_ens = result.errors.iter().any(|e| {
        matches!(
            e,
            thermite_syntax::SyntaxError::MissingClause { clause, item, .. }
                if clause == "ens" && item == &want.item
        )
    });
    assert!(
        found_missing_ens,
        "expected a missing-`ens` diagnostic for `{}`, got {:?}",
        want.item, result.errors
    );

    // Recovery: the well-formed second item `ok` still parses to its AST node.
    let names: Vec<&str> = result.program.items.iter().map(|i| i.name()).collect();
    for recovered in &facts.recovered_items {
        assert!(
            names.contains(&recovered.as_str()),
            "recovery failed: `{recovered}` not in recovered items {names:?}"
        );
    }

    // And it parses to the correct shape (REQ-3 / AC-3).
    for fact in &facts.recovered_item_facts {
        let item = result
            .program
            .items
            .iter()
            .find(|i| i.name() == fact.name)
            .unwrap_or_else(|| panic!("recovered item `{}` missing", fact.name));
        match item {
            Item::Fn(f) => {
                check_params(&f.params, &fact.params);
                assert_eq!(render_type(&f.ret), fact.ret);
                assert_eq!(Some(f.contract.ens.len()), fact.ens_count);
            }
            Item::SpecFn(_) => panic!("`ok` should be a fn"),
            // The recovery fixture's recovered item is a `fn`; ADT item kinds
            // (`.design/basis/01-adts.md`) do not appear. Additive arm so this
            // exhaustive `match` compiles.
            Item::Struct(_) | Item::Enum(_) => panic!("`ok` should be a fn, not an ADT item"),
        }
    }
}

// ---------------------------------------------------------------------------
// Address oracle (conformance/address/<name>.addresses.json)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct AddrFact {
    addr: String,
    #[serde(default)]
    text: Option<String>,
}

#[derive(Deserialize)]
struct AddressOracle {
    source: String,
    addresses: Vec<AddrFact>,
    must_error: Vec<String>,
}

fn check_addresses(oracle_file: &str) {
    let oracle: AddressOracle = read_json(oracle_file);
    let rel = oracle
        .source
        .strip_prefix("conformance/")
        .unwrap_or(&oracle.source);
    let src = read(rel);
    let result = parse(&src);
    assert!(
        result.is_clean(),
        "{}: should parse clean, got {:?}",
        oracle.source,
        result.errors
    );

    let computed = addresses_of(&result.program);
    let computed_addrs: Vec<&str> = computed.iter().map(|e| e.addr.as_str()).collect();
    let expected_addrs: Vec<&str> = oracle.addresses.iter().map(|f| f.addr.as_str()).collect();
    assert_eq!(
        computed_addrs, expected_addrs,
        "{}: address list mismatch",
        oracle.source
    );

    // Every `inv`/`dec` address must resolve to the verbatim oracle text, AND
    // resolution must be the inverse of computation (AC-4).
    for fact in &oracle.addresses {
        let entry = resolve(&result.program, &fact.addr)
            .unwrap_or_else(|e| panic!("{} should resolve: {e}", fact.addr));
        assert_eq!(entry.addr, fact.addr, "round-trip addr mismatch");
        if let Some(want_text) = &fact.text {
            assert_eq!(
                entry.text.as_deref(),
                Some(want_text.as_str()),
                "{}: clause text mismatch",
                fact.addr
            );
        }
    }

    // Every must_error address resolves to a structured error, never a panic.
    for bad in &oracle.must_error {
        match resolve(&result.program, bad) {
            Err(AddressError::NotFound(_)) | Err(AddressError::Malformed(_)) => {}
            Ok(entry) => panic!("`{bad}` should error, resolved to {entry:?}"),
        }
    }
}

#[test]
fn sum_addresses() {
    check_addresses("address/sum.addresses.json");
}

#[test]
fn binary_search_addresses() {
    check_addresses("address/binary_search.addresses.json");
}

// ---------------------------------------------------------------------------
// Stability under unrelated edits (semantic-addressing.md AC-3 / REQ-5)
// ---------------------------------------------------------------------------

#[test]
fn address_stability_under_unrelated_edit() {
    // Two functions in one file: editing/deleting one must not renumber the
    // other's blocks, because numbering reads only the enclosing item (REQ-5).
    let both = format!("{}\n{}", read("binary_search.th"), read("sum.th"));
    let with_both = parse(&both);
    assert!(
        with_both.is_clean(),
        "combined parse: {:?}",
        with_both.errors
    );

    let only_bs = parse(&read("binary_search.th"));
    assert!(only_bs.is_clean());

    let addrs_in_both: Vec<String> = addresses_of(&with_both.program)
        .into_iter()
        .filter(|e| e.addr.starts_with("binary_search"))
        .map(|e| e.addr)
        .collect();
    let addrs_alone: Vec<String> = addresses_of(&only_bs.program)
        .into_iter()
        .map(|e| e.addr)
        .collect();
    assert_eq!(
        addrs_in_both, addrs_alone,
        "binary_search addresses changed when `sum` was present (REQ-5 violated)"
    );

    // The inv#2 text is stable regardless of sibling presence.
    let inv2 = resolve(&with_both.program, "binary_search.loop#1.inv#2").unwrap();
    assert_eq!(
        inv2.text.as_deref(),
        Some("forall_below(haystack, lo, |x| x < needle)")
    );
}

// ---------------------------------------------------------------------------
// No-panic sweep + lexer value checks (lexer.md AC-2, AC-6)
// ---------------------------------------------------------------------------

#[test]
fn int_literal_underscores_strip_to_value() {
    // lexer.md AC-2: `1_000_000` lexes to the numeric value 1000000 (UNCHANGED).
    let (tokens, errors) = thermite_syntax::tokenize("1_000_000");
    assert!(errors.is_empty());
    let has_value = tokens.iter().any(|t| {
        matches!(
            t.kind,
            thermite_syntax::TokKind::Int {
                value: 1_000_000,
                ..
            }
        )
    });
    assert!(has_value, "expected Int value 1000000, got {tokens:?}");
}

#[test]
fn int_literal_preserves_raw() {
    // lexer.md AC-2b (#37): the integer token carries BOTH the numeric `value`
    // (separators stripped) AND the verbatim `raw` (separators included). The
    // expected raw is the source substring, hand-derived from the input, never
    // copied from the lexer (R-CHAR-3).
    use thermite_syntax::TokKind;

    let (tokens, errors) = thermite_syntax::tokenize("1_000_000");
    assert!(errors.is_empty());
    let int = tokens
        .iter()
        .find_map(|t| match &t.kind {
            TokKind::Int { value, raw } => Some((*value, raw.clone())),
            _ => None,
        })
        .expect("expected an Int token");
    assert_eq!(int, (1_000_000u128, "1_000_000".to_string()));

    // A literal with no separators: raw == "42".
    let (tokens, _) = thermite_syntax::tokenize("42");
    let int = tokens.iter().find_map(|t| match &t.kind {
        TokKind::Int { value, raw } => Some((*value, raw.clone())),
        _ => None,
    });
    assert_eq!(int, Some((42u128, "42".to_string())));

    // A trailing `_` (e.g. `1_`) is in NEITHER value nor raw: value 1, raw "1".
    let (tokens, _) = thermite_syntax::tokenize("1_");
    let int = tokens.iter().find_map(|t| match &t.kind {
        TokKind::Int { value, raw } => Some((*value, raw.clone())),
        _ => None,
    });
    assert_eq!(int, Some((1u128, "1".to_string())));
}

#[test]
fn int_literal_preserves_value_and_raw() {
    // ast.md AC-1b (#37): the `1_000_000` literal parses to an EXPR-level
    // `Expr::IntLit { value: 1000000, raw: "1_000_000" }` — BOTH the numeric
    // value (separators stripped, UNCHANGED) AND the verbatim raw. Expected is
    // hand-derived from the source, never copied from the parser (R-CHAR-3).
    use thermite_syntax::ast::{BinOp, Expr};

    let src = "fn f(xs: &[u32]) -> u32 req xs.len() <= 1_000_000 ens result == 0 fx pure { 0 }";
    let result = parse(src);
    assert!(result.is_clean(), "fixture should parse clean: {result:?}");

    let Item::Fn(f) = &result.program.items[0] else {
        panic!("expected a fn item");
    };
    // req is `xs.len() <= 1_000_000`: a Binary whose rhs is the IntLit.
    let Expr::Binary {
        rhs, op: BinOp::Le, ..
    } = &f.contract.req.expr
    else {
        panic!("expected a `<=` Binary req, got {:?}", f.contract.req.expr);
    };
    match rhs.as_ref() {
        Expr::IntLit { value, raw } => {
            assert_eq!(*value, 1_000_000u128, "value: separators stripped");
            assert_eq!(raw, "1_000_000", "raw: verbatim separators preserved");
        }
        other => panic!("expected an IntLit rhs, got {other:?}"),
    }

    // A separator-free literal `42` round-trips as `{ value: 42, raw: "42" }`.
    let src2 = "fn g() -> u32 req true ens result == 42 fx pure { 0 }";
    let result2 = parse(src2);
    assert!(result2.is_clean());
    let Item::Fn(g) = &result2.program.items[0] else {
        panic!("expected a fn item");
    };
    let Expr::Binary { rhs, .. } = &g.contract.ens[0].expr else {
        panic!("expected a Binary ens");
    };
    match rhs.as_ref() {
        Expr::IntLit { value, raw } => {
            assert_eq!(*value, 42u128);
            assert_eq!(raw, "42");
        }
        other => panic!("expected an IntLit, got {other:?}"),
    }
}

#[test]
fn stray_char_is_diagnostic_not_panic() {
    // lexer.md AC-6: a stray `@` yields a diagnostic, never a panic.
    let result = parse("fn f(@) -> u32 req true ens result == 0 fx pure { 0 }");
    assert!(!result.is_clean(), "stray char should produce a diagnostic");
}

#[test]
fn negative_inputs_never_panic() {
    // parser.md AC-4: no input causes a panic.
    for src in [
        "",
        "fn",
        "fn f",
        "fn f(",
        "spec fn g() -> u32 { 0 }",             // missing dec
        "fn h() -> u32 fx pure { 0 }",          // missing req/ens
        "fn h() -> u32 req true fx pure { 0 }", // missing ens
        "loop inv true dec 0 {}",
        "match",
        "@#$%",
    ] {
        let _ = parse(src); // must not panic
    }
}
