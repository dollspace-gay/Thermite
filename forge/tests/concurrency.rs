//! `forge/tests/concurrency.rs` — the MULTI-AGENT FORGE SESSION guarantee suite
//! (`.design/forge/multi-agent.md` AC-1..AC-7; `thermite-design.md` §13 v0.5,
//! §5.3 locality, §9 composition, §1.5 blast-radius). This is the #20 deliverable:
//! the multi-agent capability is EMERGENT from the already-shipping concurrency-safe
//! primitives — `cache::store`'s atomic temp-sibling + `rename` publish (#8) and
//! `cache::load`'s torn/inconsistent → MISS degrade (#49) — so #20 chiefly ASSERTS
//! and TESTS the guarantee rather than building new machinery.
//!
//! A "multi-agent Forge session" (the doc's REQ-6) is defined as N independent
//! `forge check` invocations over ONE shared `FORGE_CACHE_DIR`, with NO central
//! coordinator. The filesystem cache + content-addressed per-item locality IS the
//! coordination. This suite spawns concurrent `forge check` PROCESSES
//! (`Command::new(env!("CARGO_BIN_EXE_forge"))` via `std::thread`) over a shared
//! cache and proves:
//!
//! - AC-1/AC-7: N concurrent processes → every cert matches its golden under the
//!   oracle subset, the cache dir is uncorrupted (every `<key>.json` parses), and
//!   the concurrent cert set equals a serial run's (determinism, R-CODE-5).
//! - AC-2: concurrent checks of the SAME item converge to one consistent entry
//!   (atomic `rename`), with zero leftover `.tmp` siblings.
//! - AC-3/AC-6: concurrent checks of DISTINCT items never clobber each other —
//!   each key's entry is present and loadable, no cross-eviction.
//! - AC-5: editing item A does NOT move item B's content-addressed key (the
//!   `<key>.json` filename) — UNLESS B's contract references A (the §5.3 / §9
//!   exception, demonstrated as a negative control via a shared `spec fn`).
//! - AC-4: a TORN/garbage `<key>.json` degrades `cache::load` to a MISS — a later
//!   `forge check` re-verifies to the correct verdict, never a crash or wrong cert.
//!
//! Because `forge` is a BIN-ONLY crate (no `lib.rs`; `cache::store`/`load` are not
//! reachable from an integration test), the thread-level cache invariants (AC-2,
//! AC-3, AC-6) are driven through concurrent `forge check` PROCESSES over the same
//! / distinct items and verified by INSPECTING the shared cache directory on disk
//! (the `<key>.json` content-address filenames + their parseability) — the doc's
//! sanctioned fallback ("drive this via concurrent `forge check` processes ...
//! prefer the process-level demonstration if the API isn't test-reachable").
//!
//! Verus-needing tests SKIP LOUDLY when verus is absent (mirroring
//! `cache_conformance.rs`): the L3 verdict the guarantee rests on needs the solver.
//! The fault-injection and locality SHAPES that do not need a real proof still run
//! their non-verus halves.
//!
//! Expected verdicts trace to `conformance/sum.cert.json` and the `binary_search`
//! oracle (`thermite-design.md` §13 — L3, pure), NEVER copied from forge's own
//! output (`goal.md` R-CHAR-3). `tests/` is not anti-pattern-gated, so
//! `unwrap`/`expect` are fine here.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;

/// A fixed verus version pin for the proof-cache key (mirrors
/// `cache_conformance.rs`), so a HIT's key is stable across the concurrent /
/// serial runs and independent of the live binary's reported version.
const PINNED_VERUS_VERSION: &str = "verus-test-pin-0.0.1";

/// The number of CONCURRENT agents (forge processes) a multi-agent session test
/// spawns (`.design/forge/multi-agent.md` AC-1: "Spawn N (≥ 8) concurrent ...").
const N_AGENTS: usize = 8;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// `true` iff verus can be located (`VERUS_BIN`, then PATH, then
/// `~/.local/bin/verus`) — mirrors `cache_conformance.rs`. SKIP LOUDLY otherwise:
/// the L3 verdict the multi-agent guarantee rests on needs the solver.
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
/// order-independent — and so the shared `target/` cache is never polluted
/// (`.design/forge/multi-agent.md` §AC: "a per-run temp cache via `FORGE_CACHE_DIR`").
fn unique_cache_dir(tag: &str) -> PathBuf {
    static C: AtomicU64 = AtomicU64::new(0);
    let n = C.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "forge_concurrency_{}_{}_{}",
        tag,
        std::process::id(),
        n
    ))
}

