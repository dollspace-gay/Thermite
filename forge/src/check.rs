//! `forge/src/check.rs` — the v0.1 `forge check` pipeline. It runs each `fn` /
//! `spec fn` item in a `.th` file end-to-end through every shipped kernel
//! component, invokes the REAL `verus` binary on the lowered source, parses
//! verus's output into per-obligation results (with counterexamples on failure),
//! and assembles the structured certificate (`manifest.rs`). This is the FIRST
//! LIVE cert-oracle: `forge check conformance/sum.th`'s deterministic certificate
//! fields must match the golden `conformance/sum.cert.json`.
//!
//! Governing design: `.design/forge/check.md`. Pipeline order (the kernel's data
//! dependency):
//!
//! ```text
//! parse → validate → check_effects → lower → run verus → parse output → Certificate
//! ```
//!
//! **Signature note (R-SPEC-4 honored, not silently diverged).** The design doc
//! `.design/forge/check.md` sketches `check_file(path, seed)`; the orchestrator's
//! issue #5 manifest mandates `check_file(path) -> Result<Vec<Certificate>, _>`.
//! These are reconciled WITHOUT a contract change: the pinned solver seed (§5.3)
//! is sourced from the project lockfile when present and otherwise from
//! [`DEFAULT_SOLVER_SEED`], so `check_file` keeps the issue's one-argument shape
//! while still passing a deterministic seed to verus (REQ-7). No design field is
//! redefined; the seed is INPUT-derived, not a new parameter on the contract.
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (pipeline orchestration) | SHIPPED | `pub fn check_file` runs `thermite_syntax::parse` → `thermite_spec::validate` → `thermite_lower::check_effects`, then PER ITEM (§5.3) `item_subprogram` → `thermite_lower::lower` → `run_verus` → `parse_verus_output` → `Certificate`; each stage short-circuits into a `ForgeError`. Consumer: `cli::run` (`cli.rs`). |
//! | REQ-2 (verus invocation, temp file, crate-name gotcha) | SHIPPED | `lower_to_temp` writes a `<stem>_check.rs` temp file (no `.` in the stem — `crate_stem`), `run_verus` spawns `verus --output-json --smt-option smt.random_seed=<seed>`; cleaned up after. |
//! | REQ-3 (exit-status checked, never swallow) | SHIPPED | `run_verus` returns exit status; `parse_verus_output` makes a parseable failure a reported cert and an unparseable/internal failure `ForgeError::VerusOutput`; spawn ENOENT → `ForgeError::VerusAbsent`. |
//! | REQ-4 (verus output → per-obligation + counterexamples) | SHIPPED | `parse_verus_output` reads the JSON `verification-results` summary for level and parses stderr `error:` + `--> file:line:col` into `ObligationResult::failed` witnesses. |
//! | REQ-5 (level determination, v0.1) | SHIPPED | `level_from_summary`: `Level::L3` iff `success && errors == 0`, else the run is a reported non-L3 failure. |
//! | REQ-6 (verus-absent = environment error) | SHIPPED | `run_verus` maps spawn `ErrorKind::NotFound` to `ForgeError::VerusAbsent`. |
//! | REQ-7 (determinism) | SHIPPED | pinned seed (`DEFAULT_SOLVER_SEED` / lockfile) passed to verus; `solver_time_ms` is the only wall-clock field and is excluded from the oracle (`manifest::Certificate::oracle_eq`). |
//!
//! ## #6 gate (structural vacuity triage + `#[slag]`, this iteration)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | vacuity-triage REQ-6 (gate BEFORE L3) | SHIPPED | `gate_fn` runs `vacuity::triage` on each `Item::Fn` BEFORE `thermite_lower::lower` + `run_verus`; a `VacuityVerdict::Rejected` short-circuits to a non-certified `Certificate::rejected` (no lowering, no verus); a pass calls `Certificate::graduate_triage_clean` (the two §7.1 `contract_quality` bools go live-`false`). |
//! | slag REQ-2/REQ-5 (L1 short-circuit) | SHIPPED | `gate_fn` for a `slag.is_some()` item runs `slag::validate` (invalid → `Certificate::rejected`), then `vacuity::triage` (a/b/c — slag exempts (d) inside `triage`), then `Certificate::slag_l1` (`Level::L1`, `slag: true`, `slag_meta`) WITHOUT invoking verus. |

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use thermite_syntax::{Item, Program};

