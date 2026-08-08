//! `forge/src/build.rs` — `forge build [<file>] [--entry <fn>]`: lower a verified
//! Thermite program to executable Rust and compile it with `rustc` into a
//! contract-checked artifact. It is structurally `forge check` with the verus
//! backend swapped for rustc: it reuses `check_file`'s pipeline front
//! (`thermite_syntax::parse` → `thermite_spec::validate` →
//! `thermite_lower::check_effects`), then calls `thermite_lower::lower_l1` (the
//! always-active `thermite_check!` exec lowering) and invokes `rustc` instead of
//! `verus`. There is no new compiler: Thermite transpiles to Rust and rustc/LLVM
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
//! The scratch crate dir is a per-run [`check::ScratchDir`] removed wholesale on
//! every exit path (the #53 leak lesson; a compiled `.rlib`/binary is large). The
//! emitted artifact is copied out of the scratch dir to a stable per-run output
//! directory before the scratch dir is dropped, so the artifact survives cleanup.
//! `forge build --out <PATH>` (`-o`) then copies that artifact to a user-named path
//! (executable; REQ-7), so a built binary is a real `./<name>` you run directly,
//! with no `/tmp/forge_*_build_out_<pid>/` path / wrapper script (#128).
//!
//! ## Artifact form (REQ-3, OQ-1 decision (b))
//!
//! - Default → a compiled **library** (`--crate-type=rlib`) of the L1-checked fns.
//! - `forge build --entry <fn>` → appends a deterministic generated `main` that
//!   calls the entry fn with deterministic synthesized sample inputs per its param
//!   types (see [`synthesize_entry_main`]) and produces a runnable executable — the
//!   #57 hook (a binary a seccomp filter can be installed into; REQ-6).
//!
//! ## Reproducibility (REQ-5, §5.3)
//!
//! The `lower_l1` emission is byte-deterministic (forge owns this). `rustc` is
//! invoked with `SOURCE_DATE_EPOCH=0` pinned so the codegen is reproducible modulo
//! the residual `ar` archive member-mtime header (the caveat the [`BuildManifest`]
//! records; `.design/forge/build.md` AC-6).
//!
//! ## REQ status
//!
//! <!-- generated:reqs view=forge-build-core-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-FORGE-BUILD-ARTIFACT-FORM | shipped | `forge/src/build.rs` | Library and entry-runner artifact forms |  |
//! | REQ-FORGE-BUILD-ENTRY-SANDBOX | shipped | `forge/src/build.rs` | Entry binary sandbox hook |  |
//! | REQ-FORGE-BUILD-L1-CHECKS | shipped | `forge/src/build.rs` | L1 checks baked into build artifacts |  |
//! | REQ-FORGE-BUILD-MANIFEST | shipped | `forge/src/build.rs` | Build manifest with artifact, assurance, effects, and reproducibility |  |
//! | REQ-FORGE-BUILD-OUT-PATH | shipped | `forge/src/build.rs` | User-named build output path |  |
//! | REQ-FORGE-BUILD-PIPELINE | shipped | `forge/src/build.rs` | Build pipeline to rustc artifact |  |
//! | REQ-FORGE-BUILD-RUSTC | shipped | `forge/src/build.rs` | Checked rustc invocation and scratch cleanup |  |
//! <!-- /generated:reqs -->
//!
//! ## `.design/build/kernel-target.md` REQ status (`forge build --target kernel`, #197)
//!
//! <!-- generated:reqs view=forge-build-kernel-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-FORGE-BUILD-KERNEL-FX-REJECT | shipped | `forge/src/build.rs` | Kernel ambient syscall effect rejection |  |
//! | REQ-FORGE-BUILD-KERNEL-L1-CHECKS | shipped | `forge/src/build.rs` | Kernel profile preserves L1 runtime checks |  |
//! | REQ-FORGE-BUILD-KERNEL-L3-UNCHANGED | shipped | `forge/src/build.rs` | Kernel target leaves L3 verification unchanged |  |
//! | REQ-FORGE-BUILD-KERNEL-NOSTD | shipped | `forge/src/build.rs` | Kernel no_std plus alloc emission profile |  |
//! | REQ-FORGE-BUILD-KERNEL-TARGET | shipped | `forge/src/build.rs` | Kernel build target selection |  |
//! <!-- /generated:reqs -->

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
/// archive timestamp, and deterministic (R-CODE-5: an explicit input, not wall-clock).
const SOURCE_DATE_EPOCH: &str = "0";

