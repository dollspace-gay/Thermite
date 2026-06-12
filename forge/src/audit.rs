//! `forge/src/audit.rs` — the AUDIT MANIFEST v1, the project-level TRUST
//! DELIVERABLE (`thermite-design.md` §6/§8/§9, issue #15). `thermite-design.md`
//! §6: "The certificate attached to a build artifact lists every function's
//! level, every `#[slag]` block, and the contract-quality scores from §7. This
//! manifest **is** the deliverable's trust statement." This module is that
//! aggregate manifest — a STABLE, versioned project-level document
//! ([`AuditManifest`], `manifest_version: "v1"`) emitted by `forge audit <file>`.
//!
//! Governing design: `.design/forge/audit-manifest.md`.
//!
//! The manifest is a PURE PROJECTION of the per-fn [`Certificate`] collection
//! `forge check` already produced (`manifest.rs`), the project
//! [`AssuranceManifest`] aggregate (`manifest.rs`, #10/#17), and the toolchain
//! identity (verus version + thermite version). It computes NO verdict — it never
//! re-runs verus, re-scores mutants, or re-classifies a closure (REQ-4). Its
//! centerpiece is the §9 ENUMERABLE TRUSTED COMPUTING BASE ([`Tcb`]): exactly
//! (every `#[slag]` block ∪ every `#[boundary]` contract ∪ the toolchain itself).
//! `grep slag` over a codebase and this TCB section are the same complete
//! inventory of fiat-trusted code (§8) — nothing fiat-trusted is omitted
//! (R-DEFER-9).
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (AuditManifest v1 schema + version tag) | SHIPPED | `struct AuditManifest { manifest_version, functions, project_assurance, tcb }`; `manifest_version` defaults to [`MANIFEST_VERSION`] (`"v1"`); additive evolution via `#[serde(default)]` on `manifest_version` (the per-cert precedent). Built by `AuditManifest::from_certificates`; consumed by `cli::run_audit` (`--json` + human render). |
//! | REQ-2 (`forge audit <file>` command) | SHIPPED | `cli::run_audit` runs `check::check_file(file)` (the default-config entry, the SAME pipeline `forge check` runs at `CheckOptions::default` — no extra verification, OQ-3), parses the file once for the boundary contracts, builds the manifest via `AuditManifest::from_certificates`, and emits `--json` or the human summary. Dispatched by `cli::parse_args`'s `audit` verb. |
//! | REQ-3 (TCB = slag ∪ boundary ∪ toolchain) | SHIPPED | `Tcb::from_certificates` enumerates EVERY `cert.slag` fn (`SlagBlock` with reason/owner/review from `Certificate::slag_meta`), EVERY `cert.boundary` fn (`BoundaryContract` with `boundary_target` + the `req`/`ens`/`fx` looked up in the parsed program), and the [`Toolchain`] identity (always present — the irreducible residue). |
//! | REQ-4 (aggregation, never re-derivation) | SHIPPED | `AuditManifest::from_certificates` reads ONLY the cert collection + `AssuranceManifest::aggregate(&certs)` + the two version strings + the parsed program (for boundary contract text); it owns no prover invocation. |
//! | REQ-5 (project assurance embedded) | SHIPPED | `AuditManifest.project_assurance: ProjectAssuranceSection` embeds `AssuranceManifest::aggregate` — the `ProjectAssurance` headline, the `ProjectScope`, and the lowered-assurance fn list (from `FunctionAssurance.lowered_assurance`). |
//! | REQ-6 (determinism) | SHIPPED | the manifest is a pure function of the cert collection + program + the two pinned version strings; no wall-clock, no unordered iteration (`functions` in cert/source order; `Tcb` lists in source order). The non-deterministic `solver_time_ms` is structurally absent; the version-sensitive `mutants_killed`/`survivor` are carried in `FunctionRow.contract_quality` (shape-asserted by the oracle, not the ratio — OQ-2). |
//! | REQ-7 (#274 — `lean_fragment` membership section) | SHIPPED | `struct LeanFragment { functions: Vec<LeanFragmentRow> }` is the additive fourth section on `AuditManifest` (`#[serde(default)]`); `LeanFragment::from_certificates` builds one [`LeanFragmentRow`] per cert in source order via `LeanFragmentRow::probe`. `manifest_version` stays `"v1"`. Consumer: `AuditManifest::from_certificates` (the audit assembly) + `cli::render_audit`'s `lean fragment:` section. Oracle: `audit_conformance.rs::lean_fragment_sum`/`lean_fragment_tier_auto`/`lean_fragment_tier_interactive` (one row per fn, source order). |
//! | REQ-8 (#274 — probe = shipped dry-run export, side-effect-free) | SHIPPED | `LeanFragmentRow::probe` mints the #226 CONTRACT obligation via the shipped `check::contract_obligation` seam (a `pub(crate)` re-export of `mint_item_obligations(...).contract` — NO closure fork) and dry-runs `lean_export::export_item`; `export_item` is fs/process/env-free (the lake/scratch side effects live downstream in `LeanEngine::discharge`, never reached). Oracle: `probe_agrees_with_direct_export_item` (the row ≡ direct `export_item` result — AC-9) + `lean_fragment_present_without_lake` (AC-10, no lean toolchain). |
//! | REQ-9 (#274 — refusal classes surfaced verbatim) | SHIPPED | a non-exportable [`LeanFragmentRow`] carries `refusal: Some(LeanRefusal { class, reason })`; `class` is the stable `ExportRefusal` variant name (`refusal_class_name`, a total match over the post-(v) inventory) and `reason` is the verbatim `Display`. Oracle: `lean_fragment_refusal_optres`/`_loop`/`_boundary` (verbatim `class` + `reason` across `OptResResult`/`LoopBody`/`NotPureContract`) + `probe_sum_th_refusals_are_hand_traced` (the hand-traced `OutOfFragment` reasons for `sum`/`spec_sum`). |
//! | REQ-10 (#274 — informational only, zero default-path byte impact) | SHIPPED | the section gates nothing — `cli::run_audit`'s exit code keys on `project_assurance` ONLY (unchanged); the `Certificate` schema is untouched (`engine_attribution` stays `None` on the default path); `#[serde(default)]` on `lean_fragment` keeps a pre-amendment v1 document parsing. Oracle: `pre_amendment_v1_deserializes_into_typed_manifest` (AC-11 additive) + the existing `corpus_empty_tcb`/`slag_boundary_tcb` exit codes unchanged. |

