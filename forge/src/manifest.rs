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
//!
//! ## #16 additive schema (boundary-fn FFI cert — `.design/boundary/ffi-boundary.md`)
//!
//! | Field/symbol | Status | Evidence |
//! |---|---|---|
//! | `boundary: bool` (FFI verdict flag) | SHIPPED | `Certificate.boundary` (additive, `#[serde(default)]` so the frozen golden `conformance/sum.cert.json` — which omits it, defaulting `false` — still deserializes, R-SPEC-2). `true` ONLY on a boundary-fn cert built by `Certificate::boundary_l1`. VERDICT-relevant (qualifies the L1 as "to-the-boundary, foreign body unproven") and feeds #15 (TCB enumeration) / #17 (verified-to-the-boundary): joins `slag` in `oracle_subset`. Set by `check::gate_fn`. |
//! | `boundary_target: Option<String>` (foreign path) | SHIPPED | `Certificate.boundary_target` (additive, `#[serde(default, skip_serializing_if = "Option::is_none")]`). `Some(crate::path)` ONLY on a boundary cert (the foreign target the L1 wrapper calls); `None` otherwise. DIAGNOSTIC (the #15 audit hook's prose half): oracle-EXCLUDED. |
//! | `Certificate::boundary_l1` | SHIPPED | builds the boundary cert: `Level::L1`, `boundary: true`, `boundary_target: Some(target)`, one discharged obligation ("contract enforced at L1 (boundary); foreign body trusted by fiat"), NO verus run, `graduate_triage_clean()` (a boundary fn still passes §7.1 (a)/(b)/(c)). Modeled on `Certificate::slag_l1`. Consumed by `check::gate_fn`. |
//!
//! ## #10 additive schema (the degrade ladder + assurance aggregate, this iteration)
//!
//! | Field/symbol | Status | Evidence |
//! |---|---|---|
//! | `lowered_assurance: bool` (degrade flag) | SHIPPED | `Certificate.lowered_assurance` (additive, `#[serde(default)]` so the frozen golden `sum.cert.json` still deserializes — R-SPEC-2). `true` ONLY on a cert the #10 ladder produced by degrading a verus TIMEOUT to L2/L1; set by `Certificate::into_degraded`, produced by `degrade::run_ladder`, consumed by `check::ladder_for_timeout` + `cli::render_assurance`. VERDICT-relevant (it qualifies the level as "lowered, not proved") so NOT oracle-excluded; the corpus never degrades so the golden keeps the default `false`. |
//! | `degrade_reason: Option<RejectReason>` | SHIPPED | `Certificate.degrade_reason` (additive, `#[serde(default, skip_serializing_if)]`). `Some` ONLY on a `lowered_assurance` cert — the `VerusTimeout` reason carried from the timed-out L3 attempt (REQ-4). Set by `Certificate::into_degraded`; DIAGNOSTIC, EXCLUDED from `oracle_subset`. |
//! | `Level: Ord` (ladder ordering) | SHIPPED | `#[derive(PartialOrd, Ord)]` on `enum Level` makes the declaration order `L0 < L1 < L2 < L3` the `Ord` the aggregate's min-over-functions uses (`.design/forge/degrade-ladder.md` REQ-6). |
//! | `AssuranceManifest` + `ProjectAssurance` (the aggregate) | SHIPPED | `AssuranceManifest::aggregate(&[Certificate])` computes the per-fn `FunctionAssurance` rows + the project headline `ProjectAssurance::{Certified(min), Failed}` (REQ-5/REQ-6); a non-certifying fn (`cert_certifies` false) caps the project at `Failed` (a non-rung, REQ-2). Render-time aggregate (OQ-4 (b)). Consumed by `cli::run_check`/`render_assurance`. VERUS-ANCHORED (epic #60, REQ-10 / `.design/verified/self-verification.md` Target D): the project-level min-over-functions is anchored to the proved fold-min `thermite_verified::aggregate_level` (D1: ≤ every fn — the §5.2 no-over-claim bound; D2: attained == exactly the min) by the in-module `tests::verus_anchor` block (Option B, forge binary-only) enumerating ALL `Level` lists up to length 4 (341 lists), asserting `aggregate(certs).project == Certified(proved_min)` AND headline ≤ every level. |
//!
//! ## #17 additive schema (the §9 end-to-end vs to-the-boundary scope, this iteration)
//!
//! | Field/symbol | Status | Evidence |
//! |---|---|---|
//! | `AssuranceScope` (per-fn §9 scope) | SHIPPED | `enum AssuranceScope { EndToEnd, ToBoundary { via } }` (`.design/forge/e2e-vs-boundary.md` REQ-2/REQ-3); `Certificate.assurance_scope: Option<AssuranceScope>` (additive, `#[serde(default, skip_serializing_if = "Option::is_none")]` so the frozen golden `conformance/sum.cert.json` — which omits it — still deserializes, defaulting `None`, mirroring the `boundary_target`/`solver_profile` precedents, R-SPEC-2). Produced by `closure::classify`, set by `Certificate::with_assurance_scope`, consumed by `check::check_file_with_options`. VERDICT-RELEVANT (§9 / R-DEFER-9) so it JOINS `oracle_subset` — NORMALIZED to a bool (`scope_is_end_to_end`): `None` and `Some(EndToEnd)` are oracle-EQUAL (golden stays stable) while `Some(ToBoundary)` is oracle-visible; the `via` crossing name is diagnostic, oracle-EXCLUDED. ORTHOGONAL to `level` (REQ-5). |
//! | `ProjectScope` (project §9 claim) | SHIPPED | `enum ProjectScope { EndToEnd, ToBoundary { crossings } }` + `AssuranceManifest.scope`; `AssuranceManifest::aggregate` computes it (`project_scope`): END-TO-END iff every cert is end-to-end, else TO-THE-BOUNDARY listing the reached crossings (sorted + deduplicated, deterministic — REQ-4/REQ-6). ORTHOGONAL to the `project` level headline. Consumed by `cli::run_check`. |

use serde::{Deserialize, Serialize};
use thermite_syntax::{Effect, EffectRow};

use crate::profile::SolverProfile;
use crate::strengthen::Suggestion;

