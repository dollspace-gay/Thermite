//! The SpecTherm validator — the boundary API that walks a parsed
//! `thermite-syntax` program's contract positions and enforces §4.2's "locked
//! cage": a contract may use only registered combinators (right name + arity +
//! arg-kinds), declared `spec fn` calls, and the built-in operators / literals /
//! paths the grammar already sanctions, and nothing else.
//!
//! Governing design: `.design/spec/spectherm-combinators.md` (REQ-3/4/5).
//! Verified against the oracle at `tests/golden/combinators/` (accept.json /
//! reject.json), R-CHAR-3.
//!
//! ## REQ status
//!
//! <!-- generated:reqs view=thermite-spec-validator-core-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-SPEC-VALIDATOR-ACCEPT | shipped | `thermite-spec/src/validator.rs` | Validator accept rule |  |
//! | REQ-SPEC-VALIDATOR-DEPTH | shipped | `thermite-spec/src/validator.rs` | Validator bounded recursion |  |
//! | REQ-SPEC-VALIDATOR-FLAT-CLOSURE | shipped | `thermite-spec/src/validator.rs` | Validator flat closure fragment |  |
//! | REQ-SPEC-VALIDATOR-REJECT | shipped | `thermite-spec/src/validator.rs` | Validator reject cases |  |
//! <!-- /generated:reqs -->
//!
//! ## Basis Stage 1b — the real ADT validator (`.design/basis/01-adts.md`)
//!
//! Stage 1b replaces the 1a `UnsupportedAdt` gate with real exhaustiveness +
//! well-formedness checking. The 3 ADT corpus programs validate clean; crafted
//! negatives reject with the precise structured error. Verified against the
//! oracle `conformance/adt-validate/cases.json` (R-CHAR-3) via
//! `tests/adt_validate.rs`. Lowering stays gated (Stage 1c, thermite-lower).
//!
//! <!-- generated:reqs view=thermite-spec-validator-adt-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-SPEC-VALIDATOR-ADT-CAGE | shipped | `thermite-spec/src/validator.rs` | ADT predicates admitted as flat cage built-ins |  |
//! | REQ-SPEC-VALIDATOR-ADT-EXHAUSTIVENESS | shipped | `thermite-spec/src/validator.rs` | ADT match exhaustiveness validation |  |
//! | REQ-SPEC-VALIDATOR-ADT-UNSUPPORTED-GATE | retired | `thermite-spec/src/validator.rs` | Retired unsupported ADT gate |  |
//! | REQ-SPEC-VALIDATOR-ADT-VARIANT-CASING | shipped | `thermite-spec/src/validator.rs` | UpperCamelCase variant validation |  |
//! | REQ-SPEC-VALIDATOR-ADT-WELLFORMED | shipped | `thermite-spec/src/validator.rs` | ADT field and variant well-formedness |  |
//! <!-- /generated:reqs -->
//!
//! ## Basis Stage 2b — recursion-scheme recognition + the flat-step cage (`.design/basis/02-recursion-schemes.md`)
//!
//! Stage 2b extends the cage to recognize a recursion-scheme call
//! (`fold`/`map`/`for_all`/`exists`/`traverse`) as a named-composition leaf and
//! to reject a scheme/combinator nested in a scheme's step closure (the flat-step
//! cage). Verified against the oracle `conformance/adt-schemes/cases.json`
//! (R-CHAR-3) via `thermite-spec/tests/scheme_validate.rs`.
//!
//! <!-- generated:reqs view=thermite-spec-validator-scheme-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-SPEC-VALIDATOR-SCHEME-CAGE | shipped | `thermite-spec/src/validator.rs` | Recursion scheme cage bridge |  |
//! | REQ-SPEC-VALIDATOR-SCHEME-ERRORS | shipped | `thermite-spec/src/validator.rs` | Recursion scheme validator errors |  |
//! | REQ-SPEC-VALIDATOR-SCHEME-FLAT-STEP | shipped | `thermite-spec/src/validator.rs` | Recursion scheme flat step validation |  |
//! | REQ-SPEC-VALIDATOR-SCHEME-RECOGNITION | shipped | `thermite-spec/src/validator.rs` | Recursion scheme recognition |  |
//! <!-- /generated:reqs -->
//!
//! ## REQ status — 04-collections.md (Basis Stage 4, issue #73)
//!
//! <!-- generated:reqs view=thermite-spec-validator-collections-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-LOWER-VEC-METHOD-CAGE | shipped | `thermite-lower/src/lower.rs` | Vec contract method cage integration |  |
//! | REQ-SPEC-VALIDATOR-COLLECTIONS-CAGE | shipped | `thermite-spec/src/validator.rs` | Vec contract cage admission |  |
//! | REQ-SPEC-VALIDATOR-COLLECTIONS-ELEM-INVARIANT | not_started | `thermite-spec/src/validator.rs` | Deferred Vec element invariant validation | Add a corpus `Vec<Account>`/element-invariant program and validator oracle coverage in the Stage 4 follow-up.<br>blockers: github:dollspace-gay/Thermite#62 |
//! <!-- /generated:reqs -->
//!
//! ## REQ status — 13-map.md cluster C12 (bounded verified Map<K,V>, issue #114/#123)
//!
//! <!-- generated:reqs view=thermite-spec-validator-map-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-SPEC-VALIDATOR-MAP-CAGE | shipped | `thermite-spec/src/validator.rs` | Map contract cage admission |  |
//! <!-- /generated:reqs -->
//!
//! ## REQ status — 06-provenance-and-sinks.md (Basis Stage 6, issue #76 / blocker #77)
//!
//! <!-- generated:reqs view=thermite-spec-validator-provenance-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-SPEC-VALIDATOR-SEALED-CONSTRUCTION | shipped | `thermite-spec/src/validator.rs` | Sealed construction validator barrier |  |
//! <!-- /generated:reqs -->
//!
//! ## REQ status — 07-strings.md (Basis Stage 7 cluster C4, issue #94)
//!
//! <!-- generated:reqs view=thermite-spec-validator-string-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-STRING-BYTE-BUILDER | shipped | `thermite-lower/src/lower.rs` | String byte-builder operations |  |
//! | REQ-STRING-BYTES-EQ-CAGE | shipped | `thermite-spec/src/validator.rs` | bytes_eq validator cage entry |  |
//! | REQ-STRING-FIND | shipped | `thermite-lower/src/lower.rs` | String find operation |  |
//! | REQ-STRING-PARSE-U64 | shipped | `thermite-lower/src/lower.rs` | parse_u64 handled-or-loud parser |  |
//! | REQ-STRING-SPLIT | shipped | `thermite-lower/src/lower.rs` | String split operation |  |
//! | REQ-STRING-SUBSTRING-PREDICATES | shipped | `thermite-lower/src/lower.rs` | String substring predicates |  |
//! | REQ-STRING-TRIM | shipped | `thermite-lower/src/lower.rs` | String trim operation |  |
//! | REQ-STRING-U64-TO-STRING | shipped | `thermite-lower/src/lower.rs` | u64 decimal formatting with round-trip contract |  |
//! <!-- /generated:reqs -->
//!
//! ## REQ status — 09-option-result.md (Cluster C7, built-in Option/Result, issue #95)
//!
//! <!-- generated:reqs view=thermite-spec-validator-option-result-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-SPEC-VALIDATOR-OPTRES-BUILTINS | shipped | `thermite-spec/src/validator.rs` | Option and Result built-in validator registry |  |
//! <!-- /generated:reqs -->
//!
//! ## REQ status — 10-recursion-tuples.md (Cluster C9-A, plain-`fn` recursion, issue #108)
//!
//! <!-- generated:reqs view=thermite-spec-validator-recursion-tuples-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-SPEC-VALIDATOR-RECURSION-DECREASES | shipped | `thermite-spec/src/validator.rs` | Recursive fn decreases validation |  |
//! <!-- /generated:reqs -->
//!
//! ## REQ status — 11-ergonomics.md (Cluster C10, binding/control-flow ergonomics, issue #112)
//!
//! <!-- generated:reqs view=thermite-spec-validator-ergonomics-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-SPEC-VALIDATOR-ERGONOMICS-GUARD | shipped | `thermite-spec/src/validator.rs` | Match guard exhaustiveness validation |  |
//! | REQ-SPEC-VALIDATOR-ERGONOMICS-OR-PATTERN | shipped | `thermite-spec/src/validator.rs` | Or-pattern exhaustiveness validation |  |
//! <!-- /generated:reqs -->

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt;

use thermite_syntax::{
    ArrayLen, Block, Clause, Expr, ForgeItem, IndexArg, Item, MatchArm, Pattern, Program, Span,
    Stmt, Type, VariantShape,
};

use crate::combinators::{self, ArgKind, CombinatorSig};
use crate::schemes::{self, SchemeSig};

/// The maximum recursive-descent nesting depth the validator will follow before
/// returning an `ExpressionTooDeep` diagnostic. A fixed constant for determinism
/// (R-CODE-5), mirroring `thermite-syntax`'s parser `MAX_RECURSION_DEPTH`.
///
/// This single bound guards every recursive descent in the walk — nested
/// combinator/spec-fn arguments, `Binary`/`Index`/`Cast`/`Ref`/`Field`
/// operands, closure bodies, `Match` scrutinee + arm bodies, `If` branches, and
/// block statements/tails — so a pathological deeply-nested contract surfaces a
/// structured error rather than overflowing the native stack and aborting the
/// process (REQ-5; the #29/#31/#32 expr-only-guard lesson: do not leave any
/// recursive path unbounded).
const MAX_RECURSION_DEPTH: usize = 64;

/// Maximum capacity of one native fixed array, and maximum recursively expanded
/// element count of a nested fixed-array type. This is a language/tooling bound,
/// not a host or target `usize` fact, so validation is deterministic across
/// machines and rejects source-sized denial-of-service inputs before lowering.
pub const MAX_FIXED_ARRAY_ELEMENTS: u128 = 1_048_576;

