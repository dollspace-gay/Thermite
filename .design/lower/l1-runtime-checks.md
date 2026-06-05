# Thermite Contracts → Executable L1 Runtime Checks (the L1 rung)
<!--
tier: 3-component
status: draft
governs: thermite-lower/src/l1.rs
thesis-refs:
  - thermite-design.md §4.2
  - thermite-design.md §6
  - thermite-design.md §8
  - thermite-design.md §5.3
  - thermite-design.md Appendix A
-->

## Summary

`thermite-lower::l1` compiles a SpecTherm contract into **executable runtime
checks** — the L1 rung of the ladder (`thermite-design.md §6`). Where
`lower.rs` emits Verus annotations for an SMT *proof* (L3), `l1.rs` emits Rust
that *executes* the contract: each `req`/`ens`/`inv`/`dec` clause and each
combinator becomes a runnable `bool` expression, and a violation is detected at
the call site **in every build profile, not just debug** (§6 table: L1 =
"Violations detected at the call site, in every build profile (not just
debug)"). This is the floor that always exists: §4.2 ("Every `spec fn` is …
compilable to a runtime check … the L1 fallback rung always exists for every
contract"). It is also what `#[slag]` blocks fall back to (§8: "The contract is
still mandatory and is enforced at L1").

This component is SHIPPED (issue **#4** L1 stage): `thermite-lower/src/l1.rs`
implements `lower_l1` and every REQ is **SHIPPED** (REQ-status table below).
The golden reference lives at `tests/golden/l1/sum.l1.rs`; the emitter is
verified by EXECUTION (compile + run via `rustc`, checks fire on violation) in
`thermite-lower/tests/l1_conformance.rs`, not by strict byte-match.

## Requirements

- **REQ-1 (L1 check-emission entry point):** `lower_l1(item) -> Result<String,
  LowerError>` (sharing `lower.rs`'s `LowerError`, REQ-9 there) emits, for a
  `FnItem`, a runtime-checked Rust function: the original body wrapped so that
  every `req` clause is asserted on entry and every `ens` clause is asserted
  against `result` on exit, and every `LoopNode`'s `inv` clauses are asserted on
  each iteration. The check macro is **always-active** (not `debug_assert!`),
  per §6 ("in every build profile, not just debug"). Derived from §6 (the L1
  rung) + §8 (slag enforces at L1).

- **REQ-2 (the always-active check primitive):** Each clause compiles to a check
  of the form `if !(COND) { thermite_contract_violation("<clause kind>",
  "<verbatim clause text>", <span/addr>); }` — an always-active check (NOT
  `debug_assert!`, which is stripped in release; §6 demands every profile). The
  violation handler is structured and deterministic (R-CODE-5): it reports the
  clause kind (`req`/`ens`/`inv`/`dec`), the verbatim clause text (the AST
  `Clause.text` the parser preserved — `ast.rs` `struct Clause { text }`), and
  the semantic address. It does NOT use `panic!`/`unwrap` in the toolchain's own
  production code (R-CODE-2); the EMITTED check at the Thermite program's runtime
  is a defined abort/diagnostic, which is the contract-violation behavior, not a
  toolchain panic. Derived from §2.4 (crisp structured feedback), §6, R-CODE-2.

- **REQ-3 (combinator L1 executable forms):** For each of the 8 frozen registry
  combinators (`thermite-spec/src/combinators.rs` `static REGISTRY`) the L1 stage
  supplies a **runnable `bool`/`usize` fn over real slices** — the executable
  counterpart of the Verus(L3) `spec fn` in `.design/lower/verus-lowering.md`
  REQ-6. These are ordinary Rust loops (no `vstd`, no `Seq`): a combinator call
  in a contract lowers to a call of its L1 fn, the closure argument becoming a
  real Rust closure. The four corpus forms are pinned in Architecture; the other
  four are pinned there too. This is the L1 half of the registry's #4 lowering
  facet (the seam OQ-2 in `.design/spec/spectherm-combinators.md` reserved).
  Derived from §4.2 ("compilable to a runtime check") + the registry seam.

- **REQ-4 (`spec fn` → executable fn):** A `SpecFnItem` (e.g. `spec_sum`) lowers
  to a real, total, terminating Rust fn (§4.2 "Every `spec fn` is total,
  terminating … and compilable to a runtime check") — recursion preserved, the
  `dec` measure NOT emitted as a runtime check on a spec fn itself (it is a
  proof-time obligation; at L1 the fn just runs). `spec_sum`'s slice match
  `[] => 0, [head, ..t] => …` lowers to a slice-length branch over `&[u32]`.
  Derived from §4.2 + Appendix A.

- **REQ-5 (`dec`/termination at L1 — honest scope):** Termination (`dec`) is a
  PROOF obligation (L3) and a BOUNDED obligation (L2, Kani #9); at **L1** a loop's
  `dec` is NOT a totality guarantee — a runtime check cannot prove termination of
  a still-running loop. The L1 contract therefore (a) asserts each loop `inv` per
  iteration (REQ-1) and (b) MAY assert that the `dec` measure strictly decreased
  since the previous iteration as a runtime *progress* check (catching a
  non-decreasing measure at the call site), but it does not and cannot certify
  termination. This boundary is recorded so the L1 rung does not overclaim
  (§6 column "Termination of the check: Guaranteed" refers to the CHECK
  terminating, not the checked program). Derived from §6, §4.1, R-HONEST-3.

- **REQ-6 (golden L1 contract):** `tests/golden/l1/sum.l1.rs` is the L1 lowering
  of `conformance/sum.th` (the route's `conformance_ops = ["sum"]`): a runnable
  Rust file whose `sum` checks `req`/`ens`/`inv` at runtime and whose `spec_sum`
  is an executable fn. It is hand-authored from this design (R-CHAR-3), byte-
  diffable against the emitted output, AND compiles + runs (a positive test calls
  `sum(&[..])` and observes the asserted result; a negative test that violates a
  clause triggers the violation handler). Derived from `goal.md` verification
  model (A), R-CHAR-3.

- **REQ-7 (`fx`/effect at L1 — compile-time only in v0.1):** The effect row's
  RUNTIME syscall sandbox (§4.1 "killed at the syscall boundary") is DEFERRED to
  issue #21 (`goal.md` "EXCLUDED from the kernel: runtime effect sandbox
  (compile-time `fx` subsumption only in v0.1)"). At L1 in v0.1, `fx` produces
  NO runtime check — the effect contract is enforced at compile time by
  `effects.rs` (`.design/lower/effect-subsumption.md`). This REQ records the
  boundary so L1 does not stub a sandbox it must not build (R-SPEC-5). Derived
  from `goal.md` scope + §4.1 + issue #21.

## Acceptance criteria

- **AC-1 (`sum` L1 golden compiles, runs, checks):** `lower_l1(parse(
  "conformance/sum.th"))` equals `tests/golden/l1/sum.l1.rs` byte-for-byte; the
  golden compiles with `rustc`/`cargo`; calling the lowered `sum(&[1,2,3])`
  returns `6` and the runtime `ens result == spec_sum(xs)` check passes; a
  fixture that deliberately corrupts the body so `ens` is violated triggers the
  violation handler (observable, not silent). (REQ-1..REQ-4, REQ-6)

- **AC-2 (checks are always-active, not `debug_assert`):** The golden contains no
  `debug_assert!`; its clause checks are present in a `--release` build
  (`grep` the golden for `debug_assert` returns nothing; a release-profile test
  observes the violation handler firing). (REQ-2)

- **AC-3 (combinator L1 forms run):** Each L1 combinator fn (REQ-3) is exercised
  by a unit test over concrete slices: `forall_in(&[2,4,6], |x| x % 2 == 0)` is
  `true`, `forall_in(&[2,3], |x| x % 2 == 0)` is `false`; `sorted(&[1,2,2,3])`
  `true`, `sorted(&[3,1])` `false`; `forall_below(&[1,2,9], 2, |x| x < 5)` `true`;
  `forall_from(&[9,1,2], 1, |x| x < 5)` `true`. Expected values hand-derived
  (R-CHAR-3). (REQ-3)

- **AC-4 (`spec_sum` executable, matches L3 semantics):** The L1 `spec_sum` over
  `&[u32]` returns the same value the L3 `spec_sum` over `Seq<u32>` denotes for
  the corpus inputs (e.g. `spec_sum(&[1,2,3]) == 6`); the two rungs agree on the
  spec function (§4.2 "Spec functions are executable"). (REQ-4)

- **AC-5 (effect/`dec` scope honesty):** The golden emits NO runtime sandbox for
  `fx pure` (REQ-7) and NO termination *guarantee* for `dec` (REQ-5); the doc and
  emitted code both record these as compile-time / proof-time obligations. A
  `grep` confirms no syscall-sandbox scaffolding in the L1 output. (REQ-5, REQ-7)

- **AC-6 (`LowerError`, never panics in the toolchain):** `lower_l1` over an
  un-lowerable construct returns `Err(LowerError)`, never panics; over the corpus
  returns `Ok`. (Toolchain-side; the emitted program's violation handler is the
  separate, intended runtime behavior of REQ-2.) (REQ-1)

## Architecture

`thermite-lower/src/l1.rs`: a recursive emitter over the `thermite-syntax` AST,
sibling to `lower.rs`, sharing the `LowerError` enum. Symbol anchors:
`struct FnItem` / `struct SpecFnItem` / `struct Contract` / `struct Clause`
(`.text`) / `struct LoopNode` in `thermite-syntax/src/ast.rs`; `static REGISTRY`
/ `fn lookup` in `thermite-spec/src/combinators.rs`.

### Exec semantics, not spec semantics

Unlike `lower.rs` (which has a spec context with `Seq`/`@`/`subrange`), L1 is
ENTIRELY exec: every clause is a Rust `bool` expression over real values, every
combinator is a real loop over `&[T]`, every `spec fn` is a real recursive fn.
There is no `vstd`, no `Seq`, no proof. A clause's verbatim `Clause.text` is
carried into the violation message for legibility (§2.4).

### The always-active check primitive (REQ-2)

```rust
macro_rules! thermite_check {  // always-active (NOT debug_assert)
    ($kind:literal, $text:literal, $cond:expr) => {
        if !($cond) { thermite_contract_violation($kind, $text); }
    };
}
```

A `req` becomes `thermite_check!("req", "<text>", <lowered cond>)` on entry; each
`ens` becomes the same on exit (after `result` is bound); each loop `inv` becomes
the same at the top of each iteration. `thermite_contract_violation` is the
defined contract-failure behavior of the *generated* program (a structured abort
/ diagnostic) — this is the intended L1 runtime behavior, distinct from a
toolchain panic, which R-CODE-2 forbids in `thermite-lower` itself.

### Combinator L1 executable forms (REQ-3)

The runnable counterparts of the Verus(L3) bodies in
`.design/lower/verus-lowering.md` REQ-6. Real Rust, real slices:

```rust
fn sorted(s: &[u32]) -> bool {
    let mut i = 1;
    while i < s.len() { if s[i - 1] > s[i] { return false; } i += 1; }
    true
}
fn forall_in(s: &[u32], p: impl Fn(u32) -> bool) -> bool {
    let mut i = 0;
    while i < s.len() { if !p(s[i]) { return false; } i += 1; }
    true
}
fn forall_below(s: &[u32], n: usize, p: impl Fn(u32) -> bool) -> bool {
    let mut i = 0;
    while i < n && i < s.len() { if !p(s[i]) { return false; } i += 1; }
    true
}
fn forall_from(s: &[u32], n: usize, p: impl Fn(u32) -> bool) -> bool {
    let mut i = n;
    while i < s.len() { if !p(s[i]) { return false; } i += 1; }
    true
}
fn exists_in(s: &[u32], p: impl Fn(u32) -> bool) -> bool {
    let mut i = 0;
    while i < s.len() { if p(s[i]) { return true; } i += 1; }
    false
}
fn count_where(s: &[u32], p: impl Fn(u32) -> bool) -> usize {
    let mut i = 0; let mut c = 0;
    while i < s.len() { if p(s[i]) { c += 1; } i += 1; }
    c
}
fn disjoint(a: &[u32], b: &[u32]) -> bool {
    let mut i = 0;
    while i < a.len() { let mut j = 0;
        while j < b.len() { if a[i] == b[j] { return false; } j += 1; } i += 1; }
    true
}
fn permutation_of(a: &[u32], b: &[u32]) -> bool {
    if a.len() != b.len() { return false; }
    // multiset equality via sorted copies (deterministic, no alloc-order dep)
    let (mut va, mut vb) = (a.to_vec(), b.to_vec());
    va.sort_unstable(); vb.sort_unstable();
    va == vb
}
```

These L1 forms are the executable mirror of the L3 quantifiers: `forall_in`
short-circuits on the first `!p`, exactly the bounded `forall|i| … ==> p(s[i])`.
The arg-kinds in the registry (`Slice`, `Index`, `Pred`, `Value`) map to the
parameter list (`&[u32]`, `usize`, `impl Fn(u32)->bool`, scalar).

### `sum` L1 lowering shape (REQ-1/REQ-4), pinned for `tests/golden/l1/sum.l1.rs`

```rust
fn spec_sum(xs: &[u32]) -> u64 {        // executable spec fn (REQ-4)
    if xs.is_empty() { 0 } else { xs[0] as u64 + spec_sum(&xs[1..]) }
}

fn sum(xs: &[u32]) -> u64 {
    thermite_check!("req", "xs.len() <= 1_000_000", xs.len() <= 1000000);
    let result = {
        let mut acc: u64 = 0;
        let mut i: usize = 0;
        while i < xs.len() {
            thermite_check!("inv", "i <= xs.len()", i <= xs.len());
            thermite_check!("inv", "acc == spec_sum(&xs[..i])", acc == spec_sum(&xs[..i]));
            thermite_check!("inv", "acc <= i as u64 * u32::MAX as u64",
                acc <= i as u64 * u32::MAX as u64);
            acc = acc + xs[i] as u64;
            i = i + 1;
        }
        acc
    };
    thermite_check!("ens", "result == spec_sum(xs)", result == spec_sum(xs));
    thermite_check!("ens", "result <= xs.len() as u64 * u32::MAX as u64",
        result <= xs.len() as u64 * u32::MAX as u64);
    result
}
```

Notes: the L1 `ens` checks reference the EXECUTABLE `spec_sum` and a real
`&xs[..i]` slice (no `Seq`) — this is why §4.2 mandates `spec fn`s be executable:
the same `spec_sum` text used in the contract is runnable. No `fx` check is
emitted (REQ-7, v0.1 compile-time-only). The `dec hi - lo` / `dec xs.len() - i`
is not a termination guarantee at L1 (REQ-5).

### Determinism (§5.3)

Pure function of the AST; clause order follows source order; combinator forms are
fixed. `permutation_of` uses sorted-copy equality rather than a `HashMap`
multiset to avoid iteration-order non-determinism (R-CODE-5).

## Verification

`cargo test -p thermite-lower` over `tests/golden/l1/`:

- **AC-1:** lower `conformance/sum.th`, `assert_eq!` against
  `tests/golden/l1/sum.l1.rs` (R-CHAR-3); compile + run the golden — `sum(&[1,2,3])
  == 6`, checks pass; a corrupted-body fixture fires the violation handler.
- **AC-2:** `grep -L debug_assert` the golden; a release-profile run observes the
  check still active.
- **AC-3:** unit tests over the combinator L1 fns with hand-derived expected
  bools/counts.
- **AC-4:** assert L1 `spec_sum(&[1,2,3]) == 6` (agrees with the L3 `Seq` form).
- **AC-5:** `grep` confirms no syscall-sandbox scaffolding and no `dec`
  termination guarantee in the L1 output.
- **AC-6:** un-lowerable-construct fixture asserts `Err(LowerError)`, no toolchain
  panic.

Gauntlet (R-DEFER-6): `cargo test -p thermite-lower`,
`cargo clippy -p thermite-lower --all-targets -- -D warnings`,
`cargo fmt --check`.

**`tests/golden/l1/` does NOT exist yet** (GREENFIELD). The `sum.l1.rs` shape
above is hand-authored into the golden (R-CHAR-3) before the builder runs; it
must compile and run under `rustc`.

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (L1 check-emission entry point) | SHIPPED | `pub fn lower_l1` in `thermite-lower/src/l1.rs` emits each `FnItem` with `req` on entry, loop `inv` per iteration, `ens` against the bound `result` on exit; verified by `sum_l1_compiles_and_runs` in `thermite-lower/tests/l1_conformance.rs` (compile+run via `rustc`). |
| REQ-2 (always-active check primitive) | SHIPPED | `emit_check_macro` writes the `thermite_check!` macro (a plain `if !(cond)`, NOT `debug_assert!`) + `thermite_contract_violation` handler; asserted by `no_debug_assert_in_emission` (AC-2) + `negative_fixture_fires_violation`. |
| REQ-3 (combinator L1 executable forms) | SHIPPED | `emit_combinator_l1_defs` reads `thermite_spec::CombinatorSig.l1` (the 8 frozen runnable forms); a combinator call lowers via `lower_expr_exec`; all 8 unit-tested over concrete slices by `combinator_l1_forms_run` (AC-3). |
| REQ-4 (`spec fn` → executable fn) | SHIPPED | `lower_spec_fn_l1`/`slice_fold_body_l1` emit the slice-length-branch recursion over `&[u32]`; `spec_sum(&[1,2,3]) == 6` exercised in the `sum_l1_compiles_and_runs` positive harness (AC-4). |
| REQ-5 (`dec`/termination L1 scope) | SHIPPED | `lower_loop_l1` emits `inv` checks only, no `dec` runtime check (OQ-3); `no_syscall_sandbox_and_no_dec_guarantee` confirms no `thermite_check!("dec",..)` (AC-5). |
| REQ-6 (golden L1 contract) | SHIPPED | `tests/golden/l1/sum.l1.rs` compiles+runs under `rustc`; the emitter's output is execution-equivalent (compiles, runs, `sum(&[1,2,3])==6`, checks fire) — verified by `sum_l1_compiles_and_runs` (verify-by-execution, AC-1). |
| REQ-7 (`fx`/effect at L1 deferred to #21) | SHIPPED | no `fx` runtime check emitted; `no_syscall_sandbox_and_no_dec_guarantee` confirms no syscall-sandbox scaffolding (REQ-7/AC-5; sandbox itself remains on #21). |

## Open questions (for the orchestrator before the builder runs)

- **OQ-1 (violation handler shape):** REQ-2's `thermite_contract_violation` needs
  a defined behavior — abort with a structured message, or unwind, or set a
  global "L1 violation" flag forge reads back. The design's §2.4 ("crisp,
  machine-readable, actionable") and §6 (detected at the call site) point at a
  structured diagnostic; the exact mechanism (process abort vs. recoverable) is a
  builder decision within REQ-2. Recorded; not a blocker.

- **OQ-2 (where L1 `spec_sum` / combinator fns live):** The L1 combinator fns and
  the executable `spec_sum` could be emitted INLINE into each lowered file or
  pulled from a shared `thermite-rt` runtime crate the lowered program depends on.
  Inlining keeps the golden self-contained (the chosen pin); a runtime crate
  scales better. This doc pins the inline shape for the golden; the mechanism is a
  builder call. Recorded; not a blocker.

- **OQ-3 (`dec` progress check, REQ-5):** Whether to emit the optional
  per-iteration "measure strictly decreased" runtime check is left open — it
  catches a class of non-termination at the call site but adds a check the design
  does not strictly mandate. Conservative v0.1 default: emit `inv` checks only,
  document `dec` as proof-time. Recorded; not a blocker.
