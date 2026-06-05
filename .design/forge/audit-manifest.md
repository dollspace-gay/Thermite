# Audit Manifest Format v1 — the Trust Deliverable
<!--
tier: 3-component
status: draft
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
(it is this component's *input*, never re-derived). All REQs are **SHIPPED**
(`forge/src/audit.rs` + the `forge audit` verb in `cli.rs`), verified by
`forge/tests/audit_conformance.rs` against `conformance/audit/cases.json`;
tracked by **crosslink issue #15** (v0.3, milestone #3 Battery).

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

## The AuditManifest v1 schema (the stable contract)

The manifest is a single project-level document. Because it IS the deliverable's
trust statement (§6) and a stable contract (R-SPEC-2/R-SPEC-3), the schema
carries an explicit `manifest_version` format tag (`"v1"`) so a downstream
consumer can pin and evolve the format additively (the per-fn `Certificate`
additive-field precedent in `manifest.rs`: `#[serde(default, skip_serializing_if)]`).

The v1 field set, in three sections:

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
  fully reproducible (R-CODE-5).
- **AC-5 (stable schema / version tag):** the manifest carries
  `manifest_version: "v1"`; a downstream additive field must default so a v1
  document continues to deserialize (the per-cert `#[serde(default)]` R-SPEC-2
  precedent). The audit oracle pins the v1 field set.
- **AC-6 (no re-derivation):** the per-fn rows in the audit manifest match the
  certs `forge check <file>` emits for the same file (the manifest is a
  projection, not a recomputation) — the audit and check verdicts agree
  field-for-field on the deterministic (oracle) subset.

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

Data flow (the §6/§8/§9 deliverable, end to end):

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
      ▼
audit::AuditManifest::from(&certs, &assurance, verus_version, THERMITE_VERSION)
      │   functions[]  (project per-fn rows)
      │   project_assurance  (the #10/#17 aggregate)
      │   tcb  (slag_blocks ∪ boundary_contracts ∪ toolchain)  ── §9 enumerable TCB, R-DEFER-9
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
re-parsing the source.

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
  this doc exist.
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
- **Crate gauntlet (the kernel discipline):** `cargo test -p forge`, `cargo
  clippy -p forge --all-targets -- -D warnings`, `cargo fmt --check`, plus the
  conformance corpus (`forge audit` over `conformance/` programs — the pure
  programs must stay all-L3 / END-TO-END / empty-TCB; the slag/boundary fixtures
  must enumerate the TCB). The corpus golden `sum.cert.json` (the per-cert
  oracle) is unaffected — `forge audit` reads the same certs `forge check`
  emits, it does not change the cert schema (R-SPEC-2).

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

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (AuditManifest v1 schema + version tag) | SHIPPED | `struct AuditManifest { manifest_version, functions, project_assurance, tcb }` in `audit.rs`; `manifest_version` is `MANIFEST_VERSION` (`"v1"`) with `#[serde(default)]` for additive evolution. Built by `AuditManifest::from_certificates`; consumer `cli::run_audit` (`--json` + `cli::render_audit`). Oracle: `forge/tests/audit_conformance.rs::corpus_empty_tcb` asserts `manifest_version == "v1"`. |
| REQ-2 (`forge audit <file>` command) | SHIPPED | `cli::parse_args`'s `"audit"` verb → `Command::Audit { file, json }`; `cli::run_audit` runs `check::check_file` (the default-config pipeline, no extra verification, OQ-3) and emits `--json` or `render_audit`. Oracle: `audit_conformance.rs` drives the built binary with `forge audit <file> --json`. |
| REQ-3 (TCB enumeration = slag ∪ boundary ∪ toolchain) | SHIPPED | `Tcb::from_certificates` enumerates every `cert.slag` → `SlagBlock` (reason/owner/review from `slag_meta`), every `cert.boundary` → `BoundaryContract` (target + `req`/`ens`/`fx` looked up in the program), and `Toolchain` (always present). Oracle: `audit_conformance.rs::slag_boundary_tcb` asserts BOTH `vendored` + `ext_f` enumerated (R-DEFER-9); `corpus_empty_tcb` asserts the empty-but-toolchain pure state. |
| REQ-4 (aggregation, never re-derivation) | SHIPPED | `AuditManifest::from_certificates` reads only the cert collection + `AssuranceManifest::aggregate(&certs)` + the parsed program (boundary contract text) + the two version strings; it owns no prover invocation. `cli::run_audit` calls `check::check_file` once and projects its certs. |
| REQ-5 (project assurance embedded) | SHIPPED | `ProjectAssuranceSection::from_assurance` embeds `AssuranceManifest::aggregate` — the `ProjectAssurance` headline, the `ProjectScope`, and the lowered-assurance fn names (from `FunctionAssurance.lowered_assurance`). Oracle: `audit_conformance.rs::corpus_empty_tcb` asserts the L3/end-to-end headline; unit test `lowered_assurance_listed_in_project_section`. |
| REQ-6 (determinism) | SHIPPED | the manifest is a pure function of its inputs; `functions`/TCB lists in cert/source order; `solver_time_ms` structurally absent; `mutants_killed`/`survivor` carried but oracle-shape-asserted (OQ-2). Oracle: `audit_conformance.rs::audit_is_deterministic` (two runs → byte-identical `--json`) + unit test `manifest_is_deterministic`. |
