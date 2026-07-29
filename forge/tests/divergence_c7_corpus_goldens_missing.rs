//! Divergence pin — Cluster C7 (#95) design-AC miss: the external cert/golden oracle.
//!
//! Authority: `.design/basis/09-option-result.md` "Acceptance criteria" + "Routes to
//! add" — the doc mandates, as the external truth the toolchain does not
//! author for itself (goal.md verification model (B); R-CHAR-3):
//!
//!   > The orchestrator authors a new corpus program — `conformance/option_result.th`
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
//! Divergence: commit `bcd5ede` ships none of these four artifacts (the routes carry
//! `reference = []`). C7 is verified only against ephemeral temp-file `.th` programs in
//! `option_result_conformance.rs`, not against the design-mandated external cert oracle
//! (`conformance/<name>.cert.json`) / golden lowering (`tests/golden/lower/
//! <name>.verus.rs`). Per goal.md verification model (B) and R-CHAR-3, the deliverable
//! for `forge`/`thermite-lower` is the certificate/lowering matching a hand-authored
//! golden; an ephemeral temp program is not that oracle.
//!
//! The expected file set below is the design AC's enumerated artifact list, not copied
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

/// Divergence — the C7 corpus programs, their `.cert.json` cert goldens, and the golden
/// Verus lowerings, all enumerated by `.design/basis/09-option-result.md` "Acceptance
/// criteria", must exist. They do not in `bcd5ede` (the C7 external oracle is absent).
// Resolved (#100): all six C7 external-oracle artifacts now exist —
// `conformance/option_result.{th,cert.json}` + `conformance/parse_u64.{th,cert.json}`
// (the parse corpus is `parse_valid` only; the forced-None refusal demo is the tracked
// §7 equivalent-mutant limitation #101) + `tests/golden/lower/{option_result,
// parse_u64}.verus.rs` (verus `4`/`34 verified, 0 errors`). The pin is un-ignored and
// now passes, guarding against a regression that deletes the external oracle.
#[test]
fn divergence_c7_corpus_and_cert_goldens_exist() {
    let root = repo_root();
    // The design-AC-enumerated artifact set (authority: 09-option-result.md
    // "Acceptance criteria" / "Routes to add", not toolchain output).
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
