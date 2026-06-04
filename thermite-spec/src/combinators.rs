//! The SpecTherm combinator registry — the frozen, closed set of bounded
//! combinators (`thermite-design.md` §4.2) with their structural signature:
//! canonical name, arity, ordered argument kinds, and result kind.
//!
//! Governing design: `.design/spec/spectherm-combinators.md` (REQ-1, REQ-2).
//! The frozen contents are pinned against the hand-derived oracle at
//! `tests/golden/combinators/registry.json` (R-CHAR-3).
//!
//! ## Scope (the #2 vs #4 split)
//!
//! This registry ships only the STRUCTURAL facet a validator needs now:
//! name / arity / arg-kinds / result. The LOWERING facet of each combinator —
//! the frozen SMT trigger string, the Verus (L3) definition, and the executable
//! (L1) runtime-check form (§4.2 "frozen SMT triggers"; §6 "the L1 fallback rung
//! always exists") — is DEFERRED to issue #4, where `thermite-lower` is the
//! consumer that reads them (OQ-2). Including those fields now would be
//! vocabulary-only (no #2 consumer, R-DEFER-1). The `CombinatorSig` struct is a
//! plain named-field struct (no `#[non_exhaustive]`-hostile layout) so #4 can
//! grow it in place — that is the extensibility seam.
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (frozen combinator set) | SHIPPED | `static REGISTRY: [CombinatorSig; 8]` — the 8 frozen combinators; consumed by `validator::validate` via `lookup`; asserted field-for-field against `tests/golden/combinators/registry.json` in `tests/combinators_conformance.rs`. |
//! | REQ-2 (registry data shape — structural facet) | SHIPPED | `struct CombinatorSig { name, arity, arg_kinds, result }`, `enum ArgKind { Slice, Index, Pred, Value }`, `enum ResultKind { Bool, Usize }`; static table + `lookup(name)`. Lowering facet is #4 scope (named seam above). |
//! | REQ-6 (combinator Verus(L3) bodies — lowering facet) | SHIPPED — verus-lowering.md REQ-6 | `CombinatorSig.verus_l3` carries each combinator's frozen Verus `spec fn` definition (frozen `#[trigger]`); consumed by `thermite-lower::lower::emit_combinator_defs` (the #4 consumer that closes the OQ-2 seam, R-DEFER-1). The four corpus forms verify in `thermite-lower/tests/lower_conformance.rs` via real `verus`. |

/// The KIND of a positional argument a combinator expects (REQ-2). The validator
/// uses these to check each call argument's shape against the registry entry.
///
/// Per OQ-3, only `Pred` is syntactically decidable (it MUST be an
/// `Expr::Closure`); `Slice` / `Index` / `Value` are checked shallowly ("an
/// expression that is not a closure" in those positions) until a later
/// type-resolution pass exists — full typing is not a v0.1 kernel item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgKind {
    /// A `&[T]` slice-shaped expression (e.g. `xs`, `&xs[..i]`).
    Slice,
    /// A `usize`-valued index expression (e.g. `lo`, `i`).
    Index,
    /// A predicate closure literal `|x| <bool expr>` — the one syntactically
    /// strict kind (must be `Expr::Closure`).
    Pred,
    /// A plain scalar expression (e.g. `needle`, `5`).
    Value,
}

/// The result KIND a combinator yields (REQ-2). The v0.1 set is all `Bool`
/// except `count_where` (`Usize`); the field exists so a `usize`-result
/// combinator is representable in the table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultKind {
    /// A boolean-valued combinator (every v0.1 entry except `count_where`).
    Bool,
    /// A `usize`-valued combinator (`count_where`).
    Usize,
}

/// One registry entry: the STRUCTURAL signature of a frozen SpecTherm
/// combinator (REQ-2). Plain named-field struct so issue #4 can extend it in
/// place with the lowering facet (SMT trigger / Verus L3 def / executable L1
/// form) without a breaking layout change (OQ-2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CombinatorSig {
    /// The canonical combinator name as it appears as a call callee path.
    pub name: &'static str,
    /// The exact (fixed) argument count.
    pub arity: usize,
    /// The ordered argument kinds; `arg_kinds.len() == arity` for every entry.
    pub arg_kinds: &'static [ArgKind],
    /// The result kind the combinator yields.
    pub result: ResultKind,
    /// The frozen Verus(L3) `spec fn` definition for this combinator (the
    /// OQ-2 lowering-facet seam, closed by issue #4). This is the EXACT
    /// `spec fn <name>(...) -> ... { ... }` text `thermite-lower` emits into the
    /// `verus! { ... }` frame when a contract references this combinator. The
    /// body is the frozen bounded-quantifier form with a frozen `#[trigger]` on
    /// the predicate application (`.design/lower/verus-lowering.md` REQ-6;
    /// `thermite-design.md` §4.2 "hand-tuned, frozen SMT triggers"). Verified
    /// against the real `verus` binary by the four corpus forms in
    /// `thermite-lower/tests/lower_conformance.rs` and in isolation (AC-3).
    pub verus_l3: &'static str,
}

