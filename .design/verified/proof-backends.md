# Proof-Backend Interface + Lean as Engine #2 — backend-neutral obligations over the mechanized semantics

<!--
tier: 3-component
status: draft (v-next architecture — the obligation/engine interface; most REQs NOT-STARTED
audited-sha: cf4c5b94c807b73e788731d7ddfcaeee3cb00c44 (re-pinned 2026-06-12 — #268 needle exact-match hardening, REQ-7 anchor class: the interactive replay reconstruction's obligation-theorem needle (canonical_theorem_statement + reconstruct_replay, forge/src/engine.rs) is now an EXACT-name match via theorem_anchor_pos, never a prefix — a while-shaped multi-theorem file's thermite_obligation_<item>_entry/_converges siblings can no longer latch the bare contract theorem and drop the 5+2 conjunction; in-module pin added. Prior pin b1c7a995 re-pinned to the increment (v-b) build #264 — the while-body Lean exporter: REQ-11.4/11.5/11.7 moved SHIPPED, the §4.2.4 obligation set + the WhileBattery generator-fixed proofs land, count.th certifies L3-via-lean-auto and sum.th is the honest recursive-registry residual; the REQ-status table + the §4.2.4 sub-rows reflect the post-build state)
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
    `stabilization_exists` (the design's `stabilization_exists_for_dec_bounded`) — SHIPPED with
    GENUINE (non-identity) content (#241, the ROOT fix of Pin E's cycle-4 identity-hypothesis
    finding): `RegistryTerminating env e := ∃ v, Converges e env v` keyed on the BOTTOM-DISTINGUISHING
    none-propagating denotation `intValNB`/`denoteNB` (a fuel-0/unresolved `specCall` → `none`, every
    arm propagates `none`), with `Converges e env v := ∃N∀fuel≥N, intValNB fuel e env = some v` and
    the AGREEMENT LEMMA `converges_imp_stabilizes` (via the mutual `intValNB_agrees`/`denoteNB_agrees`/
    …) carrying genuine convergence to genuine `stabilizes`. The cycle-4 form `∃ v, stabilizes` was an
    IDENTITY HYPOTHESIS a divergent registry SATISFIED at the Int-bottom 0; the redefined hypothesis
    a divergent registry CANNOT satisfy (`intValNB` is `none` at every fuel — `PinRegistryTerminating
    .lean`'s `divergent_registry_fails_the_hypothesis`), while a genuine dec-valid registry still
    converges. The hypothesis is EXACTLY what the per-item REGISTRY-TERMINATION class REQ-1.2
    discharges (dec-validity ⟹ finite per-env unfolding ⟹ `intValNB` reaches `some`), so it is the
    named separately-discharged obligation, NOT an assumed-away premise. The §4 obligation form is
    stated against `stabilizes`, NOT a raw fuel index, and the RESULT value is bound THROUGH
    `stabilizes` (the #214 fix). ALSO SHIPPED in the
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
  - increment (iv): FILED — crosslink #253 (the exec-body bridge BUILD; the DESIGN is §4.1 of THIS
    amendment). Scope: extend the exporter past PURE-CONTRACT to STRAIGHT-LINE-BODY items — body a
    straight-line `S_B` `Block` (`let`/`assign`/bare-expr/`if`-statement/sequencing + tail, EXACTLY
    the subset `lean/Thermite/Exec/Stmt.lean` already mechanizes as `bodyDenote`/`stmtDenote`/
    `blockThread`; loops OUT). OWNS the EXEC-BODY BRIDGE (§4.1, the first S_C×S_E/S_B domain-tying
    artifact, the #212 fix), now DESIGNED: the `BVal.value` value bridge (§4.1.1); the bool-result
    binding via the `bindBool` SPINE PREREQUISITE — a DEFAULTED `Env.bools` field + `Env.bindBool`
    + the `Expr.boolVar` leaf and its arms across the mutual denotation family, landed FIRST and
    kernel-green like (ii)'s `Stabilize.lean` layer (§4.1.2); the env→State correspondence
    (`stateOf` + the typed-input `InRangeParams` premises + per-param `rfl` correspondence lemmas,
    §4.1.4); the HYPOTHESIZE obligation `bodyConverges body_block (stateOf v) r →
    ensStable(bindResult r)` with the OVERFLOW class EXPORTED ALONGSIDE per the conjunction rule
    (§4.1.5) — and NO new NB/none-propagating layer (`bodyDenote` is FUEL-FREE: `ExecExpr` has no
    `specCall`, so its `Option` already distinguishes a genuine failure from a value — the
    #241-class trap does not exist on the exec side); FOUR bridge divergence pins (§4.1.6); the
    loop-class STRUCTURED refusal (§4.1.7). Option/Result-typed RESULTS stay OUT of (iv) v1
    (blocker #254: `ExecVal` is `int (BVal) | bool (Bool)` — no Option/Result variant to bridge).
    v1 while + spec-fns-in-exec remain the future (iv) residuals after #253. The iv-b audit's
    TWO auto-battery COVERAGE residuals (a `req`-bounded `_overflow` conjunct; a nested-`ifElse`
    body's `restoreScope` denotation) DEGRADE to Unknown (fail-to-certify, SOUND) — recorded at
    REQ-10's Known-limitations note, NOT blockers.
    SPINE PREREQUISITE — SHIPPED (#253 part iv-a, ref #203), kernel-green with the standard axiom
    set `{propext, Classical.choice, Quot.sound}` (NO `sorryAx`), every existing theorem + all
    SHIPPED pins still green (exactly the (ii)-`Stabilize.lean` precedent): the `bindBool` layer
    (the DEFAULTED `Env.bools` field + `Env.bindBool` (`Denote.lean`) + the `Expr.boolVar` leaf
    (`Ast.lean`) and its arms across `denote`/`refDenote`/`intVal`/`seqVal`/`ref_sound`/`refVal_eq`
    (`Soundness.lean`)/`specCallFree`/`*_fuel_irrelevant`/`denoteNB`/the agreement lemmas
    (`Stabilize.lean`)); `bodyConverges` + the `bindResult` value bridge (`Exec/Stmt.lean`, an
    abbrev over the FUEL-FREE `bodyDenote` — NO NB layer, verified against
    `body_overflow_rhs_has_no_result`); and the FOUR bridge pins (`PinExecValueBridge`/
    `PinExecBoolBind`/`PinExecOverflowVacuity`/`PinExecStateMisMap`). The EXPORTER (`forge`:
    `stateOf`/`InRangeParams`/the HYPOTHESIZE-theorem emission + the `ExportRefusal` narrowing) is
    part (iv-b), SHIPPED (#253) — `forge/src/lean_export.rs::export_straight_line_body` +
    `emit_state_of` + `emit_body_theorems` + the exec-body encoder `encode_exec_block`/
    `encode_exec_stmt`/`encode_exec_expr`, with the `LoopBody`/`OptResResult` refusals; LIVE
    lake tests in `forge/src/engine.rs`. See REQ-10's SPINE-PREREQUISITE sub-rows.
  - increment (v): FILED — crosslink #264 (the WHILE-BODY widening BUILD; the DESIGN is §4.2 of
    THIS amendment). Scope: widen the exporter past STRAIGHT-LINE bodies (iv) to the v1 WHILE
    shape — a straight-line prefix + a single `while <cond>` with non-empty `invs` + `dec` and a
    straight-line SCALAR loop body, as the LAST statement before a REQUIRED tail, EXACTLY the set
    `thermite-tv`'s `recognize_v1_loop` already admits for loop-TV; everything else (the
    `loop`-kind, `break`/`continue`, mid-body `return`, nested loops, a loop in non-last/inside-if
    position, non-scalar mutation, spec-calling invariants, tail-less bodies) stays the STRUCTURED
    `LoopBody`-class refusal. OWNS the first S_B×S_Loop COMPOSITION (§4.2): the composed
    denotation `whileBodyDenote` (prefix `blockThread` → the SHIPPED `loopDenote` → tail
    `execDenote`) + the ∃-fuel `whileBodyConverges` (the result bound THROUGH the relation — the
    #214 discipline; uniqueness via `loopDenote_fuel_mono`/`whileBodyConverges_unique`), the
    loop-exit-to-ens composition lemma `while_compose` (the SHIPPED partial-correctness
    `while_rule` lifted through the prefix/tail segments), and the TERMINATION bridge
    `loopDenote_exits_of_dec` (dec-validity + progress ⟹ the exit witness — the REQ-1.2 mirror).
    §4.2.3 DECIDES termination honesty: a conjoined `_converges` theorem JOINTLY discharges the
    OVERFLOW + TERMINATION classes on the Lean path; NO partial-correctness-marked L3 is minted,
    NO un-run Verus dec-check is credited; the spine's `while_rule` itself stays UNCHANGED
    (partial correctness — loop-tv.md REQ-4 stands). SEQUENCED like #253: (v-a) the spine layer +
    the two pins (`PinWhileVacuity`/`PinWhileComposition`) lands FIRST, kernel-green with the
    standard axiom set, every existing theorem + pin still green; (v-b) the exporter (the
    recognizer mirroring `recognize_v1_loop`, the `Inv_item`/`mu_item` emission, FIVE per-item
    obligations + TWO generator-proved composed theorems, the loop auto battery) lands SECOND.
    BOTH (v-a) and (v-b) SHIPPED: the L1 linear family certifies L3-via-lean-auto end-to-end
    (`forge/tests/lean_while.rs`); the recursive-registry corpus `sum` is the honest §4
    interactive residual (NOT-L3-via-lean). See REQ-11.
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
  **The floor GATES the Lean path, it does NOT merely report it (the #248 fix, R-DEFER-9).** On the
  Lean-only path (`lean_proven_cert`) the kill-ratio over the `attempted` denominator MUST MEET the
  mutation floor for the item to certify L3-via-Lean; a below-floor ratio — OR a zero-attempted tally
  WITH mutants generated (every mutant untested-against-lean — the SHIPPED `0/0` backstop) — does NOT
  certify, it is a `WeakContract`-style reject (mirroring the Verus path's `mutation_score → meets_floor`
  gate, NOT a silent L3). DENOMINATOR HONESTY: the #101 `equivalence_proves_equal` exclusion is a §0.1
  verus meta-query OUTSIDE the Engine interface in v1 (F3/OQ-5) and is NOT threaded on the Lean-only
  path, so the Lean-path denominator = `attempted` with NO equivalence exclusion available (the gate
  SAYS SO in the reject detail + the `qualifier`); a `spec fn` (no `ens`) has no mutation obligation and
  certifies on the kernel proof alone (nothing to mutate, no gate). Derived from §7 + R-DEFER-9.
  **Increment (iii), SHIPPED (the Lean-path battery + the floor GATE, #247/#248).**
- **REQ-10 (the exec-body bridge — STRAIGHT-LINE-BODY items; increment (iv))** — extend the Lean
  exporter past the PURE-CONTRACT class (§4 SCOPE) to items whose body is a straight-line `S_B`
  block — `let`/`assign`/bare-expr/`if`-statement/sequencing + a tail exec expression, EXACTLY the
  subset the spine mechanizes as `Thermite.Exec.bodyDenote` (`lean/Thermite/Exec/Stmt.lean`); loops
  stay OUT. The bridge per §4.1: **REQ-10.1** the `S_E→S_C` VALUE bridge — an int-sorted result
  `r : BVal` binds `Env.bindInt env "result" r.value` (`BVal.value` is the mathematical unsigned
  value; the bridge is the IDENTITY on it — no `toNat` clamp, no re-wrap, no signed
  reinterpretation); **REQ-10.2** the `bindBool` SPINE PREREQUISITE (a DEFAULTED `Env.bools` field
  + `Env.bindBool` + the `Expr.boolVar` leaf with its `denote`/`denoteNB`/`refDenote`/
  `specCallFree`/fuel-irrelevance/agreement arms — the build lands it FIRST, kernel-green, the
  (ii)-`Stabilize.lean` precedent); **REQ-10.3** the env→State correspondence (the
  generator-emitted `stateOf : Env → State` + the typed-input `InRangeParams` premises + per-param
  `rfl` correspondence lemmas — the §4 mechanism-2 parallel); **REQ-10.4** the HYPOTHESIZE
  obligation form (`bodyConverges body_block (stateOf v) r → ensStable(bindResult r)`, sound per
  the §4.1 conjunction rule with the OVERFLOW class EXPORTED alongside as `(bodyDenote body_block
  (stateOf v)).isSome`) — with NO new NB/none-propagating layer (`bodyDenote` is fuel-free,
  `ExecExpr` has no `specCall`, its `Option` already distinguishes failure from value — §4.1.5);
  **REQ-10.5** the four bridge divergence pins (§4.1.6); **REQ-10.6** the loop-class STRUCTURED
  refusal narrowed out of `ExportRefusal::NotPureContract` (§4.1.7). Option/Result-typed RESULTS
  are OUT of (iv) v1 (blocker #254 — `ExecVal` has no Option/Result variant). Derived from §6 (the
  ladder's Lean rung over the full fragment) + `thermite-semantics.md` REQ-4 (`S_B`) + §4.1.
  **Increment (iv), blocker #253.**
  - **SPINE PREREQUISITE — SHIPPED (#253 part iv-a, ref #203).** The Lean spine additions
    REQ-10 names — landed FIRST, kernel-green with the standard axiom set
    `{propext, Classical.choice, Quot.sound}` (NO `sorryAx`/new axiom), every existing theorem
    (`ref_sound`/`ref_sound_eq`, `Exec.exec_ref_sound`, `Exec.body_ref_sound`, `lowering_faithful`,
    the `Stabilize.lean` family) and all SHIPPED kernel pins still green:
    - **REQ-10.2 SHIPPED** — the `bindBool` SPINE PREREQUISITE: the DEFAULTED `Env.bools : String →
      Bool := fun _ => false` field + `Env.bindBool` (`lean/Thermite/Denote.lean`) and the NEW
      `Expr.boolVar (name : String)` leaf (`lean/Thermite/Ast.lean`) with its arms across the WHOLE
      mutual denotation family: `denote`/`refDenote` (`env.bools x = true`, T1 re-proves by
      `Iff.rfl`), `intVal`/`seqVal` (the bool-sorted `0`/`[]` catch-alls — no new arm), the
      `ref_sound`/`refVal_eq` cases (`lean/Thermite/Soundness.lean`), and `specCallFree` /
      `intVal_fuel_irrelevant` / `seqVal_fuel_irrelevant` / `denote_fuel_irrelevant` / the
      bottom-distinguishing `denoteNB` (EXPLICIT `some (env.bools x = true)`, NOT the `some True`
      catch-all) + the `*_agrees` / `*_specCallFree` arms (`lean/Thermite/Stabilize.lean`). The
      DEFAULTED field is the minimality lever — every existing `Env` literal (Soundness teeth, the
      pins) elaborated UNCHANGED.
    - **REQ-10.1/REQ-10.4 SHIPPED (spine side)** — `bodyConverges (b : Block) (st : State) (r :
      ExecVal) : Prop := bodyDenote b st = some r` (the abbrev over the FUEL-FREE,
      Option-bottom-distinguishing `bodyDenote`; NO NB layer — verified against
      `body_overflow_rhs_has_no_result` (= `none`) vs `body_in_range_rhs_has_result` (= `some …`),
      `Exec/Stmt.lean`) + the value bridge `bindResult (env : Env) (r : ExecVal) : Env` (int →
      `Env.bindInt env "result" r.value`, the identity on `BVal.value`; bool → `Env.bindBool env
      "result" b`) — `lean/Thermite/Exec/Stmt.lean`.
    - **REQ-10.5 SHIPPED** — the FOUR bridge-divergence pins, each kernel-checked with the standard
      axiom set: `PinExecValueBridge.lean` (the signed/truncating value mis-bridge certifies a wrong
      `result < 0` at the `u64` rim; the faithful `bindResult` refutes), `PinExecBoolBind.lean` (a
      dropped bool bind certifies a negated `!result` against a `.bool true` body; faithful
      `bindBool` refutes — extends Pin H), `PinExecOverflowVacuity.lean` (an always-overflow body's
      vacuous CONTRACT obligation BOTH holds AND has its conjoined OVERFLOW class refuted — the
      certificate-level conjunction oracle), `PinExecStateMisMap.lean` (a `stateOf` dropping the
      `seqs → slices` map makes a RIGHT `xs[0]` body fail to converge; the per-param correspondence
      `rfl`-lemma agrees for the faithful map, fails `[] ≠ [7]` for the dropped — the §4.1.4
      compile-time tripwire).
    - **REQ-10.1/REQ-10.3/REQ-10.4/REQ-10.6 SHIPPED (exporter side, #253 part iv-b)** — the
      EXPORTER-side (`forge/src/lean_export.rs`) emission landed: `export_item` routes a
      STRAIGHT-LINE-BODY `Item::Fn` (a body WITH statements, or a `bool` result) to
      `export_straight_line_body`, which (a) serializes the body into the spine's
      `Exec/Stmt.lean` `Block`/`Stmt` encodings (`encode_exec_block`/`encode_exec_stmt`/
      `encode_exec_expr` — the same arm-by-arm encoder discipline as the contract `Expr`
      encoder); (b) emits the generator's `stateOf : Thermite.Env → Thermite.Exec.State`
      (`emit_state_of`: int param → `.int ⟨uW, v.ints x⟩`, bool param → `.bool (v.bools p)`,
      slice param → `(v.seqs xs).map (⟨uW, ·⟩)`; `scope := fun _ => false`) + the
      `InRangeParams` typed-input premise + the per-param correspondence `rfl`-lemmas (the
      §4.1.4 compile-time tripwire); (c) emits BOTH the HYPOTHESIZE CONTRACT theorem (the
      result bound THROUGH `bodyConverges`) AND the conjoined OVERFLOW theorem
      (`(bodyDenote …).isSome`) in ONE file (`emit_body_theorems`, the §4.1.5 conjunction
      rule); the result binds via `bindResult` (int → `bindInt … r.value`, bool →
      `bindBool … b`). The loop class is the `ExportRefusal::LoopBody` structured refusal
      (REQ-10.6, §4.1.7); an Option/Result result is `ExportRefusal::OptResResult` (#254).
      Verified LIVE (`lake env lean`, `forge/src/engine.rs` tests): a straight-line int body
      → Proven incl. the OVERFLOW conjunct (`live_straight_line_body_is_proven`); a bool body
      → Proven via `bindBool` (`live_bool_result_body_is_proven_via_bindbool`); an
      always-overflow body's vacuous ens → NOT Proven, the OVERFLOW conjunct fails
      (`live_always_overflow_body_is_not_proven`, the `PinExecOverflowVacuity` Rust mirror);
      a while body → refused (`while_body_item_refuses_export`); an optres result → refused
      (`optres_result_item_refuses_export`). **THE `scope := false` EXP ROW (VERIFIED):**
      faithful against `thermite-tv::exec_stmt_encode::body_ref_state` — its initial `Env`
      is `Env::new()` (EMPTY), so a param is a free INPUT (left verbatim by `encode_value`),
      NOT an assignable cell; a body `assign` to a param `Err`s (`!env.contains_key`) on the
      Rust side and is `none` (the `Stmt.assign` unbound-target guard) on the spine side —
      exactly the spine's `inputState` `scope := fun _ => false`. No mis-map; the
      `PinExecStateMisMap.lean` divergence oracle stands.
    - **Known limitations (increment (iv) auto-discharge) — coverage residuals, NOT blockers
      (the iv-b audit).** The exec-body EXPORTABLE surface is WIDER than the auto-DISCHARGEABLE
      surface: the shipped auto tactic batteries (`exec_body_tactic_battery` /
      `exec_overflow_tactic_battery`, `forge/src/lean_export.rs`) do NOT yet close (a) a
      bounded-overflow `_overflow` conjunct that needs the `req` premise to bound the
      arithmetic (e.g. `req a <= 100`, body `a + a` — in `u64` range ONLY because of `req`;
      the OVERFLOW battery's unfold set does not reduce the `hreq` denotation, so its `omega`
      never sees the bound), nor (b) a nested-`ifElse` body's `restoreScope` denotation (the
      nested branch needs a case split the battery lacks). BOTH currently DEGRADE to
      `Verdict::Unknown` (fail-to-certify), which is SOUND — never a false `Proven`:
      conservative INCOMPLETENESS, not unsoundness (the REQ-3 discipline — a tactic failure is
      `Unknown`, NEVER `Refuted`; and an undischarged OVERFLOW conjunct never certifies, per
      the §4.1.5 conjunction rule). The future close is a STRONGER auto battery (thread the
      `req` denotation through the OVERFLOW unfold; a `split` step for the nested branch) or
      the REQ-7 INTERACTIVE path — increment (v)'s loop battery (§4.2.4, blocker #264) MUST
      thread the `req` denotation through its unfold sets (the loop obligations consume `hreq`),
      making (v) the named vehicle for residual (a); residual (b) rides the same battery work.
      Recorded as a COVERAGE RESIDUAL of the SHIPPED REQ-10, NOT an open blocker.

- **REQ-11 (the while-body widening — v1 `while` bodies; increment (v))** — extend the Lean
  exporter past the STRAIGHT-LINE-BODY class (REQ-10) to items whose body is the v1 WHILE shape: a
  straight-line prefix + a single `Stmt::Loop(LoopNode { kind: While(cond), invs: non-empty, dec,
  body })` with a straight-line SCALAR loop body as the LAST statement before a REQUIRED tail
  `ExecExpr` — EXACTLY the set `thermite-tv`'s `recognize_v1_loop` admits for loop-TV — composed
  onto the ALREADY-PROVEN spine loop brick (`lean/Thermite/Exec/Loop.lean`: `loopDenote` + the
  partial-correctness `while_rule` + `tv_meta_loop`, axioms `[propext, Quot.sound]`). Designed in
  §4.2. Sub-requirements: **REQ-11.1** the composed whole-body denotation `whileBodyDenote`
  (prefix `blockThread` → `loopDenote` → tail `execDenote`) + the ∃-fuel convergence relation
  `whileBodyConverges` with the result bound THROUGH it (no export-time fuel exists — the
  iteration count is env-dependent, the #213 lesson; the #214 binding discipline) +
  `loopDenote_fuel_mono`/`whileBodyConverges_unique` (the `stabilizes_unique` mirror) — SPINE
  PREREQUISITE, lands FIRST (the iv-a precedent); **REQ-11.2** the loop-exit-to-ens composition
  lemma `while_compose` (the SHIPPED `while_rule` lifted through the prefix/tail segments: any
  converged whole-body result is the tail's value at an exit state satisfying `I ∧ ¬cond`) — SPINE
  PREREQUISITE; **REQ-11.3** the TERMINATION bridge `loopDenote_exits_of_dec` (dec-validity —
  strict bounded-below descent of the denoted measure per genuine `blockThread` step — plus
  cond-totality/body-progress under the invariant ⟹ the loop EXITS at some fuel; the REQ-1.2
  `converges_imp_stabilizes` mirror) — SPINE PREREQUISITE; **REQ-11.4** the exporter: the (v)
  recognizer pinned arm-by-arm to `recognize_v1_loop` (EXP), the generator-emitted `Inv_item`
  (FOUR conjunct families — user invs denoted shallowly over cells; per-cell sort/width/range;
  param FRAME equalities to `stateOf v`; scope facts) + `mu_item` (the loop `dec` denoted over
  cells), FIVE per-item obligation theorems (`_entry`/`_pres`/`_exit`/`_progress`/`_dec` —
  realizing the LOOP-{ENTRY,PRESERVATION,EXIT} + OVERFLOW + TERMINATION classes) + TWO composed
  theorems with GENERATOR-FIXED proofs (the HYPOTHESIZE CONTRACT theorem via `while_compose`; the
  conjoined `_converges` theorem via `loopDenote_exits_of_dec`), and the loop auto battery
  (§4.2.4 — `hreq` threaded through the unfold sets); **REQ-11.5** the refusal inventory stays
  LOUD (§4.2.5: the `loop`-kind, nested loops, a while inside an `if` or not in last-statement
  position, `Stmt::Break`/`Stmt::Continue`, mid-body `Stmt::Return(_)`, non-scalar mutation, empty
  `invs`/weak `inv true`, a tail-less body, spec-calling/combinator invariants (a (v) residual),
  Option/Result results #254 — each a STRUCTURED refusal, never silent); **REQ-11.6** the two
  bridge divergence pins (`PinWhileVacuity` — the termination-vacuity conjunction oracle;
  `PinWhileComposition` — the skipped/mis-ordered composition); **REQ-11.7** the REQ-9 accounting
  delta (in-grammar while-body mutants become ATTEMPTED; `UntestedAgainstLean` shrinks to
  out-of-grammar mutants; the floor gate genuinely gates while items on the Lean path).
  TERMINATION-HONESTY DECISION (§4.2.3): the conjoined `_converges` obligation discharges the
  OVERFLOW + TERMINATION classes — NO partial-correctness-marked L3 is minted and NO un-run Verus
  dec-check is credited; the spine's `while_rule` stays partial-correctness (loop-tv.md REQ-4
  unchanged). Derived from §6 (the ladder's Lean rung over the full fragment) +
  `thermite-semantics.md` REQ-1 (`S_Loop` v1) + `loop-tv.md` REQ-2/REQ-3/REQ-4 + §4.2.
  **Increment (v), blocker #264.**
  - **SPINE PREREQUISITE — SHIPPED (#264 part v-a, ref #203).** The Lean spine additions
    REQ-11.1/REQ-11.2/REQ-11.3 name — the WHILE-BODY COMPOSITION LAYER — landed FIRST,
    kernel-green with the standard axiom set `{propext, Classical.choice, Quot.sound}` (NO
    `sorryAx`/new axiom), every existing theorem (`while_rule`/`tv_meta_loop`,
    `body_ref_sound`, the `Stabilize.lean`/exec-bridge families) and ALL SHIPPED kernel pins
    still green; `Thermite.lean` imports the new module. KERNEL-BAR MECHANICS (#265 critic
    observation): the default `lake build` (`defaultTargets = ["Thermite"]`) elaborates only the
    modules reachable from `Thermite.lean`, which imports `Exec/WhileBody.lean` but NOT the
    `PinWhile*`/`PinExec*`/`Pin*` modules — so a pin's kernel check is asserted by EXPLICIT
    per-module elaboration (`lake env lean Thermite/PinWhileDecShape.lean`, with its `#print
    axioms`), not by the default build; "all pins green" means each pin module so elaborated, not
    that `lake build` touched them. ALL in `lean/Thermite/Exec/WhileBody.lean`,
    composed AROUND `Exec/Stmt.lean` + `Exec/Loop.lean` (UNCHANGED — no re-proof of
    `body_ref_sound`/`while_rule`):
    - **REQ-11.1 SHIPPED** — `def whileBodyDenote (prefixB cond lbody tail fuel st)` (the
      `Option`-monad composition prefix `blockThread` → the SHIPPED `loopDenote` → tail
      `execDenote`; the design's `prefix` binder is `prefixB` — a Lean-keyword SYNTAX
      adaptation only, semantics the §4.2.2 sketch exactly) + `abbrev whileBodyConverges
      … : Prop := ∃ fuel, whileBodyDenote … = some r` (the ∃-fuel relation, result bound
      THROUGH it, the #214 discipline; NO NB layer — `none` is GENUINE fuel-exhaustion/failure)
      + `theorem loopDenote_fuel_mono` (surplus fuel after exit is unconsumed, by induction on
      fuel) + `theorem whileBodyConverges_unique` (overlap-at-max via fuel-mono + functional
      determinism — the `stabilizes_unique` mirror). Axioms `[propext, Quot.sound]`.
    - **REQ-11.2 SHIPPED** — `theorem while_compose (prefixB lbody cond tail I) (h_pres) : ∀
      st₀ fuel r, whileBodyDenote … = some r → (∀ st₁, blockThread prefixB st₀ = some st₁ →
      I st₁) → ∃ stf, I stf ∧ condBool cond stf = some false ∧ execDenote tail stf.env = some
      r` — the SHIPPED partial-correctness `while_rule` lifted through the prefix/tail segments
      (proof: unfold the composition; the prefix step is a function so the loop-entry state is
      determined; apply `while_rule` to the middle segment). Axioms `[propext, Quot.sound]`.
    - **REQ-11.3 SHIPPED** — `theorem loopDenote_exits_of_dec (cond lbody I μ) (h_pres)
      (h_cond_total) (h_progress) (h_dec) : ∀ st, I st → ∃ fuel stf, loopDenote cond lbody
      fuel st = some stf` — the TERMINATION bridge (dec-validity + progress ⟹ the exit
      witness `while_rule` HYPOTHESIZES, the REQ-1.2 `converges_imp_stabilizes` mirror). The
      STATEMENT SHAPE matches the §4.2.2 sketch with ONE DECLARED SEMANTIC ADAPTATION (#265,
      kernel-record `lean/Thermite/PinWhileDecShape.lean`, commit 92659eb7): the shipped `h_dec`
      bound is the PRE-state `0 ≤ μ st` (`h_dec : … → μ st' < μ st ∧ 0 ≤ μ st`), which is the
      strictly WEAKER hypothesis — so the shipped theorem is strictly MORE GENERAL than a
      post-state-bounded (`0 ≤ μ st'`) one (the two shapes are NON-equivalent — the pin's
      `shipped_hdec_is_not_the_pinned_shape`; the §4.2.2 sketch is now pinned to this PRE-state
      shape to match). The pin's `loopDenote_exits_of_dec_design_shape` kernel-derives the
      post-state-bounded statement as a one-line corollary, so the adaptation strengthens the
      theorem, never weakens it. Proof: strong induction on `(μ st).toNat` (`Nat.strongRecOn`).
      PROOF-DIFFICULTY NOTE
      (for the critic — the statement is NOT weakened): because `h_dec` bounds `0 ≤ μ st` (NOT
      `0 ≤ μ st'`), the proof needs a CASE SPLIT on `0 < μ st` — at `μ st = 0` a `cond`-true
      step gives `μ st' < 0`, where `(μ st').toNat = 0` does NOT strictly decrease, but the
      NEXT condition MUST be false (`h_dec` at `st'` would force `0 ≤ μ st'`, contradicting
      `μ st' < 0`), so the loop exits in two fuel directly. This is sound and uses the
      designed premises exactly; it is recorded so the (v-b) `_dec` obligation emitter need
      not re-derive it. Axioms `[propext, Quot.sound]`.
    - **REQ-11.6 SHIPPED** — the TWO bridge-divergence pins, each kernel-checked with the
      standard axiom set: `PinWhileVacuity.lean` (the termination-vacuity conjunction oracle —
      a `while true` no-op body's `whileBodyConverges` is FALSE at every fuel, so a FALSE-`ens`
      CONTRACT obligation discharges VACUOUSLY, AND the conjoined CONVERGENCE obligation `∃ r,
      whileBodyConverges …` is REFUTED at the same env; the `PinExecOverflowVacuity` shape one
      vacuity position over — fuel-exhaustion `none` instead of overflow `none`),
      `PinWhileComposition.lean` (the composition mis-map — a loop-SKIPPING `whileBodyDenote`
      variant binds the ENTRY value `lo = 0` and CERTIFIES the wrong `ens: result == 0`, while
      the FAITHFUL composition runs the SHIPPED L1 loop to the EXIT value `lo = 3` and REFUTES
      it; both directions pinned: the poisoned discharge AND the faithful refutation).
    - **REQ-11.4/REQ-11.5/REQ-11.7 SHIPPED (v-b, #264).** The EXPORTER landed
      (`forge/src/lean_export.rs`): `export_item` routes a v1 WHILE-shaped body (the last
      `Stmt::Loop(While)` before a REQUIRED tail) to `export_while_body`, the (v) recognizer
      `recognize_while_body` mirroring `recognize_v1_loop` arm-by-arm. **REQ-11.4** — it emits
      `Inv_item` (user invs shallow over cells + per loop-CELL SORT+RANGE + per read scalar
      PARAM SORT+RANGE (the missing-param-range fix the v-a/#265 critic anticipated — the
      step's no-overflow guard `lo+1 < 2^w` needs each read param's width bound `n < 2^w`,
      established at `_entry` from `InRangeParams` and preserved trivially) + other-param frame
      + per-cell scope) and `mu_item` (the `dec` over cells), the FIVE per-item obligations in
      the §4.2.4 DESIGN shapes — `_entry` the prefix-progress-AND-entry `∃ st₁, blockThread
      prefix_block (stateOf v) = some st₁ ∧ Inv_item v st₁`; `_pres`; `_progress`; `_dec` (the
      #265 PRE-state `0 ≤ μ st` bound); `_exit` the `∃ r, execDenote tail_expr st.env = some r
      ∧ <ens>` (the tail's own obligation rides in the `∃ r`) — with GENERATOR-FIXED proofs
      (the `WhileBattery` decode chains: `Bind.bindLeft`/sort/frame decode → the overflow `if`
      `split` → `omega`, `hreq`-aware per §4.2.4), AND the TWO GENERATOR-FIXED composed theorems
      (the HYPOTHESIZE CONTRACT via `while_compose`, the conjoined `_converges` via
      `loopDenote_exits_of_dec` — no `first | … | skip` heuristics; the prefix/tail totality
      comes from `_entry`/`_exit`'s ∃-content). **REQ-11.5** — `recognize_while_body` /
      `reject_out_of_while_subset_stmt` / `encode_cell_*` keep the §4.2.5 inventory LOUD
      (`loop`-kind / nested / non-last / under-`if` / multi-loop / break / continue /
      mid-return / non-scalar / empty-or-weak-inv / tail-less → `ExportRefusal::LoopBody`;
      spec-calling inv/dec → `ExportRefusal::OutOfFragment`; optres result → `OptResResult`).
      **REQ-11.7** — `check::lean_mutation_score` routes each mutant through
      `LeanEngine::admits_auto` → `export_item`; an in-grammar while-body mutant now EXPORTS
      and is ATTEMPTED (not `UntestedAgainstLean`), the floor gate genuinely gates while items
      on the Lean path (NO `check.rs` edit — the accounting flows through the fragment
      widening). VERIFICATION: `forge/tests/lean_while.rs` — `count_certifies_l3_via_lean_auto`
      (the L1 linear `while lo < n inv lo ≤ n dec n - lo { lo = lo + 1 }` certifies L3 via
      lean-auto, all 5+2 kernel-accept with `{propext, Classical.choice, Quot.sound}`),
      `sum_does_not_certify_l3_via_lean_recursive_residual` (the corpus `sum`'s `ens result ==
      spec_sum(xs)` is the recursive-registry §4 interactive residual — it does NOT certify
      L3-via-lean, the HONEST landing), `refusal_matrix_no_lean_certification` (the §4.2.5
      matrix never certifies L3-via-lean), `while_true_no_op_is_not_proven_l3_via_lean` (the
      §4.2.3 vacuity gate has teeth), `in_grammar_while_mutants_are_attempted_not_untested`
      (REQ-11.7); plus the in-process `engine::tests` (`live_while_body_item_is_honest`
      Proven, `while_body_item_refuses_export`, `while_refusal_inventory_is_structured`,
      `live_while_true_vacuity_is_not_proven`). KNOWN LIMITATION (the auto-DISCHARGEABLE
      surface is narrower than the EXPORTABLE surface, the iv-b precedent): NONLINEAR
      invariants/measures, multi-`let`-with-non-literal-init prefixes (the `_entry` witness
      emitter folds only `let cell = <literal>`), and bodies needing deeper case splits leave
      an unsolved goal → `Verdict::Unknown` (fail-to-certify, SOUND — never a false `Proven`;
      the REQ-7 INTERACTIVE path is the close). RECORDED as a coverage residual, NOT a blocker.

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
  inflation of the kill ratio and NO regression of equivalent-mutant handling. The floor GATES (does
  NOT merely report — the #248 fix): on the Lean-only path the kill-ratio over `attempted` must meet
  the floor for L3-via-Lean, else a `WeakContract`-style reject (`LeanMutationTally::meets_floor` +
  the `0/0` backstop, mirroring the Verus `meets_floor` gate); the #101 equivalence exclusion is NOT
  available on the Lean-only path (a §0.1 verus meta-query, F3/OQ-5), so the Lean denominator =
  `attempted` with no exclusion — recorded HONESTLY in the reject detail + the `qualifier`.
- **AC-8 (the increment plan + the one filed blocker)** — the four increments are recorded, each its
  own build blocker; increment (i) is FILED (#204) and named; (iv) is FILED (#253 — the §4.1
  exec-body bridge build, this amendment); (ii)/(iii) were named as future (and have since shipped
  in part, per the REQ status rows).
- **AC-9 (the exec-body bridge design is DECISION-COMPLETE and spine-grounded — §4.1)** — every
  piece the §4.1 stub enumerated is DECIDED against the SHIPPED spine, none waved at: the value
  bridge names the exact extraction (`BVal.value`, identity on the mathematical value) and its
  pinned failure modes (signedness / truncation); the bool decision is the `bindBool` SPINE
  ADDITION (NOT an Int-0/1 encoding — `PinExportBoolResult.lean`'s
  `true_false_indistinguishable_in_intVal` grounds why), enumerated arm-by-arm with its blast
  radius across the mutual denotation family; the optres position is honestly OUT (no `ExecVal`
  variant exists — blocker #254, a structured refusal, not a silent gap); the env→State map is
  stated WITH its correspondence invariant and an emit-time `rfl`-lemma mechanism; the obligation
  takes the HYPOTHESIZE form with the OVERFLOW class exported alongside, and the no-NB-layer
  decision is grounded in `bodyDenote`'s fuel-free `Option` (quoted); the four pins are named per
  bridge position; loop bodies refuse structurally. Mechanically: each §4.1 claim quotes the
  governing spine symbol (`Exec.lean` / `Exec/Stmt.lean` / `Denote.lean`).
- **AC-10 (the while-body widening design is DECISION-COMPLETE and spine-grounded — §4.2)** — the
  (v) exportable grammar is pinned to the SHIPPED `recognize_v1_loop` admit/reject set (quoted both
  sides); the composition layer names each (v-a) lemma WITH its statement shape and pins the kernel
  bar (standard axioms, every existing theorem + pin green — the iv-a precedent, the spine's
  `while_rule`/`Exec/Stmt.lean` NOT modified); the termination decision is RECORDED with its
  forcing ground (the `while true` fuel-exhaustion vacuity) and its two REJECTED alternatives
  (crediting an un-run Verus; a partial-correctness-marked L3); the emitted obligation set
  (FIVE per-item + TWO composed) maps onto the §1 obligation classes (LOOP-{ENTRY,PRESERVATION,
  EXIT}, OVERFLOW, TERMINATION) under the REQ-1.1 conjunction; the refusal inventory quotes the
  AST variants; the pins are named per divergence position (and the no-`PinDecMeasure`-analogue
  asymmetry is argued, not assumed); the REQ-9 accounting delta is stated. Mechanically: each §4.2
  claim quotes the governing symbol (`Exec/Loop.lean` / `Exec/Stmt.lean` / `lean_export.rs` /
  `exec_stmt_encode.rs`).

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

**What the class DISCHARGES, semantically (the #241 connection).** The class discharges CONVERGENCE
of every reachable spec-call: that the per-item registry `R_item` makes `Stabilize.lean`'s
none-propagating denotation reach a GENUINE value — `RegistryTerminating env e := ∃ v, Converges e
env v`, where `Converges e env v := ∃N∀fuel≥N, intValNB fuel e env = some v` (NOT the cycle-4 identity
`∃ v, stabilizes`, which a divergent registry satisfied at the Int-bottom 0 — Pin E, now resolved).
DEC-VALIDITY is the discharge METHOD, not the semantic content: a valid well-founded `dec` measure
bounds each spec-call's unfolding to a finite per-env depth, which is exactly what makes `intValNB`
reach `some` (the convergence), and the agreement lemma `converges_imp_stabilizes` then carries that
to genuine `stabilizes` (the §4 obligation's currency).

**Discharge (two admitted paths).** (a) THE COMMON PATH (the Verus dec-check engine): Verus's existing
dec-check, which certifies the spec-fns when Verus discharges the item — a Verus-discharged item's
spec-fns have ALREADY passed Verus's recursion/decreases check, so the class is discharged by the
Verus engine; the CONVERGENCE connection is the bridge stated for this path (a valid Verus `dec`
means the spec-fn terminates, hence the denotation converges). (b) THE ENGINE-#2 PATH: a Lean
well-foundedness proof of the `dec` measure (a `termination_by`/`decreasing_by`-shaped obligation over
the encoded `R_item`, in the AUTO battery where the measure is scalar/linear, otherwise INTERACTIVE),
which on the Lean path proves `Converges` via the unfolding bound the valid measure supplies. Either
path discharges REGISTRY-TERMINATION; the conjunction rule requires ONE of them per spec-fn.

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
bounded` (SHIPPED with genuine content, #241: under `RegistryTerminating := ∃ v, Converges`, the
agreement lemma `converges_imp_stabilizes` yields genuine `stabilizes` — a divergent registry, having
no `Converges` witness, no longer discharges it): because the source `dec` measure makes every
spec-fn's recursion well-founded — a property the REGISTRY-TERMINATION class (REQ-1.2) DISCHARGES
rather than assumes — each
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

**SHIPPED (the spine prerequisites — #240/#241, ref #203, `lean/Thermite/Stabilize.lean`).** The
relations `stabilizes`/`stabilizesProp` (matching `Denote.lean`'s `intVal`/`denote`), the
uniqueness-of-stabilization fact `stabilizes_unique` (the #214 lever, overlap-at-max), the
FUEL-IRRELEVANCE lemma `intVal_fuel_irrelevant`/`denote_fuel_irrelevant` (+ `seqVal`/`denoteArms`
mutual companions; over the decidable Bool predicate `specCallFree` ranging the FULL mutual AST), and
the tier-(a) fuel-free corollaries `stabilizesProp_iff_denote_zero`/`stabilizes_iff_intVal_zero` are
all kernel-checked with `{propext, Classical.choice, Quot.sound}` (NO `sorryAx`).

**The supporting lemma `stabilization_exists_for_dec_bounded` SHIPPED with GENUINE (non-identity)
content (the #241 ROOT fix).** The cycle-4 form keyed `RegistryTerminating env e := ∃ v, stabilizes e
env v` and shipped `stabilization_exists` as the DEFINITIONAL identity over it — an IDENTITY
HYPOTHESIS that a DIVERGENT registry SATISFIED at the bottom: for `f(x)=f(x)` the bottoming `intVal`
is constantly the Int-bottom `0`, so `stabilizes (f x) env 0` HOLDS and the registry §1.2's class
exists to REJECT cleared the hypothesis (the critic's kernel-checked Pin E, commit `f7d288ef`). The
root cause is that the BOTTOMING `intVal` cannot distinguish "stabilized to a genuine value" from
"stuck at the bottom because it diverged". **The fix is a BOTTOM-DISTINGUISHING denotation:** a
second, NONE-PROPAGATING denotation `intValNB`/`denoteNB` (+ the `seqVal`/`args`/`arms`/`countWhere`
companions) mirrors the spine recursion EXACTLY save THREE points — a fuel-0 `specCall` → `none`, an
unresolved `specCall` → `none`, every arm PROPAGATES `none` — so `intValNB f e env = some v` means
`e` reached a GENUINE value WITHOUT bottoming. `Converges e env v := ∃N∀fuel≥N, intValNB fuel e env =
some v` is the genuine-convergence relation, and **the AGREEMENT LEMMA `converges_imp_stabilizes`**
(via the mutual `intValNB_agrees`/`denoteNB_agrees`/`seqValNB_agrees`/`intValArgsNB_agrees`/
`denoteArmsNB_agrees`): where NB is `some v`, no bottom arm was taken, so the bottoming `intVal` runs
identically and stabilizes to the SAME `v`. `RegistryTerminating env e` is REDEFINED `∃ v, Converges
e env v`, and `stabilization_exists` now carries the agreement lemma's content (genuine convergence
hypothesis → genuine stabilization conclusion), NOT `id`. **The divergent registry now FAILS the
hypothesis** (`PinRegistryTerminating.lean`, RE-PINNED to the resolved truth per the #186 precedent:
`divergent_call_NB_is_none` — `intValNB` is `none` at EVERY fuel — gives
`divergent_registry_fails_the_hypothesis : ¬ RegistryTerminating envD fCall`, the load-bearing
reversal of the cycle-4 divergence), while a GENUINE (dec-valid) registry `g(x)=5` still CONVERGES
and STABILIZES to 5 (`genuine_registry_converges`/`genuine_registry_stabilizes` — the fix does NOT
over-reject). The hypothesis `RegistryTerminating` (= `∃ v, Converges`) is EXACTLY what the per-item
REGISTRY-TERMINATION obligation class (REQ-1.2) discharges — the dec-validity proof of each spec-fn
in `R_item` is what supplies the per-env finite unfolding that makes `intValNB` reach `some` — so it
is the named, separately-discharged obligation the conjunction rule requires, AND it is now a
GENUINE precondition a divergent registry cannot forge (§1.2). The spec-call-free fragment Converges
(and so stabilizes) UNCONDITIONALLY (`converges_specCallFree`/`stabilization_exists_specCallFree`, no
hypothesis — `intValNB` is total-`some` and fuel-irrelevant there), so the tier-(a) auto fragment's
convergence is free. All new/changed theorems are kernel-checked with `{propext, Classical.choice,
Quot.sound}` (NO `sorryAx`). The four OTHER critic pins
(PinIntBottom/PinStabilization/PinBodyRegistry/PinDecMeasure) keep their own local
`stabilizes`/`stabilizesProp` copies and still build green (standard axioms). The exporter targeting
(REQ-6/REQ-7) and the REGISTRY-TERMINATION Lean-path well-foundedness discharge (REQ-1.2(b)) — i.e.
the Rust→Lean proof that a dec-valid `R_item` supplies a `Converges` witness — remain
increment-(ii)/(iv) work. Recorded as an AMENDMENT to increment (ii) inside the existing #204
build-blocker chain (see the header `build-blockers:` block) — NOT a separately-filed issue.

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
contract `ens` over `Env` — is NOT part of this increment-(ii) class. It is increment (iv)'s
OWN design obligation, DESIGNED in §4.1 (build blocker #253). The doc STOPS presenting a unified
S_C×S_E sketch it cannot type — §4.1 is the TYPED bridge.

#### 4.1 The exec-body bridge (increment (iv) — DESIGNED here; build blocker #253) (REQ-1.1/REQ-10, the #212 fix)

**History (why this was a stub, and what changed).** The cycle-2 sketch wrote `Env.bindInt { … }
"result" body` with `body` an item body — which does NOT typecheck against the spine:
`Env.bindInt : Env → String → Int → Env` (`Denote.lean`) takes an `Int`, but a general item body is
a `Block` denoting via `Thermite.Exec.bodyDenote : Block → State → Option ExecVal`
(`Exec/Stmt.lean`) in the BOUNDED domain. Tying `S_C` (the contract `Env`) to `S_E`/`S_B` (the exec
`State`) in one statement is a NOVELTY — the spine's own theorems relate `refDenote`/`denote` and
`bodyRefState`/`bodyDenote` SEPARATELY; there is NO single artifact tying `S_C` and `S_E`/`S_B`
today. This section WAS the honest "NOT designed here" stub; it is now the DESIGN (the #212 fix
completed), and crosslink #253 is the build blocker that owns the code. The conjunction rule and
the #212(b) HYPOTHESIZE resolution below are UNCHANGED and NORMATIVE — this design instantiates
them against the shipped spine.

**SCOPE (the STRAIGHT-LINE-BODY class).** An item is in the (iv) exportable class when its body is
a straight-line `S_B` block — the EXACT subset `Exec/Stmt.lean` mechanizes: `Stmt.letS` /
`Stmt.assign` / `Stmt.exprS` / `Stmt.ifElse` over sub-`Block`s, sequenced left-to-right by
`blockThread`, with a tail `ExecExpr` (the RHS/condition/tail positions are 2a `ExecExpr`s:
`intLit`/`boolLit`/`var`/`arith`/`cmp`/`logic`/`not`/`cast`/`index`) — and its declared result
sort is an exec int (`u8/u16/u32/u64/usize` → the §4.1.1 `BVal.value` bridge) or `bool` (→ the
§4.1.2 `bindBool` bridge). LOOPS are OUT (§4.1.7 — `Exec/Stmt.lean` has NO loop `Stmt` form,
"LOOPS are EXPLICITLY OUT (increment 2c, #163)"; `Exec/Loop.lean`'s `loopDenote`/`while_rule` is a
SEPARATE artifact NOT composed into `bodyDenote`). Option/Result-typed RESULTS are OUT (§4.1.3,
blocker #254). The contract side is UNCHANGED: `req`/`ens` keep the §4 stabilized/fuel-free forms
over `R_item` (the hard gate, the §6.1 tiers, REGISTRY-TERMINATION all apply VERBATIM — a
straight-line body's `ExecExpr`s CANNOT contain spec-calls, so the body contributes ∅ to the #226
`calledSpecFns` seed and the gate/tier reconciliation is unperturbed). The SHIPPED pure-contract
export path is NOT churned: a pure-`intVal` tail body keeps the §4 `stabilizes body_expr env r`
form; the bridge form below applies to bodies WITH statements (or with an exec-bool result).

**(§4.1.1) The S_E→S_C VALUE bridge — `BVal.value`, the mathematical value, bound as itself.**
`bodyDenote` yields `Option ExecVal` where `ExecVal = int (BVal) | bool (Bool)` (`Exec.lean`);
`BVal { ty, value : Int }` carries its type-bound (`BVal.inRange : 0 ≤ value < ty.bound`), and —
the load-bearing exec fidelity the spine already proves — "THE VALUE IS THE MATHEMATICAL RESULT
GIVEN NO OVERFLOW — never a wrap, never a nat-coercion" (`evalArith`, `Exec.lean`). So for an
int-sorted result the bridge is the IDENTITY on the mathematical value: the antecedent binds
`r : BVal` and the consequent denotes `ens` at `Env.bindInt env "result" r.value`. NOTHING ELSE:
no `Int.toNat` clamp (the `nat_coercion_underflow_breaks_soundness` bug class, already a proven
negative lemma in `Exec.lean`), no `% bound` re-wrap (the identity on in-range values, but a
NARROWER-width wrap TRUNCATES — pinned, §4.1.6), no two's-complement reinterpretation (the exec
domain is the UNSIGNED `[0, ty.bound)`; a signed re-read maps `2^64 − 1 ↦ −1` — pinned). The width
`r.ty` is deliberately NOT carried into the `Env`: `S_C` is the unbounded domain and the contract
compares MATHEMATICAL values — faithful exactly because the bounded ops yield the mathematical
result whenever they yield anything at all.

**(§4.1.2) The bool-result binding — the `bindBool` SPINE PREREQUISITE (the build lands it
FIRST).** A bool-typed result has NO binding site today: `Env` is `{ ints, seqs, optres, specs }`
(`Denote.lean`) — no bool sort — and the contract AST has no bool-sorted NAME node (`Ast.lean` has
`var`, read via `env.ints`; `seqVar`; `optResVar`; and the leaf `boolLit` — nothing reads a bool
NAME). The shipped exporter therefore REFUSES bool results (`ExportRefusal::NonIntResult`, the
#244 fix; `PinExportBoolResult.lean` — Pin H — pins WHY: `intVal` bottoms EVERY bool-sorted node
to the catch-all `0`, `true_false_indistinguishable_in_intVal`, so an Int-0/1 route would let a
contract AND its negation both certify — which is why the Int-encoding alternative stays
REJECTED). DECISION (unchanged from the stub, now designed): a GENUINE bool sort, in four named
pieces —
  1. `Env.bools : String → Bool := fun _ => false` — a DEFAULTED field, so every existing `Env`
     literal and `{ v with … }` update in the spine, the exporter's emitted files, and the critic
     pins all elaborate UNCHANGED (the minimality lever: no spine-wide literal churn).
  2. `def Env.bindBool (env : Env) (name : String) (b : Bool) : Env := { env with bools := fun s
     => if s = name then b else env.bools s }` — the `Env.bindInt` mirror.
  3. A new contract-AST leaf `Expr.boolVar (name : String)` — the bool-sorted free-name read the
     `ens` needs to mention a bool `result`. Its arms, enumerated (a new `Expr` constructor
     touches the WHOLE mutual family — this is the honest blast radius, mechanical because the
     leaf is fuel-free and subterm-free): `denote`: `| _, Expr.boolVar x, env => (env.bools x =
     true)` (the `boolLit` arm's shape); `intVal`: NO new arm — the existing bool-sorted catch-all
     `| _, _, _ => 0` covers it (a `boolVar` in Int position is the sort error the exporter's EXP
     discipline never emits); `denoteNB`: an EXPLICIT `| _, Expr.boolVar x, env => some (env.bools
     x = true)` arm (it must NOT fall to the `some True` catch-all, which would break
     `denoteNB_agrees`'s carried-proposition agreement at `env.bools x = false`); `intValNB`: the
     bool-sorted catch-all `some 0` (matches `intVal` — agreement preserved); `refDenote`
     (`RefEncode.lean` mirror): the identical arm, so T1 re-establishes; `specCallFree`: `true`
     (no subterms); the fuel-irrelevance and `*_agrees` mutual lemmas: one trivial fuel-free case
     each. This layer is THE increment-(iv) spine prerequisite, EXACTLY parallel to increment
     (ii)'s `Stabilize.lean` layer (#240): the build lands it FIRST, kernel-green with the
     standard axiom set and every existing pin still compiling, BEFORE the exporter targets it.
  4. The exporter then NARROWS `NonIntResult`: a `-> bool` straight-line item exports with the
     `.bool` antecedent (§4.1.5) and `ens` denoted at `env.bindBool "result" b`, reading `result`
     as `Expr.boolVar "result"`.

**(§4.1.3) The optres binding — recorded OUT of (iv) v1 (blocker #254), with the target form
fixed.** The stub said Option/Result-typed results "bind via the EXISTING `optres` env slot". The
BINDING side is indeed free: `env.optres : String → OptResVal` is SHIPPED (`Denote.lean`, the #180
fragment) and the binder is a plain record update (`Env.bindOptRes`, the `bindInt` shape — no new
sort needed). But the ANTECEDENT side is NOT representable in today's spine: `ExecVal` is
`int (BVal) | bool (Bool)` ONLY (`Exec.lean`) — `bodyDenote` CANNOT produce an Option/Result
result, and no `ExecExpr`/`Stmt` form constructs one. So Option/Result-typed straight-line-body
results stay REFUSED (the existing `ExportRefusal::NonIntResult` carries the type — an honest
structured skip), and the spine extension (an `ExecVal` Option/Result variant + the
`execDenote`/`bodyDenote` producing arms + `Env.bindOptRes`) is the FILED follow-on blocker #254,
NOT silently waved into #253. The doc adapts to the code: this position's TARGET form is fixed
above; its build is gated on #254.

**(§4.1.4) The env→State correspondence (`stateOf` + `InRangeParams` + the `rfl` lemmas).** The
contract `Env { ints, seqs, optres, specs }` and the exec `State { env : ExecEnv { vars : String →
ExecVal, slices : String → List BVal }, scope : String → Bool }` (`Exec/Stmt.lean`) are DISJOINT
structures; the exported theorem quantifies ONE valuation (`∀ (v : Env)` — the shipped tier
forms' shape) and DERIVES the exec state from it via a generator-emitted, item-specific
definition — the `R_item` precedent:

```
def stateOf (v : Thermite.Env) : Thermite.Exec.State :=
  { env := { vars := fun s =>
               if s = "x" then .int ⟨.u64, v.ints "x"⟩                  -- scalar param at its width
               else if s = "p" then .bool (v.bools "p")                  -- bool param (§4.1.2 field)
               else .int ⟨.u64, 0⟩                                       -- the default cell
             slices := fun s =>
               if s = "xs" then (v.seqs "xs").map (fun n => ⟨.u32, n⟩)   -- slice at its elem width
               else [] }
    scope := fun _ => false }                                            -- params are INPUTS, not cells
```

  - **A scalar int param `x : uW`** → the `State` cell `vars x = .int ⟨.uW, v.ints x⟩` AND the
    contract reads the SAME free name `v.ints x` — ONE valuation, two views.
  - **A slice param `xs : &[uW]`** → `slices xs = (v.seqs xs).map (⟨.uW, ·⟩)` — the contract's
    `List Int` and the exec's `List BVal` agree element-wise on `BVal.value`.
  - **`scope := fun _ => false`** — params are free INPUTS, exactly the spine's own exemplar
    (`Exec/Stmt.lean`'s `inputState`: "nothing `let`-bound yet (the `let`/`assign` cells are
    introduced by the body)"). Consequence: a body `assign` to a PARAM is `none` (the
    unbound-target guard). Whether the Rust `body_ref_state` seeds params as ASSIGNABLE cells is
    an EXP inspection row the build settles arm-by-arm against `exec_stmt_encode.rs`; if the
    encoder admits param-assign, `stateOf` marks params in scope instead — the §4.1.6 mis-map pin
    covers the divergence either way.
  - **The typed-input premise `InRangeParams(v)`** — emitted as hypotheses: per int param
    `0 ≤ v.ints x ∧ v.ints x < (IntTy.uW).bound`, and per slice element likewise. This is the exec
    type system's guarantee on inputs (a `u64` param IS in `[0, 2^64)` — the same assumption the
    Verus path gets from typing); without it the bare `∀ v : Env` would feed the bounded semantics
    cells no real execution can produce, spuriously failing OVERFLOW obligations (over-rejection,
    not unsoundness — hypothesized for precision, stated here for honesty).

  **THE CORRESPONDENCE INVARIANT (normative).** For every parameter of the item, the exec read and
  the contract read agree on the mathematical value: `asInt ((stateOf v).env.vars x) = some ⟨w_x,
  v.ints x⟩` (int params), `asBool ((stateOf v).env.vars p) = some (v.bools p)` (bool params), and
  `((stateOf v).env.slices xs).map BVal.value = v.seqs xs` (slice params). Because `stateOf` is
  DEFINITIONAL in `v`, the invariant is `rfl`-discharged — and the exporter EMITS the per-param
  correspondence lemmas alongside (`example : asInt ((stateOf v).env.vars "x") = some ⟨.u64,
  v.ints "x"⟩ := fun _ => rfl`-shaped), the §4 mechanism-2 parallel: a mis-mapped / dropped /
  mis-widthed param FAILS TO COMPILE, independent of inspection. `stateOf`'s faithfulness (right
  names, right widths, right sort routing) is otherwise part of EXP (arm-by-arm + the drift
  tripwire), like `R_item` body-faithfulness.

**(§4.1.5) The obligation form — the HYPOTHESIZE position realized, and NO NB layer.** First the
DECISION, grounded: the exec side needs NO bottom-distinguishing NB/none-propagating layer and no
new convergence relation with content. `bodyDenote : Block → State → Option ExecVal` is FUEL-FREE
— `ExecExpr` has NO `specCall` constructor (`Exec.lean`'s inductive:
`intLit/boolLit/var/arith/cmp/logic/not/cast/index`) and `Stmt`/`Block` add none — so there is no
registry, no fuel index, and no default-value bottom ANYWHERE on the exec side. Its `none` arises
ONLY at the GENUINE failure sites — `evalArith` overflow / div-or-rem-by-zero / negative shift,
the out-of-range `index`, the `asInt`/`asBool` sort mismatch, the `letS` re-shadow, the unbound
`assign` target, a tail-less block — and `some v` means a genuine value (the spine's own teeth:
`body_overflow_rhs_has_no_result` vs `body_in_range_rhs_has_result`, `Exec/Stmt.lean`). The
#213/#241 trap — a TOTAL denotation that FORGES a value at the bottom (`intVal`'s `0`), which is
what forced `intValNB`/`Converges` on the contract side — does NOT exist in `S_B`: the `Option`
IS the bottom-distinguishing layer. So the stub's `bodyStabilizes` placeholder is realized as a
definitional abbreviation, not a denotation:

```
abbrev bodyConverges (b : Block) (st : State) (r : ExecVal) : Prop :=
  Thermite.Exec.bodyDenote b st = some r
```

("converges", not "stabilizes" — there is no fuel to stabilize over). The exported CONTRACT
obligation for an int-result straight-line item — the #212(b) HYPOTHESIZE form (normative block
below), composed with the §4 clause forms:

```
theorem item_xyz :
  ∀ (v : Thermite.Env),
    let env := { v with specs := R_item }                   -- registry HELD FIXED (§4, unchanged)
    InRangeParams v →                                        -- the typed-input premise (§4.1.4)
    ∀ (r : Thermite.Exec.BVal),
    bodyConverges body_block (stateOf v) (.int r) →          -- the HYPOTHESIZE antecedent
    stabilizesProp req env →                                 -- reqStable (§4, unchanged)
    stabilizesProp ens (Env.bindInt env "result" r.value)    -- ensStable at the BVal.value bridge
```

(bool-result: `∀ (b : Bool), bodyConverges body_block (stateOf v) (.bool b) → … → stabilizesProp
ens (env.bindBool "result" b)`.) The clause positions keep the §4/§6.1 tier machinery VERBATIM —
a specCall-free `req`/`ens` exports the fuel-free `denote 0` shape (tier (a)); a recursive
registry marks the clause side interactive (tier (c)); the BODY antecedent is ALWAYS fuel-free.
The parallelism with §4's pure-contract form is exact: there the result is bound THROUGH
`stabilizes body_expr env r` (uniqueness of stabilization forces the true value, #214); here it is
bound THROUGH `bodyConverges` — and uniqueness is FREE (`bodyDenote` is a function, so
`some`-results are unique by `Option.some.injEq`; no analogue of `stabilizes_unique` is needed).

**The conjoined OVERFLOW export (the soundness condition, made concrete).** Per the conjunction
rule below, the HYPOTHESIZE form is sound ONLY because the OVERFLOW class is MANDATORILY conjoined
item-wide: an always-overflowing body has `bodyDenote = none`, the antecedent is false, the
CONTRACT obligation is vacuously provable — and the item STILL does not certify, because its
OVERFLOW class fails separately. Increment (iv) therefore EXPORTS the OVERFLOW obligation on the
Lean path (on the Verus path Verus discharges it in-item, as today):

```
theorem item_xyz_overflow :
  ∀ (v : Thermite.Env),
    InRangeParams v →
    stabilizesProp req { v with specs := R_item } →          -- the body may rely on req
    (Thermite.Exec.bodyDenote body_block (stateOf v)).isSome
```

— i.e. under the precondition, EVERY 2a obligation threaded through the body (overflow / div-zero
/ bounds / sort / scope) discharges. A Lean-only (`--engine lean`) straight-line item certifies
ONLY when BOTH theorems kernel-accept (the per-item conjunction at the certificate level,
REQ-1.1); a missing or failed OVERFLOW discharge is the conjunction-rule reject — NEVER a silent
L3 on the vacuous CONTRACT.

**(§4.1.6) The kernel pins per bridge position (the build authors them; NAMED here).** The Pin
A/B/C/E/F/G/H precedent: each is a kernel-checked divergence oracle showing the unsound variant
discharging where the faithful form refuses (and the faithful form behaving correctly at the same
witness), kept compiling as the regression oracle. The four the build MUST pin:
  - **`PinExecValueBridge.lean` (the wrong value bridge — signedness / truncation).** A mis-bridge
    binding the SIGNED reinterpretation (`if r.value ≥ 2^63 then r.value − 2^64 else r.value`) or
    a NARROWER-width re-wrap (`r.value % 2^32`) in place of `BVal.value` lets a WRONG contract
    certify at a witness (a body converging to `.int ⟨.u64, 2^64 − 1⟩`: the signed mis-read binds
    `−1` and `ens result < 0` discharges; the faithful bridge binds `2^64 − 1` and REFUTES it).
    Both directions pinned: the poisoned discharge AND the faithful refutation.
  - **`PinExecBoolBind.lean` (the bool-result mis-bind).** A mis-bind that DROPS the bind (the
    consequent reads the DEFAULTED `Env.bools` `false` regardless of the body's `.bool true`
    result) or routes through the rejected Int-0/1 encoding certifies a negated contract; the
    faithful `bindBool` refutes it. (Extends the SHIPPED Pin H — `PinExportBoolResult.lean`'s
    `true_false_indistinguishable_in_intVal` — from "why the Int route is refused" to "why the
    bind must be genuine".)
  - **`PinExecOverflowVacuity.lean` (the overflow-vacuity escape).** A body that ALWAYS overflows
    under the precondition (the `body_overflow_rhs_has_no_result` shape — `let a = m + m` with a
    `req` forcing `m` at the `u64` rim) makes the CONTRACT obligation vacuously kernel-accept WITH
    A FALSE `ens` — AND the pin proves the conjoined OVERFLOW obligation REFUTED at the same env.
    This is the certificate-level conjunction's regression oracle (the Pin B shape): the vacuous
    CONTRACT discharge must stay UNREACHABLE as a certificate, blocked by the failing OVERFLOW
    class.
  - **`PinExecStateMisMap.lean` (the env→State mis-map).** A `stateOf` that DROPS the seqs→slices
    map (the exec body reads `slices xs = []` while the contract's `xs.len()` reads `v.seqs xs`),
    seeds a param at the WRONG width, or routes a name to the wrong sort, makes a wrong contract
    certify / a right one fail at a witness; the faithful map agrees. The per-param `rfl`
    correspondence lemmas (§4.1.4) are the compile-time tripwire this pin motivates.

**(§4.1.7) The loop class stays OUT — the refusal made STRUCTURED.** while-body items are
NON-exportable in (iv) (post-v1): `Exec/Stmt.lean` mechanizes NO loop form ("LOOPS are EXPLICITLY
OUT (increment 2c, #163)"), and composing `Exec/Loop.lean`'s `loopDenote` + the
partial-correctness `while_rule` into a body obligation (invariant threading + the REQ-7
PARTIAL-CORRECTNESS-only certificate marking) is its OWN future design, NOT smuggled into #253
(that design now EXISTS: §4.2, increment (v), blocker #264 — the refusal narrows AGAIN when (v)
ships; until then the v1 while shape refuses via `LoopBody` like the post-v1 shapes, and §4.2.3
RESOLVES the certificate marking: no partial-correctness L3 — a conjoined convergence obligation).
Today the exporter refuses ALL statement bodies as `ExportRefusal::NotPureContract` ("a
`let`/`assign`/`return`/`loop`/`if`-statement body", `forge/src/lean_export.rs`); the build
NARROWS it: the straight-line forms become exportable, while a
`loop`/`while`/`break`/`continue`/mid-body-`return`/non-scalar-mutation (`xs[i]=e`) body gets a
DISTINCT structured refusal naming the loop residual (an honest skip the cert reports via the
`LeanUnverifiable` path — NEVER a silent omission, NEVER an attempt to denote what `S_B` does not
model). SHIPPED (#253 iv-b): the refusal IS `ExportRefusal::LoopBody` (`encode_exec_stmt`,
`forge/src/lean_export.rs`); the Option/Result-typed-RESULT refusal alongside it is
`ExportRefusal::OptResResult` (#254). This matches the §8 OUT enumeration verbatim.

**(the novelty, owned — and discharged by design).** This is still the FIRST S_C×S_E/S_B-tying
artifact; its soundness story is now EXPLICIT: the env→State correspondence is definitional
(`stateOf` + the `rfl` lemmas + the `InRangeParams` typing premise), the value/bool bridges are
identities on the mathematical value into named `Env` binders, the body antecedent is the
fuel-free genuine `Option` (no NB layer to trust), and the vacuity seam is closed by the conjoined
OVERFLOW export per the conjunction rule below. Until #253 lands, the Lean engine's IN set (§8)
remains the PURE-CONTRACT class for body-binding; exec/body/loop obligations stay their OWN
obligation classes.

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

#### 4.2 The while-body widening (increment (v) — DESIGNED here; build blocker #264) (REQ-11)

**History (what (iv) refused, what is already proven, and the feasibility check).** Increment (iv)
stopped at straight-line bodies: `encode_exec_stmt` (`forge/src/lean_export.rs`) refuses
`Stmt::Loop(node)` with the structured `ExportRefusal::LoopBody` ("§4.1.7 — S_B mechanizes NO loop
form; the while/loop residual is the future increment"), so under `--engine lean` a loop-bearing
item is today the honest `LeanUnverifiable` skip (`while_body_item_refuses_export`, the live pin).
But the LOOP brick of the spine is ALREADY kernel-proven (`lean/Thermite/Exec/Loop.lean`, axioms
`[propext, Quot.sound]`): the genuine fuel-indexed iteration

```
def loopDenote (cond : ExecExpr) (body : Block) : Nat → State → Option State
-- fuel-0 = `none` (fuel exhausted — NOT a fixpoint claim); one SHIPPED `blockThread body` step
-- per iteration; a body/cond failure `none` PROPAGATES
```

and the PARTIAL-CORRECTNESS while-rule

```
theorem while_rule (cond body I)
    (h_pres : ∀ st, I st → condBool cond st = some true →
                ∀ st', blockThread body st = some st' → I st') (fuel) :
    ∀ st stf, I st → loopDenote cond body fuel st = some stf →
      I stf ∧ condBool cond stf = some false
```

(+ `tv_meta_loop`, the L1 non-vacuity fixture `b_loop_iterates`/`l1_while_rule_certifies_exit`,
and the L2/L3 negatives `l2_no_preservation_premise_for_buggy_body`/`l3_exit_overclaim_refuted`).
So (v) is engineering on a proven foundation, exactly as §4.1 was engineering on `bodyDenote`.
**FEASIBILITY (checked against the actual statements — nothing designed around).** `while_rule`
composes with the (iv) machinery AS-IS: its `h_pres` premise is over the SHIPPED `blockThread`
(the same transformer `bodyDenote` threads), its conclusion `I stf ∧ condBool cond stf = some
false` is exactly the exit characterization the tail's `execDenote` consumes, and `loopDenote`
maps `State → Option State` (the §4.1.4 `stateOf` codomain). No statement-shape mismatch exists.
The ONE genuinely missing piece is a COMPOSED whole-body denotation — `bodyDenote` has no loop arm
(`Exec/Stmt.lean`: "LOOPS are EXPLICITLY OUT (increment 2c, #163)") and `loopDenote` yields a
`State`, not the body's `ExecVal` result — plus the termination story §4.2.3 decides. Both are NEW
NAMED work, not statement-shape repair.

**(§4.2.1) SCOPE — the (v) exportable grammar, pinned to the SHIPPED recognizer (EXP).** An item
enters the (v) class when its body is EXACTLY the shape `thermite-tv`'s `recognize_v1_loop`
(`thermite-tv/src/exec_stmt_encode.rs`) admits for loop-TV — the (v-b) exporter recognizer is
pinned arm-by-arm against it, the same EXP correspondence class as the encoders:

```
body   := prefix* while tail
prefix := straight-line Stmt (the (iv) S_B subset: letS/assign/exprS/ifElse; NO loop forms)
while  := Stmt::Loop(LoopNode { kind: While(cond), invs: non-empty, dec, body: lbody })
          — the LAST statement before the tail ("a loop followed by further straight-line
          mutation is a v1.1 extension", recognize_v1_loop)
lbody  := straight-line SCALAR Block (NO nested Stmt::Loop, NO Stmt::Break/Continue/Return,
          NO non-scalar assign — the loop-TV reject_out_of_subset_body set)
tail   := a REQUIRED tail ExecExpr (the result; a tail-less while body refuses)
result := exec int (the §4.1.1 BVal.value bridge) or bool (the §4.1.2 bindBool bridge)
inv/dec(loop) := clauses in the SCALAR shallow fragment (cmp/arith/logic over loop cells +
          params — what the §4.2.4 cell-read encoder denotes); a spec-calling/combinator
          invariant REFUSES out-of-fragment in (v) v1 (a recorded residual). Consequence:
          a (v) loop contributes ∅ new spec-calls, so the #226 `calledSpecFns` closure and
          the §4 hard gate are UNPERTURBED — no seed extension is needed in (v)
```

**(§4.2.2) The (v-a) SPINE PREREQUISITE — the composition layer (six named pieces; lands FIRST,
kernel-green, the (ii)-`Stabilize.lean` / iv-a precedent).**

```
-- (v-a) — NOT yet built (#264). The names + statement shapes are pinned HERE:
def whileBodyDenote (prefix : Block) (cond : ExecExpr) (lbody : Block)
    (tail : ExecExpr) (fuel : Nat) (st : State) : Option ExecVal := do
  let st₁ ← blockThread prefix st          -- the straight-line PREFIX (a tail-less Block)
  let stf ← loopDenote cond lbody fuel st₁ -- the SHIPPED iteration (`none` propagates)
  execDenote tail stf.env                  -- the tail at the exit state

abbrev whileBodyConverges (prefix : Block) (cond : ExecExpr) (lbody : Block)
    (tail : ExecExpr) (st : State) (r : ExecVal) : Prop :=
  ∃ fuel, whileBodyDenote prefix cond lbody tail fuel st = some r
```

1. **`whileBodyDenote`** — the composed whole-body denotation, the FIRST `S_B`×`S_Loop` artifact
   (prefix `blockThread` → `loopDenote` → tail `execDenote`, `Option`-monad composed: a prefix /
   iteration / tail failure OR fuel exhaustion is `none`). `Exec/Stmt.lean` and `Exec/Loop.lean`
   are NOT modified — the layer composes AROUND them (preserving `Exec/Loop.lean`'s
   `WhileLoop`-not-a-`Stmt`-arm modeling decision; no re-proof of `body_ref_sound`/`while_rule`).
   Its faithfulness to the source body's meaning is an EXP row against the Rust loop-TV threading
   (`recognize_v1_loop` threads the prefix by the SAME `thread_stmt`, steps one iteration by the
   SAME `body_ref_state` step the Lean `blockThread` mirrors).
2. **`whileBodyConverges`** — the ∃-fuel convergence relation. The ∃ is FORCED (the #213 lesson):
   the iteration count is env-dependent (the L1 fixture exits at fuel `n+1` with `n`
   ∀-quantified), so NO export-time fuel exists; like `stabilizes`, the relation quantifies the
   index away and the result is bound THROUGH it (the #214 discipline). NO NB layer (the §4.1.5
   argument verbatim — `whileBodyDenote`'s `none` is GENUINE: a failure or fuel exhaustion, never
   a forged value).
3. **`loopDenote_fuel_mono`** — `loopDenote cond body f st = some stf → ∀ g, f ≤ g → loopDenote
   cond body g st = some stf` (after exit, surplus fuel is unconsumed; induction on `f`).
4. **`whileBodyConverges_unique`** — `whileBodyConverges … st r₁ → whileBodyConverges … st r₂ →
   r₁ = r₂` (the `stabilizes_unique` mirror: overlap-at-max via fuel-mono, then the functional
   `blockThread`/`execDenote` + `Option.some.injEq`). Binding `r` through the ∃-fuel relation is
   thereby well-defined — the exporter computes NO value; the relation forces the true one.
5. **`while_compose`** — the loop-exit-to-ens composition lemma (the bridge wrapping the
   straight-line prefix/tail segments AROUND `while_rule`):

```
theorem while_compose (prefix lbody : Block) (cond tail : ExecExpr) (I : State → Prop)
    (h_pres : ∀ st, I st → condBool cond st = some true →
                ∀ st', blockThread lbody st = some st' → I st') :
    ∀ st₀ fuel r,
      whileBodyDenote prefix cond lbody tail fuel st₀ = some r →
      (∀ st₁, blockThread prefix st₀ = some st₁ → I st₁) →
      ∃ stf, I stf ∧ condBool cond stf = some false ∧ execDenote tail stf.env = some r
```

   In prose: ANY converged whole-body result is the tail's value at SOME exit state satisfying
   `I ∧ ¬cond` — `while_rule` lifted through the composition (proof shape: unfold
   `whileBodyDenote`; the prefix step is a function, so the entry state is determined; apply
   `while_rule` to the middle segment).
6. **`loopDenote_exits_of_dec`** — the TERMINATION bridge (§4.2.3's currency — the
   `converges_imp_stabilizes` mirror, one domain over):

```
theorem loopDenote_exits_of_dec (cond : ExecExpr) (lbody : Block)
    (I : State → Prop) (μ : State → Int)
    (h_pres : ∀ st, I st → condBool cond st = some true →
                ∀ st', blockThread lbody st = some st' → I st')
    (h_cond_total : ∀ st, I st → (condBool cond st).isSome)
    (h_progress   : ∀ st, I st → condBool cond st = some true →
                      (blockThread lbody st).isSome)
    (h_dec        : ∀ st st', I st → condBool cond st = some true →
                      blockThread lbody st = some st' → μ st' < μ st ∧ 0 ≤ μ st) :
    ∀ st, I st → ∃ fuel stf, loopDenote cond lbody fuel st = some stf
```

   dec-VALIDITY (strict, bounded-below descent of the denoted measure across each GENUINE
   `blockThread` step) + PROGRESS (the condition denotes a bool and the body steps at every
   invariant state) ⟹ the loop EXITS at some fuel — i.e. the `h_run` witness `while_rule`
   HYPOTHESIZES is shown to EXIST. Proof shape: strong induction on `(μ st).toNat`. This is the
   REQ-1.2 pattern exactly: dec-validity is the discharge METHOD, the exit witness is the
   semantic CONTENT, and the bridge lemma carries one to the other — never assumed.

   DECLARED ADAPTATION (the `h_dec` bound is the PRE-state `0 ≤ μ st`, NOT the post-state `0 ≤
   μ st'`; #265, kernel-record `lean/Thermite/PinWhileDecShape.lean`, commit 92659eb7). The
   PRE-state conjunct is the strictly WEAKER hypothesis, so the shipped theorem is strictly MORE
   GENERAL than a post-state-bounded one: `μ st' < μ st ∧ 0 ≤ μ st'` forces `0 < μ st ⟹ 0 ≤ μ
   st`, so the pin's `design_hdec_implies_shipped_hdec` derives the PRE-state premise from the
   post-state one, and `loopDenote_exits_of_dec_design_shape` kernel-derives a post-state-bounded
   COROLLARY of this very theorem (the two shapes are NON-equivalent — the pin's
   `shipped_hdec_is_not_the_pinned_shape` exhibits an L1 instance with `μ := -lo` satisfying the
   PRE-state premise while refuting the post-state one). The weaker premise costs a CASE SPLIT on
   `0 < μ st`: at `μ st = 0` a `cond`-true step gives `μ st' < 0`, where `(μ st').toNat = 0` does
   NOT strictly decrease, but `h_dec` at `st'` would then force `0 ≤ μ st'`, contradicting `μ st'
   < 0`, so the next condition MUST be false and the loop exits in two fuel directly (the
   builder's recorded proof structure — the `Exec/WhileBody.lean` `μ st = 0` arm).

KERNEL BAR (the (v-a) gauntlet): `lake build` green; `#print axioms` for every new
definition/theorem/pin within the standard set `{propext, Classical.choice, Quot.sound}` (NO
`sorryAx`, NO new axiom — `Exec/Loop.lean` itself needs only `[propext, Quot.sound]`); EVERY
existing theorem and ALL shipped pins still green; `Thermite.lean` imports the new module.

**A soundness asymmetry worth recording (why there is NO `PinDecMeasure` analogue here).** The
measure `μ` in `loopDenote_exits_of_dec` is an AUXILIARY WITNESS about the FIXED program
(`blockThread lbody`): ANY `μ` satisfying `h_dec` proves exit of a loop that GENUINELY exits — a
mis-denoted `μ` (a `mu_item` encoder reading the wrong cell) CANNOT certify a divergent loop,
because `h_dec` is stated against the real step semantics, not against a `μ`-dependent denotation
of the program. Contrast #226/Pin C, where the registry omission changed the DENOTED PROGRAM
itself. A wrong `mu_item` is therefore a COMPLETENESS bug (over-refusal — `h_dec` unprovable for
the real loop), not a soundness seam; `mu_item`'s faithfulness is an EXP inspection row, not a
pin position.

**(§4.2.3) TERMINATION HONESTY — the DECISION (the REQ-1.2 mirror; resolves REQ-7's two-branch
policy for the export path).** The forcing ground: `loopDenote` is `none` at exhausted fuel, so
for a NON-EXITING loop (a `while true`-shaped body) `whileBodyConverges` is FALSE at every `r` —
the HYPOTHESIZE CONTRACT obligation is then VACUOUSLY provable with a FALSE `ens` (the
termination twin of the §4.1.5 overflow vacuity). Something conjoined MUST fail for that item, or
the Lean-only path silently certifies a non-terminating body. **THE DECISION:** increment (v)
exports a conjoined CONVERGENCE theorem (`<thm>_converges`, §4.2.4) — under `InRangeParams` +
`req`, the whole body CONVERGES (`∃ r, whileBodyConverges …`). This single obligation JOINTLY
discharges the item's OVERFLOW and TERMINATION classes (REQ-1's enum): `whileBodyDenote = some` at
some fuel means NO failure anywhere on the executed path (prefix, every iteration, tail — every 2a
obligation) AND the loop exits. It is discharged via `loopDenote_exits_of_dec` from the per-item
`_pres`/`_progress`/`_dec` obligations — dec-validity DISCHARGED, never assumed: the REQ-1.2
two-path discipline with path (b), the engine-#2 discharge, as the ONLY admitted path on the Lean
route. TWO alternatives are REJECTED, with grounds: (1) "the `dec` residual stays
Verus-discharged" — on `--engine lean` Verus NEVER runs, and on `auto` a Verus attempt that
reached Lean returned ONE item-level `Unknown` (`classify_verus_outcome` is item-granular — there
is NO per-class dec evidence to credit); attributing the TERMINATION class to an engine that
produced no evidence would falsify the REQ-4 trust profile. (2) a PARTIAL-CORRECTNESS-marked L3
(the alternative REQ-7 sketched) — REJECTED for the export path: the partial claim is VACUOUSLY
satisfiable by exactly the non-terminating item it would be stamped on, §6's `Level` has no
partial-aware rung for the honest-min fold to respect, and L3's §6 meaning ("proven for all
inputs") would fork per engine. **The SPINE is UNCHANGED by this decision:** `while_rule` remains
partial correctness with `h_run` a hypothesis (`loop-tv.md` REQ-4 stands — on the per-run TV path
termination remains the Verus `decreases` residual, as shipped); TOTALITY lives in the EXPORTED
CONJUNCTION, where the exit witness is manufactured by `loopDenote_exits_of_dec` from DISCHARGED
obligations. On the VERUS engine nothing changes (Verus discharges loop `decreases` in-item, as
today).

**(§4.2.4) The exporter (v-b) — the emitted obligation set.** Per (v) item, the file contains the
(iv) emissions VERBATIM where they apply — `R_item` + the hard gate (the loop contributes ∅ new
spec-calls per §4.2.1, so the #226 closure is untouched), `stateOf`/`InRangeParams`/the per-param
`rfl` lemmas (§4.1.4), and the `encode_exec_block`/`encode_exec_stmt`/`encode_exec_expr` encoders
REUSED for `prefix_block`/`loop_cond`/`loop_block`/`tail_expr` — plus TWO NEW generator emissions:

- **`Inv_item : Thermite.Env → State → Prop`** — the invariant denotation, FOUR conjunct families
  (the `l1Inv` precedent, which already states the why: "the type-range fact the Verus obligation
  gets for free must be stated"): (1) the USER `inv` clauses denoted SHALLOWLY over cells
  (`execIntValue (st.env.vars "lo") ≤ …` — the Lean mirror of loop-TV's `encode_inv_clauses`);
  (2) per-cell SORT/WIDTH/RANGE facts for every scalar cell the cond/body/invs read (what
  `h_cond_total`/`h_progress` need to decode `asInt`); (3) FRAME conjuncts — each param and
  un-assigned cell equals its `stateOf v` value (what ties the exit state's param reads back to
  the `ens`'s `v.ints x` reads; the loop-TV havoc-cells+frame shape, denoted); (4) SCOPE facts —
  each loop-assigned cell is in scope (what `Stmt.assign`'s scope guard needs for progress).
- **`mu_item : State → Int`** — the loop `dec` clause denoted over cells by the same shallow
  cell-read encoder (faithfulness is the EXP row argued at §4.2.2 — a completeness concern, not a
  soundness seam).

Then FIVE per-item obligation theorems, each realizing a §1 obligation class, each AUTO-attempted
by the (v) loop battery: **`<thm>_entry`** (LOOP-ENTRY: under `InRangeParams` + reqStable,
`∃ st₁, blockThread prefix_block (stateOf v) = some st₁ ∧ Inv_item v st₁` — prefix progress AND
invariant entry); **`<thm>_pres`** (LOOP-PRESERVATION: `while_rule`'s `h_pres`, verbatim, over
`Inv_item v`); **`<thm>_exit`** (LOOP-EXIT→ens: `∀ st, Inv_item v st → condBool loop_cond st =
some false → ∃ r, execDenote tail_expr st.env = some r ∧ Thermite.denote 0 ens (bindResult … r)`
— the loop-TV EXIT obligation's opaque-but-invariant-constrained shape, the tail's own 2a
obligation riding in the `∃ r`); **`<thm>_progress`** (cond-totality + per-iteration body
progress under `Inv ∧ cond`); **`<thm>_dec`** (TERMINATION: strict bounded-below descent of
`mu_item` per genuine step — `loopDenote_exits_of_dec`'s `h_dec`, verbatim). And TWO composed
theorems whose proofs are GENERATOR-FIXED (uniform applications of the (v-a) lemmas to the five —
NOT battery-dependent):

```
theorem <thm> (v : Thermite.Env) :                        -- the HYPOTHESIZE CONTRACT theorem
  InRangeParams v → ∀ (r : Thermite.Exec.BVal),
  Thermite.Exec.whileBodyConverges prefix_block loop_cond loop_block tail_expr
      (stateOf v) (.int r) →
  Thermite.denote 0 req { v with specs := R_item } →
  Thermite.denote 0 ens (Thermite.Exec.bindResult { v with specs := R_item } (.int r))
  -- proof: while_compose (_entry, _pres) + _exit + the functional execDenote (r forced);
  -- a bool result is ∀ (b : Bool), … (.bool b) → … — the §4.1.2 bindBool bridge

theorem <thm>_converges (v : Thermite.Env) :              -- the conjoined CONVERGENCE theorem
  InRangeParams v → Thermite.denote 0 req { v with specs := R_item } →
  ∃ r, Thermite.Exec.whileBodyConverges prefix_block loop_cond loop_block tail_expr
      (stateOf v) r
  -- proof: _entry + loopDenote_exits_of_dec (_pres, _progress, _dec) + _exit's ∃ r
```

A Lean-only while item certifies ONLY when BOTH composed theorems kernel-accept — the REQ-1.1
conjunction at the certificate level, the (iv) `<thm>`/`<thm>_overflow` pair one position wider.
TIER + BATTERY: the contract clause side keeps the §6.1 tiers (a)/(b) — a recursive-registry
clause refuses to the interactive residual exactly as (iv)'s `export_straight_line_body` does
today. The FIVE loop obligations are attempted by a NEW loop battery (the
`exec_body_tactic_battery` sibling): intro the premises; unfold `Inv_item`/`mu_item`/`condBool`/
`blockThread`/`stmtDenote`/`execDenote`/`State.setVar`/`evalArith` AND the `req` denotation
(`hreq` joins the unfold set — the mechanism the iv-b residual (a) lacks, which makes (v) its
named vehicle); close by `first | decide | omega | simp_all | (simp_all; omega)`. EXPECTED
COVERAGE, stated honestly (the iv-b exportable-wider-than-dischargeable precedent): the LINEAR
corpus shapes (the L1 family `while lo < n inv lo <= n dec n - lo { lo = lo + 1 }` — `omega`
closes all five after unfolding) auto-discharge; nonlinear invariants/measures and bodies needing
deeper case splits DEGRADE to `Verdict::Unknown` (fail-to-certify, SOUND — the REQ-3 discipline;
the REQ-7 interactive path is the close). The composed theorems carry no such risk (fixed proofs).

**(§4.2.5) The refusal inventory AFTER (v) — the §4.1.7 narrowing, second round (all STRUCTURED,
never silent; the AST variants quoted).** Stays REFUSED: `Stmt::Loop(LoopNode { kind:
LoopKind::Loop, .. })` (the `loop`-kind — "the corpus `binary_search` uses `loop { if .. {
return .. } }`", the multi-exit CPS shape, per `recognize_v1_loop`); a NESTED `Stmt::Loop`
anywhere in `lbody` or the prefix, including under a `Stmt::If` branch (the loop-TV
`reject_out_of_subset_stmt` recursion); a `while` NOT in last-statement position or inside an
`if` branch ("a loop followed by further straight-line mutation is a v1.1 extension"); more than
one loop; `Stmt::Break` / `Stmt::Continue` ("a loop-control statement; S_B has no loops");
`Stmt::Return(_)` mid-body ("S_B has no early return — the body's result is its tail"); a
non-scalar assign target ("S_B mutates only a bare scalar cell; `xs[i]=e` / `m.field=e` is OUT");
empty `invs` / the trivially-weak `inv true` (the loop-TV reject) / a tail-less body; a
spec-calling or combinator `inv`/`dec` clause (the §4.2.1 v1 residual); an Option/Result result
(`ExportRefusal::OptResResult`, #254). Whether the build mints sibling `ExportRefusal` variants
or richer `LoopBody` detail strings is a build detail; the REQUIREMENT is structured + named,
reported via the `LeanUnverifiable` path — never silent, never an attempt to denote what the
spine does not model.

**(§4.2.6) The kernel pins (named per divergence position; the build/critic authors them — the
§4.1.6 precedent; per R-CHAR-3 this design names WHAT each must cover, never its content).**
- **`PinWhileVacuity.lean` (the termination-vacuity escape — the conjunction oracle).** A
  non-exiting loop (`condBool` constantly `some true`) makes `whileBodyConverges` FALSE at every
  `r`, so a FALSE-`ens` CONTRACT obligation discharges VACUOUSLY — and the pin proves the
  conjoined CONVERGENCE obligation REFUTED (`¬ ∃ r, …`) at the same env. The
  `PinExecOverflowVacuity` shape one vacuity position over (fuel-exhaustion `none` instead of
  overflow `none`); the certificate-level conjunction must keep the vacuous discharge
  UNREACHABLE.
- **`PinWhileComposition.lean` (the composition mis-map).** A `whileBodyDenote` variant that
  SKIPS the loop segment (prefix → tail directly) or iterates the WRONG block certifies a wrong
  `ens` true only at the ENTRY value, where the faithful composition refutes it at the genuine
  EXIT value (the L1 fixture's `lo = 0` entry vs `lo = 3` exit is the natural witness shape).
  Both directions pinned: the poisoned discharge AND the faithful refutation.
The rule-level teeth already EXIST on the spine and are NOT re-pinned: `h_pres` is load-bearing
(`l2_no_preservation_premise_for_buggy_body`) and the exit characterization is exactly
`inv ∧ ¬cond`, never stronger (`l3_exit_overclaim_refuted`).

**(§4.2.7) The REQ-9 delta (mutation accounting).** Today EVERY mutant of a while-body item is
`UntestedAgainstLean` (the Lean fragment admits none) and the item hits the `0/0` backstop — moot,
since the item itself refuses export. After (v): a mutant whose mutated body STAYS in the §4.2.1
grammar is ADMITTED and ATTEMPTED (kill = `Refuted ∪ Unknown-after-attempt`, semantics UNCHANGED —
an operator-flipped cond/inv/body mutant the battery fails or refutes is KILLED); a mutant leaving
the grammar stays `UntestedAgainstLean`, honestly. The accounting delta: `untested` SHRINKS from
"all mutants of every loop item" to "out-of-grammar mutants only", and the floor gate
(`LeanMutationTally::meets_floor` + the `0/0` backstop) begins to GENUINELY gate while items on
the Lean path. No change to the tally semantics, the qualifier, or the #101 non-threading.

**(§4.2.8) Sequencing — the per-increment gauntlet (the #253 iv-a/iv-b precedent).** (v-a) the
spine layer + the two pins land FIRST and ALONE: the §4.2.2 kernel bar (standard axioms, NO
`sorryAx`, every existing theorem + pin green; `Thermite.lean` imports the new module). ONLY THEN
(v-b) the exporter: `cargo test -p forge` + clippy/fmt green; the LIVE lake expectations are
enumerated in the Verification section (the faithful L1-shaped item proves BOTH composed
theorems; wrong-`ens` / non-preserved-`inv` / non-exiting-loop items do NOT certify; every §4.2.5
shape still refuses; the (iv) live tests unperturbed). Fixtures are hand-authored against that
enumeration (R-CHAR-3), never regenerated from the exporter.

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

  **THE INTERACTIVE-PROOF-FILE CONTRACT (REQ-6/REQ-7, the #252 ARCHITECTURAL decision —
  ending the command-injection whack-a-mole).** An interactive proof file supplies a PROOF
  TERM ONLY: the text of the obligation theorem's RHS (everything after the unique `theorem
  thermite_obligation_<item> : <statement> :=`). The replay RECONSTRUCTS a fresh, fully
  generator-controlled file = the canonical exporter preamble + `R_item` + the canonical
  obligation `theorem … : <statement> := <author PROOF TERM>` + the anchored `#print
  axioms`; the author's file content OUTSIDE the proof term is DROPPED (never spliced). There
  are NO file-level helper declarations — auxiliary lemmas inline as `have`/`let`/`suffices`
  INSIDE the proof term (Lean supports this fully in tactic + term mode; no expressivity loss
  for a single-obligation proof). **SOUNDNESS RATIONALE.** The author cannot supply commands
  that share the obligation theorem's ELABORATION scope. The statement is generator-emitted
  and elaborated LEFT of `:=`; the kernel type-checks the author's proof term against that
  fixed, already-elaborated goal type — a proof term cannot vacate the goal (`sorry`/`admit`
  → `sorryAx`, `native_decide` → `ofReduceBool` are caught by the axiom allowlist). **THE
  5-BYPASS HISTORY (the justification).** The earlier design spliced an author HELPERS section
  into the obligation's elaboration scope and tried to SANITIZE it with a command blocklist
  (`disallowed_helper_command`). A blocklist on a Turing-complete elaborator is unsoundable:
  #248 (axiom smuggling) → #249 (axiom-marker mask) → #250 (same-short-name decoy) → #251
  (column-0 command allowlist) → #252 (INDENTED command escapes the column-0 scan — Lean is
  whitespace-insensitive at the top level, so an indented `notation:max "Thermite.stabilizesProp"
  => (fun _ _ => True)` re-elaborates the byte-identical canonical statement to `True`); a
  unicode-whitespace / comment-nesting / `open … in` variant would have followed. Eliminating
  the helper surface ENTIRELY (proof-term-only) makes the surface sound by construction. A
  cheap BELT remains: the extracted proof term is rejected (→ Unknown) if it carries a
  top-level command keyword (`notation`/`macro`/`syntax`/`set_option`/`attribute`/`instance`/
  `open`/`import`/`#…`/…) in any position (exact-token, whitespace-independent) — defense
  against an `… in`-style command form. The #250 duplicate-declaration check, the #249/#250
  axiom-report anchor, the `statements_match` binding, and the kernel type-check all STAY.
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
increment (ii) lands the Lean REGISTRY-TERMINATION discharge. The item-loop termination policy for
the EXPORT path is now DECIDED in §4.2.3 (increment (v), #264): the while-body export mints NO
partial-correctness L3 — the conjoined `_converges` obligation (via the `loopDenote_exits_of_dec`
bridge) discharges the TERMINATION class, or the item degrades to Unknown.**

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
population, AND the REGISTRY-TERMINATION class on `R_item`'s spec-fns, #215), covering the
PURE-CONTRACT class AND — SHIPPED, #253 (the §4.1 exec-body bridge) — STRAIGHT-LINE `S_B` bodies
(loops stay OUT via the STRUCTURED `ExportRefusal::LoopBody`; Option/Result-typed RESULTS stay
OUT behind #254 via `ExportRefusal::OptResResult`) and EXP
registry-body faithfulness. The exec-body AUTO battery carries two COVERAGE residuals
(`req`-bounded overflow; nested-`ifElse` `restoreScope` — both DEGRADE to Unknown, SOUND): see
REQ-10's Known-limitations note. For
AUTO discharge specifically, the IN set NARROWS to the z3-demotion-reachable scalar/QF-linear core
exported FUEL-FREE (§6 tiers (a)/(b): specCall-free goals via fuel-irrelevance, or non-recursive
registries via static unfolding, #216); the recursive-registry `∃N∀fuel` obligations (§6 tier (c))
need INTERACTIVE proofs (or stay on Verus).

**OUT.** User ADTs in match-position (the Lean `Variant` has only the 4 built-in Option/Result
variants — `rust-lean-correspondence.md` D6/the user-variant residual); the v1 `while`-BODY export
(DESIGNED — §4.2/REQ-11, increment (v), NOT-STARTED behind blocker #264; until it ships a v1
while body refuses via the SAME structured `ExportRefusal::LoopBody` as the post-v1 shapes);
post-v1 loops (`loop`/
`break`/`continue`/multi-exit early return/nested loops/non-scalar mutation `xs[i]=e` — all honest
`Unsupported` in the encoders + absent from the Lean inductives; on the exec-body export path the
STRUCTURED `ExportRefusal::LoopBody`, §4.1.7/#253); Option/Result-typed RESULT bodies on the
exec-body export path (`ExportRefusal::OptResResult` — #254, `ExecVal` has no optres variant; a
recorded scope boundary of the loop-residual lineage, refused STRUCTURALLY, never silently);
open body holes (`?N` —
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
  disagreement alarm** — the per-obligation `{engine, trust_profile}` attribution (REQ-4), the
  Proven⊕Refuted halt (REQ-5), the honest mutation v1 (the `Refuted ∪ Unknown-after-attempt` kill +
  the untested-against-lean count, REQ-9), the interactive proof-artifact mode (skeleton emit +
  staleness gate + sorry detection + replay, REQ-7), AND the `forge check --engine verus|lean|auto`
  surface (OQ-1). **SHIPPED (#247, ref #203):** `engine::{EngineAttribution, attribution_for,
  Disagreement, check_disagreement, interactive_proof_path, replay_interactive, proof_has_sorry,
  trust_profile_interactive, LeanMutantOutcome, lean_mutant_outcome, LeanMutationTally}` +
  `manifest::Certificate::engine_attribution` + `check::{EngineSelection, check_file_with_engine,
  lean_mutation_score}` + the `cli.rs` `--engine` flag + `ForgeError::SoundnessAlarm`. The Lean-path
  REGISTRY-TERMINATION well-foundedness discharge + the cache hit-skips-replay optimization remain
  the residual future work. The §6 tier-(c) `∃N∀fuel` interactive path is the `replay_interactive`
  artifact mode (the exporter already emits the tier-(c) skeleton).
- **(iv) The full exportable fragment — FIRST the exec-body bridge (§4.1, DESIGNED in this
  amendment)** — the `bindBool` spine layer (landed FIRST, kernel-green), then the
  `stateOf`/`InRangeParams` emission, the `bodyConverges` HYPOTHESIZE theorem + the conjoined
  OVERFLOW export, the `encode_stmt`/`encode_block`/`encode_exec_expr` arm-by-arm EXP extension (a
  new `rust-lean-correspondence.md` arms-table section for the `Thermite.Exec` constructors + the
  drift tripwire), the four §4.1.6 pins, and the loop-refusal narrowing. **FILED: blocker #253.**
  The residuals AFTER #253: v1 while — now DESIGNED as increment (v) (§4.2, FILED #264);
  spec-fns-in-exec; Option/Result results (#254). **FUTURE (the remaining residuals).**
- **(v) The while-body widening — the §4.2 composition onto the proven loop brick** — (v-a) the
  spine layer FIRST (`whileBodyDenote`/`whileBodyConverges` + `loopDenote_fuel_mono`/
  `whileBodyConverges_unique` + `while_compose` + `loopDenote_exits_of_dec` + the
  `PinWhileVacuity`/`PinWhileComposition` pins), kernel-green with the standard axiom set, every
  existing theorem + pin still green (the iv-a gauntlet); (v-b) THEN the exporter (the
  `recognize_v1_loop`-mirroring recognizer, the `Inv_item`/`mu_item` emission, the FIVE per-item
  obligations + TWO generator-proved composed theorems, the loop auto battery, the refusal
  narrowing). The §4.2.3 termination decision (the conjoined `_converges`; no partial-marked L3)
  is part of the build's acceptance. **FILED: blocker #264.**

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
- **(iv), #253 (the exec-body bridge):** the `bindBool` spine layer lands FIRST and kernel-checks
  with the standard axiom set (NO `sorryAx`), every existing pin still compiling. A straight-line
  item (the `Exec/Stmt.lean` B1/B2/B3 shapes — let-chain, ordered mutation, taken branch) exports
  and kernel-accepts BOTH the CONTRACT (HYPOTHESIZE) and the OVERFLOW theorems; the SAME body with
  a WRONG `ens` is refuted/unprovable. The four §4.1.6 pins are authored kernel-checked: the
  signed/truncating value mis-bridge, the dropped bool bind, the overflow-vacuity escape (its
  vacuous CONTRACT discharge UNREACHABLE as a certificate — the conjunction gate), and the
  env→State mis-map each diverge from the faithful form at a witness. The per-param `rfl`
  correspondence lemmas compile (a mis-map fails the build). A loop body refuses with the
  STRUCTURED loop refusal (never silent, never denoted); an Option/Result-result item refuses
  behind #254. The iv-b audit's two auto-battery residuals (a `req`-bounded OVERFLOW conjunct; a
  nested-`ifElse` `restoreScope` denotation) DEGRADE to Unknown — fail-to-certify, never a false
  Proven (REQ-10's Known-limitations note). The Lean-only path certifies a straight-line item
  ONLY with BOTH theorems
  discharged (the REQ-1.1 conjunction at the certificate level). BEYOND #253: the
  fragment-coverage tests over the full frozen subset, with the OUT set honestly Skipped.
- **(v), #264 (the while-body widening):** the (v-a) spine layer lands FIRST and kernel-checks
  with the standard axiom set (NO `sorryAx`), every existing theorem + pin still compiling (the
  iv-a gauntlet). Then (v-b): an L1-shaped faithful while item (`while lo < n inv lo <= n dec
  n - lo { lo = lo + 1 }` behind a straight-line prefix + tail) exports and kernel-accepts BOTH
  composed theorems (CONTRACT + `_converges`) with all FIVE per-item obligations auto-discharged;
  the SAME item with a WRONG `ens` is NOT Proven; a NON-PRESERVED invariant fails `_pres` (or
  `_entry`) → Unknown, never Refuted; a `while true`-shaped non-exiting body with a FALSE `ens`
  does NOT certify — its vacuous CONTRACT theorem may kernel-accept but `_converges` FAILS (the
  `PinWhileVacuity` Rust mirror, the conjunction gate); every §4.2.5 refusal shape STILL refuses
  structurally (the `loop`-kind, nested, `break`/`continue`, mid-`return`, non-last-position,
  non-scalar mutation, spec-calling inv, tail-less, optres result); the (iv) straight-line live
  tests are UNPERTURBED. The two (v) pins are authored kernel-checked, each pinning the poisoned
  discharge AND the faithful behavior (R-CHAR-3: fixtures hand-authored against this enumeration,
  never regenerated from the exporter).

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (the Obligation artifact) | SHIPPED (increment (i), #204) | The prover-NEUTRAL artifact is built: `pub struct Obligation { item, class, role, ast_slice, env }` + `pub enum ObligationClass` (the FULL AC-1 union — CONTRACT/EXEC/BODY/LOOP-{entry,preservation,exit}/OVERFLOW/TERMINATION/REGISTRY-TERMINATION) + `pub enum ObligationRole` (CERTIFICATION; the §0.1 meta queries are NOT minted, OQ-5 seam) + `pub struct ObligationEnv`/`ObligationParam` carrying AST nodes + Thermite `Type`s + coercion flags (NO Verus strings), in `forge/src/obligation.rs`. Non-test consumer: `check::mint_item_obligations` mints the per-item set on the live L3 path; `engine::VerusEngine` consumes `&Obligation`. The artifact is a prover-neutral `Clone + Eq` VALUE (the `thermite-syntax` AST does not derive serde in production — adding it is outside the #204 manifest; wire serialization is increment (ii) when the Lean exporter serializes a Lean theorem string, not the raw AST). **REQ-1.2 (REGISTRY-TERMINATION) — SHIPPED (class assignment + the CORRECTED full-expression-position closure):** `ObligationClass::RegistryTermination` is minted (`Obligation::registry_termination`) for an item whose `check::reachable_spec_fn_names_full` (seed `req ∪ ens ∪ body ∪ dec(item)`, closure-step over each reached spec-fn's `body ∪ dec` — the #226 fix; `reachable_spec_fn_names_full_spec` is the spec-fn analogue) is non-empty; the forge-side closure mirror NOW walks the dec measures (the body-only omission of `reachable_spec_fn_deps` is corrected for the obligation closure). The Verus-path discharge is REQ-1.2(a) (Verus's dec-check on the woven sub-program); the Lean-path well-foundedness discharge + the per-item CONJUNCTION at the certificate level (REQ-1.1) + the exec-body bridge (§4.1) remain NOT-STARTED (increment (ii)/(iii)/(iv)). For history, the pre-#204 gap was: the content was SHIPPED only as transient Verus text: `pub fn equivalence_obligation` / `exec_equivalence_obligation` / `body_equivalence_obligation` / `loop_{entry,preservation,exit}_obligation` in `thermite-tv/src/obligation.rs` ("`thermite-tv` does NOT run verus itself: it emits the obligation TEXT") + `pub struct ObligationFrame` (the env/typing ctx). The prover-NEUTRAL artifact (AST slice + Thermite types + coercion flags + the `role` discriminator, pre-rendering) is unbuilt — that is the gap. The three §0.1 meta queries are scoped OUT (OQ-5). REQ-1.1 (the per-item CLASS-CONJUNCTION RULE — an item certifies only when EVERY class REQ-1 assigns it is discharged; the degrade ladder applies item-wide; #212(b)) + REQ-1.2 (the REGISTRY-TERMINATION class — for `calledSpecFns(item) ≠ ∅` (the #226 condition completing #224: the FULL-EXPRESSION-POSITION closure — seed `req ∪ ens ∪ body ∪ dec(item)`, step over each reached spec-fn's `body ∪ dec`, i.e. every expression the export denotes against `R_item` INCLUDING the termination measures, transitively — the SAME set the §4 hard gate populates `R_item` with), every spec-fn in `R_item` carries a dec-VALIDITY/well-foundedness obligation, conjoined item-wide; discharged by Verus's dec-check or a Lean well-foundedness proof; closes the #215 divergent-registry bottom-poisoning per Pin B AND the #226 measure-position bottom-poisoning per Pin C `lean/Thermite/PinDecMeasure.lean` — a `dec`-position spec-call omitted from `R_item` bottoms to `0` so a non-well-founded source measure denotes to a fake-descending one (`closure_measure_strictly_descends` vs `true_measure_never_descends`); the extended closure puts the measure-called spec-fn in `R_item` so the descent obligation denotes against the complete registry. The shipped forge closure `reachable_spec_fn_deps`/`collect_block_spec_fn_calls` (`forge/src/check.rs`) has the SAME body-only omission — recorded as a named increment-(i) work item (header build-blockers), load-bearing only for the NEW Lean exporter (the shipped Verus pipeline fails CLOSED on a missing dep)) + the exec-body bridge scoping (§4.1) are stated NORMATIVELY in §1.2/§4/§4.1 but likewise unbuilt — increment (iv) for the bridge, increment (i)/(iii) for the per-item conjunction at the certificate level, increment (ii) for the Lean-path registry-termination discharge. |
| REQ-2 (the Engine interface) | SHIPPED (Verus instance; increment (i), #204) | `pub trait Engine { name, fragment, discharge, trust_profile, evidence_key }` + `pub enum Verdict { Proven(Evidence) \| Refuted(Counterexample) \| Unknown(Reason) }` + `pub struct TrustProfile`/`Fragment`/`CacheKey` + `pub enum EngineName` in `forge/src/engine.rs`. `pub struct VerusEngine` fills all four slots (AC-2): FRAGMENT = the whole frozen subset (`admits_all_classes`, incl. RegistryTermination); DISCHARGE = `VerusEngine::verdict_of` lifting `classify_verus_outcome`'s three-way map to `Verdict` WITH the REQ-3.1 remap; TRUST PROFILE = {Z3, Verus VC-gen, TV/lowering theorem}; EVIDENCE = `engine_cache_key` composing the SHIPPED `cache::cache_key` hex with the engine discriminator (§2(d)). Non-test consumer: `check::ladder_for_timeout` routes the per-item L3 CERTIFICATION discharge through `VerusEngine` (selected via `default_engines`, gated via `fragment().admits`). The Lean engine (`LeanAuto`/`LeanInteractive`) is increment (ii), NOT-STARTED (forward-declared in the cache discriminator). |
| REQ-3 (Unknown degrades / Refuted hard-fails, engine-generic) | SHIPPED (increment (i), #204) | `pub fn engine::verdict_ladder_action` maps an engine `Verdict` (for `role = Certification`) to the SHIPPED `degrade::L3Verdict`: `Proven` → `Proved` (CertifyL3); `Unknown` → `Timeout` (degrade via `run_ladder` → L2/L1); `Refuted` → `Counterexample` (HardFail, never degrades — generalizing `degrade::ladder_action_l3` off the word "verus"). The failure-WITHOUT-witness rule is `engine::counterexample_is_incompleteness_unknown` (the NARROW SMT-`unknown` signature). **REQ-3.1 (the fast-unknown remap) — SHIPPED:** `VerusEngine::verdict_of` splits `VerusOutcome::Counterexample` — ONLY a span-less failure carrying the SMT-`unknown` signal (no frontend `error[E`) → `Unknown(IncompleteUnknown)` (degrade, the SOLE behavioral delta — was a hard fail); a WITNESSED countermodel AND a FRONTEND type error (E0308 — e.g. the provenance `careless_query` un-typeable IFC path) stay `Refuted` (hard-fail → L0, unchanged). The remap is INERT on the conformance corpus: it contains witnessed failures + E0308 type-error rejections (which stay hard-fail) but NOT genuine SMT-`unknown`s, so every `conformance/*.cert.json` is byte-identical. Tests: `engine.rs` (`incompleteness_discriminator_is_narrow`, `type_error_counterexample_stays_refuted`, `witnessed_counterexample_stays_refuted`, `verdict_ladder_action_follows_req3`) + the cert-oracle identity (`forge/tests/engine_interface.rs`, incl. provenance L0). The Lean-re-attempt interaction with a failed spec-fn termination proof (REQ-1.2 Lean discharge) is increment (ii). |
| REQ-4 (certificate attribution — per-obligation engine + trust profile) | SHIPPED (increment (iii), #247) | The per-obligation `{engine, trust_profile}` pair is `engine::EngineAttribution { engine, trust_profile }`, built by `engine::attribution_for(&dyn Engine)`; `manifest::Certificate::engine_attribution: Option<EngineAttribution>` is the ADDITIVE serde field (`#[serde(default, skip_serializing_if = "Option::is_none")]` — the `boundary`/`slag`/`assurance_scope` precedent), set by `Certificate::with_engine_attribution`. Populated ONLY when a NON-DEFAULT engine (Lean) discharges (the default Verus path leaves it `None`), so the cert oracle is BYTE-IDENTICAL: `serde(default)` keeps the goldens green because a corpus Verus cert never gains the field (the conformance oracle test `engine_attribution.rs::engine_verus_flag_is_byte_identical_oracle` asserts `sum`'s golden subset + the OMITTED field). Honest-min aggregation UNCHANGED (`AssuranceManifest::aggregate` untouched — the attribution is per-obligation metadata ORTHOGONAL to `Level`). DECISION (OQ-2): oracle-EXCLUDED (diagnostic-only) so the golden stays byte-identical. Non-test consumer: `check::lean_proven_cert` / `lean_interactive_proven_cert` attach it on the `--engine lean`/`auto` path (`cli::run_check`). The "smaller base" claim is along the named axes (no Z3, no Verus VC-gen — verified by the attribution test); the ordering formalization stays OQ-3. Live-verified: a `forge check --engine lean` of a scalar item emits a cert with `engine_attribution.engine == "lean-auto"` + the `{Lean kernel, propext, Classical.choice, Quot.sound, EXP}` base (`engine_attribution.rs::engine_lean_attaches_smaller_trust_base_live`). |
| REQ-5 (engine disagreement = soundness alarm) | SHIPPED (increment (iii), #247) | `engine::check_disagreement(item, engine_a, verdict_a, engine_b, verdict_b)` is the multi-engine dispatch guard: a `Proven ⊕ witnessed-Refuted` pairing on the SAME obligation returns `Err(engine::Disagreement { proven_engine, refuted_engine, item, counterexample })` — a structured HARD halt naming BOTH engines + the obligation, NEVER resolved by preference. `Proven ⊕ Unknown` (and `Unknown ⊕ anything`, `Refuted ⊕ Refuted`) is benign (`Ok`). Per REQ-3.1 a Verus witness-less fast-`unknown` is `Unknown`, so it can NEVER spuriously fire the alarm against a Lean kernel `Proven` (only a WITNESSED Verus countermodel can). Surfaced as `ForgeError::SoundnessAlarm` (`cli.rs`); the dispatch point is `check::lean_engine_cert`'s `auto` arm (Verus + Lean on the same obligation). Teeth-tested with SYNTHETIC verdicts (`engine.rs::proven_refuted_disagreement_halts` — the alarm fires both orders, naming the right engine per role + carrying the witness; `proven_unknown_is_benign` — `Proven ⊕ Unknown` and `Refuted ⊕ Refuted` do NOT halt). The anti-cheat ANCESTOR (`ladder_action_l3` → `HardFail`) stays SHIPPED. |
| REQ-6 (the Lean exporter) | SHIPPED (the PURE-CONTRACT class, tiers (a)/(b)/(c); increment (ii-b), #240) | The Thermite→Lean obligation EXPORTER is BUILT: `pub fn export_item` (`forge/src/lean_export.rs`) serializes a checked PURE-CONTRACT item into a self-contained Lean file INSTANTIATING the SHIPPED spine (`import Thermite.Stabilize` + the `Ast.lean`/`Denote.lean` encodings). `encode_expr` maps each frozen-subset `Expr` ARM-BY-ARM to its `Ast.lean` constructor (the EXP arms table is `.design/verified/rust-lean-correspondence.md` Table 4 — the drift tripwire); an OUT-of-spine construct (Field/TupleProj/StructLit/Deref/StrLit/Tuple/If/bare-closure/user-ADT-match/non-pure body/open hole) → a structured `ExportRefusal` (an honest skip, NEVER a silent omission). `R_item` is populated by the `req ∪ ens ∪ body ∪ dec` full-expression-position closure (REUSING the `Obligation.env.spec_defs` — the ONE closure, #192) with THE HARD GATE: `build_registry` REFUSES (`ExportRefusal::IncompleteRegistry`) when `calledSpecFns ⊄ dom(R_item)`, AND `export_item` independently RE-CHECKS that every spec-call appearing in the exprs is covered (catching a buggy/omitting closure — the Pin B/C/E/F bottom-poisoning), AND emits the per-name `example : R_item "f" ≠ none := by decide` resolution lemmas (§4 mechanisms 1+2). EXP body-faithfulness: each `R_item` entry binds the spec-fn's REAL `Denote`-encoded body (arm-by-arm). The §4 STABILIZED form (#213) + the result-binding form (#214) are emitted for tier (c) (`stabilizes`/`stabilizesProp`, result bound THROUGH `stabilizes`, `specs := R_item` held fixed); tiers (a)/(b) emit the fuel-free equivalent (§6.1). Non-test consumer: `engine::LeanEngine::{discharge,fragment,evidence_key}` (`forge/src/engine.rs`). LIVE-verified: a CORRECT scalar contract + a tier-(b) item kernel-ACCEPT (`lake env lean`, standard axioms only); a WRONG contract / an omitted registry / an out-of-fragment item are SKIPPED/Unknown (`forge/src/engine.rs` `live_*` tests + `forge/tests/lean_engine.rs`). The exec-body bridge (§4.1, increment (iv)) + the interactive tier-(c) discharge (increment (iii)) remain NOT-STARTED. For history, the pre-#240 status was: FUTURE (increment (ii)/(iv)). The TARGET is SHIPPED: `lean/Thermite/` mechanizes `S` (`denote`/`refDenote`/`Denote.lean`, `execDenote`/`Exec.lean`, `bodyDenote`/`Exec/Stmt.lean`, `loopDenote`+`while_rule`/`Exec/Loop.lean`) over the `Expr`/`Block` inductives, kernel-checked (axioms `{propext, Classical.choice, Quot.sound}`). Critically (#213, the critic's kernel-checked pin `lean/Thermite/PinIntBottom.lean`): `intVal` bottoms an INT-position `specCall` to `0` (`| none => 0` + fuel-0 catch-all `| _, _, _ => 0`), NOT to `True` — so the cycle-2 `∀ fuel ≥ fuel₀` form is FALSE for correct items (the pin's `obligation_form_is_false`) and is RETIRED. §4 RESTATES the obligation against a STABILIZATION relation (`stabilizes : Expr → Env → Int → Prop := ∃ N, ∀ fuel ≥ N, intVal fuel e env = v`, + the Prop analogue for `denote`): `∀ r, stabilizes body_expr env r → reqStable(env) → ensStable(env at r)`, per-env ∃-N (no global `fuel₀`, fixing the value-dependent-depth counterexample), with the RESULT value BOUND THROUGH stabilization (the #214 fix — Pin A's `wrong_contract_certifies_with_underfuelled_rbody` is now UNPROVABLE because uniqueness of stabilization forces `r` to the body's true value, `wrong_contract_fails_at_true_value`), `specs := R_item` held fixed + the export-time HARD GATE (refuse-to-emit + per-name `decide` lemmas) when `calledSpecFns(item) ⊄ dom(R_item)`, where `calledSpecFns(item)` is (the #226 fix completing #224) the FULL-EXPRESSION-POSITION closure — every spec-fn reachable from `req ∪ ens ∪ body ∪ dec(item)` TRANSITIVELY, closure-step over each reached spec-fn's `body ∪ dec` (NOT `req ∪ ens` only — the cycle-2 scope; nor `req ∪ ens ∪ body` only — the cycle-5 body-only scope: a `dec`-VALIDITY obligation denotes the measure against `R_item`, and an omitted measure-called spec-fn bottoms to the Int-bottom `0` so a non-well-founded source measure denotes to a fake-descending one — the critic's pin `lean/Thermite/PinDecMeasure.lean`'s `closure_measure_strictly_descends` vs `true_measure_never_descends`; likewise an omitted body-called spec-fn stabilizes to `0`, uniqueness forces `r = 0`, and `ens: result == 0` certifies kernel-clean — `lean/Thermite/PinBodyRegistry.lean`'s `wrong_contract_certifies_under_body_omission`, REFUTED with the full registry by `wrong_contract_fails_with_full_registry`) — no resolution PREMISE. SCOPED to the PURE-CONTRACT class (§4.1: the exec-body S_C×S_E/S_B bridge — value bridge, bool sort, optres, env→State — is increment (iv)'s own design obligation). The SPINE PREREQUISITES are SHIPPED (`lean/Thermite/Stabilize.lean`, #240/#241): `stabilizes` + uniqueness-of-stabilization (#214) + the FUEL-IRRELEVANCE lemma (#216) + `stabilization_exists_for_dec_bounded` with GENUINE content (#241: `RegistryTerminating := ∃ v, Converges` over the bottom-distinguishing `intValNB`/`denoteNB`, the AGREEMENT LEMMA `converges_imp_stabilizes`, the divergent registry FAILING the hypothesis per the re-pinned Pin E), all kernel-checked `{propext, Classical.choice, Quot.sound}`. STILL NOT-STARTED (the REQ-6 exporter itself): the Rust→Lean exporter that emits source instantiating those (with EXP = arm-by-arm + drift-tripwire + registry-body faithfulness), and the REGISTRY-TERMINATION Lean-path well-foundedness discharge that PROVES a dec-valid `R_item` supplies a `Converges` witness (REQ-1.2(b), #215) — the z3-demotion doc names the exporter "the #185-adjacent correspondence-bridge work … NOT built." |
| REQ-7 (Lean discharge modes + termination) | SHIPPED (AUTO tiers (a)/(b); tier (c) interactive-marked; increment (ii-b), #240) — interactive DISCHARGE + termination wiring NOT-STARTED | The THREE-tier export story (§6.1, #216) is BUILT: `lean_export::tier_of` classifies an obligation by registry shape — `ExportTier::FuelFreeAuto` (all of `req`/`ens`/body/dec specCall-free → the fuel-free `denote 0`/`intVal 0` form, sound via `stabilizes_iff_intVal_zero`/`stabilizesProp_iff_denote_zero`), `ExportTier::StaticUnfoldAuto` (a NON-recursive registry → `unfold_spec_calls` STATICALLY UNFOLDS every spec-call to its finite DAG depth into a specCall-free `Expr`, then the fuel-free form — `registry_is_recursive` is the DFS cycle check), and `ExportTier::RecursiveInteractive` (a recursive registry → the `∃N∀fuel` stabilized form, marked INTERACTIVE). The AUTO discharge is the LIVE `LeanEngine::discharge`: export → write scratch → `lake env lean` (cwd `lean/`, lake located via PATH/`~/.elan/bin`) → kernel accept = `Verdict::Proven`, tactic failure/timeout/lake-absent = `Verdict::Unknown` NEVER `Refuted` (a Lean tactic failure is not a witnessed countermodel — REQ-3 anti-cheat); the auto tactic battery is `first | decide | omega | simp_all | …` (the z3-demotion-grounded shallow-QF shape). Tier (c) returns `Unknown("interactive-only")` WITHOUT invoking lake (the file IS still emitted for increment-(iii) use, marked). LIVE-verified kernel-clean: the tier-(a)/(b) Proven obligations carry `#print axioms = {propext, Classical.choice, Quot.sound}` (the z3-demotion bar). What remains NOT-STARTED for REQ-7: the Lean-path REGISTRY-TERMINATION well-foundedness DISCHARGE (REQ-1.2(b), #215) — that piece is increment (ii)/(iv). **The INTERACTIVE proof-artifact mode is SHIPPED (increment (iii), #247):** `engine::interactive_proof_path(source_file, item)` is the deterministic `<file>.lean-proofs/<item>.lean` artifact path; `engine::LeanEngine::replay_interactive(source_file, &Obligation)` EMITS the skeleton (the exporter's source + the `-- evidence_key: <hex>` header, `engine::INTERACTIVE_EVIDENCE_KEY_MARKER`) when ABSENT, and REPLAYS a PRESENT proof via `lake env lean` with the obligation-hash STALENESS gate (the header's evidence key must match the current `evidence_key`; a mismatch → `Unknown("stale proof — re-derive")`, NEVER silently reused) + EXPLICIT sorry detection (`engine::proof_has_sorry` scans the SOURCE token AND a `#print axioms` `sorryAx` — lake exits 0 on a `sorry`, so a `sorry` is NEVER `Proven`); a kernel-accepted, sorry-FREE replay → `Proven` with the INTERACTIVE trust profile (`engine::trust_profile_interactive` — adds the reviewed proof author, OQ-4). **TWO further replay gates are SHIPPED (the #248 fix, R-DEFER-9):** (1) the TRUST-BASE AXIOM ALLOWLIST — after a clean lake exit, `engine::nonstandard_axiom` STRICTLY parses the `#print axioms thermite_obligation_<item>` REPORT line ("depends on axioms: [...]", anchored on the marker so a lake `simp only [...]` warning bracket can NEVER be mistaken for the axiom list) and REJECTS any axiom outside `{propext, Classical.choice, Quot.sound}` → `Unknown("non-standard axiom: <name>")`, NEVER `Proven` (a smuggled `axiom thermite_cheat : ∀ p, p` makes the enumerable trusted base a LIE — REQ-4/§1); (2) STATEMENT BINDING — `engine::canonical_theorem_statement` regenerates the obligation's theorem STATEMENT from the exporter's `exported.source` (the author fills ONLY the proof term after `:=`/`by`) and requires the present file's `theorem thermite_obligation_<item> : <statement> :=` span to match EXACTLY modulo whitespace (`engine::statements_match`); a file proving a DIFFERENT statement (e.g. `: True`) → `Unknown("statement mismatch")`, NEVER `Proven` (REQ-6 — the proof must PROVE THE OBLIGATION). **#252 ARCHITECTURAL FIX — THE HELPER-SURFACE ELIMINATION (ending a 5-bypass whack-a-mole #248..#252):** `engine::reconstruct_replay` no longer splices any author HELPERS section. The reconstructed replay file is EXACTLY the canonical exporter preamble + `R_item` + the canonical `theorem thermite_obligation_<item> : <statement> := <author PROOF TERM>` + the anchored `#print axioms`; the ONLY author-controlled text is the PROOF TERM (after the obligation theorem's first `:=`), and any author file content OUTSIDE it is DROPPED (it has nowhere to live, so it can never share the obligation's elaboration scope). The earlier blocklist sanitizer (`engine::disallowed_helper_command` + `engine::author_helpers`) — unsoundable on a Turing-complete elaborator: #251 closed column-0 commands, #252 escaped via INDENTATION (Lean is whitespace-insensitive at the top level, so an indented `notation:max "Thermite.stabilizesProp" => (fun _ _ => True)` re-elaborated the byte-identical canonical statement to `True`) — is DELETED. Auxiliary lemmas inline as `have`/`let`/`suffices` INSIDE the proof term (no expressivity loss for a single-obligation proof; the soundness rationale: the statement is generator-emitted + elaborated left of `:=`, the kernel type-checks the proof term against that fixed goal, so a proof term cannot vacate it; `sorry`/`admit`→`sorryAx`, `native_decide`→`ofReduceBool` caught by the axiom allowlist). A cheap BELT (`engine::proof_term_command_token`) REJECTS (→ Unknown) a proof term carrying a top-level command keyword (`notation`/`macro`/`macro_rules`/`syntax`/`elab`/`set_option`/`attribute`/`instance`/`open`/`export`/`import`/`namespace`/`initialize`/`#…`) in ANY position (exact-token, whitespace-independent — catches an `… in`-style command form). The #250 duplicate-declaration reject + the #249/#250 axiom-report anchor + `statements_match` + the kernel type-check all STAY. Non-test consumer: `check::lean_engine_cert`'s `--engine lean` non-auto path (`lean_interactive_proven_cert` attaches the interactive attribution). Tested: the #248 divergence PIN (`forge/tests/divergence_axiom_smuggling.rs::divergence_interactive_replay_accepts_nonstandard_axiom` — the cheat-axiom proof is NOT L3, LIVE) + the statement-mismatch reject (`engine.rs::interactive_statement_mismatch_is_unknown_never_proven`) + the strict allowlist parser (`nonstandard_axiom_parses_the_report_line_strictly`) + the canonical-statement extractor (`canonical_statement_extraction_and_whitespace_match`); the legit-path NON-regression (`interactive_filled_valid_proof_replays_proven` — a genuine kernel-accepted, allowlist-clean, statement-bound proof STILL replays Proven). The #252 helper-surface elimination is verified by: the author HELPER section is DROPPED (the #251 column-0 + the #252 INDENTED `notation` poison both vanish) and the BELT rejects an `open … in` proof term (`engine.rs::reconstruct_drops_author_helper_section`); the belt scan is position-independent (`engine.rs::proof_term_command_token_scans_position_independently`); a clean INLINE-have proof replays Proven LIVE + a `sorry`-bearing inline proof → Unknown (`engine.rs::interactive_inline_have_clean_proven_sorry_unknown`); the #252 pin `forge/tests/divergence_252_indented_command_escape.rs` (the indented-`notation` poison does NOT certify L3, LIVE). Also: skeleton emitted (`engine.rs::interactive_skeleton_emitted_when_absent`); a fresh-key sorry-free kernel-accepted proof REPLAYS Proven LIVE (`interactive_filled_valid_proof_replays_proven`); a stale hash → Unknown (`interactive_stale_hash_is_unknown_never_reused`); a sorry-carrying file → Unknown LIVE (`interactive_sorry_file_is_unknown_never_proven`); the sorry detector + the path (`sorry_detected_in_source_or_axioms`, `interactive_proof_path_is_beside_source`). The evidence-key cache STORE/LOAD round-trip (a hit-skips-replay optimization, distinct from the staleness REPLAY which IS shipped) and the Lean-path REGISTRY-TERMINATION discharge remain future. For history, the pre-#240 status was: FUTURE (increment (ii)/(iii)). The AUTO fragment is PROVEN-REACHABLE: `z3-demotion.md` shows `tv_obligation_arith_cmp`/`tv_obligation_or_le` (scalar/QF-linear contract clauses) discharged by Lean-SMT's `smt` tactic, kernel-clean (`#print axioms` = standard set only; no `sorryAx`/cvc5 oracle) — and these are SHALLOW QF goals with NO `denote`/`stabilizesProp` wrapper. §6.1 reconciles the deep-embedded §4 form to that grounding via the THREE-TIER export story (#216): (a) FUEL-FREE export for specCall-free obligations via the `intVal_fuel_irrelevant`/`denote_fuel_irrelevant` lemma (`stabilizesProp e env ↔ denote 0 e env` for specCall-free `e`) — the auto fragment's actual fuel-free shallow shape, matching the PoC; (b) STATIC UNFOLDING of non-recursive registries to finite depth, again yielding fuel-free goals; (c) the `∃N∀fuel` stabilization form reserved for RECURSIVE registries on the INTERACTIVE path only (the per-env `∃N` witness needs induction). The interactive/proof-artifact mode (staleness = the §2(d) EVIDENCE KEY changing: obligation + engine + engine-toolchain version + targeted-spine content hash) + the `dec`/partial-correctness termination policy (tied to the SHIPPED `while_rule` `h_run` premise) are unbuilt. The REGISTRY-TERMINATION termination tier's SEMANTIC currency is SHIPPED on the spine (#241: `RegistryTerminating := ∃ v, Converges` + the AGREEMENT LEMMA + the divergent registry FAILING it, `lean/Thermite/Stabilize.lean`); what remains unbuilt for REQ-7 is the engine wiring — the Lean-path well-foundedness DISCHARGE that proves a dec-valid `R_item` supplies the `Converges` witness (REQ-1.2(b), #215) and the auto/interactive battery that consumes it. |
| REQ-8 (engine ordering + ladder placement) | SHIPPED (Verus rung #204; the `--engine` surface + the Lean fallback ordering #247) | `pub fn engine::default_engines` returns the ordered engine list (Verus first); `check::ladder_for_timeout` reads the first rung (Verus) before the SHIPPED L2/L1 degrade. **The `--engine verus|lean|auto` SURFACE is SHIPPED (increment (iii), #247, OQ-1 DECISION):** `check::EngineSelection { Verus, Lean, Auto }` + the `forge check --engine <e>` flag (`cli.rs`); `check::check_file_with_engine` runs the §6 ordering — `verus` (default, byte-identical), `lean` (LeanEngine ONLY: exportable items discharged by Lean with attribution; non-exportable → an honest `LeanUnverifiable` L0 skip via `lean_unverifiable_cert`), `auto` (Verus first; on a Verus inconclusive verdict TRY Lean, upgrading to L3-via-Lean on a Lean `Proven` — `lean_engine_cert`'s `Auto` arm). The OQ-1 DECISION is recorded in the OQ-1 entry below. The per-engine SKIP/Unknown accounting (the cert's `LeanUnverifiable` reject detail + the REQ-9 untested-against-lean qualifier) is reported. The `#[engine(lean)]` per-item annotation stays the OQ-1 deferred alternative (the `--engine` whole-file form is the v1 surface). The Lean-interactive rung is REQ-7's `replay_interactive` (on demand, not automatic). |
| REQ-9 (engine-generic anti-Goodhart battery, honest v1) | SHIPPED (the Lean path; increment (iii), #247) | The Verus-path battery (`check::mutation_score`) is UNTOUCHED (the SHIPPED `Counterexample ∪ Timeout` = killed + the #101 `equivalence_proves_equal` exclusion). **The engine-generic LEAN PATH is SHIPPED:** `engine::lean_mutant_outcome(admitted, &Verdict)` classifies a Lean-engine mutant — an ADMITTED mutant Lean does not prove (`Refuted ∪ Unknown-after-attempt`) is `Killed` (= the shipped `Counterexample ∪ Timeout`), an admitted `Proven` mutant `Survived`, a mutant the Lean fragment does NOT admit (out-of-spine / tier-(c)) is `UntestedAgainstLean` (NEVER counted killed — never inflates the ratio, §7 / R-DEFER-9). `engine::LeanMutationTally` accumulates `killed / attempted` (= attempted MINUS proven-equivalent — the SHIPPED `scored` denominator) + `untested` (reported in the cert, OUTSIDE the ratio) + `equivalent` (the #101 exclusion drops a proven-equivalent survivor from BOTH the survivor set AND the denominator); `qualifier()` is the floor-guard-1 line ("K/A killed … N untested against lean"). The #101 equivalence probe is a §0.1 verus META-query OUTSIDE the Engine interface in v1 (F3/OQ-5 — not threaded on the Lean-only path, so the Lean tally reports the RAW survivor set honestly). Non-test consumer: `check::lean_mutation_score` builds a per-mutant LeanEngine over the swapped-in mutant program (`program_with_mutant`) — the LeanEngine resolves an obligation's item by NAME from its program, so a mutant must be swapped in, else every mutant would re-export the unchanged original (a false survivor); the tally is attached to the Lean-proven cert via `lean_proven_cert`. Live-verified: an `add`-shaped item's operator-flip mutant is KILLED and its non-pure-tail early-return mutant is UNTESTED-against-lean (`1/1 killed; 1 untested`). Tested: the kill semantics (`engine.rs::lean_mutant_outcome_follows_req9`) + the no-inflation tally (`lean_mutation_tally_does_not_inflate_on_untested`). The floor 0/0 backstop is `kill_ratio() = 0.0` on an empty denominator. **The floor GATES the Lean path (the #248 fix), it does NOT merely report:** `LeanMutationTally::{meets_floor, mutants_killed_string, survivor_detail}` + `check::lean_proven_cert` REJECT the item `WeakContract`-style (via `Certificate::rejected_weak_contract`) when the kill-ratio is below the threaded `mutation_floor` OR the `0/0` backstop fires (mutants generated, all untested-against-lean) — mirroring the Verus path's `mutation_score → meets_floor` gate, never a silent L3. The Lean denominator = `attempted` with NO #101 equivalence exclusion (a §0.1 verus meta-query OUTSIDE the Lean-only path, F3/OQ-5) — stated honestly in the reject detail. A `spec fn` (no `ens`) skips the gate (nothing to mutate). Tested: `engine.rs::lean_tally_floor_gate` (the 1/1 pass, the 0/0 backstop, the 1/3 below-floor reject). |
| REQ-10 (the exec-body bridge — straight-line bodies, §4.1) | SHIPPED (increment (iv), #253 — iv-a the spine, iv-b the exporter; ref #203) | **REQ-10.2 SHIPPED (iv-a)** — the `bindBool` spine layer: the DEFAULTED `Env.bools : String → Bool := fun _ => false` field + `Env.bindBool` (`lean/Thermite/Denote.lean`) + the `Expr.boolVar` leaf (`lean/Thermite/Ast.lean`) with its arms across `denote`/`refDenote`/`intVal`/`seqVal` (`Denote.lean`/`RefEncode.lean`), `ref_sound`/`refVal_eq` (`Soundness.lean`), and `specCallFree`/`intVal_fuel_irrelevant`/`seqVal_fuel_irrelevant`/`denote_fuel_irrelevant`/`denoteNB` (the EXPLICIT `some (env.bools x = true)` arm)/the `*_agrees`/`*_specCallFree` arms (`Stabilize.lean`); non-test consumers: `bindResult` + the four bridge pins + the exporter's bool-result path. **REQ-10.1/REQ-10.4 SHIPPED** — spine: `abbrev bodyConverges (b : Block) (st : State) (r : ExecVal) : Prop := bodyDenote b st = some r` (over the FUEL-FREE, genuine-`Option` `bodyDenote` — NO new NB layer, the §4.1.5 decision) + the value bridge `def bindResult` (int → `Env.bindInt env "result" r.value`, the IDENTITY on `BVal.value`; bool → `Env.bindBool`) in `lean/Thermite/Exec/Stmt.lean`; exporter: `emit_body_theorems` (`forge/src/lean_export.rs`) emits BOTH the HYPOTHESIZE CONTRACT theorem (the result bound THROUGH `bodyConverges`) AND the conjoined OVERFLOW theorem (`(bodyDenote body_block (stateOf v)).isSome`) in ONE file — the §4.1.5 conjunction rule. **REQ-10.3 SHIPPED (iv-b)** — `emit_state_of` (int param → `.int ⟨uW, v.ints x⟩`, bool → `.bool (v.bools p)`, slice → `(v.seqs xs).map (⟨uW, ·⟩)`; `scope := fun _ => false`) + the `InRangeParams` typed-input premise + the per-param `rfl` correspondence lemmas (the §4.1.4 compile-time tripwire). The `scope := false` EXP row is VERIFIED faithful against `thermite-tv::exec_stmt_encode::body_ref_state` (its initial `Env` is EMPTY — a param is a free input, a body `assign` to a param is `Err`/`none` on BOTH sides). **REQ-10.5 SHIPPED (iv-a)** — the four bridge divergence pins `PinExecValueBridge.lean`/`PinExecBoolBind.lean`/`PinExecOverflowVacuity.lean`/`PinExecStateMisMap.lean` (`lean/Thermite/`), each kernel-checked with `{propext, Classical.choice, Quot.sound}` (NO `sorryAx`), each pinning BOTH the poisoned discharge AND the faithful refutation. **REQ-10.6 SHIPPED (iv-b)** — `encode_exec_stmt` returns the STRUCTURED `ExportRefusal::LoopBody` for a `loop`/`while`/`break`/`continue`/mid-body-`return`/non-scalar-mutation body (§4.1.7, narrowed out of `NotPureContract`); an Option/Result-typed RESULT is `ExportRefusal::OptResResult` (#254 — `inductive ExecVal where | int (b : BVal) | bool (b : Bool)`, no optres variant to bridge); `LeanEngine::discharge` maps both to `Unknown` (an honest skip, never silent). Non-test consumer (the whole bridge): `engine::LeanEngine::discharge` → `export_item` → `export_straight_line_body` (`forge/src/lean_export.rs`) on the live `--engine lean` path. Verification: `lake build` green (381 jobs, the standard axiom set, every prior theorem + pin still compiling — the iv-a commit record); LIVE lake tests in `forge/src/engine.rs`: `live_straight_line_body_is_proven` (both theorems incl. the OVERFLOW conjunct), `live_bool_result_body_is_proven_via_bindbool`, `live_always_overflow_body_is_not_proven` (the `PinExecOverflowVacuity` Rust mirror — the vacuous CONTRACT never certifies), `while_body_item_refuses_export`, `optres_result_item_refuses_export`. **KNOWN LIMITATIONS (increment (iv) auto-discharge — coverage residuals, the iv-b audit; see the REQ-10 Known-limitations note):** the EXPORTABLE surface is WIDER than the auto-DISCHARGEABLE surface — (a) a `req`-bounded `_overflow` conjunct (e.g. `req a <= 100`, body `a + a`) and (b) a nested-`ifElse` body's `restoreScope` denotation both fail the shipped battery and DEGRADE to `Verdict::Unknown` (fail-to-certify — SOUND, conservative incompleteness, NEVER a false `Proven`); the close is a stronger auto battery or the REQ-7 INTERACTIVE path — a residual, NOT an open blocker (increment (v)'s loop battery, §4.2.4/#264, threads `hreq` through its unfold sets and is the named vehicle for residual (a)). RESIDUALS after #253 (recorded scope boundaries, the loop-residual lineage — refused STRUCTURALLY, never silently): Option/Result-typed results (#254), v1 while (the `loopDenote` composition — now DESIGNED as §4.2/REQ-11, increment (v), #264), spec-fns-in-exec. For history, the pre-#253 row recorded NO bridge code (every statement body `ExportRefusal::NotPureContract`, every non-int result `NonIntResult`, no `Env` bool sort) over the SHIPPED substrates (`bodyDenote`/`stmtDenote`/`blockThread`/`State`/`State.restoreScope` in `lean/Thermite/Exec/Stmt.lean`; `ExecVal`/`BVal`/`BVal.value`/`execDenote` in `lean/Thermite/Exec.lean`; the §4.1 conjunction rule + HYPOTHESIZE resolution) — superseded by this build. |
| REQ-11 (the while-body widening — v1 `while` bodies, §4.2) | SHIPPED (increment (v) — v-a the spine #264/commit 99c7f304 + the #265 adaptation 92659eb7; v-b the exporter #264) | The WHILE-BODY COMPOSITION LAYER (v-a) is built and kernel-green AND the EXPORTER (v-b) is SHIPPED. Mirrors the same doc's Requirements-section REQ-11 entry (post-v-b). **REQ-11.1 SHIPPED (v-a)** — ALL in `lean/Thermite/Exec/WhileBody.lean` (composed AROUND the UNCHANGED `Exec/Stmt.lean` + `Exec/Loop.lean`): `def whileBodyDenote (prefixB cond lbody tail fuel st)` (the `Option`-monad composition prefix `blockThread` → the SHIPPED `loopDenote` → tail `execDenote`; `prefixB` is a Lean-keyword SYNTAX adaptation of the design's `prefix` binder, semantics unchanged) + `abbrev whileBodyConverges … : Prop := ∃ fuel, whileBodyDenote … = some r` (the ∃-fuel relation, result bound THROUGH it, the #214 discipline; NO NB layer — `none` is GENUINE fuel-exhaustion/failure) + `theorem loopDenote_fuel_mono` (surplus fuel after exit unconsumed, induction on fuel) + `theorem whileBodyConverges_unique` (overlap-at-max via fuel-mono + determinism — the `stabilizes_unique` mirror). Axioms `[propext, Quot.sound]`. **REQ-11.2 SHIPPED (v-a)** — `theorem while_compose (prefixB lbody cond tail I) (h_pres) : ∀ st₀ fuel r, whileBodyDenote … = some r → (∀ st₁, blockThread prefixB st₀ = some st₁ → I st₁) → ∃ stf, I stf ∧ condBool cond stf = some false ∧ execDenote tail stf.env = some r` — the SHIPPED partial-correctness `while_rule` lifted through the prefix/tail segments. Axioms `[propext, Quot.sound]`. **REQ-11.3 SHIPPED (v-a, with the #265 DECLARED ADAPTATION)** — `theorem loopDenote_exits_of_dec (cond lbody I μ) (h_pres) (h_cond_total) (h_progress) (h_dec) : ∀ st, I st → ∃ fuel stf, loopDenote cond lbody fuel st = some stf` — the TERMINATION bridge (dec-validity + progress ⟹ the exit witness `while_rule` HYPOTHESIZES, the REQ-1.2 `converges_imp_stabilizes` mirror). ONE DECLARED SEMANTIC ADAPTATION (#265, kernel-record `lean/Thermite/PinWhileDecShape.lean`, commit 92659eb7): the shipped `h_dec` bounds the PRE-state `0 ≤ μ st` (`h_dec : … → μ st' < μ st ∧ 0 ≤ μ st`) — strictly WEAKER hypothesis, so the shipped theorem is strictly MORE GENERAL than a post-state-bounded (`0 ≤ μ st'`) one (the two shapes are NON-equivalent per the pin's `shipped_hdec_is_not_the_pinned_shape`; the §4.2.2 sketch is now pinned to this PRE-state shape, and the pin's `loopDenote_exits_of_dec_design_shape` kernel-derives the post-state-bounded statement as a corollary — the adaptation strengthens, never weakens). Axioms `[propext, Quot.sound]`. **REQ-11.6 SHIPPED (v-a)** — the TWO bridge-divergence pins, each kernel-checked with the standard axiom set: `lean/Thermite/PinWhileVacuity.lean` (the termination-vacuity conjunction oracle — a `while true` no-op body's `whileBodyConverges` is FALSE at every fuel, so a FALSE-`ens` CONTRACT obligation discharges VACUOUSLY while the conjoined CONVERGENCE obligation `∃ r, whileBodyConverges …` is REFUTED at the same env), `lean/Thermite/PinWhileComposition.lean` (the composition mis-map — a loop-SKIPPING variant binds the ENTRY value `lo = 0` and CERTIFIES the wrong `ens: result == 0`, while the FAITHFUL composition runs the SHIPPED L1 loop to the EXIT value `lo = 3` and REFUTES it; both directions pinned). MECHANICAL CROSS-CHECK of the (v-a) ship: `lean/Thermite/PinWhileDecShape.lean` (the #265 critic pin, commit 92659eb7) `import Thermite.Exec.WhileBody` and APPLIES `loopDenote_exits_of_dec` — its kernel-checked compilation is itself the existence proof for the six declarations + the three pin files. KERNEL-BAR MECHANICS (#265): the default `lake build` (`defaultTargets = ["Thermite"]`) elaborates `Exec/WhileBody.lean` but NOT the `PinWhile*` modules, so each pin's `#print axioms` is asserted by EXPLICIT per-module elaboration (`lake env lean Thermite/PinWhileDecShape.lean`), within the standard set `{propext, Classical.choice, Quot.sound}` (NO `sorryAx`/new axiom); every existing theorem + prior pin still green. **REQ-11.4/11.5/11.7 SHIPPED (the exporter, v-b — #264).** `export_item` (`forge/src/lean_export.rs`) routes a v1 WHILE-shaped body (the last `Stmt::Loop(While)` before a REQUIRED tail) to `export_while_body`, the (v) recognizer `recognize_while_body` mirroring `recognize_v1_loop` arm-by-arm. **REQ-11.4** — `Inv_item` (user invs shallow over cells + per loop-CELL SORT+RANGE + per read scalar PARAM SORT+RANGE (the step's no-overflow guard `lo+1 < 2^w` needs each read param's bound `n < 2^w`, established at `_entry` from `InRangeParams`, preserved trivially) + other-param frame + per-cell scope) + `mu_item` (the `dec` over cells); the FIVE per-item obligations in the §4.2.4 DESIGN shapes — `_entry` the prefix-progress-AND-entry `∃ st₁, blockThread prefix_block (stateOf v) = some st₁ ∧ Inv_item v st₁`; `_pres`; `_progress`; `_dec` (the #265 PRE-state `0 ≤ μ st` bound); `_exit` the `∃ r, execDenote tail_expr st.env = some r ∧ <ens>` — with GENERATOR-FIXED proofs (the `WhileBattery` decode chains: `Bind.bindLeft`/sort/frame decode → the overflow `if` `split` → `omega`, `hreq`-aware); and the TWO GENERATOR-FIXED composed theorems (CONTRACT via `while_compose`, `_converges` via `loopDenote_exits_of_dec` — NO `first | … | skip` heuristics; the prefix/tail totality comes from `_entry`/`_exit`'s ∃-content). **REQ-11.5** — `recognize_while_body` / `reject_out_of_while_subset_stmt` / `encode_cell_*` keep the §4.2.5 inventory LOUD (loop-kind/nested/non-last/under-`if`/multi-loop/break/continue/mid-return/non-scalar/empty-or-weak-inv/tail-less → `ExportRefusal::LoopBody`; spec-calling inv/dec → `OutOfFragment`; optres → `OptResResult`). **REQ-11.7** — `check::lean_mutation_score` (UNCHANGED) routes each mutant through `LeanEngine::admits_auto` → `export_item`; an in-grammar while-body mutant now EXPORTS and is ATTEMPTED (not `UntestedAgainstLean`), the floor gate genuinely gates while items on the Lean path (the accounting flows through the fragment widening, no `check.rs` edit). The D-6 width seam is threaded (`cell_ctx` carries the prefix-`let` cell widths into the cond/body/tail/cell-read encodings, not hardcoded `u64`); the dead `cond_holds` emission (D-7) is dropped; `LeanEngine::run_lake` captures BOTH stdout and stderr (R-6a). VERIFICATION (`forge/tests/lean_while.rs`): `count_certifies_l3_via_lean_auto` (the L1 linear `while lo < n inv lo ≤ n dec n - lo { lo = lo + 1 }` certifies L3 via lean-auto — all 5+2 kernel-accept, `{propext, Classical.choice, Quot.sound}`); `sum_does_not_certify_l3_via_lean_recursive_residual` (the corpus `sum`'s recursive-registry `ens` is the §4 interactive residual — NOT L3-via-lean, the HONEST landing); `refusal_matrix_no_lean_certification`; `while_true_no_op_is_not_proven_l3_via_lean` (the §4.2.3 vacuity gate); `in_grammar_while_mutants_are_attempted_not_untested` (REQ-11.7); plus the in-process `engine::tests` (`live_while_body_item_is_honest` Proven, `while_body_item_refuses_export`, `while_refusal_inventory_is_structured`, `live_while_true_vacuity_is_not_proven`). KNOWN LIMITATION (the iv-b precedent — the auto-DISCHARGEABLE surface is narrower than the EXPORTABLE surface): nonlinear invariants/measures, multi-`let`-with-non-literal-init prefixes, and deeper-case-split bodies leave an unsolved goal → `Verdict::Unknown` (fail-to-certify, SOUND — never a false `Proven`; REQ-7 INTERACTIVE is the close); a coverage residual, NOT a blocker. Termination-honesty DECISION recorded at §4.2.3 (the conjoined `_converges`; the two rejected alternatives named with grounds). |

## Open questions (for co-authorship)

These are deliberately left OPEN for a second designer (the orchestrator intends to offer them):

- **OQ-1 (the engine-annotation surface syntax) — DECIDED (increment (iii), #247): the whole-file
  `forge check --engine verus|lean|auto` flag.** The v1 surface is the FLAG, not a per-item
  attribute. DECISION + rationale: `verus` is the DEFAULT (byte-identical to the shipped pipeline —
  the conformance cert oracle is unperturbed, asserted by `engine_attribution.rs::
  engine_verus_flag_is_byte_identical_oracle`); `lean` runs the LeanEngine ONLY (exportable items
  discharged by Lean with the smaller-base attribution; a non-exportable item → an honest
  `LeanUnverifiable` L0 skip, NEVER a false verdict and NEVER a silent Verus fallback); `auto` runs
  Verus FIRST and tries Lean on a Verus inconclusive verdict (the §6 ordering — push-button Verus on
  the common case, Lean as the smaller-base fallback). Cert attribution (REQ-4) is populated whenever
  a NON-DEFAULT engine discharges. The per-item `#[engine(lean)]` attribute is REJECTED for v1 (it is
  new surface area that fails the §2.3 one-way-to-do-everything bar — the whole-file flag covers the
  "this run wants the smaller base" need without a new attribute the parser/validator must carry);
  it stays a noted future alternative if a per-function override proves necessary. Wired:
  `check::EngineSelection` + `check::check_file_with_engine` + the `cli.rs` `--engine` flag (`forge
  check --engine lean|auto|verus`).
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
