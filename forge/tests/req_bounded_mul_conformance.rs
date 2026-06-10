//! LIVE L3 conformance for the var*var overflow discharge (#196,
//! `.design/lower/verus-lowering.md` REQ-7). Drives the BUILT `forge` binary
//! with `check --json` over a temp `.th` fixture and asserts the certificate
//! `level`. This is the cert-oracle end of the #196 fix: the lowerer emits the
//! req-bounded-mul `by(nonlinear_arith)` aid (pinned in
//! `thermite-lower/tests/req_bounded_mul_aid.rs`) and verus, run by forge,
//! discharges the overflow obligation → L3.
//!
//! Mirrors `forge/tests/check_conformance.rs`: drive the binary, parse JSON,
//! SKIP LOUDLY if verus is absent (never panic on a missing solver). `tests/`
//! is not anti-pattern-gated, so `unwrap`/`expect` are fine here.
//!
//! Expected values are HAND-DERIVED from the design contract + the user's
//! `/goal` spec (R-CHAR-3): `sq` under `req n <= 30` MUST certify L3 (it
//! currently fails "possible arithmetic underflow/overflow" without the aid);
//! the UNBOUNDED `n * m` case MUST NOT certify L3 (the obligation honestly
//! fails — no fabricated aid, R-DEFER-9).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

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

/// Write `src` to a unique temp `.th`, run `forge check <file> --json`, return
/// the parsed cert array. The temp file is removed before returning.
fn check_program(tag: &str, src: &str) -> Vec<Value> {
    let fixture = std::env::temp_dir().join(format!("forge_mul_{tag}_{}.th", std::process::id()));
    std::fs::write(&fixture, src).expect("write fixture");
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(&fixture)
        .arg("--json")
        .output()
        .unwrap_or_else(|e| panic!("spawn forge: {e}"));
    let _ = std::fs::remove_file(&fixture);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!(
            "forge --json stdout must be one JSON document: {e}\nstdout:\n{stdout}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stderr)
        )
    });
    value
        .as_array()
        .unwrap_or_else(|| panic!("forge --json must emit an array: {value}"))
        .clone()
}

fn level_of<'a>(certs: &'a [Value], item: &str) -> &'a str {
    certs
        .iter()
        .find(|c| c.get("item").and_then(|v| v.as_str()) == Some(item))
        .and_then(|c| c.get("level"))
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("no level for item `{item}` in {certs:?}"))
}

/// The user's exact `/goal` case: `sq(n) req n <= 30 ens result == n * n`
/// certifies L3 — the var*var overflow discharge bites (#196). WITHOUT the
/// aid this fails "possible arithmetic underflow/overflow"; WITH it, verus
/// proves the bound via `nonlinear_arith` and the body's `n * n` overflow
/// obligation discharges.
#[test]
fn sq_certifies_l3_via_req_bounded_mul_aid() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — `forge check sq` L3 not run.");
        return;
    }
    let certs = check_program(
        "sq",
        "fn sq(n: u64) -> u64\n  req n <= 30\n  ens result == n * n\n  fx pure\n{\n  n * n\n}\n",
    );
    assert_eq!(
        level_of(&certs, "sq"),
        "L3",
        "sq must certify L3 via the #196 var*var aid: {certs:?}"
    );
}

/// HONEST NON-L3 — an UNBOUNDED factor `n * m` (no req bound on `m`): the
/// overflow obligation honestly FAILS (no fabricated aid), so `mul_nm` does
/// NOT certify L3 (#196, R-DEFER-9). The negative control proving the aid is
/// not a blanket cheat.
#[test]
fn unbounded_product_does_not_certify_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — unbounded-product non-L3 not run.");
        return;
    }
    let certs = check_program(
        "nm",
        "fn mul_nm(n: u64, m: u64) -> u64\n  req n <= 30\n  ens result == n * m\n  fx pure\n{\n  n * m\n}\n",
    );
    assert_ne!(
        level_of(&certs, "mul_nm"),
        "L3",
        "an unbounded product must NOT be lifted to L3 by a fabricated aid \
         (#196, R-DEFER-9): {certs:?}"
    );
}