use std::process::Command;

use serde::{Deserialize, Serialize};
use thermite_syntax::{Contract, EffectRow, Item, Program};

use crate::cli::ForgeError;
use crate::lean_export::{self, ExportRefusal};
use crate::manifest::{
    effects_of, AssuranceManifest, AssuranceScope, Certificate, ContractQuality, Level,
    ProjectAssurance, ProjectScope,
};

/// The stable format tag for the v1 audit manifest schema (REQ-1, R-SPEC-2). A
/// downstream consumer pins this and evolves the format ADDITIVELY (a new field
/// must `#[serde(default)]` so a v1 document keeps deserializing — the per-cert
/// `Certificate` additive-field precedent).
pub const MANIFEST_VERSION: &str = "v1";

/// The PROJECT-LEVEL audit manifest v1 — the §6/§8/§9 TRUST DELIVERABLE (REQ-1).
///
/// A single stable, versioned document aggregating the per-fn certificates
/// `forge check` produced. Three sections:
///
/// - [`AuditManifest::functions`] — the per-fn verdict-and-trust rows.
/// - [`AuditManifest::project_assurance`] — the project headline (#10/#17).
/// - [`AuditManifest::tcb`] — the §9 enumerable trusted computing base.
///
/// PURE PROJECTION (REQ-4): built by [`AuditManifest::from_certificates`] from a
/// settled cert collection; it re-derives no verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditManifest {
    /// The stable format tag (`"v1"`, REQ-1). `#[serde(default)]` (defaulting to
    /// [`MANIFEST_VERSION`]) so a future additive field cannot break a v1 reader.
    #[serde(default = "default_manifest_version")]
    pub manifest_version: String,
    /// The per-function rows, one per checked item in source order (REQ-1).
    pub functions: Vec<FunctionRow>,
    /// The project-level trust headline embedding [`AssuranceManifest`] (REQ-5).
    pub project_assurance: ProjectAssuranceSection,
    /// The §9 enumerable trusted computing base (REQ-3) — the manifest centerpiece.
    pub tcb: Tcb,
    /// The #274 LEAN-FRAGMENT MEMBERSHIP section (REQ-7) — one informational row per
    /// [`AuditManifest::functions`] row answering "would `--engine lean` attempt this
    /// item, and if not, what is the structured refusal". `#[serde(default)]` so a
    /// pre-amendment v1 document (no `lean_fragment` key) still deserializes (AC-5/
    /// AC-11 additive discipline; `manifest_version` stays `"v1"`). The section gates
    /// nothing — it changes no exit code and alters no verdict (REQ-10).
    #[serde(default)]
    pub lean_fragment: LeanFragment,
}

/// The `manifest_version` serde default (REQ-1): a v1 document that omits the tag
/// deserializes as [`MANIFEST_VERSION`].
fn default_manifest_version() -> String {
    MANIFEST_VERSION.to_string()
}

/// One function's row in the audit manifest (REQ-1) — the verdict-and-trust-
/// relevant PROJECTION of that fn's [`Certificate`]. A pure copy of cert fields;
/// no recomputation (REQ-4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionRow {
    /// The item name.
    pub name: String,
    /// The achieved assurance level (`L0..L3`).
    pub level: Level,
    /// The §9 assurance scope (end-to-end vs to-the-boundary), from
    /// `Certificate::assurance_scope`. `None` reads as end-to-end (the golden
    /// default; mirrors the cert field), `#[serde(skip_serializing_if)]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assurance_scope: Option<AssuranceScope>,
    /// The §7 contract-quality battery block (presence/shape asserted by the
    /// oracle; the version-sensitive `mutants_killed`/`survivor` ratio is NOT —
    /// OQ-2). A copy of `Certificate::contract_quality`.
    pub contract_quality: ContractQuality,
    /// The §8 fiat-trust flag — `true` iff this fn is a valid `#[slag]` block.
    pub slag: bool,
    /// The §9 FFI-crossing flag — `true` iff this fn is a `#[boundary]` fn.
    pub boundary: bool,
    /// The foreign `crate::path` a boundary fn's L1 wrapper calls; `Some` only
    /// when `boundary` is `true`. `#[serde(skip_serializing_if)]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boundary_target: Option<String>,
}

