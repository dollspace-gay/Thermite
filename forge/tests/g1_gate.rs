//! The G1 gate cert-oracle (`.design/stage1-forge-tier.md` REQ-10 / AC-14): the
//! `isqrt_class` example, end to end. `forge check --engine forge
//! conformance/forge/isqrt_class.th` drives the per-clause hybrid route — two NON-TRIVIAL
//! relaxable consequences of the integer-sqrt characterization (`r*r <= n < (r+1)^2`)
//! discharged by the nlsat relax route at L4 (the real-arithmetic squeeze, not a restatement
//! of `req`), one non-relaxable (`%`) clause discharged at L3 by the author's `proof for`
//! block — so the certificate exhibits clauses at **L4, L4, L3**, the item level is the MIN
//! (**L3**), and all four forge-tier evidence blocks are populated:
//!
//!   * `covenant_evidence` — from the `witness` block (covenant-before-burn),
//!   * `engine_attribution` + per-clause `engine`/`trust` — the axiom-gate record,
//!   * `contract_quality.mutants_killed` — the re-elaboration mutation score (anti-Goodhart),
//!   * `burn` — the L3 clause's proof tokens + cited frozen simp-lemmas.
//!
//! The route invokes z3 (nlsat, bundled with verus) and lake (the built Lean spine), so the
//! test skips when either is absent (mirroring the sibling live-spine tests); the CI lean
//! job is the authoritative gate. The deterministic oracle fields are pinned against the
//! committed golden `conformance/forge/isqrt_class.cert.json` (R-CHAR-3).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("forge crate has a parent workspace dir")
        .to_path_buf()
}

/// `verus` is reachable (the same skip-guard `check_conformance.rs` uses). z3 ships
/// alongside the verus distribution, so a present verus implies a usable nlsat solver.
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

/// `lake` on PATH / under `~/.elan` — the L3 clause + the re-elaboration mutation need it
/// (the built spine). Absent → the gate route cannot discharge, so the test skips.
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

fn run_forge_gate() -> (Option<i32>, Vec<Value>) {
    let th = repo_root().join("conformance/forge/isqrt_class.th");
    let out = Command::new(forge_bin())
        .arg("check")
        .arg("--engine")
        .arg("forge")
        .arg(&th)
        .arg("--json")
        .output()
        .unwrap_or_else(|e| panic!("spawn forge: {e}"));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!(
            "forge --json must emit one JSON document: {e}\nstdout:\n{stdout}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stderr)
        )
    });
    let arr = value
        .as_array()
        .unwrap_or_else(|| panic!("forge --json must emit a JSON array of certs: {value}"))
        .clone();
    (out.status.code(), arr)
}

fn golden() -> Value {
    let path = repo_root().join("conformance/forge/isqrt_class.cert.json");
    let src = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read golden {}: {e}", path.display()));
    serde_json::from_str(&src).unwrap_or_else(|e| panic!("parse golden: {e}"))
}

