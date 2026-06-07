//! THE COMPOSE-ANY-PROGRAM PROOF (crosslink **#103**): three ACCEPTANCE PROGRAMS
//! that prove the verified-primitive basis (C1–C7) composes into real programs —
//! a `u64` decimal FORMATTER (`examples/formatter/format.th`), a CALCULATOR core
//! (`examples/calculator/calc.th`), and a line/CSV PARSER
//! (`examples/parser/parse_lines.th`). Each is grounded against the two EXTERNAL
//! truths the toolchain does not author for itself: the real `verus` SMT prover
//! (the `forge check` cert levels + verus-on-the-lowering for the thin split
//! caller) and the real `rustc` compiler + a real process run (the built binaries).
//!
//! THE THREE PROGRAMS:
//!
//!   * FORMATTER — `format(n) ens parse_be(result) == n` certifies **L3** (the C4
//!     round-trip), and `forge build --entry format_42`/`format_0`/`format_1000000`
//!     COMPILES + RUNS, printing the human-readable MSB-first decimal: 42 → [52,50]
//!     == "42", 0 → [48] == "0", 1000000 → [49,48,48,48,48,48,48] == "1000000"
//!     (REQ-8 / blocker #96). The formatter COMPOSES CLEANLY end-to-end.
//!
//!   * CALCULATOR — `add(a, b) ens result is Some && match { Some(v) => v ==
//!     parse_be(a) + parse_be(b) }` certifies **L3** (the C7 nested-`match` parse +
//!     sum; the sum is PINNED). The arithmetic core `add_vals`/`add_2_3` also
//!     certifies L3 and BUILDS + RUNS → `Some(5)` (2+3), `Some(300)` (100+200). The
//!     GAP NOW CLOSED (crosslink #104): the FULL `calc.th` — including the
//!     STRING-PARSE front-end `add` whose contract names the C7 spec fns
//!     `all_digits` / `parse_be` / the free `parse_u64` — now `forge build`s + RUNS
//!     end-to-end, because those C7 spec fns now have an L1 (runtime / build) EXEC
//!     twin (`emit_string_runtime_l1`'s C7 block). NOT faked.
//!
//!   * PARSER — `has_sep(s, sep) ens result == contains_sub(s, sep)` certifies **L3**
//!     via the full §7-mutation-scored `forge check` ladder; `fields(s, sep) ens
//!     result.len() == 1 + count_sep(s, sep)` (the C5 split count-bound) certifies L3
//!     under REAL VERUS on the lowering (the thin `{ s.split(sep) }` caller is not
//!     §7-mutation-scoreable, the documented split-caller precedent). The runnable
//!     `split_abc` BUILDS + RUNS → 3 pieces ([97],[98],[99] == "a","b","c") for
//!     "a,b,c" split on ',' (byte 44). The full file (incl. the `fields` count-bound
//!     + `has_sep` substring contracts) now `forge build`s end-to-end too (#104).
//!
//! THE GAP NOW CLOSED (crosslink #104, the C5/#102 + C7/#95 build-side cluster): the
//! C5/C7 CONTRACT spec fns — `count_sep`, `sep_free`, `occurs_at`, `contains_sub`,
//! `all_digits`, `is_digit`, the free `parse_u64`, and `parse_be` in a C7
//! (non-numfmt) context — now HAVE an L1 runnable EXEC twin in `thermite-lower`'s
//! `emit_string_runtime_l1` (the C5 block gated on `program_uses_string_search`, the
//! C7 block on `program_uses_parse`; each twin computes the same value as its spec
//! body over the runtime `Vec<u8>`). Because `forge build` lowers EVERY fn in a file
//! to its always-active runtime `thermite_check!`, a program whose contracts name a
//! C5/C7 spec fn now resolves the named fn and builds. The formatter (C4) is
//! unaffected; the calculator's parse front-end and the parser's count-bound entry
//! now BUILD + RUN. The `forge check` ladder is unchanged (L3 — the SPEC twins +
//! verus proofs carry the check path; #104 touched only the L1/exec mirror).
//!
//! The verus checks SKIP LOUDLY when verus is absent (the `string_format_conformance`
//! / `editor_runs` precedent) — never panic on a missing solver (R-CODE-4). The
//! build + run uses `rustc` (always present, no skip). `tests/` is not
//! anti-pattern-gated, so `unwrap`/`expect`/`panic!` are fine here (R-APG-2).
//!
//! R-CHAR-3: expected levels trace to `.design/basis/07-strings.md` REQ-8 (the
//! round-trip), REQ-13/REQ-15 (the predicate / count-bound), `.design/basis/
//! 09-option-result.md` (the Option sum), and `thermite-design.md` §6 (L3 == a
//! fully-discharged real-verus proof) — NEVER copied from forge's own output. The
//! decimal byte values (52,50 / 48 / 49,48… / 97,98,99) are the ASCII design
//! constant. The build-gap error string is the rustc diagnostic for the un-lowered
//! C7 spec fn — the gap itself, not a forge self-assertion.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

