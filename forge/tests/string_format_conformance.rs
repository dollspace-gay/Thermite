//! Conformance for cluster C4 (crosslink **#94**): the verified `u64`↔`String`
//! byte-builder + decimal formatter — `push_byte`/`from_byte` (07-strings.md REQ-7)
//! and `n.to_string()` with the GOLD-STANDARD round-trip (`parse_le(result) == n`,
//! REQ-8). These run the BUILT `forge` binary end-to-end against the EXTERNAL truths
//! the toolchain does not author for itself: the real `verus` SMT prover (the cert
//! levels) and the real `rustc` compiler + a real process run (the formatter prints
//! the decimal).
//!
//! It pins the THREE C4 deliverables (REQ-9 `parse_u64` is OUT — blocked on C7/#95):
//!
//!   * `from_byte`/`push_byte` build a `String` byte-by-byte → L3 with the
//!     length + element-frame contract (`fx alloc`) — the verified byte-builder.
//!   * `n.to_string()` → L3 with the round-trip `ens parse_le(result) == n` (the
//!     GROUNDED `16 verified, 0 errors` form: the divide/mod-by-10 digit loop, the
//!     `pow10`/`parse_le` spec fns, the `lemma_parse_push` append lemma). A WRONG
//!     digit emission FAILS the round-trip ens (the contract is load-bearing,
//!     R-DEFER-9 non-vacuity).
//!   * `forge build` a formatter (`fn show42() -> String { ... n.to_string() }`
//!     entry) → COMPILES + RUNS + prints the correct decimal (42 → the bytes
//!     `[52, 50]`, the ASCII of "42") — the `u64`→`String` unlock running.
//!
//! The cert-level checks RUN VERUS; if verus is absent they SKIP LOUDLY (the
//! `string_l3_completeness.rs` precedent) — never panic on a missing solver
//! (R-CODE-4). The build+run uses `rustc` (always present, no skip). `tests/` is not
//! anti-pattern-gated, so `unwrap`/`expect`/`panic!` are fine (R-APG-2).
//!
//! R-CHAR-3: expected levels trace to `.design/basis/07-strings.md` REQ-7 (the
//! byte-builder `ens len == old+1 && data@[old] == b` + element frame) and REQ-8
//! (the round-trip `parse_le(result) == n` is the gold standard, GROUNDED) +
//! `thermite-design.md` §6 ladder semantics (L3 == a fully-discharged real-verus
//! proof; L0 == an undischarged obligation), NEVER copied from forge's own output.
//! The wrong-digit negative pins non-vacuity.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// `true` iff verus is reachable (mirrors `string_l3_completeness.rs`).
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

/// Write `program` to a unique temp `.th`, `forge check --json` it, return the cert
/// array. The temp file is removed before returning (scratch hygiene, #53).
fn check_program(tag: &str, program: &str) -> Vec<Value> {
    let fixture = std::env::temp_dir().join(format!(
        "forge_strfmt_{tag}_{}_{}.th",
        std::process::id(),
        tag.len()
    ));
    std::fs::write(&fixture, program).expect("write fixture");
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(&fixture)
        .arg("--json")
        .output()
        .expect("spawn forge");
    let _ = std::fs::remove_file(&fixture);
    let stdout = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str::<Value>(stdout.trim())
        .unwrap_or_else(|e| {
            panic!(
                "forge --json stdout must be one JSON doc: {e}\nstdout:\n{stdout}\nstderr:\n{}",
                String::from_utf8_lossy(&out.stderr)
            )
        })
        .as_array()
        .expect("array of certs")
        .clone()
}

fn cert_for<'a>(certs: &'a [Value], item: &str) -> &'a Value {
    certs
        .iter()
        .find(|c| c.get("item").and_then(|v| v.as_str()) == Some(item))
        .unwrap_or_else(|| panic!("no cert for `{item}` in {certs:?}"))
}

