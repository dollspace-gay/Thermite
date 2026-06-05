//! `forge/src/cli.rs` — the command surface of the `forge` driver. It parses
//! `argv` with a minimal hand-rolled matcher (REQ-2, NOT a derive macro),
//! dispatches `forge new <name>` and `forge check [<file>] [--json]`, renders the
//! certificate as human-readable text or (under `--json`) the §5.1 structured
//! JSON, and owns [`ForgeError`] — the BOUNDARY error that aggregates each driven
//! crate's error (`thermite_syntax::SyntaxError`, `thermite_spec::SpecError`,
//! `thermite_lower::LowerError`) plus driver-native verus/io/usage variants.
//!
//! Governing design: `.design/forge/cli.md`.
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (command surface) | SHIPPED | `run` matches `new`/`check`; an unknown verb → `ForgeError::Usage`; the other Appendix B verbs are out of #5. Consumer: `main::main`. |
//! | REQ-2 (hand-rolled arg parsing) | SHIPPED | `parse_args` is a `match` over the verb + positionals + the single `--json` flag; no `clap` dependency in `forge/Cargo.toml`. |
//! | REQ-3 (`ForgeError` aggregation) | SHIPPED | `enum ForgeError` wraps `Vec<SyntaxError>`/`Vec<SpecError>`/`Vec<LowerError>`/`LowerError` and carries `VerusAbsent`/`VerusSpawn`/`VerusOutput`/`Io`/`Usage`; `Display` forwards each inner error's diagnostic (no information lost). |
//! | REQ-4 (human + `--json` output) | SHIPPED | `render_human` / `serde_json::to_string_pretty` of the cert array; `run_check` picks the rendering from `--json`; diagnostics go to stderr so `--json` stdout is a clean document. |
//! | REQ-5 (typed exit codes) | SHIPPED | `Outcome` → `ExitCode`: verified (all L3) → 0, reported verification failure → `EXIT_VERIFICATION_FAILURE`, environment/usage/IO → `EXIT_ENVIRONMENT`. |
//! | REQ-6 (no panics; Result discipline) | SHIPPED | every fallible path returns `Result<_, ForgeError>`; no `unwrap`/`expect`/`panic!` outside `#[cfg(test)]`; verus exit status inspected in `check.rs`. |
//! | REQ-7 (`forge new` scaffold) | SHIPPED | `scaffold_project` writes `forge.toml` + `forge.lock` (pinned seed, §5.3) + `THERMITE.skill.pin`; refuses a non-empty target (`ForgeError::Usage`). |

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use thermite_lower::LowerError;
use thermite_spec::SpecError;
use thermite_syntax::SyntaxError;

use crate::check::{self, DEFAULT_SOLVER_SEED};
use crate::manifest::{Certificate, Level, ObligationStatus};

/// Exit code: a reported verification FAILURE (the certificate is a valid
/// document describing failed obligations). Distinct from an environment error
/// (REQ-5).
pub const EXIT_VERIFICATION_FAILURE: u8 = 1;
/// Exit code: an environment / usage / IO error (verus absent, bad argv,
/// unreadable file). A failed proof and a missing solver are NOT the same
/// outcome (REQ-5, R-CODE-4).
pub const EXIT_ENVIRONMENT: u8 = 2;

/// The boundary error type (REQ-3): the workspace's first AGGREGATING error. It
/// WRAPS each driven crate's error (which keeps its own type per the leaf-first
/// DAG) and adds driver-native verus/io/usage variants. It does NOT replace the
/// per-crate errors; it composes them at the driver boundary.
#[derive(Debug)]
pub enum ForgeError {
    /// Parse stage failed (`thermite_syntax`).
    Parse(Vec<SyntaxError>),
    /// Spec validation failed (`thermite_spec`).
    Spec(Vec<SpecError>),
    /// Effect-check failed (`thermite_lower::check_effects`).
    Effects(Vec<LowerError>),
    /// Lowering failed (`thermite_lower::lower`).
    Lower(LowerError),
    /// The `verus` binary was not found on `PATH` — an ENVIRONMENT error, NOT a
    /// verification failure (REQ-6 / `.design/forge/check.md` REQ-6).
    VerusAbsent { binary: String },
    /// Spawning `verus` failed for a reason other than absence (e.g. permission).
    VerusSpawn { source: std::io::Error },
    /// Verus ran but its output could not be parsed into a verification summary,
    /// or it reported an internal (VIR) error (never swallowed, REQ-3 /
    /// R-CODE-4).
    VerusOutput { detail: String },
    /// An IO error reading a source file or writing a scaffold/temp file.
    Io {
        path: String,
        source: std::io::Error,
    },
    /// A usage error: missing/unknown verb, missing positional, bad flag, or a
    /// `forge new` target that already exists.
    Usage(String),
}

