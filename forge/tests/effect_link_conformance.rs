//! Conformance test for the RUNNABLE EFFECT LINK (Basis Stage 8, issue **#81**)
//! against the EXTERNAL truths: the real `rustc` compiler, the real Linux seccomp
//! kernel, the real `verus` prover, and the hand-derived oracle
//! `conformance/effect-link/cases.json` (`.design/basis/08-runnable-effect-link.md`).
//!
//! `forge build` emits a `mod os { … }` (the syscall-wrapper impls) into the
//! generated crate, keyed by the reachable `#[boundary("os::<name>")]` targets, so a
//! VERIFIED program that uses an effect primitive COMPILES + RUNS + does real I/O.
//! `forge check` (verification) is UNCHANGED — the link is a build-only concern.
//!
//! Verification is by EXECUTION (the design's AC-1..AC-7): the centerpiece
//! `effect_link_demo.th` (`now` boundary + `elapsed_ok` L3-to-boundary caller)
//! BUILDS (rustc exit 0, no `E0433`), RUNS (a live `clock_gettime`, prints a
//! `u64` timestamp), is #57-seccomp-CONFINED (`clock_gettime` allowed, an
//! out-of-`fx` `openat` SIGSYS-killed), and `forge check` certifies IDENTICALLY
//! before and after the link. Expected values trace to the oracle / the §4.1
//! mechanism, never copied from toolchain output (R-CHAR-3).
//!
//! The build/run cases need a Linux seccomp kernel with `kill_process`; the
//! `verify_unchanged` case needs `verus`. Both SKIP LOUD (an eprintln) when their
//! external truth is absent (the precedent of `sandbox_conformance.rs` /
//! `effect_stdlib_conformance.rs`). `unwrap`/`expect`/`panic!` are fine here —
//! `tests/` is not anti-pattern-gated. The `forge` binary is invoked as a
//! subprocess so the whole `forge build --entry --sandbox …` surface is exercised.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;

fn conformance_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("conformance")
}

/// The compiled `forge` binary under test (cargo sets `CARGO_BIN_EXE_<name>`).
fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// The frozen centerpiece program the oracle pins (NEVER edited — R-CHAR-3): a
/// `#[boundary("os::now")]` Time primitive + a pure `elapsed_ok` caller that
/// composes to L3 to-the-boundary. The `now` build-link demo.
fn effect_link_demo() -> PathBuf {
    conformance_dir().join("effect_link_demo.th")
}

/// `true` iff this host's kernel offers the `kill_process` seccomp action (the #57
/// mechanism). When absent the seccomp tests SKIP LOUD (`runtime-sandbox.md` OQ-3).
fn seccomp_kill_available() -> bool {
    std::fs::read_to_string("/proc/sys/kernel/seccomp/actions_avail")
        .map(|s| s.contains("kill_process"))
        .unwrap_or(false)
}

/// `true` iff a `verus` binary is resolvable (mirrors `effect_stdlib_conformance.rs`).
/// The `verify_unchanged` case is a real verus proof of the L3 compose-through.
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

/// Run `forge check <file> --mutation-floor F --json`, returning the parsed cert
/// array (empty on a non-JSON result), the exit code, and stderr.
fn run_check_json(file: &Path, mutation_floor: f64) -> (Option<i32>, Vec<Value>, String) {
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(file)
        .arg("--mutation-floor")
        .arg(format!("{mutation_floor}"))
        .arg("--json")
        .output()
        .unwrap_or_else(|e| panic!("spawn forge check: {e}"));
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let certs = serde_json::from_str::<Value>(stdout.trim())
        .ok()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();
    (out.status.code(), certs, stderr)
}

/// Parse the `artifact:` path out of `forge build`'s `--json` document.
fn artifact_path_from_json(stdout: &str) -> PathBuf {
    let v: Value = serde_json::from_str(stdout)
        .unwrap_or_else(|e| panic!("forge build --json not JSON: {e}\n{stdout}"));
    let p = v["artifact"]
        .as_str()
        .unwrap_or_else(|| panic!("no `artifact` field in build manifest:\n{stdout}"));
    PathBuf::from(p)
}

/// Run a produced artifact (no stdin), returning `(exit_code, signal, combined)`.
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

fn find_cert<'a>(certs: &'a [Value], item: &str) -> &'a Value {
    certs
        .iter()
        .find(|c| c.get("item").and_then(|v| v.as_str()) == Some(item))
        .unwrap_or_else(|| panic!("no certificate for item `{item}` in {certs:?}"))
}

