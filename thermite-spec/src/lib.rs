//! `thermite-spec` — the SpecTherm combinator registry + validator.
//!
//! Two components, both governed by `.design/spec/spectherm-combinators.md`:
//!
//! - **`combinators`** — the FROZEN v0.1 combinator registry (name / arity /
//!   arg-kinds / result) and `lookup` (§4.2; REQ-1/REQ-2). The frozen SMT
//!   trigger + Verus (L3) + executable (L1) lowering facet is DEFERRED to issue
//!   #4 (the `CombinatorSig` struct is left extensible for it; OQ-2).
//! - **`schemes`** — the FROZEN v0.1 recursion-scheme registry (Basis Stage 2,
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
//! `combinators::lookup`), so the registry is not vocabulary-only (R-DEFER-1).
//! It is the gate `thermite-lower` (#4) and `forge` (#6) call before lowering /
//! the vacuity battery.
//!
//! Governing design: `.design/scaffold/workspace.md` (crate shape) +
//! `.design/spec/spectherm-combinators.md` (the registry + validator contract).
//!
//! ## REQ status — workspace.md (scaffold)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (workspace topology) | SHIPPED | this crate is a `lib` member of the virtual workspace in root `Cargo.toml`. |
//! | REQ-2 (dependency DAG, leaf-first) | SHIPPED | `thermite-spec/Cargo.toml` declares the single path dep `thermite-syntax`. |
//! | REQ-3 (per-crate error enum) | SHIPPED | `SpecError` is born here in `validator.rs` with the first fallible fn `validate` and `pub use`d below. |
//! | REQ-6 (compiles clean) | SHIPPED | no stubs, no `mod` pointing at a missing file; no `unwrap`/`expect`/`panic!` in `src/`. |
//!
//! ## REQ status — spectherm-combinators.md (issue #2)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (frozen combinator set) | SHIPPED | `combinators::all`/`REGISTRY` — 8 frozen entries; asserted against `tests/golden/combinators/registry.json`. |
//! | REQ-2 (registry data shape) | SHIPPED | `CombinatorSig`/`ArgKind`/`ResultKind` + `lookup`; consumed by `validate`. |
//! | REQ-3 (validator accept rule) | SHIPPED | `validate` walks `Contract.req`/`ens`, `LoopNode.invs`/`dec`, `SpecFnItem.body`; every `accept.json` case validates clean. |
//! | REQ-4 (reject cases, structured `SpecError`) | SHIPPED | `SpecError::{UnknownCombinator,WrongArity,WrongArgKind,ForbiddenCall,ExpressionTooDeep}`; every `reject.json` case yields the expected cause. |
//! | REQ-5 (bounded recursion) | SHIPPED | single `MAX_RECURSION_DEPTH` guard via `Validator::descend` over every recursive descent; deep input → `ExpressionTooDeep`. |
//! | REQ-6 (flat-closure-fragment rule) | SHIPPED | `Validator::in_combinator_closure` set on entry to a combinator `Pred`-slot closure body (kept set for all nesting); while set, `walk_call` rejects a registered-combinator callee with `SpecError::NestedCombinator`, named `spec fn` calls stay accepted. `reject.json` `nested_combinator_in_closure` → `NestedCombinator`; `accept.json` `named_spec_fn_in_closure` → `Ok`; flat corpus closures unaffected. |

pub mod combinators;
pub mod schemes;
pub mod validator;

pub use combinators::{all, lookup, ArgKind, CombinatorSig, ResultKind};
pub use schemes::{SchemeResult, SchemeSig, StepShape};
pub use validator::{validate, SpecError};
