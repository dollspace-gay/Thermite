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
//! | REQ-2 (hand-rolled arg parsing) | SHIPPED | `parse_args` is a `match` over the verb + positionals + the `--json` / `--level` / `--rlimit` flags; no `clap` dependency in `forge/Cargo.toml`. The #11 `--rlimit <FLOAT>` flag tunes the verus SMT resource budget (default `check::DEFAULT_RLIMIT`); a low value forces the timeout path (`run_check` → `check::check_file_with_rlimit`). |
//! | REQ-3 (`ForgeError` aggregation) | SHIPPED | `enum ForgeError` wraps `Vec<SyntaxError>`/`Vec<SpecError>`/`Vec<LowerError>`/`LowerError` and carries `VerusAbsent`/`VerusSpawn`/`VerusOutput`/`Io`/`Usage`; `Display` forwards each inner error's diagnostic (no information lost). |
//! | REQ-4 (human + `--json` output) | SHIPPED | `render_human` / `serde_json::to_string_pretty` of the cert array; `run_check` picks the rendering from `--json`; diagnostics go to stderr so `--json` stdout is a clean document. |
//! | REQ-5 (typed exit codes) | SHIPPED | `run_check` maps the #10 `AssuranceManifest::aggregate` headline to an `ExitCode`: `ProjectAssurance::Certified(_)` (every item certified at `L3`/`L2`/`L1` with no `reject`, via `manifest::cert_certifies`) → 0; `ProjectAssurance::Failed` (any #6 triage / slag reject, un-discharged proof, or counterexample) → `EXIT_VERIFICATION_FAILURE`; environment/usage/IO → `EXIT_ENVIRONMENT`. |
//!
//! ## #10 gate (the project assurance display, this iteration)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | degrade-ladder REQ-5/REQ-6 (display the project assurance) | SHIPPED | `run_check` computes `manifest::AssuranceManifest::aggregate(&certs)` and `render_assurance` prints the project headline (the min-over-functions, or `FAILED` when any fn does not certify) + the per-fn `lowered-assurance` flags (§5.2 "displayed on every build"). The headline also drives the exit code (REQ-5). |
//! | REQ-6 (no panics; Result discipline) | SHIPPED | every fallible path returns `Result<_, ForgeError>`; no `unwrap`/`expect`/`panic!` outside `#[cfg(test)]`; verus exit status inspected in `check.rs`. |
//! | REQ-7 (`forge new` scaffold) | SHIPPED | `scaffold_project` writes `forge.toml` + `forge.lock` (pinned seed, §5.3) + `THERMITE.skill.pin`; refuses a non-empty target (`ForgeError::Usage`). |

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use thermite_lower::LowerError;
use thermite_spec::SpecError;
use thermite_syntax::SyntaxError;

use crate::audit::{self, AuditManifest};
use crate::build::{self, BuildManifest, CrateType};
use crate::check::{self, CheckOptions, DEFAULT_RLIMIT, DEFAULT_SOLVER_SEED};
use crate::manifest::{
    AssuranceManifest, AssuranceScope, Certificate, Level, ObligationStatus, ProjectAssurance,
    ProjectScope,
};
use crate::mutation::MUTATION_FLOOR;
use crate::repair::{self, RepairItem, RepairOutcome, RepairReport};
use crate::review::{self, ReviewArtifact};

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
    /// The `cargo kani` / kani binary was not found on `PATH` — an ENVIRONMENT
    /// error, NOT a verification failure (`.design/lower/l2-kani.md` REQ-8). The
    /// L2 parallel of `VerusAbsent`.
    KaniAbsent { binary: String },
    /// Spawning kani failed for a reason other than absence (e.g. permission).
    /// The L2 parallel of `VerusSpawn`.
    KaniSpawn { source: std::io::Error },
    /// Kani ran but its output could not be parsed into a verification summary,
    /// or it reported a reachable unsupported construct / internal failure
    /// (never swallowed, `.design/lower/l2-kani.md` REQ-5 / R-CODE-4). The L2
    /// parallel of `VerusOutput`.
    KaniOutput { detail: String },
    /// The `rustc` compiler was not found on `PATH` — an ENVIRONMENT error, NOT a
    /// verification/build failure (`.design/forge/build.md` REQ-2). The `forge
    /// build` parallel of `VerusAbsent`.
    RustcAbsent { binary: String },
    /// Spawning `rustc` failed for a reason other than absence (e.g. permission).
    /// The `forge build` parallel of `VerusSpawn`.
    RustcSpawn { source: std::io::Error },
    /// `rustc` ran but exited NON-ZERO (a real lowering/codegen failure, not a
    /// runtime contract violation — a violating body still COMPILES), or produced
    /// no version string. Its stderr is surfaced (never swallowed, R-CODE-4 /
    /// `.design/forge/build.md` REQ-2 / AC-7). The `forge build` parallel of
    /// `VerusOutput`.
    RustcOutput { detail: String },
    /// An IO error reading a source file or writing a scaffold/temp file.
    Io {
        path: String,
        source: std::io::Error,
    },
    /// The `--reviewer <cmd>` external reviewer command was not found (`ENOENT`) —
    /// an ENVIRONMENT error (issue #19; `.design/forge/spec-review.md` REQ-7,
    /// OQ-1). The spec-intent verdict is the EXTERNAL reviewer's; an absent
    /// reviewer is reported, never a panic and never a fabricated `aligned`.
    ReviewerAbsent { cmd: String },
    /// Spawning the `--reviewer <cmd>` failed for a reason other than absence, or
    /// writing the artifact to its stdin failed (issue #19). The reviewer parallel
    /// of `VerusSpawn`.
    ReviewerSpawn { cmd: String, source: std::io::Error },
    /// The `--reviewer <cmd>` ran but exited NON-ZERO (issue #19). Its stderr is
    /// surfaced (never swallowed, R-CODE-4); forge does NOT fabricate a verdict.
    ReviewerFailed {
        cmd: String,
        code: Option<i32>,
        stderr: String,
    },
    /// The `--reviewer <cmd>` ran but its stdout was missing / not a parseable
    /// `ReviewVerdict` (issue #19). Reported (R-CODE-4), never a crash and never a
    /// fabricated `aligned`.
    ReviewerOutput { detail: String },
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
            ForgeError::KaniAbsent { binary } => write!(
                f,
                "the `{binary}` bounded model checker was not found on PATH (environment error, \
                 not a verification failure); install kani (`cargo install --locked kani-verifier \
                 && cargo kani setup`) or set it on PATH"
            ),
            ForgeError::KaniSpawn { source } => write!(f, "failed to spawn kani: {source}"),
            ForgeError::KaniOutput { detail } => {
                write!(f, "could not interpret kani output: {detail}")
            }
            ForgeError::RustcAbsent { binary } => write!(
                f,
                "the `{binary}` compiler was not found on PATH (environment error, not a build \
                 failure); install the Rust toolchain or set rustc on PATH"
            ),
            ForgeError::RustcSpawn { source } => write!(f, "failed to spawn rustc: {source}"),
            ForgeError::RustcOutput { detail } => {
                write!(f, "rustc failed to build the lowered artifact: {detail}")
            }
            ForgeError::Io { path, source } => write!(f, "io error at `{path}`: {source}"),
            ForgeError::ReviewerAbsent { cmd } => write!(
                f,
                "the `--reviewer` command `{cmd}` was not found (environment error, not a \
                 verification failure); the spec-intent verdict is the external reviewer's — \
                 install/correct the command or run `forge review` without `--reviewer` to emit \
                 the artifact for a manual reviewer"
            ),
            ForgeError::ReviewerSpawn { cmd, source } => {
                write!(
                    f,
                    "failed to run the `--reviewer` command `{cmd}`: {source}"
                )
            }
            ForgeError::ReviewerFailed { cmd, code, stderr } => write!(
                f,
                "the `--reviewer` command `{cmd}` exited with status {code:?} (no verdict \
                 attached; forge never fabricates a spec-intent verdict); stderr: {stderr}"
            ),
            ForgeError::ReviewerOutput { detail } => {
                write!(f, "could not read a reviewer verdict: {detail}")
            }
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