// SIGSYS = 31; a process killed by it reports raw signal 31 / shell exit 159.
const SIGSYS: i32 = 31;

// ---------------------------------------------------------------------------
// verify_unchanged (AC-4): forge check certifies IDENTICALLY before/after the link.
//   now      -> L1 boundary (os::now, fx time)
//   elapsed_ok -> L3, to_boundary via now (--mutation-floor 0)
// The link lives in build.rs codegen + rustc; forge check never emits `mod os`.
// Anchored to conformance/effect-link/cases.json `verify_unchanged` (R-CHAR-3).
// ---------------------------------------------------------------------------

#[test]
fn verify_unchanged() {
    if !verus_present() {
        eprintln!(
            "SKIP verify_unchanged: verus not available — the elapsed_ok L3 compose-through is a \
             real verus proof (`08-runnable-effect-link.md` AC-4)."
        );
        return;
    }
    let (code, certs, stderr) = run_check_json(&effect_link_demo(), 0.0);
    assert_eq!(
        code,
        Some(0),
        "forge check effect_link_demo.th --mutation-floor 0 must succeed:\n{stderr}"
    );

    // `now` — the Time effect primitive → L1 boundary (os::now, fx time). The link
    // does NOT change this cert (the boundary L1-short-circuits in `forge check`).
    let now = find_cert(&certs, "now");
    assert_eq!(now["level"], Value::from("L1"), "now is the L1 boundary");
    assert_eq!(
        now["boundary"],
        Value::from(true),
        "now carries the boundary flag"
    );
    assert_eq!(
        now["boundary_target"],
        Value::from("os::now"),
        "now records its os::now foreign target (the link target)"
    );
    let now_fx: Vec<String> = now["effects"]
        .as_array()
        .expect("effects array")
        .iter()
        .map(|e| e.as_str().expect("effect string").to_string())
        .collect();
    assert!(
        now_fx.iter().any(|e| e.contains("time")),
        "now declares the typed `time` effect: {now_fx:?}"
    );

    // `elapsed_ok` — the pure caller composes THROUGH now's assumed ens → L3 +
    // to_boundary via now (#52 + #17). UNCHANGED by the build-only link.
    let elapsed = find_cert(&certs, "elapsed_ok");
    assert_eq!(
        elapsed["level"],
        Value::from("L3"),
        "elapsed_ok proves L3 through now's assumed ens"
    );
    assert_eq!(
        elapsed["assurance_scope"]["kind"],
        Value::from("to_boundary"),
        "elapsed_ok is to-the-boundary (its closure reaches the now primitive)"
    );
    assert_eq!(
        elapsed["assurance_scope"]["via"],
        Value::from("now"),
        "elapsed_ok records the crossing via now"
    );
}

// ---------------------------------------------------------------------------
// build_and_run (AC-1): `forge build effect_link_demo.th --entry elapsed_ok` emits
// `mod os { pub fn now() -> u64 { SystemTime::now()... } }`, rustc-LINKS it (exit 0,
// no E0433), and the binary RUNS -> the linked os::now does a real clock_gettime ->
// prints `elapsed_ok() = <live Unix timestamp>` (a u64, > 0, < 4_000_000_000), exit
// 0. THE UNLOCK. The value is the world's wall clock (nondeterministic), so the
// oracle asserts the RANGE + a u64, not a fixed value (R-CHAR-3, R-CODE-5).
// Anchored to conformance/effect-link/cases.json `build_and_run`.
// ---------------------------------------------------------------------------

