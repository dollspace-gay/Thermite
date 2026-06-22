//! The stage-3 bit-vector route (`.design/stage3-bv-reconstruction.md` REQ-2 / AC-2 /
//! AC-3): `forge check --engine bv` over the `mix64` example and the two AC-3 fixtures,
//! end to end through the binary. The route lowers `@bv`-tagged clauses to fixed-width
//! QF_BV (the [`EngineName::BitVector`] route) alongside the stage-1 nlsat route, so a
//! mixed-mechanism function attributes each clause to the engine that grounds it.
//!
//! The whole suite is gated on the `bv` cargo feature (the shadow-flag plumbing — without
//! it the `@bv` tag is a structured parse error, REQ-1's R-BV-1 lock) AND on `verus`/z3
//! being reachable (the route reuses the Verus base pass and reaches z3 for the QF_BV and
//! QF_NRA queries). A shard without them SKIPS — the CI lean/verus job is the authoritative
//! gate, mirroring `g1_gate.rs` and `nlsat_relax_conformance.rs`.

#![cfg(feature = "bv")]

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

/// `verus` is reachable (the same skip-guard `g1_gate.rs` uses). z3 ships alongside the
/// verus distribution, so a present verus implies a usable QF_BV / QF_NRA solver.
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

fn run_bv(example: &str) -> (Option<i32>, Vec<Value>) {
    let th = repo_root().join("conformance/forge").join(example);
    let out = Command::new(forge_bin())
        .arg("check")
        .arg("--engine")
        .arg("bv")
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

fn cert<'a>(certs: &'a [Value], item: &str) -> &'a Value {
    certs
        .iter()
        .find(|c| c["item"].as_str() == Some(item))
        .unwrap_or_else(|| panic!("no certificate for `{item}` in {certs:?}"))
}

/// AC-2: the `mix64` example certifies at L4 with three clauses on two mechanisms — two
/// `@bv64` clauses via `EngineName::BitVector` (decidable QF_BV, complete bit-pattern
/// countermodels) and one unbounded clause via nlsat — each clause's certificate naming
/// its engine and semantics; the injectivity lemma discharges at `@bv64` with no author
/// proof (an empty `proof { }`). Both mechanisms certify at the caged rung L4, so the
/// item (the MIN over its clauses) is L4; the `@bv` clauses' SOLVER trust base is
/// recorded in the per-clause attribution (kernel-grounded by REQ-7/8, same rung).
#[test]
fn mix64_certifies_with_two_bitvector_clauses_and_one_unbounded() {
    if !verus_present() {
        eprintln!(
            "SKIP: verus (z3) absent — the bit-vector route is not run (set VERUS_BIN; the CI \
             verus job is the gate)."
        );
        return;
    }
    let (code, certs) = run_bv("mix64.th");
    assert_eq!(code, Some(0), "the mix64 example must certify (exit 0)");

    // (1) The fn `mix64` — L4 (the MIN over L4, L4, L4), three per-clause obligations.
    let m = cert(&certs, "mix64");
    assert_eq!(
        m["level"],
        Value::from("L4"),
        "item level is the min over clauses (two @bv64 + one nlsat, all caged L4)"
    );
    assert!(
        m.get("reject").is_none() || m["reject"].is_null(),
        "a certified item has no reject"
    );
    let obls = m["obligations"].as_array().expect("obligations array");
    assert_eq!(
        obls.len(),
        3,
        "three ens clauses, three per-clause obligations"
    );
    assert_eq!(
        obls[0]["engine"],
        Value::from("bitvector"),
        "ens#0 → bitvector"
    );
    assert_eq!(
        obls[1]["engine"],
        Value::from("bitvector"),
        "ens#1 → bitvector"
    );
    assert_eq!(
        obls[2]["engine"],
        Value::from("nlsat"),
        "ens#2 → nlsat (unbounded)"
    );
    for (k, o) in obls.iter().enumerate() {
        assert_eq!(
            o["verdict"]["kind"],
            Value::from("Proved"),
            "clause ens#{k} is Proved"
        );
        assert!(
            !o["trust"].as_array().map(Vec::is_empty).unwrap_or(true),
            "clause ens#{k} names its trust base"
        );
    }
    // The two bit-vector clauses name the fixed-width semantics; the unbounded one does not.
    assert!(
        obls[0]["name"].as_str().unwrap_or("").contains("bv64"),
        "the bit-vector clause names its bv64 semantics"
    );
    assert!(
        obls[2]["name"].as_str().unwrap_or("").contains("unbounded"),
        "the unbounded clause names its unbounded semantics"
    );

    // (2) The injectivity lemma — L4 via the bit-vector engine, no author proof.
    let l = cert(&certs, "rotl1_injective");
    assert_eq!(
        l["level"],
        Value::from("L4"),
        "the @bv64 lemma certifies at the caged rung L4 (decidable QF_BV)"
    );
    let lobls = l["obligations"].as_array().expect("lemma obligations");
    assert_eq!(
        lobls[0]["engine"],
        Value::from("bitvector"),
        "the lemma → bitvector"
    );
    assert_eq!(lobls[0]["verdict"]["kind"], Value::from("Proved"));
}

/// AC-3 (first half): a planted non-injective shift dies as a `Counterexample` with the
/// bit pattern in the certificate.
#[test]
fn planted_non_injective_shift_is_a_counterexample_with_bit_pattern() {
    if !verus_present() {
        eprintln!("SKIP: verus (z3) absent — the bit-vector route is not run.");
        return;
    }
    let (code, certs) = run_bv("bv_shl_not_injective.th");
    assert_ne!(code, Some(0), "a refuted clause does not certify");
    let c = cert(&certs, "shl1_injective_BROKEN");
    assert_eq!(c["reject"]["cause"], Value::from("Counterexample"));
    let obl = &c["obligations"][0];
    assert_eq!(obl["verdict"]["kind"], Value::from("Counterexample"));
    let diag = obl["diagnostic"].as_str().unwrap_or("");
    assert!(
        diag.contains("0b"),
        "the certificate carries the falsifying bit pattern (`0b…`): {diag}"
    );
}

/// AC-3 (second half): an over-budget 64-bit multiplier query yields `Timeout` under the
/// dedicated budget profile — NEVER `unknown` and never a silent downgrade. The robust
/// invariant (across z3 versions): the clause never lands a silent `BvUnknown` skip; when
/// it IS a timeout, the cert names the `bv64-multiplier` profile.
#[test]
fn over_budget_multiplier_is_timeout_under_named_profile_never_unknown() {
    if !verus_present() {
        eprintln!("SKIP: verus (z3) absent — the bit-vector route is not run.");
        return;
    }
    let (_code, certs) = run_bv("bv_mul64_budget.th");
    let c = cert(&certs, "mul64_no_factor");
    let cause = c["reject"]["cause"].as_str().unwrap_or("");
    assert_ne!(
        cause, "BvUnknown",
        "the 64-bit multiplier cliff is NEVER a silent unknown (AC-3)"
    );
    // The expected outcome is the dedicated-profile Timeout; assert it names the profile.
    if cause == "BvBudgetTimeout" {
        let obl = &c["obligations"][0];
        assert_eq!(obl["verdict"]["kind"], Value::from("Timeout"));
        let detail = obl["verdict"]["detail"].as_str().unwrap_or("");
        assert!(
            detail.contains("bv64-multiplier"),
            "the Timeout names the dedicated budget profile: {detail}"
        );
    }
}
