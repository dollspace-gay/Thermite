//! The LIVE oracle test for forge's §7.1 structural vacuity triage + the
//! `#[slag]` escape hatch (issue #6). It drives the BUILT `forge` binary
//! (`.design/forge/cli.md` Verification — same as `check_conformance.rs`) and
//! asserts the emitted certificate against the hand-derived oracles
//! `conformance/vacuity/triage.json` + `conformance/slag/slag.json` (R-CHAR-3 —
//! expected verdicts trace to the oracle, never to forge's output).
//!
//! `forge` is a pure `bin` crate (no `lib.rs`), so the structured verdict enum is
//! not importable here; instead the cert's `reject.cause` field carries the §7.1
//! verdict variant tag (`vacuity::VacuityCause::tag` / `slag::SlagError::tag`),
//! and the oracle's `"cause"` string is compared against it directly — a faithful
//! "map cause string -> verdict variant" without weakening the assertion.
//!
//! verus is needed ONLY for the non-slag triage `accept` cases (the L3 path) and
//! the corpus. Every `reject` (triage short-circuits before verus) and every slag
//! case (L1 by fiat / rejected before verus) runs WITHOUT verus. The verus-needing
//! cases SKIP LOUDLY when verus is absent (mirroring `lower_conformance.rs`'s
//! Option-resolve), never panic.
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

/// `true` iff verus is reachable (mirrors `check_conformance.rs`).
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
struct TriageOracle {
    accept: Vec<TriageAccept>,
    reject: Vec<RejectCase>,
}

