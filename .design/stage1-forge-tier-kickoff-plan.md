# Plan: Stage 1 — the forge tier (L3) + relax routing kickoff

A handoff/kickoff plan for building Stage 1 of the RFC-1 program. Unlike the
comment pass, this is **verification-mandatory feature work**, not a register
change: it follows the strict ACToR `read → write → verify → commit` loop from
`goal.md`, one increment at a time, with an adversarial critic pass between
increments.

## Authority

`.design/stage1-forge-tier.md` is the **authoritative spec** — its REQ-1..REQ-10
clusters and AC-1..AC-14 are the contract; its Architecture section grounds
every increment in the existing tree at the cited files. This plan does **not**
restate the REQs; it sequences them into committable increments and pins the
per-increment verification. The umbrella is `.design/thermite2-program.md`
(REQ-3); the gate is **G1**.

Two design questions are already **resolved** in the spec — do not re-litigate:
- **Q-ORACLE**: the cert oracle subset gains the per-clause *verdict* + *trust*
  strings, the meaning-audit hash, and the covenant record (witness/falsify
  counts + seed). The burn receipt stays oracle-*excluded*.
- **Q-BURN**: `burn.proof_tokens` = lexer-token count of committed proof text;
  `burn.authoring_tokens` optional; both oracle-excluded.

## The headline (what G1 delivers)

Out-of-cage clauses **no longer degrade down the ladder — they escalate up to
the forge** (L3). Stage 1 is a *hardening and widening of the existing Lean
path*, not a greenfield tier: the seven verdicts replace the narrower
`degrade::L3Verdict` / `engine::Verdict::Unknown` vocabularies, the covenant and
battery wrap the existing discharge path, and the axiom gate moves from
audit-time to certify-time.

## Increments (dependency order — commit per increment)

The spec's dependency order is: **1–2 foundation → 3–7 parallelize → 8
independent (may land first) → 9 trails → 10 gate.** Mapped to committable units:

| # | increment | REQs | primary files (all exist today) | self-verify |
|---|---|---|---|---|
| **0** | relax spine lemmas (independent, can land first) | REQ-8a | new Mathlib-island module under `lean/Thermite/` (`r_relax_sound`, `rencode_sound`); axiom probe `[1′]` extended; **+ a Lean-CI job (`lake build` + axiom probe) and lean-smt pinned to a SHA** (audit F4 prereq) | `lake build` green; `make audit` axiom probe covers both lemmas; the new probe runs in CI, not just locally |
| **1** | **foundation** (gates the rest) | REQ-1, REQ-2, + the REQ-4 `Engine`-signature change | **Q-KBSIGNAL signal probe first**; new *separate cert-level* seven-verdict enum (4 verdicts produced upstream, not mapped from `engine::Verdict`); `Engine::discharge` gains the non-optional covenant param (touch all call sites once); axiom-gate hoist to all Lean tiers; correspondence table + repoint the *existing* doc-drift route | full gauntlet + **`oracle_subset` byte-identical on all 7 `conformance/*.cert.json`** (AC-4) |
| **2a** | surface syntax | REQ-3 | `thermite-syntax/src/{parser,address}.rs` (new items beside `fn`; `?pN` holes; proof addresses) | `cargo test -p thermite-syntax`; AST + address round-trips |
| **2b** | covenant engine | REQ-4 | covenant *logic* on the foundation-threaded discharge param: `inhabit` type-check+execute against `req`, `falsify` via `thermite-tv/src/gen.rs` (Q3 50k fixed seed), `CovenantRefuted` hard-fail in `forge/src/degrade.rs` | `cargo test -p forge`; structural no-witness→no-proof-search test |
| **2c** | frozen battery | REQ-5 | new registry data file modeled on `thermite-spec/src/combinators.rs` `REGISTRY`; elaboration-time enforcement; `Stuck` hints | `cargo test -p forge`; unlisted-tactic refusal test |
| **2d** | anti-Goodhart at L3 | REQ-6 | re-elab tautology harness beside `forge/src/vacuity_solver.rs`; reuse `forge/src/mutation.rs` `generate()` (swapped kill check); `forge audit --meaning` in `forge/src/audit.rs` | `cargo test -p forge`; mutation catalogue-shared assertion |
| **2e** | goal/fill UX | REQ-7 | `forge/src/goal_repl.rs` proof view + `fill_hole` `?pN`; burn receipt in cert | `cargo test -p forge`; fill-to-close on merge example |
| **2f** | relax engine route | REQ-8b | new `Engine`-trait impl (relaxable check, nlsat + integrality, `RealWitness` escalation); attribution via `Certificate::with_engine_attribution` | `cargo test -p forge`; isqrt L4 push-button; `∀n. n*n≠2` → `RealWitness` |
| **3** | lemma library mechanics (trails usage) | REQ-9 | per-project lemma namespace; certified-only citation resolution; dedup-on-burn by statement hash; `dec wf` accessibility cache (`forge/src/cache.rs`) | `cargo test -p forge`; uncertified-citation failure + dedup-rewrite tests |
| **G1** | **gate** | REQ-10 | end-to-end on a real merge-class example; README headline flip | full G1 checklist (below) |

