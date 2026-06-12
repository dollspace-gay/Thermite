# Verified Recursion Schemes (Basis Stage 2)
<!--
tier: 3-component
status: draft
audited-sha: dff9ae866e3437af272a62e078993e66c1116460 (re-audited 2026-06-12: claims re-verified against current tree, #262)
governs: thermite-syntax/src/ast.rs
governs: thermite-syntax/src/parser.rs
governs: thermite-spec/src/validator.rs
governs: thermite-spec/src/schemes.rs
governs: thermite-lower/src/lower.rs
thesis-refs:
  - thermite-design.md §4.1
  - thermite-design.md §4.2
  - thermite-design.md §4.4
  - thermite-design.md §6
-->

## Summary

Stage 2 of the universal verified primitive basis (crosslink epic **#62**) adds
**verified recursion schemes** — `fold` (catamorphism), `map`, and the
structural predicates `for_all` / `exists` / `traverse` — over the recursive
ADTs Stage 1 SHIPPED (`.design/basis/01-adts.md` REQ-3/REQ-10: `enum List { Nil,
Cons(u64, Box<List>) }`, `enum Tree { … }`, both lowering + verifying L3 today).
This is the **"prove once, compose infinitely" engine**: a recursion scheme
discharges its structural induction ONCE — inside the scheme, via `decreases
<value>` on the datatype itself (Stage 1 REQ-10, SHIPPED: Verus's built-in
structural order, no manual measure) — and thereafter every catamorphism over
the structure is verified by merely supplying the **per-node step**. The
induction is already paid for.

Stage 1 ALREADY ships a hand-written recursive `spec fn` over an ADT: the
generalized fold lowering (`is_adt_fold_sum` in `lower.rs`, the #69
generalization) lowers ANY structural fold — `sum_list`, `len`, `tree_sum` — to a
Verus recursive `spec fn` with `decreases l` over the datatype value. So a
hand-written recursive fold works end-to-end NOW. What Stage 2 ADDS is two things
Stage 1 does not have: (1) the schemes as **reusable named primitives**
(`fold`/`map`/`for_all`/`exists`/`traverse`) the surface can CALL, rather than
each fold being a fresh hand-written recursive `spec fn`; and (2) the
**`fold_bound` induction-discharge multiplier** — prove the induction once in the
scheme's law, then instantiate it for any per-node property with NO re-induction.

This doc is GREENFIELD / FORWARD-LOOKING for Stage 2's surface, but it builds on a
SHIPPED Stage 1 (recursive ADTs CONSUMED, not re-litigated): the `Box<T>`-on-`Alloc`
recursive enum, `decreases l` on the value, `*tail` dereference, and the
`is_adt_fold_sum` recursive-spec-fn lowering are all live in the tree. **Every
Stage-2 REQ below is NOT-STARTED**, tracked under epic **#62** (#62 owns this
stage — no separate blocker is filed; gaps needing an independent blocker are
noted with a fresh `#`). The verified Verus forms below — including the FULL
toolchain-shaped path (the generated `fold_List` + `fold_bound_List` + the
instance citing the law) — were produced by running the real `verus
0.2026.05.24` binary during authoring (Verification). They are the lowering
contract, not guesses.

## The multiplier: induction-discharged-once (the load-bearing mechanism)

The thing this stage exists to deliver — pinned precisely, because it is the
whole point. A naive verified program proves a property over an unbounded
structure by writing a fresh `proof fn … decreases l` structural induction for
EACH property. That does not compose: N properties cost N inductions (this is
exactly what Stage 1's hand-written recursive `spec fn` requires — a fresh
recursion per fold). A recursion scheme inverts this. The scheme `fold` carries
the recursion + `decreases`; a **generic fold law** (`fold_bound_List` below)
carries the induction ONCE, parametric in the step `f` and a per-node premise. An
instance then proves its goal by **instantiating the law with its concrete step
and discharging the (non-recursive) per-node premise** — it writes NO
`decreases`, does NO `match`, performs NO recursive call. The structural
induction is encapsulated.

GROUNDED — the FULL toolchain-shaped path (verified `9 verified, 0 errors`, see
Verification). This is the EXACT Verus the pinned mechanism emits for one ADT +
one scheme call + one instance:

```verus
enum List { Nil, Cons(u64, Box<List>) }

// GENERATED per-ADT (reuses Stage-1's is_adt_fold_sum lowering): the measure.
spec fn len_list(l: List) -> nat
    decreases l,
{ match l { List::Nil => 0, List::Cons(_, tail) => 1 + len_list(*tail) } }

// GENERATED per-(ADT, scheme): the fold catamorphism — decreases lives HERE, once.
spec fn fold_list(l: List, init: nat, f: spec_fn(u64, nat) -> nat) -> nat
    decreases l,
{
    match l {
        List::Nil => init,
        List::Cons(x, tail) => f(x, fold_list(*tail, init, f)),
    }
}

// GENERATED per-(ADT, scheme): the generic induction LAW, proven ONCE for ALL steps f.
proof fn fold_bound_list(l: List, init: nat, f: spec_fn(u64, nat) -> nat, b: nat)
    requires
        init == 0,
        forall|x: u64, acc: nat| #[trigger] f(x, acc) <= acc + b,   // PER-NODE premise
    ensures fold_list(l, init, f) <= len_list(l) * b,
    decreases l,
{ match l { List::Nil => {}
    List::Cons(x, tail) => {
        fold_bound_list(*tail, init, f, b);          // the single inductive call
        assert((len_list(*tail) + 1) * b == len_list(*tail) * b + b) by(nonlinear_arith);
    } } }
```

The INSTANCE — the Thermite surface `spec fn sum_list(l: List) -> nat { fold(l, 0,
|x, acc| x as nat + acc) }` lowers to a CALL of the generated `fold_list` with the
flat step passed as a `spec_fn`. Its bound is proven with NO induction:

```verus
spec fn sum_list(l: List) -> nat {
    fold_list(l, 0, |x: u64, acc: nat| x as nat + acc)   // the scheme call → generated fold_list
}

proof fn sum_list_bounded(l: List)
    ensures sum_list(l) <= len_list(l) * (u64::MAX as nat),
{
    let f = |x: u64, acc: nat| x as nat + acc;
    assert(forall|x: u64, acc: nat| #[trigger] f(x, acc) <= acc + (u64::MAX as nat));
    fold_bound_list(l, 0, f, u64::MAX as nat);   // <-- induction comes from the scheme's LAW
}
```

`sum_list_bounded` has no `decreases`, no `match`, no recursive call: the only
proof obligation it discharges is the FLAT per-node fact `f(x, acc) <= acc + MAX`,
then it CITES `fold_bound_list`. That is the multiplier — finitely many verified
schemes (`fold`/`map`/`for_all`/…) × the per-node lemma of an instance = an
unbounded provable slice of catamorphisms. A NEGATIVE CONTROL confirms the
induction is real, not vacuous: `fold_bound_list` with the per-node premise
REMOVED FAILS (`8 verified, 1 errors`); a `fold_list` with no `decreases` is
REJECTED by Verus (`recursive function must have a decreases clause`). The
induction does work; the premise is load-bearing.

## Decision (OQ-1 RESOLVED): the scheme mechanism — generated per-(ADT, scheme) verified spec fns + an inlined/passed flat step

The core refinement. A scheme is NOT a surface higher-order generic (§4.4 forbids
user-defined abstractions; the closed built-in set binds). It is a **generation
mechanism** keyed on the (recursive ADT, scheme) pair, plus a flat step closure.
Pinned precisely so the builder has no latitude on the load-bearing choices:

**(1) Generation — what the toolchain materializes.** When an `enum E` is
declared (or, equivalently, on first scheme use over `E`), the toolchain
GENERATES the verified scheme `spec fn`s for `E` as REAL, MATERIALIZED
`Item::SpecFn`-equivalent items in the lowered Verus, NOT emitted inline at the
call site:

- `fold_<e>` — the recursive `spec fn … decreases <value>` catamorphism (reuses
  the SHIPPED Stage-1 `is_adt_fold_sum` recursive-fold lowering verbatim — same
  `decreases l`, same `*tail` deref, same `nat` return coercion);
- `map_<e>`, `for_all_<e>`, `exists_<e>`, `traverse_<e>` — likewise, one recursive
  `spec fn` per scheme;
- `len_<e>` — the structural measure (already the Stage-1 `len` shape);
- `fold_bound_<e>` (and the per-scheme laws) — the generic induction LAW
  (`proof fn … decreases <value>`), proven ONCE, parametric in the step `f` + a
  per-node premise.

**RESOLVED: the generated scheme `spec fn`s are MATERIALIZED items** (not inlined
at each call site). Rationale: (a) they are SHARED — `N` instances over the same
`E` reuse one `fold_<e>` + one `fold_bound_<e>`, so the induction is genuinely
discharged once across the whole program, not re-emitted per call; (b) they
appear BY NAME in the audit surface (§4.2 "composition happens only through named
`spec fn`s … appears by name in the audit surface") — an inlined recursion would
be an anonymous nested recursion the cage forbids; (c) the law `fold_bound_<e>`
MUST be a named item to be `proof`-cited by an instance. The generated names are a
deterministic function of the ADT + scheme (`fold_<lowercased-enum-name>`), keyed
in the new `thermite-spec/src/schemes.rs` registry so the validator resolves a
scheme call to its generated form without a string-name guess.

**(2) The AST shape (REQ-1/REQ-2 made concrete).** RESOLVED: a scheme call REUSES
the existing `Expr::Call { callee: Box<Expr>, args: Vec<Expr> }` — the `callee` is
a `Path` naming the scheme (`fold`/`map`/`for_all`/`exists`/`traverse`), the
`args` are the scrutinee structure plus the step closure. NO new `Expr` node is
added. Rationale: the step is ALREADY `Expr::Closure { params, body }` (live in
`ast.rs` today, used by slice combinators); the scheme name is just a reserved
callee `Path` the validator/schemes-registry recognizes. A dedicated
`Expr::SchemeCall { scheme, scrutinee, step }` was considered and REJECTED — it
duplicates `Call`'s shape for no gain, and the validator already walks `Call`
callees + args, so a registry lookup on the callee `Path` (the
`combinators::lookup` precedent) is the minimal new surface. The schemes are
distinguished from ordinary calls by the `schemes.rs` registry, exactly as the 8
combinators are distinguished by `combinators.rs`'s `static REGISTRY` +
`lookup`. The step closure body is a FLAT expression (one expr, no nested scheme —
REQ-2), reusing `Expr::Closure` unchanged.

**(3) Step lowering — inlined-as-`spec_fn` (RESOLVED, grounded clean).** The
scheme call `fold(l, 0, |x, acc| x as nat + acc)` lowers to `fold_list(l, 0,
|x: u64, acc: nat| x as nat + acc)` — the flat closure is lowered to a Verus
`spec_fn` and PASSED as the higher-order step argument. This is the form that
VERIFIED cleanly end-to-end (the full path above, `9 verified, 0 errors`); the
generated `fold_list` is itself higher-order-parametric in `f: spec_fn(u64, nat)
-> nat`. A pure-text inlining of the step BODY into a freshly-generated
monomorphic `fold_list_sum_list` was the alternative; the higher-order-`spec_fn`
form was chosen because it lets ONE generated `fold_list` + ONE `fold_bound_list`
serve EVERY instance over `List` (the law cites are typed in `f` generically), and
it grounded with zero friction. (Inlining would re-generate the recursion per
instance and is what Stage 1's per-fold lowering already does — Stage 2's whole
value is NOT doing that.)

**(4) The exec form is MONOMORPHIZED (OQ-2, RESOLVED — unchanged from prior
refinement).** The SPEC scheme (above) stays higher-order/parametric — the
verified engine. The RUNNABLE (exec) `fold`/`map` is MONOMORPHIZED: the lowering
inlines the per-use step into a generated `decreases`-bearing loop (the SHIPPED
`conformance/sum.th` while-loop shape — the prover handles a simple monomorphic
loop trivially, whereas Verus exec higher-order functions are heavy). The two are
the §4.2 dual, tied by an `ensures result == fold_list(l, …)`. The exec form
carries `fx alloc` (Stage 1 REQ-3, SHIPPED in `effects.rs`) when it constructs
over a heap-allocated `List`.

## The build layer map (mirrors Stage 1's 1a/1b/1c)

Stage 2 lands in three layers, mirroring how Stage 1 shipped (1a surface → 1b
validator → 1c lowering). Each layer is a separately-verifiable cut:

- **Stage 2a — surface (governs `thermite-syntax/src/ast.rs`,
  `thermite-syntax/src/parser.rs`).** Parse a scheme CALL as `Expr::Call` with a
  scheme-name callee `Path` + a scrutinee + a step `Expr::Closure` (REQ-1, REQ-2).
  No new AST node (the §"Decision" (2) result). Deliverable: `list_fold.th` and
  `tree_fold.th` PARSE to the expected `Expr::Call`/`Expr::Closure` AST; a
  parser-fixture asserts the shape. This is the analogue of Stage 1a (the
  `Item::Struct`/`Item::Enum`/`Expr::Is`/`Expr::Deref` surface).

- **Stage 2b — validator / the cage (governs `thermite-spec/src/validator.rs`,
  `thermite-spec/src/schemes.rs`).** The `schemes.rs` registry (mirroring
  `combinators.rs`'s `static REGISTRY` + `lookup`) holds each scheme's kind +
  arity + generated-name function. The validator: (i) ACCEPTS a scheme call as a
  named-composition leaf (REQ-4, mirroring the combinator-call accept of
  `.design/spec/spectherm-combinators.md` REQ-6); (ii) REJECTS a scheme call
  NESTED inside another scheme's step closure (the flat-closure cage, REQ-2/REQ-4)
  with a span-bearing `SpecError`; (iii) enforces that the step closure body is
  flat (no combinator, no nested scheme — REQ-2). The structural-`dec` enforcement
  (REQ-5) is INHERITED from Stage 1's validator (a recursive `spec fn` already must
  carry a `dec`); the generated `fold_<e>` is checked exactly as a hand-written
  recursive `spec fn`. Deliverable: `tree_fold.th` validates; the
  nested-scheme-in-step negative REJECTS with the new `SpecError` variant. This is
  the analogue of Stage 1b (#65 — the `NonExhaustiveMatch`/`UnknownVariant` checks).

- **Stage 2c — lowering + Verus grounding (governs `thermite-lower/src/lower.rs`).**
  `lower.rs` gains `lower_scheme_defs` (GENERATE the per-(ADT, scheme) `fold_<e>`
  + `for_all_<e>` + `map_<e>` recursive `spec fn`s — reusing the SHIPPED
  `is_adt_fold_sum` recursive-fold emission), `lower_scheme_law` (emit
  `fold_bound_<e>` + the fusion laws — the analogue of `.design/lower/verus-
  lowering.md` REQ-7's shape-keyed `push_lemma_for`, NOT per-program hardcoding),
  the scheme-CALL lowering (a scheme call `Expr::Call` → a call of the generated
  `fold_<e>` with the step lowered to a `spec_fn`), and the
  instance-instantiation emission (the `proof { fold_bound_<e>(l, …); }` cite +
  the flat per-node `assert`). The monomorphized exec form (OQ-2) is the inlined
  loop. Deliverable: real `verus --no-cheating` on the emitted lowering of
  `list_fold.th` / `tree_fold.th` exits 0 with `N verified, 0 errors`; the emitted
  instance proof CITES `fold_bound_<e>` and contains NO fresh `decreases`. This is
  the analogue of Stage 1c (#67 — `lower_enum`/`lower_match`/`is_adt_fold_sum`).

## Requirements

### Surface + AST — scheme primitives (governs `thermite-syntax/src/ast.rs`, `parser.rs`)

- **REQ-1 (the scheme set as named primitives):** Thermite gains five recursion
  schemes over a recursive ADT, each a NAMED entity (per §4.2 "composition happens
  only through named `spec fn`s"): **`fold`** (catamorphism — collapse to a
  value), **`map`** (transform each element, same shape), **`for_all`** /
  **`exists`** (structural predicates over a structure's elements), and
  **`traverse`** (the `for_all`/`exists` generalization — a fold whose result is a
  `bool`). They are surfaced as a closed scheme family keyed on the recursive ADT
  + the step function, NOT as user-defined higher-order generics (§4.4 closed set
  — no user traits). **AST shape (OQ-1 RESOLVED):** a scheme CALL reuses the
  existing `Expr::Call { callee: Box<Expr>, args }` — `callee` is a `Path` naming
  the scheme, `args` are the scrutinee + the step closure; NO new `Expr` node. The
  scheme is recognized by the new `thermite-spec/src/schemes.rs` registry (the
  `combinators.rs` `lookup` precedent), which keys the generated-name function
  (`fold_<e>`). Derived from §4.2 (named composition), §4.4 (closed built-in set),
  the existing `Expr::Call` (`ast.rs`), and the GROUNDED `fold_list`/`map_list`/
  `for_all_list` Verus forms.

- **REQ-2 (the step function — flat per-node closure):** A scheme call supplies a
  **step**: a closure `|x, acc| …` (fold/traverse) or `|x| …` (map/for_all/exists)
  whose body is a FLAT predicate/expression (§4.2 closure-body rule: comparisons,
  arithmetic, field/index access, calls to named `spec fn`s — but NO combinator
  and NO nested scheme). The step REUSES the existing `Expr::Closure { params,
  body }` node (`thermite-syntax/src/ast.rs`, live today for slice combinators,
  lowered per `.design/lower/verus-lowering.md` REQ-3's `Closure` row to a Verus
  `spec_fn`). The scheme call itself is the named composition point; the step is
  the flat leaf. A nested scheme in a step is a REJECT (REQ-4). Derived from §4.2
  (flat closure bodies), the existing `Expr::Closure`, and the GROUNDED `|x: u64,
  acc: nat| x as nat + acc` step that verified cleanly as a passed `spec_fn`.

- **REQ-3 (spec form + exec form — the §4.2 dual):** Each scheme has a `spec fn`
  form (the verified primitive — total, terminating via `decreases <value>`,
  carrying NO effect row, the L1-fallback-bearing contract definition) AND, where
  the scheme collapses a structure to a value used in an EXEC body, an exec form
  (`fx`-carrying: a `fold`/`map` constructing a result over a heap-allocated
  `List` carries `fx alloc` per Stage 1 REQ-3, the constructing effect). The spec
  form is primary; the exec form is its compiled mirror, related by an `ensures`
  tying the exec result to the spec fold (`result == fold_list(l, …)`). **RESOLVED
  (#62 design-refinement, OQ-2): the RUNNABLE (exec) fold is MONOMORPHIZED** — the
  lowering INLINES the per-use step into a generated dedicated loop (the prover
  handles a simple monomorphic loop trivially, the SHIPPED `conformance/sum.th`
  while-loop pattern; higher-order EXEC closures are heavy in Verus). **The SPEC
  scheme stays higher-order / parametric** (a Verus `spec_fn` step — the verified
  engine, GROUNDED above, `9 verified 0 errors`) and is UNAFFECTED: the exec/spec
  split is exactly the §4.2 dual. Derived from §4.2 ("Spec functions are
  executable" — the L1 rung), §4.1 (`fx` rows), Stage 1 REQ-3 (SHIPPED), and the
  #62 monomorphized-exec resolution.

### Validator / the SpecTherm cage — the structural-quantification bridge (governs `thermite-spec/src/validator.rs`, `thermite-spec/src/schemes.rs`)

- **REQ-4 (the cage bridge — structural quantification via named schemes, never
  anonymous nested quantifiers):** A property that must quantify over EVERY element
  of a recursive structure ("every node of a `List`/`Tree` is `< CAP`") is the
  one place §4.2's "no anonymous nested quantifiers" cage would otherwise break —
  an inline `forall|node in tree|` is exactly the unbounded anonymous quantifier
  the cage forbids. The bridge: such quantification is expressed as a **named
  `for_all` / `exists` scheme call** lowering to a generated `for_all_<e>` carrying
  its own `decreases <value>` measure (§4.2 "Genuine nested quantification is
  written as a named `spec fn` … which may itself quantify, but carries its own
  `dec` measure"). The validator ACCEPTS a scheme call (resolved via the
  `schemes.rs` registry) as a named-composition leaf (mirroring the combinator-call
  accept of `.design/spec/spectherm-combinators.md` REQ-6) and REJECTS a scheme
  call NESTED inside another scheme's step closure (the flat-closure rule, REQ-2)
  with a span-bearing `SpecError`. `fold`/`for_all`/`map`/`exists` ARE how the cage
  expresses unbounded-structure properties. Derived from §4.2 (the cage; named
  composition), Stage 1 REQ-7 (SHIPPED — ADT predicates fit the cage; the
  caged-flat walk admits `Match`/`Field`/`Is`), and
  `.design/spec/spectherm-combinators.md` REQ-6.

- **REQ-5 (scheme termination is structural — `decreases <value>`, validator
  enforces a `dec`):** A generated scheme `spec fn` over a recursive ADT carries
  `decreases l` on the datatype VALUE (Stage 1 REQ-10, SHIPPED: Verus's built-in
  structural order, no manual measure), recursing through `Box` with `*tail`. The
  validator enforces the §4.2/§4.1 rule "no spec-level recursion without a `dec`
  measure" for every scheme exactly as for an ordinary `spec fn` — this rule is
  INHERITED from Stage 1's SHIPPED validator (a recursive `spec fn` already must
  carry a structural `dec`). A scheme whose generated definition lacks the
  structural `dec` is a `SpecError`. GROUNDED: every generated scheme form
  (`fold_list`/`map_list`/`for_all_list`) verified with `decreases l`; a
  `fold_list` with NO `decreases` is REJECTED by Verus (negative control). Derived
  from §4.2 ("No spec-level recursion without a `dec` measure"), §4.1 (termination
  by default), Stage 1 REQ-10 (SHIPPED).

### Verus lowering — schemes + the discharged induction + fusion (governs `thermite-lower/src/lower.rs`)

- **REQ-6 (scheme → generated Verus recursive `spec fn` with `decreases
  <value>`):** Each scheme over an ADT `E` lowers to a GENERATED, MATERIALIZED
  Verus recursive `spec fn` `fold_<e>`/`map_<e>`/`for_all_<e>` carrying `decreases
  l` over the datatype value, matching on the ADT's variants, recursing through
  `*tail`, and applying the step `f` (passed as a Verus `spec_fn`) at each
  `Cons`/`Node`. The generation REUSES the SHIPPED Stage-1 `is_adt_fold_sum`
  recursive-fold emission (`lower.rs`). A scheme CALL `Expr::Call` lowers to a call
  of the generated `fold_<e>` with the step `Expr::Closure` lowered to a `spec_fn`
  (the `Closure` row of `.design/lower/verus-lowering.md` REQ-3). For a predicate
  scheme (`for_all`/`exists`) the result is `bool`; for `fold` it is the
  accumulator type (`nat`); for `map` it is the same ADT. **GROUNDED** (`9 verified
  0 errors` on the full path): `fold_list`/`map_list`/`for_all_list` over `List`
  with `decreases l`, `*tail`, and `Box::new(map_list(*tail, g))` for the `map`
  reconstruction; the `sum_list` INSTANCE lowering to `fold_list(l, 0, |x, acc| x
  as nat + acc)`. Derived from §4.4 ("transpile to Verus" — §3 stack), Stage 1
  REQ-10 (SHIPPED), the GROUNDED scheme forms.

- **REQ-7 (the induction-discharged-once contract shape — the multiplier
  lowering):** Each scheme over `E` ships a GENERATED **generic structural law**
  `fold_bound_<e>` (a `proof fn` parametric in the step `f` and a per-node premise,
  carrying the single `decreases l` induction) so an INSTANCE proves its goal by
  CITING the law + discharging a FLAT per-node premise — emitting NO fresh
  `decreases`/`match`/recursive call. The lowerer emits, per (ADT, scheme), the
  generic law as a proof aid (the analogue of `.design/lower/verus-lowering.md`
  REQ-7's shape-keyed templates, NOT per-program hardcoding) and, at an instance
  site, the instantiating `proof { fold_bound_<e>(l, init, f, b); }` cite plus the
  flat per-node `assert`. **GROUNDED (FULL toolchain-shaped path)**: `fold_bound_list`
  (the generated generic law, single induction) + `sum_list_bounded` (the instance
  — ZERO induction: only a flat `forall|x,acc| f(x,acc) <= acc + b` assert, then
  the `fold_bound_list(l, 0, f, MAX)` cite) verified `9 verified, 0 errors`. The
  negative control — `fold_bound_list` minus the per-node premise — FAILS (`8
  verified, 1 errors`), proving the premise is load-bearing and the induction
  non-vacuous. Derived from §6 (L3 is a real SMT proof, R-DEFER-9 no vacuity),
  §4.2, the GROUNDED multiplier proof.

- **REQ-8 (fusion / composition laws — schemes compose):** The lowering pins the
  scheme algebra so a verified scheme composes with another into a SINGLE verified
  scheme rather than a re-proof: **(a)** `map` preserves length
  (`len_list(map_list(l, g)) == len_list(l)`) — the structure-preservation law a
  downstream `fold` over a mapped list reuses; **(b)** `fold` after `map` fuses to
  a single fold (`fold_list(map_list(l, g), init, f) == fold_list(l, init, |x, acc|
  f(g(x), acc))`); **(c)** `map` of a composition is the composition of maps
  (`map_list(map_list(l, g), h) == map_list(l, |x| h(g(x)))`). Each fusion law is a
  `proof fn … decreases l` proven ONCE; a pipeline reuses it instead of
  re-inducting. **GROUNDED**: `map_preserves_len_list` (`len_list(map_list(l, g))
  == len_list(l)`) verified `0 errors` by structural recursion (part of the
  `9 verified` run). Laws (b)/(c) are pinned as the fusion family (OQ-3 flags which
  the v0.1 corpus must exercise vs. carry isolation-verified). Derived from §4.2
  (named composition), §6, the GROUNDED `map_preserves_len_list`.

- **REQ-9 (`LowerError`/`SpecError` extension, no panics):** The scheme constructs
  extend the EXISTING `thermite-lower::LowerError` (`.design/lower/verus-lowering.md`
  REQ-9) and `thermite-spec::SpecError` enums with span-bearing variants for the
  new failure modes (a scheme nested in a step closure — REQ-4; an un-lowerable
  scheme over a non-ADT value), reusing `thermite_syntax::lexer::Span`. The
  structural-`dec` reject (REQ-5) reuses Stage 1's existing recursive-`spec fn`
  `dec` diagnostic. No `unwrap`/`expect`/`panic!` in production (R-CODE-2 /
  R-APG-1). Derived from R-CODE-2, the existing error-enum discipline in
  `validator.rs` / `lower.rs`.

## Acceptance criteria

The orchestrator authors a NEW corpus program — call it `conformance/list_fold.th`
(the Stage-1 `enum List` + `spec fn sum_list(l) = fold(l, 0, |x, acc| x as nat +
acc)` + a `bounded` caller proving `sum_list(l) <= len(l) * MAX` by INSTANTIATING
the generated `fold_bound_list` law, NOT a fresh induction) — and a `tree`-shaped
program — call it `conformance/tree_fold.th` (a `Tree` from Stage 1 + a `for_all`
"every node `< CAP`" property certified via the generated `for_all_tree` + a `map`
+ the `map_preserves_len_tree` fusion law). The existing slice fold
`conformance/sum.th` is noted as the SLICE-INSTANCE prototype (the `Seq` fold
`spec_sum` of `.design/lower/verus-lowering.md` REQ-5 is the slice-shaped
precursor of this ADT fold). Golden lowerings live at
`tests/golden/lower/list_fold.verus.rs` / `tree_fold.verus.rs`, hand-authored from
this doc and confirmed to pass `verus`; certificate goldens at
`conformance/{list_fold,tree_fold}.cert.json`.

- **AC-1 (a fold scheme + its generated law parses, validates, lowers, certifies
  L3):** Parsing the `fold` call in `list_fold.th` yields an `Expr::Call`
  (scheme-name callee) + an `Expr::Closure` step (REQ-1, REQ-2); the validator
  resolves it via the `schemes.rs` registry, accepts it as a named-composition leaf
  (REQ-4); the lowerer GENERATES `fold_list … decreases l` + `len_list` + the
  generic `fold_bound_list` law (REQ-6, REQ-7); running the real `verus` binary on
  the emitted output exits 0 with `N verified, 0 errors`. The GROUNDED full path
  (`fold_list` + `fold_bound_list` + `sum_list` + `for_all_list` + `map_list` +
  `map_preserves_len_list`, `9 verified, 0 errors`) is the verified seed.
  (REQ-1, REQ-3, REQ-5, REQ-6, REQ-7.)

- **AC-2 (induction-discharged-once — the instance proves with NO fresh
  induction):** `list_fold.th`'s `bounded` certifies L3 by CITING `fold_bound_list`
  + discharging the flat per-node premise — the emitted instance proof contains NO
  `decreases`, NO `match`, NO recursive `proof fn` call other than the single
  `fold_bound_list(l, 0, f, MAX)` cite. Mechanically: the emitted instance-proof
  body contains `fold_bound_` and does NOT contain a `decreases` clause (the
  multiplier is observable in the output). The NEGATIVE control — the generated law
  minus its per-node premise — FAILS `verus` (`8 verified, 1 errors`), pinned as a
  reject fixture proving non-vacuity (R-DEFER-9, §7). (REQ-7.)

- **AC-3 (the cage bridge — structural quantification is a NAMED scheme, not an
  anonymous quantifier):** A `tree_fold.th` property "every node `< CAP`" parses to
  a NAMED `for_all` scheme call (REQ-4), validates as a named-composition leaf, and
  lowers to a generated Verus `spec fn for_all_tree … decreases l`; `verus`
  certifies it. A crafted negative — a scheme call NESTED inside another scheme's
  step closure (the flat-closure violation, REQ-2/REQ-4) — REJECTS with the
  span-bearing `SpecError`. GROUNDED `for_all_list` over `List` (`0 errors`, part
  of the `9 verified` run). (REQ-2, REQ-4, REQ-6, REQ-9.)

- **AC-4 (map + a fusion law certifies L3):** A `map` scheme parses (REQ-1), lowers
  to a generated Verus `spec fn map_<e> … decreases l` reconstructing via
  `Box::new` (REQ-6), and at least one fusion law (`len_list(map_list(l, g)) ==
  len_list(l)`, REQ-8) certifies L3 by structural recursion proven ONCE — a
  downstream `fold` over the mapped list reuses it without re-inducting. GROUNDED
  `map_list` + `map_preserves_len_list` (`0 errors`). (REQ-1, REQ-6, REQ-8.)

- **AC-5 (the slice fold + the Stage-1 hand-written fold are the prototypes / no
  regression):** `conformance/sum.th` is UNCHANGED — its `Seq` fold `spec_sum`
  (`.design/lower/verus-lowering.md` REQ-5) still lowers byte-stably and certifies
  L3. Stage 1's SHIPPED `list_sum.th` (the hand-written recursive `spec fn sum_list`
  lowered via `is_adt_fold_sum`) is UNCHANGED — Stage 2 does NOT reshape it; the
  NEW `list_fold.th` is the scheme-CALL form alongside it (the hand-written form
  remains the `is_adt_fold_sum` path, the scheme form is the generated-`fold_list`
  path). Mechanically: `cargo test -p thermite-syntax -p thermite-spec -p
  thermite-lower` + the conformance corpus pass with 0 mismatches;
  `tests/golden/lower/{sum,list_sum}.verus.rs` stay green. (All REQs; the engine
  must not break the SHIPPED Stage 1.)

- **AC-6 (reject + no-panic cases):** Crafted negatives reject with the right
  structured variant: a scheme nested in a step closure → the REQ-4/REQ-9 cage
  `SpecError`; a scheme over a non-ADT value → `LowerError`; a generated scheme
  `spec fn` lacking its structural `dec` → the Stage-1 recursive-`spec fn` `dec`
  `SpecError` (inherited). Lowering never panics; lowering the corpus returns `Ok`.
  Hand-derived expectations (R-CHAR-3), never read back from the toolchain's own
  output. (REQ-5, REQ-9.)

## Architecture

The component spans four crates, all additively, atop Stage 1's SHIPPED recursive
ADTs:

- **`thermite-syntax`** — a scheme CALL reuses the existing `Expr::Call` (`callee:
  Path` naming the scheme, args = the scrutinee + the step closure; OQ-1 RESOLVED —
  no new `Expr` node). The step closure (REQ-2) REUSES the existing `Expr::Closure`
  (`thermite-syntax/src/ast.rs`, anchor `enum Expr` / `Closure`). `parser.rs` needs
  no new node — a scheme call parses as an ordinary call; the scheme-ness is a
  validator/registry concern. The mandatory-contract discipline is unchanged: a
  generated scheme `spec fn` carries a `dec`, no `req`/`ens`/`fx` (it is spec); an
  exec scheme form (REQ-3) carries `fx` per Stage 1 REQ-3.

- **`thermite-spec`** — a NEW `thermite-spec/src/schemes.rs` registry (the analogue
  of `combinators.rs`'s `static REGISTRY` + `pub fn lookup`) holds each scheme's
  kind, arity, and generated-name function (`fold_<e>`). `validator.rs` gains the
  scheme-as-named-composition accept (REQ-4, mirroring the combinator-call accept of
  `.design/spec/spectherm-combinators.md` REQ-6 — resolved through the registry),
  and the nested-scheme-in-step rejection (REQ-2/REQ-4). The caged-flat walk
  (`walk_expr_inner` — SHIPPED Stage 1 REQ-7, admits `Match`/`Field`/`Is`/`Deref`
  as flat built-ins) is UNCHANGED: a scheme call joins combinator calls and named
  `spec fn` calls as a named-composition accept; a scheme nested in a closure body
  is the only NEW reject. New `SpecError` variants (REQ-9). The structural-`dec`
  enforcement (REQ-5) is inherited from Stage 1's SHIPPED recursive-`spec fn` check.

- **`thermite-lower`** — `lower.rs` gains `lower_scheme_defs` (GENERATE the
  per-(ADT, scheme) recursive `spec fn fold_<e>/map_<e>/for_all_<e> … decreases l`
  — REUSING the SHIPPED `is_adt_fold_sum` emission), `lower_scheme_law` (generate
  `fold_bound_<e>` + the fusion laws — the shape-keyed proof-aid analogue of
  `.design/lower/verus-lowering.md` REQ-7's `push_lemma_for`, NOT per-program
  hardcoding), the scheme-CALL lowering (a scheme `Expr::Call` → a call of the
  generated `fold_<e>` with the step `Expr::Closure` lowered to a `spec_fn`), the
  instance-instantiation emission (the `proof { fold_bound_<e>(…); }` cite + flat
  per-node assert), and the fusion-law emission (REQ-8). The two lowering contexts
  (exec vs spec, `.design/lower/verus-lowering.md`) carry over: the spec scheme
  form is the generated higher-order `spec fn`; an exec scheme form is its
  monomorphized mirror (OQ-2). Symbol anchors: `enum Expr` (`Closure`/`Call`) in
  `ast.rs`; `pub fn validate` in `validator.rs`; `static REGISTRY` / `pub fn
  lookup` in `combinators.rs` (the `schemes.rs` registry mirrors it); `pub fn
  lower` / `is_adt_fold_sum` / `lower_spec_fn` in `lower.rs`.

### The verified Verus forms (GROUNDED — the lowering contract, not guesses)

Produced by running the real `verus 0.2026.05.24` binary during authoring
(Verification). They are the seed for the golden files. The FULL toolchain-shaped
path (the generated `len_list` + `fold_list` + `fold_bound_list` + the `sum_list`
instance citing the law + `for_all_list` + `map_list` + `map_preserves_len_list`)
verified `9 verified, 0 errors`.

**The fold scheme + the generated discharged-once law + the instance (REQ-6,
REQ-7).** See the "multiplier" section above for the full `fold_list` /
`fold_bound_list` / `sum_list` / `sum_list_bounded` quad — verified as part of the
`9 verified, 0 errors` run. The load-bearing shapes: the generated `fold_list`
carries `decreases l`; the generated generic law `fold_bound_list` carries the
single induction parametric in `f` + a per-node premise; the instance `sum_list`
is a CALL of `fold_list` with the step passed as a `spec_fn`; the instance proof
`sum_list_bounded` carries NEITHER a `decreases` NOR a recursive call — only a
flat per-node `assert` and the `fold_bound_list(l, 0, f, MAX)` cite.

**The cage bridge — `for_all_list` as a named structural quantifier (REQ-4).**

```verus
spec fn for_all_list(l: List, p: spec_fn(u64) -> bool) -> bool
    decreases l,
{
    match l {
        List::Nil => true,
        List::Cons(x, tail) => p(x) && for_all_list(*tail, p),
    }
}
```

This is how "every node satisfies `p`" is written WITHOUT an anonymous nested
quantifier: a generated named `spec fn` carrying its own `decreases l` (§4.2). A
contract writes `for_all(l, |x: u64| x < CAP)` — the scheme is the
named-composition leaf, the closure `|x| x < CAP` is the flat per-node predicate
(REQ-2), and it lowers to `for_all_list(l, |x: u64| x < CAP)`.

**The map scheme + a fusion law (REQ-6, REQ-8).**

```verus
spec fn map_list(l: List, g: spec_fn(u64) -> u64) -> List
    decreases l,
{
    match l {
        List::Nil => List::Nil,
        List::Cons(x, tail) => List::Cons(g(x), Box::new(map_list(*tail, g))),
    }
}

proof fn map_preserves_len_list(l: List, g: spec_fn(u64) -> u64)
    ensures len_list(map_list(l, g)) == len_list(l),
    decreases l,
{
    match l {
        List::Nil => {}
        List::Cons(_, tail) => { map_preserves_len_list(*tail, g); }
    }
}
```

`map_list` reconstructs the structure with `Box::new` at each `Cons` (Stage 1's
SHIPPED heap primitive, REQ-3); `map_preserves_len_list` is the
structure-preservation fusion law a downstream `fold` over `map_list(l, g)` reuses
(REQ-8) instead of re-inducting.

**RECORDED FINDING (the multiplier is real, not vacuous).** Two negative controls
were run and BOTH fail correctly: (1) `fold_bound_list` with the per-node premise
REMOVED fails its postcondition (`8 verified, 1 errors`) — the per-node premise is
load-bearing, the bound is not provable for an arbitrary step; (2) a `fold_list`
with NO `decreases` clause is REJECTED by Verus (`recursive function must have a
decreases clause`). This proves the structural induction inside the generated
scheme does real work and the instance genuinely depends on the per-node lemma —
the "discharged once" claim is grounded, not a vacuous restatement.

**RECORDED FINDING (the step verifies cleanest as a passed `spec_fn`, not an
inlined body — the OQ-1 (3) resolution).** All GROUNDED forms pass the step as a
Verus `spec_fn(u64, nat) -> nat` (or `spec_fn(u64) -> bool` for predicates), with
the generated `fold_<e>`/`for_all_<e>`/`map_<e>` higher-order-parametric in the
step. The instance `sum_list` is `fold_list(l, 0, |x, acc| x as nat + acc)` — the
flat closure lowered to a `spec_fn` and PASSED. This verified end-to-end with zero
friction (`9 verified, 0 errors`) AND lets ONE generated `fold_list` + ONE
`fold_bound_list` serve every instance over `List`, which is the whole multiplier.
The alternative — inlining the step BODY into a freshly-generated monomorphic
`fold_list_sum_list` per instance — was REJECTED: it re-generates the recursion
per instance (exactly what Stage 1's `is_adt_fold_sum` already does for a
hand-written fold), defeating the "discharge once across the program" value. The
SPEC scheme — the verified contract surface, the engine — is fully grounded as the
passed-`spec_fn` form. The EXEC mirror's monomorphized shape (OQ-2) is unaffected.

## Dependency hooks (for the rest of epic #62)

- **Stage 1 (consumed — recursive ADTs, SHIPPED end-to-end):** this stage builds
  DIRECTLY on Stage 1 REQ-3/REQ-10 (both SHIPPED). `decreases l` on the datatype
  value (Stage 1 REQ-10), `*tail` dereference (`Expr::Deref` → `*t`), `Box::new`
  reconstruction (`map`, Stage 1 REQ-3's heap primitive), the `is_adt_fold_sum`
  recursive-fold emission, and REQ-7's "named `spec fn` composition" (Stage 1 REQ-7,
  SHIPPED — the cage accepts named recursion) are all consumed verbatim. Stage 1's
  SHIPPED `sum_list`/`len` (`.design/basis/01-adts.md`) ARE the fold-precursors;
  Stage 2 recasts them as `fold` scheme INSTANCES (the generated-`fold_list` form
  ALONGSIDE the existing hand-written form, not replacing it). Stage 2 can begin
  NOW — Stage 1's recursive-type + `decreases l` lowering (REQ-10) has landed.

- **Stage 4 (collections — fold over `Vec`):** a `fold`/`map` over a `Vec<T>` is
  the SAME scheme family generalized to the dynamic collection (Stage 4's heap
  generalization of Stage 1's `Box`/`alloc`). The `Seq` fold `spec_sum`
  (`.design/lower/verus-lowering.md` REQ-5) is the slice-shaped fold; a `Vec` fold
  is `vec@`-viewed to the same `Seq` fold. The scheme set + the generated
  discharged-once law shape (REQ-7) carry over unchanged — only the underlying
  structure changes.

- **Stage 5 (composition law — schemes compose):** REQ-8's fusion laws ARE the
  composition multiplier at the data-recursion level: `fold ∘ map` fuses, `map ∘
  map` fuses, so a verified pipeline of schemes is itself a verified scheme. The §9
  composition rule ("if `g` calls `f` only through `f`'s contract …") applies to a
  scheme instance through its generated law's `ensures` — a caller reasons about
  `fold_list(l, …)` through `fold_bound_list`'s contract, never by re-opening the
  recursion.

## Verification

- **Mandatory Verus grounding (DONE during authoring — real `verus
  0.2026.05.24`).** A single `verus!{}` file built on Stage 1's SHIPPED `enum List
  { Nil, Cons(u64, Box<List>) }` containing the FULL toolchain-shaped path: the
  generated `len_list` (the structural measure); the generated `fold_list`
  catamorphism with `decreases l`; the generated GENERIC `fold_bound_list` law
  (single induction, parametric in the step + a per-node premise); the `sum_list`
  fold INSTANCE (a CALL of `fold_list` with the step passed as a `spec_fn`) +
  `sum_list_bounded` (proved by CITING `fold_bound_list`, NO fresh induction); the
  generated `for_all_list` cage-bridge scheme; the generated `map_list` scheme + the
  `map_preserves_len_list` fusion law:

  ```
  verus --no-cheating /tmp/scheme_ground/scheme.rs
  verification results:: 9 verified, 0 errors
  ```

  Cheat-token grep (`assume`/`external_body`/`admit`/`verifier::external`) over the
  file: NONE (0 matches) — run under `--no-cheating`. **Two negative controls
  confirm non-vacuity:** (1) `fold_bound_list` with the per-node premise removed
  FAILS (`8 verified, 1 errors` — the bound is unprovable for an arbitrary step);
  (2) a `fold_list` with no `decreases` is REJECTED by Verus (`recursive function
  must have a decreases clause`). This proves the FULL path — a Thermite scheme
  call becoming the generated `fold_list` + the generated law + the law-citing
  instance — is Verus-feasible end to end AND that the discharge does real work.

- **AC-1/AC-2/AC-3/AC-4:** `cargo test -p thermite-syntax -p thermite-spec -p
  thermite-lower`, plus a harness that shells the real `verus` binary on the emitted
  lowering of `list_fold.th` / `tree_fold.th` and asserts exit 0 + `N verified, 0
  errors` (R-CODE-4: subprocess status checked, never swallowed), plus the AC-2
  structural assertion (the emitted instance proof contains `fold_bound_` and NO
  `decreases`), plus `forge check` matching the golden certificates.
- **AC-2/AC-6 negatives:** the per-node-premise-removed fold law and the
  nested-scheme-in-closure / non-ADT-scheme rejects are reject fixtures with
  hand-derived expectations (R-CHAR-3).
- **AC-5:** the existing `tests/golden/lower/{sum,list_sum}.verus.rs` and
  `conformance/sum.cert.json` assertions stay green (no regression on the slice
  fold or the SHIPPED Stage-1 hand-written fold).

Gauntlet (R-DEFER-6, per crate): `cargo test -p <crate>`, `cargo clippy -p <crate>
--all-targets -- -D warnings`, `cargo fmt --check`.

## Routes to add (orchestrator)

This stage adds NEW concerns to files that already carry routes, plus one new file
(`thermite-spec/src/schemes.rs`). The orchestrator adds these routes to
`tooling/spec-routes.toml` pointing at THIS doc (a file may carry multiple
governing docs — the `lower.rs` precedent):

```
[[route]]  crate_pattern = "thermite-syntax/src/ast.rs"        design = ".design/basis/02-recursion-schemes.md"   reference = ["conformance/list_fold.th", "conformance/tree_fold.th"]
[[route]]  crate_pattern = "thermite-syntax/src/parser.rs"     design = ".design/basis/02-recursion-schemes.md"   reference = ["conformance/list_fold.th", "conformance/tree_fold.th"]
[[route]]  crate_pattern = "thermite-spec/src/validator.rs"    design = ".design/basis/02-recursion-schemes.md"   reference = ["conformance/tree_fold.th"]
[[route]]  crate_pattern = "thermite-spec/src/schemes.rs"      design = ".design/basis/02-recursion-schemes.md"   reference = ["conformance/list_fold.th"]
[[route]]  crate_pattern = "thermite-lower/src/lower.rs"       design = ".design/basis/02-recursion-schemes.md"   reference = ["tests/golden/lower/list_fold.verus.rs", "tests/golden/lower/tree_fold.verus.rs"]
```

The corpus programs `conformance/list_fold.th`, `conformance/tree_fold.th`, their
`.cert.json` goldens, and the `tests/golden/lower/*.verus.rs` lowerings are
authored by the orchestrator from this doc before the builder runs (R-CHAR-3).

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (the scheme set as named primitives; AST = `Expr::Call` + registry, OQ-1 RESOLVED) | SHIPPED | #70. `thermite-spec/src/schemes.rs` `static REGISTRY: [SchemeSig; 5]` (`fold`/`map`/`for_all`/`exists`/`traverse`) + `lookup`; consumed by `validator::walk_call` (the scheme-call accept) and `thermite_lower::lower::collect_scheme_uses`/`SchemeSig::generated_fn_name`. Asserted against `conformance/adt-schemes/cases.json` in `thermite-spec/tests/scheme_validate.rs::list_fold_validates`. |
| REQ-2 (the step — flat per-node closure) | SHIPPED | #70. `validator::check_scheme` requires an `Expr::Closure` step of `SchemeSig::step_shape.arity()` params (`SchemeStepShape`) and walks the body in `in_scheme_step` mode; `walk_call` rejects a nested scheme/combinator there with `SpecError::NestedScheme`. Verified: `scheme_validate.rs::reject_cases_yield_the_oracle_error` (`nested_scheme_in_step` → "nested"). |
| REQ-3 (spec form + exec form — exec MONOMORPHIZED, RESOLVED) | NOT-STARTED | epic **#62** Stage 2c. The SPEC scheme (the generated higher-order `fold_<e>` with the step passed as a `spec_fn`, the verified engine) is SHIPPED (REQ-6). The MONOMORPHIZED EXEC mirror is NOT implemented: the v0.1 corpus `list_fold.th` is SPEC-ONLY (all three items are `spec fn`), so no exec scheme is exercised yet. The exec mirror lands when a corpus exec fn folds an ADT. |
| REQ-4 (cage bridge — named structural quantification) | SHIPPED | #70. `validator::walk_call` ACCEPTS a top-level scheme call as a named-composition leaf (via `schemes::lookup`) and REJECTS a scheme nested in a step / combinator closure (`NestedScheme`); the caged-flat walk (`walk_expr_inner`, Stage 1 REQ-7) is unchanged. The generated `for_all_list` cage form verifies. Verified: `scheme_validate.rs::list_fold_validates` (`for_all(l, |x| x > 0)` validates). |
| REQ-5 (structural `decreases <value>` enforcement) | SHIPPED | #70. Each generated scheme `spec fn` (`emit_scheme_spec_fn`) + the law (`emit_fold_bound_law`) carries `decreases l` over the datatype value, inheriting Stage 1's recursive-`spec fn` `dec` discipline. Verified: real `verus --no-cheating` `verified, 0 errors` on the emitted `list_fold.th`; the negative-control no-`decreases` fold is rejected by Verus (grounded during authoring). |
| REQ-6 (scheme → generated Verus recursive `spec fn` + `decreases <value>`) | SHIPPED | #70. `thermite_lower::lower::emit_scheme_defs` GENERATES `fold_<e>`/`for_all_<e>`/… (`emit_scheme_spec_fn`, `decreases l`, `*tail`, `Box::new`) + the measure `<e>_len`; a scheme CALL lowers via `lower_scheme_call` to a call of the generated fn with the step lowered to a typed `spec_fn` (`lower_step_closure`). Consumer: `lower`. Verified: `thermite-lower/tests/adt_schemes_conformance.rs::list_fold_lowers_to_generated_schemes_and_verifies_l3` (real `verus --no-cheating` `verified, 0 errors`). |
| REQ-7 (induction-discharged-once contract shape — the multiplier) | SHIPPED | #70. `emit_fold_bound_law` GENERATES `fold_bound_<e>` (single `decreases l` induction, parametric in `f` + a per-node premise); an instance bound is proven by CITING it with NO fresh induction. Consumer: `lower`. Verified: `adt_schemes_conformance.rs::multiplier_instance_cites_the_generated_law_no_fresh_induction` (`verus --no-cheating` `verified, 0 errors`; the instance proof cites `fold_bound_list`, no `decreases`) + `negative_control_premise_removed_fails_verus` (premise removed → verus error; the induction is real). |
| REQ-8 (fusion / composition laws) | NOT-STARTED | epic **#62** Stage 2c. `map_<e>` generation is shipped (`emit_scheme_spec_fn` `SameAdt`), but no fusion-law (`map_preserves_len_<e>`, `fold∘map`, `map∘map`) emission yet; the v0.1 corpus `list_fold.th` does not exercise `map`/fusion (OQ-3 — the fusion family ships when a pipeline corpus program exercises it). GROUNDED during authoring (`map_preserves_len_list` `0 errors`). |
| REQ-9 (`LowerError`/`SpecError` extension, no panics) | SHIPPED | #70. `SpecError::{NestedScheme, SchemeWrongArity, SchemeStepShape}` (span-bearing) in `validator.rs`; the scheme lowering reuses `LowerError::Unsupported`/`TooDeep` (a scheme over a non-ADT value / un-resolvable scrutinee). The DEC NUANCE is resolved: a scheme-call instance body lowers WITHOUT a spurious `decreases` (`lower_spec_fn` suppresses it for `is_scheme_call_body`); the generated fold/law carry their own. No `unwrap`/`expect`/`panic!` in `src/`. |

## Open questions (for the orchestrator before the builder runs)

- **OQ-1 (scheme AST + generation mechanism — RESOLVED).** *(The core of this
  refinement.)* **RESOLVED:** (a) a scheme CALL reuses the existing `Expr::Call`
  (`callee: Path` naming the scheme, args = scrutinee + step `Expr::Closure`) — NO
  new `Expr` node; the scheme is recognized by a NEW `thermite-spec/src/schemes.rs`
  registry mirroring `combinators.rs`'s `static REGISTRY` + `lookup`. (b) The
  toolchain GENERATES, per (ADT, scheme), MATERIALIZED Verus recursive `spec fn`s
  (`fold_<e>`/`map_<e>`/`for_all_<e>`/…, reusing the SHIPPED `is_adt_fold_sum`
  emission) + the generic law `fold_bound_<e>` (`proof fn`, single induction) — NOT
  emitted inline, because they are SHARED across instances and must appear by name
  in the audit surface (§4.2). (c) The step lowers as a PASSED Verus `spec_fn`
  (grounded cleanest, `9 verified, 0 errors`), so ONE generated `fold_<e>` + ONE
  `fold_bound_<e>` serves EVERY instance over `E`. Fully grounded end-to-end; the
  builder has no latitude on these three load-bearing choices.

- **OQ-2 (exec higher-order folds vs. monomorphized exec lowering — RESOLVED;
  #62 design-refinement).** **RESOLVED: the RUNNABLE (exec) fold is MONOMORPHIZED**
  — the lowering inlines the per-use step into a generated dedicated `decreases`-
  bearing loop (the verified `conformance/sum.th` while-loop shape, SHIPPED), NOT a
  true higher-order exec function (Verus exec higher-order closures are heavy).
  **The SPEC scheme stays higher-order / parametric** (the step is a PASSED Verus
  `spec_fn` — the verified engine, fully GROUNDED) and is UNAFFECTED. The two are
  the §4.2 dual: prove once in the parametric spec scheme, run via the monomorphized
  exec mirror (tied by `result == fold_list(l, …)`). Pinned as the REQ-3 exec form.

- **OQ-3 (which fusion laws the v0.1 corpus exercises):** REQ-8 pins three fusion
  laws; only `map_preserves_len_list` was GROUNDED end-to-end. `fold∘map` and
  `map∘map` fusion are pinned as the family but isolation-verified only (the
  analogue of `.design/lower/verus-lowering.md` OQ-3's not-corpus-exercised
  combinators). The orchestrator's call: which fusion law a v0.1 corpus program
  must EXERCISE (vs. carry isolation-verified for registry completeness). RECOMMEND
  `tree_fold.th` exercise `map_preserves_len` (grounded) + the discharged-once
  `fold_bound` instantiation (the multiplier); `fold∘map`/`map∘map` ship
  isolation-verified until a pipeline corpus program (Stage 5) exercises them. Not a
  blocker.
