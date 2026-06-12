# Feature: Stage 1 — the forge tier (L3) + real-relaxation routing

## Summary

Stage 1 of the RFC-1 program: the kernel proof tier (RFC-1 §5 — covenant,
frozen battery, anti-Goodhart at L3, the `Stuck`/`KernelBudget`/
`CovenantRefuted` verdicts) plus nlsat real-relaxation routing with
`RealWitness` (RFC-1 §4). No spine change beyond the small Mathlib-island
relax lemmas; the cage remains the v1 combinator menu. The headline that
lands at gate G1: out-of-cage no longer degrades down the ladder — it
escalates up to the forge. This doc covers the program plan §2's nine work
items as REQ clusters, grounded against the tree at `c46da3ac`, with the
Q-register defaults (Q1–Q4, Q6–Q8) adopted inline. Umbrella:
`.design/thermite2-program.md` (REQ-3); predecessor gate: M0
(`.design/m0-spikes.md`).

## Requirements

- REQ-1 (**verdict plumbing**, plan item 1): The seven-verdict vocabulary —
  `Proved`, `Counterexample`, `RealWitness`, `CovenantRefuted`,
  `Stuck(goals)`, `KernelBudget`, `Timeout` — becomes the closed
  certificate-level outcome set. The engine-level
  `Verdict { Proven, Refuted, Unknown(Reason) }` in `forge/src/engine.rs`
  maps total-functionally into it (no `Unknown` survives to a
  certificate). `KernelBudget` (elaboration/normalization budget, Q4
  default 30s per clause) gets its own shared discriminator alongside the
  existing solver-rlimit one in `forge/src/tv_signal.rs`
  (`is_rlimit_signal`), in the same one-shared-helper pattern. Certificate
  JSON goes to schema v2 with per-clause `engine`/`trust`/`verdict`
  fields, additive over the existing `Certificate` struct
  (`forge/src/manifest.rs`) so v1 oracle stability is preserved. The
  never-converts-silently property (R-VERDICT-1) is enforced by hermetic
  tests in the established degrade anti-cheat pattern
  (`forge/src/degrade.rs`: closure-instrumented non-invocation tests like
  `counterexample_never_degrades`).
- REQ-2 (**exporter hardening**, plan item 2): `forge/src/lean_export.rs`
  becomes the forge's front door. The per-item axiom gate
  (`#print axioms` ⊆ {propext, Classical.choice, Quot.sound}) runs as a
  *certifying* step on every forge-discharged item, not an audit-time
  step. All existing refusal paths are preserved and tested
  (`ExportRefusal`: `IncompleteRegistry` as the hard gate,
  `OutOfFragment`, `OpenHole`, …). The work includes the audit-F10
  re-inspection pass and a new correspondence-doc table for the exporter
  surface (extending `.design/verified/rust-lean-correspondence.md`'s
  drift-tripwire discipline via the shipped doc-drift gate).
- REQ-3 (**surface syntax**, plan item 3): `thermite-syntax` parses
  `prop fn`, `lemma name(args) req … ens … proof { … }`,
  `proof for f { ens#k by { … } }`, `?pN` proof holes,
  `witness { inhabit (…); falsify N; }`, refinement-type sugar
  (`x: T{P}` desugaring to req/ens + call-site obligations), and
  `dec lex(…)` / `dec wf ⟨rel⟩`. Slotting: new item kinds beside `fn` in
  the item parser; clause-level additions through `parse_contract`'s
  ordered-clause dispatch; `?pN` mirrors the existing `?N` hole machinery
  (`TokKind::Hole`, `FnItem.holes`, `HoleOutsideFnBody`) with proof-block
  position tracking. Open `?pN` holes block certification and build via
  the existing `open_hole_reason` path. Semantic addressing
  (`thermite-syntax/src/address.rs`) extends to proof blocks so
  `forge fill` can target `?pN`. Syntax is always-on (no feature flag);
  gating is forge-side via holes and routing.
- REQ-4 (**covenant engine**, plan item 4): `inhabit` witnesses are
  type-checked and *executed* against `req`; `falsify` rides the
  SplitMix64 generator (`thermite-tv/src/gen.rs`) aimed at the executable
  semantics; a hit is the hard-fail verdict `CovenantRefuted` with the
  counterexample attached (same never-degrades treatment as
  `Counterexample` in the ladder state machine, `forge/src/degrade.rs`).
  Covenant-before-burn is enforced structurally: the proof-search path
  cannot start without a covenant record — a tested invariant in the
  closure-instrumented style, not a convention. Q3 defaults adopted:
  `falsify 50_000` fixed-seed when unstated; witnesses may be
  generator-synthesized but at least one must be author-stated.
