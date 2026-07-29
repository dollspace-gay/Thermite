//! ADVERSARIAL (issue #357, pre-trust-flip review): the bv route's vacuity gap.
//!
//! The v1/default cage rejects an unsatisfiable precondition as `VacuousPrecondition`
//! (the anti-Goodhart battery, RFC-1 §10 — a `req false` contract "proves" anything and
//! is a gaming vector). The bv route runs no vacuity check: it only threads `req` into
//! the QF_BV query, so a `req false` clause discharges vacuously. For a result-referencing
//! clause the mutation gate incidentally catches it (every mutant survives → WeakContract),
//! but a PARAM-only `@bv` clause or a `@bv` lemma (no body → no mutation) sails through and
//! — post-REQ-8 — certifies L4 with a KERNEL-CHECKED trust label on a vacuous proof.
//!
//! These tests assert the correct (rejected) behavior — the regression guard for the
//! bv-route req-satisfiability vacuity gate (forge/src/bitvector.rs::req_satisfiable).
#![cfg(feature = "bv")]

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}
fn verus_present() -> bool {
    std::env::var("VERUS_BIN")
        .ok()
        .map(|p| Path::new(&p).exists())
        .unwrap_or(false)
        || Command::new("which")
            .arg("verus")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
}
fn check_bv(name: &str, src: &str) -> Vec<Value> {
    // Unique per-test path — cargo runs tests in parallel; a shared temp file races.
    let f = std::env::temp_dir().join(format!("adv_locks_{name}.th"));
    std::fs::write(&f, src).unwrap();
    let out = Command::new(forge_bin())
        .args(["check"])
        .arg(&f)
        .args(["--engine", "bv", "--json"])
        .output()
        .unwrap();
    serde_json::from_slice(&out.stdout).unwrap_or_default()
}
fn reject_cause(c: &Value) -> Option<String> {
    c.get("reject")
        .and_then(|r| r.get("cause"))
        .and_then(|x| x.as_str())
        .map(String::from)
}

/// A `@bv` lemma with an unsatisfiable precondition must not certify — it is vacuous
/// (the §10 anti-Goodhart gaming vector). Regression guard for the bv-route vacuity gate.
#[test]
fn vacuous_bv_lemma_is_rejected_not_certified() {
    if !verus_present() {
        eprintln!("SKIP: verus/z3 absent");
        return;
    }
    let certs = check_bv(
        "lemma",
        "lemma vac_lemma(a: u64, b: u64)\n    req false\n    ens@bv64 a < b\n    proof { }\n",
    );
    let c = certs
        .iter()
        .find(|c| c["item"] == "vac_lemma")
        .expect("vac_lemma cert");
    assert_ne!(
        c["level"],
        Value::from("L4"),
        "a req-false @bv lemma must not certify L4 (it is vacuous): {c}"
    );
    assert!(
        reject_cause(c).is_some(),
        "a vacuous @bv lemma must be rejected (VacuousPrecondition), not silently certified: {c}"
    );
}

/// A param-only `@bv` fn clause under an unsatisfiable precondition must not certify
/// kernel-checked — the mutation gate misses param-only clauses, so the req-SAT gate must
/// catch it. Regression guard for the bv-route vacuity gate.
#[test]
fn vacuous_param_only_bv_clause_is_not_kernel_checked() {
    if !verus_present() {
        eprintln!("SKIP: verus/z3 absent");
        return;
    }
    let certs = check_bv(
        "fn",
        "fn vac2(a: u64, b: u64) -> u64\n    req false\n    ens@bv64 a < b\n    fx pure\n{ 0 }\n",
    );
    let c = certs
        .iter()
        .find(|c| c["item"] == "vac2")
        .expect("vac2 cert");
    let kernel = c["obligations"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|o| o["trust"].as_array().cloned().unwrap_or_default())
        .any(|t| t.as_str().unwrap_or("").to_lowercase().contains("kernel"));
    assert!(!(c["level"] == "L4" && reject_cause(c).is_none() && kernel),
        "a vacuous (req false) param-only @bv clause must not certify L4 with kernel-checked trust: {c}");
}
