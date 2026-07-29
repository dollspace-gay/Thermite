//! `thermite-tv` — the contract-faithfulness translation-validation engine
//! (`.design/verified/contract-tv.md`; epic crosslink #139).
//!
//! `forge check` certifies that the emitted Verus contract holds for the
//! implementation; it does not certify that the emitted contract means the same
//! thing as the source contract the author wrote. Every existing guard (verus-
//! on-emitted, the cert oracle, the vacuity/mutation battery, the critic) takes
//! the emitted contract as ground truth, or is corpus-bounded (golden files).
//! This crate adds contract-faithfulness translation validation (TV): an
//! independent reference encoder for the SpecTherm contract sublanguage
//! ([`ref_encode`]) plus a per-clause Z3 equivalence obligation
//! ([`obligation`]) of the shape `assert(P_production <==> P_reference)`. A
//! divergence is a lowering-fidelity bug (the #122 cast-paren and #127
//! byte-view-misdispatch classes) that the five existing layers structurally
//! cannot see.
//!
//! ## The independence constraint
//!
//! TV checks `production-lowering ≡ reference-encoding`. This is N-version
//! differential validation: agreement is evidence, not proof. The reference
//! encoder is small, declarative, and auditable; the production `lower_expr` is
//! ~2000 lines. So "production agrees with an independently-auditable reference,
//! on every clause, for all inputs (Z3)" relocates faithfulness from auditing the
//! lowerer to auditing the small reference and trusting Z3 to find disagreement.
//! The honesty boundary holds at compile time: this crate depends on
//! `thermite-syntax` + `thermite-spec` only (see `Cargo.toml`), not on
//! `thermite-lower`. If the reference reused production's `lower_expr`,
//! independence would be lost and the check vacuous (`assert(X <==> X)` always
//! verifies). The dependency graph makes that a compile error (AC-6).
//!
//! ## What is re-implemented vs reused
//!
//! - Re-implemented ([`ref_encode`], the infidelity surface): the binop map
//!   (`==`/`<=`, F1), the slice→`@` view, the method→byte-view dispatch keyed on
//!   the receiver shape (`.byte_at(i)`, the #127 class, F3), the cast→`nat`/`int`
//!   (#122). Authored against `thermite-design.md` §4.2 directly.
//! - Reused (the shared frozen ground truth): `thermite_spec::lookup(name)`
//!   — the 8 combinators' frozen `verus_l3` `spec fn` bodies. The registry is the
//!   external combinator spec; reuse is correct (the combinator argument
//!   rewrites are still re-implemented, so F2's predicate infidelity is caught).
//!
//! ## REQ status
//!
//! <!-- generated:reqs view=thermite-tv-contract-tv-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-TV-CONTRACT-FORGE-PLUGIN | shipped | `forge/src/contract_tv.rs` | Contract-TV forge plug-in point |  |
//! | REQ-TV-CONTRACT-GENERATOR | shipped | `thermite-tv/src/gen.rs` | Contract-TV off-corpus generator |  |
//! | REQ-TV-CONTRACT-OBLIGATION | shipped | `thermite-tv/src/obligation.rs` | Contract-TV per-clause Z3 equivalence obligation |  |
//! | REQ-TV-CONTRACT-REF-ENCODER | shipped | `thermite-tv/src/ref_encode.rs` | Contract-TV independent reference encoder |  |
//! | REQ-TV-CONTRACT-TEETH | shipped | `thermite-tv/tests/teeth.rs` | Contract-TV teeth |  |
//! <!-- /generated:reqs -->
//!
//! ## Exec-position extension — step 2 (`.design/verified/exec-tv.md`; epic #151)
//!
//! Contract-TV (above) certifies the contract (`req`/`ens`/`inv`/`dec`); it does
//! not cover the exec body (where the #122/#146 infidelity classes generally
//! live). This crate adds exec-position TV (step 2.1): an independent
//! bounded-value reference denotation of a pure body-position exec expr
//! ([`exec_encode`]) wrapped as an exec-fn obligation `fn tv_exec_wrap(..) ensures
//! result == <reference> { <production exec lowering> }`
//! ([`obligation::exec_equivalence_obligation`]). The exec reference is bounded
//! (`u64`/`usize`, not `nat`-coerced), so an overflow/wrapping infidelity is caught
//! at the production type rather than masked. The same independence constraint
//! holds (deps `thermite-syntax` + `thermite-spec` only, no `thermite-lower`; the
//! exec reference is authored from `thermite-design.md` §4.1/§6 exec semantics, not
//! from `lower_exec_expr`).
//!
//! <!-- generated:reqs view=thermite-tv-exec-tv-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-TV-EXEC-GENERATOR | shipped | `thermite-tv/src/gen.rs` | Exec-TV off-corpus generator |  |
//! | REQ-TV-EXEC-OBLIGATION | shipped | `thermite-tv/src/obligation.rs` | Exec-TV fn-wrapped equivalence obligation |  |
//! | REQ-TV-EXEC-REF-ENCODER | shipped | `thermite-tv/src/exec_encode.rs` | Exec-TV independent reference encoder |  |
//! | REQ-TV-EXEC-TEETH | shipped | `thermite-tv/tests/exec_teeth.rs` | Exec-TV teeth |  |
//! <!-- /generated:reqs -->

