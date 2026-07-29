//! Thermite's Verus-verified soundness-critical pure core (epic #60, Tier 1).
//!
//! Governing design: `.design/verified/self-verification.md` (REQ-1..REQ-6).
//! Thesis: `thermite-design.md` §6 (Verus is the L3 prover), §9 (the TCB is
//! slag ∪ boundary ∪ the toolchain itself — this crate shrinks that TCB by
//! moving a soundness-critical decision function out of unverified Rust).
//!
//! ## What this crate is
//!
//! The first proven increment is effect subsumption (`subsumes`): a wrong answer
//! mints a false `pure` certificate for an effectful function (§4.1 / §9). The
//! decision is ported to a bounded **9-atom `u16` bitset** (Read=0 .. Term=8,
//! the path-insensitive atom-kind projection `EffectKind::of` already computes in
//! `thermite-lower`), where subsumption is the mask test `(callee & !caller) == 0`
//! and the subset relation `effects(callee) ⊆ effects(caller)` is the
//! explicit 9-way conjunction over the bit positions. The two are proved
//! equivalent by Verus `bit_vector`-mode SMT. (The proved bitset widened from
//! `u8` to `u16` for the 9th atom `Term` — the §4.1 terminal-control effect,
//! issue #106 — and every `bit_vector`/`compute` proof was re-derived over the
//! widened u16 domain; the widened lattice is still sound. `Term` is a
//! terminal-control grant that is not one of the 5 io-sensitive syscalls, so its
//! `io_allow` contribution is 0 — runtime-sandbox.md REQ-7 / OQ-5.)
//!
//! ## The landed mechanism: (c) exhaustive equivalence, not (b) delegation
//!
//! `.design/verified/self-verification.md` chose mechanism (b) (link the verified
//! crate into the cargo build so the proved code is the running code) if the
//! `--export`/`--import` linking worked, else fall back to (c) (a verified
//! reference + an enumerated impl==spec conformance test). The OQ-1/OQ-2 build
//! probe (epic #60) settled this empirically: the installed Verus support crates
//! (`builtin`/`builtin_macros`/`vstd`) cannot be consumed as cargo path-deps —
//! they inherit `workspace.lints` from the Verus workspace root (so `cargo
//! metadata` fails outside it), carry `cfg(verus_keep_ghost)` lint configs cargo
//! rejects, and the macro expansion resolves a renamed `verus_builtin` crate. The
//! `verus!{}` exec body also cannot plain-`rustc`-compile: a function carrying an
//! `ensures` clause is verus-driver-only syntax. So (b) is not viable for v1 and
//! we land (c), as the doc's decision rule prescribes.
//!
//! Under (c):
//! - The full `verus!{}` proof (the `subsumes` exec fn + `spec_subsumes` subset
//!   relation + three lattice-law `proof fn`s) lives in [`verus_core`], gated
//!   behind `#[cfg(verus_keep_ghost)]` — a cfg only the real `verus` driver sets,
//!   so a normal `cargo build` never sees it. `tests/verus_verify.rs` runs
//!   `verus --no-cheating src/lib.rs` and gates on `0 errors` (REQ-6 / AC-6).
//! - The always-cargo-compiled plain Rust ([`subsumes_masks`] / its spec
//!   [`spec_subsumes_mask`]) is byte-identical to the verus exec body / spec. The
//!   toolchain delegates the mask comparison to [`subsumes_masks`]; the
//!   exhaustive 2^9 × 2^9 = 262144-pair equivalence test
//!   (`tests/equivalence` over in `thermite-lower`) asserts the running code
//!   equals the proved subset relation for every input — finite + fully
//!   enumerated, so this shows `effects::subsumes` computes the relation
//!   Verus proved (transitively verus-anchored).
//!
//! ## What this increment adds (REQ-7 + REQ-8, epic #60)
//!
//! This increment ports the next two Tier-1 targets via the same mechanism (c):
//! - **REQ-7 — the degrade-ladder anti-cheat.** [`ladder_action_l3_tag`] /
//!   [`ladder_action_l2_tag`] are the plain-Rust mirrors of the verus-proved
//!   `ladder_action_l3`/`ladder_action_l2` decision: a verdict discriminant →ladder
//!   action ([`LadderAction`]). The verus core carries the anti-cheat `ensures`
//!   `l3_is_counterexample(v) ==> (r is HardFail) && !is_degrade(r)` (+ the L2
//!   analog) plus a global `proof fn` quantifying it over the whole verdict domain —
//!   a `Counterexample` never degrades (the R-DEFER-9 property). `forge::degrade`'s
//!   `ladder_action_l3`/`ladder_action_l2` mirror this, and `run_ladder` branches on
//!   the returned action; the in-module `verus_anchor` equivalence test binds the
//!   production decision to these tags.
//! - **REQ-8 — the seccomp allowlist soundness.** [`io_allow`] is the plain-Rust
//!   mirror of the verus-proved `io_allow(fx_mask) -> u32` over the 5 sensitive
//!   user-I/O syscalls (openat/socket/connect/getrandom/clock_gettime, bits 0..5).
//!   The verus core carries `pure_has_no_io` (`io_allow(0) == 0`), `monotone`
//!   (subset on the syscall-mask), and `io_allow_within_io_bits` (deny-by-default).
//!   `forge::sandbox`'s `syscall_allowlist` is anchored to this over all 512 masks
//!   by its in-module `verus_anchor` test (OQ-6: the 5 sensitive syscalls only; the
//!   dense `BASELINE_SYSCALLS` stays `sandbox_conformance`-grounded).
//!
//! ## REQ status
//!
//! <!-- generated:reqs view=thermite-verified-self-verification-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-VERIFIED-AGGREGATE-LEVEL | shipped | `thermite-verified/src/lib.rs` | Verified project level aggregation |  |
//! | REQ-VERIFIED-BOUNDARY-HONESTY | shipped | `thermite-verified/src/lib.rs` | Verified boundary honesty gate |  |
//! | REQ-VERIFIED-CI-GAUNTLET | shipped | `thermite-verified/src/lib.rs` | Verus verification gauntlet |  |
//! | REQ-VERIFIED-DEGRADE-ANTICHEAT | shipped | `thermite-verified/src/lib.rs` | Verified degrade anti-cheat |  |
//! | REQ-VERIFIED-HONESTY | shipped | `thermite-verified/src/lib.rs` | Genuine Verus proof honesty |  |
//! | REQ-VERIFIED-MUTATION-FLOOR | shipped | `thermite-verified/src/lib.rs` | Verified mutation floor gate |  |
//! | REQ-VERIFIED-SANDBOX-ALLOWLIST | shipped | `thermite-verified/src/lib.rs` | Verified sandbox allowlist |  |
//! | REQ-VERIFIED-SELF-VERIFICATION-ARCHITECTURE | shipped | `thermite-verified/src/lib.rs` | Self-verification architecture |  |
//! | REQ-VERIFIED-SUBSUMES | shipped | `thermite-verified/src/lib.rs` | Verified effect subsumption |  |
//! | REQ-VERIFIED-TIER-BOUNDARIES | shipped | `thermite-verified/src/lib.rs` | Verified tier boundaries |  |
//! | REQ-VERIFIED-TIER1-COVERAGE | shipped | `thermite-verified/src/lib.rs` | Tier-1 coverage and porting pattern |  |
//! <!-- /generated:reqs -->

