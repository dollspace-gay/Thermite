//! Divergence pin (critic audit of #193, commit 70a65bdb): `forge battery` —
//! the §7 anti-Goodhart battery VIEW — reports a gate-REJECTED vacuous contract
//! as **"non-vacuous (tautology=false, vacuous_precondition=false)"**.
//!
//! `check::gate_fn` rejects an `ens true` item at structural triage
//! (`vacuity::triage`, §7.1 (a)) via `Certificate::rejected`, which keeps
//! `contract_quality` at `ContractQuality::forward_declared()` (both bools
//! `false` — they are NOT-ASSERTED placeholders, not a clean verdict); the
//! REAL verdict lives in `Certificate.reject` (`cause: EnsIsTrivial`).
//! `goal_repl::render_battery_item` reads ONLY the two `contract_quality`
//! bools and never consults `cert.reject`, so the battery view renders the
//! §7.1-(a)-rejected contract as "non-vacuous" — the exact degenerate-contract
//! lie the battery exists to prevent. (`goal_repl::render_goal_item` gets this
//! right: it reports `NOT CERTIFIED — EnsIsTrivial` for the same cert.)
//!
//! AUTHORITY:
//! - `thermite-design.md` §7: "A function does not certify until its
//!   **contract** certifies"; §7.1 (a): "`ens` simplifies to `true` → reject".
//! - `.design/forge/goal-repl.md` REQ-1: `forge battery` "reports the §7
//!   anti-Goodhart battery ... WITHOUT re-defining any verdict. A thin VIEW
//!   over the existing per-item pipeline." The pipeline's verdict for this
//!   program is REJECTED `EnsIsTrivial`; the view re-defines it to non-vacuous.
//! - `conformance/vacuity/triage.json` `reject[]` entry `ens_is_true`
//!   (`cause: EnsIsTrivial`, program `fn f() -> () req true ens true fx pure { }`)
//!   — the hand-derived §7.1 oracle this test loads its input AND expected
//!   cause from (R-CHAR-3: nothing here is copied from the verb's own output).
//!
//! TOOLCHAIN (symbol anchors): `goal_repl::render_battery_item` (reads
//! `cert.contract_quality.{tautology,vacuous_precondition}` only),
//! `manifest::Certificate::rejected` (`contract_quality:
//! ContractQuality::forward_declared()`), `check::gate_fn` (the §7.1 reject).
//!
//! Expected: the battery render for the triage-rejected item surfaces the
//! gate's vacuity verdict (the `EnsIsTrivial` cause / an explicit VACUOUS
//! marker) and does NOT describe the contract as "non-vacuous".
//! Actual: `vacuity: non-vacuous (tautology=false, vacuous_precondition=false)`.
//!
//! No verus needed: the §7.1 (a) reject short-circuits BEFORE lowering/verus.
//!
//! Committed FAILING — the block until the generator fixes (tracking #194).

use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// The `ens_is_true` reject fixture from the hand-derived §7.1 triage oracle:
/// `(program, expected_cause)` — the input and the expected verdict both come
/// from `conformance/vacuity/triage.json`, never from the verb (R-CHAR-3).
fn triage_ens_is_true_fixture() -> (String, String) {
    let oracle = repo_root().join("conformance/vacuity/triage.json");
    let raw = std::fs::read_to_string(&oracle).expect("read conformance/vacuity/triage.json");
    let json: serde_json::Value = serde_json::from_str(&raw).expect("parse triage.json");
    let entry = json["reject"]
        .as_array()
        .expect("triage.json reject[]")
        .iter()
        .find(|e| e["name"] == "ens_is_true")
        .expect("triage.json reject[] entry `ens_is_true`");
    (
        entry["program"].as_str().expect("program").to_string(),
        entry["cause"].as_str().expect("cause").to_string(),
    )
}

// `.design/forge/goal-repl.md` REQ-1 / `thermite-design.md` §7.1 (a): the
// battery VIEW on a gate-rejected vacuous contract must report the gate's
// verdict (the reject), never "non-vacuous".
#[test]
fn battery_view_reports_gate_vacuity_reject() {
    let (program, expected_cause) = triage_ens_is_true_fixture();

    let tmp = std::env::temp_dir().join(format!(
        "forge_divergence_battery_vacuous_{}.th",
        std::process::id()
    ));
    std::fs::write(&tmp, &program).expect("write vacuous fixture");

    let out = Command::new(env!("CARGO_BIN_EXE_forge"))
        .args(["battery", tmp.to_str().unwrap()])
        .output()
        .expect("spawn forge battery");
    let _ = std::fs::remove_file(&tmp);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    // The gate's verdict for this program is REJECTED (§7.1 (a) `EnsIsTrivial`).
    // The battery view must NOT describe the contract as non-vacuous.
    assert!(
        !stdout.contains("non-vacuous"),
        "forge battery describes a §7.1-(a)-rejected (`ens true`) contract as \
         non-vacuous — the view re-defines the gate's verdict (goal-repl.md REQ-1); \
         stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    // ... and must surface the vacuity verdict itself (the oracle's cause, or an
    // explicit VACUOUS marker once "non-vacuous" occurrences are discounted).
    let without_negation = stdout.replace("non-vacuous", "");
    assert!(
        without_negation.contains(&expected_cause)
            || without_negation.to_lowercase().contains("vacuous"),
        "forge battery never surfaces the gate's vacuity reject \
         (`{expected_cause}`, conformance/vacuity/triage.json `ens_is_true`); \
         stdout:\n{stdout}\nstderr:\n{stderr}"
    );
}
