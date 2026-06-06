//! Conformance test for `thermite-lower`'s bounded-`String` lowering (Basis
//! Stage 7, `.design/basis/07-strings.md` REQ-1/REQ-2/REQ-3/REQ-4/REQ-5; issue
//! #79) against the EXTERNAL truths: the real `verus` binary (the emitted L3
//! output must VERIFY — `0 errors`; the reject case must FAIL — non-vacuity,
//! R-DEFER-9) and the hand-derived cert oracle (`conformance/string/cases.json` +
//! `conformance/string_demo.th` — R-CHAR-3, NEVER edited / NEVER read from
//! toolchain output). The golden `tests/golden/lower/string_demo.verus.rs` is the
//! verified REFERENCE (the verify-not-byte-match practice `collections_conformance.rs`
//! uses).
//!
//! The oracle (`cases.json`): `greeting_len` → L3, fx pure (`s.len()`);
//! `first_byte` → L3, fx pure (the no-OOB `byte_at`, `req s.len() > 0`); `join` →
//! L3, fx alloc (the constructing `concat`); `literal_len` → L3, fx alloc (the
//! `Expr::StrLit` `"hello"` materialized to bytes, `len() == 5`).
//! `oob_byte_at_no_req` (a `byte_at` with no `req s.len() > 0`) → L0 (byte_at's
//! precondition unproven → not laundered to L3).
//!
//! `unwrap`/`expect`/`panic!` are fine here — `tests/` is not anti-pattern-gated.

use std::path::{Path, PathBuf};
use std::process::Command;

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("conformance")
}

fn parse_corpus(name: &str) -> thermite_syntax::ast::Program {
    let src = std::fs::read_to_string(corpus_dir().join(format!("{name}.th")))
        .unwrap_or_else(|e| panic!("cannot read corpus {name}.th: {e}"));
    parse_src(&src, name)
}

fn parse_src(src: &str, label: &str) -> thermite_syntax::ast::Program {
    let parsed = thermite_syntax::parse(src);
    assert!(
        parsed.errors.is_empty(),
        "{label} must parse clean: {:?}",
        parsed.errors
    );
    parsed.program
}

// ---- verus driver (shared shape with collections_conformance.rs) -----------

fn verus_bin() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("VERUS_BIN") {
        let pb = PathBuf::from(p);
        if pb.exists() {
            return Some(pb);
        }
    }
    if let Ok(out) = Command::new("which").arg("verus").output() {
        if out.status.success() {
            let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !p.is_empty() {
                return Some(PathBuf::from(p));
            }
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let cand = PathBuf::from(home).join(".local/bin/verus");
        if cand.exists() {
            return Some(cand);
        }
    }
    None
}

/// Run `verus --no-cheating <file>`; `None` if verus is unavailable (caller SKIPs
/// LOUDLY). `--no-cheating` so a sneaked `assume`/`external_body` would be a hard
/// error (R-DEFER-9 — we ground the no-OOB/capacity/length guarantees).
fn run_verus(file: &Path) -> Option<(bool, String)> {
    let bin = verus_bin()?;
    let out = Command::new(bin)
        .arg("--no-cheating")
        .arg(file)
        .current_dir(std::env::temp_dir())
        .output()
        .ok()?;
    let mut combined = String::from_utf8_lossy(&out.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&out.stderr));
    Some((out.status.success(), combined))
}

/// Write `emitted` to a temp file with a VALID crate name (the verus
/// `.`-in-crate-name gotcha), run `verus`, return `(exit_success, output)` or
/// `None` if verus is unavailable.
fn verify(crate_name: &str, emitted: &str) -> Option<(bool, String)> {
    let tmp = std::env::temp_dir().join(format!("{crate_name}.rs"));
    std::fs::write(&tmp, emitted).unwrap_or_else(|e| panic!("write temp {crate_name}: {e}"));
    run_verus(&tmp)
}

fn lower_l3(program: &thermite_syntax::ast::Program) -> String {
    thermite_lower::lower(program).unwrap_or_else(|e| panic!("L3 lowering failed: {e}"))
}

