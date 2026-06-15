//! End-to-end live pins for crosslink #237 — two related completeness gaps, both
//! fail-closed today (no false certification), on legitimate frozen-subset source:
//!
//! (a) The int-literal return-typing gap (`thermite-lower/src/lower.rs`):
//!     `spec fn count(n: u64) -> u64 dec n { if n == 0 { 0 } else { 1 + count(n - 1) } }`
//!     lowered the else-arm as `1 + count((n - 1) as u64)` — `int`-typed in Verus
//!     spec against the `u64` return → E0308 → L0. Fix: narrow the body result back
//!     to the declared return type — `(1 + count((n - 1) as u64)) as u64` — same
//!     fidelity class as the #225 casts (identity on the spec domain for in-range
//!     values). This fixture now certifies L3.
//!
//! (b) The dec-position weaving gap (`forge/src/check.rs`): a `spec fn` whose `dec`
//!     calls another spec fn died E0425 — the §5.3 sub-program weaving used the
//!     body-only closure (`reachable_spec_fn_deps`), dropping the dec-position dep.
//!     Fix: the weaver now delegates to the shared `body ∪ dec` closure (the #204
//!     `reachable_spec_fn_names_from_seed`, the #192 "one closure" lesson). This
//!     fixture now certifies L3.
//!
//! The authority (R-CHAR-3): the expected level L3 is the design contract —
//! `thermite-design.md` §6 ladder semantics (L3 == a fully-discharged real-verus
//! proof) — not copied from the toolchain's own output.
//!
//! The verus check skips with a logged note when verus is absent (the
//! `editor_runs.rs` precedent); no panic on a missing solver (R-CODE-4). `tests/`
//! is not anti-pattern-gated (R-APG-2).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// `true` iff verus is reachable (mirrors `divergence_spec_call_param_cast.rs`).
/// Skip with a logged note otherwise; a missing solver is not a test failure (R-CODE-4).
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
    let fixture = std::env::temp_dir().join(format!("forge_237_{tag}_{}.th", std::process::id()));
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

/// (a) The int-literal return-typing repro — a recursive `u64` spec fn whose else
/// arm is `1 + count(n - 1)`. Certifies L3 (was E0308/L0).
const COUNT_PROGRAM: &str = "\
spec fn count(n: u64) -> u64
  dec n
{
  if n == 0 {
    0
  } else {
    1 + count(n - 1)
  }
}
";

/// (b) The dec-position weaving repro — a `spec fn` whose `dec` calls another spec
/// fn (`dec measure(n)`). The dec-position dep `measure` must be woven into the
/// §5.3 sub-program (was dropped → E0425/L0). `measure` is the identity (`-> nat`,
/// non-recursive, transparent `pub open`), so the descent `measure(n - 1) <
/// measure(n)` is provable and `walk` certifies L3. The gap was the dropped dep
/// (compile-time E0425), not a termination failure.
const DEC_DEP_PROGRAM: &str = "\
spec fn measure(n: u64) -> nat
  dec n
{
  n as nat
}

spec fn walk(n: u64) -> u64
  dec measure(n)
{
  if n == 0 {
    0
  } else {
    walk(n - 1)
  }
}
";

#[test]
fn int_literal_return_typed_spec_fn_certifies_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — #237 (a) int-literal return-typing cert not run.");
        return;
    }
    let certs = check_program("count", COUNT_PROGRAM);
    // The recursive `spec fn count(n: u64)` lowers with the result narrowed
    // `(1 + count((n - 1) as u64)) as u64` (not a bare `int`-typed body) and
    // certifies L3 — the proof of the result-narrowing cast.
    assert_eq!(
        level_of(&certs, "count"),
        "L3",
        "a u64-return int-literal-arith spec fn must lower (result-narrowing cast) + certify L3:\n{certs:#?}"
    );
}

#[test]
fn dec_position_spec_fn_dep_certifies_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — #237 (b) dec-position weaving cert not run.");
        return;
    }
    let certs = check_program("dec_dep", DEC_DEP_PROGRAM);
    // `walk`'s `dec measure(n)` names the spec fn `measure`; the §5.3 sub-program
    // weaving must include `measure` (the dec-position dep) so the lowered Verus
    // resolves it (was dropped → E0425/L0). Both certify L3.
    assert_eq!(
        level_of(&certs, "walk"),
        "L3",
        "a spec fn whose `dec` calls another spec fn must weave the dec-dep + certify L3:\n{certs:#?}"
    );
    assert_eq!(
        level_of(&certs, "measure"),
        "L3",
        "the dec-position dep `measure` must itself certify L3:\n{certs:#?}"
    );
}
