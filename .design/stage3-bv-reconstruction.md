# Feature: Stage 3 — `@bv` clause mode + reconstruction by default

> **STATUS: KICKOFF-READY (re-pass 2026-06-21).**
> Both prior `<!-- OPEN -->` blocks (Q-BVSCOPE, Q-RECON) are RESOLVED
> against post-G2 evidence — see the Decision Record at the foot of this
> doc. Baseline re-grounded to current `main @ 2d6c708e` (Stage 2
> complete, Gate G2 reached). The architecture seams below carry
> verified `file:line` anchors, not speculation. Next step: open the
> Stage-3 issue tree (umbrella + increments), structured as Stage 2 was.

| input dependency | what it decided here | resolution |
|---|---|---|
| Gate G2 (stage 2 complete, trust flip done) | The per-clause `trust:` migration mechanics this stage reuses; stable schema-v2 certificates | RESOLVED: schema-v2 live (`forge/src/manifest.rs:226-316`); `with_clause_attribution(engine, trust, verdict)` is the migration seam |
| Stage-1/2 review telemetry (`forge review`) | Q-BVSCOPE: full / `nowrap`-only / lemma-only | RESOLVED → **full tag + 3 locks**. The "bv-density telemetry" input is circular (no bv corpus can exist pre-ship), so REQ-6's density report becomes the *post-ship* F-F tripwire, not a precondition |
| Lean-SMT / cvc5 replay ecosystem assessment at G2 | Q-RECON: reconstruction engine + fragment support; default-on viability | RESOLVED → **build the Rust→Lean exporter; default-on for QF_LIA + QF_BV**. lean-smt pinned @ `7d1d8239` (vendored cvc5 FFI); `SmtDemo.lean` proves QF_LIA kernel-replay axiom-clean; cvc5 supports QF_BV. The gap was never ecosystem maturity — it is the automated obligation exporter (PoC Tier-3 hand-translation) |
| `KernelBudget`/`Timeout` telemetry on bv-shaped queries | Whether 64-bit multiplier instances need a dedicated budget profile | RESOLVED → folded into REQ-2: QF_BV 64-bit multiplication is the known cost cliff; it gets a dedicated budget profile and the `Timeout` verdict, never `unknown` |

## Summary

