//! `forge/src/review.rs` — the pluggable spec-intent review slot (`forge review`,
//! `thermite-design.md` §7 line 227, §summary line 298, issue #19). The §7
//! "residue surfaced for review": the one irreducible judgment the deterministic
//! battery (#6/#12/#13 vacuity + mutation pre-screening) cannot make — "is this
//! contract what you meant?".
//!
//! Governing design: `.design/forge/spec-review.md`.
//!
//! `forge review [item]` extracts the pre-screened declarative spec layer for each
//! `fn` — the verbatim `req`/`ens`/`fx` clauses plus the declaration (name, params,
//! return type, `dec` measure) of every directly-referenced `spec fn`, with no
//! bodies — and pairs each intent-reviewable contract with an "is this what you
//! meant?" prompt. It emits this as a machine artifact (`--json`, for a critic
//! model) and a human form, and defines the pluggable verdict slot
//! ([`ReviewVerdict`]) an external reviewer fills.
//!
//! Two data sources, both already shipped (mirroring `audit::AuditManifest`'s pure
//! projection):
//!
//! 1. The battery verdict — the `Vec<Certificate>` from [`crate::check::check_file`]
//!    (the same default-config pipeline `forge check`/`forge audit` run — no extra
//!    verification). The pre-screening predicate ([`is_intent_reviewable`]) reads
//!    `cert.reject`/`cert.level`: a cert is intent-reviewable iff it is reject-free
//!    and a certified rung (`manifest::cert_certifies` — L1/L2/L3, incl. a slag /
//!    boundary L1 whose contract is the trust statement, OQ-4). A battery-failing
//!    cert (`reject.is_some()`) is flagged [`battery_failing`] with its
//!    `reject.cause` and is not surfaced for intent review (R-DEFER-9: the
//!    mechanical failure is answered first).
//! 2. The contract surface — the parsed `Program` (`thermite_syntax::parse`). The
//!    spec layer is built from the verbatim `Clause.text` (`ast.rs`), and the
//!    spec-fn references are resolved by walking the contract clause `Expr`s for a
//!    callee name matching a top-level `Item::SpecFn`. Exclusion is structural: the
//!    projection reads `contract`/`name`/`params`/`ret`/`dec` and never touches
//!    `FnItem.body`/`SpecFnItem.body`, so "no bodies" is enforced by which fields
//!    are read (parallel to `audit::FunctionRow::from_certificate`).
//!
//! forge does not produce the `aligned` verdict (R-CODE-5): the extraction (the
//! artifact) is a deterministic pure projection; the verdict is the external
//! reviewer's. The `--reviewer <cmd>` shell-out pipes the JSON artifact to the
//! reviewer's stdin and reads the [`ReviewVerdict`] JSON from its stdout, attaching
//! it as a separate `*.review.json` record — not a `Certificate` field (OQ-2: the
//! cert is the mechanical verdict; intent review is a separate judgment). A
//! failing/absent reviewer cmd is a [`ForgeError`].
//!
//! ## REQ status
//!
//! <!-- generated:reqs view=forge-review-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-FORGE-REVIEW-COMMAND | shipped | `forge/src/review.rs` | forge review command and reviewer dispatch |  |
//! | REQ-FORGE-REVIEW-DETERMINISM | shipped | `forge/src/review.rs` | Spec-review extraction is deterministic |  |
//! | REQ-FORGE-REVIEW-DUAL-EMISSION | shipped | `forge/src/review.rs` | Spec-review emits machine and human forms |  |
//! | REQ-FORGE-REVIEW-INTENT-PROMPT | shipped | `forge/src/review.rs` | Per-contract intent-review prompt |  |
//! | REQ-FORGE-REVIEW-PRE-SCREEN | shipped | `forge/src/review.rs` | Spec-review pre-screens battery-passing contracts |  |
//! | REQ-FORGE-REVIEW-SPEC-LAYER | shipped | `forge/src/review.rs` | Spec-review extracts declarations without bodies |  |
//! | REQ-FORGE-REVIEW-VERDICT-RECORD | shipped | `forge/src/review.rs` | Review verdicts are separate records |  |
//! <!-- /generated:reqs -->
//!
//! ## Cluster C10 — ergonomics ripple (`.design/basis/11-ergonomics.md`, #112)
//!
//! <!-- generated:reqs view=forge-review-ergonomics-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-FORGE-REVIEW-MATCH-GUARD | shipped | `forge/src/review.rs` | Spec-review walks match-arm guards |  |
//! <!-- /generated:reqs -->

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use thermite_syntax::{Contract, Expr, Item, Param, Program, Type};

use crate::check;
use crate::cli::ForgeError;
use crate::manifest::{cert_certifies, effects_of, Certificate};

/// `true` iff a certificate is intent-reviewable (REQ-2): it passed the mechanical
/// battery — reject-free and a certified rung (`manifest::cert_certifies`: L1/L2/L3,
/// including a `#[slag]` / `#[boundary]` L1 whose contract is the trust statement a
/// reviewer should audit, OQ-4). A battery-failing cert (`reject.is_some()`:
/// vacuity / weak-contract / counterexample / timeout) is not intent-reviewable —
/// its failure is mechanical and answered first (R-DEFER-9). A thin alias over
/// `cert_certifies` so the review pre-screen and the project headline agree on what
/// "passed the battery" means.
pub fn is_intent_reviewable(cert: &Certificate) -> bool {
    cert_certifies(cert)
}

/// The declaration of one `spec fn` referenced by a reviewed contract (REQ-1) — a
/// body-free projection of a `SpecFnItem`. The §7 "few percent" surface the reviewer
/// reads to understand what the contract's `spec fn` means, without the body
/// (`SpecFnItem.body` is never read — the "no bodies" rule is structural).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecFnDecl {
    /// The spec fn name (the callee a contract clause references).
    pub name: String,
    /// The declaration signature rendered without the body, e.g.
    /// `spec fn spec_sum(xs: &[u32]) -> u64`. Built from `name`/`params`/`ret`.
    pub signature: String,
    /// The `dec` decreases-measure clause text (verbatim `Clause.text`), e.g.
    /// `xs.len()`. The well-formedness measure, read from the declaration.
    pub dec: String,
}

