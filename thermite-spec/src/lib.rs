//! `thermite-spec` — the SpecTherm combinator registry + validator.
//!
//! Two components, both governed by `.design/spec/spectherm-combinators.md`:
//!
//! - **`combinators`** — the frozen v0.1 combinator registry (name / arity /
//!   arg-kinds / result) and `lookup` (§4.2; REQ-1/REQ-2). The frozen SMT
//!   trigger + Verus (L3) + executable (L1) lowering facet is deferred to issue
//!   #4 (the `CombinatorSig` struct is left extensible for it; OQ-2).
//! - **`schemes`** — the frozen v0.1 recursion-scheme registry (Basis Stage 2,
//!   `.design/basis/02-recursion-schemes.md` REQ-1/REQ-2): the 5 schemes
//!   (`fold`/`map`/`for_all`/`exists`/`traverse`) over recursive ADTs, each with
//!   its step shape + result kind + generated-fn-name function. The structural
//!   complement of `combinators` (`lookup` precedent); consumed by `validator`
//!   (the scheme-call accept + flat-step cage) and `thermite-lower` (the
//!   generated `fold_<e>`/`for_all_<e>` materialization).
//! - **`validator`** — `validate`, the boundary API that walks a parsed
//!   `thermite-syntax` program's contract positions and enforces the §4.2 cage,
//!   plus `thermite-spec`'s own `SpecError` enum (workspace.md REQ-3; REQ-3/4/5).
//!
//! In the kernel DAG this crate depends on `thermite-syntax` (it consumes the
//! AST). `validate` is the registry's first production consumer (it calls
//! `combinators::lookup`), so the registry has a consumer beyond its vocabulary
//! (R-DEFER-1). It is the gate `thermite-lower` (#4) and `forge` (#6) call
//! before lowering / the vacuity battery.
//!
//! Governing design: `.design/scaffold/workspace.md` (crate shape) +
//! `.design/spec/spectherm-combinators.md` (the registry + validator contract).
//!
//! ## REQ status — workspace.md (scaffold)
//!
//! <!-- generated:reqs view=thermite-spec-scaffold-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-SCAFFOLD-SPEC-COMPILE | shipped | `thermite-spec/src/lib.rs` | Spec clean compile |  |
//! | REQ-SCAFFOLD-SPEC-DAG | shipped | `thermite-spec/src/lib.rs` | Spec dependency DAG |  |
//! | REQ-SCAFFOLD-SPEC-RESULT | shipped | `thermite-spec/src/lib.rs` | Spec result discipline |  |
//! | REQ-SCAFFOLD-SPEC-WORKSPACE | shipped | `thermite-spec/src/lib.rs` | Spec workspace topology |  |
//! <!-- /generated:reqs -->
//!
//! ## REQ status — spectherm-combinators.md (issue #2)
//!
//! <!-- generated:reqs view=thermite-spec-lib-combinators-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-SPEC-COMBINATORS-FROZEN | shipped | `thermite-spec/src/combinators.rs` | Frozen combinator set |  |
//! | REQ-SPEC-COMBINATORS-SHAPE | shipped | `thermite-spec/src/combinators.rs` | Combinator registry data shape |  |
//! | REQ-SPEC-VALIDATOR-ACCEPT | shipped | `thermite-spec/src/validator.rs` | Validator accept rule |  |
//! | REQ-SPEC-VALIDATOR-DEPTH | shipped | `thermite-spec/src/validator.rs` | Validator bounded recursion |  |
//! | REQ-SPEC-VALIDATOR-FLAT-CLOSURE | shipped | `thermite-spec/src/validator.rs` | Validator flat closure fragment |  |
//! | REQ-SPEC-VALIDATOR-REJECT | shipped | `thermite-spec/src/validator.rs` | Validator reject cases |  |
//! <!-- /generated:reqs -->

pub mod classifier;
pub mod combinators;
pub mod restratify;
pub mod s2_recon;
pub mod schemes;
pub mod validator;

pub use classifier::{admitted, classify, parse_frm, to_wire, Frm, RejectReason, Sort2, Verdict};
pub use combinators::{all, lookup, ArgKind, CombinatorSig, ResultKind};
pub use restratify::{certify, restratify, Certification, RestratResult, WithheldReason};
pub use s2_recon::{
    canonical_source_expr, from_clause as s2_recon_from_clause,
    from_obligation as s2_recon_from_obligation, QFreeAtom, QFreeFragment, S2Recon, SourceAddress,
};
pub use schemes::{SchemeResult, SchemeSig, StepShape};
pub use validator::{
    array_equality_type_is_structural, structural_array_equality_structs,
    structural_record_mutation_structs, validate, SpecError,
};
