# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
Because Thermite is a verification toolchain developed against the RFC-1 program
(GH issue #2) rather than a semver-released library, entries are organized by the
program's **stage gates** (G1, G2, G3) — each gate is the point at which a stage's
headline claim is allowed to ship (per R-GATE-1: docs change at gate time, not
merge time).

## [Unreleased]

## Gate G3 — Stage 3: fixed-width clauses and checked reconstruction (2026-07-29)

Stage 3 completes the RFC-1 program. Fixed-width arithmetic is explicit at the
clause boundary, and supported solver results are replayed as Lean theorems
before their trust profile changes.

### Added

- `ens@bvN`, `inv@bvN`, and `@bvN(nowrap)` for 8-, 16-, 32-, and 64-bit
  unsigned semantics.
- Fixed-width lowering for postconditions, lemma conclusions, struct
  invariants, and nested loop invariants.
- Per-clause `bv_shadow` records, width-aware mutation checks, fail-closed
  nowrap obligations, and audit/review density reporting.
- Default Lean replay of the solver route's `req → clause` theorem for QF_LIA and
  QF_BV. Certificates record the theorem, checker, source hash, axiom report,
  and the exact solver-input hash when the route exposes it.
- An axiom-clean LRAT reconstruction tactic and permanent 64-bit probe.
- The `g3` CI job, which runs both release parser configurations, the live
  fixed-width route, invariant checks, reconstruction tests, and the Lean
  axiom probe in one gate.

### Trust boundary

A clause moves from solver trust only after Lean accepts its validity theorem
with axioms contained in `{propext, Classical.choice, Quot.sound}`. Unsupported
or failed replay remains solver-trusted and is listed by the audit.
EPR-stratified relation and array atoms remain model-relative.

## Gate G2 — Stage 2: the stratified-FOL cage (2026-06-22)

The L4 stratified cage: an admission classifier over a sorted carrier, a
stratified-FOL extension of the Lean spine (`lean/Thermite/Strat/`), and a
two-phase trans-validation loop, gating an honestly-scoped certificate trust flip.
Shipped as 11 increments REQ-0..REQ-10 (issues #322–#332, umbrella #321), merged
to `main` (final increment @ `8547e2b9`).

### Added
- **Surface quantifiers** (REQ-0): raw `forall`/`exists` binder productions in
  `thermite-syntax` parsing to `Expr::Quantifier` over a named sorted carrier.
- **The Strat spine** (REQ-1/REQ-2): `Strat/{Syntax,Carrier,Denote}.lean` — a
  minimal de Bruijn semantic core (`Frm`/`Tm`/`Atom`, `sdenote` over finite
  carriers, QFree atoms deferring to the v1 denotation) plus the `SubstKit` binder
  lemma library. `PinFiniteEscape`/`PinBrokenLift`.
- **The admission classifier** (REQ-3/REQ-4): the rich sort-typed classifier
  surface `Strat/{Nnf,Graph,Fragment}.lean` under the `Thermite.Strat.Cls`
  namespace (NNF/prenex, sort-graph cycle detection, the index grammar, `admitted`,
  T3-C `classifier_correct`), its Rust mirror in `thermite-spec`, and a SplitMix64
  **differential battery** asserting Rust/Lean verdict agreement in CI.
- **The encoder bridge** (REQ-5): `Strat/{RefEncode,TokDenote,Soundness}.lean` —
  the trigger-free MBQI encoder and **T1-S `strat_ref_sound`** (the encoder
  transcribes the quantifier+boolean skeleton faithfully, parametric in an atom
  oracle). `PinStratCapture`/`PinStratFlip`.
- **Combinator demotion** (REQ-6): `Strat/CombDeriv.lean` proving each v1
  combinator equals its raw-quantifier expansion. `PinCombDeriv`.
- **Restratification** (REQ-7): `Strat/Restratify.lean` (**T4-R
  `restrat_conservative`**) + `forge edit --restratify`, emitting the `Side(φ',φ)`
  obligation in-cage — certification of `φ'` never counts for `φ` without a
  discharged Side (**R-SIDE-1**). `PinRestratDropSide`.
- **Faithfulness + the trust flip** (REQ-8): `Strat/Faithfulness.lean` (**T2-S
  `strat_lowering_faithful`** — qfree atoms grounded to the v1 `Thermite.denote`),
  production quantifier emission in `thermite-lower`, and two-phase TV. The
  certificate `trust:` flip from `ref_encode(strat, UNPROVEN)` to the proven form
  is itself a tested code path.
- **The G2 audit gate** (REQ-9): `make audit` grows checks `[1′]` (axiom probe over
  the stratified soundness theorems), `[4′]` (Rust⇄Lean correspondence via
  content-sha pins), `[8]` (the differential battery), `[9]` (the stratified TV
  sweep). `G2Checks.g2_flip_permitted` (`forge g2-gate`) mechanically blocks the
  flip if any check is red; new `strat-rust-lean-correspondence.md`.
- **The pin battery** (REQ-10): the 8-pin regression battery, each a kernel
  `decide` refutation (no `sorry`, no `native_decide`) — the 3 new pins
  `PinNNFPolarity`, `PinStratSelfLoop`, `PinRelaxRefute` (T5-X) plus the 5 prior,
  each cited from the theorem it guards.

### Trust boundary (as shipped)
`G2_FLIPPED=true` is in effect, **honestly scoped**: the effective L4 trust is
`[solver(z3), ref_encode(strat): structure proven (T1-S), qfree grounded to v1
(T2-S), rel/array by z3-theory (solver base; kernel-grounding rel = stage 3)]`.
The encoder skeleton (T1-S) and qfree→v1 grounding (T2-S) are kernel-proven; rel
and array atoms remain solver-model-relative pending Stage 3 reconstruction. The
stratified soundness theorems are axiom-clean (⊆ {`propext`, `Classical.choice`,
`Quot.sound`}).

### Architecture note
Stage 2 carries **two** formula languages, deliberately not unified:
`Thermite.Strat.Frm` (the minimal semantic spine) and `Thermite.Strat.Cls.Frm`
(the rich sort-typed classifier surface). A total meaning-preserving translation
between them is ill-defined; the REQ-5 encoder bridges them by encoding `Cls.Frm`
directly against a structural denotation. The `.Cls` namespace resolved an
axiom-probe constructor collision (#68).

### Changed
- **CI** (#76): the monolithic `lean` job was split into `lean-probe` (spine build
  + axiom probe) and a sharded `lean-spine-forge` matrix (the forge suite with the
  spine, partitioned 4 ways). Lean wall-clock ~16 min → ~8 min with no coverage
  loss; `lean-spine-forge` shard 1 also runs the G2 audit-gate step.

## Gate G1 — Stage 1: the forge tier (2026-06-18)

The L3 forge tier — a Lean-kernel discharge path above the Verus/Z3 solver — plus
relax routing to the new L4 rung. Declared on `main`.

### Added
- **The five-rung ladder** L0 (slag) … L4 (caged/kernel-grounded) and the **seven
  verdicts** (`Proved`, `Counterexample`, `RealWitness`, `CovenantRefuted`,
  `Stuck`, `KernelBudget`, `Timeout`), with the never-converts-silently invariant
  (**R-VERDICT-1**).
- **The forge tier**: schema-v2 certificates, the Lean discharge engine, `forge
  goal --proof` / `forge fill ?pN`, the burn receipt, and a per-project certified
  lemma library.
- **The covenant engine** (**R-COV-1**, covenant-before-burn): `witness { inhabit;
  falsify N; }` producing the `CovenantRefuted` verdict from a SplitMix64 falsifier.
- **Anti-Goodhart at L3**: arbitrary-result re-elaboration, re-elaboration mutation
  scoring (shared operator catalogue), and the definition-tower budget.
- **The frozen tactic/simp battery**: an auditable allowlist enforced at
  elaboration; citing an unlisted tactic/lemma is refused.
- **Relax routing → L4**: a relaxable-clause classifier + the `Nlsat` engine (Z3
  nlsat, QF_NRA) with an integrality check, producing `RealWitness` for
  true-over-ℤ/false-over-ℝ claims. `Level::L4` (L3 = solver proof; L4 =
  kernel-grounded: nlsat + spine lemma).
- **Governance**: `thermite2-semantics.md` (the normative semantics home), the
  R-rule register, the §6 metrics dashboard (`forge audit --metrics`, gating
  nothing), and the G1 gate artifact (the merge-class example certifying L4/L4/L3
  + the seven-verdict hermetic suite).

## v0.1 — baseline architecture

- The Thermite v0.1 toolchain: `thermite-syntax`, `thermite-spec` (the combinator
  cage + registry), `thermite-lower`, `thermite-tv`, and the v1 Lean spine with the
  L1/L2/L3 ladder and conformance corpus. See `thermite-architecture-v0.1` and
  `.design/thermite-design.md`.