/// The Rust edition every emitted crate is compiled under (mirrors
/// `l1_conformance.rs::compile_and_run` + `check.rs`'s verus invocation).
const EDITION: &str = "2021";

/// The L1 assurance statement (`.design/forge/build.md` REQ-4): `forge build`
/// builds any well-formed program; the runtime check is the assurance, not an SMT
/// proof. Not an L3 claim (R-DEFER-9).
const ASSURANCE_L1: &str = "L1 (built, runtime-checked)";

/// The codegen target profile `forge build --target` selects
/// (`.design/build/kernel-target.md` REQ-1). The default ([`BuildTarget::Std`])
/// is the unchanged hosted profile; [`BuildTarget::Kernel`] emits a freestanding
/// `no_std + alloc` library crate (no `main`, no seccomp, `panic=abort`) and
/// refuses ambient-syscall `fx` rows. A "target" is a rustc-invocation +
/// crate-prelude choice (rustc is the codegen backend — `thermite-design.md` §3);
/// the pipeline front and the L1 lowering are shared across targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildTarget {
    /// The default hosted profile: the std crate, seccomp sandbox available for an
    /// `--entry` runner, the existing `forge build` corpus byte-unchanged
    /// (`.design/build/kernel-target.md` AC-4).
    Std,
    /// The freestanding profile (REQ-1/REQ-2): a `#![no_std]` + `extern crate
    /// alloc;` library crate compiled `--crate-type=rlib -C panic=abort`, with no
    /// `main`/seccomp prelude, suitable for linking into a verified microkernel. An
    /// ambient-syscall `fx` row (`read`/`write`/`net`/`term`) is refused (REQ-3).
    Kernel,
}

/// The freestanding `#![no_std] + alloc` crate prelude prepended to `lower_l1`'s
/// output under [`BuildTarget::Kernel`] (REQ-2). `#![no_std]` drops the std
/// prelude; `extern crate alloc;` + `use alloc::vec::Vec;` resolves the bare `Vec`
/// the L1 collection wrappers (`TString { data: Vec<u8> }`, the `TVec*`/`TMap*`
/// runtime) spell (OQ-3: the L1 emission carries no `std::`-qualified path — the
/// `Vec`/`Vec::new()` spellings are bare prelude names, and the surface `String` is
/// the emitted `use TString as String;` alias, not `alloc::string::String`, so the
/// prelude must not re-import `String` (that would be a duplicate `as String`
/// import, `E0252`)). `panic!` is a core macro (no import needed); it routes to the
/// kernel host's `#[panic_handler]` under `panic=abort` (REQ-4, OQ-1). The
/// `#![allow(internal_features)]`-free, deterministic fixed string (R-CODE-5).
const KERNEL_PRELUDE: &str = "#![no_std]\nextern crate alloc;\nuse alloc::vec::Vec;\n\n";

/// The ambient-syscall effect verbs a [`BuildTarget::Kernel`] build refuses (REQ-3):
/// `read`/`write`/`net`/`term` carry a userspace syscall surface (the #57 seccomp
/// allowlist maps them to `openat`/`socket`/`ioctl`/…) with no kernel analogue, and
/// `time`/`rand` carry std-bodied effect wrappers (`effect_wrappers::WRAPPERS`
/// `os::now` = `std::time::SystemTime::now()` → `clock_gettime`; `getrandom`) which
/// `emit_mod_os` would emit into the `#![no_std]` crate (a raw `E0433` leak); a kernel
/// has no ambient clock/entropy. So a fn whose
/// transitive `fx` carries any of these cannot be a freestanding kernel library item.
/// The admit set is `pure`/`alloc`/`panic`/`diverge` (`.design/build/
/// kernel-target.md` OQ-2, amended by #198: the original "time/rand benign" premise
/// was falsified by the std-bodied `os::now` wrapper). Matched by the leading verb of
/// each `effects_of` token (`read(stdin)` → `read`), mirroring `sandbox::syscall_allowlist`.
const KERNEL_REJECTED_FX: &[&str] = &["read", "write", "net", "term", "time", "rand"];

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
/// sandbox is on by default for `--entry` (`--no-sandbox` opts out); the self-test
/// probe is injected only under `--sandbox-self-test`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SandboxConfig {
    /// Whether to inject the seccomp prelude (`SandboxMode::On` by default for
    /// `--entry`; `SandboxMode::Off` under `--no-sandbox`).
    pub mode: SandboxMode,
    /// Whether to inject the `--sandbox-self-test` `openat` probe after the prelude
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
    /// The installed host-architecture syscall allowlist (REQ-3), sorted ascending.
    /// Empty when no prelude was injected.
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

