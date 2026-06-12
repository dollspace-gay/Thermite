# Forge CLI surface

<!--
tier: 3-component
status: shipped
audited-sha: dff9ae866e3437af272a62e078993e66c1116460 (re-audited 2026-06-12: amended — full re-author from greenfield-era text to the shipped 13-verb surface, #262)
governs: forge/src/cli.rs
thesis-refs:
  - thermite-design.md §5
  - thermite-design.md §5.1
  - thermite-design.md Appendix B
-->

## Summary

`forge/src/cli.rs` (2,778 lines) is the command surface of the `forge` driver:
a hand-rolled argv matcher (`fn parse_args in cli.rs`) that dispatches THIRTEEN
verbs — `new` / `check` / `audit` / `repair` / `review` / `build` / `tv` /
`exec-tv` / `body-tv` / `goal` / `battery` / `edit` / `fill` — renders each
verb's result as human-readable text or (under `--json`) the §5.1 structured
document, owns `enum ForgeError in cli.rs` (the boundary error that AGGREGATES
every driven library's error plus driver-native subprocess/environment
variants), and maps every outcome to a typed exit code
(`0` / `EXIT_VERIFICATION_FAILURE` = 1 / `EXIT_ENVIRONMENT` = 2). It is the
only module that touches `std::env::args` / `std::process::ExitCode`
(`pub fn run in cli.rs`, consumed by `fn main in main.rs`); all verb logic
lives in the driven modules (`check.rs`, `audit.rs`, `repair.rs`, `review.rs`,
`build.rs`, `contract_tv.rs`, `exec_tv.rs`, `body_tv.rs`, `goal_repl.rs`).

This amendment (#262) replaces the greenfield-era text wholesale: the previous
revision's Summary claimed "This component is GREENFIELD … Every REQ below is
NOT-STARTED, blocked on issue #5" while its own REQ-status table said SHIPPED —
internally split, and externally stale by 21 commits. The doc below is
re-grounded in the tree at the pinned SHA.

## The arc since the bootstrap pin (1004b7a1 → dff9ae86, 21 commits)

What the old doc never saw, grouped (each verb cites its issue in the code):

- **Battery + ladder era** — vacuity triage + `#[slag]` (#6), Kani-backed
  `--level l2` (#9), the automatic degrade ladder + assurance-manifest display
  (#10), solver profiles + `--rlimit` (#11), mutation scoring +
  `--mutation-floor` (#12).
- **Audit + trust surface** — `forge audit` manifest v1 (#15), FFI boundary
  certs (#16), end-to-end vs to-the-boundary scope (#17), `forge repair` (#18),
  `forge review` + `--reviewer <cmd>` (#19).
- **Build + sandbox** — `forge build` + `rustc` artifacts (#56), the seccomp
  runtime sandbox + `--no-sandbox`/`--sandbox-self-test` (#57), `--out`/`-o`
  (#128), `--target std|kernel` (#197).
- **Translation validation** — `forge tv` (#144), `forge exec-tv` (#154/#156),
  `forge body-tv` (#162).
- **Goal REPL** — `forge goal`/`battery` views + `edit` address-splice (#193
  (i)/(ii)), `?N` holes + `forge fill` (#193 (iii)).
- **Proof backends** — `--engine verus|lean|auto` + the disagreement
  `SoundnessAlarm` halt (#247); the usage-banner `--engine` currency fix
  (#257, commit `6368550a` — the banner had drifted behind the parser).

## Requirements

- REQ-1 (command surface): `forge` exposes exactly the thirteen verbs above;
  no args, an unknown verb, a missing positional, or an unknown flag is a
  structured `ForgeError::Usage` carrying `fn usage_text in cli.rs`, never a
  panic and never exit 0.
  Source: `thermite-design.md` Appendix B (the verb inventory); §5.1.
- REQ-2 (hand-rolled argv matcher): argv is parsed by `fn parse_args in cli.rs`
  — a `match` over the verb with per-verb flag loops — NOT a derive-macro
  dependency (`forge/Cargo.toml` declares no `clap`). The original two-verb
  justification (§2.2/§2.3/§4.4 low-magic posture) was made when the grammar
  was `new <name> | check <file> [--json]`; the decision SURVIVED to 13 verbs
  and ~650 lines of matcher. Honest assessment: it still holds — every flag
  error is a precise structured diagnostic and the dependency cost stays zero —
  but the per-verb flag loops are hand-duplicated (see OQ-1).
  Source: `thermite-design.md` §2.2/§2.3/§4.4.
- REQ-3 (`ForgeError` aggregation): `enum ForgeError in cli.rs` is the boundary
  aggregation point. It wraps each driven crate's error
  (`Parse(Vec<SyntaxError>)` / `Spec(Vec<SpecError>)` /
  `Effects(Vec<LowerError>)` / `Lower(LowerError)`) and carries driver-native
  families for each subprocess the verbs spawn — verus
  (`VerusAbsent`/`VerusSpawn`/`VerusOutput`), kani
  (`KaniAbsent`/`KaniSpawn`/`KaniOutput`), rustc
  (`RustcAbsent`/`RustcSpawn`/`RustcOutput`), the external reviewer
  (`ReviewerAbsent`/`ReviewerSpawn`/`ReviewerFailed`/`ReviewerOutput`) — plus
  `Io`, `Usage`, and `SoundnessAlarm(crate::engine::Disagreement)` (#247: two
  engines returning Proven ⊕ witnessed-Refuted on the SAME obligation is a
  HARD HALT, never resolved by preference). `Display` forwards every inner
  diagnostic — no information lost at the boundary.
  Source: `goal.md` workspace.md REQ-3; R-CODE-4;
  `.design/verified/proof-backends.md` REQ-5.
- REQ-4 (human + `--json` dual rendering): every reporting verb takes `--json`
  and emits exactly one machine document on stdout under it (certs array for
  `check`, `AuditManifest` for `audit`, hand-built reports for
  `repair`/`tv`/`exec-tv`/`body-tv`, `ReviewArtifact` for `review`,
  `BuildManifest` for `build`); without it, the `render_*` family
  (`fn render_human` / `render_assurance` / `render_audit` / `render_repair` /
  `render_review` / `render_build in cli.rs`) emits readable text. Diagnostics
  go to stderr so `--json` stdout stays a clean parseable document. Exceptions
  by design: `goal`/`battery`/`edit`/`fill` are human-only REPL views (OQ-2).
  Source: `thermite-design.md` §5.1.
- REQ-5 (typed exit codes): three classes. Verification gate verbs
  (`check`/`audit`: project headline `ProjectAssurance::Certified` via
  `AssuranceManifest::aggregate`; `repair`: `report.all_upgraded()`) → 0 on
  certified, `EXIT_VERIFICATION_FAILURE` on a reported failure (the cert/report
  is still a valid document). TV verbs (`tv`/`exec-tv`/`body-tv`) → 0 on a
  clean audit, `EXIT_VERIFICATION_FAILURE` on ANY divergent clause/expr/body (a
  real lowering-fidelity finding). Every `ForgeError` →
  `fn exit_code in cli.rs` = `EXIT_ENVIRONMENT` (a failed proof and a missing
  solver are never the same outcome). View/build verbs
  (`review`/`build`/`goal`/`battery`/`edit`/`fill`) exit 0 on a successful
  render — the verdict lives IN the rendered document.
  Source: `goal.md` R-CODE-4; `thermite-design.md` §5.2.
- REQ-6 (no panics; Result discipline): every fallible path returns
  `Result<_, ForgeError>`; no `unwrap`/`expect`/`panic!` outside `#[cfg(test)]`
  in `cli.rs`; subprocess statuses are inspected in the driven modules, never
  swallowed.
  Source: `goal.md` R-CODE-2, R-CODE-4, R-APG-1.
- REQ-7 (`forge new` scaffold): `pub fn scaffold_project in cli.rs` writes
  `forge.toml` + `forge.lock` (pinned solver seed, §5.3) +
  `THERMITE.skill.pin`, and refuses a non-empty target with a structured
  `ForgeError::Usage`, never a clobber.
  Source: `thermite-design.md` Appendix B, §5.3.
- REQ-8 (`forge check` flag surface + engine routing): `--level l2|l3`
  (explicit rung, never auto-degrade), `--rlimit <FLOAT>` (finite-positive
  validated), `--mutation-floor <FLOAT>` ([0,1] validated), `--engine
  verus|lean|auto` (#247). `fn run_check in cli.rs` routes: the canonical
  default config → `check::check_file` (the only cache-serving entry); explicit
  `--rlimit`/`--mutation-floor` → `check::check_file_with_options`
  (cache-bypassed); `--engine lean|auto` → `check::check_file_with_engine`
  (the proof-backends OQ-1 surface — exportable items discharged by Lean with
  attribution; a Verus⊕Lean disagreement surfaces as
  `ForgeError::SoundnessAlarm`, the REQ-5 halt); `--level l2` →
  `check::check_l2_file`. Every flag's missing/garbage value is a Usage error,
  never a silent default.
  Source: `.design/lower/l2-kani.md` REQ-7; `.design/forge/solver-profiles.md`
  REQ-5; `.design/forge/mutation-scoring.md` REQ-5;
  `.design/verified/proof-backends.md` OQ-1/REQ-5.
- REQ-9 (usage-banner currency): `fn usage_text in cli.rs` names every verb and
  every flag the parser accepts. The #257 fix (commit `6368550a`) closed the
  one known drift — the banner had been missing `[--engine verus|lean|auto]`
  after #247 landed the flag.
  Source: `thermite-design.md` §5.1 ("every message is a prompt" — the banner
  is the agent-facing grammar).
- REQ-10 (project assurance display, #10): without `--json`, `run_check`
  renders `fn render_assurance in cli.rs` after the per-cert text — the per-fn
  `lowered-assurance` flags plus the project headline (min over functions, or
  `FAILED`), which is also exactly what drives the REQ-5 exit code (both via
  `manifest::cert_certifies`).
  Source: `thermite-design.md` §5.2 ("displayed on every build");
  `.design/forge/degrade-ladder.md` REQ-5/REQ-6.

## Acceptance criteria

- AC-1: no args, an unknown verb, a missing positional, or an unknown flag →
  usage diagnostic + `EXIT_ENVIRONMENT`, never a panic, never 0 (unit
  `usage_errors`; integration `missing_file_is_usage_error_nonzero` in
  `forge/tests/check_conformance.rs`).
- AC-2: `forge check <corpus>.th --json` writes exactly one JSON document to
  stdout (the integration harness `run_check_json` parses stdout whole;
  `sum_cert_matches_golden_deterministic_subset`).
- AC-3: a broken contract is a REPORTED failure — valid cert on stdout, exit
  `EXIT_VERIFICATION_FAILURE`, counterexample present
  (`broken_contract_is_reported_failure_with_counterexample`).
- AC-4: every flag value is validated (missing / non-numeric / out-of-range /
  unknown enum → Usage error): unit tests `parses_rlimit_flag`,
  `parses_mutation_floor_flag`, `parses_level_flag`, `parses_build_out_flag`,
  `parses_build_target_flag`, `parses_build_sandbox_flags`.
- AC-5: `ForgeError` wrapping round-trips inner diagnostics
  (`aggregation_preserves_inner_diagnostics`) and every variant maps to
  `EXIT_ENVIRONMENT` (`errors_map_to_environment_exit_code`).
- AC-6: `--engine verus` is byte-identical to the default path
  (`engine_verus_flag_is_byte_identical_oracle` in
  `forge/tests/engine_attribution.rs`); a Proven ⊕ witnessed-Refuted
  disagreement halts (`proven_refuted_disagreement_halts` in `engine.rs`
  tests), surfaced as `ForgeError::SoundnessAlarm`.
- AC-7: `forge new` writes the three-file skeleton and refuses a non-empty
  target (`scaffold_writes_layout_and_refuses_clobber`).
- AC-8: `cargo clippy -p forge --all-targets -- -D warnings` + the
  anti-pattern gate are clean of non-test `unwrap`/`expect`/`panic!`.

## Architecture

The flow is `fn main in main.rs` → `pub fn run in cli.rs` (the ONLY reader of
`std::env::args` / producer of `ExitCode`) → `fn parse_args` →
`enum Command in cli.rs` → `fn dispatch` (split from `run` so it is
unit-testable without real argv) → one `run_<verb>` function per verb.

The verbs fall into four families, each with its own exit-code convention
(REQ-5):

1. **Certification gates** — `check` (`run_check`: the four-way
   level/engine route of REQ-8, then `AssuranceManifest::aggregate` for the
   headline + exit), `audit` (`run_audit`: the SAME default pipeline projected
   into `AuditManifest`, exit mirrors the headline), `repair` (`run_repair`:
   exit 0 iff `all_upgraded`).
2. **TV deeper audits** — `tv`/`exec-tv`/`body-tv`
   (`run_tv`/`run_exec_tv`/`run_body_tv`): opt-in, NOT folded into `forge
   check`; exit fails only on a DIVERGENT finding; Unverifiable/Skipped are
   surfaced but never fail the exit (R-HONEST-3 — and verus-absent is loud,
   not a silent pass). Their `--json` documents are hand-built in
   `fn tv_report_json` / `exec_tv_report_json` / `body_tv_report_json in
   cli.rs`.
3. **View/build verbs** — `review` (`run_review`: artifact emission +
   optional `--reviewer` pipe, writing `<file>.review.json` via
   `fn review_record_path`; forge never fabricates a verdict), `build`
   (`run_build` → `build::build_file`, rendering the `BuildManifest`), and the
   #193 REPL quartet `goal`/`battery`/`edit`/`fill`
   (`run_goal`/`run_battery`/`run_edit`/`run_fill` → `goal_repl::*`): a render
   is a successful query; the verdict lives in the rendered goal state.
4. **Scaffold** — `new` → `pub fn scaffold_project` (REQ-7).

`ForgeError` (REQ-3) is where the leaf-first DAG's many error channels
converge: the driven crates keep their own error types, `cli.rs` wraps them so
any failure maps to a diagnostic + `EXIT_ENVIRONMENT` without the libraries
knowing about each other. The variant families grew with the verbs (kani #9,
rustc #56, reviewer #19, `SoundnessAlarm` #247) but the shape is unchanged
from the #5 original: wrap, forward the diagnostic, never swallow. Note the
uniform `exit_code()` — every `ForgeError`, including `SoundnessAlarm`, is the
environment class (OQ-4).

The renderers live at the bottom of the file (`render_human` and friends);
`render_human` prints the oracle-stable subset first
(`Certificate::oracle_subset`) and labels `solver_time_ms` non-deterministic so
a reader cannot mistake it for an oracle field.

## Verification

- `cargo test -p forge` unit tests in `cli.rs::tests`: verb/flag parsing
  (AC-1/AC-4), error aggregation + exit-code mapping (AC-5), scaffold
  (AC-7), assurance rendering (`render_assurance_shows_headline_and_lowered_flags`,
  `render_assurance_shows_failed_headline`), human rendering
  (`human_render_shows_failure_counterexample`).
- Integration suites driving the built binary (`forge/tests/`):
  `check_conformance.rs` (AC-2/AC-3 + golden certs), `audit_conformance.rs`,
  `repair_conformance.rs`, `review_conformance.rs`, `build_conformance.rs`,
  `sandbox_conformance.rs`, `kernel_target.rs`, `contract_tv_conformance.rs`,
  `exec_tv_conformance.rs`, `body_tv.rs`, `goal_repl.rs`, `goal_repl_fill.rs`,
  `l2_check.rs`, `engine_attribution.rs`, `engine_interface.rs`,
  `lean_engine.rs` (AC-6).
- `cargo clippy -p forge --all-targets -- -D warnings` + `cargo fmt --check` +
  the anti-pattern gate (AC-8).

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (13-verb command surface) | SHIPPED | `fn parse_args in cli.rs` matches `new`/`check`/`audit`/`repair`/`review`/`build`/`tv`/`exec-tv`/`body-tv`/`goal`/`battery`/`edit`/`fill`; the fall-through arm is `Err(ForgeError::Usage(format!("unknown command \`{other}\`. {}", usage_text())))`. Non-test consumer: `fn main in main.rs` → `cli::run`. Verification: unit `usage_errors`; integration `missing_file_is_usage_error_nonzero`. |
| REQ-2 (hand-rolled argv matcher) | SHIPPED | `fn parse_args in cli.rs` (~650 lines, verb `match` + per-verb flag loops); `grep clap forge/Cargo.toml` → no hit. Consumer: `fn dispatch in cli.rs`. Verification: the six `parses_*` unit tests. Decision survived the 2-verb → 13-verb growth; duplication cost named in OQ-1. |
| REQ-3 (`ForgeError` aggregation) | SHIPPED | `enum ForgeError in cli.rs`: `Parse(Vec<SyntaxError>)`/`Spec`/`Effects`/`Lower` + the verus/kani/rustc/reviewer Absent-Spawn-Output families + `Io`/`Usage` + `SoundnessAlarm(crate::engine::Disagreement)`; `impl fmt::Display` forwards inner diagnostics. Non-test consumers: every driven module returns it (`check::check_file -> Result<_, ForgeError>`, `pub fn check_disagreement in engine.rs` surfaces the alarm). Verification: `aggregation_preserves_inner_diagnostics`. |
| REQ-4 (human + `--json` dual rendering) | SHIPPED | `fn run_check in cli.rs`: `serde_json::to_string_pretty(&certs)` under `--json`, else `render_human` per cert + `render_assurance`; parallel paths in `run_audit`/`run_repair`/`run_review`/`run_build`/`run_tv`/`run_exec_tv`/`run_body_tv`; stderr for diagnostics (e.g. `run_review`'s `eprintln!` keeps `--json` stdout clean). Verification: `run_check_json` harness parses stdout whole in `check_conformance.rs`. |
| REQ-5 (typed exit codes) | SHIPPED | `pub const EXIT_VERIFICATION_FAILURE: u8 = 1` / `EXIT_ENVIRONMENT: u8 = 2` in `cli.rs`; `run_check`/`run_audit` gate on `matches!(.., ProjectAssurance::Certified(_))`; `run_repair` on `report.all_upgraded()`; the TV trio on `counts.divergent == 0`; `fn exit_code in cli.rs` maps every `ForgeError` to `EXIT_ENVIRONMENT`. Verification: `errors_map_to_environment_exit_code`, `broken_contract_is_reported_failure_with_counterexample` (exit 1), `divergence_audit_check2_exit_swallow.rs` (the TV exit discipline). |
| REQ-6 (no panics; Result discipline) | SHIPPED | every `run_*`/`parse_args` path returns `Result<_, ForgeError>`; no `unwrap`/`expect`/`panic!` outside `#[cfg(test)]` in `cli.rs`. Verification: clippy `-D warnings` + the anti-pattern gate in the gauntlet (HEAD commit `93d3cbc0` chain is gauntlet-green). |
| REQ-7 (`forge new` scaffold) | SHIPPED | `pub fn scaffold_project in cli.rs` writes `forge.toml`/`forge.lock` (`seed = {DEFAULT_SOLVER_SEED}`)/`THERMITE.skill.pin`; non-empty target → `ForgeError::Usage("… refusing to overwrite")`. Non-test consumer: `fn dispatch` (`Command::New` arm). Verification: `scaffold_writes_layout_and_refuses_clobber`. |
| REQ-8 (`check` flags + engine routing) | SHIPPED | `Command::Check { file, json, level, rlimit, mutation_floor, engine }`; `fn run_check` four-way route: default → `check::check_file` (cache-canonical), explicit options → `check::check_file_with_options`, `(CheckLevel::L3, sel)` lean/auto → `check::check_file_with_engine` (REQ-5 disagreement → `ForgeError::SoundnessAlarm`), `(CheckLevel::L2, _)` → `check::check_l2_file`. Verification: `parses_rlimit_flag`/`parses_mutation_floor_flag`/`parses_level_flag`, `engine_flag_parsing` + `engine_verus_flag_is_byte_identical_oracle` (`engine_attribution.rs`), `proven_refuted_disagreement_halts` (`engine.rs`). |
| REQ-9 (usage-banner currency, #257) | SHIPPED | `fn usage_text in cli.rs` names all 13 verbs and the full flag surface including `[--engine verus|lean|auto]` — added by commit `6368550a` ("forge usage banner gains [--engine verus|lean|auto] (drift fix)"). Non-test consumer: `parse_args`'s no-verb and unknown-verb arms. Verification: `usage_errors` exercises both arms; `l2_check.rs` checks the banner rejects a bogus `--level`. |
| REQ-10 (project assurance display, #10) | SHIPPED | `fn render_assurance in cli.rs` prints per-fn `lowered-assurance:` lines + the `project assurance:` headline; `run_check` computes `AssuranceManifest::aggregate(&certs)` once for both the display and the exit gate. Verification: `render_assurance_shows_headline_and_lowered_flags`, `render_assurance_shows_failed_headline`. |

## Open questions

- **OQ-1 (matcher scale):** the hand-rolled matcher survived to 13 verbs, but
  the per-verb flag loops duplicate the
  `--json`/unknown-flag/extra-positional boilerplate ~10 times and the
  value-taking-flag pattern (`iter.next().ok_or_else(Usage)`) ~9 times. The
  no-`clap` decision still pays (zero deps, precise diagnostics, REQ-9's
  banner is the single grammar source) — but a shared flag-loop helper inside
  `cli.rs` would cut the duplication without a dependency. Revisit if a 14th
  verb lands.
- **OQ-2 (REPL views have no `--json`):** `goal`/`battery`/`edit`/`fill` emit
  human text only ("pure views — no flags"). An agent-facing machine form of
  the goal state is plausibly wanted by the #21 REPL arc; not designed here.
- **OQ-3 (Appendix B `forge skill` verb):** Appendix B names a `skill` verb;
  no such verb exists in `parse_args`. The skill artifact is generated by the
  `thermite-skill` crate instead (#7 arc). Whether a CLI verb should front it
  is unowned by this doc.
- **OQ-4 (`SoundnessAlarm` exit class):** the uniform `fn exit_code` maps
  `SoundnessAlarm` to `EXIT_ENVIRONMENT` (2) — the same class as a missing
  binary. A soundness alarm is arguably a third kind of outcome (worse than
  both); a distinct exit code would let automation hard-stop on it. Today the
  distinction is carried only by the `SOUNDNESS ALARM:` stderr prefix.
- **OQ-5 (TV exit-code overload):** the TV verbs reuse
  `EXIT_VERIFICATION_FAILURE` for a lowering-fidelity finding — a different
  claim than `forge check`'s obligation failure. The code comments call this
  out as a deliberate convention; if consumers ever need to distinguish them
  mechanically, the `--json` verdict is the discriminator.
