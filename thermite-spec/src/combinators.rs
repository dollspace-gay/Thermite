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
    },
    CombinatorSig {
        name: "forall_in",
        arity: 2,
        arg_kinds: &[ArgKind::Slice, ArgKind::Pred],
        result: ResultKind::Bool,
    },
    CombinatorSig {
        name: "exists_in",
        arity: 2,
        arg_kinds: &[ArgKind::Slice, ArgKind::Pred],
        result: ResultKind::Bool,
    },
    CombinatorSig {
        name: "count_where",
        arity: 2,
        arg_kinds: &[ArgKind::Slice, ArgKind::Pred],
        result: ResultKind::Usize,
    },
    CombinatorSig {
        name: "permutation_of",
        arity: 2,
        arg_kinds: &[ArgKind::Slice, ArgKind::Slice],
        result: ResultKind::Bool,
    },
    CombinatorSig {
        name: "disjoint",
        arity: 2,
        arg_kinds: &[ArgKind::Slice, ArgKind::Slice],
        result: ResultKind::Bool,
    },
    CombinatorSig {
        name: "forall_below",
        arity: 3,
        arg_kinds: &[ArgKind::Slice, ArgKind::Index, ArgKind::Pred],
        result: ResultKind::Bool,
    },
    CombinatorSig {
        name: "forall_from",
        arity: 3,
        arg_kinds: &[ArgKind::Slice, ArgKind::Index, ArgKind::Pred],
        result: ResultKind::Bool,
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