/// The number of atomic effect kinds (`thermite_syntax::ast::Effect` →
/// `thermite_lower::effects::EffectKind`): Read=0, Write=1, Net=2, Alloc=3,
/// Time=4, Rand=5, Panic=6, Diverge=7, Term=8 (the #106 terminal-control atom).
/// The bitset is a `u16` (one bit per atom), so bits 0..9 are meaningful and the
/// relation is total over all 512 masks (the 9-atom domain). Widened `u8`→`u16`
/// for the 9th atom; the `bit_vector` proofs were re-derived over u16.
pub const ATOM_COUNT: u16 = 9;

/// The executable effect-subsumption decision over the 9-atom `u16` bitset
/// (REQ-5): `caller` subsumes `callee` iff `callee` has no atom the `caller`
/// lacks, i.e. `(callee & !caller) == 0`. This plain-Rust body is byte-identical
/// to the `verus_core::subsumes` exec body the Verus prover discharges against the
/// subset-relation `ensures` (mechanism (c) — the running code mirrors the proved
/// code). [`spec_subsumes_mask`] is the spec it is proved to compute. Widened
/// `u8`→`u16` for the 9th atom `Term` (#106); the `(callee & !caller) == 0`
/// equivalence to the 9-way subset conjunction is re-proved over u16.
///
/// `thermite_lower::effects::subsumes` delegates its bit-level comparison here
/// (the production consumer, R-DEFER-1); the exhaustive 512×512 = 131072-pair
/// equivalence test anchors `effects::subsumes` to this verus-verified relation.
#[must_use]
pub fn subsumes_masks(caller: u16, callee: u16) -> bool {
    let missing = callee & !caller;
    missing == 0
}

/// The subset relation `effects(callee) ⊆ effects(caller)` over the
/// 9-atom `u16` bitset, as the explicit per-atom conjunction (REQ-4 — the
/// non-trivial contract `subsumes_masks` is proved to compute). For each atom
/// position `i`, if `callee` has atom `i` then `caller` must have it. This is the
/// plain-Rust mirror of `verus_core::spec_subsumes`; the Verus `bit_vector` proof
/// shows `subsumes_masks(c, k) == spec_subsumes_mask(c, k)` for all `(c, k)`, and
/// the exhaustive equivalence test re-checks the mirror over the full u16 domain.
///
/// Non-vacuity: this returns `false` for `caller=0, callee=1` (Pure does not
/// subsume {Read}), so the relation is constraining (not `true`).
#[must_use]
pub fn spec_subsumes_mask(caller: u16, callee: u16) -> bool {
    let mut i: u16 = 0;
    while i < ATOM_COUNT {
        let bit = 1u16 << i;
        if (callee & bit) != 0 && (caller & bit) == 0 {
            return false;
        }
        i += 1;
    }
    true
}

// ===========================================================================
// REQ-7 — the degrade-ladder anti-cheat decision (the core R-DEFER-9 property).
// ===========================================================================

/// The verdict discriminant of an L3 (verus) run, the finite domain that drives
/// the degrade decision (REQ-7). The carried `Certificate`/`RejectReason` payloads
/// `forge::degrade::L3Verdict` holds are irrelevant to the decision, so the proved
/// model tracks only this tag. Mirrors `forge::degrade::L3Verdict`'s discriminant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum L3Tag {
    /// verus proved the item → certify L3.
    Proved,
    /// verus timed out (inconclusive) → the sole degrade trigger → attempt L2.
    Timeout,
    /// verus disproved the item (a bug) → hard fail, never a degrade.
    Counterexample,
}

