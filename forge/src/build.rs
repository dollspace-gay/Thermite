//! `forge/src/build.rs` — `forge build [<file>] [--entry <fn>]`: lower a verified
//! Thermite program to executable Rust and compile it with the REAL `rustc` into a
//! contract-checked artifact. It is structurally `forge check` with the verus
//! backend swapped for rustc: it reuses `check_file`'s pipeline FRONT
//! (`thermite_syntax::parse` → `thermite_spec::validate` →
//! `thermite_lower::check_effects`), then calls `thermite_lower::lower_l1` (the
//! always-active `thermite_check!` exec lowering) and invokes `rustc` instead of
//! `verus`. There is NO new compiler: Thermite transpiles to Rust and rustc/LLVM
//! is the codegen backend (`thermite-design.md` §3).
//!
//! Governing design: `.design/forge/build.md`. Oracle: `conformance/build/cases.json`.
//!
//! ## Pipeline (REQ-1/REQ-2)
//!
//! ```text
//! parse → validate → check_effects → lower_l1 → write crate (ScratchDir) → rustc → artifact
//! ```
//!
//! The scratch crate dir is a per-run [`check::ScratchDir`] removed WHOLESALE on
//! every exit path (the #53 leak lesson; a compiled `.rlib`/binary is large). The
//! emitted artifact is COPIED out of the scratch dir to a stable per-run output
//! directory before the scratch dir is dropped, so the artifact survives cleanup.
//! `forge build --out <PATH>` (`-o`) then COPIES that artifact to a user-named path
//! (executable; REQ-7), so a built binary is a real `./<name>` you run directly —
//! no awkward `/tmp/forge_*_build_out_<pid>/` path / wrapper script (#128).
//!
//! ## Artifact form (REQ-3, OQ-1 decision (b))
//!
//! - Default → a compiled **library** (`--crate-type=rlib`) of the L1-checked fns.
//! - `forge build --entry <fn>` → appends a deterministic generated `main` that
//!   calls the entry fn with DETERMINISTIC synthesized sample inputs per its param
//!   types (see [`synthesize_entry_main`]) and produces a RUNNABLE executable — the
//!   #57 hook (a binary a seccomp filter can be installed into; REQ-6).
//!
//! ## Reproducibility (REQ-5, §5.3)
//!
//! The `lower_l1` emission is byte-deterministic (forge owns this). `rustc` is
//! invoked with `SOURCE_DATE_EPOCH=0` pinned so the codegen is reproducible modulo
//! the residual `ar` archive member-mtime header (the honest caveat the
//! [`BuildManifest`] records; `.design/forge/build.md` AC-6).
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (build pipeline: lower_l1 → emit → rustc → artifact) | SHIPPED | `pub fn build_file` runs `parse`/`validate`/`check_effects` (the `check_file` front), `thermite_lower::lower_l1`, writes a crate, invokes `rustc` (`invoke_rustc`); short-circuits into `ForgeError`. Consumer: `cli::run_build`. Verified by `build_conformance::sum_runs`. |
//! | REQ-2 (rustc invocation: exit-checked, crate-name gotcha, scratch cleanup) | SHIPPED | `invoke_rustc` passes `--crate-name` (no `.` — `crate_name_for`), `--edition 2021`, checks `status.success()` → `ForgeError::RustcOutput`; spawn ENOENT → `ForgeError::RustcAbsent`; the `check::ScratchDir` Drop guard removes the crate dir wholesale. |
//! | REQ-3 (artifact form: library + optional `--entry` runner) | SHIPPED | `build_file(path, None)` → `CrateType::Rlib`; `build_file(path, Some(fn))` → `CrateType::Bin` with `synthesize_entry_main`'s deterministic runner. Verified by `sum_runs` (exe prints `6`). |
//! | REQ-4 (L1 checks baked in, all profiles) | SHIPPED | the artifact is `lower_l1`'s output verbatim (the always-active `thermite_check!`, NOT `debug_assert!`); `build_file` never strips it. Verified by `ens_violation_fires_at_runtime` (the runtime check fires). |
//! | REQ-5 (build manifest: path, level, fx rows, reproducibility) | SHIPPED | `BuildManifest` composes the artifact path + `CrateType`, the achieved assurance string, the per-fn `fx` rows (`effects_of`), and the `Reproducibility` block (pinned rustc identity + `SOURCE_DATE_EPOCH`). Consumer: `cli::run_build` (human + `--json`). |
//! | REQ-6 (#57 hook: runnable exe + fx rows + the seccomp sandbox) | SHIPPED | the `--entry` runnable binary (REQ-3) + `BuildManifest::functions` `fx` rows (e.g. `sum` → `["pure"]`); `synthesize_entry_main` now injects the #57 `sandbox::emit_sandbox_prelude` (the fx-derived seccomp filter) as the FIRST statements of the generated `main` (`SandboxConfig`, on by default for `--entry`), recording the installed allowlist in `BuildManifest::sandbox`. Verified by `sum_runs` (`fx == ["pure"]`) + `sandbox_conformance` (pure runs clean, the openat probe killed/allowed). |
//! | REQ-7 (`--out <PATH>`: place the artifact at a user-named runnable path) | SHIPPED | `build_file(.., out: Option<&Path>)` copies the stable /tmp artifact to `<PATH>` via `place_artifact` (overwrite + `chmod +x` so `./<PATH>` runs directly; #128), reports `<PATH>` as `BuildManifest::artifact`; `None` keeps the existing /tmp path unchanged; a bad `<PATH>` → `ForgeError::Io`. Consumer: `cli::run_build` (threads the `--out`/`-o` flag). Verified by `build_conformance::out_places_runnable_binary`. |

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;
use thermite_syntax::{FnItem, Item, PrimType, Program, Type};

