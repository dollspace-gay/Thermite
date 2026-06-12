# Self-Verifying the Toolchain with Verus (Tier 1: the soundness-critical pure core)
<!--
tier: 3-component
status: active (REQ-5 `subsumes` + REQ-7 `ladder_action` + REQ-8 `syscall_allowlist` all SHIPPED via mechanism (c), verus `19 verified, 0 errors`; epic #60 open for the remaining REQ-2 Tier-1 targets — cache_key/triage/kill_ratio/is_strictly_stronger/boundary-gate)
audited-sha: c3694256f9f88d34be30007b812920df95afd332 (bootstrap pin: decision 4 — doc-last-touch, NOT verified-current; backlog #262)
governs: thermite-verified/src/lib.rs (the verified core — `subsumes` + `ladder_action` + `io_allow` all proved + anchored; the REQ-2 set to be ported, epic #60)
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

> **THREE TIER-1 INCREMENTS SHIPPED.** The verified crate (`thermite-verified`) holds three
> proved soundness-critical cores: `effects::subsumes` (REQ-5), the degrade-ladder
> anti-cheat `ladder_action` (REQ-7), and the seccomp `io_allow` soundness (REQ-8) — all
> proved by real `verus --no-cheating --crate-type=lib thermite-verified/src/lib.rs`
> (**19 verified, 0 errors**) and anchored to the toolchain via mechanism (c): a
> verus-verified core + a plain-Rust mirror + an exhaustive impl==spec equivalence test
> (`subsumes` 65536 pairs in `thermite-lower`; `ladder_action` the 3+3 verdict enum +
> `io_allow` the 256 fx-masks in forge's in-module `verus_anchor` blocks — Option B, since
> forge is binary-only). REQ-1/3/4/5/6/7/8 are SHIPPED; REQ-2 (the remaining FIVE Tier-1
> fns) is NOT-STARTED, tracked under epic **#60**. The grounding sections (A/B) record the
> out-of-tree verus runs that first proved REQ-7/REQ-8 verus-fragment-friendly + non-vacuous.

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
| REQ-5 (`subsumes` verified + matched) | SHIPPED | `verus_core::subsumes` proved over the 9-atom `u16` bitset (WIDENED `u8`→`u16` for the #106 `Term` atom; the `(callee & !caller)==0` test bounded to the `< 512` domain so it agrees with the 9-way `spec_subsumes` conjunction) (+ three lattice-law `proof fn`s); `thermite_verified::subsumes_masks` (the plain mirror) consumed by `thermite_lower::effects::subsumes`; matched by the 262144-pair (512×512) exhaustive equivalence test (mechanism (c), AC-4, 0 mismatches); the `effects` tests still pass (AC-3). |
| REQ-6 (CI-able verus-verify gauntlet step) | SHIPPED | `thermite-verified/tests/verus_verify.rs` runs real `verus --no-cheating --crate-type=lib src/lib.rs` (skip-loud if verus absent) and asserts `verified, 0 errors`; a core fn that fails to verify is a HARD test failure (R-DEFER-6). |
| REQ-7 (degrade anti-cheat verified + anchored) | SHIPPED | epic #60. `verus_core::ladder_action_l3`/`ladder_action_l2` proved in-tree (the anti-cheat `ensures` `l3_is_counterexample(v) ==> (r is HardFail) && !is_degrade(r)` + the L2 analog + the global `anti_cheat_holds_for_all_verdicts` proof); `verus --no-cheating thermite-verified/src/lib.rs` → **19 verified, 0 errors**. The plain mirrors `thermite_verified::ladder_action_l3_tag`/`ladder_action_l2_tag` (+ `LadderAction`/`is_degrade`) are consumed by `forge::degrade::ladder_action_l3`/`ladder_action_l2`, and `run_ladder` now BRANCHES on the returned `LadderAction` (the proved decision drives the control flow, OQ-5). Anchored in-module (Option B — forge is binary-only): `degrade::verus_anchor` asserts the production decision == the proved tag over every verdict (3 L3 + 3 L2) AND the OQ-5 observable outcome (a `Counterexample` → hard-fail cert, no degrade stamp, `attempt_l2`/`attempt_l1` NOT invoked). Non-vacuity: `tests/verus_verify.rs::broken_ladder_action_counterexample_degrades_fails` (a `Counterexample`→`DegradeToL1` mutant fails the anti-cheat `ensures`). The existing `counterexample_never_degrades`/`l2_counterexample_never_drops_to_l1` still pass. |
| REQ-8 (seccomp allowlist soundness verified + anchored) | SHIPPED | epic #60 / #106. `verus_core::io_allow` (+ `widen`/`io_allow_exec`/`widen_exec`) proved in-tree over the 9-atom `u16` fx-mask (WIDENED `u8`→`u16` for the #106 `Term` atom) with the four soundness lemmas — `pure_has_no_io` (`io_allow(0)==0`), `non_widening_atoms_have_no_io` (now incl. the `Term` bit 8), `monotone` (subset on the syscall-mask), `io_allow_within_io_bits` (deny-by-default, bits 0..5) — `verus --no-cheating` → `27 verified, 0 errors`. The plain mirror `thermite_verified::io_allow` (+ `widen` + the 5 `SYS_*` bit constants) is anchored to `forge::sandbox::syscall_allowlist` over ALL 512 fx-masks by `sandbox::verus_anchor::syscall_allowlist_matches_proved_io_allow_over_all_512_masks` (membership over openat/socket/connect/getrandom/clock_gettime == the proved `io_allow` bits, R-CHAR-3). The #106 `Term` atom (bit 8) is NON-widening (`widen(8)==0`) — a terminal-control `ioctl` grant, NOT io-sensitive (runtime-sandbox.md REQ-7/OQ-5), so the soundness bitset is unaffected. OQ-6: verus proves soundness over the 5 sensitive syscalls only; the dense `BASELINE_SYSCALLS` stays `sandbox_conformance`-grounded. Non-vacuity: `tests/verus_verify.rs::broken_widen_leaks_openat_fails_pure_no_io` + `broken_io_allow_xor_fails_monotone`. The existing `sandbox_conformance` + `pure_baseline_excludes_io_syscalls` still pass. |

---

# FINAL Tier-1 batch (epic #60) — the remaining finite-domain anti-cheat/honesty gates

This batch ports the LAST three tractable Tier-1 targets via the SAME mechanism (c)
(a `verus!{}`-verified spec in `thermite-verified` + an exhaustive/equivalence test
binding the production fn). Each is a soundness-critical PURE decision whose wrong
answer is a false certificate; each is GROUNDED below with a REAL `verus --no-cheating`
run (the port + the 0-errors count + a non-triviality mutant that FAILS). After this
batch the Tier-1 coverage boundary (which soundness-critical fns remain honestly
UNVERIFIED and WHY) is pinned. These three are **NOT-STARTED in-tree** (the verus ports
+ mutants were RUN, results pasted in Grounding C/D/E; no `verus_core` extension, no
anchoring, no equivalence test has landed). Epic **#60** owns the porting; NO separate
blocker is filed (#60 is the tracker, per the constraint).

## Requirements (this batch)

- **REQ-9 (boundary HONESTY gate verified + anchored — Target C, the §9 composition
  anti-cheat):** The `lower_fn` external_body gate (`f.boundary.is_some() ||
  f.slag.is_some()`, `thermite-lower/src/lower.rs`) is ported into the verified
  `verus!{}` body as a 2-bool predicate `should_emit_external_body(has_boundary,
  has_slag)` carrying TWO real `ensures`: **(1)** `r ==
  spec_should_emit_external_body(has_boundary, has_slag)` where the spec is the
  disjunction `has_boundary || has_slag`; **(2)** the SOUNDNESS COROLLARY `(!has_boundary
  && !has_slag) ==> !r` — a REGULAR fn (neither flag) is NEVER emitted as
  `#[verifier::external_body]`, so a lying regular body can never be laundered to an
  assumed-L3 signature. A global `proof fn regular_fn_never_external_body` quantifies the
  corollary over the whole 2×2 bool domain. The production `lower_fn`/`lower_external_body_fn`
  gate is anchored by an exhaustive equivalence test over the 4 `(has_boundary, has_slag)`
  combinations binding the PRODUCTION dispatch decision (which arm `lower_fn` takes) to the
  proved predicate (R-CHAR-3). Derived from `.design/lower/boundary-composition.md`
  composition REQ-1 + `thermite-design.md` §9 + `goal.md` R-DEFER-9. **NOT-STARTED**
  (grounded below; epic #60).

- **REQ-10 (project LEVEL AGGREGATION verified + anchored — Target D, no over-claim,
  §5.2):** `AssuranceManifest::aggregate`'s min-over-functions project level
  (`forge/src/manifest.rs`, over the 4-valued `Level: Ord` lattice `L0<L1<L2<L3`) is
  ported into the verified `verus!{}` body as `aggregate_level(levels: Seq<Level>)` (the
  fold-min seeded at `L3` — the empty fold to `L3` mirrors `min().unwrap_or(Level::L3)`),
  carrying TWO real soundness lemmas: **(D1)** `aggregate_le_all` — `forall i:
  aggregate_level(levels) <= levels[i]` (the project level is ≤ EVERY cert's level; you
  cannot claim a project stronger than its weakest fn); **(D2)** `aggregate_is_attained` —
  `exists i: aggregate_level(levels) == levels[i]` (the level is ATTAINED, so D1+D2 ⟹ it
  is EXACTLY the min, not merely a lower bound). The production `aggregate` is anchored by
  an EXHAUSTIVE equivalence test over the 4-valued `Level` lists (all lists up to length
  ~4 over the 4 levels, plus the empty list) asserting `ProjectAssurance::Certified(min)`
  equals the proved `aggregate_level`, AND a property check that the headline is ≤ every
  input level (R-CHAR-3). NOTE the scope aggregation (`project_scope`: end-to-end iff ALL
  end-to-end) factors cleanly as a SEPARATE finite predicate — see OQ-D below; this REQ
  pins the LEVEL min (the over-claim soundness story). Derived from
  `.design/forge/degrade-ladder.md` REQ-5/REQ-6 + `thermite-design.md` §5.2 + `goal.md`
  R-DEFER-9. **NOT-STARTED** (grounded below; epic #60).

- **REQ-11 (mutation FLOOR gate verified + anchored — Target E, #48 anti-Goodhart, §7):**
  `MutationScore::meets_floor` (`forge/src/mutation.rs`, the f64 `kill_ratio() >= floor`
  gate, floor default `MUTATION_FLOOR = 0.60`) is ported into the verified `verus!{}` body
  in **INTEGER cross-multiply** form (NO f64 — verus reasons poorly about floats):
  `spec_meets_floor_60(killed, scored) == (scored > 0 && killed * 100 >= scored * 60)`
  (the exec form widens to `u128` so `killed*100`/`scored*60` cannot overflow), carrying
  the load-bearing **#48 anti-Goodhart** `ensures` `scored == 0 ==> !r` — a `0/0` score (no
  scoreable mutant) NEVER passes the floor (a contract that cannot be mutation-validated is
  gated `WeakContract`, not a vacuous pass). A global `proof fn zero_scored_never_passes`
  quantifies `spec_meets_floor_60(k, 0) == false` over all `k`. The production f64
  `meets_floor` is anchored by an EQUIVALENCE test over a BOUNDED killed/scored grid (e.g.
  `0..=20` each, at the default 0.60 floor) asserting the f64 `MutationScore { killed,
  scored }.meets_floor(0.60)` equals the integer `spec_meets_floor_60(killed, scored)` for
  every grid point (R-CHAR-3). The load-bearing soundness property — `scored == 0 ⟹ !pass`
  — is verus-proved (integer-only); the equivalence test is the f64↔integer anchor.
  Derived from `.design/forge/mutation-scoring.md` REQ-5 (the #48 0/0 gate) +
  `thermite-design.md` §7 + `goal.md` R-DEFER-9. **NOT-STARTED** (grounded below; epic
  #60).

## Acceptance criteria (this batch)

- **AC-9 (Target C verified + non-vacuous + anchored — REQ-9):**
  - **AC-9a:** `verus --no-cheating <core>` verifies `should_emit_external_body` (both
    `ensures`) + the global `regular_fn_never_external_body` proof with `0 errors`.
    GROUNDED below: the C/D/E core verifies **7 verified, 0 errors**.
  - **AC-9b (non-vacuity):** mutating the exec body to `true` (a REGULAR fn WOULD get
    external_body — the exact §9 laundering R-DEFER-9 forbids) makes the SAME run report an
    error (the corollary `(!has_boundary && !has_slag) ==> !r` fails). GROUNDED below: the
    broken-C variant reports **5 verified, 2 errors** (postcondition not satisfied).
  - **AC-9c (production-anchoring):** an exhaustive equivalence test enumerates the 4
    `(has_boundary, has_slag)` combinations and asserts the production `lower_fn` dispatch
    (whether it routes to `lower_external_body_fn` — observable as the emitted source
    carrying `#[verifier::external_body]`) equals the proved predicate; in particular the
    `(false, false)` REGULAR case takes the fully-proved-body arm (NO `external_body`), 0
    mismatches, expected from the proved predicate (R-CHAR-3).
  - **AC-9d (regression guard):** `forge`'s `composition_conformance::direct_boundary_caller_verifies_through_the_contract`
    and `lying_regular_fn_is_caught_never_laundered_to_l3` still pass unchanged.

- **AC-10 (Target D verified + non-vacuous + anchored — REQ-10):**
  - **AC-10a:** `verus --no-cheating <core>` verifies `aggregate_level` + the `aggregate_le_all`
    (D1) and `aggregate_is_attained` (D2) lemmas with `0 errors`. GROUNDED below: **7
    verified, 0 errors**.
  - **AC-10b (non-vacuity):** mutating `min2` to pick the MAX (`rank(a) >= rank(b)` instead
    of `<=` — an OVER-CLAIM: the project would be as strong as its STRONGEST fn) makes D1
    (`aggregate_le_all`) fail. GROUNDED below: the broken-D variant reports **5 verified, 2
    errors** (assertion failed).
  - **AC-10c (production-anchoring):** an exhaustive equivalence test enumerates all
    `Level` lists up to length ~4 (plus the empty list) and asserts
    `AssuranceManifest::aggregate(certs).project` is `Certified(min)` where `min ==`
    the proved `aggregate_level`, AND that the headline is ≤ every per-fn level, 0
    mismatches, expected from the proved fold-min (R-CHAR-3, never forge's own output).
    The `Failed`-capping (any non-certifying fn → `ProjectAssurance::Failed`) is ORTHOGONAL
    to the min and stays covered by the existing degrade-ladder tests.
  - **AC-10d (regression guard):** `.design/forge/degrade-ladder.md` AC-5 (the
    `{f:L3,g:L2,h:L1}` → project `L1` aggregate test) and the existing `manifest`/`degrade`
    tests still pass unchanged.

- **AC-11 (Target E verified + non-vacuous + anchored — REQ-11):**
  - **AC-11a:** `verus --no-cheating <core>` verifies `meets_floor_60` (both `ensures`,
    INTEGER cross-multiply) + the global `zero_scored_never_passes` proof with `0 errors`.
    GROUNDED below: **7 verified, 0 errors**.
  - **AC-11b (non-vacuity — the #48 property):** dropping the `scored > 0` guard from the
    exec body (so a `0/0` score passes: `0*100 >= 0*60`) makes the `scored == 0 ==> !r`
    soundness `ensures` fail. GROUNDED below: the broken-E variant reports **6 verified, 1
    errors** (postcondition not satisfied).
  - **AC-11c (production-anchoring — f64↔integer):** an equivalence test over the bounded
    grid `killed ∈ 0..=20`, `scored ∈ 0..=20` asserts the production f64 `MutationScore {
    killed, scored, survivor: None }.meets_floor(0.60)` equals the integer
    `spec_meets_floor_60(killed, scored)` for every grid point, 0 mismatches; in particular
    the `(0, 0)` point reads `false` on BOTH sides (the #48 gate), expected from the proved
    integer spec (R-CHAR-3). See OQ-E on the f64 boundary subtlety.
  - **AC-11d (regression guard):** the existing #48 test
    `mutation::tests::empty_score_is_below_floor` (a `0/0` score gated below floor) and
    `score_ratio_floor_and_string` still pass unchanged.

## Architecture (this batch)

**`should_emit_external_body` (REQ-9) — the §9 composition honesty gate.** The production
gate is the single boolean `f.boundary.is_some() || f.slag.is_some()` test at the head of
`lower_fn` (`thermite-lower/src/lower.rs`) that routes to `lower_external_body_fn` (which
emits `#[verifier::external_body]` + the unweakened signature). The finite domain is the
2×2 `(has_boundary, has_slag)` bool square — tiny but soundness-critical (a wrong `true` on
`(false, false)` launders a lying REGULAR body into an assumed-L3 signature, §9). The verus
model is a 2-bool predicate with the disjunction `ensures` PLUS the soundness corollary
`(!has_boundary && !has_slag) ==> !r`, and a global `proof fn` quantifying the corollary over
the whole bool square. The production anchor: an equivalence test projects each of the 4
combinations to a synthetic `FnItem` (regular / `#[boundary]` / `#[slag]` / both) and asserts
the emitted lowering carries `#[verifier::external_body]` IFF the proved predicate is `true`
— binding the OBSERVABLE dispatch (which `lower_fn` arm runs) to the proved gate, not just a
mirror of the boolean.

**`aggregate_level` (REQ-10) — the §5.2 no-over-claim min.** `AssuranceManifest::aggregate`
computes `functions.iter().map(|f| f.level).min().unwrap_or(Level::L3)` over the 4-valued
`Level` lattice (when every fn certifies; a non-certifying fn caps the project at `Failed`,
orthogonal to the min). The verus model is the recursive fold-min `aggregate_level(Seq<Level>)`
seeded at `L3` (so the empty list → `L3`, matching `unwrap_or(Level::L3)`), with `min2` over
the `rank` discriminant order (`L0=0..L3=3`). The two lemmas are the soundness pair: D1
(`aggregate <= every levels[i]` — the project is never claimed stronger than its weakest fn,
the §5.2 / R-DEFER-9 over-claim story) and D2 (`aggregate` is attained at some `i`, so it is
exactly the min). The production anchor enumerates the (finite) `Level` lists and binds
`aggregate(certs).project`'s carried min to the proved fold. The `Seq` recursion needs an
inductive proof (`aggregate_le_all` recurses on `drop_first` and re-indexes; `aggregate_is_attained`
chooses the attaining index in the tail and lifts it) — verified, not `bit_vector`/`compute`,
since the domain is a `Seq`, not a fixed-width int.

**`meets_floor_60` (REQ-11) — the #48 anti-Goodhart floor, INTEGER model.** The production
`meets_floor` is f64 (`kill_ratio() >= floor`, where `kill_ratio` is `0.0` when `scored == 0`,
else `killed as f64 / scored as f64`). Verus reasons poorly about f64, so the verus model is
the INTEGER cross-multiply at the default 60% floor: `scored > 0 && killed * 100 >= scored * 60`
(exec widens to `u128` to dodge overflow). The load-bearing `ensures` is the #48 property
`scored == 0 ==> !r` (a `0/0` contract NEVER passes — the production `kill_ratio` returns
`0.0 < 0.60`, the integer model fails the `scored > 0` guard; the SAME verdict, two
representations). The production f64 `meets_floor(0.60)` is anchored to the integer spec by a
BOUNDED-grid equivalence test (`0..=20 × 0..=20`): the f64 ratio `killed/scored >= 0.60` and
the integer `killed*100 >= scored*60` AGREE on every grid point (the cross-multiply is the
exact rational comparison; over small integers the f64 division is exact enough that the
`>= 0.60` boundary matches — confirmed empirically by the grid, with the f64 boundary subtlety
noted in OQ-E). The verus proof is over the INTEGER property only; the f64↔integer agreement is
the test's job (the doc honestly scopes the proved property to `scored == 0 ⟹ !pass` plus the
cross-multiply equivalence, NOT a claim that f64 arithmetic is itself verified).

## Grounding C/D/E (REAL verus run — mechanism (c), installed `verus 0.2026.05.24.ecee80a`)

All three cores were ported into ONE `verus!{}` file (`cde_verus.rs`, scratch in `/tmp`,
removed after — #53) and verified with the installed Verus, Z3-backed:

```
$ verus --no-cheating --crate-type=lib cde_verus.rs
verification results:: 7 verified, 0 errors
```

The 7 verified items: C's `should_emit_external_body` exec fn + `regular_fn_never_external_body`
proof; D's `aggregate_level`-using `aggregate_le_all` + `aggregate_is_attained` lemmas; E's
`meets_floor_60` exec fn + `zero_scored_never_passes` proof (the `spec`/`open spec` fns
verify as part of their users). Non-triviality — EACH target's core mutated independently,
each broken variant FAILS the SAME `verus --no-cheating` run:

```
# C — exec body mutated to `true` (a REGULAR fn would get external_body; §9 laundering):
#     the corollary `(!has_boundary && !has_slag) ==> !r` fails.
$ verus --no-cheating --crate-type=lib cde_C_broken.rs
verification results:: 5 verified, 2 errors        (postcondition not satisfied)

# D — `min2` mutated to pick the MAX (`rank(a) >= rank(b)`; an OVER-CLAIM project level):
#     `aggregate_le_all` (D1, "<= every fn") fails.
$ verus --no-cheating --crate-type=lib cde_D_broken.rs
verification results:: 5 verified, 2 errors        (assertion failed)

# E — the `scored > 0` guard DROPPED from the exec body (so 0/0 passes: 0*100 >= 0*60):
#     the #48 `scored == 0 ==> !r` anti-Goodhart `ensures` fails.
$ verus --no-cheating --crate-type=lib cde_E_broken.rs
verification results:: 6 verified, 1 errors        (postcondition not satisfied)
```

So each contract is GENUINELY constraining (REQ-4 / R-DEFER-9): C's regular⇒no-external_body
corollary, D's project-≤-every-fn over-claim bound, and E's `scored==0⇒!pass` #48 gate each
fail under the exact mutation each is meant to catch. For E specifically the INTEGER
cross-multiply model verifies the `scored == 0` property with NO float (the `u128` widening
removes the overflow obligation; the f64↔integer agreement is the AC-11c grid test, not the
proof). (Scratch verus files + the stale `/tmp/*.rlib` build artifacts removed, #53.)

## Tier-1 coverage boundary (after C/D/E — "as much as possible", honestly bounded)

With C/D/E added, the Tier-1 finite-domain decision functions are EXHAUSTED. The
soundness-critical core is now verus-anchored over its tractable members:
`subsumes` (REQ-5), `ladder_action` (REQ-7), `io_allow` (REQ-8), the boundary gate (REQ-9),
the project-level min (REQ-10), and the mutation floor (REQ-11). The following
soundness-relevant functions remain UNVERIFIED, and this is the HONEST boundary of the
self-verification effort — NOT a deferral dodge but a statement of what is and is not
exhaustively checkable in the verus fragment:

| Function | Soundness role | Why NOT verus-verified (honest) |
|---|---|---|
| `cache::cache_key` (`forge/src/cache.rs`, §5.3) | content-addressing — a collision/under-mix serves a stale cert for changed inputs | **INFEASIBLE in verus.** The key is a SHA-256 over the canonicalized inputs; modeling SHA-256's collision resistance is a cryptographic assumption, not an exhaustively-checkable finite predicate. The soundness rests on the hash primitive (a Tier-3-style trusted floor), empirically grounded by the cache's hit/miss conformance, not provable here. |
| `vacuity::triage` structural battery (`forge/src/vacuity.rs`, §7.1) | a vacuous/trivial contract must not pass the gate | **NOT exhaustively-checkable (Tier-2-adjacent).** Triage is an AST-walk over UNBOUNDED programs (arbitrary `req`/`ens` expression trees); its input domain is infinite and structural, not a fixed-width finite lattice. Verifying it is verified-compiler / verified-static-analysis territory (Tier 2, §11 "Thermite is not a proof assistant"). Grounded by the §7.1 triage conformance corpus. |
| `mutation::generate` enumeration (`forge/src/mutation.rs`) | the frozen mutant set must be complete + deterministic | **NOT exhaustively-checkable (Tier-2-adjacent).** `generate` is an AST-walk emitting mutants over an unbounded body; like `triage` its domain is infinite/structural. Determinism is grounded by the same-input double-run conformance, completeness by the frozen-family tests — empirical, not verus. (NOTE: the mutation FLOOR gate — the finite numeric decision — IS verified, REQ-11; only the unbounded ENUMERATION is out.) |
| `strengthen::is_strictly_stronger` (`forge/src/strengthen.rs`, §7) | a non-stronger candidate must not be suggested as stronger | Carried under REQ-2; it compares two contract expression trees (structural, unbounded domain — Tier-2-adjacent), so it is NOT a finite-lattice port. Honestly out of the finite-domain batch. |

The rule of the boundary: a function is a Tier-1 verus target IFF its soundness reduces to
a decision over a FINITE, fully-enumerable domain (a fixed-width bitset, a small enum
lattice, a 2-bool square, a bounded integer comparison). AST-walks over unbounded programs
and cryptographic primitives are categorically OUT — they are the trusted/Tier-2 floor, and
claiming otherwise would be the dishonesty R-DEFER-9 forbids.

## Files the builder will touch (this batch — orchestrator adds/extends routes)

- `thermite-verified/src/lib.rs` (EXTEND — add the C/D/E verus cores + their plain-Rust
  mirrors behind the same `#[cfg(verus_keep_ghost)]` split: `should_emit_external_body`,
  `aggregate_level`/`Level`/`min2`/`rank`, `meets_floor_60`/`spec_meets_floor_60`).
- `thermite-lower/src/lower.rs` (anchor the `lower_fn` external_body gate to the proved
  predicate — REQ-9). NOTE: `thermite-lower` HAS a lib, so its equivalence test CAN be an
  EXTERNAL `tests/boundary_gate_verified.rs` (the 4-combination enumeration), unlike forge.
- `forge/src/manifest.rs` (anchor `AssuranceManifest::aggregate` to the proved fold-min —
  REQ-10) and `forge/src/mutation.rs` (anchor `MutationScore::meets_floor` to the proved
  integer spec — REQ-11). Since **forge is binary-only**, BOTH anchors are in-module
  `#[cfg(test)]` `verus_anchor` blocks (Option B — the same pattern REQ-7/REQ-8 used):
  `manifest::verus_anchor` (the exhaustive `Level`-list enumeration) and
  `mutation::verus_anchor` (the `0..=20 × 0..=20` f64↔integer grid).
- `thermite-verified/tests/verus_verify.rs` (EXTEND — assert the new cores verify + add the
  three non-triviality mutation checks: `broken_should_emit_external_body_true_fails`,
  `broken_aggregate_max_fails_le_all`, `broken_meets_floor_drops_scored_guard_fails`, using
  the existing `assert_mutation_fails` helper).

## Open questions (this batch)

- **OQ-C (boundary-gate anchor observability — analogous to OQ-5):** the verus core proves
  the 2-bool DECISION. The production anchor (AC-9c) must assert the OBSERVABLE dispatch —
  that `lower_fn` on a `(false, false)` regular fn emits a fully-proved body (NO
  `#[verifier::external_body]` substring) and on any flagged fn emits the external_body
  signature — not merely that a mirror predicate returns the same bool. The risk (parallel
  to OQ-5) is a gap between "the predicate is proved" and "`lower_fn` is wired to honor it";
  the test must inspect the emitted source.
- **OQ-D (does the scope aggregation factor cleanly? — for REQ-10):** YES, cleanly but as a
  SEPARATE predicate. `project_scope` (end-to-end iff ALL certs end-to-end) is a finite
  fold-AND over the per-fn `scope_is_end_to_end` bool, structurally identical to a
  `forall`-over-a-bool-seq lemma (the dual of D's min). It could be a REQ-10 companion
  (`aggregate_scope_end_to_end(bools) == bools.fold_and()`, `ensures` end-to-end ⟹ every fn
  end-to-end). It is NOT folded into REQ-10's LEVEL min (orthogonal axes — §17), and is left
  as an OPTIONAL companion the builder MAY add; the LEVEL over-claim (D1/D2) is the
  load-bearing §5.2 soundness story this REQ pins.
- **OQ-E (f64-vs-integer equivalence — LEAST CONFIDENT for REQ-11):** the verus proof is
  INTEGER-only (`scored == 0 ⟹ !pass` + the cross-multiply). The AC-11c grid test asserts the
  PRODUCTION f64 `meets_floor(0.60)` AGREES with `killed*100 >= scored*60` over `0..=20 ×
  0..=20`. Over small integers the f64 division `killed/scored` is computed exactly enough
  that the `>= 0.60` comparison matches the rational cross-multiply at EVERY grid point —
  but f64 `0.60` is not exactly 3/5, so for a ratio EXACTLY on the boundary (e.g. `12/20 ==
  0.60`) the f64 `>=` and the integer `>=` could in principle diverge by a rounding ULP. The
  builder MUST run the grid and, if ANY point diverges, either (a) widen the grid to surface
  the divergence and document the exact boundary case, or (b) scope the anchor to the proved
  `scored == 0` property + the strictly-interior grid, documenting the f64-boundary caveat
  honestly (NEVER silently masking a divergence — R-DEFER-9). The empirical expectation
  (from the cross-multiply being the exact rational test) is 0 divergences on `0..=20`, but
  this is the claim to VERIFY at build time, not assume.

## REQ status (this batch)

The prior increments (REQ-1/3/4/5/6/7/8) stay SHIPPED. REQ-9/10/11 are now **SHIPPED
in-tree**: the C/D/E verus cores landed in `thermite-verified/src/lib.rs` (verified by real
`verus --no-cheating --crate-type=lib thermite-verified/src/lib.rs` → **26 verified, 0
errors**, up from 19), each anchored to the production fn by mechanism (c). REQ-2's
finite-domain Tier-1 set is now EXHAUSTED (see the Tier-1 coverage boundary). Epic **#60**
owns the porting (no separate blocker — #60 is the tracker).

| REQ | Status | Evidence |
|---|---|---|
| REQ-9 (boundary HONESTY gate — Target C) | SHIPPED | epic #60. `verus_core::should_emit_external_body` proved (`ensures r == has_boundary \|\| has_slag` + the soundness corollary `(!has_boundary && !has_slag) ==> !r` + the global `regular_fn_never_external_body` proof); `verus --no-cheating thermite-verified/src/lib.rs` → 26 verified, 0 errors. The plain mirror `thermite_verified::should_emit_external_body` is consumed by `thermite_lower::lower::lower_fn`'s gate, anchored by the OBSERVABLE-dispatch test `thermite-lower/tests/boundary_gate_verified.rs` (the emitted source carries `#[verifier::external_body]` IFF the proved predicate, over the 4 (boundary,slag) combos; the (false,false) regular fn carries NONE). Non-vacuity: `tests/verus_verify.rs::broken_should_emit_external_body_true_fails` (exec body → `true` fails the corollary). The existing `composition_conformance` tests still pass. |
| REQ-10 (project LEVEL AGGREGATION — Target D) | SHIPPED | epic #60. `verus_core::aggregate_level` (the `Seq<Level>` fold-min seeded at L3) proved with `aggregate_le_all` (D1: ≤ every fn — the §5.2 over-claim bound) + `aggregate_is_attained` (D2: == the min); the plain mirror `thermite_verified::aggregate_level` (+ `Level`/`min2`/`rank`) anchors `forge::manifest::AssuranceManifest::aggregate` over ALL 341 `Level` lists (len 0..=4) by `manifest::tests::verus_anchor` (Option B, forge binary-only): `aggregate(certs).project == Certified(proved_min)` AND headline ≤ every level. Non-vacuity: `tests/verus_verify.rs::broken_aggregate_max_fails_le_all` (`min2` → MAX, an over-claim, fails D1). The existing `aggregate_headline_is_min_over_functions` (AC-5) still passes. The scope aggregation (OQ-D) factors cleanly but is left as the optional companion. |
| REQ-11 (mutation FLOOR gate — Target E, #48) | SHIPPED | epic #60. `verus_core::meets_floor_60` (INTEGER cross-multiply `scored > 0 && killed*100 >= scored*60`, `u128` exec widening, NO float) proved with the `scored == 0 ==> !r` #48 `ensures` + the global `zero_scored_never_passes` proof; the plain mirror `thermite_verified::meets_floor_60` anchors the production f64 `MutationScore::meets_floor(0.60)` over the `0..=20 × 0..=20` grid by `mutation::tests::verus_anchor`. OQ-E RESULT: the f64↔integer grid AGREES on EVERY cell (0 divergences over all 441 points — the cross-multiply is the exact rational comparison, the f64 boundary like 12/20==0.60 matches; no masking). Non-vacuity: `tests/verus_verify.rs::broken_meets_floor_drops_scored_guard_fails` (drop the `scored > 0` guard fails the #48 `ensures`). The existing #48 test `empty_score_is_below_floor` (0/0 gated) still passes. |
