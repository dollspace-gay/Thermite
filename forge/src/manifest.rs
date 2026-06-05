//! `forge/src/manifest.rs` — the certificate schema (`thermite-design.md` §5.1,
//! Appendix A). The `Certificate` is the deliverable's trust statement (§6): a
//! STABLE, versioned data contract that `forge check` emits. This module owns the
//! schema and its `serde_json` (de)serialization; it performs NO I/O and runs NO
//! verification — `check.rs` (`.design/forge/check.md`) produces the values.
//!
//! Governing design: `.design/forge/certificate-manifest.md`.
//!
//! The schema is fixed NOW at its full Appendix A shape; the PRODUCERS arrive
//! over several issues (the "two-speed schema"). #5 fills `item`, `level`,
//! `effects`, `slag`, and `obligations` with real derived values; the
//! `contract_quality.*` battery fields are FORWARD-DECLARED (honest #5 values,
//! NOT asserted against the golden cert, made live by #6/#12/#13) and
//! `suggested_move` is a reserved `None`. `solver_time_ms` is present but
//! non-deterministic and excluded from the cert-oracle comparison.
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (stable schema, Appendix A) | SHIPPED | `struct Certificate { item, level, solver_time_ms, contract_quality, effects, slag, obligations, suggested_move }` mirrors Appendix A field order; consumed by `check::check_file` in `check.rs`. |
//! | REQ-2 (fields #5 produces now) | SHIPPED | `Certificate::new` sets `item`/`level`/`effects`/`slag`/`obligations` from real pipeline data; `effects_of` maps `EffectRow` to the `["pure"]` row; called by `check::assemble_certificate`. |
//! | REQ-3 (forward-declared fields) | SHIPPED | `ContractQuality::forward_declared` returns honest non-asserted #5 values (`tautology=false`, `vacuous_precondition=false`, `mutants_killed="0/0"`, `survivor=None`); `oracle_subset` excludes them. #6 graduates the two §7.1 bools to LIVE `false` via `Certificate::graduate_triage_clean`, consumed by `check::check_file` on a triage-passing item; `mutants_killed`/`survivor` stay #12-forward-declared. |
//! | REQ-4 (`suggested_move` reserved) | SHIPPED | `Certificate::new` sets `suggested_move: None`; `SuggestedMove` is the reserved (currently un-constructed in production) slot type, serialized as `null`/omitted. |
//! | REQ-5 (per-obligation results) | SHIPPED | `struct ObligationResult { name, status, location, diagnostic }` + `enum ObligationStatus`; the `obligations` field; consumed by `check::assemble_certificate` + `cli::render_human`. |
//! | REQ-6 (`solver_time_ms` excluded) | SHIPPED | `solver_time_ms: u64` present (Appendix A); `Certificate::oracle_subset` omits it (and `contract_quality`), and `cli::render_human` labels it non-deterministic. |
//! | REQ-7 (serde_json serialization) | SHIPPED | `#[derive(Serialize, Deserialize)]`; `Level` serializes to `"L0".."L3"`; `cli::run_check` serializes via `serde_json::to_string_pretty`; deterministic field order from struct declaration order. |
//!
//! ## #6 additive schema (slag-triage, this iteration)
//!
//! | Field/symbol | Status | Evidence |
//! |---|---|---|
//! | `SlagMeta` (cert metadata) | SHIPPED | `struct SlagMeta { reason, owner, review }`; `Certificate.slag_meta: Option<SlagMeta>` (additive, `skip_serializing_if`); produced by `slag::validate`, set by `Certificate::slag_l1`, consumed by `check::check_file` for a valid `#[slag]` item (`slag.md` REQ-4, OQ-1 ratified). |
//! | `RejectReason` (verdict-in-cert) | SHIPPED | `struct RejectReason { cause, detail }`; `Certificate.reject: Option<RejectReason>` (additive); a triage / slag reject is `Certificate::rejected` (`Level::L0` + cause), consumed by `check::check_file` + `cli::run_check` (exit non-zero) (`vacuity-triage.md` REQ-5, OQ-1: verdict-in-cert NOT a `ForgeError`). |
//!
//! ## #8 additive schema (proof-cache provenance, this iteration)
//!
//! | Field/symbol | Status | Evidence |
//! |---|---|---|
//! | `cached: bool` (cache provenance) | SHIPPED | `Certificate.cached: bool` (additive, `#[serde(default)]` so the frozen golden `sum.cert.json` still deserializes); set by `Certificate::with_cached`, consumed by `check::check_file` (`true` on a HIT, `false` on a fresh verify) and `cache::store` (cleared before persisting). EXCLUDED from `oracle_subset` — a hit and a fresh verify compare oracle-EQUAL (`proof-cache.md` REQ-7/REQ-2). |
//!
//! ## #11 additive schema (solver-profile timeout slot, this iteration)
//!
//! | Field/symbol | Status | Evidence |
//! |---|---|---|
//! | `solver_profile: Option<SolverProfile>` | SHIPPED | `Certificate.solver_profile` (additive, `#[serde(default, skip_serializing_if)]` so the frozen golden `sum.cert.json` still deserializes — R-SPEC-2). `Some` ONLY on a timeout cert built by `Certificate::timeout` (`Level::L0` + `RejectReason { cause: "VerusTimeout" }` + the parsed profile + a profile-derived `suggested_move`); `None` on a proved cert and a counterexample cert. Produced by `profile::parse_profile`/`profile::suggested_move`, set by `check::classify_verus_outcome`. DIAGNOSTIC + non-deterministic (§5.3): EXCLUDED from `oracle_subset` (`.design/forge/solver-profiles.md` REQ-6/REQ-7). |
//! | `suggested_move` populated | SHIPPED | the reserved #5 slot is now CONSTRUCTED in production on a timeout cert (`Certificate::timeout`) from `profile::suggested_move` — the §5.1 "trigger hints" content. Still `None` on every non-timeout cert. |
//!
//! ## #13 producer (SOLVER-vacuity reject sets a `contract_quality` bool true)
//!
//! | Field/symbol | Status | Evidence |
//! |---|---|---|
//! | `Certificate::rejected_vacuity` | SHIPPED | builds a `Level::L0` reject cert (like `Certificate::rejected`) that ALSO sets the SOLVER-confirmed `contract_quality.{tautology,vacuous_precondition}` bool the detection corresponds to (`.design/forge/solver-vacuity.md` REQ-6, OQ-1). NO schema change (R-SPEC-2) — it only makes the EXISTING Appendix A bools' `true` real (solver-confirmed) rather than #6's syntactic `false`. Produced by `vacuity_solver::solver_vacuity_check`, consumed by `check::check_file`. |

