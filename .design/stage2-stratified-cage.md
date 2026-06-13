# Feature: Stage 2 — the stratified cage + the Strat spine extension (PROVISIONAL)

> **STATUS: PROVISIONAL — do not kickoff from this document.**
> This is an interim reasoning cache, written before its input
> dependencies exist. It carries unresolved `<!-- OPEN -->`
> blocks keyed to those dependencies, so validation flags it as
> not-kickoff-ready. When all inputs below are available, re-run the
> design pass (`/design --continue stage2-stratified-cage`) to resolve
> the OPEN blocks against real results, re-validate, and only then
> `crosslink kickoff`.

| input dependency | what it decides here | produced by |
|---|---|---|
| SPIKE-1 conventions note (`.design/strat/substkit-conventions.md`) | Q-KIT: binder representation + the SubstKit lemma statements `Strat/Syntax.lean` inherits verbatim | `.design/m0-spikes.md` REQ-4 |
| SPIKE-2 hit-rate number | Q-TV2: whether the semantic TV phase ships as a thin fallback or gets its own design issue first | `.design/m0-spikes.md` REQ-7 |
| Gate G1 (stage 1 complete) | The seven-verdict enum, schema-v2 certificates, exporter front door, and covenant machinery this stage consumes | `.design/stage1-forge-tier.md` REQ-10 |
| Stage-1 routing telemetry | Whether (R2)'s narrow index grammar needs S₂.1 widening pressure noted before build | program plan §6 dashboard |

## Summary

Stage 2 of the RFC-1 program: the admission classifier (sort-graph
construction, cycle reporting, the restratify rewrite + its implication
side obligation) shipped against a stratified-FOL extension of the Lean
spine — `lean/Thermite/Strat/` per the metatheory sketch §10 — with the
v1 combinators demoted to derived lemmas. The kernel deliverables are
T1-S (stratified encoder soundness), T2-S (conditional faithfulness),
T3-C (classifier coincidence), T4-R (restratification conservativity);
T5-X (real-relaxation) already lands in stage 1 (REQ-8 there), so this
doc excludes it. Gate G2 is the certificate `trust:` flip from
`ref_encode(strat, UNPROVEN)` to the proven form, gated on audit checks
[1′][4′][8][9] green in one run. The spec of record for the mathematics
is the metatheory sketch in GH issue #2; this doc caches its adaptation
to the tree and will be re-grounded at re-pass time. Umbrella:
`.design/thermite2-program.md` (REQ-10).

## Requirements

Increment order follows metatheory §10.2; each lands green in the repo's
issue discipline.

- REQ-1 (**syntax + denote + the load-bearing pin**): `Strat/Syntax.lean`
  (Frm/Tm/Atom, de Bruijn, lift/subst — inheriting the SPIKE-1
  conventions verbatim), `Strat/Carrier.lean` (`CarrierAssign` with
  `Fintype`/`DecidableEq` fields), `Strat/Denote.lean` (`sdenote`
  Bool-valued via `decide` over `Fintype` binders, deferring to the v1
  denotation at `QFree` atoms), plus `PinFiniteEscape` pinning why (R1)
  finite carriers are load-bearing — before anything consumes the
  semantics. Core-Lean-only on this path.
- REQ-2 (**SubstKit**): the ~25-lemma binder kit (`sdenote_push_lift`,
  `sdenote_subst`, `sencode_fresh_ok`, companions), isolated in
  `Strat/SubstKit.lean` with its own micro-pins. Scheduled second, not
  last — the program's schedule variance lives here. The lemma list is
  fixed before coding starts, from the SPIKE-1 note.
- REQ-3 (**classifier, kernel half**): `Strat/Nnf.lean` (NNF + prenex
  with `nnf_sound`/`prenex_sound`), `Strat/Graph.lean` (`sortGraph` E1∪E2,
  `acyclic` with `acyclic_iff_no_cycle`), `Strat/Fragment.lean`
  (`idxGrammar` per (R2), `finCarrier` per (R1), `admitted`, the
  declarative `Frag`, and T3-C `classifier_correct`). Fragment versioned
  as S₂.0 — widenings are new grammar + citations + pins, never silent.
- REQ-4 (**classifier, ops half — M2b, ships before the encoder**): the
  Rust classifier in `thermite-spec` (mirroring NNF + graph + grammar
  checks beside the existing validator), rejection reasons from the
  frozen vocabulary (`infinite-carrier`, `seq-quantifier`, the named
  cycle), and the **differential battery**: the SplitMix64 generator
  (`thermite-tv/src/gen.rs`) extended with well-sorted binder
  productions (Q5 default: corpus-mimicking + uniform-random arms);
  every generated formula run through both the Rust classifier and
  `lake env lean --run` on `admitted`; any disagreement is a hard CI
  failure (audit check [8]). The `unknown`-on-admitted tripwire logs and
  escalates as classifier-suspect, never silently retries.
