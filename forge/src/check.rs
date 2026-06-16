//! `forge/src/check.rs` — the v0.1 `forge check` pipeline. It runs each `fn` /
//! `spec fn` item in a `.th` file end-to-end through every shipped kernel
//! component, invokes the `verus` binary on the lowered source, parses
//! verus's output into per-obligation results (with counterexamples on failure),
//! and assembles the structured certificate (`manifest.rs`). This is the first
//! live cert-oracle: `forge check conformance/sum.th`'s deterministic certificate
//! fields must match the golden `conformance/sum.cert.json`.
//!
//! Governing design: `.design/forge/check.md`. Pipeline order (the kernel's data
//! dependency):
//!
//! ```text
//! parse → validate → check_effects → lower → run verus → parse output → Certificate
//! ```
//!
//! Signature note (R-SPEC-4). The design doc
//! `.design/forge/check.md` sketches `check_file(path, seed)`; the orchestrator's
//! issue #5 manifest mandates `check_file(path) -> Result<Vec<Certificate>, _>`.
//! These are reconciled without a contract change: the pinned solver seed (§5.3)
//! is sourced from the project lockfile when present and otherwise from
//! [`DEFAULT_SOLVER_SEED`], so `check_file` keeps the issue's one-argument shape
//! while still passing a deterministic seed to verus (REQ-7). No design field is
//! redefined; the seed is input-derived, not a new parameter on the contract.
//!
//! ## REQ status
//!
//! <!-- generated:reqs view=forge-check-core-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-FORGE-CHECK-DETERMINISM | shipped | `forge/src/check.rs` | Deterministic check inputs |  |
//! | REQ-FORGE-CHECK-EXIT-STATUS | shipped | `forge/src/check.rs` | Verus exit-status discipline |  |
//! | REQ-FORGE-CHECK-LEVEL-DETERMINATION | shipped | `forge/src/check.rs` | L3 level determination |  |
//! | REQ-FORGE-CHECK-OBLIGATION-WITNESSES | shipped | `forge/src/check.rs` | Per-obligation Verus witnesses |  |
//! | REQ-FORGE-CHECK-PIPELINE | shipped | `forge/src/check.rs` | Check pipeline orchestration |  |
//! | REQ-FORGE-CHECK-VERUS-ABSENT | shipped | `forge/src/check.rs` | Verus-absent environment error |  |
//! | REQ-FORGE-CHECK-VERUS-SCRATCH | shipped | `forge/src/check.rs` | Verus invocation scratch discipline |  |
//! <!-- /generated:reqs -->
//!
//! ## #6 gate (structural vacuity triage + `#[slag]`, this iteration)
//!
//! <!-- generated:reqs view=forge-check-triage-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-FORGE-CHECK-SLAG-L1 | shipped | `forge/src/check.rs` | Slag L1 short-circuit |  |
//! | REQ-FORGE-CHECK-VACUITY-GATE | shipped | `forge/src/check.rs` | Structural vacuity gate before L3 |  |
//! <!-- /generated:reqs -->
//!
//! ## #16 gate (boundary-fn FFI L1 path, this iteration)
//!
//! <!-- generated:reqs view=forge-check-boundary-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-FORGE-CHECK-BOUNDARY-L1 | shipped | `forge/src/check.rs` | Boundary L1 short-circuit |  |
//! <!-- /generated:reqs -->
//!
//! ## #88 gate (`fx diverge` → L1 partial-correctness cap, this iteration)
//!
//! <!-- generated:reqs view=forge-check-diverge-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-FORGE-CHECK-DIVERGE-L1 | shipped | `forge/src/check.rs` | Diverge L1 partial-correctness cap |  |
//! <!-- /generated:reqs -->
//!
//! ## #8 gate (per-item content-addressed proof cache, this iteration)
//!
//! <!-- generated:reqs view=forge-check-cache-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-FORGE-CHECK-CACHE-LOOKUP-STORE | shipped | `forge/src/check.rs` | Per-item proof cache lookup and store |  |
//! | REQ-FORGE-CHECK-CACHE-VERSION-KEY | shipped | `forge/src/check.rs` | Version-keyed proof cache |  |
//! <!-- /generated:reqs -->
//!
//! ## #11 gate (solver profiles as proof-repair prompts, this iteration)
//!
//! <!-- generated:reqs view=forge-check-solver-profile-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-FORGE-CHECK-OUTCOME-CLASSIFICATION | shipped | `forge/src/check.rs` | Three-way Verus outcome classification |  |
//! | REQ-FORGE-CHECK-PROFILE-CAPTURE | shipped | `forge/src/check.rs` | Solver profile capture on rlimit hit |  |
//! | REQ-FORGE-CHECK-TIMEOUT-CERT | shipped | `forge/src/check.rs` | Distinct timeout certificate path |  |
//! <!-- /generated:reqs -->
//!
//! ## #13 gate (solver-backed tautology + vacuous-precondition checks, this iteration)
//!
//! <!-- generated:reqs view=forge-check-solver-vacuity-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-FORGE-CHECK-SOLVER-VACUITY-FAILURES | shipped | `forge/src/check.rs` | Solver-vacuity failure discipline |  |
//! | REQ-FORGE-CHECK-SOLVER-VACUITY-GATE | shipped | `forge/src/check.rs` | Solver-vacuity gate before L3 |  |
//! | REQ-FORGE-CHECK-SOLVER-VACUITY-QUALITY | shipped | `forge/src/check.rs` | Solver-confirmed contract-quality bits |  |
//! <!-- /generated:reqs -->
//!
//! ## #10 gate (the automatic L3→L2→L1 degrade ladder, this iteration)
//!
//! <!-- generated:reqs view=forge-check-degrade-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-FORGE-CHECK-DEGRADE-FAILURES | shipped | `forge/src/check.rs` | Degrade subprocess failure discipline |  |
//! | REQ-FORGE-CHECK-DEGRADE-LADDER | shipped | `forge/src/check.rs` | Default path degrade ladder |  |
//! <!-- /generated:reqs -->
//!
//! ## Cluster C10 — ergonomics ripple (`.design/basis/11-ergonomics.md`, #112)
//!
//! <!-- generated:reqs view=forge-check-ergonomics-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-FORGE-CHECK-ERGONOMICS-DEPS | shipped | `forge/src/check.rs` | Ergonomics dependency walker ripple |  |
//! <!-- /generated:reqs -->

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use thermite_syntax::{Item, Program};

use crate::cache;
use crate::cli::ForgeError;
use crate::covenant::CovenantRecord;
use crate::manifest::{effects_of, Certificate, Level, ObligationResult, RejectReason};
use crate::profile::{self, SolverProfile};

/// The `forge` toolchain version (`.design/forge/proof-cache.md` REQ-1c/REQ-5):
/// a verdict-determining cache-key input. Sourced deterministically from the
/// crate version at compile time (R-CODE-5 — no wall-clock).
const THERMITE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The pinned default solver seed (§5.3) used when no project lockfile supplies
/// one. Determinism (R-CODE-5) is a property of the input: this fixed seed plus
/// the toolchain version, never wall-clock state.
pub const DEFAULT_SOLVER_SEED: u64 = 0;

/// The pinned default verus `--rlimit` (SMT resource budget, roughly seconds) for
/// the L3 path (#11; `.design/forge/solver-profiles.md` REQ-5). Deterministic
/// (R-CODE-5 — a fixed input, not wall-clock) and set above verus's own default
/// of `10` so the conformance corpus (`sum`, `binary_search`) still proves at
/// `L3` (the cert-oracle is unperturbed). A low `--rlimit` (the
/// `forge check --rlimit 1` test lever) forces the timeout path so the three-way
/// classification is exercisable. Paired with `--profile` so an rlimit-hit
/// emits the Z3 instantiation report on stderr (the timeout discriminator).
pub const DEFAULT_RLIMIT: f64 = 30.0;

/// Run the full v0.1 `forge check` pipeline for every `fn` / `spec fn` item in
/// `path`, returning one [`Certificate`] per item in source order (REQ-1).
///
/// Stages short-circuit into the earliest failing stage's `ForgeError`. A verus
/// obligation failure is not an `Err`: it is a valid certificate describing the
/// failure (level != L3, with per-obligation witnesses). An environment /
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
#[derive(Debug, Clone)]
pub struct CheckOptions {
    /// The verus `--rlimit` SMT resource budget (#11).
    pub rlimit: f64,
    /// The mutation kill-ratio floor (#12; `.design/forge/mutation-scoring.md`
    /// REQ-5). An item that proves L3 but scores below this floor does not certify
    /// (`WeakContract` reject). Default [`mutation::MUTATION_FLOOR`] (0.60).
    pub mutation_floor: f64,
    /// The proof-backend engine selection (`.design/verified/proof-backends.md`
    /// OQ-1 / REQ-8, increment (iii), #247). [`EngineSelection::Verus`] (the default)
    /// is byte-identical to the shipped Verus path; [`EngineSelection::Lean`] /
    /// [`EngineSelection::Auto`] add the Lean engine #2 (the `--engine` surface).
    pub engine: EngineSelection,
    /// The source-file path the interactive proof artifacts (`<file>.lean-proofs/
    /// <item>.lean`) are checked in beside (REQ-7(ii)). `None` on the in-process
    /// `check_file*` entries (no `--engine lean` interactive replay); `cli::run_check`
    /// sets it to the checked file when `--engine lean`/`auto` is requested.
    pub source_file: Option<PathBuf>,
}

/// The `forge check --engine verus|lean|auto` surface (`.design/verified/
/// proof-backends.md` OQ-1 decision / REQ-8, increment (iii), #247). The decision
/// (recorded in the design's OQ-1 + the REQ-4/REQ-8 rows): `verus` is the default
/// (byte-identical to the shipped pipeline); `lean` runs the LeanEngine only
/// (exportable items discharged by Lean; a non-exportable item is reported as a
/// skip); `auto` runs Verus first and, on a Verus Unknown/timeout, tries Lean (the
/// §6 ordering). Cert attribution (REQ-4) is populated whenever a non-default engine
/// discharges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EngineSelection {
    /// `--engine verus` (default): the shipped Verus path, byte-identical.
    #[default]
    Verus,
    /// `--engine lean`: the LeanEngine only — exportable items are discharged by Lean
    /// (with the smaller trust base, attributed); a non-exportable item is reported as
    /// a skip (the Lean engine `Unknown`, not a false verdict).
    Lean,
    /// `--engine auto`: Verus first; on a Verus Unknown/timeout, try Lean (the §6
    /// ordering — Verus push-button common case, Lean as the smaller-base fallback).
    Auto,
}

impl Default for CheckOptions {
    fn default() -> Self {
        CheckOptions {
            rlimit: DEFAULT_RLIMIT,
            mutation_floor: crate::mutation::MUTATION_FLOOR,
            engine: EngineSelection::Verus,
            source_file: None,
        }
    }
}

