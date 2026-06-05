//! #16 boundary-fn cert oracle (`.design/boundary/ffi-boundary.md` AC-2;
//! `conformance/boundary/cases.json`). Drives the BUILT `forge` binary with
//! `check --json` over each case's program (written to a temp `.th` file so the
//! read-only `conformance/` fixtures are untouched — R-CHAR-3) and asserts:
//!
//! - `foreign_id` → `Level::L1`, `boundary: true`, `boundary_target ==
//!   "ext::foreign_id"`, `slag: false` — NOT L3 (no verus run on a foreign body).
//! - `bodyless_without_boundary` → a PARSE ERROR (exit non-zero; OQ-2).
//! - `boundary_vacuous_contract` → REJECTED `EnsIsTrivial` (the §7.1 triage still
//!   applies to a boundary fn: it exempts proving, not stating).
//!
//! Expected values trace to the golden `conformance/boundary/cases.json`
//! (R-CHAR-3), never copied from forge's own output.
//!
//! A boundary fn runs NO verus; but `forge check` resolves the verus version
//! up-front for the proof cache, so these SKIP LOUDLY if verus is absent (never
//! panic on a missing solver), mirroring `check_conformance.rs`.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

fn cases_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("conformance")
        .join("boundary")
        .join("cases.json")
}

/// `true` iff verus can be located — mirrors `check_conformance.rs`. A boundary
/// fn never runs verus, but `forge check` resolves the verus version up-front for
/// the proof cache, so a boundary-only file still needs the prover present.
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

/// Load the boundary cases oracle JSON (R-CHAR-3 external truth).
fn cases() -> Value {
    let src = std::fs::read_to_string(cases_path())
        .unwrap_or_else(|e| panic!("read boundary cases.json: {e}"));
    serde_json::from_str(&src).unwrap_or_else(|e| panic!("parse boundary cases.json: {e}"))
}

/// Write `program` to a unique temp `.th` file (the read-only `conformance/`
/// fixtures stay untouched) and return its path. The caller removes it.
fn write_temp_program(name: &str, program: &str) -> PathBuf {
    let pid = std::process::id();
    let path = std::env::temp_dir().join(format!("boundary_{name}_{pid}.th"));
    std::fs::write(&path, program).unwrap_or_else(|e| panic!("write temp .th: {e}"));
    path
}

/// Run `forge check <file> --json`, returning (exit_code, stdout, stderr).
fn run_check(file: &Path) -> (Option<i32>, String, String) {
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(file)
        .arg("--json")
        .output()
        .unwrap_or_else(|e| panic!("spawn forge: {e}"));
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

fn find_cert(certs: &[Value], item: &str) -> Value {
    certs
        .iter()
        .find(|c| c.get("item").and_then(|v| v.as_str()) == Some(item))
        .unwrap_or_else(|| panic!("no certificate for item `{item}` in {certs:?}"))
        .clone()
}

// AC-2: foreign_id certifies L1 + boundary, NOT L3, with the foreign target.
#[test]
fn foreign_id_certifies_l1_boundary_not_l3() {
    if !verus_present() {
        eprintln!(
            "SKIP: verus not available — boundary cert-oracle not run (a boundary fn \
             runs no verus, but `forge check` resolves the verus version up-front)."
        );
        return;
    }
    let oracle = cases();
    let case = oracle["certifies_l1_boundary"][0].clone();
    let program = case["program"].as_str().expect("program string");
    let expect_target = case["expect_target"].as_str().expect("expect_target");

    let path = write_temp_program("foreign_id", program);
    let (code, stdout, stderr) = run_check(&path);
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        code,
        Some(0),
        "a boundary fn certifies (L1) and exits 0; stderr:\n{stderr}"
    );
    let value: Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("forge --json must be one JSON doc: {e}\nstdout:\n{stdout}"));
    let certs = value.as_array().expect("JSON array of certs");
    let cert = find_cert(certs, "foreign_id");

    // Hand-derived oracle (R-CHAR-3): L1, boundary true, the foreign target, NOT L3.
    assert_eq!(cert["level"], Value::from("L1"), "boundary fn → L1, NOT L3");
    assert_ne!(
        cert["level"],
        Value::from("L3"),
        "no verus on a foreign body"
    );
    assert_eq!(cert["boundary"], Value::from(true), "boundary flag set");
    assert_eq!(
        cert["boundary_target"],
        Value::from(expect_target),
        "the foreign target is recorded for the TCB enumeration"
    );
    assert_eq!(
        cert["slag"],
        Value::from(false),
        "a boundary fn is not slag"
    );
}

// AC reject 1: a bodyless fn WITHOUT #[boundary] is a PARSE ERROR (OQ-2).
#[test]
fn bodyless_without_boundary_is_a_parse_error() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — boundary reject-oracle not run.");
        return;
    }
    let oracle = cases();
    let case = oracle["reject"]
        .as_array()
        .expect("reject array")
        .iter()
        .find(|c| c["name"] == "bodyless_without_boundary")
        .expect("bodyless_without_boundary case")
        .clone();
    let program = case["program"].as_str().expect("program string");

    let path = write_temp_program("bodyless_without_boundary", program);
    let (code, _stdout, _stderr) = run_check(&path);
    let _ = std::fs::remove_file(&path);

    assert_ne!(
        code,
        Some(0),
        "a bodyless fn WITHOUT #[boundary] is a parse error (OQ-2) → non-zero exit"
    );
}

// AC reject 2: a vacuous contract on a boundary fn is still REJECTED by §7.1
// triage (boundary exempts PROVING, not STATING).
#[test]
fn boundary_vacuous_contract_is_rejected() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — boundary reject-oracle not run.");
        return;
    }
    let oracle = cases();
    let case = oracle["reject"]
        .as_array()
        .expect("reject array")
        .iter()
        .find(|c| c["name"] == "boundary_vacuous_contract")
        .expect("boundary_vacuous_contract case")
        .clone();
    let program = case["program"].as_str().expect("program string");
    let expect_cause = case["cause"].as_str().expect("cause"); // "EnsIsTrivial"

    let path = write_temp_program("boundary_vacuous_contract", program);
    let (code, stdout, _stderr) = run_check(&path);
    let _ = std::fs::remove_file(&path);

    assert_ne!(
        code,
        Some(0),
        "a vacuous-contract boundary fn does not certify"
    );
    let value: Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("forge --json must be one JSON doc: {e}\nstdout:\n{stdout}"));
    let certs = value.as_array().expect("JSON array of certs");
    let cert = find_cert(certs, "g");
    assert_eq!(cert["level"], Value::from("L0"), "a triage reject is L0");
    assert_eq!(
        cert["reject"]["cause"],
        Value::from(expect_cause),
        "the §7.1 triage cause is the oracle's `EnsIsTrivial`"
    );
    // It did NOT slip through as a certified boundary fn.
    assert_ne!(
        cert["boundary"],
        Value::from(true),
        "a rejected fn does not certify as boundary"
    );
}
