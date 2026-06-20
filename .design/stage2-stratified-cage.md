# Feature: Stage 2 — the stratified cage + the Strat spine extension (RE-PASS COMPLETE · kickoff-ready)

> **STATUS: re-pass complete (G1 re-pass, 2026-06-19) — all four input
> dependencies resolved; kickoff-ready.**
> The two M0-spike-keyed `<!-- OPEN -->` blocks (Q-KIT, Q-TV2) resolved at
> the M0 re-pass (2026-06-13). The two remaining *non-spike* dependencies —
> Gate G1 (stage 1 complete) and stage-1 routing telemetry — both landed on
> main and are resolved by this re-pass (see the input table). The
> architecture and the (R2) index grammar are re-grounded against G1
> reality, the baseline is advanced to `904ee01c`, and the confirmed
> raw-quantifier surface-grammar gap is sized as the new foundation
> increment **REQ-0**. The Stage-2 issue tree may now open targeting gate
> G2 (REQ-0 → REQ-1 → … in the order below); proceed to `crosslink
> kickoff`.

| input dependency | what it decides here | status |
|---|---|---|
| SPIKE-1 conventions note (`.design/strat/substkit-conventions.md`, REQ-4) | Q-KIT: binder representation + the SubstKit lemma statements `Strat/Syntax.lean` inherits verbatim | **✓ RESOLVED** — plain de Bruijn confirmed; 11-lemma toy (≤ 40); no Mathlib/`Fintype` (hand-rolled finiteness witness); no fallback F-A. |
| SPIKE-2 hit-rate number (`.design/m0-spikes.md` REQ-7) | Q-TV2: whether the semantic TV phase ships as a thin fallback or gets its own design issue first | **✓ RESOLVED** — 40/40 = 100% (corpus+generated, n = 40, ≥ 90% bar) → semantic TV ships as thin fallback (F-C step 1). |
| Gate G1 (stage 1 complete, `.design/stage1-forge-tier.md` REQ-10) | The seven-verdict enum, schema-v2 certificates, exporter front door, and covenant machinery this stage consumes | **✓ RESOLVED** (G1 reached 2026-06-18, main@`904ee01c`) — all consumed machinery shipped: the seven-verdict `CertVerdict` (`RealWitness`/`CovenantRefuted`/`KernelBudget`, `forge/src/verdict.rs`+`check.rs`), schema-v2 per-clause `engine`/`trust` attribution (`forge/src/manifest.rs:253`), the exporter axiom-gate front door (`certify_lean_axioms`, `forge/src/lean_export.rs`), and the covenant engine (`forge/src/covenant_engine.rs`,`covenant_eval.rs`). |
| Stage-1 routing telemetry (program plan §6 dashboard) | Whether (R2)'s narrow index grammar needs S₂.1 widening pressure noted before build | **✓ RESOLVED** — `forge audit --metrics` ships (`forge/src/metrics.rs`, `RoutingReason::{InCage,Relaxable,Lemma}` derived from the per-clause `engine`). No S₂.1 widening pressure is observable pre-build (the cage's stratified routing does not exist until this stage), so the (R2) **narrow index grammar holds for S₂.0**; any widening stays post-G2/telemetry-driven (Out of Scope, below). |

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
to the tree, re-grounded against G1 reality at the re-pass (2026-06-19).
Umbrella: `.design/thermite2-program.md` (REQ-10).

## Requirements

Increment order follows metatheory §10.2; each lands green in the repo's
issue discipline. REQ-0 is sequenced **first** as a hard prerequisite —
the re-pass (2026-06-19) confirmed stage 1 added forge constructs only,
not raw quantifiers, so the surface grammar must exist before either the
Lean syntax (REQ-1) or the Rust classifier (REQ-4) can be built or tested.

