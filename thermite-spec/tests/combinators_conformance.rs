//! Conformance test for `thermite-spec` against the hand-derived oracle at
//! `tests/golden/combinators/` (R-CHAR-3). The oracle is the external truth;
//! the registry + validator are the artifacts under test. This file never edits
//! the oracle — a failure here is a bug in `thermite-spec`, not the fixture.
//!
//! - AC-1: the crate's registry equals `registry.json` field-for-field.
//! - AC-2: every `accept.json` program parses then validates clean (`Ok`).
//! - AC-3: every `reject.json` program parses then validates to the expected
//!   `SpecError` cause.
//! - AC-4: deeply-nested + malformed contract expressions never panic; a deep
//!   nest yields `ExpressionTooDeep`.
//!
//! `unwrap`/`expect` are fine here — `tests/` is not anti-pattern-gated.

use std::path::PathBuf;

use serde::Deserialize;

use thermite_spec::{validate, ArgKind, ResultKind, SpecError};

// ---- oracle JSON shapes ----------------------------------------------------

#[derive(Debug, Deserialize)]
struct RegistryOracle {
    combinators: Vec<RegistryEntry>,
}

#[derive(Debug, Deserialize)]
struct RegistryEntry {
    name: String,
    arity: usize,
    arg_kinds: Vec<String>,
    result: String,
}

#[derive(Debug, Deserialize)]
struct AcceptOracle {
    cases: Vec<AcceptCase>,
}

#[derive(Debug, Deserialize)]
struct AcceptCase {
    name: String,
    program: String,
}

#[derive(Debug, Deserialize)]
struct RejectOracle {
    cases: Vec<RejectCase>,
}

#[derive(Debug, Deserialize)]
struct RejectCase {
    name: String,
    expected: String,
    program: String,
}

// ---- helpers ---------------------------------------------------------------

fn oracle_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR is thermite-spec/; the oracle is at the workspace root.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("tests")
        .join("golden")
        .join("combinators")
}

fn read_oracle(file: &str) -> String {
    let path = oracle_dir().join(file);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read oracle {}: {e}", path.display()))
}

/// Parse a program and assert it parsed with zero syntax errors (the oracle
/// guarantees each program PARSES — a parse failure means thermite-syntax broke,
/// not thermite-spec).
fn parse_clean(program: &str, case: &str) -> thermite_syntax::Program {
    let result = thermite_syntax::parse(program);
    assert!(
        result.errors.is_empty(),
        "oracle case `{case}` failed to PARSE (thermite-syntax errors): {:?}",
        result.errors
    );
    result.program
}

// ---- AC-1: registry matches the oracle -------------------------------------

#[test]
fn registry_matches_oracle() {
    let oracle: RegistryOracle =
        serde_json::from_str(&read_oracle("registry.json")).expect("registry.json parses");

    let actual = thermite_spec::all();
    assert_eq!(
        actual.len(),
        oracle.combinators.len(),
        "registry length differs from oracle: code has {}, oracle has {}",
        actual.len(),
        oracle.combinators.len()
    );

    for entry in &oracle.combinators {
        let sig = thermite_spec::lookup(&entry.name)
            .unwrap_or_else(|| panic!("registry is missing oracle combinator `{}`", entry.name));

        assert_eq!(
            sig.arity, entry.arity,
            "`{}` arity: code {} vs oracle {}",
            entry.name, sig.arity, entry.arity
        );

        let actual_kinds: Vec<String> = sig
            .arg_kinds
            .iter()
            .map(|k| arg_kind_name(*k).to_string())
            .collect();
        assert_eq!(
            actual_kinds, entry.arg_kinds,
            "`{}` arg_kinds: code {:?} vs oracle {:?}",
            entry.name, actual_kinds, entry.arg_kinds
        );

        assert_eq!(
            result_kind_name(sig.result),
            entry.result,
            "`{}` result: code {} vs oracle {}",
            entry.name,
            result_kind_name(sig.result),
            entry.result
        );
    }

    // And no EXTRA combinators in the code beyond the frozen oracle set.
    for sig in actual {
        assert!(
            oracle.combinators.iter().any(|e| e.name == sig.name),
            "registry has combinator `{}` absent from the frozen oracle",
            sig.name
        );
    }
}

fn arg_kind_name(k: ArgKind) -> &'static str {
    match k {
        ArgKind::Slice => "Slice",
        ArgKind::Index => "Index",
        ArgKind::Pred => "Pred",
        ArgKind::Value => "Value",
    }
}

fn result_kind_name(r: ResultKind) -> &'static str {
    match r {
        ResultKind::Bool => "Bool",
        ResultKind::Usize => "Usize",
    }
}

// ---- AC-2: accept cases validate clean -------------------------------------

#[test]
fn accept_cases_validate_clean() {
    let oracle: AcceptOracle =
        serde_json::from_str(&read_oracle("accept.json")).expect("accept.json parses");

    for case in &oracle.cases {
        let program = parse_clean(&case.program, &case.name);
        let result = validate(&program);
        assert!(
            result.is_ok(),
            "accept case `{}` should VALIDATE CLEAN but got {:?}",
            case.name,
            result.unwrap_err()
        );
    }
}

