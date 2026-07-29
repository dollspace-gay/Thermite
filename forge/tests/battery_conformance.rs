//! The live cert-oracle for the forge-tier frozen battery (`.design/stage1-forge-tier.md`
//! REQ-5 + AC-9, increment 2c). It drives the built `forge` binary with `check --json`
//! over the `conformance/battery/` fixtures and asserts the elaboration-time refusal —
//! a proof citing an unlisted tactic OR an unlisted simp lemma is REFUSED, named.
//!
//! AC-9 is exercised here at the certificate level (the battery's verus-free logic — the
//! frozen registry, the citation scanner, the gate, the `Stuck` producer — is pinned by
//! the `battery` unit tests; this is the end-to-end binary surface):
//!   - `lemma_unlisted_tactic` — a `lemma` proof citing `apply` (not in the frozen
//!     allowlist) is refused with `BatteryUnlistedTactic`, the name `apply` in the error.
//!   - `lemma_unlisted_simp_lemma` — a proof citing `simp [melems_cons]` (the RFC-1 §8
//!     merge bridge, not in the frozen simp set) is refused with `BatteryUnlistedSimpLemma`,
//!     the name `melems_cons` in the error.
//!   - `lemma_clean` — a proof citing only frozen tactics (`omega`) passes the battery and
//!     falls through to the inert forge-item skip (no v1 cert consumer yet — proof-view
//!     discharge is 2e), so it emits no certificate.
//!
//! `forge check` resolves the verus version before the per-item loop, so these checks
//! need verus present even though the battery short-circuits before any lowering/verus
//! run; they skip with a logged note when verus is absent (mirroring
//! `covenant_conformance.rs`), never panicking on a missing solver. `tests/` is not
//! anti-pattern-gated, so `unwrap`/`expect` are fine here.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn battery_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("conformance")
        .join("battery")
}

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// `true` iff verus can be located (`VERUS_BIN`, then PATH, then `~/.local/bin/verus`)
/// — mirrors `covenant_conformance.rs`. Skips with a logged note otherwise.
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

fn skip_note() {
    eprintln!(
        "SKIP: verus not available — `forge check` resolves the verus version before the \
         per-item loop (set VERUS_BIN or install verus on PATH). The battery logic itself \
         is pinned verus-free by the `battery` unit tests."
    );
}

/// AC-9: a proof citing an unlisted tactic is refused, named, before any discharge.
#[test]
fn unlisted_tactic_is_refused_with_name() {
    if !verus_present() {
        skip_note();
        return;
    }
    let (code, certs) = run_check_json(&battery_dir().join("lemma_unlisted_tactic.th"));
    assert_ne!(code, Some(0), "a battery refusal must exit non-zero");
    assert_eq!(certs.len(), 1, "the fixture is a single lemma: {certs:?}");
    let cert = &certs[0];
    assert_eq!(cert["item"], Value::from("add_id"));
    assert_eq!(cert["level"], Value::from("L0"));
    assert_eq!(
        cert["reject"]["cause"],
        Value::from("BatteryUnlistedTactic")
    );
    assert_eq!(
        cert["lowered_assurance"],
        Value::from(false),
        "a battery refusal is a hard fail, never a lowered-assurance degrade"
    );
    let detail = cert["reject"]["detail"].as_str().expect("a detail string");
    assert!(
        detail.contains("apply"),
        "the refusal NAMES the offending tactic (AC-9): {detail}"
    );
}

/// AC-9: a proof citing an unlisted SIMP lemma is refused, named, before any discharge.
#[test]
fn unlisted_simp_lemma_is_refused_with_name() {
    if !verus_present() {
        skip_note();
        return;
    }
    let (code, certs) = run_check_json(&battery_dir().join("lemma_unlisted_simp_lemma.th"));
    assert_ne!(code, Some(0), "a battery refusal must exit non-zero");
    assert_eq!(certs.len(), 1, "the fixture is a single lemma: {certs:?}");
    let cert = &certs[0];
    assert_eq!(cert["item"], Value::from("add_id"));
    assert_eq!(cert["level"], Value::from("L0"));
    assert_eq!(
        cert["reject"]["cause"],
        Value::from("BatteryUnlistedSimpLemma")
    );
    let detail = cert["reject"]["detail"].as_str().expect("a detail string");
    assert!(
        detail.contains("melems_cons"),
        "the refusal NAMES the offending simp lemma (AC-9): {detail}"
    );
}

/// A proof citing only frozen tactics passes the battery and falls through to the inert
/// forge-item skip (no v1 cert consumer yet) — it emits no certificate, no refusal.
#[test]
fn clean_proof_is_not_refused() {
    if !verus_present() {
        skip_note();
        return;
    }
    let (code, certs) = run_check_json(&battery_dir().join("lemma_clean.th"));
    assert_eq!(
        code,
        Some(0),
        "a clean forge-only file certifies nothing and exits zero"
    );
    assert!(
        certs.is_empty(),
        "a battery-clean forge item emits no cert (inert skip until proof-view 2e): {certs:?}"
    );
}