use serde::{Deserialize, Serialize};
use thermite_syntax::{Effect, EffectRow};

use crate::profile::SolverProfile;

/// The assurance level (`thermite-design.md` §6). Serializes to the string form
/// `"L0".."L3"` to match the golden cert's `"level": "L3"` (REQ-1, REQ-7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Level {
    /// L0 — unverified / `#[slag]` escape hatch (§6, §8).
    L0,
    /// L1 — executable runtime check compiled in (§6).
    L1,
    /// L2 — bounded model check (Kani; issue #9) (§6).
    L2,
    /// L3 — SMT proof: the contract holds for all inputs (§6).
    L3,
}

/// The status of a single proof obligation (REQ-5). v0.1 records discharged or
/// failed; the failure carries a source-located diagnostic (the §5.1
/// "counterexamples, not adjectives" payload), never a bare boolean.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObligationStatus {
    /// The obligation was discharged by the solver.
    Discharged,
    /// The obligation failed; see the `diagnostic` and `location` on the result.
    Failed,
}

/// One per-obligation verification result (REQ-5, `.design/forge/check.md`
/// REQ-4). For a clean proof, `check.rs` records the verified item(s) as
/// `Discharged`; for a failure it records the failed obligation with verus's
/// `error: <clause>` description and its `--> file:line:col` source span — the
/// §5.1 "counterexample, not adjective".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObligationResult {
    /// The obligation identity (the verus function name for a discharged item,
    /// or the failed-clause description for a failure).
    pub name: String,
    /// Discharged or failed.
    pub status: ObligationStatus,
    /// `file:line:col` source span of the obligation, when verus reports one.
    /// `None` for a summary-only discharged result.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    /// The concrete failure diagnostic from verus's stderr (`error: <clause>`),
    /// present only on a failure. Never a bare "verification failed".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<String>,
}

impl ObligationResult {
    /// A discharged obligation (a verified verus function), summary-only.
    pub fn discharged(name: impl Into<String>) -> Self {
        ObligationResult {
            name: name.into(),
            status: ObligationStatus::Discharged,
            location: None,
            diagnostic: None,
        }
    }

    /// A failed obligation carrying its source location + diagnostic witness
    /// (§5.1 "counterexamples, not adjectives").
    pub fn failed(
        name: impl Into<String>,
        location: Option<String>,
        diagnostic: Option<String>,
    ) -> Self {
        ObligationResult {
            name: name.into(),
            status: ObligationStatus::Failed,
            location,
            diagnostic,
        }
    }
}

/// The contract-quality block (`thermite-design.md` §7, Appendix A) — REQ-3.
/// FORWARD-DECLARED in #5: the vacuity battery (`tautology`/
/// `vacuous_precondition`, #6/#13) and the mutation scorer
/// (`mutants_killed`/`survivor`, #12) are not yet built, so these carry honest
/// non-asserted values and are EXCLUDED from the cert-oracle comparison
/// (`Certificate::oracle_subset`). The schema reserves the slot; the value is
/// filled by its producer, never fabricated here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractQuality {
    /// Is the contract a tautology? (issue #6/#13) — `false` placeholder in #5.
    pub tautology: bool,
    /// Is the precondition vacuous? (issue #6/#13) — `false` placeholder in #5.
    pub vacuous_precondition: bool,
    /// Mutation kill ratio `"killed/total"` (issue #12) — `"0/0"` (unscored) in
    /// #5; typed `String` to match the Appendix A `"17/18"` shape (OQ-1).
    pub mutants_killed: String,
    /// The surviving-mutant description (issue #12) — `None` (unscored) in #5.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub survivor: Option<String>,
}

impl ContractQuality {
    /// The honest #5 value: the battery has not run, so nothing is asserted. NOT
    /// a fabricated pass — `mutants_killed` is the unscored `"0/0"`, not the
    /// golden `"17/18"` (REQ-3; `conformance/README.md` forward-declaration).
    pub fn forward_declared() -> Self {
        ContractQuality {
            tautology: false,
            vacuous_precondition: false,
            mutants_killed: "0/0".to_string(),
            survivor: None,
        }
    }
}

