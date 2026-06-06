//! `forge/src/review.rs` — the PLUGGABLE SPEC-INTENT REVIEW SLOT (`forge review`,
//! `thermite-design.md` §7 line 227, §summary line 298, issue #19). The §7
//! "residue surfaced for review": the one irreducible judgment the deterministic
//! battery (#6/#12/#13 vacuity + mutation pre-screening) cannot make — "is this
//! contract what you MEANT?".
//!
//! Governing design: `.design/forge/spec-review.md`.
//!
//! `forge review [item]` extracts the PRE-SCREENED DECLARATIVE SPEC LAYER for each
//! `fn` — the verbatim `req`/`ens`/`fx` clauses plus the DECLARATION (name, params,
//! return type, `dec` measure) of every DIRECTLY-referenced `spec fn`, with NO
//! bodies — and pairs each intent-reviewable contract with an "is this what you
//! meant?" prompt. It emits this as a machine artifact (`--json`, for a critic
//! model) and a human form, and defines the PLUGGABLE VERDICT SLOT
//! ([`ReviewVerdict`]) an EXTERNAL reviewer fills.
//!
//! Two data sources, both already shipped (mirroring `audit::AuditManifest`'s pure
//! projection):
//!
//! 1. The battery verdict — the `Vec<Certificate>` from [`crate::check::check_file`]
//!    (the SAME default-config pipeline `forge check`/`forge audit` run — no extra
//!    verification). The PRE-SCREENING predicate ([`is_intent_reviewable`]) reads
//!    `cert.reject`/`cert.level`: a cert is intent-reviewable iff it is reject-free
//!    AND a certified rung (`manifest::cert_certifies` — L1/L2/L3, incl. a slag /
//!    boundary L1 whose contract IS the trust statement, OQ-4). A battery-FAILING
//!    cert (`reject.is_some()`) is FLAGGED [`battery_failing`] with its
//!    `reject.cause` and is NOT surfaced for intent review (R-DEFER-9: the
//!    mechanical failure is answered first).
//! 2. The contract surface — the parsed `Program` (`thermite_syntax::parse`). The
//!    spec layer is built from the verbatim `Clause.text` (`ast.rs`), and the
//!    spec-fn references are resolved by walking the contract clause `Expr`s for a
//!    callee name matching a top-level `Item::SpecFn`. EXCLUSION is STRUCTURAL: the
//!    projection reads `contract`/`name`/`params`/`ret`/`dec` and NEVER touches
//!    `FnItem.body`/`SpecFnItem.body`, so "no bodies" is enforced by which fields
//!    are read (parallel to `audit::FunctionRow::from_certificate`).
//!
//! forge NEVER produces the `aligned` verdict (R-CODE-5): the EXTRACTION (the
//! artifact) is a deterministic pure projection; the verdict is the external
//! reviewer's. The `--reviewer <cmd>` shell-out pipes the JSON artifact to the
//! reviewer's stdin and reads the [`ReviewVerdict`] JSON from its stdout, attaching
//! it as a SEPARATE `*.review.json` record — never a `Certificate` field (OQ-2: the
//! cert is the mechanical verdict; intent review is a separate judgment, never
//! conflated). A failing/absent reviewer cmd is a [`ForgeError`], never a panic.
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (spec-layer extraction, no bodies) | SHIPPED | `pub fn review_file` → [`ReviewArtifact`]; [`SpecLayer::extract`] projects `FnItem.contract` (`req`/`ens`/`fx` verbatim `Clause.text`) + [`referenced_spec_fns`] (the directly-referenced `SpecFnItem` declarations: name/params/ret/`dec`, NO body). `FnItem.body`/`SpecFnItem.body` are never read. Consumer: `cli::run_review`. |
//! | REQ-2 (pre-screening — only battery-passing) | SHIPPED | [`is_intent_reviewable`] (= `manifest::cert_certifies`: reject-free + certified rung) partitions certs; a `reject.is_some()` cert becomes a [`BatteryFailing`] flag carrying `reject.cause`, NOT an [`IntentReview`]. Consumer: `review_file`. |
//! | REQ-3 (per-contract intent prompt) | SHIPPED | [`IntentReview::prompt`] names the item + frames the only-open-question as spec-intent alignment; built in [`IntentReview::new`]. Consumer: `cli::render_review`. |
//! | REQ-4 (pluggable verdict slot — separate record) | SHIPPED | [`ReviewVerdict { item, aligned, note }`] + [`ReviewRecord`] (the separate `*.review.json` document); attached by [`attach_verdicts`], written to `<file>.review.json` by `cli::run_review`. NEVER a `Certificate` field (the cert's `oracle_subset` is untouched). |
//! | REQ-5 (dual emission machine + human) | SHIPPED | [`ReviewArtifact`] derives `Serialize` (the `--json` machine form); `cli::render_review` is the human form. Consumer: `cli::run_review`. |
//! | REQ-6 (determinism, R-CODE-5) | SHIPPED | [`review_file`] is a pure projection of the parsed program + the cert collection; spec-fn references are resolved into a sorted-deduplicated set ([`referenced_spec_fns`]); no wall-clock, no model call. The EXTRACTION is byte-identical across runs. |
//! | REQ-7 (`forge review [item]` command + dispatch + --reviewer) | SHIPPED | `cli::parse_args`'s `review` verb + `cli::run_review`; [`run_reviewer`] is the `--reviewer <cmd>` shell-out (artifact → stdin, `ReviewVerdict` ← stdout); a spawn/parse failure is a `ForgeError`, never a panic. |

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use thermite_syntax::{Contract, Expr, Item, Param, Program, Type};

