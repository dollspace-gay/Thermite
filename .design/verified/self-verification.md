# Self-Verifying the Toolchain with Verus (Tier 1: the soundness-critical pure core)
<!--
tier: 3-component
status: active (REQ-5/7/8 increments designed; REQ-5 `subsumes` SHIPPED via mechanism (c); REQ-7 `ladder_action` + REQ-8 `syscall_allowlist` NOT-STARTED, grounded; epic #60 open for the remaining 5 Tier-1 targets)
governs: thermite-verified/src/lib.rs (the verified core — `subsumes` proved + anchored; `ladder_action`/`syscall_allowlist` to be ported, epic #60)
thesis-refs:
  - thermite-design.md §6   (Verus is the L3 prover)
  - thermite-design.md §9   (the TCB is slag ∪ boundary ∪ the toolchain itself)
  - thermite-design.md §7   (the vacuity battery — soundness of the gate)
  - thermite-design.md §5.2 (the gate degrades, never blocks — the anti-cheat ladder)
  - thermite-design.md §4.1 (the fx row is a runtime contract — the sandbox)
governs-by-delegation:
  - thermite-lower/src/effects.rs   (the FIRST target — `subsumes`)
  - forge/src/degrade.rs            (`run_ladder` — counterexample-never-degrades; the `ladder_action` decision core, REQ-7)
  - forge/src/cache.rs              (`cache_key` — content addressing)
  - forge/src/vacuity.rs            (`triage` — §7.1 structural checks)
  - forge/src/sandbox.rs            (`syscall_allowlist` — seccomp derivation, REQ-8)
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
function). This iteration adds the next two highest-value finite-domain targets via the
SAME proven mechanism (c): **(REQ-7)** the degrade-ladder **anti-cheat** (a
`Counterexample` NEVER degrades — the core R-DEFER-9 property) and **(REQ-8)** the
**seccomp allowlist soundness** (a `pure` filter permits no user-I/O syscall, and the
allowlist is MONOTONE in the effect set). Tier 2 (full functional correctness of
`lower` — verified-compiler territory) and Tier 3 (I/O / `Command`-spawning / heavy-std)
are explicitly OUT: Tier 3 is the trusted floor, sealed behind
`#[verifier::external_body]` (Verus's analog of Thermite's own `#[slag]`/`#[boundary]`),
assumed-by-contract.

> **THREE INCREMENTS DESIGNED, ONE SHIPPED.** The verified crate (`thermite-verified`)
> exists and the FIRST Tier-1 target — `effects::subsumes` (REQ-5) — is proved by real
> `verus --no-cheating` (8 verified, 0 errors) and anchored to the toolchain via mechanism
> (c) (a verus-verified core + an exhaustive 65536-pair impl==spec equivalence test). This
> iteration GROUNDS the next two targets with real `verus` runs (see Grounding A and B
> below) but leaves them **NOT-STARTED** (no in-tree port/anchor yet): REQ-7
> (`degrade::ladder_action`, the anti-cheat) and REQ-8 (`sandbox::syscall_allowlist`, the
> seccomp soundness). REQ-1/3/4/5/6 are SHIPPED; REQ-2/7/8 are NOT-STARTED, tracked under
> epic **#60**. The grounding sections record the REAL out-of-tree verus runs that prove
> each is verus-fragment-friendly and that the contracts are non-vacuous.

## The three tiers (the scope boundary)

| Tier | What | This epic | Verus treatment |
|---|---|---|---|
| **Tier 1** | Soundness-critical PURE decision fns (a bug = a false certificate) | **IN — the focus** | ported into `verus!{}` with real `requires`/`ensures`, GENUINELY proved |
| **Tier 2** | Full functional correctness of `lower` (AST→Verus-Rust) | **OUT (research-scale)** | acknowledged, not attempted — this is verified-compiler territory (`thermite-design.md` §11 "Thermite is not a proof assistant") |
| **Tier 3** | I/O, `Command`-spawning (rustc/verus/kani), fs, heavy-std | **OUT (assumed floor)** | `#[verifier::external_body]` / `external` — the trusted boundary, assumed-by-contract |

### Tier-1 target list (the soundness-critical pure core)

Each is a *pure* function whose wrong answer is a soundness hole, and each already ships
as plain Rust (the verification effort is a port + delegation, not a rewrite):

| Target | Symbol (plain Rust today) | Soundness hazard if wrong | thesis | REQ |
|---|---|---|---|---|
| effect subsumption (**SHIPPED**) | `pub fn subsumes` in `effects.rs` | mints a false `pure` cert for an effectful fn | §4.1 / §9 | REQ-5 |
| degrade-ladder anti-cheat (**NEXT, grounded**) | `pub fn run_ladder` in `degrade.rs` (verifiable core `ladder_action`) | a counterexample (`L3Verdict`/`L2Verdict`) silently DEGRADES to a pass | §5.2 / §6 | **REQ-7** |
| seccomp allowlist derivation (**NEXT, grounded**) | `pub fn syscall_allowlist` in `sandbox.rs` | the sandbox over-permits (effect escapes) or a pure filter leaks I/O | §4.1 | **REQ-8** |
| content-addressed cache key | `pub fn cache_key` in `cache.rs` | a stale cert served for changed inputs (collision/under-mixing) | §5.3 | REQ-2 |
| §7.1 structural vacuity triage | `pub fn triage` in `vacuity.rs` | a vacuous/trivial contract passes the gate | §7 | REQ-2 |
| mutation kill-ratio / floor | `MutationScore::kill_ratio` + `meets_floor` in `mutation.rs` | a weak contract scores above the floor | §7 | REQ-2 |
| strengthening strictly-stronger | `pub fn is_strictly_stronger` in `strengthen.rs` | a non-stronger candidate suggested as stronger | §7 | REQ-2 |
| boundary-composition honesty | the `external_body`-only-for-boundary gate in `lower.rs` | the proof boundary leaks (unverified body treated as verified) | §9 | REQ-2 |

The SHIPPED increment is `subsumes` ALONE (REQ-5). `ladder_action` (REQ-7) and
`syscall_allowlist` (REQ-8) are the next two, GROUNDED here and NOT-STARTED in-tree. The
remaining five stay under REQ-2.

## The chosen mechanism — (c) exhaustive equivalence (proven by the `subsumes` increment)

Three candidate mechanisms were considered; the `subsumes` grounding run settled the choice.

- **(a) `cargo verus verify` on a workspace member in place** (mixed `verus!{}` +
  `#[verifier::external_body]` for the unverifiable rest). REJECTED for v1: the grounding
  run showed `cargo-verus` requires `[package.metadata.verus] verify = true` PLUS path
  deps on the install's `vstd`/`builtin`/`builtin_macros` crates, which themselves inherit
  `workspace.lints` from the Verus workspace root and so fail `cargo metadata` outside that
  workspace (OQ-1). Out for v1.
- **(b) a dedicated `thermite-verified` crate** — the Tier-1 pure fns live in `verus!{}` +
  `vstd`, verified by standalone `verus`, and the toolchain DELEGATES to it. REJECTED for
  v1: the cross-crate *linking* of verified metadata into the toolchain build (`--export`/
  `--import`) proved infeasible (OQ-2).
- **(c) a verified REFERENCE in `thermite-verified/src/lib.rs`** verified standalone by
  `verus` (the `verus!{}` body behind `#[cfg(verus_keep_ghost)]`), + a plain-Rust mirror
  the toolchain runs, + a conformance test that the toolchain's impl matches the verified
  spec over the ENUMERATED finite input domain (R-CHAR-3). **CHOSEN and SHIPPED** for
  `subsumes` (2^8 × 2^8 = 65536 pairs). The running code is proved-EQUIVALENT to the
  verified spec over every input — finite + fully enumerated, so the equivalence is total.

**Decision (locked by the `subsumes` increment): ship (c).** Both new targets (REQ-7,
REQ-8) reuse mechanism (c) IDENTICALLY — a `verus!{}` core behind `#[cfg(verus_keep_ghost)]`,
a plain-Rust mirror, and an exhaustive impl==spec test binding the PRODUCTION fn over its
finite domain. The finite domains are tiny: REQ-7's is the verdict enum (3 L3 tags × 3 L2
tags); REQ-8's is the 2^8 fx-atom masks (the same enumeration style as `subsumes`' 65536).

