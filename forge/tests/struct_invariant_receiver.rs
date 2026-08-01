//! Regression coverage for issue #110. The source fixture's invariant contains
//! two declared struct fields beneath unary `!`. Both user-facing commands must
//! elaborate the canonical receiver-qualified predicate before they can verify
//! or render the anti-Goodhart battery.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("conformance")
        .join("struct-invariant-receiver")
        .join(name)
}

fn verus_present() -> bool {
    if let Ok(path) = std::env::var("VERUS_BIN") {
        if Path::new(&path).exists() {
            return true;
        }
    }
    if let Ok(output) = Command::new("which").arg("verus").output() {
        if output.status.success() && !String::from_utf8_lossy(&output.stdout).trim().is_empty() {
            return true;
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".local/bin/verus").exists();
    }
    false
}

fn unique_cache(tag: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "forge_struct_inv_receiver_{}_{}_{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
        tag
    ))
}

fn run_with_fresh_cache(args: &[&str], tag: &str) -> Output {
    let cache = unique_cache(tag);
    let _ = std::fs::remove_dir_all(&cache);
    let output = Command::new(forge_bin())
        .args(args)
        .env("FORGE_CACHE_DIR", &cache)
        .output()
        .unwrap_or_else(|error| panic!("spawn forge {tag}: {error}"));
    let _ = std::fs::remove_dir_all(cache);
    output
}

#[test]
fn check_and_battery_accept_unary_struct_invariant() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — issue #110 check/battery regression not run.");
        return;
    }

    // The exact issue source now reaches the mutation gate. Its intentionally
    // loose postcondition leaves `panic_latched` unconstrained, so WeakContract
    // is the hand-derived post-vacuity verdict; an E0425/harness error would
    // instead prevent certificate JSON from being emitted.
    let repro = fixture("repro.th");
    let repro_text = repro.to_string_lossy();
    let repro_check = run_with_fresh_cache(
        &["check", &repro_text, "--level", "l3", "--json"],
        "repro-check",
    );
    let repro_certs: Value = serde_json::from_slice(&repro_check.stdout).unwrap_or_else(|error| {
        panic!(
            "issue reproduction must reach certificate emission: {error}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&repro_check.stdout),
            String::from_utf8_lossy(&repro_check.stderr)
        )
    });
    let repro_clear = repro_certs
        .as_array()
        .and_then(|items| {
            items
                .iter()
                .find(|cert| cert.get("item").and_then(Value::as_str) == Some("clear"))
        })
        .unwrap_or_else(|| panic!("missing clear certificate: {repro_certs}"));
    assert_eq!(
        repro_clear
            .get("reject")
            .and_then(|reject| reject.get("cause"))
            .and_then(Value::as_str),
        Some("WeakContract")
    );
    assert!(!String::from_utf8_lossy(&repro_check.stderr).contains("E0425"));

    // The companion keeps the same unary invariant and supplies the preservation
    // fact the mutation battery needs, so both user-facing commands finish clean.
    let path = fixture("accept.th");
    let path_text = path.to_string_lossy();
    let check = run_with_fresh_cache(&["check", &path_text, "--level", "l3", "--json"], "check");
    assert!(
        check.status.success(),
        "forge check must accept the receiver-bound invariant\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&check.stdout),
        String::from_utf8_lossy(&check.stderr)
    );
    let certs: Value = serde_json::from_slice(&check.stdout).unwrap_or_else(|error| {
        panic!(
            "forge check must emit certificate JSON: {error}\nstdout:\n{}",
            String::from_utf8_lossy(&check.stdout)
        )
    });
    let clear = certs
        .as_array()
        .and_then(|items| {
            items
                .iter()
                .find(|cert| cert.get("item").and_then(Value::as_str) == Some("clear"))
        })
        .unwrap_or_else(|| panic!("missing clear certificate: {certs}"));
    assert_eq!(clear.get("level").and_then(Value::as_str), Some("L3"));

    let battery = run_with_fresh_cache(&["battery", &path_text, "clear"], "battery");
    let battery_stdout = String::from_utf8_lossy(&battery.stdout);
    assert!(
        battery.status.success(),
        "forge battery must render after the solver-vacuity harness elaborates\nstdout:\n{battery_stdout}\nstderr:\n{}",
        String::from_utf8_lossy(&battery.stderr)
    );
    assert!(
        battery_stdout.contains("battery — clear"),
        "battery output must name the checked item:\n{battery_stdout}"
    );
}
