//! `forge/tests/cache_conformance.rs` — the #8 per-item content-addressed
//! proof-cache verification (`.design/forge/proof-cache.md` §Verification /
//! AC-1..AC-5; `thermite-design.md` §5.3). It drives the BUILT `forge` binary
//! (`forge check --json`) with a HERMETIC, per-test cache directory
//! (`FORGE_CACHE_DIR`) and a PINNED verus version (`VERUS_VERSION`), so tests do
//! not pollute each other or the shared `target/` cache, and do not depend on
//! order. Each test also uses UNIQUE program contents so keys never collide.
//!
//! Verus-needing tests SKIP LOUDLY when verus is absent (mirroring
//! `check_conformance.rs` / `lower_conformance.rs`) — EXCEPT the DECISIVE
//! solver-skip test, which deliberately removes verus from PATH AFTER populating
//! the cache to prove the HIT path never spawns the solver.
//!
//! Expected verdicts trace to `thermite-design.md` / `.design/forge/proof-cache.md`,
//! never copied from forge's own output (`goal.md` R-CHAR-3). `tests/` is not
//! anti-pattern-gated, so `unwrap`/`expect` are fine here.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;

/// A fixed verus version pin for the proof cache key, so a HIT's key is stable
/// across runs even when the verus binary is later removed (AC-1). This is a
/// test-controlled constant; the key composition is what is under test.
const PINNED_VERUS_VERSION: &str = "verus-test-pin-0.0.1";

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// `true` iff verus can be located (`VERUS_BIN`, then PATH, then
/// `~/.local/bin/verus`) — mirrors `check_conformance.rs`. SKIP LOUDLY otherwise.
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

/// A unique, per-test temp cache directory so tests are hermetic and
/// order-independent (`.design/forge/proof-cache.md` §Verification — test
/// isolation).
fn unique_cache_dir(tag: &str) -> PathBuf {
    static C: AtomicU64 = AtomicU64::new(0);
    let n = C.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "forge_cachetest_{}_{}_{}",
        tag,
        std::process::id(),
        n
    ))
}

/// A unique `.th` fixture path with unique CONTENTS so its cache key cannot
/// collide with another test's item (test isolation).
fn write_fixture(tag: &str, body: &str) -> PathBuf {
    static C: AtomicU64 = AtomicU64::new(0);
    let n = C.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "forge_cachefix_{}_{}_{}.th",
        tag,
        std::process::id(),
        n
    ));
    std::fs::write(&path, body).expect("write fixture");
    path
}

/// Run `forge check <file> --json` with explicit cache dir + pinned verus
/// version, optionally overriding `PATH` (used to make verus UNavailable). Extra
/// env entries are applied on top. Returns (exit_code, parsed cert array).
fn run_check(
    file: &Path,
    cache_dir: &Path,
    extra_env: &HashMap<String, String>,
) -> (Option<i32>, Vec<Value>) {
    let mut cmd = Command::new(forge_bin());
    cmd.arg("check").arg(file).arg("--json");
    cmd.env("FORGE_CACHE_DIR", cache_dir);
    cmd.env("VERUS_VERSION", PINNED_VERUS_VERSION);
    for (k, v) in extra_env {
        cmd.env(k, v);
    }
    let out = cmd.output().unwrap_or_else(|e| panic!("spawn forge: {e}"));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!(
            "forge --json stdout must be one JSON document: {e}\nstdout:\n{stdout}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stderr)
        )
    });
    let arr = value
        .as_array()
        .unwrap_or_else(|| panic!("forge --json must emit a cert array: {value}"))
        .clone();
    (out.status.code(), arr)
}

fn find_cert<'a>(certs: &'a [Value], item: &str) -> &'a Value {
    certs
        .iter()
        .find(|c| c.get("item").and_then(|v| v.as_str()) == Some(item))
        .unwrap_or_else(|| panic!("no certificate for item `{item}` in {certs:?}"))
}

