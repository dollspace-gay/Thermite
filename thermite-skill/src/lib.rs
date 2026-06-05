//! `thermite-skill` — the `THERMITE.skill.md` generator plus the CI 6,000-token
//! budget gate (issue #7, the last v0.1-kernel leaf; `goal.md` Scope step 5).
//!
//! In the kernel DAG this crate depends on `thermite-spec` (the frozen
//! combinator registry — the single source of truth the combinator section is
//! machine-rendered from) and `thermite-syntax` (the grammar). The generator is
//! [`generate::generate`]; the budget heuristic is [`generate::token_count`].
//! The `thermite-skill` bin (`src/main.rs`, `--emit` / `--check-budget`) is the
//! CLI that the CI gauntlet runs.
//!
//! Governing design: `.design/skill/skill-generator.md`.
//! Thesis: `thermite-design.md` §2.2 (≤ 6,000-token hard budget), §10 (the skill
//! IS the spec; regenerated from the registry).
//!
//! ## REQ status
//!
//! The full per-REQ table lives on [`generate`] (the module the REQs govern).
//! Summary: REQ-1..REQ-7 SHIPPED — `generate`/`token_count` here, the bin in
//! `main.rs`, the committed `THERMITE.skill.md` at the repo root, and the
//! `--check-budget` CI step in `.github/workflows/ci.yml`.

pub mod generate;

pub use generate::{generate, token_count, SKILL_TOKEN_BUDGET};
