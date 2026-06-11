//! Divergence pin (crosslink #232, the layer UNDER #230) — a `struct` whose
//! `inv` names a user `spec fn` dies at L0 because `forge::check`'s
//! `item_subprogram` builds the struct's per-item sub-program as the item
//! ALONE: `Item::Struct(_) | Item::Enum(_) => Program { items: vec![item] }`,
//! with a STALE premise comment ("Dead-in-1a: dies at the validator gate ...
//! this arm never produces a cert" — live `forge check` DOES produce a cert,
//! at L0). Two consequences, both live-confirmed:
//!
//!   1. the spec-fn definition is absent from the emitted Verus unit, so the
//!      `well_formed` body's call is unresolvable — E0425 `cannot find
//!      function s_dec in this scope`;
//!   2. the per-item program has no spec fn, so `spec_fn_param_type_map` is
//!      EMPTY and the c116360c REQ-5 cast falls back to `as u64` (the emitted
//!      `s_dec((self.x + 0) as u64)`), though the whole-program lowering — the
//!      thermite-lower #229 pin — correctly emits `as u32`.
//!
//! Fixing #230 alone (emit `pub open spec fn` for user spec fns) does NOT
//! revive this shape: the def is still not WOVEN into the struct's
//! sub-program. The `Item::SpecFn`/`Item::Fn` arms already weave reachable
//! spec-fn deps (the #68/#71 precedent); the Struct arm must too.
//!
//! THE AUTHORITY (R-CHAR-3): expected level L3 is the design contract —
//! `thermite-design.md` §6 (L3 == fully-discharged real-verus proof) +
//! `.design/lower/verus-lowering.md` REQ-8 (struct type-invariant → enforced
//! `well_formed` predicate, the verified `bank_account` precedent). The
//! fully-woven, `pub open` form of this exact fixture verifies by hand:
//! `verus` on { pub open spec fn s_dec + pub struct Counter + well_formed
//! calling `s_dec((self.x + 0) as u32)` } reports `1 verified, 0 errors`.
//! Expected value never copied from the toolchain's output.
//!
//! Verus check SKIPS LOUDLY when verus is absent (`editor_runs.rs` precedent).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// `true` iff verus is reachable (mirrors `divergence_spec_call_param_cast.rs`).
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

fn check_program(tag: &str, program: &str) -> Vec<Value> {
    let fixture = std::env::temp_dir().join(format!(
        "forge_struct_inv_subprog_{tag}_{}.th",
        std::process::id()
    ));
    std::fs::write(&fixture, program).expect("write fixture");
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(&fixture)
        .arg("--json")
        .output()
        .expect("spawn forge check");
    let _ = std::fs::remove_file(&fixture);
    let stdout = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str::<Value>(stdout.trim())
        .unwrap_or_else(|e| {
            panic!(
                "[{tag}] forge check --json stdout must be one JSON doc: {e}\nstdout:\n{stdout}\nstderr:\n{}",
                String::from_utf8_lossy(&out.stderr)
            )
        })
        .as_array()
        .unwrap_or_else(|| panic!("[{tag}] forge check --json must emit an array of certs"))
        .clone()
}

fn level_of(certs: &[Value], item: &str) -> String {
    certs
        .iter()
        .find(|c| c.get("item").and_then(|v| v.as_str()) == Some(item))
        .unwrap_or_else(|| panic!("no cert for `{item}` in {certs:#?}"))["level"]
        .as_str()
        .unwrap_or_else(|| panic!("cert for `{item}` has no string level"))
        .to_string()
}

/// The #229 struct-inv fixture: a `u32`-param user spec fn named (with an
/// arithmetic arg over a field) in a struct invariant. The whole-program
/// lowering is correct since c116360c; only the per-item slicing (and the
/// #230 visibility tier) keeps it from certifying.
const COUNTER_PROGRAM: &str = "\
spec fn s_dec(n: u32) -> u32
  dec n
{
  if n == 0 {
    0
  } else {
    s_dec(n - 1)
  }
}

struct Counter {
  x: u32,
} inv s_dec(x + 0) == 0
";

#[test]
#[ignore = "divergence: item_subprogram's Struct arm omits reachable spec-fn deps (E0425 + empty param-type map -> as u64) so a struct inv naming a user spec fn dies at L0; needs #230's pub-open tier too; tracking #232 (+#230)"]
fn struct_inv_naming_user_spec_fn_certifies_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — struct-inv spec-fn sub-program cert not run.");
        return;
    }
    let certs = check_program("counter", COUNTER_PROGRAM);
    assert_eq!(
        level_of(&certs, "s_dec"),
        "L3",
        "the spec fn itself must certify L3"
    );
    // thermite-design.md §6: a correct source certifies L3. The struct's
    // well_formed predicate is the verified bank_account REQ-8 shape; the
    // hand-woven `pub open` form of this exact fixture is `1 verified, 0
    // errors` under verus.
    assert_eq!(
        level_of(&certs, "Counter"),
        "L3",
        "a struct whose inv names a user spec fn must certify (sub-program \
         must weave the reachable spec-fn deps, as the SpecFn/Fn arms do)"
    );
}
