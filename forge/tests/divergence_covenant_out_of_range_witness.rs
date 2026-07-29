//! Divergence test (acto-critic): the covenant engine accepts an `inhabit` witness
//! whose integer value is outside the declared parameter width, then uses that
//! out-of-domain value to manufacture a false `CovenantRefuted` on a sound item.
//!
//! Authority: `.design/stage1-forge-tier.md` REQ-4 — "`inhabit` witnesses are
//! type-checked and *executed* against `req`" and a `falsify` hit is "the hard-fail
//! verdict `CovenantRefuted` with the counterexample attached". A `CovenantRefuted`
//! is owed only for "a `req`-satisfying input the body violates `ens` on" (REQ-4 /
//! AC-8). An input is a value of the parameter's declared type; `4294967296` (= 2^32)
//! is not a `u32`, so it is not an input of `fn f(x: u32)` at all and cannot witness a
//! refutation.
//!
//! Divergence: `forge/src/covenant_engine.rs` `bind_params` checks only the value KIND
//! (`Int` vs `Bool`) against `ParamKind`, never the parameter's integer WIDTH. The AST
//! `Expr::IntLit { value: u128 }` (`thermite-syntax/src/ast.rs`) carries the full
//! literal, and `covenant_eval::eval_expr` evaluates it as the mathematical `i128`
//! `4294967296`. So an out-of-range author witness `inhabit (4294967296)` for a `u32`
//! parameter is accepted as a valid `req`-satisfying witness, then run through the body
//! → `ens`: the body `x as u32` truncates to `0` (the correct truncating-cast model),
//! `ens result == x` becomes `0 == 4294967296` → false → `CovenantRefuted`. The item is
//! sound for every actual `u32` input (`x as u32 == x` holds for all `x: u32`); only the
//! out-of-domain witness manufactures the refutation. The author witness must be
//! width-checked against the parameter type (a `WitnessTypeMismatch`/`ArityMismatch`-
//! class covenant error — an out-of-range literal is ill-typed, Verus rejects
//! `4294967296u32`), not silently widened and used to refute a correct item.
//!
//! Concrete divergence (`OOB_WITNESS`) vs control (`INRANGE_WITNESS`): the only textual
//! difference is the witness value `4294967296` (out of `u32` range) vs `5` (in range).
//! The in-range control validates; the out-of-range version is refuted.
//!
//! Tracking: #300 (filed by the critic).
//!
//! `forge check` resolves the verus version before the covenant short-circuit, so this
//! skips (logged) when verus is absent, mirroring the sibling covenant divergence tests.

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
        "forge_divcov_oobwitness_{}_{name}.th",
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

/// The divergence input: the witness `4294967296` (= 2^32) is outside the `u32` range of
/// the parameter `x`, so it is not an input of `f` at all. The item is sound for every
/// real `u32` (`x as u32 == x`), yet the out-of-domain witness manufactures a refutation.
const OOB_WITNESS: &str = "\
fn f(x: u32) -> u32
    req true
    ens result == x
    fx pure
{ x as u32 }

witness { inhabit (4294967296); falsify 100; }
";

/// The control: the same program with an IN-RANGE witness `5`. This validates
/// (no refutation). The only textual difference from `OOB_WITNESS` is the witness value,
/// isolating the divergence to the missing parameter-width check in `bind_params`.
const INRANGE_WITNESS: &str = "\
fn f(x: u32) -> u32
    req true
    ens result == x
    fx pure
{ x as u32 }

witness { inhabit (5); falsify 100; }
";

#[test]
fn out_of_range_witness_must_not_manufacture_a_false_covenant_refutation() {
    if !verus_present() {
        eprintln!(
            "SKIP: verus not available — `forge check` resolves the verus version before \
             the covenant short-circuit (set VERUS_BIN or install verus on PATH)."
        );
        return;
    }

    // Control: the in-range witness validates (no covenant reject). This proves
    // the item is sound and the refutation below is an artifact of the out-of-range
    // witness, not of the contract or the body.
    let control = first_cert(INRANGE_WITNESS, "inrange");
    assert!(
        control.get("reject").is_none_or(|r| r.is_null()),
        "control (in-range witness `5`): the item is sound for every u32, it must NOT be \
         rejected, got {control}"
    );

    // Authority (REQ-4 / AC-8): a `CovenantRefuted` is owed only for a `req`-satisfying
    // INPUT the body violates `ens` on. `4294967296` is not a `u32`, so it is not an input
    // of `f`; the author witness must be width-checked and refused as ill-typed, never
    // used to refute a sound item. The divergence: `bind_params` checks only Int-vs-Bool
    // (not the u32 width), accepts the out-of-range literal, and the truncating-cast body
    // (`x as u32` -> 0) makes `ens result == x` (0 == 2^32) false, manufacturing a
    // `CovenantRefuted` with the out-of-domain "counterexample" `(4294967296)`.
    let cert = first_cert(OOB_WITNESS, "oob");
    let reject = cert.get("reject").cloned().unwrap_or(Value::Null);
    let is_covenant_refuted = reject
        .get("cause")
        .and_then(|c| c.as_str())
        .is_some_and(|c| c == "CovenantRefuted");
    assert!(
        !is_covenant_refuted,
        "DIVERGENCE (REQ-4 / AC-8): the `inhabit (4294967296)` witness is OUTSIDE the `u32` \
         range of parameter `x` — it is not an input of `f`, and the item is SOUND for every \
         real u32 (`x as u32 == x`). The covenant must width-check the author witness and \
         refuse it as ill-typed, NOT widen it to i128 and use the truncating cast to \
         manufacture a `CovenantRefuted` on a correct item. forge instead reported {cert}"
    );
}
