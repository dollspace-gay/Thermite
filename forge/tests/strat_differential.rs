//! The stratified-cage classifier differential battery, live (`.design/stage2-stratified-cage.md`
//! REQ-4 / AC-4; audit check [8]). This is the lake-gated integration test that holds the
//! Rust admission classifier (`thermite_spec::classifier`) byte-equal to the Lean kernel
//! `Thermite.Strat.Cls.admitted` over a generated formula stream.
//!
//! It runs the shipped `forge strat-tv` command (the same path the scheduled rotating-seed
//! job and the audit check [8] gate drive), which spawns `lake env lean --run
//! Thermite/Strat/Cls/Wire.lean`. When `lake` is absent (a non-Lean machine) it SELF-SKIPS
//! (the `forge strat-tv` command itself reports a skip and exits 0); this test additionally
//! guards so it only asserts the live agreement where the Lean toolchain exists — the
//! `lean` CI job, where `cargo nextest run -p forge` runs with the spine built. There the
//! `lake_present()` branch is taken and the differential is genuinely exercised.

use std::path::PathBuf;
use std::process::Command;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// Mirrors `engine::LeanEngine::lake_binary` / the `lean_engine.rs` guard: the
/// elan-managed lake if present, else `lake` on PATH; `None` ⇒ skip the live assertion.
fn lake_present() -> bool {
    if let Ok(home) = std::env::var("HOME") {
        if PathBuf::from(home).join(".elan/bin/lake").exists() {
            return true;
        }
    }
    Command::new("lake")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// AC-4 (the headline): the Rust classifier returns the SAME verdict as the Lean
/// `admitted` on N generated formulas, with zero disagreements. Run at the pinned default
/// seed (reproducible); a disagreement exits the `forge strat-tv` command non-zero (the
/// hard CI failure check [8] raises), failing this test with the verbatim finding.
#[test]
fn rust_classifier_matches_lean_admitted_on_generated_formulas() {
    if !lake_present() {
        eprintln!(
            "SKIP: lake not available — the stratified classifier differential is not run \
             (install Lean/elan to exercise it; the `lean` CI job runs it live)."
        );
        return;
    }
    let out = Command::new(forge_bin())
        .arg("strat-tv")
        .arg("--generated")
        .arg("200")
        .arg("--json")
        .output()
        .expect("spawn forge strat-tv");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(0),
        "forge strat-tv must exit 0 (zero disagreements, zero tripwire).\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    let doc: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("forge strat-tv --json must emit a JSON object");
    // A skip (lake vanished between the guard and the spawn) is acceptable; otherwise the
    // run must have classified the full batch with zero disagreements.
    if doc.get("skipped").and_then(|v| v.as_bool()) == Some(true) {
        eprintln!("SKIP: forge strat-tv reported lake absent at run time");
        return;
    }
    assert_eq!(
        doc["checked"].as_u64(),
        Some(200),
        "the battery must classify all 200 generated formulas: {doc}"
    );
    assert_eq!(
        doc["disagreements"].as_array().map(Vec::len),
        Some(0),
        "ZERO disagreements required (Rust verdict == Lean admitted): {doc}"
    );
    assert_eq!(
        doc["tripwire_unknown_on_admitted"].as_u64(),
        Some(0),
        "the unknown-on-admitted tripwire must be 0 (no classifier-suspect formula): {doc}"
    );
    assert_eq!(
        doc["passed"].as_bool(),
        Some(true),
        "the battery must pass: {doc}"
    );
}

/// AC-4 (the watchdog dimension): a DIFFERENT seed walks a different slice of the clause
/// space and must also agree completely — the rotating-seed property the scheduled CI job
/// relies on (a seed-dependent classifier divergence would surface as a red build).
#[test]
fn rust_classifier_matches_lean_on_a_rotated_seed() {
    if !lake_present() {
        eprintln!("SKIP: lake not available — rotated-seed differential not run.");
        return;
    }
    let out = Command::new(forge_bin())
        .arg("strat-tv")
        .arg("--generated")
        .arg("120")
        .arg("--seed")
        .arg("424242")
        .arg("--json")
        .output()
        .expect("spawn forge strat-tv");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        out.status.code(),
        Some(0),
        "rotated-seed strat-tv must exit 0.\nstdout:\n{stdout}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    if let Ok(doc) = serde_json::from_str::<serde_json::Value>(stdout.trim()) {
        if doc.get("skipped").and_then(|v| v.as_bool()) == Some(true) {
            return;
        }
        assert_eq!(
            doc["disagreements"].as_array().map(Vec::len),
            Some(0),
            "ZERO disagreements on the rotated seed: {doc}"
        );
    }
}
