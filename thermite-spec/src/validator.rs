//! The SpecTherm validator — the boundary API that walks a parsed
//! `thermite-syntax` program's contract positions and enforces §4.2's "locked
//! cage": a contract may use ONLY registered combinators (right name + arity +
//! arg-kinds), declared `spec fn` calls, and the built-in operators / literals /
//! paths the grammar already sanctions — nothing else.
//!
//! Governing design: `.design/spec/spectherm-combinators.md` (REQ-3/4/5).
//! Verified against the oracle at `tests/golden/combinators/` (accept.json /
//! reject.json), R-CHAR-3.
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-3 (validator accept rule) | SHIPPED | `pub fn validate` collects `spec fn` names then walks `Contract.req`/`ens`, `LoopNode.invs`/`dec`, `SpecFnItem.body`; accepts registered combinators (via `combinators::lookup`), declared spec-fn calls, and grammar built-ins. Every `accept.json` case validates clean (`tests/combinators_conformance.rs`). |
//! | REQ-4 (reject cases, structured `SpecError`) | SHIPPED | `enum SpecError` with `UnknownCombinator`/`WrongArity`/`WrongArgKind`/`ForbiddenCall`/`ExpressionTooDeep`; `validate` returns `Result<(), Vec<SpecError>>`, never panics. Every `reject.json` case yields the expected cause. |
//! | REQ-5 (bounded recursion — no overflow) | SHIPPED | a single `MAX_RECURSION_DEPTH` guard wraps EVERY recursive descent (`walk_expr`, closure bodies, match arms, index args, if/block tails) via `descend`; deep input yields `ExpressionTooDeep`, never an overflow (`validate_never_panics`). |
//! | REQ-6 (flat-closure-fragment rule — no anonymous nested quantifiers) | SHIPPED | `check_arg_kind`'s `Pred` arm sets `Validator::in_combinator_closure` for the whole closure-body descent (kept set through all nested sub-expressions/closures); while set, `walk_call` rejects any callee resolving via `combinators::lookup` with `SpecError::NestedCombinator`, while a declared `spec fn` call stays accepted. Consumer: `validate` → `walk_clause`/`walk_block` reach `walk_call`. Verification: `reject.json` `nested_combinator_in_closure` → `NestedCombinator`; `accept.json` `named_spec_fn_in_closure` → `Ok`; the flat corpus closures stay `Ok` (`tests/combinators_conformance.rs`). |
//!
//! ## Basis Stage 1b — the REAL ADT validator (`.design/basis/01-adts.md`)
//!
//! Stage 1b REPLACES the 1a `UnsupportedAdt` gate with real exhaustiveness +
//! well-formedness checking. The 3 ADT corpus programs validate clean; crafted
//! negatives reject with the precise structured error. Verified against the
//! oracle `conformance/adt-validate/cases.json` (R-CHAR-3) via
//! `tests/adt_validate.rs`. Lowering stays gated (Stage 1c, thermite-lower).
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-5 (exhaustiveness — `NonExhaustiveMatch`/`UnreachableArm`) | SHIPPED | declaration pre-pass `Validator::new` collects `enums` (name → variant order) + `variant_to_enum`; `check_match_exhaustiveness` (reached from both the caged `walk_expr_inner` `Match` arm AND the exec-body `scan_expr_for_loops` `Match` arm) infers the matched enum from arm patterns (`variant_pattern_name`), then emits `NonExhaustiveMatch { missing }` (declaration order), `UnreachableArm` (variant twice / arm after wildcard), `UnknownVariant` (undeclared variant in a pattern). Slice/`Option` matches are inert (no regression). Consumer: `validate`. Verification: `tests/adt_validate.rs` `non_exhaustive_match` → `missing:[Rect]`; `unreachable_redundant_arm` → `UnreachableArm`; `unknown_variant_pattern` → `UnknownVariant{Square}`; `shape`/`list_sum` accept. |
//! | REQ-6 (well-formed field/variant access + `is`) | SHIPPED | pre-pass collects `struct_fields` (every `struct`/struct-variant field). `check_field` (on `Expr::Field` + `Expr::StructLit` fields, both walks) → `UnknownField` for an undeclared field (inert when no struct declared). `check_variant_ref` (on `Expr::Is`, both walks) → `UnknownVariant` for an undeclared variant. Consumer: `validate`. Verification: `unknown_field` → `UnknownField{bogus}`; `unknown_variant_is` → `UnknownVariant{Triangle}`; `bank_account`/`shape` accept. |
//! | REQ-2 (variant names UpperCamelCase — `InvalidVariantCasing`) | SHIPPED | #66. The declaration pre-pass `Validator::new` rejects any `enum` variant whose first char is not `is_ascii_uppercase()` with `SpecError::InvalidVariantCasing { name, span }`, seeded into the error list BEFORE the body/contract walk. This is load-bearing for soundness: the parser disambiguates a single-segment arm pattern by first-letter case (uppercase → `Pattern::Enum`, lowercase → `Pattern::Binding`), so forbidding lowercase variants makes that split sound — a lowercase pattern ident is unambiguously a binding (no lowercase variant can exist), closing the #66 bypass where a lowercase variant masqueraded as a catch-all and masked a non-exhaustive `match`. Consumer: `validate`. Verification: `tests/divergence_adt_validate.rs` `divergence_lowercase_variant_bypasses_exhaustiveness` → `InvalidVariantCasing{foo}`, `divergence_lowercase_arm_masks_unhandled_variant` → `InvalidVariantCasing{tri}`; `exhaustiveness_intact_uppercase_nonexhaustive` confirms uppercase enums still get `NonExhaustiveMatch`. |
//! | REQ-7 (ADT predicates fit the cage — flat built-ins) | SHIPPED | `Expr::Match`/`Field`/`Is`/`Deref` are FLAT built-ins in `walk_expr_inner` — they recurse operands without setting `in_combinator_closure` and without resolving as combinators, so they are admitted unchanged inside a combinator predicate-closure body (the existing caged-flat walk). No recursive scheme exists yet to nest (forward-declared; schemes are Stage 2). Verification: the combinator cage tests (`tests/combinators_conformance.rs`) stay green. |
//! | 1a GATE (`SpecError::UnsupportedAdt`) | RETIRED for well-formed ADTs | the variant is RETAINED (downstream `forge`/`thermite-lower` reference it by name in comments and it screams for a future un-checkable ADT form) but has NO live emitter in 1b: `run`'s `Item::Struct` now cages the `inv` clause, `Item::Enum` is a no-op, and `walk_expr_inner`'s `Expr::StructLit`/`Expr::Is`/`Expr::Deref` arms validate-or-accept. A well-formed ADT no longer dies at the gate. |
//!
//! ## Basis Stage 2b — recursion-scheme recognition + the flat-step cage (`.design/basis/02-recursion-schemes.md`)
//!
//! Stage 2b EXTENDS the cage to recognize a recursion-scheme call
//! (`fold`/`map`/`for_all`/`exists`/`traverse`) as a named-composition leaf and
//! to reject a scheme/combinator NESTED in a scheme's step closure (the flat-step
//! cage). Verified against the oracle `conformance/adt-schemes/cases.json`
//! (R-CHAR-3) via `thermite-spec/tests/scheme_validate.rs`.
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (scheme set recognized as named primitives) | SHIPPED | `walk_call` resolves a callee `Path` against `schemes::lookup` FIRST; a registered scheme call is checked by `check_scheme` (total arity = `SchemeSig::total_arity`) and ACCEPTED at top level. Consumer: `validate` → `walk_call`. Verification: `scheme_validate.rs` `list_fold_validates` (the `fold`/`for_all` calls of `conformance/list_fold.th` validate clean); `unknown_scheme` → `UnknownCombinator` (an unregistered callee like `frobnicate` is neither scheme/combinator/spec-fn). |
//! | REQ-2 (flat step closure — arity + no nested scheme) | SHIPPED | `check_scheme` requires the trailing arg to be an `Expr::Closure` whose param count matches `SchemeSig::step_shape.arity()` (`SchemeStepShape` otherwise) and walks the step body in `in_scheme_step` mode; while set, `walk_call` rejects a registered scheme OR combinator callee with `SpecError::NestedScheme`. Consumer: `validate`. Verification: `scheme_validate.rs` `nested_scheme_in_step` → `NestedScheme` (a `fold` inside a `fold` step). |
//! | REQ-4 (cage bridge — named structural quantification; nested-scheme reject) | SHIPPED | a top-level `for_all`/`fold` scheme call is ACCEPTED as a named-composition leaf (mirroring the combinator-call accept, REQ-6); a scheme nested in a step / a combinator closure is `NestedScheme`. The scheme accept JOINS the combinator-call accept in `walk_call`; the caged-flat walk (`walk_expr_inner`, Stage 1 REQ-7) is unchanged. Verification: `scheme_validate.rs` `list_fold_validates` + `nested_scheme_in_step`. |
//! | REQ-9 (`SpecError` extension, no panics) | SHIPPED | `SpecError::{NestedScheme, SchemeWrongArity, SchemeStepShape}` are span-bearing variants; `validate` returns `Result<(), Vec<SpecError>>`, never panics. The structural-`dec` reject (REQ-5) reuses Stage 1's recursive-`spec fn` `dec` diagnostic (a generated `fold_<e>` is checked exactly as a hand-written recursive `spec fn` — `thermite-lower`). |
//!
//! ## REQ status — 04-collections.md (Basis Stage 4, issue #73)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-3 (capacity + operation contracts fit the §4.2 cage) | SHIPPED | the bounded-`Vec` operation contracts are FLAT built-ins: `v.len()` (already in `BUILTIN_METHODS`) and the no-OOB accessor `get` ADDED to `BUILTIN_METHODS` so `ens result == v.get(i)` (`conformance/vec_demo.th` `checked_get`) validates inside the cage; the capacity bound `v.len() < CAP` / `result.len() == v.len() + 1` (`push_one`) are flat comparisons over the `len` built-in. The caged-flat walk (`walk_expr_inner`'s `MethodCall` arm) is UNCHANGED — a Vec `len`/`get` is the same flat built-in as a slice `len`. `push`/`pop` are EXEC-only (never in a contract), so the cage does not admit them. Consumer: `validate` → `walk_expr_inner`. Verification: `thermite-lower/tests/collections_conformance.rs` (the contracts validate clean + real verus L3). |
//! | REQ-4 (element invariant via named `spec fn`) | NOT-STARTED | epic **#62** Stage 4 (v1.1). The element invariant (`forall_in(v, |e| inv(e))` as a named `spec fn`, preserved across `push`) reuses the existing named-`spec fn` accept path UNCHANGED — but the v1 corpus oracle (`conformance/vec_demo.th`) exercises only the capacity contract + no-OOB get; no `Vec<Account>` element-invariant program is in the corpus, so this REQ is deferred to a Stage-4 follow-up (the GROUNDED `all_elems_inv` form is design-confirmed feasible, not yet corpus-exercised). |
//! | REQ-12 (`last`/`contains` in `BUILTIN_METHODS`) | SHIPPED | #98 cluster C6. `last` and `contains` ADDED to `BUILTIN_METHODS` so an `ens result == v.last()` / `ens result == v.contains(x)` validates inside the §4.2 cage exactly as the no-OOB `get` does (the lowerer maps spec-position `v.last()` to the wrapper's `spec_get((len-1) as int)`; `contains`'s `exists`-meaning is PROVED by the exec `ens`'s linear-scan invariant, R-DEFER-9). `pop_last`/`insert`/`remove` stay EXEC-only (`&mut` mutators, never in a contract). The caged-flat walk (`walk_expr_inner`'s `MethodCall` allowlist arm) is UNCHANGED. Consumer: `pub fn validate` → `walk_expr_inner`. Verification: `forge/tests/vec_completeness_conformance.rs` (the C6 ops certify L3) + `cargo test -p thermite-spec`. |
//!
//! ## REQ status — 06-provenance-and-sinks.md (Basis Stage 6, issue #76 / blocker #77)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-8 (`#[sealed]` abstraction barrier — the door is the only mint) | SHIPPED | epic **#62** Stage 6 (#77). The declaration pre-pass `Validator::new` collects `sealed_structs` (every `Item::Struct` with `sealed == true`, alongside `struct_fields`). The new span-bearing `SpecError::SealedConstruction { name, span }` variant is emitted by `check_sealed_construction`, called from BOTH `Expr::StructLit` walk arms (the exec-body `scan_expr_for_loops` AND the caged `walk_expr_inner`): any struct literal whose `path` resolves to a `#[sealed]` struct is REJECTED, so a `#[sealed]` clean/capability type (`Sql`/`Public`/`Authorized`) is obtainable ONLY as a `#[boundary]` door's return (the door body is foreign/`external_body`, no in-language `StructLit`). Inert when no `#[sealed]` struct is declared (the non-IFC corpus is UNCHANGED). Consumer: `pub fn validate` → `forge::check::check_file` (a `ForgeError::Spec`, exit non-zero, no L3 cert). Verification: `thermite-spec/tests/sealed_validate.rs` (the launder rejects with `SealedConstruction{Sql}`; the safe doored path + a plain/no-sealed struct validate clean); `forge/tests/divergence_provenance.rs` (the 3 #77 bypass tests, UN-IGNORED, REJECT on all three axes); `forge/tests/provenance_conformance.rs` unchanged (safe paths L3, naive careless L0). |
//!
//! ## REQ status — 07-strings.md (Basis Stage 7 cluster C4, issue #94)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-7 (`push_byte`/`from_byte` — verified byte-builder; `fx alloc`) | SHIPPED | #94 cluster C4. `push_byte` ADDED to `BUILTIN_METHODS` (so a contract over a byte-builder result validates inside the §4.2 cage exactly as `concat`/`slice`); `from_byte` is an associated path-call `String::from_byte(b)` (an `Expr::Call`, no `BUILTIN_METHODS` entry needed). Both are constructing ops (`fx alloc`). Consumer: `pub fn validate` → `walk_expr_inner` (`BUILTIN_METHODS` allowlist). Verification: `forge/tests/string_format_conformance.rs` (the byte-builder certifies L3 / `effects: [alloc]` against real verus, the GROUNDED `4 verified, 0 errors` form). |
//! | REQ-8 (`u64_to_string` — decimal formatting, ROUND-TRIP contract; `fx alloc`) | SHIPPED | #94 cluster C4 (+ blocker #96 MSB-first display). `to_string` ADDED to `BUILTIN_METHODS` (the `n.to_string()` method spelling); the GENERATED round-trip spec fns `parse_be`/`parse_le`/`pow10` are seeded into `spec_fns` (`GENERATED_SPEC_FNS`) so a contract `ens parse_be(result) == n` validates inside the cage as a named `spec fn` call. The lowerer (`thermite-lower::lower`) emits the `u64_to_string` exec (LSB build + reverse to MSB-first) + `pow10`/`parse_le`/`parse_be`/`seq_reverse` spec + `lemma_parse_push` + the `parse_be(seq_reverse(s)) == parse_le(s)` bridge lemmas. Consumer: `pub fn validate` → `walk_call` (the `spec_fns` accept). Verification: `forge/tests/string_format_conformance.rs` (the MSB-first round-trip `parse_be(to_string(n)) == n` certifies L3, GROUNDED `17 verified, 0 errors`; an overclaim FAILS, non-vacuous, R-DEFER-9). |
//! | REQ-9 (`parse_u64` — `String`→`u64`, PARTIAL / handled-or-loud) | SHIPPED | #95 cluster C7. `GENERATED_SPEC_FNS` += `all_digits`/`is_digit` (alongside `parse_be`/`parse_le`/`pow10`) so `parse_u64`'s round-trip witness validates inside the §4.2 cage as named `spec fn` calls; the lowerer emits the bodies. Consumer: `pub fn validate` → `walk_call`. Verified: `forge/tests/option_result_conformance.rs` (real verus `5 verified, 0 errors`). |
//! | REQ-13 (`contains`/`starts_with`/`ends_with` — boolean substring predicates; `pure`) | SHIPPED | #102 cluster C5. `starts_with`/`ends_with` ADDED to `BUILTIN_METHODS` (so `ens result == occurs_at(s@, needle@, ..)` validates inside the §4.2 cage); `contains` was already present (C6) and is RECEIVER-TYPE-dispatched by the lowerer (a `String` receiver → `TString::contains` substring scan, a `Vec` receiver → `TVec::contains` membership — Rust inherent-method resolution, no clobber). `GENERATED_SPEC_FNS` += `occurs_at`/`contains_sub` so the cage admits the named predicate. Consumer: `pub fn validate` → `walk_expr_inner` (`BUILTIN_METHODS` allowlist) + `walk_call` (`spec_fns` accept). Verified: `forge/tests/string_search_conformance.rs` (the predicates certify L3 pure under real verus; a true AND a false case proved, a broken `starts_with` FAILS — non-vacuous). |
//! | REQ-14 (`find` — first occurrence → `Option<u64>`; `pure`) | SHIPPED | #102 cluster C5. `find` ADDED to `BUILTIN_METHODS`; its `ens` is the C7 spec-`match`-in-`ens` (`match result { Some(at) => occurs_at(..), None => !contains_sub(..) }`), admitted via the existing flat-`match` cage rule (01-adts REQ-7). `contains_sub`/`occurs_at` in `GENERATED_SPEC_FNS`. Consumer: `pub fn validate`. Verified: `forge/tests/string_search_conformance.rs` (real verus L3; the PINNED Some case proves `result is Some`, the always-`None` mutant FAILS — #101 trap avoided). |
//! | REQ-15 (`split` — split on a separator byte → `Vec<String>`; `fx alloc`) | SHIPPED | #102 cluster C5. `split` ADDED to `BUILTIN_METHODS`; `GENERATED_SPEC_FNS` += `count_sep`/`sep_free` so `ens result.len() == 1 + count_sep(s@, sep) && forall|k| sep_free(..)` validates inside the §4.2 cage as named `spec fn` calls (the lowerer emits the bodies + `lemma_count_push`). Consumer: `pub fn validate`. Verified: `forge/tests/string_search_conformance.rs` (the count-bound + sep-free floor certifies L3 alloc under real verus; a `split`-drop mutant FAILS — non-vacuous). |
//! | REQ-16 (`trim` — strip leading/trailing ASCII whitespace → `String`; `fx alloc`) | SHIPPED | #102 cluster C5. `trim` ADDED to `BUILTIN_METHODS`; `GENERATED_SPEC_FNS` += `is_space` so a contract may name the whitespace predicate; the grounded `ens result.len() <= s.len() && exists|lo,hi| result == s.subrange(lo,hi)` validates inside the cage. Consumer: `pub fn validate`. Verified: `forge/tests/string_search_conformance.rs` (the length floor + subrange content certifies L3 alloc under real verus). |
//!
//! ## REQ status — 09-option-result.md (Cluster C7, built-in Option/Result, issue #95)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-3 (built-in-variant registry + spec-`match`-in-`ens` payload projection) | SHIPPED | `Validator::new` SEEDS the built-in variants `Some`/`None`→enum `Option` and `Ok`/`Err`→enum `Result` into `enums`/`variant_to_enum` (order `[Some, None]`/`[Ok, Err]`), AFTER the user pre-pass (a user re-decl wins via `or_insert`). With them registered, `Some(v)`/`Ok(v)` construction is accepted (the exec-body walk recurses `Call`/`Path`, no `UnknownVariant`), `match` over them is exhaustiveness-checked (`check_match_exhaustiveness` now infers `Option`/`Result` — both arms or a `Wildcard` required), and `r is Some` (`check_variant_ref`) accepts. The spec-`match`-in-`ens` payload projection needs NO new cage rule — `walk_expr_inner`'s `Match` arm already admits a flat `match` as a built-in (01-adts REQ-7), so `match result { Some(v) => <flat pred>, None => true }` in an `ens` is an accepted flat predicate. `GENERATED_SPEC_FNS` += `all_digits`/`is_digit`. Consumer: `pub fn validate`. Verified: `forge/tests/option_result_conformance.rs` (AC-1/AC-2 L3 construct + payload `ens`; AC-3 the broken payload REJECTED, non-vacuous). |
//!
//! ## REQ status — 10-recursion-tuples.md (Cluster C9-A, plain-`fn` recursion, issue #108)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-2 (`dec` mandatory for a recursive `fn`; the self-call validator rule) | SHIPPED | `run`'s `Item::Fn` arm detects a DIRECT self-call (`block_calls_name(body, &f.name)` — a free call whose callee path's last segment is the fn's own name, walked over every statement/expression) and, when `f.dec.is_none() && !fn_is_diverge(f)`, pushes the span-bearing `SpecError::MissingDecreases { name, span }` — the surface mirror of the Verus rule "recursive function must have a decreases clause", so a non-terminating fn never reaches an L3 cert (R-DEFER-9). The `fx diverge` exemption is honored by `fn_is_diverge` (mirroring `thermite-lower`'s — the single §4.1 truth): a diverge fn recurses WITHOUT `dec` and is L1-capped (#88). Mutual recursion (REQ-6) is OUT of v1 — a pair that calls EACH OTHER (no direct self-call) is not flagged here; it reaches Verus and is rejected there (no false L3, no crash). Consumer: `pub fn validate` → `forge::check`. Verified: `forge/tests/recursion_conformance.rs::self_call_without_dec_is_structured_error` (the MissingDecreases reject) + `diverge_recursion_without_dec_is_l1`. |