/// A reserved `suggested_move` heuristic hint (`thermite-design.md` §5.1) —
/// REQ-4. The slot exists so populating it later (missing-invariant patterns,
/// overflow-guard templates, trigger hints) is not a breaking schema change. In
/// #5 the `Certificate`'s `suggested_move` is always `None` (a reserved honest
/// absence: not a placeholder string and not an unimplemented stub).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuggestedMove {
    /// A short kind tag for the heuristic (e.g. `"missing-invariant"`).
    pub kind: String,
    /// The suggested edit text.
    pub detail: String,
}

/// The validated `#[slag]` metadata carried into a certificate (§8;
/// `.design/forge/slag.md` REQ-4). Produced by `slag::validate` once all three
/// mandatory fields are confirmed present + non-empty; recorded on the cert so a
/// reviewer can audit the fiat-trusted block (`slag: true` is the inventory flag,
/// these are the justification).
///
/// ADDITIVE schema field (`slag.md` OQ-1, ratified): Appendix A's certificate has
/// `slag: bool` only — `slag_meta` is a faithful superset, serialized only when
/// present (`#[serde(skip_serializing_if)]`), so the golden `sum.cert.json`
/// (which omits it) still deserializes (R-SPEC-2 — no frozen field renamed).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlagMeta {
    /// Why the body is fiat-trusted (non-empty after trim — `slag.rs` REQ-1).
    pub reason: String,
    /// The accountable owner (non-empty after trim).
    pub owner: String,
    /// The review status / requirement (non-empty after trim).
    pub review: String,
}

/// The structured reason a certificate is NOT certified (`.design/forge/vacuity-triage.md`
/// REQ-5; `slag.md` REQ-5). A triage / slag-validation failure is a CONTRACT-
/// certification failure surfaced INSIDE the certificate (§7 "a function does not
/// certify until its contract certifies"), not a `ForgeError` — the cert is a
/// valid document describing WHY the item did not certify. `check.rs` records
/// this on a non-certified (`Level::L0`) cert and exits non-zero.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RejectReason {
    /// A short machine-readable cause tag (the §7.1 verdict variant name, e.g.
    /// `"EnsIsTrivial"`, or a slag cause `"SlagFieldMissing"`).
    pub cause: String,
    /// A human-readable detail naming the offending clause / field.
    pub detail: String,
}

/// The certificate `forge check` emits for one item (`thermite-design.md` §5.1,
/// Appendix A). Field declaration order is the deterministic serialization order
/// (REQ-7) and mirrors Appendix A: `item`, `level`, `solver_time_ms`,
/// `contract_quality`, `effects`, `slag`; the #5 additive schema surface
/// (`obligations` — REQ-5; `suggested_move` — REQ-4) follows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Certificate {
    /// The checked item's name.
    pub item: String,
    /// The assurance level (REQ-2: L3 iff verus reports 0 errors).
    pub level: Level,
    /// Wall-clock solver time in ms — NON-DETERMINISTIC, excluded from the
    /// oracle comparison (REQ-6; `conformance/README.md`). `#[serde(default)]`
    /// so the golden deterministic-subset cert (which OMITS this non-det field)
    /// still deserializes into a full `Certificate` (certificate-manifest.md
    /// AC-2 — the schema is a faithful superset of the golden subset).
    #[serde(default)]
    pub solver_time_ms: u64,
    /// The contract-quality battery block — FORWARD-DECLARED in #5 (REQ-3).
    pub contract_quality: ContractQuality,
    /// The item's effect row (REQ-2: `["pure"]` for the corpus).
    pub effects: Vec<String>,
    /// Whether the item is `#[slag]` — `true` for a valid `#[slag]` item (#6/§8),
    /// `false` otherwise. Set by `check.rs` after `slag::validate` succeeds.
    pub slag: bool,
    /// The validated `#[slag]` metadata (#6 additive field; `slag.md` REQ-4).
    /// `Some` only on a valid slag item; `#[serde(default)]` + skip-if-none so
    /// the frozen golden cert (which omits it) still deserializes (R-SPEC-2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slag_meta: Option<SlagMeta>,
    /// The structured reason this item did NOT certify (#6 additive field;
    /// vacuity-triage.md REQ-5 / slag.md REQ-5). `Some` only on a triage / slag
    /// reject; `#[serde(default)]` + skip-if-none so a clean golden cert
    /// deserializes unchanged (R-SPEC-2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reject: Option<RejectReason>,
    /// Per-obligation results parsed from verus (REQ-5; #5 additive field).
    /// `#[serde(default)]` so a golden cert that does not enumerate the
    /// per-obligation array (the golden asserts only the item-level summary,
    /// certificate-manifest.md OQ-2) deserializes into a `Certificate`.
    #[serde(default)]
    pub obligations: Vec<ObligationResult>,
    /// Whether THIS certificate was served from the proof cache (#8 additive
    /// field; `.design/forge/proof-cache.md` REQ-7). `true` on a cache HIT (verus
    /// skipped), `false` on a fresh verify. `#[serde(default)]` so the frozen
    /// golden `conformance/sum.cert.json` (which omits it) still deserializes,
    /// mirroring the #6 `slag_meta`/`reject` additive precedent (R-SPEC-2). It is
    /// PROVENANCE, never verdict: EXCLUDED from `oracle_subset` so a cache hit and
    /// a fresh verify compare oracle-EQUAL (REQ-2, the soundness invariant).
    #[serde(default)]
    pub cached: bool,
    /// The structured Z3 quantifier-instantiation report attached on a verus
    /// TIMEOUT / rlimit-hit (issue #11 additive field;
    /// `.design/forge/solver-profiles.md` REQ-6). `Some` ONLY on a timeout cert
    /// (`Certificate::timeout`); `None` on a proved (`L3`) cert and on a
    /// counterexample-L0 cert (AC-4). `#[serde(default)]` + skip-if-none so the
    /// frozen golden `conformance/sum.cert.json` (which omits it) still
    /// deserializes (R-SPEC-2, additive only), mirroring the `slag_meta`/`reject`
    /// and `cached` additive precedents. DIAGNOSTIC and NON-deterministic (§5.3):
    /// EXCLUDED from `oracle_subset` (a timeout cert with a profile is
    /// oracle-equal to the same cert with the profile stripped).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub solver_profile: Option<SolverProfile>,
    /// Reserved heuristic-hint slot — `None` in #5 (REQ-4); populated on a
    /// timeout cert from `profile::suggested_move` (#11; the profile-derived
    /// proof-repair hint).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_move: Option<SuggestedMove>,
}

