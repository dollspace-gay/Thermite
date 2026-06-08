# Thermite AST → Verus-Annotated Rust Lowering (L3 emission)
<!--
tier: 3-component
status: draft
governs: thermite-lower/src/lower.rs
thesis-refs:
  - thermite-design.md §3
  - thermite-design.md §4.1
  - thermite-design.md §4.2
  - thermite-design.md §6
  - thermite-design.md §5.3
  - thermite-design.md Appendix A
-->

## Summary

`thermite-lower::lower` is the **L3 emission stage**: it walks a validated
`thermite-syntax` `Program` and emits a single Verus source file — a
`use vstd::prelude::*; verus! { … } fn main() {}` unit whose
`requires`/`ensures`/`invariant`/`decreases` annotations are the Thermite
contract, and whose body is the lowered Thermite body. Forge (#5/#6) hands that
file to the `verus` binary; a `0 errors` result is the L3 certificate
(`thermite-design.md §6`). The corpus programs `conformance/sum.th` and
`conformance/binary_search.th` lower to `tests/golden/lower/sum.verus.rs` and
`tests/golden/lower/binary_search.verus.rs`, each of which **must itself pass
`verus` with 0 errors** — that is the load-bearing external truth this component
is pinned against (`goal.md` "Verus/Kani/Z3 golden files").

This doc is GREENFIELD / FORWARD-LOOKING. Only the empty
`thermite-lower/src/lib.rs` scaffold root exists (no `lower.rs`). Every REQ is
**NOT-STARTED**, blocked on issue **#4**. The exact verified Verus forms below
were produced by running the real `verus 0.2026.05.24` binary during authoring;
they are the lowering contract the builder reproduces, not guesses.

## Requirements

- **REQ-1 (file frame + `fn`/`spec fn` signature lowering):** `lower(program) ->
  Result<String, LowerError>` emits one Verus file: the fixed prelude
  `use vstd::prelude::*;`, a `verus! { … }` block containing the lowered items in
  source order, and a trailing `fn main() {}`. A `FnItem` lowers to a Verus `fn`
  whose return type binds the result name (`-> (result: T)`) so the `ens` clauses
  can mention `result` (`thermite-design.md §4.1` "Must mention `result`");
  `req`→`requires`, each `ens`→`ensures`, `fx pure`→no Verus effect annotation
  (a Verus `fn` is pure by default; §4.1). A `SpecFnItem` lowers to a Verus
  `spec fn` carrying `decreases <dec>` (§4.2 "No spec-level recursion without a
  `dec` measure"). Derived from §3 ("transpile to Verus"), §4.1, Appendix A.

- **REQ-2 (type lowering):** Thermite `Type` lowers to its Verus/Rust spelling:
  `Prim(U32|U64|Usize|Bool)`→`u32|u64|usize|bool`; `Unit`→`()`;
  `Ref{mutable:false, Slice(U32)}`→`&[u32]`; `Generic{"Option", Usize}`→
  `Option<usize>`. No lifetimes are emitted (§4.4 "Explicit lifetimes →
  region inference"). The corpus exercises exactly `&[u32]`, `u64`, `usize`,
  `u32`, `Option<usize>`. Derived from `ast.rs` `enum Type` + §4.4.

- **REQ-3 (expression lowering — exec position):** Each `Expr` lowers to the
  matching Verus/Rust surface in **executable (body)** position:
  `IntLit`/`BoolLit` (an underscored literal like `1_000_000` lowers to its
  `_`-stripped numeric value `1000000` — ast.md REQ-6; the verbatim `raw` is
  kept on the AST node, not emitted);
  `Path(["u32","MAX"])`→`u32::MAX`; `Call`/`MethodCall`/`Field`; `Binary` with the
  `BinOp`→operator map (`Add`→`+`, `Le`→`<=`, `And`→`&&`, …); `Index` over the
  four `IndexArg` forms (`a[i]`, `a[..i]`, `a[i..]`, `a[i..j]`); `Cast`→`as T`;
  `Ref`→`&`/`&mut`; `Match` and `If`. `MethodCall{name:"len"}`→`.len()`.
  Derived from `ast.rs` `enum Expr` + Appendix A.

- **REQ-4 (statement + loop lowering):** `Block` lowers with its `stmts` then an
  optional `tail` expression (the block's value, e.g. `sum`'s final `acc`).
  `Stmt::Let{mutable}`→`let`/`let mut`; `Assign`→`x = e;`; `Return`→`return e;`;
  `If`→Rust `if`/`else`. A `Stmt::Loop(LoopNode)` lowers to a Verus loop carrying
  EVERY `inv` as an `invariant` clause and the single `dec` as `decreases`
  (§4.1 "Mandatory on every `loop`/`while`. Termination is proved by default").
  `LoopKind::While(c)`→`while c { … }`; `LoopKind::Loop`→`loop { … }`. Derived
  from `ast.rs` `enum Stmt` / `struct LoopNode` / `enum LoopKind` + §4.1.

- **REQ-5 (spec-context lowering — slices become `Seq`, the verified contract):**
  In **spec position** (`spec fn` bodies, `requires`/`ensures`/`invariant`), a
  `&[T]` value `xs` is referenced through its Verus view `xs@` (a `Seq<T>`), and a
  `spec fn` over a slice takes `Seq<T>` (NOT `&[T]`) — running `verus` on the
  naive `&[u32]` spec-fn form fails with `the trait bound &[u32]: Integer is not
  satisfied` (recorded finding, see Architecture). A slice expression
  `&xs[..i]` in spec position lowers to `xs@.subrange(0, i as int)`; `xs[i]` in
  spec position lowers to `xs@[i as int]`; `.len()` in spec position lowers to
  `xs@.len()` (or `xs.len()` where Verus coerces). The `spec_sum` recursion
  `match xs { [] => 0, [head, ..t] => head as u64 + spec_sum(t) }` lowers to the
  verified `Seq` recursion `if xs.len() == 0 { 0 } else { xs[0] as nat +
  spec_sum(xs.drop_first()) }`. Derived from §3 ("transpile to Verus"), §4.2
  ("Spec functions are … compilable"), and the `verus` binary's `Seq`/view model.

- **REQ-6 (combinator Verus(L3) definitions — the #4 lowering facet):** This
  component supplies the per-combinator **Verus(L3) `spec fn` definition** that
  `.design/spec/spectherm-combinators.md` (OQ-2) deferred to #4. For each of the
  8 frozen registry combinators (`thermite-spec/src/combinators.rs` `static
  REGISTRY`) the lowerer emits/links a `spec fn` whose body is the frozen
  bounded-quantifier form with a **frozen `#[trigger]`** on the predicate
  application (§4.2 "hand-tuned, frozen SMT triggers"). The four corpus
  combinators (`sorted`, `forall_in`, `forall_below`, `forall_from`) are pinned
  with verified bodies in the Architecture section; the other four
  (`exists_in`, `count_where`, `permutation_of`, `disjoint`) carry their frozen
  forms there too. A combinator call in a contract lowers to a call of its
  `spec fn`, the closure argument becoming a Verus `spec_fn` literal. Derived
  from §4.2 + the registry's named #4 seam (OQ-2 in
  `.design/spec/spectherm-combinators.md`).

- **REQ-7 (proof-aid emission — the lowering is what verifies, not a guess):**
  Where a corpus program does not verify from its bare annotations, the lowering
  contract INCLUDES the proof aids the obligation needs, emitted deterministically
  (R-CODE-5) — never `assume(false)` / `#[verifier::external]` / weakened
  contracts (R-DEFER-9). For `sum` this is: (a) an induction lemma
  `lemma_sum_push` relating `spec_sum(prefix[0..k+1])` to
  `spec_sum(prefix[0..k]) + xs[k]`, called in the loop; (b) a `by(nonlinear_arith)`
  bound discharging the `acc + xs[i]` overflow from `inv#3 + req` (Appendix A's
  asserted "overflow: discharged from inv#3 + req"); (c) the precondition
  `xs.len() <= 1_000_000` lifted into the loop invariant; (d) a `=~=` extensionality
  assert closing `subrange(0, len) == xs@`. For `binary_search` this is the
  loop-exit `assert(forall_in(...)) by { … }` case-split. These aids are pinned in
  the golden files (REQ-8) and are part of the contract. Derived from §6 (L3 is a
  real SMT proof), R-DEFER-9, Appendix A.

- **REQ-8 (golden-file contract — VERIFY, don't byte-match):** The lowerer's
  verification target is: the emitted Verus, run through the real `verus`
  binary, passes with `verification results:: N verified, 0 errors`, AND the
  emitted `requires`/`ensures`/`invariant`/`decreases` are equivalent to the
  corpus contracts (no weakening — R-DEFER-9). `tests/golden/lower/{sum,
  binary_search}.verus.rs` are the verus-verified REFERENCE (known-good output,
  hand-authored from this design, R-CHAR-3 — never regenerated from the lowerer)
  proving L3 is achievable for the corpus. **The lowerer is NOT required to
  byte-match the hand-authored PROOF AIDS** (`lemma_sum_push`'s induction, the
  `binary_search` case-split): reproducing them verbatim would force per-program
  HARDCODING — over-fitting / a cheat the critic must reject. Instead the lowerer
  emits its OWN proof aids via GENERAL shape-keyed templates (REQ-7) so the
  emitted output verifies. The MECHANICAL lowering (signature, contracts, types,
  body, combinator calls) should match the golden's corresponding lines; the
  proof-aid section need only make `verus` succeed and must be shape-general.
  The divergences the critic pins are: `verus(emitted) ≠ 0 errors`; an emitted
  contract `≠` the corpus contract; or a proof aid that is per-program HARDCODED
  rather than derived from program shape. Derived from `goal.md` verification
  model (A), R-CHAR-3, R-DEFER-9.

- **REQ-9 (`LowerError`, no panics):** `lower` returns `Result<String,
  LowerError>`; `LowerError` is `thermite-lower`'s OWN error enum, born with this
  first fallible function (per `.design/scaffold/workspace.md` REQ-3 "each crate
  introduces its OWN error enum … when its first fallible function lands"). It is
  span-bearing (reusing `thermite_syntax::lexer::Span`) and `Display`-able, with
  a variant for an un-lowerable construct (e.g. a combinator call whose callee is
  not in the registry — though validation (#2) should have caught it; the lowerer
  re-checks defensively). No `unwrap`/`expect`/`panic!` in production
  (R-CODE-2 / R-APG-1). Derived from R-CODE-2, workspace.md REQ-3.

- **REQ-12 (`break` / `continue` lowering + the verification semantics — NEW,
  #93):** `Stmt::Break` lowers to the Verus-native `break;`, `Stmt::Continue` to
  `continue;` (Verus 0.2026 supports both natively inside `while`/`loop`). The
  emission is trivial; the LOAD-BEARING contract is the VERIFICATION SEMANTICS the
  lowering MUST preserve so the proof obligations BITE (R-DEFER-9 — break/continue
  must NOT let a loop skip its invariant or launder termination). The lowering
  emits the loop annotations exactly as REQ-4 does, and Verus enforces:

  - **(a) Invariant at every `continue` AND at re-entry.** A plain Verus
    `invariant` clause must hold at the top of EVERY iteration AND at every
    `continue` point (a `continue` re-enters the loop head). A `continue` that
    leaves the loop with the invariant broken is L0. GROUNDED below (a `continue`
    that bumps the accumulator past its bound without advancing the index →
    "loop invariant not satisfied … at this continue", L0). The lowering does NOT
    add or weaken any invariant for break/continue — it emits the corpus `inv`s
    verbatim, and Verus checks them at the continue.

  - **(b) `continue` respects the `decreases` measure.** In a TERMINATING loop
    (non-`diverge`, so a `decreases` is emitted — REQ-4), the measure must strictly
    decrease BEFORE a `continue` (the `continue` re-enters the loop, so it is a
    back-edge that owes the termination obligation). A `continue` that does NOT
    decrease the measure (e.g. `continue;` without advancing the loop variable) is
    L0 with the exact Verus error "decreases not satisfied at continue". GROUNDED
    below. This is the crux pin: `continue` is a loop back-edge → it inherits the
    SAME decreases obligation as the implicit loop-end back-edge.

  - **(c) `break` exits; post-loop facts come from the loop `ensures`, NOT
    `invariant ∧ ¬guard`.** A `break` can exit the loop while the guard is STILL
    TRUE, so the usual "after a `while c` loop, `¬c` holds" reasoning does NOT
    apply at a break. Verus's model (GROUNDED — see Architecture): a plain
    `invariant` clause must ALSO hold at every `break` point (Verus checks the
    invariant at the loop EXIT, including break); an invariant that is true at
    re-entry but NOT at break must instead be written `invariant_except_break`;
    and what is provable AFTER the loop is the loop's `ensures` clause (true at
    break OR normal exit). The lowering's contract: a Thermite `inv` lowers to a
    Verus `invariant` (held at re-entry AND break); if a Thermite program needs a
    re-entry-only fact (true on continue, broken at break) that is a FUTURE
    `invariant_except_break` need (OQ-5) — the v0.1 corpus break-loop holds all
    its `inv`s at the break point, so the plain `invariant` suffices and a loop
    `ensures` (the fn's `ens`, threaded onto the loop) carries the post-break
    fact. GROUNDED below (a `break` early-exit certifies L3 once the break-true
    fact is the loop `ensures` and the re-entry-only fact is
    `invariant_except_break`).

  - **(d) `fx diverge` loop: break/continue unconstrained by `decreases`.** A
    `fx diverge` fn emits NO `decreases` on its loop and carries
    `#[verifier::exec_allows_no_decreases_clause]` on the fn (REQ-4 / the existing
    `fn_is_diverge` path). In such a loop, `break` and `continue` carry NO
    decreases obligation (there is no measure) — the editor's event loop
    `while true { let k = read(); if k == quit { break; } … }` certifies its
    INVARIANTS (partial correctness) with `break` exiting cleanly on Ctrl-Q and
    `continue` skipping an iteration, no termination claim. GROUNDED below. forge
    STRUCTURALLY caps such a fn at L1 (the #88 diverge cap,
    `degrade-ladder.md` REQ-9), regardless of how strong the loop `ensures` is —
    break/continue do not change the cap.

  - **(e) The cap is diverge-ONLY.** A NON-diverge loop with break/continue STILL
    proves termination (its `dec` + the per-`continue` decreases obligation) AND
    its invariants → L3. break/continue are NOT a termination escape hatch: a
    non-diverge loop WITHOUT a `decreases` still fails Verus ("loop must have a
    decreases clause") — GROUNDED below (the negative control). The #87/#88
    exemptions stay diverge-only (`fn_is_diverge in lower.rs`).

  This component lowers `break`/`continue` and relies on Verus to enforce
  (a)–(e); it MUST NOT suppress the obligations (no `assume`, no `external`,
  no dropping a `decreases` for a non-diverge loop — R-DEFER-9). The `Stmt`
  match-arm ripple this introduces is pinned in `ast.md` REQ-12 (Architecture →
  the `Stmt` ripple) — `lower.rs`/`l1.rs`/`l2.rs` each gain `Break`/`Continue`
  emit arms. Derived from §4.1 (the loop model), §6 (L3 = total correctness; a
  diverge loop caps at L1), R-DEFER-9, `degrade-ladder.md` REQ-9.

## Acceptance criteria

- **AC-1 (`sum` lowers + VERIFIES):** running the real `verus` binary on
  `lower(parse("conformance/sum.th"))` exits 0 with `N verified, 0 errors`; the
  emitted `requires`/`ensures`/`invariant`/`decreases` are equivalent to
  `sum.th`'s contracts (R-DEFER-9, no weakening); the emitted mechanical lowering
  (signature/types/body/combinator calls) matches the corresponding lines of
  `tests/golden/lower/sum.verus.rs`; the emitted proof aids are shape-general
  (REQ-7), NOT per-program hardcoded. (`sum.verus.rs` itself verifies `5
  verified, 0 errors` — the reference.) (REQ-1..REQ-8)

- **AC-2 (`binary_search` lowers + VERIFIES):** running `verus` on
  `lower(parse("conformance/binary_search.th"))` exits 0 with `N verified, 0
  errors`; emitted contracts equivalent to `binary_search.th`'s; mechanical
  lowering matches `tests/golden/lower/binary_search.verus.rs`'s corresponding
  lines; proof aids shape-general, not hardcoded. (`binary_search.verus.rs`
  itself verifies `2 verified, 0 errors` — the reference.) (REQ-1..REQ-8)

- **AC-3 (combinator Verus(L3) forms verify in isolation, non-vacuous):** Each
  pinned combinator `spec fn` body (REQ-6) compiles under `verus` and is
  non-vacuous: a concrete satisfying instance proves and a concrete violating
  instance fails (the `forall_in` non-vacuity sanity proof in Verification
  verified `1 verified, 0 errors` during authoring). No combinator body is
  `true` (R-DEFER-9, §7 anti-vacuity intent). (REQ-6)

- **AC-4 (type + expression mapping table is total over the corpus):** Every
  `Type` and `Expr` node the two corpus programs contain has a row in the
  Architecture mapping tables, and the emitted spelling matches. Mechanically: a
  unit test lowers each node kind present in the corpus and asserts the substring
  appears in the golden output. (REQ-2, REQ-3, REQ-4, REQ-5)

- **AC-5 (no proof cheats):** The golden files contain no `assume(false)`, no
  `#[verifier::external]`, no `#[verifier::external_body]`, no `#[slag]`, and no
  contract weakened to `true`; the `ensures` clauses are exactly the corpus
  contracts (R-DEFER-9). Mechanically: `grep` the golden files for the forbidden
  tokens (must be absent) and diff the emitted `requires`/`ensures` against the
  parsed corpus contract. (REQ-7, REQ-8)

- **AC-6 (`LowerError`, never panics):** Lowering a program with an un-lowerable
  construct returns `Err(LowerError::…)`, never panics; `lower` over the corpus
  returns `Ok`. (REQ-9)

- **AC-7 (`break`/`continue` lower + the verification obligations bite —
  GROUNDED, NEW, #93):** the four pinned probes, run through the real `verus`
  binary (Verification section):
  - a terminating `while … dec …` whose `continue` skips an element while
    preserving the invariant + decreasing the measure → L3 (`2 verified, 0
    errors`);
  - a `continue` that BREAKS the invariant → L0 ("loop invariant not satisfied …
    at this continue");
  - a `continue` that does NOT decrease the measure → L0 ("decreases not
    satisfied at continue");
  - a `break` early-exit whose post-loop fact is the loop `ensures` → L3 (`2
    verified, 0 errors`), with the re-entry-only fact as `invariant_except_break`;
  - a `fx diverge` loop with `break`/`continue` (no `decreases`,
    `#[verifier::exec_allows_no_decreases_clause]`) → verifies its invariants
    (`2 verified, 0 errors`), capped at L1 by the #88 gate;
  - NEGATIVE CONTROL: a NON-diverge loop WITHOUT a `decreases` → L0 ("loop must
    have a decreases clause") — the exemption is diverge-only. (REQ-12)

## Architecture

The component is `thermite-lower/src/lower.rs`: a recursive emitter over the
`thermite-syntax` AST producing a Verus source `String`, plus the `LowerError`
enum. It is downstream of `thermite-spec::validate` (a contract that fails
validation never reaches the lowerer — `.design/spec/spectherm-combinators.md`
"boundary role"). Symbol anchors: `struct FnItem` / `struct SpecFnItem` /
`struct Contract` / `struct LoopNode` / `enum Expr` / `enum Type` in
`thermite-syntax/src/ast.rs`; `static REGISTRY` / `fn lookup` in
`thermite-spec/src/combinators.rs`.

### Two lowering contexts: exec vs. spec

Verus distinguishes **exec** code (function bodies) from **spec** code
(`requires`/`ensures`/`invariant`/`decreases` and `spec fn` bodies). The same
Thermite expression lowers differently by context — this is the central finding
of authoring against the real binary:

- A `&[T]` slice in **exec** position is plain Rust `&[u32]`; in **spec**
  position it is referenced as `xs@`, a `vstd` `Seq<T>`.
- A `spec fn` over a slice takes `Seq<T>`, NOT `&[T]`. Running `verus` on the
  naive `spec fn spec_sum(xs: &[u32])` with `spec_sum(&xs[1..])` fails:
  `the trait bound &[u32]: Integer is not satisfied` / `expected int, found
  RangeFrom`. The verified form takes `Seq<u32>` and recurses on
  `xs.drop_first()`.
- A spec slice `&xs[..i]` lowers to `xs@.subrange(0, i as int)`; a spec index
  `xs[i]` to `xs@[i as int]`. The cast `i as int` is mandatory — Verus spec
  indices are `int`.

### `fn`/`spec fn` signature lowering (REQ-1)

```
fn NAME(P: T, …) -> RET             fn NAME(P: T, …) -> (result: RET)
  req REQ                  ===>         requires LOWER_SPEC(REQ),
  ens ENS1                              ensures
  ens ENS2                                  LOWER_SPEC(ENS1),
  fx  pure                                  LOWER_SPEC(ENS2),
{ BODY }                              { LOWER_EXEC(BODY) }
```

`fx pure` emits no annotation (Verus `fn` is pure by default). The return binder
`(result: RET)` is what lets `ens` mention `result` (§4.1). A `spec fn` lowers
with `decreases LOWER_SPEC(dec)` and a `Seq`-typed slice parameter (REQ-5).

### Type mapping (REQ-2)

| Thermite `Type` | Verus/Rust |
|---|---|
| `Prim(U32)` / `U64` / `Usize` / `Bool` | `u32` / `u64` / `usize` / `bool` |
| `Unit` | `()` |
| `Ref{mutable:false, Slice(Prim(U32))}` | `&[u32]` (exec); the view `xs@: Seq<u32>` (spec) |
| `Generic{"Option", Prim(Usize)}` | `Option<usize>` |

### Expression mapping (REQ-3) — operator and node table

| `Expr` / `BinOp` | exec spelling | spec spelling |
|---|---|---|
| `IntLit{value:1000000,raw:"1_000_000"}` | `1000000` (value, `_`-stripped) | same |
| `Path(["u32","MAX"])` | `u32::MAX` | `u32::MAX` |
| `MethodCall{name:"len"}` on `xs` | `xs.len()` | `xs@.len()` |
| `Index{Single(i)}` `xs[i]` | `xs[i]` | `xs@[i as int]` |
| `Index{RangeTo(i)}` `xs[..i]` (under `&`) | `&xs[..i]` | `xs@.subrange(0, i as int)` |
| `Cast{u64}` `e as u64` | `e as u64` | `e as nat` where a `nat` accumulator is used |
| `Binary{Add..Or}` | `+ - * / == != < <= > >= && \|\|` | same |
| `Match`/`If` | Rust `match`/`if` | spec `match`/`if` |
| `Closure{[x], body}` | (exec n/a in corpus) | `\|x: T\| LOWER_SPEC(body)` (Verus `spec_fn`) |

### `break` / `continue` lowering + the verification model (REQ-12, #93)

The EMISSION is one statement each: `Stmt::Break` → `break;`, `Stmt::Continue`
→ `continue;`, placed verbatim in the lowered loop body (Verus has native
`break`/`continue` inside `while`/`loop`). `lower_stmt` / `lower_loop_body` in
`lower.rs` (and the `l1.rs`/`l2.rs` mirrors) gain the two arms; the broader
`match Stmt` ripple is pinned in `ast.md` REQ-12.

The CONTRACT is the verification model below, which the lowering preserves by
emitting the loop annotations unchanged and letting Verus enforce them. The model
was established by running the real `verus 0.2026.05.24` binary (Verification):

- **`continue` is a loop back-edge.** Verus treats a `continue` exactly like
  falling off the end of the loop body: it must (a) re-establish every plain
  `invariant`, and (b) in a terminating loop, satisfy the `decreases` (the
  measure must have strictly decreased since loop entry). Both obligations BITE
  at the `continue` site, with distinct errors ("loop invariant not satisfied …
  at this continue" / "decreases not satisfied at continue").

- **`break` is a loop EXIT.** Verus distinguishes three loop annotations:
  - plain `invariant` — must hold at re-entry AND at every `break`/loop exit;
  - `invariant_except_break` — must hold at re-entry/continue but NOT at break;
  - `ensures` (on the loop) — what is true AFTER the loop (at break OR normal
    exit), the ONLY thing provable in post-loop code.

  This is the load-bearing finding: after a `break`, the loop GUARD is NOT known
  false (break can exit mid-guard-true), so post-loop reasoning is the loop
  `ensures`, NOT `invariant ∧ ¬guard`. The lowering maps a Thermite `inv` to a
  plain Verus `invariant` (so it must hold at break too); a Thermite `ens`
  (function postcondition) is what the post-loop code must establish — for a
  break-bearing loop the relevant facts thread onto the loop `ensures`. The v0.1
  corpus break-loops hold all their `inv`s at the break point (so plain
  `invariant` suffices); a future re-entry-only invariant would need
  `invariant_except_break` (OQ-5).

- **`fx diverge` loop.** `fn_is_diverge` (the existing #87/#88 path) suppresses
  the loop `decreases` and emits `#[verifier::exec_allows_no_decreases_clause]`
  on the fn. break/continue then carry NO decreases obligation — they exit /
  skip freely while Verus still checks the loop invariants (partial correctness).
  forge caps the fn at L1 (`degrade-ladder.md` REQ-9). The cap is STRUCTURAL
  (from the `fx diverge` declaration), decided before any prover; break/continue
  do not affect it.

The lowering MUST NOT suppress any of these obligations (no `assume`, no
`external`, no dropping a non-diverge `decreases`) — that would be a proof cheat
(R-DEFER-9). break/continue change WHERE the obligations are checked (a continue
adds a back-edge), never WHETHER.

### Combinator Verus(L3) definitions + frozen triggers (REQ-6)

These are the #4 lowering-facet bodies the registry's OQ-2 seam reserved. The
predicate parameter is a Verus `spec_fn(T) -> bool`; the frozen `#[trigger]` sits
on the predicate application `p(s[i])` so the solver instantiates the quantifier
exactly at the points the proof needs (§4.2 "hand-tuned, frozen SMT triggers").
The four corpus forms below are **verified** (see Verification).

```verus
spec fn sorted(s: Seq<u32>) -> bool {
    forall|i: int, j: int| 0 <= i <= j < s.len() ==> s[i] <= s[j]
}
spec fn forall_in(s: Seq<u32>, p: spec_fn(u32) -> bool) -> bool {
    forall|i: int| 0 <= i < s.len() ==> #[trigger] p(s[i])
}
spec fn forall_below(s: Seq<u32>, n: int, p: spec_fn(u32) -> bool) -> bool {
    forall|i: int| 0 <= i < n && i < s.len() ==> #[trigger] p(s[i])
}
spec fn forall_from(s: Seq<u32>, n: int, p: spec_fn(u32) -> bool) -> bool {
    forall|i: int| n <= i < s.len() ==> #[trigger] p(s[i])
}
```

The remaining four §4.2-named combinators (registry `static REGISTRY`), frozen
forms (carried for skill/registry completeness; not corpus-exercised, so
isolation-verified only under AC-3):

```verus
spec fn exists_in(s: Seq<u32>, p: spec_fn(u32) -> bool) -> bool {
    exists|i: int| 0 <= i < s.len() && #[trigger] p(s[i])
}
spec fn count_where(s: Seq<u32>, p: spec_fn(u32) -> bool) -> nat
    decreases s.len()
{   // recursive count — total + terminating (§4.2)
    if s.len() == 0 { 0 }
    else { (if p(s[0]) { 1nat } else { 0nat }) + count_where(s.drop_first(), p) }
}
spec fn disjoint(a: Seq<u32>, b: Seq<u32>) -> bool {
    forall|i: int, j: int|
        (0 <= i < a.len() && 0 <= j < b.len()) ==> #[trigger] a[i] != #[trigger] b[j]
}
spec fn permutation_of(a: Seq<u32>, b: Seq<u32>) -> bool {
    a.to_multiset() == b.to_multiset()   // vstd Seq::to_multiset
}
```

OQ-3: `count_where`/`disjoint`/`permutation_of` are not corpus-exercised; their
exact frozen trigger tuning is verified only in isolation (AC-3) until a corpus
program uses them. `permutation_of` via `to_multiset` is the candidate form;
flagged as least-confident (see Open questions).

### `sum` — the verified lowering (REQ-7), pinned

The golden `tests/golden/lower/sum.verus.rs` is exactly this (verified
`5 verified, 0 errors`):

```verus
spec fn spec_sum(xs: Seq<u32>) -> nat
    decreases xs.len()
{
    if xs.len() == 0 { 0 } else { xs[0] as nat + spec_sum(xs.drop_first()) }
}

proof fn lemma_sum_push(xs: Seq<u32>, k: int)
    requires 0 <= k < xs.len(),
    ensures spec_sum(xs.subrange(0, k + 1)) == spec_sum(xs.subrange(0, k)) + xs[k] as nat,
    decreases k,
{
    if k == 0 {
        assert(xs.subrange(0, 1).drop_first() =~= xs.subrange(0, 0));
    } else {
        lemma_sum_push(xs.drop_first(), k - 1);
        assert(xs.subrange(0, k + 1).drop_first() =~= xs.drop_first().subrange(0, k));
        assert(xs.subrange(0, k).drop_first() =~= xs.drop_first().subrange(0, k - 1));
    }
}

fn sum(xs: &[u32]) -> (result: u64)
    requires xs.len() <= 1000000,
    ensures
        result as nat == spec_sum(xs@),
        result <= xs.len() as u64 * u32::MAX as u64,
{
    let mut acc: u64 = 0;
    let mut i: usize = 0;
    while i < xs.len()
        invariant
            i <= xs.len(),
            xs.len() <= 1000000,
            acc as nat == spec_sum(xs@.subrange(0, i as int)),
            acc <= i as u64 * u32::MAX as u64,
        decreases xs.len() - i,
    {
        proof { lemma_sum_push(xs@, i as int); }
        assert(acc + xs[i as int] as u64 <= (i as u64 + 1) * u32::MAX as u64) by(nonlinear_arith)
            requires acc <= i as u64 * u32::MAX as u64, i < xs.len(), xs.len() <= 1000000;
        acc = acc + xs[i] as u64;
        i = i + 1;
    }
    assert(xs@.subrange(0, xs.len() as int) =~= xs@);
    acc
}
```

Mapping notes the lowerer encodes: the corpus `acc: u64` plus
`ens result == spec_sum(xs)` forces `spec_sum: Seq<u32> -> nat` with
`result as nat == spec_sum(xs@)` (a `u64`-valued `spec_sum` over-/under-flows the
`nat` invariant relation). The corpus `while i < xs.len()` maps directly to a
Verus `while`. The corpus comment "overflow: discharged from inv#3 + req" is the
`by(nonlinear_arith)` assertion. `lemma_sum_push` is the proof aid the
tail-growing loop needs to reconcile with the head-recursive `spec_sum`.

### `binary_search` — the verified lowering (REQ-7), pinned

The golden `tests/golden/lower/binary_search.verus.rs` is exactly this (verified
`2 verified, 0 errors`); note the corpus `loop` with the interior
`if lo == hi { return None; }` is preserved (not rewritten to `while`):

```verus
fn binary_search(haystack: &[u32], needle: u32) -> (result: Option<usize>)
    requires sorted(haystack@),
    ensures
        match result {
            Some(i) => i < haystack.len() && haystack@[i as int] == needle,
            None => forall_in(haystack@, |x: u32| x != needle),
        },
{
    let mut lo: usize = 0;
    let mut hi: usize = haystack.len();
    loop
        invariant
            lo <= hi <= haystack.len(),
            sorted(haystack@),
            forall_below(haystack@, lo as int, |x: u32| x < needle),
            forall_from(haystack@, hi as int, |x: u32| x > needle),
        decreases hi - lo,
    {
        if lo == hi {
            assert(forall_in(haystack@, |x: u32| x != needle)) by {
                assert forall|k: int| 0 <= k < haystack@.len()
                    implies (|x: u32| x != needle)(haystack@[k]) by {
                    if k < lo as int {
                        assert((|x: u32| x < needle)(haystack@[k]));
                    } else {
                        assert((|x: u32| x > needle)(haystack@[k]));
                    }
                }
            }
            return None;
        }
        let mid = lo + (hi - lo) / 2;
        if haystack[mid] == needle {
            return Some(mid);
        }
        if haystack[mid] < needle {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
}
```

Mapping notes: the corpus chained `inv lo <= hi && hi <= haystack.len()` lowers
to a single `invariant lo <= hi <= haystack.len()` (Verus chained-compare). The
loop-exit branch needs the `assert(forall_in(...)) by { … }` case-split proof aid
(REQ-7): at `lo == hi`, index `k < lo` is `< needle` (from `forall_below`) and
`k >= hi` is `> needle` (from `forall_from`), so `!= needle`. The closures in the
asserts are re-stated as the SAME literals as the invariants so the frozen
triggers fire. The `decreases hi - lo` is the corpus `dec`.

### Determinism (§5.3)

Emission is a pure function of the AST (no wall-clock, no env, no HashMap
iteration order in output — REQ ordering follows `Contract.ens` source order,
items follow `Program.items` order). Byte-identical output for byte-identical
input (R-CODE-5, §5.3).

## Verification

`cargo test -p thermite-lower` over `tests/golden/lower/` (this route's
`reference` in `tooling/spec-routes.toml`):

- **AC-1/AC-2:** lower the parsed corpus programs and `assert_eq!` the emitted
  `String` against the golden files (R-CHAR-3 — golden hand-authored from this
  doc). A companion harness shells `verus <golden>` and asserts exit 0 + the
  expected `N verified, 0 errors` line (R-CODE-4: subprocess exit status checked,
  never swallowed). Authoring runs (real `verus 0.2026.05.24`):
  - `sum`: `verification results:: 5 verified, 0 errors`
  - `binary_search`: `verification results:: 2 verified, 0 errors`
- **AC-3:** a Verus fixture per combinator body + a non-vacuity proof; authoring
  ran the `forall_in` sanity (a 1-element seq where the predicate holds proves;
  the relation is not `true`): `verification results:: 1 verified, 0 errors`.
- **AC-4:** unit tests assert each corpus `Type`/`Expr` node's lowered substring.
- **AC-5:** `grep` the golden files for `assume(false)` / `external` / `slag` /
  `ensures true` (all absent); diff emitted `requires`/`ensures` vs parsed corpus.
- **AC-6:** an un-lowerable-construct fixture asserts `Err(LowerError)`, no panic.

### `break`/`continue` verification semantics — GROUNDED (AC-7, REQ-12, #93)

Run against the real `verus 0.2026.05.24.ecee80a` binary THIS amendment. Each
probe is a lowered loop in the v0.1 shape; the results ARE the contract the
builder's emitted lowering must reproduce, and the hand-authored break/continue
golden(s) under `tests/golden/lower/` are pinned from these (R-CHAR-3).

**(1) terminating `while` + `dec`, `continue` preserves invariant + decreases →
L3.** Skip elements above a bound, summing the rest; the index advances BEFORE
the `continue`, the invariant `acc <= i*BOUND` holds at the continue:

```
$ verus p1_continue_ok.rs
verification results:: 2 verified, 0 errors        (L3)
```

**(2) `continue` that BREAKS the invariant → L0.** The same loop, but the
`continue` bumps `acc` past its bound without advancing the matching index:

```
$ verus p2_continue_bad_inv.rs
error: invariant not satisfied at end of loop body
   |   acc <= i as u64 * 1000000,   <-- failed this invariant
   |   continue;                    <-- at this continue
verification results:: 1 verified, 1 errors        (L0 — the invariant obligation BITES at the continue)
```

**(3) `continue` that does NOT decrease the measure → L0.** A `continue;` without
advancing the loop variable (the back-edge owes the decreases):

```
$ verus p3_continue_bad_dec.rs
error: decreases not satisfied at continue
   |   continue;   <-- NO measure decrease: i unchanged before continue
verification results:: 1 verified, 1 errors        (L0 — the decreases obligation BITES at the continue)
```

**(4) `break` early-exit → L3.** A find-loop that breaks on a hit. The
re-entry-only fact (`found == false`) is `invariant_except_break`; the
break-true fact (`found == true ==> len > 0`) is the loop `ensures` (post-loop
reasoning is the loop `ensures`, NOT `invariant ∧ ¬guard`):

```
$ verus p4b_break_ok.rs            // invariant_except_break + loop ensures
verification results:: 2 verified, 0 errors        (L3)
```

NOTE (the load-bearing finding): writing `found == false` as a PLAIN `invariant`
(not `invariant_except_break`) FAILS — `error: loop invariant not satisfied …
at this loop exit` — because a plain `invariant` must also hold at the `break`,
and `found` is `true` there. This is exactly why (c) above distinguishes
`invariant` / `invariant_except_break` / `ensures`. The lowering's Thermite-`inv`
→ Verus-`invariant` mapping is sound for the v0.1 corpus (whose `inv`s hold at
break); a re-entry-only Thermite invariant would need the
`invariant_except_break` lowering (OQ-5).

**(5) `fx diverge` loop with `break` AND `continue`, no `decreases` → verifies
(capped L1).** The editor event-loop shape; the fn carries
`#[verifier::exec_allows_no_decreases_clause]` and the loop has NO `decreases`:

```
$ verus p5_diverge_break.rs        // while/loop { … if k==quit { break; } }
verification results:: 2 verified, 0 errors        (invariants proved; partial correctness)
$ verus p6_diverge_cont_break.rs   // … if k==0 { continue; } if k==quit { break; }
verification results:: 2 verified, 0 errors        (invariants proved; partial correctness)
```

forge STRUCTURALLY caps this fn at L1 (`degrade-ladder.md` REQ-9, the #88 diverge
cap) — break/continue exit/skip cleanly, no termination claim. The editor's
`while true { let k = read(); if k == quit { break; } … }` now works WITHOUT the
`quit` flag + `dec 1` hack.

**(6) NEGATIVE CONTROL — non-diverge loop WITHOUT a `decreases` → L0.** The
exemption is diverge-ONLY; break/continue are NOT a termination escape hatch:

```
$ verus p7_nondiverge_no_dec.rs
error: loop must have a decreases clause
   = help: to disable this check, use #[verifier::exec_allows_no_decreases_clause] on the function
verification results:: 1 verified, 1 errors        (L0)
```

These six probes ground AC-7. The continue-back-edge obligations (2)+(3) and the
break-exit `invariant`/`invariant_except_break`/`ensures` distinction (4) are the
crux — they prove break/continue cannot launder the invariant or termination
(R-DEFER-9), and that the diverge cap (5) is honest and diverge-only (6).

Gauntlet (R-DEFER-6): `cargo test -p thermite-lower`,
`cargo clippy -p thermite-lower --all-targets -- -D warnings`,
`cargo fmt --check`. Because this route touches `thermite-lower`, the conformance
expectation (the golden Verus passing `verus`) is part of the gate.

**The `tests/golden/lower/` goldens do NOT exist yet** (GREENFIELD). The two
verified files pinned verbatim above are hand-authored into
`tests/golden/lower/{sum,binary_search}.verus.rs` (R-CHAR-3) before the builder
runs; each was confirmed to pass `verus` during authoring of this doc. The #93
break/continue golden(s) are hand-authored from the six probes above.

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (file frame + signature lowering) | SHIPPED | `lower` in `lower.rs` emits the `use vstd::prelude::*; verus! { .. } fn main() {}` frame; `lower_fn`/`lower_spec_fn` build `-> (result: T)`, `requires`/`ensures`, `decreases`; consumer `thermite_lower::lower`; verified by `lower_conformance::sum_emitted_verifies` (`verus`: 5 verified, 0 errors). |
| REQ-2 (type lowering) | SHIPPED | `lower_type` in `lower.rs`; consumer `lower_fn`/`emit_params`; asserted by `lower_conformance::corpus_node_substrings`. |
| REQ-3 (expression lowering) | SHIPPED | `lower_expr` (exec) + `precedence`/`lower_binary_operand` (grouping); consumer `lower_block_with_fn_aids`; verified by both corpus programs. |
| REQ-4 (statement + loop lowering) | SHIPPED | `lower_stmt`/`lower_loop` emit every `inv`→`invariant` + `dec`→`decreases`; `while`/`loop` preserved; consumer `lower_fn_body`. |
| REQ-5 (spec-context `Seq` lowering) | SHIPPED | `lower_expr` w/ `Ctx::Spec` + `lower_spec_arg`/`lower_index` (`xs@`/`subrange`/`@[i as int]`); `spec_sum` Seq recursion via `seq_fold_body`; verified by `sum_emitted_verifies`. |
| REQ-6 (combinator Verus(L3) defs + triggers) | SHIPPED | `CombinatorSig.verus_l3` in `thermite-spec/src/combinators.rs` (all 8 frozen forms); consumer `emit_combinator_defs` in `lower.rs` (closes OQ-2, R-DEFER-1); verified by `combinator_forms_compile_under_verus` (`verus`: 2 verified, 0 errors incl. non-vacuity). |
| REQ-7 (proof-aid emission) | SHIPPED | shape-keyed templates in `lower.rs`: `push_lemma_for` (a), `lift_immutable_preconds` (b), `accumulator_aid`/`match_acc_invariant` (c), `extensionality_at_exit` (d), `complementary_coverage_split` (e); NO per-program hardcoding; both corpus programs verify. |
| REQ-8 (golden-file contract — VERIFY) | SHIPPED | `lower_conformance.rs` runs the real `verus` binary on emitted output (`sum`: 5 verified; `binary_search`: 2 verified; 0 errors each) and asserts the emitted contracts equal the corpus contracts (no weakening). Goldens used as the verified reference, not byte-matched (amended REQ-8). |
| REQ-9 (`LowerError`, no panics) | SHIPPED | `enum LowerError` (span-bearing via `thermite_syntax::lexer::Span`, `Display`) born in `lower.rs`; `lower` returns `Result`; no `unwrap`/`expect`/`panic!` in `src/`; `unknown_combinator_is_err_not_panic` exercises the API surface. |
| REQ-EQ (equivalent-mutant equivalence-obligation seam, #101) | SHIPPED | `pub fn lower_equivalence_obligation(f, mutant_body) -> Result<String, LowerError>` in `lower.rs` (exported in `lib.rs`) renders `f`'s real body + a survivor mutant's body into a self-contained Verus EQUIVALENCE OBLIGATION (`spec fn equiv_real_<n>` / `spec fn equiv_mut_<n>` + a `proof fn equiv_check_<n> requires <req> ensures mut == real {}`), the seam `.design/forge/equivalent-mutants.md` REQ-1 lowers through. It REUSES the L3 exec lowering — each body's expressions go through the SAME `lower_expr`, and the design-GROUNDED `(expr) as <ret>` bounded-arithmetic coercion is applied (the EXEC coercion a naive spec render omits: `x + 0` over a `u64` return fails `verus` with `expected u64, found int` without it). NOT a hand-emitted Verus duplicate (R-CHAR-3): the obligation is built from `lower_expr` output, not from `Expr` by hand. SCALAR-only (`scalar_obligation_type`, OQ-1 of equivalent-mutants.md): a non-scalar param/return or a non-forced-output body shape returns `LowerError::Unsupported` so the survivor STAYS counted (sound-but-incomplete). Consumer: `check::equivalence_proves_equal` (`forge/src/check.rs`). Verified: `thermite-lower/tests/equivalence_obligation.rs` (real verus — equivalent body VERIFIES `2 verified, 0 errors`; distinguishing `x + 1` / `loose` early-return FAIL `0 verified, 1 errors`; non-scalar → `Unsupported`, no panic). |
| REQ-12 (`break`/`continue` lowering + verification semantics, #93) | SHIPPED | `lower_stmt` in `lower.rs` emits `Stmt::Break`→`break;` / `Stmt::Continue`→`continue;` (the Verus-native loop-control statements); mirrored by `lower_stmt_l1` in `l1.rs` (the L1 form; `l2.rs` routes through it via `lower_block_exec`/`lower_loop_exec`). The lowering emits the loop annotations UNCHANGED (no `assume`/`external`/dropped `decreases` — R-DEFER-9) and Verus enforces the GROUNDED obligations. Consumer: `lower` (via `lower_block_with_fn_aids`/`lower_loop`). The `Stmt` ripple closed across `address.rs` (leaf), `effects.rs` (no effect), `validator.rs` (no cage), `mutation.rs` (no mutant — OQ-4), `vacuity.rs`/`closure.rs`/`review.rs`/`check.rs` (leaf walks), `thermite-skill/src/generate.rs` (the loop-control prose) — NO `_`/panic fallthrough. Verified (real `verus`, `forge/tests/break_continue_conformance.rs`, 8/8): continue preserving invariant+decreases → L3 (`continue_preserving_invariant_and_decreases_certifies_l3`); invariant-breaking continue → L0 (`continue_breaking_invariant_is_l0`); non-decreasing continue → L0 (`continue_not_decreasing_measure_is_l0`); break early-exit (post-loop fact from the plain `invariant` held at break — OQ-5 policy (ii)) → L3 (`break_early_exit_certifies_l3`); `fx diverge` loop with break AND continue (no `decreases`) → invariants verify, capped L1 by #88 (`diverge_loop_with_break_and_continue_caps_at_l1`); the in-loop structural rule (`break;`/`continue;` outside a loop → `SyntaxError`) is enforced in `parser.rs` (`break_or_continue_outside_a_loop_is_a_structured_error_not_a_panic`). NO regression — `sum`/`binary_search` STILL L3 (`corpus_loops_without_break_or_continue_still_certify_l3`). |

## Open questions (for the orchestrator before the builder runs)

- **OQ-1 (`nat` vs `u64` for `spec_sum`):** The corpus `ens result == spec_sum(xs)`
  with `acc: u64` is verified by typing `spec_sum: Seq<u32> -> nat` and relating
  `result as nat == spec_sum(xs@)`. A `u64`-valued `spec_sum` would re-introduce
  the overflow obligation INTO the spec function, which the `req xs.len() <=
  1_000_000` bound is there to discharge in `sum`, not in `spec_sum`. The `nat`
  form is the verified choice; recorded because it is a place where the lowering
  makes a typing decision the corpus surface does not spell out. Not a blocker.

- **OQ-2 (proof aids in the golden vs. emitted by the lowerer):** The golden files
  contain the proof aids (`lemma_sum_push`, the `nonlinear_arith` assert, the
  case-split). The open question is whether the lowerer EMITS these from fixed
  templates keyed on the contract shape (a `spec fn` summed over a slice ⇒ emit
  the push lemma) or whether a small library of such lemmas ships in a Verus
  prelude the lowerer `use`s. This doc pins the verified OUTPUT; the emission
  mechanism is the builder's design call within REQ-7. Recorded; not a blocker —
  but it is the highest-judgment part of #4.

- **OQ-3 (`permutation_of` Verus form, least-confident):** `permutation_of` is
  §4.2-named, not corpus-exercised. Its candidate Verus body uses
  `Seq::to_multiset` equality; this was NOT verified end-to-end against a corpus
  program (no corpus program uses it) and `to_multiset` trigger behavior on large
  sequences is exactly the SMT-discontinuity risk §12 warns about. Flagged as the
  least-confident combinator form; its frozen trigger is provisional until a
  corpus program exercises it. Not a blocker for the corpus (#4's AC-1/AC-2 do not
  touch it), but a real risk for the registry's completeness claim.

- **OQ-4 (break/continue mutation, #93):** A `break`/`continue` is NOT a mutation
  target in v0.1 (`forge/src/mutation.rs` gains leaf `Stmt::Break`/`Continue` arms
  that produce no mutant — `ast.md` REQ-12 Architecture). The open question is
  whether a future mutation operator should DELETE a `break` (a loop that no
  longer exits early) or swap `break`↔`continue` to strengthen the mutation-kill
  score over loop-control logic. Recorded for the builder/critic; deleting a
  `break` in a terminating loop is usually caught by the `ens`/decreases anyway.
  Not a blocker.

- **OQ-5 (`invariant_except_break` lowering, #93 — least-confident):** REQ-12 (c)
  pins that the v0.1 corpus break-loops hold all their Thermite `inv`s at the
  break point, so a Thermite `inv` lowers to a plain Verus `invariant` (which
  Verus checks at break too) and the post-break fact threads onto the loop
  `ensures`. A Thermite program whose `inv` is true at re-entry but FALSE at break
  would need the `invariant_except_break` lowering — and Thermite has no surface
  syntax to distinguish "re-entry-only" from "always" invariants today. This is
  the LEAST-CONFIDENT part of #93: the GROUNDED break probe (4) needed
  `invariant_except_break` + a loop `ensures`, neither of which the Thermite
  surface currently spells. The builder must decide whether (i) the lowering
  INFERS which `inv`s are re-entry-only (hard — needs a break-reachability +
  fact-survival analysis), or (ii) v0.1 requires every `inv` to hold at break
  (the corpus does; a program that needs otherwise is rejected with a crisp
  diagnostic until a surface `inv_except_break` keyword is designed — a future
  amendment). Recommended: (ii) for v0.1 (simplest, matches the corpus, no
  surface change). Flagged for the orchestrator — this is the post-break
  assertion-reasoning judgement call and the one most likely to need a follow-up
  design decision. Not a blocker for the parse/AST/lower-emission work, but the
  builder MUST pin a single break-invariant policy.