use std::collections::{HashMap, HashSet};
use std::fmt;

use thermite_syntax::{
    Block, Clause, Expr, IndexArg, Item, MatchArm, Pattern, Program, Span, Stmt, VariantShape,
};

use crate::combinators::{self, ArgKind, CombinatorSig};
use crate::schemes::{self, SchemeSig};

/// The maximum recursive-descent nesting depth the validator will follow before
/// returning an `ExpressionTooDeep` diagnostic. A fixed constant for determinism
/// (R-CODE-5), mirroring `thermite-syntax`'s parser `MAX_RECURSION_DEPTH`.
///
/// This single bound guards EVERY recursive descent in the walk — nested
/// combinator/spec-fn arguments, `Binary`/`Index`/`Cast`/`Ref`/`Field`
/// operands, closure bodies, `Match` scrutinee + arm bodies, `If` branches, and
/// block statements/tails — so a pathological deeply-nested contract surfaces a
/// structured error rather than overflowing the native stack and aborting the
/// process (REQ-5; the #29/#31/#32 expr-only-guard lesson: do not leave any
/// recursive path unbounded).
const MAX_RECURSION_DEPTH: usize = 64;

/// The bounded set of built-in `MethodCall` names a CAGED position admits
/// (REQ-3(c): "the bounded built-in `MethodCall`s the grammar admits (e.g.
/// `xs.len()`)"). Any method name outside this set in a contract position is a
/// `ForbiddenCall` (REQ-4 (iv)) — the §4.2 cage is closed.
///
/// Set = `len` + the bounded-collection no-OOB accessor `get`
/// (`.design/basis/04-collections.md` REQ-3): `len` is the slice/Vec length used
/// by `sum.th`/`binary_search.th` (`haystack.len()`, `xs.len()`) and the Vec
/// capacity contract (`v.len() < CAP`, `result.len() == v.len() + 1`); `get` is
/// the verified `Vec` accessor whose result a contract names
/// (`ens result == v.get(i)` in `conformance/vec_demo.th`'s `checked_get`),
/// admitted as a FLAT built-in inside the §4.2 cage exactly as `len` is — the
/// lowerer maps the spec-position `v.get(i)` to the wrapper's `spec_get(i as int)`
/// (REQ-5). `push`/`pop` are NOT here: they are EXEC-only mutators (a fn body),
/// never named in a contract position, so the cage does not admit them. No other
/// built-in method is added — per REQ-1's frozen-set discipline and anti-goal
/// §11, the set grows only by design amendment from a corpus need, never
/// speculatively.
/// Stage 7 strings (`.design/basis/07-strings.md` REQ-3): the bounded `String`
/// operations a contract names are FLAT built-ins admitted inside the §4.2 cage
/// exactly as the slice/`Vec` `len`/`get` are. `byte_at` is the no-OOB byte
/// accessor whose result a contract names (`ens result == s.byte_at(0)` in
/// `conformance/string_demo.th`'s `first_byte`); `concat` is the bounded
/// constructing op whose length a contract names (`ens result.len() == a.len() +
/// b.len()` in `join` — the receiver `a.concat(b)`); `slice` is the bounded
/// substring (`ens result.len() == hi - lo`). The no-OOB safety is in the
/// LOWERED accessor's `req i < len` (the lowerer maps `s.byte_at(i)` to the
/// wrapper's `spec_byte_at(i as int)` whose exec mirror carries the precondition —
/// REQ-4); admitting the method here only opens the cage to name it, the bound is
/// proved by verus (the unguarded form FAILS, non-vacuity, R-DEFER-9). `push`/
/// the literal-materialization are EXEC-only (a fn body), never in a contract.
/// Cluster C4 strings (`.design/basis/07-strings.md` REQ-7/REQ-8, issue #94):
/// `push_byte` is the verified byte-builder's append op whose result a contract
/// names (the byte-builder length/element-frame `ens`) — admitted as a FLAT
/// built-in exactly as `concat`/`slice`; `to_string` is the `u64`→decimal-`String`
/// method whose round-trip a contract names (`ens parse_le(result) == n`, the
/// GROUNDED gold standard). Both are constructing ops (`fx alloc`); `from_byte` is
/// an associated path-call (`String::from_byte(b)`, an `Expr::Call`), so it needs
/// no `BUILTIN_METHODS` entry. The no-OOB / round-trip teeth are PROVED at L3 (a
/// wrong digit FAILS, R-DEFER-9); admitting the method here only opens the cage to
/// NAME it.
/// Cluster C6 collections (`.design/basis/04-collections.md` REQ-8/REQ-12, issue
/// #98): `last` is the bounded-`Vec` final-element accessor whose result a contract
/// names (`ens result == v.last()`), admitted as a FLAT built-in exactly as `get` —
/// the lowerer maps spec-position `v.last()` to the wrapper's `spec_get((len-1) as
/// int)`; `contains` is the element-membership predicate whose result a contract
/// names (`ens result == v.contains(x)`), admitted so the cage can NAME it (its
/// `exists`-meaning is PROVED by the exec `ens`'s linear-scan invariant, R-DEFER-9).
/// `pop_last`/`insert`/`remove` stay EXEC-only (`&mut` mutators, never in a
/// contract). No other built-in is added — REQ-1 frozen-set discipline.
/// Cluster C5 string search/transform (`.design/basis/07-strings.md` REQ-13..16,
/// issue #102): `starts_with`/`ends_with` are the boolean substring predicates whose
/// result a contract names (`ens result == occurs_at(s@, needle@, ..)`), `find` is
/// the first-occurrence search whose result is the built-in `Option<u64>` named via
/// the spec-`match`-in-`ens` (`ens match result { Some(at) => occurs_at(..), None =>
/// !contains_sub(..) }`), `split` is the `Vec<String>` splitter and `trim` the
/// whitespace stripper whose results a contract names (`ens result.len() == 1 +
/// count_sep(..)` / `ens exists|lo,hi| result == s.subrange(lo,hi)`). All admitted as
/// FLAT built-ins so the §4.2 cage can NAME them; their meanings are PROVED by the
/// emitted exec scans' loop invariants (R-DEFER-9). `contains` (already present from
/// C6) is SHARED by the string substring op AND the `Vec` membership op — the surface
/// NAME is one, but the lowerer RECEIVER-TYPE-dispatches it (a `String` receiver →
/// `TString::contains` substring scan; a `Vec` receiver → `TVec::contains` membership
/// scan), so no separate entry and no clobber (Rust inherent-method resolution keys on
/// the receiver type).
const BUILTIN_METHODS: &[&str] = &[
    "len",
    "get",
    "last",
    "contains",
    "byte_at",
    "concat",
    "slice",
    "push_byte",
    "to_string",
    "starts_with",
    "ends_with",
    "find",
    "split",
    "trim",
];