/// The parsed command (REQ-1/REQ-2). Drops `Eq` because `Check.rlimit` is an
/// `f64` (the verus resource budget, #11) — `PartialEq` suffices for the
/// arg-parsing unit tests' `assert_eq!`.
#[derive(Debug, PartialEq)]
enum Command {
    /// `forge new <name>`.
    New { name: String },
    /// `forge check [<file>] [--json] [--level l2|l3] [--rlimit <FLOAT>] [--mutation-floor <FLOAT>]`.
    Check {
        file: PathBuf,
        json: bool,
        level: CheckLevel,
        /// The verus `--rlimit` (SMT resource budget, roughly seconds) for the
        /// L3 path (#11; `.design/forge/solver-profiles.md` REQ-5). Defaults to
        /// the generous pinned [`DEFAULT_RLIMIT`]; a LOW value forces the timeout
        /// path so the three-way classification is testable.
        rlimit: f64,
        /// The mutation kill-ratio floor (#12; `.design/forge/mutation-scoring.md`
        /// REQ-5). Defaults to [`MUTATION_FLOOR`] (0.60); an item that proves L3
        /// but scores below this floor does NOT certify (`WeakContract` reject). A
        /// LOW value (e.g. `0.2`) flips a weak contract back to certified (AC-3).
        mutation_floor: f64,
    },
    /// `forge audit <file> [--json]` — emit the project AUDIT MANIFEST v1 (issue
    /// #15; `.design/forge/audit-manifest.md` REQ-2). Runs the SAME check pipeline
    /// `forge check` runs at the pinned default config (no extra verification),
    /// aggregates the cert collection into an `AuditManifest`, and emits it as the
    /// stable `--json` document or a human summary. The default-config path is the
    /// reproducible trust statement (OQ-3).
    Audit { file: PathBuf, json: bool },
    /// `forge repair <file> [item]` — the background L1/L2 → L3 upgrade loop
    /// (issue #18; `.design/forge/proof-repair.md` REQ-1). Re-derives the per-item
    /// certs at the default budget, finds the SUB-L3 items, and for a TIMEOUT item
    /// ONLY escalates the verus `--rlimit` along the frozen bounded ladder
    /// (`repair::REPAIR_LADDER`) to try to recover L3 — NEVER retrying a
    /// counterexample / reject (the anti-cheat, REQ-2). A one-shot re-runnable pass
    /// (OQ-4: daemon/orchestration is #20). The optional `item` restricts repair to
    /// a single function.
    Repair {
        file: PathBuf,
        item: Option<String>,
        json: bool,
    },
    /// `forge review <file> [item] [--json] [--reviewer <cmd>]` — the PLUGGABLE
    /// SPEC-INTENT REVIEW SLOT (issue #19; `.design/forge/spec-review.md` REQ-7,
    /// §7 line 227). Runs the SAME default-config check pipeline `forge check` /
    /// `forge audit` run (the battery verdict — no extra verification), extracts
    /// the PRE-SCREENED declarative spec layer (per battery-passing fn: `req`/`ens`/
    /// `fx` + the directly-referenced `spec fn` declarations, NO bodies) + an "is
    /// this what you meant?" prompt, and emits the artifact (`--json` machine form
    /// or human). An OPTIONAL `[item]` restricts the artifact to one fn. With
    /// `--reviewer <cmd>` it pipes the artifact to the EXTERNAL reviewer's stdin,
    /// reads the `ReviewVerdict` JSON from its stdout, and writes a separate
    /// `<file>.review.json` record (forge NEVER fabricates `aligned` — OQ-1/OQ-2).
    Review {
        file: PathBuf,
        item: Option<String>,
        json: bool,
        reviewer: Option<String>,
    },
    /// `forge build <file> [--entry <fn>] [--json]` — lower a Thermite program to
    /// executable Rust and compile it with `rustc` into a contract-checked artifact
    /// (issue #56; `.design/forge/build.md` REQ-1). Default → a compiled library
    /// (`rlib`); `--entry <fn>` → a runnable executable whose generated `main`
    /// calls `fn` with deterministic synthesized inputs (REQ-3), so the always-
    /// active `thermite_check!`s are observable at runtime (the #57 hook).
    Build {
        file: PathBuf,
        entry: Option<String>,
        json: bool,
    },
}