impl Certificate {
    /// Assemble a #5 certificate from the real pipeline data (REQ-2). `check.rs`
    /// derives `level`/`obligations` from verus and `effects` from the item's
    /// `fx` row; the forward-declared and reserved fields take their honest #5
    /// values here.
    pub fn new(
        item: impl Into<String>,
        level: Level,
        effects: Vec<String>,
        solver_time_ms: u64,
        obligations: Vec<ObligationResult>,
    ) -> Self {
        Certificate {
            item: item.into(),
            level,
            solver_time_ms,
            contract_quality: ContractQuality::forward_declared(),
            effects,
            slag: false,
            slag_meta: None,
            reject: None,
            obligations,
            cached: false,
            solver_profile: None,
            suggested_move: None,
        }
    }

    /// Build a TIMEOUT certificate for a verus run that exhausted its resource
    /// budget (`.design/forge/solver-profiles.md` REQ-6/REQ-7). DISTINCT from a
    /// counterexample-L0: a timeout records `Level::L0` with a structured
    /// `RejectReason { cause: "VerusTimeout" }` (NOT a `postcondition not
    /// satisfied` witness), carries the parsed `SolverProfile`, and populates
    /// `suggested_move` with the profile-derived proof-repair hint. v0.1 has no
    /// automatic degrade (#10), so the level is the un-discharged `L0` with the
    /// timeout reason — a timeout is reported, never silently treated as success
    /// (R-CODE-4). The profile + `suggested_move` are oracle-EXCLUDED (§5.3).
    pub fn timeout(
        item: impl Into<String>,
        effects: Vec<String>,
        solver_time_ms: u64,
        profile: SolverProfile,
        suggested_move: Option<SuggestedMove>,
        detail: String,
    ) -> Self {
        let reason = RejectReason {
            cause: "VerusTimeout".to_string(),
            detail,
        };
        let obligation =
            ObligationResult::failed(reason.cause.clone(), None, Some(reason.detail.clone()));
        Certificate {
            item: item.into(),
            level: Level::L0,
            solver_time_ms,
            contract_quality: ContractQuality::forward_declared(),
            effects,
            slag: false,
            slag_meta: None,
            reject: Some(reason),
            obligations: vec![obligation],
            cached: false,
            solver_profile: Some(profile),
            suggested_move,
        }
    }

    /// Set this certificate's cache-provenance flag (#8;
    /// `.design/forge/proof-cache.md` REQ-7). Returns the certificate with
    /// `cached` set: `true` when served from a HIT (verus skipped), `false` on a
    /// fresh verify. Only the provenance bit changes — every deterministic
    /// (oracle) field is untouched, so a hit stays oracle-equal to the fresh
    /// verify it was stored from (REQ-2, the soundness invariant). Consumed by
    /// `check::check_file` and `cache::store` (which clears it before persisting).
    pub fn with_cached(mut self, cached: bool) -> Self {
        self.cached = cached;
        self
    }

    /// Graduate the two §7.1 structural-triage `contract_quality` bools to their
    /// #6-LIVE `false` values on an item that PASSED triage
    /// (`.design/forge/vacuity-triage.md` REQ-6 / AC-7). The syntactic triage has
    /// confirmed the contract is not a syntactic tautology and its precondition is
    /// not syntactically vacuous, so these are ASSERTED `false` (no longer
    /// forward-declared placeholders). The SOLVER-derived truth of these fields
    /// (a genuine non-syntactic tautology / unsat precondition) stays
    /// forward-declared for #13; `mutants_killed`/`survivor` stay #12.
    pub fn graduate_triage_clean(mut self) -> Self {
        self.contract_quality.tautology = false;
        self.contract_quality.vacuous_precondition = false;
        self
    }

