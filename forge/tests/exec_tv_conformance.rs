//! Conformance for the EXEC-POSITION (body) TRANSLATION-VALIDATION phase
//! (`.design/verified/exec-tv.md` REQ-5 / REQ-3; epic crosslink #151, blockers
//! #154 / #156). Two load-bearing properties, both through the REAL `verus` binary
//! (SKIP LOUDLY if absent, mirroring `contract_tv_conformance.rs`):
//!
//! THE GENERATED run (PRIMARY — the off-corpus #122/#146 regression guard):
//! `forge exec-tv sum.th --generated 200 --json` lowers each generated, WELL-FRAMED
//! exec expr via `thermite_lower::lower_exec_expr` and discharges the exec-fn
//! obligation `result == <bounded exec reference>`. The faithful lowerer + the
//! adequate carried frames make EVERY checked expr `faithful` (0 divergent, 0
//! unverifiable, 0 skipped). ANY `divergent` is a REAL off-corpus exec-lowering
//! infidelity (the whole point — file it `-l blocker`). The CONSTRUCT COVERAGE
//! (cast-`<` / arith / cast / index) is asserted non-vacuous (the #122/#146 classes
//! are actually exercised in real numbers).
//!
//! THE CORPUS body-expr check (BEST-EFFORT, honest coverage): `forge exec-tv sum.th
//! --no-generated --json` TV-checks the derivable-frame body exprs (a `let`-RHS /
//! tail) and SKIPS the loop statement HONESTLY (out of scope, step 2.2). The CHECKED
//! exprs are all `faithful` (no false positive); the loop is `skipped` (reported,
//! never a silent pass). It is ACCEPTABLE that corpus coverage is partial — the
//! generated run is the primary value.
//!
//! Expected values trace to the design's faithful-lowering invariant + the frozen
//! exec sublanguage, never to the lowerer's output (R-CHAR-3). `unwrap`/`expect` are
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
/// `~/.local/bin/verus`). SKIP LOUDLY otherwise (mirrors `contract_tv_conformance`).
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

/// Run `forge exec-tv <file> [--generated N | --no-generated] --json`, returning the
/// parsed JSON report.
fn run_exec_tv_json(file: &Path, generated: Option<usize>) -> Value {
    let mut cmd = Command::new(forge_bin());
    cmd.arg("exec-tv").arg(file).arg("--json");
    match generated {
        Some(n) => {
            cmd.arg("--generated").arg(n.to_string());
        }
        None => {
            cmd.arg("--no-generated");
        }
    }
    let out = cmd
        .output()
        .unwrap_or_else(|e| panic!("spawn forge exec-tv: {e}"));
    let stdout = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!(
            "forge exec-tv --json stdout must be one JSON document: {e}\nstdout:\n{stdout}\n\
             stderr:\n{}",
            String::from_utf8_lossy(&out.stderr)
        )
    })
}

// ---- the generated run (PRIMARY — the off-corpus #122/#146 regression guard) ----

/// REQ-3 / AC-7: the 200-expr off-corpus generated exec run. The faithful lowerer +
/// the adequate carried frames make EVERY checked expr `faithful` (0 divergent, 0
/// unverifiable, 0 skipped). A `divergent` here is a REAL off-corpus exec-lowering
/// infidelity finding (the whole point — surfaced loudly). Also asserts the run is
/// SUBSTANTIVE and the #122/#146 construct classes are EXERCISED (else the guard is
/// vacuous).
#[test]
fn generated_exec_run_all_faithful() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — the generated exec-TV run not discharged.");
        return;
    }
    let report = run_exec_tv_json(&corpus_dir().join("sum.th"), Some(200));
    let gen = report
        .get("generated")
        .filter(|g| !g.is_null())
        .unwrap_or_else(|| panic!("--generated 200 must produce a `generated` report: {report}"));
    let counts = &gen["counts"];
    let checked = counts["checked"].as_u64().unwrap();
    let faithful = counts["faithful"].as_u64().unwrap();
    let divergent = counts["divergent"].as_u64().unwrap();
    let unverifiable = counts["unverifiable"].as_u64().unwrap();
    let skipped = counts["skipped"].as_u64().unwrap();

    assert_eq!(
        divergent, 0,
        "OFF-CORPUS EXEC DIVERGENCE — a real exec-lowering infidelity surfaced over \
         the generated exec-expr space (the thesis payoff; file it `-l blocker`). \
         {divergent} divergent of {checked} checked. report: {gen}"
    );
    assert_eq!(
        faithful, checked,
        "every CHECKED generated exec expr must be faithful (the faithful lowerer + \
         the adequate carried frame); {faithful} faithful of {checked} checked"
    );
    assert_eq!(
        unverifiable, 0,
        "a generated exec expr was UNVERIFIABLE — the obligation did not discharge \
         cleanly (an INADEQUATE frame: the generated frames are supposed to be \
         adequate so the faithful lowering verifies). report: {gen}"
    );
    assert_eq!(
        skipped, 0,
        "a generated exec expr was SKIPPED — the generator stays within the exec \
         encoder + lowerer subset, so 0 skipped is expected. report: {gen}"
    );
    assert_eq!(
        checked, 200,
        "the generated run must CHECK all 200 exprs (substantive coverage); got {checked}"
    );

    // The #122/#146 off-corpus regression guard is NON-VACUOUS: the cast-`<` class
    // (a `Cast` left of a `<`-leading op — the #146 surface), arithmetic (the
    // overflow surface), casts (the #122 surface), and indexing (the AC-5 element
    // surface) are all EXERCISED in real numbers. Every such CHECKED expr being
    // faithful (divergent == 0 above) confirms the cast-paren disciplines hold
    // off-corpus on BOTH encoders.
    let cov = &gen["coverage"];
    assert!(
        cov["cast_lt"].as_u64().unwrap() >= 1,
        "the generated run must exercise the cast-`<` class (the #146 guard); \
         cast_lt = {}. report: {gen}",
        cov["cast_lt"]
    );
    assert!(
        cov["arith"].as_u64().unwrap() >= 5,
        "the generated run must exercise arithmetic (the overflow surface); \
         arith = {}",
        cov["arith"]
    );
    assert!(
        cov["casts"].as_u64().unwrap() >= 5,
        "the generated run must exercise casts (the #122 surface); casts = {}",
        cov["casts"]
    );
    assert!(
        cov["index"].as_u64().unwrap() >= 1,
        "the generated run must exercise slice indexing (the AC-5 surface); \
         index = {}",
        cov["index"]
    );
}