/// The assurance rung `forge check` targets (`.design/lower/l2-kani.md` REQ-7,
/// OQ-1: the `--level l2` flag). The DEFAULT stays `L3` (the verus path); `--level
/// l2` is an EXPLICIT choice that runs the Kani bounded model check INSTEAD —
/// never an automatic degrade (that is #10).
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum CheckLevel {
    /// The default: the verus SMT proof path (`check::check_file`).
    L3,
    /// The Kani bounded model check path (`check::check_l2_file`).
    L2,
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
            let mut level = CheckLevel::L3;
            let mut rlimit = DEFAULT_RLIMIT;
            let mut mutation_floor = MUTATION_FLOOR;
            let mut iter = iter.peekable();
            while let Some(arg) = iter.next() {
                match arg.as_str() {
                    "--json" => json = true,
                    "--rlimit" => {
                        // `--rlimit <FLOAT>` — the verus SMT resource budget (#11;
                        // `.design/forge/solver-profiles.md` REQ-5). The value is a
                        // separate token; a missing or non-numeric value is a Usage
                        // error, never a silent default (the test lever that forces
                        // the timeout path uses a LOW value).
                        let value = iter.next().ok_or_else(|| {
                            ForgeError::Usage(
                                "`--rlimit` requires a FLOAT value (the verus SMT resource budget)"
                                    .to_string(),
                            )
                        })?;
                        rlimit = value.parse::<f64>().map_err(|_| {
                            ForgeError::Usage(format!("`--rlimit` value `{value}` is not a number"))
                        })?;
                        if !(rlimit.is_finite() && rlimit > 0.0) {
                            return Err(ForgeError::Usage(format!(
                                "`--rlimit` must be a finite positive number (got `{value}`); \
                                 verus rejects rlimit <= 0"
                            )));
                        }
                    }
                    "--mutation-floor" => {
                        // `--mutation-floor <FLOAT>` — the §7 step-4 kill-ratio floor
                        // (#12; `.design/forge/mutation-scoring.md` REQ-5). The value
                        // is a separate token; a missing / non-numeric / out-of-[0,1]
                        // value is a Usage error, never a silent default. A LOW value
                        // (e.g. `0.2`) flips a weak contract back to certified (AC-3).
                        let value = iter.next().ok_or_else(|| {
                            ForgeError::Usage(
                                "`--mutation-floor` requires a FLOAT value (the §7 kill-ratio \
                                 floor, 0.0..=1.0)"
                                    .to_string(),
                            )
                        })?;
                        mutation_floor = value.parse::<f64>().map_err(|_| {
                            ForgeError::Usage(format!(
                                "`--mutation-floor` value `{value}` is not a number"
                            ))
                        })?;
                        if !(mutation_floor.is_finite() && (0.0..=1.0).contains(&mutation_floor)) {
                            return Err(ForgeError::Usage(format!(
                                "`--mutation-floor` must be a finite ratio in 0.0..=1.0 (got \
                                 `{value}`)"
                            )));
                        }
                    }
                    "--level" => {
                        // `--level l2|l3` — an EXPLICIT rung choice (REQ-7). The
                        // value is a separate token (`--level l2`); a missing or
                        // unknown value is a Usage error, never a silent default.
                        let value = iter.next().ok_or_else(|| {
                            ForgeError::Usage(
                                "`--level` requires a value (`l2` or `l3`)".to_string(),
                            )
                        })?;
                        level = match value.as_str() {
                            "l2" | "L2" => CheckLevel::L2,
                            "l3" | "L3" => CheckLevel::L3,
                            other => {
                                return Err(ForgeError::Usage(format!(
                                    "unknown `--level` value `{other}` (expected `l2` or `l3`)"
                                )));
                            }
                        };
                    }
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
            Ok(Command::Check {
                file,
                json,
                level,
                rlimit,
                mutation_floor,
            })
        }
        "audit" => {
            // `forge audit <file> [--json]` (#15; `.design/forge/audit-manifest.md`
            // REQ-2). The canonical audit deliverable runs at the pinned default
            // config (OQ-3 — the reproducible trust statement), so this verb takes
            // ONLY the file + `--json`; the exploratory `--rlimit`/`--mutation-floor`
            // levers are NOT exposed here (the default-config path is the contract).
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
                                "`forge audit` takes at most one <file>; unexpected `{positional}`"
                            )));
                        }
                        file = Some(PathBuf::from(positional));
                    }
                }
            }
            let file = file
                .ok_or_else(|| ForgeError::Usage("`forge audit` requires a <file>".to_string()))?;
            Ok(Command::Audit { file, json })
        }
        "repair" => {
            // `forge repair <file> [item] [--json]` (#18;
            // `.design/forge/proof-repair.md` REQ-1). The first positional is the
            // file (required); an OPTIONAL second positional restricts repair to a
            // single item. Like `forge audit`, it runs at the pinned default budget
            // (the exploratory `--rlimit`/`--mutation-floor` levers are NOT exposed
            // — the ESCALATION ladder is the frozen `repair::REPAIR_LADDER`, REQ-3).
            let mut file: Option<PathBuf> = None;
            let mut item: Option<String> = None;
            let mut json = false;
            for arg in iter {
                match arg.as_str() {
                    "--json" => json = true,
                    flag if flag.starts_with("--") => {
                        return Err(ForgeError::Usage(format!("unknown flag `{flag}`")));
                    }
                    positional => {
                        if file.is_none() {
                            file = Some(PathBuf::from(positional));
                        } else if item.is_none() {
                            item = Some(positional.to_string());
                        } else {
                            return Err(ForgeError::Usage(format!(
                                "`forge repair` takes at most <file> [item]; unexpected \
                                 `{positional}`"
                            )));
                        }
                    }
                }
            }
            let file = file.ok_or_else(|| {
                ForgeError::Usage("`forge repair` requires a <file> [item]".to_string())
            })?;
            Ok(Command::Repair { file, item, json })
        }
        "review" => {
            // `forge review <file> [item] [--json] [--reviewer <cmd>]` (#19;
            // `.design/forge/spec-review.md` REQ-7). The first positional is the
            // file (required); an OPTIONAL second positional restricts the artifact
            // to a single item. Like `forge audit`, the EXTRACTION runs at the
            // pinned default budget (the exploratory `--rlimit`/`--mutation-floor`
            // levers are NOT exposed — the §7 "the certificate includes the spec
            // layer" framing). `--reviewer <cmd>` names the external reviewer
            // command (its value is a separate token; a missing value is a Usage
            // error). Without it, forge emits only the artifact (the reviewer is
            // external/manual).
            let mut file: Option<PathBuf> = None;
            let mut item: Option<String> = None;
            let mut json = false;
            let mut reviewer: Option<String> = None;
            let mut iter = iter.peekable();
            while let Some(arg) = iter.next() {
                match arg.as_str() {
                    "--json" => json = true,
                    "--reviewer" => {
                        let value = iter.next().ok_or_else(|| {
                            ForgeError::Usage(
                                "`--reviewer` requires a <cmd> value (the external reviewer \
                                 command the artifact is piped to)"
                                    .to_string(),
                            )
                        })?;
                        reviewer = Some(value.to_string());
                    }
                    flag if flag.starts_with("--") => {
                        return Err(ForgeError::Usage(format!("unknown flag `{flag}`")));
                    }
                    positional => {
                        if file.is_none() {
                            file = Some(PathBuf::from(positional));
                        } else if item.is_none() {
                            item = Some(positional.to_string());
                        } else {
                            return Err(ForgeError::Usage(format!(
                                "`forge review` takes at most <file> [item]; unexpected \
                                 `{positional}`"
                            )));
                        }
                    }
                }
            }
            let file = file.ok_or_else(|| {
                ForgeError::Usage(
                    "`forge review` requires a <file> [item] [--reviewer <cmd>]".to_string(),
                )
            })?;
            Ok(Command::Review {
                file,
                item,
                json,
                reviewer,
            })
        }
        "build" => {
            // `forge build <file> [--entry <fn>] [--json]` (#56;
            // `.design/forge/build.md` REQ-1/REQ-3). The first positional is the
            // file (required in v0.1 — no project-default item yet). `--entry <fn>`
            // names the fn the generated deterministic runner exercises (its value
            // is a separate token; a missing value is a Usage error); without it the
            // default library (`rlib`) artifact is produced.
            let mut file: Option<PathBuf> = None;
            let mut entry: Option<String> = None;
            let mut json = false;
            let mut iter = iter.peekable();
            while let Some(arg) = iter.next() {
                match arg.as_str() {
                    "--json" => json = true,
                    "--entry" => {
                        let value = iter.next().ok_or_else(|| {
                            ForgeError::Usage(
                                "`--entry` requires a <fn> value (the entry point the generated \
                                 runner calls)"
                                    .to_string(),
                            )
                        })?;
                        entry = Some(value.to_string());
                    }
                    flag if flag.starts_with("--") => {
                        return Err(ForgeError::Usage(format!("unknown flag `{flag}`")));
                    }
                    positional => {
                        if file.is_some() {
                            return Err(ForgeError::Usage(format!(
                                "`forge build` takes at most one <file>; unexpected `{positional}`"
                            )));
                        }
                        file = Some(PathBuf::from(positional));
                    }
                }
            }
            let file = file.ok_or_else(|| {
                ForgeError::Usage(
                    "`forge build` requires a <file> [--entry <fn>] in v0.1".to_string(),
                )
            })?;
            Ok(Command::Build { file, entry, json })
        }
        other => Err(ForgeError::Usage(format!(
            "unknown command `{other}`. {}",
            usage_text()
        ))),
    }
}

