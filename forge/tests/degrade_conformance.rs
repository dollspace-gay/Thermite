//! Conformance for the automatic L3→L2→L1 degrade ladder + the assurance manifest
//! (issue #10, `.design/forge/degrade-ladder.md`). The DETERMINISTIC ladder state
//! machine + the OQ-2 split + the min-over-functions aggregate are pinned
//! HERMETICALLY by the unit tests (`degrade.rs` drives `run_ladder` on synthesized
//! `L3Verdict`/`L2Attempt`; `kani.rs` drives `classify_l2_outcome`; `manifest.rs`'s
//! `AssuranceManifest::aggregate`). This file is the LIVE end-to-end layer through
//! the built `forge` binary:
//!
//! - **AC-1 (no-degrade):** the corpus `sum`/`binary_search` at the DEFAULT rlimit
//!   certify every fn at L3, project assurance L3, NO `lowered_assurance` flag.
//! - **AC-4 (THE ANTI-CHEAT AC):** a live broken-contract fixture (a provably-false
//!   `ens`) is a HARD FAIL — non-certifying, NEVER a degraded L1/L2 cert, NO
//!   `lowered_assurance` flag. The hermetic determinism of this is pinned by
//!   `degrade::tests::counterexample_never_degrades`; this asserts it end-to-end
//!   against the REAL verus.
//! - **AC-2 (forced degrade → L2):** a forced low `--rlimit` is the L3-timeout
//!   lever; BEST-EFFORT skip-loud (OQ-1: provoking a live resourceout is
//!   timing-fragile). WHEN a live degrade IS provoked, the cert MUST be a certified
//!   lower rung with `lowered_assurance: true` + a degrade reason.
//!
//! Verus/kani-dependent checks SKIP LOUDLY when the binary is absent (mirroring
//! `profile_conformance.rs` / `l2_check.rs`). `tests/` is not anti-pattern-gated,
//! so `unwrap`/`expect`/`panic!` are fine here. Do NOT edit `conformance/`
//! (R-CHAR-3): expected levels trace to the design doc's grounding (the corpus
//! proves L3; a false `ens` is a counterexample), never to forge's own output.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("conformance")
}

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// `true` iff verus can be located (mirrors `profile_conformance.rs`). SKIP
/// LOUDLY otherwise.
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

/// `true` iff `cargo-kani` is resolvable (mirrors `l2_check.rs`).
fn kani_present() -> bool {
    if let Ok(out) = Command::new("which").arg("cargo-kani").output() {
        if out.status.success() && !out.stdout.is_empty() {
            return true;
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        if PathBuf::from(home).join(".cargo/bin/cargo-kani").exists() {
            return true;
        }
    }
    false
}

/// Run `forge check <file> [extra...] --json`, returning (exit_code, cert array).
fn run_check_json(file: &Path, extra: &[&str]) -> (Option<i32>, Vec<Value>) {
    let mut cmd = Command::new(forge_bin());
    cmd.arg("check").arg(file);
    for a in extra {
        cmd.arg(a);
    }
    cmd.arg("--json");
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
        .unwrap_or_else(|| panic!("forge --json must emit a JSON array: {value}"))
        .clone();
    (out.status.code(), arr)
}

fn find_cert(certs: &[Value], item: &str) -> Value {
    certs
        .iter()
        .find(|c| c.get("item").and_then(|v| v.as_str()) == Some(item))
        .unwrap_or_else(|| panic!("no certificate for `{item}` in {certs:?}"))
        .clone()
}

fn is_lowered(cert: &Value) -> bool {
    cert.get("lowered_assurance")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

// ===== AC-1: corpus at the DEFAULT rlimit → all L3, no degrade ==============

// AC-1: `forge check conformance/sum.th` at the DEFAULT rlimit certifies BOTH
// `spec_sum` and `sum` at L3, NO `lowered_assurance` flag, NO `degrade_reason`,
// exit 0 (the ladder runs no degrade rung). The project assurance is L3 (the min).
// Expected: the corpus PROVES L3 at the generous default budget (degrade-ladder.md
// grounding "Corpus at the DEFAULT rlimit → L3, no degrade"), never forge's output.
#[test]
fn corpus_sum_no_degrade_all_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — corpus no-degrade (AC-1) not run.");
        return;
    }
    let (code, certs) = run_check_json(&corpus_dir().join("sum.th"), &[]);
    assert_eq!(
        code,
        Some(0),
        "sum proves L3 at the default rlimit (no degrade)"
    );
    for item in ["spec_sum", "sum"] {
        let cert = find_cert(&certs, item);
        assert_eq!(
            cert["level"],
            Value::from("L3"),
            "{item} is L3 at the default rlimit (no degrade): {cert}"
        );
        assert!(
            !is_lowered(&cert),
            "a non-degraded L3 cert carries NO lowered_assurance flag: {cert}"
        );
        assert!(
            cert.get("degrade_reason").is_none(),
            "a non-degraded cert carries NO degrade_reason: {cert}"
        );
    }
}

