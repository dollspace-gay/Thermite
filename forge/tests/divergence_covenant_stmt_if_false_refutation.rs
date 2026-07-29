//! Divergence test (acto-critic): the covenant engine's executable-semantics
//! evaluator (`forge/src/covenant_eval.rs`) manufactures a false refutation on a
//! statement-position `if`/`else` whose branches carry a tail expression.
//!
//! Authority: `.design/stage1-forge-tier.md` REQ-4 — the `falsify` run is "aimed at
//! the executable semantics", and a `CovenantRefuted` is "a `req`-satisfying input
//! whose body violates `ens`". AC-8 requires a `CovenantRefuted` to name a
//! counterexample. The `covenant_eval` module contract states that it never
//! silently evaluates a wrong value" (module docs, `forge/src/covenant_eval.rs`).
//!
//! Divergence: the surface grammar (`thermite-syntax/src/parser.rs`, the `TokKind::If`
//! arm of block parsing) emits `Stmt::If` for an `if`/`else` that is not in tail
//! position even when its branches HAVE tail expressions — e.g. the if/else followed
//! by a further tail. Rust/Verus executable semantics DISCARD a statement-position
//! `if`'s value. `eval_stmts` (`forge/src/covenant_eval.rs`, the `Stmt::If` arm)
//! instead reads the taken branch's `then.tail` / `else_.tail` and returns it as an
//! EARLY return of the enclosing block, so the block evaluates to the wrong value.
//!
//! Concrete divergence:
//!
//! ```text
//! fn alwayszero(x: u64) -> u64 req true ens result == 0 fx pure
//! { if x > 0 { x } else { x } 0 }
//! ```
//!
//! This function returns `0` for every input (the if/else value is discarded; the
//! tail `0` is the result), so `ens result == 0` holds universally and the covenant
//! must validate. The evaluator instead computes `result == x` and reports
//! `CovenantRefuted` whenever `x != 0`, blocking a correct item from the L3 burn.
//!
//! A control fixture with the same contract and body `{ 0 }` (no statement-if)
//! validates and burns to L3, isolating the divergence to the `Stmt::If` evaluation.
//!
//! Tracking: #298 (filed by the critic).
//!
//! `forge check` resolves the verus version before the covenant short-circuit, so
//! this skips (logged) when verus is absent, mirroring `covenant_conformance.rs`.
//! `tests/` is not anti-pattern-gated, so `unwrap`/`expect` are fine here.

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

fn write_temp(name: &str, program: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "forge_divcov_stmtif_{}_{name}.th",
        std::process::id()
    ));
    std::fs::write(&path, program).unwrap_or_else(|e| panic!("write temp {name}: {e}"));
    path
}

fn first_cert(program: &str, name: &str) -> Value {
    let file = write_temp(name, program);
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(&file)
        .arg("--json")
        .output()
        .unwrap_or_else(|e| panic!("spawn forge: {e}"));
    let _ = std::fs::remove_file(&file);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!(
            "forge --json stdout must be one JSON document: {e}\nstdout:\n{stdout}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stderr)
        )
    });
    value
        .as_array()
        .and_then(|a| a.first())
        .cloned()
        .unwrap_or_else(|| panic!("forge --json must emit at least one cert: {value}"))
}

/// A function that returns `0` for every input: the statement-position `if`/`else`
/// value is discarded by Rust/Verus executable semantics; the tail `0` is the
/// result. `ens result == 0` therefore holds universally and the covenant must
/// validate (it must not be `CovenantRefuted`). The `inhabit (0)` author witness
/// already satisfies `req` and `ens`, so any refutation comes from the `falsify`
/// generator drawing some `x != 0`.
const ALWAYS_ZERO_STMT_IF: &str = "\
fn alwayszero(x: u64) -> u64
    req true
    ens result == 0
    fx pure
{ if x > 0 { x } else { x } 0 }

witness { inhabit (0); falsify 1000; }
";

/// The control: the same contract and an equivalent body with no statement-if. It
/// validates and burns (verified against the live binary), isolating the divergence
/// to the `Stmt::If` evaluation.
const ALWAYS_ZERO_PLAIN: &str = "\
fn alwayszero(x: u64) -> u64
    req true
    ens result == 0
    fx pure
{ 0 }

witness { inhabit (0); falsify 1000; }
";

#[test]
fn stmt_position_if_value_must_be_discarded_not_a_false_refutation() {
    if !verus_present() {
        eprintln!(
            "SKIP: verus not available — `forge check` resolves the verus version before \
             the covenant short-circuit (set VERUS_BIN or install verus on PATH)."
        );
        return;
    }

    // Control: the plain body must not be CovenantRefuted (it returns 0 always).
    let control = first_cert(ALWAYS_ZERO_PLAIN, "plain");
    assert_ne!(
        control["reject"]["cause"], "CovenantRefuted",
        "control: a body `{{ 0 }}` under `ens result == 0` is correct and must not be \
         covenant-refuted, got {control}"
    );

    // Authority (REQ-4 / the covenant_eval faithfulness contract): the
    // statement-position `if`/`else` value is DISCARDED, so this body also returns 0
    // for every input and `ens result == 0` holds universally — the covenant must
    // validate, not refute. The evaluator's `Stmt::If` arm instead returns the taken
    // branch's tail as an early return, manufacturing a `CovenantRefuted` on `x != 0`.
    let cert = first_cert(ALWAYS_ZERO_STMT_IF, "stmtif");
    assert_ne!(
        cert["reject"]["cause"], "CovenantRefuted",
        "DIVERGENCE (REQ-4): a statement-position `if`/`else` value is discarded by the \
         executable semantics, so `alwayszero` returns 0 for every input and the covenant \
         must validate — the evaluator manufactured a FALSE refutation: {cert}"
    );
}
