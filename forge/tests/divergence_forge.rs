//! acto-critic divergence tests for `forge check` (commit `1004b7a`, issue #5).
//!
//! Each test pins a divergence between the `forge` driver's emitted certificate
//! and the authority chain (`thermite-design.md` §5.1/§5.3/§6, the
//! `.design/forge/*.md` REQs, `conformance/*`). Expected values trace to the
//! design/golden, NEVER to forge's own output (`goal.md` R-CHAR-3).
//!
//! These run the BUILT `forge` binary end-to-end (verus-backed). If verus is
//! absent they SKIP LOUDLY (never panic on a missing solver), matching
//! `check_conformance.rs`.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// `true` iff verus is reachable (mirrors `check_conformance.rs`).
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

/// Run `forge check <file> --json`, returning the parsed array of certificates.
fn check_json(src: &str, stem: &str) -> Vec<Value> {
    let fixture =
        std::env::temp_dir().join(format!("forge_div_{stem}_{}.th", std::process::id()));
    std::fs::write(&fixture, src).expect("write fixture");
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(&fixture)
        .arg("--json")
        .output()
        .expect("spawn forge");
    let _ = std::fs::remove_file(&fixture);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!(
            "forge --json stdout must be one JSON doc: {e}\nstdout:\n{stdout}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stderr)
        )
    });
    value.as_array().expect("array of certs").clone()
}

fn cert_for<'a>(certs: &'a [Value], item: &str) -> &'a Value {
    certs
        .iter()
        .find(|c| c.get("item").and_then(|v| v.as_str()) == Some(item))
        .unwrap_or_else(|| panic!("no cert for `{item}` in {certs:?}"))
}

/// DIVERGENCE 1 — a CORRECT item in a multi-item file is falsely certified as a
/// verification FAILURE (level != L3) when a *different*, contract-independent
/// item in the same file fails verus.
///
/// `good` has `ens result == x + x` over body `x + x` — it holds for all inputs
/// and verus discharges it. `bad` has a false `ens` and fails. Because
/// `check.rs::check_file` runs verus ONCE on the whole lowered crate and then
/// smears the single crate-level `VerusResult` (`assemble_certificate(item,
/// &verus)`) across every item, `good`'s certificate inherits `bad`'s failure.
///
/// Authority: `thermite-design.md` §5.3 — "an edit to `f` cannot invalidate
/// `g`'s certificate unless `g`'s contract references `f`'s contract." Here
/// `good`'s contract references nothing in `bad`, yet `good` is reported non-L3.
/// Also §6 — "The certificate attached to a build artifact lists every
/// function's level"; a correct function MUST list its own (L3) level, not a
/// neighbor's failure. `goal.md` R-SPEC-2 (the certificate is a contract).
/// Tracking: #41
#[test]
fn divergence_multi_item_correct_item_not_falsely_failed() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — multi-item mis-certification not exercised.");
        return;
    }
    let src = "fn good(x: u64) -> u64\n  req x < 1000\n  ens result == x + x\n  fx  pure\n{\n  x + x\n}\n\nfn bad(x: u64) -> u64\n  req x < 1000\n  ens result == x + x + x\n  fx  pure\n{\n  x + x\n}\n";
    let certs = check_json(src, "multi");
    let good = cert_for(&certs, "good");
    // AUTHORITY (§5.3 independence; §6 per-function level): `good`'s contract
    // holds for all inputs and is contract-independent of `bad`, so its level is
    // L3 and it carries no failed obligation. `bad`'s failure must not leak in.
    assert_eq!(
        good["level"],
        Value::from("L3"),
        "a correct, contract-independent item must certify L3 regardless of a sibling's failure (§5.3 / §6); got {good}"
    );
    let obs = good["obligations"].as_array().expect("obligations present");
    assert!(
        obs.iter()
            .all(|o| o.get("status").and_then(|s| s.as_str()) == Some("discharged")),
        "the correct item must carry no failed obligation (no foreign counterexample, §5.1); got {obs:?}"
    );
}

/// DIVERGENCE 2 — the per-obligation counterexample attributed to `good` is in
/// fact the source span of `bad`'s failed clause: a MISLEADING witness. §5.1
/// requires "counterexamples, not adjectives" — a *concrete witness* for the
/// failing obligation. A witness pointing at an unrelated function is worse than
/// an adjective; it asserts a falsehood about `good`.
///
/// Authority: `thermite-design.md` §5.1 (counterexamples are the failing
/// obligation's concrete witness). Tracking: #41
#[test]
fn divergence_multi_item_counterexample_misattributed() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — counterexample misattribution not exercised.");
        return;
    }
    let src = "fn good(x: u64) -> u64\n  req x < 1000\n  ens result == x + x\n  fx  pure\n{\n  x + x\n}\n\nfn bad(x: u64) -> u64\n  req x < 1000\n  ens result == x + x + x\n  fx  pure\n{\n  x + x\n}\n";
    let certs = check_json(src, "multicx");
    let good = cert_for(&certs, "good");
    let obs = good["obligations"].as_array().expect("obligations present");
    // AUTHORITY: a correct item carries no failed-obligation witness at all.
    // (If the multi-item smear is fixed, `good` has only discharged obligations;
    // this assertion then holds. It FAILS today because `good` carries a
    // `postcondition not satisfied` witness that belongs to `bad`.)
    let failed: Vec<&Value> = obs
        .iter()
        .filter(|o| o.get("status").and_then(|s| s.as_str()) == Some("failed"))
        .collect();
    assert!(
        failed.is_empty(),
        "a correct item must carry NO failed-obligation witness (§5.1); the {} witness(es) on `good` are misattributed from `bad`: {failed:?}",
        failed.len()
    );
}
