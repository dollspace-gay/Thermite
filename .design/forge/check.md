# Forge check pipeline

<!--
tier: 3-component
status: draft
audited-sha: 64a329fac3ece936c17be3d6aadf5bc125b20682 (re-pinned: the #269/#270 coordinated arc — the touched-file changes are exactly this doc's designed REQs / reviewed as claim-neutral)  (re-audited 2026-06-12: amended — stale GREENFIELD line, per-item Vec<Certificate> signature, dead parse_verus_output/level_from_summary cites, test-name fixes, engine-routing/hole-gate amendment, #262)
governs: forge/src/check.rs
thesis-refs:
  - thermite-design.md §5.1
  - thermite-design.md §5.3
  - thermite-design.md §6
  - thermite-design.md Appendix A
-->

## Summary

`forge/src/check.rs` is the `forge check` pipeline: it runs a `.th` file
end-to-end through every shipped kernel component — PER ITEM (§5.3): each `fn`
is lowered and verified in an isolated sub-program — invokes the REAL `verus`
binary on the lowered source, parses verus's output into per-obligation
results (with counterexamples on failure), and assembles one structured
certificate per item (`manifest.rs`, `.design/forge/certificate-manifest.md`),
returning `Vec<Certificate>`. It is the
FIRST LIVE cert-oracle: `forge check conformance/sum.th`'s deterministic
certificate fields must match the golden `conformance/sum.cert.json`.

This component is SHIPPED (`forge/src/check.rs`; all REQs SHIPPED — see the
REQ status table).

> **Amendment 2026-06-12 (doc-freshness re-audit, #262).** Re-verified against the
> current tree (`dff9ae86`, 16 post-pin commits to `check.rs`). Corrections:
> 1. *Entry shape.* `pub fn check_file in check.rs` is ONE-argument
>    (`path`) and returns `Vec<Certificate>` (one cert PER ITEM, §5.3) — not the
>    `(path, seed) -> Certificate` shape this doc originally sketched. The seed is
>    resolved internally (`fn resolve_seed` → `DEFAULT_SOLVER_SEED`); the variants
>    `check_file_with_rlimit` / `check_file_with_options` (the `CheckOptions`
>    surface) / `check_file_with_engine` (the #247 `--engine` surface) layer on it.
> 2. *Dead symbol cites.* `parse_verus_output` and `level_from_summary` no longer
>    exist: the verus output path is `fn classify_verus_outcome` (the #11
>    three-way Proved/Timeout/Counterexample classifier; `Proved` ⇒ `Level::L3`)
>    feeding `fn assemble_certificate`. REQ-1/REQ-3/REQ-5 rows updated.
> 3. *Engine routing (#204/#247/#248, governed by
>    `.design/verified/proof-backends.md`).* The L3 CONTRACT discharge is now
>    routed behind the backend-neutral Engine seam: `fn mint_item_obligations`
>    mints the per-item Obligation set (CONTRACT + REGISTRY-TERMINATION),
>    `engine::engine_cache_key` discriminates the cache address, and
>    `fn ladder_for_timeout` threads the obligation through `engine::VerusEngine`
>    (the default); `check_file_with_engine` is the Lean path
>    (`lean_proven_cert` / `lean_interactive_proven_cert`, attaching
>    `engine_attribution`). The verus INVOCATION itself (`run_verus` /
>    `invoke_verus`) is unchanged in shape.
> 4. *Open-hole gate (#193/#195, governed by `.design/forge/goal-repl.md`).* A fn
>    carrying an open `?N` hole is REFUSED before lowering: the per-item loop emits
>    a non-certified L0 cert with cause `"OpenHole"` via the shared
>    `goal_repl::open_hole_reason`.
> 5. *Test-name fixes.* The verus-absent integration coverage is
>    `cache_conformance::cold_cache_with_verus_unavailable_is_environment_error`
>    (not the never-existing `verus_absent_is_environment_error_not_l3`); the AC-7
>    editor witness is `editor_runs::editor_logic_certifies_l3_boundary_and_run_l1`.
> 6. *Scratch naming.* `run_verus` writes `<stem>.rs` (no-`.` stem — the AC-4
>    property, unchanged) inside the per-run scratch DIR
>    `forge_<stem>_<pid>_<n>/` (`unique_scratch_dir`), removed wholesale by the
>    `ScratchDir` Drop guard (#53).
> The post-pin language-growth arcs (#92/#93/#95/#109/#112/#121/#123 corpus
> widening, #101 equivalent-mutant exclusion, #232 struct-inv weave, #237
> narrowing) extend the pipeline without contradicting the REQs above.

The stages, in order, are:

```
parse  →  validate  →  effect-check  →  lower  →  run verus  →  parse output  →  certificate
(syntax)  (spec)        (lower)         (lower)   (subprocess)   (this crate)     (manifest)
```

## Requirements

- REQ-1 (pipeline orchestration): `pub fn check_file` runs the full v0.1
  pipeline for one source file in this fixed order — `thermite_syntax::parse`
  → `thermite_spec::validate` → `thermite_lower::check_effects` →
  `thermite_lower::lower` → run verus → parse verus output → assemble
  `Certificate`. Each stage's failure short-circuits into a `ForgeError`
  variant (`.design/forge/cli.md` REQ-3) so the cert/diagnostic reflects the
  EARLIEST failing stage. The order is the kernel's data dependency: you cannot
  validate an unparsed AST, lower an effect-illegal program, or run verus on
  un-lowered source.
  Source: `goal.md` scope ("the FULL v0.1 pipeline end-to-end"); the driven
  APIs `pub fn parse in parser.rs`, `pub fn validate in validator.rs`,
  `pub fn check_effects in effects.rs`, `pub fn lower in lower.rs`.
- REQ-2 (verus invocation — real binary, temp file, crate-name gotcha): the
  lowered Verus source is written to a temporary file whose stem is a VALID
  Rust crate name (no `.` characters), then `verus` is spawned on it. The
  emitted-source filename must NOT contain a `.` before the extension — verus
  derives the crate name from the file stem and rejects a dot. The temp file is
  created under a system temp dir (determinism is in the INPUT, not the path)
  and cleaned up after. The pinned solver seed (§5.3, from the project
  lockfile) is passed to verus.
  Source: `goal.md` ("emitted `.verus.rs` filenames with a `.` break verus's
  crate-name derivation — write to a valid-crate-name temp path"). GROUNDED:
  running `verus /tmp/sum.verus.rs` yields
  `error: invalid character '.' in crate name: sum.verus`; renaming to
  `/tmp/sum_check.rs` yields `verification results:: 5 verified, 0 errors`.
- REQ-3 (exit-status checked; never swallow): verus's process exit status is
  always inspected. A non-zero status with a parseable failure summary is a
  reported verification FAILURE (a valid certificate describing it). A non-zero
  status with UNparseable output, a spawn failure (verus absent), or a
  vir/internal error is a structured `ForgeError` (`VerusOutput` / `VerusSpawn`
  / `VerusAbsent`) — never silently treated as success and never swallowed.
  Source: `goal.md` R-CODE-4. GROUNDED: success → exit 0
  (`verification results:: 5 verified, 0 errors`); a failing obligation → exit
  1 (`verification results:: 4 verified, 1 errors`); verus absent → ENOENT on
  spawn.
- REQ-4 (verus output → per-obligation results + counterexamples): verus's
  output is parsed into structured per-obligation results. The MACHINE-readable
  summary is taken from `verus --output-json`'s `verification-results` object
  (`{success, verified, errors, encountered-error, encountered-vir-error}`);
  the per-obligation diagnostics (which obligation failed, its source location,
  the failure kind) are taken from verus's stderr `error:` lines plus their
  `--> file:line:col` source spans. Each failing obligation becomes a structured
  result carrying the obligation description and the concrete failure witness —
  "counterexamples, not adjectives" (§5.1): the result records the failed
  obligation and its source position, NOT a bare "verification failed" string.
  Source: `thermite-design.md` §5.1. GROUNDED: a broken postcondition yields
  `error: invariant not satisfied at end of loop body` with a
  `--> sum_broken.rs:37:13` span pointing at the exact `invariant` line, plus
  the JSON `{success:false, verified:4, errors:1}`.
- REQ-5 (level determination — v0.1): the assurance level is L3 if and only if
  verus reports 0 errors (`verification-results.success == true`, `errors == 0`)
  — "certified L3" means an SMT proof discharged every obligation
  (`thermite-design.md` §6: L3 = SMT proof, contract holds for all inputs). If
  verus reports obligation failures, #5 REPORTS the per-obligation failures
  (the certificate level is NOT L3 and the run is a reported failure). The full
  automatic degrade ladder L3→L2→L1 with budgets and a solver portfolio is
  EXPLICITLY OUT of #5 (issue #10; L2/Kani is #9); v0.1's level logic is binary:
  L3 on a clean proof, reported failure otherwise. A verus timeout in v0.1 is a
  reported non-L3 outcome (true budget-driven degrade is #10).
  Source: `thermite-design.md` §6; `goal.md` R-CODE-4 ("a verus timeout
  DEGRADES ... but full degrade is #10, so for #5 document the v0.1 behavior:
  L3 on 0 errors, report obligation failures otherwise").
- REQ-6 (verus-absent = environment error): if the `verus` binary is not found
  on `PATH` (spawn ENOENT), `check_file` returns `ForgeError::VerusAbsent` — an
  ENVIRONMENT error, distinct from a verification failure. It must NOT be
  reported as L3 and must NOT be silently downgraded.
  Source: `goal.md` R-CODE-4 ("verus-absent is an environment error").
  GROUNDED: with `verus` off `PATH`, the spawn fails.
- REQ-7 (determinism): the pipeline is bit-reproducible given the same
  toolchain version and pinned solver seed (§5.3). No wall-clock, no
  un-seeded randomness in the certificate's deterministic fields. The
  non-deterministic `solver_time_ms` (§5.3, `conformance/README.md`) is NOT
  part of the oracle-compared subset and is excluded from any determinism
  assertion. Stages run in fixed source order (`pub fn lower` already emits
  items "in source order" per `lower.rs`).
  Source: `thermite-design.md` §5.3; `goal.md` R-CODE-5;
  `conformance/README.md` (deterministic subset; `solver_time_ms` excluded).

- REQ-8 (`fx diverge` caps at L1 — partial correctness, mutation/strengthen
  exempt; the #16 boundary precedent): a `fn` whose effect row contains
  `diverge` (`thermite_syntax::ast::Effect::Diverge`, §4.1 "divergence requires
  `fx diverge` in the row") is NOT total — it may not terminate (an event loop,
  `examples/editor/editor.th`'s `run`). L3 means "the contract holds for ALL
  inputs" = TOTAL correctness (`thermite-design.md` §6), which a non-terminating
  fn cannot honestly claim. So `gate_fn` routes a diverge fn to an L1 cap
  (partial correctness) and the §7 mutation-kill + strengthening gate is SKIPPED
  for it — EXACTLY mirroring the #16 `#[boundary]` short-circuit
  (`f.boundary.is_some()` → `Certificate::boundary_l1`, `Level::L1`, no verus,
  no mutation, no strengthen). The cap is HONEST, not a bypass (R-DEFER-9, §7
  anti-Goodhart): the mutation gate validates a STRONG-functional `ens`, which
  is the wrong tool for a partial-correctness event loop whose `ens` is
  inherently a weak shape (`run`'s `ens result <= 256` — a `return 0` mutant
  survives, kill ratio sub-floor → a `WeakContract` L0 reject, the WRONG verdict
  for a divergent loop). Capping at L1 does NOT over-claim (it is below L3, not
  certified-total), and the §87 Verus loop-INVARIANT proof (partial correctness,
  termination exempt — already wired, `fn_is_diverge` + the
  `#[verifier::exec_allows_no_decreases_clause]` attribute in `lower_fn` in
  `lower.rs`) remains REAL assurance run alongside the L1 cap. The exemption is
  diverge-ONLY: a non-diverge fn STILL proves termination (its loop `dec`) AND
  STILL passes the §7 mutation gate to reach L3 — the diverge cap is NOT a
  termination escape hatch (the #87 termination exemption is itself diverge-only,
  `lower.rs`) and NOT a mutation escape hatch for a normal weak contract. The
  level semantics of this cap are owned by `.design/forge/degrade-ladder.md`
  (the L1/partial-correctness rung); `check.rs`'s job is the `gate_fn` routing.
  Source: `thermite-design.md` §4.1 (termination by default; `fx diverge` is the
  exemption), §6 (L3 = total; L1 = runtime contract checks), §7 (the mutation
  gate validates a strong `ens`); `goal.md` R-DEFER-9 (no laundering a weak
  contract to a high level — the cap is honest, NOT a bypass); the #16 boundary
  precedent (`gate_fn`'s `f.boundary.is_some()` → `Certificate::boundary_l1`);
  open blocker #88. GROUNDED: `run`'s loose `ens result <= 256` is satisfied by
  a `return 0` body, so the mutation battery kills a minority of mutants and
  reports `WeakContract` at L0 — which is the wrong verdict the L1 cap corrects.

## Acceptance criteria

- AC-1 (LIVE cert-oracle, sum → L3): `forge check conformance/sum.th` emits a
  certificate whose DETERMINISTIC, currently-producible fields equal the
  present fields of `conformance/sum.cert.json` — `item == "sum"`,
  `level == "L3"`, `effects == ["pure"]`, `slag == false` — and the
  per-obligation results show all obligations discharged. Forward-declared
  battery fields (`contract_quality.{tautology,vacuous_precondition,
  mutants_killed,survivor}`) and the non-deterministic `solver_time_ms` are
  excluded from the comparison per `conformance/README.md`. GROUNDED: the
  lowered `sum` verifies `5 verified, 0 errors`, exit 0.
- AC-2 (binary_search → L3): `forge check conformance/binary_search.th` produces
  `level == "L3"` (verus 0 errors). GROUNDED: the lowered `binary_search`
  verifies `2 verified, 0 errors`, exit 0. (No golden cert is asserted for
  `binary_search` yet per `conformance/README.md`; this AC asserts the LEVEL
  only.)
- AC-3 (broken contract → reported failure + counterexample): a fixture whose
  contract does not hold yields a certificate that is NOT L3, carries a
  per-obligation FAILURE result naming the failed obligation and its source
  location (not "verification failed"), and the run exits with the
  verification-failure code (`.design/forge/cli.md` REQ-5). GROUNDED: the
  broken-postcondition fixture yields `error: invariant not satisfied at end of
  loop body` at `--> :37:13`, JSON `{success:false, errors:1}`, exit 1.
- AC-4 (crate-name gotcha): the temp file `forge` writes for verus has a stem
  with no `.`; a unit test asserts the chosen temp path stem is a valid Rust
  crate name. (Regression guard for the grounded
  `invalid character '.' in crate name` failure.)
- AC-5 (verus-absent): `forge check conformance/sum.th` with `verus` removed
  from the test's `PATH` returns `ForgeError::VerusAbsent` (environment error),
  not an L3 cert and not exit 0.
- AC-6 (exit-status discipline): a unit asserts that a verus run with exit
  status ≠ 0 and a parseable failure summary becomes a reported failure cert
  (not an `Err`), while exit ≠ 0 with unparseable output becomes
  `ForgeError::VerusOutput` (R-CODE-4 — never swallowed, never success).

- AC-7 (`fx diverge` → L1 cap, mutation-exempt; the diverge-ONLY gate, #88):
  four mechanically checkable facts.
  (a) The editor's event loop `fn run() ... fx read(input), write(output),
  alloc, diverge { while quit == 0 inv ... dec 1 { ... } }`
  (`examples/editor/editor.th`) certifies `level == "L1"` (partial correctness)
  — NOT `Level::L0` `WeakContract` and NOT a forced L3-total claim — and its
  cert carries NO `strengthening` suggestions and NO mutation `survivor` reject
  (the §7 gate is skipped for it). Mechanically: `gate_fn(run)` routes to the
  diverge L1 cert before the L3/mutation/strengthen path.
  (b) A NORMAL weak-contract fn (e.g. the AC-1 strengthening fixture
  `fn f(a,b) -> u32 req a<=10 && b<=10 ens result <= 1000000 fx pure { a+b }`,
  NO `diverge`) STILL reaches the §7 mutation gate and STILL reports
  `WeakContract` at `Level::L0` at the default floor — the gate still bites a
  non-diverge weak contract (the diverge exemption is not a mutation escape
  hatch).
  (c) A NORMAL loop fn WITHOUT a `dec` (no `diverge`) STILL fails Verus
  termination — the #87 termination exemption (`fn_is_diverge` in `lower.rs`)
  is diverge-ONLY and the diverge L1 cap does not relax it for any other fn.
  (d) `conformance/sum.th` and `conformance/binary_search.th` (total, `dec`
  present, NO `diverge`) are UNCHANGED — they certify `Level::L3` exactly as
  before (the diverge gate never fires for them; the corpus oracle is
  unperturbed). GROUNDED: `run`'s loose `ens result <= 256` is met by `return
  0`, so a non-diverge run of the §7 battery yields a `WeakContract` L0 — the
  wrong verdict the L1 cap corrects.

## Architecture

`pub fn check_file(path) -> Result<Vec<Certificate>, ForgeError>` is the
boundary entry (called by `cli.rs`, `.design/forge/cli.md`; the seed is resolved
internally — Amendment item 1). It threads the shipped crates in dependency
order (stages 4-7 run PER ITEM over `item_subprogram` sub-programs):

1. **parse** — `thermite_syntax::parse(&src)` returns a `ParseResult`; if
   `!result.is_clean()` (per `pub fn is_clean in parser.rs`), the parse
   `Vec<SyntaxError>` becomes `ForgeError::Parse`.
2. **validate** — `thermite_spec::validate(&program)` (`pub fn validate in
   validator.rs`, `Result<(), Vec<SpecError>>`) enforces the SpecTherm cage;
   errors → `ForgeError::Spec`.
3. **effect-check** — `thermite_lower::check_effects(&program)` (`pub fn
   check_effects in effects.rs`, `Result<(), Vec<LowerError>>`) enforces `fx`
   subsumption (§4.1); errors → `ForgeError::Effects`.
4. **lower** — `thermite_lower::lower(&program)` (`pub fn lower in lower.rs`,
   `Result<String, LowerError>`) emits the Verus-annotated Rust source; error →
   `ForgeError::Lower`.
5. **run verus** — write the lowered source to a temp file with a
   valid-crate-name stem (REQ-2 — NO `.` in the stem), spawn `verus` with the
   pinned seed (`--smt-option smt.random_seed=<seed>`, §5.3) and
   `--output-json`, capture stdout (JSON) + stderr (diagnostics) + exit status.
6. **parse output** (this crate's core) — REQ-4. Read
   `verification-results` from the JSON for the
   `{success, verified, errors}` summary; read stderr `error:` lines + their
   `--> file:line:col` spans for per-obligation failure detail and witnesses.
7. **certificate** — assemble the `Certificate` (`manifest.rs`): `item` (the
   checked item name), `level` (REQ-5: L3 iff 0 errors), `effects` (from the
   item's `fx` row), `slag` (false in #5 — `#[slag]` handling is #6/§8),
   per-obligation results, and the forward-declared/reserved fields
   (`.design/forge/certificate-manifest.md`).

**Verus invocation reality (grounded).** verus offers two complementary output
channels and `check.rs` uses BOTH:

- `verus --output-json` → a JSON document with a `verification-results` object:
  on success `{"success": true, "verified": 5, "errors": 0,
  "encountered-error": false, "encountered-vir-error": false}`; on a failing
  obligation `{"success": false, "verified": 4, "errors": 1,
  "encountered-error": true}`. This is the machine-readable summary that drives
  level determination (REQ-5).
- stderr human-readable diagnostics → for each failed obligation, an
  `error: <obligation description>` line followed by a `--> <file>:<line>:<col>`
  source span pointing at the exact failing clause, e.g.
  `error: invariant not satisfied at end of loop body` →
  `--> sum_broken.rs:37:13`. This is the "counterexample, not adjective"
  payload (§5.1): the per-obligation result records the obligation text and its
  position, not a bare boolean.

The summary line `verification results:: <N> verified, <M> errors` also appears
on stderr in non-JSON mode and is the human fallback. `check.rs` prefers the
JSON summary (REQ-4) and uses the stderr spans for per-obligation detail.

**The cert-oracle match (AC-1).** The deterministic subset of `sum.cert.json`
that #5 can produce is `{item, level, effects, slag}` plus the per-obligation
results. `level == "L3"` is justified by the grounded `5 verified, 0 errors`.
The battery fields (`contract_quality.*`) and `solver_time_ms` are
forward-declared / non-deterministic and excluded per `conformance/README.md`
— the toolchain "grows into" the golden cert as #6/#12 land.

**Scope boundaries (OUT of #5, documented).** The full L3→L2→L1 degrade ladder
with budgets and the Z3+cvc5 portfolio (issue #10), L2/Kani bounded checking
(#9), `#[slag]` L0 handling (#6/§8), the vacuity/mutation battery fields (#6,
#12, #13), the proof cache (#8), and the incremental goal-state REPL
(`forge goal`/`fill`, issue #21) are NOT part of this component. v0.1's level
logic is binary (L3 on clean proof; reported failure otherwise) and `slag` is
always `false`.

**The `fx diverge` L1 gate (REQ-8, AC-7 — the #16 mirror).** `gate_fn` in
`check.rs` already short-circuits a `#[boundary]` fn to L1 FIRST
(`f.boundary.is_some()` → `Certificate::boundary_l1`, `Level::L1`, no verus, no
§7 mutation/strengthen). REQ-8 adds the diverge analog: a `fn_is_diverge(f)`
check (the same row-shape predicate `fn_is_diverge in lower.rs` already uses —
`matches!(f.contract.fx, EffectRow::Set(es) if es.contains(&Effect::Diverge))`,
the SINGLE source of truth for the §4.1 termination exemption) routes a diverge
fn to an L1 cap that, like `boundary_l1`/`slag_l1`, records `Level::L1` and
SKIPS the L3 (verus-total) / #12 mutation / #14 strengthen path. The certificate
shape mirrors `Certificate::boundary_l1`: `Level::L1`, `graduate_triage_clean()`
(the §7.1 (a)/(b)/(c) triage STILL applies — a diverge fn with a vacuous `ens`
is still rejected; divergence exempts proving TOTAL correctness, not STATING a
non-vacuous contract), `slag: false`, `boundary: false`, with a discharged
obligation noting the partial-correctness verdict (e.g. *"contract holds at L1
(diverge / partial correctness); termination not claimed (§4.1 `fx diverge`)"*).
The per-item loop consumes it via a `GateOutcome` arm that `continue`s exactly
like `GateOutcome::BoundaryL1`/`SlagL1` — so a diverge fn NEVER reaches the
mutation gate that mis-rejected it.

**Run Verus for the partial invariants, but CAP the level at L1 (the decision,
ratify in #88).** Two readings: (a) the diverge L1 cap RUNS Verus on the lowered
diverge fn to verify its loop INVARIANTS (partial correctness — already working
post-#87 via `#[verifier::exec_allows_no_decreases_clause]` in `lower_fn`), then
reports `Level::L1` (a genuine bonus: the invariant proof IS real assurance);
(b) the diverge fn SKIPS Verus entirely like a `#[boundary]` fn and records L1
by fiat. RECOMMEND (a): the §87 invariant proof already verifies and is real
partial-correctness assurance — discarding it would under-claim. The level is
CAPPED at L1 regardless (no L3-total over-claim) and the §7 mutation/strengthen
gate is skipped in BOTH readings. If carrying the partial proof complicates the
cert (e.g. the L1 cert must reconcile a `VerusOutcome::Proved` with a
`Level::L1` verdict), the simpler boundary-style L1-no-verus (b) is an
acceptable fallback — pin whichever is cleaner in #88. Either way the LEVEL is
L1 and the mutation gate is exempt; the only open choice is whether the
invariant proof rides along.

**Why this is honest, not a Goodhart loophole (R-DEFER-9).** The §7 mutation
gate exists so a WEAK contract cannot be GAMED up to a high level. A diverge fn
is not gaming: it is HONESTLY partial — an event loop's `ens` is a weak shape
because the loop never returns a strong functional result, and that is the TRUTH
about an event loop, not a dodge. Capping it at L1 (strictly BELOW L3-total) is
the honest verdict — it claims LESS than L3, never more than it proves. The
mutation gate is simply the wrong instrument for a partial-correctness contract
(it validates a strong-functional `ens`); the right assurance is the L1 cap PLUS
the runtime invariant checks PLUS (recommendation (a)) the §87 invariant proof.
Contrast the laundering R-DEFER-9 forbids: weakening a TOTAL fn's `ens` to dodge
the mutation floor and still claiming L3 would be a bypass — and AC-7(b) proves
that path is unchanged (a non-diverge weak contract still rejects at L0). The
exemption is keyed strictly on the `fx diverge` DECLARATION (which the agent
must write loudly, §4.1), so it cannot be silently applied to a normal fn.

## Verification

- `cargo test -p forge` — pipeline unit tests: stage ordering / short-circuit
  (REQ-1); verus output parsing on captured success and failure fixtures
  (REQ-4, AC-6); the crate-name-stem guard (AC-4); verus-absent → `VerusAbsent`
  (AC-5).
- Conformance integration (`goal.md` verification model (B); the
  `conformance` route reference): `forge check conformance/sum.th` cert's
  deterministic present fields == `conformance/sum.cert.json` (AC-1);
  `forge check conformance/binary_search.th` → `level == "L3"` (AC-2); a
  committed broken-contract fixture → reported failure + counterexample (AC-3).
  Expected values trace to `conformance/sum.cert.json` / `thermite-design.md`,
  NEVER copied from `forge`'s own output (R-CHAR-3).
- `cargo clippy -p forge --all-targets -- -D warnings`, `cargo fmt --check`,
  anti-pattern gate.

These conformance checks are the `goal.md` R-DEFER-6 gate that runs whenever a
commit touches `forge`.

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (pipeline orchestration) | SHIPPED | `pub fn check_file` (→ `check_file_with_options`) runs `parse`→`validate`→`check_effects`, then PER ITEM `item_subprogram`→`thermite_lower::lower`→`run_verus`→`classify_verus_outcome`→`assemble_certificate`, each short-circuiting into a `ForgeError`; returns `Vec<Certificate>`; consumer `cli::run_check`. Live oracle test `check_conformance::sum_cert_matches_golden_deterministic_subset`. |
| REQ-2 (verus invocation, temp file, crate-name gotcha) | SHIPPED | `fn crate_stem` strips `.`/leading-digit; `fn run_verus` writes `<stem>.rs` into the per-run scratch dir `forge_<stem>_<pid>_<n>/` (`unique_scratch_dir`) and spawns verus (`invoke_verus`: `--output-json --smt-option smt.random_seed=<seed>` + the #11 `--profile`/`--rlimit`); the `ScratchDir` Drop guard cleans up wholesale (#53). Test `crate_stem_has_no_dot_and_is_valid`. |
| REQ-3 (exit-status checked, never swallow) | SHIPPED | `fn invoke_verus` captures status+stdout+stderr; `fn classify_verus_outcome` makes a parseable failure a reported cert, unparseable/VIR-error a `ForgeError::VerusOutput`, spawn ENOENT a `VerusAbsent`. Tests `unparseable_output_is_verus_output_error`, `vir_error_is_verus_output_error`. |
| REQ-4 (verus output → per-obligation + counterexamples) | SHIPPED | `fn parse_summary` reads JSON `verification-results`; `fn parse_stderr_failures`/`fn parse_span` turn `error:` + `--> file:line:col` into `ObligationResult::failed` witnesses. Test `parseable_failure_is_reported_cert_with_counterexample`. |
| REQ-5 (level determination) | SHIPPED | `fn classify_verus_outcome`: `VerusOutcome::Proved` (`success && errors==0`) ⇒ `Level::L3` in `assemble_certificate`; a counterexample/timeout is a reported non-L3 cert (the former `level_from_summary` cite is dead — Amendment item 2; the #10 ladder may then degrade a TIMEOUT, never a counterexample). Tests `parseable_success_is_l3_cert` (L3) + `parseable_failure_is_reported_cert_with_counterexample` (L0). |
| REQ-6 (verus-absent = environment error) | SHIPPED | `fn invoke_verus` maps spawn `ErrorKind::NotFound` → `ForgeError::VerusAbsent`; integration test `cache_conformance::cold_cache_with_verus_unavailable_is_environment_error` (verus off `PATH`, cold cache → environment exit, never L3 — Amendment item 5). |
| REQ-7 (determinism) | SHIPPED | `DEFAULT_SOLVER_SEED` (via `fn resolve_seed`) passed to verus; `solver_time_ms` is the only wall-clock field, excluded from `Certificate::oracle_subset`; test `serialization_is_deterministic`. |
| REQ-8 (`fx diverge` caps at L1, mutation/strengthen exempt — the #16 mirror) | SHIPPED | `fn_is_diverge in check.rs` (the row-shape predicate mirroring `lower.rs`'s) routes a non-boundary, non-slag `fx diverge` fn in `gate_fn` to `GateOutcome::DivergeL1(diverge_l1_cert(..))` — `Level::L1`, `slag/boundary: false`, the partial-correctness discharged obligation, the §7.1 (a)/(b)/(c) triage STILL applied. The per-item loop's `GateOutcome::DivergeL1` arm `continue`s exactly like `BoundaryL1`/`SlagL1` (no verus, no #12 mutation, no #14 strengthen), so `run` never reaches the §7 gate that mis-rejected it `WeakContract` at L0. Reading (b) (boundary-style L1-no-verus) is the chosen ratification: the per-item sub-program's diverge body fails verus (the loop callees' `req`s are not re-established by the loop invariant — spurious for partial correctness), so the cap SKIPS verus; the real assurance is the L1 runtime checks + the L3-proven edit core (`insert_str`/`backspace`). DIVERGE-ONLY (R-DEFER-9): a non-diverge weak contract still rejects L0 `WeakContract`; a non-diverge non-decreasing `dec` still fails verus termination. Verified: `forge/tests/editor_runs.rs` (`editor_logic_certifies_l3_boundary_and_run_l1`, `non_diverge_weak_contract_still_rejects_l0_weakcontract`, `normal_loop_without_dec_still_fails_termination`, `corpus_still_certifies_l3_unperturbed`). |
