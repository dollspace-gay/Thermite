---
title: "M0 — the two de-risking spikes (SPIKE-1 SubstKit toy, SPIKE-2 normalizer probe)"
tags: ["design-doc"]
sources: []
contributors: ["rApq"]
created: 2026-06-15
updated: 2026-06-15
---


# Feature: M0 — the two de-risking spikes (SPIKE-1 SubstKit toy, SPIKE-2 normalizer probe)

## Summary

The two week-one spikes from the RFC-1 program plan §1, specified to
opening-an-issue precision. SPIKE-1 (the SubstKit toy) de-risks the
stage-2 binder-metatheory grind — risk row 1 of the metatheory sketch,
fallback F-A — by proving the two load-bearing de Bruijn lemmas end to end
on a 3-constructor toy formula language before `Strat/SubstKit.lean` is
scheduled. SPIKE-2 (the normalizer probe) de-risks the stage-2 TV-query
design — risk row 3, fallback F-C — by measuring the syntactic-equality
hit rate of a prototype normalizer over the conformance corpus's
combinator contracts in raw-quantifier form. Both gate M0: no stage-1 or
stage-2 implementation issue opens until both acceptance criteria are met.
Umbrella: `.design/thermite2-program.md` (REQ-1). Baseline:
`dollspace-gay/Thermite @ c46da3ac` or later.

## Requirements

### SPIKE-1 — the SubstKit toy

- REQ-1: A toy formula language lives at `lean/Thermite/Spike/SubstKit.lean`
  inside the existing lake package (`lean/lakefile.toml`), namespace
  `Thermite.Spike`, core-Lean-only (no Mathlib import — the same hot-path
  discipline as `lean/Thermite/Denote.lean`). Exactly three `Frm`
  constructors (one atom, one connective, one binder) over a de Bruijn
  term language with `lift` and `subst`, denoting into `Bool`. The carrier
  is *parametric*, not fixed: a `CarrierAssign`-lite with one opaque sort
  carrying a **hand-rolled finiteness witness** — an enumeration `List`
  plus a completeness proof (`∀ x, x ∈ enum`) and `DecidableEq` (core
  Lean, via `deriving`), **not** Mathlib's `Fintype`. This is a
  deliberate correction to the metatheory sketch's CarrierAssign (which
  literally writes `Fintype`, a Mathlib type, contradicting the sketch's
  own §4 core-Lean-only-hot-path claim): SPIKE-1 must determine whether
  finite-carrier `Bool`-denotation can stay core-Lean-only with a
  hand-rolled witness, because "fights universe/`Decidable` plumbing" is
  one of the two failure signals the spike exists to probe, and the
  Mathlib-vs-core verdict is itself a stage-2 `Strat/Carrier.lean` input.
  (The lakefile pulls Mathlib transitively via the `smt` require, so a
  `Fintype` import would *compile* — which is exactly why the discipline
  must be enforced by intent here, not by the build failing.)