fn formatter_th() -> PathBuf {
    repo_root().join("examples/formatter/format.th")
}

fn calculator_th() -> PathBuf {
    repo_root().join("examples/calculator/calc.th")
}

fn parser_th() -> PathBuf {
    repo_root().join("examples/parser/parse_lines.th")
}

/// `true` iff verus is reachable (mirrors `string_format_conformance.rs`).
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

fn verus_bin() -> PathBuf {
    if let Ok(p) = std::env::var("VERUS_BIN") {
        if Path::new(&p).exists() {
            return PathBuf::from(p);
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let p = PathBuf::from(home).join(".local/bin/verus");
        if p.exists() {
            return p;
        }
    }
    PathBuf::from("verus")
}

/// `forge check <file> --json`, returning the parsed cert array.
fn check_json(file: &Path) -> Vec<Value> {
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(file)
        .arg("--json")
        .output()
        .expect("spawn forge check");
    let stdout = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str::<Value>(stdout.trim())
        .unwrap_or_else(|e| {
            panic!(
                "forge check --json must emit one JSON doc: {e}\nstdout:\n{stdout}\nstderr:\n{}",
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
        .unwrap_or_else(|| panic!("no cert for `{item}` in {certs:#?}"))
}

fn level_of(certs: &[Value], item: &str) -> String {
    cert_for(certs, item)["level"]
        .as_str()
        .unwrap_or_else(|| panic!("cert for `{item}` has no string level"))
        .to_string()
}

/// `forge build <file> --entry <fn> --json`, returning `(ok, stdout, stderr)`.
fn build_entry(file: &Path, entry: &str) -> (bool, String, String) {
    let out = Command::new(forge_bin())
        .arg("build")
        .arg(file)
        .arg("--entry")
        .arg(entry)
        .arg("--json")
        .output()
        .expect("spawn forge build");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

fn artifact_of(stdout: &str) -> PathBuf {
    let v: Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("forge build --json not JSON: {e}\n{stdout}"));
    PathBuf::from(
        v["artifact"]
            .as_str()
            .unwrap_or_else(|| panic!("no `artifact` field in build manifest:\n{stdout}")),
    )
}

/// Write `program` to a unique temp `.th`, build the entry, run it, return the run
/// stdout. The temp file is removed before returning (#53). Used for the runnable
/// CORES (a minimal subset program — the full files now build too, #104).
fn build_run_fixture(tag: &str, program: &str, entry: &str) -> String {
    let fixture = std::env::temp_dir().join(format!(
        "forge_accept_{tag}_{}_{}.th",
        std::process::id(),
        tag.len()
    ));
    std::fs::write(&fixture, program).expect("write fixture");
    let (ok, stdout, stderr) = build_entry(&fixture, entry);
    let _ = std::fs::remove_file(&fixture);
    assert!(
        ok,
        "[{tag}] the runnable core must COMPILE (it names no un-lowered C5/C7 spec fn):\n\
         stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    let artifact = artifact_of(&stdout);
    assert!(
        artifact.exists(),
        "[{tag}] built binary missing at {}",
        artifact.display()
    );
    let run = Command::new(&artifact)
        .output()
        .unwrap_or_else(|e| panic!("[{tag}] spawn built binary `{}`: {e}", artifact.display()));
    assert!(
        run.status.success(),
        "[{tag}] the binary must exit CLEAN:\nstatus:{:?}\nstdout:{}\nstderr:{}",
        run.status,
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    String::from_utf8_lossy(&run.stdout).to_string()
}

/// Run the real `verus` binary on a program's LOWERED Verus source, returning
/// `(success, combined_output)`. The thin `split` caller cannot be §7-mutation-
/// scored by `forge check`, so its L3 is established by verus on the lowering (the
/// `string_search_conformance.rs` precedent). R-CODE-4: the status is checked.
fn verus_on_lowered(tag: &str, program: &str) -> (bool, String) {
    let parsed = thermite_syntax::parse(program);
    assert!(
        parsed.is_clean(),
        "[{tag}] surface must parse: {:?}",
        parsed.errors
    );
    let verus_src = thermite_lower::lower(&parsed.program)
        .unwrap_or_else(|e| panic!("[{tag}] lower must succeed: {e:?}"));
    let rs = std::env::temp_dir().join(format!(
        "forge_accept_verus_{tag}_{}.rs",
        std::process::id()
    ));
    std::fs::write(&rs, &verus_src).expect("write lowered .rs");
    let out = Command::new(verus_bin())
        .arg(&rs)
        .current_dir(std::env::temp_dir())
        .output()
        .expect("spawn verus");
    let _ = std::fs::remove_file(&rs);
    if let Some(stem) = rs.file_stem() {
        let _ = std::fs::remove_file(std::env::temp_dir().join(stem));
    }
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.success(), combined)
}

// ============================================================================
// PROGRAM 1 — the FORMATTER. Composes CLEANLY: forge check L3 + build + run.
// ============================================================================

/// (a) `format(n) ens parse_be(result) == n` certifies L3 — the C4 round-trip.
/// AUTHORITY: `.design/basis/07-strings.md` REQ-8 (the round-trip is the gold
/// standard, GROUNDED `17 verified, 0 errors`); `thermite-design.md` §6 (L3 == a
/// discharged verus proof).
#[test]
fn formatter_round_trip_certifies_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — formatter L3 not exercised.");
        return;
    }
    let certs = check_json(&formatter_th());
    assert_eq!(
        level_of(&certs, "format"),
        "L3",
        "DESIGN 07-strings.md REQ-8: `format(n) ens parse_be(result) == n` certifies L3 \
         (the C4 round-trip — the decimal bytes parse back to exactly n). certs:\n{certs:#?}"
    );
    assert_eq!(
        cert_for(&certs, "format")["effects"],
        serde_json::json!(["alloc"]),
        "a constructing decimal formatter carries fx alloc (REQ-8)."
    );
}

/// (b) `forge build --entry format_42`/`_0`/`_1000000` COMPILES + RUNS, printing the
/// human-readable MSB-first decimal. AUTHORITY: `.design/basis/07-strings.md` REQ-8
/// (#96 — to_string reverses to MSB-first). The ASCII bytes are the design constant
/// (R-CHAR-3): '4'=52,'2'=50; '0'=48; '1'=49.
#[test]
fn formatter_builds_and_runs_each_value() {
    // rustc always present (no skip; the string_format_conformance precedent).
    let f = formatter_th();

    let (ok, stdout, stderr) = build_entry(&f, "format_42");
    assert!(
        ok,
        "format_42 must COMPILE:\nstdout:{stdout}\nstderr:{stderr}"
    );
    let art = artifact_of(&stdout);
    let run = Command::new(&art).output().expect("run format_42");
    let s = String::from_utf8_lossy(&run.stdout);
    assert!(
        run.status.success() && s.contains("52, 50"),
        "42 → the MSB-first decimal bytes [52, 50] (== '4','2' == \"42\"):\nstdout:{s}"
    );

    let (ok, stdout, _e) = build_entry(&f, "format_0");
    assert!(ok, "format_0 must COMPILE:\n{stdout}");
    let run = Command::new(artifact_of(&stdout))
        .output()
        .expect("run format_0");
    let s = String::from_utf8_lossy(&run.stdout);
    assert!(
        run.status.success() && s.contains("[48]"),
        "0 → the single byte [48] (== '0' == \"0\"):\nstdout:{s}"
    );

    let (ok, stdout, _e) = build_entry(&f, "format_1000000");
    assert!(ok, "format_1000000 must COMPILE:\n{stdout}");
    let run = Command::new(artifact_of(&stdout))
        .output()
        .expect("run format_1000000");
    let s = String::from_utf8_lossy(&run.stdout);
    assert!(
        run.status.success() && s.contains("49, 48, 48, 48, 48, 48, 48"),
        "1000000 → the MSB-first decimal bytes [49,48,48,48,48,48,48] (== \"1000000\"):\nstdout:{s}"
    );
}

// ============================================================================
// PROGRAM 2 — the CALCULATOR. forge check L3; build+run the arithmetic core;
// the STRING-PARSE front-end now builds + runs end-to-end (#104).
// ============================================================================

/// (a) `add(a, b)` (parse two digit strings + add) certifies L3 with the PINNED sum
/// contract, AND the arithmetic core `add_vals`/`add_2_3` certify L3. AUTHORITY:
/// `.design/basis/07-strings.md` REQ-9 + `.design/basis/09-option-result.md` (the
/// C7 parse round-trip + Option + spec-match-in-ens); `thermite-design.md` §6.
#[test]
fn calculator_sum_contract_certifies_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — calculator L3 not exercised.");
        return;
    }
    let certs = check_json(&calculator_th());
    assert_eq!(
        level_of(&certs, "add"),
        "L3",
        "DESIGN 09-option-result.md + 07-strings.md REQ-9: `add(a, b)` (the nested-match \
         parse_u64 + sum, `ens result is Some && Some(v) => v == parse_be(a) + parse_be(b)`) \
         certifies L3 — the sum is PINNED. certs:\n{certs:#?}"
    );
    for core in ["add_vals", "add_2_3", "add_100_200"] {
        assert_eq!(
            level_of(&certs, core),
            "L3",
            "the arithmetic core `{core}` (Option + `+`) certifies L3 (the sum pinned)."
        );
    }
}

/// (b) the arithmetic core BUILDS + RUNS → Some(5) (2+3) and Some(300) (100+200).
/// Built from a minimal derived program (the Option + `+` core in isolation); the
/// full `calc.th` now builds + runs end-to-end too (#104,
/// `calculator_string_parse_builds_and_runs_end_to_end`). AUTHORITY: the `add_vals`
/// sum contract; `thermite-design.md` §6 (L1 runtime-checked build).
#[test]
fn calculator_arithmetic_core_builds_and_runs() {
    // The arithmetic core in isolation (Option + `+`, NO parse_u64) — the half of
    // the calculator with an L1 runnable form. 2+3 → Some(5), 100+200 → Some(300).
    let core = "fn add_vals(x: u64, y: u64) -> Option<u64>\n  \
                req x <= 9223372036854775807 && y <= 9223372036854775807\n  \
                ens match result { Some(v) => v == x + y, None => false }\n  \
                fx  pure\n{ Some(x + y) }\n\
                fn add_2_3() -> Option<u64> req true ens match result { Some(v) => v == 5, None => false } fx pure { add_vals(2, 3) }\n\
                fn add_100_200() -> Option<u64> req true ens match result { Some(v) => v == 300, None => false } fx pure { add_vals(100, 200) }\n";
    let out = build_run_fixture("calc_core_23", core, "add_2_3");
    assert!(
        out.contains("Some(5)"),
        "the calculator core 2+3 must RUN → Some(5):\nstdout:{out}"
    );
    let out = build_run_fixture("calc_core_100200", core, "add_100_200");
    assert!(
        out.contains("Some(300)"),
        "the calculator core 100+200 must RUN → Some(300):\nstdout:{out}"
    );
}

/// THE GAP NOW CLOSED (crosslink #104) — `forge build calc.th` (the FULL file,
/// including the STRING-PARSE front-end `add`) now COMPILES + RUNS end-to-end. The
/// C7 contract spec fns (`all_digits` / `parse_be` / the free `parse_u64`) now have
/// an L1 (runtime/build) EXEC twin (`thermite-lower::emit_string_runtime_l1`'s C7
/// block, gated on `program_uses_parse`), so the always-active `thermite_check!`s
/// `add`'s `req`/`ens` lower to resolve. The calculator composes end-to-end: the
/// arithmetic core entries build alongside `add`'s now-runnable contracts and RUN →
/// `add_2_3` prints `Some(5)` (2+3), `add_100_200` prints `Some(300)` (100+200).
///
/// (Was `calculator_string_parse_build_is_blocked_by_missing_l1_parse_u64`, which
/// PINNED the gap as an expected build failure; #104 emitted the missing L1 exec
/// twins, flipping it to assert the build SUCCEEDS — the forcing function fired.)
///
/// AUTHORITY: `.design/basis/07-strings.md` REQ-9 + `09-option-result.md` (the C7
/// parse spec fns) + the L1-EXEC-TWIN note; `thermite-design.md` §6 (L1 build —
/// every fn lowers to its always-active runtime check). The sum bytes (5 / 300) are
/// the arithmetic design constant (R-CHAR-3): 2+3==5, 100+200==300.
#[test]
fn calculator_string_parse_builds_and_runs_end_to_end() {
    // `forge build` lowers EVERY fn in calc.th to its runtime `thermite_check!`;
    // `add`'s `req`/`ens` name `all_digits`/`parse_be` and its body calls the free
    // `parse_u64`, all of which now have an L1 exec twin (#104) — so the FULL file
    // compiles and the runnable entries build + run from the same lowering.
    let (ok, stdout, stderr) = build_entry(&calculator_th(), "add_2_3");
    assert!(
        ok,
        "crosslink #104: `forge build calc.th` (the FULL file, with the `add` parse front-end's \
         contracts naming the C7 spec fns) must now COMPILE — the L1 exec twins of \
         `all_digits`/`parse_be`/`parse_u64` are emitted.\nstdout:{stdout}\nstderr:{stderr}"
    );
    let run = Command::new(artifact_of(&stdout))
        .output()
        .expect("run add_2_3");
    let s = String::from_utf8_lossy(&run.stdout);
    assert!(
        run.status.success() && s.contains("Some(5)"),
        "the calculator 2+3 must RUN → Some(5) (the full file, parse front-end built in):\nstdout:{s}"
    );

    let (ok, stdout, stderr) = build_entry(&calculator_th(), "add_100_200");
    assert!(
        ok,
        "add_100_200 must COMPILE:\nstdout:{stdout}\nstderr:{stderr}"
    );
    let run = Command::new(artifact_of(&stdout))
        .output()
        .expect("run add_100_200");
    let s = String::from_utf8_lossy(&run.stdout);
    assert!(
        run.status.success() && s.contains("Some(300)"),
        "the calculator 100+200 must RUN → Some(300):\nstdout:{s}"
    );
}

// ============================================================================
// PROGRAM 3 — the PARSER. forge check L3 (has_sep) + verus L3 (fields split);
// build+run the split core; the count-bound entry build is the same gap.
// ============================================================================

/// (a.1) `has_sep(s, sep) ens result == contains_sub(s, sep)` certifies L3 via the
/// FULL §7-mutation-scored `forge check` ladder (the C5 substring predicate is real
/// teeth). AUTHORITY: `.design/basis/07-strings.md` REQ-13 (GROUNDED `14 verified,
/// 0 errors`; a broken predicate FAILS); `thermite-design.md` §6.
#[test]
fn parser_contains_predicate_certifies_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — parser predicate L3 not exercised.");
        return;
    }
    let certs = check_json(&parser_th());
    assert_eq!(
        level_of(&certs, "has_sep"),
        "L3",
        "DESIGN 07-strings.md REQ-13: `has_sep(s, sep) ens result == contains_sub(s, sep)` \
         certifies L3 through the full §7 ladder (the substring predicate is mutation-scored, \
         real teeth). certs:\n{certs:#?}"
    );
}