/// The bounded set of built-in `MethodCall` names a caged position admits
/// (REQ-3(c): "the bounded built-in `MethodCall`s the grammar admits (e.g.
/// `xs.len()`)"). Any method name outside this set in a contract position is a
/// `ForbiddenCall` (REQ-4 (iv)); the §4.2 cage is closed.
///
/// Set = `len` + the bounded-collection no-OOB accessor `get`
/// (`.design/basis/04-collections.md` REQ-3): `len` is the slice/Vec length used
/// by `sum.th`/`binary_search.th` (`haystack.len()`, `xs.len()`) and the Vec
/// capacity contract (`v.len() < CAP`, `result.len() == v.len() + 1`); `get` is
/// the verified `Vec` accessor whose result a contract names
/// (`ens result == v.get(i)` in `conformance/vec_demo.th`'s `checked_get`),
/// admitted as a flat built-in inside the §4.2 cage as `len` is. The
/// lowerer maps the spec-position `v.get(i)` to the wrapper's `spec_get(i as int)`
/// (REQ-5). `push`/`pop` are not here: they are exec-only mutators (a fn body),
/// never named in a contract position, so the cage does not admit them. No other
/// built-in method is added: per REQ-1's frozen-set discipline and anti-goal
/// §11, the set grows only by design amendment from a corpus need, never
/// speculatively.
/// Stage 7 strings (`.design/basis/07-strings.md` REQ-3): the bounded `String`
/// operations a contract names are flat built-ins admitted inside the §4.2 cage
/// as the slice/`Vec` `len`/`get` are. `byte_at` is the no-OOB byte
/// accessor whose result a contract names (`ens result == s.byte_at(0)` in
/// `conformance/string_demo.th`'s `first_byte`); `concat` is the bounded
/// constructing op whose length a contract names (`ens result.len() == a.len() +
/// b.len()` in `join`, the receiver `a.concat(b)`); `slice` is the bounded
/// substring (`ens result.len() == hi - lo`). The no-OOB safety is in the
/// lowered accessor's `req i < len` (the lowerer maps `s.byte_at(i)` to the
/// wrapper's `spec_byte_at(i as int)` whose exec mirror carries the precondition,
/// REQ-4); admitting the method here only opens the cage to name it, the bound is
/// proved by verus (the unguarded form fails, non-vacuity, R-DEFER-9). `push`/
/// the literal-materialization are exec-only (a fn body), never in a contract.
/// Cluster C4 strings (`.design/basis/07-strings.md` REQ-7/REQ-8, issue #94):
/// `push_byte` is the verified byte-builder's append op whose result a contract
/// names (the byte-builder length/element-frame `ens`), admitted as a flat
/// built-in as `concat`/`slice`; `to_string` is the `u64`→decimal-`String`
/// method whose round-trip a contract names (`ens parse_le(result) == n`, the
/// grounded gold standard). Both are constructing ops (`fx alloc`); `from_byte` is
/// an associated path-call (`String::from_byte(b)`, an `Expr::Call`), so it needs
/// no `BUILTIN_METHODS` entry. The L3 proof covers the no-OOB and round-trip properties (a
/// wrong digit fails, R-DEFER-9); admitting the method here only opens the cage to
/// name it.
/// Cluster C6 collections (`.design/basis/04-collections.md` REQ-8/REQ-12, issue
/// #98): `last` is the bounded-`Vec` final-element accessor whose result a contract
/// names (`ens result == v.last()`), admitted as a flat built-in as `get`:
/// the lowerer maps spec-position `v.last()` to the wrapper's `spec_get((len-1) as
/// int)`; `contains` is the element-membership predicate whose result a contract
/// names (`ens result == v.contains(x)`), admitted so the cage can name it (its
/// `exists`-meaning is proved by the exec `ens`'s linear-scan invariant, R-DEFER-9).
/// `pop_last`/`insert`/`remove` stay exec-only (`&mut` mutators, never in a
/// contract). No other built-in is added: REQ-1 frozen-set discipline.
/// Cluster C5 string search/transform (`.design/basis/07-strings.md` REQ-13..16,
/// issue #102): `starts_with`/`ends_with` are the boolean substring predicates whose
/// result a contract names (`ens result == occurs_at(s@, needle@, ..)`), `find` is
/// the first-occurrence search whose result is the built-in `Option<u64>` named via
/// the spec-`match`-in-`ens` (`ens match result { Some(at) => occurs_at(..), None =>
/// !contains_sub(..) }`), `split` is the `Vec<String>` splitter and `trim` the
/// whitespace stripper whose results a contract names (`ens result.len() == 1 +
/// count_sep(..)` / `ens exists|lo,hi| result == s.subrange(lo,hi)`). All admitted as
/// flat built-ins so the §4.2 cage can name them; their meanings are proved by the
/// emitted exec scans' loop invariants (R-DEFER-9). `contains` (already present from
/// C6) is shared by the string substring op and the `Vec` membership op: the surface
/// name is one, but the lowerer receiver-type-dispatches it (a `String` receiver →
/// `TString::contains` substring scan; a `Vec` receiver → `TVec::contains` membership
/// scan), so no separate entry and no clobber (Rust inherent-method resolution keys on
/// the receiver type).
const BUILTIN_METHODS: &[&str] = &[
    "len",
    // Native fixed-array equality. Verus cannot model Rust array `PartialEq`, so
    // the L3 lowerer maps this explicit surface operation to a directly verified
    // const-generic scan and maps contract occurrences to finite-view equality.
    "array_eq",
    "array_same_except",
    // Total `u64` packed-bit operations. An index at least 64 leaves a word
    // unchanged for updates and observes `false`; the directly verified L3
    // helpers supply the ordinary contract bridge and distinct-bit framing
    // witnesses needed by packed storage.
    "bit_test",
    "bit_set",
    "bit_clear",
    "bit_set_preserves_other",
    "bit_clear_preserves_other",
    "get",
    "last",
    "contains",
    // Cluster C12 (`.design/basis/13-map.md` REQ-3): the bounded-`Map` membership
    // predicate whose result a contract names (`ens result == m.contains_key(k)`),
    // admitted as a flat built-in as the `Vec` `contains` / no-OOB `get`:
    // the lowerer maps spec-position `m.contains_key(k)` to the wrapper's spec
    // abstraction `m.spec_contains_key(k)` (the `exists|j| data@[j].0 == k`
    // membership). `insert` stays exec-only (`&mut` mutator, never in a contract,
    // like `push`); `get`/`len` are already present. The §4.2 caged-flat walk
    // (`walk_expr_inner`'s `MethodCall` allowlist arm) is unchanged.
    "contains_key",
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

/// The generated `spec fn` names the lowerer materializes for the C4 `u64`→`String`
/// round-trip (`.design/basis/07-strings.md` REQ-8, issue #94): `parse_be` (the
/// MSB-first / human-readable decimal value of a byte sequence — the DISPLAY-form
/// round-trip the surface contract names, blocker #96), `parse_le` (the LSB-first
/// construction-order value the bridge carries the proof through), and `pow10` (the
/// decimal weight). They are not user-declared: the lowerer
/// (`thermite-lower::lower`) emits them when the program uses `n.to_string()`, but a
/// contract names `parse_be` to state the round-trip (`ens parse_be(result) == n`),
/// so the validator must accept them as named `spec fn` calls inside the §4.2 cage
/// (seeded into `Validator::spec_fns` in `Validator::new`, alongside the
/// user-declared spec fns). This is the mechanism a user `spec fn` call uses;
/// these are reserved generated names, so a user cannot shadow them (a clash would be
/// a name collision the lowerer owns).
/// Cluster C7 (`.design/basis/09-option-result.md` REQ-3/REQ-5, issue #95) adds
/// the `parse_u64` round-trip spec fns `parse_be` (already present, the C4
/// big-endian value), `all_digits` (the all-bytes-are-`'0'..'9'` witness), and
/// `is_digit` (the per-byte `48 <= b <= 57` predicate) — so the generated
/// `parse_u64`'s round-trip `ens match result { Some(v) => all_digits(s.data@) &&
/// parse_be(s.data@) == v, None => true }` validates inside the §4.2 cage as named
/// `spec fn` calls (the lowerer emits their bodies in `emit_parse_defs`).
/// Cluster C5 (`.design/basis/07-strings.md` REQ-13..16, issue #102) adds the string
/// search/transform spec fns the lowerer emits when a program uses the C5 ops, so a
/// contract naming them validates inside the §4.2 cage as named `spec fn` calls (the
/// lowerer emits their bodies in `emit_string_search_defs`): `occurs_at(s, needle, at)`
/// (needle occurs at byte offset `at`, a flat bounded `forall|k|`), `contains_sub(s,
/// needle)` (a flat bounded `exists|at| occurs_at(..)`, the `contains`/`find`
/// meaning), `count_sep(s, sep)` (the recursive separator count, `split`'s result
/// length), `sep_free(s, sep)` (no byte equals `sep`, each `split` piece), and
/// `is_space(b)` (the ASCII-whitespace predicate `trim` strips). All flat / bounded /
/// named per §4.2 (no anonymous nested quantifiers; `occurs_at`'s inner `forall|k|`
/// lives in the named spec-fn body).
///
/// Cluster C8 (`.design/basis/07-strings.md` REQ-17, issue #278) adds the
/// byte-range-equality predicate `bytes_eq(a, b, ai, bi, n)`, a registered
/// built-in spec predicate (the `slice_id`/`insert_str` content pins), not a §2
/// frozen-combinator `REGISTRY` entry (its 5-arg String/index signature fits no
/// `ArgKind` shape, and it carries no closure). Joining `GENERATED_SPEC_FNS`
/// costs zero skill tokens (the skill renders only the combinator/scheme
/// registries, so the §2.2 budget holds at 5988/6000), where a combinator entry
/// would auto-render ~+38 tokens and blow the gate (REQ-17's verdict). The
/// lowerer (`thermite-lower::lower`) materializes the canonical `Seq<u8>`
/// low-peel recursion + the four prove-once bridge lemmas
/// (`lemma_bytes_eq_from_pointwise`/`_to_pointwise`/`_from_subrange`/`_bridge`)
/// when the program names `bytes_eq` (`program_uses_bytes_eq`).
/// The reserved prefix the lowerer mints its generated byte-view helper fns under
/// (`.design/basis/07-strings.md` REQ-4, blocker #130; mirrors
/// `thermite-lower::lower::THERMITE_RESERVED_PREFIX`, the single reserved scheme). A
/// user `fn`/`spec fn` declared in this namespace is rejected (`SpecError::Reserved
/// Name`) so the generated defs (which emit under this prefix) can never collide with
/// a user name; the byte-view name-collision class is closed at the surface.
const THERMITE_RESERVED_PREFIX: &str = "__thermite_";

const GENERATED_SPEC_FNS: &[&str] = &[
    // Verus state-view primitives for `&mut` pre/postconditions. They are not
    // emitted by the lowerer; admitting them here keeps mutable-state contracts
    // inside the closed spec-call cage while the backend resolves the built-ins.
    "old",
    "final",
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
    "bytes_eq",
];

/// `thermite-spec`'s own error enum (workspace.md REQ-3), born with this first
/// fallible function. Span-bearing (reusing `thermite_syntax::Span`) so
/// diagnostics are crisp (pillar 4); `Display`-able. The validator never panics
/// (R-CODE-2 / R-APG-1): every rejection is a variant here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecError {
    /// A fixed-array length names no package-visible `const NAME: usize = ...`.
    UnknownArrayCapacity { name: String, span: Span },
    /// Two top-level capacity declarations have the same name. Package parsing
    /// catches this across modules; this validator also closes the single-file
    /// path before lowering.
    DuplicateArrayCapacity { name: String, span: Span },
    /// One fixed-array capacity exceeds the language/tooling bound.
    ArrayCapacityTooLarge {
        value: u128,
        limit: u128,
        span: Span,
    },
    /// A nested array's recursively expanded native element count exceeds the
    /// same deterministic tooling bound.
    ArrayExpandedSizeTooLarge {
        elements: u128,
        limit: u128,
        span: Span,
    },
    /// An exact or repeat initializer disagrees with its annotated array type.
    ArrayLengthMismatch {
        expected: u128,
        found: u128,
        span: Span,
    },
    /// A repeat initializer `[value; N]` has an element type that is not
    /// copy-safe. Native array repetition evaluates one value and duplicates it;
    /// admitting owned strings, vectors, boxes, maps, mutable references, or
    /// opaque user ADTs here would either fail only in a backend or silently
    /// change ownership semantics.
    ArrayRepeatRequiresCopy { span: Span },
    /// `.array_eq(other)` and `.array_same_except(other, index)` require two
    /// equally typed fixed arrays whose element has a finite structural equality
    /// derivation. Primitive scalars, unit, nested fixed arrays/tuples, and
    /// non-sealed/non-opaque acyclic structs are admitted. Authority-bearing,
    /// recursive, enum, reference, and heap-backed shapes fail closed.
    ArrayEqualityRequiresStructuralArrays { detail: String, span: Span },
    /// A direct named-record field assignment does not belong to the frozen
    /// finite lifecycle subset. The target must be `root.field`, the root must
    /// be an exclusive named-record borrow or a mutable typed local, and the
    /// exact record must be non-sealed with only finite plain fields.
    InvalidNamedRecordMutation { detail: String, span: Span },
    /// An executable call to a frozen `thermite::atomic::*` boundary uses an
    /// ordering expression that is not an exact `AtomicOrdering::Variant`
    /// literal, supplies an ordering forbidden for that operation, or has an
    /// arity that prevents the ordering positions from being checked. Atomic
    /// ordering is part of the machine operation, so this gate is deliberately
    /// fail-closed before lowering/code generation rather than relying on a
    /// backend panic or a dynamically selected Rust ordering.
    IllegalAtomicOrdering {
        operation: String,
        detail: String,
        span: Span,
    },
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
    /// `Expr::Call`) so the diagnostic identifies the construct.
    ForbiddenCall { detail: String, span: Span },
    /// A registered combinator call appearing inside another combinator's
    /// predicate-closure body — an anonymous nested quantifier (REQ-6). The
    /// flat-closure-fragment rule forbids it: a combinator's `Pred`-slot closure
    /// body is a flat predicate (comparisons, arithmetic, boolean/logical ops,
    /// field/index, casts/refs, literals/paths, bounded built-in method calls,
    /// `Match`/`If`, and named `spec fn` calls) and may not compose another
    /// bounded quantifier. The sanctioned alternative is extracting a named
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
    /// (`.design/basis/01-adts.md`). Retained from Stage 1a as the
    /// "handled-or-loud" refusal for ADT forms the validator still does not check,
    /// but Stage 1b no longer fires it for a well-formed ADT: `struct`/`enum`
    /// items, `Expr::StructLit`, `Expr::Is`, and `Expr::Deref` are now
    /// validated (exhaustiveness REQ-5, well-formedness REQ-6) and accepted when
    /// well-formed. The variant stays in the enum so a future un-checkable ADT
    /// form has a structured refusal rather than a silent pass (the variant has
    /// no live emitter in 1b; `construct` names the unsupported surface form for
    /// a crisp diagnostic, §2.4).
    UnsupportedAdt { construct: &'static str, span: Span },
    /// A `match` over a declared `enum` value whose arms do not cover every
    /// declared variant and is not closed by a `Wildcard` arm
    /// (`.design/basis/01-adts.md` REQ-5). `missing` is the set of uncovered
    /// variant names, in the enum's declaration order (deterministic, R-CODE-5).
    /// This is the compile-time tooth of the handled-or-loud rule (REQ-12): a
    /// modeled outcome (variant) is left neither handled nor explicitly refused;
    /// the validator rejects it before the program ships.
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
    /// cage). The step closure body of a scheme is a flat per-node expression
    /// (comparisons, arithmetic, field/index, named `spec fn` calls) and may not
    /// compose another recursion scheme or a bounded combinator — that would be
    /// an anonymous nested structural quantifier the §4.2 cage forbids. The
    /// sanctioned alternative is a named `spec fn` (each `dec`-measured, in the
    /// audit surface). `name` is the nested scheme (or combinator). This is the
    /// scheme analogue of `NestedCombinator`, extending the cage's
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
    /// (`.design/basis/01-adts.md` REQ-2: "Variant names must be UpperCamelCase
    /// (uppercase-initial); the validator rejects a lowercase-initial variant
    /// declaration"). This is a soundness rule: the parser
    /// disambiguates a single-segment arm pattern by first-letter case
    /// (`Pattern::Enum` if uppercase-initial, `Pattern::Binding` otherwise).
    /// Forbidding lowercase variants makes that split sound — a lowercase ident
    /// in a pattern is then a binding, because no lowercase variant
    /// can exist, so a non-exhaustive `match` can never be silently masked by a
    /// variant-looking name being read as a catch-all binding (the #66 bypass).
    /// `name` is the offending variant. Rejected at the declaration pre-pass,
    /// before any `match`/exhaustiveness check.
    InvalidVariantCasing { name: String, span: Span },
    /// An `Expr::StructLit` constructing a `#[sealed]` clean/capability type
    /// (`.design/basis/06-provenance-and-sinks.md` REQ-8). A `#[sealed]` struct
    /// is the abstraction barrier for an IFC clean type (`Sql`/`Public`/
    /// `Authorized`): it is obtainable only as a `#[boundary]` door's return
    /// value (the door body is foreign/`external_body`, with no in-language
    /// `StructLit`), never minted directly. Minting one with a struct literal
    /// would launder a marked value into a clean type outside its door,
    /// defeating the IFC guarantee (the #77 SQLi/secret/capability bypass). The
    /// un-doored mark-change is a compile-time rejection, not a silent
    /// `L3`. `name` is the sealed struct.
    SealedConstruction { name: String, span: Span },
    /// A recursive exec `fn` (one whose body calls itself directly) that carries
    /// no `dec` termination clause and is not `fx diverge`
    /// (`.design/basis/10-recursion-tuples.md` REQ-2, C9-A). Termination is proved
    /// by default (§4.1): a self-calling `fn` must supply a `dec <measure>` so
    /// Verus can prove the recursion terminates, unless the fn declares `fx
    /// diverge` (the #88 exemption — a diverge fn is non-terminating,
    /// L1-capped). This is the surface-level mirror of the Verus rule "recursive
    /// function must have a decreases clause": Thermite reports it as its own
    /// span-bearing diagnostic so the user never reaches a raw Verus error, and a
    /// non-terminating fn can never reach L3 (the no-proof-cheat
    /// guarantee, `goal.md` R-DEFER-9). `name` is the self-recursive fn.
    MissingDecreases { name: String, span: Span },
    /// Basis Stage 7 strings (`.design/basis/07-strings.md` REQ-4, blocker #130): a
    /// user-declared `fn`/`spec fn` whose name begins with the lowerer's reserved
    /// prefix (`__thermite_`). The lowerer mints its generated byte-view helpers (the
    /// C4 numfmt round-trip, the C7 parser, the C5 search/transform spec fns + their
    /// proof lemmas) under that prefix so they never collide with a user name; the
    /// namespace is therefore the toolchain's alone. A user declaration in it is
    /// rejected here (rather than risking an E0428 double-definition in the lowered
    /// Verus source), closing the byte-view name-collision class at the surface.
    /// `name` is the offending user fn.
    ReservedName { name: String, span: Span },
}

impl SpecError {
    /// The source span this diagnostic points at.
    pub fn span(&self) -> Span {
        match self {
            SpecError::UnknownArrayCapacity { span, .. }
            | SpecError::DuplicateArrayCapacity { span, .. }
            | SpecError::ArrayCapacityTooLarge { span, .. }
            | SpecError::ArrayExpandedSizeTooLarge { span, .. }
            | SpecError::ArrayLengthMismatch { span, .. }
            | SpecError::ArrayRepeatRequiresCopy { span }
            | SpecError::ArrayEqualityRequiresStructuralArrays { span, .. }
            | SpecError::InvalidNamedRecordMutation { span, .. }
            | SpecError::IllegalAtomicOrdering { span, .. }
            | SpecError::UnknownCombinator { span, .. }
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
            | SpecError::MissingDecreases { span, .. }
            | SpecError::ReservedName { span, .. } => *span,
        }
    }
}

impl fmt::Display for SpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpecError::UnknownArrayCapacity { name, .. } => write!(
                f,
                "array capacity `{name}` is not a declared `const {name}: usize = INTEGER;`"
            ),
            SpecError::DuplicateArrayCapacity { name, .. } => {
                write!(f, "array capacity `{name}` is declared more than once")
            }
            SpecError::ArrayCapacityTooLarge { value, limit, .. } => write!(
                f,
                "array capacity {value} exceeds the fixed-array limit of {limit}"
            ),
            SpecError::ArrayExpandedSizeTooLarge {
                elements, limit, ..
            } => write!(
                f,
                "nested array expands to {elements} elements, exceeding the fixed-array limit of {limit}"
            ),
            SpecError::ArrayLengthMismatch {
                expected, found, ..
            } => write!(
                f,
                "array initializer has length {found}, but its annotated type requires {expected}"
            ),
            SpecError::ArrayRepeatRequiresCopy { .. } => write!(
                f,
                "array repeat initialization `[value; N]` requires a copy-safe element type"
            ),
            SpecError::ArrayEqualityRequiresStructuralArrays { detail, .. } => write!(
                f,
                "fixed-array relations require equally typed structurally comparable arrays: {detail}"
            ),
            SpecError::InvalidNamedRecordMutation { detail, .. } => write!(
                f,
                "named-record field mutation is outside the frozen exclusive lifecycle subset: {detail}"
            ),
            SpecError::IllegalAtomicOrdering {
                operation, detail, ..
            } => write!(
                f,
                "atomic operation `{operation}` has an illegal ordering: {detail}"
            ),
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
            SpecError::ReservedName { name, .. } => write!(
                f,
                "name `{name}` is reserved — the `__thermite_` prefix is the toolchain's own \
                 namespace for generated byte-view helpers (`.design/basis/07-strings.md` REQ-4); \
                 a user `fn`/`spec fn` may not declare a name in it"
            ),
        }
    }
}

