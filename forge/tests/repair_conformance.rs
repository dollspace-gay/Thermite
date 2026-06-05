//! Conformance for the background proof-repair loop `forge repair` (issue #18,
//! `.design/forge/proof-repair.md`; `thermite-design.md` §6, Appendix B). The
//! DETERMINISTIC loop logic — the bounded escalation ladder, the upgrade path, the
//! anti-cheat gate, bounded termination, and the environment-error propagation —
//! is pinned HERMETICALLY by the unit tests in `forge/src/repair.rs` (the loop
//! driven on SYNTHESIZED per-rung `RepairVerdict`s, the #10/#11 precedent; a live
//! timeout-that-proves is timing-fragile + Z3-version-sensitive, OQ-1). This file
//! is the LIVE layer through the built `forge` binary:
//!
//! - **THE ANTI-CHEAT (AC-3, from the oracle `never_upgraded`):** the grounded
//!   `never_provable_counterexample` (`ens result == x + 2`, body `x + 1`) is a
//!   COUNTEREXAMPLE at EVERY budget. `forge repair` MUST NOT upgrade it — it stays
//!   L0 / not-repairable, NEVER a false L3. A counterexample is reliably
//!   reproducible, so this is a LIVE test (the highest-value test, R-DEFER-9 / §12).
//! - **The no-op (AC-1, from the oracle `no_op`):** the corpus `conformance/sum.th`
//!   (both fns certify L3) → repair finds NO sub-L3 item, attempts nothing, the
//!   repair set is empty, the corpus certs are unchanged.
//!
//! Verus-dependent checks SKIP LOUDLY when verus is absent (mirroring
//! `degrade_conformance.rs` / `profile_conformance.rs`). `tests/` is not
//! anti-pattern-gated, so `unwrap`/`expect`/`panic!` are fine here. Do NOT edit
//! `conformance/` (R-CHAR-3): the never-provable program text is taken from the
//! oracle `conformance/repair/cases.json` (`never_upgraded[0].program`), written to
//! a TEMP `.th` (the conformance dir is read-only); expected outcomes trace to the
//! oracle + the doc's grounding (a false `ens` is a counterexample at every
//! budget), never to forge's own output.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("conformance")
}

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// `true` iff verus can be located (mirrors `degrade_conformance.rs`). SKIP
/// LOUDLY otherwise.
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

/// Run `forge repair <file> [item] --json`, returning (exit_code, json document).
fn run_repair_json(file: &Path, item: Option<&str>) -> (Option<i32>, Value) {
    let mut cmd = Command::new(forge_bin());
    cmd.arg("repair").arg(file);
    if let Some(it) = item {
        cmd.arg(it);
    }
    cmd.arg("--json");
    let out = cmd.output().expect("spawn forge repair");
    let code = out.status.code();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let doc: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!(
            "forge repair --json must emit a JSON document; parse error: {e}\nstdout:\n{stdout}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stderr)
        )
    });
    (code, doc)
}

