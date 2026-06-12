# SpecTherm Combinator Registry + Validator
<!--
tier: 3-component
status: draft
audited-sha: dff9ae866e3437af272a62e078993e66c1116460 (re-audited 2026-06-12: amended — REQ-6 flat-closure cage SHIPPED post-pin (#40, 4d46f8a4), greenfield Summary fixed, validator-growth currency note, #262)
governs: thermite-spec/src/combinators.rs, thermite-spec/src/validator.rs
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

This doc's REQs are SHIPPED: the registry (`combinators.rs`) + the validator
(`validator.rs`) under issue #2, and REQ-6's flat-closure cage under issue #40
(commit `4d46f8a4`, post-pin) — see the REQ status table.

> **AMENDMENT (#262 re-audit, 2026-06-12).** (a) **REQ-6 SHIPPED** post-pin
> (#40, `4d46f8a4`): the REQ-status row below is updated; the Architecture
> section's "collapses these two contexts" diagnosis describes the PRE-#40 code
> and is kept as historical rationale. OQ-4 is RESOLVED with the dedicated
> `SpecError::NestedCombinator { name, span }` variant. (b) `validator.rs` has
> since grown FAR beyond this doc's cage — ADT well-formedness + `match`
> exhaustiveness (`.design/basis/01-adts.md`), the recursion-SCHEME flat-step
> cage (`SpecError::NestedScheme`, `.design/basis/02-recursion-schemes.md`),
> the `BUILTIN_METHODS` cage admissions for Vec/String/Option/Result/Map
> (04-collections / 07-strings / 09-option-result / 13-map), the `#[sealed]`
> barrier (`SpecError::SealedConstruction`, 06-provenance-and-sinks), and the
> C10 guarded-arm exhaustiveness rule (11-ergonomics) — each owned by its
> `.design/basis/*.md` and contracted in `validator.rs`'s module-doc REQ
> tables, NOT re-owned here. This doc continues to govern the combinator
> registry + the §4.2 combinator cage (REQ-1..REQ-6).

> **Amendment (2026-06-04, issue #40):** REQ-6 (the flat-closure-fragment rule)
> and AC-6..AC-8 are ADDED to close a real hole in the §4.2 cage that an external
> reviewer caught: a combinator's predicate-closure body was UNrestricted and
> could contain ANOTHER combinator, reintroducing the nested-quantifier
> instantiation unpredictability §4.2 claims to cage. The existing REQ-1..REQ-5
> (SHIPPED under #2) are UNCHANGED; REQ-6 has since SHIPPED at #40 (`4d46f8a4`
> — see the #262 amendment above and the REQ status table). See the
> "Thesis-clarification note" section for the §4.2-wording erratum this amendment
> records (handled like the §4.3 inv-numbering case — recorded here, not edited
> into `thermite-design.md`).

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

- **REQ-6 (flat-closure-fragment rule — no anonymous nested quantifiers):** A
  registered combinator's **predicate-closure argument body** (the `Pred` slot's
  `Expr::Closure` body — REQ-2/REQ-4(iii)) is a **FLAT predicate**. The validator
  walks that body under a "**caged-flat**" mode in which the accept set is
  STRICTLY NARROWER than REQ-3's general contract-position accept rule: the body
  MAY contain comparisons, arithmetic, boolean / logical operators, field / index
  access, casts, refs, literals, paths, the bounded built-in `MethodCall`s
  (`xs.len()`), `Match` / `If`, and **calls to NAMED `spec fn`s** (resolved
  against the declared-spec-fn set, exactly as REQ-3(b)); it **MAY NOT** contain a
  call whose callee resolves to a **registered combinator** (REQ-1's set). A
  combinator call inside a predicate-closure body is rejected with a dedicated
  span-bearing `SpecError` cause — **`NestedCombinator`** (the builder picks the
  final identifier; this doc pins the REJECT outcome + the cause being a
  combinator-nested-in-closure rejection, distinct from `UnknownCombinator` and
  `ForbiddenCall`). Genuine nested quantification is expressed by extracting a
  NAMED `spec fn` (which carries its own `dec` measure and is auditable, §4.2
  "No spec-level recursion without a `dec` measure") and calling it — composition
  is **named, never anonymous**.

  **Rationale (from #40):** a combinator lowers to a bounded quantifier with a
  FROZEN trigger on the predicate application
  (`forall_in(s, p) == forall|i| 0 <= i < s.len() ==> #[trigger] p(s[i])`). The
  bounded range kills unbounded-quantifier blowup; the frozen trigger removes
  Verus's heuristic trigger-inference variance (a primary SMT-discontinuity
  source, §13 risk row "small edits flip proofs to timeouts"). For a FLAT closure
  body this genuinely locks instantiation. An ANONYMOUS nested combinator inside
  the body (`forall_in(xs, |x| exists_in(ys, |y| y == x))`) composes two bounded
  quantifiers whose instantiation interaction is exactly the unpredictability
  §4.2 claims to cage — and the loop-3 validator OVER-PERMITS it today (its
  audit accepted that program). The refined, honest invariant: **every quantifier
  in SpecTherm is a bounded combinator with a frozen trigger; composition happens
  only through named `spec fn`s, never anonymous nested quantifiers.** Honest
  caveat: a flat closure MAY still call a named `spec fn` that internally
  quantifies — depth is **named + bounded** (each named layer is `dec`-measured
  and auditable), NOT zero; what REQ-6 forbids is anonymous arbitrary nesting,
  the unpredictable part. Derived from §4.2 + §6 + issue #40's `--kind decision`
  analysis.

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
  `spec_sum(xs)` / `spec_sum(&xs[..i])`. Tied to the accept fixtures in
  `tests/golden/combinators/accept.json`. (REQ-3)

- **AC-3 (crafted negatives reject with the right variant):** Hand-crafted
  negative fixtures in `tests/golden/combinators/reject.json` each produce the
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

- **AC-6 (nested combinator in a closure body is REJECTED):** Validating a program
  whose combinator predicate-closure body contains a combinator call —
  canonically `forall_in(xs, |x| exists_in(ys, |y| y == x))` — returns `Err`
  carrying the dedicated nested-combinator cause (`NestedCombinator`, name at the
  builder's discretion; the REJECT outcome + the combinator-nested-in-closure
  cause is what is pinned), NOT `Ok(())`. The reject case is committed as
  `nested_combinator_in_closure` in `tests/golden/combinators/reject.json`
  (landed with #40). It is the exact program the loop-3 audit OVER-PERMITTED, so
  the test pins the fix. (REQ-6)

- **AC-7 (named spec-fn call inside a closure body is ACCEPTED):** Validating a
  program whose combinator predicate-closure body calls a NAMED `spec fn` —
  canonically `forall_in(xs, |x| is_even(x))` where `is_even` is a declared
  `spec fn` in the same program — returns `Ok(())`. Named composition is allowed
  (the honest-caveat case of REQ-6: a flat body may call a `dec`-measured named
  spec fn). This is a NEW accept case in `tests/golden/combinators/accept.json`
  (orchestrator adds it). (REQ-6, REQ-3(b))

- **AC-8 (the flat corpus closures remain ACCEPTED):** The existing accept-fixture
  closures whose bodies are flat predicates — `forall_in(xs, |x| x != n)`,
  `forall_below(xs, i, |x| x < 5)`, `forall_from(xs, i, |x| x > 5)`,
  `exists_in(xs, |x| x == n)`, `count_where(xs, |x| x == 0) <= xs.len()` (the
  corpus `binary_search` invariants `|x| x < needle`, `|x| x > needle`, and the
  `sum` corpus closures) — STILL return `Ok(())` after REQ-6 lands. The flat
  corpus is UNAFFECTED by the tightening (confirmed: no corpus closure body
  contains a combinator call). Mechanically: every pre-existing `accept.json`
  case continues to validate clean. (REQ-6, REQ-3)

## Architecture

The component is a statically-defined registry table plus a recursive AST walk,
in `thermite-spec/src/combinators.rs` (registry) and
`thermite-spec/src/validator.rs` (the walk). It depends on `thermite-syntax` (the
AST boundary type) and introduces `thermite-spec`'s own `SpecError` enum.

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
validator resolves `name` against the registry (`combinators::lookup`),
checks `args.len() == arity`, and checks each `args[i]` against `arg_kinds[i]` (a
`Pred` slot must be `Expr::Closure` — see `check_arg_kind`; a `Slice` slot an
expression of slice shape; etc.). A `spec fn` call resolves against the collected
spec-fn name set. Built-ins
(`Binary`/`Index`/`Cast`/`Ref`/`Field`/`Match`/`If`/literals/paths and the
grammar's bounded `MethodCall`s like `xs.len()`) are accepted structurally and
their sub-expressions recursed into.

`SpecError` is `thermite-spec`'s own error enum (workspace.md REQ-3), span-bearing
(reusing `thermite_syntax::Span`), `Display`-able, with the variants of
REQ-4 plus `ExpressionTooDeep`. No `unwrap`/`expect`/`panic!` in production
(R-CODE-2 / R-APG-1).

### The flat-closure-fragment rule (REQ-6) — the "caged-flat" walk

REQ-6 distinguishes TWO walk contexts for a contract expression:

1. **General contract position** (a `req`/`ens`/`inv`/`dec` clause expression, or
   a `spec fn` body): a combinator call IS allowed here — this is where a
   quantifier is introduced. (REQ-3's accept rule, as today: `walk_expr` →
   `walk_call`.)
2. **Caged-flat** (INSIDE a combinator's `Pred`-slot closure body): a combinator
   call is FORBIDDEN here — the body must be a flat predicate. A named-spec-fn
   call IS still allowed (named composition, the honest caveat).

**(HISTORICAL — the pre-#40 diagnosis; FIXED, see the #262 amendment.)** Before
#40, the validator collapsed these two contexts: in `check_arg_kind`, the `Pred`
arm recursed the closure body via the SAME `walk_expr` used for a general
contract position, so a nested `exists_in(...)` in that body reached `walk_call`,
where `combinators::lookup` succeeded and the nested combinator was ACCEPTED —
the over-permit the loop-3 audit observed (`forall_in(xs, |x| exists_in(ys, |y| y == x))`
validated clean), the precise hole #40 reported.

The fix (LANDED at #40) is a flat-mode walk: when
descending a `Pred`-slot closure body, the walk must reject a callee that
resolves to a registered combinator with the dedicated `NestedCombinator` cause,
WHILE still accepting a callee that resolves to a declared `spec fn` (and all the
flat built-ins). The discriminator is **exactly** the `combinators::lookup(name)`
vs. `self.spec_fns.contains(name)` test already in `walk_call` — see the subtlety
note below: the SAME callee-name that `walk_call` accepts as a combinator in a
general position is the one a flat-mode walk must reject; the SAME callee-name
`walk_call` accepts as a spec-fn call stays accepted in both contexts.

The depth guard (REQ-5) continues to wrap the flat-mode descent unchanged — REQ-6
narrows the ACCEPT SET inside a closure body, it does not change the recursion
bounding.

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
  `thermite-syntax`) and assert `validate` returns `Ok(())` — accept fixtures in
  `tests/golden/combinators/accept.json`.
- **AC-3:** for each crafted negative in `tests/golden/combinators/reject.json`,
  assert the returned `SpecError` variant + offending name/position matches the
  fixture's hand-derived expectation.
- **AC-4:** `validate_never_panics` over deeply-nested + malformed contract
  expressions asserts `Err(.. ExpressionTooDeep ..)` / `Err(..)`, never a panic
  or overflow.
- **AC-5:** confirm `validate` references the registry lookup symbol (consumer
  check; the critic greps for the non-test call site).
- **AC-6:** the `reject.json` case `nested_combinator_in_closure` —
  `forall_in(xs, |x| exists_in(ys, |y| y == x))` — asserts `Err` with the
  nested-combinator cause (the program the audit over-permitted). LANDED with
  #40: the case is in the committed oracle and green
  (`tests/combinators_conformance.rs`).
- **AC-7:** the NEW `accept.json` case `named_spec_fn_in_closure` —
  `spec fn is_even(x: u32) -> bool dec 0 { x % 2 == 0 } fn f(xs: &[u32]) -> u32 req true ens forall_in(xs, |x| is_even(x)) fx pure { 0 }`
  — asserts `Ok(())` (named composition allowed).
- **AC-8:** every pre-existing `accept.json` case (the flat corpus closures)
  continues to assert `Ok(())` after REQ-6 — the corpus is unaffected.

Gauntlet (R-DEFER-6): `cargo test -p thermite-spec`,
`cargo clippy -p thermite-spec --all-targets -- -D warnings`,
`cargo fmt --check`.

**Oracle anchors for REQ-6 (the orchestrator adds these; this doc pins the
outcome):**

- ADD to `tests/golden/combinators/reject.json`:
  ```json
  { "name": "nested_combinator", "expected": "NestedCombinator",
    "program": "fn f(xs: &[u32], ys: &[u32]) -> u32 req true ens forall_in(xs, |x| exists_in(ys, |y| y == x)) fx pure { 0 }",
    "why": "a combinator's predicate-closure body must be a FLAT predicate (REQ-6); a nested combinator call (exists_in) inside the |x| body composes an anonymous nested quantifier, reintroducing the §4.2 instantiation unpredictability the cage forbids. Named composition via a `spec fn` is the sanctioned alternative." }
  ```
- ADD to `tests/golden/combinators/accept.json`:
  ```json
  { "name": "named_spec_fn_in_closure", "combinator": "forall_in (named-spec-fn body)",
    "program": "spec fn is_even(x: u32) -> bool dec 0 { x % 2 == 0 } fn f(xs: &[u32]) -> u32 req true ens forall_in(xs, |x| is_even(x)) fx pure { 0 }" }
  ```

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (frozen combinator set) | SHIPPED | registry table in `thermite-spec/src/combinators.rs` (const `CombinatorSig` set); consumed by `validate` via `combinators::lookup` in `validator.rs`. Verification: `tests/golden/combinators/registry.{json}` asserted by `cargo test -p thermite-spec`. |
| REQ-2 (registry data shape — structural facet) | SHIPPED | `CombinatorSig { name, arity, arg_kinds, result }` + `enum ArgKind` (`Slice`/`Index`/`Pred`/`Value`) in `combinators.rs`; lookup consumed by `validate`. Lowering facet (trigger/Verus/L1) remains #4 scope, not a #2 REQ. |
| REQ-3 (validator accept rule) | SHIPPED | `pub fn validate` in `validator.rs` collects `spec fn` names, walks `Contract.req`/`ens`, `LoopNode.invs`/`dec`, `SpecFnItem.body`; accepts registered combinators (`combinators::lookup`), declared spec-fn calls, grammar built-ins. Verification: every `accept.json` case validates clean. |
| REQ-4 (reject cases, structured `SpecError`) | SHIPPED | `enum SpecError` (`UnknownCombinator`/`WrongArity`/`WrongArgKind`/`ForbiddenCall`/`ExpressionTooDeep`) in `validator.rs`; `validate` returns `Result<(), Vec<SpecError>>`, never panics. Verification: every `reject.json` case yields the expected cause. |
| REQ-5 (bounded recursion — no overflow) | SHIPPED | `MAX_RECURSION_DEPTH` + `descend` guard wraps every recursive descent in `validator.rs`; deep input yields `ExpressionTooDeep`. Verification: `validate_never_panics`. |
| REQ-6 (flat-closure-fragment rule — no anonymous nested quantifiers) | SHIPPED | #40 (`4d46f8a4`, post-pin; verified at the #262 re-audit). `check_arg_kind`'s `Pred` arm sets `Validator::in_combinator_closure` for the whole closure-body descent (`validator.rs`); while set, `walk_call` rejects any callee resolving via `combinators::lookup` with the dedicated span-bearing `SpecError::NestedCombinator { name, span }`, while a declared spec-fn callee stays accepted (named composition — AC-7). Non-test consumer: `pub fn validate in validator.rs` (the walk forge runs before lowering). Verification: `tests/golden/combinators/reject.json` case `nested_combinator_in_closure` → `NestedCombinator` + `accept.json` case `named_spec_fn_in_closure` → `Ok` (`thermite-spec/tests/combinators_conformance.rs`); the flat corpus closures stay `Ok` (AC-8); edge coverage in `thermite-spec/tests/divergence_nesting.rs` (nested-in-loop-inv / under-three-arg-outer / in-spec-fn-body all reject). |

## Thesis-clarification note (erratum-style — for the orchestrator/user; do NOT edit `thermite-design.md` here)

`thermite-design.md` §4.2's prose **"No general quantifiers. Quantification is
only available through a fixed library of bounded combinators … each with
hand-tuned, frozen SMT triggers. … Thermite locks the cage."** is realized
PRECISELY by REQ-6's flat-closure-fragment rule. The thesis wording is an
**over-compression** in two respects an external reviewer (issue #40) correctly
flagged:

1. **"locks the cage" overstates the loop-3 implementation**, which left a hole:
   a combinator's closure body was unrestricted and could nest ANOTHER
   combinator, composing quantifiers (the unpredictable, instantiation-heavy
   part the cage exists to forbid). The cage is only honestly locked once REQ-6
   forbids anonymous nested combinators inside closure bodies.
2. **"No general quantifiers" reads as "zero quantifier composition",** but the
   honest invariant is narrower and TRUE: *named* `spec fn`s CAN quantify,
   boundedly — a flat closure may call a `dec`-measured named spec fn that
   internally quantifies. Depth is named + bounded + auditable, not zero. The
   precise, honest restatement of §4.2 is: **"every quantifier in SpecTherm is a
   bounded combinator with a frozen trigger; composition happens only through
   named `spec fn`s (each `dec`-measured and auditable), never anonymous nested
   quantifiers."**

Recommended §4.2 amendment (for the user/orchestrator to apply to
`thermite-design.md`, NOT done by this doc — handled like the §4.3 inv-numbering
erratum): append to the "No general quantifiers" bullet a clarifying sentence:
*"A combinator's predicate-closure body is a flat predicate — it may call named
`spec fn`s but not other combinators; anonymous nested quantification is
forbidden, so all quantifier composition is named and bounded."* This is a
clarification, not a semantics change: the registry and the corpus are unchanged.

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

- **OQ-4 (REQ-6 `NestedCombinator` cause — new variant vs. reuse `ForbiddenCall`):**
  REQ-6 pins the REJECT outcome + that the cause is "a combinator nested in a
  closure body", distinct in MEANING from `UnknownCombinator` (a free call that
  resolves to nothing) and from a generic `ForbiddenCall`. The builder MAY add a
  dedicated `SpecError::NestedCombinator { name, span }` variant (recommended —
  the diagnostic should say "extract a named `spec fn`") OR reuse `ForbiddenCall`
  with a combinator-specific `detail`. Either satisfies AC-6 as long as the
  oracle's `expected` field names the chosen identifier; this doc recommends the
  dedicated variant for crisp feedback (pillar §2.4). Not a blocker.
