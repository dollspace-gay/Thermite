# Structural-zero mutation equivalence

<!--
tier: 3-component
status: draft
governs: forge/src/mutation.rs
thesis-refs:
  - thermite-design.md §7
-->

## Summary

A narrow syntactic classification for one equivalent-mutant shape the shipped
Verus probe cannot ask about: an early-return zero mutant whose replacement value
is the function's own body. Five fixed-collection constructors carry that shape
and survive scoring today, which makes an exact contract read as weak. This doc
specifies the rule that removes them from the denominator and collects the known
shapes in a table that grows as new cases turn up. The general-prover extension
lives in `.design/forge/equivalent-mutants.md` and stays there.

## The incident (measured)

Commit `a6963959` extended `fn zero_value_for` in `forge/src/mutation.rs` to give
a fixed array a canonical zero. A record whose fields are all zero-able therefore
gained a zero as well, and every function returning such a record gained one
early-return zero mutant. Fifteen mutants appeared across the eight collection
modules. Ten die. Five survive, and each of those five computes the same value as
the original body, so no contract can kill it. The *Gotcha table* below lists
them.

The shipped probe declines them by construction: `fn scalar_obligation_type` in
`thermite-lower/src/lower.rs` returns `None` for a `Type::Named` return, so
`fn equivalence_proves_equal` in `forge/src/check.rs` yields
`EquivOutcome::Unsupported` and the survivor stays counted. The score `70/74`
does not say which four lived or why, so a real contract gap and a
provably-equivalent mutant read identically. That is an accuracy failure in the
same family as `goal.md` R-DEFER-9, pointing the other way: R-DEFER-9 keeps a
weak contract from looking strong, and this keeps an exact contract from looking
weak.

## Requirements

- **REQ-1 (fixed-array canonical zero — §7 step 4):** the early-return zero
  ladder gives `Type::Array { elem, len }` the exact-capacity repeat of the
  element's zero, so a record whose fields are all zero-able gets a field-zero
  literal and is scored instead of escaping through a `0/0` backstop.

- **REQ-2 (an equivalent mutant leaves the denominator — §7 step 4):** a survivor
  classified equivalent is removed from `scored` and from the survivor set,
  counted in a transparency field, and never recorded as the strengthening
  prompt. The `0/0` backstop still gates the case where classification empties
  the denominator.

- **REQ-3 (structural-zero equivalence rule):** a survivor of the family-1
  zero-value early return is classified `equivalent` when three clauses hold.

  - **(a) Origin.** The mutant is the zero-value early return produced by
    `fn early_return_value`, and the outcome for it is `Survived`.
  - **(b) Body shape.** The function's body is `Block { stmts: [], tail: Some(E) }`
    — one tail expression and no statements.
  - **(c) Structural equality.** `N(Z) == N(E)` under the AST `Expr`'s derived
    `PartialEq`, where `Z` is the mutant's replacement value and `N` is the
    normalizer below, and `N(Z)` lies inside the value grammar `V`.

  The normalizer `N` is a total recursive rewrite over `Expr` with a closed rule
  list:

  1. `Expr::IntLit { value, raw }` → `IntLit` on `value`; `raw` is discarded, so
     `0`, `0x0`, and `0u8` agree.
  2. `Expr::BoolLit(b)` → itself.
  3. `Expr::Path([name])` where `name` resolves to an `Item::Const(ConstItem)`
     among the threaded `adt_deps` → `IntLit` on that item's `value`.
  4. `Expr::Cast { expr, ty }` where `N(expr)` is an integer literal `v` and `ty`
     is an unsigned integer primitive whose range contains `v` → that literal.
  5. `Expr::ArrayRepeat { value, len }` → the rewritten element with the length
     resolved: `ArrayLen::Literal` contributes its `value`, `ArrayLen::Const(n)`
     the named `ConstItem`'s `value`. An unresolvable length declines the rule.
  6. `Expr::Tuple(es)` → the element-wise rewrite.
  7. `Expr::StructLit { path, fields }` → `path` verbatim and `fields` rewritten
     element-wise, then sorted by field name. Field order is source order in a
     body and declaration order in a synthesized zero, so the comparison is
     order-insensitive over equal name sets.
  8. Every other node has no rule and is left alone, which makes clause (c) fail.

  The value grammar `V` is: integer literals, boolean literals, `None`, array
  repeats over `V` with a resolved length, tuples over `V`, and struct literals
  whose field values are in `V`. `V` contains no call, no name reference, and no
  operator, so an expression in `V` denotes one value, computes it without an
  effect, and terminates. When `N(Z) == N(E)` with `N(Z)` in `V`, the mutant
  `{ return Z; E }` and the body `{ E }` compute the same result. The argument is
  syntactic: no solver participates, no query times out, and no case is
  undecidable. The `TVec` and `TString` empty-wrapper arms of the zero ladder
  carry a `Vec::new()` call, fall outside `V`, and never classify.

