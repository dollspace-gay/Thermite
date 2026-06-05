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
//!
//! ## #8 gate (per-item content-addressed proof cache, this iteration)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | proof-cache REQ-3 (lookup-then-store, per item) | SHIPPED | `check_file`'s L3 path computes `cache::cache_key(&lowered, seed, &verus_version, &thermite_version)` and `cache::load`s BEFORE `run_verus`: a HIT returns the stored cert via `Certificate::with_cached(true)` (verus SKIPPED); a MISS runs verus, assembles + `graduate_triage_clean`s the cert, `cache::store`s it, and returns `with_cached(false)`. |
//! | proof-cache REQ-5 (version-keyed) | SHIPPED | `resolve_verus_version` captures the verus version ONCE per `check_file` (the `VERUS_VERSION` pin, else `verus --version`) and `THERMITE_VERSION = env!("CARGO_PKG_VERSION")` feeds the key — a version change forces a universal MISS. |

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use thermite_syntax::{Item, Program};

use crate::cache;
use crate::cli::ForgeError;
use crate::manifest::{effects_of, Certificate, Level, ObligationResult, RejectReason};

/// The `forge` toolchain version (`.design/forge/proof-cache.md` REQ-1c/REQ-5):
/// a verdict-determining cache-key input. Sourced deterministically from the
/// crate version at compile time (R-CODE-5 — no wall-clock).
const THERMITE_VERSION: &str = env!("CARGO_PKG_VERSION");

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

    // #8 proof cache (`.design/forge/proof-cache.md`): the verus version is
    // captured ONCE per `check_file` invocation (REQ-5) so every item this run
    // keys against the SAME prover, and the cache directory is resolved once. A
    // missing/unreadable verus version is an ENVIRONMENT error, not a silent
    // empty-string key (REQ-5) — so this resolves BEFORE the per-item loop and
    // short-circuits the whole run if the prover version cannot be determined.
    let verus_version = resolve_verus_version()?;
    let cache_dir = resolve_cache_dir();

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

        // #8 proof cache (`.design/forge/proof-cache.md` REQ-1/REQ-3): the lowered
        // source is the item's content-address — the EXACT bytes verus checks
        // (§5.3 isolated sub-program). The key composes it with the four
        // verdict-determining inputs. Consult the cache BEFORE spawning verus.
        let key = cache::cache_key(&lowered, seed, &verus_version, THERMITE_VERSION);
        if let Some(stored) = cache::load(&cache_dir, &key) {
            // HIT: skip verus entirely (REQ-3, AC-1 — the decisive solver-skip).
            // The stored cert is the canonical fresh verify; mark it served from
            // cache (`cached: true`) — provenance only, oracle fields unchanged
            // (REQ-2: a hit is oracle-equal to a fresh verify).
            certs.push(stored.with_cached(true));
            continue;
        }

        // MISS: the solver runs (REQ-3). Assemble the cert exactly as the
        // non-cached path always has.
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
        // Store the fresh verify under its content address for next time (REQ-3).
        // The cache is best-effort: a write failure must NOT fail the verdict
        // (which already stands) — degrade to "uncached," never to an error
        // (REQ-6, R-CODE-2). `store` persists the canonical `cached: false`.
        let _ = cache::store(&cache_dir, &key, &cert);
        certs.push(cert.with_cached(false));
    }
    Ok(certs)
}

