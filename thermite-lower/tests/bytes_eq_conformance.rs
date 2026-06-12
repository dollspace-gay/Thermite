//! Conformance for Cluster **C8** (crosslink **#278**): the `bytes_eq`
//! byte-range-equality content-pin layer — `bytes_eq(a, b, ai, bi, n)` as a
//! REGISTERED built-in spec predicate (07-strings.md REQ-17), the canonical
//! `Seq<u8>` low-peel def + the FOUR prove-once bridge lemmas (REQ-18), the
//! `program_uses_bytes_eq` conditional emission + the contract-keyed body-start
//! `proof { lemma_bytes_eq_bridge(); }` citation (REQ-19), and the L1 exec twin
//! (REQ-20). The SECOND #276 prerequisite (after #277's slice/concat byte-content
//! ens).
//!
//! These run against the EXTERNAL truth the toolchain does not author for itself:
//! the real `verus` binary on the EMITTED lowering of `conformance/bytes_eq_demo.th`
//! (R-CODE-4 — the subprocess status is checked, never swallowed). The constructing
//! slice/concat bodies are thin (`{ a.slice(0, a.len()) }`), so — like the C5
//! `split`/`trim` and the C7 `parse_u64` AC-4 precedent — the §7 mutation floor is
//! grounded by verus-on-the-emitted-lowering (a body mutant that breaks the pin
//! FAILS), not by `forge`'s caller-mutation scorer.
//!
//! AC-13: `slice_id(a) = a.slice(0, a.len())` with `ens bytes_eq(&result, &a, 0, 0,
//! a.len())` (the EXACT #276 counterexample) certifies L3.
//! AC-14: the three `insert_str` conjuncts (unchanged-prefix / inserted-run /
//! shifted-suffix) EACH certify L3 with ONE `lemma_bytes_eq_bridge` citation, ZERO
//! per-conjunct glue.
//! AC-16: the length-preserving head/tail-SWAP mutant FAILS verus (non-vacuity,
//! R-DEFER-9) — the pins are content teeth a length pin cannot fake; and WITHOUT the
//! REQ-19 citation the pins fail (the bridge is load-bearing, not decorative).
//!
//! The verus checks SKIP LOUDLY when verus is absent (the `string_conformance.rs`
//! precedent) — never panic on a missing solver. `tests/` is not anti-pattern-gated,
//! so `unwrap`/`expect`/`panic!` are fine (R-APG-2).
//!
//! R-CHAR-3: expected levels trace to `.design/basis/07-strings.md` REQ-17..20 (the
//! GROUNDED forms: the four lemmas + the `slice_id`/`insert_str` pins `14 verified,
//! 0 errors`; the head/tail-swap mutant `13 verified, 1 errors`) + `thermite-design.md`
//! §6 ladder semantics (L3 == a fully-discharged real-verus proof), NEVER copied from
//! the toolchain's own output.

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
    let parsed = thermite_syntax::parse(&src);
    assert!(
        parsed.errors.is_empty(),
        "{name} must parse clean: {:?}",
        parsed.errors
    );
    parsed.program
}

fn lower_l3(program: &thermite_syntax::ast::Program) -> String {
    thermite_lower::lower(program).unwrap_or_else(|e| panic!("L3 lowering failed: {e}"))
}

// ---- verus driver (shared shape with string_conformance.rs) ----------------

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
/// error (R-DEFER-9 — the bridge lemmas are REAL induction proofs).
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

fn verify(crate_name: &str, emitted: &str) -> Option<(bool, String)> {
    let tmp = std::env::temp_dir().join(format!("{crate_name}.rs"));
    std::fs::write(&tmp, emitted).unwrap_or_else(|e| panic!("write temp {crate_name}: {e}"));
    run_verus(&tmp)
}

// ---- AC-17 structure: bytes_eq joins the emitted module conditionally --------

