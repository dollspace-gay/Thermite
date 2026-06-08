//! Exhaustive equivalence test anchoring `effects::subsumes` to the
//! Verus-verified subset relation (epic #60, `.design/verified/self-verification.md`
//! mechanism (c), AC-4 / REQ-5).
//!
//! The build probe (OQ-1/OQ-2) showed mechanism (b) (linking the verified crate
//! into the cargo build) is not viable for v1: the installed `vstd`/`builtin`
//! crates inherit `workspace.lints` and a `verus!{}` exec body with an `ensures`
//! clause is verus-driver-only syntax. So we land (c): the verified relation is
//! a proved oracle, and THIS test enumerates the ENTIRE finite input domain
//! (2^9 × 2^9 = 262144 (caller_mask, callee_mask) pairs over the 9-atom `u16`
//! bitset, WIDENED for the #106 `Term` atom) and asserts `effects::subsumes`
//! (over `EffectRow`s decoded from the masks) equals the verus-proved subset
//! relation `thermite_verified::spec_subsumes_mask` for EVERY pair. Since the
//! domain is finite and fully enumerated with 0 mismatches, this PROVES
//! `effects::subsumes` computes exactly the relation `verus` proved
//! `thermite_verified::subsumes` implements → `effects::subsumes` is transitively
//! verus-anchored.
//!
//! R-CHAR-3: the expected value is the verus-verified spec relation
//! (`spec_subsumes_mask`, an EXTERNAL truth proved by `verus --no-cheating`),
//! NEVER the checker's own output. `unwrap`/`expect` are fine here — `tests/` is
//! not anti-pattern-gated.

use thermite_lower::subsumes;
use thermite_syntax::ast::{Effect, EffectRow};

/// The number of meaningful atom bits in the widened `u16` bitset (Read=0 ..
/// Term=8). The domain the proved relation is total over is `0..512`.
const ATOM_DOMAIN: u16 = 512;

/// Decode a 9-atom `u16` mask to the `EffectRow` `effects::subsumes` consumes.
/// Bit positions MUST match `EffectKind::bit` in `effects.rs` and the verus
/// core's atom ordering: Read=0, Write=1, Net=2, Alloc=3, Time=4, Rand=5,
/// Panic=6, Diverge=7, Term=8 (the #106 terminal-control atom). Path-carrying
/// atoms use a representative path (v0.1 subsumption is path-insensitive, OQ-1).
fn row_from_mask(mask: u16) -> EffectRow {
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
    if mask & (1 << 8) != 0 {
        effects.push(Effect::Term);
    }
    EffectRow::Set(effects)
}

/// AC-4: over ALL 262144 (caller, callee) mask pairs (the 9-atom u16 domain),
/// `effects::subsumes` equals the verus-proved subset relation
/// `thermite_verified::spec_subsumes_mask`.
#[test]
fn subsumes_matches_verified_spec_exhaustively() {
    let mut checked: u32 = 0;
    let mut mismatches: u32 = 0;
    for caller in 0u16..ATOM_DOMAIN {
        for callee in 0u16..ATOM_DOMAIN {
            // The EXTERNAL truth: the verus-verified subset relation (proved by
            // `verus --no-cheating`, see tests/verus_verify.rs).
            let expected = thermite_verified::spec_subsumes_mask(caller, callee);
            // The toolchain's decision over the decoded rows.
            let actual = subsumes(&row_from_mask(caller), &row_from_mask(callee));
            if actual != expected {
                mismatches += 1;
                eprintln!(
                    "MISMATCH caller={caller:#011b} callee={callee:#011b}: \
                     effects::subsumes={actual} verus_spec={expected}"
                );
            }
            checked += 1;
        }
    }
    assert_eq!(
        checked, 262144,
        "must enumerate the entire 2^9 x 2^9 domain"
    );
    assert_eq!(
        mismatches, 0,
        "effects::subsumes must equal the verus-verified subset relation for \
         every one of the 262144 mask pairs (mechanism (c), AC-4)"
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
    for caller in 0u16..ATOM_DOMAIN {
        for callee in 0u16..ATOM_DOMAIN {
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
/// `true` — Pure (mask 0) does not subsume {Read} (mask 1). Also the new #106
/// `Term` atom is genuinely constraining: a `write`-only row (mask 1<<1) does NOT
/// subsume a `term` row (mask 1<<8) — the dedicated atom is not folded into
/// `write`. Guards against a vacuous contract (R-DEFER-9).
#[test]
fn verified_spec_is_not_vacuous() {
    assert!(
        !thermite_verified::spec_subsumes_mask(0, 1),
        "Pure must NOT subsume {{Read}} — the relation is non-vacuous"
    );
    assert!(
        !thermite_verified::spec_subsumes_mask(1 << 1, 1 << 8),
        "a write-only caller must NOT subsume a term callee — term is a dedicated \
         atom, not folded into write (#106)"
    );
    assert!(
        thermite_verified::spec_subsumes_mask(1 << 8, 1 << 8),
        "a term caller subsumes a term callee (reflexive on the new atom)"
    );
    assert!(
        thermite_verified::spec_subsumes_mask(0x1FF, 0x1FF),
        "top (all 9 atoms) subsumes top (sanity)"
    );
    assert!(
        thermite_verified::spec_subsumes_mask(0, 0),
        "Pure subsumes Pure (reflexive)"
    );
}