impl fmt::Display for ForgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ForgeError::Parse(errs) => {
                writeln!(f, "parse failed ({} error(s)):", errs.len())?;
                for e in errs {
                    writeln!(f, "  - {e}")?;
                }
                Ok(())
            }
            ForgeError::Spec(errs) => {
                writeln!(f, "spec validation failed ({} error(s)):", errs.len())?;
                for e in errs {
                    writeln!(f, "  - {e}")?;
                }
                Ok(())
            }
            ForgeError::Effects(errs) => {
                writeln!(f, "effect check failed ({} error(s)):", errs.len())?;
                for e in errs {
                    writeln!(f, "  - {e}")?;
                }
                Ok(())
            }
            ForgeError::Lower(e) => write!(f, "lowering failed: {e}"),
            ForgeError::VerusAbsent { binary } => write!(
                f,
                "the `{binary}` verifier was not found on PATH (environment error, not a \
                 verification failure); install verus or set it on PATH"
            ),
            ForgeError::VerusSpawn { source } => write!(f, "failed to spawn verus: {source}"),
            ForgeError::VerusOutput { detail } => {
                write!(f, "could not interpret verus output: {detail}")
            }
            ForgeError::Io { path, source } => write!(f, "io error at `{path}`: {source}"),
            ForgeError::Usage(msg) => write!(f, "usage error: {msg}"),
        }
    }
}

impl std::error::Error for ForgeError {}

impl ForgeError {
    /// The exit code class for this error (REQ-5). Every `ForgeError` is an
    /// environment/usage/IO outcome — a verification FAILURE is NOT a
    /// `ForgeError` (it is a reported certificate). So every variant maps to
    /// [`EXIT_ENVIRONMENT`].
    fn exit_code(&self) -> u8 {
        EXIT_ENVIRONMENT
    }
}

/// The parsed command (REQ-1/REQ-2).
#[derive(Debug, PartialEq, Eq)]
enum Command {
    /// `forge new <name>`.
    New { name: String },
    /// `forge check [<file>] [--json]`.
    Check { file: PathBuf, json: bool },
}

/// Parse `argv[1..]` (the arguments after the program name) into a [`Command`]
/// (REQ-2 — hand-rolled, no derive macro). The v0.1 grammar is
/// `new <name>` | `check <file> [--json]`. An unknown verb, a missing
/// positional, or an unexpected flag is a `ForgeError::Usage`, never a panic.
fn parse_args(args: &[String]) -> Result<Command, ForgeError> {
    let mut iter = args.iter();
    let verb = iter
        .next()
        .ok_or_else(|| ForgeError::Usage(usage_text().to_string()))?;
    match verb.as_str() {
        "new" => {
            let name = iter
                .next()
                .ok_or_else(|| ForgeError::Usage("`forge new` requires a <name>".to_string()))?;
            if let Some(extra) = iter.next() {
                return Err(ForgeError::Usage(format!(
                    "`forge new` takes exactly one <name>; unexpected `{extra}`"
                )));
            }
            Ok(Command::New {
                name: name.to_string(),
            })
        }
        "check" => {
            let mut file: Option<PathBuf> = None;
            let mut json = false;
            for arg in iter {
                match arg.as_str() {
                    "--json" => json = true,
                    flag if flag.starts_with("--") => {
                        return Err(ForgeError::Usage(format!("unknown flag `{flag}`")));
                    }
                    positional => {
                        if file.is_some() {
                            return Err(ForgeError::Usage(format!(
                                "`forge check` takes at most one <file>; unexpected `{positional}`"
                            )));
                        }
                        file = Some(PathBuf::from(positional));
                    }
                }
            }
            let file = file.ok_or_else(|| {
                ForgeError::Usage(
                    "`forge check` requires a <file> in v0.1 (no project-default item yet)"
                        .to_string(),
                )
            })?;
            Ok(Command::Check { file, json })
        }
        other => Err(ForgeError::Usage(format!(
            "unknown command `{other}`. {}",
            usage_text()
        ))),
    }
}

/// The usage banner (REQ-1: the v0.1 verb subset only).
fn usage_text() -> &'static str {
    "usage: forge new <name> | forge check <file> [--json]"
}

/// The entry boundary (`.design/forge/cli.md` Architecture): reads `argv`,
/// dispatches, renders, and maps the outcome to an `ExitCode` (REQ-5). This is
/// the ONLY function that touches `std::env::args` / `ExitCode`.
pub fn run() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match dispatch(&args) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("forge: {e}");
            ExitCode::from(e.exit_code())
        }
    }
}

