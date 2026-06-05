//! Divergence tests pinning two Goodhart holes in forge's §7 step-4 mutation
//! scoring (issue #12, commit fa55760), authored by the ACToR critic.
//!
//! Authority chain (R-CHAR-3 — expected outcomes trace to the design, never to
//! forge's own output):
//!   - `thermite-design.md` §7 step 4: "Forge generates N mutants of the body ...
//!     and re-verifies each against the contract. The kill ratio ... A
//!     configurable floor (default 60%) gates certification; below it, Forge
//!     reports exactly which mutants survived." The floor's PURPOSE is to catch a
//!     contract that under-constrains the body.
//!   - `goal.md` R-DEFER-9 (anti-Goodhart): "the design's §7 battery exists
//!     precisely to catch this" — a contract that does not constrain its result
//!     must not certify clean. A path that lets a weak contract certify L3
//!     UNSCORED is a bypass = a hole.
//!   - `.design/forge/mutation-scoring.md` REQ-5 (the floor gate) + REQ-7 (the
//!     gate runs after L3, content-addressed through the proof cache).
//!
//! Both tests drive the BUILT `forge` binary (mirroring
//! `mutation_conformance.rs`); both NEED verus and SKIP LOUDLY when it is absent,
//! never panic. `unwrap`/`expect` are fine in `tests/` (not anti-pattern-gated).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// `true` iff verus is reachable (mirrors `mutation_conformance.rs`).
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

fn unique() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    N.fetch_add(1, Ordering::Relaxed)
}

fn write_temp(name: &str, program: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "forge_divmut_{}_{}_{name}.th",
        std::process::id(),
        unique()
    ));
    std::fs::write(&path, program).unwrap();
    path
}

fn unique_cache_dir() -> PathBuf {
    std::env::temp_dir().join(format!(
        "forge_divmut_cache_{}_{}",
        std::process::id(),
        unique()
    ))
}

/// Run `forge check <file> --json` against a SPECIFIC cache dir (so the
/// cache-bypass test can plant a stale entry between runs), returning the certs.
fn run_check_in(file: &Path, cache_dir: &Path) -> Vec<Value> {
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(file)
        .arg("--json")
        .env("FORGE_CACHE_DIR", cache_dir)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!(
            "forge --json must emit one JSON document: {e}\nstdout:\n{stdout}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stderr)
        )
    });
    value.as_array().unwrap().clone()
}

fn cert_for<'a>(certs: &'a [Value], item: &str) -> &'a Value {
    certs
        .iter()
        .find(|c| c.get("item").and_then(|i| i.as_str()) == Some(item))
        .unwrap_or_else(|| panic!("no `{item}` cert in {certs:?}"))
}

fn is_clean_l3(cert: &Value) -> bool {
    cert.get("level").and_then(|l| l.as_str()) == Some("L3")
        && cert.get("reject").map(|r| r.is_null()).unwrap_or(true)
}

