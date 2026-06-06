//! The recursion-scheme registry — the frozen, closed set of verified recursion
//! schemes (`.design/basis/02-recursion-schemes.md` REQ-1) over recursive ADTs:
//! `fold`, `map`, `for_all`, `exists`, `traverse`. The structural complement of
//! `combinators.rs`'s `static REGISTRY` + `lookup` (the OQ-1 (a) resolution: a
//! scheme CALL reuses `Expr::Call` with a scheme-name callee `Path`, and the
//! scheme-ness is recognized by THIS registry, not a new `Expr` node).
//!
//! Governing design: `.design/basis/02-recursion-schemes.md` (REQ-1, REQ-2).
//! Pinned against the hand-derived oracle at
//! `conformance/adt-schemes/cases.json` (R-CHAR-3).
//!
//! ## Scope (the 2b vs 2c split, mirroring combinators.rs)
//!
//! This registry ships the STRUCTURAL facet the validator (Stage 2b) needs: the
//! scheme name, the step shape (`|x, acc|` for `fold`/`traverse`, `|x|` for
//! `map`/`for_all`/`exists`), and the result kind. The validator reads it to (i)
//! recognize a scheme call as a named-composition leaf (REQ-4) and (ii) enforce
//! the FLAT step closure (REQ-2 — the right step arity, no nested scheme). The
//! GENERATION facet — the per-(ADT, scheme) Verus recursive `spec fn`
//! (`fold_<e>`/`for_all_<e>`) + the `fold_bound_<e>` law — is materialized by
//! `thermite-lower` (Stage 2c, the registry's lowering consumer), exactly as the
//! combinator `verus_l3` facet is `thermite-lower`'s consumer.
//!
//! The generated names are a DETERMINISTIC function of the scheme + the ADT name
//! (`fold_<lowercased-enum-name>`), so the lowerer and any future audit-surface
//! consumer agree on the symbol without a string-name guess (REQ-1; R-CODE-5).
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (the scheme set as named primitives; AST = `Expr::Call` + registry) | SHIPPED | `static REGISTRY: [SchemeSig; 5]` — the 5 frozen schemes (`fold`/`map`/`for_all`/`exists`/`traverse`); `lookup(name)` resolves a callee `Path` to its `SchemeSig`; consumed by `validator::validate` (the scheme-call accept) and `thermite_lower::lower` (the generated-fn name via `generated_fn_name`). Asserted against `conformance/adt-schemes/cases.json` in `thermite-spec/tests/scheme_validate.rs`. |
//! | REQ-2 (the step — flat per-node closure; step arity by scheme) | SHIPPED | `SchemeSig.step_arity` (2 for `fold`/`traverse`, 1 for `map`/`for_all`/`exists`) + `SchemeSig.scrutinee_args` (the non-step positional args before the step); `validator` checks the step closure's param count against `step_arity` and the total call arity, and rejects a nested scheme in the step body. |

/// The accumulator/result KIND a scheme yields (REQ-1/REQ-3). Keyed by the
/// lowerer to pick the generated `spec fn`'s return type (`nat` for `fold`,
/// `bool` for the structural predicates, the ADT itself for `map`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemeResult {
    /// A `fold` collapses the structure to a `nat` accumulator (the GROUNDED
    /// `fold_list -> nat` form; the spec accumulator is `nat` so the fold cannot
    /// overflow the spec relation, mirroring the Stage-1 `is_adt_fold_sum`
    /// `nat`-return discipline).
    Accumulator,
    /// A `for_all`/`exists`/`traverse` collapses the structure to a `bool` (the
    /// GROUNDED `for_all_list -> bool` cage-bridge form).
    Bool,
    /// A `map` rebuilds the SAME ADT, transformed element-wise (the GROUNDED
    /// `map_list -> List` with `Box::new` reconstruction).
    SameAdt,
}

/// The step closure's per-node parameter shape (REQ-2). The validator checks the
/// supplied `Expr::Closure`'s param count against this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepShape {
    /// `|x, acc|` — the element plus the accumulated result (`fold`, `traverse`).
    ElementAcc,
    /// `|x|` — the element alone (`map`, `for_all`, `exists`).
    Element,
}

impl StepShape {
    /// The number of parameters the step closure must declare (REQ-2): 2 for
    /// `|x, acc|`, 1 for `|x|`. The validator rejects a step whose closure param
    /// count does not match.
    pub fn arity(self) -> usize {
        match self {
            StepShape::ElementAcc => 2,
            StepShape::Element => 1,
        }
    }
}

