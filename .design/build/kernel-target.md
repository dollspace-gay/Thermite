# Freestanding `no_std` kernel build target — `forge build --target kernel`

<!--
tier: 3-component
status: draft
governs: forge/src/build.rs
thesis-refs:
  - thermite-design.md §3 (the stack — transpile to Rust, rustc is the codegen backend; the #21 realization note)
  - thermite-design.md §6 (the verification ladder; L1 always-active runtime checks; L3 = Verus-derived SMT proof)
  - thermite-design.md §13 (v0.1 kernel scope; the forward-looking verified-microkernel convergence)
epic: crosslink #169
sibling-groundwork: .design/verified/exec-stmt-tv.md (crosslink #158 — the kernel exec-language freeze; "Kernel convergence" note)
blocker: #164
-->

## Summary

`forge build --target kernel` emits a freestanding `no_std + alloc` Verus-Rust **library** crate
(`--crate-type=rlib`, no `main`, no seccomp sandbox, `panic=abort`) suitable for linking into a
verified microkernel. It is a NEW fork of the existing `forge build` verb (`build_file` /
`emit_source` / `invoke_rustc` in `build.rs`): the L1 lowering and the verification (L3) path are
**target-independent** (Verus and `thermite_lower::lower_l1` are the same), so the kernel target
changes only the EMISSION PROFILE (the crate prelude + the rustc invocation) and adds one new
**reject**: an `fx` row carrying an ambient-syscall effect (`read`/`write`/`net`/`term`) is refused,
because kernel code has no ambient userspace syscall surface.

This component is currently UNBUILT — every REQ below is NOT-STARTED behind the single build blocker
(below). The doc is the contract the builder edits `build.rs` against; it documents the v1 scope
decisions PRE-MADE in crosslink #164's plan comment and GROUNDS each against the existing build/lower/
sandbox code.

## v1 scope decisions (pre-made in crosslink #164; grounded here)

- **`no_std + alloc` library crate.** v1 emits a `#![no_std]` lib with `extern crate alloc;` — NOT a
  `no_std` with collection types rejected. Grounded by the emitted std surface (below): the L1 lib
  body's only "std" dependencies are `panic!` (the `thermite_contract_violation` handler) and
  `Vec`/`String` (the `TString`/`TVec`/`TMap` runtime wrappers), and `Vec`/`String`/`format!`/`panic!`
  are ALL in the `alloc`/core prelude. So `alloc`-only is the SMALLEST honest scope that keeps the
  shipped collection lowerings working — rejecting collections would gratuitously shrink the kernel
  exec language below what the lowerer already emits.
- **No `main`, no seccomp.** The kernel target NEVER takes `--entry` (no generated `main`, no
  `synthesize_entry_main`), so the entire `forge/src/sandbox.rs` machinery (the seccomp BPF prelude,
  the `SandboxConfig`/`SandboxRecord`) is NOT emitted — kernel code is not a sandboxed userspace
  process. `--target kernel` + `--entry` is a usage error.
- **`panic=abort`.** A freestanding crate cannot unwind; the rustc invocation pins `-C panic=abort`
  (and the kernel host supplies the `#[panic_handler]` / `#[global_allocator]` — they are NOT emitted
  by forge, see OQ-1).
- **REJECT ambient-syscall `fx` rows.** A fn whose transitive `fx` carries `read`/`write`/`net`/`term`
  is refused with a structured `ForgeError` BEFORE codegen — kernel code has no ambient userspace
  syscall (the `read`/`write`/`net`/`term` → syscall-allowlist mapping in `sandbox.rs` is a USERSPACE
  seccomp concept with no kernel analogue). `pure`/`alloc`/`panic`/`diverge`/`time`/`rand` are NOT
  rejected in v1 (they carry no userspace-syscall semantics in the kernel emission; OQ-2).
- **L3 path IDENTICAL.** `forge check` (the Verus L3 proof) is target-independent — Verus verifies the
  SAME lowered program regardless of the eventual codegen target. `--target kernel` touches ONLY
  `forge build` (the rustc codegen side), never `forge check`. (`.design/verified/exec-stmt-tv.md`
  "Kernel convergence" note: 2.2.1/2.2.2 are TV over the SAME lowering target as the rest of the
  toolchain.)

## The emitted std surface (grounded — why `alloc`-only is honest)

`thermite_lower::lower_l1` (`thermite-lower/src/l1.rs`, `pub fn lower_l1`) is the emission the kernel
target reuses VERBATIM. Its std footprint in the EMITTED Rust (not forge's own code):

- **`panic!`** — `emit_check_macro` emits `fn thermite_contract_violation(kind, text) -> ! {
  panic!("thermite L1 contract violation [{kind}]: {text}"); }`, and `thermite_check!` is a plain
  `if !($cond) { thermite_contract_violation(...) }`. `panic!` is a CORE macro (available under
  `#![no_std]`); it routes to the crate's `#[panic_handler]`. So the L1 checks FIRE in a kernel build
  exactly as in a std build — the panic lands on the kernel host's panic handler / `panic=abort`
  rather than std's unwinder. (See the L1-checks decision below.)
- **`Vec` / `String`** — `emit_string_runtime_l1` emits `struct TString { data: Vec<u8> }` and
  `emit_vec_runtime_l1`/`emit_map_runtime_l1` emit the `TVec*`/`TMap*` wrappers over `Vec`; the doc
  comment calls these "PLAIN-Rust `TString` over `std::vec::Vec<u8>`" but the spellings used are the
  bare prelude names `Vec` / `Vec::new()` / `String` (e.g. l1.rs `struct TString { data: Vec<u8> }`,
  `fn new() -> TString { TString { data: Vec::new() } }`). `Vec`, `String`, and `format!` are all in
  the `alloc` prelude, so `extern crate alloc;` + `use alloc::vec::Vec; use alloc::string::String;`
  (or the equivalent prelude) resolves them under `#![no_std]`. (The `std::vec::Vec` PROSE in l1.rs is
  a comment; the emitted CODE uses bare `Vec` — the builder must confirm no `std::`-qualified path is
  emitted in a collection lowering, see OQ-3.)
- **`println!` / `std::process::abort` / `extern "C"` seccomp** — emitted ONLY by the `--entry` runner
  (`build::synthesize_entry_main`) and the seccomp prelude (`sandbox::emit_sandbox_prelude`:
  `eprintln!`, `std::process::abort()`). These are STD-ONLY but are NOT emitted by the kernel target
  (no `main`, no sandbox), so they do not constrain the lib's `no_std`-ness.

Conclusion: the L1 LIBRARY body is `alloc`-clean; the std-only emissions are exactly the
`main`/sandbox machinery the kernel target drops. `alloc`-only is the honest minimal scope.

## The fork point in `build.rs`

The existing build is `pub fn build_file(path, entry, sandbox, out)` → `emit_source(path, entry,
sandbox)` (which calls `thermite_lower::lower_l1` + `effect_wrappers::emit_mod_os` + optional
`synthesize_entry_main`) → `invoke_rustc(crate_name, source, crate_type)`. `--target kernel` forks at
three named seams (the builder adds a `BuildTarget` enum — `Std` (default) | `Kernel` — threaded
through these):

1. **`build_file` — reject `--entry` + ambient `fx`.** Before `emit_source`, a kernel build with
   `entry.is_some()` is a `ForgeError::Usage`; and each fn's transitive `fx` (reusing
   `sandbox::transitive_fx` / `manifest::effects_of`) is scanned for `read`/`write`/`net`/`term` →
   structured refusal (the `reachable_boundary_targets` + `build_functions` projections already walk
   the program; the reject reuses that walk). NO seccomp (`sandbox_record` is `installed: false`,
   or the kernel manifest omits it).
2. **`emit_source` — the kernel prelude.** PREPEND the `#![no_std]` / `extern crate alloc;` /
   `use alloc::{vec::Vec, string::String};` prelude to `lower_l1`'s output instead of (or in addition
   to) the `effect_wrappers::emit_mod_os` block; emit NO `synthesize_entry_main`. (`emit_mod_os` is the
   `os::<name>` userspace-syscall wrapper module — a kernel build with no ambient-syscall `fx` reaches
   it with an EMPTY target set, so it emits nothing; the ambient-`fx` reject above guarantees this.)
3. **`invoke_rustc` — `panic=abort` + `--crate-type=rlib`.** Add `-C panic=abort` to the rustc
   `Command`; force `CrateType::Rlib` (a kernel target is never `Bin`). `--edition 2021` /
   `--crate-name` / `SOURCE_DATE_EPOCH=0` / `--remap-path-prefix` are unchanged (reproducibility is
   target-independent, REQ-5 of `.design/forge/build.md`).

## Requirements

- **REQ-1 (`--target kernel` verb fork)** — `forge build --target kernel <file>` selects the kernel
  emission profile: a `BuildTarget` (`Std`/`Kernel`) threaded `cli::run_build` → `build::build_file` →
  `emit_source` → `invoke_rustc`. The default (`Std`) is byte-unchanged (the existing `forge build`
  corpus is unaffected). Derived from `thermite-design.md` §3 (rustc is the codegen backend; the
  target is a codegen choice) + §13 (the v0.1 kernel scope). **Blocker #164.**
- **REQ-2 (`no_std + alloc` emission profile)** — the kernel build emits a `#![no_std]` lib crate with
  `extern crate alloc;` + the `alloc` collection prelude, REUSING `thermite_lower::lower_l1`'s output
  verbatim (the L1 checks + `TString`/`TVec`/`TMap` wrappers resolve against `alloc`). No `main`, no
  `synthesize_entry_main`, no seccomp prelude. Compiled `--crate-type=rlib -C panic=abort`. Derived
  from §3 + the emitted-std-surface grounding above. **Blocker #164.**
- **REQ-3 (ambient-syscall `fx` reject)** — a fn whose transitive `fx` (via `sandbox::transitive_fx` /
  `manifest::effects_of`) carries `read`/`write`/`net`/`term` is refused with a structured
  `ForgeError` (a NAMED-effect, nonzero-exit, NO-artifact reject) BEFORE codegen — kernel code has no
  ambient userspace syscall surface. `--target kernel` + `--entry` is likewise a usage error. Derived
  from §13 (kernel scope) + the `sandbox.rs` `fx`→syscall mapping being a USERSPACE concept. **Blocker
  #164.**
- **REQ-4 (L1 runtime checks in the kernel profile)** — the always-active `thermite_check!` /
  `thermite_contract_violation` (`panic!`) is emitted UNCHANGED; under `#![no_std]` / `panic=abort` it
  routes to the kernel host's `#[panic_handler]` rather than std's unwinder. The L1 assurance rung
  (§6) is PRESERVED: a contract violation aborts the kernel-linked code, it is not silently dropped.
  Forge does NOT emit the `#[panic_handler]` / `#[global_allocator]` (the kernel host supplies them —
  OQ-1). Derived from §6 (L1 always-active checks). **Blocker #164.**
- **REQ-5 (L3 verification path identical)** — `forge check` (the Verus L3 proof) is UNTOUCHED by
  `--target kernel`: Verus verifies the same lowered program regardless of codegen target. The kernel
  target is a `forge build` (rustc) concern only. Derived from `thermite-design.md` §3/§6 +
  `.design/verified/exec-stmt-tv.md` "Kernel convergence" note (TV/verification is over the SAME
  lowering target). **Blocker #164.**

## Acceptance criteria

- **AC-1 (pure fn → kernel rlib compiles `no_std`)** — `forge build --target kernel sum.th` (the
  corpus `sum`, `fx pure`) emits a `#![no_std]` + `extern crate alloc;` rlib and rustc exits 0
  (`--crate-type=rlib -C panic=abort`), producing `libsum_build.rlib`. The emitted source contains
  `#![no_std]` and `extern crate alloc;` and NO `fn main` / NO seccomp prelude. (Verification: a
  `forge/tests/kernel_target.rs` freestanding-compile check shelling the real `rustc` with the kernel
  profile flags.)
- **AC-2 (ambient-`fx` fn → structured refusal)** — `forge build --target kernel` on a fn carrying
  `fx read(src)` (the oracle `rf` shape) returns a `ForgeError` naming the rejected effect, nonzero
  exit, NO artifact — NOT a silent build. A `fx write`/`net`/`term` fn refuses identically; a `fx
  pure`/`alloc`/`diverge` fn builds. (Verification: a `kernel_target.rs` reject case + a `pure`/`alloc`
  accept case.)
- **AC-3 (L1 checks fire in the kernel profile — documented)** — the kernel rlib's emitted source
  carries the always-active `thermite_check!` / `thermite_contract_violation` (`panic!`) verbatim
  (NOT stripped, NOT `debug_assert!`); under `panic=abort` a violation aborts. v1 GROUNDS this as
  "the L1 check is emitted and `panic!` is core/no_std-valid"; whether the abort is observable is the
  kernel host's `#[panic_handler]` responsibility (OQ-1). The check-emission is asserted present in
  the kernel source (a string/structural assertion), and the lib COMPILES with a test
  `#[panic_handler]` supplied by the test harness. (Verification: `kernel_target.rs` asserts the
  emitted source contains `thermite_check` + `panic!`, and the freestanding compile uses a stub
  panic handler.)
- **AC-4 (default target byte-unchanged)** — `forge build sum.th` (no `--target`) emits the SAME bytes
  as before (the std profile is the unchanged default); the existing `conformance/build` oracle +
  `build_conformance::sum_runs` still pass. (Verification: the existing build conformance suite is
  unchanged.)
- **AC-5 (L3 path untouched)** — `forge check sum.th` is byte-identical with and without the kernel
  build feature present; the kernel target adds NO route or change to `forge/src/check.rs` or the
  lowering's L3 path. (Verification: the existing `forge/tests` check suite is unchanged; no
  `check.rs` edit in the increment.)

