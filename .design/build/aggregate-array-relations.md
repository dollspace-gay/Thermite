# Structural fixed-array relations for plain aggregates

<!--
tier: 3-component
status: partial
decision: array_eq, array_same_except, and array_same_except_two derive exact structural equality and quantified one/two-index frames over a fixed array's own index space for finite plain record elements without granting equality to sealed or opaque authority; the sibling logical_eq, logical_same_except, and logical_same_except_two relations quantify over a struct's declared logical index space and are specified here ahead of implementation
governs:
  - thermite-spec/src/lib.rs
  - thermite-spec/src/validator.rs
  - thermite-spec/tests/fixed_array_validate.rs
  - thermite-lower/src/lower.rs
  - thermite-lower/src/l1.rs
  - thermite-lower/src/lib.rs
  - thermite-lower/tests/fixed_array.rs
  - thermite-lower/tests/aggregate_array_relations.rs
  - thermite-tv/src/exec_encode.rs
  - thermite-tv/src/ref_encode.rs
  - thermite-tv/tests/fixed_array_tv.rs
  - forge/src/exec_tv.rs
  - forge/src/body_tv.rs
  - forge/src/verified_build.rs
  - forge/tests/body_tv.rs
  - forge/tests/contract_tv_conformance.rs
  - forge/tests/exec_tv_conformance.rs
  - forge/tests/verified_build.rs
  - conformance/verified-build/aggregate_array_relations.th
audited-content-sha256: ee5166be4cb3d1e3ceb6ff56539a08acdb8d05fa39109a9a2456f933e6af406e (re-pinned 2026-08-07 after source-oriented Forge commands resolved canonical packages through one shared front door; existing single-file behavior remains regression-covered)
extends:
  - .design/build/kernel-primitives.md
  - .design/verified/exec-tv.md
  - .design/verified/exec-stmt-tv.md
  - .design/lower/l1-runtime-checks.md
-->

## Purpose and scope

Kernel data structures routinely contain fixed tables of records rather than
only tables of integers. Thermite's native fixed arrays already provide exact
initialization, indexing, mutation, scalar extensional equality, and the
`array_same_except` frame relation. The relation family also provides
`array_same_except_two` for exact two-write frames. These primitives extend to
plain, finite record elements so a consuming project can verify descriptor,
slot, queue-entry, and capability-record tables without replacing them with
parallel scalar arrays or writing one equality scan per record type.

A collection is addressed twice over. `FixedBitmap256` stores `[u64; 4]` and
serves 256 bits; `FixedSlab64` stores three 64-element arrays and serves 64
slots. The shipped relations quantify over the storage arrays. The
`logical_eq` family specified below quantifies over the collection's own index
space, which is the vocabulary an exported contract has to use when the
representation is opaque. Both families live here because they are the same
three quantifier shapes over two different index spaces.

This is a reusable language and proof primitive. It does not define a concrete
table, allocator, scheduler, capability policy, IPC format, or kernel.

## Surface and meaning

The existing surface is unchanged:

```thermite
struct Slot {
  generation: u64,
  occupied: bool,
}

fn equal(left: [Slot; 64], right: [Slot; 64]) -> bool
req true
ens result == left.array_eq(right)
fx pure
{
  left.array_eq(right)
}

fn framed(left: [Slot; 64], right: [Slot; 64], changed: usize) -> bool
req true
ens result == left.array_same_except(right, changed)
fx pure
{
  left.array_same_except(right, changed)
}

fn framed_two(
  left: [Slot; 64],
  right: [Slot; 64],
  first: usize,
  second: usize,
) -> bool
req true
ens result == left.array_same_except_two(right, first, second)
fx pure
{
  left.array_same_except_two(right, first, second)
}
```

`array_eq` returns true exactly when every element agrees structurally.
`array_same_except` returns true exactly when every in-bounds element other
than the supplied index agrees structurally. An out-of-bounds exception means
full equality, preserving the existing scalar semantics.
`array_same_except_two` excludes exactly `first` and `second`; equal exceptions
collapse to the one-index relation, and either out-of-bounds exception excludes
no in-bounds element.

