//! The LIVE oracle test for forge's MUTATION SCORING (issue #12,
//! `thermite-design.md` §7 step 4). It drives the BUILT `forge` binary (same as
//! `solver_vacuity_conformance.rs`) and asserts the emitted certificate against
//! the hand-derived oracle `conformance/mutation/cases.json` (R-CHAR-3 — expected
//! outcomes trace to §7, never to forge's own output).
//!
//! Mutation scoring issues REAL verus queries per mutant (the kill ratio is a
//! function of which mutants verus PROVES vs REJECTS), so EVERY case here needs
//! verus. The verus-needing cases SKIP LOUDLY when verus is absent (mirroring
//! `solver_vacuity_conformance.rs` / `lower_conformance.rs`), never panic.
//!
//! The oracle is QUALITATIVE (R-CHAR-3 / `.design/forge/mutation-scoring.md`
//! REQ-8 / OQ-1): the exact `mutants_killed` ratio is tool-computed and
//! verus-version-sensitive, so it is oracle-EXCLUDED. The CHECKABLE properties
//! are:
//!   - `accept_above_floor` (AC-1): a STRONG contract kills enough mutants that
//!     `kill_ratio >= floor` → the item certifies L3 and RECORDS a `"K/N"` with
//!     `K/N >= 0.60` (the FLOOR relation, not a frozen exact count);
//!   - `reject_below_floor` (AC-2): a WEAK-but-non-vacuous contract lets enough
//!     mutants survive that `kill_ratio < floor` → the item is GATED
//!     (`RejectReason { cause: "WeakContract" }`) with a non-`None` `survivor`;
//!   - the floor is CONFIGURABLE (AC-3): the same weak fixture certifies under a
//!     LOW `--mutation-floor` and is gated under the default 0.60;
//!   - the kill ratio is DETERMINISTIC (AC-4): scoring the same fixture twice
//!     yields the byte-identical `mutants_killed` + `survivor` (a run==run
//!     property, NOT a fabricated golden — R-CHAR-3-clean).
//!
//! `forge` is a pure `bin` crate (no `lib.rs`), so the `mutation`/`manifest` types
//! are not importable here; the assertions read the cert's JSON fields directly
//! (the same shape `solver_vacuity_conformance.rs` uses). `unwrap`/`expect` are
//! fine here — `tests/` is not anti-pattern-gated.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;
use serde_json::Value;

/// The §7 default kill-ratio floor (`.design/forge/mutation-scoring.md` REQ-5,
/// `MUTATION_FLOOR`). Hand-derived from the design (R-CHAR-3), not read from forge.
const FLOOR: f64 = 0.60;

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("conformance")
}

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// `true` iff verus is reachable (mirrors `solver_vacuity_conformance.rs`).
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
    accept_above_floor: Vec<AcceptCase>,
    reject_below_floor: Vec<RejectCase>,
}

#[derive(Debug, Deserialize)]
struct AcceptCase {
    name: String,
    source: String,
}

#[derive(Debug, Deserialize)]
struct RejectCase {
    name: String,
    cause: String,
    program: String,
}

fn read_oracle() -> Oracle {
    let path = corpus_dir().join("mutation").join("cases.json");
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
        "forge_mutation_{}_{}_{name}.th",
        std::process::id(),
        unique()
    ));
    std::fs::write(&path, program).unwrap_or_else(|e| panic!("write temp {name}: {e}"));
    path
}