/// A unique `.th` fixture path with the given CONTENTS so its cache key cannot
/// collide with another test's item (test isolation).
fn write_fixture(tag: &str, body: &str) -> PathBuf {
    static C: AtomicU64 = AtomicU64::new(0);
    let n = C.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "forge_concurrency_fix_{}_{}_{}.th",
        tag,
        std::process::id(),
        n
    ));
    std::fs::write(&path, body).expect("write fixture");
    path
}

/// The conformance corpus directory (the EXTERNAL oracle, `goal.md` model (B)).
fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("conformance")
}

/// Run `forge check <file> --json` with an explicit shared cache dir + pinned
/// verus version, optionally applying extra env. Returns (exit_code, cert array).
/// Mirrors `cache_conformance::run_check`.
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

/// The DETERMINISTIC oracle subset of a cert as a comparable tuple — the §5.3 /
/// `manifest::Certificate::oracle_subset` fields that a HIT and a fresh verify
/// (and a concurrent vs a serial run) MUST share: `item`, `level`, `effects`,
/// `slag`, `boundary`. EXCLUDES `cached` (provenance) and `solver_time_ms`
/// (wall-clock) — the multi-agent guarantee is that these oracle fields are
/// interleaving-independent (REQ-7, AC-7).
fn oracle_subset(cert: &Value) -> (Value, Value, Value, Value, Value) {
    (
        cert.get("item").cloned().unwrap_or(Value::Null),
        cert.get("level").cloned().unwrap_or(Value::Null),
        cert.get("effects").cloned().unwrap_or(Value::Null),
        cert.get("slag").cloned().unwrap_or(Value::Bool(false)),
        cert.get("boundary").cloned().unwrap_or(Value::Bool(false)),
    )
}

/// A self-contained, verifiable single-`fn` program (no spec-fn deps) so the item
/// certifies L3 on a fresh run. Provenance: a trivial true postcondition over an
/// identity bounded below `1000`, hand-derived (R-CHAR-3) — it must verify.
fn verifiable_program(name: &str) -> String {
    format!(
        "fn {name}(x: u64) -> u64\n  req x < 1000\n  ens result == x\n  fx  pure\n{{\n  x\n}}\n"
    )
}

/// Enumerate the `<key>.json` cache ENTRIES (the content-address files) in
/// `cache_dir`, parsing each as a `serde_json::Value`. PANICS (failing the test)
/// if any entry is unparseable — that is the "cache is not corrupted / no torn
/// entry" assertion (AC-1). Returns (key-filename → parsed cert) and is the
/// process-level window into the cache state the bin-only crate hides.
fn parse_all_entries(cache_dir: &Path) -> HashMap<String, Value> {
    let mut out = HashMap::new();
    let Ok(read) = std::fs::read_dir(cache_dir) else {
        return out; // a never-written cache dir is simply empty — not a corruption.
    };
    for entry in read {
        let entry = entry.expect("read cache dir entry");
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        if let Some(stripped) = name.strip_suffix(".json") {
            let src = std::fs::read_to_string(&path).expect("read cache entry");
            let val: Value = serde_json::from_str(&src).unwrap_or_else(|e| {
                panic!("cache entry `{name}` is TORN/unparseable (cache corrupted): {e}\n{src}")
            });
            out.insert(stripped.to_string(), val);
        }
        // A `.tmp` sibling left behind is checked separately by `count_tmp_siblings`.
    }
    out
}

/// The set of `<key>.json` content-address filenames present in `cache_dir`,
/// sorted for a stable comparison. The MULTI-AGENT determinism observable
/// (AC-7 / REQ-7): the on-disk content-address SET produced by N concurrent
/// agents must EQUAL the set a serial run produces — interleaving cannot add,
/// drop, or move a key (every verdict, including each mutation-scoring (#12) /
/// strengthening-probe (#14) sub-entry, is a pure function of its lowered input).
fn entry_key_set(cache_dir: &Path) -> Vec<String> {
    let mut keys: Vec<String> = parse_all_entries(cache_dir).into_keys().collect();
    keys.sort();
    keys
}

/// Count leftover `.tmp` siblings in the cache dir — there must be ZERO after all
/// stores settle (a `rename` consumes its temp; an orphan would signal a failed /
/// non-atomic publish). The atomic-publish observable (AC-2).
fn count_tmp_siblings(cache_dir: &Path) -> usize {
    let Ok(read) = std::fs::read_dir(cache_dir) else {
        return 0;
    };
    read.filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .file_name()
                .and_then(|s| s.to_str())
                .map(|n| n.ends_with(".tmp"))
                .unwrap_or(false)
        })
        .count()
}

