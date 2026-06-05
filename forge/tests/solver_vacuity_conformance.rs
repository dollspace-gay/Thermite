//! The LIVE oracle test for forge's SOLVER-backed tautology + vacuous-precondition
//! checks (issue #13, `thermite-design.md` §7 steps 2-3). It drives the BUILT
//! `forge` binary (`.design/forge/cli.md` Verification — same as
//! `vacuity_slag_conformance.rs`) and asserts the emitted certificate against the
//! hand-derived oracle `conformance/solver-vacuity/cases.json` (R-CHAR-3 — expected
//! verdicts trace to the oracle, never to forge's own output).
//!
//! These checks issue REAL verus queries (the harness must PROVE for a detection),
//! so EVERY case here needs verus. The verus-needing cases SKIP LOUDLY when verus
//! is absent (mirroring `lower_conformance.rs` / `vacuity_slag_conformance.rs`),
//! never panic.
//!
//! `forge` is a pure `bin` crate (no `lib.rs`), so the `SolverVacuityCause` enum is
//! not importable here; instead the cert's `reject.cause` field carries the cause
//! tag (`vacuity_solver::SolverVacuityCause::tag`) and the oracle's `"cause"`
//! string is compared against it directly — a faithful "map cause string -> cause"
//! without weakening the assertion (the same shape `vacuity_slag_conformance.rs`
//! and `combinators_conformance.rs` use).
//!
//! AC-4 (the #6-passes-but-#13-catches value-add): each reject fixture PASSES #6's
//! structural triage (it would not carry a #6 syntactic cause) yet #13 rejects it
//! with the SOLVER cause — the two stages disagree exactly on these fixtures, which
//! is the proof #13 adds detection power over #6. The reject oracle's distinct
//! `"SemanticTautology"` / `"VacuousPrecondition"` tag namespace (not #6's
//! `"EnsIsTrivial"` etc.) is the discriminator the assertion keys on.
//!
//! `unwrap`/`expect` are fine here — `tests/` is not anti-pattern-gated.

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

/// `true` iff verus is reachable (mirrors `vacuity_slag_conformance.rs`).
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
    reject: Vec<RejectCase>,
    accept: Vec<AcceptCase>,
}

#[derive(Debug, Deserialize)]
struct RejectCase {
    name: String,
    /// The SOLVER cause tag (`"SemanticTautology"` / `"VacuousPrecondition"`).
    cause: String,
    /// The `contract_quality` bool the detection sets `true` (`"tautology"` /
    /// `"vacuous_precondition"`).
    field: String,
    program: String,
}

#[derive(Debug, Deserialize)]
struct AcceptCase {
    name: String,
    source: String,
}

fn read_oracle() -> Oracle {
    let path = corpus_dir().join("solver-vacuity").join("cases.json");
    let src = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read oracle {}: {e}", path.display()));
    serde_json::from_str(&src).unwrap_or_else(|e| panic!("parse oracle cases.json: {e}"))
}

/// Write a program string to a unique temp `.th` file (the driver reads a path).
fn write_temp(name: &str, program: &str) -> PathBuf {
    let dir = std::env::temp_dir();
    let path = dir.join(format!(
        "forge_solvervac_{}_{}_{name}.th",
        std::process::id(),
        unique()
    ));
    std::fs::write(&path, program).unwrap_or_else(|e| panic!("write temp {name}: {e}"));
    path
}

fn unique() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    N.fetch_add(1, Ordering::Relaxed)
}

/// A unique, per-call temp proof-cache dir so this test is HERMETIC and
/// order-independent (mirrors `cache_conformance.rs`): the #13 verdict is cached
/// with the item, so a shared `target/` cache would let one case's stored cert
/// (or a stale entry from a prior toolchain) leak into another. Each
/// `run_check_json` gets its own empty cache dir via `FORGE_CACHE_DIR`.
fn unique_cache_dir() -> PathBuf {
    std::env::temp_dir().join(format!(
        "forge_solvervac_cache_{}_{}",
        std::process::id(),
        unique()
    ))
}

