//! Divergence tests for forge's §7.1 structural vacuity triage (issue #6,
//! commit 838374d). Authored by acto-critic: each test pins a place where the
//! implementation diverges from the governing authority
//! (`.design/forge/vacuity-triage.md` + `thermite-design.md` §7.1) and FAILS
//! against the current toolchain.
//!
//! forge is a pure `bin` crate (no `lib.rs`), so these drive the BUILT `forge`
//! binary (same pattern as `vacuity_slag_conformance.rs`) and assert the emitted
//! certificate's `reject.cause` / `level`. Expected values trace to the design
//! REQ wording (R-CHAR-3), NEVER to forge's own output.
//!
//! `unwrap`/`expect` are fine here — `tests/` is not anti-pattern-gated.

use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

fn unique() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    N.fetch_add(1, Ordering::Relaxed)
}

fn write_temp(name: &str, program: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "forge_divvac_{}_{}_{name}.th",
        std::process::id(),
        unique()
    ));
    std::fs::write(&path, program).unwrap_or_else(|e| panic!("write temp {name}: {e}"));
    path
}

/// Run `forge check <file> --json`, returning the first certificate as JSON.
fn first_cert(program: &str, name: &str) -> Value {
    let path = write_temp(name, program);
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(&path)
        .arg("--json")
        .output()
        .unwrap_or_else(|e| panic!("spawn forge: {e}"));
    let _ = std::fs::remove_file(&path);
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
        .unwrap_or_else(|| panic!("no certificate emitted for `{name}`"))
}

fn reject_cause(cert: &Value) -> Option<String> {
    cert.get("reject")
        .and_then(|r| r.get("cause"))
        .and_then(|c| c.as_str())
        .map(|s| s.to_string())
}

