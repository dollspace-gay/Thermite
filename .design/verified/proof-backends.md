# Proof-Backend Interface + Lean as Engine #2 — backend-neutral obligations over the mechanized semantics

<!--
tier: 3-component
status: draft (v-next architecture — the obligation/engine interface; most REQs NOT-STARTED
        behind build blockers. The SHIPPED substrates this builds on are quoted-code-grounded.)
governs: forge/src/check.rs + forge/src/degrade.rs + forge/src/manifest.rs (the discharge
         pipeline, the ladder, the certificate this interface generalizes) and
         thermite-tv/src/obligation.rs (the per-run obligation materialization that the
         backend-neutral Obligation artifact reifies) and lean/Thermite/** (the mechanized
         semantics the obligations are stated against, and the Lean engine's target).
         NO production .rs is added or changed by this doc — it is the interface/architecture
         layer, like .design/verified/thermite-semantics.md. The increments that BUILD it are
         the named build blockers (#204 = increment (i), the others future).
thesis-refs:
  - thermite-design.md §1 (trust relocated: code → spec → spec-intent; "a skeptical third party
    can audit in minutes"; the enumerable trusted base)
  - thermite-design.md §6 (the verification ladder L3/L2/L1/L0; "the certificate lists every
    function's level … this manifest IS the deliverable's trust statement"; downgrades automatic)
  - thermite-design.md §7 (the anti-Goodhart battery — mutation kill-ratio + vacuity)
  - thermite-design.md §9 (composition: trust invariant under composition, not multiplicatively
    decaying — the honest-min aggregation)
  - thermite-design.md §13 (roadmap; verified-microkernel convergence)
anchor-docs:
  - .design/verified/thermite-semantics.md (the mechanized semantics S — denote/bodyDenote/
    loopDenote, the fuel-indexed spec-fn registry; the verified-validator architecture; the
    reduced-trusted-base enumeration — REQ-1..REQ-7)
  - .design/verified/z3-demotion.md (what Lean-SMT/cvc5 reconstruction reaches TODAY — the
    auto-discharge fragment for the Lean engine's tactic battery)
  - .design/verified/rust-lean-correspondence.md (the arm-by-arm inspection-tier discipline +
    the drift tripwire — the exporter's faithfulness is the SAME correspondence class)
field-refs:
  - formal-methods-sota.md finding #8 (proof-PRODUCING SMT + reconstruction; Lean-SMT/cvc5)
  - formal-methods-sota.md finding #1 (verified validator; the trust-profile economy)
build-blockers:
  - increment (i): crosslink #204 (FILED — the Obligation artifact + the Engine trait in forge;
    Verus refactored behind the interface, behavior byte-identical, the conformance cert oracle
    unperturbed; no new engine)
  - increment (ii): FUTURE (the Lean exporter + auto-discharge for the PURE-CONTRACT class)
  - increment (iii): FUTURE (interactive proofs + per-obligation certificate attribution + the
    engine-generic anti-Goodhart battery)
  - increment (iv): FUTURE (the full exportable fragment — exec exprs, straight-line bodies,
    v1 while, spec-fns via the fuel registry)
-->

## Summary

Thermite should be defined by its SEMANTICS (`S`, mechanized in `lean/Thermite/`) with provers as
PLUGINS — not defined by Verus's verifiable fragment. Today the toolchain has exactly one engine
welded into `forge::check`: Verus/Z3, reached implicitly by `run_verus`, with the obligation
existing only transiently as the per-clause/per-body/per-loop Verus text `thermite-tv`'s
`equivalence_obligation` family emits. This doc designs (a) the **Obligation** — a serializable,
backend-neutral verification artifact stated against `S`; (b) the **Engine** interface (fragment /
discharge / trust profile / evidence); (c) **certificate attribution** so an auditor sees that L3
via Lean has a SMALLER trusted base than L3 via Verus; and (d) **Lean as engine #2** (an exporter
into the existing `lean/Thermite/` spine + an auto tactic battery + interactive proofs). Most of
this is NOT-STARTED behind build blockers; the substrates it generalizes (the obligation
materialization, the discharge pipeline, the degrade ladder, the content-addressed proof cache, the
project-min aggregate, the mechanized `S`) are SHIPPED and quoted below.

## Requirements

- **REQ-1 (the Obligation — the backend-neutral artifact)** — define a serializable verification
  artifact `Obligation { item, class, ast_slice, env }` stated against the MECHANIZED semantics
  (`S`), INDEPENDENT of any prover's input language. `class` ∈ the obligation classes the pipeline
  already discharges (CONTRACT-equivalence, EXEC-value, BODY-state, LOOP-{entry,preservation,exit},
  plus the auxiliary classes Verus discharges inside an item: overflow/bounds via the bounded `S_E`,
  termination via `dec`). Today these are MATERIALIZED only as Verus text (`obligation.rs`); REQ-1
  is the reification of the same content as a prover-neutral value. Derived from §6 + the obligation
  machinery `thermite-tv/src/obligation.rs` already emits. **Increment (i), blocker #204.**
- **REQ-2 (the Engine interface)** — an engine provides four things: (a) a FRAGMENT — the obligation
  classes / construct sets it can ATTEMPT; (b) DISCHARGE — `Obligation → {Proven(evidence),
  Refuted(counterexample), Unknown(reason)}` with the strict mapping discipline below (a
  tactic/solver FAILURE is Unknown, NEVER Refuted); (c) a TRUST PROFILE — the named base added when
  this engine says Proven; (d) EVIDENCE — replayable, cacheable. The first engine behind the
  interface is Verus, refactored byte-identically (no behavior change). Derived from §6 + the
  three-way `classify_verus_outcome` SHIPPED in `check.rs`. **Increment (i), blocker #204.**
- **REQ-3 (the discharge discipline — Unknown degrades, Refuted hard-fails, generic over engines)**
  — the existing rules become engine-generic: an `Unknown` from an engine degrades per the ladder
  (`degrade::run_ladder`); a `Refuted` (a genuine countermodel) HARD-FAILS and NEVER degrades. A
  tactic battery exhausting itself or a solver timing out is `Unknown`, never `Refuted` — refutation
  requires a witnessing input. Derived from `degrade-ladder.md` REQ-2 (the anti-cheat: a
  counterexample never degrades) generalized off Verus. **Increment (i), blocker #204.**
- **REQ-4 (certificate attribution — per-obligation engine + trust profile)** — `Level::L3` keeps
  meaning "proven for all inputs"; the certificate gains, PER discharged obligation, the ENGINE that
  proved it and that engine's TRUST PROFILE, so an auditor sees that L3-via-Lean enumerates a
  smaller base ({Lean kernel + 3 standard axioms} + the exporter correspondence) than L3-via-Verus
  ({Z3, Verus VC-gen} + the TV/lowering theorem). Project aggregation stays honest-min (the SHIPPED
  `AssuranceManifest::aggregate`). Derived from §6 ("the certificate lists every function's level")
  + §1 (the enumerable trusted base). **Increment (iii), FUTURE.**
- **REQ-5 (engine disagreement = a soundness alarm)** — if one engine returns `Proven` and another
  returns `Refuted` on the SAME obligation, the toolchain HALTS with a soundness alarm — it NEVER
  silently picks the favorable verdict. (Proven + Unknown is fine: the Unknown engine simply could
  not decide.) Derived from §1 (trust is the product) + R-DEFER-9 (no proof cheats). **Increment
  (iii), FUTURE.**
- **REQ-6 (the Lean engine — the exporter)** — `forge` serializes a checked item into a Lean theorem
  statement over the EXISTING spine encodings (`Expr`/`Block` inductives + `denote`/`bodyDenote`/
  `loopDenote` in `lean/Thermite/`); the exporter emits Lean SOURCE instantiating those. Its
  faithfulness is the SAME correspondence class as the Rust↔Lean encoder correspondence
  (`rust-lean-correspondence.md`): arm-by-arm inspection + the deep-audit drift tripwire. Named here
  as a NEW trust item under that same discipline. Derived from `thermite-semantics.md` REQ-6 + the
  inspection-tier discipline. **Increment (ii)/(iv), FUTURE.**
- **REQ-7 (the Lean engine — discharge modes + termination)** — (i) AUTO: a tactic battery
  (`omega`/`simp`/`decide`/Lean-SMT's `smt`) over the fragment the z3-demotion PoC PROVES
  reconstructable (scalar/QF-linear-integer contract clauses, kernel-clean); (ii) INTERACTIVE: an
  agent authors a proof file checked in next to the source, replayed in CI; staleness = the
  obligation hash changes → the proof is INVALIDATED, never silently reused. TERMINATION: the Lean
  engine's obligation set must include the `dec` measure or the certificate honestly says
  PARTIAL-CORRECTNESS-only (tied to `while_rule`'s `h_run` premise). Derived from `z3-demotion.md`
  (the reachable fragment) + `thermite-semantics.md` (the partial-correctness `while_rule`).
  **Increment (ii)/(iii), FUTURE.**
- **REQ-8 (engine ordering + the ladder placement)** — DEFAULT order: Verus first (fast,
  push-button), Lean-auto second, Lean-interactive on demand (surface: `forge check --engine lean`
  + a per-item `#[engine(lean)]` annotation — see OQ-1); THEN the existing L2/L1 degrade. The
  SKIP/Unknown accounting per engine is reported. Derived from §6 (downgrades automatic, surfaced)
  + the SHIPPED `degrade::run_ladder`. **Increment (i) wires the ordering hook; (ii) adds the Lean
  rung.**
- **REQ-9 (the anti-Goodhart battery is engine-generic — the honest v1)** — a Lean-proven contract
  still faces the §7 mutation battery; mutants are re-discharged via the AUTO path or Verus where
  exportable, and where NEITHER engine can re-discharge a mutant the kill-ratio reporting says
  "untested against engine X" HONESTLY rather than inflating the ratio. Designed against the actual
  `mutation_score` mechanics (which today re-runs `run_verus` per mutant through the #8 cache).
  Derived from §7 + R-DEFER-9. **Increment (iii), FUTURE.**

## Acceptance criteria

This is the INTERFACE/architecture layer; its ACs are DEFINITION-COMPLETENESS + GROUNDEDNESS +
NON-VACUITY + DECISION-RECORDED, not a `cargo test`. The mechanical discharge of each AC moves to the
per-increment build blockers as they land. (Increment (i), #204, is the first build: its AC is the
byte-identical cert-oracle regression — `conformance/*.cert.json` unchanged after Verus moves behind
the interface.)

- **AC-1 (the Obligation covers exactly the classes the pipeline discharges TODAY)** — every
  obligation the SHIPPED `obligation.rs` family materializes has a backend-neutral `class`: CONTRACT
  (`equivalence_obligation`), EXEC (`exec_equivalence_obligation`), BODY (`body_equivalence_
  obligation`), LOOP-ENTRY/PRESERVATION/EXIT (`loop_{entry,preservation,exit}_obligation`), plus the
  in-item Verus-discharged auxiliaries (overflow/bounds, termination/`dec`). A class Verus discharges
  but the Obligation cannot yet represent is recorded OUT here. Mechanically: the `class` enum's
  variants = the union of the `obligation.rs` emitters + the §6/§7 auxiliary classes.
- **AC-2 (the Engine interface is non-vacuous — Verus instantiates all four slots)** — the FRAGMENT
  (the whole frozen subset via the lowering), DISCHARGE (the `classify_verus_outcome` three-way map),
  TRUST PROFILE ({Z3, Verus VC-gen} + the TV/lowering theorem), and EVIDENCE (the content-addressed
  proof cache key) are each filled for the Verus engine, from SHIPPED code, below.
- **AC-3 (the discharge discipline is stated with its anti-cheat invariant)** — Unknown→degrade,
  Refuted→hard-fail, failure→Unknown-never-Refuted are stated and tied to the SHIPPED
  `degrade::ladder_action_l3` (Counterexample is `LadderAction::HardFail`, not a degrade).
- **AC-4 (certificate attribution is specified, honest-min preserved)** — the per-obligation
  {engine, trust profile} attachment, the auditor-visible base-size difference (Lean < Verus), and
  the UNCHANGED honest-min project aggregate are specified against the SHIPPED `Certificate` +
  `AssuranceManifest`.
- **AC-5 (engine disagreement halts)** — the Proven⊕Refuted alarm is stated as a halt, distinct from
  Proven⊕Unknown (benign), with the §1 rationale.
- **AC-6 (the Lean engine's v1 fragment is pinned IN and OUT, with the exporter trust story)** — the
  exportable fragment = what `S`'s spine covers TODAY (contracts 8/8 classes, exec exprs,
  straight-line bodies, v1 while, spec-fns via the fuel registry); the OUT set is enumerated; the
  exporter's faithfulness is named as a NEW arm-by-arm-inspection + drift-tripwire trust item.
- **AC-7 (the mutation-battery v1 is honest)** — the engine-generic battery + the "untested against
  engine X" honest-reporting rule are specified against `mutation_score`'s real mechanics (the per-
  mutant `run_verus` + #8 cache loop), with NO inflation of the kill ratio.
- **AC-8 (the increment plan + the one filed blocker)** — the four increments are recorded, each its
  own build blocker; increment (i) is FILED (#204) and named; (ii)/(iii)/(iv) are named as future.

---

## Architecture

### 0. What is SHIPPED, and what this doc generalizes (the substrate)

The interface this doc designs sits ON TOP of a fully-shipped single-engine pipeline. The honest
starting point:

- **The obligation content is materialized — but only as Verus text, transiently.**
  `thermite-tv/src/obligation.rs` is the per-run obligation machinery. `pub fn
  equivalence_obligation(source, p_production, frame)` emits a SELF-CONTAINED Verus program whose
  single proof obligation is `assert((P_production) <==> (P_reference))`; its module doc states
  "`thermite-tv` does NOT run verus itself: it emits the obligation TEXT." The frame
  (`pub struct ObligationFrame { spec_defs, params, req, seq_params, nat_coerce_params,
  string_params, map_params }`) carries the env/typing context. The EXEC dual is `pub fn
  exec_equivalence_obligation` (the `tv_exec_wrap` exec-fn form), the BODY dual `pub fn
  body_equivalence_obligation` (`tv_body_wrap`), and the LOOP triple `pub fn loop_entry_obligation`
  / `loop_preservation_obligation` / `loop_exit_obligation` (each emitting a self-contained Verus
  unit). These ARE the obligation classes — the artifact (REQ-1) is their content reified
  prover-neutrally instead of as a Verus string. **SHIPPED.**
- **The discharge pipeline is welded to Verus.** `forge::check::check_file_with_options`
  (`forge/src/check.rs`) runs `parse → validate → check_effects` then per item `item_subprogram →
  thermite_lower::lower → run_verus → assemble_certificate`. The engine is implicit: `run_verus`
  spawns the real `verus` binary; `classify_verus_outcome` is the deterministic three-way split
  `Proved` / `Timeout` / `Counterexample`. There is no engine abstraction — REQ-2 introduces one
  and refactors this path behind it. **SHIPPED (the path), NOT-STARTED (the abstraction).**
- **The degrade ladder is engine-blind today but its discipline is the right one.**
  `forge::degrade::run_ladder` (`forge/src/degrade.rs`) runs `L3Verdict::Proved → certify L3`;
  `Timeout → attempt_l2 → … → L1`; and `ladder_action_l3` maps a `Counterexample` to a hard fail —
  "a `VerusOutcome::Counterexample` (verus DISPROVED the contract — a real bug) is a HARD FAIL and
  NEVER degrades (REQ-2 anti-cheat)" (`check.rs`). REQ-3 generalizes this off the word "verus".
  **SHIPPED.**
- **The certificate + honest-min aggregate are the trust statement.** `forge::manifest::Certificate`
  carries `level: Level` (`enum Level { L0, L1, L2, L3 }`, `#[derive(Ord)]` so `L0 < L1 < L2 < L3`);
  `AssuranceManifest::aggregate(&[Certificate])` computes the per-fn rows + `ProjectAssurance::
  Certified(min)` / `Failed`, "VERUS-ANCHORED … the project-level min-over-functions is anchored to
  the proved fold-min `thermite_verified::aggregate_level`." The certificate today has NO
  per-obligation engine/trust-profile field — REQ-4 adds one (additively, like `boundary`/`slag`/
  `lowered_assurance`/`assurance_scope`, each `#[serde(default)]` so the frozen golden
  `conformance/sum.cert.json` still deserializes). **SHIPPED (cert + min), NOT-STARTED (attribution).**
- **The content-addressed proof cache is the evidence substrate.** `cache::cache_key(&lowered, seed,
  &verus_version, THERMITE_VERSION)` keys a discharged verdict on the lowered source + seed +
  prover versions; `cache::load`/`store` serve/persist it. REQ-2's EVIDENCE slot generalizes this
  (the key must gain an engine discriminator so a Lean proof and a Verus proof of the same item are
  distinct cache entries). **SHIPPED (cache), NOT-STARTED (engine-keying).**
- **The mechanized semantics `S` is the obligations' target.** `lean/Thermite/` mechanizes `S` over
  the frozen `Expr`/`Block` inductives: `denote`/`refDenote` (`Denote.lean`/`RefEncode.lean`, the
  fuel-indexed contract sublanguage `S_C` with the `Env.specs` registry), `Exec.lean`'s `execDenote`
  (`S_E`, bounded value / overflow-as-`none`), `Exec/Stmt.lean`'s `bodyDenote` (`S_B`, straight-line
  state transformer), `Exec/Loop.lean`'s `loopDenote` + `while_rule` (the fuel-indexed v1-while
  iteration, PARTIAL correctness via the `h_run` exits-hypothesis). The (T1) soundness theorems
  (`ref_sound_eq`, `exec_ref_sound`, `body_ref_sound`) and the (T2) capstone `lowering_faithful`
  (`Faithfulness.lean`) are kernel-checked with axioms `{propext, Classical.choice, Quot.sound}`.
  **SHIPPED (epic #169 complete for the frozen subset).**
- **The anti-Goodhart battery is engine-blind.** `forge::check::mutation_score` generates mutants
  (`mutation::generate`), lowers + re-`run_verus`-es each (through the #8 cache), and counts
  killed/scored/equivalent. It is hardcoded to Verus. REQ-9 generalizes it. **SHIPPED (Verus-only).**

### 1. The Obligation — the backend-neutral artifact (REQ-1)

For a checked item, the verification question stated against `S`: for a contract clause,
`∀ inputs, ⟦req⟧_S → ⟦ens[result := body]⟧_S` — i.e. the (T1)-style equality the spine already
proves the reference encoder satisfies, lifted to the per-item obligation. Plus the auxiliary
classes the pipeline discharges INSIDE an item: overflow/bounds (via the bounded `S_E` — `execDenote
= none` exactly at overflow), loop entry/preservation/exit (via `loopDenote` + `while_rule`),
termination (via the source `dec` measure → the well-founded fixpoint of the fuel-indexed denotation).

The artifact reifies the content `obligation.rs` materializes, prover-neutrally:

```
Obligation {
  item:      ItemId,              // the fn / spec-fn the obligation belongs to (§5.3 per-item)
  class:     ObligationClass,     // CONTRACT | EXEC | BODY | LOOP_ENTRY | LOOP_PRESERVATION
                                  //   | LOOP_EXIT | OVERFLOW | TERMINATION   (AC-1: = the
                                  //   obligation.rs emitters ∪ the §6/§7 in-item auxiliaries)
  ast_slice: ExprOrBlock,         // the parsed thermite-syntax node(s) — the SAME `source: &Expr`
                                  //   / `body: &Block` the obligation.rs functions consume
  env:       ObligationEnv {      // the typing/env context — the prover-neutral generalization of
                                  //   thermite-tv's ObligationFrame
    params:        Vec<(Name, ThermiteType)>,   // free vars at their THERMITE types (not Verus
                                                //   strings) — the engine renders them
    req:           Option<ExprId>,              // the enclosing precondition (an AST node, not text)
    spec_defs:     Vec<SpecFnId>,               // the in-scope spec-fn / combinator defs (by id,
                                                //   resolved against the SHARED frozen registry)
    seq/string/map/nat_coerce: …                // the coercion-frame bits ObligationFrame carries,
                                                //   kept ENGINE-NEUTRAL (a Verus engine renders the
                                                //   @-view / as-nat; a Lean engine renders the
                                                //   Seq view / toNat — both from the same flag)
  },
}
```

The discriminator: today these bits exist only as the Verus STRINGS `obligation.rs` interleaves
(`param_list()`, `spec_defs` verbatim, the `as nat` rewrite in `ref_ctx`). The artifact carries the
PRE-rendering content (AST nodes + Thermite types + coercion flags), so an engine renders it into ITS
language — Verus text for the Verus engine (the existing `obligation.rs` rendering becomes the Verus
engine's `render`), Lean source over the `Expr`/`Block` inductives for the Lean engine. This is the
load-bearing inversion: the obligation stops being Verus-shaped. **NOT-STARTED — increment (i),
blocker #204.**

### 2. The Engine interface (REQ-2)

```
trait Engine {
  fn name(&self) -> EngineName;                       // Verus | LeanAuto | LeanInteractive

  // (a) FRAGMENT — which obligation classes / construct sets this engine can ATTEMPT.
  fn fragment(&self) -> Fragment;                     // a predicate on (ObligationClass, ast_slice)

  // (b) DISCHARGE — the verdict. The mapping discipline is REQ-3.
  fn discharge(&self, o: &Obligation) -> Verdict;     // Proven(Evidence) | Refuted(Counterexample)
                                                      //   | Unknown(Reason)

  // (c) TRUST PROFILE — the named base ADDED when this engine says Proven.
  fn trust_profile(&self) -> TrustProfile;            // an ENUMERATED set of named trust items

  // (d) EVIDENCE — replayable, cacheable; the cache key gains an engine discriminator.
  fn evidence_key(&self, o: &Obligation) -> CacheKey; // generalizes cache::cache_key
}
```

The first instance refactors Verus byte-identically (AC-2):

- **FRAGMENT** = the whole frozen subset reachable via the lowering (everything `thermite_lower::
  lower` + `run_verus` handle today: contracts, exec, straight-line bodies, v1 while, spec-fns,
  ADTs, the boundary/slag short-circuits stay engine-independent gates AHEAD of discharge).
- **DISCHARGE** = `classify_verus_outcome`'s three-way map, lifted to `Verdict`: `Proved` → `Proven`;
  `Counterexample` → `Refuted`; `Timeout` → `Unknown(VerusTimeout + the SolverProfile)`. (The FAST-
  `unknown` incompleteness edge that `classify_verus_outcome` today absorbs into the Counterexample
  witness path — OQ-1 of `solver-profiles.md` — must be re-examined: an incompleteness `unknown` is
  an Unknown, NOT a Refuted; see REQ-3 + OQ-3.)
- **TRUST PROFILE** = `{Z3, Verus VC-gen}` + the TV/lowering theorem (`lowering_faithful`, RELATIVE
  to `{Z3 soundness, S = intended meaning, Lean kernel}` per `Faithfulness.lean`). I.e. a Verus L3
  enumerates Z3 + the Verus VC generator + the per-run TV's Z3-trusted `h_tv` premise.
- **EVIDENCE** = the content-addressed proof cache entry; the key (`cache::cache_key`) gains an
  engine field so a Verus proof and a Lean proof of the same lowered item never collide.

The Lean engine instantiates the same four slots (§4). **NOT-STARTED — increment (i) builds the
trait + the Verus instance; blocker #204.**

### 3. The discharge discipline (REQ-3, generalized off the SHIPPED ladder)

```
Verdict::Proven(_)    → certify at this engine's level (L3 for a sound-for-all-inputs engine);
                        attach {engine, trust_profile} (REQ-4).
Verdict::Unknown(_)   → DEGRADE per degrade::run_ladder (try the next engine in REQ-8's order,
                        then L2/L1). An Unknown is NOT a failure verdict — it is "this engine
                        could not decide."
Verdict::Refuted(cx)  → HARD-FAIL. NEVER degrades, NEVER tries another engine to launder it.
                        The counterexample is the deliverable (§5.1 "counterexamples, not
                        adjectives"). This generalizes `ladder_action_l3`'s
                        `Counterexample → LadderAction::HardFail`.
```

The anti-cheat invariant (AC-3): a tactic battery EXHAUSTING itself, a solver TIMING OUT, or an SMT
`unknown` (incompleteness) is `Unknown`, **never** `Refuted`. Refutation requires a genuine
countermodel — a witnessing input on which the contract demonstrably fails. This is the
engine-generic statement of the SHIPPED rule (`degrade-ladder.md` REQ-2): a counterexample never
degrades, a timeout does. **NOT-STARTED — increment (i) wires `Verdict` into `run_ladder`; #204.**

### 4. The Lean engine (engine #2) (REQ-6/REQ-7)

**The EXPORTER (REQ-6).** `forge` serializes an `Obligation` into a Lean theorem statement over the
EXISTING spine encodings — the `Expr`/`Block` inductives + `denote`/`bodyDenote`/`loopDenote` in
`lean/Thermite/`. The exporter emits Lean SOURCE that INSTANTIATES those definitions (e.g. a contract
obligation becomes `theorem item_xyz : ∀ (env : Env), reqDenote … → denote fuel ens (envWith result
body) = true := by …`). Crucially the exporter does NOT define a new semantics — it targets the
already-kernel-proven `S`, so its faithfulness is the SAME correspondence class as the Rust↔Lean
encoder correspondence: **arm-by-arm inspection** (each Thermite AST construct ↦ its `Expr`
constructor, quoting both sides) **+ the deep-audit drift tripwire** (`scripts/audit.sh` check [4],
the SHA-pinning discipline `rust-lean-correspondence.md` uses — any change to the exporter or the
targeted spine arms invalidates the audit row and forces re-inspection). This is named here as a
NEW trust item of that exact discipline: **(EXP) — the exporter emits Lean source that, arm-by-arm,
instantiates the kernel-proven `S` definitions for the construct it exports.** It is NOT a stronger
extraction bridge; it is the inspection tier, honestly. **NOT-STARTED — increment (ii)/(iv).**

**DISCHARGE MODES (REQ-7).**
- **(i) AUTO** — a tactic battery `omega`/`simp`/`decide`/Lean-SMT's `smt` (where applicable). The
  z3-demotion PoC (`z3-demotion.md`) pins what is REACHABLE TODAY, kernel-clean: scalar comparisons
  + logical connectives + LINEAR integer arithmetic (QF_LIA) over the contract sublanguage —
  `tv_obligation_arith_cmp` / `tv_obligation_or_le` discharge with axioms `{propext,
  Classical.choice, Quot.sound}` ONLY (no `sorryAx`, no cvc5 oracle axiom). OUT of auto today: the
  QF_BV bitwise fragment (blocked by an upstream `sorry` in Lean-SMT's `Bitblast.lean`), the bounded
  quantifier combinators (~30% cvc5-rule reconstruction coverage — may FAIL, i.e. `Unknown`, never
  unsound), and recursive spec-fns / `permutation_of` (need the recursive def axiomatized). So the
  Lean-auto FRAGMENT (REQ-2(a)) is precisely the scalar/linear contract clause — the "cheapest real
  win" (increment (ii)).
- **(ii) INTERACTIVE** — an agent authors a proof file checked in NEXT TO the source, replayed in CI.
  Proof-artifact management: the proof lives at a deterministic path keyed on the item +
  obligation-hash; STALENESS is defined as the obligation hash CHANGING (the `ast_slice` + `env`
  hash, the same content the cache keys) → the proof is INVALIDATED and must be re-authored, NEVER
  silently reused against a changed obligation. This is the design's answer to the deferred
  Lean-style incremental holes (issue #21) at the WHOLE-ITEM tier — a proof artifact, not an
  in-process goal state. **NOT-STARTED — increment (iii).**

**TERMINATION (REQ-7).** The Lean engine's obligation set for a looping/recursive item MUST include
the `dec` measure (the termination obligation class) — OR the certificate honestly records
PARTIAL-CORRECTNESS-ONLY for that item. This ties directly to `while_rule`'s `h_run` premise: the
SHIPPED `while_rule` is partial correctness ("after-loop holds IF the loop EXITS"); termination is
the per-run residual (the source `dec`). A Lean engine that proves preservation+exit but NOT
termination certifies partial correctness, and the certificate must SAY so (it cannot silently claim
L3-total). **NOT-STARTED — increment (iii)/(iv).**

**TRUST PROFILE.** A Lean L3 enumerates `{Lean kernel + the 3 standard axioms (propext,
Classical.choice, Quot.sound)}` + the exporter correspondence (EXP). For the AUTO path via Lean-SMT,
cvc5 is NOT in the base (its proof is RE-CHECKED in the kernel — `z3-demotion.md`'s honesty crux), so
the base is the kernel + standard axioms + EXP. This is STRICTLY SMALLER than the Verus base (no Z3,
no Verus VC-gen) — the auditor-visible difference REQ-4 exposes.

### 5. Certificate attribution + the disagreement rule (REQ-4/REQ-5)

**Attribution (REQ-4).** `Level::L3` is unchanged — it still means "proven for all inputs." The
certificate gains a PER-OBLIGATION attribution: for each discharged obligation, the `{engine,
trust_profile}` pair. Schema-wise this is an ADDITIVE field on `Certificate` (an
`Vec<ObligationAttribution>` or a per-level engine tag), `#[serde(default,
skip_serializing_if=…)]` — exactly the precedent set by `boundary` / `slag` / `lowered_assurance` /
`assurance_scope` (each added additively so the frozen golden `conformance/sum.cert.json` still
deserializes, R-SPEC-2). An auditor reading two L3 certs can then SEE that one enumerates `{Lean
kernel, 3 axioms, EXP}` and the other `{Z3, Verus VC-gen, lowering theorem}` — the smaller base is
the stronger result, made visible. Whether the attribution JOINS the cert oracle (`oracle_subset`)
or is diagnostic-only is OQ-2 (the conservative default: oracle-visible, since the trust base IS
verdict-relevant — but that perturbs the golden, so it must be designed with the corpus re-pinned).

**Project aggregation stays honest-min.** `AssuranceManifest::aggregate` is UNCHANGED — the project
headline is still `Certified(min over functions)` / `Failed`. Attribution is per-obligation
metadata, ORTHOGONAL to the level the min folds over (§9's compose-trust discipline; the same
orthogonality `assurance_scope` already has to `level`).

**Disagreement (REQ-5, AC-5).** If, for the SAME obligation, one engine returns `Proven` and another
returns `Refuted` — that is a SOUNDNESS ALARM. The toolchain HALTS (a distinguished
`ForgeError`/non-cert abort), surfaces both verdicts + the refuting counterexample, and NEVER picks
the favorable Proven. A genuine countermodel from one engine contradicting a "proof" from another
means one engine (or the exporter/lowering, or `S` itself) is unsound, and silently proceeding would
launder unsoundness into a certificate — the exact failure §1's enumerable-trusted-base promise
forbids. Proven⊕Unknown is BENIGN (the Unknown engine simply could not decide — no contradiction);
only Proven⊕Refuted alarms. **NOT-STARTED — increment (iii).**

### 6. Engine ordering + the ladder (REQ-8)

DEFAULT order (justified): **Verus first** — it is fast, push-button, and covers the whole frozen
subset, so the common case pays no Lean cost. **Lean-auto second** — on a Verus `Unknown` (timeout),
or when explicitly requested, the Lean-auto battery attempts the scalar/linear fragment it can
kernel-check (a SMALLER trust base on success). **Lean-interactive on demand** — never automatic (it
needs a human/agent-authored proof artifact); reached by `forge check --engine lean` or a per-item
`#[engine(lean)]` annotation (the surface is OQ-1 — both are sketched; the per-item annotation is
preferred for "this one function wants the smaller base" without changing the whole-file default).
THEN the existing L2 (Kani) / L1 (runtime) degrade rungs, unchanged.

This slots into the SHIPPED `degrade::run_ladder` as additional rungs BEFORE L2: the ladder already
takes closures for L2/L1 attempts; the engine rungs are the same shape (an `attempt_engine` closure
per engine in order). The SKIP/Unknown accounting per engine is reported in the cert (which engines
attempted, which returned Unknown and why) — generalizing the SHIPPED `SolverProfile` + the
"untested against engine X" honesty of REQ-9. **NOT-STARTED — increment (i) adds the ordering hook
(Verus-only, so byte-identical); (ii) adds the Lean-auto rung.**

### 7. The anti-Goodhart battery, engine-generic (REQ-9, the honest v1)

The §7 mutation battery is ENGINE-GENERIC: a Lean-proven contract still faces mutants. The honest v1,
designed against `mutation_score`'s ACTUAL mechanics (it calls `mutation::generate(f, seed)`, then
per mutant lowers + content-addresses + `run_verus`es through the #8 cache, counting
`killed`/`scored`/`equivalent`, with a survivor → strengthening prompt):

- A mutant is "killed" if SOME engine in the fragment REFUTES it (the mutated body violates the
  contract — exactly today's `mutant_outcome_is_survivor` inverted, but now over `Verdict::Refuted`
  rather than a Verus-only outcome).
- A mutant re-discharged via the AUTO path (Lean-auto or Verus) where the mutant's obligation is in
  that engine's fragment counts normally.
- Where NEITHER engine can re-discharge a mutant's obligation (e.g. a mutant outside Lean-auto's
  scalar fragment AND Verus times out), the kill-ratio reporting says **"untested against engine X"**
  HONESTLY — it does NOT count the un-attemptable mutant as killed (which would inflate the ratio,
  violating §7 + R-DEFER-9), and it does NOT count it as a survivor against an engine that never
  tried it. The denominator records "scored against engine X"; the un-attempted mutants are reported
  separately. (This mirrors the SHIPPED `mutation_score` OQ-5: a mutant that fails to LOWER is
  DROPPED from the denominator, never an `Err` — the same "don't count what you couldn't fairly
  test" discipline, lifted to engines.)

The mutation FLOOR (`MUTATION_FLOOR`, default 0.60) gates certification per the SHIPPED `meets_floor`
— unchanged; only the kill DEFINITION becomes engine-aware. **NOT-STARTED — increment (iii).**

### 8. v1 scope — what is IN and OUT (AC-6)

The exportable/dischargeable fragment for the Lean engine = what the spine's `S` covers TODAY (epic
#169 complete for the frozen subset):

**IN.** Contracts (all 8 frozen combinator classes + the `S_C` `Expr` subset), exec expressions
(`S_E`, bounded value / overflow-as-`none`), straight-line bodies (`S_B`), the v1 `while` form
(`loopDenote` + partial-correctness `while_rule`), spec-fns via the fuel-indexed registry. For
AUTO discharge specifically, the IN set NARROWS to the z3-demotion-reachable scalar/QF-linear core;
the richer IN constructs need INTERACTIVE proofs (or stay on Verus).

**OUT.** User ADTs in match-position (the Lean `Variant` has only the 4 built-in Option/Result
variants — `rust-lean-correspondence.md` D6/the user-variant residual); post-v1 loops (`loop`/
`break`/`continue`/multi-exit early return/nested loops/non-scalar mutation `xs[i]=e` — all honest
`Unsupported` in the encoders + absent from the Lean inductives); open body holes (`?N` —
short-circuited L0 before any engine); slag bodies (fiat-trusted, no engine); boundary items
(foreign body, L1, no engine). These remain OUT exactly as they are OUT of `S` today.

### 9. The increment plan + the build blockers (AC-8)

- **(i) The Obligation reification + the Engine trait in forge** — Verus refactored behind the
  interface, behavior BYTE-IDENTICAL (the conformance cert oracle `conformance/*.cert.json`
  unperturbed — the AC for this increment). No new engine. **FILED: blocker #204.**
- **(ii) The Lean exporter + auto-discharge for the PURE-CONTRACT class** — the cheapest real win
  (the z3-demotion scalar/linear fragment, kernel-clean), behind the Lean-auto rung. **FUTURE.**
- **(iii) Interactive proofs + certificate attribution + the engine-generic battery + the
  disagreement alarm** — the per-obligation `{engine, trust_profile}` attribution, the
  Proven⊕Refuted halt, the honest mutation v1. **FUTURE.**
- **(iv) The full exportable fragment** — exec exprs, straight-line bodies, v1 while, spec-fns via
  the fuel registry, under the Lean engine. **FUTURE.**

## Verification

Per increment (this doc's own ACs are statement-completeness, discharged by review):
- **(i), #204:** `cargo test -p forge` green AND the conformance cert oracle UNCHANGED — every
  `conformance/<name>.cert.json` byte-stable after Verus moves behind the `Engine` trait (the
  decisive regression: the refactor is behavior-preserving). Plus `cargo clippy`/`fmt`.
- **(ii):** the Lean-auto rung discharges the scalar-contract corpus obligations with `#print axioms`
  = `{propext, Classical.choice, Quot.sound}` only (the z3-demotion kernel-clean bar), and a
  Lean-proven cert carries the smaller trust profile.
- **(iii):** an injected Proven⊕Refuted disagreement HALTS (a test asserting the alarm fires, not a
  favorable pick); a mutant outside both engines' fragments is reported "untested," never counted as
  killed; the attribution field round-trips the frozen golden.
- **(iv):** the Lean engine's fragment-coverage tests over the full frozen subset, with the OUT set
  honestly Skipped.

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (the Obligation artifact) | NOT-STARTED | open build blocker #204. The content is SHIPPED only as transient Verus text: `pub fn equivalence_obligation` / `exec_equivalence_obligation` / `body_equivalence_obligation` / `loop_{entry,preservation,exit}_obligation` in `thermite-tv/src/obligation.rs` ("`thermite-tv` does NOT run verus itself: it emits the obligation TEXT") + `pub struct ObligationFrame` (the env/typing ctx). The prover-NEUTRAL artifact (AST slice + Thermite types + coercion flags, pre-rendering) is unbuilt — that is the gap. |
| REQ-2 (the Engine interface) | NOT-STARTED | open build blocker #204. The discharge path is SHIPPED but Verus-welded: `forge::check::check_file_with_options` calls `run_verus` directly + `classify_verus_outcome` (the three-way `Proved`/`Timeout`/`Counterexample` split) in `forge/src/check.rs`. No `trait Engine`, no fragment/trust-profile/evidence abstraction. The Verus instance maps directly onto the SHIPPED outcome classifier (AC-2). |
| REQ-3 (Unknown degrades / Refuted hard-fails, engine-generic) | NOT-STARTED | open blocker #204. The discipline is SHIPPED for Verus: `degrade::ladder_action_l3` maps `Counterexample → LadderAction::HardFail` ("a `VerusOutcome::Counterexample` … is a HARD FAIL and NEVER degrades (REQ-2 anti-cheat)", `forge/src/check.rs`); `Timeout` degrades via `degrade::run_ladder` (`forge/src/degrade.rs`). The generalization off the word "verus" + the failure-is-Unknown-never-Refuted rule for tactic batteries is unbuilt. |
| REQ-4 (certificate attribution — per-obligation engine + trust profile) | NOT-STARTED | FUTURE (increment (iii)). The cert + honest-min are SHIPPED: `manifest::Certificate { level: Level, .. }`, `enum Level { L0, L1, L2, L3 }` (`#[derive(Ord)]`), `AssuranceManifest::aggregate → ProjectAssurance::Certified(min)` (VERUS-ANCHORED to `thermite_verified::aggregate_level`). NO per-obligation `{engine, trust_profile}` field exists; the additive-field precedent (`boundary`/`slag`/`lowered_assurance`/`assurance_scope`, all `#[serde(default)]`) is the schema model. |
| REQ-5 (engine disagreement = soundness alarm) | NOT-STARTED | FUTURE (increment (iii)). No second engine exists yet, so no disagreement path. The anti-cheat ANCESTOR is SHIPPED: a counterexample never degrades (`ladder_action_l3` → `HardFail`). The Proven⊕Refuted halt (vs benign Proven⊕Unknown) is unbuilt. |
| REQ-6 (the Lean exporter) | NOT-STARTED | FUTURE (increment (ii)/(iv)). The TARGET is SHIPPED: `lean/Thermite/` mechanizes `S` (`denote`/`refDenote`/`Denote.lean`, `execDenote`/`Exec.lean`, `bodyDenote`/`Exec/Stmt.lean`, `loopDenote`+`while_rule`/`Exec/Loop.lean`) over the `Expr`/`Block` inductives, kernel-checked (axioms `{propext, Classical.choice, Quot.sound}`). The Rust→Lean exporter that emits source instantiating those is unbuilt; the z3-demotion doc names it as "the #185-adjacent correspondence-bridge work … NOT built." Its faithfulness = the inspection tier of `rust-lean-correspondence.md`. |
| REQ-7 (Lean discharge modes + termination) | NOT-STARTED | FUTURE (increment (ii)/(iii)). The AUTO fragment is PROVEN-REACHABLE: `z3-demotion.md` shows `tv_obligation_arith_cmp`/`tv_obligation_or_le` (scalar/QF-linear contract clauses) discharged by Lean-SMT's `smt` tactic, kernel-clean (`#print axioms` = standard set only; no `sorryAx`/cvc5 oracle). The interactive/proof-artifact mode + the `dec`/partial-correctness termination policy (tied to the SHIPPED `while_rule` `h_run` premise) are unbuilt. |
| REQ-8 (engine ordering + ladder placement) | NOT-STARTED | open blocker #204 (the Verus-only hook) + FUTURE (the Lean rung). The ladder substrate is SHIPPED: `degrade::run_ladder` takes per-rung closures (`attempt_l2`/`attempt_l1`); the engine rungs are the same closure shape. The `--engine lean` / `#[engine(lean)]` surface (OQ-1) and the per-engine SKIP/Unknown accounting are unbuilt. |
| REQ-9 (engine-generic anti-Goodhart battery, honest v1) | NOT-STARTED | FUTURE (increment (iii)). The battery is SHIPPED Verus-only: `forge::check::mutation_score` generates mutants + re-`run_verus`es each through the #8 cache, counting killed/scored/equivalent; OQ-5 already DROPS un-lowerable mutants from the denominator (the "don't count what you couldn't fairly test" ancestor). The engine-aware kill definition + the "untested against engine X" honest reporting are unbuilt. |

## Open questions (for co-authorship)

These are deliberately left OPEN for a second designer (the orchestrator intends to offer them):

- **OQ-1 (the engine-annotation surface syntax).** `forge check --engine lean` (whole-file default
  override) vs a per-item `#[engine(lean)]` attribute (this-function-wants-the-smaller-base) — or
  both, with a precedence rule. The per-item form is sketched as preferred but not decided; it
  interacts with §4.4's "what Thermite removes from Rust" (one-way-to-do-everything, §2.3 — a new
  attribute is surface area that must justify itself).
- **OQ-2 (does the trust-profile attribution join the cert oracle?).** REQ-4's `{engine,
  trust_profile}` is verdict-relevant (the trust base IS the deliverable, §1), arguing for
  oracle-visible. But that perturbs the frozen golden `conformance/sum.cert.json` (which would gain a
  Verus-engine attribution), forcing a corpus re-pin. The alternative — diagnostic-only, like
  `degrade_reason` — keeps the golden stable but hides the base difference from the cert oracle. The
  `assurance_scope` precedent (verdict-relevant → normalized into the oracle as a bool) suggests a
  middle path; undecided.
- **OQ-3 (should trust profiles be lattice-ordered?).** Is `{Lean kernel, 3 axioms, EXP}` formally
  ≤ `{Z3, Verus VC-gen, lowering theorem}` in a trust lattice, so the certificate can present a
  PARTIAL ORDER over engines (and the project aggregate could fold a join/meet over trust bases, not
  just over `Level`)? Or is "smaller enumerated set" only informally comparable (the bases are not
  subsets — Lean's EXP is not a subset of Verus's)? This decides whether REQ-4's "smaller base"
  claim is a formal order or an auditor's informal read.
- **OQ-4 (the interactive-proof review policy).** Where do checked-in Lean proof artifacts live, who
  reviews them, and what is the CI staleness/replay policy beyond "obligation-hash change
  invalidates"? Does an interactive proof require second-party sign-off like a `#[slag]` block (§8 CI
  policy hooks)? Is an interactive Lean proof's trust profile DIFFERENT from an auto one (it adds the
  human/agent author as a reviewed-but-not-mechanized step)?