/// REQ-3 / AC-7 (determinism): the generated exec run is REPRODUCIBLE — two
/// `forge exec-tv --generated N` runs at the same (pinned) seed yield the IDENTICAL
/// counts (the seeded SplitMix64 generator + the pinned verus seed, R-CODE-5). Run
/// at a small N so it is cheap.
#[test]
fn generated_exec_run_is_reproducible() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — the exec-TV reproducibility check not run.");
        return;
    }
    let a = run_exec_tv_json(&corpus_dir().join("sum.th"), Some(20));
    let b = run_exec_tv_json(&corpus_dir().join("sum.th"), Some(20));
    assert_eq!(
        a["generated"]["counts"], b["generated"]["counts"],
        "two pinned-seed generated exec runs must produce identical counts (R-CODE-5)"
    );
}

// ---- the corpus body-expr check (BEST-EFFORT, honest coverage) --------------

/// REQ-5: the corpus body-expr check over `sum.th` — the CHECKED body exprs (the
/// `let`-RHSs + the tail `acc`) are all `faithful` (no false positive), and the loop
/// statement is `skipped` HONESTLY (out of scope — statements/loops/mutation are
/// step 2.2). Reports the honest coverage (checked vs skipped); partial coverage is
/// ACCEPTABLE (the generated run is the primary value).
#[test]
fn corpus_body_exprs_honest_coverage() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — the corpus exec-TV check not run.");
        return;
    }
    let report = run_exec_tv_json(&corpus_dir().join("sum.th"), None);
    let corpus = &report["corpus"];
    let counts = &corpus["counts"];
    let checked = counts["checked"].as_u64().unwrap();
    let faithful = counts["faithful"].as_u64().unwrap();
    let divergent = counts["divergent"].as_u64().unwrap();
    let skipped = counts["skipped"].as_u64().unwrap();

    // No false positive: every CHECKED corpus body expr is faithful, 0 divergent.
    assert_eq!(
        divergent, 0,
        "sum corpus body-expr check has a DIVERGENT expr — a real exec-lowering \
         finding (or a framing bug). report: {corpus}"
    );
    assert_eq!(
        faithful, checked,
        "every checked sum body expr must be faithful; {faithful} of {checked}"
    );
    // The CHECKED exprs are the derivable-frame body exprs (the `let mut acc: u64 =
    // 0`/`let mut i: usize = 0` RHSs + the tail `acc`). At least the tail + the two
    // lets reach verus.
    assert!(
        checked >= 2,
        "sum should have >= 2 derivable-frame body exprs checked (the lets + the \
         tail); got {checked}. report: {corpus}"
    );

    // The loop is SKIPPED HONESTLY (out of scope, step 2.2) — surfaced, never a
    // silent pass. So coverage is partial-by-design (honest).
    assert!(
        skipped >= 1,
        "sum's `while` loop must be SKIPPED honestly (statements/loops are step 2.2, \
         out of scope) — surfaced, never silent-passed. skipped = {skipped}. \
         report: {corpus}"
    );
    let loop_skipped = corpus["exprs"].as_array().unwrap().iter().any(|e| {
        e["expr"].as_str() == Some("sum.loop") && e["verdict"].as_str() == Some("skipped")
    });
    assert!(
        loop_skipped,
        "the `sum.loop` statement must be reported `skipped` (out-of-scope step 2.2). \
         report: {corpus}"
    );
}