// AC-1: `binary_search` likewise stays L3 with no degrade at the default rlimit.
#[test]
fn corpus_binary_search_no_degrade_all_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — corpus no-degrade (AC-1) not run.");
        return;
    }
    let (code, certs) = run_check_json(&corpus_dir().join("binary_search.th"), &[]);
    assert_eq!(
        code,
        Some(0),
        "binary_search proves L3 at the default rlimit"
    );
    let bs = find_cert(&certs, "binary_search");
    assert_eq!(bs["level"], Value::from("L3"), "binary_search is L3: {bs}");
    assert!(!is_lowered(&bs), "no lowered_assurance flag: {bs}");
}

// AC-1: the corpus golden `conformance/sum.cert.json` (which omits the #10 fields)
// still deserializes — the additive `lowered_assurance` / `degrade_reason` fields
// default `false`/absent (AC-6, R-SPEC-2). Pure (no verus): asserts the frozen
// oracle is unperturbed by the additive schema. Expected: the golden cert is L3
// with no degrade fields (the doc's AC-6), not forge's output.
#[test]
fn golden_cert_deserializes_with_additive_degrade_fields() {
    let golden_path = corpus_dir().join("sum.cert.json");
    let golden_src = std::fs::read_to_string(&golden_path).expect("read golden cert");
    let golden: Value = serde_json::from_str(&golden_src).expect("parse golden cert");
    // The golden frozen cert OMITS the #10 degrade fields (it predates #10).
    assert!(
        golden.get("lowered_assurance").is_none(),
        "the frozen golden cert omits lowered_assurance (additive default false)"
    );
    assert!(
        golden.get("degrade_reason").is_none(),
        "the frozen golden cert omits degrade_reason (additive)"
    );
    assert_eq!(golden["level"], Value::from("L3"));
}

// ===== AC-4: THE KEY ANTI-CHEAT AC — a counterexample NEVER degrades =========

// AC-4 (the highest-value test): a broken contract (a provably-FALSE `ens` for the
// body) is a HARD FAILURE end-to-end against the REAL verus — NON-zero exit,
// non-certifying (NOT L3/L2/L1), NO `lowered_assurance` flag, NO `degrade_reason`.
// The ladder must NEVER "certify L1" or degrade a disproved contract: that would
// hide a real bug behind a lowered-assurance stamp (§12, R-DEFER-9). The hermetic
// determinism of the short-circuit is pinned by
// `degrade::tests::counterexample_never_degrades`; this is the LIVE end-to-end
// witness against real verus. Expected: a false `ens` is a counterexample, not a
// degrade (degrade-ladder.md REQ-2 / "The anti-cheat distinction is real"), never
// forge's output.
#[test]
fn live_broken_contract_is_hard_fail_never_degraded() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — the live anti-cheat AC (AC-4) not run.");
        return;
    }
    let fixture =
        std::env::temp_dir().join(format!("forge_degrade_broken_{}.th", std::process::id()));
    // `ens result == x + 2` for a body returning `x + 1` — verus DISPROVES it.
    std::fs::write(
        &fixture,
        "fn add_one(x: u64) -> u64\n  req x < 1000\n  ens result == x + 2\n  fx  pure\n{\n  x + 1\n}\n",
    )
    .expect("write broken fixture");
    let (code, certs) = run_check_json(&fixture, &[]);
    let _ = std::fs::remove_file(&fixture);

    assert_eq!(
        code,
        Some(1),
        "a disproved contract exits with the verification-failure code (a project FAILURE)"
    );
    let cert = find_cert(&certs, "add_one");
    // NEVER a certified rung — a counterexample is non-certifying L0.
    assert_ne!(
        cert["level"],
        Value::from("L3"),
        "a false ens does not prove L3: {cert}"
    );
    assert_ne!(
        cert["level"],
        Value::from("L2"),
        "a counterexample is NEVER degraded to L2 (anti-cheat REQ-2): {cert}"
    );
    assert_ne!(
        cert["level"],
        Value::from("L1"),
        "a counterexample is NEVER degraded to L1 (anti-cheat REQ-2): {cert}"
    );
    // NEVER a lowered-assurance degrade stamp.
    assert!(
        !is_lowered(&cert),
        "a counterexample carries NO lowered_assurance flag — it is a FAILURE, not a degrade: {cert}"
    );
    assert!(
        cert.get("degrade_reason").is_none(),
        "a hard-failed counterexample carries NO degrade_reason: {cert}"
    );
    // It carries a per-obligation failure witness (the #5 counterexample path).
    let obs = cert["obligations"].as_array().expect("obligations present");
    assert!(
        obs.iter()
            .any(|o| o.get("status").and_then(|s| s.as_str()) == Some("failed")),
        "the counterexample cert carries a failed obligation witness: {cert}"
    );
}