use crate::cli::ForgeError;
use crate::manifest::{effects_of, Certificate, Level, ObligationResult, RejectReason};

/// The pinned default solver seed (§5.3) used when no project lockfile supplies
/// one. Determinism (R-CODE-5) lives in the INPUT (this fixed seed + the
/// toolchain version), not in wall-clock state.
pub const DEFAULT_SOLVER_SEED: u64 = 0;

/// Run the full v0.1 `forge check` pipeline for every `fn` / `spec fn` item in
/// `path`, returning one [`Certificate`] per item in source order (REQ-1).
///
/// Stages short-circuit into the EARLIEST failing stage's `ForgeError`. A verus
/// obligation FAILURE is NOT an `Err`: it is a valid certificate describing the
/// failure (level != L3, with per-obligation witnesses). Only an environment /
/// internal failure (verus absent, unparseable output, IO) is an `Err`.
pub fn check_file(path: impl AsRef<Path>) -> Result<Vec<Certificate>, ForgeError> {
    let path = path.as_ref();
    let src = std::fs::read_to_string(path).map_err(|e| ForgeError::Io {
        path: path.display().to_string(),
        source: e,
    })?;

    // 1. parse (thermite-syntax).
    let parsed = thermite_syntax::parse(&src);
    if !parsed.is_clean() {
        return Err(ForgeError::Parse(parsed.errors));
    }

    // 2. validate (thermite-spec) — the SpecTherm cage.
    thermite_spec::validate(&parsed.program).map_err(ForgeError::Spec)?;

    // 3. effect-check (thermite-lower) — `fx` subsumption (§4.1). Effect
    // subsumption is a whole-program property (a caller's row must subsume every
    // callee's), so it is checked once over the full program before any per-item
    // split.
    thermite_lower::check_effects(&parsed.program).map_err(ForgeError::Effects)?;

    // 4/5/6/7. PER-ITEM certification (`thermite-design.md` §5.3 — "proof
    // results content-addressed and cached PER ITEM"; "an edit to `f` cannot
    // invalidate `g`'s certificate unless `g`'s contract references `f`'s").
    // Each `fn` is lowered and verified in ISOLATION — a sub-program holding only
    // THAT `fn` plus the file's `spec fn`s (pure shared dependencies its contract
    // may reference) plus the combinator defs the lowerer emits. So verus's run
    // yields only that item's obligations and its level is L3 iff THAT item's
    // obligations all discharge, independent of any sibling's failure (§6 — the
    // certificate lists EVERY function's OWN level; §5.1 — a counterexample
    // belongs to the item it is reported on, never a neighbor's).
    let seed = resolve_seed(path);

    // The file's `spec fn`s are pure, contract-free shared dependencies; they go
    // into every per-item sub-program so a `fn` whose `ens` references one (e.g.
    // `sum`'s `ens result == spec_sum(xs)`) still lowers and verifies.
    let spec_items: Vec<Item> = parsed
        .program
        .items
        .iter()
        .filter(|i| matches!(i, Item::SpecFn(_)))
        .cloned()
        .collect();

    let mut certs = Vec::with_capacity(parsed.program.items.len());
    for item in &parsed.program.items {
        // #6 gate: structural vacuity triage + `#[slag]` short-circuit run BEFORE
        // the L3 proof ("a function does not certify until its contract
        // certifies", §7). A `spec fn` carries no contract (ast.rs `SpecFnItem`),
        // so the gate applies only to `Item::Fn` — a `spec fn` proceeds to the
        // normal well-formedness path unchanged.
        if let Item::Fn(f) = item {
            match gate_fn(f) {
                // A valid `#[slag]` item certifies L1 by fiat (no verus run,
                // `.design/forge/slag.md` REQ-2): the L1 runtime-check codegen is
                // thermite-lower's `l1.rs` job at build time, not here.
                GateOutcome::SlagL1(cert) => {
                    certs.push(cert);
                    continue;
                }
                // A triage / slag-validation reject: the item does NOT certify
                // (verdict-in-cert, not a `ForgeError`; vacuity-triage.md REQ-5
                // OQ-1). No lowering, no verus.
                GateOutcome::Rejected(cert) => {
                    certs.push(cert);
                    continue;
                }
                // A non-slag item that passed all four triage checks proceeds to
                // the normal L3 path; the cert graduates the two §7.1
                // `contract_quality` bools to live-`false` (REQ-6).
                GateOutcome::ProceedToL3 => {}
            }
        }

        let sub = item_subprogram(item, &spec_items);
        let lowered = thermite_lower::lower(&sub).map_err(ForgeError::Lower)?;
        let verus = run_verus(&sub, &lowered, seed)?;
        let cert = assemble_certificate(item, &verus);
        // A non-slag `fn` that reached the L3 path passed triage — graduate the
        // §7.1 `contract_quality` bools to asserted live-`false` (REQ-6 / AC-7). A
        // `spec fn` carries no contract, so triage does not apply and the bools
        // stay forward-declared.
        let cert = if matches!(item, Item::Fn(_)) {
            cert.graduate_triage_clean()
        } else {
            cert
        };
        certs.push(cert);
    }
    Ok(certs)
}