#[test]
fn elapsed_ok_builds_and_runs() {
    if !seccomp_kill_available() {
        eprintln!(
            "SKIP elapsed_ok_builds_and_runs: no seccomp kill_process (the --entry build is \
             sandboxed by default; `runtime-sandbox.md` OQ-3)."
        );
        return;
    }
    let demo = effect_link_demo();
    let (ok, stdout, stderr) =
        run_forge_build(&[demo.to_str().unwrap(), "--entry", "elapsed_ok", "--json"]);
    assert!(
        ok,
        "AC-1: forge build --entry elapsed_ok must COMPILE — the emitted `mod os` resolves \
         os::now (no E0433):\nstdout:{stdout}\nstderr:{stderr}"
    );

    let artifact = artifact_path_from_json(&stdout);
    let (code, signal, output) = run_artifact(&artifact);
    assert_eq!(
        code,
        Some(0),
        "AC-1: the linked binary RUNS clean (a real clock_gettime under the sandbox); \
         signal={signal:?}\n{output}"
    );

    // The output is `elapsed_ok() = <u64 timestamp>`: parse the trailing integer and
    // assert it is a live Unix timestamp (> 0, < 4_000_000_000 — the `now` ens
    // bound). The exact value is the world's wall clock (R-CHAR-3 — not a fixture).
    let ts: u64 = output
        .rsplit('=')
        .next()
        .and_then(|tail| tail.trim().parse().ok())
        .unwrap_or_else(|| panic!("AC-1: output must end `= <u64 timestamp>`:\n{output}"));
    assert!(
        ts > 0 && ts < 4_000_000_000,
        "AC-1: elapsed_ok prints a live Unix timestamp (0 < {ts} < 4_000_000_000) — a verified \
         program did real I/O:\n{output}"
    );
    cleanup(&artifact);
}

// ---------------------------------------------------------------------------
// sandbox_confines (AC-3): the linked os::now's clock_gettime is in the fx-time
// allowlist (#57: baseline ∪ {clock_gettime 228, clock_nanosleep 230}), so the
// binary runs clean under the default sandbox (exit 0); a `--sandbox-self-test`
// openat probe under the SAME `time` filter (openat NOT in the time allowlist) is
// SIGSYS-KILLED (exit 159 = 128+31, the #57 pure_probe_killed precedent). The
// linked foreign body is confined to EXACTLY its declared fx.
// Anchored to conformance/effect-link/cases.json `sandbox_confines`.
// ---------------------------------------------------------------------------

#[test]
fn sandbox_confines_the_linked_wrapper() {
    if !seccomp_kill_available() {
        eprintln!("SKIP sandbox_confines_the_linked_wrapper: no seccomp kill_process (OQ-3).");
        return;
    }
    let demo = effect_link_demo();

    // (A) the default-sandboxed run: the fx-time allowlist INCLUDES clock_gettime
    // (228), so the linked os::now runs clean and exits 0.
    let (ok, stdout, stderr) =
        run_forge_build(&[demo.to_str().unwrap(), "--entry", "elapsed_ok", "--json"]);
    assert!(ok, "the sandboxed build must compile:\n{stdout}\n{stderr}");
    let v: Value = serde_json::from_str(&stdout).expect("manifest JSON");
    assert_eq!(
        v["sandbox"]["installed"], true,
        "the sandbox is ON by default for --entry"
    );
    let allow: Vec<i64> = v["sandbox"]["syscall_allowlist"]
        .as_array()
        .expect("allowlist array")
        .iter()
        .map(|n| n.as_i64().unwrap())
        .collect();
    assert!(
        allow.contains(&228),
        "AC-3: the fx-time allowlist INCLUDES clock_gettime (228): {allow:?}"
    );
    assert!(
        !allow.contains(&257),
        "AC-3: the fx-time allowlist EXCLUDES openat (257) — a Time primitive cannot exceed \
         Time syscalls: {allow:?}"
    );
    let artifact = artifact_path_from_json(&stdout);
    let (code, signal, output) = run_artifact(&artifact);
    assert_eq!(
        code,
        Some(0),
        "AC-3 (A): the linked os::now's clock_gettime runs clean under the time filter (exit 0); \
         signal={signal:?}\n{output}"
    );
    cleanup(&artifact);

    // (B) the out-of-fx openat probe under the SAME `time` filter → SIGSYS kill.
    let (ok2, stdout2, stderr2) = run_forge_build(&[
        demo.to_str().unwrap(),
        "--entry",
        "elapsed_ok",
        "--sandbox-self-test",
        "--json",
    ]);
    assert!(ok2, "the probe build must compile:\n{stdout2}\n{stderr2}");
    let probe = artifact_path_from_json(&stdout2);
    let (code2, signal2, output2) = run_artifact(&probe);
    assert!(
        signal2 == Some(SIGSYS) || code2 == Some(159),
        "AC-3 (B): the openat probe under the `time` filter must be SIGSYS-KILLED (signal 31 / \
         exit 159) — openat is out of the Time fx; got code={code2:?} signal={signal2:?}\n{output2}"
    );
    cleanup(&probe);
}

// ---------------------------------------------------------------------------
// NO regression (AC-7): the pure corpus (`sum`) builds + runs IDENTICALLY — NO
// `mod os` is emitted (no boundary target reachable), `sum(&[1,2,3]) = 6` per
// build.md AC-3. The link is a no-op for a boundary-free program.
// ---------------------------------------------------------------------------