impl FunctionRow {
    /// Project one [`Certificate`] to its audit row (REQ-1, REQ-4) — a pure copy.
    fn from_certificate(cert: &Certificate) -> Self {
        FunctionRow {
            name: cert.item.clone(),
            level: cert.level,
            assurance_scope: cert.assurance_scope.clone(),
            contract_quality: cert.contract_quality.clone(),
            slag: cert.slag,
            boundary: cert.boundary,
            boundary_target: cert.boundary_target.clone(),
        }
    }
}

/// The project-level trust headline (REQ-5) — the embedded [`AssuranceManifest`]
/// aggregate (#10/#17). The min-over-functions level, the §9 project scope, and
/// the lowered-assurance fn list (so a reader sees proved vs degraded levels).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectAssuranceSection {
    /// The project headline: `Certified(min)` when every fn certifies, else
    /// `Failed` (§5.2). The embedded `manifest::ProjectAssurance`.
    pub level: ProjectAssurance,
    /// The §9 project scope: END-TO-END iff every fn is, else TO-THE-BOUNDARY
    /// listing the reached crossings. The embedded `manifest::ProjectScope`.
    pub scope: ProjectScope,
    /// The fns reached by an automatic degrade below L3 (#10) — the names whose
    /// level was lowered, not proved. Empty for a project that never degraded.
    /// Source order (deterministic, REQ-6).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lowered_assurance: Vec<String>,
}

impl ProjectAssuranceSection {
    /// Embed an [`AssuranceManifest`] aggregate into the manifest's project
    /// section (REQ-5) — a pure projection of its headline, scope, and the
    /// degraded-fn names.
    fn from_assurance(assurance: &AssuranceManifest) -> Self {
        let lowered_assurance = assurance
            .functions
            .iter()
            .filter(|f| f.lowered_assurance)
            .map(|f| f.item.clone())
            .collect();
        ProjectAssuranceSection {
            level: assurance.project,
            scope: assurance.scope.clone(),
            lowered_assurance,
        }
    }
}

/// The §9 ENUMERABLE TRUSTED COMPUTING BASE (REQ-3) — the manifest centerpiece
/// and the R-DEFER-9 honesty surface. `thermite-design.md` §9: the TCB is
/// "exactly (slag blocks ∪ boundary contracts ∪ the toolchain itself)". For a
/// pure-Thermite project the slag and boundary lists are EMPTY and only the
/// [`Toolchain`] remains — the §9 "verified, period" state, mechanically
/// witnessed (the irreducible base every artifact trusts).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tcb {
    /// Every `#[slag]` block: name + its §8 mandatory reason/owner/review.
    pub slag_blocks: Vec<SlagBlock>,
    /// Every `#[boundary]` contract: name + foreign target + enforced req/ens/fx.
    pub boundary_contracts: Vec<BoundaryContract>,
    /// The toolchain identity — ALWAYS present (the irreducible residue).
    pub toolchain: Toolchain,
}

impl Tcb {
    /// Enumerate the §9 TCB from the cert collection + parsed program + toolchain
    /// identity (REQ-3, REQ-4). Keys on the per-fn `slag`/`boundary` cert flags
    /// (set by `Certificate::slag_l1`/`boundary_l1`) and their metadata — never on
    /// re-parsing or re-classifying. EVERY fiat-trusted fn appears: a `cert.slag`
    /// becomes a [`SlagBlock`], a `cert.boundary` a [`BoundaryContract`]. The
    /// enforced `req`/`ens`/`fx` of a boundary contract is looked up in `program`
    /// (the cert carries only the target). Source order (deterministic, REQ-6).
    fn from_certificates(certs: &[Certificate], program: &Program, toolchain: Toolchain) -> Self {
        let mut slag_blocks = Vec::new();
        let mut boundary_contracts = Vec::new();
        for cert in certs {
            if cert.slag {
                // The §8 justification is the cert's `slag_meta` (validated
                // present + non-empty by `slag::validate` before `slag_l1`). A
                // valid slag cert always carries it; an absent one is recorded as
                // an explicit "<unspecified>" rather than dropped (R-DEFER-9 — the
                // block still appears in the TCB even if metadata were missing).
                let (reason, owner, review) = match &cert.slag_meta {
                    Some(meta) => (meta.reason.clone(), meta.owner.clone(), meta.review.clone()),
                    None => (
                        "<unspecified>".to_string(),
                        "<unspecified>".to_string(),
                        "<unspecified>".to_string(),
                    ),
                };
                slag_blocks.push(SlagBlock {
                    name: cert.item.clone(),
                    reason,
                    owner,
                    review,
                });
            }
            if cert.boundary {
                let target = cert.boundary_target.clone().unwrap_or_default();
                let contract = lookup_contract(program, &cert.item);
                boundary_contracts.push(BoundaryContract {
                    name: cert.item.clone(),
                    target,
                    req: contract.as_ref().map(|c| c.req.text.clone()),
                    ens: contract
                        .as_ref()
                        .map(|c| c.ens.iter().map(|cl| cl.text.clone()).collect())
                        .unwrap_or_default(),
                    fx: contract
                        .as_ref()
                        .map(|c| effects_of(&c.fx))
                        .unwrap_or_else(|| effects_of(&EffectRow::Pure)),
                });
            }
        }
        Tcb {
            slag_blocks,
            boundary_contracts,
            toolchain,
        }
    }
}