/// The usage banner (REQ-1: the v0.1 verb subset only).
fn usage_text() -> &'static str {
    "usage: forge new <name> | forge check <file> [--json] [--level l2|l3] [--rlimit <FLOAT>] \
     [--mutation-floor <FLOAT>] | forge audit <file> [--json] | forge repair <file> [item] [--json] \
     | forge review <file> [item] [--json] [--reviewer <cmd>] | forge build <file> [--entry <fn>] \
     [--json]"
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
        Command::Check {
            file,
            json,
            level,
            rlimit,
            mutation_floor,
        } => run_check(&file, json, level, rlimit, mutation_floor),
        Command::Audit { file, json } => run_audit(&file, json),
        Command::Repair { file, item, json } => run_repair(&file, item.as_deref(), json),
        Command::Review {
            file,
            item,
            json,
            reviewer,
        } => run_review(&file, item.as_deref(), json, reviewer.as_deref()),
        Command::Build { file, entry, json } => run_build(&file, entry.as_deref(), json),
    }
}

/// Run `forge check`: drive the pipeline, render every certificate, and map the
/// aggregate outcome to an exit code (REQ-4/REQ-5). Diagnostics go to stderr so
/// the `--json` stdout is a single clean machine-parseable document (AC-2).
fn run_check(
    file: &Path,
    json: bool,
    level: CheckLevel,
    rlimit: f64,
    mutation_floor: f64,
) -> Result<ExitCode, ForgeError> {
    // The DEFAULT (no flag) stays the L3 verus path; `--level l2` is an EXPLICIT
    // choice that runs the Kani bounded model check instead — never an automatic
    // degrade (`.design/lower/l2-kani.md` REQ-7; #10 owns the auto-degrade). The
    // `--rlimit` (#11) tunes the L3 verus resource budget; the L2 Kani path does
    // not consume it.
    let certs = match level {
        // The canonical config (default rlimit + default mutation floor #12) routes
        // through `check_file` (the public default entry, the only one that serves /
        // populates the shared proof cache). An explicit `--rlimit` (#11, the
        // timeout-forcing lever) or `--mutation-floor` (#12, the AC-3 floor-flip
        // lever) routes through `check_file_with_options` (cache-bypassed).
        CheckLevel::L3 if rlimit == DEFAULT_RLIMIT && mutation_floor == MUTATION_FLOOR => {
            check::check_file(file)?
        }
        CheckLevel::L3 => check::check_file_with_options(
            file,
            CheckOptions {
                rlimit,
                mutation_floor,
            },
        )?,
        CheckLevel::L2 => check::check_l2_file(file)?,
    };

    // #10 the project-level ASSURANCE MANIFEST (`.design/forge/degrade-ladder.md`
    // REQ-5/REQ-6, OQ-4 reading (b) — a render-time aggregate over the per-fn cert
    // collection, NOT a separately-materialized schema object). The headline is the
    // MIN over functions (a single L1 fn caps the project at L1; a single
    // hard-failed fn is a project FAILURE). Computed for both renderings.
    let manifest = AssuranceManifest::aggregate(&certs);

    if json {
        // One JSON document on stdout: the array of certificates. Nothing else
        // goes to stdout under --json (the per-cert `lowered_assurance` flag is in
        // each cert; the project headline is a derived display, not a schema field).
        let doc = serde_json::to_string_pretty(&certs).map_err(|e| ForgeError::VerusOutput {
            detail: format!("failed to serialize certificate JSON: {e}"),
        })?;
        println!("{doc}");
    } else {
        for cert in &certs {
            print!("{}", render_human(cert));
        }
        // #10: the project assurance headline + per-fn lowered-assurance flags
        // (§5.2 "displayed on every build"). Goes to stdout (the human document).
        print!("{}", render_assurance(&manifest));
    }

    // Aggregate outcome (REQ-5): every item must CERTIFY. An item certifies iff
    // it carries no `reject` cause AND its level is a certified rung — `L3` (the
    // verus path), `L2` (a bounded check, #9/#10 degrade), OR `L1` (a valid
    // `#[slag]` item / #10 degrade). A `#6` triage / slag-validation reject
    // (`Level::L0` + a `reject` cause) is a reported contract-certification
    // FAILURE — non-zero, but a valid cert document on stdout (verdict-in-cert).
    // The #10 assurance aggregate's `Failed` headline and this all-certified check
    // agree (both use `manifest::cert_certifies`).
    let all_certified = matches!(manifest.project, ProjectAssurance::Certified(_));
    if all_certified {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(EXIT_VERIFICATION_FAILURE))
    }
}

/// Run `forge audit`: emit the project AUDIT MANIFEST v1 (#15;
/// `.design/forge/audit-manifest.md` REQ-2). Runs the SAME check pipeline
/// `forge check` runs at the pinned default config (`CheckOptions::default` via
/// `check::check_file` — NO extra verification, NO re-derivation), parses the file
/// once for the boundary contracts' enforced `req`/`ens`/`fx` (the cert carries
/// only the target), resolves the toolchain identity, builds the
/// [`AuditManifest`] (a pure projection), and emits it as the stable `--json`
/// document or a human summary (OQ-1 — the JSON is the oracle-asserted surface).
/// The exit code mirrors `forge check`'s project headline (REQ-5): a fully-
/// certified project exits 0, else a verification-failure exit.
fn run_audit(file: &Path, json: bool) -> Result<ExitCode, ForgeError> {
    // The SAME default pipeline `forge check` runs (REQ-4 — aggregation, never
    // re-derivation): `check_file` is the canonical default-config entry (the only
    // one that serves / populates the shared proof cache). The audit re-runs no
    // verus, re-scores no mutants — it projects the cert collection this returns.
    let certs = check::check_file(file)?;

    // Parse the file once for the boundary contracts' enforced req/ens/fx (the
    // §9 per-function contracts the TCB enumerates). A pure read of the parsed AST
    // — `check_file` already validated it parses clean, so this is re-parse of a
    // known-good file (deterministic, R-CODE-5), never a re-verification.
    let src = std::fs::read_to_string(file).map_err(|e| ForgeError::Io {
        path: file.display().to_string(),
        source: e,
    })?;
    let parsed = thermite_syntax::parse(&src);
    if !parsed.is_clean() {
        return Err(ForgeError::Parse(parsed.errors));
    }

    // The toolchain identity (the irreducible §9 TCB residue): the verus version
    // (the same deterministic sourcing the proof cache uses) + the compile-time
    // thermite version. `check_file` already required verus, so resolving the
    // version adds no requirement.
    let verus_version = audit::resolve_verus_version()?;
    let toolchain = audit::Toolchain::new(verus_version);

    let manifest = AuditManifest::from_certificates(&certs, &parsed.program, toolchain);

    if json {
        // The stable v1 document on stdout (REQ-1 — the oracle-asserted surface).
        let doc = serde_json::to_string_pretty(&manifest).map_err(|e| ForgeError::VerusOutput {
            detail: format!("failed to serialize audit manifest JSON: {e}"),
        })?;
        println!("{doc}");
    } else {
        print!("{}", render_audit(&manifest));
    }

    // The audit exit code mirrors `forge check`'s project headline: the manifest is
    // a projection, so a fully-certified project exits 0 and a project with a
    // non-certifying fn exits with the verification-failure code (the headlines
    // agree — both via `manifest::cert_certifies`).
    let certified = matches!(
        manifest.project_assurance.level,
        ProjectAssurance::Certified(_)
    );
    if certified {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(EXIT_VERIFICATION_FAILURE))
    }
}

