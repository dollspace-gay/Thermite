<!--
tier: 3-component
status: draft
audited-content-sha256: a7a96c79c077a9695bbaf6e1087030ab48bc9d7d669fd5179589465bbf4bc897
governs: thermite-spec/src/classifier.rs (the Rust admission classifier) ↔
         lean/Thermite/Strat/{Nnf,Graph,Fragment}.lean (Thermite.Strat.Cls.admitted,
         T3-C classifier_correct);
         thermite-tv/src/strat_ref_encode.rs (the stratified reference encoder) ↔
         lean/Thermite/Strat/RefEncode.lean (sencode, T1-S strat_ref_sound);
         thermite-tv/src/strat_two_phase.rs (the two-phase TV + the G2 gate) ↔
         lean/Thermite/Strat/{Nnf,Faithfulness}.lean (nnf_sound/prenex_sound, T2-S
         strat_lowering_faithful).
         This doc is NOT production code; it is the audit artifact that closes the
         stratified Rust↔Lean correspondence residual at the audit-by-inspection tier and
         is the doc-drift route (check [4′]) that gates the G2 trust flip (REQ-9 / AC-9).
thesis-refs:
  - .design/stage2-stratified-cage.md REQ-3/4/5/8/9 (the stratified cage + the G2 gate)
  - the stage-2 metatheory sketch (GH issue #2) §3.2 (the classifier), §8.2 (two-phase TV)
anchor-doc:
  - .design/verified/rust-lean-correspondence.md (the v1 arm-by-arm correspondence this
    doc is the stage-2 stratified sibling of — same inspection tier, same drift discipline)
epic: crosslink #331 (stage-2 REQ-9 — audit integration / the G2 gate)
-->

# Stratified Rust↔Lean Correspondence — the stage-2 arm-by-arm audit-by-inspection

## Summary

Stage 2 ships THREE new Rust files that MIRROR kernel-proven Lean models over the stratified
`Cls.Frm` surface (`.design/stage2-two-syntax-architecture` — the classifier syntax, NOT the
minimal semantic spine `Frm`). As with the v1 encoders
(`.design/verified/rust-lean-correspondence.md`), Lean proves the **Lean** definitions sound;
that the **Rust** mirrors implement the *same* algorithm is a separate claim, discharged here
by inspection and held current by the doc-drift tripwire (`tooling/doc-drift.py`, check [4′]).
This doc is one of the four `make audit` checks that gate the G2 certificate trust flip
(REQ-9 / AC-9): a content drift in any governed file flips this row red, which mechanically
withholds the flip (`forge g2-gate`).

## The claim being audited

> **(CORR-S)** For every construct in the admitted stratified fragment S₂.0, each Rust mirror
> (`classifier.rs` / `strat_ref_encode.rs` / `strat_two_phase.rs`) computes the same result
> the corresponding kernel-proven Lean model assigns — the classifier verdict
> (`Thermite.Strat.Cls.admitted`), the reference encoding (`sencode`), and the two-phase
> equivalence/normal-form (the `Nnf`/`Faithfulness` lemmas) respectively.

The trust reduction this closes (the stratified analogue of the v1 chain):

```
  {Lean-proven stratified models}   — classifier_correct (T3-C) / strat_ref_sound (T1-S) /
                                       strat_lowering_faithful (T2-S), axiom-clean [1′]
+ {this stratified correspondence}  — CORR-S, by inspection (THIS DOC), drift-gated [4′]
+ {the differential battery}        — Rust classifier ≡ Lean admitted on generated φ [8]
+ {the two-phase TV sweep}          — production lowering ≡ reference encoder per clause [9]
= the GATED stratified trust flip       (REQ-9 / AC-9 — all four green in one make audit run)
```

At Gate G2 this flip was intentionally narrower than full reconstruction:
structure was proved, qfree atoms were connected to v1 denotation, and
relation/array atoms remained model-relative. Gate G4 now closes that downstream
residual for admitted S₂.0 clauses. This document still has the smaller job of
attesting that the Stage 2 Rust mirrors match their Lean definitions.

## Audited files (content-pinned — re-pin on any change)

The pin is the `audited-content-sha256:` digest in this doc's header — a deterministic
aggregate SHA-256 over the three governed files' content (`tooling/doc-drift.py`
`_content_digest`). Any edit to a governed file changes the digest and FAILS the doc-drift
tripwire until this doc is re-audited and re-pinned (`make doc-drift`). The content pin is
chosen over a commit pin so a squash merge cannot leave an INVALID-PIN (the lower.rs lesson).

