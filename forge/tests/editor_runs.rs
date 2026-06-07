//! THE PROOF-OF-THE-PUDDING (crosslink #83 / #88): a VERIFIED text editor that
//! RUNS. This integration test grounds `examples/editor/editor.th` end-to-end
//! against the EXTERNAL truths the toolchain does not author for itself — the real
//! `verus` SMT prover (the cert levels) and the real `rustc` compiler + a real
//! process run (the build + the piped-keystroke session).
//!
//! It pins the editor's TWO deliverables and the #88 diverge-L1 honesty gate:
//!
//!   * `forge check editor.th` — the VERIFIED EDIT CORE (`Buffer`, `insert_str`,
//!     `backspace`, `move_left`, `move_right`) certifies **L3** (total + mutation
//!     proven); the terminal boundary (`read_key`/`key_str`/`render`) is **L1
//!     boundary**; the event loop `run` (`fx diverge`) is **L1** = partial
//!     correctness (the #88 cap — NOT L0 `WeakContract`).
//!   * `forge build editor.th --entry run` — COMPILES (the #88 L1-lowering fixes:
//!     ens-after-move snapshot + empty-literal → `TString`); the produced binary
//!     RUNS with piped keystrokes (`h`, `i`, Ctrl-Q) → edits, renders, exits clean.
//!
//! And the diverge cap's HONESTY (it is diverge-ONLY, not a Goodhart bypass —
//! `goal.md` R-DEFER-9):
//!
//!   * a NON-diverge weak-contract fn STILL rejects at L0 `WeakContract` (the §7
//!     mutation gate still bites a non-diverge weak contract);
//!   * a NORMAL loop fn WITHOUT a `dec` (no `diverge`) STILL fails Verus
//!     termination (the #87 exemption stays diverge-only);
//!   * `conformance/sum.th` / `binary_search.th` STILL certify L3 (the diverge gate
//!     never fires for a total fn — the corpus oracle is unperturbed).
//!
//! Driving the BUILT `forge` binary (not a library API) keeps `forge` a pure `bin`
//! crate and exercises the real CLI surface. The cert-level checks RUN VERUS; if
//! verus is absent they SKIP LOUDLY (the `check_conformance.rs` precedent) — never
//! panic on a missing solver. `tests/` is not anti-pattern-gated, so `unwrap`/
//! `expect`/`panic!` are fine here (R-APG-2). Expected levels trace to the design
//! (`.design/forge/check.md` AC-7 / `degrade-ladder.md` AC-8) + the provers'
//! output, NEVER copied from forge's own output (R-CHAR-3).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn editor_th() -> PathBuf {
    repo_root().join("examples/editor/editor.th")
}

fn conformance_dir() -> PathBuf {
    repo_root().join("conformance")
}

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// `true` iff verus can be located (mirrors `check_conformance.rs`). SKIP LOUDLY
/// otherwise — a missing solver is never a test failure (R-CODE-4).
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

/// Run `forge check <file> --json`, returning the parsed JSON cert array.
fn run_check_json(file: &Path) -> (Option<i32>, Vec<Value>) {
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(file)
        .arg("--json")
        .output()
        .unwrap_or_else(|e| panic!("spawn forge check: {e}"));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!(
            "forge check --json stdout must be one JSON document: {e}\nstdout:\n{stdout}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stderr)
        )
    });
    let arr = value
        .as_array()
        .unwrap_or_else(|| panic!("forge check --json must emit a JSON array of certs: {value}"))
        .clone();
    (out.status.code(), arr)
}

fn find_cert(certs: &[Value], item: &str) -> Value {
    certs
        .iter()
        .find(|c| c.get("item").and_then(|v| v.as_str()) == Some(item))
        .unwrap_or_else(|| panic!("no certificate for item `{item}` in {certs:#?}"))
        .clone()
}

fn level_of(certs: &[Value], item: &str) -> String {
    find_cert(certs, item)["level"]
        .as_str()
        .unwrap_or_else(|| panic!("cert for `{item}` has no string level"))
        .to_string()
}

/// Run `forge build <args...>` and return `(exit_success, stdout, stderr)`.
fn run_forge_build(args: &[&str]) -> (bool, String, String) {
    let out = Command::new(forge_bin())
        .arg("build")
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("spawn forge build: {e}"));
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

fn artifact_path_from_json(stdout: &str) -> PathBuf {
    let v: Value = serde_json::from_str(stdout)
        .unwrap_or_else(|e| panic!("forge build --json not JSON: {e}\n{stdout}"));
    let p = v["artifact"]
        .as_str()
        .unwrap_or_else(|| panic!("no `artifact` field in build manifest:\n{stdout}"));
    PathBuf::from(p)
}

/// Write a throwaway `.th` fixture under the temp dir.
fn write_fixture(name: &str, body: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "forge_editor_test_{name}_{}.th",
        std::process::id()
    ));
    std::fs::write(&path, body).unwrap_or_else(|e| panic!("write fixture {name}: {e}"));
    path
}

// ----------------------------------------------------------------------------
// Deliverable 1 — `forge check editor.th`: edit core L3, boundary L1, run L1.
// (.design/forge/check.md AC-7(a); degrade-ladder.md AC-8.)
// ----------------------------------------------------------------------------