use crate::check;
use crate::cli::ForgeError;
use crate::manifest::{cert_certifies, effects_of, Certificate};

/// `true` iff a certificate is INTENT-REVIEWABLE (REQ-2): it PASSED the mechanical
/// battery — reject-free AND a certified rung (`manifest::cert_certifies`: L1/L2/L3,
/// including a `#[slag]` / `#[boundary]` L1 whose contract IS the trust statement a
/// reviewer should audit, OQ-4). A battery-FAILING cert (`reject.is_some()`:
/// vacuity / weak-contract / counterexample / timeout) is NOT intent-reviewable —
/// its failure is mechanical and answered first (R-DEFER-9). A thin alias over
/// `cert_certifies` so the review pre-screen and the project headline agree on what
/// "passed the battery" means.
pub fn is_intent_reviewable(cert: &Certificate) -> bool {
    cert_certifies(cert)
}

/// The DECLARATION of one `spec fn` referenced by a reviewed contract (REQ-1) — a
/// BODY-FREE projection of a `SpecFnItem`. The §7 "few percent" surface the reviewer
/// reads to understand what the contract's `spec fn` MEANS, WITHOUT the body
/// (`SpecFnItem.body` is never read — the "no bodies" rule is structural).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecFnDecl {
    /// The spec fn name (the callee a contract clause references).
    pub name: String,
    /// The declaration signature rendered WITHOUT the body, e.g.
    /// `spec fn spec_sum(xs: &[u32]) -> u64`. Built from `name`/`params`/`ret`.
    pub signature: String,
    /// The `dec` decreases-measure clause text (verbatim `Clause.text`), e.g.
    /// `xs.len()`. The well-formedness measure — declaration, never body.
    pub dec: String,
}

