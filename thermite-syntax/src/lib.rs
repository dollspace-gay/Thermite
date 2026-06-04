//! `thermite-syntax` — the Thermite surface-syntax foundation: lexer, recovering
//! parser, AST, and stable semantic addressing (`loop#1.inv#2`).
//!
//! This is the leaf crate of the v0.1 kernel DAG (REQ-2): it has no
//! intra-workspace dependencies. At scaffold time (issue #1) it is an empty,
//! clean library root — the lexer/parser/AST/addressing modules land in their
//! owning issue (#3) per the route table. Per the scaffold contract (REQ-3),
//! no error type is created here; `thermite_syntax::SyntaxError` lands with the
//! first fallible function in #3.
//!
//! Governing design: `.design/scaffold/workspace.md`.
//!
//! ## REQ status
//!
//! Only the REQs this crate root materializes are listed; whole-workspace REQs
//! (CI, toolchain pin) are tracked in the root manifest's owning files.
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (workspace topology) | SHIPPED | this crate is a `lib` member of the virtual workspace in root `Cargo.toml`. |
//! | REQ-2 (dependency DAG, leaf-first) | SHIPPED | `thermite-syntax/Cargo.toml` declares zero intra-workspace path deps (the leaf). |
//! | REQ-3 (Result discipline; no scaffold error type) | SHIPPED | this file declares no error type and no `unwrap`/`expect`/`panic!` (empty root). |
//! | REQ-6 (empty scaffold compiles clean) | SHIPPED | no stubs, no `mod` pointing at a missing file; `cargo build --workspace` is green. |
