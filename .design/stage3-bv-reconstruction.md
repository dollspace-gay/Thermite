# Feature: Stage 3 — `@bv` clause mode + reconstruction by default (PROVISIONAL)

> **STATUS: PROVISIONAL — do not kickoff from this document.**
> This is an interim reasoning cache for the program's last and
> last-staged increment. It carries unresolved
> `<!-- OPEN -->` blocks keyed to inputs that cannot exist before G2,
> so validation flags it as not-kickoff-ready. When the inputs below
> are available, re-run the design pass
> (`/design --continue stage3-bv-reconstruction`) to resolve them
> against real results, re-validate, and only then `crosslink kickoff`.

| input dependency | what it decides here | produced by |
|---|---|---|
| Gate G2 (stage 2 complete, trust flip done) | The per-clause `trust:` migration mechanics this stage reuses; stable schema-v2 certificates | `.design/stage2-stratified-cage.md` REQ-9 |
| Stage-1/2 review telemetry (`forge review`, program plan §6) | Q-BVSCOPE: whether `@bv` ships full, `nowrap`-only, or lemma-position-only (fallback F-F's ladder) | dashboard from M1 |
| Lean-SMT / cvc5 replay ecosystem assessment at G2 time | Q-RECON: the reconstruction path's engine and fragment support; whether default-on is viable or F-J (status quo ante) holds | re-pass exploration |
| `KernelBudget`/`Timeout` telemetry on bv-shaped queries | Whether 64-bit multiplier instances need a dedicated budget profile before the tag ships | stage-1 verdict plumbing |

## Summary

Stage 3 of the RFC-1 program, two independent halves. (1) The `@bv`
machine-semantics clause tag (RFC-1 §4): `ens@bvN P` interpreted over
fixed-width wraparound semantics via Verus's `by(bit_vector)` mode,
where everything including multiplication is decidable with bit-level
countermodels — shipped only with its three locks (shadow flag,
bv-semantics mutation, `nowrap` side obligation) and gate G3's
structural guarantee that a build without shadow-flag plumbing cannot
even parse the tag. (2) SMT proof reconstruction flipped to default-on
where the fragment supports it, migrating cage clauses' trust base from
solver to kernel without touching their rung — the C4 grid honesty,
delivered through the `trust:` field. The bv half is the program's one
permanent semantic fork and its one new gaming vector; it is staged
last for that reason, and fallback F-F's ladder remains
open until the re-pass. Umbrella: `.design/thermite2-program.md`
(REQ-10).

## Requirements

- REQ-1 (**the tag, parse-gated**): `thermite-syntax` parses `ens@bvN`
  / `inv@bvN` / `@bvN(nowrap)` for N ∈ {8, 16, 32, 64} **only when the
  shadow-flag plumbing is compiled in**: a build-flag test proves that
  with the plumbing absent, the tag is a parse error (G3's structural
  lock — the feature cannot exist without its visibility machinery,
  R-BV-1). `lemma` items accept the tag under the same gate.
- REQ-2 (**lowering**): tagged clauses lower through Verus
  `by(bit_vector)` (QF_BV); untagged clauses on the same item lower
  as before — one function may carry wraparound and unbounded
  clauses side by side, each labeled (the RFC's mix64 shape: two
  `@bv64` clauses + one unbounded zero-fixpoint clause). Verdicts ride
  stage-1 plumbing: bit-level `Counterexample` with the bit pattern
  attached; `Timeout` for the multiplier cost cliff, budgeted, never
  `unknown`.
- REQ-3 (**lock 1 — the shadow flag**): every tagged clause's
  certificate carries `bv_shadow { flagged, semantics, nowrap_obligation,
  note }` (the RFC §9 shape), oracle-included, and greppable at every
  layer `#[slag]` is greppable at (certificate JSON, `forge review`
  output, audit aggregation).
- REQ-4 (**lock 2 — bv-semantics mutation**): the mutation battery runs
  against bv semantics for tagged clauses — the existing frozen
  operator catalogue with the kill-check oracle evaluated at width
  (a wrap-exploiting mutant must be killable by a wrap-aware check).
- REQ-5 (**lock 3 — `nowrap`**): `@bvN(nowrap)` additionally emits a
  no-overflow side obligation discharged in-cage; its certificate's
  `bv_shadow.nowrap_obligation` records the obligation's verdict. For
  when machine width is the domain but wrap is not the intent.
- REQ-6 (**review surface**): `forge review` gains the "semantic forks
  and definition towers" section (Q9 default, decide-by G3): bv-shadow
  density per module and the burned-lemma towers — the human-audit
  surface for the two legibility risks the forge and bv add. This
  section is also the F-F tripwire: rising shadow-flag density in
  contract-bearing code is the named retreat trigger.
- REQ-7 (**reconstruction default-on**): where the obligation's
  fragment is supported by the reconstruction path, solver results are
  replayed/checked in the kernel and the clause's `trust:` migrates
  from `solver(z3)` to the kernel-checked form — same rung, smaller
  trust base, visible per clause and aggregated by the audit's
  residual-trust statement. Unsupported fragments keep per-run solver
  trust, labeled as today (fallback F-J is the free status quo
  ante; reconstruction was never load-bearing for any gate).
- REQ-8 (**gate G3**): the three locks demonstrably wired before the
  tag parses in any release build (REQ-1's build-flag test);
  reconstruction default-on behind a fragment-support check with the
  `trust:` migration visible in certificates; README/docs claims change
  at gate time only (R-GATE-1).

## Acceptance Criteria

- [ ] AC-1: A build without shadow-flag plumbing fails to parse
  `ens@bv64` with a structured syntax error (build-flag test in CI);
  with plumbing, all four widths + `nowrap` parse with AST round-trip
  tests. (REQ-1, REQ-8)
- [ ] AC-2: The mix64 example certifies with three clauses on three
  mechanisms (two `@bv64`, one unbounded), each clause's certificate
  naming its engine and semantics; the injectivity lemma discharges at
  `@bv64` with no proof block. (REQ-2)
- [ ] AC-3: A planted non-injective rotate dies as `Counterexample`
  with the bit pattern in the certificate; an over-budget multiplier
  query yields `Timeout`, never `unknown` and never a silent downgrade.
  (REQ-2)
- [ ] AC-4: `bv_shadow` appears in the oracle subset for tagged
  clauses; `grep`ing certificates for `bv_shadow` finds every tagged
  clause and nothing else; `forge review` lists them. (REQ-3, REQ-6)
- [ ] AC-5: A wrap-exploiting mutant (one that is equivalent at
  unbounded semantics but distinguishable at width) is killed by the
  bv-semantics mutation run on a tagged clause — a fixture test.
  (REQ-4)
- [ ] AC-6: A `@bv64(nowrap)` clause whose body can overflow fails its
  side obligation with a concrete overflowing input; the obligation's
  verdict is recorded in `bv_shadow.nowrap_obligation`. (REQ-5)
- [ ] AC-7: The review section reports bv-shadow density per module
  and tower depth for burned lemmas; the numbers match a fixture
  project's known counts. (REQ-6)
- [ ] AC-8: A reconstruction-supported clause's certificate shows the
  kernel-checked `trust:` form while an unsupported clause on the same
  item retains `solver(z3)`; the audit's residual-trust statement
  aggregates the split. (REQ-7)
- [ ] AC-9: All G3 items hold in one CI run before any README/docs
  claim about `@bv` or default reconstruction merges. (REQ-8)

## Architecture

**Where stage 3 plugs in (recorded now; re-verify at re-pass):**

- **Syntax**: the tag is a clause-level annotation through
  `parse_contract`'s ordered dispatch (`thermite-syntax/src/parser.rs`)
  — the same seam stage 1 used for new clause forms. The build-flag
  gate wants the parser to consult a compiled-in capability, which is
  new for `thermite-syntax` (today it has no feature flags); the
  re-pass should pick the mechanism (cargo feature vs. a capability
  struct threaded from forge) with stage-1's final crate layout in
  view.
- **Lowering + engine**: a bv route through the existing `Engine`
  trait (`forge/src/engine.rs`) or a mode flag on the Verus engine —
  the re-pass decides after seeing how stage 1's relax engine
  (`nlsat`) landed; the certificate attribution mechanism
  (`with_engine_attribution`) carries either way.
- **Mutation at width**: `forge/src/mutation.rs`'s catalogue is reused
  unchanged (the stage-1 re-elaboration precedent: only the kill check
  swaps); the bv kill check evaluates the mutant against the tagged
  clause at width.
- **Certificates**: `bv_shadow` is an additive schema-v2 field;
  oracle-inclusion follows the stage-1 Q-ORACLE precedent
  (deterministic, verdict-relevant → included).
- **Reconstruction**: the cvc5/lean-smt replay path; the lean-smt SHA
  pin from the pre-M1 debt items is the dependency anchor. All
  reconstruction work is trust-field-only — no rung changes, no new
  verdicts — which is why F-J (don't ship it) is free.

**Why last, restated from the decision record:** the bv tag is the one
piece of the program that adds a *permanent* semantic fork and a
standing Goodhart vector; C3′ over C3 was precisely the decision to
stage it after the pure-win routing (stage 1) and the metatheory-heavy
cage growth (stage 2), with the locks shipping inside the same gate as
the feature, not after.

## Open Questions

*(Deliberately unresolved — keyed to inputs that cannot exist before
G2. Resolve at the re-pass, not before.)*

<!-- OPEN: Q-BVSCOPE -->
### Q-BVSCOPE: Does `@bv` ship full, `nowrap`-only, or lemma-position-only?

Fallback F-F's ladder (full tag → `nowrap`-only → lemma-position-only →
drop) is selected by evidence that won't exist until stages 1–2 run:
shadow-flag density telemetry from `forge review`, and whether any
wrap-weakened clause shows up in practice. RFC-1 commits to the full
tag with three locks; this doc specs that, but the re-pass must
re-affirm it against the telemetry or retreat a rung before any
implementation issue opens.
**To resolve**: re-run the design pass with the §6 dashboard's bv-risk
rows in hand.
<!-- /OPEN -->

<!-- OPEN: Q-RECON -->
### Q-RECON: Reconstruction engine and fragment support — what does the ecosystem look like at G2?

The replay path (lean-smt maturity, cvc5 proof formats, which of the
cage's fragments — linear arithmetic, EPR-stratified, QF_BV — have
practical reconstruction) is an empirical question about external
tooling at G2 time, roughly two gates away. The pre-M1 lean-smt SHA pin
fixes the dependency but not the answer. F-J makes failure free, so
this question never blocks the bv half — but default-on's fragment
check (REQ-7) cannot be specified until the assessment is
done.
**To resolve**: re-run the design pass with a fresh lean-smt/cvc5
replay assessment as an exploration input.
<!-- /OPEN -->

## Out of Scope

- `@bv` as a default or file-level mode — non-goal, permanently
  per-clause and loud.
- Float/transcendental clause modes (W8 Richardson) — non-goal.
- Verified extraction/erasure — the named L3 trust residual; a future
  program.
- Any cage-fragment change — S₂ widenings are stage-2-lineage RFC
  deltas, not stage-3 work.
- Reconstruction as a gate dependency for anything — F-J is explicitly
  free; no stage gate cites it.

---

*Stage-3 spec (PROVISIONAL — reasoning cache; re-run /design before
kickoff) · child of `.design/thermite2-program.md` (REQ-10) · sources:
RFC-1 §4/§10/§12, Appendix A §1.4/F-F/F-J · gate: G3 · baseline
`dollspace-gay/Thermite @ c46da3ac` (re-ground at re-pass).*