// ---- AC-2/AC-3: String + literal lower to the TString wrapper, VERIFY (L3) --
//
// REQ-1/REQ-2/REQ-4: `string_demo.th` lowers to the `TString` newtype over
// `vstd::vec::Vec<u8>` with `well_formed`/`spec_len`/`len`/`spec_byte_at`/`byte_at`/
// `concat`/`slice`; the spec `s.len()`/`s.byte_at(i)` lower to `s.spec_len()`/
// `s.spec_byte_at(i as int)`; a string literal materializes by byte-push. Real
// verus VERIFIES the emitted output (`11 verified, 0 errors`).

#[test]
fn string_demo_lowers_wrapper_and_verifies_l3() {
    let program = parse_corpus("string_demo");
    let emitted = lower_l3(&program);

    // REQ-4 the vstd-Vec<u8> wrapper struct + the capacity invariant.
    assert!(
        emitted.contains("pub struct TString { pub data: Vec<u8> }"),
        "String → TString newtype over vstd Vec<u8> (REQ-4):\n{emitted}"
    );
    assert!(
        emitted
            .contains("pub open spec fn well_formed(&self) -> bool { self.data.len() <= 1000000 }"),
        "the well_formed capacity invariant (len() <= CAP, REQ-4):\n{emitted}"
    );
    // REQ-4 the no-OOB exec `byte_at` (req i < len, ens result == s@[i]).
    assert!(
        emitted.contains("    pub fn byte_at(&self, i: usize) -> (result: u64)")
            && emitted.contains("        requires i < self.data.len(),")
            && emitted.contains("        ensures result == self.data@[i as int],"),
        "the no-OOB byte_at accessor (req i < len, REQ-4):\n{emitted}"
    );
    // REQ-4 the bounded constructing `concat` (req len_a + len_b <= CAP, ens
    // result.len() == len_a + len_b).
    assert!(
        emitted.contains("    pub fn concat(&self, b: TString) -> (result: TString)")
            && emitted
                .contains("                result.data.len() == self.data.len() + b.data.len(),"),
        "the bounded concat with the length identity (REQ-4):\n{emitted}"
    );
    // REQ-4 the bounded `slice` (req self.well_formed() && lo <= hi && hi <= len).
    assert!(
        emitted.contains("    pub fn slice(&self, lo: usize, hi: usize) -> (result: TString)")
            && emitted
                .contains("        ensures result.well_formed(), result.data.len() == hi - lo,"),
        "the bounded slice (REQ-4):\n{emitted}"
    );
    // REQ-4 the spec-position s.len()/s.byte_at(0) → spec_len()/spec_byte_at rewrites.
    assert!(
        emitted.contains("result == s.spec_len(),"),
        "spec-position s.len() → s.spec_len() (REQ-4):\n{emitted}"
    );
    assert!(
        emitted.contains("result == s.spec_byte_at(0),"),
        "spec-position s.byte_at(0) → s.spec_byte_at(0) (REQ-4):\n{emitted}"
    );
    // REQ-1 the string literal "hello" materializes by byte-push (104,101,108,108,111).
    assert!(
        emitted.contains("data.push(104u8);")
            && emitted.contains("data.push(101u8);")
            && emitted.contains("data.push(111u8);")
            && emitted.contains("TString { data }"),
        "the string literal \"hello\" materializes by byte-push (REQ-1):\n{emitted}"
    );
    // No weakening: the corpus contracts present (R-DEFER-9).
    assert!(
        emitted.contains("result == s.spec_byte_at(0),") && emitted.contains("s.spec_len() > 0"),
        "first_byte's req/ens present, no weakening (R-DEFER-9):\n{emitted}"
    );
    assert_no_cheats(&emitted, "string_demo");

    // The EXTERNAL truth: real verus verifies the emitted output (R-CODE-4 — exit
    // status checked, never swallowed).
    match verify("string_demo_strings", &emitted) {
        Some((ok, output)) => {
            assert!(
                ok && output.contains("0 errors"),
                "verus on emitted string_demo did NOT verify (R-CODE-4). exit_success={ok}\n\
                 --- verus output ---\n{output}\n--- emitted ---\n{emitted}"
            );
            assert!(
                output.contains("verified, 0 errors"),
                "verus output for string_demo missing `verified, 0 errors`:\n{output}"
            );
        }
        None => eprintln!(
            "SKIP: verus not available — L3 verification of emitted string_demo not run \
             (set VERUS_BIN or install verus on PATH); structural asserts still run."
        ),
    }
}

