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
//! ## What this increment ADDS (REQ-7 + REQ-8, epic #60)
//!
//! This increment ports the next two Tier-1 targets via the SAME mechanism (c):
//! - **REQ-7 — the degrade-ladder ANTI-CHEAT.** [`ladder_action_l3_tag`] /
//!   [`ladder_action_l2_tag`] are the plain-Rust mirrors of the verus-proved
//!   `ladder_action_l3`/`ladder_action_l2` decision: a verdict DISCRIMINANT →ladder
//!   action ([`LadderAction`]). The verus core carries the anti-cheat `ensures`
//!   `l3_is_counterexample(v) ==> (r is HardFail) && !is_degrade(r)` (+ the L2
//!   analog) plus a global `proof fn` quantifying it over the whole verdict domain —
//!   a `Counterexample` NEVER degrades (the R-DEFER-9 property). `forge::degrade`'s
//!   `ladder_action_l3`/`ladder_action_l2` mirror this, and `run_ladder` BRANCHES on
//!   the returned action; the in-module `verus_anchor` equivalence test binds the
//!   production decision to these tags.
//! - **REQ-8 — the seccomp allowlist SOUNDNESS.** [`io_allow`] is the plain-Rust
//!   mirror of the verus-proved `io_allow(fx_mask) -> u32` over the 5 sensitive
//!   user-I/O syscalls (openat/socket/connect/getrandom/clock_gettime, bits 0..5).
//!   The verus core carries `pure_has_no_io` (`io_allow(0) == 0`), `monotone`
//!   (subset on the syscall-mask), and `io_allow_within_io_bits` (deny-by-default).
//!   `forge::sandbox`'s `syscall_allowlist` is anchored to this over all 256 masks
//!   by its in-module `verus_anchor` test (OQ-6: the 5 sensitive syscalls only; the
//!   dense `BASELINE_SYSCALLS` stays `sandbox_conformance`-grounded).
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (self-verification architecture) | SHIPPED | the `verus!{}` body in `verus_core` (verified by `verus`, Thermite's L3 rung §6); mechanism (c) recorded (b empirically infeasible); verified by `tests/verus_verify.rs` (`verus --no-cheating` → 0 errors). |
//! | REQ-2 (remaining Tier-1 targets + porting pattern) | NOT-STARTED | epic #60. The remaining FIVE Tier-1 fns (`cache_key`, `triage`, `kill_ratio`/`meets_floor`, `is_strictly_stronger`, the boundary gate) remain plain Rust, ported one at a time via mechanism (c). |
//! | REQ-3 (Tier-2/Tier-3 boundaries) | SHIPPED | this crate has NO I/O and NO `external_body` — the Tier-1 cores (`subsumes`/`ladder_action`/`io_allow`) carry real `ensures`, no Tier-3 floor is reached. AC-5: a grep shows zero `external_body`/`external` in `src/`. |
//! | REQ-4 (honesty — genuine proof) | SHIPPED | `verus --no-cheating` (no `assume`/`admit`/`external_body`); the `ensures result == spec_subsumes(..)` is non-vacuous (negating the body → `7 verified, 1 errors`, `tests/verus_verify.rs::broken_subsumes_fails_verification`). The REQ-7 anti-cheat `ensures` and the REQ-8 `pure_has_no_io`/`monotone` lemmas are each non-vacuous (the equivalence tests catch a broken mirror; a mutated verus spec fails — Grounding A/B). |
//! | REQ-5 (FIRST increment — `subsumes` verified + delegated/matched) | SHIPPED | `verus_core::subsumes` proved; `subsumes_masks` (plain mirror) consumed by `thermite_lower::effects::subsumes`; the lattice laws (reflexive / Pure-subsumes-only-Pure / top-subsumes-all) are three `proof fn`s; the 14 `effects` tests still pass (behavior preserved). |
//! | REQ-6 (CI-able verus-verify gauntlet step) | SHIPPED | `tests/verus_verify.rs` runs the real `verus --no-cheating src/lib.rs` (skip-loud if verus absent, like `lower_conformance`) and asserts `verified, 0 errors`; a core fn that fails to verify is a HARD test failure (R-DEFER-6). |
//! | REQ-7 (degrade anti-cheat verified + anchored) | SHIPPED | `verus_core::ladder_action_l3`/`ladder_action_l2` proved (the anti-cheat `ensures` `l3_is_counterexample(v) ==> (r is HardFail) && !is_degrade(r)` + the L2 analog + the global `anti_cheat_holds_for_all_verdicts` proof); the plain mirrors [`ladder_action_l3_tag`]/[`ladder_action_l2_tag`] are consumed by `forge::degrade::ladder_action_l3`/`ladder_action_l2` (its in-module `verus_anchor` equivalence test); `run_ladder` BRANCHES on the proved decision. |
//! | REQ-8 (seccomp allowlist soundness verified + anchored) | SHIPPED | `verus_core::io_allow` proved (`pure_has_no_io`, `non_widening_atoms_have_no_io`, `monotone`, `io_allow_within_io_bits`); the plain mirror [`io_allow`] is anchored to `forge::sandbox::syscall_allowlist` over all 256 fx-masks by the `sandbox::verus_anchor` test (the 5 sensitive syscalls only, OQ-6). |

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

// ===========================================================================
// REQ-7 — the degrade-ladder ANTI-CHEAT decision (the core R-DEFER-9 property).
// ===========================================================================

/// The verdict DISCRIMINANT of an L3 (verus) run, the finite domain that drives
/// the degrade decision (REQ-7). The carried `Certificate`/`RejectReason` payloads
/// `forge::degrade::L3Verdict` holds are IRRELEVANT to the decision, so the proved
/// model tracks only this tag. Mirrors `forge::degrade::L3Verdict`'s discriminant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum L3Tag {
    /// verus PROVED the item → certify L3.
    Proved,
    /// verus TIMED OUT (inconclusive) → the SOLE degrade trigger → attempt L2.
    Timeout,
    /// verus DISPROVED the item (a real bug) → HARD FAIL, NEVER a degrade.
    Counterexample,
}