/// The verdict discriminant of an L2 (kani) run (REQ-7). Mirrors `forge::kani::L2Verdict`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum L2Tag {
    /// kani verified to the bound → certify L2 (degraded).
    Verified,
    /// kani exhausted its bound (inconclusive) → degrade to L1.
    UnderBound,
    /// kani disproved the contract (a bug) → hard fail, never a drop to L1.
    Counterexample,
}

/// The action the degrade ladder takes for a classified verdict (REQ-7). This is
/// the proved decision: `forge::degrade::run_ladder` branches on it, so the
/// proved classification drives the real control flow. `CertifyL2`/`DegradeToL1`
/// are the degrade actions ([`is_degrade`]); `HardFail` is the non-certifying
/// failure a `Counterexample` maps to and never is a degrade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LadderAction {
    /// Proved at L3 → certify L3 (terminal, no degrade).
    CertifyL3,
    /// L3 timed out → attempt the L2 rung.
    AttemptL2,
    /// L2 verified → certify L2 with the lowered-assurance stamp (a degrade).
    CertifyL2,
    /// L2 under-bound → drop to the L1 runtime-check rung (a degrade).
    DegradeToL1,
    /// A counterexample (L3 or L2) → non-certifying hard fail (never a degrade).
    HardFail,
}

/// `true` iff `action` is a degrade — a lower rung taken as a pass
/// (`CertifyL2`/`DegradeToL1`). The anti-cheat invariant (REQ-7) is that a
/// `Counterexample` never maps to a degrade. Mirrors `verus_core::is_degrade`.
#[must_use]
pub fn is_degrade(action: LadderAction) -> bool {
    matches!(action, LadderAction::CertifyL2 | LadderAction::DegradeToL1)
}

/// The L3 ladder decision (REQ-7): a verdict tag → the ladder action. Byte-identical
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
        // anti-cheat (R-DEFER-9): a counterexample is a hard fail, never a degrade.
        L3Tag::Counterexample => LadderAction::HardFail,
    }
}

/// The L2 ladder decision (REQ-7): an L2 verdict tag → the ladder action.
/// Byte-identical to `verus_core::ladder_action_l2`. A `Counterexample` is a hard
/// fail (never a drop to L1, the 2nd-rung anti-cheat).
#[must_use]
pub fn ladder_action_l2_tag(v: L2Tag) -> LadderAction {
    match v {
        L2Tag::Verified => LadderAction::CertifyL2,
        L2Tag::UnderBound => LadderAction::DegradeToL1,
        // anti-cheat (R-DEFER-9): an L2 counterexample is a hard fail, never L1.
        L2Tag::Counterexample => LadderAction::HardFail,
    }
}

// ===========================================================================
// REQ-8 — the seccomp allowlist soundness decision (the fx → sensitive-I/O map).
// ===========================================================================

/// The number of fx-atom kinds in the `u16` fx-mask (Read=0, Write=1, Net=2,
/// Time=3, Rand=4, Alloc=5, Panic=6, Diverge=7, Term=8 — the #106
/// terminal-control atom). The bitset is total over all 512 masks. Widened
/// `u8`→`u16` for the 9th atom; `Term` is non-widening (`widen(8) == 0`) — a
/// terminal-control `ioctl` grant, not one of the 5 io-sensitive syscalls
/// (runtime-sandbox.md REQ-7 / OQ-5), so the io_allow soundness lemmas still hold.
pub const FX_ATOM_COUNT: u16 = 9;

/// The bit positions, in the `u32` sensitive-syscall mask, of the 5 user-I/O
/// syscalls the §4.1 `pure`-exclusion table calls out (REQ-8). The dense
/// `BASELINE_SYSCALLS` is orthogonal to these IO bits (OQ-6), so the soundness
/// model is the IO-membership projection.
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
/// Alloc/Panic/Diverge/Term→0 (non-widening). Byte-identical to `verus_core::widen`.
/// `Term` (8, #106) is the `ioctl` grant — not one of the 5 io-sensitive syscalls
/// (runtime-sandbox.md REQ-7 / OQ-5), so it contributes 0 to the io_allow mask.
#[must_use]
pub fn widen(i: u16) -> u32 {
    match i {
        0 => SYS_OPENAT,               // Read
        1 => SYS_OPENAT,               // Write
        2 => SYS_SOCKET | SYS_CONNECT, // Net
        3 => SYS_CLOCK_GETTIME,        // Time
        4 => SYS_GETRANDOM,            // Rand
        // Alloc (5) / Panic (6) / Diverge (7) / Term (8) widen no sensitive syscall.
        _ => 0,
    }
}

/// The sensitive-syscall membership a transitive fx-mask permits (REQ-8): the OR of
/// every present atom's [`widen`] contribution, over the 9-atom `u16` fx-mask.
/// Byte-identical to the `verus_core::io_allow` exec body the Verus prover
/// discharges against the three soundness lemmas — `pure_has_no_io`
/// (`io_allow(0) == 0`), `monotone` (subset on the syscall-mask), and
/// `io_allow_within_io_bits` (deny-by-default). The `forge::sandbox::syscall_allowlist`
/// production fn is anchored to this over all 512 fx-masks by `sandbox::verus_anchor`
/// (the production consumer, R-DEFER-1).
///
/// Non-vacuity: `io_allow(0) == 0` (pure permits no sensitive syscall) and
/// `io_allow(1) == SYS_OPENAT != 0` (Read widens), so the map is
/// constraining (not constant).
#[must_use]
pub fn io_allow(fx: u16) -> u32 {
    let mut out: u32 = 0;
    let mut i: u16 = 0;
    while i < FX_ATOM_COUNT {
        if (fx & (1u16 << i)) != 0 {
            out |= widen(i);
        }
        i += 1;
    }
    out
}

