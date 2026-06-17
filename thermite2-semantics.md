<!--
  thermite2-semantics.md — the single normative home for the shipped Stage-1
  semantics (.design/thermite2-program.md REQ-8 / AC-13).
  Baseline: dollspace-gay/Thermite @ c46da3ac or later (the program re-baseline).
  Authority: thermite-design.md (the product thesis) → .design/thermite2-program.md
  (the program umbrella) → this doc (the Stage-1 semantics of record) → the shipped
  forge/lean/thermite-lower code it consolidates.
-->

# Thermite 2 — Stage-1 normative semantics

This document is the one authoritative place the shipped Stage-1 forge-tier
semantics live. It consolidates the conventions that were stated across module
headers in `forge/src/`, `lean/Thermite/`, and `thermite-lower/src/` into one
normative home, so a reader has a single reference and each module header points
here rather than restating the convention (AC-13).

It is grounded in the code shipped on `main`: every section names the symbol that
implements the convention and quotes the load-bearing fragment. Where a convention
is enforced by a test or a structural seam, that enforcement is named so the claim
is checkable, not asserted.

The five areas it fixes:

1. the L0..L4 assurance ladder, with the L3 (solver) / L4 (kernel-grounded)
   boundary and the upward forge-escalation (§1);
2. the seven certificate verdicts and the never-converts-silently invariant (§2);
3. the frozen-subset / cage fragment and the relaxable-clause fragment (§3);
4. the div/rem and cast arithmetic conventions — the audit F2 corners, stated once
   here (§4);
5. the covenant (inhabit/falsify, covenant-before-burn) and the trust profiles
   (solver vs kernel-grounded) (§5).

This doc states semantics; it does not re-specify the program sequencing
(`.design/thermite2-program.md` owns that) nor the per-component REQ tables
(`.design/stage1-forge-tier.md` and the `.design/reqs/` registry own those).

---

## 1. The assurance ladder L0..L4

The assurance level is `manifest::Level`, serialized to `"L0".."L4"` to match the
golden certificate's `"level"` field (`thermite-design.md` §6). The five rungs:

| rung | meaning | trust character |
|---|---|---|
| **L0** | unverified / `#[slag]` escape hatch (§6, §8) | none |
| **L1** | executable runtime check compiled in (§6) | runtime |
| **L2** | bounded model check (Kani) (§6) | bounded |
| **L3** | SMT proof: the contract holds for all inputs (§6) | **solver** |
| **L4** | kernel-grounded proof (the relax route, increment 2f) | **solver + kernel** |

The declaration order is the ladder order, `L0 < L1 < L2 < L3 < L4`, made the
`Ord` the assurance-manifest aggregate uses: a project's headline level is the
minimum over its functions, and a function's level is the minimum over its clauses.
`Level` derives `PartialOrd, Ord` for exactly this aggregate.

### 1.1 The L3 / L4 boundary

L3 and L4 are both "proven for all inputs", and they differ in *what grounds the
proof*:

- **L3 is a solver rung.** The contract is discharged by Verus/Z3 (the default
  engine) or by Lean (`--engine lean`). The trust base is the solver's (§5.2).
- **L4 is kernel-grounded.** It is the relax route's nlsat (QF_NRA)
  real-relaxation discharge, whose trust profile is
  `solver(nlsat) + spine-lemma(kernel)`. The `manifest::Level::L4` doc states it:

  > L4 is the **kernel-grounded** rung the Stage-1 forge tier's relax route adds
  > […]: where L3 is a Verus/Z3 SOLVER proof, L4 is a clause whose trust profile
  > is `solver(nlsat) + spine-lemma(kernel)` — the nlsat real-relaxation
  > discharge, sound by the kernel-checked spine lemmas `r_relax_sound` +
  > `rencode_sound` (`lean/Thermite/Relax.lean`).

L4 is **additive**: it is reached only by the new relax engine route
(`--engine nlsat`, and per-clause via `--engine forge`), never by the default
Verus path. So the v1 conformance corpus stays at L3 and the `oracle_subset` of
every v1 golden certificate is byte-identical.

### 1.2 Upward forge-escalation

The pre-Stage-1 discipline degraded an item that fell outside the cage *down* the
ladder (L3→L2→L1). The forge tier inverts that for out-of-cage clauses: they do not
degrade down — they **escalate up** to the engine that grounds them. A relaxable
clause Verus cannot close routes up to the nlsat route and certifies at **L4, above
L3** (`manifest::Level::L4`):