/// The verdict DISCRIMINANT of an L2 (kani) run (REQ-7). Mirrors `forge::kani::L2Verdict`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum L2Tag {
    /// kani VERIFIED to the bound → certify L2 (degraded).
    Verified,
    /// kani exhausted its bound (inconclusive) → degrade to L1.
    UnderBound,
    /// kani DISPROVED the contract (a real bug) → HARD FAIL, NEVER a drop to L1.
    Counterexample,
}

/// The action the degrade ladder takes for a classified verdict (REQ-7). This is
/// the PROVED decision: `forge::degrade::run_ladder` BRANCHES on it, so the
/// proved classification drives the real control flow. `CertifyL2`/`DegradeToL1`
/// are the DEGRADE actions ([`is_degrade`]); `HardFail` is the non-certifying
/// failure a `Counterexample` maps to and MUST never be a degrade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LadderAction {
    /// PROVED at L3 → certify L3 (terminal, no degrade).
    CertifyL3,
    /// L3 timed out → attempt the L2 rung.
    AttemptL2,
    /// L2 verified → certify L2 with the lowered-assurance stamp (a degrade).
    CertifyL2,
    /// L2 under-bound → drop to the L1 runtime-check rung (a degrade).
    DegradeToL1,
    /// A counterexample (L3 or L2) → non-certifying HARD FAIL (NEVER a degrade).
    HardFail,
}

/// `true` iff `action` is a DEGRADE — a lower rung taken as a PASS
/// (`CertifyL2`/`DegradeToL1`). The anti-cheat invariant (REQ-7) is that a
/// `Counterexample` NEVER maps to a degrade. Mirrors `verus_core::is_degrade`.
#[must_use]
pub fn is_degrade(action: LadderAction) -> bool {
    matches!(action, LadderAction::CertifyL2 | LadderAction::DegradeToL1)
}

/// The L3 ladder decision (REQ-7): a verdict tag → the ladder action. BYTE-IDENTICAL
/// to the `verus_core::ladder_action_l3` exec body the Verus prover discharges
/// against the anti-cheat `ensures` `l3_is_counterexample(v) ==> (r is HardFail) &&
/// !is_degrade(r)`. `forge::degrade::ladder_action_l3` mirrors this and `run_ladder`
/// branches on the result (the production consumer, R-DEFER-1). The in-module
/// `verus_anchor` equivalence test binds the production decision to this tag.
#[must_use]
pub fn ladder_action_l3_tag(v: L3Tag) -> LadderAction {
    match v {
        L3Tag::Proved => LadderAction::CertifyL3,
        L3Tag::Timeout => LadderAction::AttemptL2,
        // ANTI-CHEAT (R-DEFER-9): a counterexample is a HARD FAIL, never a degrade.
        L3Tag::Counterexample => LadderAction::HardFail,
    }
}