fn verus_present() -> bool {
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

/// Divergence: `vacuity::ens_implied_by_req` OVER-rejects with §7.1 (c) when only
/// SOME — not every — `ens` clause is implied by `req`.
///
/// Authority: `.design/forge/vacuity-triage.md` REQ-3 — the (c) reject is defined
/// as "**every** `ens` clause's `Clause.expr` is structurally identical
/// (`PartialEq`) to ... the whole `req` ... or one of the conjuncts of `req`".
/// `thermite-design.md` §7.1: "`ens` is syntactically implied by `req` alone →
/// reject" — the WHOLE postcondition conjunction must be implied; a single
/// implied conjunct alongside a genuinely-stronger clause is NOT a vacuous `ens`.
///
/// The implementation (`ens_implied_by_req`, `vacuity.rs`) returns `Some(idx)` on
/// the FIRST matching clause, so it rejects when ANY clause is a req conjunct.
///
/// Input: `req x > 0 && x < 10` / `ens x > 0` (an implied conjunct) AND
/// `ens result == x` (a REAL postcondition NOT implied by `req`). Per REQ-3 the
/// whole `ens` is NOT implied by `req` (the `result == x` obligation constrains
/// the body), so triage must NOT reject (c). Expected: not an `EnsImpliedByReq`
/// reject (the contract is non-vacuous; it proceeds to L3 verification).
///
/// Tracking: filed as a `-l blocker`.
#[test]
fn divergence_ens_implied_by_req_over_rejects_partial_implication() {
    // `result == x` is a genuinely-stronger conjunct of `ens`; the whole `ens`
    // is therefore NOT implied by `req` alone (REQ-3 requires EVERY clause).
    let cert = first_cert(
        "fn f(x: u32) -> u32 req x > 0 && x < 10 ens x > 0 ens result == x fx pure { x }",
        "c_partial_implication",
    );
    assert_ne!(
        reject_cause(&cert).as_deref(),
        Some("EnsImpliedByReq"),
        "vacuity-triage.md REQ-3 / §7.1 (c): the (c) reject requires EVERY ens \
         clause be implied by req; `ens result == x` is a genuinely-stronger \
         postcondition NOT implied by `req x > 0 && x < 10`, so the whole `ens` is \
         not req-implied and triage must NOT reject (c). cert: {cert}"
    );
}

/// Companion (control): a contract whose ONLY `ens` clause IS a req conjunct is a
/// genuine (c) reject — pins that the fix narrows the rule to the "every clause"
/// reading WITHOUT regressing the true-positive. Authority:
/// `conformance/vacuity/triage.json` `ens_conjunct_req` (cause `EnsImpliedByReq`).
#[test]
fn ens_fully_implied_by_req_still_rejects_c() {
    let cert = first_cert(
        "fn f(x: u32) -> () req x > 0 && x < 10 ens x > 0 fx pure { }",
        "c_full_implication",
    );
    assert_eq!(
        reject_cause(&cert).as_deref(),
        Some("EnsImpliedByReq"),
        "the oracle `ens_conjunct_req` case (sole ens clause = a req conjunct) is \
         a genuine §7.1 (c) reject. cert: {cert}"
    );
}

/// Divergence (same root cause, OQ-4 boundary): a contract carrying a redundant
/// `ens true` clause ALONGSIDE a real `ens result == x` clause is wrongly
/// rejected with §7.1 (c) (`ens true` matches the `req true` conjunct).
///
/// Authority: `.design/forge/vacuity-triage.md` OQ-4 — "A contract `ens true` +
/// `ens result == x` is NOT (a)-rejected (it carries a real conjunct)." REQ-3
/// applies the same "every clause" logic: the whole `ens` is implied by `req`
/// only if every clause is, and `result == x` is not implied by `req true`. So
/// this contract is non-vacuous and must NOT be a (c) reject.
///
/// Tracking: filed as a `-l blocker` (same blocker as the partial-implication
/// divergence — one root cause in `ens_implied_by_req`).
#[test]
fn divergence_redundant_true_clause_with_real_clause_not_c_rejected() {
    let cert = first_cert(
        "fn f(x: u32) -> u32 req true ens true ens result == x fx pure { x }",
        "multi_ens_true_plus_real",
    );
    // The whole `ens` (true && result == x) is NOT implied by `req true`
    // (`result == x` is a real obligation), so (c) must not fire.
    assert_ne!(
        reject_cause(&cert).as_deref(),
        Some("EnsImpliedByReq"),
        "vacuity-triage.md REQ-3 + OQ-4: `ens true ens result == x` carries a real \
         conjunct (`result == x`) not implied by `req true`; the whole `ens` is \
         not req-implied, so triage must NOT reject (c). cert: {cert}"
    );
}

/// Guard (NOT a divergence — pins correct behavior so a fix cannot regress it):
/// `Lt`/`Ne` identity operands (`x < x`, `x != x`) are NOT syntactically `true`
/// and must NOT be (a)-rejected. Authority: `.design/forge/vacuity-triage.md`
/// REQ-1 — identity is `Eq`/`Le`/`Ge` only; "`<`/`>`/`!=` are NOT identities
/// (`x < x` is false)". With a unit return (so (b) is exempt) and a non-implied
/// `req`, these pass triage and reach verus.
#[test]
fn lt_ne_identity_is_not_a_rejected() {
    for (prog, label) in [
        (
            "fn f(x: u32) -> () req true ens x < x fx pure { }",
            "x_lt_x",
        ),
        (
            "fn f(x: u32) -> () req true ens x != x fx pure { }",
            "x_ne_x",
        ),
    ] {
        let cert = first_cert(prog, label);
        assert_ne!(
            reject_cause(&cert).as_deref(),
            Some("EnsIsTrivial"),
            "REQ-1: `{label}` is not a trivially-true identity (Lt/Ne), must not be \
             (a)-rejected. cert: {cert}"
        );
        if verus_present() {
            // It is a real (unprovable) postcondition, so it reaches verus and is
            // reported non-L3 — never silently certified.
            assert_ne!(
                cert.get("level").and_then(|l| l.as_str()),
                Some("L3"),
                "`{label}` is an unprovable postcondition; it must reach verus and \
                 NOT certify L3. cert: {cert}"
            );
        }
    }
}
