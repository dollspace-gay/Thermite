//! Thermite's Verus-verified soundness-critical pure core (epic #60, Tier 1).
//!
//! Governing design: `.design/verified/self-verification.md` (REQ-1..REQ-6).
//! Thesis: `thermite-design.md` §6 (Verus is the L3 prover), §9 (the TCB is
//! slag ∪ boundary ∪ the toolchain itself — this crate SHRINKS that TCB by
//! moving a soundness-critical decision function OUT of unverified Rust).
//!
//! ## What this crate is
//!
//! The FIRST proven increment is effect subsumption (`subsumes`): a wrong answer
//! mints a false `pure` certificate for an effectful function (§4.1 / §9). The
//! decision is ported to a bounded **8-atom `u8` bitset** (Read=0 .. Diverge=7,
//! the path-insensitive atom-kind projection `EffectKind::of` already computes in
//! `thermite-lower`), where subsumption is the mask test `(callee & !caller) == 0`
//! and the genuine subset relation `effects(callee) ⊆ effects(caller)` is the
//! explicit 8-way conjunction over the bit positions. The two are proved
//! equivalent by Verus `bit_vector`-mode SMT.
//!
//! ## The landed mechanism: (c) exhaustive equivalence (NOT (b) delegation)
//!
//! `.design/verified/self-verification.md` chose mechanism (b) (link the verified
//! crate into the cargo build so the proved code IS the running code) IF the
//! `--export`/`--import` linking landed cleanly, ELSE fall back to (c) (a verified
//! reference + an enumerated impl==spec conformance test). The OQ-1/OQ-2 build
//! probe (epic #60) settled this EMPIRICALLY: the installed Verus support crates
//! (`builtin`/`builtin_macros`/`vstd`) cannot be consumed as cargo path-deps —
//! they inherit `workspace.lints` from the Verus workspace root (so `cargo
//! metadata` fails outside it), carry `cfg(verus_keep_ghost)` lint configs cargo
//! rejects, and the macro expansion resolves a renamed `verus_builtin` crate. The
//! `verus!{}` exec body ALSO cannot plain-`rustc`-compile: a function carrying an
//! `ensures` clause is verus-driver-only syntax. So (b) is not viable for v1 and
//! we land (c), exactly as the doc's decision rule prescribes.
//!
//! Under (c):
//! - The full `verus!{}` proof (the `subsumes` exec fn + `spec_subsumes` subset
//!   relation + three lattice-law `proof fn`s) lives in [`verus_core`], gated
//!   behind `#[cfg(verus_keep_ghost)]` — a cfg ONLY the real `verus` driver sets,
//!   so a normal `cargo build` never sees it. `tests/verus_verify.rs` runs
//!   `verus --no-cheating src/lib.rs` and gates on `0 errors` (REQ-6 / AC-6).
//! - The always-cargo-compiled plain Rust ([`subsumes_masks`] / its spec
//!   [`spec_subsumes_mask`]) is BYTE-IDENTICAL to the verus exec body / spec. The
//!   toolchain delegates the mask comparison to [`subsumes_masks`]; the
//!   exhaustive 2^8 × 2^8 = 65536-pair equivalence test
//!   (`tests/equivalence` over in `thermite-lower`) asserts the running code
//!   equals the proved subset relation for EVERY input — finite + fully
//!   enumerated, so this PROVES `effects::subsumes` computes exactly the relation
//!   Verus proved (transitively verus-anchored).
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (self-verification architecture) | SHIPPED | the `verus!{}` body in `verus_core` (verified by `verus`, Thermite's L3 rung §6); mechanism (c) recorded (b empirically infeasible); verified by `tests/verus_verify.rs` (`verus --no-cheating` → 0 errors). |
//! | REQ-2 (Tier-1 target list + porting pattern) | NOT-STARTED | epic #60. `subsumes` is the FIRST and ONLY target this increment; the other seven Tier-1 fns remain plain Rust (ported one at a time after the mechanism is proven, REQ-6). |
//! | REQ-3 (Tier-2/Tier-3 boundaries) | SHIPPED | this crate has NO I/O and NO `external_body` — the Tier-1 core (`subsumes`) carries a real `ensures`, no Tier-3 floor is reached. AC-5: a grep shows zero `external_body`/`external` in `src/`. |
//! | REQ-4 (honesty — genuine proof) | SHIPPED | `verus --no-cheating` (no `assume`/`admit`/`external_body`); the `ensures result == spec_subsumes(..)` is non-vacuous (negating the body → `7 verified, 1 errors`, demonstrated by `tests/verus_verify.rs::broken_subsumes_fails_verification`). |
//! | REQ-5 (FIRST increment — `subsumes` verified + delegated/matched) | SHIPPED | `verus_core::subsumes` proved; `subsumes_masks` (plain mirror) consumed by `thermite_lower::effects::subsumes`; the lattice laws (reflexive / Pure-subsumes-only-Pure / top-subsumes-all) are three `proof fn`s; the 14 `effects` tests still pass (behavior preserved). |
//! | REQ-6 (CI-able verus-verify gauntlet step) | SHIPPED | `tests/verus_verify.rs` runs the real `verus --no-cheating src/lib.rs` (skip-loud if verus absent, like `lower_conformance`) and asserts `verified, 0 errors`; a core fn that fails to verify is a HARD test failure (R-DEFER-6). |

/// The number of atomic effect kinds (`thermite_syntax::ast::Effect` →
/// `thermite_lower::effects::EffectKind`): Read=0, Write=1, Net=2, Alloc=3,
/// Time=4, Rand=5, Panic=6, Diverge=7. The bitset is a `u8` (one bit per atom),
/// so bits 0..8 are meaningful and the relation is total over all 256 masks.
pub const ATOM_COUNT: u8 = 8;

/// The executable effect-subsumption decision over the 8-atom bitset (REQ-5):
/// `caller` subsumes `callee` iff `callee` has no atom the `caller` lacks, i.e.
/// `(callee & !caller) == 0`. This plain-Rust body is BYTE-IDENTICAL to the
/// `verus_core::subsumes` exec body the Verus prover discharges against the
/// subset-relation `ensures` (mechanism (c) — the running code mirrors the proved
/// code). [`spec_subsumes_mask`] is the spec it is proved to compute.
///
/// `thermite_lower::effects::subsumes` delegates its bit-level comparison here
/// (the production consumer, R-DEFER-1); the exhaustive 65536-pair equivalence
/// test anchors `effects::subsumes` to this verus-verified relation.
#[must_use]
pub fn subsumes_masks(caller: u8, callee: u8) -> bool {
    let missing = callee & !caller;
    missing == 0
}

/// The genuine subset relation `effects(callee) ⊆ effects(caller)` over the
/// 8-atom bitset, as the explicit per-atom conjunction (REQ-4 — the NON-trivial
/// contract `subsumes_masks` is proved to compute). For each atom position `i`,
/// if `callee` has atom `i` then `caller` must have it. This is the plain-Rust
/// mirror of `verus_core::spec_subsumes`; the Verus `bit_vector` proof shows
/// `subsumes_masks(c, k) == spec_subsumes_mask(c, k)` for all `(c, k)`, and the
/// exhaustive equivalence test re-checks the mirror over the full domain.
///
/// NON-VACUITY: this returns `false` for `caller=0, callee=1` (Pure does not
/// subsume {Read}), so the relation is genuinely constraining (not `true`).
#[must_use]
pub fn spec_subsumes_mask(caller: u8, callee: u8) -> bool {
    let mut i: u8 = 0;
    while i < ATOM_COUNT {
        let bit = 1u8 << i;
        if (callee & bit) != 0 && (caller & bit) == 0 {
            return false;
        }
        i += 1;
    }
    true
}

// ---------------------------------------------------------------------------
// The Verus-verified core (REQ-1/REQ-4/REQ-5). Compiled ONLY by the real `verus`
// driver, which sets `cfg(verus_keep_ghost)`; a normal `cargo build` skips it
// entirely (the `verus!{}` macro + `ensures` syntax is verus-driver-only). The
// proof is run by `tests/verus_verify.rs` (`verus --no-cheating src/lib.rs`).
//
// AC-5: NO `#[verifier::external_body]` / `external` appears here — `subsumes` is
// a Tier-1 core fn with a real `ensures`, not a Tier-3 I/O shim.
// ---------------------------------------------------------------------------
#[cfg(verus_keep_ghost)]
mod verus_core {
    use vstd::prelude::*;

    verus! {

    /// Atom `i` is present in `mask` (bit `i` set). 8 atoms: Read=0 .. Diverge=7.
    pub open spec fn has(mask: u8, i: u8) -> bool {
        (mask & (1u8 << i)) != 0
    }

    /// The genuine subset relation `effects(callee) ⊆ effects(caller)`, as the
    /// explicit 8-way conjunction over the atom positions (mirrors the plain-Rust
    /// `spec_subsumes_mask`). NON-vacuous (REQ-4): false when callee has an atom
    /// caller lacks.
    pub open spec fn spec_subsumes(caller: u8, callee: u8) -> bool {
        &&& (has(callee, 0) ==> has(caller, 0))
        &&& (has(callee, 1) ==> has(caller, 1))
        &&& (has(callee, 2) ==> has(caller, 2))
        &&& (has(callee, 3) ==> has(caller, 3))
        &&& (has(callee, 4) ==> has(caller, 4))
        &&& (has(callee, 5) ==> has(caller, 5))
        &&& (has(callee, 6) ==> has(caller, 6))
        &&& (has(callee, 7) ==> has(caller, 7))
    }

    /// The executable mask test, PROVED equal to the subset relation for ALL
    /// inputs (the L3 guarantee, §6). Byte-identical to the plain-Rust
    /// `subsumes_masks` the toolchain runs.
    pub fn subsumes(caller: u8, callee: u8) -> (r: bool)
        ensures r == spec_subsumes(caller, callee),
    {
        assert((callee & !caller == 0) == spec_subsumes(caller, callee)) by (bit_vector);
        let missing = callee & !caller;
        missing == 0
    }

    /// Lattice law 1 (`.design/lower/effect-subsumption.md` REQ-1): reflexive —
    /// every row subsumes itself.
    proof fn lattice_reflexive(row: u8)
        ensures spec_subsumes(row, row),
    {
        assert(spec_subsumes(row, row)) by (bit_vector);
    }

    /// Lattice law 2: Pure (the empty set, mask 0) subsumes ONLY Pure.
    proof fn lattice_pure_subsumes_only_pure(callee: u8)
        ensures spec_subsumes(0u8, callee) == (callee == 0),
    {
        assert(spec_subsumes(0u8, callee) == (callee == 0)) by (bit_vector);
    }

    /// Lattice law 3: the top row (all 8 atoms, mask 0xFF) subsumes every row.
    proof fn lattice_top_subsumes_all(callee: u8)
        ensures spec_subsumes(0xFFu8, callee),
    {
        assert(spec_subsumes(0xFFu8, callee)) by (bit_vector);
    }

    }
}
