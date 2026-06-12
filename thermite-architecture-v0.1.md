---
title: "Thermite v0.1 Architecture Decisions"
tags: ["thermite", "architecture"]
sources: []
contributors: ["Yffe"]
created: 2026-06-04
updated: 2026-06-12
---


Source: thermite-design.md (v0.1 draft). Decisions made 2026-06-04 during roadmap planning.

## Locked decisions
1. Verification path = transpile to Verus source. Thermite .th -> parse to AST (stable block IDs) -> emit Verus-annotated Rust (req->requires, ens->ensures, inv->invariant, dec->decreases, spec fn -> Verus spec fn) -> shell out to Verus/Z3 -> parse into structured JSON. NO MIR-level lowering in v0.1 (design mentions it but it is far larger; transpile is the MVP).
2. Goal-state REPL is whole-item in v0.1. Ship `forge check <item>` returning per-obligation results + counterexamples. Lean-style incremental holes (forge goal/fill, design 5.1) DEFERRED — Verus verifies whole functions; holes must be simulated atop whole-item re-verify + obligation diff.
3. Effects: compile-time subsumption only in v0.1. fx row checked at compile time (caller subsumes callee). Runtime syscall sandbox (design 4.1) DEFERRED — seccomp/OS-level subsystem.

## Deferred (tracked in issue #21)
- Runtime effect sandbox (seccomp-style syscall kill).
- True MIR-level lowering.
- Incremental goal-state holes (forge goal/fill).

## v0.1 critical path
#1 scaffold + #2 grammar/combinator registry (parallel entry points)
  -> #3 parser (stable addressing loop#1.inv#2, per-item recovery)
    -> #4 lowering to Verus (L3) + L1 runtime-check fallback
      -> #5 forge CLI (new/check, structured JSON, counterexamples)
        -> #8 reproducibility (pinned seeds) + per-item content-addressed proof cache
#3 -> #6 structural vacuity triage + #[slag]
#2 -> #7 skill generator + CI 6k-token budget gate

## Pillars to check work against
- Verification is the floor; #[slag] is the loud exception.
- Whole language fits in <=6000 tokens (THERMITE.skill.md, enforced in CI).
- One way to do everything; zero-config formatter.
- Locality: per-item parse/check/cache.
- The contract is the interface; the certificate/manifest is the deliverable.


## Addendum — state as of 2026-06-12 (#263)

Everything in the original 'Deferred' list except MIR-level lowering has SHIPPED:
- Runtime effect sandbox: live — forge build derives a seccomp-BPF filter from the fx row (.design/forge/runtime-sandbox.md); --target kernel emits freestanding rlibs.
- Incremental goal-state holes: live — forge goal/fill/edit over ?N holes (.design/forge/goal-repl.md); a holed item never certifies.
- MIR-level lowering: still deferred (#21); transpile-to-Verus remains the architecture.

Major layers the v0.1 snapshot predates:
- The verified primitive basis (Stages 1-8 + C1-C12): ADTs, recursion schemes, bounded collections/strings/Map, Option/Result, ergonomics (.design/basis/).
- Translation validation + the Lean proof spine: per-run independent-encoder TV (forge tv/exec-tv/body-tv) + the kernel-checked lowering_faithful theorem (lean/, .design/verified/).
- Proof backends (the second engine): backend-neutral Obligation + Engine interface; Verus is engine #1, Lean engine #2 via forge check --engine verus|lean|auto; per-obligation {engine, trust_profile} attribution; Proven-vs-Refuted disagreement is a hard SoundnessAlarm; interactive Lean proofs replay via canonical reconstruction (the #248-#252 injection arc closed). Exportable fragment today: pure contracts + straight-line bodies; while-loops are the named next increment (.design/verified/proof-backends.md, RATIONALE.md 'Proof backends').
- Self-verification: the soundness-critical pure core is Verus-verified (thermite-verified; epic #60 holds the remaining Tier-1 ports).
- Doc-drift tripwire: every routed .design doc pins an audited-sha; tooling/doc-drift.py + make doc-drift fail on staleness (#258; backlog work-off #262).

Roadmap v0.1->v0.5: all five milestones closed. The four proven example programs (editor, formatter, calculator, CSV parser) are live in examples/.