## Architecture

The kernel target is a CODEGEN-PROFILE fork of the shipped `forge build` (`build.rs`), not a new
pipeline. The pipeline FRONT (`parse_program` — `thermite_syntax::parse` → `thermite_spec::validate`
→ `thermite_lower::check_effects`) and the L1 lowering (`thermite_lower::lower_l1`) are SHARED with
the std target and with `forge check`; §3's realization note (rustc is the codegen backend) is why a
"target" is purely a rustc-invocation + crate-prelude choice, not a compiler change.

The three fork seams are `pub fn build_file`, `pub fn emit_source`, and `invoke_rustc` (all in
`build.rs`; see "The fork point" above). The ambient-`fx` reject reuses `sandbox::transitive_fx`
(itself reusing `closure::reachable_in_file_fns` + `manifest::effects_of`) — the SAME transitive-`fx`
walk the #57 seccomp allowlist is derived from (`forge/src/sandbox.rs` `pub fn transitive_fx`), read
in REVERSE: where the userspace target MAPS `read`/`write`/`net`/`term` to syscall numbers, the kernel
target REJECTS them. The seccomp emission (`sandbox::emit_sandbox_prelude`, `SandboxConfig`,
`SandboxRecord`) is NOT reached — a kernel build is a library (no `--entry`), and `synthesize_entry_main`
(the only injection point) is never called.

