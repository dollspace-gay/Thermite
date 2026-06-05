//! The LIVE oracle test for forge's STRENGTHENING PROBES (issue #14,
//! `thermite-design.md` §7 step 5). It drives the BUILT `forge` binary (same as
//! `mutation_conformance.rs`) and asserts the emitted certificate against the
//! hand-derived oracle `conformance/strengthening/cases.json` (R-CHAR-3 —
//! expected outcomes trace to §7 / the oracle, never to forge's own output).
//!
//! Strengthening probes issue REAL verus queries per candidate (a suggestion is
//! surfaced only if it VERIFIES against the real body AND kills the #12 survivor),
//! so EVERY case here needs verus. The verus-needing cases SKIP LOUDLY when verus
//! is absent (mirroring `mutation_conformance.rs`), never panic.
//!
//! The oracle is QUALITATIVE (R-CHAR-3 / `.design/forge/strengthening-probes.md`
//! AC anchors): the suggestion set is verus-version-sensitive (oracle-EXCLUDED),
//! so the CHECKABLE properties are presence/absence + adoptability:
//!   - `weak_loose_bound` (AC-1): a weak contract (checked under a LOW
//!     `--mutation-floor` so it reaches the probe as an L3-certified item) emits
//!     ≥1 suggestion whose clause is `result == a + b` (the oracle
//!     `expect_suggestion`) and that records the killed survivor;
//!   - `corpus_sum` (AC-2): the corpus `sum` (already exact-pinned `ens result ==
//!     spec_sum(xs)`) emits NO strengthening suggestion AND still certifies L3 with
//!     its verdict unchanged (the oracle subset unperturbed, AC-4);
//!   - `weak_loose_bound` under the probe leaves the verdict (level/reject)
//!     IDENTICAL to the same fixture checked WITHOUT reaching the probe (AC-4 —
//!     advisory, the probe never changes the verdict);
//!   - determinism (AC-5): the same fn checked twice yields the identical
//!     suggestion set.
//!
//! `forge` is a pure `bin` crate (no `lib.rs`), so the `strengthen`/`manifest`
//! types are not importable here; the assertions read the cert's JSON fields
//! directly. `unwrap`/`expect` are fine here — `tests/` is not anti-pattern-gated.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;
use serde_json::Value;

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("conformance")
}

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

// ---- oracle JSON shapes ----------------------------------------------------

#[derive(Debug, Deserialize)]
struct Oracle {
    has_suggestion: Vec<HasSuggestionCase>,
    no_suggestion: Vec<NoSuggestionCase>,
}

#[derive(Debug, Deserialize)]
struct HasSuggestionCase {
    name: String,
    expect_suggestion: String,
    program: String,
}

#[derive(Debug, Deserialize)]
struct NoSuggestionCase {
    name: String,
    source: String,
}

fn read_oracle() -> Oracle {
    let path = corpus_dir().join("strengthening").join("cases.json");
    let src = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read oracle {}: {e}", path.display()));
    serde_json::from_str(&src).unwrap_or_else(|e| panic!("parse oracle cases.json: {e}"))
}

fn unique() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    N.fetch_add(1, Ordering::Relaxed)
}

/// Write a program string to a unique temp `.th` file (the driver reads a path).
fn write_temp(name: &str, program: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "forge_strengthen_{}_{}_{name}.th",
        std::process::id(),
        unique()
    ));
    std::fs::write(&path, program).unwrap_or_else(|e| panic!("write temp {name}: {e}"));
    path
}

/// A fresh hermetic `FORGE_CACHE_DIR` per run (mirrors `mutation_conformance.rs`).
fn unique_cache_dir() -> PathBuf {
    std::env::temp_dir().join(format!(
        "forge_strengthen_cache_{}_{}",
        std::process::id(),
        unique()
    ))
}

/// Run `forge check <file> --json [extra flags]`, returning (exit_code, certs).
fn run_check_json(file: &Path, extra: &[&str]) -> (Option<i32>, Vec<Value>) {
    let cache_dir = unique_cache_dir();
    let _ = std::fs::remove_dir_all(&cache_dir);
    let mut cmd = Command::new(forge_bin());
    cmd.arg("check").arg(file).arg("--json");
    for a in extra {
        cmd.arg(a);
    }
    let out = cmd
        .env("FORGE_CACHE_DIR", &cache_dir)
        .output()
        .unwrap_or_else(|e| panic!("spawn forge: {e}"));
    let _ = std::fs::remove_dir_all(&cache_dir);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!(
            "forge --json stdout must be one JSON document: {e}\nstdout:\n{stdout}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stderr)
        )
    });
    let arr = value
        .as_array()
        .unwrap_or_else(|| panic!("forge --json must emit a JSON array: {value}"))
        .clone();
    (out.status.code(), arr)
}