/// One registry entry: the STRUCTURAL signature of a frozen recursion scheme
/// (REQ-1/REQ-2). Plain named-field struct (the `CombinatorSig` precedent) so the
/// lowering facet can grow it in place without a breaking layout change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchemeSig {
    /// The canonical scheme name as it appears as a call callee `Path`
    /// (`fold`/`map`/`for_all`/`exists`/`traverse`).
    pub name: &'static str,
    /// The number of positional NON-step arguments BEFORE the step closure
    /// (REQ-1). `fold(l, init, step)` has 2 (the scrutinee + the seed); a
    /// predicate scheme `for_all(l, step)` has 1 (the scrutinee). The total call
    /// arity is `scrutinee_args + 1` (the trailing step closure).
    pub scrutinee_args: usize,
    /// The step closure's per-node parameter shape (REQ-2).
    pub step_shape: StepShape,
    /// The result/accumulator kind the scheme collapses to (REQ-1/REQ-3).
    pub result: SchemeResult,
    /// The generated Verus `spec fn` name PREFIX for this scheme (REQ-1, OQ-1
    /// (b)). The full generated name is `<prefix>_<lowercased-enum-name>` (e.g.
    /// `fold` + `List` → `fold_list`), formed by `generated_fn_name`. The lowerer
    /// materializes `<prefix>_<e>` and lowers the scheme call to cite it.
    pub gen_prefix: &'static str,
}

/// The FROZEN v0.1 recursion-scheme set (REQ-1). Closed: adding, removing, or
/// changing an entry is a design-doc amendment (R-SPEC-4 / §4.4 closed built-in
/// set), not a code-local choice. The contents are pinned against
/// `conformance/adt-schemes/cases.json` (R-CHAR-3). `lookup` is by name, not
/// index, so order is not load-bearing. Static (deterministic, R-CODE-5).
static REGISTRY: [SchemeSig; 5] = [
    SchemeSig {
        name: "fold",
        scrutinee_args: 2,
        step_shape: StepShape::ElementAcc,
        result: SchemeResult::Accumulator,
        gen_prefix: "fold",
    },
    SchemeSig {
        name: "traverse",
        scrutinee_args: 1,
        step_shape: StepShape::ElementAcc,
        result: SchemeResult::Bool,
        gen_prefix: "traverse",
    },
    SchemeSig {
        name: "map",
        scrutinee_args: 1,
        step_shape: StepShape::Element,
        result: SchemeResult::SameAdt,
        gen_prefix: "map",
    },
    SchemeSig {
        name: "for_all",
        scrutinee_args: 1,
        step_shape: StepShape::Element,
        result: SchemeResult::Bool,
        gen_prefix: "for_all",
    },
    SchemeSig {
        name: "exists",
        scrutinee_args: 1,
        step_shape: StepShape::Element,
        result: SchemeResult::Bool,
        gen_prefix: "exists",
    },
];

impl SchemeSig {
    /// The TOTAL positional argument count of a well-formed scheme call (REQ-1/
    /// REQ-2): the scrutinee/seed args plus the single trailing step closure.
    /// `fold(l, init, step)` → 3; `for_all(l, step)` → 2.
    pub fn total_arity(&self) -> usize {
        self.scrutinee_args + 1
    }

    /// The generated Verus `spec fn` name for this scheme over an ADT named
    /// `enum_name` (REQ-1, OQ-1 (b)): `<prefix>_<lowercased-enum-name>`. The
    /// lowercasing is deterministic (R-CODE-5) and matches the GROUNDED
    /// `fold_list`/`for_all_list` symbols (`enum List` → `list`). This is the
    /// single source of truth for the generated symbol shared by the lowerer
    /// (which materializes it) and any future audit-surface consumer.
    pub fn generated_fn_name(&self, enum_name: &str) -> String {
        format!("{}_{}", self.gen_prefix, enum_name.to_ascii_lowercase())
    }
}

/// Resolve a scheme by its canonical name (REQ-1). Returns the static signature
/// if `name` is a registered scheme, else `None` (the validator then treats the
/// callee as a combinator / declared spec-fn call, or rejects it as unknown).
/// This is the registry's public lookup API and the validator's non-test
/// consumer (R-DEFER-1), mirroring `combinators::lookup`.
pub fn lookup(name: &str) -> Option<&'static SchemeSig> {
    REGISTRY.iter().find(|entry| entry.name == name)
}

/// The frozen registry as a slice (REQ-1). Exposed so the conformance test can
/// assert the full table against the oracle and so a later consumer
/// (`thermite-skill` #7) can regenerate the skill's scheme section from the
/// single source of truth.
pub fn all() -> &'static [SchemeSig] {
    &REGISTRY
}
