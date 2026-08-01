# Canonical REQ Registry and Generated Status Views

<!--
tier: 3-component
status: draft
audited-sha: f09f8ca376257cc1e2543b8ebc9fb771bffd04df (content-sha256 re-pinned 2026-07-29 for stage-3 REQ-1..REQ-9 / gate G3 (#80, crosslink #351): the registry and generated status view carry the fixed-width reconstruction requirements and verified evidence; req-registry.py --check is clean (462 requirements, 119 views). The legacy commit pin remains the f09f8ca3 stable-main ancestor; the active content digest tracks the governed files. prior: 2026-06-21 stage-2 REQ-10 / AC-10 (#332), the pin battery and final gate G2 increment)
audited-content-sha256: 97cdb29abe7211f41c787696709e04f229ef8aaf50f94eecb3d0c18690783704 (re-pinned 2026-07-31 after registering issue #108 kernel byte-slice REQ-KERNELBYTES-1..REQ-KERNELBYTES-5)
governs:
  - .design/reqs/registry.toml
  - .design/reqs/status.md
  - tooling/req-registry.py
  - tooling/reqs
  - tooling/req-status.py (legacy bridge only)
  - tooling/spec-routes.toml entries for this registry
  - Makefile req-registry targets
  - .github/workflows/ci.yml req-registry step
  - generated regions declared by `.design/reqs/registry.toml`
thesis-refs:
  - thermite-design.md §1 (auditability by a skeptical third party)
  - thermite-design.md §8 (unverified residue must be loud)
issue: GitHub #17
-->

> **Gate G4 re-audit (2026-07-29).** The schema and renderer are unchanged.
> The registry now includes `REQ-G4-1` through `REQ-G4-10`, with file, symbol,
> test, and command evidence for the shipped reconstruction gate. Regenerating
> the 119 views produces 472 validated requirements.

> **Rich composition increment (2026-07-31).** `REQ-L3COMPOSE-1` through
> `REQ-L3COMPOSE-10` bind issue #104's CLI, closure, policy, exact-source,
> receipt, publication, kernel-link, and codegen acceptance evidence. The
> generated registry now contains 496 validated requirements.

> **Kernel byte-slice increment (2026-07-31).** `REQ-KERNELBYTES-1` through
> `REQ-KERNELBYTES-5` bind issue #108's pinned no-std proof model, exact-content
> reads, receipt/replay boundary, negative rejection matrix, and deterministic
> hosted/freestanding consumers. The registry now contains 502 validated
> requirements across 122 generated views.

## Summary

The comment-level `tooling/req-status.py` gate is a useful tripwire: it catches
obvious contradictions in repeated `//! | REQ | SHIPPED/NOT-STARTED | evidence |`
rows. It is not a source of truth. Exact-label matching can miss renamed
requirements, symbol existence does not prove semantic coverage, and future-scope
keywords do not prove that a blocker exists or owns the work.

This component introduces the next layer: a canonical machine-readable registry
under `.design/reqs/registry.toml`, a validator/generator at
`tooling/req-registry.py`, and generated status views such as
`.design/reqs/status.md`. The registry is deliberately harness-neutral: git plus
a TOML parser is enough to read and check the offline contract, and live tracker
or CI integrations can be thin adapters over the same file. Source comments
should keep stable invariants and non-obvious mechanisms; volatile status,
evidence, blockers, and migration state belong in registry data and generated
views.
The source-comment turnover is complete for the hand-maintained REQ status rows:
source files now carry generated regions or links back to canonical owner rows,
and `tooling/req-status.py` remains only as a compatibility tripwire that should
stay quiet when no legacy rows are reintroduced.

## Design Decisions

1. **TOML, stdlib-only.** The registry uses TOML because Python 3.11+ includes
   `tomllib`, matching the route-table tooling style. No runtime dependency is
   added for CI or local gauntlet use.
2. **Stable IDs are the identity.** Requirement titles can change; IDs must not.
   Aliases are metadata only. Conflict detection should key on `id`, not prose.
3. **One owner, many contributors.** Every requirement has one owner field: the
   doc/module accountable for status. Optional contributors can be listed, and
   evidence can point to any number of files, tests, symbols, commands, docs, or
   tracker references. Other modules reference the owner's entry by ID; they do
   not restate status.
4. **Typed evidence, not proof by prose.** Evidence has a `kind` and `target`.
   `file`, `doc`, and `test` targets must resolve as paths; `symbol` targets
   must resolve in repo text; `issue` targets use tracker-neutral references
   (`github:owner/repo#N`, `crosslink:144`, `req:REQ-ID`, or a URI); `command`
   targets must parse, resolve their executable, and resolve repo-path
   arguments. Commands are not executed by this gate.
5. **Status policy is registry-declared.** The checker does not hard-code
   Thermite's status vocabulary. Top-level `[[status]]` records declare accepted
   status names and their generic validation rules: required evidence kind sets,
   blocker requirements, and remaining-scope requirements.
6. **Generated output is checked, not trusted by convention.** CI runs the
   registry with `--check`; generated regions must match renderer output
   exactly. Generated tables live inside marked
   `<!-- generated:reqs view=... -->` blocks so surrounding prose can remain
   hand-authored. Source-comment regions use a declared `comment_prefix`, such
   as `//! ` for Rust module docs, so generated content stays syntactically valid
   in its target file.
7. **Legacy comment rows are bridged, not bulk-converted blindly.** Reviewed
   `[[legacy_mapping]]` records bind each old `(path, label)` pair to a
   canonical ID and replacement generated view. With the current turnover done,
   those mappings serve as audit history and `req-status.py` remains as the
   contradiction tripwire for accidental reintroduction.

## Registry Schema v1

Top-level fields:

- `schema_version = 1`
- `[[status]]`: project-declared status policy
- `[[view]]`: generated output target
- `[[legacy_mapping]]`: reviewed mapping from an old source-comment label to a
  stable registry ID and replacement generated view
- `[[requirement]]`: canonical requirement record

Status fields:

- `name`: accepted status token
- `final`: whether the status represents completed work
- `required_evidence_any`: optional evidence-kind set; at least one listed kind
  must appear on requirements with this status
- `requires_blocker`: whether requirements with this status need at least one
  blocker reference
- `requires_remaining_scope`: whether requirements with this status need
  `remaining_scope`

View fields:

- `name`: stable view name referenced by requirements
- `path`: generated target path; whole-file generated views must stay under
  `.design/`, while region views may target source files
- `kind`: `full_inventory` or `reference_list`
- `mode`: `file` or `region`
- `region`: generated-region name when `mode = "region"`
- `comment_prefix`: optional line prefix for generated region content, used for
  source-comment targets such as Rust `//!` docs
- `title`: optional generated document title

Legacy mapping fields:

- `path`: source path that carried the old hand-maintained row
- `label`: exact legacy row label being reviewed
- `id`: canonical requirement ID that owns the status/evidence
- `replacement_view`: generated view that replaces the old copied row in the
  same path
- `note`: optional migration context

Requirement fields:

- `id`: stable `REQ-*` token
- `title`: human-readable name
- `owner`: accountable doc/module/path
- `status`: one of the names declared in top-level `[[status]]` records
- `scope`: area such as `tooling`, `forge`, `syntax`, `verified`, or `basis`
- `summary`: short prose summary
- `remaining_scope`: required when the status policy says so
- `aliases`: optional old names or source-comment labels
- `contributors`: optional related files/docs/modules that contribute evidence
- `blockers`: tracker-neutral refs such as `github:dollspace-gay/Thermite#17`,
  `crosslink:144`, `req:REQ-REG-6`, or a URI
- `generated_to`: named views that should include the requirement
- `[[requirement.evidence]]`: typed evidence entries

Evidence fields:

- `kind`: `file`, `symbol`, `test`, `issue`, `doc`, or `command`
- `target`: path, symbol, issue ref, or command string depending on kind
- `note`: optional human context

Tracker references are structurally checked offline. A live adapter may later
resolve open/closed state for a specific tracker, but no tracker credentials are
required for the default gate to pass. The first live adapter is optional:
`tooling/reqs check --live-issues` asks `gh` to resolve GitHub issue references
and fails if any blocker resolves closed.

## CLI

- `tooling/reqs check`: validate the registry and fail if generated regions are
  stale.
- `tooling/reqs render`: rewrite generated views and regions from the registry.
- `tooling/reqs query`: print the normalized registry inventory; add `--json`
  for machine-readable output.

The historical flags remain valid through `python3 tooling/req-registry.py` for
scripts that have not switched to the facade.

## Requirements

- **REQ-REG-1 (stable requirement identity and ownership):** every canonical row
  has a stable ID, title, owner, status, scope, generated-view membership, and
  typed evidence.
- **REQ-REG-2 (registry-declared status policy):** the status vocabulary and
  per-status validation requirements are declared in registry data, not hard-coded
  by the checker.
- **REQ-REG-3 (typed evidence validation):** evidence references are mechanically
  checked at the level this gate can honestly validate: path existence, symbol
  occurrence, command parseability and executable/path resolution,
  tracker-neutral ref shape, and `req:` blocker resolution.
- **REQ-REG-4 (generated status regions):** generated markdown views are rendered
  deterministically from the registry into marked regions, and CI fails when
  checked-in output is stale.
- **REQ-REG-5 (legacy source-comment bridge):** `tooling/req-status.py` remains
  active as a compatibility tripwire after the repeated source-comment status
  rows have been mapped to stable IDs.
- **REQ-REG-6 (generated-region migration):** hand-maintained source status
  copies have been replaced with generated source-comment regions or links after
  each stable-ID mapping was reviewed.

## Acceptance Criteria

- AC-1: a duplicate requirement ID fails validation.
- AC-2: an undeclared status fails validation.
- AC-3: a requirement whose status declares `required_evidence_any` fails without
  at least one matching evidence kind.
- AC-4: unresolved `file`, `doc`, or `test` evidence fails validation.
- AC-5: statuses declaring `requires_blocker` fail without a structurally valid
  blocker; `req:REQ-ID` blockers must resolve to a known registry ID.
- AC-6: `--check` fails when a generated region differs from renderer output.
- AC-7: `--write` rewrites the generated view deterministically.
- AC-8: `tooling/reqs check` is wired into Makefile and CI.
- AC-9: `reference_list` views can render into Rust doc-comment regions with a
  declared `comment_prefix`.
- AC-10: a legacy mapping fails if its canonical ID or replacement view does not
  resolve; once the old label is removed, the replacement generated region must
  be present in the same file.
- AC-11: command evidence fails if its shell syntax, executable, or repo-path
  arguments do not resolve.
- AC-12: `tooling/reqs check --live-issues` fails when a GitHub blocker resolves
  closed through the optional `gh` adapter.

## Migration Plan

1. Land schema v1, validator/generator, generated inventory, routes, and CI.
2. Export the current `req-status.py --json` rows as candidate aliases.
3. Review and assign stable IDs by owner doc/module, not by exact label alone.
4. Add canonical registry records plus `[[legacy_mapping]]` records for migrated
   requirements.
5. Replace repeated source-comment status copies with generated regions or
   links. This is complete for the legacy rows covered by the registry turnover.
6. Keep the bridge tight: `req-status.py` should stay green and any reintroduced
   hand-maintained source status row should be treated as a regression to map or
   remove.

## Known Limits

This registry does not prove semantic adequacy. A symbol can exist without being
the right symbol; a command can resolve without being executed by this gate; a
tracker ref can be parseable without being live-resolved unless
`--live-issues` is explicitly enabled. Stronger checks require later integration
with Rust item indexing, CI job metadata, or additional tracker adapters.
Legacy mappings are likewise structural: they prove that a reviewed old label
points at a stable ID and that a replacement region exists, not that the human
ID assignment was semantically perfect. Schema v1 deliberately keeps those as
explicit future hardening points rather than pretending string validation is
proof.