- REQ-5 (**encoder + T1-S**): `Strat/RefEncode.lean` (`sencode`,
  trigger-free MBQI surface, fresh-name discipline),
  `Strat/TokDenote.lean`, `Strat/Soundness.lean` (T1-S
  `strat_ref_sound` + `strat_ref_wf`), with `PinStratCapture` and
  `PinStratFlip` landing in the same increment.
- REQ-6 (**combinator demotion**): `Strat/CombDeriv.lean` — the eight
  `comb_deriv_*` lemmas proving each v1 combinator's denotation equals
  its raw-quantifier expansion (closing the v1 embedding), plus
  `PinCombDeriv` refuting an off-by-one expansion. The combinator
  registry (`thermite-spec/src/combinators.rs`) is untouched as surface
  syntax; SPIKE-2's hand-written expansions are replaced by these
  mechanized ones wherever the probe fixtures survive as tests.
- REQ-7 (**restratify**): `Strat/Restratify.lean` (T4-R
  `restrat_conservative` + `restrat_admits` + `restrat_complete` +
  `side_admitted`), `forge edit --restratify` wiring (the rewrite emits
  the `Side(φ', φ)` obligation in-cage; certification of φ' without a
  discharged Side never counts for φ — R-SIDE-1), and
  `PinRestratDropSide` exhibiting the mis-certification that dropping
  Side would permit.
- REQ-8 (**faithfulness + two-phase TV + the flip**):
  `Strat/Faithfulness.lean` (`SFnTvWitness` with explicit req-frame
  conditioning, T2-S `strat_lowering_faithful`); production quantifier
  emission in `thermite-lower`; the stratified reference encoder in
  `thermite-tv`; two-phase TV per metatheory §8.2 — syntactic phase
  (the SPIKE-2 normalizer, now carrying `nnf_sound`/`prenex_sound`),
  semantic phase per Q-TV2's resolution, honest `Timeout` fallback
  withholding the certificate. During the rollout window stratified
  clauses carry `trust: solver(z3) + ref_encode(strat, UNPROVEN — stage
  2 in progress)`; the flip to the proven form is a one-line change
  gated on G2 and is itself a tested code path.
- REQ-9 (**audit integration — the G2 gate**): `make audit` grows
  [1′] (axiom probe extended to `strat_ref_sound`,
  `strat_lowering_faithful`, `classifier_correct`,
  `restrat_conservative`; allowed axioms unchanged), [4′] (doc-drift
  rows for the three new mirrored Rust files via the shipped tripwire
  gate), [8] (the differential battery, fixed seed + the rotating-seed
  scheduled job), [9] (stratified TV sweep reporting the
  syntactic/semantic/timeout split). G2 = all four green in a single
  run, gating the trust flip.
- REQ-10 (**the pin battery, complete**): all eight stage-2 pins from
  metatheory §9 exist (`PinStratCapture`, `PinStratFlip`,
  `PinStratSelfLoop`, `PinNNFPolarity`, `PinRestratDropSide`,
  `PinRelaxRefute` — landed with stage 1's relax work if not already —
  `PinFiniteEscape`, `PinCombDeriv`), in the repo's established
  `Pin*.lean` style.

## Acceptance Criteria

- [ ] AC-1: `lake build` green with `Strat/Syntax,Carrier,Denote` and
  `PinFiniteEscape`; zero `sorry` under `lean/Thermite/Strat/`; no
  Mathlib import on the Denote path. (REQ-1)
- [ ] AC-2: `Strat/SubstKit.lean` proves the kit with lemma statements
  matching the SPIKE-1 conventions note (a comment cites the note's
  hash); micro-pin refutes a broken `lift`. (REQ-2)
- [ ] AC-3: `classifier_correct : ∀ φ, admitted φ = true ↔ Frag φ` is
  axiom-clean; the four §3.2 worked micro-examples (`a[a[i]]` self-loop,
  cast cycle, kv alternation cycle, sortedness) are `decide`-checked
  test theorems with the expected admit/reject outcomes. (REQ-3)
- [ ] AC-4: The Rust classifier returns the same verdict as Lean
  `admitted` on N generated formulas in CI with zero disagreements
  (check [8]); a rejection names its reason from the frozen vocabulary;
  `unknown`-on-admitted increments a counted, logged tripwire. (REQ-4)
- [ ] AC-5: T1-S and `strat_ref_wf` proven; `PinStratCapture` and
  `PinStratFlip` refute the broken-encoder neighbors at small carriers.
  (REQ-5)
- [ ] AC-6: All eight `comb_deriv_*` lemmas proven; the v1 conformance
  corpus certifies unchanged with combinators routed through the
  derived-lemma path. (REQ-6)
- [ ] AC-7: `forge edit --restratify` performs the kv-example rewrite
  end to end, emits and discharges `Side` in-cage, and a test proves
  certification is withheld when Side is undischarged;
  `PinRestratDropSide` exists. (REQ-7)
- [ ] AC-8: Two-phase TV runs over the stratified corpus + generated
  clauses reporting the phase split (check [9]); the `trust:` string for
  stratified clauses reads the UNPROVEN form before G2 and the proven
  form after; the flip is exercised by a test that toggles the gate.
  (REQ-8, REQ-9)