/// Find the cert for `item` in a certs array.
fn cert_for<'a>(certs: &'a [Value], item: &str) -> &'a Value {
    certs
        .iter()
        .find(|c| c.get("item").and_then(|i| i.as_str()) == Some(item))
        .unwrap_or_else(|| panic!("no `{item}` cert in {certs:?}"))
}

/// The strengthening suggestion CLAUSE strings on a cert (empty/absent → []).
fn suggestion_clauses(cert: &Value) -> Vec<String> {
    cert.get("strengthening")
        .and_then(|s| s.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|s| s.get("clause").and_then(|c| c.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

// ---- AC-1: a weak contract -> a verifying, strictly-stronger suggestion ------

#[test]
fn weak_contract_emits_verifying_strictly_stronger_suggestion() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — strengthening probes need the per-candidate proofs.");
        return;
    }
    let oracle = read_oracle();
    for case in &oracle.has_suggestion {
        // The weak fixture is GATED `WeakContract` under the default floor (the
        // early-return-0 mutant survives). To reach the §7 step-5 probe it must
        // CERTIFY L3, so check it under a LOW `--mutation-floor` (REQ-5 — the probe
        // runs on an L3-certified + scored item). This is a test LEVER, not a
        // conformance edit (R-CHAR-3 — the oracle is untouched).
        let path = write_temp(&case.name, &case.program);
        let (code, certs) = run_check_json(&path, &["--mutation-floor", "0.0"]);
        let _ = std::fs::remove_file(&path);

        // Under the low floor the weak fixture certifies (exit 0).
        assert_eq!(
            code,
            Some(0),
            "weak `{}` must certify L3 under --mutation-floor 0.0 to reach the probe; certs: {certs:?}",
            case.name
        );
        let cert = certs
            .first()
            .unwrap_or_else(|| panic!("no cert for `{}`", case.name));
        assert_eq!(
            cert.get("level").and_then(|l| l.as_str()),
            Some("L3"),
            "weak `{}` certifies L3 (the probe runs only on a settled L3 item); cert: {cert}",
            case.name
        );

        // AC-1: ≥1 suggestion, and it INCLUDES the oracle `expect_suggestion`
        // (`result == a + b`) — a clause that VERIFIES against the body (the probe
        // only surfaces verifying candidates) and is strictly stronger.
        let clauses = suggestion_clauses(cert);
        assert!(
            !clauses.is_empty(),
            "weak `{}` must emit ≥1 strengthening suggestion; cert: {cert}",
            case.name
        );
        assert!(
            clauses.contains(&case.expect_suggestion),
            "weak `{}` suggestions {clauses:?} must include the oracle `{}`",
            case.name,
            case.expect_suggestion
        );

        // AC-1: the expected suggestion records the killed survivor (the §7
        // strictly-stronger witness / the kill link).
        let suggestions = cert
            .get("strengthening")
            .and_then(|s| s.as_array())
            .unwrap_or_else(|| panic!("`{}` strengthening array", case.name));
        let expected = suggestions
            .iter()
            .find(|s| s.get("clause").and_then(|c| c.as_str()) == Some(&case.expect_suggestion))
            .unwrap_or_else(|| panic!("`{}` has the expected suggestion", case.name));
        let kills = expected
            .get("kills_survivor")
            .and_then(|k| k.as_str())
            .unwrap_or_default();
        assert!(
            kills.contains("return 0"),
            "the `{}` suggestion records the early-return-0 survivor it kills; got `{kills}`",
            case.expect_suggestion
        );
    }
}

// ---- AC-2: the corpus `sum` -> NO suggestion (already exact-pinned) ----------

#[test]
fn corpus_sum_emits_no_suggestion_and_certifies_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — strengthening probes need the per-candidate proofs.");
        return;
    }
    let oracle = read_oracle();
    for case in &oracle.no_suggestion {
        let src = case.source.trim_start_matches("conformance/");
        let path = corpus_dir().join(src);
        let item_name = Path::new(src)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&case.name);

        // The corpus `sum` certifies at the DEFAULT config (it meets the floor).
        let (code, certs) = run_check_json(&path, &[]);
        assert_eq!(
            code,
            Some(0),
            "`{}` ({src}) must certify (exit 0); certs: {certs:?}",
            case.name
        );
        let cert = cert_for(&certs, item_name);

        // AC-2 / AC-4: the verdict is UNCHANGED — still L3, not rejected.
        assert_eq!(
            cert.get("level").and_then(|l| l.as_str()),
            Some("L3"),
            "`{}` certifies L3 (the probe never changes the verdict); cert: {cert}",
            case.name
        );
        assert!(
            cert.get("reject").map(|r| r.is_null()).unwrap_or(true),
            "`{}` is not rejected; cert: {cert}",
            case.name
        );

        // AC-2: NO strengthening suggestion (already exact-pinned).
        let clauses = suggestion_clauses(cert);
        assert!(
            clauses.is_empty(),
            "`{}` is already exact-pinned → NO suggestion; got {clauses:?}",
            case.name
        );
    }
}

