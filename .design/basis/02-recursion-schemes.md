# Verified Recursion Schemes (Basis Stage 2)
<!--
tier: 3-component
status: draft
governs: thermite-syntax/src/ast.rs
governs: thermite-spec/src/validator.rs
governs: thermite-lower/src/lower.rs
governs: thermite-spec/src/schemes.rs
thesis-refs:
  - thermite-design.md §4.1
  - thermite-design.md §4.2
  - thermite-design.md §6
-->

## Summary

Stage 2 of the universal verified primitive basis (crosslink epic **#62**) adds
**verified recursion schemes** — `fold` (catamorphism), `map`, and the
structural predicates `for_all` / `exists` / `traverse` — over the recursive
ADTs Stage 1 pinned (`.design/basis/01-adts.md` REQ-3/REQ-10: `enum List { Nil,
Cons(u64, Box<List>) }`, `enum Tree { … }`). This is the **"prove once, compose
infinitely" engine**: a recursion scheme discharges its structural induction
ONCE — inside the scheme, via `decreases <value>` on the datatype itself (Stage 1
REQ-10: Verus's built-in structural order, no manual measure) — and thereafter
every catamorphism over the structure is verified by merely supplying the
**per-node step**. The induction is already paid for.

Each scheme has two surface forms (the §4.2 dual): a `spec fn` form for contracts
(total, terminating, the L1-fallback-bearing definition) and — where the scheme
collapses to a value an exec body needs — an exec form for `forge build`. The
spec form is the verified primitive; the exec form is its compiled mirror.

This doc is GREENFIELD / FORWARD-LOOKING. It builds DIRECTLY on Stage 1's
recursive-type representation (CONSUMED, not re-litigated): the `Box<T>`-on-`Alloc`
recursive enum, `decreases l` on the value, `*tail` dereference. **Every REQ below
is NOT-STARTED**, tracked under epic **#62** (#62 owns this stage — no separate
blocker is filed; gaps needing an independent blocker are noted with a fresh `#`).
The verified Verus forms below were produced by running the real `verus
0.2026.05.24` binary during authoring (Verification) — they are the lowering
contract, not guesses.

## The multiplier: induction-discharged-once (the load-bearing mechanism)

The thing this stage exists to deliver — pinned precisely, because it is the
whole point. A naive verified program proves a property over an unbounded
structure by writing a fresh `proof fn … decreases l` structural induction for
EACH property. That does not compose: N properties cost N inductions. A recursion
scheme inverts this. The scheme `fold` carries the recursion + `decreases`; a
**generic fold law** (`fold_bound` below) carries the induction ONCE, parametric
in the step `f` and a per-node premise. An instance then proves its goal by
**instantiating the law with its concrete step and discharging the (non-recursive)
per-node premise** — it writes NO `decreases`, does NO `match`, performs NO
recursive call. The structural induction is encapsulated.

GROUNDED. The generic law (verified `0 errors`):

```verus
spec fn fold(l: List, init: nat, f: spec_fn(u64, nat) -> nat) -> nat
    decreases l,          // <-- the structural induction lives HERE, once
{
    match l {
        List::Nil => init,
        List::Cons(x, tail) => f(x, fold(*tail, init, f)),
    }
}

proof fn fold_bound(l: List, init: nat, f: spec_fn(u64, nat) -> nat, b: nat)
    requires
        init == 0,
        forall|x: u64, acc: nat| #[trigger] f(x, acc) <= acc + b,   // PER-NODE premise
    ensures fold(l, init, f) <= len(l) * b,
    decreases l,          // <-- the induction, proven ONCE for ALL steps f
{ match l { List::Nil => {}
    List::Cons(x, tail) => {
        fold_bound(*tail, init, f, b);          // the single inductive call
        assert((len(*tail) + 1) * b == len(*tail) * b + b) by(nonlinear_arith);
    } } }
```

The INSTANCE (`sum_list`) proves its bound with NO induction — it instantiates:

```verus
spec fn sum_list(l: List) -> nat { fold(l, 0, |x: u64, acc: nat| add_step(x, acc)) }

proof fn sum_list_bounded(l: List)
    ensures sum_list(l) <= len(l) * (u64::MAX as nat),
{
    let f = |x: u64, acc: nat| add_step(x, acc);
    assert(forall|x: u64, acc: nat| #[trigger] f(x, acc) <= acc + (u64::MAX as nat));
    fold_bound(l, 0, f, u64::MAX as nat);   // <-- induction comes from the scheme
}
```

`sum_list_bounded` has no `decreases`, no `match`, no recursive call: the only
proof obligation it discharges is the FLAT per-node fact `f(x, acc) <= acc + MAX`.
That is the multiplier — finitely many verified schemes (`fold`/`map`/`for_all`/…)
× the per-node lemma of an instance = an unbounded provable slice of catamorphisms.
A NEGATIVE CONTROL confirms the induction is real, not vacuous: a `fold_bound`
that drops the per-node premise FAILS (`2 verified, 1 errors`); a `fold` with no
`decreases` is REJECTED by Verus. The induction does work; the premise is load-bearing.

## Requirements

### Surface + AST — scheme primitives (governs `thermite-syntax/src/ast.rs`, `parser.rs`)

- **REQ-1 (the scheme set as named primitives):** Thermite gains five recursion
  schemes over a recursive ADT, each a NAMED entity (per §4.2 "composition happens
  only through named `spec fn`s"): **`fold`** (catamorphism — collapse to a
  value), **`map`** (transform each element, same shape), **`for_all`** /
  **`exists`** (structural predicates over a structure's elements), and
  **`traverse`** (the `for_all`/`exists` generalization — a fold whose result is a
  `bool`). They are surfaced as a closed scheme family keyed on the recursive ADT
  + the step function, NOT as user-defined higher-order generics (the §4.4 closed
  set still binds — no user traits). The AST representation (a dedicated
  `Item::Scheme` registry vs. desugaring each scheme call to a generated named
  `spec fn` per (ADT, scheme) pair) is OQ-1. Derived from §4.2 (named composition),
  §4.4 (closed built-in set), and the GROUNDED `fold`/`map`/`for_all` Verus forms.

- **REQ-2 (the step function — flat per-node closure):** A scheme call supplies a
  **step**: a closure `|x, acc| …` (fold/traverse) or `|x| …` (map/for_all/exists)
  whose body is a FLAT predicate/expression (§4.2 closure-body rule: comparisons,
  arithmetic, field/index access, calls to named `spec fn`s — but NO nested
  combinator and NO nested scheme). The step reuses the existing `Expr::Closure`
  node (`thermite-syntax/src/ast.rs`, already lowered per
  `.design/lower/verus-lowering.md` REQ-3's `Closure` row to a Verus `spec_fn`).
  The scheme call itself is the named composition point; the step is the flat leaf.
  Derived from §4.2 (flat closure bodies), the existing `Expr::Closure`, and the
  GROUNDED `|x: u64, acc: nat| add_step(x, acc)` step.

- **REQ-3 (spec form + exec form — the §4.2 dual):** Each scheme has a `spec fn`
  form (the verified primitive — total, terminating via `decreases <value>`,
  carrying NO effect row, the L1-fallback-bearing contract definition) AND, where
  the scheme collapses a structure to a value used in an EXEC body, an exec form
  (`fx`-carrying: a `fold`/`map` that constructs a result over a heap-allocated
  `List` carries `fx alloc` per Stage 1 REQ-3, the constructing effect). The spec
  form is primary; the exec form is its compiled mirror, related by an `ensures`
  tying the exec result to the spec fold (`result == fold(l, …)`). OQ-2 records
  the least-confident scoping call: whether the exec form is a true higher-order
  exec `fold` (a closure passed at exec time) or whether exec folds are
  monomorphized per-step at lowering (the closure inlined), since Verus exec
  higher-order functions are heavier than spec `spec_fn`. Derived from §4.2
  ("Spec functions are executable" — the L1 rung), §4.1 (`fx` rows), Stage 1 REQ-3.

### Validator / the SpecTherm cage — the structural-quantification bridge (governs `thermite-spec/src/validator.rs`)

- **REQ-4 (the cage bridge — structural quantification via named schemes, never
  anonymous nested quantifiers):** A property that must quantify over EVERY element
  of a recursive structure ("every node of a `List`/`Tree` is `< CAP`") is the
  one place §4.2's "no anonymous nested quantifiers" cage would otherwise break —
  an inline `forall|node in tree|` is exactly the unbounded anonymous quantifier
  the cage forbids. The bridge: such quantification is expressed as a **named
  `for_all` / `exists` scheme call** carrying its own `decreases <value>` measure
  (§4.2 "Genuine nested quantification is written as a named `spec fn` … which may
  itself quantify, but carries its own `dec` measure"). The validator ACCEPTS a
  scheme call as a named-composition leaf (mirroring the combinator-call accept of
  `.design/spec/spectherm-combinators.md` REQ-6) and REJECTS a scheme call nested
  inside another scheme's step closure (the flat-closure rule, REQ-2) with a
  span-bearing `SpecError`. `fold`/`for_all`/`map`/`exists` ARE how the cage
  expresses unbounded-structure properties. Derived from §4.2 (the cage; named
  composition), Stage 1 REQ-7 (ADT predicates fit the cage), and
  `.design/spec/spectherm-combinators.md` REQ-6.

- **REQ-5 (scheme termination is structural — `decreases <value>`, validator
  enforces a `dec`):** A scheme `spec fn` over a recursive ADT carries `decreases
  l` on the datatype VALUE (Stage 1 REQ-10: Verus's built-in structural order, no
  manual measure), recursing through `Box` with `*tail`. The validator enforces
  the §4.2/§4.1 rule "no spec-level recursion without a `dec` measure" for every
  scheme exactly as for an ordinary `spec fn` — a scheme whose definition lacks the
  structural `dec` is a `SpecError` (the same diagnostic the slice-`spec_sum`
  already uses). GROUNDED: every scheme form (`fold`/`map`/`for_all`) verified with
  `decreases l`; a `fold` with NO `decreases` is REJECTED by Verus (negative
  control). Derived from §4.2 ("No spec-level recursion without a `dec` measure"),
  §4.1 (termination by default), Stage 1 REQ-10.

### Verus lowering — schemes + the discharged induction + fusion (governs `thermite-lower/src/lower.rs`)

- **REQ-6 (scheme → Verus recursive `spec fn` with `decreases <value>`):** Each
  scheme lowers to a Verus recursive `spec fn` carrying `decreases l` over the
  datatype value, matching on the ADT's variants, recursing through `*tail`, and
  applying the step `f` at each `Cons`/`Node`. The step closure lowers to a Verus
  `spec_fn` (the `Closure` row of `.design/lower/verus-lowering.md` REQ-3). For a
  predicate scheme (`for_all`/`exists`) the result type is `bool`; for `fold` it
  is the accumulator type (`nat`/`u64`); for `map` it is the same ADT.
  **GROUNDED** (`0 errors`): `fold`/`map`/`for_all` over `List` with `decreases l`,
  `*tail`, and `Box::new(map(*tail, g))` for the `map` reconstruction. Derived from
  §3 ("transpile to Verus"), Stage 1 REQ-10, the GROUNDED scheme forms.

- **REQ-7 (the induction-discharged-once contract shape — the multiplier
  lowering):** A scheme ships with a **generic structural law** (a `proof fn`
  parametric in the step `f` and a per-node premise, carrying the single
  `decreases l` induction) so that an INSTANCE proves its goal by instantiating the
  law + discharging a FLAT per-node premise — emitting NO fresh `decreases`/`match`/
  recursive call. The lowerer emits, per scheme, the generic law as a proof aid
  (the analogue of `.design/lower/verus-lowering.md` REQ-7's shape-keyed templates,
  NOT per-program hardcoding) and, at an instance site, the instantiating
  `proof { scheme_law(l, f, …); }` call plus the flat per-node `assert`.
  **GROUNDED**: `fold_bound` (the generic law, single induction) + `sum_list_bounded`
  (the instance, ZERO induction — only a flat `forall|x,acc| f(x,acc) <= acc + b`
  assert, then `fold_bound(l, 0, f, MAX)`) verified `0 errors`. The negative control
  — `fold_bound` minus the per-node premise — FAILS (`2 verified, 1 errors`),
  proving the premise is load-bearing and the induction non-vacuous. Derived from §6
  (L3 is a real SMT proof, R-DEFER-9 no vacuity), §4.2, the GROUNDED multiplier proof.

- **REQ-8 (fusion / composition laws — schemes compose):** The lowering pins the
  scheme algebra so a verified scheme composes with another into a SINGLE verified
  scheme rather than a re-proof: **(a)** `map` preserves length (`len(map(l, g))
  == len(l)`) — the structure-preservation law a downstream `fold` over a mapped
  list reuses; **(b)** `fold` after `map` fuses to a single fold
  (`fold(map(l, g), init, f) == fold(l, init, |x, acc| f(g(x), acc))`); **(c)**
  `map` of a composition is the composition of maps (`map(map(l, g), h) ==
  map(l, |x| h(g(x)))`). Each fusion law is a `proof fn … decreases l` proven ONCE;
  a pipeline reuses it instead of re-inducting. **GROUNDED**: `map_preserves_len`
  (`len(map(l, g)) == len(l)`) verified `0 errors` by structural recursion. Laws
  (b)/(c) are pinned as the fusion family (OQ-3 flags which the v0.1 corpus must
  exercise vs. carry isolation-verified). Derived from §4.2 (named composition),
  §6, the GROUNDED `map_preserves_len`.

- **REQ-9 (`LowerError`/`SpecError` extension, no panics):** The scheme constructs
  extend the EXISTING `thermite-lower::LowerError` (`.design/lower/verus-lowering.md`
  REQ-9) and `thermite-spec::SpecError` enums with span-bearing variants for the
  new failure modes (a scheme nested in a step closure — REQ-4; a scheme missing
  its structural `dec` — REQ-5; an un-lowerable scheme over a non-ADT value),
  reusing `thermite_syntax::lexer::Span`. No `unwrap`/`expect`/`panic!` in
  production (R-CODE-2 / R-APG-1). Derived from R-CODE-2, the existing error-enum
  discipline in `validator.rs` / `lower.rs`.

## Acceptance criteria

The orchestrator authors a NEW corpus program — call it `conformance/tree_fold.th`
(a `Tree` from Stage 1 + a `fold`-based property certified by INSTANTIATING the
verified scheme, NOT a fresh induction) — and EXTENDS Stage 1's
`conformance/list_sum.th` so its `sum_list` is recast as the `fold` INSTANCE
(`sum_list = fold(l, 0, +)` with `sum_list_bounded` via `fold_bound`). The
existing slice fold `conformance/sum.th` is noted as the SLICE-INSTANCE prototype
(the `Seq` fold `spec_sum` of `.design/lower/verus-lowering.md` REQ-5 is the
slice-shaped precursor of this ADT fold). Golden lowerings live at
`tests/golden/lower/tree_fold.verus.rs` / the extended `list_sum.verus.rs`,
hand-authored from this doc and confirmed to pass `verus`; certificate goldens at
`conformance/{tree_fold,list_sum}.cert.json`.

- **AC-1 (a fold scheme + its generic law parses, validates, lowers, certifies
  L3):** Parsing the `fold` scheme yields its named primitive (REQ-1); the
  validator accepts the scheme call as a named-composition leaf and enforces the
  structural `dec` (REQ-4, REQ-5); the lowerer emits a Verus `spec fn fold … 
  decreases l` plus the generic `fold_bound` law (REQ-6, REQ-7); running the real
  `verus` binary on the emitted output exits 0 with `N verified, 0 errors`. The
  GROUNDED `fold` + `fold_bound` (`8 verified, 0 errors`) is the verified seed.
  (REQ-1, REQ-3, REQ-5, REQ-6, REQ-7.)

- **AC-2 (induction-discharged-once — the instance proves with NO fresh
  induction):** `list_sum.th`'s `sum_list_bounded` certifies L3 by INSTANTIATING
  `fold_bound` + discharging the flat per-node premise — the emitted instance proof
  contains NO `decreases`, NO `match`, NO recursive `proof fn` call other than the
  single `fold_bound(l, 0, f, MAX)` instantiation. Mechanically: the emitted
  instance-proof body contains `fold_bound(` and does NOT contain a `decreases`
  clause (the multiplier is observable in the output). The NEGATIVE control — the
  generic law minus its per-node premise — FAILS `verus` (`2 verified, 1 errors`),
  pinned as a reject fixture proving non-vacuity (R-DEFER-9, §7). (REQ-7.)

- **AC-3 (the cage bridge — structural quantification is a NAMED scheme, not an
  anonymous quantifier):** A `tree_fold.th` property "every node `< CAP`" parses
  to a NAMED `for_all` scheme call (REQ-4), validates as a named-composition leaf,
  and lowers to a Verus recursive `spec fn for_all … decreases l`; `verus`
  certifies it. A crafted negative — a scheme call NESTED inside another scheme's
  step closure (the flat-closure violation, REQ-2/REQ-4) — REJECTS with the
  span-bearing `SpecError`. GROUNDED `for_all` over `List` (`0 errors`). (REQ-2,
  REQ-4, REQ-6, REQ-9.)

- **AC-4 (map + a fusion law certifies L3):** A `map` scheme parses (REQ-1),
  lowers to a Verus `spec fn map … decreases l` reconstructing via `Box::new`
  (REQ-6), and at least one fusion law (`len(map(l, g)) == len(l)`, REQ-8)
  certifies L3 by structural recursion proven ONCE — a downstream `fold` over the
  mapped list reuses it without re-inducting. GROUNDED `map` + `map_preserves_len`
  (`0 errors`). (REQ-1, REQ-6, REQ-8.)

- **AC-5 (the slice fold is the prototype / no regression):** `conformance/sum.th`
  is UNCHANGED — its `Seq` fold `spec_sum` (`.design/lower/verus-lowering.md`
  REQ-5) still lowers byte-stably and certifies L3; this stage recasts it as the
  SLICE INSTANCE of the fold family in the doc only, with no code reshape of the
  existing slice path. Stage 1's `list_sum.th` extension is purely additive (the
  `sum_list` recursive `spec fn` becomes a `fold` instance; the existing recursive
  form remains verifiable). Mechanically: `cargo test -p thermite-syntax -p
  thermite-spec -p thermite-lower` + the conformance corpus pass with 0 mismatches;
  `tests/golden/lower/sum.verus.rs` stays green. (All REQs; the engine must not
  break the kernel.)

- **AC-6 (reject + no-panic cases):** Crafted negatives reject with the right
  structured variant: a scheme nested in a step closure → the REQ-4/REQ-9 cage
  `SpecError`; a scheme `spec fn` lacking its structural `dec` → the REQ-5 `dec`
  `SpecError`; an un-lowerable scheme over a non-ADT value → `LowerError`. Lowering
  never panics; lowering the corpus returns `Ok`. Hand-derived expectations
  (R-CHAR-3), never read back from the toolchain's own output. (REQ-5, REQ-9.)

## Architecture

The component spans three crates, all additively, atop Stage 1's recursive ADTs:

- **`thermite-syntax`** — the scheme set (REQ-1) is either a dedicated
  `Item::Scheme` registry node or a desugaring of each scheme call into a generated
  named `spec fn` per (ADT, scheme) pair (OQ-1). The step closure (REQ-2) REUSES
  the existing `Expr::Closure` (`thermite-syntax/src/ast.rs`) — no new node for the
  step. A scheme CALL reuses `Expr::Call` (`callee: Path` naming the scheme, args =
  the structure + the step closure). The mandatory-contract discipline is
  unchanged: a scheme `spec fn` carries a `dec`, no `req`/`ens`/`fx` (it is spec);
  an exec scheme form (REQ-3) carries `fx` per Stage 1 REQ-3.

- **`thermite-spec`** — `validator.rs` gains the scheme-as-named-composition accept
  (REQ-4, mirroring the combinator-call accept of
  `.design/spec/spectherm-combinators.md` REQ-6 — a scheme call is a flat
  named-composition leaf), the nested-scheme-in-step rejection (REQ-2/REQ-4), and
  the structural-`dec` enforcement for a scheme definition (REQ-5). The caged-flat
  walk (that doc's REQ-6, Stage 1 REQ-7) is UNCHANGED: a scheme call joins
  combinator calls and named `spec fn` calls as a named-composition accept; a
  scheme nested in a closure body is the only NEW reject. New `SpecError` variants
  (REQ-9). A new `thermite-spec/src/schemes.rs` registry (the analogue of
  `combinators.rs` `static REGISTRY`) holds each scheme's frozen Verus form.

- **`thermite-lower`** — `lower.rs` gains `lower_scheme` (emit the recursive `spec
  fn … decreases l`, REQ-6), the generic-law emission (`scheme_law_for`, the
  shape-keyed proof-aid template of REQ-7 — analogue of
  `.design/lower/verus-lowering.md` REQ-7's `push_lemma_for`), the instance
  instantiation emission (the `proof { scheme_law(…); }` + flat per-node assert),
  and the fusion-law emission (REQ-8). The two lowering contexts (exec vs spec,
  `.design/lower/verus-lowering.md`) carry over: the spec scheme form is primary;
  an exec scheme form is its mirror (OQ-2). Symbol anchors: `enum Expr`
  (`Closure`/`Call`) in `ast.rs`; `pub fn validate` in `validator.rs`; `pub fn
  lower` / `lower_expr` in `lower.rs`; `static REGISTRY` in `combinators.rs` (the
  schemes registry mirrors it).

### The verified Verus forms (GROUNDED — the lowering contract, not guesses)

Produced by running the real `verus 0.2026.05.24` binary during authoring
(Verification). They are the seed for the golden files.

**The fold scheme + the discharged-once law + the instance (REQ-6, REQ-7).** See
the "multiplier" section above for the full `fold` / `fold_bound` /
`sum_list_bounded` triple — verified `8 verified, 0 errors`. The load-bearing
shapes: the scheme carries `decreases l`; the generic law carries the single
induction parametric in `f` + a per-node premise; the instance carries NEITHER a
`decreases` NOR a recursive call — only a flat per-node `assert` and the
instantiating `fold_bound(l, 0, f, MAX)`.

**The cage bridge — `for_all` as a named structural quantifier (REQ-4).**

```verus
spec fn for_all(l: List, p: spec_fn(u64) -> bool) -> bool
    decreases l,
{
    match l {
        List::Nil => true,
        List::Cons(x, tail) => p(x) && for_all(*tail, p),
    }
}
```

This is how "every node satisfies `p`" is written WITHOUT an anonymous nested
quantifier: a named `spec fn` carrying its own `decreases l` (§4.2). A contract
writes `for_all(l, |x: u64| x < CAP)` — the scheme is the named-composition leaf,
the closure `|x| x < CAP` is the flat per-node predicate (REQ-2).

**The map scheme + a fusion law (REQ-6, REQ-8).**

```verus
spec fn map(l: List, g: spec_fn(u64) -> u64) -> List
    decreases l,
{
    match l {
        List::Nil => List::Nil,
        List::Cons(x, tail) => List::Cons(g(x), Box::new(map(*tail, g))),
    }
}

proof fn map_preserves_len(l: List, g: spec_fn(u64) -> u64)
    ensures len(map(l, g)) == len(l),
    decreases l,
{
    match l {
        List::Nil => {}
        List::Cons(_, tail) => { map_preserves_len(*tail, g); }
    }
}
```

`map` reconstructs the structure with `Box::new` at each `Cons` (Stage 1's heap
primitive, REQ-3); `map_preserves_len` is the structure-preservation fusion law a
downstream `fold` over `map(l, g)` reuses (REQ-8) instead of re-inducting.

**RECORDED FINDING (the multiplier is real, not vacuous).** Two negative controls
were run and BOTH fail correctly: (1) `fold_bound` with the per-node premise
REMOVED fails its postcondition (`2 verified, 1 errors`) — the per-node premise is
load-bearing, the bound is not provable for an arbitrary step; (2) a `fold` with
NO `decreases` clause is REJECTED by Verus (`recursive function must have a
decreases clause`). This proves the structural induction inside the scheme does
real work and the instance genuinely depends on the per-node lemma — the
"discharged once" claim is grounded, not a vacuous restatement.

**RECORDED FINDING (the step is a spec `spec_fn`, not an exec closure — the
least-confident scope edge).** All GROUNDED forms pass the step as a Verus
`spec_fn(u64, nat) -> nat` in SPEC position. An EXEC higher-order `fold` (a closure
passed at run time, REQ-3 exec form) was NOT grounded — Verus exec higher-order
functions are heavier and the corpus's exec folds (the `sum.th` while-loop) are
written as monomorphic loops, not closure-passing. OQ-2 flags whether exec scheme
forms ship as true higher-order exec functions or as per-step monomorphized
lowerings (the closure inlined into a generated loop). The SPEC scheme primitive —
the verified contract surface, the engine — is fully grounded; the exec mirror's
higher-order shape is the open scoping call.

## Dependency hooks (for the rest of epic #62)

- **Stage 1 (consumed — recursive ADTs):** this stage builds DIRECTLY on Stage 1
  REQ-3/REQ-10. `decreases l` on the datatype value (Stage 1 REQ-10), `*tail`
  dereference, `Box::new` reconstruction (`map`, Stage 1 REQ-3's heap primitive),
  and REQ-7's "named `spec fn` composition" (Stage 1 REQ-7 — the cage accepts named
  recursion) are all consumed verbatim. Stage 1's `sum_list`/`len`
  (`.design/basis/01-adts.md` Architecture) ARE the fold-precursors; this stage
  recasts them as `fold` INSTANCES. Stage 2 cannot begin until Stage 1 lands the
  recursive-type + `decreases l` lowering (REQ-10).

- **Stage 4 (collections — fold over `Vec`):** a `fold`/`map` over a `Vec<T>` is
  the SAME scheme family generalized to the dynamic collection (Stage 4's heap
  generalization of Stage 1's `Box`/`alloc`). The `Seq` fold `spec_sum`
  (`.design/lower/verus-lowering.md` REQ-5) is the slice-shaped fold; a `Vec` fold
  is `vec@`-viewed to the same `Seq` fold. The scheme set + the discharged-once law
  shape (REQ-7) carry over unchanged — only the underlying structure changes.

- **Stage 5 (composition law — schemes compose):** REQ-8's fusion laws ARE the
  composition multiplier at the data-recursion level: `fold ∘ map` fuses, `map ∘
  map` fuses, so a verified pipeline of schemes is itself a verified scheme. The §9
  composition rule ("if `g` calls `f` only through `f`'s contract …") applies to a
  scheme instance through its generic law's `ensures` — a caller reasons about
  `fold(l, …)` through `fold_bound`'s contract, never by re-opening the recursion.

## Verification

- **Mandatory Verus grounding (DONE during authoring — real `verus
  0.2026.05.24`).** A single `verus!{}` file built on Stage 1's `enum List { Nil,
  Cons(u64, Box<List>) }` containing: `len` (the structural measure); the `fold`
  catamorphism with `decreases l`; the GENERIC `fold_bound` law (single induction,
  parametric in the step + a per-node premise); the `sum_list` fold INSTANCE +
  `sum_list_bounded` (proved by INSTANTIATING `fold_bound`, NO fresh induction);
  the `for_all` cage-bridge scheme; the `map` scheme + the `map_preserves_len`
  fusion law:

  ```
  verus --no-cheating /tmp/.../scheme.rs
  verification results:: 8 verified, 0 errors
  ```

  Cheat-token grep (`assume`/`external_body`/`admit`/`verifier::external`) over the
  file: NONE — run under `--no-cheating`. **Two negative controls confirm
  non-vacuity:** (1) `fold_bound` with the per-node premise removed FAILS
  (`2 verified, 1 errors` — the bound is unprovable for an arbitrary step); (2) a
  `fold` with no `decreases` is REJECTED by Verus. This proves the
  induction-discharged-once engine is Verus-feasible end to end AND that the
  discharge does real work — the foundation for Stage 4's collection folds and
  Stage 5's scheme composition.

- **AC-1/AC-2/AC-3/AC-4:** `cargo test -p thermite-syntax -p thermite-spec -p
  thermite-lower`, plus a harness that shells the real `verus` binary on the emitted
  lowering of `tree_fold.th` / the extended `list_sum.th` and asserts exit 0 +
  `N verified, 0 errors` (R-CODE-4: subprocess status checked, never swallowed),
  plus the AC-2 structural assertion (the emitted instance proof contains
  `fold_bound(` and NO `decreases`), plus `forge check` matching the golden
  certificates.
- **AC-2/AC-6 negatives:** the per-node-premise-removed fold law and the
  nested-scheme-in-closure / missing-`dec` validator rejects are reject fixtures
  with hand-derived expectations (R-CHAR-3).
- **AC-5:** the existing `tests/golden/lower/sum.verus.rs` and
  `conformance/sum.cert.json` assertions stay green (no regression on the slice
  fold).

Gauntlet (R-DEFER-6, per crate): `cargo test -p <crate>`, `cargo clippy -p <crate>
--all-targets -- -D warnings`, `cargo fmt --check`.

## Routes to add (orchestrator)

This stage adds NEW concerns to files that already carry routes, plus one new file
(`thermite-spec/src/schemes.rs`). The orchestrator adds these routes to
`tooling/spec-routes.toml` pointing at THIS doc (a file may carry multiple
governing docs — the `lower.rs` precedent):

```
[[route]]  crate_pattern = "thermite-syntax/src/ast.rs"        design = ".design/basis/02-recursion-schemes.md"   reference = ["conformance/list_sum.th", "conformance/tree_fold.th"]
[[route]]  crate_pattern = "thermite-syntax/src/parser.rs"     design = ".design/basis/02-recursion-schemes.md"   reference = ["conformance/list_sum.th", "conformance/tree_fold.th"]
[[route]]  crate_pattern = "thermite-spec/src/validator.rs"    design = ".design/basis/02-recursion-schemes.md"   reference = ["conformance/tree_fold.th"]
[[route]]  crate_pattern = "thermite-spec/src/schemes.rs"      design = ".design/basis/02-recursion-schemes.md"   reference = ["conformance/list_sum.th"]
[[route]]  crate_pattern = "thermite-lower/src/lower.rs"       design = ".design/basis/02-recursion-schemes.md"   reference = ["tests/golden/lower/list_sum.verus.rs", "tests/golden/lower/tree_fold.verus.rs"]
```

The corpus programs `conformance/tree_fold.th`, the extended
`conformance/list_sum.th`, their `.cert.json` goldens, and the
`tests/golden/lower/*.verus.rs` lowerings are authored by the orchestrator from
this doc before the builder runs (R-CHAR-3).

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (the scheme set as named primitives) | NOT-STARTED | epic **#62** Stage 2. No `fold`/`map`/`for_all`/`exists`/`traverse` scheme in `thermite-syntax/src/ast.rs` or a `thermite-spec/src/schemes.rs` registry; the surface admits no scheme today. GROUNDED-feasible (`fold`/`map`/`for_all` verus `0 errors`), not implemented. Depends on Stage 1 REQ-3/REQ-10 (recursive ADTs), itself NOT-STARTED. |
| REQ-2 (the step — flat per-node closure) | NOT-STARTED | epic **#62** Stage 2. `Expr::Closure` exists (slice-combinator closures) but no scheme-step validation; the flat-closure / no-nested-scheme rule is unimplemented. |
| REQ-3 (spec form + exec form) | NOT-STARTED | epic **#62** Stage 2. No scheme `spec fn` primitive and no exec mirror; the spec form is GROUNDED, the exec higher-order form is OQ-2 (least-confident, not grounded). |
| REQ-4 (cage bridge — named structural quantification) | NOT-STARTED | epic **#62** Stage 2. `validator.rs` has no scheme-as-named-composition accept nor the nested-scheme-in-step reject; depends on the caged-flat walk (`.design/spec/spectherm-combinators.md` REQ-6, blocker #40) + Stage 1 REQ-7, both NOT-STARTED. `for_all` cage form GROUNDED (`0 errors`). |
| REQ-5 (structural `decreases <value>` enforcement) | NOT-STARTED | epic **#62** Stage 2. `validator.rs` does not yet enforce a structural `dec` for a scheme definition. GROUNDED: every scheme verified with `decreases l`; a no-`decreases` fold is REJECTED by Verus (negative control). |
| REQ-6 (scheme → Verus recursive `spec fn` + `decreases <value>`) | NOT-STARTED | epic **#62** Stage 2. `lower.rs` has no `lower_scheme`. GROUNDED (`fold`/`map`/`for_all` over `List`, `decreases l`, `*tail`, `Box::new`, `0 errors`). |
| REQ-7 (induction-discharged-once contract shape — the multiplier) | NOT-STARTED | epic **#62** Stage 2. No generic-law (`scheme_law_for`) proof-aid emission and no instance-instantiation emission. GROUNDED: `fold_bound` (single induction) + `sum_list_bounded` (NO induction) `0 errors`; negative control (premise removed) FAILS `2 verified, 1 errors`. |
| REQ-8 (fusion / composition laws) | NOT-STARTED | epic **#62** Stage 2. No fusion-law emission in `lower.rs`. GROUNDED: `map_preserves_len` (`len(map(l,g)) == len(l)`) `0 errors`; `fold∘map` / `map∘map` laws pinned (OQ-3). |
| REQ-9 (`LowerError`/`SpecError` extension, no panics) | NOT-STARTED | epic **#62** Stage 2. The scheme reject/lower failure variants are not yet added to the existing error enums in `validator.rs`/`lower.rs`. |

## Open questions (for the orchestrator before the builder runs)

- **OQ-1 (scheme AST representation — `Item::Scheme` registry vs. per-(ADT,scheme)
  desugaring):** REQ-1 needs the scheme set representable. Two shapes: a dedicated
  `Item::Scheme`/registry node (`thermite-spec/src/schemes.rs` mirroring
  `combinators.rs` `static REGISTRY` — the scheme is a first-class named primitive),
  or desugar each scheme call into a generated named `spec fn` per (ADT, scheme)
  pair at lowering (less new surface, but the generated names leak into the audit
  surface). RECOMMEND the registry + a generated monomorphic `spec fn` per
  instance (the scheme is the abstraction; the generated `spec fn` is its lowering),
  so the validator can key the named-composition accept (REQ-4) on the scheme kind
  rather than a string-name match. Not a blocker; pinned for the builder.

- **OQ-2 (least-confident: exec higher-order folds vs. monomorphized exec
  lowering).** The SPEC scheme primitive — the verified engine — is fully GROUNDED
  (the step is a Verus `spec_fn`). The EXEC scheme form (REQ-3) was NOT grounded:
  Verus exec higher-order functions are heavier than spec `spec_fn`s, and the
  corpus's only exec fold (`sum.th`) is a monomorphic while-loop, not a
  closure-passing higher-order call. The open call: does an exec `fold`/`map` ship
  as a true higher-order exec function (a closure passed at run time), or is the
  step MONOMORPHIZED at lowering (the closure inlined into a generated loop, the
  `sum.th` shape)? RECOMMEND monomorphizing the exec form (inline the step,
  generate a loop — it matches the verified `sum.th` exec pattern and dodges Verus
  exec-closure weight), while keeping the SPEC scheme higher-order (`spec_fn`). This
  is the highest-judgment, least-confident part of the stage; the spec engine does
  not depend on it. Not a blocker for the corpus (the orchestrator's goldens pin
  the verified output).

- **OQ-3 (which fusion laws the v0.1 corpus exercises):** REQ-8 pins three fusion
  laws; only `map_preserves_len` was GROUNDED end-to-end. `fold∘map` and `map∘map`
  fusion are pinned as the family but isolation-verified only (the analogue of
  `.design/lower/verus-lowering.md` OQ-3's not-corpus-exercised combinators). The
  orchestrator's call: which fusion law a v0.1 corpus program must EXERCISE (vs.
  carry isolation-verified for registry completeness). RECOMMEND `tree_fold.th`
  exercise `map_preserves_len` (grounded) + the discharged-once `fold_bound`
  instantiation (the multiplier); `fold∘map`/`map∘map` ship isolation-verified
  until a pipeline corpus program (Stage 5) exercises them. Not a blocker.
