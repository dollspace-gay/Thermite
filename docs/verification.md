# Verification

## What the ladder rungs mean

L3, the top rung, is a machine-checked proof that a clause holds for every
input, discharged by the Verus prover emitting verification conditions to the
Z3 solver. L2 is bounded model checking (Kani/CBMC) up to a stated size. L1 is
runtime contract monitoring. L0 is a trusted-by-fiat `#[slag]` annotation.
[Overview](overview.md) has the full table.

## Grading the contract, not only the proof

A clause that asserts nothing (`ens true`) would pass trivially. Every contract
therefore goes through an anti-Goodhart battery: it is checked for emptiness
(vacuity detection), then run against mutant copies of the code that introduce
bugs (mutation testing). A contract that fails to catch the mutants is rejected.
Mandatory contracts are the precondition for this: a tool that grades contract
strength is meaningless if a function can score by declaring no contract.

## Translation validation

Thermite translates source into the prover's language, so a faulty translator
could prove the wrong statement. Two mechanisms check the translation, both
machine-checked:

1. **Per run.** A second, independent translator (kept from sharing code with
   the first by the build system) re-translates the contracts and bodies, and
   Z3 must prove the two translations equivalent on your program.
2. **Across all programs.** That independent translator is small enough to prove
   correct in Lean. The theorem (`lean/`, `Thermite.lowering_faithful`) states
   that every program passing the cross-check is translated meaning-for-meaning;
   it is quantified over all programs and re-checkable by your own Lean kernel
   (audit check [1] in [Trust](trust.md)). Each translation bug previously found
   by testing is refuted by that theorem.

This is the verified-validator architecture from the compiler-verification
literature (the CompCert lineage). Thermite's meaning is defined by the Lean
semantics; Verus is the first proof engine against it, proven faithful.

## Two proof engines

L3 obligations are discharged by Verus by default, or by Lean under
`forge check --engine lean|auto`. The Lean path is kernel-checked, with a replay
that rejects `sorry` and non-standard axioms. Each certificate records which
engine proved each obligation and under which trust assumptions. If the two
engines disagree on the same obligation — one proves it, the other produces a
counterexample — `forge` halts with a soundness alarm rather than resolving the
disagreement by preference.