Stage 3 of the RFC-1 program — its last gate (G3) — in two halves that
now converge on QF_BV. (1) The `@bv` machine-semantics clause tag
(RFC-1 §4): `ens@bvN P` / `inv@bvN P` interpreted over fixed-width
wraparound semantics as QF_BV decided by Z3 directly (the same
bit-blaster Verus's `by(bit_vector)` invokes), where everything
including multiplication is decidable with bit-level countermodels —
certifying at the caged rung L4, shipped only with its three locks
(shadow flag,
bv-semantics mutation, `nowrap` side obligation) and gate G3's
structural guarantee that a build without shadow-flag plumbing cannot
even *parse* the tag. (2) SMT proof reconstruction flipped to
default-on, migrating cage clauses' trust base from solver to kernel
*without touching their rung* — the C4-grid honesty, delivered through
the `trust:` field. The reconstruction half builds the automated
Rust→Lean obligation exporter that `SmtDemo.lean`'s Tier-3 PoC stubbed
as hand-translation, and turns default-on for the QF_LIA scalar
fragment (PoC-proven) and QF_BV (cvc5-supported) — so a `@bv` clause is
not only width-decidable but *kernel-checked*, the two halves meeting
on the bit-vector fragment. The G2 trust residual (rel/array
EPR-stratified atoms, still z3-model-relative) is migrated *where the
reconstruction path supports it* and otherwise named in the audit's
residual-trust statement (fallback F-J: reconstruction was never
load-bearing for any gate, so an unsupported fragment is free to stay
labeled). The bv half is the program's one permanent semantic fork and
its one new gaming vector; it is staged last for that reason, with the
locks shipping inside the same gate as the feature. Umbrella:
`.design/thermite2-program.md` (REQ-10).

## Requirements

- REQ-1 (**the tag, parse-gated**): `thermite-syntax` parses `ens@bvN`
  / `inv@bvN` / `@bvN(nowrap)` for N ∈ {8, 16, 32, 64} **only when the
  shadow-flag plumbing is compiled in**: a build-flag test proves that
  with the plumbing absent, the tag is a structured parse error (G3's
  structural lock — the feature cannot exist without its visibility
  machinery, R-BV-1). `lemma` items accept the tag under the same gate.
  The tag is the FIRST clause-level annotation in the language: today
  `Clause` (`thermite-syntax/src/ast.rs:596`) is `{ expr, text, span }`
  with no annotation field, and `parse_contract` (`parser.rs:1376`)
  dispatches req/ens/fx with no tag slot — both grow a `bv: Option<BvTag>`.
  The gate is a cargo feature on `thermite-syntax` (it has none today,
  `thermite-syntax/Cargo.toml`); the parser consults
  `cfg!(feature = "bv")` at the dispatch site so a release build with
  the feature off rejects the tag at parse time.
- REQ-2 (**lowering**): tagged clauses lower to **QF_BV decided by Z3
  directly** — the same decision procedure Verus's `by(bit_vector)`
  invokes (Z3's bit-blaster), reached as its own `EngineName::BitVector`
  route exactly as the stage-1 `Nlsat` route reaches Z3's nlsat directly
  rather than through a Verus VC round-trip (`forge/src/engine.rs`,
  `forge/src/bitvector.rs`). Direct emission is deliberate: the rendered
  SMT-LIB2 QF_BV query *is* the artifact REQ-8 reconstruction replays, so
  this route hands off cleanly to the kernel-grounding half. Untagged
  clauses on the same item lower as before — one function may carry
  wraparound and unbounded clauses side by side, each labeled (the RFC's
  mix64 shape: two `@bv64` clauses + one unbounded zero-fixpoint clause).
  A `@bv` clause certifies at **L4** (the caged rung — decidable, complete
  bit-pattern countermodels per RFC §2/§4; never degraded), with its
  SOLVER trust base (`solver Z3 QF_BV`) recorded **separately** in the
  per-clause attribution — rung and trust are orthogonal axes, and REQ-8
  shrinks the trust at the same rung. (The general Verus/Z3 cage still
  certifies at L3 in code pending its own L4 promotion — a tracked
  follow-up, not this increment.) Verdicts ride stage-1 plumbing:
  bit-level `Counterexample` with the bit pattern attached; `Timeout` for
  the multiplier cost cliff — QF_BV 64-bit multiplication gets a
  **dedicated budget profile**, reported as `Timeout`/`KernelBudget`,
  never `unknown` and never a silent downgrade.
- REQ-3 (**lock 1 — the shadow flag**): every tagged clause's
  certificate carries `bv_shadow { flagged, semantics, nowrap_obligation,
  note }` (the RFC §9 shape) as an additive schema-v2 field on
  `ObligationResult` (`forge/src/manifest.rs:226`, following the
  `#[serde(default, skip_serializing_if)]` pattern that keeps v1
  goldens byte-identical), oracle-included (Q-ORACLE precedent:
  deterministic + verdict-relevant → included), and greppable at every
  layer `#[slag]` is greppable at (certificate JSON, `forge review`
  output, `forge audit` aggregation).
- REQ-4 (**lock 2 — bv-semantics mutation**): the mutation battery
  (`forge/src/mutation.rs`) runs against bv semantics for tagged
  clauses — the existing frozen operator catalogue is reused unchanged
  (the stage-1 re-elaboration precedent: only the kill check swaps),
  with `classify_mutant`'s oracle (`mutation.rs:99`) evaluated *at
  width* so a wrap-exploiting mutant — equivalent at unbounded
  semantics, distinguishable at width — must be killable by a
  wrap-aware check.
- REQ-5 (**lock 3 — `nowrap`**): `@bvN(nowrap)` additionally emits a
  no-overflow side obligation discharged in-cage; its certificate's
  `bv_shadow.nowrap_obligation` records the obligation's verdict. For
  when machine width is the domain but wrap is not the intent.
- REQ-6 (**review surface, the F-F tripwire**): `forge review` /
  `forge audit` gain a "semantic forks and definition towers" section
  (the audit infra exists — `forge/src/audit.rs`, `forge/src/meaning.rs`
  tower analysis, `forge/src/review.rs` burned-lemma surfacing — this
  section is additive over it): bv-shadow density per module and the
  burned-lemma tower depths — the human-audit surface for the two
  legibility risks the forge and bv add. Because no bv corpus exists
  until the tag ships (the Q-BVSCOPE circularity), this density report
  is the **post-ship retreat trigger**: rising shadow-flag density in
  contract-bearing code is the named F-F tripwire down the ladder
  (full → `nowrap`-only → lemma-only → drop).
- REQ-7 (**the Rust→Lean obligation exporter**): build the automated
  exporter that turns a per-clause obligation into the
  `smt`-dischargeable Lean goal `(P_production) ⟺ (P_reference)` —
  the step `SmtDemo.lean`'s Tier-3 PoC performs by hand
  (`lean/Thermite/SmtDemo.lean`, "the hand-translation step is the gap
  an automated Rust→Lean exporter would close"). The exporter covers
  the QF_LIA scalar fragment (comparisons + connectives over `int`,
  the PoC's proven shape) and the QF_BV fragment (the bit-vector
  clauses REQ-2 produces). Its output is fed to the lean-smt `smt`
  tactic (pinned @ `7d1d8239`, `lean/lakefile.toml`) and the resulting
  theorem's `#print axioms` must stay within `{propext,
  Classical.choice, Quot.sound}` for the fragment to count as
  reconstruction-supported.
- REQ-8 (**reconstruction default-on**): where the obligation's
  fragment is reconstruction-supported (REQ-7's QF_LIA + QF_BV, axiom
  check passing), solver results are replayed/checked in the kernel and
  the clause's `trust:` migrates from `solver(z3)` to the
  kernel-checked form via `with_clause_attribution`
  (`forge/src/manifest.rs:306`) — same rung, smaller trust base,
  visible per clause and aggregated by the audit's residual-trust
  statement. The fragment-support check ADDITIONALLY migrates
  EPR-stratified rel/array clauses (the G2 residual that left
  `strat_lowering_faithful`'s rel atoms z3-model-relative) *where the
  reconstruction path supports them*; unsupported fragments keep
  per-run solver trust, labeled as today, and the audit names exactly
  what stayed solver-trusted (fallback F-J is free — reconstruction was
  never load-bearing for any gate, so no gate regresses if a fragment
  is unsupported). **REQ-8 owns the `render_bv_prop` faithfulness
  obligation:** because REQ-2 emits QF_BV via a *direct* SMT-LIB2 render
  (`forge/src/bitvector.rs`) rather than through the Verus lowering, that
  render is a new translation NOT covered by the existing Verus-path
  translation validation — it enters the TCB until reconstruction
  replays forge's *actual rendered query* in the kernel. Closing that
  bv-render trust gap (replaying the real query, not a re-derivation) is
  an explicit REQ-8 acceptance condition, so the gap is tracked, not
  silent.
- REQ-9 (**gate G3**): the three locks demonstrably wired before the
  tag parses in any release build (REQ-1's build-flag test); the
  exporter + reconstruction default-on behind the fragment-support
  check with the `trust:` migration visible in certificates and the
  residual-trust statement honest about what stayed solver-trusted;
  README/RATIONALE/docs claims about `@bv` and default reconstruction
  change **at gate time only** (R-GATE-1), as G1 and G2 did.

## Acceptance Criteria

- [ ] AC-1: A build with the `bv` feature off fails to parse `ens@bv64`
  with a structured syntax error (build-flag test in CI); with the
  feature on, all four widths + `nowrap` parse with AST round-trip
  tests over the new `Clause.bv` field. (REQ-1, REQ-9)
- [ ] AC-2: The mix64 example certifies with three clauses on three
  mechanisms (two `@bv64` via `EngineName::BitVector`, one unbounded),
  each clause's certificate naming its engine and semantics; the
  injectivity lemma discharges at `@bv64` with no proof block. (REQ-2)
- [ ] AC-3: A planted non-injective rotate dies as `Counterexample`
  with the bit pattern in the certificate; an over-budget 64-bit
  multiplier query yields `Timeout` under the dedicated bv budget
  profile, never `unknown` and never a silent downgrade. (REQ-2)
- [ ] AC-4: `bv_shadow` appears in the oracle subset for tagged
  clauses; `grep`ing certificates for `bv_shadow` finds every tagged
  clause and nothing else; `forge review` and `forge audit` list them.
  (REQ-3, REQ-6)
- [ ] AC-5: A wrap-exploiting mutant (equivalent at unbounded
  semantics, distinguishable at width) is killed by the bv-semantics
  mutation run on a tagged clause; the same mutant survives the
  unbounded check — a fixture test pinning the width-aware kill.
  (REQ-4)
- [ ] AC-6: A `@bv64(nowrap)` clause whose body can overflow fails its
  side obligation with a concrete overflowing input; the obligation's
  verdict is recorded in `bv_shadow.nowrap_obligation`. (REQ-5)
- [ ] AC-7: The review section reports bv-shadow density per module and
  tower depth for burned lemmas; the numbers match a fixture project's
  known counts; a synthetic density spike trips the named F-F warning.
  (REQ-6)
- [ ] AC-8: The exporter emits a `(P_prod) ⟺ (P_ref)` Lean goal for a
  QF_LIA scalar clause AND a QF_BV `@bv` clause; each is discharged by
  `smt` and `#print axioms` reports ⊆ `{propext, Classical.choice,
  Quot.sound}` (no `Smt`-internal oracle, no `ofReduceBool`). (REQ-7)
- [ ] AC-9: A reconstruction-supported clause (QF_LIA or QF_BV)'s
  certificate shows the kernel-checked `trust:` form while an
  unsupported clause on the same item retains `solver(z3)`; the audit's
  residual-trust statement aggregates the split and names the
  still-solver-trusted fragments. (REQ-8)
- [ ] AC-10: All G3 items hold in one CI run before any
  README/RATIONALE claim about `@bv` or default reconstruction merges
  (the gate-time flip, R-GATE-1). (REQ-9)

## Architecture

**Where stage 3 plugs in (re-grounded against `main @ 2d6c708e`):**

- **Syntax** (`thermite-syntax`): the tag is the language's first
  clause-level annotation. `Clause` (`src/ast.rs:596`, today `{ expr,
  text, span }`) grows `bv: Option<BvTag>` where `BvTag { width:
  BvWidth, nowrap: bool }`; `parse_contract` (`src/parser.rs:1376`,
  the req→ens→fx ordered dispatch) grows a tag-parsing step guarded by
  `cfg!(feature = "bv")`. `thermite-syntax/Cargo.toml` gains its first
  `[features]` entry (`bv`). The build-flag test compiles the crate
  twice and asserts the off build rejects `@bvN` at parse time — the
  R-BV-1 structural lock. Note stage-1/2 added clause *content* as
  expression-level constructs (`Expr::Quantifier`, `parser.rs:2600`);
  the tag is genuinely new mechanism, not a copy of that seam.
- **Lowering + engine** (`forge`): a new `EngineName::BitVector` in
  `src/engine.rs` (joining `Verus`, `LeanAuto`, `LeanInteractive`,
  `Nlsat`), rendering tagged clauses to SMT-LIB2 QF_BV and deciding them
  with Z3 directly (`src/bitvector.rs`) — the procedure `by(bit_vector)`
  invokes, reached as its own route per the `Nlsat` precedent. A `Proved`
  clause certifies at `Level::L4` (caged rung; trust `solver Z3 QF_BV`
  recorded separately). Certificate attribution rides the existing
  `with_clause_attribution(engine, trust, verdict)`
  (`src/manifest.rs:306`) — the same mechanism stage-1's `Nlsat` route
  used, so a mixed-mechanism function (mix64) attributes per clause for
  free. The 64-bit multiplier budget profile is a `BitVector`-specific
  rlimit/budget setting threaded through the same verdict plumbing that
  produces `Timeout`/`KernelBudget`.
- **Mutation at width** (`forge/src/mutation.rs`): the frozen catalogue
  (`MUTANT_CAP = 64`, `mutation.rs:61`) is reused unchanged; only the
  kill check swaps — `classify_mutant` (`mutation.rs:99`) evaluates the
  mutant against the tagged clause *at width*. This is exactly the
  stage-1 re-elaboration precedent (catalogue fixed, oracle swapped).
- **Certificates** (`forge/src/manifest.rs`): `bv_shadow` is an
  additive schema-v2 field on `ObligationResult` (`:226`), using the
  established `#[serde(default, skip_serializing_if)]` pattern so v1
  goldens stay byte-identical and the v1 oracle subset is unchanged for
  untagged clauses.
- **Reconstruction** (`lean/` + `forge`): lean-smt is pinned @
  `7d1d8239` with vendored cvc5 over FFI, toolchain v4.29.0 + Mathlib
  (`lean/lakefile.toml`). `lean/Thermite/SmtDemo.lean` already proves
  the path works and stays axiom-clean for QF_LIA (Tier 2 toy +
  Tier 3 one TV obligation, both `smt`-discharged, `#print axioms` ⊆
  the standard set). The Stage-3 exporter (REQ-7) closes the Tier-3
  hand-translation gap: a Rust→Lean obligation exporter producing the
  `(P_prod) ⟺ (P_ref)` goal for QF_LIA and QF_BV. cvc5 supports QF_BV
  (`lean/.lake/packages/cvc5` exercises `setLogic "QF_BV"`), so the bv
  half's queries are reconstruction-eligible. All reconstruction work
  is trust-field-only — no rung changes, no new verdicts — which is why
  F-J (an unsupported fragment stays labeled solver-trusted) is free.
- **Audit/review** (`forge/src/{audit,review,meaning}.rs`): the
  "semantic forks and definition towers" section is additive over the
  existing audit manifest (functions / project_assurance / tcb /
  lean_fragment) and the existing tower analysis in `meaning.rs`; it
  surfaces bv-shadow density and burned-lemma tower depth, and carries
  the F-F density tripwire.

**Why last, restated from the decision record:** the bv tag is the one
piece of the program that adds a *permanent* semantic fork and a
standing Goodhart vector; C3′ over C3 was precisely the decision to
stage it after the pure-win routing (stage 1) and the metatheory-heavy
cage growth (stage 2), with the locks shipping inside the same gate as
the feature, not after.

## Decision Record (re-pass 2026-06-21, resolves the prior OPEN blocks)

**D-BVSCOPE (was Q-BVSCOPE) → full tag + three locks.** RFC-1's
committed shape. The original input — "shadow-flag density telemetry
from `forge review`" — is circular: no bv clause exists to measure
until the tag ships. The honest resolution is to ship the full tag
guarded by its three locks, and make REQ-6's per-module density report
the *post-ship* F-F tripwire. The ladder (full → `nowrap`-only →
lemma-only → drop) is preserved as a documented retreat path keyed to
that live telemetry, not pre-committed downward.

**D-RECON (was Q-RECON) → build the exporter; default-on for QF_LIA +
QF_BV.** The G2-time ecosystem assessment found the cvc5/lean-smt path
mature and kernel-clean: `SmtDemo.lean` discharges QF_LIA equivalences
through `smt` with `#print axioms` inside the standard set, and the
vendored cvc5 supports QF_BV. The only gap is the automated Rust→Lean
obligation exporter (the PoC's Tier-3 hand-translation). Stage 3 builds
it (REQ-7) and turns reconstruction default-on (REQ-8) for QF_LIA and
QF_BV. The EPR-stratified rel/array residual that G2 left
solver-model-relative is migrated where the reconstruction path
supports it and otherwise named honestly in the audit — F-J keeps that
free, so G3 does not overclaim closing the entire rel/array gap.

## Out of Scope

- `@bv` as a default or file-level mode — non-goal, permanently
  per-clause and loud.
- Float/transcendental clause modes (W8 Richardson) — non-goal.
- Verified extraction/erasure — the named L3 trust residual; a future
  program.
- Any cage-fragment change — S₂ widenings are stage-2-lineage RFC
  deltas, not stage-3 work.
- Reconstruction as a gate dependency for anything — F-J is explicitly
  free; no stage gate cites it, and an unsupported fragment never
  blocks G3.
- Full kernel-grounding of every rel/array atom — Stage 3 migrates what
  the reconstruction path supports and names the rest; closing the
  entire EPR/array residual is bounded by external cvc5/lean-smt
  reconstruction support, not by this stage.

---

*Stage-3 spec (KICKOFF-READY · re-pass 2026-06-21) · child of
`.design/thermite2-program.md` (REQ-10) · sources: RFC-1 §4/§10/§12,
Appendix A §1.4/F-F/F-J · gate: G3 · baseline `dollspace-gay/Thermite @
2d6c708e`.*
