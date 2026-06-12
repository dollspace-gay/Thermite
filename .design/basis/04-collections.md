# Bounded Dynamic Collections — Vec / Map (Basis Stage 4)
<!--
tier: 3-component
status: draft
audited-sha: 64a329fac3ece936c17be3d6aadf5bc125b20682 (re-pinned: the #269/#270 coordinated arc — the touched-file changes are exactly this doc's designed REQs / reviewed as claim-neutral)  (re-audited 2026-06-12: amended — Map half (REQ-2/REQ-6) SHIPPED via #123/C12 as the Vec-of-pairs TMap (see 13-map.md), not the vstd::map sketch; OQ-2/OQ-3 resolved, #262)
governs: thermite-syntax/src/ast.rs
governs: thermite-syntax/src/parser.rs
governs: thermite-spec/src/validator.rs
governs: thermite-lower/src/lower.rs
thesis-refs:
  - thermite-design.md §4.2
  - thermite-design.md §4.4
  - thermite-design.md §6
-->

## Summary

Stage 4 of the universal verified primitive basis (crosslink epic **#62**) adds
the **practical data-structure stdlib**: a **bounded `Vec<T>`** (a growable
sequence with a `len() <= CAP` capacity invariant + verified `push`/`pop`/`get`/
`len`) and a **bounded `Map<K,V>`** (`insert`/`get`/`contains`/`len` with a
key-uniqueness invariant). Both are the **`Box`/`Alloc` heap primitive of Stage 1
generalized**: a `Vec`-building `fn` carries `fx alloc`. The unlock beyond the
existing read-only `&[T]` algorithms (`sum`/`binary_search`) is *growth* — a
capacity-preserving `push` and a no-OOB `get` — plus **collections of verified
data**: a `Vec<Account>` where every element satisfies a Stage-1 `well_formed`
invariant, carried as a `forall|i| inv(v@[i])` predicate through a named
`spec fn` (the §4.2 cage bridge).

This doc is GREENFIELD / FORWARD-LOOKING. Thermite v0.1 today admits exactly
`u32`/`u64`/`usize`, `bool`, and the read-only slice `&[T]` (`thermite-syntax/
src/ast.rs` `enum Type` = `Prim`/`Unit`/`Ref`/`Slice`/`Generic`); there is no
`Vec`, `Map`, or growth. **UPDATE (#73): the `Vec` half is SHIPPED** — REQ-1/REQ-3/
REQ-5/REQ-7 landed (the bounded `Vec<u64>` over `vstd::vec::Vec`, capacity
invariant + no-OOB `get` + capacity-preserving `push`, real verus L3).
**UPDATE (#123, cluster C12): the `Map` half (REQ-2/REQ-6) is ALSO SHIPPED** — as
the dedicated two-arg `Type::Map(Box<Type>, Box<Type>)` node + a `TMap`
Vec-of-pairs wrapper (`insert`/`get`/`contains_key`/`len`), governed in detail by
the dedicated `.design/basis/13-map.md`; the shipped backing DIVERGES from this
doc's `vstd::map::Map` sketch (see the REQ-6 row). The `Vec` element invariant
(REQ-4) remains deferred to a Stage-4 v1.1 follow-up under epic **#62** (no
separate blocker is filed — #62
owns this stage; a gap needing an independent blocker is noted with a fresh `#`). The Stage-1 `Box`/`Alloc` keystone
(`.design/basis/01-adts.md` REQ-3/REQ-10) is the load-bearing prerequisite: a
growable `Vec` is the same `Alloc`-effect heap as `Box<T>`, generalized from a
single boxed cell to a contiguous run. Stage 4 cannot land before Stage 1's
recursive-type + `alloc` lowering does (R-DEFER-7).

## Decision: wrap vstd `Vec`/`Map` with a bounded newtype + capacity invariant

The cage prefers **bounded** structures — a `Vec` with `len() <= CAP` keeps the
solver decidable and matches the existing `xs.len() <= 1_000_000` corpus idiom
(`conformance/sum.th` `req xs.len() <= 1_000_000`). Three representations were
considered for the verified `Vec`:

- **(a) wrap vstd `Vec`** — a Thermite `Vec<T>` lowers to a newtype over Verus's
  `vstd::vec::Vec<T>`, with the capacity bound carried as a Thermite-level
  `well_formed` predicate (`self.data.len() <= CAP`) threaded through contracts.
  vstd already gives verified `push`/`index`/`len` with `Seq` views (`v@`).
- **(b) a from-scratch custom bounded Vec** — re-derive a length-tracked backing
  store with hand-written push/get proofs.
- **(c) a fixed-capacity inline array** — `[T; CAP]` with a length field; no heap,
  no `alloc`.

**DECIDED: option (a), wrap vstd `Vec`.** It is the only option that (1) Verus
accepts end-to-end *today* (GROUNDED below — `BVec` over `Vec<u64>` with a
`well_formed`/`push`/`get` + a preserved element invariant verified `5 verified,
0 errors`), (2) inherits vstd's verified `push`/`index`/`len` rather than
re-proving heap mechanics, and (3) reuses the Stage-1 `Alloc` heap story directly
(vstd `Vec` allocates → the constructing `fn` carries `fx alloc`, REQ-5).
Option (b) re-proves what vstd already verifies (wasted proof surface, no
expressiveness gain). Option (c) caps the structure below "growable" — it cannot
`push` past `CAP` slots fixed at compile time, failing the practical-stdlib claim;
and it removes the `alloc` effect that is the point of generalizing Stage 1's
`Box`. The capacity bound `CAP` is the SAME constant idiom as the corpus
(`1_000_000`); a `Vec` is bounded by design so the §4.2 cage never sees an
unbounded sequence.

**RESOLVED (#62 design-refinement, OQ-1): v1 WRAPS `vstd::vec::Vec`** —
proven-for-free (vstd's `push`/`index`/`len` carry the heap proof; the GROUNDED
`BVec` over `Vec<u64>` is exactly this form, `5 verified, 0 errors`), and `vstd` is
VERSION-PINNED alongside Verus, so the coupling is to a PINNED dep (low-risk, not
an unpinned moving target). The decisive REQUIREMENT this resolution pins
(REQ-5): the **Thermite-surface `Vec` contract — `push`/`pop`/`get`/`len` plus the
capacity invariant (`len() <= CAP`) and the element invariant — is specified
BACKING-AGNOSTIC**, independently of vstd. The surface contract names what each
operation guarantees (`push`: `req len < CAP, ens len' == len+1 && v@[old_len] ==
x`; `get`: `req i < len, ens result == v@[i]`) WITHOUT referencing
`vstd::vec::Vec` in the contract itself; vstd is the v1 IMPLEMENTATION behind that
contract, not the contract. **Migration path:** a later decouple to a custom
backing store (a Thermite-owned `Seq`-backed run) swaps the IMPLEMENTATION — the
lowering target changes from `self.data: vstd::vec::Vec<T>` to the custom store —
WITHOUT changing the surface contract or any user `.th` code, exactly because the
contract is backing-agnostic (the §6/§9 "the contract is the interface" property).
OQ-1 below records the certificate/golden-stability consequence; OQ-3 the Map
first-cut depth.

## Requirements

### Surface + AST (governs `thermite-syntax/src/ast.rs`, `parser.rs`)

- **REQ-1 (`Vec<T>` type + operation surface):** The surface admits a `Vec<T>`
  type and its bounded operations `push`/`pop`/`get`/`len`. The AST `enum Type`
  gains the `Vec` element-type indirection — either a dedicated `Type::Vec(Box<
  Type>)` or reuse of the existing `Generic { name: "Vec", arg }` machinery (the
  same OQ-1 shape `.design/basis/01-adts.md` raised for `Box`). `Vec` is the
  growth generalization of the existing `Type::Slice(Box<Type>)` (`ast.rs`): a
  `&[T]` is a read-only borrowed view, a `Vec<T>` owns a growable backing run
  whose borrowed slice is `&v[..]` and whose `Seq` view is `v@`. Operations are
  ordinary calls (`v.push(x)`, `v.get(i)`, `v.len()`) reusing `Expr::Call` /
  `Expr::Method` (no new expression node). Derived from §4.4 (one call syntax,
  closed built-in interface set — `Vec` is a built-in, not a user type) and the
  existing `Type::Slice`/`Type::Generic` machinery.

- **REQ-2 (`Map<K,V>` type + operation surface):** The surface admits a
  `Map<K,V>` type and `insert`/`get`/`contains`/`len`. The AST `enum Type` gains a
  two-argument generic for `Map` (the existing `Generic { name, arg: Box<Type> }`
  is single-arg — `Map` needs a key AND a value type, so either a dedicated
  `Type::Map { key, value }` or a generalized multi-arg generic, OQ-2). `Map` may
  be a thinner first cut than `Vec` (see OQ-3): `contains`/`get`/`insert` with a
  key-uniqueness invariant, modeled on `vstd::map::Map`. Derived from §4.4 (closed
  built-in interfaces — `Map` is built in) and the vstd `Map` model.

### Validator / the SpecTherm cage (governs `thermite-spec/src/validator.rs`)

- **REQ-3 (capacity + operation contracts fit the §4.2 cage):** The bounded
  collection contracts are written with FLAT, named predicates, never anonymous
  nested quantifiers. The capacity bound (`v.len() <= CAP`) is a flat comparison;
  the operation contracts (`push` `req len < CAP, ens len' == len+1 && v@[old_len]
  == x`; `get` `req i < len, ens result == v@[i]`; `pop`; `len`) are flat
  built-ins admitted inside a combinator's predicate-closure body. The §4.2 cage
  is preserved exactly: a property quantifying over a collection's elements is the
  EXISTING bounded combinator `forall_in(v, |x| …)` (the `&[T]` form of
  `conformance/binary_search.th` `forall_in(haystack, |x| x != needle)`, now
  reading `v@` instead of a slice), whose closure body is flat; a deeper property
  is a NAMED `spec fn`. The validator's caged-flat walk
  (`.design/spec/spectherm-combinators.md` REQ-6) is UNCHANGED — `v@`-indexing and
  `v.len()` are flat built-ins, `forall_in` over a `Vec` is the same frozen-trigger
  combinator as over a slice. Derived from §4.2 (the cage — every quantifier is a
  bounded combinator with a frozen trigger; composition through named `spec fn`s),
  `.design/spec/spectherm-combinators.md` REQ-6, and the GROUNDED `all_elems_inv`
  named-`spec fn` element invariant.

- **REQ-4 (element invariant — `forall|i| inv(v@[i])` via a named `spec fn`):** A
  `Vec<T>` may carry an element invariant: every element satisfies a predicate
  (e.g. a `Vec<Account>` where each element satisfies Stage-1 `Account::
  well_formed`, `.design/basis/01-adts.md` REQ-8). This is expressed as a NAMED
  `spec fn all_elems_inv(v) -> bool` whose body is the bounded combinator
  `forall_in(v, |e| inv(e))` (or, at the Verus layer, `forall|i| 0 <= i < v.len()
  ==> inv(v@[i])` with a frozen trigger) — never an anonymous nested quantifier.
  The validator accepts the element invariant exactly as it accepts any named
  `spec fn` (`.design/spec/spectherm-combinators.md` REQ-3 name collection). The
  invariant must be PRESERVED by `push` (the contract conjunct `all_elems_inv(v')`
  given `all_elems_inv(v) && inv(x)`). Derived from §4.2 (named-`spec fn`
  composition), the decided scope (verified collections of verified data), and the
  GROUNDED `push_preserving` proof (element invariant preserved across `push`).

### Verus lowering (governs `thermite-lower/src/lower.rs`)

- **REQ-5 (`Vec<T>` → vstd `Vec` wrapper; `push`/`get`/`len` → verified vstd ops;
  the `alloc` effect):** A Thermite `Vec<T>` lowers to a newtype over `vstd::vec::
  Vec<T>` (or vstd `Vec` directly) with the capacity bound as a `pub open spec fn
  well_formed(&self) -> bool { self.data.len() <= CAP }` threaded through
  `requires`/`ensures` (the SAME data-invariant-threading mechanism as Stage-1
  `Account::well_formed`, `.design/basis/01-adts.md` REQ-8 / OQ-3). `push` lowers
  to `self.data.push(x)` with `req old(self).well_formed() && old(self).data.len()
  < CAP, ens final(self).well_formed() && final(self).data.len() == old.len()+1 &&
  final(self).data@[old_len] == x` plus the element-preservation frame; `get`
  lowers to `self.data[i]` with `req i < self.data.len(), ens result ==
  self.data@[i as int]` (the verified no-OOB index); `len` to `self.data.len()`.
  A `fn` CONSTRUCTING / `push`-ing a `Vec` allocates, so it carries `fx alloc`
  (`Effect::Alloc`, `thermite-syntax/src/ast.rs` `enum Effect` `Alloc`, already
  present) — the SAME effect-row rule and effect-subsumption acceptance as Stage-1
  `Box` construction (`.design/basis/01-adts.md` REQ-3). **GROUNDED**: a `BVec`
  newtype over `Vec<u64>`, verified `well_formed`/`push`/`get`/`accumulate`,
  `5 verified, 0 errors`; the no-OOB `get` and capacity-preserving `push` proven,
  the broken forms (push without the cap guard, get without the bound) FAIL.
  Derived from §3 (transpile to Verus), §4.1 (the `alloc` effect; row
  subsumption), §6 (L3), and the GROUNDED `BVec` proof. **BACKING-AGNOSTIC SURFACE CONTRACT
  (#62 resolution, REQUIRED).** The Thermite-surface `Vec` contract
  (`push`/`pop`/`get`/`len` + the capacity and element invariants) is specified
  INDEPENDENTLY of vstd — the contract names the operation guarantees over the
  `Seq` view `v@`, never `vstd::vec::Vec` itself. v1 IMPLEMENTS that contract by
  wrapping `vstd::vec::Vec` (proven-for-free; vstd is version-pinned alongside
  Verus, so the coupling is to a pinned dep). Because the contract is
  backing-agnostic, a later decouple to a custom Thermite-owned backing store swaps
  the lowering target (`self.data: vstd::vec::Vec<T>` → the custom store) WITHOUT
  changing the surface contract or any user `.th` code (§6/§9 "the contract is the
  interface"). The golden lowering is pinned to a recorded `verus`/vstd version
  (OQ-1).

- **REQ-6 (`Map<K,V>` → vstd `Map` wrapper; `insert`/`get`/`contains` → verified
  ops; key-uniqueness invariant):** A Thermite `Map<K,V>` lowers to a wrapper over
  `vstd::map::Map<K,V>` with `insert` (`ens get(k) after insert(k,v) == v`),
  `get`, `contains`, `len`, and a key-uniqueness invariant (a Verus `Map` has at
  most one value per key by construction — the invariant is the model's, surfaced
  as a `well_formed` if a bound on `len()` is also carried). A `Map`-mutating `fn`
  carries `fx alloc` (REQ-5's rule). This may be a THINNER first cut than `Vec`
  (OQ-3): the minimum is `insert`/`get`/`contains` with the post-insert-get
  contract; `pop`/iteration may follow. Derived from §3, §4.1 (`alloc`), the vstd
  `Map` model, and the decided scope.

- **REQ-7 (`LowerError`/`SpecError` extension, no panics):** The new collection
  constructs extend the EXISTING `thermite-lower::LowerError` and `thermite-spec::
  SpecError` enums with span-bearing variants for the new failure modes (a `get`
  whose index bound cannot be discharged is a Verus proof failure surfaced through
  the ladder, not a lowerer panic; an un-lowerable collection construct is a
  `LowerError` variant), reusing `thermite_syntax::lexer::Span`. No `unwrap`/
  `expect`/`panic!` in production (R-CODE-2 / R-APG-1). Derived from R-CODE-2 and
  the existing error-enum discipline in `validator.rs` / `lower.rs`.

### Cluster C6 (#98) — Vec completeness: the missing ops + non-Copy elements + the reachability fix

Cluster C6 (crosslink **#98**) closes the probe-confirmed gaps that left `Vec` an
incomplete primitive: the missing operations (`pop`/`insert`/`remove`/`contains`),
`Vec<T>` for **NON-COPY** element types (`Vec<String>`, `Vec<struct>`, nested
`Vec<Vec<_>>`), and the `Vec::new()`-no-param wrapper-emission bug. All forms below
were GROUNDED end-to-end with the real `verus 0.2026.05.24` binary during authoring
(Verification — the C6 grounding record), non-vacuous, no cheat tokens.

- **REQ-8 (the missing ops — `pop_last`/`last`/`insert`/`remove`/`contains`, all
  TUPLE-FREE):** The surface tuple-less today (tuples land in C9), so the ops are
  pinned to tuple-free shapes:
  - **`pop_last(self)`** — drop the LAST element. `req len > 0`, `ens len' == len - 1
    && (forall j in [0,len') => v'@[j] == v@[j])` (the kept prefix is preserved). It
    does NOT return the popped value (no tuple); the companion accessor returns it.
  - **`last(self) -> T`** — the final element accessor. `req len > 0`, `ens result ==
    v@[len - 1]`. (The tuple-free split of a classic `pop -> (Vec, Option<T>)`:
    `last()` reads, `pop_last()` shortens.)
  - **`insert(self, i, x)`** — splice `x` at index `i`, shifting the suffix right.
    `req well_formed() && len < CAP && i <= len`, `ens len' == len + 1 && v'@ ==
    v@.insert(i, x)`. The `i <= len` bound is the no-OOB safety (NOT `i < len` — an
    insert AT `len` is an append).
  - **`remove(self, i)`** — delete the element at `i`, shifting the suffix left.
    `req i < len`, `ens len' == len - 1 && v'@ == v@.remove(i)`.
  - **`contains(self, x) -> bool`** — an EXEC linear scan over the `Vec` (element
    equality `==` on the element type). `req well_formed()`, `ens result == (exists|k|
    0 <= k < len && v@[k] == x)`. The loop carries the standard `forall|k| 0 <= k < i
    ==> v@[k] != x` invariant + `decreases len - i`.

  `pop_last`/`insert`/`remove` are `&mut self`-mutating, so their `ens` is written
  with **`final(self)`** (the Stage-4 / REQ-5 `&mut` grounding finding — verus
  0.2026.05.24 requires `final(self)`, not bare `self`); `last`/`contains` are
  `&self`-reading (no `final`). All lower to vstd's verified `Vec::pop`/`Vec::insert`/
  `Vec::remove`/`Vec::index` (which carry the heap + shift proof). `insert`/`remove`/
  `pop_last` allocate/mutate-in-place → the constructing fn carries `fx alloc` (the
  REQ-5 rule); `last`/`contains` over a `&Vec` are `pure`. **GROUNDED** (`vec_ops`
  probe): `pop_last`/`last`/`insert`/`remove`/`contains` over `Vec<u64>` all verify
  together — **`9 verified, 0 errors`**, no cheat tokens; the broken `insert` dropping
  the `i <= len` guard FAILS (**`8 verified, 1 errors`**, vstd `insert` precondition
  — the no-OOB bound is load-bearing, non-vacuous, R-DEFER-9). Derived from §4.2 (the
  cage — every op bounded), §4.4 (closed built-in interface), §6 (L3), and the C6
  GROUNDED `vec_ops` proof.

- **REQ-9 (`Vec<T>` for NON-COPY element types — `Vec<String>`, `Vec<struct>`,
  nested `Vec<Vec<_>>` — via BORROW-returning `get`):** THE HARD GAP. The Stage-4
  finding (recorded in REQ-5 / the `tvec_name` doc) was that a GENERIC `TVec<T>`
  failed verus because vstd's index `self.data[i]` MOVES a non-`Copy` `T` out of the
  backing `Vec` (`E0507: cannot move out of index`), which forced the u64-only
  monomorphization (`TVecU64`). For non-`Copy` elements (`Vec<String>` /
  `Vec<struct>` / nested) the resolution is: the monomorphized wrapper's exec `get`
  returns a **BORROW** `&T`, not a moved `T` — `pub fn get(&self, i: usize) ->
  (result: &T) requires i < self.data.len(), ensures *result == self.data@[i as int]
  { &self.data[i] }`. The `&self.data[i]` reads through the index WITHOUT moving;
  the `ens` dereferences (`*result == v@[i]`). `push(x: T)` CONSUMES (moves) the owned
  element in — no `Copy` needed for push. The per-element-type monomorphization
  (REQ-5 `tvec_name`) EXTENDS to non-`Copy` elements: `Vec<String>` → `TVecTString`,
  `Vec<UserStruct>` → `TVec<UserStruct>` (the struct name suffix), nested
  `Vec<Vec<u64>>` → `TVecTVecU64`. A Copy element (`u64`) MAY keep the by-value `get
  -> T` (the existing GROUNDED `TVecU64` form, byte-stable for `vec_demo.th`); a
  non-Copy element MUST use the borrow `get -> &T`. **GROUNDED**: `Vec<String>`
  (`TVecTString` over `vstd::vec::Vec<TString>`, push a `String`, `get` it back by
  borrow, read its `len`) verifies **`4 verified, 0 errors`**, no cheat tokens — and
  the by-value-move form of the SAME probe FAILS with **`E0507: cannot move out of
  index of std::vec::Vec<TString>`** (the exact Stage-4 finding, proving the borrow
  is the fix, not a convenience). `Vec<struct>` (`TVecPoint` over a 2-field
  `Point { x, y }`, push + borrow-`get` + field read) verifies **`4 verified, 0
  errors`**. Nested `Vec<Vec<u64>>` (`TVecTVecU64`, the element `TVecU64` itself
  non-Copy) ALSO verifies **`4 verified, 0 errors`** with the same borrow-`get`.
  Derived from §3 (transpile to Verus), §4.4 (the closed built-in over any element
  type), §6 (L3), the Stage-4 non-Copy-move finding, and the C6 GROUNDED
  `vec_string`/`vec_struct`/`vec_nested` proofs.

- **REQ-10 (the element type's own wrapper MUST be woven — the #68/#86 weave):**
  A non-Copy element wrapper references the element's own decl/wrapper: `TVecTString`
  names `TString` (the Stage-7 string wrapper), `TVec<UserStruct>` names the user
  `struct UserStruct` decl, `TVecTVecU64` names the inner `TVecU64` wrapper. Verus
  needs each in scope BEFORE the outer wrapper, exactly as the #68 ADT-decl weave
  (`forge::reachable_adt_deps` recursing through `Type::Vec(inner)` to reach the
  element `struct`) and the #86 String-reachability weave (`program_uses_string`
  recursing through `Type::Vec(inner)` to reach `String`). For `Vec<struct>` the
  forge per-item sub-program weave is ALREADY correct — `collect_type_adt_refs`
  (`forge/src/check.rs`) recurses `Type::Vec(inner) => collect_type_adt_refs(inner)`,
  so a `Vec<Account>` already weaves the `Account` decl (a CONSUMED capability, no
  change). For `Vec<String>` the `TString` wrapper is emitted whenever
  `program_uses_string` holds, which ALREADY recurses through `Type::Vec(inner)` to
  reach `String` (`ty_reaches_string`, `lower.rs`) — a CONSUMED capability. The
  builder's residual work is emission ORDER: `emit_vec_wrappers` must emit the
  element's wrapper (or the struct decl be in scope) BEFORE the `TVec<elem>` newtype.
  GROUNDED feasible (the nested + struct + string probes verify with the element
  wrapper/decl declared first). Derived from §4.2 (named composition), the #68 ADT
  weave (`.design/basis/01-adts.md`), and the #86 String-reachability weave
  (`.design/basis/07-strings.md` REQ-4 / `program_uses_string`).

- **REQ-11 (the `Vec::new()`-no-param wrapper-reachability fix — the #86 analog):**
  A `Vec` built LOCALLY with `Vec::new()` and used only inside a fn body — with NO
  `Vec`-typed parameter or return — fails `E0425 cannot find type TVecU64` today,
  because `emit_vec_wrappers` collects element types via `collect_vec_elem_types`
  which walks ONLY `fn`/`spec fn` PARAMETER and RETURN positions (`lower.rs`). It
  misses a body-local `let mut v: Vec<u64> = Vec::new();` (and a `Vec<u64>` struct
  FIELD, an enum-variant payload), so the `TVecU64` wrapper is never emitted yet the
  fn body references it. The fix is the EXACT #86 String-reachability pattern: the
  `Vec`-wrapper emission must trigger when a `Vec<T>` is REACHABLE ANYWHERE — a
  param/return (current), a struct/enum FIELD, OR a fn-body local `let` annotation —
  mirroring `program_uses_string` (`lower.rs`), which walks param/return + struct
  field + enum variant + local `let` + literal for `String`. `collect_vec_elem_types`
  EXTENDS to the same reachability closure (struct fields, enum-variant payloads,
  body-local `let` type annotations), keyed on `Type::Vec(inner)` exactly as
  `ty_reaches_string` keys on `Type::String`. GROUNDED: the verus form of a local
  `Vec::new()` fn (build locally, push, borrow-`get`, read len) verifies — the gap
  is purely the wrapper-emission reachability, NOT the verification (the same
  `Vec::new()` local appears verified inside every GROUNDED C6 probe's `build_*`
  body). Derived from the #86 String-reachability weave
  (`.design/basis/07-strings.md` REQ-4 / `program_uses_string`) and §4.4.

- **REQ-12 (`pop`/`insert`/`remove`/`contains` in `BUILTIN_METHODS`; non-Copy
  `tvec_name` extension; no panics):** The new EXEC ops `pop_last`/`insert`/`remove`
  are EXEC-only (never in a contract — they mutate). `last`/`contains` MAY be named
  in a contract (`ens result == v.last()` / a `contains` predicate), so `last` and
  `contains` (alongside the existing `get`/`len`) are admitted in `BUILTIN_METHODS`
  (`thermite-spec/src/validator.rs`) so their `ens` validates inside the §4.2 cage.
  `tvec_name` (`lower.rs`) EXTENDS its `match` from Copy primitives to also accept a
  `Type::String` element (→ `TVecTString`), a `Type::Named(struct)` element
  (→ `TVec<StructName>`), and a `Type::Vec(inner)` nested element (→ recursive
  suffix), each emitting the borrow-`get` form (REQ-9). A still-unlowerable element
  is the existing `LowerError::Unsupported` (no new variant, no `unwrap`/`expect`/
  `panic!`, R-CODE-2 / R-APG-1). Derived from R-CODE-2, the Stage-4 `BUILTIN_METHODS`
  precedent, and the existing error-enum discipline.

## Acceptance criteria

The orchestrator authors a NEW corpus program — call it `conformance/vec_accum.th`
(a `Vec<u64>` accumulator with a `len() <= CAP` capacity bound, a verified `push`
and a no-OOB `get`/sum) — and a NEW element-invariant program `conformance/
vec_accounts.th` (a `Vec<Account>` reusing the Stage-1 `struct Account` whose
`all_elems_inv` is preserved across `push`). Their golden lowerings live at
`tests/golden/lower/vec_accum.verus.rs` / `tests/golden/lower/vec_accounts.verus.rs`,
hand-authored from this doc and confirmed to pass `verus` (the GROUNDED `BVec`
form below is the verified seed). The certificate goldens live at `conformance/
vec_accum.cert.json` / `conformance/vec_accounts.cert.json`.

- **AC-1 (bounded `Vec` accumulator parses, validates, lowers, certifies L3):**
  Parsing `vec_accum.th` yields a `Vec<u64>`-typed value; the validator accepts
  the capacity-bound + operation contracts in the §4.2 cage (REQ-3); the lowerer
  emits the vstd-`Vec` wrapper + `well_formed` predicate + `push`/`get`/`len`
  (REQ-5); the constructing `fn` carries `fx alloc` and passes effect-subsumption;
  running the real `verus` binary on the emitted output exits 0 with `N verified,
  0 errors` — the no-OOB `get` and the capacity-preserving `push` proven; the
  emitted certificate matches `vec_accum.cert.json` (L3, non-vacuous). A crafted
  negative — a `push` without the `len < CAP` guard, or a `get` without the
  `i < len` bound — FAILS to verify (R-DEFER-9 non-vacuity; GROUNDED: `3 verified,
  2 errors`). (REQ-1, REQ-3, REQ-5, REQ-7.)

- **AC-2 (`Vec<Account>` element invariant preserved by `push`, certifies L3):**
  Parsing `vec_accounts.th` yields a `Vec<Account>` whose element invariant is the
  named `spec fn all_elems_inv(v)` (each element satisfies Stage-1
  `Account::well_formed`); the validator accepts it as named-`spec fn` composition
  (REQ-4); the lowerer threads `all_elems_inv(v)` through `push`'s `requires`/
  `ensures`; `verus` proves `all_elems_inv(v')` from `all_elems_inv(v) &&
  Account::well_formed(x)` (`N verified, 0 errors`). A crafted `push` of an element
  violating `well_formed` (without the `elem_inv(x)` precondition) FAILS — the
  invariant is real, not vacuous. (REQ-2 not required; REQ-1, REQ-4, REQ-5.)

- **AC-3 (`Map<K,V>` insert/get round-trip certifies L3):** Parsing a `Map<K,V>`
  program yields a `Map`-typed value; the lowerer emits the vstd-`Map` wrapper;
  `verus` certifies the post-insert-get contract (`get(k)` after `insert(k,v) ==
  v`) and the key-uniqueness invariant (`N verified, 0 errors`). If the Map first
  cut is thinner (OQ-3), this AC pins exactly `insert`/`get`/`contains`. (REQ-2,
  REQ-6.)

- **AC-4 (the slice↔Vec relation; existing corpus unchanged — no regression):**
  The read-only `&[T]` algorithms `conformance/sum.th` and
  `conformance/binary_search.th` are UNCHANGED — they still parse to the same AST,
  validate clean, lower to the byte-stable `tests/golden/lower/{sum,binary_search}.
  verus.rs`, and certify L3. A `Vec`'s backing slice is `&v[..]` and its `Seq` view
  is `v@` — the SAME `Seq` the slice `forall_in`/`spec_sum` already quantify over;
  the collection additions are purely additive (new `Type` variant(s), the `Vec`/
  `Map` lowering paths). Mechanically: `cargo test -p thermite-syntax -p
  thermite-spec -p thermite-lower` and the conformance corpus pass with 0
  mismatches. (All REQs; Stage 4 must not break the kernel.)

### Cluster C6 acceptance criteria (#98 — Vec completeness, GROUNDED)

The orchestrator authors NEW corpus programs from the C6 GROUNDED forms below: a
`Vec<u64>` ops program (`pop_last`/`last`/`insert`/`remove`/`contains`), a
`Vec<String>` program, a `Vec<struct>` program, and a local-`Vec::new()` program.

- **AC-5 (the missing ops certify L3; the OOB negative certifies L0):** A `Vec<u64>`
  program exercising `pop_last`/`last`/`insert`/`remove`/`contains` parses, validates
  (`last`/`contains` accepted in `BUILTIN_METHODS`, REQ-12), lowers (the ops emitted
  on `TVecU64`, REQ-8), and the real `verus` binary on the emitted output exits 0
  with `N verified, 0 errors` (GROUNDED `9 verified, 0 errors`). A crafted `insert`
  WITHOUT the `i <= len` guard FAILS to verify (GROUNDED `8 verified, 1 errors`, the
  L0 demonstration; R-DEFER-9 non-vacuity). The mutating ops carry `fx alloc`; the
  reading ops are `pure`. (REQ-8, REQ-12.)

- **AC-6 (`Vec<String>` builds/indexes via borrow-`get`, certifies L3):** A fn
  building a `Vec<String>` (push a `String`, `get` it back, read its `len`) parses,
  lowers to `TVecTString` over `vstd::vec::Vec<TString>` with the BORROW-returning
  `get -> &TString` (REQ-9), the `TString` element wrapper woven before it (REQ-10),
  and `verus` exits 0 (`N verified, 0 errors`; GROUNDED `4 verified, 0 errors`). The
  by-value-move form of `get` FAILS (`E0507: cannot move out of index`), proving the
  borrow is the load-bearing fix. (This unblocks cluster 5 `split` returning
  `Vec<String>`.) (REQ-9, REQ-10, REQ-12.)

- **AC-7 (`Vec<struct>` push/borrow-get certifies L3):** A fn building a
  `Vec<a-2-field-struct>` (push a struct, borrow-`get`, read a field) lowers to
  `TVec<Struct>` with the borrow-`get` (REQ-9) and the struct decl woven before it
  (REQ-10, the #68 `collect_type_adt_refs` recursion through `Type::Vec`, CONSUMED),
  and `verus` exits 0 (GROUNDED `4 verified, 0 errors`). (REQ-9, REQ-10.)

- **AC-8 (a local `Vec::new()` with NO Vec param certifies L3 — the reachability
  fix):** A fn whose ONLY `Vec` is a body-local `let mut v: Vec<u64> = Vec::new();`
  (no `Vec`-typed param/return) parses, the wrapper-reachability fix emits `TVecU64`
  (REQ-11 — `collect_vec_elem_types` extended to body-local `let`s, the #86 analog),
  and `verus` exits 0 — NOT `E0425 cannot find type TVecU64`. (REQ-11.)

## Architecture

The component spans three crates, all additively:

- **`thermite-syntax`** — `enum Type` (`thermite-syntax/src/ast.rs`) gains the
  `Vec` element-type indirection (REQ-1, OQ-1) and the `Map` key/value
  indirection (REQ-2, OQ-2), generalizing the existing single-arg `Generic { name,
  arg: Box<Type> }` and the read-only `Slice(Box<Type>)`. `Vec`/`Map` operations
  are ordinary calls — no new `Expr` node (`Expr::Call`/method form). `parser.rs`
  parses the `Vec<T>` / `Map<K,V>` type spellings; the mandatory-contract
  discipline of `Contract` (`.design/syntax/ast.md` REQ-2) is unchanged.

- **`thermite-spec`** — `validator.rs` (`pub fn validate`) accepts the
  capacity/operation contracts as FLAT built-ins (REQ-3) and the element invariant
  as a named `spec fn` (REQ-4). The caged-flat walk
  (`.design/spec/spectherm-combinators.md` REQ-6) is UNCHANGED: `v@`-indexing,
  `v.len()`, and `forall_in(v, …)` over a `Vec` are the same flat-built-in /
  frozen-trigger-combinator forms as over a `&[T]`. ADT element invariants compose
  through named `spec fn`s, never anonymous nested quantifiers — the §4.2 cage is
  preserved.

- **`thermite-lower`** — `lower.rs` (`pub fn lower` / `lower_expr`) gains the
  `Vec`/`Map` lowering paths (REQ-5/REQ-6): the vstd-`Vec`/`Map` wrapper, the
  `well_formed` capacity predicate (REQ-5), and the operation contracts. The
  data-invariant-threading mechanism is the SAME as Stage-1 `Account::well_formed`
  (`.design/basis/01-adts.md` REQ-8 / OQ-3 — automatic threading vs. authored).
  The two lowering contexts (exec vs. spec, `.design/lower/verus-lowering.md`)
  extend: `v.push(x)` is exec position (carries `fx alloc`); `v@[i]` / `v.len()` /
  `forall_in(v, …)` are spec position over the `Seq` view. Symbol anchors:
  `enum Type` in `ast.rs`; `enum Effect` `Alloc` in `ast.rs`; `pub fn validate` in
  `validator.rs`; `pub fn lower` / `lower_expr` in `lower.rs`.

### The verified Verus form (GROUNDED — the lowering contract, not guesses)

Produced by the real `verus 0.2026.05.24` binary during authoring (Verification).
This is the seed for the `vec_accum.th` / `vec_accounts.th` golden lowerings.

```verus
pub spec const CAP: usize = 1_000_000;

pub struct BVec { pub data: Vec<u64> }

impl BVec {
    pub open spec fn well_formed(&self) -> bool { self.data.len() <= CAP }
    pub open spec fn len(&self) -> nat { self.data.len() as nat }

    pub fn get(&self, i: usize) -> (result: u64)
        requires i < self.data.len(),                       // the no-OOB bound
        ensures result == self.data@[i as int],             // result == v@[i]
    { self.data[i] }

    pub fn push(&mut self, x: u64)
        requires old(self).well_formed(), old(self).data.len() < CAP,  // cap guard
        ensures
            final(self).well_formed(),                       // capacity preserved
            final(self).data.len() == old(self).data.len() + 1,
            final(self).data@[old(self).data.len() as int] == x,
            forall|j: int| 0 <= j < old(self).data.len()     // element frame
                ==> final(self).data@[j] == old(self).data@[j],
    { self.data.push(x) }
}

pub open spec fn elem_inv(x: u64) -> bool { x <= CAP as u64 }
pub open spec fn all_elems_inv(v: Seq<u64>) -> bool {
    forall|i: int| 0 <= i < v.len() ==> elem_inv(#[trigger] v[i])
}

pub fn push_preserving(bv: &mut BVec, x: u64)             // element invariant preserved
    requires old(bv).well_formed(), old(bv).data.len() < CAP,
             all_elems_inv(old(bv).data@), elem_inv(x),
    ensures  final(bv).well_formed(), all_elems_inv(final(bv).data@),
             final(bv).data.len() == old(bv).data.len() + 1,
{ bv.data.push(x); /* assert all_elems_inv by the element frame */ }
```

**RECORDED FINDING (the bounded-collection stack is end-to-end feasible).** The
`well_formed` capacity invariant (`len() <= CAP`), the no-OOB `get` (`req i < len`),
the capacity-preserving `push` (`req len < CAP`), and the element invariant
(`all_elems_inv`, a named `spec fn` over `forall|i|`, PRESERVED across `push` via
the element frame) all verify together — `5 verified, 0 errors`. Cheat-token grep
(`assume`/`external_body`/`admit`/`verifier::external`): NONE. Non-vacuity
confirmed by a companion run dropping the `req len < CAP` from `push` and the
`req i < len` from `get`: the unguarded `push` FAILS its `well_formed`
postcondition and the unbounded `get` FAILS its index precondition (`3 verified,
2 errors`). **Migration note:** this `verus` version (0.2026.05.24) requires
`final(self)` (not bare `self`) to disambiguate a `&mut` parameter in a
postcondition — the lowerer must emit `final(...)` for `&mut`-mutating
collection-operation `ensures`. The verified `BVec` over vstd `Vec<u64>` is the
exact wrap-vstd form REQ-5 lowers to: vstd's verified `Vec::push`/`Vec::index`/
`Vec::len` carry the heap proof; the capacity bound and element invariant are the
Thermite-level additions threaded through contracts.

### The C6 grounding record (GROUNDED — real `verus 0.2026.05.24`, the #98 seed)

The four C6 forms below were GROUNDED with the real `verus 0.2026.05.24` binary
during authoring (`verus --no-cheating`), non-vacuous, cheat-token grep
(`assume`/`external_body`/`admit`/`verifier::external`) NONE. Scratch cleaned (§53).

- **The missing ops over `Vec<u64>` (REQ-8) — `9 verified, 0 errors`.** A `TVecU64`
  with `well_formed`/`len`/`spec_get`/`get`/`push` PLUS the tuple-free
  `pop_last` (`&mut`, `req len > 0`, `ens final(self).data.len() == old.len()-1` +
  the kept-prefix frame), `last` (`&self`, `req len > 0`, `ens result ==
  v@[len-1]`), `insert` (`&mut`, `req well_formed && len < CAP && i <= len`, `ens
  final(self).data@ == old.data@.insert(i, x)`), `remove` (`&mut`, `req i < len`,
  `ens final(self).data@ == old.data@.remove(i)`), and `contains` (`&self`, the
  exec linear scan, `ens result == exists|k| 0<=k<len && v@[k]==x`) all verify
  together. The `&mut` ops use `final(self)` (the REQ-5 finding). NON-VACUITY: the
  same file with the `i <= len` guard dropped from `insert` FAILS — `8 verified, 1
  errors` (vstd `insert` precondition; the no-OOB bound is load-bearing, R-DEFER-9).

- **`Vec<String>` non-Copy via borrow-`get` (REQ-9) — `4 verified, 0 errors`.** A
  `TVecTString { data: Vec<TString> }` whose exec `get` returns a BORROW:
  `pub fn get(&self, i: usize) -> (result: &TString) requires i < self.data.len(),
  ensures *result == self.data@[i as int] { &self.data[i] }`, `push(x: TString)`
  consuming the owned element, and a `build_and_read` fn (build a local
  `Vec::new()`, push a `String`, `get` it back by borrow, read its `len`). THE
  ACCESS FORM THAT WORKED: `&self.data[i]` (a borrow), with the `ens` dereferencing
  `*result == v@[i]`. THE FAILURE THE BORROW SOLVES: the same probe with `get`
  returning `TString` by value (`{ self.data[i] }`) FAILS — `error[E0507]: cannot
  move out of index of std::vec::Vec<TString>` (the exact Stage-4 non-Copy finding).

- **`Vec<struct>` non-Copy (REQ-9) — `4 verified, 0 errors`.** A `TVecPoint { data:
  Vec<Point> }` over `struct Point { x: u64, y: u64 }` (a 2-field non-Copy struct),
  the SAME borrow-`get -> &Point`, push by move, and a fn pushing a `Point`,
  borrow-`get`-ing it, reading `e.x`. The struct decl is in scope before the
  `TVecPoint` wrapper (REQ-10 weave).

- **Nested `Vec<Vec<u64>>` (REQ-9, feasible NOT deferred) — `4 verified, 0 errors`.**
  A `TVecTVecU64 { data: Vec<TVecU64> }` whose element `TVecU64` is itself non-Copy,
  the SAME borrow-`get -> &TVecU64`. The inner `TVecU64` wrapper is declared before
  the outer (REQ-10). So nested Vecs are GROUNDED-feasible by the same borrow rule +
  per-element-type monomorphization; they are NOT deferred.

- **The local `Vec::new()`-no-param case (REQ-11).** Every C6 probe's `build_*` body
  contains `let mut v: TVec*  = TVec* { data: Vec::new() };` — the local `Vec::new()`
  verifies in verus. The bug is purely Thermite-side wrapper-emission reachability
  (`collect_vec_elem_types` not walking body-local `let`s), NOT verification.

**Migration note (C6):** the per-element-type monomorphization (REQ-5 `tvec_name`)
EXTENDS to non-Copy elements with a BORROW-returning `get -> &T` (Copy elements may
keep the by-value `get -> T`, the byte-stable `vec_demo.th` form). The element type's
own wrapper/decl must be in scope before the `TVec<elem>` newtype (REQ-10, the
#68/#86 weave). The `final(self)` `&mut` rule (REQ-5) carries to `pop_last`/`insert`/
`remove`.

## Dependency hooks (for the rest of epic #62)

- **Stage 1 (ADTs — `Box`/`alloc`, type invariants — CONSUMED):** Stage 4 is the
  generalization of Stage 1's `Box<T>` heap primitive (`.design/basis/01-adts.md`
  REQ-3/REQ-10) from a single boxed cell to a growable run. The `fx alloc`
  effect-row rule (a constructing `fn` carries `fx alloc`) and the
  effect-subsumption acceptance of `alloc` are REUSED VERBATIM (REQ-5). The
  element invariant (REQ-4) reuses the Stage-1 `well_formed` type-invariant
  mechanism (`.design/basis/01-adts.md` REQ-8) — a `Vec<Account>` element
  satisfies the SAME `Account::well_formed` predicate, now under a `forall|i|`. The
  data-invariant-threading question (`.design/basis/01-adts.md` OQ-3) is shared:
  the capacity `well_formed` is threaded exactly as the struct invariant is.

- **Stage 2 (recursion schemes — fold/map — CONSUMES this):** a `Vec` is
  foldable. The Stage-2 scheme set (`fold`/`map` over a structure;
  `.design/basis/02-recursion-schemes.md`, being authored in parallel — reference
  by name, do not re-derive) instantiates over a `Vec`'s `Seq` view `v@` exactly as
  over a `Box`-recursive list: a `Vec` fold is a `spec fn` with `decreases v.len()`
  (or a `Seq`-indexed loop with a `forall_in` element invariant, the
  `conformance/sum.th` `spec_sum` form generalized from a slice to `v@`). The
  no-anonymous-quantifier composition (REQ-3/REQ-4) is exactly how a Vec fold
  quantifies over elements inside the cage.

- **Stage 5 (composition law — CONSUMES this):** reasons over the collection
  contracts pinned here — the capacity invariant (`well_formed`, REQ-5) and the
  element invariant (`all_elems_inv`, REQ-4) are the contract surface a
  composition law quantifies the collection-half over. The §9 composition rule
  ("if `g` calls `f` only through `f`'s contract …") applies to `Vec`/`Map`-valued
  contracts unchanged: a `fn` returning a `well_formed` `Vec` with an element
  invariant exposes exactly that contract to its caller.

## Verification

- **Mandatory Verus grounding (DONE during authoring — real `verus
  0.2026.05.24`).** A single `verus!{}` file containing the bounded `BVec` over
  vstd `Vec<u64>` (`well_formed`/`len`/`get`/`push` with the capacity invariant,
  no-OOB index, capacity-preserving push + element frame), the element invariant
  (`elem_inv`/`all_elems_inv` named `spec fn`s + `push_preserving` proving the
  invariant preserved across `push`), and an `accumulate` exercising
  push-then-no-OOB-get verified:

  ```
  verus --no-cheating /tmp/coll_ground.rs
  verification results:: 5 verified, 0 errors
  ```

  Cheat-token grep (`assume`/`external_body`/`admit`/`verifier::external`) over
  the file: NONE. Non-vacuity confirmed by a companion run dropping the cap guard
  from `push` and the bound from `get`: both correctly FAIL (`3 verified, 2
  errors` — the unguarded `push` violates `well_formed`, the unbounded `get`
  violates its index precondition). This proves the bounded-`Vec` + capacity-
  invariant + no-OOB-get + element-invariant stack is Verus-feasible end to end —
  the practical-stdlib foundation Stages 2 and 5 build on. (Scratch cleaned per
  §53 — no stray `*.rlib`/`*.d` left.)

- **AC-1/AC-2/AC-3:** `cargo test -p thermite-syntax -p thermite-spec -p
  thermite-lower`, plus a harness that shells the real `verus` binary on the
  emitted lowering of `vec_accum.th` / `vec_accounts.th` / the Map program and
  asserts exit 0 + `N verified, 0 errors` (R-CODE-4: subprocess status checked,
  never swallowed), plus `forge check` matching the golden certificates
  (`conformance/{vec_accum,vec_accounts}.cert.json`). The non-vacuity negatives
  (unguarded `push`, unbounded `get`, `push` of a `well_formed`-violating element)
  must FAIL to verify (R-DEFER-9).
- **AC-4:** the existing `tests/golden/lower/{sum,binary_search}.verus.rs` and
  `conformance/sum.cert.json` assertions stay green (no regression); the slice↔Vec
  relation (`&v[..]`, `v@`) is documented, not a new node reshape.

Gauntlet (R-DEFER-6, per crate): `cargo test -p <crate>`, `cargo clippy -p
<crate> --all-targets -- -D warnings`, `cargo fmt --check`.

## Routes to add (orchestrator)

This stage adds NEW concerns to files that already carry routes; the orchestrator
adds these routes to `tooling/spec-routes.toml` pointing at THIS doc (a file may
carry multiple governing docs — the `lower.rs` precedent):

```
[[route]]  crate_pattern = "thermite-syntax/src/ast.rs"        design = ".design/basis/04-collections.md"   reference = ["conformance/vec_accum.th", "conformance/vec_accounts.th"]
[[route]]  crate_pattern = "thermite-syntax/src/parser.rs"     design = ".design/basis/04-collections.md"   reference = ["conformance/vec_accum.th", "conformance/vec_accounts.th"]
[[route]]  crate_pattern = "thermite-spec/src/validator.rs"    design = ".design/basis/04-collections.md"   reference = ["conformance/vec_accum.th"]
[[route]]  crate_pattern = "thermite-lower/src/lower.rs"       design = ".design/basis/04-collections.md"   reference = ["tests/golden/lower/vec_accum.verus.rs", "tests/golden/lower/vec_accounts.verus.rs"]
```

The corpus programs `conformance/vec_accum.th`, `conformance/vec_accounts.th`,
their `.cert.json` goldens, and the `tests/golden/lower/*.verus.rs` lowerings are
authored by the orchestrator from this doc (and the GROUNDED `BVec` seed) before
the builder runs (R-CHAR-3).

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (`Vec<T>` type + push/pop/get/len surface) | SHIPPED | #73. `Type::Vec(Box<Type>)` in `thermite-syntax/src/ast.rs` (dedicated node mirroring `Type::Box`, OQ-2 RESOLVED); parsed by `parser::parse_type` on the contextual `Vec` ident; `push`/`pop`/`get`/`len` reuse `Expr::MethodCall` (no new node). Consumer: `thermite_lower::lower`. Verified: `v: Vec<u64>` parses + lowers + verus-verifies (`thermite-lower/tests/collections_conformance.rs`, the `conformance/vec_demo.th` oracle). |
| REQ-2 (`Map<K,V>` type + insert/get/contains/len surface) | SHIPPED | #123 (cluster C12 — detailed in `.design/basis/13-map.md`). `Type::Map(Box<Type>, Box<Type>)` in `thermite-syntax/src/ast.rs` — a dedicated two-arg node (OQ-2 RESOLVED as recommended), parsed by `parse_type`'s `"Map"` contextual-ident arm in `parser.rs` (`Map<u64, u64>` → `Type::Map(..)`); `insert`/`get`/`contains_key`/`len` reuse `Expr::MethodCall` (no new node; the membership predicate shipped spelled `contains_key`, not this doc's `contains`; `get` returns the C7 `Option<V>`). Consumer: `thermite_lower::lower::lower_type` (→ `tmap_name`). Verified: `forge/tests/map_conformance.rs` (real verus L3 on `conformance/map_kv.th`). |
| REQ-3 (capacity + operation contracts fit the §4.2 cage) | SHIPPED | #73. The bounded-`Vec` contracts are FLAT built-ins: `v.len()` (already admitted) + the no-OOB accessor `get` ADDED to `BUILTIN_METHODS` (`thermite-spec/src/validator.rs`) so `ens result == v.get(i)` validates inside the cage; `v.len() < CAP` / `result.len() == v.len() + 1` are flat `len` comparisons. The caged-flat walk (`walk_expr_inner`'s `MethodCall` arm) is UNCHANGED. `push`/`pop` are EXEC-only (never in a contract). Consumer: `validate`. Verified: `collections_conformance.rs` (contracts validate clean + real verus L3). |
| REQ-4 (element invariant via named `spec fn` `forall|i| inv(v@[i])`) | NOT-STARTED | epic **#62** Stage 4 (v1.1). The named-`spec fn` accept path it reuses is SHIPPED, but the v1 corpus (`conformance/vec_demo.th`) exercises only the capacity contract + no-OOB get — no `Vec<Account>` element-invariant program is in the corpus. The GROUNDED `all_elems_inv` form (preserved across `push`, `0 errors`) is design-confirmed feasible; deferred to a Stage-4 follow-up. |
| REQ-5 (`Vec` → vstd `Vec` wrapper; push/get/len; `fx alloc`; BACKING-AGNOSTIC surface) | SHIPPED | #73 (OQ-1 RESOLVED: v1 WRAPS `vstd::vec::Vec`). `lower.rs`: `Type::Vec(elem)` → `tvec_name` (`Vec<u64>` → `TVecU64`); `emit_vec_wrappers` materializes ONCE per element type the GROUNDED `TVec<elem>` newtype over `vstd::vec::Vec<elem>` with `well_formed` (`len() <= CAP`), spec `len`/`spec_get`, the no-OOB exec `get` (`req i < len`), and the capacity-preserving exec `push` (`req well_formed && len < CAP`, `ens final(self)...` — the `final(self)` &mut grounding finding). Spec-position `v.get(i)` → `v.spec_get(i as int)`. `fx alloc` accepted by effect-subsumption (`push` is an intrinsic, no callee row). Consumer: `lower`. Verified: real `verus --no-cheating` on emitted `vec_demo.th` — `checked_get` L3/pure, `push_one` L3/alloc (`4 verified, 0 errors`); the no-`req` `get` reject FAILS (L0, R-DEFER-9). BACKING-AGNOSTIC surface preserved (the contract names `len`/`get`/`push` over `v@`, never `vstd::vec::Vec`). |
| REQ-6 (`Map` lowering; insert/get/contains_key; key-uniqueness) | SHIPPED | #123 (cluster C12 — detailed in `.design/basis/13-map.md`). DIVERGENCE from this doc's sketch: the shipped backing is NOT `vstd::map::Map` — `emit_map_wrappers`/`tmap_name` in `thermite-lower/src/lower.rs` materialize, once per `(K, V)` pair, a `TMap` newtype over a `vstd::vec::Vec<(K, V)>` Vec-of-pairs backing, with key-uniqueness + capacity carried in `well_formed` and the spec view `spec_dom`/`spec_contains_key`; `lower_type` maps `Type::Map(k, v)` → `tmap_name(k, v)`; `contains_key` is admitted in `BUILTIN_METHODS` (`thermite-spec/src/validator.rs`) so `ens result == m.contains_key(k)` validates in the §4.2 cage; `insert` stays EXEC-only (`&mut`, `final(self)`); a `Map`-mutating `fn` carries `fx alloc` (this doc's REQ-5 rule, as designed). The first cut is exactly OQ-3's recommended thin one (`insert`/`get`/`contains_key`/`len`; no `remove`/iteration). Verified: `forge/tests/map_conformance.rs` (real `verus --no-cheating` — insert-then-get round-trip + absent→`None`; the `Some(0)`-for-absent negative FAILS, non-vacuous R-DEFER-9). |
| REQ-7 (`LowerError`/`SpecError` extension, no panics) | SHIPPED | #73. The `Vec` lowering reuses the existing `LowerError::Unsupported` (`tvec_name` on a non-primitive element type) — no new variant needed; the validator reuses its existing reject path (a forbidden method in a contract). No `unwrap`/`expect`/`panic!` added (R-CODE-2 / R-APG-1); verified by `cargo clippy --workspace -D warnings` + the anti-pattern-gate. |
| REQ-8 (`pop_last`/`last`/`insert`/`remove`/`contains` — tuple-free missing ops) | SHIPPED | #98. `emit_one_vec_wrapper` (`thermite-lower/src/lower.rs`, per element type) emits all five ops on `TVec<elem>`: `pop_last`/`insert`/`remove` are `&mut` with `final(self)`, `last` is `&self`-reading, `contains` is the exec linear scan (the `forall|k| 0<=k<i ==> v@[k]!=x` invariant + `decreases len-i`). `insert` carries the load-bearing `i <= len` no-OOB guard. Spec-position `v.last()` → `v.spec_get((v.len()-1) as int)` (`lower_expr`). Consumer: `lower`. Verified: `forge/tests/vec_completeness_conformance.rs::vec_u64_ops_certify_l3` — real `verus --no-cheating` `9 verified, 0 errors`; the unguarded `insert` FAILS `8 verified, 1 errors` (non-vacuity, R-DEFER-9). |
| REQ-9 (`Vec<T>` non-Copy elements — `Vec<String>`/`Vec<struct>`/nested via borrow-`get`) | SHIPPED | #98. `tvec_name` `match` EXTENDS to a `String` element (→ `TVecTString`), a `Named` struct/enum element (→ `TVec<Name>`), and a nested `Vec(inner)` element (→ recursive `tvec_name(inner)` suffix, `Vec<Vec<u64>>` → `TVecTVecU64`). `elem_is_copy` selects the accessor: Copy → by-value `get -> T`/`last -> T` + `contains`; NON-Copy → BORROW `get -> &T`/`last -> &T` (`&self.data[i]`, `ens *result == v@[i]`) — vstd's index MOVES a non-Copy element out (`E0507`), so the borrow is the load-bearing fix; `push(x: T)` consumes the owned element. Consumer: `lower`. Verified: `vec_completeness_conformance.rs` — `Vec<String>` `17 verified, 0 errors` (make-or-break), `Vec<struct>` `7 verified`, nested `Vec<Vec<u64>>` `15 verified`, all `0 errors`; the by-value form FAILS `E0507`. |
| REQ-10 (element-type wrapper/decl woven before the `TVec` — #68/#86 pattern) | SHIPPED | #98. `collect_vec_elem_types`'s `note_vec_elems` notes a nested `Vec` element INNER-FIRST, so `emit_vec_wrappers` emits `TVecU64` before `TVecTVecU64` (the two-wrapper order). For a `String`/struct element the wrapper/decl is woven via the CONSUMED `program_uses_string`/`forge::collect_type_adt_refs` (both recurse `Type::Vec`) — verus resolves references within the `verus!` block order-independently (the 17/0 + 7/0 verifies confirm; literal source order is not load-bearing within the block). Consumer: `lower`. Verified: `vec_completeness_conformance.rs` (element wrapper/decl present + whole program L3). |
| REQ-11 (`Vec::new()`-no-param wrapper-reachability fix — the #86 analog) | SHIPPED | #98. `collect_vec_elem_types` EXTENDS its reachability closure from fn/spec-fn param+return to ALSO walk `struct`/`enum`-variant FIELD types and `fn`-body local `let` annotations (`note_block_vec_elems`/`note_stmt_vec_elems`, keyed on `Type::Vec(inner)`, the #86 analog). `lower_stmt`'s `Stmt::Let` rewrites a `Vec`-typed `Vec::new()` init (`is_vec_new`) to `<TVec> { data: Vec::new() }` (a bare `Vec::new()` cannot inhabit the newtype, `E0308`); L1 mirrors with `<TVec>::new()`. Consumer: `lower`/`lower_l1`. Verified: `vec_completeness_conformance.rs::local_vec_new_no_param_certifies_l3` (a body-local-only `Vec::new()` certifies L3 — NOT `E0425`). |
| REQ-12 (`last`/`contains` in `BUILTIN_METHODS`; non-Copy `tvec_name` extension; no panics) | SHIPPED | #98. `last`/`contains` ADDED to `BUILTIN_METHODS` (`thermite-spec/src/validator.rs`) so an `ens result == v.last()`/`v.contains(x)` validates in the §4.2 cage; `pop_last`/`insert`/`remove` stay EXEC-only. `tvec_name` extends to `String`/`Named`/nested `Vec` elements (the borrow-`get` form, REQ-9). A still-unlowerable element is the existing `LowerError::Unsupported` — no new variant, no `unwrap`/`expect`/`panic!` (R-CODE-2 / R-APG-1). Consumer: `validate`/`lower`. Verified: `vec_completeness_conformance.rs` + `cargo test -p thermite-spec`. |

## Open questions (for the orchestrator before the builder runs)

- **OQ-1 (wrap vstd `Vec` vs. a custom bounded backing — RESOLVED; #62
  design-refinement).** *(This is the OQ the #62 pass refers to for the Vec
  backing; in this doc it is OQ-1.)* **RESOLVED: v1 WRAPS `vstd::vec::Vec`** behind
  a thin Thermite-owned newtype (`struct Vec<T> { data: vstd::vec::Vec<T> }`, the
  GROUNDED `BVec` shape, `5 verified, 0 errors`) — proven-for-free, and `vstd` is
  version-pinned alongside Verus so the coupling is to a PINNED dep (low-risk). The
  capacity invariant + the `fx alloc` boundary stay Thermite's own. The decisive
  REQUIREMENT (REQ-5): the surface contract is **BACKING-AGNOSTIC** — specified
  independently of vstd — so the residual certificate/golden-stability concern (the
  golden lowering references vstd's `Vec` API, which can shift across Verus
  versions — cf. the `final(self)` migration note this version forced) is handled
  by pinning the golden lowering to a RECORDED `verus`/vstd version, and the
  MIGRATION PATH is clean: swapping to a custom Thermite-owned backing store later
  changes only the lowering target, never the surface contract or user `.th` code.
  The GROUNDED proof uses `vstd::vec::Vec` directly; a fully vstd-decoupled custom
  backing store is the designed-but-unproven future swap that the backing-agnostic
  contract makes safe. Pinned for the builder.

- **OQ-2 (`Vec`/`Map` as dedicated `Type` nodes vs. generalized `Generic`):** the
  existing `Generic { name: Ident, arg: Box<Type> }` (`ast.rs`) is single-arg —
  fine for `Vec<T>`, insufficient for `Map<K,V>` (two type args). Two shapes: a
  dedicated `Type::Vec(Box<Type>)` + `Type::Map { key, value }` (clearest — keys
  the lowerer/effect check on node kind), or a generalized multi-arg `Generic {
  name, args: Vec<Type> }`. The same OQ-1 shape `.design/basis/01-adts.md` raised
  for `Box`. RECOMMEND dedicated nodes so the `fx alloc` / capacity-invariant
  emission keys on the node kind, not a string-name match. Not a blocker.
  **RESOLVED as recommended:** dedicated nodes shipped — `Type::Vec(Box<Type>)`
  (#73) and the two-arg `Type::Map(Box<Type>, Box<Type>)` (#123, `ast.rs`).

- **OQ-3 (Map first-cut depth):** REQ-6 may ship a THINNER `Map` than `Vec`. The
  minimum first cut is `insert`/`get`/`contains`/`len` with the post-insert-get
  contract and the vstd-`Map` key-uniqueness invariant; `pop`/`remove`/iteration
  and a `len() <= CAP` capacity bound on the Map may follow in a later pass.
  RECOMMEND the thin first cut (insert/get/contains/len) at Stage 4 — it certifies
  the practical "verified key-value store" claim without the iteration proof
  surface — and defer `remove`/iteration to a Stage-4 follow-up under #62. Not a
  blocker; flagged so the builder does not over-scope the Map.
  **RESOLVED (#123): the thin first cut shipped** (`insert`/`get`/`contains_key`/
  `len`, the post-insert-get round-trip + absent→`None`; `remove`/iteration still
  deferred — see `.design/basis/13-map.md`).

- **OQ-4 (`fx alloc` as a non-`pure` corpus program — shared with Stage 1):** a
  `Vec`-constructing / `push`-ing `fn` is non-`pure` (`fx alloc`), exercising
  `Effect::Alloc` and effect-subsumption (`.design/lower/effect-subsumption.md`).
  This is the SAME first-non-`pure` exercise Stage 1's `Box` construction raises
  (`.design/basis/01-adts.md` OQ-4). If Stage 1 lands the `fx alloc` exec
  constructor first (RECOMMENDED there), Stage 4 inherits the exercised effect; if
  not, `vec_accum.th` is the first `fx alloc` corpus entry. RECOMMEND Stage 1 land
  `alloc` first (R-DEFER-7); Stage 4's `Vec` then reuses it. Not a blocker.
