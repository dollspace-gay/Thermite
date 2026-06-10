//! End-to-end live pin for crosslink #225 — a recursive `spec fn` over a `u32`
//! param, named in an exec twin's contract, MUST certify L3 under real verus.
//!
//! THE BUG (root cause `thermite-lower/src/lower.rs`, the `plain_user_spec_call`
//! arm): the recursive call `s_dec(n - 1)` in the `spec fn` body, plus the
//! contract call `s_dec(n)`, hardcoded `as u64` on the arithmetic arg even though
//! `s_dec`'s declared param is `u32`. The emitted `s_dec((n - 1) as u64)` is
//! ill-typed Verus (`expected u32, found u64`), so the WHOLE item died at L0 with
//! an opaque obligation failure though the Thermite source is fine.
//!
//! THE AUTHORITY (R-CHAR-3): the expected level L3 is the design contract —
//! `thermite-design.md` §6 ladder semantics (L3 == a fully-discharged real-verus
//! proof) — NOT copied from the toolchain's own output. The narrowing cast is
//! legitimate (Verus spec arithmetic is the unbounded `int`); the fix only
//! redirects its TARGET to the callee's declared param type (`u32`). The negative
//! arm pins non-vacuity: a broken exec twin (returns the seed `0`) no longer
//! equals `s_dec(n)` and rejects below L3 (R-DEFER-9).
//!
//! The verus check SKIPS LOUDLY when verus is absent (the `editor_runs.rs`
//! precedent) — never panic on a missing solver (R-CODE-4). `tests/` is not
//! anti-pattern-gated, so `unwrap`/`expect`/`panic!` are fine here (R-APG-2).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// `true` iff verus is reachable (mirrors `spec_fn_string_param.rs`). SKIP LOUDLY
/// otherwise — a missing solver is never a test failure (R-CODE-4).
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
        "forge_spec_call_cast_{tag}_{}.th",
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

/// A recursive `spec fn` over a `u32` param (the #225 repro shape), its exec twin,
/// and a fn naming the spec fn in a contract. All three must certify L3.
const U32_PROGRAM: &str = "\
spec fn s_dec(n: u32) -> u32
  dec n
{
  if n == 0 {
    0
  } else {
    s_dec(n - 1)
  }
}

fn dec_exec(n: u32) -> u32
  req true
  ens result == s_dec(n)
  fx  pure
  dec n
{
  if n == 0 {
    0
  } else {
    dec_exec(n - 1)
  }
}

fn use_dec(n: u32) -> u32
  req true
  ens result == s_dec(n)
  fx  pure
{
  dec_exec(n)
}
";

#[test]
fn u32_recursive_spec_fn_certifies_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — #225 u32 recursive spec-fn cert not run.");
        return;
    }
    let certs = check_program("ok", U32_PROGRAM);
    // The recursive `spec fn s_dec(n: u32)` lowers with `s_dec((n - 1) as u32)`
    // (NOT the ill-typed `as u64`) and certifies L3 — the proof of the
    // param-type-directed cast.
    assert_eq!(
        level_of(&certs, "s_dec"),
        "L3",
        "a u32-param recursive spec fn must lower (type-directed cast) + certify L3:\n{certs:#?}"
    );
    // The exec twin proves `ens result == s_dec(n)` — the contract NAMES the spec
    // fn and discharges through the same param-type-directed cast.
    assert_eq!(
        level_of(&certs, "dec_exec"),
        "L3",
        "the exec twin pinned to `s_dec` must certify L3:\n{certs:#?}"
    );
    // A fn naming the spec fn in its contract (the contract-call `s_dec(n)` path)
    // certifies L3 — the #225 end-to-end payoff.
    assert_eq!(
        level_of(&certs, "use_dec"),
        "L3",
        "a fn naming the u32 spec fn in a contract must certify L3:\n{certs:#?}"
    );
}

#[test]
fn broken_dec_twin_rejects_below_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — #225 non-vacuity not run.");
        return;
    }
    // Non-vacuity (R-DEFER-9): a mutated exec twin that returns the seed `0`
    // unconditionally no longer equals `s_dec(n)`, so the `ens result == s_dec(n)`
    // equality FAILS and `dec_exec` rejects BELOW L3. The contract is real.
    let mutant = U32_PROGRAM.replace(
        "fn dec_exec(n: u32) -> u32\n  req true\n  ens result == s_dec(n)\n  fx  pure\n  dec n\n{\n  if n == 0 {\n    0\n  } else {\n    dec_exec(n - 1)\n  }\n}",
        "fn dec_exec(n: u32) -> u32\n  req true\n  ens result == s_dec(n)\n  fx  pure\n  dec n\n{\n  if n == 0 {\n    0\n  } else {\n    0\n  }\n}",
    );
    assert_ne!(
        mutant, U32_PROGRAM,
        "the mutation must actually change the exec twin's recursive arm (else vacuous)"
    );
    let certs = check_program("broken", &mutant);
    assert_ne!(
        level_of(&certs, "dec_exec"),
        "L3",
        "a broken exec twin (returns `0`) must NOT certify L3 — the \
         `ens result == s_dec(n)` is non-vacuous:\n{certs:#?}"
    );
}