The first aggregate derivation admits a finite structural closure built from:

- `u8`, `u16`, `u32`, `u64`, `usize`, `bool`, and unit;
- fixed arrays whose elements are themselves admitted;
- tuples whose components are admitted; and
- ordinary, non-recursive Thermite structs whose fields are admitted.

Nested records, tuples, and fixed arrays are therefore included. Recursive
records, enums, references, `Box`, `Vec`, `String`, `Map`, `Option`, `Result`,
and generic/heap-backed values are rejected by the validator in this
increment. Later increments may add finite enum equality with an equally direct
proof; they must not silently inherit Rust trait behavior.

## Authority and abstraction barrier

`#[sealed]` and `#[opaque]` structs are deliberately excluded. Sealed values are
platform-minted authority, and opacity promises that representation-dependent
operations are introduced explicitly by the declaring module. A compiler-
derived structural comparator would create an ambient observation channel and
would weaken both promises. A library may instead expose an explicitly verified
identity or equivalence operation appropriate to that authority.

This primitive is ordinary equality, not ownership. It does not make a record
`Copy`, affine, linear, unique, or safe to duplicate.

## Validation

Before lowering, the validator computes the least finite set of structurally
comparable struct declarations. A struct enters the set only when it is neither
sealed nor opaque and all of its fields are already comparable. The monotone
fixed point admits declaration-order-independent nesting and rejects recursive
cycles.

Every relation operand must still be a named array (or direct reference/deref
of them) with exactly the same element type and capacity. The array element
must be in the structural-comparison closure. Invalid arity, scalar receivers,
capacity mismatch, hidden authority, recursive records, and unsupported fields
fail before code generation.

## L3 implementation and proof

Verus does not verify Rust's native array `PartialEq`, so L3 continues to use a
generated allocation-free linear scan. For every comparable aggregate array
shape declared in a program that uses a relation, lowering emits an exact
element comparator with

```text
ensures result <==> *left == *right
```

and then emits the existing const-generic array-relation implementation for
that element. Struct comparators conjoin comparisons of every field. Nested
array fields call the already verified finite-array scan and explicitly bridge
finite-view extensional equality to value equality. Tuples and nested structs
compose their corresponding exact comparators. There is no `external_body`,
assumed lemma, native `PartialEq` proof shortcut, or trusted implementation.

The array scan retains its exact contracts:

- `result <==> self@ =~= right@`; and
- `result <==> forall j, 0 <= j < N && j != except ==> self@[j] == right@[j]`;
  and
- `result <==> forall j, 0 <= j < N && j != first && j != second ==> self@[j] == right@[j]`.

The generated helper closure is deterministic and appears only when a program
uses one of the fixed-array relations. Emitting the finite declaration closure,
rather than performing call-graph reachability, keeps validation, L1 trait
derivation, L3 proof support, and TV frames on one source-stable type inventory.

## Executable and independent-validation paths

L1 derives Rust `PartialEq`/`Eq` only for plain structs in that same declared
aggregate-array closure, then uses bounded native/explicit scans. It never
derives those traits for sealed or opaque structs. L2's general ADT harness
remains outside the existing bounded-checking rung and is not upgraded by this
increment; such programs prove at L3 and remain runnable at L1.

Strict public exports admit these same finite plain values by value, and admit
their elements inside borrowed slices/fixed arrays. Each export ABI fingerprint
contains the resolved capacity values and complete transitive record field
layout/order in addition to the pinned compiler and target; changing a record
definition or a named capacity therefore changes the ABI identity directly.
Direct finite non-sealed named-record roots are now separately admitted by
`.design/build/named-record-lifecycle.md`; nested opaque records and borrowed
returns remain excluded from this array-relation subset.

