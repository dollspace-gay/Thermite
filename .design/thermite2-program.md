# Feature: Thermite 2 program (RFC-1) — umbrella

## Summary

This is the program-level umbrella for RFC-1 ("Thermite 2 — a dependent-type
tier, a stratified cage, and new ladder boundaries", GH issue #2 on
`dollspace-gay/Thermite`) and its three companion documents in the same
thread: the stage-2 metatheory sketch, the program plan, and Appendix A (the
walls + fallback atlas). It indexes the chosen configuration (C3′), the
milestone tree (M0–M4) with its stage gates (G1–G3), the two week-one
de-risking spikes, the pre-M1 debt interleave, and the adopted defaults from
the Q1–Q10 open-question register — re-baselined from the RFC's pinned
`93d3cbc0` to the current tree at `c46da3ac` (18 commits of drift, deltas
catalogued in Architecture). Per-stage design docs follow as separate
`/design` passes; this doc is their index and the program's single
source of sequencing truth in the `.design/` tree.

## Program documents of record

| document | where | role |
|---|---|---|
| RFC-1 | GH issue #2 body | The specification: ladder, seven verdicts, cage admission, forge tier, `@bv`, certificates, anti-Goodhart, staging + decision record |
| Stage-2 metatheory sketch | GH issue #2, comment 2 | The S₂ fragment, T1-S/T2-S/T3-C/T4-R/T5-X, `lean/Thermite/Strat/` module plan, negative-pin battery, audit checks [1′][4′][8][9] |
| Program plan | GH issue #2, comment 3 | SPIKE-1/SPIKE-2, stage-1 work breakdown (9 items), milestone tree + gates, debt interleave, Q1–Q10 register, metrics, governance deliverables |
| Appendix A | GH issue #2, comment 4 | The walls W1–W26, the C1–C4 configuration space, the fallback atlas F-A…F-J and the global retreat order |
| This doc | `.design/thermite2-program.md` | Program index, re-baseline, requirement/acceptance framing for the scaffolding work |

## Requirements

- REQ-1: SPIKE-1 (the SubstKit toy: 3-constructor `Frm`, plain de Bruijn,
  `sdenote_subst` + `sdenote_push_lift` end to end, one broken-`lift`
  micro-pin) and SPIKE-2 (the normalizer probe: corpus contracts
  hand-expanded to raw-quantifier S₂ form, NNF + prenex + canonical-binder
  normalizer, syntactic-equality hit rate measured) are opened as tracked
  issues with the program plan's acceptance criteria quoted verbatim, before
  any stage-1 or stage-2 implementation issue opens. M0 gates on both.
- REQ-2: The four pre-M1 debt items land before any M1-tagged issue closes:
  (a) a Lean CI job building `lean/` via `lake` in
  `.github/workflows/ci.yml` (which today contains no Lean step) plus a
  lean-smt SHA pin; (b) the #148 cast-paren fix in `lower_inv_operand`
  (`thermite-lower/src/lower.rs:1187`), clearing the known-divergence
  ledger; (c) a rotating-seed scheduled CI job for the `--generated`
  machinery riding `thermite-tv/src/gen.rs` (SplitMix64); (d) the README
  regime-split + editor-claim-qualifier paragraph.
- REQ-3: The stage-1 issue tree (the program plan §2's nine items: verdict
  plumbing, exporter hardening of `forge/src/lean_export.rs`, surface
  syntax, covenant engine, frozen battery, anti-Goodhart at L3,
  `forge goal --proof`/`forge fill` UX, relax routing, lemma-library
  mechanics) is opened with gate G1 as the milestone, in the listed
  dependency order (1–2 foundation, 3–7 parallelizable, 8 independent, 9
  trailing).
- REQ-4: The seven-verdict vocabulary (`Proved`, `Counterexample`,
  `RealWitness`, `CovenantRefuted`, `Stuck`, `KernelBudget`, `Timeout`) is a
  closed set wired end to end in stage-1 item 1, with the
  never-converts-silently property enforced by hermetic tests in the
  existing degrade anti-cheat pattern (`forge/src/degrade.rs`,
  `forge/src/tv_signal.rs` discriminator style).
- REQ-5: No stage's headline claim ships before its gate: G1 (end-to-end L3
  certificate on a merge-class example with covenant, axiom gate,
  re-elaboration mutation, burn receipt; all seven verdicts hermetically
  exercised; zero v1 corpus regressions; exporter re-inspection table), G2
  (audit checks [1′][4′][8][9] green in one `make audit` run gating the
  certificate `trust:` flip as a tested code path), G3 (`@bv` parse-gated on
  shadow-flag plumbing; reconstruction default-on behind a fragment-support
  check). README/docs change at gate time, not merge time.