// ---- AC-4: ADVISORY — the probe never changes the verdict --------------------

#[test]
fn probe_never_changes_the_verdict() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — strengthening probes need the per-candidate proofs.");
        return;
    }
    let oracle = read_oracle();
    // For the corpus `sum`: its oracle subset (item, level, effects, slag) is the
    // SAME whether or not a probe runs (the probe touches only the additive
    // `strengthening` field). Compare the deterministic verdict fields against the
    // GOLDEN cert (R-CHAR-3 — the anchor is the golden, not forge's output).
    let golden_src = std::fs::read_to_string(corpus_dir().join("sum.cert.json"))
        .expect("read golden sum.cert.json");
    let golden: Value = serde_json::from_str(&golden_src).expect("parse golden");

    let path = corpus_dir().join("sum.th");
    let (_, certs) = run_check_json(&path, &[]);
    let cert = cert_for(&certs, "sum");

    for field in ["item", "level", "effects", "slag"] {
        assert_eq!(
            cert.get(field),
            golden.get(field),
            "the probe must leave the oracle field `{field}` identical to the golden"
        );
    }
    // The probe also never introduces a reject on the certified item.
    assert!(
        cert.get("reject").map(|r| r.is_null()).unwrap_or(true),
        "the probe must not introduce a reject on `sum`"
    );

    // For the weak fixture: the level/reject under the probe (low floor) is the
    // same L3 verdict — the probe added suggestions, not a verdict change.
    for case in &oracle.has_suggestion {
        let path = write_temp(&case.name, &case.program);
        let (code, certs) = run_check_json(&path, &["--mutation-floor", "0.0"]);
        let _ = std::fs::remove_file(&path);
        assert_eq!(
            code,
            Some(0),
            "`{}` certifies under the low floor",
            case.name
        );
        let cert = certs.first().expect("cert");
        assert_eq!(
            cert.get("level").and_then(|l| l.as_str()),
            Some("L3"),
            "the probe leaves `{}` at L3 (advisory, never a verdict change)",
            case.name
        );
        assert!(
            cert.get("reject").map(|r| r.is_null()).unwrap_or(true),
            "the probe introduces no reject on `{}`",
            case.name
        );
    }
}

// ---- AC-5: determinism (same fn -> same suggestions) -------------------------

#[test]
fn suggestions_are_deterministic_across_two_runs() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — strengthening probes need the per-candidate proofs.");
        return;
    }
    let oracle = read_oracle();
    for case in &oracle.has_suggestion {
        let path = write_temp(&case.name, &case.program);
        let (_, certs1) = run_check_json(&path, &["--mutation-floor", "0.0"]);
        let (_, certs2) = run_check_json(&path, &["--mutation-floor", "0.0"]);
        let _ = std::fs::remove_file(&path);
        let s1 = cert_for(&certs1, "f").get("strengthening").cloned();
        let s2 = cert_for(&certs2, "f").get("strengthening").cloned();
        assert_eq!(
            s1, s2,
            "`{}` strengthening suggestions must be byte-identical across runs (REQ-6)",
            case.name
        );
    }
}