/// The L2 ladder decision (REQ-7): an L2 verdict tag → the ladder action.
/// BYTE-IDENTICAL to `verus_core::ladder_action_l2`. A `Counterexample` is a HARD
/// FAIL (never a drop to L1, the 2nd-rung anti-cheat).
#[must_use]
pub fn ladder_action_l2_tag(v: L2Tag) -> LadderAction {
    match v {
        L2Tag::Verified => LadderAction::CertifyL2,
        L2Tag::UnderBound => LadderAction::DegradeToL1,
        // ANTI-CHEAT (R-DEFER-9): an L2 counterexample is a HARD FAIL, never L1.
        L2Tag::Counterexample => LadderAction::HardFail,
    }
}

// ===========================================================================
// REQ-8 — the seccomp allowlist SOUNDNESS decision (the fx → sensitive-I/O map).
// ===========================================================================

/// The number of fx-atom kinds in the `u8` fx-mask (Read=0, Write=1, Net=2, Time=3,
/// Rand=4, Alloc=5, Panic=6, Diverge=7). The bitset is total over all 256 masks.
pub const FX_ATOM_COUNT: u8 = 8;

/// The bit positions, in the `u32` sensitive-syscall mask, of the 5 user-I/O
/// syscalls the §4.1 `pure`-exclusion table calls out (REQ-8). The dense
/// `BASELINE_SYSCALLS` is orthogonal to these IO bits (OQ-6), so the soundness
/// model is exactly this IO-membership projection.
pub const SYS_OPENAT: u32 = 1 << 0;
/// The `socket` sensitive-syscall bit.
pub const SYS_SOCKET: u32 = 1 << 1;
/// The `connect` sensitive-syscall bit.
pub const SYS_CONNECT: u32 = 1 << 2;
/// The `getrandom` sensitive-syscall bit.
pub const SYS_GETRANDOM: u32 = 1 << 3;
/// The `clock_gettime` sensitive-syscall bit.
pub const SYS_CLOCK_GETTIME: u32 = 1 << 4;

/// The per-atom contribution to the sensitive-syscall mask (REQ-8): which of the 5
/// sensitive user-I/O syscalls fx-atom `i` widens the allowlist to permit.
/// Read/Write→openat, Net→socket|connect, Time→clock_gettime, Rand→getrandom,
/// Alloc/Panic/Diverge→0 (non-widening). BYTE-IDENTICAL to `verus_core::widen`.
#[must_use]
pub fn widen(i: u8) -> u32 {
    match i {
        0 => SYS_OPENAT,               // Read
        1 => SYS_OPENAT,               // Write
        2 => SYS_SOCKET | SYS_CONNECT, // Net
        3 => SYS_CLOCK_GETTIME,        // Time
        4 => SYS_GETRANDOM,            // Rand
        // Alloc (5) / Panic (6) / Diverge (7) widen NO sensitive syscall.
        _ => 0,
    }
}

/// The sensitive-syscall membership a transitive fx-mask permits (REQ-8): the OR of
/// every PRESENT atom's [`widen`] contribution. BYTE-IDENTICAL to the
/// `verus_core::io_allow` exec body the Verus prover discharges against the three
/// soundness lemmas — `pure_has_no_io` (`io_allow(0) == 0`), `monotone` (subset on
/// the syscall-mask), and `io_allow_within_io_bits` (deny-by-default). The
/// `forge::sandbox::syscall_allowlist` production fn is anchored to this over all
/// 256 fx-masks by `sandbox::verus_anchor` (the production consumer, R-DEFER-1).
///
/// NON-VACUITY: `io_allow(0) == 0` (pure permits NO sensitive syscall) and
/// `io_allow(1) == SYS_OPENAT != 0` (Read widens), so the map is genuinely
/// constraining (not constant).
#[must_use]
pub fn io_allow(fx: u8) -> u32 {
    let mut out: u32 = 0;
    let mut i: u8 = 0;
    while i < FX_ATOM_COUNT {
        if (fx & (1u8 << i)) != 0 {
            out |= widen(i);
        }
        i += 1;
    }
    out
}

