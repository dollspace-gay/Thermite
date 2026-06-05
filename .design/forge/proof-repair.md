# Background Proof Repair: driving L1/L2 back up to L3

<!--
tier: 3-component
status: draft
governs: forge/src/repair.rs
thesis-refs:
  - thermite-design.md §5.2
  - thermite-design.md §5.3
  - thermite-design.md §6
  - thermite-design.md §12
  - thermite-design.md Appendix B
-->

## Summary

`forge repair [item]` is the **background L1/L2 → L3 upgrade loop** (Appendix B).
Once a `forge check` run has degraded an item below L3 (the #10 ladder, a verus
TIMEOUT degraded to L2/L1), repair is the standing background task that tries to
drive that item back UP to L3 — **mechanically where it can, and by surfacing the
repair PROMPT (the #11 solver profile + the failing obligation) for the rest.**
This is exactly §6's "driving L1s and L2s back up to L3 is a background task
agents can run unattended (proof repair is a local, checkable move — the task
shape LLMs are best at)" and §6's "upgrades are a standing background task."

The mechanical move repair attempts is **budget escalation**: an item that TIMED
OUT at the default `--rlimit` may PROVE with more SMT budget. Repair re-verifies
the item along a **fixed, bounded geometric escalation ladder** (2×, 4×, … up to
a cap), reusing the #8 proof cache so re-verifies are cheap; a budget at which the
item proves is a real, checkable upgrade to L3 (the "local, checkable move"). An
item that still does not prove at the cap stays sub-L3 and repair surfaces the
#11 repair prompt (the solver profile + the obligation) for the agent's own move
(an LLM-written custom proof hint — "agents run it unattended" = the loop + the
checkable feedback).

**THE ANTI-CHEAT INVARIANT (pinned hard, `goal.md` R-DEFER-9, §12).** Repair
escalates **INCONCLUSIVENESS** — a TIMEOUT (verus could not prove within budget)
— and NOTHING ELSE. It NEVER retries a COUNTEREXAMPLE (verus DISPROVED the
contract) or a vacuity / weak-contract REJECT, and it NEVER upgrades one to L3. A
disproved or vacuous item is a HARD FAIL; throwing more budget at it must not
"repair" it into a pass. This is grounded below: a genuinely FALSE contract gives
`postcondition not satisfied` (a counterexample, **no profile report**) at EVERY
budget from `--rlimit 1` to `--rlimit 200` — more budget never makes a false thing
true. Repair retries only items whose sub-L3 status is a TIMEOUT (the #11
classification, `VerusOutcome::Timeout` / a `Certificate.reject.cause ==
"VerusTimeout"` / a `lowered_assurance` cert carrying that degrade reason); it
REPORTS (never upgrades) counterexamples and rejects.

It is GREENFIELD: `forge/src/repair.rs` does not exist, there is no `Command::Repair`
in `cli.rs` (`parse_args` matches only `new` / `check` / `audit`), and no route for
it in `tooling/spec-routes.toml`. The pieces repair COMPOSES all ship — the #10
ladder + degrade (`degrade.rs`), the #11 solver profile = the repair prompt
(`profile.rs`), the #8 proof cache (`cache.rs`), the `--rlimit` budget seam
(`check::check_file_with_rlimit`), and the lower.rs shape-keyed proof-aid templates
— but the repair loop, the escalation ladder, the anti-cheat gating, and the prompt
surfacing did not until #18. **All REQs are now SHIPPED (issue #18);** see the REQ
status table below. OQ-1/OQ-2/OQ-4 are ratified per their RECOMMENDATIONs (hermetic
loop logic + live anti-cheat; escalated verifies uncached in v0.5; `forge repair` is
a one-shot re-runnable CLI pass, no daemon).

OUT OF SCOPE (boundaries): the multi-agent Forge orchestration is **#20**; the
critic-model spec-review slot is **#19**; the solver profile itself is **#11**
(done — reuse it as the prompt); the auto-degrade ladder that PRODUCES the sub-L3
certs repair consumes is **#10** (done — reuse it). #18 is the repair loop +
budget/aid escalation + the anti-cheat + the prompt surfacing, nothing more.

## Requirements

- **REQ-1 (`forge repair [item]` — the upgrade loop)**: a `pub fn` (and a
  `Command::Repair` dispatch in `cli.rs`) that, given a `.th` file (optionally a
  single item), (a) identifies the **sub-L3 items** (the certs the #10 ladder /
  #11 classification left below L3 — `Level::{L0,L1,L2}` whose degrade/reject
  reason is a TIMEOUT), (b) for each TIMEOUT item, **re-verifies at escalated
  budget** along the bounded ladder, (c) **upgrades to L3** on the first budget
  that PROVES (recording the budget that worked), and (d) otherwise leaves it
  sub-L3 and emits the #11 **repair prompt** (the `SolverProfile` +
  `suggested_move` + the failing obligation). Derived from Appendix B
  (`forge repair [item]` = "background L1/L2 → L3 upgrade loop"), §6 lines 185 +
  205.

