//! #19 spec-intent review-slot cert oracle (`.design/forge/spec-review.md` REQ-1/
//! REQ-2/REQ-7; `conformance/review/cases.json`). Drives the BUILT `forge` binary
//! with `review <file> [--json] [--reviewer <cmd>]` and asserts the emitted
//! `ReviewArtifact` / `*.review.json` record against the HAND-DERIVED oracle
//! (R-CHAR-3 — expected values trace to `conformance/review/cases.json` +
//! `conformance/sum.th` + `thermite-design.md` §7, NEVER copied from forge's own
//! output):
//!
//! - `corpus_sum` (`forge review conformance/sum.th --json`): `sum` is
//!   INTENT-REVIEWABLE; its spec layer includes `req`, `ens`, `fx`, and
//!   `spec_sum`'s DECLARATION; NO body text (sum's accumulator loop / spec_sum's
//!   match arms are EXCLUDED). The artifact is DETERMINISTIC (byte-identical across
//!   two runs) — AC-1/AC-2/AC-4.
//! - `vacuous` (`forge review conformance/review/vacuous.th --json`): `f` is flagged
//!   `battery_failing` (cause `EnsIsTrivial`), NOT intent_reviewable — AC-3,
//!   R-DEFER-9.
//! - the `--reviewer` shell-out: a STUB reviewer command (a tiny `cat`-replacing
//!   shell stub emitting a fixed `ReviewVerdict`) → `forge review --reviewer <stub>`
//!   writes the verdict to a `<file>.review.json` record; a FAILING / ABSENT
//!   reviewer cmd → a `ForgeError` (non-zero exit, no panic).
//!
//! The review runs the check pipeline (which resolves the verus version up front),
//! so these SKIP LOUDLY if verus is absent (never panic on a missing solver),
//! mirroring `audit_conformance.rs` / `check_conformance.rs`.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn conformance_dir() -> PathBuf {
    repo_root().join("conformance")
}

fn cases() -> Value {
    let src = std::fs::read_to_string(conformance_dir().join("review").join("cases.json"))
        .unwrap_or_else(|e| panic!("read review cases.json: {e}"));
    serde_json::from_str(&src).unwrap_or_else(|e| panic!("parse review cases.json: {e}"))
}

/// `true` iff verus can be located — mirrors `audit_conformance.rs`. The review
/// runs the check pipeline, which resolves the verus version up-front.
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

/// Run `forge review <args...>`, returning (exit_code, stdout, stderr).
fn run_review(args: &[&str]) -> (Option<i32>, String, String) {
    let out = Command::new(forge_bin())
        .arg("review")
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("spawn forge: {e}"));
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

fn parse_artifact(stdout: &str) -> Value {
    serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!("forge review --json must be one JSON doc: {e}\nstdout:\n{stdout}")
    })
}

fn find_reviewable<'a>(artifact: &'a Value, name: &str) -> Option<&'a Value> {
    artifact["intent_reviewable"]
        .as_array()
        .expect("intent_reviewable array")
        .iter()
        .find(|r| r["item"].as_str() == Some(name))
}

fn find_failing<'a>(artifact: &'a Value, name: &str) -> Option<&'a Value> {
    artifact["battery_failing"]
        .as_array()
        .expect("battery_failing array")
        .iter()
        .find(|r| r["item"].as_str() == Some(name))
}

