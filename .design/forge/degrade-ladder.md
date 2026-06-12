# The Automatic Degrade Ladder + Assurance Manifest
<!--
tier: 3-component
status: draft
audited-sha: deed87036c1570762b267255df9a83bd18c96f6d (bootstrap pin: decision 4 — doc-last-touch, NOT verified-current; backlog #262)
governs: forge/src/degrade.rs
thesis-refs:
  - thermite-design.md §5.2
  - thermite-design.md §5.1
  - thermite-design.md §5.3
  - thermite-design.md §6
  - thermite-design.md §12
-->

## Summary

`forge check`'s contract with the agent is **"the gate degrades, it never
blocks"** (§5.2). This component is the per-item L3→L2→L1 **automatic degrade
ladder** and the project-level **assurance manifest**. On the DEFAULT `forge
check` path (no `--level` flag), an item is first attempted at L3 (the verus SMT
proof). When verus cannot PROVE it within its resource budget — a **TIMEOUT**
(inconclusive) — Forge automatically attempts **L2** (Kani bounded model check,
#9's type-driven bound) and, if L2 also cannot bound-verify within budget, drops
the obligation to **L1** (the SpecTherm contract compiled to runtime checks,
#9/L1's `lower_l1`). Each rung below L3 attaches a `lowered-assurance` flag and a
degrade REASON (#11's `SolverProfile` / timeout reason as "here's where I got
lost"). The assurance manifest aggregates the per-fn certificates into a
project-level view whose headline number is the **min over functions**, displayed
on every build (§5.2, §6).

This component COMPOSES three shipped pieces, it does not reinvent them:
- #11's timeout-vs-counterexample-vs-success classification
  (`fn classify_verus_outcome in check.rs` → `enum VerusOutcome { Proved, Timeout,
  Counterexample }`) — the degrade TRIGGER.
- #9's L2 path (`thermite_lower::lower_l2` + `pub fn run_kani in kani.rs` +
  `pub fn check_l2_file in check.rs`) — the L2 rung.
- L1 (`thermite_lower::lower_l1`) — the always-existing runtime-check rung (§4.2:
  every `spec fn` is executable, so an L1 fallback always exists).

**THE CRITICAL ANTI-CHEAT INVARIANT (§5.2, R-DEFER-9, R-CODE-4).** The ladder
degrades **INCONCLUSIVENESS, never FALSITY**. Degrade happens ONLY on a TIMEOUT
(verus couldn't prove — inconclusive). A **COUNTEREXAMPLE** (verus DISPROVED the
contract — a real bug) MUST NOT degrade: it is a hard failure, reported with its
witness, never "certified L1." Degrading a disproved contract to L1 would hide a
real bug behind a `lowered-assurance` stamp — the worst possible outcome, exactly
the failure §12 ("bounded checks oversold as proofs") and §7 (anti-Goodhart)
exist to prevent.

It is GREENFIELD: `forge/src/degrade.rs` does not yet exist. The classification
that triggers a degrade (#11) and the L2 path it degrades INTO (#9) both ship; the
auto-degrade COMPOSITION and the min-over-functions aggregate do not. Today the
default path emits a `VerusOutcome::Timeout` cert at `Level::L0` + `VerusTimeout`
(`fn assemble_certificate in check.rs`, via `Certificate::timeout`) and STOPS —
the v0.1 deliberate non-auto-degrade noted in `check.rs`'s `solver-profiles REQ-7`
row ("v0.1 does not auto-degrade (#10)") and in `check_l2_file`'s doc ("#9 does
NOT wire L2 as an automatic fallback … that is #10's change"). This doc is
forward-looking; all REQs are NOT-STARTED, blocked on #10 (blocker #50).

OUT OF SCOPE (boundaries): the background proof-repair loop that drives L1/L2
back UP to L3 is #18 (§5.2 "upgrades are a standing background task"); solver
profiles themselves are #11 (done — reuse them); `forge audit`'s full
slag+boundary inventory is a separate surface (Appendix B). #10 is the
auto-degrade ladder + the assurance aggregate, nothing more.

## Requirements

- **REQ-1 (the degrade state machine)**: a `pub fn` (the per-item ladder, the
  default `forge check` path) that, given one item, runs the rungs in order and
  returns its achieved-level certificate:
  `L3 (verus) ──[Proved]──▶ certify L3`;
  `──[Timeout]──▶ attempt L2 (kani)`;
  `L2 ──[verified to bound]──▶ certify L2 (+ lowered-assurance flag + the bound)`;
  `L2 ──[Timeout / cannot bound-verify]──▶ L1 (lower_l1) → certify L1
  (lowered-assurance flag)`. The classification at each rung is #11's
  `classify_verus_outcome` (L3) and `kani.rs`'s `parse_kani_output` (L2). Derived
  from §5.2, §6 ("Downgrades are automatic, logged, and surfaced").

- **REQ-2 (the anti-cheat invariant — a counterexample NEVER degrades)**: a
  `VerusOutcome::Counterexample` at L3 (verus DISPROVED the contract) is a HARD
  FAILURE — the cert reports the counterexample witness (the existing
  `Level::L0` + per-obligation `ObligationResult::failed` path) and the ladder
  STOPS. It MUST NOT attempt L2 and MUST NOT drop to L1. Likewise, an L2
  COUNTEREXAMPLE (a Kani `VERIFICATION:- FAILED` with a concrete `Failed Checks:`
  witness, as opposed to an under-bound/timeout) is a hard failure, NOT a drop to
  L1. The ladder degrades inconclusiveness (Timeout), never falsity
  (Counterexample). Derived from §5.2, §12, `goal.md` R-DEFER-9 / R-CODE-4.

- **REQ-3 (L1 fallback rung)**: when L2 also times out / cannot bound-verify, the
  obligation drops to L1: `thermite_lower::lower_l1` compiles the SpecTherm
  contract to always-active runtime checks (§4.2 — every `spec fn` is executable,
  so L1 ALWAYS exists for every contract). The item certifies `Level::L1` with the
  `lowered-assurance` flag. Whether L1 certification must drive `l1.rs`'s
  runtime-check EMISSION at degrade time or merely RECORD the achieved level is
  OQ-3. Derived from §5.2, §6, §4.2.

- **REQ-4 (`lowered-assurance` flag + degrade reason on the certificate)**: a cert
  achieved below L3 carries a `lowered-assurance` flag and the degrade REASON —
  reusing #11's `SolverProfile` / the `VerusTimeout` reason as the "why it
  degraded" plus the profile-derived strengthening prompt (`profile::suggested_move`).
  ADDITIVE to the existing certificate schema (R-SPEC-2): the flag + reason live
  in `#[serde(default, skip_serializing_if)]` fields so the frozen golden
  `conformance/sum.cert.json` (which omits them) still deserializes, mirroring the
  #8 `cached` / #11 `solver_profile` additive precedents. Derived from §5.2 ("a
  `lowered-assurance` flag is attached to the build manifest"), §6, §5.1.

- **REQ-5 (the assurance manifest — per-fn aggregate)**: a `pub fn` /
  `pub struct` that aggregates the per-fn certificates (`Vec<Certificate>`) into a
  project-level view: each fn's achieved level, its `lowered-assurance` flag + the
  degrade reason where present, and the project headline. The manifest is an
  AGGREGATE of the cert collection `forge check` already returns; where it is
  emitted (a `forge check`-level manifest vs an aggregate computed at render time)
  is OQ-4 (additive, never a verdict mutation of a per-item cert — R-SPEC-2).
  Derived from §6 ("The certificate attached to a build artifact lists every
  function's level … This manifest IS the deliverable's trust statement"), §5.2.

- **REQ-6 (min-over-functions project assurance level)**: the whole-project
  assurance level is the **min over functions** (with the ladder ordering
  `L0 < L1 < L2 < L3`, `Level`'s declaration order in `manifest.rs`), displayed on
  every build. A single function at L1 caps the project at L1; a single
  hard-failed (counterexample / reject) function is a project FAILURE (not a
  lowered level — a non-certifying item is not a rung, REQ-2). Derived from §5.2
  ("the whole-project assurance level is the min over functions, displayed on
  every build").

- **REQ-7 (determinism — the achieved level is deterministic given pinned budgets)**:
  given the pinned `--rlimit` (verus) and the pinned `SLICE_BOUND` / unwind bound
  (kani), the achieved rung for each item is deterministic and the project
  min-over-functions is deterministic. The degrade REASON content (the
  `SolverProfile` instantiation counts) is NON-deterministic (§5.3) and
  oracle-EXCLUDED, exactly like #11's profile. The ladder introduces no wall-clock
  / unseeded input into the verdict (R-CODE-5). Derived from §5.3, R-CODE-5.

- **REQ-8 (subprocess failures never silently degrade)**: an ENVIRONMENT failure —
  verus absent (`ForgeError::VerusAbsent`), kani absent (`ForgeError::KaniAbsent`),
  unparseable output (`ForgeError::VerusOutput` / `KaniOutput`), VIR/internal error
  — is a `ForgeError`, NOT a degrade. A degrade is triggered ONLY by a genuine,
  classified TIMEOUT verdict, never by "the prover wasn't there" or "I couldn't
  read the output" (R-CODE-4: "no swallowing of solver/subprocess failures … a
  timeout degrades the ladder … never silently treated as success"). Derived from
  R-CODE-4.

- **REQ-9 (the `fx diverge` L1 cap is a PARTIAL-CORRECTNESS rung, distinct from a
  TIMEOUT degrade — the level semantics, #88)**: this doc owns the LEVEL meaning of
  every rung, and a `fx diverge` fn's honest level is **L1 = partial correctness**.
  A diverge fn (effect row contains `Effect::Diverge`, §4.1 "divergence requires
  `fx diverge` in the row") is NOT total: it may not terminate, so it CANNOT be L3
  — L3 means "the contract holds for ALL inputs" = TOTAL correctness (§6 ladder
  table). Its honest assurance is PARTIAL correctness: the loop INVARIANTS + the
  `ens` hold GIVEN the runtime contract checks, but termination + a strong
  functional postcondition are NOT claimed. Therefore a diverge fn is CAPPED at L1
  (this rung's partial-correctness reading of L1, alongside the §6 "runtime contract
  checks" reading) and is EXEMPT from the §7 mutation-kill + strengthening gate
  (which validates a strong-functional `ens`, inapplicable to a partial-correctness
  event loop). This cap is the SAME honesty move as #16's `#[boundary]` L1 cap (a
  boundary fn caps at L1, mutation-exempt, because its foreign body is unproven) and
  #18/slag's L1 — three distinct REASONS a fn honestly sits at L1 below an L3-total
  claim. CRITICAL: the diverge L1 cap is NOT a degrade-ladder TIMEOUT rung — it is a
  STRUCTURAL cap decided by `gate_fn` from the `fx diverge` declaration BEFORE any
  prover runs (the `.design/forge/check.md` REQ-8 `gate_fn` routing), never a
  budget-exhaustion degrade (REQ-1) and never a counterexample (REQ-2). The cap is
  diverge-ONLY: a non-diverge fn STILL proves termination (its `dec`) AND STILL
  passes the §7 mutation gate to reach L3; the exemption is not a termination
  (#87, `fn_is_diverge in lower.rs` keeps the termination exemption diverge-only) or
  mutation escape hatch for a normal weak contract. Derived from §4.1, §6, §7,
  `goal.md` R-DEFER-9 (the cap is honest, not a laundering bypass), the #16 boundary
  precedent; open blocker #88.

## Acceptance criteria

- **AC-1 (corpus → all L3, project assurance L3, no degrade)**: `forge check
  conformance/sum.th` and `conformance/binary_search.th` at the DEFAULT `--rlimit`
  certify every fn at `Level::L3` (`VerusOutcome::Proved`), the ladder runs NO L2
  / L1 rung, no cert carries the `lowered-assurance` flag, and the
  min-over-functions project assurance is L3. Mechanically: assert every cert's
  level is L3, `lowered-assurance` is absent on each, and the aggregate headline is
  L3. GROUNDED below (`forge check conformance/sum.th` → both items `level: L3`).

- **AC-2 (forced L3-timeout whose L2 verifies → certified L2 + lowered-assurance +
  reason)**: an item that L3-TIMES-OUT (the verus run classifies as
  `VerusOutcome::Timeout` — a profile report present on stderr) but whose
  `lower_l2` Kani harness bound-verifies → certifies `Level::L2` with the
  `lowered-assurance` flag, the bound string (`slice <= N, unwind K`), and the
  degrade reason (the `SolverProfile` / `VerusTimeout` reason). Mechanically: a
  deterministic forced-degrade test (low `--rlimit` forces the L3 timeout on a
  genuinely budget-consuming item, the same item's L2 harness verifies). GROUNDED
  below (real verus rlimit-exceeded → profile report; real kani on the same `sum`
  L2 harness → `VERIFICATION:- SUCCESSFUL`). See OQ-1 on test determinism.

- **AC-3 (forced L3+L2-timeout → L1, lowered-assurance)**: if constructible, an
  item that times out at L3 AND at L2 (Kani cannot bound-verify within budget /
  exhausts unwind) drops to `Level::L1` (`lower_l1` runtime checks) with the
  `lowered-assurance` flag and the degrade reason. Mechanically: a fixture
  (or, if a deterministic L2-timeout is not reliably provokable, a unit test
  driving the ladder state machine on a synthesized L2-Timeout `L2Result`). See
  OQ-2 (L2 counterexample-vs-timeout distinction) and OQ-1.

- **AC-4 (THE KEY ANTI-CHEAT AC — a counterexample NEVER degrades)**: a broken
  contract (a fn whose `ens` is provably FALSE — verus returns
  `VerusOutcome::Counterexample`, `success: false, errors: 1`, NO profile report)
  is a HARD FAILURE: the cert is non-certifying (`Level::L0` + the counterexample
  witness), the ladder runs NO L2 and NO L1 rung, and the cert carries NO
  `lowered-assurance` flag and is NOT `Level::L1`. Mechanically: assert the broken
  fixture's cert is non-certifying, has no `lowered-assurance`, and no L2/L1 rung
  was attempted (e.g. a probe asserting the ladder short-circuits on
  `VerusOutcome::Counterexample`). This is the AC that, if it regresses, hides a
  real bug behind a lowered stamp. GROUNDED below (real verus on a disproved
  contract → counterexample, NO profile, distinct from the timeout).

- **AC-5 (min-over-functions aggregate)**: a multi-fn program with achieved levels
  e.g. `{f: L3, g: L2, h: L1}` produces project assurance `L1` (the min); a program
  with a single hard-failed fn produces a project FAILURE (not a lowered level).
  Mechanically: a table-driven test over synthesized per-fn cert sets asserting the
  aggregate headline equals the min, and that a non-certifying cert is not treated
  as a rung.

- **AC-6 (additive + oracle-excluded)**: the `lowered-assurance` flag + degrade
  reason are `#[serde(default, skip_serializing_if)]` additive fields — the frozen
  golden `conformance/sum.cert.json` still deserializes (R-SPEC-2). The degrade
  REASON (the `SolverProfile`) is EXCLUDED from `Certificate::oracle_subset` (it is
  diagnostic + non-deterministic, §5.3); the achieved `level` and the
  `lowered-assurance` flag, being verdict-relevant, are NOT excluded. Mechanically:
  golden round-trip + an oracle-equality assertion ignoring the reason but
  catching a level / flag difference.

- **AC-7 (determinism)**: re-running `forge check` over the corpus at the pinned
  budgets yields byte-identical achieved levels and project assurance across runs
  (R-CODE-5); the degrade reason content is asserted only structurally
  (oracle-excluded). Mechanically: two runs, oracle-subset-equal certs + equal
  aggregate headline.

- **AC-8 (the `fx diverge` L1 cap — partial correctness, mutation-exempt, the
  diverge-ONLY scope; #88)**: the editor's event loop `fn run() ... fx
  read(input), write(output), alloc, diverge { while quit == 0 inv ... dec 1 { ...
  } }` (`examples/editor/editor.th`) certifies `Level::L1` (partial correctness) —
  NOT `Level::L0` `WeakContract` and NOT a forced L3 — with NO `strengthening`
  suggestion and NO mutation `survivor` reject (the §7 gate is skipped). A NORMAL
  weak-contract fn (no `diverge`) STILL rejects at `Level::L0` `WeakContract` at the
  default floor (the gate still bites a non-diverge weak contract); a NORMAL loop fn
  without a `dec` (no `diverge`) STILL fails Verus termination (the #87 exemption
  stays diverge-only); `conformance/sum.th` + `conformance/binary_search.th` are
  UNCHANGED at `Level::L3` (the diverge gate never fires for a total fn). The
  min-over-functions aggregate (REQ-6) treats a diverge L1 cert as a genuine L1 rung
  (NOT a hard failure) — a project containing the editor's `run` caps at L1, exactly
  as one containing a boundary L1 fn does. Mechanically: assert `run`'s cert is L1
  with no reject and no strengthening; assert a non-diverge weak fixture still L0;
  assert the corpus still L3; assert the aggregate over `{run: L1, others: L3}` is
  L1. GROUNDED: `run`'s loose `ens result <= 256` is met by `return 0`, so the §7
  battery (if run) kills a minority of mutants and reports `WeakContract` at L0 —
  the wrong verdict the L1 cap corrects.

## Architecture

The component is a per-item ladder driver (`degrade.rs`) plus an aggregate
(the assurance manifest). It sits BETWEEN the default `forge check` entry
(`check_file_with_options in check.rs`) and the three shipped rungs, replacing the
v0.1 "emit a `VerusTimeout` L0 cert and stop" behavior with the laddered
auto-degrade. It owns NO new prover invocation logic — it composes `run_verus`'s
classification and `run_kani` / `check_l2_file`'s L2 path.

### The ladder state machine (REQ-1, REQ-2, REQ-3)

```text
                         forge check <file>   (DEFAULT path, no --level)
                                  │
                                  ▼
                       L3:  run_verus  →  classify_verus_outcome
                                  │
        ┌──────────────┬──────────┴───────────────┐
        ▼              ▼                           ▼
     Proved        Timeout                   Counterexample
        │          (profile present)         (DISPROVED — a real bug)
        ▼              │                           │
  certify L3           ▼                           ▼
              L2:  lower_l2 → run_kani        HARD FAIL  (REQ-2 anti-cheat)
                        │                     report the witness; cert is
        ┌───────────────┼─────────────┐       Level::L0 non-certifying;
        ▼               ▼             ▼       NO L2, NO L1, NO lowered stamp
   verified         Timeout /     Counterexample
   to bound        under-bound    (Kani DISPROVED)
        │               │             │
        ▼               ▼             ▼
  certify L2       L1: lower_l1   HARD FAIL  (REQ-2 anti-cheat, 2nd rung)
  + lowered-       runtime checks  report the witness; NOT a drop to L1
  assurance        → certify L1
  + bound          + lowered-assurance + reason
```

The TRIGGER for every degrade edge is a classified **Timeout**, never a
Counterexample. At L3 the discriminator is #11's `classify_verus_outcome` (a
`--profile` instantiation report on stderr = Timeout; an error WITHOUT a profile =
Counterexample/failure — `enum VerusOutcome in check.rs`). At L2 the discriminator
is `kani.rs`'s `parse_kani_output`: `VERIFICATION:- SUCCESSFUL` → L2; a
`VERIFICATION:- FAILED` carrying a concrete `Failed Checks:` witness → a
counterexample (hard fail); the `unwinding assertion loop 0` under-bound failure →
an inconclusive bound exhaustion that degrades to L1 (OQ-2 is the precise
counterexample-vs-under-bound split at this rung).

### Composition with the shipped pieces (symbol anchors)

- `fn classify_verus_outcome in check.rs` → `enum VerusOutcome { Proved, Timeout,
  Counterexample }` — the L3 degrade trigger. `Proved` → certify L3; `Timeout` →
  degrade edge to L2; `Counterexample` → hard fail (REQ-2).
- `fn run_verus` / `fn invoke_verus in check.rs` — the verus driver (already passes
  `--profile` + the pinned `--rlimit`; the rlimit is the timeout-forcing lever).
- `pub fn check_l2_file in check.rs` and `pub fn run_kani in kani.rs` +
  `thermite_lower::lower_l2` + `thermite_lower::bound_string` — the L2 rung. The
  EXPLICIT `--level l2` path (from #9, `enum CheckLevel in cli.rs`) still forces L2
  directly and is UNCHANGED; the default path is now the laddered auto-degrade.
- `struct L2Result in kani.rs` (`level`, `obligations`) and `fn parse_kani_output
  in kani.rs` — the L2 verdict the ladder reads (`Level::L2` success vs the
  counterexample/under-bound `Level::L0`).
- `thermite_lower::lower_l1` (`pub fn lower_l1 in l1.rs`) — the L1 fallback rung
  (§4.2 always-exists guarantee).
- `profile::SolverProfile` / `pub fn suggested_move in profile.rs` — the degrade
  REASON + strengthening prompt reused on a degraded cert (REQ-4).
- `struct Certificate` / `fn oracle_subset` / `Certificate::timeout` /
  `Certificate::with_cached` in `manifest.rs` and `enum Level { L0, L1, L2, L3 }` —
  the cert schema. REQ-4's `lowered-assurance` flag + reason are ADDITIVE fields
  on `Certificate` (the #8 `cached` / #11 `solver_profile` precedent); REQ-5/REQ-6
  add the aggregate (a new `struct` aggregating `&[Certificate]`).
- `fn run_check in cli.rs` (`fn is_certified`, which already treats
  `Level::{L3,L2,L1}` as certified rungs) — the consumer that renders the laddered
  cert + the project assurance headline.

### How the default path changes (the v0.1 → v0.2 delta)

Today, `check_file_with_options`'s per-item loop, on a `VerusOutcome::Timeout`,
calls `Certificate::timeout` → `Level::L0` + `RejectReason { cause: "VerusTimeout" }`
and STOPS (documented in `check.rs`'s `solver-profiles REQ-7` REQ-status row and
in `Certificate::timeout`'s doc: "v0.1 has no automatic degrade (#10)"). #10
replaces that STOP with the ladder: a `Timeout` becomes a degrade EDGE into L2,
and an L2 timeout a degrade edge into L1. A `Counterexample` keeps its existing
hard-fail behavior unchanged (REQ-2). This is purely additive to the rungs that
already ship — the L3 proof, the L2 bounded check, the L1 runtime checks are all
present; #10 wires the automatic transition between them and the aggregate over
the result.

### Why a Timeout cert is never cached as proved (interaction with #8)

`check.rs` already documents that a non-default `--rlimit` BYPASSES the proof
cache and that a `VerusTimeout` cert is NEVER cached ("a timeout is never cached
as proved"). The ladder must preserve this: a degraded L2/L1 verdict is
budget-dependent (a larger budget might prove L3), so it is not a settled
content-addressed verdict and must not pollute the canonical-budget cache. The
ladder runs INSIDE the existing per-item loop where this invariant already holds.

### The L1 rung has THREE distinct partial-correctness causes (REQ-9, the level semantics)

This component owns the LEVEL meaning of each rung, so the diverge cap's semantics
live here even though the gate ROUTING that produces it lives in `gate_fn`
(`.design/forge/check.md` REQ-8). L1 is reached for THREE structurally distinct,
all-honest reasons — none of which is an L3-total over-claim:

- **Degrade-to-L1** (REQ-3): a TOTAL fn that L3-TIMED-OUT and L2-TIMED-OUT drops to
  the runtime-check rung. Budget-dependent, carries `lowered-assurance` + a degrade
  REASON. This is the §5.2 inconclusiveness degrade.
- **Boundary-L1 / slag-L1** (#16 / #18): a fn whose BODY is unproven (foreign, or
  fiat-trusted) — the contract is L1-enforced at the crossing, the body is not
  proved. `boundary: true` / `slag: true`. Structural (from the attribute), NOT a
  degrade.
- **Diverge-L1 (REQ-9, NEW)**: a `fx diverge` fn whose body IS (partially) proved —
  its loop INVARIANTS verify (post-#87, partial correctness) — but which is NOT
  total (it may not terminate), so it cannot claim L3-total. Structural (from the
  `fx diverge` declaration in `gate_fn`), NOT a degrade and NOT a body-trust gap.
  The honest level is L1 = partial correctness; the §7 mutation/strengthen gate
  (which validates a strong-functional `ens`) is SKIPPED, exactly as it is for a
  boundary fn.

The min-over-functions aggregate (REQ-6) treats ALL THREE L1 causes as genuine L1
RUNGS (a project with a diverge `run` caps at L1, not a project FAILURE) — only a
counterexample / hard reject (REQ-2) is a non-rung failure. The diverge cap is
decided BEFORE the prover by `gate_fn` (so it never flows through this doc's
TIMEOUT-degrade edges); the degrade-ladder's anti-cheat invariant (degrade
inconclusiveness, never falsity) is UNTOUCHED — a diverge fn is not a degrade at
all, it is a structural cap. Whether the diverge cert RUNS Verus for its partial
invariants (recommended — the #87 proof is real assurance) or skips Verus like a
boundary fn is pinned in `.design/forge/check.md` REQ-8 (the gate that builds the
cert); this doc fixes only that the resulting LEVEL is L1 = partial correctness.

## Verification

- `cargo test -p forge` — unit tests over the ladder state machine: (REQ-1) a
  `Timeout` L3 outcome drives an L2 attempt; (REQ-2) a `Counterexample` L3 outcome
  short-circuits to a hard fail with NO L2/L1 attempt (the key anti-cheat test);
  (REQ-3) an L2 timeout drives an L1 fallback; (REQ-6) the min-over-functions
  aggregate over synthesized per-fn cert sets; (REQ-7/AC-7) determinism of the
  achieved level. The state machine is driven on SYNTHESIZED `VerusOutcome` /
  `L2Result` inputs so the unit tests are hermetic (do not depend on provoking a
  live resourceout, which OQ-1 shows is timing-fragile).
- A FORCED-DEGRADE conformance test (`forge/tests/`): a low `--rlimit` on a
  genuinely budget-consuming corpus/fixture item forces the L3 timeout; the same
  item's `lower_l2` harness verifies → an L2 cert with `lowered-assurance`. Per
  OQ-1, the LIVE forced-timeout is best-effort (skip-loud when no rlimit-hit is
  provoked); the DETERMINISTIC ladder logic is pinned by the hermetic unit tests.
- Conformance: `forge check conformance/sum.th` still emits the golden
  `sum.cert.json` (`Level::L3`, no `lowered-assurance`) at the default budget — the
  additive flag + the ladder must not perturb the frozen oracle (AC-1, AC-6).
- The anti-cheat AC-4 is pinned as a test asserting a broken-contract fixture is a
  non-certifying cert with NO `lowered-assurance` and NO L1 — a regression here is
  the worst possible failure (a hidden bug), so it is the highest-value test.

### The grounded forced degrade (real verus + real kani, this session)

The full L3-timeout → L2-success chain on the SAME `sum` item is grounded against
the real binaries (verus `0.2026.05.24`-class, Z3 4.12.5; `cargo kani 0.67.0`).
R-CHAR-3: the expected shapes are the PROVERS' output, never forge's.

**Corpus at the DEFAULT rlimit → L3, no degrade (AC-1).** `forge check
conformance/sum.th`:

```text
item: spec_sum   level: L3   effects: [pure]   slag: false
item: sum        level: L3   effects: [pure]   slag: false
```

Both items `Proved` at the default budget → project assurance L3, the ladder runs
no degrade rung.

**Forced L3 TIMEOUT (the degrade trigger, REQ-1 / AC-2).** A genuinely
budget-consuming nonlinear goal under `verus --output-json --profile --rlimit 1`
(the #11 rlimit seam) produces a real resourceout that `classify_verus_outcome`
reads as `VerusOutcome::Timeout` — summary `success: false, errors: 1`, AND a
`--profile` instantiation report on STDERR (the timeout discriminator):

```text
error: assert_nonlinear_by: Resource limit (rlimit) exceeded
 --> .../to_check.rs:8:5
note: Profile statistics for to_check::nl
note: Observed 0 total instantiations of user-level quantifiers
error: aborting due to 1 previous error
```

`classify_verus_outcome` keys on the `Observed N total instantiations` line
(`profile::parse_profile` returns `Some`) → `VerusOutcome::Timeout`, NOT
`Counterexample`. This is the edge that, under #10, degrades to L2.

**L2 SUCCESS on the SAME item (the degrade target, REQ-1 / AC-2).** The real
`thermite_lower::lower_l2` harness for `sum` (bound `slice <= 4, unwind 5`),
written into a temp cargo crate exactly as `kani.rs::write_kani_crate` does and
run under `cargo kani --output-format terse`:

```text
VERIFICATION RESULT:
 ** 0 of 38 failed
VERIFICATION:- SUCCESSFUL
```

`kani.rs::parse_kani_output` reads this as `Level::L2` with the bound recorded →
the degraded cert is `Level::L2` + `lowered-assurance` + `slice <= 4, unwind 5`.
So the forced degrade is real end-to-end: `sum` L3-TIMES-OUT (low budget) → L2
VERIFIES.

**The anti-cheat distinction is real (REQ-2 / AC-4).** A genuinely DISPROVED
contract (`ens r == x + 2` for a body returning `x + 1`) under `verus --profile
--rlimit 30`:

```text
error: postcondition not satisfied
 --> .../broken_check.rs:5:13
5 |     ensures r == x + 2,
  |             ^^^^^^^^^^ failed this postcondition
```

Summary `success: false, errors: 1`, and the `Observed … instantiations` count is
**0 (no profile report)** → `profile::parse_profile` returns `None` →
`VerusOutcome::Counterexample`. This is structurally distinct from the timeout
(profile PRESENT vs ABSENT) and MUST be a hard failure — never a degrade to L1.

### Constructing a deterministic forced-degrade TEST

A low `--rlimit` (the #11 seam threaded by `check_file_with_rlimit` /
`cli`'s `--rlimit` flag) on a genuinely budget-consuming item forces the L3
timeout deterministically when the item actually exhausts the budget (the
`assert_nonlinear_by` resourceout above is reliable; the corpus `sum` /
`binary_search` prove even at `--rlimit 1`, so they are NOT timeout fixtures — a
dedicated budget-hungry fixture is needed). Per OQ-1, a LIVE rlimit-exceeded run
is timing-fragile, so the DETERMINISTIC ladder test drives the state machine on
synthesized `VerusOutcome::Timeout` / `L2Result` inputs (hermetic), and the LIVE
forced-degrade conformance test is best-effort skip-loud — exactly the pattern
#11's `profile_conformance.rs` established.

## REQ status

All REQs are NOT-STARTED. `forge/src/degrade.rs` does not exist; there is no route
for it in `tooling/spec-routes.toml`; the default `forge check` path emits a
`VerusTimeout` L0 cert and STOPS instead of laddering. The pieces this component
COMPOSES (#11 classification, #9 L2, L1) all ship, but the auto-degrade
composition and the min-over-functions aggregate do not. Open prereq blocker:
**#50**.

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (degrade state machine L3→L2→L1) | NOT-STARTED | open prereq blocker #50. No `degrade.rs`; `check_file_with_options in check.rs` on a `VerusOutcome::Timeout` calls `Certificate::timeout` (`Level::L0` + `VerusTimeout`) and STOPS — documented in `check.rs`'s `solver-profiles REQ-7` row ("v0.1 does not auto-degrade (#10)") and `Certificate::timeout`'s doc; `check_l2_file`'s doc states "#9 does NOT wire L2 as an automatic fallback … that is #10". |
| REQ-2 (anti-cheat: a counterexample NEVER degrades) | NOT-STARTED | open prereq blocker #50. The classification that distinguishes the two (`VerusOutcome::{Timeout,Counterexample}` in `check.rs`) ships and is grounded (profile present = Timeout; absent = Counterexample), but no consumer routes a `Timeout` into L2 / a `Counterexample` into a hard fail vs degrade — there is no auto-degrade to make the distinction load-bearing yet. |
| REQ-3 (L1 fallback rung) | NOT-STARTED | open prereq blocker #50. `thermite_lower::lower_l1` (`pub fn lower_l1 in l1.rs`) ships and emits always-active runtime checks, but no degrade path invokes it as an L2-timeout fallback (it is wired only as build-time codegen, per `check.rs`'s slag-L1 comment "the L1 runtime-check codegen is thermite-lower's `l1.rs` job at build time, not here"). |
| REQ-4 (lowered-assurance flag + degrade reason on the cert) | NOT-STARTED | open prereq blocker #50. `Certificate` (`manifest.rs`) carries `solver_profile` + `suggested_move` (the reason material, #11) but NO `lowered-assurance` flag field; no producer sets a degrade flag because no degrade occurs. |
| REQ-5 (assurance manifest — per-fn aggregate) | NOT-STARTED | open prereq blocker #50. `check_file` returns `Vec<Certificate>` (the raw per-fn certs) and `cli::run_check` renders them individually, but there is no aggregate `struct` / `pub fn` computing a project-level view over the collection. |
| REQ-6 (min-over-functions project assurance) | NOT-STARTED | open prereq blocker #50. `enum Level { L0, L1, L2, L3 }` (`manifest.rs`) gives the ordering, but nothing computes the min over a cert collection or displays a project headline. |
| REQ-7 (determinism of the achieved level) | NOT-STARTED | open prereq blocker #50. The inputs are deterministic given pinned budgets (the pinned `DEFAULT_RLIMIT in check.rs`, the fixed `SLICE_BOUND in l2.rs`), and #11's classification is deterministic, but the ladder that would produce a deterministic achieved-level does not exist. |
| REQ-8 (subprocess failures never silently degrade) | NOT-STARTED | open prereq blocker #50. `run_verus` / `run_kani` already surface `ForgeError::{VerusAbsent,KaniAbsent,VerusOutput,KaniOutput}` (R-CODE-4 honored at the driver level), but no ladder exists to (correctly) NOT treat those as a degrade trigger — the invariant is unenforced because the ladder is absent. |
| REQ-9 (`fx diverge` L1 cap = partial-correctness rung, mutation/strengthen exempt, the #16 mirror) | SHIPPED | the diverge-L1-partial-correctness LEVEL is produced by the gate routing in `.design/forge/check.md` REQ-8 (`gate_fn`'s `fn_is_diverge` → `GateOutcome::DivergeL1(diverge_l1_cert(..))`, `Level::L1`, mutation/strengthen exempt). This doc owns the level SEMANTICS: the diverge L1 cap is a STRUCTURAL partial-correctness rung decided BEFORE any prover (NOT a TIMEOUT degrade — REQ-1 — and NOT a counterexample — REQ-2), the third distinct honest L1 cause alongside degrade-L1 and boundary/slag-L1. The min-over-functions aggregate treats it as a genuine L1 rung (the editor project caps at L1, not a hard failure). DIVERGE-ONLY: a non-diverge fn still proves termination (its `dec`) and still passes the §7 gate to reach L3 (`fn_is_diverge in lower.rs` keeps the #87 termination exemption diverge-only). Verified: `forge/tests/editor_runs.rs` (`run` L1 with no reject/strengthening + project assurance L1; non-diverge regressions still L0/termination-fail; corpus still L3). |

## Open questions

- **OQ-1 (live forced-timeout determinism — inherited from #11).** Provoking a
  LIVE verus resourceout is timing-fragile: the corpus proves even at `--rlimit 1`,
  and Z3 often returns `unknown` FAST (no profile report) rather than a
  resourceout on synthetic goals — so a `--profile` report is not always emitted on
  a low budget (the grounded `assert_nonlinear_by` resourceout IS reliable, but
  general items are not). RECOMMENDATION (ratify in #10): the DETERMINISTIC ladder
  logic is pinned by hermetic unit tests on synthesized `VerusOutcome` / `L2Result`
  inputs; the LIVE forced-degrade conformance test is best-effort skip-loud
  (the #11 `profile_conformance.rs` precedent). The achieved LEVEL is deterministic
  given a pinned budget; the question is only whether a given fixture reliably HITS
  the budget.

- **OQ-2 (the L2 counterexample-vs-timeout distinction at the second rung — the
  one I am LEAST confident about).** At L3 the timeout-vs-counterexample split is
  #11's `SolverProfile`-presence discriminator. At L2, Kani's `parse_kani_output`
  (`kani.rs`) already distinguishes `VERIFICATION:- SUCCESSFUL` (→ L2) from a
  `VERIFICATION:- FAILED` carrying `Failed Checks:` witnesses, and it parses the
  `unwinding assertion loop 0` UNDER-BOUND case as a reported failure
  (`under_bound_is_reported_failure`). But #10 must split that `FAILED` bucket
  TWO ways for the ladder: a genuine COUNTEREXAMPLE (a reachable contract `assert!`
  failed — a real bug, HARD FAIL per REQ-2) vs an INCONCLUSIVE bound exhaustion
  (`unwinding assertion loop` / kani ran out of unwind — degrade to L1). Today both
  are `Level::L0` in `L2Result`; the ladder needs to tell them apart. The
  `unwinding assertion` description is the candidate discriminator (a concrete
  `Failed Checks: assertion failed: <ens clause>` = counterexample; `unwinding
  assertion loop N` = under-bound/inconclusive), mirroring #11's stderr-shape
  approach. This is the riskiest decision: misclassifying a real L2 counterexample
  as "under-bound → degrade to L1" would hide a bug exactly as REQ-2 forbids. The
  conservative default (per §5.2 degrade-don't-block AND R-DEFER-9 never-hide-a-bug
  — which pull in OPPOSITE directions here) should favor R-DEFER-9: when the L2
  `FAILED` shape is ambiguous, treat it as a COUNTEREXAMPLE (hard fail), NOT a
  degrade, because hiding a bug is worse than over-reporting a failure.

- **OQ-3 (does L1 certification emit the runtime checks, or just record the level?).**
  REQ-3 drops a doubly-timed-out item to L1. Two readings: (a) the degrade must
  invoke `thermite_lower::lower_l1` to EMIT the always-active runtime checks at
  degrade time (so the shipped artifact actually carries them); or (b) the degrade
  merely RECORDS `Level::L1` + `lowered-assurance` on the cert, and the L1
  runtime-check EMISSION is `l1.rs`'s separate build-time job (the `check.rs`
  slag-L1 precedent: "the L1 runtime-check codegen is thermite-lower's `l1.rs` job
  at build time, not here"). Reading (b) matches the existing slag-L1 cert
  (`Certificate::slag_l1` records `Level::L1` WITHOUT running `lower_l1`), and keeps
  the ladder a pure verdict-aggregator. RECOMMENDATION: (b) — the degrade RECORDS
  the achieved level; emission stays `l1.rs`'s build-time concern. To ratify in
  #10.

- **OQ-4 (where the assurance manifest is emitted).** REQ-5 aggregates the per-fn
  certs. Candidates: (a) a `forge check`-emitted top-level manifest object
  (wrapping the `Vec<Certificate>` + the headline) — a schema addition; (b) an
  aggregate computed at render time in `cli::run_check` from the existing
  `Vec<Certificate>` (no schema change, the headline is a derived display). Reading
  (b) is additive and R-SPEC-2-safe (no frozen field touched, the golden per-item
  cert is unchanged); a future `forge audit` (Appendix B) may want the materialized
  manifest object of (a). RECOMMENDATION: (b) for #10's display + a small aggregate
  `struct` for the computation; the materialized `forge audit` manifest is a later
  surface. To ratify in #10.

- **OQ-5 (route registration).** `forge/src/degrade.rs` (and any assurance-manifest
  module) is not in `tooling/spec-routes.toml`. The orchestrator must add a route
  `crate_pattern = "forge/src/degrade.rs"` → `design = ".design/forge/degrade-ladder.md"`
  (and, if the aggregate lands in a separate module, a route for it) before the #10
  builder can edit it (R-XLATE-2/3). This doc-author does not edit the route table.