Increments **2a–2f** parallelize once increment 1 lands (they depend on the
seven-verdict enum + the certify-time exporter gate, not on each other).
Increment **0** is independent and may run anytime — landing it first de-risks
the Mathlib-island axiom story early.

## Per-increment gauntlet (from `goal.md`)

Every increment, before its commit:
- `cargo test -p <crate>` — green, 0 failures.
- `cargo clippy -p <crate> --all-targets -- -D warnings` — clean (fix, don't suppress).
- `cargo fmt --check` — clean.
- **Cert stability**: `forge check` over `conformance/` still matches all 7
  golden certs; `manifest.rs::oracle_subset` byte-identical for v1 items (new
  forge-tier goldens carry the covenant block from day one — a deliberate
  covenant change is a reviewed golden update, not a silent drift).
- Lean-touching increments (0, and any spine work): `lake build` green; `make
  audit` axiom probe extended to any new theorem, allowed-axiom set unchanged.

Then **commit per increment** (cite the REQ + the `thermite-design.md` §) so
progress survives a timeout. A critic re-audit (`acto-critic`) runs against each
"done" increment before the next starts — it writes a failing test for any
divergence from the AC; `acto-fixer` closes it; never skip-and-commit.

## Hard constraints / scope guards

- **Additive only.** Schema v2 extends `Certificate` without disturbing the v1
  `oracle_subset()` tuple. No `Unknown` survives to a certificate (total map
  from `engine::Verdict`).
- **No cage / combinator-menu change.** The v1 combinator menu is untouched;
  out-of-menu clauses route to the forge instead of degrading — that *is* the
  headline. Touching `thermite-spec`'s `REGISTRY` signatures is out of scope.
- **README/docs headline flips at G1 gate time, not per-merge** (R-GATE-1). The
  "out-of-cage no longer degrades" claim lands only when the G1 checklist is
  green.
- **Hot path stays core-Lean-only.** The two relax spine lemmas live in a single
  isolated Mathlib-importing island; no Mathlib on the denotation path.
- **Never stage `.crosslink/hook-config.json`** (local maintainer variant).
- Syntax (REQ-3) is **always-on, no feature flag**; gating is forge-side via
  holes + routing.

## Out of scope (deferred, do not build)

- The stratified cage — classifier, sort graph, restratify, `Strat/` modules,
  two-phase TV, generator binder productions — **stage 2** (gated on G1; spec
  spike-clean per PR #13, still needs G1 + routing telemetry).
- `@bv` + the three locks; SMT proof reconstruction — **stage 3**.
- Cross-project lemma sharing; human review workflow beyond `forge review`
  surfacing.
- Float/transcendental (W8), string (W25), heap-predicate (W26) fragments.

## Issue tracking

Per the umbrella's Q-TRACK split: **one GH issue per REQ cluster** referencing
G1; build increments tracked as crosslink issues under them. The kickoff creates
its tracking issue automatically.

## Gap-analysis groundings (2026-06-15)

Folded in from a `crosslink kickoff plan` analysis + a git-history sweep; the
authoritative spec was amended to match (see its dated amendment note):

- **Verdict architecture** — the seven verdicts are a *separate cert-level enum*;
  only `Proved`/`Counterexample`/`Timeout` map from `engine::Verdict`, the other
  four are produced upstream. Build the enum + its construction sites, not a
  match on the 3-arm engine type.
- **KernelBudget signal is unverified** (Q-KBSIGNAL) — probe first; fall back to a
  forge-side elaboration-timeout wrapper if no distinct signal text exists.
- **Covenant is a foundation signature change** — `Engine::discharge` gains the
  non-optional covenant param in increment 1, so call sites ripple once.
- **REQ-2 is lighter than it reads** — the exporter soundness gates are already
  shipped (#243–#246/#253/#264); the doc-drift gate is built and already routes
  `lean_export.rs` (#258). New work = axiom-gate hoist to all tiers + author the
  correspondence table + repoint the route. This re-inspection answers the
  trust-audit's finding **F10** (exporter = freshest/least-soaked trust surface);
  the holes F10 implied are now closed, so the table is its durable mitigation.
  (That audit, baseline `93d3cbc0`, predates much of the current tree and is not
  committed — treat its findings as history, the present look as authoritative.)
- **`forge audit` gates nothing** (#274) — certify-time gating lives on the
  discharge path; `forge audit --meaning` stays a read-only projection.
- **Decisions recorded**: `dec wf <rel>` ASCII (Q-DECWF); a new
  `EngineName::Nlsat` direct Z3 nlsat tactic (Q-NLSAT); refinement sugar needs a
  new post-parse desugaring pass.

**Risk hotspots** (from the 19-subtask estimate — weight critic effort here):
REQ-1b KernelBudget signal, REQ-3c desugaring/`dec`-forms, REQ-4 covenant
signature ripple, REQ-6a/b re-elaboration (perf: ≤64 re-typechecks/item within
the 30s budget — verify early), REQ-6c definition-tower instrumentation, REQ-8b/8c
nlsat + integrality, REQ-9 lemma-library schema expansion.

**Audit reconciliation (2026-06-15).** The historical trust-audit (baseline
`93d3cbc0`; preserved as the `trust-audit-93d3cbc0` knowledge page) was
reconciled against HEAD. Stage-1-relevant live items folded in above: **F4**
(Lean spine not in CI; lean-smt on a branch) → increment-0 prerequisite;
**F5b/F9** (README overstates `forge check` / coverage) → G1 headline must split
by regime; **F3** (correspondence still inspection-tier, no mechanized extraction
bridge) → REQ-2's new table stays inspection-tier, does not close F3. **F2**
(negative-div / out-of-range-cast pins) is orthogonal — the `relaxable` check
already excludes div/mod/casts. Addressed since the audit: F1, F5a, F6, F8, F10.

## Recommended execution

Stage 1 is one coherent feature gated by G1, but it is large and high-risk, so
**do not run it as a single all-REQ agent.** Two viable shapes:

1. **Foundation-first, then fan out (recommended).** Kick off increment **0 +
   1** on one `feature/stage1-forge-tier` branch and land it (optionally as its
   own PR — it is independently valuable and de-risks everything). Then run
   **2a–2f** as parallel builder+critic pairs on the same branch (they are
   independent), then **3**, then close the **G1** gate and flip the headline.
   One PR at G1, or a foundation PR + a feature PR.
2. **Single gated kickoff, stop-at-boundary.** One background kickoff that works
   the increments in order, commits per increment, and **stops at an increment
   boundary** if low on time/budget, reporting which increments are done — then
   re-run the same kickoff to resume on the same branch (the comment-pass
   pattern). Lower orchestration overhead; less parallelism.

Either way: one `feature/stage1-forge-tier` branch, `--verify ci`, critic pass
between increments, PR when (at least the foundation, ideally) G1 is green.

## G1 done-when checklist (REQ-10 / AC-14)

- [ ] Merge-class example certifies **L3** (clauses L4, L4, L3) with all four
  evidence blocks populated: covenant, certify-time axiom gate, re-elaboration
  mutation score, burn receipt.
- [ ] Each of the seven verdicts exercised by a hermetic test (seven tests named
  for their verdict); `CovenantRefuted`/`Counterexample` proven (instrumented
  closures) never to invoke a lower-ladder/retry path.
- [ ] `forge check` over `conformance/` matches all existing golden certs — zero
  regressions.
- [ ] Exporter re-inspection correspondence table merged + routed by the
  doc-drift gate.
- [ ] README/docs headline updated (gate-time) — and, per audit F5b/F9, the
  flip **splits claims by regime** (fragment-covered = kernel + per-run TV vs
  fragment-external = Verus-only, lowering by inspection) and separates
  `forge check` from the faithfulness (TV) leg. The new "out-of-cage escalates
  to the forge" claim lands honestly, not on top of the existing overstatement.

## Context already landed (do not redo)

- **M0 de-risking spikes** complete: SPIKE-1 (plain de Bruijn, no `Fintype`),
  SPIKE-2 (100% normalizer hit rate) — merged.
- **Stage-2 spec** spike-clean (PR #13) — but it is stage 2, gated on this gate
  (G1). Do not build it here.
- **Source-comment tone pass** (PR #14) is the rebase base this work sits on;
  `main` already carries the retoned comments.
