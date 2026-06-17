//! AC-12 (umbrella `.design/thermite2-program.md` REQ-7): the §6 metrics dashboard.
//! Drives the built `forge` binary with `audit --metrics` and asserts that forge emits
//! the routing-reason + verdict + TV-phase telemetry and the audit PRINTS the dashboard
//! — and, critically, that the dashboard GATES NOTHING (#274,
//! `.design/forge/audit-manifest.md` REQ-10): the exit code is byte-for-byte identical
//! with and without `--metrics`, on a certifying project AND on a failing one.
//!
//! Three telemetry kinds, exercised across two fixtures:
//! - the v1 Verus corpus (`conformance/sum.th`) drives the **TV phase split** (the
//!   contract-TV phase runs over its fn/spec-fn clauses) and the **in-cage** routing
//!   share (its clauses carry no forge-tier attribution — the cage by construction);
//! - the forge-tier `conformance/forge/isqrt_class.th` drives the **cage-vs-forge share
//!   by routing reason** (2 nlsat=relaxable + 1 lean=lemma → forge) and the
//!   **seven-verdict counts** (3 `Proved`).
//!
//! The audit runs the check pipeline (which requires verus), so these skip with a logged
//! reason if verus / lake is absent, mirroring `audit_conformance.rs` / `g1_gate.rs`.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("forge crate has a parent workspace dir")
        .to_path_buf()
}

/// `verus` is reachable (the same skip-guard `audit_conformance.rs` uses). z3 ships
/// alongside the verus distribution, so a present verus implies a usable nlsat solver.
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

