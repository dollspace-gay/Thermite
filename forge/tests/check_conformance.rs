//! The LIVE cert-oracle for `forge check` (`goal.md` verification model (B);
//! `.design/forge/check.md` AC-1/AC-2/AC-3). It drives the BUILT `forge` binary
//! (`.design/forge/cli.md` Verification: "a CLI integration test drives the built
//! `forge` binary") with `check --json`, parses the emitted certificate JSON, and
//! asserts its DETERMINISTIC fields match the golden `conformance/<name>.cert.json`
//! — `item`, `level`, `effects`, `slag` — under the forward-declaration contract
//! (`conformance/README.md`): `contract_quality.*` and `solver_time_ms` are NOT
//! asserted (R-CHAR-3 — expected values trace to the golden cert, never to forge's
//! own output).
//!
//! Driving the binary (rather than calling a library API) keeps `forge` a pure
//! `bin` crate (no `lib.rs`) AND exercises the real REQ-4/REQ-5 stream + exit-code
//! surface end to end.
//!
//! These checks RUN VERUS. If verus is absent they SKIP LOUDLY (mirroring
//! `thermite-lower/tests/lower_conformance.rs`'s Option-resolve + eprintln-skip)
//! — never panic on a missing solver. `tests/` is not anti-pattern-gated, so
//! `unwrap`/`expect` are fine here.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("conformance")
}

/// Path to the freshly built `forge` binary (cargo sets this for integration
/// tests).
fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// `true` iff verus can be located (`VERUS_BIN`, then PATH, then
/// `~/.local/bin/verus`) — mirrors `lower_conformance.rs`. SKIP LOUDLY otherwise.
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

/// Run `forge check <file> --json`, returning (exit_code, parsed JSON array of
/// certificates). stdout under `--json` must be a single JSON document (AC-2).
fn run_check_json(file: &Path) -> (Option<i32>, Vec<Value>) {
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(file)
        .arg("--json")
        .output()
        .unwrap_or_else(|e| panic!("spawn forge: {e}"));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!(
            "forge --json stdout must be one JSON document: {e}\nstdout:\n{stdout}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stderr)
        )
    });
    let arr = value
        .as_array()
        .unwrap_or_else(|| panic!("forge --json must emit a JSON array of certs: {value}"))
        .clone();
    (out.status.code(), arr)
}

/// The golden certificate JSON for `<name>` (the EXTERNAL oracle, R-CHAR-3).
fn golden_cert(name: &str) -> Value {
    let path = corpus_dir().join(format!("{name}.cert.json"));
    let src = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read golden cert {}: {e}", path.display()));
    serde_json::from_str(&src).unwrap_or_else(|e| panic!("parse golden cert {name}: {e}"))
}

fn find_cert(certs: &[Value], item: &str) -> Value {
    certs
        .iter()
        .find(|c| c.get("item").and_then(|v| v.as_str()) == Some(item))
        .unwrap_or_else(|| panic!("no certificate for item `{item}` in {certs:?}"))
        .clone()
}

// ---- AC-1: sum → L3, deterministic fields == golden -----------------------

#[test]
fn sum_cert_matches_golden_deterministic_subset() {
    if !verus_present() {
        eprintln!(
            "SKIP: verus not available — `forge check sum.th` cert-oracle not run \
             (set VERUS_BIN or install verus on PATH)."
        );
        return;
    }
    let (code, certs) = run_check_json(&corpus_dir().join("sum.th"));
    assert_eq!(code, Some(0), "a fully verified sum must exit 0");
    let sum = find_cert(&certs, "sum");
    let golden = golden_cert("sum");

    // The DETERMINISTIC subset must match the golden oracle; NOT
    // contract_quality.* / solver_time_ms (forward-declared / non-det).
    assert_eq!(sum["item"], golden["item"]);
    assert_eq!(sum["item"], Value::from("sum"));
    assert_eq!(sum["level"], Value::from("L3"), "sum must verify L3");
    assert_eq!(sum["level"], golden["level"]);
    assert_eq!(sum["effects"], golden["effects"]);
    assert_eq!(sum["effects"], serde_json::json!(["pure"]));
    assert_eq!(sum["slag"], golden["slag"]);
    assert_eq!(sum["slag"], Value::from(false));

    // Per-obligation list: present and all discharged.
    let obs = sum["obligations"]
        .as_array()
        .expect("obligations array present");
    assert!(!obs.is_empty(), "discharged cert carries a non-empty list");
    assert!(
        obs.iter()
            .all(|o| o.get("status").and_then(|s| s.as_str()) == Some("discharged")),
        "every sum obligation discharged: {obs:?}"
    );
}