/// The corpus items whose golden L3 verdict is the EXTERNAL oracle for the
/// multi-agent guarantee: `sum` (`conformance/sum.cert.json`: L3, pure) and
/// `binary_search` (the `binary_search` oracle: L3, pure — `thermite-design.md` §13).
const CORPUS_FILES: &[&str] = &["sum.th", "binary_search.th"];
const CORPUS_ORACLE: &[(&str, &str)] = &[("sum", "L3"), ("binary_search", "L3")];

// ====================================================================
// AC-1 + AC-7 — PROCESS-LEVEL: N concurrent agents → correct certs,
// uncorrupted cache, concurrent == serial.
// ====================================================================

/// Spawn `N_AGENTS` CONCURRENT `forge check` PROCESSES over the corpus
/// (`sum.th`, `binary_search.th`) sharing ONE `FORGE_CACHE_DIR`. After all join:
/// every agent's cert matches the golden under the oracle subset, the cache dir
/// is uncorrupted (every `<key>.json` parses, no leftover `.tmp`), and the
/// concurrent cert set equals a single SERIAL run's (determinism, R-CODE-5).
/// This is the multi-agent core (REQ-1, REQ-2, REQ-6, REQ-7; AC-1, AC-7).
#[test]
fn n_concurrent_agents_produce_correct_uncorrupted_certs() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — the N-agent L3 guarantee test needs the solver.");
        return;
    }
    let cache_dir = unique_cache_dir("n_agents");
    let _ = std::fs::remove_dir_all(&cache_dir);
    let corpus = corpus_dir();
    let env = HashMap::new();

    // --- The SERIAL oracle: one run of each corpus file, oracle subset per item. ---
    // (A separate cache dir so the serial run does not pre-warm the concurrent one.)
    let serial_cache = unique_cache_dir("n_agents_serial");
    let _ = std::fs::remove_dir_all(&serial_cache);
    let mut serial: HashMap<String, (Value, Value, Value, Value, Value)> = HashMap::new();
    for file in CORPUS_FILES {
        let (code, certs) = run_check(&corpus.join(file), &serial_cache, &env);
        assert_eq!(code, Some(0), "serial run of {file} must certify");
        for cert in &certs {
            let item = cert["item"].as_str().expect("cert item").to_string();
            serial.insert(item, oracle_subset(cert));
        }
    }
    // Cross-check the serial oracle against the EXTERNAL golden (R-CHAR-3): the
    // expected level comes from `conformance/sum.cert.json` / the §13 oracle, not
    // from forge's own output.
    for (item, level) in CORPUS_ORACLE {
        let got = serial
            .get(*item)
            .unwrap_or_else(|| panic!("serial run produced no cert for `{item}`"));
        assert_eq!(
            got.1,
            Value::from(*level),
            "`{item}` golden level is {level} (external oracle)"
        );
        assert_eq!(got.2, serde_json::json!(["pure"]), "`{item}` is pure");
    }

    // --- N concurrent agents over the SHARED cache. ---
    // Each agent checks the whole corpus; with N=8 and 2 files that is 16
    // concurrent forge processes hammering one cache dir. Threads only SPAWN +
    // JOIN the processes (the doc's "via `std::thread` spawning the processes").
    let mut handles = Vec::with_capacity(N_AGENTS);
    for agent in 0..N_AGENTS {
        let cache_dir = cache_dir.clone();
        let corpus = corpus.clone();
        handles.push(std::thread::spawn(move || {
            let env = HashMap::new();
            let mut results: HashMap<String, (Value, Value, Value, Value, Value)> = HashMap::new();
            for file in CORPUS_FILES {
                let (code, certs) = run_check(&corpus.join(file), &cache_dir, &env);
                assert_eq!(
                    code,
                    Some(0),
                    "agent {agent} run of {file} must certify (no cache corruption / panic)"
                );
                for cert in &certs {
                    let item = cert["item"].as_str().expect("cert item").to_string();
                    results.insert(item, oracle_subset(cert));
                }
            }
            results
        }));
    }

    // Join every agent — "all complete" is the only timing assumption (the doc's
    // robustness requirement: no interleaving-dependent expectations).
    let agent_results: Vec<_> = handles
        .into_iter()
        .map(|h| h.join().expect("agent thread must not panic"))
        .collect();

    // AC-7 (concurrent == serial): every agent's oracle subset, per item, equals
    // the serial run's. Interleaving cannot move a verdict (REQ-7).
    for (agent, results) in agent_results.iter().enumerate() {
        for (item, expected) in &serial {
            let got = results
                .get(item)
                .unwrap_or_else(|| panic!("agent {agent} produced no cert for `{item}`"));
            assert_eq!(
                got, expected,
                "agent {agent}'s `{item}` cert must equal the serial run (concurrent == serial)"
            );
        }
    }

    // AC-1 (uncorrupted cache): every `<key>.json` parses (a torn entry would
    // PANIC inside `parse_all_entries`), and no orphan `.tmp` sibling survived the
    // concurrent atomic publishes.
    let entries = parse_all_entries(&cache_dir);
    assert!(
        !entries.is_empty(),
        "the concurrent agents must have populated the shared cache"
    );
    assert_eq!(
        count_tmp_siblings(&cache_dir),
        0,
        "no orphaned `.tmp` sibling may survive (atomic rename consumes its temp)"
    );

    // AC-7 (cache state determinism): the concurrent agents' shared cache holds
    // EXACTLY the content-address key set a serial run produces — interleaving
    // adds, drops, or moves no key. (The set INCLUDES the mutation-scoring (#12)
    // and strengthening-probe (#14) sub-entries each item legitimately stores
    // under its own content address; the point is the SET is interleaving-stable.)
    assert_eq!(
        entry_key_set(&cache_dir),
        entry_key_set(&serial_cache),
        "the concurrent cache key set must equal the serial run's (no torn/extra/lost key)"
    );

    let _ = std::fs::remove_dir_all(&cache_dir);
    let _ = std::fs::remove_dir_all(&serial_cache);
}