/// Run `forge repair`: the background L1/L2 → L3 upgrade loop (#18;
/// `.design/forge/proof-repair.md` REQ-1/REQ-6). Drives `repair::repair_file`
/// (re-derive the sub-L3 certs at the default budget, escalate the bounded ladder
/// for TIMEOUT items ONLY, report the rest), then renders the per-item repair
/// report. A one-shot, deterministic, re-runnable pass (OQ-4 reading (a)).
///
/// The exit code (REQ-5 parallel): SUCCESS iff every repaired item upgraded to L3
/// AND no item remains a hard fail (a no-op corpus is vacuously success); else the
/// verification-failure code (a still-sub-L3 or not-repairable item means the
/// project does not fully certify). An ENVIRONMENT failure (verus absent /
/// unparseable) propagates as a `ForgeError` (REQ-7), never a silent success.
fn run_repair(file: &Path, item: Option<&str>, json: bool) -> Result<ExitCode, ForgeError> {
    let report = repair::repair_file(file, item)?;

    if json {
        let doc = serde_json::to_string_pretty(&repair_report_json(&report)).map_err(|e| {
            ForgeError::VerusOutput {
                detail: format!("failed to serialize repair report JSON: {e}"),
            }
        })?;
        println!("{doc}");
    } else {
        print!("{}", render_repair(&report));
    }

    // SUCCESS iff every sub-L3 item was upgraded (or there were none to repair).
    // A still-sub-L3 or not-repairable residue is a non-zero exit (the project
    // does not fully certify), parallel to `forge check`'s headline.
    if report.all_upgraded() {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(EXIT_VERIFICATION_FAILURE))
    }
}

/// Run `forge review`: the PLUGGABLE SPEC-INTENT REVIEW SLOT (#19;
/// `.design/forge/spec-review.md` REQ-5/REQ-7, §7 line 227). Extracts the
/// pre-screened declarative spec-layer ARTIFACT (`review::review_file` — a pure
/// projection of the battery cert collection + the parsed contract surface, no
/// bodies), emits it (`--json` machine form or human), and — with `--reviewer
/// <cmd>` — pipes it to the EXTERNAL reviewer, reads the `ReviewVerdict` JSON from
/// its stdout, and writes a SEPARATE `<file>.review.json` record (OQ-2: the verdict
/// is the reviewer's, never a `Certificate` field; forge NEVER fabricates
/// `aligned`).
///
/// Exit code: the EXTRACTION succeeding is a SUCCESS (the artifact is a valid
/// document — surfacing a battery-failing fn is not a forge failure, it is the
/// artifact's content). An ENVIRONMENT failure (verus absent for the pre-screen, a
/// `--reviewer` cmd absent/failing/garbage, an IO error) propagates as a
/// `ForgeError` (the environment exit code), never a silent success.
fn run_review(
    file: &Path,
    item: Option<&str>,
    json: bool,
    reviewer: Option<&str>,
) -> Result<ExitCode, ForgeError> {
    // The pre-screened spec-layer artifact (REQ-1/REQ-2/REQ-6) — the deterministic
    // pure projection. Runs the SAME default-config check pipeline `forge check`
    // runs (the battery verdict), then projects; it re-runs no verus.
    let artifact = review::review_file(file, item)?;

    if let Some(cmd) = reviewer {
        // The PLUGGABLE INTEGRATION (REQ-7, OQ-1): pipe the artifact JSON to the
        // external reviewer's stdin, read the `ReviewVerdict` JSON from its stdout
        // (the reviewer's judgment — forge never fabricates `aligned`), and write
        // the SEPARATE `<file>.review.json` record. A spawn/exit/parse failure is a
        // `ForgeError` (handled above by `?`), never a panic.
        let verdicts = review::run_reviewer(cmd, &artifact)?;
        let record = review::attach_verdicts(&file.display().to_string(), verdicts);
        let record_path = review_record_path(file);
        let doc =
            serde_json::to_string_pretty(&record).map_err(|e| ForgeError::ReviewerOutput {
                detail: format!("failed to serialize the review record JSON: {e}"),
            })?;
        std::fs::write(&record_path, format!("{doc}\n")).map_err(|e| ForgeError::Io {
            path: record_path.display().to_string(),
            source: e,
        })?;
        // Echo what was attached + where (stderr keeps `--json` stdout clean).
        eprintln!(
            "forge review: attached {} verdict(s) to `{}`",
            record.verdicts.len(),
            record_path.display()
        );
    }

    if json {
        let doc =
            serde_json::to_string_pretty(&artifact).map_err(|e| ForgeError::ReviewerOutput {
                detail: format!("failed to serialize the review artifact JSON: {e}"),
            })?;
        println!("{doc}");
    } else {
        print!("{}", render_review(&artifact));
    }

    Ok(ExitCode::SUCCESS)
}

/// Run `forge build`: lower the program to executable Rust and compile it with
/// `rustc` into a contract-checked artifact (#56; `.design/forge/build.md`
/// REQ-1/REQ-5). Drives `build::build_file` (the parse→validate→check_effects→
/// lower_l1→rustc pipeline), then renders the [`BuildManifest`] (the artifact
/// path and crate-type, the achieved assurance, the per-fn `fx` rows — the #57
/// hook — and the reproducibility block) as human text or (under `--json`) the
/// structured document.
///
/// `forge build` does NOT itself RUN the produced `--entry` executable: running is
/// left to the consumer / the conformance test (which exercises the runtime
/// `thermite_check!` behavior directly). This keeps `forge build` a pure
/// build-and-report step; observing the runtime check fire is the test's job
/// (`build_conformance::ens_violation_fires_at_runtime`).
///
/// Exit code: a successful build exits 0. A front-of-pipeline failure (parse /
/// spec / effects / lowering), an absent/failing rustc, or an IO error propagates
/// as a `ForgeError` (the environment exit code, REQ-2 / R-CODE-4), never a silent
/// success.
fn run_build(file: &Path, entry: Option<&str>, json: bool) -> Result<ExitCode, ForgeError> {
    let manifest = build::build_file(file, entry)?;
    if json {
        let doc = serde_json::to_string_pretty(&manifest).map_err(|e| ForgeError::RustcOutput {
            detail: format!("failed to serialize the build manifest JSON: {e}"),
        })?;
        println!("{doc}");
    } else {
        print!("{}", render_build(&manifest));
    }
    Ok(ExitCode::SUCCESS)
}