/// The §9 ASSURANCE SCOPE of a function (issue #17,
/// `.design/forge/e2e-vs-boundary.md` REQ-2/REQ-3; `thermite-design.md` §9). The
/// manifest distinction "verified to the boundary" vs "verified, period":
///
/// - [`AssuranceScope::EndToEnd`] — the fn's transitive intra-file call closure
///   reaches NO `#[boundary]` (foreign body) and NO `#[slag]` (fiat-trusted body)
///   fn; the whole-program guarantee rests only on the toolchain ("verified,
///   period").
/// - [`AssuranceScope::ToBoundary`] — the closure transitively reaches a crossing;
///   `via` names the first reached `#[boundary]`/`#[slag]` fn. The fn's own
///   contract is verified, but the end-to-end guarantee crosses a foreign/unproven
///   body (`goal.md` R-DEFER-9 — honestly mark such a guarantee).
///
/// ORTHOGONAL to [`Level`] (REQ-5): a `ToBoundary` fn may be `Level::L3` (its own
/// body fully SMT-proved against the crossing's contract). Produced by
/// [`crate::closure::classify`] (the structural call-closure analysis); recorded
/// as the additive [`Certificate::assurance_scope`] field.
///
/// The serialized form is a tagged enum (`{"kind": "end_to_end"}` /
/// `{"kind": "to_boundary", "via": "<fn>"}`), mirroring [`ProjectAssurance`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum AssuranceScope {
    /// "Verified, period": no `#[boundary]`/`#[slag]` fn in the transitive closure.
    EndToEnd,
    /// "Verified to the boundary": the closure reaches a crossing; `via` is the
    /// first reached `#[boundary]`/`#[slag]` fn (the deterministic crossing).
    ToBoundary {
        /// The name of the first `#[boundary]`/`#[slag]` fn the closure reaches.
        via: String,
    },
}

impl AssuranceScope {
    /// `true` iff this scope is END-TO-END ("verified, period"). The verdict-
    /// relevant bit the cert-oracle compares (see [`Certificate::oracle_subset`]):
    /// a `None` `assurance_scope` (the frozen golden `sum.cert.json`, which omits
    /// the field) and `Some(EndToEnd)` (a freshly-classified pure fn) BOTH read
    /// `true` here, so the golden subset stays stable (R-SPEC-2) while a
    /// `ToBoundary` verdict is oracle-visible.
    pub fn is_end_to_end(&self) -> bool {
        matches!(self, AssuranceScope::EndToEnd)
    }
}

/// `true` iff `scope` is END-TO-END for the oracle (REQ-3): `None` (field absent,
/// the golden default) OR `Some(EndToEnd)`. A `Some(ToBoundary)` reads `false`.
/// This is the normalization that keeps the golden `sum.cert.json` (no
/// `assurance_scope` key) oracle-equal to a freshly-classified `Some(EndToEnd)`
/// `sum` cert (`.design/forge/e2e-vs-boundary.md` Verification).
fn scope_is_end_to_end(scope: &Option<AssuranceScope>) -> bool {
    match scope {
        None => true,
        Some(s) => s.is_end_to_end(),
    }
}