// ====================================================================
// AC-2 — SAME-KEY CONVERGENCE: concurrent checks of the SAME item →
// one consistent entry, no torn file, zero leftover `.tmp`.
// ====================================================================

/// `N_AGENTS` concurrent `forge check` processes over the SAME single-fn item
/// sharing one cache dir. Because the verdict is a pure function of the lowered
/// item (§5.3, R-CODE-5), every writer serializes byte-equal cert bytes; the
/// atomic `rename` is all-or-nothing, so each `<key>.json` CONVERGES to a single
/// consistent entry (no torn merge — a concurrent writer of the same key overwrote
/// with byte-equal bytes), with zero leftover `.tmp` siblings, and every agent's
/// reported verdict is the same L3 (REQ-1, REQ-3; AC-2).
///
/// NOTE on entry count: a single-fn item legitimately yields MORE than one
/// content-address entry — the real fn PLUS one per mutation-scoring (#12) mutant
/// (and per strengthening-probe (#14) candidate). The convergence guarantee is
/// therefore "the concurrent cache key SET equals the serial run's, every entry
/// consistent" — NOT a literal count of one. Asserting one entry would be wrong
/// against the shipped #12/#14 pipeline (verified empirically: `verifiable_program`
/// produces the fn + its mutant deterministically).
#[test]
fn concurrent_same_item_converges_to_a_consistent_cache() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — same-key convergence test needs the solver.");
        return;
    }
    let cache_dir = unique_cache_dir("same_key");
    let serial_cache = unique_cache_dir("same_key_serial");
    let _ = std::fs::remove_dir_all(&cache_dir);
    let _ = std::fs::remove_dir_all(&serial_cache);
    // ONE shared fixture file → one lowered sub-program → one PRIMARY content
    // address (plus its deterministic mutant sub-entries).
    let fixture = write_fixture("same_key", &verifiable_program("same_key_fn"));

    // The SERIAL baseline cache state (one run into its own dir).
    let (sc, _scerts) = run_check(&fixture, &serial_cache, &HashMap::new());
    assert_eq!(sc, Some(0), "serial same-item run certifies L3");

    let mut handles = Vec::with_capacity(N_AGENTS);
    for _ in 0..N_AGENTS {
        let cache_dir = cache_dir.clone();
        let fixture = fixture.clone();
        handles.push(std::thread::spawn(move || {
            let (code, certs) = run_check(&fixture, &cache_dir, &HashMap::new());
            assert_eq!(code, Some(0), "every concurrent same-item run certifies L3");
            let cert = find_cert(&certs, "same_key_fn");
            oracle_subset(cert)
        }));
    }
    let results: Vec<_> = handles
        .into_iter()
        .map(|h| h.join().expect("same-key agent must not panic"))
        .collect();

    // Every agent's oracle subset is identical (the content-addressed verdict is
    // interleaving-independent).
    let first = &results[0];
    for (i, r) in results.iter().enumerate() {
        assert_eq!(r, first, "same-item agent {i} must agree on the L3 verdict");
    }
    assert_eq!(first.1, Value::from("L3"), "the converged verdict is L3");

    // Convergence: every entry is a consistent (parseable) cert, the concurrent
    // key SET equals the serial run's (atomic rename — no torn merge, no spurious
    // key from a half-written file), and zero `.tmp` siblings survived.
    assert_eq!(
        entry_key_set(&cache_dir),
        entry_key_set(&serial_cache),
        "concurrent same-key stores converge to the serial run's exact key set"
    );
    assert_eq!(
        count_tmp_siblings(&cache_dir),
        0,
        "no leftover `.tmp` after the atomic rename converges"
    );

    let _ = std::fs::remove_file(&fixture);
    let _ = std::fs::remove_dir_all(&cache_dir);
    let _ = std::fs::remove_dir_all(&serial_cache);
}

