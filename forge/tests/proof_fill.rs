//! `forge goal --proof` + `forge fill ?pN` end-to-end (`.design/stage1-forge-tier.md`
//! REQ-7 / AC-11, increment 2e). The proof view renders a forge-routed goal with its
//! hypotheses in scope; `forge fill` splices tactics at a `?pN` proof hole, re-checks
//! (the frozen battery refuses an unlisted tactic — REQ-5/2c), and re-presents the
//! proof view (surfacing any new holes the fill introduced — the §5.1 loop).
//!
//! These assertions drive the `forge` binary (the CLI surface). The proof-view render
//! and the proof-hole splice precede any verus/lake discharge, so they need no prover;
//! the battery refusal is the elaboration-time gate (also prover-free).

use std::path::PathBuf;
use std::process::Command;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

static TEMP_NONCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn temp_th(tag: &str, src: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "forge_prooffill_{tag}_{}_{}.th",
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

// AC-11: `forge goal --proof` renders a forge-routed goal (a `proof for f` obligation)
// with its hypotheses in scope — `f`'s typed params — and the resolved `ens#k` goal,
// plus the open `?pN` proof hole as the `forge fill` operand.
#[test]
fn proof_view_renders_hypotheses_for_a_forge_routed_goal() {
    let th = temp_th(
        "view",
        "fn maxv(x: u64, y: u64) -> u64 req true ens result >= x ens result >= y \
         fx pure { if x > y { x } else { y } }\n\
         proof for maxv { ens#0 by { ?p0 } }",
    );
    let (out, err, ok) = run_forge(&["goal", th.to_str().unwrap(), "--proof"]);
    assert!(ok, "`forge goal --proof` should succeed: {out}{err}");
    assert!(out.contains("PROOF VIEW — proof for maxv.ens#0"), "{out}");
    assert!(out.contains("hypotheses in scope:"), "{out}");
    assert!(out.contains("x : u64") && out.contains("y : u64"), "{out}");
    // `ens#0` is the first ens clause (0-based), `result >= x`.
    assert!(out.contains("\u{22a2} goal: result >= x"), "{out}");
    assert!(
        out.contains("maxv.proof.ens#0.?p0"),
        "the fill operand: {out}"
    );
    let _ = std::fs::remove_file(&th);
}

// AC-11: `forge fill <item> ?p0 "<tactics>"` splices the tactics at the proof hole and
// re-presents a now hole-free proof view. The committed file carries the tactics.
#[test]
fn fill_closes_a_proof_hole_and_commits_the_tactics() {
    let th = temp_th(
        "close",
        "lemma le_id(a: u64, b: u64) req a <= b ens a <= b proof { ?p0 }",
    );
    let (out, err, ok) = run_forge(&["fill", th.to_str().unwrap(), "le_id.proof.?p0", "omega"]);
    assert!(
        ok,
        "`forge fill` on a proof hole should succeed: {out}{err}"
    );
    assert!(
        out.contains("proof: authored (no open holes)"),
        "the hole is closed: {out}"
    );
    let committed = std::fs::read_to_string(&th).expect("read filled file");
    assert!(
        committed.contains("proof { omega }"),
        "the tactics are spliced into the proof block: {committed}"
    );
    let _ = std::fs::remove_file(&th);
}

// AC-11 / REQ-5: a fill citing a tactic OUTSIDE the frozen battery is REFUSED on the
// re-check, named — never silently accepted (the 2c battery is the elaboration gate).
#[test]
fn fill_with_an_unlisted_tactic_is_refused_by_the_battery() {
    let th = temp_th(
        "battery",
        "lemma le_id(a: u64, b: u64) req a <= b ens a <= b proof { ?p0 }",
    );
    // `sorry` is not in the frozen allowlist.
    let (out, err, _ok) = run_forge(&["fill", th.to_str().unwrap(), "le_id.proof.?p0", "sorry"]);
    let combined = format!("{out}{err}");
    assert!(
        combined.contains("BatteryUnlistedTactic") && combined.contains("sorry"),
        "an unlisted tactic must be refused, named: {combined}"
    );
    let _ = std::fs::remove_file(&th);
}

// The §5.1 fill loop: a fill whose tactics introduce a NEW `?pN` hole re-presents the
// new open hole (and the item stays non-certified until it is closed).
#[test]
fn fill_introducing_a_new_hole_re_presents_it() {
    let th = temp_th(
        "loop",
        "lemma le_id(a: u64, b: u64) req a <= b ens a <= b proof { ?p0 }",
    );
    let (out, err, ok) = run_forge(&[
        "fill",
        th.to_str().unwrap(),
        "le_id.proof.?p0",
        "induction a; ?p1",
    ]);
    assert!(
        ok,
        "`forge fill` should succeed even when it opens a new hole: {out}{err}"
    );
    assert!(
        out.contains("?p1 : open"),
        "the new hole the fill introduced is re-presented: {out}"
    );
    let _ = std::fs::remove_file(&th);
}

// R-CODE-2: a `forge fill` aimed at a NON-hole address (a `fn` root) is a structured
// usage error pointing at `forge edit`, never a silent splice or a panic.
#[test]
fn fill_on_a_non_hole_address_is_an_honest_error() {
    let th = temp_th(
        "nonhole",
        "fn f(n: u32) -> u32 req true ens result == n fx pure { n }",
    );
    let (out, err, ok) = run_forge(&["fill", th.to_str().unwrap(), "f", "omega"]);
    assert!(!ok, "filling a non-hole address must fail");
    let combined = format!("{out}{err}");
    assert!(
        combined.contains("not a hole") && combined.contains("forge edit"),
        "the error must name the contract + point at `forge edit`: {combined}"
    );
    let _ = std::fs::remove_file(&th);
}