/// The assurance level (`thermite-design.md` §6). Serializes to the string form
/// `"L0".."L3"` to match the golden cert's `"level": "L3"` (REQ-1, REQ-7).
///
/// The declaration order `L0 < L1 < L2 < L3` IS the ladder ordering
/// (`.design/forge/degrade-ladder.md` REQ-6): `#[derive(PartialOrd, Ord)]` makes
/// it the `Ord` the assurance-manifest aggregate uses for the min-over-functions
/// project headline. The discriminant order is load-bearing — do not reorder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
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
    /// Whether this certificate was achieved by an AUTOMATIC DEGRADE below L3
    /// (issue #10 additive field; `.design/forge/degrade-ladder.md` REQ-4). `true`
    /// on a cert the L3→L2→L1 ladder produced after a verus TIMEOUT degraded the
    /// item to L2 (kani bounded check) or L1 (runtime checks); `false` on every
    /// directly-achieved cert (an L3 proof, an EXPLICIT `--level l2`/`--level l1`
    /// choice, a `#[slag]` L1-by-fiat, a reject). `#[serde(default)]` so the frozen
    /// golden `conformance/sum.cert.json` (which omits it) still deserializes,
    /// mirroring the `cached` additive precedent (R-SPEC-2). It is VERDICT-RELEVANT
    /// (it qualifies the achieved level as "lowered, not proved") so it is NOT
    /// oracle-excluded — but the corpus at the default budget never degrades, so the
    /// golden cert keeps the default `false` (AC-1/AC-6).
    #[serde(default)]
    pub lowered_assurance: bool,
    /// The structured reason this item was DEGRADED below L3 (issue #10 additive
    /// field; `.design/forge/degrade-ladder.md` REQ-4). `Some` ONLY on a
    /// `lowered_assurance` cert — the `VerusTimeout` reason ("here's where I got
    /// lost") carried from the L3 attempt that timed out. `#[serde(default,
    /// skip_serializing_if)]` so a non-degraded cert (the golden) deserializes
    /// unchanged (R-SPEC-2). DIAGNOSTIC + non-deterministic in content (it carries
    /// the same kind of material as the §5.3 `solver_profile`): EXCLUDED from
    /// `oracle_subset` (a degraded cert is oracle-compared on its `level` +
    /// `lowered_assurance` flag, not on the prose reason).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub degrade_reason: Option<RejectReason>,
    /// The §7 step-5 STRENGTHENING SUGGESTIONS surfaced for this item (issue #14
    /// additive field; `.design/forge/strengthening-probes.md` REQ-4). Each
    /// [`Suggestion`] is an adoptable stronger-`ens` clause that VERIFIES against
    /// the real body AND is strictly stronger than the current `ens` (it would
    /// kill a #12 survivor / adds an equality the `ens` lacks). ADVISORY: a probe
    /// NEVER changes the verdict (`level`/`reject`/the oracle subset) — it only
    /// ADDS these. `#[serde(default, skip_serializing_if = Vec::is_empty)]` so the
    /// frozen golden `conformance/sum.cert.json` (which omits it, and for which the
    /// probe emits nothing) still deserializes (R-SPEC-2, additive only), mirroring
    /// the `solver_profile` additive precedent. DIAGNOSTIC + verus-version-
    /// sensitive (a future verus might prove a candidate today's cannot), so it is
    /// EXCLUDED from `oracle_subset` (parallel to `solver_profile`/`mutants_killed`,
    /// OQ-3). An item with no surviving candidate carries an EMPTY list (an honest
    /// absence, mirroring the `suggested_move: None` precedent).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub strengthening: Vec<Suggestion>,
    /// Whether this item is a FOREIGN-CROSSING boundary fn (issue #16 additive
    /// field; `.design/boundary/ffi-boundary.md` REQ-5). `true` ONLY on a
    /// boundary-fn cert (`Certificate::boundary_l1`): a `#[boundary("crate::path")]`
    /// fn whose foreign body is UNPROVEN and whose contract is enforced at L1 (the
    /// FFI analog of `slag: true`). `#[serde(default)]` so the frozen golden
    /// `conformance/sum.cert.json` (which omits it) still deserializes, defaulting
    /// `false`, mirroring the `slag`/`cached`/`lowered_assurance` additive
    /// precedents (R-SPEC-2). VERDICT-RELEVANT (it qualifies the achieved L1 as
    /// "to-the-boundary, foreign body unproven", the #15 TCB-enumeration + #17
    /// verified-to-the-boundary input), so it JOINS `slag` in `oracle_subset`.
    #[serde(default)]
    pub boundary: bool,
    /// The foreign `crate::path` target a boundary fn's L1 wrapper calls (issue #16
    /// additive field; `.design/boundary/ffi-boundary.md` REQ-5). `Some` ONLY on a
    /// boundary cert (`Certificate::boundary_l1`); `None` otherwise. `#[serde(default,
    /// skip_serializing_if = "Option::is_none")]` so a non-boundary cert (the golden)
    /// deserializes unchanged (R-SPEC-2). DIAGNOSTIC — the prose half of the #15
    /// audit hook (the `boundary` flag is the verdict-relevant half): EXCLUDED from
    /// `oracle_subset` (parallel to `slag_meta`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boundary_target: Option<String>,
    /// The §9 ASSURANCE SCOPE of this fn (issue #17 additive field;
    /// `.design/forge/e2e-vs-boundary.md` REQ-3). `Some(EndToEnd)` when the fn's
    /// transitive intra-file call closure reaches no `#[boundary]`/`#[slag]` fn;
    /// `Some(ToBoundary { via })` when it does (the first reached crossing). `None`
    /// only on a cert built BEFORE classification ran (the constructors below set
    /// `None`; `check::check_file_with_options` attaches the real scope via
    /// `Certificate::with_assurance_scope`). `#[serde(default, skip_serializing_if =
    /// "Option::is_none")]` so the frozen golden `conformance/sum.cert.json` (which
    /// OMITS this field) still deserializes, defaulting `None`, mirroring the
    /// `slag_meta`/`solver_profile`/`boundary_target` additive precedents (R-SPEC-2).
    ///
    /// VERDICT-RELEVANT (§9 / R-DEFER-9 — a guarantee depending on an unproven
    /// foreign body must be HONESTLY marked), so it JOINS the `oracle_subset` — but
    /// NORMALIZED to a bool (`scope_is_end_to_end`): `None` (the golden default) and
    /// `Some(EndToEnd)` (a freshly-classified pure fn) are oracle-EQUAL, so the
    /// golden `sum.cert.json` stays stable while a `ToBoundary` verdict is
    /// oracle-visible (the design's stability requirement). ORTHOGONAL to `level`
    /// (REQ-5): recorded alongside, never merged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assurance_scope: Option<AssuranceScope>,
    /// The PER-OBLIGATION ENGINE ATTRIBUTION (`.design/verified/proof-backends.md`
    /// REQ-4, increment (iii), #247): the `{engine, trust_profile}` pair recorded when
    /// a NON-DEFAULT engine (Lean) discharged this item's certification obligation —
    /// so an auditor SEES that L3-via-Lean enumerates a SMALLER trusted base ({Lean
    /// kernel + 3 axioms, EXP}) than L3-via-Verus ({Z3, Verus VC-gen, lowering
    /// theorem}). `Some` ONLY when a non-default engine discharged (the default Verus
    /// path leaves it `None`); set by `Certificate::with_engine_attribution`, consumed
    /// by `cli::run_check`'s `--engine lean` path. `#[serde(default,
    /// skip_serializing_if = "Option::is_none")]` so the frozen golden
    /// `conformance/sum.cert.json` (which OMITS it — the default Verus path never
    /// populates it) still deserializes, defaulting `None`, mirroring the
    /// `slag_meta`/`solver_profile`/`assurance_scope` additive precedents (R-SPEC-2 —
    /// the cert oracle stays byte-identical because `serde(default)` keeps the golden
    /// green: a Verus cert never gains the field). DIAGNOSTIC + verdict-orthogonal (the
    /// `Level` is unchanged — L3 still means "proven for all inputs"; the trust base is
    /// the auditor-visible refinement): EXCLUDED from `oracle_subset` (OQ-2 decided
    /// diagnostic-only so the golden stays stable; the project-min aggregate is
    /// UNCHANGED — REQ-4 "honest-min aggregation UNCHANGED").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine_attribution: Option<crate::engine::EngineAttribution>,
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
            lowered_assurance: false,
            degrade_reason: None,
            strengthening: Vec::new(),
            boundary: false,
            boundary_target: None,
            assurance_scope: None,
            engine_attribution: None,
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
            lowered_assurance: false,
            degrade_reason: None,
            strengthening: Vec::new(),
            boundary: false,
            boundary_target: None,
            assurance_scope: None,
            engine_attribution: None,
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
            lowered_assurance: false,
            degrade_reason: None,
            strengthening: Vec::new(),
            boundary: false,
            boundary_target: None,
            assurance_scope: None,
            engine_attribution: None,
        }
        .graduate_triage_clean()
    }

    /// Build a BOUNDARY-fn certificate (`.design/boundary/ffi-boundary.md` REQ-5,
    /// §9). The FFI analog of [`Certificate::slag_l1`]: a `#[boundary("crate::path")]`
    /// fn whose FOREIGN body is UNPROVEN, so it certifies at `Level::L1` (the
    /// contract enforced at the crossing — `req` before, `ens` after — by
    /// `thermite_lower::l1`'s wrapper) with `boundary: true` and the foreign
    /// `target` recorded for the #15 TCB enumeration — NOT L3 (no verus run on a
    /// foreign body). A single discharged obligation records the trusted-by-fiat
    /// fact (NOT a verus obligation). The §7.1 (a)/(b)/(c) triage STILL applies (a
    /// boundary fn with a vacuous contract is rejected — slag-adjacent: it exempts
    /// PROVING, not STATING), so the triage bools graduate to live-`false`
    /// (`graduate_triage_clean`, the slag precedent). `slag` stays `false` — a
    /// boundary fn is a distinct TCB category from a `#[slag]` block.
    pub fn boundary_l1(item: impl Into<String>, effects: Vec<String>, target: String) -> Self {
        Certificate {
            item: item.into(),
            level: Level::L1,
            solver_time_ms: 0,
            contract_quality: ContractQuality::forward_declared(),
            effects,
            slag: false,
            slag_meta: None,
            reject: None,
            obligations: vec![ObligationResult::discharged(
                "contract enforced at L1 (boundary); foreign body trusted by fiat",
            )],
            cached: false,
            solver_profile: None,
            suggested_move: None,
            lowered_assurance: false,
            degrade_reason: None,
            strengthening: Vec::new(),
            boundary: true,
            boundary_target: Some(target),
            assurance_scope: None,
            engine_attribution: None,
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
            lowered_assurance: false,
            degrade_reason: None,
            strengthening: Vec::new(),
            boundary: false,
            boundary_target: None,
            assurance_scope: None,
            engine_attribution: None,
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

    /// Stamp this certificate as an AUTOMATIC DEGRADE below L3 (issue #10;
    /// `.design/forge/degrade-ladder.md` REQ-4). Called by the ladder
    /// (`degrade::run_ladder`) on a cert achieved at L2 (kani) or L1 after a verus
    /// L3 TIMEOUT: sets the `lowered_assurance` flag `true` and records the
    /// `degrade_reason` (the `VerusTimeout` reason — "here's where I got lost").
    /// ONLY the two degrade fields change — `level`, `effects`, `obligations`, and
    /// the rest are the underlying rung's verdict, untouched. This is NEVER applied
    /// to a hard-failed cert (a counterexample): the ladder short-circuits a
    /// counterexample to a hard fail WITHOUT degrading (REQ-2 anti-cheat).
    pub fn into_degraded(mut self, reason: RejectReason) -> Self {
        self.lowered_assurance = true;
        self.degrade_reason = Some(reason);
        self
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

    /// Attach the §7 step-5 STRENGTHENING SUGGESTIONS to this certificate (#14;
    /// `.design/forge/strengthening-probes.md` REQ-4). ADVISORY: only the additive
    /// `strengthening` field and the reserved `suggested_move` headline change —
    /// `level`, `reject`, and the `oracle_subset` are UNTOUCHED, so a probe NEVER
    /// changes the verdict (a `fn` that certified L3 still certifies L3 with the
    /// same oracle subset, now carrying suggestions). The top suggestion (the first
    /// in the deterministic family order) becomes the `suggested_move` headline
    /// (§5.1 "every message is a prompt"); the full ordered list lives in
    /// `strengthening`. An EMPTY `suggestions` is a no-op (an honest absence — the
    /// `suggested_move` stays whatever it was, the list stays empty). Consumed by
    /// `check::strengthen_certificate`.
    pub fn with_strengthening(mut self, suggestions: Vec<Suggestion>) -> Self {
        if let Some(top) = suggestions.first() {
            // The headline hint (the §5.1 reserved `suggested_move` slot): the
            // top adoptable tightening. A probe NEVER overwrites a NON-`None`
            // `suggested_move` (e.g. a timeout cert's profile hint), but a probe
            // only runs on a CERTIFIED L3 item whose `suggested_move` is `None`,
            // so this is the first writer in that path.
            self.suggested_move = Some(SuggestedMove {
                kind: "strengthen-ens".to_string(),
                detail: match &top.kills_survivor {
                    Some(survivor) => format!(
                        "consider strengthening `ens` with `{}` — it holds for your body and \
                         would kill survivor `{survivor}`",
                        top.clause
                    ),
                    None => format!(
                        "consider strengthening `ens` with `{}` — it holds for your body and \
                         pins the result more tightly than the current `ens`",
                        top.clause
                    ),
                },
            });
        }
        self.strengthening = suggestions;
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

    /// Attach the §9 assurance scope to this certificate (#17;
    /// `.design/forge/e2e-vs-boundary.md` REQ-3). Returns the cert with
    /// `assurance_scope` set to the classified value. ORTHOGONAL to the verdict
    /// (REQ-5): ONLY this field changes — `level`, `reject`, `boundary`, `slag`
    /// are untouched, so a fn keeps its achieved level AND records its scope (an
    /// L3 fn whose closure crosses a boundary stays `Level::L3` + `ToBoundary`).
    /// Set by `check::check_file_with_options` after `closure::classify`.
    pub fn with_assurance_scope(mut self, scope: AssuranceScope) -> Self {
        self.assurance_scope = Some(scope);
        self
    }

    /// Attach the per-obligation ENGINE ATTRIBUTION (`.design/verified/
    /// proof-backends.md` REQ-4, increment (iii), #247). Returns the cert with
    /// `engine_attribution` set to the discharging engine's `{engine, trust_profile}`
    /// pair — recorded ONLY when a NON-DEFAULT engine (Lean) proved the item, so an
    /// auditor sees the SMALLER trusted base. ORTHOGONAL to the verdict (REQ-4 — the
    /// `Level` is unchanged; the trust base is the auditor-visible refinement): ONLY
    /// this field changes. The default Verus path NEVER calls this (the field stays
    /// `None`), so the cert oracle is byte-identical (the `serde(default)` keeps the
    /// golden green). Set by `cli::run_check`'s `--engine lean` path.
    #[must_use]
    pub fn with_engine_attribution(
        mut self,
        attribution: crate::engine::EngineAttribution,
    ) -> Self {
        self.engine_attribution = Some(attribution);
        self
    }

    /// The DETERMINISTIC, currently-producible oracle subset (REQ-3/REQ-6,
    /// `.design/forge/check.md` AC-1; ffi-boundary.md REQ-5/AC-2; e2e-vs-boundary.md
    /// REQ-3): `(item, level, effects, slag, boundary, end_to_end)`. The
    /// forward-declared `contract_quality.*` and the non-deterministic
    /// `solver_time_ms` are STRUCTURALLY excluded by being absent from this tuple.
    /// `boundary` joins because it is verdict-relevant (an L1 "to-the-boundary" is
    /// distinct from a proved/runtime L1); `boundary_target` is diagnostic and
    /// stays EXCLUDED (parallel to `slag_meta`).
    ///
    /// `end_to_end` is the §9 assurance-scope bit (#17), NORMALIZED via
    /// `scope_is_end_to_end`: `None` (the frozen golden `sum.cert.json`, which omits
    /// `assurance_scope`) and `Some(EndToEnd)` (a freshly-classified pure fn) are
    /// BOTH `true`, so the golden subset stays oracle-stable (R-SPEC-2) while a
    /// `Some(ToBoundary)` verdict reads `false` and is oracle-visible (§9 / R-DEFER-9
    /// — a to-the-boundary guarantee must be honestly distinguished). The `via`
    /// crossing name is diagnostic detail and stays EXCLUDED (parallel to
    /// `boundary_target`).
    pub fn oracle_subset(&self) -> (&str, Level, &[String], bool, bool, bool) {
        (
            &self.item,
            self.level,
            &self.effects,
            self.slag,
            self.boundary,
            scope_is_end_to_end(&self.assurance_scope),
        )
    }
}

/// The project-level assurance headline an aggregate of the per-fn certificates
/// resolves to (`.design/forge/degrade-ladder.md` REQ-6). DISTINCT from a per-fn
/// `Level`: a single hard-failed (non-certifying) function makes the WHOLE project
/// `Failed` — a rejected item is NOT a rung the min ranges over (REQ-2/REQ-6 — "a
/// non-certifying item is not a rung"). When every function certifies, the
/// headline is the min over their achieved levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "level")]
pub enum ProjectAssurance {
    /// Every function certifies; the headline is the MIN over their levels (§5.2
    /// "the whole-project assurance level is the min over functions"). The carried
    /// `Level` is the weakest function's rung.
    Certified(Level),
    /// At least one function did NOT certify (a counterexample / reject /
    /// un-discharged proof). The project does not certify at any rung — it is a
    /// FAILURE, not a lowered level (REQ-2 anti-cheat: falsity never becomes a rung).
    Failed,
}

/// The project-level assurance manifest: an AGGREGATE over the per-fn certificate
/// collection `forge check` returns (`.design/forge/degrade-ladder.md` REQ-5,
/// OQ-4 reading (b) — a render-time aggregate, NOT a separately-materialized
/// schema object). It is computed from `&[Certificate]` and carries the
/// project headline (the min-over-functions, REQ-6) plus the per-fn degrade view
/// (each fn's name, achieved level, and whether it was a lowered-assurance
/// degrade). Consumed by `cli::run_check` to display the project assurance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssuranceManifest {
    /// The project headline: the min over functions when all certify, else
    /// `Failed` (REQ-6).
    pub project: ProjectAssurance,
    /// The §9 PROJECT ASSURANCE SCOPE (issue #17;
    /// `.design/forge/e2e-vs-boundary.md` REQ-4): END-TO-END iff EVERY fn is
    /// end-to-end, else TO-THE-BOUNDARY listing the crossings. A render-time
    /// aggregate of the per-fn `Certificate::assurance_scope`, ORTHOGONAL to the
    /// `project` level headline (a project can be `Certified(L3)` AND
    /// `ToBoundary` — every fn proved its own contract while the closure crosses a
    /// foreign body).
    pub scope: ProjectScope,
    /// The per-fn degrade view in cert order: `(item, level, lowered_assurance)`.
    pub functions: Vec<FunctionAssurance>,
}

/// The project-level §9 assurance-scope claim (issue #17;
/// `.design/forge/e2e-vs-boundary.md` REQ-4). The aggregate of the per-fn
/// [`AssuranceScope`]s, ORTHOGONAL to [`ProjectAssurance`] (the level headline):
///
/// - [`ProjectScope::EndToEnd`] — EVERY fn is END-TO-END (no fn's closure reaches a
///   `#[boundary]`/`#[slag]`); the whole project is "verified, period".
/// - [`ProjectScope::ToBoundary`] — at least one fn is TO-THE-BOUNDARY; `crossings`
///   lists the reached `#[boundary]`/`#[slag]` fns (deduplicated, sorted —
///   deterministic, R-CODE-5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ProjectScope {
    /// Every fn is end-to-end: the project is "verified, period".
    EndToEnd,
    /// At least one fn reaches a crossing; `crossings` are the reached
    /// `#[boundary]`/`#[slag]` fns (sorted, deduplicated).
    ToBoundary {
        /// The `#[boundary]`/`#[slag]` fns the project's closures reach.
        crossings: Vec<String>,
    },
}

/// One function's row in the [`AssuranceManifest`] (REQ-5): its achieved level,
/// whether it certifies, and whether it was an automatic degrade.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionAssurance {
    /// The function name.
    pub item: String,
    /// The achieved assurance level.
    pub level: Level,
    /// `true` iff this item certifies (a certified rung with no `reject`).
    pub certified: bool,
    /// `true` iff this level was reached by an automatic degrade below L3 (#10).
    pub lowered_assurance: bool,
}

