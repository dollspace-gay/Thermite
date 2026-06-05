//! Conformance test for `forge build`'s runtime effect sandbox (issue #57) against
//! the EXTERNAL truth: the real `rustc` compiler + the real Linux seccomp kernel +
//! the hand-derived oracle `conformance/sandbox/cases.json`
//! (`.design/forge/runtime-sandbox.md`).
//!
//! `forge build --entry` injects (ON BY DEFAULT) a seccomp-bpf filter-install
//! prelude into the generated `main`, BEFORE the entry runs; the allowlist is the
//! entry's transitive `fx` projection. A syscall off the allowlist → `SIGSYS` →
//! the process is killed (exit 159 = 128+SIGSYS(31)). PURE Thermite has no I/O
//! surface, so the kill is DEMONSTRATED via `--sandbox-self-test` (an `openat`
//! probe AFTER the filter): denied under a `pure` filter, allowed under `read(_)`.
//!
//! Verification is by EXECUTION (the design's AC-1..AC-3 + the critical
//! panic-not-killed interaction): build each fixture through the REAL CLI, RUN the
//! produced binary, and assert the exit code / output. Expected values
//! (exit 0 + `6`; exit 159; exit 101 + `[ens]`) trace to the oracle / the §4.1
//! mechanism, never copied from toolchain output (R-CHAR-3).
//!
//! These tests RUN binaries and need a Linux seccomp kernel with `kill_process`.
//! When seccomp is unavailable (a non-Linux host, an old kernel) they SKIP LOUD
//! (an eprintln) rather than fail — the install is host-kernel-dependent
//! (`.design/forge/runtime-sandbox.md` Verification, OQ-3). `unwrap`/`expect`/
//! `panic!` are fine here — `tests/` is not anti-pattern-gated. The `forge` binary
//! is invoked as a subprocess so the whole `forge build --entry --sandbox …`
//! surface is exercised end-to-end.

use std::path::{Path, PathBuf};
use std::process::Command;

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("conformance")
}

/// The compiled `forge` binary under test (cargo sets `CARGO_BIN_EXE_<name>`).
fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// `true` iff this host's kernel offers the `kill_process` seccomp action (the
/// REQ-1 mechanism). When absent the seccomp tests SKIP LOUD (OQ-3). The probe
/// crate's grounding (`.design/forge/runtime-sandbox.md`) read this same file.
fn seccomp_kill_available() -> bool {
    std::fs::read_to_string("/proc/sys/kernel/seccomp/actions_avail")
        .map(|s| s.contains("kill_process"))
        .unwrap_or(false)
}

/// Run `forge build <args...>` and return `(exit_success, stdout, stderr)`.
fn run_forge_build(args: &[&str]) -> (bool, String, String) {
    let out = Command::new(forge_bin())
        .arg("build")
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("spawning `forge build` failed: {e}"));
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

/// Parse the `artifact:` path out of `forge build`'s `--json` document.
fn artifact_path_from_json(stdout: &str) -> PathBuf {
    let v: serde_json::Value = serde_json::from_str(stdout)
        .unwrap_or_else(|e| panic!("forge build --json not JSON: {e}\n{stdout}"));
    let p = v["artifact"]
        .as_str()
        .unwrap_or_else(|| panic!("no `artifact` field in build manifest:\n{stdout}"));
    PathBuf::from(p)
}

/// Run a produced executable artifact, returning `(exit_code, combined_output)`.
/// The exit code is the process's: `Some(code)` for a normal exit, `None` for a
/// signal-terminated process. Bash reports a signal-killed process as `128 +
/// signal`; we read the raw exit code via `ExitStatus::code()` and the signal via
/// the unix extension so the `159` / `101` distinction (kill vs panic) is exact.
fn run_artifact(path: &Path) -> (Option<i32>, Option<i32>, String) {
    use std::os::unix::process::ExitStatusExt;
    let out = Command::new(path)
        .output()
        .unwrap_or_else(|e| panic!("running artifact `{}` failed: {e}", path.display()));
    let mut combined = String::from_utf8_lossy(&out.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.code(), out.status.signal(), combined)
}

/// Remove a produced artifact + its per-run output dir (#53 — no leaked build
/// artifacts). The scratch (source + rustc intermediates) is cleaned by `forge`'s
/// own Drop guard; this removes the copied-out artifact.
fn cleanup(artifact: &Path) {
    let _ = std::fs::remove_file(artifact);
    if let Some(parent) = artifact.parent() {
        let _ = std::fs::remove_dir_all(parent);
    }
}

/// Write a throwaway `.th` fixture under the temp dir (cleaned by the caller).
fn write_fixture(name: &str, body: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "forge_sandbox_test_{name}_{}.th",
        std::process::id()
    ));
    std::fs::write(&path, body).unwrap_or_else(|e| panic!("write fixture {name}: {e}"));
    path
}

/// SIGSYS = 31; a process killed by it reports raw signal 31 / shell exit 159.
const SIGSYS: i32 = 31;
/// A Rust panic (the `[ens]` L1 contract violation) aborts with process exit 101.
const PANIC_EXIT: i32 = 101;

