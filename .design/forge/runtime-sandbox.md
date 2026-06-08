# forge build — runtime effect sandbox: a seccomp-bpf syscall filter derived from the entry's transitive `fx` row
<!--
tier: 3-component
status: draft
governs: forge/src/sandbox.rs
thesis-refs:
  - thermite-design.md §4.1
  - thermite-design.md §9
-->

## Summary

The runtime effect sandbox makes the `fx` row a *runtime* contract, not only a
compile-time one. `forge build --entry <fn>` injects, into the generated `main`, a
**seccomp-bpf filter-install prelude** that runs BEFORE the entry fn is called. The
filter is an ALLOWLIST derived from the entry's **transitive `fx` row** (the union of
the `fx` of the entry plus every fn in its intra-file call closure, reusing
`closure.rs`'s `reachable_in_file_fns`). A syscall outside the allowlist makes the
kernel kill the process with `SIGSYS`. This is the §4.1 promise discharged: *"a
function declared `fx pure` that attempts I/O is killed at the syscall boundary, not
trusted at the type level alone."*

This component is GREENFIELD: `forge/src/sandbox.rs` does not exist. Every REQ is
NOT-STARTED, blocked on crosslink issue **#57**. It builds on **#56** (the
`forge build --entry` runnable executable + the `BuildManifest` `fx` rows, both
SHIPPED in `forge/src/build.rs`). This doc is the forward-looking contract the builder
implements against; the seccomp mechanism is empirically grounded against real
`rustc`/`libc` on Linux (see [Verification](#verification)).

> **Honest v0.1 limitation (read this first).** Pure Thermite has NO I/O surface — a
> `forge`-built binary lowered from pure Thermite never *attempts* a disallowed
> syscall, so a pure program never *triggers* the sandbox. The sandbox's real value is
> (a) **confining `#[boundary]`/`#[slag]` code**: a foreign/fiat body (§9) that COULD
> do I/O is, once it executes, held to ITS declared `fx` at the syscall boundary; and
> (b) a **defense-in-depth backstop** against a miscompilation or lowering escape. For
> v0.1 demonstrability, the enforcement is shown with an explicit **probe** that
> attempts a denied syscall (→ killed) and a genuinely-pure program that runs clean.
> Compiling foreign/boundary BODIES so they actually run and are confined is OUT of
> scope for #57 (the foreign target is external); cross-platform (non-Linux)
> sandboxing is future.

## Requirements

- **REQ-1 (seccomp prelude install, derived from §4.1):** the generated `main` of a
  `forge build --entry <fn>` executable installs a seccomp-bpf filter via
  `prctl(PR_SET_NO_NEW_PRIVS)` + `prctl(PR_SET_SECCOMP, SECCOMP_MODE_FILTER, …)` (raw
  `libc`, no external seccomp crate — see Verification) BEFORE the call to the entry
  fn. The filter's default action is `SECCOMP_RET_KILL_PROCESS`; allowlisted syscalls
  return `SECCOMP_RET_ALLOW`. A non-allowlisted syscall → `SIGSYS` → process killed.

- **REQ-2 (transitive-`fx` derivation, reusing `closure.rs`):** the allowlist is
  derived from the UNION of the entry fn's `fx` and the `fx` of every fn in its
  transitive intra-file call closure (`closure::reachable_in_file_fns(program, entry)`
  ∪ `{entry}`). A `#[boundary]`/`#[slag]` fn's *declared* `fx` is included (that
  fiat-trusted code is confined to its declared effects). The closure walk is the same
  cycle-safe, source-order DFS `closure::classify` uses — never a duplicate walker.

- **REQ-3 (`fx` → syscall allowlist mapping, pinned):** each `fx` token maps to a
  fixed set of x86_64 syscall numbers (the [mapping table](#fx--syscall-allowlist-mapping)).
  `pure` → the minimal baseline a Rust binary needs to run, print, and exit (NO
  `openat`/`socket`). `read(_)`, `write(_)`, `net(_)`, `time`, `rand` ADD their
  syscalls; `alloc` is already in the baseline. The mapping is deterministic: the same
  transitive `fx` set yields the byte-identical filter (R-CODE-5).

- **REQ-4 (sandbox-on-by-default for `--entry`, `--no-sandbox` opt-out):** a
  `forge build --entry <fn>` produces a sandboxed executable BY DEFAULT (the §4.1
  default is enforcement, not opt-in). A `--no-sandbox` flag suppresses the prelude
  (escape hatch for debugging / a platform with no seccomp). The default-library build
  (`forge build` with no `--entry`) emits no prelude (an rlib has no `main`).

- **REQ-5 (the prelude is reproducible + recorded in the manifest):** the emitted
  prelude is byte-deterministic (same fx set → same BPF program bytes), and the
  `BuildManifest` records the sandbox state (the syscall allowlist actually installed,
  derived from the transitive `fx`), so the audit surface (§9) shows what the binary
  is confined to.

- **REQ-6 (demonstrable enforcement — probe + clean pure run):** the toolchain ships a
  way to demonstrate the kill: a test-only **probe** mode (`--sandbox-self-test`, see
  [Probe](#the-probe-how-a-kill-is-demonstrated)) injects, after the prelude, an
  attempt at a syscall NOT on the entry's allowlist (`openat` for a pure filter) so the
  kill is observable; a normal pure run produces clean output.

- **REQ-7 (the `term` terminal-control effect + the `ioctl` grant — issue #106):**
  the §4.1 effect lattice gains a DEDICATED terminal-control atom, surfaced as the
  `fx term` token, whose syscall-allowlist widening is `{ioctl:16}` (the
  [mapping table](#fx--syscall-allowlist-mapping) `TERM_SYSCALLS` row). A fn that
  reaches the terminal (the editor's `os::raw_mode_on`/`os::raw_mode_off`
  `tcgetattr`/`tcsetattr` boundary, which issue the x86_64 `ioctl` syscall #16 with
  the termios `TCGETS`/`TCSETS` cmds) declares `fx term`; its transitive caller's
  allowlist then INCLUDES `ioctl`, so the binary runs CONFINED (raw mode allowed,
  everything else still killed). A program WITHOUT `term` in its transitive `fx`
  attempting `ioctl` is STILL `SIGSYS`-killed — the grant is scoped to the effect,
  NOT folded into `write` (a dedicated atom keeps a plain `write`-program — `print`,
  `write_file` — from silently acquiring `ioctl`). **The grant is `ioctl`-BROAD**
  (any `ioctl` cmd, not only `TCGETS`/`TCSETS`): classic seccomp-bpf cannot filter
  `ioctl` by its `cmd` argument without arg-inspection (the cmd is in a register, and
  v0.1's filter compares only `seccomp_data.nr`), so the honest v1 grant is the whole
  `ioctl` syscall under `term`, DOCUMENTED as the scope (see [OQ-5](#open-questions)).
  Derived from §4.1 (the `fx` row is a runtime contract) + §4.2 (the editor as the
  acceptance program whose raw-mode boundary needs `ioctl`). This is a NEW `Effect`
  ATOM (`Effect::Term` in `ast.rs`) — see [the ripple](#the-term-atom-ripple-issue-106).

## Acceptance criteria

Tied to a `conformance/sandbox/cases.json` oracle the orchestrator authors (the route
[noted below](#route-to-add-orchestrator)). The grounded mechanism numbers
(exit 159 = 128+SIGSYS(31), clean exit 0) are the oracle's expected values.

- **AC-1 (pure runs clean):** `forge build --entry sum conformance/sum.th` (sum is
  `fx pure`) → the sandboxed exe runs CLEAN: prints `6`, exit 0. Baseline syscalls are
  allowed; the program never attempts a denied syscall, so the filter never fires.
  Oracle: `build_and_run` / `pure_runs_clean`, `expect_run_contains: "6"`,
  `expect_run_exit: 0`.

- **AC-2 (denied syscall → killed):** the same pure exe built with
  `--sandbox-self-test` (the probe attempts `openat` after the prelude) → the process
  is KILLED by `SIGSYS`: non-zero exit, terminating signal 31, exit code **159**.
  Oracle: `kill` / `pure_probe_killed`, `expect_run_signal: 31` (or
  `expect_run_exit: 159`), `expect_run_nonzero: true`.

- **AC-3 (allowlist widens for non-pure `fx`):** an entry whose transitive `fx`
  includes `read(x)` → the installed allowlist INCLUDES `openat`/`read`/`close`, so the
  same `openat` probe is ALLOWED (returns an fd / `-errno`, NOT a kill): exit 0. This
  proves the filter is fx-DERIVED, not a constant. Oracle: `widen` /
  `read_fx_allows_openat`.

- **AC-4 (deterministic filter):** building the same entry twice yields a byte-
  identical seccomp prelude (and the same `BuildManifest` allowlist). Oracle: a
  `determinism` case asserting `emit_sandbox_prelude` over the same transitive `fx`
  returns equal bytes (mirrors `build.md` AC-6's `emit_source` determinism).

- **AC-5 (library build has no prelude):** `forge build conformance/sum.th` (no
  `--entry`) emits an rlib with NO seccomp prelude (REQ-4); and `--no-sandbox --entry`
  emits a runner with no prelude. Oracle: `no_prelude` cases asserting the emitted
  source contains no `PR_SET_SECCOMP` call.

- **AC-6 (the `term` fx grants `ioctl`; a non-`term` program's `ioctl` → SIGSYS;
  the editor runs FULLY sandboxed — issue #106):** an entry whose transitive `fx`
  includes `term` → the installed allowlist INCLUDES `ioctl`:16 (so a `tcgetattr`
  ioctl is ALLOWED); the SAME program with a `pure`/`read(_)`/`write(_)` (no `term`)
  filter EXCLUDES `ioctl`:16 (so the editor's raw-mode `ioctl` is `SIGSYS`-killed,
  exit 159 — the #106 bug as it stands TODAY). With `term` granted, the
  `examples/editor/editor.th` `run` entry builds WITH the sandbox (NOT
  `--no-sandbox`) and RUNS clean end-to-end on piped keystrokes: raw mode enters
  (the `ioctl`), a key reads (`read`), an edit applies (the L3 ops), a frame writes
  (`write`), and Ctrl-S SAVES the buffer to the file (the `read`/`write`-covered
  `openat`/`write`) — exit 0. GROUNDED (see
  [Grounding the #106 fix](#grounding-the-106-fix-real-forgerustc)). Oracle: a
  `term_grants_ioctl` case (allowlist membership) + the `forge/tests/editor_runs.rs`
  sandboxed editor run (exit 0, the frames + the saved file).

## Architecture

`forge/src/sandbox.rs` is a NEW module composing two existing, SHIPPED seams; it owns
NO new walker and NO new effect vocabulary.

**1. Transitive `fx` (reuse `closure.rs` + `effects_of`).** The allowlist input is the
union of `effects_of(&f.contract.fx)` over `{entry} ∪
closure::reachable_in_file_fns(program, entry)`. `reachable_in_file_fns` (in
`closure.rs`) is the §9/#17 cycle-safe, source-order DFS already consumed by
`check::item_subprogram`; it traverses THROUGH `#[boundary]`/`#[slag]`/`spec fn`
intermediaries and returns the in-file `Item::Fn` names the entry reaches. A
`#[boundary]`/`#[slag]` fn reached in the closure contributes its *declared* `fx` row
(it is fiat-trusted to declare honestly; the sandbox confines it to exactly that). The
per-token projection is `manifest::effects_of` / `effect_token` (the SAME `["pure"]` /
`read(x)` strings the `BuildManifest.functions` rows carry), so the sandbox input is
the manifest's `fx` rows, restricted to the entry's closure (§4.1: "a caller's row
must subsume every callee's row" — the union IS the entry's effective row).

**2. The BPF filter (raw `libc`, no seccomp crate).** No `seccompiler`/`libseccomp`
crate is cached offline; `libc` (cached, `0.2.x`) is. The prelude hand-builds a classic
`sock_filter[]` program: load `seccomp_data.arch`, guard `AUDIT_ARCH_X86_64`, load
`seccomp_data.nr`, then a `BPF_JEQ` per allowlisted syscall number → `SECCOMP_RET_ALLOW`,
defaulting to `SECCOMP_RET_KILL_PROCESS`. Installed via `prctl(PR_SET_NO_NEW_PRIVS,1)`
then `prctl(PR_SET_SECCOMP, SECCOMP_MODE_FILTER, &sock_fprog)`. This is grounded below.

**3. Injection point (the generated `main`).** `build::synthesize_entry_main` (in
`build.rs`) already emits `fn main() { let r = entry(args); println!(…); }` for
`--entry`. The sandbox prelude is injected as the FIRST statements of that `main`,
before the `let r = entry(…)` call — so the entry fn (and any boundary/slag body it
calls) runs UNDER the filter. The prelude emits an `unsafe` block of the raw `libc`
calls plus the const `sock_filter[]` array literal. The probe (REQ-6) is injected
between the prelude and the entry call only under `--sandbox-self-test`.

### `fx` → syscall allowlist mapping

x86_64 syscall numbers. The baseline was derived empirically (a trivial Rust binary
ran CLEAN under a kill-default filter with exactly this set; see Verification).

| `fx` token | adds (x86_64 syscall : nr) |
|---|---|
| **baseline** (always, incl. `pure`, `alloc`) | `read`:0, `write`:1, `close`:3, `mmap`:9, `mprotect`:10, `munmap`:11, `brk`:12, `rt_sigaction`:13, `rt_sigprocmask`:14, `rt_sigreturn`:15, `madvise`:28, `poll`:7, `sigaltstack`:131, `arch_prctl`:158, `gettid`:186, `futex`:202, `sched_getaffinity`:204, `set_tid_address`:218, `exit`:60, `exit_group`:231, `set_robust_list`:273, `prlimit64`:302, `rseq`:334 |
| `read(_)` | `openat`:257, (`read`:0 already in baseline), `close`:3, `lseek`:8, `statx`:332, `newfstatat`:262 |
| `write(_)` | `openat`:257, (`write`:1 already), `fsync`:74, `newfstatat`:262 |
| `net(_)` | `socket`:41, `connect`:42, `sendto`:44, `recvfrom`:45, `setsockopt`:54, `getsockopt`:55 |
| `alloc` | (already baseline: `mmap`/`munmap`/`brk`/`mprotect`) |
| `time` | `clock_gettime`:228, `clock_nanosleep`:230 |
| `rand` | `getrandom`:318 |
| `term` (NEW, #106) | `ioctl`:16 (termios `TCGETS`/`TCSETS` — but seccomp can't filter the cmd; the whole `ioctl` is granted under `term`, OQ-5) |
| `panic` | (baseline `write`+`exit_group`; panic unwinds + prints to stderr) |
| `diverge` | (no added syscall; divergence is a non-termination effect) |

> The baseline is a SUPERSET-of-minimum: it allows the syscalls the Rust runtime
> startup/teardown + `println!` + the L1 `thermite_check!` panic path need. It pointedly
> EXCLUDES `openat`, `socket`, `connect`, `getrandom`, `clock_gettime` — so a `pure`
> filter denies file I/O, network, rand, and time. See OQ-1 on `read`/`statx` exactness.

### The probe (how a kill is demonstrated)

Because pure Thermite never attempts a denied syscall, the kill is demonstrated by an
explicit probe under a test-only flag `--sandbox-self-test`. The generated `main`
becomes: `[install prelude] → [unsafe { libc::syscall(openat, AT_FDCWD, …) }] →
[let r = entry(…); println!]`. Under a `pure` filter, the `openat` is non-allowlisted →
`SIGSYS` → the process dies BEFORE reaching the entry call (observable: exit 159, no
output). Under a `read(x)` filter, `openat` is allowlisted → the probe returns and the
entry runs normally. The probe is NEVER emitted without the flag (production runners
have no probe). This is the v0.1 demonstrability device; the genuine future trigger
surface is foreign/boundary bodies, not pure Thermite.

## The `term` atom ripple (issue #106)

Granting `ioctl` is NOT a code-local sandbox-table tweak — the PRINCIPLED fix is a
new effect ATOM, because the grant must be scoped to a declared effect (else any
`write`-program acquires `ioctl`). A new `Effect::Term` (the `term` token) is the
right home, and adding it ripples across the toolchain's Effect-exhaustive seams.
This doc PINS the contract; the builder lands the atom under blocker **#132**. The
ripple (all are `match`-exhaustive on `enum Effect`, so the compiler enforces an arm
on each):

1. **`thermite-syntax/src/ast.rs`** — add `Effect::Term` to `enum Effect` (alongside
   `Read`/`Write`/`Net`/`Alloc`/`Time`/`Rand`/`Panic`/`Diverge`). A bare atom (no path
   arg), like `time`/`rand`.
2. **`thermite-syntax/src/parser.rs`** — add a `"term"` arm to `parse_effect` (the
   bare-atom branch beside `"alloc"`/`"time"`/`"rand"`/`"panic"`/`"diverge"`).
3. **`forge/src/manifest.rs`** — add `Effect::Term => "term".to_string()` to
   `effect_token` (the exhaustive match `effects_of` projects through); the
   `effects_of_covers_every_variant` test gains the atom.
4. **`forge/src/sandbox.rs`** — add a `TERM_SYSCALLS: &[u32] = &[16 /* ioctl */]`
   const and a `"term" => TERM_SYSCALLS` arm in `syscall_allowlist`'s leading-verb
   match (beside `"read"`/`"write"`/`"net"`/`"time"`/`"rand"`); the `verus_anchor`
   `mask_to_tokens` exhaustive-mask test gains the atom's bit (ioctl is NOT one of
   the five sensitive user-I/O syscalls the `io_allow` soundness proof covers, so the
   proved-bitset binding is unaffected — `ioctl` is a sixth, terminal-control grant).
5. **The dynamic skill (`thermite-skill`)** — the effect-vocabulary table the skill
   emits auto-requires an arm for each `Effect` variant; `term` gets a one-line row
   (terminal control / `ioctl`). The ≤6,000-token budget is unaffected (one row).
6. **The validator / lowering** — `term` is a valid `fx` atom subject to the SAME
   §4.1 row-subsumption (`.design/lower/effect-subsumption.md`): every transitive
   caller of a `fx term` fn must declare `term`. The lowering treats it like any
   other atom (no special L1/L3 handling — it carries no proof obligation, only the
   syscall grant).
7. **`examples/editor/editor.th`** — `os::raw_mode_on` / `os::raw_mode_off` change
   their declared `fx write(output)` to `fx term` (or `fx term, write(output)` if a
   wrapper also writes; the termios wrappers issue only `ioctl`, so `fx term` is the
   honest minimal row). The editor's `run` loop transitively unions `term` into its
   row, so `forge build --entry run` derives an allowlist with `ioctl`.

`effect_wrappers.rs` (the `os::raw_mode_on`/`os::raw_mode_off` `TERMIOS_RAW_MODE_SOURCE`
wrapper) is UNCHANGED — the wrapper already issues `tcgetattr`/`tcsetattr` (the
`ioctl`); only the *declared `fx`* of the editor's boundary fns and the sandbox table
change. The honest-scope note: a `term` grant is `ioctl`-BROAD (any cmd) because
classic seccomp-bpf compares only `seccomp_data.nr`, not the `cmd` register — a
TCGETS-only filter would need an arg-inspecting filter (a future refinement, OQ-5).

## Grounding the #106 fix (real `forge`/`rustc`)

Grounded on this Linux host (`rustc 1.95.0`, kernel `6.6.87.2-microsoft-WSL2`,
`/proc/sys/kernel/seccomp/actions_avail` ⊇ `kill_process`) against `forge` built from
this tree. The atom is not yet implemented (no `Effect::Term`), so the grant was
grounded by a PROBE — temporarily adding `16 // ioctl` to `WRITE_SYSCALLS` to prove
`ioctl` is the EXACT missing syscall — then REVERTED (the tree is clean; the probe
edit lived only during grounding). The PRINCIPLED landing is the dedicated `term`
atom above (#132), but the grant's SHAPE (the one syscall, the kill→clean flip) is
identical.

**(1) The editor SIGSYS-dies sandboxed TODAY (the #106 bug).** `forge build
examples/editor/editor.th --entry run` (default sandbox) → transitive
`fx = [alloc, diverge, pure, read(input), write(output)]`, allowlist EXCLUDES
`ioctl`:16. Run with piped keystrokes (`printf 'ab\x11'`, Ctrl-Q):

```
Bad system call (core dumped)
=== sandboxed run exit: 159 ===          # 159 = 128 + SIGSYS(31), killed at tcgetattr
```

No output — the kill fires on the FIRST `tcgetattr` `ioctl` inside `raw_mode_on`,
before any frame is written. This is why `forge/tests/editor_runs.rs` builds the
editor `--no-sandbox` today (the honest seam it documents).

**(2) With `ioctl` granted, the editor runs FULLY sandboxed (exit 0).** The probe
adds `ioctl`:16; rebuild + `forge build … --entry run` (default sandbox) → allowlist
INCLUDES 16. Run `printf 'XY\x13\x11'` (insert X, Y; Ctrl-S save; Ctrl-Q quit) over
the demo file `hello world`:

```
=== SANDBOXED run exit: 0 ===
stdout (od): [2J[H hello world [1;1H  ...  XYhello world [1;3H
saved file: XYhello world
```

Raw mode enters (the `ioctl` ALLOWED), the frames render (`\x1b[2J\x1b[H` + buffer +
the cursor coordinate `\x1b[1;3H`), the splice "XY" lands, and Ctrl-S SAVES
`XYhello world` to the file — exit 0, NO stderr. **The editor runs FULLY sandboxed:
every syscall it issues is granted by its transitive `fx`** (`ioctl` by `term`,
`read`/`openat` by `read(input)`, `write`/`openat` by `write(output)`, the heap by
the baseline). NO residual syscall gap (see the file-syscall note below).

**(3) The grant is SCOPED — a non-`term` program's `ioctl` is still killed.** Under
the probe (folded into `write` for grounding), a `pure` program's allowlist
EXCLUDES `ioctl`:16, and a `read(src)` program's allowlist EXCLUDES `ioctl`:16:

```
pure:     ioctl(16) in allowlist: False   | fx: ['pure']
readonly: ioctl(16) in allowlist: False   | fx: ['read(src)']
```

So a program without the terminal-control effect attempting `ioctl` is `SIGSYS`-killed
— the grant is `fx`-DERIVED, not a global constant. (Under the principled `term`
atom, even a `write(_)` program excludes `ioctl` — only `term` grants it. The probe
folded it under `write` ONLY to ground the missing-syscall identity; the dedicated
atom is tighter.) **Reverting the probe restores the kill**: the reverted-`forge`
sandboxed editor run exits 159 again, confirming `ioctl` is the lone gating syscall.

**The file-open/write syscalls (read_file/write_file) are COVERED — no residual gap.**
The editor's `os::read_file` declares `fx read(input)` and `os::write_file` declares
`fx write(output)`. `read(_)` widens with `openat`:257 (+`read`:0 baseline) and
`write(_)` widens with `openat`:257 (+`write`:1 baseline) — so the Ctrl-S save's
`std::fs::write` (`openat` + `write`) and the initial `std::fs::read` load (`openat` +
`read`) are ALREADY granted by the editor's existing `read`/`write` fx. GROUNDED in
(2): the Ctrl-S save wrote `XYhello world` to the file under the sandbox with NO extra
syscall gap. The ONLY missing syscall was `ioctl` (the termios boundary); no separate
"file effect" is needed.

## Verification

Grounded on this Linux host (`rustc 1.95.0`, kernel `6.6.87.2-microsoft-WSL2`,
`/proc/sys/kernel/seccomp/actions_avail` includes `kill_process`). A standalone probe
crate (raw `libc`, the REQ-1 mechanism) was built offline and run in four modes:

```
### PURE (baseline kill-default filter; pure println)   → sum = 6        exit 0   (clean)
### PROBE (attempt openat under the pure filter)         → Bad system call (core dumped)  exit 159
### READ (baseline + openat allowlisted, then openat)    → openat allowed, fd/err = 3     exit 0
### DISCOVER (TRAP-default; pure println)                → sum = 6, NO trap printed       (baseline complete)
```

`exit 159 = 128 + SIGSYS(31)` — the kernel killed the process at the syscall boundary,
exactly §4.1. The PURE/READ split proves the allowlist is `fx`-derived (widening for
`read` lets `openat` through). DISCOVER (a `SECCOMP_RET_TRAP` default + a `SIGSYS`
handler printing `si_syscall`) printed NO trap for the pure println, confirming the
[baseline table](#fx--syscall-allowlist-mapping) is complete for a trivial Rust binary.
Determinism confirmed: `probe` exits `159, 159`; `pure` outputs `sum = 6, sum = 6`.

The discharging checks (post-implementation):

- `cargo test -p forge sandbox_conformance` — drives the `conformance/sandbox/cases.json`
  oracle (AC-1..AC-5): build each fixture with the sandbox, run the exe, assert exit /
  signal / stdout.
- `cargo test -p forge --test sandbox_conformance pure_probe_killed` — AC-2: the
  `openat` probe under a pure filter exits 159 (SIGSYS).
- a `determinism` unit test over `emit_sandbox_prelude(transitive_fx)` — AC-4.
- the standalone grounding above re-runnable as a `divergence`-style smoke (the BPF
  install + kill is host-kernel-dependent, gated on `actions_avail` containing
  `kill_process`).

## Open questions

- **OQ-1 (`read`/`write` syscall exactness):** the exact extra syscalls a Thermite
  `read(path)` body needs depend on what foreign body actually runs (which is OUT of
  #57). The table is a conservative starting set (`openat`/`read`/`close`/`statx`);
  the empirically-grounded part is the `pure` baseline + the `openat`-allow-vs-kill
  split, which is what AC-1..AC-3 test. Refine when real boundary bodies land.
- **OQ-2 (KILL_PROCESS vs KILL_THREAD vs TRAP+report):** `SECCOMP_RET_KILL_PROCESS`
  (grounded) kills hard with no diagnostic of WHICH syscall. A `SECCOMP_RET_TRAP` +
  a SIGSYS handler (the DISCOVER mode) could print the offending syscall for a crisper
  §5 message, at the cost of a handler in every binary. Decision deferred to the
  builder; the doc pins KILL_PROCESS as the default (simplest, matches §4.1 "killed").
- **OQ-3 (architecture portability):** the filter pins `AUDIT_ARCH_X86_64`. A non-x86_64
  Linux host (or non-Linux) needs a different table / a no-op `--no-sandbox` fallback.
  v0.1 is x86_64-Linux only (documented limitation), `--no-sandbox` is the escape.
- **OQ-5 (`ioctl`-broad vs `TCGETS`/`TCSETS`-only — #106):** the `term` grant is the
  WHOLE `ioctl` syscall (#16, any cmd), because classic seccomp-bpf compares only
  `seccomp_data.nr` — it cannot gate `ioctl` by its `cmd` register without an
  arg-inspecting filter (a `BPF_JEQ` on the second arg, a v1.1 refinement). The honest
  v1 scope is therefore "`term` ⇒ any `ioctl`," DOCUMENTED in REQ-7 + the manifest's
  recorded allowlist. A tighter `cmd`-filtered grant (only `TCGETS` 0x5401 / `TCSETS`
  0x5402) is future work; it does not change the atom or the editor's declared `fx`.
- **OQ-4 (vsyscall/VDSO `clock_gettime`):** `time` may resolve via the VDSO (no real
  syscall) on some libc/kernel combos; the syscall-number allowlist is then a
  belt-and-suspenders. Harmless (allowing an unused syscall is safe).

## Route to add (orchestrator)

```toml
# forge build runtime effect sandbox — seccomp filter from the fx row (issue #57)
[[route]]
crate_pattern = "forge/src/sandbox.rs"
design = ".design/forge/runtime-sandbox.md"
reference = ["conformance/sandbox"]
conformance_ops = ["pure_runs_clean", "pure_probe_killed", "read_fx_allows_openat"]
```

The orchestrator authors `conformance/sandbox/cases.json` (the oracle this doc's ACs
cite) and the route above. This doc does NOT author the oracle or the route (R-DOC-1).

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (seccomp prelude install via raw `libc` `prctl`) | SHIPPED | `sandbox::emit_sandbox_prelude` emits a `sock_filter[]` program (arch-guard + per-syscall `BPF_JEQ` → `SECCOMP_RET_ALLOW`, default `SECCOMP_RET_KILL_PROCESS`) installed via `prctl(PR_SET_NO_NEW_PRIVS)` + `prctl(PR_SET_SECCOMP, SECCOMP_MODE_FILTER)`, raw `extern "C"` (no libc crate). Consumer: `build::synthesize_entry_main`. Verified by `sandbox_conformance::pure_runs_clean` + `probe_killed` (real seccomp kill, exit 159). |
| REQ-2 (transitive-`fx` derivation via `closure.rs`) | SHIPPED | `sandbox::transitive_fx` unions `manifest::effects_of` over `{entry} ∪ closure::reachable_in_file_fns(program, entry)` (the #17 walker, never duplicated). Consumer: `build::synthesize_entry_main` + `build::build_file`. Verified by `sandbox_conformance::probe_allowed_when_fx_widens` (the `read` fx widens the allowlist → openat permitted). |
| REQ-3 (`fx` → syscall allowlist mapping) | SHIPPED | `sandbox::syscall_allowlist` maps each token's leading verb to the pinned x86_64 set (the [mapping table](#fx--syscall-allowlist-mapping)); `pure` baseline excludes `openat`, `read(_)` adds it; sorted + deduped (deterministic). Verified by `probe_killed` (pure, no openat → kill) vs `probe_allowed_when_fx_widens` (read, openat → allowed). |
| REQ-4 (sandbox-on-by-default for `--entry`, `--no-sandbox` opt-out) | SHIPPED | `build::SandboxConfig::default` is `SandboxMode::On`; `synthesize_entry_main` injects the prelude FIRST when on; `--no-sandbox` → `SandboxMode::Off` (no prelude); a library build emits no `main`. Consumer: `cli::run_build` (the `--sandbox`/`--no-sandbox` flags). Verified by `sandbox_conformance::no_sandbox_omits_prelude`. |
| REQ-5 (reproducible prelude + manifest record) | SHIPPED | `emit_sandbox_prelude` is byte-deterministic (sorted allowlist); `build::BuildManifest::sandbox` (`SandboxRecord`) records the installed allowlist (the §9 audit surface). Verified by `sandbox::tests::prelude_installs_and_is_deterministic` + `sandbox_conformance::pure_runs_clean`. |
| REQ-6 (demonstrable enforcement — probe + clean pure run) | SHIPPED | `sandbox::emit_probe` injects (under `--sandbox-self-test`, AFTER the filter) a raw `syscall(SYS_openat, ...)`. Consumer: `build::synthesize_entry_main`. Verified by `probe_killed` (exit 159 under pure) vs `probe_allowed_when_fx_widens` (exit 0 under read). The critical interaction — a contract violation still PANICS `[ens]` (exit 101), NOT seccomp-killed — is verified by `contract_violation_panics_not_killed` (the baseline allows the panic/abort path). |
| REQ-7 (the `term` terminal-control atom + the `ioctl` grant, #106) | NOT-STARTED | open prereq blocker **#132**. There is NO `Effect::Term` atom: `enum Effect in ast.rs` is `{Read, Write, Net, Alloc, Time, Rand, Panic, Diverge}` (no `Term`); `parse_effect in parser.rs` accepts no `"term"` token; `syscall_allowlist in sandbox.rs` has no `TERM_SYSCALLS`/`"term"` arm (no `ioctl`:16 in any widening set — `WRITE_SYSCALLS` is `{fsync:74, openat:257, newfstatat:262}`). GROUNDED diagnostic: `forge build examples/editor/editor.th --entry run` (default sandbox) derives `fx=[alloc,diverge,pure,read(input),write(output)]`, the allowlist EXCLUDES `ioctl`:16, and the editor's `raw_mode_on` `tcgetattr` is `SIGSYS`-killed (exit 159) before any output — so `forge/tests/editor_runs.rs` builds the editor `--no-sandbox` today. A probe adding `ioctl`:16 made the SAME editor run FULLY sandboxed (exit 0: raw mode + edit + Ctrl-S save `XYhello world`), proving `ioctl` is the lone gating syscall and the file-I/O `openat`/`write` are already covered by `read`/`write` fx (no residual gap). The probe was REVERTED (the tree is clean); the principled landing is the `Effect::Term` atom + `TERM_SYSCALLS={ioctl:16}` + the editor's `raw_mode_on`/`off` declaring `fx term`, per [the ripple](#the-term-atom-ripple-issue-106). |
