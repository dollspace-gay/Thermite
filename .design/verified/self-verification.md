# Self-Verifying the Toolchain with Verus (Tier 1: the soundness-critical pure core)
<!--
tier: 3-component
status: active (REQ-5 first increment SHIPPED via mechanism (c); epic #60 open for the remaining 7 Tier-1 targets)
governs: thermite-verified/src/lib.rs (the verified core — SCAFFOLDED; `subsumes` proved + anchored, epic #60)
thesis-refs:
  - thermite-design.md §6   (Verus is the L3 prover)
  - thermite-design.md §9   (the TCB is slag ∪ boundary ∪ the toolchain itself)
  - thermite-design.md §7   (the vacuity battery — soundness of the gate)
governs-by-delegation:
  - thermite-lower/src/effects.rs   (the FIRST target — `subsumes`)
  - forge/src/degrade.rs            (`run_ladder` — counterexample-never-degrades)
  - forge/src/cache.rs              (`cache_key` — content addressing)
  - forge/src/vacuity.rs            (`triage` — §7.1 structural checks)
  - forge/src/sandbox.rs            (`syscall_allowlist` — seccomp derivation)
  - forge/src/mutation.rs           (`MutationScore::kill_ratio` / `meets_floor`)
  - forge/src/strengthen.rs         (`is_strictly_stronger`)
-->

## Summary

This component makes the Thermite **toolchain verify itself**. Today the toolchain is
plain Rust; a bug in its *soundness-critical pure core* is not a crash, it is a **false
certificate** — a wrong `subsumes` answer mints a `pure` certificate for an effectful
function. `goal.md` (§9) names the trusted computing base as "exactly (slag blocks ∪
boundary contracts ∪ **the toolchain itself**)". This component SHRINKS that TCB: it
ports the soundness-critical pure decision functions (**Tier 1**) into the Verus
fragment with real `requires`/`ensures` contracts, proves them with the same Verus
prover that `thermite-design.md` §6 names as the L3 rung, and has the toolchain
**delegate** to the verified code — so the code that runs IS the code that was proved.
This is true self-verification: Thermite uses its own L3 prover on its own kernel.

The first proven increment is `effects::subsumes` (the effect-subsumption decision
function). Tier 2 (full functional correctness of `lower` — verified-compiler territory)
and Tier 3 (I/O / `Command`-spawning / heavy-std) are explicitly OUT: Tier 3 is the
trusted floor, sealed behind `#[verifier::external_body]` (Verus's analog of Thermite's
own `#[slag]`/`#[boundary]`), assumed-by-contract.

> **FIRST INCREMENT SHIPPED.** The verified crate (`thermite-verified`) now exists and the
> FIRST Tier-1 target — `effects::subsumes` (REQ-5) — is proved by real `verus
> --no-cheating` (8 verified, 0 errors) and anchored to the toolchain via mechanism (c)
> (a verus-verified core + an exhaustive 65536-pair impl==spec equivalence test; mechanism
> (b) was shown empirically infeasible for v1, OQ-1/OQ-2). REQ-1/3/4/5/6 are SHIPPED; REQ-2
> (the remaining seven Tier-1 targets) is NOT-STARTED, tracked under epic **#60**. The
> grounding section below records the original out-of-tree verus run; the in-tree proof is
> now permanent and CI-runnable (`thermite-verified/tests/verus_verify.rs`).

## The three tiers (the scope boundary)

| Tier | What | This epic | Verus treatment |
|---|---|---|---|
| **Tier 1** | Soundness-critical PURE decision fns (a bug = a false certificate) | **IN — the focus** | ported into `verus!{}` with real `requires`/`ensures`, GENUINELY proved |
| **Tier 2** | Full functional correctness of `lower` (AST→Verus-Rust) | **OUT (research-scale)** | acknowledged, not attempted — this is verified-compiler territory (`thermite-design.md` §11 "Thermite is not a proof assistant") |
| **Tier 3** | I/O, `Command`-spawning (rustc/verus/kani), fs, heavy-std | **OUT (assumed floor)** | `#[verifier::external_body]` / `external` — the trusted boundary, assumed-by-contract |

### Tier-1 target list (the soundness-critical pure core)

Each is a *pure* function whose wrong answer is a soundness hole, and each already ships
as plain Rust (the verification effort is a port + delegation, not a rewrite):

| Target | Symbol (plain Rust today) | Soundness hazard if wrong | thesis |
|---|---|---|---|
| effect subsumption (**FIRST**) | `pub fn subsumes` in `effects.rs` | mints a false `pure` cert for an effectful fn | §4.1 / §9 |
| degrade-ladder classification | `pub fn run_ladder` in `degrade.rs` | a counterexample (`L3Verdict`) silently DEGRADES to a pass | §5.2 / §6 |
| content-addressed cache key | `pub fn cache_key` in `cache.rs` | a stale cert served for changed inputs (collision/under-mixing) | §5.3 |
| §7.1 structural vacuity triage | `pub fn triage` in `vacuity.rs` | a vacuous/trivial contract passes the gate | §7 |
| seccomp allowlist derivation | `pub fn syscall_allowlist` in `sandbox.rs` | the sandbox over-permits (effect escapes) | §4.1 |
| mutation kill-ratio / floor | `MutationScore::kill_ratio` + `meets_floor` in `mutation.rs` | a weak contract scores above the floor | §7 |
| strengthening strictly-stronger | `pub fn is_strictly_stronger` in `strengthen.rs` | a non-stronger candidate suggested as stronger | §7 |
| boundary-composition honesty | the `external_body`-only-for-boundary gate in `lower.rs` | the proof boundary leaks (unverified body treated as verified) | §9 |

The FIRST SHIPPED increment is `subsumes` ALONE (REQ-5). The remaining targets are
NOT-STARTED, ported one at a time AFTER the mechanism is proven end-to-end (REQ-6).

## The chosen mechanism — (b) a verified delegation crate

Three candidate mechanisms were considered; the grounding run (below) settled the choice.

- **(a) `cargo verus verify` on a workspace member in place** (mixed `verus!{}` +
  `#[verifier::external_body]` for the unverifiable rest). REJECTED for v1: the grounding
  run showed `cargo-verus` requires `[package.metadata.verus] verify = true` PLUS path
  deps on the install's `vstd`/`builtin`/`builtin_macros` crates, which themselves inherit
  `workspace.lints` from the Verus workspace root and so fail `cargo metadata` outside that
  workspace (OQ-1). Wiring the whole toolchain workspace through `cargo-verus` is heavy and
  couples every crate's build to the prover. Out for v1.
- **(b) a dedicated `thermite-verified` crate** — the Tier-1 pure fns live in `verus!{}` +
  `vstd`, verified by standalone `verus` (which auto-loads `vstd`, GROUNDED below as
  working), and the toolchain DELEGATES to it (`effects::subsumes` becomes a thin
  re-export / call into `thermite_verified::subsumes`). **CHOSEN.** The verified code IS
  what runs → true self-verification. The standalone `verus` invocation is real and CI-able
  TODAY; the cross-crate *linking* of verified metadata into the toolchain build is the one
  open detail (OQ-2 — `--export`/`--import`, see below).
- **(c) a verified REFERENCE file** verified standalone by `verus`, + a conformance test
  that the toolchain's impl matches it (golden-file style, R-CHAR-3). FALLBACK if (b)'s
  linking proves too heavy for v1: the verified `.rs` is a hand-authored oracle, and a
  conformance test asserts impl-behavior == verified-spec-behavior over an enumerated input
  domain (for `subsumes`: all `(caller, callee)` over the 8-atom bitset, 2^8 × 2^8). Weaker
  (the running code is not literally the proved code, only proved-equivalent), but it makes
  the proof CI-gating immediately and is strictly better than nothing.

**Decision: ship (b) if the `--export`/`--import` linking lands cleanly; otherwise fall
back to (c).** Both keep a REAL `verus` run in the gauntlet. The least-confident decision
is exactly whether (b)'s linking is realistic for v1 vs. having to settle for (c) — see
OQ-2 and "least confident" in the report.

### The delegation/reference pattern (behavior preservation)

- **(b) delegation:** `thermite-verified` exposes a verified `pub fn subsumes(caller, callee)`
  over a Verus-fragment-friendly representation (the 8-atom bitset, see grounding). The
  toolchain's `effects::subsumes` becomes a thin adapter: project `EffectRow` → the 8-bit
  mask, call the verified fn, done. **Behavior MUST be preserved** — the existing
  `effects` tests (`tests/effects.rs`: `lattice_law_reflexive`,
  `lattice_law_pure_subsumes_only_pure`, `lattice_law_top_subsumes_everything`,
  `lattice_law_table`, the accept/reject corpus) MUST still pass unchanged (they encode the
  SAME laws the Verus contract proves — verified at the grounding step: all 14 pass today).
- **(c) reference:** the verified `.rs` is the oracle; a conformance test
  (`conformance/verified/subsumes_matches_verified` or a `#[test]`) enumerates the bitset
  domain and asserts `effects::subsumes` == the verified spec relation. Expected values come
  from the verified spec (an external truth), NOT from the toolchain's own output (R-CHAR-3).

## Requirements

- **REQ-1 (self-verification architecture):** A verified core exists as a `verus!{}` body,
  verified by the same Verus prover that is Thermite's L3 rung (`thermite-design.md` §6),
  via mechanism (b) (a `thermite-verified` crate the toolchain delegates to) or, as a
  documented fallback, (c) (a verified reference + impl==spec conformance test). The chosen
  mechanism is recorded with its build/verify commands. Derived from §6 + §9 (the toolchain
  is in the TCB; self-verification shrinks it).
- **REQ-2 (Tier-1 target list + porting pattern):** The eight soundness-critical pure
  decision functions (the table above) are the in-scope set. Each is ported by projecting
  its inputs to a Verus-fragment representation, carrying a real `requires`/`ensures`, and
  having the toolchain delegate (b) or match (c). Derived from the three-tier scope.
- **REQ-3 (Tier-2/Tier-3 boundaries):** Tier 2 (functional correctness of `lower`) is
  acknowledged and NOT attempted (§11). Tier 3 (I/O, `Command`, fs, heavy-std) is sealed
  behind `#[verifier::external_body]`/`external` — the trusted floor, assumed-by-contract,
  the Verus analog of `#[slag]`/`#[boundary]` (§9). No Tier-3 function is proved; no Tier-1
  *core* function carries `external_body`.
- **REQ-4 (honesty — genuine proof, R-DEFER-9):** The verified core is GENUINELY proved:
  the `verus` run uses `--no-cheating` (no `assume`/`admit`/`external_body` on a core fn);
  the `ensures` is non-trivial (a wrong impl FAILS verification — demonstrated); no
  vacuously-true `ensures`. A vacuous contract IS a divergence (R-CHAR-3 / §7 — the battery
  exists precisely to catch fake contracts).
- **REQ-5 (FIRST increment — `subsumes` verified + delegated/matched):** `effects::subsumes`
  is ported into the verified core with the effect-lattice contract (`result ==
  (effects(callee) ⊆ effects(caller))` + the lattice laws: reflexive, `Pure` subsumes only
  `Pure`, top subsumes all), proved in REAL Verus, and the toolchain delegates to (b) or is
  conformance-matched against (c) it. The existing `effects` tests still pass (behavior
  preserved). Derived from §4.1 + `effect-subsumption.md` REQ-2.
- **REQ-6 (CI-able verus-verify gauntlet step):** The Verus verification of the core runs
  in the gauntlet/CI as a real `verus`/`cargo verus` invocation (like the L3 verus tests in
  the ladder), gating on `verified: N, errors: 0`. A core function that fails to verify is a
  HARD gate failure (R-DEFER-6), not a skip. Derived from §6 + `goal.md` R-DEFER-6.

## Acceptance criteria

- **AC-1 (real verus run verifies `subsumes`):** A `verus --no-cheating <core>` run reports
  `verified: N, errors: 0` (N ≥ 4: the `subsumes` fn + the three lattice-law lemmas) for
  the ported `subsumes`. GROUNDED below: `verification results:: 8 verified, 0 errors`.
- **AC-2 (non-triviality — breaking the impl fails):** Mutating the verified `subsumes`
  body (e.g. `missing == 0` → `missing != 0`) makes the SAME `verus` run report `errors: 1`
  (postcondition not satisfied). GROUNDED below: the broken variant reports
  `7 verified, 1 errors`. This proves the `ensures` is non-vacuous (REQ-4).
- **AC-3 (behavior preserved):** After delegation (b) / matching (c), `cargo test -p
  thermite-lower --test effects` passes with **0 failures** — the verified `subsumes`
  behaves identically to the plain-Rust one (the lattice-law tests encode the proved laws).
  Baseline GROUNDED today: 14 passed, 0 failed.
- **AC-4 (conformance — impl == verified spec):** Under (b), the delegation is the identity
  (same code runs). Under (c), a conformance test enumerates the 8-atom bitset domain
  (2^8 × 2^8 pairs) and asserts `effects::subsumes(caller, callee)` == the verified spec
  relation for every pair, with 0 mismatches; expected values trace to the verified spec,
  never to the impl's own output (R-CHAR-3).
- **AC-5 (Tier-3 floor is the only `external_body`):** A grep over the verified core shows
  `#[verifier::external_body]`/`external` appears ONLY on Tier-3 I/O shims, never on a
  Tier-1 decision function; `--no-cheating` is passed on every verify invocation (REQ-4).
- **AC-6 (CI step is wired):** The gauntlet runs the `verus`/`cargo verus` verification of
  the core and fails the build on `errors > 0` (REQ-6).

## Architecture

The verified core is a `verus!{}` body (mechanism (b): in a `thermite-verified` crate;
mechanism (c): in a standalone reference file). Verus's surface is a Rust subset plus
`spec`/`proof`/`exec` modes; an `exec fn` carrying `ensures` is verified to satisfy it for
ALL inputs (the L3 guarantee, `thermite-design.md` §6). The Tier-1 functions are pure, so
they fit the `exec` fragment after a representation port.

**The representation port.** `EffectRow`/`Effect` (`thermite_syntax::ast`) is a `Vec`-backed
algebraic type — heap-allocating, not in Verus's cheapest fragment. The port projects the
path-insensitive 8-atom *kind* set (`EffectKind` in `effects.rs` — already the atom-kind
projection the v0.1 impl uses, OQ-1 in `effect-subsumption.md`) onto a **bounded bitset
(`u8`)**, one bit per atom (Read=0 … Diverge=7). `subsumes` is then the bit-level mask test
`(callee & !caller) == 0`, and the spec-level subset relation `spec_subsumes` (the genuine
"`effects(callee) ⊆ effects(caller)`") is the explicit 8-way conjunction over the bit
positions. `bit_vector`-mode SMT discharges the equivalence natively. The toolchain's
`EffectKind::of` already performs this exact projection, so the adapter is the existing
projection followed by a fold into the mask.

**Delegation surface (b).** `effects::subsumes` (the existing boundary `pub fn`, grandfathered
under R-DEFER-1) keeps its signature `(caller: &EffectRow, callee: &EffectRow) -> bool` and
becomes a thin adapter over `thermite_verified::subsumes(mask(caller), mask(callee))`. Its
consumer `check_effects` (in `effects.rs`) is unchanged. No new pub API is added to the
toolchain's surface, so behavior preservation (AC-3) is the whole correctness story.

**The trusted floor (Tier 3, §9).** `forge`'s `run_verus`/`run_kani` subprocess shims, the
fs/`Command` paths, and any heavy-std the core transitively touches are `external_body` —
Verus treats them as having an assumed contract, exactly as Thermite's `#[boundary]`/`#[slag]`
treats a foreign body (§9: "Boundary modules are slag-adjacent … the trusted computing base
is enumerable"). The self-verification effort moves the Tier-1 *decision* logic OUT of the
TCB and leaves only the Tier-3 floor in it — the TCB shrinks, which is the point of §9.

## Verification

- **The verus invocation (grounded, CI-able):** `verus --no-cheating <core>.rs` for the
  standalone reference, or `verus --crate-type=lib --export thermite_verified=<path> <core>.rs`
  to produce linkable metadata for delegation (b) (the `--export`/`--import` flag pair is the
  cross-crate link; the exact arg syntax is OQ-2). The gauntlet step gates on
  `verified: N, errors: 0` (REQ-6 / AC-6).
- **Behavior preservation:** `cargo test -p thermite-lower --test effects` (AC-3).
- **Non-triviality:** a CI mutation-sanity check that a deliberately-broken core fails
  verification (AC-2) — or, lighter, a one-time documented demonstration (done in grounding).
- **Conformance (mechanism c):** an enumerated impl==spec test over the bitset domain (AC-4).

### Grounding (REAL verus run — done while authoring this doc)

The `subsumes` port was written and verified with the installed Verus
(`0.2026.05.24.ecee80a`, Z3-backed). The verified core (8-atom `u8` bitset; `spec_subsumes`
= explicit 8-way subset conjunction; `subsumes` = `(callee & !caller)==0` with an `ensures
result == spec_subsumes(caller, callee)`; three lattice-law `proof fn`s):

```
$ verus --no-cheating effects_verus.rs
verification results:: 8 verified, 0 errors
```

Non-triviality check — the body `missing == 0` was mutated to `missing != 0`:

```
$ verus --no-cheating effects_verus_broken.rs
   |     missing != 0  // BROKEN: negated result, must FAIL verification
   |     ------------ at the end of the function body
verification results:: 7 verified, 1 errors   (postcondition not satisfied)
```

So the `ensures` is genuinely constraining (REQ-4 / AC-2). Behavior-preservation baseline:
`cargo test -p thermite-lower --test effects` → **14 passed, 0 failed** today; the three
`lattice_law_*` tests encode the SAME laws the three `proof fn`s prove. The chosen
representation (the `u8` bitset) is exactly the path-insensitive atom-kind projection
`EffectKind::of` already computes, so the delegation adapter is mechanical.

`cargo verus verify` in-place (mechanism a) was tried and rejected: it needs
`[package.metadata.verus] verify = true` + path deps on the install's
`vstd`/`builtin`/`builtin_macros`, which inherit `workspace.lints` from the Verus workspace
root and fail `cargo metadata` outside it (OQ-1). Standalone `verus` (which auto-loads
`vstd`) is the working, CI-able path today.

## Open questions

- **OQ-1 (cargo-verus integration):** wiring the toolchain workspace through `cargo verus
  verify` needs the install's `vstd`/`builtin` crates, which inherit `workspace.lints` and
  fail `cargo metadata` outside the Verus workspace. Is a vendored/published `vstd` dep, or a
  Verus-workspace shim, worth it — or is standalone `verus` the permanent v1 answer? (Drives
  mechanism (a) vs (b).)
- **OQ-2 (delegation linking — the (b)-vs-(c) decider):** can `--export`/`--import` link the
  verified `thermite-verified` metadata into the toolchain's normal `cargo build` cleanly,
  so the proved code is literally the running code? The standalone proof works today, but the
  `--export NAME=PATH` arg syntax did not parse in the grounding attempt (treated the whole
  token as a filename). If linking proves too heavy for v1, fall back to (c) (reference +
  enumerated conformance). LEAST-CONFIDENT decision.
- **OQ-3 (representation fidelity):** the `u8`-bitset port is path-INSENSITIVE (atom-kind
  level), matching the v0.1 `subsumes` (OQ-1 in `effect-subsumption.md`). If/when v0.2 adds
  path-granular subsumption, the verified representation must grow a path lattice — out of
  scope here, but the contract should not bake in path-insensitivity as a SOUNDNESS claim.
- **OQ-4 (port cost for the other 7 targets):** `subsumes` is bitset-clean; `cache_key`
  (sha256 over byte buffers), `triage` (AST walk), and `syscall_allowlist` (string→set maps)
  are heavier ports. Which of the remaining seven are Verus-fragment-friendly without a
  large `vstd` proof, and which need a representation port like `subsumes` got? Ordering
  driven by soundness-hazard severity (REQ-6).

## Routes to add (orchestrator — NOT done here; no Edit to routes)

When the verified crate is scaffolded (epic #60), add to `tooling/spec-routes.toml`:
- `thermite-verified/src/lib.rs` → `.design/verified/self-verification.md` (the verified core)
- (delegation) update the `thermite-lower/src/effects.rs` route to also reference this doc,
  OR add a `reference = [".design/verified/self-verification.md"]` so the delegation edit is
  gated on this contract.

## REQ status

The FIRST increment (REQ-5: `subsumes`) is SHIPPED. The empirical b-vs-c decision
(OQ-1/OQ-2) was settled in-tree: mechanism **(b)** (link the verified crate into
the cargo build) is NOT viable for v1 — the installed `vstd`/`builtin`/`builtin_macros`
crates inherit `workspace.lints` from the Verus workspace root (so `cargo metadata`
fails on them outside it), carry `cfg(verus_keep_ghost)` lint configs cargo rejects,
resolve a renamed `verus_builtin` crate, and a `verus!{}` exec body with an `ensures`
clause is verus-driver-only syntax (plain `rustc` cannot compile it). So we landed
mechanism **(c)** exactly as the decision rule prescribes: the `verus!{}` proof lives
behind `#[cfg(verus_keep_ghost)]` in `thermite-verified/src/lib.rs` (verified by
`thermite-verified/tests/verus_verify.rs` running real `verus --no-cheating
--crate-type=lib src/lib.rs` → **8 verified, 0 errors**), and the always-cargo-compiled
plain-Rust mirror (`subsumes_masks` / `spec_subsumes_mask`) is what runs;
`thermite_lower::effects::subsumes` delegates its bit comparison to `subsumes_masks`
and is anchored to the proved subset relation by the exhaustive 2^8×2^8 = 65536-pair
equivalence test `thermite-lower/tests/effects_verified.rs` (0 mismatches).

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (self-verification architecture) | SHIPPED | `verus_core` in `thermite-verified/src/lib.rs` (the `verus!{}` body, verified by `verus`, Thermite's L3 rung §6); mechanism (c) landed + recorded (b empirically infeasible, see above); `tests/verus_verify.rs` runs `verus --no-cheating` → 8 verified, 0 errors. |
| REQ-2 (Tier-1 target list + porting pattern) | NOT-STARTED | epic #60. `subsumes` is the FIRST and ONLY ported target (REQ-5); the other seven Tier-1 fns (`run_ladder`, `cache_key`, `triage`, `syscall_allowlist`, `kill_ratio`/`meets_floor`, `is_strictly_stronger`, the boundary gate) remain plain Rust, ported one at a time AFTER the mechanism is proven (REQ-6). |
| REQ-3 (Tier-2/Tier-3 boundaries) | SHIPPED | `thermite-verified` has NO I/O and NO `external_body`/`external` (AC-5 grep: zero occurrences in `src/`); the Tier-1 core (`subsumes`) carries a real `ensures`, reaching no Tier-3 floor. Tier 2 acknowledged, not attempted. |
| REQ-4 (honesty — genuine proof) | SHIPPED | `verus --no-cheating` on the core (no `assume`/`admit`/`external_body`); `ensures result == spec_subsumes(..)` is non-vacuous — negating the body (`missing == 0` → `missing != 0`) → `7 verified, 1 errors`, demonstrated by `tests/verus_verify.rs::broken_subsumes_fails_verification`; `effects_verified.rs::verified_spec_is_not_vacuous` re-checks the relation rejects Pure⊇{Read}. |
| REQ-5 (FIRST increment — `subsumes` verified + delegated/matched) | SHIPPED | `verus_core::subsumes` proved (+ three lattice-law `proof fn`s: reflexive / Pure-subsumes-only-Pure / top-subsumes-all); `thermite_verified::subsumes_masks` (the plain mirror) consumed by `thermite_lower::effects::subsumes`; matched by the 65536-pair exhaustive equivalence test (mechanism (c), AC-4, 0 mismatches); the 14 `effects` tests still pass (behavior preserved, AC-3). |
| REQ-6 (CI-able verus-verify gauntlet step) | SHIPPED | `thermite-verified/tests/verus_verify.rs` runs real `verus --no-cheating --crate-type=lib src/lib.rs` (skip-loud if verus absent, like `lower_conformance`) and asserts `verified, 0 errors`; a core fn that fails to verify is a HARD test failure (R-DEFER-6). |