/// The reproducibility block (REQ-5, §5.3): the pinned toolchain identity, the
/// deterministic-source guarantee, and the archive-timestamp caveat
/// (`.design/forge/build.md` AC-6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Reproducibility {
    /// The resolved rustc version string (`rustc --version`, honoring a
    /// `RUSTC_VERSION` env pin). The §5.3 pinned-toolchain field — the bit
    /// reproducibility claim is "same toolchain → same codegen".
    pub rustc: String,
    /// The pinned `SOURCE_DATE_EPOCH` value passed to rustc (always `"0"`).
    pub source_date_epoch: String,
    /// The caveat: the emitted source is byte-identical and the codegen is
    /// reproducible modulo the `ar` archive member-mtime header (one byte; pinned
    /// via `SOURCE_DATE_EPOCH`).
    pub note: String,
}

/// The build record `forge build` emits alongside the artifact (REQ-5). A separate
/// struct that composes the `forge check` cert vocabulary (`effects_of`); it does
/// not mutate the frozen `Certificate` schema (R-SPEC-2, OQ-3 decision). Carries
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
    /// program; the always-active runtime `thermite_check!` is the assurance
    /// (`.design/forge/build.md` REQ-4), so this records the L1 statement
    /// `"L1 (built, runtime-checked)"`, not an L3 proof claim.
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
/// A front-of-pipeline failure short-circuits into a `ForgeError` as `check_file`
/// does; a non-zero rustc exit is `ForgeError::RustcOutput` (R-CODE-4). `forge
/// build` at L1 builds any well-formed program: a contract-violating body builds
/// and its `thermite_check!` fires at runtime (the oracle's `runtime_violation`
/// case).
pub fn build_file(
    path: impl AsRef<Path>,
    entry: Option<&str>,
    sandbox: SandboxConfig,
    out: Option<&Path>,
    target: BuildTarget,
) -> Result<BuildManifest, ForgeError> {
    let path = path.as_ref();
    let program = parse_program(path)?;

    // `.design/build/kernel-target.md` REQ-1/REQ-3: a kernel build is a library
    // (no `main`), so `--target kernel` + `--entry` is a usage error — a kernel
    // crate has no userspace process entry point / seccomp sandbox.
    if matches!(target, BuildTarget::Kernel) {
        if let Some(name) = entry {
            return Err(ForgeError::Usage(format!(
                "`forge build --target kernel` emits a no_std LIBRARY crate and takes no \
                 `--entry` (a kernel crate has no userspace `main`/seccomp surface); drop \
                 `--entry {name}` to build the kernel rlib"
            )));
        }
    }

    // REQ-3: under the kernel target, refuse any fn whose transitive `fx` carries an
    // ambient-syscall effect (`read`/`write`/`net`/`term`); kernel code has no
    // ambient userspace syscall surface (the `sandbox.rs` `fx`→syscall mapping is a
    // userspace seccomp concept). The reject is a named-effect, nonzero-exit,
    // no-artifact structured error before codegen, reusing the `sandbox::
    // transitive_fx` walk the #57 allowlist is derived from (read in reverse: where
    // the userspace target maps these to syscalls, the kernel target rejects them).
    // `pure`/`alloc`/`panic`/`diverge` are admitted; `time`/`rand` are rejected too
    // (#198: their std-bodied effect wrappers leak into `#![no_std]`; OQ-2 amended).
    // Every in-language fn is scanned (the whole class, not just an `--entry` closure) so
    // a library exporting an ambient-`fx` fn is refused regardless of call site.
    if matches!(target, BuildTarget::Kernel) {
        reject_ambient_fx_for_kernel(&program)?;
    }

    // #193/#195 open-hole refusal (`.design/forge/goal-repl.md` REQ-4/REQ-5;
    // `thermite-design.md` §6): a fn carrying any open body hole (`?N`) is
    // L0-equivalent (incomplete) and does not lower. Because a hole is recorded on
    // `FnItem.holes` (not a `Stmt` variant), lowering a holed body would drop the
    // open goal and emit a trust-stamped artifact for an incomplete
    // program (the §6 build manifest is the deliverable's trust statement). So
    // `build` refuses before `emit_source`/`lower_l1` (a structured error naming
    // the open hole(s), nonzero exit, no artifact), mirroring `check`'s `OpenHole`
    // reject (the shared `goal_repl::open_hole_reason`, the #192 single-copy lesson).
    if let Some(detail) = program.items.iter().find_map(|i| match i {
        Item::Fn(f) => crate::goal_repl::open_hole_reason(f),
        Item::Const(_) | Item::SpecFn(_) | Item::Struct(_) | Item::Enum(_) => None,
        // A Stage-1 forge-tier item (`.design/stage1-forge-tier.md` REQ-3 / AC-7):
        // an open `?pN` proof hole blocks the build, the proof-tier mirror of the
        // `?N` body-hole refusal — a holed proof must not ship a trust-stamped
        // artifact. Hole-free forge items contribute no reason (`None`).
        Item::Forge(forge) => crate::goal_repl::open_proof_hole_reason(forge),
    }) {
        return Err(ForgeError::Usage(format!(
            "`forge build` refuses a holed item: {detail} `forge build` lowers to a \
             trust-stamped artifact, so a holed body would silently drop the open goal \
             — fill every hole first (`forge fill`)."
        )));
    }

    // The full compiled source (lower_l1 + any --entry runner + the #57 sandbox
    // prelude) — the byte-deterministic emission the reproducibility check
    // (AC-6) asserts is stable (`emit_source` is this build's source-of-truth,
    // REQ-5).
    let source = emit_source(path, entry, sandbox, target)?;

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
    let built = invoke_rustc(&crate_name, &source, crate_type, target)?;

    // REQ-7 (`--out <PATH>`): the artifact lives at a stable per-run /tmp output
    // dir (`built`). When `--out <PATH>` is given, copy it to the user-named path
    // (overwriting — a build output), mark it executable, and report `<PATH>` as
    // the final artifact path. Without `--out`, the existing /tmp path is the
    // artifact (unchanged). The copy is a placement step: the artifact is
    // byte-identical (verification/lowering are untouched, R-CODE-5).
    let artifact = match out {
        Some(dest) => place_artifact(&built, dest)?,
        None => built,
    };

    // REQ-5/REQ-6: the build record — per-fn `fx` rows + the #57 sandbox record +
    // reproducibility. The sandbox is only installed for an `--entry` runner with
    // `SandboxMode::On`; a library build / `--no-sandbox` records `installed: false`
    // with an empty allowlist.
    let functions = build_functions(&program);
    let sandbox_record = match (entry, sandbox.mode) {
        (Some(name), SandboxMode::On) => {
            let fx = sandbox::transitive_fx(&program, name);
            let allowlist = sandbox::syscall_allowlist_for_host(&fx);
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

/// Run the shared `check_file` front (resolve → parse → validate →
/// check_effects) over `path`, returning the validated `Program` (REQ-1). Any
/// front-of-pipeline failure short-circuits into the earliest stage's
/// `ForgeError`, as `check_file` does. Used by both `emit_source` (the codegen)
/// and `build_file` (the manifest + entry-fn lookup) so the front is shared
/// verbatim. `path` is a single `.th` file or a canonical `.thpkg.json` manifest;
/// `thermite_package::load_source` owns that distinction for every
/// source-oriented verb.
fn parse_program(path: &Path) -> Result<Program, ForgeError> {
    let resolved = crate::thermite_package::load_source(path)?;
    let program = resolved.program();
    thermite_spec::validate(program).map_err(ForgeError::Spec)?;
    thermite_lower::check_effects(program).map_err(ForgeError::Effects)?;
    Ok(program.clone())
}

/// Emit the full compiled L1 source for `path` (incl. any `--entry` runner)
/// without compiling it (REQ-1/REQ-5/AC-6). This is `build_file`'s codegen
/// source-of-truth — `build_file` compiles these bytes, and the
/// reproducibility test asserts they are byte-identical across two calls (forge
/// owns the emission determinism, independent of any rustc nondeterminism). The
/// `--entry` runner is appended deterministically (`synthesize_entry_main`).
pub fn emit_source(
    path: impl AsRef<Path>,
    entry: Option<&str>,
    sandbox: SandboxConfig,
    target: BuildTarget,
) -> Result<String, ForgeError> {
    let path = path.as_ref();
    let program = parse_program(path)?;
    let lowered = thermite_lower::lower_l1(&program).map_err(ForgeError::Lower)?;

    // `.design/build/kernel-target.md` REQ-2: under the kernel target, prepend the
    // `#![no_std]` + `extern crate alloc;` + `use alloc::vec::Vec;` prelude (a crate
    // inner attribute must be the first token) before the lowered body. The std
    // default prepends nothing (the existing emission is byte-unchanged, AC-4). The
    // L1 body itself is emitted verbatim (REQ-4: the always-active `thermite_check!`/
    // `panic!` is `alloc`-clean — OQ-3 — and resolves against the prelude).
    let mut source = match target {
        BuildTarget::Std => String::new(),
        BuildTarget::Kernel => KERNEL_PRELUDE.to_string(),
    };

    // Basis Stage 8 (`.design/basis/08-runnable-effect-link.md` REQ-2): emit a
    // self-contained `mod os { … }` carrying the wrappers the program's
    // `#[boundary("os::<name>")]` targets name, prepended to the lowered code so
    // `lower_boundary_fn_l1`'s `let result = os::<name>(args);` crossing resolves
    // under raw rustc (closing the `E0433`). A program with no `os::` boundary emits
    // no module (the pure corpus is byte-unaffected, AC-7). Under the kernel target
    // the ambient-`fx` reject (REQ-3) guarantees no `read`/`write`/`net`/`term`
    // boundary survives, so `reachable_boundary_targets` is empty and `emit_mod_os`
    // emits nothing (no userspace syscall wrapper in a kernel crate).
    let targets = reachable_boundary_targets(&program);
    source.push_str(&effect_wrappers::emit_mod_os(&targets)?);
    source.push_str(&lowered);

    // REQ-2: a kernel build is a library — no `synthesize_entry_main` (no `main`, no
    // seccomp prelude). `build_file` already rejected `--target kernel` + `--entry`,
    // so `entry` is `None` here under the kernel target; the guard keeps the
    // invariant local and explicit.
    if let (Some(name), BuildTarget::Std) = (entry, target) {
        let f = find_entry_fn(&program, name)?;
        source.push_str(&synthesize_entry_main(&program, f, sandbox)?);
    }
    Ok(source)
}

/// Refuse a [`BuildTarget::Kernel`] build of `program` if any in-language `fn`'s
/// transitive `fx` carries an ambient-syscall effect (`read`/`write`/`net`/`term`,
/// [`KERNEL_REJECTED_FX`]) — REQ-3. Kernel code has no ambient userspace syscall
/// surface, so an item carrying one cannot be a freestanding kernel library item.
/// Reuses `sandbox::transitive_fx` (the #57 reachability walk the userspace seccomp
/// allowlist is derived from) per `Item::Fn`, scanning the whole function class
/// (not just an `--entry` closure — a kernel build has no `--entry`). Returns a
/// structured `ForgeError::Usage` naming the rejected effect + the carrying fn on
/// the first offender (source order, deterministic — R-CODE-5); `Ok(())` when every
/// fn is ambient-syscall-free (`pure`/`alloc`/`panic`/`diverge` admit; `time`/`rand`
/// are rejected — #198, their std-bodied effect wrappers leak into the `#![no_std]`
/// crate and a kernel has no ambient clock/entropy).
fn reject_ambient_fx_for_kernel(program: &Program) -> Result<(), ForgeError> {
    for item in &program.items {
        let Item::Fn(f) = item else { continue };
        let fx = sandbox::transitive_fx(program, &f.name);
        for tok in &fx {
            // Match the leading verb (before any `(`), mirroring
            // `sandbox::syscall_allowlist` so `read(stdin)` → `read`.
            let verb = tok.split('(').next().unwrap_or(tok);
            if KERNEL_REJECTED_FX.contains(&verb) {
                return Err(ForgeError::Usage(format!(
                    "`forge build --target kernel` refuses `{}`: its transitive effect row \
                     carries the ambient-syscall effect `{tok}` (a `{verb}` userspace syscall), \
                     which kernel code has no ambient surface for. The admitted kernel effects \
                     are pure/alloc/panic/diverge; build it for the default (std) target, or \
                     remove the `{verb}` effect.",
                    f.name
                )));
            }
        }
    }
    Ok(())
}

/// Collect the distinct `#[boundary("os::<name>")]` foreign targets the built
/// program names (Stage 8 REQ-2). `thermite_lower::lower_l1` emits a boundary L1
/// wrapper — containing the `os::<name>(args)` crossing — for every `#[boundary]`
/// `Item::Fn` in the program (`thermite-lower/src/l1.rs` `lower_l1`'s match guard),
/// so the self-contained crate must resolve each such target regardless of the
/// `--entry`. The set is keyed by the `BoundaryAttr.target` string (the foreign
/// target the lowered crossing calls), so the emitted `mod os` is the program's
/// live TCB surface, nothing more (minimal TCB, REQ-2/REQ-6). Returns a `BTreeSet`
/// (sorted, deterministic; R-CODE-5).
fn reachable_boundary_targets(program: &Program) -> BTreeSet<String> {
    program
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Fn(f) => f.boundary.as_ref().map(|b| b.target.clone()),
            Item::Const(_) | Item::SpecFn(_) | Item::Struct(_) | Item::Enum(_) => None,
            // Forge-tier item (stage1-forge-tier.md REQ-3): no v1 boundary-target
            // consumer yet (increments 2b-3); declares no boundary crossing (neutral
            // `None`), mirroring the inert ADT-decl arm.
            Item::Forge(_) => None,
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
            Item::Const(_) | Item::SpecFn(_) => None,
            // Forge-tier item (stage1-forge-tier.md REQ-3): no v1 manifest consumer
            // yet (increments 2b-3); carries no `fx` row → contributes no manifest
            // function (neutral `None`), mirroring the inert ADT-decl arm.
            Item::Forge(_) => None,
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
        Some(Item::Const(_)) => Err(ForgeError::Usage(format!(
            "`--entry {name}` names a compile-time capacity, not a runnable `fn`; name a `fn`"
        ))),
        // Basis Stage 1a (`.design/basis/01-adts.md`): a `struct`/`enum` type
        // is not a runnable entry point — the neutral value is the same `Usage`
        // refusal as a `spec fn` name. Dead-in-1a (the ADT program dies at the
        // validator before `forge build --entry` resolves an entry).
        Some(Item::Struct(_)) | Some(Item::Enum(_)) => Err(ForgeError::Usage(format!(
            "`--entry {name}` names a `struct`/`enum` type, not a runnable `fn`; name a `fn`"
        ))),
        // Forge-tier item (stage1-forge-tier.md REQ-3): no v1 entry-resolution consumer
        // yet (increments 2b-3); a forge item is not a runnable entry — the same
        // structured `Usage` refusal as a `struct`/`enum` name, mirroring the ADT arm.
        Some(Item::Forge(_)) => Err(ForgeError::Usage(format!(
            "`--entry {name}` names a forge-tier item (prop/lemma/proof/witness), not a \
             runnable `fn`; name a `fn`"
        ))),
        None => Err(ForgeError::Usage(format!(
            "`--entry {name}` names no `fn` in the program"
        ))),
    }
}

/// Synthesize a deterministic `fn main` that (under the #57 sandbox) installs the
/// seccomp prelude first, then calls the entry fn with fixed sample inputs per its
/// parameter types (REQ-3, R-CODE-5: no wall-clock / rand). The synthesis
/// convention (v0.1 fixed literals; a richer `--input` convention is future work,
/// OQ-1):
///
/// - `&[u32]` / `&[u64]` / `&[usize]` → `&[1, 2, 3]` (typed), the corpus `sum`
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
///     <openat self-test probe>   // --sandbox-self-test only; REQ-6
///     let r = entry(<args>);     // runs under the filter; the L1 thermite_check! still panics
///     println!("entry(args) = {r:?}");
/// }
/// ```
///
/// The seccomp prelude is the first statement(s) so the entry (and any boundary/slag
/// body it reaches) runs under the filter; the allowlist is the entry's transitive
/// `fx` projection (REQ-2/REQ-3). The result is `println!`'d so the runtime is
/// observable; for the `sum` corpus this prints `sum(&[1u32, 2, 3]) = 6` (the
/// oracle's `expect_run_contains: "6"`). A parameter type with no deterministic
/// synthesis is a structured error (R-CODE-2), not a panic.
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

    // REQ-1/REQ-4: the #57 seccomp prelude is the first statement of `main` (so the
    // entry runs under the filter), with the allowlist derived from the entry's
    // transitive `fx` (REQ-2/REQ-3). `SandboxMode::Off` (`--no-sandbox`) emits none.
    let prelude = match sandbox.mode {
        SandboxMode::On => {
            let fx = sandbox::transitive_fx(program, &f.name);
            sandbox::emit_sandbox_prelude(&fx)
        }
        SandboxMode::Off => String::new(),
    };
    // REQ-6: the `--sandbox-self-test` probe is injected after the prelude (so the
    // filter is already installed) and before the entry call (so the kill is
    // observed before any entry output). Never emitted without the flag.
    let probe = if sandbox.self_test {
        sandbox::emit_probe()
    } else {
        String::new()
    };

    // The runner binds the result and prints it; the `thermite_check!`s inside the
    // fn fire before the tail returns on a violation (REQ-4), and the baseline
    // allowlist permits that panic/abort path, so a contract violation panics rather
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
        Type::Prim(PrimType::U8) => Some("1u8".to_string()),
        Type::Prim(PrimType::U16) => Some("1u16".to_string()),
        Type::Prim(PrimType::U32) => Some("1u32".to_string()),
        Type::Prim(PrimType::U64) => Some("1u64".to_string()),
        Type::Prim(PrimType::Usize) => Some("1usize".to_string()),
        Type::Prim(PrimType::Bool) => Some("true".to_string()),
        // `&[T]` (a `Ref` of a `Slice`): a fixed three-element typed slice. The
        // corpus `sum(&[1,2,3]) == 6` is this case.
        Type::Ref { inner, .. } => match inner.as_ref() {
            Type::Slice(elem) => match elem.as_ref() {
                Type::Prim(PrimType::U8) => Some("&[1u8, 2, 3]".to_string()),
                Type::Prim(PrimType::U16) => Some("&[1u16, 2, 3]".to_string()),
                Type::Prim(PrimType::U32) => Some("&[1u32, 2, 3]".to_string()),
                Type::Prim(PrimType::U64) => Some("&[1u64, 2, 3]".to_string()),
                Type::Prim(PrimType::Usize) => Some("&[1usize, 2, 3]".to_string()),
                _ => None,
            },
            // `&u32` etc. — a referenced scalar.
            Type::Prim(PrimType::U8) => Some("&1u8".to_string()),
            Type::Prim(PrimType::U16) => Some("&1u16".to_string()),
            Type::Prim(PrimType::U32) => Some("&1u32".to_string()),
            Type::Prim(PrimType::U64) => Some("&1u64".to_string()),
            Type::Prim(PrimType::Usize) => Some("&1usize".to_string()),
            Type::Prim(PrimType::Bool) => Some("&true".to_string()),
            _ => None,
        },
        _ => None,
    }
}

