# Formal-methods state of the art — context for Thermite's lowering-soundness architecture

<!--
tier: research
status: survey (deep-research, 2026-06-09)
purpose: contextualize Thermite's step-3 ("lowering faithfulness") architecture in the
         formal-methods SOTA, and correct lexical drift before committing the design.
method: fan-out web search (5 angles) -> 26 primary sources -> 120 claims ->
        25 verified by 3-vote adversarial verification (2/3 refutes to kill) ->
        25 confirmed, 0 killed -> 8 synthesized findings. Issue #175 / epic #169.
-->

## Headline

The technique Thermite needs for "lowering faithfulness" is **well-established**, and the
field gives it two precise framings. We are not reinventing — we have been *naming it
wrong*. The corrective: state the work in the field's vocabulary (**semantic
preservation**, **forward simulation**, `S ≈ C`), and be explicit that an L3 proof
**currently trusts Z3** unless we adopt proof reconstruction.

All findings below carry unanimous (3-0) adversarial-verification votes against the cited
primary sources.

## Findings

1. **"Lowering faithfulness" IS the field's "semantic preservation" for a source→target
   translation.** Two established ways to establish it: *verify-the-compiler* (a universal
   theorem, CompCert) vs *translation validation* (a per-run check, Pnueli/Siegel/Singerman
   1998; Necula's GCC validator 2000; Alive2 for LLVM). Leroy proves a **verified validator
   composed with an unverified compiler is as strong as a verified compiler — "provided the
   validator is smaller and simpler than the compiler."** This is exactly Thermite's
   "existential→universal" axis: a per-run validator is the local/existential guarantee; a
   once-for-all metatheorem (or a *verified* validator) is the universal one. **Critical
   nuance: only a *verified* (proven-sound) validator gives the strong guarantee** — an
   unverified validator could itself be wrong. (This is precisely our soundness theorem T1:
   the reference encoder must be proven sound.)
   — Leroy CACM (compcert-CACM.pdf); Pnueli/Siegel/Singerman (BFb0054170).

2. **Semantic preservation is rigorously stated as a relaxed "non-going-wrong" property
   and proved by forward-simulation diagrams** — the exact vocabulary our theorem should
   adopt. CompCert proves, for deterministic languages, `∀B∉Wrong, S⇓B ⇒ C⇓B` (every
   non-going-wrong source behavior is a target behavior), "generally much easier to prove
   than" full equivalence "since the proof can proceed by induction on the execution of S."
   The proof is the **simulation diagram**: each source transition corresponds to target
   transitions with the same observable effects, preserving a binary relation `∼` between
   states; backward simulation is the converse (derivable when the target is deterministic).
   **For Thermite: the source→Verus theorem should be a forward simulation over a relation
   between Thermite states and the (Verus-annotated) Rust states, preserving observable
   effects — and the caged quantifier fragment + the effect rows define exactly what
   "observable" means.**
   — Leroy CACM.

3. **Verification never *eliminates* the trusted base; it reduces it to an enumerable set**
   — directly validating our "certificate (auditable artifact + enumerable trusted base)"
   framing, which should *cite CompCert*. Leroy enumerates CompCert's residual trust:
   (1) the formal semantics of source + target; (2) the unverified passes (parser, assembler,
   linker); (3) the extraction + OCaml runtime; (4) Coq itself. **He flags item (1) — the
   formal semantics — as the most delicate ("how can we make sure a formal semantics agrees
   with language standards and common practice?").** Thermite's analogous list: the Thermite
   operational semantics + the Verus/Rust target semantics, the unverified lowering passes,
   Z3 (unless reconstructed), Verus, rustc/build chain. **The mechanized operational
   semantics we plan to write is the single most delicate item — its agreement with the
   *intended* meaning of Thermite is itself an unprovable-from-within assumption.**
   — Leroy CACM.

4. **Translation validation is cheap to build (≈ one compiler pass of effort), needs no
   compiler cooperation, and SMT-backed *bounded* TV (Alive2) is the modern SOTA.** Necula's
   GCC validator compares IR before/after each pass with heuristics, no optimizer help,
   "about the effort... of one compiler pass." Alive2 is "fully automatic through an SMT
   solver... no changes to LLVM"; its boundedness (loop unrolling to a bound, "misses bugs"
   in some cases) is deliberate — "designed to avoid false alarms" (sound-for-reported-
   violations, incomplete). **This is exactly Thermite's L2 tier (Kani/CBMC bounded model
   check). Caveat: Necula's prototype had ~10% false-alarm rates and was not production —
   "cheap to build" ≠ "cheap to make sound and usable."**
   — Necula PLDI'00 (349299.349314); Alive2 PLDI'21 (3453483.3454030).

5. **End-to-end verified compilation of a *real* general-purpose language is achievable
   (CakeML, in HOL4) — but only over a *fixed subset*,** with one machine-checked theorem
   from source string to executing machine code. This is the existence proof that **a small,
   frozen source language can be carried to a universal correctness theorem — it endorses
   Thermite's decision to keep the language small and freeze the subset.**
   — CakeML POPL'14 (2535838.2535841).