/// The GENERATED `spec fn` names the lowerer materializes for the C4 `u64`→`String`
/// round-trip (`.design/basis/07-strings.md` REQ-8, issue #94): `parse_be` (the
/// MSB-first / human-readable decimal value of a byte sequence — the DISPLAY-form
/// round-trip the surface contract names, blocker #96), `parse_le` (the LSB-first
/// construction-order value the bridge carries the proof through), and `pow10` (the
/// decimal weight). They are NOT user-declared — the lowerer
/// (`thermite-lower::lower`) emits them when the program uses `n.to_string()` — but a
/// contract names `parse_be` to state the round-trip (`ens parse_be(result) == n`),
/// so the validator must accept them as named `spec fn` calls inside the §4.2 cage
/// (seeded into `Validator::spec_fns` in `Validator::new`, alongside the
/// user-declared spec fns). This is the EXACT mechanism a user `spec fn` call uses —
/// these are reserved generated names, so a user cannot shadow them (a clash would be
/// a name collision the lowerer owns).
/// Cluster C7 (`.design/basis/09-option-result.md` REQ-3/REQ-5, issue #95) adds
/// the `parse_u64` round-trip spec fns `parse_be` (already present, the C4
/// big-endian value), `all_digits` (the all-bytes-are-`'0'..'9'` witness), and
/// `is_digit` (the per-byte `48 <= b <= 57` predicate) — so the GENERATED
/// `parse_u64`'s round-trip `ens match result { Some(v) => all_digits(s.data@) &&
/// parse_be(s.data@) == v, None => true }` validates inside the §4.2 cage as named
/// `spec fn` calls (the lowerer emits their bodies in `emit_parse_defs`).
/// Cluster C5 (`.design/basis/07-strings.md` REQ-13..16, issue #102) adds the string
/// search/transform spec fns the lowerer emits when a program uses the C5 ops, so a
/// contract NAMING them validates inside the §4.2 cage as named `spec fn` calls (the
/// lowerer emits their bodies in `emit_string_search_defs`): `occurs_at(s, needle, at)`
/// (needle occurs at byte offset `at` — a flat bounded `forall|k|`), `contains_sub(s,
/// needle)` (a flat bounded `exists|at| occurs_at(..)` — the `contains`/`find`
/// meaning), `count_sep(s, sep)` (the recursive separator count — `split`'s result
/// length), `sep_free(s, sep)` (no byte equals `sep` — each `split` piece), and
/// `is_space(b)` (the ASCII-whitespace predicate `trim` strips). All flat / bounded /
/// named per §4.2 (no anonymous nested quantifiers — `occurs_at`'s inner `forall|k|`
/// lives in the NAMED spec-fn body).
const GENERATED_SPEC_FNS: &[&str] = &[
    "parse_be",
    "parse_le",
    "pow10",
    "all_digits",
    "is_digit",
    "occurs_at",
    "contains_sub",
    "count_sep",
    "sep_free",
    "is_space",
];

/// `thermite-spec`'s own error enum (workspace.md REQ-3), born with this first
/// fallible function. Span-bearing (reusing `thermite_syntax::Span`) so
/// diagnostics are crisp (pillar 4); `Display`-able. The validator NEVER panics
/// (R-CODE-2 / R-APG-1) — every rejection is a variant here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecError {
    /// A call in a contract position whose callee is neither a registered
    /// combinator nor a declared `spec fn` — an arbitrary free-function call,
    /// forbidden by the §4.2 cage (REQ-4 (i)). `name` is the unresolved callee.
    UnknownCombinator { name: String, span: Span },
    /// A registered combinator called with the wrong number of arguments
    /// (REQ-4 (ii)).
    WrongArity {
        name: String,
        expected: usize,
        found: usize,
        span: Span,
    },
    /// A registered combinator whose positional argument has the wrong kind —
    /// e.g. a non-closure where a `Pred` is required (REQ-4 (iii)). `position`
    /// is 0-based.
    WrongArgKind {
        name: String,
        position: usize,
        expected: ArgKind,
        span: Span,
    },
    /// A construct the contract sublanguage forbids that nonetheless parsed —
    /// e.g. a `MethodCall` whose callee is not a grammar built-in, or a non-call
    /// callee shape (REQ-4 (iv)). Distinct from `UnknownCombinator` (a free
    /// `Expr::Call`) so the diagnostic names the construct precisely.
    ForbiddenCall { detail: String, span: Span },
    /// A registered combinator call appearing INSIDE another combinator's
    /// predicate-closure body — an anonymous nested quantifier (REQ-6). The
    /// flat-closure-fragment rule forbids it: a combinator's `Pred`-slot closure
    /// body is a FLAT predicate (comparisons, arithmetic, boolean/logical ops,
    /// field/index, casts/refs, literals/paths, bounded built-in method calls,
    /// `Match`/`If`, and NAMED `spec fn` calls) and MAY NOT compose another
    /// bounded quantifier. The sanctioned alternative is extracting a NAMED
    /// `spec fn` (each `dec`-measured and auditable). `name` is the nested
    /// combinator. Distinct from `UnknownCombinator` (a free call resolving to
    /// nothing) and `ForbiddenCall` (a generic forbidden construct) so the
    /// diagnostic can say "extract a named `spec fn`" (§4.2; issue #40).
    NestedCombinator { name: String, span: Span },
    /// A contract expression nested past `MAX_RECURSION_DEPTH` — surfaced as a
    /// structured diagnostic so external input can never overflow the stack
    /// (REQ-5).
    ExpressionTooDeep { limit: usize, span: Span },
    /// An ADT surface construct (`struct`/`enum` item, struct-literal
    /// construction, `is` discrimination, or a `Box` deref) reached the
    /// validator before the validator knows how to check it
    /// (`.design/basis/01-adts.md`). RETAINED from Stage 1a as the honest
    /// "handled-or-loud" SCREAM for ADT forms the validator still does not check
    /// — but Stage 1b NO LONGER fires it for a WELL-FORMED ADT: `struct`/`enum`
    /// items, `Expr::StructLit`, `Expr::Is`, and `Expr::Deref` are now
    /// validated (exhaustiveness REQ-5, well-formedness REQ-6) and ACCEPTED when
    /// well-formed. The variant stays in the enum so a future un-checkable ADT
    /// form has a structured refusal rather than a silent pass (the variant has
    /// no live emitter in 1b; `construct` names the unsupported surface form for
    /// a crisp diagnostic, §2.4).
    UnsupportedAdt { construct: &'static str, span: Span },
    /// A `match` over a DECLARED `enum` value whose arms do not cover every
    /// declared variant and is not closed by a `Wildcard` arm
    /// (`.design/basis/01-adts.md` REQ-5). `missing` is the set of uncovered
    /// variant names, in the enum's declaration order (deterministic, R-CODE-5).
    /// This is the COMPILE-TIME tooth of the handled-or-loud law (REQ-12): a
    /// modeled outcome (variant) is left neither handled nor explicitly screamed
    /// over — the validator rejects it BEFORE the program ships.
    NonExhaustiveMatch { missing: Vec<String>, span: Span },
    /// A `match` arm that can never be reached (`.design/basis/01-adts.md`
    /// REQ-5): a variant matched twice (the second arm is dead), or any arm
    /// after a catch-all `Wildcard` (the wildcard already absorbed it). A
    /// redundant arm is a program error, not a no-op.
    UnreachableArm { span: Span },
    /// Field access (`Expr::Field` `a.balance`, or a struct-literal field) to a
    /// name no declared `struct`/struct-variant declares
    /// (`.design/basis/01-adts.md` REQ-6). `name` is the unknown field.
    UnknownField { name: String, span: Span },
    /// A variant a declared `enum` does not declare, in a `match` pattern, an
    /// `is` discrimination (`r is Triangle`), or a struct-variant construction
    /// (`.design/basis/01-adts.md` REQ-6). `name` is the unknown variant.
    UnknownVariant { name: String, span: Span },
    /// A recursion-scheme call (`fold`/`map`/`for_all`/`exists`/`traverse`)
    /// NESTED inside another scheme's step closure
    /// (`.design/basis/02-recursion-schemes.md` REQ-2/REQ-4 — the flat-step
    /// cage). The step closure body of a scheme is a FLAT per-node expression
    /// (comparisons, arithmetic, field/index, named `spec fn` calls) and MAY NOT
    /// compose another recursion scheme or a bounded combinator — that would be
    /// an anonymous nested structural quantifier the §4.2 cage forbids. The
    /// sanctioned alternative is a NAMED `spec fn` (each `dec`-measured, in the
    /// audit surface). `name` is the nested scheme (or combinator). This is the
    /// scheme analogue of `NestedCombinator`, EXTENDING the cage's
    /// no-anonymous-nested rule to schemes (REQ-4).
    NestedScheme { name: String, span: Span },
    /// A recursion-scheme call with the wrong number of arguments
    /// (`.design/basis/02-recursion-schemes.md` REQ-1/REQ-2): a scheme call is
    /// `<scheme>(<scrutinee/seed args>, <step closure>)`, so `fold` takes 3 args
    /// (scrutinee + seed + step) and a predicate scheme (`for_all`/`map`/…) 2
    /// (scrutinee + step). `expected`/`found` are the total positional arities.
    SchemeWrongArity {
        name: String,
        expected: usize,
        found: usize,
        span: Span,
    },
    /// A recursion-scheme call whose trailing step argument is not a closure of
    /// the scheme's required per-node shape
    /// (`.design/basis/02-recursion-schemes.md` REQ-2): `fold`/`traverse` take a
    /// 2-param step `|x, acc|`, `map`/`for_all`/`exists` a 1-param step `|x|`.
    /// `expected`/`found` are the step closure's parameter counts; a non-closure
    /// in the step slot reports `found: 0` against the scheme's `expected`.
    SchemeStepShape {
        name: String,
        expected: usize,
        found: usize,
        span: Span,
    },
    /// An `enum` variant declared with a lowercase-initial name
    /// (`.design/basis/01-adts.md` REQ-2: "Variant names MUST be UpperCamelCase
    /// (uppercase-initial); the validator rejects a lowercase-initial variant
    /// declaration"). This is LOAD-BEARING for soundness, not style: the parser
    /// disambiguates a single-segment arm pattern by first-letter case
    /// (`Pattern::Enum` if uppercase-initial, `Pattern::Binding` otherwise).
    /// Forbidding lowercase variants makes that split SOUND — a lowercase ident
    /// in a pattern is *unambiguously* a binding, because no lowercase variant
    /// can exist, so a non-exhaustive `match` can never be silently masked by a
    /// variant-looking name being read as a catch-all binding (the #66 bypass).
    /// `name` is the offending variant. Rejected at the DECLARATION pre-pass,
    /// before any `match`/exhaustiveness check.
    InvalidVariantCasing { name: String, span: Span },
    /// An `Expr::StructLit` constructing a `#[sealed]` clean/capability type
    /// (`.design/basis/06-provenance-and-sinks.md` REQ-8). A `#[sealed]` struct
    /// is the ABSTRACTION BARRIER for an IFC clean type (`Sql`/`Public`/
    /// `Authorized`): it is obtainable ONLY as a `#[boundary]` door's return
    /// value (the door body is foreign/`external_body`, with no in-language
    /// `StructLit`), never minted directly. Minting one with a struct literal
    /// would LAUNDER a marked value into a clean type outside its door —
    /// defeating the IFC guarantee (the #77 SQLi/secret/capability bypass). This
    /// is the LOUDEST tooth of handled-or-loud, in IFC: the un-doored mark-change
    /// is a compile-time SCREAM, not a silent `L3`. `name` is the sealed struct.
    SealedConstruction { name: String, span: Span },
    /// A RECURSIVE exec `fn` (one whose body calls itself directly) that carries
    /// NO `dec` termination clause and is NOT `fx diverge`
    /// (`.design/basis/10-recursion-tuples.md` REQ-2, C9-A). Termination is proved
    /// by default (§4.1): a self-calling `fn` MUST supply a `dec <measure>` so
    /// Verus can prove the recursion terminates, UNLESS the fn declares `fx
    /// diverge` (the #88 exemption — a diverge fn is honestly non-terminating,
    /// L1-capped). This is the SURFACE-LEVEL mirror of the Verus rule "recursive
    /// function must have a decreases clause": Thermite reports it as its OWN
    /// span-bearing diagnostic so the user never reaches a raw Verus error, and a
    /// non-terminating fn can never be laundered to L3 (the no-proof-cheat
    /// guarantee, `goal.md` R-DEFER-9). `name` is the self-recursive fn.
    MissingDecreases { name: String, span: Span },
}