Contract and executable translation validation continue to derive the finite
view meaning independently of the production helper generator. Their frames
must carry the exact required struct declarations and native aggregate-array
types. A production comparator that drops a field, swaps a field, skips an
element, or mishandles the exception index must fail the corresponding real-
Verus obligation. The two-index path additionally rejects a reference that uses
the first exception twice or otherwise drops the second exception.

## Quantified logical-index relations

Everything from here to the requirements table is specification ahead of
implementation. `fn check_array_relation_call in thermite-spec/src/validator.rs`
requires both operands to be `Type::Array`, and `fn lower_inv_expr in
thermite-lower/src/lower.rs` recognises three method names, so no part of this
form exists in the toolchain today. The surface, meaning, admitted shapes, and
lowering obligation are fixed here so an implementation is written against a
contract. The requirement rows carry the open blockers.

### Declaring a logical view

A struct declares its index space and its one-index observation with a single
attribute:

```thermite
#[logical(bound = "FIXED_SLAB_CAPACITY", observe = "fixed_slab_slot_spec")]
#[opaque] struct FixedSlab64 {
  slab_used: [bool; FIXED_SLAB_CAPACITY],
  slab_generation: [u64; FIXED_SLAB_CAPACITY],
  slab_values: [u64; FIXED_SLAB_CAPACITY],
}

spec fn fixed_slab_slot_spec(
  slab: &FixedSlab64,
  slot: usize,
) -> (bool, u64, u64)
  dec slot
{
  (
    slab.slab_used[slot],
    slab.slab_generation[slot],
    slab.slab_values[slot],
  )
}
```

`bound` names the size of the index space, which is `0 <= i < bound` over
`usize`. `observe` names the `spec fn` that reads one index. The attribute uses
the `ident = "string"` field list that `#[slag(...)]` already uses and is parsed
by the same `fn parse_attribute in thermite-syntax/src/parser.rs` dispatch that
handles `sealed`, `opaque`, `boundary`, and `slag`.

The declaration is what makes the form work for an opaque collection. The
relations read the observer and never a field, so a foreign module states a
complete all-index claim about `FixedSlab64` without naming `slab_used`,
`slab_generation`, or `slab_values`. Opacity is preserved, and the public
contract is written in the collection's vocabulary rather than its layout.

A struct carries one `#[logical]`. A collection with several observations
returns them together: the observer's result may be a tuple or a plain record,
and the finite structural closure above already fixes what equality means for
those.

### Surface

Three relations mirror the three storage relations over the declared space:

```thermite
fn fixed_slab_release(
  slab: FixedSlab64,
  handle: FixedSlabHandle64,
) -> FixedSlabRelease64
req ...
ens match result {
  SlabReleased64 { slab: released, .. } =>
    released.logical_same_except(&slab, handle_slot_spec(&handle)),
  ...
}
fx pure
```

- `left.logical_eq(right)`
- `left.logical_same_except(right, except)`
- `left.logical_same_except_two(right, first, second)`

Typing selects the family, so one relation applies to a given receiver:

| Receiver, references stripped | Family | Quantifier range |
|---|---|---|
| `[T; N]` | `array_eq` / `array_same_except` / `array_same_except_two` | `0 <= j < N` |
| a struct carrying `#[logical(bound = C, …)]` | `logical_eq` / `logical_same_except` / `logical_same_except_two` | `0 <= i < C` |
| any other type | rejected before lowering | |

Each `logical_*` relation takes the same nominal struct type as its first
argument and `usize` exception indices thereafter. Operands resolve the way the
storage family already resolves them in `fn array_equality_operand_type in
thermite-spec/src/validator.rs`: a bare name, `&name`, or `*name`.

The storage family states a transition's internal contract, where the fields
are visible. `fixed_bitmap_insert` pins
`result.words[fixed_bitmap_word_spec(bit)]` against the exact `bit_set` update
and frames the other three words with `array_same_except`; those are claims
about `[u64; 4]` and stay storage-shaped. The logical family states the exported
contract that a foreign module composes against, where the fields are out of
reach and the index space is the collection's own.

### Meaning