// AC-1/AC-2/AC-4: `sum` is intent-reviewable with its declarative spec layer
// (req/ens/fx + spec_sum's DECLARATION), NO bodies, and the artifact is
// byte-deterministic. Expected fields trace to conformance/review/cases.json +
// conformance/sum.th (R-CHAR-3).
#[test]
fn corpus_sum_intent_reviewable_no_bodies() {
    if !verus_present() {
        eprintln!(
            "SKIP: verus not available — review corpus oracle not run (review runs the check \
             pipeline for pre-screening, which requires the prover)."
        );
        return;
    }
    let oracle = cases();
    let case = oracle["intent_reviewable"]
        .as_array()
        .expect("intent_reviewable cases")
        .iter()
        .find(|c| c["name"] == "corpus_sum")
        .expect("corpus_sum case");
    let source = case["source"].as_str().expect("source"); // conformance/sum.th
    let fn_name = case["fn"].as_str().expect("fn"); // sum

    let file = repo_root().join(source);
    let file_str = file.to_string_lossy().to_string();
    let (code, stdout, stderr) = run_review(&[&file_str, "--json"]);
    assert_eq!(
        code,
        Some(0),
        "the extraction succeeds (the artifact is a valid document); stderr:\n{stderr}"
    );
    let artifact = parse_artifact(&stdout);

    // `sum` is intent-reviewable; `spec_sum` (a pure dependency, no contract) is not
    // a reviewed item (it appears only as `sum`'s referenced declaration).
    let sum = find_reviewable(&artifact, fn_name)
        .unwrap_or_else(|| panic!("`{fn_name}` must be intent-reviewable:\n{stdout}"));

    // The spec layer: req/ens/fx verbatim (traces to conformance/sum.th lines 11-14).
    assert_eq!(
        sum["spec_layer"]["req"].as_str(),
        Some("xs.len() <= 1_000_000")
    );
    let ens: Vec<&str> = sum["spec_layer"]["ens"]
        .as_array()
        .expect("ens array")
        .iter()
        .map(|v| v.as_str().expect("ens str"))
        .collect();
    assert_eq!(
        ens,
        vec![
            "result == spec_sum(xs)",
            "result <= xs.len() as u64 * u32::MAX as u64",
        ]
    );
    let fx: Vec<&str> = sum["spec_layer"]["fx"]
        .as_array()
        .expect("fx array")
        .iter()
        .map(|v| v.as_str().expect("fx str"))
        .collect();
    assert_eq!(fx, vec!["pure"]);

    // spec_sum's DECLARATION included (name + signature + dec); body EXCLUDED.
    let refs = sum["spec_layer"]["referenced_spec_fns"]
        .as_array()
        .expect("referenced_spec_fns array");
    assert_eq!(
        refs.len(),
        1,
        "spec_sum is the one directly-referenced spec fn"
    );
    assert_eq!(refs[0]["name"].as_str(), Some("spec_sum"));
    assert_eq!(
        refs[0]["signature"].as_str(),
        Some("spec fn spec_sum(xs: &[u32]) -> u64")
    );
    assert_eq!(refs[0]["dec"].as_str(), Some("xs.len()"));

    // The intent prompt names the item (REQ-3).
    assert!(
        sum["prompt"]
            .as_str()
            .map(|p| p.contains("sum") && p.contains("what you"))
            .unwrap_or(false),
        "the prompt names the item + frames intent: {:?}",
        sum["prompt"]
    );

    // NO body tokens anywhere in the artifact (the "no bodies" rule, R-DEFER-9):
    // sum's accumulator loop + spec_sum's match arms must be absent.
    for body_token in ["acc", "while", "[head, ..t]", "match", "head as u64"] {
        assert!(
            !stdout.contains(body_token),
            "the spec-layer artifact must EXCLUDE body token `{body_token}`:\n{stdout}"
        );
    }

    // AC-4 determinism: a second run is byte-identical.
    let (_c2, stdout2, _e2) = run_review(&[&file_str, "--json"]);
    assert_eq!(
        stdout, stdout2,
        "the extraction is deterministic (byte-identical across runs)"
    );
}

// AC-3 (R-DEFER-9): the vacuous fixture's `f` (`ens true`) is flagged
// battery_failing with cause `EnsIsTrivial`, NOT intent-reviewable.
#[test]
fn vacuous_flagged_not_surfaced() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — review battery_failing oracle not run.");
        return;
    }
    let oracle = cases();
    let case = oracle["battery_failing"]
        .as_array()
        .expect("battery_failing cases")
        .iter()
        .find(|c| c["name"] == "vacuous")
        .expect("vacuous case");
    let source = case["source"].as_str().expect("source"); // conformance/review/vacuous.th
    let fn_name = case["fn"].as_str().expect("fn"); // f

    let file = repo_root().join(source);
    let file_str = file.to_string_lossy().to_string();
    let (_code, stdout, stderr) = run_review(&[&file_str, "--json"]);
    let artifact = parse_artifact(&stdout);

    assert!(
        find_reviewable(&artifact, fn_name).is_none(),
        "a battery-failing fn is NOT surfaced for intent review (R-DEFER-9):\n{stderr}\n{stdout}"
    );
    let failing = find_failing(&artifact, fn_name)
        .unwrap_or_else(|| panic!("`{fn_name}` must be flagged battery_failing:\n{stdout}"));
    assert_eq!(
        failing["cause"].as_str(),
        Some("EnsIsTrivial"),
        "the §7.1 vacuity reject cause is surfaced"
    );
}