use std::collections::BTreeSet;

use crate::check::{unique_scratch_dir, ScratchDir};
use crate::cli::ForgeError;
use crate::effect_wrappers;
use crate::manifest::effects_of;
use crate::sandbox::{self, SandboxMode};

/// The pinned `SOURCE_DATE_EPOCH` for every `forge build` rustc invocation
/// (REQ-5, §5.3). A fixed `0` makes the codegen reproducible modulo the residual
/// archive timestamp — DETERMINISTIC (R-CODE-5: an explicit input, not wall-clock).
const SOURCE_DATE_EPOCH: &str = "0";

/// The Rust edition every emitted crate is compiled under (mirrors
/// `l1_conformance.rs::compile_and_run` + `check.rs`'s verus invocation).
const EDITION: &str = "2021";

/// The honest L1 assurance statement (`.design/forge/build.md` REQ-4): `forge
/// build` builds any well-formed program; the runtime check is the assurance, not
/// an SMT proof. NOT a forged L3 claim (R-DEFER-9).
const ASSURANCE_L1: &str = "L1 (built, runtime-checked)";

/// The artifact kind `forge build` produces (REQ-3). The default is a library;
/// `--entry <fn>` produces a runnable executable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CrateType {
    /// A compiled library of the L1-checked fns (`--crate-type=rlib`). The
    /// baseline deliverable (REQ-3 (a)).
    Rlib,
    /// A runnable executable: the L1-checked fns plus a deterministic generated
    /// `main` exercising the `--entry` fn (REQ-3 (b)). The #57 hook (REQ-6).
    Bin,
}

impl CrateType {
    /// The `--crate-type` value rustc expects.
    fn rustc_arg(self) -> &'static str {
        match self {
            CrateType::Rlib => "rlib",
            CrateType::Bin => "bin",
        }
    }
}

/// The #57 sandbox configuration for a `forge build --entry` (REQ-4/REQ-6). The
/// sandbox is ON BY DEFAULT for `--entry` (`--no-sandbox` opts out); the self-test
/// probe is injected ONLY under `--sandbox-self-test`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SandboxConfig {
    /// Whether to inject the seccomp prelude (`SandboxMode::On` by default for
    /// `--entry`; `SandboxMode::Off` under `--no-sandbox`).
    pub mode: SandboxMode,
    /// Whether to inject the `--sandbox-self-test` `openat` probe AFTER the prelude
    /// (test-only demonstrability device; never in a production runner).
    pub self_test: bool,
}

impl Default for SandboxConfig {
    /// The `--entry` default (REQ-4): sandbox ON, no self-test probe.
    fn default() -> Self {
        SandboxConfig {
            mode: SandboxMode::On,
            self_test: false,
        }
    }
}

/// The #57 sandbox record on the [`BuildManifest`] (REQ-5): what the runnable
/// binary is confined to. Recorded for the audit surface (§9) — the installed
/// syscall allowlist, derived from the entry's transitive `fx`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SandboxRecord {
    /// `true` iff the seccomp prelude was injected (REQ-4: on by default for
    /// `--entry`, suppressed by `--no-sandbox` / a library build).
    pub installed: bool,
    /// The transitive `fx` tokens the allowlist was derived from (REQ-2), sorted.
    pub transitive_fx: Vec<String>,
    /// The installed x86_64 syscall allowlist (REQ-3), sorted ascending. Empty when
    /// no prelude was injected.
    pub syscall_allowlist: Vec<u32>,
}