#[test]
fn bytes_eq_demo_emits_def_and_lemmas_and_citation() {
    let program = parse_corpus("bytes_eq_demo");
    let emitted = lower_l3(&program);

    // REQ-18: the canonical Seq<u8> low-peel def (reserved-named, #130).
    assert!(
        emitted.contains(
            "pub open spec fn __thermite_bytes_eq(a: Seq<u8>, b: Seq<u8>, ai: int, bi: int, n: int) -> bool"
        ),
        "the bytes_eq Seq<u8> def is emitted (reserved-named, REQ-18):\n{emitted}"
    );
    assert!(
        emitted.contains(
            "if n <= 0 { true } else { a[ai] == b[bi] && __thermite_bytes_eq(a, b, ai + 1, bi + 1, n - 1) }"
        ),
        "the LOW-PEEL recursion body (REQ-18):\n{emitted}"
    );
    // REQ-18: the four prove-once bridge lemmas, all present (reserved-named).
    for lemma in [
        "__thermite_lemma_bytes_eq_from_pointwise",
        "__thermite_lemma_bytes_eq_to_pointwise",
        "__thermite_lemma_bytes_eq_from_subrange",
        "__thermite_lemma_bytes_eq_bridge",
    ] {
        assert!(
            emitted.contains(&format!("pub proof fn {lemma}")),
            "the prove-once lemma {lemma} is emitted (REQ-18):\n{emitted}"
        );
    }
    // The LOAD-BEARING explicit trigger on the arithmetic index (REQ-18 recorded finding).
    assert!(
        emitted.contains("#[trigger] a[ai + k] == b[bi + k]"),
        "the load-bearing explicit #[trigger] a[ai + k] (REQ-18):\n{emitted}"
    );
    // REQ-19: the contract-keyed body-start citation (a fn naming bytes_eq).
    assert!(
        emitted.contains("proof { __thermite_lemma_bytes_eq_bridge(); }"),
        "the contract-keyed lemma_bytes_eq_bridge citation (REQ-19):\n{emitted}"
    );
    // REQ-17/REQ-19: the contract args lower the String operands to .data@ byte-views
    // (the `&result`/`&a` ref form stripped to the byte Seq) and the index args `as int`
    // (the #276 slice_id pin).
    assert!(
        emitted.contains(
            "__thermite_bytes_eq(result.data@, a.data@, 0 as int, 0 as int, a.spec_len() as int)"
        ),
        "the slice_id pin lowers a/b to byte-views + the index args (REQ-17/REQ-19):\n{emitted}"
    );
    // #279: a FIELD-ACCESS String operand (`&result.text`/`&b.text` — the editor's
    // `Buf { text: String }`) lowers to the field byte-view (`result.text.data@`/
    // `b.text.data@`), NOT a `&TString` reference (E0308). The whole operand class
    // (bare path / &path / field / &field) is covered.
    assert!(
        emitted.contains(
            "__thermite_bytes_eq(result.text.data@, b.text.data@, 0 as int, 0 as int, b.cursor as int)"
        ),
        "the buf_prefix_pin field-access pin lowers result.text/b.text to byte-views (#279):\n{emitted}"
    );
    assert!(
        !emitted.contains("__thermite_bytes_eq(&result.text"),
        "no field-access operand may lower to a &TString reference (E0308, #279):\n{emitted}"
    );
    // R-DEFER-9: no proof cheats in the generated stack.
    for cheat in [
        "assume(",
        "external_body",
        "admit(",
        "#[verifier::external]",
    ] {
        assert!(
            !emitted.contains(cheat),
            "no proof cheat `{cheat}` in the bytes_eq lowering (R-DEFER-9):\n{emitted}"
        );
    }
}

// ---- AC-17 conditional emission: a non-bytes_eq program is byte-stable -------

#[test]
fn non_bytes_eq_program_does_not_emit_bytes_eq() {
    // `string_demo.th` (greeting_len/first_byte/join/literal_len) names NO bytes_eq.
    let program = parse_corpus("string_demo");
    let emitted = lower_l3(&program);
    assert!(
        !emitted.contains("__thermite_bytes_eq"),
        "a non-bytes_eq program must NOT materialize the bytes_eq def (REQ-19 gate, byte-stable):\n{emitted}"
    );
    assert!(
        !emitted.contains("lemma_bytes_eq_bridge"),
        "a non-bytes_eq program must NOT emit the citation (REQ-19 gate):\n{emitted}"
    );
}

// ---- AC-13 cert oracle: the hand-derived cert subset (R-CHAR-3) --------------
//
// The golden `conformance/bytes_eq_demo.cert.json` is the hand-derived oracle
// (R-CHAR-3 — never read from forge's output): `slice_id` → L3, fx alloc,
// non-vacuous, not-slag. We assert the oracle's deterministic subset directly from
// the raw JSON AND that the parsed `fx` row of `slice_id` matches the oracle's
// `effects: ["alloc"]` (the constructing-slice effect, REQ-4 rule). `forge check`'s
// LIVE comparison against this oracle is exercised end-to-end at the forge layer;
// here the cert artifact's internal consistency with the corpus is pinned.

#[test]
fn bytes_eq_demo_matches_cert_oracle() {
    let cert = std::fs::read_to_string(corpus_dir().join("bytes_eq_demo.cert.json"))
        .expect("read conformance/bytes_eq_demo.cert.json");
    for needle in [
        "\"item\": \"slice_id\"",
        "\"level\": \"L3\"",
        "\"effects\": [\"alloc\"]",
        "\"tautology\": false",
        "\"vacuous_precondition\": false",
        "\"slag\": false",
    ] {
        assert!(
            cert.contains(needle),
            "bytes_eq_demo.cert.json oracle missing `{needle}`:\n{cert}"
        );
    }

    // The parsed `fx alloc` row of slice_id matches the oracle.
    use thermite_syntax::ast::{Effect, EffectRow, Item};
    let program = parse_corpus("bytes_eq_demo");
    let mut saw_slice_id = false;
    for item in &program.items {
        if let Item::Fn(f) = item {
            if f.name == "slice_id" {
                saw_slice_id = true;
                assert!(
                    matches!(&f.contract.fx, EffectRow::Set(es) if es == &vec![Effect::Alloc]),
                    "slice_id must be fx alloc (oracle); got {:?}",
                    f.contract.fx
                );
            }
        }
    }
    assert!(saw_slice_id, "slice_id must be present in the corpus");

    // Effect-subsumption accepts the constructing slice's `alloc` row (REQ-4 rule).
    assert!(
        thermite_lower::check_effects(&program).is_ok(),
        "bytes_eq_demo (slice_id/insert_str fx alloc) must pass effect-subsumption"
    );
}