/// A fresh hermetic `FORGE_CACHE_DIR` per run so no cache entry is shared across
/// cases (mirrors `solver_vacuity_conformance.rs`).
fn unique_cache_dir() -> PathBuf {
    std::env::temp_dir().join(format!(
        "forge_mutation_cache_{}_{}",
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

/// Parse a `"K/N"` mutants_killed string into (killed, scored).
fn parse_ratio(mk: &str) -> (u64, u64) {
    let (k, n) = mk
        .split_once('/')
        .unwrap_or_else(|| panic!("mutants_killed `{mk}` is not `K/N`"));
    (
        k.parse().unwrap_or_else(|_| panic!("bad killed in `{mk}`")),
        n.parse().unwrap_or_else(|_| panic!("bad scored in `{mk}`")),
    )
}

// ---- AC-1: strong corpus contracts -> kill_ratio >= floor -> certify L3 -----

#[test]
fn accept_fixtures_score_at_or_above_floor_and_certify_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — mutation scoring needs the per-mutant proofs.");
        return;
    }
    let oracle = read_oracle();
    for case in &oracle.accept_above_floor {
        let src = case.source.trim_start_matches("conformance/");
        let path = corpus_dir().join(src);
        let item_name = Path::new(src)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&case.name);

        let (code, certs) = run_check_json(&path, &[]);
        assert_eq!(
            code,
            Some(0),
            "accept `{}` ({src}) must certify (exit 0); certs: {certs:?}",
            case.name
        );
        let cert = cert_for(&certs, item_name);

        // AC-1: still L3, not rejected.
        assert_eq!(
            cert.get("level").and_then(|l| l.as_str()),
            Some("L3"),
            "accept `{}` must certify L3; cert: {cert}",
            case.name
        );
        assert!(
            cert.get("reject").map(|r| r.is_null()).unwrap_or(true),
            "accept `{}` must NOT be rejected; cert: {cert}",
            case.name
        );

        // AC-1: `mutants_killed` is RECORDED (graduated from "0/0") and its ratio is
        // >= the floor (the threshold relation, NOT a frozen exact count — REQ-8).
        let mk = cert
            .get("contract_quality")
            .and_then(|q| q.get("mutants_killed"))
            .and_then(|m| m.as_str())
            .unwrap_or_else(|| panic!("accept `{}` cert missing mutants_killed", case.name));
        let (killed, scored) = parse_ratio(mk);
        assert!(
            scored > 0,
            "accept `{}` must score mutants (not `0/0`); got `{mk}`",
            case.name
        );
        let ratio = killed as f64 / scored as f64;
        assert!(
            ratio >= FLOOR,
            "accept `{}` kill ratio {mk} (={ratio}) must be >= the floor {FLOOR}; cert: {cert}",
            case.name
        );
    }
}

// ---- AC-2: a weak-but-non-vacuous contract -> kill_ratio < floor -> gated ----

#[test]
fn reject_fixture_scores_below_floor_and_is_gated_weak_contract() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — mutation scoring needs the per-mutant proofs.");
        return;
    }
    let oracle = read_oracle();
    for case in &oracle.reject_below_floor {
        assert_eq!(
            case.cause, "WeakContract",
            "the reject oracle cause is the §7 step-4 `WeakContract` tag"
        );
        let path = write_temp(&case.name, &case.program);
        let (code, certs) = run_check_json(&path, &[]);
        let _ = std::fs::remove_file(&path);

        // A WeakContract reject is a reported contract-certification FAILURE:
        // non-zero exit, a valid cert document, NOT certified.
        assert_eq!(
            code,
            Some(1),
            "reject `{}` must exit with the verification-failure code; certs: {certs:?}",
            case.name
        );
        let cert = certs
            .first()
            .unwrap_or_else(|| panic!("no cert for `{}`", case.name));

        // AC-2: the cause is `WeakContract` and the item does not certify L3/L1.
        assert_eq!(
            cert.get("reject")
                .and_then(|r| r.get("cause"))
                .and_then(|c| c.as_str()),
            Some("WeakContract"),
            "reject `{}` must carry cause WeakContract; cert: {cert}",
            case.name
        );
        let level = cert.get("level").and_then(|l| l.as_str());
        assert_ne!(level, Some("L3"), "`{}` must not certify L3", case.name);
        assert_ne!(level, Some("L1"), "`{}` must not certify L1", case.name);

        // AC-2: the kill ratio is recorded and BELOW the floor; a survivor is named.
        let cq = cert
            .get("contract_quality")
            .expect("contract_quality block");
        let mk = cq
            .get("mutants_killed")
            .and_then(|m| m.as_str())
            .unwrap_or_else(|| panic!("reject `{}` missing mutants_killed", case.name));
        let (killed, scored) = parse_ratio(mk);
        assert!(
            scored > 0,
            "reject `{}` must score mutants; got `{mk}`",
            case.name
        );
        let ratio = killed as f64 / scored as f64;
        assert!(
            ratio < FLOOR,
            "reject `{}` kill ratio {mk} (={ratio}) must be < the floor {FLOOR} \
             (if it is not, the fixture needs weakening — report to the orchestrator, \
             do NOT edit conformance/); cert: {cert}",
            case.name
        );
        let survivor = cq.get("survivor").and_then(|s| s.as_str());
        assert!(
            survivor.is_some() && !survivor.unwrap().is_empty(),
            "reject `{}` must name a surviving mutant (the strengthening prompt); cert: {cert}",
            case.name
        );
    }
}

