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
//! <!-- generated:reqs view=forge-main-scaffold-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-SCAFFOLD-FORGE-COMPILE | shipped | `forge/src/main.rs` | Forge clean compile |  |
//! | REQ-SCAFFOLD-FORGE-DAG | shipped | `forge/src/main.rs` | Forge dependency DAG |  |
//! | REQ-SCAFFOLD-FORGE-RESULT | shipped | `forge/src/main.rs` | Forge result discipline |  |
//! | REQ-SCAFFOLD-FORGE-WORKSPACE | shipped | `forge/src/main.rs` | Forge workspace topology |  |
//! <!-- /generated:reqs -->

mod accessibility;
mod audit;
mod battery;
mod bitvector;
mod body_tv;
mod build;
mod burn;
mod cache;
mod check;
mod cli;
mod closure;
mod contract_tv;
mod covenant;
mod covenant_engine;
mod covenant_eval;
mod degrade;
mod effect_wrappers;
mod engine;
mod epr_reconstruct;
mod exec_tv;
mod forks;
mod goal_repl;
mod kani;
mod lean_export;
mod lean_smt_export;
mod lemma_library;
mod manifest;
mod meaning;
mod metrics;
mod mutation;
mod obligation;
mod profile;
mod relax;
mod repair;
mod review;
mod sandbox;
/// The seven-verdict hermetic suite (REQ-10 / AC-14) — test-only.
#[cfg(test)]
mod seven_verdicts;
mod slag;
mod strat_faithful;
mod strat_tv;
mod strengthen;
mod tv_signal;
mod vacuity;
mod vacuity_solver;
mod verdict;
mod verified_build;

use std::process::ExitCode;

/// The driver entry point. All logic — `argv`, dispatch, rendering, exit-code
/// mapping — lives in `cli::run` (`.design/forge/cli.md`).
fn main() -> ExitCode {
    cli::run()
}