Let `L` carry `#[logical(bound = C, observe = obs)]`, where `C` resolves to a
`usize` constant and `obs` is a `spec fn obs(value: &L, index: usize) -> V`.

- `left.logical_eq(right)` is true exactly when, for every `i: usize` with
  `i < C`, `obs(&left, i) == obs(&right, i)`.
- `left.logical_same_except(right, except)` is true exactly when, for every
  `i: usize` with `i < C` and `i != except`,
  `obs(&left, i) == obs(&right, i)`.
- `left.logical_same_except_two(right, first, second)` is true exactly when,
  for every `i: usize` with `i < C`, `i != first`, and `i != second`,
  `obs(&left, i) == obs(&right, i)`.

The out-of-bounds conventions are the ones the storage relations already use.
An exception at or above `C` excludes no index of the space, so the relation
means full logical equality. Equal `first` and `second` collapse
`logical_same_except_two` to `logical_same_except`.

`==` at the observer's result type is the finite structural equality the
aggregate closure already fixes: value equality at a scalar or unit,
componentwise at a tuple or plain record, and finite-view extensional equality
at a `[T; N]`.

The index space comes from the attribute and from nowhere else. `C` resolves
under the rules that resolve an `[T; N]` capacity: a non-negative integer
literal or one `const NAME: usize` visible in the declaring module. It is not
read from a storage field's capacity, not inferred from a field name, and not
target-dependent. `FixedBitmap256` declares `bound = "FIXED_BITMAP_BITS"`,
whose value is 256, while its storage array has capacity 4, and the two numbers
are unrelated by construction.

### Index-transparency

An observer is index-transparent when every occurrence of its index parameter
is the index expression of a fixed-array projection rooted at its receiver
parameter, such as `slab.slab_values[slot]`, and the parameter appears nowhere
else. An observer that passes its index to another `spec fn` is
index-transparent when that callee is, decided as a monotone fixed point over
the acyclic declaration closure in the same shape as
`pub fn structural_array_equality_structs in thermite-spec/src/validator.rs`.

An index-transparent observer reads storage at the same index the logical space
uses, so a storage frame at index `k` and a logical frame at index `k` describe
the same set of preserved observations. `FixedRing64`, `FixedVec64`,
`FixedSlab64`, `FixedFreelist64`, `FixedIntrusiveList64`, `FixedDirectMap64`,
and `FixedOpenMap64` store one element per logical index, so their slot
observers are index-transparent.

`fixed_bitmap_contains_spec` applies `bit / 64` and `bit % 64` to its index, so
it is a derived-index observer: 256 logical indices share four storage words,
and a storage frame at word `w` says nothing about which of the 64 bits inside
`w` moved. A rotated view such as a ring's FIFO position, which would read
`ring.slots[(ring.head + pos) % 64]`, is derived-index for the same reason.

Index-transparency gates the two frame relations, and it does not gate
`logical_eq`. The reason is in the bridges below: whole-state equality closes by
congruence for any observer, while a frame has to relate two different index
spaces.

### Admitted shapes and fail-closed boundary

A `#[logical]` declaration is admitted when all of the following hold. A
declaration failing any of them is an error before lowering.

1. The receiver is an ordinary or `#[opaque]` struct declared in the module
   carrying the attribute, and it is non-recursive under the acyclicity that
   `pub fn structural_array_equality_structs in thermite-spec/src/validator.rs`
   already computes.
2. The receiver is not `#[sealed]`. Sealed values are platform-minted, and
   their observations come from bodyless boundary declarations, so a quantified
   claim over them would range over facts no rung has proved.
3. The struct carries one `#[logical]`. A second declaration is a duplicate
   error naming both locations.
4. `bound` resolves to a `usize` constant in `0..=1_048_576`, the Forge element
   bound applied to the index space.
5. `observe` names a `spec fn` in the same module with two parameters typed
   `(&L, usize)` and a result type inside the finite structural closure that
   `pub fn array_equality_type_is_structural in thermite-spec/src/validator.rs`
   decides. A `Vec`, `String`, `Map`, `Box`, `Option`, `Result`, enum,
   reference-bearing, generic, sealed, or foreign-opaque result is refused.