impl SpecError {
    /// The source span this diagnostic points at.
    pub fn span(&self) -> Span {
        match self {
            SpecError::UnknownCombinator { span, .. }
            | SpecError::WrongArity { span, .. }
            | SpecError::WrongArgKind { span, .. }
            | SpecError::ForbiddenCall { span, .. }
            | SpecError::NestedCombinator { span, .. }
            | SpecError::ExpressionTooDeep { span, .. }
            | SpecError::UnsupportedAdt { span, .. }
            | SpecError::NonExhaustiveMatch { span, .. }
            | SpecError::UnreachableArm { span, .. }
            | SpecError::UnknownField { span, .. }
            | SpecError::UnknownVariant { span, .. }
            | SpecError::NestedScheme { span, .. }
            | SpecError::SchemeWrongArity { span, .. }
            | SpecError::SchemeStepShape { span, .. }
            | SpecError::InvalidVariantCasing { span, .. }
            | SpecError::SealedConstruction { span, .. }
            | SpecError::MissingDecreases { span, .. } => *span,
        }
    }
}

impl fmt::Display for SpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpecError::UnknownCombinator { name, .. } => write!(
                f,
                "`{name}` is not a registered SpecTherm combinator or a declared `spec fn`; \
                 contracts admit only the frozen combinator set (§4.2)"
            ),
            SpecError::WrongArity {
                name,
                expected,
                found,
                ..
            } => write!(
                f,
                "combinator `{name}` expects {expected} argument(s), found {found}"
            ),
            SpecError::WrongArgKind {
                name,
                position,
                expected,
                ..
            } => write!(
                f,
                "combinator `{name}` argument {position} must be of kind {expected:?}"
            ),
            SpecError::ForbiddenCall { detail, .. } => {
                write!(f, "construct not permitted in a contract: {detail}")
            }
            SpecError::NestedCombinator { name, .. } => write!(
                f,
                "combinator `{name}` may not appear inside another combinator's \
                 predicate-closure body — that body must be a FLAT predicate (REQ-6); \
                 express nested quantification through a named `spec fn` instead"
            ),
            SpecError::ExpressionTooDeep { limit, .. } => write!(
                f,
                "contract expression nested deeper than the validator limit of {limit}"
            ),
            SpecError::UnsupportedAdt { construct, .. } => write!(
                f,
                "ADT construct `{construct}` is not yet checkable by the validator \
                 (`.design/basis/01-adts.md`)"
            ),
            SpecError::NonExhaustiveMatch { missing, .. } => write!(
                f,
                "non-exhaustive `match`: the variant(s) {missing:?} are neither handled by an arm \
                 nor covered by a `_` wildcard — every modeled outcome must be handled or an \
                 explicit catch must scream (REQ-5, §4.4)"
            ),
            SpecError::UnreachableArm { .. } => write!(
                f,
                "unreachable `match` arm: a variant matched twice, or an arm after a `_` wildcard \
                 that already absorbs it (REQ-5)"
            ),
            SpecError::UnknownField { name, .. } => write!(
                f,
                "`{name}` is not a field of any declared `struct` or struct-variant (REQ-6)"
            ),
            SpecError::UnknownVariant { name, .. } => write!(
                f,
                "`{name}` is not a declared variant of its `enum` (REQ-6)"
            ),
            SpecError::NestedScheme { name, .. } => write!(
                f,
                "recursion scheme `{name}` may not appear nested inside another \
                 scheme's step closure — that step body must be a FLAT per-node \
                 expression (REQ-2); express nested structural quantification \
                 through a named `spec fn` instead"
            ),
            SpecError::SchemeWrongArity {
                name,
                expected,
                found,
                ..
            } => write!(
                f,
                "recursion scheme `{name}` expects {expected} argument(s) \
                 (scrutinee/seed args + the trailing step closure), found {found}"
            ),
            SpecError::SchemeStepShape {
                name,
                expected,
                found,
                ..
            } => write!(
                f,
                "recursion scheme `{name}` step must be a closure with {expected} \
                 parameter(s) (the per-node step shape), found {found}"
            ),
            SpecError::InvalidVariantCasing { name, .. } => write!(
                f,
                "enum variant `{name}` must be UpperCamelCase (uppercase-initial) (REQ-2)"
            ),
            SpecError::SealedConstruction { name, .. } => write!(
                f,
                "`{name}` is a `#[sealed]` type and cannot be constructed with a struct literal — \
                 a sealed clean/capability type is obtainable ONLY through its `#[boundary]` door \
                 (the abstraction barrier; `.design/basis/06-provenance-and-sinks.md` REQ-8); \
                 minting it directly would launder a marked value past its door"
            ),
            SpecError::MissingDecreases { name, .. } => write!(
                f,
                "recursive function `{name}` must have a decreases clause — a `fn` that calls \
                 itself MUST supply a `dec <measure>` so termination is proved (§4.1; \
                 `.design/basis/10-recursion-tuples.md` REQ-2), UNLESS it declares `fx diverge`"
            ),
        }
    }
}

impl std::error::Error for SpecError {}

/// Validate every contract position of a parsed program against the SpecTherm
/// cage (REQ-3). Returns `Ok(())` if every contract expression is accepted, else
/// `Err` with one `SpecError` per violation (accumulated, not first-stop, for
/// crisp feedback, §2.4). NEVER panics (REQ-4/REQ-5).
///
/// This is `thermite-spec`'s boundary API: the validator is the registry's first
/// production consumer (AC-5, via `combinators::lookup`), and is the gate
/// `thermite-lower` (#4) and `forge` (#6) call before lowering / the vacuity
/// battery.
pub fn validate(program: &Program) -> Result<(), Vec<SpecError>> {
    let mut v = Validator::new(program);
    v.run(program);
    if v.errors.is_empty() {
        Ok(())
    } else {
        Err(v.errors)
    }
}

/// The walk state: the declared `spec fn` name set, the current recursion depth,
/// the accumulated diagnostics, and the "caged-flat" mode flag (REQ-6).
struct Validator {
    spec_fns: HashSet<String>,
    /// REQ-5: each declared `enum`'s variant names, in declaration order
    /// (collected from `Item::Enum` in the pre-pass). Keyed by enum name. The
    /// exhaustiveness check reads this to compute the missing-variant set; the
    /// declaration order makes that set deterministic (R-CODE-5).
    enums: HashMap<String, Vec<String>>,
    /// REQ-5/REQ-6: reverse index variant-name → owning-enum-name, built from
    /// `enums`. A `match` arm / `is` test / pattern naming a variant resolves
    /// the matched enum through this map; a name absent here (in a context
    /// already identified as a declared-enum match/`is`) is `UnknownVariant`.
    variant_to_enum: HashMap<String, String>,
    /// REQ-6: every field name declared by any `struct` or struct-variant
    /// (`VariantShape::Struct`). The AST is untyped (OQ-3: no type resolution),
    /// so field well-formedness is the shallow, mechanically-decidable check the
    /// design admits — an accessed field must be declared SOMEWHERE; a name no
    /// struct/struct-variant declares is `UnknownField`.
    struct_fields: HashSet<String>,
    /// REQ-8 (`.design/basis/06-provenance-and-sinks.md`): the names of every
    /// `#[sealed]` `struct` (collected from `Item::Struct` with `sealed == true`
    /// in the pre-pass, alongside `struct_fields`). The abstraction-barrier rule
    /// REJECTS any `Expr::StructLit` whose `path` resolves to a name in this set
    /// (`SealedConstruction`) — a sealed clean/capability type is door-only-
    /// mintable. Inert when no `#[sealed]` struct is declared (the non-IFC corpus
    /// is UNCHANGED), exactly like `struct_fields`.
    sealed_structs: HashSet<String>,
    depth: usize,
    errors: Vec<SpecError>,
    /// REQ-6 flat-closure-fragment mode. Set ONCE on entry to a combinator's
    /// `Pred`-slot closure body and kept set for ALL nested sub-expressions and
    /// nested closures within it. While set, a call resolving to a registered
    /// combinator (`combinators::lookup(name).is_some()`) is REJECTED with
    /// `NestedCombinator` (an anonymous nested quantifier); a NAMED `spec fn`
    /// call stays accepted (named composition). In a top-level contract position
    /// (flag clear) a combinator call is accepted as before (REQ-3 (a)).
    in_combinator_closure: bool,
    /// REQ-2/REQ-4 flat-scheme-step mode (`.design/basis/02-recursion-schemes.md`).
    /// Set ONCE on entry to a recursion scheme's step closure body and kept set
    /// for ALL nested sub-expressions within it. While set, a call resolving to a
    /// registered scheme (`schemes::lookup`) OR a registered combinator
    /// (`combinators::lookup`) is REJECTED with `NestedScheme` — the step body is
    /// a FLAT per-node expression and may not compose another structural
    /// quantifier (a named `spec fn` call stays accepted). This EXTENDS the
    /// existing combinator-closure cage (the `in_combinator_closure` rule) to
    /// scheme steps, exactly as the design's REQ-4 mandates.
    in_scheme_step: bool,
}

