//! THE PROOF-OF-THE-PUDDING (crosslink #125, builds on #90, ref #83 #105): the
//! MAX-VERIFIED interactive MULTI-LINE editor that RUNS. This integration test
//! grounds `examples/editor/editor.th` end-to-end against the EXTERNAL truths the
//! toolchain does not author for itself — the real `verus` SMT prover (the cert
//! levels) and the real `rustc` compiler + a real process run (the build + the
//! piped-keystroke session).
//!
//! THE #125 MULTI-LINE EXTENSION: on top of the shipped edit core, the editor adds
//! the VERIFIED NAV / LAYOUT core — `count_nl`/`line_start`/`line_end`/`min2`
//! (verified recursive line scans), `cursor_row`/`cursor_col` (the cursor's
//! ROW/COLUMN), `move_up`/`move_down` (up/down line navigation), and `to_1based`
//! (the proven 0→1-based ANSI conversion) — all L3, plus the file LOAD/SAVE
//! boundaries `read_file`/`write_file` (L1). `editor_multiline_enter_up_nav_and_ctrl_s_save`
//! grounds the runnable proof: Enter inserts a `\n` (the cursor drops to row 2), the
//! UP arrow moves to the same column on the previous line, and Ctrl-S saves the
//! multi-line buffer (the `\n` round-trips through the file).
//!
//! THE #90 THESIS — the editor's bug-prone LOGIC (display + input + NAV/LAYOUT) is
//! PROVEN; only the raw read/write/ioctl/open SYSCALLS are trusted:
//!
//!   * `forge check editor.th` certifies:
//!       - the VERIFIED EDIT CORE (`Buffer`, `insert_str`, `backspace`,
//!         `move_left`, `move_right`) at **L3** (cursor math + length deltas PROVEN);
//!       - the VERIFIED RENDER-FRAME (`render_frame`) at **L3** — THE THESIS: the
//!         display-frame construction is PROVEN Thermite, not trusted glue (the C4
//!         cursor coordinate `(b.cursor+1).to_string()` now discharges the bounded
//!         `concat` §4.2 CAP because `u64_to_string`'s `ens` bounds the formatted
//!         length `<= 20`, blocker #105);
//!       - the VERIFIED DECODE (`decode`) at **L3** — the keystroke interpretation
//!         is a PURE TOTAL function, proven;
//!       - the MINIMAL TRUSTED SYSCALL BOUNDARY (`raw_mode_on`, `raw_mode_off`,
//!         `read_key_raw`, `write_frame`) at **L1 boundary** (the foreign termios /
//!         read / write bodies, trusted-by-fiat, contract-stated);
//!       - the event loop `run` (`fx diverge`) at **L1** = partial correctness (the
//!         #88 cap — NOT L0 `WeakContract`).
//!   * `forge build editor.th --entry run` COMPILES (`render_frame(&Buffer)` borrows
//!     `b`, no E0382) and the produced binary RUNS with piped keystrokes — insert,
//!     a LEFT arrow, a mid-text insert (splice), backspace, Ctrl-Q — and the frames
//!     reflect the L3-proven edits.
//!
//! THE TERMIOS BOUNDARY NEEDS `ioctl`: `raw_mode_on`/`raw_mode_off` call
//! `tcgetattr`/`tcsetattr`, which on Linux issue the `ioctl` syscall (16). The v0.1
//! `write(output)` seccomp set (`forge/src/sandbox.rs` `WRITE_SYSCALLS`) does NOT
//! grant `ioctl`, so the SANDBOXED binary is SIGSYS-killed before `tcgetattr` can
//! return its graceful non-TTY status. The piped run therefore builds with
//! `--no-sandbox` (the honest seam: the terminal-control boundary is trusted-by-fiat
//! and its ioctl is not yet in the sandbox table — a separate `sandbox.rs` /
//! `runtime-sandbox.md` gap, reported as spillover). Under `--no-sandbox` the
//! wrapper's own non-TTY handling (`tcgetattr` returns ENOTTY -> the wrapper returns
//! 1, no crash) is exercised by the piped (non-TTY) stdin.
//!
//! And the diverge cap's HONESTY (it is diverge-ONLY, not a Goodhart bypass —
//! `goal.md` R-DEFER-9):
//!
//!   * a NON-diverge weak-contract fn STILL rejects at L0 `WeakContract`;
//!   * a NORMAL loop fn WITHOUT a strictly-decreasing `dec` STILL fails termination;
//!   * `conformance/sum.th` / `binary_search.th` STILL certify L3 (the corpus oracle
//!     is unperturbed — the `u64_to_string` upper-bound strengthening did not break
//!     the total corpus).
//!
//! Driving the BUILT `forge` binary (not a library API) keeps `forge` a pure `bin`
//! crate and exercises the real CLI surface. The cert-level checks RUN VERUS; if
//! verus is absent they SKIP LOUDLY (the `check_conformance.rs` precedent) — never
//! panic on a missing solver. `tests/` is not anti-pattern-gated, so `unwrap`/
//! `expect`/`panic!` are fine here (R-APG-2). Expected levels trace to the design
//! (`.design/forge/check.md` AC-7 / `degrade-ladder.md` AC-8) + the #90 thesis +
//! the provers' output, NEVER copied from forge's own output (R-CHAR-3).

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
// Deliverable 1 — `forge check editor.th`: edit core L3, render_frame L3,
// decode L3, boundary L1, run L1. (.design/forge/check.md AC-7(a);
// degrade-ladder.md AC-8; the #90 thesis + blocker #105.)
// ----------------------------------------------------------------------------