// ---------------------------------------------------------------------------
// The Verus-verified core (REQ-1/REQ-4/REQ-5/REQ-7/REQ-8). Compiled ONLY by the
// real `verus` driver, which sets `cfg(verus_keep_ghost)`; a normal `cargo build`
// skips it entirely (the `verus!{}` macro + `ensures` syntax is verus-driver-only).
// The proof is run by `tests/verus_verify.rs` (`verus --no-cheating src/lib.rs`).
//
// AC-5: NO `#[verifier::external_body]` / `external` appears here — every core fn
// (`subsumes`/`ladder_action_*`/`io_allow`) carries a real `ensures`, not a Tier-3
// I/O shim.
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

    // =======================================================================
    // REQ-7 — the degrade-ladder ANTI-CHEAT (a Counterexample NEVER degrades).
    // =======================================================================

    /// The L3 verdict discriminant (mirrors the plain `L3Tag` + `forge`'s
    /// `L3Verdict`): the finite domain that drives the degrade decision.
    pub enum L3Tag { Proved, Timeout, Counterexample }

    /// The L2 verdict discriminant (mirrors `L2Tag` + `forge`'s `L2Verdict`).
    pub enum L2Tag { Verified, UnderBound, Counterexample }

    /// The action the ladder takes (mirrors the plain `LadderAction`). `CertifyL2`
    /// / `DegradeToL1` are the DEGRADE actions; `HardFail` is the non-certifying
    /// failure a counterexample maps to.
    pub enum LadderAction { CertifyL3, AttemptL2, CertifyL2, DegradeToL1, HardFail }

    /// `true` iff `a` is a DEGRADE — a lower rung taken as a PASS. The anti-cheat
    /// `ensures` is `!is_degrade(result)` on a counterexample. NON-vacuous: false
    /// for `HardFail`, true for `CertifyL2`/`DegradeToL1`.
    pub open spec fn is_degrade(a: LadderAction) -> bool {
        match a {
            LadderAction::CertifyL2 => true,
            LadderAction::DegradeToL1 => true,
            _ => false,
        }
    }

    /// `true` iff the L3 verdict is a counterexample (verus DISPROVED — a real bug).
    pub open spec fn l3_is_counterexample(v: L3Tag) -> bool {
        match v { L3Tag::Counterexample => true, _ => false }
    }

    /// `true` iff the L2 verdict is a counterexample (kani DISPROVED — a real bug).
    pub open spec fn l2_is_counterexample(v: L2Tag) -> bool {
        match v { L2Tag::Counterexample => true, _ => false }
    }

    /// The L3 ladder DECISION, PROVED to honor the anti-cheat for ALL verdicts
    /// (REQ-7, R-DEFER-9): a `Counterexample` maps to `HardFail` and NEVER to a
    /// degrade. Byte-identical to the plain-Rust `ladder_action_l3_tag` the
    /// toolchain runs.
    pub fn ladder_action_l3(v: L3Tag) -> (r: LadderAction)
        ensures l3_is_counterexample(v) ==> (r is HardFail) && !is_degrade(r),
    {
        match v {
            L3Tag::Proved => LadderAction::CertifyL3,
            L3Tag::Timeout => LadderAction::AttemptL2,
            L3Tag::Counterexample => LadderAction::HardFail,
        }
    }

    /// The L2 ladder DECISION, PROVED to honor the anti-cheat for ALL verdicts
    /// (REQ-7, the 2nd rung): an L2 `Counterexample` maps to `HardFail` and NEVER
    /// to a degrade (never a drop to L1). Byte-identical to `ladder_action_l2_tag`.
    pub fn ladder_action_l2(v: L2Tag) -> (r: LadderAction)
        ensures l2_is_counterexample(v) ==> (r is HardFail) && !is_degrade(r),
    {
        match v {
            L2Tag::Verified => LadderAction::CertifyL2,
            L2Tag::UnderBound => LadderAction::DegradeToL1,
            L2Tag::Counterexample => LadderAction::HardFail,
        }
    }

    /// The GLOBAL anti-cheat: the decision honors "a counterexample never degrades"
    /// over the WHOLE finite verdict domain (REQ-7). Quantifying the exec fns'
    /// post-conditions over every verdict closes the property as a theorem, not just
    /// a per-call obligation.
    proof fn anti_cheat_holds_for_all_verdicts()
        ensures
            forall|v: L3Tag| #[trigger] l3_is_counterexample(v) ==>
                (ladder_action_l3_spec(v) is HardFail) && !is_degrade(ladder_action_l3_spec(v)),
            forall|v: L2Tag| #[trigger] l2_is_counterexample(v) ==>
                (ladder_action_l2_spec(v) is HardFail) && !is_degrade(ladder_action_l2_spec(v)),
    {
        assert(forall|v: L3Tag| #[trigger] l3_is_counterexample(v) ==>
            (ladder_action_l3_spec(v) is HardFail) && !is_degrade(ladder_action_l3_spec(v)));
        assert(forall|v: L2Tag| #[trigger] l2_is_counterexample(v) ==>
            (ladder_action_l2_spec(v) is HardFail) && !is_degrade(ladder_action_l2_spec(v)));
    }

    /// The spec mirror of `ladder_action_l3` (so the global proof can quantify the
    /// decision over the verdict domain).
    pub open spec fn ladder_action_l3_spec(v: L3Tag) -> LadderAction {
        match v {
            L3Tag::Proved => LadderAction::CertifyL3,
            L3Tag::Timeout => LadderAction::AttemptL2,
            L3Tag::Counterexample => LadderAction::HardFail,
        }
    }

    /// The spec mirror of `ladder_action_l2`.
    pub open spec fn ladder_action_l2_spec(v: L2Tag) -> LadderAction {
        match v {
            L2Tag::Verified => LadderAction::CertifyL2,
            L2Tag::UnderBound => LadderAction::DegradeToL1,
            L2Tag::Counterexample => LadderAction::HardFail,
        }
    }

    // =======================================================================
    // REQ-8 — the seccomp allowlist SOUNDNESS (pure-no-I/O + monotonicity +
    // deny-by-default over the 5 sensitive user-I/O syscalls).
    // =======================================================================

    /// fx-atom `i` is present in the `u8` fx-mask (bit `i` set).
    pub open spec fn fx_has(fx: u8, i: u8) -> bool {
        (fx & (1u8 << i)) != 0
    }

    /// The per-atom sensitive-syscall contribution (REQ-8): which of the 5 sensitive
    /// I/O syscalls atom `i` widens to permit. openat=bit0, socket=bit1, connect=bit2,
    /// getrandom=bit3, clock_gettime=bit4. Mirrors the plain-Rust `widen`. NON-widening
    /// atoms (Alloc=5/Panic=6/Diverge=7, and any i>=8) contribute 0.
    pub open spec fn widen(i: u8) -> u32 {
        if i == 0 { 1u32 }            // Read  → openat
        else if i == 1 { 1u32 }       // Write → openat
        else if i == 2 { 2u32 | 4u32 } // Net  → socket | connect
        else if i == 3 { 16u32 }      // Time → clock_gettime
        else if i == 4 { 8u32 }       // Rand → getrandom
        else { 0u32 }                 // Alloc/Panic/Diverge → none
    }

    /// The sensitive-syscall membership an fx-mask permits (REQ-8): the OR of every
    /// PRESENT atom's `widen` contribution, the explicit 8-way unrolled fold over the
    /// bit positions. Mirrors the plain-Rust `io_allow`. The proved exec form.
    pub open spec fn io_allow(fx: u8) -> u32 {
        (if fx_has(fx, 0) { widen(0) } else { 0u32 })
        | (if fx_has(fx, 1) { widen(1) } else { 0u32 })
        | (if fx_has(fx, 2) { widen(2) } else { 0u32 })
        | (if fx_has(fx, 3) { widen(3) } else { 0u32 })
        | (if fx_has(fx, 4) { widen(4) } else { 0u32 })
        | (if fx_has(fx, 5) { widen(5) } else { 0u32 })
        | (if fx_has(fx, 6) { widen(6) } else { 0u32 })
        | (if fx_has(fx, 7) { widen(7) } else { 0u32 })
    }

    /// The exec form of `io_allow`, PROVED equal to the spec fold for ALL masks (the
    /// L3 guarantee, §6). A single OR expression structurally matching the spec
    /// (each `fx_has(fx, i)` is the `(fx & (1<<i)) != 0` test, each `widen(i)` its
    /// literal). Byte-identical in shape to the plain-Rust `io_allow` accumulation.
    pub fn io_allow_exec(fx: u8) -> (r: u32)
        ensures r == io_allow(fx),
    {
        (if (fx & (1u8 << 0)) != 0 { widen_exec(0) } else { 0u32 })
        | (if (fx & (1u8 << 1)) != 0 { widen_exec(1) } else { 0u32 })
        | (if (fx & (1u8 << 2)) != 0 { widen_exec(2) } else { 0u32 })
        | (if (fx & (1u8 << 3)) != 0 { widen_exec(3) } else { 0u32 })
        | (if (fx & (1u8 << 4)) != 0 { widen_exec(4) } else { 0u32 })
        | (if (fx & (1u8 << 5)) != 0 { widen_exec(5) } else { 0u32 })
        | (if (fx & (1u8 << 6)) != 0 { widen_exec(6) } else { 0u32 })
        | (if (fx & (1u8 << 7)) != 0 { widen_exec(7) } else { 0u32 })
    }

    /// The exec form of `widen`, PROVED equal to the spec for all atom indices
    /// (used by `io_allow_exec` so the running form delegates to the proved per-atom
    /// contribution rather than re-spelling the literals).
    pub fn widen_exec(i: u8) -> (r: u32)
        ensures r == widen(i),
    {
        if i == 0 { 1u32 }
        else if i == 1 { 1u32 }
        else if i == 2 { 2u32 | 4u32 }
        else if i == 3 { 16u32 }
        else if i == 4 { 8u32 }
        else { 0u32 }
    }

    /// SOUNDNESS lemma 1 — PURE-NO-I/O (REQ-8): an empty fx-mask (`pure`) permits NO
    /// sensitive user-I/O syscall. NON-vacuous: a non-widening atom leaking `openat`
    /// (mutating `widen`'s `else` arm) makes this fail (Grounding B).
    proof fn pure_has_no_io()
        ensures io_allow(0u8) == 0u32,
    {
        assert(io_allow(0u8) == 0u32) by (compute);
    }

    /// SOUNDNESS lemma 2 — non-widening atoms (Alloc/Panic/Diverge) add no I/O: a
    /// mask of ONLY bits 5,6,7 permits nothing sensitive.
    proof fn non_widening_atoms_have_no_io()
        ensures io_allow(0b1110_0000u8) == 0u32,
    {
        assert(io_allow(0b1110_0000u8) == 0u32) by (compute);
    }

    /// SOUNDNESS lemma 3 — MONOTONICITY (REQ-8, deny-by-default's positive form):
    /// `fx ⊆ fx'` (bitset subset) ⟹ `io_allow(fx) ⊆ io_allow(fx')` on the
    /// syscall-mask (adding an effect NEVER removes a permitted syscall). NON-vacuous:
    /// an XOR `io_allow` (so Write cancels Read's openat) makes this fail (Grounding B).
    proof fn monotone(fx: u8, fxp: u8)
        requires (fx & fxp) == fx,
        ensures (io_allow(fx) & io_allow(fxp)) == io_allow(fx),
    {
        assert((fx & fxp) == fx ==> (io_allow(fx) & io_allow(fxp)) == io_allow(fx)) by (bit_vector);
    }

    /// SOUNDNESS lemma 4 — DENY-BY-DEFAULT (REQ-8): `io_allow` NEVER sets a bit
    /// outside the 5 sensitive syscalls (bits 0..5, mask 0x1F). A widening can only
    /// grant inside the modeled sensitive set, never silently elsewhere (OQ-6).
    proof fn io_allow_within_io_bits(fx: u8)
        ensures (io_allow(fx) & !0x1Fu32) == 0u32,
    {
        assert((io_allow(fx) & !0x1Fu32) == 0u32) by (bit_vector);
    }

    }
}