/// `check_file` with an explicit verus `--rlimit` (#11;
/// `.design/forge/solver-profiles.md` REQ-5). [`check_file`] delegates here with
/// the pinned generous [`DEFAULT_RLIMIT`]; `cli::run_check` passes the
/// `--rlimit <FLOAT>` flag value so a low budget forces the timeout path
/// (timeout cert with a `SolverProfile`), exercising the three-way
/// classification ([`classify_verus_outcome`]). The corpus proves at the default
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

    // 4/5/6/7. Per-item certification (`thermite-design.md` §5.3 — "proof
    // results content-addressed and cached per item"; "an edit to `f` cannot
    // invalidate `g`'s certificate unless `g`'s contract references `f`'s").
    // Each `fn` is lowered and verified in isolation — a sub-program holding only
    // that `fn` plus the file's `spec fn`s (pure shared dependencies its contract
    // may reference) plus the combinator defs the lowerer emits. So verus's run
    // yields only that item's obligations and its level is L3 iff that item's
    // obligations all discharge, independent of any sibling's failure (§6 — the
    // certificate lists every function's own level; §5.1 — a counterexample
    // belongs to the item it is reported on, never a neighbor's).
    let seed = resolve_seed(path);

    // #8 proof cache (`.design/forge/proof-cache.md`): the verus version is
    // captured once per `check_file` invocation (REQ-5) so every item this run
    // keys against the same prover, and the cache directory is resolved once. A
    // missing/unreadable verus version is an environment error (REQ-5), so this
    // resolves before the per-item loop and short-circuits the whole run if the
    // prover version cannot be determined.
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

    // C11 (`.design/basis/12-mutual-recursion.md` REQ-2, crosslink #121/#113): the
    // in-file `fn`s in a mutual-recursion cycle (`a -> b -> a`, …) whose
    // termination is not supplied — a cycle containing at least one non-`fx
    // diverge` member that lacks a `dec` measure. A dec-complete cycle (every
    // member has `dec` or is `fx diverge`) is not in this set: it falls through to
    // the normal per-item lower/verus ladder, where the C9 source-order
    // single-`verus!`-block emission presents Verus a valid mutual-`decreases`
    // group → Verus proves it terminates → L3 (REQ-1/REQ-3). The validator's C9
    // REQ-2 self-call rule (`block_calls_name`) catches only a direct self-call,
    // so a missing-`dec` mutual cycle would otherwise reach Verus and be rejected
    // with `recursive function must have a decreases clause`
    // (`encountered-vir-error`), which `classify_verus_outcome` maps to a
    // `ForgeError::VerusOutput` environment abort (exit 2, empty `--json` stdout,
    // no cert) — a crash in the design's sense (AC-3). We catch the missing-`dec`
    // cycle here, before lowering / verus, and emit a `Certificate::rejected` per
    // member (`Level::L0`, the `MutualRecursionMissingDecreases` cause) — the same
    // verdict-in-cert shape as the single-fn non-decreasing L0 cert, so `forge
    // check` exits non-zero with a parseable cert array. Computed once (a pure
    // function of the program, R-CODE-5).
    let mutual_missing_dec_fns = mutual_recursion_cycle_fns(&parsed.program);

    // Stage-1 forge tier — the covenant bindings (`.design/stage1-forge-tier.md`
    // REQ-4, increment 2b). Each entry maps a `fn` to the `witness { inhabit (…);
    // falsify N; }` block that covenants it (a witness covenants the `fn` it follows in
    // source order). Computed once (a pure function of the program, R-CODE-5). A `fn`
    // ABSENT from this map is a plain v1 item (not covenant-routed) and burns
    // unchanged; a `fn` PRESENT is forge-routed and must pass its covenant BEFORE the
    // L3 burn (R-COV-1, covenant-before-burn). No v1 corpus item carries a `witness`
    // block, so the map is empty on the conformance corpus — a no-op on the v1 oracle.
    let covenant_bindings = crate::covenant_engine::witness_bindings(&parsed.program);
    // The covenant evidence for each VALIDATED covenant, attached to the item's cert
    // after the burn (Q-ORACLE: the evidence joins the forge-tier cert oracle). A
    // refuted/refused covenant carries its own evidence (or none) on its short-circuit
    // cert and is absent here.
    let mut covenant_evidence: std::collections::BTreeMap<
        String,
        crate::covenant_engine::CovenantEvidence,
    > = std::collections::BTreeMap::new();

    let mut certs = Vec::with_capacity(parsed.program.items.len());
    for item in &parsed.program.items {
        // C11 REQ-2 mutual-recursion missing-`dec` reject (no false L3, no crash):
        // a `fn` in a mutual cycle that lacks a complete `dec` group does not
        // certify — it is rejected as a clean L0 cert verdict (never lowered / sent
        // to verus), so the rejection is a parseable cert, not the raw VIR-error
        // abort. A dec-complete cycle is absent from this set and proceeds below.
        if let Item::Fn(f) = item {
            if mutual_missing_dec_fns.contains(&f.name) {
                certs.push(Certificate::rejected(
                    f.name.clone(),
                    effects_of(&f.contract.fx),
                    false,
                    RejectReason {
                        cause: "MutualRecursionMissingDecreases".to_string(),
                        detail: format!(
                            "`{}` is a member of a mutual-recursion cycle in which at least one \
                             member lacks a `dec` measure. Every member of a mutual-recursion \
                             cycle must carry a `dec` measure (Verus proves the group via a \
                             mutual-`decreases` group — `.design/basis/12-mutual-recursion.md` \
                             REQ-1/REQ-2), or declare `fx diverge` for a non-terminating loop.",
                            f.name
                        ),
                    },
                ));
                continue;
            }
        }

        // #193 open-hole short-circuit (`.design/forge/goal-repl.md` REQ-5): a fn
        // carrying any open body hole (`?N`) never certifies — it is incomplete.
        // It short-circuits here, before the #6 gate / lowering / verus (the same
        // short-circuit shape the vacuity gate uses for a rejected item), to a
        // non-certified L0 cert with an `OpenHole` cause naming the first open hole
        // (the goal-state's open goal — the §5.1 `holes: ?N : body` line). A holed
        // item never reaches verus, so it cannot certify; `forge
        // fill` must close every hole before the item proceeds to the L3 path. The
        // detail names every open hole address so `forge goal`/`forge fill` can list
        // them (R-CODE-5 — a pure function of the fn's holes).
        if let Item::Fn(f) = item {
            if let Some(detail) = crate::goal_repl::open_hole_reason(f) {
                certs.push(Certificate::rejected(
                    f.name.clone(),
                    effects_of(&f.contract.fx),
                    f.slag.is_some(),
                    RejectReason {
                        cause: "OpenHole".to_string(),
                        detail,
                    },
                ));
                continue;
            }
        }

        // Stage-1 forge-tier items (`.design/stage1-forge-tier.md` REQ-3) have no
        // v1 certification consumer yet (covenant 2b, battery 2c, proof view 2e,
        // library 3): they are SKIPPED here (no v1 cert), so a hole-free forge item
        // emits no certificate. EXCEPT (AC-7): a forge item carrying any open `?pN`
        // proof hole is incomplete and must NOT certify — it short-circuits to a
        // non-certified `OpenHole` cert through the shared `open_proof_hole_reason`
        // path (the proof-tier mirror of the `?N` body-hole short-circuit above),
        // before any lowering/verus. No corpus item is forge-tier, so this is a
        // no-op on the conformance oracle.
        if let Item::Forge(forge) = item {
            if let Some(detail) = crate::goal_repl::open_proof_hole_reason(forge) {
                certs.push(Certificate::rejected(
                    item.name().to_string(),
                    vec!["pure".to_string()],
                    false,
                    RejectReason {
                        cause: "OpenHole".to_string(),
                        detail,
                    },
                ));
                continue;
            }
            // Stage-1 forge tier — the frozen battery (`.design/stage1-forge-tier.md`
            // REQ-5 / AC-9, increment 2c), the elaboration-time gate. A `lemma`/`proof`
            // block's VERBATIM tactic content (captured by 2a) is scanned against the
            // frozen tactic allowlist + the frozen simp set; a proof citing an unlisted
            // tactic OR an unlisted simp lemma is REFUSED — named — never warned (the
            // proof-tier mirror of the `thermite_spec::validate` contract cage / the
            // covenant refusal). The refusal lands its non-certified L0 cert before any
            // discharge. A clean proof block falls through to the inert forge-item skip
            // (no v1 cert consumer yet — proof-view discharge is 2e). No conformance `.th`
            // is forge-tier, so this is a no-op on the v1 oracle.
            if let Err(violation) = crate::battery::enforce_forge_item(forge) {
                certs.push(Certificate::rejected(
                    violation.item().to_string(),
                    vec!["pure".to_string()],
                    false,
                    RejectReason {
                        cause: violation.cause().to_string(),
                        detail: violation.detail(),
                    },
                ));
            }
            continue;
        }

        // #6 gate: structural vacuity triage + `#[slag]` short-circuit run before
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
                // is unproven, so it never enters L3/L2/mutation/strengthening; the
                // L1 wrapper codegen is thermite-lower's `l1.rs` build-time job. No
                // verus run — so a boundary-only file does not require the prover.
                GateOutcome::BoundaryL1(cert) => {
                    certs.push(cert);
                    continue;
                }
                // A `fx diverge` item certifies L1 = partial correctness by the
                // structural cap (`.design/forge/check.md` REQ-8, `degrade-ladder.md`
                // REQ-9): an event loop may not terminate, so it cannot claim
                // L3-total. Like `BoundaryL1`/`SlagL1`, it never reaches the L3
                // (verus-total) / mutation / strengthen path — so the §7 mutation
                // gate that mis-rejects `run` at L0 (`WeakContract`, its loose
                // `ens result <= 256` is met by `return 0`) is skipped. The cap
                // claims less than L3, never more (R-DEFER-9), and is
                // diverge-only (a non-diverge weak contract still bites at L0).
                GateOutcome::DivergeL1(cert) => {
                    certs.push(cert);
                    continue;
                }
                // A triage / slag-validation reject: the item does not certify
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

        // Stage-1 forge tier — the covenant engine (`.design/stage1-forge-tier.md`
        // REQ-4, increment 2b), gating the L3 BURN. It runs AFTER the gate_fn
        // short-circuits (`#[slag]`/`#[boundary]`/`fx diverge` certify L1 by fiat and a
        // vacuity/weak-contract reject lands its L0 cert — none of these BURN, so the
        // covenant, which gates the burn, does not pre-empt them: a proof-exempt slag
        // item keeps `slag: true`/L1, slag.md REQ-2). A forge-routed `fn` (one carrying a
        // `witness` block) that reaches HERE is on the L3 proof-search path and must pass
        // its covenant first (R-COV-1, covenant-before-burn): author `inhabit` witnesses
        // are EXECUTED against `req` (a witness not satisfying `req` is a loud covenant
        // error, never dropped), and a `falsify` run rides the SplitMix64 generator over
        // the item's executable semantics for a `req`-satisfying input the body violates
        // `ens` on. A malformed/absent covenant is REFUSED — named — and a `falsify` hit
        // is `CovenantRefuted` (a hard fail, never degraded); BOTH short-circuit here (the
        // `continue`), so the L3 proof search below is never reached without a validated
        // covenant (the closure-instrumented `covenant_engine::covenant_gate` pins the
        // structural invariant as a unit test). A VALIDATED covenant records its evidence
        // and falls through to the burn with the record in hand. A `fn` with no `witness`
        // block is not covenant-routed and burns unchanged (no v1 corpus item carries one
        // — a no-op on the v1 oracle).
        if let Item::Fn(f) = item {
            if let Some(witness) = covenant_bindings.get(&f.name) {
                use crate::covenant_engine::{analyze_covenant, covenant_gate, CovenantGate};
                let effects = effects_of(&f.contract.fx);
                let gate = covenant_gate(analyze_covenant(f, witness), |record| {
                    debug_assert!(
                        record.declared,
                        "covenant-before-burn (R-COV-1): the burn is authorized only with \
                         a declared, validated covenant record in hand"
                    );
                });
                match gate {
                    CovenantGate::Refused { error } => {
                        certs.push(Certificate::rejected(
                            error.item().to_string(),
                            effects,
                            false,
                            RejectReason {
                                cause: error.cause().to_string(),
                                detail: error.detail(),
                            },
                        ));
                        continue;
                    }
                    CovenantGate::Refuted {
                        counterexample,
                        evidence,
                    } => {
                        certs.push(Certificate::covenant_refuted(
                            f.name.clone(),
                            effects,
                            &counterexample,
                            evidence,
                        ));
                        continue;
                    }
                    CovenantGate::Burned {
                        result: (),
                        evidence,
                    } => {
                        // The covenant validated and authorized the burn. Record its
                        // evidence (attached to the post-burn cert below) and fall through
                        // to the normal L3 proof search with the covenant in hand.
                        covenant_evidence.insert(f.name.clone(), evidence);
                    }
                }
            }
        }

        // #52 §9 composition weaving (`.design/lower/boundary-composition.md`
        // REQ-2): weave the in-file `fn`s this item transitively references into
        // its §5.3 sub-program — regular fns with their real body, boundary/slag
        // fns as `#[verifier::external_body]` signatures — so `verus` resolves the
        // callee and the caller proves through its contract (was an undefined-callee
        // L0). Empty for a fn referencing only spec fns / combinators (the pure
        // corpus), so the corpus cert + lowering are byte-stable (AC-4).
        let fn_deps = reachable_fn_deps(&parsed.program, item.name());
        // #68: also weave the `struct`/`enum` declarations the checked item and
        // its woven fn-deps reference (transitively closed over the ADT type
        // graph), so the per-item Verus emission has the type decls + their
        // `well_formed` invariants in scope (was an undefined-type L0).
        let mut referrers: Vec<&Item> = vec![item];
        referrers.extend(fn_deps.iter());
        // #71: the spec-fn set woven into this item's sub-program. An `Item::Fn`
        // keeps the whole file's `spec_items` (an exec fn's contract
        // may reference any spec fn — e.g. `sum`'s `ens result == spec_sum(xs)` —
        // so verus must resolve every one; this dep-weaving is unchanged). An
        // `Item::SpecFn` instead weaves only itself + the spec fns it transitively
        // references (`reachable_spec_fn_deps`, which includes the start), so a
        // multi-spec-fn file no longer lowers every spec fn to byte-identical Verus
        // (the cache-collision cause — the sub-program is now distinct per spec fn).
        let item_spec_items: Vec<Item> = match item {
            Item::SpecFn(_) => reachable_spec_fn_deps(&parsed.program, item.name()),
            _ => spec_items.clone(),
        };
        // For a checked `Item::SpecFn`, the ADT referrers are its own reachable
        // spec-fn set (which includes the spec fn itself), so its `enum`/`struct`
        // decls are present even though the file's full `spec_items` is no longer
        // woven (#71). The `Item::Fn` path is unchanged — its ADT referrers stay
        // `[item] + fn_deps` exactly as before (the exec sub-program is byte-stable;
        // the corpus cert oracle is unperturbed). The Fn arm of `item_subprogram`
        // still weaves the full `spec_items`.
        if matches!(item, Item::SpecFn(_)) {
            referrers.clear();
            referrers.extend(item_spec_items.iter());
        }
        let adt_deps = reachable_adt_deps(&parsed.program, &referrers);
        let sub = item_subprogram(item, &item_spec_items, &fn_deps, &adt_deps);
        let lowered = thermite_lower::lower(&sub).map_err(ForgeError::Lower)?;

        // #8 proof cache (`.design/forge/proof-cache.md` REQ-1/REQ-3): the lowered
        // source is the item's content-address — the bytes verus checks
        // (§5.3 isolated sub-program). The key composes it with the four
        // verdict-determining inputs. Consult the cache before spawning verus.
        //
        // #11: the cache is keyed at the canonical [`DEFAULT_RLIMIT`] budget only.
        // A non-default `--rlimit` (the timeout-forcing / exploratory lever) is a
        // budget-dependent verdict (a timeout at `--rlimit 1` is not the cached
        // `L3` proved at the generous default), so it bypasses the cache entirely
        // — neither served from nor written to it. This keeps the cache key
        // (`cache::cache_key`, four inputs) unchanged while staying sound (a
        // timeout verdict is never cached as if proved).
        // #12: a non-default `--mutation-floor` (the AC-3 floor-flip lever) is also a
        // verdict-changing knob not in the cache key (the same lowered source can
        // certify under a low floor and reject `WeakContract` under the default), so
        // a non-default floor likewise bypasses the cache — neither served nor
        // written. The canonical-config run (default rlimit + default floor) is the
        // only one that populates / serves the shared `target/` cache, keeping the
        // four-input `cache::cache_key` unchanged while staying sound.
        let use_cache =
            rlimit == DEFAULT_RLIMIT && options.mutation_floor == crate::mutation::MUTATION_FLOOR;
        let key = cache::cache_key(&lowered, seed, &verus_version, THERMITE_VERSION);
        if use_cache {
            if let Some(stored) = cache::load(&cache_dir, &key) {
                // Hit: skip verus entirely (REQ-3, AC-1 — the solver-skip). The
                // stored cert is the canonical fresh verify; mark it served from
                // cache (`cached: true`) — provenance only, oracle fields unchanged
                // (REQ-2: a hit is oracle-equal to a fresh verify). A #13
                // solver-vacuity reject was cached like a proof verdict, so a hit
                // serves it without re-running the two harness queries (the cache
                // hit is a verus-free path end-to-end).
                certs.push(stored.with_cached(true));
                continue;
            }
        }

        // #13 solver-vacuity gate (`.design/forge/solver-vacuity.md` REQ-5): on a
        // cache miss, after #6's free structural triage passed (`ProceedToL3`,
        // above) and before the item's own L3 proof (a contract that survives the
        // syntactic checks may still be semantically degenerate — the §7
        // cheapest-first ordering). The two checks reuse the existing contract
        // lowering + verus driver to detect a semantic tautology (`ens` holds for
        // an arbitrary result) or an unsatisfiable precondition. A `Detected`
        // short-circuits to a non-certified `Certificate::rejected_vacuity`
        // (verdict-in-cert, the matching `contract_quality` bool solver-confirmed
        // `true`) without running the L3 proof on a known-degenerate contract; a
        // `Clean` falls through to the existing L3 path where `graduate_triage_clean`
        // keeps both bools live-`false`, now solver-confirmed (REQ-6). An
        // environment / internal verus failure on a harness query surfaces a
        // `ForgeError` (R-CODE-4), never a silent clean. The gate runs inside the
        // cache-miss branch so the deterministic #13 verdict is cached with the item
        // (OQ-2): a later hit serves the cached reject / clean cert without a verus
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
                // A #13 reject is a settled, deterministic verdict (a function of the
                // lowered contract + seed + versions), so it is cached like a
                // counterexample cert: a re-check serves the hit without re-running
                // the harness queries. Best-effort store (a write failure never fails
                // the verdict — R-CODE-2), at the canonical budget only.
                if use_cache {
                    let _ = cache::store(&cache_dir, &key, &cert);
                }
                certs.push(cert.with_cached(false));
                continue;
            }
        }

        // Clean (or a `spec fn`, which carries no contract to check): the solver
        // runs the real L3 proof (REQ-3). Assemble the cert exactly as the
        // non-cached path always has.
        let verus = run_verus(&sub, &lowered, seed, rlimit)?;
        let cert = assemble_certificate(item, &verus);

        // #10 automatic degrade ladder (`.design/forge/degrade-ladder.md`, the
        // default `forge check` path). On a `VerusOutcome::Timeout` (verus could
        // not prove within budget — inconclusive) the v0.1 behavior was to emit the
        // `VerusTimeout` L0 cert and stop; #10 replaces that stop with the ladder:
        // attempt L2 (kani) and, on an under-bound, drop to L1. A
        // `VerusOutcome::Counterexample` (verus disproved the contract — a real
        // bug) is a hard fail and never degrades (REQ-2 anti-cheat); the ladder
        // short-circuits it. The ladder runs only for an `Item::Fn` (a `spec fn`
        // carries no `req`/`ens` to bound-check at L2). An environment failure on a
        // lower rung (kani absent / unparseable) propagates as a `ForgeError`
        // (REQ-8), never a silent degrade.
        // proof-backends #204: mint the per-item backend-neutral obligation set
        // (`.design/verified/proof-backends.md` REQ-1/REQ-1.2) using the corrected
        // full-expression-position called-spec-fn closure (REQ-1.2 / #226: seed
        // `req ∪ ens ∪ body ∪ dec(item)`, closure-step over each reached spec-fn's
        // `body ∪ dec`). The set is the contract obligation plus — when the closure
        // is non-empty — the registry-termination obligation (conjoined item-wide,
        // REQ-1.2). The Verus engine admits every class (incl. RegistryTermination,
        // REQ-1.2(a) — its dec-check is the common discharge path), confirmed via
        // the fragment gate; an unadmitted class would block certification per the
        // conjunction rule (the seam a narrower future engine keys on).
        let item_obligations = mint_item_obligations(&parsed.program, item);
        let evidence_key =
            crate::engine::engine_cache_key(crate::engine::EngineName::Verus, key.clone());
        // REQ-1.2(a) conjunction discharge: when the item carries a
        // registry-termination obligation (a non-empty called-spec-fn closure), it
        // is discharged alongside the contract obligation by the same Verus run —
        // the woven spec-fns in the per-item sub-program have already passed
        // Verus's recursion/decreases check, so a `Proved` outcome certifies both
        // classes (REQ-1.2(a), the common path). Its per-obligation evidence key
        // (the engine-discriminated address, §2(d)) participates in the item's
        // content address so a change to a reached spec-fn's measure invalidates
        // the cached cert. The Lean-path well-foundedness discharge is increment
        // (ii) (NOT-STARTED); on the Verus path the conjunction is automatic.
        if let Some(rt) = &item_obligations.registry_termination {
            use crate::engine::Engine as _;
            let _rt_key = crate::engine::VerusEngine.evidence_key(rt);
        }
        let cert = if let Item::Fn(f) = item {
            // Route the L3 contract discharge through the Verus engine
            // (REQ-2/REQ-3/REQ-3.1). The contract obligation is the head of the set.
            ladder_for_timeout(
                f,
                &sub,
                &verus.outcome,
                cert,
                &item_obligations.contract,
                evidence_key,
            )?
        } else {
            cert
        };

        // A non-slag `fn` that reached the L3 path passed triage — graduate the
        // §7.1 `contract_quality` bools to asserted live-`false` (REQ-6 / AC-7). A
        // `spec fn` carries no contract, so triage does not apply and the bools
        // stay forward-declared. A timeout cert (`VerusTimeout`) is not graduated:
        // its triage bools stay forward-declared (nothing about the contract's
        // syntactic quality was newly confirmed by a budget exhaustion).
        let cert = if matches!(item, Item::Fn(_)) && cert.reject.is_none() {
            cert.graduate_triage_clean()
        } else {
            cert
        };

        // #12 §7 step 4 — mutation scoring, after a successful L3 proof of the real
        // body (`.design/forge/mutation-scoring.md` REQ-7). Reached only on a
        // `VerusOutcome::Proved` real body: the cert is `Level::L3` with no reject.
        // A non-proving item (counterexample / timeout / a `spec fn`) is never
        // scored — §7's premise is "mutate a known-good body". Each mutant's
        // re-verify is content-addressed through the same proof cache (#8), so a
        // re-`forge check` re-scores from the cache cheaply. A sub-floor kill ratio
        // turns the cert into a `WeakContract` reject (verdict-in-cert); a met floor
        // graduates `mutants_killed`/`survivor` on the certified cert.
        let cert = if let Item::Fn(f) = item {
            if cert.level == Level::L3 && cert.reject.is_none() {
                let score = mutation_score(
                    f,
                    &spec_items,
                    &fn_deps,
                    &adt_deps,
                    seed,
                    rlimit,
                    &verus_version,
                    &cache_dir,
                    use_cache,
                )?;
                let effects = effects_of(&f.contract.fx);
                if score.meets_floor(options.mutation_floor) {
                    // #14 §7 step 5 — strengthening probe
                    // (`.design/forge/strengthening-probes.md` REQ-5). The item is a
                    // settled L3-certified + scored item (level L3, no reject, a
                    // `MutationScore` produced), so the probe runs: it generates the
                    // frozen candidate stronger-`ens` set, verifies each against the
                    // real body via the same `run_verus` + #8 cache, keeps the
                    // verifying + strictly-stronger ones, and attaches them as
                    // advisory suggestions. The probe never changes the verdict
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
                        &adt_deps,
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
                    // no mutant could be scored (un-synthesizable return type) — a
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

        // #11/#10: a timeout cert (budget-dependent, `VerusTimeout`) and any cert
        // the #10 ladder produced by degrading a timeout (`lowered_assurance`) are
        // not cached — they are not settled verdicts (a larger budget might prove
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
        // The cache is best-effort: a write failure must not fail the verdict
        // (which already stands) — degrade to "uncached," never to an error
        // (REQ-6, R-CODE-2). `store` persists the canonical `cached: false`. Only
        // at the canonical [`DEFAULT_RLIMIT`] (#11): a non-default budget's verdict
        // is not cached (it bypassed the lookup too).
        if use_cache {
            let _ = cache::store(&cache_dir, &key, &cert);
        }
        certs.push(cert.with_cached(false));
    }

    // #17 §9 end-to-end vs to-the-boundary classification
    // (`.design/forge/e2e-vs-boundary.md` REQ-1/REQ-2/REQ-3). Run the structural
    // transitive-call-closure analysis once over the whole file's program (it is a
    // pure function of the parsed `Program`, R-CODE-5), then attach each fn's
    // assurance scope to its certificate. Orthogonal to the verdict (REQ-5): the
    // scope is recorded alongside the already-achieved level — a fn whose body
    // SMT-proved at L3 but whose closure crosses a `#[boundary]`/`#[slag]` fn keeps
    // `Level::L3` and records `ToBoundary { via }`. The classification keys on the
    // in-file `#[boundary]`/`#[slag]` node (the §9 composition rule), never a
    // sibling's verdict, so it does not perturb any oracle-stable level.
    let scopes = crate::closure::classify(&parsed.program);
    let certs = certs
        .into_iter()
        .map(|cert| {
            let cert = match scopes.get(&cert.item) {
                Some(scope) => cert.with_assurance_scope(scope.clone()),
                // A cert whose item has no node keeps its `None` scope, which
                // `oracle_subset` reads as end-to-end (the golden-stable default).
                None => cert,
            };
            // Attach the covenant evidence to a VALIDATED forge-routed item's cert
            // (REQ-4 / Q-ORACLE). A refuted/refused covenant short-circuited above and
            // carries its own evidence (or none), so it is absent from this map; a v1
            // item is absent too (no covenant), keeping its cert byte-identical.
            match covenant_evidence.get(&cert.item) {
                Some(evidence) => cert.with_covenant_evidence(*evidence),
                None => cert,
            }
        })
        .collect();
    Ok(certs)
}

/// Run `forge check --engine lean|auto` (`.design/verified/proof-backends.md` OQ-1 /
/// REQ-4/REQ-5/REQ-8, increment (iii), #247). The Verus default path is
/// [`check_file_with_options`]; this adds the Lean engine #2 surface:
///
/// - `auto` (`.design/verified/proof-backends.md` §6 ordering): run Verus first
///   (the byte-identical base certs). For each item where Verus is inconclusive (a
///   degrade / timeout `lowered_assurance` / a non-L3 non-counterexample), try Lean:
///   on a Lean `Proven` the cert is upgraded to L3 with the Lean attribution (the
///   smaller base, REQ-4). A Verus `Proven` (L3, no reject) is kept as-is (Lean is not
///   run — no Lean cost on the common case, no false disagreement). The disagreement
///   halt (REQ-5) fires when both engines produced a verdict on the same obligation
///   and they contradict (Proven ⊕ Refuted): a `ForgeError::SoundnessAlarm`.
/// - `lean` (LeanEngine only): each item is discharged by Lean. An exportable,
///   auto-tier item that Lean proves → an L3 cert with the Lean attribution; a
///   tier-(c) item → the interactive replay (REQ-7); a non-exportable item → an
///   `Unverifiable` skip (Level::L0, no false verdict — the LeanEngine `Unknown`).
///
/// Verus stays the sole engine for the §0.1 meta/battery queries (vacuity / mutation /
/// strengthen) in v1 (OQ-5); the Lean mutation battery (REQ-9) runs only on the items
/// the Lean engine discharges.
pub fn check_file_with_engine(
    path: impl AsRef<Path>,
    options: CheckOptions,
) -> Result<Vec<Certificate>, ForgeError> {
    let path = path.as_ref();
    let selection = options.engine;
    let source_file = options
        .source_file
        .clone()
        .unwrap_or_else(|| path.to_path_buf());

    // The Verus base certs (byte-identical to the default path). `auto` keeps a Verus
    // `Proven`; `lean` ignores the Verus verdict (LeanEngine only) but reuses the same
    // parse/validate/effect-check gate via this call (a parse/validate failure is the
    // same `ForgeError` either way).
    let base = check_file_with_options(
        path,
        CheckOptions {
            engine: EngineSelection::Verus,
            ..options
        },
    )?;

    // Re-parse for the Lean engine (the exporter needs the spec-fn defs + the item).
    let src = std::fs::read_to_string(path).map_err(|e| ForgeError::Io {
        path: path.display().to_string(),
        source: e,
    })?;
    let parsed = thermite_syntax::parse(&src);
    if !parsed.is_clean() {
        return Err(ForgeError::Parse(parsed.errors));
    }
    let lean = crate::engine::LeanEngine::new(parsed.program.clone(), lean_package_root());

    let mut out = Vec::with_capacity(base.len());
    for cert in base {
        let item = match crate::lean_export::find_item(&parsed.program, &cert.item) {
            Some(i) => i,
            // No matching node — keep the base cert untouched.
            None => {
                out.push(cert);
                continue;
            }
        };
        let obligations = mint_item_obligations(&parsed.program, item);
        let new_cert = lean_engine_cert(
            &lean,
            &source_file,
            cert,
            &obligations.contract,
            selection,
            options.mutation_floor,
        )?;
        // REQ-6c anti-Goodhart (increment 2d): the certify-time definition-tower
        // budget gate on the forge/Lean discharge path. A forge-tier cert (a
        // non-default engine discharged it) whose contract unfolds a tower deeper /
        // wider than the Q2 budget is refused here, at certify time — never in
        // `forge audit` (its "gates nothing" invariant is shipped, #274). A
        // within-budget forge-tier cert pins the unfolded-tower hash. The Verus
        // default path (no engine attribution) is untouched, so the v1 goldens stay
        // byte-identical.
        let new_cert = gate_definition_tower(new_cert, &parsed.program, &src, item);
        out.push(new_cert);
    }
    Ok(out)
}

/// The REQ-6c certify-time definition-tower budget gate (increment 2d;
/// `.design/stage1-forge-tier.md` REQ-6 / AC-10). Applied to a freshly-produced cert
/// on the forge/Lean discharge path:
///
/// - a v1 / Verus-path cert (no `engine_attribution`) is returned UNCHANGED — the
///   gate is a forge-tier gate, so the v1 goldens stay byte-identical;
/// - a non-`fn` item (a `spec fn` has no contract to root a tower) is returned
///   unchanged;
/// - a forge-tier cert whose contract tower exceeds the Q2 budget (depth 4 / 40
///   definitions) is REFUSED — replaced with a `DefinitionTowerBudget` reject cert
///   that still pins the unfolded-tower hash (AC-10);
/// - a within-budget forge-tier cert keeps its verdict and gains the pinned
///   `meaning_audit` (the unfolded-tower hash + depth + count).
///
/// Read-only / pure (no prover): the tower is a projection of the AST + source
/// (`meaning::build_tower`), exactly the same artifact `forge audit --meaning` prints.
fn gate_definition_tower(
    cert: Certificate,
    program: &Program,
    src: &str,
    item: &Item,
) -> Certificate {
    // The gate is forge-tier-only: a cert with no engine attribution is the v1 Verus
    // path (or an honest skip), left byte-identical.
    if cert.engine_attribution.is_none() {
        return cert;
    }
    // Only a `fn` carries a `req`/`ens` contract that roots a meaning tower.
    let Item::Fn(f) = item else {
        return cert;
    };
    let tower = crate::meaning::build_tower(program, src, f);
    match tower.over_budget_detail() {
        Some(detail) => Certificate::rejected_over_budget_tower(
            &cert.item,
            cert.effects.clone(),
            detail,
            tower.meaning_audit(),
        ),
        None => cert.with_meaning_audit(tower.meaning_audit()),
    }
}

/// The `lean/` package root for the Lean engine (`.design/verified/proof-backends.md`
/// REQ-6 — the cwd `lake env lean` runs in). Resolved relative to the `forge` crate
/// dir (the workspace's `lean/` sibling). Deterministic (R-CODE-5).
fn lean_package_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("lean")
}

