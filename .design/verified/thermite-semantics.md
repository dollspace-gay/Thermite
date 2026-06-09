# Thermite Mechanized Semantics + the Semantic-Preservation Soundness Theorem (the keystone)

<!--
tier: 3-component
status: committed-architecture (prover + vocabulary + architecture LOCKED; increments unbuilt)
governs: lean/  (the Lean 4 development — created by the builder next dispatch, increment (a);
          no production .rs is added/changed by this doc — this is the meta-theorem layer.
          It is GROUNDED in the existing reference encoders
          thermite-tv/src/{ref_encode,exec_encode,exec_stmt_encode}.rs and the frozen
          thermite-spec/src/combinators.rs. The semantics S is mechanized in Lean 4
          (Mathlib + Lean-SMT) — DECIDED, see "Tooling: the committed prover".)
thesis-refs:
  - thermite-design.md §1 (trust relocated: code → spec → spec-intent; "a skeptical third party can audit in minutes")
  - thermite-design.md §4.1 (contract-first functions; the exec body they guard)
  - thermite-design.md §4.2 (the deliberately-weak SpecTherm sublanguage; the frozen combinator cage; spec fns are total/terminating/executable)
  - thermite-design.md §6 (the verification ladder; L3 = Verus-derived SMT proof; L1 bounded exec values)
  - thermite-design.md §13 (roadmap; the verified-microkernel convergence)
field-refs:                              # the corrective vocabulary (lexical-drift fix, issue #175)
  - formal-methods-sota.md finding #1/#2 (semantic preservation; forward simulation; Leroy/CompCert)
  - formal-methods-sota.md finding #3 (reduced-trusted-base; the source semantics is the delicate item)
  - formal-methods-sota.md finding #4 (bounded translation validation; Alive2; "sound-for-reported-violations, incomplete")
  - formal-methods-sota.md finding #8 (proof-PRODUCING SMT + reconstruction; Lean-SMT/cvc5; the Z3-demotion path)