/// Run `forge check <file> --json`, returning (exit_code, certs array). Runs with
/// a fresh hermetic `FORGE_CACHE_DIR` so no cache entry is shared across cases.
fn run_check_json(file: &Path) -> (Option<i32>, Vec<Value>) {
    let cache_dir = unique_cache_dir();
    let _ = std::fs::remove_dir_all(&cache_dir);
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(file)
        .arg("--json")
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

fn first_cert(certs: &[Value]) -> &Value {
    certs
        .first()
        .unwrap_or_else(|| panic!("no certificate emitted"))
}

// ---- the #6-passes-but-#13-catches value-add (AC-4) ------------------------

/// Each reject fixture PASSES #6's structural triage — i.e. it would NOT carry a
/// #6 syntactic cause; only the #13 SOLVER stage rejects it. The reject test below
/// asserts the SOLVER cause (`SemanticTautology`/`VacuousPrecondition`), which is a
/// DISTINCT tag namespace from #6's syntactic causes (`EnsIsTrivial` etc.). So a
/// reject carrying a SOLVER cause IS the proof #6 passed and #13 caught it (AC-4):
/// if #6 had rejected, the cause would be a syntactic tag, not a solver tag.
const SOLVER_CAUSES: [&str; 2] = ["SemanticTautology", "VacuousPrecondition"];

// ---- reject: TAUTOLOGY / VACUOUS-PRECONDITION detected (AC-2 / AC-3 / AC-5) -

#[test]
fn solver_rejects_match_oracle_cause_and_field() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — #13 reject detection needs the harness proof.");
        return;
    }
    let oracle = read_oracle();
    for case in &oracle.reject {
        // The cause must be in the SOLVER namespace (AC-4: distinct from #6).
        assert!(
            SOLVER_CAUSES.contains(&case.cause.as_str()),
            "reject `{}` oracle cause `{}` must be a SOLVER cause (the AC-4 value-add)",
            case.name,
            case.cause
        );

        let path = write_temp(&case.name, &case.program);
        let (code, certs) = run_check_json(&path);
        let _ = std::fs::remove_file(&path);

        // A SOLVER-vacuity reject is a reported contract-certification FAILURE:
        // non-zero exit, a valid cert document, NOT certified (no L3/L1).
        assert_eq!(
            code,
            Some(1),
            "solver-vacuity reject `{}` must exit with the verification-failure code; certs: {certs:?}",
            case.name
        );
        let cert = first_cert(&certs);

        // AC-5: the cert names the SOLVER cause (R-CHAR-3 — the oracle tag).
        let got_cause = cert
            .get("reject")
            .and_then(|r| r.get("cause"))
            .and_then(|c| c.as_str());
        assert_eq!(
            got_cause,
            Some(case.cause.as_str()),
            "reject `{}` must carry the oracle cause `{}`; cert: {cert}",
            case.name,
            case.cause
        );

        // AC-2/AC-3 + AC-5: the matching contract_quality bool is SOLVER-confirmed
        // `true` (the field the oracle names), and the OTHER stays false.
        let cq = cert
            .get("contract_quality")
            .expect("contract_quality block");
        assert_eq!(
            cq.get(&case.field),
            Some(&Value::from(true)),
            "reject `{}` must set contract_quality.{} = true; cert: {cert}",
            case.name,
            case.field
        );
        let other = if case.field == "tautology" {
            "vacuous_precondition"
        } else {
            "tautology"
        };
        assert_eq!(
            cq.get(other),
            Some(&Value::from(false)),
            "reject `{}` must leave contract_quality.{other} = false; cert: {cert}",
            case.name
        );

        // The rejected item never certifies L3 (nor L1) — the §7 "does not
        // certify until its contract certifies", verdict-in-cert.
        let level = cert.get("level").and_then(|l| l.as_str());
        assert_ne!(level, Some("L3"), "`{}` must not certify L3", case.name);
        assert_ne!(level, Some("L1"), "`{}` must not certify L1", case.name);
    }
}