/// Apply the Lean engine to one item's base Verus cert (`.design/verified/
/// proof-backends.md` OQ-1 / REQ-4/REQ-5/REQ-7). Returns the cert the `--engine`
/// surface emits for the item, or a `ForgeError::SoundnessAlarm` on a Proven ⊕ Refuted
/// disagreement (REQ-5). The Verus base cert's verdict (Proven / Refuted / Unknown) is
/// reconstructed from its `level`/`reject` for the disagreement check + the `auto`
/// inconclusive test.
fn lean_engine_cert(
    lean: &crate::engine::LeanEngine,
    source_file: &Path,
    verus_cert: Certificate,
    obligation: &crate::obligation::Obligation,
    selection: EngineSelection,
    mutation_floor: f64,
) -> Result<Certificate, ForgeError> {
    use crate::engine::{Engine as _, EngineName, Verdict};

    match selection {
        EngineSelection::Verus => Ok(verus_cert),
        EngineSelection::Auto => {
            // Verus first: a Verus `Proven` (L3, no reject) is kept — Lean is not run
            // (no cost on the common case, no spurious disagreement). Only an
            // inconclusive Verus result (degrade / timeout / non-L3) tries Lean.
            let verus_verdict = verus_verdict_of(&verus_cert);
            if matches!(verus_verdict, Verdict::Proven(_)) && verus_cert.reject.is_none() {
                return Ok(verus_cert);
            }
            // Verus inconclusive → try Lean (the §6 ordering). The disagreement guard
            // (REQ-5) fires only if Verus witnessed a refutation (Refuted) and Lean
            // proves — the real-unsoundness case. A Verus Unknown/timeout + a Lean
            // Proven is benign (Verus could not decide).
            let lean_verdict = lean.discharge(obligation, &CovenantRecord::none());
            if let Err(disagreement) = crate::engine::check_disagreement(
                &obligation.item,
                EngineName::Verus,
                &verus_verdict,
                lean.name(),
                &lean_verdict,
            ) {
                return Err(ForgeError::SoundnessAlarm(disagreement));
            }
            match lean_verdict {
                Verdict::Proven(_) => Ok(lean_proven_cert(lean, &verus_cert, mutation_floor)),
                // Lean could not discharge either — keep the Verus base cert (the
                // degrade/timeout verdict stands).
                _ => Ok(verus_cert),
            }
        }
        EngineSelection::Lean => {
            // LeanEngine only: discharge the item by Lean. Tier-(c) → interactive
            // replay; auto tiers → live lake; non-exportable → a skip.
            let (verdict, interactive) = if lean.admits_auto(obligation) {
                (lean.discharge(obligation, &CovenantRecord::none()), false)
            } else {
                // Not auto-exportable: try the interactive (tier-c) replay path; a
                // non-tier-c non-exportable item is an Unverifiable skip. An
                // interactive (replayed) proof carries the interactive trust profile
                // (the author is a reviewed step, REQ-7(ii)/OQ-4).
                (lean.replay_interactive(source_file, obligation), true)
            };
            match verdict {
                Verdict::Proven(_) if interactive => {
                    Ok(lean_interactive_proven_cert(lean, &verus_cert))
                }
                Verdict::Proven(_) => Ok(lean_proven_cert(lean, &verus_cert, mutation_floor)),
                Verdict::Refuted(_) => {
                    // A Lean witnessed refutation (not produced by the current export
                    // path, but total): an L0 reject, never a silent pass.
                    Ok(Certificate::rejected(
                        &verus_cert.item,
                        verus_cert.effects.clone(),
                        false,
                        crate::manifest::RejectReason {
                            cause: "LeanRefuted".to_string(),
                            detail: "the Lean engine refuted the obligation".to_string(),
                        },
                    ))
                }
                Verdict::Unknown(reason) => Ok(lean_unverifiable_cert(&verus_cert, &reason)),
            }
        }
    }
}

/// Reconstruct an `engine::Verdict` from a Verus base cert (`.design/verified/
/// proof-backends.md` REQ-5 — for the disagreement check + the `auto` inconclusive
/// test). L3 + no reject = `Proven`; a counterexample reject (a witnessed
/// `postcondition not satisfied`) = `Refuted`; everything else (timeout / degrade /
/// L0-no-witness) = `Unknown`. The `Refuted` reconstruction keys on a witnessing
/// obligation location (REQ-3.1: a witness-less failure is `Unknown`, never `Refuted`).
fn verus_verdict_of(cert: &Certificate) -> crate::engine::Verdict {
    use crate::engine::{CacheKey, Counterexample, Evidence, Reason, Verdict};
    let key = CacheKey {
        engine: crate::engine::EngineName::Verus,
        content_address: format!("verus::{}", cert.item),
    };
    if cert.level == Level::L3 && cert.reject.is_none() {
        return Verdict::Proven(Evidence { verified: 1, key });
    }
    // A witnessed counterexample (a failing obligation carrying a `--> span`) is a
    // genuine refutation; a witness-less failure (timeout / fast-unknown) is Unknown
    // (REQ-3.1 — refutation requires a witnessing input).
    let witnessed = cert.obligations.iter().any(|o| o.location.is_some());
    if witnessed {
        Verdict::Refuted(Counterexample {
            obligations: cert.obligations.clone(),
        })
    } else {
        Verdict::Unknown(Reason::IncompleteUnknown(format!(
            "verus did not prove `{}` (no witnessing counterexample)",
            cert.item
        )))
    }
}

/// Build the L3 cert a Lean `Proven` produces (`.design/verified/proof-backends.md`
/// REQ-4/REQ-9): the item certifies at L3 (Lean kernel-checked) with the Lean engine
/// attribution (the smaller trusted base — the auditor-visible refinement) and the
/// engine-generic mutation tally (REQ-9 — mutants re-discharged via the same Lean
/// path, the kill ratio + the "untested against lean" count). The `Level` is
/// unchanged-meaning L3 ("proven for all inputs"); the attribution records which
/// engine + its base; the mutation qualifier is attached additively.
fn lean_proven_cert(
    lean: &crate::engine::LeanEngine,
    base: &Certificate,
    mutation_floor: f64,
) -> Certificate {
    let attribution = crate::engine::attribution_for(lean);
    // Schema-v2 per-clause block (REQ-1/AC-4): a Lean-discharged clause records its
    // engine, named trust base, and the cert-level verdict (`Proved` — this function is
    // reached ONLY on a Lean `Verdict::Proven`). This is a forge-tier path, never the v1
    // Verus corpus, so the v1 golden certs stay byte-identical (their clauses carry no
    // per-clause block).
    let cert = Certificate::new(
        &base.item,
        Level::L3,
        base.effects.clone(),
        0,
        vec![crate::manifest::ObligationResult::discharged(
            "discharged by the Lean engine (kernel-accepted; the smaller trusted base \
             {Lean kernel + 3 axioms, EXP} — proof-backends REQ-4)",
        )
        .with_clause_attribution(
            attribution.engine.clone(),
            attribution.trust_profile.clone(),
            crate::verdict::CertVerdict::Proved,
        )],
    )
    .graduate_triage_clean()
    .with_engine_attribution(attribution);
    // REQ-9 engine-generic battery (the Lean path): re-discharge the frozen mutant set
    // via the same Lean engine. The Verus-path battery (`mutation_score`) is untouched.
    // Only an `Item::Fn` (the cert's item) is mutation-scored — a `spec fn` carries no
    // `ens`. A `spec fn` (no `ens`) has no mutation obligation, so it certifies on the
    // Lean kernel proof alone (no floor gate — there is nothing to mutate).
    let Some(Item::Fn(f)) = crate::lean_export::find_item(lean_program(lean), &base.item) else {
        return cert;
    };
    let tally = lean_mutation_score(lean, f);
    // REQ-9/AC-7 (the floor gates the Lean path — proof-backends.md §7, the #248 fix):
    // the kill-ratio over the `attempted` denominator must meet the mutation floor for
    // the item to certify L3-via-Lean, mirroring the Verus path's `meets_floor` gate
    // (`mutation_score` → `WeakContract`). On the Lean-only path the #101 equivalence
    // probe is outside the Engine interface (a §0.1 verus meta-query, F3/OQ-5) and is
    // not threaded, so the denominator = `attempted` with no equivalence exclusion,
    // which the qualifier records. The shipped 0/0 backstop (`kill_ratio() == 0.0` on
    // an empty denominator) means an item that generated mutants but attempted none
    // against Lean (all untested) is below any positive floor → does not certify (a
    // `WeakContract` reject, never a silent L3). A below-floor item (survivors the
    // contract does not catch) likewise rejects.
    if tally.meets_floor(mutation_floor) {
        cert.with_mutation_score(tally.qualifier(), None)
    } else {
        // Below the floor (or 0/0 with mutants generated): a `WeakContract`-style reject
        // — the contract under-constrains the body, so the Lean kernel proof does not
        // license an L3 cert (proof-backends REQ-9/AC-7; the WeakContract mirror).
        Certificate::rejected_weak_contract(
            &base.item,
            base.effects.clone(),
            tally.mutants_killed_string(),
            tally.survivor_detail(),
        )
    }
}

/// Build the L3 cert a tier-(c) interactive Lean proof produces (`.design/verified/
/// proof-backends.md` REQ-7(ii) / OQ-4): like [`lean_proven_cert`] but the attribution
/// carries the interactive trust profile (the human/agent proof author is a reviewed,
/// not mechanized, step, added to {Lean kernel + 3 axioms, EXP}). The auditor sees
/// the extra reviewed-author item. No mutation tally on the interactive path (a
/// recursive-registry obligation's mutants are tier-(c) too — "untested against
/// lean-auto"; the §6 tier-(c) interactive battery is out of v1's auto scope).
fn lean_interactive_proven_cert(
    lean: &crate::engine::LeanEngine,
    base: &Certificate,
) -> Certificate {
    use crate::engine::Engine as _;
    let attribution = crate::engine::EngineAttribution {
        engine: lean.name().tag().to_string(),
        trust_profile: crate::engine::trust_profile_interactive().items,
    };
    Certificate::new(
        &base.item,
        Level::L3,
        base.effects.clone(),
        0,
        vec![crate::manifest::ObligationResult::discharged(
            "discharged by an INTERACTIVE Lean proof (kernel-accepted, sorry-free replay; \
             the trusted base adds the reviewed proof author — proof-backends REQ-7(ii))",
        )
        .with_clause_attribution(
            attribution.engine.clone(),
            attribution.trust_profile.clone(),
            crate::verdict::CertVerdict::Proved,
        )],
    )
    .graduate_triage_clean()
    .with_engine_attribution(attribution)
}

/// Borrow the Lean engine's parsed program (for the per-item + per-mutant obligation
/// minting on the REQ-9 Lean mutation path). The engine carries the program; this is
/// the read accessor the mutation battery needs.
fn lean_program(lean: &crate::engine::LeanEngine) -> &Program {
    lean.program()
}

/// The SHARED mutant catalogue the L3 re-elaboration mutation battery scores
/// (`.design/stage1-forge-tier.md` REQ-6 / AC-10, increment 2d — anti-Goodhart defense
/// (b)). The L3 counterpart of the shipped Verus mutation gate (`mutation_score`, #12):
/// it reuses the FROZEN mutation operator catalogue [`crate::mutation::generate`]
/// UNCHANGED — the same operator families and the same `MUTANT_CAP` = 64 deterministic
/// order-prefix `generate` applies internally. The catalogue is SHARED, never forked
/// (AC-10 pins this with a test: the re-elaboration battery's mutant set IS
/// `mutation::generate`'s, so a future fork breaks the test).
///
/// Only the KILL CHECK differs from the Verus gate, exactly as REQ-6b specifies: the
/// Verus gate runs a per-mutant Verus SOLVER search; the L3 path RE-ELABORATES the
/// mutant's obligation through the existing Lean discharge path
/// ([`lean_mutation_score`] → [`crate::engine::LeanEngine::discharge`], which exports
/// the obligation and runs lake) — a decidable per-mutant type-check, not a search
/// (the substrate note: drive the existing elaborator, do not build a new one). A
/// mutant the proof still elaborates against survived (the contract under-constrains
/// the body); one it fails is killed. Survivors keep counting against the floor (the
/// Budd–Angluin floor gate, [`crate::engine::LeanMutationTally::meets_floor`]).
///
/// Performance (the flagged REQ-6a/b risk): up to `MUTANT_CAP` = 64 re-elaborations
/// per item. Each is ONE lake elaboration (no proof search), and the battery is a
/// POST-proof QUALITY gate — exactly parallel to the shipped Verus `mutation_score`,
/// which already runs up to 64 verus runs per item AFTER the L3 proof. It is NOT inside
/// the per-clause [`crate::engine`] `KernelBudget` (Q4 30s/clause), which bounds the
/// discharge of ONE clause's proof, not the post-proof mutation battery. So the 64
/// re-typechecks do not exceed the per-clause budget (they are not within it); the
/// `MUTANT_CAP` budget is the same bound the Verus gate already lives under.
pub(crate) fn reelaboration_mutants(
    f: &thermite_syntax::FnItem,
    adt_deps: &[Item],
) -> Vec<crate::mutation::Mutant> {
    // The SHARED frozen catalogue (REQ-6b / AC-10 — not a fork). `generate` applies the
    // `MUTANT_CAP` 64 order-prefix internally, so the returned set is already bounded.
    crate::mutation::generate(f, 0, adt_deps)
}

/// Score the frozen mutant set of `f` against its own contract via the Lean engine
/// (`.design/verified/proof-backends.md` REQ-9, increment (iii), #247). The
/// engine-generic battery: each mutant is attempted via the same Lean engine path; a
/// mutant the Lean fragment admits and that Lean does not prove (Refuted ∪
/// Unknown-after-attempt) is killed; a mutant the fragment does not admit is "untested
/// against lean" (reported, never counted killed); a Lean-Proven mutant survived.
///
/// The #101 proven-equivalent exclusion is a §0.1 meta-query outside the Engine
/// interface in v1 (it stays a direct verus query, F3/OQ-5); on the Lean-only path no
/// verus run is threaded, so the tally reports the raw survivor set (a
/// survivor is reported, never silently excluded as equivalent without the verus
/// probe). The Verus-path battery (`mutation_score`) keeps the shipped #101 exclusion
/// unchanged. Deterministic (R-CODE-5): the mutant set is a pure function of the AST.
fn lean_mutation_score(
    lean: &crate::engine::LeanEngine,
    f: &thermite_syntax::FnItem,
) -> crate::engine::LeanMutationTally {
    use crate::engine::Engine as _;
    let mut tally = crate::engine::LeanMutationTally::default();
    let base_program = lean_program(lean);
    // The Lean-path caller threads the whole program's items as `adt_deps`
    // (REQ-11) so the F-STRUCT-ZERO family resolves any struct return — the same
    // items the per-mutant Lean engine exports from (`program_with_mutant`). The mutant
    // set is the SHARED frozen catalogue via `reelaboration_mutants` (REQ-6b / AC-10:
    // the L3 re-elaboration battery reuses `mutation::generate`, never a fork) — the
    // per-mutant kill check below is the re-elaboration (export → lake type-check), not
    // a Verus solver run.
    for mutant in reelaboration_mutants(f, &base_program.items) {
        // The LeanEngine exports the item by name from its stored program (the
        // exporter re-fetches `o.item` from `self.program`), so to score a mutant we
        // must build a per-mutant engine whose program carries the mutant body in
        // place of the original `f` (else the engine would re-export the unchanged
        // original — every mutant would discharge identically, a false survivor).
        let mutant_program = program_with_mutant(base_program, &mutant.item);
        let mutant_engine =
            crate::engine::LeanEngine::new(mutant_program.clone(), lean_package_root());
        // The mutant's contract obligation (the same closure the real item carries —
        // a mutant body references the same spec-fns), over the mutant program.
        let called = reachable_spec_fn_names_full(&mutant_program, &mutant.item);
        let obligation = crate::obligation::Obligation::contract_for_fn(&mutant.item, called);
        // The Lean fragment's admission is the export-success + auto-tier test
        // (REQ-9: a non-admitted mutant is "untested against lean", never a kill).
        let admitted = mutant_engine.admits_auto(&obligation);
        let verdict = if admitted {
            mutant_engine.discharge(&obligation, &CovenantRecord::none())
        } else {
            // Not attempted (the fragment does not admit it) — a placeholder Unknown;
            // `lean_mutant_outcome` maps `admitted = false` to UntestedAgainstLean
            // regardless of the verdict.
            crate::engine::Verdict::Unknown(crate::engine::Reason::IncompleteUnknown(
                "not admitted by the Lean fragment (untested against lean)".to_string(),
            ))
        };
        let outcome = crate::engine::lean_mutant_outcome(admitted, &verdict);
        // The #101 equivalence probe is a §0.1 verus meta-query outside the engine
        // interface in v1 (not threaded on the Lean-only path) — report the raw
        // survivor (proven_equivalent = false), never a silent exclusion.
        tally.record(outcome, false);
    }
    tally
}

/// Build a copy of `base` with the fn named `mutant.name` replaced by `mutant`
/// (`.design/verified/proof-backends.md` REQ-9 — the per-mutant program the LeanEngine
/// exports from). The LeanEngine resolves an obligation's item by name from its stored
/// program, so a mutant must be swapped in for the engine to discharge the mutated
/// body (not the original). The spec-fn deps are unchanged (a mutant keeps the
/// original's signature + contract).
fn program_with_mutant(base: &Program, mutant: &thermite_syntax::FnItem) -> Program {
    let items = base
        .items
        .iter()
        .map(|i| match i {
            Item::Fn(f) if f.name == mutant.name => Item::Fn(mutant.clone()),
            other => other.clone(),
        })
        .collect();
    Program { items }
}

/// Build the Unverifiable-skip cert a Lean `Unknown` produces under `--engine
/// lean` (`.design/verified/proof-backends.md` OQ-1 — "non-exportable → honest
/// Unverifiable/skip reporting"). Level::L0 with a structured reject naming the skip
/// reason, never a false `Proven` and never a silent pass.
fn lean_unverifiable_cert(base: &Certificate, reason: &crate::engine::Reason) -> Certificate {
    let detail = match reason {
        crate::engine::Reason::VerusTimeout(d) | crate::engine::Reason::IncompleteUnknown(d) => {
            d.clone()
        }
    };
    // Classify the non-discharge through the cert-level vocabulary (REQ-1/AC-1): a Lean
    // elaboration/kernel-budget exhaustion is `KernelBudget` (produced UPSTREAM via the
    // textually-distinct signal in the reason detail, Q-KBSIGNAL), never mis-labelled a
    // solver `Timeout`. The classification is recorded in the reject reason so the skip
    // is honestly attributed (a budget exhaustion vs a plain unverifiable skip).
    let cert_verdict = crate::verdict::cert_verdict_for_lean(
        &detail,
        &crate::engine::Verdict::Unknown(reason.clone()),
    );
    Certificate::rejected(
        &base.item,
        base.effects.clone(),
        false,
        crate::manifest::RejectReason {
            cause: "LeanUnverifiable".to_string(),
            detail: format!(
                "the Lean engine could not discharge this item [{}] (not exportable / not \
                 auto-dischargeable / interactive-only / budget-exhausted — an honest skip \
                 under --engine lean): {detail}",
                cert_verdict.kind()
            ),
        },
    )
}