impl Validator {
    fn new(program: &Program) -> Self {
        // Collect every declared `spec fn` name first so a forward reference in
        // a contract (`ens result == sz(xs)` before `spec fn sz` is seen) still
        // resolves (REQ-3 (b)).
        let mut spec_fns: HashSet<String> = program
            .items
            .iter()
            .filter_map(|item| match item {
                Item::SpecFn(s) => Some(s.name.clone()),
                Item::Fn(_) => None,
                // A `struct`/`enum` item declares no `spec fn` name
                // (`.design/basis/01-adts.md`). The ADT declarations are
                // collected separately below.
                Item::Struct(_) | Item::Enum(_) => None,
            })
            .collect();
        // Cluster C4 strings (`.design/basis/07-strings.md` REQ-8, issue #94): seed
        // the GENERATED round-trip spec fns (`parse_le`/`pow10`) so a contract
        // `ens parse_le(result) == n` validates inside the §4.2 cage as a named
        // `spec fn` call (the lowerer materializes their bodies). These are reserved
        // names the lowerer owns — accepted exactly as a user-declared spec fn.
        for name in GENERATED_SPEC_FNS {
            spec_fns.insert((*name).to_string());
        }

        // The ADT DECLARATION PRE-PASS (`.design/basis/01-adts.md` REQ-5/REQ-6;
        // mirrors the spec-fn-name collection above). A program references types
        // across items in any order (`fn f(s: Shape)` may precede `enum Shape`),
        // so the body/contract walk must see EVERY declared type before walking
        // any body — order-independent, like the spec-fn resolution.
        let mut enums: HashMap<String, Vec<String>> = HashMap::new();
        let mut variant_to_enum: HashMap<String, String> = HashMap::new();
        let mut struct_fields: HashSet<String> = HashSet::new();
        // REQ-8 (`.design/basis/06-provenance-and-sinks.md`): the `#[sealed]`
        // clean/capability struct names — the abstraction barrier the
        // `Expr::StructLit` walk keys off to REJECT a direct mint. Collected in
        // the SAME pre-pass as `struct_fields` so a forward reference (`fn
        // f() { Sql { … } }` before `#[sealed] struct Sql`) is seen.
        let mut sealed_structs: HashSet<String> = HashSet::new();
        // `.design/basis/01-adts.md` REQ-2: every `enum` variant name MUST be
        // UpperCamelCase (uppercase-initial). A lowercase-initial variant is
        // rejected HERE, at the declaration pre-pass, BEFORE any
        // match/exhaustiveness check — this is the cause of the #66 bypass: the
        // parser disambiguates a single-segment arm pattern by first-letter case
        // (uppercase → `Pattern::Enum`, lowercase → `Pattern::Binding`), so a
        // lowercase variant in a `match` arm masquerades as a catch-all binding
        // and a non-exhaustive match is silently accepted. Forbidding lowercase
        // variants at the declaration makes that case-based split SOUND. These
        // casing diagnostics SEED the validator's error list so a lowercase-
        // variant program never reaches the (now-sound) body/contract walk.
        let mut casing_errors: Vec<SpecError> = Vec::new();
        for item in &program.items {
            match item {
                Item::Enum(e) => {
                    let mut variant_names = Vec::with_capacity(e.variants.len());
                    for variant in &e.variants {
                        // A variant name is uppercase-initial iff its first char
                        // is `is_ascii_uppercase()`. An empty name (a parser
                        // edge) is treated as non-uppercase → rejected.
                        if !variant
                            .name
                            .chars()
                            .next()
                            .is_some_and(|c| c.is_ascii_uppercase())
                        {
                            casing_errors.push(SpecError::InvalidVariantCasing {
                                name: variant.name.clone(),
                                span: e.span,
                            });
                        }
                        variant_names.push(variant.name.clone());
                        // Last writer wins on a duplicated variant name across
                        // enums; the validator's job here is well-formedness of
                        // ACCESS, not enum-declaration uniqueness (a separate
                        // concern not in this REQ). A struct-shaped variant's
                        // fields join the struct field set (REQ-6: `Field`
                        // access is checked against struct AND struct-variant
                        // fields).
                        variant_to_enum.insert(variant.name.clone(), e.name.clone());
                        if let VariantShape::Struct(fields) = &variant.shape {
                            for field in fields {
                                struct_fields.insert(field.name.clone());
                            }
                        }
                    }
                    enums.insert(e.name.clone(), variant_names);
                }
                Item::Struct(s) => {
                    for field in &s.fields {
                        struct_fields.insert(field.name.clone());
                    }
                    // REQ-8: a `#[sealed]` struct joins the abstraction-barrier
                    // set — its name will REJECT any `StructLit` mint.
                    if s.sealed {
                        sealed_structs.insert(s.name.clone());
                    }
                }
                Item::Fn(_) | Item::SpecFn(_) => {}
            }
        }

        // Cluster C7 (`.design/basis/09-option-result.md` REQ-1/REQ-2/REQ-3, issue
        // #95): SEED the BUILT-IN variants `Some`/`None` (enum `Option`) and
        // `Ok`/`Err` (enum `Result`) into the SAME `enums`/`variant_to_enum`
        // registry the user-`Item::Enum` pre-pass fills. This is the ONE validator
        // change C7 needs: with the built-in variants registered, construction
        // (`Some(v)`/`Ok(v)` reach the exec-body walk's `Call`/`Path` recursion, no
        // longer an `UnknownVariant`), `match` arms over them (`check_match_-
        // exhaustiveness` now infers `Option`/`Result` and requires BOTH arms or a
        // wildcard), and `is` discrimination (`r is Some` via `check_variant_ref`)
        // are ALL ACCEPTED — exactly as a user enum (01-adts REQ-5/REQ-6). The
        // declaration ORDER pins the exhaustiveness `missing` set: `Option` is
        // `[Some, None]`, `Result` is `[Ok, Err]`. The spec-`match`-in-`ens` payload
        // projection needs NO new cage rule — `walk_expr_inner`'s `Match` arm
        // already admits a flat `match` as a built-in (01-adts REQ-7), so a
        // `match result { Some(v) => <flat pred>, None => true }` in a contract is
        // already an accepted flat predicate once the variants are registered. A
        // user `enum` named `Option`/`Result` (or a variant `Some`/`Ok`/…) is a
        // re-declaration; last-writer-wins matches the existing duplicate-variant
        // policy (the built-ins are seeded FIRST, so a user re-decl overrides — the
        // user's intent wins, no silent shadow of a user type).
        for (enum_name, variant_names) in [("Option", ["Some", "None"]), ("Result", ["Ok", "Err"])]
        {
            enums
                .entry(enum_name.to_string())
                .or_insert_with(|| variant_names.iter().map(|v| v.to_string()).collect());
            for variant in variant_names {
                variant_to_enum
                    .entry(variant.to_string())
                    .or_insert_with(|| enum_name.to_string());
            }
        }

        Validator {
            spec_fns,
            enums,
            variant_to_enum,
            struct_fields,
            sealed_structs,
            depth: 0,
            // REQ-2: lowercase-variant casing diagnostics from the pre-pass seed
            // the error list, so a lowercase-variant `enum` is rejected at the
            // declaration BEFORE the (now-sound) match/exhaustiveness walk runs.
            errors: casing_errors,
            in_combinator_closure: false,
            in_scheme_step: false,
        }
    }

    /// Walk every contract position of every item.
    fn run(&mut self, program: &Program) {
        for item in &program.items {
            match item {
                Item::Fn(f) => {
                    self.walk_clause(&f.contract.req);
                    for clause in &f.contract.ens {
                        self.walk_clause(clause);
                    }
                    // REQ-3: a `fn` BODY is executable surface code, NOT a
                    // contract position. We traverse it STRUCTURALLY only — to
                    // find nested `LoopNode`s and cage each loop's `invs`/`dec`
                    // (the only contract positions inside a body). The body's
                    // other expressions (`return Some(mid)`, `haystack[mid]`,
                    // assignments, …) are surface code and are NOT cage-checked.
                    // A boundary fn (ffi-boundary.md REQ-2) has `body: None` — the
                    // body is foreign, so there are no in-language loops to scan
                    // for caged `inv`/`dec` clauses. Its `req`/`ens` (walked above)
                    // are still fully caged. An in-language fn's body is scanned
                    // structurally as before.
                    if let Some(body) = &f.body {
                        self.scan_block_for_loops(body, f.span);
                        // C9-A (`.design/basis/10-recursion-tuples.md` REQ-2): a
                        // RECURSIVE exec `fn` — one whose body calls itself directly
                        // — MUST carry a `dec` termination measure so Verus can prove
                        // the recursion terminates (§4.1 "Termination is proved by
                        // default"), UNLESS it declares `fx diverge` (the #88
                        // exemption — a diverge fn is honestly non-terminating,
                        // L1-capped). A self-call WITHOUT `dec` AND not `fx diverge`
                        // is a structured `MissingDecreases` — the surface mirror of
                        // the Verus rule "recursive function must have a decreases
                        // clause", so a non-terminating fn never reaches an L3 cert
                        // (R-DEFER-9). Mutual recursion (REQ-6) is OUT of v1: a pair
                        // that calls EACH OTHER (neither calls itself directly) is not
                        // a direct self-call, so it is not flagged here — it reaches
                        // Verus and is rejected there (no false L3, no crash).
                        if f.dec.is_none() && !fn_is_diverge(f) && block_calls_name(body, &f.name) {
                            self.errors.push(SpecError::MissingDecreases {
                                name: f.name.clone(),
                                span: f.span,
                            });
                        }
                    }
                }
                Item::SpecFn(s) => {
                    // A `spec fn` body is itself a contract-position expression
                    // tree (REQ-3) — fully caged; its `dec` measure is a clause.
                    self.walk_clause(&s.dec);
                    self.walk_block(&s.body, s.span);
                }
                // Basis Stage 1b (`.design/basis/01-adts.md` REQ-5/REQ-6): the
                // `struct`/`enum` declarations were collected in the pre-pass
                // (`Validator::new`). A `struct`'s type-invariant `inv` clause is
                // a CONTRACT POSITION (REQ-1: Verus enforces it at construction /
                // use) — it is fully caged here exactly like a `req`/`ens`,
                // including its `Field` access well-formedness (REQ-6). An `enum`
                // item carries no contract position of its own; its variant set
                // (collected above) drives the exhaustiveness/`is` checks at the
                // `match`/`is` sites. The 1a `UnsupportedAdt` gate is GONE: a
                // well-formed ADT now validates.
                Item::Struct(s) => {
                    if let Some(inv) = &s.inv {
                        self.walk_clause(inv);
                    }
                }
                Item::Enum(_) => {}
            }
        }
    }

    /// Run `inner` one recursion level deeper, returning `false` (and recording
    /// an `ExpressionTooDeep` at `span`) if the limit is hit. The SINGLE shared
    /// guard for every recursive descent (REQ-5). `span` is the enclosing
    /// clause/item span (the AST does not carry per-`Expr` spans).
    fn descend(&mut self, span: Span, inner: impl FnOnce(&mut Self)) {
        if self.depth >= MAX_RECURSION_DEPTH {
            self.errors.push(SpecError::ExpressionTooDeep {
                limit: MAX_RECURSION_DEPTH,
                span,
            });
            return;
        }
        self.depth += 1;
        inner(self);
        self.depth -= 1;
    }

    /// Walk a contract clause (`req`/`ens`/`inv`/`dec`): its expression must be
    /// accepted by the cage rule. The clause span anchors any diagnostic.
    fn walk_clause(&mut self, clause: &Clause) {
        let span = clause.span;
        self.walk_expr(&clause.expr, span);
    }

    /// STRUCTURAL traversal of a (non-caged) `fn` body block (REQ-3): descend
    /// through statements / nested blocks / `if` / `loop` ONLY to FIND nested
    /// `LoopNode`s and cage each loop's `invs`/`dec` (recursively, for loops
    /// nested in loops). The block's own expressions — calls like `Some(mid)`,
    /// `return None`, assignments, `haystack[mid]` — are executable surface code
    /// and are NOT cage-checked here. This is the counterpart to the caged
    /// `walk_block` (used for `spec fn` bodies and caged sub-expressions): same
    /// shape walk, but it cage-checks NOTHING except the loop contract clauses it
    /// discovers.
    fn scan_block_for_loops(&mut self, block: &Block, span: Span) {
        for stmt in &block.stmts {
            self.scan_stmt_for_loops(stmt, span);
        }
        if let Some(tail) = &block.tail {
            self.scan_expr_for_loops(tail, span);
        }
    }