    /// Build a valid-`#[slag]` certificate (`.design/forge/slag.md` REQ-2/REQ-4):
    /// `Level::L1` (contract runtime-enforced; body fiat-trusted), `slag: true`,
    /// the validated metadata, and a single discharged obligation recording the
    /// proof-exempt-by-fiat fact (NOT a verus obligation — no proof was run). The
    /// triage bools graduate to live-`false` (a slag item still passes (a)/(b)/(c)
    /// triage before this is built).
    pub fn slag_l1(item: impl Into<String>, effects: Vec<String>, meta: SlagMeta) -> Self {
        Certificate {
            item: item.into(),
            level: Level::L1,
            solver_time_ms: 0,
            contract_quality: ContractQuality::forward_declared(),
            effects,
            slag: true,
            slag_meta: Some(meta),
            reject: None,
            obligations: vec![ObligationResult::discharged(
                "contract enforced at L1 (slag); proof exempt by fiat",
            )],
            cached: false,
            solver_profile: None,
            suggested_move: None,
        }
        .graduate_triage_clean()
    }

    /// Build a NON-certified certificate for a triage / slag-validation reject
    /// (`.design/forge/vacuity-triage.md` REQ-5 / `slag.md` REQ-5). The item did
    /// not certify (`Level::L0`); the cert is a valid document carrying the
    /// structured `reject` cause + a single failed obligation naming it. `slag`
    /// records whether the rejected item carried a `#[slag]` attribute (its
    /// metadata is NOT carried — the item did not certify).
    pub fn rejected(
        item: impl Into<String>,
        effects: Vec<String>,
        slag: bool,
        reason: RejectReason,
    ) -> Self {
        let obligation =
            ObligationResult::failed(reason.cause.clone(), None, Some(reason.detail.clone()));
        Certificate {
            item: item.into(),
            level: Level::L0,
            solver_time_ms: 0,
            contract_quality: ContractQuality::forward_declared(),
            effects,
            slag,
            slag_meta: None,
            reject: Some(reason),
            obligations: vec![obligation],
            cached: false,
            solver_profile: None,
            suggested_move: None,
        }
    }

    /// Build a NON-certified certificate for a SOLVER-vacuity reject (#13;
    /// `.design/forge/solver-vacuity.md` REQ-5/REQ-6). Like [`Certificate::rejected`]
    /// (`Level::L0`, the structured `reject` cause, one failed obligation naming
    /// it), but it ALSO sets the SOLVER-confirmed `contract_quality` bool that the
    /// detected degeneracy corresponds to (REQ-6, OQ-1): a `"SemanticTautology"`
    /// reject sets `contract_quality.tautology = true`; a `"VacuousPrecondition"`
    /// reject sets `contract_quality.vacuous_precondition = true`. `set_tautology` /
    /// `set_vacuous_precondition` are the two existing Appendix A bools — NO schema
    /// change (R-SPEC-2); #13 only makes the `true` detection real (solver-confirmed)
    /// rather than the #6-syntactic `false`. Consumed by `check::gate_fn`.
    pub fn rejected_vacuity(
        item: impl Into<String>,
        effects: Vec<String>,
        reason: RejectReason,
        set_tautology: bool,
        set_vacuous_precondition: bool,
    ) -> Self {
        let mut cert = Certificate::rejected(item, effects, false, reason);
        cert.contract_quality.tautology = set_tautology;
        cert.contract_quality.vacuous_precondition = set_vacuous_precondition;
        cert
    }

    /// Graduate the mutation-scoring `contract_quality` fields on a CERTIFIED
    /// (kill-ratio-met) item (#12; `.design/forge/mutation-scoring.md` REQ-6). The
    /// item proved L3 AND its frozen mutant set met the floor, so the cert records
    /// the real `"<killed>/<scored>"` kill ratio (graduated from the forward-
    /// declared `"0/0"`) and a representative `survivor` (the first surviving
    /// mutant's description, or `None` when every scored mutant was killed). NO
    /// schema field is added or renamed (R-SPEC-2) — this only makes the two
    /// EXISTING Appendix A `contract_quality` fields LIVE. Consumed by
    /// `check::check_file_with_options`'s post-L3 mutation stage.
    pub fn with_mutation_score(mut self, mutants_killed: String, survivor: Option<String>) -> Self {
        self.contract_quality.mutants_killed = mutants_killed;
        self.contract_quality.survivor = survivor;
        self
    }

    /// Build a NON-certified certificate for a WEAK-CONTRACT reject (#12;
    /// `.design/forge/mutation-scoring.md` REQ-5/REQ-6). The item's REAL body
    /// proved L3, but its frozen mutant set scored BELOW the floor — the contract
    /// under-constrains the body (mutants survive). Like [`Certificate::rejected`]
    /// (`Level::L0`, the structured `reject` cause, one failed obligation naming
    /// it), but it ALSO records the real `mutants_killed` ratio and the surviving-
    /// mutant `survivor` — the §7 "precise strengthening prompt". The `cause` is
    /// `"WeakContract"` (a distinct tag namespace from #6/#13's vacuity causes), so
    /// a cert reader can tell an under-constraining contract from a degenerate one.
    /// Consumed by `check::check_file_with_options`.
    pub fn rejected_weak_contract(
        item: impl Into<String>,
        effects: Vec<String>,
        mutants_killed: String,
        survivor: String,
    ) -> Self {
        let reason = RejectReason {
            cause: "WeakContract".to_string(),
            detail: format!(
                "§7 step 4: the contract under-constrains the body — mutation kill ratio \
                 {mutants_killed} is below the floor; mutant `{survivor}` survived (verus \
                 proved the deliberately-wrong body against this contract), so the contract \
                 does not distinguish it from the real body — strengthen the `ens` to pin \
                 the behavior `{survivor}` changes"
            ),
        };
        // Reuse the triage-clean reject shape (the item PASSED #6 + #13 + L3; the
        // only defect is contract strength), then record the mutation fields.
        Certificate::rejected(item, effects, false, reason)
            .with_mutation_score(mutants_killed, Some(survivor))
    }