impl std::error::Error for SpecError {}

/// Whether native `[value; N]` repetition can duplicate one element without
/// cloning, allocation, aliasing mutable authority, or depending on an opaque
/// user type's backend traits. This deliberately mirrors the closed Thermite
/// type surface rather than asking Rust/Verus after lowering.
fn array_repeat_element_is_copy_safe(ty: &Type) -> bool {
    match ty {
        Type::Prim(_) | Type::Unit => true,
        Type::Array { elem, .. } | Type::Option(elem) => array_repeat_element_is_copy_safe(elem),
        Type::Result(ok, err) => {
            array_repeat_element_is_copy_safe(ok) && array_repeat_element_is_copy_safe(err)
        }
        Type::Tuple(elements) => elements.iter().all(array_repeat_element_is_copy_safe),
        Type::Ref { mutable, .. } => !mutable,
        Type::Slice(_)
        | Type::Generic { .. }
        | Type::Named(_)
        | Type::Box(_)
        | Type::Vec(_)
        | Type::String
        | Type::Map(_, _) => false,
    }
}

/// Compute the declaration-order-independent closure of plain structs whose
/// values have a finite compiler-derived structural equality. This is the
/// shared source of truth for the validator and lowering: a struct joins only
/// after every named field dependency has already joined. Recursive cycles
/// therefore never enter the least fixed point. Sealed and opaque structs are
/// excluded even when their representation is scalar, because ambient derived
/// equality would weaken their authority/representation barrier.
pub fn structural_array_equality_structs(program: &Program) -> BTreeSet<String> {
    let mut admitted = BTreeSet::new();
    loop {
        let before = admitted.len();
        for item in &program.items {
            let Item::Struct(structure) = item else {
                continue;
            };
            if structure.sealed || structure.opaque || admitted.contains(&structure.name) {
                continue;
            }
            if structure
                .fields
                .iter()
                .all(|field| array_equality_type_is_structural(&field.ty, &admitted))
            {
                admitted.insert(structure.name.clone());
            }
        }
        if admitted.len() == before {
            return admitted;
        }
    }
}

/// Whether one array element type belongs to the finite structural equality
/// language. Kept public for lowering so code generation cannot silently admit
/// a wider type class than validation.
pub fn array_equality_type_is_structural(ty: &Type, admitted_structs: &BTreeSet<String>) -> bool {
    match ty {
        Type::Prim(_) | Type::Unit => true,
        Type::Array { elem, .. } => array_equality_type_is_structural(elem, admitted_structs),
        Type::Tuple(elements) => elements
            .iter()
            .all(|element| array_equality_type_is_structural(element, admitted_structs)),
        Type::Named(name) => admitted_structs.contains(name),
        Type::Ref { .. }
        | Type::Slice(_)
        | Type::Generic { .. }
        | Type::Box(_)
        | Type::Vec(_)
        | Type::String
        | Type::Option(_)
        | Type::Result(_, _)
        | Type::Map(_, _) => false,
    }
}

/// Compute the named-record roots whose direct fields may participate in the
/// exact exclusive-borrow lifecycle primitive. Ordinary finite structs are the
/// structural equality closure itself. An opaque root is additionally admitted
/// when each of its fields belongs to that closure: its declaring module needs
/// to implement verified state transitions, but opaque values never become
/// ambiently comparable or valid as nested derived state. Sealed roots remain
/// excluded because their representation is boundary-owned authority.
pub fn structural_record_mutation_structs(program: &Program) -> BTreeSet<String> {
    let plain = structural_array_equality_structs(program);
    program
        .items
        .iter()
        .filter_map(|item| {
            let Item::Struct(structure) = item else {
                return None;
            };
            (!structure.sealed
                && structure
                    .fields
                    .iter()
                    .all(|field| array_equality_type_is_structural(&field.ty, &plain)))
            .then(|| structure.name.clone())
        })
        .collect()
}

/// Validate every contract position of a parsed program against the SpecTherm
/// cage (REQ-3). Returns `Ok(())` if every contract expression is accepted, else
/// `Err` with one `SpecError` per violation (accumulated, not first-stop, for
/// crisp feedback, §2.4). Never panics (REQ-4/REQ-5).
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

/// The ordering-sensitive shapes in the frozen atomic boundary namespace.
/// Initialization has no ordering argument and therefore needs no entry here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AtomicOperation {
    Load,
    Store,
    ReadModifyWrite,
    CompareExchange,
    Fence,
}