/// Dispatch a parsed command, returning the process exit code (REQ-5). Split
/// from [`run`] so it is unit-testable without touching the real `argv`.
fn dispatch(args: &[String]) -> Result<ExitCode, ForgeError> {
    match parse_args(args)? {
        Command::New { name } => {
            scaffold_project(Path::new(&name))?;
            println!("created Thermite project `{name}`");
            Ok(ExitCode::SUCCESS)
        }
        Command::Check { file, json } => run_check(&file, json),
    }
}

/// Run `forge check`: drive the pipeline, render every certificate, and map the
/// aggregate outcome to an exit code (REQ-4/REQ-5). Diagnostics go to stderr so
/// the `--json` stdout is a single clean machine-parseable document (AC-2).
fn run_check(file: &Path, json: bool) -> Result<ExitCode, ForgeError> {
    let certs = check::check_file(file)?;

    if json {
        // One JSON document on stdout: the array of certificates. Nothing else
        // goes to stdout under --json.
        let doc = serde_json::to_string_pretty(&certs).map_err(|e| ForgeError::VerusOutput {
            detail: format!("failed to serialize certificate JSON: {e}"),
        })?;
        println!("{doc}");
    } else {
        for cert in &certs {
            print!("{}", render_human(cert));
        }
    }

    // Aggregate outcome (REQ-5): all items L3 → success; any non-L3 → reported
    // verification failure.
    let all_l3 = certs.iter().all(|c| c.level == Level::L3);
    if all_l3 {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(EXIT_VERIFICATION_FAILURE))
    }
}

/// Render a [`Certificate`] as human-readable text (REQ-4, §5.1 "rendered to
/// readable text"). The §5.1 structured JSON is the `--json` rendering; this is
/// the default.
fn render_human(cert: &Certificate) -> String {
    // The deterministic oracle-stable subset (manifest::Certificate::oracle_subset):
    // item / level / effects / slag — the fields the cert-oracle compares — are
    // rendered first, then the non-deterministic `solver_time_ms` labelled as
    // such so a reader does not mistake it for an oracle field.
    let (item, level, effects, slag) = cert.oracle_subset();
    let mut out = String::new();
    out.push_str(&format!("item: {item}\n"));
    out.push_str(&format!("level: {}\n", level_str(level)));
    out.push_str(&format!("effects: [{}]\n", effects.join(", ")));
    out.push_str(&format!("slag: {slag}\n"));
    out.push_str(&format!(
        "solver_time_ms: {} (non-deterministic; not part of the cert oracle)\n",
        cert.solver_time_ms
    ));
    out.push_str("obligations:\n");
    for ob in &cert.obligations {
        match ob.status {
            ObligationStatus::Discharged => {
                out.push_str(&format!("  [ok] {}\n", ob.name));
            }
            ObligationStatus::Failed => {
                let loc = ob
                    .location
                    .as_deref()
                    .map(|l| format!(" @ {l}"))
                    .unwrap_or_default();
                out.push_str(&format!("  [FAIL] {}{loc}\n", ob.name));
                if let Some(d) = &ob.diagnostic {
                    out.push_str(&format!("         {d}\n"));
                }
            }
        }
    }
    out
}

/// The string form of a [`Level`] for human output (`"L3"` etc.).
fn level_str(level: Level) -> &'static str {
    match level {
        Level::L0 => "L0",
        Level::L1 => "L1",
        Level::L2 => "L2",
        Level::L3 => "L3",
    }
}

/// `forge new <name>` (REQ-7): create a minimal v0.1 project skeleton — a
/// manifest, a lockfile carrying the pinned solver seed (§5.3), and a skill pin
/// (Appendix B). Refuses to overwrite a non-empty target (a structured error,
/// not a clobber).
pub fn scaffold_project(target: &Path) -> Result<(), ForgeError> {
    if target.exists() {
        let non_empty = target.is_file()
            || std::fs::read_dir(target)
                .map(|mut d| d.next().is_some())
                .unwrap_or(true);
        if non_empty {
            return Err(ForgeError::Usage(format!(
                "`{}` already exists and is not empty; refusing to overwrite",
                target.display()
            )));
        }
    }
    std::fs::create_dir_all(target).map_err(|e| ForgeError::Io {
        path: target.display().to_string(),
        source: e,
    })?;

    let name = target
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("project");

    // Manifest (project config schema — distinct from the per-item certificate
    // schema in `manifest.rs`).
    write_file(
        &target.join("forge.toml"),
        &format!("[project]\nname = \"{name}\"\nedition = \"v0.1\"\n"),
    )?;
    // Lockfile: the pinned solver seed (§5.3) `check.rs` feeds verus, so
    // determinism is project-scoped (R-CODE-5).
    write_file(
        &target.join("forge.lock"),
        &format!("[solver]\nseed = {DEFAULT_SOLVER_SEED}\n"),
    )?;
    // Skill pin (Appendix B).
    write_file(
        &target.join("THERMITE.skill.pin"),
        "# pin the THERMITE.skill.md version this project was authored against\nskill = \"v0.1\"\n",
    )?;
    Ok(())
}

