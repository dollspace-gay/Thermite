//! Adversarial critic probes for forge's solver-backed tautology +
//! vacuous-precondition checks (issue #13, `thermite-design.md` §7 steps 2-3,
//! `.design/forge/solver-vacuity.md`).
//!
//! The crux these tests guard is the dangerous direction: a false positive,
//! flagging a genuinely good contract as a tautology / vacuous precondition and
//! rejecting valid code. Every "good" case below uses a fresh contract (not the
//! `conformance/solver-vacuity/cases.json` oracle fixtures, not `sum`/
//! `binary_search`) whose `ens` constrains the result or whose `req` is
//! satisfiable; the authority (§7 / AC-1 of `solver-vacuity.md`) says
//! such a contract must reach L3 with both `contract_quality` bools `false`. The
//! "true positive" cases use fresh degeneracies (a different tautology / a
//! different unsat `req` than the fixtures) and the authority (AC-2 / AC-3) says
//! they must be detected, so the gate is not neutered.
//!
//! R-CHAR-3: every expected value traces to `thermite-design.md` §7 ("is `ens`
//! provable from `req` + types without the body" → reject; "is `req`
//! satisfiable" → reject when not) and `.design/forge/solver-vacuity.md`
//! AC-1/AC-2/AC-3 + the "Resolved" vacuity-first ordering, not literal-copied
//! from forge's own output.
//!
//! These checks issue real verus queries, so they skip with an eprintln when verus is
//! absent (mirroring `solver_vacuity_conformance.rs`), never panic. `unwrap`/
//! `expect` are fine here — `tests/` is not anti-pattern-gated.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// `true` iff verus is reachable (mirrors `solver_vacuity_conformance.rs`).
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

fn unique() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    N.fetch_add(1, Ordering::Relaxed)
}

fn write_temp(name: &str, program: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "forge_divsolvervac_{}_{}_{name}.th",
        std::process::id(),
        unique()
    ));
    std::fs::write(&path, program).unwrap_or_else(|e| panic!("write temp {name}: {e}"));
    path
}

/// A hermetic proof-cache dir per run (the #13 verdict is cached with the
/// item; a shared cache would leak verdicts across cases).
fn unique_cache_dir() -> PathBuf {
    std::env::temp_dir().join(format!(
        "forge_divsolvervac_cache_{}_{}",
        std::process::id(),
        unique()
    ))
}

/// Run `forge check <program> --json` on a temp file, return the first cert.
fn check(name: &str, program: &str) -> Value {
    let path = write_temp(name, program);
    let cache_dir = unique_cache_dir();
    let _ = std::fs::remove_dir_all(&cache_dir);
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(&path)
        .arg("--json")
        .env("FORGE_CACHE_DIR", &cache_dir)
        .output()
        .unwrap_or_else(|e| panic!("spawn forge: {e}"));
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir_all(&cache_dir);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!(
            "forge --json must emit one JSON document: {e}\nstdout:\n{stdout}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stderr)
        )
    });
    value
        .as_array()
        .and_then(|a| a.first())
        .cloned()
        .unwrap_or_else(|| panic!("no certificate emitted for `{name}`: {value}"))
}

fn level(c: &Value) -> &str {
    c["level"].as_str().unwrap_or("<no level>")
}
fn taut(c: &Value) -> bool {
    c["contract_quality"]["tautology"].as_bool().unwrap_or(true)
}
fn vac(c: &Value) -> bool {
    c["contract_quality"]["vacuous_precondition"]
        .as_bool()
        .unwrap_or(true)
}
fn cause(c: &Value) -> Option<&str> {
    c.get("reject")
        .and_then(|r| r.get("cause"))
        .and_then(|v| v.as_str())
}

// ===========================================================================
// The crux — false-positive guard: a good contract must not be flagged.
// Authority: `.design/forge/solver-vacuity.md` AC-1 ("verus fails to prove `ens`
// for an arbitrary result" → tautology=false; "verus fails to prove
// `assert(false)` under a satisfiable `req`" → vacuous_precondition=false → L3);
// `thermite-design.md` §7 ("a function does not certify until its contract
// certifies"; a non-degenerate contract must be allowed to certify).
// ===========================================================================