impl SpecFnDecl {
    /// Project a `SpecFnItem` to its body-free declaration (REQ-1). Reads `name`,
    /// `params`, `ret`, and `dec` — never `body` (the §7 "no bodies" rule, enforced
    /// structurally by which fields this reads, paralleling
    /// `audit::FunctionRow::from_certificate`).
    fn from_spec_fn(s: &thermite_syntax::SpecFnItem) -> Self {
        SpecFnDecl {
            name: s.name.clone(),
            signature: format!(
                "spec fn {}({}) -> {}",
                s.name,
                render_params(&s.params),
                render_type(&s.ret),
            ),
            dec: s.dec.text.clone(),
        }
    }
}

/// The declarative spec layer of one reviewed `fn` (REQ-1) — the verbatim contract
/// surface a reviewer reads, with no bodies. The §7 "the certificate includes the
/// full spec layer". Built by [`SpecLayer::extract`] from `FnItem.contract` (the
/// verbatim `Clause.text`) + the directly-referenced `spec fn` declarations; the
/// `fn`'s own body and every spec fn's body are excluded (structural — the
/// projection never reads `body`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecLayer {
    /// The verbatim `req` precondition clause text (`Clause.text`).
    pub req: String,
    /// The verbatim `ens` postcondition clause texts, in source order.
    pub ens: Vec<String>,
    /// The effect row as tokens (e.g. `["pure"]`) — the `fx` row (via
    /// `manifest::effects_of`, the same projection the cert uses).
    pub fx: Vec<String>,
    /// The declarations of the directly-referenced `spec fn`s (OQ-3 direct-only),
    /// sorted + deduplicated by name (deterministic, R-CODE-5). No bodies.
    pub referenced_spec_fns: Vec<SpecFnDecl>,
}

impl SpecLayer {
    /// Extract the body-free declarative spec layer for one `fn` (REQ-1). Reads the
    /// verbatim `Contract` clauses + resolves the directly-referenced `spec fn`
    /// declarations against `spec_fns`; never touches `FnItem.body` /
    /// `SpecFnItem.body`.
    fn extract(contract: &Contract, spec_fns: &[&thermite_syntax::SpecFnItem]) -> Self {
        SpecLayer {
            req: contract.req.text.clone(),
            ens: contract.ens.iter().map(|c| c.text.clone()).collect(),
            fx: effects_of(&contract.fx),
            referenced_spec_fns: referenced_spec_fns(contract, spec_fns),
        }
    }
}

/// One intent-reviewable function in the artifact (REQ-2/REQ-3) — a battery-passing
/// contract surfaced with its spec layer + the "is this what you meant?" prompt and
/// (after a reviewer runs) an optional [`ReviewVerdict`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntentReview {
    /// The function name.
    pub item: String,
    /// The body-free declarative spec layer (REQ-1).
    pub spec_layer: SpecLayer,
    /// The per-contract "is this what you meant?" prompt (REQ-3) — the §7 question,
    /// framed so the only open question is spec-intent alignment (the mechanical
    /// questions already discharged by the battery).
    pub prompt: String,
}

impl IntentReview {
    /// Build the intent-review entry for one battery-passing `fn` (REQ-2/REQ-3).
    fn new(item: String, spec_layer: SpecLayer) -> Self {
        let prompt = IntentReview::prompt(&item);
        IntentReview {
            item,
            spec_layer,
            prompt,
        }
    }

    /// The per-contract "is this what you meant?" intent-review prompt (REQ-3, §7
    /// line 227). Names the item and frames the only open question as spec-intent
    /// alignment — the mechanical questions (vacuity #6/#13, contract strength #12)
    /// are already discharged by the battery this item passed. Deterministic: a pure
    /// function of the item name (R-CODE-5).
    fn prompt(item: &str) -> String {
        format!(
            "`{item}` passed the mechanical battery (non-vacuous, non-trivially-weak, \
             mutation-scored). The only open question is spec-intent alignment: does this \
             contract say what you MEANT `{item}` to guarantee? (is this what you meant?)"
        )
    }
}

/// One battery-failing function in the artifact (REQ-2) — a contract the battery
/// rejected (vacuity / weak-contract / counterexample / timeout). Flagged with its
/// `reject.cause` and not surfaced for intent review (R-DEFER-9: the reviewer is
/// never asked "is this what you meant?" about a mechanically-failing contract — the
/// mechanical failure is answered first).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatteryFailing {
    /// The function name.
    pub item: String,
    /// The §7 battery reject cause tag (e.g. `"EnsIsTrivial"`, `"WeakContract"`,
    /// `"VerusTimeout"`) — from `Certificate::reject.cause`.
    pub cause: String,
    /// The human-readable reject detail — from `Certificate::reject.detail`.
    pub detail: String,
}

/// The spec-intent review artifact (REQ-1/REQ-2/REQ-5) — the machine + human
/// deliverable `forge review` emits. A pure projection (REQ-6) of the parsed
/// program and the battery cert collection, partitioning the file's `fn`s into the
/// intent-reviewable (battery-passing, with spec layers + prompts) and the
/// battery-failing (flagged, not surfaced). The `--json` form is the critic-model
/// surface; `cli::render_review` is the human form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewArtifact {
    /// The intent-reviewable functions (battery-passing) — surfaced with spec
    /// layers + prompts, in source order (REQ-2).
    pub intent_reviewable: Vec<IntentReview>,
    /// The battery-failing functions (flagged, not surfaced for intent review), in
    /// source order (REQ-2; R-DEFER-9).
    pub battery_failing: Vec<BatteryFailing>,
    /// The BURNED forge-tier lemmas (`.design/stage1-forge-tier.md` REQ-9, increment 3):
    /// each certified `lemma` surfaced with its burn receipt — as a certified item
    /// surfaces, so a reviewer sees the project's proven lemma library alongside the
    /// reviewed fns. In source order. Omitted (empty) on the default Verus path / the v1
    /// corpus (which discharge no forge lemma), and `#[serde(default,
    /// skip_serializing_if)]` so a v1 review artifact serializes BYTE-IDENTICALLY (additive
    /// only, mirroring the cert layer's additive discipline).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub burned_lemmas: Vec<BurnedLemma>,
    /// The `@bv`-tagged clauses' shadow flags (`.design/stage3-bv-reconstruction.md`
    /// REQ-3 / AC-4 — Lock 1): every machine-semantics clause surfaced for review, so a
    /// reviewer sees the project's semantic forks alongside the contracts. One entry per
    /// tagged clause (read from each obligation's `bv_shadow`), in cert/obligation order.
    /// Omitted (empty) on the default Verus path / the v1 corpus (no `@bv` tag), and
    /// `#[serde(default, skip_serializing_if)]` so a v1 review artifact serializes
    /// BYTE-IDENTICALLY (the additive `burned_lemmas` discipline). `grep bv_shadow` over
    /// `forge review --json` ≡ the project's tagged clauses (AC-4).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bv_shadows: Vec<BvShadowClause>,
    /// The "semantic forks and definition towers" section
    /// (`.design/stage3-bv-reconstruction.md` REQ-6 / AC-7): the aggregate legibility
    /// surface a reviewer reads alongside the per-clause `bv_shadows` + `burned_lemmas`
    /// above — bv-shadow density per module, every burned lemma's definition-tower depth,
    /// and the post-ship **F-F density tripwire**. A pure projection
    /// ([`crate::forks::SemanticForks::build`]); `None` (and omitted) on the default Verus
    /// path / the v1 corpus (no `@bv` tag, no burned lemma), so a v1 review artifact
    /// serializes BYTE-IDENTICALLY (the additive `bv_shadows`/`burned_lemmas` discipline).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_forks: Option<crate::forks::SemanticForks>,
}

