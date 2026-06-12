# Forge mutation scoring (the kill-ratio floor)

<!--
tier: 3-component
status: draft
audited-sha: dff9ae866e3437af272a62e078993e66c1116460 (re-audited 2026-06-12: amended — shipped-status Summary; #48/#74/#80 early-return synthesis + 0/0 backstop, the #101 equivalence-excluded denominator, golden-anchored ratios, and the #247 Lean-battery consumer, #262. Amended 2026-06-12 (#269): the TWO MISSING early-return families F-IDENT (identity return) + F-STRUCT-ZERO (named-struct field-zeros) are specced REQ-9..REQ-13, NOT-STARTED — the outside review's item 5, the `move_up` weak-contract escape.)
governs: forge/src/mutation.rs
thesis-refs:
  - thermite-design.md §7
  - thermite-design.md §5.1
  - thermite-design.md §5.3
  - thermite-design.md §6
-->

## Summary

`forge/src/mutation.rs` is **§7 step 4** of the vacuity battery: it generates a
FROZEN, DETERMINISTIC set of mutants of a verifying `fn`'s BODY (operator flips,
off-by-ones, early returns, branch swaps — `thermite-design.md` §7 line 224), re-
lowers and re-verifies each against that `fn`'s OWN (unchanged) contract, and
records the **kill ratio** in the certificate (`contract_quality.mutants_killed`,
Appendix A's `"17/18"`). A mutant verus REJECTS is **killed** (the contract
caught the change — good); a mutant verus PROVES is a **survivor** (the contract
cannot tell the mutant from the real body — too weak). A configurable floor
(default **60%**, §7) gates certification: `kill_ratio >= floor` certifies and
records the ratio; `kill_ratio < floor` does NOT certify (verdict-in-cert reject)
and the cert reports the surviving mutants as a precise strengthening prompt
(`survivor`). The probe runs AFTER a successful L3 proof — you mutate a
known-good body to measure CONTRACT strength, and reuse the proof cache (#8) so
each mutant's re-verify is content-addressed and cheap on re-runs.

SHIPPED (#12/#46) — `forge/src/mutation.rs` implements the frozen mutator
set, the kill-ratio floor gate, and the score type; the REQ-status table below
is the per-REQ evidence, and the **Post-pin amendments** section records what
the fourteen commits since the bootstrap pin changed (re-audited, #262).
The load-bearing prerequisites all ship
and are what this component composes: `forge check` (#5, `check::check_file`),
the per-item verus driver (#5, `check::run_verus` + `classify_verus_outcome`),
the structural triage gate (#6, `vacuity::triage`), the SOLVER vacuity gate (#13,
`vacuity_solver::solver_vacuity_check`), the proof cache (#8, `cache::cache_key`/
`load`/`store`), and the cert schema with the FORWARD-DECLARED
`contract_quality.mutants_killed` / `survivor` fields (`manifest::ContractQuality`
— made live by this component). Real verus is at `~/.local/bin/verus`
(`0.2026.05.24.ecee80a`); the GROUNDING below ran against it.

SHIPPED (#269/#270 coordinated arc): the battery gained the TWO early-return
families — **F-IDENT** (return a same-typed parameter verbatim) and
**F-STRUCT-ZERO** (a named-struct return's field-zero literal). The motivating
finding (the outside review's item 5): `examples/editor/editor.th::move_up(b:
Buffer) -> Buffer` carried `ens result.text.len() == b.text.len() &&
result.cursor <= b.cursor` — a literal `return b` SATISFIED the whole contract
and was L3-provable, yet the pre-#269 battery COULD NOT generate that mutant
(`zero_value_for` had no struct arm, and no identity-return family existed). The
two families now generate it; the editor contracts are tightened (#270) so the
identity mutants are KILLED. REQ-9..REQ-13 below are SHIPPED; the call-bearing
equivalence-exclusion interaction (REQ-13) lands through the companion
`.design/forge/equivalent-mutants.md` REQ-7 exec-harness arm in the SAME arc — so
a §9 composition caller's GENUINELY-equivalent identity survivor is excluded
(certifying L3) while the editor's DISTINGUISHING identity survivors are killed.
See *Cert/oracle impact + landing order*.

## Post-pin amendments (re-audited 2026-06-12, #262)

Fourteen commits touched `mutation.rs` after the bootstrap pin `fa557601`. The
behavior-bearing arcs, verified against the current tree:

- **#48 (`64ec916c`) — the 0/0 escape is gated + slice early-returns.**
  `MutationScore::kill_ratio` returns `0.0` when `scored == 0`: a contract the
  battery cannot exercise is BELOW any positive floor → gated `WeakContract`,
  never a silent vacuous `1.0` pass (anti-Goodhart, R-DEFER-9). And
  `early_return_value` synthesizes the empty-slice literal `&[]` / `&mut []`
  for a reference-to-slice return, so a slice-returning body is SCORED instead
  of 0/0-gated.
- **#74 / #80 — empty-`Vec` / empty-`String` early-return mutants.**
  `empty_vec_value` synthesizes `TVec<Suffix> { data: Vec::new() }` for a
  bounded-`Vec` return and `empty_string_value` synthesizes
  `TString { data: Vec::new() }` for a `String` return (mirroring the #48 `&[]`
  precedent), so those return classes score too. Each verdict-changing widening
  bumped the proof cache's `CHECK_SCHEMA_VERSION` (proof-cache.md, #49) so no
  stale gate verdict is served on an unchanged lowered-source key.
- **#101 (`cb1462d5`) — equivalent-mutant exclusion (governing doc:
  `.design/forge/equivalent-mutants.md`).** A survivor Verus PROVES observably
  equivalent to the real body under `req` (`check::equivalence_proves_equal`)
  drops from the kill-ratio DENOMINATOR: `MutationScore` gained
  `pub equivalent: usize` (a transparency count; `scored` is already net of
  the excluded mutants), and `survivor` NEVER records a proved-equivalent
  mutant. The #48 backstop is preserved: a fn whose mutants are all
  killed-or-equivalent with NONE killed reduces to `0/0` → still gated.
- **#60 (`4dcfabf1`) — the floor compare is verus-anchored.** `meets_floor`'s
  f64 compare is anchored to the proved integer cross-multiply
  `thermite_verified::meets_floor_60` via the in-module f64↔integer agreement
  grid (`mutation::tests::verus_anchor`); the verus-proved fact includes the
  #48 `scored == 0 ⟹ !pass` polarity.
- **Surface ripples** (#92 operators, #93 break/continue, #95 Option/Result,
  #109 tuples, #112 C10, #37 the verbatim `IntLit { value, .. }` node): the
  mutator walk covers the grown `Expr`/`Stmt` surface; the frozen family ORDER
  and the `MUTANT_CAP = 64` order-prefix selection are unchanged.
- **#247 — a SECOND production consumer (engine-generic Lean battery;
  `mutation.rs` itself unchanged by it).** `check::lean_mutation_score` drives
  the SAME `mutation::generate` frozen set through the Lean engine with
  engine-generic kill semantics (`engine::lean_mutant_outcome`): a mutant the
  Lean fragment does not ADMIT is "untested against lean" (NEVER counted
  killed), a Lean-`Proven` mutant SURVIVED, and the floor gates the Lean path
  via `engine::LeanMutationTally::meets_floor` — mirroring this component's
  gate. The #101 equivalence probe is a verus meta-query OUTSIDE the engine
  interface, so the Lean-only path reports the RAW survivor set (an honest
  non-exclusion). The Verus-path battery documented here is untouched by #247.

## Scope boundaries (documented, attributed)

- **IN:** exactly §7 step 4 — generate the frozen mutant set of a `fn`'s body,
  re-verify each against the same contract, score the kill ratio, gate on the
  floor, and report survivors. Nothing more.
- **OUT — strengthening probes** (§7 step 5: auto-PROPOSE a stronger `ens` that
  proves with no body change) are issue **#14**; this component only REPORTS
  which mutants survived (the "precise prompt for strengthening", §7 line 224),
  it never synthesizes a tightened contract.
- **OUT — tautology / vacuity** (§7 steps 2–3) are #13 (`vacuity_solver.rs`,
  done); **structural triage** (§7 step 1) is #6 (`vacuity.rs`, done). Mutation
  scoring runs strictly AFTER those gates pass and AFTER the real L3 proof
  succeeds.
- **OUT — mutating the CONTRACT.** Mutators target the `FnItem.body` only. The
  `req`/`ens`/`fx` and the loop `inv`/`dec` are the FIXED reference the mutants
  are scored against (you measure whether the contract constrains the body, so
  the contract must not move).
- **OUT (#269/#270 split) — tightening the editor's contracts.** This component
  GENERATES the mutants that expose `move_up`-class weakness; the contract fix
  itself (`examples/editor/editor.th` `ens` tightening) is issue **#270**, a
  coordinated Tier-1 landing, never a forge change.

## The #269 gap, grounded (why the battery cannot expose `move_up`)

The shipped early-return ladder is `early_return_value` in
`forge/src/mutation.rs`: scalar zero via `zero_value_for`, else the `&[]` /
`TVec<Suffix> { data: Vec::new() }` / `TString { data: Vec::new() }` syntheses,
else `None`. `zero_value_for`'s match covers
`Type::Prim(U32|U64|Usize)` → `0`, `Type::Prim(Bool)` → `false`,
`Type::Option(_)` → `None`, `Type::Tuple(tys)` → the per-element recursion
(`elems.push(zero_value_for(t)?)` — "Returns `None` if ANY element lacks a
scalar zero … dropped from the denominator, OQ-5"), and then `_ => None`. A
user struct return is `Type::Named(Ident)` (`thermite_syntax::ast` — "a
parameter `a: Account`, a return type `-> Shape` … are all `Type::Named`"), so
it falls into the `_ => None` arm: a struct-returning `fn` gets NO early-return
mutant at all. And no mutator family returns a PARAMETER, so the one mutant
that refutes-or-exposes `move_up`'s contract — the literal `return b` — is
unreachable by `mutation::generate` today. (`move_up` still scores: its body
has binop/intlit/if sites, so it does not hit the 0/0 backstop — it certifies
L3 with a tally that simply never contains the killing question.)

Two further grounded facts shape the spec:

- `pub fn generate(f: &FnItem, _seed: u64)` has NO access to the program's
  struct definitions — but all three production callers already hold them:
  `check::mutation_score` and `check::strengthen_certificate` both take
  `adt_deps: &[Item]` (woven into every mutant's sub-program via
  `check::item_subprogram` so ADT types resolve), and `check::lean_mutation_score`
  holds the whole program via `lean_program(lean)`. The struct-defs seam (REQ-11)
  is a parameter threading, not a new lookup mechanism.
- The struct DEFINITION carries the field list F-STRUCT-ZERO needs:
  `Item::Struct(StructItem)` with
  `StructItem { name, fields: Vec<FieldDef>, inv, sealed, span }` and
  `FieldDef { name, ty }` (`thermite_syntax::ast`); the AST `Type` derives
  `PartialEq`, so F-IDENT's type-match (REQ-9) is the derived structural
  equality, no new comparator.

## Requirements

- **REQ-1 (frozen deterministic mutator set — §7 line 224):** a FIXED set of body
  mutators applied to a verifying `FnItem.body`, each producing one mutant
  `FnItem` (contract untouched) plus a human description. The set, exactly:
  - **operator flips** on `Expr::Binary.op` (`ast.rs` `BinOp`): `Add`↔`Sub`,
    `Mul`↔`Div`, `Lt`↔`Le`, `Gt`↔`Ge`, `Eq`↔`Ne`, `And`↔`Or`;
  - **off-by-ones** on `Expr::IntLit(n)` (`ast.rs` `Expr::IntLit(u128)`):
    `n`→`n+1` and `n`→`n-1` (the `n-1` mutation is skipped when `n == 0` —
    `IntLit` is `u128`, so it cannot represent `-1`; documented, not a silent
    wrap);
  - **early returns**: insert a `Stmt::Return(Some(<default-of-ret-type>))` at the
    FRONT of the body block (`return 0` for an integer return, `return None` for
    an `Option`, `return false` for `bool`; the default is the return type's
    canonical zero value — OQ-3). *(Amended #48/#74/#80: for a slice /
    bounded-`Vec` / `String` return, `early_return_value` synthesizes the empty
    value — `&[]`/`&mut []`, `TVec<Suffix> { data: Vec::new() }`,
    `TString { data: Vec::new() }` — instead of skipping the mutator. Amended
    #269, NOT-STARTED: the family also grows the F-IDENT identity-return and
    F-STRUCT-ZERO struct-zero mutants — REQ-9/REQ-10.)*
  - **branch swaps** on a `Stmt::If` / `Expr::If`: negate the condition (wrap in a
    logical-not — encoded as the `==`↔`!=`/`<`↔`>=` flip already in the operator
    set when the condition is a comparison, else swap the `then`/`else_` arms).
  The mutator set is a `const`/`enum`-fixed table (R-CODE-5 determinism) — no
  config, no plugin surface (`thermite-design.md` pillar §2.3 "one way"). Source:
  `thermite-design.md` §7 line 224 ("operator flips, off-by-ones, early returns,
  branch swaps — fixed deterministic mutator set").

- **REQ-2 (deterministic enumeration order + seed + budget):** mutants are
  enumerated in a DETERMINISTIC order — a pre-order walk of the body AST in
  source order, applying each mutator family in a fixed family order at each site
  — and the resulting list is bounded by a fixed budget `MUTANT_CAP` (a documented
  `const`, OQ-2). When the number of candidate mutation sites exceeds the cap,
  selection is the first `MUTANT_CAP` mutants in the deterministic enumeration
  order, seeded from the pinned solver seed (§5.3 "seeded from the lockfile";
  v0.3 sources `check::DEFAULT_SOLVER_SEED == 0` via `check::resolve_seed`, the
  same seam the L3 path uses — documented, not a new parameter). Same `fn` + same
  mutator set + same seed ⇒ the same ordered mutant list, every run. Source:
  `thermite-design.md` §7 line 224 ("seeded from the lockfile"), §5.3
  (determinism); `goal.md` R-CODE-5.

- **REQ-3 (re-lower + re-verify each mutant against the SAME contract):** each
  mutant `FnItem` is woven into the same per-item sub-program shape
  `check::item_subprogram` builds (the file's `spec fn`s + this `fn`), lowered via
  the EXISTING `thermite_lower::lower`, and run through the existing verus driver
  (`check::run_verus`-class invocation) under the SAME pinned `seed` + `rlimit`
  and against the mutant's `requires`/`ensures`/`invariant`/`decreases` — which
  are the ORIGINAL contract's, unchanged, because only the body was mutated. The
  contract lowering is byte-identical to the real proof's (the same reuse
  `vacuity_solver::extract_lowered_fn` relies on). Source: `thermite-design.md`
  §7 line 224 ("re-verifies each against the contract"); §5.3 (per-item
  isolation).

- **REQ-4 (KILLED vs SURVIVED semantics):** a mutant's verus run is classified by
  the existing three-way `check::classify_verus_outcome` reading:
  - **KILLED** = verus does NOT prove the mutant — a `Counterexample`
    (`success: false`, e.g. "postcondition not satisfied" / "invariant not
    satisfied" / "arithmetic underflow") — the contract caught the change. GOOD.
  - **SURVIVED** = verus PROVES the mutant (`Proved`, `success: true, errors: 0`)
    — the contract holds for the mutant too, so it cannot distinguish the mutant
    from the real body. The contract is too weak there.
  - A mutant whose verus run is a **TIMEOUT** (rlimit-hit, `VerusOutcome::Timeout`)
    is conservatively counted as **KILLED** (an un-proved mutant is not a
    survivor; the floor is a strength FLOOR, so counting an undetermined mutant as
    killed is the non-strict reading — documented as OQ-4, the sound polarity is
    "only a verus SUCCESS is a survivor"). A mutant that fails to LOWER (a
    structurally-degenerate mutant, e.g. an unrepresentable off-by-one) is dropped
    from the denominator (not a mutant, not scored — OQ-5), never an `Err` that
    fails the whole gate. An ENVIRONMENT/internal verus failure (absent /
    unparseable / VIR) on a mutant run surfaces a `ForgeError` (R-CODE-4), never a
    silent kill or survive. Source: `thermite-design.md` §7 line 224; `goal.md`
    R-CODE-4, R-DEFER-9.

- **REQ-5 (kill ratio + floor gate — §7, default 60%):** `kill_ratio = killed /
  total` where `total` is the count of SCORED mutants (those that lowered + ran).
  *(Amended #101: `scored` is additionally NET of survivors Verus proved
  observably equivalent to the real body — equivalent-mutants.md. Amended #48:
  `scored == 0` yields `kill_ratio = 0.0`, below any positive floor, so an
  unscoreable contract is gated `WeakContract`, never vacuously passed.)*
  A configurable floor `MUTATION_FLOOR` (default **0.60**, §7 "a configurable
  floor (default 60%)") gates certification:
  - `kill_ratio >= floor` → the item still certifies; the cert records
    `contract_quality.mutants_killed = "<killed>/<total>"` (the Appendix A
    `"17/18"` shape, a `String`) and `survivor` carries a representative surviving
    mutant's description if any survived, else `None`.
  - `kill_ratio < floor` → the item does NOT certify: a verdict-in-cert reject
    (`Certificate::rejected`-class, `Level::L0`, `RejectReason { cause:
    "WeakContract" }`) carrying `mutants_killed = "<killed>/<total>"` and a
    `survivor` description naming a concrete surviving mutant ("the contract does
    not constrain <behavior>; mutant <desc> survived") — the strengthening prompt.
  The floor surface is a `const MUTATION_FLOOR: f64 = 0.60` (and the `cli`
  `--mutation-floor <FLOAT>` lever, mirroring the existing `--rlimit` lever in
  `cli.rs`); a non-default floor is a deliberate choice, documented. Source:
  `thermite-design.md` §7 line 224, §6 (the certificate is the trust statement),
  §12 ("mutation kill-ratio floor").

- **REQ-6 (graduate `contract_quality.mutants_killed` / `survivor` from forward-
  declared):** `manifest::ContractQuality` ships these two fields FORWARD-DECLARED
  (`ContractQuality::forward_declared` → `mutants_killed: "0/0"`, `survivor:
  None`; EXCLUDED from `Certificate::oracle_subset`). This component makes them
  LIVE: a scored item carries the real `"<killed>/<total>"` and a real `survivor`
  (or `None`). A new `Certificate` constructor (`with_mutation_score` / a
  `rejected_weak_contract`, mirroring #13's `Certificate::rejected_vacuity`) sets
  these two EXISTING Appendix A fields — NO frozen schema field is added or
  renamed (R-SPEC-2). Source: `thermite-design.md` Appendix A
  (`contract_quality.mutants_killed`/`survivor`);
  `.design/forge/certificate-manifest.md` REQ-3.

- **REQ-7 (gate wiring — AFTER L3, reuse the proof cache):** mutation scoring runs
  in `check::check_file`'s per-item L3 path, AFTER the item's REAL body verifies
  L3 (`VerusOutcome::Proved`) and AFTER #6 + #13 passed. A body that does not
  itself verify is never mutation-scored (you mutate a KNOWN-GOOD body — §7's
  premise). Each mutant's re-verify is content-addressed by its LOWERED source
  (`cache::cache_key(&mutant_lowered, seed, &verus_version, &thermite_version)`)
  and consults `cache::load` before spawning verus, exactly as the L3 path does
  (#8 makes re-runs cheap — a re-`forge check` of an unchanged file re-scores from
  the cache). A mutant cert is NOT itself surfaced to the user (it is an internal
  scoring run); only the parent item's `mutants_killed`/`survivor` is recorded.
  Source: `thermite-design.md` §7 (the battery runs inside the gate), §5.3
  (content-addressed per-item cache); `.design/forge/proof-cache.md`.

- **REQ-8 (determinism of the kill ratio — R-CODE-5, oracle-eligibility):** given
  the FROZEN mutator set + the pinned seed + a fixed toolchain (verus + thermite
  version), the ordered mutant list is deterministic (REQ-2), each mutant's verus
  verdict is deterministic (the same property the L3 proof and #13 rely on,
  `cache.rs`'s soundness invariant), so `kill_ratio` and `mutants_killed` are
  DETERMINISTIC — the same `fn` scores the same `"K/N"` every run. This makes
  `mutants_killed` ORACLE-CHECKABLE in principle (it is a deterministic function
  of the input). v0.1/v0.3 STANCE (OQ-1): `mutants_killed` and `survivor` REMAIN
  oracle-EXCLUDED in `Certificate::oracle_subset` for now — the kill ratio is
  deterministic given a pinned toolchain, but it is sensitive to the verus
  VERSION (a prover that proves one more mutant shifts the ratio), so pinning it
  in a frozen golden cert (`sum.cert.json`'s `"17/18"`) would make the oracle
  brittle across verus upgrades. The deterministic claim is verified by a
  same-input-twice AC instead (AC-4). Promoting `mutants_killed` into the oracle
  subset is a `certificate-manifest.md` amendment, made when the corpus pins a
  verus version. Source: `thermite-design.md` §5.3; `goal.md` R-CODE-5, R-CHAR-3;
  `conformance/README.md` ("forward-declared fields ... becomes a LIVE assertion
  when its producing component lands").

### Amendment #269 — the two missing early-return families (NOT-STARTED)

- **REQ-9 (F-IDENT — identity-return mutants):** for EACH parameter `p` of `f`
  whose type EXACTLY equals the return type — `p.ty == f.ret` under the AST
  `Type`'s derived `PartialEq` (structural equality; the v1 scope decision:
  **by-value exact-type match only**, NO ref-stripping — a `b: &Buffer` param
  with a `Buffer` return would need a synthesized clone/deref the surface AST
  cannot express without inventing a copy op, so it generates NO identity
  mutant in v1; OQ-7. Note exact equality still covers a ref-typed param
  returned at a ref-typed return, e.g. the divergence fixture
  `pick(xs: &[u32]) -> &[u32]` — `return xs` borrows nothing new) — synthesize
  ONE mutant inserting `Stmt::Return(Some(Expr::Path(vec![p.name])))` at the
  FRONT of the body block, one mutant PER matching param in declaration order,
  each labeled with the param name (e.g. ``insert early `return b` at body head
  (identity of param `b`)``) so multi-param matches stay distinguishable
  (OQ-8). Family placement: F-IDENT joins family 1 (early returns), emitted
  immediately AFTER the existing zero-value early-return mutant and BEFORE
  families 2–4, so the `MUTANT_CAP` order-prefix never crowds out the
  discriminator mutants (the shipped "listed first so the cap never crowds it
  out" rationale). KILL SEMANTICS: a STRONG contract refutes the identity
  (killed — `to_1based`'s `ens result == x + 1` rejects `return x`; `min2`'s
  `result <= a && result <= b` rejects both `return a` and `return b` via the
  cross counterexamples); a WEAK contract proves it (survivor → the §7 floor
  gates / the `survivor` prompt names it). This is the mutant that exposes
  `move_up`: `return b` proves `result.text.len() == b.text.len() &&
  result.cursor <= b.cursor` verbatim. INTENDED-SIGNAL note: a `fn` whose
  CORRECT behavior IS the identity on SOME inputs (e.g. `move_up` at row 0,
  the `if ls == 0 { b }` arm) does NOT make the mutant survive — the mutant
  survives only if the contract cannot distinguish the identity on ANY admitted
  input, which is exactly the §7 weakness being measured. Source:
  `thermite-design.md` §7 line 224 ("early returns" — the identity is the
  early-return family completed over parameter values, not just zeros);
  R-DEFER-9 (anti-Goodhart: the battery must be able to ASK the question the
  weak contract fails).

- **REQ-10 (F-STRUCT-ZERO — named-struct field-zero early returns):** extend the
  early-return zero synthesis to a `Type::Named(name)` return when `name`
  resolves to an `Item::Struct(StructItem)` among the threaded defs (REQ-11):
  synthesize `Expr::StructLit { path: vec![name], fields: <per-field zeros> }`,
  where each `FieldDef { name, ty }`'s zero comes from the SAME synthesis ladder
  the early-return family already owns (the scalar `zero_value_for` arms, the
  #74 `TVec<Suffix> { data: Vec::new() }` / #80 `TString { data: Vec::new() }`
  empty-wrapper literals for `Vec`/`String` fields, the C9-B tuple recursion,
  and — recursively — a nested named-struct's own field zeros). ANY field
  without a synthesizable zero (a `Box`/`Ref`/`Result`/enum-typed field) ⇒ NO
  mutant for that return type (the OQ-5 drop), MIRRORING the shipped
  `Type::Tuple` rule verbatim ("Returns `None` if ANY element lacks a scalar
  zero … dropped from the denominator, OQ-5 — never an over-gate"). An
  ENUM-named return gets no F-STRUCT-ZERO mutant (no canonical variant to
  choose — OQ-5 drop). TYPE-INVARIANT interaction: a struct `inv` is CONTRACT —
  if the field-zero literal violates it (e.g. a hypothetical `inv balance >=
  10`), Verus fails the construction obligation and the mutant is KILLED (the
  honest polarity: the type invariant caught the wrong body); for the corpus
  structs the zeros satisfy the `inv` (`Account { balance: 0 }`: `0 <=
  1_000_000`; `Buffer { text: <empty>, cursor: 0 }`: `0 <= 0 && 0 <=
  1_000_000`) so the mutant is scored against the `ens`. Grounded kills:
  `deposit`'s `ens result.balance == a.balance + amount` rejects
  `Account { balance: 0 }` (counterexample `a.balance + amount >= 1`);
  `move_up`'s length-identity `ens` rejects the empty-`Buffer` zero for any
  non-empty text — F-STRUCT-ZERO alone does NOT expose `move_up` (that is
  F-IDENT's job, REQ-9); it closes the "struct-returning fn has NO early-return
  mutant at all" hole. Source: `thermite-design.md` §7 line 224; the
  #48/#74/#80 synthesis precedent chain.

- **REQ-11 (the struct-defs seam — `generate` gains program access):**
  F-STRUCT-ZERO needs the struct DEFINITIONS (`StructItem.fields`), which
  `pub fn generate(f: &FnItem, _seed: u64)` cannot see today. The seam:
  `generate` gains a third parameter carrying the ADT items (shape:
  `generate(f, seed, adt_deps: &[Item])`, or an equivalent narrow
  `&[&StructItem]` projection — builder's choice, pinned at the definition
  site), and `early_return_value`/`zero_value_for` grow a defs-threaded form
  (a new sibling, e.g. `zero_value_with_defs(ty, defs)`, keeping the existing
  `zero_value_for` as the leaf for def-free types — no behavior change for the
  shipped arms). GROUNDED: all three production callers already hold the items
  — `check::mutation_score(.., adt_deps: &[Item], ..)` and
  `check::strengthen_certificate(.., adt_deps, ..)` thread `adt_deps` into
  every mutant's `item_subprogram` weave already, and `check::lean_mutation_score`
  holds the full program via `lean_program(lean)` (`.items`). The ripple is
  three call sites plus the in-crate tests; no new lookup machinery, no config.
  Recursion guard: a struct field can only reference another struct through
  `Box` (direct self-reference is unrepresentable), and `Box` has no zero (the
  OQ-5 drop), so the nested-struct recursion terminates without an explicit
  cycle check — documented at the recursion site.

- **REQ-12 (verdict-stability discipline — cache schema bump + re-derived
  frozen-order oracle):** both families are VERDICT-CHANGING widenings of the
  frozen set (an item's `K/N` and even its certify/gate verdict can change), so
  landing them bumps `cache.rs`'s `const CHECK_SCHEMA_VERSION` (currently `5`;
  the #48/#74/#80 + #49 discipline — no stale gate verdict served on an
  unchanged lowered-source key), and the AC-5 frozen-set/order unit oracle is
  RE-DERIVED BY HAND from REQ-1/REQ-9/REQ-10's family order (R-CHAR-3 — never
  copied from the generator's output). Determinism (REQ-2/REQ-8) is preserved:
  the new mutants are a pure function of the AST + the threaded struct defs
  (themselves part of the same parsed program), enumeration stays seed-stable
  order-prefix.

- **REQ-13 (equivalence-exclusion interaction — the #101 rule applies
  unchanged):** an F-IDENT mutant of a `fn` whose body IS already
  `return <param>`-equivalent under `req` (the equivalent-mutants fixtures
  `refuse(x) { x }` and `clamp_zero(x) req x == 0 { let y = x + 0; y }`) will
  SURVIVE and then be handled by the SHIPPED exclusion: `check::mutation_score`
  routes every survivor through `check::equivalence_proves_equal`
  (`thermite_lower::lower_equivalence_obligation` — "under `req`, mutant_body ==
  real_body" for all inputs); a verus-PROVED obligation excludes the mutant from
  BOTH the numerator and the denominator (`equivalent += 1; continue;` — never
  recorded as `survivor`), and the #48 `0/0` backstop still gates the
  all-equivalent case (`refuse` stays gated). HONEST LIMIT (carried from #101
  OQ-1, not new): the obligation seam renders only scalar shapes —
  `lower_equivalence_obligation` returns `Unsupported` for a non-scalar result
  and the caller maps that to `Ok(false)` — so a STRUCT-returning identity
  mutant that happens to be truly equivalent stays a COUNTED survivor
  (sound-but-incomplete: never launders a distinguishing mutant; for `move_up`
  the mutant is genuinely distinguishing anyway). On the Lean path
  (`check::lean_mutation_score`) the new families flow through the SAME
  `mutation::generate` set with the #247 engine-generic semantics: a mutant the
  fragment does not ADMIT is "untested against lean" (never counted killed),
  and the raw survivor set is reported (no equivalence probe) — OQ-9.

## Acceptance criteria

ACs tie to a `conformance/mutation/` oracle (authored by the orchestrator, NOT
this component), shaped like `conformance/solver-vacuity/cases.json`
(`accept`/`reject` entries hand-derived from §7, R-CHAR-3). The fixture programs
below PARSE clean and `forge check` runs them today; the verus mutant verdicts
are GROUNDED (the real verus outputs are pasted in *Ground the mutants*).

- **AC-1 (strong corpus contracts → high kill ratio → certify):**
  `conformance/sum.th` (`sum`) and `conformance/binary_search.th`
  (`binary_search`) score `kill_ratio >= 0.60` and certify L3 with
  `contract_quality.mutants_killed = "<K>/<N>"` for `K/N >= 0.60`. GROUNDED for
  `sum`: the three hand-applied body mutants (`+`→`-`, `i=i+1`→`i=i+2`, early
  `return 0`) are ALL killed by `sum`'s real `ens result == spec_sum(xs)` (verus
  `success: false` on each — see *Ground the mutants*), i.e. a 3/3 sample. The
  oracle asserts `mutants_killed` is `>= floor` (a ratio threshold, NOT a frozen
  exact string — REQ-8/OQ-1), so it is robust to the exact denominator the frozen
  mutator set produces.

- **AC-2 (a WEAK-but-non-vacuous contract → low kill ratio → gated, survivor
  reported):** the fixture
  `conformance/mutation/weak_sum.th` (PARSE-VERIFIED, the exact program below):
  ```thermite
  fn sum(xs: &[u32]) -> u64
    req xs.len() <= 1_000_000
    ens result <= 1_000_000 * u32::MAX as u64
    fx  pure
  {
    let mut acc: u64 = 0;
    let mut i: usize = 0;
    while i < xs.len()
      inv i <= xs.len()
      inv acc <= i as u64 * u32::MAX as u64
      dec xs.len() - i
    {
      acc = acc + xs[i] as u64;
      i = i + 1;
    }
    acc
  }
  ```
  This contract PASSES #6 (the `ens` mentions `result`, is not literal-`true`,
  not an identity, not a `req` conjunct) and PASSES #13 (the `ens` does NOT hold
  for an arbitrary `result: u64` — `0 <= 1_000_000 * u32::MAX` holds but
  `u64::MAX <= 1_000_000 * u32::MAX` does NOT, so it is not a semantic tautology;
  the `req` is satisfiable). It also VERIFIES L3 (the real body proves it — see
  *Ground the mutants*). Yet it UNDER-CONSTRAINS the result: the **early
  `return 0`** mutant SURVIVES (verus PROVES it — `0 <= 1_000_000 * u32::MAX`),
  so the kill ratio drops below `0.60` on the mutant set and the item is GATED
  (does NOT certify), `RejectReason { cause: "WeakContract" }`, with `survivor`
  naming the early-return mutant ("the contract does not constrain the computed
  sum; mutant `insert return 0 at body head` survives `ens`"). This is the §7
  value-add and the discriminator from AC-1's strong contract (where the SAME
  early-return mutant is killed).

- **AC-3 (the floor is the gate, configurable):** the oracle asserts the verdict
  flips on the floor — `weak_sum.th` certifies under a low `--mutation-floor`
  (e.g. `0.2`, below its kill ratio) and is gated under the default `0.60`,
  exercising the configurable floor (REQ-5).

- **AC-4 (kill ratio is DETERMINISTIC — R-CODE-5):** scoring the SAME fixture
  twice (same fn, same frozen mutator set, same pinned seed + toolchain) yields
  the byte-identical `mutants_killed` string and the same `survivor`. A unit / a
  conformance double-run asserts equality of the two `"K/N"` strings (NOT against
  forge's own output as a golden — against ITSELF across two runs, the
  determinism property; R-CHAR-3-clean because the asserted relation is
  "run1 == run2", a property, not a fabricated constant).

- **AC-5 (mutators are the frozen set, deterministic order):** a unit test over
  the mutator generator asserts the produced mutant list for a fixed small `fn`
  is exactly the documented frozen set in the documented order (operator flips at
  each `Binary` site, off-by-ones at each `IntLit`, the early return, branch
  swaps), capped at `MUTANT_CAP`. Expected mutants trace to REQ-1's table
  (R-CHAR-3), not to the generator's own output. *(Amended #269: the hand-derived
  list grows the F-IDENT + F-STRUCT-ZERO family-1 entries and their order —
  zero-return first, then identity-returns in param order, then families 2–4 —
  re-derived from REQ-9/REQ-10, REQ-12.)*

- **AC-6 (the body must verify first; environment failure never a silent kill):**
  mutation scoring is reached ONLY on a `VerusOutcome::Proved` real body (REQ-7) —
  a non-verifying `fn` produces its L3 counterexample cert and is never scored. A
  mutant run that hits verus-absent / unparseable / VIR surfaces a `ForgeError`
  (R-CODE-4), asserted via a unit test over the classification path with a
  synthetic verus error (mirroring `vacuity_solver`'s `interpret_summary` tests).

### Amendment #269 ACs (NOT-STARTED)

- **AC-7 (the motivating editor case — F-IDENT exposes `move_up`):** under the
  new battery, `examples/editor/editor.th::move_up` gains the mutant
  ``insert early `return b` at body head (identity of param `b`)``.
  PRE-TIGHTENING (the current contract `ens result.text.len() == b.text.len()
  && result.cursor <= b.cursor`) the mutant SURVIVES — hand-derivation: for
  `result = b`, `b.text.len() == b.text.len()` is reflexive and `b.cursor <=
  b.cursor` is reflexive, both provable for ALL inputs, and the equivalence
  probe cannot exclude it (`Buffer` is non-scalar → `Unsupported` → counted
  survivor; it is also genuinely distinguishing — the real body moves the
  cursor). POST-TIGHTENING (#270 — an `ens` pinning the computed up-cursor) the
  SAME mutant is KILLED. The oracle asserts the qualitative pair
  ("pre-tightening: a surviving identity mutant is reported / the item gates;
  post-tightening: the identity mutant is killed and the item certifies L3"),
  never a frozen ratio.

- **AC-8 (struct-zero + identity against a STRONG struct contract — no
  over-gating):** `conformance/bank_account.th::deposit(a: Account, amount:
  u64) -> Account` gains BOTH new mutants and KILLS both — `return a` fails
  `ens result.balance == a.balance + amount` (counterexample any admitted
  `amount >= 1`) and `return Account { balance: 0 }` fails it (counterexample
  `a.balance + amount >= 1`; the `inv balance <= 1_000_000` construction
  obligation is satisfied by the zero, so the kill is the `ens`, not a lowering
  drop). `deposit` STILL certifies L3 with a raised ratio — the families enable
  scoring, they do not auto-gate strong contracts.

- **AC-9 (equivalence interplay — the #101 fixtures stay stable):** the
  equivalent-mutants fixtures gain identity mutants that are PROVED EQUIVALENT
  and excluded: `refuse(x) req x == 0 ens result == 0 { x }`'s `return x` IS the
  body — excluded, `refuse` still reduces to `0/0` and stays GATED (the #48
  backstop, equivalent-mutants.md AC-5 preserved); `clamp_zero`'s `return x` ≡
  `{ let y = x + 0; y }` under `req x == 0` — excluded from numerator AND
  denominator, never `survivor`. The `add(a,b) ens result == a + b` fixture's
  two identity mutants are KILLED (cross counterexamples).

- **AC-10 (unsynthesizable struct → OQ-5 drop, never an error):** a struct
  return whose field set contains a zero-less type (a `Box`-typed field, an
  enum-typed field) generates NO F-STRUCT-ZERO mutant — the fn's remaining
  mutants score normally and no `ForgeError` surfaces; if NOTHING is
  synthesizable and the body has no site, the #48 `0/0` backstop gates (the
  floor-of-last-resort, unchanged).

- **AC-11 (verdict-stability — schema bump + determinism):** landing the
  families bumps `CHECK_SCHEMA_VERSION` (a unit assertion that the const moved,
  or the proof-cache conformance re-key check), and the AC-4 run==run
  determinism double-run passes over a fixture containing both new families
  (an identity-eligible struct-returning fn).

## Architecture

`mutation.rs` is a new `mod mutation;` in `forge/src/lib.rs`, consumed by
`check.rs` in the per-item L3 path. It depends on `thermite_syntax::ast`
(`FnItem`, `Block`, `Stmt`, `Expr`, `BinOp`, `IntLit`) for the AST it mutates,
`thermite_lower::lower` (re-lowering each mutant, reused unchanged), the existing
verus driver in `check.rs` (the same `run_verus` + `classify_verus_outcome` the
L3 path uses), and `cache.rs` (content-addressing each mutant's re-verify). It
owns NO new schema: it sets the two EXISTING `manifest::ContractQuality` fields
(`mutants_killed`, `survivor`) and, on a sub-floor ratio, produces a
`manifest::RejectReason { cause: "WeakContract" }`.

### Data flow (the gate stage)

```text
check::check_file per-item L3 path, on VerusOutcome::Proved (real body verifies):
  mutation::generate(f)                  → Vec<Mutant { item: FnItem, desc: String }>   (REQ-1/REQ-2, frozen + ordered + capped)
  for each mutant (up to MUTANT_CAP):
    item_subprogram(mutant) → lower      → mutant_lowered                                (REQ-3, reuse)
    cache::cache_key(mutant_lowered,..)  → load? else run_verus + store                  (REQ-7, #8 reuse)
    classify_verus_outcome               → Proved=SURVIVED | (Counterexample|Timeout)=KILLED  (REQ-4)
  kill_ratio = killed / scored                                                            (REQ-5)
  kill_ratio >= MUTATION_FLOOR  → Certificate.with_mutation_score("K/N", survivor?)  (certify, REQ-5/REQ-6)
  kill_ratio <  MUTATION_FLOOR  → Certificate::rejected_weak_contract("K/N", survivor) (gate, REQ-5/REQ-6)
```

The mutated unit is the `FnItem`'s body ONLY (REQ-1); the lowered mutant's
`requires`/`ensures`/`invariant`/`decreases` are the original contract's
(`thermite-design.md` §7 — "re-verifies each against the contract"). The mutant
re-uses `check::item_subprogram`'s weaving (`spec fn`s + combinator defs) so a
mutant of `sum` still resolves `spec_sum`.

### Why a mutant verus SUCCESS is the BAD news (the polarity)

A mutant is a DELIBERATELY-WRONG body. If verus still PROVES it against the
contract, the contract is satisfied by both the right body and the wrong one —
the contract does not distinguish them, i.e. it under-specifies. So `Proved` =
SURVIVED = a hole in the contract; a verus FAILURE = KILLED = the contract did
its job (REQ-4). This is the same polarity inversion #13's harnesses use (verus
proving the degenerate-property harness is the bad news), applied to the body
instead of the contract.

### Determinism and the oracle (REQ-8)

The mutant list is a pure function of the AST + the frozen mutator table + the
seed (REQ-2); each mutant verdict is the same deterministic verus run the L3 path
+ cache rely on. So `mutants_killed` is deterministic given a pinned toolchain.
It is NOT promoted into `Certificate::oracle_subset` in v0.3 (OQ-1) because the
exact ratio is verus-VERSION-sensitive (a stronger prover may prove one more
mutant); the AC pins the deterministic PROPERTY (AC-4, run==run) and a ratio
THRESHOLD (AC-1, `>= floor`), not a frozen exact string — keeping the oracle
robust across verus upgrades (`conformance/README.md`'s forward-declaration
discipline; R-CHAR-3).

### Cert/oracle impact + landing order (#269/#270 — the critical section)

Both families change `scored`/`killed` (and possibly the certify/gate verdict)
on every affected item, so the blast radius over the corpus goldens and
hand-derived oracles is enumerated HERE, hand-derived per fixture (R-CHAR-3 —
the ORCHESTRATOR re-certifies every changed expectation; the toolchain NEVER
authors its own oracle):

- **Frozen conformance goldens (`conformance/*.cert.json`): NO byte change
  expected under the v1 scopes.** `conformance/sum.cert.json` is the ONLY
  golden carrying a live tally (`"mutants_killed": "17/18"` + the `survivor`
  string) — `sum(xs: &[u32]) -> u64` has no by-value param equal to `u64` and
  no struct return, so it gains NO new mutant (likewise
  `binary_search(haystack: &[u32], needle: u32) -> Option<usize>`). The other
  five goldens (`bank_account`, `shape`, `map_kv`, `option_result`,
  `parse_u64`) deliberately OMIT `mutants_killed` (their `note` fields pin it
  oracle-EXCLUDED), so even `deposit`'s tally shift (+2 killed, +2 scored —
  AC-8) changes no golden bytes. The orchestrator CONFIRMS this no-change claim
  at landing by re-running the cert oracle suite — if a frozen golden DOES
  move, that is a scope violation to investigate, not a golden to silently
  regenerate.
- **`forge/tests/editor_runs.rs` (the e2e editor oracle) — EXPECTED RED
  pre-tightening.** `editor_logic_certifies_l3_boundary_and_run_l1` pins EVERY
  editor logic item at L3. Hand-derivation of the new mutants: `move_up` and
  `move_down` each gain a BY-CONSTRUCTION-surviving identity mutant
  (`return b` proves the length-identity + cursor-bound `ens`, AC-7);
  `line_end(text, i, n) -> u64` gains TWO surviving identities (`return i` and
  `return n` both prove the bounds-only `ens result >= i && result <= n`);
  `min2`/`to_1based`/`decode`/`count_nl`/`line_start`/`insert_str`/`backspace`/
  `move_left`/`move_right` gain identity and/or `Buffer`-zero mutants that are
  KILLED by their pinning `ens`. Whether each weak item's ratio crosses the
  `0.60` floor (gate → the L3 assertion fails) or merely records a survivor is
  tool-computed (OQ-10) — the landing PLANS for the gate (the dispatching
  review and #269 expect `WeakContract` on the weak items). The exact-pin
  `cursor_col == "4/4"` is UNAFFECTED under the v1 by-value scope
  (`cursor_col(b: &Buffer) -> u64` — a ref param, no `u64` by-value param).
- **`forge/tests/equivalent_mutants_conformance.rs`** — `clamp_zero`/`loose`/
  `refuse` gain identity mutants that are PROVED-EQUIVALENT → excluded
  (tallies stable, the `equivalent` transparency count rises by one); `add`
  gains two KILLED identities. The relational asserts are expected to hold;
  the orchestrator re-derives any exact-count expectation by hand (AC-9).
- **`conformance/mutation/cases.json`** — `weak_loose_bound` (`f(a: u32, b:
  u32) -> u32 ens result <= 1_000_000`) gains two SURVIVING identity mutants
  (`a <= 10 ⟹ a <= 1_000_000`), pushing it further below the floor; the
  qualitative below-floor oracle holds unchanged.
- **`forge/tests/divergence_mutation.rs`** — the `pick(xs: &[u32]) -> &[u32]`
  authority pin gains the identity `return xs` (exact ref-type match, REQ-9),
  a counted survivor alongside the #48 `&[]` survivor; the item stays gated
  and the divergence assertion stays satisfied.
- **Lean-path tallies (`check::lean_mutation_score` consumers, e.g.
  `forge/tests/lean_while.rs`)** — the new mutants flow into the Lean battery's
  admitted/untested buckets (OQ-9); the orchestrator re-derives any affected
  tally expectation.

**Landing order (MANDATORY — never a red-main window):** the families land in
ONE coordinated Tier-1 arc — (1) the #269 battery widening (REQ-9..REQ-13,
schema bump, re-derived unit oracles), (2) the #270 editor contract tightening
(`move_up`/`move_down`/`line_end` `ens` pinned so the identity mutants are
killed — the `cursor_col`/#126 tight-contract precedent), and (3) the
orchestrator's hand-re-certified oracle expectations (R-CHAR-3 ceremony) —
merged TOGETHER, so there is no window where `main` carries the new battery
against the un-tightened editor and `editor_runs.rs` is red. The editor gating
`WeakContract` pre-tightening is the EXPECTED and CORRECT behavior of the new
battery (the §7 value-add), which is precisely why the contract fix and the
battery must ship as one arc.

## Verification

- `cargo test -p forge` — unit tests for the mutator generator (AC-5: frozen set
  + order + cap), the kill-ratio + floor classification (AC-2/AC-3 over synthetic
  verdicts), the determinism property (AC-4), and the environment-error path
  (AC-6, synthetic verus error → `ForgeError`). *(#269: the AC-5 hand-derived
  list grows the F-IDENT/F-STRUCT-ZERO entries; new unit oracles for the
  type-match rule (REQ-9), the field-zero recursion + OQ-5 drop (REQ-10/AC-10),
  and the seam threading (REQ-11) — expectations derived from this doc, never
  from `generate`'s output.)*
- `forge/tests/mutation_conformance.rs` — the conformance oracle: parses
  `conformance/mutation/cases.json`, runs the real scoring (real verus) over each
  `accept` fixture (corpus `sum`/`binary_search` → `kill_ratio >= 0.60`, certify,
  AC-1) and each `reject` fixture (`weak_sum.th` → `kill_ratio < 0.60`, gated,
  `RejectReason { cause: "WeakContract" }` + a non-`None` `survivor`, AC-2),
  asserting the floor flip (AC-3) and the deterministic re-score (AC-4). Expected
  verdicts are hand-derived from §7 (R-CHAR-3), never copied from forge's output.
- *(#269)* the AC-7 editor pre/post-tightening pair and the AC-8 `deposit`
  no-over-gating check land as orchestrator-authored conformance entries in the
  same arc as #270 (see *Cert/oracle impact + landing order*).
- `cargo clippy -p forge --all-targets -- -D warnings`, `cargo fmt --check` (the
  gauntlet, `goal.md`).

## Route to add (orchestrator, NOT this component)

`tooling/spec-routes.toml` gains a route for the greenfield file (this doc does
NOT edit the route table — `goal.md` R-XLATE-2 is the orchestrator's job; the
spec-discipline hook blocks the builder's first edit until the route exists):

```toml
[[route]]
crate_pattern = "forge/src/mutation.rs"
design = ".design/forge/mutation-scoring.md"
reference = ["conformance/mutation"]
```

*(Shipped — the route exists at `tooling/spec-routes.toml` ("v0.3 — mutation
scoring (issue #12)") with `conformance_ops = ["weak_contract", "sum"]`; no
route change is needed for the #269 amendment — it governs the same file.)*

## Ground the mutants (real verus, `0.2026.05.24.ecee80a`)

These are the MANDATORY grounding runs: `sum`'s body was lowered via the real
`thermite_lower::lower`, mutated BY HAND in the lowered exec body (the contract /
invariants left intact), and re-run through the real `verus` binary. They confirm
the KILLED / SURVIVED polarity and the strong-vs-weak value-add.

### Baseline: the real `sum` (strong `ens result == spec_sum(xs)`) verifies

```text
verification-results: {success: True, verified: 5, errors: 0}
```

(The premise of §7 step 4: you mutate a KNOWN-GOOD body — REQ-7.)

### Strong contract → all three mutants KILLED (3/3)

Mutating `sum`'s lowered body against its REAL `ens result == spec_sum(xs)`:

```text
MUTANT acc = acc + xs[i] → acc = acc - xs[i]   (operator flip Add→Sub)
  verification-results: {success: False, verified: 4, errors: 1}
  error: invariant not satisfied at end of loop body
  error: possible arithmetic underflow/overflow            → KILLED

MUTANT i = i + 1 → i = i + 2                    (off-by-one)
  verification-results: {success: False, verified: 4, errors: 1}
  error: invariant not satisfied at end of loop body        → KILLED

MUTANT insert `return 0;` at body head         (early return)
  verification-results: {success: False, verified: 2, errors: 1}
  error: postcondition not satisfied                        → KILLED
```

A 3/3 sample → kill ratio above the 60% floor → `sum` certifies (AC-1).

### Weak contract → the early-return mutant SURVIVES (the value-add)

The PARSE-VERIFIED weak fixture (`ens result <= 1_000_000 * u32::MAX as u64`,
AC-2) lowers and VERIFIES L3 on the real body, AND passes #6 + #13. Its body
mutants:

```text
WEAK BASELINE (real body)
  verification-results: {success: True, verified: 3, errors: 0}     (verifies L3)

WEAK MUTANT  acc + → acc -            : {success: False, errors: 1}  → KILLED (loop inv `acc <= i*MAX` + underflow)
WEAK MUTANT  i = i + 1 → i = i + 2    : {success: False, errors: 1}  → KILLED (loop inv broken)
WEAK MUTANT  insert `return 0;`       : {success: True,  errors: 0}  → SURVIVED  ★
```

The early-return-`0` mutant SURVIVES the weak contract — verus PROVES it, because
`0 <= 1_000_000 * u32::MAX` holds, so the weak `ens` cannot tell `return 0` from
the real sum. The SAME mutant against the strong contract is KILLED (above). The
surviving mutant IS the strengthening prompt §7 describes ("which behavior the
contract fails to constrain"): the weak `ens` does not pin `result` to the
computed sum. This single mutant drops the kill ratio below the floor → the weak
contract is GATED (REQ-5) with `survivor` naming the early-return mutant — the
precise value-add the floor catches.

### #269 hand-derivation: the identity mutant against `move_up` (NOT yet a tool run)

The F-IDENT family does not exist, so this grounding is the BY-HAND semantic
derivation the families' kill semantics rest on (the builder re-grounds with
real verus at landing): for `move_up(b: Buffer) -> Buffer` with
`ens result.text.len() == b.text.len()` and `ens result.cursor <= b.cursor`,
the mutant body `{ return b; … }` yields `result = b`, so both clauses reduce
to reflexivity (`b.text.len() == b.text.len()`, `b.cursor <= b.cursor`) — a
Verus `Proved` for all inputs (the same trivial-discharge class as the GROUNDED
`return 0` survival above) → SURVIVED. The real body sets
`cursor = prev_ls + min2(col, prev_len)` on the `ls != 0` arm, so the mutant is
NOT observably equivalent (any two-line buffer with the cursor on line 2 is a
counterexample) — and the equivalence probe could not exclude it anyway (a
`Buffer` result is non-scalar → `lower_equivalence_obligation` `Unsupported` →
counted survivor, REQ-13). Conversely `to_1based`'s `ens result == x + 1`
REFUTES `return x` (`x != x + 1`, the same counterexample class as the grounded
strong-`sum` kills) — the families do not over-gate strong contracts.

## Open questions

- **OQ-1 (oracle promotion of `mutants_killed`):** the kill ratio is
  deterministic (REQ-8) and so COULD live in `Certificate::oracle_subset`, but it
  is verus-version-sensitive. v0.3 keeps it oracle-EXCLUDED (forward-declared,
  asserted by a `>= floor` threshold + a run==run determinism AC). Promote it to
  a frozen exact `"K/N"` golden only once the corpus pins a verus version — a
  `certificate-manifest.md` amendment. **(Least confident — see report.)**
- **OQ-2 (`MUTANT_CAP` value):** §7 says "budgeted" without a number. The cap is a
  documented `const`; a small fixed value (e.g. the count the corpus `fn`s
  naturally produce, on the order of tens) keeps the gate fast (each mutant is a
  full verus run). The exact number is a builder decision pinned at the const's
  definition site; the design only mandates that it be FIXED (R-CODE-5) and
  documented.
- **OQ-3 (early-return default value):** the early-return mutant inserts
  `return <default>`; the default is the return type's canonical zero (`0` /
  `false` / `None`). For a type with no obvious zero this mutator is skipped for
  that `fn` (dropped from the set, not an error). Documented at the mutator site.
  *(Resolved post-pin: #48/#74/#80 synthesize empty-slice/-`Vec`/-`String`
  values, narrowing the skip set to genuinely un-synthesizable types — where the
  #48 0/0 backstop gates rather than vacuously passing. #269 narrows it further:
  named-struct returns with all-synthesizable fields, REQ-10.)*
- **OQ-4 (timeout polarity):** a mutant whose verus run TIMES OUT is counted
  KILLED (an un-proved mutant is not a survivor). The sound invariant is "only a
  verus SUCCESS is a survivor"; a timeout is the non-strict reading and is
  documented. The generous `check::DEFAULT_RLIMIT` makes a mutant timeout
  unlikely.
- **OQ-5 (un-lowerable / structurally-degenerate mutants):** a mutant that fails
  to lower (e.g. an off-by-one that produces a type-invalid literal) is DROPPED
  from the denominator (not scored), never an `Err` that fails the gate — the
  frozen set is still applied uniformly; only the realizable mutants are scored.
- **OQ-6 (combinator-bearing bodies):** the corpus bodies are exec code (no
  combinators in the BODY — combinators live in `req`/`ens`/`inv`, which the
  mutator never touches). So body mutation is well-defined for the corpus.
  Whether a body that itself calls a `spec fn`/combinator (not in v0.1's corpus)
  yields meaningful mutants is open; for v0.3 the mutator set targets the
  arithmetic/comparison/control-flow constructs §7 names, which are exec-body
  shapes. **(Noted as a least-confident edge — see report.)**
- **OQ-7 (#269 — ref-param identity scope):** v1 F-IDENT is EXACT structural
  `Type` equality only (REQ-9). A ref-STRIPPING match (`b: &Buffer -> Buffer`)
  would need a synthesized clone/deref — the surface AST has no copy operation
  to express it, and inventing one for the mutator would put the battery ahead
  of the language. If a future basis stage adds an explicit clone, widening
  F-IDENT to ref-stripped matches is a one-arm amendment (and a
  `CHECK_SCHEMA_VERSION` bump, REQ-12). Concretely deferred-out:
  `cursor_row`/`cursor_col(b: &Buffer) -> u64` gain no identity mutant in v1.
- **OQ-8 (#269 — multi-param dedup):** two params of the same matching type
  yield TWO identity mutants (`min2` → `return a` AND `return b`) — no dedup,
  even when the params could be provably equal under `req` (the #101 exclusion
  handles a genuinely-equivalent survivor; pre-deduping in the generator would
  be a semantic judgment the frozen syntactic table must not make). Cap
  interaction: a many-param fn spends cap budget on its identities; family-1
  placement keeps them ahead of the prefix cut (REQ-9), accepted as the
  designed bias (the discriminator mutants matter most).
- **OQ-9 (#269 — Lean-path attempt semantics for the new families):** the
  families flow into `check::lean_mutation_score` automatically (same
  `mutation::generate`). Admission is per-mutant (`LeanEngine::admits_auto` over
  the mutant program): a struct-zero / identity mutant whose obligation the Lean
  fragment does not admit is "untested against lean" — reported, NEVER counted
  killed (#247 semantics); an admitted Lean-`Proven` identity mutant SURVIVES
  with NO equivalence exclusion (the probe is a verus meta-query the Lean-only
  path doesn't thread — the raw survivor set is the honest report). Whether the
  v1 Lean fragment admits `Buffer`-shaped obligations at all is tool-determined
  at landing; the design only pins the bucket polarity.
- **OQ-10 (#269 — gate-or-survivor on the editor items):** the identity
  survivors on `move_up`/`move_down`/`line_end` are hand-derived CERTAIN
  (AC-7); whether each item's full ratio lands below `0.60` (gate
  `WeakContract`) or above (certify-with-survivor) depends on the kill verdicts
  of the items' other ~10–20 mutants against their weak `ens` — tool-computed,
  not hand-derivable to the digit. The landing arc treats EITHER outcome as a
  red-risk to `editor_runs.rs`'s L3 pins and ships #269+#270 together
  (*Cert/oracle impact + landing order*); #270's tightening makes the question
  moot post-arc.

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (frozen mutator set) | SHIPPED | `mutation::generate` in `forge/src/mutation.rs` walks a `FnItem.body` and applies the frozen families: operator flips (`flip_binop`: `Add`↔`Sub`/`Mul`↔`Div`/`Lt`↔`Le`/`Gt`↔`Ge`/`Eq`↔`Ne`/`And`↔`Or`), off-by-ones (`Expr::IntLit n`→`n+1`/`n-1`, `n-1` skipped at 0), early returns (`early_return_value`: scalar zero via `zero_value_for` — `Option`→`None`/int→`0`/bool→`false` — OR the synthesized empty `&[]`/`&mut []` slice (#48), `TVec<Suffix> { data: Vec::new() }` (#74), `TString { data: Vec::new() }` (#80)), branch swaps (`negate_comparison` / arm swap). Consumers: `check::mutation_score` and (post-pin, #247) `check::lean_mutation_score` (the engine-generic Lean battery) in `check.rs`. |
| REQ-2 (deterministic order + seed + cap) | SHIPPED | `mutation::generate` enumerates in a fixed pre-order family sequence (`MutantSink`/`Applier` with per-kind `Counters`), capped by `pub const MUTANT_CAP = 64`; the seam takes `check::DEFAULT_SOLVER_SEED`. Verified by `mutation::tests::frozen_set_and_order_for_small_fn` + `generate_is_deterministic` + `capped_at_mutant_cap`. |
| REQ-3 (re-lower + re-verify vs same contract) | SHIPPED | `check::mutation_score` weaves each `Mutant.item` via `check::item_subprogram` + `thermite_lower::lower` and runs `check::run_verus`; the contract is the original's (only `body` mutated — `mutation::tests::mutant_keeps_contract_changes_only_body`). |
| REQ-4 (KILLED vs SURVIVED) | SHIPPED | `mutation::classify_mutant` + `check::mutant_outcome_is_survivor`/`mutant_cert_is_survivor`: a `Proved` mutant SURVIVED, a counterexample/timeout is KILLED. Post-pin (#101): a SURVIVOR additionally runs the equivalence query (`check::equivalence_proves_equal`) — proved-equivalent → excluded from BOTH the survivor set AND the denominator (`MutationScore.equivalent` records the count), per `.design/forge/equivalent-mutants.md`. Verified by `mutation::tests::classify_polarity_is_inverted` + `mutation_conformance.rs` + `equivalent_mutants_conformance.rs`. |
| REQ-5 (kill ratio + 60% floor gate) | SHIPPED | `mutation::MutationScore::{kill_ratio,meets_floor,mutants_killed_string}` + `pub const MUTATION_FLOOR = 0.60`; the gate in `check::check_file_with_options` certifies `>= floor` and produces `Certificate::rejected_weak_contract` (`RejectReason { cause: "WeakContract" }`) below it. `scored == 0` ⇒ `kill_ratio = 0.0` (the #48 backstop, below any positive floor); `meets_floor` is verus-anchored to `thermite_verified::meets_floor_60` (#60). The `cli` `--mutation-floor <FLOAT>` lever threads a non-default floor. Verified by `mutation_conformance.rs` (AC-2/AC-3: the `reject_below_floor` oracle entry `weak_loose_bound` is gated `WeakContract`; the oracle asserts the threshold relation, not a frozen count). |
| REQ-6 (graduate `mutants_killed`/`survivor`) | SHIPPED | `Certificate::with_mutation_score` (certified path) + `Certificate::rejected_weak_contract` (reject path) set the two EXISTING Appendix A fields; no schema change (R-SPEC-2). Verified by `manifest::tests::with_mutation_score_graduates_fields_and_stays_oracle_excluded` + `rejected_weak_contract_carries_cause_ratio_and_survivor`. |
| REQ-7 (gate AFTER L3, reuse proof cache) | SHIPPED | `check::mutation_score` runs only when `cert.level == L3 && reject.is_none()` (a proved real body); each mutant content-addresses via `cache::cache_key`/`load`/`store` (a non-default rlimit/floor bypasses the cache). Consumer: `check::check_file_with_options`'s post-L3 stage. |
| REQ-8 (deterministic kill ratio, oracle stance) | SHIPPED | `generate` is a pure function of the AST + frozen table; the kill ratio is deterministic (verified by `mutation_conformance.rs::kill_ratio_is_deterministic_across_two_runs`, run==run). `mutants_killed`/`survivor` stay oracle-EXCLUDED in `Certificate::oracle_subset` (OQ-1, verus-version-sensitive). GROUNDED at the current tree: the frozen golden `conformance/sum.cert.json` pins `mutants_killed: "17/18"` with `survivor` "mutant#11: `i = i + 1` → `i = i + 2` survives ens but killed by inv#2" (the pin-era 7/7 sample predates the #92-operator mutant-set growth); the conformance oracle asserts threshold relations (`>= floor` / `< floor`), never frozen exact counts. |
| REQ-9 (F-IDENT identity-return family) | SHIPPED | `mutation::generate(f, _seed, adt_deps)` (`forge/src/mutation.rs`) emits, in family 1 immediately AFTER the zero-value early-return and BEFORE families 2–4, ONE identity mutant per param whose `p.ty == f.ret` (AST `Type` derived `PartialEq`, by-value exact-type match — OQ-7), each `Stmt::Return(Some(Expr::Path(vec![p.name])))` labeled ``insert early `return <p>` at body head (identity of param `<p>`)`` (OQ-8 multi-param distinct). This is the mutant that exposes `move_up`: `return b` proves the length-identity + cursor-bound `ens`. Consumers: `check::mutation_score` + `check::lean_mutation_score`. Verified: `forge/tests/editor_runs.rs::editor_logic_certifies_l3_boundary_and_run_l1` (the tightened editor certifies L3 — the identity mutants KILLED post-#270) + `mutation::tests` (the re-derived frozen-order oracle). |
| REQ-10 (F-STRUCT-ZERO named-struct field-zero family) | SHIPPED | `struct_zero_value(name, adt_deps)` + `zero_value_with_defs(ty, adt_deps)` + `find_struct` (`forge/src/mutation.rs`): a `Type::Named(name)` return resolving to an `Item::Struct` synthesizes `Expr::StructLit { path: vec![name], fields: <per-field zeros> }`, each field's zero from the SAME ladder (scalar `zero_value_for`, the #74/#80 empty-wrapper literals, the tuple recursion, and recursively a nested struct's own zeros). ANY field without a synthesizable zero ⇒ NO mutant (the OQ-5 drop, mirroring `Type::Tuple`); an enum-named return ⇒ no mutant (no canonical variant). Verified: `forge/tests/editor_runs.rs` (`Buffer`-zero mutants KILLED by the pinning `ens`) + `mutation::tests`. |
| REQ-11 (struct-defs seam into `generate`) | SHIPPED | `pub fn generate(f: &FnItem, _seed: u64, adt_deps: &[Item])` gained the third ADT-items parameter; the three production callers thread it — `check::mutation_score(.., adt_deps, ..)` and `check::strengthen_certificate(.., adt_deps, ..)` (the SAME `adt_deps` they weave into `item_subprogram`) and `check::lean_mutation_score` via `lean_program(lean)`. `early_return_value(f, adt_deps)` / `zero_value_with_defs(ty, adt_deps)` are the defs-threaded forms; `zero_value_for` stays the def-free leaf (no behavior change for the shipped arms). The nested-struct recursion terminates via the `Box`-has-no-zero OQ-5 drop. |
| REQ-12 (cache schema bump + re-derived frozen-order oracle) | SHIPPED | `cache.rs`'s `const CHECK_SCHEMA_VERSION` bumped to `7` (the families bumped `5 → 6`; the #269 call-bearing equivalence arm — `.design/forge/equivalent-mutants.md` REQ-7 — bumped `6 → 7`, the schema-history note records both): both are VERDICT-CHANGING widenings, so no stale gate verdict is served on an unchanged lowered-source key (the #48/#74/#80/#49 discipline). The `mutation::tests` frozen-set/order oracle is re-derived by hand from REQ-1/REQ-9/REQ-10 (R-CHAR-3); determinism (REQ-2/REQ-8) is preserved (the new mutants are a pure function of the AST + the threaded struct defs). |
| REQ-13 (equivalence-exclusion + Lean-path interaction) | SHIPPED | An F-IDENT survivor routes through the SHIPPED exclusion: `check::mutation_score` calls `check::equivalence_proves_equal` on every survivor; a verus-PROVED obligation excludes it (`equivalent += 1; continue;`, never `survivor`). For a CALL-FREE identity (`refuse(x) { x }`, `clamp_zero`) the scalar spec-fn obligation proves equivalence → excluded; the #48 `0/0` backstop still gates `refuse`. For a CALL-BEARING identity (the §9 composition `caller(x) { ext_id(x) }`) the #269 REQ-7 exec harness proves equivalence MODULO ext_id's contract → excluded → `caller` certifies L3 (AC-6). The HONEST LIMIT narrowed (no longer "non-scalar always counted"): a non-scalar/out-of-scope identity still stays a COUNTED survivor, now with the structured `Unsupported` reason carried (equivalent-mutants.md REQ-9). On the Lean path the new families flow through the SAME `mutation::generate` with #247 engine-generic semantics (non-admitted ⇒ "untested against lean", never killed; raw survivor set, no equivalence probe — OQ-9). Verified: `forge/tests/equivalent_mutants_conformance.rs` (5/5, the #101 fixtures stay stable) + `forge/tests/composition_conformance.rs` (AC-6/AC-8). |