- [ ] AC-9: One `make audit` run shows [1′][4′][8][9] all green; the
  trust flip is mechanically blocked while any of the four is red.
  (REQ-9)
- [ ] AC-10: All eight stage-2 pins present and green; each is cited
  from the theorem it guards in the correspondence/battery doc. (REQ-10)

## Architecture

The mathematics, module boundaries, and loc estimates are the metatheory
sketch's (§10.1: ~5.5k loc Lean across 15 `Strat/` modules; ~4–6k loc
Rust). What this doc adds is tree placement, recorded now and
re-verified at re-pass time:

- **Lean**: `lean/Thermite/Strat/` as a sibling namespace to the v1
  spine; `QFree` atoms defer to the existing `Denote.lean` machinery, so
  the v1 arithmetic/cast/byte-view layers are consumed, not re-proven.
  The while-composition layer that landed post-RFC-baseline
  (`whileBodyDenote`/`while_compose`, #264/#265) is untouched by Strat —
  loop obligations stay v1-shaped in stage 2.
- **Rust classifier**: lives in `thermite-spec` beside the existing
  registry-free validator; the parser (`thermite-syntax`) already parses
  raw `forall`/`exists` only if stage-1 REQ-3 added them — *correction
  recorded for re-pass*: raw quantifier parsing is NOT in stage 1's
  surface-syntax list (it added forge constructs only), so stage 2's
  Rust work includes the quantifier surface grammar
  (`parse_expr_bp`-level binder productions) before the classifier can
  see formulas. This is the one work item the program plan's stage
  split leaves implicit; the re-pass should size it.
- **TV**: the SPIKE-2 normalizer (`thermite-tv/src/normalize.rs`)
  graduates from experimental to load-bearing, gaining the
  `nnf_sound`/`prenex_sound` lemma citations; the stratified reference
  encoder mirrors `Strat/RefEncode.lean` under a new correspondence-doc
  table and doc-drift route (check [4′]).
- **Generator**: binder productions extend `gen.rs`'s `gen_bool`
  dispatch; the differential battery and the covenant falsifier share
  the productions (the triple-use the rotating-seed CI job was sized
  for).
- **CI**: check [8] needs `lake env lean --run` in CI — the pre-M1 Lean
  CI job (umbrella REQ-2a) is a hard prerequisite, already sequenced.

Fallback posture if increments stall, from Appendix A: F-A (locally
nameless → single-prefix S₂⁻ → macro-combinators), F-B (ship `admitted`
as differential-tested oracle without the declarative theorem; `trust:`
reads `oracle(executable, differential-tested)`), F-C (emission
convergence → structural TV → scope retreat), F-D (finite-bound
assertion mode → fragment retreat → solver portfolio). Every retreat
preserves the verdict/covenant/ladder architecture.

## Open Questions

*(Deliberately unresolved — each is keyed to an input dependency from
the table above. Resolve at the re-pass, not before.)*

<!-- OPEN: Q-KIT -->
### Q-KIT: Binder representation — confirm plain de Bruijn, or take fallback F-A?

The metatheory commits to plain de Bruijn with a hand-rolled ~25-lemma
kit, and SPIKE-1 exists to validate that before REQ-2 is
scheduled. If the spike's conventions note reports the failure signal
(>40 lemmas, or `Fintype`/`DecidableEq`/universe plumbing fights), the
re-pass must choose among F-A's ladder (locally nameless; single-prefix
S₂⁻ with mandatory restratification; macro-combinators) and re-derive
REQ-2/REQ-5's statements accordingly.
**To resolve**: re-run the design pass with
`.design/strat/substkit-conventions.md` in hand.
<!-- /OPEN -->

<!-- OPEN: Q-TV2 -->
### Q-TV2: The semantic TV phase — thin fallback or its own design issue?

REQ-8's two-phase TV assumes the SPIKE-2 hit rate decides this: ≥90%
syntactic coverage means the quantified-equivalence Z3 query (the
negation-unfriendly biimplication) ships as a rarely-hit thin fallback
with finite-bound assertions; below that, the program plan requires a
dedicated design issue for the semantic query (or fallback F-C's
emission-convergence move) before stage 2 commits at all.
**To resolve**: re-run the design pass with the SPIKE-2 number and its
recorded decision-rule branch.
<!-- /OPEN -->

## Out of Scope

- T5-X / `Relax.lean` — landed in stage 1 (REQ-8 there); only the [1′]
  probe rows recur here.
- Sequence-sort quantifiers, nested sequences, unbounded-int binders —
  fragment v2.1+ / non-goals; forge-routed with named reasons.
- `@bv`, reconstruction — stage 3.
- Any (R2) widening past S₂.0 — telemetry-driven, post-G2, its own RFC
  delta.

---

*Stage-2 spec (PROVISIONAL — reasoning cache; re-run /design before
kickoff) · child of `.design/thermite2-program.md` (REQ-10) · spec of
record: the stage-2 metatheory sketch, GH issue #2 · gate: G2 ·
baseline `dollspace-gay/Thermite @ c46da3ac` (re-ground at re-pass).*
