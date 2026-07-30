# Stage 4: checked reconstruction for the stratified cage

Status: implementation in progress. The completion command will be
`bash scripts/g4-gate.sh`.

Stage 2 proved the shape of the stratified encoder, but left relation and
array-property atoms interpreted by the solver. Stage 3 added checked replay for
QF_LIA and QF_BV. Stage 4 closes the remaining S₂.0 trust gap: every formula
accepted by the current stratified classifier must either produce a genuine
countermodel or a kernel-checked proof of its actual `req → clause` theorem.

The external solver may search for a proof, but it is not trusted. Lean rebuilds
the finite grounding and CNF, checks an LRAT certificate, and derives the clause
theorem. `scripts/install-g4-tools.sh` builds CaDiCaL 2.1.3 and drat-trim at
commit `effa1dcce85c878236f8313133dff1a2b766cd7c`; the gate accepts only that
toolchain and never falls back to an arbitrary system executable.

## Scope

`S2Recon` is exactly the current S₂.0 admission language:

- formulas: atoms, negation, conjunction, disjunction, implication, and sorted
  universal and existential binders;
- terms: bound variables, named constants, valued literals, sequence reads and
  lengths, width-preserving casts, index offsets, admitted multiplication, and
  unary spec-function applications;
- relations: equality, inequality, and the four ordered comparisons;
- embedded quantifier-free atoms, discharged through the existing QF_LIA and
  QF_BV replay paths.

An admitted formula returning `Unsupported` is a gate failure. Later fragment
versions may add sequence-sort binders, nested sequences, floating point, or
higher-order and recursive propositions; those are not S₂.0.

## Requirements

### REQ-1 — one canonical clause representation

Translate real `thermite_syntax::Expr` clauses into the classifier language.
The translation carries:

- actual literal values and stable names for free constants;
- binder sorts and de Bruijn indices;
- source item and clause addresses;
- declared function signatures;
- array, length, cast, and index operations.

Its wire form is deterministic. Classification, SMT emission, Lean replay,
certificate hashing, diagnostics, and drift checks consume this representation.

### REQ-2 — typed relation and array semantics

Lean defines sort-indexed carriers and interpretations for constants, relations,
unary functions, sequences, reads, and lengths. `evalTm`, `evalAtom`, and
`evalFrm` are total on well-sorted S₂.0 syntax.

`strat_lowering_faithful` is strengthened so relation atoms use this semantics
instead of an unconstrained Boolean oracle.

### REQ-3 — checked normalization and Skolemization

NNF, prenex conversion, substitution, and Skolemization preserve the relevant
meaning or satisfiability statement. The reconstruction polarity is the
negation of the actual validity query: `req ∧ ¬clause`.

### REQ-4 — finite grounding

Lean builds a sort-indexed ground universe from constants and Skolem terms,
closing under admitted functions in sort-graph order. Acyclicity proves
termination. Exhaustive instantiation is equisatisfiable with every admitted
S₂.0 formula.

### REQ-5 — checked ground theory

The ground formula includes justified clauses for equality, relation and
function congruence, reads, lengths, supported array extensionality, casts, and
index operations. Arithmetic leaves use the QF_LIA or QF_BV checker. Every
theory lemma is proved in Lean or has its own checked replay evidence.

### REQ-6 — CNF and LRAT

Lean recomputes a Tseitin CNF from the grounded formula and proves the CNF
correspondence. A pinned proof-producing SAT solver emits LRAT. The existing
kernel LRAT checker derives unsatisfiability and therefore `req → clause`.

Missing Lean, SAT, or certificate tooling is a failure, never a skip.

### REQ-7 — complete evidence and cache keys

Reconstruction evidence records the source, canonical IR, solver query, ground
universe, CNF, and LRAT hashes; fragment version; instantiation and theory-clause
counts; theorem and checker; axiom report; elapsed time; and budget result.

Every verdict-determining field participates in the proof-cache key.

### REQ-8 — automatic production routing

Normal `forge check` classifies each real clause. Admitted S₂.0 clauses route
through reconstruction by default and certify at L4 only after successful
kernel replay. False clauses return concrete finite models. Timeouts and replay
failures remain named failures and never migrate trust.

The `@bv` parser plumbing is enabled in normal release builds, and a tagged
clause automatically selects the bit-vector route. Explicit engine flags remain
diagnostic overrides.

### REQ-9 — usable repair and forge surfaces

`forge edit <file> --restratify <clause> --witness <name>` edits the selected
source clause, adds the witness definition, emits the implication side
obligation, and certifies only after the side obligation passes.

Forge definitions used by proofs, including `prop fn`, have a real Lean
definition/export path. Product documentation states any intentionally
unsupported refinement surface explicitly.

### REQ-10 — Gate G4

One fail-fast gate covers the complete S₂.0 constructor inventory, true and
false formulas, malformed or tampered evidence, missing dependencies, generated
Rust/Lean/solver differential tests, the axiom allowlist, and the absence of
`sorry`, `admit`, custom axioms, and `native_decide`.

## Acceptance criteria

- [ ] Source clauses translate deterministically to a well-sorted canonical IR,
  and every source construct in S₂.0 has a positive and a refusal test.
- [ ] Relation and array atoms have typed Lean semantics and no longer pass
  through a free `relModel`.
- [ ] Normalization, Skolemization, and grounding theorems are axiom-clean and
  have negative pins for polarity, capture, dependency omission, and empty
  carriers.
- [ ] Every admitted corpus clause reconstructs or returns a checked
  countermodel; none is unsupported.
- [ ] Lean rebuilds the CNF and accepts the exact `req → clause` theorem only
  after LRAT verification.
- [ ] Equality, congruence, reads, lengths, casts, index offsets, and mixed
  QF_LIA/QF_BV leaves have end-to-end tests.
- [ ] Certificate tampering at every recorded hash boundary fails.
- [ ] Normal release builds accept `@bv`, and ordinary `forge check` performs
  clause routing without an engine flag.
- [ ] Restratification operates on a selected source clause rather than the
  built-in demonstration.
- [ ] `bash scripts/g4-gate.sh`, the workspace tests, Lean build and axiom probe,
  audit, drift checks, and requirement-registry checks all pass with no skipped
  dependency.

## Residual trust after G4

The Lean kernel, its standard axiom set, the Rust-to-Lean source correspondence,
and the compiler toolchain remain visible trust boundaries. The SAT solver and
SMT solver do not remain in the trust line of a successfully reconstructed
S₂.0 clause.

Formulas rejected by S₂.0 remain forge-routed with their classifier reason.
Fragment widenings require a new grammar, semantics, pins, and gate.