A relation call over an admitted declaration is refused before lowering, with
the diagnostic naming the rule, for:

- `logical_same_except` or `logical_same_except_two` on a derived-index
  observer, until REQ-AGGREL-5 lands. `FixedBitmap256` therefore declares its
  256-bit view and uses `logical_eq`, while its frames stay storage-shaped. The
  refusal keeps an author from stating a relation that no rung can discharge;
- operands of different nominal types, or an argument whose type carries no
  `#[logical]`;
- an arity other than one, two, or three;
- a receiver that is a fixed array, tuple, scalar, enum, `Option`, `Result`,
  `Vec`, `Map`, `String`, `Box`, or reference-bearing value. An enum has no
  index space of its own; a transition returning `FixedSlabAllocate64` states
  the relation on the `slab` field inside the variant;
- an operand that is not a bare name, `&name`, or `*name`;
- a `#[logical]` whose `bound` or `observe` fails to resolve, resolves across a
  module without an import, is cyclic, or is not `usize`;
- a key-addressed view. `FixedOpenMap64` and `FixedDirectMap64` are keyed by
  arbitrary `usize`, and an unbounded key space has no `bound`. Both admit a
  view over their 64 slots, which is the index space their storage already
  uses;
- an executable position. The family is a specification relation and appears in
  `req`, `ens`, `inv`, and `spec fn` bodies. An executable body naming it is
  refused the way `fn lower_expr in thermite-lower/src/l1.rs` refuses a raw
  quantifier in executable position.

### Lowering the relation

For each declared view reachable from a program that names the family, lowering
emits three `open spec fn`s, one per relation, each carrying a single
first-order quantifier:

```text
open spec fn __thermite_logical_same_except_FixedSlab64(
    left: &FixedSlab64,
    right: &FixedSlab64,
    except: usize,
) -> bool {
    forall|i: usize|
        #![trigger fixed_slab_slot_spec(left, i)]
        #![trigger fixed_slab_slot_spec(right, i)]
        i < FIXED_SLAB_CAPACITY && i != except
            ==> fixed_slab_slot_spec(left, i) == fixed_slab_slot_spec(right, i)
}
```

Three properties of that shape carry the proof.

One quantifier, no recursion. A depth-`C` recursive `spec fn` that unrolls the
index space certifies L0. `forge/tests/divergence_aggregate_collection_state.rs`
pins that outcome for `fixed_bitmap_state_same_except_spec` at `count = 256`:
Verus unfolds the definition once per index before the postcondition is
reachable, the unfolding budget is spent first, and the obligation returns
`error: postcondition not satisfied`. A `forall` moves the work from unfolding
to instantiation.

Frozen triggers. The quantifier's triggers are the observer applied to each
operand, written as alternatives so either term fires on its own. A consumer
that mentions `fixed_slab_slot_spec(&result, k)` for a concrete `k` gets the
instantiation at `i = k` with no hint. This follows the frozen-trigger
discipline of `thermite-design.md` §4.

`usize` indices. The relation quantifies over `usize`, so `i < C` is the
comparison a Thermite author writes and no `as int` coercion enters a trigger
term.

### What makes the form dischargeable

Consuming the relation is one instantiation and needs nothing further.
Establishing it is the direction that needs support, and lowering emits a
generated `proof fn` bridge per declared view for each relation it admits. The bridge's
premises are facts the transition's other `ens` clauses already establish, and
lowering emits a `proof { }` invocation of the bridge in the generated body when
a postcondition names the relation. A lowering that discharges the same
implication by another route satisfies AC-4 as long as no author-written hint is
required and the golden lowering matches.

The equality bridge takes value equality of the fields the observer reads:

```text
proof fn __thermite_logical_bridge_eq_FixedBitmap256(
    left: &FixedBitmap256,
    right: &FixedBitmap256,
)
    requires left.words == right.words,
    ensures __thermite_logical_eq_FixedBitmap256(left, right),
```