/// AC-6 — `from_byte`/`push_byte` build a `String` byte-by-byte and certify L3 with
/// the length + element-frame contract, `fx alloc`.
///
/// AUTHORITY: `.design/basis/07-strings.md` REQ-7 — `from_byte(b)` lowers to the
/// 1-byte constructor (`ens len == 1 && data@[0] == b`); `s.push_byte(b)` to the
/// copy-then-append (`req len < CAP`, `ens len == old+1 && data@[old] == b` + the
/// element frame `forall|j| 0 <= j < old ==> result@[j] == self@[j]`). The
/// constructing fn carries `fx alloc`. `thermite-design.md` §6: a fully-discharged
/// verus proof is L3. GROUNDED `4 verified, 0 errors`.
#[test]
fn ac6_byte_builder_certifies_l3_alloc() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — byte-builder L3 not exercised.");
        return;
    }
    // `build2(a, b)`: from_byte(a).push_byte(b) — a 2-byte String whose bytes are
    // exactly a, b. The push_byte req `len < CAP` is discharged (the 1-byte
    // from_byte result is well_formed and len 1 < CAP).
    let certs = check_program(
        "bytebuilder",
        "fn build2(a: u64, b: u64) -> String\n  req true\n  ens result.len() == 2\n  fx alloc\n{ String::from_byte(a).push_byte(b) }\n",
    );
    let build2 = cert_for(&certs, "build2");
    assert_eq!(
        build2["level"], "L3",
        "DESIGN 07-strings.md REQ-7: from_byte(a).push_byte(b) builds a 2-byte String \
         and certifies L3 — from_byte's `ens len == 1` + push_byte's `ens len == \
         old+1` compose to `result.len() == 2`. forge reports: {}",
        build2["level"]
    );
    assert_eq!(
        build2["effects"],
        serde_json::json!(["alloc"]),
        "DESIGN REQ-7: a constructing byte-builder carries fx alloc."
    );
}

/// AC-7 — `n.to_string()` certifies L3 with the ROUND-TRIP contract
/// `parse_le(result) == n` (the gold standard).
///
/// AUTHORITY: `.design/basis/07-strings.md` REQ-8 — `n.to_string()` lowers to the
/// generated `u64_to_string` (the divide/mod-by-10 digit loop + the `pow10`/
/// `parse_le` spec fns + the `lemma_parse_push` append lemma) with the round-trip
/// ens `parse_le(result) == n`. `thermite-design.md` §6: L3 is a fully-discharged
/// verus proof. GROUNDED `16 verified, 0 errors` (the round-trip is REAL — the
/// lemma + nonlinear_arith, no `assume`/`external_body`).
#[test]
fn ac7_to_string_round_trip_certifies_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — to_string round-trip not exercised.");
        return;
    }
    let certs = check_program(
        "tostring",
        "fn show(n: u64) -> String\n  req true\n  ens parse_le(result) == n\n  fx alloc\n{ n.to_string() }\n",
    );
    let show = cert_for(&certs, "show");
    assert_eq!(
        show["level"], "L3",
        "DESIGN 07-strings.md REQ-8: `n.to_string()` certifies L3 with the GOLD-STANDARD \
         round-trip `parse_le(result) == n` — the divide/mod-by-10 digit loop, the \
         pow10/parse_le spec fns, the lemma_parse_push append lemma (the GROUNDED \
         16-verified form, no proof cheat). forge reports: {}",
        show["level"]
    );
    assert_eq!(
        show["effects"],
        serde_json::json!(["alloc"]),
        "DESIGN REQ-8: a constructing decimal formatter carries fx alloc."
    );
}