/// Compute a valid Rust crate name from a source path (REQ-2 / the dotted-filename
/// gotcha). Mirrors `check.rs::crate_stem`: the file stem with every
/// non-alphanumeric char replaced by `_`, a leading-digit guard, suffixed
/// `_build`. rustc derives the crate name from the file stem and rejects a `.`
/// (the `*.l1.rs` gotcha), so we always pass `--crate-name` and keep the `.rs`
/// filename stem `.`-free.
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
/// pinned), check the exit status, copy the artifact out of the scratch dir to a
/// stable per-run output dir, and let the [`ScratchDir`] Drop guard remove the
/// scratch dir wholesale on every exit path (REQ-2, #53). Returns the stable
/// artifact path. A spawn ENOENT → `ForgeError::RustcAbsent`; a non-zero exit →
/// `ForgeError::RustcOutput` (R-CODE-4).
fn invoke_rustc(
    crate_name: &str,
    source: &str,
    crate_type: CrateType,
    target: BuildTarget,
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
    // The canonical scratch path (REQ-5/AC-6, byte-reproducibility). On macOS the
    // temp root `/var/folders/...` is reached through the `/var → /private/var`
    // symlink, and rustc records the canonical cwd (`/private/var/...`, the
    // DW_AT_comp_dir) in the artifact. A `--remap-path-prefix` keyed only on the
    // non-canonical `scratch.path` (`/var/...`) therefore silently MISSES, and the
    // per-run, PID-bearing absolute path leaks into the rlib — so two same-input
    // builds differ (the `rebuilt_library_is_byte_identical` failure). We remap the
    // canonical form too; on Linux `canonicalize` is a no-op (no `/var` symlink), so
    // this stays a single portable code path. A canonicalize failure falls back to
    // the non-canonical path (no worse than before).
    let canonical_scratch =
        std::fs::canonicalize(&scratch.path).unwrap_or_else(|_| scratch.path.clone());
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

    let mut command = Command::new("rustc");
    command
        .arg("--crate-name")
        .arg(crate_name)
        .arg("--edition")
        .arg(EDITION)
        .arg("--crate-type")
        .arg(crate_type.rustc_arg())
        // Pass the relative filename (cwd is the scratch dir) so the per-run
        // absolute scratch path is not baked into the artifact's debug metadata
        // (the path-nondeterminism that otherwise breaks byte-reproducibility,
        // REQ-5/AC-6). `--remap-path-prefix` pins the cwd to a stable `.` so even
        // the relative path resolves identically across runs.
        .arg(&rs_name)
        .arg("--remap-path-prefix")
        .arg(format!("{}=.", scratch.path.display()))
        // Also remap the canonical scratch path: on macOS rustc records the
        // `/private/var/...` form, which the non-canonical remap above misses
        // (REQ-5/AC-6 — the byte-reproducibility fix). On Linux this equals the
        // line above (a harmless duplicate).
        .arg("--remap-path-prefix")
        .arg(format!("{}=.", canonical_scratch.display()))
        .arg("-o")
        .arg(&scratch_out)
        // Reproducibility (REQ-5, §5.3): pin SOURCE_DATE_EPOCH so the archive
        // member-mtime is fixed, making the codegen reproducible modulo nothing.
        .env("SOURCE_DATE_EPOCH", SOURCE_DATE_EPOCH)
        .current_dir(&scratch.path);

    // `.design/build/kernel-target.md` REQ-2: a freestanding crate cannot unwind, so
    // pin `-C panic=abort` under the kernel target (`--edition`/`--crate-name`/
    // `SOURCE_DATE_EPOCH`/`--remap-path-prefix` are target-independent, unchanged).
    // The std default adds nothing here, so the existing build is byte-unchanged
    // (AC-4). The kernel `#![no_std]` rlib needs no `#[panic_handler]`/allocator to
    // compile (only a final bin/staticlib link does — OQ-1; the test harness supplies
    // a stub for the freestanding-compile AC).
    if matches!(target, BuildTarget::Kernel) {
        command.arg("-C").arg("panic=abort");
    }

    let output = command.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ForgeError::RustcAbsent {
                binary: "rustc".to_string(),
            }
        } else {
            ForgeError::RustcSpawn { source: e }
        }
    })?;

    // R-CODE-4: a non-zero rustc exit is a structured error. A
    // contract-violating body still compiles (only the runtime check fires), so a
    // rustc failure here is a lowering/codegen problem, surfaced with stderr.
    if !output.status.success() {
        return Err(ForgeError::RustcOutput {
            detail: format!(
                "rustc exited with status {:?}; stderr:\n{}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }

    // Copy the artifact out to a stable per-run output dir before `scratch` drops
    // (the Drop guard would otherwise take the artifact with it). The output dir is
    // a sibling per-run dir under the temp root (uniqueness via
    // `unique_scratch_dir`'s pid+counter scheme); the caller owns it. The scratch
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
/// user-named `dest` (`forge build --out <PATH>`, REQ-7), overwriting any existing
/// file (a build output is regenerable), mark it executable, and return `dest` as
/// the final artifact path. This is a placement step: the bytes are identical to
/// `built` (the verification/lowering are untouched), it only moves a runnable
/// binary out of the `/tmp/..._build_out_<pid>/` dir to a real `./<name>` the user
/// runs directly. An unwritable `dest` (bad directory, permission) is a structured
/// `ForgeError::Io`, never a panic (R-CODE-2).
fn place_artifact(built: &Path, dest: &Path) -> Result<PathBuf, ForgeError> {
    // `std::fs::copy` overwrites the destination if it exists and preserves the
    // source's permission bits (so a `bin` stays executable). It does not create
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

#[cfg(test)]
mod tests {
    use super::*;

    fn corpus(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("conformance")
            .join(name)
    }

    /// #198 (critic audit-note gap): no test inspected forge's emitted kernel source
    /// — only `kernel_target.rs::reconstruct_kernel_source` (an independent N-version
    /// reconstruction). Close the gap directly against `emit_source`: the kernel
    /// emission for the pure corpus item `sum.th` must carry the design-pinned
    /// `#![no_std]` prelude (`.design/build/kernel-target.md` REQ-2:
    /// [`KERNEL_PRELUDE`]) and not a `std::`-qualified path / `fn main` (a pure lib).
    #[test]
    fn kernel_emit_source_carries_no_std_prelude() -> Result<(), ForgeError> {
        let source = emit_source(
            corpus("sum.th"),
            None,
            SandboxConfig::default(),
            BuildTarget::Kernel,
        )?;

        // The actual emission begins with the design-pinned freestanding prelude.
        assert!(
            source.starts_with("#![no_std]"),
            "forge's actual kernel emission must START with `#![no_std]` (a crate inner \
             attribute is the first token):\n{source}"
        );
        assert!(
            source.contains("extern crate alloc;"),
            "the kernel emission must carry `extern crate alloc;`:\n{source}"
        );
        assert!(
            source.contains("use alloc::vec::Vec;"),
            "the kernel emission must import the bare `Vec` from the alloc prelude:\n{source}"
        );
        // A pure (boundary-free) library: no `mod os` userspace wrapper, no `main`, no
        // `std::`-qualified leak (the #198 divergence class).
        assert!(
            !source.contains("std::"),
            "the kernel emission must carry NO `std::`-qualified path (REQ-2/OQ-3):\n{source}"
        );
        assert!(
            !source.contains("fn main"),
            "a kernel LIBRARY emits no `fn main`:\n{source}"
        );
        Ok(())
    }
}
