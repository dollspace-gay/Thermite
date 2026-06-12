# Audit Manifest Format v1 — the Trust Deliverable
<!--
tier: 3-component
status: draft
audited-sha: a8b2f71d59940dc26fd30622ded03708ac978078 (bootstrap pin: decision 4 — doc-last-touch, NOT verified-current; backlog #262)
governs: forge/src/audit.rs
thesis-refs:
  - thermite-design.md §6
  - thermite-design.md §8
  - thermite-design.md §9
  - thermite-design.md Appendix A
  - thermite-design.md Appendix B
-->

## Summary

`thermite-design.md` §6 promises: "The certificate attached to a build artifact
lists every function's level, every `#[slag]` block, and the contract-quality
scores from §7. This manifest **is** the deliverable's trust statement." This
component is that aggregate manifest: a **stable, versioned project-level
document** (the `AuditManifest` v1 schema) and the `forge audit <file>` command
that emits it (JSON + a human summary). It aggregates the per-function
certificates `forge check` already produces (`Certificate`, `manifest.rs`) and
the project assurance aggregate (`AssuranceManifest`, `manifest.rs`) into one
trust statement whose centerpiece is the **enumerable trusted computing base**
(§9): the explicit lists of every `#[slag]` block ∪ every boundary contract ∪
the toolchain identity. `grep slag` over a codebase and the audit manifest's TCB
section are the same complete inventory of fiat-trusted code (§8).

This is **greenfield**: there is no `forge audit` command, no `forge/src/audit.rs`,
and no aggregate `AuditManifest` schema in `forge` today (verified below — the
only existing aggregate, `AssuranceManifest`, is a render-time level/scope
headline, not the §6/§9 audit deliverable). ALL the underlying per-fn data ships
(it is this component's *input*, never re-derived). REQ-1..REQ-6 are **SHIPPED**
(`forge/src/audit.rs` + the `forge audit` verb in `cli.rs`), verified by
`forge/tests/audit_conformance.rs` against `conformance/audit/cases.json`;
tracked by **crosslink issue #15** (v0.3, milestone #3 Battery). The **#274
amendment** adds the LEAN-FRAGMENT MEMBERSHIP REPORT (REQ-7..REQ-10, all
**NOT-STARTED**, blocker #274) — a per-function, informational section answering:
would `--engine lean` attempt this fn, and if not, what is the structured refusal.

## Decided scope

Issue #15 = the **audit manifest v1** (a STABLE aggregate format = THE trust
deliverable, a versioned schema under R-SPEC-2/R-SPEC-3) + the **`forge audit`
command** that emits it. The manifest AGGREGATES the existing per-fn certs + the
assurance manifest; it does **NOT** re-derive any verdict. Explicitly OUT (these
are inputs/boundaries, never deferred-as-status — they all SHIP):

- The per-fn `Certificate` schema — `level`, `contract_quality` (§7),
  `slag`/`slag_meta`, `boundary`/`boundary_target`, `assurance_scope` — is
  **#5/#6/#13/#16/#17** (`.design/forge/certificate-manifest.md`,
  `slag.md`, `solver-vacuity.md`, `.design/boundary/ffi-boundary.md`,
  `e2e-vs-boundary.md`, all SHIPPED). #15 *reads* these fields; it never adds or
  recomputes them.
- The per-fn `AssuranceManifest` / `ProjectAssurance` (min-over-functions level)
  / `ProjectScope` (§9 end-to-end vs to-the-boundary) aggregate is **#10/#17**
  (`.design/forge/degrade-ladder.md`, `e2e-vs-boundary.md`, SHIPPED). #15 EMBEDS
  this as the manifest's project-assurance section.
- The proof cache (#8, `.design/forge/proof-cache.md`) and the degrade ladder
  (#10) are *inputs* to the certificate collection `forge audit` aggregates —
  not part of this component.
- The check pipeline (`check::check_file_with_options`,
  `.design/forge/check.md`) is the producer `forge audit` calls to obtain the
  cert collection. #15 wraps it, it does not reimplement it.
- **(#274 amendment)** The Lean exporter/engine themselves (`pub fn export_item
  in lean_export.rs`, `LeanEngine` in `engine.rs`, `.design/verified/
  proof-backends.md`) are this report's *substrate*, never re-implemented here.
  The report REUSES the shipped export probe; it adds **no lowering work, no
  recognizer fork** (see REQ-8). Certificate **engine attribution stays
  untouched**: `Certificate.engine_attribution` is "`Some` ONLY when a
  non-default engine discharged (the default Verus path leaves it `None`)"
  (`manifest.rs`) — the byte-identity decision for the default path. Fragment
  visibility belongs in the AUDIT report, not in certs; this amendment changes
  ZERO `Certificate` bytes.

## The AuditManifest v1 schema (the stable contract)

The manifest is a single project-level document. Because it IS the deliverable's
trust statement (§6) and a stable contract (R-SPEC-2/R-SPEC-3), the schema
carries an explicit `manifest_version` format tag (`"v1"`) so a downstream
consumer can pin and evolve the format additively (the per-fn `Certificate`
additive-field precedent in `manifest.rs`: `#[serde(default, skip_serializing_if)]`).

The v1 field set, in three sections (a fourth, additive section is the #274
amendment — next heading):

1. **`functions`** — the per-function rows, one per checked `Item::Fn` in source
   order. Each row carries the verdict-and-trust-relevant projection of that
   fn's `Certificate`:
   - `name` (the item),
   - `level` (`L0..L3`, the ladder rung),
   - `assurance_scope` (§9: end-to-end vs to-the-boundary, from
     `Certificate.assurance_scope`),
   - `contract_quality` (the §7 battery block: `tautology`,
     `vacuous_precondition`, `mutants_killed`, `survivor` — from
     `Certificate.contract_quality`),
   - `slag` (the §8 fiat-trust flag),
   - `boundary` + `boundary_target` (the §9 FFI-crossing flag + foreign path).

2. **`project_assurance`** — the project-level trust headline, embedding the
   existing `AssuranceManifest` aggregate (#10/#17):
   - the `ProjectAssurance` headline (the min-over-functions level when every fn
     certifies, else `Failed` — §5.2),
   - the `ProjectScope` (§9: END-TO-END iff every fn is, else TO-THE-BOUNDARY
     listing the reached crossings),
   - the list of **lowered-assurance** fns (the #10 auto-degraded items —
     `FunctionAssurance.lowered_assurance`), so a reader sees which levels were
     proved vs degraded.

3. **`tcb`** — the **enumerable trusted computing base** (§9: "exactly (slag
   blocks ∪ boundary contracts ∪ the toolchain itself)"), the manifest's
   centerpiece and the R-DEFER-9 honesty surface:
   - `slag_blocks` — every `#[slag]` fn: `name` + its `reason`/`owner`/`review`
     (from `Certificate.slag_meta` — §8's mandatory justification),
   - `boundary_contracts` — every `#[boundary]` fn: `name` + the foreign
     `boundary_target` + its enforced contract (the `req`/`ens`/`fx`, §9
     per-function contracts),
   - `toolchain` — the toolchain identity: the `verus` version
     (`resolve_verus_version` in `check.rs`) and the `thermite`/`forge` version
     (`THERMITE_VERSION = env!("CARGO_PKG_VERSION")` in `check.rs`).

The TCB section is EMPTY of `slag_blocks` and `boundary_contracts` for a
pure-Thermite project (only the `toolchain` entry remains — the irreducible base
every artifact trusts). That empty-but-for-toolchain state is the §9 "verified,
period" claim, mechanically witnessed.

### Determinism (R-CODE-5)

The manifest is a **pure, deterministic function** of the certificate collection
plus the two pinned version strings. Every field traces to a deterministic cert
field. The single non-deterministic cert field, `solver_time_ms` (§5.3,
wall-clock), is **excluded** from the manifest (the `Certificate::oracle_subset`
precedent — `solver_time_ms` is structurally absent from the oracle tuple). The
`mutants_killed`/`survivor` battery fields are verus-version-sensitive (the
`certificate-manifest.md` / `mutation-scoring.md` precedent: oracle-EXCLUDED from
the per-cert oracle); the audit fixtures therefore pin `verus` via the
`VERUS_VERSION` seam (`resolve_verus_version`) so the corpus manifest is
reproducible, and the audit oracle asserts `contract_quality` *presence/shape*,
not the version-sensitive ratio string (OQ-2).

## The Lean-fragment membership report (the #274 amendment — NOT-STARTED)

The non-breaking half of outside-review item 7: `forge audit` gains a
**per-function report of Lean-exportable-fragment membership**. For each
`functions` row in the audited file the report answers: IS this item inside the
Lean engine's exportable fragment (would `--engine lean` attempt it), and if
NOT, the **structured refusal class** — surfaced verbatim from the shipped
`ExportRefusal`. Rationale: certificate attribution stays `None` on the default
Verus path (the byte-identity decision — untouched here, see "Decided scope");
the audit report — the project's trust/visibility document — is where fragment
visibility belongs. The section is **informational**: it gates nothing, changes
no exit code, and alters no verdict (REQ-10).

### Report shape (the additive `lean_fragment` section)

A fourth top-level `AuditManifest` section, one row per `functions` row (so it
covers checked `fn`s AND `spec fn`s — both receive certs and both are
`export_item` subjects), in the same source order (R-CODE-5):

```json
"lean_fragment": {
  "functions": [
    { "name": "count",    "exportable": true,  "tier": "auto",
      "tier_tag": "fuel-free-auto" },
    { "name": "spec_sum", "exportable": true,  "tier": "interactive",
      "tier_tag": "recursive-interactive" },
    { "name": "sum",      "exportable": false, "tier": "none",
      "refusal": { "class": "OutOfFragment",
                   "reason": "out-of-fragment construct (not in S's frozen subset): …" } }
  ]
}
```

Per-row fields:

- `name` — the item (matches the `functions` row).
- `exportable: bool` — `export_item` returned `Ok(ExportedObligation)`.
- `tier` — the coarse attempt class:
  - `"auto"` iff exportable and `ExportTier::is_auto()` (tiers (a)/(b):
    `FuelFreeAuto`/`StaticUnfoldAuto`) — `--engine lean` would export AND
    lake-invoke the auto battery;
  - `"interactive"` iff exportable and `RecursiveInteractive` (tier (c)) —
    `--engine lean` would export but NOT invoke lake (the engine returns
    `Unknown`; the theorem is interactive-only);
  - `"none"` iff refused — `--engine lean` would honestly skip
    (`Verdict::Unknown`, "the fragment does not admit this obligation").
- `tier_tag` — the fine-grained shipped tag, `ExportTier::tag()`
  (`"fuel-free-auto"` / `"static-unfold-auto"` / `"recursive-interactive"`);
  present iff `exportable` (`#[serde(skip_serializing_if = "Option::is_none")]`).
- `refusal: Option<{class, reason}>` — present iff NOT exportable:
  - `class` — the `ExportRefusal` **variant name**, a stable enum surface:
    `OutOfFragment` | `NotPureContract` | `IncompleteRegistry` | `NonIntResult`
    | `OpenHole` | `LoopBody` | `OptResResult` (the post-(v) inventory in
    `pub enum ExportRefusal in lean_export.rs`);
  - `reason` — the refusal's `Display` rendering, **verbatim** (e.g. the
    REQ-11.5 loud inventory: "loop-class body (§4.1.7 — S_B mechanizes NO loop
    form …)"). Human diagnostic; class is the machine-stable field (OQ-5).

Membership is scoped to the item's **CONTRACT certification obligation** (the
head of the per-item set — the obligation `LeanEngine::discharge` exports via
`export_item`, `check.rs` passes `&obligations.contract`); the
REGISTRY-TERMINATION obligation is engine-internal and not separately reported.
A `slag` or `boundary` fn reports its *structural* membership like any other row
(a boundary fn has no in-language body → the shipped refusal
`ExportRefusal::NotPureContract("fn `…` is a boundary fn (foreign body, no
in-language body)")` in `export_item`); the check pipeline never routes a
slag/boundary item to an engine anyway, and the same fn's `functions` row
carries the disambiguating `slag`/`boundary` flags.

### Probe mechanics (dry-run export — reuse, never fork)

**Decision: the membership probe IS a dry-run `export_item` call, NOT an
extracted recognizer-only predicate.** Grounding:

- **The probe exists and is pure.** `pub fn export_item(obligation: &Obligation,
  program: &Program, item: &Item) -> Result<ExportedObligation, ExportRefusal>`
  in `lean_export.rs` is string-building only — `lean_export.rs` contains **no
  `std::fs`, no `process::Command`, no `std::env`** use (verified by grep over
  the module). The filesystem/lake side effects live exclusively downstream in
  `LeanEngine::discharge`/`run_lake` in `engine.rs` ("export → write to a
  scratch dir → `lake env lean <file>`"); calling `export_item` alone touches
  neither. The successful result carries the tier (`ExportedObligation.tier`,
  classified by `tier_of`/`registry_is_recursive` in `lean_export.rs`).
- **Per-item dry-run export is a precedented hot-path cost.**
  `LeanEngine::obligation_content_hash` in `engine.rs` ALREADY runs the full
  `find_item` + `export_item` per obligation on every Lean cache-key
  computation ("On an export refusal … the content degrades to a STRUCTURED
  refusal marker"). Running it once per fn during `forge audit` — which has just
  run the full verus check pipeline — is negligible by comparison.
- **A recognizer-only predicate would be a fork.** The fragment is defined
  arm-by-arm by the exporter itself (`encode_expr`/`encode_exec_stmt`/
  `recognize_while_body` — the EXP drift-tripwire surface of
  `.design/verified/rust-lean-correspondence.md`); a parallel predicate would
  drift from the real admission decision and could misreport membership. The
  dry-run probe is by construction the SAME decision `--engine lean` makes
  (`LeanEngine::export` → `export_item`; a refusal maps to the `Unknown` skip in
  `discharge`).

Probe assembly per row (the builder's contract):

1. `lean_export::find_item(&parsed.program, &row.name)` — `cli::run_audit`
   already re-parses the file for the TCB boundary contracts, so the program is
   in hand. A `None` (defensive; certs come from the same file) reports
   `exportable: false` with the engine's own marker class (`OutOfFragment`,
   "item not found" — mirroring `LeanEngine::export`).
2. Mint the item's CONTRACT obligation with **the SAME #226 full
   expression-position closure the check pipeline uses** —
   `Obligation::contract_for_fn`/`contract_for_spec_fn` (`obligation.rs`) fed by
   `reachable_spec_fn_names_full`/`…_full_spec` (`check.rs`, currently private;
   the builder exposes a crate-visible seam, e.g. via `mint_item_obligations`).
   **Re-implementing the closure is forbidden**: `export_item`'s HARD GATE
   cross-checks the obligation's `called` closure against the spec-calls
   actually appearing in `req ∪ ens ∪ body ∪ dec`, so a forked/weaker closure
   would yield spurious `IncompleteRegistry` refusals (or mask real ones — the
   Pin B/C/G bottom-poisoning surface).
3. `export_item(&contract, &program, item)` → `Ok` ⇒ `exportable: true` +
   tier mapping; `Err(refusal)` ⇒ `exportable: false` + `{class, reason}`.

No lake, no scratch file, no `lean/` package access: the probe never reads
`lean_root`, never computes the spine hash, never spawns a process. Determinism
(REQ-6 extended): `export_item` is a pure function of (obligation, program,
item) and the refusal `Display` strings are static formats — same input file ⇒
byte-identical `lean_fragment` section.

### Renderings

- **JSON**: the `lean_fragment` section above, ALWAYS emitted (deterministic,
  not environment-gated — the probe needs no Lean toolchain), `#[serde(default)]`
  on the field so a pre-amendment v1 document still deserializes (AC-5
  discipline). `manifest_version` stays `"v1"` — this is exactly the additive
  evolution REQ-1/AC-5 reserved.
- **Human** (`render_audit` in `cli.rs`): a line-oriented, greppable section
  (the OQ-1 precedent), e.g.:

  ```text
  lean fragment:
    count L3 exportable tier=auto (fuel-free-auto)
    sum L3 NOT-exportable refusal=OutOfFragment: out-of-fragment construct …
  ```

### Consumption: informational, no gate

**Recommendation (REQ-10): `make audit`/CI treat the section as informational —
no gate.** The `forge audit` exit code stays keyed on `project_assurance` ONLY
(`cli::run_audit`'s `certified` match — unchanged). Fragment membership is a
capability statement about an alternate engine, not a trust verdict; gating CI
on it would make Lean-fragment growth a breaking event. A future opt-in gate
(e.g. `--require-lean-fragment`) is out of scope (OQ-6).

### Versioning + byte-impact on existing goldens (R-CHAR-3)

- `manifest_version` stays `"v1"`; the new section follows the AC-5 additive
  discipline (`#[serde(default)]` on deserialize; old v1 docs keep parsing).
- **No existing golden pins full audit bytes.** `forge/tests/audit_conformance.rs`
  asserts individual fields (`manifest["manifest_version"]`, the
  `functions`/`tcb` projections) — never a whole-document byte comparison — and
  `audit_is_deterministic` compares two runs of the SAME binary, so an added
  section cannot break it. `conformance/audit/cases.json` is a hand-derived
  field oracle; the amendment ADDS cases, edits none. The per-cert goldens
  (`conformance/sum.cert.json` etc.) are untouched — the `Certificate` schema
  does not change (the attribution non-decision above). **Conclusion: additive,
  no hand-re-certification of existing goldens required.** The NEW oracle
  entries' expected memberships are hand-derived from the refusal inventory in
  `proof-backends.md` §4/§4.1.7/§4.2.5 + `pub enum ExportRefusal`, never copied
  from forge output (R-CHAR-3).
- The refusal `reason` strings co-evolve with the exporter (the
  `LEAN_SCHEMA_VERSION` bump discipline in `engine.rs`); the oracle pins the
  stable `class` for every case and the verbatim `reason` only for the pinned
  inline-program cases (OQ-5).

## Requirements

- **REQ-1 (the AuditManifest v1 schema — stable field set + version tag):**
  define a project-level `AuditManifest` carrying `manifest_version` (`"v1"`),
  the per-fn `functions` rows (name, level, assurance_scope, contract_quality,
  slag, boundary + boundary_target), the `project_assurance` section (the #10/#17
  aggregate: level headline + project scope + lowered-assurance list), and the
  `tcb` section (slag_blocks ∪ boundary_contracts ∪ toolchain). Additive
  evolution only (`#[serde(default, skip_serializing_if)]` precedent). Derived
  from `thermite-design.md` §6 (the manifest IS the trust statement: level +
  slag + §7 scores) + R-SPEC-2/R-SPEC-3 (a stable versioned contract).
- **REQ-2 (`forge audit <file>` — emit JSON + human summary):** a `forge audit
  <file>` command runs the check pipeline over the file (the same
  `check::check_file_with_options` the default `forge check` runs — NO extra
  verification, NO re-derivation), aggregates the resulting cert collection into
  an `AuditManifest`, and emits it as `--json` (the stable document) or a human
  summary (the default). Derived from `thermite-design.md` Appendix B (`forge
  audit` = "full slag + boundary + assurance inventory") + §5.1 (structured,
  machine-readable, rendered to text).
- **REQ-3 (the TCB enumeration = slag ∪ boundary ∪ toolchain):** the `tcb`
  section enumerates EVERY `#[slag]` block (name + reason/owner/review), EVERY
  `#[boundary]` contract (name + foreign target + the enforced req/ens/fx), and
  the toolchain identity (verus version + thermite version). Nothing
  fiat-trusted is omitted: the TCB is exactly (slag ∪ boundary ∪ toolchain).
  Derived from `thermite-design.md` §9 ("the trusted computing base is
  enumerable — it is exactly (slag blocks ∪ boundary contracts ∪ the toolchain
  itself)") + §8 (`grep slag` is the complete inventory) + `goal.md` R-DEFER-9
  (the manifest must HONESTLY enumerate the entire fiat-trusted base).
- **REQ-4 (aggregation, never re-derivation):** the manifest is a pure
  projection of the per-fn `Certificate` collection + `AssuranceManifest` +
  the two version strings. It computes NO verdict — it never re-runs verus,
  re-scores mutants, or re-classifies a closure. Derived from §6 (the
  certificate is the source of truth the manifest *lists*) + the §9 composition
  rule (trust is established per-item; the manifest aggregates it).
- **REQ-5 (project assurance embedded):** the `project_assurance` section is the
  existing `AssuranceManifest::aggregate` output — the min-over-functions
  `ProjectAssurance`, the `ProjectScope` (§9 end-to-end vs to-the-boundary), and
  the lowered-assurance fns. A degraded-or-to-boundary project is reflected
  honestly. Derived from §5.2 (whole-project assurance is the min over
  functions, displayed on every build) + §9 (verified-to-the-boundary vs
  verified-period).
- **REQ-6 (determinism):** the manifest is a deterministic function of its
  inputs (R-CODE-5): no wall-clock, no unordered iteration in the document. The
  non-deterministic `solver_time_ms` is excluded; the version-sensitive
  `mutants_killed`/`survivor` are present-but-oracle-shape-asserted (OQ-2).
  Derived from §5.3 + `goal.md` R-CODE-5.
- **REQ-7 (#274 — the `lean_fragment` membership section):** the manifest gains
  an additive `lean_fragment` section: one row per `functions` row, in source
  order, carrying `{name, exportable: bool, tier: "auto"|"interactive"|"none",
  tier_tag?, refusal?: {class, reason}}` per the shape above. `manifest_version`
  stays `"v1"` (the AC-5 additive discipline; `#[serde(default)]` on
  deserialize). Derived from `thermite-design.md` §5.1 (structured trust
  reporting) + §6 (the audit document is where per-fn toolchain visibility
  lives) + the honest-skip doctrine of `.design/verified/proof-backends.md`
  REQ-3 (a refusal is surfaced, never silent).
- **REQ-8 (#274 — the probe is the shipped dry-run export, side-effect-free):**
  membership is decided by calling the SHIPPED entry points —
  `lean_export::find_item` + the #226-closure CONTRACT obligation
  (`Obligation::contract_for_fn`/`contract_for_spec_fn` fed by `check.rs`'s
  `reachable_spec_fn_names_full*`, exposed crate-internally, never
  re-implemented) + `pub fn export_item in lean_export.rs`. The probe performs
  NO lowering work of its own, touches NO filesystem, spawns NO process, and
  never invokes lake (grounded: `lean_export.rs` is fs/process/env-free; the
  side effects live in `LeanEngine::discharge`). NO recognizer fork. Derived
  from the §9 composition rule (reuse the settled decision procedure) +
  R-CODE-5 (a pure deterministic probe) + the Pin B/C/G closure-fidelity
  hazard (proof-backends §4 hard gate).
- **REQ-9 (#274 — refusal classes surfaced verbatim):** a non-exportable row
  carries the `ExportRefusal` variant name as `class` (the stable machine
  surface: `OutOfFragment`/`NotPureContract`/`IncompleteRegistry`/
  `NonIntResult`/`OpenHole`/`LoopBody`/`OptResResult`) and its `Display`
  rendering as `reason`, verbatim — the post-(v) §4.2.5 LOUD inventory
  (proof-backends REQ-11.5) made visible in the trust document. Never a
  paraphrase, never a silent omission. Derived from proof-backends §4 ("a
  structured export REFUSAL … honest skip") + R-DEFER-9 (honest enumeration of
  what is NOT covered).
- **REQ-10 (#274 — informational only; zero default-path byte impact):** the
  section gates nothing: the `forge audit` exit code stays keyed on
  `project_assurance` alone; `make audit`/CI consume the section as
  information, not a gate. The `Certificate` schema is untouched —
  `engine_attribution` remains `None` on the default Verus path (the
  byte-identity decision; `manifest.rs`: "the default Verus path leaves it
  `None`"). `forge check` output bytes are unchanged. Derived from §5.2 (the
  trust headline is the assurance level, not engine capability) + R-SPEC-2
  (additive, non-breaking evolution).

## Acceptance criteria

ACs tie to a `conformance/audit/` oracle (a hand-derived JSON cases file, the
`conformance/boundary/cases.json` / `conformance/e2e/cases.json` precedent —
authored by the orchestrator, NOT this doc; R-CHAR-3, expected values
hand-derived from `thermite-design.md`, never copied from forge output).

- **AC-1 (pure corpus → all-L3, project end-to-end, contract_quality present,
  TCB empty-but-toolchain):** `forge audit conformance/sum.th` emits an
  `AuditManifest` with `manifest_version: "v1"`; the `functions` rows for `sum`
  (and `spec_sum`'s well-formedness) are present; `project_assurance` is
  `Certified(L3)` END-TO-END; each fn row carries a `contract_quality` block
  (shape asserted, not the version-sensitive ratio — OQ-2); and the `tcb`
  section has EMPTY `slag_blocks` and EMPTY `boundary_contracts`, with only the
  `toolchain` (verus + thermite versions) populated — the §9 "verified, period"
  TCB. Same for `conformance/binary_search.th`.
- **AC-2 (slag + boundary file → TCB lists BOTH):** `forge audit` over a fixture
  containing a valid `#[slag(reason=…, owner=…, review=…)]` fn AND a
  `#[boundary("crate::path")]` fn emits a `tcb` whose `slag_blocks` lists the
  slag fn with its `reason`/`owner`/`review` (from `slag_meta`) and whose
  `boundary_contracts` lists the boundary fn with its `boundary_target` + its
  enforced `req`/`ens`/`fx`. The §8/§9 "grep slag"-complete fiat-trust
  enumeration — nothing omitted (R-DEFER-9). The slag/boundary fns certify L1
  (their existing `Certificate::slag_l1`/`boundary_l1` verdicts, unchanged).
- **AC-3 (degraded / to-boundary project → project_assurance reflects it):** a
  fixture project with a fn whose closure crosses a boundary → `project_assurance`
  reports `ToBoundary` listing the crossing(s); a fixture with a lowered-assurance
  (auto-degraded) fn → `project_assurance` lists it under lowered-assurance and
  the headline is the min-over-functions rung (e.g. `Certified(L2)`). The audit
  manifest never claims a stronger trust state than the per-fn certs support.
- **AC-4 (determinism):** `forge audit` over a fixture twice yields a
  byte-identical `--json` document, modulo the excluded `solver_time_ms` (which
  is absent from the manifest). With `VERUS_VERSION` pinned, the manifest is
  fully reproducible (R-CODE-5). Extends to the `lean_fragment` section (the
  probe is pure; the refusal strings are static formats).
- **AC-5 (stable schema / version tag):** the manifest carries
  `manifest_version: "v1"`; a downstream additive field must default so a v1
  document continues to deserialize (the per-cert `#[serde(default)]` R-SPEC-2
  precedent). The audit oracle pins the v1 field set.
- **AC-6 (no re-derivation):** the per-fn rows in the audit manifest match the
  certs `forge check <file>` emits for the same file (the manifest is a
  projection, not a recomputation) — the audit and check verdicts agree
  field-for-field on the deterministic (oracle) subset.
- **AC-7 (#274 — membership rows present, one per fn, classes hand-derivable):**
  `forge audit conformance/sum.th --json` emits a `lean_fragment.functions`
  array with exactly one row per `functions` row (`spec_sum`, `sum`), source
  order. Hand-derived expectations (R-CHAR-3, from the proof-backends refusal
  inventory — NOT from forge output): `sum`'s row is `exportable: false` with a
  structured refusal whose `class` matches the hand-derived membership of its
  while-body shape (its `inv acc == spec_sum(&xs[..i])` is a spec-calling inv —
  the (v) v1 `OutOfFragment` residual, proof-backends REQ-11.5); the oracle pins
  the exact class + verbatim reason. A pinned pure-int-tail inline program (the
  `slag_boundary_tcb` inline-`program` precedent, e.g. the lean_engine.rs
  `count` shape) reports `exportable: true, tier: "auto"`; a pinned
  recursive-spec-call program reports its caller `exportable: true, tier:
  "interactive", tier_tag: "recursive-interactive"`.
- **AC-8 (#274 — refusal classes verbatim across the inventory):** pinned inline
  programs exercise at least: an Option/Result-typed result →
  `class: "OptResResult"`; a `loop`-kind loop body → `class: "LoopBody"`; a
  boundary fn → `class: "NotPureContract"` (foreign body); each row's `reason`
  equals the shipped `ExportRefusal` `Display` string verbatim (REQ-9).
- **AC-9 (#274 — the probe agrees with the engine):** for every row,
  `exportable`/`tier`/`refusal.class` equal what `export_item` returns for that
  item's CONTRACT obligation (a unit/integration test calls `export_item`
  directly via the same minting seam and compares) — i.e. the report and the
  `--engine lean` admission decision can never disagree (REQ-8 reuse).
- **AC-10 (#274 — side-effect-free + no-gate):** producing the report runs no
  `lake`, writes no scratch file, and requires no `lean/` toolchain present
  (the audit conformance run asserts the section appears even with lake
  absent); the `forge audit` exit code for every existing oracle case is
  unchanged (REQ-10).
- **AC-11 (#274 — existing goldens unchanged / additive):** every pre-amendment
  `audit_conformance.rs` assertion passes unmodified (field-asserting, not
  byte-golden — verified above); the per-cert goldens (`conformance/*.cert.json`)
  are byte-identical (the `Certificate` schema untouched; `engine_attribution`
  stays `None` on the default path). A pre-amendment v1 JSON document still
  deserializes (`#[serde(default)]` on `lean_fragment`).

## Architecture

The manifest is a new pure aggregation module, expected at `forge/src/audit.rs`
(the route the orchestrator must add — see Verification). It depends ONLY on the
certificate collection `check::check_file_with_options` returns, the
`AssuranceManifest::aggregate` over that collection (both in `manifest.rs`), and
the two version strings (`resolve_verus_version` + `THERMITE_VERSION`, both in
`check.rs`). It owns NO prover invocation and computes NO verdict — it LAYERS a
stable serializable trust statement on top of the per-fn certificates `forge
check` already produced (the §6 "the certificate IS the trust statement" made a
project-level document).

Data flow (the §6/§8/§9 deliverable, end to end; the #274 probe branch in
brackets):

```text
forge audit <file>
      │
      ▼
check::check_file_with_options(file, default)  ── the SAME pipeline forge check runs (no extra verification)
      │   → Vec<Certificate>   (per-fn: level, contract_quality, slag/slag_meta,
      │                          boundary/boundary_target, assurance_scope)
      ▼
manifest::AssuranceManifest::aggregate(&certs)  ── project headline (min level) + ProjectScope (§9)
      │
      │   [#274: per row → find_item + the #226 CONTRACT obligation
      │          → lean_export::export_item (PURE dry run, no lake/fs)
      │          → Ok(tier) | Err(ExportRefusal{class, reason})]
      ▼
audit::AuditManifest::from(&certs, &assurance, verus_version, THERMITE_VERSION)
      │   functions[]  (project per-fn rows)
      │   project_assurance  (the #10/#17 aggregate)
      │   tcb  (slag_blocks ∪ boundary_contracts ∪ toolchain)  ── §9 enumerable TCB, R-DEFER-9
      │   lean_fragment[]  (#274 — informational membership rows)
      ▼
cli: --json (the stable AuditManifest document) | human summary  (§5.1)
```

The §9 composition rule is exactly why the manifest is an *aggregate* and not a
whole-program reverification: each `Certificate`'s trust was established
per-item (`g` calling `f` only through `f`'s contract); the manifest collects
those settled verdicts. The TCB enumeration keys on the per-fn `slag` /
`boundary` flags (`Certificate.slag` set by `Certificate::slag_l1`;
`Certificate.boundary` + `boundary_target` set by `Certificate::boundary_l1`,
both in `manifest.rs`) and their justification metadata (`slag_meta`), never on
re-parsing the source. The #274 membership rows are the one place the audit
consults a decision procedure rather than a cert field — but that procedure
(`export_item`) is itself a pure, settled, shipped function reused verbatim
(REQ-8), not a re-derivation of any verdict: it answers "what WOULD the Lean
engine admit", never "what is proven".

### Why the toolchain identity is part of the TCB (R-DEFER-9)

§9 states the TCB is *exactly* (slag ∪ boundary ∪ the toolchain itself). Omitting
the toolchain identity would make a pure-Thermite project's TCB appear empty,
which is dishonest — every artifact trusts the prover that produced its
certificates. The `toolchain` entry (verus version + thermite version) is the
irreducible residue, so even an all-L3 end-to-end project has a non-empty,
honestly-enumerated TCB. The two versions are the same strings the proof cache
keys on (`resolve_verus_version` + `THERMITE_VERSION` in `check.rs`), so the TCB
identity and the cache provenance agree.

## Verification

- **Route to add (orchestrator, not this doc):** add a `[[route]]` to
  `tooling/spec-routes.toml` mapping `forge/src/audit.rs` → this doc, with
  `reference = ["conformance/audit"]` and `conformance_ops = ["sum",
  "binary_search", "slag_boundary", "to_boundary_project"]`. The spec-discipline
  hook (R-XLATE-2/R-XLATE-3) blocks the builder's edit until both the route and
  this doc exist. *(Status: the route exists at `tooling/spec-routes.toml`
  `crate_pattern = "forge/src/audit.rs"` → this doc — no #274 route change
  needed; the amendment edits files already routed here.)*
- **Oracle (orchestrator-authored):** a `conformance/audit/cases.json`
  hand-derived fixture file (the `conformance/boundary/cases.json` /
  `conformance/e2e/cases.json` precedent) carrying the AC-1..AC-3 fixtures and
  their expected manifest projections — the per-fn rows, the project assurance,
  and the TCB enumeration. The audit-oracle test (`forge/tests/`) asserts the
  emitted `AuditManifest` against this golden file on the deterministic subset
  (`solver_time_ms` absent; `contract_quality` shape, not the version-sensitive
  ratio). The EXACT fixtures:
  - `sum` (`conformance/sum.th`) and `binary_search`
    (`conformance/binary_search.th`) — all-L3, project END-TO-END,
    `contract_quality` present, TCB empty-but-toolchain (AC-1).
  - `slag_boundary` — a program with one valid `#[slag(...)]` fn (modeled on
    `conformance/slag/slag.json`'s `simd_sum_l1`) AND one
    `#[boundary("ext::foreign_id")]` fn (modeled on
    `conformance/boundary/cases.json`'s `foreign_id`): the TCB lists BOTH (slag
    with reason/owner/review; boundary with target + contract) (AC-2).
  - `to_boundary_project` — a pure-Thermite caller whose closure reaches the
    boundary fn (modeled on `conformance/e2e/cases.json`'s `boundary_caller`):
    `project_assurance` is `ToBoundary` listing the crossing (AC-3).
  - **(#274, NEW cases — additive to `cases.json`, no existing case edited):**
    `lean_fragment_sum` (the `sum.th` membership rows, AC-7),
    `lean_fragment_tiers` (inline pure-int-tail + recursive-spec-call programs,
    AC-7), `lean_fragment_refusals` (inline OptResResult / LoopBody /
    boundary-NotPureContract programs, AC-8). Expected `class`/`reason` values
    hand-derived from `pub enum ExportRefusal in lean_export.rs` + its
    `Display` impl + proof-backends §4/§4.1.7/§4.2.5 (R-CHAR-3).
- **(#274) Agreement + purity tests:** `forge/tests/audit_conformance.rs` (or a
  sibling) adds: the AC-9 agreement test (report row ≡ direct `export_item`
  result per item); the AC-10 no-lake assertion (the section is present and
  identical with `lake` absent from PATH — mirror the lean_engine.rs
  lake-absence seam); the AC-11 old-document deserialization test (a
  pre-amendment v1 JSON literal still parses).
- **Crate gauntlet (the kernel discipline):** `cargo test -p forge`, `cargo
  clippy -p forge --all-targets -- -D warnings`, `cargo fmt --check`, plus the
  conformance corpus (`forge audit` over `conformance/` programs — the pure
  programs must stay all-L3 / END-TO-END / empty-TCB; the slag/boundary fixtures
  must enumerate the TCB). The corpus golden `sum.cert.json` (the per-cert
  oracle) is unaffected — `forge audit` reads the same certs `forge check`
  emits, it does not change the cert schema (R-SPEC-2). The #274 amendment keeps
  this invariant: NO `Certificate` field changes, `engine_attribution` stays
  `None` on the default path.

## Open questions

- **OQ-1 (human-summary shape):** the `--json` document is the stable contract
  (REQ-1); the human summary's exact text is a rendering detail (the
  `cli::render_human` / `render_assurance` precedent). The §8 "`grep slag` is the
  complete inventory" framing suggests the human TCB section should be
  greppable/line-oriented. Decision deferred to the builder; the JSON is the
  oracle-asserted surface.
- **OQ-2 (what the audit oracle asserts in `contract_quality`):** `mutants_killed`
  / `survivor` are verus-version-sensitive (oracle-EXCLUDED from the per-cert
  oracle, `certificate-manifest.md` / `mutation-scoring.md`). The audit oracle
  asserts the block's *presence and shape* and the two §7 bools
  (`tautology`/`vacuous_precondition`), not the ratio string — mirroring the
  per-cert precedent. Ratified by that precedent; flagged here for the builder.
- **OQ-3 (does `forge audit` accept the `--rlimit`/`--mutation-floor` levers?):**
  the canonical audit deliverable runs at the pinned default config
  (`CheckOptions::default`) so the manifest is the reproducible trust statement.
  Whether `forge audit` exposes the exploratory levers (like `forge check` does)
  is a CLI-surface question; the default-config path is the contract.
- **OQ-4 (#274 — the obligation-minting seam):** `reachable_spec_fn_names_full`
  / `mint_item_obligations` are private to `check.rs`. The builder picks the
  seam (a `pub(crate)` re-export, or `run_audit` receiving pre-minted
  obligations from a `check::` helper); the CONTRACT is only that the probe's
  closure is the #226 one, byte-for-byte the check pipeline's (REQ-8). No
  closure re-implementation under any seam choice.
- **OQ-5 (#274 — reason-string stability):** `refusal.class` is the stable
  machine surface; `refusal.reason` (the `Display` text) co-evolves with the
  exporter (proof-backends increments routinely refine refusal wording, cf. the
  (v-b) §4.2.5 inventory). The oracle pins `class` for every case and verbatim
  `reason` only for the pinned inline cases — accepting that an exporter
  increment may require re-deriving those strings (a LOUD oracle edit, the
  R-CHAR-3 hand-re-derivation discipline, not a silent re-copy).
- **OQ-6 (#274 — a future opt-in fragment gate):** should CI ever gate on
  fragment coverage (e.g. "no fn may regress from exportable to refused")?
  Deliberately OUT of this amendment (REQ-10: informational only). If wanted, it
  is a separate flag + design increment; the report's determinism (AC-4) makes
  such a gate cheap to add later.

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (AuditManifest v1 schema + version tag) | SHIPPED | `struct AuditManifest { manifest_version, functions, project_assurance, tcb }` in `audit.rs`; `manifest_version` is `MANIFEST_VERSION` (`"v1"`) with `#[serde(default)]` for additive evolution. Built by `AuditManifest::from_certificates`; consumer `cli::run_audit` (`--json` + `cli::render_audit`). Oracle: `forge/tests/audit_conformance.rs::corpus_empty_tcb` asserts `manifest_version == "v1"`. |
| REQ-2 (`forge audit <file>` command) | SHIPPED | `cli::parse_args`'s `"audit"` verb → `Command::Audit { file, json }`; `cli::run_audit` runs `check::check_file` (the default-config pipeline, no extra verification, OQ-3) and emits `--json` or `render_audit`. Oracle: `audit_conformance.rs` drives the built binary with `forge audit <file> --json`. |
| REQ-3 (TCB enumeration = slag ∪ boundary ∪ toolchain) | SHIPPED | `Tcb::from_certificates` enumerates every `cert.slag` → `SlagBlock` (reason/owner/review from `slag_meta`), every `cert.boundary` → `BoundaryContract` (target + `req`/`ens`/`fx` looked up in the program), and `Toolchain` (always present). Oracle: `audit_conformance.rs::slag_boundary_tcb` asserts BOTH `vendored` + `ext_f` enumerated (R-DEFER-9); `corpus_empty_tcb` asserts the empty-but-toolchain pure state. |
| REQ-4 (aggregation, never re-derivation) | SHIPPED | `AuditManifest::from_certificates` reads only the cert collection + `AssuranceManifest::aggregate(&certs)` + the parsed program (boundary contract text) + the two version strings; it owns no prover invocation. `cli::run_audit` calls `check::check_file` once and projects its certs. |
| REQ-5 (project assurance embedded) | SHIPPED | `ProjectAssuranceSection::from_assurance` embeds `AssuranceManifest::aggregate` — the `ProjectAssurance` headline, the `ProjectScope`, and the lowered-assurance fn names (from `FunctionAssurance.lowered_assurance`). Oracle: `audit_conformance.rs::corpus_empty_tcb` asserts the L3/end-to-end headline; unit test `lowered_assurance_listed_in_project_section`. |
| REQ-6 (determinism) | SHIPPED | the manifest is a pure function of its inputs; `functions`/TCB lists in cert/source order; `solver_time_ms` structurally absent; `mutants_killed`/`survivor` carried but oracle-shape-asserted (OQ-2). Oracle: `audit_conformance.rs::audit_is_deterministic` (two runs → byte-identical `--json`) + unit test `manifest_is_deterministic`. |
| REQ-7 (#274 — `lean_fragment` membership section) | NOT-STARTED | blocker #274. No `lean_fragment` field exists on `struct AuditManifest` in `audit.rs` (the shipped struct has exactly `manifest_version`/`functions`/`project_assurance`/`tcb`); `cli::render_audit` has no fragment section. Substrate SHIPPED: `pub enum ExportTier` + `ExportTier::{is_auto, tag}` in `lean_export.rs` supply the tier vocabulary. |
| REQ-8 (#274 — probe = shipped dry-run export, side-effect-free) | NOT-STARTED | blocker #274. The probe's parts all SHIP (`pub fn export_item in lean_export.rs` — pure string-building, no fs/process; precedent `LeanEngine::obligation_content_hash in engine.rs` already dry-runs it per obligation; closure minting `mint_item_obligations`/`reachable_spec_fn_names_full in check.rs`) but nothing wires them into the audit assembly; the minting seam is private to `check.rs` (OQ-4). |
| REQ-9 (#274 — refusal classes surfaced verbatim) | NOT-STARTED | blocker #274. `pub enum ExportRefusal in lean_export.rs` (`OutOfFragment`/`NotPureContract`/`IncompleteRegistry`/`NonIntResult`/`OpenHole`/`LoopBody`/`OptResResult` + `Display`) ships and is consumed by `LeanEngine::discharge`'s `Unknown` skip — but no audit surface carries it; today a refusal is visible only inside an `--engine lean` run's reason string. |
| REQ-10 (#274 — informational only, zero default-path byte impact) | NOT-STARTED | blocker #274. The non-gating contract is a property of the unbuilt section; the substrate invariant it preserves SHIPS today: `Certificate.engine_attribution` is `None` on the default path (`manifest.rs`: "the default Verus path leaves it `None`") and `cli::run_audit`'s exit code keys on `project_assurance` only — both must remain byte-/behavior-identical after the amendment (AC-10/AC-11). |