6. **Standard toolchain: Ott/Sail author the *definitions*; a foundational proof assistant
   (Coq/Isabelle/HOL4) carries the *metatheory*.** Ott compiles one definition to LaTeX +
   Coq/HOL/Isabelle + OCaml, **but "is not itself a proof tool."** Sail emits emulators + Coq/
   Isabelle/HOL4 definitions; its models boot Linux/FreeBSD/seL4. **Implication: a tool can
   author the Thermite semantics and emit prover definitions, but the semantics-preservation
   proof itself must be done in Coq/Lean/Isabelle. This is a concrete buy-vs-build decision.**
   — Ott JFP (ott-jfp.pdf); Sail POPL'19 (sail-popl2019.pdf).

7. **Rust itself is formalized *foundationally* (Stacked Borrows + RustBelt/Iris in Coq),
   not via SMT** — the field reaches for foundational assistants + separation logic for
   real-language metatheory and effect/aliasing reasoning. Stacked Borrows is "an operational
   semantics for memory accesses in Rust [defining] an aliasing discipline," soundness
   mechanized in Coq; RustBelt is built on Iris, "a generic higher-order concurrent
   separation logic... in Coq." **Two load-bearing consequences for Thermite: (a) since we
   lower to *Rust*, our target semantics inherits Rust's aliasing/UB model — Stacked Borrows
   is the reference our "Verus-annotated Rust" target must be reconciled against; (b) the
   field's effect-reasoning tool is separation logic (Iris), which contrasts with — and does
   not combine — our static effect-rows + seccomp approach (see the "genuine extension" note).**
   — Stacked Borrows POPL'20 (rustbelt/stacked-borrows); Jung PhD thesis.

8. **The "trust Z3" problem is handled by proof-PRODUCING SMT + reconstruction:** the solver
   emits a proof that is replayed and re-checked by the proof assistant's kernel, so the
   solver is *not trusted*. Lean-SMT dispatches to cvc5 and "reconstructs SMT proofs into
   native Lean proofs... submitted to the Lean kernel" (coverage partial: ~30% of cvc5's
   proof rules today); cf. SMTCoq, Isabelle's Metis replay of Sledgehammer. **This is the
   missing piece between Thermite's L3 (which trusts Z3) and a foundational certificate.
   Verus/Z3 do NOT produce reconstructable proofs by default — so *today* an L3 certificate
   must enumerate Z3 (and Verus) in its trusted base.** The path to demote Z3 from *trusted*
   to *checked* exists, but is not free.
   — Lean-SMT (arXiv 2505.15796).

## Terminology map (drift / duplication / genuine extension)

| Thermite term | Field term | Verdict |
|---|---|---|
| "lowering faithfulness" | **semantic preservation** (`S ≈ C`), via **forward simulation** | DUPLICATES — restate in this vocabulary; cite CompCert/Leroy |
| L3 (SMT total-correctness over the caged fragment) | SMT-discharged verification; the cage = a decidability/automation lever | mostly standard; the *cage* is a genuine lever (below) |
| L2 (bounded model check) | bounded translation validation / BMC — "sound-for-reported-violations, incomplete" (Alive2, CBMC) | DUPLICATES — use the established guarantee phrasing |
| L1 (runtime contract checks) | runtime contract monitoring | DUPLICATES |
| L0 (`#[slag]`, trusted-by-fiat) | an enumerated trusted axiom — CompCert's trusted-base concept, made *per-function* | DUPLICATES the concept; the per-function granularity is a presentation choice |
| "certificate (auditable artifact + enumerable trusted base)" | CompCert's reduced-trusted-base framing | DUPLICATES — cite it |
| "translation validation as existential→universal" | the TV-vs-verified-compilation axis (Leroy: verified validator ≡ verified compiler) | CORRECTLY uses the field's axis |
| **"caged quantifier fragment"** (bounded combinators + frozen triggers) | a deliberate decidability/automation lever | **GENUINE EXTENSION** — no direct analogue in the surveyed verified-compilation lit; needs a targeted survey to confirm novelty |
| **"anti-Goodhart battery"** (mutation-kill-ratio + vacuity/tautology detection) | spec-quality / spec-mutation / vacuity detection | **GENUINE EXTENSION** — no analogue in the surveyed verified-compilation lit |
| **static effect-rows (`fx`) + seccomp confinement** | algebraic/row effects (Koka/Eff/Frank) + capability/sandbox confinement (seccomp/CHERI) — but the *hybrid* | **GENUINE EXTENSION** — the surveyed effect lit (Iris/separation logic) does not combine static effect typing with runtime syscall confinement |

## Architecture implications (for step 3, epic #169)

- **Adopt the vocabulary.** Rename/relate "lowering faithfulness" → *semantic preservation*;
  state the theorem as a **forward simulation** over a relation `∼` between Thermite states
  and Verus-Rust states, preserving observable effects. Cite Leroy for the trusted-base
  framing. This is the direct fix for the lexical-drift concern.
