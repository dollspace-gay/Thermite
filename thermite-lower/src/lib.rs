//! `thermite-lower` — lowering Thermite AST to Verus-annotated Rust source, plus
//! L1 runtime-check compilation.
//!
//! In the v0.1 kernel DAG (REQ-2) this crate depends on `thermite-syntax` and
//! `thermite-spec`. The L3 emission stage (`lower`) lands in `lower.rs` (issue
//! #4) per the route table; the L1 runtime-check stage (`l1::lower_l1`) is the
//! sibling `l1.rs` (`.design/lower/l1-runtime-checks.md`); the L2 Kani-harness
//! stage (`l2::lower_l2`) is the sibling `l2.rs` (`.design/lower/l2-kani.md`,
//! issue #9 / v0.2) and reuses the L1 executable lowering; effect subsumption is
//! a separate dispatch. This crate's own error type (`LowerError`) is born in
//! `lower.rs` with its first fallible function `lower` (workspace.md REQ-3) and
//! is shared by `l1::lower_l1` and `l2::lower_l2`.
//!
//! Governing design: `.design/scaffold/workspace.md` (crate topology),
//! `.design/lower/verus-lowering.md` (the L3 emission contract).
//!
//! ## REQ status
//!
//! <!-- generated:reqs view=thermite-lower-scaffold-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-SCAFFOLD-LOWER-COMPILE | shipped | `thermite-lower/src/lib.rs` | Lower clean compile |  |
//! | REQ-SCAFFOLD-LOWER-DAG | shipped | `thermite-lower/src/lib.rs` | Lower dependency DAG |  |
//! | REQ-SCAFFOLD-LOWER-RESULT | shipped | `thermite-lower/src/lib.rs` | Lower result discipline |  |
//! | REQ-SCAFFOLD-LOWER-WORKSPACE | shipped | `thermite-lower/src/lib.rs` | Lower workspace topology |  |
//! <!-- /generated:reqs -->

pub mod effects;
pub mod l1;
pub mod l2;
pub mod lower;

pub use effects::{check_effects, subsumes};
pub use l1::lower_l1;
pub use l2::{bound_string, lower_l2};
pub use lower::{
    block_uses_fixed_array_equality, expr_uses_fixed_array_equality, fixed_array_equality_defs,
    lower, lower_contract_expr, lower_equivalence_obligation, lower_exec_body, lower_exec_expr,
    lower_l3_library, lower_l3_library_with_boundaries, spec_fn_param_type_map, L3BoundaryBinding,
    L3Export, L3ExportVisibility, L3LibraryTarget, LowerError,
};
