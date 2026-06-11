//! Pinning regression for crosslink #238 (critic re-audit of the #237 fix,
//! commit 39cae0bc): the #237 result-narrowing gate
//! (`thermite-lower/src/lower.rs` `block_result_is_int_literal_arith`) requires
//! the result-position arithmetic to MENTION AN INTEGER LITERAL
//! (`expr_mentions_int_literal`). That premise is wrong: Verus types ALL spec
//! arithmetic as the unbounded `int` — `n + n` over `u64` params is `int`-typed
//! exactly as `1 + count(n - 1)` is — so a literal-free arithmetic result on a
//! sized-int return is still E0308 (`expected u64, found int`) → L0 on
//! legitimate frozen-subset source. Fail-CLOSED (no false certification), the
//! SAME completeness-gap class #237 itself was filed as.
//!
//! Live repro (verus 2026, this re-audit): `spec fn double(n: u64) -> u64 dec n
//! { n + n }` → forge check L0, diagnostic `error[E0308]: ... 7 | n + n ^^^^^
//! expected `u64`, found `int``.
//!
//! THE AUTHORITY (R-CHAR-3): the expected level L3 is the design contract —
//! `thermite-design.md` §6 ladder semantics (L3 == a fully-discharged real-verus
//! proof, reachable for legitimate frozen-subset source) and
//! `.design/lower/verus-lowering.md` REQ-5 (spec-context lowering must emit
//! Verus that typechecks) — NOT copied from the toolchain's own output.
//!
//! `#[ignore]`d per the #233-chain critic convention: blocker #238 tracks the
//! fix; un-ignore when the narrowing gate covers literal-free spec arithmetic.
//!
//! The verus check SKIPS LOUDLY when verus is absent (the `editor_runs.rs`
//! precedent) — never panic on a missing solver (R-CODE-4). `tests/` is not
//! anti-pattern-gated (R-APG-2).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// `true` iff verus is reachable (mirrors `divergence_intlit_dec_spec_fn.rs`).
/// SKIP LOUDLY otherwise — a missing solver is never a test failure (R-CODE-4).
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
    let fixture = std::env::temp_dir().join(format!("forge_238_{tag}_{}.th", std::process::id()));
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

/// Literal-free result-position arithmetic over a sized-int return — Verus
/// spec arithmetic is `int`-typed with or without a literal operand, so this
/// needs the SAME result-narrowing as #237's `1 + count(n - 1)`. The #238 gap:
/// `expr_mentions_int_literal` finds no literal, the gate stays cold, the bare
/// `int`-typed body dies E0308 → L0.
const DOUBLE_PROGRAM: &str = "\
spec fn double(n: u64) -> u64
  dec n
{
  n + n
}
";

/// The two-param literal-free variant (the `f(a) + f(b)` operand class without
/// the literal-carrying decrement that masks the gap in recursive fixtures).
const ADD_PROGRAM: &str = "\
spec fn add(a: u64, b: u64) -> u64
  dec a
{
  a + b
}
";

#[test]
#[ignore = "divergence: #237 narrowing gate misses literal-free spec arithmetic (n + n -> E0308 -> L0); tracking #238"]
fn literal_free_arith_spec_fn_certifies_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — #238 literal-free spec-arith cert not run.");
        return;
    }
    let certs = check_program("double", DOUBLE_PROGRAM);
    assert_eq!(
        level_of(&certs, "double"),
        "L3",
        "a u64-return spec fn whose result is literal-free arithmetic (`n + n`, \
         `int`-typed in Verus spec) must lower to typechecking Verus + certify L3:\n{certs:#?}"
    );
}

#[test]
#[ignore = "divergence: #237 narrowing gate misses literal-free spec arithmetic (a + b -> E0308 -> L0); tracking #238"]
fn literal_free_two_param_arith_spec_fn_certifies_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — #238 literal-free spec-arith cert not run.");
        return;
    }
    let certs = check_program("add", ADD_PROGRAM);
    assert_eq!(
        level_of(&certs, "add"),
        "L3",
        "a u64-return spec fn whose result is literal-free arithmetic (`a + b`) \
         must lower to typechecking Verus + certify L3:\n{certs:#?}"
    );
}
