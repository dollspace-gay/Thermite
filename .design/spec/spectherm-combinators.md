# SpecTherm Combinator Registry + Validator
<!--
tier: 3-component
status: draft
governs: thermite-spec/src/combinators.rs
thesis-refs:
  - thermite-design.md §4.1
  - thermite-design.md §4.2
  - thermite-design.md §6
  - thermite-design.md §10
  - thermite-design.md §11
-->

## Summary

`thermite-spec` ships the **SpecTherm combinator registry** — the frozen, closed
set of bounded combinators (§4.2) with their name / arity / argument-kinds /
result type — and the **SpecTherm validator**, the boundary API that walks a
parsed `thermite-syntax` AST's contract positions (`req`/`ens`/`inv`/`dec` and
`spec fn` bodies) and enforces §4.2's "locked cage": a contract may use ONLY
registered combinators (correct name + arity + arg-kinds), declared `spec fn`
calls, and the built-in operators / literals / paths the grammar already allows
— and nothing else. The validator is the registry's production consumer (so the
registry is not vocabulary-only, R-DEFER-1) and is the boundary API `thermite-lower`
(#4) and `forge` (#6) call before lowering or running the vacuity battery.

This doc is GREENFIELD / FORWARD-LOOKING: no `combinators.rs` exists (only the
empty crate root `thermite-spec/src/lib.rs`). Every REQ is **NOT-STARTED**,
blocked on issue **#2**. The acto-builder satisfies these REQs next.

## Scope boundary (what ships in #2 vs. what #4 adds)

`thermite-spec` v0.1 (#2) ships the registry's **structural** facet — the part a
consumer needs NOW to *validate*: name, arity, the KIND of each argument, and the
result type. The **lowering** facet of each combinator — the frozen SMT
**trigger** string, the **Verus (L3)** definition, and the **executable (L1)**
runtime-check form (§4.2 "frozen SMT triggers"; §6 "the L1 fallback rung always
exists") — is **DEFERRED to issue #4 (lowering)**, where `thermite-lower` is the
consumer that reads them. Including those fields now would be vocabulary-only (no
consumer in #2, R-DEFER-1). They are coming; this doc names the seam (REQ-2,
OQ-2) and attributes the fields to #4. This is a SCOPE split, not a deferred REQ:
the #4 fields are not REQs of #2 at all.

## Requirements

- **REQ-1 (frozen combinator set):** The registry enumerates the v0.1 SpecTherm
  combinators as a **closed, frozen** set (§4.2 "a fixed library of bounded
  combinators"; §11 "becomes a SpecTherm combinator through the (slow,
  budget-gated) RFC process — never a user-level abstraction mechanism").
  Adding, removing, or changing a combinator is an RFC / design-doc amendment
  (R-SPEC-4), not a code-local choice. The v0.1 set, each entry's source, and
  each entry's signature are pinned in the Architecture section below. Derived
  from §4.2 + the conformance corpus (`sorted`, `forall_in`, `forall_below`,
  `forall_from`).

- **REQ-2 (registry data shape — structural facet only):** Each registry entry
  carries: a canonical **name** (`&str`), an **arity** (fixed argument count), an
  ordered list of **argument kinds** (`ArgKind` — one of: `Slice` a `&[T]`
  expression; `Index` a `usize`-valued expression; `Pred` a predicate closure
  `|x| <bool expr>`; `Value` a plain scalar expression), and a **result kind**
  (the v0.1 combinators all yield `bool`; the field exists so a future
  `count_where`-style `usize` result is representable). The lowering facet
  (trigger / Verus def / L1 form) is OUT OF SCOPE here and added by #4 (see Scope
  boundary). The registry is a deterministic, statically-defined table (no
  wall-clock, no env; R-CODE-5). Derived from §4.2 + §10 ("the skill is
  regenerated from the grammar and combinator registry" — the registry is the
  single source of truth #7 reads).

- **REQ-3 (validator contract — the accept rule):** A boundary function
  (`validate`, taking the parsed `Program` plus the set of declared `spec fn`
  names) walks every **contract position** — each `Contract.req` clause, each
  `Contract.ens` clause, every `LoopNode.invs` clause and `LoopNode.dec` clause,
  and every `SpecFnItem.body` expression tree — and accepts an expression iff
  every sub-expression is one of: **(a)** a `Expr::Call` whose callee resolves to
  a registered combinator name with the **right arity and arg-kinds** (each
  positional arg matches the entry's `ArgKind` — e.g. a `Pred` position holds an
  `Expr::Closure`); **(b)** an `Expr::Call` / `Expr::MethodCall` whose callee is a
  declared `spec fn` name (e.g. `spec_sum`); **(c)** a built-in the grammar
  already sanctions — `IntLit`/`BoolLit`, `Path` (`u32::MAX`, `lo`, `Some`,
  `None`), `Binary`, `Index`, `Cast`, `Ref`, `Field`, `Match`, `If`, and the
  bounded built-in `MethodCall`s the grammar admits (e.g. `xs.len()`). Anything
  else is rejected (REQ-4). Derived from §4.1 (where contracts appear), §4.2
  ("No general quantifiers … only … a fixed library of bounded combinators"),
  and `goal.md` ("the parser is REGISTRY-FREE … the fixed-combinator-set rule is
  a SEMANTIC check enforced here").

- **REQ-4 (validator contract — the reject cases, structured `SpecError`):** The
  validator returns `Result<(), Vec<SpecError>>` (or equivalent
  multi-diagnostic), NEVER panicking (R-CODE-2 / R-APG-1). It rejects, with a
  span-bearing structured `SpecError` variant for each: **(i)** an unknown
  combinator / free-function call in a contract position
  (`UnknownCombinator { name, span }`); **(ii)** a registered combinator with the
  wrong arity (`WrongArity { name, expected, found, span }`); **(iii)** a
  registered combinator with a wrong argument kind — e.g. a non-closure where a
  `Pred` is required, or a non-slice where a `Slice` is required
  (`WrongArgKind { name, position, expected, found, span }`); **(iv)** a
  construct the contract sublanguage forbids that nonetheless parsed (e.g. an
  arbitrary call expression in a contract whose callee is neither a combinator
  nor a declared spec fn). `SpecError` is `thermite-spec`'s OWN error enum, born
  with this first fallible function (per workspace.md REQ-3: "each crate
  introduces its OWN error enum … when its first fallible function lands, which
  is this issue"). Derived from §2.4 (crisp structured feedback), R-CODE-2,
  workspace.md REQ-3.

- **REQ-5 (bounded recursion — no overflow):** The validator's expression walk is
  recursive (the AST is a tree: `Binary`/`Index`/`Match`/`If`/`Call` args nest
  arbitrarily), so it MUST bound its descent depth from the first commit and
  return a structured `SpecError::ExpressionTooDeep { limit, span }` rather than
  overflowing the native stack — mirroring the `thermite-syntax` parser's
  `guard_recursion` / `MAX_RECURSION_DEPTH` precedent (a fixed constant, for
  determinism, R-CODE-5; this guard is the lesson the parser re-audit
  (#29/#31/#32) hard-coded). A pathological deeply-nested contract expression is
  a structured error, never a process abort. Derived from R-CODE-2 +
  `thermite-syntax/src/parser.rs` (`guard_recursion`, `MAX_RECURSION_DEPTH`),
  §2.4 ("a timeout is never the final answer … the gate degrades, it does not
  block").

## Acceptance criteria

- **AC-1 (registry contents match the frozen oracle):** The registry's entries —
  name, arity, ordered arg-kinds, result kind for every combinator in REQ-1's
  set — equal the hand-authored oracle at `tests/golden/combinators/registry.json`
  (or `.txt`), field-for-field. Expected values are hand-derived from §4.2 + the
  corpus, never read back from the registry's own output (R-CHAR-3). Mechanically:
  `cargo test -p thermite-spec` asserts the registry against the golden file.
  (REQ-1, REQ-2)

- **AC-2 (corpus contracts validate clean):** Validating the parsed
  `conformance/sum.th` and `conformance/binary_search.th` returns `Ok(())` — every
  combinator and spec-fn call in their contract positions is accepted:
  `sorted(haystack)`, `forall_in(haystack, |x| x != needle)`,
  `forall_below(haystack, lo, |x| x < needle)`,
  `forall_from(haystack, hi, |x| x > needle)`, and the spec-fn calls
  `spec_sum(xs)` / `spec_sum(&xs[..i])`. Tied to the accept fixtures under
  `tests/golden/combinators/accept/`. (REQ-3)

- **AC-3 (crafted negatives reject with the right variant):** Hand-crafted
  negative fixtures under `tests/golden/combinators/reject/` each produce the
  expected `SpecError` variant: an unknown combinator (`frobnicate(haystack)`) →
  `UnknownCombinator`; `forall_in(haystack)` (1 arg) → `WrongArity`;
  `forall_in(haystack, needle)` (non-closure in the `Pred` slot) →
  `WrongArgKind`; an arbitrary free call in `ens` whose callee is neither a
  combinator nor a declared spec fn → the forbidden-call rejection. Each
  fixture's expected variant + offending name/position is hand-derived
  (R-CHAR-3). (REQ-3, REQ-4)

- **AC-4 (no panic, bounded recursion):** Validating a pathological deeply-nested
  contract expression (nesting past the recursion bound) returns
  `Err([SpecError::ExpressionTooDeep { .. }])` — it does NOT overflow or panic.
  Validating any structurally well-formed-but-semantically-rejected input returns
  `Err`, never panics. Mechanically: a `validate_never_panics` test over crafted
  deep / malformed inputs. (REQ-4, REQ-5)

- **AC-5 (validator is the registry's consumer — R-DEFER-1):** The registry's
  public lookup API has a non-test production consumer in the same crate: the
  validator (`validate`) calls the registry lookup to resolve each contract-call
  callee. Mechanically: the registry's lookup symbol is referenced from
  `validate`, not only from tests. (REQ-2, REQ-3)

## Architecture

The component is a statically-defined registry table plus a recursive AST walk,
in `thermite-spec/src/combinators.rs`. It depends on `thermite-syntax` (the AST
boundary type) and introduces `thermite-spec`'s own `SpecError` enum.

### The frozen v0.1 combinator set (REQ-1)

Each row: **name** — **arity** — **arg-kinds (ordered)** → **result** — **source
justification**. Arg-kind vocabulary: `Slice` (`&[T]` expr), `Index` (`usize`
expr), `Pred` (predicate closure `|x| bool`), `Value` (scalar expr).

| Combinator | Arity | Arg-kinds | Result | Source |
|---|---|---|---|---|
| `forall_in` | 2 | `Slice, Pred` | `bool` | §4.2 named list; corpus `binary_search` `ens` (`forall_in(haystack, |x| x != needle)`). |
| `exists_in` | 2 | `Slice, Pred` | `bool` | §4.2 named list ("`exists_in`"). The dual of `forall_in`; same shape. |
| `forall_below` | 3 | `Slice, Index, Pred` | `bool` | corpus `binary_search` `inv` (`forall_below(haystack, lo, |x| x < needle)`). Bounded `forall` over the prefix `[..lo]`. |
| `forall_from` | 3 | `Slice, Index, Pred` | `bool` | **corpus-required, NOT in §4.2's named list** — see note below; corpus `binary_search` `inv` (`forall_from(haystack, hi, |x| x > needle)`). Bounded `forall` over the suffix `[hi..]`. |
| `count_where` | 2 | `Slice, Pred` | `usize` | §4.2 named list ("`count_where`"). The one v0.1 combinator whose result is `usize`, not `bool` (motivates the `result kind` field, REQ-2). |
| `sorted` | 1 | `Slice` | `bool` | §4.2 named list; corpus `binary_search` `req` (`sorted(haystack)`). |
| `permutation_of` | 2 | `Slice, Slice` | `bool` | §4.2 named list ("`permutation_of`"). |
| `disjoint` | 2 | `Slice, Slice` | `bool` | §4.2 named list ("`disjoint`"). |

**`forall_from` justification (the one corpus-vs-§4.2 gap).** §4.2's combinator
list is explicitly open-ended ("`forall_in`, `forall_below`, `exists_in`,
`count_where`, `sorted`, `permutation_of`, `disjoint`, **…**"). The corpus
`binary_search.th` — a hand-certified external truth (`goal.md`) — uses
`forall_from(haystack, hi, |x| x > needle)` as a loop invariant. It is the exact
suffix-dual of the §4.2-named `forall_below` (prefix), with the same
`Slice, Index, Pred` shape; the binary-search invariant pair (`forall_below` over
`[..lo]`, `forall_from` over `[hi..]`) is the canonical use the design's own
Appendix-A-adjacent §4.1 example demands. It is therefore admitted as
**corpus-required** under §4.2's `…`, recorded explicitly here rather than
silently invented. No other combinator is added beyond §4.2's named list + this
one corpus entry (anti-goal §11: no expressiveness for its own sake).

**`count_where` inclusion.** Named in §4.2 but NOT exercised by the corpus. It is
admitted on the strength of §4.2's explicit naming (the registry is "the single
source of truth" #7's skill regenerates from, §10, so the named set must be
present), and it is the motivating case for the `result kind` field (REQ-2). See
OQ-1: whether to ship the full §4.2-named set or only the corpus-exercised subset
in v0.1 is the one genuine scope question for the orchestrator.

### The registry data shape (REQ-2)

A static table of entries `{ name, arity, arg_kinds: &[ArgKind], result: ResultKind }`,
exposed through a lookup (`lookup(name) -> Option<&CombinatorSig>`). The table is
`const`/`static` (deterministic, R-CODE-5). The lowering facet (frozen SMT
trigger, Verus def, L1 form) is intentionally absent — it is added to each entry
by #4 where `thermite-lower` consumes it (Scope boundary; OQ-2). Keeping the #4
fields out now is what keeps the registry from being vocabulary-only in #2.

### The validator (REQ-3/REQ-4/REQ-5)

`validate(program: &Program) -> Result<(), Vec<SpecError>>` first collects the
declared `spec fn` names (every `Item::SpecFn(s)` → `s.name`), then walks each
contract position. The contract positions are exactly the AST clauses
`thermite-syntax` already models (cite `Contract.req`, `Contract.ens`,
`LoopNode.invs`, `LoopNode.dec` in `ast.rs`; `SpecFnItem.body`). The walk
descends `Expr` recursively under `guard_recursion`-style depth bounding (REQ-5),
applying the accept rule (REQ-3) at each node and emitting a `SpecError` (REQ-4)
on each violation; it accumulates diagnostics rather than failing on the first
(crisp feedback, §2.4).

The frontend stays REGISTRY-FREE by design (`ast.md`: "combinator calls
(`forall_in`, `sorted`) are ordinary `Expr::Call` nodes"; `surface-grammar.md`
Scope boundary): the `Expr::Call` node carries no "is-a-combinator" mark. The
registry distinction — accept iff a registered combinator with matching
arity+arg-kinds, OR a declared spec-fn call, OR a built-in — happens HERE, in the
validator, exactly as `goal.md`'s authority chain places it. A combinator
appears in the AST as `Expr::Call { callee: Expr::Path([name]), args }`; the
validator resolves `name` against the registry, checks `args.len() == arity`, and
checks each `args[i]` against `arg_kinds[i]` (a `Pred` slot must be
`Expr::Closure`; a `Slice` slot an expression of slice shape; etc.). A `spec fn`
call resolves against the collected spec-fn name set. Built-ins
(`Binary`/`Index`/`Cast`/`Ref`/`Field`/`Match`/`If`/literals/paths and the
grammar's bounded `MethodCall`s like `xs.len()`) are accepted structurally and
their sub-expressions recursed into.

`SpecError` is `thermite-spec`'s own error enum (workspace.md REQ-3), span-bearing
(reusing `thermite_syntax::lexer::Span`), `Display`-able, with the variants of
REQ-4 plus `ExpressionTooDeep`. No `unwrap`/`expect`/`panic!` in production
(R-CODE-2 / R-APG-1).

### Boundary role (the consumer chain)

`validate` is the boundary API `thermite-lower` (#4) calls before lowering a
contract (a contract that fails validation must not reach the lowerer) and that
`forge` (#6) calls before the vacuity battery. Within #2 the validator is itself
the registry's first production consumer (AC-5), discharging R-DEFER-1 without
waiting for #4. The registry is also the artifact `thermite-skill` (#7)
regenerates the SpecTherm section of `THERMITE.skill.md` from (§10) — a second,
later consumer.

## Verification

`cargo test -p thermite-spec` over the oracle at `tests/golden/combinators/`
(declared as this route's `reference` in `tooling/spec-routes.toml`):

- **AC-1:** assert the registry table equals the hand-authored
  `tests/golden/combinators/registry.{json,txt}` (every name/arity/arg-kinds/
  result), expected values hand-derived from §4.2 + corpus (R-CHAR-3).
- **AC-2:** parse `conformance/sum.th` and `conformance/binary_search.th` (via
  `thermite-syntax`) and assert `validate` returns `Ok(())` — accept fixtures
  under `tests/golden/combinators/accept/`.
- **AC-3:** for each crafted negative under `tests/golden/combinators/reject/`,
  assert the returned `SpecError` variant + offending name/position matches the
  fixture's hand-derived expectation.
- **AC-4:** `validate_never_panics` over deeply-nested + malformed contract
  expressions asserts `Err(.. ExpressionTooDeep ..)` / `Err(..)`, never a panic
  or overflow.
- **AC-5:** confirm `validate` references the registry lookup symbol (consumer
  check; the critic greps for the non-test call site).

Gauntlet (R-DEFER-6): `cargo test -p thermite-spec`,
`cargo clippy -p thermite-spec --all-targets -- -D warnings`,
`cargo fmt --check`.

**The `tests/golden/combinators/` oracle does NOT exist yet** — that is expected
(GREENFIELD). The orchestrator hand-authors it (R-CHAR-3) from §4.2 + the corpus
before the builder runs. It should contain: (1) `registry.{json,txt}` — the
expected registry contents (the table above, serialized); (2) `accept/` — the
contract-position expressions from `sum.th` / `binary_search.th` that must
validate clean; (3) `reject/` — crafted negatives, each paired with its expected
`SpecError` variant (unknown combinator, wrong arity, wrong arg-kind, arbitrary
contract call). See AC-1..AC-3 for the exact expected contents.

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (frozen combinator set) | NOT-STARTED | blocked on #2; no `combinators.rs` (only the empty `thermite-spec/src/lib.rs` scaffold root). The frozen set is pinned in this doc's Architecture table awaiting impl. |
| REQ-2 (registry data shape — structural facet) | NOT-STARTED | blocked on #2; no registry table / `CombinatorSig` / `ArgKind` type exists yet. Lowering facet (trigger/Verus/L1) is #4 scope, not a #2 REQ. |
| REQ-3 (validator accept rule) | NOT-STARTED | blocked on #2; no `validate` fn. AST contract positions (`Contract.req`/`ens`, `LoopNode.invs`/`dec`, `SpecFnItem.body`) exist in `thermite-syntax/src/ast.rs` (prereq SHIPPED), but the walk does not. |
| REQ-4 (reject cases, structured `SpecError`) | NOT-STARTED | blocked on #2; `thermite-spec` has no error enum yet (workspace.md REQ-3 defers `SpecError` to this issue). Parser precedent `SyntaxError` in `thermite-syntax/src/parser.rs` is the shape to mirror. |
| REQ-5 (bounded recursion — no overflow) | NOT-STARTED | blocked on #2; no validator walk to bound. Precedent `guard_recursion` / `MAX_RECURSION_DEPTH` exists in `thermite-syntax/src/parser.rs` and is the pattern to replicate. |

## Open questions (for the orchestrator before the builder runs)

- **OQ-1 (full §4.2-named set vs. corpus-exercised subset):** This doc ships the
  full §4.2-named set (`forall_in`, `exists_in`, `count_where`, `sorted`,
  `permutation_of`, `disjoint`) plus the two prefix/suffix bounded forms
  (`forall_below` named, `forall_from` corpus-required) — eight combinators.
  Of these, only four are exercised by the corpus (`sorted`, `forall_in`,
  `forall_below`, `forall_from`). Rationale for shipping the full named set: the
  registry is the single source of truth `thermite-skill` (#7) regenerates the
  skill's combinator library from (§10), so a combinator §4.2 names but the
  corpus omits still belongs in the registry. The unexercised four
  (`exists_in`, `count_where`, `permutation_of`, `disjoint`) will have AC-1
  registry-shape coverage but no AC-2 accept-fixture coverage until a corpus
  program uses them. Flagged for confirmation; not a blocker. If the orchestrator
  prefers a corpus-only subset for v0.1, REQ-1's table shrinks to four rows.

- **OQ-2 (the #4 lowering-facet seam):** REQ-2 ships only name/arity/arg-kinds/
  result; the frozen SMT trigger, Verus (L3) def, and executable (L1) form are
  added per-entry by #4. The open question is the *shape* of that extension —
  whether #4 adds fields to the existing `CombinatorSig` struct or a parallel
  `LoweringSig` table keyed by name. This doc does not decide it (it is #4's call,
  governed by `.design/lower/verus-lowering.md`); recorded so the #2 builder
  leaves the struct extensible (e.g. avoids `#[non_exhaustive]`-hostile layout)
  and the critic does not flag the absent fields as a #2 miss. Not a blocker.

- **OQ-3 (`Slice`/`Value` arg-kind checking depth):** REQ-3's arg-kind check is
  strongest for `Pred` (must be `Expr::Closure` — syntactically decidable) and
  weakest for `Slice` vs `Value` (the AST is untyped; `haystack` is a `Path`, and
  whether it denotes a `&[T]` requires the param types). v0.1 decision: the
  validator checks the *syntactically* decidable kinds (`Pred` ⇒ closure; arity)
  precisely, and treats `Slice`/`Index`/`Value` as "an expression in that
  position" with shallow shape checks (e.g. a `Pred` slot rejects a non-closure,
  but a `Slice` slot accepts any non-closure expression), leaving full type
  checking to a later type-resolution pass (not a v0.1 kernel item). This keeps
  #2 honest about what it can mechanically enforce without a type checker.
  Recorded; resolvable from §4.2's intent (the cage is about *which combinators*,
  not full typing); not a blocker.