| Mirror (Rust) | Lean model | Pinning theorem (axiom-probed, [1′]) |
|---|---|---|
| `thermite-spec/src/classifier.rs` | `lean/Thermite/Strat/{Nnf,Graph,Fragment}.lean` | `Thermite.Strat.Cls.classifier_correct` (T3-C) |
| `thermite-tv/src/strat_ref_encode.rs` | `lean/Thermite/Strat/RefEncode.lean` | `Thermite.Strat.strat_ref_sound` (T1-S) |
| `thermite-tv/src/strat_two_phase.rs` | `lean/Thermite/Strat/{Nnf,Faithfulness}.lean` | `Thermite.Strat.strat_lowering_faithful` (T2-S) |

## Table 1 — `classifier.rs` ↔ `Strat/{Nnf,Graph,Fragment}.lean`

The Rust classifier is a line-for-line transliteration of `Thermite.Strat.Cls.admitted`:

```text
  admitted φ = finCarrier φ && idxGrammar φ && acyclic (sortGraph (nnf φ))
```

| Rust arm (`classifier.rs`) | Lean arm | Pinned by |
|---|---|---|
| `ScalarValue`; `Tm::Var` / `Const` / valued `Lit` / `Read` / `Len` / `Cast` / `IdxOp` / `Mul` / `App1`; `Atom::Rel` / ID-bearing `QFree` | the matching `Sort₂`-indexed syntax in `Strat/Nnf.lean` | constructor-by-constructor inspection; `classifier_correct` consumes the result |
| `fin_sort` / `fin_carrier` (`:245`/`:256`) | `finSort` / `finCarrier` (`Fragment.lean`) — opaque/seq carriers rejected (R1) | `classifier_correct` |
| `idx_ok_tm` / `idx_grammar_at` / `idx_grammar` (`:304`/`:325`/`:338`) | `idxGrammar` (`Fragment.lean`) — the (R2) index grammar | `classifier_correct` |
| `nnf` / `nnf_neg` (`:349`/`:362`) | `nnf` / `nnfNeg` (`Nnf.lean`) — NNF normalisation | `nnf_sound` |
| `has_scoped_var`; `edges_tm` / `edges_atom` / `edges_frm` / `sort_graph` | `hasScopedVar`; `edgesTm`/`edgesAtom`/`edgesFrm`/`sortGraph` E1∪E2 (`Graph.lean`) | `classifier_correct`; existential occurrences count because Skolemization can carry an earlier universal dependency through them |
| `classify` / `admitted` (`:645`/`:666`) | `admitted` (`Fragment.lean`, the `Frag` decision) | `classifier_correct` (T3-C: `admitted φ = true ↔ Frag φ`) |
| `RejectReason` / `tag` (`:571`/`:608`) | the frozen rejection vocabulary (`infinite-carrier`/`seq-quantifier`/`index-grammar`/`…-cycle`) | n/a (reason naming — REQ-4) |

**The one intentional divergence (recorded, not a defect):** acyclicity. The Lean kernel uses
the exponential Roy–Warshall `reach` recursion (`Graph.lean`, fine for `decide` on the §3.2
micro-examples); the Rust side computes the SAME boolean by a polynomial transitive closure
(`Graph::acyclic`). The two agree by `acyclic_iff_no_cycle`; the **differential battery**
([8], `forge strat-tv`) is the empirical witness over the generated clause space.

The `to_wire` / `parse_frm` pair is the differential wire protocol. Version 2
preserves the sort and stable ID of source constants, the actual integer/Boolean
literal value, function IDs, and qfree leaf IDs. The parser rejects malformed
values instead of substituting a default.