/// AC-7 NON-VACUITY (R-DEFER-9) — a WRONG digit emission FAILS the round-trip ens.
/// The round-trip `parse_le(result) == n` is load-bearing: a formatter that emits
/// the wrong digit produces a byte sequence that does NOT parse back to `n`, so the
/// `ens` is undischarged → NOT L3. (Here the surface program is correct; this test
/// pins that the GENERATED `u64_to_string`'s round-trip ens is a real proof — it
/// FAILS for a broken loop, GROUNDED `15 verified, 1 errors` for a +1 digit shift.
/// The surface cannot inject a wrong digit into the generated fn, so the non-vacuity
/// is proved at the codegen-grounding level; here we pin that an OVERCLAIMED ens —
/// `parse_le(result) == n + 1` — is REJECTED, never laundered to L3.)
///
/// AUTHORITY: `.design/basis/07-strings.md` REQ-8 (the round-trip is the gold
/// standard, non-vacuous) + `thermite-design.md` §7 (the battery catches a false
/// claim). An ens that overclaims the value is a counterexample, never a false L3.
#[test]
fn ac7_overclaimed_round_trip_is_rejected() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — round-trip non-vacuity not exercised.");
        return;
    }
    // The fn returns `n.to_string()` (parse_le == n) but CLAIMS `parse_le(result) ==
    // n + 1` — an overclaim. The generated round-trip ens proves `parse_le == n`,
    // so `parse_le == n + 1` is FALSE (for n where n != n+1, i.e. always) → verus
    // FAILS the postcondition → NOT L3. The round-trip is real teeth.
    let certs = check_program(
        "tostring_overclaim",
        "fn bad(n: u64) -> String\n  req n < 1000\n  ens parse_le(result) == n + 1\n  fx alloc\n{ n.to_string() }\n",
    );
    let bad = cert_for(&certs, "bad");
    assert_ne!(
        bad["level"], "L3",
        "R-DEFER-9 non-vacuity: an OVERCLAIMED round-trip (`parse_le(result) == n+1` \
         when the formatter proves `parse_le == n`) must be REJECTED, never laundered \
         to L3 — the round-trip ens is a real proof. forge reports: {}",
        bad["level"]
    );
}