/// Write `contents` to `path`, mapping IO failure to `ForgeError::Io`.
fn write_file(path: &Path, contents: &str) -> Result<(), ForgeError> {
    std::fs::write(path, contents).map_err(|e| ForgeError::Io {
        path: path.display().to_string(),
        source: e,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| s.to_string()).collect()
    }

    // REQ-2: verb dispatch + the --json flag + positional.
    #[test]
    fn parses_new_and_check() {
        assert_eq!(
            parse_args(&argv(&["new", "proj"])).expect("new"),
            Command::New {
                name: "proj".to_string()
            }
        );
        assert_eq!(
            parse_args(&argv(&["check", "a.th"])).expect("check"),
            Command::Check {
                file: PathBuf::from("a.th"),
                json: false
            }
        );
        assert_eq!(
            parse_args(&argv(&["check", "a.th", "--json"])).expect("check json"),
            Command::Check {
                file: PathBuf::from("a.th"),
                json: true
            }
        );
    }

    // AC-1: no args / unknown verb / missing positional → Usage error, never a
    // panic and never exit 0.
    #[test]
    fn usage_errors() {
        assert!(matches!(parse_args(&argv(&[])), Err(ForgeError::Usage(_))));
        assert!(matches!(
            parse_args(&argv(&["frobnicate"])),
            Err(ForgeError::Usage(_))
        ));
        assert!(matches!(
            parse_args(&argv(&["new"])),
            Err(ForgeError::Usage(_))
        ));
        assert!(matches!(
            parse_args(&argv(&["check"])),
            Err(ForgeError::Usage(_))
        ));
        assert!(matches!(
            parse_args(&argv(&["check", "a.th", "--bogus"])),
            Err(ForgeError::Usage(_))
        ));
    }

    // AC-5: every wrapping variant forwards its inner error's diagnostic — no
    // information lost at the boundary (R-CODE-4 "never swallow").
    #[test]
    fn aggregation_preserves_inner_diagnostics() {
        // Drive a real parse error through thermite_syntax so the wrapped
        // SyntaxError's Display text survives into ForgeError's Display.
        let parsed = thermite_syntax::parse("fn (");
        assert!(!parsed.is_clean(), "`fn (` must be a parse error");
        let inner_text = parsed
            .errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>();
        let wrapped = ForgeError::Parse(parsed.errors);
        let shown = wrapped.to_string();
        for t in &inner_text {
            assert!(
                shown.contains(t.as_str()),
                "wrapped Parse error must forward inner `{t}`:\n{shown}"
            );
        }
    }

    // REQ-5: every ForgeError maps to the environment exit code (a verification
    // FAILURE is a cert, not a ForgeError).
    #[test]
    fn errors_map_to_environment_exit_code() {
        let e = ForgeError::VerusAbsent {
            binary: "verus".to_string(),
        };
        assert_eq!(e.exit_code(), EXIT_ENVIRONMENT);
        let e = ForgeError::Usage("x".to_string());
        assert_eq!(e.exit_code(), EXIT_ENVIRONMENT);
    }

    // REQ-7: scaffold layout + no-clobber.
    #[test]
    fn scaffold_writes_layout_and_refuses_clobber() {
        let dir = std::env::temp_dir().join(format!("forge_scaffold_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        scaffold_project(&dir).expect("scaffold");
        assert!(dir.join("forge.toml").exists());
        assert!(dir.join("forge.lock").exists());
        assert!(dir.join("THERMITE.skill.pin").exists());
        let lock = std::fs::read_to_string(dir.join("forge.lock")).expect("read lock");
        assert!(lock.contains("seed ="), "lockfile pins the solver seed");
        // No-clobber: a second scaffold over the now-non-empty dir is a Usage err.
        assert!(matches!(scaffold_project(&dir), Err(ForgeError::Usage(_))));
        let _ = std::fs::remove_dir_all(&dir);
    }

    // REQ-4 human rendering: failed obligation shows location + diagnostic.
    #[test]
    fn human_render_shows_failure_counterexample() {
        use crate::manifest::ObligationResult;
        let cert = Certificate::new(
            "add_one",
            Level::L0,
            vec!["pure".to_string()],
            5,
            vec![ObligationResult::failed(
                "postcondition not satisfied",
                Some("broken_check.rs:5:13".to_string()),
                Some("error: postcondition not satisfied".to_string()),
            )],
        );
        let text = render_human(&cert);
        assert!(text.contains("level: L0"));
        assert!(text.contains("[FAIL] postcondition not satisfied @ broken_check.rs:5:13"));
    }
}