/// One `@bv`-tagged clause's shadow flag in the review artifact
/// (`.design/stage3-bv-reconstruction.md` REQ-3 / AC-4 — Lock 1). A pure projection of an
/// obligation's [`crate::manifest::BvShadow`]: the owning item, the per-clause obligation
/// name, and the shadow block (flag + semantics + `nowrap_obligation` + note).
/// Never fabricated — read verbatim from the cert (R-CODE-5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BvShadowClause {
    /// The owning item (the `fn` / `lemma` whose clause carries the tag).
    pub item: String,
    /// The per-clause obligation name (e.g. `mix64::ens#0: …` / `mix64#ens#0`).
    pub clause: String,
    /// The shadow flag block — `flagged` / `semantics` / `nowrap_obligation` / `note`
    /// (RFC §9), read verbatim from the obligation.
    pub shadow: crate::manifest::BvShadow,
}

/// One burned forge-tier lemma in the review artifact (`.design/stage1-forge-tier.md` REQ-9,
/// increment 3) — a certified `lemma` surfaced like any certified item, carrying the burn
/// receipt's auditable figures (the committed-proof token count + the lemmas it cited). A
/// pure projection of the lemma's certificate; never fabricated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BurnedLemma {
    /// The lemma name (the certified item).
    pub item: String,
    /// The committed-proof lexer-token count from the burn receipt (REQ-7 / Q-burn).
    pub proof_tokens: usize,
    /// The lemmas this lemma's proof cited (post-dedup-rewrite, REQ-9) — empty if it cited
    /// none. Omitted when empty (mirrors the burn receipt's own field discipline).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cited_lemmas: Vec<String>,
}

/// The pluggable verdict slot (REQ-4, OQ-2) — the structured per-contract judgment
/// an external reviewer (a human, or a critic model whose only question is
/// spec-intent alignment) fills. forge does not fabricate `aligned` (R-CODE-5): this
/// is the reviewer's annotation, read from the `--reviewer <cmd>`'s stdout and
/// attached as a separate `*.review.json` record — not a `Certificate` field (the
/// cert is the mechanical verdict; this is the spec-intent judgment).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewVerdict {
    /// The function the verdict is about (must match an `intent_reviewable` item).
    pub item: String,
    /// The reviewer's judgment: does the contract say what the author meant?
    pub aligned: bool,
    /// An optional reviewer note (the "why" / the suggested correction).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// The separate review record written to `<file>.review.json` (REQ-4, OQ-2 reading
/// (a)). It keeps the verdict outside the certificate entirely — the §1 "skeptical
/// third party audits the residue" framing (the verdict is the third party's
/// annotation, not the toolchain's certificate). A pure data document: the file
/// path it reviews + the collected per-item verdicts. Attaching a verdict never
/// touches any `Certificate`'s `oracle_subset` (the soundness invariant, R-SPEC-2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewRecord {
    /// The reviewed file (the artifact's provenance).
    pub file: String,
    /// The per-item verdicts the external reviewer filled. Empty until a reviewer
    /// runs (the artifact-only path).
    pub verdicts: Vec<ReviewVerdict>,
}

/// Run the spec-intent extraction for `path` (REQ-1/REQ-2/REQ-6). Runs the same
/// default-config check pipeline `forge check` / `forge audit` run
/// ([`check::check_file`] — no extra verification), parses the file once for the
/// contract surface, and projects the two into the [`ReviewArtifact`]:
///
/// - a cert that passed the battery ([`is_intent_reviewable`]) → an [`IntentReview`]
///   carrying its body-free [`SpecLayer`] + the §7 prompt;
/// - a cert the battery rejected → a [`BatteryFailing`] flag (not surfaced).
///
/// A pure projection (REQ-6): the artifact is a deterministic function of the parsed
/// program + the cert collection — no wall-clock, no model call. An optional
/// `item_filter` (the `forge review <file> [item]` positional) restricts the
/// artifact to a single function (both partitions are filtered).
pub fn review_file(
    path: impl AsRef<Path>,
    item_filter: Option<&str>,
) -> Result<ReviewArtifact, ForgeError> {
    let path = path.as_ref();

    // Resolve the source once for the contract surface (REQ-1) and to decide the
    // route below. A re-parse of a source `check_file` re-validates (deterministic,
    // R-CODE-5), never a re-verification — the `audit` precedent. The argument is a
    // single `.th` file or a canonical `.thpkg.json` manifest; a manifest resolves
    // its transitive module closure and each item keeps its module-local span.
    let resolved = crate::thermite_package::load_source(path)?;
    let program = resolved.program();

    // The battery cert collection `review` projects (REQ-2 — re-runs no verus). A
    // bit-vector project (any `@bv`-tagged clause, stage-3 REQ-3 / AC-4) routes through
    // the bv engine so the per-clause shadow flags surface in the artifact's `bv_shadows`
    // section; every tag-free project (the whole v1 corpus) keeps the default `check_file`
    // pipeline byte-identical (the same collection `forge check`/`forge audit` project).
    let certs = if check::program_has_bv_tag(program) {
        check::check_file_with_engine(
            path,
            check::CheckOptions {
                engine: check::EngineSelection::Bv,
                ..Default::default()
            },
        )?
    } else {
        check::check_file(path)?
    };

    Ok(project_artifact(&certs, program, item_filter))
}