> Out-of-cage clauses no longer degrade DOWN the ladder — they escalate UP to the
> forge, and a relax-discharged clause certifies at L4 above L3.

The per-clause hybrid route (`--engine forge`, `check::forge_gate_check` /
`forge_gate_item_cert`) classifies each `ens` clause and sends it to its grounding
engine: a relaxable clause to the nlsat route (L4), a non-relaxable clause to the
author-proof Lean discharge (L3 + burn). The item certifies at the minimum over its
clauses.

The degrade-loudly discipline (`forge/src/degrade.rs`) is unchanged: a solver
timeout or subprocess failure is still reported, never silently treated as success
(R-CODE-4). Escalation is about *which engine grounds a clause*, not about
softening a failure.

---

## 2. The seven certificate verdicts

The forge tier's outcome vocabulary is the closed seven-variant
`verdict::CertVerdict`, a **separate cert-level enum**, not arms of the three-arm
`engine::Verdict` (`Proven` / `Refuted` / `Unknown`). The seven:

| verdict | meaning | disposition |
|---|---|---|
| `Proved` | proven for all inputs at the engine's level | certify |
| `Counterexample { obligations }` | a witnessed countermodel | hard fail, never degrades |
| `RealWitness { point }` | a clause true over ℤ, false over ℝ | escalate up, never `Counterexample` |
| `CovenantRefuted { counterexample }` | a `falsify` hit | hard fail (Counterexample-class) |
| `Stuck { goals, hint }` | the proof elaborated but did not close | not certified |
| `KernelBudget { detail }` | a Lean elaboration/kernel-budget exhaustion | not certified |
| `Timeout { detail }` | a solver resource-limit (rlimit) exhaustion | not certified |

`CertVerdict::kind()` is the stable string each clause records in the schema-v2
block, and the seven `kind` strings are a distinct closed set (the
`all_seven_variants_round_trip` test asserts exactly seven distinct kinds).

### 2.1 Where each verdict comes from

Three of the seven are the image of the engine's three-arm `Verdict` under the
**total map** `CertVerdict::from_engine_verdict`, an exhaustive match with no
wildcard arm:

> `Proven → Proved`, `Refuted → Counterexample`, `Unknown → Timeout`. […] The map
> is total, so NO `engine::Verdict::Unknown` survives into a certificate.

The other four have no engine-level source; they are produced upstream at the forge
orchestration layer:

- **`RealWitness`** — the relax route. A real countermodel of a clause true over ℤ
  but false over ℝ, carrying the raw real point (`verdict::RealPoint`, the textual
  rationals nlsat returns). It escalates **up** to the forge; it is never demoted to
  `Counterexample` (the three-arm engine `Verdict` cannot carry a non-integer
  point).
- **`CovenantRefuted`** — the covenant check. A `falsify` hit (`req` held, the body
  violated `ens`), a hard fail in the degrade ladder with the same never-degrades
  treatment as `Counterexample`.
- **`Stuck`** — the frozen battery. A proof that elaborated but left a residual
  goal; carries the residual goal(s) and an optional missing-bridge hint.
- **`KernelBudget`** — the budget wrapper. A Lean elaboration/kernel-budget
  exhaustion (Q4: 30s/clause), detected by the textually-distinct Lean signal
  `tv_signal::is_kernel_budget_signal` (`(deterministic) timeout … maximum number
  of heartbeats` / `maximum recursion depth has been reached`), never confusable
  with the Z3 rlimit text.

`verdict::cert_verdict_for_lean` shows the upstream discrimination order: a Lean
budget exhaustion is classed `KernelBudget` first; a residual-goal failure is
`Stuck`; only a budget-less, residual-less incompleteness falls through to the
total engine map as `Timeout`.

### 2.2 The never-converts-silently invariant (R-VERDICT-1)

`Proved` is constructed **only** from `engine::Verdict::Proven`. No other path
produces `Proved` from a non-`Proven` value, and the total map guarantees no
`Unknown` is silently dropped — every `Unknown` becomes a recorded `Timeout` (or is
discriminated upstream into `KernelBudget` / `Stuck` before the map). A
`Refuted`/`Unknown` never yields `Proved`.

