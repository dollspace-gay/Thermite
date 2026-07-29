//! The proof-of-the-pudding (crosslink #125, builds on #90, ref #83 #105): the
//! max-verified interactive multi-line editor that runs. This integration test
//! grounds `examples/editor/editor.th` end-to-end against the external truths the
//! toolchain does not author for itself — the real `verus` SMT prover (the cert
//! levels) and the real `rustc` compiler + a process run (the build + the
//! piped-keystroke session).
//!
//! The #125 multi-line extension: on top of the shipped edit core, the editor adds
//! the verified nav / layout core — `count_nl`/`line_start`/`line_end`/`min2`
//! (verified recursive line scans), `cursor_row`/`cursor_col` (the cursor's
//! row/column), `move_up`/`move_down` (up/down line navigation), and `to_1based`
//! (the proven 0→1-based ANSI conversion) — all L3, plus the file load/save
//! boundaries `read_file`/`write_file` (L1). `editor_multiline_enter_up_nav_and_ctrl_s_save`
//! grounds the runnable proof: Enter inserts a `\n` (the cursor drops to row 2), the
//! up arrow moves to the same column on the previous line, and Ctrl-S saves the
//! multi-line buffer (the `\n` round-trips through the file).
//!
//! The #90 thesis — the editor's bug-prone logic (display + input + nav/layout) is
//! proven; only the raw read/write/ioctl/open syscalls are trusted:
//!
//!   * `forge check editor.th` certifies:
//!       - the verified edit core (`Buffer`, `insert_str`, `backspace`,
//!         `move_left`, `move_right`) at **L3** (cursor math + length deltas proven);
//!       - the verified render-frame (`render_frame`) at **L3** — the thesis: the
//!         display-frame construction is proven Thermite, not trusted glue (the C4
//!         cursor coordinate `(b.cursor+1).to_string()` now discharges the bounded
//!         `concat` §4.2 CAP because `u64_to_string`'s `ens` bounds the formatted
//!         length `<= 20`, blocker #105);
//!       - the verified decode (`decode`) at **L3** — the keystroke interpretation
//!         is a pure total function, proven;
//!       - the minimal trusted syscall boundary (`raw_mode_on`, `raw_mode_off`,
//!         `read_key_raw`, `write_frame`) at **L1 boundary** (the foreign termios /
//!         read / write bodies, trusted-by-fiat, contract-stated);
//!       - the event loop `run` (`fx diverge`) at **L1** = partial correctness (the
//!         #88 cap — not L0 `WeakContract`).
//!   * `forge build editor.th --entry run` compiles (`render_frame(&Buffer)` borrows
//!     `b`, no E0382) and the produced binary runs with piped keystrokes — insert,
//!     a left arrow, a mid-text insert (splice), backspace, Ctrl-Q — and the frames
//!     reflect the L3-proven edits.
//!
//! The termios boundary needs `ioctl` — now granted (#106/#132): `raw_mode_on`/
//! `raw_mode_off` call `tcgetattr`/`tcsetattr`, which on Linux issue the `ioctl`
//! syscall (16). They now declare `fx term` (the dedicated #106 terminal-control
//! atom), whose `forge/src/sandbox.rs` `TERM_SYSCALLS = {ioctl:16}` widening grants
//! `ioctl`. So the sandboxed binary (default seccomp, no `--no-sandbox`) runs clean:
//! raw mode enters (the `ioctl` allowed), keys read (`read`), edits apply (the L3
//! ops), frames write (`write`), and Ctrl-S saves (`openat`/`write`). The grant is
//! scoped to the effect — a `pure`/`read`/`write`/`net` program's `ioctl` is still
//! SIGSYS-killed. The wrapper's own non-TTY handling (`tcgetattr` returns ENOTTY ->
//! the wrapper returns 1, no crash) is exercised by the piped (non-TTY) stdin.
//!
//! And the diverge cap is diverge-only (not a Goodhart bypass —
//! `goal.md` R-DEFER-9):
//!
//!   * a non-diverge weak-contract fn still rejects at L0 `WeakContract`;
//!   * a normal loop fn without a strictly-decreasing `dec` still fails termination;
//!   * `conformance/sum.th` / `binary_search.th` still certify L3 (the corpus oracle
//!     is unperturbed — the `u64_to_string` upper-bound strengthening did not break
//!     the total corpus).
//!
//! Driving the built `forge` binary (not a library API) keeps `forge` a pure `bin`
//! crate and exercises the CLI surface. The cert-level checks run verus; if
//! verus is absent they skip with a logged reason (the `check_conformance.rs`
//! precedent) — never panic on a missing solver. `tests/` is not anti-pattern-gated,
//! so `unwrap`/`expect`/`panic!` are fine here (R-APG-2). Expected levels trace to
//! the design (`.design/forge/check.md` AC-7 / `degrade-ladder.md` AC-8) + the #90
//! thesis + the provers' output, never copied from forge's own output (R-CHAR-3).

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

