//! Conformance test for `thermite-lower`'s bounded-collection lowering (Basis
//! Stage 4, `.design/basis/04-collections.md` REQ-3/REQ-5/REQ-7; issue #73)
//! against the EXTERNAL truths: the real `verus` binary (the emitted L3 output
//! must VERIFY — `0 errors`; the reject case must FAIL — non-vacuity, R-DEFER-9)
//! and the hand-derived cert oracle (`conformance/collections/cases.json` +
//! `conformance/vec_demo.th` — R-CHAR-3, NEVER edited / NEVER read from toolchain
//! output). The golden `tests/golden/lower/vec_demo.verus.rs` is the verified
//! REFERENCE (the verify-not-byte-match practice the existing
//! `adt_lower_conformance.rs` uses).
//!
//! The oracle (`cases.json`): `checked_get` → L3, fx pure (the no-OOB accessor:
//! `req i < v.len()` discharges get's bound); `push_one` → L3, fx alloc (the
//! capacity-preserving push). `oob_get_no_req` (a get with no `req i < len`) → L0
//! (get's precondition unproven → not laundered to L3).
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

// ---- verus driver (shared shape with adt_lower_conformance.rs) -------------

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
/// error (R-DEFER-9 — we ground the no-OOB/capacity guarantees, never launder).
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

// ---- AC-1: bounded Vec accessor + push lower to the wrapper, VERIFY (L3) ----
//
// REQ-5: `vec_demo.th` lowers to the `TVecU64` newtype over `vstd::vec::Vec<u64>`
// with `well_formed`/`len`/`spec_get`/`get`/`push`; the spec `v.get(i)` lowers to
// `v.spec_get(i as int)`; the `&mut push` postcondition uses `final(self)`. Real
// verus VERIFIES (`4 verified, 0 errors` — 2 fns + the wrapper's get/push).

#[test]
fn vec_demo_lowers_wrapper_and_verifies_l3() {
    let program = parse_corpus("vec_demo");
    let emitted = lower_l3(&program);

    // REQ-5 the vstd-Vec wrapper struct + the capacity invariant.
    assert!(
        emitted.contains("pub struct TVecU64 { pub data: Vec<u64> }"),
        "Vec<u64> → TVecU64 newtype over vstd Vec<u64> (REQ-5):\n{emitted}"
    );
    assert!(
        emitted
            .contains("pub open spec fn well_formed(&self) -> bool { self.data.len() <= 1000000 }"),
        "the well_formed capacity invariant (len() <= CAP, REQ-5):\n{emitted}"
    );
    // REQ-5 the no-OOB exec `get` (req i < len, ens result == v@[i]).
    assert!(
        emitted.contains("    pub fn get(&self, i: usize) -> (result: u64)")
            && emitted.contains("        requires i < self.data.len(),")
            && emitted.contains("        ensures result == self.data@[i as int],"),
        "the no-OOB get accessor (req i < len, REQ-5):\n{emitted}"
    );
    // REQ-5 the capacity-preserving exec `push` with the `final(self)` &mut
    // postcondition (the grounding finding — verus 0.2026.05.24 needs final(self)).
    assert!(
        emitted.contains("    pub fn push(&mut self, x: u64)")
            && emitted.contains(
                "        requires old(self).well_formed(), old(self).data.len() < 1000000,"
            )
            && emitted.contains("            final(self).well_formed(),")
            && emitted.contains("            final(self).data.len() == old(self).data.len() + 1,"),
        "the capacity-preserving push with final(self) (REQ-5 grounding finding):\n{emitted}"
    );
    // REQ-5 the spec-position v.get(i) → v.spec_get(i as int) rewrite.
    assert!(
        emitted.contains("result == v.spec_get(i as int),"),
        "spec-position v.get(i) → v.spec_get(i as int) (REQ-5):\n{emitted}"
    );
    // No weakening: the corpus contracts present (R-DEFER-9).
    assert!(
        emitted.contains("requires i < v.len(),")
            && emitted.contains("requires v.len() < 1000000,"),
        "corpus req present, no weakening (R-DEFER-9):\n{emitted}"
    );
    assert!(
        emitted.contains("result.len() == v.len() + 1,"),
        "push_one's capacity-preserving ens present (no weakening):\n{emitted}"
    );
    assert_no_cheats(&emitted, "vec_demo");

    // The EXTERNAL truth: real verus verifies the emitted output (R-CODE-4 — exit
    // status checked, never swallowed).
    match verify("vec_demo_collections", &emitted) {
        Some((ok, output)) => {
            assert!(
                ok && output.contains("0 errors"),
                "verus on emitted vec_demo did NOT verify (R-CODE-4). exit_success={ok}\n\
                 --- verus output ---\n{output}\n--- emitted ---\n{emitted}"
            );
            assert!(
                output.contains("verified, 0 errors"),
                "verus output for vec_demo missing `verified, 0 errors`:\n{output}"
            );
        }
        None => eprintln!(
            "SKIP: verus not available — L3 verification of emitted vec_demo not run \
             (set VERUS_BIN or install verus on PATH); structural asserts still run."
        ),
    }
}

// ---- AC-1 cert oracle: checked_get → L3/pure, push_one → L3/alloc -----------
//
// The oracle (`conformance/collections/cases.json`, R-CHAR-3 — never edited)
// pins: checked_get L3 fx pure; push_one L3 fx alloc. We assert the oracle fields
// directly from the raw JSON AND that the lowering enables that judgement: the
// emitted verus verifies (L3 above) and the parsed `fx` rows match the oracle.