This is enforced **now**, in `forge/src/verdict.rs`:

- `from_engine_verdict` is an exhaustive match with no wildcard arm (a new engine
  arm would fail to compile rather than fall through to `Proved`).
- `engine_verdict_maps_totally_and_no_unknown_survives` asserts every engine arm
  maps to its image and both `Unknown` reasons become `Timeout`.
- `proved_is_constructed_only_from_proven` asserts `Proved` arises only from
  `Proven`.
- `lean_kernel_budget_is_upstream_not_timeout` asserts the `KernelBudget` / `Stuck`
  upstream discrimination.

The seven-verdict hermetic suite (`forge/src/seven_verdicts.rs`) exercises each
verdict at the boundary that produces it, so the closed set is covered end to end.

---

## 3. The fragments

Two fragments bound what the forge tier admits. They are independent: one bounds
the *proof moves* an author may cite, the other bounds the *clauses* the nlsat route
may attempt.

### 3.1 The frozen-subset / cage fragment

The cage is the closed set of proof moves an author `proof` / `lemma` block may use.
It is the **frozen tactic allowlist** in `forge/src/battery.rs`: a fixed set of
tactic names and simp-lemma names, the "auditable source of what the frozen battery
knows how to rewrite". A citation outside the allowlist is a hard, named error
(`battery::enforce`), never a warning:

> A proof citing an unlisted simp [lemma] […] is not in the frozen battery simp set
> (REQ-5). A HARD error, named — never a warning (R-BAT-1).

Freezing the proof-move set is what makes a closed proof auditable: a reader knows
the proof used only moves on the allowlist, and a proof that needs a move outside it
is surfaced (the `Stuck` verdict carries the residual and a missing-bridge hint)
rather than admitted silently.

### 3.2 The relaxable-clause fragment

The relaxable fragment (`forge/src/relax.rs`, `relax::classify_fn` /
`RelaxVerdict`) is the set of contracts the nlsat real-relaxation route may attempt.
A contract is **relaxable** iff it is a universally-quantified statement over
integer-scalar parameters whose clause atoms are *polynomial*:

> built only from variables, integer literals, `+`/`-`/`*`, the comparison
> relations, and the boolean connectives. Crucially it contains **no
> div/mod/shifts/casts** […]: those are not polynomial over ℝ (`/`, `%`), are
> bit-level rather than arithmetic (`<<`/`>>`/`&`/`|`/`^`), or change the carrier
> (`as`), so the real relaxation `∀ x : ℝ, …` would not faithfully encode the
> integer clause.

The quantifier is implicit: a `fn` contract is `∀ params, req → ⋀ ens`. A
non-relaxable contract is named by the first disqualifying construct
(`RelaxVerdict::NotRelaxable(String)`), so the route's skip is honest and the
auditor sees *why* — never a bare boolean (R-CODE-4).

What the route does with a relaxable contract: it hands nlsat the negation
`∃ vars : ℝ, req ∧ ¬(⋀ ens)`.

- `unsat` → no real counterexample → the relaxation holds → by the kernel-checked
  `r_relax_sound` the integer clause holds → certify at **L4**.
- `sat` → a real countermodel → the **integrality check** (Q8): round the real
  point into a radius-2 ℤⁿ box and test whether any integer point genuinely
  falsifies the integer clause. An integer falsifier is a real `Counterexample`; if
  none does, the countermodel is real-only (true over ℤ, false over ℝ) → a
  `RealWitness` escalation, never a `Counterexample`.

The two fragments meet in the per-clause hybrid route: a relaxable clause goes to
nlsat (L4); a non-relaxable clause goes to an author proof that must stay inside the
cage (L3).

---

## 4. The arithmetic conventions (the audit F2 corners)

These are the corners the external trust-audit's F2 finding flagged as needing one
normative statement rather than per-module restatement. They are stated here once.
The governing principle for both: a partial or width-sensitive operator carries its
side-condition as a **source-side obligation** (an L0 obligation discharged outside
the contract clause), and the spec denotation is chosen so that the source
(`denote`) and the encoder (`refDenote`) agree regardless of the side-condition's
value — the soundness theorem is about the operator map being faithful, not about
the partial-point value.

### 4.1 Division and remainder (Euclidean vs T-division)