/// `true` iff verus can be located (mirrors `check_conformance.rs`). Skips with a
/// logged reason otherwise — a missing solver is never a test failure (R-CODE-4).
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

/// `true` iff the `forge build --entry` runnable artifact can link + run here. The
/// #57 runtime seccomp sandbox (`forge/src/sandbox.rs`) is native Linux only, with
/// generated filters for x86_64 and aarch64. The emitted runner does not link off
/// Linux (`Undefined symbols: _prctl` on macOS).
/// The build+run tests SKIP with an explicit warning on any non-Linux platform —
/// full acceptance OF the build+run PATH requires LINUX CI. Mirrors the
/// `verus_present()` skip precedent (a missing capability is a logged skip, not a
/// panic, R-CODE-4).
fn linux_build_run_supported(test: &str) -> bool {
    if cfg!(target_os = "linux") && (cfg!(target_arch = "x86_64") || cfg!(target_arch = "aarch64"))
    {
        return true;
    }
    eprintln!(
        "SKIP {test}: the #57 runtime seccomp sandbox supports x86_64/aarch64 Linux \
         runners only (the `forge build --entry` runner emits a raw `prctl` seccomp \
         prelude). FULL ACCEPTANCE OF THE BUILD+RUN PATH REQUIRES SUPPORTED LINUX CI; \
         `cargo test` on this platform skips the runnable end-to-end twin."
    );
    false
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

    // The verified logic — every total edit op, the render-frame, and the decode are
    // L3 (the #90 thesis: the editor's bug-prone display + input logic is proven, not
    // trusted glue). `render_frame` L3 is the thesis — it discharges only because
    // `u64_to_string`'s `ens` now bounds the formatted length `<= 20` (blocker #105),
    // so the bounded `concat` §4.2 CAP precondition holds.
    for op in [
        "Buffer",
        "insert_str",
        "backspace",
        "move_left",
        "move_right",
        // The multi-line nav / layout core (#125): the verified row/col scans + the
        // up/down line navigation + the proven 1-based ANSI conversion. The editor's
        // navigation + cursor-layout logic is proven, not trusted glue.
        "count_nl",
        "line_start",
        // The #126 spec twin of `line_start` (the `\n`-scan as a `spec fn`) — proves
        // the toolchain now lowers a String-scanning spec fn (`byte_at` over a
        // `&String` param) so `cursor_col`'s `ens` can name the exact line start.
        "spec_line_start",
        "line_end",
        "min2",
        "cursor_row",
        "cursor_col",
        "move_up",
        "move_down",
        "to_1based",
        "render_frame",
        "decode",
        // The #270 spec twins added in the coordinated #269+#270 arc — each lowers +
        // certifies L3 on its own, and each pins its exec partner's `ens` so the new
        // F-IDENT identity-return mutants are killed (the §7 battery's #269 widening).
        // `spec_count_nl`/`spec_cursor_row` pin the row scan; `spec_line_end` pins the
        // line-end scan (killing `line_end`'s `return i`/`return n` identities);
        // `spec_min2` pins the clamp; `spec_move_up_from`/`spec_move_up_target` and
        // `spec_move_down_from`/`spec_move_down_target` pin the navigation target
        // cursor (killing the review's `move_up`/`move_down` `return b` escape).
        "spec_count_nl",
        "spec_cursor_row",
        "spec_line_end",
        "spec_min2",
        "spec_move_up_from",
        "spec_move_up_target",
        "spec_move_down_from",
        "spec_move_down_target",
    ] {
        assert_eq!(
            level_of(&certs, op),
            "L3",
            "the verified editor-logic item `{op}` must certify L3 (the #90/#125/#270 thesis)"
        );
    }

    // #270 (the coordinated #269+#270 arc) — the navigation contracts are now pinned
    // to their exact target cursors, so the §7 F-IDENT battery's identity-return
    // mutants are killed, not survivors. Pre-arc (the outside review's item 5) a
    // literal `return b` provably satisfied `move_up`/`move_down`'s loose `ens`
    // (`result.cursor <= b.cursor` / `<= b.text.len()`) — the weak-contract escape —
    // and `return i`/`return n` satisfied `line_end`'s bounds-only `ens`. Post-arc
    // each carries an exact `ens result(.cursor) == spec_<fn>(...)` twin, so every
    // scored mutant is killed (no surviving identity). The expectation traces to the
    // design authority (`.design/forge/mutation-scoring.md` AC-7/AC-8 + the #270
    // tightening) + the provers' verdict, never copied from forge's own output
    // (R-CHAR-3): the asserted property is "no identity-return mutant survives".
    for op in ["move_up", "move_down", "line_end"] {
        let cert = find_cert(&certs, op);
        let survivor = cert["contract_quality"]["survivor"].as_str().unwrap_or("");
        assert!(
            !survivor.contains("identity of param")
                && !survivor.contains("return i")
                && !survivor.contains("return n")
                && !survivor.contains("return b"),
            "the #270-tightened `{op}` must KILL its F-IDENT identity-return mutant \
             (the review's weak-contract escape is closed), no surviving identity: \
             survivor={survivor:?}\n{cert:#?}"
        );
        // The tightened item still certifies L3 (the pin is provable by the exec body
        // — the families enable scoring + kill the escape, they do not over-gate).
        assert_eq!(
            cert["level"],
            Value::from("L3"),
            "the #270-tightened `{op}` certifies L3 (exact-target pin proven):\n{cert:#?}"
        );
    }

    // #126 — `cursor_col`'s contract is now pinned, not merely bounded: the
    // `ens result == b.cursor - spec_line_start(&b.text, 0, b.cursor, 0)` ties the
    // column to the exact line start (the verified spec twin), so the §7 mutation
    // gate scores it 4/4 — the surviving return-0 (and return-cursor) mutant is
    // killed (it no longer satisfies the equality). The ratio is the design
    // authority's non-vacuity floor (R-DEFER-9), not copied from forge's output.
    assert_eq!(
        find_cert(&certs, "cursor_col")["contract_quality"]["mutants_killed"],
        Value::from("4/4"),
        "the PINNED `cursor_col` (ens == cursor - spec_line_start) must score 4/4 — \
         the return-0 mutant is killed (#126):\n{:#?}",
        find_cert(&certs, "cursor_col")
    );

    // `decode` is a pure total function (the keystroke interpretation, proven).
    assert_eq!(
        find_cert(&certs, "decode")["effects"],
        Value::from(vec!["pure"]),
        "`decode` is a PURE total function (fx pure)"
    );

    // The minimal trusted syscall boundary — L1, boundary:true (foreign termios /
    // read / write bodies, trusted-by-fiat).
    for prim in [
        "raw_mode_on",
        "raw_mode_off",
        "read_key_raw",
        "write_frame",
        // The file load / save boundaries (#125) — extern-C `std::fs` read/write,
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

    // The #88 cap — `run` (fx diverge) is L1 = partial correctness: not L0
    // `WeakContract`, not a forced L3, and not a boundary fn.
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
// Deliverable 1b — the `bytes_eq` content pins (#276, review items 2+3): the edit
// ops now certify their result's bytes, not merely its length. `.design/basis/
// 07-strings.md` AC-14 (insert_str three conjuncts each L3), AC-15 (backspace +
// render_frame payload pins L3), REQ-18 (the built-in `bytes_eq` + the prove-once
// `lemma_bytes_eq_bridge`), REQ-19 (the contract-keyed body-start citation), #279
// (the field-access `&result.text`/`&b.text` operand byte-view). The whole editor
// still certifies L3 (O-1) and the content windows kill content-substitution
// mutants a length pin cannot (O-5 non-vacuity).
//
// The expected shapes trace to the design authority (the manifest pin list +
// 07-strings REQ-18 pin shapes + the `conformance/bytes_eq_demo.th::buf_prefix_pin`
// field-access oracle that certifies the exact editor pattern), never copied from
// forge's own output (R-CHAR-3): the asserted property is "the content-pinned op
// certifies L3 with no surviving mutant", and the source carries the design's
// verbatim pin shapes.
// ----------------------------------------------------------------------------

#[test]
fn editor_content_pins_present_in_source() {
    // The editor's three content-bearing edit ops carry the exact `bytes_eq` pin
    // shapes the manifest / 07-strings REQ-18 spell — over the field-access operand
    // (`&result.text`/`&b.text`, the editor's Buffer-wrapped String, #279). These
    // check content in addition to the existing length pins: the bytes are certified
    // byte-for-byte. A reviewer reading the source must see the windows verbatim, so
    // a silent regression to a length-only pin (the O-5 cheat) is caught here.
    let src = std::fs::read_to_string(editor_th()).expect("read editor.th");

    // The operand spelling is the bare-field form (`result.text`/`b.text`/`ins`, not
    // `&result.text`) — one of #279's four sanctioned operand shapes. The editor is
    // the keystone that must both `forge check` and `forge build --entry run`; the
    // bare-field form lowers through both the L3 spec byte-view and the L1
    // exec runtime-check twin, whereas the `&`-prefixed form trips an exec-twin
    // `&`-strip gap at build time (spillover finding, a thermite-lower follow-up).

    // insert_str — unchanged-prefix / inserted-run / shifted-suffix (AC-14).
    assert!(
        src.contains("ens bytes_eq(result.text, b.text, 0, 0, b.cursor)"),
        "insert_str must pin the UNCHANGED PREFIX text[0..cursor) byte-for-byte"
    );
    assert!(
        src.contains("ens bytes_eq(result.text, ins, b.cursor, 0, ins.len())"),
        "insert_str must pin the INSERTED RUN ins[0..ins.len()) at the cursor"
    );
    assert!(
        src.contains(
            "ens bytes_eq(result.text, b.text, b.cursor + ins.len(), b.cursor, b.text.len() - b.cursor)"
        ),
        "insert_str must pin the SHIFTED SUFFIX text[cursor..end) at cursor+ins.len()"
    );

    // backspace — unchanged-prefix (0..cursor-1) / shifted-suffix (AC-15).
    assert!(
        src.contains("ens bytes_eq(result.text, b.text, 0, 0, b.cursor - 1)"),
        "backspace must pin the UNCHANGED PREFIX text[0..cursor-1) byte-for-byte"
    );
    assert!(
        src.contains(
            "ens bytes_eq(result.text, b.text, b.cursor - 1, b.cursor, b.text.len() - b.cursor)"
        ),
        "backspace must pin the SHIFTED SUFFIX text[cursor..end) pulled back one byte"
    );

    // render_frame — the payload pin at the post-clear offset 7 (AC-15). The leading
    // `clear` is the fixed 7-byte escape, so the buffer body lands at offset 7.
    assert!(
        src.contains("ens bytes_eq(result, b.text, 7, 0, b.text.len())"),
        "render_frame must pin the WHOLE buffer text VERBATIM at the post-clear offset 7"
    );
}

#[test]
fn editor_content_pinned_ops_still_certify_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — editor content-pin cert-oracle not run.");
        return;
    }
    // The content pins discharge at verus (the #277 slice/concat byte-content
    // ens + the #278 `lemma_bytes_eq_bridge` + the #279 field-access operand view all
    // landed): each content-pinned op still certifies L3 — the windows are proven,
    // not asserted (R-DEFER-9). The whole editor exits 0 (O-1). Expected = L3 from
    // the design (AC-14/AC-15), the verdict from live verus, never copied from forge.
    let (code, certs) = run_check_json(&editor_th());
    assert_eq!(
        code,
        Some(0),
        "the content-pinned editor STILL certifies clean (exit 0):\n{certs:#?}"
    );
    for op in ["insert_str", "backspace", "render_frame"] {
        let cert = find_cert(&certs, op);
        assert_eq!(
            cert["level"],
            Value::from("L3"),
            "the `bytes_eq`-content-pinned `{op}` must certify L3 — the byte windows \
             discharge via the prove-once bridge (AC-14/AC-15):\n{cert:#?}"
        );
        // O-2/O-3: no new survivor. The content pins add byte-level checks to the
        // length pins; the §7 scored ratio is unperturbed and carries no survivor.
        let survivor = cert["contract_quality"]["survivor"].as_str().unwrap_or("");
        assert!(
            survivor.is_empty(),
            "the content-pinned `{op}` must have NO surviving mutant (O-2/O-3): \
             survivor={survivor:?}\n{cert:#?}"
        );
    }
}

