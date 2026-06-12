# C10 — Binding / control-flow ergonomics (sugar over the proven core)
<!--
tier: 3-component
status: draft
audited-sha: 64a329fac3ece936c17be3d6aadf5bc125b20682 (re-pinned: the #269/#270 coordinated arc — the touched-file changes are exactly this doc's designed REQs / reviewed as claim-neutral)  (re-audited 2026-06-12: claims re-verified against current tree, #262)
governs: thermite-syntax/src/parser.rs
governs: thermite-syntax/src/ast.rs
governs: thermite-lower/src/lower.rs
thesis-refs:
  - thermite-design.md §4.1
  - thermite-design.md §4.4
  - thermite-design.md §2.3
-->

## Summary

C10 adds five binding/control-flow ergonomics that AI agents reach for
constantly — tuple destructuring `let (x, y) = e`, `for i in 0..n` loops, match
guards `x if cond =>`, or-patterns `1 | 2 =>`, and `if let` / `while let`. Each
is **SUGAR** over machinery Thermite already ships and proves: `while`+`inv`/
`dec` (loop lowering, `lower_loop` in `lower.rs`), `match` (`lower_match`),
tuple projection (`Expr::TupleProj`), and the ADT/`Option` exhaustiveness checker
(`check_match_exhaustiveness` in `validator.rs`). The C10 principle, lifted from
§4.4 ("One desugaring, always explicit") and §2.3 ("one way to do everything"):
**every ergonomic lowers to the shipped, proven core** — either by desugaring to
existing AST nodes in the parser/lower, or (where Verus has a native form that is
itself the proven core) by emitting that native form directly. No ergonomic adds
a new proof rule, weakens an obligation, or introduces a verification path that
the core does not already discharge (R-DEFER-9).

This doc is GREENFIELD for C10: probes confirm none of the five surfaces parse
today (no `for`/`in` keyword, no guard field on `MatchArm`, no `Pattern::Or`, no
`Pattern::Tuple`, no `if let`/`while let` form). Every REQ is **NOT-STARTED**,
blocked on the C10 build (#112) plus per-feature prereqs filed below.

## The desugaring map (the contract)

| Surface | Desugars to (shipped core) | New AST? | Pure-desugar or new lowering? |
|---|---|---|---|
| `let (x, y) = e;` | `let t = e; let x = t.0; let y = t.1;` (`Expr::TupleProj`) | `Pattern::Tuple` (parse-only; lowered away) | **pure-desugar** — parse → existing `Let`+`TupleProj` |
| `for i in 0..n inv … { B }` | `let mut i = 0; while i < n inv … dec n-i { B; i = i + 1; }` | `Stmt::For` (parse-only; lowered away) | **pure-desugar** — parse → existing `LoopNode`(`While`) |
| `x if cond =>` | Verus-native guarded arm `pat if cond => …` | `guard: Option<Expr>` on `MatchArm` | **new lowering** — `lower_match` emits the `if` clause |
| `1 \| 2 =>` | Verus-native or-pattern `1 \| 2 => …` | `Pattern::Or(Vec<Pattern>)` | **new lowering** — `lower_pattern` emits the `\|` join |
| `if let P = e { T } else { E }` | `match e { P => T, _ => E }` | `Stmt::IfLet` / expr form (parse-only) | **pure-desugar** — parse → existing `Expr::Match` |
| `while let P = e { B }` | `while (e is P-discriminant) { let P-bindings = e; B }` | `Stmt::WhileLet` (parse-only) | **pure-desugar** — parse → existing `LoopNode`(`While`) |

Two features (guards, or-patterns) add an AST field/variant the lowering must
emit because Verus's match **is** the proven core and supports both natively
(GROUNDED below); the other three are pure parse-time desugar to nodes that
already lower and verify.

## Requirements

- **REQ-1 (tuple destructuring `let (x, y) = e`):** A `let` binding accepts a
  `Pattern::Tuple(Vec<Pattern>)` on the left of an n-tuple-valued initializer.
  Surface: `let (x, y) = swap(a, b);`. It DESUGARS to a fresh temp plus one
  `Expr::TupleProj` per element — `let t = e; let x = t.0; let y = t.1;` — reusing
  the SHIPPED tuple-projection node (`Expr::TupleProj` in `ast.rs`, lowered to the
  Verus-native `r.0`/`r.1` by `lower_expr`, `.design/basis/10-recursion-tuples.md`
  REQ-5/REQ-8). A nested binding (`let (Some(x), y) = …`) is OUT of v0.1 scope —
  v0.1 admits only flat `Binding`/`Wildcard` sub-patterns in a tuple `let`
  (§2.3 one-way). New AST: `Pattern::Tuple` (parse-only; the desugar discharges it
  before lowering, so NO `lower_pattern` arm is required — the desugar is a parser
  transform). Derived from §4.4 ("One desugaring, always explicit") + §2.3.

- **REQ-2 (`for i in 0..n` loops):** A `for` loop over a bounded integer range
  `lo..hi` is sugar over the SHIPPED `while`+`inv`/`dec` core. Surface:
  ```thermite
  for i in 0..n
    inv acc == i
  { acc = acc + 1; }
  ```
  It DESUGARS to `let mut i = 0; while i < n inv … dec n - i { B; i = i + 1; }`
  — i.e. a `LoopNode { kind: While(i < hi), invs, dec, body }` whose body is the
  user body with `i = i + 1;` appended (a `Stmt::Assign`), and whose loop variable
  is initialized by a preceding `Stmt::Let { mutable: true, .. }`.
  - **Inv/dec surface (PINNED).** The user supplies the loop **invariant** via a
    `for … inv …` clause, mirroring `while`'s mandatory `inv` (§4.1: "Mandatory
    on every `loop`/`while`"). At least one `inv` is required (the for-loop is a
    loop; the mandatory-inv rule of §4.1 carries unchanged). The **`dec` is
    AUTOMATIC** for a bounded range `lo..hi`: the desugar synthesizes
    `dec hi - i` (the monotone, range-bounded measure), so the user does NOT write
    a `dec` for a `for`. This is the ONE place an ergonomic supplies a clause the
    user omits — and it is sound because the range bound makes the measure
    canonical (`hi - i` strictly decreases each `i = i + 1`, floored at 0).
    A `for` over a non-range iterator is OUT of v0.1 scope (no general `Iter` in
    v0.1, §4.4); only the integer-range form is admitted.
  - New AST: `Stmt::For { var, lo, hi, invs, body }` (parse-only; the desugar to
    a `LoopNode`(`While`) discharges it before `lower_loop`, so no new lowering
    arm is required — `lower_loop` already threads `inv`→`invariant` /
    `dec`→`decreases`). Derived from §4.1 (the loop model; termination proved by
    default) + §4.4 + §2.3.

- **REQ-3 (match guards `x if cond =>`):** A match arm accepts an optional guard
  `if cond`, lowered to the Verus-NATIVE guarded arm `pat if cond => body` (Verus
  0.2026 supports match guards; `lower_match` is the proven core). The guard
  `cond` is an `Expr` evaluated in the arm's binding scope. New AST:
  `MatchArm.guard: Option<Expr>` (a field on the SHIPPED `MatchArm` struct).
  **Exhaustiveness (PINNED, GROUNDED):** a guard does NOT complete a match. An
  arm `Some(v) if v < 10 => …` does NOT cover `Some(_)` — the validator
  (`check_match_exhaustiveness`) MUST treat a guarded arm as covering NONE of its
  pattern's cases for exhaustiveness purposes (the guard may fail), exactly as
  Rust/Verus does (`error[E0004]: non-exhaustive patterns: Some(_) not covered`,
  GROUNDED below). New lowering: `lower_match` emits the `if cond` after the
  pattern. Derived from §4.4 (exhaustiveness is mandatory) + §4.1.

- **REQ-4 (or-patterns `1 | 2 =>`):** A pattern accepts a `|`-joined alternation
  `Pattern::Or(Vec<Pattern>)`, lowered to the Verus-NATIVE or-pattern
  `p0 | p1 | … => body`. **Exhaustiveness (PINNED, GROUNDED):** an or-pattern
  covers EXACTLY the union of its alternatives' covered cases — `Some(_) | None`
  is exhaustive over `Option` (GROUNDED below), so the validator's
  `check_match_exhaustiveness` MUST count each alternative of an `Or` toward the
  covered-variant set (a `Pattern::Or` covering every variant closes the match;
  one covering a strict subset does not). The alternatives must bind the same set
  of names (a v0.1 restriction matching Verus); v0.1 admits literal/variant
  alternatives. New lowering: `lower_pattern` emits the `|`-joined alternatives.
  Derived from §4.4 + `.design/basis/01-adts.md` REQ-5 (exhaustiveness).

- **REQ-5 (`if let` / `while let`):** Both are sugar over the SHIPPED `match` /
  `while` core.
  - **`if let P = e { T } else { E }`** DESUGARS to `match e { P => T, _ => E }`
    (the SHIPPED `Expr::Match`, lowered by `lower_match`). An `if let` WITHOUT an
    `else` requires `()`-typed branches (the missing arm is `_ => ()`), matching
    the statement-`if` rule.
  - **`while let P = e { B }`** DESUGARS to a `while`-with-discriminant-condition:
    `while (e is P-head) { let <P-bindings> = e; B }` — i.e. a
    `LoopNode { kind: While(<e is Variant>), .. }` whose body re-binds the payload
    and runs the user body. **PINNED (GROUNDED):** the `while (cond)` form is the
    canonical desugar, NOT a `loop { match … None => break }` — the
    `loop`+`break` form fails to carry the post-exit fact without an explicit loop
    `ensures` (GROUNDED below: the `loop+break` shape is L0 on the obvious
    postcondition; the `while (e is Some)` shape is L3). The user supplies the
    loop `inv`/`dec` exactly as for a `while` (mandatory, §4.1).
  - New AST: `Stmt::IfLet` / `Expr::IfLet` and `Stmt::WhileLet` (parse-only; the
    desugar discharges them before lowering). Derived from §4.4 + §4.1 +
    `.design/basis/09-option-result.md` (`Option`/`match`).

## Acceptance criteria

- **AC-1 (tuple destructuring → L3):** `let t = swap(a, b); let x = t.0; let y =
  t.1;` (the desugar of `let (x, y) = swap(a, b);`) using `y` certifies L3.
  GROUNDED: `swap` with `ens r.0 == b, r.1 == a` + a consumer returning `y` with
  `ens r == a` → `2 verified, 0 errors`. (REQ-1)
- **AC-2 (`for` over a range → L3; auto-dec proves):** the desugar
  `let mut acc = 0; let mut i = 0; while i < n inv acc == i, i <= n dec n - i {
  acc = acc + 1; i = i + 1; }` with `ens r == n` certifies L3 — the user `inv`
  + the AUTO-`dec n - i` discharge termination. GROUNDED: `2 verified, 0 errors`.
  (REQ-2)
- **AC-2b (a bad `for`-inv → L0):** the SAME loop with the body stepping
  `acc = acc + 2` while the inv still claims `acc == i` FAILS verification
  (`invariant not satisfied`, L0) — the inv obligation BITES through the desugar,
  it is not laundered. GROUNDED: `1 verified, 1 errors`. (REQ-2)
- **AC-3 (guarded match → L3):** `match x { n if n < 10 => 0, _ => 1 }` with a
  non-vacuous `ens r == (if x < 10 { 0 } else { 1 })` certifies L3. GROUNDED
  (part of the `3 verified, 0 errors` guards/or/if-let batch). (REQ-3)
- **AC-3b (a guard does NOT complete a match → compile error):** `match e {
  Some(v) if v < 10 => v, None => 0 }` over an `Option` is NON-EXHAUSTIVE — the
  guarded `Some` arm does not cover `Some(_)`. The validator MUST reject it
  (matching Verus's `error[E0004]: non-exhaustive patterns: Some(_) not
  covered`). GROUNDED. (REQ-3)
- **AC-4 (or-pattern → L3; covers its listed cases):** `match x { 1 | 2 => true,
  _ => false }` with `ens r == (x == 1 || x == 2)` certifies L3; and `match e {
  Some(_) | None => 0 }` over an `Option` is EXHAUSTIVE (the or-pattern covers
  both variants). GROUNDED: the first in the `3 verified, 0 errors` batch, the
  second `1 verified, 0 errors`. (REQ-4)
- **AC-5 (`if let` → L3):** `match e { Some(v) => v, None => 0 }` (the desugar of
  `if let Some(v) = e { v } else { 0 }`) with `ens r == (match e { Some(v) => v,
  None => 0 })` certifies L3. GROUNDED (part of the `3 verified, 0 errors`
  batch). (REQ-5)
- **AC-6 (`while let` → L3 via `while (cond)`):** the `while cur > 0 inv count +
  cur == n, cur <= n dec cur { count += 1; cur -= 1; }` desugar (of `while let
  Some(_) = …`) with `ens r == n` certifies L3 (`2 verified, 0 errors`); the
  `loop { match … None => break }` alternative FAILS the same postcondition
  (L0) — pinning the `while (cond)` form as canonical. GROUNDED. (REQ-5)

## Architecture

All five ergonomics live at the **parser + lower** boundary; the verification
core (Verus's `while`/`match`/projection + the §4.1 `inv`/`dec` obligations +
the §4.2 vacuity gate) is UNCHANGED — that is the C10 thesis (§4.4 "one
desugaring, always explicit").

**Pure-desugar features (REQ-1 tuple, REQ-2 `for`, REQ-5 `if let`/`while let`).**
The parser builds a transient AST node (`Pattern::Tuple`, `Stmt::For`,
`Stmt::IfLet`/`WhileLet`) and a desugar pass (parser-level, or a normalize step
before lowering) rewrites it into nodes that ALREADY lower and verify:
- `Pattern::Tuple` in a `let` → `Let` + N `Let{ init: TupleProj }` (reuses
  `Expr::TupleProj`, `lower_expr` in `lower.rs`).
- `Stmt::For` → a `Stmt::Let{mutable}` (the loop var) + a `LoopNode{ kind:
  While(var < hi), invs, dec: hi - var, body: B ++ [var = var + 1] }` (reuses
  `lower_loop`, which threads `inv`→`invariant` / `dec`→`decreases`).
- `Stmt::IfLet` → `Expr::Match` (reuses `lower_match`); `Stmt::WhileLet` → a
  `LoopNode{ kind: While(e is Variant), .. }` + a payload re-bind `Let` (reuses
  `lower_loop` + `Expr::Is`, the SHIPPED discriminant test).
Because the desugar discharges these nodes BEFORE lowering, NO new `lower_*` /
`lower_pattern` / `lower_stmt` arm is required for them — the existing exhaustive
matches over `Stmt`/`Pattern`/`Expr` in the workspace are UNTOUCHED (no
match-arm ripple). The desugar is the load-bearing code, sited in the parser /
a pre-lowering normalize.

**New-lowering features (REQ-3 guards, REQ-4 or-patterns).** Verus's `match`
natively supports both, so the proven core is REUSED directly — but the AST must
carry the new shape, which RIPPLES into the exhaustive matches:
- **`MatchArm.guard: Option<Expr>` (REQ-3).** A NEW field on the SHIPPED
  `MatchArm` struct (`ast.rs`). Adding a field (not a variant) does NOT break
  exhaustive `match` arms but DOES touch every `MatchArm { pattern, body }`
  construction/destructuring site: `parser.rs` (`parse_match` builds it),
  `lower.rs` (`lower_match` emits ` if <guard>` after the pattern when `Some`),
  the L1 mirror in `l1.rs`, the `Expr`/`MatchArm` walks in `effects.rs`,
  `mutation.rs`, `vacuity.rs`, `closure.rs`, `review.rs`, `check.rs`, and the
  validator's `check_match_exhaustiveness` (a guarded arm covers NO cases). The
  skill layer (`thermite-skill/src/generate.rs`) gains a guard fragment.
- **`Pattern::Or(Vec<Pattern>)` (REQ-4).** A NEW `Pattern` variant — this DOES
  break every exhaustive `match Pattern` in the workspace (`lower_pattern` in
  `lower.rs`, the validator's pattern walks, any `address.rs`/`effects.rs`
  pattern descent). The builder MUST add an `Or` arm at each (no `_`/panic
  fallthrough — R-APG-1): `lower_pattern` emits `p0 | p1 | …`; the validator's
  `variant_pattern_name`/`check_match_exhaustiveness` counts EACH alternative
  toward the covered set; the skill gains an or-pattern fragment.

**Exhaustiveness with guards + or-patterns (REQ-3/REQ-4 — the load-bearing
validator rule).** The §4.4 exhaustiveness guarantee (`.design/basis/01-adts.md`
REQ-5, `check_match_exhaustiveness`) is PRESERVED, not relaxed:
- A **guarded** arm covers NONE of its pattern's cases (the guard may be false)
  — GROUNDED: Verus rejects a guarded-only `Some` arm as non-exhaustive.
- An **or-pattern** arm covers the UNION of its alternatives' cases — GROUNDED:
  `Some(_) | None` is exhaustive over `Option`. The validator counts each
  alternative; an `Or` over a strict subset of variants still leaves the rest
  uncovered (a `NonExhaustiveMatch` unless a `Wildcard`/`Or` closes them).

**For-loop inv/dec story (REQ-2).** The user writes the `inv` (mandatory, §4.1);
the `dec` is AUTOMATIC for a bounded range — the desugar synthesizes
`dec hi - var`, the canonical monotone measure (strictly decreases on each
`var = var + 1`, floored at 0). This is the only ergonomic that supplies an
omitted clause, and it is sound precisely because the range bound makes the
measure canonical — there is no agent choice to get wrong. A `for` whose user
`inv` does not hold is L0 (AC-2b), so the obligation still bites end-to-end.

## Verification

`cargo test -p thermite-syntax` over parse fixtures asserting the new surface
parses to the transient nodes (`Pattern::Tuple`, `Stmt::For`,
`MatchArm.guard`, `Pattern::Or`, `Stmt::IfLet`/`WhileLet`) and the desugar
rewrites them to the expected core nodes (hand-derived expected shapes,
R-CHAR-3). `cargo test -p thermite-spec` asserts the exhaustiveness rules
(guarded arm → still non-exhaustive; or-pattern → covers its cases). The
`thermite-lower` golden files (`tests/golden/lower/*`) assert the desugar emits
the SAME Verus the hand-written core would — a `for` golden diffs against the
equivalent `while` golden.

**END-TO-END GROUNDING (real `verus 0.2026.05.24.ecee80a`, this iteration).**
Each desugaring was lowered to the Verus its rewrite implies and run; the `ens`
are NON-VACUOUS (`r == n` / `r == <match>`), so a wrong value is rejected:

```
REQ-1  tuple destructuring  let t=swap(a,b); let x=t.0; let y=t.1; ens r==a   -> 2 verified, 0 errors   (L3)
REQ-2  for→while  acc==i inv + AUTO dec n-i, ens r==n                          -> 2 verified, 0 errors   (L3)
REQ-2  bad for-inv (body steps acc+2, inv still acc==i)                        -> 1 verified, 1 errors   ("invariant not satisfied", L0)
REQ-3  guarded match  n if n<10 => 0, _ => 1, ens r==(if x<10 {0} else {1})    -> in 3 verified, 0 errors (L3)
REQ-3  guard non-exhaustive  Some(v) if v<10 => v, None => 0  (no Some(_))      -> error[E0004]: non-exhaustive patterns: `Some(_)` not covered
REQ-4  or-pattern  1 | 2 => true, _ => false, ens r==(x==1||x==2)              -> in 3 verified, 0 errors (L3)
REQ-4  or-pattern exhaustive  Some(_) | None => 0  over Option                  -> 1 verified, 0 errors   (L3)
REQ-5  if let  match e { Some(v)=>v, None=>0 }, ens r==match e {...}           -> in 3 verified, 0 errors (L3)
REQ-5  while let (while cond)  while cur>0 inv count+cur==n dec cur, ens r==n   -> 2 verified, 0 errors   (L3)
REQ-5  while let (loop+break) — the rejected alternative form                   -> 1 verified, 1 errors   ("postcondition not satisfied", L0)
```

The two L0 lines (bad for-inv, loop+break while-let) are the NEGATIVE controls:
they confirm the desugar does NOT launder the obligation (R-DEFER-9) and pin the
canonical forms (auto-`dec`, `while (cond)`).

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (tuple destructuring `let (x,y)=e`) | SHIPPED | #112. PURE-DESUGAR in the parser (OQ-1 parser-site — NO new AST node). `parser::parse_let` returns `Vec<Stmt>`; a `(` after `let [mut]` routes to `parse_let_tuple_destructure`, desugaring `let (x, y) = e;` to a fresh temp `let __td<n> = e;` + per-element `let x = __td<n>.0;` (reusing the SHIPPED `Expr::TupleProj`, C9-B; `_` drops an element). Consumer: `lower_stmt` (the projection `let`s lower unchanged). Verified: `forge/tests/ergonomics_conformance.rs::req1_tuple_destructuring_certifies_l3` (real verus L3). |
| REQ-2 (`for i in 0..n` loop) | SHIPPED | #112. PURE-DESUGAR in the parser (NO new AST node — `for`/`in` are CONTEXTUAL identifiers matched by name, NOT reserved keywords, so `lexer.rs` is untouched). `parser::parse_for` (dispatched on `Ident("for")` at statement head) desugars `for i in lo..hi inv … { B }` to `let mut i = lo;` + `LoopNode { While(i < hi), invs: <user>, dec: hi - i, body: B ++ [i = i + 1;] }` — the AUTO-`dec hi - i` is a real `dec` `Clause` (`lower_loop` emits it as `decreases`, R-DEFER-9). A user `dec` on a `for` is rejected. Consumer: `lower_loop`. Verified: `forge/tests/ergonomics_conformance.rs::req2_for_range_certifies_l3` (L3) + `req2_bad_for_inv_is_l0` (a bad inv → L0). |
| REQ-3 (match guards `x if cond =>`) | SHIPPED | #112. NEW field `MatchArm.guard: Option<Expr>` (`ast.rs`). `parser::parse_match` parses an optional `if <cond>` (no-struct-literal head) before `=>`; `lower::lower_match` / `l1::lower_match_exec` emit the Verus/Rust-native guarded arm `pat if <guard> => body`; `validator::check_match_exhaustiveness` treats a guarded arm as covering NONE of its cases (a guard does NOT complete a match). The guard `Expr` is walked by every effect/combinator/spec-fn/result-mention/callee walk (lower/l1/validator/check/review/mutation/vacuity/closure). Consumer: `lower`/`validate`. Verified: `forge/tests/ergonomics_conformance.rs::req3_guarded_match_certifies_l3` (L3) + `req3_guarded_only_arm_is_non_exhaustive` (`NonExhaustiveMatch`). |
| REQ-4 (or-patterns `1 \| 2 =>`) | SHIPPED | #112. NEW variant `Pattern::Or(Vec<Pattern>)` (`ast.rs`). `parser::parse_pattern` parses a `\|`-joined alternation (flat — a single alt stays the bare pattern, byte-stable); `lower::lower_pattern` / `l1::lower_pattern_exec` emit `p0 \| p1 \| …`; `validator::collect_covered_variants` counts EACH alternative toward the covered-variant set (union — `Some(_) \| None` is exhaustive). The new variant rippled to every exhaustive `match Pattern` (lower/l1/validator/check/generate) with honest arms — NO `_`/panic. Consumer: `lower_match`/`validate`. Verified: `forge/tests/ergonomics_conformance.rs::req4_or_pattern_certifies_l3` (L3) + `req4_or_pattern_exhaustive_via_union`. |
| REQ-5 (`if let` / `while let`) | SHIPPED | #112. PURE-DESUGAR in the parser (NO new AST node). `parser::parse_if_let` (dispatched on `if` then `let` via `peek_nth`) desugars `if let P = e { T } else { E }` to the SHIPPED `Expr::Match { e, [P => T, _ => E] }` (value form, mandatory `else`); `parser::parse_while_let` desugars `while let Variant(_) = e inv … dec … { B }` to a `LoopNode { While(e is Variant), … }` (the canonical `while (cond)` form, NOT loop+break — the GROUNDED L3 vs L0 pin). Consumer: `lower_match`/`lower_loop`. Verified: `forge/tests/ergonomics_conformance.rs::req5_if_let_certifies_l3` + `req5_while_let_certifies_l3` (L3). |

## Open questions (for the orchestrator)

- **OQ-1 (desugar site — parser vs pre-lowering normalize):** the three
  pure-desugar features can rewrite in `parser.rs` (build the core node directly,
  no transient AST variant) OR build a transient `Stmt::For`/`Pattern::Tuple`/
  `Stmt::IfLet` and rewrite in a normalize pass before `lower`. The former adds
  ZERO AST variants (least ripple); the latter keeps the surface visible in the
  AST (better for addressing/round-trip). Builder's choice; the doc pins the
  desugar TARGET, not the site. Not a blocker.
- **OQ-2 (`for` range exclusivity + step):** v0.1 admits only `lo..hi`
  (exclusive, step +1). A `..=` inclusive range or a custom step is a future
  amendment (§2.3 one-way). Not a blocker.
- **OQ-3 (or-pattern binding parity):** v0.1 requires all alternatives of a
  `Pattern::Or` to bind the same names (Verus's rule). A v0.1 corpus or-pattern
  is over literals/unit-variants (no payload binding) to sidestep this. Recorded;
  not a blocker.
- **OQ-4 (`if let` without `else` — `()` only):** an `if let` with no `else`
  desugars to `_ => ()`, so both branches must be `()`-typed (the statement-`if`
  rule). A value-producing `if let` requires an `else`. Pinned to match the
  `if`-expression rule (§4.1); not a blocker.