impl AtomicOperation {
    fn expected_arity(self) -> usize {
        match self {
            AtomicOperation::Load => 2,
            AtomicOperation::Store | AtomicOperation::ReadModifyWrite => 3,
            AtomicOperation::CompareExchange => 5,
            AtomicOperation::Fence => 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AtomicOrdering {
    Relaxed,
    Acquire,
    Release,
    AcqRel,
    SeqCst,
}

impl AtomicOrdering {
    fn parse_literal(expr: &Expr) -> Option<Self> {
        let Expr::Path(path) = expr else {
            return None;
        };
        match path.as_slice() {
            [ty, variant] if ty == "AtomicOrdering" => match variant.as_str() {
                "Relaxed" => Some(AtomicOrdering::Relaxed),
                "Acquire" => Some(AtomicOrdering::Acquire),
                "Release" => Some(AtomicOrdering::Release),
                "AcqRel" => Some(AtomicOrdering::AcqRel),
                "SeqCst" => Some(AtomicOrdering::SeqCst),
                _ => None,
            },
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            AtomicOrdering::Relaxed => "Relaxed",
            AtomicOrdering::Acquire => "Acquire",
            AtomicOrdering::Release => "Release",
            AtomicOrdering::AcqRel => "AcqRel",
            AtomicOrdering::SeqCst => "SeqCst",
        }
    }
}

/// Recognize only exact frozen boundary targets. This is intentionally keyed by
/// declaration metadata rather than the Thermite function name: a package may
/// rename an imported declaration, while an unrelated function named
/// `atomic_u64_load` must retain ordinary language semantics.
fn classify_atomic_boundary(target: &str) -> Option<AtomicOperation> {
    let suffix = target.strip_prefix("thermite::atomic::")?;
    if matches!(suffix, "compiler_fence" | "hardware_fence") {
        return Some(AtomicOperation::Fence);
    }

    let (width, operation) = suffix.split_once("::")?;
    if !matches!(width, "bool" | "u32" | "u64" | "usize") {
        return None;
    }
    match operation {
        "load" => Some(AtomicOperation::Load),
        "store" => Some(AtomicOperation::Store),
        "compare_exchange" | "compare_exchange_weak" => Some(AtomicOperation::CompareExchange),
        "swap" | "fetch_add" | "fetch_sub" | "fetch_and" | "fetch_or" | "fetch_xor"
        | "fetch_min" | "fetch_max" => Some(AtomicOperation::ReadModifyWrite),
        _ => None,
    }
}

/// The walk state: the declared `spec fn` name set, the current recursion depth,
/// the accumulated diagnostics, and the "caged-flat" mode flag (REQ-6).
struct Validator {
    spec_fns: HashSet<String>,
    /// Thermite declaration name -> exact frozen atomic operation shape. The
    /// map contains only declarations carrying a recognized
    /// `#[boundary("thermite::atomic::...")]` target.
    atomic_functions: HashMap<String, AtomicOperation>,
    /// Closed, literal-valued package capacity declarations. The parser admits
    /// no expressions here, so resolution cannot be cyclic or target-dependent.
    array_capacities: HashMap<String, u128>,
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
    /// design admits — an accessed field must be declared somewhere; a name no
    /// struct/struct-variant declares is `UnknownField`.
    struct_fields: HashSet<String>,
    /// REQ-8 (`.design/basis/06-provenance-and-sinks.md`): the names of every
    /// `#[sealed]` `struct` (collected from `Item::Struct` with `sealed == true`
    /// in the pre-pass, alongside `struct_fields`). The abstraction-barrier rule
    /// rejects any `Expr::StructLit` whose `path` resolves to a name in this set
    /// (`SealedConstruction`); a sealed clean/capability type is door-only-
    /// mintable. Inert when no `#[sealed]` struct is declared (the non-IFC corpus
    /// is unchanged), like `struct_fields`.
    sealed_structs: HashSet<String>,
    /// Least fixed point of ordinary acyclic struct declarations whose fields
    /// all have finite structural equality. Used only by the explicit fixed-
    /// array relation built-ins; it grants no ambient equality operation.
    array_equality_structs: BTreeSet<String>,
    /// Exact struct-name -> declared direct field names. Unlike the historical
    /// global `struct_fields` set, named-record mutation must resolve a field on
    /// the receiver's actual record type before code generation.
    record_fields: HashMap<String, HashSet<String>>,
    /// Finite non-sealed roots admitted by the direct named-record lifecycle
    /// primitive. Opaque roots may join; opaque representation ownership across
    /// package modules is enforced by Forge's package resolver.
    record_mutation_structs: BTreeSet<String>,
    /// Types of the current item's named parameters, result, fields, and typed
    /// locals. The dedicated fixed-array equality check uses this small lexical
    /// environment because the general v0.1 validator is otherwise intentionally
    /// not a full type checker.
    array_equality_types: HashMap<String, Type>,
    /// Source bindings that may be assigned through. Parameters join only when
    /// they are exclusive references; typed locals join only for `let mut`.
    mutable_bindings: HashSet<String>,
    depth: usize,
    errors: Vec<SpecError>,
    /// REQ-6 flat-closure-fragment mode. Set once on entry to a combinator's
    /// `Pred`-slot closure body and kept set for all nested sub-expressions and
    /// nested closures within it. While set, a call resolving to a registered
    /// combinator (`combinators::lookup(name).is_some()`) is rejected with
    /// `NestedCombinator` (an anonymous nested quantifier); a named `spec fn`
    /// call stays accepted (named composition). In a top-level contract position
    /// (flag clear) a combinator call is accepted as before (REQ-3 (a)).
    in_combinator_closure: bool,
    /// REQ-2/REQ-4 flat-scheme-step mode (`.design/basis/02-recursion-schemes.md`).
    /// Set once on entry to a recursion scheme's step closure body and kept set
    /// for all nested sub-expressions within it. While set, a call resolving to a
    /// registered scheme (`schemes::lookup`) or a registered combinator
    /// (`combinators::lookup`) is rejected with `NestedScheme`: the step body is
    /// a flat per-node expression and may not compose another structural
    /// quantifier (a named `spec fn` call stays accepted). This extends the
    /// existing combinator-closure cage (the `in_combinator_closure` rule) to
    /// scheme steps, as the design's REQ-4 mandates.
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
                Item::Const(_) | Item::Fn(_) => None,
                // A `struct`/`enum` item declares no `spec fn` name
                // (`.design/basis/01-adts.md`). The ADT declarations are
                // collected separately below.
                Item::Struct(_) | Item::Enum(_) => None,
                // A Stage-1 forge-tier item (`.design/stage1-forge-tier.md` REQ-3)
                // declares no `spec fn` name. A `prop fn` IS a forge-tier definition,
                // but it is consumed by the forge increments (2b–3), not the v1 spec
                // validator — it is not seeded as a v1 spec-fn callee here.
                Item::Forge(_) => None,
            })
            .collect();
        // Cluster C4 strings (`.design/basis/07-strings.md` REQ-8, issue #94): seed
        // the generated round-trip spec fns (`parse_le`/`pow10`) so a contract
        // `ens parse_le(result) == n` validates inside the §4.2 cage as a named
        // `spec fn` call (the lowerer materializes their bodies). These are reserved
        // names the lowerer owns — accepted as a user-declared spec fn.
        for name in GENERATED_SPEC_FNS {
            spec_fns.insert((*name).to_string());
        }

        let atomic_functions: HashMap<String, AtomicOperation> = program
            .items
            .iter()
            .filter_map(|item| {
                let Item::Fn(function) = item else {
                    return None;
                };
                let boundary = function.boundary.as_ref()?;
                classify_atomic_boundary(&boundary.target)
                    .map(|operation| (function.name.clone(), operation))
            })
            .collect();

        // The ADT declaration pre-pass (`.design/basis/01-adts.md` REQ-5/REQ-6;
        // mirrors the spec-fn-name collection above). A program references types
        // across items in any order (`fn f(s: Shape)` may precede `enum Shape`),
        // so the body/contract walk must see every declared type before walking
        // any body, order-independent, like the spec-fn resolution.
        let mut enums: HashMap<String, Vec<String>> = HashMap::new();
        let mut variant_to_enum: HashMap<String, String> = HashMap::new();
        let mut struct_fields: HashSet<String> = HashSet::new();
        let mut record_fields: HashMap<String, HashSet<String>> = HashMap::new();
        // REQ-8 (`.design/basis/06-provenance-and-sinks.md`): the `#[sealed]`
        // clean/capability struct names — the abstraction barrier the
        // `Expr::StructLit` walk keys off to REJECT a direct mint. Collected in
        // the same pre-pass as `struct_fields` so a forward reference (`fn
        // f() { Sql { … } }` before `#[sealed] struct Sql`) is seen.
        let mut sealed_structs: HashSet<String> = HashSet::new();
        // `.design/basis/01-adts.md` REQ-2: every `enum` variant name must be
        // UpperCamelCase (uppercase-initial). A lowercase-initial variant is
        // rejected here, at the declaration pre-pass, before any
        // match/exhaustiveness check. This is the cause of the #66 bypass: the
        // parser disambiguates a single-segment arm pattern by first-letter case
        // (uppercase → `Pattern::Enum`, lowercase → `Pattern::Binding`), so a
        // lowercase variant in a `match` arm masquerades as a catch-all binding
        // and a non-exhaustive match is silently accepted. Forbidding lowercase
        // variants at the declaration makes that case-based split sound. These
        // casing diagnostics seed the validator's error list so a lowercase-
        // variant program never reaches the (now-sound) body/contract walk.
        let mut prepass_errors: Vec<SpecError> = Vec::new();
        let mut array_capacities: HashMap<String, u128> = HashMap::new();
        for item in &program.items {
            match item {
                Item::Const(c) => {
                    if array_capacities.insert(c.name.clone(), c.value).is_some() {
                        prepass_errors.push(SpecError::DuplicateArrayCapacity {
                            name: c.name.clone(),
                            span: c.span,
                        });
                    }
                    if c.value > MAX_FIXED_ARRAY_ELEMENTS {
                        prepass_errors.push(SpecError::ArrayCapacityTooLarge {
                            value: c.value,
                            limit: MAX_FIXED_ARRAY_ELEMENTS,
                            span: c.span,
                        });
                    }
                }
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
                            prepass_errors.push(SpecError::InvalidVariantCasing {
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
                        // access is checked against struct and struct-variant
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
                    let exact_fields = record_fields.entry(s.name.clone()).or_default();
                    for field in &s.fields {
                        struct_fields.insert(field.name.clone());
                        exact_fields.insert(field.name.clone());
                    }
                    // REQ-8: a `#[sealed]` struct joins the abstraction-barrier
                    // set — its name will reject any `StructLit` mint.
                    if s.sealed {
                        sealed_structs.insert(s.name.clone());
                    }
                }
                Item::Fn(_) | Item::SpecFn(_) => {}
                // A forge-tier item (`.design/stage1-forge-tier.md` REQ-3) declares
                // no enum/struct, so it raises no variant-casing concern here.
                Item::Forge(_) => {}
            }
        }

        // Cluster C7 (`.design/basis/09-option-result.md` REQ-1/REQ-2/REQ-3, issue
        // #95): seed the built-in variants `Some`/`None` (enum `Option`) and
        // `Ok`/`Err` (enum `Result`) into the same `enums`/`variant_to_enum`
        // registry the user-`Item::Enum` pre-pass fills. This is the one validator
        // change C7 needs: with the built-in variants registered, construction
        // (`Some(v)`/`Ok(v)` reach the exec-body walk's `Call`/`Path` recursion, no
        // longer an `UnknownVariant`), `match` arms over them (`check_match_-
        // exhaustiveness` now infers `Option`/`Result` and requires both arms or a
        // wildcard), and `is` discrimination (`r is Some` via `check_variant_ref`)
        // are all accepted, as a user enum (01-adts REQ-5/REQ-6). The
        // declaration order pins the exhaustiveness `missing` set: `Option` is
        // `[Some, None]`, `Result` is `[Ok, Err]`. The spec-`match`-in-`ens` payload
        // projection needs no new cage rule: `walk_expr_inner`'s `Match` arm
        // already admits a flat `match` as a built-in (01-adts REQ-7), so a
        // `match result { Some(v) => <flat pred>, None => true }` in a contract is
        // already an accepted flat predicate once the variants are registered. A
        // user `enum` named `Option`/`Result` (or a variant `Some`/`Ok`/…) is a
        // re-declaration; last-writer-wins matches the existing duplicate-variant
        // policy (the built-ins are seeded first, so a user re-decl overrides; the
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
            atomic_functions,
            array_capacities,
            enums,
            variant_to_enum,
            struct_fields,
            sealed_structs,
            array_equality_structs: structural_array_equality_structs(program),
            record_fields,
            record_mutation_structs: structural_record_mutation_structs(program),
            array_equality_types: HashMap::new(),
            mutable_bindings: HashSet::new(),
            depth: 0,
            // REQ-2: lowercase-variant casing diagnostics from the pre-pass seed
            // the error list, so a lowercase-variant `enum` is rejected at the
            // declaration before the (now-sound) match/exhaustiveness walk runs.
            errors: prepass_errors,
            in_combinator_closure: false,
            in_scheme_step: false,
        }
    }

