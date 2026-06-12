# Forge Spec-Intent Review Slot (`forge review`)
<!--
tier: 3-component
status: draft
audited-sha: 63d3953f917fc0161524954d02807b795d9cb432 (bootstrap pin: decision 4 — doc-last-touch, NOT verified-current; backlog #262)
governs: forge/src/review.rs
thesis-refs:
  - thermite-design.md §7
  - thermite-design.md §1
  - thermite-design.md §12 (Risks: "Spec-intent gap")
  - thermite-design.md Appendix B (command surface — `forge review` is an ADDITION)
crosslink: #19 (v0.5 — critic-model spec-review integration)
prereq-blocker: #19
-->

## Summary

`forge review [item]` extracts the **pre-screened spec layer** — the declarative
contract surface (`req`/`ens`/`fx` plus any referenced `spec fn` declarations,
**with no bodies**) for each function that **passed the battery** — and pairs each
contract with an "is this what you meant?" intent-review prompt. It emits this as a
machine artifact (`--json`, for a critic model) and a human form, and it defines the
**pluggable verdict slot**: a structured per-contract `aligned: bool` + optional
`note` form that an **external** reviewer (a human, or a critic model whose only
question is spec-intent alignment) fills and attaches back additively. This is the
§7 "residue surfaced for review" — the one irreducible judgment the deterministic
battery cannot make (thermite-design.md §1 line 26, §12 "Spec-intent gap").

`forge review` does **not** call an LLM. Forge is a deterministic Rust toolchain
(R-CODE-5); a built-in model call would be non-deterministic and require an external
API. #19 provides the **artifact** the critic model consumes and the **verdict
interface** it fills; the model call itself is the integrator's (external) job —
exactly the "pluggable" framing of thermite-design.md §7 line 227 and §summary
line 298.

## Requirements

- **REQ-1 (spec-layer extraction):** for each `fn`, `forge review` extracts the
  DECLARATIVE spec layer — the verbatim `req` clause, every `ens` clause, the `fx`
  effect row, and the **declaration** (name, params, return type, `dec` measure) of
  every `spec fn` the contract references — with **no fn bodies and no spec-fn
  bodies**. This is the §7 "few percent of total line count" surface the reviewer
  reads. Derived from thermite-design.md §7 line 227 ("the certificate includes the
  full spec layer ... pre-screened") and §4.1/§4.2 (the contract surface: `req`/`ens`/
  `fx`, named `spec fn`s).

- **REQ-2 (pre-screening — only battery-passing contracts are intent-reviewable):**
  a contract is surfaced for INTENT review only if it PASSED the mechanical battery
  (non-vacuous per #6/#13, non-trivially-weak per #12, certified/mutation-scored).
  A battery-FAILING contract (a `reject` cert, or a non-certified `Level::L0`) is
  **flagged as battery-failing and NOT surfaced for intent review** — its failure is
  mechanical and answered first (goal.md R-DEFER-9: a vacuous contract is caught
  mechanically, never "passed" by an intent review). Derived from thermite-design.md
  §7 line 227 ("pre-screened to be non-vacuous, non-trivially-weak, and
  mutation-scored ... the reviewer's job is reduced to ... 'is this what I meant?'").

- **REQ-3 (the per-contract intent-review prompt):** each surfaced contract carries a
  structured "is this what you meant?" prompt — the §7 question, naming the item and
  presenting its spec layer, framed so the only open question is spec-intent
  alignment (the mechanical questions already discharged). Derived from
  thermite-design.md §7 line 227.

- **REQ-4 (the pluggable verdict slot — schema + attach point):** `forge review`
  defines the review-verdict INTERFACE: a structured per-contract form
  (`item`, `aligned: bool`, optional `note: String`) that an EXTERNAL reviewer fills.
  The verdict attaches back **additively** (goal.md R-SPEC-2): documented here as an
  additive review record / optional additive manifest field — never a change to the
  frozen oracle subset of `Certificate`. Derived from thermite-design.md §summary
  line 298 ("pluggable critic-model/human review slot") and §1 line 26 (the
  spec-intent question is the irreducible residue a skeptical third party audits).

- **REQ-5 (dual emission — machine + human):** the artifact emits as `--json` (a
  stable schema for a critic model to consume programmatically) and as a human form
  (the same spec layer + prompt, rendered for a person), mirroring the existing
  `forge audit` / `forge check` `--json` + `render_human` precedent in `cli.rs`.
  Derived from thermite-design.md §7 line 227 ("a human, or a critic model").

- **REQ-6 (determinism — R-CODE-5):** the extraction is a PURE PROJECTION of the
  parsed program + the battery verdict (the `Certificate` collection `forge check`
  already produced). Same file → byte-identical spec-layer artifact. No wall clock,
  no un-seeded ordering, no model call. Derived from goal.md R-CODE-5 and
  thermite-design.md §7 (the certificate / spec layer is the deterministic
  deliverable; the verdict is the external reviewer's).

- **REQ-7 (`forge review [item]` command + dispatch):** a `forge review <file>
  [item] [--json]` verb, parsed by `cli::parse_args` and dispatched by `cli::run`,
  reusing `check::check_file` to obtain the battery verdict (the SAME pipeline
  `forge check` / `forge audit` run at `CheckOptions::default` — no extra
  verification, the §7 "the certificate includes the spec layer" framing). An
  optional `[item]` filters to one function. Derived from thermite-design.md
  Appendix B (command-surface shape; `forge review` is an explicit ADDITION — see
  Architecture).

## Acceptance criteria

All ACs tie to a `conformance/review/` oracle the orchestrator authors (this doc
specifies the EXACT fixtures + expected extraction; it does NOT author the oracle —
R-DOC-1, no code/oracle/route changes).

- **AC-1 (spec layer for `sum`):** `forge review conformance/sum.th --json` (or for
  the `sum` item) emits, for `sum`, EXACTLY its declarative spec layer:
  - `req`: `xs.len() <= 1_000_000`
  - `ens`: `[ "result == spec_sum(xs)", "result <= xs.len() as u64 * u32::MAX as u64" ]`
  - `fx`: `pure`
  - referenced spec fns: the **declaration** of `spec_sum` — `spec fn spec_sum(xs: &[u32]) -> u64` with `dec xs.len()` — and **NO body** (the `match xs { ... }` block of `conformance/sum.th` lines 4–7 is EXCLUDED).
  - `sum`'s own body (`conformance/sum.th` lines 15–28, the `let`/`while`/`acc`) is
    EXCLUDED.
  Each clause text is the verbatim `Clause.text` (ast.rs `struct Clause`), not a
  re-rendered form. Expected values trace to `conformance/sum.th` / Appendix A
  (R-CHAR-3 — never copied from `forge`'s own output).

- **AC-2 (intent prompt present, pre-screened):** because `sum` is the certified,
  non-vacuous, mutation-scored corpus program (`conformance/sum.cert.json`: `L3`,
  `tautology:false`, `vacuous_precondition:false`, `mutants_killed:"17/18"`,
  `reject` absent), its spec layer is SURFACED with the per-contract "is this what
  you meant?" prompt naming `sum`. The artifact's verdict slot for `sum` is present
  and UNFILLED (`aligned` unset / awaiting an external reviewer).

- **AC-3 (battery-failing fn flagged, NOT intent-reviewable):** a fixture
  `conformance/review/vacuous.th` whose fn carries a contract the battery REJECTS
  (e.g. `ens true`, structurally rejected per §7 step 1 — its `forge check` cert is a
  `reject` / `Level::L0` cert) appears in the artifact FLAGGED as `battery_failing`
  (with the cert's `reject.cause`) and is **NOT** surfaced with an intent-review
  prompt or a verdict slot. The reviewer is never asked "is this what you meant?"
  about a mechanically-failing contract (REQ-2; R-DEFER-9).

- **AC-4 (determinism):** `forge review conformance/sum.th --json` run twice yields
  byte-identical output (REQ-6, R-CODE-5). A golden `conformance/review/sum.review.json`
  oracle (orchestrator-authored) diffs equal across runs.

- **AC-5 (verdict round-trips additively):** a filled verdict record
  (`{ item: "sum", aligned: true, note: "..." }`) attaches back via the documented
  additive path WITHOUT changing any `Certificate.oracle_subset` field — verified by
  showing the `sum` cert's `oracle_subset` is identical before and after a verdict is
  attached (R-SPEC-2: additive only; the verdict is provenance/judgment, never the
  mechanical verdict).

## Architecture

`forge review` is a **pure projection** layered on top of the existing pipeline,
exactly mirroring how `forge audit` projects the per-fn `Certificate` collection
(`audit::AuditManifest::from_certificates` in `audit.rs` is a PURE PROJECTION,
"it owns no prover invocation" — audit.rs REQ-4). `review` does the same: it runs
`check::check_file` (the default-config entry — `pub fn check_file` in `check.rs`),
parses the file once for the contract surface, and projects.

**`forge review` is an ADDITION to Appendix B.** thermite-design.md Appendix B
(v0.1 command surface) lists `new/goal/fill/edit/check/battery/audit/skill/repair`
but NOT `review`. #19 is a v0.5 item (the roadmap §13 "critic-model spec-review
integration"); `forge review` is its surface verb. This is a sanctioned addition
under the §7/§summary "pluggable review slot" — flagged here so the addition is
explicit (R-SPEC-4: the command surface grows by design intent, not code-local
choice).

**Two data sources, both already present:**

1. **The battery verdict** — the `Vec<Certificate>` from `check::check_file`. Each
   `Certificate` (manifest.rs `struct Certificate`) carries `level: Level`,
   `reject: Option<RejectReason>`, and the `contract_quality` battery block
   (`tautology`, `vacuous_precondition`, `mutants_killed`, `survivor`). The
   **pre-screening predicate** (REQ-2) reads exactly these: a cert is
   **battery-passing / intent-reviewable** iff `reject.is_none()` AND it is a
   certified rung (not the `Level::L0` reject shape `Certificate::rejected` /
   `Certificate::rejected_vacuity` / `Certificate::rejected_weak_contract` produce).
   Everything else is `battery_failing` and is flagged, not surfaced (REQ-2).

2. **The contract surface** — the parsed `Program` (`thermite_syntax::parse`). Each
   `Item::Fn(FnItem)` (ast.rs `enum Item`, `struct FnItem`) exposes
   `FnItem.contract: Contract` — `Contract.req: Clause`, `Contract.ens: Vec<Clause>`,
   `Contract.fx: EffectRow` (ast.rs `struct Contract`). The spec layer is built from
   the verbatim `Clause.text` field (ast.rs `struct Clause` — `text` is the verbatim
   source, the same field `address.rs` resolves against). The **spec-fn references**
   are resolved by walking the contract `Clause` exprs for `Expr::Call`/`Expr::Path`
   nodes (ast.rs `enum Expr`) whose callee name matches a top-level
   `Item::SpecFn(SpecFnItem)`; for each match, the spec layer includes the
   `SpecFnItem` **declaration** — `name`, `params`, `ret`, and the `dec: Clause`
   measure (ast.rs `struct SpecFnItem`) — and **omits `SpecFnItem.body: Block`**
   (the "NO bodies" rule, REQ-1). `FnItem.body: Option<Block>` is likewise never
   emitted.

**Exclusion is structural, not heuristic.** The extraction reads `contract`, `name`,
`params`, `ret`, and `dec` and never touches `body` — so "no bodies" is enforced by
which fields the projection reads, paralleling how `audit::FunctionRow::from_certificate`
projects only the verdict-and-trust fields.

**The pluggable verdict slot (REQ-4)** is the §7/§summary plug. The artifact carries,
per intent-reviewable contract, a verdict slot the EXTERNAL reviewer fills:

```
ReviewVerdict { item: String, aligned: bool, note: Option<String> }
```

The reviewer is external (R-CODE-5: forge does not produce `aligned` — a human or a
critic model does). The verdict attaches back **additively** (R-SPEC-2), following
the additive-field precedent that `slag_meta`/`reject`/`cached`/`solver_profile`/
`strengthening`/`boundary` set on `Certificate`: as an additive review record (a
separate `*.review.json` document) and/or, if attached into the manifest, an additive
`#[serde(default, skip_serializing_if)]` field that is EXCLUDED from
`Certificate::oracle_subset` (manifest.rs `pub fn oracle_subset`) — so attaching a
verdict NEVER changes the mechanical verdict (the soundness invariant the
`oracle_subset` enshrines). See OQ-2 for the open choice between record-vs-field.

**Dual emission (REQ-5)** mirrors `cli.rs`'s established `--json` + human-render
split (`run_audit` emits `--json` or a human summary; the per-fn human render lives
beside it). `forge review --json` is the critic-model artifact; the human form is the
same content for a person.

## Verification

- `cargo test -p forge` — unit fixtures over `review.rs` projecting hand-built
  `(Program, Vec<Certificate>)` pairs: spec-layer-excludes-bodies, spec-fn-decl-included,
  battery-failing-flagged-not-surfaced, determinism (twice-equal), verdict-attach-is-additive
  (`oracle_subset` unchanged).
- **Conformance oracle** `conformance/review/` (orchestrator-authored, the external
  truth — R-CHAR-3): `forge review conformance/sum.th --json` ⇒ diff-equal to the
  golden `conformance/review/sum.review.json` (AC-1/AC-2/AC-4); `forge review
  conformance/review/vacuous.th --json` ⇒ `vacuous` flagged `battery_failing`, no
  intent prompt (AC-3). Expected fields trace to `conformance/sum.th` + Appendix A +
  `conformance/sum.cert.json`, never to forge's own output.
- Gauntlet: `cargo test -p forge`, `cargo clippy -p forge --all-targets -- -D
  warnings`, `cargo fmt --check`.

## REQ status

SHIPPED (#19): `forge/src/review.rs` (the spec-layer extraction + verdict slot +
`--reviewer` shell-out) + the `forge review` verb in `cli.rs` +
`forge/tests/review_conformance.rs` against the `conformance/review/` oracle. The
EXTRACTION is a deterministic pure projection of the battery cert collection
(`check::check_file`) + the parsed contract surface; the verdict is the EXTERNAL
reviewer's (forge never fabricates `aligned`). OQ-1 decided: the thin
`--reviewer <cmd>` shell-out harness IS shipped (artifact → stdin, `ReviewVerdict`
← stdout). OQ-2 decided: reading (a) — a SEPARATE `*.review.json` record, never a
`Certificate` field (the cert's `oracle_subset` is untouched). OQ-3: direct-only
spec-fn references. OQ-4: slag/boundary L1 (reject-free certified rung) ARE
intent-reviewable.

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (spec-layer extraction, no bodies) | SHIPPED | `pub fn review_file` in `review.rs` → `ReviewArtifact`; `SpecLayer::extract` projects `FnItem.contract` (verbatim `Clause.text` for `req`/`ens`/`fx`) + `referenced_spec_fns` (the directly-referenced `SpecFnItem` declarations: name/params/ret/`dec`); `FnItem.body`/`SpecFnItem.body` are NEVER read (structural exclusion). Consumer: `cli::run_review`. Verified: `corpus_sum_intent_reviewable_no_bodies` (no body tokens). |
| REQ-2 (pre-screening — only battery-passing) | SHIPPED | `is_intent_reviewable` (= `manifest::cert_certifies`: reject-free + certified rung) partitions certs in `project_artifact`; a `reject.is_some()` cert becomes a `BatteryFailing` flag carrying `reject.cause`, NOT surfaced. Consumer: `review_file`. Verified: `vacuous_flagged_not_surfaced` (`EnsIsTrivial`, not surfaced). |
| REQ-3 (per-contract intent prompt) | SHIPPED | `IntentReview::prompt` names the item + frames the only-open-question as spec-intent alignment; built in `IntentReview::new`. Consumer: `cli::render_review`. Verified: the `prompt` assertion in `corpus_sum_intent_reviewable_no_bodies`. |
| REQ-4 (pluggable verdict slot — separate record) | SHIPPED | `struct ReviewVerdict { item, aligned, note }` + `struct ReviewRecord` (the separate `*.review.json` document); `attach_verdicts` builds it, `cli::run_review` writes `<file>.review.json`. NEVER a `Certificate` field — the cert's `oracle_subset` is untouched. Verified: `reviewer_shellout_attaches_verdict`. |
| REQ-5 (dual emission machine + human) | SHIPPED | `ReviewArtifact` derives `Serialize` (the `--json` machine form); `cli::render_review` is the human form; `cli::run_review` selects on `--json`. Verified: the `--json` artifact asserted by `review_conformance.rs`. |
| REQ-6 (determinism, R-CODE-5) | SHIPPED | `review_file`/`project_artifact` are a pure projection of the parsed program + cert collection; `referenced_spec_fns` resolves into a sorted-deduplicated set; no wall-clock, no model call. Verified: `artifact_is_deterministic` (unit) + the byte-identical second run in `corpus_sum_intent_reviewable_no_bodies`. |
| REQ-7 (`forge review [item]` command + dispatch + `--reviewer`) | SHIPPED | `cli::parse_args`'s `review` verb (`Command::Review`) + `cli::run_review`; `review::run_reviewer` is the `--reviewer <cmd>` shell-out (artifact → stdin, `ReviewVerdict` ← stdout); a spawn/exit/parse failure is a `ForgeError` (`ReviewerAbsent`/`ReviewerSpawn`/`ReviewerFailed`/`ReviewerOutput`), never a panic. Verified: `reviewer_failure_is_error_not_panic`. |

## Open questions

- **OQ-1 (what "integrate a critic model" means without a built-in LLM call):**
  DECIDED for #19 — `forge review` is the **artifact + the verdict interface**, not a
  model call. Forge stays deterministic (R-CODE-5); a built-in LLM call would be
  non-deterministic and need an API key. "Critic-model spec-review integration"
  (roadmap §13) means: an external agent runs `forge review --json`, reads the
  pre-screened spec layer + intent prompts, answers the per-contract `aligned`
  question, and attaches the verdict back. The model is the PLUG; #19 is the SLOT.
  Least-confident: whether §13 also expects #19 to ship a thin invocation harness
  (a documented `forge review --reviewer <cmd>` shell-out) vs. leaving the call
  entirely to the integrator. This doc takes the narrower reading (artifact + slot
  only); a harness would be a separate, additive verb. Flag for #19 decision.

- **OQ-2 (where the verdict attaches — record vs additive manifest field):** two
  R-SPEC-2-compatible options: (a) a separate `*.review.json` review record keyed by
  item (cleanest separation; the verdict lives outside the cert entirely), or (b) an
  additive `review: Option<ReviewVerdict>` field on `Certificate`/`FunctionRow`,
  `#[serde(default, skip_serializing_if)]` and EXCLUDED from `oracle_subset` (mirrors
  the `slag_meta`/`solver_profile` precedent). This doc documents BOTH as valid and
  leans (a) — a separate record keeps the cert oracle untouched and matches the §1
  "skeptical third party audits the residue" framing (the verdict is the third
  party's annotation, not the toolchain's certificate). Final choice is a #19
  decision; either way the attach is additive and the `oracle_subset` is untouched
  (AC-5). Least-confident decision in this doc.

- **OQ-3 (spec-fn reference resolution — transitive?):** if a referenced `spec fn`
  itself references another `spec fn` (e.g. `spec_sum` calling a helper), does the
  spec layer include the transitive closure of spec-fn declarations or only the
  directly-referenced ones? The corpus (`spec_sum`) is non-transitive, so the corpus
  does not decide it. This doc specifies DIRECTLY-referenced (REQ-1, AC-1); transitive
  closure is a natural extension flagged for #19 if a future fixture needs it. Reading
  declarations (not bodies) makes transitive inclusion cheap, but it grows the "few
  percent" surface, so the default is the minimal direct set.

- **OQ-4 (slag / boundary fns in the review surface):** a `#[slag]` or
  `#[boundary]` fn has a body-UNPROVEN cert (`Certificate::slag_l1` /
  `boundary_l1`, `Level::L1`) but a MANDATORY, battery-relevant contract (§8: "the
  contract is still mandatory"). Its contract IS exactly the property a reviewer
  should intent-review (the body is fiat-trusted, so the contract is the only thing
  standing). This doc's default: slag/boundary fns with a non-rejected L1 cert ARE
  intent-reviewable (their contract passed the mandatory checks); a slag fn whose
  contract was REJECTED is battery-failing like any other. Flag for #19 — the
  pre-screening predicate (REQ-2) must treat L1-by-fiat as battery-passing-for-intent
  (the body proof is exempted, the contract is not).
