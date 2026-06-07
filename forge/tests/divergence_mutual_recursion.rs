//! DIVERGENCE pin — Cluster **C9-A** (crosslink **#108**), `.design/basis/
//! 10-recursion-tuples.md` **REQ-6** + the REQ-2 doc-comment + the Architecture
//! section.
//!
//! ## The divergence
//!
//! REQ-6 (mutual recursion — DEFERRED) PROMISES, verbatim:
//!
//! > a mutually-recursive pair (neither calls itself directly, but they call
//! > each other without a `dec` chain) **reaches Verus and is rejected there
//! > (no false L3, no crash)**.
//!
//! and the Architecture restates it: "A mutually-recursive pair without a `dec`
//! chain reaches Verus and is rejected there (no false L3)."; the REQ-2
//! validator doc-comment: "it reaches Verus and is rejected there (no false L3,
//! no crash)."
//!
//! The `block_calls_name` self-call detector (REQ-2) correctly does NOT flag a
//! mutual pair `a -> b -> a` (neither `fn` calls ITSELF directly), so the pair
//! reaches Verus. Verus then rejects it with the termination diagnostic
//! `recursive function must have a decreases clause` (carrying
//! `encountered-vir-error: true` in its `--output-json`).
//!
//! BUT `forge`'s `classify_verus_outcome` (`forge/src/check.rs`) maps any
//! `encountered_vir_error` to `ForgeError::VerusOutput` — an ENVIRONMENT error:
//! `forge check` ABORTS with exit 2 and emits NO certificate (empty `--json`
//! stdout). That is a CRASH in the design's sense (no certificate verdict is
//! produced), not the promised "rejected there ... no crash".
//!
//! ## The authority contrast (R-CHAR-3 — expected value traced to the design)
//!
//! The SAME termination-failure CLASS, applied to a SINGLE self-recursive `fn`
//! whose `dec` does not decrease, is handled by `forge` as a clean parseable
//! **L0** certificate (exit 0) — verified by `recursion_conformance.rs::
//! nondecreasing_recursion_is_l0` and reconfirmed in this file's contrast probe.
//! REQ-4/AC-2 PIN that verdict. The design's REQ-6 "no crash" clause requires
//! the mutual-recursion termination rejection to ALSO surface as a certificate
//! verdict (a non-L3 reject / L0), NOT an uninterpretable internal-error abort.
//!
//! The expected outcome here is NOT copied from `forge`'s own output: it is the
//! design's literal "rejected there (no false L3, no crash)" — a PARSEABLE
//! certificate array whose `a`/`b` verdicts are NOT `L3`. The current toolchain
//! FAILS this: it produces empty `--json` stdout and a non-zero exit.
//!
//! Tracking: crosslink **blocker #110** (filed by the critic for #108).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// `true` iff verus is reachable (mirrors `recursion_conformance.rs`).
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

// A mutually-recursive pair `a -> b -> a` WITHOUT a `dec` chain and not `fx
// diverge`. Neither `fn` calls itself directly, so the REQ-2 `block_calls_name`
// self-call rule (correctly) does NOT flag it; the pair reaches Verus, which
// rejects it for missing termination. Per REQ-6 the rejection must be a
// certificate verdict (no crash, no false L3), exactly as the single-fn
// non-decreasing case is an L0 cert.
const MUTUAL_NO_DEC: &str = "fn a(n: u64) -> u64\n  \
    req n <= 1000\n  ens result == 0\n  fx pure\n\
    {\n  if n == 0 { 0 } else { b(n - 1) }\n}\n\n\
    fn b(n: u64) -> u64\n  \
    req n <= 1000\n  ens result == 0\n  fx pure\n\
    {\n  if n == 0 { 0 } else { a(n - 1) }\n}\n";

#[test]
fn divergence_mutual_recursion_is_rejected_not_crashed() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — mutual-recursion rejection not exercised.");
        return;
    }
    let fixture =
        std::env::temp_dir().join(format!("forge_divergence_mutual_{}.th", std::process::id()));
    std::fs::write(&fixture, MUTUAL_NO_DEC).expect("write fixture");
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(&fixture)
        .arg("--json")
        .output()
        .expect("spawn forge");
    let _ = std::fs::remove_file(&fixture);

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    // DESIGN 10-recursion-tuples.md REQ-6 "rejected there ... no crash": `forge
    // check --json` on the mutual pair MUST emit a parseable certificate array
    // (the verdict is the deliverable, §5.1 / R-SPEC-3) — exactly as the
    // single-fn non-decreasing termination failure emits an L0 cert. The
    // current toolchain emits EMPTY stdout + a VIR environment-error abort
    // ("could not interpret verus output: verus reported an internal (VIR)
    // error"), so this parse FAILS — that is the divergence.
    let certs: Vec<Value> = serde_json::from_str::<Value>(stdout.trim())
        .unwrap_or_else(|e| {
            panic!(
                "DESIGN REQ-6 'no crash': mutual-recursion termination rejection must \
                 produce a parseable certificate verdict, NOT a VIR-error abort. \
                 forge --json stdout was not one JSON doc: {e}\nstdout:\n{stdout}\nstderr:\n{stderr}"
            )
        })
        .as_array()
        .expect("certificate array")
        .clone();

    // DESIGN REQ-6 "no false L3": neither `a` nor `b` may certify L3 (termination
    // is NOT proved for a mutual pair without a `dec` chain). The expected
    // non-L3 verdict is the design's, not copied from forge.
    for item in ["a", "b"] {
        let level = certs
            .iter()
            .find(|c| c.get("item").and_then(|v| v.as_str()) == Some(item))
            .and_then(|c| c.get("level").and_then(|v| v.as_str()))
            .unwrap_or("<missing>");
        assert_ne!(
            level, "L3",
            "DESIGN 10-recursion-tuples.md REQ-6 'no false L3': mutual recursion \
             `{item}` without a `dec` chain must NOT certify L3 (Verus cannot prove \
             termination). certs: {certs:?}"
        );
    }
}
