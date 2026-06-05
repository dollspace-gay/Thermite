//! #17 end-to-end vs to-the-boundary cert oracle (`.design/forge/e2e-vs-boundary.md`
//! AC-1..AC-5; `conformance/e2e/cases.json`). Drives the BUILT `forge` binary with
//! `check --json` over each case's program (written to a temp `.th` file so the
//! read-only `conformance/` fixtures are untouched — R-CHAR-3) and asserts each
//! fn's `assurance_scope` field against the hand-derived oracle:
//!
//! - `end_to_end` cases (corpus `sum` → `sum`; corpus `binary_search` →
//!   `binary_search`) → `assurance_scope.kind == "end_to_end"`.
//! - `to_boundary` cases (`direct_boundary_caller` → `caller` via `ext_id`;
//!   `transitive_boundary_caller` → `h` via `ext_id`; `slag_caller` → `caller` via
//!   `vendored`) → `assurance_scope.kind == "to_boundary"` with the oracle `via`.
//! - PROJECT claim (the design REQ-4 rule, derived from the cert array): a file
//!   with any to-boundary fn → project TO-THE-BOUNDARY; the pure corpus → project
//!   END-TO-END.
//!
//! The classification is SYNTACTIC, but `forge check` runs the full pipeline, so
//! these SKIP LOUDLY if verus is absent (a pure caller of a boundary fn still
//! L3-verifies against the boundary's contract; never panic on a missing solver),
//! mirroring `boundary_conformance.rs`. Expected values trace to the golden
//! `conformance/e2e/cases.json` (R-CHAR-3), never copied from forge's own output.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

fn conformance_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("conformance")
}

fn cases() -> Value {
    let path = conformance_dir().join("e2e").join("cases.json");
    let src = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read e2e cases.json: {e}"));
    serde_json::from_str(&src).unwrap_or_else(|e| panic!("parse e2e cases.json: {e}"))
}

/// `true` iff verus can be located — mirrors `boundary_conformance.rs`. A pure
/// caller of a boundary fn still runs verus (it L3-verifies against the boundary's
/// contract), so the to-boundary cases need the prover present.
fn verus_present() -> bool {
    if let Ok(p) = std::env::var("VERUS_BIN") {
        if Path::new(&p).exists() {
            return true;
        }
    }
    if let Ok(out) = Command::new("which").arg("verus").output() {
        if out.status.success() && !String::from_utf8_lossy(&out.stdout).trim().is_empty() {
            return true;
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        if PathBuf::from(home).join(".local/bin/verus").exists() {
            return true;
        }
    }
    false
}

fn write_temp_program(name: &str, program: &str) -> PathBuf {
    let pid = std::process::id();
    let path = std::env::temp_dir().join(format!("e2e_{name}_{pid}.th"));
    std::fs::write(&path, program).unwrap_or_else(|e| panic!("write temp .th: {e}"));
    path
}

/// Run `forge check <file> --json`, returning the parsed cert array (or the raw
/// stdout/stderr on a non-JSON / non-zero result).
fn run_check_json(file: &Path) -> (Option<i32>, Vec<Value>, String) {
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(file)
        .arg("--json")
        .output()
        .unwrap_or_else(|e| panic!("spawn forge: {e}"));
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let certs = serde_json::from_str::<Value>(stdout.trim())
        .ok()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();
    (out.status.code(), certs, stderr)
}

fn find_cert<'a>(certs: &'a [Value], item: &str) -> &'a Value {
    certs
        .iter()
        .find(|c| c.get("item").and_then(|v| v.as_str()) == Some(item))
        .unwrap_or_else(|| panic!("no certificate for item `{item}` in {certs:?}"))
}

/// The §9 PROJECT claim derived from the cert array (the design REQ-4 rule): the
/// project is END-TO-END iff EVERY cert is end-to-end. A cert is end-to-end iff its
/// `assurance_scope` is absent (the golden default) or `kind == "end_to_end"`.
fn project_is_end_to_end(certs: &[Value]) -> bool {
    certs.iter().all(|c| match c.get("assurance_scope") {
        None | Some(Value::Null) => true,
        Some(scope) => scope.get("kind").and_then(|v| v.as_str()) == Some("end_to_end"),
    })
}