    /// Walk every contract position of every item.
    fn run(&mut self, program: &Program) {
        for item in &program.items {
            // Basis Stage 7 (`.design/basis/07-strings.md` REQ-4, blocker #130): a
            // user `fn`/`spec fn` may not declare a name in the lowerer's reserved
            // namespace (`__thermite_`), where the generated byte-view helpers live,
            // so a user name can never collide with a generated def. Checked once per
            // item before its contract/body walk; a clash is `ReservedName`.
            let declared = match item {
                Item::Const(_) => None,
                Item::Fn(f) => Some((&f.name, f.span)),
                Item::SpecFn(s) => Some((&s.name, s.span)),
                Item::Struct(_) | Item::Enum(_) => None,
                // A forge-tier item (`.design/stage1-forge-tier.md` REQ-3) does not
                // lower into the reserved `__thermite_` namespace in v1 (its
                // consumers are increments 2b–3), so it is not reserved-name checked
                // here.
                Item::Forge(_) => None,
            };
            if let Some((name, span)) = declared {
                if name.starts_with(THERMITE_RESERVED_PREFIX) {
                    self.errors.push(SpecError::ReservedName {
                        name: name.clone(),
                        span,
                    });
                }
            }
            self.array_equality_types.clear();
            self.mutable_bindings.clear();
            match item {
                Item::Const(_) => {}
                Item::Fn(f) => {
                    for param in &f.params {
                        self.array_equality_types
                            .insert(param.name.clone(), param.ty.clone());
                        if matches!(&param.ty, Type::Ref { mutable: true, .. }) {
                            self.mutable_bindings.insert(param.name.clone());
                        }
                    }
                    self.array_equality_types
                        .insert("result".to_string(), f.ret.clone());
                    for param in &f.params {
                        self.validate_type(&param.ty, f.span);
                    }
                    self.validate_type(&f.ret, f.span);
                    self.walk_clause(&f.contract.req);
                    for clause in &f.contract.ens {
                        self.walk_clause(clause);
                    }
                    // REQ-3: a `fn` body is executable surface code, not a
                    // contract position. We traverse it structurally only, to
                    // find nested `LoopNode`s and cage each loop's `invs`/`dec`
                    // (the only contract positions inside a body). The body's
                    // other expressions (`return Some(mid)`, `haystack[mid]`,
                    // assignments, …) are surface code and are not cage-checked.
                    // A boundary fn (ffi-boundary.md REQ-2) has `body: None` — the
                    // body is foreign, so there are no in-language loops to scan
                    // for caged `inv`/`dec` clauses. Its `req`/`ens` (walked above)
                    // are still fully caged. An in-language fn's body is scanned
                    // structurally as before.
                    if let Some(body) = &f.body {
                        self.scan_block_for_loops(body, f.span);
                        if let Some(tail) = &body.tail {
                            self.validate_array_initializer(&f.ret, tail, f.span);
                        }
                        // C9-A (`.design/basis/10-recursion-tuples.md` REQ-2): a
                        // recursive exec `fn` — one whose body calls itself directly
                        // — must carry a `dec` termination measure so Verus can prove
                        // the recursion terminates (§4.1 "Termination is proved by
                        // default"), unless it declares `fx diverge` (the #88
                        // exemption — a diverge fn is non-terminating,
                        // L1-capped). A self-call without `dec` and not `fx diverge`
                        // is a structured `MissingDecreases`, the surface mirror of
                        // the Verus rule "recursive function must have a decreases
                        // clause", so a non-terminating fn never reaches an L3 cert
                        // (R-DEFER-9). Mutual recursion (REQ-6) is out of v1: a pair
                        // that calls each other (neither calls itself directly) is not
                        // a direct self-call, so it is not flagged here; it reaches
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
                    for param in &s.params {
                        self.array_equality_types
                            .insert(param.name.clone(), param.ty.clone());
                        if matches!(&param.ty, Type::Ref { mutable: true, .. }) {
                            self.mutable_bindings.insert(param.name.clone());
                        }
                    }
                    self.array_equality_types
                        .insert("result".to_string(), s.ret.clone());
                    for param in &s.params {
                        self.validate_type(&param.ty, s.span);
                    }
                    self.validate_type(&s.ret, s.span);
                    // A `spec fn` body is itself a contract-position expression
                    // tree (REQ-3) — fully caged; its `dec` measure is a clause.
                    self.walk_clause(&s.dec);
                    self.walk_block(&s.body, s.span);
                    if let Some(tail) = &s.body.tail {
                        self.validate_array_initializer(&s.ret, tail, s.span);
                    }
                }
                // Basis Stage 1b (`.design/basis/01-adts.md` REQ-5/REQ-6): the
                // `struct`/`enum` declarations were collected in the pre-pass
                // (`Validator::new`). A `struct`'s type-invariant `inv` clause is
                // a contract position (REQ-1: Verus enforces it at construction /
                // use); it is fully caged here like a `req`/`ens`,
                // including its `Field` access well-formedness (REQ-6). An `enum`
                // item carries no contract position of its own; its variant set
                // (collected above) drives the exhaustiveness/`is` checks at the
                // `match`/`is` sites. The 1a `UnsupportedAdt` gate is gone: a
                // well-formed ADT now validates.
                Item::Struct(s) => {
                    for field in &s.fields {
                        self.array_equality_types
                            .insert(field.name.clone(), field.ty.clone());
                    }
                    for field in &s.fields {
                        self.validate_type(&field.ty, s.span);
                    }
                    if let Some(inv) = &s.inv {
                        self.walk_clause(inv);
                    }
                }
                Item::Enum(e) => {
                    for variant in &e.variants {
                        match &variant.shape {
                            VariantShape::Unit => {}
                            VariantShape::Tuple(types) => {
                                for ty in types {
                                    self.validate_type(ty, e.span);
                                }
                            }
                            VariantShape::Struct(fields) => {
                                for field in fields {
                                    self.validate_type(&field.ty, e.span);
                                }
                            }
                        }
                    }
                }
                // A Stage-1 forge-tier item (`.design/stage1-forge-tier.md` REQ-3):
                // its contract/proof positions (`prop fn` body, `lemma`/`proof`
                // clauses + proof blocks, `witness` directives) are consumed by the
                // forge increments (2b covenant, 2c battery, 2e proof view, 3
                // library), not the v1 spec cage. No v1 contract walk applies here;
                // the surface is parse/address/round-trip tested in thermite-syntax.
                Item::Forge(forge) => self.validate_forge_types(forge),
            }
        }
    }

    fn validate_forge_types(&mut self, forge: &ForgeItem) {
        match forge {
            ForgeItem::PropFn(item) => {
                for param in &item.params {
                    self.validate_type(&param.ty, item.span);
                }
                self.validate_type(&item.ret, item.span);
            }
            ForgeItem::Lemma(item) => {
                for param in &item.params {
                    self.validate_type(&param.ty, item.span);
                }
            }
            ForgeItem::Proof(_) | ForgeItem::Witness(_) => {}
        }
    }

    fn resolve_array_len(&mut self, len: &ArrayLen, span: Span) -> Option<u128> {
        let value = match len {
            ArrayLen::Literal { value, .. } => *value,
            ArrayLen::Const(name) => match self.array_capacities.get(name) {
                Some(value) => *value,
                None => {
                    self.errors.push(SpecError::UnknownArrayCapacity {
                        name: name.clone(),
                        span,
                    });
                    return None;
                }
            },
        };
        if value > MAX_FIXED_ARRAY_ELEMENTS {
            if matches!(len, ArrayLen::Literal { .. }) {
                self.errors.push(SpecError::ArrayCapacityTooLarge {
                    value,
                    limit: MAX_FIXED_ARRAY_ELEMENTS,
                    span,
                });
            }
            None
        } else {
            Some(value)
        }
    }

    /// Validate every capacity in a type and return its recursively expanded
    /// native element count. Non-array leaves count as one storage element.
    fn validate_type(&mut self, ty: &Type, span: Span) -> Option<u128> {
        match ty {
            Type::Array { elem, len } => {
                let len = self.resolve_array_len(len, span)?;
                let inner = self.validate_type(elem, span)?;
                let elements = len.saturating_mul(inner);
                if elements > MAX_FIXED_ARRAY_ELEMENTS {
                    self.errors.push(SpecError::ArrayExpandedSizeTooLarge {
                        elements,
                        limit: MAX_FIXED_ARRAY_ELEMENTS,
                        span,
                    });
                    None
                } else {
                    Some(elements)
                }
            }
            Type::Ref { inner, .. }
            | Type::Slice(inner)
            | Type::Generic { arg: inner, .. }
            | Type::Box(inner)
            | Type::Vec(inner)
            | Type::Option(inner) => self.validate_type(inner, span),
            Type::Result(ok, err) | Type::Map(ok, err) => {
                let left = self.validate_type(ok, span);
                let right = self.validate_type(err, span);
                left.zip(right).map(|_| 1)
            }
            Type::Tuple(types) => {
                let mut valid = true;
                for ty in types {
                    valid &= self.validate_type(ty, span).is_some();
                }
                valid.then_some(1)
            }
            Type::Prim(_) | Type::Unit | Type::Named(_) | Type::String => Some(1),
        }
    }

    fn validate_array_initializer(&mut self, ty: &Type, init: &Expr, span: Span) {
        let Type::Array { elem, len } = ty else {
            return;
        };
        let Some(expected) = self.resolve_array_len(len, span) else {
            return;
        };
        let found = match init {
            Expr::Array(elements) => Some(elements.len() as u128),
            Expr::ArrayRepeat { len, .. } => self.resolve_array_len(len, span),
            _ => None,
        };
        if let Some(found) = found {
            if found != expected {
                self.errors.push(SpecError::ArrayLengthMismatch {
                    expected,
                    found,
                    span,
                });
            }
        }

        match init {
            Expr::Array(elements) => {
                for element in elements {
                    self.validate_array_initializer(elem, element, span);
                }
            }
            Expr::ArrayRepeat { value, .. } => {
                if !array_repeat_element_is_copy_safe(elem) {
                    self.errors
                        .push(SpecError::ArrayRepeatRequiresCopy { span });
                }
                self.validate_array_initializer(elem, value, span);
            }
            _ => {}
        }
    }

    fn array_equality_operand_type<'b>(&'b self, expr: &Expr) -> Option<&'b Type> {
        let ty = match expr {
            Expr::Path(segments) if segments.len() == 1 => {
                self.array_equality_types.get(&segments[0])?
            }
            Expr::Ref { expr, .. } | Expr::Deref(expr) => {
                return self.array_equality_operand_type(expr);
            }
            _ => return None,
        };
        let mut current = ty;
        while let Type::Ref { inner, .. } = current {
            current = inner;
        }
        Some(current)
    }

    fn check_array_relation_call(
        &mut self,
        name: &str,
        receiver: &Expr,
        args: &[Expr],
        span: Span,
    ) {
        let expected_arity = if name == "array_eq" { 1 } else { 2 };
        let valid = if args.len() == expected_arity {
            let right = &args[0];
            match (
                self.array_equality_operand_type(receiver),
                self.array_equality_operand_type(right),
            ) {
                (Some(left @ Type::Array { elem, .. }), Some(right @ Type::Array { .. })) => {
                    array_equality_type_is_structural(elem, &self.array_equality_structs)
                        && left == right
                }
                _ => false,
            }
        } else {
            false
        };
        if !valid {
            self.errors
                .push(SpecError::ArrayEqualityRequiresStructuralArrays {
                    detail: if args.len() == expected_arity {
                        "both operands must be named arrays with the same structural element type and capacity; sealed, opaque, recursive, enum, reference, and heap-backed elements are not derived"
                            .to_string()
                    } else {
                        format!(
                            "`.{name}()` expects {expected_arity} argument(s), found {}",
                            args.len()
                        )
                    },
                    span,
                });
        }
    }

    /// Validate the frozen direct named-record assignment target. This is
    /// deliberately narrower than Rust's lvalue grammar: independent lifecycle
    /// TV models one exact direct field cell, so nested projections and unknown
    /// receiver types fail before lowering instead of becoming backend behavior.
    fn check_named_record_assignment(&mut self, target: &Expr, span: Span) {
        let Expr::Field { receiver, name } = target else {
            return;
        };
        let Expr::Path(path) = receiver.as_ref() else {
            self.errors.push(SpecError::InvalidNamedRecordMutation {
                detail: "the target must be exactly `root.field`; nested field, index, dereference, and computed receivers are not in the direct lifecycle subset"
                    .to_string(),
                span,
            });
            return;
        };
        let [root] = path.as_slice() else {
            self.errors.push(SpecError::InvalidNamedRecordMutation {
                detail: "the record root must be one local or parameter name".to_string(),
                span,
            });
            return;
        };
        let Some(root_ty) = self.array_equality_types.get(root) else {
            self.errors.push(SpecError::InvalidNamedRecordMutation {
                detail: format!(
                    "record root `{root}` has no declared source type; use a typed `let mut` or an `&mut Name` parameter"
                ),
                span,
            });
            return;
        };
        let record_name = match root_ty {
            Type::Named(record) => record,
            Type::Ref { inner, .. } => match inner.as_ref() {
                Type::Named(record) => record,
                _ => {
                    self.errors.push(SpecError::InvalidNamedRecordMutation {
                        detail: format!(
                            "record root `{root}` is not a named record value or borrow"
                        ),
                        span,
                    });
                    return;
                }
            },
            _ => {
                self.errors.push(SpecError::InvalidNamedRecordMutation {
                    detail: format!("record root `{root}` is not a named record value or borrow"),
                    span,
                });
                return;
            }
        };
        if !self.mutable_bindings.contains(root) {
            self.errors.push(SpecError::InvalidNamedRecordMutation {
                detail: format!(
                    "record root `{root}` is not writable; use an exclusive `&mut {record_name}` parameter or a typed `let mut`"
                ),
                span,
            });
            return;
        }
        if !self.record_mutation_structs.contains(record_name) {
            self.errors.push(SpecError::InvalidNamedRecordMutation {
                detail: format!(
                    "record `{record_name}` is sealed, recursive, or contains a reference, enum, opaque nested record, or heap-backed field"
                ),
                span,
            });
            return;
        }
        if !self
            .record_fields
            .get(record_name)
            .is_some_and(|fields| fields.contains(name))
        {
            self.errors.push(SpecError::InvalidNamedRecordMutation {
                detail: format!(
                    "field `{name}` is not declared by the receiver's exact record type `{record_name}`"
                ),
                span,
            });
        }
    }

    fn check_u64_bit_method_call(&mut self, name: &str, args: &[Expr], span: Span) {
        let expected = if matches!(name, "bit_test" | "bit_set" | "bit_clear") {
            Some(1)
        } else if matches!(
            name,
            "bit_set_preserves_other" | "bit_clear_preserves_other"
        ) {
            Some(2)
        } else {
            None
        };
        if let Some(expected) = expected.filter(|expected| args.len() != *expected) {
            let expected_label = if expected == 1 {
                "one".to_string()
            } else {
                expected.to_string()
            };
            self.errors.push(SpecError::ForbiddenCall {
                detail: format!(
                    "`.{name}()` expects exactly {expected_label} bit-index argument(s), found {}",
                    args.len()
                ),
                span,
            });
        }
    }

    /// Run `inner` one recursion level deeper, returning `false` (and recording
    /// an `ExpressionTooDeep` at `span`) if the limit is hit. The single shared
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

    /// Structural traversal of a (non-caged) `fn` body block (REQ-3): descend
    /// through statements / nested blocks / `if` / `loop` only to find nested
    /// `LoopNode`s and cage each loop's `invs`/`dec` (recursively, for loops
    /// nested in loops). The block's own expressions — calls like `Some(mid)`,
    /// `return None`, assignments, `haystack[mid]` — are executable surface code
    /// and are not cage-checked here. This is the counterpart to the caged
    /// `walk_block` (used for `spec fn` bodies and caged sub-expressions): same
    /// shape walk, but it cage-checks only the loop contract clauses it
    /// discovers.
    fn scan_block_for_loops(&mut self, block: &Block, span: Span) {
        let outer_types = self.array_equality_types.clone();
        let outer_mutable = self.mutable_bindings.clone();
        for stmt in &block.stmts {
            self.scan_stmt_for_loops(stmt, span);
        }
        if let Some(tail) = &block.tail {
            self.scan_expr_for_loops(tail, span);
        }
        self.array_equality_types = outer_types;
        self.mutable_bindings = outer_mutable;
    }

    /// Structural traversal of a `fn`-body statement: cage the `invs`/`dec` of
    /// any nested loop (the only contract positions in a body) and keep
    /// descending through control flow to find deeper loops. Surface expressions
    /// are descended into only to reach nested loops (e.g. a `loop` inside an
    /// `if` block), never cage-checked.
    fn scan_stmt_for_loops(&mut self, stmt: &Stmt, span: Span) {
        match stmt {
            Stmt::Loop(loop_node) => {
                // The loop's `invs`/`dec` are contract positions — cage them.
                for inv in &loop_node.invs {
                    self.walk_clause(inv);
                }
                self.walk_clause(&loop_node.dec);
                // The loop body is still executable surface code: scan it
                // structurally for further nested loops, do not cage it.
                self.scan_block_for_loops(&loop_node.body, loop_node.span);
            }
            Stmt::Let {
                mutable,
                name,
                ty,
                init,
            } => {
                if let Some(ty) = ty {
                    self.validate_type(ty, span);
                    self.validate_array_initializer(ty, init, span);
                    self.array_equality_types.insert(name.clone(), ty.clone());
                    if *mutable {
                        self.mutable_bindings.insert(name.clone());
                    }
                }
                self.scan_expr_for_loops(init, span);
            }
            Stmt::Assign { target, value } => {
                self.check_named_record_assignment(target, span);
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

    /// Structural traversal of a `fn`-body expression. It descends to find
    /// nested `loop`s (caging each loop's `invs`/`dec`) and — Basis Stage 1b —
    /// applies the ADT well-formedness checks (REQ-5 exhaustiveness, REQ-6
    /// field/variant access) to every ADT node, because the validator rejecting
    /// a non-exhaustive `match` is the compile-time tooth (REQ-12) and a `match`
    /// over an enum lives in `fn`-body (exec) position, not a contract position.
    /// These ADT checks are not cage checks: the body's combinator/spec-fn
    /// resolution is still not performed here (a body `Some(mid)` call stays
    /// surface code). The two concerns are orthogonal: the cage gates contract
    /// positions; the ADT well-formedness gates every modeled-outcome site.
    /// `span` is the enclosing `fn`/loop span (the AST carries no per-`Expr`
    /// span). When no ADT is declared, every ADT check is inert, so the existing
    /// non-ADT corpus body walk (`binary_search.th`) is unchanged.
    fn check_atomic_ordering_call(&mut self, callee: &Expr, args: &[Expr], span: Span) {
        let Expr::Path(path) = callee else {
            return;
        };
        let [name] = path.as_slice() else {
            return;
        };
        let Some(operation) = self.atomic_functions.get(name).copied() else {
            return;
        };

        let expected = operation.expected_arity();
        if args.len() != expected {
            self.errors.push(SpecError::IllegalAtomicOrdering {
                operation: name.clone(),
                detail: format!(
                    "the frozen declaration expects {expected} argument(s), found {}",
                    args.len()
                ),
                span,
            });
            return;
        }

        let literal = |position: usize| AtomicOrdering::parse_literal(&args[position]);
        match operation {
            AtomicOperation::Load => match literal(1) {
                Some(
                    AtomicOrdering::Relaxed
                    | AtomicOrdering::Acquire
                    | AtomicOrdering::SeqCst,
                ) => {}
                Some(order) => self.errors.push(SpecError::IllegalAtomicOrdering {
                    operation: name.clone(),
                    detail: format!(
                        "load does not permit AtomicOrdering::{}; expected Relaxed, Acquire, or SeqCst",
                        order.name()
                    ),
                    span,
                }),
                None => self.errors.push(SpecError::IllegalAtomicOrdering {
                    operation: name.clone(),
                    detail: "load ordering must be an exact `AtomicOrdering::Relaxed`, `::Acquire`, or `::SeqCst` literal".to_string(),
                    span,
                }),
            },
            AtomicOperation::Store => match literal(2) {
                Some(
                    AtomicOrdering::Relaxed
                    | AtomicOrdering::Release
                    | AtomicOrdering::SeqCst,
                ) => {}
                Some(order) => self.errors.push(SpecError::IllegalAtomicOrdering {
                    operation: name.clone(),
                    detail: format!(
                        "store does not permit AtomicOrdering::{}; expected Relaxed, Release, or SeqCst",
                        order.name()
                    ),
                    span,
                }),
                None => self.errors.push(SpecError::IllegalAtomicOrdering {
                    operation: name.clone(),
                    detail: "store ordering must be an exact `AtomicOrdering::Relaxed`, `::Release`, or `::SeqCst` literal".to_string(),
                    span,
                }),
            },
            AtomicOperation::ReadModifyWrite => {
                if literal(2).is_none() {
                    self.errors.push(SpecError::IllegalAtomicOrdering {
                        operation: name.clone(),
                        detail: "read-modify-write ordering must be an exact `AtomicOrdering` variant literal".to_string(),
                        span,
                    });
                }
            }
            AtomicOperation::Fence => match literal(0) {
                Some(AtomicOrdering::Relaxed) => {
                    self.errors.push(SpecError::IllegalAtomicOrdering {
                        operation: name.clone(),
                        detail: "fences do not permit AtomicOrdering::Relaxed; expected Acquire, Release, AcqRel, or SeqCst".to_string(),
                        span,
                    })
                }
                Some(
                    AtomicOrdering::Acquire
                    | AtomicOrdering::Release
                    | AtomicOrdering::AcqRel
                    | AtomicOrdering::SeqCst,
                ) => {}
                None => self.errors.push(SpecError::IllegalAtomicOrdering {
                    operation: name.clone(),
                    detail: "fence ordering must be an exact non-Relaxed `AtomicOrdering` variant literal".to_string(),
                    span,
                }),
            },
            AtomicOperation::CompareExchange => {
                let success = literal(3);
                let failure = literal(4);
                let (Some(success), Some(failure)) = (success, failure) else {
                    self.errors.push(SpecError::IllegalAtomicOrdering {
                        operation: name.clone(),
                        detail: "compare-exchange success and failure orderings must both be exact `AtomicOrdering` variant literals".to_string(),
                        span,
                    });
                    return;
                };
                let legal = matches!(
                    (success, failure),
                    (AtomicOrdering::Relaxed, AtomicOrdering::Relaxed)
                        | (AtomicOrdering::Acquire, AtomicOrdering::Relaxed)
                        | (AtomicOrdering::Acquire, AtomicOrdering::Acquire)
                        | (AtomicOrdering::Release, AtomicOrdering::Relaxed)
                        | (AtomicOrdering::AcqRel, AtomicOrdering::Relaxed)
                        | (AtomicOrdering::AcqRel, AtomicOrdering::Acquire)
                        | (AtomicOrdering::SeqCst, AtomicOrdering::Relaxed)
                        | (AtomicOrdering::SeqCst, AtomicOrdering::Acquire)
                        | (AtomicOrdering::SeqCst, AtomicOrdering::SeqCst)
                );
                if !legal {
                    self.errors.push(SpecError::IllegalAtomicOrdering {
                        operation: name.clone(),
                        detail: format!(
                            "compare-exchange pair AtomicOrdering::{}/AtomicOrdering::{} is forbidden: failure may not be Release or AcqRel and may not be stronger than success",
                            success.name(),
                            failure.name()
                        ),
                        span,
                    });
                }
            }
        }
    }

    fn scan_expr_for_loops(&mut self, expr: &Expr, span: Span) {
        match expr {
            Expr::Array(elements) => {
                for element in elements {
                    self.scan_expr_for_loops(element, span);
                }
            }
            Expr::ArrayRepeat { value, len } => {
                self.resolve_array_len(len, span);
                self.scan_expr_for_loops(value, span);
            }
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
                for arm in arms {
                    // A C10 match guard is an `Expr` evaluated in the arm scope —
                    // scan it for loops too (`.design/basis/11-ergonomics.md`
                    // REQ-3), not just the body.
                    if let Some(guard) = &arm.guard {
                        self.scan_expr_for_loops(guard, span);
                    }
                    self.scan_expr_for_loops(&arm.body, span);
                }
            }
            Expr::Call { callee, args } => {
                let direct_atomic = match callee.as_ref() {
                    Expr::Path(path) => match path.as_slice() {
                        [name] => self.atomic_functions.contains_key(name),
                        _ => false,
                    },
                    _ => false,
                };
                self.check_atomic_ordering_call(callee, args, span);
                // Atomic boundary calls are intentionally first-order. If the
                // callee is not the recognized direct path, descend into it so
                // an attempted alias/function-value reference is rejected by
                // the Path arm below rather than bypassing ordering inspection.
                if !direct_atomic {
                    self.scan_expr_for_loops(callee, span);
                }
                for arg in args {
                    self.scan_expr_for_loops(arg, span);
                }
            }
            Expr::MethodCall {
                receiver,
                name,
                args,
            } => {
                if name == "array_eq" || name == "array_same_except" {
                    self.check_array_relation_call(name, receiver, args, span);
                }
                self.check_u64_bit_method_call(name, args, span);
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
            Expr::Cast { expr: inner, ty } => {
                self.validate_type(ty, span);
                self.scan_expr_for_loops(inner, span);
            }
            Expr::Ref { expr: inner, .. } => self.scan_expr_for_loops(inner, span),
            Expr::Closure { body, .. } => self.scan_expr_for_loops(body, span),
            // REQ-6: a struct / struct-variant construction's field names must be
            // declared; the field values are descended for nested loops/ADTs.
            // REQ-8: minting a `#[sealed]` clean type with a literal is the #77
            // door-bypass — rejected here (the exec-body walk) so a laundering
            // `query(Sql { stmt: input.raw })` is rejected at validation.
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
            // Cluster C9-B (`.design/basis/10-recursion-tuples.md` REQ-8, #109): a
            // tuple construction descends into each element (a nested loop / ADT can
            // live in any element); a projection descends into its receiver. A
            // tuple's well-formedness is its elements' (REQ-8 leaf descent).
            Expr::Tuple(elems) => {
                for e in elems {
                    self.scan_expr_for_loops(e, span);
                }
            }
            Expr::TupleProj { receiver, .. } => self.scan_expr_for_loops(receiver, span),
            // A raw quantified formula (`.design/stage2-stratified-cage.md` REQ-0):
            // descend into the domain and body for nested loops / ADT well-formedness,
            // like any other compound expression. The stratified fragment/sort checks
            // are the classifier's job (REQ-4), not this exec-body walk.
            Expr::Quantifier { domain, body, .. } => {
                self.scan_expr_for_loops(domain, span);
                self.scan_expr_for_loops(body, span);
            }
            // Leaves — no nested loop / ADT node possible. A string literal
            // (`.design/basis/07-strings.md` REQ-1) is a value-carrying leaf, like
            // an int/bool literal — no sub-expression to descend.
            Expr::Path(path) => {
                if let [name] = path.as_slice() {
                    if self.atomic_functions.contains_key(name) {
                        self.errors.push(SpecError::IllegalAtomicOrdering {
                            operation: name.clone(),
                            detail: "a frozen atomic boundary must be called directly; aliases and function-value references cannot preserve the statically checked ordering positions".to_string(),
                            span,
                        });
                    }
                }
            }
            Expr::IntLit { .. } | Expr::BoolLit(_) | Expr::StrLit(_) => {}
        }
    }

    /// Walk a caged block (a `spec fn` body, or a block nested inside a caged
    /// expression such as an `if`'s arm): every statement expression and the
    /// tail expression is a contract-position expression and is cage-checked.
    /// Any `loop`/`while` it contains carries its own `invs`/`dec` clauses.
    fn walk_block(&mut self, block: &Block, span: Span) {
        let outer_types = self.array_equality_types.clone();
        let outer_mutable = self.mutable_bindings.clone();
        self.descend(span, |s| {
            for stmt in &block.stmts {
                s.walk_stmt(stmt, span);
            }
            if let Some(tail) = &block.tail {
                s.walk_expr(tail, span);
            }
        });
        self.array_equality_types = outer_types;
        self.mutable_bindings = outer_mutable;
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
            Stmt::Let {
                mutable,
                name,
                ty,
                init,
            } => {
                if let Some(ty) = ty {
                    self.validate_type(ty, span);
                    self.validate_array_initializer(ty, init, span);
                    self.array_equality_types.insert(name.clone(), ty.clone());
                    if *mutable {
                        self.mutable_bindings.insert(name.clone());
                    }
                }
                self.walk_expr(init, span);
            }
            Stmt::Assign { target, value } => {
                self.check_named_record_assignment(target, span);
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
            // leaf admitted in a contract position as an int/bool literal
            // — e.g. the editor case `s == "needle"`; no sub-expression to walk.
            Expr::IntLit { .. } | Expr::BoolLit(_) | Expr::Path(_) | Expr::StrLit(_) => {}

            Expr::Array(elements) => {
                for element in elements {
                    self.walk_expr(element, span);
                }
            }
            Expr::ArrayRepeat { value, len } => {
                self.resolve_array_len(len, span);
                self.walk_expr(value, span);
            }

            // (a)/(b)/(iv): a free call is a combinator, a spec-fn call, or
            // forbidden.
            Expr::Call { callee, args } => self.walk_call(callee, args, span),

            // (c) bounded built-in method calls. REQ-3(c) admits only "the
            // bounded built-in `MethodCall`s the grammar admits (e.g.
            // `xs.len()`)", not an arbitrary method name. A non-allowlisted
            // method name in a caged position is forbidden (REQ-4 (iv) ->
            // `ForbiddenCall`). The allowlist is `BUILTIN_METHODS`; a permitted
            // method's receiver and args are recursed into.
            Expr::MethodCall {
                receiver,
                name,
                args,
            } => {
                if name == "array_eq" || name == "array_same_except" {
                    self.check_array_relation_call(name, receiver, args, span);
                }
                self.check_u64_bit_method_call(name, args, span);
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
            // REQ-6: a `Field` whose name no `struct`/struct-variant declares
            // is `UnknownField`. The AST is untyped (OQ-3), so this is the
            // shallow, mechanically-decidable check the design admits: the field
            // must exist somewhere. When no ADT is declared (`struct_fields`
            // empty), the check is inert, so the existing non-ADT corpus
            // (`sum.th`/`binary_search.th`, which have no struct field access) is
            // unchanged.
            Expr::Field { receiver, name } => {
                self.check_field(name, span);
                self.walk_expr(receiver, span);
            }
            Expr::Binary { lhs, rhs, .. } => {
                self.walk_expr(lhs, span);
                self.walk_expr(rhs, span);
            }
            // The prefix `!` (#92, ast.md REQ-10): a structural built-in operator,
            // admitted in a contract position like a `Binary`. The cage is
            // untyped (OQ-3), so the bitwise-vs-logical / valid-operand-type
            // discrimination is not a validator check; it is resolved downstream by
            // Verus's type-directed `!` (ast.md OQ-4): `!` on a non-integer /
            // non-bool operand (e.g. `&[u32]`) is rejected at L3 as a Verus type
            // error, not here. The operand is recursed (depth-guarded, REQ-5) so
            // nested forbidden content still surfaces.
            Expr::Unary { expr: inner, .. } => self.walk_expr(inner, span),
            Expr::Index { base, index } => {
                self.walk_expr(base, span);
                self.walk_index(index, span);
            }
            Expr::Cast { expr: inner, ty } => {
                self.validate_type(ty, span);
                self.walk_expr(inner, span);
            }
            Expr::Ref { expr: inner, .. } => self.walk_expr(inner, span),

            // (c) match / if — built-in control forms. A `match` over a declared
            // `enum` value is exhaustiveness/well-formedness-checked (REQ-5/
            // REQ-6); a slice `match` (`sum.th`) or a `match` over a built-in
            // (`Option`'s `Some`/`None` in `binary_search.th`) is unchanged:
            // `check_match_exhaustiveness` only fires when an arm pattern names a
            // variant of a declared enum. `Match`/`Field`/`If`/`Is` stay flat
            // built-ins inside a combinator closure (REQ-7); the caged-flat
            // mode is untouched by this descent.
            Expr::Match { scrutinee, arms } => {
                self.walk_expr(scrutinee, span);
                self.check_match_exhaustiveness(arms, span);
                for arm in arms {
                    // A C10 match guard is an `Expr` in the cage walk too — a guard
                    // mentioning an unknown field/variant must still be flagged
                    // (`.design/basis/11-ergonomics.md` REQ-3).
                    if let Some(guard) = &arm.guard {
                        self.walk_expr(guard, span);
                    }
                    self.walk_expr(&arm.body, span);
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
            // / construction expressions are now validated, not gated.
            //
            // A struct / struct-variant construction `Path { field: val, … }`:
            // each initializer field must be declared by some
            // `struct`/struct-variant (REQ-6, the same shallow, untyped check as
            // `Field`); the field values are recursed (depth-guarded, REQ-5). The
            // last `path` segment naming a known struct-variant is well-formed by
            // construction; a `path` naming nothing checkable is left to lowering
            // (1c); the 1a `UnsupportedAdt` refusal is gone for a well-formed
            // literal.
            Expr::StructLit { path, fields } => {
                // REQ-8: a `#[sealed]` clean type is door-only-mintable; a
                // literal of one (anywhere, contract or caged body) is the #77
                // launder and is rejected with `SealedConstruction`.
                self.check_sealed_construction(path, span);
                for (field_name, value) in fields {
                    self.check_field(field_name, span);
                    self.walk_expr(value, span);
                }
            }
            // `SCRUTINEE is Variant` (REQ-6): the `variant` must name a declared
            // enum variant, else `UnknownVariant`. `is` is a flat `bool` built-in
            // and joins `Match`/`Field`/`If` in the caged-flat accept set (REQ-7);
            // it is admitted inside a combinator predicate-closure body
            // unchanged. The scrutinee is recursed (depth-guarded).
            Expr::Is { scrutinee, variant } => {
                self.check_variant_ref(variant, span);
                self.walk_expr(scrutinee, span);
            }
            // A `Box` deref `*EXPR` (REQ-3): accepted structurally here (the
            // recursive deref `sum_list(*t)` of `list_sum.th`); its `Box` SEMANTICS
            // are Stage 1c. Recurse the inner expression (depth-guarded).
            Expr::Deref(inner) => self.walk_expr(inner, span),
            // Cluster C9-B (`.design/basis/10-recursion-tuples.md` REQ-8, #109): a
            // tuple construction `(a, b, …)` is a flat structural built-in (its
            // elements are recursed, depth-guarded); a projection `e.0`/`e.1` is a
            // flat built-in like `Field`, admitted inside the §4.2 cage — an `ens
            // result.0 == b` reads a tuple element as `Field` reads a
            // struct field. A tuple is well-formed iff its elements are.
            Expr::Tuple(elems) => {
                for e in elems {
                    self.walk_expr(e, span);
                }
            }
            Expr::TupleProj { receiver, .. } => self.walk_expr(receiver, span),
            // A raw quantified formula `forall (x : S) in <dom>. φ`
            // (`.design/stage2-stratified-cage.md` REQ-0): surface + parse exists
            // now, but STRATIFIED ADMISSION (the sort-graph/fragment classifier) is
            // REQ-4, a separate pass added beside this validator. This walker is the
            // v1 cage; it neither certifies nor models the binder yet. To stay
            // non-breaking for the foundation increment we recurse the domain and
            // body — so any forbidden nested content still surfaces (REQ-5,
            // depth-guarded) — and otherwise leave the binder structurally accepted
            // for the REQ-4 classifier to admit or reject. No corpus uses raw
            // quantifiers, so the existing cage behavior is unchanged.
            Expr::Quantifier { domain, body, .. } => {
                self.walk_expr(domain, span);
                self.walk_expr(body, span);
            }
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
    /// The AST is untyped (OQ-3): the matched enum is inferred from the arm
    /// patterns, not the scrutinee. A `match` is a declared-enum match iff some
    /// arm names a variant of a declared `enum` (`variant_to_enum`); otherwise it
    /// is a slice `match` (`sum.th`'s `[]`/`[head, ..t]`) or a `match` over a
    /// built-in (`Option`'s `Some`/`None` in `binary_search.th` — `Option` is no
    /// declared `Item::Enum`) and is left unchanged (the AC-6 no-regression
    /// invariant). Once identified as a declared-enum match:
    /// - an arm naming a variant of a different/undeclared enum is `UnknownVariant`;
    /// - a variant matched twice, or an arm after a catch-all, is `UnreachableArm`;
    /// - if no catch-all closes the match, every uncovered declared variant is
    ///   collected into `NonExhaustiveMatch { missing }` (declaration order).
    fn check_match_exhaustiveness(&mut self, arms: &[MatchArm], span: Span) {
        // Identify the matched enum: the owning enum of the first arm pattern
        // that names a declared variant.
        let matched_enum = arms.iter().find_map(|arm| {
            variant_pattern_name(&arm.pattern).and_then(|v| self.variant_to_enum.get(v).cloned())
        });
        let Some(enum_name) = matched_enum else {
            // No arm names a declared variant, so the enum could not be inferred
            // from the patterns. Two sub-cases:
            //   (a) a slice / Option / integer / bindings match (the existing
            //       inert behavior — Verus discharges any non-exhaustiveness);
            //   (b) a match whose every arm is guarded (`_ if cond =>`, `x if
            //       cond =>`, …) and so covers nothing unconditionally. Per
            //       `.design/basis/11-ergonomics.md` REQ-3 / AC-3b a guard does
            //       not complete a match (the guard may fail at runtime), so a
            //       guarded-only match — including a guarded catch-all
            //       `match m { _ if cond => 0 }` over an enum, where no arm names
            //       a variant to reveal the enum — is non-exhaustive. An unguarded
            //       catch-all (`match m { _ => 0 }`) completes the match and
            //       is left accepted (it would have set `wildcard_seen` in the
            //       main loop; here we require at least one arm and every arm
            //       guarded, so a plain `_` arm keeps the match inert).
            // Detecting (b) is the only sound reject we can make without the
            // scrutinee's (untyped, OQ-3) declared enum: a bare `_ if cond`
            // guarantees nothing, so the match is non-exhaustive regardless of
            // the scrutinee's type.
            if !arms.is_empty() && arms.iter().all(|arm| arm.guard.is_some()) {
                self.errors.push(SpecError::NonExhaustiveMatch {
                    missing: vec!["<guarded-only: no arm completes the match>".to_string()],
                    span,
                });
            }
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
            // A guarded arm covers none of its pattern's cases — the guard may
            // fail (`.design/basis/11-ergonomics.md` REQ-3, grounded: Verus
            // rejects a guarded-only `Some` arm as non-exhaustive). It is never a
            // catch-all and never marks a variant covered. It is still reachable
            // (a guarded arm after a catch-all is dead, handled below), and its
            // variant must still be declared (a guarded `r is Bogus` is still
            // `UnknownVariant`).
            let guarded = arm.guard.is_some();

            // A catch-all (`_`/binding, or an or-pattern containing one) closes
            // the match, unless it is guarded (the guard may fail). A second
            // catch-all, or any arm after one, is unreachable.
            if !guarded && pattern_is_catch_all(&arm.pattern) {
                if wildcard_seen {
                    self.errors.push(SpecError::UnreachableArm { span });
                }
                wildcard_seen = true;
                continue;
            }

            // Validate + count each declared-enum variant the pattern names. An
            // or-pattern contributes the union of its alternatives' variants
            // (REQ-4). A non-variant pattern (a bare literal) names none — left to
            // the shallow checking (no false `UnknownVariant`).
            let mut variants = Vec::new();
            collect_covered_variants(&arm.pattern, &mut variants);
            if variants.is_empty() {
                // No declared variant named: if this is a dead arm after a
                // catch-all, still flag it; otherwise nothing to count.
                if wildcard_seen && variant_pattern_name(&arm.pattern).is_some() {
                    self.errors.push(SpecError::UnreachableArm { span });
                }
                continue;
            }
            for variant in variants {
                if wildcard_seen {
                    // Any arm after a catch-all is dead.
                    self.errors.push(SpecError::UnreachableArm { span });
                } else if !declared.iter().any(|d| d == variant) {
                    self.errors.push(SpecError::UnknownVariant {
                        name: variant.to_string(),
                        span,
                    });
                } else if guarded {
                    // A guarded arm does not cover its variant for exhaustiveness
                    // (REQ-3): it neither closes the match nor is a redundant
                    // re-cover (a later unguarded arm for the same variant is the
                    // real handler, not unreachable). Validate the variant
                    // (declared) but do not insert into `covered`.
                } else if !covered.insert(variant) {
                    // Variant matched twice (unguarded) → the second arm is dead.
                    self.errors.push(SpecError::UnreachableArm { span });
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
    /// is rejected (`SealedConstruction`); a sealed type is door-only-mintable.
    /// `path` is the literal's path; the constructed type is its last segment
    /// (`Sql` in `Sql { … }`). Inert when no `#[sealed]` struct is declared (the
    /// non-IFC corpus is unchanged). The `#[boundary]` door is unaffected: its
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
    /// declared by some `struct`/struct-variant. Shallow + untyped (OQ-3): inert
    /// when no ADT declares any field (the non-ADT corpus is unchanged), and a
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
        // REQ-4): a callee resolving to a registered recursion scheme
        // (`fold`/`map`/`for_all`/`exists`/`traverse`) is handled first. A scheme
        // nested inside another scheme's step or inside a combinator's
        // predicate-closure body is an anonymous nested structural quantifier the
        // cage forbids (`NestedScheme`); a top-level scheme call is accepted as a
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
                // another combinator's predicate-closure body or a scheme's flat
                // step closure is an anonymous nested quantifier — forbidden. The
                // discriminator is `combinators::lookup` succeeding (the
                // same test that accepts this callee in a top-level contract
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
    /// shape, and the step body is flat (no nested scheme/combinator — enforced by
    /// walking the body in `in_scheme_step` mode). The scrutinee/seed args are
    /// ordinary contract expressions (recursed, depth-guarded). Accepted at the
    /// top level (the cage bridge, REQ-4), the §4.2 "named composition" leaf.
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
            // walked without scheme-step mode — a malformed call is not a valid
            // step context.
            for arg in args {
                self.walk_expr(arg, span);
            }
            return;
        }

        // The leading `scrutinee_args` are ordinary contract sub-expressions (the
        // scrutinee structure + a fold seed), recursed under the depth guard. They
        // are not in scheme-step mode — a scheme/combinator there is a legitimate
        // nested named composition at the call's argument level (only the step
        // body is the flat-cage position, REQ-2).
        let step_position = scheme.scrutinee_args;
        for arg in &args[..step_position] {
            self.walk_expr(arg, span);
        }

        // The trailing argument is the per-node step: it must be a closure of the
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
                // Enter flat-scheme-step mode for the body. Set once here and keep
                // it set for the entire body descent (save/restore so a sibling
                // scheme call's step is checked independently and re-entry from a
                // rejected nested scheme is a harmless no-op).
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
    /// Per OQ-3, only `Pred` is syntactically decidable (must be `Expr::Closure`);
    /// `Slice`/`Index`/`Value` are checked shallowly: any non-closure expression
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
                // body — the legitimate contract sub-expression — rather than
                // the closure node (which `walk_expr` would flag as a misplaced
                // bare closure). This bounds the body's depth too (REQ-5).
                //
                // REQ-6: enter "caged-flat" mode for the body. Set once here and
                // keep it set for the entire body descent (all nested
                // sub-expressions and any nested closure), then restore so a
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
/// both name a variant by the last path segment (the variant name; an enclosing
/// `Shape::` prefix is the type). A `Wildcard`/`Binding`/`Literal`/`Slice`
/// pattern names no variant — used to distinguish a declared-enum match from a
/// slice match (`sum.th`) and to drive the covered-variant set.
fn variant_pattern_name(pattern: &Pattern) -> Option<&str> {
    match pattern {
        Pattern::Enum { path, .. } | Pattern::Struct { path, .. } => {
            path.last().map(|s| s.as_str())
        }
        // An or-pattern names a variant iff some alternative does (used only to
        // identify the matched enum — the first variant-naming arm). The full
        // covered-variant union is collected by `collect_covered_variants`
        // (`.design/basis/11-ergonomics.md` REQ-4).
        Pattern::Or(alts) => alts.iter().find_map(variant_pattern_name),
        Pattern::Wildcard | Pattern::Binding(_) | Pattern::Literal(_) | Pattern::Slice(_) => None,
    }
}

/// True iff `pattern` is a catch-all — a bare `_`/binding, or an or-pattern any of
/// whose alternatives is a catch-all (`.design/basis/11-ergonomics.md` REQ-4). A
/// catch-all closes a `match` (every remaining case is covered). A guarded arm is
/// never a catch-all for exhaustiveness (REQ-3 — handled at the call site).
fn pattern_is_catch_all(pattern: &Pattern) -> bool {
    match pattern {
        Pattern::Wildcard | Pattern::Binding(_) => true,
        Pattern::Or(alts) => alts.iter().any(pattern_is_catch_all),
        _ => false,
    }
}

/// Collect every declared-enum variant name an unguarded `pattern` covers into
/// `out` (`.design/basis/11-ergonomics.md` REQ-4). A `Pattern::Enum`/`Struct`
/// covers its one variant; a `Pattern::Or` covers the union of its alternatives'
/// variants (the or-pattern rule — `Some(_) | None` covers both). A
/// catch-all/literal/slice contributes no specific variant (a catch-all closes
/// the match separately via `pattern_is_catch_all`).
fn collect_covered_variants<'p>(pattern: &'p Pattern, out: &mut Vec<&'p str>) {
    match pattern {
        Pattern::Enum { path, .. } | Pattern::Struct { path, .. } => {
            if let Some(v) = path.last() {
                out.push(v.as_str());
            }
        }
        Pattern::Or(alts) => {
            for alt in alts {
                collect_covered_variants(alt, out);
            }
        }
        Pattern::Wildcard | Pattern::Binding(_) | Pattern::Literal(_) | Pattern::Slice(_) => {}
    }
}

/// True iff the exec `fn` declares `fx diverge` (`.design/basis/10-recursion-tuples.md`
/// REQ-2, §4.1: "divergence requires `fx diverge`"). A diverge fn is
/// non-terminating (an event loop), so it is exempt from the mandatory-`dec` rule
/// — it may recurse without a termination measure (the #88 L1-cap; the lowerer
/// emits `#[verifier::exec_allows_no_decreases_clause]`). Keyed on the shape of
/// the effect row, mirroring `thermite-lower`'s `fn_is_diverge` (the single source
/// of truth for the §4.1 termination exemption).
fn fn_is_diverge(f: &thermite_syntax::ast::FnItem) -> bool {
    use thermite_syntax::ast::{Effect, EffectRow};
    matches!(&f.contract.fx, EffectRow::Set(es) if es.contains(&Effect::Diverge))
}

/// True iff `block` contains a direct call to `name` — the self-reference test
/// for the mandatory-`dec` rule (`.design/basis/10-recursion-tuples.md` REQ-2).
/// "Direct" means a free-function call `name(..)` whose callee path's last
/// segment is `name` (a self-recursive call); a method call `recv.name(..)` is
/// not a self-call (the receiver dispatches it). Walks every statement + nested
/// expression of the body (let inits, assigns, returns, ifs, loops, match arms,
/// call args, …) so a self-call anywhere in the body is found. Mutual recursion
/// (a call to a different fn that calls back) is not detected here (REQ-6,
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
        Expr::Array(elements) => elements.iter().any(|e| expr_calls_name(e, name)),
        Expr::ArrayRepeat { value, .. } => expr_calls_name(value, name),
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
        // Cluster C9-B (`.design/basis/10-recursion-tuples.md` REQ-8, #109): a
        // recursive call can live in any tuple element (a recursive fn returning a
        // tuple) or under a projection's receiver — the self-call detection (REQ-2)
        // descends into both.
        Expr::Tuple(elems) => elems.iter().any(|e| expr_calls_name(e, name)),
        Expr::TupleProj { receiver, .. } => expr_calls_name(receiver, name),
        // A raw quantified formula (`.design/stage2-stratified-cage.md` REQ-0): a
        // (recursive) call can live in either the domain or the body, so the
        // self-call detection (REQ-2) descends into both.
        Expr::Quantifier { domain, body, .. } => {
            expr_calls_name(domain, name) || expr_calls_name(body, name)
        }
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