/// (a.2) the `fields` split count-bound certifies L3 under REAL VERUS on the
/// lowering. The thin `{ s.split(sep) }` caller is not §7-mutation-scoreable by
/// `forge check` (no scoreable body mutant — the documented split-caller precedent,
/// `string_search_conformance.rs`), so its L3 is established by verus directly.
/// AUTHORITY: `.design/basis/07-strings.md` REQ-15 (the count-bound + sep-free,
/// GROUNDED `7 verified, 0 errors`).
#[test]
fn parser_split_count_bound_verifies_under_real_verus() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — split count-bound not exercised.");
        return;
    }
    // The EXACT `fields` contract from `parse_lines.th`, lowered + run under verus.
    let (ok, output) = verus_on_lowered(
        "fields",
        "fn fields(s: String, sep: u64) -> Vec<String>\n  req true\n  \
         ens result.len() == 1 + count_sep(s, sep)\n  fx alloc\n{ s.split(sep) }\n",
    );
    assert!(
        ok && output.contains("0 errors"),
        "DESIGN 07-strings.md REQ-15: the `fields` split count-bound lowering (the Vec<String> \
         push-loop + count partial + sep-free invariant + lemma_count_push) must VERIFY under \
         real verus `0 errors` (GROUNDED `7 verified, 0 errors`). verus reports:\n{output}"
    );
}