- **REQ-4 (the under-claim is the default):** a mutant failing any clause of
  REQ-3 stays a counted survivor. The rule adds no path that removes a mutant
  without a syntactic witness, so a contract gap keeps its survivor and keeps
  depressing the ratio. This is the polarity of the `EquivOutcome::NotProved` arm
  in `fn equivalence_proves_equal`, carried over.

- **REQ-5 (certificate effect, cache key, and pins):**
  - No certificate field is added, renamed, removed, or retyped.
    `contract_quality.mutants_killed` keeps the `"K/N"` string shape that
    `fn mutants_killed_string` renders; classification lowers `N` by one per
    classified mutant. `goal.md` R-SPEC-2 governs the field's shape, and the
    shape holds. The change is additive in the schema sense: every slot keeps its
    name, type, and presence, and only the value in an existing slot moves.
  - `Certificate::oracle_subset` in `forge/src/manifest.rs` does not carry
    `contract_quality`, so the existing oracle subset is untouched and no frozen
    `conformance/*.cert.json` golden moves on this account.
  - The transparency count reuses the existing `MutationScore::equivalent` field,
    so the struct gains no field. A classified mutant is never written to
    `survivor`.
  - Classification changes a verdict-bearing output for unchanged sources, so
    landing bumps `const CHECK_SCHEMA_VERSION` in `forge/src/cache.rs` (currently
    `8`) per `.design/forge/proof-cache.md`.
  - The aggregate `(killed, total)` pins move once more. The cause to state in
    the landing commit: five equivalent mutants leave the denominator.

    | Pin site | Module | Before | After |
    |---|---|---|---|
    | `forge/tests/fixed_collections.rs` | `ring.th` | `(64, 72)` | `(64, 71)` |
    | `forge/tests/fixed_collections.rs` | `vector.th` | `(46, 51)` | `(46, 50)` |
    | `forge/tests/fixed_collections.rs` | `direct_map.th` | `(54, 59)` | `(54, 58)` |
    | `forge/tests/fixed_collections.rs` | `open_map.th` | `(72, 81)` | `(72, 80)` |
    | `forge/tests/fixed_collections.rs` | `bitmap.th` | `(114, 121)` | unchanged |
    | `forge/tests/fixed_slab.rs` | `slab.th` | `(70, 74)` | `(70, 73)` |

    `forge/tests/fixed_freelist.rs`, `forge/tests/fixed_intrusive.rs`, and
    `forge/tests/synchronization_primitives.rs` carry the same aggregate-pin
    shape. No gotcha row covers a module they check today, so their pins hold;
    landing re-derives all of them from the tool so a sixth case does not pass
    unnoticed. The *Auditable metrics* figure in
    `.design/build/fixed-collections.md` moves from `742/794` to `742/789`.

## Acceptance criteria

- **AC-1:** each of the five equivalent rows in the *Gotcha table* classifies,
  and each module's aggregate total drops by one to the *After* column above.
  Checked by `forge/tests/fixed_collections.rs` and `forge/tests/fixed_slab.rs`.
- **AC-2 (the mechanism control):** `fn fixed_bitmap_empty` does not classify.
  Its zero has `capacity == 0`, which `ens result.capacity == FIXED_BITMAP_BITS`
  refutes, so the mutant dies and the bitmap pin `(114, 121)` holds.