The body is congruence. For a skolem `i`, both sides unfold to the observer
applied to arguments that are equal by the premise, so the two terms are equal
whatever the observer does with `i`. No arithmetic, no bit-vector reasoning, and
no index-transparency requirement. The author reaches the premise from
`left.words.array_eq(&right.words)` through the finite-view-to-value bridge that
the shipped comparators already emit, which is why `logical_eq` is admitted for
every declared view.

The frame bridge takes the storage frame at the same index:

```text
proof fn __thermite_logical_bridge_same_except_FixedSlab64(
    left: &FixedSlab64,
    right: &FixedSlab64,
    except: usize,
)
    requires
        left.slab_used
            .__thermite_fixed_array_same_except_spec(&right.slab_used, except),
        left.slab_generation
            .__thermite_fixed_array_same_except_spec(&right.slab_generation, except),
        left.slab_values
            .__thermite_fixed_array_same_except_spec(&right.slab_values, except),
    ensures
        __thermite_logical_same_except_FixedSlab64(left, right, except),
```

Index-transparency is what discharges that body. Every field the observer reads
is indexed by `i` directly, so each storage relation instantiated at `j = i`
gives the field equality, and the observer's two applications then differ only
in arguments that are equal. The step is congruence plus one instantiation per
read field, with no arithmetic. The two-index bridge has the same shape over
`array_same_except_two`.

A `.th` author therefore writes the storage frame inside the transition, where
the fields are visible, and states the logical frame in the public contract,
where they are not.

### The packed frame and what it needs

`fixed_bitmap_contains_spec` reads
`bitmap.words[bit / 64].bit_test(bit % 64)`. Its 256 logical indices share four
storage words, so the frame bridge above does not apply and the two frame
relations are refused on that receiver. Establishing
`result.logical_same_except(&bitmap, bit)` from the postconditions
`fixed_bitmap_insert` already proves needs three facts the toolchain does not
supply:

1. `i < 256 ==> i / 64 < 4`, to instantiate the storage frame at the derived
   word.
2. `i / 64 == bit / 64 && i != bit ==> i % 64 != bit % 64`, to reach the case
   where the observed bit shares the written word.
3. A proof-position form of the bit-preservation witness.
   `pub fn u64_bit_defs in thermite-lower/src/lower.rs` emits
   `__thermite_u64_bit_set_preserves_other` and
   `__thermite_u64_bit_clear_preserves_other` as executable functions whose
   `ensures` carries the fact, and a generated bridge proof cannot call an
   executable function.

Items 1 and 2 are Euclidean division and modulus facts with a literal divisor,
reachable from Verus's division axioms or from `vstd::arithmetic::div_mod`. The
lowerer already emits proof bodies in that register: `pub fn u64_bit_defs`
produces `__thermite_u64_bit_mask_shift_lemma` with 64 `by(bit_vector)` arms,
and the string lemmas use `by(nonlinear_arith)`. The technique is present in the
repository; the artifacts for this decomposition are not, and item 3 is a new
generated artifact class. REQ-AGGREL-5 owns the work, and the bridge it emits
also needs the index decomposition itself, since the compiler learns `i / 64`
and `i % 64` from the observer's body rather than from a declaration.

An exhaustive case split over the index space is not the route. Splitting on the
observed bit alone leaves the exception index dynamic, so the shared-word case
still needs fact 2, and splitting on both indices is 65,536 arms.

### Independent translation validation

Contract, expression, and body translation validation derive the index space and
the observer application from the source declaration rather than importing the
production emitter, matching how they already derive the finite view for the
storage relations. A reference that reads the wrong `bound`, applies the
observer to one operand twice, drops a field the observer reads, or ignores the
exception index must fail its direct Verus obligation.