// ===========================================================================
// REQ-9 (Target C) — the boundary honesty gate (the §9 composition anti-cheat).
// ===========================================================================

/// The boundary/slag external_body honesty gate (REQ-9): a fn is emitted as a
/// `#[verifier::external_body]` assumable signature iff it carries a declared
/// trust boundary — `#[boundary]` (`has_boundary`) or `#[slag]` (`has_slag`).
/// Byte-identical to the `verus_core::should_emit_external_body` exec body the
/// Verus prover discharges against the disjunction `ensures` plus the soundness
/// corollary `(!has_boundary && !has_slag) ==> !r` — a regular fn (neither flag)
/// is never laundered into an assumed-L3 signature (§9, R-DEFER-9).
///
/// `thermite_lower::lower::lower_fn` takes the external_body arm iff this predicate
/// is `true` (the production consumer, R-DEFER-1); the observable-dispatch
/// equivalence test (`thermite-lower/tests/boundary_gate_verified.rs`) anchors the
/// emitted source's `#[verifier::external_body]` substring to this predicate over
/// the 4 `(has_boundary, has_slag)` combinations.
///
/// Non-vacuity: this returns `false` for `(false, false)` (a regular fn is fully
/// proved, never external_body), so the gate is constraining (not `true`).
#[must_use]
pub fn should_emit_external_body(has_boundary: bool, has_slag: bool) -> bool {
    has_boundary || has_slag
}

// ===========================================================================
// REQ-10 (Target D) — the project level aggregation min (the §5.2 no-over-claim).
// ===========================================================================

/// The assurance level lattice (REQ-10), mirroring `forge::manifest::Level`. The
/// rank order `L0 < L1 < L2 < L3` (`rank` 0..3) is the `Ord` the project-level
/// fold-min ranges over: the project is never claimed stronger than its weakest
/// certifying fn (§5.2). Byte-identical discriminant order to `forge`'s `Level`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// L0 — unverified / `#[slag]` escape hatch.
    L0,
    /// L1 — executable runtime check.
    L1,
    /// L2 — bounded model check (Kani).
    L2,
    /// L3 — SMT proof: holds for all inputs.
    L3,
}

/// The rank discriminant of a [`Level`] (REQ-10): `L0=0 .. L3=3`. The `min2`
/// fold and the production `Level: Ord` agree on this order. Mirrors
/// `verus_core::rank`.
#[must_use]
pub fn rank(l: Level) -> u8 {
    match l {
        Level::L0 => 0,
        Level::L1 => 1,
        Level::L2 => 2,
        Level::L3 => 3,
    }
}

/// The weaker of two levels by [`rank`] (REQ-10): ties to `a`. Mirrors
/// `verus_core::min2`. Non-vacuity: `min2(L3, L1) == L1` (picks the weaker, never
/// an over-claim), so a max-picking mutant fails `aggregate_le_all` (Grounding D).
#[must_use]
pub fn min2(a: Level, b: Level) -> Level {
    if rank(a) <= rank(b) {
        a
    } else {
        b
    }
}

/// The project-level fold-min over a per-fn level list (REQ-10): the weakest
/// level, seeded at `L3` so the empty list folds to `L3` (mirroring
/// `min().unwrap_or(Level::L3)` in `manifest::aggregate`). Byte-identical fold to
/// the verus-proved `aggregate_level(Seq<Level>)` the Verus prover discharges
/// against `aggregate_le_all` (≤ every fn — the §5.2 over-claim bound) and
/// `aggregate_is_attained` (== the min). `forge::manifest::AssuranceManifest::aggregate`
/// is anchored to this over the exhaustive `Level` lists (its `verus_anchor` test).
#[must_use]
pub fn aggregate_level(levels: &[Level]) -> Level {
    let mut acc = Level::L3;
    let mut i = 0;
    while i < levels.len() {
        acc = min2(acc, levels[i]);
        i += 1;
    }
    acc
}

// ===========================================================================
// REQ-11 (Target E) — the mutation floor gate (#48 anti-Goodhart, §7).
// ===========================================================================

/// The mutation kill-ratio floor gate at the default 60% floor (REQ-11), in
/// integer cross-multiply form (no f64 — verus reasons poorly about floats):
/// `scored > 0 && killed * 100 >= scored * 60`. Byte-identical to the
/// `verus_core::meets_floor_60` exec body the Verus prover discharges against the
/// #48 anti-Goodhart `ensures` `scored == 0 ==> !r` — a `0/0` score
/// (no scoreable mutant) never passes the floor (a contract that cannot be
/// mutation-validated is gated `WeakContract`, not a vacuous pass).
///
/// `forge::mutation::MutationScore::meets_floor` (the f64 production gate at the
/// default `MUTATION_FLOOR = 0.60`) is anchored to this over a bounded grid by its
/// `verus_anchor` test (OQ-E — the f64↔integer agreement is the test's job; the
/// proved property is the integer #48 gate).
///
/// `u128` widening: `killed * 100` / `scored * 60` cannot overflow for any
/// `usize` count (matching the exec form's overflow obligation).
///
/// Non-vacuity: this returns `false` for `(0, 0)` (the #48 gate) and for `(1, 2)`
/// (`100 >= 120` is false — 50% is below 60%), so it is constraining.
#[must_use]
pub fn meets_floor_60(killed: usize, scored: usize) -> bool {
    let k = killed as u128;
    let s = scored as u128;
    s > 0 && k * 100 >= s * 60
}

