---
rfc: 2
title: Canonical REQ registry and generated status tables
status: accepted
supersedes: []
introduces: []
discussion: https://github.com/dollspace-gay/Thermite/issues/17
---

## Context

The current doc-drift gate catches routed design documents that are stale relative to audited file SHA pins. That is useful, but it does not catch semantic drift inside long source comments. The recent `REQ-5 (forge plug-in point)` mismatch is the concrete example: one module documented the forge TV plug-in point as `NOT-STARTED` while the forge side documented and implemented it as `SHIPPED`.

The new `tooling/req-status.py` lint is a pragmatic tripwire for this class of mistake. It scans `//! | REQ ... | SHIPPED/NOT-STARTED | ... |` source-comment rows and fails when exact labels disagree, `NOT-STARTED` rows do not cite future/deferred scope, or `SHIPPED` rows lack at least one resolving backtick file/symbol citation.

That guard is intentionally mechanical. It reduces obvious contradictions, but it should not become the long-term source of truth.

## Problem

REQ status is currently repeated in multiple long source comments. That creates several failure modes:

- The same requirement can be renamed slightly and evade exact-label matching.
- A real symbol/file citation can exist without actually proving the status claim.
- Future/blocker wording is keyword-based and does not prove that an issue exists or is the correct blocker.
- Source comments mix stable implementation invariants with volatile project status and history.
- Ownership is implicit, so there is no machine-readable answer to which module owns a requirement.

## Proposal

Introduce a canonical machine-readable REQ registry and generate status tables from it instead of hand-maintaining them in source comments.

The registry should assign each requirement a stable ID and explicit ownership, for example:

- `id`: stable requirement ID, e.g. `REQ-5`
- `title`: human-readable name
- `owner`: owning crate/module/doc
- `status`: accepted enum such as `shipped`, `not_started`, `partial`, `blocked`, `deferred`
- `scope`: contract/exec/forge/spec/docs/etc.
- `evidence`: typed entries, not prose-only citations
- `blockers`: issue/PR IDs required for non-shipped work
- `generated_to`: source/doc locations where rendered tables should appear
- `last_reviewed`: optional review marker for high-risk requirements

Evidence should be typed so the gate can validate more than string existence:

- `symbol`: resolves to a real Rust path or item where practical
- `file`: resolves to a repo path
- `test`: resolves to a test file/name and preferably a CI job target
- `issue`: resolves to an open/closed GitHub issue depending on status
- `doc`: resolves to a design doc section or routed doc pin
- `command`: names the verification command expected to cover the requirement

## Generated Output

Generate markdown tables for source comments and design docs from the registry. Source comments should keep durable invariants and non-obvious mechanisms. Volatile status, shipped evidence, blockers, and history should be generated into a small number of status views or inserted into marked generated regions.

Possible generated views:

- full REQ inventory
- per-crate REQ status
- not-started/blocked work queue
- shipped evidence index
- requirements with weak or missing verification commands

## Enforcement Plan

1. Keep `tooling/req-status.py` as the short-term contradiction lint.
2. Add a registry file, likely under `.design/reqs/` or `tooling/reqs/`.
3. Add a generator that renders tables from the registry.
4. Add a check that generated tables are up to date.
5. Replace hand-written source-comment status rows with generated rows or links to generated status docs.
6. Tighten validation once evidence is typed:
   - `shipped` requires test/symbol/file evidence.
   - `not_started` requires blocker or deferred scope.
   - `blocked` requires an open issue.
   - `partial` requires explicit remaining scope.
   - generated table diffs fail CI.

## Known Edge Cases To Design For

- Requirement aliases and renamed titles should still map to stable IDs.
- Requirements spanning multiple crates need one owner plus contributors, not multiple conflicting owners.
- Generated regions should be easy to review and should avoid excessive churn.
- Evidence may be valid only under features or external tools such as Verus; the schema should capture that.
- Test files can exist while tests are skipped, ignored, or not run in CI; evidence should eventually link to commands/jobs.
- Some requirements are intentionally aspirational or roadmap-level; those need an explicit status instead of being forced into `NOT-STARTED` prose.

## Acceptance Criteria

- A canonical registry exists with stable IDs, owners, statuses, and typed evidence.
- Generated status views replace duplicated hand-written REQ status tables.
- CI fails when generated output is stale.
- CI fails when registry entries have invalid statuses, unresolved evidence, or missing blockers.
- Existing source comments retain durable design invariants while volatile status moves to generated output.

This should make doc drift a data consistency problem instead of a prose archaeology problem.