// ---- oracle `pure_runs_clean` (AC-1) ----------------------------------------
//
// `forge build conformance/sum.th --entry sum --sandbox` → the pure seccomp filter
// allows the baseline (run + print + exit), so the sandboxed binary runs CLEAN:
// prints `6` (sum(&[1,2,3]) == 6, hand-derived from Appendix A's spec_sum), exit 0.
// The sandbox does not impede a program that stays within its declared (empty I/O)
// effects.

#[test]
fn pure_runs_clean() {
    if !seccomp_kill_available() {
        eprintln!(
            "SKIP pure_runs_clean: no /proc/sys/kernel/seccomp/actions_avail kill_process \
             (host lacks the seccomp mechanism; OQ-3)"
        );
        return;
    }
    let sum = corpus_dir().join("sum.th");
    let (ok, stdout, stderr) = run_forge_build(&[
        sum.to_str().unwrap(),
        "--entry",
        "sum",
        "--sandbox",
        "--json",
    ]);
    assert!(
        ok,
        "forge build sum.th --entry sum --sandbox must succeed:\n{stdout}\n{stderr}"
    );

    // The manifest records the installed pure allowlist (REQ-5) and that it
    // EXCLUDES openat (257) — a pure filter denies file I/O.
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("manifest JSON");
    assert_eq!(
        v["sandbox"]["installed"], true,
        "the sandbox is on by default for --entry"
    );
    let allow: Vec<i64> = v["sandbox"]["syscall_allowlist"]
        .as_array()
        .expect("allowlist array")
        .iter()
        .map(|n| n.as_i64().unwrap())
        .collect();
    assert!(
        !allow.contains(&257),
        "AC-1: the pure filter excludes openat (257): {allow:?}"
    );

    let artifact = artifact_path_from_json(&stdout);
    let (code, signal, output) = run_artifact(&artifact);
    assert_eq!(
        code,
        Some(0),
        "AC-1: the pure sandboxed binary runs clean (exit 0); signal={signal:?}\n{output}"
    );
    assert!(
        output.contains('6'),
        "AC-1: prints 6 (sum(&[1,2,3]) == 6, hand-derived):\n{output}"
    );
    cleanup(&artifact);
}

// ---- oracle `probe_killed` (AC-2) -------------------------------------------
//
// `--entry sum --sandbox --sandbox-self-test` → the openat probe (a disallowed I/O
// syscall under sum's PURE filter) is KILLED by seccomp → SIGSYS, exit 159
// (128+31). Demonstrates the filter ENFORCES: a syscall outside the declared fx is
// killed at the boundary (§4.1).

#[test]
fn probe_killed() {
    if !seccomp_kill_available() {
        eprintln!("SKIP probe_killed: no seccomp kill_process action available (OQ-3)");
        return;
    }
    let sum = corpus_dir().join("sum.th");
    let (ok, stdout, stderr) = run_forge_build(&[
        sum.to_str().unwrap(),
        "--entry",
        "sum",
        "--sandbox",
        "--sandbox-self-test",
        "--json",
    ]);
    assert!(
        ok,
        "the probe build must compile (rustc exit 0):\n{stdout}\n{stderr}"
    );
    let artifact = artifact_path_from_json(&stdout);

    let (code, signal, output) = run_artifact(&artifact);
    // The kernel kills the process at the openat boundary: raw signal 31 (SIGSYS),
    // shell exit 159. Either witness is the kill (a normal exit 0 would be a miss).
    assert!(
        signal == Some(SIGSYS) || code == Some(159),
        "AC-2: the openat probe under the PURE filter must be KILLED by SIGSYS (signal 31 / \
         exit 159); got code={code:?} signal={signal:?}\n{output}"
    );
    // The kill happens BEFORE the entry call → no `6` output.
    assert!(
        !output.contains('6'),
        "AC-2: the kill precedes the entry call (no entry output):\n{output}"
    );
    cleanup(&artifact);
}

// ---- oracle `probe_allowed_when_fx_widens` (AC-3) ---------------------------
//
// The `rf` fixture (`fx read(src)`) `--entry rf --sandbox --sandbox-self-test` →
// the allowlist WIDENS to include openat, so the SAME probe is ALLOWED (no kill) →
// exit 0. Confirms the allowlist is fx-DERIVED (read → openat permitted), not a
// constant.

