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
//! | REQ-2 (verus invocation, temp file, crate-name gotcha) | SHIPPED | `run_verus` writes a `<stem>.rs` file (no `.` in the stem — `crate_stem`) INSIDE a per-run scratch DIR (`unique_scratch_dir`), spawns `verus --output-json --smt-option smt.random_seed=<seed>` with `current_dir` = the scratch dir, and removes the scratch dir WHOLESALE via the `ScratchDir` Drop guard on EVERY exit path (source + verus's compiled-binary sibling go together — blocker #53). |
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
//! ## #16 gate (boundary-fn FFI L1 path, this iteration)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | ffi REQ-5/REQ-7 (route boundary fns to L1 EARLY) | SHIPPED | `gate_fn` detects `f.boundary.is_some()` FIRST (before the slag/non-slag forks): validates a non-empty target, runs `vacuity::triage` (a)/(b)/(c) (rule (d) exempt — `triage` reads `f.boundary`), then `Certificate::boundary_l1` (`Level::L1`, `boundary: true`, `boundary_target`, NO verus). The per-item loop's `GateOutcome::BoundaryL1` short-circuits like `SlagL1` — a boundary fn NEVER reaches the L3 (verus) / L2 (kani) / #12 mutation / #14 strengthen paths, so `g` calling `f` sees only `f`'s contract (§9 composition independence, REQ-7). |
//!
//! ## #8 gate (per-item content-addressed proof cache, this iteration)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | proof-cache REQ-3 (lookup-then-store, per item) | SHIPPED | `check_file`'s L3 path computes `cache::cache_key(&lowered, seed, &verus_version, &thermite_version)` and `cache::load`s BEFORE `run_verus`: a HIT returns the stored cert via `Certificate::with_cached(true)` (verus SKIPPED); a MISS runs verus, assembles + `graduate_triage_clean`s the cert, `cache::store`s it, and returns `with_cached(false)`. |
//! | proof-cache REQ-5 (version-keyed) | SHIPPED | `resolve_verus_version` captures the verus version ONCE per `check_file` (the `VERUS_VERSION` pin, else `verus --version`) and `THERMITE_VERSION = env!("CARGO_PKG_VERSION")` feeds the key — a version change forces a universal MISS. |
//!
//! ## #11 gate (solver profiles as proof-repair prompts, this iteration)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | solver-profiles REQ-2 (capture on rlimit-hit) | SHIPPED | `invoke_verus` ALWAYS passes `--profile` + `--rlimit <rlimit>` (the pinned generous `DEFAULT_RLIMIT = 30.0`, so the corpus still PROVES L3); `--profile` emits the Z3 instantiation report on STDERR ONLY on an rlimit-hit. |
//! | solver-profiles REQ-5 (three-way classification) | SHIPPED | `classify_verus_outcome` is the deterministic three-way split: `Proved` (`success && errors==0` → L3, no profile) / `Timeout` (an error WITH a `profile::parse_profile` report present on stderr → attach the `SolverProfile`) / `Counterexample` (an error WITHOUT a profile → the #5 witness path, which ALSO absorbs the incompleteness-unknown FAST-`unknown` edge — OQ-1). Consumer: `assemble_certificate`. |
//! | solver-profiles REQ-7 (timeout cert level, distinct) | SHIPPED | the `Timeout` outcome → `Certificate::timeout` (`Level::L0` + `RejectReason { cause: "VerusTimeout" }` + the profile + a `profile::suggested_move` hint), DISTINCT from a counterexample-L0 (no profile, a `postcondition not satisfied` reason). v0.1 does not auto-degrade (#10). `--rlimit` is exposed via `cli.rs`; `check_file_with_rlimit` threads it; a non-default budget bypasses the proof cache (a timeout is never cached as proved). |
//!
//! ## #13 gate (SOLVER-backed tautology + vacuous-precondition checks, this iteration)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | solver-vacuity REQ-5 (gate AFTER #6, before L3) | SHIPPED | `check_file_with_rlimit`'s per-item loop calls `vacuity_solver::solver_vacuity_check(f, &spec_items, seed, rlimit)` AFTER `gate_fn` returns `ProceedToL3` and BEFORE the L3 lower/`run_verus`; a `Detected` short-circuits to `Certificate::rejected_vacuity` (verdict-in-cert, no L3 proof) and a `Clean` falls through to the existing L3 path. |
//! | solver-vacuity REQ-6 (graduate the two bools to solver-confirmed) | SHIPPED | a `SemanticTautology` detection sets `contract_quality.tautology = true`, a `VacuousPrecondition` sets `vacuous_precondition = true` (via `Certificate::rejected_vacuity`); a `Clean` reaches the L3 path whose `graduate_triage_clean` keeps both live-`false`, now solver-confirmed. |
//! | solver-vacuity REQ-3/REQ-7 (R-CODE-4 + determinism) | SHIPPED | a harness environment/internal verus failure propagates as a `ForgeError` from `solver_vacuity_check` (the `?`), never a silent clean; the two queries run under the same pinned `seed` + `rlimit` as the L3 path. |
//!
//! ## #10 gate (the automatic L3→L2→L1 degrade ladder, this iteration)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | degrade-ladder REQ-1/REQ-2/REQ-3 (wire the default path → the ladder) | SHIPPED | `check_file_with_options`'s per-item L3 path calls `ladder_for_timeout(f, &sub, &verus.outcome, cert)` after `assemble_certificate`: it maps the `VerusOutcome` to a `degrade::L3Verdict` and runs `degrade::run_ladder` with LIVE L2 (`lower_l2`→`run_kani`→`classify_l2_outcome`) + L1 (`lower_l1` record, OQ-3 (b)) closures. A `Proved` is unchanged (no degrade); a `Counterexample` short-circuits to a hard fail with NO L2/L1 (REQ-2 anti-cheat); a `Timeout` degrades. The EXPLICIT `--level l2` path (`check_l2_file`) is UNCHANGED. |
//! | degrade-ladder REQ-8 (subprocess failures never silently degrade) | SHIPPED | the ladder's L2/L1 closures return `Result`; a `ForgeError` (kani absent, lowering failure) propagates via the `?` in `run_ladder` and out of `ladder_for_timeout`, NEVER a degrade. A degraded cert (`lowered_assurance`) is NEVER cached (budget-dependent, parallel to the `VerusTimeout` no-cache rule). |

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use thermite_syntax::{Item, Program};

use crate::cache;
use crate::cli::ForgeError;
use crate::manifest::{effects_of, Certificate, Level, ObligationResult, RejectReason};
use crate::profile::{self, SolverProfile};

/// The `forge` toolchain version (`.design/forge/proof-cache.md` REQ-1c/REQ-5):
/// a verdict-determining cache-key input. Sourced deterministically from the
/// crate version at compile time (R-CODE-5 — no wall-clock).
const THERMITE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The pinned default solver seed (§5.3) used when no project lockfile supplies
/// one. Determinism (R-CODE-5) lives in the INPUT (this fixed seed + the
/// toolchain version), not in wall-clock state.
pub const DEFAULT_SOLVER_SEED: u64 = 0;

/// The pinned default verus `--rlimit` (SMT resource budget, roughly seconds) for
/// the L3 path (#11; `.design/forge/solver-profiles.md` REQ-5). DETERMINISTIC
/// (R-CODE-5 — a fixed input, not wall-clock) and GENEROUS: comfortably above
/// verus's own default of `10` so the conformance corpus (`sum`, `binary_search`)
/// still PROVES at `L3` (the cert-oracle is unperturbed). A LOW `--rlimit` (the
/// `forge check --rlimit 1` test lever) forces the timeout path so the three-way
/// classification is exercisable. Always paired with `--profile` so an rlimit-hit
/// emits the Z3 instantiation report on STDERR (the timeout discriminator).
pub const DEFAULT_RLIMIT: f64 = 30.0;