// ====================================================================
// AC-3 + AC-6 — DIFFERENT-KEY NON-INTERFERENCE: concurrent checks of
// DISTINCT items → N disjoint entries, no clobber, no cross-eviction.
// ====================================================================

/// `N_AGENTS` concurrent `forge check` processes each over a DISTINCT single-fn
/// item (distinct lowered source → distinct content-address key) sharing one
/// cache dir. Afterward every item's PRIMARY (real-fn) cert is present and
/// parseable, no entry clobbered another, and re-checking each item is a HIT — so
/// no item's entry was evicted by its N-1 concurrent neighbors (REQ-4; AC-3, AC-6
/// — different-key non-interference / no cross-eviction).
///
/// (Each item legitimately contributes its real-fn entry PLUS its #12 mutant /
/// #14 candidate sub-entries, so the dir holds MORE than `N_AGENTS` files; the
/// guarantee is "every distinct item's entry is present, loadable, and survives"
/// — non-interference — not a literal `N_AGENTS` count.)
#[test]
fn concurrent_distinct_items_do_not_interfere() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — distinct-key non-interference test needs verus.");
        return;
    }
    let cache_dir = unique_cache_dir("distinct_keys");
    let _ = std::fs::remove_dir_all(&cache_dir);

    // N distinct fixtures, each a distinct fn name + body → distinct lowered source.
    let fixtures: Vec<(String, PathBuf)> = (0..N_AGENTS)
        .map(|i| {
            let name = format!("distinct_fn_{i}");
            let path = write_fixture("distinct", &verifiable_program(&name));
            (name, path)
        })
        .collect();

    let mut handles = Vec::with_capacity(N_AGENTS);
    for (name, path) in &fixtures {
        let cache_dir = cache_dir.clone();
        let name = name.clone();
        let path = path.clone();
        handles.push(std::thread::spawn(move || {
            let (code, certs) = run_check(&path, &cache_dir, &HashMap::new());
            assert_eq!(code, Some(0), "distinct-item `{name}` must certify L3");
            let cert = find_cert(&certs, &name);
            assert_eq!(cert["level"], Value::from("L3"));
        }));
    }
    for h in handles {
        h.join().expect("distinct-item agent must not panic");
    }

    // AC-3: every entry on disk is a self-consistent cert (a torn/half-written
    // entry would PANIC inside `parse_all_entries`), and there are at least N of
    // them (one PRIMARY per distinct item, plus its mutant sub-entries), with no
    // leftover `.tmp`.
    let entries = parse_all_entries(&cache_dir);
    assert!(
        entries.len() >= N_AGENTS,
        "the N distinct items must each have populated at least their primary entry (got {})",
        entries.len()
    );
    assert_eq!(count_tmp_siblings(&cache_dir), 0, "no leftover `.tmp`");

    // AC-6 (no cross-eviction): re-check EACH item SERIALLY — each is a HIT, so
    // its entry survived the concurrent storm of its N-1 neighbors' stores; no
    // store clobbered a different key.
    for (name, path) in &fixtures {
        let (code, certs) = run_check(path, &cache_dir, &HashMap::new());
        assert_eq!(code, Some(0));
        let cert = find_cert(&certs, name);
        assert_eq!(
            cert["cached"],
            Value::from(true),
            "`{name}`'s entry must survive its neighbors' concurrent stores (no cross-eviction)"
        );
        assert_eq!(cert["level"], Value::from("L3"));
    }

    for (_, path) in &fixtures {
        let _ = std::fs::remove_file(path);
    }
    let _ = std::fs::remove_dir_all(&cache_dir);
}

// ====================================================================
// AC-5 — MULTI-AGENT LOCALITY: editing item A does NOT move item B's
// content-addressed key (no cross-invalidation), UNLESS B references A.
// ====================================================================