#[test]
fn editor_logic_certifies_l3_boundary_and_run_l1() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — editor cert-oracle not run.");
        return;
    }
    let (code, certs) = run_check_json(&editor_th());
    assert_eq!(
        code,
        Some(0),
        "a fully-certifying editor (logic L3 + boundary/run L1) exits 0; certs:\n{certs:#?}"
    );

    // THE VERIFIED LOGIC — every total edit op, the render-frame, and the decode are
    // L3 (the #90 thesis: the editor's bug-prone display + input logic is PROVEN, not
    // trusted glue). `render_frame` L3 is THE THESIS — it discharges only because
    // `u64_to_string`'s `ens` now bounds the formatted length `<= 20` (blocker #105),
    // so the bounded `concat` §4.2 CAP precondition holds.
    for op in [
        "Buffer",
        "insert_str",
        "backspace",
        "move_left",
        "move_right",
        // The multi-line NAV / LAYOUT core (#125): the verified row/col scans + the
        // up/down line navigation + the proven 1-based ANSI conversion. The editor's
        // navigation + cursor-layout LOGIC is PROVEN, not trusted glue.
        "count_nl",
        "line_start",
        "line_end",
        "min2",
        "cursor_row",
        "cursor_col",
        "move_up",
        "move_down",
        "to_1based",
        "render_frame",
        "decode",
    ] {
        assert_eq!(
            level_of(&certs, op),
            "L3",
            "the verified editor-logic item `{op}` must certify L3 (the #90/#125 thesis)"
        );
    }

    // `decode` is a PURE total function (the keystroke interpretation, proven).
    assert_eq!(
        find_cert(&certs, "decode")["effects"],
        Value::from(vec!["pure"]),
        "`decode` is a PURE total function (fx pure)"
    );

    // THE MINIMAL TRUSTED SYSCALL BOUNDARY — L1, boundary:true (foreign termios /
    // read / write bodies, trusted-by-fiat).
    for prim in [
        "raw_mode_on",
        "raw_mode_off",
        "read_key_raw",
        "write_frame",
        // The file LOAD / SAVE boundaries (#125) — extern-C `std::fs` read/write,
        // trusted-by-fiat, enumerated in the TCB.
        "read_file",
        "write_file",
    ] {
        let cert = find_cert(&certs, prim);
        assert_eq!(cert["level"], Value::from("L1"), "{prim} is an L1 boundary");
        assert_eq!(
            cert["boundary"],
            Value::from(true),
            "{prim} is a `#[boundary]` fn"
        );
    }

    // THE #88 CAP — `run` (fx diverge) is L1 = PARTIAL correctness: NOT L0
    // `WeakContract`, NOT a forced L3, and NOT a boundary fn.
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
// Deliverable 2 — `forge build editor.th --entry run`: COMPILES (no E0382, the
// `render_frame(&Buffer)` borrow) + RUNS with piped keystrokes (arrow-move +
// mid-text splice). (#90; #105 divergence 2; 08-runnable-effect-link.md.)
// ----------------------------------------------------------------------------