impl SpecFnDecl {
    /// Project a `SpecFnItem` to its body-free declaration (REQ-1). Reads `name`,
    /// `params`, `ret`, and `dec` — NEVER `body` (the §7 "no bodies" rule, enforced
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

/// The DECLARATIVE SPEC LAYER of one reviewed `fn` (REQ-1) — the verbatim contract
/// surface a reviewer reads, with NO bodies. The §7 "the certificate includes the
/// full spec layer". Built by [`SpecLayer::extract`] from `FnItem.contract` (the
/// verbatim `Clause.text`) + the directly-referenced `spec fn` declarations; the
/// `fn`'s own body and every spec fn's body are EXCLUDED (structural — the
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
    /// The DECLARATIONS of the directly-referenced `spec fn`s (OQ-3 direct-only),
    /// sorted + deduplicated by name (deterministic, R-CODE-5). NO bodies.
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

/// One INTENT-REVIEWABLE function in the artifact (REQ-2/REQ-3) — a battery-passing
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
    /// line 227). Names the item and frames the ONLY open question as spec-intent
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

/// One BATTERY-FAILING function in the artifact (REQ-2) — a contract the battery
/// REJECTED (vacuity / weak-contract / counterexample / timeout). FLAGGED with its
/// `reject.cause` and NOT surfaced for intent review (R-DEFER-9: the reviewer is
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

/// The SPEC-INTENT REVIEW ARTIFACT (REQ-1/REQ-2/REQ-5) — the machine + human
/// deliverable `forge review` emits. A PURE PROJECTION (REQ-6) of the parsed
/// program and the battery cert collection, partitioning the file's `fn`s into the
/// INTENT-REVIEWABLE (battery-passing, with spec layers + prompts) and the
/// BATTERY-FAILING (flagged, not surfaced). The `--json` form is the critic-model
/// surface; `cli::render_review` is the human form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewArtifact {
    /// The intent-reviewable functions (battery-passing) — surfaced with spec
    /// layers + prompts, in source order (REQ-2).
    pub intent_reviewable: Vec<IntentReview>,
    /// The battery-failing functions (flagged, NOT surfaced for intent review), in
    /// source order (REQ-2; R-DEFER-9).
    pub battery_failing: Vec<BatteryFailing>,
}

/// The PLUGGABLE VERDICT SLOT (REQ-4, OQ-2) — the structured per-contract judgment
/// an EXTERNAL reviewer (a human, or a critic model whose only question is
/// spec-intent alignment) fills. forge NEVER fabricates `aligned` (R-CODE-5): this
/// is the reviewer's annotation, read from the `--reviewer <cmd>`'s stdout and
/// attached as a SEPARATE `*.review.json` record — never a `Certificate` field (the
/// cert is the mechanical verdict; this is the spec-intent judgment, never
/// conflated).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewVerdict {
    /// The function the verdict is about (must match an `intent_reviewable` item).
    pub item: String,
    /// The reviewer's judgment: does the contract say what the author MEANT?
    pub aligned: bool,
    /// An optional reviewer note (the "why" / the suggested correction).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// The SEPARATE review record written to `<file>.review.json` (REQ-4, OQ-2 reading
/// (a)). It keeps the verdict OUTSIDE the certificate entirely — the §1 "skeptical
/// third party audits the residue" framing (the verdict is the third party's
/// annotation, not the toolchain's certificate). A pure data document: the file
/// path it reviews + the collected per-item verdicts. Attaching a verdict NEVER
/// touches any `Certificate`'s `oracle_subset` (the soundness invariant, R-SPEC-2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewRecord {
    /// The reviewed file (the artifact's provenance).
    pub file: String,
    /// The per-item verdicts the external reviewer filled. Empty until a reviewer
    /// runs (the artifact-only path).
    pub verdicts: Vec<ReviewVerdict>,
}

/// Run the spec-intent extraction for `path` (REQ-1/REQ-2/REQ-6). Runs the SAME
/// default-config check pipeline `forge check` / `forge audit` run
/// ([`check::check_file`] — no extra verification), parses the file once for the
/// contract surface, and PROJECTS the two into the [`ReviewArtifact`]:
///
/// - a cert that PASSED the battery ([`is_intent_reviewable`]) → an [`IntentReview`]
///   carrying its body-free [`SpecLayer`] + the §7 prompt;
/// - a cert the battery REJECTED → a [`BatteryFailing`] flag (NOT surfaced).
///
/// PURE PROJECTION (REQ-6): the artifact is a deterministic function of the parsed
/// program + the cert collection — no wall-clock, no model call. An optional
/// `item_filter` (the `forge review <file> [item]` positional) restricts the
/// artifact to a single function (both partitions are filtered).
pub fn review_file(
    path: impl AsRef<Path>,
    item_filter: Option<&str>,
) -> Result<ReviewArtifact, ForgeError> {
    let path = path.as_ref();

    // The SAME default pipeline `forge check`/`forge audit` run (the battery
    // verdict; REQ-2). `review` re-runs no verus — it projects this collection.
    let certs = check::check_file(path)?;

    // Parse the file once for the contract surface (REQ-1). `check_file` already
    // validated it parses clean, so this is a re-parse of a known-good file
    // (deterministic, R-CODE-5), never a re-verification — the `audit` precedent.
    let src = std::fs::read_to_string(path).map_err(|e| ForgeError::Io {
        path: path.display().to_string(),
        source: e,
    })?;
    let parsed = thermite_syntax::parse(&src);
    if !parsed.is_clean() {
        return Err(ForgeError::Parse(parsed.errors));
    }

    Ok(project_artifact(&certs, &parsed.program, item_filter))
}