- REQ-0 (**surface quantifiers — the foundation increment**): raw
  `forall`/`exists` binder productions in `thermite-syntax`. Grounded by
  the re-pass: `thermite-syntax` parses no raw quantifiers today —
  `forall_in`/`forall_below`/`forall_from`/`sorted` are registry-free
  combinator identifiers lowered as ordinary `Expr::Call` nodes
  (`thermite-syntax/src/parser.rs:1428` "no special parse; the plain
  path"). REQ-0 adds the binder grammar at `parse_expr_bp` level
  (`forall (x : S) in <dom>. φ` / `exists …` over a named sorted carrier,
  the index-grammar surface (R2) admits), the matching `Expr` AST node(s),
  and parser pins for the binder/scope corner cases. The combinator
  registry (`thermite-spec/src/combinators.rs`) is untouched as surface
  syntax. This increment **blocks REQ-1 and REQ-4** — it is the one work
  item the program plan's stage split left implicit, now made explicit per
  the re-pass.
- REQ-1 (**syntax + denote + the load-bearing pin**): `Strat/Syntax.lean`
  (Frm/Tm/Atom, de Bruijn, lift/subst — inheriting the SPIKE-1
  conventions verbatim), `Strat/Carrier.lean` (a `CarrierAssign` bundling
  each opaque sort with a **hand-rolled finiteness witness** — `enum :
  List C` + `complete : ∀ x, x ∈ enum` + `DecidableEq` carried as data,
  per the SPIKE-1 carrier verdict; **no Mathlib `Fintype`**),
  `Strat/Denote.lean` (`sdenote` Bool-valued via `List.all` folds over the
  enumeration, with `sdenote_all_iff` upgrading the fold to a genuine `∀`
  through the completeness witness, deferring to the v1 denotation at
  `QFree` atoms), plus `PinFiniteEscape` pinning why (R1) finite carriers
  are load-bearing — before anything consumes the semantics. Core-Lean-only
  on this path (SPIKE-1 confirmed the hand-rolled witness keeps it so).
- REQ-2 (**SubstKit**): the ~25-lemma binder kit (`sdenote_push_lift`,
  `sdenote_subst`, `sencode_fresh_ok`, companions), isolated in
  `Strat/SubstKit.lean` with its own micro-pins. Scheduled second, not
  last — the program's schedule variance lives here. The lemma list is
  fixed before coding starts from the SPIKE-1 note, which proves the two
  load-bearing lemmas (`sdenote_push_lift`, `sdenote_subst`) end to end on
  the toy in 11 supporting lemmas — consistent with the ~25 estimate, well
  under the 40-lemma F-A trigger, so **plain de Bruijn is confirmed and no
  fallback F-A review is required**. `Strat/Syntax.lean` inherits the
  note's exact statement shapes verbatim (formula as the induction target,
  with cutoff/index/value/env quantified after it) and its `cons`/`insert`
  commutation lemma; decidability on the bundled carrier sort is routed as
  data (the note's §6 carry-as-data convention), not via a
  `[DecidableEq]` instance.
- REQ-3 (**classifier, kernel half**): `Strat/Nnf.lean` (NNF + prenex
  with `nnf_sound`/`prenex_sound`), `Strat/Graph.lean` (`sortGraph` E1∪E2,
  `acyclic` with `acyclic_iff_no_cycle`), `Strat/Fragment.lean`
  (`idxGrammar` per (R2), `finCarrier` per (R1), `admitted`, the
  declarative `Frag`, and T3-C `classifier_correct`). Fragment versioned
  as S₂.0 — widenings are new grammar + citations + pins, never silent.
  **IMPLEMENTATION NOTE (binding, as shipped #325 — supersedes the
  single-syntax reading of REQ-1):** the classifier operates on its **own
  sort-typed surface syntax** — `Sort₂` (mach/seq/opaque) + a rich
  `Tm`/`Atom`/`Frm` carrying sorts on binders and the array-property term
  vocabulary (`Read`/`Len`/`Cast`/`IdxOp`/spec-fns) — because the admission
  traps (`a[a[i]]`, the cast cycle) are properties of that sort/term
  structure that REQ-1's deliberately *minimal* semantic-spine `Frm`
  (single carrier, unsorted `all`/`ex`, `Tm.var` only) cannot express.
  These live in the **`Thermite.Strat.Cls` namespace** (NOT `Thermite.Strat`
  — that collision was the #68 axiom-probe failure). So Stage 2 carries
  **two formula languages**: REQ-1's minimal `Thermite.Strat.Frm` (the
  semantic/SubstKit/encoder spine) and REQ-3's `Thermite.Strat.Cls.Frm`
  (the classifier surface). Downstream REQs must target the right one.
- REQ-4 (**classifier, ops half — M2b, ships before the encoder**):
  **consumes REQ-0's surface quantifier grammar** (the classifier cannot
  see formulas until raw `forall`/`exists` parse). The Rust classifier in
  `thermite-spec` (mirroring NNF + graph + grammar
  checks beside the existing validator), rejection reasons from the
  frozen vocabulary (`infinite-carrier`, `seq-quantifier`, the named
  cycle), and the **differential battery**: the SplitMix64 generator.
  **NOTE (per #325):** the Rust classifier mirrors REQ-3's **sort-typed
  `Thermite.Strat.Cls` syntax** (`Sort₂` + the array-property term
  vocabulary), NOT REQ-1's minimal spine `Frm` — it must reproduce the same
  `admitted` over the same sorted structure. The SplitMix64 generator
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
  **NOTE (per #325 — REQ-5 IS THE BRIDGE):** the two-syntax split (REQ-3
  note) makes REQ-5 the increment that reconciles the classifier surface
  (`Thermite.Strat.Cls.Frm`) with the semantic spine
  (`Thermite.Strat.Frm` + `sdenote`): the encoder/soundness track is where
  an admitted (`Cls.Frm`) formula gets connected to a sound denotation. Size
  this reconciliation explicitly — it was deferred here by design, not
  forgotten. (REQ-3's `nnf_sound`/`prenex_sound` are proved against a
  *structural* denotation that erases sorts, which is **stronger** than any
  single per-sort model, so it composes with the eventual `sdenote`.)
- REQ-6 (**combinator demotion**): `Strat/CombDeriv.lean` — the eight
  `comb_deriv_*` lemmas proving each v1 combinator's denotation equals
  its raw-quantifier expansion (closing the v1 embedding), plus
  `PinCombDeriv` refuting an off-by-one expansion. The SPIKE-2 shape
  census found that two of the eight have **no layer-1 raw-quantifier
  spelling** — `count_where` (a recursive `nat` fold) and
  `permutation_of` (a multiset equality) — so their `comb_deriv_*`
  lemmas demote to those definitional forms, not to a quantifier
  expansion, and their stratified TV handling routes through the
  semantic phase (see REQ-8), not the syntactic normalizer. The combinator
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
  semantic phase as a **thin fallback (Q-TV2 resolved: SPIKE-2 measured
  40/40 = 100% syntactic coverage, clearing the ≥ 90% bar, so the
  negation-unfriendly quantified-equivalence Z3 query ships as the
  rarely-hit path with finite-bound assertions — not a dedicated design
  issue)**, honest `Timeout` fallback withholding the certificate. The
  two non-quantifier combinators (`count_where`, `permutation_of`; REQ-6)
  have no syntactic normal form and land directly in the semantic phase.
  During the rollout window stratified
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

- [ ] AC-0: `thermite-syntax` parses raw `forall (x : S) in <dom>. φ` and
  `exists …` into the new binder AST node(s); a round-trip/parse test and
  the binder/scope parser pins are green; the combinator registry is
  unchanged. REQ-1 and REQ-4 build against this surface. (REQ-0)
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
Rust). What this doc adds is tree placement, verified against the tree at
the G1 re-pass (2026-06-19, main@`904ee01c`):