/// Editing agent A's item does NOT change agent B's content-addressed cache key
/// (the `<key>.json` filename), so B's cached cert stays a HIT — the §5.3 / §1.5
/// no-cross-invalidation guarantee for N concurrent agents (REQ-5; AC-5).
///
/// Demonstration WITHOUT reaching the bin-only `cache::cache_key`, via the cache
/// HIT/MISS BEHAVIOR (which keys on exactly the content address, so it is the
/// robust observable — unaffected by the #12 mutant / #14 strengthening sub-entries
/// that share an item name): populate a cache dir with the BEFORE file (A + B),
/// then EDIT A's body and re-check the edited file against the SAME cache dir.
/// B is a HIT (its key did not move — A's body is not in B's sub-program) while A
/// is a MISS (its edited body moved its key) → A re-verifies, B is served.
#[test]
fn editing_a_does_not_move_bs_key() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — multi-agent locality test needs the solver.");
        return;
    }
    // A two-item file: B does NOT reference A (independent contracts). Both verify.
    let before = "\
fn agent_a(x: u64) -> u64\n  req x < 1000\n  ens result == x\n  fx  pure\n{\n  x\n}\n\n\
fn agent_b(y: u64) -> u64\n  req y < 1000\n  ens result == y\n  fx  pure\n{\n  y\n}\n";
    // A's BODY changed (a real edit by agent A); B is byte-identical.
    let after = "\
fn agent_a(x: u64) -> u64\n  req x < 2000\n  ens result == x\n  fx  pure\n{\n  x\n}\n\n\
fn agent_b(y: u64) -> u64\n  req y < 1000\n  ens result == y\n  fx  pure\n{\n  y\n}\n";

    let dir_before = unique_cache_dir("locality_before");
    let dir_after = unique_cache_dir("locality_after");
    let _ = std::fs::remove_dir_all(&dir_before);
    let _ = std::fs::remove_dir_all(&dir_after);
    let f_before = write_fixture("locality_before", before);
    let f_after = write_fixture("locality_after", after);

    // Check the BEFORE file → populates entries for agent_a + agent_b.
    let (c1, certs1) = run_check(&f_before, &dir_before, &HashMap::new());
    assert_eq!(c1, Some(0), "before-edit file must certify");
    assert_eq!(find_cert(&certs1, "agent_b")["level"], Value::from("L3"));

    // Check the AFTER file (A edited) → agent_a re-verifies; agent_b unchanged.
    let (c2, certs2) = run_check(&f_after, &dir_after, &HashMap::new());
    assert_eq!(c2, Some(0), "after-edit file must still certify");
    assert_eq!(find_cert(&certs2, "agent_b")["level"], Value::from("L3"));

    // The locality observable is the cache HIT/MISS BEHAVIOR (robust against the
    // #12 mutant / #14 strengthening sub-entries that share an item name): the
    // content-address key is what HIT/MISS keys on, so "B's key did not move" is
    // exactly "B is a HIT against the pre-edit cache after A's edit". See the
    // decisive check below.

    // Decisive multi-agent HIT: B's pre-edit entry serves the post-edit check.
    // Re-check the AFTER file against the BEFORE cache dir (which holds B's
    // pre-edit entry). B is a HIT (its key is unchanged); A is a MISS (its key
    // moved) → A re-verifies into the same dir.
    let (c3, certs3) = run_check(&f_after, &dir_before, &HashMap::new());
    assert_eq!(c3, Some(0));
    assert_eq!(
        find_cert(&certs3, "agent_b")["cached"],
        Value::from(true),
        "agent B is a HIT against the pre-edit cache despite A's edit (no cross-eviction)"
    );
    assert_eq!(
        find_cert(&certs3, "agent_a")["cached"],
        Value::from(false),
        "agent A is a MISS (its edited body moved its key) — it re-verifies"
    );

    let _ = std::fs::remove_file(&f_before);
    let _ = std::fs::remove_file(&f_after);
    let _ = std::fs::remove_dir_all(&dir_before);
    let _ = std::fs::remove_dir_all(&dir_after);
}