/// AC-14 core: the `isqrt_class` example certifies L3 with clauses L4, L4, L3 (engine
/// attribution nlsat / nlsat / lean) and all four evidence blocks present + populated. The
/// two L4 clauses are non-trivial nlsat-earned consequences of the integer-sqrt
/// characterization (the real-arithmetic squeeze), not restatements of `req`.
#[test]
fn isqrt_class_certifies_l3_with_l4_l4_l3_clauses_and_four_evidence_blocks() {
    if !verus_present() || !lake_present() {
        eprintln!(
            "SKIP: verus (z3) and/or lake absent — the G1 gate per-clause hybrid route is not \
             run (set VERUS_BIN + build the Lean spine; the CI lean job is the gate)."
        );
        return;
    }
    let (code, certs) = run_forge_gate();
    assert_eq!(code, Some(0), "the G1 gate example must certify (exit 0)");
    assert_eq!(certs.len(), 1, "one fn, one certificate");
    let cert = &certs[0];
    let golden = golden();

    // (1) Item level == L3 (the MIN over the clauses), == golden.
    assert_eq!(cert["item"], Value::from("isqrt_class"));
    assert_eq!(
        cert["level"],
        Value::from("L3"),
        "item level is the min over clauses"
    );
    assert_eq!(cert["level"], golden["level"]);
    assert!(
        cert.get("reject").is_none() || cert["reject"].is_null(),
        "a certified item has no reject"
    );

    // (2) The three clauses are L4, L4, L3 by their engine attribution: the two relaxable
    // side-conditions route to nlsat (the kernel-grounded L4 rung); the non-relaxable `%`
    // clause routes to the Lean engine (L3). Each verdict is `Proved`.
    let obls = cert["obligations"].as_array().expect("obligations array");
    assert_eq!(
        obls.len(),
        3,
        "three ens clauses, three per-clause obligations"
    );
    assert_eq!(
        obls[0]["engine"],
        Value::from("nlsat"),
        "ens#0 → nlsat (L4)"
    );
    assert_eq!(
        obls[1]["engine"],
        Value::from("nlsat"),
        "ens#1 → nlsat (L4)"
    );
    let l3_engine = obls[2]["engine"].as_str().expect("ens#2 carries an engine");
    assert!(
        l3_engine.starts_with("lean"),
        "ens#2 → the Lean engine (L3), got `{l3_engine}`"
    );
    for (k, o) in obls.iter().enumerate() {
        assert_eq!(
            o["verdict"]["kind"],
            Value::from("Proved"),
            "clause ens#{k} is Proved"
        );
    }

    // (3) All four forge-tier evidence blocks present + populated.
    // (3a) Covenant evidence — deterministic, == golden (Q-ORACLE).
    let cov = &cert["covenant_evidence"];
    assert!(
        cov.is_object(),
        "covenant_evidence present (from the witness block)"
    );
    assert!(
        cov["witness_count"].as_u64().unwrap_or(0) >= 1,
        "≥1 author inhabit witness"
    );
    assert_eq!(
        cov["falsify_refuted"],
        Value::from(0),
        "covenant holds (not refuted)"
    );
    assert_eq!(
        *cov, golden["covenant_evidence"],
        "covenant evidence == golden"
    );

    // (3b) The axiom-gate record (the engine attribution): the L3 Lean discharge is
    // recorded with its named trust base.
    let attr = &cert["engine_attribution"];
    assert!(
        attr.is_object(),
        "engine_attribution present (the axiom-gate record)"
    );
    assert!(
        !attr["trust_profile"]
            .as_array()
            .map(Vec::is_empty)
            .unwrap_or(true),
        "the trust profile names the trusted base"
    );

    // (3c) The re-elaboration mutation score (anti-Goodhart) — a real, populated ratio
    // (not the unscored `0/0`), == golden.
    let mut_killed = cert["contract_quality"]["mutants_killed"]
        .as_str()
        .expect("mutants_killed present");
    assert_ne!(
        mut_killed, "0/0",
        "the mutation score is populated (scored)"
    );
    assert_eq!(
        cert["contract_quality"]["mutants_killed"], golden["contract_quality"]["mutants_killed"],
        "mutation score == golden"
    );

    // (3d) The burn receipt (the L3 clause's proof tokens + cited frozen simp-lemmas).
    let burn = &cert["burn"];
    assert!(
        burn.is_object(),
        "burn receipt present (the L3 author proof)"
    );
    assert!(
        burn["proof_tokens"].as_u64().unwrap_or(0) > 0,
        "the burn receipt counts the committed proof tokens"
    );
    assert!(
        !burn["cited_lemmas"]
            .as_array()
            .map(Vec::is_empty)
            .unwrap_or(true),
        "the burn receipt cites the frozen simp-lemmas"
    );
    assert_eq!(*burn, golden["burn"], "burn receipt == golden");

    // The definition-tower meaning audit is pinned too (the fifth forge-tier datum).
    assert_eq!(
        cert["meaning_audit"], golden["meaning_audit"],
        "meaning audit == golden"
    );
}
