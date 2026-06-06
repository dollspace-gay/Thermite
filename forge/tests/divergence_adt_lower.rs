//! acto-critic divergence tests for `forge check` on the Stage 1c ADT corpus
//! (commit `322d479`, crosslink #67).
//!
//! Each test pins a divergence between `forge check`'s emitted certificate and
//! the cert oracle (`conformance/{bank_account,shape,list_sum}.cert.json`,
//! hand-derived from `.design/basis/01-adts.md` REQ-8/9/10). Expected values
//! trace to the oracle / the design doc, NEVER to forge's own output
//! (`goal.md` R-CHAR-3).
//!
//! These run the BUILT `forge` binary end-to-end (verus-backed). If verus is
//! absent they SKIP LOUDLY (never panic on a missing solver), matching
//! `check_conformance.rs`.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is `<root>/forge`.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("forge has a parent dir")
        .to_path_buf()
}

/// `true` iff verus is reachable (mirrors `divergence_forge.rs`).
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

/// Run `forge check conformance/<name>.th --json`, returning the parsed certs.
fn check_corpus(name: &str) -> Vec<Value> {
    let path = repo_root().join("conformance").join(format!("{name}.th"));
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(&path)
        .arg("--json")
        .output()
        .expect("spawn forge");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!(
            "forge --json stdout must be one JSON doc: {e}\nstdout:\n{stdout}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stderr)
        )
    });
    value.as_array().expect("array of certs").clone()
}

fn cert_for<'a>(certs: &'a [Value], item: &str) -> &'a Value {
    certs
        .iter()
        .find(|c| c.get("item").and_then(|v| v.as_str()) == Some(item))
        .unwrap_or_else(|| panic!("no cert for `{item}` in {certs:?}"))
}

/// DIVERGENCE 1 — `forge check conformance/bank_account.th` certifies the exec
/// `fn deposit` at **L0**, but the cert oracle says **L3**.
///
/// Root cause: `forge`'s per-item path (`check::item_subprogram`) builds a
/// sub-program for `deposit` containing only the `fn` itself (+ `spec fn` deps
/// + reachable `fn` deps) — it does NOT weave the `struct Account` declaration
/// (nor its `well_formed` invariant) that `deposit`'s signature references. The
/// emitted Verus per-item lowering therefore fails to compile
/// (`error[E0425]: cannot find type Account in this scope`), so the item
/// degrades to L0. The builder's `adt_lower_conformance` test passed because it
/// lowers the WHOLE program once (struct + fn together); the real `forge check`
/// per-item path does not. This is the ADT-type-dependency analog of #52
/// (which weaves `fn`/`spec fn` deps into `item_subprogram` but not ADT decls).
///
/// Authority: `conformance/bank_account.cert.json` — `"item": "deposit"`,
/// `"level": "L3"`. `.design/basis/01-adts.md` REQ-8 (struct → Verus struct +
/// `well_formed` invariant referenced from `requires`/`ensures`) + AC-1
/// ("running the real `verus` binary on the emitted output exits 0 …; the
/// emitted certificate matches `bank_account.cert.json` (L3, non-vacuous)").
/// `goal.md` R-SPEC-2 (the certificate is a contract).
/// Tracking: #68
#[test]
fn divergence_deposit_certifies_l0_not_oracle_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — per-item ADT cert not exercised.");
        return;
    }
    let certs = check_corpus("bank_account");
    let deposit = cert_for(&certs, "deposit");
    // AUTHORITY: conformance/bank_account.cert.json -> "level": "L3".
    assert_eq!(
        deposit["level"],
        Value::from("L3"),
        "oracle conformance/bank_account.cert.json says deposit -> L3; \
         forge's per-item path drops the `struct Account` decl from \
         item_subprogram so the lowering fails to compile and degrades to L0; \
         got {deposit}"
    );
}

/// DIVERGENCE 2 — `forge check conformance/shape.th` certifies the exec
/// `fn is_circle` at **L0**, but the cert oracle says **L3**.
///
/// Same root cause: `item_subprogram` does not weave the `enum Shape`
/// declaration into `is_circle`'s sub-program, so the emitted Verus fails with
/// `cannot find type Shape` / `cannot find tuple variant Circle`, degrading the
/// item to L0.
///
/// Authority: `conformance/shape.cert.json` — `"item": "is_circle"`,
/// `"level": "L3"`. `.design/basis/01-adts.md` REQ-9 (enum → Verus enum;
/// `match` → Verus `match`; `is` → variant test) + AC-2/AC-4
/// (`verus` certifies L3). Tracking: #68
#[test]
fn divergence_is_circle_certifies_l0_not_oracle_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — per-item ADT cert not exercised.");
        return;
    }
    let certs = check_corpus("shape");
    let is_circle = cert_for(&certs, "is_circle");
    // AUTHORITY: conformance/shape.cert.json -> "level": "L3".
    assert_eq!(
        is_circle["level"],
        Value::from("L3"),
        "oracle conformance/shape.cert.json says is_circle -> L3; \
         forge's per-item path drops the `enum Shape` decl from item_subprogram \
         so the lowering fails to compile and degrades to L0; got {is_circle}"
    );
}

/// DIVERGENCE 3 — `forge check conformance/list_sum.th` certifies the recursive
/// `spec fn sum_list` at **L0**.
///
/// Same root cause as 1/2 applied to the `enum List` declaration: the per-item
/// sub-program for `sum_list` lacks the `enum List { Nil, Cons(u64, Box<List>) }`
/// decl, so the emitted Verus fails (`cannot find type List` / `cannot find
/// tuple variant Cons`). `.design/basis/01-adts.md` REQ-10 + AC-3 require the
/// recursive fold to certify L3 (`verus` `N verified, 0 errors`). A `spec fn`'s
/// well-formedness (its `decreases` measure terminating) is its discharged
/// result — it must NOT be L0.
///
/// Authority: `.design/basis/01-adts.md` AC-3 ("`verus` certifies L3
/// (`N verified, 0 errors`)") + REQ-10. Tracking: #68
#[test]
fn divergence_sum_list_certifies_l0_not_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — per-item recursive ADT cert not exercised.");
        return;
    }
    let certs = check_corpus("list_sum");
    let sum_list = cert_for(&certs, "sum_list");
    // AUTHORITY: .design/basis/01-adts.md AC-3 — the recursive fold certifies L3.
    assert_eq!(
        sum_list["level"],
        Value::from("L3"),
        ".design/basis/01-adts.md AC-3/REQ-10 require the recursive `sum_list` \
         fold to certify L3; forge's per-item path drops the `enum List` decl \
         from item_subprogram so the lowering fails to compile and degrades to \
         L0; got {sum_list}"
    );
}