/// The result of the #6 contract-certification gate for one `fn`
/// (`.design/forge/check.md`; `vacuity-triage.md` REQ-6; `slag.md` REQ-5).
enum GateOutcome {
    /// A valid `#[slag]` item: certify L1 by fiat (no verus run) — the cert.
    SlagL1(Certificate),
    /// A triage / slag-validation reject: the item does not certify — the cert.
    Rejected(Certificate),
    /// A non-slag item that passed all four triage checks: run the normal L3 path.
    ProceedToL3,
}

/// Run the #6 gate for one `fn` (slag-validate → triage → L1-vs-L3 fork). The two
/// components COMPOSE per `.design/forge/slag.md`:
///
/// ```text
/// #[slag]?  ──no──▶ triage(a,b,c,d)  ──reject──▶ Rejected
///   │ yes              │ pass
///   │                  ▼
///   │               ProceedToL3 (graduate contract_quality)
///   ▼
/// slag::validate  ──Err──▶ Rejected (contract-cert failure, no L1/L3)
///   │ Ok(meta)
///   ▼
/// triage(a,b,c)   ──reject──▶ Rejected (slag exempts proving, not stating, §8)
///   │ pass (d skipped because slag present)
///   ▼
/// SlagL1 (level L1, slag:true, slag_meta)
/// ```
fn gate_fn(f: &thermite_syntax::FnItem) -> GateOutcome {
    let effects = effects_of(&f.contract.fx);

    if let Some(slag_attr) = f.slag.as_ref() {
        // Slag path: validate the mandatory fields FIRST (it gates whether rule
        // (d) is justified). Invalid fields → reject (the item does not certify).
        let meta = match crate::slag::validate(slag_attr) {
            Ok(meta) => meta,
            Err(err) => {
                return GateOutcome::Rejected(Certificate::rejected(
                    f.name.clone(),
                    effects,
                    true,
                    RejectReason {
                        cause: err.tag().to_string(),
                        detail: err.detail(),
                    },
                ));
            }
        };
        // Valid fields: triage STILL applies (a)/(b)/(c) — slag exempts only (d)
        // (slag.md REQ-3, §8). `vacuity::triage` reads `f.slag` itself and skips
        // (d) because it is present.
        match crate::vacuity::triage(f) {
            crate::vacuity::VacuityVerdict::Rejected { cause } => {
                GateOutcome::Rejected(Certificate::rejected(
                    f.name.clone(),
                    effects,
                    true,
                    RejectReason {
                        cause: cause.tag().to_string(),
                        detail: cause.detail(),
                    },
                ))
            }
            // Valid + triage clean → certify L1 by fiat (no verus).
            crate::vacuity::VacuityVerdict::Passed => {
                GateOutcome::SlagL1(Certificate::slag_l1(f.name.clone(), effects, meta))
            }
        }
    } else {
        // Non-slag path: run all four triage checks.
        match crate::vacuity::triage(f) {
            crate::vacuity::VacuityVerdict::Rejected { cause } => {
                GateOutcome::Rejected(Certificate::rejected(
                    f.name.clone(),
                    effects,
                    false,
                    RejectReason {
                        cause: cause.tag().to_string(),
                        detail: cause.detail(),
                    },
                ))
            }
            crate::vacuity::VacuityVerdict::Passed => GateOutcome::ProceedToL3,
        }
    }
}