// AC-7 / REQ-7 (determinism): two runs of the broken fixture yield the SAME
// achieved level + the SAME (non-)degrade verdict (the achieved level is
// deterministic given the pinned budget). The degrade reason content is
// oracle-excluded; we assert only the level + the lowered_assurance flag.
#[test]
fn broken_fixture_verdict_is_deterministic() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — determinism (AC-7) not run.");
        return;
    }
    let fixture = std::env::temp_dir().join(format!("forge_degrade_det_{}.th", std::process::id()));
    std::fs::write(
        &fixture,
        "fn add_one(x: u64) -> u64\n  req x < 1000\n  ens result == x + 2\n  fx  pure\n{\n  x + 1\n}\n",
    )
    .expect("write fixture");
    let (c1, certs1) = run_check_json(&fixture, &[]);
    let (c2, certs2) = run_check_json(&fixture, &[]);
    let _ = std::fs::remove_file(&fixture);
    assert_eq!(c1, c2, "the exit code is deterministic");
    let a = find_cert(&certs1, "add_one");
    let b = find_cert(&certs2, "add_one");
    assert_eq!(
        a["level"], b["level"],
        "the achieved level is deterministic"
    );
    assert_eq!(
        is_lowered(&a),
        is_lowered(&b),
        "the (non-)degrade verdict is deterministic"
    );
}

// ===== AC-2: the forced L3-timeout → L2 degrade (LIVE, best-effort) =========

// AC-2: a forced low `--rlimit` is the L3-timeout LEVER. BEST-EFFORT (OQ-1,
// inherited from #11): provoking a live resourceout is timing-fragile — verus
// often returns `unknown` FAST without a `--profile` report. WHEN a live degrade IS
// provoked (the item L3-times-out AND its L2 harness verifies), the cert MUST be a
// CERTIFIED lower rung (L2 or L1) carrying `lowered_assurance: true` + a
// `degrade_reason`. WHEN no live timeout is provoked, the item stays L3 (no
// degrade) — also valid. NEVER a non-certifying cert with lowered_assurance (that
// would be degrading falsity, REQ-2). The DETERMINISTIC ladder is pinned
// hermetically by `degrade::tests::timeout_then_l2_verified_certifies_l2_degraded`.
#[test]
fn forced_low_rlimit_degrade_is_certified_lower_rung_when_provoked() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — forced-degrade lever (AC-2) not run.");
        return;
    }
    // The corpus proves even at `--rlimit 1` (degrade-ladder.md OQ-1), so a degrade
    // is NOT expected to fire; this test asserts the INVARIANT either way: any cert
    // that IS lowered_assurance must be a CERTIFIED lower rung, never a hard fail.
    let (_code, certs) = run_check_json(&corpus_dir().join("sum.th"), &["--rlimit", "1"]);
    let mut saw_degrade = false;
    for cert in &certs {
        if is_lowered(cert) {
            saw_degrade = true;
            let level = cert["level"].as_str().unwrap_or("");
            assert!(
                level == "L2" || level == "L1",
                "a lowered_assurance cert MUST be a certified lower rung (L2/L1), NEVER a hard \
                 fail — degrading falsity is the anti-cheat violation (REQ-2): {cert}"
            );
            assert!(
                cert.get("degrade_reason").is_some(),
                "a degraded cert carries the degrade reason (REQ-4): {cert}"
            );
            // A degrade requires the kani L2 rung; if kani is absent the run would
            // have errored (REQ-8), so reaching a degraded cert means kani ran.
            assert!(
                kani_present(),
                "a live L2 degrade was produced, so kani must have been available"
            );
        }
    }
    if !saw_degrade {
        eprintln!(
            "SKIP (OQ-1): `--rlimit 1` on sum did not provoke a live L3 timeout (the corpus \
             proves even at a tiny budget — the documented timing-fragility). The DETERMINISTIC \
             L3-timeout→L2 degrade is pinned hermetically by \
             `degrade::tests::timeout_then_l2_verified_certifies_l2_degraded`."
        );
    }
}

// AC-2 (human display): a no-degrade corpus run prints the project assurance
// headline `project assurance: L3` and NO `lowered-assurance:` lines (§5.2
// "displayed on every build"). Asserts the cli render path end-to-end.
#[test]
fn human_output_shows_project_assurance_headline() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — assurance headline display not run.");
        return;
    }
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(corpus_dir().join("sum.th"))
        .output()
        .expect("spawn forge");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("project assurance: L3"),
        "the human output shows the project assurance headline (the min, L3 for the corpus):\n{stdout}"
    );
    assert!(
        !stdout.contains("lowered-assurance:"),
        "a no-degrade build shows NO lowered-assurance lines (AC-1):\n{stdout}"
    );
}
