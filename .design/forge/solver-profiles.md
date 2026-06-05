# Solver Profiles: timeouts as proof-repair prompts
<!--
tier: 3-component
status: draft
governs: forge/src/profile.rs
thesis-refs:
  - thermite-design.md §5.1
  - thermite-design.md §5.2
  - thermite-design.md §6
  - thermite-design.md §12
-->

## Summary

When Verus cannot PROVE an item within its resource budget — a TIMEOUT / rlimit
exhaustion, as opposed to a real COUNTEREXAMPLE — `forge check` must surface WHY,
not an opaque "timeout." This component captures Verus's `--profile` /
`--profile-all` Z3 quantifier-instantiation report, parses it into a structured
`SolverProfile` (the top-instantiated quantifiers / their selected triggers, the
instantiation counts and per-quantifier cost, the total instantiation count), and
renders it as actionable PROOF-REPAIR PROMPTS ("quantifier with trigger `e(x,y)`
instantiated 7 times — 63% of the total budget; likely a trigger loop; suggested
move: tighten or split the trigger"). This extends §5.1 ("counterexamples, not
adjectives") and §5.2 ("Forge emits a solver profile on every timeout … so
'maybe' becomes 'here's where I got lost'") to the timeout case.

It is GREENFIELD: `forge/src/profile.rs` does not yet exist; `check.rs`'s
`run_verus` path today classifies a non-clean proof identically whether it is a
counterexample or a timeout (`level_from_summary` → `Level::L0`, with the stderr
`error:` parsed as a witness). Telling the two apart is part of this work.

This component PRODUCES the structured prompts. It does NOT consume them to retry
(the proof-repair LOOP is issue #18) and does NOT perform the automatic
L3→L2→L1 degrade (issue #10). #11 produces; #10 degrades; #18 retries.

## Requirements

- **REQ-1 (`SolverProfile` schema)**: a `pub struct SolverProfile` capturing the
  parsed Z3 quantifier-instantiation report from Verus's profiler: the total
  user-level instantiation count, and a ranked list of `QuantifierProfile`
  entries (each: the selected trigger text, the instantiation count, the
  percentage of the total, the per-instantiation cost, the `cost * instantiations`
  product Verus ranks by, and the `file:basename:line:col` span of the
  quantifier). Diagnostic-only and NON-deterministic (§5.3) — like
  `solver_time_ms`, oracle-EXCLUDED. Derived from `thermite-design.md` §5.2.

- **REQ-2 (profile capture on an rlimit-hit)**: a `pub fn` that, given a Verus
  run that exhausted its resource budget, invokes Verus with the profiler enabled
  (`--profile` for the rlimit-exceeded path; the deterministic test fixture uses
  `--profile-all --verify-root`) and returns the captured profiler stderr blob
  for parsing. The profiler report lands on STDERR (the `--output-json` summary on
  stdout does NOT carry it). Derived from §5.2.

- **REQ-3 (parse the Z3 instantiation report)**: a `pub fn` parsing the grounded
  profiler stderr format (see Architecture) into a `SolverProfile`: the
  `note: Observed N total instantiations of user-level quantifiers` line and the
  repeated `note: Cost * Instantiations: <P> (Instantiated <N> times - <pct>% of
  the total, cost <C>) top <i> of <k> user-level quantifiers.` blocks, each
  followed by a `--> file:line:col` span and a `Triggers selected for this
  quantifier` source annotation. Best-effort and tolerant (do not over-fit to one
  Z3 version's wording). Derived from §5.1, §5.2.

- **REQ-4 (render proof-repair prompts)**: a `pub fn` turning a `SolverProfile`
  into one or more `SuggestedMove`s (`manifest.rs`, the §5.1 reserved slot) /
  structured prompt strings naming the top-instantiated quantifier and its
  trigger, the instantiation share, and a heuristic hint (trigger-loop suspicion
  when one quantifier dominates the budget; "add a `dec` / split the trigger /
  introduce a lemma" templates). Deterministic given a `SolverProfile` (the
  RENDERING is deterministic; the input profile is not). Derived from §5.1 pillar
  3 ("reserves a `suggested_move` slot populated by deterministic heuristics …
  trigger hints"), §5.2.

- **REQ-5 (timeout-vs-counterexample-vs-success classification in `run_verus`)**:
  `check.rs`'s `run_verus` / `parse_verus_output` path must distinguish three
  outcomes deterministically: (a) PROVED (`success == true && errors == 0` →
  `Level::L3`); (b) DISPROVED-COUNTEREXAMPLE (Z3 returned `sat` for some
  obligation → the §5.1 witness, the existing #5 path, NO profile); (c)
  TIMEOUT / RLIMIT-EXCEEDED (Z3 returned `unknown` with `:reason-unknown`
  `resourceout` / `canceled` → emit a `SolverProfile`). The classification is
  DETERMINISTIC; the profile CONTENT it attaches is not. Derived from §5.2, §6
  ("L3 … Not guaranteed → budget + downgrade"), §12 (the SMT-discontinuity risk).
  See OQ-1 for the empirically-confirmed difficulty of (b) vs (c).

- **REQ-6 (additive certificate slot, never a verdict mutation)**: the profile /
  prompts live in the certificate as an ADDITIVE field (mirroring #6's
  `slag_meta` / `reject` and #8's `cached`) OR populate the reserved
  `suggested_move` slot from #5 (`manifest.rs` `SuggestedMove`). The chosen slot
  is `#[serde(default, skip_serializing_if)]` so the frozen golden
  `conformance/sum.cert.json` still deserializes (R-SPEC-2 — additive only). The
  profile is EXCLUDED from `Certificate::oracle_subset` (non-deterministic, §5.3).
  Derived from §5.1, §6, `goal.md` R-SPEC-2. See OQ-2.

- **REQ-7 (timeout cert level, distinct from both L3 and a counterexample)**: a
  timeout cert records "not proved (timeout)" — NOT `Level::L3` (nothing was
  proved for all inputs) and distinguished from a counterexample-L0 by the
  attached reason (a timeout reason / profile vs a counterexample witness). v0.1
  has no automatic degrade (#10), so the level is the un-discharged level with a
  structured timeout reason; #11 does not itself degrade to L2/L1. Derived from
  §6, R-CODE-4 ("a timeout degrades the ladder … and is reported, never silently
  treated as success").

## Acceptance criteria

- **AC-1 (forced-timeout fixture → structured profile)**: a deterministic
  forced-timeout fixture (a quantifier-heavy `.th`/`.rs` harness run under
  `--profile-all --verify-root`, or a genuine rlimit-exceeded run) yields a
  `SolverProfile` whose top entry names the dominant quantifier's selected
  trigger and its instantiation count/share. Anchored to the grounded profiler
  format below (R-CHAR-3 — the expected fields are Verus's report shape, never
  forge's own output). Mechanically: parse the captured fixture stderr → assert
  `total_instantiations` and the top `QuantifierProfile.trigger` /
  `.instantiations` match the fixture's hand-derived expected values.

- **AC-2 (proof-repair prompt names the bottleneck)**: rendering the AC-1
  `SolverProfile` produces a prompt naming the top-instantiated quantifier's
  trigger and its share of the budget, with a trigger-loop hint when one
  quantifier dominates. Mechanically: assert the rendered `SuggestedMove.detail`
  contains the trigger text and the instantiation count.

- **AC-3 (three-way classification is deterministic)**: over three fixtures —
  the corpus `sum.th` (PROVED → `Level::L3`, NO profile), a broken-contract
  fixture (DISPROVED → counterexample, NO profile), and the forced-timeout
  fixture (TIMEOUT → a profile, NOT a counterexample) — `run_verus` classifies
  each into the correct one of the three outcomes, and the classification is
  byte-stable across runs (R-CODE-5). Mechanically: a table-driven test asserting
  the outcome tag per fixture; the profile CONTENT is asserted only structurally
  (oracle-excluded), the OUTCOME TAG is asserted exactly.

- **AC-4 (no profile on success or counterexample)**: a PROVED cert and a
  DISPROVED-counterexample cert carry NO `SolverProfile` (the additive field /
  `suggested_move` is `None`); only the timeout cert carries one. Mechanically:
  assert the profile slot `is_none()` on the `sum.th` cert and the broken cert,
  and `is_some()` on the timeout cert.

- **AC-5 (additive + oracle-excluded)**: the frozen golden
  `conformance/sum.cert.json` (which omits the profile slot) still deserializes
  into a `Certificate` (additive, R-SPEC-2), and a timeout cert with a profile is
  `oracle_subset`-EQUAL to the same cert with the profile stripped (the profile is
  provenance/diagnostic, never a verdict field, §5.3). Mechanically: golden
  round-trip + an `oracle_eq` assertion ignoring the profile.

- **AC-6 (rendering is deterministic, content is not)**: rendering the SAME
  `SolverProfile` twice is byte-identical (R-CODE-5); the test does NOT assert the
  profile is reproducible across Verus runs (it is not — §5.3 oracle-excluded).

## Architecture

The component is a parser + renderer over Verus's profiler output, plus a
classification refinement in `check.rs`. It sits beside the existing #5 verus
path (`run_verus` / `invoke_verus` / `parse_verus_output` in `check.rs`) and the
certificate schema (`Certificate` / `SuggestedMove` in `manifest.rs`).

### The grounded three-way distinction (real Verus 0.2026.05.24, Z3 4.12.5)

Captured by running the real `verus` binary at `~/.local/bin/verus`. The
`--output-json` `verification-results` summary on STDOUT is the level driver
today; the profiler report lands on STDERR.

**(a) PROVED.** Summary: `success: true, errors: 0, encountered-error: false`.
Exit 0. STDERR empty (no `--profile`). Existing `level_from_summary` → `L3`.

**(b) DISPROVED — counterexample.** A broken postcondition (`r == x + 2` for a
body returning `x + 1`). Summary: `success: false, errors: 1,
encountered-error: true, encountered-vir-error: false`. Exit 1. STDERR:

```text
error: postcondition not satisfied
 --> /tmp/broken_check.rs:5:13
  |
5 |     ensures r == x + 2,
  |             ^^^^^^^^^^ failed this postcondition
...
error: aborting due to 1 previous error
```

This is the existing #5 path: `parse_stderr_failures` turns it into an
`ObligationResult::failed` witness. NO profile is emitted.

**(c) TIMEOUT — rlimit exceeded.** Z3 ran out of its resource budget mid-search.
With `--profile` Verus emits the quantifier-instantiation report on STDERR. The
captured GROUNDED format (a transitivity / connectivity quantifier set under
`--profile-all --verify-root`):

```text
note: verifying root module
note: Analyzing prover log for root module ...
Z3 4.12.5
note: Log analysis complete for root module
note: Profile statistics for root module
note: Observed 11 total instantiations of user-level quantifiers
note: Cost * Instantiations: 70 (Instantiated 7 times - 63% of the total, cost 10) top 1 of 2 user-level quantifiers.

 --> /tmp/pa_check.rs:8:51
  |
8 |         forall|x: int, y: int, z: int| #[trigger] e(x, y) && #[trigger] e(y, z) ==> e(x, z),
  |         ------------------------------------------^^^^^^^---------------^^^^^^^------------ Triggers selected for this quantifier
note: Cost * Instantiations: 36 (Instantiated 4 times - 36% of the total, cost 9) top 2 of 2 user-level quantifiers.

 --> /tmp/pa_check.rs:7:43
  |
7 |         forall|x: int, y: int| #[trigger] e(x, y) ==> e(y, x),
  |         ----------------------------------^^^^^^^------------ Triggers selected for this quantifier
```

`SolverProfile` parses, from this:
- `total_instantiations = 11` (the `Observed N total instantiations` line).
- Two ranked `QuantifierProfile` entries: `{ cost_x_inst: 70, instantiations: 7,
  pct: 63, cost: 10, span: "pa_check.rs:8:51", trigger: "e(x, y) && e(y, z)" }`
  and the symmetry quantifier at 36 / 4 / 36% / cost 9.

### How a timeout is told apart from a counterexample (REQ-5)

This is the crux, and the empirical grounding (below) shows it is NOT free from
the `--output-json` summary alone:

- **The `--output-json` summary CANNOT distinguish (b) from (c).** Both report
  `success: false, errors: 1, encountered-error: true`, exit 1, and an identical
  STDERR `error: postcondition not satisfied … failed this postcondition`. A
  true-but-expensive goal under a low `--rlimit` (e.g.
  `(a²+b²+c²+d²+e²)*5 >= (a+b+c+d+e)²`, which IS valid) reports "postcondition not
  satisfied" verbatim — the same string a genuine counterexample produces. This
  is exactly the §12 "SMT discontinuity" risk made concrete.

- **The reliable signal is Z3's `:reason-unknown`.** The Verus binary's strings
  (`air/src/profiler.rs`, the SMT reader) confirm Z3 reports
  `(:reason-unknown "canceled")` and `(:reason-unknown resourceout)` for the
  rlimit/budget-out case, versus `sat` (a model → a genuine counterexample) and
  `(:reason-unknown incomplete)` / `"(incomplete quantifiers)"` for an
  incompleteness-unknown. The classification in `run_verus` must surface this
  distinction (the candidate mechanisms in OQ-1), because the JSON summary
  collapses (b) and (c).

- **`--profile` reports ONLY when the rlimit is exceeded** ("--profile reports
  prover performance data only when rlimits are exceeded, use --profile-all to
  always report profiler results"). So the PRESENCE of a profiler report on
  STDERR after a `--profile` run is itself a timeout signal — but see OQ-1: in
  practice Z3 often returns `unknown` FAST (without exhausting the rlimit) for
  these synthetic goals, so absence of a profile is NOT proof of a
  counterexample.

### Module shape (the contract, not a code proposal)

`profile.rs` (`pub` surface, consumed by `check.rs`'s timeout branch):
- `struct SolverProfile { total_instantiations, quantifiers: Vec<QuantifierProfile> }`
- `struct QuantifierProfile { trigger, instantiations, pct_of_total, cost,
  cost_x_instantiations, span }`
- `fn parse_profile(stderr: &str) -> Option<SolverProfile>` (REQ-3; tolerant).
- `fn capture_profile(...) -> Result<String, ForgeError>` (REQ-2; re-invokes
  Verus with the profiler enabled on the timed-out item — never swallows a spawn
  failure, R-CODE-4).
- `fn render_prompts(&SolverProfile) -> Vec<SuggestedMove>` (REQ-4; deterministic).

`check.rs` changes (refinement of the existing path, owned by the #11 builder):
the `parse_verus_output` / `run_verus` outcome becomes three-way (REQ-5), and the
timeout branch attaches the profile to the certificate via the REQ-6 slot.

`manifest.rs` (additive, R-SPEC-2): either a new
`Certificate.solver_profile: Option<SolverProfile>` additive field, or reuse the
existing reserved `Certificate.suggested_move: Option<SuggestedMove>` slot
(`SuggestedMove { kind, detail }` already exists and is documented as the home of
"trigger hints"). Either choice is `oracle_subset`-excluded. See OQ-2.

### Symbol anchors (existing code this component integrates with)

- `pub fn check_file` in `check.rs` — the pipeline that owns the per-item verus
  run and would attach the profile.
- `fn run_verus` / `fn invoke_verus` / `fn parse_verus_output` /
  `fn level_from_summary` / `fn parse_stderr_failures` in `check.rs` — the
  current two-way (success/failure) classification this component splits into
  three.
- `struct VerusSummary` in `check.rs` — the parsed `verification-results`; note it
  does NOT today carry a reason-unknown field (REQ-5 adds the distinction).
- `struct SuggestedMove` / `struct Certificate` / `fn oracle_subset` in
  `manifest.rs` — the reserved slot and the oracle-exclusion mechanism.

## Verification

- `cargo test -p forge` — unit tests over `parse_profile` against the grounded
  fixture stderr (AC-1), `render_prompts` (AC-2, AC-6), the three-way outcome
  table (AC-3, AC-4), and the additive/oracle-excluded cert property (AC-5).
- A FORCED-TIMEOUT FIXTURE checked in under `tests/golden/profile/` (or
  `conformance/profile/`): the captured profiler stderr blob from a deterministic
  `--profile-all --verify-root` run of a quantifier-heavy harness, so the parse
  test is hermetic (does NOT require provoking a live Z3 resourceout, which OQ-1
  shows is timing-fragile). The harness `.rs`/`.th` source is checked in beside
  it. Expected `total_instantiations` / top-trigger values are hand-derived from
  the captured blob (R-CHAR-3), never regenerated from forge.
- Conformance: `forge check conformance/sum.th` still emits the golden
  `sum.cert.json` (no profile, `Level::L3`) — the additive slot must not perturb
  the frozen oracle (AC-5).

### Constructing a deterministic forced-timeout fixture

Empirically (grounded), a reliable deterministic profiler report comes from
`--profile-all --verify-root` on a quantifier-bearing program — it ALWAYS reports
(no dependence on actually exhausting a budget), e.g. the captured transitivity
set: `forall|x,y| e(x,y) ==> e(y,x)` + `forall|x,y,z| e(x,y) && e(y,z) ==> e(x,z)`
with a few `e(i,i+1)` facts and a connectivity goal. This yields a stable
`Observed N total instantiations` report. For the live `--profile` (rlimit-exceeded)
path, a very low `--rlimit` on a genuinely budget-consuming nonlinear/quantifier
goal is the lever, but see OQ-1 on its fragility. The fixture for the parse test
should be the CAPTURED stderr blob, not a live run.

## REQ status

All REQs are NOT-STARTED: `forge/src/profile.rs` does not exist, and `check.rs`'s
`run_verus` does not yet distinguish a timeout from a counterexample (it is the
#5 two-way `level_from_summary` path). Blocked on issue **#11** (the feature
itself; milestone #2 v0.2 Ladder). This doc is forward-looking — the per-component
contract the #11 builder implements against.

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (`SolverProfile` schema) | NOT-STARTED | blocker #11. `forge/src/profile.rs` does not exist; no `SolverProfile` type. The profiler report it models is grounded (captured `note: Observed N total instantiations …` + `Cost * Instantiations` blocks). |
| REQ-2 (profile capture on rlimit-hit) | NOT-STARTED | blocker #11. No capture path; `invoke_verus` in `check.rs` runs `--output-json` only (no `--profile`/`--profile-all`). |
| REQ-3 (parse the Z3 report) | NOT-STARTED | blocker #11. No `parse_profile`. The grounded format is documented (Architecture) for the implementation to anchor to. |
| REQ-4 (render proof-repair prompts) | NOT-STARTED | blocker #11. No renderer; `manifest.rs` `SuggestedMove` exists but is documented as "reserved (currently un-constructed in production)" — never populated. |
| REQ-5 (three-way classification in `run_verus`) | NOT-STARTED | blocker #11. `level_from_summary` in `check.rs` is two-way (`L3` vs `L0`); `parse_verus_output` treats every non-clean run as a reported failure regardless of timeout vs counterexample; `VerusSummary` carries no reason-unknown field. |
| REQ-6 (additive cert slot, oracle-excluded) | NOT-STARTED | blocker #11. The slot does not exist; `Certificate.suggested_move` is reserved `None` and `Certificate::oracle_subset` returns only `(item, level, effects, slag)` — a profile field would be additive + excluded, but is unbuilt. |
| REQ-7 (timeout cert level, distinct) | NOT-STARTED | blocker #11. A timeout currently lands in the same `Level::L0` bucket as a counterexample (`level_from_summary`), with no timeout-vs-counterexample reason distinction in the cert. |

## Open questions

- **OQ-1 (timeout-vs-counterexample reliability — the central risk).** EMPIRICALLY
  GROUNDED and unresolved: the `--output-json` `verification-results` summary
  CANNOT distinguish a counterexample from a timeout — both are
  `success: false, errors: 1` with an identical `error: postcondition not
  satisfied` STDERR (confirmed by running a true-but-expensive goal under a low
  `--rlimit`, which reports "postcondition not satisfied" exactly like a real
  counterexample). The robust signal is Z3's `:reason-unknown` (`resourceout` /
  `canceled` = timeout; `sat` = counterexample; `incomplete` = an
  incompleteness-unknown). Candidate mechanisms for `run_verus` to obtain it,
  to be settled in implementation: (a) parse Z3's `:reason-unknown` from a Verus
  log / `--log-all` artifact; (b) treat the PRESENCE of a `--profile` report on
  STDERR as the timeout signal (but Z3 often returns `unknown` FAST without
  exhausting the rlimit on synthetic goals, so absence is not proof of a
  counterexample — fragile); (c) a Verus flag/JSON field that exposes
  reason-unknown directly, if one exists in this Verus version (not found in the
  `--output-json` summary as captured). Until settled, REQ-5's three-way
  classification may need a conservative default (treat an ambiguous `unknown` as
  a timeout — degrade-don't-block, §5.2 — rather than misreport a counterexample).
  This is the REQ I am least confident is cleanly mechanizable.

- **OQ-2 (cert slot: new additive field vs reuse `suggested_move`).** The scope
  permits either an additive `Certificate.solver_profile` field OR populating the
  reserved `suggested_move` slot. The rendered PROMPTS are a natural fit for
  `suggested_move` (§5.1 explicitly lists "trigger hints" as its content); the raw
  structured `SolverProfile` (instantiation counts, spans) is richer than the
  `SuggestedMove { kind, detail }` shape and may warrant its own additive field.
  Recommendation to ratify in implementation: a new additive
  `solver_profile: Option<SolverProfile>` for the structured data AND populate
  `suggested_move` from `render_prompts` for the human-facing hint — both
  oracle-excluded. Both are R-SPEC-2-additive.

- **OQ-3 (profiler invocation cost).** Capturing the profile re-invokes Verus
  with the profiler enabled (a second run of the timed-out item), or runs the
  profiler inline on the timeout run. The §11 anti-goal accepts slow verification,
  but the cheapest correct path (single run with `--profile`, parsed only when the
  timeout signal fires) should be preferred; to be settled with OQ-1.

- **OQ-4 (route registration).** `forge/src/profile.rs` is not yet in
  `tooling/spec-routes.toml`. The orchestrator must add a route
  `crate_pattern = "forge/src/profile.rs"` → `design = ".design/forge/solver-profiles.md"`
  before the #11 builder can edit it (R-XLATE-2/3). This doc-author does not edit
  the route table.
