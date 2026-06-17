//! Divergence test (acto-critic): the covenant engine's executable-semantics
//! evaluator (`forge/src/covenant_eval.rs`) SILENTLY accepts an assignment to a
//! NON-`mut` `let` binding and computes a value, validating a covenant on a body
//! that is ill-formed in Rust/Verus.
//!
//! Authority: `.design/stage1-forge-tier.md` REQ-4 — the `falsify` run is "aimed at
//! the executable semantics". `covenant_eval.rs` module docs (the faithfulness
//! contract): the evaluator admits the pure scalar fragment, and "anything outside
//! it … is an honest `CovenantEvalError::Unsupported` … it never silently evaluates
//! a wrong value, mirroring `thermite_tv::exec_encode`'s `RefEncodeError::Unsupported`
//! (R-CODE-2 / R-APG-1)." `thermite-syntax/src/ast.rs` `Stmt::Let { mutable: bool, .. }`
//! records whether a binding is `mut`; the parser populates it.
//!
//! Divergence: `covenant_eval.rs` `eval_stmts` destructures `Stmt::Let { name, init, .. }`
//! and DISCARDS `mutable`, and its `Stmt::Assign` arm re-`insert`s the target name into
//! the env unconditionally. So an assignment `r = 1` to a binding introduced by an
//! IMMUTABLE `let r = 0;` is accepted as a valid mutation and the body evaluates to a
//! concrete value — while Rust/Verus reject the body outright
//! (`error[E0384]: cannot assign twice to immutable variable`). The evaluator therefore
//! validates a covenant (reports a clean `falsify_generated > 0`, `falsify_refuted == 0`
//! run) on a body that has NO well-defined value: a silent wrong value, the precise
//! thing the module contract and REQ-4 forbid. An assignment to a non-`mut` binding is
//! outside the executable fragment and must surface as a loud covenant error.
//!
//! Concrete divergence (`ASSIGN_IMMUTABLE`):
//!
//! ```text
//! fn setone(x: u64) -> u64 req true ens result == 1 fx pure
//! { let r = 0; if x > 0 { r = 1; } else { r = 1; } r }
//! ```
//!
//! `r` is declared by an IMMUTABLE `let r = 0;`, so `r = 1;` is a compile error in
//! Verus — the body never builds. The covenant evaluator instead threads the (illegal)
//! mutation and reports a clean validated `falsify` run, identical to the control with
//! a legal `let mut r = 0;` (`ASSIGN_MUT_CONTROL`). The control isolates the divergence
//! to the dropped `mutable` flag: the only difference between the two programs is the
//! `mut` keyword, yet the evaluator produces identical covenant evidence.
//!
//! Tracking: #299 (filed by the critic).
//!
//! `forge check` resolves the verus version before the covenant short-circuit, so this
//! skips (logged) when verus is absent, mirroring `divergence_covenant_stmt_if_false_refutation.rs`.

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
        "forge_divcov_assignimm_{}_{name}.th",
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

/// The DIVERGENCE input: `r` is introduced by an IMMUTABLE `let r = 0;`, then assigned
/// `r = 1;` in both `if` branches. Verus rejects this body (`cannot assign twice to
/// immutable variable`), so it has no executable value and the covenant must NOT report
/// a clean validated `falsify` run — the evaluator must surface the assignment to a
/// non-`mut` binding as a loud covenant error (outside the executable fragment).
const ASSIGN_IMMUTABLE: &str = "\
fn setone(x: u64) -> u64
    req true
    ens result == 1
    fx pure
{ let r = 0; if x > 0 { r = 1; } else { r = 1; } r }

witness { inhabit (0); falsify 1000; }
";

/// The control: the SAME program with a LEGAL `let mut r = 0;`. This body is well-formed
/// and returns 1 for every input, so the covenant validates with a clean run. The ONLY
/// textual difference from `ASSIGN_IMMUTABLE` is the `mut` keyword — isolating the
/// divergence to the dropped `Stmt::Let.mutable` flag in `covenant_eval`.
const ASSIGN_MUT_CONTROL: &str = "\
fn setone(x: u64) -> u64
    req true
    ens result == 1
    fx pure
{ let mut r = 0; if x > 0 { r = 1; } else { r = 1; } r }

witness { inhabit (0); falsify 1000; }
";

#[test]
fn assignment_to_immutable_let_is_not_a_silent_clean_covenant() {
    if !verus_present() {
        eprintln!(
            "SKIP: verus not available — `forge check` resolves the verus version before \
             the covenant short-circuit (set VERUS_BIN or install verus on PATH)."
        );
        return;
    }

    // Control: the legal `let mut` body validates cleanly (no covenant refutation, a
    // clean falsify run). This establishes that the covenant evidence shape below is the
    // "validated" shape, not an artifact of some unrelated gate.
    let control = first_cert(ASSIGN_MUT_CONTROL, "mut");
    let cev = &control["covenant_evidence"];
    assert_eq!(
        cev["falsify_refuted"], 0,
        "control (`let mut`): a well-formed body returning 1 must validate the covenant \
         cleanly, got {control}"
    );
    assert!(
        cev["falsify_generated"].as_u64().unwrap_or(0) > 0,
        "control (`let mut`): the falsify run must have generated inputs, got {control}"
    );

    // Authority (REQ-4 / the covenant_eval faithfulness contract): `r` is declared by an
    // IMMUTABLE `let r = 0;`, so `r = 1;` is ill-formed in Rust/Verus and the body has no
    // executable value. The evaluator must surface the assignment to a non-`mut` binding
    // as a loud covenant error — it must NOT silently thread the illegal mutation and
    // report a clean validated falsify run. The divergence: it drops `Stmt::Let.mutable`,
    // accepts the assignment, and produces covenant_evidence IDENTICAL to the `let mut`
    // control (a clean `falsify_refuted == 0` run over `falsify_generated > 0` inputs) on
    // a body that does not even compile.
    let cert = first_cert(ASSIGN_IMMUTABLE, "immutable");
    let ev = &cert["covenant_evidence"];
    let clean_validated = ev["falsify_refuted"] == serde_json::json!(0)
        && ev["falsify_generated"].as_u64().unwrap_or(0) > 0;
    assert!(
        !clean_validated,
        "DIVERGENCE (REQ-4 / covenant_eval faithfulness): `let r = 0; ... r = 1;` assigns \
         to an IMMUTABLE binding — Verus rejects the body (`cannot assign twice to \
         immutable variable`) so it has no executable value. The covenant evaluator must \
         surface this as a loud covenant error, NOT silently evaluate the body and report \
         a clean validated falsify run. It instead produced covenant_evidence {ev} — \
         identical to the `let mut` control — silently evaluating a wrong value: {cert}"
    );
}