/// `lake` on PATH / under `~/.elan` — the forge-tier L3 (lemma) clause needs the built
/// Lean spine. Absent → the gate route cannot discharge, so the forge-tier test SKIPS.
fn lake_present() -> bool {
    if let Ok(home) = std::env::var("HOME") {
        if PathBuf::from(home).join(".elan/bin/lake").exists() {
            return true;
        }
    }
    Command::new("lake")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Run `forge audit <file> [extra args]`, returning (exit_code, stdout, stderr).
fn run_audit(file: &Path, extra: &[&str]) -> (Option<i32>, String, String) {
    let mut cmd = Command::new(forge_bin());
    cmd.arg("audit").arg(file);
    for a in extra {
        cmd.arg(a);
    }
    let out = cmd.output().unwrap_or_else(|e| panic!("spawn forge: {e}"));
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

fn write_temp_program(name: &str, program: &str) -> PathBuf {
    let pid = std::process::id();
    let path = std::env::temp_dir().join(format!("metrics_{name}_{pid}.th"));
    std::fs::write(&path, program).unwrap_or_else(|e| panic!("write temp .th: {e}"));
    path
}

// AC-12: `forge audit --metrics` over the v1 corpus PRINTS the §6 dashboard (the TV
// phase split + the in-cage routing share) and GATES NOTHING — the exit code is
// identical to the bare audit, and the metrics are appended after the manifest.
#[test]
fn audit_metrics_prints_dashboard_and_gates_nothing() {
    if !verus_present() {
        eprintln!(
            "SKIP: verus not available — the audit runs the check pipeline, which requires \
             the prover."
        );
        return;
    }
    let file = repo_root().join("conformance/sum.th");

    let (bare_code, bare_stdout, _) = run_audit(&file, &[]);
    let (metrics_code, metrics_stdout, _) = run_audit(&file, &["--metrics"]);

    // GATES NOTHING: the exit code is byte-for-byte identical with and without --metrics.
    assert_eq!(
        bare_code, metrics_code,
        "`forge audit --metrics` must not change the exit code (audit gates nothing, #274)"
    );

    // The dashboard is PRINTED (a new section), appended after the manifest.
    assert!(
        metrics_stdout.contains("§6 metrics dashboard"),
        "the metrics section header must be printed:\n{metrics_stdout}"
    );
    assert!(
        metrics_stdout.contains("routing (cage vs forge, by reason)"),
        "the routing share section must be printed:\n{metrics_stdout}"
    );
    assert!(
        metrics_stdout.contains("verdicts (the seven CertVerdict)"),
        "the seven-verdict section must be printed:\n{metrics_stdout}"
    );
    // The TV phase split is present (the contract-TV phase runs over the v1 corpus).
    assert!(
        metrics_stdout.contains("tv phase split (syntactic / semantic / timeout)"),
        "the TV phase split must be printed for the v1 corpus:\n{metrics_stdout}"
    );
    assert!(
        metrics_stdout.contains("faithful :"),
        "the TV split must report the faithful baseline:\n{metrics_stdout}"
    );

    // The bare audit prints NO metrics section (the flag is opt-in).
    assert!(
        !bare_stdout.contains("§6 metrics dashboard"),
        "the bare `forge audit` must NOT print the metrics section:\n{bare_stdout}"
    );

    // The v1 corpus clauses route to the cage (no forge-tier attribution) and carry no
    // recorded seven-verdict — the cage routing share is the whole, verdicts unattributed.
    assert!(
        metrics_stdout.contains("in-cage [cage]:"),
        "the v1 corpus must show in-cage routing:\n{metrics_stdout}"
    );
}

// AC-12 (determinism, R-CODE-5): the dashboard output is byte-identical across two runs
// (the metrics are a deterministic projection; the TV phase uses the pinned seed/rlimit).
#[test]
fn audit_metrics_output_is_deterministic() {
    if !verus_present() {
        eprintln!("SKIP: verus not available.");
        return;
    }
    let file = repo_root().join("conformance/sum.th");
    let (_c1, s1, _e1) = run_audit(&file, &["--metrics"]);
    let (_c2, s2, _e2) = run_audit(&file, &["--metrics"]);
    let section = |s: &str| {
        s.split("§6 metrics dashboard")
            .nth(1)
            .map(str::to_string)
            .unwrap_or_default()
    };
    assert_eq!(
        section(&s1),
        section(&s2),
        "the §6 metrics section must be deterministic across runs"
    );
    assert!(
        !section(&s1).is_empty(),
        "the metrics section must be present"
    );
}

// AC-12: under `--json` the metrics go to stderr so stdout stays ONE valid v1 manifest
// document (the dashboard never corrupts the oracle-asserted JSON surface).
#[test]
fn audit_metrics_json_keeps_stdout_a_single_manifest() {
    if !verus_present() {
        eprintln!("SKIP: verus not available.");
        return;
    }
    let file = repo_root().join("conformance/sum.th");
    let (_code, stdout, stderr) = run_audit(&file, &["--metrics", "--json"]);
    // stdout is a single JSON manifest (the metrics did NOT leak into it).
    let manifest: Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!("`forge audit --metrics --json` stdout must be one JSON doc: {e}\n{stdout}")
    });
    assert_eq!(
        manifest["manifest_version"], "v1",
        "stdout is the v1 manifest, metrics excluded"
    );
    assert!(
        !stdout.contains("§6 metrics dashboard"),
        "the metrics must NOT appear on the JSON stdout:\n{stdout}"
    );
    // The dashboard is on stderr instead.
    assert!(
        stderr.contains("§6 metrics dashboard"),
        "under --json the dashboard goes to stderr:\n{stderr}"
    );
}

/// Run `forge check --engine forge <file> --json`, returning (exit_code, certs array).
fn run_check_forge_json(file: &Path) -> (Option<i32>, Vec<Value>) {
    let out = Command::new(forge_bin())
        .arg("check")
        .arg("--engine")
        .arg("forge")
        .arg(file)
        .arg("--json")
        .output()
        .unwrap_or_else(|e| panic!("spawn forge: {e}"));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!(
            "forge --json must emit one JSON document: {e}\nstdout:\n{stdout}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stderr)
        )
    });
    let arr = value
        .as_array()
        .unwrap_or_else(|| panic!("forge check --json must emit a JSON array of certs: {value}"))
        .clone();
    (out.status.code(), arr)
}

