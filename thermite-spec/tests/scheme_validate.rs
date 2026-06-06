//! Conformance test for the Basis Stage 2b recursion-scheme validator against
//! the hand-derived oracle at `conformance/adt-schemes/cases.json` (R-CHAR-3).
//! The oracle is the external truth; the validator is the artifact under test.
//! This file NEVER edits the oracle — a failure here is a bug in `thermite-spec`.
//!
//! Governing design: `.design/basis/02-recursion-schemes.md` REQ-1 (the scheme
//! set as named primitives), REQ-2 (the FLAT step closure — a nested scheme is
//! rejected), REQ-4 (the cage bridge — a scheme call is a named-composition leaf;
//! a nested scheme rejects), REQ-9 (structured `SpecError`, no panics).
//!
//! - `certify` (the `list_fold.th` source): the `fold`/`for_all` scheme calls of
//!   `conformance/list_fold.th` validate clean (the cage ACCEPTS a top-level
//!   scheme call — REQ-1/REQ-4).
//! - `reject`: each crafted negative yields a `SpecError` whose `Display`
//!   CONTAINS the oracle's `expect_error_contains` substring — hand-derived,
//!   never read back from the validator (R-CHAR-3). `nested_scheme_in_step`
//!   ("nested") + `unknown_scheme` ("not a registered").
//!
//! `unwrap`/`expect` are fine here — `tests/` is not anti-pattern-gated.

use std::path::PathBuf;

use serde::Deserialize;

use thermite_spec::validate;

// ---- oracle JSON shape -----------------------------------------------------

#[derive(Debug, Deserialize)]
struct Oracle {
    reject: Vec<RejectCase>,
}

#[derive(Debug, Deserialize)]
struct RejectCase {
    name: String,
    /// A substring the rejecting `SpecError`'s `Display` must contain.
    expect_error_contains: String,
    /// The inline Thermite source the validator runs over.
    program: String,
}

// ---- paths -----------------------------------------------------------------

/// `CARGO_MANIFEST_DIR` is `thermite-spec/`; the oracle + corpus live at the
/// workspace root.
fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn load_oracle() -> Oracle {
    let path = workspace_root().join("conformance/adt-schemes/cases.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read oracle {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse oracle {}: {e}", path.display()))
}

/// Parse the corpus source, asserting it parses CLEANLY (used for the certify
/// `list_fold.th`, a real corpus program with no syntax noise).
fn parse_program(src: &str) -> thermite_syntax::Program {
    let result = thermite_syntax::parse(src);
    assert!(
        result.errors.is_empty(),
        "the oracle program must PARSE (syntax is a scheme-free concern); \
         parse errors: {:?}",
        result.errors
    );
    result.program
}

/// Parse an inline reject-case program with the recovering parser, returning the
/// recovered `Program` REGARDLESS of any syntax noise. The oracle reject programs
/// are inline snippets exercising the VALIDATOR (the scheme cage), not the
/// parser; the validator is what is under test, so we run it on whatever the
/// recovering parser produces (the scheme-call items are recovered intact even
/// when a trailing token is stray).
fn parse_program_recovering(src: &str) -> thermite_syntax::Program {
    thermite_syntax::parse(src).program
}

// ---- REQ-1/REQ-4: the certify source validates clean ----------------------

/// `conformance/list_fold.th` — three `spec fn`s whose bodies are scheme calls
/// (`fold(l, 0, |x, acc| …)`, `for_all(l, |x| …)`) — VALIDATES (REQ-1/REQ-4: a
/// top-level scheme call is a named-composition leaf, its flat step admitted).
#[test]
fn list_fold_validates() {
    let path = workspace_root().join("conformance/list_fold.th");
    let src =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let program = parse_program(&src);
    let outcome = validate(&program);
    assert!(
        outcome.is_ok(),
        "conformance/list_fold.th must validate clean (its `fold`/`for_all` \
         scheme calls are named-composition leaves); got {:?}",
        outcome.err()
    );
}

// ---- REQ-2/REQ-9: the reject cases ----------------------------------------

/// Each oracle `reject` case is REJECTED, and the rejecting `SpecError`'s
/// `Display` CONTAINS the oracle's `expect_error_contains` substring (R-CHAR-3 —
/// the expectation is hand-derived in the oracle, never read back from the
/// validator). Covers `nested_scheme_in_step` (a `fold` nested in a `fold` step
/// — REQ-2) and `unknown_scheme` (an unregistered callee — REQ-1).
#[test]
fn reject_cases_yield_the_oracle_error() {
    let oracle = load_oracle();
    assert!(
        oracle.reject.len() >= 2,
        "oracle must carry the 2 reject cases"
    );
    for case in &oracle.reject {
        let program = parse_program_recovering(&case.program);
        let errors = match validate(&program) {
            Ok(()) => panic!(
                "reject case `{}` must FAIL validation but it passed",
                case.name
            ),
            Err(errors) => errors,
        };
        let rendered: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
        assert!(
            rendered
                .iter()
                .any(|msg| msg.contains(&case.expect_error_contains)),
            "reject case `{}`: expected an error whose Display contains {:?}, \
             got {:?}",
            case.name,
            case.expect_error_contains,
            rendered
        );
    }
}
