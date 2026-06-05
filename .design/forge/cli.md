# Forge CLI surface

<!--
tier: 3-component
status: draft
governs: forge/src/cli.rs
thesis-refs:
  - thermite-design.md §5
  - thermite-design.md §5.1
  - thermite-design.md Appendix B
-->

## Summary

`forge/src/cli.rs` is the command surface of the `forge` driver: it parses
`argv`, dispatches to `forge new <name>` (project scaffold) and
`forge check [<file>]` (the v0.1 ladder pipeline), renders results either as
human-readable text or — under `--json` — as the structured certificate
(`thermite-design.md` §5.1), and owns `ForgeError`, the boundary error type
that AGGREGATES the per-crate errors of the driven libraries. It is the only
entry point that touches `std::env::args` / `std::process::ExitCode`; the
pipeline logic lives in `check.rs` (`.design/forge/check.md`) and the schema in
`manifest.rs` (`.design/forge/certificate-manifest.md`).

This component is GREENFIELD. The only shipped artifact is the empty
`forge/src/main.rs` scaffold (it exits 0, has no command surface, no
`ForgeError`). Every REQ below is NOT-STARTED, blocked on issue #5.

## Requirements

- REQ-1 (command surface): `forge` exposes exactly the v0.1-kernel subset of
  the Appendix B surface that issue #5 owns — `forge new <name>` and
  `forge check [<file>]`. The other Appendix B verbs (`goal`, `fill`, `edit`,
  `battery`, `audit`, `skill`, `repair`) are NOT this component's concern in #5
  (`goal`/`fill`/`edit` are the deferred incremental REPL, issue #21;
  `battery`/`audit` are #6/#12/#13/#15; `skill` is #7; `repair` is #10/#18). An
  unknown verb is a structured usage error, never a panic.
  Source: `thermite-design.md` Appendix B; `goal.md` scope (v0.1 kernel =
  whole-item `forge check`).
- REQ-2 (argument parsing): argv is parsed by a minimal hand-rolled matcher
  (verb in `argv[1]`, then positional `<name>`/`<file>` and the single
  `--json` flag) — NOT a derive-macro dependency. Justification: pillar §2.2
  ("the whole language fits in a skill") and §2.3 ("one way to do everything,
  no config knobs") argue for the smallest possible surface; the v0.1 verb set
  is two commands with one flag, well below the threshold where `clap` earns
  its compile-time and dependency cost. The macro removal in §4.4 reinforces a
  low-magic posture. (See OQ-1 — this is the least-settled decision.)
  Source: `thermite-design.md` §2.2/§2.3/§4.4.
- REQ-3 (`ForgeError` aggregation): `forge` introduces its own `ForgeError`
  enum — the BOUNDARY aggregation point. Each driven crate keeps its own error
  per workspace.md REQ-3 (`thermite_syntax::SyntaxError`,
  `thermite_spec::SpecError`, `thermite_lower::LowerError`), and `ForgeError`
  carries variants that WRAP each (`ForgeError::Parse(Vec<SyntaxError>)`,
  `ForgeError::Spec(Vec<SpecError>)`, `ForgeError::Effects(Vec<LowerError>)`,
  `ForgeError::Lower(LowerError)`), plus driver-native variants for the verus
  subprocess and environment (`ForgeError::VerusAbsent`,
  `ForgeError::VerusSpawn`, `ForgeError::VerusOutput`, `ForgeError::Io`,
  `ForgeError::Usage`). `ForgeError` does NOT replace the per-crate errors; it
  composes them at the driver boundary.
  Source: `goal.md` workspace.md REQ-3 (per-crate error type); R-SPEC-3.
- REQ-4 (human-readable + `--json` output): without `--json`, `forge check`
  renders a readable rendering of the certificate (§5.1 "rendered to readable
  text") to stdout; with `--json`, it serializes the certificate JSON
  (`manifest.rs`, §5.1 "JSON with a stable schema") to stdout and nothing else.
  Diagnostics and progress go to stderr so `--json` stdout is a clean
  machine-parseable document. The certificate value is produced by `check.rs`;
  this component only chooses the rendering.
  Source: `thermite-design.md` §5.1.
- REQ-5 (exit codes): process exit is a typed mapping from the run outcome, not
  an ad-hoc integer — verification success (all obligations discharged, L3) →
  0; a reported verification FAILURE (obligations failed; the cert is still a
  valid document describing the failure) → a distinct non-zero code; an
  environment/usage/IO error (verus absent, bad argv, unreadable file) → a
  different non-zero code. A failed proof and a missing solver are not the same
  outcome and must be distinguishable by exit code.
  Source: `goal.md` R-CODE-4 (verus-absent is an environment error; obligation
  failure is reported, not crashed); `thermite-design.md` §5.2 (degrade ≠
  block — in #5 the v0.1 behavior is "report").
- REQ-6 (no panics; Result discipline): every fallible path returns
  `Result<_, ForgeError>`; `main`/`cli` contain no `unwrap`/`expect`/`panic!`
  outside `#[cfg(test)]`. The verus subprocess exit status is always inspected;
  a non-zero status or unparseable output is a structured `ForgeError`, never
  swallowed and never treated as success.
  Source: `goal.md` R-CODE-2, R-CODE-4, R-APG-1.
- REQ-7 (`forge new` scaffold — minimal): `forge new <name>` creates a project
  directory carrying a manifest, a lockfile (the pinned solver seed lives here,
  §5.3), and a skill pin (Appendix B: "manifest, lockfile, skill pin"). v0.1
  keeps this minimal — enough that `forge check` inside the project can read the
  pinned seed deterministically. The on-disk manifest/lockfile shape is the
  project-config schema; the per-item certificate schema is a separate concern
  (`manifest.rs`). It must refuse to overwrite an existing non-empty target
  (a structured error, not a clobber).
  Source: `thermite-design.md` Appendix B, §5.3.

## Acceptance criteria

- AC-1: `forge` with no args, an unknown verb, or `forge check` with a missing
  positional prints a usage diagnostic to stderr and exits with the
  usage/environment code (REQ-5), never a panic and never exit 0.
- AC-2: `forge check conformance/sum.th --json` writes exactly one JSON
  document to stdout (parseable by `serde_json` / `python3 -m json.tool`) and
  nothing else to stdout; without `--json` the same run writes human-readable
  text and no raw JSON to stdout. (The certificate's CONTENTS are asserted by
  `check.md` AC / the cert-oracle; this AC asserts only the rendering split and
  stream discipline.)
- AC-3: when verus is absent from `PATH`, `forge check conformance/sum.th`
  exits with the environment-error code and a `ForgeError::VerusAbsent`
  diagnostic naming the missing binary — it does NOT report L3 and does NOT
  exit 0. (Grounded: with `verus` off `PATH`, spawn fails with ENOENT.)
- AC-4: `cargo clippy -p forge --all-targets -- -D warnings` is clean of
  `unwrap`/`expect`/`panic!` in non-test code, and the anti-pattern gate
  (`tooling/anti-pattern-gate.py`, R-APG-1) passes on the patch.
- AC-5: every `ForgeError` variant either wraps a named per-crate error
  (`SyntaxError`/`SpecError`/`LowerError`) or is a driver-native verus/io/usage
  variant; a `cargo test -p forge` unit asserts each wrapping variant
  round-trips its inner error's diagnostic (no information lost at the boundary,
  R-CODE-4 "never swallow").

## Architecture

`cli.rs` is a thin dispatcher. `pub fn run` (the planned boundary entry) reads
`std::env::args`, matches the verb, and delegates:

- `new` → `pub fn scaffold_project` (REQ-7), which writes the project skeleton.
- `check` → `check::check_file` in `check.rs` (`.design/forge/check.md`), which
  returns a `Certificate` (the type owned by `manifest.rs`,
  `.design/forge/certificate-manifest.md`). `cli.rs` then renders it: under
  `--json`, `serde_json::to_string_pretty` of the `Certificate`; otherwise a
  text rendering. The rendering choice is the ONLY logic `cli.rs` adds on top of
  `check.rs`.

The arg matcher is hand-rolled (REQ-2). The v0.1 verb grammar is small enough
(`new <name>` | `check [<file>] [--json]`) that a `match` over `argv` is
clearer and lighter than a derive dependency, consistent with the
no-magic/one-way posture of `thermite-design.md` §2.3 and §4.4. `--json` is the
sole flag; it selects the §5.1 structured certificate as the stdout document.

`ForgeError` (REQ-3) is the workspace's first AGGREGATING error. The driven
crates each return their own error per the leaf-first DAG (`parse` →
`ParseResult` with `Vec<SyntaxError>`; `validate` → `Result<(), Vec<SpecError>>`
per `pub fn validate in validator.rs`; `check_effects` →
`Result<(), Vec<LowerError>>` per `pub fn check_effects in effects.rs`; `lower`
→ `Result<String, LowerError>` per `pub fn lower in lower.rs`). `forge`, sitting
at the top of the DAG, is where these many error channels converge into a single
`ForgeError` so the CLI can map any failure to a diagnostic + exit code (REQ-5)
without the libraries needing to know about each other. This is the boundary
aggregation point named in `goal.md` workspace.md REQ-3.

Exit-code mapping (REQ-5) distinguishes the three outcomes the design treats
differently: a discharged proof (0), a reported verification failure (the
certificate is a valid document; §5.2 "the gate degrades, it never blocks" —
in #5 the v0.1 realization is "report the failure," full degrade L3→L2→L1 is
issue #10), and an environment/usage error (R-CODE-4: verus absent is an
environment error, NOT a verification failure).

`forge new` (REQ-7) is intentionally minimal for v0.1: manifest + lockfile +
skill pin (Appendix B). The lockfile carries the pinned solver seed (§5.3) that
`check.rs` later feeds to verus, so determinism (R-CODE-5) is project-scoped.

## Verification

- `cargo test -p forge` — unit tests for the arg matcher (verb dispatch, the
  `--json` flag, usage errors), `ForgeError` wrapping/round-trip (AC-5), exit
  code mapping (AC-1/AC-5), and `forge new` scaffold layout + no-clobber
  (AC: REQ-7).
- A CLI integration test (`tests/cli.rs`) drives the built `forge` binary:
  `forge check conformance/sum.th --json` → single JSON doc on stdout (AC-2);
  `forge check` with no file → usage error + non-zero exit (AC-1); verus removed
  from a scoped `PATH` → `VerusAbsent` + environment exit code (AC-3).
- `cargo clippy -p forge --all-targets -- -D warnings` + `cargo fmt --check` +
  the anti-pattern gate (AC-4).

The end-to-end cert-oracle assertion (cert fields == `conformance/sum.cert.json`)
is owned by `.design/forge/check.md`; `cli.rs` is verified for the rendering and
error-mapping surface only.

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (command surface) | SHIPPED | `fn run` + `fn dispatch` in `cli.rs` match `new`/`check`; unknown verb → `ForgeError::Usage`; consumer `fn main` in `main.rs`. Other Appendix B verbs out of #5. |
| REQ-2 (hand-rolled arg parsing) | SHIPPED | `fn parse_args` in `cli.rs` is a `match` over verb + positionals + `--json`; `forge/Cargo.toml` declares no `clap`. |
| REQ-3 (`ForgeError` aggregation) | SHIPPED | `enum ForgeError` in `cli.rs` wraps `Vec<SyntaxError>`/`Vec<SpecError>`/`Vec<LowerError>`/`LowerError` + `VerusAbsent`/`VerusSpawn`/`VerusOutput`/`Io`/`Usage`; `Display` forwards inner diagnostics (test `aggregation_preserves_inner_diagnostics`). |
| REQ-4 (human + `--json` output) | SHIPPED | `fn render_human` + `serde_json::to_string_pretty` in `fn run_check`; diagnostics to stderr; integration test `sum_cert_matches_golden_deterministic_subset` parses the clean `--json` stdout. |
| REQ-5 (typed exit codes) | SHIPPED | `fn run_check` returns `ExitCode`: all-L3 → 0, reported failure → `EXIT_VERIFICATION_FAILURE`(1); `ForgeError::exit_code` → `EXIT_ENVIRONMENT`(2). Tests `broken_contract_is_reported_failure_with_counterexample` (exit 1) + `missing_file_is_usage_error_nonzero`. |
| REQ-6 (no panics; Result discipline) | SHIPPED | every fallible `cli.rs` path returns `Result<_, ForgeError>`; no `unwrap`/`expect`/`panic!` in non-test code (anti-pattern gate + clippy `-D warnings` pass); verus exit status inspected in `check::invoke_verus`. |
| REQ-7 (`forge new` scaffold) | SHIPPED | `pub fn scaffold_project` in `cli.rs` writes `forge.toml`+`forge.lock`(pinned seed)+`THERMITE.skill.pin`, refuses non-empty target; consumer `fn dispatch`; test `scaffold_writes_layout_and_refuses_clobber`. |