/// One function's per-fn row in the [`BuildManifest`] (REQ-5/REQ-6): its name and
/// its effect row (`fx`), the input #57's seccomp filter is derived from. A `spec
/// fn` carries no contract (§4.2), so only `Item::Fn`s contribute rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BuildFunction {
    /// The function name.
    pub name: String,
    /// The effect row tokens (`effects_of` projection): `sum` → `["pure"]`.
    pub fx: Vec<String>,
}

/// The reproducibility block (REQ-5, §5.3): the pinned toolchain identity + the
/// deterministic-source guarantee + the honest archive-timestamp caveat
/// (`.design/forge/build.md` AC-6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Reproducibility {
    /// The resolved rustc version string (`rustc --version`, honoring a
    /// `RUSTC_VERSION` env pin). The §5.3 pinned-toolchain field — the bit
    /// reproducibility claim is "same toolchain → same codegen".
    pub rustc: String,
    /// The pinned `SOURCE_DATE_EPOCH` value passed to rustc (always `"0"`).
    pub source_date_epoch: String,
    /// The honest caveat: the emitted source is byte-identical and the codegen is
    /// reproducible modulo the `ar` archive member-mtime header (one byte; pinned
    /// via `SOURCE_DATE_EPOCH`).
    pub note: String,
}

/// The build record `forge build` emits alongside the artifact (REQ-5). A SEPARATE
/// struct that COMPOSES the `forge check` cert vocabulary (`effects_of`) — it does
/// NOT mutate the frozen `Certificate` schema (R-SPEC-2, OQ-3 decision). Carries
/// the artifact path + crate-type, the achieved assurance level, the per-fn `fx`
/// rows (REQ-6), and the reproducibility block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BuildManifest {
    /// The compiled artifact's path (the stable output, copied out of the scratch
    /// dir before cleanup).
    pub artifact: PathBuf,
    /// The artifact kind (rlib library or runnable bin).
    pub crate_type: CrateType,
    /// The achieved assurance level. `forge build` at L1 builds any well-formed
    /// program; the always-active runtime `thermite_check!` IS the assurance
    /// (`.design/forge/build.md` REQ-4), so this records the honest L1 statement
    /// `"L1 (built, runtime-checked)"` — NOT a forged L3 proof claim.
    pub assurance: String,
    /// The `--entry` fn, when a runnable executable was produced (REQ-3 (b)).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry: Option<String>,
    /// The per-fn `fx` rows (REQ-5/REQ-6) in source order — the #57 seccomp input.
    pub functions: Vec<BuildFunction>,
    /// The #57 sandbox record (REQ-5): the installed syscall allowlist, derived
    /// from the entry's transitive `fx`. `installed == false` for a library build /
    /// `--no-sandbox`.
    pub sandbox: SandboxRecord,
    /// The reproducibility block (REQ-5, §5.3).
    pub reproducibility: Reproducibility,
}

