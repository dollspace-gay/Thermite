# Basis Cluster C12 — Bounded Verified `Map<K, V>`
<!--
tier: 3-component
status: draft
audited-sha: 0c75cc0bf39299c38fc6242d0989ca05832188c5 (bootstrap pin: decision 4 — doc-last-touch, NOT verified-current; backlog #262)
governs: thermite-syntax/src/ast.rs
governs: thermite-syntax/src/parser.rs
governs: thermite-spec/src/validator.rs
governs: thermite-lower/src/lower.rs
thesis-refs:
  - thermite-design.md §4.1
  - thermite-design.md §4.2
  - thermite-design.md §4.4
  - thermite-design.md §6
-->

## Summary

Cluster **C12** (crosslink **#114**) adds the last deferred basis-stdlib primitive:
a **bounded, verified `Map<K, V>`** — a key-value collection with `insert` /
`get` / `contains_key` / `len`, a capacity bound (`len() <= CAP`) and a
key-uniqueness invariant, the no-OOB accessor (`get(absent) -> None`, NOT a wrong
value), and the §4.2 cage preserved (every operation bounded, every quantifier a
named / frozen-trigger predicate). It is the `Map` half of
`.design/basis/04-collections.md` REQ-2/REQ-6 (deferred there to a Stage-4
follow-up); this doc grounds and pins it as its own cluster.

`Map` is the **HARD** basis collection: it is a NEW two-type-argument `Type`
variant (`Type::Map(Box<Type>, Box<Type>)`, mirroring the shipped two-arg
`Type::Result(Box, Box)` from C7) — a new exec backing, a new spec abstraction
view, and the no-OOB / capacity contracts, all at once. It is the third
wrap-a-backing collection after `Vec` (`.design/basis/04-collections.md` REQ-5,
SHIPPED) and `String` (`.design/basis/07-strings.md`, SHIPPED); it reuses C7's
`Option` (`get -> Option<V>`, the handled-or-loud refusal), C6's `Vec<T>` backing
(`.design/basis/04-collections.md` REQ-9 non-Copy / tuple element), and C9's
tuples (`.design/basis/10-recursion-tuples.md` REQ-5, the `(K, V)` pair).

This doc ADAPTS to the existing code: `Map` is **probe-confirmed missing** — there
is no `Map` arm in `enum Type` (`thermite-syntax/src/ast.rs` carries
`Prim`/`Unit`/`Ref`/`Slice`/`Generic`/`Named`/`Box`/`Vec`/`String`/`Option`/
`Result`/`Tuple`, no `Map`), no `parse_type` `"Map"` arm, no `emit_map_wrappers`,
and `insert`/`contains_key` are not in `BUILTIN_METHODS`
(`thermite-spec/src/validator.rs` — `len`/`get`/`last`/`contains`/… only). **UPDATE
(#123): all REQs SHIPPED** — `Type::Map` two-arg node + `emit_map_wrappers` (the
Vec-of-pairs `TMap` backing + spec view + the ops) + the Type-match/skill ripple
landed, real verus `9 verified, 0 errors` (the insert-then-get round-trip +
absent→None), the broken `Some(0)`-for-absent FAILS (non-vacuity); see the REQ
status table. The full Verus path was
GROUNDED with the real `verus 0.2026.05.24` binary before this contract was pinned
(see Verification — the insert-then-get round-trip and the absent→None refusal
both certify L3 NON-VACUOUSLY).

## Decision: the exec backing — a `Vec<(K, V)>`-of-pairs with a spec abstraction view

vstd offers an exec map (`vstd::std_specs::hash` / `HashMapWithView`) and a pure
spec `vstd::map::Map`. Three backings were considered:

- **(a) wrap vstd's exec hash map** — `HashMapWithView<K, V>` with a `view()` to
  `vstd::map::Map`. The richest abstraction, but couples to vstd's hashing
  specifications (key `Hash`/`Eq` obligations, the `View` trait bound) and a
  heavier proof surface than the bounded basis needs — and the round-trip
  obligations are non-trivial to drive without leaning on vstd lemmas that move
  across versions (the `final(self)` migration class, §6.x of 04-collections).
- **(b) a `Vec<(K, V)>` of key-value pairs with a linear scan** — the EXACT
  generalization of the SHIPPED bounded `Vec` (`.design/basis/04-collections.md`
  REQ-5/REQ-9): a Thermite `Map<K, V>` lowers to a `TMap<K,V>` newtype over
  `vstd::vec::Vec<(K, V)>`, with a key-uniqueness invariant + a capacity bound
  carried as a `well_formed` predicate, `insert` an append (under
  `!contains_key(k)`), `get`/`contains_key` an exec linear scan, and a spec
  abstraction (`spec_contains_key`/`spec_dom`) over the backing `Seq` view. This
  is the SAME wrap-a-`Vec` form as `String` (a `TString` over `Vec<u8>`) and the
  Vec-of-pairs reuses C9's `(K, V)` tuple + C6's `Vec<tuple>`.
- **(c) parallel key/value `Vec`s** — two `Vec`s indexed in lockstep. Avoids the
  tuple dependency but doubles the invariant surface (the lengths must stay
  equal) for no expressiveness gain over (b).

**DECIDED: option (b), a `Vec<(K, V)>`-of-pairs with a spec abstraction view.** It
is the only option GROUNDED end-to-end *today* (Verification — `TMapU64U64` over
`Vec<(u64, u64)>` with `well_formed`/`spec_contains_key`/`get`/`contains_key`/
`insert` + the insert-then-get round-trip + the absent→None refusal, **`8
verified, 0 errors`**); it reuses the SHIPPED `Vec` wrap-a-backing pattern
(`emit_map_wrappers` mirrors `emit_vec_wrappers`), composes the SHIPPED C9 tuple +
C6 `Vec<tuple>` + C7 `Option`, and keeps the §4.2 cage (the scan invariant is a
flat `forall|j| 0 <= j < i ==> data@[j].0 != k`, the abstraction a named spec fn).
The capacity bound `CAP` is the SAME `1_000_000` idiom as the `Vec` (`VEC_CAP`).
Option (a) is the richer future backing the BACKING-AGNOSTIC surface contract
(below) makes safe to migrate to; option (c) is strictly worse than (b).

**BACKING-AGNOSTIC SURFACE CONTRACT (the #62 `Vec` resolution, REQUIRED).** The
Thermite-surface `Map` contract — `insert`/`get`/`contains_key`/`len` + the
capacity and key-uniqueness invariants — is specified INDEPENDENTLY of the
backing. The contract names what each operation guarantees over a spec map
abstraction (`get(k) -> Some(v)` iff `k` maps `v`; `get(absent) -> None`;
`contains_key(k) == k in dom`), never `vstd::vec::Vec<(K,V)>`. v1 IMPLEMENTS that
contract with the Vec-of-pairs (proven-for-free against vstd's `Vec`); a later
decouple to vstd's exec hash map (option a) or a custom store swaps the lowering
target WITHOUT changing the surface contract or any user `.th` code (§6/§9 "the
contract is the interface"), exactly as `Vec`'s backing-agnostic resolution pins.

## The §4.2 cage + handled-or-loud (`get` returns `Option`, absent → `None`)

`Map`'s `get` IS the data-side incarnation of the toolchain's **handled-or-loud**
law (`.design/basis/01-adts.md` "the unifying principle";
`.design/basis/06-provenance-and-sinks.md`): a lookup of an ABSENT key returns
`None` (the C7 `Option`, REUSED) — NOT a wrong value, NOT a panic, NOT a sentinel
0. The consumer's exhaustive `match result { Some(v) => …, None => … }` (the
COMPILE-TIME tooth, `.design/basis/01-adts.md` REQ-5/REQ-12, SHIPPED) forces the
absent case to be HANDLED or to SCREAM. This is the no-OOB accessor generalized
from `Vec::get(i)`'s index bound to `Map::get(k)`'s key membership: where the
`Vec` `get` carries `req i < len`, the `Map` `get` is TOTAL (it takes any `k`) and
encodes the partiality in the `Option` RESULT — strictly louder than a precondition
the caller might forget. GROUNDED non-vacuous: a `get` returning `Some(0)` for an
absent key FAILS verus (`2 verified, 1 errors` — the `None => !spec_contains_key(k)`
arm bites).

Every `Map` operation is **bounded** (§4.2 cage): `len() <= CAP`; `insert` is
guarded by `len < CAP`; `get`/`contains_key` are exec linear scans whose loop
carries the flat invariant `forall|j: int| 0 <= j < i ==> data@[j].0 != k` +
`decreases data.len() - i` (the SAME scan shape as the SHIPPED `Vec::contains`,
`.design/basis/04-collections.md` REQ-8). The spec abstraction
(`spec_contains_key`/`spec_dom`/`len`) is a named spec fn / `Set`-comprehension
with a frozen trigger — never an anonymous nested quantifier admitted raw into the
cage. The validator's caged-flat walk (`walk_expr_inner`'s `MethodCall` arm,
`.design/spec/spectherm-combinators.md` REQ-6) is UNCHANGED — `m.get(k)`,
`m.contains_key(k)`, `m.len()` are flat built-ins added to `BUILTIN_METHODS`
exactly as the `Vec`/`String` accessors were.

## Requirements

### Surface + AST (governs `thermite-syntax/src/ast.rs`, `parser.rs`)

- **REQ-1 (`Map<K, V>` type — the two-type-argument AST node + grammar):** `enum
  Type` (`thermite-syntax/src/ast.rs`) gains a dedicated `Type::Map(Box<Type>,
  Box<Type>)` node — a SECOND two-type-argument type, mirroring the SHIPPED
  `Type::Result(Box<Type>, Box<Type>)` (C7, `.design/basis/09-option-result.md`
  REQ-2) verbatim (the single-arg `Generic { name, arg }` cannot carry a key AND a
  value — it dies at the comma, the EXACT C7 finding). A dedicated node (NOT a
  generalized multi-arg `Generic`) keys the lowerer/validator on the node KIND,
  consistent with every other built-in collection (`Type::Vec`/`Box`/`String`/
  `Option`/`Result`/`Tuple` are all dedicated nodes). `parser::parse_type`'s
  `Ident` arm gains a `"Map"` contextual-ident arm that parses `<K, V>` (the
  comma + second type + `>`) — the SAME two-arg parse as the `"Result"` arm. The
  mandatory-contract discipline (`.design/syntax/ast.md` REQ-2) is unchanged.
  Derived from §4.4 (closed built-in interface set — `Map` is a built-in, not a
  user type) and the SHIPPED two-arg `Type::Result` precedent.

- **REQ-2 (`Map` operations are ordinary calls — no new `Expr` node):** `insert`/
  `get`/`contains_key`/`len` are ordinary method calls (`m.insert(k, v)`,
  `m.get(k)`, `m.contains_key(k)`, `m.len()`) reusing the EXISTING
  `Expr::MethodCall` (no new expression node — the one call syntax, §4.4),
  EXACTLY as `Vec`'s `push`/`get`/`len` do
  (`.design/basis/04-collections.md` REQ-1). `get` returns the C7 `Option<V>`
  (REUSED `Type::Option`); the others return `Option`/`bool`/`u64`. Derived from
  §4.4 and the SHIPPED `Vec`/`Expr::MethodCall` precedent.

### Validator / the SpecTherm cage (governs `thermite-spec/src/validator.rs`)

- **REQ-3 (`contains_key`/`insert` in `BUILTIN_METHODS`; the capacity / op
  contracts fit the §4.2 cage):** `contains_key` is ADDED to `BUILTIN_METHODS`
  (`thermite-spec/src/validator.rs`) so an `ens result == m.contains_key(k)`
  validates inside the §4.2 cage as a flat built-in (exactly as `Vec`'s
  `contains` and the no-OOB `get` were, `.design/basis/04-collections.md` REQ-12);
  `get` and `len` are ALREADY present. `insert` is EXEC-only (it mutates — never
  named in a contract), so like `push`/`pop_last` it needs NO `BUILTIN_METHODS`
  entry. The capacity bound (`m.len() <= CAP`, `m.len() < CAP`) is a flat
  comparison over the `len` built-in; the key-membership / round-trip contracts
  are written as the C7 spec-`match`-in-`ens` over the `get` result (`match
  m.get(k) { Some(v) => …, None => … }`, an admitted flat built-in,
  `.design/basis/01-adts.md` REQ-7). The caged-flat walk is UNCHANGED. Derived
  from §4.2 (the cage — every op bounded, every quantifier named/frozen),
  `.design/spec/spectherm-combinators.md` REQ-6, and the SHIPPED `BUILTIN_METHODS`
  precedent.

### Verus lowering (governs `thermite-lower/src/lower.rs`)

- **REQ-4 (`Map<K,V>` → the Vec-of-pairs wrapper + the spec abstraction view;
  `insert`/`get`/`contains_key`/`len` → verified ops; `fx alloc`):** A Thermite
  `Map<K, V>` lowers to a `TMap<K,V>` newtype over `vstd::vec::Vec<(K, V)>` (the
  Vec-of-pairs backing — C9 tuple `(K, V)` + C6 `Vec<tuple>`), materialized ONCE
  per `(K, V)` pair by a new `emit_map_wrappers` (MIRRORING `emit_vec_wrappers` /
  `emit_one_vec_wrapper`, `thermite-lower/src/lower.rs`). `lower_type`'s
  `Type::Map(k, v)` arm emits the monomorphized wrapper name (a `tmap_name(k, v)`
  helper mirroring `tvec_name`); a `map_name` reachability collector
  (mirroring `collect_vec_elem_types` / `note_vec_elems`) drives emission from
  any reachable `Map` position. The wrapper carries (GROUNDED form, Verification):
  - `pub open spec fn well_formed(&self) -> bool` = `data.len() <= CAP` (capacity)
    `&&` the key-uniqueness invariant (`forall|a, b| a != b ==> data@[a].0 !=
    data@[b].0`);
  - `pub open spec fn spec_contains_key(&self, k) -> bool` = `exists|j| 0 <= j <
    data.len() && data@[j].0 == k` (the spec membership abstraction);
  - `pub open spec fn len(&self) -> nat` = `data.len() as nat`;
  - `pub fn contains_key(&self, k) -> bool` — the exec linear scan, `req
    well_formed`, `ens result == spec_contains_key(k)`, the scan invariant +
    `decreases`; PURE;
  - `pub fn get(&self, k) -> Option<V>` — the no-OOB accessor, `req well_formed`,
    `ens match result { Some(v) => spec_contains_key(k) && (exists|j| … data@[j].0
    == k && data@[j].1 == v), None => !spec_contains_key(k) }` (the absent →
    `None` handled-or-loud refusal); PURE; reuses the C7 `Option` lowering;
  - `pub fn insert(&mut self, k, v)` — the append (under `!spec_contains_key(k)`),
    `req well_formed && len < CAP && !spec_contains_key(k)`, `ens final(self)...`
    (the `final(self)` &mut postcondition — the SHIPPED `Vec::push` grounding
    finding) `&& final(self).spec_contains_key(k) && (exists|j| … == k && … == v)
    && len' == len + 1`; carries `fx alloc` (the `Vec`-`push` / Stage-1
    `Effect::Alloc` rule, `.design/basis/04-collections.md` REQ-5).
  Spec-position `m.get(k)`/`m.contains_key(k)`/`m.len()` map to the wrapper's spec
  fns (the `lower_expr` `MethodCall` arm, mirroring `v.get(i)` →
  `v.spec_get(i as int)`). The `(K, V)` tuple element reuses C9's `Type::Tuple`
  lowering. Derived from §3 (transpile to Verus), §4.1 (`alloc`), §6 (L3), the
  SHIPPED `Vec` wrap-a-backing pattern, and the GROUNDED `TMapU64U64` proof.

- **REQ-5 (the `Type::Map` exhaustive-match ripple — the #95/#109-class
  Type-match + skill ripple):** `Type::Map(Box<Type>, Box<Type>)` is a NEW
  exhaustive-match-breaking `Type` variant (the SAME ripple class as the C7
  two-arg `Type::Result` and the C9 `Type::Tuple` — `.design/basis/
  10-recursion-tuples.md` REQ-8). Every exhaustive `match Type` across the
  workspace MUST gain a `Type::Map` arm with NO `_`/panic fallthrough
  (`goal.md` R-APG-1). The sites the builder MUST extend (the EXACT set that
  already carries `Type::Result`/`Type::Tuple` arms — grep-confirmed):
  `thermite-syntax/src/parser.rs` (the `parse_type` `"Map"` arm),
  `thermite-syntax/src/ast.rs` (the variant + doc-comment),
  `thermite-lower/src/lower.rs` (`lower_type`, the `tmap_name`/`map`-reachability
  collectors, `ty_reaches_string` recursing both args, `note_vec_elems` recursing
  both args), `thermite-lower/src/l1.rs` and `l2.rs` (the exec mirror + the
  bounded `type_label`), `forge/src/check.rs` (`collect_type_adt_refs` recursing
  both args — so a `Map<u64, Account>` weaves the `Account` decl, the #68 ADT
  weave), `forge/src/mutation.rs` and `forge/src/review.rs` (the `Type` walks),
  and `thermite-skill/src/generate.rs` (a `SkillFragment` teaching the `Map<K,V>`
  type + `insert`/`get`/`contains_key`/`len` — the skill-layer ripple, the
  6,000-token budget gate must still pass). Derived from the AST-boundary-stability
  contract (`.design/syntax/ast.md` REQ-9), §4.1, and the SHIPPED `Type::Result`/
  `Type::Tuple` ripple precedent.

- **REQ-6 (`LowerError`/`SpecError` extension, no panics):** The `Map` constructs
  reuse the EXISTING `thermite-lower::LowerError` (`Unsupported`/`TooDeep`) and
  `thermite-spec::SpecError` — a still-unlowerable `(K, V)` (a non-primitive,
  non-Named, non-String key/value the wrapper cannot monomorphize) is the existing
  `LowerError::Unsupported` (no new variant), exactly as `tvec_name` on an
  unsupported element. No `unwrap`/`expect`/`panic!` in production (R-CODE-2 /
  R-APG-1). Derived from R-CODE-2 and the existing error-enum discipline.

## Acceptance criteria

The orchestrator authors a NEW corpus program — `conformance/map_kv.th` (a
`Map<u64, u64>` with an `insert` then a `get` round-trip + an absent→`None`
lookup + a `contains_key` true/false, certifying L3) — and its golden lowering at
`tests/golden/lower/map_kv.verus.rs`, hand-authored from the GROUNDED form below
and confirmed to pass `verus`. The cert golden lives at
`conformance/map_kv.cert.json`.

- **AC-1 (`Map<u64,u64>` parses, validates, lowers, the insert-then-get
  round-trip certifies L3 — GROUNDED):** Parsing `map_kv.th` yields a
  `Map<u64, u64>`-typed value (`Type::Map`, the two-arg parse, REQ-1); the
  validator accepts the capacity + op contracts in the §4.2 cage (`contains_key`
  in `BUILTIN_METHODS`, the C7 spec-`match` over `get`'s result, REQ-3); the
  lowerer emits the Vec-of-pairs `TMap` wrapper + the spec abstraction + the four
  ops (REQ-4); the `insert`-ing fn carries `fx alloc` and passes
  effect-subsumption; running the real `verus` binary on the emitted output exits
  0 with `N verified, 0 errors` — `insert(k, v)` then `get(k) == Some(v)` proven
  (the round-trip, the inserted key maps the value). **GROUNDED `8 verified, 0
  errors`** (the `insert_then_get` fn ensures `result == Some(v)`). (REQ-1, REQ-3,
  REQ-4.)

- **AC-2 (absent key → `None`, the handled-or-loud refusal certifies L3, and a
  wrong value FAILS — GROUNDED non-vacuity):** A `get(absent_key)` over a `Map`
  not containing `absent_key` certifies `result is None` at L3 (the
  `None => !spec_contains_key(k)` arm — a key NOT inserted is NOT silently mapped
  to a wrong value). A crafted `get` returning `Some(0)` for an absent key FAILS
  to verify (R-DEFER-9 non-vacuity — the `None` refusal arm has teeth). **GROUNDED:
  `get_absent` ensures `result is None` at L3 (within the `8 verified, 0 errors`);
  the broken `Some(0)`-for-absent form FAILS `2 verified, 1 errors`.** (REQ-2,
  REQ-4.)

- **AC-3 (`contains_key` true AND false both provable, capacity bound enforced —
  GROUNDED):** `contains_key(k)` certifies `result == spec_contains_key(k)` for
  BOTH a present key (true) and an absent key (false) at L3; `insert` under
  `len < CAP` preserves `well_formed` (the capacity bound). The `len < CAP` guard
  is load-bearing — an `insert` without it cannot prove `final(self).well_formed`.
  **GROUNDED: `contains_key`'s `ens result == spec_contains_key(k)` verifies (the
  scan invariant proves both branches); the capacity bound is in the `well_formed`
  the round-trip threads.** (REQ-3, REQ-4.)

- **AC-4 (the `Type::Map` ripple is closed; the existing corpus is byte-stable —
  no regression):** Every exhaustive `match Type` gains a `Type::Map` arm (no
  `_`/panic fallthrough, REQ-5); the `thermite-skill` 6,000-token budget gate
  still passes with the new `Map` fragment. The existing corpus
  (`conformance/{sum,binary_search,vec_demo,option_result,parse_u64,…}.th` and
  their `.cert.json` / `tests/golden/lower/*.verus.rs` goldens) is UNCHANGED —
  `Map` is purely additive (a new `Type` variant + the `Map` lowering path + the
  `contains_key` `BUILTIN_METHODS` entry touch no existing node shape). Mechanically:
  `cargo test -p thermite-syntax -p thermite-spec -p thermite-lower`, the
  conformance corpus, and `cargo run -p thermite-skill -- --check-budget` pass with
  0 mismatches. (All REQs; C12 must not break the kernel.)

## Architecture

C12 spans three crates, additively, mirroring the C7 / collections layer split:

- **`thermite-syntax`** — `enum Type` (`thermite-syntax/src/ast.rs`) gains
  `Map(Box<Type>, Box<Type>)`, the SECOND two-type-argument node (the first being
  `Type::Result`, C7). `parse_type`'s `Ident` arm gains the `"Map"`
  contextual-ident arm parsing `<K, V>` (the comma + second type + `>`, the SAME
  two-arg parse as `"Result"`). `insert`/`get`/`contains_key`/`len` reuse
  `Expr::MethodCall` (no reshape).

- **`thermite-spec`** — `validator.rs`'s `BUILTIN_METHODS` gains `contains_key`
  (`get`/`len` already present); `insert` stays EXEC-only (no entry). The
  caged-flat walk (`walk_expr_inner`'s `MethodCall` arm, the §4.2 cage) is
  UNCHANGED — a `Map` `get`/`contains_key`/`len` is the same flat built-in as a
  `Vec` accessor; the round-trip contract is the C7 spec-`match` over `get`'s
  `Option` result (an admitted flat built-in).

- **`thermite-lower`** — `lower.rs` (`pub fn lower` / `lower_type` / `lower_expr`)
  gains the `Map` lowering path: a new `emit_map_wrappers` (mirroring
  `emit_vec_wrappers`, called from `lower` alongside it) materializing the
  Vec-of-pairs `TMap` newtype + the spec abstraction view + the four ops; a
  `tmap_name(k, v)` monomorphization helper (mirroring `tvec_name`); a `Map`
  reachability collector (mirroring `collect_vec_elem_types`). The two lowering
  contexts (exec vs spec) extend: `m.insert(k, v)` is exec position (carries `fx
  alloc`); `m.get(k)` / `m.contains_key(k)` / `m.len()` are spec position over the
  abstraction. The L1 mirror (`l1.rs`) and bounded `l2.rs` gain the `Type::Map`
  arms (REQ-5).

Symbol anchors: `enum Type` (`Map`) in `ast.rs`; `fn parse_type` in `parser.rs`;
`pub fn validate` + `BUILTIN_METHODS` in `validator.rs`; `pub fn lower` /
`lower_type` / `lower_expr` / `emit_map_wrappers` / `tmap_name` in `lower.rs`;
`enum Effect` `Alloc` in `ast.rs` (REUSED). `Map` operations are
`Expr::MethodCall` (no new `Expr` node).

### The verified Verus form (GROUNDED — the lowering contract, not guesses)

Produced by the real `verus 0.2026.05.24` binary during authoring (Verification).
This is the seed for the `map_kv.th` golden lowering. The exec backing is a
`Vec<(u64, u64)>`-of-pairs (C9 tuple + C6 `Vec<tuple>`); the spec abstraction is
`spec_contains_key` / `spec_dom` over the backing `Seq` view; `get` returns the
C7 `Option<u64>`.

```verus
pub spec const MAP_CAP: usize = 1_000_000;

pub struct TMapU64U64 { pub data: Vec<(u64, u64)> }

impl TMapU64U64 {
    pub open spec fn spec_dom(&self) -> Set<int> {              // the key-set abstraction
        Set::new(|k: int| exists|j: int|
            0 <= j < self.data.len() && self.data@[j].0 as int == k)
    }
    pub open spec fn well_formed(&self) -> bool {               // capacity + key-uniqueness
        &&& self.data.len() <= MAP_CAP
        &&& (forall|a: int, b: int|
                0 <= a < self.data.len() && 0 <= b < self.data.len() && a != b
                ==> self.data@[a].0 != self.data@[b].0)
    }
    pub open spec fn spec_contains_key(&self, k: u64) -> bool { // the membership abstraction
        exists|j: int| 0 <= j < self.data.len() && self.data@[j].0 == k
    }
    pub open spec fn len(&self) -> nat { self.data.len() as nat }

    pub fn contains_key(&self, k: u64) -> (result: bool)        // exec scan, both branches proved
        requires self.well_formed(),
        ensures result == self.spec_contains_key(k),
    { /* linear scan: invariant forall|j| 0<=j<i ==> data@[j].0 != k; decreases len-i */ }

    pub fn get(&self, k: u64) -> (result: Option<u64>)          // the no-OOB / handled-or-loud accessor
        requires self.well_formed(),
        ensures match result {
            Some(v) => self.spec_contains_key(k)
                && (exists|j: int| 0 <= j < self.data.len()
                       && self.data@[j].0 == k && self.data@[j].1 == v),
            None => !self.spec_contains_key(k),                 // absent → None, NOT a wrong value
        },
    { /* linear scan; on match return Some(v); fall through to None */ }

    pub fn insert(&mut self, k: u64, v: u64)                    // append under !contains; fx alloc
        requires old(self).well_formed(), old(self).data.len() < MAP_CAP,
                 !old(self).spec_contains_key(k),
        ensures
            final(self).well_formed(),                          // capacity + uniqueness preserved
            final(self).spec_contains_key(k),
            exists|j: int| 0 <= j < final(self).data.len()
                && final(self).data@[j].0 == k && final(self).data@[j].1 == v,
            final(self).data.len() == old(self).data.len() + 1,
    { self.data.push((k, v)); /* assert the new pair witnesses k -> v */ }
}

fn insert_then_get(m: &mut TMapU64U64, k: u64, v: u64) -> (result: Option<u64>)  // THE ROUND-TRIP
    requires old(m).well_formed(), old(m).data.len() < MAP_CAP, !old(m).spec_contains_key(k),
    ensures result == Some(v),                                  // insert(k,v) then get(k) == Some(v)
{ m.insert(k, v); m.get(k) }

fn get_absent(m: &TMapU64U64, k: u64) -> (result: Option<u64>) // THE ABSENT → None refusal
    requires m.well_formed(), !m.spec_contains_key(k),
    ensures result is None,
{ m.get(k) }
```

**RECORDED FINDING (the bounded-`Map` stack is end-to-end feasible).** The
capacity invariant + key-uniqueness (`well_formed`), the membership abstraction
(`spec_contains_key`), the exec linear-scan `contains_key`/`get`, the
append-under-`!contains` `insert` (`final(self)`), the **insert-then-get
round-trip** (`insert_then_get` ensures `result == Some(v)`), and the **absent→None
refusal** (`get_absent` ensures `result is None`) all verify together — **`8
verified, 0 errors`**. Cheat-token grep (`assume`/`external_body`/`admit`/
`verifier::external`): NONE. Non-vacuity confirmed: a companion `get` returning
`Some(0)` for an absent key FAILS — **`2 verified, 1 errors`** (`postcondition not
satisfied` — the `None => !spec_contains_key(k)` arm has teeth, R-DEFER-9). The
two-type-arg monomorphization composes: the backing `Vec<(u64, u64)>` is C6's
`Vec<tuple>` over C9's `(u64, u64)` pair (the SAME non-Copy / tuple element path
the SHIPPED Vec completeness proved), and `get -> Option<u64>` is C7's `Option`.
**Migration note:** the verus version (0.2026.05.24) requires `final(self)` (not
bare `self`) in the `&mut insert` postcondition — the lowerer emits `final(...)`
for the `&mut`-mutating `insert` ensures, EXACTLY as `Vec::push`/`pop_last`/
`insert`/`remove` already do (`.design/basis/04-collections.md` REQ-5/REQ-8).
**Scratch cleaned (§53)** — no stray `*.rlib`/`*.d` left.

### The dependency chain (composes — GROUNDED)

`Map<K, V>` is the composition of THREE shipped clusters; the chain was confirmed
to compose end-to-end in the grounding (`8 verified, 0 errors`):

- **C7 `Option<T>`** (`.design/basis/09-option-result.md` REQ-1, SHIPPED) —
  `get(k) -> Option<V>` reuses the C7 `Type::Option` lowering and the
  spec-`match`-in-`ens` for the round-trip / refusal contract. The absent→`None`
  is the C7 handled-or-loud `None`.
- **C6 `Vec<tuple>` / non-Copy** (`.design/basis/04-collections.md` REQ-9, SHIPPED)
  — the backing is `Vec<(K, V)>`, a `Vec` of a tuple (non-Copy in general); the
  `emit_map_wrappers` MIRRORS `emit_one_vec_wrapper` and the per-`(K,V)`
  monomorphization mirrors `tvec_name`. **Confirmed `Vec<(u64, u64)>` composes**
  (the backing in the grounded `8 verified, 0 errors`).
- **C9 tuples** (`.design/basis/10-recursion-tuples.md` REQ-5/REQ-7, SHIPPED) —
  the `(K, V)` pair is C9's `Type::Tuple`; `data@[j].0` / `data@[j].1` are C9's
  projection (`Expr::TupleProj`) over the backing element. **Confirmed `(u64, u64)`
  + `.0`/`.1` compose** (the spec abstraction in the grounded form indexes
  `data@[j].0`/`.1`).

**No gap found.** The Vec-of-pairs backing is the precise composition of C6 + C9,
and `get -> Option` is C7 — all three SHIPPED, all three compose in the grounded
`8 verified, 0 errors`. (If a future builder hits a tuple-in-`Vec` monomorphization
edge the SHIPPED `Vec<struct>` path does not cover, that is a fresh blocker filed
then — none surfaced in the grounding.)

## Verification

- **Mandatory Verus grounding (DONE during authoring — real `verus
  0.2026.05.24`).** A single `verus!{}` file containing the `TMapU64U64` over
  `Vec<(u64, u64)>` (`well_formed` = capacity + key-uniqueness, `spec_dom`/
  `spec_contains_key` abstraction, the exec linear-scan `contains_key`/`get`, the
  append `insert` with `final(self)`), the insert-then-get round-trip
  (`insert_then_get` ensures `result == Some(v)`), and the absent→None refusal
  (`get_absent` ensures `result is None`) verified:

  ```
  verus --no-cheating /tmp/map_ground.rs
  verification results:: 8 verified, 0 errors
  ```

  Cheat-token grep (`assume`/`external_body`/`admit`/`verifier::external`) over
  the file: NONE. Non-vacuity confirmed by a companion run returning `Some(0)` for
  an absent key in `get`: it correctly FAILS (`2 verified, 1 errors` —
  `postcondition not satisfied`, the `None` refusal arm bites). This proves the
  bounded-`Map` + capacity + key-uniqueness + no-OOB-get(Option) + insert-then-get
  round-trip stack is Verus-feasible end to end. (Scratch cleaned per §53.)

- **AC-1/AC-2/AC-3:** `cargo test -p thermite-syntax -p thermite-spec -p
  thermite-lower`, plus a harness that shells the real `verus` binary on the
  emitted lowering of `map_kv.th` and asserts exit 0 + `N verified, 0 errors`
  (R-CODE-4: subprocess status checked, never swallowed), plus `forge check`
  matching `conformance/map_kv.cert.json`. The non-vacuity negative (a `get`
  returning a wrong value for an absent key) must FAIL to verify (R-DEFER-9).
- **AC-4:** the existing `tests/golden/lower/*.verus.rs` + `*.cert.json` assertions
  stay green (no regression); `cargo run -p thermite-skill -- --check-budget`
  passes with the new `Map` fragment.

Gauntlet (R-DEFER-6, per crate): `cargo test -p <crate>`, `cargo clippy -p
<crate> --all-targets -- -D warnings`, `cargo fmt --check`.

## Routes to add (orchestrator)

C12 adds NEW concerns to files that already carry routes; add these routes to
`tooling/spec-routes.toml` pointing at THIS doc (a file may carry multiple
governing docs — the `lower.rs` precedent):

```
[[route]]  crate_pattern = "thermite-syntax/src/ast.rs"      design = ".design/basis/13-map.md"  reference = ["conformance/map_kv.th"]
[[route]]  crate_pattern = "thermite-syntax/src/parser.rs"   design = ".design/basis/13-map.md"  reference = ["conformance/map_kv.th"]
[[route]]  crate_pattern = "thermite-spec/src/validator.rs"  design = ".design/basis/13-map.md"  reference = ["conformance/map_kv.th"]
[[route]]  crate_pattern = "thermite-lower/src/lower.rs"     design = ".design/basis/13-map.md"  reference = ["tests/golden/lower/map_kv.verus.rs"]
```

The corpus program `conformance/map_kv.th`, its `.cert.json` golden, and the
`tests/golden/lower/map_kv.verus.rs` lowering are authored by the orchestrator
from this doc (and the GROUNDED `TMapU64U64` seed) before the builder runs
(R-CHAR-3).

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (`Map<K,V>` two-arg `Type` node + grammar) | SHIPPED | #123. `enum Type` (`thermite-syntax/src/ast.rs`) gains `Map(Box<Type>, Box<Type>)` — the SECOND two-type-argument node, mirroring the SHIPPED `Type::Result(Box, Box)`. `parser::parse_type`'s `"Map"` contextual-ident arm parses `<K, V>` (a comma + a second type + `>`, the SAME two-arg parse as `"Result"`). `Map<u64, u64>` parses to `Type::Map(Box::new(u64), Box::new(u64))`. Consumer: `thermite_lower::lower::lower_type`. Verified: `forge/tests/map_conformance.rs` (the `map_kv.th` `Map<u64,u64>` parses + lowers + verus L3). |
| REQ-2 (`Map` ops are `Expr::MethodCall` — no new node) | SHIPPED | #123. `insert`/`get`/`contains_key`/`len` over a `Map` reuse the EXISTING `Expr::MethodCall` (no new expression node — the one call syntax), parsed by `parse_postfix`. `get` returns the C7 `Option<V>`. Verified: `forge/tests/map_conformance.rs` (the corpus fns exercise all four ops). |
| REQ-3 (`contains_key`/`insert` cage admission; capacity/op contracts in §4.2) | SHIPPED | #123. `contains_key` ADDED to `BUILTIN_METHODS` (`thermite-spec/src/validator.rs`) so `ens result == m.contains_key(k)` validates inside the §4.2 cage as a flat built-in (the lowerer maps spec-position `m.contains_key(k)` → `m.spec_contains_key(k)`); `get`/`len` already present; `insert` stays EXEC-only (`&mut`, like `push`). The round-trip / absent→None contracts are the C7 spec-`match`-in-`ens` over `get`'s `Option` result. The caged-flat walk is UNCHANGED. Consumer: `validate`. Verified: `forge/tests/map_conformance.rs::ac3_..._certifies_l3` (`has_key` L3). |
| REQ-4 (`Map` → Vec-of-pairs wrapper + spec view; ops; `fx alloc`) | SHIPPED | #123. `lower.rs`: `Type::Map(k, v)` → `tmap_name` (`Map<u64,u64>` → `TMapU64U64`); `emit_map_wrappers` materializes ONCE per `(K,V)` pair the GROUNDED `TMapU64U64` newtype over `vstd::vec::Vec<(u64,u64)>` with `spec_dom`/`well_formed` (capacity + key-uniqueness)/`spec_contains_key`/`len` spec view, the exec linear-scan `contains_key`, the no-OOB / handled-or-loud `get -> Option<V>` (absent → None), and the append-under-`!contains_key` `insert` (`ens final(self)...`). A `Map`-param weaves `well_formed()` (`is_map_param_ty`); `Map::new()` `let`-init rewrites to `<TMap> { data: Vec::new() }` (`is_map_new`). `fx alloc` accepted by effect-subsumption. Consumer: `lower`. Verified: real `verus --no-cheating` — the GROUNDED `TMapU64U64` + the insert-then-get round-trip → `Some(v)` + absent → `None` + contains_key both branches = **`9 verified, 0 errors`** (`forge/tests/map_conformance.rs::ac1_2_3`); the broken `Some(0)`-for-absent FAILS **`verified, 1 errors`** (`ac2_broken_..`, non-vacuity R-DEFER-9); the emitted `map_kv.th` lowering verifies `0 errors` (`ac1_..._lowering`) + builds+runs (`ac1_..._builds_and_runs`, `demo() = 42`). |
| REQ-5 (`Type::Map` exhaustive-match + skill ripple) | SHIPPED | #123. The new two-arg `Type::Map` rippled to every exhaustive `match Type`: `parser.rs` (the `"Map"` arm), `ast.rs` (the variant + doc), `lower.rs` (`lower_type`/`tmap_name`/`tmap_type_suffix`/`collect_map_kv_types`/`note_map_kv`/`note_vec_elems`/`ty_reaches_string`), `l1.rs` (`lower_type` + `emit_map_runtime_l1` + the `Map::new()` rewrite + `ty_is_string`), `l2.rs` (`type_label`), `forge/src/check.rs` (`collect_type_adt_refs` both args — the #68 ADT weave), `forge/src/review.rs` (`render_type`), `thermite-skill/src/generate.rs` (the `Map<K,V>` SkillFragment + inventory). `mutation.rs`'s `zero_value_for`/`zero_desc` route a `Map`-returning fn to the no-scalar-zero `_` catch-all (like `Result`) — honest. No `_`/panic fallthrough. The 6,000-token skill budget gate passes. Verified: `cargo build --workspace` (exhaustiveness) + `cargo run -p thermite-skill -- --check-budget` + `forge/tests/map_conformance.rs`. |
| REQ-6 (`LowerError`/`SpecError` extension, no panics) | SHIPPED | #123. Reuses the EXISTING `LowerError::Unsupported` (`tmap_name`/`tmap_type_suffix` on a non-Copy key / unsupported key-value type — v1 grounds `Map<u64,u64>` Copy keys, OQ-4) — no new variant. No `unwrap`/`expect`/`panic!` added (R-CODE-2 / R-APG-1); verified by `cargo clippy --workspace -D warnings` + the anti-pattern-gate. |

## Open questions (for the orchestrator before the builder runs)

- **OQ-1 (the exec backing — Vec-of-pairs vs vstd's exec hash map — DECIDED):**
  the Decision section pins the **Vec-of-pairs `Vec<(K, V)>`** with a spec
  abstraction view (option b) — the only backing GROUNDED today (`8 verified, 0
  errors`), reusing the SHIPPED `Vec` wrap-a-backing pattern + the C6 `Vec<tuple>`
  + C9 tuple + C7 `Option`. vstd's exec hash map (option a) is the richer future
  backing the BACKING-AGNOSTIC surface contract makes safe to migrate to later
  WITHOUT changing user `.th` code. Pinned for the builder; not a blocker.

- **OQ-2 (`insert` semantics — append-under-`!contains` vs replace-on-collision):**
  the GROUNDED v1 `insert` is the APPEND form, `req !spec_contains_key(k)` (a
  re-insert of an existing key is a precondition violation, the caller's
  responsibility — the simplest form that grounds the round-trip + key-uniqueness
  cleanly). A REPLACE-on-collision `insert` (overwrite the value, no precondition)
  is a thicker proof surface (the uniqueness invariant must be re-established after
  an in-place value update) and is a clean additive follow-up under #114. RECOMMEND
  the append form at C12; defer replace to a follow-up. Not a blocker; flagged so
  the builder does not over-scope.

- **OQ-3 (`remove` / iteration / a `len`-capacity bound beyond `well_formed` —
  thin first cut):** REQ-4 ships the THIN first cut (`insert`/`get`/`contains_key`/
  `len`) — the `.design/basis/04-collections.md` OQ-3 thin-`Map` recommendation.
  `remove`, key/value iteration, and a tighter `dom().len()` capacity are deferred
  to a Stage-4 follow-up under #114. RECOMMEND the thin first cut (it certifies the
  practical "verified key-value store" claim without the iteration/removal proof
  surface). Not a blocker.

- **OQ-4 (the `(K, V)` monomorphization breadth — `u64`/`String`/`Named` keys and
  values):** the GROUNDED form is `Map<u64, u64>`. The `tmap_name`/`emit_map_wrappers`
  monomorphization MIRRORS `tvec_name`'s element match (`u64`/`String`/`Named`/
  nested), so `Map<String, u64>` / `Map<u64, Account>` should compose by the same
  rule the SHIPPED `Vec<String>`/`Vec<struct>` proved — but only `Map<u64, u64>`
  is GROUNDED here. The builder should ground a non-Copy key/value before claiming
  the broader monomorphization SHIPPED (a `Map<String, u64>` may need the borrow
  rule for the key comparison, the REQ-9 finding). Flagged as the breadth the v1
  corpus (`map_kv.th`, `Map<u64,u64>`) does NOT exercise. Not a blocker for the
  `Map<u64,u64>` first cut.
```

