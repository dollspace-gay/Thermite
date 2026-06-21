<!--
tier: 3-component
status: draft
audited-content-sha256: 7fd2a7ef2df1f575e3be0f688d5c884f3e52e60902785e2ca7205ed5d218c9a3
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

The flip is HONESTLY SCOPED (REQ-5 option B / #330–#331): structure proven (T1-S), `qfree`
atoms grounded to the v1 `Thermite.denote` (T2-S), and `rel`/array atoms discharged by Z3's
theory (the solver base — model-relative). Kernel-grounding the `rel`/array atoms is stage-3
reconstruction; this doc does NOT claim it (see "Residuals", below).

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
| `fin_sort` / `fin_carrier` (`:245`/`:256`) | `finSort` / `finCarrier` (`Fragment.lean`) — opaque/seq carriers rejected (R1) | `classifier_correct` |
| `idx_ok_tm` / `idx_grammar_at` / `idx_grammar` (`:304`/`:325`/`:338`) | `idxGrammar` (`Fragment.lean`) — the (R2) index grammar | `classifier_correct` |
| `nnf` / `nnf_neg` (`:349`/`:362`) | `nnf` / `nnfNeg` (`Nnf.lean`) — NNF normalisation | `nnf_sound` |
| `edges_tm` / `edges_atom` / `edges_frm` / `sort_graph` (`:464`–`:550`) | `edgesTm`/`edgesAtom`/`edgesFrm`/`sortGraph` E1∪E2 (`Graph.lean`) | `classifier_correct` |
| `classify` / `admitted` (`:645`/`:666`) | `admitted` (`Fragment.lean`, the `Frag` decision) | `classifier_correct` (T3-C: `admitted φ = true ↔ Frag φ`) |
| `RejectReason` / `tag` (`:571`/`:608`) | the frozen rejection vocabulary (`infinite-carrier`/`seq-quantifier`/`index-grammar`/`…-cycle`) | n/a (reason naming — REQ-4) |

**The one intentional divergence (recorded, not a defect):** acyclicity. The Lean kernel uses
the exponential Roy–Warshall `reach` recursion (`Graph.lean`, fine for `decide` on the §3.2
micro-examples); the Rust side computes the SAME boolean by a polynomial transitive closure
(`Graph::acyclic`). The two agree by `acyclic_iff_no_cycle`; the **differential battery**
([8], `forge strat-tv`) is the empirical witness over the generated clause space.

The `to_wire` / `parse_frm` pair (`:679`/`:848`) is the wire protocol the differential battery
speaks to `lake env lean --run` on `Thermite.Strat.Cls.Wire`; it is mechanism, not a
correspondence arm.

## Table 2 — `strat_ref_encode.rs` ↔ `Strat/RefEncode.lean`

| Rust arm | Lean arm | Pinned by |
|---|---|---|
| `enc_name(d, i) = "v{d-1-i}"` (`:35`) | `encName d i = d - 1 - i` (de Bruijn LEVEL naming, fresh-name discipline — names strictly increase down every path, so no capture) | `strat_ref_sound` (T1-S, `PinStratCapture`) |
| `strat_ref_encode` (`:150`) | `sencode` — transcribes the boolean + relational + array-property SKELETON; sorts erased over the abstract `dom` | `strat_ref_sound` (parametric in the atom oracle `q : Atom → Bool`) |

T1-S proves only the STRUCTURAL layer (the quantifier/boolean skeleton, parametric in `q`);
atom-grounding is T2-S's obligation (Table 3). The encoder is INDEPENDENT of `thermite-lower`
(the TV honesty boundary — a reference that reused the production lowerer would make the
equivalence check vacuous).

## Table 3 — `strat_two_phase.rs` ↔ `Strat/{Nnf,Faithfulness}.lean` + the G2 gate

| Rust arm | Lean arm / role | Pinned by |
|---|---|---|
| `classify_pair` / phase 1 `normalize::equivalent` (`:117`) | the SYNTACTIC phase — the SPIKE-2 normaliser carrying `nnf_sound`/`prenex_sound` (`Nnf.lean`) | `nnf_sound` / `prenex_sound` |
| `semantic_obligation` / `FINITE_CARRIER_BOUND` (`:145`/`:166`) | the SEMANTIC phase — the finite-bound quantified-equivalence Z3 query (metatheory §8.2; the (R1) finiteness datum mirrored at the solver) | T2-S atom-grounding |
| `TvVerdict::Withheld` on `Timeout` (`:90`) | the honest `Timeout` fallback — withholds the certificate, never a false pass | (design invariant) |
| `strat_trust_profile` / `REF_ENCODE_{PROVEN,UNPROVEN}` (`:336`/`:321`/`:311`) | the trust label — proven form HONESTLY SCOPED to T1-S structure + T2-S qfree-grounding + Z3-theory rel | `strat_lowering_faithful` (T2-S) |
| `G2Checks` / `g2_flip_permitted` / `strat_trust_profile_gated` (`:363`/`:428`/`:437`) | THE G2 GATE — the flip is permitted iff declared AND all four checks green; the AC-9 mechanical block (toggle-each-red tests) | REQ-9 / AC-9 |

## Residuals (the honest scope — what this inspection does NOT cover)

- **Kernel-grounding of `rel`/array atoms.** T2-S grounds `qfree` atoms to the v1
  `Thermite.denote`; `rel`/array atoms stay MODEL-RELATIVE (discharged by Z3's theory — the
  solver base, the honest L4 boundary). Kernel-grounding them is stage-3 reconstruction. The
  G2 flip's `REF_ENCODE_PROVEN` string says so inline; it does not over-claim.
- **The two-syntax split.** This doc governs the `Cls.Frm` classifier surface (the mirror
  target). REQ-1's minimal semantic-spine `Frm` (`Strat/Denote.lean`) is the grounding
  instance, audited under its own axiom probe, not a correspondence row here.
- **String-level SMT formatting** and **the production lowerer** are out of scope — that is
  exactly what the two-phase TV ([9]) discharges per run, not a static inspection.
- **The extraction-bridge tier** (a mechanized Lean→Rust extraction making the Rust mirror
  equal the Lean model by construction) is the named stronger closure of this same residual,
  not in this doc's scope — identical to the v1 doc's REQ-2.