`Div` / `Rem` (and the shifts `Shl` / `Shr`) are **partial in the source**: a zero
divisor or a zero/over-width shift is rejected as a source precondition, an L0
obligation discharged outside the contract clause (`ast.rs`: `BinOp::Rem`
"requires a nonzero divisor"). The spec denotation models them with Lean's **total
`Int` operations**, stated once in `lean/Thermite/Denote.lean`:

> We model them with Lean's total `Int` operations (`Int./` is Euclidean-ish /
> T-division; `Int.%` its companion). This is sound for `S_C` because: (i) the
> divisor-≠0 precondition is a source-side obligation, not part of the binop's
> contract meaning; (ii) […] `denote` and `refDenote` route the op through the same
> shared `arithDenote` function, so whatever total convention is chosen, both sides
> agree.

The normative statement: **the spec models `div`/`rem` with Lean `Int` division and
its companion remainder, and the divisor-nonzero side-condition is a source
obligation, not part of the binop's meaning.** Because both denotations share the
one `arithDenote`, the soundness equation (T1) holds regardless of which total
convention the partial point takes — the convention need only be *consistent across
the two denotations*, which it is by construction.

The executable evaluator (`forge/src/covenant_eval.rs`) computes the same operations
concretely for the `falsify` driver; a genuine divide-by-zero / shift-out-of-range
with no `ens` bearing is a `CovenantEvalError::Trap` (the input is skipped, not
counted as a refutation — `req` is expected to guard the partial operator).

### 4.2 Casts (value-preserving vs truncating; the no-overflow source obligation)

A cast has two readings, reconciled by a source obligation:

- **Spec (contract) position is value-preserving.** `as int` / `as u64` / `as u32`
  / `as usize` denote the value unchanged on the spec `int` domain; `as nat`
  injects into the naturals (a non-negative `int` to itself, a negative `int`
  clamped to `0` via `Int.toNat`, and an `as nat` always carries a `≥ 0` source
  frame so the clamp point is never the intended value). `lean/Thermite/Denote.lean`
  states it:

  > `as int`/`as u64`/`as u32`/`as usize` are value-preserving on the spec `int`
  > domain (the bounded cast carries its no-overflow frame as a source obligation,
  > like div-by-zero), so at the contract level they denote the value unchanged.

- **Exec position truncates.** In executable code (`forge/src/covenant_eval.rs`,
  the exec-TV surface, the L1 lowering) an `as` to an integer width reduces mod
  `2^bits`: `x as u32` is `x mod 2³²`.

The reconciliation is the **no-overflow source obligation**: the value-preserving
spec reading is sound exactly when the source establishes the cast does not
overflow its target width (the same shape as the divisor-nonzero obligation). The
covenant evaluator does not discriminate on the width-truncated value — it checks
the *agreement* between the body's computed value and what `ens` asserts, evaluating
both sides under the same `i128` model, so a truncation that one side computes the
other also computes (it can never manufacture a spurious refutation). The absolute
width-truncated value is the exec/L3 surface's concern, not the covenant's
discrimination target.

The normative statement: **a cast is value-preserving in spec position and
truncating in exec position; the two agree under the cast's no-overflow source
obligation, which the source carries as an L0 obligation.**

---

## 5. The covenant and the trust profiles

### 5.1 The covenant (inhabit / falsify, covenant-before-burn)

A covenant (RFC-1 §5) checks an item against its *own declared meaning* by executing
it. It is authored as a `witness { inhabit (…); falsify N; }` block that covenants
the `fn` it immediately follows in source order (`covenant_engine::witness_bindings`
computes the binding). The covenant has two moves, both in
`covenant_engine::analyze_covenant`:

- **`inhabit`** — an author-stated witness that must satisfy `req`. A covenant must
  carry at least one author `inhabit` witness (R-COV-1); a witness block with only
  `falsify` is refused, named (`CovenantError::NoAuthorWitness`). A witness that
  does not satisfy `req` is a loud error (`CovenantError::WitnessRefutesReq`), never
  silently dropped — the author claims a precondition inhabitant that is not one.
  Witnesses may augment the generator's inputs but cannot be the only ones.