L1 evaluates a contract clause naming the relation as a bounded `for i in 0..C`
loop over the observer's L1 executable twin, which `fn lower_spec_fn in
thermite-lower/src/l1.rs` already produces for every `spec fn`. The observer's
result type is inside the aggregate closure, so its runtime equality is the
`PartialEq`/`Eq` derivation L1 already performs for that closure.

## Requirements

<!-- generated:reqs view=forge-aggregate-array-relations-status -->
Source: `.design/reqs/registry.toml`

| ID | Status | Owner | Title | Follow-up |
|---|---|---|---|---|
| REQ-AGGREL-1 | shipped | `.design/build/aggregate-array-relations.md` | Storage-space fixed-array relations |  |
| REQ-AGGREL-2 | not_started | `.design/build/aggregate-array-relations.md` | Declared logical index space and observer | Parse the attribute in `fn parse_attribute in thermite-syntax/src/parser.rs`, carry `bound`/`observe` on `StructItem`, resolve the capacity under the existing `[T; N]` const rules, and resolve the observer to a two-parameter `spec fn` typed `(&Self, usize) -> V` with `V` inside the finite structural closure.<br>blockers: github:dollspace-gay/Thermite#131 |
| REQ-AGGREL-3 | not_started | `.design/build/aggregate-array-relations.md` | Logical-index relation surface and fail-closed validation | Extend the relation gate in `fn check_array_relation_call in thermite-spec/src/validator.rs` so a struct receiver carrying a logical view is admitted, and refuse sealed receivers, enums, key-addressed views, mismatched nominal operands, non-path operands, wrong arity, duplicate declarations, executable positions, and a frame relation over a derived-index observer.<br>blockers: github:dollspace-gay/Thermite#131 |
| REQ-AGGREL-4 | not_started | `.design/build/aggregate-array-relations.md` | Quantified lowering and index-transparent bridge | Emit the three `open spec fn` relations and the three bridge `proof fn`s in `thermite-lower/src/lower.rs`, recognise the three method names in `fn lower_inv_expr`, derive the index space independently in contract/expression/body TV, and evaluate the relation at L1 as a bounded loop over the observer's executable twin.<br>blockers: github:dollspace-gay/Thermite#131 |
| REQ-AGGREL-5 | not_started | `.design/build/aggregate-array-relations.md` | Derived-index logical frames | Emit a `proof fn` twin of the bit-preservation witnesses produced by `pub fn u64_bit_defs in thermite-lower/src/lower.rs`, supply the literal-divisor division and modulus facts relating a logical index to its storage slot and offset, and admit a declared index decomposition so the bridge can case-split on the shared slot.<br>blockers: github:dollspace-gay/Thermite#132 |
<!-- /generated:reqs -->

### REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-AGGREL-1 (storage index space) | SHIPPED | impl `fn check_array_relation_call in thermite-spec/src/validator.rs` gates arity, operand shape, capacity, and element closure; `pub fn fixed_array_equality_defs_for_program in thermite-lower/src/lower.rs` emits the const-generic scans whose `ensures` carries `result <==> forall j, 0 <= j < N && j != except ==> self@[j] == right@[j]`. Non-test consumers: `fn lower_inv_expr in thermite-lower/src/lower.rs` (the three method arms) and `stdlib/kernel-primitives/collections/bitmap.th` (`fixed_bitmap_words_same_except_spec`). Verification: `thermite-lower/tests/aggregate_array_relations.rs`, `thermite-spec/tests/fixed_array_validate.rs`, `thermite-tv/tests/fixed_array_tv.rs`, and `conformance/verified-build/aggregate_array_relations.th`. |
| REQ-AGGREL-2 (logical view declaration) | NOT-STARTED | open prereq blocker #131. `fn parse_attribute in thermite-syntax/src/parser.rs` accepts `slag`, `boundary`, `sealed`, and `opaque` only, so `#[logical(bound = …, observe = …)]` has no parse, no AST field, and no capacity or observer resolution. |
| REQ-AGGREL-3 (logical relation surface) | NOT-STARTED | open prereq blocker #131. `fn check_array_relation_call in thermite-spec/src/validator.rs` matches `(Some(Type::Array), Some(Type::Array))`, so a struct receiver is refused and the three `logical_*` names are unknown to the validator. |
| REQ-AGGREL-4 (quantified lowering and bridges) | NOT-STARTED | open prereq blocker #131. `fn lower_inv_expr in thermite-lower/src/lower.rs` recognises `array_eq`, `array_same_except`, and `array_same_except_two`; no emitter produces a per-view `forall` relation or the congruence `proof fn` bridges, and no TV encoder derives a logical index space. |
| REQ-AGGREL-5 (packed frame) | NOT-STARTED | open prereq blocker #132. The bitmap's 256-over-4 decomposition needs a proof-position twin of the witnesses emitted by `pub fn u64_bit_defs in thermite-lower/src/lower.rs` plus literal-divisor division and modulus facts; neither exists, so `logical_same_except` and `logical_same_except_two` are refused on `fixed_bitmap_contains_spec` until it lands. `forge/tests/divergence_aggregate_collection_state.rs` holds the pinned L0 outcome for the recursive encoding. |