impl AssuranceManifest {
    /// Aggregate a per-fn certificate collection into the project-level manifest
    /// (`.design/forge/degrade-ladder.md` REQ-5/REQ-6). The headline is the MIN
    /// over functions (`Level`'s `Ord`, `L0 < L1 < L2 < L3`) when EVERY function
    /// certifies; if ANY function does NOT certify (a counterexample / reject /
    /// un-discharged proof — `cert_certifies` is `false`), the project is `Failed`
    /// (REQ-2/REQ-6: a non-certifying item is a project FAILURE, never a lowered
    /// rung). An empty collection certifies vacuously at the top rung (`L3`) — a
    /// file with no `fn` items has nothing un-proved. DETERMINISTIC (REQ-7): a pure
    /// function of the cert collection, no wall-clock / ordering nondeterminism.
    pub fn aggregate(certs: &[Certificate]) -> Self {
        let functions: Vec<FunctionAssurance> = certs
            .iter()
            .map(|c| FunctionAssurance {
                item: c.item.clone(),
                level: c.level,
                certified: cert_certifies(c),
                lowered_assurance: c.lowered_assurance,
            })
            .collect();
        let project = if functions.iter().any(|f| !f.certified) {
            // REQ-2/REQ-6: any non-certifying function caps the project at FAILURE,
            // never a lowered level — falsity is not a rung.
            ProjectAssurance::Failed
        } else {
            // Min over the certified functions' levels (REQ-6). Empty → vacuous L3.
            let min = functions.iter().map(|f| f.level).min().unwrap_or(Level::L3);
            ProjectAssurance::Certified(min)
        };
        let scope = project_scope(certs);
        AssuranceManifest {
            project,
            scope,
            functions,
        }
    }
}