/// Project a settled cert collection + parsed program into the [`ReviewArtifact`]
/// (REQ-1/REQ-2/REQ-6) — the pure-projection core, split out so it is unit-testable
/// without spawning verus. Partitions each `fn`'s cert: a battery-passing cert
/// becomes an [`IntentReview`] with its body-free spec layer; a rejected cert
/// becomes a [`BatteryFailing`] flag. A `spec fn` carries no contract, so it is a
/// pure shared dependency the spec layer references, never a reviewed item itself.
fn project_artifact(
    certs: &[Certificate],
    program: &Program,
    item_filter: Option<&str>,
) -> ReviewArtifact {
    let spec_fns: Vec<&thermite_syntax::SpecFnItem> = program
        .items
        .iter()
        .filter_map(|i| match i {
            Item::SpecFn(s) => Some(s),
            Item::Const(_) | Item::Fn(_) => None,
            // Basis Stage 1a (`.design/basis/01-adts.md`): a `struct`/`enum`
            // item is not a `spec fn` — it contributes nothing to the
            // referenced-spec-fn projection (neutral value `None`). Dead-in-1a
            // (an ADT program dies at the validator before a cert is reviewed).
            Item::Struct(_) | Item::Enum(_) => None,
            // Forge-tier item (stage1-forge-tier.md REQ-3): no v1 review/projection
            // consumer yet (increments 2b-3); not a `spec fn` → contributes nothing
            // (neutral `None`), mirroring the inert ADT-decl arm.
            Item::Forge(_) => None,
        })
        .collect();

    let mut intent_reviewable = Vec::new();
    let mut battery_failing = Vec::new();
    let mut burned_lemmas = Vec::new();
    let mut bv_shadows = Vec::new();

    for cert in certs {
        if let Some(filter) = item_filter {
            if cert.item != filter {
                continue;
            }
        }
        // Lock 1 (`.design/stage3-bv-reconstruction.md` REQ-3 / AC-4): surface every
        // `@bv`-tagged clause's shadow flag, read verbatim from its obligations. This is
        // ORTHOGONAL to the battery partition below (a tagged clause surfaces whether its
        // item certifies, fails, or is a lemma), so it runs over every cert before the
        // partition's `continue`s — the machine-semantics fork stays visible regardless of
        // verdict.
        for obl in &cert.obligations {
            if let Some(shadow) = &obl.bv_shadow {
                bv_shadows.push(BvShadowClause {
                    item: cert.item.clone(),
                    clause: obl.name.clone(),
                    shadow: shadow.clone(),
                });
            }
        }
        // A BURNED forge-tier lemma surfaces like any certified item (REQ-9, increment 3):
        // a certified `lemma` (an L3 forge `lemma` item carrying a burn receipt) is added to
        // the burned-lemmas partition with its receipt figures. It has no `fn` contract (a
        // lemma is pure proof), so it would otherwise be skipped by the contract lookup
        // below; handle it here, before that skip.
        if let Some(burned) = burned_lemma_projection(program, cert) {
            burned_lemmas.push(burned);
            continue;
        }
        // A `spec fn` carries no `req`/`ens`/`fx` contract (§4.2) — it is a pure
        // shared dependency the reviewed `fn`s' spec layers reference, not an
        // intent-reviewed item itself. Skip it (it has no contract to review).
        let contract = match lookup_fn_contract(program, &cert.item) {
            Some(c) => c,
            None => continue,
        };

        if is_intent_reviewable(cert) {
            let spec_layer = SpecLayer::extract(contract, &spec_fns);
            intent_reviewable.push(IntentReview::new(cert.item.clone(), spec_layer));
        } else if let Some(reject) = &cert.reject {
            // Battery-failing (R-DEFER-9): flagged with its cause, not surfaced for
            // intent review. A non-certifying cert always carries a `reject`
            // (`Certificate::rejected*` / `timeout`); a `None` (an un-discharged L0
            // with no reject) is recorded with an explicit cause so nothing
            // mechanically-failing is silently dropped.
            battery_failing.push(BatteryFailing {
                item: cert.item.clone(),
                cause: reject.cause.clone(),
                detail: reject.detail.clone(),
            });
        } else {
            battery_failing.push(BatteryFailing {
                item: cert.item.clone(),
                cause: "NotCertified".to_string(),
                detail: "the item did not reach a certified rung (no reject cause recorded)"
                    .to_string(),
            });
        }
    }

    // REQ-6 / AC-7: the aggregate "semantic forks and definition towers" section — the
    // bv-shadow density per module, the burned-lemma tower depths, and the F-F density
    // tripwire. `None` (omitted) on the default Verus / v1 path (no tag, no burned lemma).
    let semantic_forks = crate::forks::SemanticForks::build(certs, program);

    ReviewArtifact {
        intent_reviewable,
        battery_failing,
        burned_lemmas,
        bv_shadows,
        semantic_forks,
    }
}

/// Project a certified forge-tier `lemma` cert into a [`BurnedLemma`] (REQ-9, increment 3),
/// or `None` if `cert` is not a burned lemma. A burned lemma is a `cert` whose item is a
/// top-level `lemma` in `program`, that certified ([`is_intent_reviewable`] — L3, no reject),
/// and that carries a burn receipt (the proof closed a goal). The projection reads the
/// receipt's auditable figures verbatim — pure, never fabricated (R-CODE-5).
fn burned_lemma_projection(program: &Program, cert: &Certificate) -> Option<BurnedLemma> {
    let is_lemma = program.items.iter().any(
        |i| matches!(i, Item::Forge(thermite_syntax::ForgeItem::Lemma(l)) if l.name == cert.item),
    );
    if !is_lemma || !is_intent_reviewable(cert) {
        return None;
    }
    let receipt = cert.burn.as_ref()?;
    Some(BurnedLemma {
        item: cert.item.clone(),
        proof_tokens: receipt.proof_tokens,
        cited_lemmas: receipt.cited_lemmas.clone(),
    })
}

/// Look up a `fn`'s [`Contract`] in the parsed program by name (REQ-1). Returns the
/// contract of the matching `Item::Fn`, or `None` (a `spec fn` carries no contract,
/// and a name with no node has none). A read of the parsed AST — no re-parsing,
/// no re-verification (the `audit::lookup_contract` precedent).
fn lookup_fn_contract<'a>(program: &'a Program, name: &str) -> Option<&'a Contract> {
    program.items.iter().find_map(|item| match item {
        Item::Fn(f) if f.name == name => Some(&f.contract),
        _ => None,
    })
}

