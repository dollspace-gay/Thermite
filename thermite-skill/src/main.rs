//! The `thermite-skill` bin — the `--emit` / `--check-budget` CLI that the CI
//! gauntlet runs (`.design/skill/skill-generator.md` REQ-6).
//!
//! - `--emit`: print `generate()` to stdout, exit 0 (the regeneration path for
//!   the committed `THERMITE.skill.md`, REQ-5).
//! - `--check-budget`: print the token count, exit 0 iff `<= SKILL_TOKEN_BUDGET`,
//!   non-zero otherwise (the §10 / §2.2 hard CI gate, REQ-4).
//!
//! Two flags, hand-matched (no `clap` dep — the no-magic posture of pillar §2.3
//! and `.design/forge/cli.md` REQ-2). No panics: every path returns an
//! `ExitCode` and an unknown/missing flag is a structured usage error to stderr
//! with a non-zero exit (R-CODE-2 / R-APG-1). Governing design:
//! `.design/skill/skill-generator.md`.

use std::process::ExitCode;

use thermite_skill::{generate, token_count, SKILL_TOKEN_BUDGET};

fn main() -> ExitCode {
    // Skip argv[0] (the program name); the surface is exactly one mode flag.
    match run(std::env::args().skip(1)) {
        Outcome::Success => ExitCode::SUCCESS,
        Outcome::Failure => ExitCode::FAILURE,
    }
}

/// The result of a single bin invocation. A plain two-state enum (not `ExitCode`,
/// which is not comparable) so the bin's logic is unit-testable (R-CHAR-3) while
/// `main` maps it to a real process exit code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Success,
    Failure,
}

/// Dispatch on the (single) mode flag, returning an [`Outcome`] (never panics).
///
/// `run` is the bin's testable core and the non-test production consumer of
/// both `generate` and `token_count` (R-DEFER-1).
fn run(mut args: impl Iterator<Item = String>) -> Outcome {
    let mode = args.next();
    // Exactly one flag is accepted; a trailing extra arg is a usage error.
    if args.next().is_some() {
        return usage_error("unexpected extra argument");
    }
    match mode.as_deref() {
        Some("--emit") => {
            // The skill goes to stdout; the caller's shell handles any redirect
            // (`--emit > THERMITE.skill.md`), so the bin writes only stdout.
            print!("{}", generate());
            Outcome::Success
        }
        Some("--check-budget") => {
            let count = token_count(&generate());
            println!("skill token count: {count} (budget {SKILL_TOKEN_BUDGET})");
            if count <= SKILL_TOKEN_BUDGET {
                Outcome::Success
            } else {
                eprintln!(
                    "error: skill is {count} tokens, over the {SKILL_TOKEN_BUDGET}-token budget; \
                     revert the feature that pushed it over (design §10)"
                );
                Outcome::Failure
            }
        }
        Some(other) => usage_error(&format!("unknown flag `{other}`")),
        None => usage_error("missing mode flag"),
    }
}

/// Print a structured usage error to stderr and return a failing [`Outcome`]
/// (never panics — R-CODE-2).
fn usage_error(detail: &str) -> Outcome {
    eprintln!("error: {detail}");
    eprintln!("usage: thermite-skill (--emit | --check-budget)");
    Outcome::Failure
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_args(args: &[&str]) -> Outcome {
        run(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn emit_exits_success() {
        assert_eq!(run_args(&["--emit"]), Outcome::Success);
    }

    #[test]
    fn check_budget_exits_success_under_budget() {
        // The real skill is well under budget, so --check-budget exits 0.
        assert_eq!(run_args(&["--check-budget"]), Outcome::Success);
    }

    #[test]
    fn unknown_flag_is_usage_error() {
        assert_eq!(run_args(&["--nonsense"]), Outcome::Failure);
    }

    #[test]
    fn missing_flag_is_usage_error() {
        assert_eq!(run_args(&[]), Outcome::Failure);
    }

    #[test]
    fn extra_arg_is_usage_error() {
        assert_eq!(run_args(&["--emit", "extra"]), Outcome::Failure);
    }
}