The `no_std` prelude is a fixed string prepended in `emit_source` (parallel to the existing
`effect_wrappers::emit_mod_os` prepend); `emit_mod_os` emits the `os::<name>` USERSPACE syscall
wrappers and, because the ambient-`fx` reject (REQ-3) guarantees no `read`/`write`/`net`/`term`
boundary survives, reaches the kernel build with an empty target set (it emits nothing — the pure
corpus is byte-unaffected, `build.rs` `reachable_boundary_targets`). The `-C panic=abort` +
`--crate-type=rlib` are added to the `invoke_rustc` `Command` under the kernel target; `--edition`,
`--crate-name`, `SOURCE_DATE_EPOCH`, `--remap-path-prefix` are target-independent and unchanged
(reproducibility, REQ-5 of `.design/forge/build.md`).

## Verification

The increment ships a `forge/tests/kernel_target.rs` conformance test shelling the REAL `rustc`
(mirroring `build_conformance.rs` / `l1_conformance.rs::compile_and_run`):

- AC-1: `sum.th` → kernel rlib, rustc exit 0, emitted source asserted to contain `#![no_std]` /
  `extern crate alloc;` and NOT `fn main` / NOT `PR_SET_SECCOMP`.
- AC-2: a `fx read(src)` fn → `ForgeError` naming `read`, no artifact; a `fx pure`/`alloc` fn builds.
- AC-3: the emitted kernel source contains `thermite_check` + `panic!`; the freestanding compile
  links a test-supplied `#[panic_handler]` (the kernel-host stand-in) so the `no_std` rlib genuinely
  compiles.