/// One `#[slag]` block in the §9 TCB (REQ-3) — a fiat-trusted body. Carries the
/// §8 mandatory justification (reason/owner/review) from `Certificate::slag_meta`
/// so a reviewer can audit the trust grant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlagBlock {
    /// The fn name.
    pub name: String,
    /// Why the body is fiat-trusted (§8).
    pub reason: String,
    /// The accountable owner (§8).
    pub owner: String,
    /// The review status / requirement (§8).
    pub review: String,
}

/// One `#[boundary]` contract in the §9 TCB (REQ-3) — a foreign (unproven) body
/// whose Thermite contract is enforced at the crossing (L1). Carries the foreign
/// `target` and the enforced `req`/`ens`/`fx` (§9 per-function contracts).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundaryContract {
    /// The fn name.
    pub name: String,
    /// The foreign `crate::path` the L1 wrapper calls.
    pub target: String,
    /// The enforced precondition text (`req`), when resolvable from the program.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub req: Option<String>,
    /// The enforced postcondition clauses (`ens`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ens: Vec<String>,
    /// The declared effect row (`fx`) as tokens (e.g. `["pure"]`).
    pub fx: Vec<String>,
}

/// The TOOLCHAIN identity — the irreducible §9 TCB residue (REQ-3). Every
/// artifact trusts the prover that produced its certificates; omitting this would
/// make a pure project's TCB falsely appear empty (`audit-manifest.md` "Why the
/// toolchain identity is part of the TCB"). The two strings are the same the
/// proof cache keys on, so the TCB identity and the cache provenance agree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Toolchain {
    /// The `verus` version (the SMT prover that discharged the L3 obligations).
    pub verus: String,
    /// The `thermite`/`forge` version (the toolchain that lowered + drove the
    /// proofs). `env!("CARGO_PKG_VERSION")` — deterministic at compile time.
    pub thermite: String,
}

impl Toolchain {
    /// The thermite/forge version — the crate version at compile time (R-CODE-5,
    /// no wall-clock). Identical to `check::THERMITE_VERSION` (the same
    /// `CARGO_PKG_VERSION` the proof cache keys on).
    pub const THERMITE_VERSION: &'static str = env!("CARGO_PKG_VERSION");

    /// Build the toolchain identity from a resolved verus version string (REQ-3).
    /// The caller (`cli::run_audit`) sources the verus version deterministically
    /// (the `VERUS_VERSION` pin, else `verus --version` — the same order
    /// `check::resolve_verus_version` uses for the proof cache). The thermite
    /// version is the compile-time crate version.
    pub fn new(verus: impl Into<String>) -> Self {
        Toolchain {
            verus: verus.into(),
            thermite: Self::THERMITE_VERSION.to_string(),
        }
    }
}

/// Resolve the `verus` version string for the toolchain identity (REQ-3,
/// R-CODE-5). The DETERMINISTIC sourcing order mirrors the proof cache's
/// `check::resolve_verus_version` so the TCB toolchain identity and the cache
/// provenance agree (`audit-manifest.md` "Why the toolchain identity is part of
/// the TCB"):
///
/// 1. the `VERUS_VERSION` env var when set + non-empty (the pinned/CI/hermetic-
///    test override — the same seam the cache uses so a pinned version makes the
///    corpus manifest reproducible);
/// 2. otherwise `verus --version` stdout (the live binary's version).
///
/// A missing/unreadable verus version (verus absent AND no `VERUS_VERSION`) is an
/// ENVIRONMENT error (`ForgeError::VerusAbsent`), NEVER a silent empty-string TCB
/// entry (R-DEFER-9 — the toolchain MUST be honestly identified). `forge audit`
/// runs the check pipeline (which already requires verus), so this never adds a
/// requirement the audit did not already have.
pub fn resolve_verus_version() -> Result<String, ForgeError> {
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
            detail: "`verus --version` produced no version string (cannot identify the toolchain \
                     in the audit TCB deterministically); set VERUS_VERSION to pin it"
                .to_string(),
        });
    }
    Ok(version)
}

/// The #274 LEAN-FRAGMENT MEMBERSHIP section (REQ-7) — an INFORMATIONAL,
/// additive `AuditManifest` section reporting, per checked item, whether
/// `--engine lean` would attempt it and (if not) the structured refusal class.
///
/// The membership decision is the SHIPPED dry-run `lean_export::export_item` over
/// the item's #226 CONTRACT obligation (REQ-8) — the SAME decision procedure
/// `--engine lean` makes (`LeanEngine::export` → `export_item`; a refusal maps to
/// the engine's `Unknown` honest skip). It is a PURE function of the parsed program
/// (`export_item` is fs/process/env-free; the lake/scratch side effects live
/// downstream in `LeanEngine::discharge`, never reached here): NO lake, NO scratch
/// file, NO `lean/` toolchain — same input file ⇒ byte-identical section (REQ-6
/// extended, AC-4/AC-10). The section gates NOTHING (REQ-10).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeanFragment {
    /// One membership row per [`AuditManifest::functions`] row, in source order
    /// (so it covers checked `fn`s AND `spec fn`s — both receive certs and both are
    /// `export_item` subjects). `#[serde(default)]` so an empty/absent section
    /// deserializes (AC-11).
    #[serde(default)]
    pub functions: Vec<LeanFragmentRow>,
}

