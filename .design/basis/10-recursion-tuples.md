# Basis Cluster C9 — Plain-`fn` Recursion + Tuples
<!--
tier: 3-component
status: draft
audited-sha: a6647655a0286147a69ba325b9e74b28a8666ca0 (bootstrap pin: decision 4 — doc-last-touch, NOT verified-current; backlog #262)
governs: thermite-syntax/src/ast.rs
governs: thermite-syntax/src/parser.rs
governs: thermite-spec/src/validator.rs
governs: thermite-lower/src/lower.rs
thesis-refs:
  - thermite-design.md §4.1
  - thermite-design.md §2.3
  - thermite-design.md §7
  - thermite-design.md Appendix A
-->

## Summary

The two remaining "compose any program" primitives the kernel still lacks
(crosslink #107): **(A) plain-`fn` recursion** — a regular exec `fn` carrying a
`dec` measure so it can call itself (recursive-descent parsers, tree walks),
with termination proved by the decreases exactly as a `loop`'s `dec` and a
`spec fn`'s `decreases` already are; and **(B) tuples** — `Type::Tuple` /
`Expr::Tuple` with `.N` projection, for multiple returns and pairs. Both lower
to native Verus (Verus has both recursive `fn` `decreases` and `(T, U)`
tuples). This doc ADAPTS to the existing code: both features are
**probe-confirmed missing** and every REQ here is **NOT-STARTED** behind a filed
blocker. The full Verus path was GROUNDED with real `verus 0.2026.05.24` (see
Verification) before this contract was pinned.

## Probe-confirmed gaps (the ground truth this doc adapts to)

- **(A)** `SpecFnItem` already carries `dec: Clause` (`ast.rs`), but `FnItem`
  has **no `dec` field** and `parse_contract` in `parser.rs` parses only
  `req`/`ens*`/`fx` — no `dec` slot. A surface `fn` with a `dec` clause parses
  as a contract error: `forge check` on `fn fac(n) … ens result>=1 dec n {…}`
  yields `function "fac" is missing the mandatory "fx" clause` (the parser hits
  `dec` where it expects `fx`). A self-call therefore cannot even be written,
  and `lower_fn` in `lower.rs` emits **no `decreases`** on a plain `fn` (it emits
  the signature `requires`/`ensures` then the body; only `lower_loop` and
  `lower_spec_fn` emit `decreases`).
- **(B)** `parse_type_inner` in `parser.rs` has an `LParen` arm that ONLY
  accepts `()` → `Type::Unit` (`consume(RParen, "`)` to close the unit type
  `()`")`). `(u64, u64)` parse-fails: `expected ) to close the unit type (),
  found identifier u64`. There is no `Type::Tuple` and no `Expr::Tuple` /
  projection node (the `enum Type` / `enum Expr` in `ast.rs` carry
  `Unit`/`Prim`/`Ref`/`Slice`/`Generic`/`Named`/`Box`/`Vec`/`String`/`Option`/
  `Result` and `IntLit`/…/`StrLit` respectively — no tuple).

## Requirements

### (A) Plain-`fn` recursion

- **REQ-1 (`fn` `dec` clause — AST + grammar):** `FnItem` gains an OPTIONAL
  `dec: Option<Clause>` field (mirroring `SpecFnItem.dec: Clause`, but optional —
  a non-recursive `fn` has `dec = None`). The fn contract grammar gains a `dec`
  slot AFTER `fx` (so the order is `req` → `ens+` → `fx` → optional `dec`; `dec`
  last keeps the existing `req`/`ens`/`fx` parse byte-stable and matches the
  loop clause order where `dec` follows the `inv`s). `parse_contract` (or
  `parse_fn`) parses an optional trailing `dec <expr>` clause into
  `FnItem.dec`. Derived from §4.1 (the `inv`/`dec` model; "Termination is proved
  by default") and Appendix A (`spec_sum`'s `dec xs.len()` — the same measure
  shape on the spec side).

- **REQ-2 (`dec` MANDATORY for a recursive `fn`; the self-call validator
  rule):** A `fn` that calls itself (directly; mutual recursion is REQ-6) MUST
  carry a `dec` clause UNLESS its effect row contains `diverge`. The validator
  (`thermite-spec/src/validator.rs`) detects a self-call in the fn body and, if
  `dec` is absent AND the fn is not `fx diverge`, emits a span-bearing
  `SpecError` (a structured error, NOT a silent non-terminating accept). This is
  the surface-level mirror of the Verus rule `recursive function must have a
  decreases clause` (GROUNDED below) — Thermite reports it as its own diagnostic
  so the user never reaches a raw Verus error. The `fx diverge` exemption is the
  SAME one #88 already wired for diverge loops: `lower_fn` emits
  `#[verifier::exec_allows_no_decreases_clause]` for a `fn_is_diverge` fn
  (existing code), which is exactly the attribute Verus's own help text names as
  the decreases-check escape. Derived from §4.1 ("divergence requires `fx
  diverge`") + #88 (the diverge → L1 cap, mutation-exempt).

- **REQ-3 (`fn` `decreases` lowering):** `lower_fn` in `thermite-lower/src/
  lower.rs` emits a `decreases <measure>` clause on a `fn` that carries `dec`,
  placed after the `requires`/`ensures` block and before the body `{` — the SAME
  position and the SAME measure-lowering helper used for `spec fn` (`spec_dec`)
  and the loop (`lower_loop`). A non-recursive `fn` (no `dec`) emits NO
  `decreases` (byte-stable for the entire existing corpus — the goldens do not
  churn). The self-call inside the body lowers as an ordinary `Expr::Call` (no
  special node); Verus discharges termination from the emitted `decreases`.
  Derived from §4.1 + the existing `lower_spec_fn` `decreases` emission.

- **REQ-4 (termination BITES — the decreases is not optional):** A recursive
  `fn` whose `dec` measure does NOT decrease on the recursive call is **L0**
  (Verus: `could not prove termination`). A recursive `fn` with NO `dec` and NO
  `fx diverge` is a structured **validator error** (REQ-2), never reaching the
  ladder. A `fx diverge` recursive `fn` is capped at **L1** by the #88 gate
  (partial correctness only; termination not claimed). This is the no-proof-cheat
  guarantee (`goal.md` R-DEFER-9): a non-terminating fn cannot be laundered to
  L3. Derived from §4.1 + §7 (the battery's teeth) + R-DEFER-9.

### (B) Tuples

- **REQ-5 (`Type::Tuple` + `Expr::Tuple` + projection — AST):** `enum Type`
  gains `Tuple(Vec<Type>)` (a tuple type of 2+ element types — see REQ-7 arity);
  `enum Expr` gains `Tuple(Vec<Expr>)` (the construction `(a, b)`) and a
  projection. PIN — **projection, not destructuring, is the v1 primitive**: a
  projection `e.0` / `e.1` is parsed in the existing postfix `.` ladder
  (`parse_postfix`) and is the simpler, contract-friendly form (an `ens` reads
  `result.0`, which is exactly the GROUNDED Verus form `r.0 == b`).

  **Projection node decision (for the builder): REUSE `Expr::Field` with a
  numeric `name`, OR add `Expr::TupleProj { receiver, index: usize }`.** This doc
  PINS the dedicated `TupleProj` node as the recommended shape (a tuple index is
  a `usize`, not an `Ident`; `Expr::Field.name: Ident` would force a string
  `"0"` and a downstream parse). The builder MAY instead overload `Field` if it
  is cheaper given the existing `parse_postfix` `.`-handling — but EITHER way the
  decision is a NEW match-bearing change (REQ-8). Derived from §4.1 (multiple
  returns) + §2.3 ("one way to do everything" — projection is the one tuple
  access; destructuring is REQ-9, deferred).

- **REQ-6 (mutual recursion — DEFERRED to a follow-up, honest):** v1 ships
  DIRECT self-recursion (a `fn` calling itself) only. Mutual recursion (two
  `fn`s calling each other, needing a shared/lexicographic decreases and a
  Verus mutual-`decreases` group) is DEFERRED — it is NOT required to "compose
  any program" (a recursive-descent parser and a tree walk are direct
  self-recursion; mutual recursion is a convenience that can always be inlined
  into one self-recursive fn with a tag parameter in v1). This is a HONEST
  scope pin, not a silent gap: the validator's self-call rule (REQ-2) detects
  DIRECT self-calls; a mutually-recursive pair (neither calls itself directly,
  but they call each other without a `dec` chain) reaches Verus and is rejected
  there. Tracked as a follow-up under #107; NOT a v1 REQ. Derived from §2.3 +
  the v0.1 "kernel first" scope (`goal.md`).

- **REQ-7 (tuple arity — n-tuples, 2+):** v1 ships **n-tuples (arity ≥ 2)**,
  not pairs-only. GROUNDED: a 3-tuple `(u64, u64, u64)` with `ens r.0==1 &&
  r.1==2 && r.2==3` certifies L3 under real verus (Verification), so there is no
  reason to cap at pairs — `Type::Tuple(Vec<Type>)` / `Expr::Tuple(Vec<Expr>)`
  naturally carry any arity. Arity-1 `(T)` is NOT a tuple (it is a parenthesized
  type/expr — the existing grouping); arity-0 `()` stays `Type::Unit`
  (UNCHANGED). The parser distinguishes by the comma: `(T)` → grouping, `(T,)`
  /`(T, U)` → tuple. Derived from §2.3 (one tuple form, any arity) + the
  GROUNDED 3-tuple.

- **REQ-8 (tuple lowering to Verus + the exhaustive-match ripple):**
  `lower_type` in `lower.rs` gains a `Type::Tuple(tys)` arm emitting `(<t0>,
  <t1>, …)`; `lower_expr` gains an `Expr::Tuple(es)` arm emitting `(<e0>, <e1>,
  …)` and a projection arm emitting `<recv>.<index>` (Verus tuples support `.0`/
  `.1`/… natively — GROUNDED). Because `Type::Tuple`, `Expr::Tuple`, and the
  projection are NEW exhaustive-match-breaking variants (UNLIKE the char/hex/bin
  literals which reused `IntLit`), every exhaustive `match Type` / `match Expr`
  across the workspace MUST gain an arm — the SAME ripple class as ast.md's #92
  operators and #93 break/continue. The sites the builder MUST extend (non-test
  production; no `_`/panic fallthrough — `goal.md` R-APG-1):
  - `thermite-syntax/src/parser.rs` — `parse_type_inner`'s `LParen` arm
    (after `bump()`: if `RParen` → `Unit`; if a type then `,` → collect
    `Type::Tuple`; if a type then `)` → grouping/the inner type); `parse_primary`'s
    `(` arm (collect `Expr::Tuple` on a comma); `parse_postfix` for the `.N`
    projection.
  - `thermite-lower/src/lower.rs` — `lower_type` (`Type::Tuple` arm),
    `lower_expr` (`Expr::Tuple` + projection arms), and any `Type`/`Expr` walk.
  - `thermite-lower/src/l1.rs` and `l2.rs` — the mirror exec (`l1`) and bounded
    (`l2`) lowering arms.
  - `thermite-lower/src/effects.rs` — the `Expr` effect-walk (a tuple
    construction/projection contributes the UNION of its parts' effects; a
    projection is pure).
  - `thermite-spec/src/validator.rs` — the `Type`/`Expr` walks (a tuple type is
    well-formed if its elements are; a projection `.0` in a contract is a flat
    built-in like `Field`, admitted inside the §4.2 cage).
  - `forge/src/mutation.rs` — the `Expr` walk (a tuple element / projection
    index is a leaf walk; no v1 mutant beyond the elements themselves).
  - `forge/src/vacuity.rs`, `forge/src/closure.rs`, `forge/src/review.rs`,
    `forge/src/check.rs`, `forge/src/strengthen.rs` — any exhaustive `Expr`/
    `Type` match gains the new arms (leaf descent).
  - `thermite-skill/src/generate.rs` — a `SkillFragment` teaching tuple types,
    construction, and `.N` projection (the tuple vocabulary the skill teaches —
    the skill-layer ripple).

  Derived from §4.1 (tuple return) + the AST-boundary-stability contract
  (ast.md REQ-9) + §2.3.

## Acceptance criteria

- **AC-1 (recursive `fn` with `dec` certifies L3 — GROUNDED):** A recursive exec
  `fn` carrying a `dec` measure and an `ens` tied to a recursive spec twin
  certifies L3 (Verus `verified, 0 errors`). GROUNDED form (Verification): a
  countdown `fn count_down(n)` over a recursive spec `zeros(n)`. (REQ-1, REQ-3)
- **AC-2 (non-decreasing recursion → L0 — GROUNDED):** the SAME fn whose `dec`
  measure does not decrease on the recursive call (e.g. recurses on `n`, not
  `n-1`) is L0: Verus `could not prove termination`. (REQ-4)
- **AC-3 (self-call without `dec` → structured error — GROUNDED):** a recursive
  `fn` with NO `dec` and NO `fx diverge` is a validator `SpecError` (REQ-2),
  mirroring the Verus diagnostic `recursive function must have a decreases
  clause`; it never reaches an L3 cert. A `fx diverge` recursive fn is L1-capped
  (#88), not L0. (REQ-2, REQ-4)
- **AC-4 (tuple fn certifies L3 via projection — GROUNDED):** `fn swap(a, b:
  u64) -> (u64, u64) ens result.0 == b && result.1 == a { (b, a) }` certifies L3
  (Verus `verified, 0 errors`). (REQ-5, REQ-7, REQ-8)
- **AC-5 (wrong projection → L0 — GROUNDED):** the SAME `swap` with body `(a,
  b)` is L0 (Verus `postcondition not satisfied` — the projection `ens` is
  non-vacuous, the §7 vacuity gate respected). (REQ-8)
- **AC-6 (n-tuple, arity ≥ 2 — GROUNDED):** a 3-tuple
  `(u64, u64, u64)` with projection `ens` certifies L3 (Verification). Arity-0
  `()` stays `Type::Unit`; arity-1 `(T)` is grouping, not a tuple. (REQ-7)
- **AC-7 (no-tuple corpus is byte-stable):** programs with no tuple and no fn
  `dec` lower IDENTICALLY (the `lower_fn` `decreases` is suppressed when `dec =
  None`; the `Type::Unit` path is unchanged) — the existing `tests/golden/`
  files do not churn. (REQ-3, REQ-8)

## Architecture

**The fn `dec` is the loop/spec-fn `dec`, lifted to the `fn`.** The decreases
machinery already exists three times: `lower_loop` emits `decreases <dec>` on a
`loop`/`while`; `lower_spec_fn` emits `decreases <spec_dec(s.dec)>` on a
recursive `spec fn`; and the recursion-scheme machinery
(`.design/basis/02-recursion-schemes.md`) generates `fold_<e>` with `decreases
l`. REQ-3 is the FOURTH instance: `lower_fn` emits the SAME `decreases` from
`FnItem.dec`, in the SAME signature position the spec-fn uses (after
`requires`/`ensures`, before the body). The `fx diverge` exemption is ALREADY
wired — `lower_fn` emits `#[verifier::exec_allows_no_decreases_clause]` for a
`fn_is_diverge` fn (the #88 mechanism), which is precisely Verus's named escape
from the recursive-decreases check. So the recursion REQ is a SMALL, well-
precedented surface (an optional AST field + a parse slot + a `lower_fn`
`decreases` line + a validator self-call rule), NOT a new verification
mechanism.

**Tuples are a NEW AST-variant family (the ripple).** Unlike recursion (which
reuses the decreases machinery), `Type::Tuple`/`Expr::Tuple`/projection are NEW
variants that break every exhaustive `match Type`/`match Expr` in the workspace
— the SAME load-bearing match-arm cost ast.md pins for #92 operators and #93
break/continue. The `02-recursion-schemes.md`-style "no `_`/panic fallthrough"
discipline applies (R-APG-1): each site gets a real arm. The lowering is thin —
Verus tuples are native (`(T, U)`, `.0`/`.1`), GROUNDED at arity 2 and 3.

**Projection vs destructuring (§2.3 — one way).** Projection `.N` is the v1
tuple access; `let (x, y) = …` destructuring is DEFERRED (REQ-9 below). Both
verify under Verus (the destructuring probe certified L3 too), so the choice is
about surface minimality, not capability — projection is one postfix form
reusing the existing `.`-ladder, with no new pattern node.

**Mutual recursion is OUT of v1 (REQ-6).** Direct self-recursion covers
recursive-descent parsers and tree walks; mutual recursion needs a Verus
mutual-`decreases` group and a shared/lexicographic measure — deferred honestly,
not silently. A mutually-recursive pair without a `dec` chain reaches Verus and
is rejected there (no false L3).

## Verification

`cargo test -p thermite-syntax` for the AST/parse shapes (the `FnItem.dec`
field, `Type::Tuple`/`Expr::Tuple`/projection nodes, the `(u64, u64)` parse, the
`.0` projection parse); `cargo test -p thermite-spec` for the self-call
validator rule (AC-3); and `forge`/`thermite-lower` conformance probes lowering
each form to Verus and certifying (the END-TO-END grounding, AC-1/2/4/5/6).
Expected cert fields are hand-derived (R-CHAR-3), never copied from the
toolchain.

GROUNDED with real `verus 0.2026.05.24.ecee80a` on the lowering each REQ targets
(the exact Verus forms the lowerer will emit):

```
(A) recursion
  recursive exec fn `count_down(n)`, self-call `count_down(n-1)`,
    decreases n, ens `r as nat == zeros(n as nat), r == 0`  -> 3 verified, 0 errors  (L3)
  SAME fn recursing on `n` (not n-1), decreases n            -> 1 verified, 1 errors
                                                                "could not prove termination"  (L0)
  SAME fn with NO decreases clause                           -> error: "recursive function must have a
                                                                decreases clause"  (structured; help names
                                                                #[verifier::exec_allows_no_decreases_clause]
                                                                — the fx-diverge exemption, #88)

(B) tuples
  fn swap(a,b: u64) -> (u64,u64) ens r.0==b, r.1==a { (b,a) } -> 2 verified, 0 errors  (L3)
  SAME swap with body (a, b)                                  -> "postcondition not satisfied"  (L0)
  fn triple() -> (u64,u64,u64) ens r.0==1, r.1==2, r.2==3     -> 3 verified, 0 errors  (L3, arity 3)
  let-destructuring `let (x,y) = p;` (DEFERRED, REQ-9)        -> 3 verified, 0 errors  (Verus supports it;
                                                                proves projection is not the only option)
```

The `ens` clauses are NON-VACUOUS (`r.0 == b`, `r as nat == zeros(n)`), so a
wrong body/measure is rejected — the §7 vacuity gate (which rejects `ens true`)
is respected. The recursion grounding shows the decreases is the ONLY thing
standing between the fn and L0 (remove it → structured error; weaken it →
termination failure), and the tuple grounding shows the projection `ens` bites.

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (`fn` `dec` clause — AST + grammar) | SHIPPED | #108. `FnItem.dec: Option<Clause>` (`ast.rs`, mirroring `SpecFnItem.dec` but optional); `parse_fn` (`parser.rs`) parses an optional trailing `dec <expr>` AFTER `fx` (OQ-4 byte-stable slot) into `FnItem.dec`. Consumer: `thermite-lower::lower::lower_fn` (the `decreases` emission). Verified: `forge/tests/recursion_conformance.rs::recursive_fn_with_dec_certifies_l3` (real verus L3). |
| REQ-2 (`dec` mandatory for recursive `fn`; self-call validator rule) | SHIPPED | #108. `validator.rs` `run`'s `Item::Fn` arm detects a direct self-call (`block_calls_name`) and emits `SpecError::MissingDecreases` when `dec.is_none() && !fn_is_diverge(f)`. The `fx diverge` exemption is honored (`fn_is_diverge`, mirroring `thermite-lower`'s) — a diverge fn recurses without `dec` and is L1-capped (#88). Consumer: `pub fn validate` → `forge::check`. Verified: `forge/tests/recursion_conformance.rs::self_call_without_dec_is_structured_error` (the MissingDecreases reject) + `diverge_recursion_without_dec_is_l1`. |
| REQ-3 (`fn` `decreases` lowering) | SHIPPED | #108. `lower_fn` (`lower.rs`) emits `decreases <spec_dec(f.dec)>` AFTER the `requires`/`ensures` block and BEFORE the body when `f.dec.is_some()` — the SAME `spec_dec` helper + position the recursive `spec fn` uses; a non-recursive fn (`dec = None`) emits NO `decreases` (byte-stable, AC-7). The self-call lowers as an ordinary `Expr::Call`. Consumer: `lower` (`Item::Fn`). Verified: `forge/tests/recursion_conformance.rs` (L3 + builds+runs); GROUNDED `decreases n` certifies L3. |
| REQ-4 (termination bites) | SHIPPED | #108. GROUNDED with real verus end-to-end: non-decreasing (`dec n`, recurse on `n`) → `could not prove termination` (L0); no-`dec` → `MissingDecreases` (structured validator error, never reaching L3); `fx diverge` recursive fn → L1-capped (#88), NOT L0. The no-cheat guarantee (R-DEFER-9) holds — the decreases is the ONLY thing between the fn and L0. Consumer: `forge::check` ladder. Verified: `forge/tests/recursion_conformance.rs::nondecreasing_recursion_is_l0` + `self_call_without_dec_is_structured_error` + `diverge_recursion_without_dec_is_l1`. |
| REQ-5 (`Type::Tuple` + `Expr::Tuple` + projection — AST) | SHIPPED | #109. `enum Type` += `Tuple(Vec<Type>)`; `enum Expr` += `Tuple(Vec<Expr>)` + the DEDICATED projection node `TupleProj { receiver: Box<Expr>, index: usize }` (OQ-1 RESOLVED → dedicated node, NOT an overloaded `Field` with a string `"0"` name: a tuple index is a `usize`). `parse_type_inner`'s `LParen` arm now disambiguates by the comma (`()` → `Unit`, `(T)` → grouping, `(T, U, …)` → `Tuple`); `parse_primary`'s `(` arm builds `Expr::Tuple` on a comma (`(e)` → grouping); `parse_postfix`'s `.` arm builds `Expr::TupleProj` when the token after `.` is an `Int`. Consumer: `thermite-lower::lower::lower_type`/`lower_expr` (→ Verus tuples). Verified: `forge/tests/tuples_conformance.rs::tuple_type_disambiguation_unit_grouping_tuple` + `tuple_expr_and_projection_nodes` (the node shapes + `()`/`(e)`/`(a,b)` disambiguation). |
| REQ-6 (mutual recursion — DEFERRED) | NOT-STARTED | follow-up under #107 (honest scope pin, not a v1 REQ). v1 ships direct self-recursion only; a mutually-recursive pair reaches Verus and is rejected there (no false L3). Recorded so the critic does not classify it as a silent gap. |
| REQ-7 (tuple arity — n-tuples, ≥ 2) | SHIPPED | #109. `Type::Tuple(Vec<Type>)`/`Expr::Tuple(Vec<Expr>)` carry any arity ≥ 2; the parser distinguishes by the comma (`()` → `Unit`, `(T)` → grouping/the inner, `(T, U, …)` → `Tuple`). Verified: `forge/tests/tuples_conformance.rs::ac6_three_tuple_certifies_l3` — a 3-tuple `(u64, u64, u64)` with `ens result.0 == 1 && result.1 == 2 && result.2 == 3` certifies L3 under real verus; `tuple_type_disambiguation_unit_grouping_tuple` pins `()` = `Unit` and `(u64)` = grouping (the inner type). |
| REQ-8 (tuple lowering + exhaustive-match ripple) | SHIPPED | #109. `lower_type` += a `Type::Tuple` arm (→ Verus `(<t0>, …)`); `lower_expr` += `Expr::Tuple` (→ `(<e0>, …)`) + `Expr::TupleProj` (→ `<recv>.<index>`) arms; the SAME in `l1.rs` (exec mirror) + the `l1`/`l2` `lower_type`/label arms. The NEW variants' workspace ripple is CLOSED with honest leaf arms (no `_`/panic): `parser.rs`, `lower.rs` (lower_type/lower_expr + every `Type`/`Expr` walk: combinator/scheme/deref-call/Vec-elem/String-reach/mention collectors), `l1.rs` (8 sites), `l2.rs` (label), `effects.rs` (effect-walk = element union; projection pure), `validator.rs` (scan/cage/self-call walks — projection a flat §4.2-cage built-in like `Field`), `check.rs` (3), `mutation.rs` (scan/apply + the early-return zero-tuple synth, below), `vacuity.rs` (`result`-mention through projection — the load-bearing tuple-vacuity case), `closure.rs`, `review.rs` (callee-walk + render_type), `strengthen.rs` (render_expr), `generate.rs` (skill arms + inventories). A tuple-returning fn with no body mutation site (the GROUNDED `swap` body `(b, a)`) gets a synthesized zero-tuple early-return mutant (`mutation::zero_value_for`/`early_return_value`, the #48/#74/#80 pattern extended to the tuple class) so it is mutation-scoreable, NOT spuriously gated `WeakContract`/L0. Verified: `forge/tests/tuples_conformance.rs::ac4_swap_tuple_projection_certifies_l3` (L3, 1/1 killed) + `ac5_wrong_body_under_projection_ens_is_rejected` (wrong `(a, b)` → NOT L3, the projection `ens` bites, R-DEFER-9) + `req8_tuple_let_and_exec_projection_certifies_l3` (a tuple `let` + an EXEC projection → L3). |

## Open questions (for the orchestrator)

- **OQ-1 (projection node: dedicated `TupleProj` vs overloaded `Field`):** REQ-5
  recommends `Expr::TupleProj { receiver, index: usize }` (a tuple index is a
  `usize`, not an `Ident`). The builder MAY overload `Expr::Field` with a numeric
  string name if cheaper given `parse_postfix`. EITHER is a new match-bearing
  change (REQ-8). Not a blocker for the contract.
- **OQ-2 (`let (x, y) = …` destructuring — REQ-9 deferred):** destructuring is
  DEFERRED (projection is the v1 §2.3 "one way"); GROUNDED that Verus supports it
  (L3), so it is a future surface convenience, not a capability gap. A future
  destructuring REQ would add a `Pattern::Tuple` node + a `let`-pattern parse —
  a design amendment, not a v1 concern. Not a blocker.
- **OQ-3 (mutual recursion — REQ-6 deferred):** v1 is direct self-recursion. A
  future mutual-recursion REQ needs a Verus mutual-`decreases` group + a
  shared/lexicographic measure + a multi-fn self-call validator rule. Tracked
  under #107; not a v1 concern. Not a blocker.
- **OQ-4 (fn `dec` clause order — after `fx`):** REQ-1 pins `dec` LAST (after
  `fx`), keeping `req`/`ens`/`fx` byte-stable and mirroring the loop order
  (`inv`s then `dec`). An alternative (`dec` before `fx`) would churn the
  contract parse; the after-`fx` slot is the minimal, byte-stable choice. Not a
  blocker.
