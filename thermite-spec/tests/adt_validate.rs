//! Conformance test for the Basis Stage 1b ADT validator against the
//! hand-derived oracle at `conformance/adt-validate/cases.json` (R-CHAR-3). The
//! oracle is the external truth; the validator is the artifact under test. This
//! file NEVER edits the oracle — a failure here is a bug in `thermite-spec`.
//!
//! Governing design: `.design/basis/01-adts.md` REQ-5 (exhaustiveness —
//! `NonExhaustiveMatch`/`UnreachableArm`), REQ-6 (well-formed field/variant
//! access — `UnknownField`/`UnknownVariant`; `is` against a declared variant),
//! REQ-7 (ADT predicates fit the SpecTherm cage).
//!
//! - `accept`: the 3 ADT corpus programs (`bank_account`/`shape`/`list_sum`)
//!   validate clean (the 1a `UnsupportedAdt` gate is gone).
//! - `reject`: each crafted negative yields the EXACT `SpecError` variant the
//!   oracle names, with the key payload (missing variant set / field / variant
//!   name) — hand-derived, never read back from the validator (R-CHAR-3).
//!
//! `unwrap`/`expect` are fine here — `tests/` is not anti-pattern-gated.

use std::path::PathBuf;

use serde::Deserialize;

use thermite_spec::{validate, SpecError};

// ---- oracle JSON shape -----------------------------------------------------

#[derive(Debug, Deserialize)]
struct Oracle {
    accept: Vec<AcceptCase>,
    reject: Vec<RejectCase>,
}

#[derive(Debug, Deserialize)]
struct AcceptCase {
    name: String,
    /// Path (relative to the workspace root) to the `.th` corpus program.
    source: String,
}

#[derive(Debug, Deserialize)]
struct RejectCase {
    name: String,
    /// The exact `SpecError` variant identifier the oracle expects.
    expect_error: String,
    /// REQ-5: the expected `NonExhaustiveMatch.missing` variant set (when set).
    #[serde(default)]
    expect_missing: Option<Vec<String>>,
    /// REQ-6: the expected `UnknownField`/`UnknownVariant.name` (when set).
    #[serde(default)]
    expect_name: Option<String>,
    /// The inline Thermite source the validator runs over.
    program: String,
}

// ---- paths -----------------------------------------------------------------

/// `CARGO_MANIFEST_DIR` is `thermite-spec/`; the oracle + corpus live at the
/// workspace root.
fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn read_oracle() -> Oracle {
    let path = workspace_root()
        .join("conformance")
        .join("adt-validate")
        .join("cases.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read oracle {}: {e}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("oracle {} does not parse: {e}", path.display()))
}

fn read_corpus(rel: &str) -> String {
    let path = workspace_root().join(rel);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read corpus {}: {e}", path.display()))
}

/// Parse a program and assert it parsed with zero syntax errors — Stage 1a
/// already lands the ADT surface, so a parse failure means `thermite-syntax`
/// broke, not the validator.
fn parse_clean(program: &str, case: &str) -> thermite_syntax::Program {
    let result = thermite_syntax::parse(program);
    assert!(
        result.errors.is_empty(),
        "case `{case}` failed to PARSE (thermite-syntax errors): {:?}",
        result.errors
    );
    result.program
}

// ---- accept: the 3 ADT corpus programs validate clean ----------------------

#[test]
fn adt_corpus_programs_validate_clean() {
    let oracle = read_oracle();
    assert_eq!(
        oracle.accept.len(),
        3,
        "the oracle pins exactly 3 ADT accept programs"
    );
    for case in &oracle.accept {
        let source = read_corpus(&case.source);
        let program = parse_clean(&source, &case.name);
        let result = validate(&program);
        assert_eq!(
            result,
            Ok(()),
            "accept case `{}` ({}) MUST validate clean, got {:?}",
            case.name,
            case.source,
            result
        );
    }
}

// ---- reject: each crafted negative yields the exact structured error -------

#[test]
fn adt_reject_cases_yield_exact_error() {
    let oracle = read_oracle();
    assert_eq!(
        oracle.reject.len(),
        5,
        "the oracle pins exactly 5 ADT reject programs"
    );
    for case in &oracle.reject {
        let program = parse_clean(&case.program, &case.name);
        let errors = match validate(&program) {
            Ok(()) => panic!(
                "reject case `{}` MUST reject (expected {}), but validated clean",
                case.name, case.expect_error
            ),
            Err(errors) => errors,
        };

        // Assert the expected variant is present AND its key payload matches the
        // oracle's hand-derived expectation (R-CHAR-3). We search for the
        // expected variant rather than asserting it is the SOLE error — the
        // validator accumulates diagnostics — but each oracle case is crafted to
        // have exactly one ADT fault, so the matching error is the verdict.
        let found = errors.iter().find(|e| variant_matches(e, case));
        assert!(
            found.is_some(),
            "reject case `{}` expected `{}` (missing={:?}, name={:?}); got {:?}",
            case.name,
            case.expect_error,
            case.expect_missing,
            case.expect_name,
            errors
        );
    }
}

/// True iff `error` is the variant the oracle names for `case`, with the key
/// payload matching. Hand-derived expectations from `.design/basis/01-adts.md`
/// (R-CHAR-3) — the test asserts the validator EQUALS the oracle, never itself.
fn variant_matches(error: &SpecError, case: &RejectCase) -> bool {
    match (case.expect_error.as_str(), error) {
        ("NonExhaustiveMatch", SpecError::NonExhaustiveMatch { missing, .. }) => {
            match &case.expect_missing {
                Some(expected) => missing == expected,
                None => true,
            }
        }
        ("UnreachableArm", SpecError::UnreachableArm { .. }) => true,
        ("UnknownField", SpecError::UnknownField { name, .. }) => match &case.expect_name {
            Some(expected) => name == expected,
            None => true,
        },
        ("UnknownVariant", SpecError::UnknownVariant { name, .. }) => match &case.expect_name {
            Some(expected) => name == expected,
            None => true,
        },
        _ => false,
    }
}

// ---- the validator never panics on a crafted negative ----------------------

#[test]
fn adt_validation_never_panics() {
    // Every reject program is structurally well-formed-but-semantically-rejected
    // — `validate` must return `Err`, never panic (R-CODE-2).
    let oracle = read_oracle();
    for case in &oracle.reject {
        let program = parse_clean(&case.program, &case.name);
        let _ = validate(&program);
    }
}
