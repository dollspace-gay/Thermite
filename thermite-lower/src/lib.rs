//! `thermite-lower` — lowering Thermite AST to Verus-annotated Rust source, plus
//! L1 runtime-check compilation.
//!
//! In the v0.1 kernel DAG (REQ-2) this crate depends on `thermite-syntax` and
//! `thermite-spec`. The L3 emission stage (`lower`) lands in `lower.rs` (issue
//! #4) per the route table; the L1 runtime-check stage (`l1::lower_l1`) is the
//! sibling `l1.rs` (`.design/lower/l1-runtime-checks.md`); effect subsumption is
//! a separate dispatch. This crate's OWN error type (`LowerError`) is born in
//! `lower.rs` with its first fallible function `lower` (workspace.md REQ-3) and
//! is shared by `l1::lower_l1`.
//!
//! Governing design: `.design/scaffold/workspace.md` (crate topology),
//! `.design/lower/verus-lowering.md` (the L3 emission contract).
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (workspace topology) | SHIPPED | this crate is a `lib` member of the virtual workspace in root `Cargo.toml`. |
//! | REQ-2 (dependency DAG, leaf-first) | SHIPPED | `thermite-lower/Cargo.toml` declares path deps `thermite-syntax` + `thermite-spec`. |
//! | REQ-3 (Result discipline; crate error type born with first fallible fn) | SHIPPED | `LowerError` is declared in `lower.rs` with `lower`; `pub use`d below. |
//! | REQ-6 (scaffold compiles clean) | SHIPPED | no stubs, no `mod` pointing at a missing file; `cargo build --workspace` is green. |

pub mod effects;
pub mod l1;
pub mod lower;

pub use effects::{check_effects, subsumes};
pub use l1::lower_l1;
pub use lower::{lower, LowerError};