// REQ-7 / OQ-1: a STUB --reviewer command emitting a fixed ReviewVerdict → the
// verdict is written to a separate `<file>.review.json` record (forge never
// fabricates `aligned`; the verdict is the reviewer's). Uses a temp copy of
// conformance/sum.th so the read-only corpus stays untouched (R-CHAR-3).
#[test]
fn reviewer_shellout_attaches_verdict() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — reviewer shell-out oracle not run.");
        return;
    }
    let pid = std::process::id();
    // A temp copy of the corpus program (the read-only fixture stays untouched).
    let src = std::fs::read_to_string(conformance_dir().join("sum.th")).expect("read sum.th");
    let th = std::env::temp_dir().join(format!("review_stub_{pid}.th"));
    std::fs::write(&th, &src).expect("write temp .th");
    let record = std::env::temp_dir().join(format!("review_stub_{pid}.th.review.json"));
    let _ = std::fs::remove_file(&record);

    // A STUB reviewer: ignore stdin, emit a fixed ReviewVerdict on stdout. `head`
    // would block; a `cat >/dev/null` then `echo` drains stdin first so the writer's
    // EOF is honored.
    let stub = r#"cat >/dev/null; echo '{"item":"sum","aligned":true,"note":"matches Appendix A intent"}'"#;

    let th_str = th.to_string_lossy().to_string();
    let (code, _stdout, stderr) = run_review(&[&th_str, "--reviewer", stub]);
    assert_eq!(
        code,
        Some(0),
        "the artifact + a successful reviewer is a SUCCESS; stderr:\n{stderr}"
    );

    let written = std::fs::read_to_string(&record)
        .unwrap_or_else(|e| panic!("the reviewer verdict record must be written: {e}"));
    let rec: Value = serde_json::from_str(&written).expect("parse review record");
    let verdicts = rec["verdicts"].as_array().expect("verdicts array");
    assert_eq!(verdicts.len(), 1, "one verdict attached");
    assert_eq!(verdicts[0]["item"].as_str(), Some("sum"));
    assert_eq!(verdicts[0]["aligned"].as_bool(), Some(true));
    assert_eq!(
        verdicts[0]["note"].as_str(),
        Some("matches Appendix A intent")
    );

    let _ = std::fs::remove_file(&th);
    let _ = std::fs::remove_file(&record);
}

// REQ-7: an ABSENT / FAILING reviewer cmd → a ForgeError (non-zero exit, no panic,
// no fabricated verdict). A reviewer that exits non-zero is the graceful-failure
// case the design mandates.
#[test]
fn reviewer_failure_is_error_not_panic() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — reviewer-failure oracle not run.");
        return;
    }
    let file = conformance_dir().join("sum.th");
    let file_str = file.to_string_lossy().to_string();

    // A reviewer that exits non-zero (drains stdin, then fails). Must be a non-zero
    // forge exit with a diagnostic on stderr — never a panic, never a written record.
    let failing = "cat >/dev/null; exit 7";
    let (code, _stdout, stderr) = run_review(&[&file_str, "--reviewer", failing]);
    assert_ne!(code, Some(0), "a failing reviewer is a non-zero exit");
    assert!(
        stderr.contains("reviewer"),
        "the failure names the reviewer (no panic):\n{stderr}"
    );

    // An ABSENT reviewer command → a non-zero exit, never a panic.
    let absent = "/no/such/reviewer/binary/at/all";
    let (code2, _stdout2, stderr2) = run_review(&[&file_str, "--reviewer", absent]);
    assert_ne!(code2, Some(0), "an absent reviewer is a non-zero exit");
    assert!(
        !stderr2.is_empty(),
        "the absent reviewer surfaces a diagnostic (no panic)"
    );
}