/// Read the grounded never-provable counterexample PROGRAM from the oracle
/// `conformance/repair/cases.json` (R-CHAR-3 — the program text is the oracle's,
/// not forge's), write it to a temp `.th` (the conformance dir is read-only), and
/// return the temp path. The expected OUTCOME (stays L0, never upgraded) traces to
/// the doc's grounding, asserted by the caller.
fn write_never_provable_fixture() -> PathBuf {
    let cases_path = corpus_dir().join("repair").join("cases.json");
    let cases: Value =
        serde_json::from_str(&std::fs::read_to_string(&cases_path).expect("read cases.json"))
            .expect("parse cases.json");
    let program = cases["never_upgraded"][0]["program"]
        .as_str()
        .expect("never_upgraded[0].program is a string");
    let dir = std::env::temp_dir().join(format!("forge_repair_test_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("never_provable_counterexample.th");
    std::fs::write(&path, program).expect("write temp fixture");
    path
}

// ── AC-1: corpus `sum.th` is already L3 → repair is a NO-OP ───────────────────
//
// `forge repair conformance/sum.th` finds NO sub-L3 item (both `spec_sum` and
// `sum` certify L3 at the default budget — grounded, `conformance/sum.cert.json`
// is `"level": "L3"`), so the repair set is EMPTY and repair attempts no
// escalation. Exit 0 (a no-op is a fully-repaired pass).
#[test]
fn corpus_sum_is_a_repair_noop() {
    if !verus_present() {
        eprintln!("SKIP corpus_sum_is_a_repair_noop: verus not found on PATH");
        return;
    }
    let sum = corpus_dir().join("sum.th");
    let (code, doc) = run_repair_json(&sum, None);
    let repaired = doc["repaired"].as_array().expect("`repaired` is an array");
    assert!(
        repaired.is_empty(),
        "AC-1: an already-L3 corpus must have an EMPTY repair set (no sub-L3 item); got: {doc}"
    );
    assert!(
        doc["total_checked"].as_u64().unwrap_or(0) >= 1,
        "the corpus has at least one checked item: {doc}"
    );
    assert_eq!(code, Some(0), "a no-op repair pass exits 0: {doc}");
}

// ── AC-3: THE ANTI-CHEAT — a counterexample is NEVER upgraded to L3 ───────────
//
// The grounded `never_provable_counterexample` (`ens result == x + 2`, body
// `x + 1`) is a COUNTEREXAMPLE (`postcondition not satisfied`, NO profile report)
// at EVERY budget. `forge repair` MUST NOT retry it and MUST NOT upgrade it: it
// stays a sub-L3 not-repairable item, NEVER a false L3. A regression laundering a
// false contract into L3 is the worst possible failure (§12, R-DEFER-9) — the
// highest-value test, and reliably LIVE (a counterexample is reproducible).
#[test]
fn counterexample_is_never_upgraded() {
    if !verus_present() {
        eprintln!("SKIP counterexample_is_never_upgraded: verus not found on PATH");
        return;
    }
    let fixture = write_never_provable_fixture();
    let (code, doc) = run_repair_json(&fixture, None);

    let repaired = doc["repaired"].as_array().expect("`repaired` is an array");
    // The single fn `inc` is sub-L3 and must be in the repair set as a HARD FAIL.
    let inc = repaired
        .iter()
        .find(|i| i["item"].as_str() == Some("inc"))
        .unwrap_or_else(|| panic!("`inc` must be a reported sub-L3 item: {doc}"));

    let outcome = inc["outcome"].as_str().unwrap_or("");
    assert_eq!(
        outcome, "not_repairable",
        "AC-3 / THE ANTI-CHEAT: a counterexample is REPORTED not-repairable, NEVER \
         upgraded_to_l3 / still_sub_l3-with-a-retry; got `{outcome}`: {doc}"
    );
    assert_ne!(
        outcome, "upgraded_to_l3",
        "AC-3: a false contract must NEVER be laundered into L3: {doc}"
    );
    assert_eq!(
        inc["level"].as_str(),
        Some("L0"),
        "the counterexample item stays L0: {doc}"
    );
    // A non-fully-repaired pass exits non-zero (the project does not certify).
    assert_ne!(
        code,
        Some(0),
        "a not-repairable residue is a non-zero exit: {doc}"
    );

    let _ = std::fs::remove_file(&fixture);
}

// ── The anti-cheat holds even when repair is restricted to the single item ────
//
// `forge repair <file> inc` (the optional item selector) must classify `inc` as a
// counterexample and refuse to retry/upgrade it — the anti-cheat is not bypassed
// by the single-item path.
#[test]
fn counterexample_never_upgraded_single_item() {
    if !verus_present() {
        eprintln!("SKIP counterexample_never_upgraded_single_item: verus not found on PATH");
        return;
    }
    let fixture = write_never_provable_fixture();
    let (_code, doc) = run_repair_json(&fixture, Some("inc"));
    let repaired = doc["repaired"].as_array().expect("array");
    assert_eq!(
        repaired.len(),
        1,
        "only the selected item is in the set: {doc}"
    );
    assert_eq!(
        repaired[0]["outcome"].as_str(),
        Some("not_repairable"),
        "AC-3 (single-item): a counterexample is never upgraded: {doc}"
    );
    let _ = std::fs::remove_file(&fixture);
}