/// The FROZEN v0.1 SpecTherm combinator set (REQ-1). Closed: adding, removing,
/// or changing an entry is an RFC / design-doc amendment (R-SPEC-4), not a
/// code-local choice. The contents are pinned against
/// `tests/golden/combinators/registry.json` (R-CHAR-3); the order here mirrors
/// the oracle for readability but `lookup` is by name, not index, so order is
/// not load-bearing. Static and `const`-derived (deterministic, R-CODE-5).
static REGISTRY: [CombinatorSig; 8] = [
    CombinatorSig {
        name: "sorted",
        arity: 1,
        arg_kinds: &[ArgKind::Slice],
        result: ResultKind::Bool,
        verus_l3: "spec fn sorted(s: Seq<u32>) -> bool {\n    forall|i: int, j: int| 0 <= i <= j < s.len() ==> s[i] <= s[j]\n}",
    },
    CombinatorSig {
        name: "forall_in",
        arity: 2,
        arg_kinds: &[ArgKind::Slice, ArgKind::Pred],
        result: ResultKind::Bool,
        verus_l3: "spec fn forall_in(s: Seq<u32>, p: spec_fn(u32) -> bool) -> bool {\n    forall|i: int| 0 <= i < s.len() ==> #[trigger] p(s[i])\n}",
    },
    CombinatorSig {
        name: "exists_in",
        arity: 2,
        arg_kinds: &[ArgKind::Slice, ArgKind::Pred],
        result: ResultKind::Bool,
        verus_l3: "spec fn exists_in(s: Seq<u32>, p: spec_fn(u32) -> bool) -> bool {\n    exists|i: int| 0 <= i < s.len() && #[trigger] p(s[i])\n}",
    },
    CombinatorSig {
        name: "count_where",
        arity: 2,
        arg_kinds: &[ArgKind::Slice, ArgKind::Pred],
        result: ResultKind::Usize,
        verus_l3: "spec fn count_where(s: Seq<u32>, p: spec_fn(u32) -> bool) -> nat\n    decreases s.len()\n{\n    if s.len() == 0 { 0 } else { (if p(s[0]) { 1nat } else { 0nat }) + count_where(s.drop_first(), p) }\n}",
    },
    CombinatorSig {
        name: "permutation_of",
        arity: 2,
        arg_kinds: &[ArgKind::Slice, ArgKind::Slice],
        result: ResultKind::Bool,
        verus_l3: "spec fn permutation_of(a: Seq<u32>, b: Seq<u32>) -> bool {\n    a.to_multiset() == b.to_multiset()\n}",
    },
    CombinatorSig {
        name: "disjoint",
        arity: 2,
        arg_kinds: &[ArgKind::Slice, ArgKind::Slice],
        result: ResultKind::Bool,
        verus_l3: "spec fn disjoint(a: Seq<u32>, b: Seq<u32>) -> bool {\n    forall|i: int, j: int|\n        (0 <= i < a.len() && 0 <= j < b.len()) ==> #[trigger] a[i] != #[trigger] b[j]\n}",
    },
    CombinatorSig {
        name: "forall_below",
        arity: 3,
        arg_kinds: &[ArgKind::Slice, ArgKind::Index, ArgKind::Pred],
        result: ResultKind::Bool,
        verus_l3: "spec fn forall_below(s: Seq<u32>, n: int, p: spec_fn(u32) -> bool) -> bool {\n    forall|i: int| 0 <= i < n && i < s.len() ==> #[trigger] p(s[i])\n}",
    },
    CombinatorSig {
        name: "forall_from",
        arity: 3,
        arg_kinds: &[ArgKind::Slice, ArgKind::Index, ArgKind::Pred],
        result: ResultKind::Bool,
        verus_l3: "spec fn forall_from(s: Seq<u32>, n: int, p: spec_fn(u32) -> bool) -> bool {\n    forall|i: int| n <= i < s.len() ==> #[trigger] p(s[i])\n}",
    },
];

/// Resolve a combinator by its canonical name (REQ-2). Returns the static
/// signature if `name` is a registered combinator, else `None` (the validator
/// then treats the callee as a candidate spec-fn call or rejects it). This is
/// the registry's public lookup API and the validator's non-test consumer
/// (AC-5 / R-DEFER-1).
pub fn lookup(name: &str) -> Option<&'static CombinatorSig> {
    REGISTRY.iter().find(|entry| entry.name == name)
}

/// The frozen registry as a slice (REQ-1). Exposed so the conformance test can
/// assert the full table against the oracle field-for-field (AC-1) and so a
/// later consumer (`thermite-skill` #7, §10) can regenerate the skill's
/// combinator section from the single source of truth.
pub fn all() -> &'static [CombinatorSig] {
    &REGISTRY
}