## Acceptance criteria

- AC-1 (REQ-AGGREL-1): `cargo test -p thermite-lower --test aggregate_array_relations`
  and `cargo test -p thermite-spec --test fixed_array_validate` pass, and
  `conformance/verified-build/aggregate_array_relations.th` builds and replays at
  strict L3.
- AC-2 (REQ-AGGREL-2): a `#[logical]` whose `bound` is unknown, cyclic,
  non-`usize`, or outside `0..=1_048_576`, whose `observe` has the wrong arity
  or an inadmissible result type, or which appears twice on one struct, fails
  validation with the rule named, and a `#[sealed]` receiver is refused.
- AC-3 (REQ-AGGREL-3): for a declared view with `bound = C`, a program stating
  `left.logical_same_except(right, k)` over an index-transparent observer
  validates, while a fixed-array receiver, a mismatched nominal argument, a
  computed operand, a key-addressed view, an enum receiver, an
  executable-position use, and a frame relation over a derived-index observer
  each fail before lowering.
- AC-4 (REQ-AGGREL-4): a golden lowering under `tests/golden/lower/` pins the
  emitted `forall` with its two alternative observer triggers and both bridge
  `proof fn`s; a real-Verus fixture over an index-transparent 64-slot view
  discharges `logical_eq`, `logical_same_except`, and
  `logical_same_except_two` from the corresponding storage relations at L3
  without an author-written hint; a mutant that drops one read field from the
  observer, reuses the first exception index, or shifts `bound` by one fails its
  direct obligation; and contract, expression, and body TV rows derive the index
  space independently.
- AC-5 (REQ-AGGREL-5): `FixedBitmap256`'s `fixed_bitmap_insert`,
  `fixed_bitmap_remove`, and `fixed_bitmap_set_to` export
  `logical_same_except` against the requested bit at strict L3, a mutant that
  frames the wrong bit fails its obligation, and the pinned rows in
  `forge/tests/divergence_aggregate_collection_state.rs` go green without an
  `#[ignore]`.

## Evidence and residual work

Completion evidence for the shipped storage increment must include:

1. validator accept tests for nested plain records and reject tests for hidden,
   recursive, mismatched, and unsupported record shapes;
2. exact emitted-helper tests and a real Verus proof over a representative
   nested record array;
3. independent exec/contract/body translation-validation proofs plus a
   dropped-field generated-comparator mutant that fails its direct Verus
   contract;
4. strict L3 build, receipt replay, and bound-source tamper rejection for a
   policy-free aggregate fixture, including ABI-layout/capacity fingerprint
   sensitivity and a downstream codegen-pinned Rust consumer that constructs
   the public records and executes the generated comparator; and
5. workspace formatting, lint, requirement-registry, and documentation-drift
   gates.

This increment does not complete static ownership, aggregate mutation through
named borrows, enum equality, full aggregate lifecycle body TV, affine
authority, atomic integration, or machine-operation refinement. The quantified
logical-index family above is specified and unimplemented; blockers #131 and
#132 own its two stages.
