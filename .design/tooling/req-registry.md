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
`.design/reqs/status.md`. Source comments should keep stable invariants and
non-obvious mechanisms; volatile status, evidence, blockers, and migration state
belong in registry data and generated views.

## Design Decisions

1. **TOML, stdlib-only.** The registry uses TOML because Python 3.11+ includes
   `tomllib`, matching the route-table tooling style. No runtime dependency is
   added for CI or local gauntlet use.
2. **Stable IDs are the identity.** Requirement titles can change; IDs must not.
   Aliases are metadata only. Conflict detection should key on `id`, not prose.
3. **One owner, many contributors.** Every requirement has one owner field: the
   doc/module accountable for status. Evidence can point to any number of files,
   tests, symbols, commands, or issues.
4. **Typed evidence, not proof by prose.** Evidence has a `kind` and `target`.
   `file`, `doc`, and `test` targets must resolve as paths; `symbol` targets
   must resolve in repo text; `issue` targets must use a parseable issue ref;
   `command` targets are recorded but not executed by this gate.
5. **Generated output is checked, not trusted by convention.** CI runs the
   registry with `--check`; generated views must match renderer output exactly.
6. **Legacy comment rows are bridged, not bulk-converted blindly.** The existing
   429 source-comment rows need a reviewed stable-ID mapping. Until that lands,
   `req-status.py` remains as the contradiction tripwire.

## Registry Schema v1

Top-level fields:

- `schema_version = 1`
- `[[view]]`: generated output target
- `[[requirement]]`: canonical requirement record

View fields:

- `name`: stable view name referenced by requirements
- `path`: generated markdown path, under `.design/`
- `kind`: currently `full_inventory`
- `title`: optional generated document title

Requirement fields:

- `id`: stable `REQ-*` token
- `title`: human-readable name
- `owner`: accountable doc/module/path
- `status`: one of `shipped`, `not_started`, `partial`, `blocked`, `deferred`
- `scope`: area such as `tooling`, `forge`, `syntax`, `verified`, or `basis`
- `summary`: short prose summary
- `remaining_scope`: required for `partial`; required for future statuses unless
  blockers alone explain the remaining work
- `aliases`: optional old names or source-comment labels
- `blockers`: issue refs such as `#17`; required for `blocked`
- `generated_to`: named views that should include the requirement
- `[[requirement.evidence]]`: typed evidence entries

Evidence fields:

- `kind`: `file`, `symbol`, `test`, `issue`, `doc`, or `command`
- `target`: path, symbol, issue ref, or command string depending on kind
- `note`: optional human context

## Requirements

- **REQ-REG-1 (stable requirement identity and ownership):** every canonical row
  has a stable ID, title, owner, status, scope, generated-view membership, and
  typed evidence.
- **REQ-REG-2 (accepted status enum):** the status vocabulary is closed over
  `shipped`, `not_started`, `partial`, `blocked`, and `deferred`.
- **REQ-REG-3 (typed evidence validation):** evidence references are mechanically
  checked at the level this gate can honestly validate: path existence, symbol
  occurrence, issue-ref shape, and blocker shape.
- **REQ-REG-4 (generated status views):** generated markdown views are rendered
  deterministically from the registry and CI fails when checked-in output is
  stale.
- **REQ-REG-5 (legacy source-comment bridge):** `tooling/req-status.py` remains
  active until the repeated source-comment status rows are mapped to stable IDs.
- **REQ-REG-6 (generated-region migration):** replacing hand-maintained source
  status tables with generated regions or links is deferred until the stable-ID
  mapping is reviewed.

## Acceptance Criteria

- AC-1: a duplicate requirement ID fails validation.
- AC-2: an unknown status fails validation.
- AC-3: a shipped requirement without file/symbol/test evidence fails validation.
- AC-4: unresolved `file`, `doc`, or `test` evidence fails validation.
- AC-5: `blocked` requirements without issue-shaped blockers fail validation.
- AC-6: `--check` fails when a generated view differs from renderer output.
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
an issue ref can be parseable without being open. Those checks require later
integration with Rust item indexing, CI job metadata, or the GitHub API. Schema
v1 deliberately keeps those as explicit future hardening points rather than
pretending string validation is proof.