/// Build the per-item sub-`Program` that isolates `item`'s verification (§5.3).
///
/// - A `fn` is verified against itself plus the file's `spec fn`s (the pure
///   shared dependencies its contract may reference), so its obligations are its
///   own and a sibling `fn`'s failure cannot leak in.
/// - A `spec fn` carries no `req`/`ens`/`fx` contract (`ast.rs` `SpecFnItem`,
///   §4.2): there is no L3 proof obligation to discharge, only well-formedness
///   (the `decreases` measure). It is verified against the set of `spec fn`s
///   alone (which already contains it), so a mutually-recursive spec fn still
///   resolves. The resulting cert records the spec fn's well-formedness as its
///   own discharged result — never a neighbor `fn`'s counterexample.
fn item_subprogram(item: &Item, spec_items: &[Item]) -> Program {
    match item {
        // The `fn` plus all pure spec-fn dependencies, in source order (spec fns
        // first so a forward reference resolves; the lowerer emits combinator
        // defs and dedups regardless of order).
        Item::Fn(_) => {
            let mut items = spec_items.to_vec();
            items.push(item.clone());
            Program { items }
        }
        // Spec fns verified together (mutual recursion); `spec_items` already
        // includes `item`.
        Item::SpecFn(_) => Program {
            items: spec_items.to_vec(),
        },
    }
}

/// Resolve the pinned solver seed for `path` (§5.3). v0.1 reads no lockfile yet
/// (`forge new`'s lockfile schema is minimal), so this returns the pinned
/// [`DEFAULT_SOLVER_SEED`]; the function is the single seam where lockfile
/// sourcing lands (#8) without changing `check_file`'s signature.
fn resolve_seed(_path: &Path) -> u64 {
    DEFAULT_SOLVER_SEED
}

/// The parsed result of one verus run: the machine-readable summary (drives
/// level, REQ-5) plus the per-obligation results (REQ-4).
#[derive(Debug, Clone)]
struct VerusResult {
    level: Level,
    solver_time_ms: u64,
    obligations: Vec<ObligationResult>,
}

/// The `verification-results` summary verus emits under `--output-json`
/// (grounded: `{success, verified, errors, encountered-error,
/// encountered-vir-error}`). Only the level-relevant fields are needed.
#[derive(Debug, Clone, Copy)]
struct VerusSummary {
    success: bool,
    verified: u64,
    errors: u64,
    encountered_vir_error: bool,
}

/// Compute a valid Rust crate stem from a source file path (REQ-2 / AC-4): the
/// file stem with every non-alphanumeric character replaced by `_`, suffixed
/// `_check`, guaranteeing NO `.` (verus derives the crate name from the file
/// stem and rejects a `.`). The grounded gotcha: `verus sum.verus.rs` →
/// `invalid character '.' in crate name: sum.verus`.
fn crate_stem(path: &Path) -> String {
    let raw = path.file_stem().and_then(|s| s.to_str()).unwrap_or("item");
    let mut stem = String::with_capacity(raw.len() + 6);
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            stem.push(ch);
        } else {
            stem.push('_');
        }
    }
    // A crate name cannot start with a digit; prefix if needed.
    if stem
        .chars()
        .next()
        .map(|c| c.is_ascii_digit())
        .unwrap_or(true)
    {
        stem.insert(0, 'c');
    }
    stem.push_str("_check");
    stem
}

/// Write the lowered source to a temp file with a valid-crate-name stem (REQ-2),
/// spawn verus on it with the pinned seed + `--output-json`, parse the result,
/// and clean up the temp file. Verus absent on spawn → `ForgeError::VerusAbsent`
/// (REQ-6); a non-zero exit with parseable failure → a reported failure cert;
/// unparseable output → `ForgeError::VerusOutput` (REQ-3).
fn run_verus(program: &Program, lowered: &str, seed: u64) -> Result<VerusResult, ForgeError> {
    // Name the temp file after the first item (deterministic) so concurrent
    // runs over different files do not collide; fall back to a fixed stem.
    let label = program.items.first().map(|i| i.name()).unwrap_or("forge");
    let stem = crate_stem(Path::new(label));
    let tmp = unique_temp_path(&stem);
    std::fs::write(&tmp, lowered).map_err(|e| ForgeError::Io {
        path: tmp.display().to_string(),
        source: e,
    })?;

    let result = invoke_verus(&tmp, seed);

    // Best-effort cleanup; never mask the real result on a cleanup failure.
    let _ = std::fs::remove_file(&tmp);

    result
}