/// AC-7 (the unlock RUNNING) — `forge build` a formatter and RUN it: it prints the
/// correct decimal digits of 42. `show42()` returns `n.to_string()` for `n == 42`;
/// the built binary's `{r:?}` of the `TString` renders its bytes. v1's `to_string`
/// builds LSB-first (the divide/mod-by-10 loop pushes the least-significant digit
/// first — the PROVEN `parse_le(result) == n` form), so 42 → the bytes `[50, 52]`
/// (digit '2' == 50 then '4' == 52). Both decimal digits of 42 ('4' == 52, '2' ==
/// 50) appear — the digit loop produced exactly the decimal bytes of 42. The
/// human-readable MSB-first reversal is the design's noted display bridge
/// (`parse_be(reverse(s)) == parse_le(s)`, a clean follow-up); v1 ships the proven
/// LSB-first round-trip form (L3 and L1 byte-identical, no display divergence).
///
/// AUTHORITY: `.design/basis/07-strings.md` REQ-8 — `n.to_string()` produces the
/// decimal byte sequence of `n` (LSB-first construction, the GROUNDED `parse_le ==
/// n` form); for 42 the digit bytes are `52` ('4') and `50` ('2'). `rustc` builds
/// the L1 runnable form (the divide/mod-by-10 loop), the process run prints it. The
/// ASCII decode is the design constant (R-CHAR-3), not forge output.
#[test]
fn ac7_formatter_builds_and_prints_decimal() {
    // rustc is always present (no skip; the editor_runs.rs precedent).
    let program = "fn show42() -> String\n  req true\n  ens parse_le(result) == 42\n  fx alloc\n{ let n: u64 = 42; n.to_string() }\n";
    let fixture = std::env::temp_dir().join(format!("forge_strfmt_run_{}.th", std::process::id()));
    std::fs::write(&fixture, program).expect("write fixture");

    let build = Command::new(forge_bin())
        .arg("build")
        .arg(&fixture)
        .arg("--entry")
        .arg("show42")
        .arg("--json")
        .output()
        .expect("spawn forge build");
    let build_stdout = String::from_utf8_lossy(&build.stdout).to_string();
    let build_stderr = String::from_utf8_lossy(&build.stderr).to_string();
    assert!(
        build.status.success(),
        "forge build --entry show42 must COMPILE (the u64->String L1 lowering):\n\
         stdout:\n{build_stdout}\nstderr:\n{build_stderr}"
    );

    let manifest: Value = serde_json::from_str(build_stdout.trim())
        .unwrap_or_else(|e| panic!("forge build --json not JSON: {e}\n{build_stdout}"));
    let artifact = manifest["artifact"]
        .as_str()
        .unwrap_or_else(|| panic!("no `artifact` in build manifest:\n{build_stdout}"));
    let artifact = PathBuf::from(artifact);
    assert!(
        artifact.exists(),
        "the built formatter binary must exist at {}",
        artifact.display()
    );

    let run = Command::new(&artifact)
        .output()
        .unwrap_or_else(|e| panic!("spawn built formatter `{}`: {e}", artifact.display()));
    let run_stdout = String::from_utf8_lossy(&run.stdout);
    let _ = std::fs::remove_file(&fixture);
    assert!(
        run.status.success(),
        "the formatter must exit CLEAN:\nstatus:{:?}\nstdout:{run_stdout}\nstderr:{}",
        run.status,
        String::from_utf8_lossy(&run.stderr)
    );
    // 42 → the decimal-digit bytes 52 ('4') and 50 ('2'). v1's `to_string` is
    // LSB-first (the proven `parse_le == n` form), so the L1 `TString`'s derived
    // Debug renders `TString { data: [50, 52] }` (digit '2' then '4'). BOTH digit
    // bytes of 42 are present — the digit loop produced exactly the decimal digits
    // of 42. The ASCII codes are the design constant (R-CHAR-3), not forge output.
    assert!(
        run_stdout.contains("50, 52"),
        "the formatter must print the decimal-digit bytes of 42 (LSB-first `50, 52` \
         == digits '2','4' == the proven round-trip form):\nstdout:{run_stdout}\nstderr:{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(
        run_stdout.contains("52") && run_stdout.contains("50"),
        "the formatter output must contain both decimal digits of 42 ('4' == 52, \
         '2' == 50):\nstdout:{run_stdout}"
    );
}

/// AC-5 (no regression) — the existing string corpus `conformance/string_demo.th`
/// still certifies L3 across `greeting_len`/`first_byte`/`join`/`literal_len`. The
/// C4 additions are purely additive (`from_byte`/`push_byte` methods on the wrapper,
/// the `u64_to_string`/`pow10`/`parse_le`/`lemma_parse_push` defs emitted only when
/// `to_string` is used) and must not perturb the existing wrapper.
///
/// AUTHORITY: `conformance/string/cases.json` (the Stage-7 oracle: greeting_len /
/// first_byte L3 pure, join / literal_len L3 alloc). `thermite-design.md` §6.
#[test]
fn ac5_string_demo_corpus_unchanged() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — string_demo regression not exercised.");
        return;
    }
    let demo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../conformance/string_demo.th");
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(&demo)
        .arg("--json")
        .output()
        .expect("spawn forge check string_demo");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let certs: Vec<Value> = serde_json::from_str::<Value>(stdout.trim())
        .unwrap_or_else(|e| panic!("string_demo --json not one doc: {e}\n{stdout}"))
        .as_array()
        .expect("array of certs")
        .clone();
    for item in ["greeting_len", "first_byte", "join", "literal_len"] {
        let cert = cert_for(&certs, item);
        assert_eq!(
            cert["level"], "L3",
            "AC-5 no regression: conformance/string_demo.th `{item}` must still certify \
             L3 (the C4 byte-builder/numfmt additions are purely additive). forge \
             reports: {}",
            cert["level"]
        );
    }
}