// ---- AC-2/AC-3 cert oracle: the L3/fx judgements from cases.json -----------
//
// The oracle (`conformance/string/cases.json`, R-CHAR-3 — never edited) pins:
// greeting_len/first_byte L3 fx pure; join/literal_len L3 fx alloc;
// oob_byte_at_no_req L0. We assert the oracle fields directly from the raw JSON
// AND that the parsed `fx` rows match.

#[test]
fn string_demo_matches_cert_oracle() {
    let cases = std::fs::read_to_string(corpus_dir().join("string").join("cases.json"))
        .expect("read conformance/string/cases.json");
    for needle in [
        "\"name\": \"greeting_len\"",
        "\"level\": \"L3\"",
        "\"effects\": [\"pure\"]",
        "\"name\": \"first_byte\"",
        "\"name\": \"join\"",
        "\"effects\": [\"alloc\"]",
        "\"name\": \"literal_len\"",
        "\"name\": \"oob_byte_at_no_req\"",
        "\"expect_level\": \"L0\"",
    ] {
        assert!(
            cases.contains(needle),
            "string cases.json oracle missing `{needle}`:\n{cases}"
        );
    }

    // The parsed `fx` rows match the oracle.
    use thermite_syntax::ast::{Effect, EffectRow, Item};
    let program = parse_corpus("string_demo");
    for item in &program.items {
        if let Item::Fn(f) = item {
            match f.name.as_str() {
                "greeting_len" | "first_byte" => assert!(
                    matches!(f.contract.fx, EffectRow::Pure),
                    "{} must be fx pure (oracle)",
                    f.name
                ),
                "join" | "literal_len" => assert!(
                    matches!(&f.contract.fx, EffectRow::Set(es) if es == &vec![Effect::Alloc]),
                    "{} must be fx alloc (oracle); got {:?}",
                    f.name,
                    f.contract.fx
                ),
                other => panic!("unexpected fn {other} in string_demo"),
            }
        }
    }

    // The `fx alloc` of join/literal_len passes effect-subsumption: `concat`/the
    // literal-materialization are intrinsics (no declared callee row to subsume) —
    // the caller's declared `alloc` row is accepted (the Stage-1 Alloc heap rule).
    assert!(
        thermite_lower::check_effects(&program).is_ok(),
        "string_demo (greeting_len/first_byte pure, join/literal_len alloc) must pass \
         effect-subsumption"
    );
}

// ---- AC-1: a string literal parses as an expression (Expr::StrLit) ---------
//
// REQ-1: `let s = "hello"` yields `Expr::StrLit("hello")`; `parse_primary`
// accepts `TokKind::Str` as a primary expr. The `literal_len` corpus fn's body
// is `"hello".len()` — a MethodCall whose receiver is a StrLit.

#[test]
fn string_literal_parses_as_expression() {
    use thermite_syntax::ast::{Expr, Item, Stmt};
    let program = parse_corpus("string_demo");
    let literal_len = program
        .items
        .iter()
        .find_map(|i| match i {
            Item::Fn(f) if f.name == "literal_len" => Some(f),
            _ => None,
        })
        .expect("literal_len fn present");
    let body = literal_len.body.as_ref().expect("literal_len has a body");
    // The body tail (or last stmt) is `"hello".len()`: a MethodCall over a StrLit.
    let tail = body
        .tail
        .as_deref()
        .or_else(|| {
            body.stmts.iter().rev().find_map(|s| match s {
                Stmt::Expr(e) => Some(e),
                _ => None,
            })
        })
        .expect("literal_len has a value expression");
    match tail {
        Expr::MethodCall { receiver, name, .. } => {
            assert_eq!(name, "len", "the op is `.len()`");
            assert!(
                matches!(receiver.as_ref(), Expr::StrLit(s) if s == "hello"),
                "the receiver is the string literal Expr::StrLit(\"hello\") (REQ-1); got {:?}",
                receiver
            );
        }
        other => panic!("expected `\"hello\".len()` MethodCall, got {other:?}"),
    }
}

