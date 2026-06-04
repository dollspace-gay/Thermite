//! `thermite-skill` — the `THERMITE.skill.md` generator plus the CI 6,000-token
//! budget gate.
//!
//! In the v0.1 kernel DAG (REQ-2) this crate depends on `thermite-spec` (the
//! combinator registry) and `thermite-syntax` (the grammar). At scaffold time
//! (issue #1) it is an empty, clean library root — the generator and the
//! `--check-budget` CI step land in issue #7 (REQ-7), NOT here. Per the scaffold
//! contract (REQ-3), no error type is created here.
//!
//! Governing design: `.design/scaffold/workspace.md`.
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (workspace topology) | SHIPPED | this crate is a `lib` member of the virtual workspace in root `Cargo.toml`. |
//! | REQ-2 (dependency DAG, leaf-first) | SHIPPED | `thermite-skill/Cargo.toml` declares path deps `thermite-spec` + `thermite-syntax`. |
//! | REQ-3 (Result discipline; no scaffold error type) | SHIPPED | this file declares no error type and no `unwrap`/`expect`/`panic!` (empty root). |
//! | REQ-6 (empty scaffold compiles clean) | SHIPPED | no stubs, no `mod` pointing at a missing file; `cargo build --workspace` is green. |
//! | REQ-7 (skill-budget gate deferred to #7) | NOT-STARTED | open prereq issue #7; no `generate.rs`, no `--check-budget` CI step at scaffold time. |
