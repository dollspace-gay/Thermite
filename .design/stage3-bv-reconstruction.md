# Stage 3: fixed-width clauses and checked reconstruction

Status: Gate G3 implemented. The gate command is `bash scripts/g3-gate.sh`.

Stage 3 adds an explicit machine-arithmetic mode and reduces solver trust for
the fragments Lean can check. The two changes meet on QF_BV:

- `ens@bvN` and `inv@bvN` use unsigned `N`-bit semantics for
  `N ∈ {8, 16, 32, 64}`.
- Successful QF_LIA and QF_BV solver verdicts are replayed as the actual Lean
  theorem `req → clause`.

The feature is per clause. Untagged clauses retain their existing unbounded
semantics.

## What ships

### The `@bv` tag

`ens@bvN P` interprets `P` over unsigned `N`-bit values. Arithmetic wraps,
division and remainder use the SMT-LIB zero-divisor cases, and shifts by at
least the width return zero.

`inv@bvN P` uses the same values and operations inside the ordinary Verus
invariant VC. This keeps invariant initiation and preservation at L3 while
making the arithmetic meaning match the tag.

`@bvN(nowrap)` keeps the fixed-width domain and adds a no-overflow obligation.
A counterexample, timeout, unavailable solver, or otherwise undecided nowrap
check withholds certification.

Preconditions are inputs to tagged postconditions and are interpreted at that
postcondition's width. They do not carry their own `@bv` tag.

### Three visibility and quality locks

Every accepted tag has three accompanying checks:

1. A `bv_shadow` certificate record names the width, wrap mode, and nowrap
   result.
2. The mutation battery evaluates result-bearing clauses at their tagged
   width.
3. `nowrap` has a separate fail-closed side obligation.

`forge review` and `forge audit` report tagged-clause density, burned-lemma
tower depth, and the remaining solver-trusted clauses. The density warning is
the retreat signal if fixed-width contracts become too common to review
comfortably.

### Checked reconstruction

Solver success alone does not move trust. Forge generates a Lean module
containing the actual validity theorem:

```lean
theorem generated (variables...) : semantic_req → query_clause := by
  ...
```

For QF_BV, `query_clause` has `result` replaced by the body expression. For
QF_LIA, `result` remains a quantified solver variable and `semantic_req`
includes the unsigned-domain guards from the nlsat input.

The clause moves to kernel trust only when all of the following hold:

- Lean accepts the theorem.
- The theorem's anchored `#print axioms` report is a subset of
  `{propext, Classical.choice, Quot.sound}`.
- The certificate records the theorem name, checker, full generated-source
  SHA-256, fragment, and axiom list.
- When the solver route exposes its input, evidence records that input's SHA-256.
  This covers the direct QF_BV query and the QF_LIA nlsat input.

QF_LIA uses Lean's verified `omega` procedure. QF_BV uses an axiom-clean
portfolio containing Thermite's proof-producing LRAT checker, Lean
automation, and proved library lemmas. The evidence names the successful
path. `bv_decide` is not used because its native evaluation adds an axiom to
the theorem.

The older production/reference equivalence exporter remains an inspection
tool. Its theorem is useful for auditing the emitters, but it is not validity
evidence and cannot change a certificate's trust.

Unsupported expressions, a missing Lean installation, a failed theorem, or a
disallowed axiom leave the clause solver-trusted. The audit lists those
clauses. At Gate G3, EPR-stratified relation and array atoms were still outside
this reconstruction fragment. Gate G4 now handles the admitted S₂.0
relation/array fragment; see `stage4-epr-reconstruction.md`.

## Requirements

- **REQ-1 — syntax gate.** A release build without the `bv` feature rejects
  the tag with `BvTagWithoutShadowPlumbing`. A feature-on build accepts all
  four widths, `nowrap`, tagged postconditions, tagged invariants, and tagged
  lemma conclusions.
- **REQ-2 — fixed-width route.** Tagged postconditions and lemma conclusions
  are decided as QF_BV by `EngineName::BitVector`. Untagged clauses keep their
  previous route. The 64-bit multiplication cliff has its own timeout profile.