/// Project a settled cert collection + parsed program into the [`ReviewArtifact`]
/// (REQ-1/REQ-2/REQ-6) — the PURE PROJECTION core, split out so it is unit-testable
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
            Item::Fn(_) => None,
            // Basis Stage 1a (`.design/basis/01-adts.md`): a `struct`/`enum`
            // item is not a `spec fn` — it contributes nothing to the
            // referenced-spec-fn projection (neutral value `None`). Dead-in-1a
            // (an ADT program dies at the validator before a cert is reviewed).
            Item::Struct(_) | Item::Enum(_) => None,
        })
        .collect();

    let mut intent_reviewable = Vec::new();
    let mut battery_failing = Vec::new();

    for cert in certs {
        if let Some(filter) = item_filter {
            if cert.item != filter {
                continue;
            }
        }
        // A `spec fn` carries no `req`/`ens`/`fx` contract (§4.2) — it is a pure
        // shared dependency the reviewed `fn`s' spec layers REFERENCE, never an
        // intent-reviewed item itself. Skip it (it has no contract to review).
        let contract = match lookup_fn_contract(program, &cert.item) {
            Some(c) => c,
            None => continue,
        };

        if is_intent_reviewable(cert) {
            let spec_layer = SpecLayer::extract(contract, &spec_fns);
            intent_reviewable.push(IntentReview::new(cert.item.clone(), spec_layer));
        } else if let Some(reject) = &cert.reject {
            // Battery-FAILING (R-DEFER-9): flagged with its cause, NOT surfaced for
            // intent review. A non-certifying cert always carries a `reject`
            // (`Certificate::rejected*` / `timeout`); a defensive `None` (an
            // un-discharged L0 with no reject) is recorded with an explicit cause so
            // nothing mechanically-failing is silently dropped.
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

    ReviewArtifact {
        intent_reviewable,
        battery_failing,
    }
}

/// Look up a `fn`'s [`Contract`] in the parsed program by name (REQ-1). Returns the
/// contract of the matching `Item::Fn`, or `None` (a `spec fn` carries no contract,
/// and a name with no node has none). Pure read of the parsed AST — no re-parsing,
/// no re-verification (the `audit::lookup_contract` precedent).
fn lookup_fn_contract<'a>(program: &'a Program, name: &str) -> Option<&'a Contract> {
    program.items.iter().find_map(|item| match item {
        Item::Fn(f) if f.name == name => Some(&f.contract),
        _ => None,
    })
}

/// Resolve the DIRECTLY-referenced `spec fn` declarations of a contract (REQ-1,
/// OQ-3 direct-only). Walks every `req`/`ens` clause `Expr` for a callee name
/// matching a top-level `SpecFnItem`, and projects each match to its body-free
/// [`SpecFnDecl`]. The result is sorted + deduplicated by name (deterministic,
/// R-CODE-5 — a clause referencing `spec_sum` twice yields one declaration). NO
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