/// Run the L2 (Kani bounded model check) pipeline for every `fn` item in `path`,
/// returning one [`Certificate`] (at `Level::L2` on success) per `fn` in source
/// order (`.design/lower/l2-kani.md` REQ-7). This is the explicit `--level l2`
/// path: `forge check --level l2 <file>` runs it instead of the default L3 verus
/// path (`check_file`). #9 does not wire L2 as an automatic fallback on a verus
/// timeout (that is #10's `level_from_summary` change); `--level l2` is an
/// explicit choice (REQ-7 / `goal.md` R-DEFER-4).
///
/// Pipeline order (parallel to `check_file`): parse → validate → check_effects →
/// per item (`thermite_lower::lower_l2` of the item's isolated sub-program) →
/// `kani::run_kani` → an L2 `Certificate`. A `spec fn` carries no `req`/`ens`
/// contract (§4.2), so it has no L2 obligation to discharge — it is a pure shared
/// dependency woven into every `fn`'s sub-program (so a `fn` whose `ens` calls
/// `spec_sum` lowers + checks), and produces no certificate of its own. Stages
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
        // The explicit `--level l2` (kani) path does not weave the §9 composition
        // deps: #52's external_body arm lives in the L3 `lower`, not `lower_l2`, and
        // the composition oracle (`conformance/composition`) is L3-only. A boundary
        // caller at L2 is out of #52's v0.1 scope, so this stays the prior shape
        // (no fn-dep weaving) to keep the explicit-L2 behavior byte-stable.
        // The explicit `--level l2` path does not weave the §9 composition deps
        // (#52) nor the #68 ADT decls — the kani-backed L2 corpus is scalar-only
        // and the composition/ADT oracles are L3; keep this byte-stable.
        let sub = item_subprogram(item, &spec_items, &[], &[]);
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
    /// A `fx diverge` item (`.design/forge/check.md` REQ-8,
    /// `degrade-ladder.md` REQ-9): an event loop that may not terminate, so it
    /// cannot claim L3-total — it caps at `Level::L1` = partial correctness and is
    /// exempt from the §7 mutation/strengthen gate (which validates a
    /// strong-functional `ens`, the wrong instrument for a partial-correctness
    /// loop). The structural analog of [`GateOutcome::BoundaryL1`]: the cert is
    /// built before any prover runs, keyed strictly on the `fx diverge`
    /// declaration (R-DEFER-9 — a non-diverge fn never reaches this arm).
    DivergeL1(Certificate),
    /// A triage / slag-validation reject: the item does not certify — the cert.
    Rejected(Certificate),
    /// A non-slag item that passed all four triage checks: run the normal L3 path.
    ProceedToL3,
}

/// Run the #6 gate for one `fn` (slag-validate → triage → L1-vs-L3 fork). The two
/// components compose per `.design/forge/slag.md`:
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

    // #16 boundary (FFI) path, detected first (`.design/boundary/ffi-boundary.md`
    // REQ-5, §9): a `#[boundary("crate::path")]` fn's foreign body is unproven, so
    // it never enters the L3 (verus) / L2 (kani) / mutation / strengthening paths —
    // it certifies at L1 to-the-boundary. The §7.1 (a)/(b)/(c) triage still applies
    // (slag-adjacent: it exempts proving the body, not stating a non-vacuous
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
        // Slag path: validate the mandatory fields first (it gates whether rule
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
        // Valid fields: triage still applies (a)/(b)/(c) — slag exempts only (d)
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
    } else if fn_is_diverge(f) {
        // #88 `fx diverge` partial-correctness cap (`.design/forge/check.md`
        // REQ-8, `degrade-ladder.md` REQ-9), mirroring the `#[boundary]`
        // short-circuit above. A diverge fn (effect row contains
        // `Effect::Diverge`, §4.1) is an event loop that may not terminate, so it
        // cannot claim L3-total correctness — it caps at `Level::L1` = partial
        // correctness and is exempt from the §7 mutation/strengthen gate (which
        // validates a strong-functional `ens`, inapplicable to a partial-correctness
        // loop whose `ens` is inherently a weak shape — `run`'s `ens result <= 256`
        // is met by `return 0`, so the §7 battery would mis-reject it `WeakContract`
        // at L0). The §7.1 (a)/(b)/(c) triage still applies (divergence exempts
        // proving total correctness, not stating a non-vacuous contract — a diverge
        // fn with a vacuous `ens` is still rejected), exactly as for `#[boundary]`.
        // The cap is built here, before any prover runs, keyed strictly on the
        // `fx diverge` declaration (R-DEFER-9): it is a structural cap, not a
        // verus-timeout degrade and not a counterexample (degrade-ladder.md REQ-9).
        // Verus is not run on the diverge body: in the per-item sub-program `run`'s
        // loop calls `backspace`/`insert_str` whose `req`s the loop invariant does
        // not (and need not) re-establish, so verus would report a
        // spurious-for-partial-correctness failure. The boundary-style L1-no-verus
        // reading (`.design/forge/check.md` REQ-8 reading (b), the sanctioned
        // fallback) applies. The runtime contract checks (`thermite_lower::l1`) plus
        // the proven edit core (`insert_str`/`backspace` are L3) carry the assurance.
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
            crate::vacuity::VacuityVerdict::Passed => {
                GateOutcome::DivergeL1(diverge_l1_cert(f.name.clone(), effects))
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

/// True iff `f`'s effect row contains `diverge` (§4.1: "divergence requires
/// `fx diverge` in the row"). Mirrors `thermite_lower`'s `fn_is_diverge` (the
/// single source of truth for the §4.1 termination exemption in the lowerer); the
/// two share the row-shape predicate so the #88 L1 cap and the #87 termination
/// exemption fire on the same set of fns. Keyed on the shape of the
/// effect row — a `pure` row never diverges; a `Set` row diverges iff it lists
/// [`thermite_syntax::ast::Effect::Diverge`]. The cap is applied only to a fn
/// that declares `fx diverge`, never to a normal fn (R-DEFER-9).
fn fn_is_diverge(f: &thermite_syntax::FnItem) -> bool {
    use thermite_syntax::ast::{Effect, EffectRow};
    matches!(&f.contract.fx, EffectRow::Set(es) if es.contains(&Effect::Diverge))
}

/// Build a `fx diverge` partial-correctness L1 certificate (`.design/forge/check.md`
/// REQ-8, `degrade-ladder.md` REQ-9). The diverge analog of
/// [`Certificate::boundary_l1`] / [`Certificate::slag_l1`]: `Level::L1` (partial
/// correctness — the loop runs under its runtime contract checks; termination +
/// strong functional postcondition not claimed), `slag: false`, `boundary: false`,
/// the §7.1 triage bools graduated to live-`false` (a diverge fn passes (a)/(b)/(c)
/// before this is built), and a single discharged obligation recording the
/// partial-correctness verdict (not a verus obligation — no proof was run on the
/// non-terminating body). Mirrors the `boundary_l1` field layout exactly; the only
/// cap-specific datum is the obligation note. Built inline here (rather than as a
/// `manifest.rs` ctor) so the #88 fix stays within the authorized file manifest.
fn diverge_l1_cert(item: String, effects: Vec<String>) -> Certificate {
    let mut cert = Certificate::boundary_l1(item, effects, String::new());
    // Re-shape the boundary cert into the diverge cap: it is not a boundary fn
    // (its body is in-language and partially proven via the L3-proven edit ops it
    // calls), so clear the boundary flag/target and replace the obligation note.
    cert.boundary = false;
    cert.boundary_target = None;
    cert.obligations = vec![ObligationResult::discharged(
        "contract holds at L1 (diverge / partial correctness); termination not \
         claimed (§4.1 `fx diverge`); §7 mutation/strengthen gate exempt",
    )];
    cert
}

/// Build the per-item sub-`Program` that isolates `item`'s verification (§5.3).
///
/// - A `fn` is verified against itself, the file's `spec fn`s (the pure shared
///   dependencies its contract may reference), and the in-file `fn`s its body
///   transitively references (`fn_deps`, the #52 §9 composition weaving). A
///   regular reachable fn is woven with its real body (fully lowered + proved);
///   a `#[boundary]`/`#[slag]` reachable fn is woven as a
///   `#[verifier::external_body]` signature (`thermite_lower::lower`'s
///   composition arm), so `verus` resolves the foreign callee and the caller
///   proves through its contract (§9). Its obligations stay its own (§5.3) — a
///   sibling fn not in the closure never enters, so an unrelated sibling's
///   failure cannot leak in.
/// - A `spec fn` carries no `req`/`ens`/`fx` contract (`ast.rs` `SpecFnItem`,
///   §4.2): there is no L3 proof obligation to discharge, only well-formedness
///   (the `decreases` measure). It is verified against the set of `spec fn`s
///   alone (which already contains it), so a mutually-recursive spec fn still
///   resolves. The resulting cert records the spec fn's well-formedness as its
///   own discharged result — never a neighbor `fn`'s counterexample. A `spec fn`
///   has no `fn_deps` (its body can call only spec fns / combinators, §4.2).
fn item_subprogram(
    item: &Item,
    spec_items: &[Item],
    fn_deps: &[Item],
    adt_deps: &[Item],
) -> Program {
    match item {
        // The referenced `struct`/`enum` declarations first (#68 — so verus has
        // the type decls + their `well_formed` invariants in scope before any fn
        // that references them), then all pure spec-fn dependencies, then the
        // transitively reachable in-file `fn` dependencies (#52), then the item
        // itself last (so a forward reference resolves; the lowerer dedups
        // combinator defs regardless of order). Empty `adt_deps` for a fn that
        // references no ADT (the pure scalar corpus), so the existing sub-program
        // is byte-stable (no regression).
        Item::Fn(_) => {
            let mut items = adt_deps.to_vec();
            items.extend(spec_items.iter().cloned());
            items.extend(fn_deps.iter().cloned());
            items.push(item.clone());
            Program { items }
        }
        // A checked spec fn's sub-program is distinct + minimal (#71): it weaves
        // `item` plus only the spec fns `item` transitively references (`spec_items`
        // here is the reachable spec-fn closure from `reachable_spec_fn_deps`, which
        // includes `item` itself), not every `spec fn` in the file. Weaving all spec
        // fns made every spec fn in a multi-spec-fn file lower to byte-identical
        // Verus (the checked item was indistinguishable — it just rode along in the
        // full set), so the proof-cache content-address collided and the first
        // item's cert was served for every sibling (the cert identity was a
        // neighbor's, violating §6). With only the reachable deps, each spec fn's
        // sub-program differs by its own focused body → a distinct content-address
        // → a distinct, correctly-named cert. A spec fn that references no other
        // spec fn (the `list_fold.th` instances) gets a sub-program of just itself
        // (+ its ADT decls) → three distinct lowerings (different fold steps). A
        // spec fn that does reference a sibling Y still includes Y (reachability),
        // so verus resolves Y and mutual recursion still verifies (§4.2). A spec fn
        // has no `fn` dependencies to weave (§4.2), but a recursive fold over an ADT
        // does reference the `enum` decl, so weave the referenced ADT decls first
        // (#68 — without `enum List` in scope the fold's lowering degrades to L0).
        Item::SpecFn(_) => {
            let mut items = adt_deps.to_vec();
            items.extend(spec_items.iter().cloned());
            Program { items }
        }
        // A `struct`/`enum` whose `inv`/`well_formed` predicate names a user
        // `spec fn` must weave that spec fn's definition into its sub-program —
        // exactly as the `Item::Fn` arm weaves the file's `spec_items` (#232).
        // The stale "dead-in-1a: dies at the validator" premise was wrong: a
        // struct with an `inv` lowers to a `pub open spec fn well_formed` whose
        // body calls the named spec fn, and live `forge check` does produce a
        // cert for it (at L0 without the def — E0425 `cannot find function`).
        // Weave the referenced ADT decls first (#68 — type decls in scope), then
        // the spec-fn deps (so the `well_formed` body resolves and the per-item
        // `spec_fn_param_type_map` is non-empty, restoring the #229 REQ-5 cast
        // off the `as u64` fallback), then the item itself last (forward refs
        // resolve; the lowerer dedups regardless of order). An ADT carries no
        // `fn` dependency closure (a `well_formed` body calls only spec fns /
        // combinators, §4.2), so `fn_deps` is intentionally unwoven here. Empty
        // `adt_deps`/`spec_items` for an `inv`-free struct keeps the sub-program
        // the item alone (byte-stable for the no-invariant corpus).
        Item::Struct(_) | Item::Enum(_) => {
            let mut items = adt_deps.to_vec();
            items.extend(spec_items.iter().cloned());
            items.push(item.clone());
            Program { items }
        }
        // Forge-tier item (stage1-forge-tier.md REQ-3): no v1 sub-program/cert
        // consumer yet (increments 2b-3); the item carries no v1 lowering output, so
        // its sub-program is the item alone (inert — lowers to nothing), mirroring
        // the non-fn ADT-decl path's self-contained weave.
        Item::Forge(_) => {
            let mut items = adt_deps.to_vec();
            items.extend(spec_items.iter().cloned());
            items.push(item.clone());
            Program { items }
        }
    }
}

/// The in-file `Item::Fn`s a fn named `start` transitively references — the §9
/// composition dependencies woven into `start`'s sub-program (#52). Resolves
/// `closure::reachable_in_file_fns` (the reused #17 call-graph walk) to the
/// matching `Item::Fn` clones from `program`, in source order (deterministic,
/// R-CODE-5). Excludes `start` itself and every `spec fn` (the latter is woven by
/// the separate `spec_items` set — no duplication). For a fn with no in-file fn
/// references (e.g. the pure corpus `sum`, which calls only the `spec fn`
/// `spec_sum`) this is empty, so the §52 weaving is a no-op and no external_body
/// is emitted (the AC-4 corpus-unaffected invariant).
fn reachable_fn_deps(program: &Program, start: &str) -> Vec<Item> {
    let names = crate::closure::reachable_in_file_fns(program, start);
    program
        .items
        .iter()
        .filter(|i| matches!(i, Item::Fn(_)) && names.contains(i.name()))
        .cloned()
        .collect()
}

/// The set of in-file `fn` names that participate in a mutual-recursion cycle
/// (`a -> b -> a`, `a -> b -> c -> a`, …, SCC size ≥ 2) whose termination is not
/// supplied — i.e. a cycle containing at least one non-`fx diverge` member that
/// lacks a `dec` measure (`.design/basis/12-mutual-recursion.md` REQ-1/REQ-2,
/// crosslink #121/#113). These are the only mutual-cycle members `forge check`
/// rejects.
///
/// A `fn` `f` is in a mutual cycle iff it is reachable from itself through at
/// least one other `fn` (`g != f`, `g` reachable from `f` and `f` reachable from
/// `g` — the same SCC of size ≥ 2). This generalizes the
/// validator's direct self-call rule (C9 REQ-2, `block_calls_name`) from a
/// self-edge to a call-graph cycle.
///
/// C11 (the #121 refinement): the reject is conditional on missing `dec`. A
/// mutual cycle where every member carries a `dec` measure (or is `fx diverge`)
/// is no longer rejected here — it falls through to the normal per-item
/// lower/verus ladder, where the existing C9 source-order single-`verus!`-block
/// emission (`lower`/`lower_fn`) presents Verus a valid mutual-`decreases` group.
/// Verus discovers the recursive SCC from each member's emitted `decreases` and
/// proves the group terminates (each cross-call strictly decreases the measure)
/// → L3 (`12-mutual-recursion.md` REQ-1/REQ-3/REQ-4, grounded: `is_even`/`is_odd`
/// `dec n` cross `n-1` → `4 verified, 0 errors`). A dec-complete cycle whose
/// measures don't decrease still reaches Verus and is rejected there as a clean
/// `could not prove termination` L0 (the same shape as the single-fn
/// non-decreasing L0, C9 REQ-4 / `12-mutual-recursion.md` AC-2): the `decreases`
/// is the only thing between the cycle and L0, as for a self-recursive fn
/// (R-DEFER-9). So this function returns a member only when its cycle has a
/// structural reason to reject pre-Verus: a member lacking `dec`.
///
/// The reject this set drives (the per-item loop emits a `Certificate::rejected`,
/// `Level::L0`, cause `MutualRecursionMissingDecreases`) fires before lowering /
/// verus, so a missing-`dec` cycle is a clean certificate verdict (a parseable
/// cert array, exit non-zero — the verdict-in-cert shape, §5.1 / R-SPEC-3) rather
/// than the raw Verus VIR-error abort (`recursive function must have a decreases
/// clause`, `encountered-vir-error: true`) that `classify_verus_outcome` maps to
/// a `ForgeError::VerusOutput` environment crash (exit 2, empty `--json` stdout,
/// no certificate) — `12-mutual-recursion.md` AC-3.
///
/// A whole class, not the one reported pair (`goal.md` "fix the cause … its whole
/// class"): the membership test runs over every `fn`, so any missing-`dec` mutual
/// cycle of size ≥ 2 (pairs, 3-cycles, …) is caught, and when one member of a
/// cycle lacks `dec`, the whole non-diverge cycle is rejected (Verus would reject
/// the entire recursive group; a clean per-member cert mirrors that).
/// A `fx diverge` `fn` is exempt from being a reject trigger and is itself never
/// rejected for missing `dec` (the #88 exemption: a diverge fn lowers with
/// `#[verifier::exec_allows_no_decreases_clause]` and is L1-capped, so it never
/// enters the termination check).
///
/// Deterministic (R-CODE-5): a pure function of the parsed `Program`, returning a
/// sorted `BTreeSet` (the `reachable_in_file_fns` walk is itself source-ordered +
/// cycle-safe + bounded — `closure::CallGraph`).
fn mutual_recursion_cycle_fns(program: &Program) -> std::collections::BTreeSet<String> {
    use std::collections::BTreeSet;

    // Index `Item::Fn`s by name so a cycle member's `dec`/diverge shape is a O(1)
    // lookup while scanning a member's SCC.
    let fns: std::collections::BTreeMap<&str, &thermite_syntax::FnItem> = program
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Fn(f) => Some((f.name.as_str(), f)),
            _ => None,
        })
        .collect();

    // The full SCC (size ≥ 2) a member sits in, excluding itself: the set of
    // other fns `g` reachable from `f` and from which `f` is reachable. Empty iff
    // `f` is not in a mutual cycle (a pure direct self-recursion `f -> f` has an
    // empty SCC-minus-self — the witness `g` must be distinct, so it is excluded,
    // staying the C9 supported self-recursion case). Reuses only the existing
    // source-ordered, cycle-safe, bounded `reachable_in_file_fns` walk.
    let scc_minus_self = |f: &str| -> Vec<String> {
        crate::closure::reachable_in_file_fns(program, f)
            .into_iter()
            .filter(|g| g != f && crate::closure::reachable_in_file_fns(program, g).contains(f))
            .collect()
    };

    let mut members: BTreeSet<String> = BTreeSet::new();
    for item in &program.items {
        let Item::Fn(f) = item else {
            continue;
        };
        // The #88 diverge exemption: a `fx diverge` fn is
        // non-terminating, lowers with `#[verifier::exec_allows_no_decreases_clause]`,
        // and is L1-capped — it never enters the termination check, so it is
        // never rejected for missing `dec` (mirrors the validator's
        // `block_calls_name` diverge skip + the single-fn diverge L1 cap).
        if fn_is_diverge(f) {
            continue;
        }
        let scc = scc_minus_self(&f.name);
        if scc.is_empty() {
            // Not in a mutual cycle (≥ 2): a non-cyclic fn or a pure direct
            // self-recursion (the C9 supported case) — never rejected here.
            continue;
        }
        // C11 (REQ-2): reject `f` only if its cycle is termination-incomplete —
        // i.e. at least one non-`fx diverge` member of the SCC (including `f`
        // itself) lacks a `dec` measure. A cycle where every non-diverge member
        // carries `dec` falls through to the lower/verus ladder (REQ-1): Verus
        // discovers the SCC from each member's emitted `decreases` and proves the
        // mutual-decreases group → L3. The whole non-diverge cycle is rejected
        // together when any member is missing `dec` (Verus rejects the entire
        // recursive group; the clean per-member cert mirrors that).
        let cycle_missing_dec = std::iter::once(f.name.clone())
            .chain(scc)
            .filter_map(|name| fns.get(name.as_str()).copied())
            .any(|m| !fn_is_diverge(m) && m.dec.is_none());
        if cycle_missing_dec {
            members.insert(f.name.clone());
        }
    }
    members
}