- **REQ-3 — shadow record.** Every tagged clause or invariant contributes a
  `bv_shadow` record to its certificate and audit views.
- **REQ-4 — width-aware mutation.** The existing mutation catalogue is checked
  at the clause width. Undecided mutants are not counted as kills.
- **REQ-5 — nowrap.** `@bvN(nowrap)` emits a separate no-overflow check and
  records its result. Any undecided or false result withholds certification.
- **REQ-6 — review surface.** Review and audit show tag density, tower depth,
  and the density warning.
- **REQ-7 — Lean export.** Forge can render QF_LIA and the full shipped QF_BV
  term surface as Lean propositions.
- **REQ-8 — default checked replay.** A solver-proved QF_LIA or QF_BV clause is
  replayed by default as its actual `req → clause` theorem. Trust changes only
  after successful axiom validation, and the certificate carries the replay
  evidence.
- **REQ-9 — Gate G3.** One CI job runs both release parser configurations,
  fixed-width lowering, tagged invariant checking, live clause
  reconstruction, the full bit-vector conformance suite, and the Lean spine
  axiom probe. A missing dependency is a failure.

## Acceptance criteria

- [x] **AC-1:** feature-off and feature-on release parser tests cover the
  structured rejection, four widths, `nowrap`, invariants, and lemmas.
- [x] **AC-2:** `mix64` carries separate fixed-width and unbounded clause
  attributions; the rotate injectivity lemma needs no author proof block.
- [x] **AC-3:** a false shift property returns a concrete bit pattern and the
  bounded multiplier path returns `Timeout`, never an unlabeled unknown.
- [x] **AC-4:** `bv_shadow` is present for every accepted tagged clause and
  invariant and absent from untagged clauses.
- [x] **AC-5:** a wrap-sensitive mutant is killed at width while its unbounded
  counterpart survives.
- [x] **AC-6:** a false or undecided nowrap obligation rejects the certificate
  and records the reason or overflowing input.
- [x] **AC-7:** audit fixtures pin tag density, tower depth, and the density
  warning.
- [x] **AC-8:** live tests check true and false QF_LIA goals and the complete
  QF_BV operator surface. Successful theorems have only the allowed axioms.
- [x] **AC-9:** certificates contain checked evidence for the actual validity
  theorem; failed, unavailable, unsafe, and unsupported replays do not migrate
  trust. The audit names the residual solver trust.
- [x] **AC-10:** `scripts/g3-gate.sh` runs all G3 checks in one CI job.

## Evidence map

| Concern | Implementation and tests |
|---|---|
| syntax gate | `thermite-syntax/src/parser.rs`, `thermite-syntax/tests/bv_tag_parse.rs` |
| fixed-width solver | `forge/src/bitvector.rs`, `forge/tests/bv_lowering.rs` |
| certificate and audit visibility | `forge/src/manifest.rs`, `forge/src/audit.rs`, `forge/src/forks.rs` |
| mutation and nowrap | `forge/src/check.rs`, `forge/tests/bv_lowering.rs` |
| invariant semantics | `thermite-lower/src/lower.rs`, `forge/tests/bv_invariants.rs` |
| validity export and replay | `forge/src/lean_smt_export.rs`, `lean/Thermite/Reconstruct.lean` |
| permanent axiom probe | `lean/Thermite/PinReconstruction.lean`, `scripts/lean-axiom-probe.sh` |
| combined gate | `scripts/g3-gate.sh`, `.github/workflows/ci.yml` |

## Residual trust and limits

Reconstruction reduces the trust base of a successful clause; it does not
change the clause's assurance rung. The generated Rust-to-Lean and Rust-to-SMT
renderers remain inspection-tier, so their correspondence is still audited by
source review and drift pins.

At Gate G3, quantified relation and array formulas outside QF_LIA/QF_BV stayed
solver-trusted. Gate G4 now reconstructs the admitted S₂.0 subset. Formulas the
S₂.0 classifier rejects, and quantifier-free leaves outside the checked
QF_LIA/QF_BV source surface, remain visible out-of-fragment cases.

File-level `@bv` modes and floating-point clause modes remain outside Stage 3.
Full S₂.0 relation/array reconstruction is the separate Stage 4 result.