// ---- AC-4 reject: oob_byte_at_no_req → L0 (the no-OOB guarantee is real) ----
//
// REQ-3/REQ-4 non-vacuity (R-DEFER-9): a `byte_at` WITHOUT `req s.len() > 0`
// leaves byte_at's index precondition undischarged → verus FAILS → NOT laundered
// to L3. The reject program is the oracle's `program` field (R-CHAR-3).

#[test]
fn oob_byte_at_without_req_fails_verus_l0() {
    // The oracle's reject program (cases.json `oob_byte_at_no_req.program`).
    let src =
        "fn oob_byte_at_no_req(s: String) -> u64 req true ens result == s.byte_at(0) fx pure { s.byte_at(0) }";
    let program = parse_src(src, "oob_byte_at_no_req");
    let emitted = lower_l3(&program);
    // It still LOWERS (a well-formed program); the FAILURE is at verus (L0), not a
    // lowerer error — the no-OOB guarantee is enforced by the proof.
    assert!(
        emitted.contains("fn oob_byte_at_no_req(s: TString)"),
        "the reject program lowers to the wrapper accessor:\n{emitted}"
    );
    assert_no_cheats(&emitted, "oob_byte_at_no_req");

    match verify("string_oob_reject_strings", &emitted) {
        Some((ok, output)) => {
            assert!(
                !ok || !output.contains("0 errors") || output.contains("1 errors"),
                "the unguarded byte_at MUST FAIL verus (L0, not laundered to L3); \
                 instead verus accepted it:\n{output}\n--- emitted ---\n{emitted}"
            );
            assert!(
                output.contains("error") && !output.contains("0 errors\n"),
                "expected a verus verification error for the unguarded byte_at:\n{output}"
            );
        }
        None => eprintln!(
            "SKIP: verus not available — L0 reject of oob_byte_at_no_req not run \
             (set VERUS_BIN or install verus on PATH)."
        ),
    }
}

// ---- AC-5 no regression: the golden reference verifies, slice corpus unchanged

#[test]
fn string_demo_golden_reference_verifies() {
    // The hand-authored golden (`tests/golden/lower/string_demo.verus.rs`,
    // R-CHAR-3) is the verified reference. Confirm it itself passes verus (the
    // external truth the lowering is pinned against), reading it through a
    // valid-crate-name temp copy (the `.verus.rs` filename gotcha).
    let golden = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("tests/golden/lower/string_demo.verus.rs");
    let src = std::fs::read_to_string(&golden).expect("read string_demo.verus.rs golden");
    match verify("string_demo_golden_strings", &src) {
        Some((ok, output)) => assert!(
            ok && output.contains("verified, 0 errors"),
            "the string_demo golden reference must pass verus (`verified, 0 errors`):\n{output}"
        ),
        None => eprintln!("SKIP: verus not available — golden reference not verified."),
    }
}

#[test]
fn non_string_corpus_unchanged_no_regression() {
    // The String additions are purely additive (a new `Type::String` node, a new
    // `Expr::StrLit` node, the wrapper lowering path); a non-String program must
    // still lower to verus that verifies and must NOT emit the TString wrapper.
    for name in ["sum", "binary_search", "vec_demo"] {
        let program = parse_corpus(name);
        let emitted = lower_l3(&program);
        assert!(
            !emitted.contains("pub struct TString"),
            "{name} (no String) must not emit the TString wrapper (byte-stable, no regression):\n{emitted}"
        );
        match verify(&format!("{name}_regression_strings"), &emitted) {
            Some((ok, output)) => assert!(
                ok && output.contains("verified, 0 errors"),
                "{name} must still verify L3 (no regression):\n{output}"
            ),
            None => eprintln!("SKIP: verus not available — {name} regression not verified."),
        }
    }
}

// ---- no proof cheats (R-DEFER-9) -------------------------------------------

fn assert_no_cheats(emitted: &str, name: &str) {
    for forbidden in [
        "assume(false)",
        "assume(",
        "#[verifier::external]",
        "#[verifier::external_body]",
        "admit(",
        "#[slag]",
        "ensures true",
        "ensures\n        true,",
    ] {
        assert!(
            !emitted.contains(forbidden),
            "{name} emission contains forbidden cheat token `{forbidden}` (R-DEFER-9):\n{emitted}"
        );
    }
}
