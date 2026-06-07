//! DIVERGENCE PIN — Cluster C7 (#95) design-AC miss: the external cert/golden oracle.
//!
//! AUTHORITY: `.design/basis/09-option-result.md` "Acceptance criteria" + "Routes to
//! add" — the doc explicitly mandates, as the EXTERNAL truth the toolchain does not
//! author for itself (goal.md verification model (B); R-CHAR-3):
//!
//!   > The orchestrator authors a NEW corpus program — `conformance/option_result.th`
//!   > (`Some`/`None`/`Ok`/`Err` construct + match + `is` + a payload-in-contract
//!   > `ens`, certifying L3) and extends the C4 string corpus with
//!   > `conformance/parse_u64.th` (the `String`→`Option<u64>` parser, certifying L3
//!   > pure). Their golden lowerings live at
//!   > `tests/golden/lower/option_result.verus.rs` /
//!   > `tests/golden/lower/parse_u64.verus.rs` [...]. The cert goldens live at
//!   > `conformance/option_result.cert.json` / `conformance/parse_u64.cert.json`.
//!
//! And the "Routes to add" block sets `reference = ["conformance/option_result.th",
//! "conformance/parse_u64.th"]` / `["tests/golden/lower/option_result.verus.rs",
//! "tests/golden/lower/parse_u64.verus.rs"]` on the C7 routes.
//!
//! DIVERGENCE: commit `bcd5ede` ships NONE of these four artifacts (the routes carry
//! `reference = []`). C7 is verified ONLY against ephemeral temp-file `.th` programs in
//! `option_result_conformance.rs`, NOT against the design-mandated EXTERNAL cert oracle
//! (`conformance/<name>.cert.json`) / golden lowering (`tests/golden/lower/
//! <name>.verus.rs`). Per goal.md verification model (B) and R-CHAR-3, the deliverable
//! for `forge`/`thermite-lower` is the certificate/lowering MATCHING a hand-authored
//! golden — an ephemeral temp program is not that oracle.
//!
//! The expected file SET below is the design AC's enumerated artifact list, NOT copied
//! from any toolchain output (R-CHAR-3).
//!
//! Tracking: crosslink #100.

use std::path::PathBuf;

/// The four C7 acceptance-criteria artifacts the design doc enumerates by name.
/// (Paths relative to the repo root — `CARGO_MANIFEST_DIR` is `forge/`.)
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .expect("repo root resolves")
}

/// DIVERGENCE — the C7 corpus programs + their `.cert.json` cert goldens + the golden
/// Verus lowerings, all enumerated by `.design/basis/09-option-result.md` "Acceptance
/// criteria", MUST exist. They do not in `bcd5ede` (the C7 external oracle is absent).
#[test]
#[ignore = "blocker #100 — un-ignore when fixed: C7 corpus + cert/lowering goldens absent (09-option-result.md Acceptance criteria)"]
fn divergence_c7_corpus_and_cert_goldens_exist() {
    let root = repo_root();
    // The design-AC-enumerated artifact set (authority: 09-option-result.md
    // "Acceptance criteria" / "Routes to add" — NOT toolchain output).
    let required = [
        "conformance/option_result.th",
        "conformance/option_result.cert.json",
        "conformance/parse_u64.th",
        "conformance/parse_u64.cert.json",
        "tests/golden/lower/option_result.verus.rs",
        "tests/golden/lower/parse_u64.verus.rs",
    ];
    let missing: Vec<&str> = required
        .iter()
        .copied()
        .filter(|rel| !root.join(rel).exists())
        .collect();
    assert!(
        missing.is_empty(),
        "DESIGN 09-option-result.md Acceptance criteria: the C7 external cert/golden \
         oracle MUST be authored (goal.md verification model (B); R-CHAR-3). Missing \
         design-mandated artifacts: {missing:?}. C7 is currently verified only against \
         ephemeral temp `.th` programs in option_result_conformance.rs, which is NOT \
         the external oracle the design AC enumerates."
    );
}
