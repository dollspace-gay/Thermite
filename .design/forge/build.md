# forge build — lower a Thermite program to executable Rust and compile it with rustc into a contract-checked artifact
<!--
tier: 3-component
status: draft
audited-sha: dff9ae866e3437af272a62e078993e66c1116460 (re-audited 2026-06-12: amended — stale GREENFIELD Summary + pre-pin #57 sandbox-installed REQ-6 corrected, post-pin kernel-target/open-hole drift noted, #262)
governs: forge/src/build.rs
thesis-refs:
  - thermite-design.md §3
  - thermite-design.md §5.3
  - thermite-design.md §6
  - thermite-design.md §9
  - thermite-design.md Appendix A
  - thermite-design.md Appendix B
-->

## Summary

`forge build <file.th>` is the missing assembly step that turns a verified Thermite
program into a compiled, runnable artifact: parse → validate → effect-check →
(reuse `forge check`'s verification) → `thermite_lower::lower_l1` to a self-contained
executable Rust crate → invoke real `rustc`/`cargo` → a compiled artifact whose L1
`thermite_check!` contract checks are baked in and active in EVERY build profile
(§6). It is the SAME pipeline shape as `forge check` (the per-item parse→validate→
effect→lower→backend loop), but the backend is `rustc` (COMPILE) instead of `verus`
(VERIFY). There is **no new compiler**: Thermite transpiles to Rust and rustc/LLVM
is the codegen backend (§3, the stack). Alongside the artifact, `forge build` emits a
**build manifest** recording the artifact path, the achieved assurance level (reusing
`forge check`'s `Certificate`/`AssuranceManifest`), the per-fn `fx` rows, and
reproducibility info (§5.3). The `fx` rows and the runnable executable are the hooks
the #57 seccomp sandbox consumes.

This component is SHIPPED: `forge/src/build.rs` implements every REQ below (see the
REQ status table), grounded against real `rustc` (see Verification). Since the
bootstrap pin the pipeline also gained the #195 open-hole refusal and the
`--target kernel` fork (#197/#198 — governed by `.design/build/kernel-target.md`,
not this doc); see the Amendment below.

> **Appendix B note.** `forge build` is **not** in the v0.1 command surface listed in
> `thermite-design.md` Appendix B (which lists `new`/`goal`/`fill`/`edit`/`check`/
> `battery`/`audit`/`skill`/`repair`). It is an additive command tracked by crosslink
> issue #56, motivated by §3 ("Thermite lowers to Rust … inheriting the optimizer")
> and §6 (L1 checks "active all profiles") — the toolchain already EMITS compilable,
> runnable Rust at L1 (`tests/golden/l1/sum.l1.rs` is compiled and run under real
> rustc by `l1_conformance.rs`), so `build` is the documented act of turning that
> emission into a real compiled deliverable. Adding the command is an Appendix-B
> amendment the builder records in the commit (R-SPEC-4), not a code-local choice.

> **Amendment 2026-06-12 (doc-freshness re-audit, #262).** Re-verified against the
> current tree (`dff9ae86`); three drifts corrected, the REQ-1..REQ-5/REQ-7 evidence
> re-verified live (symbols + cited tests all present):
> 1. *Stale Summary*: the "GREENFIELD … every REQ NOT-STARTED" paragraph predated the
>    #56 build (the REQ-status table was already all-SHIPPED); rewritten.
> 2. *REQ-6 falsified by #57 shipping* (`56c23565`, BEFORE this doc's bootstrap pin):
>    `forge build` no longer merely "emits hooks" — `fn synthesize_entry_main in
>    build.rs` installs the seccomp prelude by default and `BuildManifest` carries
>    `pub sandbox: SandboxRecord`. REQ-6 body, the Boundaries bullet, and the REQ-6
>    status row updated.
> 3. *Post-pin drift* (`5cda1db8..HEAD`, 3 commits: #195/#197/#198): `pub fn
>    build_file in build.rs` and `pub fn emit_source in build.rs` gained a trailing
>    `target: BuildTarget` parameter, and `build_file` now REFUSES (a) ambient-`fx`
>    fns / `--entry` under `--target kernel` (#197/#198 — governed by
>    `.design/build/kernel-target.md`, NOT this doc) and (b) any open-holed item
>    BEFORE lowering (#193/#195 — the shared `goal_repl::open_hole_reason`, governed
>    by `.design/forge/goal-repl.md`). The std-target emission this doc governs is
>    byte-unchanged (`kernel_target::default_target_source_is_byte_identical_to_no_target_flag`).

## Requirements

- **REQ-1 (build pipeline: lower_l1 → emit crate → rustc → artifact).** Derived from
  §3 (Thermite lowers to Rust; rustc/LLVM is codegen) + §6 (L1 active all profiles).
  `forge build <file.th>` runs the same front of the pipeline `forge check` runs —
  `thermite_syntax::parse` → `thermite_spec::validate` → `thermite_lower::check_effects`
  — then `thermite_lower::lower_l1` to a single self-contained executable Rust source,
  emits it as a crate file, and invokes `rustc`/`cargo` to produce a compiled artifact.
  Any front-of-pipeline failure short-circuits into a `ForgeError` exactly as
  `check_file` does; no stage is skipped.

- **REQ-2 (rustc invocation: exit-status checked, crate-name gotcha handled).** Derived
  from `goal.md` R-CODE-4 (check subprocess exit status, never swallow). The `rustc`/
  `cargo` invocation mirrors `l1_conformance.rs::compile_and_run` and
  `check.rs::run_verus`: write the lowered source to a `<stem>.rs` file inside a
  per-run scratch dir, always pass `--crate-name` (the `.`-in-filename gotcha — a
  `*.l1.rs` filename breaks rustc's crate-name derivation), pass `--edition 2021`,
  CHECK the exit status, and surface a non-zero rustc exit as a structured `ForgeError`
  (mirroring the new `ForgeError::VerusAbsent`/`VerusSpawn`/`VerusOutput` family — a
  `RustcAbsent`/`RustcSpawn`/`RustcOutput` analogue). The scratch crate dir is removed
  on every exit path (the #53 leak lesson; compiled binaries are large).

- **REQ-3 (artifact form: a compiled library, with an optional generated entry runner).**
  Derived from §3 + §9 (a Thermite program is a library of contract-carrying `fn`s; the
  corpus `sum`/`binary_search` have no `main`) and the #57 setup requirement. The v0.1
  baseline deliverable is a **compiled library** (`--crate-type=rlib`) of the
  L1-checked fns. `forge build --entry <fn>` additionally appends a deterministic
  **generated entry runner** (a tiny `main` exercising the designated fn over fixed
  inputs) and produces a **runnable executable** — the form #57 needs to install a
  syscall filter and observe a violation killing the process. The entry runner uses
  only deterministic inputs (no wall-clock / rand — R-CODE-5). See OQ-1 for the
  load-bearing entry-point question.

- **REQ-4 (L1 checks baked in, active in every build profile).** Derived from §6 ("L1 …
  active all profiles") and the §3 active-all-profiles fix. The compiled artifact
  carries the always-active `thermite_check!` macro (a plain `if !(cond)`, NOT
  `debug_assert!`) that `lower_l1` emits, so every `req`/`ens`/loop-`inv` clause fires
  on violation in any profile (debug or release). `forge build` does not strip or gate
  the checks; the emitted `thermite_contract_violation` handler is the artifact's
  defined contract-failure behavior.

- **REQ-5 (build manifest: artifact path, assurance level, fx rows, reproducibility).**
  Derived from §6 (the manifest IS the trust statement), §5.3 (bit-reproducible builds,
  pinned toolchain + seeds), and Appendix A (the certificate shape). `forge build`
  emits a build-record document: the artifact path + crate-type, the achieved assurance
  level (the `Certificate`/`AssuranceManifest` produced by reusing the `forge check`
  pipeline), the per-fn `fx` rows (the `effects_of`/`EffectRow` projection — the input
  #57's seccomp filter is derived from), and reproducibility info (the pinned toolchain
  identity + the deterministic-source guarantee, with the honest archive-timestamp
  caveat from Verification).

- **REQ-6 (the #57 hook: runnable executable + fx rows + the installed sandbox).**
  Derived from §9 (the runtime "enforces the row as a sandbox … killed at the syscall
  boundary") and the issue-#57 setup. `forge build` provides the two inputs #57
  consumes: (a) a runnable executable (REQ-3 `--entry`) and (b) the per-fn `fx` rows
  in the build manifest (REQ-5) the syscall filter is derived from. SUPERSEDED IN
  PART by #57 shipping (`56c23565`, pre-pin — Amendment 2026-06-12): `forge build
  --entry` now also INSTALLS the sandbox itself. `fn synthesize_entry_main in
  build.rs` injects `sandbox::emit_sandbox_prelude(&allowlist)` — allowlist =
  `sandbox::syscall_allowlist(&sandbox::transitive_fx(program, &f.name))` — as the
  FIRST statements of the generated `main`, ON BY DEFAULT (`SandboxConfig::default()`
  → `SandboxMode::On`; `--no-sandbox` opts out), and records the result as
  `pub sandbox: SandboxRecord` on the `BuildManifest` (`installed` +
  `transitive_fx` + `syscall_allowlist`). The sandbox MECHANISM (`sandbox.rs`) is
  governed by `.design/forge/runtime-sandbox.md`; this doc governs the build-side
  wiring only.

- **REQ-7 (`--out <PATH>`: place the artifact at a user-named runnable path).** Derived
  from §3 (the artifact is a real compiled deliverable) + the #128 motivation. By
  default the compiled artifact lives at a stable per-run `/tmp/forge_*_build_out_<pid>/`
  output dir (REQ-2 — it is copied out of the ScratchDir before cleanup so it survives
  #53), which forces a wrapper script to run it. `forge build --out <PATH>` (and the
  short `-o <PATH>`) COPIES that artifact to the user-named `<PATH>` and marks it
  executable, so a built binary is a real `./<PATH>` run directly (the verified editor
  becomes a standalone `./nano`). An existing `<PATH>` is OVERWRITTEN (a build output is
  regenerable). The build manifest / human + `--json` output reports the FINAL path
  (`<PATH>` when `--out`, else the existing /tmp path). This is a build-output PLACEMENT
  convenience only — it does NOT change verification/lowering; the artifact is
  BYTE-IDENTICAL, just placed at `<PATH>`. A copy failure (bad directory, permission) is
  a structured `ForgeError::Io` (R-CODE-4), never a panic; `<PATH>` is never partially
  written on failure.

## Acceptance criteria

All ACs are mechanically checkable. They tie to a `conformance/build/` oracle the
ORCHESTRATOR authors (not this doc — see "Fixtures the orchestrator must author"), and
reuse the corpus + the `l1_conformance.rs` compile-and-run pattern. Each is grounded
below (Verification) against real rustc.

- **AC-1 (sum builds: rustc exit 0).** `forge build conformance/sum.th` lowers via
  `lower_l1` and produces a compiled artifact with `rustc`/`cargo` **exit status 0**.
  The emitted source compiles clean (warnings — unused `fn` in a lib — are allowed; a
  non-zero exit is a hard fail surfaced as `ForgeError`).

- **AC-2 (checks baked in).** The compiled artifact's source contains the always-active
  `thermite_check!` macro (`if !($cond)`) and NO `debug_assert` — the §6 every-profile
  property is structurally present (the same check `l1_conformance.rs::
  no_debug_assert_in_emission` asserts on the lowered source).

- **AC-3 (the executable runs correctly).** `forge build conformance/sum.th --entry sum`
  produces a runnable binary that, run, prints `sum(&[1,2,3]) = 6` and exits 0
  (`sum(&[1,2,3]) == 6` is the hand-derived value from §Appendix A's `spec_sum`
  denotation — R-CHAR-3, never copied from toolchain output).

- **AC-4 (the check FIRES on a violation, observably).** A corrupted sum body
  (`acc = acc + xs[i] as u64` → `… + 1`) still COMPILES (rustc exit 0 — only the
  runtime check is affected), but the built binary, run, ABORTS with a non-zero exit
  and the structured diagnostic `thermite L1 contract violation [inv]` (or `[ens]`) —
  the contract failure is OBSERVABLE, never silent (this is the #57-relevant kill
  behavior; mirrors `l1_conformance.rs::negative_fixture_fires_violation`).

- **AC-5 (build manifest records sum's `fx pure`).** The build manifest for
  `conformance/sum.th` lists `sum` with effect row `["pure"]` (the
  `effects_of`/`EffectRow::Pure` projection; Appendix A's certificate has
  `"effects": ["pure"]`) — the per-fn `fx` row #57's filter is derived from.

- **AC-6 (deterministic source; reproducible artifact).** The `lower_l1`-emitted
  source for `conformance/sum.th` is **bit-identical** across two builds (forge owns
  this determinism; §5.3). The compiled `.rlib` is **byte-identical** across two
  same-input builds once two nondeterminism sources are pinned: (1) the archive
  member-mtime, pinned via `SOURCE_DATE_EPOCH=0`; and (2) the per-run scratch path
  baked into the artifact's debug metadata, pinned by compiling the RELATIVE filename
  (cwd = the scratch dir) plus `--remap-path-prefix=<scratch>=.`. With both pinned the
  residual is **zero bytes** — `build_conformance::rebuilt_library_is_byte_identical`
  asserts a byte-for-byte equal rlib. (The original grounding measured one residual
  byte using ABSOLUTE source paths + no remap; the shipped impl additionally pins the
  path, closing that byte. The manifest's `reproducibility.note` states the
  `SOURCE_DATE_EPOCH` pin honestly.)

- **AC-7 (exit-status discipline).** A `rustc` failure (e.g. an intentionally
  un-compilable injected fixture) yields a non-zero `forge build` exit and a structured
  `ForgeError`, never a silent success (R-CODE-4).

- **AC-8 (`--out` places a runnable binary).** `forge build conformance/sum.th --entry
  sum --out <tmpdir>/sum` places the compiled binary at EXACTLY `<tmpdir>/sum` (the
  manifest's `artifact` is that path), the file is executable, and running it DIRECTLY
  prints `6` (the hand-derived `sum(&[1,2,3]) == 6`). The short `-o <PATH>` is
  equivalent. Without `--out` the existing /tmp output path is reported (unchanged). A
  `--out` under a non-existent directory yields a non-zero exit + a structured
  `ForgeError::Io`, never a panic, and writes no artifact (`build_conformance::
  out_places_runnable_binary` + `out_bad_path_is_structured_error`).

## Architecture

`forge build` is structurally `forge check` with the verus backend swapped for rustc.
The front of the pipeline is shared verbatim: `forge check`'s `check_file` (in
`check.rs`) runs `thermite_syntax::parse` → `thermite_spec::validate` →
`thermite_lower::check_effects`, then per item assembles an `item_subprogram`
(`check.rs`) and lowers it. `forge build` reuses that front, then diverges at the
backend:

- **The lowering.** `forge check`'s L3 path calls `thermite_lower::lower` (Verus
  source); `forge build` calls `pub fn lower_l1 in l1.rs`, which already emits a single
  self-contained, runnable Rust source — the always-active `thermite_check!` macro +
  `thermite_contract_violation` handler (`emit_check_macro` in `l1.rs`), every
  combinator's executable form (`emit_combinator_l1_defs`, sourced from the
  `thermite-spec` registry `l1` field), every `spec fn` as a real recursive Rust fn
  (`lower_spec_fn_l1`), and every `fn` with its `req`/`ens`/`inv` checks woven in
  (`lower_fn_l1`). `lower_l1` does NOT emit a `main` — the program is a library of fns
  (REQ-3, OQ-1). This is the same emission the L1 golden `tests/golden/l1/sum.l1.rs`
  pins and `l1_conformance.rs::compile_and_run` compiles + runs under real rustc.

- **The backend.** Where `forge check` calls `run_verus` (in `check.rs`) — which writes
  a `<stem>.rs` (no `.` in the stem, via its `crate_stem` helper) inside a per-run
  scratch dir (`unique_scratch_dir`), spawns the verifier with `current_dir` set to the
  scratch dir, checks the exit code, and removes the scratch dir wholesale via a
  `ScratchDir` Drop guard on every exit path (the #53 leak fix) — `forge build` does the
  analogous thing with `rustc`/`cargo`: write the lowered source, pass `--crate-name`
  (the dotted-filename gotcha that `l1_conformance.rs::compile_and_run` documents),
  `--edition 2021`, and the crate-type (`rlib` for the library; a bin for `--entry`),
  check the exit status, surface a non-zero exit as a structured `ForgeError`, and clean
  the scratch dir on every path (REQ-2; compiled artifacts are large — the #53 lesson).

- **Toolchain identity for reproducibility.** `forge check` resolves and pins the verus
  version (`resolve_verus_version` in `check.rs`, honoring a `VERUS_VERSION` env pin) so
  the proof cache is keyed deterministically. `forge build` resolves the analogous rustc
  identity (`rustc --version`/`--version --verbose` commit hash, honoring an env pin) and
  records it in the build manifest as the §5.3 pinned-toolchain field — the bit
  reproducibility claim is "same toolchain → same codegen".

- **The build manifest.** `forge build` reuses the `Certificate`/`AssuranceManifest`
  vocabulary that `manifest.rs` defines: `struct Certificate` (Appendix A field order,
  incl. `effects: Vec<String>` from `effects_of`), `AssuranceManifest::aggregate` (the
  per-fn `FunctionAssurance` rows + the `ProjectAssurance::{Certified(min)|Failed}`
  headline). The build record adds the artifact path + crate-type and the
  reproducibility block (pinned rustc identity + the deterministic-source guarantee).
  The per-fn `fx` rows come from the `EffectRow`/`effects_of` projection in `effects.rs`
  / `manifest.rs`. These rows + the runnable executable are the #57 seccomp hooks (REQ-6,
  §9). The `forge build` entry is dispatched from `cli.rs`'s `run`/`parse_args` as a new
  `Command::Build { file, entry, json, .. }` arm, mapping its outcome to an `ExitCode`
  via the existing `EXIT_VERIFICATION_FAILURE`/`EXIT_ENVIRONMENT` constants (`cli.rs`).

Boundaries (what `forge build` is NOT):
- `forge check` (#5) VERIFIES (verus, the L3/SMT path); `forge build` (#56) COMPILES
  (rustc). They share the pipeline front, not the backend.
- The runtime seccomp SANDBOX mechanism (the `fx`→syscall mapping + filter emission,
  `sandbox.rs`) is governed by `.design/forge/runtime-sandbox.md` — and since #57
  shipped, `forge build --entry` INSTALLS it by default via the generated `main`'s
  prelude (REQ-6 as amended). This doc governs only the build-side wiring
  (`SandboxConfig` threading + the `BuildManifest::sandbox` record).
- Cross-platform packaging, optimization-flag selection, multi-file Thermite projects:
  future work, out of v0.1 scope.

## Verification

The ACs are discharged by the `forge build` conformance test the builder adds, against
the `conformance/build/` oracle the orchestrator authors, reusing the corpus and the
`l1_conformance.rs::compile_and_run` pattern. The discharge commands:

- `cargo test -p forge` — the `build.rs` unit + integration tests (AC-1..AC-7).
- The build conformance test compiles `forge build conformance/sum.th`'s emitted crate
  under real rustc and asserts exit 0 (AC-1), the baked-in macro (AC-2), runs the
  `--entry sum` binary and asserts `sum(&[1,2,3]) = 6` (AC-3), runs the corrupted binary
  and asserts a non-zero exit + the `[inv]`/`[ens]` diagnostic (AC-4), asserts the
  manifest's `sum` row carries `["pure"]` (AC-5), double-builds and diffs the emitted
  source (bit-identical) + the rlib codegen (AC-6), and asserts a non-zero `forge build`
  exit on an un-compilable fixture (AC-7).
- `cargo clippy -p forge --all-targets -- -D warnings` and `cargo fmt --check` (the
  gauntlet).

**This doc is grounded against real rustc (rustc 1.95.0).** The exact `lower_l1` output
for `conformance/sum.th` was emitted (the production `thermite_lower::lower_l1`), then:

1. **Library form (REQ-3 baseline).** Compiled `--crate-type=rlib --crate-name
   sum_thermite` → **rustc exit 0**, produced `libsum_thermite.rlib` (the L1-checked fns
   as a library; rustc emits dead-code WARNINGS for the unused fns, not errors — AC-1).
2. **Executable form (REQ-3 `--entry`).** Appended a generated `fn main() { let r =
   sum(&[1u32,2,3]); println!("sum(&[1,2,3]) = {r}"); }` runner, compiled → **rustc exit
   0**, ran the binary → printed `sum(&[1,2,3]) = 6`, **exit 0** (AC-3). The
   `--crate-name` flag is mandatory (the `.l1.rs` dotted-filename gotcha; AC handled in
   REQ-2).
3. **Violation form (REQ-4 / AC-4 — the #57 kill behavior).** Corrupted the fold
   (`+ xs[i] as u64` → `+ xs[i] as u64 + 1`); the binary still **compiled (rustc exit
   0)** but, run, ABORTED with **exit 101** and printed
   `thermite L1 contract violation [inv]: acc == spec_sum(&xs[..i])` — the always-active
   check fired observably, never reaching the runner's tail.
4. **Reproducibility (REQ-5 / AC-6, §5.3).** The `lower_l1` source was **bit-identical**
   across two emissions (forge-owned determinism). The original grounding (ABSOLUTE source
   path, no remap, `SOURCE_DATE_EPOCH` unpinned) measured the `.rlib` differing in the `ar`
   archive member-mtime header. The SHIPPED `invoke_rustc` pins BOTH residuals —
   `SOURCE_DATE_EPOCH=0` (the mtime) and the relative-filename + `--remap-path-prefix` (the
   embedded scratch path) — so two same-input `.rlib` builds are now **byte-identical**
   (`build_conformance::rebuilt_library_is_byte_identical`). The manifest's
   `reproducibility.note` records the `SOURCE_DATE_EPOCH` pin honestly.

All grounding scratch was created under `/tmp` and removed; no artifacts leaked into the
repo tree (the #53 lesson — compiled artifacts are large).

## Fixtures the orchestrator must author (NOT this doc)

The doc-author does not author production code, routes, or the oracle. The orchestrator
must:

- Add the route to `tooling/spec-routes.toml`:
  `crate_pattern = "forge/src/build.rs"`, `design = ".design/forge/build.md"`,
  `reference = ["conformance/build"]`, `conformance_ops = ["sum"]`.
- Author the `conformance/build/` oracle the ACs reference — at minimum the EXACT
  fixtures used in the grounding:
  - **`conformance/sum.th`** (existing corpus) — the build input for AC-1/2/3/5/6.
  - the positive `--entry sum` expected stdout `sum(&[1,2,3]) = 6` (R-CHAR-3:
    hand-derived from Appendix A's `spec_sum`, not toolchain output) — AC-3.
  - the corrupted-body fixture (`acc = acc + xs[i] as u64 + 1;`) + its expected non-zero
    exit and `thermite L1 contract violation [inv]`/`[ens]` diagnostic — AC-4.
  - an un-compilable fixture (or an injected source edit) for the AC-7 exit-status check.
  - the expected build-manifest `sum` row: `"effects": ["pure"]` (Appendix A) — AC-5.

## Open questions

- **OQ-1 (load-bearing — the entry-point form).** A v0.1 Thermite program is a library
  of fns with no `main` (the corpus). What runnable form does `forge build` produce for
  #57? Options laid out:
  - (a) **library only** (`.rlib`) — the baseline; but #57 needs a runnable binary, so
    insufficient alone.
  - (b) **library + an optional generated `--entry <fn>` runner** — a tiny deterministic
    `main` calling the designated fn; this doc DECIDES this (REQ-3): the library is the
    baseline deliverable and `--entry` produces the observable binary #57 installs a
    filter into.
  - (c) **a designated-fn harness binary for the #57 demo** (a fixed small binary that
    exercises one fn) — subsumed by (b)'s `--entry`.
  The DECISION (b) is grounded above (both forms compile under rustc; the `--entry`
  binary runs and the check fires). The residual question for #56/#57: what argument/
  input convention the generated runner uses (fixed literals vs a `--input` flag). v0.1
  uses fixed deterministic literals (R-CODE-5); a richer convention is future work.
- **OQ-2 (rustc vs cargo).** The grounding used raw `rustc` (matching
  `l1_conformance.rs` and `run_verus`'s direct-spawn pattern, which keeps the build
  hermetic and deterministic). `cargo build` would add a generated `Cargo.toml` +
  dependency resolution; v0.1 single-file programs do not need it. DECIDED: raw `rustc`
  for v0.1 (consistent with the existing compile-and-run pattern); `cargo` is future
  work for multi-file/dependency programs.
- **OQ-3 (manifest format unification).** Should the build manifest be a distinct
  document or an extension of the `forge check`/`forge audit` `AssuranceManifest`? This
  doc reuses the `Certificate`/`AssuranceManifest` vocabulary (REQ-5) and adds the
  artifact-path + reproducibility block; whether that is a new `BuildManifest` struct or
  an additive field set on the existing manifest is a builder decision constrained by
  R-SPEC-2 (no breaking the frozen certificate schema).

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (build pipeline: lower_l1 → emit → rustc) | SHIPPED | `pub fn build_file in build.rs` runs `parse`/`validate`/`check_effects` (the `check_file` front, via `parse_program`), `thermite_lower::lower_l1` (via `emit_source`), writes a crate, invokes `rustc` (`invoke_rustc`); short-circuits into `ForgeError`. Consumer: `cli::run_build` (`cli.rs`). Verified by `build_conformance::sum_runs` + `sum_builds_as_library`. |
| REQ-2 (rustc invocation; exit-status; crate-name gotcha) | SHIPPED | `invoke_rustc in build.rs` passes `--crate-name` (no `.` — `crate_name_for`), `--edition 2021`, checks `status.success()` → `ForgeError::RustcOutput`; spawn ENOENT → `ForgeError::RustcAbsent`; reuses `check::ScratchDir`'s Drop guard + `unique_scratch_dir` to remove the crate dir wholesale. `RustcAbsent`/`RustcSpawn`/`RustcOutput` added to `ForgeError` in `cli.rs`. Verified by `uncompilable_lowering_is_nonzero_exit` (AC-7). |
| REQ-3 (artifact form: library + optional `--entry` runner) | SHIPPED | `build_file(.., entry: None, ..)` → `CrateType::Rlib`; `build_file(.., entry: Some(fn), ..)` → `CrateType::Bin` (the signature has since gained `sandbox: SandboxConfig` (#57) and a trailing `target: BuildTarget` (#197) — Amendment 2026-06-12) with `synthesize_entry_main`'s deterministic runner (`&[u32]` → `&[1u32,2,3]`, scalars → fixed literals). Verified by `sum_runs` (exe prints `6`) + `sum_builds_as_library`. |
| REQ-4 (L1 checks baked in, all profiles) | SHIPPED | the artifact is `lower_l1`'s output verbatim (the always-active `thermite_check!`, NOT `debug_assert!`); `build_file` never strips it. Verified by `ens_violation_fires_at_runtime` (the runtime `[ens]` check fires, non-zero exit) + `checks_are_baked_in` (AC-2: macro present, no `debug_assert`). |
| REQ-5 (build manifest: path, level, fx rows, reproducibility) | SHIPPED | `struct BuildManifest in build.rs` composes the artifact path + `CrateType`, the assurance string `"L1 (built, runtime-checked)"`, the per-fn `fx` rows (`effects_of` via `build_functions`), and the `Reproducibility` block (pinned `rustc` identity via `resolve_rustc_version` + `SOURCE_DATE_EPOCH=0`). Consumer: `cli::run_build` (human `render_build` + `--json`). Verified by `rebuilt_library_is_byte_identical` (AC-6: byte-identical rlib via `SOURCE_DATE_EPOCH` + `--remap-path-prefix`). |
| REQ-6 (#57 hook: runnable exe + fx rows + installed sandbox) | SHIPPED | the `--entry` runnable binary (REQ-3) + `BuildManifest::functions` `fx` rows (`sum` → `["pure"]`); since #57 (`56c23565`, pre-pin), `fn synthesize_entry_main in build.rs` injects `sandbox::emit_sandbox_prelude` (the fx-derived allowlist) as the FIRST statements of the generated `main`, on by default (`SandboxConfig::default()` → `SandboxMode::On`; `--no-sandbox` opts out), recorded as `pub sandbox: SandboxRecord` on `BuildManifest`. Consumer: `cli::run_build` (`cli.rs`). Verified by `build_conformance::sum_runs` (`fx == ["pure"]` + the binary runs) + the `sandbox_conformance.rs` suite. |
| REQ-7 (`--out <PATH>`: place the artifact at a user-named runnable path) | SHIPPED | `build_file(.., out: Option<&Path>)` copies the stable /tmp artifact to `<PATH>` via `place_artifact in build.rs` (overwrite + `chmod +x`; #128), reports `<PATH>` as `BuildManifest::artifact`; `None` keeps the existing /tmp path; a bad `<PATH>` → `ForgeError::Io`. Consumer: `cli::run_build` threads the `--out`/`-o` flag (`Command::Build.out`). Verified by `build_conformance::out_places_runnable_binary` (AC-8: placed, executable, runs, prints 6) + `out_bad_path_is_structured_error` (structured error, no panic) + `cli::parses_build_out_flag`. |
