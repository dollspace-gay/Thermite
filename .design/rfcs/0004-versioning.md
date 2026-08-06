---
rfc: 4
title: Versioning — what a Thermite version number promises
status: draft
supersedes: []
introduces: []
discussion: https://github.com/dollspace-gay/Thermite/issues/120
---

# RFC-3: Versioning — what a Thermite version number promises

| | |
|---|---|
| **Status** | Draft for discussion — not routed, not content-pinned |
| **Companion** | [RFC-2](https://github.com/dollspace-gay/Thermite/issues/119) — coupled; the Lx removal is a beta-line break under this contract |
| **Baseline** | `dollspace-gay/Thermite @ 84d276e7` (gates G1–G4 shipped) |

---

## 0. Where we are

| | |
|---|---|
| `workspace.package.version` | **`0.0.1`** |
| git tags, entire repository | **two** — `v0.0.1`, `v0.0.2` |
| gates shipped under `0.0.1` | **four** — G1, G2, G3, G4 |
| `CHANGELOG.md` sections | `[Unreleased]`, G3, G2, G1, `v0.1 — baseline architecture` |
| occurrences of "G4" in the changelog | **zero** |

The changelog states the current position explicitly:

> Because Thermite is a verification toolchain developed against the RFC-1
> program (GH issue #2) rather than a semver-released library, entries are
> organized by the program's **stage gates** (G1, G2, G3).

That was a reasonable call for a research program with one consumer. It has two consequences that are now costing something:

1. **A four-gate program, including a trust-base migration, shipped entirely under `0.0.1`.** Nothing in the version stream distinguishes the toolchain before and after checked reconstruction.
2. **The changelog still says "Stage 3 completes the RFC-1 program,"** written before Stage 4 existed, and has no G4 entry. The gate vocabulary drifted because gates were doing a job — marking releases — that they are not shaped for.

There is also now a second consumer. A downstream repository pins Thermite at a commit SHA, because a commit SHA is the only thing precise enough to pin.

## 1. What a version contracts over

Thermite is not a library, so "the public API" needs saying explicitly. The version makes promises about **four surfaces**:

| surface | what breaks | who notices |
|---|---|---|
| **Certificate schema** | the per-clause record: level/tuple, engine, trust, verdict, evidence blocks | anyone consuming or archiving certificates |
| **Assurance semantics** | *what a given certification claims* — even with the schema byte-identical | anyone relying on a certificate to mean something |
| **Language surface** | `.th` grammar: clause forms, tags, effects, refinement sugar | every program |
| **Forge CLI / method surface** | command names, flags, exit codes, JSON shapes | every script and CI pipeline |

The second is the one conventional semver has no vocabulary for, and it is the one that has already bitten.

## 2. The motivating example

`Level::L3` came to denote two mechanisms with materially different trust bases and refutation stories — general Verus/Z3, and the Lean forge. No schema field changed. No CLI flag changed. No `.th` program changed.

**But what an `L3` certificate claimed became strictly weaker**, and every archived `L3` certificate became ambiguous retroactively: the number never carried enough information to say which mechanism produced it (see [RFC-2](https://github.com/dollspace-gay/Thermite/issues/119) §6).

Under this RFC that is a **breaking change to assurance semantics** — and it shipped silently, under `0.0.1`, with no release boundary marking it.

That is the argument for putting assurance semantics inside the version contract. A change that alters what your evidence means is breaking even when every byte of the schema is identical.

## 3. Gates and versions are different vocabularies

**Gates are milestones. Versions are contracts.** They answer different questions and should not be made to substitute for each other:

- a gate says *this stage's headline claim is now defensible*
- a version says *here is what I promise about the four surfaces*

A release may contain zero gates, one, or several. A gate may land mid-release without changing any contract.

**Rule.** Gate headings are nested **inside** semver release sections in the changelog, never in place of them. The gate vocabulary stays; it stops being the shipping unit.

## 4. Pre-release semantics

**`2.0.0-beta.N` means the certification surface is still moving.** Specifically:

- The certificate schema **may break between betas.** Consumers pin an exact beta, and the changelog says what moved.
- Assurance semantics may be refined between betas, and **any such change must be called out explicitly** — that is the class of change §2 shows is otherwise invisible.
- Language surface and CLI changes follow ordinary semver intuitions within the beta line: additive freely, removals called out.

**`2.0.0` is the point the certificate schema freezes.** After it, a schema break requires a major bump, and assurance-semantics changes require one too.

This is what lets RFC-2 and RFC-3 land in parallel: removing Lx breaks both the schema and the assurance semantics, which is permitted inside the beta line and is exactly what the beta line is for.

## 5. Schema sprawl — govern the relationship, not the numbers

Six things are independently versioned today, with no stated relationship between any of them:

| | where |
|---|---|
| `workspace.package.version = "0.0.1"` | `Cargo.toml` |
| `CHECK_SCHEMA_VERSION: u32 = 7` | `forge/src/cache.rs:126` — **private**, a cache-key input |
| `ThermiteBootableKernelReceiptV1` | kernel image receipt |
| `ThermiteBootableKernelValidationV1` | kernel image validation report |
| `ThermiteKernelPlatformProfileV1` | `platform/*/profile.toml` |
| `ThermitePlatformRegistryBindingV1` | `platform/*/registry.toml` |

RFC-3 does **not** unify these. Coupling unrelated artifacts means a kernel receipt tweak bumps the toolchain, which is worse than the current situation.

Instead: **every schema that crosses a repository boundary must declare, in one published place, its compatibility relationship to the project version.** Is it frozen? Does it track minor releases? Does a break in it require a project major? Independent numbering survives; it stops being arbitrary.

Schemas that do not cross a boundary — `CHECK_SCHEMA_VERSION` is internal to the proof cache — are explicitly *not* public contract, and should be marked as such so nobody pins to them.

## 6. The prerequisite: certificates carry no version marker

`forge/src/manifest.rs:489` — the `Certificate` struct has **no schema version field**. `CHECK_SCHEMA_VERSION` is a private constant in `cache.rs` used as a cache-key input, not a certificate field, and no `schema_version` field exists anywhere in the codebase.

So a consumer holding a certificate cannot determine which schema produced it. The 12 checked-in oracle files are pre-RFC-2 schema and are indistinguishable from post-RFC-2 ones except by inspecting which fields happen to be present.

**This is a prerequisite for RFC-2's R2-9** (removing Lx) and arguably for any schema change at all. It should land first, and it is small: one additive field, set at emit time.

## 7. The cut

| | |
|---|---|
| **`2.0.0-beta.1`** at `84d276e7` | Thermite 2, gates G1–G4 complete. Set `workspace.package.version` to match. |
| `2.0.0-beta.N` | per RFC-2 increment or gate |
| `2.0.0-rc.1` | when certificate schema v3 is stable |
| `2.0.0` | when the schema freezes and the assurance vocabulary is final |

Changelog work that comes with it: backfill a G4 section, strike "Stage 3 completes the RFC-1 program," and re-nest the existing G1–G3 headings under release sections.

Why `2.x` rather than continuing `0.x`: the product is called **Thermite 2**, the RFC-1 program that defines it is complete through G4, and the artifacts say `0.0.1`. The mismatch is not cosmetic — it is the same class of error the rest of this document is about, where the record does not say what happened.

## 8. Identifier convention

While we are here, because there will be more of these:

**RFC numbers are identifiers assigned at draft time, not queue positions.** Acceptance and posting order routinely differ from numeric order — as with PEPs and Rust RFCs — and nothing is renumbered to compensate. RFC-3 may land before RFC-2 without either being wrong.

The same principle is the subject of this RFC: an identifier's job is to name a thing unambiguously, not to encode its position in a sequence.

## 9. Increments

| # | increment | risk |
|---|---|---|
| R3-1 | Add a schema version field to `Certificate` (§6) | low — additive, prerequisite for RFC-2 |
| R3-2 | Cut `2.0.0-beta.1`; set `workspace.package.version` | low |
| R3-3 | Restructure `CHANGELOG.md`: gates nested inside releases; backfill G4; strike the stale completion claim | low |
| R3-4 | Publish the four-surface contract and the pre-release semantics | low — documentation |
| R3-5 | Declare each public schema's compatibility relationship; mark internal ones non-contract | low |

## 10. Open questions

- **OQ-1** — Does a downstream consumer pin a version or a commit SHA? Today only a SHA is precise enough. If versions are to be pinnable, tags must be cut at a cadence that makes that practical.
- **OQ-2** — Should assurance-semantics changes get their own changelog section, separate from Added/Changed/Fixed? They are the class most easily missed, and §2 is the evidence.
- **OQ-3** — What is the deprecation window inside a beta line? "May break between betas" is permissive; a stated minimum notice may be worth more than the freedom.