/// The in-file `Item::SpecFn`s a spec fn named `start` transitively references —
/// the spec-fn analog of [`reachable_fn_deps`], including `start` itself (#71).
///
/// A `spec fn`'s §5.3 sub-program must be distinct and minimal: it weaves `start`
/// plus only the spec fns `start`'s body transitively calls — not every `spec fn`
/// in the file. Weaving all of them made every spec fn in a multi-spec-fn file
/// (e.g. `len_list`/`sum_list`/`all_positive` in `conformance/list_fold.th`) lower
/// to byte-identical Verus, so the proof-cache content-address collided and the
/// first item's certificate was served for every sibling (the cert identity was a
/// neighbor's, violating §6 "the certificate lists every function's level"). With
/// only the transitive deps, each spec fn's sub-program differs (its own focused
/// body) → a distinct content-address → a distinct, correctly-named certificate.
///
/// `start` is included in the result (unlike [`reachable_fn_deps`], which excludes
/// its own start because the checked fn is pushed separately): the `Item::SpecFn`
/// arm of [`item_subprogram`] builds the whole spec-fn set from this result, so
/// `start` must be in it. A spec fn that references no other spec fn (the
/// `list_fold.th` instances) yields exactly `{start}`.
///
/// Resolution mirrors the §52/§68 pattern: walk the spec-fn call graph from
/// `start` over `Expr::Call`/`MethodCall` callee names that resolve to an in-file
/// `Item::SpecFn`, transitively (cycle-safe via a visited set so a mutually- or
/// self-recursive spec fn terminates), then return the matching `Item::SpecFn`
/// clones in source order (deterministic, R-CODE-5). A `spec fn` body can call only
/// other spec fns / combinators (§4.2), so no `Item::Fn` is ever pulled in.
///
/// One closure, all callers (the #192 lesson; crosslink #237 closure
/// unification). The closure-walk is the same `body ∪ dec` step the #226/#204
/// obligation closure uses ([`reachable_spec_fn_names_from_seed`]) — this fn
/// delegates to it, seeded at `{start}`, then maps the reached names back to their
/// `Item::SpecFn` clones. The prior body-only walk here was a drift hazard: a spec
/// fn whose `dec` calls another spec fn (`dec t_size(n)`) had that dec-position dep
/// dropped from the woven §5.3 sub-program, so the lowered Verus referenced an
/// undefined fn and the item died E0425 — fail-closed, but a completeness gap on
/// legitimate frozen-subset source. Walking `dec` too (via the shared closure)
/// weaves the dec-position dep, so the sub-program is self-contained.
fn reachable_spec_fn_deps(program: &Program, start: &str) -> Vec<Item> {
    // The set of in-file spec-fn names → their declaring item, for resolution.
    let spec_decls: std::collections::BTreeMap<&str, &thermite_syntax::SpecFnItem> = program
        .items
        .iter()
        .filter_map(|i| match i {
            Item::SpecFn(s) => Some((s.name.as_str(), s)),
            _ => None,
        })
        .collect();

    // Seed the shared `body ∪ dec` closure at `{start}` (which is itself a spec
    // fn): `reachable_spec_fn_names_from_seed` inserts `start` into `reached` on the
    // first pop and then walks its `body ∪ dec` — so the result includes `start`
    // plus the transitive `body ∪ dec` deps, exactly the set this weaver needs. A
    // name with no in-file spec-fn decl is dropped (a combinator / scheme /
    // cross-file callee — §4.2 pure).
    let mut seed: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    seed.insert(start.to_string());
    let reached: std::collections::BTreeSet<String> =
        reachable_spec_fn_names_from_seed(&spec_decls, seed, program)
            .into_iter()
            .collect();

    // Return the reached spec fns in source order (deterministic) so the lowered
    // sub-program is byte-stable for a given program.
    program
        .items
        .iter()
        .filter(|i| matches!(i, Item::SpecFn(_)) && reached.contains(i.name()))
        .cloned()
        .collect()
}

/// The per-item backend-neutral obligation set (`.design/verified/
/// proof-backends.md` REQ-1/REQ-1.2, #204). The contract certification obligation
/// (always present) plus, for an item whose full-expression-position called-spec-fn
/// closure is non-empty, the registry-termination obligation (REQ-1.2, conjoined
/// item-wide). The set is what the engine discharges; the conjunction rule (REQ-1.2)
/// requires every obligation in the set discharged for the item to certify.
struct ItemObligations {
    /// The contract certification obligation (`.design/verified/proof-backends.md`
    /// §1) — the head of the set; the L3 discharge routes through the engine on it.
    contract: crate::obligation::Obligation,
    /// The registry-termination obligation (REQ-1.2), present iff the item's
    /// called-spec-fn closure is non-empty. `None` for a spec-fn-free item.
    registry_termination: Option<crate::obligation::Obligation>,
}

/// Mint the per-item obligation set (`.design/verified/proof-backends.md`
/// REQ-1/REQ-1.2, #204), using the corrected full-expression-position called-spec-fn
/// closure (the #226 fix). For an `Item::Fn` the closure seeds at
/// `req ∪ ens ∪ body ∪ dec`; for an `Item::SpecFn` at its own `body ∪ dec`; both
/// step over each reached spec-fn's `body ∪ dec`. A non-empty closure also mints
/// the registry-termination obligation (REQ-1.2). An ADT item carries no
/// certification obligation in v1 (it has no contract / spec body), so it gets an
/// empty contract obligation over an empty body (the engine never discharges it —
/// an ADT dies at the validator before a cert is assembled), keeping the function
/// total without a panic. The set asserts, via the engine fragment gate, that the
/// Verus engine admits every minted class (REQ-1.2(a)).
fn mint_item_obligations(program: &Program, item: &Item) -> ItemObligations {
    use crate::engine::Engine as _;
    use crate::obligation::{AstSlice, Obligation};
    let (contract, called) = match item {
        Item::Fn(f) => {
            let called = reachable_spec_fn_names_full(program, f);
            (Obligation::contract_for_fn(f, called.clone()), called)
        }
        Item::SpecFn(s) => {
            let called = reachable_spec_fn_names_full_spec(program, s);
            (Obligation::contract_for_spec_fn(s, called.clone()), called)
        }
        // An ADT item has no in-language certification obligation in v1 (it dies at
        // the validator before a cert is assembled). Mint an empty contract
        // obligation so the function is total without a panic (R-APG-1); it is
        // never discharged.
        // A forge-tier item (stage1-forge-tier.md REQ-3) likewise has no in-language
        // certification obligation in v1 (no v1 consumer until increments 2b-3); mint
        // the same empty contract obligation as the ADT-decl arm so the function stays
        // total without a panic (R-APG-1) — it is never discharged.
        Item::Struct(_) | Item::Enum(_) | Item::Forge(_) => (
            Obligation {
                item: item.name().to_string(),
                class: crate::obligation::ObligationClass::Contract,
                role: crate::obligation::ObligationRole::Certification,
                ast_slice: AstSlice::Block(Box::new(thermite_syntax::Block {
                    stmts: Vec::new(),
                    tail: None,
                })),
                env: crate::obligation::ObligationEnv::default(),
            },
            Vec::new(),
        ),
    };
    // REQ-1.2: a non-empty called-spec-fn closure mints the registry-termination
    // obligation (the descent measures of every reached spec-fn). The `ast_slice`
    // is the item's own body (the contract obligation's slice).
    let registry_termination =
        Obligation::registry_termination(item.name(), contract.ast_slice.clone(), called);
    // REQ-1.2(a) conjunction gate: the Verus engine must admit every minted class
    // (its dec-check is the common registry-termination discharge path). The Verus
    // fragment admits the whole frozen subset, so this holds; a narrower future
    // engine that did not admit a class would block the conjunction (the obligation
    // would be an honest `Unknown`, never a silent skip). `debug_assert` records the
    // invariant without changing the release verdict (R-CODE-2 — no panic in prod).
    let engine = crate::engine::VerusEngine;
    debug_assert!(
        engine.fragment().admits(&contract),
        "the Verus engine admits the CONTRACT class (REQ-1.2(a))"
    );
    if let Some(rt) = &registry_termination {
        debug_assert!(
            engine.fragment().admits(rt),
            "the Verus engine admits the REGISTRY-TERMINATION class (REQ-1.2(a))"
        );
    }
    ItemObligations {
        contract,
        registry_termination,
    }
}

/// The contract certification obligation for one item, minted with the same #226
/// full-expression-position closure the check pipeline uses (`.design/forge/
/// audit-manifest.md` REQ-8 / OQ-4). A thin `pub(crate)` re-export of
/// [`mint_item_obligations`]`(program, item).contract` — the head of the per-item
/// set, the obligation `LeanEngine::discharge` exports via `export_item` (the
/// `check::check_file_with_engine` Lean path passes `&obligations.contract`).
///
/// The audit's Lean-fragment membership probe (`audit::LeanFragment`) needs the
/// contract obligation to dry-run `lean_export::export_item`; it must be the
/// byte-identical #226 closure (`reachable_spec_fn_names_full` /
/// `reachable_spec_fn_names_full_spec` → `Obligation::contract_for_fn` /
/// `contract_for_spec_fn`), because `export_item`'s hard gate cross-checks the
/// obligation's `called` closure against the spec-calls in `req ∪ ens ∪ body ∪ dec`
/// — a forked/weaker closure would yield spurious `IncompleteRegistry` refusals (or
/// mask real ones — the Pin B/C/G bottom-poisoning surface). Re-implementing the
/// closure is forbidden (REQ-8); this seam guarantees the audit and the
/// `--engine lean` admission decision can never disagree (AC-9). The
/// registry-termination obligation is engine-internal and not separately probed.
pub(crate) fn contract_obligation(program: &Program, item: &Item) -> crate::obligation::Obligation {
    mint_item_obligations(program, item).contract
}

/// The corrected full-expression-position called-spec-fn closure for a checked
/// `fn` (`.design/verified/proof-backends.md` REQ-1.2 / §4, the #226 fix
/// completing #224). Returns the spec-fn names the per-item Obligation env
/// (`obligation::Obligation::contract_for_fn`) and the registry-termination class
/// (REQ-1.2) key on, in source order (deterministic, R-CODE-5).
///
/// Why the existing `reachable_spec_fn_deps` is not enough (the #226 finding).
/// `reachable_spec_fn_deps` (the #71 weaving helper) seeds at a start spec-fn and
/// its closure step walks `decl.body` only — it never walks `decl.dec`, and for an
/// exec `fn` it does not even seed from the contract clauses. The §4 hard gate /
/// REQ-1.2 require the full expression-position closure: the seed is the spec-fn
/// calls in `req ∪ ens ∪ body ∪ dec(item)` and the closure step walks each reached
/// spec-fn's `body ∪ dec`. A `dec`-position spec-call (a `dec spec_size(t)` natural
/// tree measure) or a body/ens-position one that the body-only closure dropped
/// would leave the spec-fn absent from `R_item` — bottoming to the `intVal`
/// Int-bottom `0` and faking a descent / certifying a wrong contract
/// (`lean/Thermite/PinDecMeasure.lean` / `PinBodyRegistry.lean`). This function is
/// the forge-side closure mirror increment (i) owns; it walks every expression
/// position.
fn reachable_spec_fn_names_full(program: &Program, f: &thermite_syntax::FnItem) -> Vec<String> {
    let spec_decls: std::collections::BTreeMap<&str, &thermite_syntax::SpecFnItem> = program
        .items
        .iter()
        .filter_map(|i| match i {
            Item::SpecFn(s) => Some((s.name.as_str(), s)),
            _ => None,
        })
        .collect();

    // Seed: the spec-fn calls in `req ∪ ens ∪ body ∪ dec(item)` (the full
    // expression-position seed, #226).
    let mut seed: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    collect_expr_spec_fn_calls(&f.contract.req.expr, &spec_decls, &mut seed);
    for ens in &f.contract.ens {
        collect_expr_spec_fn_calls(&ens.expr, &spec_decls, &mut seed);
    }
    if let Some(body) = &f.body {
        collect_block_spec_fn_calls(body, &spec_decls, &mut seed);
    }
    if let Some(dec) = &f.dec {
        collect_expr_spec_fn_calls(&dec.expr, &spec_decls, &mut seed);
    }

    reachable_spec_fn_names_from_seed(&spec_decls, seed, program)
}

/// The corrected closure for a checked `spec fn` (`.design/verified/
/// proof-backends.md` REQ-1.2): the seed is the spec-fn's own `body ∪ dec`, and the
/// closure step walks each reached spec-fn's `body ∪ dec`. Returns the reached
/// spec-fn names in source order (deterministic). A spec fn carries no `req`/`ens`
/// (§4.2), so the seed is `body ∪ dec` only.
fn reachable_spec_fn_names_full_spec(
    program: &Program,
    s: &thermite_syntax::SpecFnItem,
) -> Vec<String> {
    let spec_decls: std::collections::BTreeMap<&str, &thermite_syntax::SpecFnItem> = program
        .items
        .iter()
        .filter_map(|i| match i {
            Item::SpecFn(d) => Some((d.name.as_str(), d)),
            _ => None,
        })
        .collect();
    let mut seed: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    collect_block_spec_fn_calls(&s.body, &spec_decls, &mut seed);
    collect_expr_spec_fn_calls(&s.dec.expr, &spec_decls, &mut seed);
    reachable_spec_fn_names_from_seed(&spec_decls, seed, program)
}

/// The transitive closure step shared by both seed builders (`.design/verified/
/// proof-backends.md` REQ-1.2 / #226): from a seed set of spec-fn names, close
/// under "a reached spec-fn's own `body ∪ dec` may call further spec-fns" — the
/// `dec` measure is walked too (the #226 correction over the body-only
/// `reachable_spec_fn_deps`). Returns the reached names in source order
/// (deterministic, R-CODE-5).
fn reachable_spec_fn_names_from_seed(
    spec_decls: &std::collections::BTreeMap<&str, &thermite_syntax::SpecFnItem>,
    seed: std::collections::BTreeSet<String>,
    program: &Program,
) -> Vec<String> {
    let mut reached: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut worklist: Vec<String> = seed.into_iter().collect();
    while let Some(name) = worklist.pop() {
        if !reached.insert(name.clone()) {
            continue;
        }
        if let Some(decl) = spec_decls.get(name.as_str()) {
            let mut callees: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
            // The closure step walks `body ∪ dec` (the #226 fix — not body-only).
            collect_block_spec_fn_calls(&decl.body, spec_decls, &mut callees);
            collect_expr_spec_fn_calls(&decl.dec.expr, spec_decls, &mut callees);
            for callee in callees {
                if !reached.contains(&callee) {
                    worklist.push(callee);
                }
            }
        }
    }
    program
        .items
        .iter()
        .filter_map(|i| match i {
            Item::SpecFn(s) if reached.contains(&s.name) => Some(s.name.clone()),
            _ => None,
        })
        .collect()
}

/// Collect the in-file spec-fn names a `Block` calls, walking statements + tail
/// (#71). Only a callee name resolving to an in-file `Item::SpecFn` (`spec_decls`)
/// is emitted — a combinator / scheme / cross-file callee is ignored (§4.2 pure).
pub(crate) fn collect_block_spec_fn_calls(
    block: &thermite_syntax::Block,
    spec_decls: &std::collections::BTreeMap<&str, &thermite_syntax::SpecFnItem>,
    out: &mut std::collections::BTreeSet<String>,
) {
    for stmt in &block.stmts {
        collect_stmt_spec_fn_calls(stmt, spec_decls, out);
    }
    if let Some(tail) = &block.tail {
        collect_expr_spec_fn_calls(tail, spec_decls, out);
    }
}

/// Collect the in-file spec-fn names one `Stmt` calls (#71), recursing into the
/// `let`/assign/return/if/loop/expr forms a spec-fn body may contain.
fn collect_stmt_spec_fn_calls(
    stmt: &thermite_syntax::Stmt,
    spec_decls: &std::collections::BTreeMap<&str, &thermite_syntax::SpecFnItem>,
    out: &mut std::collections::BTreeSet<String>,
) {
    use thermite_syntax::Stmt;
    match stmt {
        Stmt::Let { init, .. } => collect_expr_spec_fn_calls(init, spec_decls, out),
        Stmt::Assign { target, value } => {
            collect_expr_spec_fn_calls(target, spec_decls, out);
            collect_expr_spec_fn_calls(value, spec_decls, out);
        }
        Stmt::Return(opt) => {
            if let Some(e) = opt {
                collect_expr_spec_fn_calls(e, spec_decls, out);
            }
        }
        Stmt::If { cond, then, else_ } => {
            collect_expr_spec_fn_calls(cond, spec_decls, out);
            collect_block_spec_fn_calls(then, spec_decls, out);
            if let Some(b) = else_ {
                collect_block_spec_fn_calls(b, spec_decls, out);
            }
        }
        Stmt::Loop(node) => {
            for inv in &node.invs {
                collect_expr_spec_fn_calls(&inv.expr, spec_decls, out);
            }
            collect_expr_spec_fn_calls(&node.dec.expr, spec_decls, out);
            if let thermite_syntax::LoopKind::While(cond) = &node.kind {
                collect_expr_spec_fn_calls(cond, spec_decls, out);
            }
            collect_block_spec_fn_calls(&node.body, spec_decls, out);
        }
        Stmt::Expr(e) => collect_expr_spec_fn_calls(e, spec_decls, out),
        // break/continue carry no sub-expression (#93): no spec-fn call.
        Stmt::Break | Stmt::Continue => {}
    }
}

/// Collect the in-file spec-fn names one `Expr` calls (#71): an `Expr::Call` whose
/// leading `Path` segment names an in-file spec fn, an `Expr::MethodCall` whose
/// method name does, and every nested sub-expression (a call argument, a closure
/// body, a match arm — so a spec-fn call inside a scheme step closure is found).
pub(crate) fn collect_expr_spec_fn_calls(
    expr: &thermite_syntax::Expr,
    spec_decls: &std::collections::BTreeMap<&str, &thermite_syntax::SpecFnItem>,
    out: &mut std::collections::BTreeSet<String>,
) {
    use thermite_syntax::Expr;
    let note = |name: &str, out: &mut std::collections::BTreeSet<String>| {
        if spec_decls.contains_key(name) {
            out.insert(name.to_string());
        }
    };
    match expr {
        Expr::Call { callee, args } => {
            if let Expr::Path(segments) = callee.as_ref() {
                if let Some(first) = segments.first() {
                    note(first, out);
                }
            } else {
                collect_expr_spec_fn_calls(callee, spec_decls, out);
            }
            for a in args {
                collect_expr_spec_fn_calls(a, spec_decls, out);
            }
        }
        Expr::MethodCall {
            receiver,
            name,
            args,
        } => {
            note(name, out);
            collect_expr_spec_fn_calls(receiver, spec_decls, out);
            for a in args {
                collect_expr_spec_fn_calls(a, spec_decls, out);
            }
        }
        Expr::Field { receiver, .. } => collect_expr_spec_fn_calls(receiver, spec_decls, out),
        Expr::Closure { body, .. } => collect_expr_spec_fn_calls(body, spec_decls, out),
        Expr::Match { scrutinee, arms } => {
            collect_expr_spec_fn_calls(scrutinee, spec_decls, out);
            for arm in arms {
                // A C10 match guard is an `Expr` that may call a spec fn
                // (`.design/basis/11-ergonomics.md` REQ-3) — walk it too.
                if let Some(guard) = &arm.guard {
                    collect_expr_spec_fn_calls(guard, spec_decls, out);
                }
                collect_expr_spec_fn_calls(&arm.body, spec_decls, out);
            }
        }
        Expr::If { cond, then, else_ } => {
            collect_expr_spec_fn_calls(cond, spec_decls, out);
            collect_block_spec_fn_calls(then, spec_decls, out);
            collect_block_spec_fn_calls(else_, spec_decls, out);
        }
        Expr::Binary { lhs, rhs, .. } => {
            collect_expr_spec_fn_calls(lhs, spec_decls, out);
            collect_expr_spec_fn_calls(rhs, spec_decls, out);
        }
        Expr::Index { base, index } => {
            collect_expr_spec_fn_calls(base, spec_decls, out);
            match index {
                thermite_syntax::IndexArg::Single(e)
                | thermite_syntax::IndexArg::RangeTo(e)
                | thermite_syntax::IndexArg::RangeFrom(e) => {
                    collect_expr_spec_fn_calls(e, spec_decls, out)
                }
                thermite_syntax::IndexArg::Range(a, b) => {
                    collect_expr_spec_fn_calls(a, spec_decls, out);
                    collect_expr_spec_fn_calls(b, spec_decls, out);
                }
            }
        }
        Expr::Cast { expr, .. } => collect_expr_spec_fn_calls(expr, spec_decls, out),
        Expr::Ref { expr, .. } => collect_expr_spec_fn_calls(expr, spec_decls, out),
        Expr::Deref(inner) => collect_expr_spec_fn_calls(inner, spec_decls, out),
        Expr::StructLit { fields, .. } => {
            for (_, value) in fields {
                collect_expr_spec_fn_calls(value, spec_decls, out);
            }
        }
        Expr::Is { scrutinee, .. } => collect_expr_spec_fn_calls(scrutinee, spec_decls, out),
        // The prefix `!` (#92): a spec-fn call could sit under it (`!is_sorted(xs)`).
        Expr::Unary { expr, .. } => collect_expr_spec_fn_calls(expr, spec_decls, out),
        // Cluster C9-B (`.design/basis/10-recursion-tuples.md` REQ-8, #109): a
        // spec-fn call could sit in any tuple element or projection receiver.
        Expr::Tuple(elems) => {
            for e in elems {
                collect_expr_spec_fn_calls(e, spec_decls, out);
            }
        }
        Expr::TupleProj { receiver, .. } => collect_expr_spec_fn_calls(receiver, spec_decls, out),
        // A string literal is a leaf (`.design/basis/07-strings.md` REQ-1): no
        // sub-expression, so it calls no spec fn — the no-op leaf arm.
        Expr::IntLit { .. } | Expr::BoolLit(_) | Expr::Path(_) | Expr::StrLit(_) => {}
    }
}

