//! Divergence pin for Cluster **C10** (crosslink **#112**), audit of commit
//! `d72e60f`. The C10 REQ-3 rule (`.design/basis/11-ergonomics.md`) — restated in
//! the doc's Architecture section — is:
//!
//! > A **guarded** arm covers NONE of its pattern's cases (the guard may be
//! > false) — GROUNDED: Verus rejects a guarded-only `Some` arm as
//! > non-exhaustive.
//!
//! and AC-3b: "a guard does NOT complete a match ... The validator MUST reject
//! it (matching Verus's `error[E0004]: non-exhaustive patterns`)."
//!
//! DIVERGENCE: `validator::check_match_exhaustiveness` identifies the matched
//! enum ONLY from an arm whose pattern NAMES a declared variant
//! (`variant_pattern_name`). A match whose only arm is a GUARDED catch-all
//! (`_ if cond => …`) over an enum names no variant, so `matched_enum` is `None`
//! and the function returns early WITHOUT any exhaustiveness check — accepting a
//! match that the design's own rule says is non-exhaustive (the guard may fail,
//! so `_ if cond` covers nothing). The corpus test
//! `forge/tests/ergonomics_conformance.rs::req3_guarded_only_arm_is_non_exhaustive`
//! only exercises the case where a SIBLING `No` arm names a variant (so the enum
//! is detected); the all-catch-all case is unpinned.
//!
//! Verus DOES backstop this end-to-end (the lowered match is `error[E0004]`
//! non-exhaustive → forge reports L0), so this is NOT an L3-laundering soundness
//! hole; it is a validator-completeness divergence from the design's literal
//! REQ-3 rule (the toolchain should pre-empt with a structured
//! `NonExhaustiveMatch`, not defer to an opaque verus L0).
//!
//! AUTHORITY: `.design/basis/11-ergonomics.md` REQ-3 / AC-3b ("a guard does NOT
//! complete a match"). The expected outcome (`NonExhaustiveMatch`) is
//! hand-derived from the design rule (R-CHAR-3 — not copied from the toolchain;
//! the toolchain currently returns `Ok(())`, the opposite).
//!
//! Tracking: blocker #120.

use thermite_spec::{validate, SpecError};

/// REQ-3 / AC-3b — a match over `enum Maybe { Yes(u64), No }` whose ONLY arm is a
/// guarded catch-all (`_ if true => 0`) is NON-exhaustive: a guard does NOT
/// complete a match, so neither `Yes` nor `No` is covered. The validator MUST
/// emit `SpecError::NonExhaustiveMatch`. It currently returns `Ok(())` because
/// the enum is never identified (no arm names a variant).
#[test]
#[ignore = "blocker #120 — un-ignore when fixed"]
fn divergence_guarded_only_catchall_is_non_exhaustive() {
    let parsed = thermite_syntax::parse(
        "enum Maybe { Yes(u64), No } fn f(m: Maybe) -> u64 req true ens result == 0 fx pure { match m { _ if true => 0 } }",
    );
    assert!(parsed.is_clean(), "must parse: {:?}", parsed.errors);

    let result = validate(&parsed.program);
    let errors = result.expect_err(
        "DESIGN 11-ergonomics.md REQ-3 / AC-3b: a guarded catch-all `_ if true => 0` \
         does NOT complete a match (the guard may fail), so a match over `Maybe` with \
         only that arm is NON-exhaustive — `validate` must reject it with \
         NonExhaustiveMatch, not return Ok(()).",
    );
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SpecError::NonExhaustiveMatch { .. })),
        "DESIGN 11-ergonomics.md REQ-3 / AC-3b: expected a NonExhaustiveMatch (a guard \
         does NOT complete a match); got {errors:?}"
    );
}
