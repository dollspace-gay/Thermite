//! Conformance for the exec-position (body) translation-validation phase
//! (`.design/verified/exec-tv.md` REQ-5 / REQ-3; epic crosslink #151, blockers
//! #154 / #156). Two required properties, both through the real `verus` binary
//! (skip with a diagnostic if absent, mirroring `contract_tv_conformance.rs`):
//!
//! The generated run (primary — the off-corpus #122/#146 regression guard):
//! `forge exec-tv sum.th --generated 200 --json` lowers each generated, well-framed
//! exec expr via `thermite_lower::lower_exec_expr` and discharges the exec-fn
//! obligation `result == <bounded exec reference>`. The faithful lowerer + the
//! adequate carried frames make every checked expr `faithful` (0 divergent, 0
//! unverifiable, 0 skipped). A `divergent` is a off-corpus exec-lowering
//! infidelity (the point — file it `-l blocker`). The construct coverage
//! (cast-`<` / arith / cast / index) is asserted non-vacuous (the #122/#146 classes
//! are exercised in real numbers).
//!
//! The corpus body-expr check (best-effort coverage): `forge exec-tv sum.th
//! --no-generated --json` TV-checks the derivable-frame body exprs (a `let`-RHS /
//! tail) and skips the loop statement (out of scope, step 2.2). The checked
//! exprs are all `faithful` (no false positive); the loop is `skipped` (reported,
//! never a silent pass). Partial corpus coverage is acceptable; the
//! generated run is the primary value.
//!
//! Expected values trace to the design's faithful-lowering invariant + the frozen
//! exec sublanguage, not to the lowerer's output (R-CHAR-3). `unwrap`/`expect` are
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
/// `~/.local/bin/verus`). Skips with a logged note otherwise (mirrors `contract_tv_conformance`).
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

// ---- the generated run (primary — the off-corpus #122/#146 regression guard) ----

/// REQ-3 / AC-7: the 200-expr off-corpus generated exec run. The faithful lowerer +
/// the adequate carried frames make every checked expr `faithful` (0 divergent, 0
/// unverifiable, 0 skipped). A `divergent` here is a off-corpus exec-lowering
/// infidelity finding (the point — surfaced with a diagnostic). Also asserts the run is
/// substantive and the #122/#146 construct classes are exercised (else the guard is
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

    // The #122/#146 off-corpus regression guard is non-vacuous: the cast-`<` class
    // (a `Cast` left of a `<`-leading op — the #146 surface), arithmetic (the
    // overflow surface), casts (the #122 surface), and indexing (the AC-5 element
    // surface) are all exercised in real numbers. Every such checked expr being
    // faithful (divergent == 0 above) confirms the cast-paren disciplines hold
    // off-corpus on both encoders.
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

#[test]
fn fixed_array_read_expression_is_faithful() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — fixed-array exec-TV not discharged.");
        return;
    }
    let source = concat!(
        "const SLOTS: usize = 4;\n",
        "fn read(slots: [u64; SLOTS], at: usize) -> u64\n",
        "  req at < SLOTS\n",
        "  ens result == slots[at]\n",
        "  fx pure\n",
        "{ slots[at] }\n",
        "fn array_len(slots: [u64; SLOTS]) -> usize\n",
        "  req true\n",
        "  ens result == slots.len()\n",
        "  fx pure\n",
        "{ slots.len() }\n",
        "fn arrays_equal(left: [u64; SLOTS], right: [u64; SLOTS]) -> bool\n",
        "  req true\n",
        "  ens result == left.array_eq(right)\n",
        "  fx pure\n",
        "{ left.array_eq(right) }\n",
        "fn arrays_same_except(left: [u64; SLOTS], right: [u64; SLOTS], at: usize) -> bool\n",
        "  req true\n",
        "  ens result == left.array_same_except(right, at)\n",
        "  fx pure\n",
        "{ left.array_same_except(right, at) }\n",
    );
    let path = std::env::temp_dir().join("thermite_exec_tv_fixed_array.th");
    std::fs::write(&path, source).expect("write fixed-array exec-TV fixture");
    let report = run_exec_tv_json(&path, None);
    let counts = &report["corpus"]["counts"];
    assert_eq!(counts["checked"].as_u64(), Some(4), "{report}");
    assert_eq!(counts["faithful"].as_u64(), Some(4), "{report}");
    assert_eq!(counts["divergent"].as_u64(), Some(0), "{report}");
    assert!(report["corpus"]["exprs"]
        .as_array()
        .unwrap()
        .iter()
        .any(|expr| {
            expr["expr"].as_str() == Some("read.tail")
                && expr["verdict"].as_str() == Some("faithful")
        }));
    assert!(report["corpus"]["exprs"]
        .as_array()
        .unwrap()
        .iter()
        .any(|expr| {
            expr["expr"].as_str() == Some("array_len.tail")
                && expr["verdict"].as_str() == Some("faithful")
        }));
    assert!(report["corpus"]["exprs"]
        .as_array()
        .unwrap()
        .iter()
        .any(|expr| {
            expr["expr"].as_str() == Some("arrays_equal.tail")
                && expr["verdict"].as_str() == Some("faithful")
        }));
    assert!(report["corpus"]["exprs"]
        .as_array()
        .unwrap()
        .iter()
        .any(|expr| {
            expr["expr"].as_str() == Some("arrays_same_except.tail")
                && expr["verdict"].as_str() == Some("faithful")
        }));
}

