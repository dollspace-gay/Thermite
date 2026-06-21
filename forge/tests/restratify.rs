//! `forge edit --restratify` — the restratification rewrite, end to end
//! (`.design/stage2-stratified-cage.md` REQ-7 / AC-7). Drives the built `forge` binary
//! and asserts the §6 kv-alternation example is rewritten and certified IN-CAGE, plus the
//! R-SIDE-1 withheld-certification discipline (certification is WITHHELD when `Side` is
//! undischarged). The certification logic itself is the pure-Rust
//! `thermite_spec::restratify` (unit-tested in-crate + the Lean `restrat_conservative` /
//! `PinRestratDropSide`); this test pins the CLI WIRING.

use std::process::Command;

use serde_json::Value;
use thermite_spec::restratify::{certify, kv_example, Certification, WithheldReason};

fn forge_bin() -> &'static str {
    env!("CARGO_BIN_EXE_forge")
}

/// AC-7 — the end-to-end rewrite: original REJECTED (cycle), φ' and `Side` both ADMITTED,
/// `Side` discharged in-cage, φ CERTIFIED. The JSON report attests each step.
#[test]
fn restratify_certifies_kv_example_end_to_end() {
    let out = Command::new(forge_bin())
        .args(["edit", "--restratify", "--json"])
        .output()
        .expect("run forge edit --restratify --json");
    assert!(
        out.status.success(),
        "exit must be 0 (certified); stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let doc: Value = serde_json::from_slice(&out.stdout).expect("valid JSON report");

    assert_eq!(doc["original"]["verdict"], "rejected:sort-graph-cycle");
    assert_eq!(doc["rewritten"]["verdict"], "admitted");
    assert_eq!(doc["side"]["verdict"], "admitted");
    assert_eq!(doc["side_discharged"], Value::Bool(true));
    assert_eq!(doc["certified"], Value::Bool(true));
    // R-SIDE-1 is attested by the report: undischarged Side ⇒ withheld.
    assert_eq!(doc["withheld_when_side_undischarged"], Value::Bool(true));
}

/// The human-readable mode runs the same path and exits 0 (certified).
#[test]
fn restratify_human_mode_succeeds() {
    let out = Command::new(forge_bin())
        .args(["edit", "--restratify"])
        .output()
        .expect("run forge edit --restratify");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("CERTIFIED"));
    assert!(stdout.contains("WITHHELD"));
}

/// `forge edit --restratify` takes no positionals — a stray argument is a usage error
/// (exit 2), never a panic.
#[test]
fn restratify_rejects_positional_args() {
    let out = Command::new(forge_bin())
        .args(["edit", "--restratify", "some_file.th"])
        .output()
        .expect("run forge edit --restratify with a stray arg");
    assert_eq!(out.status.code(), Some(2), "usage error is exit 2");
}

/// AC-7 — the WITHHELD-CERTIFICATION discipline (R-SIDE-1), at the certification API: φ'
/// is admitted, but with `Side` UNDISCHARGED the φ-certificate is WITHHELD. This is the
/// mis-certification that dropping `Side` would permit — the Lean `PinRestratDropSide`
/// mirror.
#[test]
fn certification_withheld_when_side_undischarged() {
    let phi = kv_example();

    // Side discharged in-cage ⇒ certified.
    assert!(certify(&phi, true).is_certified());

    // Side undischarged ⇒ WITHHELD, with the R-SIDE-1 reason.
    let withheld = certify(&phi, false);
    assert!(!withheld.is_certified());
    assert!(matches!(
        withheld,
        Certification::Withheld(WithheldReason::SideUndischarged, Some(_))
    ));
}