#[test]
fn editor_builds_and_runs_arrow_move_then_splice() {
    // rustc is always present (no skip; the build_conformance.rs precedent). This is
    // THE proof: a verified editor that runs. Build with `--no-sandbox` because the
    // termios boundary issues `ioctl` (16), which the v0.1 `write(output)` seccomp set
    // does not grant (a separate sandbox-table gap — see the module note); the
    // `--no-sandbox` build still links the SAME extern-C termios wrappers and proves
    // the runnable thesis. The compile path (E0382 fix) is identical under either flag.
    let editor = editor_th();
    let (ok, stdout, stderr) = run_forge_build(&[
        editor.to_str().unwrap(),
        "--entry",
        "run",
        "--no-sandbox",
        "--json",
    ]);
    assert!(
        ok,
        "forge build editor.th --entry run must COMPILE (render_frame(&Buffer) borrows \
         b — no E0382 borrow-after-move):\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    let artifact = artifact_path_from_json(&stdout);
    assert!(
        artifact.exists(),
        "the built editor binary must exist at {}",
        artifact.display()
    );

    // RUN it with piped keystrokes: insert 'a','b'; a LEFT arrow (ESC [ D =
    // 0x1b 0x5b 0x44 -> decode 1003, cursor moves left to between 'a' and 'b'); insert
    // 'X' (the L3 `insert_str` SPLICES mid-text -> "aXb"); backspace (0x7f -> decode
    // 127, deletes 'X' -> "ab"); Ctrl-Q (0x11 -> decode 17, clean quit). The frames
    // must show the mid-text splice ("aXb") then the backspace undo ("ab").
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
        .write_all(b"ab\x1b[DX\x7f\x11")
        .expect("pipe keystrokes to editor");
    let out = child.wait_with_output().expect("editor run completes");

    assert!(
        out.status.success(),
        "the editor must exit CLEAN (exit 0) on Ctrl-Q (the non-TTY stdin is handled \
         gracefully — no crash):\nstatus:{:?}\nstdout:{}\nstderr:{}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    // The mid-text splice: after the LEFT arrow + 'X' the buffer is "aXb" (the proven
    // `insert_str` spliced at the moved cursor), which appears in a rendered frame.
    assert!(
        stdout.contains("aXb"),
        "the editor must render the mid-text splice `aXb` (LEFT arrow then insert ran \
         the L3 `move_left` + `insert_str`):\nstdout:{stdout}\nstderr:{}",
        String::from_utf8_lossy(&out.stderr)
    );
    // After the backspace, the buffer returns to "ab" (the proven `backspace` deleted
    // the spliced 'X'), the FINAL rendered buffer.
    assert!(
        stdout.contains("ab"),
        "the editor must render `ab` after the backspace (the L3 `backspace` ran):\n\
         stdout:{stdout}\nstderr:{}",
        String::from_utf8_lossy(&out.stderr)
    );
    // The cursor-coordinate escape is the C4 `u64_to_string` formatted column — its
    // presence confirms the L3 `render_frame` (the proven display logic) produced the
    // frame, not a trusted print.
    assert!(
        stdout.contains("\x1b[1;"),
        "the frame must carry the C4 cursor-coordinate escape (render_frame ran):\n\
         stdout:{stdout}"
    );
}

// ----------------------------------------------------------------------------
// Deliverable 2b — the MULTI-LINE session (#125): Enter inserts a `\n` (the cursor
// drops to the next row), the UP arrow moves the cursor to the same column on the
// previous line (the L3 `move_up` over the verified row/col scans), and Ctrl-S
// SAVES the multi-line buffer to a file (the `os::write_file` boundary). The frames
// show TWO lines and the cursor moving between them; the saved file round-trips the
// `\n`. (#125; the verified nav/layout core L3 + the file boundary L1.)
// ----------------------------------------------------------------------------

#[test]
fn editor_multiline_enter_up_nav_and_ctrl_s_save() {
    let editor = editor_th();
    let (ok, stdout, stderr) = run_forge_build(&[
        editor.to_str().unwrap(),
        "--entry",
        "run",
        "--no-sandbox",
        "--json",
    ]);
    assert!(
        ok,
        "forge build editor.th --entry run must COMPILE (the multi-line nav scans + \
         file boundaries lower to L1):\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    let artifact = artifact_path_from_json(&stdout);
    assert!(artifact.exists(), "the built editor binary must exist");

    // A dedicated save target so the test is hermetic + asserts the round-trip. The
    // editor's `os::read_file`/`os::write_file` wrappers honor THERMITE_EDITOR_FILE.
    let save_path = std::env::temp_dir().join(format!(
        "thermite_editor_multiline_{}.txt",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&save_path);

    // Keystrokes: 'a','b'; ENTER (CR 0x0d -> decode 1004 -> insert "\n"); 'c','d';
    // UP arrow (ESC [ A = 0x1b 0x5b 0x41 -> decode 1000 -> move_up); Ctrl-S (0x13 ->
    // decode 19 -> write_file SAVE); Ctrl-Q (0x11 -> decode 17 -> clean quit).
    let mut child = Command::new(&artifact)
        .env("THERMITE_EDITOR_FILE", &save_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("spawn built editor `{}`: {e}", artifact.display()));
    child
        .stdin
        .as_mut()
        .expect("editor stdin")
        .write_all(b"ab\rcd\x1b[A\x13\x11")
        .expect("pipe multi-line keystrokes");
    let out = child.wait_with_output().expect("editor run completes");

    assert!(
        out.status.success(),
        "the multi-line editor must exit CLEAN on Ctrl-Q:\nstatus:{:?}\nstderr:{}",
        out.status,
        String::from_utf8_lossy(&out.stderr),
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    // The two-line buffer is rendered (the `\n` byte is carried into the frame by the
    // L3 `render_frame`, so the body shows "ab\ncd").
    assert!(
        stdout.contains("ab\ncd"),
        "the editor must render the TWO-line buffer `ab\\ncd` (Enter inserted the \
         L3 newline):\nstdout:{stdout:?}"
    );
    // After Enter the cursor drops to row 2, col 1 — the verified cursor_row/cursor_col
    // produce the `\x1b[2;1H` coordinate (a SECOND row, the multi-line proof).
    assert!(
        stdout.contains("\x1b[2;1H"),
        "the frame after Enter must position the cursor on row 2 (the L3 cursor_row \
         counted the inserted newline):\nstdout:{stdout:?}"
    );
    // After the UP arrow the cursor returns to row 1 (col 3) — the L3 `move_up` walked
    // the verified line boundaries to the same column on the previous line.
    assert!(
        stdout.contains("\x1b[1;3H"),
        "the frame after UP must position the cursor back on row 1 col 3 (the L3 \
         `move_up` over the verified row/col scans):\nstdout:{stdout:?}"
    );
    // Ctrl-S SAVED the multi-line buffer to the file (the `os::write_file` boundary);
    // the saved bytes round-trip the `\n` line break.
    let saved = std::fs::read(&save_path).unwrap_or_else(|e| {
        panic!(
            "the editor's Ctrl-S must have saved {}: {e}",
            save_path.display()
        )
    });
    assert_eq!(
        saved, b"ab\ncd",
        "Ctrl-S must save the multi-line buffer verbatim (the `\\n` preserved) via \
         the os::write_file boundary; got {saved:?}"
    );
    let _ = std::fs::remove_file(&save_path);
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
    // The AC-7(b) fixture: a TOTAL `fx pure` fn with a LOOSE `ens` and NO `diverge`.
    // The §7 mutation gate must STILL bite it (a `return 0`-style mutant survives the
    // loose `ens result <= 1000000`), rejecting at L0 `WeakContract`.
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
    // The AC-7(c) fixture: a NORMAL (non-diverge) fn with a `while` loop whose `dec`
    // measure does NOT strictly decrease (`dec n`, constant). Verus must STILL DEMAND
    // a strictly-decreasing measure and FAIL — the #87 termination exemption is
    // diverge-ONLY, and the #88 diverge L1 cap does not relax it for any other fn.
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
    // The AC-7(d) anchor: the total corpus (NO `diverge`, `dec` present) is UNCHANGED
    // at L3 — neither the diverge gate nor the `u64_to_string` upper-bound
    // strengthening (blocker #105) perturbs it.
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
