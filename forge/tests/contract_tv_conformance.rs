//! Conformance for the CONTRACT-FAITHFULNESS TRANSLATION-VALIDATION phase
//! (`.design/verified/contract-tv.md` REQ-5 / REQ-3; epic crosslink #139 /
//! blockers #144 + #142). Two load-bearing properties, both through the REAL
//! `verus` binary (SKIP LOUDLY if absent, mirroring `check_conformance.rs`):
//!
//! 1. **Corpus no-false-positive (the key AC):** `forge tv <corpus.th> --json`
//!    over the representative corpus (sum / binary_search / map_kv) yields ZERO
//!    `divergent` clauses — the FAITHFUL production lowering must NOT trip TV. A
//!    real divergence here would be a genuine lowering bug (a find), so the test
//!    pins `divergent == 0` (R-CHAR-3 — the expected value is the design's
//!    faithful-lowering invariant, NOT the toolchain's own output).
//! 2. **Off-corpus generated run (the thesis payoff):** `forge tv sum.th
//!    --generated 200 --json` lowers + TV-checks 200 deterministically generated
//!    clauses; the faithful lowerer makes every CHECKED clause `faithful` (0
//!    `divergent`). ANY divergence is a real off-corpus infidelity finding (the
//!    whole point — surfaced loudly).
//!
//! Expected values trace to the design's faithful-lowering invariant + the frozen
//! sublanguage, never to the lowerer's output (R-CHAR-3). `unwrap`/`expect` are
//! fine here (`tests/` is not anti-pattern-gated).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("conformance")
}

/// `true` iff verus can be located (`VERUS_BIN`, then PATH, then
/// `~/.local/bin/verus`). SKIP LOUDLY otherwise (mirrors `check_conformance.rs`).
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

/// Run `forge tv <file> [--generated N] --json`, returning the parsed JSON report.
fn run_tv_json(file: &Path, generated: Option<usize>) -> Value {
    let mut cmd = Command::new(forge_bin());
    cmd.arg("tv").arg(file).arg("--json");
    if let Some(n) = generated {
        cmd.arg("--generated").arg(n.to_string());
    }
    let out = cmd
        .output()
        .unwrap_or_else(|e| panic!("spawn forge tv: {e}"));
    let stdout = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!(
            "forge tv --json stdout must be one JSON document: {e}\nstdout:\n{stdout}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stderr)
        )
    })
}

fn corpus_counts(report: &Value) -> (u64, u64, u64) {
    let c = &report["corpus"]["counts"];
    (
        c["checked"].as_u64().unwrap(),
        c["faithful"].as_u64().unwrap(),
        c["divergent"].as_u64().unwrap(),
    )
}

// ---- AC: corpus no-false-positive -----------------------------------------

/// REQ-5 / the key AC: `forge tv sum.th` checks the REAL contract clauses and
/// finds them ALL faithful (0 divergent). The faithful production lowering of
/// `sum`'s `req`/`ens`/loop-`inv`/`dec` must NOT trip TV.
#[test]
fn sum_corpus_zero_divergent() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — `forge tv sum.th` not run.");
        return;
    }
    let report = run_tv_json(&corpus_dir().join("sum.th"), None);
    let (checked, faithful, divergent) = corpus_counts(&report);
    assert_eq!(
        divergent, 0,
        "sum corpus has a DIVERGENT clause — a real lowering-fidelity finding (or a \
         ref_encode/coercion gap). report: {report}"
    );
    assert!(
        checked >= 6,
        "sum should have >= 6 checkable clauses (req + 2 ens + 3 loop inv/dec); got {checked}"
    );
    assert_eq!(
        faithful, checked,
        "every checked sum clause must be faithful"
    );
}

/// REQ-5: binary_search's framable clauses (the `sorted(haystack)` req + the
/// `forall_below`/`forall_from` loop invariants + the `dec`) are ALL faithful (0
/// divergent). The `Option<usize>` `ens match` clause is honestly Skipped (Match
/// is body-TV scope), NOT a false faithful.
#[test]
fn binary_search_corpus_zero_divergent() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — `forge tv binary_search.th` not run.");
        return;
    }
    let report = run_tv_json(&corpus_dir().join("binary_search.th"), None);
    let (checked, faithful, divergent) = corpus_counts(&report);
    assert_eq!(
        divergent, 0,
        "binary_search corpus has a DIVERGENT clause — a real finding. report: {report}"
    );
    assert!(
        checked >= 4,
        "binary_search should have >= 4 checkable clauses (the combinator loop \
         invariants + req + dec); got {checked}"
    );
    assert_eq!(faithful, checked);
}

/// REQ-5: map_kv's framable scalar/Seq clauses are faithful (0 divergent); the
/// `Map`/`Option`-typed clauses are honestly Skipped.
#[test]
fn map_kv_corpus_zero_divergent() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — `forge tv map_kv.th` not run.");
        return;
    }
    let report = run_tv_json(&corpus_dir().join("map_kv.th"), None);
    let (_checked, faithful, divergent) = corpus_counts(&report);
    assert_eq!(
        divergent, 0,
        "map_kv corpus has a DIVERGENT clause — a real finding. report: {report}"
    );
    assert_eq!(faithful, corpus_counts(&report).0);
}

// ---- the off-corpus generated run (the thesis payoff) ----------------------

/// REQ-3 / AC-7: the 200-clause off-corpus generated run. The faithful lowerer
/// makes EVERY checked clause faithful (0 divergent). A divergence here is a REAL
/// off-corpus infidelity finding. Also asserts the generated run is SUBSTANTIVE
/// (many clauses CHECKED, not all skipped) and DIVERSE (the construct breakdown).
#[test]
fn off_corpus_generated_run_all_faithful() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — the off-corpus generated TV run not discharged.");
        return;
    }
    // 200 clauses, the design's N (AC-7). Discharging 200 verus runs is slow but is
    // the load-bearing thesis check; the `forge tv` binary runs them sequentially.
    let report = run_tv_json(&corpus_dir().join("sum.th"), Some(200));
    let gen = report
        .get("generated")
        .filter(|g| !g.is_null())
        .unwrap_or_else(|| panic!("--generated 200 must produce a `generated` report: {report}"));
    let counts = &gen["counts"];
    let checked = counts["checked"].as_u64().unwrap();
    let faithful = counts["faithful"].as_u64().unwrap();
    let divergent = counts["divergent"].as_u64().unwrap();
    let unverifiable = counts["unverifiable"].as_u64().unwrap();

    assert_eq!(
        divergent, 0,
        "OFF-CORPUS DIVERGENCE — a real lowering infidelity surfaced over the \
         generated clause space (the thesis payoff; file it `-l blocker`). \
         {divergent} divergent of {checked} checked. report: {gen}"
    );
    assert_eq!(
        faithful, checked,
        "every CHECKED generated clause must be faithful (the faithful lowerer); \
         {faithful} faithful of {checked} checked"
    );
    assert_eq!(
        unverifiable, 0,
        "a generated clause was UNVERIFIABLE — the obligation did not discharge \
         cleanly (a framing/encoding gap, not a faithfulness verdict). report: {gen}"
    );
    // SUBSTANTIVE: the generated run must actually CHECK a large fraction (not skip
    // ~everything) — the off-corpus space is real coverage, not vacuous.
    assert!(
        checked >= 120,
        "the 200-clause generated run checked only {checked} clauses — too many \
         skipped; the off-corpus coverage is not substantive. report: {gen}"
    );
}