/// (b) the split core BUILDS + RUNS → 3 pieces for "a,b,c" split on ',' (byte 44).
/// Built from a minimal split-only program; the full `parse_lines.th` now builds +
/// runs end-to-end too (#104, `parser_builds_and_runs_end_to_end`).
/// AUTHORITY: `.design/basis/07-strings.md` REQ-15; the byte values 97/98/99 are the
/// ASCII design constant (R-CHAR-3): 'a'=97,'b'=98,'c'=99.
#[test]
fn parser_split_core_builds_and_runs_three_pieces() {
    let split_only = "fn split_abc() -> Vec<String>\n  req true\n  ens result.len() >= 1\n  \
                      fx alloc\n{ let s: String = \"a,b,c\"; s.split(44) }\n";
    let out = build_run_fixture("split_abc", split_only, "split_abc");
    // 3 pieces: "a"=[97], "b"=[98], "c"=[99]. The Vec<String> Debug renders each
    // TString's bytes; all three piece-bytes are present (3 pieces from 2 commas).
    assert!(
        out.contains("[97]") && out.contains("[98]") && out.contains("[99]"),
        "\"a,b,c\" split on ',' (44) must RUN → 3 pieces [97],[98],[99] (== \"a\",\"b\",\"c\"):\nstdout:{out}"
    );
    // Count the piece elements: each piece renders `TString { data: [<byte>] }`
    // INSIDE the outer `TVecTString { data: [ ... ] }`. The outer wrapper name
    // `TVecTString` itself contains the substring `TString`, so we count on the
    // element pattern `data: [9` (every piece byte 97/98/99 starts with '9'),
    // which the outer wrapper's `data: [TString...` does NOT match.
    let pieces = out.matches("data: [9").count();
    assert_eq!(
        pieces, 3,
        "the parser must produce exactly 3 pieces from \"a,b,c\":\nstdout:{out}"
    );
}