impl LeanFragment {
    /// Probe each checked item's Lean-fragment membership (REQ-7, REQ-8). For every
    /// `cert` (in source order) mint the #226 CONTRACT obligation via the shipped
    /// `check::contract_obligation` seam (NO closure fork — the byte-identical
    /// pipeline closure, the AC-9 agreement guarantee) and dry-run
    /// `lean_export::export_item` over it; map `Ok`/`Err` to a [`LeanFragmentRow`].
    /// An item absent from `program` (defensive; the certs come from the same parsed
    /// file) reports `exportable: false` with the engine's own "item not found"
    /// marker class (`OutOfFragment`, mirroring `LeanEngine::export`). PURE — no
    /// lake, no fs, no process (REQ-8).
    fn from_certificates(certs: &[Certificate], program: &Program) -> Self {
        let functions = certs
            .iter()
            .map(|cert| LeanFragmentRow::probe(&cert.item, program))
            .collect();
        LeanFragment { functions }
    }
}

/// One Lean-fragment membership row (REQ-7) — the per-item answer to "would
/// `--engine lean` attempt this, and if not, why". Mirrors the `functions` row by
/// `name`; carries the coarse [attempt class](LeanFragmentRow::tier), the
/// fine-grained shipped tag, and (when refused) the verbatim `ExportRefusal`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeanFragmentRow {
    /// The item name (matches the `functions` row).
    pub name: String,
    /// `true` iff `export_item` returned `Ok(ExportedObligation)` — `--engine lean`
    /// would export this item's CONTRACT obligation.
    pub exportable: bool,
    /// The coarse attempt class (REQ-7):
    /// - `"auto"` — exportable AND [`ExportTier::is_auto`](crate::lean_export::ExportTier::is_auto) (tiers (a)/(b)):
    ///   `--engine lean` would export AND lake-invoke the auto battery;
    /// - `"interactive"` — exportable AND `RecursiveInteractive` (tier (c)):
    ///   `--engine lean` exports but does NOT invoke lake (returns `Unknown`);
    /// - `"none"` — refused: `--engine lean` honestly skips (`Verdict::Unknown`).
    pub tier: String,
    /// The fine-grained shipped tag ([`ExportTier::tag`](crate::lean_export::ExportTier::tag):
    /// `"fuel-free-auto"`/`"static-unfold-auto"`/`"recursive-interactive"`); present
    /// iff `exportable` (`#[serde(skip_serializing_if)]`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier_tag: Option<String>,
    /// The structured refusal (REQ-9); present iff NOT exportable
    /// (`#[serde(skip_serializing_if)]`). `class` is the stable machine surface
    /// (the `ExportRefusal` variant name); `reason` is its `Display`, verbatim.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<LeanRefusal>,
}

/// The coarse `tier` string for a refused row (REQ-7) — `--engine lean` honestly
/// skips an item it cannot export.
const TIER_NONE: &str = "none";
/// The coarse `tier` string for an exportable AUTO-tier row (REQ-7) — tiers (a)/(b).
const TIER_AUTO: &str = "auto";
/// The coarse `tier` string for an exportable INTERACTIVE-tier row (REQ-7) — tier (c).
const TIER_INTERACTIVE: &str = "interactive";

impl LeanFragmentRow {
    /// Probe one item's Lean-fragment membership via the shipped dry-run
    /// `export_item` (REQ-7, REQ-8). Mints the #226 CONTRACT obligation through the
    /// `check::contract_obligation` seam (the byte-identical pipeline closure — NO
    /// fork) and classifies the result. PURE: `export_item` builds strings only (no
    /// fs/process/env), so this row is a deterministic function of `(name, program)`
    /// (REQ-6/AC-4). An item not in `program` reports the engine's "item not found"
    /// `OutOfFragment` marker (mirrors `LeanEngine::export`).
    fn probe(name: &str, program: &Program) -> Self {
        let Some(item) = lean_export::find_item(program, name) else {
            // Defensive (the certs come from the same parsed file): mirror the
            // engine's own "item not found" skip rather than drop the row.
            return LeanFragmentRow {
                name: name.to_string(),
                exportable: false,
                tier: TIER_NONE.to_string(),
                tier_tag: None,
                refusal: Some(LeanRefusal {
                    class: refusal_class_name(&ExportRefusal::OutOfFragment(String::new()))
                        .to_string(),
                    reason: format!("item `{name}` not found in the parsed program"),
                }),
            };
        };
        // Mint the #226 CONTRACT obligation via the shipped seam — the SAME closure
        // the check pipeline / `--engine lean` use (REQ-8; NO fork). Dry-run
        // `export_item` over it: the membership decision IS the engine's.
        let obligation = crate::check::contract_obligation(program, item);
        match lean_export::export_item(&obligation, program, item) {
            Ok(exported) => {
                let tier = if exported.tier.is_auto() {
                    TIER_AUTO
                } else {
                    TIER_INTERACTIVE
                };
                LeanFragmentRow {
                    name: name.to_string(),
                    exportable: true,
                    tier: tier.to_string(),
                    tier_tag: Some(exported.tier.tag().to_string()),
                    refusal: None,
                }
            }
            Err(refusal) => LeanFragmentRow {
                name: name.to_string(),
                exportable: false,
                tier: TIER_NONE.to_string(),
                tier_tag: None,
                refusal: Some(LeanRefusal {
                    class: refusal_class_name(&refusal).to_string(),
                    reason: refusal.to_string(),
                }),
            },
        }
    }
}

