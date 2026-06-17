//! `forge/tests/nlsat_relax_conformance.rs` — the REQ-8 relax-route acceptance test
//! (`.design/stage1-forge-tier.md` REQ-8 / AC-12, increment 2f). Exercises the
//! `forge check --engine nlsat` binary surface end to end:
//!
//! - the isqrt characterization certifies **L4** push-button with `engine: nlsat`
//!   attribution (the relaxable squeeze admitted, real-valid → kernel-grounded);
//! - `∀ n. n*n ≠ 2` (true over ℤ, false over ℝ) yields a **RealWitness** carrying the
//!   raw real point (√2), NEVER a `Counterexample`;
//! - a div-containing clause is rejected by the relaxable check (an honest skip).
//!
//! Live: gated on `z3` (the relax route issues a direct Z3 nlsat QF_NRA query). CI
//! test-shards without z3 SKIP rather than fail, mirroring the sibling lake-gated live
//! tests (`divergence_249_axiom_mask.rs`).

use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// The z3 skip-guard (mirrors `divergence_249_axiom_mask.rs`'s `lake_present`). z3 is
/// bundled alongside the verus distribution and resolved on PATH.
fn z3_present() -> bool {
    Command::new("z3")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Write `src` to a fresh scratch `.th` and `forge check --engine nlsat --json` it,
/// returning the first cert's JSON value.
fn nlsat_check(stem: &str, src: &str) -> Value {
    let dir = std::env::temp_dir().join(format!(
        "forge_nlsat_{stem}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir creatable");
    let th = dir.join(format!("{stem}.th"));
    std::fs::write(&th, src).expect("source writable");
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(&th)
        .arg("--engine")
        .arg("nlsat")
        .arg("--json")
        .output()
        .expect("forge invokes");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let _ = std::fs::remove_dir_all(&dir);
    let v: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("forge --json emitted parseable JSON (err {e}); got: {stdout}"));
    v.as_array()
        .and_then(|a| a.first())
        .cloned()
        .unwrap_or_else(|| panic!("forge emitted at least one cert; got: {stdout}"))
}

// AC-12: the isqrt example certifies L4 push-button with `engine: nlsat` attribution.
// The characterization `r*r<=n ∧ n<(r+1)² ∧ 1<=r → r<=n` is a real-valid universal
// polynomial implication; the nlsat relax route proves the relaxation unsat-negation
// over ℝ and certifies the integer clause at the kernel-grounded L4.
#[test]
fn nlsat_isqrt_certifies_l4_pushbutton() {
    if !z3_present() {
        eprintln!("SKIP: z3 not present — the nlsat relax route is not run.");
        return;
    }
    let cert = nlsat_check(
        "isqrt",
        "fn isqrt_bound(n: u64, r: u64) -> u64\n  \
         req r * r <= n && n < (r + 1) * (r + 1) && 1 <= r\n  \
         ens r <= n\n  fx pure\n{ r }\n",
    );
    assert_eq!(
        cert.get("level").and_then(Value::as_str),
        Some("L4"),
        "the isqrt characterization certifies L4 via nlsat; got cert: {cert}"
    );
    // The engine attribution names nlsat (the kernel-grounded relax route).
    let engine = cert
        .get("engine_attribution")
        .and_then(|a| a.get("engine"))
        .and_then(Value::as_str);
    assert_eq!(
        engine,
        Some("nlsat"),
        "engine: nlsat attribution; got cert: {cert}"
    );
    // The per-clause verdict is Proved.
    let verdict_kind = cert
        .get("obligations")
        .and_then(Value::as_array)
        .and_then(|o| o.first())
        .and_then(|o| o.get("verdict"))
        .and_then(|v| v.get("kind"))
        .and_then(Value::as_str);
    assert_eq!(
        verdict_kind,
        Some("Proved"),
        "per-clause verdict Proved; got: {cert}"
    );
}

// AC-12: a true-over-ℤ, false-over-ℝ claim (`∀ n. n*n ≠ 2`) yields a RealWitness
// carrying the real point, NEVER a Counterexample.
#[test]
fn nlsat_n_squared_ne_two_is_real_witness_never_counterexample() {
    if !z3_present() {
        eprintln!("SKIP: z3 not present — the nlsat relax route is not run.");
        return;
    }
    let cert = nlsat_check(
        "sqrt2",
        "fn sq(n: u64) -> u64\n  req true\n  ens n * n != 2\n  fx pure\n{ n }\n",
    );
    let verdict = cert
        .get("obligations")
        .and_then(Value::as_array)
        .and_then(|o| o.first())
        .and_then(|o| o.get("verdict"))
        .unwrap_or_else(|| panic!("the cert carries a per-clause verdict; got: {cert}"));
    assert_eq!(
        verdict.get("kind").and_then(Value::as_str),
        Some("RealWitness"),
        "`∀ n. n*n≠2` yields RealWitness, NEVER Counterexample; got verdict: {verdict}"
    );
    // The RealWitness carries the raw real point (n ≈ √2).
    let n_val = verdict
        .get("point")
        .and_then(|p| p.get("assignment"))
        .and_then(Value::as_array)
        .and_then(|a| {
            a.iter().find_map(|pair| {
                let pair = pair.as_array()?;
                if pair.first()?.as_str()? == "n" {
                    pair.get(1)?.as_str().map(str::to_owned)
                } else {
                    None
                }
            })
        })
        .unwrap_or_default();
    assert!(
        n_val.starts_with("1.41"),
        "the RealWitness carries the raw real point n ≈ √2 (got `{n_val}`); cert: {cert}"
    );
    // And it is NOT a counterexample reject (the headline: it escalates UP, not down).
    assert_ne!(
        cert.get("reject")
            .and_then(|r| r.get("cause"))
            .and_then(Value::as_str),
        Some("Counterexample"),
        "a RealWitness is NEVER a Counterexample"
    );
}

// REQ-8b: a div-containing clause is rejected by the relaxable check (an honest skip,
// non-certified — not relaxable, never a false verdict). No z3 needed.
#[test]
fn nlsat_div_clause_is_not_relaxable_skip() {
    let cert = nlsat_check(
        "divclause",
        "fn g(n: u64) -> u64\n  req true\n  ens result == n / 2\n  fx pure\n{ n }\n",
    );
    assert_ne!(
        cert.get("level").and_then(Value::as_str),
        Some("L4"),
        "a div clause is out of the relax fragment — not certified L4"
    );
    assert_eq!(
        cert.get("reject")
            .and_then(|r| r.get("cause"))
            .and_then(Value::as_str),
        Some("NotRelaxable"),
        "the div clause is an honest NotRelaxable skip; got cert: {cert}"
    );
}