/// The in-file `Item::Struct`/`Item::Enum` declarations the items in
/// `referrers` reference, transitively closed over the ADT type graph (#68 — the
/// ADT-decl analog of `reachable_fn_deps`). `check::item_subprogram` weaves these
/// first into a checked item's §5.3 sub-program so the per-item Verus emission has
/// the type decls + their `well_formed` invariants in scope — without them a fn
/// referencing `Account`/`Shape`/`List` lowers to Verus with no decl
/// (`error[E0425]: cannot find type`), verus fails, and the item degrades to L0.
///
/// `referrers` is the checked item plus every woven `fn` dependency (a regular
/// fn-dep woven with its real body may itself reference an ADT — the whole
/// dependency class must resolve, not just the checked item). The roots are
/// collected from each referrer's signature types (param + return), contract
/// clauses (`req`/`ens`/`dec`), and body; the closure then follows the field types
/// of each weaved struct/enum into the types they reference (a struct field of an
/// ADT type, a recursive `Cons(u64, Box<List>)` occurrence). The walk is
/// cycle-safe (a visited set keyed by type name, so the self-referential `List`
/// terminates) and deterministic (R-CODE-5): the result is returned in source
/// order. Empty for a referrer set that names no ADT (the pure scalar corpus —
/// `sum`/`binary_search`), so the existing sub-program is byte-stable (AC-6).
fn reachable_adt_deps(program: &Program, referrers: &[&Item]) -> Vec<Item> {
    // The set of in-file ADT type names → their declaring item, for resolution.
    let adt_decls: std::collections::BTreeMap<&str, &Item> = program
        .items
        .iter()
        .filter(|i| matches!(i, Item::Struct(_) | Item::Enum(_)))
        .map(|i| (i.name(), i))
        .collect();
    if adt_decls.is_empty() {
        return Vec::new();
    }

    // Seed the worklist with every ADT type name the referrers reference.
    let mut names: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for item in referrers {
        collect_item_adt_refs(item, &adt_decls, &mut names);
    }

    // Fixed-point over the type graph: a weaved struct/enum's field types may
    // reference further ADT decls (a struct field of an ADT type; the recursive
    // `Box<List>` occurrence). Cycle-safe via the `visited` set — the
    // self-referential `List` is entered once.
    let mut visited: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut worklist: Vec<String> = names.iter().cloned().collect();
    while let Some(name) = worklist.pop() {
        if !visited.insert(name.clone()) {
            continue;
        }
        if let Some(decl) = adt_decls.get(name.as_str()) {
            let mut refs: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
            collect_decl_field_adt_refs(decl, &adt_decls, &mut refs);
            for r in refs {
                names.insert(r.clone());
                if !visited.contains(&r) {
                    worklist.push(r);
                }
            }
        }
    }

    // Return the weaved decls in source order (deterministic), so the lowered
    // sub-program is byte-stable for a given program.
    program
        .items
        .iter()
        .filter(|i| matches!(i, Item::Struct(_) | Item::Enum(_)) && names.contains(i.name()))
        .cloned()
        .collect()
}

/// Collect the in-file ADT type names one `Item` (a checked fn / weaved fn-dep /
/// spec fn) references, from its signature types, contract clauses, and body
/// (#68). Only names resolving to an in-file `Item::Struct`/`Item::Enum`
/// (`adt_decls`) are emitted — a primitive / slice / `Option` type, or an unknown
/// name, is ignored.
fn collect_item_adt_refs(
    item: &Item,
    adt_decls: &std::collections::BTreeMap<&str, &Item>,
    out: &mut std::collections::BTreeSet<String>,
) {
    match item {
        Item::Fn(f) => {
            for p in &f.params {
                collect_type_adt_refs(&p.ty, adt_decls, out);
            }
            collect_type_adt_refs(&f.ret, adt_decls, out);
            collect_expr_adt_refs(&f.contract.req.expr, adt_decls, out);
            for ens in &f.contract.ens {
                collect_expr_adt_refs(&ens.expr, adt_decls, out);
            }
            if let Some(body) = &f.body {
                collect_block_adt_refs(body, adt_decls, out);
            }
        }
        Item::SpecFn(s) => {
            for p in &s.params {
                collect_type_adt_refs(&p.ty, adt_decls, out);
            }
            collect_type_adt_refs(&s.ret, adt_decls, out);
            collect_expr_adt_refs(&s.dec.expr, adt_decls, out);
            collect_block_adt_refs(&s.body, adt_decls, out);
        }
        // A struct/enum decl's own field types are followed by the type-graph
        // fixed point (`collect_decl_field_adt_refs`), not here.
        // Forge-tier item (stage1-forge-tier.md REQ-3): no v1 ADT-ref consumer yet
        // (increments 2b-3); references no in-file ADT here, mirroring the ADT-decl arm.
        Item::Struct(_) | Item::Enum(_) | Item::Forge(_) => {}
    }
}

/// Collect the in-file ADT type names a struct/enum decl references through its
/// field types (#68 transitive type graph): a struct field of an ADT type, a
/// tuple/struct enum-variant payload of an ADT type, and the recursive `Box<T>`
/// occurrence (`Cons(u64, Box<List>)`).
fn collect_decl_field_adt_refs(
    decl: &Item,
    adt_decls: &std::collections::BTreeMap<&str, &Item>,
    out: &mut std::collections::BTreeSet<String>,
) {
    match decl {
        Item::Struct(s) => {
            for field in &s.fields {
                collect_type_adt_refs(&field.ty, adt_decls, out);
            }
        }
        Item::Enum(e) => {
            for variant in &e.variants {
                match &variant.shape {
                    thermite_syntax::VariantShape::Unit => {}
                    thermite_syntax::VariantShape::Tuple(tys) => {
                        for ty in tys {
                            collect_type_adt_refs(ty, adt_decls, out);
                        }
                    }
                    thermite_syntax::VariantShape::Struct(fields) => {
                        for field in fields {
                            collect_type_adt_refs(&field.ty, adt_decls, out);
                        }
                    }
                }
            }
        }
        // Forge-tier item (stage1-forge-tier.md REQ-3): not an ADT decl → no field
        // type graph to follow (increments 2b-3); inert, mirroring the non-decl arm.
        Item::Fn(_) | Item::SpecFn(_) | Item::Forge(_) => {}
    }
}

/// Emit every in-file ADT type name reachable through a `Type` (#68): a
/// `Type::Named` resolving to an in-file ADT decl, recursing through `Box<T>`,
/// `&T`, `[T]`, and `Generic<T>` inner types so a `Box<List>` reaches `List`.
fn collect_type_adt_refs(
    ty: &thermite_syntax::Type,
    adt_decls: &std::collections::BTreeMap<&str, &Item>,
    out: &mut std::collections::BTreeSet<String>,
) {
    match ty {
        thermite_syntax::Type::Named(name) => {
            if adt_decls.contains_key(name.as_str()) {
                out.insert(name.clone());
            }
        }
        // Basis Stage 4 (`.design/basis/04-collections.md`): a bounded `Vec<T>`
        // recurses into its element type so a `Vec<Account>` reaches `Account`
        // (the element-invariant ADT ref), exactly as `Box<List>` reaches `List`.
        thermite_syntax::Type::Box(inner)
        | thermite_syntax::Type::Slice(inner)
        | thermite_syntax::Type::Vec(inner) => {
            collect_type_adt_refs(inner, adt_decls, out);
        }
        thermite_syntax::Type::Ref { inner, .. } => {
            collect_type_adt_refs(inner, adt_decls, out);
        }
        thermite_syntax::Type::Generic { arg, .. } => {
            collect_type_adt_refs(arg, adt_decls, out);
        }
        // Cluster C7 (`.design/basis/09-option-result.md` REQ-2): the built-in
        // `Option<T>` / `Result<T, E>` recurse into their type argument(s) so a
        // `Result<u64, ParseErr>` reaches the in-file error enum `ParseErr` (the
        // `E` parameter is an ordinary user ADT), exactly as `Box<List>` reaches
        // `List`. `Option`/`Result` themselves are built-ins, never an in-file ADT.
        thermite_syntax::Type::Option(inner) => {
            collect_type_adt_refs(inner, adt_decls, out);
        }
        thermite_syntax::Type::Result(ok, err) => {
            collect_type_adt_refs(ok, adt_decls, out);
            collect_type_adt_refs(err, adt_decls, out);
        }
        // Cluster C12 (`.design/basis/13-map.md` REQ-5): a `Map<K, V>` reaches an
        // in-file ADT in either type argument (a `Map<u64, Account>` reaches
        // `Account` — the #68 ADT weave so the value's decl is woven into the
        // per-item subprogram), so both the key and value are recursed, exactly as
        // `Result`'s two arguments. `Map` itself is a built-in, never an in-file ADT.
        thermite_syntax::Type::Map(k, v) => {
            collect_type_adt_refs(k, adt_decls, out);
            collect_type_adt_refs(v, adt_decls, out);
        }
        // Cluster C9-B (`.design/basis/10-recursion-tuples.md` REQ-8, #109): a
        // tuple type `(T, U, …)` reaches an in-file ADT in any element (a
        // `(Account, u64)` reaches `Account`), so every element is recursed.
        thermite_syntax::Type::Tuple(tys) => {
            for t in tys {
                collect_type_adt_refs(t, adt_decls, out);
            }
        }
        // Basis Stage 7 (`.design/basis/07-strings.md` REQ-2): `String` is a
        // built-in (not a user ADT) nullary type — no inner type to recurse into
        // and never an in-file ADT decl, so it references no ADT (the no-op leaf
        // arm alongside `Prim`/`Unit`).
        thermite_syntax::Type::Prim(_)
        | thermite_syntax::Type::Unit
        | thermite_syntax::Type::String => {}
    }
}

/// Emit every in-file ADT type name an `Expr` references (#68): a `StructLit`
/// path (`Account { .. }`), an `Is` variant (`s is Circle`), a `Match` arm's
/// `Pattern::Enum`/`Pattern::Struct` variant path — and recurse into every
/// sub-expression so a nested reference is not missed. A `Path`/`Field`/method
/// segment naming a variant or the enclosing type resolves through the
/// `adt_decls` map (the variant→enum resolution is handled by walking the
/// pattern/`Is`/`StructLit` paths against the in-file ADT name set, plus the
/// `enum`/`struct` names directly).
fn collect_expr_adt_refs(
    expr: &thermite_syntax::Expr,
    adt_decls: &std::collections::BTreeMap<&str, &Item>,
    out: &mut std::collections::BTreeSet<String>,
) {
    use thermite_syntax::Expr;
    // A `path` segment list may name `Type::Variant` or a bare `Type`/`Variant`.
    // Any segment resolving to an in-file ADT type name is a direct reference;
    // a bare variant name is resolved to its enum by `resolve_variant_owner`.
    let note_path = |segments: &[String], out: &mut std::collections::BTreeSet<String>| {
        for seg in segments {
            if adt_decls.contains_key(seg.as_str()) {
                out.insert(seg.clone());
            } else if let Some(owner) = resolve_variant_owner(seg, adt_decls) {
                out.insert(owner);
            }
        }
    };
    match expr {
        Expr::StructLit { path, fields } => {
            note_path(path, out);
            for (_, value) in fields {
                collect_expr_adt_refs(value, adt_decls, out);
            }
        }
        Expr::Is { scrutinee, variant } => {
            note_path(variant, out);
            collect_expr_adt_refs(scrutinee, adt_decls, out);
        }
        Expr::Path(segments) => note_path(segments, out),
        Expr::Match { scrutinee, arms } => {
            collect_expr_adt_refs(scrutinee, adt_decls, out);
            for arm in arms {
                collect_pattern_adt_refs(&arm.pattern, adt_decls, out);
                // A C10 match guard may reference an ADT
                // (`.design/basis/11-ergonomics.md` REQ-3).
                if let Some(guard) = &arm.guard {
                    collect_expr_adt_refs(guard, adt_decls, out);
                }
                collect_expr_adt_refs(&arm.body, adt_decls, out);
            }
        }
        Expr::Call { callee, args } => {
            collect_expr_adt_refs(callee, adt_decls, out);
            for a in args {
                collect_expr_adt_refs(a, adt_decls, out);
            }
        }
        Expr::MethodCall { receiver, args, .. } => {
            collect_expr_adt_refs(receiver, adt_decls, out);
            for a in args {
                collect_expr_adt_refs(a, adt_decls, out);
            }
        }
        Expr::Field { receiver, .. } => collect_expr_adt_refs(receiver, adt_decls, out),
        Expr::Closure { body, .. } => collect_expr_adt_refs(body, adt_decls, out),
        Expr::If { cond, then, else_ } => {
            collect_expr_adt_refs(cond, adt_decls, out);
            collect_block_adt_refs(then, adt_decls, out);
            collect_block_adt_refs(else_, adt_decls, out);
        }
        Expr::Binary { lhs, rhs, .. } => {
            collect_expr_adt_refs(lhs, adt_decls, out);
            collect_expr_adt_refs(rhs, adt_decls, out);
        }
        Expr::Index { base, index } => {
            collect_expr_adt_refs(base, adt_decls, out);
            match index {
                thermite_syntax::IndexArg::Single(e)
                | thermite_syntax::IndexArg::RangeTo(e)
                | thermite_syntax::IndexArg::RangeFrom(e) => {
                    collect_expr_adt_refs(e, adt_decls, out)
                }
                thermite_syntax::IndexArg::Range(a, b) => {
                    collect_expr_adt_refs(a, adt_decls, out);
                    collect_expr_adt_refs(b, adt_decls, out);
                }
            }
        }
        Expr::Cast { expr, ty } => {
            collect_expr_adt_refs(expr, adt_decls, out);
            collect_type_adt_refs(ty, adt_decls, out);
        }
        Expr::Ref { expr, .. } => collect_expr_adt_refs(expr, adt_decls, out),
        Expr::Deref(inner) => collect_expr_adt_refs(inner, adt_decls, out),
        // The prefix `!` (#92): an ADT ref could sit under it; descend the operand.
        Expr::Unary { expr, .. } => collect_expr_adt_refs(expr, adt_decls, out),
        // Cluster C9-B (`.design/basis/10-recursion-tuples.md` REQ-8, #109): an ADT
        // ref could sit in any tuple element or projection receiver; descend both.
        Expr::Tuple(elems) => {
            for e in elems {
                collect_expr_adt_refs(e, adt_decls, out);
            }
        }
        Expr::TupleProj { receiver, .. } => collect_expr_adt_refs(receiver, adt_decls, out),
        // A string literal is a leaf (`.design/basis/07-strings.md` REQ-1): no
        // sub-expression, no path — it references no ADT (the no-op leaf arm).
        Expr::IntLit { .. } | Expr::BoolLit(_) | Expr::StrLit(_) => {}
    }
}

/// Walk a `Block`'s statements + tail for ADT type references (#68).
fn collect_block_adt_refs(
    block: &thermite_syntax::Block,
    adt_decls: &std::collections::BTreeMap<&str, &Item>,
    out: &mut std::collections::BTreeSet<String>,
) {
    for stmt in &block.stmts {
        collect_stmt_adt_refs(stmt, adt_decls, out);
    }
    if let Some(tail) = &block.tail {
        collect_expr_adt_refs(tail, adt_decls, out);
    }
}

/// Walk one `Stmt` for ADT type references (#68), including a `let` annotation
/// type and a loop's spec clauses + body.
fn collect_stmt_adt_refs(
    stmt: &thermite_syntax::Stmt,
    adt_decls: &std::collections::BTreeMap<&str, &Item>,
    out: &mut std::collections::BTreeSet<String>,
) {
    use thermite_syntax::Stmt;
    match stmt {
        Stmt::Let { ty, init, .. } => {
            if let Some(ty) = ty {
                collect_type_adt_refs(ty, adt_decls, out);
            }
            collect_expr_adt_refs(init, adt_decls, out);
        }
        Stmt::Assign { target, value } => {
            collect_expr_adt_refs(target, adt_decls, out);
            collect_expr_adt_refs(value, adt_decls, out);
        }
        Stmt::Return(opt) => {
            if let Some(e) = opt {
                collect_expr_adt_refs(e, adt_decls, out);
            }
        }
        Stmt::If { cond, then, else_ } => {
            collect_expr_adt_refs(cond, adt_decls, out);
            collect_block_adt_refs(then, adt_decls, out);
            if let Some(b) = else_ {
                collect_block_adt_refs(b, adt_decls, out);
            }
        }
        Stmt::Loop(node) => {
            for inv in &node.invs {
                collect_expr_adt_refs(&inv.expr, adt_decls, out);
            }
            collect_expr_adt_refs(&node.dec.expr, adt_decls, out);
            if let thermite_syntax::LoopKind::While(cond) = &node.kind {
                collect_expr_adt_refs(cond, adt_decls, out);
            }
            collect_block_adt_refs(&node.body, adt_decls, out);
        }
        Stmt::Expr(e) => collect_expr_adt_refs(e, adt_decls, out),
        // break/continue carry no type and no sub-expression (#93): no ADT ref.
        Stmt::Break | Stmt::Continue => {}
    }
}

/// Walk one `Pattern` for ADT type references (#68): a `Pattern::Enum`/
/// `Pattern::Struct` path names a variant (or bare type), resolved to its in-file
/// enum/struct; nested sub-patterns are walked.
fn collect_pattern_adt_refs(
    pat: &thermite_syntax::Pattern,
    adt_decls: &std::collections::BTreeMap<&str, &Item>,
    out: &mut std::collections::BTreeSet<String>,
) {
    use thermite_syntax::Pattern;
    let note_path = |segments: &[String], out: &mut std::collections::BTreeSet<String>| {
        for seg in segments {
            if adt_decls.contains_key(seg.as_str()) {
                out.insert(seg.clone());
            } else if let Some(owner) = resolve_variant_owner(seg, adt_decls) {
                out.insert(owner);
            }
        }
    };
    match pat {
        Pattern::Enum { path, fields } => {
            note_path(path, out);
            for f in fields {
                collect_pattern_adt_refs(f, adt_decls, out);
            }
        }
        Pattern::Struct { path, fields, .. } => {
            note_path(path, out);
            for (_, p) in fields {
                collect_pattern_adt_refs(p, adt_decls, out);
            }
        }
        Pattern::Slice(pats) => {
            for sp in pats {
                if let thermite_syntax::SlicePat::Pat(p) = sp {
                    collect_pattern_adt_refs(p, adt_decls, out);
                }
            }
        }
        Pattern::Literal(e) => collect_expr_adt_refs(e, adt_decls, out),
        // A C10 or-pattern `p0 | p1 | …` (`.design/basis/11-ergonomics.md` REQ-4):
        // an ADT ref appears iff some alternative names one — walk each.
        Pattern::Or(alts) => {
            for alt in alts {
                collect_pattern_adt_refs(alt, adt_decls, out);
            }
        }
        Pattern::Wildcard | Pattern::Binding(_) => {}
    }
}