// ---- AC-2: binary_search → L3 (level only; no golden cert yet) ------------

#[test]
fn binary_search_is_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — `forge check binary_search.th` not run.");
        return;
    }
    let (code, certs) = run_check_json(&corpus_dir().join("binary_search.th"));
    assert_eq!(code, Some(0));
    let bs = find_cert(&certs, "binary_search");
    assert_eq!(
        bs["level"],
        Value::from("L3"),
        "binary_search must verify L3"
    );
    assert_eq!(bs["effects"], serde_json::json!(["pure"]));
}

// ---- AC-3: broken contract → reported non-L3 + counterexample -------------

#[test]
fn broken_contract_is_reported_failure_with_counterexample() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — broken-contract cert not run.");
        return;
    }
    // `ens result == x + 2` but the body returns `x + 1`: parses, validates,
    // effect-checks, and lowers cleanly — only the SMT proof fails. Written to a
    // temp `.th` fixture (no committed broken corpus entry needed).
    let fixture = std::env::temp_dir().join(format!("forge_broken_{}.th", std::process::id()));
    std::fs::write(
        &fixture,
        "fn add_one(x: u64) -> u64\n  req x < 1000\n  ens result == x + 2\n  fx  pure\n{\n  x + 1\n}\n",
    )
    .expect("write broken fixture");

    let (code, certs) = run_check_json(&fixture);
    let _ = std::fs::remove_file(&fixture);

    // A reported verification failure: NON-zero exit (the verification-failure
    // code, NOT the environment code), but a valid cert document on stdout.
    assert_eq!(
        code,
        Some(1),
        "a reported verification failure exits with the verification-failure code"
    );
    let cert = find_cert(&certs, "add_one");
    assert_ne!(
        cert["level"],
        Value::from("L3"),
        "a false ens must NOT certify L3"
    );
    let obs = cert["obligations"].as_array().expect("obligations present");
    let failures: Vec<&Value> = obs
        .iter()
        .filter(|o| o.get("status").and_then(|s| s.as_str()) == Some("failed"))
        .collect();
    assert!(
        !failures.is_empty(),
        "the cert must carry a per-obligation failure (the counterexample): {obs:?}"
    );
    // The failure names the obligation + carries a source location / diagnostic
    // (§5.1 "counterexamples, not adjectives"), never a bare adjective.
    let f = failures[0];
    let name = f.get("name").and_then(|v| v.as_str()).unwrap_or("");
    assert!(
        !name.is_empty() && name != "verification failed",
        "failure must name the obligation, not a bare adjective: {f:?}"
    );
    assert!(
        f.get("location").is_some() || f.get("diagnostic").is_some(),
        "failure must carry a source location or diagnostic witness: {f:?}"
    );
}

// ---- AC-2 (stream discipline) + AC-1: usage error exits non-zero ----------

#[test]
fn missing_file_is_usage_error_nonzero() {
    // No verus needed: arg parsing fails before the pipeline.
    let out = Command::new(forge_bin())
        .arg("check")
        .output()
        .expect("spawn forge");
    assert_ne!(out.status.code(), Some(0), "missing <file> must not exit 0");
    assert!(
        out.stdout.is_empty(),
        "a usage error writes nothing to stdout (diagnostics go to stderr)"
    );
    assert!(
        !out.stderr.is_empty(),
        "a usage error writes a diagnostic to stderr"
    );
}
