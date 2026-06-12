# The Runnable Effect Link (Basis Stage 8)
<!--
tier: 3-component
status: draft
audited-sha: bb0fbd9ebaafcab8c0ad5d758cf8d7521baa4836 (bootstrap pin: decision 4 — doc-last-touch, NOT verified-current; backlog #262)
governs: thermite-stdlib/src/effect/read.rs
governs: thermite-stdlib/src/effect/write.rs
governs: thermite-stdlib/src/effect/time.rs
governs: forge/src/build.rs
thesis-refs:
  - thermite-design.md §1
  - thermite-design.md §3
  - thermite-design.md §4.1
  - thermite-design.md §9
-->

## Summary

Stage 8 of the universal-verified-basis buildout (crosslink issue **#81**) closes
the **v1.1 runnable-foreign-body LINK** that Stage 3 (`03-effect-stdlib.md`,
Resolution 2 / OQ-4) explicitly DEFERRED: `forge build` LINKS a Rust syscall-wrapper
`fn` for each `#[boundary("os::<name>")]` target the built program CALLS, so a
*verified* Thermite program using an effect primitive (`now`, `read_byte`,
`write`/`print`) actually **COMPILES, RUNS, and does real I/O** under the #57
seccomp sandbox. Today the verification layer ships (Stage 3: a boundary primitive
certifies L1, the pure caller composes to L3 + `to-the-boundary`) but `forge build
--entry` of a CALLER `rustc`-fails `error[E0433]: cannot find module or crate \`os\``
(GROUNDED below) — there is no `os::` module to link. Stage 8 supplies it.

The wrappers are the **TRUSTED-by-fiat TCB** (you cannot prove the kernel): the
audit manifest (#15) enumerates the boundaries, the #57 seccomp filter confines them
to exactly their declared `fx` syscalls. The LINK is a **`forge build` (running)
concern only** — `forge check` (verification) is UNCHANGED and independent (GROUNDED:
the same program still certifies L1 boundary + L3 to-boundary before and after the
link exists).

This component is **GREENFIELD**. No `thermite-stdlib` crate exists; `forge build`
emits no `os` module. Every REQ is **NOT-STARTED**, owned by issue **#81** (no
redundant blocker filed — #81 is the owning issue).

## The wrapper-link mechanism (PINNED — emit-`mod os`-into-the-crate)

The decision the dispatch asked to pin, resolved EMPIRICALLY (the working path is
GROUNDED under [Grounding the full path](#grounding-the-full-path-real-forgerustc-output)):

**`forge build` EMITS a self-contained `mod os { … }` directly into the generated
crate, keyed off exactly the `#[boundary("os::<name>")]` targets the built program's
reachable fns NAME — option (a) from the dispatch.** NOT a `thermite-stdlib` crate
dependency the generated crate `use`s (option (b)).

The decision rationale, pinned:

- **Self-contained binary, no dependency resolution.** `forge build` v1 invokes raw
  `rustc` on a single self-contained `.rs` (decision OQ-2 in `build.md`: raw rustc, no
  `cargo`, no `Cargo.toml`, no dependency graph). Option (b) (a `thermite-stdlib`
  crate the generated crate depends on) would force `cargo` + dependency resolution +
  an `--extern` path, breaking the hermetic single-source `invoke_rustc` shape. Option
  (a) keeps the binary self-contained — `rustc <crate>.rs` produces the artifact with
  no external link.
- **Keyed off the program's boundaries (minimal TCB surface).** `forge build` emits a
  wrapper ONLY for each distinct `os::<name>` target reachable from the build (the
  union of `BoundaryAttr.target`s over the program's boundary fns, mirroring
  `sandbox::transitive_fx`'s reachability). A program that touches only `os::now`
  links only `now` — the emitted `os` module is exactly the live TCB, nothing more.
- **`thermite-stdlib` is the AUTHORITY for the wrapper SOURCE, not a link target.**
  The wrapper bodies live in `thermite-stdlib/src/effect/{read,write,time}.rs` (the
  `governs` paths inherited from `03-effect-stdlib.md`) as the canonical, reviewed Rust
  over `std` (e.g. `now` → `std::time::SystemTime`, `read_byte` → `std::io::stdin().read`,
  `print` → `std::io::stdout().write`). `forge build` EMITS these bodies (a fixed,
  audited mapping `os::<name>` → wrapper source) INTO the crate's `mod os`; the crate
  does not depend on the `thermite-stdlib` crate at link time. The crate is the
  single source-of-authority for *what the wrapper does*; `forge build`'s emit table
  is keyed by the boundary target string.

So the link is: `lower_boundary_fn_l1` already emits `let result = os::now(args);`
(GROUNDED — the wrapper forwards its params to the foreign target string verbatim,
`thermite-lower/src/l1.rs`); Stage 8 makes `os::now` RESOLVE by emitting a matching
`mod os { pub fn now(...) -> ... { <std body> } }` ahead of it.

### The v1 wrapper set (Read / Write / Time — the demo + Stage 3 primitives)

The wrappers v1 ships match exactly the Stage 3 v1 primitive families
(`03-effect-stdlib.md` Resolution 3) plus what the build/run demo needs:

| `os::<name>` target | Effect (`fx`) | Wrapper body (real `std`) | Return | `fx`→syscall (the #57 table) |
|---|---|---|---|---|
| `os::now` | `time` | `std::time::SystemTime::now().duration_since(UNIX_EPOCH).map(\|d\| d.as_secs())` | `u64` (no failure arm) | `clock_gettime` 228, `clock_nanosleep` 230 |
| `os::read_byte` | `read(input)` | `std::io::stdin().read(&mut [0u8;1])` → byte or EOF sentinel | `u64` (closed: EOF) | + `openat` 257, `lseek` 8, `newfstatat` 262, `statx` 332 |
| `os::read_line` | `read(input)` | `std::io::stdin().read_line(&mut String)` (Stage 7 `String`) | `String`/`Option` | + read set (as above) |
| `os::write` / `os::print` | `write(output)` | `std::io::stdout().write_all(s.as_bytes())` (Stage 7 `String` arg) | `Result<(), _>` status | + `write` 1 (baseline), `fsync` 74, `openat` 257 |

`Net` (`os::net_connect`/`net_send`/`net_recv`), `Rand` (`os::random`), and `Alloc`
(no `os::` wrapper — the Rust allocator under the baseline allowlist) follow the
IDENTICAL emit shape and are **v1.1** (one wrapper-table entry each), exactly as
`03-effect-stdlib.md` scopes the verification side. `os::now` is the minimal demo (no
input, no failure arm); `os::read_byte` is the closed-outcome-set (EOF) demo —
**both GROUNDED to RUN + do real I/O below.**

### The seccomp confinement of the linked wrappers (#57 still applies)

The link does NOT widen the trust boundary: the linked wrapper runs UNDER the SAME
#57 `fx`-derived seccomp filter `synthesize_entry_main` already installs (GROUNDED in
`03-effect-stdlib.md` AC-3, the shipped `forge/src/sandbox.rs`). The prelude is the
FIRST statement of the generated `main`, so the entry — *and the `os::<name>` wrapper
body it reaches* — runs confined to its transitive `fx` allowlist. A `time` program is
confined to `baseline ∪ {clock_gettime, clock_nanosleep}`; the linked `os::now`'s
`clock_gettime` is ALLOWED, but an out-of-`fx` syscall (e.g. `openat`) is
`SIGSYS`-killed (GROUNDED below: exit 159 = 128 + 31). The wrapper is trusted-by-fiat
to do its declared effect; the kernel ENFORCES that it does no more. This is the
second half of the §9 honesty story made RUNNABLE: the assumed contract says "this
only reads the clock," and the seccomp filter kills it if it tries anything else.

## Layer map (the build order)

| Layer | Deliverable | Mechanism |
|---|---|---|
| **8a** | the wrapper stdlib (`thermite-stdlib/src/effect/{read,write,time}.rs`: real `std`/`libc` syscall wrappers for the v1 `os::<name>` targets) + the `forge build` LINK (emit a `mod os { … }` keyed off the program's reachable `#[boundary("os::…")]` targets, ahead of `lower_boundary_fn_l1`'s `os::<name>()` call) | NEW: the wrapper bodies + the `forge/src/build.rs` emit-table + target-reachability keying |
| **8b** | the RUN + sandbox-confinement DEMO (a verified program calling an effect primitive `forge build --entry`s → COMPILES + RUNS + does real I/O; the #57 filter confines the linked wrapper; an out-of-`fx` syscall → SIGSYS) | NEW corpus + a build/run test over the SHIPPED #57 `sandbox::emit_sandbox_prelude` + the 8a link |

8a touches `forge/src/build.rs` (the link emit) + the new `thermite-stdlib` crate.
8b is a corpus + a `build_conformance`-style test. No NEW mechanism is invented in
the sandbox (#57 is verbatim), the lowering (`lower_boundary_fn_l1` is verbatim), or
the verification (`forge check` is untouched) — Stage 8 supplies the missing wrapper
SOURCE and the emit that makes `os::<name>` resolve.

## Requirements

- **REQ-1 (the `os::<name>` wrapper stdlib — real `std`/`libc` syscall bodies):**
  `thermite-stdlib/src/effect/{read,write,time}.rs` provide a real Rust syscall
  wrapper `fn` for each v1 `os::<name>` target: `os::now` (`std::time::SystemTime`),
  `os::read_byte`/`os::read_line` (`std::io::stdin().read`/`read_line`, the latter over
  Stage 7 `String`), `os::write`/`os::print` (`std::io::stdout().write_all`, Stage 7
  `String` arg). Each wrapper's signature MATCHES the `#[boundary]` primitive it backs
  (params + return). Net/Rand follow the same shape (v1.1). Derived from
  `thermite-design.md` §9 (the foreign body is the syscall) + §4.1 (the effect lattice
  these instantiate) + §1 ("verify everything except this small, contracted set") +
  Stage 3 (`03-effect-stdlib.md` REQ-2, the v1 primitive families) + Stage 7
  (`07-strings.md`, `String`/`TString` over `vstd::vec::Vec<u8>` enabling string I/O).

- **REQ-2 (`forge build` LINKS the wrappers by emitting `mod os` keyed off the
  program's boundary targets):** for each distinct `os::<name>` target reachable from
  the build (the union of `BoundaryAttr.target` over the program's `#[boundary]` fns),
  `forge build` emits the matching wrapper SOURCE into a self-contained `mod os { … }`
  ahead of the lowered code (ahead of `lower_boundary_fn_l1`'s `let result =
  os::<name>(args);` crossing), so the generated crate is self-contained and `rustc`
  resolves `os::<name>` (closing the `E0433`). The emit is keyed by the target string,
  emits ONLY the wrappers the program names (minimal TCB), and is byte-deterministic
  (R-CODE-5, §5.3). Derived from §3 (Thermite lowers to self-contained Rust; rustc is
  the codegen backend) + `build.md` REQ-1/REQ-2 (the single-source `invoke_rustc` shape)
  + the GROUNDED `E0433` gap.

- **REQ-3 (a verified program using an effect primitive COMPILES + RUNS + does real
  I/O):** `forge build --entry <caller>` of a program whose reachable fns call a
  v1 effect primitive (`now`/`read_byte`/`print`) produces a runnable binary that, run,
  executes the linked `os::<name>` wrapper, performs the REAL syscall (reads the clock,
  reads stdin, writes stdout), and produces correct output (GROUNDED: `os::now` →
  a live Unix timestamp; `os::read_byte` of byte 'A' → `doubled() = 130`, EOF → `0`).
  The L1 `thermite_check!` on the wrapper's `ens` still fires on a violation (REQ-4 of
  `build.md` is preserved). Derived from §1 (the unlock: a verified program that RUNS
  + does I/O) + `build.md` REQ-3/REQ-4 (the `--entry` runnable form + baked-in checks).

- **REQ-4 (the linked wrapper is #57-seccomp-CONFINED — the live foreign-body run):**
  the linked `os::<name>` wrapper runs UNDER the SHIPPED #57 `fx`-derived seccomp
  filter (`sandbox::emit_sandbox_prelude` installed FIRST in the generated `main`,
  `forge/src/build.rs` `synthesize_entry_main`): the wrapper's declared-`fx` syscalls
  are ALLOWED (a `time` program's `clock_gettime`; a `read` program's `openat`/`read`),
  but a syscall OUTSIDE the entry's transitive `fx` allowlist is `SIGSYS`-killed (exit
  159, GROUNDED). This makes the §9 "killed at the syscall boundary" REAL for the
  LIVE foreign body, not just the `--sandbox-self-test` probe Stage 3 used. The #57
  allowlist derivation is UNCHANGED (`sandbox::transitive_fx`/`syscall_allowlist`
  verbatim). Derived from §4.1 ("the runtime enforces the row … killed at the syscall
  boundary") + §9 + `runtime-sandbox.md` REQ-1/REQ-3 + `03-effect-stdlib.md` REQ-5.

- **REQ-5 (the verification is UNCHANGED — the link is a BUILD concern only):** `forge
  check` / `forge audit` of a program using an effect primitive certify IDENTICALLY
  before and after the link exists — the boundary primitive certifies `Level::L1` +
  `boundary: true` + `boundary_target`, the pure caller composes to `Level::L3` +
  `assurance_scope == ToBoundary { via }`, and the audit manifest enumerates the
  primitive in the TCB (all GROUNDED, all on the SHIPPED Stage 3 / #16 / #52 / #17 /
  #15 path). The link adds NO verification obligation and removes none. Derived from
  §9 (the contract is the interface — independent of the body) + the §52 composition
  property + `03-effect-stdlib.md` AC-1/AC-2 (the GROUNDED verification path).

- **REQ-6 (the wrappers are the TRUSTED-by-fiat TCB — enumerated + confined):** the
  linked `os::<name>` wrappers are trusted by fiat (you cannot prove the kernel/disk),
  and that trust is HONEST exactly because (a) the #15 audit manifest enumerates each
  reached boundary (name + assumed contract + foreign target + effect — GROUNDED:
  `boundary: now -> os::now (req=… ens=… fx=[time])`), and (b) the #57 seccomp filter
  confines each wrapper to its declared `fx` (REQ-4). A program is `verified pure logic
  (L3) + this enumerated, contracted, confined effect base` (§1/§9). The link does not
  enlarge the TCB beyond the boundaries the manifest already enumerates — it makes that
  same enumerated base RUNNABLE. Derived from §1 + §9 ("the TCB is exactly (slag ∪
  boundary contracts ∪ the toolchain)") + `goal.md` R-DEFER-9 (no forged whole-program
  claim).

## Acceptance criteria

ACs tie to a NEW `conformance/effect-link/cases.json` oracle the ORCHESTRATOR authors
(hand-derived per R-CHAR-3, the `conformance/build` + `conformance/sandbox` precedents),
plus a `build_conformance`-style build/run test. The centerpiece programs + their
EXACT expected run output are PINNED below; the builder reproduces them, never copies
toolchain output blindly. The two grounded programs:

```thermite
// time_demo.th — the minimal effect primitive (no input, no failure arm)
#[boundary("os::now")]
fn now() -> u64
  req true
  ens result < 4000000000
  fx  time
;
fn elapsed_ok() -> u64
  req true
  ens result < 4000000000
  fx  time
{
  now()
}
```

```thermite
// read_demo.th — the closed-outcome-set (EOF) effect primitive over stdin
#[boundary("os::read_byte")]
fn read_byte() -> u64
  req true
  ens result <= 256          // 256 = the EOF sentinel; closes the outcome SET
  fx  read(input)
;
fn doubled() -> u64
  req true
  ens result < 512           // holds whether a byte (<256 ⇒ v+v<512) or EOF (0)
  fx  read(input)
{
  let v = read_byte();
  if v < 256 { v + v } else { 0 }   // BOTH outcomes handled
}
```

- **AC-1 (a verified program RUNS + does real I/O — `time`, GROUNDED):** `forge build
  time_demo.th --entry elapsed_ok` COMPILES (rustc exit 0 — the linked `mod os { pub
  fn now() … }` resolves `os::now()`, no `E0433`) and produces a runnable binary that,
  run, executes the real `clock_gettime` syscall and prints `elapsed_ok() = <live Unix
  timestamp>` (a `u64`, e.g. the GROUNDED `1780779978`), exit 0. The output is
  nondeterministic (wall clock), so the oracle asserts the binary exits 0 and prints a
  `u64` matching `elapsed_ok() = \d+`, NOT a fixed value (R-CHAR-3 — the value is the
  world's, not a fixture).

- **AC-2 (a verified program reads stdin + does real I/O — `read`, GROUNDED):** `forge
  build read_demo.th --entry doubled` COMPILES (the linked `os::read_byte` resolves)
  and, run with stdin `A` (byte 65), prints `doubled() = 130` (the hand-derived value:
  `65 + 65 = 130 < 512`, R-CHAR-3); run with EMPTY stdin (EOF), prints `doubled() = 0`
  (the handled EOF arm). Both arms of the closed outcome set RUN correctly. The two
  expected outputs are deterministic given the pinned stdin (the input IS the explicit
  determinism source, R-CODE-5).

- **AC-3 (the linked wrapper is #57-seccomp-CONFINED — the LIVE foreign body,
  GROUNDED):** the `--entry elapsed_ok` binary, run under the default #57 sandbox (ON
  for `--entry`), executes the linked `os::now`'s `clock_gettime` SUCCESSFULLY (it is in
  the `time`-widened allowlist: `baseline ∪ {228, 230}`) — exit 0. An out-of-`fx`
  syscall under the SAME `time` filter (the `--sandbox-self-test` `openat` probe, NOT
  in the `time` allowlist) is `SIGSYS`-KILLED (exit 159 = 128 + 31, the `#57`
  `pure_probe_killed` precedent). GROUNDED: the `time`-confined run prints its timestamp
  and exits 0; the `openat` probe under the `time` filter cores with `Bad system call`,
  exit 159. This is the live-foreign-body confinement #57/OQ-4 deferred — now real.

- **AC-4 (verification is UNCHANGED — the link is build-only, GROUNDED):** `forge check
  time_demo.th --mutation-floor 0` certifies `now` at `Level::L1`, `boundary == true`,
  `boundary_target == "os::now"`, and `elapsed_ok` at `Level::L3`, `assurance_scope ==
  ToBoundary { via: "now" }` — IDENTICAL before and after Stage 8 (the link adds no
  verification path). GROUNDED: reproduced on the SHIPPED tree (output under
  [Grounding](#grounding-the-full-path-real-forgerustc-output)). The `--mutation-floor
  0` is required for the bound-style effect caller (the `03-effect-stdlib.md` #12 note)
  and MUST NOT leak to the pure corpus.

- **AC-5 (the audit manifest enumerates the linked wrapper as the TCB, GROUNDED):**
  `forge audit time_demo.th` emits an `AuditManifest` (#15) whose `tcb` section
  enumerates `now -> os::now (req=… ens=[result < 4000000000] fx=[time])` as a
  `boundary` member; the pure logic appears as L3 + `to-the-boundary`; nothing
  fiat-trusted is omitted (R-DEFER-9). The linked wrapper IS exactly this enumerated
  boundary made runnable.

- **AC-6 (the baked-in L1 check still fires on the linked wrapper, GROUNDED-by-shape):**
  a corrupted `os::now` wrapper or a primitive whose `ens` is violated at runtime
  ABORTS with the always-active `thermite L1 contract violation [ens]` diagnostic and a
  non-zero exit (`build.md` REQ-4 / AC-4, preserved through the link) — the foreign
  body is trusted-by-fiat but its assumed `ens` is L1-CHECKED on every crossing
  (`lower_boundary_fn_l1`'s exit `ens`-check, `thermite-lower/src/l1.rs`).

- **AC-7 (corpus + verification corpus unaffected):** the pure corpus (`sum`,
  `binary_search`) builds + runs IDENTICALLY (no `mod os` emitted — no boundary target
  reachable; `sum(&[1,2,3]) = 6` per `build.md` AC-3), and the Stage 3
  `conformance/effect-stdlib` verification corpus certifies an IDENTICAL cert
  (`forge check` is untouched by the link).

## Architecture

Stage 8 adds the missing wrapper SOURCE + the emit that makes `os::<name>` resolve;
it touches `forge/src/build.rs` (the emit) and adds the `thermite-stdlib` crate. The
full path, GROUNDED:

```text
forge build <program calling os::now> --entry elapsed_ok
  │
  ├─ emit_source (forge/src/build.rs):
  │     thermite_lower::lower_l1(program)                                  [SHIPPED]
  │        → includes lower_boundary_fn_l1's `let result = os::now(args);` [SHIPPED, l1.rs]
  │     + synthesize_entry_main (the deterministic main + #57 prelude)     [SHIPPED]
  │     + NEW: emit `mod os { pub fn now() -> u64 { <std body> } }`        [STAGE 8 — REQ-2]
  │        keyed off the program's reachable #[boundary("os::…")] targets
  │        ▼
  ├─ invoke_rustc (single self-contained .rs):                            [SHIPPED]
  │     rustc resolves os::now  →  EXIT 0 (was E0433 without the link)    [STAGE 8 — REQ-2/3]
  │        ▼
  ├─ run the binary:
  │     #57 seccomp prelude installs the time allowlist FIRST             [SHIPPED, sandbox.rs]
  │     os::now() does the real clock_gettime UNDER the filter            [STAGE 8 — REQ-3/4]
  │     ens-check `result < 4000000000` fires if violated (L1)            [SHIPPED, l1.rs]
  │     prints `elapsed_ok() = <timestamp>`, exit 0                       [STAGE 8 — REQ-3]
  │     an out-of-fx syscall → SIGSYS kill (exit 159)                     [SHIPPED #57 — REQ-4]
  │
forge check <same program> --mutation-floor 0  →  L1 boundary + L3 to-boundary
forge audit <same program>  →  tcb: now -> os::now (…)   [UNCHANGED by the link — REQ-5/6]
```

- **The wrapper SOURCE** lives in `thermite-stdlib/src/effect/{read,write,time}.rs`
  (the canonical reviewed Rust over `std`). `forge build`'s emit maps each reachable
  `os::<name>` target string to the matching wrapper body and emits a `mod os { … }`.
- **The emit site** is `emit_source` / `synthesize_entry_main` in `forge/src/build.rs`:
  the new `mod os` is prepended to (or interleaved with) `lower_l1`'s output, ahead of
  the boundary wrapper's `os::<name>()` crossing.
- **The crossing** is `lower_boundary_fn_l1`'s `let result = <target>(<args>);` in
  `thermite-lower/src/l1.rs` (SHIPPED, verbatim — the wrapper forwards its params to the
  target string). Stage 8 makes the target RESOLVE; the lowering is unchanged.
- **The confinement** is `sandbox::emit_sandbox_prelude` over `transitive_fx` /
  `syscall_allowlist` in `forge/src/sandbox.rs` (SHIPPED, verbatim — the linked wrapper
  runs under the same filter the entry's `fx` derives).
- **The verification surface** (`#16` `gate_fn` BoundaryL1, `#52` `lower_external_body_fn`
  weave, `#17` `AssuranceScope::ToBoundary`, `#15` `AuditManifest.tcb`) is UNTOUCHED —
  `forge check` never emits `mod os` and never invokes rustc.

Boundaries (what Stage 8 is NOT):
- It does NOT change `forge check` (verification), the lowering of the boundary crossing,
  or the #57 allowlist derivation — those ship.
- It is x86_64-Linux only (the #57 seccomp filter + the `os::` wrappers target
  x86_64-Linux; cross-platform is future work, as `runtime-sandbox.md` / OQ-4 scope).
- `Net`/`Rand` wrappers are v1.1 (identical emit shape); `Alloc` needs no `os::`
  wrapper (the Rust allocator under the baseline allowlist).
- The generated `main`'s input convention stays v0.1 fixed-literal (`build.md` OQ-1);
  stdin is fed by the test harness (AC-2), a richer `--input` convention is future work.

## Verification

The ACs are discharged by a `build_conformance`-style build/run test the builder adds,
against the `conformance/effect-link/cases.json` oracle the orchestrator authors,
reusing the `build.rs` + `sandbox.rs` patterns. Discharge commands:

- `cargo test -p forge` — the `build.rs` link + run tests (AC-1..AC-7); `cargo test -p
  thermite-stdlib` — the wrapper unit tests (each `os::<name>` does its syscall).
- The build/run test: `forge build time_demo.th --entry elapsed_ok` → rustc exit 0
  (AC-1), run the binary → exit 0 + stdout matches `elapsed_ok() = \d+` (AC-1); `forge
  build read_demo.th --entry doubled`, run with stdin `A` → `doubled() = 130`, with EOF
  → `doubled() = 0` (AC-2); run under the default sandbox → exit 0, run the
  `--sandbox-self-test` `openat` probe under the `time` filter → exit 159 (AC-3); `forge
  check … --mutation-floor 0` → the unchanged L1 + L3 cert (AC-4); `forge audit` → the
  `tcb` enumeration (AC-5); a violating-`ens` wrapper → the `[ens]` abort (AC-6); `sum`
  builds + runs unchanged (AC-7).
- `cargo clippy -p forge -p thermite-stdlib --all-targets -- -D warnings`, `cargo fmt
  --check` (the gauntlet).
- **Golden link (R-CHAR-3):** a `tests/golden/build/effect-link.rs` hand-authored from
  THIS design — the lowered program with the emitted `mod os { pub fn now() … }` ahead
  of the boundary wrapper — which MUST itself compile + run under real rustc.

**This doc is GROUNDED against the SHIPPED tree (`forge` built from this tree, real
`rustc 1.95.0` / `verus 0.2026.05.24`; all scratch removed per #53).** See the next
section.

## Grounding the full path (REAL `forge`/`rustc` output)

**(1) The GAP — `forge check` certifies, `forge build --entry` `E0433`s.** Over
`time_demo.th` (the `now` program above):

`forge check time_demo.th --mutation-floor 0`:
```
item: now
level: L1
boundary: true
boundary_target: os::now
assurance_scope: to-the-boundary (via now)
item: elapsed_ok
level: L3
assurance_scope: to-the-boundary (via now)
```
`forge build time_demo.th --entry elapsed_ok`:
```
forge: rustc failed to build the lowered artifact: rustc exited with status Some(1); stderr:
error[E0433]: cannot find module or crate `os` in this scope
  --> now_demo_build.rs:24:18
   |
24 |     let result = os::now();
   |                  ^^ use of unresolved module or unlinked crate `os`
```
The verification SHIPS (L1 boundary + L3 to-boundary); the runnable link is the gap.

**(2) The LINK works — emit `mod os` → COMPILES + RUNS + does real I/O.** Emitting the
v1 wrapper `mod os { pub fn now() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH)
.map(\|d\| d.as_secs()).unwrap_or(0) } }` ahead of `lower_boundary_fn_l1`'s
`let result = os::now();` and compiling the self-contained crate under real rustc:
```
rustc exit: 0
elapsed_ok() = 1780779938        ← the LIVE clock_gettime result (a real Unix timestamp)
run exit: 0
```
A VERIFIED Thermite program RAN and did real I/O — the unlock. (The `read` family
grounds identically: `os::read_byte` over `std::io::stdin().read`, fed stdin `A` →
`doubled() = 130`; fed EOF → `doubled() = 0`. Both arms of the closed outcome set run.)

**(3) The #57 sandbox STILL confines the linked wrapper.** Installing the SHIPPED #57
prelude with the `time` allowlist (`baseline ∪ {clock_gettime 228, clock_nanosleep
230}`, the `syscall_allowlist(&{time})` projection) as the FIRST statement of `main`,
then running:
```
=== (A) time-confined run: real clock_gettime allowed, prints ===
elapsed_ok() = 1780779978        ← the linked os::now's clock_gettime ALLOWED under the filter
exit: 0
=== (B) out-of-fx openat under the time filter -> SIGSYS kill ===
Bad system call (core dumped)
exit: 159                         ← 128 + 31 (SIGSYS), the #57 pure_probe_killed precedent
```
The linked foreign body runs confined to EXACTLY its declared `fx` — `clock_gettime`
allowed, `openat` (out of the `time` row) killed. This is the live-foreign-body
confinement OQ-4 deferred, now REAL.

All grounding scratch was created under `/tmp` and removed; no artifacts leaked into
the repo tree (#53 — compiled binaries are large).

## Open questions

- **OQ-1 (emit-`mod os` vs link-`thermite-stdlib`-crate — RESOLVED to emit-`mod os`):**
  pinned to option (a) (emit a self-contained `mod os` into the crate) over option (b)
  (a `thermite-stdlib` crate dependency the generated crate links). Rationale: (a)
  keeps the single-source raw-`rustc` build hermetic (no `cargo`/dependency resolution,
  `build.md` OQ-2) and emits only the wrappers the program names (minimal TCB).
  `thermite-stdlib` remains the AUTHORITY for the wrapper bodies (reviewed, unit-tested,
  the `governs` paths), which `forge build` emits from a fixed target→source table. If a
  future need (large stdlib, shared codegen) forces a crate link, that is a v1.2 OQ; v1
  emits.

- **OQ-2 (the emit-table location — `forge/src/build.rs` vs a generated module):** the
  fixed `os::<name>` → wrapper-source mapping the emit consults — whether it is an inline
  table in `build.rs`, a `const &str` re-exported from `thermite-stdlib`, or read from
  the `thermite-stdlib` source at build time — is a builder/orchestrator decision under
  R-SPEC-2. This doc governs the CONTRACT (the emitted `mod os` must contain exactly the
  reachable wrappers, byte-deterministically); the packaging is settled by the
  orchestrator (mirrors `03-effect-stdlib.md` OQ-2 on the declaration packaging).

- **OQ-3 (the EOF / failure sentinel vs a Stage-1 ADT return):** the GROUNDED `read`
  demo used a `u64` with a `256` EOF sentinel (the simplest closed outcome set that
  RUNS today). A richer `Option<u64>`/`Result` return (the `03-effect-stdlib.md`
  centerpiece) is the more honest closed-outcome-set shape and grounds on the SHIPPED
  built-in `Option` verification path; the runnable wrapper for it returns the Rust
  `Option`. v1 may ship either; the user-enum-match-in-`ens` lowering caveat
  (`03-effect-stdlib.md` OQ-5) still applies — use `Option`/`Result` or the sentinel,
  not a user enum, until that lowering is fixed.

- **OQ-4 (cross-platform — x86_64-Linux only, as #57 scopes):** the `os::` wrappers use
  `std` (portable) but the #57 seccomp filter is x86_64-Linux-specific, so the
  confined-run guarantee (REQ-4) is x86_64-Linux only in v1. A non-Linux `forge build`
  could emit the `mod os` + compile + run but without the seccomp confinement — out of
  v1 scope (matches `runtime-sandbox.md`'s platform scope).

## Routes to add (orchestrator)

The four `governs` files map to this doc. `forge/src/build.rs` already carries a route
to `build.md` (a file may carry multiple governing docs — the `lower.rs` precedent);
ADD the Stage 8 route alongside it. The `thermite-stdlib/src/effect/{read,write,time}.rs`
routes already exist pointing to `03-effect-stdlib.md`; ADD the Stage 8 link route to
each (multiple governing docs per file).

```toml
# Stage 8 — the runnable effect link (issue #81): forge build emits a `mod os`
# linking the v1 os::<name> syscall wrappers so a verified program RUNS + does I/O.
[[route]]
crate_pattern = "forge/src/build.rs"
design = ".design/basis/08-runnable-effect-link.md"
reference = ["conformance/effect-link", "conformance/build"]
conformance_ops = ["now_runs", "read_runs", "now_sandbox_confined"]

[[route]]
crate_pattern = "thermite-stdlib/src/effect/time.rs"
design = ".design/basis/08-runnable-effect-link.md"
reference = ["conformance/effect-link"]
conformance_ops = ["now_runs", "now_sandbox_confined"]

[[route]]
crate_pattern = "thermite-stdlib/src/effect/read.rs"
design = ".design/basis/08-runnable-effect-link.md"
reference = ["conformance/effect-link"]
conformance_ops = ["read_runs"]

[[route]]
crate_pattern = "thermite-stdlib/src/effect/write.rs"
design = ".design/basis/08-runnable-effect-link.md"
reference = ["conformance/effect-link"]
conformance_ops = ["write_runs"]
```

The orchestrator authors `conformance/effect-link/cases.json` (the AC-1..AC-7 programs
+ expected run output / exit codes), the `tests/golden/build/effect-link.rs` golden,
the routes above, and the v1 `os::<name>` wrapper bodies in `thermite-stdlib`. This doc
does NOT author the oracle, the golden, the routes, or the wrappers (R-DOC-1).

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (the `os::<name>` wrapper stdlib — real `std` syscall bodies) | SHIPPED | the `WRAPPERS` table in `forge/src/effect_wrappers.rs` holds a real `std` body for each v1 target: `os::now` (`std::time::SystemTime::now().duration_since(UNIX_EPOCH).map(\|d\| d.as_secs())`), `os::read_byte`/`os::read_line` (`std::io::stdin().read`/`read_line`, the latter → `TString`), `os::write`/`os::print` (`std::io::stdout().write_all` over `TString`). Each handles its error arm honestly (the EOF sentinel 256 / a status code, no `unwrap`-panic). Consumer: `effect_wrappers::emit_mod_os` (emitted by `build::emit_source`). Verified by `effect_link_conformance::elapsed_ok_builds_and_runs` (the linked `os::now` runs a real `clock_gettime`) + `read_byte_links_and_runs_both_arms` (`A`→130, EOF→0) + the `effect_wrappers::tests` unit battery (7 tests). The OQ-2 packaging is the INLINE `forge/src/` table (the orchestrator's settled decision), NOT a `thermite-stdlib` crate. |
| REQ-2 (`forge build` LINKS via emit-`mod os` keyed off boundary targets) | SHIPPED | `build::reachable_boundary_targets` collects the distinct `BoundaryAttr.target` over the program's `#[boundary]` `Item::Fn`s (every one is lowered with an `os::<name>(args)` crossing by `lower_l1`); `effect_wrappers::emit_mod_os` assembles a sorted, deterministic `mod os { … }` carrying EXACTLY those wrappers; `build::emit_source` PREPENDS it to `lower_l1`'s output, closing the GROUNDED `E0433`. Consumer: `build::emit_source` → `build::build_file` → `cli::run_build`. Verified by `effect_link_conformance::elapsed_ok_builds_and_runs` (rustc exit 0, no `E0433`) + `effect_wrappers::tests::{emits_only_named_wrappers,emission_is_sorted_deterministic}` (minimal-TCB keying + R-CODE-5 determinism). |
| REQ-3 (a verified program COMPILES + RUNS + does real I/O) | SHIPPED | `forge build effect_link_demo.th --entry elapsed_ok` compiles + the binary RUNS the linked `os::now`'s real `clock_gettime` → prints `elapsed_ok() = <live Unix timestamp>` (e.g. `1780780684`), exit 0; `os::read_byte` over stdin → `doubled() = 130` (byte `A`) / `0` (EOF, the handled arm). Verified by `effect_link_conformance::elapsed_ok_builds_and_runs` (run exit 0, output a u64 in `(0, 4_000_000_000)`) + `read_byte_links_and_runs_both_arms`. THE UNLOCK: a verified Thermite program runs + does real I/O. |
| REQ-4 (the linked wrapper is #57-seccomp-CONFINED) | SHIPPED | the linked `os::now` runs UNDER the SHIPPED #57 `sandbox::emit_sandbox_prelude` (installed FIRST in `synthesize_entry_main`'s `main`, UNCHANGED): the `time` allowlist INCLUDES `clock_gettime` (228) → the live `os::now` runs clean (exit 0), and EXCLUDES `openat` (257) → the `--sandbox-self-test` probe under the SAME `time` filter is `SIGSYS`-KILLED (exit 159). Verified by `effect_link_conformance::sandbox_confines_the_linked_wrapper` (the live-foreign-body confinement OQ-4 deferred, now real). The #57 allowlist derivation is verbatim. |
| REQ-5 (verification UNCHANGED — link is build-only) | SHIPPED | `forge check effect_link_demo.th --mutation-floor 0` certifies `now` at `L1` + `boundary` + `boundary_target os::now` + `fx time`, and `elapsed_ok` at `L3` + `assurance_scope to_boundary { via: now }` — IDENTICAL to the pre-link cert (the link lives in `build::emit_source` codegen + rustc; `forge check` never emits `mod os` or invokes rustc). Verified by `effect_link_conformance::verify_unchanged` (the before/after invariance now PINNED by the Stage-8 oracle). `forge check`/lowering-for-check is untouched. |
| REQ-6 (the wrappers are the TRUSTED-by-fiat TCB — enumerated + confined) | SHIPPED | the linked `os::now` IS exactly the boundary the SHIPPED #15 `AuditManifest.tcb` enumerates (`boundary: now -> os::now (req=… ens=[result < 4000000000] fx=[time])`, the `forge check` cert's `boundary`/`boundary_target`/`effects` fields, PINNED by `verify_unchanged`) made RUNNABLE under the #57 confinement (REQ-4). `emit_mod_os` emits ONLY the wrappers the program names (minimal TCB), so the link does not enlarge the TCB beyond the enumerated boundaries. Verified by `effect_link_conformance::{verify_unchanged,sandbox_confines_the_linked_wrapper}` + `effect_wrappers::tests::emits_only_named_wrappers`. |