/// Lower the program at `path` to executable Rust and compile it with `rustc`
/// into a contract-checked artifact (REQ-1). Reuses the `forge check` pipeline
/// front (parse → validate → check_effects), then `thermite_lower::lower_l1`s the
/// program, writes a self-contained crate into a per-run scratch dir, invokes
/// rustc, copies the artifact out, and returns the [`BuildManifest`].
///
/// - `entry == None` → a library artifact (`--crate-type=rlib`).
/// - `entry == Some(fn)` → a runnable executable whose generated `main` calls
///   `fn` with deterministic synthesized inputs (REQ-3).
///
/// A front-of-pipeline failure short-circuits into a `ForgeError` exactly as
/// `check_file` does; a non-zero rustc exit is `ForgeError::RustcOutput`
/// (R-CODE-4). `forge build` at L1 builds any well-formed program — a
/// contract-violating body BUILDS and its `thermite_check!` fires at RUNTIME (that
/// is the point; the oracle's `runtime_violation` case).
pub fn build_file(
    path: impl AsRef<Path>,
    entry: Option<&str>,
    sandbox: SandboxConfig,
    out: Option<&Path>,
) -> Result<BuildManifest, ForgeError> {
    let path = path.as_ref();
    // The full compiled source (lower_l1 + any --entry runner + the #57 sandbox
    // prelude) — the SAME byte-deterministic emission the reproducibility check
    // (AC-6) asserts is stable (`emit_source` is this build's source-of-truth,
    // REQ-5).
    let source = emit_source(path, entry, sandbox)?;
    let program = parse_program(path)?;

    // REQ-3: a `--entry` produced the deterministic generated runner inside
    // `emit_source` → a runnable executable; the default is a library (rlib) of the
    // L1-checked fns.
    let (crate_type, entry_name) = match entry {
        Some(name) => (CrateType::Bin, Some(name.to_string())),
        None => (CrateType::Rlib, None),
    };

    // 5. rustc: write the crate into a per-run scratch dir, compile, copy the
    // artifact out, and clean the scratch dir wholesale on every exit path (#53).
    let crate_name = crate_name_for(path);
    let built = invoke_rustc(&crate_name, &source, crate_type)?;

    // REQ-7 (`--out <PATH>`): the artifact lives at a stable per-run /tmp output
    // dir (`built`). When `--out <PATH>` is given, COPY it to the user-named path
    // (overwriting — a build output), mark it executable, and report `<PATH>` as
    // the FINAL artifact path. Without `--out`, the existing /tmp path is the
    // artifact (unchanged). The copy is a pure placement step: the artifact is
    // BYTE-IDENTICAL (verification/lowering are untouched, R-CODE-5).
    let artifact = match out {
        Some(dest) => place_artifact(&built, dest)?,
        None => built,
    };

    // REQ-5/REQ-6: the build record — per-fn `fx` rows + the #57 sandbox record +
    // reproducibility. The sandbox is only INSTALLED for an `--entry` runner with
    // `SandboxMode::On`; a library build / `--no-sandbox` records `installed: false`
    // with an empty allowlist.
    let functions = build_functions(&program);
    let sandbox_record = match (entry, sandbox.mode) {
        (Some(name), SandboxMode::On) => {
            let fx = sandbox::transitive_fx(&program, name);
            let allowlist = sandbox::syscall_allowlist(&fx);
            SandboxRecord {
                installed: true,
                transitive_fx: fx.into_iter().collect(),
                syscall_allowlist: allowlist,
            }
        }
        _ => SandboxRecord {
            installed: false,
            transitive_fx: Vec::new(),
            syscall_allowlist: Vec::new(),
        },
    };
    let reproducibility = Reproducibility {
        rustc: resolve_rustc_version()?,
        source_date_epoch: SOURCE_DATE_EPOCH.to_string(),
        note: "the emitted L1 source is byte-deterministic and the codegen is \
               reproducible modulo the `ar` archive member-mtime header (one byte, \
               pinned via SOURCE_DATE_EPOCH=0)"
            .to_string(),
    };

    Ok(BuildManifest {
        artifact,
        crate_type,
        assurance: ASSURANCE_L1.to_string(),
        entry: entry_name,
        functions,
        sandbox: sandbox_record,
        reproducibility,
    })
}

/// Run the shared `check_file` FRONT (parse → validate → check_effects) over
/// `path`, returning the validated `Program` (REQ-1). Any front-of-pipeline
/// failure short-circuits into the earliest stage's `ForgeError`, exactly as
/// `check_file` does. Used by both `emit_source` (the codegen) and `build_file`
/// (the manifest + entry-fn lookup) so the front is shared verbatim.
fn parse_program(path: &Path) -> Result<Program, ForgeError> {
    let src = std::fs::read_to_string(path).map_err(|e| ForgeError::Io {
        path: path.display().to_string(),
        source: e,
    })?;
    let parsed = thermite_syntax::parse(&src);
    if !parsed.is_clean() {
        return Err(ForgeError::Parse(parsed.errors));
    }
    thermite_spec::validate(&parsed.program).map_err(ForgeError::Spec)?;
    thermite_lower::check_effects(&parsed.program).map_err(ForgeError::Effects)?;
    Ok(parsed.program)
}

/// Emit the FULL compiled L1 source for `path` (incl. any `--entry` runner)
/// WITHOUT compiling it (REQ-1/REQ-5/AC-6). This is `build_file`'s codegen
/// source-of-truth — `build_file` compiles exactly these bytes, and the
/// reproducibility test asserts they are byte-identical across two calls (forge
/// owns the emission determinism, independent of any rustc nondeterminism). The
/// `--entry` runner is appended deterministically (`synthesize_entry_main`).
pub fn emit_source(
    path: impl AsRef<Path>,
    entry: Option<&str>,
    sandbox: SandboxConfig,
) -> Result<String, ForgeError> {
    let path = path.as_ref();
    let program = parse_program(path)?;
    let lowered = thermite_lower::lower_l1(&program).map_err(ForgeError::Lower)?;

    // Basis Stage 8 (`.design/basis/08-runnable-effect-link.md` REQ-2): emit a
    // self-contained `mod os { … }` carrying EXACTLY the wrappers the program's
    // `#[boundary("os::<name>")]` targets name, PREPENDED to the lowered code so
    // `lower_boundary_fn_l1`'s `let result = os::<name>(args);` crossing RESOLVES
    // under raw rustc (closing the GROUNDED `E0433`). A program with no `os::`
    // boundary emits no module (the pure corpus is byte-unaffected, AC-7).
    let targets = reachable_boundary_targets(&program);
    let mut source = effect_wrappers::emit_mod_os(&targets)?;
    source.push_str(&lowered);

    if let Some(name) = entry {
        let f = find_entry_fn(&program, name)?;
        source.push_str(&synthesize_entry_main(&program, f, sandbox)?);
    }
    Ok(source)
}