/// THE GAP NOW CLOSED (parser side, same class — crosslink #104) — `forge build
/// parse_lines.th` (the FULL file, including the `fields` count-bound + `has_sep`
/// substring contracts) now COMPILES + RUNS. The C5 contract spec fns (`count_sep`
/// / `contains_sub` / `sep_free` / `occurs_at`) now have an L1 exec twin
/// (`thermite-lower::emit_string_runtime_l1`'s C5 block, gated on
/// `program_uses_string_search`), so the always-active `thermite_check!`s of
/// `fields`/`has_sep` resolve. The runnable `split_abc` builds alongside them + RUNS
/// → 3 pieces ([97],[98],[99] == "a","b","c") for "a,b,c" split on ',' (byte 44).
///
/// (Was `parser_build_is_blocked_by_missing_l1_count_sep`, which PINNED the gap as
/// an expected build failure; #104 emitted the missing L1 exec twins, flipping it.)
///
/// AUTHORITY: `.design/basis/07-strings.md` REQ-13/REQ-15 (the C5 spec fns) + the
/// L1-EXEC-TWIN note; `thermite-design.md` §6. The byte values 97/98/99 are the
/// ASCII design constant (R-CHAR-3): 'a'=97,'b'=98,'c'=99.
#[test]
fn parser_builds_and_runs_end_to_end() {
    // The FULL parse_lines.th — `fields`'s `ens result.len() == 1 + count_sep(s, sep)`
    // and `has_sep`'s `ens result == contains_sub(s, sep)` now lower to runnable L1
    // checks (the C5 exec twins, #104), so the file compiles and `split_abc` runs.
    let (ok, stdout, stderr) = build_entry(&parser_th(), "split_abc");
    assert!(
        ok,
        "crosslink #104: `forge build parse_lines.th` (the FULL file, with the C5 count-bound + \
         substring contracts) must now COMPILE — the L1 exec twins of \
         `count_sep`/`contains_sub`/`sep_free`/`occurs_at` are emitted.\nstdout:{stdout}\nstderr:{stderr}"
    );
    let run = Command::new(artifact_of(&stdout))
        .output()
        .expect("run split_abc");
    let out = String::from_utf8_lossy(&run.stdout);
    assert!(
        run.status.success()
            && out.contains("[97]")
            && out.contains("[98]")
            && out.contains("[99]"),
        "\"a,b,c\" split on ',' (44) must RUN → 3 pieces [97],[98],[99] (the full file built):\nstdout:{out}"
    );
    // Exactly 3 pieces from 2 commas (the per-element `data: [9` pattern, see
    // `parser_split_core_builds_and_runs_three_pieces`).
    assert_eq!(
        out.matches("data: [9").count(),
        3,
        "the parser must produce exactly 3 pieces from \"a,b,c\":\nstdout:{out}"
    );
}