/// A structured Lean-export refusal in a membership row (REQ-9) — the post-(v)
/// §4.2.5 LOUD inventory surfaced in the trust document. `class` is the STABLE
/// machine surface (the `ExportRefusal` variant name, an enum-stable string);
/// `reason` is the refusal's `Display` rendering, VERBATIM (a human diagnostic,
/// co-evolving with the exporter — OQ-5). Never a paraphrase, never a silent
/// omission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeanRefusal {
    /// The `ExportRefusal` variant name — the stable machine surface:
    /// `OutOfFragment`/`NotPureContract`/`IncompleteRegistry`/`NonIntResult`/
    /// `OpenHole`/`LoopBody`/`OptResResult`.
    pub class: String,
    /// The refusal's `Display` rendering, verbatim (REQ-9).
    pub reason: String,
}

/// The STABLE machine-surface variant name of an [`ExportRefusal`] (REQ-9). A total
/// match over the post-(v) inventory `pub enum ExportRefusal in lean_export.rs` — a
/// future variant is a compile error here (the closed-enum discipline), never a
/// silently-dropped class.
fn refusal_class_name(refusal: &ExportRefusal) -> &'static str {
    match refusal {
        ExportRefusal::OutOfFragment(_) => "OutOfFragment",
        ExportRefusal::NotPureContract(_) => "NotPureContract",
        ExportRefusal::IncompleteRegistry(_) => "IncompleteRegistry",
        ExportRefusal::NonIntResult(_) => "NonIntResult",
        ExportRefusal::OpenHole(_) => "OpenHole",
        ExportRefusal::LoopBody(_) => "LoopBody",
        ExportRefusal::OptResResult(_) => "OptResResult",
    }
}

impl AuditManifest {
    /// Build the v1 audit manifest from a settled certificate collection (REQ-1,
    /// REQ-4) — a PURE PROJECTION. Aggregates:
    ///
    /// - `functions` — each cert projected to a [`FunctionRow`] (REQ-1).
    /// - `project_assurance` — the [`AssuranceManifest::aggregate`] over `certs`
    ///   embedded as a [`ProjectAssuranceSection`] (REQ-5).
    /// - `tcb` — the §9 enumerable TCB (REQ-3): every slag ∪ every boundary ∪ the
    ///   `toolchain`.
    ///
    /// It re-runs NO verus, re-scores NO mutants, re-classifies NO closure: every
    /// field traces to a cert field, the assurance aggregate, the program's
    /// boundary contracts, or the two version strings (REQ-4, REQ-6). `program`
    /// supplies the boundary contracts' `req`/`ens`/`fx` text (the cert carries
    /// only the target).
    pub fn from_certificates(
        certs: &[Certificate],
        program: &Program,
        toolchain: Toolchain,
    ) -> Self {
        let functions = certs.iter().map(FunctionRow::from_certificate).collect();
        let assurance = AssuranceManifest::aggregate(certs);
        let project_assurance = ProjectAssuranceSection::from_assurance(&assurance);
        let tcb = Tcb::from_certificates(certs, program, toolchain);
        // The #274 informational membership section (REQ-7): one dry-run
        // `export_item` probe per cert, in source order (REQ-8, pure — no lake/fs).
        let lean_fragment = LeanFragment::from_certificates(certs, program);
        AuditManifest {
            manifest_version: MANIFEST_VERSION.to_string(),
            functions,
            project_assurance,
            tcb,
            lean_fragment,
        }
    }
}

