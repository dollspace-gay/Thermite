# Forge mutation scoring (the kill-ratio floor)

<!--
tier: 3-component
status: draft
audited-sha: dff9ae866e3437af272a62e078993e66c1116460 (re-audited 2026-06-12: amended — shipped-status Summary; #48/#74/#80 early-return synthesis + 0/0 backstop, the #101 equivalence-excluded denominator, golden-anchored ratios, and the #247 Lean-battery consumer, #262)
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
    `TString { data: Vec::new() }` — instead of skipping the mutator.)*
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
  (R-CHAR-3), not to the generator's own output.

- **AC-6 (the body must verify first; environment failure never a silent kill):**
  mutation scoring is reached ONLY on a `VerusOutcome::Proved` real body (REQ-7) —
  a non-verifying `fn` produces its L3 counterexample cert and is never scored. A
  mutant run that hits verus-absent / unparseable / VIR surfaces a `ForgeError`
  (R-CODE-4), asserted via a unit test over the classification path with a
  synthetic verus error (mirroring `vacuity_solver`'s `interpret_summary` tests).

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

## Verification

- `cargo test -p forge` — unit tests for the mutator generator (AC-5: frozen set
  + order + cap), the kill-ratio + floor classification (AC-2/AC-3 over synthetic
  verdicts), the determinism property (AC-4), and the environment-error path
  (AC-6, synthetic verus error → `ForgeError`).
- `forge/tests/mutation_conformance.rs` — the conformance oracle: parses
  `conformance/mutation/cases.json`, runs the real scoring (real verus) over each
  `accept` fixture (corpus `sum`/`binary_search` → `kill_ratio >= 0.60`, certify,
  AC-1) and each `reject` fixture (`weak_sum.th` → `kill_ratio < 0.60`, gated,
  `RejectReason { cause: "WeakContract" }` + a non-`None` `survivor`, AC-2),
  asserting the floor flip (AC-3) and the deterministic re-score (AC-4). Expected
  verdicts are hand-derived from §7 (R-CHAR-3), never copied from forge's output.
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
  #48 0/0 backstop gates rather than vacuously passing.)*
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