/// Collect the DISTINCT `#[boundary("os::<name>")]` foreign targets the built
/// program names (Stage 8 REQ-2). `thermite_lower::lower_l1` emits a boundary L1
/// wrapper — containing the `os::<name>(args)` crossing — for EVERY `#[boundary]`
/// `Item::Fn` in the program (`thermite-lower/src/l1.rs` `lower_l1`'s match guard),
/// so the self-contained crate must resolve EACH such target regardless of the
/// `--entry`. The set is keyed by the `BoundaryAttr.target` string (the foreign
/// target the lowered crossing calls), so the emitted `mod os` is exactly the
/// program's live TCB surface — nothing more (minimal TCB, REQ-2/REQ-6). Returns a
/// `BTreeSet` (sorted, deterministic; R-CODE-5).
fn reachable_boundary_targets(program: &Program) -> BTreeSet<String> {
    program
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Fn(f) => f.boundary.as_ref().map(|b| b.target.clone()),
            Item::SpecFn(_) | Item::Struct(_) | Item::Enum(_) => None,
        })
        .collect()
}

/// Project the program's `Item::Fn`s into the per-fn `fx` rows for the manifest
/// (REQ-5/REQ-6). A `spec fn` carries no `req`/`ens`/`fx` contract (§4.2), so it
/// contributes no row. Source order (deterministic, R-CODE-5).
fn build_functions(program: &Program) -> Vec<BuildFunction> {
    program
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Fn(f) => Some(BuildFunction {
                name: f.name.clone(),
                fx: effects_of(&f.contract.fx),
            }),
            Item::SpecFn(_) => None,
            // Basis Stage 1a (`.design/basis/01-adts.md`): a `struct`/`enum`
            // item carries no `fx` contract row → contributes no manifest
            // function (neutral value `None`). Dead-in-1a: an ADT program dies
            // at the validator before `forge build` projects its functions.
            Item::Struct(_) | Item::Enum(_) => None,
        })
        .collect()
}

/// Resolve the `Item::Fn` named `name` in `program` (REQ-3). A missing name (or a
/// `spec fn` / boundary fn — which has no in-language body to run) is a
/// `ForgeError::Usage`, never a panic (R-CODE-2).
fn find_entry_fn<'a>(program: &'a Program, name: &str) -> Result<&'a FnItem, ForgeError> {
    let item = program.items.iter().find(|i| i.name() == name);
    match item {
        Some(Item::Fn(f)) if f.body.is_some() => Ok(f),
        Some(Item::Fn(_)) => Err(ForgeError::Usage(format!(
            "`--entry {name}` names a boundary (foreign-body) fn; its body is not in-language \
             and cannot be run as a deterministic entry point"
        ))),
        Some(Item::SpecFn(_)) => Err(ForgeError::Usage(format!(
            "`--entry {name}` names a `spec fn` (a pure spec dependency, not a runnable entry \
             point); name a `fn`"
        ))),
        // Basis Stage 1a (`.design/basis/01-adts.md`): a `struct`/`enum` type
        // is not a runnable entry point — the honest neutral value is the same
        // `Usage` refusal as a `spec fn` name. Dead-in-1a (the ADT program dies
        // at the validator before `forge build --entry` resolves an entry).
        Some(Item::Struct(_)) | Some(Item::Enum(_)) => Err(ForgeError::Usage(format!(
            "`--entry {name}` names a `struct`/`enum` type, not a runnable `fn`; name a `fn`"
        ))),
        None => Err(ForgeError::Usage(format!(
            "`--entry {name}` names no `fn` in the program"
        ))),
    }
}

