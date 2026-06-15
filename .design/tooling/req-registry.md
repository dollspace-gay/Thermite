# Canonical REQ Registry and Generated Status Views

<!--
tier: 3-component
status: draft
audited-sha: 4b05c3a33d47665145fb2739d7d7cf00cd39d96d (re-pinned 2026-06-15: schema v1 registry, generated status view, validator/generator, routes, Makefile, and CI gate landed; RFC #17)
governs:
  - .design/reqs/registry.toml
  - .design/reqs/status.md
  - tooling/req-registry.py
  - tooling/req-status.py (legacy bridge only)
  - tooling/spec-routes.toml entries for this registry
  - Makefile req-registry targets
  - .github/workflows/ci.yml req-registry step
thesis-refs:
  - thermite-design.md §1 (auditability by a skeptical third party)
  - thermite-design.md §8 (unverified residue must be loud)
issue: GitHub #17
-->

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
   targets are recorded but not executed by this gate.
5. **Status policy is registry-declared.** The checker does not hard-code
   Thermite's status vocabulary. Top-level `[[status]]` records declare accepted
   status names and their generic validation rules: required evidence kind sets,
   blocker requirements, and remaining-scope requirements.
6. **Generated output is checked, not trusted by convention.** CI runs the
   registry with `--check`; generated regions must match renderer output
   exactly. Generated tables live inside marked
   `<!-- generated:reqs view=... -->` blocks so surrounding prose can remain
   hand-authored.
7. **Legacy comment rows are bridged, not bulk-converted blindly.** The existing
   429 source-comment rows need a reviewed stable-ID mapping. Until that lands,
   `req-status.py` remains as the contradiction tripwire.

## Registry Schema v1

Top-level fields:

- `schema_version = 1`
- `[[status]]`: project-declared status policy
- `[[view]]`: generated output target
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
- `path`: generated markdown path, under `.design/`
- `kind`: currently `full_inventory`
- `mode`: `file` or `region`
- `region`: generated-region name when `mode = "region"`
- `title`: optional generated document title

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
required for the default gate to pass.

## Requirements

- **REQ-REG-1 (stable requirement identity and ownership):** every canonical row
  has a stable ID, title, owner, status, scope, generated-view membership, and
  typed evidence.
- **REQ-REG-2 (registry-declared status policy):** the status vocabulary and
  per-status validation requirements are declared in registry data, not hard-coded
  by the checker.
- **REQ-REG-3 (typed evidence validation):** evidence references are mechanically
  checked at the level this gate can honestly validate: path existence, symbol
  occurrence, tracker-neutral ref shape, and `req:` blocker resolution.
- **REQ-REG-4 (generated status regions):** generated markdown views are rendered
  deterministically from the registry into marked regions, and CI fails when
  checked-in output is stale.
- **REQ-REG-5 (legacy source-comment bridge):** `tooling/req-status.py` remains
  active until the repeated source-comment status rows are mapped to stable IDs.
- **REQ-REG-6 (generated-region migration):** replacing hand-maintained source
  status tables with generated regions or links is deferred until the stable-ID
  mapping is reviewed.

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
- AC-8: `python3 tooling/req-registry.py --check` is wired into Makefile and CI.

## Migration Plan

1. Land schema v1, validator/generator, generated inventory, routes, and CI.
2. Export the current `req-status.py --json` rows as candidate aliases.
3. Review and assign stable IDs by owner doc/module, not by exact label alone.
4. Add canonical registry records for migrated requirements.
5. Replace repeated source-comment status tables with generated regions or links.
6. Tighten the bridge: fail on unmapped legacy rows once migration coverage is
   high enough to make that signal useful.

## Known Limits

This registry does not prove semantic adequacy. A symbol can exist without being
the right symbol; a command can be recorded without being executed by this gate;
a tracker ref can be parseable without being open. Those checks require later
integration with Rust item indexing, CI job metadata, or tracker adapters.
Schema v1 deliberately keeps those as explicit future hardening points rather
than pretending string validation is proof.
