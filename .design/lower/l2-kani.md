# Thermite Contracts → Kani Bounded Model Check (the L2 rung)
<!--
tier: 3-component
status: draft
audited-sha: dff9ae866e3437af272a62e078993e66c1116460 (re-audited 2026-06-12: amended — #10 degrade ladder shipped since (scope-boundary now historical), run_kani 3-arg signature, #51 classify_l2_outcome note, #262)
governs: thermite-lower/src/l2.rs, forge/src/kani.rs
thesis-refs:
  - thermite-design.md §6
  - thermite-design.md §5.1
  - thermite-design.md §5.3
  - thermite-design.md §4.2
  - thermite-design.md §13
-->

## Summary

The **L2 rung** of the verification ladder (`thermite-design.md §6`: "Bounded
model check (Kani-derived) — Contract holds for all inputs **up to bound** —
termination of the check **Guaranteed**"). It has two halves, parallel to the
shipped L3 (`lower.rs` + `forge::check::run_verus`) and L1 (`l1.rs`) rungs:

1. **`thermite-lower::lower_l2(program) -> Result<String, LowerError>`** —
   produces a **Kani proof harness**: a `#[kani::proof]` fn (with
   `#[kani::unwind(N)]` where loops/recursion need it) that creates symbolic
   inputs (`kani::any()` + `kani::assume` bounds), `assume`s the `req`, calls the
   *executable* contract body, and `assert`s the `ens`. The harness reuses the L1
   executable lowering — Kani checks executable Rust like L1, but symbolically and
   bounded.

2. **`forge::kani::run_kani(harness, label, bound) -> Result<L2Result,
   ForgeError>`** — runs
   the real `cargo kani` / `kani` binary on the harness (temp crate/file), checks
   exit status (`goal.md` R-CODE-4), and parses Kani's output into
   **verified-up-to-bound** vs a **concrete counterexample** (§5.1 "counterexamples,
   not adjectives"). Emits a `Level::L2` certificate (`manifest.rs` `Level::L2`
   already exists).

This is the **#9 / v0.2** ladder component (`thermite-design.md §13` v0.2: "Kani-
backed L2 with type-driven bound inference"). The headline of #9 is **type-driven
bound inference**: the symbolic bound is inferred from the parameter *types*.

This component **SHIPPED** in #9 (v0.2): `thermite-lower/src/l2.rs`
(`pub fn lower_l2`) and `forge/src/kani.rs` (`pub fn run_kani`) exist and every
REQ below is **SHIPPED** (see the REQ-status table), verified against real
`cargo kani 0.67.0`. L3 (`lower.rs`), L1 (`l1.rs`), and `forge::check`
(per-item, runs verus) ship alongside.

### Scope boundary (what #9 is and is NOT)

#9 is the **L2 mechanism only**: generate harness + run kani + emit an L2
certificate, invokable + tested directly (e.g. a `forge check --level l2` flag /
distinct entry). #9 does **NOT** build:

- the **automatic L3→L2→L1 degrade ladder** (`thermite-design.md §5.2`; that is
  issue **#10**). AMENDED at the #262 re-audit: #10 has SINCE SHIPPED the
  ladder (`degrade::run_ladder`, wired from `check_file_with_options` via
  `ladder_for_timeout in check.rs` — governed by
  `.design/forge/degrade-ladder.md`), and #51 added `classify_l2_outcome` /
  `enum L2Verdict in kani.rs` (the CONSERVATIVE under-bound vs counterexample
  split — only `unwinding assertion` boilerplate is UnderBound). The EXPLICIT
  `--level l2` path (`check_l2_file`) is unchanged; this #9 scope boundary
  stands as history, not as a current-tree claim;
- **solver profiles / portfolio seeds** (§5.2; issue **#11**).

## Requirements

- **REQ-1 (L2 harness-emission entry point):** `lower_l2(program) ->
  Result<String, LowerError>` (sharing `lower.rs`'s `LowerError`, REQ-9 there)
  emits a single self-contained Rust source `String` containing, for each
  `FnItem`, a `#[kani::proof]` harness fn. The harness shape (Architecture §
  "Harness shape"): create symbolic inputs from the parameter types, `kani::assume`
  the bound predicates, `kani::assume` the `req`, call the executable body, bind
  `result`, `assert!` each `ens`. The body and spec fns reuse the **L1 executable
  lowering** (`l1.rs`), since Kani checks executable Rust (the same `spec_sum`
  recursion, the same combinator `&[u32]` loops). Derived from §6 (the L2 rung) +
  §4.2 ("spec functions are executable").

- **REQ-2 (type-driven bound inference — the #9 headline):** the symbolic bound
  for each parameter is inferred from its **type**, never from the program name
  (SHAPE-keyed, like `lower.rs`'s proof aids). The frozen inference rules
  (Architecture § "Bound inference rules"):
  - `&[T]` / `&mut [T]` → a symbolic array `[T; N]` of fixed capacity `N` plus a
    symbolic `len: usize` with `kani::assume(len <= N)`, sliced `&data[..len]`. `N`
    is the fixed **default slice bound** (a small constant, see § "The slice bound
    `N`"); grounded working value `N = 4`;
  - integer scalars (`u32`/`u64`/`usize`) → `kani::any()` (full symbolic range)
    unless the contract's `req` already bounds them;
  - `bool` → `kani::any()`.
  The bound is a **fixed constant** (determinism, `goal.md` R-CODE-5 / §5.3). The
  certificate must state the bound explicitly (§6 / §12 "L2 and L3 are visually and
  programmatically distinct everywhere they appear"; the bound is the L2 caveat).
  Derived from §13 v0.2 ("type-driven bound inference") + §6.

- **REQ-3 (unwind bounds for loops/recursion):** every loop and every recursive
  call in the executable body needs a CBMC unwind bound, emitted as
  `#[kani::unwind(K)]` on the harness (Architecture § "Unwind bounds"). `K` is
  derived from the slice bound `N`: a slice-driven loop or a slice-recursive
  `spec fn` runs at most `N` iterations, so `K = N + 1` (CBMC needs one extra
  iteration to prove the loop exits — the "unwinding assertion"). Grounded:
  `unwind(5)` discharges `sum`'s `while` over an `N = 4` slice; `unwind(6)`
  discharges `binary_search`'s `loop` (`O(log N)` but CBMC counts concrete
  iterations, so the bound is set conservatively above the slice length). A
  too-small unwind produces an explicit **"unwinding assertion loop 0"** failure
  (grounded below), so an under-bound is a reported non-L2 result, never a false
  pass. `K` is fixed (R-CODE-5). Derived from §6 ("termination of the check
  guaranteed" = CBMC bounds the search) + the grounded Kani output.

- **REQ-4 (`run_kani` invocation — real binary, temp crate, exit status):**
  `forge::kani::run_kani(harness, label, bound) -> Result<L2Result, ForgeError>`
  (the `bound` string is recorded on the cert, REQ-6) writes the
  harness to a temp crate/file (the `crate_stem` no-`.` discipline already in
  `check.rs::crate_stem` applies), spawns the real `cargo kani` / `kani` binary,
  and **checks the exit status** (`goal.md` R-CODE-4 — never swallow a
  subprocess failure). The invocation pins the bound (REQ-2) and any Kani/CBMC
  seed so the run is deterministic (§5.3 "kani seed/bound pinned", R-CODE-5).
  Parallel to `check.rs::run_verus` / `invoke_verus`. Derived from §5.1 + §5.3 +
  R-CODE-4.

- **REQ-5 (Kani output → L2-or-counterexample parsing):** `run_kani` parses Kani's
  output into either (a) **verified up to bound** → `Level::L2` with a discharged
  obligation that *records the bound* (e.g. `"slice ≤ 4, unwind 5"`), or (b) a
  concrete **counterexample** → a non-L2 reported certificate carrying the failed
  `assert!` description + its `src:line` location as an `ObligationResult::failed`
  witness (§5.1). The grounded success/failure markers are pinned in Architecture §
  "Parsing Kani output". An unparseable/internal Kani failure (e.g. an
  "unsupported construct" that becomes reachable) is a `ForgeError`, never a silent
  success (R-CODE-4, mirroring `parse_verus_output`'s VIR-error branch). Derived
  from §5.1 + R-CODE-4.

- **REQ-6 (the L2 semantics caveat — "up to bound", not "for all"):** the emitted
  `Level::L2` certificate means the contract **holds for all inputs UP TO the
  bound** (slices of length ≤ `N`, loops/recursion unwound ≤ `K`), NOT for all
  inputs (that is L3). The certificate states the bound; L2 is `< L3` in the
  assurance order. This boundary is recorded so L2 is not oversold as a proof
  (§12 risk "Bounded checks oversold as proofs → Manifest states bounds
  explicitly; L2 and L3 are visually and programmatically distinct"). Derived from
  §6 + §12.

- **REQ-7 (forge L2 exposure — invokable, NOT auto-degrade):** `forge` exposes L2
  as an explicit entry — a `forge check --level l2` flag or a distinct
  `forge check-l2` path (the exact surface is OQ-1) — that runs
  `thermite_lower::lower_l2` → `forge::kani::run_kani` → an L2 `Certificate`, in the
  per-item shape `check.rs` already uses (`item_subprogram`, the spec-fn
  dependencies, the temp-file pattern). #9 did **NOT** wire L2 as the automatic
  fallback on a verus timeout (that was #10's job — SINCE SHIPPED, see the
  amended scope boundary) and did **NOT** add solver profiles (#11). Derived from §13 v0.2 scope boundary +
  `goal.md` R-DEFER-4/R-DEFER-7.

- **REQ-8 (Kani-absent = environment error / skip-loud, mirroring verus):** a
  spawn `ErrorKind::NotFound` for the kani binary maps to a structured
  `ForgeError::KaniAbsent` (parallel to `ForgeError::VerusAbsent` in `cli.rs`),
  never a silent success (R-CODE-4). Because `cargo kani` is a heavy external
  toolchain, the L2 *integration* tests that spawn the real binary must
  **skip-LOUD when kani is absent** (a `println!`/eprintln diagnostic + early
  return, NOT `#[ignore]`), mirroring the verus-absent skip pattern the L3 tests
  use. The PURE parsing tests (the analogue of `check.rs`'s
  `parse_verus_output` unit tests) run unconditionally — they feed canned Kani
  output strings and assert the `L2Result`, with no binary spawn. Derived from
  R-CODE-4 + R-DEFER-3 (no `#[ignore]` escape) + the grounded setup cost below.

- **REQ-9 (determinism, §5.3 / R-CODE-5):** L2 results are reproducible given the
  same toolchain version, bound, and seed. The slice bound `N`, the unwind bound
  `K`, and any Kani/CBMC seed are **fixed constants / pinned inputs**, never
  wall-clock-derived. Kani is a bounded model check (CBMC over a fixed
  unwinding) — given the same harness + bound it is deterministic; the only
  wall-clock field is `solver_time_ms`, which (like the L3 path) is EXCLUDED from
  the certificate oracle (`manifest::Certificate::oracle_subset`). Derived from
  §5.3 + R-CODE-5 + the L3 precedent.

## Acceptance criteria

- **AC-1 (`sum` harness verifies up to bound):** `lower_l2(parse("conformance/
  sum.th"))` emits a `#[kani::proof] #[kani::unwind(5)]` harness for `sum` whose
  body is the executable L1 `sum` + `spec_sum`, with a symbolic `&[u32]` of length
  ≤ `N` and `assume(xs.len() <= 1_000_000)`; `run_kani` on it returns `Level::L2`
  ("VERIFICATION:- SUCCESSFUL"). **Grounded** (this run is the external truth,
  R-CHAR-3): with `N = 4`, `unwind(5)`, Kani reports `0 of 36 failed` /
  `VERIFICATION:- SUCCESSFUL`. (REQ-1, REQ-2, REQ-3, REQ-5)

- **AC-2 (`binary_search` harness verifies up to bound):** `lower_l2(parse(
  "conformance/binary_search.th"))` emits a `#[kani::proof] #[kani::unwind(6)]`
  harness with a symbolic `&[u32]` (length ≤ `N`) + symbolic `needle: u32`,
  `assume(sorted(haystack))`, and the `Option` `ens` match as the assertion;
  `run_kani` returns `Level::L2`. **Grounded:** with `N = 4`, `unwind(6)`, Kani
  reports `0 of 82 failed` / `VERIFICATION:- SUCCESSFUL`. (REQ-1, REQ-2, REQ-3,
  REQ-5)

- **AC-3 (broken contract → counterexample → NOT L2):** an L2 harness over a
  body that violates its `ens` (a `sum` body mutated to `i = i + 2`) makes
  `run_kani` return a non-L2 reported certificate carrying the failed assertion +
  its location, NOT a `ForgeError` and NOT `Level::L2`. **Grounded:** the mutated
  body yields `1 of 36 failed`, `Failed Checks: assertion failed: result ==
  spec_sum(xs)`, `File: "src/lib.rs", line 27`, `VERIFICATION:- FAILED`. (REQ-5,
  §5.1)

- **AC-4 (bound is type-derived, not name-derived):** the slice bound `N` and the
  `&[T]`→symbolic-array lowering are chosen from the parameter *types* (the
  `&[u32]` of `sum`/`binary_search`), provable by a unit test that lowers a
  synthetic `fn f(xs: &[u32], k: u32)` and observes the same `kani::any()`/
  `assume(len <= N)` slice scaffolding + an unbounded `kani::any()` for `k` — no
  `if name == ...`. (REQ-2)

- **AC-5 (under-bound is a reported failure, never a false pass):** an unwind set
  below the slice bound makes `run_kani` report a non-L2 result, not a spurious
  L2. **Grounded:** `binary_search` with `unwind(2)` yields `3 of 82 failed (79
  undetermined)`, `Failed Checks: unwinding assertion loop 0`, `VERIFICATION:-
  FAILED`, with Kani's tip "Consider increasing the unwinding value". The emitter's
  `K = N + 1` rule (REQ-3) avoids this for the corpus. (REQ-3, REQ-5)

- **AC-6 (the L2 cert states the bound):** the `Level::L2` certificate's discharged
  obligation (or a dedicated field) names the bound (`slice ≤ N, unwind K`) so a
  reader sees the L2 caveat (§6, §12). A round-trip test asserts the bound string
  is present and the level is `L2`, distinct from `L3`. (REQ-6)

- **AC-7 (Kani-absent = structured error + skip-loud test):** `run_kani` with the
  kani binary absent returns `ForgeError::KaniAbsent` (no panic, no silent
  success). **Grounded:** a `PATH` without kani yields ENOENT (`env:
  'cargo-kani': No such file or directory`). The integration test that spawns
  real kani skips LOUD (prints a diagnostic + returns) when the binary is absent,
  never `#[ignore]`; the pure output-parsing tests run unconditionally. (REQ-8,
  R-CODE-4)

- **AC-8 (determinism):** two `run_kani` invocations over the same harness + bound
  + seed produce oracle-equal certificates (same `level`, same bound, same
  obligations modulo `solver_time_ms`). `solver_time_ms` is excluded from the
  oracle (`oracle_subset`), exactly as in the L3 path. (REQ-9)

- **AC-9 (`LowerError`, never panics):** `lower_l2` over an un-lowerable construct
  returns `Err(LowerError)`, never panics; over the corpus returns `Ok`. The
  emitted harness's `assert!`/`kani::assume` are the intended *Kani* checks, not a
  toolchain panic (the R-CODE-2 boundary, exactly as `l1.rs`'s
  `thermite_contract_violation` documents). (REQ-1, R-CODE-2)

## Architecture

Two files, sibling to the shipped rungs:

- `thermite-lower/src/l2.rs` — a recursive emitter over the `thermite-syntax` AST,
  sibling to `lower.rs` (L3) and `l1.rs` (L1), sharing the `LowerError` enum
  (`enum LowerError in lower.rs`). It REUSES the L1 executable lowering for the
  body and spec fns (Kani checks executable Rust): `pub fn lower_l1 in l1.rs`,
  `fn lower_spec_fn_l1 in l1.rs`, `fn emit_combinator_l1_defs in l1.rs`,
  `fn lower_expr_exec in l1.rs`. The new surface it adds is the *harness wrapper*
  (symbolic inputs + assume/assert) and the *bound inference*.
- `forge/src/kani.rs` — the L2 driver, parallel to `forge/src/check.rs`'s verus
  path: `fn run_verus in check.rs`, `fn invoke_verus in check.rs`,
  `fn parse_verus_output in check.rs`, `fn crate_stem in check.rs`,
  `fn unique_temp_path in check.rs`. `run_kani` mirrors these for the kani binary.

Symbol anchors used: `struct FnItem` / `struct SpecFnItem` / `struct Param` /
`struct Type` / `struct LoopNode` in `thermite-syntax/src/ast.rs`;
`enum Level` (`Level::L2`) / `struct Certificate` / `struct ObligationResult` /
`fn oracle_subset` in `forge/src/manifest.rs`; `enum ForgeError` (`VerusAbsent`,
the `KaniAbsent` parallel) in `forge/src/cli.rs`; `static REGISTRY` / `fn lookup`
/ `CombinatorSig.l1` in `thermite-spec/src/combinators.rs`.

### Why reuse L1, not L3 (REQ-1)

Kani verifies **executable Rust** symbolically (CBMC over the compiled body), not
SMT spec annotations. So the L2 body is the L1 form: real `&[u32]` slices (no
`vstd`/`Seq`/`@`), the real recursive `spec_sum`, the real combinator loops
(`sorted`, `forall_in`). The harness then makes the inputs *symbolic* and turns
`req`/`ens` into `kani::assume`/`assert!`. The grounded harness for `sum` reused
exactly the `l1.rs` `spec_sum` (`if xs.is_empty() { 0 } else { xs[0] as u64 +
spec_sum(&xs[1..]) }`) and a `sum` body identical to the L1 lowering sans the
per-iteration `inv` checks (Kani derives the loop bound from the unwind, not from
the invariant).

### Harness shape (REQ-1), pinned for `sum`

The grounded, Kani-verified `sum` harness (`N = 4`, the external truth for AC-1):

```rust
fn spec_sum(xs: &[u32]) -> u64 {                       // reused L1 spec fn
    if xs.is_empty() { 0 } else { xs[0] as u64 + spec_sum(&xs[1..]) }
}
fn sum(xs: &[u32]) -> u64 {                            // reused L1 body (no inv checks)
    let mut acc: u64 = 0;
    let mut i: usize = 0;
    while i < xs.len() { acc = acc + xs[i] as u64; i = i + 1; }
    acc
}

#[cfg(kani)]
#[kani::proof]
#[kani::unwind(5)]                                     // K = N + 1 (REQ-3)
fn check_sum() {
    const N: usize = 4;                                // slice bound (REQ-2)
    let len: usize = kani::any();
    kani::assume(len <= N);                            // type-driven slice bound
    let data: [u32; N] = kani::any();                  // symbolic array
    let xs: &[u32] = &data[..len];
    kani::assume(xs.len() <= 1_000_000);               // req
    let result = sum(xs);                              // call the executable body
    assert!(result == spec_sum(xs));                   // ens
    // (the second ens — result <= len * u32::MAX — is a second assert!)
}
```

### Harness shape, pinned for `binary_search`

The grounded, Kani-verified `binary_search` harness (`N = 4`, `unwind(6)`, the
external truth for AC-2): a symbolic `&[u32]` length ≤ `N`, a symbolic `needle:
u32`, `kani::assume(sorted(haystack))` for the `req`, the executable
`binary_search` body (with its `loop`), and the `Option` `ens` lowered to a
`match result { Some(i) => assert!(i < haystack.len() && haystack[i] == needle),
None => assert!(forall_in(haystack, |x| x != needle)) }`. The combinator L1 forms
(`sorted`, `forall_in`) are emitted from `CombinatorSig.l1`, exactly as the L1
rung does via `emit_combinator_l1_defs`.

### Bound inference rules (REQ-2 — the #9 headline)

SHAPE-keyed on the `struct Param`'s `struct Type` (never on the program name):

| Param type | Symbolic construction | Bound |
|---|---|---|
| `&[T]` / `&mut [T]` | `let len: usize = kani::any(); kani::assume(len <= N); let data: [T; N] = kani::any(); let xs = &data[..len];` | `len ≤ N` |
| `u32` / `u64` / `usize` | `let x: T = kani::any();` | full symbolic range (or narrowed by a `req`) |
| `bool` | `let b: bool = kani::any();` | both values |

The `req` clause is `kani::assume`d AFTER the symbolic construction, so a `req`
that further bounds an integer (e.g. `sum`'s `xs.len() <= 1_000_000`) prunes the
search without changing the type-driven scaffolding. This is what makes the bound
**type-driven**: the slice scaffolding is emitted purely from seeing a `&[T]`
parameter, identical for `sum` and `binary_search` (AC-4).

### The slice bound `N`

`N` is the **fixed default slice bound** — a small constant (the design's §5.2
example phrases it "slices ≤ 8"). The grounded working value here is `N = 4`,
which verifies both corpus programs in under a second (sum: 0.98s; binary_search:
0.44s). The design text (§5.2) uses `8` as an illustration; the builder picks the
concrete `N` (OQ-2), but it MUST be a fixed constant (R-CODE-5) and MUST be
reflected in the certificate's bound string (REQ-6 / AC-6). A larger `N` raises
assurance and cost; `N = 4`–`8` is the documented range.

### Unwind bounds (REQ-3)

CBMC unwinds each loop a fixed number of times and emits an **unwinding
assertion** that the loop exits within the bound. For a slice-driven loop or a
slice-recursive spec fn bounded by `N`, the loop runs ≤ `N` times, so the harness
needs `#[kani::unwind(K)]` with `K = N + 1` (the extra iteration lets CBMC prove
the exit). Grounded: `unwind(5)` for `sum`'s `N = 4` slice; `unwind(6)` for
`binary_search`. A too-small `K` is NOT a false pass — Kani reports an explicit
`unwinding assertion loop 0` failure and `... (79 undetermined)` (grounded AC-5),
which `run_kani` parses as a non-L2 result. The emitter derives `K` from the
slice bound `N` (the loops in the corpus are all slice-length-bounded); a program
with a non-slice loop bound is OQ-3.

### `run_kani` invocation (REQ-4)

Parallel to `check.rs::run_verus` → `invoke_verus`:

1. write the harness to a temp file/crate with a valid no-`.` crate stem
   (`crate_stem in check.rs` is the existing discipline — reuse or mirror it);
2. spawn `cargo kani` (or `kani`) on it, pinning the bound (the harness encodes
   `N`/`K`) and any CBMC seed for determinism (§5.3);
3. capture stdout/stderr + exit status; ENOENT → `ForgeError::KaniAbsent`
   (REQ-8); any other spawn failure → a `KaniSpawn` error (mirror `VerusSpawn`);
4. clean up the temp file (best-effort, never mask the real result — exactly
   `run_verus`'s pattern);
5. parse (REQ-5).

The grounded invocation is `cargo kani` run in the crate dir; `--output-format
terse` gives the compact `VERIFICATION RESULT` / `Failed Checks` summary the
parser keys on. Determinism (§5.3): the bound + seed are the pinned inputs; the
temp path may vary (process-id + counter, never wall-clock — the
`unique_temp_path in check.rs` precedent: "determinism is a property of the
CERTIFICATE, not the scratch path").

### Parsing Kani output (REQ-5)

Grounded markers (the external truth, R-CHAR-3 — these are Kani 0.67.0's real
format, not forge's output):

- **success →** `Level::L2`: the line `VERIFICATION:- SUCCESSFUL` (and `** 0 of N
  failed`). The discharged obligation records the bound (`slice ≤ N, unwind K`).
- **counterexample →** non-L2 reported cert: `VERIFICATION:- FAILED` plus, under
  `--output-format terse`, `Failed Checks: <description>` and `File: "<src>",
  line <n>`. Each failed check becomes an `ObligationResult::failed(description,
  Some("file:line"), Some(raw))` — the §5.1 counterexample witness. The unwinding-
  assertion failure (`Failed Checks: unwinding assertion loop 0`) is parsed the
  same way (a reported failure, AC-5).
- **internal/unsupported failure →** `ForgeError`: Kani warns "Found the following
  unsupported constructs … Verification will fail if reachable". If the run fails
  because an unsupported construct is *reachable* (not because a contract
  `assert!` failed), that is a tooling failure, surfaced as a `ForgeError`
  (parallel to `parse_verus_output`'s `encountered_vir_error` branch), never a
  swallowed success (R-CODE-4).

Note (grounded): even a SUCCESSFUL run emits dozens of internal CBMC checks
(pointer-deref, slice-index, `checked_sub` overflow) — all `Status: SUCCESS`. The
parser keys on the **summary** line (`VERIFICATION:- SUCCESSFUL/FAILED` + `N of M
failed`), NOT on enumerating every check, exactly as `parse_verus_output` trusts
the `verification-results` summary rather than over-fitting.

### The L2 certificate (REQ-6)

`run_kani` builds a `Certificate` with `level = Level::L2` (the enum variant
already exists in `manifest.rs`), the bound recorded in the discharged obligation
(or a dedicated bound field if the builder adds one additively, mirroring the
`slag_meta`/`cached` additive-field precedent in `manifest.rs`). L2 is
programmatically distinct from L3 (§12): a reader sees `L2` + the bound and knows
the contract holds only up to that bound.

### Determinism (REQ-9, §5.3)

The slice bound `N`, the unwind bound `K`, and any CBMC/Kani seed are fixed
constants. Kani is a bounded model check (a SAT/SMT query over a finite unwinding)
— deterministic given the bound. `solver_time_ms` is the only wall-clock field
and is excluded from the oracle (`oracle_subset in manifest.rs`), exactly as the
L3 path excludes verus's `solver_time_ms`.

### Setup / environment status (grounded)

`cargo-kani 0.67.0` is installed at `~/.cargo/bin/cargo-kani`; `cargo kani setup`
is **already complete** — `~/.kani/kani-0.67.0/` carries the prebuilt rlibs
(`libkani.rlib`, `libstd.rlib`) and a pinned toolchain symlink
(`nightly-2025-11-21`). A `cargo kani` run on a fresh tiny crate completed in
≈1s (sum) / 0.44s (binary_search), so the per-harness cost is small ONCE setup is
done. But `cargo kani` is a heavy external toolchain (its own nightly + CBMC); CI
or a fresh machine may lack it. Hence REQ-8's skip-loud discipline for the
binary-spawning integration tests (mirroring the verus-absent skip), and the
pure-parsing unit tests that run unconditionally on canned output.

## Verification

`cargo test -p thermite-lower` (the `lower_l2` emitter) + `cargo test -p forge`
(the `run_kani` driver), gauntlet per `goal.md`:

- **AC-1 / AC-2:** lower `conformance/sum.th` + `conformance/binary_search.th`,
  emit the harness, spawn real `cargo kani`, assert `Level::L2` — **skip-loud if
  kani absent** (REQ-8). Expected `VERIFICATION:- SUCCESSFUL` traces to the
  grounded Kani runs above (R-CHAR-3), not forge's output.
- **AC-3:** a mutated `sum` body (the `i = i + 2` off-by harness) → `run_kani`
  returns a non-L2 cert with the failed-assertion witness (grounded
  counterexample).
- **AC-4:** lower a synthetic `fn f(xs: &[u32], k: u32)` and assert the slice
  scaffolding + unbounded `k` are type-derived (a pure emitter test, no kani
  spawn).
- **AC-5:** a deliberately-too-small unwind → reported `unwinding assertion`
  failure (grounded), parsed as non-L2.
- **AC-6:** the L2 cert carries the bound string and `level == L2 != L3`.
- **AC-7:** `run_kani` with kani absent → `ForgeError::KaniAbsent` (a pure
  spawn-path test pointing at a non-existent binary); the integration tests skip
  loud (no `#[ignore]`, R-DEFER-3).
- **AC-8:** two runs over the same harness → oracle-equal certs.
- **AC-9:** an un-lowerable construct → `Err(LowerError)`, no panic.

The pure output-parsing tests (the analogue of `check.rs`'s
`parseable_success_is_l3_cert` / `parseable_failure_is_reported_cert_with_counterexample`
/ `unparseable_output_is_verus_output_error`) feed canned Kani output strings
(the grounded `VERIFICATION:- SUCCESSFUL` / `Failed Checks: …` / `unwinding
assertion` blobs) and assert the `L2Result` — these run unconditionally with NO
kani spawn (R-CHAR-3: the expected strings are Kani's real format).

**Both files SHIPPED in #9.** The harness shapes above were grounded against
real `cargo kani 0.67.0` runs and remain the external truth the tests anchor
to.

### Routes (added at #9; verified present in `tooling/spec-routes.toml` at the #262 re-audit)

```toml
[[route]]
crate_pattern = "thermite-lower/src/l2.rs"
design = ".design/lower/l2-kani.md"
reference = ["conformance/sum.th", "conformance/binary_search.th"]
conformance_ops = ["sum", "binary_search"]

[[route]]
crate_pattern = "forge/src/kani.rs"
design = ".design/lower/l2-kani.md"
reference = ["conformance/sum.th", "conformance/binary_search.th"]
conformance_ops = ["sum", "binary_search"]
```

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (L2 harness-emission entry point) | SHIPPED | `pub fn lower_l2 in l2.rs` emits a `#[kani::proof]` harness per `FnItem`, reusing the L1 lowering (`lower_spec_fn_l1`/`emit_combinator_l1_defs`/`lower_expr_exec` from `l1.rs`); consumer `check::check_l2_file in check.rs`; verified by `l2_conformance::sum_harness_verifies_to_bound` (real kani → `VERIFICATION:- SUCCESSFUL`). |
| REQ-2 (type-driven bound inference) | SHIPPED | `infer_symbolic_input in l2.rs` keys on the `struct Param`'s `struct Type` (`&[T]`→array+`len<=N`, scalar→`kani::any()`); `l2_conformance::bound_is_type_derived_not_name_derived` (AC-4 — synthetic `fn f(xs: &[u32], k: u32)`, no name check). |
| REQ-3 (unwind bounds for loops/recursion) | SHIPPED | `unwind_bound in l2.rs` derives `K` SHAPE-keyed on the loop kind (`while`→`N+1`=5 for sum, unconditional `loop`→`N+2`=6 for binary_search); `l2_conformance::sum_harness_verifies_to_bound`/`binary_search_harness_verifies_to_bound`. Under-bound → `unwinding assertion` (AC-5, `under_bound_is_reported_failure_not_false_pass`). |
| REQ-4 (`run_kani` invocation, exit status) | SHIPPED | `pub fn run_kani in kani.rs` writes a temp cargo crate (`write_kani_crate`, no-`.` `crate_stem`), spawns `cargo-kani`, checks exit status; ENOENT → `ForgeError::KaniAbsent`, other → `KaniSpawn`; temp crate removed. Consumer `check::check_l2_file`. |
| REQ-5 (Kani output → L2-or-counterexample) | SHIPPED | `parse_kani_output in kani.rs` keys on `VERIFICATION:- SUCCESSFUL`/`FAILED` + `Failed Checks:`/`File:`; no summary → `KaniOutput` (R-CODE-4). Pure tests `success_terse_is_l2`/`failure_terse_is_counterexample`/`under_bound_is_reported_failure`/`no_summary_is_kani_output_error`. |
| REQ-6 (L2 "up to bound" caveat in the cert) | SHIPPED | `bound_string in l2.rs` (`slice <= 4, unwind K`) is recorded on the discharged obligation by `run_kani`; the cert is `Level::L2 in manifest.rs` (distinct from L3); `kani::bound_recorded_on_l2_cert` (AC-6) + `l2_check::forge_check_level_l2_sum_is_l2`. |
| REQ-7 (forge L2 exposure, not auto-degrade) | SHIPPED | `forge check --level l2 <file>` runs `check::check_l2_file` (per-`fn` `lower_l2`→`run_kani`→L2 cert); default stays L3; the `CheckLevel` flag in `cli::parse_args`; NO auto-degrade (#10). `l2_check::forge_check_level_l2_sum_is_l2` + `cli::parses_level_flag`. |
| REQ-8 (Kani-absent = env error / skip-loud) | SHIPPED | `ForgeError::KaniAbsent in cli.rs`; `run_kani` maps ENOENT to it (`kani::run_kani_with_absent_binary_is_kani_absent`, AC-7); the kani-spawning integration tests skip LOUD (eprintln + return, no `#[ignore]`). |
| REQ-9 (determinism, pinned bound + seed) | SHIPPED | `SLICE_BOUND in l2.rs` is a fixed `const`; `unwind_bound`/`lower_l2` are pure functions of the AST (`l2_conformance::lowering_is_deterministic`); the temp crate path uses pid+counter (not wall-clock); `solver_time_ms` excluded from `oracle_subset in manifest.rs`. |

## Open questions (for the orchestrator before the builder runs)

- **OQ-1 (forge L2 surface):** `forge check --level l2` flag vs. a distinct
  `forge check-l2` subcommand vs. an explicit `--engine kani`. The design (§13
  v0.2) names the L2 mechanism but not the CLI verb; `.design/forge/cli.md` is the
  governing surface. Recorded; not a blocker for the mechanism.

- **OQ-2 (the concrete slice bound `N`):** §5.2 illustrates "slices ≤ 8"; the
  grounded working value is `N = 4` (verifies the corpus in < 1s). The builder
  picks the concrete constant in the `4`–`8` range; it MUST be fixed (R-CODE-5)
  and reflected in the cert bound string (REQ-6). Recorded; not a blocker.

- **OQ-3 (non-slice loop bounds):** `K = N + 1` is derived from the slice bound
  because every corpus loop is slice-length-bounded. A future program with a loop
  bounded by an integer parameter (not a slice) would need a different unwind
  derivation (e.g. from the integer's `req` bound). Out of the corpus's scope;
  recorded for when such a program enters the corpus.

- **OQ-4 (temp crate vs. temp file + `--cfg kani`):** `cargo kani` expects a crate
  context (the grounding used a tiny crate with `[package]` + a `lib.rs`). Whether
  `run_kani` writes a throwaway `Cargo.toml` + `src/lib.rs` per item, or reuses a
  fixed scratch crate and rewrites `lib.rs`, or uses standalone `kani` on a single
  file, is a builder mechanism within REQ-4. The grounding used a per-run tiny
  crate. Recorded; not a blocker.
