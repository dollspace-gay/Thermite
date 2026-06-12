# Cargo Workspace Scaffold
<!--
tier: 3-component
status: draft
audited-sha: 05c65b73b8ed8629ff65080f836aec0a32eef7e5 (bootstrap pin: decision 4 — doc-last-touch, NOT verified-current; backlog #262)
governs:
  - Cargo.toml (virtual workspace manifest)
  - rust-toolchain.toml
  - thermite-syntax/{Cargo.toml,src/lib.rs}
  - thermite-spec/{Cargo.toml,src/lib.rs}
  - thermite-lower/{Cargo.toml,src/lib.rs}
  - forge/{Cargo.toml,src/main.rs}
  - thermite-skill/{Cargo.toml,src/lib.rs}
  - .github/workflows/ci.yml
thesis-refs:
  - thermite-design.md §3
  - thermite-design.md §13
-->

## Summary

The scaffold is the empty-but-buildable Cargo workspace that every other v0.1
kernel unit lands inside. It fixes the crate topology (the five member crates
of `tooling/spec-routes.toml`), the internal dependency DAG (leaf-first per
R-DEFER-7), the shared error / `Result` discipline (R-CODE-2), the pinned Rust
edition + MSRV for determinism (R-CODE-5), and the CI gauntlet that is the
acceptance gate. Nothing in this scaffold contains language logic — it is the
skeleton whose only job is to compile clean and pass the full gauntlet green
on an empty workspace.

This doc is GREENFIELD and FORWARD-LOOKING: there is no toolchain code yet
(no `*.rs`, no `Cargo.toml` in the tree). Every REQ below is therefore
**NOT-STARTED**, blocked on issue #1 (the scaffold itself). The acto-builder
satisfies these REQs next; this doc is the contract it builds to.

## Requirements

- **REQ-1 (workspace topology):** A virtual Cargo workspace (root `Cargo.toml`
  with `[workspace]`, no root `[package]`) whose members are exactly the five
  crates implied by `tooling/spec-routes.toml`: `thermite-syntax`,
  `thermite-spec`, `thermite-lower`, `forge`, `thermite-skill`. `forge` is the
  sole binary crate (the CLI); the other four are libraries. No crate is
  invented that the routes / `goal.md` do not imply. Derived from
  `thermite-design.md §3` (the stack: Forge toolchain over the surface language,
  lowering through Rust) and `goal.md` "Scope" dependency order.

- **REQ-2 (dependency DAG, leaf-first):** Internal crate dependencies form an
  acyclic graph in the order `goal.md` mandates (R-DEFER-7):
  - `thermite-syntax` — no internal dependencies (the foundation / leaf).
  - `thermite-spec` — depends on `thermite-syntax` (consumes the AST).
  - `thermite-lower` — depends on `thermite-syntax` + `thermite-spec`.
  - `forge` — depends on all three libs (`thermite-syntax`, `thermite-spec`,
    `thermite-lower`) and on `thermite-skill`.
  - `thermite-skill` — depends on `thermite-spec` (combinator registry) +
    `thermite-syntax` (grammar).
  Circular dependencies are forbidden; `cargo build --workspace` cycle-checks
  this for free. Derived from `goal.md` "Scope" (the 1→5 dependency order) and
  R-DEFER-7.

- **REQ-3 (Result discipline; error types deferred):** Every crate observes the
  R-CODE-2 contract — fallible operations return `Result<T, _>` with
  context-bearing error variants; no `unwrap`/`expect`/`panic!` in production
  (outside `#[cfg(test)]`). **Decision (orchestrator, overriding the original
  shared-`ThermiteError`-in-`thermite-syntax` proposal): the scaffold creates NO
  error type.** Reasons: (a) a `pub enum ThermiteError` with a placeholder
  variant and no production consumer is vocabulary-only and violates R-DEFER-1
  in the very first commit; (b) anchoring a toolchain-wide error in the parser
  crate is backwards coupling — forge's future `solver-timeout` variant must not
  live in `thermite-syntax`. Instead, **each crate introduces its OWN error enum
  (e.g. `thermite_syntax::SyntaxError`, `forge::ForgeError`) when its first
  fallible function lands in the owning component issue**, and `forge` aggregates
  downstream errors via `#[from]` conversions. The scaffold's empty
  `lib.rs`/`main.rs` therefore contain no error type at all. This REQ records the
  Result-discipline convention as binding; the per-crate error enums are verified
  in their owning issues, not here. Derived from R-CODE-2, R-DEFER-1, and
  `thermite-design.md §2.4` (crisp structured feedback).

- **REQ-4 (pinned edition + MSRV via `rust-toolchain.toml`):** A
  `rust-toolchain.toml` pins a concrete toolchain channel so builds, formatting,
  and (later) codegen are bit-reproducible across machines (R-CODE-5,
  `thermite-design.md §5.3`). **Decision (orchestrator, aligned to the installed
  stable toolchain `rustc 1.95.0`): edition `2021`, pinned stable channel
  `1.95.0`** (set both `rust-toolchain.toml` `channel = "1.95.0"` and a
  workspace `rust-version = "1.85"` MSRV floor). Justification: edition 2021 is the
  stable, broadly-supported edition and is the edition the Verus/Kani
  toolchains (arriving issues #4/#9) interoperate with; a pinned stable channel
  (not `nightly`) keeps the scaffold reproducible and does not foreclose the
  later Verus integration, which is invoked as an out-of-process transpilation
  target (`thermite-design.md §3`: "transpile to Verus instead") rather than as
  an in-tree nightly proc-macro dependency. The scaffold does NOT wire Verus or
  Kani — those land in #4/#9 — but the pin must not break that future (no
  edition or channel choice that the Verus passthrough cannot consume). See
  open question OQ-2 on the exact patch version.

- **REQ-5 (CI gauntlet as acceptance gate):** A GitHub Actions workflow at
  `.github/workflows/ci.yml` runs the full gauntlet on the workspace, and each
  command is a hard gate (non-zero exit fails CI):
  1. `cargo build --workspace`
  2. `cargo test --workspace`
  3. `cargo clippy --workspace --all-targets -- -D warnings`
  4. `cargo fmt --all --check`
  This mirrors the per-crate gauntlet in `goal.md` ("Gauntlet (every crate)")
  hoisted to the workspace level for the scaffold gate. Derived from `goal.md`
  "The verification model" gauntlet definition and R-DEFER-6 (verification is a
  hard gate). The skill-budget CI step is explicitly OUT of scope here (see
  REQ-7 / OQ-3).

- **REQ-6 (empty scaffold compiles clean — anti-stub):** Each crate has a
  minimal `lib.rs` / `main.rs` containing NO `todo!()`/`unimplemented!()`/
  `unreachable!()`, NO `unwrap`/`expect`/`panic!` outside `#[cfg(test)]`, NO
  module-root `#![allow(..)]`, and NO declared-but-missing modules (`mod foo;`
  pointing at a file that does not exist). `forge`'s `main.rs` is a real entry
  point that exits cleanly (returns `()` / exits 0; no error type yet — forge's
  `ForgeError` lands with the CLI in #5; it does not `panic!`). The full gauntlet (REQ-5) passes green on this empty
  workspace. Derived from R-DEFER-9 / R-APG-1 (anti-pattern gate) and
  `goal.md` "empty scaffold compiles clean" intent. The per-route source files
  named in `spec-routes.toml` (e.g. `thermite-syntax/src/lexer.rs`) are NOT
  created by the scaffold — they arrive in their owning component issues; the
  scaffold ships only `lib.rs`/`main.rs` so there are no declared-but-missing
  module references.

- **REQ-7 (skill-budget gate is deferred to #7):** The 6,000-token
  `THERMITE.skill.md` budget gate and its CI step
  (`cargo run -p thermite-skill -- --check-budget`, per `goal.md` "Gauntlet")
  are NOT part of this scaffold. `thermite-skill` exists as an empty member
  crate (REQ-1) but its generator and budget CI step land in issue #7
  (`thermite-design.md §2` pillar 2 / §10 "the skill is the spec"). This REQ
  records the boundary so the scaffold's CI does not claim a gate it does not
  enforce. Derived from `thermite-design.md §10` and crosslink issue #7.

## Acceptance criteria

- **AC-1 (members):** `cargo metadata --no-deps --format-version 1` lists
  exactly five workspace members with package names `thermite-syntax`,
  `thermite-spec`, `thermite-lower`, `forge`, `thermite-skill`, and the root
  `Cargo.toml` has a `[workspace]` table and no `[package]` table. Exactly one
  member produces a `bin` target (`forge`); the other four produce a `lib`
  target only. (REQ-1)

- **AC-2 (DAG + acyclicity):** For each crate, its `Cargo.toml`
  `[dependencies]` path-deps equal exactly the set in REQ-2 (no more, no less).
  `cargo build --workspace` succeeds (Cargo errors on dependency cycles), and
  `thermite-syntax/Cargo.toml` declares zero intra-workspace path
  dependencies. (REQ-2)

- **AC-3 (Result discipline; no scaffold error type):** No error type is
  created at scaffold time — `rg -n 'enum ThermiteError'` returns nothing, and
  no crate re-exports a shared error. `rg` finds no `.unwrap()`/`.expect(`/
  `panic!(` outside `#[cfg(test)]` in any crate's `src` (trivially satisfied by
  the empty crates). The Result-discipline convention is documented; per-crate
  error enums are verified in their owning component issues, not here. (REQ-3)

- **AC-4 (toolchain pin):** `rust-toolchain.toml` exists with a concrete
  `channel` (a pinned version string, not `stable`/`nightly` floating) and a
  declared `edition`/`rust-version` MSRV in the workspace manifest;
  `cargo +<pinned> build --workspace` succeeds under the pinned toolchain.
  (REQ-4)

- **AC-5 (gauntlet green):** All four gauntlet commands exit 0 on the empty
  workspace:
  `cargo build --workspace`, `cargo test --workspace`,
  `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo fmt --all --check`. The CI workflow file `.github/workflows/ci.yml`
  invokes all four as separate, must-pass steps. (REQ-5, REQ-6)

- **AC-6 (no stubs / no missing modules):** `rg -n 'todo!\(|unimplemented!\(|unreachable!\(' --glob '*.rs'`
  returns nothing in production code; `rg -n '#!\[allow' --glob '*.rs'` returns
  nothing at module/crate root; `cargo build --workspace` (which fails on a
  `mod x;` with no `x.rs`) succeeds. No file listed in `spec-routes.toml` other
  than `lib.rs`/`main.rs` is created by the scaffold commit. (REQ-6)

- **AC-7 (skill gate absent by design):** `.github/workflows/ci.yml` contains
  NO `--check-budget` step, and `thermite-skill/src/generate.rs` is NOT created
  by the scaffold. The doc explicitly attributes the budget gate to issue #7.
  (REQ-7)

## Architecture

The scaffold is a **virtual Cargo workspace**: the root `Cargo.toml` carries a
`[workspace]` table with `members = [...]` and no `[package]`, so the root is
not itself a publishable crate. This is the conventional layout for a
multi-crate toolchain and keeps each component independently testable
(`cargo test -p <crate>`), which the per-crate gauntlet in `goal.md` requires.

The crate set and file layout are fixed by `tooling/spec-routes.toml` — the
authoritative module map (`goal.md` "Scope": *"the route table is the
authoritative module map"*). The scaffold materializes the five crates it
names:

```
Cargo.toml                     # [workspace] members = the five crates below
rust-toolchain.toml            # channel + edition/MSRV pin (REQ-4)
.github/workflows/ci.yml       # the gauntlet (REQ-5)
thermite-syntax/   (lib)   leaf; owns ThermiteError; routes: lexer/parser/ast/address.rs
thermite-spec/     (lib)   dep: thermite-syntax;       routes: combinators/grammar.rs
thermite-lower/    (lib)   dep: thermite-syntax, thermite-spec; routes: lower/l1/effects.rs
forge/             (bin)   dep: all libs;              routes: cli/check/manifest/vacuity/slag/cache.rs
thermite-skill/    (lib)   dep: thermite-spec, thermite-syntax; route: generate.rs
```

The dependency DAG (REQ-2) reflects the data flow of the toolchain in
`thermite-design.md §3`: source text → tokens/AST (`thermite-syntax`) → spec
combinators over that AST (`thermite-spec`) → lowering to Verus-annotated Rust
(`thermite-lower`) → driven by the CLI (`forge`); `thermite-skill` reads the
grammar and combinator registry to emit `THERMITE.skill.md` (§10). The order is
exactly the `goal.md` "Scope" sequence, which R-DEFER-7 (no leapfrog) makes
binding: `thermite-syntax` before `thermite-spec` before `thermite-lower`
before `forge`.

**Error handling (deferred).** The scaffold creates no error type (REQ-3). A
shared `ThermiteError` was considered and rejected: a `pub` enum with no
production consumer is vocabulary-only (R-DEFER-1 violation in the first
commit), and anchoring a toolchain-wide error in the parser crate is backwards
coupling (forge's solver-timeout variant must not live in `thermite-syntax`).
Each crate instead grows its own error enum when its first fallible function
lands; `forge` aggregates downstream errors via `#[from]`. The empty
`lib.rs`/`main.rs` carry no error type. R-CODE-2's "no `unwrap`/`panic` in
production" stands as a convention from commit one.

**Toolchain pin.** `rust-toolchain.toml` pins a concrete stable channel and the
workspace declares an MSRV `rust-version` (REQ-4). Determinism is a contract
(R-CODE-5, `thermite-design.md §5.3`: *"Builds, formatting, codegen, and check
results are bit-reproducible given the same toolchain version and solver
seeds"*). The scaffold pin is the toolchain-version half of that contract;
solver-seed pinning belongs to `forge`'s proof-cache / check path (issue #8),
not here. Verus/Kani (`thermite-design.md §3`: *"Verification reuses the Verus
and Kani toolchains"*) arrive in issues #4/#9 as out-of-process transpilation
targets; the scaffold must not foreclose them, but wires neither — hence a
stable channel rather than a Verus-specific nightly.

**The skill-budget boundary.** Pillar 2 (`thermite-design.md §2`) — *"The whole
language fits in a skill … a hard budget, enforced in CI"* — and §10 mandate a
6k-token CI gate. That gate is issue #7, NOT the scaffold (REQ-7). The scaffold
ships `thermite-skill` as an empty member only.

## Verification

Discharge is entirely mechanical and runs on the empty workspace once the
builder lands it:

- **AC-1/AC-2:** `cargo metadata --no-deps --format-version 1 | python3 -c "..."`
  to assert the five member names, the single `bin` target, and per-crate
  path-dep sets; `cargo build --workspace` to confirm acyclicity.
- **AC-3:** `cargo build --workspace` (re-exports resolve) +
  `rg -n '\.unwrap\(|\.expect\(|panic!\(' thermite-*/src forge/src --glob '!*test*'`
  (cross-checked against `#[cfg(test)]` scoping) returns no production hits.
- **AC-4:** presence + concrete-version check of `rust-toolchain.toml` and the
  workspace `rust-version`; `cargo build --workspace` under the pinned channel.
- **AC-5:** the four gauntlet commands each exit 0; the CI YAML names all four
  as distinct steps. This is the gate `goal.md` defines and R-DEFER-6 enforces.
- **AC-6:** the anti-pattern greps (`todo!`/`unimplemented!`/`unreachable!`,
  module-root `#![allow]`) return nothing; `cargo build --workspace` (which
  fails on a dangling `mod`) passes; a diff of the scaffold commit shows only
  `lib.rs`/`main.rs` per crate, no other routed source files.
- **AC-7:** grep `.github/workflows/ci.yml` for `--check-budget` (must be
  absent); confirm `thermite-skill/src/generate.rs` does not exist after the
  scaffold commit.

There is no conformance-corpus or golden-file check at scaffold time — the
scaffold contains no language behavior. The corpus (`conformance/sum.th`,
`conformance/sum.cert.json`, `conformance/binary_search.th`) is exercised by
`forge`/`thermite-lower` in their own issues, not here.

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (workspace topology) | SHIPPED | root `Cargo.toml` is a virtual workspace (`[workspace]`, no `[package]`) with exactly the five members; `forge` is the sole `bin` (its `[[bin]]`), the other four are `lib`. |
| REQ-2 (dependency DAG, leaf-first) | SHIPPED | per-crate `[dependencies]`: `thermite-syntax` (none), `thermite-spec`→syntax, `thermite-lower`→syntax+spec, `forge`→all three libs+skill, `thermite-skill`→spec+syntax; `cargo build --workspace` green (acyclic). |
| REQ-3 (Result discipline; error types deferred) | SHIPPED | no error type created (`rg 'enum ThermiteError'` empty); no `unwrap`/`expect`/`panic!` in any `src` (empty crate roots + `fn main` returning `()`). |
| REQ-4 (edition + MSRV pin) | SHIPPED | `rust-toolchain.toml` pins `channel = "1.95.0"` + `components = ["rustfmt","clippy"]`; `[workspace.package]` sets `edition = "2021"`, `rust-version = "1.85"`; each crate inherits via `.workspace = true`. |
| REQ-5 (CI gauntlet gate) | SHIPPED | `.github/workflows/ci.yml` runs the four gauntlet commands as four separate must-pass steps; no `--check-budget`. |
| REQ-6 (empty scaffold compiles clean) | SHIPPED | only `lib.rs`/`main.rs` materialized per crate; no stubs, no module-root `#![allow]`, no dangling `mod`; `forge/src/main.rs` exits 0; full gauntlet green. |
| REQ-7 (skill-budget gate deferred to #7) | NOT-STARTED | open prereq issue #7; scaffold CI has no `--check-budget` step and `thermite-skill/src/generate.rs` is not created — boundary recorded, gate tracked by #7. |

## Open questions (for the orchestrator before the builder runs)

- **OQ-1 (thermite-spec membership vs issue #1 comment):** The `goal.md` Scope
  and `tooling/spec-routes.toml` both list `thermite-spec` as a distinct crate
  (`thermite-spec/src/{combinators,grammar}.rs`), but the issue #1 `[decision]`
  comment enumerates only `thermite-syntax`, `thermite-lower`, `forge`,
  `thermite-skill` (it omits `thermite-spec`). This doc follows the route table
  + `goal.md` (the authoritative module map) and includes all five crates. If
  the orchestrator intends `thermite-spec` to be folded into another crate,
  REQ-1/REQ-2 must be amended (and the routes adjusted by the builder) before
  scaffolding. Not filed as a blocker — it is resolvable by confirming the
  route table is authoritative, which `goal.md` already states.

- **OQ-2 (exact MSRV patch version):** REQ-4 picks edition 2021 / channel
  `1.78.0` as a concrete, defensible pin, but the precise patch version is a
  judgment call the orchestrator may want to set to the team's installed
  toolchain. Any pinned stable ≥ the version supporting `--all-targets` clippy
  and edition 2021 satisfies the ACs; the builder should confirm the chosen
  version is installed locally so the gauntlet runs.

- **OQ-3 (skill gate CI ownership):** REQ-7 excludes the `--check-budget` step.
  Confirmed against issue #7 (the budget gate is its deliverable, and #7 is
  blocked by #2). Recorded, not blocking.

## Orchestrator resolutions (2026-06-04, before builder dispatch)

- **OQ-1 → RESOLVED: five crates, `thermite-spec` included.** The route table +
  `goal.md` are the authoritative module map; the abbreviated issue-#1 comment
  is not. REQ-1/REQ-2 stand as written (five members).
- **OQ-2 → RESOLVED: channel `1.95.0`, MSRV `1.85`, edition `2021`.** Aligned to
  the installed stable toolchain (`rustc 1.95.0`) so the gauntlet runs green
  locally and in CI. REQ-4 amended.
- **OQ-3 → RESOLVED: skill-budget gate stays in #7.** REQ-7 stands.
- **Error architecture → OVERRIDDEN:** the original shared-`ThermiteError`-in-
  `thermite-syntax` proposal is rejected (vocabulary-only / R-DEFER-1; backwards
  coupling). The scaffold creates no error type; per-crate error enums land in
  owning issues. REQ-3, AC-3, and the Architecture section amended accordingly.