/// A good contract `ens result >= x` constrains the result (it does not hold
/// for an arbitrary `u32`). Must reach L3, not flagged. (§7 step 2: the ens is
/// not provable for an arbitrary result, so it is no tautology.)
#[test]
fn good_ens_ge_x_is_clean_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — false-positive guard needs the harness proof.");
        return;
    }
    let c = check(
        "ge_x",
        "fn keep(x: u32) -> u32\n  req x > 0\n  ens result >= x\n  fx pure\n{ x }\n",
    );
    assert_eq!(
        level(&c),
        "L3",
        "good `ens result >= x` must certify L3: {c}"
    );
    assert!(
        !taut(&c),
        "good `ens result >= x` MUST NOT be flagged tautology: {c}"
    );
    assert!(
        !vac(&c),
        "good `ens result >= x` MUST NOT be flagged vacuous: {c}"
    );
}

/// A good contract `ens result == x` with a satisfiable conjunction `req`.
/// Authority AC-1: a satisfiable `req` is not vacuous; a result-constraining
/// `ens` is not a tautology → L3, both bools false.
#[test]
fn good_ens_eq_x_satisfiable_conjunction_is_clean_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent.");
        return;
    }
    let c = check(
        "eq_x",
        "fn id1(x: u32) -> u32\n  req x > 5 && x < 100\n  ens result == x\n  fx pure\n{ x }\n",
    );
    assert_eq!(
        level(&c),
        "L3",
        "good `ens result == x` must certify L3: {c}"
    );
    assert!(!taut(&c), "MUST NOT be flagged tautology: {c}");
    assert!(
        !vac(&c),
        "satisfiable `req x > 5 && x < 100` MUST NOT be flagged vacuous: {c}"
    );
}

/// A good multi-clause constraining `ens` (`result >= x && result <= x`).
/// Both clauses constrain the result; the conjunction is no tautology → L3.
#[test]
fn good_multiclause_constraining_ens_is_clean_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent.");
        return;
    }
    let c = check(
        "multi",
        "fn clampy(x: u32) -> u32\n  req x > 0 && x < 50\n  ens result >= x && result <= x\n  fx pure\n{ x }\n",
    );
    assert_eq!(
        level(&c),
        "L3",
        "good multi-clause ens must certify L3: {c}"
    );
    assert!(!taut(&c), "MUST NOT be flagged tautology: {c}");
    assert!(!vac(&c), "MUST NOT be flagged vacuous: {c}");
}

/// The OQ-4 probe — a non-trivial param/return shape: a slice param with a
/// constraining `ens` over `xs.len()`. The arbitrary-`result` binder
/// (`result: usize`) plus the `&[u32]` proof-fn param must not accidentally
/// constrain `result` or prove the ens for an arbitrary result → L3, not flagged.
/// Authority: AC-1 + OQ-4 ("the `result` param must not be bound to the body's
/// output"); a false positive here means the arbitrary-result encoding is broken.
#[test]
fn good_slice_param_lenconstraining_ens_is_clean_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent.");
        return;
    }
    let c = check(
        "slicelen",
        "fn firstlen(xs: &[u32]) -> usize\n  req xs.len() > 0\n  ens result == xs.len()\n  fx pure\n{ xs.len() }\n",
    );
    assert_eq!(level(&c), "L3", "good slice-param ens must certify L3: {c}");
    assert!(
        !taut(&c),
        "the arbitrary-result binder over a slice-param fn MUST NOT prove a constraining ens: {c}"
    );
    assert!(
        !vac(&c),
        "satisfiable `xs.len() > 0` MUST NOT be flagged vacuous: {c}"
    );
}

// ===========================================================================
// True-positive — the gate must still catch real vacuity (not neutered).
// Authority: §7 step 2/3; AC-2 (tautology → SemanticTautology, tautology=true);
// AC-3 (unsat req → VacuousPrecondition, vacuous_precondition=true).
// Fresh degeneracies, different from the `cases.json` fixtures.
// ===========================================================================