- **AC-3 (the body-shape control):** `fn fixed_vec_set` does not classify. Its
  body has statements before the tail, so clause (b) declines, and the mutant
  dies against `ens result.slots[index] == value`. A unit assertion over the
  classifier pins the decline independently of the Verus verdict.
- **AC-4 (the under-claim default):** a body whose tail is a call, a field read,
  or a binary expression never classifies, even when its type's zero has the same
  outward shape. A unit case asserts the classifier declines and the survivor
  stays counted.
- **AC-5 (schema stability):** the `Certificate::oracle_subset` output and the
  frozen `conformance/*.cert.json` goldens are byte-identical across the landing,
  and `const CHECK_SCHEMA_VERSION` moved.

## Gotcha table

One row per known equivalent-mutant shape, seeded with the five measured cases
and two controls. New cases are added here before the normalizer grows a rule for
them, so the rule list stays closed and the collection stays the record.

| Shape | Function | Body | Its type's zero | Classification |
|---|---|---|---|---|
| All-zero record constructor | `fn fixed_ring_empty` (`ring.th`) | `{ [0; N], 0, 0 }` | identical | equivalent |
| All-zero record constructor | `fn fixed_vec_empty` (`vector.th`) | `{ [0; N], 0 }` | identical | equivalent |
| All-zero record constructor | `fn fixed_slab_empty` (`slab.th`) | `{ [false; N], [0; N], [0; N] }` | identical | equivalent |
| All-zero record constructor, parameter reaches only the contract | `fn fixed_direct_map_empty_for` (`direct_map.th`) | `{ [false; N], [0; N], [0; N], 0 }` | identical | equivalent |
| Const-folded zero | `fn fixed_open_map_empty` (`open_map.th`) | `{ [OPEN_MAP_EMPTY as u8; N], [0; N], [0; N] }` | identical after normalizer rules 3 and 4, since `const OPEN_MAP_EMPTY: usize = 0` | equivalent |
| Control: a field with a nonzero contract | `fn fixed_bitmap_empty` (`bitmap.th`) | `{ [0; N], FIXED_BITMAP_BITS }` | differs in `capacity` | survivor; Verus kills it via `ens result.capacity == FIXED_BITMAP_BITS` |
| Control: transition with a let-chain | `fn fixed_vec_set` (`vector.th`) | `let mut slots = vector.slots; slots[index] = value; { slots, vector.len }` | differs | survivor; clause (b) declines and Verus kills it via `ens result.slots[index] == value` |

`FixedVec64` shows both shapes at once. Two functions return it: the
constructor's mutant classifies and the transition's does not, because a
transition's postconditions relate the result to its input.

## Architecture

The classifier is a seam in `forge/src/mutation.rs`, consumed at the survivor
branch of `fn mutation_score` in `forge/src/check.rs` — the same site where
`fn equivalence_proves_equal` already routes a survivor and where a classified
mutant already skips the `scored` increment. Ordering: the syntactic rule runs
first, since it is a pure AST comparison over data the caller already holds
(`adt_deps` is threaded into `pub fn generate` today, and the const items travel
with it). A classified mutant needs no Verus run. Anything the rule declines
falls through to the shipped probe unchanged.

The rule owns no schema, no configuration, and no new prover path.

## Considered and deferred

Each of these was weighed against the five measured cases and left out. The list
records where the boundary was drawn.

- **A general equivalence prover.** The Verus probe in
  `.design/forge/equivalent-mutants.md` decides the scalar case by proof;
  extending it to record returns needs a non-scalar obligation renderer. The
  syntactic rule decides all five measured cases without one, and it cannot
  misclassify a real weakness.
- **A survivor taxonomy beyond `equivalent`.** One classification with one
  denominator effect.
- **Further certificate reshaping.** `mutants_killed` keeps its shape and no
  per-survivor record joins the certificate.
- **Per-item pins.** The aggregate `(killed, total)` pins stay aggregate.
- **Binding the generator's identity into the score.** Out.

## Verification