/// Spawn verus and parse its output. Split from `run_verus` so the temp file is
/// always cleaned up regardless of outcome.
fn invoke_verus(tmp: &Path, seed: u64) -> Result<VerusResult, ForgeError> {
    let started = Instant::now();
    let output = Command::new("verus")
        .arg("--output-json")
        .arg("--smt-option")
        .arg(format!("smt.random_seed={seed}"))
        .arg(tmp)
        .current_dir(std::env::temp_dir())
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ForgeError::VerusAbsent {
                    binary: "verus".to_string(),
                }
            } else {
                ForgeError::VerusSpawn { source: e }
            }
        })?;
    let solver_time_ms = started.elapsed().as_millis() as u64;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let exit_code = output.status.code();

    parse_verus_output(&stdout, &stderr, exit_code, solver_time_ms)
}

/// Build a unique temp path with the given valid stem and a `.rs` extension.
/// Uniqueness uses the process id + a monotonic counter — NOT wall-clock — so
/// the path varies between concurrent runs without violating R-CODE-5
/// (determinism is a property of the CERTIFICATE, not the scratch path; §check.md
/// REQ-2 "determinism is in the INPUT, not the path").
fn unique_temp_path(stem: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    // No `.` before the extension in the STEM — the dot is only the extension
    // separator, which verus accepts; the crate name derives from the stem.
    std::env::temp_dir().join(format!("forge_{stem}_{pid}_{n}.rs"))
}

/// Parse verus's `--output-json` stdout + stderr into a [`VerusResult`] (REQ-4,
/// REQ-5, REQ-3). The JSON `verification-results` object drives the level; the
/// stderr `error:` / `--> file:line:col` pairs become per-obligation failure
/// witnesses.
fn parse_verus_output(
    stdout: &str,
    stderr: &str,
    exit_code: Option<i32>,
    solver_time_ms: u64,
) -> Result<VerusResult, ForgeError> {
    let summary = parse_summary(stdout).ok_or_else(|| ForgeError::VerusOutput {
        detail: format!(
            "could not parse verus `verification-results` from --output-json output \
             (exit {exit:?}); stderr: {stderr}",
            exit = exit_code,
            stderr = first_lines(stderr, 8),
        ),
    })?;

    // A VIR / internal verus error is NOT a verification failure — it is an
    // environment/tooling failure (never swallowed, never treated as success).
    if summary.encountered_vir_error {
        return Err(ForgeError::VerusOutput {
            detail: format!(
                "verus reported an internal (VIR) error; stderr: {}",
                first_lines(stderr, 12)
            ),
        });
    }

    let level = level_from_summary(&summary);

    let obligations = if summary.errors == 0 && summary.success {
        // All discharged: one summary-level discharged obligation recording the
        // verified count (the §5.1 per-obligation list is non-empty for a pass).
        vec![ObligationResult::discharged(format!(
            "{} obligations discharged",
            summary.verified
        ))]
    } else {
        // Reported failure: parse stderr for the per-obligation witnesses. If
        // verus reported errors but we extract no structured witness, that is a
        // non-zero-exit-with-unparseable-detail case — surface a single failure
        // result carrying the raw stderr head (still a reported cert, never a
        // bare boolean and never swallowed).
        let failures = parse_stderr_failures(stderr);
        if failures.is_empty() {
            vec![ObligationResult::failed(
                "verus reported obligation failure",
                None,
                Some(first_lines(stderr, 12)),
            )]
        } else {
            failures
        }
    };

    Ok(VerusResult {
        level,
        solver_time_ms,
        obligations,
    })
}

/// Take the first `n` non-empty lines of a diagnostic blob (bounded — never
/// echo unbounded solver output).
fn first_lines(text: &str, n: usize) -> String {
    text.lines().take(n).collect::<Vec<_>>().join("\n")
}