- **Lean**: `lean/Thermite/Strat/` as a sibling namespace to the v1
  spine; `QFree` atoms defer to the existing `Denote.lean` machinery, so
  the v1 arithmetic/cast/byte-view layers are consumed, not re-proven.
  The while-composition layer that landed post-RFC-baseline
  (`whileBodyDenote`/`while_compose`, #264/#265) is untouched by Strat —
  loop obligations stay v1-shaped in stage 2.
- **Rust classifier**: lives in `thermite-spec` beside the existing
  registry-free validator. *Re-pass finding (2026-06-19, confirmed in
  code):* raw quantifier parsing is NOT in stage 1's surface syntax — it
  added forge constructs only, and `forall_in`/`sorted` remain
  registry-free combinator identifiers (`thermite-syntax/src/parser.rs:1428`).
  The quantifier surface grammar (`parse_expr_bp`-level binder
  productions) is therefore sized as the foundation increment **REQ-0**,
  sequenced first and blocking both REQ-1 and REQ-4 — the one work item
  the program plan's stage split left implicit, now explicit.
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

*(All input dependencies are resolved as of the G1 re-pass (2026-06-19):
the two M0-spike-keyed questions resolved at the M0 re-pass — see the dated
`<!-- RESOLVED -->` records below, folded into the REQs above — and the two
non-spike inputs (G1, stage-1 routing telemetry) resolved in the input
table at the top. No open question remains; the doc is kickoff-ready.)*

<!-- RESOLVED: Q-KIT (M0 re-pass 2026-06-13) -->
### Q-KIT: Binder representation — plain de Bruijn, confirmed.

**Resolved by SPIKE-1** (`.design/strat/substkit-conventions.md`, proven
against `lean/Thermite/Spike/SubstKit.lean` on toolchain `v4.29.0`).
Plain de Bruijn is **confirmed** for REQ-2; **no fallback F-A** review is
required. Neither failure signal fired: the toy proved both load-bearing
lemmas (`sdenote_push_lift`, `sdenote_subst`) end to end in 11 supporting
lemmas (≤ 40), and the instance plumbing did not fight back. The carrier
stayed core-Lean-only via a hand-rolled finiteness witness (`enum` +
`complete` + `DecidableEq` as data) — **`Fintype` is not needed** and is
not imported, resolving the metatheory sketch's §2/§4
`Fintype`-vs-core-Lean tension in favour of core Lean (folded into
REQ-1). The two load-bearing lemmas never touch finiteness; only
`sdenote_all_iff` consumes the witness. One ergonomic finding carried
into REQ-2: decidability on the bundled carrier sort is routed as data,
not as a `[DecidableEq]` instance. The §4 statement shapes are inherited
verbatim by `Strat/Syntax.lean`.
<!-- /RESOLVED -->

<!-- RESOLVED: Q-TV2 (M0 re-pass 2026-06-13) -->
### Q-TV2: The semantic TV phase — thin fallback, confirmed.

**Resolved by SPIKE-2** (`thermite-tv/tests/fixtures/strat_probe/`,
prototype normalizer `thermite-tv/src/normalize.rs`). The
corpus+generated syntactic hit rate is **40/40 = 100%** (n = 40,
threshold-bearing; corpus-only 4/4, flagged small-n), clearing the ≥ 90%
bar across all six probed shapes (`sorted`, `forall_in`, `forall_below`,
`forall_from`, `exists_in`, `disjoint`). Decision-rule branch recorded:
the stage-2 semantic TV phase ships as a **thin fallback (F-C step 1)** —
the negation-unfriendly quantified-equivalence Z3 query is the rarely-hit
path with finite-bound assertions; **no dedicated semantic-query design
issue is required** before stage 2 commits. Census caveat folded into
REQ-6/REQ-8: two of the eight frozen registry combinators —
`count_where` (a recursive `nat` fold) and `permutation_of` (a multiset
equality) — have no layer-1 raw-quantifier spelling, so their stratified
TV handling routes through the semantic phase, not the syntactic
normalizer.
<!-- /RESOLVED -->

## Out of Scope

- T5-X / `Relax.lean` — landed in stage 1 (REQ-8 there); only the [1′]
  probe rows recur here.
- Sequence-sort quantifiers, nested sequences, unbounded-int binders —
  fragment v2.1+ / non-goals; forge-routed with named reasons.
- `@bv`, reconstruction — stage 3.
- Any (R2) widening past S₂.0 — telemetry-driven, post-G2, its own RFC
  delta.

---

*Stage-2 spec (re-pass complete, G1 re-pass 2026-06-19 — all four input
dependencies resolved, architecture and (R2) grammar re-grounded against
G1 reality, kickoff-ready) · child of `.design/thermite2-program.md`
(REQ-10) · spec of record: the stage-2 metatheory sketch, GH issue #2 ·
gate: G2 · baseline `dollspace-gay/Thermite @ 904ee01c`.*