// ----------------------------------------------------------------------------
// DIVERGENCE 1 — the 0/0 escape: a WEAK contract certifies L3 UNSCORED.
//
// Authority: `thermite-design.md` §7 step 4 (the floor gates a contract that
// under-constrains its body) + `goal.md` R-DEFER-9 (a path that lets a weak
// contract certify is a Goodhart hole).
//
// Program: `pick(xs: &[u32]) -> &[u32]  req xs.len() <= 10  ens result.len() <= 10
//           fx pure { xs }`. The `ens` does NOT pin WHICH slice `result` is — a
// body returning ANY slice with `len <= 10` (e.g. an empty slice) satisfies it,
// so the contract under-constrains the result exactly the way §7's floor is
// meant to catch. Yet `mutation::generate` emits ZERO mutants for it: the early-
// return mutator is SKIPPED because `&[u32]` has no canonical zero
// (`mutation::zero_value_for` -> None), and the body `{ xs }` has no
// Binary/IntLit/If site. With `scored == 0`, `MutationScore::kill_ratio` returns
// 1.0 (`mutation.rs`), the floor is vacuously met, and the item certifies L3
// with `mutants_killed: "0/0"`.
//
// Expected (authority): a contract that fails to constrain its result must NOT
// certify clean L3 unscored — the floor must catch it (§7 / R-DEFER-9). The
// early-return mutant should be generatable for ANY body so 0/0 is unreachable
// for a real fn.
//
// Actual (fa55760): clean L3 certify, `mutants_killed: "0/0"`. UNGATED.
//
// This test asserts the AUTHORITY's expectation (NOT a clean unscored L3) and
// therefore FAILS against the current toolchain — pinning the divergence.
// Tracking: filed as a `-l blocker` crosslink issue (see report).
// ----------------------------------------------------------------------------
#[test]
fn divergence_weak_contract_escapes_floor_via_zero_scored_mutants() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — mutation scoring needs per-mutant proofs.");
        return;
    }
    // A non-zero-valued return type (&[u32]) + a body with no mutation site +
    // a weak `ens` that does not pin the result.
    let program = "fn pick(xs: &[u32]) -> &[u32]\n  \
                   req xs.len() <= 10\n  \
                   ens result.len() <= 10\n  \
                   fx pure\n{\n  xs\n}\n";
    let path = write_temp("weak_unscored", program);
    let cache_dir = unique_cache_dir();
    let _ = std::fs::remove_dir_all(&cache_dir);
    let certs = run_check_in(&path, &cache_dir);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir_all(&cache_dir);

    let cert = cert_for(&certs, "pick");

    // The AUTHORITY's expectation: §7's floor must catch a contract that
    // under-constrains its result. A weak contract must NOT certify clean L3 with
    // a vacuous `0/0` score. Either it is gated (`WeakContract`) OR it is scored
    // with a real denominator — never an unscored clean certify.
    let mk = cert
        .get("contract_quality")
        .and_then(|q| q.get("mutants_killed"))
        .and_then(|m| m.as_str())
        .unwrap_or("0/0");
    let unscored_clean_certify = is_clean_l3(cert) && mk == "0/0";
    assert!(
        !unscored_clean_certify,
        "§7 / R-DEFER-9 divergence: the weak contract `ens result.len() <= 10` \
         certifies clean L3 UNSCORED (`mutants_killed: \"0/0\"`) — the floor was \
         bypassed because `&[u32]` generates no early-return mutant and the body \
         has no mutation site. A contract that fails to pin its result must be \
         caught by the §7 floor, not certified vacuously. cert: {cert}"
    );
}