- **A verified *bounded validator* may suffice — and is far cheaper than a full universal
  simulation proof.** Leroy: a verified validator ≡ a verified compiler. Necula: ≈ one pass
  of effort. **Open architectural question: does L3 need a full mechanized-semantics +
  universal simulation proof, or does an Alive2-style *verified bounded validator* on each
  lowering run meet the threat model?** The latter is closer to what TV already is; the
  missing piece is *proving the validator (our reference encoder) sound* (T1) — which is the
  cheaper crux either way.
- **The source semantics is the delicate item** (finding 3). Its agreement with the
  *intended* meaning of Thermite is an unprovable-from-within assumption — this is the
  irreducible residue (Gödel), and it should be stated, not hidden.
- **The target inherits Rust's UB/aliasing model** (Stacked Borrows). The "Verus-annotated
  Rust" target semantics must be reconciled against it (Verus's own model already does much
  of this; the obligation is to not silently diverge from it).
- **The Z3-trust resolution is proof reconstruction** (Lean-SMT/SMTCoq). This is the path to
  demote Z3 from *trusted* to *kernel-checked*; today it is not in place, so L3 honestly
  enumerates Z3 + Verus. **This bears on the proof-assistant choice: Lean 4 has the live
  cvc5-reconstruction path (Lean-SMT).**

## Proof-assistant landscape (informs the open #173 fork)

The survey did not yield a single hard recommendation (an honest gap), but it maps what the
field *uses for this kind of work*:

- **Coq** — CompCert, RustBelt, Stacked Borrows, Iris, Vellvm. The proven home for verified
  compilation + Rust/effect metatheory. Strongest precedent for *exactly our task*.
- **Isabelle/HOL** — seL4 / L4.verified (the verified microkernel). The choice if the
  kernel direction dominates.
- **HOL4** — CakeML (end-to-end verified compiler over a frozen subset). Closest precedent
  for our "small frozen language → universal theorem."
- **Lean 4** — Mathlib + Lean-SMT (the cvc5 proof-reconstruction path). The modern choice and
  the one with the live route to *demoting Z3* — directly relevant to shrinking Thermite's
  trusted base.

## Honest gaps + open questions (the research could NOT confirm these)

1. **Thermite's central economic thesis — that AI agents make the historically-prohibitive
   annotation/proof burden affordable by paying it in compute — is UNVERIFIED by this
   evidence set** (the autoformalization angle, Lean Copilot/AlphaProof/Baldur/LLM-generated
   Verus, did not survive into confirmed claims). **It must be treated as an open, load-
   bearing assumption, not a supported finding.** It warrants its own targeted survey.
2. The SMT-verifier-vs-proof-assistant *trusted-base size* comparison (Dafny/F*/Viper-Boogie)
   was not confirmed beyond the Lean-SMT angle.
3. The row-effect-system literature (Koka/Eff/Frank/F* effects) and the seccomp/CHERI
   confinement comparison (part 5) were not confirmed — so the novelty of the
   effect-rows+seccomp *hybrid* is asserted-by-absence, needing its own survey.
4. **Does L3 need a faithfulness metatheorem at all, or does a verified bounded validator
   suffice?** (The cost asymmetry — one pass vs a full mechanized semantics — makes this the
   first thing to resolve before committing the architecture.)
5. **Can Verus/Z3 be made proof-producing/reconstructable** (à la Lean-SMT/SMTCoq) so an L3
   certificate demotes Z3 from trusted to kernel-checked, and at what coverage/latency cost?

## Primary sources (selected; all `quality: primary`)

- Leroy, *Formal verification of a realistic compiler* (CompCert), CACM — xavierleroy.org/publi/compcert-CACM.pdf
- Pnueli, Siegel, Singerman, *Translation Validation*, TACAS'98 — link.springer.com/chapter/10.1007/BFb0054170
- Necula, *Translation validation for an optimizing compiler*, PLDI'00 — dl.acm.org/doi/10.1145/349299.349314
- Lopes et al., *Alive2: Bounded Translation Validation for LLVM*, PLDI'21 — dl.acm.org/doi/10.1145/3453483.3454030
- Kumar et al., *CakeML: A Verified Implementation of ML*, POPL'14 — dl.acm.org/doi/10.1145/2535838.2535841
- Sewell et al., *Ott*, JFP — cl.cam.ac.uk/~pes20/ott/ott-jfp.pdf
- Armstrong et al., *Sail*, POPL'19 — cl.cam.ac.uk/~pes20/sail/sail-popl2019.pdf
- Jung et al., *Stacked Borrows*, POPL'20 — plv.mpi-sws.org/rustbelt/stacked-borrows/paper.pdf
- Jung, *Understanding and Evolving the Rust Programming Language* (PhD, Iris/RustBelt) — research.ralfj.de/phd/thesis-print.pdf
- *Lean-SMT* (cvc5 proof reconstruction), 2025 — arxiv.org/html/2505.15796
- (also surfaced: SMTCoq, the Verus paper, QuerySMT, CompCertM/compositional compcert, Iris affect, Alpha-Verus — see issue #175)