/// Synthesize a deterministic `fn main` that (under the #57 sandbox) installs the
/// seccomp prelude FIRST, then calls the entry fn with fixed sample inputs per its
/// parameter types (REQ-3, R-CODE-5 — NO wall-clock / rand). The synthesis
/// convention (v0.1 fixed literals; a richer `--input` convention is future work,
/// OQ-1):
///
/// - `&[u32]` / `&[u64]` / `&[usize]` → `&[1, 2, 3]` (typed) — the corpus `sum`
///   case (`sum(&[1,2,3]) == 6`, the hand-derived value).
/// - `u32` / `u64` / `usize` → `1`.
/// - `bool` → `true`.
///
/// The generated `main` structure (#57; `.design/forge/runtime-sandbox.md`
/// Architecture, the prelude-injection point):
///
/// ```text
/// fn main() {
///     <seccomp prelude>          // SandboxMode::On (default for --entry); REQ-1/REQ-4
///     <openat self-test probe>   // --sandbox-self-test ONLY; REQ-6
///     let r = entry(<args>);     // runs UNDER the filter; the L1 thermite_check! still PANICS
///     println!("entry(args) = {r:?}");
/// }
/// ```
///
/// The seccomp prelude is the FIRST statement(s) so the entry (and any boundary/slag
/// body it reaches) runs UNDER the filter; the allowlist is the entry's transitive
/// `fx` projection (REQ-2/REQ-3). The result is `println!`'d so the runtime is
/// observable; for the `sum` corpus this prints `sum(&[1u32, 2, 3]) = 6` (the
/// oracle's `expect_run_contains: "6"`). A parameter type with no deterministic
/// synthesis is a structured error (R-CODE-2), never a panic.
fn synthesize_entry_main(
    program: &Program,
    f: &FnItem,
    sandbox: SandboxConfig,
) -> Result<String, ForgeError> {
    let mut args: Vec<String> = Vec::with_capacity(f.params.len());
    for p in &f.params {
        args.push(synthesize_arg(&p.ty).ok_or_else(|| {
            ForgeError::Usage(format!(
                "`--entry {}` has a parameter `{}` of a type the v0.1 deterministic runner \
                 cannot synthesize a sample input for; supported: &[u32|u64|usize], \
                 u32/u64/usize, bool",
                f.name, p.name
            ))
        })?);
    }
    let arglist = args.join(", ");

    // REQ-1/REQ-4: the #57 seccomp prelude is the FIRST statement of `main` (so the
    // entry runs UNDER the filter), with the allowlist derived from the entry's
    // transitive `fx` (REQ-2/REQ-3). `SandboxMode::Off` (`--no-sandbox`) emits none.
    let prelude = match sandbox.mode {
        SandboxMode::On => {
            let fx = sandbox::transitive_fx(program, &f.name);
            let allowlist = sandbox::syscall_allowlist(&fx);
            sandbox::emit_sandbox_prelude(&allowlist)
        }
        SandboxMode::Off => String::new(),
    };
    // REQ-6: the `--sandbox-self-test` probe is injected AFTER the prelude (so the
    // filter is already installed) and BEFORE the entry call (so the kill is
    // observed before any entry output). Never emitted without the flag.
    let probe = if sandbox.self_test {
        sandbox::emit_probe()
    } else {
        String::new()
    };

    // The runner binds the result and prints it; the `thermite_check!`s inside the
    // fn fire BEFORE the tail returns on a violation (REQ-4) — and the baseline
    // allowlist permits that PANIC/abort path, so a contract violation PANICS rather
    // than being seccomp-killed. `{r:?}` covers every primitive return type.
    Ok(format!(
        "\nfn main() {{\n{prelude}{probe}    let r = {name}({arglist});\n    println!(\"{name}({arglist}) = {{r:?}}\");\n}}\n",
        name = f.name
    ))
}

/// The deterministic sample argument for one parameter type (REQ-3, R-CODE-5).
/// Returns `None` for an unsupported type (the caller maps it to a structured
/// error). Covers the corpus shapes + the primitive scalars.
fn synthesize_arg(ty: &Type) -> Option<String> {
    match ty {
        Type::Prim(PrimType::U32) => Some("1u32".to_string()),
        Type::Prim(PrimType::U64) => Some("1u64".to_string()),
        Type::Prim(PrimType::Usize) => Some("1usize".to_string()),
        Type::Prim(PrimType::Bool) => Some("true".to_string()),
        // `&[T]` (a `Ref` of a `Slice`): a fixed three-element typed slice. The
        // corpus `sum(&[1,2,3]) == 6` is this case.
        Type::Ref { inner, .. } => match inner.as_ref() {
            Type::Slice(elem) => match elem.as_ref() {
                Type::Prim(PrimType::U32) => Some("&[1u32, 2, 3]".to_string()),
                Type::Prim(PrimType::U64) => Some("&[1u64, 2, 3]".to_string()),
                Type::Prim(PrimType::Usize) => Some("&[1usize, 2, 3]".to_string()),
                _ => None,
            },
            // `&u32` etc. — a referenced scalar.
            Type::Prim(PrimType::U32) => Some("&1u32".to_string()),
            Type::Prim(PrimType::U64) => Some("&1u64".to_string()),
            Type::Prim(PrimType::Usize) => Some("&1usize".to_string()),
            Type::Prim(PrimType::Bool) => Some("&true".to_string()),
            _ => None,
        },
        _ => None,
    }
}