## Table 2 — `strat_ref_encode.rs` ↔ `Strat/RefEncode.lean`

| Rust arm | Lean arm | Pinned by |
|---|---|---|
| `enc_name(d, i) = "v{d-1-i}"` (`:35`) | `encName d i = d - 1 - i` (de Bruijn LEVEL naming, fresh-name discipline — names strictly increase down every path, so no capture) | `strat_ref_sound` (T1-S, `PinStratCapture`) |
| `const_name` and the `Tm::Const` / valued `Tm::Lit` arms | source constants remain distinct by sort and ID; integer and Boolean literal values are preserved | direct constructor inspection |
| `strat_ref_encode` (`:150`) | `sencode` — transcribes the boolean + relational + array-property SKELETON; sorts erased over the abstract `dom` | `strat_ref_sound` (parametric in the atom oracle `q : Atom → Bool`) |

T1-S proves only the STRUCTURAL layer (the quantifier/boolean skeleton, parametric in `q`);
atom-grounding is T2-S's obligation (Table 3). The encoder is INDEPENDENT of `thermite-lower`
(the TV honesty boundary — a reference that reused the production lowerer would make the
equivalence check vacuous). Its qfree arm remains deliberately opaque for this
structural Stage 2 TV; the production Gate G4 bridge uses the stable leaf ID to
recover and reconstruct the exact source expression.

## Table 3 — `strat_two_phase.rs` ↔ `Strat/{Nnf,Faithfulness}.lean` + the G2 gate

| Rust arm | Lean arm / role | Pinned by |
|---|---|---|
| `classify_pair` / phase 1 `normalize::equivalent` (`:117`) | the SYNTACTIC phase — the SPIKE-2 normaliser carrying `nnf_sound`/`prenex_sound` (`Nnf.lean`) | `nnf_sound` / `prenex_sound` |
| `semantic_obligation` / `FINITE_CARRIER_BOUND` (`:145`/`:166`) | the SEMANTIC phase — the finite-bound quantified-equivalence Z3 query (metatheory §8.2; the (R1) finiteness datum mirrored at the solver) | T2-S atom-grounding |
| `TvVerdict::Withheld` on `Timeout` (`:90`) | the honest `Timeout` fallback — withholds the certificate, never a false pass | (design invariant) |
| `strat_trust_profile` / `REF_ENCODE_{PROVEN,UNPROVEN}` (`:336`/`:321`/`:311`) | the trust label — proven form HONESTLY SCOPED to T1-S structure + T2-S qfree-grounding + Z3-theory rel | `strat_lowering_faithful` (T2-S) |
| `G2Checks` / `g2_flip_permitted` / `strat_trust_profile_gated` (`:363`/`:428`/`:437`) | THE G2 GATE — the flip is permitted iff declared AND all four checks green; the AC-9 mechanical block (toggle-each-red tests) | REQ-9 / AC-9 |

## The stage-2 pin battery (AC-10 — the eight refutation pins)

Each correspondence claim above rests on a kernel-proven Lean theorem. The **pin battery**
is the adversarial complement: for every load-bearing theorem, an in-tree
`lean/Thermite/Pin*.lean` file exhibits the BROKEN NEIGHBOUR — the smallest plausible
mis-implementation — and `decide`-checks (or, for the relax island, kernel-proves) that it
DIVERGES from the proven definition on a concrete witness. A pin is the executable
statement of *why the theorem's hypotheses are necessary*: drop the discipline the theorem
names and this witness flips. All eight are BUILD targets in `scripts/lean-axiom-probe.sh`,
so a `sorry` or a broken `decide` in any of them fails the Lean CI job; each is axiom-clean
(⊆ {propext, Classical.choice, Quot.sound}; no `native_decide`).

The metatheory §9 battery, **cited from the theorem each pin guards**:

| Pin (`lean/Thermite/…`) | Theorem it guards (the proven anchor) | The broken neighbour the pin refutes |
|---|---|---|
| `PinFiniteEscape.lean` | `Strat/Denote.lean` `sdenote_all_iff` (the (R1) completeness datum behind every `sdenote` `∀`; the spine grounding instance of `strat_ref_sound`) | a `List.all` fold over an INCOMPLETE enumeration reports `true` where the genuine `∀` is `false` — the soundness escape dropping the carrier `complete` witness would permit |
| `PinStratCapture.lean` | `Strat/Soundness.lean` `Thermite.Strat.strat_ref_sound` (T1-S) | an encoder that REUSES the de Bruijn name `0` for every binder, so an inner binder captures an outer variable (`∀x.∃y. x=c0` collapses to `…y=c0`) |
| `PinStratFlip.lean` | `Strat/Soundness.lean` `Thermite.Strat.strat_ref_sound` (T1-S) | an encoder that SWAPS the quantifier kinds (`all↔ex`), flipping the truth of `∃x. x=c0` |
| `PinStratSelfLoop.lean` | `Strat/Fragment.lean` `Thermite.Strat.Cls.classifier_correct` (T3-C) — the acyclicity arm | a classifier that STRIPS reflexive edges before `acyclic`, so it admits the `a[a[i]]` self-loop (`ex_selfLoop`) the real `admitted` rejects |
| `PinNNFPolarity.lean` | `Strat/Fragment.lean` `Thermite.Strat.Cls.classifier_correct` (T3-C) on `nnf` (`Nnf.lean` `nnf_sound`) | a classifier that builds the sort graph PRE-NNF, so a `(¬∃k.∀v…) ∧ (¬∃v.∀k…)` whose NNF flips into a `Key ⇄ Value` cycle is wrongly admitted |
| `PinRestratDropSide.lean` | `Strat/Restratify.lean` `Thermite.Strat.Cls.restrat_conservative` (T4-R) | a restratify that CERTIFIES the original φ from the rewrite φ' while DROPPING the `Side(φ',φ)` obligation (R-SIDE-1) — attesting a false φ |
| `PinRelaxRefute.lean` | `Relax.lean` `Thermite.Relax.r_relax_sound` (T5-X, stage-1) | the illegitimate CONVERSE — reading a failed real relaxation (`x²−x` at `x=1/2`) as an integer counterexample, where the integer clause is in fact valid (the `RealWitness` escalation) |
| `PinCombDeriv.lean` | `Strat/CombDeriv.lean` the eight `comb_deriv_*` demotion lemmas | an off-by-one expansion (`i ≤ len` for `i < len`) that lets the boundary index into range, diverging from the faithful `forall_in` |

The five soundness anchors (`strat_ref_sound`, `classifier_correct`, `restrat_conservative`,
`r_relax_sound`, plus the spine-grounding `strat_lowering_faithful` of Table 3) are the SAME
theorems the axiom probe [1′] gates; the battery is their negative-space witness set. Adding a
ninth admission trap is a NEW pin + a new row here, never a silent widening (the §3.2 / S₂.0
conservatism the classifier tables already record).

## Scope boundaries

- **Full relation/array reconstruction is downstream of this mirror audit.**
  This was an open residual at Gate G2. Gate G4 now reconstructs admitted S₂.0
  clauses through typed Lean semantics, exhaustive grounding, checked theory
  clauses, and LRAT. The remaining unsupported cases are formulas outside S₂.0
  or qfree leaves outside the checked QF_LIA/QF_BV source surface.
- **The two-syntax split.** This doc governs the `Cls.Frm` classifier surface (the mirror
  target). REQ-1's minimal semantic-spine `Frm` (`Strat/Denote.lean`) is the grounding
  instance, audited under its own axiom probe, not a correspondence row here.
- **String-level SMT formatting** and **the production lowerer** are out of scope — that is
  exactly what the two-phase TV ([9]) discharges per run, not a static inspection.
- **The extraction-bridge tier** (a mechanized Lean→Rust extraction making the Rust mirror
  equal the Lean model by construction) is the named stronger closure of this same residual,
  not in this doc's scope — identical to the v1 doc's REQ-2.