/// Aggregate the per-fn [`AssuranceScope`]s into the §9 PROJECT scope claim (issue
/// #17; `.design/forge/e2e-vs-boundary.md` REQ-4): [`ProjectScope::EndToEnd`] iff
/// EVERY cert is end-to-end (a `None` scope reads end-to-end — the golden default),
/// else [`ProjectScope::ToBoundary`] listing the reached crossings (the `via` fns,
/// deduplicated + sorted — DETERMINISTIC, R-CODE-5). An empty collection is
/// vacuously END-TO-END (nothing crosses a boundary). ORTHOGONAL to the level
/// headline: a project can be `Certified(L3)` AND `ToBoundary`.
fn project_scope(certs: &[Certificate]) -> ProjectScope {
    // BTreeSet → sorted + deduplicated crossings (deterministic).
    let mut crossings: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for cert in certs {
        if let Some(AssuranceScope::ToBoundary { via }) = &cert.assurance_scope {
            crossings.insert(via.clone());
        }
    }
    if crossings.is_empty() {
        ProjectScope::EndToEnd
    } else {
        ProjectScope::ToBoundary {
            crossings: crossings.into_iter().collect(),
        }
    }
}

/// `true` iff a certificate represents a CERTIFIED item: no reject cause and a
/// certified assurance rung (`L3` proved, `L2` bounded, or `L1` runtime/slag).
/// `L0` (a triage / counterexample / timeout reject, or an un-discharged proof)
/// is NOT certified. Shared by the assurance aggregate (REQ-6) and `cli`'s
/// exit-code path (so the project headline and the exit code agree on what
/// "certifies").
pub fn cert_certifies(cert: &Certificate) -> bool {
    cert.reject.is_none() && matches!(cert.level, Level::L3 | Level::L2 | Level::L1)
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
        // The #106 terminal-control atom (`fx term` → the `ioctl` seccomp grant,
        // runtime-sandbox.md REQ-7). A bare atom like `alloc`/`time`.
        Effect::Term => "term".to_string(),
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

    // #10 (degrade-ladder REQ-6): Level's derived Ord is the ladder ordering
    // L0 < L1 < L2 < L3. Expected from the design doc's REQ-6 (R-CHAR-3).
    #[test]
    fn level_ord_is_the_ladder_ordering() {
        assert!(Level::L0 < Level::L1);
        assert!(Level::L1 < Level::L2);
        assert!(Level::L2 < Level::L3);
        // min over a mixed set is the weakest rung.
        let levels = [Level::L3, Level::L1, Level::L2];
        assert_eq!(levels.iter().min().copied(), Some(Level::L1));
    }

    // #10 (degrade-ladder REQ-4): into_degraded stamps the lowered_assurance flag +
    // the degrade reason, leaving the level + obligations untouched.
    #[test]
    fn into_degraded_stamps_flag_and_reason() {
        let base = Certificate::new("g", Level::L2, vec!["pure".to_string()], 0, vec![]);
        assert!(!base.lowered_assurance);
        let reason = RejectReason {
            cause: "VerusTimeout".to_string(),
            detail: "rlimit exhausted".to_string(),
        };
        let degraded = base.clone().into_degraded(reason);
        assert!(degraded.lowered_assurance);
        assert_eq!(
            degraded.degrade_reason.as_ref().map(|r| r.cause.as_str()),
            Some("VerusTimeout")
        );
        // The achieved level is untouched — into_degraded qualifies it, not mutates.
        assert_eq!(degraded.level, Level::L2);
    }

    // #10 (degrade-ladder AC-6, R-SPEC-2): the lowered_assurance / degrade_reason
    // fields are ADDITIVE — absent on a plain cert and on the golden, so the frozen
    // golden `sum.cert.json` still deserializes. A non-degraded cert serializes
    // lowered_assurance:false and omits degrade_reason.
    #[test]
    fn degrade_fields_are_additive() {
        let cert = Certificate::new("f", Level::L3, vec!["pure".to_string()], 0, vec![]);
        assert!(!cert.lowered_assurance);
        assert!(cert.degrade_reason.is_none());
        let json = serialize(&cert);
        assert!(
            !json.contains("degrade_reason"),
            "None degrade_reason is omitted:\n{json}"
        );
        // The frozen golden cert (no #10 fields) deserializes, defaulting the flag.
        let golden_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("conformance")
            .join("sum.cert.json");
        if let Ok(src) = std::fs::read_to_string(&golden_path) {
            let golden: Result<Certificate, _> = serde_json::from_str(&src);
            assert!(
                golden.is_ok(),
                "golden deserializes with #10 additive fields: {golden:?}"
            );
            if let Ok(g) = golden {
                assert!(
                    !g.lowered_assurance,
                    "golden defaults lowered_assurance false"
                );
                assert!(g.degrade_reason.is_none());
            }
        }
    }

    // #10 (degrade-ladder REQ-5/REQ-6 / AC-5): the assurance aggregate headline is
    // the MIN over functions. {L3,L2,L1} → Certified(L1). Expected: Level's Ord
    // (REQ-6), not forge's output (R-CHAR-3).
    #[test]
    fn aggregate_headline_is_min_over_functions() {
        let certs = vec![
            Certificate::new("f", Level::L3, vec!["pure".to_string()], 0, vec![]),
            Certificate::new("g", Level::L2, vec!["pure".to_string()], 0, vec![]),
            Certificate::new("h", Level::L1, vec!["pure".to_string()], 0, vec![]),
        ];
        let m = AssuranceManifest::aggregate(&certs);
        assert_eq!(m.project, ProjectAssurance::Certified(Level::L1));
        assert_eq!(m.functions.len(), 3);
    }

    // #10 (REQ-2/REQ-6 / AC-5): a single non-certifying (counterexample / reject)
    // fn caps the whole project at FAILURE — never a lowered rung (falsity is not a
    // rung). Expected from REQ-6 (R-CHAR-3).
    #[test]
    fn aggregate_hard_fail_is_project_failure() {
        let reason = RejectReason {
            cause: "EnsIsTrivial".to_string(),
            detail: "ens#0 is true".to_string(),
        };
        let certs = vec![
            Certificate::new("f", Level::L3, vec!["pure".to_string()], 0, vec![]),
            Certificate::rejected("bad", vec!["pure".to_string()], false, reason),
        ];
        let m = AssuranceManifest::aggregate(&certs);
        assert_eq!(m.project, ProjectAssurance::Failed);
        // The rejected fn is recorded as non-certified in its row.
        let bad = m.functions.iter().find(|r| r.item == "bad");
        assert_eq!(bad.map(|r| r.certified), Some(false));
    }

    // #10 (REQ-6): an empty cert collection certifies vacuously at the top rung —
    // a file with no fn has nothing un-proved.
    #[test]
    fn aggregate_empty_is_vacuous_l3() {
        let m = AssuranceManifest::aggregate(&[]);
        assert_eq!(m.project, ProjectAssurance::Certified(Level::L3));
        assert!(m.functions.is_empty());
    }

    // #10: cert_certifies treats L3/L2/L1 (no reject) as certified and L0 / a
    // reject as not — the shared predicate the aggregate + cli exit code use.
    #[test]
    fn cert_certifies_recognizes_the_certified_rungs() {
        assert!(cert_certifies(&Certificate::new(
            "a",
            Level::L3,
            vec![],
            0,
            vec![]
        )));
        assert!(cert_certifies(&Certificate::new(
            "b",
            Level::L2,
            vec![],
            0,
            vec![]
        )));
        assert!(cert_certifies(&Certificate::new(
            "c",
            Level::L1,
            vec![],
            0,
            vec![]
        )));
        assert!(!cert_certifies(&Certificate::new(
            "d",
            Level::L0,
            vec![],
            0,
            vec![]
        )));
        let reason = RejectReason {
            cause: "WeakContract".to_string(),
            detail: "x".to_string(),
        };
        // An L3 cert WITH a reject (e.g. a WeakContract reject built on L0) does not
        // certify — the reject dominates.
        assert!(!cert_certifies(&Certificate::rejected(
            "e",
            vec![],
            false,
            reason
        )));
    }

    // #17 (e2e-vs-boundary REQ-3, R-SPEC-2): `assurance_scope` is ADDITIVE — absent
    // on a plain cert and on the golden, defaulting `None`, so the frozen golden
    // `conformance/sum.cert.json` still deserializes. The oracle NORMALIZATION makes
    // `None` (golden) oracle-equal to `Some(EndToEnd)` (a classified pure fn), so the
    // golden subset stays stable; a `Some(ToBoundary)` is oracle-DISTINCT (verdict-
    // relevant, §9 / R-DEFER-9). Expected behavior traces to the design REQ-3 + the
    // Verification section (R-CHAR-3), not forge output.
    #[test]
    fn assurance_scope_is_additive_normalized_and_golden_stable() {
        let plain = Certificate::new("f", Level::L3, vec!["pure".to_string()], 0, vec![]);
        assert!(plain.assurance_scope.is_none(), "additive: defaults None");
        let json = serialize(&plain);
        assert!(
            !json.contains("assurance_scope"),
            "None assurance_scope is omitted:\n{json}"
        );

        // None and Some(EndToEnd) are ORACLE-EQUAL (the normalization keeping the
        // golden stable).
        let e2e = plain.clone().with_assurance_scope(AssuranceScope::EndToEnd);
        assert!(
            oracle_eq(&plain, &e2e),
            "None and Some(EndToEnd) must be oracle-equal (golden stability)"
        );

        // Some(ToBoundary) is ORACLE-DISTINCT from end-to-end (verdict-relevant).
        let to_boundary = plain
            .clone()
            .with_assurance_scope(AssuranceScope::ToBoundary {
                via: "ext_id".to_string(),
            });
        assert!(
            !oracle_eq(&plain, &to_boundary),
            "a to-the-boundary scope must be oracle-visible (§9 / R-DEFER-9)"
        );
        // The achieved level is UNTOUCHED — scope ⊥ level (REQ-5).
        assert_eq!(to_boundary.level, Level::L3, "scope is orthogonal to level");

        // The frozen golden `sum.cert.json` (no assurance_scope) deserializes,
        // defaulting None, and is oracle-equal to a classified EndToEnd `sum`.
        let golden_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("conformance")
            .join("sum.cert.json");
        if let Ok(src) = std::fs::read_to_string(&golden_path) {
            let golden: Result<Certificate, _> = serde_json::from_str(&src);
            assert!(
                golden.is_ok(),
                "golden deserializes with the #17 additive field: {golden:?}"
            );
            if let Ok(g) = golden {
                assert!(g.assurance_scope.is_none(), "golden omits assurance_scope");
                let classified = g.clone().with_assurance_scope(AssuranceScope::EndToEnd);
                assert!(
                    oracle_eq(&g, &classified),
                    "the golden subset is stable once `sum` is classified EndToEnd"
                );
            }
        }
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
            Effect::Term,
        ]);
        assert_eq!(
            effects_of(&row),
            vec![
                "read(x)", "write(y)", "net(z)", "alloc", "time", "rand", "panic", "diverge",
                "term"
            ]
        );
    }

    // =======================================================================
    // REQ-10 (Target D) — the Verus-anchor for the project LEVEL AGGREGATION min
    // (`.design/verified/self-verification.md` REQ-10 / AC-10c, mechanism (c)).
    //
    // PLACEMENT DEVIATION (Option B, orchestrator-authorized): the design doc names
    // a `manifest::verus_anchor` block (forge is binary-only, so an external test
    // cannot reach `AssuranceManifest::aggregate`/`Certificate`). Nested in the
    // existing `tests` module so the anti-pattern gate's `#[cfg(test)]` exemption
    // covers it. `thermite-verified` is a forge DEV-dependency.
    //
    // AC-10c — the EXHAUSTIVE `Level`-list equivalence: enumerate ALL per-fn `Level`
    // lists up to length 4 over the 4 levels (plus the empty list) and assert, for
    // each, that `AssuranceManifest::aggregate(certs).project` agrees with the VERUS-
    // PROVED fold-min `thermite_verified::aggregate_level`. The production `aggregate`
    // splits two ORTHOGONAL axes (REQ-2/REQ-6): a NON-certifying fn (a plain `L0`
    // cert carries no rung — `cert_certifies` is false) caps the project at `Failed`,
    // independent of the min; when EVERY fn certifies (the list is empty or over the
    // certifying rungs L1/L2/L3) the headline is `Certified(min)`. The anchor binds
    // the LEVEL MIN (D's §5.2 no-over-claim story) on the all-certifying lists —
    // `Certified(proved_min)` AND headline ≤ every level — and ALSO confirms the
    // orthogonal `Failed`-cap fires IFF an `L0` is present (so the enumeration is
    // exhaustive over the full 4-level alphabet, not just the certifying subset).
    // Expected = the proved fold-min (R-CHAR-3, never forge's own output) — binding
    // the production min to the proved D1 (≤ every fn) + D2 (attained == the min).
    // =======================================================================
    mod verus_anchor {
        use super::*;
        use thermite_verified::{aggregate_level, Level as VLevel};

        /// The 4 production levels in rank order (`L0 < L1 < L2 < L3`), each paired
        /// with the verus-proved `thermite_verified::Level` mirror. The pairing IS
        /// the representation bridge the anchor binds (R-CHAR-3 — the design's
        /// lattice, not forge output).
        const LEVELS: &[(Level, VLevel)] = &[
            (Level::L0, VLevel::L0),
            (Level::L1, VLevel::L1),
            (Level::L2, VLevel::L2),
            (Level::L3, VLevel::L3),
        ];

        /// Map a proved `thermite_verified::Level` back to the production `Level`
        /// via the lattice bridge. Total over the 4-level alphabet.
        fn prod_of(v: VLevel) -> Level {
            match v {
                VLevel::L0 => Level::L0,
                VLevel::L1 => Level::L1,
                VLevel::L2 => Level::L2,
                VLevel::L3 => Level::L3,
            }
        }

        /// Build a per-fn cert list from a production-level list. Each cert is
        /// `Certificate::new` (no reject); a plain `L0` cert does NOT certify
        /// (`cert_certifies` is false for `L0`), so a list containing `L0` exercises
        /// the orthogonal `Failed`-cap path, while a list over the certifying rungs
        /// (L1/L2/L3) exercises the min-over-functions path the D anchor binds.
        fn level_certs(levels: &[Level]) -> Vec<Certificate> {
            levels
                .iter()
                .enumerate()
                .map(|(i, &lvl)| {
                    Certificate::new(format!("f{i}"), lvl, vec!["pure".to_string()], 0, vec![])
                })
                .collect()
        }

        /// One enumerated `(Level, VLevel)` list element (the production level
        /// paired with its verus mirror).
        type LevelPair = (Level, VLevel);

        /// Recursively enumerate every [`LevelPair`] list up to `max_len`
        /// (inclusive, plus the empty list) and call `visit` on each. The 4-level
        /// alphabet over lengths 0..=4 is `1 + 4 + 16 + 64 + 256 = 341` lists.
        fn for_each_list(
            max_len: usize,
            acc: &mut Vec<LevelPair>,
            visit: &mut dyn FnMut(&[LevelPair]),
        ) {
            visit(acc);
            if acc.len() == max_len {
                return;
            }
            for &pair in LEVELS {
                acc.push(pair);
                for_each_list(max_len, acc, visit);
                acc.pop();
            }
        }

        /// AC-10c — over EVERY `Level` list (length 0..=4) the production
        /// `AssuranceManifest::aggregate` project headline agrees with the VERUS-
        /// PROVED `aggregate_level`: on an all-certifying list (empty or over
        /// L1/L2/L3) it is `Certified(proved_min)` AND ≤ every per-fn level (the
        /// §5.2 / R-DEFER-9 over-claim bound, proved D1); on a list with an `L0`
        /// (non-certifying) it is the orthogonal `Failed`-cap. 0 mismatches over the
        /// full finite domain.
        #[test]
        fn aggregate_project_min_matches_proved_aggregate_level_over_all_level_lists() {
            let mut checked = 0usize;
            let mut min_anchored = 0usize;
            let mut acc: Vec<(Level, VLevel)> = Vec::new();
            let mut visit = |list: &[(Level, VLevel)]| {
                let prod_levels: Vec<Level> = list.iter().map(|&(p, _)| p).collect();
                let v_levels: Vec<VLevel> = list.iter().map(|&(_, v)| v).collect();

                // R-CHAR-3: the EXPECTED min is the verus-proved fold, mapped back to
                // a production `Level` via the lattice bridge.
                let expected_min = prod_of(aggregate_level(&v_levels));

                let certs = level_certs(&prod_levels);
                let m = AssuranceManifest::aggregate(&certs);

                if prod_levels.contains(&Level::L0) {
                    // ORTHOGONAL `Failed`-cap: a non-certifying (L0) fn caps the
                    // project regardless of the min (REQ-2/REQ-6).
                    assert_eq!(
                        m.project,
                        ProjectAssurance::Failed,
                        "an L0 fn must cap the project at Failed for {prod_levels:?}"
                    );
                } else {
                    // The min-over-functions path the D anchor binds.
                    assert_eq!(
                        m.project,
                        ProjectAssurance::Certified(expected_min),
                        "aggregate project min != proved aggregate_level for {prod_levels:?}"
                    );
                    // D1 OBSERVABLE: the headline is ≤ every per-fn level.
                    for &lvl in &prod_levels {
                        assert!(
                            expected_min <= lvl,
                            "project min {expected_min:?} must be <= every fn level (got {lvl:?})"
                        );
                    }
                    min_anchored += 1;
                }
                checked += 1;
            };
            for_each_list(4, &mut acc, &mut visit);
            // 1 + 4 + 16 + 64 + 256 = 341 lists over the 4-level alphabet (0..=4).
            assert_eq!(checked, 341, "all Level lists up to length 4 enumerated");
            // The min-anchored subset (no L0) is 1 + 3 + 9 + 27 + 81 = 121 lists.
            assert_eq!(
                min_anchored, 121,
                "the all-certifying (no-L0) lists bind the proved min"
            );
        }
    }
}