/// Run the full v0.1 `forge check` pipeline for every `fn` / `spec fn` item in
/// `path`, returning one [`Certificate`] per item in source order (REQ-1).
///
/// Stages short-circuit into the EARLIEST failing stage's `ForgeError`. A verus
/// obligation FAILURE is NOT an `Err`: it is a valid certificate describing the
/// failure (level != L3, with per-obligation witnesses). Only an environment /
/// internal failure (verus absent, unparseable output, IO) is an `Err`.
pub fn check_file(path: impl AsRef<Path>) -> Result<Vec<Certificate>, ForgeError> {
    check_file_with_rlimit(path, DEFAULT_RLIMIT)
}

/// The tunable knobs `cli` threads into the per-item L3 pipeline (the verus
/// resource budget #11, and the mutation kill-ratio floor #12). A single struct
/// keeps the public `check_file*` entries stable while the cli passes both levers
/// (R-SPEC-3 — no new positional contract per knob). `Default` is the pinned
/// canonical configuration ([`DEFAULT_RLIMIT`] + [`mutation::MUTATION_FLOOR`]),
/// the values `check_file` uses.
#[derive(Debug, Clone, Copy)]
pub struct CheckOptions {
    /// The verus `--rlimit` SMT resource budget (#11).
    pub rlimit: f64,
    /// The mutation kill-ratio floor (#12; `.design/forge/mutation-scoring.md`
    /// REQ-5). An item that proves L3 but scores BELOW this floor does NOT certify
    /// (`WeakContract` reject). Default [`mutation::MUTATION_FLOOR`] (0.60).
    pub mutation_floor: f64,
}

impl Default for CheckOptions {
    fn default() -> Self {
        CheckOptions {
            rlimit: DEFAULT_RLIMIT,
            mutation_floor: crate::mutation::MUTATION_FLOOR,
        }
    }
}

/// `check_file` with an explicit verus `--rlimit` (#11;
/// `.design/forge/solver-profiles.md` REQ-5). [`check_file`] delegates here with
/// the pinned generous [`DEFAULT_RLIMIT`]; `cli::run_check` passes the
/// `--rlimit <FLOAT>` flag value so a LOW budget forces the timeout path
/// (TIMEOUT cert with a `SolverProfile`), exercising the three-way
/// classification ([`classify_verus_outcome`]). The corpus PROVES at the default
/// rlimit, so the cert-oracle is unperturbed.
pub fn check_file_with_rlimit(
    path: impl AsRef<Path>,
    rlimit: f64,
) -> Result<Vec<Certificate>, ForgeError> {
    check_file_with_options(
        path,
        CheckOptions {
            rlimit,
            ..CheckOptions::default()
        },
    )
}