/// A self-contained, verifiable single-`fn` program (no spec-fn deps) so the
/// item certifies L3 on a fresh run. Provenance: a trivial true postcondition
/// over a saturating identity, hand-derived (R-CHAR-3) — it must verify.
fn verifiable_program(name: &str) -> String {
    format!(
        "fn {name}(x: u64) -> u64\n  req x < 1000\n  ens result == x\n  fx  pure\n{{\n  x\n}}\n"
    )
}

// ---- (1) HIT: second run is cached:true with equal deterministic fields ----

#[test]
fn second_run_is_a_cache_hit_with_equal_deterministic_fields() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — proof-cache HIT test not run.");
        return;
    }
    let cache_dir = unique_cache_dir("hit");
    let _ = std::fs::remove_dir_all(&cache_dir);
    let fixture = write_fixture("hit", &verifiable_program("hit_fn"));
    let env = HashMap::new();

    // First run populates the cache: a fresh verify, cached:false.
    let (code1, certs1) = run_check(&fixture, &cache_dir, &env);
    assert_eq!(code1, Some(0), "fresh verify must certify");
    let c1 = find_cert(&certs1, "hit_fn");
    assert_eq!(c1["level"], Value::from("L3"), "fresh run certifies L3");
    assert_eq!(c1["cached"], Value::from(false), "first run is not cached");

    // Second run is a HIT: cached:true, deterministic fields equal.
    let (code2, certs2) = run_check(&fixture, &cache_dir, &env);
    assert_eq!(code2, Some(0));
    let c2 = find_cert(&certs2, "hit_fn");
    assert_eq!(c2["cached"], Value::from(true), "second run is a cache HIT");
    // Deterministic (oracle) fields are byte-equal (REQ-2 — hit == fresh verify).
    for f in ["item", "level", "effects", "slag"] {
        assert_eq!(
            c1[f], c2[f],
            "deterministic field `{f}` must equal across hit"
        );
    }

    let _ = std::fs::remove_file(&fixture);
    let _ = std::fs::remove_dir_all(&cache_dir);
}

// ---- (2) DECISIVE: solver skipped — HIT with verus UNAVAILABLE --------------

#[test]
fn cache_hit_serves_l3_with_verus_unavailable() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — cannot populate the cache for the decisive test.");
        return;
    }
    let cache_dir = unique_cache_dir("decisive");
    let _ = std::fs::remove_dir_all(&cache_dir);
    let fixture = write_fixture("decisive", &verifiable_program("decisive_fn"));

    // Populate the cache WITH verus present (pinned version, real binary on PATH).
    let (code1, certs1) = run_check(&fixture, &cache_dir, &HashMap::new());
    assert_eq!(code1, Some(0), "population run must certify L3");
    assert_eq!(
        find_cert(&certs1, "decisive_fn")["cached"],
        Value::from(false)
    );

    // Re-run with verus made UNAVAILABLE: empty PATH so `verus` cannot be spawned.
    // The pinned VERUS_VERSION keeps the key identical → the HIT is served WITHOUT
    // a verus spawn. If the solver were NOT skipped, this would be
    // ForgeError::VerusAbsent (exit 2, empty stdout) per check.md REQ-6 — the
    // decisive solver-skip evidence (AC-1).
    let mut no_verus = HashMap::new();
    no_verus.insert("PATH".to_string(), String::new());
    let (code2, certs2) = run_check(&fixture, &cache_dir, &no_verus);
    assert_eq!(
        code2,
        Some(0),
        "the cached L3 must be served even with verus absent (the solver was skipped)"
    );
    let c2 = find_cert(&certs2, "decisive_fn");
    assert_eq!(c2["cached"], Value::from(true), "served from the cache");
    assert_eq!(
        c2["level"],
        Value::from("L3"),
        "the cached L3 verdict is served unchanged"
    );

    let _ = std::fs::remove_file(&fixture);
    let _ = std::fs::remove_dir_all(&cache_dir);
}

// ---- (2b) Control: verus-absent on a COLD cache IS VerusAbsent --------------
// Proves the decisive test's exit-0 is due to the cache, not a no-op: the SAME
// verus-absent environment on an EMPTY cache fails (exit 2, empty stdout).