- AC-4/AC-5: the EXISTING `build_conformance` + `forge check` suites are unchanged (the std default
  and the L3 path are byte-stable).

Crate gauntlet (when built): `cargo test -p forge` (incl. `kernel_target`), `cargo clippy -p forge
--all-targets -- -D warnings`, `cargo fmt --check`. Scratch/rustc temp cleaned per the `ScratchDir`
Drop guard (`build.rs` `invoke_rustc`, #53).

## Increment plan (ONE builder increment)

A single builder dispatch delivers the whole v1:

1. **The verb fork** — a `BuildTarget` enum (`Std`/`Kernel`) + the `--target kernel` CLI flag
   (`cli.rs` `Command::Build` + `run_build`), threaded into `build::build_file`. `--target kernel` +
   `--entry` is a usage error (REQ-1/REQ-3).
2. **The `no_std` emission profile** — the kernel prelude prepend in `emit_source`, the
   `synthesize_entry_main`/seccomp suppression, and the `-C panic=abort` + forced-`rlib` in
   `invoke_rustc` (REQ-2/REQ-4). The ambient-`fx` reject in `build_file` reusing `transitive_fx`
   (REQ-3).
3. **The freestanding compile-check test** — `forge/tests/kernel_target.rs` (AC-1..AC-3) +
   confirming the std default + L3 path are byte-stable (AC-4/AC-5).

At manifest time the builder ADDS the spec-route below (the convention for a new governed change site
in `build.rs` — `build.rs` already carries multiple routes: `build.md`, `08-runnable-effect-link.md`).

## Open questions (for the builder / critic)

- **OQ-1 (panic handler / global allocator)** — v1 does NOT emit `#[panic_handler]` /
  `#[global_allocator]` (the kernel HOST supplies them; an rlib needs neither to COMPILE under
  `no_std`, only a final `bin`/`staticlib` link does — the test harness supplies a stub). Whether a
  future `--target kernel-bin` profile emits a default abort handler is OUT of v1.
- **OQ-2 (non-ambient effect atoms)** — `time`/`rand` carry userspace-syscall semantics in the std
  seccomp table (`clock_gettime`/`getrandom`) but are NOT in the v1 reject set (the plan names only
  `read`/`write`/`net`/`term`). v1 follows the plan literally; the critic should decide whether
  `time`/`rand` ALSO warrant a kernel reject (a kernel has no `getrandom` syscall either) or are
  benign because the kernel emission carries no syscall mapping at all.
- **OQ-3 (`std::`-qualified paths in collection lowerings)** — the l1.rs PROSE says "`std::vec::Vec`"
  but the emitted CODE spellings observed are bare `Vec`/`Vec::new()`/`String`. The builder must
  confirm NO `std::`-qualified path is emitted in any reachable collection/string lowering (a
  `std::`-qualified spelling would break `#![no_std]` even with `extern crate alloc`); if one exists,
  it is a NOT-STARTED blocker against the lowerer, NOT a kernel-target code change (R-DOC-1 — the doc
  adapts to the code).

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (`--target kernel` verb fork) | SHIPPED | `build.rs` `enum BuildTarget { Std, Kernel }` threaded `cli::run_build` → `build::build_file` → `emit_source` → `invoke_rustc`; `cli.rs` parses `--target std\|kernel` (default `Std`, unknown/missing value → `Usage`). Consumer: `cli::run_build`. Verified by `cli::tests::parses_build_target_flag` + `forge/tests/kernel_target.rs::pure_fn_builds_no_std_kernel_rlib`; the std default is byte-unchanged (`default_target_source_is_byte_identical_to_no_target_flag` + the unaffected `build_conformance` suite, AC-4). |
| REQ-2 (`no_std + alloc` emission profile) | SHIPPED | `emit_source` prepends `KERNEL_PRELUDE` (`#![no_std]` + `extern crate alloc;` + `use alloc::vec::Vec;`) under `BuildTarget::Kernel`, reuses `lower_l1`'s output VERBATIM, emits NO `synthesize_entry_main`; `invoke_rustc` forces `--crate-type=rlib` + `-C panic=abort`. Consumer: `cli::run_build`. Verified by `kernel_target.rs::pure_fn_builds_no_std_kernel_rlib` (rustc exit 0 + reconstructed-source freestanding compile) + `pure_and_alloc_fx_fns_build_for_kernel`. |
| REQ-3 (ambient-syscall `fx` reject) | SHIPPED | `reject_ambient_fx_for_kernel` scans EVERY `Item::Fn`'s `sandbox::transitive_fx` for `KERNEL_REJECTED_FX = ["read","write","net","term"]` → a NAMED-effect `ForgeError::Usage` (nonzero exit, NO artifact) BEFORE codegen; `--target kernel` + `--entry` is likewise a `ForgeError::Usage`. Consumer: `build_file`. Verified by `kernel_target.rs::ambient_read_fx_fn_is_refused` + `ambient_write_net_term_fx_refuse_identically` + `kernel_target_with_entry_is_usage_error`; `pure`/`alloc`/`time`/`rand`/`panic`/`diverge` admit (OQ-2; `pure_and_alloc_fx_fns_build_for_kernel`). |
| REQ-4 (L1 runtime checks in the kernel profile) | SHIPPED | `lower_l1`'s `thermite_check!` / `thermite_contract_violation` (`panic!`) is emitted UNCHANGED (NOT stripped, NOT `debug_assert!`); under `#![no_std]`/`panic=abort` it routes to the host `#[panic_handler]` (OQ-1: forge emits neither handler nor allocator — the test supplies the stand-in). Consumer: `emit_source` (no strip). Verified by `kernel_target.rs::l1_checks_emitted_verbatim_in_kernel_source` (macro + handler + `panic!` present, no `debug_assert!`, compiles with a test `#[panic_handler]`/`#[global_allocator]`). |
| REQ-5 (L3 verification path identical) | SHIPPED | `--target kernel` touches ONLY `build.rs`/`cli.rs` (the rustc codegen side); NO edit to `check.rs` or the L3 lowering. The existing `forge check` suites stay green (no diff). Verified: no `check.rs` change in the increment + the full `cargo test -p forge` green. |

## OQ resolutions (this increment)

- **OQ-1** — forge emits NEITHER `#[panic_handler]` NOR `#[global_allocator]`; the
  kernel-target rlib compiles `no_std` without them (only a final bin/staticlib link
  needs them). The `kernel_target.rs` freestanding-compile supplies a test
  `#[panic_handler]` + a `NullAlloc` `#[global_allocator]` (the kernel-host stand-in).
- **OQ-2** — v1 follows the plan literally: ONLY `read`/`write`/`net`/`term` are
  rejected; `pure`/`alloc`/`panic`/`diverge`/`time`/`rand` are admitted (a kernel
  emission carries no syscall mapping at all, so `time`/`rand` are benign here). The
  critic may revisit whether `time`/`rand` warrant a kernel reject.
- **OQ-3** — CONFIRMED no `std::`-qualified path is emitted in any reachable
  collection/string lowering: the l1.rs `std::vec::Vec` PROSE is in `//`/`//!`
  comments only; the emitted CODE uses bare `Vec`/`Vec::new()` (`TString { data:
  Vec<u8> }`, the `TVec*`/`TMap*` wrappers) and `use TString as String;` for the
  surface `String`. So the kernel prelude imports ONLY `Vec` (importing `String` from
  `alloc` would collide with the `as String` alias — `E0252`). No lowerer change
  needed (no NOT-STARTED blocker against the lowerer).
