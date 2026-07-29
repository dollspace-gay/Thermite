//! Divergence test (acto-critic, increment 2b final sweep): the covenant evaluator
//! treats the prefix `!` on an INTEGER operand as a type error, refusing an item that
//! is in-language and L3-certifiable.
//!
//! Authority:
//!   - `.design/syntax/ast.md` REQ-10 / OQ-4: there is one `!` token / one
//!     `UnaryOp::Not` whose meaning is per the operand type — "**bitwise-not on an
//!     integer type, logical-not on `bool`** ... resolved by Verus's type-directed
//!     `!`". OQ-4: "a fn using `!` on an integer and a fn using `!` on `bool` both
//!     certify under Verus's type-directed `!`." So `fn f(x: u32) -> u32 ... { !x }`
//!     is a sound, L3-certifiable v1 item.
//!   - `thermite_syntax::ast::UnaryOp::Not` doc: "its meaning is per the operand type
//!     (logical-not on `bool`, bitwise-not on an integer)".
//!   - `forge/src/covenant_eval.rs` module docs (the evaluator's own stated fragment):
//!     "The evaluator admits the pure scalar fragment: integer (`u32`/`u64`/`usize`)
//!     and `bool` values; the arithmetic/comparison/logical/**bitwise** operators;
//!     `!`; ...". Integer `!` is therefore claimed to be inside the fragment, not an
//!     out-of-fragment `Unsupported`.
//!
//! Divergence: `covenant_eval::eval_expr`'s `Expr::Unary { op: UnaryOp::Not, .. }` arm
//! evaluates only `Value::Bool(!v.as_bool()?)` — an integer operand hits `as_bool()` and
//! returns `CovenantEvalError::Type("expected a bool, found integer N")`. In
//! `covenant_engine::analyze_covenant` that non-Trap eval error becomes a
//! `CovenantError::UnsupportedItem`, so the covenant gate refuses the item before burn
//! (`CovenantUnsupportedItem`, L0). The same item without a witness block certifies L3.
//! A sound, in-language item is downgraded from L3 to an L0 refusal purely because it
//! carries a covenant and uses the documented integer `!` — a fragment-completeness
//! divergence (REQ-4), not an out-of-fragment Unsupported.
//!
//! Control: the same `fn` with no witness block certifies L3 (verified against the live
//! binary), isolating the divergence to the covenant evaluator's unary-not arm.
//!
//! Tracking: filed by the critic (see report). `forge check` resolves the verus version
//! before the covenant short-circuit, so this skips (logged) when verus is absent,
//! mirroring `divergence_covenant_slag_ordering.rs`. `tests/` is not anti-pattern-gated.

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
        "forge_divcov_bnot_{}_{name}.th",
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

/// `fn flipbits(x: u32) -> u32 ... { !x }` with `ens result == !x` — a sound, in-language
/// item. `!x` on a `u32` is bitwise-not (ast.md REQ-10/OQ-4), Verus-native. The body
/// agrees with `ens` for every `x`, so the covenant should validate and the item should
/// certify L3 with covenant evidence. The only textual difference from the control is the
/// trailing `witness` block.
const FLIP_WITH_WITNESS: &str = "\
fn flipbits(x: u32) -> u32
    req true
    ens result == !x
    fx pure
{ !x }

witness { inhabit (5); falsify 20; }
";

/// The control: the same `fn` with no witness block. It certifies L3 (the integer `!`
/// is Verus-native; ast.md OQ-4 — it certifies under Verus's type-directed `!`).
const FLIP_NO_WITNESS: &str = "\
fn flipbits(x: u32) -> u32
    req true
    ens result == !x
    fx pure
{ !x }
";

#[test]
fn covenant_admits_integer_bitwise_not_in_its_fragment() {
    if !verus_present() {
        eprintln!(
            "SKIP: verus not available — `forge check` resolves the verus version before \
             the covenant short-circuit (set VERUS_BIN or install verus on PATH)."
        );
        return;
    }

    // Control (ast.md OQ-4): the same item without a witness certifies L3 — integer `!`
    // is bitwise-not, Verus-native, sound.
    let control = first_cert(FLIP_NO_WITNESS, "nowitness");
    assert_eq!(
        control["level"],
        Value::from("L3"),
        "control: an integer-`!` item is Verus-native and certifies L3 (ast.md OQ-4): \
         {control}"
    );

    // Authority (ast.md REQ-10/OQ-4 + covenant_eval.rs's own stated fragment): integer
    // `!` is inside the covenant scalar fragment. The covenanted item must therefore be
    // covenant-CHECKED (validated, since the body == `ens` for all x) and certify L3 with
    // covenant evidence — not refused as `CovenantUnsupportedItem`.
    let cert = first_cert(FLIP_WITH_WITNESS, "withwitness");
    assert_eq!(
        cert["level"],
        Value::from("L3"),
        "DIVERGENCE (REQ-4 / ast.md OQ-4): a covenanted item using the documented integer \
         `!` (bitwise-not) must be covenant-checked and certify L3; the covenant \
         evaluator's `UnaryOp::Not` arm rejects an integer operand as a type mismatch, \
         refusing the item as CovenantUnsupportedItem (L0): {cert}"
    );
    assert!(
        cert.get("covenant_evidence").is_some_and(|e| !e.is_null()),
        "DIVERGENCE: a validated covenant on an integer-`!` item must record covenant \
         evidence; got none (the item was refused before burn): {cert}"
    );
}
