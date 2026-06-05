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

/// Mutate `lib.rs` (a TEMP copy), write it, and assert `verus --no-cheating`
/// reports an error (non-vacuity). `from` must appear EXACTLY once in the source —
/// a shape change fails LOUDLY (the mutation point must be the REAL proved body).
fn assert_mutation_fails(label: &str, from: &str, to: &str) {
    if verus_bin().is_none() {
        eprintln!("SKIP: verus not available — {label} non-triviality not run.");
        return;
    }
    let src = std::fs::read_to_string(lib_rs()).expect("read verified lib.rs");
    assert!(
        src.contains(from),
        "the proved body shape changed — update the {label} mutation point \
         (the mutation must hit the REAL body):\nlooked for:\n{from}"
    );
    let mutated = src.replacen(from, to, 1);
    assert_ne!(mutated, src, "mutation must change the source ({label})");
    let tmp = std::env::temp_dir().join(format!("thermite_verified_broken_{label}.rs"));
    std::fs::write(&tmp, &mutated).expect("write mutated temp lib.rs");
    match run_verus(&tmp) {
        Some((ok, output)) => {
            assert!(
                !ok || !output.contains(", 0 errors"),
                "the BROKEN {label} MUST fail verification (non-vacuous contract) \
                 but verus reported success:\n{output}"
            );
            assert!(
                output.contains("error"),
                "the broken {label} should report a verus error:\n{output}"
            );
        }
        None => eprintln!("SKIP: verus disappeared mid-test — {label} not run."),
    }
    let _ = std::fs::remove_file(&tmp);
}

/// AC-7b (non-vacuity, REQ-7): mutating `ladder_action_l3`'s `Counterexample` arm
/// from `HardFail` to `DegradeToL1` (a counterexample DEGRADES — the exact cheat
/// R-DEFER-9 forbids) makes the anti-cheat `ensures`
/// `l3_is_counterexample(v) ==> (r is HardFail) && !is_degrade(r)` fail. This
/// proves the anti-cheat invariant is genuinely constraining, not vacuous.
#[test]
fn broken_ladder_action_counterexample_degrades_fails() {
    assert_mutation_fails(
        "ladder_action",
        "        L3Tag::Counterexample => LadderAction::HardFail,\n        }\n    }\n\n    /// The L2 ladder DECISION",
        "        L3Tag::Counterexample => LadderAction::DegradeToL1,\n        }\n    }\n\n    /// The L2 ladder DECISION",
    );
}

/// AC-8b (non-vacuity #1, REQ-8): mutating `widen`'s non-widening `else` arm so a
/// non-widening atom (Alloc/Panic/Diverge) leaks `openat` makes `pure_has_no_io`
/// (and `non_widening_atoms_have_no_io`) fail. PURE-NO-I/O is genuinely constraining.
#[test]
fn broken_widen_leaks_openat_fails_pure_no_io() {
    assert_mutation_fails(
        "widen_leak",
        "        else if i == 4 { 8u32 }       // Rand → getrandom\n        else { 0u32 }",
        "        else if i == 4 { 8u32 }       // Rand → getrandom\n        else { 1u32 }",
    );
}

/// AC-8b (non-vacuity #2, REQ-8): mutating the `io_allow` spec fold to use XOR
/// (`^`) instead of OR (`|`) so a `Write` atom cancels a `Read` atom's `openat`
/// (non-monotone) makes the `monotone` lemma fail. MONOTONICITY is genuinely
/// constraining (adding an effect must never remove a permitted syscall).
#[test]
fn broken_io_allow_xor_fails_monotone() {
    assert_mutation_fails(
        "io_allow_xor",
        "        (if fx_has(fx, 0) { widen(0) } else { 0u32 })\n        | (if fx_has(fx, 1) { widen(1) } else { 0u32 })",
        "        (if fx_has(fx, 0) { widen(0) } else { 0u32 })\n        ^ (if fx_has(fx, 1) { widen(1) } else { 0u32 })",
    );
}

/// AC-9b (non-vacuity, REQ-9 / Target C): mutating the `should_emit_external_body`
/// exec body to `true` (a REGULAR fn WOULD get external_body — the exact §9
/// laundering R-DEFER-9 forbids) makes the soundness corollary
/// `(!has_boundary && !has_slag) ==> !r` fail. The boundary HONESTY gate is
/// genuinely constraining (a regular fn is NEVER laundered to an assumed-L3 sig).
#[test]
fn broken_should_emit_external_body_true_fails() {
    assert_mutation_fails(
        "should_emit_external_body_true",
        "            (!has_boundary && !has_slag) ==> !r,\n    {\n        has_boundary || has_slag\n    }",
        "            (!has_boundary && !has_slag) ==> !r,\n    {\n        true\n    }",
    );
}

/// AC-10b (non-vacuity, REQ-10 / Target D): mutating `min2` to pick the MAX
/// (`rank(a) >= rank(b)` instead of `<=` — an OVER-CLAIM: the project would be as
/// strong as its STRONGEST fn) makes `aggregate_le_all` (D1, "≤ every fn") fail.
/// The no-over-claim min is genuinely constraining (§5.2).
#[test]
fn broken_aggregate_max_fails_le_all() {
    assert_mutation_fails(
        "aggregate_max",
        "    pub open spec fn min2(a: Level, b: Level) -> Level {\n        if rank(a) <= rank(b) { a } else { b }\n    }",
        "    pub open spec fn min2(a: Level, b: Level) -> Level {\n        if rank(a) >= rank(b) { a } else { b }\n    }",
    );
}

/// AC-11b (non-vacuity — the #48 property, REQ-11 / Target E): dropping the
/// `scored > 0` guard from the exec body (so a `0/0` score passes:
/// `0 * 100 >= 0 * 60`) makes the `scored == 0 ==> !r` anti-Goodhart `ensures`
/// fail. The #48 floor gate is genuinely constraining (a `0/0` never passes).
#[test]
fn broken_meets_floor_drops_scored_guard_fails() {
    assert_mutation_fails(
        "meets_floor_drops_guard",
        "        scored == 0 ==> !r,\n    {\n        scored > 0 && killed * 100 >= scored * 60\n    }",
        "        scored == 0 ==> !r,\n    {\n        killed * 100 >= scored * 60\n    }",
    );
}