// ---- AC-3: the floor is the gate, configurable ------------------------------

#[test]
fn floor_is_configurable_weak_fixture_certifies_under_low_floor() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — mutation scoring needs the per-mutant proofs.");
        return;
    }
    let oracle = read_oracle();
    for case in &oracle.reject_below_floor {
        let path = write_temp(&case.name, &case.program);

        // Under the DEFAULT floor (0.60) the weak fixture is GATED (exit 1).
        let (code_default, _) = run_check_json(&path, &[]);
        assert_eq!(
            code_default,
            Some(1),
            "reject `{}` must be gated under the default floor",
            case.name
        );

        // Under a LOW floor (0.0) the SAME fixture certifies (exit 0) — the verdict
        // flips on the floor (REQ-5 / AC-3). 0.0 is below any non-zero kill ratio.
        let (code_low, certs_low) = run_check_json(&path, &["--mutation-floor", "0.0"]);
        let _ = std::fs::remove_file(&path);
        assert_eq!(
            code_low,
            Some(0),
            "reject `{}` must certify under `--mutation-floor 0.0`; certs: {certs_low:?}",
            case.name
        );
        let cert = certs_low
            .first()
            .unwrap_or_else(|| panic!("no cert for `{}`", case.name));
        assert_eq!(
            cert.get("level").and_then(|l| l.as_str()),
            Some("L3"),
            "reject `{}` under a low floor certifies L3; cert: {cert}",
            case.name
        );
        assert!(
            cert.get("reject").map(|r| r.is_null()).unwrap_or(true),
            "reject `{}` under a low floor is not gated; cert: {cert}",
            case.name
        );
    }
}

// ---- AC-4: the kill ratio is DETERMINISTIC (run == run) ---------------------

#[test]
fn kill_ratio_is_deterministic_across_two_runs() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — mutation scoring needs the per-mutant proofs.");
        return;
    }
    // Use the corpus `sum` (a stable parse/lower fixture). The asserted relation is
    // run1 == run2 (a determinism PROPERTY), never against a fabricated constant
    // (R-CHAR-3-clean).
    let path = corpus_dir().join("sum.th");
    let (_, certs1) = run_check_json(&path, &[]);
    let (_, certs2) = run_check_json(&path, &[]);
    let mk1 = cert_for(&certs1, "sum")["contract_quality"]["mutants_killed"].clone();
    let mk2 = cert_for(&certs2, "sum")["contract_quality"]["mutants_killed"].clone();
    assert_eq!(
        mk1, mk2,
        "the same fixture scored twice must yield the identical mutants_killed (frozen \
         set + seed + toolchain — REQ-8/AC-4)"
    );
    let s1 = cert_for(&certs1, "sum")["contract_quality"]
        .get("survivor")
        .cloned();
    let s2 = cert_for(&certs2, "sum")["contract_quality"]
        .get("survivor")
        .cloned();
    assert_eq!(s1, s2, "the survivor (if any) is deterministic too");
}
