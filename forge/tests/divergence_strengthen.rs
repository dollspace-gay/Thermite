//! ADVERSARIAL audit of forge's STRENGTHENING PROBES (issue #14, §7 step 5,
//! `.design/forge/strengthening-probes.md`). The builder's
//! `strengthening_conformance.rs` checks that the EXPECTED clause (`result == a +
//! b`) is present and kills the survivor; it does NOT independently re-verify
//! EVERY surfaced suggestion across the template families. This file closes the
//! R-DEFER-1 adoptability gap:
//!
//! - **probe #2 (the key one) — every surfaced suggestion is ADOPTABLE.** REQ-2 /
//!   `goal.md` R-DEFER-1: "a suggestion must be a REAL adoptable clause". The §7
//!   step-5 promise is "if a strictly stronger contract PROVES with no body
//!   change, Forge suggests it". So: for each fixture, take EVERY clause the probe
//!   surfaces, ADOPT it (paste `ens <clause>` into the SAME body + SAME req/fx),
//!   and re-check through forge's OWN lowering+verus pipeline. Each MUST certify
//!   L3 — that is the adoptability invariant. A surfaced clause that does NOT
//!   certify L3 when adopted is VAPORWARE (the agent would adopt an unprovable
//!   contract) → divergence.
//!
//! The fixtures exercise template family 2 across operators (`a - b` under a
//! `b <= a` precondition; a bare parameter `a`) — both surface a suggestion, so
//! they are real adoptability witnesses (verified live below). The expected value
//! (L3 on adoption) traces to REQ-2 + the §7 step-5 adoptability invariant, NOT to
//! forge's own output (R-CHAR-3).
//!
//! REAL verus queries; SKIP LOUDLY when verus is absent (mirrors
//! `strengthening_conformance.rs`), never panic. `unwrap`/`expect` are fine in
//! `tests/` (not anti-pattern-gated).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// `true` iff verus is reachable (mirrors `strengthening_conformance.rs`).
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
        "forge_divergence_strengthen_{}_{}_{name}.th",
        std::process::id(),
        unique()
    ));
    std::fs::write(&path, program).unwrap_or_else(|e| panic!("write temp {name}: {e}"));
    path
}

fn unique_cache_dir() -> PathBuf {
    std::env::temp_dir().join(format!(
        "forge_divergence_strengthen_cache_{}_{}",
        std::process::id(),
        unique()
    ))
}

/// Run `forge check <file> --json [extra]`, returning (exit_code, certs).
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

/// For a single-`fn` program (`name` = the fn's name, with header `head` =
/// "fn NAME(...) -> T req ... fx ..." and `body`), check it under a low floor so
/// it reaches the probe, return the surfaced suggestion clauses.
fn surfaced_suggestions(label: &str, program: &str, item: &str) -> (Option<i32>, Vec<String>) {
    let path = write_temp(label, program);
    let (code, certs) = run_check_json(&path, &["--mutation-floor", "0.0"]);
    let _ = std::fs::remove_file(&path);
    let cert = certs
        .iter()
        .find(|c| c.get("item").and_then(|i| i.as_str()) == Some(item));
    let clauses = cert.map(suggestion_clauses).unwrap_or_default();
    (code, clauses)
}

/// PROBE #2 (R-DEFER-1, REQ-2): EVERY surfaced suggestion must be ADOPTABLE.
///
/// For each fixture that surfaces ≥1 suggestion, ADOPT each surfaced clause
/// (replace the loose `ens` with `ens <clause>` over the SAME body + req/fx) and
/// re-check through forge's OWN pipeline; the adopted contract MUST certify L3
/// (§7 step-5 "proves with no body change" / R-DEFER-1). A surfaced clause that
/// does NOT certify L3 on adoption is vaporware → divergence.
#[test]
fn every_surfaced_suggestion_is_adoptable_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — adoptability re-verify needs the per-clause proofs.");
        return;
    }

    // (label, fn name, req + fx prefix, return type, body). The `ens` is loose
    // (`result <= N`) so the probe runs; each fixture is a different family-2 shape.
    struct Fixture {
        label: &'static str,
        item: &'static str,
        header: &'static str, // up to and including `fx pure`, with a `{BODY}` tail appended
        body: &'static str,
    }
    let fixtures = [
        Fixture {
            label: "sub",
            item: "f",
            header:
                "fn f(a: u32, b: u32) -> u32 req b <= a && a <= 10 ens result <= 1000000 fx pure",
            body: "a - b",
        },
        Fixture {
            label: "bare",
            item: "f",
            header:
                "fn f(a: u32, b: u32) -> u32 req a <= 10 && b <= 10 ens result <= 1000000 fx pure",
            body: "a",
        },
        Fixture {
            label: "addtwo",
            item: "f",
            header:
                "fn f(a: u32, b: u32) -> u32 req a <= 10 && b <= 10 ens result <= 1000000 fx pure",
            body: "a + b",
        },
    ];

    let mut surfaced_any = false;
    let mut unadoptable: Vec<(String, String, Option<String>, Option<i32>)> = Vec::new();

    for fx in &fixtures {
        let program = format!("{} {{ {} }}", fx.header, fx.body);
        let (code, clauses) = surfaced_suggestions(fx.label, &program, fx.item);
        assert_eq!(
            code,
            Some(0),
            "fixture `{}` must certify L3 under --mutation-floor 0.0 to reach the probe",
            fx.label
        );
        for clause in &clauses {
            surfaced_any = true;
            // ADOPT: replace the loose `ens` with the surfaced clause, same body.
            // The header up to `ens` is reused; we splice the clause in.
            let adopted = fx
                .header
                .replacen("ens result <= 1000000", &format!("ens {clause}"), 1);
            let adopted = format!("{adopted} {{ {} }}", fx.body);
            let apath = write_temp(&format!("{}_adopt", fx.label), &adopted);
            let (acode, acerts) = run_check_json(&apath, &["--mutation-floor", "0.0"]);
            let _ = std::fs::remove_file(&apath);
            let alevel = acerts
                .first()
                .and_then(|c| c.get("level").and_then(|l| l.as_str()).map(String::from));
            if acode != Some(0) || alevel.as_deref() != Some("L3") {
                unadoptable.push((fx.label.to_string(), clause.clone(), alevel, acode));
            }
        }
    }

    assert!(
        surfaced_any,
        "no fixture surfaced any suggestion — the adoptability invariant is untested; \
         (this fixture set is known to surface suggestions live)"
    );
    assert!(
        unadoptable.is_empty(),
        "R-DEFER-1 / §7 step-5: every surfaced suggestion must be ADOPTABLE (certify L3 \
         when pasted as the `ens` over the same body). These surfaced suggestions are \
         VAPORWARE (did not certify L3 on adoption): {unadoptable:?}"
    );
}
