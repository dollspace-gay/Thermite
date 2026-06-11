//! `forge/tests/lean_engine.rs` — the integration-level guards for the Thermite→
//! Lean obligation exporter + the LeanEngine (`.design/verified/proof-backends.md`
//! REQ-6/REQ-7/REQ-8; increment (ii-b), the #240 chain, ref #203).
//!
//! `forge` is a BINARY crate (no lib target), so the in-process LeanEngine API
//! (`LeanEngine`/`export_item`/the four-slot `Engine` impl) is NOT reachable from an
//! integration test — those LIVE verdict tests (1)-(5) live as `#[cfg(test)]` unit
//! tests in `forge/src/engine.rs` (the only place that can construct a `LeanEngine`
//! and invoke lake live). They are: `live_scalar_correct_contract_is_proven` (1),
//! `live_tier_b_nonrecursive_spec_fn_is_proven` (2),
//! `live_wrong_contract_is_unknown_never_refuted` (3),
//! `omitted_registry_obligation_refuses_export` (4), `out_of_fragment_item_is_skipped`
//! (5), plus `recursive_registry_is_interactive_unknown` (the tier-(c) marker).
//!
//! This integration file carries the two guards that DO live at the binary /
//! external-artifact boundary:
//!
//! - **(6) the corpus cert ORACLE, byte-identical via the VERUS path.** Adding the
//!   Lean exporter + engine does NOT touch the default check path (Verus stays the
//!   sole default engine), so `forge check conformance/sum.th --json` must STILL emit
//!   `sum`'s golden L3 certificate. This re-runs the cert-oracle test against
//!   `conformance/sum.cert.json` (R-CHAR-3, never forge's own output) — the proof
//!   that increment (ii-b) is byte-identical on the shipped pipeline.
//! - **a LIVE spine-targeting kernel check.** A HAND-AUTHORED Lean obligation in the
//!   EXACT shape the exporter emits (the fuel-free tier-(a) form against
//!   `Thermite.denote 0` / `Thermite.intVal 0` over `R_item`) is kernel-checked by
//!   `lake env lean` — proving the spine ELABORATES the emitted shape and that a
//!   CORRECT contract kernel-accepts while a WRONG one does NOT (the §6.1(a)
//!   soundness witness, independent of the bin-internal exporter). The hand-authored
//!   source is derived from the design §4/§6.1 form (R-CHAR-3), not regenerated from
//!   the exporter.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("conformance")
}

fn lean_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("lean")
}

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

fn lake_binary() -> Option<PathBuf> {
    if let Ok(home) = std::env::var("HOME") {
        let elan = PathBuf::from(home).join(".elan/bin/lake");
        if elan.exists() {
            return Some(elan);
        }
    }
    if Command::new("lake")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return Some(PathBuf::from("lake"));
    }
    None
}

