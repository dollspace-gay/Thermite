//! Exhaustive equivalence test anchoring `effects::subsumes` to the
//! Verus-verified subset relation (epic #60, `.design/verified/self-verification.md`
//! mechanism (c), AC-4 / REQ-5).
//!
//! The build probe (OQ-1/OQ-2) showed mechanism (b) (linking the verified crate
//! into the cargo build) is not viable for v1: the installed `vstd`/`builtin`
//! crates inherit `workspace.lints` and a `verus!{}` exec body with an `ensures`
//! clause is verus-driver-only syntax. So we land (c): the verified relation is
//! a proved oracle, and THIS test enumerates the ENTIRE finite input domain
//! (2^8 × 2^8 = 65536 (caller_mask, callee_mask) pairs) and asserts
//! `effects::subsumes` (over `EffectRow`s decoded from the masks) equals the
//! verus-proved subset relation `thermite_verified::spec_subsumes_mask` for EVERY
//! pair. Since the domain is finite and fully enumerated with 0 mismatches, this
//! PROVES `effects::subsumes` computes exactly the relation `verus` proved
//! `thermite_verified::subsumes` implements → `effects::subsumes` is transitively
//! verus-anchored.
//!
//! R-CHAR-3: the expected value is the verus-verified spec relation
//! (`spec_subsumes_mask`, an EXTERNAL truth proved by `verus --no-cheating`),
//! NEVER the checker's own output. `unwrap`/`expect` are fine here — `tests/` is
//! not anti-pattern-gated.

use thermite_lower::subsumes;
use thermite_syntax::ast::{Effect, EffectRow};

/// Decode an 8-atom `u8` mask to the `EffectRow` `effects::subsumes` consumes.
/// Bit positions MUST match `EffectKind::bit` in `effects.rs` and the verus
/// core's atom ordering: Read=0, Write=1, Net=2, Alloc=3, Time=4, Rand=5,
/// Panic=6, Diverge=7. Path-carrying atoms use a representative path (v0.1
/// subsumption is path-insensitive, OQ-1).
fn row_from_mask(mask: u8) -> EffectRow {
    if mask == 0 {
        return EffectRow::Pure;
    }
    let mut effects = Vec::new();
    if mask & (1 << 0) != 0 {
        effects.push(Effect::Read("p".to_string()));
    }
    if mask & (1 << 1) != 0 {
        effects.push(Effect::Write("p".to_string()));
    }
    if mask & (1 << 2) != 0 {
        effects.push(Effect::Net("d".to_string()));
    }
    if mask & (1 << 3) != 0 {
        effects.push(Effect::Alloc);
    }
    if mask & (1 << 4) != 0 {
        effects.push(Effect::Time);
    }
    if mask & (1 << 5) != 0 {
        effects.push(Effect::Rand);
    }
    if mask & (1 << 6) != 0 {
        effects.push(Effect::Panic);
    }
    if mask & (1 << 7) != 0 {
        effects.push(Effect::Diverge);
    }
    EffectRow::Set(effects)
}

/// AC-4: over ALL 65536 (caller, callee) mask pairs, `effects::subsumes` equals
/// the verus-proved subset relation `thermite_verified::spec_subsumes_mask`.
#[test]
fn subsumes_matches_verified_spec_exhaustively() {
    let mut checked: u32 = 0;
    let mut mismatches: u32 = 0;
    for caller in 0u8..=255 {
        for callee in 0u8..=255 {
            // The EXTERNAL truth: the verus-verified subset relation (proved by
            // `verus --no-cheating`, see tests/verus_verify.rs).
            let expected = thermite_verified::spec_subsumes_mask(caller, callee);
            // The toolchain's decision over the decoded rows.
            let actual = subsumes(&row_from_mask(caller), &row_from_mask(callee));
            if actual != expected {
                mismatches += 1;
                eprintln!(
                    "MISMATCH caller={caller:#010b} callee={callee:#010b}: \
                     effects::subsumes={actual} verus_spec={expected}"
                );
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 65536, "must enumerate the entire 2^8 x 2^8 domain");
    assert_eq!(
        mismatches, 0,
        "effects::subsumes must equal the verus-verified subset relation for \
         every one of the 65536 mask pairs (mechanism (c), AC-4)"
    );
}

/// Cross-check that the verified `subsumes_masks` (the plain-Rust mirror of the
/// verus exec body the toolchain delegates to) and the verified
/// `spec_subsumes_mask` (the proved subset relation) agree over the full domain
/// — i.e. the mirror the `verus` proof's `ensures` constrains is the same
/// function `effects::subsumes` calls. (The `verus` proof guarantees this for
/// all inputs; this re-checks the plain-Rust mirror, R-CHAR-3: spec is the
/// oracle.)
#[test]
fn verified_mirror_equals_spec_exhaustively() {
    for caller in 0u8..=255 {
        for callee in 0u8..=255 {
            assert_eq!(
                thermite_verified::subsumes_masks(caller, callee),
                thermite_verified::spec_subsumes_mask(caller, callee),
                "verified exec mirror must equal the proved subset relation at \
                 caller={caller}, callee={callee}"
            );
        }
    }
}

/// Non-triviality (AC-2 mirror in Rust): the subset relation is NOT the constant
/// `true` — Pure (mask 0) does not subsume {Read} (mask 1). Guards against a
/// vacuous contract (R-DEFER-9).
#[test]
fn verified_spec_is_not_vacuous() {
    assert!(
        !thermite_verified::spec_subsumes_mask(0, 1),
        "Pure must NOT subsume {{Read}} — the relation is non-vacuous"
    );
    assert!(
        thermite_verified::spec_subsumes_mask(0xFF, 0xFF),
        "top subsumes top (sanity)"
    );
    assert!(
        thermite_verified::spec_subsumes_mask(0, 0),
        "Pure subsumes Pure (reflexive)"
    );
}