epic: crosslink #169
prover-fork: #173 (RESOLVED — Lean 4 / Mathlib / Lean-SMT; see "Tooling: the committed prover")
increment-(a)-blocker: #170 (spec-sublanguage S_C + prove ref_contract_pred sound, in Lean — NEXT BUILD)
increment-(b)-blocker: #171 (exec-expression S_E + prove exec_ref_value sound, in Lean)
increment-(c)-blocker: #172 (exec-statement S_B + prove body_ref_state sound, in Lean)
increment-(d)-blocker: #174 (compose → whole-frozen-subset semantic-preservation theorem, in Lean)
research: #175 (the formal-methods SOTA survey + terminology map — the source of the field vocabulary below)
prior-arc:
  - .design/verified/contract-tv.md (#139 — the contract TV reference encoder R_C, SHIPPED + total on corpus)
  - .design/verified/exec-tv.md (#151 — the exec-expression TV reference encoder R_E, SHIPPED)
  - .design/verified/exec-stmt-tv.md (#158 — the straight-line body state-transformer R_B, SHIPPED; the EXISTING state-transformer semantics this doc UNIFIES)
-->

## Summary

This is Thermite's CompCert moment — and the formal-methods SOTA survey (issue #175) names it
precisely: the architecture is a **verified validator** in Leroy's sense (CompCert/CACM), and the
property is **semantic preservation** (`S ≈ C`), stated as a **forward simulation**. We were not
reinventing the technique; the survey corrects the lexical drift ("lowering faithfulness" → the
field's *semantic preservation* / *forward simulation*) and pins the cheap, load-bearing
architecture.

The lowering-soundness arc (epic #169) has a single open link: the production lowerer
(`thermite-lower`, thousands of lines) is checked PER RUN by translation validation (TV) — Z3
proves `lower(P) ⟺ R(P)` against a small independent reference encoder `R` — but that gives only
EXISTENTIAL evidence: *for the programs we ran, the lowering agreed with `R`*. It does not yet say
the reference `R` is itself CORRECT, so it does not yet say anything UNIVERSAL about meaning.

**The committed architecture (decided; resolves this doc's own open question):** a VERIFIED
VALIDATOR, NOT a verified lowerer, and NOT (necessarily) a full universal forward-simulation proof
of the whole translation. The survey's load-bearing insight (Leroy, finding #1; Necula PLDI'00) is
that **a verified validator composed with an unverified compiler is as strong as a verified
compiler, "provided the validator is smaller and simpler than the compiler," at ≈ one
compiler-pass of effort.** We ALREADY have the per-run validator: the TV reference encoder
(`ref_contract_pred` / `exec_ref_value` / `body_ref_state`) + the Z3 equivalence check. The missing
piece — and THE chosen target — is **proving the reference encoder SOUND against the mechanized
semantics `S` (the T1 obligation), in Lean 4.**

So this doc defines a mechanized semantics **S** — the formal meaning `⟦·⟧_S` of a Thermite
program — and states the **soundness theorem** that turns the per-run TV check into a universal
guarantee:

1. **(T1) — validator soundness.** Prove the SMALL reference encoders sound against `S`:
   `∀ P, ⟦R(P)⟧ = ⟦P⟧_S`. Tractable because `R` (`ref_contract_pred`, `exec_ref_value`,
   `body_ref_state` — all already SHIPPED in `thermite-tv`) is small, declarative, and INDEPENDENT
   of the production lowerer. This is the verified-validator obligation (Leroy, finding #1).
2. **(T2) — semantic preservation, stated as forward simulation.** Compose: the existing per-run
   check `[Z3 ⊢ lower(P) ⟺ R(P)]` together with **(T1)** yields, for ALL `P` passing TV,
   `⟦lower(P)⟧ = ⟦P⟧_S` — the lowering PRESERVES MEANING (`S ≈ C`), relative only to
   `{Z3 soundness, S being the intended meaning}`. The relation `∼` of the forward simulation, and
   what "observable" means, are DEFINED by the caged quantifier fragment + the effect rows
   (finding #2).

The key economy (Leroy, finding #1; Necula, finding #4): you verify the small reference encoder
against `S` (auditable, structural induction, ≈ one-pass effort), NOT the big production lowerer
(the verify-the-compiler path, intractable here). The full `∀`-proof is multi-cycle
(CompCert/seL4-class); this doc is HONEST about that boundary throughout.

This is the project's answer to the boss's "what does Thermite add over Verus": Verus verifies a
Rust program against an annotation; Thermite additionally proves **semantic preservation** from the
Thermite surface to that annotated Rust — a verified-validator meta-theorem Verus does not provide
for its own front end.

## Field vocabulary — the lexical-drift fix (issue #175; cite, do not reinvent)

The SOTA survey's central corrective: state the work in the field's established vocabulary, with
citations. This section is the authoritative term map; the rest of the doc uses the field terms.

| Thermite term (was) | Field term (now) | Citation |
|---|---|---|
| "lowering faithfulness" | **semantic preservation** (`S ≈ C`), established via a **forward simulation** | Leroy, *Formal verification of a realistic compiler* (CompCert), CACM (finding #1/#2) |
| "certificate + enumerable trusted base" | CompCert's **reduced-trusted-base** framing | Leroy CACM (finding #3) |
| "translation validation as existential→universal" | the **TV-vs-verified-compilation** axis; a *verified* validator ≡ a verified compiler | Leroy CACM; Pnueli/Siegel/Singerman, *Translation Validation*, TACAS'98; Necula, *Translation validation for an optimizing compiler*, PLDI'00 (findings #1/#4) |
| L3 (SMT total-correctness over the caged fragment) | SMT-discharged verification; the **cage = a decidability/automation lever** | finding #4; Verus/Z3 |
| L2 (bounded model check) | **bounded translation validation / BMC** — "sound-for-reported-violations, incomplete" | Lopes et al., *Alive2*, PLDI'21; CBMC (finding #4) |
| L1 (runtime contract checks) | **runtime contract monitoring** | finding #4 (terminology map) |
| L0 (`#[slag]`, trusted-by-fiat) | **an enumerated trusted axiom** (CompCert's trusted-base concept, made per-function) | Leroy CACM (finding #3) |

**Forward simulation — the precise statement of (T2).** Per Leroy/CompCert (finding #2): semantic
preservation for a deterministic language is the relaxed "non-going-wrong" property
`∀ B ∉ Wrong, S ⇓ B ⟹ C ⇓ B`, proved by a **simulation diagram** — each source transition
corresponds to target transitions with the SAME OBSERVABLE EFFECTS, preserving a binary relation
`∼` between source states and target states. For Thermite:

- `∼` relates a Thermite state to its emitted (Verus-annotated) Rust target state.
- **"Observable" is DEFINED by the caged quantifier fragment + the effect rows.** A behavior is
  observable iff it is expressible in the frozen contract sublanguage `S_C` (the predicate the
  program is checked against) or recorded in the static `fx` effect row. This is the project-local
  content the field's "observable effects" slot is filled with — and it is a GENUINE EXTENSION
  (below).

**Reduced trusted base (cite CompCert, finding #3).** Verification never *eliminates* the trusted
base; it reduces it to an ENUMERABLE set. Leroy enumerates CompCert's residual trust:
(1) the formal semantics of source + target; (2) the unverified passes (parser, assembler,
linker); (3) extraction + the OCaml runtime; (4) Coq itself. Thermite's analogous enumeration:

| # | CompCert residual trust | Thermite analogue |
|---|---|---|
| 1 | formal semantics of source + target | the Thermite operational semantics `S` + the Verus/Rust target semantics — **THE MOST DELICATE ITEM** (see below) |
| 2 | unverified passes (parser/assembler/linker) | the unverified production lowerer `thermite-lower` (replaced per-run by the *verified* validator), plus the Thermite lexer/parser |
| 3 | extraction + runtime | the Rust↔Lean encoder-correspondence (the Rust `thermite-tv` code matching the Lean-proved algorithm — see "The Rust↔Lean correspondence gap"), rustc/LLVM/the build chain |
| 4 | Coq itself | **the Lean 4 kernel** + (today) Z3/Verus, with the Lean-SMT cvc5-reconstruction path the route to demote Z3 (finding #8) |

**The source semantics is the irreducible residue (finding #3, Leroy's item (1)).** `S`'s agreement
with the *intended* meaning of Thermite is the single most delicate item — an
unprovable-from-within assumption (Gödel; the §1 "spec-intent alignment" slot). It is STATED, never
hidden, never machine-closed.

**Genuine extensions — labeled as such (finding #1 terminology map; no direct analogue in the
surveyed verified-compilation literature).** These are Thermite-original and are NOT renamed to a
field term:

- **The caged quantifier fragment** (the 8 frozen combinators + frozen triggers) — a deliberate
  decidability/automation lever. (Novelty needs a targeted survey to confirm; asserted-by-absence.)
- **The anti-Goodhart battery** (mutation-kill-ratio + vacuity/tautology detection) — spec-quality
  / spec-mutation, no analogue in the surveyed verified-compilation lit.
- **The static-effect-rows (`fx`) + seccomp confinement hybrid** — the surveyed effect literature
  (Iris/separation logic, finding #7) does not combine static effect typing with runtime syscall
  confinement. The *hybrid* is the extension (Stacked Borrows, POPL'20, is the reference our
  Verus-annotated-Rust *target* must be reconciled against — finding #7).

## What this covers, and what it inherits (the relativity — state it plainly, Gödel)

The semantic-preservation theorem is NOT unconditional. It is RELATIVE to two named assumptions:

- **Z3 soundness.** TV's per-run agreement is `Z3 ⊢ lower(P) ⟺ R(P)`. If Z3 is unsound on a
  query, (T2) inherits that unsoundness. (This is the same trust Verus/CompCert's SMT-backed
  obligations carry; it is the floor of any SMT-discharged result, not a Thermite-specific gap.)
  **The path to demote Z3 from *trusted* to *kernel-checked* is proof reconstruction (Lean-SMT's
  cvc5 path, finding #8) — this is a primary reason Lean 4 was chosen (below); it is NOT in place
  today, so an L3 certificate currently enumerates Z3 + Verus.**
- **`S` being the intended meaning.** (T1)/(T2) say `⟦lower(P)⟧ = ⟦P⟧_S`. They do NOT prove that
  `S` is the meaning the *human wanted* — that residue is exactly the §1 "spec-intent alignment"
  slot, surfaced for review, never machine-closed (Gödel forbids a system certifying its own
  intended-meaning fixpoint; this is Leroy's delicate item (1), finding #3). `S` is a HUMAN-AUDITED
  definition; its auditability (small, declarative, mechanized in a small-kernel prover) is the
  whole design goal, mirroring why `R` is auditable.

**What the theorem COVERS:** the LOWERING link of the semantic-preservation chain
`Thermite surface → emitted Verus-annotated Rust`. For every `P` passing TV, the emitted artifact
MEANS what `P` means under `S` (the forward-simulation relation `∼` holds, preserving observable
effects).

**What it does NOT cover (inherited, unchanged):**

- The Verus VC-generator and Z3 (the `emitted Rust → SMT obligation → proof` link) stay inherited
  trusted components — exactly as in every Verus-based result. This doc does not re-verify them.
- Rust's borrow checker / LLVM codegen (the `Rust → machine code` link, `thermite-design.md` §3
  stack) — inherited from the Rust toolchain, out of scope (this is the CompCert/RustBelt
  boundary; Stacked Borrows is the model the target is reconciled against, finding #7).
- Loops in the exec-statement subset (`while`/`loop`/`break`/`continue`) — kernel-gated, framed
  in `exec-stmt-tv.md` step 2.2.2 (blocker #163); `S_B` here is the STRAIGHT-LINE fragment (the
  straight-line `S_B` is now MECHANIZED + `body_ref_state` proved sound, #172 SHIPPED — see the
  S_B Architecture section). A loop body's lowering stays checked by per-run TV-with-invariant +
  golden files until #163; a mid-body early `return` (multi-exit CPS) and a non-scalar mutation
  `xs[i]=e` are likewise OUT of the straight-line fragment (`Unsupported` in `body_ref_state`).
- **The whole-translation universal forward-simulation proof is EXPLICITLY NOT the target.** We do
  not verify the production lowerer, and we do not commit to a once-for-all simulation proof of the
  entire translation. The verified PER-RUN validator (T1 + the per-run TV check = T2) suffices for
  the threat model (Leroy: a verified validator ≡ a verified compiler, at ≈ one-pass cost).

So the theorem upgrades ONE link (lowering) from existential to universal SEMANTIC PRESERVATION,
and is explicit that the other links stay where they were. That honesty is the deliverable.

## Requirements

- **REQ-1 (the semantics S — a denotation over the frozen AST subset, mechanized in Lean 4)** —
  define `⟦·⟧_S` as inference rules / a denotation function over the `thermite-syntax::ast` node
  set, UNIFYING three sub-denotations: `S_C` (contract/spec sublanguage → a logical predicate),
  `S_E` (exec expression → a bounded value), `S_B` (exec body → a state transformer, unified from
  the EXISTING `exec-stmt-tv.md` REQ-2 big-step semantics). The math is stated prover-neutrally
  here (inference rules) AND mechanized as Lean 4 definitions (REQ-6). Covers the frozen subset:
  the contract `Expr` subset + the 8 frozen combinators (`combinators.rs` `verus_l3`), the
  pure-exec `Expr` subset, and the frozen straight-line exec-statement subset (`exec-stmt-tv.md`
  REQ-1). Derived from `thermite-design.md` §4.1/§4.2/§6. **Blocker #169 (epic); per-sub-denotation
  #170/#171/#172.**
- **REQ-2 (the verified-validator soundness theorem T1 — reference-encoder correctness w.r.t. S)**
  — state precisely: `∀ P, ⟦R(P)⟧ = ⟦P⟧_S`, where `R ∈ {ref_contract_pred, exec_ref_value,
  body_ref_state}` (the SHIPPED `thermite-tv` encoders) and `⟦R(P)⟧` is the meaning under `S` of
  the Verus STRING `R` produces. This is the VERIFIED-VALIDATOR obligation (Leroy, finding #1): the
  thing to PROVE, by structural induction over the AST subset, as a Lean `theorem`. Sketch ONE
  obligation concretely (below) to show it is non-vacuous.
- **REQ-3 (the composition T2 — semantic preservation, as forward simulation)** — state precisely:
  `[ TV: Z3 ⊢ lower(P) ⟺ R(P) ] ∧ (T1) ⟹ ⟦lower(P)⟧ = ⟦P⟧_S`, quantified over all `P` passing TV.
  State it in forward-simulation vocabulary (the relation `∼`, observable effects = caged fragment
  + effect rows; finding #2), the relativity (`{Z3, S}`), and what it does/does-not cover (above).
  This is the payoff: TV evidence + a verified validator (T1) = universal semantic preservation.
- **REQ-4 (the increment roadmap — Lean-targeted, honest, multi-cycle)** — the spec-first ordering
  (a) `S_C` + prove `ref_contract_pred` IN LEAN; (b) `S_E` + `exec_ref_value`; (c) `S_B` +
  `body_ref_state`; (d) compose. Each a future blocker (#170/#171/#172/#174); **increment (a),
  #170, is the NEXT BUILD.** Honest magnitude: CompCert/seL4-class.
- **REQ-5 (the tooling decision — COMMITTED: Lean 4 / Mathlib / Lean-SMT)** — record the DECIDED
  prover (was: "assess, do not decide"; the #173 fork is RESOLVED). Rationale: the Lean-SMT cvc5
  proof-reconstruction path is the live route to demote Z3 from *trusted* to *kernel-checked* (the
  TCB-shrink goal, finding #8); modern ecosystem. Coq / Isabelle / Verus-native recorded as
  considered-and-deferred with a one-line reason each. **Blocker #173 RESOLVED.**
- **REQ-6 (the Lean project setup — pin it for the builder)** — specify the new top-level `lean/`
  directory: a `lakefile.toml` depending on Mathlib + Lean-SMT, the toolchain pinned via
  `lean-toolchain`; the module layout — (a) the Thermite AST as a Lean inductive (the frozen subset,
  mirroring `thermite-syntax/src/ast.rs`); (b) the denotation `⟦·⟧_S` as a Lean function/relation
  (the contract sublanguage `S_C` first); (c) a Lean model of the reference encoder
  `ref_contract_pred`'s output; (d) the soundness theorem T1 as a Lean `theorem`. Document the
  Rust↔Lean correspondence honestly (below). **The builder writes this project next dispatch
  (increment (a), #170); this REQ is the spec it builds to.**
- **REQ-7 (the architecture decision — a verified validator)** — record the DECIDED architecture
  (was: this doc's own open question): a VERIFIED VALIDATOR (prove `R` sound against `S` = T1),
  NOT a verified lowerer, and NOT (necessarily) a full universal forward-simulation proof of the
  whole translation. The per-run verified validator suffices for the threat model (Leroy, finding
  #1; Necula, finding #4). This resolves the open architectural question (finding #1 + open-question
  #2 of the research doc).

## Acceptance criteria

This component is the META-THEOREM layer + the committed architecture; its ACs are
STATEMENT-COMPLETENESS + NON-VACUITY + GROUNDEDNESS + DECISION-RECORDED, not a `cargo test` (the
mechanization is increment (a), #170, the next build). The mechanical discharge of each soundness AC
moves to the per-increment Lean blockers as they land.

- **AC-1 (S is stated over the exact frozen subset)** — every node admitted by the SHIPPED
  encoders has a denotation rule here: the contract `Expr` subset that `ref_contract_pred`
  admits, the pure-exec subset `exec_ref_value` admits, and the straight-line exec-statement
  subset `body_ref_state` admits (`exec-stmt-tv.md` REQ-1 IN set). A node the encoders honestly
  `Err`/Skip on (`RefEncodeError::Unsupported`) is OUT of `S` and explicitly marked OUT here.
  Mechanically: the denotation domain = the union of the three encoders' admitted-node sets.
- **AC-2 (T1 is non-vacuous — a concrete obligation exists)** — at least one (T1) obligation is
  written out fully: the denotation `⟦P⟧_S`, the encoder output `R(P)`, the meaning `⟦R(P)⟧`, and
  the equality to prove, for a real clause. GROUNDED below for `ens result == spec_sum(xs)` and
  for the `forall_in` combinator. The obligation is NOT `X = X` (the production-coercion / binop
  re-statement is where the content lives — the same `==`-vs-`<=` content the TV teeth bite on).
- **AC-3 (T2 composition is valid given T1)** — the composition is a one-line logical step (modus
  ponens over `⟺` and `=`) once (T1) holds; stated below with its relativity and its
  forward-simulation reading. Mechanically: the proof of (T2) from (T1) + the TV obligation is the
  trivial transitivity `⟦lower(P)⟧ = ⟦R(P)⟧` (from `Z3 ⊢ lower(P) ⟺ R(P)` interpreted under S)
  `= ⟦P⟧_S` (from T1).
- **AC-4 (the relativity is explicit + the coverage boundary stated, in the reduced-trusted-base
  framing)** — the `{Z3, S}` relativity, the CompCert-style trusted-base enumeration (cite Leroy,
  finding #3), and the inherited links (Verus VC-gen, borrow checker, LLVM, loops) are named (above
  + below), so no reader mistakes the theorem for unconditional or whole-toolchain preservation.
- **AC-5 (the tooling + architecture decisions are RECORDED)** — Lean 4 (Mathlib + Lean-SMT) is
  recorded as COMMITTED with its TCB-shrink rationale; Coq/Isabelle/Verus-native as
  considered-and-deferred; the verified-validator architecture (not a verified lowerer, not a full
  universal simulation) is recorded as DECIDED. (#173 RESOLVED.)
- **AC-6 (the Lean project is specified for the builder)** — the `lean/` directory layout,
  `lakefile.toml` deps (Mathlib + Lean-SMT), `lean-toolchain` pin, and the four-module structure
  (AST inductive / denotation / encoder model / T1 theorem) are specified, with the Rust↔Lean
  correspondence gap stated. This is the spec increment (a) (#170) builds to.

## Architecture — the semantics S (the denotation)

`S` is a denotation function `⟦·⟧_S` from the frozen `thermite-syntax::ast` subset to a
mathematical meaning. It UNIFIES three sub-denotations into one program meaning. The metavariable
`P` ranges over a Thermite fn-or-clause; `lower(P)` is the production-emitted Verus-annotated Rust;
`R(P)` is the reference-encoder output (a Verus string). `⟦·⟧` applied to a Verus string is its
meaning in the standard Verus/vstd model (the model Verus itself denotes into — `S` does not
re-define Verus's meaning, it ANCHORS to it; this is the same move `ref_contract_pred` makes when
it reuses `lookup(name).verus_l3` as the combinator denotation). In the forward-simulation framing
(finding #2), `⟦lower(P)⟧ = ⟦P⟧_S` IS the relation `∼` holding between `P`'s Thermite state and
`lower(P)`'s target state, preserving the observable effects (the caged fragment + the `fx` rows).

### S_C — the spec/contract sublanguage (smallest, most stable — START HERE)

The contract denotation `⟦·⟧_{S_C} : ContractExpr → Predicate`. A `Clause` holds an ordinary
`Expr` (`ast.rs` `struct Clause { expr, .. }`); there are NO statements/loops/mutation in a clause
(`contract-tv.md` Architecture), so `S_C` is a TOTAL structural recursion. Inference rules over the
admitted `Expr` subset:

```
                                                       (literals / refs denote themselves)
⟦ IntLit{value=n} ⟧_{S_C}     = n                     (the integer n)
⟦ BoolLit(b) ⟧_{S_C}          = b
⟦ Path([x]) ⟧_{S_C}           = x                      (a free var: a param, or result/old(_))
⟦ result ⟧_{S_C}              = result                 (the distinguished return binding)
⟦ Call(old, [x]) ⟧_{S_C}      = old(x)                 (the pre-state binding; a free var)

                                                       (comparisons / logical / arithmetic
                                                        denote the corresponding math relation,
                                                        re-stated INDEPENDENTLY — the binop map)
⟦ Binary(Eq, a, b) ⟧_{S_C}    = ( ⟦a⟧ = ⟦b⟧ )         ⟦ Binary(Le, a, b) ⟧ = ( ⟦a⟧ ≤ ⟦b⟧ )
⟦ Binary(Lt, a, b) ⟧_{S_C}    = ( ⟦a⟧ < ⟦b⟧ )         ... (Ne/Gt/Ge analogous)
⟦ Binary(And, a, b) ⟧_{S_C}   = ( ⟦a⟧ ∧ ⟦b⟧ )         ⟦ Binary(Or, a, b) ⟧ = ( ⟦a⟧ ∨ ⟦b⟧ )
⟦ Unary(Not, a) ⟧_{S_C}       = ¬ ⟦a⟧
⟦ Binary(Add, a, b) ⟧_{S_C}   = ⟦a⟧ + ⟦b⟧             (arithmetic in subterms; at the
                                                        spec-context numeric domain — see coercions)

                                                       (spec-fn calls denote their recursive,
                                                        dec-measured, TERMINATING denotation)
⟦ Call(f, args) ⟧_{S_C}       = ⟦body_of(f)⟧_{S_C}[params ↦ ⟦args⟧]     when f is a named spec fn
                                                        (well-defined because §4.2 mandates a `dec`
                                                         measure ⟹ the recursion terminates ⟹ the
                                                         denotation is a well-founded fixpoint)

                                                       (the 8 frozen combinators denote their
                                                        FROZEN verus_l3 quantifier form — these ARE
                                                        a denotation already, combinators.rs)
⟦ Call(C, args) ⟧_{S_C}       = ⟦ lookup(C).verus_l3 [formals ↦ ⟦args⟧] ⟧
                                  for C ∈ {sorted, forall_in, exists_in, count_where,
                                           permutation_of, disjoint, forall_below, forall_from}
```

**The spec-context rewrites are DENOTATION-PRESERVING COERCIONS to pin (the content of S_C).**
These are exactly where production fidelity bugs live (`contract-tv.md`: the #122/#127/#146
classes), so they are the load-bearing part of `S_C`'s definition AND of the (T1) proof:

- **slice → `@` view.** A slice term in spec position denotes its `Seq` view: `⟦xs⟧` (slice) is the
  sequence `xs@`. `⟦Ref(Index(xs, RangeTo(i)))⟧` (`&xs[..i]`) = `xs@.subrange(0, ⟦i⟧ as int)`.
  These are denotation-PRESERVING: the `@`/subrange is the identity-on-meaning coercion from the
  exec slice to its spec sequence (the meaning is the same sequence of elements).
- **method → `spec_*` byte-view.** `⟦s.len()⟧` = `s.spec_len()`, `⟦s.byte_at(i)⟧` =
  `s.spec_byte_at(⟦i⟧ as int)` for a `String`/`&TString` receiver — the wrapper spec fns denote the
  same length/byte (the #127 class: a wrong index is a DIFFERENT meaning, caught).
- **cast → `nat`/`int`.** In spec position an integer cast denotes the value as an unbounded
  `nat`/`int` (no wrap — spec arithmetic is mathematical), e.g. `⟦(n - 1) as nat⟧` = the natural
  number `⟦n⟧ - 1` under the `⟦n⟧ ≥ 1` frame. The PARENTHESIZATION (the #122 class) is a property of
  the production STRING, not the meaning; `S_C` denotes the AST, so the (T1) obligation is precisely
  "does the production string PARSE to an AST whose denotation matches" — which is why the
  paren-drop is a (T1) failure (a different parse = a different denotation).

`S_C` is the MOST stable + smallest sub-denotation (a clause is a pure predicate), so it is
increment (a) (#170) — and it is where the boss's canonical `==`-vs-`<=` semantic-preservation
question lives.

### S_E — the exec-expression sublanguage (bounded values)

`⟦·⟧_{S_E} : ExecExpr → BoundedValue` where `BoundedValue ∈ {u64, u32, usize, bool}` — NOT
unbounded `nat`/`int` (the load-bearing dual of `S_C`'s coercion soundness, `exec-tv.md` "the
exec-value semantics"). An exec expression denotes a value AT ITS BOUNDED TYPE, carrying the
always-active L1 overflow semantics (`thermite-design.md` §6):

```
⟦ Binary(Add, a, b) ⟧_{S_E}   = ⟦a⟧ +_{checked} ⟦b⟧      (bounded add; DEFINED only when no
                                                            overflow — the overflow obligation;
                                                            a wrapping_add is a DIFFERENT denotation)
⟦ Cast(e, T) ⟧_{S_E}          = narrow_T(⟦e⟧)             (narrowing/wrapping AT the target type T)
⟦ Binary(Lt, a, b) ⟧_{S_E}    = ( ⟦a⟧ < ⟦b⟧ ) : bool
⟦ Index(xs, Single(i)) ⟧_{S_E} = ⟦xs⟧[⟦i⟧]               (the element value; the spec view xs[i as int]
                                                            denotes the SAME element)
⟦ Call(f, args) ⟧_{S_E}       = the exec value of f applied to ⟦args⟧
```

The denotation is NEVER nat-coerced (a `nat`-coerced exec reference would mask the wrap point —
exactly the soundness hole `exec-tv.md` AC-4 catches). `S_E` is increment (b) (#171).

### S_B — the exec-body sublanguage (state transformer; UNIFIED from exec-stmt-tv.md)

`S_B` is NOT re-invented — it is the EXISTING big-step state-transformer designed + GROUNDED in
`.design/verified/exec-stmt-tv.md` REQ-2, lifted into `S`. In the forward-simulation framing, the
body's big-step transition is exactly a source transition whose target counterpart must preserve
the relation `∼` (finding #2). Quoting that doc's semantics verbatim (it is already the operational
meaning of the frozen straight-line subset):

> a statement sequence is a STATE TRANSFORMER. The **program state** is the environment of
> in-scope mutable + immutable bindings (name → bounded-scalar value). For a straight-line body the
> denotation is a **big-step** evaluation threading an initial environment (the fn params) through
> the statement sequence to a FINAL environment, and the body's value is the tail expression
> evaluated in that final environment.

So `⟦·⟧_{S_B} : Block × Env → Env × BoundedValue`, big-step over the FROZEN straight-line subset
(`exec-stmt-tv.md` REQ-1 IN set): `Stmt::Let`/`Assign`/`If`/`Expr`/tail-`Return` + `Block`
sequencing/tail. Inference rules (big-step `⟨B, σ⟩ ⇓ ⟨σ', v⟩`):

```
                ⟨init, σ⟩ ⇓_{S_E} v
─────────────────────────────────────────────────  (LET — extend env)
⟨ Let{name, init} ; rest, σ⟩ ⇓ ⟨σ'', v''⟩    where ⟨rest, σ[name ↦ v]⟩ ⇓ ⟨σ'', v''⟩

                ⟨value, σ⟩ ⇓_{S_E} v
─────────────────────────────────────────────────  (ASSIGN — update cell, ORDER-SENSITIVE)
⟨ Assign{target=x, value} ; rest, σ⟩ ⇓ ⟨σ'', v''⟩   where ⟨rest, σ[x ↦ v]⟩ ⇓ ⟨σ'', v''⟩

⟨cond, σ⟩ ⇓_{S_E} true     ⟨then, σ⟩ ⇓ ⟨σ', _⟩
─────────────────────────────────────────────────  (IF-THEN; IF-ELSE symmetric — branch compose)
⟨ If{cond, then, else_} ; rest, σ⟩ ⇓ ⟨σ'', v''⟩     where ⟨rest, σ', ⟩ ⇓ ⟨σ'', v''⟩

──────────────────────────────────  (TAIL — the body value = the projection of the final env)
⟨ {stmts ; tail}, σ⟩ ⇓ ⟨σ_final, ⟦tail⟧_{S_E} in σ_final⟩
```

`S_B` composes `S_E` on each statement's RHS / `if` condition / tail (the per-RHS value
denotation), adding ONLY the state-threading / mutation-substitution / branch-composition. The
loop rules (`Stmt::Loop`/`Break`/`Continue`) are OUT of `S_B` here (kernel-gated, #163): a loop's
denotation is a fixpoint, not a finite substitution. `S_B` is increment (c) (#172).

**SHIPPED (#172) — `S_B` mechanized + `body_ref_state` proved sound for the straight-line block
fragment, in Lean 4 (`lean/Thermite/Exec/Stmt.lean`, namespace `Thermite.Exec`).** `S_B` is the
big-step state transformer `bodyDenote : Block → State → Option ExecVal`, where a `State` is 2a's
`ExecEnv` (var → bounded `ExecVal`, slices → element sequences) PLUS the in-scope cell set (the
re-shadow / unbound-target guards `body_ref_state` enforces). The straight-line `Stmt`/`Block`
forms are `letS` (bind a fresh cell), `assign` (ORDER-SENSITIVE scalar-cell rebind — the v1
mutation; a non-scalar `xs[i]=e` is `Unsupported` in `body_ref_state`, so it is honestly ABSENT,
not embed-then-`sorry`), `exprS` (no state effect, well-formedness only), `ifElse` (branch
composition), sequencing (`blockThread`), and the tail value (`execDenote` in the final state).
`bodyRefState` models `body_ref_state`'s operational threading independently (it routes each value
position through 2a's `execRefValue`), and `theorem body_ref_sound : bodyRefState b st =
bodyDenote b st` lifts 2a's `exec_ref_sound` through the threading (axioms `propext`/`Quot.sound`
only — standard, no `sorry`/`native_decide`/custom axiom). The state transformer is GENUINE (not
blanket vacuity): the obligation-`none` of 2a (overflow / div-zero / out-of-range) PROPAGATES
through the body (`body_overflow_rhs_has_no_result` vs `body_in_range_rhs_has_result`), and three
negative lemmas bite — `wrong_var_assign_breaks_soundness` (a wrong-cell assign), 
`sequencing_order_breaks_soundness` (the assign order reordered), `mutation_not_applied_breaks_
soundness` (a dropped assign). LOOPS (`Stmt::Loop`/`Break`/`Continue`) + a mid-body early `return`
(multi-exit CPS) + a non-scalar mutation remain OUT (2c #163, kernel-gated).

`S = S_C ⊔ S_E ⊔ S_B` is the unified program meaning. A whole fn's meaning is: its `req`/`ens`
clauses denote under `S_C` (the predicate the body must satisfy), its body denotes under `S_B` (the
state transformer over `S_E`-valued statements), and semantic preservation of the whole fn is
preservation of all three sub-denotations composed (increment (d), #174).

## The SOUNDNESS THEOREM (T1 + T2), stated precisely

Let `R` denote whichever reference encoder applies to `P`'s syntactic class:
`R_C = ref_contract_pred` for a contract clause, `R_E = exec_ref_value` for a pure exec expr,
`R_B = body_ref_state` for a straight-line body. All three are SHIPPED in `thermite-tv` and are
INDEPENDENT of the production lowerer (a COMPILE constraint: `cargo tree -p thermite-tv` =
`thermite-syntax` + `thermite-spec` only — no `thermite-lower`; `contract-tv.md` AC-6). In Leroy's
terms (finding #1), `R` + the per-run Z3 check IS the validator; (T1) is what makes it a *verified*
validator.

> **(T1) — verified-validator soundness (reference-encoder correctness w.r.t. S).**
> `∀ P, ⟦ R(P) ⟧ = ⟦ P ⟧_S`
>
> i.e. the meaning (under the Verus/vstd model `S` anchors to) of the Verus STRING the reference
> encoder produces EQUALS the denotation of the source `P` under `S`. This is the OBLIGATION TO
> PROVE, by structural induction over the frozen AST subset (one case per inference rule above),
> as a Lean 4 `theorem` (REQ-6).

> **(T2) — semantic preservation, stated as a forward simulation.**
> For all `P` that PASS translation validation,
> `[ TV:  Z3 ⊢ lower(P) ⟺ R(P) ]  ∧  (T1)  ⟹  ⟦ lower(P) ⟧ = ⟦ P ⟧_S`
>
> i.e. `S ≈ C` for this `P`: the relation `∼` holds between `P`'s Thermite state and `lower(P)`'s
> target state, preserving the observable effects (the caged fragment + the `fx` rows define
> "observable", finding #2).
>
> PROOF (the one-line modus-ponens, AC-3): TV gives `lower(P) ⟺ R(P)` (Z3-proved; interpreted
> under `S`'s anchor model this is `⟦lower(P)⟧ = ⟦R(P)⟧`). (T1) gives `⟦R(P)⟧ = ⟦P⟧_S`. Transitivity:
> `⟦lower(P)⟧ = ⟦R(P)⟧ = ⟦P⟧_S`. ∎ (relative to {Z3 soundness, S = intended meaning})

**What (T2) buys (the existential → universal upgrade; Leroy finding #1).** Before: TV gave "for the
programs we ran, `lower(P) ⟺ R(P)`" — existential, and silent on whether `R` is correct. After: for
ANY `P` passing TV, the LOWERING IS SEMANTICS-PRESERVING w.r.t. `S`. The per-run Z3 check is
unchanged; (T1) is the one-time, structural-induction investment (≈ one compiler-pass of effort,
Necula finding #4) that makes every future TV pass a universal semantic-preservation guarantee. The
production lowerer STAYS UNVERIFIED — it is checked per-run against a now-VERIFIED reference (a
verified validator ≡ a verified compiler, Leroy).

**The relativity, restated (Gödel, AC-4):** (T2) is NOT unconditional. It is relative to
`{Z3 soundness (the TV check's discharge), S being the intended meaning (a human-audited
definition — Leroy's delicate item (1), finding #3)}`. It covers the LOWERING link only; the Verus
VC-gen, the borrow checker, LLVM, and loops stay inherited / kernel-gated. No reader should read
(T2) as whole-toolchain or unconditional preservation.

## The concrete (T1) obligation, written out (AC-2 — proof that the theorem is not vacuous)

We cannot fully MECHANIZE (T1) yet (the Lean project is increment (a), #170, the next build). But
the obligation is concrete and non-trivial. Two cases.

### Case 1 — `ens result == spec_sum(xs)` (the boss's ==-vs-<= surface lives here)

The source clause `P = Binary(Eq, result, Call(spec_sum, [xs]))`.

- **`⟦P⟧_{S_C}`** (the denotation, by the rules above):
  `( ⟦result⟧ = ⟦Call(spec_sum, [xs])⟧ )` = `( result = spec_sum(xs@) )`, where `spec_sum(xs@)` is
  the well-founded recursive denotation of the `dec`-measured `spec fn spec_sum`, and `xs@` is the
  slice→`Seq` coercion (denotation-preserving). Numeric domain: spec `nat` (the cast coercion).
- **`R_C(P)` (the SHIPPED encoder output)**: `result as nat == spec_sum(xs)` — note the `as nat`
  coercion `ref_contract_pred` infers (its `RefCtx` `nat_coerce` rule, independently re-stating
  production's `lower_nat_equality`, `contract-tv.md` REQ-1).
- **`⟦R_C(P)⟧`** (the meaning of that Verus string under the vstd model):
  `( (result as nat) = spec_sum(xs) )` — `xs` bound as `Seq<u32>` so `spec_sum(xs)` is the same
  recursive sum; `result as nat` is the same coercion to the spec `nat` domain.
- **The (T1) obligation:** prove `⟦R_C(P)⟧ = ⟦P⟧_{S_C}`, i.e.
  `( (result as nat) = spec_sum(xs) )  =  ( result = spec_sum(xs@) )` — discharged by: the `as nat`
  coercion is the identity-on-value injection into the `nat` domain (the spec numeric domain `S_C`
  denotes into), and `xs ≡ xs@` (the slice→`Seq` view is the identity-on-meaning coercion). NOT
  vacuous: if production had emitted `result as nat <= spec_sum(xs)` (the `<=` infidelity), the
  encoder's `==` would denote a DIFFERENT predicate, the TV `⟺` would FAIL (Z3 counterexample,
  `contract-tv.md` AC-2), and the chain would correctly refuse to conclude preservation. The
  content of the obligation is exactly the binop + coercion re-statement `S_C` defines
  INDEPENDENTLY of production — which is why a production binop/coercion bug is visible.

### Case 2 — the `forall_in` combinator (the combinator denotation is the frozen verus_l3)

The source clause uses `forall_in(xs, |x| x <= 10)`, `P = Call(forall_in, [xs, |x| x <= 10])`.

- **`⟦P⟧_{S_C}`** = `⟦ lookup("forall_in").verus_l3 [s ↦ xs@, p ↦ (λx. x ≤ 10)] ⟧`. From
  `combinators.rs` the frozen `verus_l3` is
  `forall|i: int| 0 <= i < s.len() ==> #[trigger] p(s[i])`, so the denotation is the bounded
  universal `∀ i. 0 ≤ i < |xs@| ⟹ (xs@[i] ≤ 10)`.
- **`R_C(P)`** reuses the SAME frozen `lookup("forall_in").verus_l3` (the shared external ground
  truth, `contract-tv.md` Architecture "REUSED") for the combinator BODY, while INDEPENDENTLY
  encoding the slice→`@` of the slice arg and the predicate closure `|x| x <= 10`.
- **The (T1) obligation:** `⟦R_C(P)⟧ = ⟦P⟧_{S_C}` — discharged by: the combinator body is shared
  verbatim (so its denotation is identical by construction — this case of the induction is the
  trivial reflexivity ON THE BODY), and the obligation's CONTENT is the ARGUMENT rewrites (the
  slice `@`-view + the predicate-closure encoding), which `R_C` re-implements independently. NOT
  vacuous: a wrong predicate (`|x| x < 10` for source `|x| x <= 10`) is a different denotation, TV
  fails (`contract-tv.md` AC-3). NOTE the induction structure: the combinators are a CLOSED set of
  8, so the combinator-body cases are 8 base cases (no recursion — the bodies are frozen strings),
  and `forall_in`'s denotation is well-defined because the predicate-closure body is a FLAT
  predicate (§4.2 "no anonymous nested quantifiers"), so the structural induction on the closure
  body terminates. This is the strongest reason to believe (T1) IS provable by structural
  induction for `S_C` (see least-confident note).

## REQ-4 — the increment roadmap (Lean-targeted, honest, multi-cycle; spec-first)

The full `∀`-proof is multi-cycle. The ordering maximizes value-per-increment (spec-first, where
the highest-value preservation content lives) and respects the dependency order (compose last).
**All four increments are now Lean 4 proofs (the #173 fork is resolved).**

| # | Increment | Sub-denotation | Encoder to prove (in Lean) | Blocker | Why this order |
|---|---|---|---|---|---|
| (a) | spec/contract-sublanguage `S_C` + prove `ref_contract_pred` sound | `S_C` | `ref_contract_pred` | **#170 (NEXT BUILD)** | SMALLEST + most stable (a clause is a pure predicate, no state); HIGHEST value (the boss's `==`-vs-`<=`, the #122/#127 classes); closed 8-combinator induction (Case 2) most tractable. Also stands up the `lean/` project (REQ-6). FIRST. |
| (b) | exec-expression `S_E` + prove `exec_ref_value` sound | `S_E` | `exec_ref_value` | **#171 (SHIPPED — Layer 2 OPENED)** | bounded-value denotation (`Thermite.Exec`, `lean/Thermite/Exec.lean`); `theorem exec_ref_sound` kernel-checked; `S_E ≠ S_C` (BOUNDED, overflow-as-OBLIGATION, NEVER nat-coerced); the nat-coercion negative lemma `nat_coercion_underflow_breaks_soundness` bites; reuses the (a) Lean project. |
| (c) | exec-statement `S_B` + prove `body_ref_state` sound | `S_B` (straight-line) | `body_ref_state` | **#172 (SHIPPED — straight-line block fragment)** | the big-step STATE TRANSFORMER (`Thermite.Exec.Stmt`, `lean/Thermite/Exec/Stmt.lean`); `theorem body_ref_sound` kernel-checked (axioms `propext`/`Quot.sound` only, no `sorry`). `S_B` is a `State → Option (State × tail value)` over the frozen straight-line `Stmt`/`Block` forms (`letS`/`assign`/`exprS`/`ifElse`/sequencing/tail), UNIFIED from exec-stmt-tv.md REQ-2; composes 2a's `execDenote` per RHS / condition / tail (the obligation-`none` PROPAGATES). 3 negative lemmas bite: `wrong_var_assign_breaks_soundness`, `sequencing_order_breaks_soundness`, `mutation_not_applied_breaks_soundness`. LOOPS stay kernel-gated (#163). |
| (d) | COMPOSE → the whole-frozen-subset semantic-preservation theorem | `S = S_C ⊔ S_E ⊔ S_B` | all three | **#174** | the capstone (T2) over the whole frozen subset; depends on (a)(b)(c). |

Each increment is a future blocker; none is started (this doc is the skeleton + committed
architecture). The honest magnitude: this is CompCert/seL4-class effort. CompCert took
person-decades to verify a C compiler in Coq; Thermite's economy (Leroy finding #1, Necula finding
#4) is that it verifies the SMALL reference VALIDATOR (auditable; increment (a) is a few-hundred-line
structural induction over a closed AST subset, ≈ one compiler-pass of effort), NOT the production
lowerer — but even that is multi-cycle.

## REQ-5 — the TOOLING DECISION: Lean 4 (Mathlib + Lean-SMT) — COMMITTED (#173 RESOLVED)

The proof-assistant fork (#173) is RESOLVED. `S` is mechanized and (T1) is proved in **Lean 4**,
with **Mathlib** and **Lean-SMT**.

**Rationale (the TCB-shrink goal, finding #8).** Lean-SMT dispatches to **cvc5** and "reconstructs
SMT proofs into native Lean proofs ... submitted to the Lean kernel" (Lean-SMT, arXiv 2505.15796) —
the live route to demote Z3 (the TV check's discharge) from a *trusted* component to a
*kernel-checked* one, shrinking Thermite's enumerable trusted base (the §1 "skeptical third party
audits in minutes" residue, and Leroy's reduced-trusted-base item (4), finding #3). Lean 4 also has
a small, auditable kernel and a modern ecosystem (Mathlib). A (T1) proof that type-checks in Lean
lands the verified-validator theorem in a small, independently-auditable TCB.

**Considered and DEFERRED (one-line reasons):**

- **Coq** — the precedent-strongest alternative (CompCert, RustBelt/Iris, Stacked Borrows all live
  in Coq; finding #6/#7). DEFERRED: kept as the mature fallback IF the lowering-target Rust
  semantics are later brought into scope (CompCert/RustBelt reuse); for the NARROW (T1) object over
  Thermite's small AST, it has no cvc5-reconstruction path as live as Lean-SMT, so it does not
  advance the Z3-demotion goal as directly.
- **Isabelle/HOL** — the kernel-direction pick (seL4 / L4.verified; finding, proof-assistant
  landscape). DEFERRED: chosen only if the verified-microkernel direction (§13) later dominates; not
  the modern choice for this object and no live cvc5-reconstruction route.
- **Verus-native** — lowest integration cost (one tool, no new dependency). DEFERRED / recommended
  AGAINST for the (T1) proof: a Verus proof is discharged by Z3, so it folds the metatheory's trust
  BACK into Z3 (no TCB shrink — the opposite of the goal), and a metatheorem over a STRING-PRODUCING
  encoder is awkward in Verus (it reasons about values, not string semantics). The per-run TV check
  itself correctly STAYS in Verus/Z3 (a Rust-program-verification task, Verus's home); only the
  (T1) metatheorem moves to Lean.

## REQ-6 — the Lean project setup (pinned for the builder; increment (a), #170)

A NEW top-level directory **`lean/`** holds the Lean 4 development. (Chosen over
`thermite-semantics/` to avoid colliding with the crate-naming convention and to read as "the Lean
side of the toolchain".) It is NOT a Cargo crate and is NOT routed in `tooling/spec-routes.toml`
(which routes `thermite-*/src/**/*.rs` + `forge/src/**/*.rs`); it is governed by THIS doc directly.

**Build files (the builder writes these — this doc does not):**

- `lean/lakefile.toml` — a Lake package depending on **Mathlib** and **Lean-SMT** (confirm the exact
  Lake `require` names against the Lean-SMT repository at build time; per arXiv 2505.15796 it is the
  cvc5-reconstruction project — the dependency name should be taken from its `lakefile`, not guessed
  here).
- `lean/lean-toolchain` — pins the Lean 4 toolchain version (must match the Mathlib + Lean-SMT pin;
  Mathlib's `lean-toolchain` is the constraint to follow).

**Module layout (the four pieces, in increment (a) order):**

| Module (proposed) | Content | Grounded in (Rust side) |
|---|---|---|
| (a) `Thermite/Ast.lean` | the Thermite AST embedded as a Lean `inductive` — the FROZEN contract subset first (`Expr`: `IntLit`/`BoolLit`/`Path`/`Call`/`Binary`/`Unary`/the slice & method forms) | mirrors `thermite-syntax/src/ast.rs` (the `Expr`/`Clause` node set the encoders admit) |
| (b) `Thermite/Denote.lean` | the denotation `⟦·⟧_S` as a Lean function/relation — `S_C` first (the combinators via their frozen `verus_l3` forms, comparisons/logical/spec-fns/the coercion rewrites above) | the `S_C` inference rules above; `thermite-spec/src/combinators.rs` (`verus_l3`) |
| (c) `Thermite/RefEncode.lean` | a Lean model of the reference encoder `ref_contract_pred`'s OUTPUT (the algorithm it implements, as a Lean function) | models `thermite-tv/src/ref_encode.rs` (`ref_contract_pred`) |
| (d) `Thermite/Soundness.lean` | the soundness theorem **T1** as a Lean `theorem`: `∀ P, ⟦R(P)⟧ = ⟦P⟧_S` (proved by `induction` on the AST, one case per rule) | (T1) above |

**The Rust↔Lean correspondence gap (state it honestly — a residual trusted item, finding #3 item
(3)).** The Lean side proves the ALGORITHM the Rust encoder implements is sound. That the Rust code
in `thermite-tv` ACTUALLY MATCHES the Lean-proved algorithm is NOT itself proved by Lean — it is a
gap (analogous to CompCert's extraction/runtime trust, Leroy's item (3)). Two honest mitigations,
neither a proof: (1) keep the `ref_contract_pred` algorithm SIMPLE ENOUGH to audit by inspection
against the Lean model (`contract-tv.md` already constrains it to be small + declarative); (2) the
per-run TV check still bites on any divergence between the actual Rust encoder output and production
(it just cannot confirm the Rust encoder matches the Lean model — only a human audit closes that).
This gap is enumerated in the reduced-trusted-base table (item 3) and is NOT hidden.

## REQ-7 — the ARCHITECTURE DECISION: a verified validator (DECIDED; #173)

The architecture is DECIDED (this resolves the doc's own open question — research finding #1 +
open-question #2):

- **A VERIFIED VALIDATOR** — prove the reference encoder `R` sound against `S` (= T1), so the
  existing per-run TV check becomes a *verified* validator. Leroy (finding #1): a verified validator
  composed with an unverified compiler is as strong as a verified compiler, "provided the validator
  is smaller and simpler than the compiler"; Necula (finding #4): ≈ one compiler-pass of effort. We
  already HAVE the validator (`ref_contract_pred`/`exec_ref_value`/`body_ref_state` + Z3); the
  missing piece is its soundness proof (T1).
- **NOT a verified lowerer.** We do NOT verify `thermite-lower` (the verify-the-compiler path —
  intractable for a thousands-of-lines production lowerer; the whole economy is to avoid it).
- **NOT (necessarily) a full universal forward-simulation proof of the whole translation.** The
  verified PER-RUN validator (T1 + the per-run TV check = T2) suffices for the threat model. We do
  not commit to a once-for-all simulation proof of the entire translation; (T2) is a per-run
  semantic-preservation guarantee that becomes universal-over-passing-`P` once (T1) holds.

## Honest scope (do NOT overclaim)

- This doc is the FORMAL SKELETON + the committed architecture (prover + vocabulary + verified-
  validator) + the Lean project spec + the first-increment plan. It is NOT the proof. The full
  `∀`-proof of (T1) across `S_C`/`S_E`/`S_B` + the (T2) composition is MULTI-CYCLE,
  CompCert/seL4-class work.
- **The economic thesis is an ACCEPTED PREMISE, not a finding.** Thermite's central thesis — that
  AI agents make the historically-prohibitive annotation/proof burden affordable by paying it in
  compute — is UNVERIFIED by the SOTA evidence set (finding, honest gap #1: the autoformalization /
  LLM-proof angle did not survive adversarial verification). It is the HUMAN'S CALL to accept it as
  a load-bearing PREMISE of the project; this doc records it as an accepted premise, NOT something to
  be formally proven here. (It warrants its own targeted survey; out of scope for #169.)
- **The Rust↔Lean encoder-correspondence gap is real** (REQ-6 above): Lean proves the ALGORITHM
  sound; that the Rust `thermite-tv` code matches the algorithm is a residual trusted item (audit by
  inspection + simple encoders, not a proof). Enumerated in the reduced-trusted-base table (item 3).
- (T2) is RELATIVE, not unconditional: `{Z3 soundness, S = intended meaning}`, lowering-link only,
  loops kernel-gated, Verus-VC-gen/borrow-checker/LLVM inherited (the reduced-trusted-base framing,
  Leroy finding #3).
- The Lean-SMT Z3-demotion path is the GOAL, not a present fact: Lean-SMT's cvc5 reconstruction
  covers ~30% of cvc5's proof rules today (finding #8), and Verus/Z3 do not emit reconstructable
  proofs by default — so TODAY an L3 certificate still enumerates Z3 + Verus. The DEMOTION is the
  reason Lean was chosen; whether Lean-SMT is mature enough to actually demote Z3 for OUR VCs is
  flagged least-confident (below).
- This is the soundness ARC's keystone, not its completion. The keystone makes the arch STATEABLE +
  load-bearing; the increments (#170/#171/#172/#174) build it, in Lean.

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (the semantics S — denotation over the frozen subset, mechanized in Lean 4) | NOT-STARTED (S_C is COMPLETE 8/8; S_E is SHIPPED-for-the-exec-EXPR-fragment under #171; S_B remains — open blocker #172 + loops #163) | `S` is stated as inference rules ABOVE (Architecture), GROUNDED in the SHIPPED encoders' admitted-node sets (`ref_contract_pred`/`exec_ref_value`/`body_ref_state` in `thermite-tv/src/{ref_encode,exec_encode,exec_stmt_encode}.rs`) and the frozen `lookup(C).verus_l3` (`thermite-spec/src/combinators.rs`) and UNIFIES the EXISTING state-transformer from `exec-stmt-tv.md` REQ-2. **`S_C` (contract) is MECHANIZED + COMPLETE (8/8 construct classes, `lean/Thermite/{Ast,Denote,RefEncode,Soundness}.lean`).** **`S_E` (the exec-EXPRESSION sublanguage — Layer 2 OPENED, #171) is now MECHANIZED in `lean/Thermite/Exec.lean` as `def execDenote : ExecExpr → ExecEnv → Option ExecVal`: a BOUNDED-VALUE denotation (`ExecVal = .int BVal | .bool Bool`, the integer ALWAYS carrying its type's bound `2^width` — `S_E ≠ S_C`: bounded not unbounded `int`, overflow carried as a PROOF OBLIGATION via `Option`-partiality, NEVER nat-coerced), GROUNDED in `thermite-tv/src/exec_encode.rs::exec_ref_value`'s admitted node set (int/bool lit, var, arith, cmp, logical, `!`, cast, slice-index). The exec-BODY `S_B` (state transformer) remains — increment (c), #172, + loops #163. |
| REQ-2 (T1 — verified-validator / reference-encoder soundness w.r.t. S) | NOT-STARTED (the FULL `S_C` `∀ P`; the comparison/logical FRAGMENT is kernel-checked under #170, the ARITHMETIC + CAST FRAGMENT under #176/#177, the SPEC-CONTEXT REWRITES under #178, the 6 BOUNDED-QUANTIFIER COMBINATORS under #179, the MATCH-IN-ENS / `is` C7 PAYLOAD-IN-CONTRACT forms under #180, the NAMED SPEC-FN CALLS — incl. RECURSION — under #181, the 2 RECURSIVE/AGGREGATE COMBINATORS `count_where`/`permutation_of` under #182 — the contract sublanguage `S_C` is now 8/8 construct classes proven; the EXEC-EXPRESSION sublanguage `S_E` is kernel-checked under #171 — Layer 2 OPENED) | blocker #170 (the FULL S_C proof: the contract sublanguage 8/8 is COMPLETE under #182; the EXEC-EXPRESSION `S_E` is COMPLETE under #171 (Layer 2 opened); what remains for the FULL `∀ P` is general USER-ADT match/is + the exec-BODY `S_B`, #172, + loops #163). (T1) `∀ P, ⟦R(P)⟧ = ⟦P⟧_S` is STATED precisely above + written out CONCRETELY for two cases (AC-2). **Proved-so-far:** `theorem ref_sound (fuel : Nat) (e : Expr) (env : Env) : refDenote fuel e env ↔ denote fuel e env` in `lean/Thermite/Soundness.lean` — kernel-checked, the non-spec-fn fragment by structural recursion + the #181 spec-fn calls by WELL-FOUNDED recursion on `(fuel, sizeOf e)` (`termination_by`/`decreasing_by`, core Lean; mutual `ref_sound`/`ref_sound_arms`, fuel-MATCHED in the header for `specCall` so the measure sees the `n+1`→`n` decrease), NON-VACUOUS (`refDenote`/`refIntVal`/`refSeqVal` via the encoder's maps/dispatch vs `denote`/`intVal`/`seqVal`'s source meaning, defined in separate modules), over: (i) **#170** the comparison/logical fragment (`Eq/Ne/Lt/Le/Gt/Ge/And/Or/Not`; the negative `eq_le_infidelity_breaks_soundness` shows an `Eq→<=` map BREAKS soundness — the boss's `==`-vs-`<=` teeth); (ii) **#176** the ARITHMETIC operators (`Add/Sub/Mul/Div/Rem/Shl/Shr/BitAnd/BitOr/BitXor`) over the unbounded-`int` spec domain (NO wraparound — overflow is the exec-side #171 obligation), via the shared `arithDenote` + the `encArith`/`tokArith` operator-map round-trip (`tokArith_encArith`); the PARTIALITY of `Div/Rem/Shl/Shr` (zero divisor/shift) is a SOURCE precondition (L0), modelled with Lean's TOTAL `Int` ops under the divisor-≠0 convention, held CONSISTENT between `denote` and `refDenote` (so T1 is insensitive to the partial point); (iii) **#177** the CASTS (`as u64/u32/usize/nat/int`) via the shared `castDenote` + the `encCast`/`tokCast` cast-target round-trip (`tokCast_encCast`), `as nat` = `Int.toNat` under a `≥0` source frame; (iv) **#178** the SPEC-CONTEXT REWRITES — slice→`@` (`seqVar`/`strVar` denote the SAME sequence — the `@`-view is the identity), `xs[i]`→`xs@[i]` (`Expr.idx` → the shared `seqIdx`), `&xs[..i]`/`&xs[a..b]`/`&xs[a..]`→`xs@.subrange(..)` (`Expr.subrange` + `RangeArg` → the shared `seqSub`), and the #127 byte-view DISPATCH `s.byte_at(i)`→`s.spec_byte_at(i)` / `s.len()`→`s.spec_len()` (`Expr.byteAt`/`Expr.seqLen` → the dispatch token `VerusByteView`/`byteView`, round-trips `byteView_encByteAt`/`byteView_encLen`); the sequence env is `structure Env { ints, seqs }`, the access partiality (`xs[i]` in-range) a SOURCE precondition modelled with total `List.getD`/`take`/`drop`, held consistent across both denotations — all PROVEN denotation-preserving via the combined `refVal_eq`. **#122/#146 RETIRED:** the negative `cast_paren_drop_breaks_soundness` proves a paren-DROPPED encoder (`(n-1) as nat` → `n - 1 as nat` re-parsing as `n - (1 as nat)`) DISAGREES at `n=-1` (`0 ≠ -2`). **#127 RETIRED on the contract side:** the negatives `byteview_wrong_index_breaks_soundness` (a wrong byte-view INDEX `s.spec_byte_at(0+1)` reads byte `20`≠ faithful `10`) and `byteview_misdispatch_breaks_soundness` (a wrong RECEIVER-METHOD — `byte_at` mis-dispatched to `spec_len` — reads length `3`≠ byte `10`) PROVE a faulty byte-view dispatch breaks T1 at a concrete sequence env (`s := [10,20,30]`); the faithful `encByteAt`/`encLen` dispatch is exactly what makes `ref_sound` hold. (v) **#179** the 6 BOUNDED-QUANTIFIER COMBINATORS (`forall_in`/`exists_in`/`sorted`/`forall_below`/`forall_from`/`disjoint`) — each denotes its FROZEN `verus_l3` quantifier form (`thermite-spec/src/combinators.rs`, matched EXACTLY: `forall_in` = `∀i, 0≤i<s.len() → p(s[i])`, `exists_in` = `∃i, 0≤i<s.len() ∧ p(s[i])`, `sorted` = `∀i j, 0≤i≤j<s.len() → s[i]≤s[j]`, `forall_below` = `∀i, 0≤i<n ∧ i<s.len() → p(s[i])`, `forall_from` = `∀i, n≤i<s.len() → p(s[i])`, `disjoint` = `∀i j, (0≤i<a.len() ∧ 0≤j<b.len()) → a[i]≠b[j]`), with each ARG threaded PER ITS REGISTRY ARG-KIND (`CombinatorSig.arg_kinds`, FAITHFUL to `thermite-tv/src/ref_encode.rs`'s `encode_combinator_call`/`encode_combinator_arg`): Slice→the `@`-view (`refSeqVal`), Index→a SCALAR `int` (`refIntVal`, NOT a slice `@`-view — the #145 fix), Pred→the flat-predicate closure body re-encoded by the same recursion and applied at the i-th element via the SHARED `Env.bindInt` (`Ast.lean` `inductive CombName` + `Expr.comb`/`Pred`; `Denote.lean`/`RefEncode.lean` the `comb` denotation arms; `Soundness.lean`'s `comb` case of `ref_sound`/`refVal_eq`, the predicate-equivalence threaded by the recursive `ref_sound` IH on the FLAT closure body + `forall_congr'`/`exists_congr`). The combinator BODY (the quantifier form) is the SHARED registry ground truth (`encode_combinator_call` reuses `lookup(C).verus_l3` verbatim), so the soundness CONTENT is the per-arg-kind threading (the #145 class). The 2 RECURSIVE/AGGREGATE combinators `count_where`/`permutation_of` are SHIPPED under #182 (see (viii) below — CORE Lean sufficed, NO Mathlib). **#179 wrong-combinator RETIRED:** the negative `wrong_combinator_breaks_soundness` proves emitting `exists_in` for source `forall_in` DISAGREES at `s := [10,20,30]`, `|x| x≤15` (source `∀` FALSE — `20>15`; wrong `∃` TRUE — `10≤15`). **#145 arg-kind RETIRED on the contract side:** the negative `index_argkind_slice_view_breaks_soundness` proves slice-`@`-viewing `forall_below`'s `ArgKind::Index` bound `n` (reading `n@.len()`=3 instead of the scalar `n`=1) DISAGREES at `s := [10,20,30]` (faithful scalar-bound TRUE; #145-buggy view-length-bound FALSE — `20>15` at `i=1`); the faithful `encode_index_value` SCALAR threading is exactly what `ref_sound`'s `comb`/`forallBelow` arm pins (via `refIntVal_eq_intVal`). (vi) **#180** the MATCH-IN-ENS / `is` PAYLOAD-IN-CONTRACT forms (the C7 class, `.design/basis/09-option-result.md`) — `match scrut { Some(v) => P(v), None => Q }` (and the `Ok`/`Err` Result form) + `scrut is Some/None/Ok/Err`, FAITHFUL to `thermite-tv/src/ref_encode.rs`'s `encode_match`/`encode_pattern` (the #150 work) + the `Expr::Is` arm: a built-in `Option`/`Result` scrutinee denotes an `OptResVal` (`none`/`some v`/`ok v`/`err e`, the payload an `Int` — the C7 corpus shape; `Denote.lean` `inductive OptResVal` + `Env.optres`), the `match` denotes the arm SELECTED by the scrutinee's variant with the payload BOUND via `Env.bindInt` (`Denote.lean`/`RefEncode.lean` `denoteArms`/`refDenoteArms` — STRUCTURALLY identical, the Verus `match` selection reused verbatim; the soundness content is the scrutinee/body encoding via the SAME recursion + the pattern's variant/binder choice from `encode_pattern`), the `is`-test denotes the variant discriminant (`OptResVal.isVariant`). Proved in the MUTUAL `ref_sound`/`ref_sound_arms` (`Ast.lean` `inductive Variant` + `Expr.{optResVar,match_,is_}` + the mutual `MatchArm`; the `match_` case threads `ref_sound_arms`, the arm-walk soundness via the recursive `ref_sound` IH on each arm body). **#180 match-arm-swap RETIRED:** the negative `match_arm_swap_breaks_soundness` proves a `Some`/`None` arm-body SWAP DISAGREES at `result := Some 7` (source `Some(v) => v==7` TRUE; swapped `Some(v) => false` FALSE). **#180 wrong-`is`-variant RETIRED:** the negative `is_wrong_variant_breaks_soundness` proves `is Some` tested as `is None` DISAGREES at `result := Some 7` (TRUE vs FALSE). Positives `match_faithful_is_sound`/`match_result_faithful_is_sound` (Option AND Result)/`is_faithful_is_sound` confirm the faithful encoder is sound. GENERAL USER ADTs are SCOPED OUT (honest): `encode_pattern`'s `is_builtin_variant` gate `Err`s on a user variant (no enum-qualification map), so user ADTs are OUT of what the encoder produces → not in `S_C` here, DELIBERATELY not embedded (no embed-then-`sorry`). (vii) **#181** the NAMED SPEC-FN CALLS — incl. WELL-FOUNDED RECURSION (the design `⟦Call(f,args)⟧ = ⟦body_of(f)⟧[params ↦ ⟦args⟧]`, "well-defined because §4.2 mandates a `dec` measure"). A `specCall name args` (`ast.rs` `Expr::Call` for a non-combinator/non-`old` callee — `ref_encode.rs::encode_call`'s case (3), which emits `name(<encoded args>)` and does NOT inline the body) resolves `name` in a SHARED `Registry` (`Ast.lean` `structure SpecFn { params, body }` + `abbrev Registry := String → Option SpecFn`, carried in `Env.specs`), binds the params to the denoted args (`Env.bindParams`), and denotes the BODY (an `Expr` of the SAME fragment, MAY recurse via further `specCall`s). The well-founded denotation is FUEL-INDEXED (`denote`/`intVal`/`refDenote`/`refIntVal` all take `fuel : Nat`; a `specCall` consumes one unit `fuel+1 → fuel`, a structural subterm keeps the same fuel — well-founded on `(fuel, sizeOf e)`, core Lean, NO Mathlib). This is the FULLY-GENERAL recursive-registry soundness (path 1): `ref_sound` is proved for ALL fuel and ALL registries (arbitrary recursion), with the SOURCE and ENCODER SHARING the fuel + registry — so it is NOT a fuel-cap vacuity dodge (T1 holds at EVERY fuel, including the fuel-`0` shared bottom where both sides denote the IDENTICAL default; the `dec`-bounded source spec fn terminates so a real call always reaches its fixpoint at some fuel). The call-site soundness is the GENERIC theorem "the args agree (the `refIntValArgs_eq`/`refVal_eq` IH, args in order) + the SAME registry resolves the SAME body, denoted at the SAME fuel" — `Ast.lean` `Expr.specCall` + `SpecFn`/`Registry`; `Denote.lean`/`RefEncode.lean` the fuel-indexed `intVal`/`refIntVal`/`denote`/`refDenote` `specCall` arms + `intValArgs`/`refIntValArgs` + `Env.bindParams`; `Soundness.lean` the mutual `refVal_eq`/`refIntValArgs_eq` + the `specCall` cases of `ref_sound`/`refVal_eq`. **#181 wrong-arg-order RETIRED:** the negative `specfn_arg_order_breaks_soundness` proves `sub_fn(b, a)` for source `sub_fn(a, b)` (`sub_fn(p,q) = p-q`, NON-commutative) DISAGREES at `a:=1,b:=2` (faithful `-1` vs swapped `1`). **#181 wrong-resolution RETIRED:** the negative `specfn_wrong_resolution_breaks_soundness` proves resolving the call to `add_fn` where the source resolves to `sub_fn` DISAGREES (`-1` vs `3`). The recursive-denotation NON-VACUITY is witnessed by `specfn_nested_resolution_value` (`g(p) = sub_fn(p,1)`; `g(5)` unfolds through TWO registry entries at fuel `2` to the genuine `4`, NOT the fuel-`0` default `0`) + the positive `specfn_call_faithful_is_sound (fuel : Nat)` (the faithful call sound at EVERY fuel). (viii) **#182** the 2 RECURSIVE/AGGREGATE COMBINATORS — the LAST contract brick, COMPLETING the closed 8-combinator set 8/8: (1) `count_where(s, p)` — a VALUE-combinator (`ResultKind::Usize`, threads `intVal`/`refIntVal` NOT `denote`), the recursive `nat` COUNT, modelled FAITHFULLY to the frozen `verus_l3` (`combinators.rs`, matched EXACTLY: `if s.len()==0 {0} else {(if p(s[0]) {1} else {0}) + count_where(s.drop_first(), p)}`) by the SHARED `countWhereVal` — STRUCTURAL recursion over the source `List` (core Lean, NO Mathlib, NO fuel: the list shrinks by `List.tail` mirroring `drop_first`/`decreases s.len()`), the per-element predicate the closure body via `denote`/`refDenote` at the element (`Env.bindInt`), using `Classical` decidability for the `if p(s[0])` test; (2) `permutation_of(a, b)` — a `Prop`-combinator (two slices, no predicate, like `disjoint`), MULTISET equality `a.to_multiset() == b.to_multiset()` (matched EXACTLY) modelled via the COUNT-CHARACTERIZATION `permEq a b := ∀ x, a.count x = b.count x` (core `List.count` — NOT Mathlib's `Multiset`; this IS multiset equality). Both reuse `Expr.comb` with the new `CombName.{countWhere,permutationOf}`; proved in the MERGED mutual block `refVal_eq`/`refIntValArgs_eq`/`ref_sound`/`ref_sound_arms` (the `count_where` case threads `countWhereVal_congr` + the recursive `ref_sound` IH on the flat closure body; `permutation_of` reduces to the two slices agreeing via `refVal_eq`). **#182 count_where wrong-predicate + off-by-one RETIRED:** the negatives `count_where_wrong_pred_breaks_soundness` (count of `|x| x≤15` over `[10,20,30]` is `1`, of `|x| x≤25` is `2` — DISAGREE) and `count_where_off_by_one_breaks_soundness` (`1 ≠ 1+1`); the genuine recursive count is `count_where_value_is_one` (`1`, NOT a vacuous bottom). **#182 permutation_of MULTISET-vs-SET RETIRED (the KEY fidelity check):** the negative `permutation_set_model_breaks_soundness` proves the CANONICAL witness `a := [1,1,2]`, `b := [1,2,2]` — SAME set `{1,2}` (the SET model `permSetModel` is TRUE) but DIFFERENT multisets (`count 1` is `2` vs `1`, so the faithful `permEq` is FALSE) — DISAGREE, PROVING `permutation_of` is `to_multiset()` equality NOT set equality. Positives `permutation_faithful_is_sound` + `permutation_true_on_real_permutation` (`[1,2,3]`~`[3,1,2]` via `List.Perm.count_eq`) + `count_where_faithful_intval_matches_source` confirm the faithful encoder is sound and the models are non-vacuous. `#print axioms ref_sound → [propext, Classical.choice, Quot.sound]` (no `sorryAx`/custom axiom; `Classical.choice` enters via `count_where`'s decidable predicate test — standard). CORE Lean only (no Mathlib — the expected Mathlib wall did NOT materialize; `List.count`/`List.Perm`/structural recursion sufficed). The contract sublanguage `S_C` is now 8/8 construct classes proven; the remaining `S_C` construct (general user-ADT match/is) is NOT yet embedded (no `sorry`). **(ix) #171 — LAYER 2 OPENED: the EXEC-EXPRESSION sublanguage `S_E` + `exec_ref_value` proven SOUND.** This is a DIFFERENT semantics from `S_C`, kept in a SEPARATE module/namespace `Thermite.Exec` (`lean/Thermite/Exec.lean`): `S_E` is the BOUNDED EXECUTABLE value, NOT unbounded `int`. `theorem exec_ref_sound (e : ExecExpr) (env : ExecEnv) : execRefValue e env = execDenote e env` — kernel-checked by STRUCTURAL recursion over the pure-exec subset (`#print axioms → [propext, Quot.sound]`, no `sorryAx`/custom axiom, CORE Lean only), NON-VACUOUS (`execRefValue` threads each construct through the encoder's `binop_str`/`cast_target` maps — `tokArith ∘ encArith`, `tokCast ∘ encCast` — vs `execDenote`'s source bounded ops `evalArith`/`castVal`, defined independently; the round-trips `tokArith_encArith`/`tokCast_encCast` are the content). FAITHFUL to `thermite-tv/src/exec_encode.rs::exec_ref_value`: the value domain is `ExecVal = .int BVal | .bool Bool` where `BVal { ty : IntTy, value : Int }` carries its type's BOUND `2^width` (the issue's "the Int value together with its type's bound"); int/bool lit, var, arith (`Add/Sub/Mul/Div/Rem/Shl/Shr/BitAnd/BitOr/BitXor`), cmp (`Eq/Ne/Lt/Le/Gt/Ge`), logical (`And/Or`), `!`, cast (`as u8/u16/u32/u64/usize`), slice-index — exactly the nodes `exec_encode.rs::encode` admits. **THE THREE `S_E ≠ S_C` FIDELITY PROPERTIES (the issue title):** (1) **BOUNDED** — a value is well-formed only in `[0, 2^width)` (`BVal.inRange`); (2) **OVERFLOW AS A PROOF OBLIGATION carried alongside the value** — `evalArith op a b : Option BVal` returns `some r` only when the mathematical result is in range (the no-overflow obligation DISCHARGED) and `none` on overflow (the obligation FAILS — the value is not defined, exactly a Verus exec `+` rejected because overflow is possible; the obligation is named `arithObligation`); div/shift-by-zero is a SOURCE precondition (`rawArith → none`); an out-of-range index is the bounds obligation; (3) **NEVER nat-coerced** — a cast WRAPS at the target width (`castVal t v = v.value % 2^t.width`, stays a BOUNDED value), it does NOT inject into an unbounded nat; there is NO `nat`/`int` cast token (`CastTok` = `u8/u16/u32/u64/usize` only). **The OVERFLOW-OBLIGATION treatment is GENUINE (not silently unbounded):** `add_overflow_has_no_value` proves `a + b` with `a = 2^64-1, b = 1` (both `u64`) has `execDenote = none` (a silently-unbounded model would return `some (2^64)`); `add_in_range_has_value` proves the non-overflowing `1+1=2` HAS its value (the partiality is the obligation, not blanket); `encoder_agrees_on_overflow` proves the encoder carries the SAME `none` (neither masks nor invents). **NEGATIVE LEMMA — the "never nat-coerced" discipline PROVEN (mirrors `cast_paren_drop_breaks_soundness`):** `nat_coercion_underflow_breaks_soundness` proves a NAT-COERCED `a - b` (the forbidden encoding `(a-b) as nat`, modelled `subNatCoerced` via `Int.toNat` which CLAMPS the underflow to `0`) produces `some (.int ⟨u64, 0⟩)` at `envUnderflow` (`a:=0, b:=1`, so `0-1=-1` underflows `u64`) while the faithful bounded `S_E` produces `none` (`sub_underflow_has_no_value` — the underflow obligation fails); `some 0 ≠ none`, so a nat-coercing encoder does NOT satisfy `exec_ref_sound`. The positives `sub_faithful_is_sound`/`slice_index_faithful_is_sound`/`slice_index_value_is_twenty` confirm the faithful encoder is sound + the index value is the genuine element `20` (non-vacuous). Honest deferral (no embed-then-`sorry`): method calls / Vec-String accessors (`exec_ref_value` `Err`s — #154/#156), the exec-BODY statement forms (`let`/`if`/loops/mutation) are increment 2b #172 / 2c #163, and a non-path callee / non-slice index / slice-RANGE index (`exec_ref_value` `Err`s) — all OUT of the pure-exec EXPRESSION subset `S_E`, NOT modelled. **The exec-BODY `S_B` (2b #172) + loops (2c #163) remain.** |
| REQ-3 (T2 — semantic preservation, as forward simulation) | NOT-STARTED | open blocker #174 (compose). (T2) `[Z3 ⊢ lower(P) ⟺ R(P)] ∧ (T1) ⟹ ⟦lower(P)⟧ = ⟦P⟧_S` is STATED in forward-simulation vocabulary (the relation `∼`, observable = caged fragment + `fx` rows; Leroy/CompCert finding #2) with its one-line modus-ponens proof + its `{Z3, S}` relativity + the reduced-trusted-base coverage boundary (AC-3/AC-4, finding #3). Contingent on (T1) (#170/#171/#172). Unbuilt because (T1) is unbuilt. |
| REQ-4 (the increment roadmap — Lean-targeted, honest, multi-cycle) | SHIPPED | the spec-first ordering table above ((a) #170 → (b) #171 → (c) #172 → (d) #174), all four now Lean 4 proofs, with per-increment rationale + the honest-magnitude statement (≈ one-pass effort, Necula finding #4). The plan is authored + the blockers are filed (#170–#174 all open). Non-doc consumer: the five filed blockers reference back to this doc; increment (a) #170 is teed up as the next build. |
| REQ-5 (the tooling decision — Lean 4 / Mathlib / Lean-SMT, COMMITTED) | SHIPPED | the decision is RECORDED above with its TCB-shrink rationale (Lean-SMT cvc5 reconstruction = the Z3-demotion path, finding #8) and Coq / Isabelle / Verus-native recorded as considered-and-deferred with one-line reasons. Blocker #173 RESOLVED (the human's call, per the #173 result comment). The DECISION is taken; this is no longer an open fork. |
| REQ-6 (the Lean project setup — pinned for the builder) | SHIPPED (for the comparison/logical fragment; increment (a) opening move, #170) | The `lean/` project EXISTS and `lake build` kernel-checks clean (NO `sorry`/`axiom`/`admit`/`native_decide`). Files: `lean/lean-toolchain` (`leanprover/lean4:v4.30.0`), `lean/lakefile.toml` (library `Thermite`), `lean/Thermite.lean` (root) + the four modules `Thermite/Ast.lean` (the `inductive Expr` for the comparison/logical fragment — `intLit`/`boolLit`/`var`/`cmp`/`logic`/`neg`, mirroring the `BinOp::{Eq,Ne,Lt,Le,Gt,Ge,And,Or}`/`UnaryOp::Not` arms of `thermite-syntax/src/ast.rs`), `Thermite/Denote.lean` (`def denote`/`def intVal` — the source `S_C` meaning), `Thermite/RefEncode.lean` (`def refDenote` via `encOp`/`encLog` mirroring `ref_encode.rs::binop_str`), `Thermite/Soundness.lean` (`theorem ref_sound`/`ref_sound_eq` + the negative `eq_le_infidelity_breaks_soundness` + — #176/#177 — `tokArith_encArith`/`tokCast_encCast`/`refIntVal_eq_intVal` and the cast-paren negative `cast_paren_drop_breaks_soundness`). **#176/#177 EXTENSION:** `Ast.lean` += `inductive ArithOp` (the 10 arithmetic ops) + `inductive CastTy` (`u64/u32/usize/nat/int`) + the `Expr.arith`/`Expr.cast` constructors (mirroring `BinOp::{Add..BitXor}` + `Expr::Cast`/`Type`/`PrimType`); `Denote.lean` += `def arithDenote`/`def castDenote` (the SHARED int-meaning/coercion, routed through by both denotations) and `intVal`'s `arith`/`cast` arms; `RefEncode.lean` += `encArith`/`encCast` (mirroring `ref_encode.rs::binop_str` arithmetic arms + `cast_target`) + `tokArith`/`tokCast` + `refIntVal`'s `arith`/`cast` arms (FAITHFUL to `encode_binary`'s whole-binary paren + `encode_cast`'s `({inner}) as {target}` paren — the #122 discipline). The Rust↔Lean correspondence gap is stated (above). **#178 EXTENSION (the spec-context rewrites):** `Ast.lean` += a MUTUAL `Expr`/`RangeArg` block with the sequence/index/byte-view constructors `seqVar`/`strVar` (a free `&[u32]`-slice / `String`-bytes SEQUENCE name), `idx` (`xs[i]`), `subrange` (`&xs[..i]`/`&xs[a..b]`/`&xs[a..]` via `RangeArg.{rangeTo,range,rangeFrom}`), `seqLen` (`.len()`), `byteAt` (`.byte_at(i)`) — mirroring `Expr::{Index,Ref,MethodCall}`/`IndexArg`; `Denote.lean` += `structure Env { ints, seqs }` (the SEQUENCE env), `seqIdx`/`seqSub` (the shared total access/subrange under the in-range source frame) + a mutual `seqVal`/`intVal` (the source `@`/element/prefix/byte/length meanings); `RefEncode.lean` += `VerusByteView` + `encByteAt`/`encLen`/`byteView` (the #127 DISPATCH as an explicit step) + a mutual `refSeqVal`/`refIntVal` (FAITHFUL to `ref_encode.rs::{encode_slice_arg,encode_index,encode_ref,encode_string_byteview}` — the `@`-view identity, `recv[idx]`, `recv.subrange(..)`, the `spec_byte_at`/`spec_len` dispatch); `Soundness.lean` += the combined `refVal_eq` (mutual structural recursion) + `byteView_encByteAt`/`byteView_encLen` round-trips + the #127 negatives `byteview_wrong_index_breaks_soundness`/`byteview_misdispatch_breaks_soundness` + positive witnesses `byteat_faithful_intval_matches_source`/`subrange_index_faithful_matches_source`. **HONEST DEVIATION from this REQ's dependency list:** still Lean 4 CORE ONLY (no Mathlib, no Lean-SMT) — the arithmetic+cast+rewrite fragment is provable with core `Int`/`Nat`/`Bool`/`Prop`/`List` + `simp`/`rfl`/`cases` (the byte-view uses core `List.getD`/`take`/`drop`/`length`, no Mathlib needed); Mathlib/Lean-SMT are added when a later increment's proof (the recursive combinators `count_where`/`permutation_of`, #182) genuinely needs them. **#179 EXTENSION (the 6 bounded-quantifier combinators):** `Ast.lean` += `inductive CombName` (the 6 frozen bounded combinator names — the 2 recursive `count_where`/`permutation_of` DELIBERATELY ABSENT, #182) + a MUTUAL `Pred` (the flat `|x| <body>` closure) + the `Expr.comb` constructor (carrying the slice / optional second slice / optional SCALAR index / optional predicate per `CombinatorSig.arg_kinds`); `Denote.lean` += `Env.bindInt` (the SHARED predicate-at-element env update) + `denote`'s `comb` arm (the frozen `verus_l3` quantifier forms); `RefEncode.lean` += `refDenote`'s `comb` arm (the SAME quantifier form, args threaded per ARG-KIND — `refSeqVal` Slice / `refIntVal` SCALAR Index #145 / `refDenote`-of-body Pred — FAITHFUL to `encode_combinator_call`/`encode_combinator_arg`); `Soundness.lean` += the `comb` case of `ref_sound`/`refVal_eq` + the negatives `wrong_combinator_breaks_soundness` (the wrong `∀`-vs-`∃` combinator) and `index_argkind_slice_view_breaks_soundness` (the #145 slice-viewed Index arg) + the positive witness `forall_below_faithful_is_sound`. **#180 EXTENSION (the C7 match-in-ens / `is` payload-in-contract forms):** `Ast.lean` += `inductive Variant` (the 4 built-in `Some/None/Ok/Err` — `ref_encode.rs::is_builtin_variant`; user variants DELIBERATELY ABSENT) + the `Expr.{optResVar,match_,is_}` constructors + a MUTUAL `MatchArm` (the variant pattern + optional payload binder + body — `Pattern::Enum` RESTRICTED to the built-in payload patterns); `Denote.lean` += `inductive OptResVal` (the `none`/`some v`/`ok v`/`err e` scrutinee value, the payload an `Int`) + `Env.optres` + `OptResVal.{variant,payload,isVariant}` (the shared Verus match/is discriminant) + `scrutVal` + a MUTUAL `denote`/`denoteArms` (the arm SELECTION + payload BINDING); `RefEncode.lean` += a MUTUAL `refDenote`/`refDenoteArms` (the SAME arm-selection structure — the Verus `match` reused verbatim — with each body via the encoder's `refDenote`; the `is`-test via `isVariant`, FAITHFUL to `encode_match`/`encode_pattern` + the `Expr::Is` arm); `Soundness.lean` += a MUTUAL `ref_sound`/`ref_sound_arms` (the `match_` case threads `ref_sound_arms`, the arm-walk soundness via the recursive `ref_sound` IH on each arm body; the `is_` case is the shared discriminant) + the negatives `match_arm_swap_breaks_soundness` (a `Some`/`None` body SWAP) and `is_wrong_variant_breaks_soundness` (`is Some` tested as `is None`) + the positives `match_faithful_is_sound`/`match_result_faithful_is_sound` (Option AND Result)/`is_faithful_is_sound`. Still Lean 4 CORE ONLY (the match/is fragment is provable with core inductives + `cases`/`by_cases`/`simp` — no Mathlib). **#181 EXTENSION (the named spec-fn calls — incl. well-founded recursion, increment 1e):** `Ast.lean` += `Expr.specCall (name : String) (args : List Expr)` (mirroring `Expr::Call` for a non-combinator/non-`old` callee) + `structure SpecFn { params : List String, body : Expr }` + `abbrev Registry := String → Option SpecFn` (the SHARED spec-fn registry, the external ground truth like the combinator `lookup`); `Denote.lean` += `Env.specs : Registry` + `Env.bindParams` (bind params to denoted args) + the FUEL-INDEXED mutual `seqVal`/`intVal`/`intValArgs`/`denote`/`denoteArms` (a `specCall` resolves `name` in `Env.specs`, binds params, denotes the body at the CONSUMED fuel; the well-founded `(fuel, sizeOf e)` recursion is auto-derived); `RefEncode.lean` += the fuel-indexed mutual `refIntVal`/`refSeqVal`/`refIntValArgs`/`refDenote`/`refDenoteArms` (the `specCall` arm FAITHFUL to `encode_call`'s case (3) — `name(<encoded args>)`, NOT inlined; the body is the shared registry entry); `Soundness.lean` += the fuel-indexed mutual `refVal_eq`/`refIntValArgs_eq` (`termination_by (fuel, sizeOf …)`/`decreasing_by Prod.Lex`) + the `specCall` cases of `ref_sound`/`refVal_eq` (fuel-MATCHED in the header so the measure sees the `n+1`→`n` decrease) + the negatives `specfn_arg_order_breaks_soundness` (wrong arg order, non-commutative `sub_fn`) and `specfn_wrong_resolution_breaks_soundness` (call resolves to the wrong spec fn) + the recursive-denotation witness `specfn_nested_resolution_value` (`g(5)` unfolds to `4` at fuel `2`, NOT the fuel-`0` bottom) + the positive `specfn_call_faithful_is_sound (fuel : Nat)`. Scoping PATH 1 (fully-general recursive registry): `ref_sound` is proved for ALL fuel + ALL registries (arbitrary recursion) — the fuel-indexing is NOT a vacuity dodge (source + encoder share the fuel; T1 is fuel-uniform). Still Lean 4 CORE ONLY (the well-founded recursion is core `termination_by`/`decreasing_by`; no Mathlib). **#182 EXTENSION (the 2 recursive/aggregate combinators — the LAST contract brick, 8/8 complete, increment 1d-ii):** `Ast.lean` += `CombName.{countWhere,permutationOf}` (the 8-combinator set now COMPLETE; `count_where` documented as VALUE-sorted, `permutation_of` as `Prop`/multiset, both reusing `Expr.comb`); `Denote.lean` += `noncomputable def countWhereVal` (structural recursion over `List Int`, faithful to the recursive `verus_l3`, `Classical` decidability for the `if p(s[0])` test) + `countWhereVal_cons` + `def permEq` (the count-characterization `∀ x, a.count x = b.count x` of multiset equality) + `intVal`'s `comb CombName.countWhere` arm (the value side) + `denote`'s `comb` `permutationOf`/`countWhere` arms (and EXPLICIT `termination_by (fuel, sizeOf …)` added to the now-`noncomputable` block); `RefEncode.lean` += the SAME `comb` arms (the two mutual blocks MERGED into one so `refIntVal`'s `count_where` arm can reference `refDenote` for the predicate; all `noncomputable`, explicit `termination_by`); `Soundness.lean` += `theorem countWhereVal_congr` (the count depends on the predicate only through its truth at each element) + the `count_where`/`permutationOf` arms of the now-MERGED `refVal_eq`/`ref_sound` block (the `count_where` case threads `countWhereVal_congr` + the recursive `ref_sound` IH on the flat closure body) + the negatives `count_where_wrong_pred_breaks_soundness`/`count_where_off_by_one_breaks_soundness` (wrong predicate / off-by-one count) and `permutation_set_model_breaks_soundness` (the MULTISET-vs-SET teeth, canonical witness `[1,1,2]`/`[1,2,2]`) + the positives `count_where_value_is_one`/`count_where_faithful_intval_matches_source`/`permutation_faithful_is_sound`/`permutation_true_on_real_permutation`. CORE Lean only — the expected Mathlib wall did NOT materialize (`List.count`/`List.Perm`/structural recursion sufficed); `Classical.choice` enters the axiom set via `count_where`'s decidable predicate test (standard, sanctioned). The remaining deferred AST construct (general USER-ADT match/is beyond the built-in Option/Result) is listed in `lean/Thermite/Ast.lean` as a future sub-increment — NOT embedded-then-`sorry` (forbidden), simply left out of the inductive. |
| REQ-7 (the architecture decision — a verified validator) | SHIPPED | the decision is RECORDED above (REQ-7 section): a VERIFIED VALIDATOR (prove `R` sound = T1), NOT a verified lowerer, NOT a full universal simulation proof — Leroy finding #1 + Necula finding #4. This resolves the doc's own open architectural question (research open-question #2). Recorded in #173's result comment. Non-doc consumer: the #173 resolution + the increment blockers #170–#174 build to this architecture. |