/// Run the L2 (Kani bounded model check) pipeline for every `fn` item in `path`,
/// returning one [`Certificate`] (at `Level::L2` on success) per `fn` in source
/// order (`.design/lower/l2-kani.md` REQ-7). This is the EXPLICIT `--level l2`
/// path: `forge check --level l2 <file>` runs it INSTEAD of the default L3 verus
/// path (`check_file`). #9 does NOT wire L2 as an automatic fallback on a verus
/// timeout (that is #10's `level_from_summary` change) — `--level l2` is a
/// deliberate choice (REQ-7 / `goal.md` R-DEFER-4).
///
/// Pipeline order (parallel to `check_file`): parse → validate → check_effects →
/// PER ITEM (`thermite_lower::lower_l2` of the item's isolated sub-program) →
/// `kani::run_kani` → an L2 `Certificate`. A `spec fn` carries no `req`/`ens`
/// contract (§4.2), so it has no L2 obligation to discharge — it is a pure shared
/// dependency woven into every `fn`'s sub-program (so a `fn` whose `ens` calls
/// `spec_sum` lowers + checks), and produces NO certificate of its own. Stages
/// short-circuit into the earliest failing stage's `ForgeError`; a reachable
/// contract `assert!` failure is a valid non-L2 certificate, not an `Err` (the
/// counterexample, §5.1).
pub fn check_l2_file(path: impl AsRef<Path>) -> Result<Vec<Certificate>, ForgeError> {
    let path = path.as_ref();
    let src = std::fs::read_to_string(path).map_err(|e| ForgeError::Io {
        path: path.display().to_string(),
        source: e,
    })?;

    // 1. parse; 2. validate; 3. effect-check — identical to the L3 path.
    let parsed = thermite_syntax::parse(&src);
    if !parsed.is_clean() {
        return Err(ForgeError::Parse(parsed.errors));
    }
    thermite_spec::validate(&parsed.program).map_err(ForgeError::Spec)?;
    thermite_lower::check_effects(&parsed.program).map_err(ForgeError::Effects)?;

    // The file's pure `spec fn`s are shared dependencies woven into every per-item
    // sub-program (so a `fn` whose `ens` references one still lowers + checks),
    // exactly as the L3 path does (§5.3 per-item isolation).
    let spec_items: Vec<Item> = parsed
        .program
        .items
        .iter()
        .filter(|i| matches!(i, Item::SpecFn(_)))
        .cloned()
        .collect();

    let mut certs = Vec::new();
    for item in &parsed.program.items {
        // Only `fn`s carry an L2 contract obligation; a `spec fn` is a pure
        // dependency with no `req`/`ens` to bounded-check (§4.2).
        let Item::Fn(f) = item else {
            continue;
        };
        let sub = item_subprogram(item, &spec_items);
        let harness = thermite_lower::lower_l2(&sub).map_err(ForgeError::Lower)?;
        let bound = thermite_lower::bound_string(&sub);
        let l2 = crate::kani::run_kani(&harness, &f.name, &bound)?;
        let effects = effects_of(&f.contract.fx);
        certs.push(crate::kani::assemble_l2_certificate(&f.name, effects, &l2));
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

/// Resolve the verus version that keys the proof cache for THIS run
/// (`.design/forge/proof-cache.md` REQ-5). Captured ONCE per `check_file` so
/// every item keys against the same prover; a verus or thermite upgrade changes
/// this string, the key, and forces a universal re-verify.
///
/// Sourcing order (deterministic, R-CODE-5 — no wall-clock):
/// 1. `VERUS_VERSION` env var, when set — the pinned/CI override. This is also
///    the hermetic-test seam: a test pins a fixed version so the key is stable
///    even when the verus BINARY is later removed (the AC-1 decisive
///    solver-skip test populates the cache, then removes verus from PATH; the
///    pinned version keeps the key matching so the HIT is served WITHOUT a verus
///    spawn).
/// 2. otherwise `verus --version` stdout (the live binary's version).
///
/// A missing/unreadable verus version (verus absent AND no `VERUS_VERSION`) is an
/// ENVIRONMENT error (`ForgeError::VerusAbsent`), NOT a silent empty-string key
/// (REQ-5) — an empty key input would let two different provers collide.
fn resolve_verus_version() -> Result<String, ForgeError> {
    if let Ok(pinned) = std::env::var("VERUS_VERSION") {
        let pinned = pinned.trim().to_string();
        if !pinned.is_empty() {
            return Ok(pinned);
        }
    }
    let output = Command::new("verus")
        .arg("--version")
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
    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if version.is_empty() {
        return Err(ForgeError::VerusOutput {
            detail: "`verus --version` produced no version string (cannot key the proof cache \
                     deterministically); set VERUS_VERSION to pin it"
                .to_string(),
        });
    }
    Ok(version)
}

/// Resolve the proof-cache directory for this run (`.design/forge/proof-cache.md`
/// REQ-6). The production default is `cache::default_cache_dir()`
/// (`target/thermite-proof-cache/`, under the git-ignored `target/`). The
/// `FORGE_CACHE_DIR` env var overrides it — the hermetic-test seam so a test can
/// point the cache at a per-test temp dir, keeping the shared `target/` cache
/// free of test pollution and tests independent of order. Deterministic
/// (R-CODE-5): the location is an explicit input, never wall-clock-derived.
fn resolve_cache_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("FORGE_CACHE_DIR") {
        if !dir.trim().is_empty() {
            return PathBuf::from(dir);
        }
    }
    cache::default_cache_dir()
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

    // #8 proof-cache AC-3 (LOCALITY) + AC-4 (DETERMINISM), exercised over the
    // REAL `item_subprogram` → `thermite_lower::lower` → `cache::cache_key`
    // pipeline (not a re-implementation): in a two-item file `f`,`g` where `g`
    // does not reference `f`, editing `f`'s body leaves `g`'s key byte-identical
    // while `f`'s key changes. Expected behavior traces to
    // `.design/forge/proof-cache.md` REQ-4/AC-3 (R-CHAR-3), not forge's output.
    #[test]
    fn cache_key_is_local_to_the_item() {
        const VERUS: &str = "verus-test-pin";
        const THERMITE: &str = "0.0.0-test";

        // Two independent `fn`s; `g` does not reference `f`. Original program.
        let src_v1 = "fn f(x: u64) -> u64\n  req x < 10\n  ens result == x\n  fx  pure\n{\n  x\n}\n\
                      fn g(y: u64) -> u64\n  req y < 10\n  ens result == y\n  fx  pure\n{\n  y\n}\n";
        // Same `g`, but `f`'s body/contract edited.
        let src_v2 = "fn f(x: u64) -> u64\n  req x < 20\n  ens result == x\n  fx  pure\n{\n  x\n}\n\
                      fn g(y: u64) -> u64\n  req y < 10\n  ens result == y\n  fx  pure\n{\n  y\n}\n";

        let key_of = |src: &str, target: &str| -> Option<String> {
            let parsed = thermite_syntax::parse(src);
            if !parsed.is_clean() {
                return None;
            }
            let spec_items: Vec<Item> = parsed
                .program
                .items
                .iter()
                .filter(|i| matches!(i, Item::SpecFn(_)))
                .cloned()
                .collect();
            let item = parsed.program.items.iter().find(|i| i.name() == target)?;
            let sub = item_subprogram(item, &spec_items);
            let lowered = thermite_lower::lower(&sub).ok()?;
            Some(cache::cache_key(&lowered, 0, VERUS, THERMITE))
        };

        let g_v1 = key_of(src_v1, "g");
        let g_v2 = key_of(src_v2, "g");
        let f_v1 = key_of(src_v1, "f");
        let f_v2 = key_of(src_v2, "f");

        assert!(
            g_v1.is_some() && f_v1.is_some(),
            "the two-item program must parse + lower (g_v1={g_v1:?}, f_v1={f_v1:?})"
        );
        // DETERMINISM (AC-4): the same item over identical input yields the same key.
        assert_eq!(
            key_of(src_v1, "g"),
            g_v1,
            "key is deterministic for unchanged input"
        );
        // LOCALITY (AC-3): editing `f` does NOT change `g`'s key.
        assert_eq!(
            g_v1, g_v2,
            "g's key is invariant under an f-only edit (locality)"
        );
        // INVALIDATION (AC-2): editing `f` DOES change `f`'s key.
        assert_ne!(f_v1, f_v2, "f's key changes when f's contract changes");
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
