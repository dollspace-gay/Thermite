//! The live cert-oracle for the forge-tier covenant engine (`.design/stage1-forge-tier.md`
//! REQ-4 + AC-8, increment 2b). It drives the built `forge` binary with `check --json`
//! over the `conformance/covenant/` fixtures and asserts the covenant verdict + the
//! oracle-included covenant evidence block match the golden `*.cert.json`.
//!
//! AC-8 is exercised here at the certificate level (the engine's verus-free logic is
//! pinned by the `covenant_engine` unit tests; this is the end-to-end binary surface):
//!   - `max_buggy` — a planted bug (`ens result >= y` with a body returning `x`) dies as
//!     `CovenantRefuted` with the concrete `falsify` counterexample in the certificate,
//!     BEFORE any L3 proof search (covenant-before-burn).
//!   - `max_no_witness` — a forge-routed item whose `witness` block carries no author
//!     `inhabit` witness is refused, NAMED (`CovenantNoAuthorWitness`), before burn.
//!   - `max_correct` — a correct item's covenant validates (the unstated-vs-stated budget
//!     run finds no refutation), burns to L3 WITH the covenant in hand, and carries the
//!     deterministic evidence block (witness count, falsify generated/refuted = 2002/0,
//!     fixed seed).
//!
//! `forge check` resolves the verus version before the per-item loop, so these checks
//! need verus present even though the covenant short-circuits BEFORE the L3 burn; they
//! skip with a logged note when verus is absent (mirroring `check_conformance.rs`),
//! never panicking on a missing solver. `tests/` is not anti-pattern-gated, so
//! `unwrap`/`expect` are fine here.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn covenant_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("conformance")
        .join("covenant")
}

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// `true` iff verus can be located (`VERUS_BIN`, then PATH, then `~/.local/bin/verus`)
/// — mirrors `check_conformance.rs`. Skips with a logged note otherwise.
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

/// The golden certificate JSON for a `conformance/covenant/<name>.cert.json` fixture
/// (the external oracle, R-CHAR-3).
fn golden_cert(name: &str) -> Value {
    let path = covenant_dir().join(format!("{name}.cert.json"));
    let src = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read golden cert {}: {e}", path.display()));
    serde_json::from_str(&src).unwrap_or_else(|e| panic!("parse golden cert {name}: {e}"))
}

fn only_cert(certs: &[Value]) -> Value {
    assert_eq!(
        certs.len(),
        1,
        "the covenant fixtures are single-fn: {certs:?}"
    );
    certs[0].clone()
}

// ---- AC-8: a planted bug dies as CovenantRefuted with the concrete counterexample ----

#[test]
fn planted_bug_is_covenant_refuted_with_counterexample_in_cert() {
    if !verus_present() {
        eprintln!(
            "SKIP: verus not available — `forge check` resolves the verus version before \
             the covenant short-circuit (set VERUS_BIN or install verus on PATH). The \
             covenant logic itself is pinned verus-free by the covenant_engine unit tests."
        );
        return;
    }
    let (code, certs) = run_check_json(&covenant_dir().join("max_buggy.th"));
    assert_ne!(code, Some(0), "a refuted covenant must exit non-zero");
    let cert = only_cert(&certs);
    let golden = golden_cert("max_buggy");

    // The verdict: a non-certifying L0 with the CovenantRefuted reason (never degraded).
    assert_eq!(cert["item"], Value::from("maxv"));
    assert_eq!(cert["level"], Value::from("L0"));
    assert_eq!(cert["level"], golden["level"]);
    assert_eq!(cert["reject"]["cause"], Value::from("CovenantRefuted"));
    assert_eq!(cert["reject"]["cause"], golden["reject"]["cause"]);
    assert_eq!(
        cert["lowered_assurance"],
        Value::from(false),
        "a covenant refutation is a hard fail, NEVER a lowered-assurance degrade (R-DEFER-9)"
    );

    // The concrete counterexample is in the certificate (AC-8) and is deterministic
    // (the fixed seed), so it equals the golden's.
    let diag = cert["obligations"][0]["diagnostic"]
        .as_str()
        .expect("a counterexample diagnostic");
    assert!(
        diag.contains("counterexample: ("),
        "the cert carries the concrete falsifying input: {diag}"
    );
    assert_eq!(
        cert["obligations"][0]["diagnostic"], golden["obligations"][0]["diagnostic"],
        "the falsify counterexample is deterministic (fixed seed)"
    );

    // The oracle-included covenant evidence block (Q-ORACLE): deterministic, == golden.
    assert_eq!(
        cert["covenant_evidence"], golden["covenant_evidence"],
        "the covenant evidence block is deterministic and oracle-included"
    );
    assert_eq!(cert["covenant_evidence"]["falsify_refuted"], Value::from(1));
    assert_eq!(cert["covenant_evidence"]["witness_count"], Value::from(2));
}

// ---- AC-8 / R-COV-1: a forge-routed item with no author witness is refused, named ----

#[test]
fn no_author_witness_is_refused_before_burn_named() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — covenant no-witness cert not run.");
        return;
    }
    let (code, certs) = run_check_json(&covenant_dir().join("max_no_witness.th"));
    assert_ne!(code, Some(0), "a refused covenant must exit non-zero");
    let cert = only_cert(&certs);
    let golden = golden_cert("max_no_witness");

    assert_eq!(cert["item"], Value::from("maxv"));
    assert_eq!(
        cert["level"],
        Value::from("L0"),
        "refused before burn — not L3"
    );
    assert_eq!(
        cert["reject"]["cause"],
        Value::from("CovenantNoAuthorWitness"),
        "the refusal names the missing-author-witness cause (R-COV-1)"
    );
    assert_eq!(cert["reject"]["cause"], golden["reject"]["cause"]);
    // No covenant record was produced (the block is absent — the witness had no author
    // witness to build one from), so the cert carries no covenant evidence.
    assert!(
        cert.get("covenant_evidence").is_none(),
        "a refused (never-validated) covenant produces no evidence block: {cert}"
    );
}

// ---- A correct covenant validates, burns L3, and records its evidence (REQ-4) -------

#[test]
fn correct_covenant_validates_burns_l3_and_records_evidence() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — correct-covenant L3 cert not run.");
        return;
    }
    let (code, certs) = run_check_json(&covenant_dir().join("max_correct.th"));
    assert_eq!(
        code,
        Some(0),
        "a correct covenanted item certifies (exit 0)"
    );
    let cert = only_cert(&certs);
    let golden = golden_cert("max_correct");

    assert_eq!(cert["item"], Value::from("maxv"));
    assert_eq!(
        cert["level"],
        Value::from("L3"),
        "the covenant validated and the item burned to L3 with the covenant in hand"
    );
    assert_eq!(cert["level"], golden["level"]);

    // The deterministic covenant evidence (Q-ORACLE): no refutation, the author
    // witnesses + the stated `falsify 2000` budget all req-satisfying (req is `true`).
    assert_eq!(cert["covenant_evidence"], golden["covenant_evidence"]);
    assert_eq!(cert["covenant_evidence"]["falsify_refuted"], Value::from(0));
    assert_eq!(cert["covenant_evidence"]["witness_count"], Value::from(2));
    assert_eq!(
        cert["covenant_evidence"]["falsify_generated"],
        Value::from(2002),
        "2 author witnesses + 2000 generated inputs, all req-satisfying"
    );
}