- `cargo test -p forge --lib mutation` — the classifier's unit oracles: the five
  gotcha shapes classify, the two controls decline (AC-2, AC-3), and the
  out-of-grammar bodies decline (AC-4). Expectations are hand-derived from REQ-3's
  clause list under `goal.md` R-CHAR-3.
- `cargo test -p forge --test fixed_collections --test fixed_slab` — the
  aggregate pins land on the *After* column (AC-1).
- `cargo test -p forge --test equivalent_mutants_conformance` — the shipped
  probe's exclude/keep polarity is unchanged by the added rule.
- `cargo test -p forge --lib manifest` — `fn golden_deterministic_subset_round_trips`
  over `conformance/sum.cert.json`, plus the golden-cert consumers under
  `forge/tests/`: no byte change (AC-5).

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (fixed-array canonical zero) | SHIPPED | Impl: `fn zero_value_for in forge/src/mutation.rs`, arm `Type::Array { elem, len } => Some(Expr::ArrayRepeat { value: Box::new(zero_value_for(elem)?), len: len.clone() })`, whose comment records why named-record elements are absent ("Thermite does not derive ambient Copy for them"). Non-test consumer: `fn early_return_value in forge/src/mutation.rs` (`if let Some(zero) = zero_value_for(&f.ret)`), reached from `pub fn generate in forge/src/mutation.rs` and `fn mutation_score in forge/src/check.rs`. Verification: `fn struct_zero_composes_fixed_array_and_scalar_fields in forge/src/mutation.rs` asserts the mutant `insert early \`return Bank { <field zeros> }\` at body head` is generated for a record with a fixed-array field; the corpus effect is the aggregate rise recorded in `.design/build/fixed-collections.md` from `732/779` to `742/794`. |
| REQ-2 (equivalent mutants leave the denominator) | SHIPPED | Impl: `pub struct MutationScore in forge/src/mutation.rs` carries `pub equivalent: usize` beside `scored`, documented as "`scored` is already net of them"; `fn kill_ratio` keeps the `scored == 0 ⟹ 0.0` backstop. Non-test consumer: `fn mutation_score in forge/src/check.rs`, at the survivor branch `equivalent += 1; continue;` — the `continue` is the denominator drop, and `survivor` is left unset for that mutant. The rendered value reaches the certificate through `fn mutants_killed_string in forge/src/mutation.rs` and `Certificate::with_mutation_score in forge/src/manifest.rs`. Verification: `forge/tests/equivalent_mutants_conformance.rs`. |
| REQ-3 (structural-zero equivalence rule) | NOT-STARTED | Open prereq blocker: *structural-zero classification* (see *Blocker* below). No classifier exists; the five survivors reach `fn equivalence_proves_equal in forge/src/check.rs`, which returns `EquivOutcome::Unsupported` because `fn scalar_obligation_type in thermite-lower/src/lower.rs` has no arm for `Type::Named`, so each stays a counted survivor. |
| REQ-4 (the under-claim is the default) | NOT-STARTED | Open prereq blocker: *structural-zero classification*. The polarity has no code to hold it, since REQ-3's classifier is unbuilt; the equivalent shipped polarity is the `EquivOutcome::NotProved` arm of `fn equivalence_proves_equal in forge/src/check.rs`. |
| REQ-5 (certificate effect, cache key, and pins) | NOT-STARTED | Open prereq blocker: *structural-zero classification*. `const CHECK_SCHEMA_VERSION in forge/src/cache.rs` is `8` and the aggregate pins in `forge/tests/fixed_collections.rs` and `forge/tests/fixed_slab.rs` still carry the *Before* column, because nothing yet removes a mutant from the denominator on this rule. |

## Blocker

**Structural-zero classification** — REQ-3, REQ-4, and REQ-5 are unbuilt. The
issue text is prepared but unfiled: this environment's GitHub token is denied
issue creation (`gh api -X POST repos/dollspace-gay/Thermite/issues` returns
`403 Resource not accessible by personal access token`) and `crosslink` is not
installed. The orchestrator files it and substitutes the number in the three
rows above. Scope: the normalizer and value grammar of REQ-3, the decline
polarity of REQ-4, and the schema-version bump plus aggregate-pin re-derivation
of REQ-5.
