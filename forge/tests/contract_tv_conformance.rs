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

/// The verdict string for a named corpus clause (`"faithful"`/`"divergent"`/
/// `"skipped"`/`"unverifiable"`), or `None` if the clause is absent.
fn corpus_clause_verdict<'a>(report: &'a Value, clause: &str) -> Option<&'a str> {
    report["corpus"]["clauses"]
        .as_array()?
        .iter()
        .find(|c| c["clause"].as_str() == Some(clause))?["verdict"]
        .as_str()
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

    // #147 — the `&xs[..i]` slice-ref class: the ref-encoder gap is CLOSED
    // (`ref_encode::encode_ref` now encodes the subrange), so `sum.loop#1.inv#2`
    // (`acc == spec_sum(&xs[..i])`) is NO LONGER `skipped` for "unsupported
    // construct: reference". It currently discharges as `unverifiable` (NOT
    // `faithful`) because of a downstream FRAMING mismatch the ref-encoder does not
    // own: production's `lower_index` emits `xs@.subrange(..)` (an unconditional
    // `Seq::view`), but the obligation binds the slice param as `Seq<u32>`, which has
    // no `view()` — so the PRODUCTION column does not typecheck. Closing it to
    // `faithful` needs a `forge::contract_tv` frame change (bind the indexed slice as
    // `Vec<u32>` + pass it via the production `slices` list so production's `@` is
    // consistent) — a follow-on blocker outside #147's manifest. This assertion pins
    // the HONEST current state (not `skipped`, not a false `faithful`); flip it to
    // `faithful` when the frame fix lands.
    let inv2 = corpus_clause_verdict(&report, "sum.loop#1.inv#2");
    assert_ne!(
        inv2,
        Some("skipped"),
        "the `&xs[..i]` slice-ref ref-encoder gap is closed (#147) — inv#2 must no \
         longer be `skipped` for an unsupported reference construct. report: {report}"
    );
    assert_eq!(
        inv2,
        Some("unverifiable"),
        "inv#2 (`acc == spec_sum(&xs[..i])`) is `unverifiable` pending the \
         `forge::contract_tv` frame fix (production `xs@.subrange` vs the `Seq`-bound \
         obligation param) — flip to `faithful` when the frame binds the indexed \
         slice with a view. report: {report}"
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

    // #147 — the cast-`<` / non-Eq-nat REGRESSION GUARD for #146/#148 off-corpus.
    // The generator now emits a `Cast` left operand of a `<`-leading op (`n as u32 <
    // k`) and non-`Eq` nat comparisons (`acc <= spec_sum(xs)`); every such CHECKED
    // clause being `faithful` (0 divergent, asserted above) confirms the #146/#148
    // cast-paren fix + the #147 gap #2 Eq-only coercion hold off-corpus on BOTH
    // encoders. A `divergent`/`unverifiable` here = a real off-corpus hole. The
    // construct PRESENCE (so this guard is not vacuous) is asserted directly on the
    // deterministic generator in `thermite_tv::gen::tests::diverse_construct_coverage`
    // (`cast_lt >= 1`, `non_eq_nat_cmp >= 1`); here we re-confirm the run is the
    // EXTENDED one by requiring the clause count grew past the old 175-checked ceiling
    // is NOT asserted (byte-view ratio varies by seed) — the load-bearing guard is the
    // `divergent == 0 && unverifiable == 0` over the cast-`<`-bearing stream above.
}