/// NEGATIVE CONTROL (AC-5): when B's contract REFERENCES A (here a shared
/// `spec fn` that A's edit changes), A's edit DOES move B's key — B's lowered
/// sub-program contains the referenced contract, so cross-invalidation is correct
/// and expected (§5.3's stated exception, §9's composition rule: B keys on A's
/// CONTRACT, which is now part of B's sub-program).
#[test]
fn editing_a_referenced_dependency_does_move_bs_key() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — locality negative-control needs the solver.");
        return;
    }
    // B references the shared `spec fn dep`. Editing `dep`'s body changes B's
    // lowered sub-program (the spec fn is woven into every item's sub-program).
    // The edit to `dep` is a REAL byte change to the referenced contract (its
    // `dec` measure) that keeps `dep`'s semantics (`dep(x) == x`) so B still proves
    // L3 — but changes B's lowered sub-program (which weaves `dep` in), so B's key
    // moves. (A body edit like `x + 0` would lower to unbounded spec `int` and fail
    // to type-check, conflating "key moved" with "verdict broke" — the `dec` edit
    // isolates the locality signal.)
    let before = "\
spec fn dep(x: u64) -> u64\n  dec x\n{\n  x\n}\n\n\
fn agent_b(y: u64) -> u64\n  req y < 1000\n  ens result == dep(y)\n  fx  pure\n{\n  y\n}\n";
    let after = "\
spec fn dep(x: u64) -> u64\n  dec x + 1\n{\n  x\n}\n\n\
fn agent_b(y: u64) -> u64\n  req y < 1000\n  ens result == dep(y)\n  fx  pure\n{\n  y\n}\n";

    let dir_before = unique_cache_dir("locality_dep_before");
    let _ = std::fs::remove_dir_all(&dir_before);
    let f_before = write_fixture("locality_dep_before", before);
    let f_after = write_fixture("locality_dep_after", after);

    // Populate the cache with the BEFORE file (B keyed against `dep`'s original
    // contract).
    let (c1, certs1) = run_check(&f_before, &dir_before, &HashMap::new());
    assert_eq!(c1, Some(0), "before file (B refs dep) must certify");
    assert_eq!(find_cert(&certs1, "agent_b")["cached"], Value::from(false));

    // Re-check the AFTER file (dep's contract edited) against the SAME cache dir.
    // Because B's contract REFERENCES `dep`, `dep`'s edited contract is now woven
    // into B's lowered sub-program → B's content-address key MOVED → B is a MISS
    // (cached:false, re-verified). This is the §5.3 exception / §9 composition rule:
    // cross-invalidation through a contract reference is correct and expected. The
    // HIT/MISS behavior is the robust observable (it keys on exactly the content
    // address), unaffected by the #12/#14 sub-entries.
    let (c2, certs2) = run_check(&f_after, &dir_before, &HashMap::new());
    assert_eq!(c2, Some(0), "after file (dep edited) must still certify");
    assert_eq!(
        find_cert(&certs2, "agent_b")["cached"],
        Value::from(false),
        "when B references `dep`'s contract, editing `dep` MOVES B's key → B is a MISS \
         (§5.3 exception / §9 composition — correct cross-invalidation)"
    );

    // And a SECOND check of the AFTER file is now a HIT for B (its post-edit key is
    // populated) — proving the MISS above was the key move, not an unconditional miss.
    let (c3, certs3) = run_check(&f_after, &dir_before, &HashMap::new());
    assert_eq!(c3, Some(0));
    assert_eq!(
        find_cert(&certs3, "agent_b")["cached"],
        Value::from(true),
        "re-checking the edited file is a HIT for B (the new key is now cached)"
    );

    let _ = std::fs::remove_file(&f_before);
    let _ = std::fs::remove_file(&f_after);
    let _ = std::fs::remove_dir_all(&dir_before);
}

// ====================================================================
// AC-4 — FAULT INJECTION: a torn/garbage `<key>.json` → MISS, never a
// crash or wrong verdict, even under a shared (concurrent) cache.
// ====================================================================