#[test]
fn editor_content_pins_are_nonvacuous_content_teeth() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — content-teeth non-vacuity not run.");
        return;
    }
    // O-5 / AC-16: the content pins are non-vacuous — a length-preserving content
    // mutant dies. We mutate insert_str's body to the head/tail-swap
    // (`tail ++ ins ++ head`): the same length (so the length pins `result.text.len()
    // == b.text.len() + ins.len()` still hold), but the bytes are scrambled — every
    // `bytes_eq` window must fail. If this mutant survived, the pins would be
    // length-only in disguise (the exact O-5 cheat). The expected verdict (not L3 —
    // the swap fails the content windows) traces to AC-16 (the content mutant fails
    // verus), never copied from forge.
    let swapped = "\
struct Buffer { text: String, cursor: u64 }\n  \
  inv cursor <= text.len() && text.len() <= 1_000_000\n\n\
fn insert_str(b: Buffer, ins: String) -> Buffer\n  \
  req b.text.len() + ins.len() <= 1_000_000\n  \
  ens result.text.len() == b.text.len() + ins.len()\n  \
  ens bytes_eq(result.text, b.text, 0, 0, b.cursor)\n  \
  ens bytes_eq(result.text, ins, b.cursor, 0, ins.len())\n  \
  ens bytes_eq(result.text, b.text, b.cursor + ins.len(), b.cursor, b.text.len() - b.cursor)\n  \
  fx  alloc\n{\n  \
  let n: u64 = ins.len();\n  \
  let head: String = b.text.slice(0, b.cursor);\n  \
  let tail: String = b.text.slice(b.cursor, b.text.len());\n  \
  Buffer { text: tail.concat(ins).concat(head), cursor: b.cursor + n }\n}\n";
    let fixture = write_fixture("insert_swap_mutant", swapped);
    let (_code, certs) = run_check_json(&fixture);
    let cert = find_cert(&certs, "insert_str");
    assert_ne!(
        cert["level"],
        Value::from("L3"),
        "the head/tail-SWAP mutant (same length, scrambled bytes) must NOT certify \
         L3 — the content windows are non-vacuous teeth a length pin cannot fake \
         (O-5 / AC-16):\n{cert:#?}"
    );
    let _ = std::fs::remove_file(&fixture);
}