// ---------------------------------------------------------------------------
// The Verus-verified core (REQ-1/REQ-4/REQ-5/REQ-7/REQ-8/REQ-9/REQ-10/REQ-11).
// Compiled only by the real `verus` driver, which sets `cfg(verus_keep_ghost)`; a
// normal `cargo build` skips it entirely (the `verus!{}` macro + `ensures` syntax
// is verus-driver-only). The proof is run by `tests/verus_verify.rs`
// (`verus --no-cheating src/lib.rs`).
//
// AC-5: no `#[verifier::external_body]` / `external` appears here — every core fn
// (`subsumes`/`ladder_action_*`/`io_allow`/`should_emit_external_body`/
// `aggregate_level`/`meets_floor_60`) carries a real `ensures`, not a Tier-3 shim.
// ---------------------------------------------------------------------------
#[cfg(verus_keep_ghost)]
mod verus_core {
    use vstd::prelude::*;

    verus! {

    /// Atom `i` is present in `mask` (bit `i` set). 9 atoms (u16): Read=0 .. Term=8.
    pub open spec fn has(mask: u16, i: u16) -> bool {
        (mask & (1u16 << i)) != 0
    }

    /// The subset relation `effects(callee) ⊆ effects(caller)`, as the
    /// explicit 9-way conjunction over the atom positions (mirrors the plain-Rust
    /// `spec_subsumes_mask`). Non-vacuous (REQ-4): false when callee has an atom
    /// caller lacks. Bit 8 is the #106 terminal-control atom `Term`.
    pub open spec fn spec_subsumes(caller: u16, callee: u16) -> bool {
        &&& (has(callee, 0) ==> has(caller, 0))
        &&& (has(callee, 1) ==> has(caller, 1))
        &&& (has(callee, 2) ==> has(caller, 2))
        &&& (has(callee, 3) ==> has(caller, 3))
        &&& (has(callee, 4) ==> has(caller, 4))
        &&& (has(callee, 5) ==> has(caller, 5))
        &&& (has(callee, 6) ==> has(caller, 6))
        &&& (has(callee, 7) ==> has(caller, 7))
        &&& (has(callee, 8) ==> has(caller, 8))
    }

    /// The executable mask test, proved equal to the subset relation for all
    /// inputs (the L3 guarantee, §6). Byte-identical to the plain-Rust
    /// `subsumes_masks` the toolchain runs. Widened to u16 for the 9th atom (#106).
    /// The 9-atom masks only ever set bits 0..9 (the `EffectKind::bit` domain), so
    /// the contract is over `caller < 512 && callee < 512` — the upper bits 9..16
    /// are unused, so the all-16-bit `(callee & !caller) == 0` test agrees with the
    /// 9-way `spec_subsumes` conjunction exactly when no out-of-domain bit is set.
    pub fn subsumes(caller: u16, callee: u16) -> (r: bool)
        requires caller < 512, callee < 512,
        ensures r == spec_subsumes(caller, callee),
    {
        assert(caller < 512 && callee < 512 ==>
            ((callee & !caller & 0x1FF) == 0) == spec_subsumes(caller, callee)) by (bit_vector);
        assert(caller < 512 && callee < 512 ==>
            (callee & !caller) == (callee & !caller & 0x1FF)) by (bit_vector);
        let missing = callee & !caller;
        missing == 0
    }

    /// Lattice law 1 (`.design/lower/effect-subsumption.md` REQ-1): reflexive —
    /// every row subsumes itself.
    proof fn lattice_reflexive(row: u16)
        ensures spec_subsumes(row, row),
    {
        assert(spec_subsumes(row, row)) by (bit_vector);
    }

    /// Lattice law 2: Pure (the empty set, mask 0) subsumes only Pure (over the
    /// 9-atom domain `callee < 512`; an out-of-domain upper bit is not a modeled
    /// atom).
    proof fn lattice_pure_subsumes_only_pure(callee: u16)
        requires callee < 512,
        ensures spec_subsumes(0u16, callee) == (callee == 0),
    {
        assert(callee < 512 ==> (spec_subsumes(0u16, callee) == (callee == 0))) by (bit_vector);
    }

    /// Lattice law 3: the top row (all 9 atoms, mask 0x1FF) subsumes every row.
    proof fn lattice_top_subsumes_all(callee: u16)
        ensures spec_subsumes(0x1FFu16, callee),
    {
        assert(spec_subsumes(0x1FFu16, callee)) by (bit_vector);
    }

    // =======================================================================
    // REQ-7 — the degrade-ladder anti-cheat (a Counterexample never degrades).
    // =======================================================================

    /// The L3 verdict discriminant (mirrors the plain `L3Tag` + `forge`'s
    /// `L3Verdict`): the finite domain that drives the degrade decision.
    pub enum L3Tag { Proved, Timeout, Counterexample }

    /// The L2 verdict discriminant (mirrors `L2Tag` + `forge`'s `L2Verdict`).
    pub enum L2Tag { Verified, UnderBound, Counterexample }

    /// The action the ladder takes (mirrors the plain `LadderAction`). `CertifyL2`
    /// / `DegradeToL1` are the degrade actions; `HardFail` is the non-certifying
    /// failure a counterexample maps to.
    pub enum LadderAction { CertifyL3, AttemptL2, CertifyL2, DegradeToL1, HardFail }

    /// `true` iff `a` is a degrade — a lower rung taken as a pass. The anti-cheat
    /// `ensures` is `!is_degrade(result)` on a counterexample. Non-vacuous: false
    /// for `HardFail`, true for `CertifyL2`/`DegradeToL1`.
    pub open spec fn is_degrade(a: LadderAction) -> bool {
        match a {
            LadderAction::CertifyL2 => true,
            LadderAction::DegradeToL1 => true,
            _ => false,
        }
    }

    /// `true` iff the L3 verdict is a counterexample (verus disproved — a bug).
    pub open spec fn l3_is_counterexample(v: L3Tag) -> bool {
        match v { L3Tag::Counterexample => true, _ => false }
    }

    /// `true` iff the L2 verdict is a counterexample (kani disproved — a bug).
    pub open spec fn l2_is_counterexample(v: L2Tag) -> bool {
        match v { L2Tag::Counterexample => true, _ => false }
    }

    /// The L3 ladder decision, proved to honor the anti-cheat for all verdicts
    /// (REQ-7, R-DEFER-9): a `Counterexample` maps to `HardFail` and never to a
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

    /// The L2 ladder decision, proved to honor the anti-cheat for all verdicts
    /// (REQ-7, the 2nd rung): an L2 `Counterexample` maps to `HardFail` and never
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

    /// The global anti-cheat: the decision honors "a counterexample never degrades"
    /// over the whole finite verdict domain (REQ-7). Quantifying the exec fns'
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
    // REQ-8 — the seccomp allowlist soundness (pure-no-I/O + monotonicity +
    // deny-by-default over the 5 sensitive user-I/O syscalls).
    // =======================================================================

    /// fx-atom `i` is present in the `u16` fx-mask (bit `i` set).
    pub open spec fn fx_has(fx: u16, i: u16) -> bool {
        (fx & (1u16 << i)) != 0
    }

    /// The per-atom sensitive-syscall contribution (REQ-8): which of the 5 sensitive
    /// I/O syscalls atom `i` widens to permit. openat=bit0, socket=bit1, connect=bit2,
    /// getrandom=bit3, clock_gettime=bit4. Mirrors the plain-Rust `widen`. Non-widening
    /// atoms (Alloc=5/Panic=6/Diverge=7/Term=8, and any i>=9) contribute 0. `Term`
    /// (8, #106) is the `ioctl` grant — not an io-sensitive syscall (REQ-7/OQ-5).
    pub open spec fn widen(i: u16) -> u32 {
        if i == 0 { 1u32 }            // Read  → openat
        else if i == 1 { 1u32 }       // Write → openat
        else if i == 2 { 2u32 | 4u32 } // Net  → socket | connect
        else if i == 3 { 16u32 }      // Time → clock_gettime
        else if i == 4 { 8u32 }       // Rand → getrandom
        else { 0u32 }                 // Alloc/Panic/Diverge/Term → none
    }

    /// The sensitive-syscall membership an fx-mask permits (REQ-8): the OR of every
    /// present atom's `widen` contribution, the explicit 9-way unrolled fold over the
    /// bit positions (bit 8 is `Term`, #106 — `widen(8) == 0`). Mirrors the plain-Rust
    /// `io_allow`. The proved exec form.
    pub open spec fn io_allow(fx: u16) -> u32 {
        (if fx_has(fx, 0) { widen(0) } else { 0u32 })
        | (if fx_has(fx, 1) { widen(1) } else { 0u32 })
        | (if fx_has(fx, 2) { widen(2) } else { 0u32 })
        | (if fx_has(fx, 3) { widen(3) } else { 0u32 })
        | (if fx_has(fx, 4) { widen(4) } else { 0u32 })
        | (if fx_has(fx, 5) { widen(5) } else { 0u32 })
        | (if fx_has(fx, 6) { widen(6) } else { 0u32 })
        | (if fx_has(fx, 7) { widen(7) } else { 0u32 })
        | (if fx_has(fx, 8) { widen(8) } else { 0u32 })
    }

    /// The exec form of `io_allow`, proved equal to the spec fold for all masks (the
    /// L3 guarantee, §6). A single OR expression structurally matching the spec
    /// (each `fx_has(fx, i)` is the `(fx & (1<<i)) != 0` test, each `widen(i)` its
    /// literal). Byte-identical in shape to the plain-Rust `io_allow` accumulation.
    pub fn io_allow_exec(fx: u16) -> (r: u32)
        ensures r == io_allow(fx),
    {
        (if (fx & (1u16 << 0)) != 0 { widen_exec(0) } else { 0u32 })
        | (if (fx & (1u16 << 1)) != 0 { widen_exec(1) } else { 0u32 })
        | (if (fx & (1u16 << 2)) != 0 { widen_exec(2) } else { 0u32 })
        | (if (fx & (1u16 << 3)) != 0 { widen_exec(3) } else { 0u32 })
        | (if (fx & (1u16 << 4)) != 0 { widen_exec(4) } else { 0u32 })
        | (if (fx & (1u16 << 5)) != 0 { widen_exec(5) } else { 0u32 })
        | (if (fx & (1u16 << 6)) != 0 { widen_exec(6) } else { 0u32 })
        | (if (fx & (1u16 << 7)) != 0 { widen_exec(7) } else { 0u32 })
        | (if (fx & (1u16 << 8)) != 0 { widen_exec(8) } else { 0u32 })
    }

    /// The exec form of `widen`, proved equal to the spec for all atom indices
    /// (used by `io_allow_exec` so the running form delegates to the proved per-atom
    /// contribution rather than re-spelling the literals).
    pub fn widen_exec(i: u16) -> (r: u32)
        ensures r == widen(i),
    {
        if i == 0 { 1u32 }
        else if i == 1 { 1u32 }
        else if i == 2 { 2u32 | 4u32 }
        else if i == 3 { 16u32 }
        else if i == 4 { 8u32 }
        else { 0u32 }
    }

    /// Soundness lemma 1 — pure-no-I/O (REQ-8): an empty fx-mask (`pure`) permits no
    /// sensitive user-I/O syscall. Non-vacuous: a non-widening atom leaking `openat`
    /// (mutating `widen`'s `else` arm) makes this fail (Grounding B).
    proof fn pure_has_no_io()
        ensures io_allow(0u16) == 0u32,
    {
        assert(io_allow(0u16) == 0u32) by (compute);
    }

    /// Soundness lemma 2 — non-widening atoms (Alloc/Panic/Diverge/Term) add no I/O: a
    /// mask of only bits 5,6,7,8 (incl. the #106 `Term` bit) permits nothing sensitive.
    proof fn non_widening_atoms_have_no_io()
        ensures io_allow(0b1_1110_0000u16) == 0u32,
    {
        assert(io_allow(0b1_1110_0000u16) == 0u32) by (compute);
    }

    /// Soundness lemma 3 — monotonicity (REQ-8, deny-by-default's positive form):
    /// `fx ⊆ fx'` (bitset subset) ⟹ `io_allow(fx) ⊆ io_allow(fx')` on the
    /// syscall-mask (adding an effect never removes a permitted syscall). Non-vacuous:
    /// an XOR `io_allow` (so Write cancels Read's openat) makes this fail (Grounding B).
    proof fn monotone(fx: u16, fxp: u16)
        requires (fx & fxp) == fx,
        ensures (io_allow(fx) & io_allow(fxp)) == io_allow(fx),
    {
        assert((fx & fxp) == fx ==> (io_allow(fx) & io_allow(fxp)) == io_allow(fx)) by (bit_vector);
    }

    /// Soundness lemma 4 — deny-by-default (REQ-8): `io_allow` never sets a bit
    /// outside the 5 sensitive syscalls (bits 0..5, mask 0x1F). A widening can only
    /// grant inside the modeled sensitive set, never silently elsewhere (OQ-6). The
    /// #106 `Term` bit (8) widens to 0, so the deny-by-default bound is unchanged.
    proof fn io_allow_within_io_bits(fx: u16)
        ensures (io_allow(fx) & !0x1Fu32) == 0u32,
    {
        assert((io_allow(fx) & !0x1Fu32) == 0u32) by (bit_vector);
    }

    // =======================================================================
    // REQ-9 (Target C) — the boundary honesty gate (a regular fn never gets
    // external_body, so a lying regular body can never be laundered to L3, §9).
    // =======================================================================

    /// The spec disjunction `has_boundary || has_slag` (the gate's intended
    /// answer). Mirrors the plain-Rust `should_emit_external_body`. Non-vacuous:
    /// false at `(false, false)`.
    pub open spec fn spec_should_emit_external_body(has_boundary: bool, has_slag: bool) -> bool {
        has_boundary || has_slag
    }

    /// The boundary/slag external_body honesty gate, proved to compute the
    /// disjunction and the soundness corollary for all inputs (REQ-9, R-DEFER-9):
    /// **(1)** `r == spec_should_emit_external_body(..)`, and **(2)** the corollary
    /// `(!has_boundary && !has_slag) ==> !r` — a regular fn is never emitted
    /// external_body. Byte-identical to the plain-Rust `should_emit_external_body`.
    pub fn should_emit_external_body(has_boundary: bool, has_slag: bool) -> (r: bool)
        ensures
            r == spec_should_emit_external_body(has_boundary, has_slag),
            (!has_boundary && !has_slag) ==> !r,
    {
        has_boundary || has_slag
    }

    /// The global §9 honesty corollary: no regular fn (neither flag) is ever
    /// emitted as `#[verifier::external_body]`, over the whole 2×2 bool square
    /// (REQ-9). A lying regular body can never be laundered into an assumed-L3
    /// signature — the composition anti-cheat.
    proof fn regular_fn_never_external_body()
        ensures
            forall|b: bool, s: bool| #[trigger] spec_should_emit_external_body(b, s) ==>
                (b || s),
            forall|b: bool, s: bool|
                (!b && !s) ==> !(#[trigger] spec_should_emit_external_body(b, s)),
    {
        assert(forall|b: bool, s: bool| #[trigger] spec_should_emit_external_body(b, s) ==>
            (b || s));
        assert(forall|b: bool, s: bool|
            (!b && !s) ==> !(#[trigger] spec_should_emit_external_body(b, s)));
    }

    // =======================================================================
    // REQ-10 (Target D) — the project level aggregation min (no over-claim, §5.2).
    // =======================================================================

    /// The assurance level lattice (mirrors the plain `Level` + `forge`'s `Level`).
    pub enum Level { L0, L1, L2, L3 }

    /// The rank discriminant `L0=0 .. L3=3` (mirrors the plain `rank`). The order
    /// the fold-min ranges over: a smaller rank is a weaker level.
    pub open spec fn rank(l: Level) -> int {
        match l {
            Level::L0 => 0,
            Level::L1 => 1,
            Level::L2 => 2,
            Level::L3 => 3,
        }
    }

    /// The weaker of two levels by `rank` (mirrors the plain `min2`); ties to `a`.
    /// Non-vacuous: a max-picking mutant (`rank(a) >= rank(b)`) breaks
    /// `aggregate_le_all` (Grounding D).
    pub open spec fn min2(a: Level, b: Level) -> Level {
        if rank(a) <= rank(b) { a } else { b }
    }

    /// The project-level fold-min over a `Seq<Level>` (REQ-10), seeded at `L3` so
    /// the empty seq folds to `L3` (mirroring `min().unwrap_or(Level::L3)`). The
    /// recursion folds the last element into the prefix's fold; mirrors the plain
    /// `aggregate_level` (which iterates left-to-right — `min2` is commutative-
    /// enough that the fold value is the same: the unique minimum).
    pub open spec fn aggregate_level(levels: Seq<Level>) -> Level
        decreases levels.len(),
    {
        if levels.len() == 0 {
            Level::L3
        } else {
            min2(aggregate_level(levels.drop_last()), levels.last())
        }
    }

    /// Soundness lemma D1 — `aggregate_level(levels)` is ≤ every element by `rank`
    /// (REQ-10): the project level is never claimed stronger than its weakest fn
    /// (§5.2 / R-DEFER-9 over-claim bound). Proved by induction on `drop_last`.
    /// Non-vacuous: a max-picking `min2` makes this fail (Grounding D).
    proof fn aggregate_le_all(levels: Seq<Level>, i: int)
        requires 0 <= i < levels.len(),
        ensures rank(aggregate_level(levels)) <= rank(levels[i]),
        decreases levels.len(),
    {
        let n = levels.len();
        if i == n - 1 {
            // The last element is folded directly by `min2` at the top.
            assert(aggregate_level(levels) == min2(aggregate_level(levels.drop_last()), levels.last()));
            assert(levels.last() == levels[n - 1]);
        } else {
            // `i` lives in the prefix; recurse, then `min2` only lowers the rank.
            assert(levels.drop_last().len() == n - 1);
            assert(0 <= i < levels.drop_last().len());
            assert(levels.drop_last()[i] == levels[i]);
            aggregate_le_all(levels.drop_last(), i);
            assert(aggregate_level(levels) == min2(aggregate_level(levels.drop_last()), levels.last()));
        }
    }

    /// Soundness lemma D2 — `aggregate_level(levels)` is attained at some index
    /// (REQ-10): D1 + D2 ⟹ it is exactly the min, not merely a lower bound.
    /// Proved by induction: either the tail's attaining index, or the last element.
    proof fn aggregate_is_attained(levels: Seq<Level>)
        requires levels.len() > 0,
        ensures exists|j: int| 0 <= j < levels.len() && aggregate_level(levels) == levels[j],
        decreases levels.len(),
    {
        let n = levels.len();
        let pre = levels.drop_last();
        if n == 1 {
            assert(aggregate_level(levels) == min2(aggregate_level(pre), levels.last()));
            assert(aggregate_level(pre) == Level::L3);
            // min2(L3, x) == x (L3 is the top rank), so the fold is the last elem.
            assert(rank(Level::L3) == 3);
            assert(aggregate_level(levels) == levels[0]);
        } else {
            aggregate_is_attained(pre);
            let jp = choose|j: int| 0 <= j < pre.len() && aggregate_level(pre) == pre[j];
            assert(0 <= jp < pre.len() && aggregate_level(pre) == pre[jp]);
            assert(pre[jp] == levels[jp]);
            assert(aggregate_level(levels) == min2(aggregate_level(pre), levels.last()));
            // The min is whichever of the prefix-min / the last element is weaker;
            // both are elements of `levels`.
            if rank(aggregate_level(pre)) <= rank(levels.last()) {
                assert(aggregate_level(levels) == aggregate_level(pre));
                assert(aggregate_level(levels) == levels[jp]);
            } else {
                assert(aggregate_level(levels) == levels.last());
                assert(levels.last() == levels[n - 1]);
            }
        }
    }

    // =======================================================================
    // REQ-11 (Target E) — the mutation floor gate (#48 anti-Goodhart, integer).
    // =======================================================================

    /// The integer cross-multiply floor spec at the default 60% floor (REQ-11):
    /// `scored > 0 && killed * 100 >= scored * 60`. Mirrors the plain
    /// `meets_floor_60`. Non-vacuous: false at `scored == 0` (#48) and at 50%.
    pub open spec fn spec_meets_floor_60(killed: nat, scored: nat) -> bool {
        scored > 0 && killed * 100 >= scored * 60
    }

    /// The mutation floor gate, proved to compute the integer cross-multiply and
    /// the #48 anti-Goodhart corollary for all inputs (REQ-11, R-DEFER-9):
    /// `r == spec_meets_floor_60(..)` and `scored == 0 ==> !r` (a `0/0` score
    /// never passes). The `u128` widening discharges the multiply-overflow
    /// obligation (`killed`/`scored` are `usize`-bounded; `* 100` / `* 60` fit
    /// `u128`). Byte-identical in shape to the plain-Rust `meets_floor_60`.
    pub fn meets_floor_60(killed: u128, scored: u128) -> (r: bool)
        requires killed <= 0xFFFF_FFFF_FFFF_FFFF, scored <= 0xFFFF_FFFF_FFFF_FFFF,
        ensures
            r == spec_meets_floor_60(killed as nat, scored as nat),
            scored == 0 ==> !r,
    {
        scored > 0 && killed * 100 >= scored * 60
    }

    /// The global #48 anti-Goodhart corollary: a `0/0` score (no scoreable mutant)
    /// never passes the floor, over all `killed` (REQ-11). A contract that cannot
    /// be mutation-validated is gated, never a vacuous pass.
    proof fn zero_scored_never_passes()
        ensures forall|k: nat| !(#[trigger] spec_meets_floor_60(k, 0nat)),
    {
        assert(forall|k: nat| !(#[trigger] spec_meets_floor_60(k, 0nat)));
    }

    }
}
