//! AC-7 (`.design/stage1-forge-tier.md` REQ-3): an OPEN `?pN` proof hole blocks
//! certification and build through the shared `goal_repl::open_proof_hole_reason`
//! path — the proof-tier mirror of the `?N` body-hole short-circuit (#193). A
//! forge-tier item carrying an open proof hole is incomplete: it must never
//! certify and never ship a build artifact.
//!
//! These assertions need NO verus: the open-hole short-circuit precedes lowering /
//! verus (exactly like the body-hole `OpenHole` gate in `goal_repl_fill.rs`).

use std::path::PathBuf;
use std::process::Command;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

static TEMP_NONCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn temp_th(tag: &str, src: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "forge_proofhole_{tag}_{}_{}.th",
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

// AC-7: a `lemma` whose proof block carries an open `?p0` is reported by
// `forge check --json` as a non-certified `OpenHole` item — no lowering, no verus
// (the short-circuit precedes it).
#[test]
fn open_proof_hole_blocks_certification() {
    let th = temp_th(
        "cert",
        "lemma add_id(a: u64) req true ens a == a proof { ?p0 }",
    );
    let (cout, cerr, _ok) = run_forge(&["check", th.to_str().unwrap(), "--json"]);
    let combined = format!("{cout}{cerr}");
    assert!(
        combined.contains("OpenHole"),
        "an open `?pN` lemma must carry the OpenHole reject cause: {combined}"
    );
    assert!(
        !combined.contains("ALL GOALS DISCHARGED"),
        "a holed-proof item must NEVER claim discharge: {combined}"
    );
    let _ = std::fs::remove_file(&th);
}

// AC-7: `forge build` refuses an item with an open `?pN` proof hole (it would
// otherwise ship a trust-stamped artifact for an unfinished proof). The refusal
// precedes lowering, so it needs no verus.
#[test]
fn open_proof_hole_blocks_build() {
    let th = temp_th(
        "build",
        "lemma add_id(a: u64) req true ens a == a proof { ?p0 }",
    );
    let (bout, berr, ok) = run_forge(&["build", th.to_str().unwrap()]);
    assert!(!ok, "`forge build` must fail on an open proof hole");
    let combined = format!("{bout}{berr}");
    assert!(
        combined.contains("proof hole") || combined.contains("?p0"),
        "the build refusal must name the open proof hole: {combined}"
    );
    let _ = std::fs::remove_file(&th);
}

// A hole-FREE lemma is NOT rejected for the open-hole reason — the gate fires only
// on an actual open proof hole (no false positives).
#[test]
fn hole_free_lemma_is_not_open_hole_rejected() {
    let th = temp_th(
        "clean",
        "lemma add_id(a: u64) req true ens a == a proof { omega }",
    );
    let (cout, cerr, _ok) = run_forge(&["check", th.to_str().unwrap(), "--json"]);
    let combined = format!("{cout}{cerr}");
    assert!(
        !combined.contains("OpenHole"),
        "a hole-free lemma must not be OpenHole-rejected: {combined}"
    );
    let _ = std::fs::remove_file(&th);
}