// ---- accept: the corpus passes BOTH checks, still L3 (AC-1) -----------------

#[test]
fn corpus_accepts_pass_both_checks_and_still_certify_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — corpus #13 clean + L3 not run.");
        return;
    }
    let oracle = read_oracle();
    for case in &oracle.accept {
        let src = case.source.trim_start_matches("conformance/");
        let path = corpus_dir().join(src);
        let (code, certs) = run_check_json(&path);

        // The corpus item passes BOTH SOLVER checks (verus FAILS to prove either
        // harness) AND its real L3 proof — it certifies (exit 0).
        assert_eq!(
            code,
            Some(0),
            "corpus `{}` ({src}) must still certify (exit 0); certs: {certs:?}",
            case.name
        );
        let item_name = Path::new(src)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&case.name);
        let cert = certs
            .iter()
            .find(|c| c.get("item").and_then(|i| i.as_str()) == Some(item_name))
            .unwrap_or_else(|| panic!("no `{item_name}` cert in {certs:?}"));

        // AC-1: still L3, not rejected.
        assert_eq!(
            cert.get("level").and_then(|l| l.as_str()),
            Some("L3"),
            "corpus `{}` must still certify L3; cert: {cert}",
            case.name
        );
        assert!(
            cert.get("reject").map(|r| r.is_null()).unwrap_or(true),
            "corpus `{}` must NOT be rejected by #13; cert: {cert}",
            case.name
        );

        // AC-1: both contract_quality bools are SOLVER-confirmed `false` (verus
        // could prove NEITHER harness). Expected `false` traces to the design's §7
        // (a non-tautological ens / a satisfiable req), not to forge's output.
        let cq = cert
            .get("contract_quality")
            .expect("contract_quality block");
        assert_eq!(
            cq.get("tautology"),
            Some(&Value::from(false)),
            "corpus `{}` tautology must be solver-confirmed false; cert: {cert}",
            case.name
        );
        assert_eq!(
            cq.get("vacuous_precondition"),
            Some(&Value::from(false)),
            "corpus `{}` vacuous_precondition must be solver-confirmed false; cert: {cert}",
            case.name
        );
    }
}

// ---- the sum.cert.json golden oracle still passes (AC-1, no regress) --------

#[test]
fn corpus_sum_still_matches_golden() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — sum golden oracle not run.");
        return;
    }
    let (code, certs) = run_check_json(&corpus_dir().join("sum.th"));
    assert_eq!(code, Some(0), "sum must still certify (exit 0): {certs:?}");
    let sum = certs
        .iter()
        .find(|c| c.get("item").and_then(|i| i.as_str()) == Some("sum"))
        .unwrap_or_else(|| panic!("no sum cert: {certs:?}"));

    // The golden deterministic subset (item/level/effects/slag) still matches, and
    // the two §7.1 contract_quality bools (now SOLVER-confirmed) match the golden's
    // hand-derived false (R-CHAR-3 — anchored to `conformance/sum.cert.json`).
    let golden_src =
        std::fs::read_to_string(corpus_dir().join("sum.cert.json")).expect("read golden sum cert");
    let golden: Value = serde_json::from_str(&golden_src).expect("parse golden");
    assert_eq!(sum["item"], golden["item"]);
    assert_eq!(sum["level"], Value::from("L3"), "sum must still verify L3");
    assert_eq!(sum["level"], golden["level"]);
    assert_eq!(sum["effects"], golden["effects"]);
    assert_eq!(sum["slag"], golden["slag"]);
    assert_eq!(
        sum["contract_quality"]["tautology"],
        golden["contract_quality"]["tautology"]
    );
    assert_eq!(sum["contract_quality"]["tautology"], Value::from(false));
    assert_eq!(
        sum["contract_quality"]["vacuous_precondition"],
        golden["contract_quality"]["vacuous_precondition"]
    );
    assert_eq!(
        sum["contract_quality"]["vacuous_precondition"],
        Value::from(false)
    );
}