// AC-1: the pure corpus is END-TO-END (sum → spec_sum; binary_search → combinators)
// and the project claim is END-TO-END. Anchored to `cases.json` `end_to_end`.
#[test]
fn corpus_programs_are_end_to_end() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — e2e corpus oracle not run.");
        return;
    }
    let oracle = cases();
    for case in oracle["end_to_end"].as_array().expect("end_to_end array") {
        let src_rel = case["source"].as_str().expect("source");
        let item = case["fn"].as_str().expect("fn");
        let src_path = conformance_dir()
            .join("..")
            .join(src_rel)
            .canonicalize()
            .unwrap_or_else(|_| conformance_dir().join("..").join(src_rel));

        let (code, certs, stderr) = run_check_json(&src_path);
        assert_eq!(
            code,
            Some(0),
            "pure corpus program `{src_rel}` certifies; stderr:\n{stderr}"
        );
        let cert = find_cert(&certs, item);
        assert_eq!(
            cert["assurance_scope"]["kind"],
            Value::from("end_to_end"),
            "`{item}` (source {src_rel}) closure reaches no boundary/slag → END-TO-END"
        );
        // PROJECT claim (REQ-4): the pure corpus file is END-TO-END.
        assert!(
            project_is_end_to_end(&certs),
            "the pure corpus file `{src_rel}` is a project END-TO-END"
        );
    }
}

// Scope ⊥ level (REQ-5): `sum` keeps level L3 AND assurance_scope end_to_end.
#[test]
fn sum_keeps_l3_and_is_end_to_end() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — e2e sum orthogonality oracle not run.");
        return;
    }
    let sum_path = conformance_dir().join("sum.th");
    let (code, certs, stderr) = run_check_json(&sum_path);
    assert_eq!(code, Some(0), "sum certifies; stderr:\n{stderr}");
    let cert = find_cert(&certs, "sum");
    assert_eq!(cert["level"], Value::from("L3"), "sum proves L3");
    assert_eq!(
        cert["assurance_scope"]["kind"],
        Value::from("end_to_end"),
        "sum is END-TO-END AND L3 — scope is orthogonal to level"
    );
}

// AC-2/AC-3/AC-4: each to_boundary case classifies TO-THE-BOUNDARY with the oracle
// `via`, and the project claim is TO-THE-BOUNDARY. Anchored to `cases.json`
// `to_boundary` (R-CHAR-3).
#[test]
fn to_boundary_cases_classify_via_the_crossing() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — e2e to-boundary oracle not run.");
        return;
    }
    let oracle = cases();
    for case in oracle["to_boundary"].as_array().expect("to_boundary array") {
        let name = case["name"].as_str().expect("name");
        let item = case["fn"].as_str().expect("fn");
        let via = case["via"].as_str().expect("via");
        let program = case["program"].as_str().expect("program");

        let path = write_temp_program(name, program);
        let (_code, certs, _stderr) = run_check_json(&path);
        let _ = std::fs::remove_file(&path);

        // #17 asserts the CLASSIFICATION only — `assurance_scope` is SYNTACTIC and
        // orthogonal to the fn's verification level (cases.json note). We do NOT
        // assert the caller certifies: a pure caller verifying *through* a boundary
        // fn's contract (the §9 composition rule) needs thermite-lower to emit the
        // boundary fn as a verus-assumable signature — tracked separately as #52.
        // Until #52, a boundary-caller is L0 but still classified to-the-boundary.
        let cert = find_cert(&certs, item);
        assert_eq!(
            cert["assurance_scope"]["kind"],
            Value::from("to_boundary"),
            "`{item}` (case {name}) closure reaches a crossing → TO-THE-BOUNDARY"
        );
        assert_eq!(
            cert["assurance_scope"]["via"],
            Value::from(via),
            "`{item}` (case {name}) records the oracle crossing `via`"
        );
        // PROJECT claim (REQ-4): a file with any to-boundary fn is TO-THE-BOUNDARY.
        assert!(
            !project_is_end_to_end(&certs),
            "case `{name}` has a to-boundary fn → project TO-THE-BOUNDARY"
        );
    }
}