/// A tautology distinct from the fixture (`nonneg`'s `result >= 0`): an upper
/// bound that holds for every `u32` (`result <= 4294967295`). Authority §7 step 2
/// + AC-2 → detected as `SemanticTautology`, `tautology == true`, L0.
#[test]
fn fresh_tautology_upper_bound_is_detected() {
    if !verus_present() {
        eprintln!("SKIP: verus absent.");
        return;
    }
    let c = check(
        "taut_le",
        "fn anyu(x: u32) -> u32\n  req x > 0\n  ens result <= 4294967295\n  fx pure\n{ x }\n",
    );
    assert_eq!(level(&c), "L0", "a real tautology must NOT certify: {c}");
    assert!(
        taut(&c),
        "`ens result <= u32::MAX` is a tautology (§7 step 2): {c}"
    );
    assert_eq!(
        cause(&c),
        Some("SemanticTautology"),
        "fresh tautology must report the SemanticTautology cause (AC-2/AC-5): {c}"
    );
}

/// An unsat `req` distinct from the fixture (`x > 0 && x < 0`): `x > 100 &&
/// x < 10`. Authority §7 step 3 + AC-3 → detected as `VacuousPrecondition`,
/// `vacuous_precondition == true`, L0.
#[test]
fn fresh_unsat_req_is_detected() {
    if !verus_present() {
        eprintln!("SKIP: verus absent.");
        return;
    }
    let c = check(
        "unsat",
        "fn dead(x: u32) -> u32\n  req x > 100 && x < 10\n  ens result == x\n  fx pure\n{ x }\n",
    );
    assert_eq!(
        level(&c),
        "L0",
        "an unsat-req contract must NOT certify: {c}"
    );
    assert!(
        vac(&c),
        "`req x > 100 && x < 10` is unsatisfiable (§7 step 3): {c}"
    );
    assert_eq!(
        cause(&c),
        Some("VacuousPrecondition"),
        "fresh unsat req must report the VacuousPrecondition cause (AC-3/AC-5): {c}"
    );
}

// ===========================================================================
// Vacuity-first ordering — `.design/forge/solver-vacuity.md` "Resolved"
// (CHECK-ORDER): an unsat `req` makes every `ensures` vacuously provable, so the
// root cause must be reported as `VacuousPrecondition`, not mislabeled
// `SemanticTautology`. Conversely a genuine tautology with a satisfiable `req`
// must still report `SemanticTautology`.
// ===========================================================================

/// An unsat `req` plus an `ens` that is also independently a tautology. The
/// tautology harness would also prove (a false premise proves anything), but the
/// authority's vacuity-first ordering mandates `VacuousPrecondition` (the true
/// root cause), not `SemanticTautology`.
#[test]
fn unsat_req_with_tautological_ens_is_reported_as_vacuous_not_tautology() {
    if !verus_present() {
        eprintln!("SKIP: verus absent.");
        return;
    }
    let c = check(
        "order",
        "fn both(x: u32) -> u32\n  req x > 100 && x < 10\n  ens result <= 4294967295\n  fx pure\n{ x }\n",
    );
    assert_eq!(level(&c), "L0", "must not certify: {c}");
    assert_eq!(
        cause(&c),
        Some("VacuousPrecondition"),
        "unsat req + tautological ens must report the ROOT cause VacuousPrecondition (CHECK-ORDER): {c}"
    );
    assert!(
        vac(&c) && !taut(&c),
        "the vacuous bool is set, the tautology bool is NOT (the unsat req is the defect): {c}"
    );
}

/// A genuine tautology with a satisfiable `req` (`x < 100`): the vacuity check
/// passes (req is satisfiable), so the tautology check runs and fires. Authority:
/// the tautology check runs only on a satisfiable precondition → `SemanticTautology`.
#[test]
fn tautology_with_satisfiable_req_is_reported_as_tautology() {
    if !verus_present() {
        eprintln!("SKIP: verus absent.");
        return;
    }
    let c = check(
        "taut_sat",
        "fn t(x: u32) -> u32\n  req x < 100\n  ens result >= 0\n  fx pure\n{ x }\n",
    );
    assert_eq!(level(&c), "L0", "must not certify: {c}");
    assert_eq!(
        cause(&c),
        Some("SemanticTautology"),
        "satisfiable req + tautological ens must report SemanticTautology: {c}"
    );
    assert!(
        taut(&c) && !vac(&c),
        "the tautology bool is set, the vacuous bool is NOT (the req is satisfiable): {c}"
    );
}
