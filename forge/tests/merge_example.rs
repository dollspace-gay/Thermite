//! AC-11 (`.design/stage1-forge-tier.md` REQ-7, increment 2e) — the merge example,
//! end to end: `forge goal --proof` renders a forge-routed goal with its hypotheses in
//! scope; `forge fill <item> ?p0 "<tactics>"` closes the goal; the resulting certificate
//! (the Lean discharge of the forge-tier `lemma`) carries the BURN RECEIPT (the committed
//! proof's token count + cited lemmas).
//!
//! The discharge legs (`fill`'s re-check, `check --engine lean`) invoke lake against the
//! built Lean spine, so they SKIP when lake is absent (mirroring the `lean_engine.rs`
//! live-spine tests); the CI lean job is the authoritative gate. The proof-view render +
//! the proof-hole splice need no prover and run unconditionally.

use std::path::PathBuf;
use std::process::Command;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// `lake` on PATH / under `~/.elan` — the live Lean discharge needs it (the built spine).
/// Absent → the discharge legs SKIP (the proof-view/fill legs still run).
fn lake_present() -> bool {
    if let Ok(home) = std::env::var("HOME") {
        if PathBuf::from(home).join(".elan/bin/lake").exists() {
            return true;
        }
    }
    Command::new("lake")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

static TEMP_NONCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn temp_th(src: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "forge_merge_ac11_{}_{}.th",
        std::process::id(),
        TEMP_NONCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::write(&path, src).expect("write temp .th");
    path
}

fn run_forge(args: &[&str]) -> (String, String, bool) {
    let out = Command::new(forge_bin())
        .args(args)
        .output()
        .expect("spawn forge");
    (
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
        out.status.success(),
    )
}

/// The merge example as a HOLED forge-tier lemma (the `?p0` the agent fills).
const MERGE_HOLED: &str =
    "lemma merge_advance(i: u64, n: u64)\n    req i < n\n    ens i + 1 <= n\n    proof { ?p0 }\n";

/// The frozen-battery tactics that close the merge goal against the denotation spine.
const MERGE_TACTICS: &str =
    "simp [Thermite.denote, Thermite.Env.bindInt, Thermite.intVal, Thermite.arithDenote]; omega";

// AC-11 (no prover needed): `forge goal --proof` renders the forge-routed merge goal with
// its hypotheses in scope (`i`/`n` typed, `req` as `h_req`), the `⊢ goal`, and the `?p0`
// fill operand.
#[test]
fn proof_view_renders_the_merge_goal_with_hypotheses() {
    let th = temp_th(MERGE_HOLED);
    let (out, err, ok) = run_forge(&["goal", th.to_str().unwrap(), "--proof"]);
    assert!(ok, "`forge goal --proof` should succeed: {out}{err}");
    assert!(
        out.contains("PROOF VIEW — merge_advance (lemma, forge-routed"),
        "{out}"
    );
    assert!(
        out.contains("i : u64") && out.contains("n : u64"),
        "typed hyps: {out}"
    );
    assert!(
        out.contains("h_req : i < n"),
        "the req precondition is in scope: {out}"
    );
    assert!(out.contains("\u{22a2} goal: i + 1 <= n"), "the goal: {out}");
    assert!(
        out.contains("merge_advance.proof.?p0"),
        "the fill operand: {out}"
    );
    let _ = std::fs::remove_file(&th);
}

// AC-11 (live spine): `forge fill <item> ?p0 "<tactics>"` closes the merge goal end to
// end, and the re-check certifies L3 + surfaces the BURN RECEIPT (token count + cited
// lemmas). SKIPs when lake is absent (the discharge needs the built spine).
#[test]
fn fill_closes_the_merge_goal_and_the_cert_carries_the_burn_receipt() {
    if !lake_present() {
        eprintln!("SKIP: lake not present — the live merge-lemma discharge is not run.");
        return;
    }
    let th = temp_th(MERGE_HOLED);
    let (out, err, ok) = run_forge(&[
        "fill",
        th.to_str().unwrap(),
        "merge_advance.proof.?p0",
        MERGE_TACTICS,
    ]);
    assert!(ok, "`forge fill` should succeed: {out}{err}");
    assert!(
        out.contains("proof: authored (no open holes)"),
        "the goal is closed: {out}"
    );
    assert!(
        out.contains("re-check: certified L3"),
        "the closed forge-tier goal certifies L3: {out}{err}"
    );
    assert!(
        out.contains("burn:") && out.contains("proof token(s)") && out.contains("cited lemmas:"),
        "the resulting certificate carries the burn receipt (token count + cited lemmas): {out}"
    );
    let _ = std::fs::remove_file(&th);
}

// AC-11 (live spine): the committed `conformance/forge/merge.th` certifies L3 via
// `forge check --engine lean` and its certificate carries the burn receipt in the JSON.
#[test]
fn merge_conformance_certifies_l3_with_burn_via_lean() {
    if !lake_present() {
        eprintln!("SKIP: lake not present — the live merge conformance discharge is not run.");
        return;
    }
    let (out, err, _ok) = run_forge(&[
        "check",
        "--engine",
        "lean",
        "../conformance/forge/merge.th",
        "--json",
    ]);
    let combined = format!("{out}{err}");
    // The merge_advance lemma certifies L3 and carries the burn receipt block.
    assert!(
        combined.contains("\"item\": \"merge_advance\""),
        "{combined}"
    );
    assert!(
        combined.contains("\"level\": \"L3\""),
        "merge_advance certifies L3: {combined}"
    );
    assert!(
        combined.contains("\"burn\""),
        "the cert carries the burn receipt: {combined}"
    );
    assert!(
        combined.contains("\"proof_tokens\""),
        "burn has the token count: {combined}"
    );
    assert!(
        combined.contains("\"cited_lemmas\""),
        "burn has the cited lemmas: {combined}"
    );
}