## Requirements

- **REQ-1 (self-verification architecture):** A verified core exists as a `verus!{}` body,
  verified by the same Verus prover that is Thermite's L3 rung (`thermite-design.md` §6),
  via mechanism (c) (a verified reference + impl==spec conformance test). The chosen
  mechanism is recorded with its build/verify commands. Derived from §6 + §9.
- **REQ-2 (remaining Tier-1 targets + porting pattern):** The remaining soundness-critical
  pure decision functions (`cache_key`, `triage`, `kill_ratio`/`meets_floor`,
  `is_strictly_stronger`, the boundary gate) are the in-scope set, ported by projecting
  inputs to a Verus-fragment representation, carrying a real `requires`/`ensures`, and
  anchoring the toolchain impl by exhaustive equivalence (c). Derived from the three-tier
  scope. (`subsumes` = REQ-5; `ladder_action` = REQ-7; `syscall_allowlist` = REQ-8.)
- **REQ-3 (Tier-2/Tier-3 boundaries):** Tier 2 (functional correctness of `lower`) is
  acknowledged and NOT attempted (§11). Tier 3 (I/O, `Command`, fs, heavy-std) is sealed
  behind `#[verifier::external_body]`/`external` — the trusted floor, assumed-by-contract
  (§9). No Tier-3 function is proved; no Tier-1 *core* function carries `external_body`.