/// Resolve the directly-referenced `spec fn` declarations of a contract (REQ-1,
/// OQ-3 direct-only). Walks every `req`/`ens` clause `Expr` for a callee name
/// matching a top-level `SpecFnItem`, and projects each match to its body-free
/// [`SpecFnDecl`]. The result is sorted + deduplicated by name (deterministic,
/// R-CODE-5 — a clause referencing `spec_sum` twice yields one declaration). No
/// bodies (each `SpecFnDecl` reads only the declaration fields).
fn referenced_spec_fns(
    contract: &Contract,
    spec_fns: &[&thermite_syntax::SpecFnItem],
) -> Vec<SpecFnDecl> {
    // Collect referenced names from every contract clause expr (req + each ens). A
    // BTreeSet → sorted + deduplicated (deterministic), and only names that resolve
    // to a top-level spec fn are kept (OQ-3 direct-only — no transitive closure).
    let mut names: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    collect_callee_names(&contract.req.expr, &mut names);
    for clause in &contract.ens {
        collect_callee_names(&clause.expr, &mut names);
    }
    names
        .into_iter()
        .filter_map(|name| {
            spec_fns
                .iter()
                .find(|s| s.name == name)
                .map(|s| SpecFnDecl::from_spec_fn(s))
        })
        .collect()
}

/// Walk an `Expr` collecting every callee name that is a plain path (a free
/// `f(args)` call or a bare `Path`), recursing into every sub-expression so a
/// reference nested in a binary/cast/index/method-call/etc. is found. Used to
/// resolve a contract's directly-referenced `spec fn`s. Reads only the expression
/// shape — never a body (a contract clause holds no body).
fn collect_callee_names(expr: &Expr, out: &mut std::collections::BTreeSet<String>) {
    match expr {
        Expr::Array(elements) => {
            for element in elements {
                collect_callee_names(element, out);
            }
        }
        Expr::ArrayRepeat { value, .. } => collect_callee_names(value, out),
        Expr::Path(segments) => {
            // A bare path: the last segment is the referenced name (`spec_sum`,
            // `u32::MAX` → `MAX`). The spec-fn match below keeps only real spec fns,
            // so a non-spec-fn path (a local, a const) is harmlessly collected.
            if let Some(last) = segments.last() {
                out.insert(last.clone());
            }
        }
        Expr::Call { callee, args } => {
            collect_callee_names(callee, out);
            for a in args {
                collect_callee_names(a, out);
            }
        }
        Expr::MethodCall {
            receiver,
            name: _,
            args,
        } => {
            // A method call `recv.m(args)` is postfix sugar — `m` is not a free
            // spec-fn reference (a spec fn is called free: `spec_sum(xs)`), so only
            // the receiver + args are walked, not the method name.
            collect_callee_names(receiver, out);
            for a in args {
                collect_callee_names(a, out);
            }
        }
        Expr::Field { receiver, name: _ } => collect_callee_names(receiver, out),
        Expr::Closure { params: _, body } => collect_callee_names(body, out),
        Expr::Match { scrutinee, arms } => {
            collect_callee_names(scrutinee, out);
            for arm in arms {
                // A C10 match guard may call a fn (`.design/basis/11-ergonomics.md`
                // REQ-3) — its callees are part of the review surface.
                if let Some(guard) = &arm.guard {
                    collect_callee_names(guard, out);
                }
                collect_callee_names(&arm.body, out);
            }
        }
        Expr::If { cond, then, else_ } => {
            collect_callee_names(cond, out);
            collect_block_callee_names(then, out);
            collect_block_callee_names(else_, out);
        }
        Expr::Binary { op: _, lhs, rhs } => {
            collect_callee_names(lhs, out);
            collect_callee_names(rhs, out);
        }
        Expr::Index { base, index } => {
            collect_callee_names(base, out);
            collect_index_callee_names(index, out);
        }
        Expr::Cast { expr, ty: _ } => collect_callee_names(expr, out),
        Expr::Ref { mutable: _, expr } => collect_callee_names(expr, out),
        // Basis Stage 1a (`.design/basis/01-adts.md`): dead-in-1a ADT
        // expressions, but the collector descends into their sub-expressions
        // so a referenced spec-fn name nested inside is found.
        Expr::StructLit { path: _, fields } => {
            for (_, value) in fields {
                collect_callee_names(value, out);
            }
        }
        Expr::Is {
            scrutinee,
            variant: _,
        } => collect_callee_names(scrutinee, out),
        Expr::Deref(inner) => collect_callee_names(inner, out),
        // The prefix `!` (#92): a spec-fn name could be referenced under it; descend.
        Expr::Unary { expr, .. } => collect_callee_names(expr, out),
        // Cluster C9-B (`.design/basis/10-recursion-tuples.md` REQ-8, #109): a
        // spec-fn name could be referenced in any tuple element or projection
        // receiver — descend into both.
        Expr::Tuple(elems) => {
            for e in elems {
                collect_callee_names(e, out);
            }
        }
        Expr::TupleProj { receiver, .. } => collect_callee_names(receiver, out),
        // A raw quantified formula (`.design/stage2-stratified-cage.md` REQ-0): a
        // callee name can appear in the domain or the body.
        Expr::Quantifier { domain, body, .. } => {
            collect_callee_names(domain, out);
            collect_callee_names(body, out);
        }
        // A string literal (`.design/basis/07-strings.md` REQ-1) is a leaf: no
        // sub-expression, no callee path — it references no spec fn (the no-op
        // leaf arm alongside `IntLit`/`BoolLit`).
        Expr::IntLit { .. } | Expr::BoolLit(_) | Expr::StrLit(_) => {}
    }
}

/// Walk a `Block`'s statements + tail collecting callee names (the `if`-expr arms in
/// a contract clause). A contract clause's `if` carries blocks whose exprs may
/// reference a spec fn.
fn collect_block_callee_names(
    block: &thermite_syntax::Block,
    out: &mut std::collections::BTreeSet<String>,
) {
    for stmt in &block.stmts {
        collect_stmt_callee_names(stmt, out);
    }
    if let Some(tail) = &block.tail {
        collect_callee_names(tail, out);
    }
}

