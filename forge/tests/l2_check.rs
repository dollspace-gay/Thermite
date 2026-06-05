//! Integration test for `forge check --level l2` against the EXTERNAL truth: the
//! real `cargo kani` binary (`.design/lower/l2-kani.md` AC-1, REQ-7). It runs the
//! `forge` binary with `--level l2 --json` on `conformance/sum.th` and asserts the
//! `sum` certificate is `Level::L2` (verified up to bound), distinct from the
//! default L3 verus path.
//!
//! `cargo kani` is a HEAVY external toolchain, so the kani-spawning assertion
//! SKIPS LOUDLY (a diagnostic + early return, NOT `#[ignore]`) when kani is
//! absent (REQ-8), mirroring the verus-absent skip pattern. The L2 level / the
//! `slice <= 4, unwind 5` bound caveat trace to the grounded real-kani runs
//! (R-CHAR-3), not forge's own output. `unwrap`/`expect` are fine here
//! (`tests/` is not anti-pattern-gated).

use std::path::PathBuf;
use std::process::Command;

fn corpus(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("conformance")
        .join(format!("{name}.th"))
}

/// `true` if the kani plugin binary is resolvable (PATH or `~/.cargo/bin`), so
/// the kani-spawning test SKIPS LOUDLY when absent rather than failing (REQ-8).
fn kani_available() -> bool {
    if let Ok(out) = Command::new("which").arg("cargo-kani").output() {
        if out.status.success() && !out.stdout.is_empty() {
            return true;
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        if PathBuf::from(home).join(".cargo/bin/cargo-kani").exists() {
            return true;
        }
    }
    false
}

/// Run the `forge` binary (the build artifact via `CARGO_BIN_EXE_forge`) with the
/// given args, returning `(success, stdout, stderr)`.
fn run_forge(args: &[&str]) -> (bool, String, String) {
    let exe = env!("CARGO_BIN_EXE_forge");
    let out = Command::new(exe)
        .args(args)
        .output()
        .expect("spawn forge binary");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

// AC-1 / REQ-7: `forge check --level l2 conformance/sum.th` certifies `sum` at
// Level::L2 (the Kani bounded model check), distinct from the default L3 path.
#[test]
fn forge_check_level_l2_sum_is_l2() {
    if !kani_available() {
        eprintln!(
            "SKIP: kani not available — `forge check --level l2` live path not run \
             (install cargo kani). The pure parse/emitter paths are covered elsewhere."
        );
        return;
    }
    let path = corpus("sum");
    let path_str = path.to_str().expect("utf8 path");
    let (ok, stdout, stderr) = run_forge(&["check", "--level", "l2", "--json", path_str]);
    assert!(
        ok,
        "`forge check --level l2 sum.th` must exit 0 (sum verifies to bound).\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    // The JSON cert array on stdout: parse it and find `sum`.
    let certs: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("forge --json emits a JSON cert array");
    let arr = certs.as_array().expect("cert array");
    let sum = arr
        .iter()
        .find(|c| c.get("item").and_then(|v| v.as_str()) == Some("sum"))
        .expect("a `sum` certificate is present");
    assert_eq!(
        sum.get("level").and_then(|v| v.as_str()),
        Some("L2"),
        "`sum`'s --level l2 cert is L2 (verified up to bound), not L3:\n{stdout}"
    );
    // The bound caveat is recorded on the discharged obligation (REQ-6 / AC-6).
    let obligations = sum
        .get("obligations")
        .and_then(|v| v.as_array())
        .expect("obligations");
    assert!(
        obligations.iter().any(|o| o
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .contains("slice <= 4")),
        "the L2 cert states the bound caveat (slice <= 4):\n{stdout}"
    );
}

// REQ-7: the DEFAULT (no --level) stays the L3 verus path — distinct from L2.
// This is a pure CLI-surface assertion (no kani spawn): we only assert the flag
// dispatch differs by checking the usage banner rejects a bogus --level value.
#[test]
fn bogus_level_value_is_usage_error() {
    let path = corpus("sum");
    let path_str = path.to_str().expect("utf8 path");
    let (ok, _stdout, stderr) = run_forge(&["check", "--level", "l9", path_str]);
    assert!(!ok, "a bogus --level value must not exit 0");
    assert!(
        stderr.contains("--level") || stderr.contains("usage"),
        "the bogus --level value is a usage error:\n{stderr}"
    );
}