- REQ-2: `sdenote_push_lift` and `sdenote_subst` (the metatheory sketch
  §4's statement shapes, specialized to the toy) are proven end to end:
  zero `sorry`, axioms of both lemmas ⊆ {propext, Classical.choice,
  Quot.sound} under `#print axioms`.
- REQ-3: A micro-pin `lean/Thermite/Spike/PinBrokenLift.lean` refutes a
  deliberately broken `lift` (off-by-one cutoff shift) in the repo's
  established `Pin*.lean` style: the wrong lemma instance is disproven on
  a concrete small carrier via `decide`.
- REQ-4: The conventions note — the spike's real deliverable — lands at
  `.design/strat/substkit-conventions.md` (one page): lift direction,
  environment push order, binder-traversal convention, the exact lemma
  statement shapes as proven (verbatim-inheritable by
  `lean/Thermite/Strat/Syntax.lean`), the final lemma count, the
  **carrier verdict** (did the hand-rolled finiteness witness keep the
  denotation core-Lean-only, or was Mathlib's `Fintype` needed? — a
  direct input to stage-2 `Strat/Carrier.lean` and a resolution of the
  metatheory sketch's §2/§4 tension), and an
  explicit failure-signal verdict: if the toy needed more than 40 lemmas
  or the instance plumbing fought back, the note says so and a fallback
  F-A review (locally nameless / single-prefix S₂⁻) is opened before the
  real SubstKit is scheduled.

### SPIKE-2 — the normalizer probe

- REQ-5: The fixture set is built per **combinator shape**, not per
  instance, to keep the hit-rate denominator meaningful. For each of the
  (≤8) registry combinator shapes that appears, write two raw-quantifier
  S₂ expansion templates: a production-style spelling (following the
  shape `thermite-lower/src/lower.rs` emits for that combinator today)
  and a reference-style spelling (following `lean/Thermite/RefEncode.lean`).
  Apply each shape's templates to (a) every combinator-bearing clause in
  `conformance/*.th` — which the plan analysis confirms is **only
  `binary_search.th`, 4 clauses** (`req sorted`, the `ens` None-arm
  `forall_in`, `inv forall_below`, `inv forall_from`) — and (b) a
  generator-drawn sample of instances of those same shapes from
  `thermite-tv/src/gen.rs`'s `gen_combinator` (≥ 5 per shape), so the
  denominator is ≥ ~30 rather than 4. Fixture home:
  `thermite-tv/tests/fixtures/strat_probe/`, one file per instance pair,
  README naming the source: a scheme-validated address for `inv`
  clauses (`binary_search.loop#1.inv#2`/`inv#3`) and an informal
  designation for `req`/`ens` clauses (`binary_search.req`,
  `binary_search.ens.None`), since `thermite-syntax/src/address.rs`
  addresses loop/inv/dec but not req/ens.
- REQ-6: A prototype normalizer at `thermite-tv/src/normalize.rs` —
  experimental, exported but not referenced by any TV pipeline code path —
  implementing the four passes from metatheory §8.2 layer 1: NNF, prenex,
  canonical bound-name/de Bruijn form, atom ordering. Unit tests cover
  each pass on the fixtures (no soundness lemmas in the spike — those are
  stage-2 work; the spike wants the *number*).
- REQ-7: The hit rate — the fraction of fixture pairs whose normalized
  forms are syntactically equal — is computed by a test/bin target,
  reported as a number in the SPIKE-2 issue and recorded in the fixtures
  README, **broken out two ways**: the corpus-only rate (n=4, flagged as
  small-n and not threshold-bearing on its own) and the
  corpus+generated rate (n ≥ ~30, the threshold-bearing number). The
  decision rule from the program plan is then applied to the
  corpus+generated rate and recorded: hit rate ≥ 90% → the stage-2
  semantic TV phase ships as a thin fallback; below → a dedicated
  quantified-equivalence-query design issue opens before stage 2
  commits. The per-shape breakdown is also reported so a single
  pathological shape is visible rather than averaged away.

### The gate

- REQ-8: M0 closes only when AC-1 through AC-7 hold. Per the umbrella's
  Q-TRACK split: each spike gets a GH issue on `dollspace-gay/Thermite`
  (gate-visible), with implementation increments tracked as crosslink
  issues referencing it. No stage-1 or stage-2 implementation issue opens
  before both GH spike issues close with results.

## Acceptance Criteria

- [ ] AC-1: `lake build` (from `lean/`) is green with the two Spike files
  present; `grep -r sorry lean/Thermite/Spike/` returns nothing;
  `import Mathlib` (and any `Mathlib.*` import) is absent from the Spike
  files (the core-Lean-only discipline); `#print axioms` on
  `sdenote_push_lift` and `sdenote_subst` shows only the gated three or
  fewer, run as a spike-local probe (not via `make audit`, whose theorem
  list is fixed and must not be perturbed by Spike files). (REQ-1, REQ-2)
- [ ] AC-2: `PinBrokenLift.lean` contains a `lift` variant differing only
  in the cutoff arithmetic, and a `decide`-discharged theorem showing the
  push/lift lemma *fails* for it on a concrete carrier. (REQ-3)
- [ ] AC-3: `.design/strat/substkit-conventions.md` exists with all six
  required sections (lift direction, push order, traversal, statement
  shapes, lemma count, carrier verdict) and a one-line failure-signal
  verdict. (REQ-4)
- [ ] AC-4: If the verdict reports >40 lemmas or instance-plumbing
  fights, an F-A review issue exists before any `Strat/SubstKit.lean`
  issue is opened; otherwise the note states the count and that no
  trigger fired. (REQ-4)
- [ ] AC-5: Each appearing combinator shape has production-style and
  reference-style expansion templates; they are applied to all 4
  `binary_search.th` clauses and to ≥5 generator-drawn instances per
  shape, yielding ≥ ~30 fixture pairs under
  `thermite-tv/tests/fixtures/strat_probe/`; the README lists each
  source (scheme-validated address for `inv`, informal for `req`/`ens`);
  `cargo test -p thermite-tv` passes the normalizer unit tests.
  (REQ-5, REQ-6)
- [ ] AC-6: No module in the TV pipeline path imports
  `thermite_tv::normalize` (mechanically: `grep -rn "normalize" thermite-tv/src/ forge/src/` shows
  no non-test, non-`normalize.rs` consumer). (REQ-6)
- [ ] AC-7: Both hit-rate numbers (corpus-only n=4, flagged small-n; and
  corpus+generated n ≥ ~30, threshold-bearing) plus the per-shape
  breakdown appear in the SPIKE-2 GH issue and the fixtures README, with
  the decision rule's branch recorded against the corpus+generated rate;
  if < 90%, a quantified-equivalence design issue exists. (REQ-7)
- [ ] AC-8: Two GH spike issues exist (open before any stage-labeled
  issue) quoting the acceptance text from the program plan §1 (GH issue
  #2, third comment — quoted via `gh issue view 2`, since the program
  plan lives as an issue comment, not a repo file), and both are closed
  with results before any stage-1/stage-2 implementation issue opens.
  (REQ-8)

## Architecture

**SPIKE-1.** Lives inside the audited lake package
(`lean/lakefile.toml`) as a `Thermite.Spike` namespace — zero new build
plumbing, and the de Bruijn conventions get proven against the exact
toolchain pin (`lean/lean-toolchain`) that `Strat/` will use. The
`make audit` axiom probe targets a fixed theorem list, so Spike files
don't perturb it; the spike's own axiom discipline is checked by AC-1
directly. Style models in-tree: `lean/Thermite/Denote.lean` (Bool-valued
total denotation), the `lean/Thermite/Pin*.lean` battery (the
refute-a-plausibly-wrong-neighbor shape REQ-3 copies). The whole
`Spike/` directory is deletable scaffolding: it is removed in the same
change that lands `lean/Thermite/Strat/Syntax.lean` inheriting its
conventions, with the conventions note (`.design/strat/`) as the
surviving artifact — the note also seeds the `.design/strat/` area where
stage-2 house docs will live.

**SPIKE-2.** Lives in `thermite-tv` because its production successor is
stage-2's two-phase TV (metatheory §8.2): the prototype normalizer
evolves in place rather than being rewritten. `thermite-tv/src/gen.rs`
(the SplitMix64 generator) and the existing TV pipeline are untouched —
`normalize.rs` is a leaf module consumed only by its tests and the
hit-rate target until stage 2 wires it in behind `nnf_sound`/
`prenex_sound` lemmas. The expansion templates are hand-written because
neither emitter exists yet for stratified forms: the production-style
spelling mimics `thermite-lower/src/lower.rs`'s current combinator
emission conventions, the reference-style mimics
`lean/Thermite/RefEncode.lean`'s — the two real columns the stage-2 TV
will eventually compare. The hand-work is bounded *per combinator
shape* (≤8 shapes), then instantiated across the 4 corpus clauses plus
generator-drawn instances (read out of the existing `gen_combinator` —
no new generator productions, those are stage-2 binder work) so the
threshold-bearing denominator is meaningful rather than n=4. This is the
plan analysis's most material correction: a 90% bar over 4 clauses is
4/4, which the corpus alone cannot support. This mimicry is the probe's known
approximation, and it biases the measured hit rate *downward* if
anything (real stage-2 emitters can be nudged toward convergence,
fallback F-C step 1), so a high measured rate is trustworthy evidence
and a low one triggers the design issue — the asymmetry is safe.

**What the spikes deliberately do not do:** no soundness lemmas for the
normalizer passes, no `Strat/` modules, no classifier, no changes to
`forge/`, no generator binder productions. Each of those belongs to a
stage with its own design pass.

## Open Questions

None. The two genuine candidates — fixture format and emission-spelling
derivation — are resolved by adopted defaults above (plain fixture files
under `thermite-tv/tests/fixtures/strat_probe/`; spellings derived from
the v1 emitters' current shapes), both cheap to revisit at stage-2
design time.

## Out of Scope

- The real `Strat/SubstKit.lean` (~25-lemma kit) and all
  `lean/Thermite/Strat/` modules — stage 2.
- Normalizer soundness lemmas (`nnf_sound`, `prenex_sound`) and wiring
  the normalizer into TV — stage 2.
- The classifier, the sort graph, restratification — stage 2.
- Generator binder productions in `thermite-tv/src/gen.rs` — M2b.
- Any `forge/` or `thermite-syntax/` change — stage 1.

---

*M0 spike spec · child of `.design/thermite2-program.md` (REQ-1) ·
sources: RFC-1 program plan §1, metatheory sketch §4/§8.2/§11 ·
baseline `dollspace-gay/Thermite @ c46da3ac`.*
