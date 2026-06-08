# Forge equivalent-mutant exclusion (the §7 kill-ratio denominator fix)

<!--
tier: 3-component
status: draft
governs: forge/src/mutation.rs
thesis-refs:
  - thermite-design.md §7
  - thermite-design.md §6
-->

## Summary

This component refines **§7 step 4** (`.design/forge/mutation-scoring.md`): a
SURVIVING mutant that Verus PROVES is observably-equivalent to the real body
*under the precondition* is dropped from the kill-ratio DENOMINATOR rather than
counted as a survivor. The bug it fixes (crosslink **#101**): the mutation gate
counts a mutant that is PROVABLY EQUIVALENT to the real body as a survivor,
falsely flagging an honest contract `WeakContract`. The textbook case is a
forced-output refusal — under a precondition that pins an output, the real body
and an "early-`return <that output>`" mutant are behaviorally identical, so no
input distinguishes them, yet the equivalent mutant survives and depresses the
ratio. The exclusion is **sound-but-incomplete**: a mutant is dropped ONLY on a
Verus PROOF of observable equivalence under `req`; a mutant Verus cannot prove
equivalent (a distinguishing input exists, or the proof times out)
conservatively STAYS counted. A genuinely-distinguishing survivor — the symptom
of a real `WeakContract` — is never excluded, so the exclusion cannot launder a
weak contract (`goal.md` R-DEFER-9).

This doc governs the per-survivor equivalence check + the denominator drop in
`forge/src/mutation.rs`; the gate-wiring change is in `check.rs`
(`mutation_score`) and is threaded through the existing `MutationScore` shape.

## Scope boundaries (documented, attributed)

- **IN:** for each mutant classified a SURVIVOR by §7 step 4 (Verus PROVED it
  against the unchanged contract), run ONE additional Verus EQUIVALENCE query —
  "under `req`, does the mutant body produce the same observable result as the
  real body for all inputs?". A PROVED-equivalent mutant is removed from BOTH the
  survivor set AND the `scored` denominator; an unproven one stays a counted
  survivor. The denominator/kill-ratio arithmetic + the `MutationScore` field
  carrying the excluded count; the `CHECK_SCHEMA_VERSION` bump (the gate verdict
  changes for forced-output fns).
- **OUT — the killed/survived classification itself** (§7 step 4, REQ-4 of
  `.design/forge/mutation-scoring.md`). A KILLED mutant is never equivalence-
  checked (Verus already rejected it against the contract — by definition it is
  distinguished). Only SURVIVORS are candidates for exclusion.
- **OUT — mutating the contract / the mutator set** (`mutation-scoring.md`
  REQ-1/REQ-3). The equivalence check reads the SAME real body and the SAME
  mutant body the §7 gate produced; it adds no mutant and touches no contract.
- **OUT — strengthening probes** (§7 step 5, `.design/forge/strengthening-
  probes.md`). The probe runs only on a met-floor cert; this exclusion changes
  whether the floor is met for a forced-output fn, but it adds no suggestion.

## Requirements

- **REQ-1 (per-survivor Verus equivalence check — §7):** for each mutant the §7
  gate classifies a SURVIVOR (`MutantOutcome::Survived` — Verus proved it against
  the unchanged contract), the gate issues ONE further Verus query asking whether,
  *under the function's `req`*, the mutant body's observable result equals the
  real body's observable result for ALL inputs. The query reuses the EXISTING
  Verus driver (`check::run_verus`-class invocation): the equivalence obligation
  is `ensures mutant_result == real_result`, discharged under `requires <the
  fn's req>` (and the fn's parameter types). It is a new CALLER of the existing
  prover path, not a new prover. Source: `thermite-design.md` §7 ("re-verifies
  each against the contract" — this is the dual: re-verify the survivor against
  the real body); `goal.md` R-CODE-4 (an environment/VIR failure surfaces a
  `ForgeError`, never a silent equivalence).

- **REQ-2 (PROVED-equivalent → dropped from the denominator; sound-but-
  incomplete):** if the equivalence query VERIFIES (Verus proves the mutant
  observably equal to the real body under `req`), the mutant is a TRUE equivalent
  mutant — not a contract weakness — and is removed from the kill-ratio
  DENOMINATOR (it does not count as a survivor, and it does not count as scored).
  If the query does NOT verify — Verus finds a distinguishing input
  (counterexample) OR the query times out — the mutant STAYS a counted survivor
  (the conservative, sound reading: exclude ONLY on a proof). This mirrors the
  existing OQ-5 / OQ-4 polarity in `mutation-scoring.md` (an un-lowerable mutant
  is already dropped from the denominator; an un-proved mutant is already
  conservatively the strict reading). Source: `thermite-design.md` §7; `goal.md`
  R-DEFER-9.

- **REQ-3 (the soundness line — a distinguishing mutant is NEVER excluded):** the
  exclusion is gated on a Verus PROOF of equivalence, so a mutant that DIFFERS
  from the real body under `req` (a genuinely-distinguishing survivor — exactly
  the symptom of a contract too weak to pin the behavior) fails the equivalence
  query and is NEVER dropped. The kill-ratio denominator for a genuinely-weak
  contract is therefore unchanged by this component; a weak contract still gates
  `WeakContract`. The exclusion narrows the denominator ONLY by mutants the
  prover certifies are indistinguishable from the truth. Source: `goal.md`
  R-DEFER-9 ("never discharge an obligation by weakening it"); `thermite-design.md`
  §7 (the battery's anti-Goodhart purpose).

- **REQ-4 (the `MutationScore` denominator change + the `K/N` cert):**
  `mutation::MutationScore` records the proved-equivalent exclusions so that
  `scored` (the denominator) is the count of mutants that lowered, ran, AND were
  NOT proved equivalent. `kill_ratio = killed / scored` and the Appendix A
  `mutants_killed = "killed/scored"` string both reflect the REDUCED denominator.
  The `0/0` backstop (`mutation-scoring.md` REQ-5: a `scored == 0` score is below
  floor, gated `WeakContract`) is UNCHANGED — if every scored mutant is killed or
  proved-equivalent, leaving a non-empty killed set, the ratio certifies; if
  exclusion empties the denominator entirely (every mutant proved equivalent and
  none killed), the `0/0` backstop STILL gates (a contract that cannot be
  mutation-validated has not met the §7 bar). Source: `thermite-design.md`
  Appendix A (`contract_quality.mutants_killed`); `mutation-scoring.md` REQ-5/REQ-6.

- **REQ-5 (`CHECK_SCHEMA_VERSION` bump — cache invalidation):** because the gate
  verdict CHANGES for forced-output fns (a `0/1` `WeakContract` becomes a
  certifying score once the equivalent mutant is excluded), the check logic is
  no longer the same function of its inputs. `cache::CHECK_SCHEMA_VERSION` is
  bumped (the on-disk cache key input, `cache.rs` `cache_key`), so stale cached
  verdicts for forced-output fns are invalidated and re-scored under the new
  logic. Source: `.design/forge/proof-cache.md` (the schema-version cache-key
  input, blocker #49); `cache.rs` `CHECK_SCHEMA_VERSION`.

- **REQ-6 (determinism + bounded cost — R-CODE-5):** the equivalence check is a
  Verus proof obligation — a deterministic function of the (mutant body, real
  body, `req`, parameter types) under a pinned seed + rlimit + toolchain, so a
  proved-equivalent exclusion is DETERMINISTIC (the same fn excludes the same
  mutants every run; `mutants_killed` stays deterministic, `mutation-scoring.md`
  REQ-8). The cost is ONE extra Verus run PER SURVIVOR (not per mutant): survivors
  are the few mutants the contract failed to kill, so the added cost is bounded by
  the survivor count, itself bounded by `MUTANT_CAP`. Each equivalence query is
  content-addressed through the SAME proof cache (#8), so re-runs are cheap.
  Source: `goal.md` R-CODE-5; `mutation-scoring.md` REQ-7/REQ-8.

## Acceptance criteria

ACs tie to a `conformance/mutation/` oracle (authored by the orchestrator);
expected verdicts are hand-derived from §7 (R-CHAR-3), GROUNDED against real
Verus (`0.2026.05.24.ecee80a`) below in *Ground the path*.

- **AC-1 (a forced-output fn → the equivalent mutant excluded → certifies; was
  depressed):** the fixture `clamp_zero` (`req x == 0`, `ens result == 0`,
  body `{ let y: u64 = x + 0; y }`) scores `1/3` BEFORE this component (the
  early-`return 0` and the `x - 0` binop-flip survivors are both proved equal to
  the real body under `x == 0`; only the `x + 1` off-by-one is killed) →
  `WeakContract`. With exclusion, both proved-equivalent survivors drop from the
  denominator → `1/1 = 1.0 >= 0.60` → CERTIFIES L3 with `mutants_killed = "1/1"`.
  The verdict FLIP from `WeakContract` to certify is the #101 fix.

- **AC-2 (a genuinely-weak contract → STILL `WeakContract`; not laundered):** the
  fixture `loose` (`req x <= 100`, `ens result <= 1000`, body `{ let y: u64 =
  x + 0; y }`) has a SURVIVING early-`return 0` mutant that is NOT equivalent to
  the real body (under `x <= 100`, `0 != x` for `x = 5`), so its equivalence
  query FAILS → it STAYS counted → the score remains below floor → `WeakContract`
  (`mutants_killed = "0/2"`, survivor reported). (The `x - 0` arithmetic-flip
  mutant IS proved-equivalent — `x - 0 == x + 0` for all `x`, independent of
  `req` — and is soundly excluded, dropping the denominator 3 → 2; the
  DISTINGUISHING `return 0` survivor is what keeps the verdict `WeakContract`.)
  The exclusion does NOT launder it (R-DEFER-9).

- **AC-3 (the exclusion is Verus-PROVED, no heuristic):** a mutant is excluded
  ONLY when the equivalence query VERIFIES (`success: true, errors: 0`); a
  counterexample or timeout never excludes. A unit/conformance test asserts the
  exclusion decision is the Verus verdict, not a syntactic shape match: the
  `return 0` mutant is excluded ONLY under `req x == 0` (which makes `0 == x`)
  and STAYS counted under the looser `req x <= 100` (AC-2), whereas the `x - 0`
  flip is excluded under ANY `req` (it equals `x + 0` for all `x`) — the verdict
  tracks provable equivalence per precondition, not the mutant's shape.

- **AC-4 (determinism — R-CODE-5):** scoring the SAME forced-output fixture twice
  yields the byte-identical reduced `mutants_killed` and the same exclusion set.

- **AC-5 (the `0/0` backstop survives exclusion):** a forced-output fn ALL of
  whose mutants are proved equivalent and none killed (the degenerate `refuse(x)
  req x == 0 ens result == 0 { x }`, whose sole early-`return 0` mutant is its only
  scored mutant and is proved equivalent) reduces to `0/0` → the #48 backstop
  STILL gates it (`kill_ratio == 0.0` < floor) — exclusion never opens a vacuous
  `1.0` pass for a fn the battery could not exercise.

## Architecture

The equivalence check is a new seam in `forge/src/mutation.rs` (the equivalence-
obligation formulation) consumed by `check::mutation_score` in `check.rs` (it
weaves the obligation as a per-item sub-program, lowers via the EXISTING
`thermite_lower::lower`, content-addresses through `cache.rs`, and runs the
EXISTING `run_verus`). It depends on `thermite_syntax` (the `FnItem` whose `req`
+ params + return type frame the obligation, the real `body`, and the survivor's
`body`), `thermite_lower::lower` (reused), the `check.rs` Verus driver (reused),
and `cache.rs` (the `CHECK_SCHEMA_VERSION` bump + per-query content addressing).

### The equivalence obligation (the formulation)

Given the function `f` (its `req`, params, return type), its real body `B_real`,
and a survivor mutant's body `B_mut`, the equivalence obligation asks Verus to
prove that, under `req`, `B_mut`'s observable result equals `B_real`'s for all
inputs. Hand-derived to spec form (the GROUNDED query below): two `spec fn`s — one
per body — over the same parameters, with a `proof fn` that `requires <req>` and
`ensures B_mut(params..) == B_real(params..)`. A VERIFIED result is a PROOF of
observable equivalence (REQ-2 → exclude); a postcondition-not-satisfied result is
a distinguishing input (REQ-3 → stays counted); a timeout is the conservative
stay-counted reading (REQ-2, sound-but-incomplete).

### Data flow (the refined §7 step 4)

```text
mutation_score, per mutant Verus-classified SURVIVED:
  build the equivalence obligation (B_mut, B_real, f.req, f.params)        (REQ-1)
  item_subprogram(obligation) -> thermite_lower::lower -> cache::cache_key  (REQ-1/REQ-6, reuse)
  load? else run_verus + store                                             (REQ-6, #8 cache)
    VERIFIED        -> PROVED equivalent -> drop from denominator           (REQ-2: scored -= 1, not a survivor)
    counterexample  -> distinguishing    -> STAYS a counted survivor        (REQ-3)
    timeout         -> unproven          -> STAYS a counted survivor        (REQ-2, conservative)
  kill_ratio = killed / scored   (scored is the REDUCED denominator)        (REQ-4)
```

### Why this cannot launder a weak contract (the soundness line)

A `WeakContract` verdict means a mutant survived that the contract should have
killed — i.e. a body DIFFERENT from the real body that the `ens` nonetheless
admits. That mutant DIFFERS from the real body, so its equivalence query has a
distinguishing input and FAILS to prove (REQ-3, GROUNDED below: the `x + 1`
off-by-one and the `loose` early-`return 0` both fail equivalence). The exclusion
can only ever remove mutants the prover certifies are indistinguishable from the
truth — those were never evidence of weakness. The denominator a genuinely-weak
contract is scored against is unchanged (`goal.md` R-DEFER-9).

## Verification

- `cargo test -p forge` — unit tests for the equivalence-obligation formulation
  (the spec-fn pair + the `requires req / ensures B_mut == B_real` shape), the
  exclude/keep decision over synthetic Verus verdicts (VERIFIED → exclude,
  counterexample → keep, timeout → keep, AC-3), and the determinism property
  (AC-4).
- `forge/tests/mutation_conformance.rs` (extended) — the conformance oracle over
  `conformance/mutation/` runs real scoring (real Verus): the forced-output
  `accept` fixture (`clamp_zero` → certifies `1/1`, AC-1), the genuinely-weak
  `reject` fixture (`loose` → `WeakContract` `0/2`, AC-2), and the `0/0`-backstop
  `reject` fixture (`refuse` → `WeakContract` `0/0`, AC-5). Expected verdicts are
  hand-derived from §7 (R-CHAR-3).
- `cargo clippy -p forge --all-targets -- -D warnings`, `cargo fmt --check`.

## Route to add (orchestrator, NOT this component)

`forge/src/mutation.rs` already routes to `.design/forge/mutation-scoring.md`.
This refinement co-governs the same file; the orchestrator adds the additional
design path to the existing route (the spec-discipline hook reads each route's
`design`):

```toml
[[route]]
crate_pattern = "forge/src/mutation.rs"
design = ".design/forge/mutation-scoring.md"
# add:
# design = ".design/forge/equivalent-mutants.md"
reference = ["conformance/mutation"]
```

## Ground the path (real Verus, `0.2026.05.24.ecee80a`; real `forge check`)

The full #101 path was ground end-to-end against the real `forge` binary and the
real `verus` binary. (Scratch fixtures were under `/tmp`; removed.)

### 1. The bug — a forced-output fn is falsely `WeakContract` (BEFORE)

`refuse(x: u64) -> u64  req x == 0  ens result == 0  { x }` — `forge check`:

```text
item: refuse
level: L0
reject: WeakContract — §7 step 4 ... mutation kill ratio 0/1 is below the floor;
        mutant `insert early `return 0` at body head` survived ...
```

`clamp_zero(x: u64) -> u64  req x == 0  ens result == 0  { let y: u64 = x + 0; y }`:

```text
item: clamp_zero
level: L0
reject: WeakContract — ... mutation kill ratio 1/3 is below the floor;
        mutant `insert early `return 0` at body head` survived ...
```

Both honest (the `ens` pins `result == 0`, which the real body provably satisfies
under `req x == 0`), both falsely gated.

### 2. The equivalence proof — the survivors ARE equivalent (Verus VERIFIED)

`clamp_zero`'s three mutants under `req x == 0`: `x + 1` (off-by-one) is KILLED;
the early-`return 0` and the `x - 0` (binop flip) SURVIVE. Both survivors' bodies
are observably equal to the real body `x + 0` under `x == 0`:

```rust
spec fn real_body(x: u64) -> u64 { (x + 0) as u64 }
spec fn mut_early(x: u64) -> u64 { 0 }
spec fn mut_sub(x: u64) -> u64 { (x - 0) as u64 }
proof fn equiv_early(x: u64) requires x == 0, ensures mut_early(x) == real_body(x) {}
proof fn equiv_sub(x: u64)   requires x == 0, ensures mut_sub(x)   == real_body(x) {}
```
```text
verification results:: 2 verified, 0 errors
```

Both survivors are PROVED equivalent → excluded from the denominator (REQ-2). The
`refuse` sole survivor likewise verifies:

```rust
spec fn real_body(x: u64) -> u64 { x }
spec fn mutant_body(x: u64) -> u64 { 0 }
proof fn equivalence_under_req(x: u64) requires x == 0, ensures mutant_body(x) == real_body(x) {}
```
```text
verification results:: 1 verified, 0 errors
```

### 3. AFTER exclusion — the honest ratio certifies

- `clamp_zero`: `1/3` → exclude the 2 proved-equivalent survivors → `1/1 = 1.0`
  `>= 0.60` → CERTIFIES L3, `mutants_killed = "1/1"` (AC-1).
- `refuse`: `0/1` → exclude the sole proved-equivalent survivor → `0/0` → the #48
  backstop STILL gates `WeakContract` (AC-5 — exclusion never opens a vacuous
  pass; the fn is genuinely unscoreable).

### 4. The soundness line — a distinguishing survivor STAYS counted (NOT laundered)

The KILLED `x + 1` mutant is NOT equivalent (the equivalence query FAILS):

```rust
spec fn real_body(x: u64) -> u64 { (x + 0) as u64 }
spec fn mut_offbyone(x: u64) -> u64 { (x + 1) as u64 }
proof fn equiv_offbyone(x: u64) requires x == 0, ensures mut_offbyone(x) == real_body(x) {}
```
```text
verification results:: 0 verified, 1 errors        (postcondition not satisfied)
```

A genuinely-WEAK contract stays `WeakContract`. `loose(x: u64) -> u64  req x <= 100
ens result <= 1000  { let y: u64 = x + 0; y }` — `forge check`:

```text
item: loose
level: L0
reject: WeakContract — ... mutation kill ratio 0/2 is below the floor;
        mutant `insert early `return 0` at body head` survived ...
        (the `x - 0` flip was proved-equivalent and excluded, 3 → 2)
```

Its early-`return 0` survivor's equivalence query FAILS under the looser `req`
(the SAME mutant that was excludable under `x == 0` is NOT excludable under
`x <= 100`, AC-3 — the decision is the Verus verdict, not a syntactic match):

```rust
spec fn real_body(x: u64) -> u64 { (x + 0) as u64 }
spec fn mut_early(x: u64) -> u64 { 0 }
proof fn equiv_early(x: u64) requires x <= 100, ensures mut_early(x) == real_body(x) {}
```
```text
verification results:: 0 verified, 1 errors        (postcondition not satisfied)
```

So `loose` stays `0/2` → STILL `WeakContract` (the distinguishing `return 0`
survivor keeps it below floor; only the genuinely-equivalent `x - 0` flip was
excluded). The exclusion narrows the denominator ONLY by prover-certified-
indistinguishable mutants (REQ-3, R-DEFER-9).

## Open questions

- **OQ-1 (equivalence over richer return types):** the GROUNDED cases are scalar
  (`u64`) returns where observable equality is value equality. For
  reference/slice/`Vec`/`String` returns (the #48/#74/#80 early-return classes)
  the observable-equivalence obligation is structural equality of the returned
  value (`==` over the lowered wrapper); the formulation generalizes (the spec-fn
  pair returns the wrapper type and `ensures` its `==`), but only scalar returns
  are GROUNDED here. **(Least confident — see report.)**
- **OQ-2 (effectful bodies):** v0.1 mutation scores `fx pure` exec bodies; an
  effectful body's "observable result" would also include its effect trace, not
  just the return value. The corpus forced-output fns are `pure`, so value
  equality is the full observable. A non-pure forced-output fn is out of v0.1
  scope (effects subsume at compile time, §4).
- **OQ-3 (the per-survivor cost vs. the cap):** the cost is one Verus run per
  SURVIVOR, bounded by `MUTANT_CAP` and the proof cache. For a contract with many
  survivors (a very weak contract) the equivalence sweep runs once per survivor —
  but a very weak contract gates `WeakContract` regardless of the sweep's outcome
  (the survivors that fail equivalence keep it below floor), so the sweep is
  wasted work only on a pathologically-weak contract. Bounded, deterministic,
  acceptable (§11 accepts slow verification).

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (per-survivor Verus equivalence check) | SHIPPED | The seam `thermite_lower::lower_equivalence_obligation` (`thermite-lower/src/lower.rs`, exported in `lib.rs`) renders `f`'s real body + a survivor's body into the GROUNDED `spec fn equiv_real_<n>` / `spec fn equiv_mut_<n>` + `proof fn equiv_check_<n> requires <req> ensures mut == real {}` Verus unit, REUSING the L3 exec coercions (`lower_expr` + the `(expr) as <ret>` bounded-arith coercion — a naive spec render of `x + 0` over `u64` fails `verus` with `expected u64, found int`, R-CHAR-3 no hand-emit). Consumer: `check::equivalence_proves_equal` (`check.rs`), called per SURVIVOR from `check::mutation_score`. Verified: `thermite-lower/tests/equivalence_obligation.rs` (real verus — equivalent body VERIFIES, distinguishing `x + 1` / `loose` early-return FAIL, non-scalar → `Unsupported`) + `forge/tests/equivalent_mutants_conformance.rs`. |
| REQ-2 (PROVED-equivalent → drop from denominator; sound-but-incomplete) | SHIPPED | `check::mutation_score` runs `equivalence_proves_equal` on each SURVIVOR; a VERIFIED query (`mutant_outcome_is_survivor`/`mutant_cert_is_survivor` true) `continue`s WITHOUT incrementing `scored` (the survivor drops from the denominator) and bumps `MutationScore.equivalent`; an unproven query (counterexample/timeout/un-renderable) increments `scored` (stays counted). Verified: `forge/tests/equivalent_mutants_conformance.rs::ac1_forced_output_excludes_equivalents_and_certifies` (`clamp_zero` 1/3 → 1/1 L3, real verus). |
| REQ-3 (soundness line — distinguishing mutant never excluded) | SHIPPED | Exclusion is gated on `equivalence_proves_equal == Ok(true)` (a Verus PROOF, `0 errors`); a counterexample/timeout/`Unsupported` returns `Ok(false)` → the survivor stays counted. Verified: `forge/tests/equivalent_mutants_conformance.rs::ac2_weak_contract_survivor_stays_counted` (`loose`'s distinguishing early-`return 0` FAILS the query → STILL `WeakContract`, NOT laundered) + the seam test's `distinguishing_offbyone_fails` / `loose_early_return_stays_distinguishing`. |
| REQ-4 (`MutationScore` denominator + `K/N` cert) | SHIPPED | `mutation::MutationScore` gains `equivalent: usize` (the proved-equivalent exclusion count); `scored` is now NET of proved-equivalents, so `kill_ratio = killed / scored` and `mutants_killed_string` reflect the REDUCED denominator. The `0/0` backstop in `kill_ratio` is unchanged (`scored == 0 ⟹ 0.0`). Verified: `clamp_zero` cert `mutants_killed = "1/1"` (AC-1); `refuse` `"0/0"` (AC-3); `loose` below floor (AC-2). |
| REQ-5 (`CHECK_SCHEMA_VERSION` bump) | SHIPPED | `cache::CHECK_SCHEMA_VERSION` bumped `4 → 5` (`cache.rs`, with the schema-history note): the verdict-changing exclusion invalidates stale forced-output verdicts so a `WeakContract` cached under schema 4 is re-scored under the new logic. |
| REQ-6 (determinism + bounded per-survivor cost) | SHIPPED | `equivalence_proves_equal` content-addresses the obligation through the SAME `cache::cache_key`/`load`/`store` (#8) and runs the deterministic pinned-seed/rlimit `run_verus`; ONE extra run PER SURVIVOR (a killed mutant is never queried — `mutation_score` `continue`s before the query). Verified: `forge/tests/equivalent_mutants_conformance.rs::req6_exclusion_is_deterministic` (byte-identical reduced `mutants_killed` across runs). |