pub mod exec_encode;
pub mod exec_stmt_encode;
pub mod gen;
// SPIKE-2 (`.design/m0-spikes.md` REQ-6 / AC-6): the prototype normalizer probe.
// It is `pub` so the spike's `tests/strat_probe.rs` hit-rate target can reach it,
// but it is a leaf: no TV pipeline code path references it. This `pub mod` is the
// mandated export, not a consumer (REQ-6: "exported but not referenced by any TV
// pipeline code path"); the AC-6 grep finds only this declaration + the module's
// own body, no call site in `thermite-tv/src/` or `forge/src/`.
pub mod normalize;
pub mod obligation;
pub mod ref_encode;
// Stage-2 REQ-8 (`.design/stage2-stratified-cage.md` REQ-8 / AC-8): the stratified
// reference encoder + the two-phase TV (syntactic normalizer + thin semantic fallback)
// + the trust flip. Unlike `normalize` (the SPIKE-2 leaf), these ARE required TV
// pipeline modules (consumed by `forge`'s stratified faithfulness sweep).
pub mod strat_ref_encode;
pub mod strat_two_phase;

pub use exec_encode::{exec_ref_value, ExecRefCtx};
pub use exec_stmt_encode::{
    body_ref_state, body_ref_state_ensures, loop_ref_obligations, negate_condition, BodyRefCtx,
    LoopObligations,
};
pub use gen::{gen_exec_exprs, generate_clauses, ExecClause, Rng};
pub use obligation::{
    body_equivalence_obligation, equivalence_obligation, exec_equivalence_obligation,
    loop_entry_obligation, loop_exit_obligation, loop_preservation_obligation, BodyObligationFrame,
    BodyParamDecl, ExecObligationFrame, ExecParamDecl, LoopObligationFrame, LoopParamDecl,
    ObligationFrame, ParamDecl,
};
pub use ref_encode::{ref_contract_pred, RefCtx, RefEncodeError};
pub use strat_ref_encode::strat_ref_encode;
pub use strat_two_phase::{
    classify_pair, g2_flip_permitted, run_two_phase, semantic_obligation, strat_trust_profile,
    strat_trust_profile_current, strat_trust_profile_gated, ClauseRoute, G2Checks, PhaseSplit,
    SemanticOutcome, StratClause, TvPhase, TvVerdict, TwoPhaseReport, G2_FLIPPED,
    REF_ENCODE_PROVEN, REF_ENCODE_UNPROVEN, SOLVER_Z3,
};
