//! Divergence: the lowering golden oracle for `sum.th` is STALE with respect to
//! the current emitter on the `1_000_000` literal.
//!
//! Authority (goal.md §B / "Why the critic still has teeth"): the Verus/L1
//! golden files under `tests/golden/` are the EXTERNAL lowering oracle — the
//! emitted source MUST match the golden. ast.md REQ-6 CRITICAL note pins the
//! intended form: lowering "continues to emit the numeric `value` (e.g.
//! `1000000`), NOT the raw — so the `tests/golden/lower/*.verus.rs` files do
//! NOT change". So the AUTHORITY says both (a) the emitter emits `1000000` and
//! (b) the golden contains `1000000`.
//!
//! REALITY (a2c0f73): the emitter emits `1000000` (correct, AC-1b), but the
//! golden files `tests/golden/lower/sum.verus.rs` and `tests/golden/l1/sum.l1.rs`
//! still contain the RAW form `1_000_000` in their executable expressions. The
//! emitter and the golden oracle therefore DISAGREE. No existing test
//! byte-matches these goldens (`lower_conformance.rs` / `l1_conformance.rs`
//! verify via verus/rustc + contract presence, not byte equality), so the
//! staleness was never caught and #37's "no golden churn" check
//! (`git diff tests/golden/` empty) passed over a golden that was ALREADY out
//! of sync (the parent commit a2c0f73^ goldens are identical, also `1_000_000`).
//!
//! This is a real golden-vs-emitter divergence (goal.md R-DEFER-5: every
//! divergence on `main` is ours, no "pre-existing safe to defer"). The fix is
//! the generator's: either regenerate the golden's executable expression to
//! the emitted `1000000`, or — if the design intends the golden to keep the
//! verbatim separators — amend ast.md's CRITICAL note (R-SPEC-4). The critic
//! does not choose; it pins the disagreement.
//!
//! Expected values are hand-derived from ast.md REQ-6 CRITICAL note (`1000000`,
//! the `_`-stripped value), never copied from the emitter (R-CHAR-3).

use std::path::PathBuf;

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("conformance")
}

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("tests")
        .join("golden")
}

fn lower_sum_l3() -> String {
    let src = std::fs::read_to_string(corpus_dir().join("sum.th")).unwrap();
    let parsed = thermite_syntax::parse(&src);
    thermite_lower::lower(&parsed.program).unwrap()
}

fn lower_sum_l1() -> String {
    let src = std::fs::read_to_string(corpus_dir().join("sum.th")).unwrap();
    let parsed = thermite_syntax::parse(&src);
    thermite_lower::lower_l1(&parsed.program).unwrap()
}

/// The L3 emitter and the L3 golden oracle must agree on the lowered
/// `xs.len() <= 1_000_000` requires-clause. ast.md REQ-6 says both emit the
/// value `1000000`. The emitter does; the golden does not.
#[test]
#[ignore = "divergence: L3 golden sum.verus.rs stale (raw 1_000_000 vs emitted 1000000); tracking #54"]
fn divergence_l3_golden_stale_on_underscore_literal() {
    let emitted = lower_sum_l3();
    let golden = std::fs::read_to_string(golden_dir().join("lower").join("sum.verus.rs")).unwrap();

    // The emitter lowers the literal to the stripped value `1000000` (AC-1b).
    assert!(
        emitted.contains("xs.len() <= 1000000"),
        "emitter must lower the req literal to the value 1000000:\n{emitted}"
    );

    // The golden oracle MUST carry the same lowered form (goal.md §B). It does
    // not — it still has the raw `xs.len() <= 1_000_000`. This assertion FAILS,
    // pinning the emitter-vs-golden divergence.
    assert!(
        golden.contains("xs.len() <= 1000000"),
        "L3 golden `tests/golden/lower/sum.verus.rs` is STALE: it must carry the \
         emitted value form `xs.len() <= 1000000` (ast.md REQ-6), but contains \
         the raw `1_000_000`. Emitter and golden oracle disagree."
    );
}

/// The L1 emitter and the L1 golden oracle must agree on the executable check
/// expression for the `1_000_000` literal. The emitter emits `xs.len() <=
/// 1000000`; the golden's executable form is `xs.len() <= 1_000_000`.
/// (The verbatim `1_000_000` inside the diagnostic LABEL string is correct and
/// is not what this asserts — we pin the executable comparison only.)
#[test]
#[ignore = "divergence: L1 golden sum.l1.rs stale executable check (raw 1_000_000 vs emitted 1000000); tracking #54"]
fn divergence_l1_golden_stale_on_underscore_literal() {
    let emitted = lower_sum_l1();
    let golden = std::fs::read_to_string(golden_dir().join("l1").join("sum.l1.rs")).unwrap();

    // Emitter: the executable check is `xs.len() <= 1000000)` (value).
    assert!(
        emitted.contains("xs.len() <= 1000000)"),
        "emitter must lower the L1 check expr to the value 1000000:\n{emitted}"
    );

    // Golden oracle MUST carry the same executable form. It carries
    // `xs.len() <= 1_000_000)` (raw). This assertion FAILS, pinning the
    // emitter-vs-golden divergence.
    assert!(
        golden.contains("xs.len() <= 1000000)"),
        "L1 golden `tests/golden/l1/sum.l1.rs` is STALE: its executable check \
         must be `xs.len() <= 1000000` (the emitted value), but contains the raw \
         `xs.len() <= 1_000_000`. Emitter and golden oracle disagree."
    );
}