/// Walk a `Stmt` collecting callee names (covers every statement shape so a
/// spec-fn reference nested in a contract-clause block is found).
fn collect_stmt_callee_names(
    stmt: &thermite_syntax::Stmt,
    out: &mut std::collections::BTreeSet<String>,
) {
    use thermite_syntax::Stmt;
    match stmt {
        Stmt::Let {
            mutable: _,
            name: _,
            ty: _,
            init,
        } => collect_callee_names(init, out),
        Stmt::Assign { target, value } => {
            collect_callee_names(target, out);
            collect_callee_names(value, out);
        }
        Stmt::Return(Some(e)) => collect_callee_names(e, out),
        Stmt::Return(None) => {}
        Stmt::If { cond, then, else_ } => {
            collect_callee_names(cond, out);
            collect_block_callee_names(then, out);
            if let Some(else_block) = else_ {
                collect_block_callee_names(else_block, out);
            }
        }
        Stmt::Loop(loop_node) => {
            for inv in &loop_node.invs {
                collect_callee_names(&inv.expr, out);
            }
            collect_callee_names(&loop_node.dec.expr, out);
            collect_block_callee_names(&loop_node.body, out);
        }
        Stmt::Expr(e) => collect_callee_names(e, out),
        // break/continue carry no sub-expression and no callee (#93): no name
        // to collect (the layer-neutral leaf value).
        Stmt::Break | Stmt::Continue => {}
    }
}

/// Walk an `IndexArg` collecting callee names (a contract clause may index with a
/// spec-fn-derived bound, e.g. `xs[..spec_len(xs)]`).
fn collect_index_callee_names(
    index: &thermite_syntax::IndexArg,
    out: &mut std::collections::BTreeSet<String>,
) {
    use thermite_syntax::IndexArg;
    match index {
        IndexArg::Single(e) => collect_callee_names(e, out),
        IndexArg::RangeTo(e) => collect_callee_names(e, out),
        IndexArg::RangeFrom(e) => collect_callee_names(e, out),
        IndexArg::Range(a, b) => {
            collect_callee_names(a, out);
            collect_callee_names(b, out);
        }
    }
}