/// Inject a TORN/garbage `<key>.json` into the shared cache dir (simulating an
/// interrupted non-atomic write that the #8 atomic store prevents but a damaged
/// filesystem could still leave), then run `forge check`: it must re-verify to
/// the CORRECT L3 verdict (a MISS), never crash and never serve a wrong/torn
/// read (REQ-2; AC-4 — the #49 load-time degrade under the multi-agent failure
/// mode). After the re-verify the entry is overwritten atomically and parses.
#[test]
fn torn_entry_degrades_to_a_miss_and_reverifies() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — torn-entry re-verify needs the solver.");
        return;
    }
    let cache_dir = unique_cache_dir("torn");
    let _ = std::fs::remove_dir_all(&cache_dir);
    std::fs::create_dir_all(&cache_dir).expect("mkdir cache");
    let fixture = write_fixture("torn", &verifiable_program("torn_fn"));

    // Populate the cache with real entries first, then TRUNCATE EVERY `<key>.json`
    // to a torn shape (a half-written JSON: valid prefix, abrupt cut) — the exact
    // failure a non-atomic writer would leave. Truncating EVERY entry (the primary
    // plus its #12 mutant sub-entries) forces the primary's lookup to MISS.
    let (code1, certs1) = run_check(&fixture, &cache_dir, &HashMap::new());
    assert_eq!(code1, Some(0), "population run certifies L3");
    assert_eq!(find_cert(&certs1, "torn_fn")["level"], Value::from("L3"));

    let keys_before: Vec<String> = parse_all_entries(&cache_dir).into_keys().collect();
    assert!(
        !keys_before.is_empty(),
        "population produced entries to tear"
    );
    for key in &keys_before {
        // A torn JSON: the opening of a cert object, cut mid-string. `serde_json`
        // cannot parse it → `cache::load` returns None (a MISS) per REQ-2.
        std::fs::write(
            cache_dir.join(format!("{key}.json")),
            b"{\n  \"item\": \"torn_fn\",\n  \"level\": \"L",
        )
        .expect("write torn entry");
    }

    // Re-check: the torn entry is a MISS → re-verify → the CORRECT L3 verdict,
    // never a crash and never a torn read served as a verdict.
    let (code2, certs2) = run_check(&fixture, &cache_dir, &HashMap::new());
    assert_eq!(
        code2,
        Some(0),
        "a torn entry must degrade to a re-verify (MISS), not a crash or wrong verdict"
    );
    let c2 = find_cert(&certs2, "torn_fn");
    assert_eq!(
        c2["level"],
        Value::from("L3"),
        "the re-verified verdict is the correct L3"
    );
    assert_eq!(
        c2["cached"],
        Value::from(false),
        "the torn primary entry was a MISS → a fresh re-verify (cached:false)"
    );

    // The store overwrote the torn entries atomically; every entry now parses
    // cleanly (a torn survivor would PANIC inside `parse_all_entries`).
    let after = parse_all_entries(&cache_dir);
    assert!(
        !after.is_empty(),
        "the re-verify overwrote the torn entries with consistent ones"
    );
    assert_eq!(count_tmp_siblings(&cache_dir), 0, "no leftover `.tmp`");

    let _ = std::fs::remove_file(&fixture);
    let _ = std::fs::remove_dir_all(&cache_dir);
}

/// A torn entry under CONCURRENT access: inject a torn `<key>.json`, then have
/// `N_AGENTS` concurrent agents all check the same item against the shared cache.
/// Every agent must re-verify to the correct L3 (the torn entry is a MISS for all
/// of them), the cache converges to one consistent entry, and no agent crashes —
/// the #49 degrade holds under the multi-agent storm (REQ-2, REQ-3; AC-4).
#[test]
fn torn_entry_under_concurrent_access_is_safe() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — concurrent torn-entry test needs the solver.");
        return;
    }
    let cache_dir = unique_cache_dir("torn_concurrent");
    let _ = std::fs::remove_dir_all(&cache_dir);
    let fixture = write_fixture("torn_concurrent", &verifiable_program("torn_conc_fn"));

    // Populate, then truncate EVERY entry to a torn shape (primary + mutants).
    let (code1, _c1) = run_check(&fixture, &cache_dir, &HashMap::new());
    assert_eq!(code1, Some(0));
    let keys: Vec<String> = parse_all_entries(&cache_dir).into_keys().collect();
    assert!(!keys.is_empty(), "population produced entries to tear");
    for key in &keys {
        std::fs::write(
            cache_dir.join(format!("{key}.json")),
            b"{ \"item\": \"torn_conc_fn\", \"level\": ",
        )
        .expect("write torn entry");
    }

    // N concurrent agents all hit the torn entry → all degrade to a re-verify.
    let mut handles = Vec::with_capacity(N_AGENTS);
    for _ in 0..N_AGENTS {
        let cache_dir = cache_dir.clone();
        let fixture = fixture.clone();
        handles.push(std::thread::spawn(move || {
            let (code, certs) = run_check(&fixture, &cache_dir, &HashMap::new());
            assert_eq!(
                code,
                Some(0),
                "every agent must re-verify the torn entry to L3, never crash"
            );
            find_cert(&certs, "torn_conc_fn")["level"].clone()
        }));
    }
    for h in handles {
        let level = h.join().expect("torn-concurrent agent must not panic");
        assert_eq!(level, Value::from("L3"), "the re-verified verdict is L3");
    }

    // Converged to consistent, parseable entries (a torn survivor would PANIC in
    // `parse_all_entries`); no orphan temp.
    let entries = parse_all_entries(&cache_dir);
    assert!(
        !entries.is_empty(),
        "the re-verify repopulated the torn cache with consistent entries"
    );
    assert_eq!(count_tmp_siblings(&cache_dir), 0, "no leftover `.tmp`");

    let _ = std::fs::remove_file(&fixture);
    let _ = std::fs::remove_dir_all(&cache_dir);
}