#[test]
fn cold_cache_with_verus_unavailable_is_environment_error() {
    let cache_dir = unique_cache_dir("cold");
    let _ = std::fs::remove_dir_all(&cache_dir);
    let fixture = write_fixture("cold", &verifiable_program("cold_fn"));

    // Empty PATH so `verus` cannot be spawned, cold cache → environment error.
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(&fixture)
        .arg("--json")
        .env("FORGE_CACHE_DIR", &cache_dir)
        .env("VERUS_VERSION", PINNED_VERUS_VERSION)
        .env("PATH", "")
        .output()
        .expect("spawn forge");
    assert_eq!(
        out.status.code(),
        Some(2),
        "verus-absent on a cold cache is the environment exit code, not a verdict"
    );
    assert!(
        out.stdout.is_empty(),
        "an environment error writes nothing to stdout"
    );

    let _ = std::fs::remove_file(&fixture);
    let _ = std::fs::remove_dir_all(&cache_dir);
}

// ---- (3) INVALIDATION: changed body → MISS (cached:false, re-verified) ------

#[test]
fn changed_body_is_a_cache_miss() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — invalidation test not run.");
        return;
    }
    let cache_dir = unique_cache_dir("invalidate");
    let _ = std::fs::remove_dir_all(&cache_dir);
    let env = HashMap::new();

    // Populate with one body.
    let f1 = write_fixture(
        "invalidate",
        "fn inv_fn(x: u64) -> u64\n  req x < 1000\n  ens result == x\n  fx  pure\n{\n  x\n}\n",
    );
    let (code1, certs1) = run_check(&f1, &cache_dir, &env);
    assert_eq!(code1, Some(0));
    assert_eq!(find_cert(&certs1, "inv_fn")["cached"], Value::from(false));

    // Re-check with a DIFFERENT body+contract for the same name → different
    // lowered source → MISS (cached:false, re-verified). Distinct file so the
    // first cache entry persists; the key differs because the lowered source does.
    let f2 = write_fixture(
        "invalidate",
        "fn inv_fn(x: u64) -> u64\n  req x < 1000\n  ens result == x + 1\n  fx  pure\n{\n  x + 1\n}\n",
    );
    let (_code2, certs2) = run_check(&f2, &cache_dir, &env);
    assert_eq!(
        find_cert(&certs2, "inv_fn")["cached"],
        Value::from(false),
        "a changed body/contract is a MISS — the key tracks the lowered source"
    );

    let _ = std::fs::remove_file(&f1);
    let _ = std::fs::remove_file(&f2);
    let _ = std::fs::remove_dir_all(&cache_dir);
}

// ---- (6) corpus: sum still certifies L3 with the cache wired ---------------

#[test]
fn corpus_sum_still_certifies_l3_through_the_cache() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — corpus-through-cache test not run.");
        return;
    }
    let corpus = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("conformance")
        .join("sum.th");
    let cache_dir = unique_cache_dir("corpus_sum");
    let _ = std::fs::remove_dir_all(&cache_dir);
    let env = HashMap::new();

    // Fresh: certifies L3 (the cache changes performance, not verdict — AC-5).
    let (code1, certs1) = run_check(&corpus, &cache_dir, &env);
    assert_eq!(code1, Some(0), "sum must certify L3 on a fresh run");
    let s1 = find_cert(&certs1, "sum");
    assert_eq!(s1["level"], Value::from("L3"));
    assert_eq!(s1["effects"], serde_json::json!(["pure"]));
    assert_eq!(s1["cached"], Value::from(false));

    // Cached: still L3, deterministic fields equal (the soundness invariant).
    let (code2, certs2) = run_check(&corpus, &cache_dir, &env);
    assert_eq!(code2, Some(0));
    let s2 = find_cert(&certs2, "sum");
    assert_eq!(s2["cached"], Value::from(true), "second sum run is a HIT");
    assert_eq!(
        s2["level"],
        Value::from("L3"),
        "the cached verdict is still L3"
    );
    assert_eq!(s1["effects"], s2["effects"]);
    assert_eq!(s1["slag"], s2["slag"]);

    let _ = std::fs::remove_dir_all(&cache_dir);
}