- **REQ-2 (THE ANTI-CHEAT — only a TIMEOUT is retried; falsity is NEVER upgraded)**:
  repair retries an item ONLY when its sub-L3 verdict is a genuine
  INCONCLUSIVENESS — a verus TIMEOUT (the #11 `VerusOutcome::Timeout`, equivalently
  a `Certificate.reject` whose `cause == "VerusTimeout"`, or a `lowered_assurance`
  cert carrying that degrade reason). A **COUNTEREXAMPLE** (a `Level::L0` cert from
  `VerusOutcome::Counterexample` — verus DISPROVED the contract, with a failed
  `ObligationResult` and NO `solver_profile`), a **vacuity reject**
  (`Certificate::rejected_vacuity`, a `SemanticTautology` / `VacuousPrecondition`
  cause), and a **weak-contract reject** (`Certificate::rejected_weak_contract`)
  are HARD FAILS: repair REPORTS them (surfaces the witness / the reject reason) and
  NEVER retries them at a higher budget and NEVER upgrades them to L3. The repair
  loop must not change a sound non-certifying verdict into a falsely-better one.
  Derived from `goal.md` R-DEFER-9 ("never discharge an obligation by weakening it
  … a contract that won't verify is a real blocker"), §12, §7.

- **REQ-3 (the bounded escalation ladder)**: the budget escalation is a FIXED,
  BOUNDED ladder — a geometric sequence of `--rlimit` multipliers (e.g.
  `[2, 4, 8, 16]` × the default, with a hard CAP) tried in order, each a CHECKABLE
  re-verify. The ladder is finite (a max escalation count / a budget cap), so
  repair always terminates per item: an item that never proves even at the cap is
  reported still-sub-L3 with the repair prompt, never retried forever. The ladder
  is a frozen constant (no wall-clock, no adaptive budget derived from observed
  solver time), so the achieved level after repair is a deterministic function of
  the item + the pinned ladder (REQ-5). Derived from §5.2 (the fixed solver budget
  + degrade-don't-block), §11 ("Verification time is an accepted cost … never by
  weakening the gate").

- **REQ-4 (cache-backed re-verify, per item)**: each escalated re-verify reuses the
  #8 proof cache so a repair pass is cheap. A re-verify at an escalated budget is
  the SAME `run_verus` path `check.rs` already drives, content-addressed through
  `cache::cache_key`. NOTE the cache interaction (§Architecture): `check.rs`
  documents that a NON-default `--rlimit` BYPASSES the cache ("a timeout is never
  cached as proved") and that a `VerusTimeout` / `lowered_assurance` cert is NEVER
  cached. Repair runs at escalated (non-default) budgets, so each escalated rung is
  a fresh verify by the current rule; whether a *successful* escalated proof should
  be cached under its escalated budget (a fifth cache-key input, the budget) is
  **OQ-2**. Per-item locality (§5.3): repairing `f` never touches `g`'s cert.
  Derived from §5.3 ("proof results are content-addressed and cached per item"),
  Appendix B.

- **REQ-5 (determinism — the achieved level after repair is deterministic)**: given
  the pinned escalation ladder (REQ-3) and the pinned solver seed (§5.3), the level
  each item reaches after a repair pass is deterministic and re-runnable: the same
  item that proves at the `×16` rung this run proves at `×16` next run. The repair
  REPORT's diagnostic content (the residual `SolverProfile` instantiation counts) is
  NON-deterministic (§5.3) and oracle-EXCLUDED, exactly like #11's profile; the
  achieved LEVEL and the upgraded/still-sub-L3 verdict are deterministic
  (R-CODE-5). Repair introduces no wall-clock / unseeded input into the verdict.
  Derived from §5.3, R-CODE-5.

- **REQ-6 (the repair report — per sub-L3 item)**: repair emits, per sub-L3 item,
  one of two outcomes: **upgraded-to-L3** (with the budget rung that worked — the
  escalated `--rlimit` at which the item proved), or **still-sub-L3** (with the #11
  repair PROMPT: the `SolverProfile` + the `profile::suggested_move` headline + the
  failing obligation — the actionable "here's where I got lost" for the agent to act
  on with a custom proof hint). The report is structured (the §5.1 stable schema,
  human + `--json`), like every other Forge output. An item with NO sub-L3 obligation
  (already L3, the corpus case) is a NO-OP — not in the report's repair set. Derived
  from §5.1 ("every message is a prompt"), §5.2 (the solver profile is "actionable
  for proof repair"), §6.

- **REQ-7 (subprocess failures never silently degrade a repair verdict)**: an
  ENVIRONMENT failure during an escalated re-verify (verus absent
  `ForgeError::VerusAbsent`, unparseable output `ForgeError::VerusOutput`, a VIR /
  internal error) is a `ForgeError`, NOT a "still-sub-L3" verdict and NOT a silent
  upgrade. A repair upgrade is granted ONLY by a genuine `VerusOutcome::Proved` at an
  escalated budget; "the prover wasn't there" or "I couldn't read the output" is an
  error, never an upgrade or a degrade (R-CODE-4: "no swallowing of
  solver/subprocess failures"). Derived from R-CODE-4.

## Acceptance criteria

The orchestrator authors a `conformance/repair/` oracle. The EXACT fixtures
(below) and expectations trace to the design + the PROVERS' grounded output
(R-CHAR-3 — never copied from forge's own output).

- **AC-1 (corpus already L3 → repair is a NO-OP)**: `forge repair conformance/sum.th`
  at the default budget finds NO sub-L3 item (both `spec_sum` and `sum` certify
  `Level::L3`, grounded: the golden `conformance/sum.cert.json` is `"level": "L3"`),
  so repair attempts no escalation and the report's repair set is empty. Mechanically:
  the repair report over the corpus lists zero items to repair; the corpus certs are
  unchanged. GROUNDED below.

- **AC-2 (forced-timeout-but-provable item → repair UPGRADES it to L3, records the
  budget)**: a fixture whose obligation **TIMES OUT** at the default/low `--rlimit`
  (verus reports `Resource limit (rlimit) exceeded` + a `--profile` instantiation
  report present on stderr → `VerusOutcome::Timeout`) but **PROVES** at a higher
  rung of the ladder → repair UPGRADES it to `Level::L3` and records the escalated
  budget that worked. **Fixture: `conformance/repair/provable_with_budget.th`** — the
  grounded `(a*b)^9 == a^9·b^9` nonlinear monomial reassociation: it RESOURCE-OUTS at
  `--rlimit 1,2,4,8` and PROVES at `--rlimit 16,32,64` (grounded below — real verus
  `0.2026.05.24`, Z3 4.12.5). Mechanically: assert the repaired cert is `Level::L3`
  and the recorded repair budget is the first ladder rung ≥ 16. GROUNDED below.

- **AC-3 (THE KEY ANTI-CHEAT AC — a counterexample is NEVER upgraded)**: a fixture
  whose contract is genuinely **FALSE** (verus returns `VerusOutcome::Counterexample`
  — `postcondition not satisfied`, NO profile report) is a HARD FAIL: repair does
  NOT retry it at ANY escalated budget and does NOT upgrade it to L3. The report
  surfaces the COUNTEREXAMPLE (the witness / failing obligation), NOT a false L3.
  **Fixture: `conformance/repair/never_provable_counterexample.th`** — the grounded
  `inc(x) -> r ensures r == x + 2 { x + 1 }`: it gives `postcondition not satisfied`
  with NO `Resource limit` message and NO profile report at EVERY budget `--rlimit
  1,4,16,64,200` (grounded below). Mechanically: assert the item's cert stays
  non-certifying (`Level::L0`, the counterexample obligation), carries NO
  `lowered_assurance` and is NOT `Level::L3`, and that NO escalation rung was even
  attempted (the loop short-circuits on a non-`Timeout` verdict). A regression here
  laundering a false contract into L3 is the worst possible failure (§12, R-DEFER-9)
  — the highest-value test. GROUNDED below.

- **AC-4 (bounded escalation — an item that never proves even at the cap stays
  sub-L3 + the prompt)**: a fixture that TIMES OUT at every rung up to the cap (never
  proves) → repair leaves it sub-L3 and emits the #11 repair prompt (the
  `SolverProfile` + `suggested_move` + the obligation), having tried the ladder a
  BOUNDED number of times and stopped. Mechanically: assert the loop made exactly
  the ladder's rung-count of attempts, the verdict stays sub-L3, and the report
  carries the prompt (not an upgrade). A candidate fixture is a higher-degree
  monomial (the grounded N=10 reassociation resource-outs even at `--rlimit 30`); per
  OQ-1 a LIVE always-timeout is timing-fragile, so the DETERMINISTIC bound is pinned
  by a hermetic test driving the loop on synthesized per-rung `VerusOutcome::Timeout`
  results.

- **AC-5 (determinism)**: re-running `forge repair` over a fixture at the pinned
  ladder yields the byte-identical achieved level + the same recorded repair budget
  across runs (R-CODE-5); the residual repair prompt's profile content is asserted
  only structurally (oracle-excluded). Mechanically: two runs, equal achieved level
  + equal recorded budget.

- **AC-6 (subprocess failure is an error, never an upgrade)**: with `verus` removed
  from `PATH` during an escalated re-verify, repair returns `ForgeError::VerusAbsent`
  — NOT a "still-sub-L3" verdict and NOT a silent upgrade (R-CODE-4). Mechanically: a
  hermetic test asserting the environment error propagates out of the repair loop.

## Architecture

Repair is a per-item loop driver (`repair.rs`) plus a `Command::Repair` dispatch in
`cli.rs`. It sits AFTER `forge check` (it consumes the sub-L3 certs the #10 ladder
produced) and BELOW the verus driver (it re-invokes the SAME `run_verus` path at
escalated budgets). It owns NO new prover-invocation logic — it composes the #11
classification, the #10 ladder's verdict shapes, the #8 cache, and the `--rlimit`
seam.

### The repair loop (REQ-1, REQ-2, REQ-3)

```text
            forge repair <file> [item]
                     │
                     ▼
     check_file_with_rlimit(file, DEFAULT_RLIMIT)   (re-)derive per-item certs
                     │
        ┌────────────┴───────────────────────────────────┐
        ▼                          ▼                       ▼
   level == L3              reject == VerusTimeout    Counterexample / vacuity /
   (or no reject)           OR lowered_assurance      weak-contract reject
        │                   (a TIMEOUT — INCONCLUSIVE) (FALSITY / degenerate)
        ▼                          │                       │
   NO-OP (already L3,              ▼                       ▼
   not in repair set)      ESCALATION LADDER         HARD FAIL — REPORT only
                           ×2 ─▶ ×4 ─▶ ×8 ─▶ ×16      (REQ-2 anti-cheat:
                           (bounded, capped)          NEVER retried, NEVER
                                  │                   upgraded to L3)
                   ┌──────────────┴──────────────┐
                   ▼                              ▼
              Proved at rung r            Timeout at every rung ≤ cap
                   │                              │
                   ▼                              ▼
           UPGRADE to L3                  STILL sub-L3 +
           (record budget r)              the #11 repair PROMPT
                                          (SolverProfile + suggested_move
                                           + the failing obligation)
```

The TRIGGER for an escalation is a classified **Timeout**, never a Counterexample
or a reject — the exact same anti-cheat the #10 ladder enforces on the way DOWN,
mirrored on the way UP. At L3 the discriminator is #11's `classify_verus_outcome`
(`fn classify_verus_outcome in check.rs` → `enum VerusOutcome { Proved, Timeout,
Counterexample }`): a `--profile` instantiation report present on stderr =
`Timeout` (retry); an error WITHOUT a profile = `Counterexample` (hard fail, never
retry).

### Composition with the shipped pieces (symbol anchors)

- `pub fn check_file_with_rlimit in check.rs` (and `check_file_with_options`) — the
  re-verify entry repair calls at each escalated rung. The `--rlimit <FLOAT>` seam
  is already exposed (`cli.rs`'s `--rlimit` flag → `run_check`); repair drives the
  same seam programmatically along the ladder.
- `fn classify_verus_outcome in check.rs` → `enum VerusOutcome { Proved, Timeout,
  Counterexample }` — the retry TRIGGER. `Timeout` → escalate; `Proved` → upgrade;
  `Counterexample` → hard fail (REQ-2). The `Timeout` discriminator is the PRESENCE
  of a `--profile` report (`profile::parse_profile` returns `Some`).
- `enum L3Verdict in degrade.rs` (`Proved` / `Timeout { reason }` / `Counterexample`)
  and `pub fn run_ladder in degrade.rs` — the #10 ladder that PRODUCED the sub-L3
  certs repair consumes. A `lowered_assurance` cert with a `VerusTimeout`
  `degrade_reason` is the down-ladder's record of the very TIMEOUT repair retries.
- `struct Certificate` / `Certificate.reject` (`RejectReason { cause, detail }`) /
  `Certificate.lowered_assurance` / `Certificate.degrade_reason` /
  `Certificate.solver_profile` / `Certificate.suggested_move` /
  `Certificate.level` + `enum Level { L0, L1, L2, L3 }` in `manifest.rs` — the cert
  fields repair READS to classify a sub-L3 item (TIMEOUT vs counterexample vs reject)
  and WRITES on an upgrade (the new `Level::L3` cert). The `VerusTimeout` cause is
  set by `Certificate::timeout`; the counterexample cert is `Certificate::new` at
  `Level::L0` with a failed obligation and NO profile; the vacuity reject is
  `Certificate::rejected_vacuity`; the weak-contract reject is
  `Certificate::rejected_weak_contract`.
- `struct SolverProfile in profile.rs` + `pub fn suggested_move in profile.rs` +
  `pub fn render_prompts in profile.rs` — the #11 repair PROMPT repair surfaces on a
  still-sub-L3 item (REQ-6). #11 PRODUCES the prompt; #18 retries and, on failure,
  surfaces it.
- `pub fn cache_key in cache.rs` / `pub fn load` / `pub fn store` — the #8
  content-addressed cache each re-verify rides (REQ-4). NOTE: `check.rs` already
  bypasses the cache at a non-default `--rlimit` and never caches a `VerusTimeout` /
  `lowered_assurance` cert; repair runs at escalated (non-default) budgets, so by the
  current rule each escalated rung is a fresh verify (see OQ-2 on caching a
  successful escalated proof).
- `thermite_lower::lower` (the shape-keyed proof-aid templates lower.rs already
  emits) — the substrate of OQ-3's proof-aid escalation; budget escalation is the
  v0.5 mechanical move, proof-aid escalation is the open extension.
- `enum Command in cli.rs` (`parse_args`, the hand-rolled matcher over
  `new`/`check`/`audit`) + `fn run in cli.rs` — the consumer. A new
  `Command::Repair { file, item, json }` arm dispatches to a `run_repair` that
  renders the per-item repair report (REQ-6), mirroring `run_check` / `run_audit`.

### How `forge repair` differs from `forge check`

`forge check` runs the ladder DOWN (L3 → L2 → L1 on a timeout, #10) and STOPS at the
achieved rung. `forge repair` runs the ladder back UP for the TIMEOUT items only:
it re-verifies at escalated budget to try to recover L3. The two are duals across
the same anti-cheat boundary — `check` never degrades a counterexample DOWN to L1
(#10 REQ-2), and `repair` never escalates a counterexample UP to L3 (REQ-2 here).
Both refuse to let falsity move along the ladder; only inconclusiveness (a TIMEOUT)
moves.

### Why budget escalation is a REAL, checkable upgrade (the grounded core)

The whole loop rests on one empirical fact: **some TRUE obligations time out at a
low SMT budget but PROVE at a higher one** — and a FALSE obligation proves at NO
budget. Both halves are grounded below against the real verus binary; the upgrade
is a genuine re-verification (verus actually proves it at the escalated rung), not a
relabel. This is the §6 "local, checkable move."

## Verification

- `cargo test -p forge` — hermetic unit tests over the repair loop driven on
  SYNTHESIZED per-rung `VerusOutcome` / `Certificate` inputs (the #10/#11 precedent,
  to avoid depending on provoking a live resourceout, which OQ-1 shows is
  timing-fragile): (REQ-2) a `Counterexample` / vacuity / weak-contract sub-L3 cert
  is NEVER escalated (the loop short-circuits — the key anti-cheat test; the
  escalation closure must NOT run); (REQ-1) a `Timeout` cert that proves at rung r
  upgrades to L3 with budget r recorded; (REQ-3/AC-4) a `Timeout` cert that times out
  at every rung makes exactly the ladder's rung-count of attempts then stops + emits
  the prompt; (REQ-5) determinism of the achieved level; (REQ-7/AC-6) an environment
  error propagates, never an upgrade.
- A `conformance/repair/` oracle (the orchestrator authors it): the two EXACT
  fixtures `provable_with_budget.th` (AC-2 — upgrades to L3, records the budget) and
  `never_provable_counterexample.th` (AC-3 — stays a hard fail, prompt reports the
  counterexample, never a false L3), plus the corpus no-op (AC-1). Per OQ-1, the LIVE
  forced-timeout conformance test is best-effort skip-loud; the DETERMINISTIC loop
  logic is pinned by the hermetic unit tests.
- Conformance no-op: `forge repair conformance/sum.th` finds no sub-L3 item (AC-1) —
  repair must not perturb the frozen golden `sum.cert.json`.
- The anti-cheat AC-3 is pinned as the highest-value test: a regression upgrading a
  counterexample to L3 hides a real bug behind a proof stamp — the exact failure
  §12 / §7 / R-DEFER-9 exist to prevent.

### The grounded budget escalation + anti-cheat (real verus, this session)

Real verus `0.2026.05.24.ecee80a`, Z3 4.12.5. R-CHAR-3: the expected shapes are
verus's output, never forge's.

**Budget-escalation upgrade is REAL (AC-2).** The nonlinear monomial reassociation
`(a*b)^9 == a^9 · b^9` (`provable_with_budget.th`), a TRUE goal proved
`by(nonlinear_arith)`, under `verus --output-json --profile --rlimit <RL>`:

```text
rlimit=1  success=False errors=1  Resource-limit-exceeded=1  profile(Observed)=1   (TIMEOUT)
rlimit=2  success=False errors=1  Resource-limit-exceeded=1  profile=1             (TIMEOUT)
rlimit=4  success=False errors=1  Resource-limit-exceeded=1  profile=1             (TIMEOUT)
rlimit=8  success=False errors=1  Resource-limit-exceeded=1  profile=1             (TIMEOUT)
rlimit=16 success=True  errors=0                                                   (PROVED → L3)
rlimit=32 success=True  errors=0                                                   (PROVED → L3)
rlimit=64 success=True  errors=0                                                   (PROVED → L3)
```

The `--rlimit 1` stderr is the textbook timeout the loop retries:

```text
error: assert_nonlinear_by: Resource limit (rlimit) exceeded
 --> .../esc9.rs:5:3
note: Observed N total instantiations of user-level quantifiers   (the --profile report)
```

`classify_verus_outcome` reads the `Observed` line (`profile::parse_profile` returns
`Some`) → `VerusOutcome::Timeout`, NOT `Counterexample`. Escalating the budget along
the ladder `[2,4,8,16]` re-verifies and `--rlimit 16` returns `success: true,
errors: 0` → the item UPGRADES to `Level::L3`, recording the `×16` rung. This is the
mechanical repair: the SAME obligation, a larger budget, a real verus proof.

**The anti-cheat is REAL (AC-3).** A genuinely FALSE contract
(`never_provable_counterexample.th`): `fn inc(x: u32) -> (r: u32) requires x <
1000, ensures r == x + 2 { x + 1 }`, under `verus --profile --rlimit <RL>`:

```text
rlimit=1   success=False errors=1  Resource-limit=0  profile=0  postcondition-fail=1
rlimit=4   success=False errors=1  Resource-limit=0  profile=0  postcondition-fail=1
rlimit=16  success=False errors=1  Resource-limit=0  profile=0  postcondition-fail=1
rlimit=64  success=False errors=1  Resource-limit=0  profile=0  postcondition-fail=1
rlimit=200 success=False errors=1  Resource-limit=0  profile=0  postcondition-fail=1
```

```text
error: postcondition not satisfied
 --> .../anticheat.rs:5:13
5 |     ensures r == x + 2,
```

At EVERY budget the verdict is `postcondition not satisfied` — a COUNTEREXAMPLE,
with NO `Resource limit` message and NO `--profile` report (`profile::parse_profile`
returns `None` → `VerusOutcome::Counterexample`). More budget never makes it true.
This is structurally distinct from the timeout (**profile ABSENT vs PRESENT**), and
repair MUST treat it as a hard fail — never retry it, never upgrade it. This is the
load-bearing distinction: repair escalates only the profile-PRESENT (TIMEOUT) items.

**Corpus is L3 at default → repair no-op (AC-1).** The golden
`conformance/sum.cert.json` is `"level": "L3"`; `sum` and `spec_sum` both prove at
the default budget, so there is no sub-L3 item for repair to retry.

### A note on the timing-fragility of the boundary (OQ-1)

The clean window above (resource-out at `≤8`, proves at `≥16`) was found by
bisecting the monomial degree: degree ≤ 8 proves even at `--rlimit 1`; degree 9 is
the clean escalation fixture; degree ≥ 10 resource-outs even at `--rlimit 30`
(a candidate AC-4 never-proves fixture). The exact rung at which a goal flips is
Z3-version-sensitive, so the LIVE forced-timeout fixtures are best-effort skip-loud
and the DETERMINISTIC loop logic is pinned hermetically — exactly the #10/#11
pattern.

## REQ status

All REQs are SHIPPED (issue #18). `forge/src/repair.rs` exists (the repair loop +
the bounded escalation ladder + the anti-cheat gate); `cli.rs` has the
`Command::Repair` arm (`parse_args` matches `repair`; the `usage` string lists it)
+ `run_repair`/`render_repair`; `main.rs` declares `mod repair;`. The pieces repair
COMPOSES all ship and are reused: the #10 ladder's `VerusTimeout` degrade reason +
#11's `VerusOutcome`-classified `VerusTimeout` reject = the timeout trigger, the #11
`SolverProfile`/`suggested_move` = the surfaced prompt, the `--rlimit` seam
(`check::check_file_with_rlimit`) = the escalated re-verify.

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (`forge repair [item]` upgrade loop) | SHIPPED | `pub fn repair_file in repair.rs` re-derives the certs at `DEFAULT_RLIMIT`, classifies via `classify_sub_l3`, and for a `SubL3Status::Timeout` drives `escalate` along `REPAIR_LADDER`, upgrading to L3 on the first proving rung (recording the budget) else surfacing the prompt. Consumer: `fn run_repair in cli.rs` (the `Command::Repair` arm). Hermetic test `escalation_upgrades_at_the_proving_budget`. |
| REQ-2 (anti-cheat: only a TIMEOUT is retried; falsity NEVER upgraded) | SHIPPED | `fn classify_sub_l3 in repair.rs` returns `SubL3Status::Timeout` ONLY for a `VerusTimeout` reject / a `lowered_assurance` `VerusTimeout` degrade; a counterexample / vacuity / weak-contract reject / bare-`L0` returns `NotRepairable`. `fn repair_item` calls `escalate` (and thus the verify closure) ONLY on `Timeout`; a `NotRepairable` short-circuits with the closure UNUSED. Hermetic `counterexample_is_never_retried` (asserts 0 closure calls) + `rejects_are_never_retried`; LIVE `counterexample_is_never_upgraded` in `repair_conformance.rs` (the grounded `inc` stays L0 / not-repairable through real verus). |
| REQ-3 (bounded escalation ladder) | SHIPPED | `pub const REPAIR_LADDER: [f64; 4] = [2.0, 4.0, 8.0, 16.0] in repair.rs` (multipliers of `DEFAULT_RLIMIT`); `fn escalate` iterates it and STOPS at the cap → at most `REPAIR_LADDER.len()` re-verifies, always terminates. Hermetic test `escalation_is_bounded_and_terminates` (asserts exactly the rung-count of attempts). |
| REQ-4 (cache-backed re-verify, per item) | SHIPPED | each rung calls `check::check_file_with_rlimit` at a NON-default budget; per the `check.rs` rule a non-default `--rlimit` bypasses the cache (a budget-dependent verdict is never cached as the canonical proof). OQ-2 reading (a) ratified for the v0.5 kernel: escalated verifies are uncached, no #8 cache-key change. Evidence: the `verify` closure in `repair_file`. |
| REQ-5 (determinism of the achieved level after repair) | SHIPPED | `REPAIR_LADDER` is a frozen const + the per-rung verdict is `check::classify_verus_outcome` under the pinned `DEFAULT_SOLVER_SEED`, so the achieved level + the recorded winning budget are deterministic (R-CODE-5). Hermetic test `escalation_is_deterministic` (two runs, equal budget). |
| REQ-6 (the repair report — per sub-L3 item) | SHIPPED | `pub struct RepairReport`/`RepairItem` + `enum RepairOutcome { UpgradedToL3 { budget }, StillSubL3 { level, profile, suggested_move, detail }, NotRepairable { level, cause, detail } } in repair.rs`; an already-L3 item is a NO-OP (not in `items`). Rendered by `fn render_repair`/`render_repair_item` + `repair_report_json in cli.rs`. LIVE no-op `corpus_sum_is_a_repair_noop` (empty repair set). |
| REQ-7 (subprocess failures never silently degrade a repair verdict) | SHIPPED | `fn escalate`'s verify closure returns `Result<RepairVerdict, ForgeError>`; an environment failure propagates via `?` out of `repair_file`, NEVER a still-sub-L3 verdict and NEVER an upgrade (R-CODE-4). Hermetic test `environment_error_propagates`. |

## Open questions

- **OQ-1 (live forced-timeout determinism — inherited from #10/#11, the one I am
  LEAST confident about for the LIVE fixtures).** Provoking a LIVE verus resourceout
  that ALSO proves at a higher budget is a narrow window: nonlinear goals are
  cliff-shaped (the grounded monomial proves fast at degree ≤ 8, resource-outs
  indefinitely at degree ≥ 10; only degree 9 has a clean resource-out-at-low /
  proves-at-high window), and the exact rung is Z3-version-sensitive. RECOMMENDATION
  (ratify in #18): pin the DETERMINISTIC loop logic with hermetic unit tests on
  synthesized per-rung `VerusOutcome` inputs (the #10/#11 precedent); make the LIVE
  `conformance/repair/` forced-timeout test best-effort skip-loud. The achieved LEVEL
  is deterministic given a pinned ladder; the only fragility is whether a given
  fixture reliably HITS the resource-out-then-proves window on a given Z3.

- **OQ-2 (does a successful escalated proof get cached — and under what key?).**
  `check.rs` today BYPASSES the proof cache at a non-default `--rlimit` and NEVER
  caches a `VerusTimeout` cert ("a timeout is never cached as proved"). Repair runs
  at escalated (non-default) budgets, so by the current rule each escalated rung is
  a fresh verify and a successful escalated proof is NOT cached — a second
  `forge repair` re-escalates from scratch. Two readings: (a) keep the current rule
  (escalated verifies are uncached; correct + simple, but a repeated repair pass
  re-pays the escalation); (b) extend `cache::cache_key` with a FIFTH verdict-input,
  the budget (the rlimit), so a *successful* escalated proof caches under
  `(lowered, seed, verus, thermite, check-schema, rlimit)` and a re-run is a cheap
  HIT. Reading (b) is sound (the budget IS a verdict-determining input, like the seed)
  and matches REQ-4's "re-verifies are cheap," but it is a cache-key change touching
  the #8 contract (`.design/forge/proof-cache.md`). RECOMMENDATION: (a) for the v0.5
  kernel of #18 (correct, no #8 contract change), (b) as a follow-up perf extension.
  To ratify in #18.

- **OQ-3 (how far does proof-aid escalation go in v0.5 — budget-only, or also the
  alternate shape-keyed templates?).** §6's mechanical repair has two candidate
  moves: BUDGET escalation (this doc's REQ-3, fully grounded) and PROOF-AID
  escalation — trying additional / alternate shape-keyed proof-aid templates that
  lower.rs already emits (a different invariant template, an added overflow-guard
  lemma). Budget escalation is purely a `--rlimit` re-verify (no source change, the
  cleanest "local, checkable move"); proof-aid escalation re-LOWERS the item with a
  different aid set, which is a larger move and risks blurring the line with the
  agent's own custom-hint move (the prompt-surfacing half of REQ-6). RECOMMENDATION:
  v0.5 #18 ships BUDGET escalation as the mechanical move and SURFACES the prompt for
  everything else (proof-aid choice becomes the agent's move, fed by the prompt);
  alternate-template proof-aid escalation is a deliberate follow-up so the mechanical
  vs agent boundary stays crisp. To ratify in #18.

- **OQ-4 (what does "background / unattended" mean for a CLI command in v0.5 —
  second-least-confident).** §6 calls repair "a background task agents can run
  unattended" and "a standing background task," and Appendix B lists `forge repair
  [item]` as a CLI verb. v0.1–v0.4 Forge is a synchronous CLI (no daemon, no watch
  loop), and multi-agent orchestration is explicitly #20 (out of scope). Two
  readings: (a) "background" is descriptive of the WORKLOAD shape (a checkable,
  parallelizable, patience-bounded task an agent loop invokes repeatedly), and
  `forge repair` in v0.5 is a one-shot CLI pass the agent's outer loop schedules —
  no daemon; (b) `forge repair` itself runs a standing watch/daemon loop. Reading (a)
  matches the v0.1–v0.4 synchronous-CLI precedent and keeps the daemon/orchestration
  concern in #20. RECOMMENDATION: (a) — `forge repair` is a one-shot, deterministic,
  re-runnable CLI pass over the sub-L3 items; "unattended/background" is the agent's
  outer-loop scheduling (and #20's multi-agent orchestration), not a daemon inside
  `forge repair`. To ratify in #18.

- **OQ-5 (route registration).** `forge/src/repair.rs` is not in
  `tooling/spec-routes.toml`. The orchestrator must add a route
  `crate_pattern = "forge/src/repair.rs"` → `design = ".design/forge/proof-repair.md"`
  before the #18 builder can edit it (R-XLATE-2/3), and the `Command::Repair`
  dispatch lands in `forge/src/cli.rs` (already routed to `.design/forge/cli.md`; the
  cli.md contract gains a repair-verb REQ, a cli.md amendment). This doc-author does
  not edit the route table.
