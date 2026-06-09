//! `thermite-tv` — the contract-faithfulness translation-validation engine
//! (`.design/verified/contract-tv.md`; epic crosslink #139).
//!
//! `forge check` certifies that the EMITTED Verus contract holds for the
//! implementation; it does NOT certify that the emitted contract MEANS THE SAME
//! THING as the source contract the author wrote. Every existing guard (verus-
//! on-emitted, the cert oracle, the vacuity/mutation battery, the critic) takes
//! the emitted contract as ground truth, or is corpus-bounded (golden files).
//! This crate adds **contract-faithfulness translation validation (TV)**: an
//! INDEPENDENT reference encoder for the SpecTherm contract sublanguage
//! ([`ref_encode`]) plus a per-clause Z3 equivalence obligation
//! ([`obligation`]) of the shape `assert(P_production <==> P_reference)`. A
//! divergence is a real lowering-fidelity bug (the #122 cast-paren and #127
//! byte-view-misdispatch classes) that the five existing layers structurally
//! cannot see.
//!
//! ## THE INDEPENDENCE CONSTRAINT (non-negotiable — the whole point)
//!
//! TV checks `production-lowering ≡ reference-encoding`. This is N-version
//! differential validation: agreement is EVIDENCE, not proof. The reference
//! encoder is small, declarative, and auditable; the production `lower_expr` is
//! ~2000 lines. So "production agrees with an independently-auditable reference,
//! on every clause, for all inputs (Z3)" relocates faithfulness from *audit the
//! lowerer* to *audit the small reference + trust Z3 finds disagreement*. The
//! honesty boundary is HARD: this crate depends on `thermite-syntax` +
//! `thermite-spec` ONLY (see `Cargo.toml`) — NOT on `thermite-lower`. If the
//! reference reused production's `lower_expr`, independence would be lost and the
//! check vacuous (`assert(X <==> X)` always verifies). The dependency graph makes
//! that a compile error rather than a temptation (AC-6).
//!
//! ## What is re-implemented vs reused
//!
//! - **RE-IMPLEMENTED ([`ref_encode`], the infidelity surface):** the binop map
//!   (`==`/`<=`, F1), the slice→`@` view, the method→byte-view dispatch keyed on
//!   the receiver shape (`.byte_at(i)`, the #127 class, F3), the cast→`nat`/`int`
//!   (#122). Authored against `thermite-design.md` §4.2 directly.
//! - **REUSED (the shared frozen ground truth):** `thermite_spec::lookup(name)`
//!   — the 8 combinators' frozen `verus_l3` `spec fn` bodies. The registry IS the
//!   external combinator spec; reuse is correct (and the combinator ARGUMENT
//!   rewrites are still re-implemented, so F2's predicate infidelity is caught).
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (independent reference encoder) | SHIPPED | `ref_encode::ref_contract_pred` (`ref_encode.rs`); non-test consumer `obligation::equivalence_obligation`; verified by `tests/teeth.rs` F1–F4 under real verus. Deps `thermite-syntax` + `thermite-spec` ONLY (`Cargo.toml`) — no `thermite-lower` (AC-6). |
//! | REQ-2 (per-clause Z3 equivalence obligation + discharge) | SHIPPED | `obligation::equivalence_obligation` + `ObligationFrame` (`obligation.rs`); emits a self-contained `proof fn tv_check(<params>) requires <req> { assert((P_production) <==> (P_reference)); }`; discharged by `tests/teeth.rs` through real verus (the teeth-test is the non-test consumer of the obligation TEXT). |
//! | REQ-3 (off-corpus generator) | SHIPPED | `gen::generate_clauses` (`gen.rs`) — a DETERMINISTIC (SplitMix64-seeded, no `rand`/clock, R-CODE-5) generator of well-typed `bool`-valued contract-position `Expr`s over the frozen sublanguage (all comparison `BinOp`s, logical connectives incl. nesting, the 8 combinators with the correct arg KINDS per `thermite_spec::lookup(_).arg_kinds`, `spec_sum` calls, `result`/`old(acc)`, byte-view method calls, casts). Non-test consumer: the forge off-corpus run `forge::contract_tv::run_generated` (lowers each via `thermite_lower::lower_contract_expr` → TV-checks via `equivalence_obligation`). Pure generation in this INDEPENDENT crate — no `thermite-lower` dep (AC-6 intact). Determinism + construct-coverage asserted in `gen::tests` + `forge/tests/contract_tv_conformance.rs` (AC-7). |
//! | REQ-4 (the teeth — R-CHAR-3) | SHIPPED | `tests/teeth.rs` — F1 (comparison `==`/`<=`), F2 (combinator predicate `<`/`<=`), F3 (#127 byte-view index `0`/`1`), F4 (structural-drop conjunct): each FAITHFUL p_production VERIFIES + each INFIDEL produces a verus COUNTEREXAMPLE (`errors >= 1`). Skip-loudly if verus absent. |
//! | REQ-5 (forge plug-in point) | NOT-STARTED | open prereq blocker #144 (next dispatch — `forge/src/contract_tv.rs` unbuilt; not on this manifest, independence-respecting). |
//!
//! ## EXEC-position extension — step 2 (`.design/verified/exec-tv.md`; epic #151)
//!
//! Contract-TV (above) certifies the CONTRACT (`req`/`ens`/`inv`/`dec`); it does
//! NOT cover the EXEC BODY (where the #122/#146 infidelity classes GENERALLY
//! live). This crate now adds **exec-position TV (step 2.1)**: an INDEPENDENT
//! BOUNDED-VALUE reference denotation of a pure body-position exec expr
//! ([`exec_encode`]) wrapped as an EXEC-FN obligation `fn tv_exec_wrap(..) ensures
//! result == <reference> { <production exec lowering> }`
//! ([`obligation::exec_equivalence_obligation`]). The exec reference is BOUNDED
//! (`u64`/`usize`, NOT `nat`-coerced), so an overflow/wrapping infidelity is CAUGHT
//! at the production type rather than masked. The same INDEPENDENCE CONSTRAINT
//! holds (deps `thermite-syntax` + `thermite-spec` ONLY — no `thermite-lower`; the
//! exec reference is authored from `thermite-design.md` §4.1/§6 exec semantics, NOT
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
