# Checked solver replay

Status: shipped for QF_LIA, the Stage 3 QF_BV surface, and admitted S₂.0
relation/array clauses through the Stage 4 finite-ground replay.

## Purpose

The Lean spine proves general lowering theorems, but a per-run solver still
decides concrete obligations. Checked replay reduces that remaining trust: the
solver finds the result, then Lean checks the theorem the result asserts.

For clause certification, that theorem is:

```lean
theorem generated (variables...) : semantic_req → query_clause := by
  ...
```

The route determines those two expressions. QF_BV grounds `result` with the
body used in its direct query. QF_LIA quantifies `result` and includes the
unsigned-domain guards from its nlsat input.

Production/reference equivalence theorems are still useful translation audits.
They do not prove that a clause is valid and therefore cannot change its trust
profile.

## Current coverage

| Fragment | Lean representation | Checker |
|---|---|---|
| QF_LIA | `Int` arithmetic and propositions | `omega` |
| QF_BV | literal `BitVec N` terms for N = 8, 16, 32, 64 | axiom-clean LRAT, Lean automation, and proved lemmas |
| admitted S₂.0 relations, functions, and sequences | typed finite grounding and theory closure | pinned CaDiCaL proof search + kernel-checked LRAT |
| formulas rejected by S₂.0, or QF leaves outside QF_LIA/QF_BV | outside checked reconstruction | named fallback or failure |

The QF_BV exporter covers wrapping addition, subtraction, and multiplication;
unsigned division and remainder; bitwise not, and, or, and xor; logical and
right shifts; unsigned comparisons; equality; inequality; and boolean
connectives.

## The production check

`forge/src/lean_smt_export.rs` performs the replay:

1. Render the solver route's validity theorem from the same source AST and
   domain assumptions.
2. Run Lean through the pinned Lake environment.
3. Require Lean to accept the theorem.
4. Parse the theorem's anchored `#print axioms` report.
5. Accept only `{propext, Classical.choice, Quot.sound}`.

For scalar replay, only a `ReconstructionOutcome::Checked` result changes the
clause's trust profile. The other outcomes preserve useful distinctions:

- `Unsupported`: the expression is outside the fragment.
- `Unavailable`: Lean or the pinned package could not run.
- `Failed`: Lean rejected the theorem or its axiom report.

Each checked certificate stores:

- the generated theorem name;
- the successful checker;
- the full generated-source SHA-256;
- the fragment;
- the validated axiom list;
- the SHA-256 of the exact solver input when the route exposes it.

The query hash prevents evidence for a separately rendered approximation from
being presented as evidence for the solver query. The remaining
Rust-to-SMT/Rust-to-Lean correspondence is inspection-tier and stays in the
trust statement.

Stage 4 extends that evidence with the canonical S₂.0 IR, source clause,
ground universe, instantiation, theory, propositional problem, CNF, LRAT,
counts, elapsed time, budget result, and a verdict key covering every
verdict-determining hash. Cached artifacts are recomputed and replayed before
they can change trust.

## QF_BV reconstruction

Lean's `bv_decide` obtains and checks an LRAT certificate, then uses native
evaluation for the final Boolean check. That adds a generated-code evaluation
axiom, so Thermite does not use it for trust migration.

`lean/Thermite/Reconstruct.lean` reuses Lean's bit-blaster, SAT solver, LRAT
parser, and proved LRAT soundness theorem. Its `bv_reconstruct` tactic has the
kernel reduce the certificate check. Successful theorems therefore stay within
the standard axiom allowlist.

Lean 4.29's call-by-value reducer can reject some repeated-subterm circuits
while folding internal projection terms. Reconstruction then tries ordinary
kernel-checked automation or an applicable proved lemma. The certificate names
the path that succeeded; a failed LRAT attempt is never labeled as LRAT
evidence.

`lean/Thermite/PinReconstruction.lean` keeps a permanent 64-bit theorem on the
LRAT path. `scripts/lean-axiom-probe.sh` builds it and checks its axiom report.

## History

`lean/Thermite/SmtDemo.lean` established the first QF_LIA proof of concept by
re-discharge through Lean-SMT/cvc5. It showed that the scalar obligations could
be reconstructed with the standard Lean axioms.

Stage 3 then added two distinct production pieces:

- a translation-equivalence exporter for auditing the production and reference
  encodings;
- a validity exporter and replay path for clause trust migration.

The second piece is the one certification uses. QF_LIA now uses `omega`, and
QF_BV uses literal `BitVec` proofs rather than Lean-SMT's partial BitVec
reconstructor.

## Trust statement

For a checked QF_LIA, QF_BV, or admitted S₂.0 clause, the trust base contains
the Lean kernel, the allowed standard axioms, and the inspection-tier renderer
correspondence. Z3 and CaDiCaL are not needed for the truth of the checked
clause theorem.

If replay is unsupported, unavailable, or unsuccessful, the existing solver
trust remains. The project audit names those clauses. It reports zero S₂.0
relation/array residuals and lists only formulas outside S₂.0 or its checked
QF_LIA/QF_BV leaves as unsupported fragments.

This is per-clause trust reduction. It does not claim that every Verus VC,
translation-validation theorem, or quantified formula has been reconstructed.

## Verification

```sh
cargo test -p forge --features bv --bin forge lean_smt_export::tests -- --nocapture
cargo test -p forge --features bv --test bv_lowering -- --nocapture
cd lean && lake build Thermite.PinReconstruction
bash scripts/lean-axiom-probe.sh
bash scripts/g3-gate.sh
```
