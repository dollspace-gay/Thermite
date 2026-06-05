//! `forge` — the Thermite CLI / verification driver. v0.1 (issue #5) ships the
//! first end-to-end `forge check <file.th> → certificate`: `forge new <name>`
//! (project scaffold) and `forge check <file> [--json]` (the verus-backed ladder
//! pipeline emitting a structured per-obligation certificate).
//!
//! `main.rs` is the thin entry point (`.design/forge/cli.md` Architecture): it
//! delegates to `cli::run`, which owns `argv` parsing, dispatch, rendering, and
//! the typed exit-code mapping. The pipeline lives in `check.rs`
//! (`.design/forge/check.md`) and the certificate schema in `manifest.rs`
//! (`.design/forge/certificate-manifest.md`).
//!
//! Governing design: `.design/forge/cli.md`, `check.md`, `certificate-manifest.md`.
//!
//! ## REQ status (scaffold REQs, `.design/scaffold/workspace.md`)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (workspace topology) | SHIPPED | sole `bin` member; `[[bin]]` in `forge/Cargo.toml`. |
//! | REQ-2 (dependency DAG, leaf-first) | SHIPPED | path deps on all three libs + `thermite-skill`; `check.rs` drives `parse`/`validate`/`check_effects`/`lower`. |
//! | REQ-3 (Result discipline; error type) | SHIPPED | `ForgeError` born in `cli.rs`; `main` returns `ExitCode` from `cli::run`; no `unwrap`/`expect`/`panic!`. |
//! | REQ-6 (clean compile) | SHIPPED | gauntlet green; the anti-pattern gate passes (no placeholder macros). |

mod cache;
mod check;
mod cli;
mod kani;
mod manifest;
mod profile;
mod slag;
mod vacuity;

use std::process::ExitCode;

/// The driver entry point. All logic — `argv`, dispatch, rendering, exit-code
/// mapping — lives in `cli::run` (`.design/forge/cli.md`).
fn main() -> ExitCode {
    cli::run()
}