// ---- AC-13/AC-14: the emitted lowering VERIFIES under real verus (L3) --------

#[test]
fn bytes_eq_demo_verifies_l3_under_real_verus() {
    let program = parse_corpus("bytes_eq_demo");
    let emitted = lower_l3(&program);
    match verify("bytes_eq_demo_c8", &emitted) {
        Some((ok, output)) => {
            assert!(
                ok && output.contains("0 errors"),
                "verus on emitted bytes_eq_demo did NOT verify (R-CODE-4, AC-13/AC-14). \
                 exit_success={ok}\n--- verus output ---\n{output}\n--- emitted ---\n{emitted}"
            );
            assert!(
                output.contains("verified, 0 errors"),
                "verus output for bytes_eq_demo missing `verified, 0 errors`:\n{output}"
            );
        }
        None => eprintln!(
            "SKIP: verus not available — L3 verification of emitted bytes_eq_demo not run \
             (set VERUS_BIN or install verus on PATH); structural asserts still run."
        ),
    }
}

// ---- AC-16: the head/tail-SWAP mutant FAILS verus (non-vacuity) --------------
//
// The length-preserving swap (`tail.concat(ins).concat(head)`) keeps every length
// identity but BREAKS the byte-content pins — the design's `15 verified, 1 errors`
// mutant, here through the real pipeline (the body is mutated in the lowered SOURCE,
// the contract + the prove-once lemmas unchanged). If verus still PASSED the mutant,
// the pins would be vacuous (a length pin a content pin cannot distinguish) — R-DEFER-9.

#[test]
fn bytes_eq_demo_content_mutant_fails_real_verus() {
    let program = parse_corpus("bytes_eq_demo");
    let emitted = lower_l3(&program);
    // Swap the insert_str body's head/tail order in the EMITTED source. The lowered
    // body builds `head.concat(ins-copy).concat(tail)`; the slice for the prefix is
    // `self.slice(0, cursor)` (head) and the suffix `self.slice(cursor, len)` (tail).
    // Mutate by swapping the two slice ranges' upper/lower so head and tail trade
    // places — a content swap that preserves total length.
    let mutated = swap_head_tail_in_insert_str(&emitted);
    assert_ne!(
        mutated, emitted,
        "the head/tail swap mutation must change the lowered source (AC-16 setup)"
    );
    match verify("bytes_eq_demo_c8_mutant", &mutated) {
        Some((ok, output)) => {
            assert!(
                !ok || !output.contains(", 0 errors"),
                "the head/tail-SWAP mutant must FAIL verus (non-vacuity, R-DEFER-9/AC-16): \
                 the content pins are real teeth a length pin cannot fake.\n\
                 --- verus output ---\n{output}\n--- mutated ---\n{mutated}"
            );
        }
        None => eprintln!(
            "SKIP: verus not available — the AC-16 non-vacuity mutant not run \
             (set VERUS_BIN or install verus on PATH)."
        ),
    }
}

/// Swap the head/tail slices in the lowered `insert_str` body — a length-preserving
/// CONTENT swap (the design's AC-16 mutant, here on the emitted source). The body
/// lowers to a chained `concat`: `text.slice(0, cursor ...).concat(ins...).concat(
/// text.slice(cursor ..., text.len() ...))`. Swapping the head slice
/// `text.slice(0, cursor as usize)` with the tail slice
/// `text.slice(cursor as usize, text.len() as usize)` keeps the TOTAL length
/// identical (head_len + ins_len + tail_len) but puts the WRONG bytes in the
/// prefix/suffix windows — so the unchanged-prefix and shifted-suffix `bytes_eq`
/// pins can no longer discharge. If verus still passed this, the pins would be
/// vacuous (R-DEFER-9).
fn swap_head_tail_in_insert_str(src: &str) -> String {
    let head = "text.slice(0, cursor as usize)";
    let tail = "text.slice(cursor as usize, text.len() as usize)";
    assert!(
        src.contains(head) && src.contains(tail),
        "the lowered insert_str body must contain both the head and tail slices to swap \
         (AC-16 setup):\n{src}"
    );
    let sentinel = "text.slice(__HEAD__)";
    let s1 = src.replacen(head, sentinel, 1);
    let s2 = s1.replacen(tail, head, 1);
    s2.replacen(sentinel, tail, 1)
}
