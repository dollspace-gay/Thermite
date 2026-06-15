//! Blocker #53 regression: `forge check`'s verus path must not orphan its
//! per-run scratch directory (the `.rs` source or verus's ~4.3M compiled-binary
//! sibling) in the shared temp dir, on either the success path or the verus
//! error (counterexample) path. The v0.1 driver ran verus with `current_dir =
//! std::env::temp_dir()` and removed only the `.rs` source, so a verus run that
//! errored mid-compile orphaned the binary → unbounded `/tmp` growth under
//! sustained multi-agent fresh-verification (the ENOSPC seen during #18/#20).
//!
//! The expected behavior traces to `crosslink issue #53` (the authority): after a
//! `forge check`, no `forge_*` scratch entry the run created survives in the temp
//! dir. We assert it by pointing the spawned `forge` at its own isolated `TMPDIR`
//! (so `std::env::temp_dir()` inside `forge` resolves there, immune to parallel
//! tests), then requiring that dir hold no `forge_*` entry afterward, on both the
//! success corpus program and a counterexample fixture.
//!
//! These checks run verus. If verus is absent they skip with an eprintln (mirroring
//! `check_conformance.rs`) rather than panic on a missing solver. `tests/` is not
//! anti-pattern-gated, so `unwrap`/`expect` are fine here.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("conformance")
}

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// `true` iff verus can be located (`VERUS_BIN`, then PATH, then
/// `~/.local/bin/verus`); mirrors `check_conformance.rs`. Skip with an eprintln
/// otherwise.
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

/// A fresh per-test isolated temp dir (pid + monotonic counter, so it never
/// collides with a parallel test). `forge` is pointed at it via `TMPDIR`, so every
/// `forge_*` entry inside it belongs to this run.
fn isolated_tmpdir() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("forge53_isolated_{}_{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create isolated tmpdir");
    dir
}

/// The `forge_*` entries (files or directories) currently in `dir`: the leak
/// surface. Both the v0.1 orphaned binary (`forge_<stem>_<pid>_<n>`, no
/// extension) and the new per-run scratch dir (same prefix) match this glob, so a
/// leak of either shape is caught.
fn forge_entries_in(dir: &Path) -> Vec<PathBuf> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    read.flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("forge_"))
        .map(|e| e.path())
        .collect()
}

/// Run `forge check <file>` with `TMPDIR` pointed at `tmpdir` (so `forge`'s
/// `std::env::temp_dir()` resolves there). Returns the exit code.
fn run_check_in(file: &Path, tmpdir: &Path) -> Option<i32> {
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(file)
        .env("TMPDIR", tmpdir)
        .output()
        .unwrap_or_else(|e| panic!("spawn forge: {e}"));
    out.status.code()
}

/// Assert that running `forge check <file>` (in an isolated `TMPDIR`) leaves no
/// `forge_*` entry behind there: the per-run scratch dir and verus's
/// compiled-binary sibling were removed wholesale (blocker #53). Returns the exit
/// code. Cleans up the isolated dir itself afterward.
fn assert_no_scratch_leak(file: &Path, label: &str) -> Option<i32> {
    let tmpdir = isolated_tmpdir();
    let code = run_check_in(file, &tmpdir);
    let leaked = forge_entries_in(&tmpdir);
    let ok = leaked.is_empty();
    if ok {
        let _ = std::fs::remove_dir_all(&tmpdir);
    }
    assert!(
        ok,
        "blocker #53: `forge check` ({label}) orphaned scratch entries in {tmp}: {leaked:?} \
         (the per-run scratch dir + verus's compiled-binary sibling must be removed wholesale)",
        tmp = tmpdir.display(),
    );
    code
}

// ---- #53: the SUCCESS path leaves no orphan ------------------------------

#[test]
fn success_path_leaves_no_scratch_orphan() {
    if !verus_present() {
        eprintln!(
            "SKIP: verus not available — blocker #53 scratch-cleanup (success path) not run \
             (set VERUS_BIN or install verus on PATH)."
        );
        return;
    }
    // `sum` certifies L3 (a clean verus exit): the scratch dir should be gone.
    let code = assert_no_scratch_leak(&corpus_dir().join("sum.th"), "sum.th (L3 success)");
    assert_eq!(
        code,
        Some(0),
        "sum must certify L3 (exit 0) — no regression"
    );
}

// ---- #53: the ERROR (counterexample) path leaves no orphan ----------------

#[test]
fn error_path_leaves_no_scratch_orphan() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — blocker #53 scratch-cleanup (error path) not run.");
        return;
    }
    // A contract that disproves (`ens result == x + 2` but the body returns
    // `x + 1`): parses/validates/effect-checks/lowers cleanly, then verus reports
    // a counterexample and exits non-zero, the orphan-prone error path. The
    // scratch dir (including any partial compiled artifact) must still be removed.
    // The fixture is the input `.th` (not a `forge_*` scratch entry, and written
    // outside the per-run isolated TMPDIR the leak check inspects).
    let fixture = std::env::temp_dir().join(format!("th53_broken_{}.th", std::process::id()));
    std::fs::write(
        &fixture,
        "fn add_one(x: u64) -> u64\n  req x < 1000\n  ens result == x + 2\n  fx  pure\n{\n  x + 1\n}\n",
    )
    .expect("write broken fixture");

    let code = assert_no_scratch_leak(&fixture, "add_one (counterexample, exit 1)");

    let _ = std::fs::remove_file(&fixture);
    assert_eq!(
        code,
        Some(1),
        "a reported verification failure exits with the verification-failure code"
    );
}

// ---- #53 (reopened): the #13 vacuity-solver harness path leaves no orphan -----

#[test]
fn vacuity_harness_success_leaves_no_scratch_orphan() {
    if !verus_present() {
        eprintln!(
            "SKIP: verus not available — blocker #53 scratch-cleanup (vacuity-solver harness \
             success path) not run."
        );
        return;
    }
    // The same #53 leak lived in `vacuity_solver.rs`'s verus invocation: the
    // #13 gate runs a tautology + a vacuity harness on every fn before L3, each its
    // own verus query. When a harness succeeds (a tautology fn / an unsat-`req` fn,
    // the rejected cases) verus compiles and leaves the ~4.3M binary sibling
    // orphaned in the working dir. A tautology fixture (the
    // `conformance/solver-vacuity` oracle's `semantic_tautology`: `ens result >= 0`
    // holds for any u32) makes the tautology harness prove, the leak-prone success
    // path. The scratch dir + that compiled binary must still be removed wholesale.
    //
    // The fixture parses/validates/effect-checks/lowers cleanly and passes #6's free
    // structural triage, so the #13 solver gate runs its harness queries (the leak
    // surface). It is written outside the per-run isolated TMPDIR the leak check
    // inspects (its own `.th` is not a `forge_*` scratch entry).
    let fixture = std::env::temp_dir().join(format!("th53_taut_{}.th", std::process::id()));
    std::fs::write(
        &fixture,
        "fn f(x: u32) -> u32\n  req x > 0\n  ens result >= 0\n  fx  pure\n{\n  x\n}\n",
    )
    .expect("write tautology fixture");

    let code = assert_no_scratch_leak(
        &fixture,
        "tautology fn (vacuity-solver harness SUCCEEDS — the leak case)",
    );

    let _ = std::fs::remove_file(&fixture);
    // A #13 SemanticTautology reject is a non-certified cert (Level::L0), so the run
    // exits with the verification-failure code — not an environment error.
    assert_eq!(
        code,
        Some(1),
        "a SemanticTautology reject is a non-certified (failure-code) verdict, not an env error"
    );
}
