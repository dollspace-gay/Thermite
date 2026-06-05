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

use std::process::Command;

use serde::{Deserialize, Serialize};
use thermite_syntax::{Contract, EffectRow, Item, Program};

use crate::cli::ForgeError;
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
        AuditManifest {
            manifest_version: MANIFEST_VERSION.to_string(),
            functions,
            project_assurance,
            tcb,
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
}