    /// The DETERMINISTIC, currently-producible oracle subset (REQ-3/REQ-6,
    /// `.design/forge/check.md` AC-1): `(item, level, effects, slag)`. The
    /// forward-declared `contract_quality.*` and the non-deterministic
    /// `solver_time_ms` are STRUCTURALLY excluded by being absent from this
    /// tuple. The cert-oracle (`tests/check_conformance.rs`) and the human
    /// renderer (`cli::render_human`, which prints exactly this subset plus the
    /// excluded `solver_time_ms` labelled as such) treat these four as the
    /// oracle-stable fields in #5.
    pub fn oracle_subset(&self) -> (&str, Level, &[String], bool) {
        (&self.item, self.level, &self.effects, self.slag)
    }
}

/// Map a parsed `EffectRow` to the certificate's `effects` string vector
/// (REQ-2). `Pure` → `["pure"]`; a non-pure row maps each `Effect` to its
/// canonical lowercase token in declaration order (deterministic, R-CODE-5).
/// Covers EVERY `Effect` variant (the whole closed enum), not just the corpus's
/// `pure`.
pub fn effects_of(fx: &EffectRow) -> Vec<String> {
    match fx {
        EffectRow::Pure => vec!["pure".to_string()],
        EffectRow::Set(effects) => effects.iter().map(effect_token).collect(),
    }
}