/// Render the [`BuildManifest`] as human-readable text (#56;
/// `.design/forge/build.md` REQ-5 — the `--json` form is the machine surface). The
/// artifact path + crate-type, the achieved assurance, the per-fn `fx` rows (the
/// #57 seccomp input), and the reproducibility block.
fn render_build(manifest: &BuildManifest) -> String {
    let mut out = String::new();
    let kind = match manifest.crate_type {
        CrateType::Rlib => "library (rlib)",
        CrateType::Bin => "executable (bin)",
    };
    out.push_str(&format!(
        "artifact: {} [{kind}]\n",
        manifest.artifact.display()
    ));
    if let Some(entry) = &manifest.entry {
        out.push_str(&format!(
            "entry: {entry} (deterministic synthesized inputs)\n"
        ));
    }
    out.push_str(&format!("assurance: {}\n", manifest.assurance));
    out.push_str("functions:\n");
    for f in &manifest.functions {
        out.push_str(&format!("  {} fx=[{}]\n", f.name, f.fx.join(", ")));
    }
    out.push_str("reproducibility:\n");
    out.push_str(&format!("  rustc: {}\n", manifest.reproducibility.rustc));
    out.push_str(&format!(
        "  SOURCE_DATE_EPOCH: {}\n",
        manifest.reproducibility.source_date_epoch
    ));
    out.push_str(&format!("  note: {}\n", manifest.reproducibility.note));
    out
}

/// The `<file>.review.json` record path for a reviewed `<file>` (#19; REQ-4, OQ-2 —
/// a SEPARATE document keyed by the reviewed file). `conformance/sum.th` →
/// `conformance/sum.th.review.json`.
fn review_record_path(file: &Path) -> PathBuf {
    let mut name = file
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(".review.json");
    file.with_file_name(name)
}

/// Render the spec-intent review ARTIFACT as human-readable text (#19;
/// `.design/forge/spec-review.md` REQ-5, §7 — the human half of the dual emission;
/// the `--json` form is the critic-model surface). Per intent-reviewable fn: its
/// declarative spec layer (req/ens/fx plus referenced spec-fn declarations, NO
/// bodies) and the "is this what you meant?" prompt; then the battery-failing fns
/// flagged with their cause (NOT surfaced for intent review, R-DEFER-9).
fn render_review(artifact: &ReviewArtifact) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "spec-intent review: {} intent-reviewable, {} battery-failing\n",
        artifact.intent_reviewable.len(),
        artifact.battery_failing.len()
    ));
    for r in &artifact.intent_reviewable {
        out.push_str(&format!(
            "\nfn {} (battery-passing — spec layer):\n",
            r.item
        ));
        out.push_str(&format!("  req {}\n", r.spec_layer.req));
        for e in &r.spec_layer.ens {
            out.push_str(&format!("  ens {e}\n"));
        }
        out.push_str(&format!("  fx  [{}]\n", r.spec_layer.fx.join(", ")));
        for decl in &r.spec_layer.referenced_spec_fns {
            out.push_str(&format!("  {} dec {}\n", decl.signature, decl.dec));
        }
        out.push_str(&format!("  prompt: {}\n", r.prompt));
    }
    if !artifact.battery_failing.is_empty() {
        out.push_str(
            "\nbattery-failing (NOT surfaced for intent review — mechanical failure first):\n",
        );
        for b in &artifact.battery_failing {
            out.push_str(&format!("  {} — {} ({})\n", b.item, b.cause, b.detail));
        }
    }
    out
}

/// Build the `--json` document for a repair report (#18; §5.1 structured output).
/// `RepairReport` is a runtime aggregate (not a serde schema type), so the JSON is
/// hand-built here — the stable surface a calling agent reads.
fn repair_report_json(report: &RepairReport) -> serde_json::Value {
    use serde_json::json;
    let items: Vec<serde_json::Value> = report
        .items
        .iter()
        .map(|i| match &i.outcome {
            RepairOutcome::UpgradedToL3 { budget } => json!({
                "item": i.item,
                "outcome": "upgraded_to_l3",
                "budget": budget,
            }),
            RepairOutcome::StillSubL3 {
                level,
                profile,
                suggested_move,
                detail,
            } => json!({
                "item": i.item,
                "outcome": "still_sub_l3",
                "level": level_str(*level),
                "total_instantiations": profile.as_ref().map(|p| p.total_instantiations),
                "suggested_move": suggested_move.as_ref().map(|m| json!({
                    "kind": m.kind, "detail": m.detail,
                })),
                "detail": detail,
            }),
            RepairOutcome::NotRepairable {
                level,
                cause,
                detail,
            } => json!({
                "item": i.item,
                "outcome": "not_repairable",
                "level": level_str(*level),
                "cause": cause,
                "detail": detail,
            }),
        })
        .collect();
    json!({
        "total_checked": report.total_checked,
        "repaired": items,
    })
}

/// Render the repair report as human-readable text (#18; REQ-6, §5.1 "every
/// message is a prompt"). One line per sub-L3 item: `upgraded to L3 (budget=N)` /
/// `still <level> — <#11 repair prompt>` / `counterexample/reject — not repairable
/// (not retried)`. A no-op (the corpus, AC-1) prints the "nothing to repair" line.
fn render_repair(report: &RepairReport) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "repair: {} item(s) checked, {} sub-L3 item(s) to repair\n",
        report.total_checked,
        report.items.len()
    ));
    if report.is_noop() {
        out.push_str("nothing to repair — every item already certifies at L3\n");
        return out;
    }
    for item in &report.items {
        out.push_str(&render_repair_item(item));
    }
    out
}

/// Render one item's repair outcome line (#18; REQ-6).
fn render_repair_item(item: &RepairItem) -> String {
    match &item.outcome {
        RepairOutcome::UpgradedToL3 { budget } => {
            format!("  {} — upgraded to L3 (budget={budget})\n", item.item)
        }
        RepairOutcome::StillSubL3 {
            level,
            profile: _,
            suggested_move,
            detail,
        } => {
            let prompt = suggested_move
                .as_ref()
                .map(|m| format!("{} — {}", m.kind, m.detail))
                .unwrap_or_else(|| detail.clone());
            format!(
                "  {} — still {} (not proved at the ladder cap) — repair prompt: {prompt}\n",
                item.item,
                level_str(*level),
            )
        }
        RepairOutcome::NotRepairable {
            level,
            cause,
            detail,
        } => format!(
            "  {} — {} {} — not repairable (not retried; more budget never makes a false \
             contract true): {detail}\n",
            item.item,
            level_str(*level),
            cause,
        ),
    }
}

