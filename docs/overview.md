# Overview

## The problem

When an AI writes code, the question is whether it is correct. The usual
answers are "read it yourself" or "trust that the tests pass." Reading does not
scale — human review is the bottleneck AI was supposed to remove — and passing
tests only cover the cases someone thought to test.

Formal verification — mathematically proving code correct — has existed for
decades. It never caught on because writing the proofs is expensive for humans,
who pay for it in attention. Agents pay in compute, which is cheap and parallel.
Thermite's premise is that this changes the economics of proof: spend the cheap
resource (tokens) to buy the expensive one (trust).

Thermite is therefore strict in a way no human would tolerate, because humans
are not the intended author. People decide what the software should do and read
the resulting certificates; the agent writes the proofs.

## The assurance ladder

Each contract clause is graded on a five-rung ladder. `forge`, the toolchain,
aims for the top rung and records where each clause landed.

| Rung | Meaning |
|---|---|
| **L4** | A kernel-grounded proof. The nonlinear-arithmetic route combines a Z3 nlsat result with Lean-checked soundness lemmas that connect the real relaxation back to integer semantics. |
| **L3** | A machine-checked proof that the clause holds for every input. (SMT-backed deductive verification via the Verus prover and the Z3 solver.) |
| **L2** | Proven for all inputs up to a stated size. (Bounded model checking, via Kani/CBMC.) |
| **L1** | Checked while the program runs; a violation stops it. (Runtime contract monitoring.) |
| **L0** | Trusted by fiat — the `#[slag]` escape hatch, greppable so the trusted lines are enumerable. |

A function's level is the minimum over its clauses. A counterexample — a
concrete input where a clause fails — is a hard failure; it is never recorded
as a lower grade.

[Verification](verification.md) explains how the upper rungs are discharged and
re-checked. [Trust](trust.md) lists what remains trusted after a clean run.
