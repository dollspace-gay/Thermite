//! DIVERGENCE PIN — Cluster C12 (#123/#114) design-AC miss: the external cert/golden
//! oracle for `Map<K, V>` (the C7-#100 oracle-gap class, recurring).
//!
//! AUTHORITY: `.design/basis/13-map.md` "Acceptance criteria" — the doc explicitly
//! mandates, as the EXTERNAL truth the toolchain does NOT author for itself (goal.md
//! verification model (A)/(B); R-CHAR-3):
//!
//!   > The orchestrator authors a NEW corpus program — `conformance/map_kv.th` [...]
//!   > and its golden lowering at `tests/golden/lower/map_kv.verus.rs`, hand-authored
//!   > from the GROUNDED form below and confirmed to pass `verus`. The cert golden
//!   > lives at `conformance/map_kv.cert.json`.
//!
//! And the "Routes to add" block mandates `reference = ["conformance/map_kv.th"]` (the
//! syntax/spec routes) and `reference = ["tests/golden/lower/map_kv.verus.rs"]` (the
//! `lower.rs` route).
//!
//! DIVERGENCE: commit `0c75cc0` ships `conformance/map_kv.th` but NEITHER
//! design-mandated golden artifact: `conformance/map_kv.cert.json` (the cert oracle,
//! goal.md model (B)) and `tests/golden/lower/map_kv.verus.rs` (the golden lowering,
//! goal.md model (A)) are both ABSENT. C12's lowering/cert is verified ONLY against the
//! real-`verus`-on-emitted-source harness in `map_conformance.rs` — that grounds the
//! emitted Verus VERIFIES, but it is NOT the diffable hand-authored golden the design AC
//! enumerates: there is no R-CHAR-3 oracle pinning the EXACT cert field shape
//! (`conformance/map_kv.cert.json`) or the EXACT emitted lowering bytes
//! (`tests/golden/lower/map_kv.verus.rs`). Per goal.md verification model (A)/(B), the
//! deliverable for `thermite-lower`/`forge` is the lowering/certificate MATCHING a
//! hand-authored golden; a "verus says 0 errors" harness is a weaker oracle (it cannot
//! catch a lowering that verifies but drifts from the design's GROUNDED `TMapU64U64`
//! shape, e.g. a different abstraction that happens to also verify).
//!
//! The expected file SET below is the design AC's enumerated artifact list, NOT copied
//! from any toolchain output (R-CHAR-3).
//!
//! Tracking: crosslink #124.

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .expect("repo root resolves")
}

/// DIVERGENCE — the C12 cert golden + the golden Verus lowering, both enumerated by
/// `.design/basis/13-map.md` "Acceptance criteria" / "Routes to add", MUST exist. They
/// do not in `0c75cc0` (the C12 external oracle is absent; only `conformance/map_kv.th`
/// was authored). Un-ignore when the orchestrator authors the two goldens.
#[test]
fn divergence_c12_map_cert_and_lowering_goldens_exist() {
    let root = repo_root();
    // The design-AC-enumerated artifact set (authority: 13-map.md "Acceptance criteria"
    // / "Routes to add" — NOT toolchain output, R-CHAR-3).
    let required = [
        "conformance/map_kv.cert.json",
        "tests/golden/lower/map_kv.verus.rs",
    ];
    let missing: Vec<&str> = required
        .iter()
        .copied()
        .filter(|rel| !root.join(rel).exists())
        .collect();
    assert!(
        missing.is_empty(),
        "DESIGN 13-map.md Acceptance criteria: the C12 external cert/golden oracle MUST \
         be authored (goal.md verification model (A)/(B); R-CHAR-3). Missing \
         design-mandated artifacts: {missing:?}. C12 is currently verified only against \
         the real-verus-on-emitted harness in map_conformance.rs, which grounds that the \
         emitted Verus verifies but is NOT the diffable hand-authored golden the design \
         AC enumerates (no oracle pins the exact cert field shape or the exact lowering \
         bytes against the GROUNDED TMapU64U64 form)."
    );
}