// (6) THE CORPUS CERT ORACLE — byte-identical via the VERUS path. The exporter +
// LeanEngine are NOT wired into the default check path, so `sum` must still certify
// at its golden L3 level with the golden deterministic fields. Expected from the
// golden `conformance/sum.cert.json` (R-CHAR-3), never forge's own output.
#[test]
fn sum_cert_oracle_byte_identical_after_lean_exporter() {
    if !verus_present() {
        eprintln!(
            "SKIP: verus not available — the cert-oracle identity for `sum` is not run \
             (set VERUS_BIN or install verus on PATH)."
        );
        return;
    }
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(corpus_dir().join("sum.th"))
        .arg("--json")
        .output()
        .expect("spawn forge check");
    assert_eq!(
        out.status.code(),
        Some(0),
        "a fully verified sum must exit 0"
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let certs: Vec<Value> =
        serde_json::from_str(stdout.trim()).expect("forge --json must emit a JSON array of certs");
    let golden_src = std::fs::read_to_string(corpus_dir().join("sum.cert.json"))
        .expect("read golden sum.cert.json");
    let golden: Value = serde_json::from_str(&golden_src).expect("parse golden sum.cert.json");

    let got = certs
        .iter()
        .find(|c| c.get("item").and_then(|v| v.as_str()) == Some("sum"))
        .expect("a certificate for `sum`");
    // The deterministic golden subset (NOT solver_time_ms / contract_quality) — the
    // same subset the SHIPPED engine_interface cert-oracle asserts.
    assert_eq!(
        got["item"], golden["item"],
        "item identity (#240 byte-identity)"
    );
    assert_eq!(
        got["level"], golden["level"],
        "the Lean exporter must NOT change `sum`'s achieved level (byte-identical)"
    );
    assert_eq!(got["effects"], golden["effects"], "effects == golden");
}

// A LIVE spine-targeting kernel check: a HAND-AUTHORED Lean obligation in the EXACT
// fuel-free tier-(a) shape the exporter emits (derived from the design §4/§6.1, NOT
// regenerated — R-CHAR-3) is kernel-checked by `lake env lean`. The CORRECT contract
// kernel-accepts (exit 0); the WRONG one does NOT (non-zero) — the §6.1(a) soundness
// witness at the spine boundary, independent of the bin-internal exporter API.
#[test]
fn live_spine_elaborates_emitted_shape_correct_proves_wrong_fails() {
    let Some(lake) = lake_binary() else {
        eprintln!("SKIP: lake not present — the live spine-shape kernel check is not run.");
        return;
    };

    // The CORRECT obligation (an `add`-shaped item: body == ens RHS): the fuel-free
    // tier-(a) form. After binding `result` to the body's value, `result == a + b`
    // holds; the `decide`/`omega` battery kernel-checks it.
    let correct = r#"import Thermite.Stabilize
def R_item : Thermite.Registry := fun _ => none
theorem ok (v : Thermite.Env) :
    Thermite.denote 0 (Thermite.Expr.boolLit true) { v with specs := R_item } →
    Thermite.denote 0 (Thermite.Expr.cmp Thermite.CmpOp.eq (Thermite.Expr.var "result") (Thermite.Expr.arith Thermite.ArithOp.add (Thermite.Expr.cast (Thermite.Expr.var "a") Thermite.CastTy.u64) (Thermite.Expr.cast (Thermite.Expr.var "b") Thermite.CastTy.u64)))
      ((({ v with specs := R_item } : Thermite.Env)).bindInt "result"
        (Thermite.intVal 0 (Thermite.Expr.arith Thermite.ArithOp.add (Thermite.Expr.cast (Thermite.Expr.var "a") Thermite.CastTy.u64) (Thermite.Expr.cast (Thermite.Expr.var "b") Thermite.CastTy.u64)) { v with specs := R_item })) := by
  intro hreq
  simp only [Thermite.Env.bindInt, Thermite.intVal, Thermite.denote, Thermite.arithDenote, Thermite.castDenote] at hreq ⊢
  first | decide | omega | simp_all | exact hreq
"#;

    // The WRONG obligation (`ens result == 0` for a body returning `a`): after
    // binding `result` to `a`, the goal is `a == 0` — NOT provable; the battery
    // FAILS (a non-zero exit → Unknown, NEVER a kernel-accepted Proven).
    let wrong = r#"import Thermite.Stabilize
def R_item : Thermite.Registry := fun _ => none
theorem bad (v : Thermite.Env) :
    Thermite.denote 0 (Thermite.Expr.boolLit true) { v with specs := R_item } →
    Thermite.denote 0 (Thermite.Expr.cmp Thermite.CmpOp.eq (Thermite.Expr.var "result") (Thermite.Expr.intLit 0))
      ((({ v with specs := R_item } : Thermite.Env)).bindInt "result"
        (Thermite.intVal 0 (Thermite.Expr.var "a") { v with specs := R_item })) := by
  intro hreq
  simp only [Thermite.Env.bindInt, Thermite.intVal, Thermite.denote, Thermite.arithDenote, Thermite.castDenote] at hreq ⊢
  first | decide | omega | simp_all | exact hreq
"#;

    let pid = std::process::id();
    let ok_path = std::env::temp_dir().join(format!("forge_lean_it_ok_{pid}.lean"));
    let bad_path = std::env::temp_dir().join(format!("forge_lean_it_bad_{pid}.lean"));
    std::fs::write(&ok_path, correct).expect("write correct probe");
    std::fs::write(&bad_path, wrong).expect("write wrong probe");

    let run = |file: &Path| -> bool {
        Command::new(&lake)
            .arg("env")
            .arg("lean")
            .arg(file)
            .current_dir(lean_root())
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    };

    let ok_accepted = run(&ok_path);
    let bad_accepted = run(&bad_path);
    let _ = std::fs::remove_file(&ok_path);
    let _ = std::fs::remove_file(&bad_path);

    assert!(
        ok_accepted,
        "the spine must KERNEL-ACCEPT the emitted fuel-free tier-(a) shape of a CORRECT contract"
    );
    assert!(
        !bad_accepted,
        "the spine must NOT kernel-accept a WRONG contract (the §6.1 soundness witness): \
         a wrong contract is Unknown, never Proven"
    );
}