/// Render a parameter list as declaration text, e.g. `xs: &[u32]` (REQ-1). Reads
/// only the declaration (`name`/`ty`), no body.
fn render_params(params: &[Param]) -> String {
    params
        .iter()
        .map(|p| format!("{}: {}", p.name, render_type(&p.ty)))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Render a `Type` as surface text (REQ-1) — the declaration form a reviewer reads.
/// Deterministic (R-CODE-5).
fn render_type(ty: &Type) -> String {
    use thermite_syntax::PrimType;
    match ty {
        Type::Prim(PrimType::U8) => "u8".to_string(),
        Type::Prim(PrimType::U16) => "u16".to_string(),
        Type::Prim(PrimType::U32) => "u32".to_string(),
        Type::Prim(PrimType::U64) => "u64".to_string(),
        Type::Prim(PrimType::Usize) => "usize".to_string(),
        Type::Prim(PrimType::Bool) => "bool".to_string(),
        Type::Unit => "()".to_string(),
        Type::Array { elem, len } => {
            let len = match len {
                thermite_syntax::ArrayLen::Literal { raw, .. } => raw,
                thermite_syntax::ArrayLen::Const(name) => name,
            };
            format!("[{}; {len}]", render_type(elem))
        }
        Type::Ref { mutable, inner } => {
            format!(
                "&{}{}",
                if *mutable { "mut " } else { "" },
                render_type(inner)
            )
        }
        Type::Slice(inner) => format!("[{}]", render_type(inner)),
        Type::Generic { name, arg } => format!("{}<{}>", name, render_type(arg)),
        // Basis Stage 1a (`.design/basis/01-adts.md` REQ-1/REQ-2/REQ-3): the
        // surface rendering of a user `Named` type or a `Box<T>` is its surface
        // text — the declaration form a reviewer reads (`Account`,
        // `Box<List>`). The neutral value for an infallible surface renderer.
        // Dead-in-1a (an ADT cert is never reviewed — it dies at the validator).
        Type::Named(name) => name.clone(),
        Type::Box(inner) => format!("Box<{}>", render_type(inner)),
        // Basis Stage 4 (`.design/basis/04-collections.md`): the surface rendering
        // of a bounded `Vec<T>` is its surface text `Vec<T>` — the declaration a
        // reviewer reads. The neutral value for the infallible surface renderer.
        Type::Vec(inner) => format!("Vec<{}>", render_type(inner)),
        // Basis Stage 7 (`.design/basis/07-strings.md` REQ-2): the surface
        // rendering of the bounded owned text primitive is its surface text
        // `String` — the declaration a reviewer reads. The neutral value for the
        // infallible surface renderer.
        Type::String => "String".to_string(),
        // Cluster C7 (`.design/basis/09-option-result.md` REQ-1/REQ-2): the surface
        // rendering of the built-in `Option<T>` / `Result<T, E>` is its surface text
        // — the declaration a reviewer reads (`Option<u64>`,
        // `Result<u64, ParseErr>`). The neutral value for the infallible
        // surface renderer.
        Type::Option(inner) => format!("Option<{}>", render_type(inner)),
        Type::Result(ok, err) => {
            format!("Result<{}, {}>", render_type(ok), render_type(err))
        }
        // Cluster C12 (`.design/basis/13-map.md` REQ-1/REQ-5): the surface rendering
        // of the bounded verified `Map<K, V>` is its surface text `Map<K, V>` — the
        // declaration a reviewer reads. The neutral value for the
        // infallible surface renderer.
        Type::Map(k, v) => format!("Map<{}, {}>", render_type(k), render_type(v)),
        // Cluster C9-B (`.design/basis/10-recursion-tuples.md` REQ-5/REQ-7): the
        // surface rendering of an n-tuple type is its surface text `(T, U, …)` —
        // the declaration a reviewer reads. The neutral value for
        // the infallible surface renderer.
        Type::Tuple(tys) => {
            let parts: Vec<String> = tys.iter().map(render_type).collect();
            format!("({})", parts.join(", "))
        }
    }
}

/// Attach a reviewer's collected verdicts to a [`ReviewRecord`] for `file` (REQ-4)
/// — the additive attach. The verdicts are the external reviewer's (forge does not
/// fabricate `aligned`). A pure constructor: it touches no `Certificate` (the
/// cert's `oracle_subset` is structurally untouched — the verdict lives in a
/// separate document, OQ-2 reading (a)).
pub fn attach_verdicts(file: &str, verdicts: Vec<ReviewVerdict>) -> ReviewRecord {
    ReviewRecord {
        file: file.to_string(),
        verdicts,
    }
}

/// Run the external `--reviewer <cmd>` shell-out (REQ-7, OQ-1 — the pluggable
/// integration). Pipes the artifact JSON to `<cmd>`'s stdin, reads the reviewer's
/// [`ReviewVerdict`] JSON from its stdout, and returns the parsed verdicts (the
/// reviewer's judgment — forge does not fabricate `aligned`, R-CODE-5).
///
/// The reviewer may emit either a single `ReviewVerdict` object or a JSON array of
/// them (a reviewer judging multiple items in one pass). Failure modes (the
/// design's "handle the cmd failing/absent gracefully — a `ForgeError`, never a
/// panic"):
///
/// - the cmd is absent (`ENOENT`) → [`ForgeError::ReviewerAbsent`];
/// - the cmd fails to spawn / its stdin pipe breaks → [`ForgeError::ReviewerSpawn`];
/// - the cmd exits non-zero → [`ForgeError::ReviewerFailed`] (its stderr surfaced);
/// - the cmd's stdout is missing / garbage (not a `ReviewVerdict`) →
///   [`ForgeError::ReviewerOutput`] (reported, not a crash and not a fabricated
///   verdict).
///
/// `cmd` is run via the platform shell (`sh -c <cmd>`) so a multi-word command (a
/// script + args, a `cat`-based stub) works as one `--reviewer` argument.
pub fn run_reviewer(
    cmd: &str,
    artifact: &ReviewArtifact,
) -> Result<Vec<ReviewVerdict>, ForgeError> {
    let artifact_json =
        serde_json::to_string_pretty(artifact).map_err(|e| ForgeError::ReviewerOutput {
            detail: format!("failed to serialize the review artifact for the reviewer: {e}"),
        })?;

    let mut child = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ForgeError::ReviewerAbsent {
                    cmd: cmd.to_string(),
                }
            } else {
                ForgeError::ReviewerSpawn {
                    cmd: cmd.to_string(),
                    source: e,
                }
            }
        })?;

    // Write the artifact to the reviewer's stdin, then drop the handle so the
    // reviewer sees EOF (a `cat`-based stub blocks until EOF otherwise).
    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| ForgeError::ReviewerOutput {
                detail: "the reviewer command did not expose a stdin pipe".to_string(),
            })?;
        stdin
            .write_all(artifact_json.as_bytes())
            .map_err(|e| ForgeError::ReviewerSpawn {
                cmd: cmd.to_string(),
                source: e,
            })?;
    }
    // `child.stdin` is dropped at the end of `wait_with_output` is not enough —
    // explicitly take + drop it so the writer end closes and the reviewer sees EOF.
    drop(child.stdin.take());

    let output = child
        .wait_with_output()
        .map_err(|e| ForgeError::ReviewerSpawn {
            cmd: cmd.to_string(),
            source: e,
        })?;

    if !output.status.success() {
        return Err(ForgeError::ReviewerFailed {
            cmd: cmd.to_string(),
            code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_reviewer_verdicts(&stdout).ok_or_else(|| ForgeError::ReviewerOutput {
        detail: format!(
            "the reviewer's stdout was not a `ReviewVerdict` object or array \
             (expected {{\"item\":..,\"aligned\":bool,\"note\":..}}); got:\n{}",
            stdout.trim()
        ),
    })
}

/// Parse the reviewer's stdout into a verdict list (REQ-7): accept either a single
/// [`ReviewVerdict`] object or a JSON array of them. Returns `None` on garbage /
/// missing output (the caller surfaces a [`ForgeError::ReviewerOutput`], not a
/// fabricated verdict).
fn parse_reviewer_verdicts(stdout: &str) -> Option<Vec<ReviewVerdict>> {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(single) = serde_json::from_str::<ReviewVerdict>(trimmed) {
        return Some(vec![single]);
    }
    if let Ok(many) = serde_json::from_str::<Vec<ReviewVerdict>>(trimmed) {
        return Some(many);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{Certificate, Level, RejectReason};

    fn parse_ok(src: &str) -> Program {
        let parsed = thermite_syntax::parse(src);
        assert!(parsed.is_clean(), "fixture must parse clean: {:?}", parsed);
        parsed.program
    }

    // REQ-1/REQ-2: a battery-passing `sum` projects an intent-reviewable entry whose
    // spec layer carries req/ens/fx + spec_sum's declaration and no bodies. The
    // expected clause texts trace to `conformance/sum.th` (R-CHAR-3).
    #[test]
    fn sum_intent_reviewable_no_bodies() {
        let program = parse_ok(include_str!("../../conformance/sum.th"));
        let certs = vec![
            Certificate::new("spec_sum", Level::L3, vec!["pure".to_string()], 0, vec![]),
            Certificate::new("sum", Level::L3, vec!["pure".to_string()], 0, vec![]),
        ];
        let artifact = project_artifact(&certs, &program, None);

        assert_eq!(artifact.battery_failing.len(), 0);
        assert_eq!(
            artifact.intent_reviewable.len(),
            1,
            "only `sum` has a contract; `spec_sum` is a pure dependency, not reviewed"
        );
        let sum = &artifact.intent_reviewable[0];
        assert_eq!(sum.item, "sum");
        assert_eq!(sum.spec_layer.req, "xs.len() <= 1_000_000");
        assert_eq!(
            sum.spec_layer.ens,
            vec![
                "result == spec_sum(xs)".to_string(),
                "result <= xs.len() as u64 * u32::MAX as u64".to_string(),
            ]
        );
        assert_eq!(sum.spec_layer.fx, vec!["pure".to_string()]);

        // spec_sum's declaration is included; its body (the match) is not.
        assert_eq!(sum.spec_layer.referenced_spec_fns.len(), 1);
        let decl = &sum.spec_layer.referenced_spec_fns[0];
        assert_eq!(decl.name, "spec_sum");
        assert_eq!(decl.signature, "spec fn spec_sum(xs: &[u32]) -> u64");
        assert_eq!(decl.dec, "xs.len()");

        // No body tokens anywhere in the serialized artifact (R-DEFER-9 / the
        // "no bodies" rule): sum's accumulator loop + spec_sum's match arms.
        let json = serde_json::to_string(&artifact).expect("serialize");
        for body_token in ["acc", "while", "[head, ..t]", "match", "head as u64"] {
            assert!(
                !json.contains(body_token),
                "the spec layer must EXCLUDE body token `{body_token}`:\n{json}"
            );
        }
    }

    // REQ-2 (R-DEFER-9): a battery-failing fn (a `reject` cert) is flagged
    // battery_failing with its cause and is not surfaced for intent review.
    #[test]
    fn rejected_fn_flagged_not_surfaced() {
        let program = parse_ok("fn f(x: u32) -> u32 req true ens true fx pure { x }");
        let certs = vec![Certificate::rejected(
            "f",
            vec!["pure".to_string()],
            false,
            RejectReason {
                cause: "EnsIsTrivial".to_string(),
                detail: "ens is the literal `true`".to_string(),
            },
        )];
        let artifact = project_artifact(&certs, &program, None);
        assert_eq!(artifact.intent_reviewable.len(), 0, "not surfaced");
        assert_eq!(artifact.battery_failing.len(), 1);
        assert_eq!(artifact.battery_failing[0].item, "f");
        assert_eq!(artifact.battery_failing[0].cause, "EnsIsTrivial");
    }

    // REQ-6 (determinism): same inputs → byte-identical artifact JSON.
    #[test]
    fn artifact_is_deterministic() {
        let program = parse_ok(include_str!("../../conformance/sum.th"));
        let certs = vec![Certificate::new(
            "sum",
            Level::L3,
            vec!["pure".to_string()],
            7,
            vec![],
        )];
        let a = project_artifact(&certs, &program, None);
        let b = project_artifact(&certs, &program, None);
        let ja = serde_json::to_string(&a).expect("a");
        let jb = serde_json::to_string(&b).expect("b");
        assert_eq!(ja, jb);
    }

    // REQ-7: the reviewer verdict parser accepts a single object or an array.
    #[test]
    fn parses_single_and_array_verdicts() {
        let single = parse_reviewer_verdicts(r#"{"item":"sum","aligned":true}"#).expect("single");
        assert_eq!(single.len(), 1);
        assert!(single[0].aligned);
        let many = parse_reviewer_verdicts(
            r#"[{"item":"sum","aligned":true},{"item":"g","aligned":false,"note":"weak"}]"#,
        )
        .expect("array");
        assert_eq!(many.len(), 2);
        assert_eq!(many[1].note.as_deref(), Some("weak"));
        // Garbage → None (the caller surfaces a ForgeError, never a fabricated verdict).
        assert!(parse_reviewer_verdicts("not json").is_none());
        assert!(parse_reviewer_verdicts("").is_none());
    }

    // REQ-4: a verdict attaches to a SEPARATE record (never a Certificate field).
    #[test]
    fn verdict_attaches_to_separate_record() {
        let record = attach_verdicts(
            "conformance/sum.th",
            vec![ReviewVerdict {
                item: "sum".to_string(),
                aligned: true,
                note: Some("matches Appendix A intent".to_string()),
            }],
        );
        assert_eq!(record.file, "conformance/sum.th");
        assert_eq!(record.verdicts.len(), 1);
        assert!(record.verdicts[0].aligned);
    }

    // REQ-2: the [item] filter restricts the artifact to one function.
    #[test]
    fn item_filter_restricts() {
        let program = parse_ok(include_str!("../../conformance/sum.th"));
        let certs = vec![
            Certificate::new("spec_sum", Level::L3, vec!["pure".to_string()], 0, vec![]),
            Certificate::new("sum", Level::L3, vec!["pure".to_string()], 0, vec![]),
        ];
        let artifact = project_artifact(&certs, &program, Some("sum"));
        assert_eq!(artifact.intent_reviewable.len(), 1);
        assert_eq!(artifact.intent_reviewable[0].item, "sum");
    }

    // REQ-9 (increment 3): a certified forge-tier `lemma` carrying a burn receipt surfaces
    // in the review artifact's `burned_lemmas` partition — "like any certified item" — with
    // its proof-token count + cited lemmas; it is not mis-filed as a battery-failing fn (a
    // lemma has no fn contract). A v1 program (no lemma) carries an empty partition.
    #[test]
    fn burned_lemma_surfaces_in_review() {
        let program = parse_ok(
            "lemma melems_cons(n: u32) req n > 0 ens n >= 1 proof { simp [Thermite.denote]; omega }",
        );
        let burned = Certificate::new(
            "melems_cons",
            Level::L3,
            vec!["pure".to_string()],
            0,
            vec![crate::manifest::ObligationResult::discharged("melems_cons")],
        )
        .graduate_triage_clean()
        .with_burn(crate::burn::BurnReceipt::for_proof_text(
            "simp [Thermite.denote]; omega",
        ));
        let artifact = project_artifact(&[burned], &program, None);
        assert_eq!(
            artifact.burned_lemmas.len(),
            1,
            "the certified lemma surfaces"
        );
        assert_eq!(
            artifact.battery_failing.len(),
            0,
            "a certified lemma is not a battery-failing fn"
        );
        assert_eq!(
            artifact.intent_reviewable.len(),
            0,
            "a lemma is not a reviewed fn"
        );
        let l = &artifact.burned_lemmas[0];
        assert_eq!(l.item, "melems_cons");
        assert!(
            l.proof_tokens > 0,
            "the burn receipt's token count is carried"
        );
        assert_eq!(
            l.cited_lemmas,
            vec!["Thermite.denote".to_string()],
            "the cited lemmas are carried from the burn receipt"
        );
    }

    // REQ-9: an UNcertified lemma (a `reject` cert) does not surface as a burned lemma — only
    // a certified item does (the "like any certified item" rule).
    #[test]
    fn uncertified_lemma_does_not_surface_as_burned() {
        let program =
            parse_ok("lemma bad(n: u32) req n > 0 ens n >= 1 proof { simp [Thermite.denote] }");
        let rejected = Certificate::rejected(
            "bad".to_string(),
            vec!["pure".to_string()],
            false,
            RejectReason {
                cause: "LeanUnknown".to_string(),
                detail: "did not discharge".to_string(),
            },
        );
        let artifact = project_artifact(&[rejected], &program, None);
        assert_eq!(
            artifact.burned_lemmas.len(),
            0,
            "an uncertified lemma does not surface"
        );
    }
}
