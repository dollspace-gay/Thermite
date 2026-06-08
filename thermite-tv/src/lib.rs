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
//! | REQ-3 (off-corpus generator) | NOT-STARTED | open prereq blocker #142 (next dispatch — `src/gen.rs` unbuilt; not on this manifest). |
//! | REQ-4 (the teeth — R-CHAR-3) | SHIPPED | `tests/teeth.rs` — F1 (comparison `==`/`<=`), F2 (combinator predicate `<`/`<=`), F3 (#127 byte-view index `0`/`1`), F4 (structural-drop conjunct): each FAITHFUL p_production VERIFIES + each INFIDEL produces a verus COUNTEREXAMPLE (`errors >= 1`). Skip-loudly if verus absent. |
//! | REQ-5 (forge plug-in point) | NOT-STARTED | open prereq blocker #144 (next dispatch — `forge/src/contract_tv.rs` unbuilt; not on this manifest, independence-respecting). |

pub mod obligation;
pub mod ref_encode;

pub use obligation::{equivalence_obligation, ObligationFrame, ParamDecl};
pub use ref_encode::{ref_contract_pred, RefCtx, RefEncodeError};
