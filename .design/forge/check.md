# Forge check pipeline

<!--
tier: 3-component
status: draft
governs: forge/src/check.rs
thesis-refs:
  - thermite-design.md §5.1
  - thermite-design.md §5.3
  - thermite-design.md §6
  - thermite-design.md Appendix A
-->

## Summary

`forge/src/check.rs` is the v0.1 `forge check` pipeline: it runs a single
`.th` item end-to-end through every shipped kernel component, invokes the REAL
`verus` binary on the lowered source, parses verus's output into per-obligation
results (with counterexamples on failure), and assembles the structured
certificate (`manifest.rs`, `.design/forge/certificate-manifest.md`). It is the
FIRST LIVE cert-oracle: `forge check conformance/sum.th`'s deterministic
certificate fields must match the golden `conformance/sum.cert.json`.

GREENFIELD — no `check.rs` exists; the only artifact is the empty
`forge/src/main.rs` scaffold. All REQs NOT-STARTED, blocked on issue #5.

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

## Architecture

`pub fn check_file(path, seed) -> Result<Certificate, ForgeError>` is the
boundary entry (called by `cli.rs`, `.design/forge/cli.md`). It threads the
shipped crates in dependency order:

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
| REQ-1 (pipeline orchestration) | NOT-STARTED | open issue #5. No `check.rs`; `forge/src/main.rs` is the empty scaffold. The driven APIs exist (`parse`, `validate`, `check_effects`, `lower`) but no orchestrator calls them. |
| REQ-2 (verus invocation, temp file, crate-name gotcha) | NOT-STARTED | open issue #5. No verus subprocess code exists. Gotcha grounded: `verus <file>.verus.rs` → `invalid character '.' in crate name`. |
| REQ-3 (exit-status checked, never swallow) | NOT-STARTED | open issue #5. No subprocess invocation exists yet. |
| REQ-4 (verus output → per-obligation + counterexamples) | NOT-STARTED | open issue #5. No output parser exists. Output formats grounded (JSON `verification-results` + stderr `error:`/`-->` spans). |
| REQ-5 (level determination, v0.1) | NOT-STARTED | open issue #5. No level logic exists. L3-on-0-errors grounded; full degrade is issue #10. |
| REQ-6 (verus-absent = environment error) | NOT-STARTED | open issue #5. No `VerusAbsent` handling exists; `forge` has no error type yet (`main.rs`: "`ForgeError` ... land with issue #5"). |
| REQ-7 (determinism) | NOT-STARTED | open issue #5. No seed plumbing exists; `solver_time_ms` exclusion is fixed by `conformance/README.md`. |