#[derive(Debug, Deserialize)]
struct TriageAccept {
    name: String,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    program: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SlagOracle {
    accept: Vec<SlagAccept>,
    reject: Vec<RejectCase>,
}

#[derive(Debug, Deserialize)]
struct SlagAccept {
    name: String,
    expect_level: String,
    expect_slag: bool,
    program: String,
}

#[derive(Debug, Deserialize)]
struct RejectCase {
    name: String,
    cause: String,
    program: String,
}

fn read_oracle<T: for<'de> Deserialize<'de>>(rel: &str) -> T {
    let path = corpus_dir().join(rel);
    let src = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read oracle {}: {e}", path.display()));
    serde_json::from_str(&src).unwrap_or_else(|e| panic!("parse oracle {rel}: {e}"))
}

/// Write a program string to a unique temp `.th` file (the driver reads a path).
fn write_temp(name: &str, program: &str) -> PathBuf {
    let dir = std::env::temp_dir();
    let path = dir.join(format!(
        "forge_vacslag_{}_{}_{name}.th",
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

/// Run `forge check <file> --json`, returning (exit_code, certs array).
fn run_check_json(file: &Path) -> (Option<i32>, Vec<Value>) {
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(file)
        .arg("--json")
        .output()
        .unwrap_or_else(|e| panic!("spawn forge: {e}"));
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

// ---- triage rejects (no verus needed — short-circuit before the proof) -----

#[test]
fn triage_rejects_match_oracle_cause() {
    let oracle: TriageOracle = read_oracle("vacuity/triage.json");
    for case in &oracle.reject {
        let path = write_temp(&case.name, &case.program);
        let (code, certs) = run_check_json(&path);
        let _ = std::fs::remove_file(&path);

        // A triage reject is a reported contract-certification FAILURE: non-zero
        // exit, a valid cert document, NOT certified (no L3/L1).
        assert_eq!(
            code,
            Some(1),
            "triage reject `{}` must exit with the verification-failure code",
            case.name
        );
        let cert = first_cert(&certs);
        let got_cause = cert
            .get("reject")
            .and_then(|r| r.get("cause"))
            .and_then(|c| c.as_str());
        assert_eq!(
            got_cause,
            Some(case.cause.as_str()),
            "triage reject `{}` must carry the oracle cause `{}`; cert: {cert}",
            case.name,
            case.cause
        );
        // A rejected item never certifies L3 (nor L1).
        let level = cert.get("level").and_then(|l| l.as_str());
        assert_ne!(level, Some("L3"), "`{}` must not certify L3", case.name);
        assert_ne!(level, Some("L1"), "`{}` must not certify L1", case.name);
    }
}

// ---- triage accepts --------------------------------------------------------

#[test]
fn triage_accepts_pass_triage() {
    let oracle: TriageOracle = read_oracle("vacuity/triage.json");
    let need_verus = !verus_present();
    for case in &oracle.accept {
        // Resolve the program: either an inline `program` or a corpus `source`.
        let (path, is_temp) = if let Some(prog) = &case.program {
            (write_temp(&case.name, prog), true)
        } else if let Some(src) = &case.source {
            (
                corpus_dir().join(src.trim_start_matches("conformance/")),
                false,
            )
        } else {
            panic!("accept case `{}` has neither program nor source", case.name);
        };

        // An accept that passes triage proceeds to the L3 verus path, so it needs
        // verus. SKIP LOUDLY if absent. NOTE: the oracle property for a triage
        // `accept` is "PASSES triage (is NON-vacuous)", NOT "certifies L3" — e.g.
        // `unit_omits_result_ok`'s `ens x > 0` passes triage but is NOT provable
        // for all x (the body returns `()`), so its verus level is L0. The
        // assertion is therefore: NO `reject` cause (triage passed) + the two
        // §7.1 `contract_quality` bools graduated to live-`false` (which a triage
        // PASS sets regardless of the proof outcome, vacuity-triage.md AC-7). The
        // corpus L3 cases (`source`) keep their L3 path, asserted in the dedicated
        // corpus tests.
        if need_verus {
            eprintln!(
                "SKIP: verus absent — `{}` (a triage-accept) needs the L3 path to confirm \
                 it was NOT rejected.",
                case.name
            );
            if is_temp {
                let _ = std::fs::remove_file(&path);
            }
            continue;
        }

        let (_code, certs) = run_check_json(&path);
        if is_temp {
            let _ = std::fs::remove_file(&path);
        }
        // Every emitted cert passed triage: none carries a `reject` cause.
        for cert in &certs {
            assert!(
                cert.get("reject").map(|r| r.is_null()).unwrap_or(true),
                "accept `{}` must PASS triage (no reject cause): {cert}",
                case.name
            );
            // The two §7.1 contract_quality bools are now LIVE-`false` on a triage
            // pass (vacuity-triage.md AC-7); present in the full --json cert.
            let cq = cert.get("contract_quality");
            assert_eq!(
                cq.and_then(|c| c.get("tautology")),
                Some(&Value::from(false)),
                "accept `{}` contract_quality.tautology must be live-false",
                case.name
            );
            assert_eq!(
                cq.and_then(|c| c.get("vacuous_precondition")),
                Some(&Value::from(false)),
                "accept `{}` contract_quality.vacuous_precondition must be live-false",
                case.name
            );
        }
    }
}

// ---- slag accepts (L1 by fiat — no verus needed) ---------------------------

#[test]
fn slag_accepts_certify_l1_slag_true() {
    let oracle: SlagOracle = read_oracle("slag/slag.json");
    for case in &oracle.accept {
        let path = write_temp(&case.name, &case.program);
        let (code, certs) = run_check_json(&path);
        let _ = std::fs::remove_file(&path);

        // A valid slag item certifies L1 by fiat — no verus, exit 0.
        assert_eq!(
            code,
            Some(0),
            "valid slag `{}` certifies (exit 0): {certs:?}",
            case.name
        );
        let cert = first_cert(&certs);
        assert_eq!(
            cert.get("level").and_then(|l| l.as_str()),
            Some(case.expect_level.as_str()),
            "slag `{}` must certify level {}",
            case.name,
            case.expect_level
        );
        assert_eq!(
            cert.get("slag").and_then(|s| s.as_bool()),
            Some(case.expect_slag),
            "slag `{}` must carry slag={}",
            case.name,
            case.expect_slag
        );
        // The audit metadata is carried (§8 visibility, slag.md REQ-4).
        assert!(
            cert.get("slag_meta").map(|m| !m.is_null()).unwrap_or(false),
            "slag `{}` must carry slag_meta: {cert}",
            case.name
        );
    }
}

// ---- slag rejects (invalid fields / vacuous contract — no verus needed) ----

#[test]
fn slag_rejects_match_oracle_cause() {
    let oracle: SlagOracle = read_oracle("slag/slag.json");
    for case in &oracle.reject {
        let path = write_temp(&case.name, &case.program);
        let (code, certs) = run_check_json(&path);
        let _ = std::fs::remove_file(&path);

        assert_eq!(
            code,
            Some(1),
            "slag reject `{}` must exit with the verification-failure code",
            case.name
        );
        let cert = first_cert(&certs);
        let got_cause = cert
            .get("reject")
            .and_then(|r| r.get("cause"))
            .and_then(|c| c.as_str());
        assert_eq!(
            got_cause,
            Some(case.cause.as_str()),
            "slag reject `{}` must carry the oracle cause `{}`; cert: {cert}",
            case.name,
            case.cause
        );
        // A rejected slag item does NOT certify L1.
        assert_ne!(
            cert.get("level").and_then(|l| l.as_str()),
            Some("L1"),
            "rejected slag `{}` must not certify L1",
            case.name
        );
    }
}

// ---- the corpus still certifies L3 + matches the golden (no regress) -------

#[test]
fn corpus_sum_still_l3_and_matches_golden() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — corpus sum L3 oracle not run.");
        return;
    }
    let (code, certs) = run_check_json(&corpus_dir().join("sum.th"));
    assert_eq!(code, Some(0), "sum must still certify (exit 0)");
    let sum = certs
        .iter()
        .find(|c| c.get("item").and_then(|i| i.as_str()) == Some("sum"))
        .unwrap_or_else(|| panic!("no sum cert: {certs:?}"));

    // The golden deterministic subset (item/level/effects/slag) still matches.
    let golden_src =
        std::fs::read_to_string(corpus_dir().join("sum.cert.json")).expect("read golden sum cert");
    let golden: Value = serde_json::from_str(&golden_src).expect("parse golden");
    assert_eq!(sum["item"], golden["item"]);
    assert_eq!(sum["level"], Value::from("L3"), "sum must still verify L3");
    assert_eq!(sum["level"], golden["level"]);
    assert_eq!(sum["effects"], golden["effects"]);
    assert_eq!(sum["slag"], golden["slag"]);
    assert_eq!(sum["slag"], Value::from(false));
    // sum is not slag and not rejected.
    assert!(sum.get("reject").map(|r| r.is_null()).unwrap_or(true));
    // #6-live: the two §7.1 contract_quality bools are now asserted-false AND
    // match the golden's hand-derived false (the golden carries them; the existing
    // check_conformance oracle compares only the deterministic subset, so this is
    // the new live assertion for these two fields — AC-7).
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

#[test]
fn corpus_binary_search_still_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — corpus binary_search L3 not run.");
        return;
    }
    let (code, certs) = run_check_json(&corpus_dir().join("binary_search.th"));
    assert_eq!(code, Some(0), "binary_search must still certify (exit 0)");
    let bs = certs
        .iter()
        .find(|c| c.get("item").and_then(|i| i.as_str()) == Some("binary_search"))
        .unwrap_or_else(|| panic!("no binary_search cert: {certs:?}"));
    assert_eq!(
        bs["level"],
        Value::from("L3"),
        "binary_search must verify L3"
    );
    assert!(bs.get("reject").map(|r| r.is_null()).unwrap_or(true));
}