// ----------------------------------------------------------------------------
// DIVERGENCE 2 — the cache-bypass: a stale SAME-VERSION pre-gate cert is re-served
// WITHOUT mutation gating.
//
// Authority: `.design/forge/mutation-scoring.md` REQ-7 (the gate runs on every
// L3 proof; the proof cache makes re-runs cheap but must not let a stale verdict
// skip the gate) + `goal.md` R-DEFER-9 (a cache that re-serves a pre-gate clean
// L3 for a weak contract is a bypass = a hole).
//
// The cache key (`cache::cache_key`) is (lowered_src, seed, verus_version,
// THERMITE_VERSION). Commit fa55760 introduced the mutation gate but did NOT bump
// `forge`'s version (still 0.1.0), and the gate's existence / the floor are NOT
// in the key. So a cert stored by PRE-#12 forge (#5/#6/#13 all shipped at 0.1.0
// and populate the SAME `target/` cache) for the weak-contract program `f` is an
// L3-CLEAN cert under the identical key — and a post-#12 warm check serves it on
// a HIT (`check::check_file_with_options`, line ~273) BEFORE the gate runs,
// certifying the weak contract L3.
//
// This is genuinely reachable: any developer/CI with a proof cache populated by
// pre-#12 forge at version 0.1.0 (the gate-introducing commit did not invalidate
// the cache) bypasses the new gate. The fix is a cache-key version bump (or a
// gate-version tag in the key) by the gate-introducing commit.
//
// The test simulates the pre-#12 cache entry (the gate is the ONLY new behavior
// since #13; a pre-gate forge at the same version stored an L3-clean cert for `f`)
// by planting an L3-clean cert at the main-item key, then runs a warm check. The
// AUTHORITY's expectation is that the weak contract stays GATED; the current
// toolchain serves the stale clean cert -> the test FAILS, pinning the bypass.
// Tracking: filed as a `-l blocker` crosslink issue (see report).
// ----------------------------------------------------------------------------
#[test]
fn divergence_stale_same_version_cache_entry_bypasses_mutation_gate() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — mutation scoring needs per-mutant proofs.");
        return;
    }
    // The AC-2 weak-but-non-vacuous contract: a cold check GATES it `WeakContract`
    // (kill ratio 1/2 < 0.60), with the early-return-0 mutant surviving.
    let program = "fn f(a: u32, b: u32) -> u32\n  \
                   req a <= 10 && b <= 10\n  \
                   ens result <= 1000000\n  \
                   fx pure\n{\n  a + b\n}\n";
    let path = write_temp("cache_bypass", program);
    let cache_dir = unique_cache_dir();
    let _ = std::fs::remove_dir_all(&cache_dir);

    // Cold run: confirm the gate fires (the cold verdict is the GROUND TRUTH the
    // warm run must preserve). This populates the cache with the post-gate certs.
    let cold = run_check_in(&path, &cache_dir);
    let cold_cert = cert_for(&cold, "f");
    let cold_gated = cold_cert
        .get("reject")
        .and_then(|r| r.get("cause"))
        .and_then(|c| c.as_str())
        == Some("WeakContract");
    assert!(
        cold_gated,
        "precondition: the weak contract must be GATED on a cold check; cert: {cold_cert}"
    );

    // Locate the MAIN-ITEM cache entry (the `f` cert: the WeakContract L0 reject)
    // and OVERWRITE it with the cert PRE-#12 forge would have stored under the
    // SAME key (same lowered source, same seed, same verus+thermite version 0.1.0):
    // an L3-CLEAN cert with the forward-declared `mutants_killed: "0/0"`. No
    // production code is touched — we only model the stale same-version cache file.
    let mut planted = false;
    for entry in std::fs::read_dir(&cache_dir).unwrap() {
        let p = entry.unwrap().path();
        if p.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let mut d: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        let is_main_weak = d
            .get("reject")
            .and_then(|r| r.get("cause"))
            .and_then(|c| c.as_str())
            == Some("WeakContract");
        if is_main_weak {
            d["level"] = Value::String("L3".to_string());
            d["reject"] = Value::Null;
            if let Some(q) = d
                .get_mut("contract_quality")
                .and_then(|q| q.as_object_mut())
            {
                q.insert(
                    "mutants_killed".to_string(),
                    Value::String("0/0".to_string()),
                );
                q.remove("survivor");
            }
            std::fs::write(&p, serde_json::to_string_pretty(&d).unwrap()).unwrap();
            planted = true;
            break;
        }
    }
    assert!(
        planted,
        "could not find the main-item cache entry to model the pre-#12 stale cert"
    );

    // Warm run: re-check the SAME program against the cache holding the stale
    // pre-#12 L3-clean cert.
    let warm = run_check_in(&path, &cache_dir);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir_all(&cache_dir);
    let warm_cert = cert_for(&warm, "f");

    // AUTHORITY (REQ-7 / R-DEFER-9): the weak contract's verdict must be IDENTICAL
    // to the cold gate — a stale same-version cache entry must NOT let the weak
    // contract certify clean L3, bypassing the mutation gate.
    assert!(
        !is_clean_l3(warm_cert),
        "REQ-7 / R-DEFER-9 divergence: a stale SAME-VERSION (thermite 0.1.0) pre-gate \
         cache entry is re-served as a clean L3 certify, BYPASSING the §7 mutation \
         floor — the weak contract `ens result <= 1000000` is gated `WeakContract` on \
         a cold check but certifies L3 from the stale cache. The gate-introducing \
         commit must invalidate the cache (bump the cache-key version input). warm \
         cert: {warm_cert}"
    );
}