#[test]
fn u64_bit_method_expressions_are_faithful() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — u64-bit exec-TV not discharged.");
        return;
    }
    let source = concat!(
        "fn set_bit(word: u64, bit: usize) -> u64\n",
        "  req true\n",
        "  ens result == word.bit_set(bit)\n",
        "  fx pure\n",
        "{ word.bit_set(bit) }\n",
        "fn clear_bit(word: u64, bit: usize) -> u64\n",
        "  req true\n",
        "  ens result == word.bit_clear(bit)\n",
        "  fx pure\n",
        "{ word.bit_clear(bit) }\n",
        "fn test_bit(word: u64, bit: usize) -> bool\n",
        "  req true\n",
        "  ens result == word.bit_test(bit)\n",
        "  fx pure\n",
        "{ word.bit_test(bit) }\n",
        "fn set_preserves(word: u64, changed: usize, observed: usize) -> bool\n",
        "  req true\n",
        "  ens result == word.bit_set_preserves_other(changed, observed)\n",
        "  fx pure\n",
        "{ word.bit_set_preserves_other(changed, observed) }\n",
        "fn clear_preserves(word: u64, changed: usize, observed: usize) -> bool\n",
        "  req true\n",
        "  ens result == word.bit_clear_preserves_other(changed, observed)\n",
        "  fx pure\n",
        "{ word.bit_clear_preserves_other(changed, observed) }\n",
    );
    let path = std::env::temp_dir().join("thermite_exec_tv_u64_bits.th");
    std::fs::write(&path, source).expect("write u64-bit exec-TV fixture");
    let report = run_exec_tv_json(&path, None);
    let counts = &report["corpus"]["counts"];
    assert_eq!(counts["checked"].as_u64(), Some(5), "{report}");
    assert_eq!(counts["faithful"].as_u64(), Some(5), "{report}");
    assert_eq!(counts["divergent"].as_u64(), Some(0), "{report}");
    for expression in [
        "set_bit.tail",
        "clear_bit.tail",
        "test_bit.tail",
        "set_preserves.tail",
        "clear_preserves.tail",
    ] {
        assert!(
            report["corpus"]["exprs"]
                .as_array()
                .unwrap()
                .iter()
                .any(|expr| expr["expr"].as_str() == Some(expression)
                    && expr["verdict"].as_str() == Some("faithful")),
            "{expression}: {report}"
        );
    }
}

/// REQ-3 / AC-7 (determinism): the generated exec run is reproducible — two
/// `forge exec-tv --generated N` runs at the same (pinned) seed yield identical
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

// ---- the corpus body-expr check (best-effort coverage) --------------

/// REQ-5: the corpus body-expr check over `sum.th` — the checked body exprs (the
/// `let`-RHSs + the tail `acc`) are all `faithful` (no false positive), and the loop
/// statement is `skipped` (out of scope — statements/loops/mutation are
/// step 2.2). Reports the coverage (checked vs skipped); partial coverage is
/// acceptable (the generated run is the primary value).
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

    // No false positive: every checked corpus body expr is faithful, 0 divergent.
    assert_eq!(
        divergent, 0,
        "sum corpus body-expr check has a DIVERGENT expr — a real exec-lowering \
         finding (or a framing bug). report: {corpus}"
    );
    assert_eq!(
        faithful, checked,
        "every checked sum body expr must be faithful; {faithful} of {checked}"
    );
    // The checked exprs are the derivable-frame body exprs (the `let mut acc: u64 =
    // 0`/`let mut i: usize = 0` RHSs + the tail `acc`). At least the tail + the two
    // lets reach verus.
    assert!(
        checked >= 2,
        "sum should have >= 2 derivable-frame body exprs checked (the lets + the \
         tail); got {checked}. report: {corpus}"
    );

    // The loop is skipped (out of scope, step 2.2) — surfaced, never a
    // silent pass. So coverage is partial by design.
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