/// Resolve a bare variant name (`Circle`, `Cons`, `Nil`) to its declaring in-file
/// `enum` name (#68). The surface drops the enum qualifier in a `match` arm / `is`
/// (`Circle(r)`, `s is Circle`), so a pattern/`Is` path segment is a variant, not
/// the type — this maps it back to the `enum` decl that must be woven. Returns the
/// first declaring enum in source order (deterministic); `None` for a name that is
/// no declared variant (a binding, a combinator).
fn resolve_variant_owner(
    name: &str,
    adt_decls: &std::collections::BTreeMap<&str, &Item>,
) -> Option<String> {
    for decl in adt_decls.values() {
        if let Item::Enum(e) = decl {
            if e.variants.iter().any(|v| v.name == name) {
                return Some(e.name.clone());
            }
        }
    }
    None
}

/// Resolve the pinned solver seed for `path` (§5.3). v0.1 reads no lockfile yet
/// (`forge new`'s lockfile schema is minimal), so this returns the pinned
/// [`DEFAULT_SOLVER_SEED`]; the function is the single seam where lockfile
/// sourcing lands (#8) without changing `check_file`'s signature.
fn resolve_seed(_path: &Path) -> u64 {
    DEFAULT_SOLVER_SEED
}

/// Resolve the verus version that keys the proof cache for this run
/// (`.design/forge/proof-cache.md` REQ-5). Captured once per `check_file` so
/// every item keys against the same prover; a verus or thermite upgrade changes
/// this string, the key, and forces a universal re-verify.
///
/// Sourcing order (deterministic, R-CODE-5 — no wall-clock):
/// 1. `VERUS_VERSION` env var, when set — the pinned/CI override. This is also
///    the hermetic-test seam: a test pins a fixed version so the key is stable
///    even when the verus binary is later removed (the AC-1 decisive
///    solver-skip test populates the cache, then removes verus from PATH; the
///    pinned version keeps the key matching so the hit is served without a verus
///    spawn).
/// 2. otherwise `verus --version` stdout (the live binary's version).
///
/// A missing/unreadable verus version (verus absent and no `VERUS_VERSION`) is an
/// environment error (`ForgeError::VerusAbsent`), not a silent empty-string key
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

/// The three-way classification of one verus run (#11;
/// `.design/forge/solver-profiles.md` REQ-5). Deterministic; the profile content
/// it attaches on a timeout is not (§5.3).
///
/// `pub(crate)` so the backend-neutral Verus engine (`engine::VerusEngine::
/// verdict_of`, `.design/verified/proof-backends.md` REQ-2/REQ-3.1, #204) reads
/// it: the engine carries the verdict policy (the three-way map lifted to
/// `engine::Verdict`, with the REQ-3.1 fast-unknown remap) while `check.rs` keeps
/// the `run_verus` I/O.
#[derive(Debug, Clone)]
pub(crate) enum VerusOutcome {
    /// (a) Proved: `success == true && errors == 0` → `Level::L3`, one discharged
    /// summary obligation, no profile.
    Proved { verified: u64 },
    /// (c) Timeout / rlimit-exceeded: an error run whose stderr carries a
    /// `--profile` Z3 instantiation report (the timeout discriminator — `--profile`
    /// reports only on an rlimit-hit). Carries the parsed `SolverProfile`.
    Timeout {
        profile: SolverProfile,
        detail: String,
    },
    /// (b) Counterexample / the failure path: an error run without a profile (the
    /// existing #5 witness path, e.g. `postcondition not satisfied`). This bucket
    /// also absorbs the incompleteness-unknown edge (an `unknown` returned fast
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
/// `_check`, guaranteeing no `.` (verus derives the crate name from the file
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

/// A per-run scratch directory for one verus invocation, removed wholesale on
/// `Drop` (blocker #53). verus compiles the lowered `.rs` into a sibling binary
/// (`<stem>`, no extension, ~4.3M) in its working directory; the v0.1 driver ran
/// verus in the shared `std::env::temp_dir()` and cleaned only the `.rs` source,
/// so a verus run that errored mid-compile orphaned the binary → unbounded `/tmp`
/// growth under sustained multi-agent fresh-verification (the ENOSPC seen during
/// #18/#20). The fix: every verus run gets its own scratch dir (the `.rs` source,
/// verus's compiled-binary sibling, and any other verus artifact all land
/// inside), and this guard's `Drop` does a `remove_dir_all` that fires on every
/// exit path — success, a reported counterexample, or a `?` early-return on an
/// environment/IO error. Cleanup is best-effort (`let _ =`), never a panic
/// (R-CODE-2): a removal failure must not mask the verus result.
///
/// This mirrors the L2 (kani) driver's discipline, which already runs in a
/// per-run scratch crate removed wholesale via `kani.rs::run_kani`'s
/// `remove_dir_all(&crate_dir)`, so the kani path does not share this leak. The
/// shared cause's class ("an external-tool invocation must run in a scratch dir
/// removed wholesale, even on error") is now uniform across both rungs.
pub(crate) struct ScratchDir {
    pub(crate) path: PathBuf,
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        // Best-effort wholesale removal: the `.rs` source, verus's compiled
        // binary, and any other artifact go together. A failure here never fails
        // the verdict (R-CODE-2) — degrade to "left on disk", never to a panic.
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Write the lowered source to a `.rs` file with a valid-crate-name stem (REQ-2)
/// inside a per-run scratch directory, spawn verus there (`current_dir` is the
/// scratch dir, so verus's compiled-binary sibling lands inside it), parse
/// the result, and remove the scratch dir wholesale on every exit path
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
    // the source + verus's compiled binary + everything wholesale (blocker #53).
    drop(scratch);

    result
}

/// Spawn verus and parse its output. Split from `run_verus` so the scratch dir is
/// always cleaned up regardless of outcome. `cwd` is the per-run scratch
/// directory (blocker #53): verus's working-directory artifacts — most notably
/// the ~4.3M compiled-binary sibling — land there, so the caller's `ScratchDir`
/// guard removes them wholesale.
///
/// #11: always passes `--profile` + the pinned `--rlimit <rlimit>`. `--profile`
/// emits the Z3 instantiation report on STDERR only when the rlimit is exceeded
/// (the timeout discriminator); a clean proof / a fast counterexample emits no
/// report, so its presence is the timeout signal that
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

/// Build a unique per-run scratch directory path for one verus invocation
/// (blocker #53). Uniqueness uses the process id + a monotonic counter, not
/// wall-clock, so the path varies between concurrent runs without violating
/// R-CODE-5 (determinism is a property of the certificate, not the scratch path;
/// §check.md REQ-2 "determinism is in the input, not the path"). The directory
/// (not a bare `.rs` file) is what gets removed wholesale, taking the `.rs`
/// source and verus's compiled-binary sibling with it.
pub(crate) fn unique_scratch_dir(stem: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    std::env::temp_dir().join(format!("forge_{stem}_{pid}_{n}"))
}

/// Classify one verus run three ways deterministically (#11;
/// `.design/forge/solver-profiles.md` REQ-5). The JSON `verification-results`
/// summary on stdout cannot tell a timeout from a counterexample (both report
/// `success: false, errors: 1` — OQ-1), so the discriminator is the presence of
/// a `--profile` Z3 instantiation report on STDERR (verus emits it only on an
/// rlimit-hit):
///
/// - (a) proved (`success && errors == 0`) → [`VerusOutcome::Proved`] → `Level::L3`.
/// - (c) error with a profile report present → [`VerusOutcome::Timeout`]: parse
///   the `SolverProfile`, attach it (the cert is `Level::L0` + `VerusTimeout`).
/// - (b) error without a profile → [`VerusOutcome::Counterexample`] (the existing
///   #5 witness path). This also absorbs the documented incompleteness-unknown
///   edge (an `unknown` returned fast without exhausting the rlimit → no profile →
///   the failure path), so a timeout is never silently reported as success
///   (R-CODE-4 — degrade/report, do not treat as success).
///
/// The classification is deterministic; the profile content (instantiation
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

    // A VIR / internal verus error is not a verification failure. It is an
    // environment/tooling failure (never swallowed, never treated as success).
    if summary.encountered_vir_error {
        return Err(ForgeError::VerusOutput {
            detail: format!(
                "verus reported an internal (VIR) error; stderr: {}",
                first_lines(stderr, 12)
            ),
        });
    }

    // (a) proved.
    if summary.success && summary.errors == 0 {
        return Ok(VerusOutcome::Proved {
            verified: summary.verified,
        });
    }

    // (c) timeout: an error run whose stderr carries a `--profile` instantiation
    // report. `--profile` reports only on an rlimit-hit, so the report's presence
    // is the timeout discriminator (REQ-5 / the doc's Architecture).
    if let Some(profile) = profile::parse_profile(stderr) {
        let detail = format!(
            "verus exhausted its SMT resource budget (rlimit) before proving this item; \
             {} total quantifier instantiations observed (see solver_profile / suggested_move)",
            profile.total_instantiations
        );
        return Ok(VerusOutcome::Timeout { profile, detail });
    }