/// The canonical lowercase token for one `Effect` (e.g. `read(x)`, `alloc`).
fn effect_token(effect: &Effect) -> String {
    match effect {
        Effect::Read(name) => format!("read({name})"),
        Effect::Write(name) => format!("write({name})"),
        Effect::Net(name) => format!("net({name})"),
        Effect::Alloc => "alloc".to_string(),
        Effect::Time => "time".to_string(),
        Effect::Rand => "rand".to_string(),
        Effect::Panic => "panic".to_string(),
        Effect::Diverge => "diverge".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test-local serializer (mirrors `cli::run_check`'s
    /// `serde_json::to_string_pretty`).
    fn serialize(cert: &Certificate) -> String {
        serde_json::to_string_pretty(cert).expect("serialize cert")
    }

    /// The deterministic-subset equality the cert-oracle uses, expressed via the
    /// production `oracle_subset` accessor (so the test exercises the real schema
    /// property, not a re-implementation).
    fn oracle_eq(a: &Certificate, b: &Certificate) -> bool {
        a.oracle_subset() == b.oracle_subset()
    }

    // AC-1: schema matches Appendix A — every documented key present, Level::L3
    // serializes to "L3". Expected keys/values trace to `thermite-design.md`
    // Appendix A (R-CHAR-3), not to forge's own output.
    #[test]
    fn schema_matches_appendix_a() {
        let cert = Certificate::new(
            "sum",
            Level::L3,
            vec!["pure".to_string()],
            612,
            vec![ObligationResult::discharged("sum_check::sum")],
        );
        let json = serialize(&cert);
        let value: serde_json::Value = serde_json::from_str(&json).expect("parse");
        // Appendix A keys.
        for key in [
            "item",
            "level",
            "solver_time_ms",
            "contract_quality",
            "effects",
            "slag",
        ] {
            assert!(value.get(key).is_some(), "missing Appendix A key `{key}`");
        }
        // contract_quality sub-keys (Appendix A).
        let cq = value.get("contract_quality").expect("contract_quality");
        for key in ["tautology", "vacuous_precondition", "mutants_killed"] {
            assert!(cq.get(key).is_some(), "missing contract_quality.{key}");
        }
        // Level::L3 serializes to the string "L3".
        assert_eq!(value.get("level").and_then(|v| v.as_str()), Some("L3"));
    }

    // AC-2: the golden cert's deterministic subset deserializes into a
    // Certificate and re-serializes equal on those fields. Anchors to the GOLDEN
    // `conformance/sum.cert.json` (R-CHAR-3), not forge's output.
    #[test]
    fn golden_deterministic_subset_round_trips() {
        let golden_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("conformance")
            .join("sum.cert.json");
        let golden_src = std::fs::read_to_string(&golden_path).expect("read golden cert");
        let golden: Certificate = serde_json::from_str(&golden_src).expect("deserialize golden");
        assert_eq!(golden.item, "sum");
        assert_eq!(golden.level, Level::L3);
        assert_eq!(golden.effects, vec!["pure".to_string()]);
        assert!(!golden.slag);
        // A freshly assembled #5 cert with the same deterministic fields is
        // oracle-equal to the golden, despite differing battery / time fields.
        let ours = Certificate::new(
            "sum",
            Level::L3,
            vec!["pure".to_string()],
            42,
            vec![ObligationResult::discharged("sum_check::sum")],
        );
        assert!(
            oracle_eq(&golden, &ours),
            "the golden subset must oracle-match a #5 cert"
        );
    }

    // AC-3: forward-declared fields excluded from the live oracle — two certs
    // differing ONLY in contract_quality / solver_time_ms compare equal.
    #[test]
    fn oracle_ignores_forward_declared_and_time() {
        let mut a = Certificate::new("f", Level::L3, vec!["pure".to_string()], 1, vec![]);
        let mut b = a.clone();
        b.solver_time_ms = 99_999;
        b.contract_quality.mutants_killed = "17/18".to_string();
        b.contract_quality.tautology = true;
        assert!(
            oracle_eq(&a, &b),
            "oracle must ignore time + battery fields"
        );
        // But a differing deterministic field IS caught.
        a.level = Level::L1;
        assert!(!oracle_eq(&a, &b), "oracle must catch a level mismatch");
    }

    // AC-4: suggested_move is a reserved absence — serializes as omitted (its
    // Option is None), never a placeholder.
    #[test]
    fn suggested_move_is_reserved_absence() {
        let cert = Certificate::new("f", Level::L3, vec!["pure".to_string()], 0, vec![]);
        assert!(cert.suggested_move.is_none());
        let json = serialize(&cert);
        assert!(
            !json.contains("suggested_move"),
            "None suggested_move must be omitted, not a placeholder:\n{json}"
        );
    }

    // AC-5: per-obligation list present for pass and fail; a failure carries a
    // source-located diagnostic.
    #[test]
    fn obligation_results_present() {
        let pass = ObligationResult::discharged("sum_check::sum");
        assert_eq!(pass.status, ObligationStatus::Discharged);
        let fail = ObligationResult::failed(
            "postcondition not satisfied",
            Some("broken_check.rs:5:13".to_string()),
            Some("error: postcondition not satisfied".to_string()),
        );
        assert_eq!(fail.status, ObligationStatus::Failed);
        assert!(fail.location.is_some(), "failure carries a source location");
        assert!(fail.diagnostic.is_some(), "failure carries a diagnostic");
    }

    // AC-6: determinism — serializing the same Certificate twice is
    // byte-identical (R-CODE-5).
    #[test]
    fn serialization_is_deterministic() {
        let cert = Certificate::new(
            "sum",
            Level::L3,
            vec!["pure".to_string()],
            612,
            vec![ObligationResult::discharged("sum_check::sum")],
        );
        let a = serialize(&cert);
        let b = serialize(&cert);
        assert_eq!(a, b);
    }

    // #6 AC: the additive `slag_meta`/`reject` fields are ABSENT on a plain #5
    // cert and on the golden — so the golden `sum.cert.json` still deserializes
    // (R-SPEC-2). A None `slag_meta`/`reject` must not serialize.
    #[test]
    fn slag_and_reject_fields_are_additive_and_skipped_when_none() {
        let cert = Certificate::new("f", Level::L3, vec!["pure".to_string()], 0, vec![]);
        assert!(cert.slag_meta.is_none());
        assert!(cert.reject.is_none());
        let json = serialize(&cert);
        assert!(
            !json.contains("slag_meta"),
            "None slag_meta omitted:\n{json}"
        );
        assert!(!json.contains("reject"), "None reject omitted:\n{json}");
        // The frozen golden cert (no slag_meta/reject) deserializes unchanged.
        let golden_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("conformance")
            .join("sum.cert.json");
        let golden_src = std::fs::read_to_string(&golden_path);
        assert!(golden_src.is_ok(), "read golden: {golden_src:?}");
        if let Ok(src) = golden_src {
            let golden: Result<Certificate, _> = serde_json::from_str(&src);
            assert!(golden.is_ok(), "golden deserializes: {golden:?}");
            if let Ok(g) = golden {
                assert!(g.slag_meta.is_none());
                assert!(g.reject.is_none());
            }
        }
    }

    // #8 (proof-cache REQ-7 / AC-5): `cached` is an ADDITIVE field that defaults
    // `false` (the golden `sum.cert.json` omits it) and is EXCLUDED from the
    // oracle subset -- a HIT (`cached: true`) is oracle-equal to the fresh verify
    // it was stored from. Expected behavior traces to `proof-cache.md` REQ-7/REQ-2
    // (R-CHAR-3), not forge's output.
    #[test]
    fn cached_field_is_additive_and_oracle_excluded() {
        let fresh = Certificate::new("f", Level::L3, vec!["pure".to_string()], 1, vec![]);
        assert!(!fresh.cached, "a fresh cert is not cached by default");

        // `with_cached(true)` flips ONLY the provenance bit; the oracle subset is
        // unchanged, so a hit is oracle-equal to the fresh verify (REQ-2).
        let hit = fresh.clone().with_cached(true);
        assert!(hit.cached);
        assert!(
            oracle_eq(&fresh, &hit),
            "a cache hit must be oracle-equal to the fresh verify it was stored from"
        );

        // The golden `conformance/sum.cert.json` (which omits `cached`) still
        // deserializes, defaulting `cached` to `false` (additive, R-SPEC-2).
        let golden_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("conformance")
            .join("sum.cert.json");
        let golden_src = std::fs::read_to_string(&golden_path);
        assert!(golden_src.is_ok(), "read golden cert: {golden_src:?}");
        if let Ok(src) = golden_src {
            let golden: Result<Certificate, _> = serde_json::from_str(&src);
            assert!(golden.is_ok(), "golden deserializes: {golden:?}");
            if let Ok(g) = golden {
                assert!(
                    !g.cached,
                    "golden omits `cached`, defaults false (additive)"
                );
            }
        }
    }

    // #6 AC-1/AC-4 (slag.md): a valid slag cert is L1, slag:true, carries the
    // metadata, and is NOT a verus obligation. Expected level/flag trace to
    // `slag.md` REQ-2/REQ-4 (R-CHAR-3), not forge's output.
    #[test]
    fn slag_l1_cert_shape() {
        let meta = SlagMeta {
            reason: "vendored".to_string(),
            owner: "agent:forge-7".to_string(),
            review: "required".to_string(),
        };
        let cert = Certificate::slag_l1("simd_sum", vec!["pure".to_string()], meta.clone());
        assert_eq!(cert.level, Level::L1);
        assert!(cert.slag);
        assert_eq!(cert.slag_meta, Some(meta));
        // The triage bools graduated to live-false even on the slag path.
        assert!(!cert.contract_quality.tautology);
        assert!(!cert.contract_quality.vacuous_precondition);
        let json = serialize(&cert);
        assert!(json.contains("slag_meta"), "slag cert carries metadata");
    }

    // #6 (vacuity-triage REQ-5): a triage reject is a NON-certified (L0) cert
    // carrying the structured cause, not a ForgeError.
    #[test]
    fn rejected_cert_carries_cause_and_is_not_l3() {
        let reason = RejectReason {
            cause: "EnsIsTrivial".to_string(),
            detail: "ens#0 is the literal `true`".to_string(),
        };
        let cert = Certificate::rejected("f", vec!["pure".to_string()], false, reason);
        assert_eq!(cert.level, Level::L0);
        assert_ne!(cert.level, Level::L3);
        assert_eq!(
            cert.reject.as_ref().map(|r| r.cause.as_str()),
            Some("EnsIsTrivial")
        );
        assert_eq!(cert.obligations.len(), 1);
        assert_eq!(cert.obligations[0].status, ObligationStatus::Failed);
    }

    // #6 (vacuity-triage AC-7): a triage-passing item graduates the two bools to
    // asserted live-false.
    #[test]
    fn graduate_triage_clean_sets_live_false() {
        let cert = Certificate::new("f", Level::L3, vec!["pure".to_string()], 0, vec![])
            .graduate_triage_clean();
        assert!(!cert.contract_quality.tautology);
        assert!(!cert.contract_quality.vacuous_precondition);
    }

    // #12 (mutation-scoring REQ-6): `with_mutation_score` graduates the two
    // forward-declared fields to LIVE on a certified item; the oracle subset is
    // unchanged (the fields stay oracle-excluded). Expected behavior traces to
    // `mutation-scoring.md` REQ-6 (R-CHAR-3), not forge's output.
    #[test]
    fn with_mutation_score_graduates_fields_and_stays_oracle_excluded() {
        let base = Certificate::new("sum", Level::L3, vec!["pure".to_string()], 0, vec![]);
        // Forward-declared default before scoring.
        assert_eq!(base.contract_quality.mutants_killed, "0/0");
        assert!(base.contract_quality.survivor.is_none());

        let scored = base.clone().with_mutation_score("17/18".to_string(), None);
        assert_eq!(scored.contract_quality.mutants_killed, "17/18");
        assert!(scored.contract_quality.survivor.is_none());
        // The kill ratio is oracle-EXCLUDED: a graduated cert is oracle-equal to the
        // forward-declared one (OQ-1 — the ratio is verus-version-sensitive).
        assert!(oracle_eq(&base, &scored));
        assert_eq!(base.level, scored.level);
    }

    // #12 (mutation-scoring REQ-5/REQ-6): a `WeakContract` reject is a NON-certified
    // (L0) cert carrying the `"WeakContract"` cause, the real kill ratio, and a
    // surviving-mutant `survivor` (the §7 strengthening prompt). Expected cause/level
    // trace to `mutation-scoring.md` REQ-5 (R-CHAR-3), not forge's output.
    #[test]
    fn rejected_weak_contract_carries_cause_ratio_and_survivor() {
        let cert = Certificate::rejected_weak_contract(
            "f",
            vec!["pure".to_string()],
            "1/3".to_string(),
            "insert early `return 0` at body head".to_string(),
        );
        assert_eq!(cert.level, Level::L0);
        assert_ne!(cert.level, Level::L3);
        assert_eq!(
            cert.reject.as_ref().map(|r| r.cause.as_str()),
            Some("WeakContract")
        );
        assert_eq!(cert.contract_quality.mutants_killed, "1/3");
        assert_eq!(
            cert.contract_quality.survivor.as_deref(),
            Some("insert early `return 0` at body head")
        );
        // The detail names the surviving mutant (the precise prompt §7 describes).
        let detail = cert
            .reject
            .as_ref()
            .map(|r| r.detail.clone())
            .unwrap_or_default();
        assert!(
            detail.contains("insert early `return 0` at body head"),
            "detail names the survivor: {detail}"
        );
    }

    // effects_of covers the whole Effect enum, not just `pure` (R-DEFER-8: fix
    // the whole class). Expected tokens are this module's documented mapping.
    #[test]
    fn effects_of_covers_every_variant() {
        assert_eq!(effects_of(&EffectRow::Pure), vec!["pure".to_string()]);
        let row = EffectRow::Set(vec![
            Effect::Read("x".to_string()),
            Effect::Write("y".to_string()),
            Effect::Net("z".to_string()),
            Effect::Alloc,
            Effect::Time,
            Effect::Rand,
            Effect::Panic,
            Effect::Diverge,
        ]);
        assert_eq!(
            effects_of(&row),
            vec!["read(x)", "write(y)", "net(z)", "alloc", "time", "rand", "panic", "diverge"]
        );
    }
}