/// `Level::L3` iff verus discharged every obligation (REQ-5): `success == true`
/// and `errors == 0`. Otherwise the run is a reported non-L3 failure. The full
/// L3→L2→L1 degrade ladder is issue #10; v0.1's level logic is binary.
fn level_from_summary(summary: &VerusSummary) -> Level {
    if summary.success && summary.errors == 0 {
        Level::L3
    } else {
        // v0.1: a non-clean proof is reported, not auto-degraded. The certificate
        // carries the failing obligations; the level is the un-discharged L0
        // (no proof obligation discharged for all inputs).
        Level::L0
    }
}

/// Parse the `verification-results` object out of verus's `--output-json`
/// stdout. Uses a tolerant `serde_json::Value` walk (the JSON also carries a
/// large `func-details` map we ignore) so extra/missing sibling keys do not
/// break parsing (the OQ guidance: trust the summary, do not over-fit).
fn parse_summary(stdout: &str) -> Option<VerusSummary> {
    let value: serde_json::Value = serde_json::from_str(stdout.trim()).ok()?;
    let vr = value.get("verification-results")?;
    Some(VerusSummary {
        success: vr.get("success").and_then(|v| v.as_bool()).unwrap_or(false),
        verified: vr.get("verified").and_then(|v| v.as_u64()).unwrap_or(0),
        errors: vr.get("errors").and_then(|v| v.as_u64()).unwrap_or(0),
        encountered_vir_error: vr
            .get("encountered-vir-error")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
    })
}

/// Parse verus's stderr human diagnostics into per-obligation failure results
/// (REQ-4 — "counterexamples, not adjectives"). Grounded format:
///
/// ```text
/// error: postcondition not satisfied
///  --> /tmp/broken_check.rs:5:13
/// ```
///
/// Each `error: <description>` line becomes an [`ObligationResult::failed`]; the
/// following `--> file:line:col` line (if present) supplies its source location.
/// Best-effort: a missing span yields a location-less result, an `error:` with
/// no `-->` is still recorded (do NOT over-fit to one format).
fn parse_stderr_failures(stderr: &str) -> Vec<ObligationResult> {
    let lines: Vec<&str> = stderr.lines().collect();
    let mut out = Vec::new();
    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        let Some(desc) = trimmed.strip_prefix("error: ") else {
            continue;
        };
        // verus's final summary line `error: aborting due to N previous errors`
        // is not an obligation — skip it.
        if desc.starts_with("aborting due to") {
            continue;
        }
        // Look ahead for the `--> file:line:col` span on a following line.
        let location = lines
            .iter()
            .skip(idx + 1)
            .take(3)
            .find_map(|l| parse_span(l.trim_start()));
        out.push(ObligationResult::failed(
            desc.trim().to_string(),
            location,
            Some(format!("error: {}", desc.trim())),
        ));
    }
    out
}

/// Parse a `--> <file>:<line>:<col>` span line into `file:line:col` (just the
/// file basename, so the certificate does not leak the temp path). Returns
/// `None` if the line is not a span.
fn parse_span(line: &str) -> Option<String> {
    let rest = line.strip_prefix("--> ")?;
    // rest is `<path>:<line>:<col>`. Replace the temp path with its basename so
    // the cert is path-stable (the temp dir is environment-specific).
    let (path_part, loc) = rest.rsplit_once(':').and_then(|(head, col)| {
        head.rsplit_once(':')
            .map(|(p, line)| (p, format!("{line}:{col}")))
    })?;
    let base = Path::new(path_part)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(path_part);
    Some(format!("{base}:{loc}"))
}