/// Render the AUDIT MANIFEST v1 as a human-readable summary (#15;
/// `.design/forge/audit-manifest.md` REQ-2, OQ-1 — the human shape is a rendering
/// detail; the `--json` document is the stable contract). Three sections: the
/// per-fn table, the project assurance, and the §8/§9 greppable TCB inventory.
fn render_audit(manifest: &AuditManifest) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "audit manifest {} ({} function(s))\n",
        manifest.manifest_version,
        manifest.functions.len()
    ));

    // The per-fn table (§6 — every function's level + slag/boundary flags).
    out.push_str("functions:\n");
    for f in &manifest.functions {
        let scope = match &f.assurance_scope {
            Some(AssuranceScope::EndToEnd) => " scope=end-to-end".to_string(),
            Some(AssuranceScope::ToBoundary { via }) => {
                format!(" scope=to-the-boundary(via {via})")
            }
            None => String::new(),
        };
        let flags = format!(
            "{}{}",
            if f.slag { " slag" } else { "" },
            if f.boundary { " boundary" } else { "" }
        );
        out.push_str(&format!(
            "  {} {}{}{}\n",
            f.name,
            level_str(f.level),
            flags,
            scope
        ));
    }

    // The project assurance headline + scope + lowered-assurance fns (REQ-5).
    out.push_str("project assurance:\n");
    let level_line = match manifest.project_assurance.level {
        ProjectAssurance::Certified(level) => {
            format!("  level: {} (min over functions)\n", level_str(level))
        }
        ProjectAssurance::Failed => "  level: FAILED (a function did not certify)\n".to_string(),
    };
    out.push_str(&level_line);
    match &manifest.project_assurance.scope {
        ProjectScope::EndToEnd => out.push_str("  scope: end-to-end (verified, period)\n"),
        ProjectScope::ToBoundary { crossings } => out.push_str(&format!(
            "  scope: to-the-boundary (crossings: {})\n",
            crossings.join(", ")
        )),
    }
    for name in &manifest.project_assurance.lowered_assurance {
        out.push_str(&format!(
            "  lowered-assurance: {name} (auto-degraded below L3)\n"
        ));
    }

    // The §9 ENUMERABLE TCB — slag ∪ boundary ∪ toolchain. The §8 "`grep slag` is
    // the complete inventory" framing → a line-oriented, greppable section.
    out.push_str("tcb (trusted computing base):\n");
    if manifest.tcb.slag_blocks.is_empty() && manifest.tcb.boundary_contracts.is_empty() {
        out.push_str("  slag: (none) — no fiat-trusted bodies\n");
        out.push_str("  boundary: (none) — no foreign crossings\n");
    } else {
        for b in &manifest.tcb.slag_blocks {
            out.push_str(&format!(
                "  slag: {} reason={:?} owner={:?} review={:?}\n",
                b.name, b.reason, b.owner, b.review
            ));
        }
        for c in &manifest.tcb.boundary_contracts {
            out.push_str(&format!(
                "  boundary: {} -> {} (req={:?} ens=[{}] fx=[{}])\n",
                c.name,
                c.target,
                c.req.as_deref().unwrap_or("(unresolved)"),
                c.ens.join("; "),
                c.fx.join(", ")
            ));
        }
    }
    out.push_str(&format!(
        "  toolchain: verus={} thermite={}\n",
        manifest.tcb.toolchain.verus, manifest.tcb.toolchain.thermite
    ));
    out
}

/// Render the #10 project ASSURANCE MANIFEST as human-readable text
/// (`.design/forge/degrade-ladder.md` REQ-5/REQ-6, §5.2 "displayed on every
/// build"). The project headline (the min-over-functions, or `FAILED` when any fn
/// does not certify) plus, when any function was an automatic degrade, the per-fn
/// lowered-assurance flags. The headline goes LAST so it is the final line a reader
/// (or an agent) sees.
fn render_assurance(manifest: &AssuranceManifest) -> String {
    let mut out = String::new();
    out.push_str("---\n");
    // The per-fn lowered-assurance view: surface each fn that was auto-degraded
    // (REQ-5) so the headline's "why" is visible. A no-degrade build (the corpus)
    // prints none of these (AC-1).
    for f in &manifest.functions {
        if f.lowered_assurance {
            out.push_str(&format!(
                "lowered-assurance: {} achieved {} (auto-degraded below L3)\n",
                f.item,
                level_str(f.level)
            ));
        }
    }
    let headline = match manifest.project {
        ProjectAssurance::Certified(level) => {
            format!(
                "project assurance: {} (min over functions)",
                level_str(level)
            )
        }
        ProjectAssurance::Failed => {
            "project assurance: FAILED (a function did not certify — not a lowered rung)"
                .to_string()
        }
    };
    out.push_str(&headline);
    out.push('\n');
    out
}