#[test]
fn vec_demo_matches_cert_oracle() {
    let cases = std::fs::read_to_string(corpus_dir().join("collections").join("cases.json"))
        .expect("read conformance/collections/cases.json");
    for needle in [
        "\"name\": \"checked_get\"",
        "\"level\": \"L3\"",
        "\"effects\": [\"pure\"]",
        "\"name\": \"push_one\"",
        "\"effects\": [\"alloc\"]",
        "\"name\": \"oob_get_no_req\"",
        "\"expect_level\": \"L0\"",
    ] {
        assert!(
            cases.contains(needle),
            "collections cases.json oracle missing `{needle}`:\n{cases}"
        );
    }

    // The parsed `fx` rows match the oracle (checked_get pure, push_one alloc).
    use thermite_syntax::ast::{Effect, EffectRow, Item};
    let program = parse_corpus("vec_demo");
    for item in &program.items {
        if let Item::Fn(f) = item {
            match f.name.as_str() {
                "checked_get" => assert!(
                    matches!(f.contract.fx, EffectRow::Pure),
                    "checked_get must be fx pure (oracle)"
                ),
                "push_one" => assert!(
                    matches!(&f.contract.fx, EffectRow::Set(es) if es == &vec![Effect::Alloc]),
                    "push_one must be fx alloc (oracle); got {:?}",
                    f.contract.fx
                ),
                other => panic!("unexpected fn {other} in vec_demo"),
            }
        }
    }

    // The `fx alloc` of push_one passes effect-subsumption: `push` is an intrinsic
    // (an unresolved method-call callee), so there is no callee row to subsume —
    // the caller's declared `alloc` row is accepted (the Stage-1 Alloc heap rule).
    assert!(
        thermite_lower::check_effects(&program).is_ok(),
        "vec_demo (checked_get fx pure, push_one fx alloc) must pass effect-subsumption"
    );
}

// ---- AC-1 reject: oob_get_no_req → L0 (the no-OOB guarantee is real) --------
//
// REQ-5 non-vacuity (R-DEFER-9): a `get` WITHOUT `req i < v.len()` leaves get's
// index precondition undischarged → verus FAILS → NOT laundered to L3. The
// reject program is the oracle's `program` field (R-CHAR-3 — hand-derived).

#[test]
fn oob_get_without_req_fails_verus_l0() {
    // The oracle's reject program verbatim (cases.json `oob_get_no_req.program`).
    let src =
        "fn bad(v: Vec<u64>, i: usize) -> u64 req true ens result == v.get(i) fx pure { v.get(i) }";
    let program = parse_src(src, "oob_get_no_req");
    let emitted = lower_l3(&program);
    // It still LOWERS (a well-formed program); the FAILURE is at verus (L0), not a
    // lowerer error — the no-OOB guarantee is enforced by the proof, not the
    // emitter.
    assert!(
        emitted.contains("fn bad(v: TVecU64, i: usize)"),
        "the reject program lowers to the wrapper accessor:\n{emitted}"
    );
    assert_no_cheats(&emitted, "oob_get_no_req");

    match verify("vec_oob_reject_collections", &emitted) {
        Some((ok, output)) => {
            assert!(
                !ok || !output.contains("0 errors") || output.contains("1 errors"),
                "the unguarded get MUST FAIL verus (L0, not laundered to L3); \
                 instead verus accepted it:\n{output}\n--- emitted ---\n{emitted}"
            );
            // Specifically: a verification error (the get precondition unproven).
            assert!(
                output.contains("error") && !output.contains("0 errors\n"),
                "expected a verus verification error for the unguarded get:\n{output}"
            );
        }
        None => eprintln!(
            "SKIP: verus not available — L0 reject of oob_get_no_req not run \
             (set VERUS_BIN or install verus on PATH)."
        ),
    }
}

// ---- AC-4 no regression: the golden reference verifies, slice corpus unchanged

#[test]
fn vec_demo_golden_reference_verifies() {
    // The hand-authored golden (`tests/golden/lower/vec_demo.verus.rs`, R-CHAR-3)
    // is the verified reference. Confirm it itself passes verus (the byte-stable
    // external truth the lowering is pinned against), reading it through a
    // valid-crate-name temp copy (the `.verus.rs` filename gotcha).
    let golden = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("tests/golden/lower/vec_demo.verus.rs");
    let src = std::fs::read_to_string(&golden).expect("read vec_demo.verus.rs golden");
    match verify("vec_demo_golden_collections", &src) {
        Some((ok, output)) => assert!(
            ok && output.contains("verified, 0 errors"),
            "the vec_demo golden reference must pass verus (`verified, 0 errors`):\n{output}"
        ),
        None => eprintln!("SKIP: verus not available — golden reference not verified."),
    }
}

#[test]
fn slice_corpus_unchanged_no_regression() {
    // The Vec additions are purely additive (a new `Type::Vec` node + the wrapper
    // lowering path); the read-only `&[T]` algorithms must still lower to verus
    // that verifies. Verification by verus, not a byte-match (the verify-not-byte-
    // match practice).
    for name in ["sum", "binary_search"] {
        let program = parse_corpus(name);
        let emitted = lower_l3(&program);
        // The Vec wrapper must NOT leak into a non-Vec program (byte-stable).
        assert!(
            !emitted.contains("TVecU64") && !emitted.contains("pub data: Vec<"),
            "{name} (no Vec) must not emit the Vec wrapper (byte-stable, no regression):\n{emitted}"
        );
        match verify(&format!("{name}_regression_collections"), &emitted) {
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