#[test]
fn editor_edit_core_certifies_l3_and_run_caps_at_l1() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — editor cert-oracle not run.");
        return;
    }
    let (code, certs) = run_check_json(&editor_th());
    assert_eq!(
        code,
        Some(0),
        "a fully-certifying editor (edit core L3 + boundary/run L1) exits 0; certs:\n{certs:#?}"
    );

    // The VERIFIED EDIT CORE — every total edit op is L3 (cursor math + length
    // deltas PROVEN, the §7 mutation gate passed). The expected level is the
    // design's claim (.design/forge/check.md AC-7), grounded by real verus.
    for op in [
        "Buffer",
        "insert_str",
        "backspace",
        "move_left",
        "move_right",
    ] {
        assert_eq!(
            level_of(&certs, op),
            "L3",
            "the verified edit core op `{op}` must certify L3"
        );
    }

    // The TRUSTED terminal boundary — L1, boundary:true (foreign body unproven).
    for prim in ["read_key", "key_str", "render"] {
        let cert = find_cert(&certs, prim);
        assert_eq!(cert["level"], Value::from("L1"), "{prim} is an L1 boundary");
        assert_eq!(
            cert["boundary"],
            Value::from(true),
            "{prim} is a `#[boundary]` fn"
        );
    }

    // THE #88 CAP — `run` (fx diverge) is L1 = PARTIAL correctness: NOT L0
    // `WeakContract`, NOT a forced L3, and NOT a boundary fn. No reject, no
    // strengthening suggestion (the §7 mutation/strengthen gate is SKIPPED).
    let run = find_cert(&certs, "run");
    assert_eq!(
        run["level"],
        Value::from("L1"),
        "the diverge event loop `run` caps at L1 (partial correctness), NOT L0:\n{run:#?}"
    );
    assert_eq!(
        run["boundary"],
        Value::from(false),
        "`run` is an in-language diverge fn, NOT a boundary"
    );
    assert!(
        run.get("reject").map(|r| r.is_null()).unwrap_or(true),
        "the diverge cap is NOT a reject (no `WeakContract`):\n{run:#?}"
    );
    let strengthening = run
        .get("strengthening")
        .and_then(|s| s.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    assert_eq!(
        strengthening, 0,
        "the §7 strengthen gate is SKIPPED for a diverge fn:\n{run:#?}"
    );

    // The diverge effect row is present (the cap is keyed on it, §4.1).
    let effects: Vec<String> = run["effects"]
        .as_array()
        .expect("effects array")
        .iter()
        .map(|v| v.as_str().unwrap_or_default().to_string())
        .collect();
    assert!(
        effects.iter().any(|e| e == "diverge"),
        "`run` declares `fx diverge`: {effects:?}"
    );
}

// ----------------------------------------------------------------------------
// Deliverable 2 — `forge build editor.th --entry run`: COMPILES + RUNS.
// (#88 blockers 2+3: ens-after-move snapshot + empty-literal → TString.)
// ----------------------------------------------------------------------------

#[test]
fn editor_builds_and_runs_with_piped_keystrokes() {
    // rustc is always present (no skip; the build_conformance.rs precedent). This
    // is THE proof: a verified editor that runs.
    let editor = editor_th();
    let (ok, stdout, stderr) =
        run_forge_build(&[editor.to_str().unwrap(), "--entry", "run", "--json"]);
    assert!(
        ok,
        "forge build editor.th --entry run must COMPILE (blockers 2+3 fixed):\n\
         stdout:\n{stdout}\nstderr:\n{stderr}"
    );

    let artifact = artifact_path_from_json(&stdout);
    assert!(
        artifact.exists(),
        "the built editor binary must exist at {}",
        artifact.display()
    );

    // RUN it with piped keystrokes: 'h', 'i', then Ctrl-Q (byte 0x11 = 17 = quit).
    // The editor inserts 'h' and 'i' (the L3-proven `insert_str`), renders the
    // buffer after each keystroke, sees Ctrl-Q, and exits clean.
    let mut child = Command::new(&artifact)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("spawn built editor `{}`: {e}", artifact.display()));
    child
        .stdin
        .as_mut()
        .expect("editor stdin")
        .write_all(b"hi\x11")
        .expect("pipe keystrokes to editor");
    let out = child.wait_with_output().expect("editor run completes");

    assert!(
        out.status.success(),
        "the editor must exit CLEAN (exit 0) on Ctrl-Q:\nstatus:{:?}\nstdout:{}\nstderr:{}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );

    // The rendered output must reflect the edits: after 'h' the buffer renders
    // "h", after 'i' it renders "hi". So the buffer text "hi" appears in the
    // rendered stream (the proven edit core ran). We assert the buffer reached
    // "hi" (the substring), not an exact transcript (the render cadence is the
    // host glue's, not the verified core's).
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("hi"),
        "the editor must render the edited buffer `hi` (the L3 `insert_str` ran):\n\
         stdout:{stdout}\nstderr:{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// ----------------------------------------------------------------------------
// #88 honesty — the diverge cap is DIVERGE-ONLY (not a Goodhart bypass).
// (.design/forge/check.md AC-7(b)(c)(d); degrade-ladder.md AC-8; R-DEFER-9.)
// ----------------------------------------------------------------------------

#[test]
fn non_diverge_weak_contract_still_rejects_l0_weakcontract() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — non-diverge weak-contract regression not run.");
        return;
    }
    // The AC-7(b) fixture: a TOTAL `fx pure` fn with a LOOSE `ens` and NO
    // `diverge`. The §7 mutation gate must STILL bite it (a `return 0`-style mutant
    // survives the loose `ens result <= 1000000`), rejecting at L0 `WeakContract` —
    // the diverge exemption is NOT a mutation escape hatch for a normal fn.
    let fixture = write_fixture(
        "weak_total",
        "fn f(a: u32, b: u32) -> u32\n  \
           req a <= 10 && b <= 10\n  \
           ens result <= 1000000\n  \
           fx  pure\n{\n  a + b\n}\n",
    );
    let (_code, certs) = run_check_json(&fixture);
    let cert = find_cert(&certs, "f");
    assert_eq!(
        cert["level"],
        Value::from("L0"),
        "a NON-diverge weak contract must STILL reject at L0 (the gate still bites):\n{cert:#?}"
    );
    let cause = cert
        .get("reject")
        .and_then(|r| r.get("cause"))
        .and_then(|c| c.as_str())
        .unwrap_or("");
    assert_eq!(
        cause, "WeakContract",
        "a non-diverge weak contract rejects specifically as WeakContract:\n{cert:#?}"
    );
    let _ = std::fs::remove_file(&fixture);
}