/// Render a [`Certificate`] as human-readable text (REQ-4, §5.1 "rendered to
/// readable text"). The §5.1 structured JSON is the `--json` rendering; this is
/// the default.
fn render_human(cert: &Certificate) -> String {
    // The deterministic oracle-stable subset (manifest::Certificate::oracle_subset):
    // item / level / effects / slag — the fields the cert-oracle compares — are
    // rendered first, then the non-deterministic `solver_time_ms` labelled as
    // such so a reader does not mistake it for an oracle field.
    let (item, level, effects, slag, boundary, _scope_end_to_end) = cert.oracle_subset();
    let mut out = String::new();
    out.push_str(&format!("item: {item}\n"));
    out.push_str(&format!("level: {}\n", level_str(level)));
    out.push_str(&format!("effects: [{}]\n", effects.join(", ")));
    out.push_str(&format!("slag: {slag}\n"));
    // #16: a boundary fn (FFI crossing) renders its flag + foreign target so the
    // §9 "to-the-boundary, body unproven" status is visible (the #15 TCB hook).
    out.push_str(&format!("boundary: {boundary}\n"));
    if let Some(target) = &cert.boundary_target {
        out.push_str(&format!("boundary_target: {target}\n"));
    }
    // #17: end-to-end vs to-the-boundary (§9) — whether the verified guarantee
    // depends on an unproven foreign/slag body anywhere in the call closure.
    match &cert.assurance_scope {
        Some(AssuranceScope::EndToEnd) => {
            out.push_str("assurance_scope: end-to-end\n");
        }
        Some(AssuranceScope::ToBoundary { via }) => {
            out.push_str(&format!("assurance_scope: to-the-boundary (via {via})\n"));
        }
        None => {}
    }
    // #6: a valid `#[slag]` item carries its audit metadata (§8 visibility).
    if let Some(meta) = &cert.slag_meta {
        out.push_str(&format!(
            "slag_meta: reason={:?}, owner={:?}, review={:?}\n",
            meta.reason, meta.owner, meta.review
        ));
    }
    // #6: a triage / slag-validation reject names its structured cause.
    if let Some(reject) = &cert.reject {
        out.push_str(&format!("reject: {} — {}\n", reject.cause, reject.detail));
    }
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
            parse_args(&argv(&["check", "a.th"])).ok(),
            Some(Command::Check {
                file: PathBuf::from("a.th"),
                json: false,
                level: CheckLevel::L3,
                rlimit: DEFAULT_RLIMIT,
                mutation_floor: MUTATION_FLOOR,
            })
        );
        assert_eq!(
            parse_args(&argv(&["check", "a.th", "--json"])).ok(),
            Some(Command::Check {
                file: PathBuf::from("a.th"),
                json: true,
                level: CheckLevel::L3,
                rlimit: DEFAULT_RLIMIT,
                mutation_floor: MUTATION_FLOOR,
            })
        );
    }

    // #11 (`.design/forge/solver-profiles.md` REQ-5): `--rlimit <FLOAT>` parses
    // into the `Check.rlimit`; the DEFAULT (no flag) is the pinned generous
    // `DEFAULT_RLIMIT`; a missing / non-numeric / non-positive value is a Usage
    // error (the test lever for the timeout path uses a LOW value like `1`).
    #[test]
    fn parses_rlimit_flag() {
        assert_eq!(
            parse_args(&argv(&["check", "a.th", "--rlimit", "1"])).ok(),
            Some(Command::Check {
                file: PathBuf::from("a.th"),
                json: false,
                level: CheckLevel::L3,
                rlimit: 1.0,
                mutation_floor: MUTATION_FLOOR,
            })
        );
        // Default when the flag is absent.
        assert_eq!(
            parse_args(&argv(&["check", "a.th"])).ok(),
            Some(Command::Check {
                file: PathBuf::from("a.th"),
                json: false,
                level: CheckLevel::L3,
                rlimit: DEFAULT_RLIMIT,
                mutation_floor: MUTATION_FLOOR,
            })
        );
        // Missing value, non-numeric, and non-positive are Usage errors.
        assert!(matches!(
            parse_args(&argv(&["check", "a.th", "--rlimit"])),
            Err(ForgeError::Usage(_))
        ));
        assert!(matches!(
            parse_args(&argv(&["check", "a.th", "--rlimit", "nope"])),
            Err(ForgeError::Usage(_))
        ));
        assert!(matches!(
            parse_args(&argv(&["check", "a.th", "--rlimit", "0"])),
            Err(ForgeError::Usage(_))
        ));
    }

    // #12 (`.design/forge/mutation-scoring.md` REQ-5): `--mutation-floor <FLOAT>`
    // parses into `Check.mutation_floor`; the DEFAULT (no flag) is `MUTATION_FLOOR`
    // (0.60); a missing / non-numeric / out-of-[0,1] value is a Usage error (the
    // AC-3 floor-flip lever uses a LOW value like `0.2`).
    #[test]
    fn parses_mutation_floor_flag() {
        assert_eq!(
            parse_args(&argv(&["check", "a.th", "--mutation-floor", "0.2"])).ok(),
            Some(Command::Check {
                file: PathBuf::from("a.th"),
                json: false,
                level: CheckLevel::L3,
                rlimit: DEFAULT_RLIMIT,
                mutation_floor: 0.2,
            })
        );
        // Default when the flag is absent.
        assert_eq!(
            parse_args(&argv(&["check", "a.th"]))
                .ok()
                .and_then(|c| match c {
                    Command::Check { mutation_floor, .. } => Some(mutation_floor),
                    _ => None,
                }),
            Some(MUTATION_FLOOR)
        );
        // Missing value, non-numeric, and out-of-range are Usage errors.
        assert!(matches!(
            parse_args(&argv(&["check", "a.th", "--mutation-floor"])),
            Err(ForgeError::Usage(_))
        ));
        assert!(matches!(
            parse_args(&argv(&["check", "a.th", "--mutation-floor", "nope"])),
            Err(ForgeError::Usage(_))
        ));
        assert!(matches!(
            parse_args(&argv(&["check", "a.th", "--mutation-floor", "1.5"])),
            Err(ForgeError::Usage(_))
        ));
    }

    // REQ-7 (`.design/lower/l2-kani.md`): `--level l2` selects the Kani path; the
    // DEFAULT (no flag) is L3; an unknown / missing value is a Usage error.
    #[test]
    fn parses_level_flag() {
        assert_eq!(
            parse_args(&argv(&["check", "a.th", "--level", "l2"])).ok(),
            Some(Command::Check {
                file: PathBuf::from("a.th"),
                json: false,
                level: CheckLevel::L2,
                rlimit: DEFAULT_RLIMIT,
                mutation_floor: MUTATION_FLOOR,
            })
        );
        assert_eq!(
            parse_args(&argv(&["check", "a.th", "--level", "l3"])).ok(),
            Some(Command::Check {
                file: PathBuf::from("a.th"),
                json: false,
                level: CheckLevel::L3,
                rlimit: DEFAULT_RLIMIT,
                mutation_floor: MUTATION_FLOOR,
            })
        );
        assert!(matches!(
            parse_args(&argv(&["check", "a.th", "--level", "l9"])),
            Err(ForgeError::Usage(_))
        ));
        assert!(matches!(
            parse_args(&argv(&["check", "a.th", "--level"])),
            Err(ForgeError::Usage(_))
        ));
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

    // #10 (degrade-ladder REQ-5/REQ-6): render_assurance prints the project
    // headline (the min-over-functions) and the per-fn lowered-assurance lines. A
    // {L3,L2} set with the L2 degraded → headline L2 + one lowered-assurance line.
    #[test]
    fn render_assurance_shows_headline_and_lowered_flags() {
        use crate::manifest::{AssuranceManifest, RejectReason};
        let reason = RejectReason {
            cause: "VerusTimeout".to_string(),
            detail: "rlimit".to_string(),
        };
        let certs = vec![
            Certificate::new("f", Level::L3, vec!["pure".to_string()], 0, vec![]),
            Certificate::new("g", Level::L2, vec!["pure".to_string()], 0, vec![])
                .into_degraded(reason),
        ];
        let m = AssuranceManifest::aggregate(&certs);
        let text = render_assurance(&m);
        assert!(
            text.contains("project assurance: L2"),
            "headline is the min over functions (L2):\n{text}"
        );
        assert!(
            text.contains("lowered-assurance: g achieved L2"),
            "the degraded fn is surfaced:\n{text}"
        );
    }

    // #10 (REQ-2): a project with a hard-failed fn shows the FAILED headline (not a
    // lowered rung).
    #[test]
    fn render_assurance_shows_failed_headline() {
        use crate::manifest::{AssuranceManifest, RejectReason};
        let reason = RejectReason {
            cause: "EnsIsTrivial".to_string(),
            detail: "x".to_string(),
        };
        let certs = vec![Certificate::rejected(
            "bad",
            vec!["pure".to_string()],
            false,
            reason,
        )];
        let m = AssuranceManifest::aggregate(&certs);
        let text = render_assurance(&m);
        assert!(
            text.contains("project assurance: FAILED"),
            "a non-certifying fn is a project FAILURE:\n{text}"
        );
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
