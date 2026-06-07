//! acto-critic divergence test for cluster C4 (#94): `u64_to_string` DISPLAY ORDER.
//!
//! Commit `a6b598c` shipped a GENUINE round-trip proof for `n.to_string()`
//! (`parse_le(result@) == n`, L3, no proof cheat) — but the produced byte sequence
//! is built **LSB-first and is NEVER reversed**, so the formatter emits the digits
//! of a number in REVERSED order. `42` materializes as the bytes `[50, 52]`
//! (ASCII `'2'`, `'4'`) — which, read as a human / terminal decimal (MSB-first),
//! reads "24", not "42".
//!
//! This is a "proven-but-wrong-output" gap. The round-trip `ens` is satisfied
//! because `parse_le` is itself LSB-first, so a reversed byte sequence still parses
//! back to `n`; the proof is real, but the OUTPUT is unusable for the design's
//! stated purpose.
//!
//! AUTHORITY — `.design/basis/07-strings.md` REQ-8 (`u64_to_string` … REQ-8 prose):
//!   "The surface emits the human-readable MSB-first decimal (the construction is
//!    LSB-first; the display form reverses — `parse_be(reverse(s)) == parse_le(s)`
//!    proved … so the displayed bytes round-trip against a big-endian parse)."
//! The Summary names the consumers: the editor's "ANSI cursor coordinates —
//! `ESC[<row>;<col>H` needs `u64`→decimal text" and "a number formatter /
//! calculator". A reversed decimal makes `ESC[<col>H` address the wrong column and
//! a calculator print "24" for 42 — the acceptance programs CANNOT use it.
//!
//! Per REQ-8 the displayed bytes are MSB-first: `42` → `[52, 50]` (`'4'` then
//! `'2'`). The ASCII codes are the design constant (`'0'` == 48, REQ-6 escape
//! table / the `+ 48u8` digit convention), NOT copied from forge's own output
//! (`goal.md` R-CHAR-3). The shipped `parse_be(reverse(s)) == parse_le(s)` bridge
//! lemma is the DISPLAY contract REQ-8 names but the formatter does not apply.
//!
//! FIX DIRECTION (for the generator, NOT this critic): build MSB-first with a
//! `parse_be` round-trip, OR reverse the LSB-first buffer before returning and
//! carry the proved `parse_be(reverse(s)) == parse_le(s)` bridge. State only.

use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// DIVERGENCE (highest value) — `n.to_string()` emits the decimal digits of `n` in
/// REVERSED order: `42` builds to `[50, 52]` (`'2','4'` == "24"), not the
/// human-readable MSB-first `[52, 50]` (`'4','2'` == "42") REQ-8 mandates.
///
/// Builds + RUNS a formatter on `n == 42` and asserts the produced byte sequence is
/// the MSB-first decimal of 42 (`[52, 50]`). FAILS against `a6b598c` (the L1
/// `u64_to_string` pushes `(m%10)+48` then `m/=10` and returns `data` un-reversed,
/// `thermite-lower/src/l1.rs` `emit_string_runtime_l1`).
///
/// AUTHORITY: `.design/basis/07-strings.md` REQ-8 — "The surface emits the
/// human-readable MSB-first decimal". MSB-first 42 == `[52, 50]`; ASCII `'4'`==52,
/// `'2'`==50 are the design digit constants (`+ 48u8`), not forge output (R-CHAR-3).
/// Tracking: #96.
#[test]
fn divergence_to_string_display_order_msb_first() {
    // REQ-8 (blocker #96): the surface round-trip is the MSB-first `parse_be` — the
    // displayed bytes round-trip against a big-endian parse. `u64_to_string` now
    // reverses the LSB construction buffer, carrying the proof via the
    // `parse_be(seq_reverse(s)) == parse_le(s)` bridge.
    let program = "fn show42() -> String\n  req true\n  ens parse_be(result) == 42\n  fx alloc\n{ let n: u64 = 42; n.to_string() }\n";
    let fixture =
        std::env::temp_dir().join(format!("forge_numfmt_order_{}.th", std::process::id()));
    std::fs::write(&fixture, program).expect("write fixture");

    let build = Command::new(forge_bin())
        .arg("build")
        .arg(&fixture)
        .arg("--entry")
        .arg("show42")
        .arg("--json")
        .output()
        .expect("spawn forge build");
    assert!(
        build.status.success(),
        "forge build --entry show42 must COMPILE:\nstderr:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let build_stdout = String::from_utf8_lossy(&build.stdout).to_string();
    let manifest: Value = serde_json::from_str(build_stdout.trim())
        .unwrap_or_else(|e| panic!("forge build --json not JSON: {e}\n{build_stdout}"));
    let artifact = PathBuf::from(
        manifest["artifact"]
            .as_str()
            .unwrap_or_else(|| panic!("no `artifact` in build manifest:\n{build_stdout}")),
    );

    let run = Command::new(&artifact)
        .output()
        .unwrap_or_else(|e| panic!("spawn built formatter `{}`: {e}", artifact.display()));
    let run_stdout = String::from_utf8_lossy(&run.stdout).to_string();
    let _ = std::fs::remove_file(&fixture);
    assert!(
        run.status.success(),
        "the formatter must exit clean:\nstdout:{run_stdout}\nstderr:{}",
        String::from_utf8_lossy(&run.stderr)
    );

    // REQ-8: the SURFACE emits the human-readable MSB-first decimal. For 42 the
    // MSB-first byte order is the most-significant digit first: '4' (ASCII 52) then
    // '2' (ASCII 50) => the byte sequence [52, 50]. The reversed (LSB-first) order
    // [50, 52] reads "24" and is the divergence. (ASCII '4'==52, '2'==50 are the
    // design `+ 48u8` digit constants, R-CHAR-3.)
    assert!(
        run_stdout.contains("52, 50"),
        "DESIGN 07-strings.md REQ-8: `n.to_string()` must emit the human-readable \
         MSB-first decimal — 42 => the byte sequence [52, 50] ('4' then '2'). The \
         shipped formatter emits the REVERSED LSB-first order (42 => [50, 52] == \
         \"24\"), which the editor's ANSI cursor coords and a calculator cannot use. \
         formatter output:\n{run_stdout}"
    );
}