/// Compute a valid Rust crate NAME from a source path (REQ-2 / the dotted-filename
/// gotcha). Mirrors `check.rs::crate_stem`: the file stem with every
/// non-alphanumeric char replaced by `_`, a leading-digit guard, suffixed
/// `_build`. rustc derives the crate name from the file stem and REJECTS a `.`
/// (the grounded `*.l1.rs` gotcha), so we always pass `--crate-name` AND keep the
/// `.rs` filename stem `.`-free.
fn crate_name_for(path: &Path) -> String {
    let raw = path.file_stem().and_then(|s| s.to_str()).unwrap_or("item");
    let mut name = String::with_capacity(raw.len() + 6);
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            name.push(ch);
        } else {
            name.push('_');
        }
    }
    if name
        .chars()
        .next()
        .map(|c| c.is_ascii_digit())
        .unwrap_or(true)
    {
        name.insert(0, 'c');
    }
    name.push_str("_build");
    name
}

/// Write `source` to a `<crate_name>.rs` file inside a per-run scratch dir, invoke
/// `rustc` (`--crate-name`, `--edition 2021`, `--crate-type`, `SOURCE_DATE_EPOCH=0`
/// pinned), check the exit status, copy the artifact OUT of the scratch dir to a
/// stable per-run output dir, and let the [`ScratchDir`] Drop guard remove the
/// scratch dir WHOLESALE on every exit path (REQ-2, #53). Returns the stable
/// artifact path. A spawn ENOENT → `ForgeError::RustcAbsent`; a non-zero exit →
/// `ForgeError::RustcOutput` (R-CODE-4 — never swallowed).
fn invoke_rustc(
    crate_name: &str,
    source: &str,
    crate_type: CrateType,
) -> Result<PathBuf, ForgeError> {
    // Per-run scratch dir (the source + rustc's intermediate artifacts land here),
    // removed wholesale via the `ScratchDir` Drop guard on every exit path (#53).
    let scratch = ScratchDir {
        path: unique_scratch_dir(crate_name),
    };
    std::fs::create_dir_all(&scratch.path).map_err(|e| ForgeError::Io {
        path: scratch.path.display().to_string(),
        source: e,
    })?;
    // The `.rs` stem is `.`-free (the crate-name gotcha); we still pass
    // `--crate-name` explicitly (REQ-2).
    let rs_name = format!("{crate_name}.rs");
    let rs = scratch.path.join(&rs_name);
    std::fs::write(&rs, source).map_err(|e| ForgeError::Io {
        path: rs.display().to_string(),
        source: e,
    })?;

    // The artifact filename rustc produces for the crate type. For an rlib rustc
    // emits `lib<name>.rlib`; for a bin, `<name>`.
    let out_name = match crate_type {
        CrateType::Rlib => format!("lib{crate_name}.rlib"),
        CrateType::Bin => crate_name.to_string(),
    };
    let scratch_out = scratch.path.join(&out_name);

    let output = Command::new("rustc")
        .arg("--crate-name")
        .arg(crate_name)
        .arg("--edition")
        .arg(EDITION)
        .arg("--crate-type")
        .arg(crate_type.rustc_arg())
        // Pass the RELATIVE filename (cwd is the scratch dir) so the per-run
        // absolute scratch path is NOT baked into the artifact's debug metadata
        // (the path-nondeterminism that otherwise breaks byte-reproducibility,
        // REQ-5/AC-6). `--remap-path-prefix` pins the cwd to a stable `.` so even
        // the relative path resolves identically across runs.
        .arg(&rs_name)
        .arg("--remap-path-prefix")
        .arg(format!("{}=.", scratch.path.display()))
        .arg("-o")
        .arg(&scratch_out)
        // Reproducibility (REQ-5, §5.3): pin SOURCE_DATE_EPOCH so the archive
        // member-mtime is fixed, making the codegen reproducible modulo nothing.
        .env("SOURCE_DATE_EPOCH", SOURCE_DATE_EPOCH)
        .current_dir(&scratch.path)
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ForgeError::RustcAbsent {
                    binary: "rustc".to_string(),
                }
            } else {
                ForgeError::RustcSpawn { source: e }
            }
        })?;

    // R-CODE-4: a non-zero rustc exit is a structured error, never swallowed. A
    // contract-VIOLATING body still COMPILES (only the runtime check fires), so a
    // rustc failure here is a real lowering/codegen problem, surfaced with stderr.
    if !output.status.success() {
        return Err(ForgeError::RustcOutput {
            detail: format!(
                "rustc exited with status {:?}; stderr:\n{}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }

    // Copy the artifact OUT to a stable per-run output dir before `scratch` drops
    // (the Drop guard would otherwise take the artifact with it). The output dir is
    // a sibling per-run dir under the temp root (uniqueness via
    // `unique_scratch_dir`'s pid+counter scheme); the caller owns it. The SCRATCH
    // dir (source + rustc intermediates) is still cleaned wholesale.
    let out_dir = unique_scratch_dir(&format!("{crate_name}_out"));
    std::fs::create_dir_all(&out_dir).map_err(|e| ForgeError::Io {
        path: out_dir.display().to_string(),
        source: e,
    })?;
    let artifact = out_dir.join(&out_name);
    std::fs::copy(&scratch_out, &artifact).map_err(|e| ForgeError::Io {
        path: artifact.display().to_string(),
        source: e,
    })?;

    // `scratch` drops here, removing the `.rs` source + rustc intermediates
    // wholesale (#53). The copied-out artifact survives.
    drop(scratch);
    Ok(artifact)
}

/// Copy the freshly-built artifact `built` (the stable per-run /tmp path) to the
/// user-named `dest` (`forge build --out <PATH>`, REQ-7), OVERWRITING any existing
/// file (a build output is regenerable), mark it executable, and return `dest` as
/// the FINAL artifact path. This is a pure PLACEMENT step — the bytes are
/// identical to `built` (the verification/lowering are untouched), it only moves a
/// runnable binary out of the awkward `/tmp/..._build_out_<pid>/` dir to a real
/// `./<name>` the user runs directly. An unwritable `dest` (bad directory,
/// permission) is a structured `ForgeError::Io`, never a panic (R-CODE-2).
fn place_artifact(built: &Path, dest: &Path) -> Result<PathBuf, ForgeError> {
    // `std::fs::copy` overwrites the destination if it exists and preserves the
    // source's permission bits (so a `bin` stays executable). It does NOT create
    // missing parent directories — a `--out dir/that/does/not/exist/name` surfaces
    // the OS error as a structured `ForgeError::Io` (R-CODE-4), never a panic.
    std::fs::copy(built, dest).map_err(|e| ForgeError::Io {
        path: dest.display().to_string(),
        source: e,
    })?;

    // Ensure the placed artifact is executable so `./<dest>` runs directly (the #128
    // motivation). `std::fs::copy` preserves the source mode, but an `--out` over an
    // existing non-executable file (or a platform/umask quirk) could leave it
    // non-`+x`; set the owner/group/other execute bits explicitly on Unix. A
    // metadata/permission failure is a structured `ForgeError::Io` (R-CODE-2).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(dest)
            .map_err(|e| ForgeError::Io {
                path: dest.display().to_string(),
                source: e,
            })?
            .permissions();
        // OR-in the execute bits (rwxr-xr-x ∪ existing), preserving the read/write
        // bits the copy carried over. `0o111` = u+x,g+x,o+x.
        let mode = perms.mode() | 0o111;
        perms.set_mode(mode);
        std::fs::set_permissions(dest, perms).map_err(|e| ForgeError::Io {
            path: dest.display().to_string(),
            source: e,
        })?;
    }

    Ok(dest.to_path_buf())
}

/// Resolve the rustc version that the [`BuildManifest`] records as the pinned
/// toolchain identity (REQ-5, §5.3). Sourcing order (deterministic, R-CODE-5 — no
/// wall-clock), mirroring `check.rs::resolve_verus_version`:
///
/// 1. `RUSTC_VERSION` env var, when set — the pinned/CI override + the hermetic
///    test seam.
/// 2. otherwise `rustc --version` stdout (the live compiler's version).
///
/// rustc was already required to compile the artifact, so resolving the version
/// adds no requirement; a spawn ENOENT is still mapped to `RustcAbsent` (R-CODE-4).
fn resolve_rustc_version() -> Result<String, ForgeError> {
    if let Ok(pinned) = std::env::var("RUSTC_VERSION") {
        let pinned = pinned.trim().to_string();
        if !pinned.is_empty() {
            return Ok(pinned);
        }
    }
    let output = Command::new("rustc")
        .arg("--version")
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ForgeError::RustcAbsent {
                    binary: "rustc".to_string(),
                }
            } else {
                ForgeError::RustcSpawn { source: e }
            }
        })?;
    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if version.is_empty() {
        return Err(ForgeError::RustcOutput {
            detail: "`rustc --version` produced no version string (cannot record the pinned \
                     toolchain identity); set RUSTC_VERSION to pin it"
                .to_string(),
        });
    }
    Ok(version)
}