#[test]
fn probe_allowed_when_fx_widens() {
    if !seccomp_kill_available() {
        eprintln!("SKIP probe_allowed_when_fx_widens: no seccomp kill_process action (OQ-3)");
        return;
    }
    // The oracle's `rf` fixture (declared inline per cases.json `program`).
    let fixture = write_fixture(
        "rf",
        "fn rf(x: u32) -> u32 req x < 100 ens result == x fx read(src) { x }\n",
    );
    let (ok, stdout, stderr) = run_forge_build(&[
        fixture.to_str().unwrap(),
        "--entry",
        "rf",
        "--sandbox",
        "--sandbox-self-test",
        "--json",
    ]);
    assert!(ok, "the rf probe build must compile:\n{stdout}\n{stderr}");

    // REQ-2/REQ-3: the read fx widened the allowlist to include openat (257).
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("manifest JSON");
    assert_eq!(
        v["sandbox"]["transitive_fx"],
        serde_json::json!(["read(src)"])
    );
    let allow: Vec<i64> = v["sandbox"]["syscall_allowlist"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n.as_i64().unwrap())
        .collect();
    assert!(
        allow.contains(&257),
        "AC-3: read(_) widens the allowlist to include openat (257): {allow:?}"
    );

    let artifact = artifact_path_from_json(&stdout);
    let (code, signal, output) = run_artifact(&artifact);
    assert_eq!(
        code,
        Some(0),
        "AC-3: under the read(src) filter the openat probe is ALLOWED → exit 0 (no kill); \
         signal={signal:?}\n{output}"
    );
    cleanup(&artifact);
    let _ = std::fs::remove_file(&fixture);
}

// ---- CRITICAL: a contract violation still PANICS, not seccomp-killed --------
//
// A `bad` program (`ens result == x`, body `x + 1`) `--entry bad --sandbox` → run
// → exit 101 + an `[ens]` contract-violation panic (NOT exit 159 / SIGSYS). This is
// the key correctness interaction: the pure baseline allowlist MUST include the
// panic/abort path (write + exit_group + rt_sigreturn), so a violated contract
// PANICS (the §4.1 enforcement the user sees) rather than being silently
// seccomp-killed. A SIGSYS here would mean the baseline is broken.

#[test]
fn contract_violation_panics_not_killed() {
    if !seccomp_kill_available() {
        eprintln!("SKIP contract_violation_panics_not_killed: no seccomp kill_process (OQ-3)");
        return;
    }
    let fixture = write_fixture(
        "bad",
        "fn bad(x: u32) -> u32 req x < 100 ens result == x fx pure { x + 1 }\n",
    );
    let (ok, stdout, stderr) = run_forge_build(&[
        fixture.to_str().unwrap(),
        "--entry",
        "bad",
        "--sandbox",
        "--json",
    ]);
    assert!(
        ok,
        "forge build (L1) must COMPILE the contract-violating `bad`:\n{stdout}\n{stderr}"
    );
    let artifact = artifact_path_from_json(&stdout);

    let (code, signal, output) = run_artifact(&artifact);
    // It must NOT be seccomp-killed: the baseline allows the panic/abort path.
    assert_ne!(
        signal,
        Some(SIGSYS),
        "REGRESSION: a contract violation was seccomp-KILLED (SIGSYS) instead of panicking — the \
         pure baseline must allow the panic/abort path (write/exit_group/rt_sigreturn):\n{output}"
    );
    // It panics with exit 101 and the [ens] diagnostic (the §4.1 enforcement).
    assert_eq!(
        code,
        Some(PANIC_EXIT),
        "the violated ens check must PANIC (exit 101), not seccomp-kill; signal={signal:?}\n{output}"
    );
    assert!(
        output.contains("[ens]") || output.contains("contract violation"),
        "the runtime ens check must fire with an [ens] diagnostic:\n{output}"
    );
    cleanup(&artifact);
    let _ = std::fs::remove_file(&fixture);
}

// ---- AC-5: --no-sandbox / library build emit no prelude ---------------------
//
// `--no-sandbox --entry` records `sandbox.installed == false` (no PR_SET_SECCOMP),
// and a library build (no --entry) likewise. The oracle's `no_prelude` family.

#[test]
fn no_sandbox_omits_prelude() {
    // No seccomp needed: this only inspects the manifest, never runs a filter.
    let sum = corpus_dir().join("sum.th");
    let (ok, stdout, stderr) = run_forge_build(&[
        sum.to_str().unwrap(),
        "--entry",
        "sum",
        "--no-sandbox",
        "--json",
    ]);
    assert!(
        ok,
        "forge build --no-sandbox must succeed:\n{stdout}\n{stderr}"
    );
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("manifest JSON");
    assert_eq!(
        v["sandbox"]["installed"], false,
        "AC-5: --no-sandbox emits no prelude"
    );
    let artifact = artifact_path_from_json(&stdout);

    // The --no-sandbox binary runs clean (no filter installed) and prints 6.
    let (code, _signal, output) = run_artifact(&artifact);
    assert_eq!(
        code,
        Some(0),
        "the --no-sandbox binary runs unconfined:\n{output}"
    );
    assert!(output.contains('6'));
    cleanup(&artifact);

    // A library build (no --entry) also records no sandbox (an rlib has no main).
    let (ok2, stdout2, stderr2) = run_forge_build(&[sum.to_str().unwrap(), "--json"]);
    assert!(
        ok2,
        "forge build (library) must succeed:\n{stdout2}\n{stderr2}"
    );
    let v2: serde_json::Value = serde_json::from_str(&stdout2).expect("manifest JSON");
    assert_eq!(
        v2["sandbox"]["installed"], false,
        "AC-5: a library build has no prelude"
    );
    let lib = artifact_path_from_json(&stdout2);
    cleanup(&lib);
}