    /// STRUCTURAL traversal of a `fn`-body statement: cage the `invs`/`dec` of
    /// any nested loop (the only contract positions in a body) and keep
    /// descending through control flow to find deeper loops. Surface expressions
    /// are descended into ONLY to reach nested loops (e.g. a `loop` inside an
    /// `if` block), never cage-checked.
    fn scan_stmt_for_loops(&mut self, stmt: &Stmt, span: Span) {
        match stmt {
            Stmt::Loop(loop_node) => {
                // The loop's `invs`/`dec` ARE contract positions — cage them.
                for inv in &loop_node.invs {
                    self.walk_clause(inv);
                }
                self.walk_clause(&loop_node.dec);
                // The loop BODY is still executable surface code: scan it
                // structurally for further nested loops, do not cage it.
                self.scan_block_for_loops(&loop_node.body, loop_node.span);
            }
            Stmt::Let { init, .. } => self.scan_expr_for_loops(init, span),
            Stmt::Assign { target, value } => {
                self.scan_expr_for_loops(target, span);
                self.scan_expr_for_loops(value, span);
            }
            Stmt::Return(Some(e)) | Stmt::Expr(e) => self.scan_expr_for_loops(e, span),
            Stmt::Return(None) => {}
            Stmt::If { cond, then, else_ } => {
                self.scan_expr_for_loops(cond, span);
                self.scan_block_for_loops(then, span);
                if let Some(else_block) = else_ {
                    self.scan_block_for_loops(else_block, span);
                }
            }
            // break/continue carry no sub-expression and no nested loop (#93):
            // nothing to scan or cage (the layer-neutral leaf value).
            Stmt::Break | Stmt::Continue => {}
        }
    }

    /// STRUCTURAL traversal of a `fn`-body expression. It descends to find
    /// nested `loop`s (caging each loop's `invs`/`dec`) AND — Basis Stage 1b —
    /// applies the ADT WELL-FORMEDNESS checks (REQ-5 exhaustiveness, REQ-6
    /// field/variant access) to every ADT node, because the validator rejecting
    /// a non-exhaustive `match` is the COMPILE-TIME tooth (REQ-12) and a `match`
    /// over an enum lives in `fn`-body (exec) position, not a contract position.
    /// These ADT checks are NOT cage checks: the body's combinator/spec-fn
    /// resolution is still NOT performed here (a body `Some(mid)` call stays
    /// surface code). The two concerns are orthogonal — the cage gates contract
    /// positions; the ADT well-formedness gates every modeled-outcome site.
    /// `span` is the enclosing `fn`/loop span (the AST carries no per-`Expr`
    /// span). When no ADT is declared, every ADT check is inert, so the existing
    /// non-ADT corpus body walk (`binary_search.th`) is UNCHANGED.
    fn scan_expr_for_loops(&mut self, expr: &Expr, span: Span) {
        match expr {
            Expr::If { cond, then, else_ } => {
                self.scan_expr_for_loops(cond, span);
                self.scan_block_for_loops(then, span);
                self.scan_block_for_loops(else_, span);
            }
            Expr::Match { scrutinee, arms } => {
                self.scan_expr_for_loops(scrutinee, span);
                // REQ-5: a `match` over a declared enum is exhaustiveness-checked
                // even in exec position (the reject fixtures put the `match` in a
                // `fn` body). A slice/Option `match` is inert (see the helper).
                self.check_match_exhaustiveness(arms, span);
                for MatchArm { body, .. } in arms {
                    self.scan_expr_for_loops(body, span);
                }
            }
            Expr::Call { args, .. } => {
                for arg in args {
                    self.scan_expr_for_loops(arg, span);
                }
            }
            Expr::MethodCall { receiver, args, .. } => {
                self.scan_expr_for_loops(receiver, span);
                for arg in args {
                    self.scan_expr_for_loops(arg, span);
                }
            }
            // REQ-6: field access well-formedness applies in exec position too.
            Expr::Field { receiver, name } => {
                self.check_field(name, span);
                self.scan_expr_for_loops(receiver, span);
            }
            Expr::Binary { lhs, rhs, .. } => {
                self.scan_expr_for_loops(lhs, span);
                self.scan_expr_for_loops(rhs, span);
            }
            Expr::Index { base, index } => {
                self.scan_expr_for_loops(base, span);
                match index {
                    IndexArg::Single(e) | IndexArg::RangeTo(e) | IndexArg::RangeFrom(e) => {
                        self.scan_expr_for_loops(e, span)
                    }
                    IndexArg::Range(lo, hi) => {
                        self.scan_expr_for_loops(lo, span);
                        self.scan_expr_for_loops(hi, span);
                    }
                }
            }
            Expr::Cast { expr: inner, .. } | Expr::Ref { expr: inner, .. } => {
                self.scan_expr_for_loops(inner, span)
            }
            Expr::Closure { body, .. } => self.scan_expr_for_loops(body, span),
            // REQ-6: a struct / struct-variant construction's field names must be
            // declared; the field VALUES are descended for nested loops/ADTs.
            // REQ-8: minting a `#[sealed]` clean type with a literal is the #77
            // door-bypass — REJECTED here (the exec-body walk) so a laundering
            // `query(Sql { stmt: input.raw })` screams at validation.
            Expr::StructLit { path, fields } => {
                self.check_sealed_construction(path, span);
                for (field_name, value) in fields {
                    self.check_field(field_name, span);
                    self.scan_expr_for_loops(value, span);
                }
            }
            // REQ-6: `is` discrimination well-formedness applies in exec position.
            Expr::Is { scrutinee, variant } => {
                self.check_variant_ref(variant, span);
                self.scan_expr_for_loops(scrutinee, span);
            }
            Expr::Deref(inner) => self.scan_expr_for_loops(inner, span),
            // The prefix `!` (#92): descend into the operand for nested loops/ADTs.
            Expr::Unary { expr, .. } => self.scan_expr_for_loops(expr, span),
            // Leaves — no nested loop / ADT node possible. A string literal
            // (`.design/basis/07-strings.md` REQ-1) is a value-carrying leaf, like
            // an int/bool literal — no sub-expression to descend.
            Expr::IntLit { .. } | Expr::BoolLit(_) | Expr::Path(_) | Expr::StrLit(_) => {}
        }
    }

    /// Walk a CAGED block (a `spec fn` body, or a block nested inside a caged
    /// expression such as an `if`'s arm): every statement expression and the
    /// tail expression IS a contract-position expression and is cage-checked.
    /// Any `loop`/`while` it contains carries its own `invs`/`dec` clauses.
    fn walk_block(&mut self, block: &Block, span: Span) {
        self.descend(span, |s| {
            for stmt in &block.stmts {
                s.walk_stmt(stmt, span);
            }
            if let Some(tail) = &block.tail {
                s.walk_expr(tail, span);
            }
        });
    }

    /// Walk a statement, descending into nested loops (which carry their own
    /// `invs`/`dec` contract clauses) and the expressions they hold.
    fn walk_stmt(&mut self, stmt: &Stmt, span: Span) {
        match stmt {
            Stmt::Loop(loop_node) => {
                for inv in &loop_node.invs {
                    self.walk_clause(inv);
                }
                self.walk_clause(&loop_node.dec);
                self.walk_block(&loop_node.body, loop_node.span);
            }
            Stmt::Let { init, .. } => self.walk_expr(init, span),
            Stmt::Assign { target, value } => {
                self.walk_expr(target, span);
                self.walk_expr(value, span);
            }
            Stmt::Return(Some(e)) | Stmt::Expr(e) => self.walk_expr(e, span),
            Stmt::Return(None) => {}
            Stmt::If { cond, then, else_ } => {
                self.walk_expr(cond, span);
                self.walk_block(then, span);
                if let Some(else_block) = else_ {
                    self.walk_block(else_block, span);
                }
            }
            // break/continue carry no sub-expression (#93): no ADT node to
            // well-formedness-check (the layer-neutral leaf value).
            Stmt::Break | Stmt::Continue => {}
        }
    }

    /// The accept rule (REQ-3) applied at one expression node, recursing into
    /// sub-expressions under the shared depth guard (REQ-5). `span` is the
    /// enclosing clause/item span used for any diagnostic.
    fn walk_expr(&mut self, expr: &Expr, span: Span) {
        self.descend(span, |s| s.walk_expr_inner(expr, span));
    }

    fn walk_expr_inner(&mut self, expr: &Expr, span: Span) {
        match expr {
            // (c) grammar built-ins: literals and paths are leaves. A string
            // literal (`.design/basis/07-strings.md` REQ-1) is a value-carrying
            // leaf admitted in a contract position exactly as an int/bool literal
            // — e.g. the editor case `s == "needle"`; no sub-expression to walk.
            Expr::IntLit { .. } | Expr::BoolLit(_) | Expr::Path(_) | Expr::StrLit(_) => {}

            // (a)/(b)/(iv): a free call is a combinator, a spec-fn call, or
            // forbidden.
            Expr::Call { callee, args } => self.walk_call(callee, args, span),

            // (c) bounded built-in method calls. REQ-3(c) admits only "the
            // bounded built-in `MethodCall`s the grammar admits (e.g.
            // `xs.len()`)" — NOT an arbitrary method name. A non-allowlisted
            // method name in a caged position is forbidden (REQ-4 (iv) ->
            // `ForbiddenCall`). The allowlist is `BUILTIN_METHODS`; a permitted
            // method's receiver and args are recursed into.
            Expr::MethodCall {
                receiver,
                name,
                args,
            } => {
                if !BUILTIN_METHODS.contains(&name.as_str()) {
                    self.errors.push(SpecError::ForbiddenCall {
                        detail: format!(
                            "`.{name}()` is not a bounded built-in method permitted in a \
                             contract (only {BUILTIN_METHODS:?})"
                        ),
                        span,
                    });
                }
                // Recurse operands regardless so deep/forbidden nested content
                // still surfaces (REQ-5), even on a rejected method name.
                self.walk_expr(receiver, span);
                for arg in args {
                    self.walk_expr(arg, span);
                }
            }

            // (c) field access, binary, index, cast, ref — structural built-ins.
            // REQ-6: a `Field` whose name is declared by NO `struct`/struct-variant
            // is `UnknownField`. The AST is untyped (OQ-3), so this is the
            // shallow, mechanically-decidable check the design admits — the field
            // must exist SOMEWHERE. When no ADT is declared (`struct_fields`
            // empty), the check is inert, so the existing non-ADT corpus
            // (`sum.th`/`binary_search.th`, which have no struct field access) is
            // UNCHANGED.
            Expr::Field { receiver, name } => {
                self.check_field(name, span);
                self.walk_expr(receiver, span);
            }
            Expr::Binary { lhs, rhs, .. } => {
                self.walk_expr(lhs, span);
                self.walk_expr(rhs, span);
            }
            // The prefix `!` (#92, ast.md REQ-10): a structural built-in operator,
            // admitted in a contract position exactly like a `Binary`. The cage is
            // UNTYPED (OQ-3), so the bitwise-vs-logical / valid-operand-type
            // discrimination is NOT a validator check — it is resolved DOWNSTREAM by
            // Verus's type-directed `!` (ast.md OQ-4): `!` on a non-integer /
            // non-bool operand (e.g. `&[u32]`) is rejected at L3 as a Verus type
            // error, not here. The operand is recursed (depth-guarded, REQ-5) so
            // nested forbidden content still surfaces.
            Expr::Unary { expr: inner, .. } => self.walk_expr(inner, span),
            Expr::Index { base, index } => {
                self.walk_expr(base, span);
                self.walk_index(index, span);
            }
            Expr::Cast { expr: inner, .. } => self.walk_expr(inner, span),
            Expr::Ref { expr: inner, .. } => self.walk_expr(inner, span),

            // (c) match / if — built-in control forms. A `match` over a DECLARED
            // `enum` value is exhaustiveness/well-formedness-checked (REQ-5/
            // REQ-6); a slice `match` (`sum.th`) or a `match` over a built-in
            // (`Option`'s `Some`/`None` in `binary_search.th`) is UNCHANGED —
            // `check_match_exhaustiveness` only fires when an arm pattern names a
            // variant of a declared enum. `Match`/`Field`/`If`/`Is` stay FLAT
            // built-ins inside a combinator closure (REQ-7) — the caged-flat
            // mode is untouched by this descent.
            Expr::Match { scrutinee, arms } => {
                self.walk_expr(scrutinee, span);
                self.check_match_exhaustiveness(arms, span);
                for MatchArm { body, .. } in arms {
                    self.walk_expr(body, span);
                }
            }
            Expr::If { cond, then, else_ } => {
                self.walk_expr(cond, span);
                self.walk_block(then, span);
                self.walk_block(else_, span);
            }

            // A bare closure outside a `Pred` argument slot has no meaning in a
            // contract position (a combinator's `Pred` arg is handled in
            // `walk_call`). We still recurse the body so a deeply-nested body is
            // bounded, but flag the misplaced closure.
            Expr::Closure { body, .. } => {
                self.errors.push(SpecError::ForbiddenCall {
                    detail: "a closure may appear only as a combinator predicate argument"
                        .to_string(),
                    span,
                });
                self.walk_expr(body, span);
            }

            // Basis Stage 1b (`.design/basis/01-adts.md` REQ-6): the ADT contract
            // / construction expressions are now VALIDATED, not gated.
            //
            // A struct / struct-variant construction `Path { field: val, … }`:
            // each initializer field must be declared by some
            // `struct`/struct-variant (REQ-6 — same shallow, untyped check as
            // `Field`); the field VALUES are recursed (depth-guarded, REQ-5). The
            // last `path` segment naming a known struct-variant is well-formed by
            // construction; a `path` naming nothing checkable is left to lowering
            // (1c) — the 1a `UnsupportedAdt` scream is gone for a well-formed
            // literal.
            Expr::StructLit { path, fields } => {
                // REQ-8: a `#[sealed]` clean type is door-only-mintable; a
                // literal of one (anywhere — contract OR caged body) is the #77
                // launder and is REJECTED with `SealedConstruction`.
                self.check_sealed_construction(path, span);
                for (field_name, value) in fields {
                    self.check_field(field_name, span);
                    self.walk_expr(value, span);
                }
            }
            // `SCRUTINEE is Variant` (REQ-6): the `variant` must name a declared
            // enum variant, else `UnknownVariant`. `is` is a FLAT `bool` built-in
            // and joins `Match`/`Field`/`If` in the caged-flat accept set (REQ-7)
            // — it is admitted inside a combinator predicate-closure body
            // unchanged. The scrutinee is recursed (depth-guarded).
            Expr::Is { scrutinee, variant } => {
                self.check_variant_ref(variant, span);
                self.walk_expr(scrutinee, span);
            }
            // A `Box` deref `*EXPR` (REQ-3): accepted STRUCTURALLY here (the
            // recursive deref `sum_list(*t)` of `list_sum.th`); its `Box` SEMANTICS
            // are Stage 1c. Recurse the inner expression (depth-guarded).
            Expr::Deref(inner) => self.walk_expr(inner, span),
        }
    }