// ---- AC-3: reject cases reject with the expected cause ---------------------

#[test]
fn reject_cases_reject_with_expected_cause() {
    let oracle: RejectOracle =
        serde_json::from_str(&read_oracle("reject.json")).expect("reject.json parses");

    for case in &oracle.cases {
        let program = parse_clean(&case.program, &case.name);
        let errors = match validate(&program) {
            Ok(()) => panic!(
                "reject case `{}` should REJECT (expected {}) but validated clean",
                case.name, case.expected
            ),
            Err(errs) => errs,
        };

        assert!(
            errors.iter().any(|e| matches_expected(e, &case.expected)),
            "reject case `{}` expected cause `{}` but got {:?}",
            case.name,
            case.expected,
            errors
        );
    }
}

/// Map an oracle `expected` cause string to a predicate over `SpecError`. The
/// oracle's variant names mirror the design doc REQ-4 names (README "Note on
/// the reject `expected` variant names"); we assert the cause, not a brittle
/// exact Display string.
fn matches_expected(err: &SpecError, expected: &str) -> bool {
    match expected {
        "UnknownCombinator" => matches!(err, SpecError::UnknownCombinator { .. }),
        "WrongArity" => matches!(err, SpecError::WrongArity { .. }),
        "WrongArgKind" => matches!(err, SpecError::WrongArgKind { .. }),
        // Forbidden arbitrary contract call (REQ-4 (iv)).
        "ForbiddenCall" => matches!(err, SpecError::ForbiddenCall { .. }),
        // Nested combinator in a closure body (REQ-6, #40 flat-closure rule).
        "NestedCombinator" => matches!(err, SpecError::NestedCombinator { .. }),
        other => panic!("reject.json has an unrecognized expected cause `{other}`"),
    }
}

// ---- AC-4: no panic, bounded recursion -------------------------------------

#[test]
fn validate_never_panics_on_deep_nesting() {
    // The parser has its OWN recursion guard (it would reject 400-deep SOURCE
    // before the validator ever sees it), so to exercise the VALIDATOR's guard
    // (REQ-5 / AC-4) we construct the deeply-nested AST DIRECTLY, bypassing the
    // parser. A 400-level `Binary` chain is far past the validator's
    // MAX_RECURSION_DEPTH (=64); the validator must surface a structured
    // `ExpressionTooDeep`, never overflow the native stack.
    use thermite_syntax::{
        BinOp, Block, Clause, Contract, EffectRow, Expr, FnItem, Item, PrimType, Program, Span,
        Type,
    };

    let mut expr = Expr::IntLit {
        value: 0,
        raw: "0".to_string(),
    };
    for _ in 0..400 {
        expr = Expr::Binary {
            op: BinOp::Add,
            lhs: Box::new(Expr::IntLit {
                value: 0,
                raw: "0".to_string(),
            }),
            rhs: Box::new(expr),
        };
    }
    let span = Span::new(0, 1);
    let clause = |e: Expr| Clause {
        expr: e,
        text: String::new(),
        span,
    };
    let program = Program {
        items: vec![Item::Fn(FnItem {
            slag: None,
            boundary: None,
            name: "f".to_string(),
            params: vec![],
            ret: Type::Prim(PrimType::U32),
            contract: Contract {
                req: clause(expr),
                ens: vec![clause(Expr::BoolLit(true))],
                fx: EffectRow::Pure,
            },
            body: Some(Block {
                stmts: vec![],
                tail: Some(Box::new(Expr::IntLit {
                    value: 0,
                    raw: "0".to_string(),
                })),
            }),
            span,
        })],
    };

    match validate(&program) {
        Err(errs) => assert!(
            errs.iter()
                .any(|e| matches!(e, SpecError::ExpressionTooDeep { .. })),
            "a 400-deep contract expression must surface ExpressionTooDeep, got {errs:?}"
        ),
        Ok(()) => panic!("a 400-deep contract expression must not validate clean"),
    }
}

#[test]
fn validate_never_panics_on_malformed_but_parsed() {
    // A grab-bag of parseable-but-cage-violating contracts. None may panic; each
    // must return Err.
    let programs = [
        // arbitrary free call in ens
        "fn f(xs: &[u32]) -> u32 req true ens g(xs) fx pure { 0 }",
        // path-qualified callee (forbidden shape)
        "fn f(xs: &[u32]) -> u32 req u32::cmp(xs) ens result == 0 fx pure { 0 }",
        // closure outside a Pred slot
        "fn f(xs: &[u32]) -> u32 req true ens (|x| x) == 0 fx pure { 0 }",
    ];
    for program in programs {
        let parsed = thermite_syntax::parse(program);
        if parsed.errors.is_empty() {
            let result = validate(&parsed.program);
            assert!(
                result.is_err(),
                "malformed contract `{program}` must be rejected, not accepted"
            );
        }
    }
}