/// `check_file` with the full [`CheckOptions`] lever set (#11 rlimit + #12
/// mutation floor). [`check_file`] / [`check_file_with_rlimit`] delegate here;
/// `cli::run_check` passes the `--rlimit` and `--mutation-floor` flag values so a
/// non-default floor (e.g. `0.2`) flips the §7 step-4 gate (AC-3). The corpus
/// certifies at the default floor, so the cert-oracle is unperturbed.
pub fn check_file_with_options(
    path: impl AsRef<Path>,
    options: CheckOptions,
) -> Result<Vec<Certificate>, ForgeError> {
    let rlimit = options.rlimit;
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
                // A valid `#[boundary]` (FFI) item certifies L1 to-the-boundary by
                // fiat (`.design/boundary/ffi-boundary.md` REQ-5): the foreign body
                // is unproven, so it NEVER enters L3/L2/mutation/strengthening; the
                // L1 wrapper codegen is thermite-lower's `l1.rs` build-time job. No
                // verus run — so a boundary-only file does not require the prover.
                GateOutcome::BoundaryL1(cert) => {
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

        // #52 §9 composition weaving (`.design/lower/boundary-composition.md`
        // REQ-2): weave the in-file `fn`s this item transitively references into
        // its §5.3 sub-program — regular fns with their real body, boundary/slag
        // fns as `#[verifier::external_body]` signatures — so `verus` resolves the
        // callee and the caller proves THROUGH its contract (was an undefined-callee
        // L0). EMPTY for a fn referencing only spec fns / combinators (the pure
        // corpus), so the corpus cert + lowering are byte-stable (AC-4).
        let fn_deps = reachable_fn_deps(&parsed.program, item.name());
        let sub = item_subprogram(item, &spec_items, &fn_deps);
        let lowered = thermite_lower::lower(&sub).map_err(ForgeError::Lower)?;

        // #8 proof cache (`.design/forge/proof-cache.md` REQ-1/REQ-3): the lowered
        // source is the item's content-address — the EXACT bytes verus checks
        // (§5.3 isolated sub-program). The key composes it with the four
        // verdict-determining inputs. Consult the cache BEFORE spawning verus.
        //
        // #11: the cache is keyed at the CANONICAL [`DEFAULT_RLIMIT`] budget only.
        // A NON-default `--rlimit` (the timeout-forcing / exploratory lever) is a
        // budget-dependent verdict (a TIMEOUT at `--rlimit 1` is NOT the cached
        // `L3` proved at the generous default), so it BYPASSES the cache entirely
        // — neither served from nor written to it. This keeps the cache key
        // (`cache::cache_key`, four inputs) unchanged while staying sound (a
        // timeout verdict is never cached as if proved).
        // #12: a NON-default `--mutation-floor` (the AC-3 floor-flip lever) is also a
        // verdict-changing knob NOT in the cache key (the same lowered source can
        // certify under a low floor and reject `WeakContract` under the default), so
        // a non-default floor likewise BYPASSES the cache — neither served nor
        // written. The canonical-config run (default rlimit + default floor) is the
        // only one that populates / serves the shared `target/` cache, keeping the
        // four-input `cache::cache_key` unchanged while staying sound.
        let use_cache =
            rlimit == DEFAULT_RLIMIT && options.mutation_floor == crate::mutation::MUTATION_FLOOR;
        let key = cache::cache_key(&lowered, seed, &verus_version, THERMITE_VERSION);
        if use_cache {
            if let Some(stored) = cache::load(&cache_dir, &key) {
                // HIT: skip verus entirely (REQ-3, AC-1 — the decisive solver-skip).
                // The stored cert is the canonical fresh verify; mark it served from
                // cache (`cached: true`) — provenance only, oracle fields unchanged
                // (REQ-2: a hit is oracle-equal to a fresh verify). A #13
                // SOLVER-vacuity reject was cached just like a proof verdict, so a
                // HIT serves it WITHOUT re-running the two harness queries (the cache
                // hit is a verus-free path end-to-end).
                certs.push(stored.with_cached(true));
                continue;
            }
        }

        // #13 SOLVER-vacuity gate (`.design/forge/solver-vacuity.md` REQ-5): on a
        // cache MISS, AFTER #6's free structural triage passed (`ProceedToL3`,
        // above) and BEFORE the item's own L3 proof (a contract that survives the
        // syntactic checks may still be SEMANTICALLY degenerate — the §7
        // cheapest-first ordering). The two checks reuse the existing contract
        // lowering + verus driver to detect a semantic tautology (`ens` holds for
        // an arbitrary result) or an unsatisfiable precondition. A `Detected`
        // short-circuits to a non-certified `Certificate::rejected_vacuity`
        // (verdict-in-cert, the matching `contract_quality` bool SOLVER-confirmed
        // `true`) WITHOUT running the L3 proof on a known-degenerate contract; a
        // `Clean` falls through to the existing L3 path where `graduate_triage_clean`
        // keeps both bools live-`false`, now solver-confirmed (REQ-6). An
        // environment / internal verus failure on a harness query surfaces a
        // `ForgeError` (R-CODE-4), never a silent clean. The gate runs INSIDE the
        // cache-miss branch so the deterministic #13 verdict is CACHED with the item
        // (OQ-2): a later HIT serves the cached reject / clean cert without a verus
        // spawn (the cache-hit verus-free invariant, proof-cache.md AC-1).
        if let Item::Fn(f) = item {
            if let crate::vacuity_solver::SolverVacuityVerdict::Detected { cause } =
                crate::vacuity_solver::solver_vacuity_check(f, &spec_items, seed, rlimit)?
            {
                let (taut, vac) = match cause {
                    crate::vacuity_solver::SolverVacuityCause::SemanticTautology => (true, false),
                    crate::vacuity_solver::SolverVacuityCause::VacuousPrecondition => (false, true),
                };
                let cert = Certificate::rejected_vacuity(
                    f.name.clone(),
                    effects_of(&f.contract.fx),
                    RejectReason {
                        cause: cause.tag().to_string(),
                        detail: cause.detail(),
                    },
                    taut,
                    vac,
                );
                // A #13 reject is a SETTLED, deterministic verdict (a function of the
                // lowered contract + seed + versions), so it is cached like a
                // counterexample cert: a re-check serves the HIT without re-running
                // the harness queries. Best-effort store (a write failure never fails
                // the verdict — R-CODE-2), at the canonical budget only.
                if use_cache {
                    let _ = cache::store(&cache_dir, &key, &cert);
                }
                certs.push(cert.with_cached(false));
                continue;
            }
        }

        // CLEAN (or a `spec fn`, which carries no contract to check): the solver
        // runs the real L3 proof (REQ-3). Assemble the cert exactly as the
        // non-cached path always has.
        let verus = run_verus(&sub, &lowered, seed, rlimit)?;
        let cert = assemble_certificate(item, &verus);

        // #10 AUTOMATIC DEGRADE LADDER (`.design/forge/degrade-ladder.md`, the
        // DEFAULT `forge check` path). On a `VerusOutcome::Timeout` (verus could
        // not PROVE within budget — INCONCLUSIVE) the v0.1 behavior was to emit the
        // `VerusTimeout` L0 cert and STOP; #10 replaces that STOP with the ladder:
        // attempt L2 (kani) and, on an under-bound, drop to L1. A
        // `VerusOutcome::Counterexample` (verus DISPROVED the contract — a real
        // bug) is a HARD FAIL and NEVER degrades (REQ-2 anti-cheat); the ladder
        // short-circuits it. The ladder runs ONLY for an `Item::Fn` (a `spec fn`
        // carries no `req`/`ens` to bound-check at L2). An ENVIRONMENT failure on a
        // lower rung (kani absent / unparseable) propagates as a `ForgeError`
        // (REQ-8), never a silent degrade.
        let cert = if let Item::Fn(f) = item {
            ladder_for_timeout(f, &sub, &verus.outcome, cert)?
        } else {
            cert
        };

        // A non-slag `fn` that reached the L3 path passed triage — graduate the
        // §7.1 `contract_quality` bools to asserted live-`false` (REQ-6 / AC-7). A
        // `spec fn` carries no contract, so triage does not apply and the bools
        // stay forward-declared. A TIMEOUT cert (`VerusTimeout`) is NOT graduated:
        // its triage bools stay forward-declared (nothing about the contract's
        // syntactic quality was newly confirmed by a budget exhaustion).
        let cert = if matches!(item, Item::Fn(_)) && cert.reject.is_none() {
            cert.graduate_triage_clean()
        } else {
            cert
        };

        // #12 §7 step 4 — MUTATION SCORING, AFTER a successful L3 proof of the REAL
        // body (`.design/forge/mutation-scoring.md` REQ-7). Reached ONLY on a
        // `VerusOutcome::Proved` real body: the cert is `Level::L3` with no reject.
        // A non-proving item (counterexample / timeout / a `spec fn`) is never
        // scored — §7's premise is "mutate a KNOWN-GOOD body". Each mutant's
        // re-verify is content-addressed through the SAME proof cache (#8), so a
        // re-`forge check` re-scores from the cache cheaply. A sub-floor kill ratio
        // turns the cert into a `WeakContract` reject (verdict-in-cert); a met floor
        // graduates `mutants_killed`/`survivor` on the certified cert.
        let cert = if let Item::Fn(f) = item {
            if cert.level == Level::L3 && cert.reject.is_none() {
                let score = mutation_score(
                    f,
                    &spec_items,
                    &fn_deps,
                    seed,
                    rlimit,
                    &verus_version,
                    &cache_dir,
                    use_cache,
                )?;
                let effects = effects_of(&f.contract.fx);
                if score.meets_floor(options.mutation_floor) {
                    // #14 §7 step 5 — STRENGTHENING PROBE
                    // (`.design/forge/strengthening-probes.md` REQ-5). The item is a
                    // SETTLED L3-certified + scored item (level L3, no reject, a
                    // `MutationScore` produced), so the probe runs: it generates the
                    // frozen candidate stronger-`ens` set, verifies each against the
                    // REAL body via the SAME `run_verus` + #8 cache, keeps the
                    // verifying + strictly-stronger ones, and attaches them as
                    // ADVISORY suggestions. The probe NEVER changes the verdict
                    // (`with_strengthening` only adds the additive field + the
                    // `suggested_move` headline; `level`/`reject`/oracle subset
                    // untouched, REQ-4). An environment failure on a candidate verus
                    // run propagates (R-CODE-4).
                    let scored_cert = cert
                        .with_mutation_score(score.mutants_killed_string(), score.survivor.clone());
                    let suggestions = strengthen_certificate(
                        f,
                        &spec_items,
                        &fn_deps,
                        &score,
                        seed,
                        rlimit,
                        &verus_version,
                        &cache_dir,
                        use_cache,
                    )?;
                    scored_cert.with_strengthening(suggestions)
                } else {
                    // Sub-floor: the contract under-constrains the body. Below the
                    // floor a survivor is normally present (a < 1.0 ratio means ≥1
                    // mutant survived). The one exception is the 0/0 backstop (#48):
                    // NO mutant could be scored (un-synthesizable return type) — a
                    // contract that cannot be mutation-validated has not met the §7
                    // bar, so it is gated with an explicit unscoreable prompt.
                    let survivor = score.survivor.clone().unwrap_or_else(|| {
                        if score.scored == 0 {
                            "the contract could not be mutation-validated (no \
                             scoreable mutant); it does not meet the §7 floor"
                                .to_string()
                        } else {
                            "a mutant survived the contract".to_string()
                        }
                    });
                    Certificate::rejected_weak_contract(
                        f.name.clone(),
                        effects,
                        score.mutants_killed_string(),
                        survivor,
                    )
                }
            } else {
                cert
            }
        } else {
            cert
        };

        // #11/#10: a TIMEOUT cert (budget-dependent, `VerusTimeout`) and any cert
        // the #10 ladder produced by DEGRADING a timeout (`lowered_assurance`) are
        // NEVER cached — they are not settled verdicts (a larger budget might prove
        // the item at L3, so a degraded L2/L1 cert must not pollute the
        // canonical-budget cache as if it were the final word; degrade-ladder.md
        // "Why a Timeout cert is never cached as proved"). Only a settled cert
        // (proved L3 / counterexample) at the default budget is stored.
        if cert.reject.as_ref().map(|r| r.cause.as_str()) == Some("VerusTimeout")
            || cert.lowered_assurance
        {
            certs.push(cert.with_cached(false));
            continue;
        }
        // Store the fresh verify under its content address for next time (REQ-3).
        // The cache is best-effort: a write failure must NOT fail the verdict
        // (which already stands) — degrade to "uncached," never to an error
        // (REQ-6, R-CODE-2). `store` persists the canonical `cached: false`. Only
        // at the canonical [`DEFAULT_RLIMIT`] (#11): a non-default budget's verdict
        // is not cached (it bypassed the lookup too).
        if use_cache {
            let _ = cache::store(&cache_dir, &key, &cert);
        }
        certs.push(cert.with_cached(false));
    }

    // #17 §9 END-TO-END vs TO-THE-BOUNDARY classification
    // (`.design/forge/e2e-vs-boundary.md` REQ-1/REQ-2/REQ-3). Run the structural
    // transitive-call-closure analysis ONCE over the whole file's program (it is a
    // pure function of the parsed `Program`, R-CODE-5), then attach each fn's
    // assurance scope to its certificate. ORTHOGONAL to the verdict (REQ-5): the
    // scope is recorded ALONGSIDE the already-achieved level — a fn whose body
    // SMT-proved at L3 but whose closure crosses a `#[boundary]`/`#[slag]` fn keeps
    // `Level::L3` AND records `ToBoundary { via }`. The classification keys on the
    // in-file `#[boundary]`/`#[slag]` NODE (the §9 composition rule), never a
    // sibling's verdict, so it does not perturb any oracle-stable level.
    let scopes = crate::closure::classify(&parsed.program);
    let certs = certs
        .into_iter()
        .map(|cert| match scopes.get(&cert.item) {
            Some(scope) => cert.with_assurance_scope(scope.clone()),
            // A cert whose item has no node (defensive — every checked item is a
            // node) keeps its `None` scope, which `oracle_subset` reads as
            // end-to-end (the golden-stable default).
            None => cert,
        })
        .collect();
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
        // The explicit `--level l2` (kani) path does NOT weave the §9 composition
        // deps: #52's external_body arm lives in the L3 `lower`, not `lower_l2`, and
        // the composition oracle (`conformance/composition`) is L3-only. A boundary
        // caller at L2 is out of #52's v0.1 scope, so this stays the prior shape
        // (no fn-dep weaving) to keep the explicit-L2 behavior byte-stable.
        let sub = item_subprogram(item, &spec_items, &[]);
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
    /// A valid `#[boundary]` (FFI) item: certify L1 to-the-boundary (foreign body
    /// unproven, contract enforced at the crossing; no verus run) — the cert
    /// (`.design/boundary/ffi-boundary.md` REQ-5).
    BoundaryL1(Certificate),
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

    // #16 BOUNDARY (FFI) path, detected FIRST (`.design/boundary/ffi-boundary.md`
    // REQ-5, §9): a `#[boundary("crate::path")]` fn's FOREIGN body is unproven, so
    // it NEVER enters the L3 (verus) / L2 (kani) / mutation / strengthening paths —
    // it certifies at L1 to-the-boundary. The §7.1 (a)/(b)/(c) triage STILL applies
    // (slag-adjacent: it exempts PROVING the body, not STATING a non-vacuous
    // contract — a `#[boundary]` fn with `ens true` is still rejected). Rule (d) is
    // exempt (a foreign body's effects are trusted-by-fiat, OQ-4 — `triage` reads
    // `f.boundary` and skips (d)). The target's non-emptiness is validated here:
    // an empty `#[boundary("")]` target is a contract-certification reject.
    if let Some(boundary_attr) = f.boundary.as_ref() {
        let target = boundary_attr.target.trim();
        if target.is_empty() {
            return GateOutcome::Rejected(Certificate::rejected(
                f.name.clone(),
                effects,
                false,
                RejectReason {
                    cause: "BoundaryTargetEmpty".to_string(),
                    detail: "a `#[boundary(\"...\")]` attribute must name a non-empty foreign \
                             `crate::path` target"
                        .to_string(),
                },
            ));
        }
        return match crate::vacuity::triage(f) {
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
            // Triage clean → certify L1 to-the-boundary (no verus): the contract is
            // enforced at the crossing by `thermite_lower::l1`'s boundary wrapper.
            crate::vacuity::VacuityVerdict::Passed => GateOutcome::BoundaryL1(
                Certificate::boundary_l1(f.name.clone(), effects, target.to_string()),
            ),
        };
    }

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
/// - A `fn` is verified against itself, the file's `spec fn`s (the pure shared
///   dependencies its contract may reference), AND the in-file `fn`s its body
///   TRANSITIVELY references (`fn_deps`, the #52 §9 composition weaving). A
///   regular reachable fn is woven with its REAL body (fully lowered + proved);
///   a `#[boundary]`/`#[slag]` reachable fn is woven as a
///   `#[verifier::external_body]` signature (`thermite_lower::lower`'s
///   composition arm), so `verus` resolves the foreign callee and the caller
///   proves THROUGH its contract (§9). Its obligations stay its own (§5.3) — a
///   sibling fn NOT in the closure never enters, so an unrelated sibling's
///   failure cannot leak in.
/// - A `spec fn` carries no `req`/`ens`/`fx` contract (`ast.rs` `SpecFnItem`,
///   §4.2): there is no L3 proof obligation to discharge, only well-formedness
///   (the `decreases` measure). It is verified against the set of `spec fn`s
///   alone (which already contains it), so a mutually-recursive spec fn still
///   resolves. The resulting cert records the spec fn's well-formedness as its
///   own discharged result — never a neighbor `fn`'s counterexample. A `spec fn`
///   has no `fn_deps` (its body can call only spec fns / combinators, §4.2).
fn item_subprogram(item: &Item, spec_items: &[Item], fn_deps: &[Item]) -> Program {
    match item {
        // The `fn` plus all pure spec-fn dependencies plus the transitively
        // reachable in-file `fn` dependencies (#52), then the item itself last
        // (so a forward reference resolves; the lowerer dedups combinator defs
        // regardless of order).
        Item::Fn(_) => {
            let mut items = spec_items.to_vec();
            items.extend(fn_deps.iter().cloned());
            items.push(item.clone());
            Program { items }
        }
        // Spec fns verified together (mutual recursion); `spec_items` already
        // includes `item`. A spec fn has no `fn` dependencies to weave (§4.2).
        Item::SpecFn(_) => Program {
            items: spec_items.to_vec(),
        },
    }
}

/// The in-file `Item::Fn`s a fn named `start` transitively references — the §9
/// composition dependencies woven into `start`'s sub-program (#52). Resolves
/// `closure::reachable_in_file_fns` (the reused #17 call-graph walk) to the
/// matching `Item::Fn` clones from `program`, in source order (DETERMINISTIC,
/// R-CODE-5). EXCLUDES `start` itself and every `spec fn` (the latter is woven by
/// the separate `spec_items` set — no duplication). For a fn with no in-file fn
/// references (e.g. the pure corpus `sum`, which calls only the `spec fn`
/// `spec_sum`) this is EMPTY, so the §52 weaving is a no-op and no external_body
/// is ever emitted (the AC-4 corpus-unaffected / honesty-gate invariant).
fn reachable_fn_deps(program: &Program, start: &str) -> Vec<Item> {
    let names = crate::closure::reachable_in_file_fns(program, start);
    program
        .items
        .iter()
        .filter(|i| matches!(i, Item::Fn(_)) && names.contains(i.name()))
        .cloned()
        .collect()
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

/// The parsed result of one verus run: the deterministically-classified outcome
/// (#11 three-way, REQ-5) plus the wall-clock solver time (REQ-6, oracle-excluded).
#[derive(Debug, Clone)]
struct VerusResult {
    outcome: VerusOutcome,
    solver_time_ms: u64,
}

/// The THREE-WAY classification of one verus run (#11;
/// `.design/forge/solver-profiles.md` REQ-5). DETERMINISTIC; the profile CONTENT
/// it attaches on a timeout is not (§5.3).
#[derive(Debug, Clone)]
enum VerusOutcome {
    /// (a) PROVED: `success == true && errors == 0` → `Level::L3`, one discharged
    /// summary obligation, NO profile.
    Proved { verified: u64 },
    /// (c) TIMEOUT / rlimit-exceeded: an error run WHOSE STDERR carries a
    /// `--profile` Z3 instantiation report (the timeout discriminator — `--profile`
    /// reports ONLY on an rlimit-hit). Carries the parsed `SolverProfile`.
    Timeout {
        profile: SolverProfile,
        detail: String,
    },
    /// (b) COUNTEREXAMPLE / the failure path: an error run WITHOUT a profile (the
    /// existing #5 witness path, e.g. `postcondition not satisfied`). This bucket
    /// ALSO absorbs the incompleteness-unknown edge (an `unknown` returned FAST
    /// without exhausting the rlimit → no profile → treated as the failure path,
    /// OQ-1), so a timeout is never silently reported as success (R-CODE-4).
    Counterexample { obligations: Vec<ObligationResult> },
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

/// A per-run scratch DIRECTORY for one verus invocation, removed WHOLESALE on
/// `Drop` (blocker #53). verus compiles the lowered `.rs` into a sibling binary
/// (`<stem>`, no extension, ~4.3M) in its working directory; the v0.1 driver ran
/// verus in the SHARED `std::env::temp_dir()` and cleaned ONLY the `.rs` source,
/// so a verus run that ERRORED mid-compile orphaned the binary → unbounded `/tmp`
/// growth under sustained multi-agent fresh-verification (the ENOSPC seen during
/// #18/#20). The fix: every verus run gets its OWN scratch dir (the `.rs` source,
/// verus's compiled-binary sibling, and ANY other verus artifact all land
/// inside), and this guard's `Drop` does a `remove_dir_all` that fires on EVERY
/// exit path — success, a reported counterexample, OR a `?` early-return on an
/// environment/IO error. Cleanup is best-effort (`let _ =`), never a panic
/// (R-CODE-2): a removal failure must not mask the real verus result.
///
/// This mirrors the L2 (kani) driver's discipline, which already runs in a
/// per-run scratch CRATE removed wholesale via `kani.rs::run_kani`'s
/// `remove_dir_all(&crate_dir)` — so the kani path does NOT share this leak. The
/// shared cause's class ("an external-tool invocation must run in a scratch dir
/// removed wholesale, even on error") is now uniform across both rungs.
struct ScratchDir {
    path: PathBuf,
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        // Best-effort wholesale removal: the `.rs` source AND verus's compiled
        // binary AND any other artifact go together. A failure here NEVER fails
        // the verdict (R-CODE-2) — degrade to "left on disk", never to a panic.
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Write the lowered source to a `.rs` file with a valid-crate-name stem (REQ-2)
/// INSIDE a per-run scratch DIRECTORY, spawn verus there (`current_dir` is the
/// scratch dir, so verus's compiled-binary sibling lands inside it), parse
/// the result, and remove the scratch dir WHOLESALE on every exit path
/// (blocker #53 — the `ScratchDir` Drop guard cleans even on a `?` early-return).
/// Verus absent on spawn → `ForgeError::VerusAbsent` (REQ-6); a non-zero exit
/// with parseable failure → a reported failure cert; unparseable output →
/// `ForgeError::VerusOutput` (REQ-3).
fn run_verus(
    program: &Program,
    lowered: &str,
    seed: u64,
    rlimit: f64,
) -> Result<VerusResult, ForgeError> {
    // Name the scratch dir + `.rs` after the first item (deterministic) so
    // concurrent runs over different files do not collide; fall back to a fixed
    // stem. The crate-name gotcha (REQ-2 / AC-4) is unchanged: the `.rs` stem is
    // still the no-`.` `crate_stem`, so verus's crate-name derivation succeeds.
    let label = program.items.first().map(|i| i.name()).unwrap_or("forge");
    let stem = crate_stem(Path::new(label));
    let scratch = ScratchDir {
        path: unique_scratch_dir(&stem),
    };
    std::fs::create_dir_all(&scratch.path).map_err(|e| ForgeError::Io {
        path: scratch.path.display().to_string(),
        source: e,
    })?;
    let tmp = scratch.path.join(format!("{stem}.rs"));
    std::fs::write(&tmp, lowered).map_err(|e| ForgeError::Io {
        path: tmp.display().to_string(),
        source: e,
    })?;

    // The `?` here still cleans up: `scratch` is dropped on the early-return.
    let result = invoke_verus(&scratch.path, &tmp, seed, rlimit);

    // `scratch` drops at the end of this scope (and on the `?` above), removing
    // the source + verus's compiled binary + everything WHOLESALE (blocker #53).
    drop(scratch);

    result
}

/// Spawn verus and parse its output. Split from `run_verus` so the scratch dir is
/// always cleaned up regardless of outcome. `cwd` is the per-run scratch
/// directory (blocker #53): verus's working-directory artifacts — most notably
/// the ~4.3M compiled-binary sibling — land THERE, so the caller's `ScratchDir`
/// guard removes them wholesale.
///
/// #11: ALWAYS passes `--profile` + the pinned `--rlimit <rlimit>`. `--profile`
/// emits the Z3 instantiation report on STDERR ONLY when the rlimit is exceeded
/// (the timeout discriminator); a clean proof / a fast counterexample emits no
/// report, so its PRESENCE is the timeout signal that
/// [`classify_verus_outcome`] keys on.
fn invoke_verus(cwd: &Path, tmp: &Path, seed: u64, rlimit: f64) -> Result<VerusResult, ForgeError> {
    let started = Instant::now();
    let output = Command::new("verus")
        .arg("--output-json")
        .arg("--profile")
        .arg("--rlimit")
        .arg(format!("{rlimit}"))
        .arg("--smt-option")
        .arg(format!("smt.random_seed={seed}"))
        .arg(tmp)
        .current_dir(cwd)
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

    let outcome = classify_verus_outcome(&stdout, &stderr, exit_code)?;
    Ok(VerusResult {
        outcome,
        solver_time_ms,
    })
}

/// Build a unique per-run scratch DIRECTORY path for one verus invocation
/// (blocker #53). Uniqueness uses the process id + a monotonic counter — NOT
/// wall-clock — so the path varies between concurrent runs without violating
/// R-CODE-5 (determinism is a property of the CERTIFICATE, not the scratch path;
/// §check.md REQ-2 "determinism is in the INPUT, not the path"). The directory
/// (not a bare `.rs` file) is what gets removed wholesale, taking the `.rs`
/// source AND verus's compiled-binary sibling with it.
fn unique_scratch_dir(stem: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    std::env::temp_dir().join(format!("forge_{stem}_{pid}_{n}"))
}

/// Classify one verus run THREE ways DETERMINISTICALLY (#11;
/// `.design/forge/solver-profiles.md` REQ-5). The JSON `verification-results`
/// summary on stdout cannot tell a timeout from a counterexample (both report
/// `success: false, errors: 1` — OQ-1), so the discriminator is the PRESENCE of
/// a `--profile` Z3 instantiation report on STDERR (verus emits it ONLY on an
/// rlimit-hit):
///
/// - (a) PROVED (`success && errors == 0`) → [`VerusOutcome::Proved`] → `Level::L3`.
/// - (c) error WITH a profile report present → [`VerusOutcome::Timeout`]: parse
///   the `SolverProfile`, attach it (the cert is `Level::L0` + `VerusTimeout`).
/// - (b) error WITHOUT a profile → [`VerusOutcome::Counterexample`] (the existing
///   #5 witness path). This ALSO absorbs the documented incompleteness-unknown
///   edge (an `unknown` returned FAST without exhausting the rlimit → no profile →
///   the failure path), so a timeout is never silently reported as success
///   (R-CODE-4 — degrade/report, do not treat as success).
///
/// The classification is DETERMINISTIC; the profile CONTENT (instantiation
/// counts) is not (§5.3). A VIR / internal error and unparseable output stay
/// environment errors (`ForgeError::VerusOutput`), never a verification verdict.
fn classify_verus_outcome(
    stdout: &str,
    stderr: &str,
    exit_code: Option<i32>,
) -> Result<VerusOutcome, ForgeError> {
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

    // (a) PROVED.
    if summary.success && summary.errors == 0 {
        return Ok(VerusOutcome::Proved {
            verified: summary.verified,
        });
    }

    // (c) TIMEOUT: an error run whose stderr carries a `--profile` instantiation
    // report. `--profile` reports ONLY on an rlimit-hit, so the report's PRESENCE
    // is the timeout discriminator (REQ-5 / the doc's Architecture).
    if let Some(profile) = profile::parse_profile(stderr) {
        let detail = format!(
            "verus exhausted its SMT resource budget (rlimit) before proving this item; \
             {} total quantifier instantiations observed (see solver_profile / suggested_move)",
            profile.total_instantiations
        );
        return Ok(VerusOutcome::Timeout { profile, detail });
    }

    // (b) COUNTEREXAMPLE / the failure path (no profile report). Parse stderr for
    // the per-obligation witnesses (the existing #5 path). If verus reported
    // errors but no structured witness is extractable, surface a single failure
    // carrying the raw stderr head (still a reported cert, never swallowed).
    let failures = parse_stderr_failures(stderr);
    let obligations = if failures.is_empty() {
        vec![ObligationResult::failed(
            "verus reported obligation failure",
            None,
            Some(first_lines(stderr, 12)),
        )]
    } else {
        failures
    };
    Ok(VerusOutcome::Counterexample { obligations })
}

/// Take the first `n` non-empty lines of a diagnostic blob (bounded — never
/// echo unbounded solver output).
fn first_lines(text: &str, n: usize) -> String {
    text.lines().take(n).collect::<Vec<_>>().join("\n")
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
/// isolated sub-program (`item_subprogram`), so its outcome reflects only this
/// item — never a sibling's. `item` is the item name; `effects` is the item's
/// `fx` row (`spec fn`s are pure — they carry no `fx`).
///
/// #11 — the THREE-WAY outcome maps to three certificate shapes:
/// - [`VerusOutcome::Proved`] → `Level::L3`, one discharged summary obligation,
///   NO profile.
/// - [`VerusOutcome::Timeout`] → `Certificate::timeout` (`Level::L0` +
///   `RejectReason { cause: "VerusTimeout" }` + the `SolverProfile` + the
///   profile-derived `suggested_move`), DISTINCT from a counterexample.
/// - [`VerusOutcome::Counterexample`] → `Level::L0` with the per-obligation
///   witnesses (the existing #5 path), NO profile.
fn assemble_certificate(item: &Item, verus: &VerusResult) -> Certificate {
    let effects = match item {
        Item::Fn(f) => effects_of(&f.contract.fx),
        // `spec fn`s have no `fx` row (§4.2) — they are pure by construction.
        Item::SpecFn(_) => vec!["pure".to_string()],
    };
    match &verus.outcome {
        VerusOutcome::Proved { verified } => Certificate::new(
            item.name(),
            Level::L3,
            effects,
            verus.solver_time_ms,
            vec![ObligationResult::discharged(format!(
                "{verified} obligations discharged"
            ))],
        ),
        VerusOutcome::Timeout { profile, detail } => {
            // The profile-derived proof-repair hint populates the reserved
            // `suggested_move` slot (#11 REQ-4); the structured profile lands in
            // the additive `solver_profile` field. Both oracle-excluded (§5.3).
            let suggested = profile::suggested_move(profile);
            Certificate::timeout(
                item.name(),
                effects,
                verus.solver_time_ms,
                profile.clone(),
                suggested,
                detail.clone(),
            )
        }
        VerusOutcome::Counterexample { obligations } => Certificate::new(
            item.name(),
            Level::L0,
            effects,
            verus.solver_time_ms,
            obligations.clone(),
        ),
    }
}

/// Drive the #10 automatic degrade ladder for ONE `fn` whose L3 verus run is in
/// `outcome` (`.design/forge/degrade-ladder.md` REQ-1/REQ-2/REQ-3). `l3_cert` is
/// the cert `assemble_certificate` already built from `outcome` (the L3 cert on
/// `Proved`, the `VerusTimeout` cert on `Timeout`, the counterexample cert on
/// `Counterexample`). Returns the ACHIEVED-level cert:
///
/// - `Proved` → the L3 cert UNCHANGED (no degrade — the common path).
/// - `Counterexample` → the counterexample cert UNCHANGED — HARD FAIL, NO L2/L1
///   rung is attempted (REQ-2 anti-cheat: the ladder short-circuits falsity).
/// - `Timeout` → run the ladder: lower + kani the SAME item (L2); on a verified
///   bound certify L2 + lowered-assurance; on an under-bound drop to L1 (a RECORDED
///   `Level::L1` cert, OQ-3 (b) — `lower_l1`'s runtime-check emission stays a
///   build-time concern); on an L2 counterexample HARD FAIL (REQ-2, 2nd rung).
///
/// An ENVIRONMENT failure on a lower rung (kani absent, unparseable output,
/// lowering failure) propagates as a `ForgeError` (REQ-8), NEVER a silent degrade.
/// The L2/L1 closures are LAZY — they run ONLY on the timeout edge, so the common
/// `Proved` path never spawns kani.
fn ladder_for_timeout(
    f: &thermite_syntax::FnItem,
    sub: &Program,
    outcome: &VerusOutcome,
    l3_cert: Certificate,
) -> Result<Certificate, ForgeError> {
    let l3 = match outcome {
        // PROVED / COUNTEREXAMPLE are TERMINAL — the ladder returns the existing
        // cert with NO lower rung (REQ-2: a counterexample never degrades).
        VerusOutcome::Proved { .. } => crate::degrade::L3Verdict::Proved(l3_cert),
        VerusOutcome::Counterexample { .. } => crate::degrade::L3Verdict::Counterexample(l3_cert),
        // TIMEOUT — the SOLE degrade trigger. Carry the `VerusTimeout` reason onto
        // the lower rung (REQ-4). The reason is the `l3_cert`'s reject (the
        // `Certificate::timeout` `RejectReason { cause: "VerusTimeout", .. }`).
        VerusOutcome::Timeout { detail, .. } => {
            let reason = l3_cert.reject.clone().unwrap_or_else(|| RejectReason {
                cause: "VerusTimeout".to_string(),
                detail: detail.clone(),
            });
            crate::degrade::L3Verdict::Timeout { reason }
        }
    };

    let effects = effects_of(&f.contract.fx);
    let l1_effects = effects.clone();
    let fname = f.name.clone();

    crate::degrade::run_ladder(
        l3,
        // The L2 rung (lazy): lower the SAME item to a kani harness, run the real
        // kani binary, classify (the OQ-2 split). An environment failure → Err.
        || {
            let harness = thermite_lower::lower_l2(sub).map_err(ForgeError::Lower)?;
            let bound = thermite_lower::bound_string(sub);
            let l2 = crate::kani::run_kani(&harness, &fname, &bound)?;
            let verdict = crate::kani::classify_l2_outcome(&l2);
            let cert = crate::kani::assemble_l2_certificate(&fname, effects, &l2);
            Ok(crate::degrade::L2Attempt { verdict, cert })
        },
        // The L1 fallback rung (lazy): RECORD the achieved `Level::L1` (OQ-3 (b)).
        // The contract's runtime-check EMISSION is `thermite_lower::lower_l1`'s
        // build-time job, not the verdict-aggregator's — exactly the
        // `Certificate::slag_l1` precedent (records L1 without running a prover).
        // `lower_l1` is invoked here only to CONFIRM the contract lowers to runtime
        // checks (so the recorded L1 is real, never a fiat the build cannot honor);
        // a lowering failure is an environment error (REQ-8), never a silent drop.
        || {
            thermite_lower::lower_l1(sub).map_err(ForgeError::Lower)?;
            Ok(Certificate::new(
                f.name.clone(),
                Level::L1,
                l1_effects,
                0,
                vec![ObligationResult::discharged(
                    "contract recorded at L1 (runtime checks emitted at build by \
                     thermite_lower::lower_l1); L3 proof and L2 bounded check both \
                     inconclusive within budget",
                )],
            ))
        },
    )
}

/// Score the frozen mutant set of `f` against its OWN (unchanged) contract (#12
/// §7 step 4; `.design/forge/mutation-scoring.md` REQ-3/REQ-4/REQ-5/REQ-7).
/// Called from the per-item L3 path ONLY after `f`'s REAL body proved L3 (the
/// caller gates on `cert.level == L3 && reject.is_none()`).
///
/// For each mutant (`mutation::generate`, the frozen + ordered + capped set):
/// 1. weave it into the same per-item sub-program shape ([`item_subprogram`]) and
///    lower via the existing `thermite_lower::lower`. A mutant that FAILS to lower
///    is DROPPED from the denominator (not scored — OQ-5), never an `Err` that
///    fails the gate.
/// 2. content-address the lowered mutant via the SAME proof cache (#8;
///    `cache::cache_key`/`load`/`store`) — a HIT serves the stored verdict without
///    spawning verus, so a re-`forge check` re-scores cheaply (REQ-7). The cache
///    is consulted ONLY at the canonical config (`use_cache`); a non-default
///    rlimit / floor run bypasses it (the caller's invariant).
/// 3. run the existing `run_verus` on a MISS and classify (REQ-4): a `Proved`
///    mutant SURVIVED (the contract is too weak); a `Counterexample` / `Timeout`
///    mutant is KILLED. An ENVIRONMENT / VIR failure surfaces a `ForgeError`
///    (R-CODE-4), never a silent kill or survive.
///
/// Returns the [`mutation::MutationScore`] (`killed`/`scored` + the first
/// surviving mutant's description). DETERMINISTIC (REQ-8): the mutant list is a
/// pure function of the AST + the frozen table, and each mutant verdict is the
/// same deterministic verus run the L3 path + cache rely on.
#[allow(
    clippy::too_many_arguments,
    reason = "the L3-path seams (spec items, \
    seed, rlimit, verus version, cache dir + enable) are all verdict-determining \
    inputs threaded from check_file; bundling them would obscure the per-mutant \
    cache-key composition"
)]
fn mutation_score(
    f: &thermite_syntax::FnItem,
    spec_items: &[Item],
    fn_deps: &[Item],
    seed: u64,
    rlimit: f64,
    verus_version: &str,
    cache_dir: &Path,
    use_cache: bool,
) -> Result<crate::mutation::MutationScore, ForgeError> {
    let mutants = crate::mutation::generate(f, seed);
    let mut killed = 0usize;
    let mut scored = 0usize;
    let mut survivor: Option<String> = None;

    for mutant in mutants {
        let item = Item::Fn(mutant.item);
        // Weave the SAME §9 composition deps as the original `f` (#52): a mutant
        // body still references the original's boundary/regular callees, so they
        // must resolve in the mutant's sub-program too.
        let sub = item_subprogram(&item, spec_items, fn_deps);
        // OQ-5: a mutant that fails to LOWER (structurally degenerate) is DROPPED
        // from the denominator, never an `Err` that fails the whole gate.
        let lowered = match thermite_lower::lower(&sub) {
            Ok(s) => s,
            Err(_) => continue,
        };

        // Content-address the mutant exactly as the L3 path does (#8). A mutant's
        // verdict is a deterministic function of its lowered source + seed +
        // versions, so it caches like any item.
        let key = cache::cache_key(&lowered, seed, verus_version, THERMITE_VERSION);
        let proved = if use_cache {
            if let Some(stored) = cache::load(cache_dir, &key) {
                mutant_cert_is_survivor(&stored)
            } else {
                let verus = run_verus(&sub, &lowered, seed, rlimit)?;
                let cert = assemble_certificate(&item, &verus);
                let _ = cache::store(cache_dir, &key, &cert);
                mutant_cert_is_survivor(&cert)
            }
        } else {
            let verus = run_verus(&sub, &lowered, seed, rlimit)?;
            mutant_outcome_is_survivor(&verus.outcome)
        };

        scored += 1;
        if crate::mutation::classify_mutant(proved) == crate::mutation::MutantOutcome::Killed {
            killed += 1;
        } else if survivor.is_none() {
            // The FIRST survivor in deterministic enumeration order (REQ-2) is the
            // representative strengthening prompt.
            survivor = Some(mutant.desc);
        }
    }

    Ok(crate::mutation::MutationScore {
        killed,
        scored,
        survivor,
    })
}

/// Run the #14 §7 step-5 STRENGTHENING PROBE for `f`
/// (`.design/forge/strengthening-probes.md` REQ-2/REQ-3/REQ-4). Called from the
/// per-item L3 path ONLY after `f`'s REAL body proved L3 AND its mutant set met
/// the floor (the caller gates on `cert.level == L3 && reject.is_none()` + a
/// produced `MutationScore`, REQ-5). It delegates the candidate template +
/// verify/filter pipeline to `strengthen::probe`, threading TWO verify closures
/// that reuse the EXISTING verus driver:
///
/// - `verify_body` — weave the candidate `ens` into a COPY of `f` (body
///   UNCHANGED, `strengthen::candidate_fn`), build the SAME per-item sub-program
///   (`item_subprogram`), lower (`thermite_lower::lower`), content-address (the #8
///   cache), and `run_verus`. Returns `Ok(true)` iff verus PROVED the candidate
///   against the real body (the §7 "proves with no body change"); `Ok(false)` on a
///   non-`Proved` outcome OR an un-lowerable woven fn (parallel to #12's drop), and
///   `Err` on an environment failure (R-CODE-4).
/// - `verify_survivor` — verify the candidate `ens` against the SURVIVOR body (the
///   #12 mutant whose description is the recorded survivor). The survivor body
///   comes from the SAME frozen mutator (`mutation::generate`), so the kill witness
///   is the design's grounded `result == a + b` against `{ return 0; }`. Returns
///   `Ok(true)` iff verus PROVED the candidate against the survivor body (NOT
///   killed); `Ok(false)` when it did not (KILLED — the strictly-stronger witness).
///
/// Returns the ordered, deterministic list of adoptable [`strengthen::Suggestion`]s
/// (possibly empty — an honest absence, REQ-4). The probe introduces NO new prover
/// invocation path; it is a new caller of `run_verus` (REQ-2 / the doc's "the probe
/// introduces NO new prover invocation path, only a new caller of the existing one").
#[allow(
    clippy::too_many_arguments,
    reason = "the L3-path seams (spec items, seed, rlimit, verus version, cache \
    dir + enable) are the same verdict-determining inputs `mutation_score` threads; \
    they compose the per-candidate cache key, so bundling them would obscure the \
    content-addressing the probe reuses"
)]
fn strengthen_certificate(
    f: &thermite_syntax::FnItem,
    spec_items: &[Item],
    fn_deps: &[Item],
    score: &crate::mutation::MutationScore,
    seed: u64,
    rlimit: f64,
    verus_version: &str,
    cache_dir: &Path,
    use_cache: bool,
) -> Result<Vec<crate::strengthen::Suggestion>, ForgeError> {
    // The SURVIVOR body the kill witness verifies against: the #12 mutant whose
    // description matches the recorded survivor (the SAME frozen mutator). Resolved
    // once; reused for every survivor-linked candidate.
    let survivor_body: Option<thermite_syntax::FnItem> = score.survivor.as_ref().and_then(|desc| {
        crate::mutation::generate(f, seed)
            .into_iter()
            .find(|m| &m.desc == desc)
            .map(|m| m.item)
    });

    // A single content-addressed verify of a woven `fn` (the candidate `ens` over
    // a given body): lower the per-item sub-program, consult the #8 cache, else
    // `run_verus` + store. Returns whether verus PROVED it (the cert is L3 with no
    // reject). An un-lowerable woven fn is `Ok(false)` (parallel to #12's drop).
    let verify_woven = |woven: &thermite_syntax::FnItem| -> Result<bool, ForgeError> {
        let item = Item::Fn(woven.clone());
        // The candidate weaves the SAME §9 composition deps as `f` (#52) so a
        // boundary/regular callee in `f`'s body resolves in the candidate too.
        let sub = item_subprogram(&item, spec_items, fn_deps);
        let lowered = match thermite_lower::lower(&sub) {
            Ok(s) => s,
            Err(_) => return Ok(false),
        };
        let key = cache::cache_key(&lowered, seed, verus_version, THERMITE_VERSION);
        if use_cache {
            if let Some(stored) = cache::load(cache_dir, &key) {
                return Ok(mutant_cert_is_survivor(&stored));
            }
            let verus = run_verus(&sub, &lowered, seed, rlimit)?;
            let cert = assemble_certificate(&item, &verus);
            let _ = cache::store(cache_dir, &key, &cert);
            Ok(mutant_cert_is_survivor(&cert))
        } else {
            let verus = run_verus(&sub, &lowered, seed, rlimit)?;
            Ok(mutant_outcome_is_survivor(&verus.outcome))
        }
    };

    crate::strengthen::probe(
        f,
        spec_items,
        score,
        // verify_body: the candidate `ens` over the REAL body.
        |woven| verify_woven(woven),
        // verify_survivor: the candidate `ens` over the SURVIVOR body. If the
        // survivor body could not be resolved (no recorded survivor), the candidate
        // is treated as PROVING (not killed) so it is not credited a kill it cannot
        // witness — the structural-equality witness still applies.
        |candidate| match &survivor_body {
            Some(body) => {
                let woven = crate::strengthen::candidate_fn(body, candidate);
                verify_woven(&woven)
            }
            None => Ok(true),
        },
    )
}

/// `true` iff a verus outcome on a MUTANT body is a SURVIVOR (REQ-4): verus
/// PROVED the deliberately-wrong body (`VerusOutcome::Proved`). A counterexample
/// or a timeout is KILLED (OQ-4 — an un-proved mutant is not a survivor).
fn mutant_outcome_is_survivor(outcome: &VerusOutcome) -> bool {
    matches!(outcome, VerusOutcome::Proved { .. })
}

/// `true` iff a CACHED mutant cert is a SURVIVOR (REQ-4). The stored cert is a
/// full item cert: a `Level::L3` with no reject means verus PROVED the mutant (a
/// survivor); anything else (a counterexample-L0, a timeout reject) is KILLED.
fn mutant_cert_is_survivor(cert: &Certificate) -> bool {
    cert.level == Level::L3 && cert.reject.is_none()
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
        let outcome = classify_verus_outcome(stdout, "", Some(0));
        assert!(
            matches!(outcome, Ok(VerusOutcome::Proved { verified: 5 })),
            "a success summary classifies as Proved (L3): {outcome:?}"
        );
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
        // #11: a failure summary with NO profile report on stderr classifies as a
        // COUNTEREXAMPLE (the failure path), NOT a timeout (AC-3, AC-4).
        let outcome = classify_verus_outcome(stdout, stderr, Some(1));
        assert!(
            matches!(outcome, Ok(VerusOutcome::Counterexample { .. })),
            "a no-profile failure must classify as Counterexample: {outcome:?}"
        );
        let obligations = match outcome {
            Ok(VerusOutcome::Counterexample { obligations }) => obligations,
            _ => Vec::new(),
        };
        let failed: Vec<_> = obligations
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
        let r = classify_verus_outcome("not json at all", "boom", Some(101));
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
        let r = classify_verus_outcome(stdout, "internal error", Some(1));
        assert!(matches!(r, Err(ForgeError::VerusOutput { .. })));
    }

    // #11 / AC-3 / AC-4: a failure summary WHOSE STDERR carries a `--profile` Z3
    // instantiation report classifies as a TIMEOUT (not a counterexample) and the
    // parsed `SolverProfile` is attached. The profile blob is the captured real
    // verus report (R-CHAR-3 — verus's format, not forge's). The classification
    // is the deterministic crux; the profile content is oracle-excluded.
    #[test]
    fn failure_with_profile_report_classifies_as_timeout() {
        let stdout = r#"{
          "verification-results": {
            "encountered-error": true,
            "encountered-vir-error": false,
            "success": false,
            "verified": 0,
            "errors": 1
          }
        }"#;
        // The error: line is present (verus's JSON cannot tell timeout from
        // counterexample, OQ-1); the PROFILE REPORT on stderr is the discriminator.
        let stderr = "\
error: postcondition not satisfied
 --> /tmp/x_check.rs:9:9
note: Observed 14 total instantiations of user-level quantifiers
note: Cost * Instantiations: 150 (Instantiated 10 times - 71% of the total, cost 15) top 1 of 2 user-level quantifiers.

  --> /tmp/x_check.rs:13:51
   |
13 |         forall|x: int, y: int, z: int| #[trigger] e(x, y) && #[trigger] e(y, z) ==> e(x, z),
   |         ------------------------------------------^^^^^^^---------------^^^^^^^------------ Triggers selected for this quantifier
";
        let outcome = classify_verus_outcome(stdout, stderr, Some(1));
        assert!(
            matches!(outcome, Ok(VerusOutcome::Timeout { .. })),
            "a profile report present on stderr is the timeout signal: {outcome:?}"
        );
        if let Ok(VerusOutcome::Timeout { profile, .. }) = outcome {
            // Hand-derived from the blob (R-CHAR-3): 14 total, top quantifier 10.
            assert_eq!(profile.total_instantiations, 14);
            assert_eq!(profile.quantifiers[0].instantiations, 10);
        }
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
            // #52: weave the item's transitively-reachable in-file fn deps (empty
            // here — `g` references no in-file fn, so locality is preserved).
            let fn_deps = reachable_fn_deps(&parsed.program, item.name());
            let sub = item_subprogram(item, &spec_items, &fn_deps);
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