- **REQ-4 (honesty — genuine proof, R-DEFER-9):** The verified core is GENUINELY proved:
  the `verus` run uses `--no-cheating` (no `assume`/`admit`/`external_body` on a core fn);
  the `ensures` is non-trivial (a wrong impl FAILS verification — demonstrated for EACH
  target); no vacuously-true `ensures`. A vacuous contract IS a divergence (R-CHAR-3 / §7).
- **REQ-5 (`subsumes` verified + matched):** `effects::subsumes` is ported into the verified
  core with the effect-lattice contract, proved in REAL Verus, and the toolchain is
  conformance-matched against it (mechanism (c)). The existing `effects` tests still pass.
  Derived from §4.1 + `effect-subsumption.md` REQ-2. **SHIPPED.**
- **REQ-6 (CI-able verus-verify gauntlet step):** The Verus verification of the core runs
  in the gauntlet/CI as a real `verus`/`cargo verus` invocation, gating on `verified: N,
  errors: 0`. A core function that fails to verify is a HARD gate failure (R-DEFER-6).
  Derived from §6 + `goal.md` R-DEFER-6.

- **REQ-7 (degrade-ladder ANTI-CHEAT verified + anchored — the core R-DEFER-9 property):**
  The degrade-ladder's verifiable decision core is ported into the verified `verus!{}`
  body as a PURE classification `ladder_action(verdict) -> LadderAction`, where
  `LadderAction ∈ {CertifyL3, AttemptL2, CertifyL2, DegradeToL1, HardFail}`. The
  **anti-cheat invariant** is proved as a real `ensures`: a `Counterexample` (L3 OR L2)
  maps to `HardFail` and NEVER to a degrade action (`CertifyL2`/`DegradeToL1`) — formally
  `l3_is_counterexample(v) ==> (result is HardFail) && !is_degrade(result)` (and the L2
  analog). `run_ladder` (`forge/src/degrade.rs`) DELEGATES its branching to the extracted
  `ladder_action` so the proved decision drives the real control flow, and is anchored by
  an exhaustive equivalence test over the verdict enum (3 L3 tags × 3 L2 tags) binding the
  PRODUCTION decision (R-CHAR-3). Derived from `.design/forge/degrade-ladder.md` REQ-2 +
  `thermite-design.md` §5.2 + `goal.md` R-DEFER-9 / R-CODE-4. **NOT-STARTED** (grounded
  below; epic #60).

- **REQ-8 (seccomp allowlist SOUNDNESS verified + anchored):** The `fx`-atom-set →
  syscall-set mapping (`sandbox::syscall_allowlist`) is ported into the verified `verus!{}`
  body as a **bitset map** over the ~8 fx-atom kinds (`u8` fx-mask) → a membership over the
  sensitive user-I/O syscalls (`openat`/`socket`/`connect`/`getrandom`/`clock_gettime`),
  carrying two real `ensures` soundness lemmas: **(1) PURE-NO-I/O** — an empty fx-mask
  (`pure`, no widening atom) maps to a syscall set containing NO user-I/O syscall
  (`io_allow(0) == 0`); **(2) MONOTONICITY** — `fx ⊆ fx'` (bitset subset) ⟹
  `allowlist(fx) ⊆ allowlist(fx')` (adding an effect NEVER removes a permitted syscall,
  and never silently grants outside the sensitive set — deny-by-default holds). The
  production `syscall_allowlist` is anchored by exhaustive equivalence over the 2^8
  fx-atom-masks (the same enumeration style as `subsumes`' 65536), binding the PRODUCTION
  fn to the proved bitset spec (R-CHAR-3). Derived from `.design/forge/runtime-sandbox.md`
  REQ-3 + `thermite-design.md` §4.1. **NOT-STARTED** (grounded below; epic #60).

## Acceptance criteria

- **AC-1 (real verus run verifies `subsumes`):** `verus --no-cheating <core>` reports
  `verified: N, errors: 0` (N ≥ 4) for the ported `subsumes`. GROUNDED: `8 verified, 0 errors`.
- **AC-2 (non-triviality — breaking the impl fails):** Mutating the verified `subsumes`
  body makes the SAME run report `errors: 1`. GROUNDED: the broken variant reports `7
  verified, 1 errors`.
- **AC-3 (behavior preserved):** After matching, `cargo test -p thermite-lower --test
  effects` passes with **0 failures**. Baseline GROUNDED: 14 passed, 0 failed.
- **AC-4 (conformance — impl == verified spec):** A conformance test enumerates the 8-atom
  bitset domain (2^8 × 2^8 pairs) and asserts `effects::subsumes` == the verified spec
  relation for every pair, 0 mismatches; expected values trace to the verified spec (R-CHAR-3).
- **AC-5 (Tier-3 floor is the only `external_body`):** A grep over the verified core shows
  `#[verifier::external_body]`/`external` appears ONLY on Tier-3 I/O shims, never on a
  Tier-1 decision function; `--no-cheating` is passed on every verify invocation (REQ-4).
- **AC-6 (CI step is wired):** The gauntlet runs the `verus`/`cargo verus` verification of
  the core and fails the build on `errors > 0` (REQ-6).

- **AC-7 (degrade anti-cheat verified + non-vacuous + anchored — REQ-7):**
  - **AC-7a:** `verus --no-cheating <core>` verifies the `ladder_action_l3` /
    `ladder_action_l2` exec fns + the global anti-cheat `proof fn` with `0 errors`.
    GROUNDED below: `3 verified, 0 errors`.
  - **AC-7b (non-vacuity):** mutating the decision so a `Counterexample` maps to
    `DegradeToL1` makes the SAME run report `errors ≥ 1` (the anti-cheat `ensures` fails).
    GROUNDED below: the broken variant reports `2 verified, 1 errors` ("failed this
    postcondition").
  - **AC-7c (production-anchoring):** `run_ladder` delegates its `match` to the extracted
    `ladder_action`, and an exhaustive equivalence test enumerates EVERY verdict
    combination (3 L3 tags, and on `Timeout` 3 L2 tags) and asserts the production ladder's
    achieved level/outcome equals the proved decision — in particular that EVERY
    `Counterexample` path returns the non-certifying hard-fail cert (`Level::L0`,
    `!lowered_assurance`, no degrade reason), 0 mismatches, expected from the proved spec
    (R-CHAR-3, never forge's own output).
  - **AC-7d (regression guard):** the existing hermetic tests
    `degrade::tests::counterexample_never_degrades` and `l2_counterexample_never_drops_to_l1`
    still pass unchanged (behavior preserved).

- **AC-8 (seccomp soundness verified + non-vacuous + anchored — REQ-8):**
  - **AC-8a:** `verus --no-cheating <core>` verifies the `pure_has_no_io`,
    `non_widening_atoms_have_no_io`, `monotone`, and `io_allow_within_io_bits` `proof fn`s
    with `0 errors`. GROUNDED below: `15 verified, 0 errors`.
  - **AC-8b (non-vacuity — BOTH lemmas):** mutating the spec so a non-widening atom leaks
    `openat` makes `pure_has_no_io` fail; mutating `io_allow` to XOR (so `Write` cancels
    `Read`'s `openat`) makes `monotone` fail. GROUNDED below: each broken variant reports
    `14 verified, 1 errors`.
  - **AC-8c (production-anchoring):** an exhaustive equivalence test enumerates all 2^8
    fx-atom masks, projects each to the production token set, and asserts
    `sandbox::syscall_allowlist(tokens)` membership over the sensitive syscalls equals the
    proved `io_allow(mask)` bits for every mask, 0 mismatches; in particular `pure` (mask 0)
    contains none of `openat`/`socket`/`connect`/`getrandom`/`clock_gettime`, and any
    superset of fx never drops a syscall. Expected from the proved bitset spec (R-CHAR-3).
  - **AC-8d (regression guard):** the existing `sandbox::tests::pure_baseline_excludes_io_syscalls`,
    `read_fx_widens_to_openat`, `widening_tokens_cover_the_family`, and the
    `sandbox_conformance` oracle (`pure_runs_clean`, `probe_killed`,
    `probe_allowed_when_fx_widens`) still pass unchanged.

## Architecture

The verified core is a `verus!{}` body in `thermite-verified/src/lib.rs` (mechanism (c)),
gated behind `#[cfg(verus_keep_ghost)]` so a normal `cargo build` skips it and only the
`verus` driver compiles it. Verus's surface is a Rust subset plus `spec`/`proof`/`exec`
modes; an `exec fn` carrying `ensures` is verified to satisfy it for ALL inputs (the L3
guarantee, `thermite-design.md` §6). The Tier-1 functions are pure, so they fit the `exec`
fragment after a representation port.

**`subsumes` (REQ-5).** `EffectKind` (8 atoms) → a `u8` bitset; `subsumes` is the mask test
`(callee & !caller) == 0`; the spec subset relation is the explicit 8-way conjunction;
`bit_vector`-mode SMT discharges the equivalence. The toolchain's `EffectKind::of` already
performs this projection.

**`ladder_action` (REQ-7) — the degrade anti-cheat core.** `run_ladder` (`degrade.rs`) is
higher-order (it takes `attempt_l2`/`attempt_l1` closures) so it is NOT directly
verus-able. The VERIFIABLE CORE is a PURE decision: the verdict discriminant → an action.
The verus model carries the discriminant only (the carried `Certificate`/`RejectReason`
payloads are irrelevant to the *decision*, exactly the finite domain that drives the
branch): `L3Tag ∈ {Proved, Timeout, Counterexample}`, `L2Tag ∈ {Verified, UnderBound,
Counterexample}`, `LadderAction ∈ {CertifyL3, AttemptL2, CertifyL2, DegradeToL1,
HardFail}`. `is_degrade(a)` is true for `CertifyL2`/`DegradeToL1` (the rungs taken as a
PASS). The anti-cheat `ensures` is `l3_is_counterexample(v) ==> (r is HardFail) &&
!is_degrade(r)` (and the L2 analog), plus a global `proof fn` quantifying it over the whole
finite domain. The production extraction: `degrade::ladder_action_l3(&L3Verdict) ->
LadderAction` and `ladder_action_l2(L2Verdict) -> LadderAction` are pulled out of
`run_ladder`'s `match`, and `run_ladder` re-expresses its control flow by matching on the
returned `LadderAction` (so the proved decision DRIVES the real branching, `goal.md`
R-DEFER-9). The closures stay (they perform the actual L2/L1 attempts) but the
classification that decides whether to degrade is the proved fn.

**`syscall_allowlist` (REQ-8) — the seccomp soundness core.** The fx-atom kinds map to bit
positions in a `u8` fx-mask (Read=0, Write=1, Net=2, Time=3, Rand=4, Alloc=5, Panic=6,
Diverge=7). The verus model tracks membership over the *sensitive* user-I/O syscalls as a
`u32` syscall-mask (openat=bit0, socket=bit1, connect=bit2, getrandom=bit3,
clock_gettime=bit4); the dense baseline (read/write/mmap/exit/…) is orthogonal to these IO
bits, so the soundness model is the IO-membership projection. `widen(i)` is the per-atom
contribution (Read/Write→openat, Net→socket|connect, Time→clock_gettime, Rand→getrandom,
Alloc/Panic/Diverge→0); `io_allow(fx)` ORs every present atom's `widen`. The two lemmas:
`pure_has_no_io` (`io_allow(0) == 0`) and `monotone` (`(fx & fx') == fx ⟹ (io_allow(fx) &
io_allow(fx')) == io_allow(fx)`, i.e. subset on the syscall-mask), plus
`io_allow_within_io_bits` (deny-by-default — `io_allow` never sets a bit outside the
sensitive set). `bit_vector`-mode SMT discharges all three. The production extraction:
`sandbox::syscall_allowlist` keeps its `BTreeSet<String>`→`Vec<u32>` signature; an
equivalence test projects each of the 2^8 fx-atom masks to the production token set
(`read(_)`/`write(_)`/`net(_)`/`time`/`rand`/`alloc`/`panic`/`diverge`), runs
`syscall_allowlist`, and asserts membership over the five sensitive syscalls equals the
proved `io_allow(mask)` bits — anchoring the production string-keyed mapping to the proved
bitset spec.

**The trusted floor (Tier 3, §9).** `forge`'s `run_verus`/`run_kani` subprocess shims, the
fs/`Command` paths, and any heavy-std the core transitively touches are `external_body`.
The self-verification effort moves the Tier-1 *decision* logic OUT of the TCB and leaves
only the Tier-3 floor in it — the TCB shrinks, which is the point of §9.

## Verification

- **The verus invocation (grounded, CI-able):** `verus --no-cheating --crate-type=lib
  src/lib.rs` for the verified core; the gauntlet step gates on `verified: N, errors: 0`
  (REQ-6 / AC-6), run by `thermite-verified/tests/verus_verify.rs` (skip-LOUD if verus
  absent, temp-dir cwd so no scratch lands in the tree, #53).
- **Behavior preservation:** `cargo test -p thermite-lower --test effects` (AC-3);
  `cargo test -p forge degrade::tests` (AC-7d); `cargo test -p forge sandbox::tests` +
  the `sandbox_conformance` oracle (AC-8d).
- **Non-triviality:** a CI mutation-sanity check that each deliberately-broken core fails
  verification (AC-2 / AC-7b / AC-8b) — pattern: `tests/verus_verify.rs` writes a mutated
  temp copy of `lib.rs` and asserts the SAME `verus --no-cheating` run reports an error.
- **Conformance (mechanism c):** the enumerated impl==spec tests over the finite domains —
  `subsumes` 65536 pairs (AC-4); `ladder_action` the 3×3 verdict enum (AC-7c);
  `syscall_allowlist` the 2^8 fx-atom masks (AC-8c).

### Grounding (REAL verus run — `subsumes`, mechanism (c))

The `subsumes` port (8-atom `u8` bitset; `spec_subsumes` = explicit 8-way subset
conjunction; `subsumes` = `(callee & !caller)==0` with `ensures result ==
spec_subsumes(..)`; three lattice-law `proof fn`s) verifies with the installed Verus
(`0.2026.05.24.ecee80a`, Z3-backed):

```
$ verus --no-cheating effects_verus.rs
verification results:: 8 verified, 0 errors
```

Non-triviality — body `missing == 0` mutated to `missing != 0`:

```
$ verus --no-cheating effects_verus_broken.rs
verification results:: 7 verified, 1 errors   (postcondition not satisfied)
```

Behavior-preservation baseline: `cargo test -p thermite-lower --test effects` → **14
passed, 0 failed**. This is the SHIPPED increment; the in-tree proof + 65536-pair anchor
are permanent (`thermite-verified/src/lib.rs`, `thermite-lower/tests/effects_verified.rs`).

### Grounding A (REAL verus run — `ladder_action`, REQ-7, the anti-cheat)

The `ladder_action` decision was ported into a `verus!{}` form: `L3Tag`/`L2Tag`/
`LadderAction` enums, an `is_degrade` spec, the `l3_is_counterexample`/`l2_is_counterexample`
specs, the two exec decision fns carrying the anti-cheat `ensures`
(`l3_is_counterexample(v) ==> (r is HardFail) && !is_degrade(r)` + the L2 analog), and a
global `anti_cheat_holds_for_all_verdicts` `proof fn` quantifying it over the whole verdict
domain. Verified with the installed Verus:

```
$ verus --no-cheating --crate-type=lib ladder_action_verus.rs
verification results:: 3 verified, 0 errors
```

Non-triviality (AC-7b) — the `ladder_action_l3` body's `Counterexample` arm was mutated
from `HardFail` to `DegradeToL1` (a counterexample DEGRADES — the exact cheat R-DEFER-9
forbids); the anti-cheat `ensures` then fails:

```
$ verus --no-cheating --crate-type=lib ladder_action_broken.rs
   |  L3Tag::Counterexample => LadderAction::DegradeToL1,  // BROKEN
   |  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ failed this postcondition
verification results:: 2 verified, 1 errors
```

So the anti-cheat invariant is genuinely constraining (REQ-4 / R-DEFER-9). The verus core
models the verdict DISCRIMINANT only — the `Certificate`/`RejectReason` payloads
`run_ladder` carries are irrelevant to the degrade DECISION, so the proved finite domain
(3 L3 tags × 3 L2 tags) is exactly the branch driver. (Scratch verus files written to
`/tmp` and removed, #53.)

### Grounding B (REAL verus run — `syscall_allowlist`, REQ-8, the seccomp soundness)

The fx→syscall mapping was ported into a `verus!{}` bitset form: a `u8` fx-mask
(Read=0..Diverge=7), a `widen(i)` per-atom syscall-bit contribution over the five sensitive
syscalls (openat/socket/connect/getrandom/clock_gettime), an `io_allow(fx)` that ORs the
present atoms' contributions, and four `proof fn`s — `pure_has_no_io` (`io_allow(0) == 0`),
`non_widening_atoms_have_no_io`, `monotone` (`(fx & fx') == fx ⟹ (io_allow(fx) &
io_allow(fx')) == io_allow(fx)`), and `io_allow_within_io_bits` (deny-by-default). All
discharge via `bit_vector`-mode SMT:

```
$ verus --no-cheating --crate-type=lib sandbox_verus.rs
verification results:: 15 verified, 0 errors
```

Non-triviality (AC-8b) — BOTH lemmas were broken independently:

```
# (1) non-widening atom leaks openat (else { 0 } -> else { OPENAT }): pure_has_no_io fails
$ verus --no-cheating --crate-type=lib sandbox_broken1.rs
verification results:: 14 verified, 1 errors

# (2) io_allow uses XOR so Write cancels Read's openat (non-monotone): monotone fails
$ verus --no-cheating --crate-type=lib sandbox_broken2.rs
verification results:: 14 verified, 1 errors
```

So PURE-NO-I/O and MONOTONICITY are each genuinely constraining (REQ-4). The bitset model
covers the soundness-relevant projection (the five user-I/O syscalls the
`runtime-sandbox.md` REQ-3 table calls out as the `pure`-excluded set); the dense baseline
is orthogonal to these IO bits, so modeling IO membership is the soundness story. The
production `syscall_allowlist` is string-keyed (`read(src)` → the `read` widening); the
2^8-mask equivalence test (AC-8c) maps each mask to the production tokens and binds the two
representations. (Scratch verus files written to `/tmp` and removed, #53.)

## Open questions

- **OQ-1 (cargo-verus integration):** unchanged — standalone `verus` is the permanent v1
  answer; the toolchain workspace cannot consume the install's `vstd`/`builtin` as path-deps.
- **OQ-2 (delegation linking):** settled — mechanism (b) is not viable; (c) is the landed
  pattern for ALL Tier-1 targets.
- **OQ-3 (representation fidelity):** the `u8`-bitset ports (subsumes, syscall_allowlist) and
  the discriminant model (ladder_action) are path-INSENSITIVE / payload-insensitive, matching
  the v0.1 impls. The contracts must not bake in those simplifications as SOUNDNESS claims
  beyond what the impl computes.
- **OQ-5 (ladder_action extraction fidelity — LEAST CONFIDENT for REQ-7):** the verus core
  proves the DECISION (verdict→action). The production anchor requires `run_ladder` to
  delegate its `match` to the extracted `ladder_action` AND for the action→control-flow
  mapping (e.g. `HardFail` → "return the carried cert unchanged, run no closure") to be
  faithfully exercised by the equivalence test. The risk is a gap between "the decision is
  proved" and "the closures are wired to honor the decision" — the equivalence test (AC-7c)
  must assert the OBSERVABLE outcome (achieved level + no-degrade-stamp + closures-not-run),
  not just that `ladder_action` returns `HardFail`. The `attempt_l2`/`attempt_l1` closures
  stay unproved (Tier-3-adjacent); only the classification is proved.
- **OQ-6 (syscall bitset fidelity — REQ-8):** the verus model proves soundness over the FIVE
  sensitive syscalls. The equivalence test (AC-8c) binds the production string→`Vec<u32>`
  mapping to the bitset spec ONLY over those five (plus a baseline-membership invariant); it
  does NOT prove the dense baseline list itself is correct (that is empirically grounded by
  the `sandbox_conformance` oracle, not verus). If a future widening adds a syscall outside
  the modeled five, the bitset must grow a bit — the contract should not claim completeness
  over ALL syscalls, only the soundness properties (pure-no-I/O, monotonicity) over the
  modeled sensitive set.

## Routes to add (orchestrator — NOT done here; no Edit to routes)

The verified crate is routed: `thermite-verified/src/lib.rs` → this doc. For REQ-7/REQ-8 the
builder will touch (orchestrator adds/extends routes as needed):
- `thermite-verified/src/lib.rs` (EXTEND — add the `ladder_action` + `syscall_allowlist`
  verus cores + their plain-Rust mirrors, behind the same `#[cfg(verus_keep_ghost)]` split).
- `forge/src/degrade.rs` (extract `ladder_action_l3`/`ladder_action_l2` and delegate
  `run_ladder`'s `match` to them — REQ-7; route references this doc).
- `forge/src/sandbox.rs` (anchor `syscall_allowlist` to the proved bitset spec — REQ-8;
  route references this doc).
- the equivalence tests: `forge/tests/ladder_action_verified.rs` (the 3×3 verdict
  enumeration) and `forge/tests/sandbox_verified.rs` (the 2^8 fx-mask enumeration), each
  binding the PRODUCTION fn to the verified spec (R-CHAR-3).
- `thermite-verified/tests/verus_verify.rs` (EXTEND — assert the new cores verify + add the
  two non-triviality mutation checks).

## REQ status

The SHIPPED increment is REQ-5 (`subsumes`). REQ-7 (`ladder_action`, the anti-cheat) and
REQ-8 (`syscall_allowlist`, the seccomp soundness) are GROUNDED (the verus ports + the
non-triviality mutations were RUN with the installed `verus --no-cheating`, results pasted
in Grounding A/B) but **NOT-STARTED in-tree** — no `verus_core` extension, no
`run_ladder`/`syscall_allowlist` anchoring, no equivalence test has landed yet. The
mechanism is (c), proven end-to-end by REQ-5. Epic **#60** owns all remaining Tier-1
porting (no separate blocker filed — #60 is the tracker).

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (self-verification architecture) | SHIPPED | `verus_core` in `thermite-verified/src/lib.rs` (the `verus!{}` body, verified by `verus`, Thermite's L3 rung §6); mechanism (c) landed + recorded; `tests/verus_verify.rs` runs `verus --no-cheating` → 8 verified, 0 errors. |
| REQ-2 (remaining Tier-1 targets) | NOT-STARTED | epic #60. The remaining FIVE Tier-1 fns (`cache_key`, `triage`, `kill_ratio`/`meets_floor`, `is_strictly_stronger`, the boundary gate) remain plain Rust, ported one at a time via mechanism (c). |
| REQ-3 (Tier-2/Tier-3 boundaries) | SHIPPED | `thermite-verified` has NO I/O and NO `external_body`/`external` (AC-5 grep: zero in `src/`); the Tier-1 core carries a real `ensures`, reaching no Tier-3 floor. Tier 2 acknowledged, not attempted. |
| REQ-4 (honesty — genuine proof) | SHIPPED | `verus --no-cheating` on the core; `ensures result == spec_subsumes(..)` non-vacuous (negating the body → `7 verified, 1 errors`, `tests/verus_verify.rs::broken_subsumes_fails_verification`). The REQ-7/REQ-8 groundings ALSO each demonstrate non-vacuity (Grounding A: `2 verified, 1 errors`; Grounding B: `14 verified, 1 errors` ×2). |
| REQ-5 (`subsumes` verified + matched) | SHIPPED | `verus_core::subsumes` proved (+ three lattice-law `proof fn`s); `thermite_verified::subsumes_masks` (the plain mirror) consumed by `thermite_lower::effects::subsumes`; matched by the 65536-pair exhaustive equivalence test (mechanism (c), AC-4, 0 mismatches); the 14 `effects` tests still pass (AC-3). |
| REQ-6 (CI-able verus-verify gauntlet step) | SHIPPED | `thermite-verified/tests/verus_verify.rs` runs real `verus --no-cheating --crate-type=lib src/lib.rs` (skip-loud if verus absent) and asserts `verified, 0 errors`; a core fn that fails to verify is a HARD test failure (R-DEFER-6). |
| REQ-7 (degrade anti-cheat verified + anchored) | NOT-STARTED | epic #60. GROUNDED (Grounding A): the `ladder_action` verus port verifies `3 verified, 0 errors` and the anti-cheat `ensures` is non-vacuous (a `Counterexample`→`DegradeToL1` mutant → `2 verified, 1 errors`, "failed this postcondition"). NOT yet in-tree: no `verus_core` extension, `run_ladder` does NOT yet delegate to an extracted `ladder_action`, no `forge/tests/ladder_action_verified.rs` equivalence test. Today `forge/src/degrade.rs` `run_ladder`'s anti-cheat (REQ-2 there) is HERMETICALLY tested only (`counterexample_never_degrades` / `l2_counterexample_never_drops_to_l1`), not verus-anchored. |
| REQ-8 (seccomp allowlist soundness verified + anchored) | NOT-STARTED | epic #60. GROUNDED (Grounding B): the fx→syscall bitset port verifies `15 verified, 0 errors`; PURE-NO-I/O and MONOTONICITY are each non-vacuous (leak-openat mutant → `14 verified, 1 errors`; XOR non-monotone mutant → `14 verified, 1 errors`). NOT yet in-tree: no `verus_core` extension, `forge/src/sandbox.rs` `syscall_allowlist` is NOT yet anchored to the proved spec, no `forge/tests/sandbox_verified.rs` 2^8-mask equivalence test. Today `syscall_allowlist`'s soundness (REQ-3 there) is unit-tested only (`pure_baseline_excludes_io_syscalls`) + the `sandbox_conformance` oracle, not verus-anchored. |