- **`falsify`** — a search for an input that satisfies `req` but whose executable
  body violates `ens`. Such an input is a `CovenantCounterexample` and yields the
  `CertVerdict::CovenantRefuted` hard fail. The run rides the deterministic
  SplitMix64 generator (`thermite_tv::Rng`); the Q3 default is a fixed-seed
  `falsify 50_000` when the budget is unstated. The evidence (witness count,
  generated/refuted counts, seed) is deterministic and joins the certificate's
  oracle, so weakening a budget or dropping a witness changes a recorded number.

**Covenant-before-burn (R-COV-1)** is enforced **now**, structurally, in
`forge/src/covenant_engine.rs`. `covenant_gate` invokes the burn closure (the L3
proof search) **only** on a validated covenant; on a refutation or a malformed/absent
covenant it returns without invoking burn:

> invoke the `burn` closure (the L3 proof search) ONLY when the covenant validated;
> on a refutation or a malformed/absent covenant return WITHOUT invoking it. This is
> the closure-instrumented invariant in the [`degrade`] style — the proof-search
> path cannot start without a valid covenant record.

The `Engine::discharge` seam takes a non-optional `CovenantRecord`, so the
proof-search path cannot be entered without a covenant in hand — covenant-before-burn
is a type-level seam, not a runtime convention. The
`covenant_gate_never_burns_without_covenant` test proves the closure never runs on
the non-`Validated` arms.

### 5.2 The trust profiles (solver vs kernel-grounded)

When an engine says `Proven`, it adds a named **trust base** (`engine::TrustProfile`,
an enumerated set of named items) so an auditor can compare what each rung rests on.
The base is recorded per obligation (`ObligationResult::engine` / `trust`) and per
certificate (`Certificate::engine_attribution`). The base is **orthogonal to the
level**: L3 still means "proven for all inputs"; the trust base is the
auditor-visible refinement of *how*. The three shipped bases:

| engine | rung | trust base |
|---|---|---|
| Verus/Z3 (default) | L3 | `{Z3, Verus VC-gen, TV/lowering theorem}` |
| Lean (`--engine lean`) | L3 | `{Lean kernel, propext, Classical.choice, Quot.sound, EXP}` |
| nlsat (`--engine nlsat`) | L4 | `{Z3 nlsat (QF_NRA), r_relax_sound, rencode_sound}` (the last two kernel-checked) |

The distinction the profiles make visible:

- A **solver** profile (L3-via-Verus) rests on the SMT solver's soundness plus the
  VC-generation and lowering theorem. The clause is "proven" to the extent the
  solver and the lowering are trusted.
- A **kernel-grounded** profile rests additionally on a kernel-checked theorem. The
  Lean L3 base enumerates a smaller, named set (the kernel plus three standard
  axioms). The nlsat L4 base rests on Z3's nlsat real-arithmetic decision **plus**
  the kernel-checked relax spine lemmas `r_relax_sound` (real→integer relaxation
  soundness) and `rencode_sound` (real-encoding faithfulness), whose axiom footprint
  is probed to stay ⊆ `{propext, Classical.choice, Quot.sound}` (`scripts/audit.sh`
  check [1]).

An auditor reading a certificate sees not only the level but the enumerated base, so
an L4-via-nlsat clause and an L3-via-Verus clause are distinguishable by what their
proofs trust, which is the point of recording the profile rather than a bare level.

---

## Cross-references

- `thermite-design.md` — the product thesis and the §6 ladder / §7 battery / §8
  `#[slag]` framing this doc's Stage-1 conventions extend.
- `.design/thermite2-program.md` — the program umbrella (REQ-8 / AC-13); owns
  sequencing and gates.
- `.design/stage1-forge-tier.md` — the Stage-1 forge-tier design doc; owns the
  per-increment REQ tables.
- `.design/forge/degrade-ladder.md` — the ladder ordering and the degrade-loudly
  discipline, reconciled to the L0..L4 ladder here.
- The shipped code consolidated here: `forge/src/manifest.rs` (`Level`),
  `forge/src/verdict.rs` (`CertVerdict`), `forge/src/relax.rs` (the relaxable
  fragment), `forge/src/battery.rs` (the frozen tactic allowlist),
  `forge/src/covenant_engine.rs` (the covenant + covenant-before-burn),
  `forge/src/engine.rs` (`TrustProfile`), `lean/Thermite/Denote.lean` and
  `lean/Thermite/Ast.lean` (the div/rem + cast conventions),
  `lean/Thermite/Relax.lean` (the relax spine lemmas).