// AC-12 ("forge emits the routing-reason + verdict telemetry fields the §6 dashboard
// needs"): the forge-tier `isqrt_class` example, checked with `--engine forge`, EMITS the
// per-clause telemetry the metrics dashboard projects into the cage-vs-forge share by
// reason + the seven-verdict counts — 2 nlsat (relaxable → forge) + 1 lean (lemma →
// forge) clauses, all `Proved`. (The dashboard's aggregation of these fields into the
// routing share + verdict counts is unit-tested in `metrics::tests`; this pins the
// binary-level emission of the fields it consumes.)
#[test]
fn forge_engine_emits_routing_and_verdict_telemetry() {
    if !verus_present() || !lake_present() {
        eprintln!(
            "SKIP: verus (z3) and/or lake absent — the forge-tier per-clause hybrid route is \
             not run (set VERUS_BIN + build the Lean spine; the CI lean job is the gate)."
        );
        return;
    }
    let file = repo_root().join("conformance/forge/isqrt_class.th");
    let (code, certs) = run_check_forge_json(&file);
    assert_eq!(code, Some(0), "the forge-tier example certifies (exit 0)");
    assert_eq!(certs.len(), 1, "one fn, one certificate");
    let obls = certs[0]["obligations"]
        .as_array()
        .expect("the cert carries per-clause obligations");
    assert_eq!(obls.len(), 3, "three ens clauses, three obligations");

    // The ROUTING telemetry: the per-clause `engine` field (which the dashboard projects
    // to in-cage / relaxable / lemma). Two nlsat (relaxable) + one lean (lemma).
    let engines: Vec<&str> = obls
        .iter()
        .map(|o| o["engine"].as_str().expect("each clause carries an engine"))
        .collect();
    assert_eq!(
        engines.iter().filter(|e| **e == "nlsat").count(),
        2,
        "two relaxable (nlsat) clauses: {engines:?}"
    );
    assert_eq!(
        engines.iter().filter(|e| e.starts_with("lean")).count(),
        1,
        "one lemma (lean) clause: {engines:?}"
    );

    // The VERDICT telemetry: the per-clause seven-verdict — all three `Proved`.
    for (k, o) in obls.iter().enumerate() {
        assert_eq!(
            o["verdict"]["kind"],
            Value::from("Proved"),
            "clause ens#{k} carries the Proved verdict"
        );
    }
}

// AC-12: the dashboard gates nothing on a FAILING project too — a non-certifying fn
// exits with the verification-failure code, and `--metrics` does not change it (the
// metrics are a read-only projection, never a gate).
#[test]
fn audit_metrics_gates_nothing_on_failing_project() {
    if !verus_present() {
        eprintln!("SKIP: verus not available.");
        return;
    }
    // `bad` claims `result > n` but returns `n` (result == n) — a counterexample, so the
    // project does not certify and the audit exits non-zero.
    let file = write_temp_program(
        "failing",
        "fn bad(n: u64) -> u64\n  req true\n  ens result > n\n  fx pure\n{ n }\n",
    );

    let (bare_code, _bs, _be) = run_audit(&file, &[]);
    let (metrics_code, metrics_stdout, _me) = run_audit(&file, &["--metrics"]);
    let _ = std::fs::remove_file(&file);

    assert_ne!(
        bare_code,
        Some(0),
        "the failing project must exit non-zero (a counterexample does not certify)"
    );
    assert_eq!(
        bare_code, metrics_code,
        "`--metrics` must not change the exit code on a failing project (gates nothing)"
    );
    assert!(
        metrics_stdout.contains("§6 metrics dashboard"),
        "the dashboard is still printed for a failing project:\n{metrics_stdout}"
    );
}