// ----------------------------------------------------------------------------
// Deliverable 2 — `forge build editor.th --entry run`: compiles (no E0382, the
// `render_frame(&Buffer)` borrow) + runs with piped keystrokes (arrow-move +
// mid-text splice). (#90; #105 divergence 2; 08-runnable-effect-link.md.)
// ----------------------------------------------------------------------------

#[test]
fn editor_builds_and_runs_arrow_move_then_splice() {
    if !linux_build_run_supported("editor_builds_and_runs_arrow_move_then_splice") {
        return;
    }
    // rustc is always present (no skip; the build_conformance.rs precedent). This is
    // the proof: a verified editor that runs fully sandboxed (#106/#132). The
    // termios boundary's `ioctl` (16) is now granted by the editor's `fx term`
    // (raw_mode_on/off declare it), so the binary builds with the default seccomp
    // sandbox (no --no-sandbox) and runs clean — every syscall it issues is granted
    // by its transitive fx (ioctl by term, read/openat by read(input), write/openat
    // by write(output), the heap by the baseline).
    let editor = editor_th();
    let (ok, stdout, stderr) =
        run_forge_build(&[editor.to_str().unwrap(), "--entry", "run", "--json"]);
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

    // Run it with piped keystrokes: insert 'a','b'; a left arrow (ESC [ D =
    // 0x1b 0x5b 0x44 -> decode 1003, cursor moves left to between 'a' and 'b'); insert
    // 'X' (the L3 `insert_str` splices mid-text -> "aXb"); backspace (0x7f -> decode
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
    // The mid-text splice: after the left arrow + 'X' the buffer is "aXb" (the proven
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
// Deliverable 2b — the multi-line session (#125): Enter inserts a `\n` (the cursor
// drops to the next row), the up arrow moves the cursor to the same column on the
// previous line (the L3 `move_up` over the verified row/col scans), and Ctrl-S
// saves the multi-line buffer to a file (the `os::write_file` boundary). The frames
// show two lines and the cursor moving between them; the saved file round-trips the
// `\n`. (#125; the verified nav/layout core L3 + the file boundary L1.)
// ----------------------------------------------------------------------------

#[test]
fn editor_multiline_enter_up_nav_and_ctrl_s_save() {
    if !linux_build_run_supported("editor_multiline_enter_up_nav_and_ctrl_s_save") {
        return;
    }
    let editor = editor_th();
    // Fully sandboxed (#106/#132): no --no-sandbox; the editor's `fx term` grants the
    // termios `ioctl`, and read(input)/write(output) grant the file load/save syscalls.
    let (ok, stdout, stderr) =
        run_forge_build(&[editor.to_str().unwrap(), "--entry", "run", "--json"]);
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

    // Keystrokes: 'a','b'; Enter (CR 0x0d -> decode 1004 -> insert "\n"); 'c','d';
    // up arrow (ESC [ A = 0x1b 0x5b 0x41 -> decode 1000 -> move_up); Ctrl-S (0x13 ->
    // decode 19 -> write_file save); Ctrl-Q (0x11 -> decode 17 -> clean quit).
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
    // produce the `\x1b[2;1H` coordinate (a second row, the multi-line proof).
    assert!(
        stdout.contains("\x1b[2;1H"),
        "the frame after Enter must position the cursor on row 2 (the L3 cursor_row \
         counted the inserted newline):\nstdout:{stdout:?}"
    );
    // After the up arrow the cursor returns to row 1 (col 3) — the L3 `move_up` walked
    // the verified line boundaries to the same column on the previous line.
    assert!(
        stdout.contains("\x1b[1;3H"),
        "the frame after UP must position the cursor back on row 1 col 3 (the L3 \
         `move_up` over the verified row/col scans):\nstdout:{stdout:?}"
    );
    // Ctrl-S saved the multi-line buffer to the file (the `os::write_file` boundary);
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
// #88 honesty — the diverge cap is diverge-only (not a Goodhart bypass).
// (.design/forge/check.md AC-7(b)(c)(d); degrade-ladder.md AC-8; R-DEFER-9.)
// ----------------------------------------------------------------------------

#[test]
fn non_diverge_weak_contract_still_rejects_l0_weakcontract() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — non-diverge weak-contract regression not run.");
        return;
    }
    // The AC-7(b) fixture: a total `fx pure` fn with a loose `ens` and no `diverge`.
    // The §7 mutation gate must still bite it (a `return 0`-style mutant survives the
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
    // The AC-7(c) fixture: a normal (non-diverge) fn with a `while` loop whose `dec`
    // measure does not strictly decrease (`dec n`, constant). Verus must still demand
    // a strictly-decreasing measure and fail — the #87 termination exemption is
    // diverge-only, and the #88 diverge L1 cap does not relax it for any other fn.
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
    // The AC-7(d) anchor: the total corpus (no `diverge`, `dec` present) is unchanged
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