    /// Walk an index argument (`a[i]`, `a[..i]`, `a[i..]`, `a[i..j]`) — each
    /// bound is a sub-expression, guarded by the shared depth counter (REQ-5).
    fn walk_index(&mut self, index: &IndexArg, span: Span) {
        match index {
            IndexArg::Single(e) | IndexArg::RangeTo(e) | IndexArg::RangeFrom(e) => {
                self.walk_expr(e, span)
            }
            IndexArg::Range(lo, hi) => {
                self.walk_expr(lo, span);
                self.walk_expr(hi, span);
            }
        }
    }

    /// REQ-5 exhaustiveness + REQ-6 variant well-formedness for a `match`'s arms.
    ///
    /// The AST is untyped (OQ-3): the matched enum is inferred from the ARM
    /// PATTERNS, not the scrutinee. A `match` is a DECLARED-enum match iff some
    /// arm names a variant of a declared `enum` (`variant_to_enum`); otherwise it
    /// is a slice `match` (`sum.th`'s `[]`/`[head, ..t]`) or a `match` over a
    /// built-in (`Option`'s `Some`/`None` in `binary_search.th` — `Option` is no
    /// declared `Item::Enum`) and is left UNCHANGED (the AC-6 no-regression
    /// invariant). Once identified as a declared-enum match:
    /// - an arm naming a variant of a DIFFERENT/undeclared enum is `UnknownVariant`;
    /// - a variant matched twice, or an arm after a catch-all, is `UnreachableArm`;
    /// - if no catch-all closes the match, every uncovered declared variant is
    ///   collected into `NonExhaustiveMatch { missing }` (declaration order).
    fn check_match_exhaustiveness(&mut self, arms: &[MatchArm], span: Span) {
        // Identify the matched enum: the owning enum of the FIRST arm pattern
        // that names a declared variant.
        let matched_enum = arms.iter().find_map(|arm| {
            variant_pattern_name(&arm.pattern).and_then(|v| self.variant_to_enum.get(v).cloned())
        });
        let Some(enum_name) = matched_enum else {
            // Not a declared-enum match (slice / Option / bindings only) — the
            // existing behavior, untouched.
            return;
        };
        // `enum_name` was resolved from `variant_to_enum`, which is built only
        // from keys present in `enums`, so this lookup always succeeds; the
        // `else` keeps the function total without a panic (R-CODE-2).
        let Some(declared) = self.enums.get(&enum_name).cloned() else {
            return;
        };

        let mut covered: HashSet<&str> = HashSet::new();
        let mut wildcard_seen = false;
        for arm in arms {
            match &arm.pattern {
                // A bare `_` or a whole-scrutinee binding (`x => …`) is a
                // catch-all: it closes the match. A second catch-all, or any arm
                // after it, can never be reached.
                Pattern::Wildcard | Pattern::Binding(_) => {
                    if wildcard_seen {
                        self.errors.push(SpecError::UnreachableArm { span });
                    }
                    wildcard_seen = true;
                }
                _ => {
                    let Some(variant) = variant_pattern_name(&arm.pattern) else {
                        // A non-variant pattern (a literal) in a declared-enum
                        // match is not a well-formed enum arm; leave it to the
                        // (untyped) shallow checking — no false UnknownVariant.
                        continue;
                    };
                    if wildcard_seen {
                        // Any arm after a catch-all is dead.
                        self.errors.push(SpecError::UnreachableArm { span });
                    } else if !declared.iter().any(|d| d == variant) {
                        self.errors.push(SpecError::UnknownVariant {
                            name: variant.to_string(),
                            span,
                        });
                    } else if !covered.insert(variant) {
                        // Variant matched twice → the second arm is unreachable.
                        self.errors.push(SpecError::UnreachableArm { span });
                    }
                }
            }
        }

        if !wildcard_seen {
            let missing: Vec<String> = declared
                .iter()
                .filter(|d| !covered.contains(d.as_str()))
                .cloned()
                .collect();
            if !missing.is_empty() {
                self.errors
                    .push(SpecError::NonExhaustiveMatch { missing, span });
            }
        }
    }

    /// REQ-8 abstraction barrier (`.design/basis/06-provenance-and-sinks.md`): a
    /// `StructLit` whose constructed type is a `#[sealed]` clean/capability type
    /// is REJECTED (`SealedConstruction`) — a sealed type is door-only-mintable.
    /// `path` is the literal's path; the constructed type is its LAST segment
    /// (`Sql` in `Sql { … }`). Inert when no `#[sealed]` struct is declared (the
    /// non-IFC corpus is UNCHANGED). The `#[boundary]` door is unaffected: its
    /// body is foreign/`external_body`, with no in-language `StructLit`, so the
    /// safe path `query(parameterize(input))` carries no sealed literal.
    fn check_sealed_construction(&mut self, path: &[String], span: Span) {
        if let Some(name) = path.last() {
            if self.sealed_structs.contains(name) {
                self.errors.push(SpecError::SealedConstruction {
                    name: name.clone(),
                    span,
                });
            }
        }
    }

    /// REQ-6 field well-formedness: a `Field`/struct-literal field name must be
    /// declared by SOME `struct`/struct-variant. Shallow + untyped (OQ-3): inert
    /// when no ADT declares any field (the non-ADT corpus is UNCHANGED), and a
    /// name no declared struct/struct-variant carries is `UnknownField`.
    fn check_field(&mut self, name: &str, span: Span) {
        if !self.struct_fields.is_empty() && !self.struct_fields.contains(name) {
            self.errors.push(SpecError::UnknownField {
                name: name.to_string(),
                span,
            });
        }
    }

    /// REQ-6 variant well-formedness for an `is` discrimination (`r is Circle`)
    /// — the variant (last path segment) must name a declared enum variant, else
    /// `UnknownVariant`.
    fn check_variant_ref(&mut self, variant: &[String], span: Span) {
        if let Some(name) = variant.last() {
            if !self.variant_to_enum.contains_key(name) {
                self.errors.push(SpecError::UnknownVariant {
                    name: name.clone(),
                    span,
                });
            }
        }
    }

    /// Resolve a free `Expr::Call` callee against the cage (REQ-3 (a)/(b),
    /// REQ-4). The callee is expected to be a single-segment `Path`.
    fn walk_call(&mut self, callee: &Expr, args: &[Expr], span: Span) {
        let name = match callee {
            Expr::Path(segments) if segments.len() == 1 => &segments[0],
            // A path with `::` segments or a non-path callee is not a combinator
            // or spec-fn call the grammar admits in a contract (REQ-4 (iv)).
            _ => {
                self.errors.push(SpecError::ForbiddenCall {
                    detail: "a contract call's callee must be a bare combinator or `spec fn` name"
                        .to_string(),
                    span,
                });
                // Still recurse args so nested forbidden/deep content surfaces.
                for arg in args {
                    self.walk_expr(arg, span);
                }
                return;
            }
        };

        // Basis Stage 2 (`.design/basis/02-recursion-schemes.md` REQ-1/REQ-2/
        // REQ-4): a callee resolving to a registered recursion SCHEME
        // (`fold`/`map`/`for_all`/`exists`/`traverse`) is handled FIRST. A scheme
        // nested inside another scheme's step OR inside a combinator's
        // predicate-closure body is an anonymous nested structural quantifier the
        // cage forbids (`NestedScheme`); a top-level scheme call is ACCEPTED as a
        // named-composition leaf after its arity + flat step are checked. The
        // scheme registry is disjoint from the combinator registry (distinct name
        // sets), so this branch never shadows a combinator.
        if let Some(scheme) = schemes::lookup(name) {
            if self.in_scheme_step || self.in_combinator_closure {
                self.errors.push(SpecError::NestedScheme {
                    name: name.clone(),
                    span,
                });
                // Still recurse the args (staying in the caged mode) so deeper
                // nested schemes / forbidden / too-deep content also surfaces.
                for arg in args {
                    self.walk_expr(arg, span);
                }
            } else {
                self.check_scheme(scheme, args, span);
            }
            return;
        }

        if let Some(sig) = combinators::lookup(name) {
            if self.in_combinator_closure || self.in_scheme_step {
                // REQ-6 (combinators) / REQ-2 (schemes): a combinator call inside
                // another combinator's predicate-closure body OR a scheme's flat
                // step closure is an anonymous nested quantifier — forbidden. The
                // discriminator is EXACTLY `combinators::lookup` succeeding (the
                // same test that ACCEPTS this callee in a top-level contract
                // position); the verdict is context-dependent. Inside a scheme
                // step the diagnostic is `NestedScheme` (the flat-step cage,
                // 02-recursion-schemes.md REQ-2); inside a combinator closure it
                // stays `NestedCombinator` (spectherm-combinators.md REQ-6).
                if self.in_scheme_step {
                    self.errors.push(SpecError::NestedScheme {
                        name: name.clone(),
                        span,
                    });
                } else {
                    self.errors.push(SpecError::NestedCombinator {
                        name: name.clone(),
                        span,
                    });
                }
                // Still recurse the args (staying in caged mode) so deeper
                // nested combinators / forbidden / too-deep content also surfaces
                // (REQ-5), and so a doubly-nested combinator is reported too.
                for arg in args {
                    self.walk_expr(arg, span);
                }
            } else {
                self.check_combinator(sig, args, span);
            }
        } else if self.spec_fns.contains(name) {
            // (b) a declared spec-fn call: accept; its arguments are ordinary
            // contract expressions (recursed, depth-guarded).
            for arg in args {
                self.walk_expr(arg, span);
            }
        } else {
            // (i) neither a combinator nor a declared spec fn — forbidden.
            self.errors.push(SpecError::UnknownCombinator {
                name: name.clone(),
                span,
            });
            for arg in args {
                self.walk_expr(arg, span);
            }
        }
    }

