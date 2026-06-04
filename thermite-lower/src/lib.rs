//! `thermite-lower` — lowering Thermite AST to Verus-annotated Rust source, plus
//! L1 runtime-check compilation.
//!
//! In the v0.1 kernel DAG (REQ-2) this crate depends on `thermite-syntax` and
//! `thermite-spec`. At scaffold time (issue #1) it is an empty, clean library
//! root — the lowering / L1 / effect-subsumption modules land in their owning
//! issue (#4) per the route table. Per the scaffold contract (REQ-3), no error
//! type is created here.
//!
//! Governing design: `.design/scaffold/workspace.md`.
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (workspace topology) | SHIPPED | this crate is a `lib` member of the virtual workspace in root `Cargo.toml`. |
//! | REQ-2 (dependency DAG, leaf-first) | SHIPPED | `thermite-lower/Cargo.toml` declares path deps `thermite-syntax` + `thermite-spec`. |
//! | REQ-3 (Result discipline; no scaffold error type) | SHIPPED | this file declares no error type and no `unwrap`/`expect`/`panic!` (empty root). |
//! | REQ-6 (empty scaffold compiles clean) | SHIPPED | no stubs, no `mod` pointing at a missing file; `cargo build --workspace` is green. |
