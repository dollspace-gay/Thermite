//! The permanent, CI-runnable Verus proof of Thermite's soundness-critical core
//! (epic #60, `.design/verified/self-verification.md` REQ-6 / AC-1 / AC-2 / AC-6).
//!
//! Runs the REAL `verus --no-cheating` on the verified crate's `verus!{}` core
//! (the `subsumes` exec fn + the `spec_subsumes` subset relation + the three
//! lattice-law `proof fn`s) and asserts `verified, 0 errors` (REQ-4: no
//! `assume`/`admit`/`external_body` — `--no-cheating` enforces it; AC-1: N ≥ 4).
//! A core fn that fails to verify is a HARD test failure, not a skip (R-DEFER-6).
//!
//! The verus-invocation pattern (env override → PATH → `~/.local/bin/verus`,
//! skip-LOUD if absent, check exit status + stdout, run in a temp dir so no
//! scratch lands in the tree) MIRRORS `thermite-lower/tests/lower_conformance.rs`
//! (R-CODE-4: exit status checked, never swallowed; #53: no temp pollution).
//! `unwrap`/`expect` are fine here — `tests/` is not anti-pattern-gated.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The verified crate's `src/lib.rs` — the file `verus` checks. The `verus!{}`
/// core is behind `#[cfg(verus_keep_ghost)]`, which the `verus` driver sets, so
/// `verus src/lib.rs` compiles and verifies the proof while a normal `cargo
/// build` skips it.
fn lib_rs() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("lib.rs")
}

/// Locate the `verus` binary: `VERUS_BIN` env override, then PATH (`which`), then
/// `~/.local/bin/verus`. `None` ⇒ verus genuinely absent ⇒ the caller SKIPs
/// LOUDLY (the suite must run where verus is not installed, e.g. CI without the
/// toolchain). MIRRORS `lower_conformance::verus_bin`.
fn verus_bin() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("VERUS_BIN") {
        let pb = PathBuf::from(p);
        if pb.exists() {
            return Some(pb);
        }
    }
    if let Ok(out) = Command::new("which").arg("verus").output() {
        if out.status.success() {
            let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !p.is_empty() {
                return Some(PathBuf::from(p));
            }
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let cand = PathBuf::from(home).join(".local/bin/verus");
        if cand.exists() {
            return Some(cand);
        }
    }
    None
}

/// Run `verus --no-cheating --crate-type=lib <file>`; `None` ⇒ verus unavailable
/// (caller SKIPs). `--crate-type=lib` (forwarded to rustc) tells verus the file
/// is a LIBRARY crate root, so it does not demand a `main` (the file is the
/// crate's real `src/lib.rs`). Working dir is the temp dir so the compiled-crate
/// artifact lands there, not in the repo tree (#53 — no scratch pollution).
fn run_verus(file: &Path) -> Option<(bool, String)> {
    let bin = verus_bin()?;
    let out = Command::new(bin)
        .arg("--no-cheating")
        .arg("--crate-type=lib")
        .arg(file)
        .current_dir(std::env::temp_dir())
        .output()
        .ok()?;
    let mut combined = String::from_utf8_lossy(&out.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&out.stderr));
    Some((out.status.success(), combined))
}

/// AC-1 / AC-6: `verus --no-cheating src/lib.rs` verifies the core with 0 errors.
#[test]
fn verified_core_passes_verus_no_cheating() {
    let lib = lib_rs();
    match run_verus(&lib) {
        Some((ok, output)) => {
            assert!(
                ok && output.contains("0 errors"),
                "verus --no-cheating on the verified core did NOT verify \
                 (R-DEFER-6 HARD gate). exit_success={ok}\n--- verus output ---\n{output}"
            );
            assert!(
                output.contains("verified, 0 errors"),
                "verus output missing the expected `verified, 0 errors` line:\n{output}"
            );
        }
        None => eprintln!(
            "SKIP: verus not available — self-verification proof of the soundness-critical \
             core NOT run (set VERUS_BIN or install verus on PATH). The exhaustive \
             equivalence test in thermite-lower still anchors effects::subsumes."
        ),
    }
}

/// AC-2 (non-triviality): mutating the proved `subsumes` body (`missing == 0` →
/// `missing != 0`) makes the SAME `verus --no-cheating` run report `errors: 1`
/// (postcondition not satisfied). This proves the `ensures result ==
/// spec_subsumes(..)` is genuinely constraining, NOT vacuous (REQ-4 / R-DEFER-9).
/// The mutant is written to a TEMP copy of `lib.rs` (never edits the tree).
#[test]
fn broken_subsumes_fails_verification() {
    if verus_bin().is_none() {
        eprintln!("SKIP: verus not available — non-triviality (AC-2) demonstration not run.");
        return;
    }
    let src = std::fs::read_to_string(lib_rs()).expect("read verified lib.rs");
    // The proved exec body's last statement. Negating it must break the proof.
    let from = "    let missing = callee & !caller;\n        missing == 0\n";
    let to = "    let missing = callee & !caller;\n        missing != 0\n";
    assert!(
        src.contains(from),
        "the proved `subsumes` body shape changed — update the mutation point \
         (AC-2 must mutate the REAL body):\n{src}"
    );
    let mutated = src.replacen(from, to, 1);
    assert_ne!(mutated, src, "mutation must change the source");
    let tmp = std::env::temp_dir().join("thermite_verified_broken_lib.rs");
    std::fs::write(&tmp, &mutated).expect("write mutated temp lib.rs");
    match run_verus(&tmp) {
        Some((ok, output)) => {
            assert!(
                !ok || !output.contains(", 0 errors"),
                "the BROKEN `subsumes` (missing != 0) MUST fail verification \
                 (non-vacuous contract, AC-2) but verus reported success:\n{output}"
            );
            assert!(
                output.contains("1 errors") || output.contains("error"),
                "the broken variant should report a postcondition error:\n{output}"
            );
        }
        None => eprintln!("SKIP: verus disappeared mid-test — AC-2 not run."),
    }
    let _ = std::fs::remove_file(&tmp);
}