    /// Check a registered combinator call: arity (REQ-4 (ii)) then each
    /// argument's kind (REQ-4 (iii)), recursing into argument sub-expressions.
    fn check_combinator(&mut self, sig: &CombinatorSig, args: &[Expr], span: Span) {
        if args.len() != sig.arity {
            self.errors.push(SpecError::WrongArity {
                name: sig.name.to_string(),
                expected: sig.arity,
                found: args.len(),
                span,
            });
            // Arity is wrong; still recurse the supplied args (depth guard,
            // nested-content surfacing) but skip per-position kind checks (the
            // positions don't line up).
            for arg in args {
                self.walk_expr(arg, span);
            }
            return;
        }

        for (position, (arg, kind)) in args.iter().zip(sig.arg_kinds.iter()).enumerate() {
            self.check_arg_kind(sig.name, position, *kind, arg, span);
        }
    }

    /// Check a registered recursion-scheme call as a named-composition leaf
    /// (`.design/basis/02-recursion-schemes.md` REQ-1/REQ-2/REQ-4): the total
    /// arity matches the scheme (scrutinee/seed args + the trailing step closure),
    /// the trailing argument is an `Expr::Closure` of the scheme's per-node step
    /// shape, and the step body is FLAT (no nested scheme/combinator — enforced by
    /// walking the body in `in_scheme_step` mode). The scrutinee/seed args are
    /// ordinary contract expressions (recursed, depth-guarded). ACCEPTED at the
    /// top level (the cage bridge, REQ-4) — the §4.2 "named composition" leaf.
    fn check_scheme(&mut self, scheme: &SchemeSig, args: &[Expr], span: Span) {
        let expected = scheme.total_arity();
        if args.len() != expected {
            self.errors.push(SpecError::SchemeWrongArity {
                name: scheme.name.to_string(),
                expected,
                found: args.len(),
                span,
            });
            // Arity is wrong; still recurse the supplied args (depth guard,
            // nested-content surfacing) but skip the per-position step check (the
            // step slot is not where we expect it). The step body, if any, is
            // walked WITHOUT scheme-step mode — a malformed call is not a valid
            // step context.
            for arg in args {
                self.walk_expr(arg, span);
            }
            return;
        }

        // The leading `scrutinee_args` are ordinary contract sub-expressions (the
        // scrutinee structure + a fold seed), recursed under the depth guard. They
        // are NOT in scheme-step mode — a scheme/combinator there is a legitimate
        // nested named composition at the call's argument level (only the STEP
        // body is the flat-cage position, REQ-2).
        let step_position = scheme.scrutinee_args;
        for arg in &args[..step_position] {
            self.walk_expr(arg, span);
        }

        // The trailing argument is the per-node STEP: it must be a closure of the
        // scheme's step shape, and its body is walked in `in_scheme_step` mode so
        // a nested scheme/combinator is rejected (the flat-step cage, REQ-2/REQ-4).
        let step_arity = scheme.step_shape.arity();
        match &args[step_position] {
            Expr::Closure { params, body } => {
                if params.len() != step_arity {
                    self.errors.push(SpecError::SchemeStepShape {
                        name: scheme.name.to_string(),
                        expected: step_arity,
                        found: params.len(),
                        span,
                    });
                }
                // Enter flat-scheme-step mode for the body. Set ONCE here and keep
                // it set for the entire body descent (save/restore so a sibling
                // scheme call's step is checked independently and re-entry from a
                // — rejected — nested scheme is a harmless no-op).
                let saved = self.in_scheme_step;
                self.in_scheme_step = true;
                self.walk_expr(body, span);
                self.in_scheme_step = saved;
            }
            other => {
                // A non-closure in the step slot: report the shape error
                // (`found: 0` params against the scheme's expected step arity) and
                // still recurse the expression for deep/forbidden nested content.
                self.errors.push(SpecError::SchemeStepShape {
                    name: scheme.name.to_string(),
                    expected: step_arity,
                    found: 0,
                    span,
                });
                self.walk_expr(other, span);
            }
        }
    }

    /// Check one positional argument against its expected `ArgKind` (REQ-4
    /// (iii)), then recurse into the argument's sub-expressions.
    ///
    /// Per OQ-3, only `Pred` is syntactically decidable (MUST be `Expr::Closure`);
    /// `Slice`/`Index`/`Value` are checked shallowly: any NON-closure expression
    /// is accepted in those positions (a closure there is the only decidable
    /// error), with full typing deferred to a later pass (not a v0.1 item).
    fn check_arg_kind(
        &mut self,
        name: &'static str,
        position: usize,
        kind: ArgKind,
        arg: &Expr,
        span: Span,
    ) {
        match kind {
            ArgKind::Pred => match arg {
                // A `Pred` slot is satisfied by a closure literal (the one
                // syntactically strict kind, OQ-3). Recurse into the closure
                // BODY — the legitimate contract sub-expression — rather than
                // the closure node (which `walk_expr` would flag as a misplaced
                // bare closure). This bounds the body's depth too (REQ-5).
                //
                // REQ-6: enter "caged-flat" mode for the body. Set ONCE here and
                // keep it set for the entire body descent (all nested
                // sub-expressions AND any nested closure), then restore so a
                // sibling top-level `Pred` slot is checked independently. Inside
                // this mode a registered-combinator call is rejected with
                // `NestedCombinator` (see `walk_call`); a named `spec fn` call
                // stays accepted (named composition is the sanctioned alternative).
                // The save/restore makes re-entry a harmless no-op (a nested
                // `Pred` body's `|y|` re-sets an already-set flag).
                Expr::Closure { body, .. } => {
                    let saved = self.in_combinator_closure;
                    self.in_combinator_closure = true;
                    self.walk_expr(body, span);
                    self.in_combinator_closure = saved;
                }
                _ => {
                    self.errors.push(SpecError::WrongArgKind {
                        name: name.to_string(),
                        position,
                        expected: ArgKind::Pred,
                        span,
                    });
                    // A non-closure in a Pred slot is still an expression we
                    // recurse for deep/forbidden nested content (REQ-5).
                    self.walk_expr(arg, span);
                }
            },
            ArgKind::Slice | ArgKind::Index | ArgKind::Value => {
                if matches!(arg, Expr::Closure { .. }) {
                    // A closure in a non-Pred slot is decidably wrong; emit the
                    // kind error (the recursion below also flags the bare
                    // closure, but the precise kind diagnostic is the primary).
                    self.errors.push(SpecError::WrongArgKind {
                        name: name.to_string(),
                        position,
                        expected: kind,
                        span,
                    });
                }
                // Recurse into the argument (a `Slice`'s index expression, a
                // `Value`'s operands, etc.) so deep/forbidden nested content is
                // bounded and surfaced (REQ-5).
                self.walk_expr(arg, span);
            }
        }
    }
}

/// The variant name a `match` arm pattern names, or `None` for a non-variant
/// pattern (`.design/basis/01-adts.md` REQ-5). A `Pattern::Enum`
/// (`Circle(r)`, `Nil`, `Some(i)`) and a `Pattern::Struct` (`Rect { w, h }`)
/// both name a variant by the LAST path segment (the variant name; an enclosing
/// `Shape::` prefix is the type). A `Wildcard`/`Binding`/`Literal`/`Slice`
/// pattern names no variant — used to distinguish a declared-enum match from a
/// slice match (`sum.th`) and to drive the covered-variant set.
fn variant_pattern_name(pattern: &Pattern) -> Option<&str> {
    match pattern {
        Pattern::Enum { path, .. } | Pattern::Struct { path, .. } => {
            path.last().map(|s| s.as_str())
        }
        Pattern::Wildcard | Pattern::Binding(_) | Pattern::Literal(_) | Pattern::Slice(_) => None,
    }
}

/// True iff the exec `fn` declares `fx diverge` (`.design/basis/10-recursion-tuples.md`
/// REQ-2, §4.1: "divergence requires `fx diverge`"). A diverge fn is honestly
/// non-terminating (an event loop), so it is EXEMPT from the mandatory-`dec` rule
/// — it may recurse without a termination measure (the #88 L1-cap; the lowerer
/// emits `#[verifier::exec_allows_no_decreases_clause]`). Keyed on the SHAPE of
/// the effect row, mirroring `thermite-lower`'s `fn_is_diverge` (the single source
/// of truth for the §4.1 termination exemption).
fn fn_is_diverge(f: &thermite_syntax::ast::FnItem) -> bool {
    use thermite_syntax::ast::{Effect, EffectRow};
    matches!(&f.contract.fx, EffectRow::Set(es) if es.contains(&Effect::Diverge))
}

/// True iff `block` contains a DIRECT call to `name` — the self-reference test
/// for the mandatory-`dec` rule (`.design/basis/10-recursion-tuples.md` REQ-2).
/// "Direct" means a free-function call `name(..)` whose callee path's last
/// segment is `name` (a self-recursive call); a method call `recv.name(..)` is
/// NOT a self-call (the receiver dispatches it). Walks every statement + nested
/// expression of the body (let inits, assigns, returns, ifs, loops, match arms,
/// call args, …) so a self-call anywhere in the body is found. Mutual recursion
/// (a call to a DIFFERENT fn that calls back) is NOT detected here (REQ-6,
/// deferred) — only a direct self-call triggers the rule.
fn block_calls_name(block: &Block, name: &str) -> bool {
    block.stmts.iter().any(|s| stmt_calls_name(s, name))
        || block
            .tail
            .as_ref()
            .is_some_and(|e| expr_calls_name(e, name))
}

fn stmt_calls_name(stmt: &Stmt, name: &str) -> bool {
    match stmt {
        Stmt::Let { init, .. } => expr_calls_name(init, name),
        Stmt::Assign { target, value } => {
            expr_calls_name(target, name) || expr_calls_name(value, name)
        }
        Stmt::Return(opt) => opt.as_ref().is_some_and(|e| expr_calls_name(e, name)),
        Stmt::If { cond, then, else_ } => {
            expr_calls_name(cond, name)
                || block_calls_name(then, name)
                || else_.as_ref().is_some_and(|b| block_calls_name(b, name))
        }
        Stmt::Loop(l) => block_calls_name(&l.body, name),
        Stmt::Break | Stmt::Continue => false,
        Stmt::Expr(e) => expr_calls_name(e, name),
    }
}

fn expr_calls_name(expr: &Expr, name: &str) -> bool {
    match expr {
        Expr::Call { callee, args } => {
            let callee_is_self = matches!(
                callee.as_ref(),
                Expr::Path(segs) if segs.last().map(|s| s.as_str()) == Some(name)
            );
            callee_is_self
                || expr_calls_name(callee, name)
                || args.iter().any(|a| expr_calls_name(a, name))
        }
        Expr::MethodCall { receiver, args, .. } => {
            expr_calls_name(receiver, name) || args.iter().any(|a| expr_calls_name(a, name))
        }
        Expr::Field { receiver, .. } => expr_calls_name(receiver, name),
        Expr::Closure { body, .. } => expr_calls_name(body, name),
        Expr::Match { scrutinee, arms } => {
            expr_calls_name(scrutinee, name) || arms.iter().any(|a| expr_calls_name(&a.body, name))
        }
        Expr::If { cond, then, else_ } => {
            expr_calls_name(cond, name)
                || block_calls_name(then, name)
                || block_calls_name(else_, name)
        }
        Expr::Binary { lhs, rhs, .. } => expr_calls_name(lhs, name) || expr_calls_name(rhs, name),
        Expr::Unary { expr, .. } => expr_calls_name(expr, name),
        Expr::Index { base, index } => expr_calls_name(base, name) || index_calls_name(index, name),
        Expr::Cast { expr, .. } => expr_calls_name(expr, name),
        Expr::Ref { expr, .. } => expr_calls_name(expr, name),
        Expr::StructLit { fields, .. } => fields.iter().any(|(_, e)| expr_calls_name(e, name)),
        Expr::Is { scrutinee, .. } => expr_calls_name(scrutinee, name),
        Expr::Deref(e) => expr_calls_name(e, name),
        Expr::IntLit { .. } | Expr::BoolLit(_) | Expr::Path(_) | Expr::StrLit(_) => false,
    }
}

fn index_calls_name(index: &IndexArg, name: &str) -> bool {
    match index {
        IndexArg::Single(e) | IndexArg::RangeTo(e) | IndexArg::RangeFrom(e) => {
            expr_calls_name(e, name)
        }
        IndexArg::Range(a, b) => expr_calls_name(a, name) || expr_calls_name(b, name),
    }
}