/// Walk an `Expr` collecting every CALLEE name that is a plain path (a free
/// `f(args)` call or a bare `Path`), recursing into every sub-expression so a
/// reference nested in a binary/cast/index/method-call/etc. is found. Used to
/// resolve a contract's directly-referenced `spec fn`s. Reads only the expression
/// shape — never a body (a contract clause holds no body).
fn collect_callee_names(expr: &Expr, out: &mut std::collections::BTreeSet<String>) {
    match expr {
        Expr::Path(segments) => {
            // A bare path: the LAST segment is the referenced name (`spec_sum`,
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
        // expressions, but the honest collector descends into their
        // sub-expressions so a referenced spec-fn name nested inside is found.
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
        Expr::IntLit { .. } | Expr::BoolLit(_) => {}
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
        Type::Prim(PrimType::U32) => "u32".to_string(),
        Type::Prim(PrimType::U64) => "u64".to_string(),
        Type::Prim(PrimType::Usize) => "usize".to_string(),
        Type::Prim(PrimType::Bool) => "bool".to_string(),
        Type::Unit => "()".to_string(),
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
        // SURFACE rendering of a user `Named` type or a `Box<T>` is its surface
        // text — the faithful declaration form a reviewer reads (`Account`,
        // `Box<List>`). This is the honest neutral value for an infallible
        // surface renderer, NOT a stub. Dead-in-1a (an ADT cert is never
        // reviewed — it dies at the validator).
        Type::Named(name) => name.clone(),
        Type::Box(inner) => format!("Box<{}>", render_type(inner)),
        // Basis Stage 4 (`.design/basis/04-collections.md`): the SURFACE rendering
        // of a bounded `Vec<T>` is its surface text `Vec<T>` — the faithful
        // declaration a reviewer reads. The honest neutral value for the
        // infallible surface renderer, NOT a stub.
        Type::Vec(inner) => format!("Vec<{}>", render_type(inner)),
    }
}

/// Attach a reviewer's collected verdicts to a [`ReviewRecord`] for `file` (REQ-4)
/// — the additive attach. The verdicts are the EXTERNAL reviewer's (forge never
/// fabricates `aligned`). A pure constructor: it touches NO `Certificate` (the
/// cert's `oracle_subset` is structurally untouched — the verdict lives in a
/// separate document, OQ-2 reading (a)).
pub fn attach_verdicts(file: &str, verdicts: Vec<ReviewVerdict>) -> ReviewRecord {
    ReviewRecord {
        file: file.to_string(),
        verdicts,
    }
}

/// Run the EXTERNAL `--reviewer <cmd>` shell-out (REQ-7, OQ-1 — the pluggable
/// integration). Pipes the artifact JSON to `<cmd>`'s STDIN, reads the reviewer's
/// [`ReviewVerdict`] JSON from its STDOUT, and returns the parsed verdicts (the
/// reviewer's judgment — forge never fabricates `aligned`, R-CODE-5).
///
/// The reviewer may emit EITHER a single `ReviewVerdict` object OR a JSON array of
/// them (a reviewer judging multiple items in one pass). Graceful failure (the
/// design's "handle the cmd failing/absent gracefully — a `ForgeError`, never a
/// panic"):
///
/// - the cmd is ABSENT (`ENOENT`) → [`ForgeError::ReviewerAbsent`];
/// - the cmd fails to spawn / its stdin pipe breaks → [`ForgeError::ReviewerSpawn`];
/// - the cmd exits NON-ZERO → [`ForgeError::ReviewerFailed`] (its stderr surfaced);
/// - the cmd's stdout is MISSING / GARBAGE (not a `ReviewVerdict`) →
///   [`ForgeError::ReviewerOutput`] (reported, never a crash and never a fabricated
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

/// Parse the reviewer's stdout into a verdict list (REQ-7): accept EITHER a single
/// [`ReviewVerdict`] object OR a JSON array of them. Returns `None` on garbage /
/// missing output (the caller surfaces a [`ForgeError::ReviewerOutput`] — never a
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
    // spec layer carries req/ens/fx + spec_sum's DECLARATION and NO bodies. The
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

        // spec_sum's DECLARATION is included; its body (the match) is NOT.
        assert_eq!(sum.spec_layer.referenced_spec_fns.len(), 1);
        let decl = &sum.spec_layer.referenced_spec_fns[0];
        assert_eq!(decl.name, "spec_sum");
        assert_eq!(decl.signature, "spec fn spec_sum(xs: &[u32]) -> u64");
        assert_eq!(decl.dec, "xs.len()");

        // NO body tokens anywhere in the serialized artifact (R-DEFER-9 / the
        // "no bodies" rule): sum's accumulator loop + spec_sum's match arms.
        let json = serde_json::to_string(&artifact).expect("serialize");
        for body_token in ["acc", "while", "[head, ..t]", "match", "head as u64"] {
            assert!(
                !json.contains(body_token),
                "the spec layer must EXCLUDE body token `{body_token}`:\n{json}"
            );
        }
    }

    // REQ-2 (R-DEFER-9): a battery-FAILING fn (a `reject` cert) is flagged
    // battery_failing with its cause and is NOT surfaced for intent review.
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
}
