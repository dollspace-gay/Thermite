//! `forge` — the Thermite CLI / verification driver: `forge new`, `forge check`
//! (run the ladder, structured per-obligation JSON + counterexamples),
//! structural vacuity triage, `#[slag]`, the proof cache, and pinned seeds.
//!
//! `forge` is the sole binary crate of the v0.1 kernel (REQ-1). In the DAG
//! (REQ-2) it depends on all three libraries (`thermite-syntax`, `thermite-spec`,
//! `thermite-lower`) and on `thermite-skill`. At scaffold time (issue #1) it is a
//! real entry point that exits cleanly (exit code 0) with no command surface yet
//! and no error type — `forge::ForgeError` and the CLI land with issue #5 (REQ-3,
//! REQ-6). It does not `panic!` and contains no stubs.
//!
//! Governing design: `.design/scaffold/workspace.md`.
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (workspace topology) | SHIPPED | this crate is the sole `bin` member of the virtual workspace (root `Cargo.toml` + `[[bin]]` in `forge/Cargo.toml`). |
//! | REQ-2 (dependency DAG, leaf-first) | SHIPPED | `forge/Cargo.toml` declares path deps on all three libs + `thermite-skill`. |
//! | REQ-3 (Result discipline; no scaffold error type) | SHIPPED | `fn main` returns `()` and exits 0; no error type, no `unwrap`/`expect`/`panic!`. |
//! | REQ-6 (empty scaffold compiles clean) | SHIPPED | real entry point exiting cleanly; no stubs, no missing-module `mod`; gauntlet green. |

fn main() {
    // Empty-but-clean scaffold entry point (REQ-6): exits 0. The command
    // surface (`new`/`check`/...) and `ForgeError` arrive in issue #5.
}