/// Look up a fn's [`Contract`] in the parsed program by name (REQ-3). Returns the
/// contract of the matching `Item::Fn`, or `None` (a `spec fn` carries no
/// contract, and a name with no node has none). Pure read of the parsed AST — no
/// re-parsing, no re-verification.
fn lookup_contract<'a>(program: &'a Program, name: &str) -> Option<&'a Contract> {
    program.items.iter().find_map(|item| match item {
        Item::Fn(f) if f.name == name => Some(&f.contract),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{Certificate, SlagMeta};

    fn empty_program() -> Program {
        Program { items: Vec::new() }
    }

    fn toolchain() -> Toolchain {
        Toolchain::new("verus-test-0.0")
    }

    // REQ-1/REQ-4: a pure-Thermite cert collection projects to all-L3 rows + an
    // empty slag/boundary TCB (only the toolchain). Mirrors the corpus_empty_tcb
    // oracle shape (the live oracle is asserted in tests/audit_conformance.rs).
    #[test]
    fn pure_project_has_empty_slag_and_boundary_tcb() {
        let certs = vec![
            Certificate::new("sum", Level::L3, vec!["pure".to_string()], 0, vec![]),
            Certificate::new("spec_sum", Level::L3, vec!["pure".to_string()], 0, vec![]),
        ];
        let m = AuditManifest::from_certificates(&certs, &empty_program(), toolchain());
        assert_eq!(m.manifest_version, "v1");
        assert_eq!(m.functions.len(), 2);
        assert!(m.tcb.slag_blocks.is_empty(), "pure project: no slag blocks");
        assert!(
            m.tcb.boundary_contracts.is_empty(),
            "pure project: no boundary contracts"
        );
        assert_eq!(m.tcb.toolchain.verus, "verus-test-0.0");
        assert_eq!(m.tcb.toolchain.thermite, Toolchain::THERMITE_VERSION);
        assert_eq!(
            m.project_assurance.level,
            ProjectAssurance::Certified(Level::L3)
        );
    }

    // REQ-3: a valid slag cert enumerates a SlagBlock carrying reason/owner/review.
    #[test]
    fn slag_cert_enumerated_in_tcb() {
        let meta = SlagMeta {
            reason: "hand-tuned".to_string(),
            owner: "agent:x".to_string(),
            review: "required".to_string(),
        };
        let certs = vec![Certificate::slag_l1(
            "vendored",
            vec!["pure".to_string()],
            meta,
        )];
        let m = AuditManifest::from_certificates(&certs, &empty_program(), toolchain());
        assert_eq!(m.tcb.slag_blocks.len(), 1);
        let block = &m.tcb.slag_blocks[0];
        assert_eq!(block.name, "vendored");
        assert_eq!(block.reason, "hand-tuned");
        assert_eq!(block.owner, "agent:x");
        assert_eq!(block.review, "required");
    }

    // REQ-3: a boundary cert enumerates a BoundaryContract carrying the target.
    #[test]
    fn boundary_cert_enumerated_in_tcb() {
        let certs = vec![Certificate::boundary_l1(
            "ext_f",
            vec!["pure".to_string()],
            "ext::ext_f".to_string(),
        )];
        let m = AuditManifest::from_certificates(&certs, &empty_program(), toolchain());
        assert_eq!(m.tcb.boundary_contracts.len(), 1);
        let bc = &m.tcb.boundary_contracts[0];
        assert_eq!(bc.name, "ext_f");
        assert_eq!(bc.target, "ext::ext_f");
    }

    // REQ-5: a degraded fn appears in the lowered_assurance list.
    #[test]
    fn lowered_assurance_listed_in_project_section() {
        use crate::manifest::RejectReason;
        let reason = RejectReason {
            cause: "VerusTimeout".to_string(),
            detail: "rlimit".to_string(),
        };
        let certs = vec![
            Certificate::new("f", Level::L3, vec!["pure".to_string()], 0, vec![]),
            Certificate::new("g", Level::L2, vec!["pure".to_string()], 0, vec![])
                .into_degraded(reason),
        ];
        let m = AuditManifest::from_certificates(&certs, &empty_program(), toolchain());
        assert_eq!(m.project_assurance.lowered_assurance, vec!["g".to_string()]);
        assert_eq!(
            m.project_assurance.level,
            ProjectAssurance::Certified(Level::L2)
        );
    }

    // REQ-6 (determinism): same inputs → byte-identical JSON.
    #[test]
    fn manifest_is_deterministic() {
        let certs = vec![Certificate::new(
            "sum",
            Level::L3,
            vec!["pure".to_string()],
            0,
            vec![],
        )];
        let a = AuditManifest::from_certificates(&certs, &empty_program(), toolchain());
        let b = AuditManifest::from_certificates(&certs, &empty_program(), toolchain());
        let ja = serde_json::to_string(&a).expect("serialize a");
        let jb = serde_json::to_string(&b).expect("serialize b");
        assert_eq!(ja, jb);
    }

    // --- #274 lean_fragment membership unit tests (REQ-7..10) --------------------

    fn parse_program(src: &str) -> Program {
        let parsed = thermite_syntax::parse(src);
        assert!(parsed.is_clean(), "fixture must parse: {:?}", parsed.errors);
        parsed.program
    }

    // REQ-7/REQ-8 (AC-7): a pure-int-tail specCall-free body probes exportable
    // tier=auto (fuel-free-auto) — the membership decision is the shipped dry-run
    // export, with NO verus run (the probe is pure). Expected from the §6.1 tier (a)
    // definition (R-CHAR-3), not forge stdout.
    #[test]
    fn probe_pure_int_tail_is_auto() {
        let program =
            parse_program("fn count(n: u32) -> u32 req n < 100 ens result == n fx pure { n }");
        let row = LeanFragmentRow::probe("count", &program);
        assert!(
            row.exportable,
            "a specCall-free pure-int-tail body is exportable"
        );
        assert_eq!(row.tier, "auto");
        assert_eq!(row.tier_tag.as_deref(), Some("fuel-free-auto"));
        assert!(
            row.refusal.is_none(),
            "an exportable row carries no refusal"
        );
    }

    // REQ-9 (AC-8): a boundary fn (foreign body, no in-language body) probes
    // NotPureContract with the verbatim shipped Display reason.
    #[test]
    fn probe_boundary_is_not_pure_contract() {
        let program = parse_program(
            "#[boundary(\"ext::e\")] fn bnd(x: u32) -> u32 req x < 100 ens result == x fx pure ;",
        );
        let row = LeanFragmentRow::probe("bnd", &program);
        assert!(!row.exportable);
        assert_eq!(row.tier, "none");
        assert_eq!(row.tier_tag, None, "a refused row carries no tier_tag");
        // Compare the whole `Option<LeanRefusal>` (derives PartialEq) — the verbatim
        // shipped Display reason (REQ-9), no fallible extraction.
        assert_eq!(
            row.refusal,
            Some(LeanRefusal {
                class: "NotPureContract".to_string(),
                reason: "not a pure-contract item (the §4 scope): fn `bnd` is a boundary fn \
                         (foreign body, no in-language body)"
                    .to_string(),
            })
        );
    }

    // REQ-8 (AC-9): the probe row EQUALS what `export_item` returns for that item's
    // CONTRACT obligation minted via the SAME `check::contract_obligation` seam — the
    // report and the `--engine lean` admission decision can never disagree. Covers an
    // exportable item AND a refused (boundary) item.
    #[test]
    fn probe_agrees_with_direct_export_item() {
        for (name, src) in [
            (
                "count",
                "fn count(n: u32) -> u32 req n < 100 ens result == n fx pure { n }",
            ),
            (
                "bnd",
                "#[boundary(\"ext::e\")] fn bnd(x: u32) -> u32 req x < 100 ens result == x fx pure ;",
            ),
        ] {
            let program = parse_program(src);
            let found = lean_export::find_item(&program, name);
            assert!(found.is_some(), "item `{name}` parses");
            let Some(item) = found else { continue };
            let obligation = crate::check::contract_obligation(&program, item);
            let direct = lean_export::export_item(&obligation, &program, item);
            let row = LeanFragmentRow::probe(name, &program);
            match direct {
                Ok(exported) => {
                    assert!(row.exportable, "row agrees: exportable");
                    let expect_tier = if exported.tier.is_auto() {
                        "auto"
                    } else {
                        "interactive"
                    };
                    assert_eq!(row.tier, expect_tier, "row tier agrees with export_item");
                    assert_eq!(
                        row.tier_tag.as_deref(),
                        Some(exported.tier.tag()),
                        "row tier_tag agrees with ExportTier::tag"
                    );
                    assert_eq!(row.refusal, None, "an exportable row carries no refusal");
                }
                Err(refusal) => {
                    assert!(!row.exportable, "row agrees: refused");
                    // The row's refusal EQUALS the direct export_item refusal,
                    // field-for-field (stable class + verbatim Display reason).
                    assert_eq!(
                        row.refusal,
                        Some(LeanRefusal {
                            class: refusal_class_name(&refusal).to_string(),
                            reason: refusal.to_string(),
                        }),
                        "the row refusal agrees with export_item field-for-field"
                    );
                }
            }
        }
    }

    // REQ-10 (AC-11): a PRE-AMENDMENT v1 document (no `lean_fragment` key) still
    // deserializes into the TYPED `AuditManifest` — the `#[serde(default)]` additive
    // discipline (the new section defaults to an empty `LeanFragment`).
    #[test]
    fn pre_amendment_v1_deserializes_into_typed_manifest() {
        let pre = r#"{
            "manifest_version": "v1",
            "functions": [],
            "project_assurance": {
                "level": { "kind": "certified", "level": "L3" },
                "scope": { "kind": "end_to_end" }
            },
            "tcb": {
                "slag_blocks": [],
                "boundary_contracts": [],
                "toolchain": { "verus": "x", "thermite": "y" }
            }
        }"#;
        let parsed: Result<AuditManifest, _> = serde_json::from_str(pre);
        assert!(
            parsed.is_ok(),
            "pre-amendment v1 doc must deserialize (serde default): {:?}",
            parsed.as_ref().err()
        );
        let Ok(m) = parsed else { return };
        assert_eq!(m.manifest_version, "v1");
        assert!(
            m.lean_fragment.functions.is_empty(),
            "the absent lean_fragment defaults to an empty section"
        );
    }

    // REQ-9 (AC-7) — the sum.th HAND-TRACE VERDICT, pinned in-crate: BOTH rows refuse
    // OutOfFragment but NOT for the spec-calling-inv reason the doc narrative grounded
    // — `sum` is the recursive-registry contract over a while body; `spec_sum` is the
    // slice-pattern match body. The probe needs no verus (pure). The exact verbatim
    // reasons are hand-derived from the exporter (R-CHAR-3) — see cases.json.
    #[test]
    fn probe_sum_th_refusals_are_hand_traced() {
        let src = include_str!("../../conformance/sum.th");
        let program = parse_program(src);

        let sum = LeanFragmentRow::probe("sum", &program);
        assert!(!sum.exportable);
        assert!(sum.refusal.is_some(), "sum refusal present");
        if let Some(r) = sum.refusal {
            assert_eq!(r.class, "OutOfFragment");
            assert!(
                r.reason
                    .contains("RECURSIVE-registry contract clause over a while body"),
                "sum refuses the recursive-registry-over-while-body OutOfFragment (the §4 \
                 interactive residual), NOT the spec-calling-inv reason: {}",
                r.reason
            );
        }

        let spec_sum = LeanFragmentRow::probe("spec_sum", &program);
        assert!(!spec_sum.exportable);
        assert!(spec_sum.refusal.is_some(), "spec_sum refusal present");
        if let Some(r) = spec_sum.refusal {
            assert_eq!(r.class, "OutOfFragment");
            assert!(
                r.reason.contains("Slice"),
                "spec_sum refuses its slice-pattern match body (OUT of S_C): {}",
                r.reason
            );
        }
    }
}
