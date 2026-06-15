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
//! divergence is a real lowering-fidelity bug (the #122 cast-paren and #127
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
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | exec-REQ-1 (exec-expr reference encoder) | SHIPPED | `exec_encode::exec_ref_value` (`exec_encode.rs`) — bounded `u64`/`u32`/`usize`/`bool` value, the #122 inner-paren + #146 cast-`<` outer-paren (independent `is_lt_leading`), the `xs[i as int]` element value; non-test consumer `obligation::exec_equivalence_obligation`; verified by `tests/exec_teeth.rs` E1–E4 under real verus. No `thermite-lower` dep (`Cargo.toml`, AC-6). |
//! | exec-REQ-2 (exec-fn-wrapped obligation + discharge) | SHIPPED | `obligation::exec_equivalence_obligation` + `ExecObligationFrame`/`ExecParamDecl` (`obligation.rs`); emits the self-contained `fn tv_exec_wrap(..) requires <req>, ensures result == <ref> { <p_prod> }` EXEC-FN form; discharged by `tests/exec_teeth.rs` through real verus (the teeth-test is the non-test consumer of the obligation TEXT). |
//! | exec-REQ-3 (off-corpus exec generator) | SHIPPED | `gen::gen_exec_exprs` + `gen::ExecClause` (`gen.rs`) — a DETERMINISTIC (SplitMix64-seeded, no `rand`/clock, R-CODE-5) generator of WELL-FRAMED exec-position `Expr`s over the bounded exec sublanguage: `u64`/`usize` arithmetic (`+`/`-`/`*`), shifts, bitwise, narrowing/widening casts (`as u8`/`u16`/`u32`/`u64`/`usize`), the cast-`<` surface (`x as u32 < k` — the #146 guard), and slice indexing (`xs[i]`). Each `ExecClause` carries the ADEQUATE overflow/index FRAME (every base scalar `<= 1000` + an index `< xs.len()`) so the FAITHFUL lowering VERIFIES (the overflow obligation does not spuriously fire — the critic's frame concern). Non-test consumer: `forge::exec_tv::run_generated` (lowers each `expr` via `thermite_lower::lower_exec_expr` → discharges `exec_equivalence_obligation`). Pure generation in this INDEPENDENT crate — no `thermite-lower` dep (AC-6 intact). Determinism + construct coverage + self-framing asserted in `gen::tests` + `forge/tests/exec_tv_conformance.rs` (AC-7). |
//! | exec-REQ-4 (the exec teeth — R-CHAR-3) | SHIPPED | `tests/exec_teeth.rs` — E1 (#122 cast-paren), E2 (#146 cast-`<`), E3 (wrong-op/overflow), E4 (off-by-one index): each FAITHFUL `p_production` (the real `lower_exec_expr`) VERIFIES + each INFIDEL is CAUGHT (E0308/parse error/postcondition counterexample). Skip-loudly if verus absent. |

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

pub use exec_encode::{exec_ref_value, ExecRefCtx};
pub use exec_stmt_encode::{
    body_ref_state, body_ref_state_ensures, loop_ref_obligations, negate_condition, BodyRefCtx,
    LoopObligations,
};
pub use gen::{gen_exec_exprs, generate_clauses, ExecClause};
pub use obligation::{
    body_equivalence_obligation, equivalence_obligation, exec_equivalence_obligation,
    loop_entry_obligation, loop_exit_obligation, loop_preservation_obligation, BodyObligationFrame,
    BodyParamDecl, ExecObligationFrame, ExecParamDecl, LoopObligationFrame, LoopParamDecl,
    ObligationFrame, ParamDecl,
};
pub use ref_encode::{ref_contract_pred, RefCtx, RefEncodeError};