- REQ-5 (**frozen battery**, plan item 5): the tactic allowlist (`omega`,
  `simp`, `nlinarith`, `induction`, `decide`, `calc`, `exact`, `from`,
  `push_neg`) and the frozen simp set ship as a registry data file
  modeled on the combinator registry
  (`thermite-spec/src/combinators.rs`'s static `REGISTRY` of frozen
  signature entries), enforced at elaboration — a proof citing an
  unlisted tactic or simp lemma is refused, not warned. `Stuck` verdicts
  carry battery hints (residual goal + the "missing simp bridge"
  heuristic from RFC-1 §8's transcript).
- REQ-6 (**anti-Goodhart at L3**, plan item 6): three defenses.
  (a) Arbitrary-result re-elaboration for tautology: substitute an opaque
  result into the proof term; if it still elaborates, the `ens` said
  nothing → reject (the L3 counterpart of
  `forge/src/vacuity_solver.rs`'s `build_tautology_harness`).
  (b) Re-elaboration mutation: reuse the frozen mutation operator
  catalogue (`forge/src/mutation.rs` `generate()`, `MUTANT_CAP` 64)
  unchanged — only the kill check changes, from per-mutant solver run to
  "does the existing proof term still typecheck", decidable per mutant;
  survivors keep counting against the floor (Budd–Angluin). (c) The
  definition-tower budget (Q2 default: depth 4 / 40 definitions) plus
  `forge audit --meaning` printing the unfolded tower with its hash
  pinned in the certificate.
- REQ-7 (**`forge goal --proof` / `forge fill` UX**, plan item 7): the
  goal REPL (`forge/src/goal_repl.rs`) grows a proof view — forge-routed
  goals rendered with hypotheses in scope — and `fill_hole` accepts `?pN`
  addresses. Burn receipts (proof token count, lemmas cited) land in the
  certificate, per the RFC-1 §9 L3 certificate shape.
- REQ-8 (**relax routing**, plan item 8): the `relaxable` syntactic check
  (universally quantified, polynomial atoms, no div/mod/shifts/casts in
  atoms); nlsat invocation as an engine route recorded via the existing
  engine-attribution mechanism (`Certificate::with_engine_attribution`);
  the integrality check on real countermodels (Q8 default: rounding +
  radius-2 ℤⁿ box, 1s budget); `RealWitness` escalation carrying the raw
  real point to the forge as proof-search guidance. The spine lemmas
  `r_relax_sound` + `rencode_sound` (metatheory §7) land in a
  Mathlib-importing island module with the axiom probe extended to cover
  them — this sub-item is independent and can land first.
- REQ-9 (**lemma library mechanics**, plan item 9): per-project lemma
  namespace (Q1); citations resolve against certified lemmas only
  (`simp [melems_cons]` fails if `melems_cons` lacks a certificate);
  dedup-on-burn by statement hash with citation rewrite (Q1); `dec wf`
  accessibility proofs cached by (relation, carrier) hash, invalidated
  like other certificates (Q7); burned lemmas surface in `forge review`
  like any certified item. Proof sharing per Q6: one `proof for f` block
  may discharge several `ens#k` with shared `let`-style local lemmas; no
  cross-function sharing except via `lemma`. This item trails usage —
  last in dependency order.
- REQ-10 (**gate G1**): stage 1 is done when an end-to-end L3 certificate
  exists on a real merge-class example with covenant, axiom gate,
  re-elaboration mutation, and burn receipt all present; each of the
  seven verdicts is exercised by a hermetic test; the v1 conformance
  corpus passes with zero regressions; and the exporter re-inspection
  table is merged. The README/docs headline ("out-of-cage no longer
  degrades") changes at gate time, not merge time (R-GATE-1).

## Acceptance Criteria

- [ ] AC-1: A single seven-variant verdict enum exists in `forge/src`
  with serde round-trip tests; a total mapping from
  `engine::Verdict` is exhaustively match-tested (no wildcard arm, no
  path yielding a certificate without one of the seven). (REQ-1)
- [ ] AC-2: `tv_signal.rs` (or its successor module) distinguishes
  kernel-budget exhaustion from solver rlimit with two discriminators
  and a negative test proving neither matches the other's signal text.
  (REQ-1)
- [ ] AC-3: Hermetic tests assert, with instrumented closures, that
  `CovenantRefuted` and `Counterexample` never invoke any lower-ladder
  or retry path, and that no code path constructs `Proved` from a
  non-`Proved` verdict value. (REQ-1, REQ-4)
- [ ] AC-4: Schema-v2 certificates serialize the per-clause
  `engine`/`trust`/`verdict` block; the v1 oracle subset
  (`manifest.rs::oracle_subset`) is byte-identical for all existing
  golden certs (`conformance/*.cert.json`). (REQ-1)
- [ ] AC-5: A forge-discharged item whose proof introduces a fourth
  axiom fails certification with the axiom named; a test exercises this
  with a `Classical`-adjacent axiom. Every `ExportRefusal` variant has
  at least one test. (REQ-2)
- [ ] AC-6: The exporter correspondence table exists under
  `.design/verified/` and the doc-drift gate routes
  `forge/src/lean_export.rs` to it. (REQ-2)
- [ ] AC-7: Parser accepts all six new surface forms with AST + address
  round-trip tests; `?pN` outside a proof block is a structured syntax
  error; an open `?pN` blocks build and certification through
  `open_hole_reason`. (REQ-3)
- [ ] AC-8: An item with forge-routed clauses and no `witness` block
  does not reach proof search (tested structurally); a planted-bug
  example dies as `CovenantRefuted` with the concrete counterexample in
  the certificate; the unstated-budget default is 50,000 with a fixed
  seed recorded. (REQ-4)
- [ ] AC-9: The battery registry file exists; a proof citing an
  unlisted tactic or simp lemma is refused with the name in the error;
  a `Stuck` verdict on the RFC's merge example carries the residual
  goal and a missing-bridge hint. (REQ-5)
- [ ] AC-10: The arbitrary-result re-elaboration check rejects a
  body-ignoring `ens` (test fixture); re-elaboration mutation reuses
  `mutation::generate` (asserted by a test that the operator catalogue
  is shared, not forked) and reports killed/scored in the certificate;
  a tower deeper than the budget fails `forge audit --meaning` and the
  certificate pins the unfolded hash. (REQ-6)
- [ ] AC-11: `forge goal --proof` renders hypotheses for a forge-routed
  goal; `forge fill <item> ?p0 "<tactics>"` closes a goal end to end on
  the merge example; the resulting certificate contains the burn
  receipt (token count + cited lemmas). (REQ-7)
- [ ] AC-12: `relaxable` admits the isqrt postconditions and rejects a
  div-containing clause (unit tests); the isqrt example certifies L4
  push-button with `engine: nlsat` attribution; a true-over-ℤ,
  false-over-ℝ claim (`∀ n . n*n ≠ 2`) yields `RealWitness` carrying
  the real point, never `Counterexample`; the axiom probe covers
  `r_relax_sound` and `rencode_sound`. (REQ-8)
- [ ] AC-13: Citing an uncertified lemma fails with the lemma named;
  burning a statement-hash duplicate rewrites the citation instead of
  storing a copy (test with two identical lemmas under different
  names); a `dec wf` re-check hits the accessibility cache (observable
  via the cache layer, `forge/src/cache.rs` conventions). (REQ-9)
- [ ] AC-14: G1 checklist green in one run: the merge-class example
  certifies L3 (clauses L4, L4, L3) with all four evidence blocks
  populated; a hermetic test per verdict (seven tests named for their
  verdict); `forge check` over `conformance/` matches all existing
  golden certificates; the re-inspection table from AC-6 is merged.
  (REQ-10)

## Architecture

**What stage 1 builds on (all shipped, cited from the tree at
`c46da3ac`):** The forge already routes L3 through an engine abstraction
— `forge/src/engine.rs` defines the `Engine` trait (name / fragment /
discharge / trust_profile / evidence_key) with `VerusEngine` default and
`LeanAuto`/`LeanInteractive` engines behind `--engine`, recording
`engine_attribution` on non-default routes. The exporter
(`forge/src/lean_export.rs`) already has three tiers (fuel-free auto,
static-unfold auto, recursive-interactive emitting stabilization
theorems) and a structured `ExportRefusal` enum with `IncompleteRegistry`
as the hard gate. So stage 1 is a *hardening and widening* of an
existing Lean path, not a greenfield tier: the seven verdicts replace
the narrower `L3Verdict`/`Verdict::Unknown` vocabularies, the covenant
and battery wrap the existing discharge path, and the axiom gate moves
from audit-time to certify-time.

**Verdicts and the ladder.** `forge/src/degrade.rs` is the model and the
constraint: its `LadderAction` state machine and closure-instrumented
anti-cheat tests (counterexample never degrades, L2 counterexample never
drops to L1) are the pattern REQ-1's R-VERDICT-1 tests extend.
`CovenantRefuted` is wired as a `Counterexample`-class hard fail in that
state machine. The certificate layer (`forge/src/manifest.rs`) keeps its
additive-field discipline: schema v2 adds per-clause verdict blocks
without disturbing `oracle_subset()`'s 6-tuple for v1 items.

**Syntax.** `thermite-syntax` is registry-free recursive descent;
`parse_contract` enforces clause order and `parse_fn` records `?N` holes
with depth tracking. New items (`prop fn`, `lemma`, `proof for`,
`witness`) are item-level productions beside `fn`; refinement sugar
desugars in a post-parse pass to req/ens so downstream stages
(`thermite-spec` validation, lowering) see only the v1 clause shapes
plus the new item kinds. `thermite-syntax/src/address.rs` gains proof
addresses (`f.proof.ens#k`, `?pN`) so `goal_repl.rs`'s `edit_file`/
`fill_hole` machinery transfers unchanged.

**Covenant.** Executes through the same executable-semantics surface the
TV generator targets; `thermite-tv/src/gen.rs`'s `Rng` (SplitMix64,
deterministic, no clock) supplies `falsify` inputs. The
covenant-before-burn invariant is structural: the forge discharge entry
point takes the covenant record as a non-optional argument, so the
type system — not a runtime check — enforces R-COV-1, and the hermetic
test proves the proof-search closure never runs without it.

**Anti-Goodhart.** The L3 column of RFC-1 §10's table maps onto existing
modules: tautology → a re-elaboration harness beside
`vacuity_solver.rs`'s solver harnesses; weak contract →
`mutation.rs`'s catalogue with a swapped kill check; meaning →
new `forge audit --meaning` in `forge/src/audit.rs`'s numbered-check
style. Battery ordering follows the shipped precedent: vacuity-class
checks before proof, mutation after `Proved`.

**Relax route.** A new engine implementing the existing `Engine` trait
(fragment = relaxable clauses; discharge = nlsat query + integrality
check; trust_profile = `solver(nlsat) + spine-lemma(kernel)`), so
routing, attribution, and certificates need no new mechanism. The
`RealWitness` escalation hands the real point to the forge path as goal
metadata. The two spine lemmas live in a single Mathlib-importing Lean
module, isolated so the hot path stays core-Lean-only, with the audit
axiom probe extended to them (the metatheory §12 [1′] pattern, applied
early).

**Dependency order (the plan's):** items 1–2 (REQ-1, REQ-2) are the
foundation; 3–7 (REQ-3..7) parallelize once the verdict enum and
exporter gate exist; 8 (REQ-8) is independent and may land first; 9
(REQ-9) trails usage. Per the umbrella's Q-TRACK split: one GH issue per
REQ cluster referencing G1, increments in crosslink.

## Open Questions

### Q-ORACLE: Which schema-v2 fields join the certificate oracle subset? (resolved)

**Decision:** the oracle subset gains the per-clause *verdict* and
*trust* strings, the meaning-audit hash, **and the covenant record**
(witness count, falsify generated/refuted counts, seed). All four are
deterministic — the covenant because Q3's default is fixed-seed — so
golden-cert stability is preserved, and covenant evidence cannot drift
silently: weakening a falsify budget or dropping a witness changes the
oracle and fails the golden comparison. The burn receipt stays
oracle-excluded like `solver_time_ms` (its committed-proof token count
is deterministic, but re-authoring a proof legitimately changes it
without changing what was proven). Consequence for AC-4: the golden-cert
byte-stability claim applies to *v1 items*; new forge-tier goldens
include the covenant block from day one. A deliberate covenant change
(e.g. raising a falsify budget) is a golden-cert update, reviewed as
such.

### Q-BURN: What unit is the burn receipt's "proof tokens"? (resolved)

**Decision:** hybrid. `burn.proof_tokens` always records the lexer-token
count of the committed proof text — deterministic and re-derivable by a
skeptic. An optional `burn.authoring_tokens` field records LLM tokens
spent when the authoring harness supplies them (absent otherwise),
keeping the thesis's "burn the cheap resource" measurable where the
harness cooperates. Both fields stay oracle-excluded per Q-ORACLE
(re-authoring a proof legitimately changes both). RFC-1 §9's
`proof_tokens: 287` reads as the lexer count; the burn-economics metric
in the program plan §6 (tokens per discharged L3 clause) consumes
`authoring_tokens` where present and falls back to `proof_tokens` as a
proxy, with the dashboard labeling which it is.

## Out of Scope

- The stratified cage: classifier, sort graph, restratify, `Strat/`
  spine modules, two-phase TV, generator binder productions — stage 2
  (its own design pass per umbrella REQ-10).
- `@bv` and the three locks; SMT proof reconstruction — stage 3.
- Any combinator-menu change: the cage's fragment is untouched in
  stage 1; out-of-menu clauses route to the forge instead of degrading,
  which is the whole headline.
- Cross-project lemma sharing, human review workflow for burned lemmas
  beyond `forge review` surfacing (Q1's default scope).
- Float/transcendental routing (W8), string fragments (W25), heap
  predicates (W26) — program non-goals.

---

*Stage-1 spec · child of `.design/thermite2-program.md` (REQ-3) ·
sources: RFC-1 §4/§5/§8/§9/§10/§12, program plan §2/§5, metatheory §7 ·
gate: G1 · baseline `dollspace-gay/Thermite @ c46da3ac`.*