    // (b) counterexample / the failure path (no profile report). Parse stderr for
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
/// no `-->` is still recorded (do not over-fit to one format).
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

/// Assemble one item's [`Certificate`] from that item's own verus result (REQ-1
/// final stage; §5.3 per-item). `verus` is the result of verifying `item`'s
/// isolated sub-program (`item_subprogram`), so its outcome reflects only this
/// item — never a sibling's. `item` is the item name; `effects` is the item's
/// `fx` row (`spec fn`s are pure — they carry no `fx`).
///
/// #11 — the three-way outcome maps to three certificate shapes:
/// - [`VerusOutcome::Proved`] → `Level::L3`, one discharged summary obligation,
///   no profile.
/// - [`VerusOutcome::Timeout`] → `Certificate::timeout` (`Level::L0` +
///   `RejectReason { cause: "VerusTimeout" }` + the `SolverProfile` + the
///   profile-derived `suggested_move`), distinct from a counterexample.
/// - [`VerusOutcome::Counterexample`] → `Level::L0` with the per-obligation
///   witnesses (the existing #5 path), no profile.
fn assemble_certificate(item: &Item, verus: &VerusResult) -> Certificate {
    let effects = match item {
        Item::Fn(f) => effects_of(&f.contract.fx),
        // `spec fn`s have no `fx` row (§4.2) — they are pure by construction.
        Item::SpecFn(_) => vec!["pure".to_string()],
        // Basis Stage 1a (`.design/basis/01-adts.md`): a `struct`/`enum` type
        // declares no `fx` row — its neutral effect projection is `pure` (the
        // same empty-effect value as a `spec fn`). Dead-in-1a: an ADT item dies
        // at the validator before a certificate is ever assembled for it.
        Item::Struct(_) | Item::Enum(_) => vec!["pure".to_string()],
        // Forge-tier item (stage1-forge-tier.md REQ-3): no v1 cert consumer yet
        // (increments 2b-3); declares no `fx` row → the same neutral `pure`
        // projection as a `spec fn`/ADT decl, mirroring the inert ADT-decl arm.
        Item::Forge(_) => vec!["pure".to_string()],
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

/// Drive the #10 automatic degrade ladder for one `fn` whose L3 verus run is in
/// `outcome` (`.design/forge/degrade-ladder.md` REQ-1/REQ-2/REQ-3). `l3_cert` is
/// the cert `assemble_certificate` already built from `outcome` (the L3 cert on
/// `Proved`, the `VerusTimeout` cert on `Timeout`, the counterexample cert on
/// `Counterexample`). Returns the achieved-level cert:
///
/// - `Proved` → the L3 cert unchanged (no degrade — the common path).
/// - `Counterexample` → the counterexample cert unchanged; a hard fail, no L2/L1
///   rung is attempted (REQ-2 anti-cheat: the ladder short-circuits falsity).
/// - `Timeout` → run the ladder: lower + kani the same item (L2); on a verified
///   bound certify L2 + lowered-assurance; on an under-bound drop to L1 (a recorded
///   `Level::L1` cert, OQ-3 (b) — `lower_l1`'s runtime-check emission stays a
///   build-time concern); on an L2 counterexample a hard fail (REQ-2, 2nd rung).
///
/// An environment failure on a lower rung (kani absent, unparseable output,
/// lowering failure) propagates as a `ForgeError` (REQ-8), never a silent degrade.
/// The L2/L1 closures are lazy — they run only on the timeout edge, so the common
/// `Proved` path never spawns kani.
fn ladder_for_timeout(
    f: &thermite_syntax::FnItem,
    sub: &Program,
    outcome: &VerusOutcome,
    l3_cert: Certificate,
    obligation: &crate::obligation::Obligation,
    evidence_key: crate::engine::CacheKey,
) -> Result<Certificate, ForgeError> {
    // proof-backends #204: route the per-item L3 certification discharge through
    // the backend-neutral Verus engine (`engine::VerusEngine`,
    // `.design/verified/proof-backends.md` REQ-2/REQ-3/REQ-3.1). The engine maps
    // the shipped `VerusOutcome` to a backend-neutral `engine::Verdict` (the
    // three-way `classify_verus_outcome` lifted, with the REQ-3.1 fast-unknown
    // remap), then `verdict_ladder_action` maps the verdict to the shipped
    // `degrade::L3Verdict` the ladder consumes (REQ-3: Proven→certify,
    // Unknown→degrade, Refuted→hard-fail). This is byte-identical to the prior
    // direct `VerusOutcome → L3Verdict` map except the REQ-3.1 delta: a witness-less
    // `Counterexample` (the fast-`unknown` edge — no parsed `--> span`) now maps
    // to `Unknown` → degrade (was a hard fail). A witnessed countermodel still
    // maps to `Refuted` → hard fail. The conformance corpus is unperturbed: every
    // corpus item proves at L3, so no corpus item produces a `Counterexample` of
    // either kind.
    use crate::engine::Engine as _;
    // REQ-8: select the first engine in the default ordering (Verus first; the Lean
    // rungs are increment (ii)). The ordering hook is wired here with the single
    // Verus rung.
    let engine = match crate::engine::default_engines().first() {
        Some(crate::engine::EngineName::Verus) | None => crate::engine::VerusEngine,
        // Increment (i) ships only the Verus engine; any other ordering head is a
        // future rung not yet built — fall back to Verus (the default rung) rather
        // than panic (R-APG-1). When the Lean engine lands (increment (ii)) this
        // arm dispatches to it.
        Some(_) => crate::engine::VerusEngine,
    };
    // REQ-2(a) fragment gate: the engine must admit the obligation's class before
    // it attempts a discharge. The Verus engine admits the whole frozen subset
    // (incl. RegistryTermination, REQ-1.2(a)), so this is always `true` today; it
    // is the REQ-3-compliant seam a narrower future engine keys on (an unadmitted
    // obligation is an `Unknown`, never a witness-less `Refuted`).
    let verdict = if engine.fragment().admits(obligation) {
        engine.verdict_of(outcome, evidence_key)
    } else {
        engine.discharge(obligation, &CovenantRecord::none())
    };
    // REQ-2(c) trust profile: the named base this engine would add on a `Proven`
    // (the §1 enumerable trusted base). Folded into the degrade-reason detail on an
    // `Unknown` so the auditor sees which engine's base was attempted before the
    // degrade (oracle-free — the cert's degrade reason is not in the cert oracle;
    // per-obligation attribution as a cert field is REQ-4, increment (iii)).
    let trust = engine.trust_profile();
    // REQ-2(c): fold the engine's named trust base into a fast-`unknown` degrade
    // reason so the auditor sees which engine's base was attempted before the
    // degrade. This enriches only the REQ-3.1 incompleteness-`unknown` path (a
    // genuine `Timeout` keeps its shipped profile-derived reject below); it is
    // oracle-free (the degrade reason is not in the cert oracle). Per-obligation
    // attribution as a cert field is REQ-4, increment (iii).
    let verdict = match verdict {
        crate::engine::Verdict::Unknown(crate::engine::Reason::IncompleteUnknown(d)) => {
            crate::engine::Verdict::Unknown(crate::engine::Reason::IncompleteUnknown(format!(
                "{d} [engine {}, trust base: {}]",
                engine.name().tag(),
                trust.items.join(", ")
            )))
        }
        other => other,
    };
    // The degrade `reason` carried onto a lower rung (REQ-4) on the `Unknown`
    // (timeout / fast-unknown) edge prefers the assembled `l3_cert`'s reject (the
    // `Certificate::timeout` `RejectReason`) so the existing `VerusTimeout` reason
    // text is preserved byte-identically for the genuine-timeout case.
    let timeout_reason = l3_cert.reject.clone();
    let proved_cert = l3_cert.clone();
    let cx_cert = l3_cert;
    let l3 = crate::engine::verdict_ladder_action(&verdict, obligation.role, proved_cert, cx_cert);
    // Preserve the shipped `VerusTimeout` reason text on a genuine timeout (REQ-4
    // byte-identity): `verdict_ladder_action` synthesizes a generic reason, but the
    // assembled `Certificate::timeout` reject carries the profile-derived detail —
    // splice it back so the degrade reason on a timeout is unchanged.
    let l3 = match (l3, timeout_reason) {
        (crate::degrade::L3Verdict::Timeout { reason: generic }, Some(reject))
            if reject.cause == "VerusTimeout" =>
        {
            // A genuine timeout: keep the shipped profile-derived reject text.
            let _ = generic;
            crate::degrade::L3Verdict::Timeout { reason: reject }
        }
        (other, _) => other,
    };

    let effects = effects_of(&f.contract.fx);
    let l1_effects = effects.clone();
    let fname = f.name.clone();

    crate::degrade::run_ladder(
        l3,
        // The L2 rung (lazy): lower the same item to a kani harness, run the real
        // kani binary, classify (the OQ-2 split). An environment failure → Err.
        || {
            let harness = thermite_lower::lower_l2(sub).map_err(ForgeError::Lower)?;
            let bound = thermite_lower::bound_string(sub);
            let l2 = crate::kani::run_kani(&harness, &fname, &bound)?;
            let verdict = crate::kani::classify_l2_outcome(&l2);
            let cert = crate::kani::assemble_l2_certificate(&fname, effects, &l2);
            Ok(crate::degrade::L2Attempt { verdict, cert })
        },
        // The L1 fallback rung (lazy): record the achieved `Level::L1` (OQ-3 (b)).
        // The contract's runtime-check emission is `thermite_lower::lower_l1`'s
        // build-time job, not the verdict-aggregator's — exactly the
        // `Certificate::slag_l1` precedent (records L1 without running a prover).
        // `lower_l1` is invoked here only to confirm the contract lowers to runtime
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

/// Score the frozen mutant set of `f` against its own (unchanged) contract (#12
/// §7 step 4; `.design/forge/mutation-scoring.md` REQ-3/REQ-4/REQ-5/REQ-7).
/// Called from the per-item L3 path only after `f`'s real body proved L3 (the
/// caller gates on `cert.level == L3 && reject.is_none()`).
///
/// For each mutant (`mutation::generate`, the frozen + ordered + capped set):
/// 1. weave it into the same per-item sub-program shape ([`item_subprogram`]) and
///    lower via the existing `thermite_lower::lower`. A mutant that fails to lower
///    is dropped from the denominator (not scored — OQ-5), never an `Err` that
///    fails the gate.
/// 2. content-address the lowered mutant via the same proof cache (#8;
///    `cache::cache_key`/`load`/`store`) — a hit serves the stored verdict without
///    spawning verus, so a re-`forge check` re-scores cheaply (REQ-7). The cache
///    is consulted only at the canonical config (`use_cache`); a non-default
///    rlimit / floor run bypasses it (the caller's invariant).
/// 3. run the existing `run_verus` on a miss and classify (REQ-4): a `Proved`
///    mutant survived (the contract is too weak); a `Counterexample` / `Timeout`
///    mutant is killed. An environment / VIR failure surfaces a `ForgeError`
///    (R-CODE-4), never a silent kill or survive.
///
/// Returns the [`mutation::MutationScore`] (`killed`/`scored` + the first
/// surviving mutant's description). Deterministic (REQ-8): the mutant list is a
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
    adt_deps: &[Item],
    seed: u64,
    rlimit: f64,
    verus_version: &str,
    cache_dir: &Path,
    use_cache: bool,
) -> Result<crate::mutation::MutationScore, ForgeError> {
    let mutants = crate::mutation::generate(f, seed, adt_deps);
    let mut killed = 0usize;
    let mut scored = 0usize;
    let mut equivalent = 0usize;
    let mut survivor: Option<String> = None;

    for mutant in mutants {
        // Keep the mutant's `FnItem` so a survivor can be equivalence-checked
        // against the real body (#101); clone what the obligation needs before
        // moving the item into the sub-program.
        let mutant_item = mutant.item.clone();
        let item = Item::Fn(mutant.item);
        // Weave the same §9 composition deps as the original `f` (#52) and the
        // same #68 ADT decls: a mutant body still references the original's
        // boundary/regular callees + ADT types, so they must resolve in the
        // mutant's sub-program too (else every mutant fails to lower and the score
        // is the 0/0 backstop — a spurious `WeakContract` reject of an ADT fn).
        let sub = item_subprogram(&item, spec_items, fn_deps, adt_deps);
        // OQ-5: a mutant that fails to lower (structurally degenerate) is dropped
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

        if crate::mutation::classify_mutant(proved) == crate::mutation::MutantOutcome::Killed {
            // A killed mutant is distinguished by definition — never equivalence-
            // checked (`.design/forge/equivalent-mutants.md` scope: out). It counts.
            scored += 1;
            killed += 1;
            continue;
        }

        // The mutant survived (verus proved it against the unchanged contract).
        // Issue the per-survivor equivalence query (#101 REQ-1; #269 REQ-7): is the
        // mutant body observably equal to the real body under `f`'s `req`, for all
        // inputs (modulo callee contracts when the body is call-bearing — the same
        // `fn_deps` closure woven above)? A verified query is a proof of
        // equivalence → the survivor is a true equivalent mutant (not contract
        // weakness) and drops from the denominator (REQ-2). A counterexample /
        // timeout / weak-callee-unprovable harness leaves it a counted survivor
        // (REQ-3/REQ-8); an un-renderable obligation also leaves it counted but
        // records the structured reason (REQ-9 — never a silent exclusion, never a
        // silent collapse). Exclusion fires on `Proved` alone.
        let equiv = equivalence_proves_equal(
            f,
            mutant_item.body.as_ref(),
            fn_deps,
            seed,
            rlimit,
            verus_version,
            cache_dir,
            use_cache,
        )?;
        match equiv {
            EquivOutcome::Proved => {
                // REQ-2/REQ-4: excluded from both the survivor set and `scored`.
                equivalent += 1;
                continue;
            }
            EquivOutcome::NotProved => {
                // A distinguishing / unprovable survivor (REQ-3/REQ-8):
                // stays counted; the representative strengthening prompt if first.
                scored += 1;
                if survivor.is_none() {
                    survivor = Some(mutant.desc);
                }
            }
            EquivOutcome::Unsupported(reason) => {
                // REQ-9 (R-HONEST-3): the probe could not ask the question. The
                // survivor stays counted (no proof) and the structured reason is
                // carried to the survivor transparency surface, so an operator
                // distinguishes "proved distinguishing" from "the probe could not
                // ask" — never a silent collapse into the distinguishing bucket.
                scored += 1;
                if survivor.is_none() {
                    survivor = Some(format!(
                        "{} (equivalence probe Unsupported — survivor COUNTED, not \
                         excluded: {reason})",
                        mutant.desc
                    ));
                }
            }
        }
    }

    Ok(crate::mutation::MutationScore {
        killed,
        scored,
        equivalent,
        survivor,
    })
}

/// The §7 equivalent-mutant equivalence query for one survivor
/// (`.design/forge/equivalent-mutants.md` REQ-1, #101): lower the equivalence
/// obligation (the `thermite_lower::lower_equivalence_obligation` seam — `under
/// req, mutant_body == real_body` for all inputs), content-address it through the
/// same #8 proof cache, and run the existing `run_verus`. Returns `Ok(true)` iff
/// verus proved the obligation (`0 errors` → the mutant is observably equivalent
/// → REQ-2 exclude); `Ok(false)` on a counterexample, a timeout, or an
/// un-renderable obligation (a non-scalar / non-forced-output body the seam
/// returns `Unsupported` for — OQ-1, the natural sound-but-incomplete fallback:
/// no proof ⇒ the survivor stays counted, REQ-3). `Err` only on an environment /
/// VIR failure (R-CODE-4 — never a silent equivalence).
///
/// This is a new caller of the existing prover path, not a new prover (REQ-1):
/// it reuses `lower_equivalence_obligation` (which reuses the L3 exec
/// coercions — no hand-emitted Verus, R-CHAR-3), `cache::cache_key`/`load`/
/// `store` (REQ-6, deterministic content-addressed verdict), and `run_verus`.
///
/// Call-free bodies (the shipped #101 corpus): the obligation is self-contained
/// (the seam emits a whole `use vstd::prelude::*; verus! { .. } fn main() {}` unit
/// over only scalar spec fns + a proof fn), so `fn_deps` is empty and no §9
/// composition deps are woven.
///
/// Call-bearing bodies (`.design/forge/equivalent-mutants.md` REQ-7, #269): the
/// same `fn_deps` closure `mutation_score` weaves into each mutant's
/// `item_subprogram` (the caller's `reachable_fn_deps`) is threaded into the seam,
/// which emits an exec-position proof harness with the closure woven (boundary
/// callees as external_body signatures, regular callees as full defs) — the
/// equivalence query then runs with the same call-site semantics the caller's own
/// L3 proof used (modulo callee contracts, §9). A weak callee contract that
/// cannot pin `real == mutant` → `eq` unprovable → `Ok(false)` → counted survivor
/// (REQ-8); an out-of-scope shape → `Unsupported` → `Ok(false)` + the reason is
/// surfaced to the score's transparency note (REQ-9 — never a silent exclusion).
#[allow(
    clippy::too_many_arguments,
    reason = "the L3-path seams (the fn_deps closure, seed, rlimit, verus version, \
    cache dir + enable) are the SAME verdict-determining inputs `mutation_score` \
    threads and compose the obligation cache key; bundling them would obscure the \
    content-addressing this query reuses"
)]
fn equivalence_proves_equal(
    f: &thermite_syntax::FnItem,
    mutant_body: Option<&thermite_syntax::ast::Block>,
    fn_deps: &[Item],
    seed: u64,
    rlimit: f64,
    verus_version: &str,
    cache_dir: &Path,
    use_cache: bool,
) -> Result<EquivOutcome, ForgeError> {
    let Some(body) = mutant_body else {
        // A bodyless (boundary) mutant cannot arise (mutation never scores a
        // boundary fn), but treat a missing body as no-proof (stays counted).
        return Ok(EquivOutcome::NotProved);
    };
    let obligation = match thermite_lower::lower_equivalence_obligation(f, body, fn_deps) {
        Ok(s) => s,
        // REQ-9: an un-renderable obligation (non-scalar / out-of-scope shape) is
        // no proof — the survivor stays counted — and the structured reason is
        // carried (never a silent collapse into the proved-distinguishing bucket).
        Err(e) => return Ok(EquivOutcome::Unsupported(e.to_string())),
    };
    // The obligation is a complete Verus program (the seam emits the frame); run
    // it as a single-item program for the `run_verus` scratch-dir/label machinery.
    let label_program = Program {
        items: vec![Item::Fn(f.clone())],
    };
    let key = cache::cache_key(&obligation, seed, verus_version, THERMITE_VERSION);
    let proved = if use_cache {
        if let Some(stored) = cache::load(cache_dir, &key) {
            // A cached cert: the equivalence query proved iff the stored cert is
            // L3 with no reject (the same `mutant_cert_is_survivor` polarity the
            // mutant kill-check caches — a `Proved` obligation is "survivor"-true).
            mutant_cert_is_survivor(&stored)
        } else {
            let verus = run_verus(&label_program, &obligation, seed, rlimit)?;
            let proved = mutant_outcome_is_survivor(&verus.outcome);
            // Cache the equivalence verdict (REQ-6 determinism): assemble + store
            // the same cert shape the mutant kill-check stores, keyed on the
            // obligation source so a re-`forge check` serves it without re-spawning
            // verus.
            let cert = assemble_certificate(&Item::Fn(f.clone()), &verus);
            let _ = cache::store(cache_dir, &key, &cert);
            proved
        }
    } else {
        let verus = run_verus(&label_program, &obligation, seed, rlimit)?;
        mutant_outcome_is_survivor(&verus.outcome)
    };
    // The exclusion fires only on a verus-proved `ensures` (REQ-2/REQ-3/REQ-8):
    // a `Proved` obligation/harness is a true equivalent → `Proved`; a
    // counterexample/timeout (including a weak-callee-unprovable harness) is
    // NotProved → the survivor stays counted.
    Ok(if proved {
        EquivOutcome::Proved
    } else {
        EquivOutcome::NotProved
    })
}

/// The outcome of a per-survivor equivalence query (`.design/forge/equivalent-
/// mutants.md` REQ-2/REQ-3/REQ-8/REQ-9). Exclusion fires on `Proved` alone — a
/// verus proof of the `ensures` (the call-free obligation's `mut == real` or the
/// call-bearing harness's `eq`). Every other outcome keeps the survivor counted;
/// the variants distinguish "the prover found a distinguishing input / timed out"
/// (`NotProved`) from "the probe could not even ask the question" (`Unsupported`,
/// carrying the structured reason — REQ-9, so an operator can tell a genuine
/// contract weakness from an out-of-scope obligation shape, R-HONEST-3).
#[derive(Debug, Clone)]
enum EquivOutcome {
    /// Verus proved equivalence (modulo callee contracts for a call-bearing body)
    /// → the survivor is a true equivalent mutant → dropped from the denominator.
    Proved,
    /// Verus did not prove it (a distinguishing counterexample, a timeout, or a
    /// weak-callee-unprovable harness) → the survivor stays counted (REQ-3/REQ-8).
    NotProved,
    /// The obligation could not be rendered (an out-of-scope body shape) → the
    /// survivor stays counted and the structured reason is recorded (REQ-9).
    Unsupported(String),
}

/// Run the #14 §7 step-5 strengthening probe for `f`
/// (`.design/forge/strengthening-probes.md` REQ-2/REQ-3/REQ-4). Called from the
/// per-item L3 path only after `f`'s real body proved L3 and its mutant set met
/// the floor (the caller gates on `cert.level == L3 && reject.is_none()` + a
/// produced `MutationScore`, REQ-5). It delegates the candidate template +
/// verify/filter pipeline to `strengthen::probe`, threading two verify closures
/// that reuse the existing verus driver:
///
/// - `verify_body` — weave the candidate `ens` into a copy of `f` (body
///   unchanged, `strengthen::candidate_fn`), build the same per-item sub-program
///   (`item_subprogram`), lower (`thermite_lower::lower`), content-address (the #8
///   cache), and `run_verus`. Returns `Ok(true)` iff verus proved the candidate
///   against the real body (the §7 "proves with no body change"); `Ok(false)` on a
///   non-`Proved` outcome or an un-lowerable woven fn (parallel to #12's drop), and
///   `Err` on an environment failure (R-CODE-4).
/// - `verify_survivor` — verify the candidate `ens` against the survivor body (the
///   #12 mutant whose description is the recorded survivor). The survivor body
///   comes from the same frozen mutator (`mutation::generate`), so the kill witness
///   is the design's grounded `result == a + b` against `{ return 0; }`. Returns
///   `Ok(true)` iff verus proved the candidate against the survivor body (not
///   killed); `Ok(false)` when it did not (killed — the strictly-stronger witness).
///
/// Returns the ordered, deterministic list of adoptable [`strengthen::Suggestion`]s
/// (possibly empty, an absence the cert records, REQ-4). The probe introduces no new
/// prover invocation path; it is a new caller of `run_verus` (REQ-2 / the doc's "the
/// probe introduces no new prover invocation path, only a new caller of the existing one").
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
    adt_deps: &[Item],
    score: &crate::mutation::MutationScore,
    seed: u64,
    rlimit: f64,
    verus_version: &str,
    cache_dir: &Path,
    use_cache: bool,
) -> Result<Vec<crate::strengthen::Suggestion>, ForgeError> {
    // The survivor body the kill witness verifies against: the #12 mutant whose
    // description matches the recorded survivor (the same frozen mutator). Resolved
    // once; reused for every survivor-linked candidate.
    let survivor_body: Option<thermite_syntax::FnItem> = score.survivor.as_ref().and_then(|desc| {
        crate::mutation::generate(f, seed, adt_deps)
            .into_iter()
            .find(|m| &m.desc == desc)
            .map(|m| m.item)
    });

    // A single content-addressed verify of a woven `fn` (the candidate `ens` over
    // a given body): lower the per-item sub-program, consult the #8 cache, else
    // `run_verus` + store. Returns whether verus proved it (the cert is L3 with no
    // reject). An un-lowerable woven fn is `Ok(false)` (parallel to #12's drop).
    let verify_woven = |woven: &thermite_syntax::FnItem| -> Result<bool, ForgeError> {
        let item = Item::Fn(woven.clone());
        // The candidate weaves the same §9 composition deps as `f` (#52) and the
        // same #68 ADT decls so a boundary/regular callee in `f`'s body + every
        // referenced ADT type resolves in the candidate too.
        let sub = item_subprogram(&item, spec_items, fn_deps, adt_deps);
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
        // verify_body: the candidate `ens` over the real body.
        |woven| verify_woven(woven),
        // verify_survivor: the candidate `ens` over the survivor body. If the
        // survivor body could not be resolved (no recorded survivor), the candidate
        // is treated as proving (not killed) so it is not credited a kill it cannot
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

/// `true` iff a verus outcome on a mutant body is a survivor (REQ-4): verus
/// proved the wrong body (`VerusOutcome::Proved`). A counterexample
/// or a timeout is killed (OQ-4 — an un-proved mutant is not a survivor).
fn mutant_outcome_is_survivor(outcome: &VerusOutcome) -> bool {
    matches!(outcome, VerusOutcome::Proved { .. })
}

/// `true` iff a cached mutant cert is a survivor (REQ-4). The stored cert is a
/// full item cert: a `Level::L3` with no reject means verus proved the mutant (a
/// survivor); anything else (a counterexample-L0, a timeout reject) is killed.
fn mutant_cert_is_survivor(cert: &Certificate) -> bool {
    cert.level == Level::L3 && cert.reject.is_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::ObligationStatus;

    // REQ-6 / AC-10 (increment 2d, anti-Goodhart defense (b)): the L3 re-elaboration
    // mutation battery reuses the FROZEN mutation operator catalogue
    // `mutation::generate` — the catalogue is SHARED, not forked. This test pins that
    // contract: the mutant set the re-elaboration seam (`reelaboration_mutants`, the
    // set `lean_mutation_score` re-elaborates per mutant) scores is byte-for-byte
    // `mutation::generate`'s — same families, same order, same descriptions, same
    // `MUTANT_CAP` bound. A future fork of the operator set into a second catalogue
    // would break this assertion.
    #[test]
    fn reelaboration_mutation_shares_the_frozen_catalogue_not_forked() {
        let src = "\
fn to_1based(x: u32) -> u32
  req x < 1000
  ens result == x + 1
  fx pure
{ x + 1 }
";
        let parsed = thermite_syntax::parse(src);
        assert!(parsed.is_clean(), "fixture must parse: {:?}", parsed.errors);
        let f = parsed
            .program
            .items
            .iter()
            .find_map(|i| match i {
                Item::Fn(f) if f.name == "to_1based" => Some(f.clone()),
                _ => None,
            })
            .expect("fixture has fn to_1based");

        // The re-elaboration battery's catalogue IS `mutation::generate` (the SHARED
        // frozen set), not a fork.
        let shared = reelaboration_mutants(&f, &parsed.program.items);
        let frozen = crate::mutation::generate(&f, 0, &parsed.program.items);
        assert!(
            !shared.is_empty(),
            "the fixture must produce mutants to score"
        );
        assert_eq!(
            shared.len(),
            frozen.len(),
            "the re-elaboration catalogue is the SAME size as mutation::generate (shared, not forked)"
        );
        for (a, b) in shared.iter().zip(frozen.iter()) {
            assert_eq!(
                a.desc, b.desc,
                "the re-elaboration battery scores the SAME mutant (same operator family, \
                 same deterministic order) as mutation::generate — not a forked catalogue"
            );
        }
        // The `MUTANT_CAP` = 64 budget is honored at the shared catalogue source (the
        // ≤64 re-typecheck bound the REQ-6a/b perf note depends on).
        assert!(
            shared.len() <= crate::mutation::MUTANT_CAP,
            "the shared catalogue is bounded by MUTANT_CAP ({} > {})",
            shared.len(),
            crate::mutation::MUTANT_CAP
        );
    }

    // proof-backends #204 / REQ-1.2 / #226 — the closure mirror: a spec-fn called
    // only from a `dec` measure position reaches the per-item Obligation env's
    // `spec_defs` (the corrected full-expression-position closure walks the dec
    // measures, not body-only). A body-only closure (the shipped
    // `reachable_spec_fn_deps`) would drop it, bottoming it to the `intVal`
    // Int-bottom and faking a descent (`lean/Thermite/PinDecMeasure.lean`). Expected
    // from REQ-1.2's full-expression-position principle (R-CHAR-3).
    #[test]
    fn dec_position_spec_fn_reaches_obligation_env() {
        // `measured` calls the spec fn `tree_size` only from its own `dec` measure
        // (`dec tree_size(xs)`), never from its body — the #226 measure-position
        // case. The full closure must still reach `tree_size`.
        let src = "\
spec fn tree_size(xs: &[u32]) -> u64 dec xs.len() { xs.len() as u64 }
fn measured(xs: &[u32]) -> u64
  req xs.len() <= 10
  ens result == xs.len() as u64
  fx pure
  dec tree_size(xs)
{ xs.len() as u64 }
";
        let parsed = thermite_syntax::parse(src);
        assert!(parsed.is_clean(), "fixture must parse: {:?}", parsed.errors);
        let mut measured: Option<&thermite_syntax::FnItem> = None;
        for i in &parsed.program.items {
            if let Item::Fn(f) = i {
                if f.name == "measured" {
                    measured = Some(f);
                }
            }
        }
        assert!(measured.is_some(), "measured present");
        if let Some(f) = measured {
            // The corrected closure reaches the dec-position callee.
            let full = reachable_spec_fn_names_full(&parsed.program, f);
            assert!(
                full.contains(&"tree_size".to_string()),
                "the full-expression-position closure must reach a DEC-position \
                 spec-fn dep (REQ-1.2/#226); got {full:?}"
            );
            // The minted Obligation carries it in `env.spec_defs` (the artifact
            // reifies the closure), and the item gets a registry-termination
            // obligation.
            let obs = mint_item_obligations(&parsed.program, &Item::Fn(f.clone()));
            assert!(
                obs.contract
                    .env
                    .spec_defs
                    .contains(&"tree_size".to_string()),
                "the Obligation env must carry the dec-position spec-fn dep (REQ-1)"
            );
            assert!(
                obs.registry_termination.is_some(),
                "a non-empty closure mints REGISTRY-TERMINATION (REQ-1.2)"
            );
        }
    }

    // proof-backends #204 / REQ-1.2: an item with no spec-fn dependency gets no
    // registry-termination obligation (the class is assigned iff the closure is
    // non-empty). Expected from REQ-1.2's assignment condition (R-CHAR-3).
    #[test]
    fn spec_fn_free_item_has_no_registry_termination() {
        let src = "fn add(x: u64, y: u64) -> u64 req x < 100 ens result == x + y fx pure { x + y }";
        let parsed = thermite_syntax::parse(src);
        assert!(parsed.is_clean(), "fixture must parse: {:?}", parsed.errors);
        let mut add: Option<&Item> = None;
        for i in &parsed.program.items {
            if i.name() == "add" {
                add = Some(i);
            }
        }
        assert!(add.is_some(), "add present");
        if let Some(item) = add {
            let obs = mint_item_obligations(&parsed.program, item);
            assert!(obs.contract.env.spec_defs.is_empty());
            assert!(
                obs.registry_termination.is_none(),
                "a spec-fn-free item has NO registry-termination obligation (REQ-1.2)"
            );
        }
    }

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

    // AC-6 + REQ-4: a parseable failure summary + stderr witness → a reported
    // non-L3 cert (not an Err) carrying the failed obligation with its source
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
        // #11: a failure summary with no profile report on stderr classifies as a
        // counterexample (the failure path), not a timeout (AC-3, AC-4).
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

    // REQ-3 / AC-6: exit != 0 with unparseable output → ForgeError::VerusOutput
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

    // #11 / AC-3 / AC-4: a failure summary whose STDERR carries a `--profile` Z3
    // instantiation report classifies as a timeout (not a counterexample) and the
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
        // counterexample, OQ-1); the profile report on stderr is the discriminator.
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

    // #8 proof-cache AC-3 (locality) + AC-4 (determinism), exercised over the
    // real `item_subprogram` → `thermite_lower::lower` → `cache::cache_key`
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
            // #68: no ADT in this fixture (`g`/`f` are scalar), so the ADT-decl
            // weave is empty and the key is unchanged.
            let mut referrers: Vec<&Item> = vec![item];
            referrers.extend(fn_deps.iter());
            let adt_deps = reachable_adt_deps(&parsed.program, &referrers);
            let sub = item_subprogram(item, &spec_items, &fn_deps, &adt_deps);
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
        // Determinism (AC-4): the same item over identical input yields the same key.
        assert_eq!(
            key_of(src_v1, "g"),
            g_v1,
            "key is deterministic for unchanged input"
        );
        // Locality (AC-3): editing `f` does not change `g`'s key.
        assert_eq!(
            g_v1, g_v2,
            "g's key is invariant under an f-only edit (locality)"
        );
        // Invalidation (AC-2): editing `f` does change `f`'s key.
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
