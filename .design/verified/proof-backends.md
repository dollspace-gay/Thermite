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
    function's level … this manifest IS the deliverable's trust statement"; downgrades automatic;
    degrade-on-timeout)
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
    Verus refactored behind the interface, behavior byte-identical EXCEPT the named fast-unknown
    remap of §2/REQ-3.1; the conformance cert oracle unperturbed; no new engine). AMENDED (#226):
    the reified Obligation's REGISTRY-TERMINATION class + `calledSpecFns(item)` MUST use the
    CORRECTED full-expression-position closure (seed `req ∪ ens ∪ body ∪ dec(item)`, step over each
    reached spec-fn's `body ∪ dec`). The SHIPPED forge closure `reachable_spec_fn_deps` /
    `collect_block_spec_fn_calls` (`forge/src/check.rs`) currently walks `decl.body` ONLY (never
    `decl.dec`) and seeds at the start spec-fn — the SAME body-only omission the #226 finding names;
    increment (i)'s reification must correct it to the full-expression-position closure when it lifts
    `calledSpecFns` into the Obligation. TODAY'S EXPOSURE is LOAD-BEARING only for the NEW Lean
    exporter: on the SHIPPED Verus pipeline a measure-called spec-fn omitted from the lowered
    sub-program leaves the lowered Verus text REFERENCING an undefined function, so Verus type-checks
    the measure against real definitions and the pipeline fails CLOSED (a Verus error, not a silent
    certification) — matching the critic's pin, which exhibits the unsoundness on the Lean
    `R_item`/stabilization path (the bottom-to-`0` denotation), NOT the Verus path. The correction
    is recorded HERE as a named increment-(i) work item (NOT a separate issue).
  - increment (ii): FUTURE (the Lean exporter + auto-discharge for the PURE-CONTRACT class).
    SPINE PREREQUISITE (a small NAMED Lean addition, part of THIS increment) — SHIPPED in
    `lean/Thermite/Stabilize.lean` (#240, ref #203; imported by `lean/Thermite.lean`),
    kernel-checked with the standard axiom set `{propext, Classical.choice, Quot.sound}` (NO
    `sorryAx`): the `stabilizes` relation (`stabilizes (e : Expr) (env : Env) (v : Int) : Prop` for
    `intVal`, and `stabilizesProp` for `denote`, matching `Denote.lean`'s signatures/universes) +
    `stabilizes_unique` (the #214 uniqueness lever, overlap-at-max) + the supporting lemma
    `stabilization_exists` (the design's `stabilization_exists_for_dec_bounded`) — SHIPPED in the
    HYPOTHESIS form keyed on `RegistryTerminating env e := ∃ v, stabilizes e env v` (the
    fully-general core-Lean form is NOT provable: the registry is an arbitrary `String → Option
    SpecFn` with no spine-available well-foundedness, and a divergent registry sits at the fuel-0
    Int-bottom for all fuel; the hypothesis is EXACTLY what the per-item REGISTRY-TERMINATION
    obligation class REQ-1.2 discharges, so it is the named separately-discharged obligation, NOT an
    assumed-away premise). The §4 obligation form is stated against `stabilizes`, NOT a raw fuel
    index, and the RESULT value is bound THROUGH `stabilizes` (the #214 fix). ALSO SHIPPED in the
    same module: the FUEL-IRRELEVANCE lemma (`specCallFree e → intVal f e env = intVal g e env` for
    all fuels — `intVal_fuel_irrelevant`/`denote_fuel_irrelevant`, the Prop analogue, by the mutual
    well-founded recursion over `intVal`/`seqVal`/`denote`/`denoteArms` with `env` generalized;
    `specCallFree` is a Bool predicate over the FULL mutual AST Expr/Pred/MatchArm/RangeArg) + the
    FUEL-FREE tier-(a) export keys `stabilizesProp_iff_denote_zero` / `stabilizes_iff_intVal_zero`
    (for spec-call-free `e`, the `∃N∀` relation collapses to the fuel-0 value — the #216
    normalization bridge that lets the exporter emit FUEL-FREE shallow statements for the
    specCall-free auto fragment, §4/§6). (Tracked in THIS #204-chain as an AMENDMENT to increment
    (ii) — no new issue; see §4 "the stabilized form" + "the normalization story" + the build
    blocker note there. The four critic pins keep their own local `stabilizes`/`stabilizesProp`
    copies and still build green against the new defs.)
  - increment (iii): FUTURE (interactive proofs + per-obligation certificate attribution + the
    engine-generic anti-Goodhart battery)
  - increment (iv): FUTURE (the full exportable fragment — exec exprs, straight-line bodies,
    v1 while, spec-fns via the fuel registry). OWNS the EXEC-BODY BRIDGE (§4.1, the first
    S_C×S_E/S_B domain-tying artifact) as its OWN design obligation (the #212 fix): the BVal.value
    value bridge, the bool-result binding (a `bindBool` spine addition — an increment-(iv)
    prerequisite, since `Env` has no bool sort), the optres binding, and the env→State
    correspondence. §4 here designs only increment (ii)'s PURE-CONTRACT class; §4.1 NAMES the
    bridge as open increment-(iv) work.
-->

## Summary

Thermite should be defined by its SEMANTICS (`S`, mechanized in `lean/Thermite/`) with provers as
PLUGINS — not defined by Verus's verifiable fragment. Today the toolchain has exactly one engine
welded into `forge::check`: Verus/Z3, reached implicitly by `run_verus`, with the obligation
existing only transiently as the per-clause/per-body/per-loop Verus text `thermite-tv`'s
`equivalence_obligation` family emits. This doc designs (a) the **Obligation** — a serializable,
backend-neutral verification artifact stated against `S`; (b) the **Engine** interface (fragment /
discharge / trust profile / evidence); (c) **certificate attribution** so an auditor sees that L3
via Lean has a smaller trusted base than L3 via Verus; and (d) **Lean as engine #2** (an exporter
into the existing `lean/Thermite/` spine + an auto tactic battery + interactive proofs). Most of
this is NOT-STARTED behind build blockers; the substrates it generalizes (the obligation
materialization, the discharge pipeline, the degrade ladder, the content-addressed proof cache, the
project-min aggregate, the mechanized `S`) are SHIPPED and quoted below.

## Requirements

- **REQ-1 (the Obligation — the backend-neutral artifact)** — define a serializable verification
  artifact `Obligation { item, class, role, ast_slice, env }` stated against the MECHANIZED
  semantics (`S`), INDEPENDENT of any prover's input language. `class` ∈ the obligation classes the
  pipeline already discharges (CONTRACT-equivalence, EXEC-value, BODY-state,
  LOOP-{entry,preservation,exit}, plus the auxiliary classes Verus discharges inside an item:
  overflow/bounds via the bounded `S_E`, termination via `dec`), PLUS the **REGISTRY-TERMINATION**
  class (REQ-1.2, the #215 fix): for an item with `calledSpecFns(item) ≠ ∅` (the #226 condition —
  the SAME reachability set the §4 hard gate uses: the FULL-EXPRESSION-POSITION closure of every
  spec-fn reachable from `req ∪ ens ∪ body ∪ dec(item)`, where the closure step walks each reached
  spec-fn's `body ∪ dec` — i.e. EVERY expression the export denotes against `R_item`, INCLUDING the
  termination measures, contributes its spec-calls, transitively), EVERY spec-fn in
  `R_item` carries a per-spec-fn obligation that its `dec` measure is VALID (well-founded descent),
  conjoined item-wide by the conjunction rule. `role` is the polarity/intent discriminator
  (CERTIFICATION vs the meta/battery queries of §0.1) that REQ-3 keys its discipline on. Today these
  bits exist only MATERIALIZED as Verus text (`obligation.rs`); REQ-1 is the reification of the same
  content as a prover-neutral value. Derived from §6 + the obligation machinery
  `thermite-tv/src/obligation.rs` already emits. **Increment (i), blocker #204.**
- **REQ-2 (the Engine interface)** — an engine provides four things: (a) a FRAGMENT — the obligation
  classes / construct sets it can ATTEMPT; (b) DISCHARGE — `Obligation → {Proven(evidence),
  Refuted(counterexample), Unknown(reason)}` with the strict mapping discipline below (a
  tactic/solver FAILURE without a witnessing input is Unknown, NEVER Refuted); (c) a TRUST PROFILE —
  the named base added when this engine says Proven; (d) EVIDENCE — replayable, cacheable. The first
  engine behind the interface is Verus, refactored byte-identically EXCEPT the one named, justified
  fast-unknown remap of REQ-3.1. Derived from §6 + the three-way `classify_verus_outcome` SHIPPED in
  `check.rs`. **Increment (i), blocker #204.**
- **REQ-3 (the discharge discipline — Unknown degrades, Refuted hard-fails, generic over engines)**
  — the existing rules become engine-generic FOR CERTIFICATION obligations (`role =
  CERTIFICATION`): an `Unknown` from an engine degrades per the ladder (`degrade::run_ladder`); a
  `Refuted` (a genuine WITNESSED countermodel) HARD-FAILS and NEVER degrades. A tactic battery
  exhausting itself, a solver timing out, OR an SMT incompleteness-`unknown` is `Unknown`, never
  `Refuted` — refutation requires a witnessing input. The polarity-inverted meta/battery queries of
  §0.1 are OUTSIDE this discipline (they have their own role). Derived from `degrade-ladder.md`
  REQ-2 (the anti-cheat: a counterexample never degrades) generalized off Verus. **Subsection
  REQ-3.1 (the fast-unknown seam)** decides the one behavioral delta the Verus engine introduces.
  **Increment (i), blocker #204.**
- **REQ-4 (certificate attribution — per-obligation engine + trust profile)** — `Level::L3` keeps
  meaning "proven for all inputs"; the certificate gains, PER discharged obligation, the ENGINE that
  proved it and that engine's TRUST PROFILE, so an auditor sees that L3-via-Lean enumerates a
  smaller base ({Lean kernel + 3 standard axioms} + the exporter correspondence) than L3-via-Verus
  ({Z3, Verus VC-gen} + the TV/lowering theorem). Project aggregation stays honest-min (the SHIPPED
  `AssuranceManifest::aggregate`). Derived from §6 ("the certificate lists every function's level")
  + §1 (the enumerable trusted base). **Increment (iii), FUTURE.**
- **REQ-5 (engine disagreement = a soundness alarm)** — if one engine returns `Proven` and another
  returns `Refuted` (a WITNESSED countermodel) on the SAME certification obligation, the toolchain
  HALTS with a soundness alarm — it NEVER silently picks the favorable verdict. (Proven + Unknown is
  fine: the Unknown engine simply could not decide — and per REQ-3.1 a witness-less Verus failure is
  Unknown, so it cannot spuriously trigger this alarm.) Derived from §1 (trust is the product) +
  R-DEFER-9 (no proof cheats). **Increment (iii), FUTURE.**
- **REQ-6 (the Lean engine — the exporter)** — `forge` serializes a checked item into a Lean theorem
  statement over the EXISTING spine encodings (`Expr`/`Block` inductives + `denote`/`bodyDenote`/
  `loopDenote` in `lean/Thermite/`); the exporter emits Lean SOURCE instantiating those, with the
  FUEL form pinned by §4 (the obligation must be sound against `Denote.lean`'s fuel-0-bottom = True
  semantics — see §4 "the stabilized form", #213-corrected, with the RESULT value bound THROUGH
  stabilization, #214-corrected). Its faithfulness is the SAME correspondence class as the
  Rust↔Lean encoder correspondence (`rust-lean-correspondence.md`): arm-by-arm inspection + the
  deep-audit drift tripwire — AND it must include registry-population faithfulness (the exported
  registry contains exactly the item's spec-fns with their real bodies; §4 EXP). Named here as a NEW
  trust item under that same discipline. Derived from `thermite-semantics.md` REQ-6 + the
  inspection-tier discipline. **Increment (ii)/(iv), FUTURE.**
- **REQ-7 (the Lean engine — discharge modes + termination)** — (i) AUTO: a tactic battery
  (`omega`/`simp`/`decide`/Lean-SMT's `smt`) over the fragment the z3-demotion PoC PROVES
  reconstructable (scalar/QF-linear-integer contract clauses, kernel-clean), where the exporter emits
  FUEL-FREE shallow statements for the specCall-free goals via the FUEL-IRRELEVANCE lemma (or, for a
  non-recursive registry, via static UNFOLDING to finite depth) — the three-tier normalization story
  of §4/§6, the #216 fix; (ii) INTERACTIVE: an agent authors a proof file checked in next to the
  source, replayed in CI; staleness = the EVIDENCE KEY changes (§2(d): obligation hash + engine +
  engine-toolchain version + the targeted spine content hash) → the proof is INVALIDATED, never
  silently reused. The ∃N∀fuel stabilization forms remain ONLY for this INTERACTIVE path (recursive
  registries). TERMINATION: the Lean engine's obligation set must include the item's `dec` measure or
  the certificate honestly says PARTIAL-CORRECTNESS-only (tied to `while_rule`'s `h_run` premise),
  AND — per REQ-1.2 / the #215 fix — the REGISTRY-TERMINATION class for every spec-fn in `R_item`.
  Derived from `z3-demotion.md` (the reachable fragment) + `thermite-semantics.md` (the
  partial-correctness `while_rule`). **Increment (ii)/(iii), FUTURE.**
- **REQ-8 (engine ordering + the ladder placement)** — DEFAULT order: Verus first (fast,
  push-button), Lean-auto second, Lean-interactive on demand (surface: `forge check --engine lean`
  + a per-item `#[engine(lean)]` annotation — see OQ-1); THEN the existing L2/L1 degrade. The
  SKIP/Unknown accounting per engine is reported. Derived from §6 (downgrades automatic, surfaced)
  + the SHIPPED `degrade::run_ladder`. **Increment (i) wires the ordering hook; (ii) adds the Lean
  rung.**
- **REQ-9 (the anti-Goodhart battery is engine-generic — the honest v1)** — a Lean-proven contract
  still faces the §7 mutation battery; mutants are re-discharged via the AUTO path or Verus where
  exportable, and where NEITHER engine can ATTEMPT a mutant the kill-ratio reporting says "untested
  against engine X" HONESTLY rather than inflating the ratio. The ENGINE-GENERIC kill semantics =
  `Refuted ∪ Unknown-after-attempt` (the mutant was attempted and NOT proven — matching today's
  `Counterexample ∪ Timeout` exactly); "untested" = no engine's fragment ADMITS the mutant (never
  attempted). The §7 floor rules INCORPORATE the SHIPPED #101 equivalence exclusion: survivor =
  (`Proved`-after-attempt) MINUS proven-equivalent; denominator = attempted MINUS proven-equivalent
  (the SHIPPED `scored`); the proven-equivalent are dropped from BOTH (the `equivalence_proves_equal`
  step) so equivalent mutants never re-enter as spurious survivors. The equivalence probe is one of
  the §0.1 meta-queries — consistent with F3, it stays OUTSIDE the Engine interface in v1 (a direct
  verus query). The floor is per the SHIPPED `meets_floor` with an ADDED minimum-attempted guard.
  Derived from §7 + R-DEFER-9. **Increment (iii), FUTURE.**

## Acceptance criteria

This is the INTERFACE/architecture layer; its ACs are DEFINITION-COMPLETENESS + GROUNDEDNESS +
NON-VACUITY + DECISION-RECORDED, not a `cargo test`. The mechanical discharge of each AC moves to the
per-increment build blockers as they land. (Increment (i), #204, is the first build: its AC is the
cert-oracle regression — `conformance/*.cert.json` byte-identical after Verus moves behind the
interface, with NO exception: the REQ-3.1 fast-`unknown` remap is shipped as a NARROW signature that
matches no grounded verus output today and is therefore INERT (the behavioral delta is undelivered
until Z3's `:reason-unknown` is surfaced — `solver-profiles.md` OQ-1); see REQ-3.1 / the Verification
section.)

- **AC-1 (the Obligation covers exactly the classes the pipeline discharges TODAY)** — every
  obligation the SHIPPED `obligation.rs` family materializes has a backend-neutral `class`: CONTRACT
  (`equivalence_obligation`), EXEC (`exec_equivalence_obligation`), BODY (`body_equivalence_
  obligation`), LOOP-ENTRY/PRESERVATION/EXIT (`loop_{entry,preservation,exit}_obligation`), plus the
  in-item Verus-discharged auxiliaries (overflow/bounds, termination/`dec`), PLUS the
  REGISTRY-TERMINATION class (REQ-1.2) for an item with `calledSpecFns(item) ≠ ∅` (§4's reachability
  set: `req ∪ ens ∪ body ∪ dec(item)`, transitively, closure-step over `body ∪ dec` — the
  full-expression-position #226 condition completing #224). The THREE additional
  SHIPPED verus-query classes that are NOT item-correctness certifications — the solver-vacuity
  harnesses (`solver_vacuity_check`, INVERTED polarity), the #101 survivor-equivalence query
  (`equivalence_proves_equal`), and the strengthen probe (`strengthen::probe`) — are enumerated in
  §0.1 and scoped explicitly OUT of the Engine interface in v1 (direct verus invocations, named as a
  deliberate v1 boundary + OQ-5). A class Verus discharges but the Obligation cannot yet represent is
  recorded OUT here. Mechanically: the `class` enum's variants = the union of the `obligation.rs`
  emitters + the §6/§7 in-item auxiliaries + REGISTRY-TERMINATION; the meta/battery queries carry a
  distinct `role`.
- **AC-2 (the Engine interface is non-vacuous — Verus instantiates all four slots)** — the FRAGMENT
  (the whole frozen subset via the lowering), DISCHARGE (the `classify_verus_outcome` three-way map,
  WITH the REQ-3.1 fast-unknown remap), TRUST PROFILE ({Z3, Verus VC-gen} + the TV/lowering
  theorem), and EVIDENCE (the content-addressed proof cache key, generalized per §2(d)) are each
  filled for the Verus engine, from SHIPPED code, below.
- **AC-3 (the discharge discipline is stated with its anti-cheat invariant)** — Unknown→degrade,
  Refuted→hard-fail, failure-WITHOUT-witness→Unknown-never-Refuted are stated and tied to the
  SHIPPED `degrade::ladder_action_l3` (Counterexample is `LadderAction::HardFail`, not a degrade),
  with the fast-unknown remap (REQ-3.1) named as the one behavioral delta.
- **AC-4 (certificate attribution is specified, honest-min preserved)** — the per-obligation
  {engine, trust profile} attachment, the auditor-visible base-size difference (Lean < Verus along
  the named axes), and the UNCHANGED honest-min project aggregate are specified against the SHIPPED
  `Certificate` + `AssuranceManifest`.
- **AC-5 (engine disagreement halts)** — the Proven⊕Refuted alarm is stated as a halt, distinct from
  Proven⊕Unknown (benign), with the §1 rationale, AND guarded against the spurious-trigger the
  fast-unknown seam would otherwise cause (REQ-3.1).
- **AC-6 (the Lean engine's v1 fragment is pinned IN and OUT, with the exporter trust story)** — the
  exportable fragment = what `S`'s spine covers TODAY (contracts 8/8 classes, exec exprs,
  straight-line bodies, v1 while, spec-fns via the fuel registry); the OUT set is enumerated; the
  exporter's faithfulness is named as a NEW arm-by-arm-inspection + drift-tripwire trust item,
  INCLUDING the stabilized form (§4, the #213-corrected obligation stated against `stabilizes`, NOT a
  raw fuel index, with the RESULT value bound THROUGH stabilization, #214) and registry-population
  faithfulness (EXP), AND the three-tier export story (§4/§6 — fuel-free auto via fuel-irrelevance or
  static unfolding; ∃N∀fuel forms only for the interactive recursive path, #216).
- **AC-7 (the mutation-battery v1 is honest)** — the engine-generic battery is specified against
  `mutation_score`'s real mechanics (the per-mutant `run_verus` + #8 cache loop + the per-survivor
  #101 `equivalence_proves_equal` query). The kill semantics is stated ACCURATELY against the shipped
  `Counterexample ∪ Timeout` = killed, generalized to `Refuted ∪ Unknown-after-attempt`; "untested
  against engine X" = never-attempted (no fragment admits it). The floor rule incorporates the SHIPPED
  #101 exclusion — survivor = (Proved-after-attempt) MINUS proven-equivalent, denominator = attempted
  MINUS proven-equivalent (the SHIPPED `scored`), the equivalence probe a §0.1 meta-query OUTSIDE the
  Engine interface in v1 (F3) — plus the minimum-attempted guard + the 0/0 backstop; DECIDED; NO
  inflation of the kill ratio and NO regression of equivalent-mutant handling.
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
  `Proved` / `Timeout` / `Counterexample` (the docs at `VerusOutcome::Counterexample` note that
  bucket ALSO absorbs the fast-`unknown` incompleteness edge — see §0.1 / REQ-3.1). There is no
  engine abstraction — REQ-2 introduces one and refactors this path behind it. **SHIPPED (the path),
  NOT-STARTED (the abstraction).**
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
- **The content-addressed proof cache is the evidence substrate.** `pub fn cache::cache_key(
  lowered_src, seed, verus_version, thermite_version)` hashes those FOUR args PLUS the
  `CHECK_SCHEMA_VERSION` check-logic version (blocker #49), each domain-tagged + length-prefixed,
  into a sha256 content address; `cache::load`/`store` serve/persist it. The key is NOT keyed on a
  bare AST/env hash — it is `{lowered source, seed, verus_version, thermite_version,
  CHECK_SCHEMA_VERSION}`, so a verus toolchain bump or a gate-logic change forces a universal MISS
  ("version-keyed invalidation", REQ-5 of `cache.rs`). REQ-2's EVIDENCE slot generalizes this — see
  §2(d) (the key must gain an engine discriminator AND the per-engine analogs of `verus_version`:
  the engine-toolchain version + the targeted spine content hash). **SHIPPED (cache),
  NOT-STARTED (engine-keying).**
- **The mechanized semantics `S` is the obligations' target.** `lean/Thermite/` mechanizes `S` over
  the frozen `Expr`/`Block` inductives: `denote`/`refDenote` (`Denote.lean`/`RefEncode.lean`, the
  fuel-indexed contract sublanguage `S_C` with the `Env.specs` registry), `Exec.lean`'s `execDenote`
  (`S_E`, bounded value / overflow-as-`none`), `Exec/Stmt.lean`'s `bodyDenote` (`S_B`, straight-line
  state transformer), `Exec/Loop.lean`'s `loopDenote` + `while_rule` (the fuel-indexed v1-while
  iteration, PARTIAL correctness via the `h_run` exits-hypothesis). The (T1) soundness theorems
  (`ref_sound_eq`, `exec_ref_sound`, `body_ref_sound`) and the (T2) capstone `lowering_faithful`
  (`Faithfulness.lean`) are kernel-checked with axioms `{propext, Classical.choice, Quot.sound}`.
  **Critically for §4 (the stabilized form, #213):** the `specCall` arm is FUEL-INDEXED and bottoms
  in TWO sorts (the #213 ground truth, against the spine): in PROP position `denote`'s `specCall`
  bottoms to `True` (the `fuel+1, Expr.specCall …` arm unmatched at fuel 0 → catch-all
  `| _, _, _ => True`, AND `| none => True` at an unresolved name); in INT position `intVal`'s
  `specCall` bottoms to `0` (`| none => 0` + fuel-0 catch-all `| _, _, _ => 0` — `Denote.lean`).
  Both bottoms are sound for T1 (an EQUALITY of two IDENTICALLY-fuelled denotations — `refDenote`
  bottoms identically) but are the trap §4 must close for the ONE-SIDED exported obligation — and
  the INT-position `0` bottom (the CANONICAL `result == spec_sum(xs)` shape) is exactly what made the
  cycle-2 fuel form FALSE for correct items (the critic's pin `PinIntBottom.lean`); §4 closes it with
  the STABILIZATION form, not a fuel index. **SHIPPED (epic #169 complete for the frozen subset).**
- **The anti-Goodhart battery is engine-blind.** `forge::check::mutation_score` generates mutants
  (`mutation::generate`), lowers + re-`run_verus`-es each (through the #8 cache), and counts
  `killed`/`scored`/`equivalent`. Its SHIPPED kill rule (step 3): "a `Proved` mutant SURVIVED; a
  `Counterexample` / `Timeout` mutant is KILLED" (`mutant_outcome_is_survivor =
  matches!(Proved)`; `mutation.rs` REQ-4 "Killed (counterexample / timeout)"). **Critically (the #101
  equivalence exclusion):** a surviving (`Proved`) mutant is then run through the
  `equivalence_proves_equal` query (`check.rs`); if it is PROVEN semantically equal to the real body
  it is EXCLUDED from BOTH the survivor set AND `scored` (the code: `if proved_equivalent { equivalent
  += 1; continue; }`, commented "REQ-2/REQ-4: excluded from BOTH the survivor set AND `scored`"). So
  the SHIPPED denominator `scored` = attempted MINUS proven-equivalent, and the SHIPPED survivor set =
  `Proved`-after-attempt MINUS proven-equivalent. The kill ratio is `killed / scored` over that
  reduced denominator. It is hardcoded to Verus, and `equivalence_proves_equal` is one of the §0.1
  meta-queries (scoped OUT of the Engine interface in v1, OQ-5). REQ-9 generalizes it — see §7 for the
  accurate restatement of this `Counterexample ∪ Timeout` kill semantics WITH the #101 exclusion.
  **SHIPPED (Verus-only).**

### 0.1 The three SHIPPED verus-query classes that are NOT certification obligations (AC-1, F3)

Beyond the per-item L3 certification path (`lower → run_verus → assemble_certificate`),
`check.rs`'s pipeline issues THREE further classes of direct verus query whose verdict discipline is
NOT "Proven→certify / Unknown→degrade". They are battery/meta queries ABOUT contracts, not
item-correctness obligations, and the §0 pipeline summary
(`parse → validate → check_effects then per item lower → run_verus → assemble_certificate`) omitted
them. The full pipeline, ordered:

1. `parse → validate → check_effects`;
2. per item: `gate_fn` (#6 structural triage / slag / boundary short-circuits) →
   **`vacuity_solver::solver_vacuity_check(f, &spec_items, seed, rlimit)`** (AFTER the gate, BEFORE
   L3) → `item_subprogram → lower → run_verus → assemble_certificate` → the #12 mutation battery
   (`mutation_score`, which internally re-`run_verus`-es per mutant) → the #14 strengthen probe
   (`strengthen::probe`, which threads a `run_verus` closure);
3. the #101 survivor-equivalence query (**`equivalence_proves_equal`**) where a mutation survivor
   must be proven semantically equal.

The three meta/battery classes, and why each is scoped OUT of the Engine interface in v1:

- **(A) `solver_vacuity_check` — INVERTED polarity.** Per `vacuity_solver.rs`: it runs TWO verus
  harnesses per fn before L3; the verdict map (`interpret_summary`) is "PROVED → `Detected`
  (the BAD news — the contract is degenerate → REJECT); FAILED → `Clean` (proceed)". This is the
  EXACT INVERSE of REQ-3's `Proven→certify / Unknown→degrade` discipline: here `Proven → REJECT`,
  `Failed → proceed`. That polarity inversion is the proof that this query does NOT fit the Verdict
  discipline — it is a tautology/vacuous-precondition DETECTOR, not a correctness obligation.
- **(B) `equivalence_proves_equal` (#101).** A survivor-equivalence query: discharges
  "the mutated survivor is semantically EQUAL to the original" via `run_verus` + the #8 cache. It is
  a battery sub-query (a meta question about a mutant), not the item's own correctness obligation.
- **(C) `strengthen::probe` (#14).** Verifies CANDIDATE `ens` against the real body via the threaded
  `run_verus` closure (a `Proved` candidate HOLDS, a non-`Proved` is DISCARDED) and surfaces
  advisory `Suggestion`s — it never gates the cert (`Certificate::with_strengthening` is additive,
  `level`/`reject`/oracle untouched). It is a meta query about how to TIGHTEN a contract.

**v1 scoping DECISION (F3): all three remain DIRECT verus invocations OUTSIDE the Engine interface.**
They are battery/meta queries about contracts, not item-correctness obligations; the polarity
inversion of (A) demonstrates they do not fit the Verdict discipline (REQ-2/REQ-3). They keep their
own bespoke verus calls in increment (i) — the byte-identical Verus refactor moves the per-item L3
CERTIFICATION path behind the trait, not these. This is named as a deliberate v1 boundary, with
**OQ-5** carrying the future work of bringing the anti-Goodhart battery + vacuity engine-generic
(so that, e.g., the vacuity harness becomes a polarity-flagged `role = VACUITY-PROBE` obligation
with an inverted certify rule). Recorded OUT here so increment (i) does not silently regress them.

### 1. The Obligation — the backend-neutral artifact (REQ-1)

For a checked item, the verification question stated against `S`: for a contract clause,
`∀ inputs, ⟦req⟧_S → ⟦ens[result := body]⟧_S` — i.e. the (T1)-style equality the spine already
proves the reference encoder satisfies, lifted to the per-item obligation. The FUEL quantification of
this one-sided statement is pinned in §4 (the stabilized form — `stabilizes`, NOT a raw fuel index, with the
RESULT value bound THROUGH `stabilizes`, #214), NOT left free. Plus the auxiliary
classes the pipeline discharges INSIDE an item: overflow/bounds (via the bounded `S_E` — `execDenote
= none` exactly at overflow), loop entry/preservation/exit (via `loopDenote` + `while_rule`),
termination (via the source `dec` measure → the well-founded fixpoint of the fuel-indexed denotation),
AND the REGISTRY-TERMINATION class (REQ-1.2 below — the spec-fn registry's own well-foundedness).

#### 1.2 The REGISTRY-TERMINATION obligation class (REQ-1.2, the #215 fix)

**The gap (the critic's Pin B, `PinStabilization.lean`).** §4's stabilization soundness is scoped
"for a DEC-VALID (terminating) registry", but on the Lean path NOTHING discharges that hypothesis.
The parser enforces dec PRESENCE only (`SpecFnItem::dec` mandatory, `thermite-syntax/src/ast.rs`);
dec VALIDITY — that the measure actually DECREASES — is proven ONLY by Verus, and the Lean rung sits
exactly DOWNSTREAM of a Verus `Unknown` (REQ-3.1 remaps a witness-less failure, INCLUDING a failed
spec-fn termination proof, to `Unknown` → degrade → Lean attempts). For a divergent registry
`f(x) = f(x)`, the fuel denotation is CONSTANTLY the bottom 0 (the pin's
`divergent_call_is_const_bottom`), so `stabilizes` HOLDS with the bottom value
(`divergent_registry_stabilizes_to_bottom`) and `ens: result == f(x)` STABILIZES TO TRUE at
result = 0 (`divergent_contract_certifies`) — the obligation is provable with a BOTTOM-POISONED
meaning, NOT safely unprovable. Without a registry-termination obligation, a divergent spec-fn
silently self-certifies.

**The class.** REQ-1 gains a REGISTRY-TERMINATION obligation class: for an item with
`calledSpecFns(item) ≠ ∅` (the #226 assignment condition — the SAME `req ∪ ens ∪ body ∪ dec(item)`
full-expression-position transitive reachability set the §4 hard gate computes, with the closure step
over each reached spec-fn's `body ∪ dec`, NOT "non-empty registry"; a body-only OR a measure-position
spec-call therefore triggers BOTH the gate and this termination class), EVERY spec-fn `s ∈ R_item`
carries a per-spec-fn obligation that `s.dec` is a
VALID well-founded measure (it strictly descends on every recursive call, well-founded under the
sort's order). This class is ASSIGNED to every item whose `calledSpecFns(item) ≠ ∅`, and is conjoined
ITEM-WIDE by the conjunction rule of §4.1 — i.e. an item certifies only when its REGISTRY-TERMINATION
class is discharged ALONGSIDE its CONTRACT/EXEC/BODY/LOOP/OVERFLOW classes. The parser's dec PRESENCE
guarantee is the SYNTACTIC precondition; this class is the SEMANTIC one (validity), and it is NEVER
assumed.

**Discharge (two admitted paths).** (a) THE COMMON PATH: Verus's existing dec-check, which certifies
the spec-fns when Verus discharges the item — a Verus-discharged item's spec-fns have ALREADY passed
Verus's recursion/decreases check, so the class is discharged by the Verus engine. (b) THE ENGINE-#2
PATH: a Lean well-foundedness proof of the `dec` measure (a `termination_by`/`decreasing_by`-shaped
obligation over the encoded `R_item`, in the AUTO battery where the measure is scalar/linear,
otherwise INTERACTIVE). Either path discharges REGISTRY-TERMINATION; the conjunction rule requires
ONE of them per spec-fn.

**Why the measure-position closure is load-bearing (the #226 fix — Pin C, `PinDecMeasure.lean`).**
The class is ABOUT the `dec` measures, and a `dec` measure is a FULL `Expr` (`SpecFnItem.dec : Clause`
wraps an `Expr`, `thermite-syntax/src/ast.rs`) that MAY itself call spec-fns (`dec spec_size(t)` is a
natural tree measure). The Lean discharge path (b) states a `decreasing_by`-shaped descent obligation
over the encoded `R_item` (REQ-1.2(b)) — so the measure is DENOTED against `R_item`. If the closure
seed/step omitted the `dec`-position spec-calls (the cycle-5 `req ∪ ens ∪ body` body-only scope), a
spec-fn called ONLY from a measure would be ABSENT from `R_item`, its `specCall` would bottom to the
`intVal` Int-bottom `0` at every fuel, and the measure as DENOTED would differ from the SOURCE measure
— affirming a strict descent the source measure lacks. The critic's kernel-checked pin
`lean/Thermite/PinDecMeasure.lean` (Pin C) is exactly this: measure `x - t(x)` with true registry
`t(x) = x` — the SOURCE measure is `x - x = 0` (CONSTANT, the non-well-founded divergent case
REGISTRY-TERMINATION exists to reject, `true_measure_never_descends`), but denoted against a `t`-omitting
`R_item` the dec-position `t(x)` bottoms to `0` and the measure denotes `x`, which STRICTLY DESCENDS on
`x → x-1` (`closure_measure_strictly_descends`) — so a non-well-founded measure FAKES descent and the
class falsely discharges, one position to the left of #224, re-opening Pin B's divergent-contract path.
The #226 fix CLOSES it by extending the closure to the measure positions: `dec(item)` is in the SEED and
each reached spec-fn's `dec` is walked by the STEP, so a measure-called spec-fn (`t`) is now in
`calledSpecFns(item)` ⊆ `dom(R_item)`, the measure denotes against the COMPLETE `R_item`, and a
non-well-founded source measure no longer denotes to a fake-descending one. Pin C is the kernel-checked
regression oracle: its `closure_measure_strictly_descends` (the poisoned affirmation) vs
`true_measure_never_descends` (the real source measure) must stay the documented divergence the
extended closure removes. (The pin is the critic's audit artifact and is NOT touched by this doc.)

**The closure (the regression oracle: Pin B).** With the measure-position closure in force, a divergent
spec-fn `f(x) = f(x)` FAILS this class
on BOTH paths (Verus's dec-check rejects the non-decreasing measure; a Lean well-foundedness proof of
the descent obligation over the COMPLETE `R_item` cannot be authored for it — the measure now denotes
against the real registry, not a bottom-poisoned one). So the conjunction rule BLOCKS the certificate BEFORE the
poisoned-bottom stabilization can certify anything — the `divergent_contract_certifies` discharge the
pin records can no longer reach a certificate, because the item never clears REGISTRY-TERMINATION.
`lean/Thermite/PinStabilization.lean` (Pin B) is the kernel-checked regression oracle for this class:
its `divergent_registry_stabilizes_to_bottom` / `divergent_contract_certifies` are exactly the
bottom-poisoned discharge this class must keep UNREACHABLE at the certificate level. (The pin is the
critic's audit artifact and is NOT touched by this doc.) **NOT-STARTED — increment (ii) lands the
Lean-path discharge; the class assignment is increment (i)'s REQ-1 reification.**

The artifact reifies the content `obligation.rs` materializes, prover-neutrally:

```
Obligation {
  item:      ItemId,              // the fn / spec-fn the obligation belongs to (§5.3 per-item)
  class:     ObligationClass,     // CONTRACT | EXEC | BODY | LOOP_ENTRY | LOOP_PRESERVATION
                                  //   | LOOP_EXIT | OVERFLOW | TERMINATION | REGISTRY_TERMINATION
                                  //   (AC-1: = the obligation.rs emitters ∪ the §6/§7 in-item
                                  //   auxiliaries ∪ REQ-1.2's registry-termination class)
  role:      ObligationRole,      // CERTIFICATION (REQ-3's discipline applies). The §0.1 meta
                                  //   queries (vacuity/equivalence/strengthen) are NOT minted as
                                  //   Obligations in v1 (they stay direct verus, OQ-5); the field
                                  //   is the seam that will carry their inverted/advisory roles.
  ast_slice: ExprOrBlock,         // the parsed thermite-syntax node(s) — the SAME `source: &Expr`
                                  //   / `body: &Block` the obligation.rs functions consume
  env:       ObligationEnv {      // the typing/env context — the prover-neutral generalization of
                                  //   thermite-tv's ObligationFrame
    params:        Vec<(Name, ThermiteType)>,   // free vars at their THERMITE types (not Verus
                                                //   strings) — the engine renders them
    req:           Option<ExprId>,              // the enclosing precondition (an AST node, not text)
    spec_defs:     Vec<SpecFnId>,               // the in-scope spec-fn / combinator defs (by id,
                                                //   resolved against the SHARED frozen registry);
                                                //   §4 EXP requires the EXPORTED registry contain
                                                //   exactly these with their REAL bodies, AND each
                                                //   carries a REGISTRY_TERMINATION obligation (REQ-1.2)
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
load-bearing inversion: the obligation stops being Verus-shaped. **SHIPPED — increment (i),
blocker #204** (`forge/src/obligation.rs`: `Obligation`/`ObligationClass`/`ObligationRole`/`ObligationEnv`; the Verus rendering stays the Verus engine's job).

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

  // (d) EVIDENCE — replayable, cacheable; the cache key gains an engine discriminator + the
  //     per-engine version axes (see below).
  fn evidence_key(&self, o: &Obligation) -> CacheKey; // generalizes cache::cache_key
}
```

The first instance refactors Verus byte-identically EXCEPT the named REQ-3.1 fast-unknown remap
(AC-2):

- **FRAGMENT** = the whole frozen subset reachable via the lowering (everything `thermite_lower::
  lower` + `run_verus` handle today: contracts, exec, straight-line bodies, v1 while, spec-fns,
  ADTs, the boundary/slag short-circuits stay engine-independent gates AHEAD of discharge). Verus
  ADMITS the REGISTRY_TERMINATION class (its dec-check is the common discharge path, REQ-1.2(a)).
- **DISCHARGE** = `classify_verus_outcome`'s three-way map, lifted to `Verdict`: `Proved` →
  `Proven`; `Timeout` → `Unknown(VerusTimeout + the SolverProfile)`; and `Counterexample` SPLIT by
  REQ-3.1 — `Refuted` is reserved for a genuine WITNESSED countermodel OR a definitive FRONTEND
  rejection (an ill-typed lowered unit — the IFC un-typeable-by-design tooth, which the provenance
  corpus `06-provenance/cases.json` pins at L0; e.g. the `careless_query` E0308 path). The remap to
  `Unknown(VerusIncompleteUnknown)` is implemented as a NARROW signature — a span-less diagnostic
  whose text carries the SMT-`unknown` substring AND no frontend `error[E…]` — which, per the
  grounded verus-output study (`solver-profiles.md`: a genuine fast SMT-`unknown` prints `error:
  postcondition not satisfied` VERBATIM WITH a span, the SAME spanned string a witnessed
  counterexample prints), matches NO grounded verus output today and is therefore INERT. Degrading
  genuine fast-unknowns requires Z3's `:reason-unknown` surfaced (the `solver-profiles.md` OQ-1
  prerequisite — `(incomplete quantifiers)` vs `resourceout` vs `sat`); until that activation
  condition lands the narrow remap fires on no real input and the conservative hard-fail stands
  (remapping on the available spanned signal would launder genuine countermodels to L1 — the
  anti-cheat catastrophe). So the refactor is byte-identical to the shipped pipeline: the seam is
  shipped, the behavioral delta is not yet deliverable. See REQ-3.1.
- **TRUST PROFILE** = `{Z3, Verus VC-gen}` + the TV/lowering theorem (`lowering_faithful`, RELATIVE
  to `{Z3 soundness, S = intended meaning, Lean kernel}` per `Faithfulness.lean`). I.e. a Verus L3
  enumerates Z3 + the Verus VC generator + the per-run TV's Z3-trusted `h_tv` premise.
- **EVIDENCE** = the content-addressed proof cache entry. The SHIPPED `cache_key` is
  `{lowered source, seed, verus_version, thermite_version, CHECK_SCHEMA_VERSION}` (NOT a bare AST/env
  hash). The generalized `evidence_key` (F4) is `{obligation content, seed, ENGINE name,
  ENGINE-TOOLCHAIN version, TARGETED-SPINE content hash, thermite_version, schema_version}` where:
  - the ENGINE name is the new discriminator so a Verus proof and a Lean proof of the same item never
    collide;
  - the ENGINE-TOOLCHAIN version is `verus --version` for the Verus engine (the existing
    `verus_version` slot), and for a Lean engine it is the `lean-toolchain` rev + the `lake-manifest`
    revs (mathlib / Lean-SMT / cvc5) — the Lean analog the shipped key has NONE of;
  - the TARGETED-SPINE content hash is the `lean/Thermite/` definitions the exported theorem
    INSTANTIATES (a content hash of the spine, or a pinned tag) — so a change to `Denote.lean`/
    `Exec/*` that the obligation depends on invalidates a cached `Proven`;
  - a toolchain OR spine bump therefore forces a universal MISS (matching the shipped
    `verus_version`/`CHECK_SCHEMA_VERSION` version-keyed invalidation, `cache.rs` REQ-5), so a cache
    HIT == a FRESH verify against the CURRENT semantics + toolchain (`cache.rs` REQ-2). CI replays
    evidence: on a toolchain/spine bump the affected cache entries MISS and the proofs re-run in CI
    (a hit skips replay, so the version axes — not CI alone — are what guarantees freshness). For
    grounding: the SHIPPED `cache::cache_key(lowered_src, seed, verus_version, thermite_version)`
    takes FOUR arguments and folds the `CHECK_SCHEMA_VERSION` constant in internally (`cache.rs`:
    "hashes the four args PLUS the `CHECK_SCHEMA_VERSION`"), so the shipped key composes FIVE inputs:
    {lowered source, seed, verus_version, thermite_version, CHECK_SCHEMA_VERSION}. The generalized
    `evidence_key` (F4) likewise composes the verdict-determining inputs: the item/obligation content,
    the seed, the ENGINE name, the ENGINE-TOOLCHAIN version (the `verus_version` analog), the
    TARGETED-SPINE content hash / pinned tag (the semantics version), the thermite_version, and the
    obligation schema version.

The Lean engine instantiates the same four slots (§4). **SHIPPED (Verus instance) — increment (i) built the
trait + the Verus instance (`forge/src/engine.rs` `Engine`/`VerusEngine`); the Lean instance is increment (ii). Blocker #204.**

### 3. The discharge discipline (REQ-3, generalized off the SHIPPED ladder)

For `role = CERTIFICATION` obligations (the §0.1 meta/battery queries are out of scope — they keep
their own polarity):

```
Verdict::Proven(_)    → certify at this engine's level (L3 for a sound-for-all-inputs engine);
                        attach {engine, trust_profile} (REQ-4).
Verdict::Unknown(_)   → DEGRADE per degrade::run_ladder (try the next engine in REQ-8's order,
                        then L2/L1). An Unknown is NOT a failure verdict — it is "this engine
                        could not decide." A witness-less prover failure is HERE (REQ-3.1).
Verdict::Refuted(cx)  → HARD-FAIL. NEVER degrades, NEVER tries another engine to launder it.
                        The counterexample is the deliverable (§5.1 "counterexamples, not
                        adjectives"). Refuted requires a WITNESSING input. This generalizes
                        `ladder_action_l3`'s `Counterexample → LadderAction::HardFail`.
```

The anti-cheat invariant (AC-3): a tactic battery EXHAUSTING itself, a solver TIMING OUT, or an SMT
`unknown` (incompleteness) is `Unknown`, **never** `Refuted`. Refutation requires a genuine
countermodel — a witnessing input on which the contract demonstrably fails. This is the
engine-generic statement of the SHIPPED rule (`degrade-ladder.md` REQ-2): a counterexample never
degrades, a timeout does. **SHIPPED — increment (i) wired `Verdict` into `run_ladder` via `engine::verdict_ladder_action`; #204.**

#### 3.1 The fast-unknown seam (REQ-3.1, F5/F1 decision)

The SHIPPED `classify_verus_outcome` absorbs the SMT incompleteness-`unknown` into the
`Counterexample` bucket. Grounded: the `VerusOutcome::Counterexample` doc says this bucket "ALSO
absorbs the incompleteness-unknown edge (an `unknown` returned FAST without exhausting the rlimit →
no profile → treated as the failure path, OQ-1)", and the witness-less fallback emits a generic
`ObligationResult::failed` with NO witnessing input. So a naive byte-identical Verus engine would map
this fast-`unknown` to `Verdict::Refuted` → `ladder_action_l3` HardFail — which CONTRADICTS REQ-3
("an SMT unknown is `Unknown`, never `Refuted`; refutation requires a witnessing input") from day
one.

**DECISION (increment (i) ships a NARROW remap; it is currently INERT).** `Refuted` is reserved for
two definitive signals: (1) a genuine WITNESSED countermodel (a `Counterexample` carrying a parsed
failing input — a real disproof), and (2) a definitive FRONTEND rejection — an ill-typed lowered unit,
the IFC un-typeable-by-design tooth, which the provenance corpus `06-provenance/cases.json` pins at L0
(e.g. the `careless_query` E0308 path; a literal "witness-less ⇒ Unknown" reading would WRONGLY degrade
this corpus-pinned L0 frontend rejection — that is why the discriminator is narrow, not broad). The
remap to `Unknown(VerusIncompleteUnknown)` is implemented as a NARROW positive signature
(`engine::counterexample_is_incompleteness_unknown`): a span-less diagnostic whose text carries the
SMT-`unknown` substring AND no frontend `error[E…]`. **This narrow signature matches NO grounded verus
output today, so the remap is INERT.** Per the grounded verus-output study (`solver-profiles.md`:
"The reliable signal is Z3's `:reason-unknown`"; the `--output-json` summary CANNOT tell a fast SMT
`unknown` from a counterexample), a genuine fast SMT-`unknown` prints `error: postcondition not
satisfied` VERBATIM WITH a span — the SAME spanned string a witnessed countermodel prints (live repro:
a VALID Cauchy-Schwarz contract `(a²+…+e²)*5 ≥ (a+…+e)²` under `req a<10…` has NO countermodel yet
forge gives an L0 hard-fail in ~214ms with a spanned `postcondition not satisfied`, NO degrade). So the
span-less + `unknown`-substring positive signal never fires on real stderr.

**Behavioral delta, stated honestly — undelivered and not-yet-deliverable.** The seam is shipped; the
behavioral delta is NOT, because no real input reaches the remap. Today's pipeline HARD-FAILS a genuine
fast-`unknown` (spanned `postcondition not satisfied` → the witnessed-`Counterexample` path →
`ladder_action_l3` HardFail), and the narrow remap does NOT change that — its span-less +
`unknown`-substring trigger is absent from grounded verus output. **The activation condition** for a
LIVE remap (a future increment) is Z3's `:reason-unknown` surfaced — distinguishing
`(incomplete quantifiers)` (degrade) from `resourceout` (timeout-degrade) from `sat`
(witnessed-refute) — i.e. the `solver-profiles.md` OQ-1 prerequisite (the `--log-all`-artifact
mechanism). **Until then the conservative hard-fail STANDS, and is justified:** degrading on the only
signal currently available (a spanned `postcondition not satisfied`) would launder genuine WITNESSED
countermodels to L1 — the anti-cheat catastrophe (§7 / `degrade-ladder.md` REQ-2: a counterexample
NEVER degrades). The conformance cert-oracle regression is therefore byte-identical with NO exception:
the corpus contains witnessed failures + E0308 frontend rejections (both stay `Refuted` → hard-fail)
but no genuine SMT-`unknown` matching the narrow signature, so every `conformance/*.cert.json` is
unchanged.

**A note on a failed registry-termination proof (REQ-1.2 interaction).** A spec-fn whose `dec`
fails Verus's decreases-check produces a witness-less Verus failure → `Unknown` → degrade → Lean
attempts. Per REQ-1.2 this does NOT let a divergent registry sneak through: the Lean rung must still
discharge the REGISTRY-TERMINATION class (a Lean well-foundedness proof), which a divergent spec-fn
cannot satisfy, so the conjunction rule blocks the item. The remap routes a failed termination proof
to a re-attempt, not to a silent certification.

**The interactions this closes (F1):**
- **No spurious Proven⊕Refuted halt (REQ-5).** A Verus fast-`unknown` can no longer be misread as
  `Refuted`, so a Lean kernel `Proven` + a Verus fast-`unknown` is now `Proven ⊕ Unknown` (benign),
  NOT a false soundness alarm.
- **REQ-9 kills require witnessed refutation OR the F2 rule.** A mutant that produces a Verus
  fast-`unknown` is no longer counted as `Refuted`-killed; it is `Unknown`. Under §7's engine-generic
  kill semantics (`Refuted ∪ Unknown-after-attempt`) it STILL counts as killed because it was
  ATTEMPTED and not Proven — preserving the shipped `Timeout`/unknown=killed behavior — but it is NOT
  laundered into the witnessed-refutation count. The two paths to "killed" are explicit: a witnessed
  `Refuted`, or an attempted-and-unproven `Unknown` (F2).

**SHIPPED — increment (i) implements the remap (`engine::VerusEngine::verdict_of` + `counterexample_is_incompleteness_unknown`, the NARROW SMT-`unknown` signature); #204.**

### 4. The Lean engine (engine #2) (REQ-6/REQ-7)

**The EXPORTER (REQ-6).** `forge` serializes an `Obligation` into a Lean theorem statement over the
EXISTING spine encodings — the `Expr`/`Block` inductives + `denote`/`bodyDenote`/`loopDenote` in
`lean/Thermite/`. The exporter emits Lean SOURCE that INSTANTIATES those definitions. Crucially the
exporter does NOT define a new semantics — it targets the already-kernel-proven `S`, so its
faithfulness is the SAME correspondence class as the Rust↔Lean encoder correspondence: **arm-by-arm
inspection** (each Thermite AST construct ↦ its `Expr` constructor, quoting both sides) **+ the
deep-audit drift tripwire** (`scripts/audit.sh` check [4], the SHA-pinning discipline
`rust-lean-correspondence.md` uses — any change to the exporter or the targeted spine arms
invalidates the audit row and forces re-inspection). This is named here as a NEW trust item of that
exact discipline: **(EXP) — the exporter emits Lean source that, arm-by-arm, instantiates the
kernel-proven `S` definitions for the construct it exports, AND populates the spec-fn registry
faithfully** (see "registry faithfulness" below). It is NOT a stronger extraction bridge; it is the
inspection tier, honestly. **NOT-STARTED — increment (ii)/(iv).**

**The stabilized form (REQ-6, F5 DECISION — restated for #213; the result-binding fix for #214;
supersedes the cycle-2 "fuel form").**
The exported obligation is stated against a STABILIZATION RELATION, not a raw fuel index, and the
RESULT value is bound THROUGH that relation, not at a concrete value. This is the
load-bearing soundness choice, and the cycle-2 "∀ fuel ≥ fuel₀" form is RETIRED — it was FALSE for
correct items. The correction credits the critic's kernel-checked pins `lean/Thermite/PinIntBottom.lean`
(`obligation_form_is_false`, the #213 oracle) and `lean/Thermite/PinStabilization.lean` (Pin A, the
#214 oracle), which are KEPT as the regression oracles for this section (the new form must stay
consistent with BOTH — see "consistency with the pins" below). Both pin files are the critic's audit
artifacts and are NOT touched by this doc.

**Why the fuel form was false (the #213 ground truth, against the spine).** `Denote.lean`'s `intVal`
bottoms an INT-position `specCall` to `0` (the `fuel+1, Expr.specCall …` arm resolves `| none => 0`,
and the fuel-0 catch-all is `| _, _, _ => 0`) — NOT to `True`. The CANONICAL contract shape
(`result == spec_sum(xs)` — the doc's own flagship, quoted in `Exec.lean`'s header) puts the
`specCall` in INT (comparison-operand) position, where `intVal` governs. So at a fuel BELOW the call's
unfolding depth the conjunct is the CONTENTFUL `result = 0` (the bottomed value), which is FALSE for a
CORRECT item — not a trivially-true conjunct. The cycle-2 claims — "`denote` can only make the
obligation EASIER by bottoming a `specCall` to `True`" and "an under-computed fuel₀ only adds a
TRIVIALLY-TRUE conjunct" — are therefore both FALSE; only the PROP-position bottom (`denote`'s
`specCall` arm, `| none => True` / fuel-0 catch-all `True`) is `True`, and the canonical case is the
Int-position one. Worse (the value-dependent corollary the pin records): for `result == spec_f(xs)`
with unfolding depth |xs| and `v` ∀-quantified over unbounded seqs, EVERY finite fuel admits an env
with |xs| > fuel whose conjunct is false — so NO globally-fixed `fuel₀` makes the ∀-fuel form hold for
the headline recursive item. fuel₀ is RETIRED from the form (it survives ONLY as a non-load-bearing
exporter HINT to seed the auto-tactics' unfolding budget — see "fuel₀ as a hint" — it is NOT part of
the obligation statement).

**Why the result must be bound through stabilization (the #214 ground truth, Pin A).** The cycle-3
form displayed `stabilizesProp ens (Env.bindInt env "result" rbody)` with `rbody` bound by NOTHING —
no quantifier, only the prose "binds via `Env.bindInt` after stabilization" and the fuel₀ hint as the
only computational story. For an ENV-DEPENDENT body there is NO concrete value computable at export
time, so the prose cannot be implemented as a value — and the only computational rendering §4 offered
(the fuel hint) RE-INTRODUCES the #213 Int-bottom unsoundness on the BODY side: the critic's Pin A
registry (`f(x)=g(x)`, `g(x)=5`) has the body `f(x)` stabilizing to 5 (`body_stabilizes_to_5`) but
bottoming to 0 at the hint fuel 1 (`rbody_at_hint_fuel_is_bottom`), and the WRONG contract
`ens: result == 0` DISCHARGES under the hint-fuel rendering (`wrong_contract_certifies_with_
underfuelled_rbody`) while being REFUTED at the true value 5 (`wrong_contract_fails_at_true_value`).
The fix: BIND the result THROUGH the stabilization relation — quantify `r` and require the body to
STABILIZE to it, asserting NO concrete export-time value. By the uniqueness of stabilization (a
per-env `N` beyond which `intVal` has stopped changing pins ONE value — argued below), `r` is forced
to the body's TRUE stabilized value; there is nothing for the exporter to compute and emit.

**The form: stabilization.** Define (the increment-(ii) spine prerequisite, a small Lean addition —
see the build-blocker note below) the stabilization relation, per-env, on the INT side and its Prop
analogue on the Prop side:

```
-- the SPINE PREREQUISITE (increment (ii) lands this in lean/Thermite/, NOT yet built):
def stabilizes (e : Expr) (env : Env) (v : Int) : Prop :=
  ∃ N, ∀ fuel, fuel ≥ N → intVal fuel e env = v        -- the INT-position stabilized value

def stabilizesProp (e : Expr) (env : Env) : Prop :=     -- the Prop-position analogue
  ∃ N, ∀ fuel, fuel ≥ N → denote fuel e env             -- "denote stabilizes to True"
```

`stabilizes e env v` says: there is a per-env threshold `N` beyond which `intVal` has stopped changing
and equals `v`. `stabilizesProp e env` says: there is a per-env `N` beyond which `denote` is `True`.
The threshold `N` is PER-ENV — it is NOT a global `fuel₀`; this is exactly what fixes the critic's
value-dependent-depth counterexample (an env with a large |xs| simply has a large `N`, and there is no
claim of one finite fuel that works for all envs). **Uniqueness of stabilization (the #214 lever):**
`stabilizes e env v` determines `v` UNIQUELY — if `stabilizes e env v₁` and `stabilizes e env v₂`,
then at any `fuel ≥ max(N₁, N₂)` we have `v₁ = intVal fuel e env = v₂`. This is what makes binding the
result THROUGH the relation (rather than at a concrete export-time value) well-defined: the bound `r`
below is forced to the body's true stabilized value, and an env-dependent body needs no export-time
value at all.

The exported obligation, for a CONTRACT clause over the concretely-fixed registry `R_item` (still
held fixed — see the registry hard gate below, which is UNCHANGED), is:

```
-- the EXPORTED file fixes the registry concretely (UNCHANGED — see the hard gate below):
def R_item : Thermite.Registry := fun name =>
  match name with
  | "spec_sum" => some { params := ["xs"], body := <Expr-encoding of spec_sum's real body> }
  | …          => …                       -- exactly calledSpecFns(item) (§4 hard gate: every
  --                                          spec-fn reachable from req ∪ ens ∪ body ∪ dec(item),
  --                                          TRANSITIVELY, closure-step over body ∪ dec; #226),
  --                                          each real-bodied
  | _          => none

-- reqStable / ensStable: each clause STABILIZES to True at the env (Prop side); a comparison whose
-- operand is an Int-position specCall stabilizes via the underlying `stabilizes` on that operand.
-- The RESULT value is bound THROUGH `stabilizes body_expr env r` (the #214 fix) — NO concrete
-- export-time value; uniqueness of stabilization forces r to the body's true stabilized value.
theorem item_xyz :
  ∀ (v : Env),
    let env := { v with specs := R_item }                 -- registry HELD FIXED
    ∀ (r : Int),                                          -- the result value, quantified
    stabilizes body_expr env r →                          -- ... and BOUND through stabilization
    stabilizesProp req env →                               -- reqStable(env): req stabilizes to True
    stabilizesProp ens (Env.bindInt env "result" r)        -- ensStable: ens stabilizes to True at r
```

(`{ v with specs := R_item }` is the Lean env-composition: `v` provides the `ints`/`seqs`/`optres`
valuation, `R_item` OVERRIDES `specs`. `req`/`ens`/`body_expr` are the encoded contract / body
`Expr`s; `r` is the PURE-CONTRACT item's result value, bound THROUGH `stabilizes body_expr env r` —
see §4.1 for why this is an `Int` denoting via `intVal` ONLY for the pure-contract class scoped here,
and why the general exec-body bridge is increment (iv)'s own work. The prose names
`reqStable`/`ensStable` are `stabilizesProp req env` / `stabilizesProp ens …`. The `∀ r, stabilizes
body_expr env r →` premise is the #214 result-binding fix — the exporter emits NO concrete `rbody`
value; uniqueness of stabilization makes `r` the body's true stabilized value.)

**Soundness argument (one paragraph — crediting the critic's #213/#214 pins).** The
obligation says: at every env, for the (unique) value `r` the body STABILIZES to, IF `req` stabilizes
to True THEN `ens` stabilizes to True at `result = r`. This is
sound for a DEC-VALID (terminating) registry by the supporting lemma `stabilization_exists_for_dec_
bounded`: because the source `dec` measure makes every spec-fn's recursion well-founded — a property
the REGISTRY-TERMINATION class (REQ-1.2) DISCHARGES rather than assumes — each
`specCall` reachable from `req`/`ens`/the body/the dec measures — the FULL `calledSpecFns(item)`
full-expression-position transitive closure the
hard gate populates `R_item` with (#226) — has a FINITE unfolding depth PER ENV, so
`intVal`/`denote` reach a fixed value at some finite per-env `N` and STAY there — the stabilized value
EXISTS and equals `S`'s intended meaning of the clause at that env. The body's `r` is bound THROUGH
`stabilizes body_expr env r`, so it is the body's TRUE stabilized value, NOT a fuel-bottomed artifact;
uniqueness of stabilization makes it the only `r` the premise admits. Crucially the existential `N` is
PER ENV: no global finite `fuel₀` is claimed, so the value-dependent-depth counterexample the critic
recorded (an env with |xs| > any fixed fuel) is no longer a falsifier — that env simply has a larger
`N`. The Int-bottom (`intVal`'s `0` arm) and the Prop-bottom (`denote`'s `True` arm) live ONLY at
fuels BELOW `N` — below the per-env stabilization threshold — and the stabilized value is by
definition the value at fuels `≥ N`, so the bottom arms NEVER touch the stabilized value the
obligation quantifies over (for `req`/`ens` OR for the body's `r`). (Contrast:
the retired ∀-fuel form quantified over fuels BELOW `N` too, which is exactly where the `0`/`True`
bottoms made a correct item's conjunct false — the bug `PinIntBottom.lean` disproves; and the cycle-3
unbound-`rbody` form rendered the body at the hint fuel, where the body's `0`-bottom certified a wrong
contract — the bug `PinStabilization.lean` Pin A disproves.) The obligation is therefore
faithful to `S`'s intended per-env meaning and free of the under-fuel artifact on BOTH the clause and
the result-binding sides.

**Consistency with the pins (the regression oracles).** TWO pins gate this section, and the new form
must pass both:
- **`PinIntBottom.lean` (the #213 oracle).** The pin's registry (`f(x)=g(x)`, `g(x)=x`) is dec-bounded
  and complete; at its env (`x=1`, `result=1`, the CORRECT item) `intVal fuel (f x) env` is `0` at
  fuel 1 (the bottom) but `1` at every fuel ≥ 2 — so it STABILIZES to `1`, and `ens = (result == f(x))`
  stabilizes to `result = 1`, i.e. to True. Under the new form the obligation HOLDS at this env (the
  stabilized value is 1, `result = 1`), exactly as it should for a correct item — whereas the retired
  ∀-fuel form was FALSE here (the pin's `obligation_form_is_false`).
- **`PinStabilization.lean` Pin A (the #214 oracle).** The pin's registry (`f(x)=g(x)`, `g(x)=5`,
  dec-bounded, complete — every §4 gate passes) makes the body `f(x)` STABILIZE to 5
  (`body_stabilizes_to_5`), so under the NEW form the result `r` is bound THROUGH `stabilizes body
  env r` and forced to `r = 5`. The wrong contract `ens: result == 0` is now UNPROVABLE: with `r = 5`
  the obligation requires `stabilizesProp (result == 0) (bindInt env "result" 5)`, which is exactly
  the pin's `wrong_contract_fails_at_true_value` (REFUTED). The unsound discharge the pin records
  (`wrong_contract_certifies_with_underfuelled_rbody`) relied on rendering `rbody` at the hint fuel 1
  (where the body is the bottom 0); the new form NEVER renders the result at a fuel — it binds `r`
  through stabilization — so that discharge path is gone. So the new form is consistent with Pin A:
  the WRONG contract is unprovable (r=5 is forced) and a CORRECT contract (`result == 5`) holds.
Any future change to this section must re-check against `PinIntBottom.lean` (#213),
`PinStabilization.lean` (#214/#215), `PinBodyRegistry.lean` (the #224 gate oracle — see the
registry hard gate below), AND `PinDecMeasure.lean` (the #226 measure-position oracle — the dec-VALIDITY
measure must denote against the COMPLETE `R_item`, see §1.2).

**fuel₀ as a non-load-bearing hint.** fuel₀ (the exporter-computed static-nesting bound) no longer
appears in the obligation. **It explicitly CANNOT influence the theorem's truth** — it is a TACTIC
HINT ONLY. It MAY survive as an EXPORTER HINT — a starting unfolding budget the
auto-tactic battery (§4 DISCHARGE / `decide`/`simp` unfolding) seeds itself with to find the
stabilized value faster. It is EXPLICITLY non-load-bearing: an under-computed hint costs the tactic
more unfolding steps, never soundness, because the obligation is stated over `stabilizes` (the
∃-N form) and binds the result THROUGH `stabilizes` (not at any fuel the hint pins). Pin A is precisely
the demonstration of what goes wrong if the hint EVER becomes load-bearing (it rendered the result at
the hint fuel and certified a wrong contract) — which is why the form binds `r` through stabilization
and the hint is restated here as a tactic hint that cannot touch the theorem's truth.

**The normalization story — how the AUTO fragment is actually dischargeable (the #216 fix; see §6).**
The exported obligation above is an `∃N∀fuel` statement over the DEEP embedding (`denote`/`intVal`
applied to `Expr` encodings). A `decide`/`simp`/Lean-SMT `smt` battery will NOT chew an `∃N∀fuel` goal
over the deep embedding — and the z3-demotion PoC that grounds the AUTO claim discharged SHALLOW QF_LIA
theorems (`tv_obligation_arith_cmp`), hand-translated, with NO `denote`, NO `stabilizesProp` wrapper.
The reconciliation is a THREE-TIER export story, detailed in §6, that makes the auto fragment's actual
SHAPE fuel-free and shallow (matching the z3-demotion grounding) and reserves the `∃N∀fuel` forms for
the interactive path only. The spine prerequisite this adds (increment (ii), see the build-blocker
note) is the FUEL-IRRELEVANCE lemma:

```
-- the FUEL-IRRELEVANCE lemma (increment (ii) spine prerequisite, NOT yet built):
theorem intVal_fuel_irrelevant (e : Expr) (env : Env) (h : specCallFree e) :
    ∀ f g, intVal f e env = intVal g e env
theorem denote_fuel_irrelevant (e : Expr) (env : Env) (h : specCallFree e) :
    ∀ f g, denote f e env = denote g e env     -- the Prop analogue
```

`specCallFree e` (a decidable predicate the exporter computes) means `e` contains NO `specCall` — so
its denotation does NOT depend on fuel (fuel only matters at a `specCall` unfolding). For such `e`,
`stabilizesProp e env ↔ denote 0 e env` (the witness `N = 0` works because the value is constant in
fuel), so the exporter can emit the FUEL-FREE shallow statement `denote 0 e env` — exactly the QF
shape the z3-demotion PoC discharges. This is the bridge §6 tier (a) builds on.

**BUILD-BLOCKER NOTE (the #204-chain amendment, NOT a new issue — #213/#214/#215/#216/#226 fixes).**
The full-expression-position closure (`calledSpecFns(item)` seeded by `req ∪ ens ∪ body ∪ dec(item)`,
stepped over each reached spec-fn's `body ∪ dec`) and the `specCallFree` predicate ranging over the
SAME positions (the dec clauses included where the termination tier applies, §6.1(a)) are the #226
correction — owned by the SAME #204 chain (the forge-closure mirror is recorded as a named
increment-(i) work item in the header build-blockers block; the spine-side prerequisites below stay
increment (ii)). The
`stabilizes` / `stabilizesProp` relations, the supporting lemma `stabilization_exists_for_dec_bounded`
(for a dec-VALID registry every reachable `specCall` has a finite per-env unfolding depth, so the
stabilized value exists and equals `S`'s intended meaning), the uniqueness-of-stabilization fact (the
#214 lever), the FUEL-IRRELEVANCE lemma (`intVal_fuel_irrelevant`/`denote_fuel_irrelevant`, the #216
normalization bridge), and the Lean-path REGISTRY-TERMINATION discharge (the #215 well-foundedness
proof obligation, REQ-1.2(b)) are a SMALL NAMED Lean addition that
increment (ii) MUST land in the spine BEFORE the exporter can target this form.

**SHIPPED (the spine prerequisites — #240, ref #203, `lean/Thermite/Stabilize.lean`).** The
relations `stabilizes`/`stabilizesProp` (matching `Denote.lean`'s `intVal`/`denote`), the
uniqueness-of-stabilization fact `stabilizes_unique` (the #214 lever, overlap-at-max), the
FUEL-IRRELEVANCE lemma `intVal_fuel_irrelevant`/`denote_fuel_irrelevant` (+ `seqVal`/`denoteArms`
mutual companions; over the decidable Bool predicate `specCallFree` ranging the FULL mutual AST), and
the tier-(a) fuel-free corollaries `stabilizesProp_iff_denote_zero`/`stabilizes_iff_intVal_zero` are
all kernel-checked with `{propext, Classical.choice, Quot.sound}` (NO `sorryAx`). **The supporting
lemma `stabilization_exists_for_dec_bounded` SHIPPED in the HYPOTHESIS form** (`stabilization_exists`
keyed on `RegistryTerminating env e := ∃ v, stabilizes e env v`), recording the doc-author's
least-confident-assertion flag honestly: the fully-general core-Lean form is NOT provable (the spine
registry is an arbitrary `String → Option SpecFn` with no available well-foundedness, and a divergent
registry stays at the fuel-0 Int-bottom for all fuel — the #213/#214/#215 trap, not a genuine
stabilized value), so no env-indexed measure exists in core Lean to discharge it unconditionally. The
hypothesis `RegistryTerminating` is EXACTLY the per-item REGISTRY-TERMINATION obligation class
(REQ-1.2) — the dec-validity proof of each spec-fn in `R_item` supplies it — so it is the named,
separately-discharged obligation the conjunction rule already requires, NOT an assumed-away premise
(§1.2). The spec-call-free fragment stabilizes UNCONDITIONALLY (`stabilization_exists_specCallFree`,
no hypothesis — its witness is the fuel-0 value), so the tier-(a) auto fragment's stabilization is
free. The four critic pins (PinIntBottom/PinStabilization/PinBodyRegistry/PinDecMeasure) keep their
own local `stabilizes`/`stabilizesProp` copies and still build green against the new defs (their
theorems' axioms are the standard set). The exporter targeting (REQ-6/REQ-7) and the REGISTRY-
TERMINATION Lean-path well-foundedness discharge (REQ-1.2(b)) remain increment-(ii)/(iv) work. This
is recorded as an AMENDMENT to increment (ii) inside the existing #204 build-blocker chain (see the
header `build-blockers:` block) — NOT a separately-filed issue.

**Registry population is an EXPORTER-SIDE HARD GATE (F5, the #210 fix) — not a hypothesis.** Two
mechanisms, belt-and-suspenders:

1. **The export refuses to emit on an incomplete registry.** The exporter computes
   `calledSpecFns(item)` and FAILS the export —
   refuses to write the Lean file — if `calledSpecFns(item) ⊄ dom(R_item)`. This is a mechanical
   check at export time. **`calledSpecFns(item)` is defined (the #226 fix, completing #224) by the
   FULL EXPRESSION-POSITION PRINCIPLE: EVERY expression the export denotes against `R_item` — the
   clauses (`req`, `ens`), the body, AND all termination measures (the item's `dec` and each reached
   spec-fn's `dec`) — contributes its spec-calls, transitively. Concretely the SEED is
   `req ∪ ens ∪ body ∪ dec(item)` and the closure STEP walks each reachable spec-fn's `body ∪ dec`;
   the set is closed under "a reachable spec-fn's OWN body or OWN dec measure may call further
   spec-fns," so it is the transitive closure of the call relation seeded by that union** (not
   `req ∪ ens` only — the cycle-2 scope; nor `req ∪ ens ∪ body` only — the cycle-5 body-only scope,
   which the #226 finding made unsound: a `dec`-VALIDITY obligation DENOTES the measure against
   `R_item`, and an omitted measure-called spec-fn bottoms to the `intVal` Int-bottom `0`, so a
   non-well-founded source measure denotes to a fake-descending one and REGISTRY-TERMINATION falsely
   discharges — `lean/Thermite/PinDecMeasure.lean`'s `closure_measure_strictly_descends` vs
   `true_measure_never_descends`; and, on the contract side, the prior cycle-2 hole an omitted
   body-called spec-fn opened — it STABILIZES to the Int-bottom `0`, uniqueness forces `r = 0`, and a
   wrong contract `ens: result == 0` certifies kernel-clean,
   `lean/Thermite/PinBodyRegistry.lean`'s `wrong_contract_certifies_under_body_omission`). Including
   the body AND every `dec` measure — transitively — in the reachability set is what closes both
   holes: the body-called or measure-called spec-fn is now in `calledSpecFns(item)`, so an omission
   FAILS this gate. Because the theorem holds `specs := R_item` fixed and carries NO resolution
   premise, an omission cannot self-certify a vacuous obligation: an unbuildable export is a hard
   error, not a True-bottom that proves itself. (Contrast the rejected hypothesis form, where an
   omitted entry FALSIFIED a resolution antecedent and the whole obligation followed from the false
   premise — kernel-clean but meaningless.)
2. **Per-name `decide`/`rfl` resolution lemmas are emitted ALONGSIDE.** For each
   `name ∈ calledSpecFns(item)` — i.e. for every spec-fn reachable from `req ∪ ens ∪ body ∪ dec(item)`
   transitively, closure-step over `body ∪ dec` (the #226 definition above), so a body-only call AND a
   measure-position call each get a lemma TOO — the exporter also
   emits a resolution lemma of the form
   `example : R_item "spec_sum" ≠ none := by decide` (or the `rfl`/`Option.isSome`-shaped variant
   that `decide`s on the concrete `R_item`). If the exporter ever omits a called spec-fn from
   `R_item`, the corresponding lemma FAILS TO COMPILE — so an omission also breaks the kernel check,
   independent of the build-time gate. Both stated: the gate refuses to emit, and the emitted lemmas
   refuse to compile.

**The gate's regression oracles (the #224 + #226 pins — BOTH directions).**
`lean/Thermite/PinBodyRegistry.lean` (the #224 oracle) is the kernel-checked regression oracle for the
BODY reach of the `req ∪ ens ∪ body ∪ dec(item)` reachability definition, and
`lean/Thermite/PinDecMeasure.lean` (the #226 oracle, see §1.2) is the kernel-checked regression oracle
for the MEASURE-POSITION reach (the `dec` clauses), both against the
shipped spine (their `stabilizes`/`stabilizesProp` are copied VERBATIM from the §4 definition block):
- **the omitted-registry form must be UNREACHABLE through the gate.** The pin's
  `wrong_contract_certifies_under_body_omission` discharges `ens: result == 0` for an item whose body
  `h(x)` (a spec-call reachable from the BODY only, true result 5) is OMITTED from `R_item` — under the
  cycle-2 `req ∪ ens`-only `calledSpecFns`, that omission passed the gate vacuously
  (`calledSpecFns = ∅`, `∅ ⊆ dom(R_item)`) and ZERO per-name lemmas were emitted, so the spine bottoms
  the unresolved call to `0` (`body_bottoms_at_every_fuel`), it STABILIZES to the bottom
  (`omitted_body_stabilizes_to_bottom`), uniqueness forces `r = 0` (`omission_forces_r_zero`), and the
  wrong contract certifies kernel-clean. Under the #224 definition `h ∈ calledSpecFns(item)` (it is
  body-reachable), so the export REFUSES to emit (mechanism 1) AND the `h` resolution lemma fails to
  compile (mechanism 2) — the `wrong_contract_certifies_under_body_omission` form can no longer be
  EXPORTED, so the unsound discharge is unreachable through the gate.
- **the complete-registry obligation correctly REFUSES the wrong contract.** With `R_item` populated to
  the full `calledSpecFns` (`h(x) = 5` present), the body stabilizes to 5
  (`body_stabilizes_to_5_with_full_registry`), uniqueness forces `r = 5`
  (`full_registry_forces_r_five`), and the same `ens: result == 0` obligation is REFUTED
  (`wrong_contract_fails_with_full_registry`) — confirming the omission discharge was PURELY the
  gate's old `req ∪ ens`-only scope, exactly what the #224 redefinition closes. (The pin is the
  critic's audit artifact and is NOT touched by this doc.)
- **the dec-MEASURE-position omission must be UNREACHABLE through the gate (the #226 direction).**
  `PinDecMeasure.lean`'s measure `x - t(x)` calls `t` from a `dec` position ONLY. Under the cycle-5
  `req ∪ ens ∪ body` body-only `calledSpecFns`, `t ∉ R_item`, the dec-position `t(x)` bottoms to `0`,
  and the measure denotes `x` — STRICTLY DESCENDING (`closure_measure_strictly_descends`) while the
  SOURCE measure `x - x = 0` is constant and never descends (`true_measure_never_descends`), so a
  divergent measure FAKES descent and REGISTRY-TERMINATION discharges falsely. Under the #226 set
  `t ∈ calledSpecFns(item)` (it is reachable from `dec(item)`), so the export REFUSES to emit
  (mechanism 1) AND the `t` resolution lemma fails to compile (mechanism 2) — the descent obligation
  now denotes the measure against the COMPLETE `R_item` (where `t(x) = x`, so the measure is the real
  constant `0`), and the fake-descending form can no longer be EXPORTED. (The pin is the critic's
  audit artifact and is NOT touched by this doc.)

**Registry PRESENCE ≠ registry TERMINATION (the #215 boundary).** The hard gate above guarantees the
registry is POPULATED (every called spec-fn is present, real-bodied). It does NOT guarantee the
spec-fns TERMINATE — a present-but-divergent `f(x)=f(x)` clears the gate (it is complete and
real-bodied; the pin's `Rdiv` passes the per-name `decide` lemmas). Registry VALIDITY (well-founded
descent) is the SEPARATE REGISTRY-TERMINATION obligation class (REQ-1.2), discharged by Verus's
dec-check or a Lean well-foundedness proof, and conjoined item-wide — NEVER assumed. The parser
guarantees dec PRESENCE only; this class is dec VALIDITY. §1.2 is the closure.

**Registry faithfulness stays part of EXP (the inspection tier).** Beyond presence (the hard gate
above), the EXPORTED `R_item` must bind each name to its REAL `Denote`-encoded body — a WRONG body is
an unsound certification the gate cannot catch. So body-faithfulness (each `SpecFnId` in
`Obligation.env.spec_defs` ↦ its `R_item` entry with the matching `Expr` body — for EVERY name in
the `req ∪ ens ∪ body ∪ dec(item)` full-expression-position transitive `calledSpecFns(item)`, #226,
including the body-only, measure-position, and transitively-reached names) remains part of the
arm-by-arm EXP inspection + drift-tripwire discipline. The hard gate guarantees PRESENCE mechanically;
REGISTRY-TERMINATION guarantees TERMINATION (REQ-1.2); EXP inspection guarantees the BODIES are right.

**§4 SCOPE — this sketch covers increment (ii)'s PURE-CONTRACT items ONLY (the #212 fix).** The
stabilized form above types and is sound for exactly ONE class: PURE-CONTRACT items — defined
precisely as items whose body is a PURE EXPRESSION denoting in `intVal` (the `S_C` domain), so the
result `r` is an `Int` that binds via `Env.bindInt` after stabilization. For these items, `req`,
`ens`, and the body all live in `S_C`, the `Env` is the right structure, and `stabilizesProp` /
`stabilizes` are the right relations. The FULL exec-body bridge — binding a body that denotes in the
BOUNDED `S_E`/`S_B` domain (`bodyDenote : Block → State → Option ExecVal`, `Exec/Stmt.lean`) into a
contract `ens` over `Env` — is NOT designed here. It is increment (iv)'s OWN design obligation, and is
named (not waved at) in §4.1. The doc STOPS presenting a unified S_C×S_E sketch it cannot type.

#### 4.1 The exec-body bridge (increment (iv), NOT designed here) (REQ-1.1, the #212 fix)

The cycle-2 sketch wrote `Env.bindInt { … } "result" body` with `body` an item body — which does NOT
typecheck against the spine: `Env.bindInt : Env → String → Int → Env` (`Denote.lean`) takes an `Int`,
but a general item body is a `Block` denoting via `Thermite.Exec.bodyDenote : Block → State → Option
ExecVal` (`Exec/Stmt.lean`) in the BOUNDED domain. Tying `S_C` (the contract `Env`) to `S_E`/`S_B`
(the exec `State`) in one statement is a NOVELTY — the spine's own theorems relate `refDenote`/`denote`
and `bodyRefState`/`bodyDenote` SEPARATELY; there is NO single artifact tying `S_C` and `S_E`/`S_B`
today. This sketch would be the FIRST, and the doc OWNS that this is unbuilt, open increment-(iv)
design work. The pieces increment (iv) must design (each enumerated, none designed here):

- **(value bridge)** `bodyDenote` yields `Option ExecVal` where `ExecVal = int (BVal) | bool (Bool)`
  (`Exec.lean`); `BVal { ty, value : Int }` carries its type-bound. Binding a bounded INT result into
  a contract `ens` over `Env` needs the `BVal.value : Int` extraction (the `S_E → S_C` value bridge) —
  stated NOWHERE today. (`asInt`/`asBool` project an `ExecVal`; `BVal.value` reads the bounded int.)
- **(bool sort — a SPINE ADDITION, the increment-(iv) prerequisite)** A bool-typed result has NO
  binding site: `Env` has `ints`/`seqs`/`optres` only — no bool sort, no `bindBool`. DECISION: add a
  `bindBool` to the spine (an increment-(iv) prerequisite Lean addition) rather than encoding bool as
  `Int 0/1` — the bool-sort addition keeps the bridge faithful to `ExecVal`'s `bool` variant. Named
  here as increment (iv)'s spine prerequisite (parallel to increment (ii)'s `stabilizes`).
- **(optres binding)** Option/Result-typed results (the #180 match-in-ens fragment, IN per §8) bind
  via the EXISTING `optres` env slot (`env.optres : String → OptResVal`, `Denote.lean`) — increment
  (iv) wires the body's `OptResVal` result into `optres` (the binding helper is open).
- **(env → State correspondence)** The contract `Env { ints, seqs, optres, specs }` and the exec
  `State { env : ExecEnv { vars, slices }, scope }` (`Exec/Stmt.lean`) are DISJOINT structures. The
  bridge needs the correspondence map: params → in-range `BVal` cells at their widths, `seqs : List
  Int` → `slices : List BVal`. This correspondence is specified NOWHERE; it is increment (iv)'s to
  design (it is the same correspondence class as the spine's separate `bodyRefState`/`bodyDenote`
  relation, now to be TIED to the contract side).
- **(the novelty owned)** This is the FIRST S_C×S_E/S_B-tying artifact; increment (iv) owns its
  soundness story (the env→State correspondence + the value/bool/optres bridges composed with the
  stabilized contract form of §4). Until then, the Lean engine's IN set (§8) is honestly the
  PURE-CONTRACT class for the exporter's body-binding; exec/body/loop obligations are exported as
  their OWN obligation classes (CONTRACT over the result is the increment-(iv) tie).

**THE CONJUNCTION RULE (new, NORMATIVE — closes the Option-position hole) (REQ-1.1, the #212(b) fix).**
An ITEM certifies at level L via engine E only when EVERY obligation class REQ-1 assigns to that item
is discharged — each by E or by another ADMITTED engine. The certificate's per-item entry LISTS the
classes and their per-class engine attribution (REQ-4); a MISSING class means the item does NOT
certify (and the degrade ladder applies ITEM-WIDE, not per-class — an item with one undischarged class
degrades as a whole). This forbids the hole the critic named: nothing previously stopped an engine
from certifying an item on the CONTRACT class ALONE while ignoring its OVERFLOW/BODY classes. With the
conjunction rule, that is impossible — the OVERFLOW class is MANDATORILY conjoined for any item with an
exec body, AND (REQ-1.2) the REGISTRY-TERMINATION class is MANDATORILY conjoined for any item with
`calledSpecFns(item) ≠ ∅` (the §4/#226 `req ∪ ens ∪ body ∪ dec(item)` full-expression-position
transitive set, closure-step over `body ∪ dec` — so a body-only OR a measure-position spec-call
conjoins it too). This is the rule that closes the #215 divergent-registry hole: a divergent
spec-fn fails REGISTRY-TERMINATION, the conjunction rule blocks the item, and the bottom-poisoned
stabilization (Pin B's `divergent_contract_certifies`) can never reach a certificate.

**Resolution of #212(b) — the Option position takes the HYPOTHESIZE form.** `bodyDenote` is `none`
exactly when an exec obligation fails (overflow / div-by-zero / out-of-bounds). The exec-body
obligation (increment (iv)) takes the HYPOTHESIZE position — `bodyStabilizes v = some r → ensStable(r)`
(i.e. IF the body produces a result `r`, THEN `ens` stabilizes to True at `r`). The vacuous-on-overflow
case (an always-overflowing body satisfies `ens` vacuously because `bodyDenote = none` makes the
antecedent false) is SOUND precisely because the OVERFLOW class is MANDATORILY conjoined per the
conjunction rule above: an always-overflowing body FAILS its OVERFLOW class, so the item does not
certify regardless of the vacuously-satisfied CONTRACT class. The HYPOTHESIZE form is therefore safe —
the conjoined OVERFLOW class is what rules out the vacuity, not a `∧ bodyDenote v = some r` baked into
the contract obligation (which would make REQ-1's separate OVERFLOW class redundant). This resolves the
critic's (i)-vs-(ii) tension explicitly in favor of (ii), referencing the conjunction rule as the
soundness condition. (Note the parallel with the PURE-CONTRACT result-binding of §4: there the result
is bound THROUGH `stabilizes body_expr env r` — the #214 form — which is the pure-`S_C` analogue of
this `bodyStabilizes v = some r →` HYPOTHESIZE position.)

**DISCHARGE MODES (REQ-7).**
- **(i) AUTO** — a tactic battery `omega`/`simp`/`decide`/Lean-SMT's `smt` (where applicable). The
  z3-demotion PoC (`z3-demotion.md`) pins what is REACHABLE TODAY, kernel-clean: scalar comparisons
  + logical connectives + LINEAR integer arithmetic (QF_LIA) over the contract sublanguage —
  `tv_obligation_arith_cmp` / `tv_obligation_or_le` discharge with axioms `{propext,
  Classical.choice, Quot.sound}` ONLY (no `sorryAx`, no cvc5 oracle axiom). **The AUTO fragment's
  ACTUAL SHAPE is FUEL-FREE shallow goals** (the #216 reconciliation, §6 tiers (a)/(b)): the exporter
  emits `denote 0 e env`-style statements for specCall-free obligations (via the fuel-irrelevance
  lemma) or statically-unfolded goals for non-recursive registries — exactly the QF shape the PoC's
  `tv_obligation_*` theorems are, NOT raw `∃N∀fuel` goals. OUT of auto today: the
  QF_BV bitwise fragment (blocked by an upstream `sorry` in Lean-SMT's `Bitblast.lean`), the bounded
  quantifier combinators (~30% cvc5-rule reconstruction coverage — may FAIL, i.e. `Unknown`, never
  unsound), and recursive spec-fns / `permutation_of` (need the `∃N∀fuel` stabilization form + an
  induction on the per-env depth — INTERACTIVE only, §6 tier (c)). So the
  Lean-auto FRAGMENT (REQ-2(a)) is precisely the scalar/linear contract clause exported FUEL-FREE —
  the "cheapest real win" (increment (ii)).
- **(ii) INTERACTIVE** — an agent authors a proof file checked in NEXT TO the source, replayed in CI.
  This is the path for the `∃N∀fuel` stabilization forms (recursive registries) — where synthesizing
  the per-env `∃N` witness needs induction a tactic battery cannot do. Proof-artifact management: the
  proof lives at a deterministic path keyed on the item +
  the EVIDENCE KEY (§2(d): obligation content + engine + engine-toolchain version + the targeted
  spine content hash); STALENESS is defined as the EVIDENCE KEY changing — so a changed obligation,
  a Lean-toolchain/mathlib/Lean-SMT bump, OR a change to the targeted `lean/Thermite/` spine
  definitions each INVALIDATE the proof, which must be re-authored, NEVER silently reused. This
  closes the F4 gap (an obligation-hash-only key would silently revalidate a proof after a toolchain
  or spine bump). This is the design's answer to the deferred Lean-style incremental holes (issue
  #21) at the WHOLE-ITEM tier — a proof artifact, not an in-process goal state.
  **NOT-STARTED — increment (iii).**

**TERMINATION (REQ-7).** The Lean engine's obligation set for a looping/recursive item MUST include
the item's `dec` measure (the termination obligation class) — OR the certificate honestly records
PARTIAL-CORRECTNESS-ONLY for that item. This ties directly to `while_rule`'s `h_run` premise: the
SHIPPED `while_rule` is partial correctness ("after-loop holds IF the loop EXITS"); termination is
the per-run residual (the source `dec`). A Lean engine that proves preservation+exit but NOT
termination certifies partial correctness, and the certificate must SAY so (it cannot silently claim
L3-total). **And, distinctly, the REGISTRY-TERMINATION class (REQ-1.2):** the item's spec-fn registry
`R_item` carries a per-spec-fn well-foundedness obligation that the `dec` measure VALIDLY descends —
discharged by Verus's dec-check (the common path) or a Lean well-foundedness proof (the engine-#2
path), conjoined item-wide. This is the registry analogue of the item's own `while_rule` termination,
and is what keeps the stabilized form sound (a divergent registry fails it; §1.2). **NOT-STARTED —
increment (ii) lands the Lean REGISTRY-TERMINATION discharge; increment (iii)/(iv) the item-loop
termination policy.**

**TRUST PROFILE.** A Lean L3 enumerates `{Lean kernel + the 3 standard axioms (propext,
Classical.choice, Quot.sound)}` + the exporter correspondence (EXP, now including the stabilized form (#213) +
the result-binding form (#214) + registry faithfulness). For the AUTO path via Lean-SMT, cvc5 is NOT in the base (its proof is
RE-CHECKED in the kernel — `z3-demotion.md`'s honesty crux), so the base is the kernel + standard
axioms + EXP. This base is SMALLER than the Verus base ALONG THE NAMED AXES (no Z3, no Verus VC-gen)
— the auditor-visible difference REQ-4 exposes. Whether this is a STRICT ORDER (a trust lattice) or
only "smaller along the named axes" is OQ-3 — the bases are not literal subsets (Lean's EXP is not a
subset of Verus's lowering theorem), so the ordering FORMALIZATION is deferred to OQ-3; this doc
claims only the named-axis comparison.

### 5. Certificate attribution + the disagreement rule (REQ-4/REQ-5)

**Attribution (REQ-4).** `Level::L3` is unchanged — it still means "proven for all inputs." The
certificate gains a PER-OBLIGATION attribution: for each discharged obligation, the `{engine,
trust_profile}` pair. Schema-wise this is an ADDITIVE field on `Certificate` (an
`Vec<ObligationAttribution>` or a per-level engine tag), `#[serde(default,
skip_serializing_if=…)]` — exactly the precedent set by `boundary` / `slag` / `lowered_assurance` /
`assurance_scope` (each added additively so the frozen golden `conformance/sum.cert.json` still
deserializes, R-SPEC-2). An auditor reading two L3 certs can then SEE that one enumerates `{Lean
kernel, 3 axioms, EXP}` and the other `{Z3, Verus VC-gen, lowering theorem}` — the smaller base
(along the named axes; the ordering formalization is OQ-3) is the stronger result, made visible.
Whether the attribution JOINS the cert oracle (`oracle_subset`) or is diagnostic-only is OQ-2 (the
conservative default: oracle-visible, since the trust base IS verdict-relevant — but that perturbs
the golden, so it must be designed with the corpus re-pinned).

**Project aggregation stays honest-min.** `AssuranceManifest::aggregate` is UNCHANGED — the project
headline is still `Certified(min over functions)` / `Failed`. Attribution is per-obligation
metadata, ORTHOGONAL to the level the min folds over (§9's compose-trust discipline; the same
orthogonality `assurance_scope` already has to `level`).

**Disagreement (REQ-5, AC-5).** If, for the SAME certification obligation, one engine returns
`Proven` and another returns `Refuted` (a WITNESSED countermodel) — that is a SOUNDNESS ALARM. The
toolchain HALTS (a distinguished `ForgeError`/non-cert abort), surfaces both verdicts + the refuting
counterexample, and NEVER picks the favorable Proven. A genuine countermodel from one engine
contradicting a "proof" from another means one engine (or the exporter/lowering, or `S` itself) is
unsound, and silently proceeding would launder unsoundness into a certificate — the exact failure
§1's enumerable-trusted-base promise forbids. Proven⊕Unknown is BENIGN (the Unknown engine simply
could not decide — no contradiction). **Crucially (REQ-3.1 guard):** because a Verus witness-less
fast-`unknown` now maps to `Unknown` (not `Refuted`), it CANNOT spuriously fire this alarm against a
Lean kernel `Proven` — only a WITNESSED Verus countermodel can, which is exactly the real-unsoundness
case the alarm is for. **NOT-STARTED — increment (iii).**

### 6. Engine ordering + the ladder (REQ-8)

DEFAULT order (justified): **Verus first** — it is fast, push-button, and covers the whole frozen
subset, so the common case pays no Lean cost. **Lean-auto second** — on a Verus `Unknown` (timeout
OR the REQ-3.1 fast-`unknown`), or when explicitly requested, the Lean-auto battery attempts the
scalar/linear fragment it can kernel-check (a smaller trust base on success). **Lean-interactive on
demand** — never automatic (it needs a human/agent-authored proof artifact); reached by
`forge check --engine lean` or a per-item `#[engine(lean)]` annotation (the surface is OQ-1 — both
are sketched; the per-item annotation is preferred for "this one function wants the smaller base"
without changing the whole-file default). THEN the existing L2 (Kani) / L1 (runtime) degrade rungs,
unchanged.

#### 6.1 The three-tier export story (REQ-7, the #216 fix — what the Lean engine ACTUALLY emits)

The exported obligation of §4 is an `∃N∀fuel` statement over the DEEP embedding. An auto battery
(`omega`/`simp`/`decide`/Lean-SMT `smt`) cannot discharge such a goal — and the z3-demotion PoC that
grounds the AUTO claim discharged SHALLOW QF_LIA theorems with no `denote`, no `stabilizesProp`. The
reconciliation: the exporter emits one of THREE tiers depending on the obligation's registry shape, so
the AUTO tiers produce exactly the fuel-free shallow goals the PoC demonstrates, and only the
INTERACTIVE tier carries the `∃N∀fuel` form.

- **(a) FUEL-FREE export for specCall-free obligations (AUTO).** When the obligation's `Expr`s are
  `specCallFree` (the common scalar-contract case — no spec-fn appears in any expression the export
  denotes against `R_item`: `req`/`ens`/body AND every `dec` measure carried as a REGISTRY-TERMINATION
  obligation for the item — i.e. `calledSpecFns(item) = ∅` under the #226 `req ∪ ens ∪ body ∪ dec(item)`
  full-expression-position definition; the `specCallFree` predicate the exporter computes must
  therefore range over the SAME positions the closure does, the dec clauses INCLUDED where the
  termination tier applies, so the gate and this tier stay reconciled — `calledSpecFns = ∅ ⟺
  specCallFree over {req, ens, body, dec}`), the
  exporter emits the FUEL-FREE statement `denote 0 e env` / `intVal 0 e env = …` rather than the
  `∃N∀fuel` wrapper. This is sound by the FUEL-IRRELEVANCE lemma (§4: `specCallFree e → ∀ f g, intVal
  f e env = intVal g e env`, and the Prop analogue) — for a specCall-free `e`, `stabilizesProp e env ↔
  denote 0 e env`, so the fuel-free goal is EQUIVALENT to the stabilized form but is a SHALLOW QF goal.
  This is the auto fragment's ACTUAL shape and reconciles the z3-demotion grounding: the PoC's shallow
  QF goals (`tv_obligation_arith_cmp`) are PRECISELY what fuel-free export produces. The exporter emits
  the fuel-irrelevance discharge inline (or the goal is already fuel-free after `denote 0` reduction),
  so the `smt`/`omega`/`decide` battery sees a QF_LIA goal, kernel-clean.
- **(b) STATIC UNFOLDING for NON-recursive registries (AUTO).** When the registry is non-empty but
  NON-RECURSIVE (every spec-fn's body calls only strictly-earlier spec-fns — a finite DAG), the
  exporter may STATICALLY UNFOLD every spec-fn call to its FINITE depth at export time, producing a
  specCall-free `Expr`, then apply tier (a). The unfolding depth is exactly the DAG depth (bounded,
  computed at export), so the unfolded goal is again a fuel-free SHALLOW goal the auto battery
  discharges. (The exporter's unfolding is itself part of EXP — the unfolded `Expr` must equal the
  spec-fn's real body substituted, arm-by-arm; a wrong unfolding is an unsound export the inspection
  tier catches.)
- **(c) The `∃N∀fuel` STABILIZATION form for RECURSIVE registries (INTERACTIVE only).** When the
  registry is RECURSIVE (a spec-fn whose unfolding depth is ENV-dependent — `spec_sum(xs)` with depth
  |xs|), there is NO finite static unfolding and NO fuel-free shape: the per-env `∃N` witness genuinely
  depends on the input (`N` grows with |xs|) and requires INDUCTION to synthesize. These obligations
  take the §4 `∃N∀fuel` stabilization form and are reserved for the INTERACTIVE path (REQ-7(ii)) —
  a human/agent-authored proof handles the induction; the auto battery does NOT attempt them (its
  fragment, REQ-2(a), does not ADMIT a recursive-registry obligation, so per REQ-9 it is "untested by
  Lean-auto," not a false kill or an `Unknown`). This is the honest boundary: AUTO reaches tiers
  (a)/(b) (fuel-free, the z3-demotion-grounded fragment); INTERACTIVE owns tier (c) (`∃N∀fuel`).

The §4 `∃N∀fuel` form is therefore the SEMANTIC SPECIFICATION of every obligation (it is what
soundness is argued against — §4, both pins), but the EXPORTED ARTIFACT for tiers (a)/(b) is the
fuel-free shallow equivalent (proven equivalent by fuel-irrelevance), and only tier (c) ships the
`∃N∀fuel` form to an interactive prover. This is the reconciliation the #216 finding demanded between
the deep-embedded obligation form and the shallow QF_LIA z3-demotion grounding.

This slots into the SHIPPED `degrade::run_ladder` as additional rungs BEFORE L2: the ladder already
takes closures for L2/L1 attempts; the engine rungs are the same shape (an `attempt_engine` closure
per engine in order). The SKIP/Unknown accounting per engine is reported in the cert (which engines
attempted, which returned Unknown and why) — generalizing the SHIPPED `SolverProfile` + the
"untested against engine X" honesty of REQ-9. **SHIPPED (Verus-only hook) — increment (i) added the ordering hook
(`engine::default_engines`, Verus-only, so byte-identical modulo REQ-3.1); (ii) adds the Lean-auto rung (tiers (a)/(b)); (iii)
adds the interactive tier (c).**

### 7. The anti-Goodhart battery, engine-generic (REQ-9, the honest v1)

The §7 mutation battery is ENGINE-GENERIC: a Lean-proven contract still faces mutants. First, the
SHIPPED semantics, stated ACCURATELY (F2): `mutation_score` calls `mutation::generate(f, seed)`, then
per mutant lowers + content-addresses + `run_verus`es through the #8 cache, and step 3's kill rule is
"a `Proved` mutant SURVIVED; a `Counterexample` / `Timeout` mutant is KILLED"
(`mutant_outcome_is_survivor = matches!(Proved)`; `mutation.rs` REQ-4 "Killed (counterexample /
timeout)"). So a TIMEOUT-killed mutant counts as killed TODAY — kills are NOT Refuted-only. **And a
surviving (`Proved`) mutant is then run through `equivalence_proves_equal` (#101, a §0.1 meta-query):
a mutant PROVEN semantically equal to the real body is EXCLUDED from BOTH the survivor set AND `scored`
(`if proved_equivalent { equivalent += 1; continue; }`).** So the SHIPPED accounting is:
`scored` (the denominator) = attempted MINUS proven-equivalent; survivor set = `Proved`-after-attempt
MINUS proven-equivalent; `kill_ratio = killed / scored`.

The engine-generic v1 (DECISION, F2) — preserving that #101 exclusion exactly:

- **Engine-generic kill = `Refuted ∪ Unknown-after-attempt`.** A mutant is "killed" if SOME engine in
  whose fragment the mutant falls (i) `Refuted`s it (a witnessed countermodel — the mutated body
  violates the contract) OR (ii) returns `Unknown` after ATTEMPTING it (the mutant was attempted and
  NOT proven). This maps EXACTLY onto today's `Counterexample ∪ Timeout` = killed (a Verus `Timeout`
  / fast-`unknown` becomes `Unknown-after-attempt`, a Verus witnessed `Counterexample` becomes
  `Refuted`), so the shipped behavior is PRESERVED — this is a faithful generalization, NOT the
  Refuted-only narrowing an earlier draft mis-stated.
- **The survivor set = (`Proved`-after-attempt) MINUS the proven-equivalent (#101 exclusion).** A
  proven mutant means the mutation did not break the contract — a SURVIVOR — UNLESS it is then proven
  semantically EQUAL to the real body, in which case it is an equivalent mutant and is dropped from
  BOTH the survivor set AND the denominator (the SHIPPED `equivalence_proves_equal` step). Only a
  genuinely-DISTINGUISHING `Proved` mutant is a survivor → the strengthening prompt. The equivalence
  probe is one of the §0.1 meta-queries; consistent with the F3 scoping, it stays OUTSIDE the Engine
  interface in v1 (a direct verus invocation, OQ-5) — so in v1 the equivalence-exclusion step runs as
  the SHIPPED Verus query regardless of which engine discharged the mutant's certification obligation.
- **"Untested against engine X" = NEVER ATTEMPTED.** A mutant whose obligation NO engine's fragment
  ADMITS (e.g. outside Lean-auto's scalar fragment AND un-lowerable for Verus, OR a recursive-registry
  obligation that only the §6 tier-(c) interactive path admits) is "untested" — it is
  NOT counted as killed (which would inflate the ratio, violating §7 + R-DEFER-9) and NOT counted as
  a survivor (no engine ever tried it). This is distinct from `Unknown-after-attempt` (which IS a
  kill): untested = no fragment admits it; unknown = a fragment admitted it and the engine could not
  decide.

**The floor (DECISION, F2).** The denominator = ATTEMPTED mutants MINUS the proven-equivalent — i.e.
exactly the SHIPPED `scored` (attempted MINUS proven-equivalent), generalized: every generated mutant
that SOME engine's fragment admits is attempted (the OQ-5 rule already DROPS un-lowerable mutants), and
of those, the proven-equivalent are removed from BOTH the numerator-eligible survivor set and the
denominator (the #101 exclusion). The `MUTATION_FLOOR` (default 0.60) gate via the SHIPPED
`meets_floor` is UNCHANGED on that ratio. Stating the v1 rule precisely so a literal implementation
does NOT regress #101: survivor = (Proved-after-attempt) MINUS proven-equivalent; denominator =
attempted MINUS proven-equivalent; `kill_ratio = killed / denominator`. A naive "denominator =
attempted, survivor = Proved" implementation would re-admit equivalent mutants as survivors → spurious
`WeakContract` floor failures; the #101 exclusion is therefore NORMATIVE here. Two ADDED guards close
the shrunken-denominator hole the critic named:

1. **Minimum-attempted reporting + qualifier.** If `attempted < generated` (some mutants were never
   attempted by any engine), the certificate REPORTS the untested count PER ENGINE, AND the
   kill-ratio line carries a qualifier (e.g. "1.00 over 1 attempted; N untested" — so a `1/1` ratio
   with N untested mutants can never read as a clean `1.00` without the untested count beside it). An
   auditor sees the shrunken denominator; the ratio cannot silently launder coverage gaps. (The
   proven-equivalent drop is the SHIPPED behavior and is reported separately as `equivalent`, distinct
   from `untested` — an equivalent mutant WAS attempted and proven; an untested one was never tried.)
2. **The 0-attempted backstop.** The shipped `scored == 0 → below-floor` backstop is KEPT: if NO
   engine attempted ANY mutant, OR every attempted mutant proved equivalent (so `scored == 0`), the
   item is below floor (the shipped `0/0` floor backstop) — an item cannot certify on an all-untested
   or all-equivalent mutation set.

**NOT-STARTED — increment (iii).**

### 8. v1 scope — what is IN and OUT (AC-6)

The exportable/dischargeable fragment for the Lean engine = what the spine's `S` covers TODAY (epic
#169 complete for the frozen subset):

**IN.** Contracts (all 8 frozen combinator classes + the `S_C` `Expr` subset), exec expressions
(`S_E`, bounded value / overflow-as-`none`), straight-line bodies (`S_B`), the v1 `while` form
(`loopDenote` + partial-correctness `while_rule`), spec-fns via the fuel-indexed registry — under the
§4 STABILIZED form (the obligation stated against `stabilizes`/`stabilizesProp` — the per-env ∃-N
stabilization relation, NOT a raw fuel index, #213 — with the RESULT value bound THROUGH `stabilizes
body_expr env r`, #214, with `specs := R_item` HELD FIXED, the export-time hard gate on registry
population, AND the REGISTRY-TERMINATION class on `R_item`'s spec-fns, #215), scoped to the
PURE-CONTRACT class (§4.1: the exec-body bridge is increment (iv) design work) and EXP registry-body
faithfulness. For
AUTO discharge specifically, the IN set NARROWS to the z3-demotion-reachable scalar/QF-linear core
exported FUEL-FREE (§6 tiers (a)/(b): specCall-free goals via fuel-irrelevance, or non-recursive
registries via static unfolding, #216); the recursive-registry `∃N∀fuel` obligations (§6 tier (c))
need INTERACTIVE proofs (or stay on Verus).

**OUT.** User ADTs in match-position (the Lean `Variant` has only the 4 built-in Option/Result
variants — `rust-lean-correspondence.md` D6/the user-variant residual); post-v1 loops (`loop`/
`break`/`continue`/multi-exit early return/nested loops/non-scalar mutation `xs[i]=e` — all honest
`Unsupported` in the encoders + absent from the Lean inductives); open body holes (`?N` —
short-circuited L0 before any engine); slag bodies (fiat-trusted, no engine); boundary items
(foreign body, L1, no engine); divergent spec-fns (a non-well-founded `dec` — REJECTED by the
REGISTRY-TERMINATION class before any contract obligation is reached, §1.2). These remain OUT exactly
as they are OUT of `S` today. The three §0.1
meta/battery query classes (vacuity / equivalence / strengthen) are OUT of the Engine interface in v1
(OQ-5).

### 9. The increment plan + the build blockers (AC-8)

- **(i) The Obligation reification + the Engine trait in forge** — Verus refactored behind the
  interface, behavior BYTE-IDENTICAL. The named REQ-3.1 fast-unknown remap is shipped as a NARROW
  signature (span-less + `unknown`-substring) that matches no grounded verus output today and is
  therefore INERT — the behavioral delta is undelivered until Z3's `:reason-unknown` is surfaced
  (`solver-profiles.md` OQ-1). The conformance cert oracle `conformance/*.cert.json` is unperturbed
  with NO exception (the AC for this increment).
  The REGISTRY-TERMINATION class is ASSIGNED here (REQ-1 reification; Verus's dec-check is its common
  discharge). No new engine. **FILED: blocker #204.**
- **(ii) The Lean exporter + auto-discharge for the PURE-CONTRACT class** — the cheapest real win
  (the z3-demotion scalar/linear fragment, kernel-clean, exported FUEL-FREE per §6 tiers (a)/(b)),
  behind the Lean-auto rung; the §4 STABILIZED form (#213) + the result-binding form (#214)
  + EXP registry faithfulness are built here, AND the SPINE PREREQUISITES this increment lands: the
  `stabilizes` relation + the `stabilization_exists_for_dec_bounded` lemma + uniqueness-of-stabilization
  (#214) + the FUEL-IRRELEVANCE lemma (#216) + the Lean-path REGISTRY-TERMINATION well-foundedness
  discharge (#215, REQ-1.2(b)) (the §4 build-blocker note). **FUTURE.**
- **(iii) Interactive proofs + certificate attribution + the engine-generic battery + the
  disagreement alarm** — the per-obligation `{engine, trust_profile}` attribution, the
  Proven⊕Refuted halt, the honest mutation v1 (the `Refuted ∪ Unknown-after-attempt` kill + the
  floor guards), AND the §6 tier-(c) interactive `∃N∀fuel` path for recursive registries. **FUTURE.**
- **(iv) The full exportable fragment** — exec exprs, straight-line bodies, v1 while, spec-fns via
  the fuel registry, under the Lean engine. **FUTURE.**

## Verification

Per increment (this doc's own ACs are statement-completeness, discharged by review):
- **(i), #204:** `cargo test -p forge` green AND the conformance cert oracle UNCHANGED — every
  `conformance/<name>.cert.json` byte-stable after Verus moves behind the `Engine` trait, with NO
  exception: the REQ-3.1 fast-unknown remap is shipped as a NARROW signature (span-less +
  `unknown`-substring) that matches no grounded verus output today, so it is INERT and no cert
  changes (the behavioral delta is undelivered until Z3's `:reason-unknown` is surfaced — the
  `solver-profiles.md` OQ-1 activation condition). A unit test asserts the discriminator is narrow
  (a witnessed countermodel AND an E0308 frontend rejection both stay `Refuted` → hard-fail). Plus
  `cargo clippy`/`fmt`.
- **(ii):** the Lean-auto rung discharges the scalar-contract corpus obligations with `#print axioms`
  = `{propext, Classical.choice, Quot.sound}` only (the z3-demotion kernel-clean bar), and a
  Lean-proven cert carries the smaller trust profile. **The AUTO obligations are emitted FUEL-FREE**
  (§6 tiers (a)/(b): specCall-free goals reduced via `intVal_fuel_irrelevant` to `denote 0`-shape, or
  non-recursive registries statically unfolded) — a test asserts the exported AUTO goal is shallow/QF
  (the z3-demotion `tv_obligation_*` shape), NOT a raw `∃N∀fuel` goal; the `∃N∀fuel` form is exported
  ONLY for recursive registries on the interactive path. A spec-fn-calling contract obligation with an
  under-fuelled body OR an omitted registry entry FAILS a vacuity-tripwire test (the
  obligation must NOT be provable by the below-`N` Int-`0`/Prop-`True` bottom — §4; the regression
  oracles are `lean/Thermite/PinIntBottom.lean` (`obligation_form_is_false`, the #213 form),
  `lean/Thermite/PinStabilization.lean` Pin A (`wrong_contract_certifies_with_underfuelled_rbody` must
  NOT be reachable once the result is bound through stabilization, the #214 form), AND
  `lean/Thermite/PinBodyRegistry.lean` (the #224 gate oracle:
  `wrong_contract_certifies_under_body_omission` must NOT be EXPORTABLE — the
  `req ∪ ens ∪ body ∪ dec(item)`
  transitive `calledSpecFns` puts the body-only spec-fn in `R_item`, so the gate refuses the
  omitted-registry form; `wrong_contract_fails_with_full_registry` shows the complete-registry
  obligation correctly REFUSES the wrong contract), AND `lean/Thermite/PinDecMeasure.lean` (the #226
  measure-position oracle: a `dec`-position spec-call must put its callee in `calledSpecFns(item)` so
  the descent obligation denotes the measure against the COMPLETE `R_item` —
  `closure_measure_strictly_descends` is the fake-descent a `t`-omitting `R_item` produces and must
  NOT be EXPORTABLE; `true_measure_never_descends` is the real source measure the extended closure
  exposes), which the new forms
  pass). **A DIVERGENT spec-fn registry FAILS the REGISTRY-TERMINATION class** (#215/#226, REQ-1.2): a
  test
  asserts `f(x)=f(x)` is REJECTED before any contract obligation certifies AND that a spec-fn called
  ONLY from a `dec` measure is in `R_item` so its measure is validity-checked against the real
  registry (the regression oracles are
  `lean/Thermite/PinStabilization.lean` Pin B (`divergent_contract_certifies`) — that bottom-poisoned
  discharge must NOT reach a certificate, blocked by the conjunction rule — AND
  `lean/Thermite/PinDecMeasure.lean` Pin C (`closure_measure_strictly_descends` vs
  `true_measure_never_descends`) — the measure-position fake descent must NOT be exportable once the
  closure ranges over the dec clauses).
- **(iii):** an injected Proven⊕Refuted disagreement HALTS (a test asserting the alarm fires, not a
  favorable pick); a Verus fast-unknown + Lean Proven does NOT halt (REQ-3.1 guard); a mutant outside
  every engine's fragment is reported "untested," never counted as killed, and an item with
  `attempted < generated` carries the untested-count qualifier; the attribution field round-trips the
  frozen golden; a recursive-registry obligation routed to the §6 tier-(c) interactive path replays
  its proof artifact (staleness on the evidence-key change).
- **(iv):** the Lean engine's fragment-coverage tests over the full frozen subset, with the OUT set
  honestly Skipped.

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (the Obligation artifact) | SHIPPED (increment (i), #204) | The prover-NEUTRAL artifact is built: `pub struct Obligation { item, class, role, ast_slice, env }` + `pub enum ObligationClass` (the FULL AC-1 union — CONTRACT/EXEC/BODY/LOOP-{entry,preservation,exit}/OVERFLOW/TERMINATION/REGISTRY-TERMINATION) + `pub enum ObligationRole` (CERTIFICATION; the §0.1 meta queries are NOT minted, OQ-5 seam) + `pub struct ObligationEnv`/`ObligationParam` carrying AST nodes + Thermite `Type`s + coercion flags (NO Verus strings), in `forge/src/obligation.rs`. Non-test consumer: `check::mint_item_obligations` mints the per-item set on the live L3 path; `engine::VerusEngine` consumes `&Obligation`. The artifact is a prover-neutral `Clone + Eq` VALUE (the `thermite-syntax` AST does not derive serde in production — adding it is outside the #204 manifest; wire serialization is increment (ii) when the Lean exporter serializes a Lean theorem string, not the raw AST). **REQ-1.2 (REGISTRY-TERMINATION) — SHIPPED (class assignment + the CORRECTED full-expression-position closure):** `ObligationClass::RegistryTermination` is minted (`Obligation::registry_termination`) for an item whose `check::reachable_spec_fn_names_full` (seed `req ∪ ens ∪ body ∪ dec(item)`, closure-step over each reached spec-fn's `body ∪ dec` — the #226 fix; `reachable_spec_fn_names_full_spec` is the spec-fn analogue) is non-empty; the forge-side closure mirror NOW walks the dec measures (the body-only omission of `reachable_spec_fn_deps` is corrected for the obligation closure). The Verus-path discharge is REQ-1.2(a) (Verus's dec-check on the woven sub-program); the Lean-path well-foundedness discharge + the per-item CONJUNCTION at the certificate level (REQ-1.1) + the exec-body bridge (§4.1) remain NOT-STARTED (increment (ii)/(iii)/(iv)). For history, the pre-#204 gap was: the content was SHIPPED only as transient Verus text: `pub fn equivalence_obligation` / `exec_equivalence_obligation` / `body_equivalence_obligation` / `loop_{entry,preservation,exit}_obligation` in `thermite-tv/src/obligation.rs` ("`thermite-tv` does NOT run verus itself: it emits the obligation TEXT") + `pub struct ObligationFrame` (the env/typing ctx). The prover-NEUTRAL artifact (AST slice + Thermite types + coercion flags + the `role` discriminator, pre-rendering) is unbuilt — that is the gap. The three §0.1 meta queries are scoped OUT (OQ-5). REQ-1.1 (the per-item CLASS-CONJUNCTION RULE — an item certifies only when EVERY class REQ-1 assigns it is discharged; the degrade ladder applies item-wide; #212(b)) + REQ-1.2 (the REGISTRY-TERMINATION class — for `calledSpecFns(item) ≠ ∅` (the #226 condition completing #224: the FULL-EXPRESSION-POSITION closure — seed `req ∪ ens ∪ body ∪ dec(item)`, step over each reached spec-fn's `body ∪ dec`, i.e. every expression the export denotes against `R_item` INCLUDING the termination measures, transitively — the SAME set the §4 hard gate populates `R_item` with), every spec-fn in `R_item` carries a dec-VALIDITY/well-foundedness obligation, conjoined item-wide; discharged by Verus's dec-check or a Lean well-foundedness proof; closes the #215 divergent-registry bottom-poisoning per Pin B AND the #226 measure-position bottom-poisoning per Pin C `lean/Thermite/PinDecMeasure.lean` — a `dec`-position spec-call omitted from `R_item` bottoms to `0` so a non-well-founded source measure denotes to a fake-descending one (`closure_measure_strictly_descends` vs `true_measure_never_descends`); the extended closure puts the measure-called spec-fn in `R_item` so the descent obligation denotes against the complete registry. The shipped forge closure `reachable_spec_fn_deps`/`collect_block_spec_fn_calls` (`forge/src/check.rs`) has the SAME body-only omission — recorded as a named increment-(i) work item (header build-blockers), load-bearing only for the NEW Lean exporter (the shipped Verus pipeline fails CLOSED on a missing dep)) + the exec-body bridge scoping (§4.1) are stated NORMATIVELY in §1.2/§4/§4.1 but likewise unbuilt — increment (iv) for the bridge, increment (i)/(iii) for the per-item conjunction at the certificate level, increment (ii) for the Lean-path registry-termination discharge. |
| REQ-2 (the Engine interface) | SHIPPED (Verus instance; increment (i), #204) | `pub trait Engine { name, fragment, discharge, trust_profile, evidence_key }` + `pub enum Verdict { Proven(Evidence) \| Refuted(Counterexample) \| Unknown(Reason) }` + `pub struct TrustProfile`/`Fragment`/`CacheKey` + `pub enum EngineName` in `forge/src/engine.rs`. `pub struct VerusEngine` fills all four slots (AC-2): FRAGMENT = the whole frozen subset (`admits_all_classes`, incl. RegistryTermination); DISCHARGE = `VerusEngine::verdict_of` lifting `classify_verus_outcome`'s three-way map to `Verdict` WITH the REQ-3.1 remap; TRUST PROFILE = {Z3, Verus VC-gen, TV/lowering theorem}; EVIDENCE = `engine_cache_key` composing the SHIPPED `cache::cache_key` hex with the engine discriminator (§2(d)). Non-test consumer: `check::ladder_for_timeout` routes the per-item L3 CERTIFICATION discharge through `VerusEngine` (selected via `default_engines`, gated via `fragment().admits`). The Lean engine (`LeanAuto`/`LeanInteractive`) is increment (ii), NOT-STARTED (forward-declared in the cache discriminator). |
| REQ-3 (Unknown degrades / Refuted hard-fails, engine-generic) | SHIPPED (increment (i), #204) | `pub fn engine::verdict_ladder_action` maps an engine `Verdict` (for `role = Certification`) to the SHIPPED `degrade::L3Verdict`: `Proven` → `Proved` (CertifyL3); `Unknown` → `Timeout` (degrade via `run_ladder` → L2/L1); `Refuted` → `Counterexample` (HardFail, never degrades — generalizing `degrade::ladder_action_l3` off the word "verus"). The failure-WITHOUT-witness rule is `engine::counterexample_is_incompleteness_unknown` (the NARROW SMT-`unknown` signature). **REQ-3.1 (the fast-unknown remap) — SHIPPED:** `VerusEngine::verdict_of` splits `VerusOutcome::Counterexample` — ONLY a span-less failure carrying the SMT-`unknown` signal (no frontend `error[E`) → `Unknown(IncompleteUnknown)` (degrade, the SOLE behavioral delta — was a hard fail); a WITNESSED countermodel AND a FRONTEND type error (E0308 — e.g. the provenance `careless_query` un-typeable IFC path) stay `Refuted` (hard-fail → L0, unchanged). The remap is INERT on the conformance corpus: it contains witnessed failures + E0308 type-error rejections (which stay hard-fail) but NOT genuine SMT-`unknown`s, so every `conformance/*.cert.json` is byte-identical. Tests: `engine.rs` (`incompleteness_discriminator_is_narrow`, `type_error_counterexample_stays_refuted`, `witnessed_counterexample_stays_refuted`, `verdict_ladder_action_follows_req3`) + the cert-oracle identity (`forge/tests/engine_interface.rs`, incl. provenance L0). The Lean-re-attempt interaction with a failed spec-fn termination proof (REQ-1.2 Lean discharge) is increment (ii). |
| REQ-4 (certificate attribution — per-obligation engine + trust profile) | NOT-STARTED | FUTURE (increment (iii)). The cert + honest-min are SHIPPED: `manifest::Certificate { level: Level, .. }`, `enum Level { L0, L1, L2, L3 }` (`#[derive(Ord)]`), `AssuranceManifest::aggregate → ProjectAssurance::Certified(min)` (VERUS-ANCHORED to `thermite_verified::aggregate_level`). NO per-obligation `{engine, trust_profile}` field exists; the additive-field precedent (`boundary`/`slag`/`lowered_assurance`/`assurance_scope`, all `#[serde(default)]`) is the schema model. The "smaller base" claim is along the named axes; the ordering formalization is OQ-3. |
| REQ-5 (engine disagreement = soundness alarm) | NOT-STARTED | FUTURE (increment (iii)). No second engine exists yet, so no disagreement path. The anti-cheat ANCESTOR is SHIPPED: a counterexample never degrades (`ladder_action_l3` → `HardFail`). The Proven⊕Refuted halt (vs benign Proven⊕Unknown), guarded against the REQ-3.1 fast-unknown spurious trigger, is unbuilt. |
| REQ-6 (the Lean exporter) | NOT-STARTED | FUTURE (increment (ii)/(iv)). The TARGET is SHIPPED: `lean/Thermite/` mechanizes `S` (`denote`/`refDenote`/`Denote.lean`, `execDenote`/`Exec.lean`, `bodyDenote`/`Exec/Stmt.lean`, `loopDenote`+`while_rule`/`Exec/Loop.lean`) over the `Expr`/`Block` inductives, kernel-checked (axioms `{propext, Classical.choice, Quot.sound}`). Critically (#213, the critic's kernel-checked pin `lean/Thermite/PinIntBottom.lean`): `intVal` bottoms an INT-position `specCall` to `0` (`| none => 0` + fuel-0 catch-all `| _, _, _ => 0`), NOT to `True` — so the cycle-2 `∀ fuel ≥ fuel₀` form is FALSE for correct items (the pin's `obligation_form_is_false`) and is RETIRED. §4 RESTATES the obligation against a STABILIZATION relation (`stabilizes : Expr → Env → Int → Prop := ∃ N, ∀ fuel ≥ N, intVal fuel e env = v`, + the Prop analogue for `denote`): `∀ r, stabilizes body_expr env r → reqStable(env) → ensStable(env at r)`, per-env ∃-N (no global `fuel₀`, fixing the value-dependent-depth counterexample), with the RESULT value BOUND THROUGH stabilization (the #214 fix — Pin A's `wrong_contract_certifies_with_underfuelled_rbody` is now UNPROVABLE because uniqueness of stabilization forces `r` to the body's true value, `wrong_contract_fails_at_true_value`), `specs := R_item` held fixed + the export-time HARD GATE (refuse-to-emit + per-name `decide` lemmas) when `calledSpecFns(item) ⊄ dom(R_item)`, where `calledSpecFns(item)` is (the #226 fix completing #224) the FULL-EXPRESSION-POSITION closure — every spec-fn reachable from `req ∪ ens ∪ body ∪ dec(item)` TRANSITIVELY, closure-step over each reached spec-fn's `body ∪ dec` (NOT `req ∪ ens` only — the cycle-2 scope; nor `req ∪ ens ∪ body` only — the cycle-5 body-only scope: a `dec`-VALIDITY obligation denotes the measure against `R_item`, and an omitted measure-called spec-fn bottoms to the Int-bottom `0` so a non-well-founded source measure denotes to a fake-descending one — the critic's pin `lean/Thermite/PinDecMeasure.lean`'s `closure_measure_strictly_descends` vs `true_measure_never_descends`; likewise an omitted body-called spec-fn stabilizes to `0`, uniqueness forces `r = 0`, and `ens: result == 0` certifies kernel-clean — `lean/Thermite/PinBodyRegistry.lean`'s `wrong_contract_certifies_under_body_omission`, REFUTED with the full registry by `wrong_contract_fails_with_full_registry`) — no resolution PREMISE. SCOPED to the PURE-CONTRACT class (§4.1: the exec-body S_C×S_E/S_B bridge — value bridge, bool sort, optres, env→State — is increment (iv)'s own design obligation). The SPINE PREREQUISITES (increment (ii), NOT yet built): `stabilizes` + `stabilization_exists_for_dec_bounded` + uniqueness-of-stabilization (#214) + the FUEL-IRRELEVANCE lemma (#216) + the REGISTRY-TERMINATION discharge (#215). The Rust→Lean exporter that emits source instantiating those (with EXP = arm-by-arm + drift-tripwire + registry-body faithfulness) is unbuilt; the z3-demotion doc names it "the #185-adjacent correspondence-bridge work … NOT built." |
| REQ-7 (Lean discharge modes + termination) | NOT-STARTED | FUTURE (increment (ii)/(iii)). The AUTO fragment is PROVEN-REACHABLE: `z3-demotion.md` shows `tv_obligation_arith_cmp`/`tv_obligation_or_le` (scalar/QF-linear contract clauses) discharged by Lean-SMT's `smt` tactic, kernel-clean (`#print axioms` = standard set only; no `sorryAx`/cvc5 oracle) — and these are SHALLOW QF goals with NO `denote`/`stabilizesProp` wrapper. §6.1 reconciles the deep-embedded §4 form to that grounding via the THREE-TIER export story (#216): (a) FUEL-FREE export for specCall-free obligations via the `intVal_fuel_irrelevant`/`denote_fuel_irrelevant` lemma (`stabilizesProp e env ↔ denote 0 e env` for specCall-free `e`) — the auto fragment's actual fuel-free shallow shape, matching the PoC; (b) STATIC UNFOLDING of non-recursive registries to finite depth, again yielding fuel-free goals; (c) the `∃N∀fuel` stabilization form reserved for RECURSIVE registries on the INTERACTIVE path only (the per-env `∃N` witness needs induction). The interactive/proof-artifact mode (staleness = the §2(d) EVIDENCE KEY changing: obligation + engine + engine-toolchain version + targeted-spine content hash) + the `dec`/partial-correctness termination policy (tied to the SHIPPED `while_rule` `h_run` premise) + the REGISTRY-TERMINATION class (#215, REQ-1.2) are unbuilt. |
| REQ-8 (engine ordering + ladder placement) | SHIPPED (Verus rung; increment (i), #204) | `pub fn engine::default_engines` returns the ordered engine list (Verus first); `check::ladder_for_timeout` reads the first rung (Verus) before the SHIPPED L2/L1 degrade. Increment (i) wires the ordering hook with the single Verus rung. The Lean-auto / Lean-interactive rungs, the `--engine lean` / `#[engine(lean)]` surface (OQ-1), and the per-engine SKIP/Unknown accounting are NOT-STARTED (increment (ii)). |
| REQ-9 (engine-generic anti-Goodhart battery, honest v1) | NOT-STARTED | FUTURE (increment (iii)). The battery is SHIPPED Verus-only: `forge::check::mutation_score` generates mutants + re-`run_verus`es each through the #8 cache; its kill rule is "a `Proved` mutant SURVIVED; a `Counterexample` / `Timeout` mutant is KILLED" (`mutant_outcome_is_survivor = matches!(Proved)`; `mutation.rs` REQ-4 "Killed (counterexample / timeout)"); and each SURVIVOR is run through the #101 `equivalence_proves_equal` query — a proven-equivalent survivor is excluded from BOTH the survivor set AND `scored` (`if proved_equivalent { equivalent += 1; continue; }`, "REQ-2/REQ-4: excluded from BOTH the survivor set AND `scored`"), so the SHIPPED `scored` = attempted MINUS proven-equivalent. OQ-5 already DROPS un-lowerable mutants from the denominator. The engine-generic kill (`Refuted ∪ Unknown-after-attempt`, = the shipped `Counterexample ∪ Timeout`), the "untested = never-attempted" rule (now including recursive-registry obligations that only the §6 tier-(c) interactive path admits), the #101-preserving floor (survivor/denominator both MINUS proven-equivalent; the equivalence probe a §0.1 meta-query outside the Engine interface, F3), and the floor guards (minimum-attempted qualifier + the 0/0 backstop) are unbuilt. |

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
  claim is a formal order or an auditor's informal read. §4 / REQ-4 deliberately claim ONLY
  "smaller along the named axes" pending this OQ.
- **OQ-4 (the interactive-proof review policy).** Where do checked-in Lean proof artifacts live, who
  reviews them, and what is the CI staleness/replay policy beyond "evidence-key change invalidates"?
  Does an interactive proof require second-party sign-off like a `#[slag]` block (§8 CI policy
  hooks)? Is an interactive Lean proof's trust profile DIFFERENT from an auto one (it adds the
  human/agent author as a reviewed-but-not-mechanized step)?
- **OQ-5 (bringing the §0.1 meta/battery queries engine-generic).** The three SHIPPED non-certification
  verus query classes — `solver_vacuity_check` (INVERTED polarity: Proven→reject), the #101
  `equivalence_proves_equal` survivor-equivalence query, and `strengthen::probe` — are scoped OUT of
  the Engine interface in v1 (direct verus invocations, §0.1). The future question: should they
  become `role`-discriminated Obligations (a `VACUITY-PROBE` role with an inverted certify rule, an
  `EQUIVALENCE` role, an `ADVISORY` role) so a second engine can also run the anti-Goodhart battery +
  vacuity triage with its smaller trust base? This is the bound on "engine-generic" in v1.