#[test]
fn pure_corpus_unaffected_by_the_link() {
    let sum = conformance_dir().join("sum.th");
    let (ok, stdout, stderr) = run_forge_build(&[
        sum.to_str().unwrap(),
        "--entry",
        "sum",
        "--no-sandbox",
        "--json",
    ]);
    assert!(
        ok,
        "AC-7: the pure `sum` must still build (the link is a no-op without a boundary):\n\
         stdout:{stdout}\nstderr:{stderr}"
    );
    let artifact = artifact_path_from_json(&stdout);

    // AC-7: a boundary-free build runs identically — `sum(&[1,2,3]) = 6`.
    let (code, _signal, output) = run_artifact(&artifact);
    assert_eq!(code, Some(0), "the pure sum binary runs clean:\n{output}");
    assert!(
        output.contains('6'),
        "AC-7: sum(&[1,2,3]) == 6 (hand-derived, unchanged by the link):\n{output}"
    );
    cleanup(&artifact);
}

// ---------------------------------------------------------------------------
// read_byte links + runs (REQ-1/REQ-3, the design's read_demo shape): a
// `#[boundary("os::read_byte")] -> u64` (EOF sentinel 256) + a `doubled` caller
// builds, and run with stdin `A` (byte 65) prints `doubled() = 130` (65+65, the
// hand-derived value); run with EOF prints `doubled() = 0` (the handled EOF arm).
// Both arms of the closed outcome set RUN. The inputs are the explicit determinism
// source (R-CODE-5). The fixture mirrors the design's read_demo (R-CHAR-3); it is a
// throwaway temp file (the frozen corpus is the `now` demo).
// ---------------------------------------------------------------------------

#[test]
fn read_byte_links_and_runs_both_arms() {
    if !seccomp_kill_available() {
        eprintln!("SKIP read_byte_links_and_runs_both_arms: no seccomp kill_process (OQ-3).");
        return;
    }
    // The design's read_demo (08-runnable-effect-link.md AC-2), hand-derived.
    let prog = "#[boundary(\"os::read_byte\")] fn read_byte() -> u64\n  req true\n  \
                ens result <= 256\n  fx  read(input)\n  ;\n\n\
                fn doubled() -> u64\n  req true\n  ens result < 512\n  fx  read(input)\n{\n  \
                let v = read_byte();\n  if v < 256 { v + v } else { 0 }\n}\n";
    let fixture = std::env::temp_dir().join(format!("effect_link_read_{}.th", std::process::id()));
    std::fs::write(&fixture, prog).expect("write read fixture");

    let (ok, stdout, stderr) =
        run_forge_build(&[fixture.to_str().unwrap(), "--entry", "doubled", "--json"]);
    assert!(
        ok,
        "REQ-3: forge build --entry doubled must COMPILE — the emitted `mod os` resolves \
         os::read_byte (no E0433):\nstdout:{stdout}\nstderr:{stderr}"
    );
    let artifact = artifact_path_from_json(&stdout);

    // Run with stdin `A` (byte 65) → doubled() = 130 (65+65 < 512, the Some arm).
    let out_a = run_with_stdin(&artifact, b"A");
    assert!(
        out_a.contains("130"),
        "AC-2: stdin `A` (byte 65) → doubled() = 130 (65+65, hand-derived):\n{out_a}"
    );
    // Run with EMPTY stdin (EOF) → doubled() = 0 (the handled EOF/256 arm).
    let out_eof = run_with_stdin(&artifact, b"");
    assert!(
        out_eof.contains("doubled() = 0"),
        "AC-2: empty stdin (EOF, read_byte=256) → doubled() = 0 (the handled EOF arm):\n{out_eof}"
    );

    cleanup(&artifact);
    let _ = std::fs::remove_file(&fixture);
}

/// Run a produced artifact feeding `input` on stdin, returning the combined output.
fn run_with_stdin(path: &Path, input: &[u8]) -> String {
    use std::io::Write;
    let mut child = Command::new(path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("spawn artifact `{}`: {e}", path.display()));
    child
        .stdin
        .take()
        .expect("child stdin")
        .write_all(input)
        .expect("write stdin");
    let out = child.wait_with_output().expect("wait artifact");
    let mut combined = String::from_utf8_lossy(&out.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&out.stderr));
    combined
}