- REQ-6: The Q1–Q10 register defaults are adopted as stated in the program
  plan §5, each carrying its decide-by milestone (Q1/Q3/Q4/Q6/Q7/Q8 by M1
  exit; Q5 by M2b; Q2/Q9 by G2; Q10 post-G2); a merge that contradicts a
  default must update the register in the issue thread first.
- REQ-7: The §6 metrics dashboard (cage coverage, repair-verb mix, burn
  economics, covenant catch rate, TV phase split, classifier-differential
  disagreements, unknown-on-admitted events, editor coverage) is
  instrumented from M1 so v2.1 fragment-widening decisions are
  evidence-driven.
- REQ-8: The governance deliverables exist by their owning milestones:
  `thermite2-semantics.md` (single normative semantics home),
  `goal.md` R-rule candidates (R-SIDE-1, R-COV-1, R-BV-1, R-VERDICT-1,
  R-GATE-1), `THERMITE.skill.md` v2 (the agent-facing loop: verdicts, routing
  reasons, restratify recipe, covenant authoring), and RFC thread hygiene
  (deltas as comments, changelog table at the top of the issue body, a
  pinned comment per gate completion).
- REQ-9: All program documents are re-baselined to `c46da3ac` (or later),
  with the 93d3cbc0→HEAD deltas that touch program surfaces named: the
  while-body Lean composition layer and exporter (v-a/v-b, #264/#265), and
  the doc-drift tripwire tooling (#258–#262) that audit check [4′] extends.
- REQ-10: Each stage gets its own `/design` pass before its first
  implementation issue opens: stage 1 (the forge tier + relax routing),
  stage 2 (the S₂ spine + classifier), stage 3 (`@bv` + reconstruction);
  this umbrella links them as they land. Child docs:
  `.design/m0-spikes.md` (M0, final), `.design/stage1-forge-tier.md`
  (stage 1, final), `.design/stage2-stratified-cage.md` (FINAL — G1
  re-pass 2026-06-19 resolved all four input dependencies, re-baselined to
  `904ee01c`, sized the REQ-0 surface-quantifier foundation increment;
  kickoff-ready), `.design/stage3-bv-reconstruction.md`
  (PROVISIONAL — pending G2-era telemetry and reconstruction
  assessment; re-run the design pass before kickoff). A provisional doc
  does NOT satisfy this REQ's "own design pass" condition — the re-pass
  that resolves its OPEN blocks does.

## Acceptance Criteria

- [ ] AC-1: Two issues titled for SPIKE-1 and SPIKE-2 exist and are open
  before any issue labeled stage-1 or stage-2 exists; each issue body
  contains the plan's acceptance text (SPIKE-1: "zero sorries", the
  ">40 lemmas" failure signal, the conventions note deliverable; SPIKE-2:
  "hit rate measured and reported, whatever it is"). (REQ-1)
- [x] AC-2: `.github/workflows/ci.yml` contains a job that runs `lake build`
  (or `lake env lean`) against `lean/lakefile.toml`, and the lean-smt
  dependency is pinned by SHA; the job is green on main before any M1 issue
  closes. (REQ-2a)
- [x] AC-3: `lower_inv_operand` in `thermite-lower/src/lower.rs` emits
  parenthesized casts (#148); the known-divergence ledger entry is removed
  and a regression test pins the fix. (REQ-2b)
- [x] AC-4: A scheduled (cron-triggered) CI workflow runs the `--generated`
  corpus with a rotating seed and fails on any divergence. (REQ-2c)
- [x] AC-5: README contains the regime-split paragraph qualifying the editor
  claim. (REQ-2d) — the editor claim is scoped to "editing logic, line
  navigation, and cursor math are proven correct for every input (L3)" with
  the syscall boundary as L1, and the ladder table names each regime.
- [ ] AC-6: Nine stage-1 issues exist, each referencing gate G1, with
  dependency order stated; items 1–2 are closed before any of 3–7 starts
  review. (REQ-3)
- [x] AC-7: A hermetic test suite exercises all seven verdicts and asserts
  the never-converts-silently property (no test path turns any non-`Proved`
  verdict into `Proved` or into another verdict without a hard failure).
  (REQ-4)
- [x] AC-8: G1 is encoded as a checklist in the stage-1 milestone: the
  merge-class L3 certificate exists in `conformance/` with covenant
  evidence, axiom gate, re-elaboration mutation score, and burn receipt
  fields populated; the v1 corpus (`conformance/*.th` + golden
  `*.cert.json`) passes unchanged. (REQ-5)
- [ ] AC-9: The G2 `trust:` flip (from `ref_encode(strat, UNPROVEN)` to the
  proven form) is a code path with its own test, gated on audit checks
  [1′][4′][8][9] all green in a single `make audit` run. (REQ-5)
- [ ] AC-10: A release build without shadow-flag plumbing rejects `@bv` at
  parse time (a build-flag test exists before the tag parses anywhere).
  (REQ-5)
- [ ] AC-11: This doc's Q-register table matches the program plan §5
  defaults; any merged change contradicting a default is preceded by a
  register-updating comment on GH issue #2. (REQ-6)
- [x] AC-12: From M1, `forge` emits the routing-reason and verdict telemetry
  fields the §6 dashboard needs (cage-vs-forge share by reason, verdict
  counts, TV phase split), and the audit prints them. (REQ-7)
- [x] AC-13: `thermite2-semantics.md` exists and module-header comments in
  `lean/Thermite/` and `thermite-lower/` point at it rather than restating
  conventions; `goal.md` contains the five R-rule candidates; the issue #2
  body carries a changelog table. (REQ-8)
- [x] AC-14: Every program document merged into `.design/` after this one
  states baseline `c46da3ac` or later and links this umbrella. (REQ-9) —
  `thermite2-semantics.md`, `.design/stage1-forge-tier.md`, and
  `.design/m0-spikes.md` each cite baseline `c46da3ac` (or later) and link
  `.design/thermite2-program.md`.
- [x] AC-15: `.design/` contains a stage-1 design doc before the first
  stage-1 implementation issue is worked, and likewise for stages 2 and 3.
  (REQ-10)

## Architecture

### What exists at HEAD that the program builds on

The v1 toolchain this program extends is shipped and audited in-tree:

- **The forge pipeline** — `forge/src/check.rs` (per-item certification,
  governed by `.design/forge/check.md`), `forge/src/manifest.rs`
  (certificates), `forge/src/audit.rs` (the numbered-check `make audit`),
  `forge/src/mutation.rs` + `forge/src/vacuity.rs` +
  `forge/src/degrade.rs` (the anti-Goodhart battery and the
  degrade-loudly discipline the seven-verdict work extends),
  `forge/src/goal_repl.rs` (the goal surface `forge goal --proof` grows
  from), and `forge/src/engine.rs` (the `--engine` routing surface the
  relax route plugs into).
- **The exporter** — `forge/src/lean_export.rs`. Stage-1 item 2 grows it
  from "export obligations for the demotion arc" into the forge's front
  door (per-item axiom gate as a certifying step). Materially: since the
  RFC baseline, #264 already extended it with the while-body recognizer +
  Inv/mu emission + the 5+2 loop obligations, so the hardening pass starts
  from a more capable exporter than RFC-1 §12 assumed.
- **The Lean spine** — `lean/Thermite/` (Ast, Denote, RefEncode,
  Soundness, Faithfulness, Exec, Stabilize + the `Pin*.lean` negative
  battery). Stage 2 adds the sibling tree `lean/Thermite/Strat/` per the
  metatheory sketch §10.1 and reuses the v1 arithmetic denotation at
  `QFree` atoms. Since the RFC baseline, the while-body composition layer
  (`whileBodyDenote`, `while_compose`, `loopDenote_exits_of_dec`, #264,
  plus the #265 critic pin on the `h_dec` statement shape) landed here —
  stage-2 work must baseline against this, not 93d3cbc0.
- **TV + the generator** — `thermite-tv/` with the SplitMix64 generator in
  `thermite-tv/src/gen.rs`. Three program consumers ride this one
  generator: the TV corpus, the covenant falsifier (stage-1 item 4), and
  the classifier differential battery (stage 2, M2b) — which is why the
  rotating-seed scheduled job is a pre-M1 debt item rather than a
  stage-2 nicety.
- **The lowering** — `thermite-lower/src/lower.rs`; carries the one known
  divergence (#148, `lower_inv_operand` at line 1187, cast parens), to be
  cleared before new lowering work begins.
- **Doc-drift tooling** — the tripwire gate landed post-baseline
  (#258–#262, governed by `.design/tooling/doc-drift-tripwire.md`). Audit
  check [4′] (drift rows for the three new mirrored Rust files) extends
  this existing mechanism instead of inventing one — a delta in the
  program's favor.
- **CI** — `.github/workflows/ci.yml` has no Lean job today; the Lean
  build is currently un-CI'd, which is why debt item (a) is a
  pre-M1 blocker: from M1 the Lean build is load-bearing per-certificate.

### Stage → surface map

| stage | Rust surfaces | Lean surfaces | gate |
|---|---|---|---|
| M0 spikes | none (SPIKE-2 prototype normalizer may live under `thermite-tv/` or a scratch crate; throwaway) | SPIKE-1 toy outside `lean/Thermite/` proper | both spike ACs |
| Stage 1 | `forge/src/` (verdicts in manifest/check/engine, `lean_export.rs` hardening, covenant engine, battery registry, `goal_repl.rs`), `thermite-syntax/` (prop fn, lemma, proof blocks, `?pN` holes, witness blocks, refinement sugar, `dec lex`/`dec wf`) | `r_relax_sound` + `rencode_sound` (small Mathlib island) | G1 |
| Stage 2 | `thermite-spec/` classifier (+ NNF/graph mirror), `thermite-lower/` quantifier emission, `thermite-tv/` strat reference encoder + two-phase TV + binder productions | `lean/Thermite/Strat/` (the §10.1 module plan, ≈5.5k loc) | G2 |
| Stage 3 | `@bv` lowering + the three locks; reconstruction default-on | none new | G3 |

### Baseline drift, 93d3cbc0 → c46da3ac (18 commits)

What changed that the program documents reference:

1. `lean/Thermite/` gained the while-body composition layer (v-a) and its
   pins, including a kernel-proven statement-shape divergence pin (#265).
2. `forge/src/lean_export.rs` gained the while-body exporter (v-b): loop
   recognizer, Inv/mu emission, 5+2 obligations, loop battery.
3. The doc-drift tripwire shipped as tooling with make targets and a
   route-table oracle (#258–#262), including three hardening fixes for
   wrong-shaped/empty route tables.
4. `.design/verified/proof-backends.md` went through SHIPPED-status
   re-audits; the interactive replay theorem-needle became exact-match
   (#268).

None of these contradict RFC-1; (1) and (2) advance the exporter surface
stage 1 hardens, and (3) supplies the mechanism audit check [4′] assumed.
Program documents merged after this one cite `c46da3ac` or later.

### Adopted defaults (the Q-register, restated for the tree)

| # | default adopted | decide by |
|---|---|---|
| Q1 | Per-project lemma namespace; dedup by statement-hash with citation rewrite; burned lemmas surface in `forge review` | M1 exit |
| Q2 | Tower budget depth 4 / 40 definitions | G2 |
| Q3 | `falsify` 50,000 fixed-seed; witnesses may be generator-synthesized but ≥1 author-stated | M1 exit |
| Q4 | KernelBudget 30s elaboration, per-clause | M1 exit |
| Q5 | Corpus-mimicking + uniform-random generator arms; measure kill/disagreement per arm | M2b |
| Q6 | One `proof for f` block may discharge several `ens#k` with shared local lemmas; no cross-function sharing except via `lemma` | M1 exit |
| Q7 | `dec wf` accessibility proofs cached by (relation, carrier) hash | M1 exit |
| Q8 | RealWitness integrality search: rounding + radius-2 box, 1s budget | M1 exit |
| Q9 | `forge review` gets a "semantic forks and definition towers" section | G3 / G2 |
| Q10 | Seq-sort quantifiers stay out until S₂.0 telemetry exists | post-G2 |

## Open Questions

### Q-TRACK: Where do the program's issues live — GitHub or crosslink? (resolved)

**Decision:** split by audience. Crosslink owns detailed intermediate
tracking of agent-based milestones — increments, fixes, critic pins, the
day-to-day issue discipline the repo already runs (#264/#265/#268 style).
GitHub owns the larger, PR-level items: anything with human input or
gating — the spikes' open/close announcements, the stage gates G1–G3 and
their pinned completion comments on issue #2, and milestone-scoped
umbrella issues. Concretely for the ACs: AC-1 (spikes) and AC-6 (stage-1
tree) are satisfied by GH issues for the gate-visible scaffolding, with
each GH issue's implementation increments tracked as crosslink issues
referencing it.

## Out of Scope

- The non-goals registry (program plan §8) in full: sequence-sort/nested
  quantifiers, unbounded-int quantifiers in the cage, user-extensible simp
  sets or tactic plugins, extensional equality and sized types, a 2-D
  ladder in the product, `@bv` as a default or file-level mode, verified
  extraction/erasure.
- Re-specifying stage-2 metatheory — the sketch in issue #2 owns it; the
  stage-2 `/design` pass (REQ-10) adapts it to the tree, this doc only
  indexes it.
- Any implementation in this pass: this document changes `.design/` only.
- Fragment widenings past S₂.0 (versioned, evidence-driven, post-G2).

---

*Program umbrella · companion to GH issue #2 (RFC-1 + metatheory sketch +
program plan + Appendix A) · baseline `dollspace-gay/Thermite @ c46da3ac`.*