#[test]
fn normal_loop_without_dec_still_fails_termination() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — termination-exemption regression not run.");
        return;
    }
    // The AC-7(c) fixture: a NORMAL (non-diverge) fn with a `while` loop whose
    // `dec` measure does NOT strictly decrease (`dec n`, constant across the loop —
    // a loop's `dec` is mandatory syntactically, so "no dec" is a parse error, not
    // a verus outcome; a NON-decreasing dec is the faithful "termination unproven"
    // shape). Verus must STILL DEMAND a strictly-decreasing measure and FAIL — the
    // #87 termination exemption (`#[verifier::exec_allows_no_decreases_clause]`,
    // which SUPPRESSES the decreases obligation) is diverge-ONLY, and the #88
    // diverge L1 cap does not relax it for any other fn. (No `diverge` in the row →
    // the gate routes it to the normal L3 path, where the non-decreasing `dec` is a
    // verus termination failure, NOT a diverge cap.)
    let fixture = write_fixture(
        "loop_bad_dec",
        "fn spin(n: u32) -> u32\n  \
           req true\n  \
           ens result <= 1\n  \
           fx  pure\n{\n  \
             let mut i: u32 = 0;\n  \
             while i < n\n    \
               inv i <= n\n    \
               dec n\n  \
             {\n    i = i + 1;\n  }\n  \
             0\n}\n",
    );
    let (_code, certs) = run_check_json(&fixture);
    let cert = find_cert(&certs, "spin");
    assert_ne!(
        cert["level"],
        Value::from("L1"),
        "a non-diverge loop with a non-decreasing `dec` must NOT get the diverge L1 cap:\n{cert:#?}"
    );
    assert_ne!(
        cert["level"],
        Value::from("L3"),
        "a non-diverge loop with a non-decreasing `dec` must NOT certify L3:\n{cert:#?}"
    );
    // The failure is specifically a TERMINATION obligation (the decreases measure),
    // proving that obligation is still LIVE for a non-diverge fn (the #87 exemption
    // did not fire) — NOT a diverge cap, NOT a weak-contract reject.
    let obs = cert["obligations"].as_array().expect("obligations array");
    let mentions_termination = obs.iter().any(|o| {
        o.get("name")
            .and_then(|d| d.as_str())
            .map(|s| s.to_lowercase().contains("decreases"))
            .unwrap_or(false)
    });
    assert!(
        mentions_termination,
        "a non-diverge loop's termination obligation must STILL fire (decreases not \
         satisfied) — the #87 exemption is diverge-only:\n{cert:#?}"
    );
    let _ = std::fs::remove_file(&fixture);
}

#[test]
fn corpus_still_certifies_l3_unperturbed() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — corpus L3 regression not run.");
        return;
    }
    // The AC-7(d) anchor: the total corpus (NO `diverge`, `dec` present) is
    // UNCHANGED at L3 — the diverge gate never fires for it.
    let (code, sum_certs) = run_check_json(&conformance_dir().join("sum.th"));
    assert_eq!(code, Some(0), "sum.th still verifies clean (exit 0)");
    assert_eq!(level_of(&sum_certs, "sum"), "L3", "sum still L3");

    let (code, bs_certs) = run_check_json(&conformance_dir().join("binary_search.th"));
    assert_eq!(
        code,
        Some(0),
        "binary_search.th still verifies clean (exit 0)"
    );
    assert_eq!(
        level_of(&bs_certs, "binary_search"),
        "L3",
        "binary_search still L3"
    );
}