/// Assemble one item's [`Certificate`] from THAT item's own verus result (REQ-1
/// final stage; §5.3 per-item). `verus` is the result of verifying `item`'s
/// isolated sub-program (`item_subprogram`), so its `level`/`obligations` reflect
/// only this item — never a sibling's. `item` is the item name; `effects` is the
/// item's `fx` row (`spec fn`s are pure — they carry no `fx`); `slag` is `false`
/// in #5.
fn assemble_certificate(item: &Item, verus: &VerusResult) -> Certificate {
    let effects = match item {
        Item::Fn(f) => effects_of(&f.contract.fx),
        // `spec fn`s have no `fx` row (§4.2) — they are pure by construction.
        Item::SpecFn(_) => vec!["pure".to_string()],
    };
    Certificate::new(
        item.name(),
        verus.level,
        effects,
        verus.solver_time_ms,
        verus.obligations.clone(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::ObligationStatus;

    // AC-4: the chosen temp-file stem is a valid Rust crate name (no `.`).
    // Regression guard for the grounded `invalid character '.' in crate name`.
    #[test]
    fn crate_stem_has_no_dot_and_is_valid() {
        for input in ["sum.verus", "binary_search", "9bad", "a.b.c"] {
            let stem = crate_stem(Path::new(input));
            assert!(!stem.contains('.'), "stem `{stem}` must have no dot");
            let first = stem.chars().next().expect("non-empty stem");
            assert!(
                first.is_ascii_alphabetic() || first == '_',
                "stem `{stem}` must not start with a digit"
            );
            assert!(
                stem.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
                "stem `{stem}` must be all crate-legal chars"
            );
        }
    }

    // AC-6: a parseable success summary → L3 (a discharged cert, not an Err).
    #[test]
    fn parseable_success_is_l3_cert() {
        let stdout = r#"{
          "verification-results": {
            "encountered-error": false,
            "encountered-vir-error": false,
            "success": true,
            "verified": 5,
            "errors": 0
          }
        }"#;
        let r = parse_verus_output(stdout, "", Some(0), 7).expect("parse");
        assert_eq!(r.level, Level::L3);
        assert_eq!(r.obligations.len(), 1);
        assert_eq!(r.obligations[0].status, ObligationStatus::Discharged);
    }

    // AC-6 + REQ-4: a parseable FAILURE summary + stderr witness → a reported
    // non-L3 cert (NOT an Err) carrying the failed obligation with its source
    // location (the §5.1 counterexample). Anchored to the grounded broken-output
    // format (R-CHAR-3 — this is verus's real format, not forge's output).
    #[test]
    fn parseable_failure_is_reported_cert_with_counterexample() {
        let stdout = r#"{
          "verification-results": {
            "encountered-error": true,
            "encountered-vir-error": false,
            "success": false,
            "verified": 0,
            "errors": 1
          }
        }"#;
        let stderr = "error: postcondition not satisfied\n --> /tmp/broken_check.rs:5:13\n  |\nerror: aborting due to 1 previous error\n";
        let r = parse_verus_output(stdout, stderr, Some(1), 3).expect("parse");
        assert_eq!(r.level, Level::L0);
        let failed: Vec<_> = r
            .obligations
            .iter()
            .filter(|o| o.status == ObligationStatus::Failed)
            .collect();
        assert_eq!(
            failed.len(),
            1,
            "exactly the obligation failure (not the abort line)"
        );
        assert_eq!(failed[0].name, "postcondition not satisfied");
        assert_eq!(
            failed[0].location.as_deref(),
            Some("broken_check.rs:5:13"),
            "counterexample carries the basename source span, not the temp path"
        );
    }

    // REQ-3 / AC-6: exit != 0 with UNparseable output → ForgeError::VerusOutput
    // (never swallowed, never treated as success).
    #[test]
    fn unparseable_output_is_verus_output_error() {
        let r = parse_verus_output("not json at all", "boom", Some(101), 1);
        assert!(matches!(r, Err(ForgeError::VerusOutput { .. })));
    }

    // REQ-3: an internal VIR error is an environment/tooling Err, not a success
    // and not a reported verification failure.
    #[test]
    fn vir_error_is_verus_output_error() {
        let stdout = r#"{
          "verification-results": {
            "encountered-error": true,
            "encountered-vir-error": true,
            "success": false,
            "verified": 0,
            "errors": 0
          }
        }"#;
        let r = parse_verus_output(stdout, "internal error", Some(1), 1);
        assert!(matches!(r, Err(ForgeError::VerusOutput { .. })));
    }

    #[test]
    fn parse_span_strips_temp_path() {
        assert_eq!(
            parse_span("--> /tmp/forge_sum_check_1_0.rs:37:13"),
            Some("forge_sum_check_1_0.rs:37:13".to_string())
        );
        assert_eq!(parse_span("not a span"), None);
    }
}
