//! L3 emission: lower a validated `thermite-syntax` `Program` to a single
//! Verus-annotated Rust source `String` whose `requires`/`ensures`/`invariant`/
//! `decreases` annotations ARE the Thermite contract and whose body is the
//! lowered Thermite body. Forge (#5/#6) hands the emitted file to the `verus`
//! binary; a `0 errors` result is the L3 certificate
//! (`.design/lower/verus-lowering.md`; `thermite-design.md` §3/§4.1/§4.2/§6).
//!
//! Governing design: `.design/lower/verus-lowering.md`.
//! Reference (verus-verified, hand-authored): `tests/golden/lower/sum.verus.rs`,
//! `tests/golden/lower/binary_search.verus.rs`.
//!
//! ## Two lowering contexts (the central finding, REQ-5)
//!
//! Verus distinguishes EXEC code (`fn` bodies) from SPEC code
//! (`requires`/`ensures`/`invariant`/`decreases` and `spec fn` bodies). The same
//! Thermite expression lowers differently per context: a `&[T]` slice `xs` is
//! plain `xs` in exec position but `xs@` (a `vstd` `Seq<T>`) in spec position;
//! `xs[i]` is `xs[i]` in exec but `xs@[i as int]` in spec; `&xs[..i]` is
//! `&xs[..i]` in exec but `xs@.subrange(0, i as int)` in spec. A `spec fn` over a
//! slice takes `Seq<T>` (NOT `&[T]`) and recurses on `xs.drop_first()`
//! (verus-lowering.md REQ-5; the naive `&[u32]` spec-fn form fails `verus`).
//!
//! ## Proof aids are SHAPE-keyed, never program-keyed (REQ-7)
//!
//! Where a corpus program does not verify from its bare annotations, the lowerer
//! derives the needed proof aids from the program's AST/contract SHAPE — never
//! from its identity (no `if name == "binary_search"`). The shape keys are
//! documented at each template's emission site (`push_lemma_for`,
//! `nonlinear_overflow_assert`, `lift_immutable_preconds`, `extensionality_at_exit`,
//! `complementary_coverage_split`). This is the load-bearing honesty boundary
//! (`goal.md` "THE HONEST MANDATE", R-DEFER-9).
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (file frame + `fn`/`spec fn` signature) | SHIPPED | `lower` emits the `use vstd::prelude::*; verus! { .. } fn main() {}` frame; `lower_fn`/`lower_spec_fn`; verified by `lower_conformance::sum_emitted_verifies`. |
//! | REQ-2 (type lowering) | SHIPPED | `lower_type`; consumer `lower_fn`/`lower_spec_fn`; asserted by `corpus_node_substrings`. |
//! | REQ-3 (expression lowering — exec) | SHIPPED | `lower_expr` with `Ctx::exec()`; consumer `lower_block`; asserted by corpus verification. |
//! | REQ-4 (statement + loop lowering) | SHIPPED | `lower_stmt`/`lower_loop` emit every `inv`→`invariant` + `dec`→`decreases`; consumer `lower_block`. |
//! | REQ-5 (spec-context `Seq` lowering) | SHIPPED | `lower_expr` with `Ctx::spec_seq()` (`xs@`/`subrange`/`@[i as int]`); `spec_sum` recursion via `lower_spec_fn` Seq form. |
//! | REQ-6 (combinator Verus(L3) defs) | SHIPPED | `emit_combinator_defs` reads `thermite_spec::CombinatorSig.verus_l3`; closes OQ-2 (R-DEFER-1 consumer of the #2 registry seam). |
//! | REQ-7 (proof-aid emission, shape-keyed) | SHIPPED | `push_lemma_for`/`nonlinear_overflow_assert`/`lift_immutable_preconds`/`extensionality_at_exit`/`complementary_coverage_split`; each keys on AST/contract shape, documented at site. |
//! | REQ-8 (golden-file contract — VERIFY) | SHIPPED | emitted output run through real `verus` in `lower_conformance.rs`; contracts asserted equivalent to the corpus (no weakening). |
//! | REQ-9 (`LowerError`, no panics) | SHIPPED | `enum LowerError` (span-bearing, `Display`); `lower` returns `Result`; no `unwrap`/`expect`/`panic!` in this file. |
//!
//! ## #52 §9 boundary-composition arm (`.design/lower/boundary-composition.md`)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | composition REQ-1 (assumable-signature emission, boundary/slag only) | SHIPPED | `lower_fn` dispatches a `f.boundary.is_some() \|\| f.slag.is_some()` fn to `lower_external_body_fn`, which emits `#[verifier::external_body]` + the SAME `lower_fn_signature` (unweakened `requires`/`ensures`) + a synthetic `{ unimplemented!() }` body verus never checks. THE HONESTY GATE: external_body iff the syntactic `#[boundary]`/`#[slag]` flag — a regular fn ALWAYS takes the fully-proved-body arm. The 2-bool decision is DELEGATED to the Verus-verified `thermite_verified::should_emit_external_body` (epic #60, REQ-9 / `.design/verified/self-verification.md` Target C, mechanism (c)): its `ensures` proves the disjunction AND the §9 corollary `(!boundary && !slag) ==> !r`, anchored by the OBSERVABLE-dispatch test `tests/boundary_gate_verified.rs` (the emitted `#[verifier::external_body]` substring over the 4 (boundary,slag) combos). Consumer: `forge::check::item_subprogram` weaves a boundary/slag dep through this arm. Verified: `forge`'s `composition_conformance::direct_boundary_caller_verifies_through_the_contract` (caller L3) + `lying_regular_fn_is_caught_never_laundered_to_l3` (a regular lie is CAUGHT — verus `postcondition not satisfied`). The `#[verifier::external_body]` lives in the lowered verus STRING (a generated foreign-fn artifact), never in this `.rs` source. |
//!
//! ## Basis Stage 1c ADT arm (`.design/basis/01-adts.md`)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-8 (struct → Verus struct; type-invariant → enforced predicate) | SHIPPED | `lower_struct` emits a `pub struct` + `pub` fields + `impl { pub open spec fn well_formed(&self) -> bool { <inv with self.field> } }` (`lower_inv_expr`); OQ-3 RESOLVED — automatic threading: `lower_fn_signature` weaves `<param>.well_formed()` / `result.well_formed()` into `requires`/`ensures` for every invariant-bearing struct param/return (`inv_structs`). The `pub` visibility tier is the recorded grounding finding (a `pub open spec fn` body needs `pub` struct+fields). Consumer: `lower` (`Item::Struct` arm) + `lower_fn`. Verified: real verus `1 verified, 0 errors` on the emitted `bank_account` lowering, the cert oracle (`conformance/bank_account.cert.json` L3/pure/non-vacuous) — `tests/adt_lower_conformance.rs::bank_account_lowers_struct_invariant_and_verifies_l3` + `deposit_matches_cert_oracle_stable_subset`. |
//! | REQ-9 (enum → Verus enum; `match` → Verus `match`; `is` → variant test) | SHIPPED | `lower_enum` emits a Verus `enum` (unit/tuple/struct variants); `lower_match`/`lower_pattern` emit ENUM-QUALIFIED arms via the program `variants` map (`qualify_variant_path`) incl. `Pattern::Struct` (`Rect { w, h }`/`..`); `Expr::Is` → the Verus-native `(s is Circle)` discriminant. Consumer: `lower` (`Item::Enum` arm) + `lower_fn`. Verified: real verus `1 verified, 0 errors` on the emitted `shape` lowering + the cert oracle (`conformance/shape.cert.json` L3/pure/non-vacuous) — `shape_lowers_enum_match_is_and_verifies_l3` + `is_circle_matches_cert_oracle_stable_subset`. |
//! | REQ-10 (recursive type → Verus recursive enum; `Box`; structural `decreases`) | SHIPPED | `lower_enum` emits `Cons(u64, Box<List>)` (`lower_type` `Type::Box`→`Box<…>`); a `spec fn` matching the ADT-fold-sum shape (`is_adt_fold_sum`) lowers `-> nat` with `decreases l` over the datatype VALUE (Verus's built-in structural order) and `Expr::Deref`→`*t`, casts coerced `as nat` (`Ctx::nat_ret`). Consumer: `lower` (`Item::SpecFn`/`Item::Enum`). Verified: real verus `1 verified, 0 errors` on the emitted `list_sum` lowering — `list_sum_lowers_recursive_box_and_verifies_l3`. |
//! | REQ-11 (`LowerError`/no panics) | SHIPPED | the ADT arms reuse the existing `LowerError` (`Unsupported`/`TooDeep`); no new variant needed (the validator #65 owns the reject cases); no `unwrap`/`expect`/`panic!` added. Verified: `cargo clippy --workspace -D warnings` + the anti-pattern-gate. |
//! | REQ-12 (handled-or-loud — compile-time tooth) | SHIPPED | the exhaustiveness mechanism it names is the #65 validator (`SpecError::NonExhaustiveMatch`); this stage's L3/L1 lowering of the accepted `match` preserves it (every arm is emitted; a non-exhaustive match never reaches the lowerer). No regression to the compile-time tooth. |
//!
//! ## REQ status — 02-recursion-schemes.md (Basis Stage 2c, issue #70)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-6 (scheme → generated Verus recursive `spec fn` + `decreases <value>`) | SHIPPED | `emit_scheme_defs` (called from `lower` after `emit_combinator_defs`) GENERATES, per (ADT, scheme) the program uses (`collect_scheme_uses` resolving the scrutinee path → the param's `enum` type), the recursive `spec fn fold_<e>`/`for_all_<e>`/`map_<e>`/… (`emit_scheme_spec_fn`, `decreases l`, `*tail` deref, `Box::new` for `map`) reusing the Stage-1 recursive-fold shape. A scheme CALL lowers (`lower_expr` `Call` arm → `lower_scheme_call`) to a call of the generated `fold_<e>` with the step `Expr::Closure` lowered to a TYPED Verus `spec_fn` (`lower_step_closure`: `|x: u64, acc: nat| (<body>) as nat` for `fold`, `|x: u64| <body>` for `for_all`). Consumer: `lower`. Verified: real `verus --no-cheating` `verified, 0 errors` on the emitted `list_fold.th` (len_list/sum_list/all_positive → L3) — `tests/adt_schemes_conformance.rs::list_fold_lowers_to_generated_schemes_and_verifies_l3`. |
//! | REQ-7 (induction-discharged-once — the multiplier) | SHIPPED | `emit_fold_bound_law` GENERATES `fold_bound_<e>` (a `proof fn` parametric in the step `f` + a per-node premise, carrying the SINGLE `decreases l` induction, proving `fold_<e>(l, init, f) <= <e>_len(l) * b`) per ADT a `fold` folds over; `emit_len_measure` generates the structural measure `<e>_len`. An instance bound is proven by CITING the law with NO fresh induction. Consumer: `lower`. Verified: `verus --no-cheating` `verified, 0 errors` on the law + the GROUNDED `sum_list_bounded` instance (cites `fold_bound_list`, NO `decreases`) — `multiplier_instance_cites_the_generated_law_no_fresh_induction`; the NEGATIVE CONTROL (per-node premise removed) FAILS verus — `negative_control_premise_removed_fails_verus` (the induction is real, R-DEFER-9). |
//! | REQ-9 (`LowerError` extension, no panics) | SHIPPED | the scheme lowering reuses the existing `LowerError::Unsupported` (a scheme over a non-ADT value, an un-resolvable scrutinee) and `TooDeep`; no new variant needed. The DEC NUANCE is resolved: a scheme-CALL instance body (non-recursive — the recursion is in the generated `fold_<e>`) lowers WITHOUT a spurious `decreases` (`lower_spec_fn` suppresses it for `is_scheme_call_body`), while the generated `fold_<e>`/law carry their own `decreases l`. No `unwrap`/`expect`/`panic!` added (R-CODE-2 / R-APG-1). |
//! | REQ-3 (exec form — MONOMORPHIZED, OQ-2) | NOT-STARTED | epic **#62** Stage 2c. The SPEC scheme (the verified engine — the generated higher-order `fold_<e>` with the step passed as a `spec_fn`) is SHIPPED above. The MONOMORPHIZED EXEC mirror (an inlined `decreases`-bearing loop, the `conformance/sum.th` while-loop shape) is NOT implemented: the v0.1 corpus `list_fold.th` is SPEC-ONLY (all three items are `spec fn`), so no exec scheme is exercised yet — `collect_scheme_uses` collects spec-fn uses only. The exec mirror lands when a corpus exec fn folds an ADT (no blocker filed; #62 Stage 2c owns it). |
//!
//! ## REQ status — 04-collections.md (Basis Stage 4, issue #73)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-5 (`Vec<T>` → vstd-`Vec` newtype; push/get/len; `fx alloc`; backing-agnostic) | SHIPPED | `lower_type` maps `Type::Vec(elem)` → `tvec_name` (`Vec<u64>` → `TVecU64`); `emit_vec_wrappers` (called from `lower` after `emit_scheme_defs`) materializes ONCE per element type the GROUNDED `TVec<elem>` newtype over `vstd::vec::Vec<elem>` with `well_formed` (`len() <= CAP`), spec `len`/`spec_get`, the no-OOB exec `get` (`req i < len`), and the capacity-preserving exec `push` (`req well_formed && len < CAP`, `ens final(self)...` — the `final(self)` &mut grounding finding). Spec-position `v.get(i)` lowers to `v.spec_get(i as int)` (`lower_expr` MethodCall arm). `fx alloc` emits no verus annotation (effects are a Thermite-level row; `push`'s allocation is the Stage-1 `Effect::Alloc`, accepted by effect-subsumption since `push` is an intrinsic, not a declared callee). Consumer: `lower`. Verified: real `verus --no-cheating` on the emitted `vec_demo.th` — `checked_get`/`push_one` `verified, 0 errors` (`thermite-lower/tests/collections_conformance.rs`); the no-`req` `get` reject FAILS (non-vacuity, L0). BACKING-AGNOSTIC (#62): the surface contract names `len`/`get`/`push` over `v@`, never `vstd::vec::Vec`; a later custom-backing decouple swaps `TVec`'s `data` field WITHOUT changing user `.th` code. |
//! | REQ-6 (`Map<K,V>` → vstd `Map` wrapper) | NOT-STARTED | epic **#62** Stage 4 (OQ-3 thin-first-cut, v1.1). No `Type::Map` node / no `Map` lowering; the v1 corpus oracle (`conformance/vec_demo.th`) is `Vec`-only. Modeled on `vstd::map::Map`, deferred to a Stage-4 follow-up under #62. |
//! | REQ-7 (`LowerError` extension, no panics) | SHIPPED | the `Vec` lowering reuses the existing `LowerError::Unsupported` (`tvec_name` on a non-primitive element type) — no new variant needed; no `unwrap`/`expect`/`panic!` added (R-CODE-2 / R-APG-1). |

use std::fmt::Write as _;

use thermite_syntax::ast::{
    BinOp, Block, Clause, EnumItem, Expr, FnItem, IndexArg, Item, MatchArm, Param, Pattern,
    PrimType, Program, SlicePat, SpecFnItem, Stmt, Type, UnaryOp, VariantDef, VariantShape,
};
use thermite_syntax::lexer::Span;

/// The maximum recursive-descent emission depth before `lower` returns
/// `LowerError::TooDeep`. The lowerer recurses over the AST (expressions,
/// blocks, statements, types, patterns); like `thermite-syntax`'s parser guard
/// (its `MAX_RECURSION_DEPTH`, the #29/#31/#32 lesson) a single shared counter
/// bounds EVERY recursive family here so a pathological (or adversarial,
/// post-recovery) AST cannot overflow the native stack and abort the process.
/// Fixed constant (determinism, `goal.md` R-CODE-5). Set well above any
/// human-authored nesting; `thermite-syntax` itself caps parse nesting at 64, so
/// a well-formed AST cannot exceed that — this is a defensive backstop.
const MAX_EMIT_DEPTH: usize = 256;

/// `thermite-lower`'s own error type — born here with this crate's first
/// fallible function (`.design/scaffold/workspace.md` REQ-3). Span-bearing
/// (reusing `thermite_syntax::lexer::Span`) and `Display`-able. No panics
/// (`goal.md` R-CODE-2 / R-APG-1): an un-lowerable construct is an `Err`, never
/// an `unwrap`/`expect`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LowerError {
    /// A combinator call whose callee path is not in the `thermite-spec`
    /// registry. Validation (#2) should have caught this; the lowerer re-checks
    /// defensively (verus-lowering.md REQ-9).
    UnknownCombinator { name: String, span: Span },
    /// An expression/type/statement nested past `MAX_EMIT_DEPTH` — surfaced
    /// structurally so input can never overflow the C stack (REQ-9, R-CODE-2).
    TooDeep { limit: usize, span: Span },
    /// A construct the v0.1 lowering does not cover (e.g. a `Type` or `Expr`
    /// shape outside the corpus mapping tables). Carries a human description.
    Unsupported { what: String, span: Span },
    /// A call site where the caller's `fx` row does NOT subsume the callee's
    /// (`.design/lower/effect-subsumption.md` REQ-4; `thermite-design.md` §4.1
    /// "a caller's row must subsume every callee's row"). `missing` names the
    /// atomic effects the callee has that the caller's row lacks
    /// (`effects(callee) \ effects(caller)`), so the diagnostic tells the agent
    /// exactly which effect to add to the caller's row (or remove from the
    /// callee). Produced by `effects::check_effects`; NEVER a panic (R-CODE-2).
    EffectNotSubsumed {
        caller: String,
        callee: String,
        missing: Vec<thermite_syntax::ast::Effect>,
        span: Span,
    },
}

impl std::fmt::Display for LowerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LowerError::UnknownCombinator { name, span } => write!(
                f,
                "unknown combinator `{name}` at byte {}..{} (not in the SpecTherm registry)",
                span.start,
                span.end()
            ),
            LowerError::TooDeep { limit, span } => write!(
                f,
                "expression nested past the lowerer's depth limit of {limit} at byte {}..{}",
                span.start,
                span.end()
            ),
            LowerError::Unsupported { what, span } => write!(
                f,
                "unsupported construct for L3 lowering: {what} at byte {}..{}",
                span.start,
                span.end()
            ),
            LowerError::EffectNotSubsumed {
                caller,
                callee,
                missing,
                span,
            } => {
                let atoms: Vec<String> = missing.iter().map(effect_atom_name).collect();
                write!(
                    f,
                    "effect row of `{caller}` does not subsume callee `{callee}` at byte {}..{}: \
                     missing effect(s) [{}] (add them to `{caller}`'s `fx` row or remove them from `{callee}`)",
                    span.start,
                    span.end(),
                    atoms.join(", ")
                )
            }
        }
    }
}

/// The surface atom name of an `Effect` for an `EffectNotSubsumed` diagnostic
/// (REQ-4). v0.1 subsumption is path-insensitive (`.design/lower/effect-subsumption.md`
/// OQ-1), so the carrier atoms (`read`/`write`/`net`) are reported by KIND
/// without their (empty) path argument — the agent's fix is to add the effect
/// kind to the caller's row.
fn effect_atom_name(effect: &thermite_syntax::ast::Effect) -> String {
    use thermite_syntax::ast::Effect;
    match effect {
        Effect::Read(_) => "read".to_string(),
        Effect::Write(_) => "write".to_string(),
        Effect::Net(_) => "net".to_string(),
        Effect::Alloc => "alloc".to_string(),
        Effect::Time => "time".to_string(),
        Effect::Rand => "rand".to_string(),
        Effect::Panic => "panic".to_string(),
        Effect::Diverge => "diverge".to_string(),
    }
}

impl std::error::Error for LowerError {}

/// Lowering position: spec (`requires`/`ensures`/`invariant`/`decreases` and
/// `spec fn` bodies) vs exec (`fn` bodies). Drives the slice→`Seq` rewrite
/// (REQ-5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pos {
    Exec,
    Spec,
}

/// Lowering context: the position plus the set of in-scope slice-typed
/// parameter names. In SPEC position a bare slice-param path `xs` becomes the
/// `vstd` view `xs@` (a `Seq<T>`) — REQ-5. The set is computed per item from the
/// parameter types (a SHAPE-derived fact, not a name list), so the `@` rewrite
/// generalizes to any slice-typed parameter.
#[derive(Debug, Clone, Copy)]
struct Ctx<'a> {
    pos: Pos,
    slices: &'a [&'a str],
    /// Names of `spec fn`s lowered with a `nat` return type (the head-fold-sum
    /// shape — OQ-1). An `Eq` between a `u64`-valued scalar and a call to one of
    /// these coerces the scalar with `as nat`, since `nat` and `u64` are not the
    /// same Verus type. Computed program-wide, SHAPE-derived.
    nat_fns: &'a [&'a str],
    /// The program's `(variant_name, enum_name)` map (REQ-9): a `match` arm /
    /// pattern over a user enum variant lowers to the Verus-required ENUM-QUALIFIED
    /// path `Enum::Variant` (verus rejects a bare `Nil`/`Circle`). `Some`/`None`
    /// and slice patterns are NOT in this map, so they lower unqualified (Verus
    /// knows the `Option` built-in) — the qualification is keyed on membership.
    variants: &'a [(&'a str, &'a str)],
    /// True inside the body of a `nat`-returning spec fn (REQ-10): an integer
    /// cast (`h as u64`) coerces to `as nat` so the fold's arithmetic stays `nat`
    /// (no overflow obligation in spec context), the GROUNDED `sum_list` form.
    nat_ret: bool,
    /// Basis Stage 2 (`.design/basis/02-recursion-schemes.md` REQ-6): the
    /// recursion-scheme bindings IN SCOPE for the spec fn currently being lowered
    /// — one per (scheme name → resolved generated fn + element/result types) the
    /// fn's scrutinee resolves to. A scheme CALL `fold(l, 0, |x, acc| …)` lowers
    /// (in `lower_expr`'s `Call` arm) to a CALL of the generated `fold_<e>` with
    /// the step closure lowered to a typed `spec_fn`. EMPTY for a non-scheme fn
    /// (byte-stable for the existing corpus).
    schemes: &'a [SchemeBinding],
    /// Basis Stage 7 (`.design/basis/07-strings.md` REQ-4): the names that denote
    /// a `String` value IN SCOPE for the spec context currently being lowered —
    /// every `String`/`&String` parameter plus `result` when the return type is
    /// `String`. A `String` receiver's `.len()` / `.byte_at(i)` in SPEC position
    /// rewrites to the wrapper's spec fns `.spec_len()` / `.spec_byte_at(i as int)`
    /// (the exec `len`/`byte_at` return `u64` and cannot be named in a contract; a
    /// Verus spec index is `int`). Keyed on the receiver being a `String`-named
    /// path so a `Vec` receiver's `.len()` (whose wrapper spec fn IS named `len`)
    /// is UNCHANGED — the rewrite is `String`-specific. EMPTY for a non-`String`
    /// fn (byte-stable for the existing corpus).
    strings: &'a [&'a str],
    /// Basis Stage 7 (`.design/basis/07-strings.md` REQ-4): the program-wide set of
    /// FIELD names whose declared type reaches `String` (the editor core `Buf {
    /// text: String }`). A spec-position method call whose receiver is a FIELD
    /// access `<x>.<field>` where `<field>` is in this set rewrites `.len()`/
    /// `.byte_at(i)` to the wrapper SPEC fns `.spec_len()`/`.spec_byte_at(i as int)`
    /// — the field analog of `strings` (which keys a bare `String` VALUE path). A
    /// contract `b.text.len()` / `result.text.len()` over a `String` field needs the
    /// spec accessor (the exec `len`/`byte_at` cannot be named in a contract). EMPTY
    /// for a program with no `String` field (byte-stable for the existing corpus).
    string_fields: &'a [&'a str],
}

/// A resolved recursion-scheme binding in scope while lowering a spec fn body
/// (REQ-6): the surface scheme name (`fold`), the generated Verus fn name it
/// lowers to (`fold_list`), the ADT element type (`u64` — the step's element
/// parameter type), and the scheme's result kind (drives the step's accumulator
/// type + the `as nat` coercion of the step body).
#[derive(Debug, Clone)]
struct SchemeBinding {
    scheme_name: &'static str,
    gen_name: String,
    elem_ty: String,
    result: thermite_spec::SchemeResult,
}

const NO_SCHEMES: &[SchemeBinding] = &[];

const NO_SLICES: &[&str] = &[];
const NO_VARIANTS: &[(&str, &str)] = &[];

impl<'a> Ctx<'a> {
    fn exec() -> Ctx<'static> {
        Ctx {
            pos: Pos::Exec,
            slices: NO_SLICES,
            nat_fns: NO_SLICES,
            variants: NO_VARIANTS,
            nat_ret: false,
            schemes: NO_SCHEMES,
            strings: NO_SLICES,
            string_fields: NO_SLICES,
        }
    }
    fn spec(slices: &'a [&'a str], nat_fns: &'a [&'a str]) -> Ctx<'a> {
        Ctx {
            pos: Pos::Spec,
            slices,
            nat_fns,
            variants: NO_VARIANTS,
            nat_ret: false,
            schemes: NO_SCHEMES,
            strings: NO_SLICES,
            string_fields: NO_SLICES,
        }
    }
    /// A spec context with no slice-view names — for positions where every
    /// slice value is already a `Seq` (spec-fn bodies, whose slice params are
    /// `Seq<T>`) or where no slice appears (scalar predicates, literals).
    fn spec_seq() -> Ctx<'static> {
        Ctx {
            pos: Pos::Spec,
            slices: NO_SLICES,
            nat_fns: NO_SLICES,
            variants: NO_VARIANTS,
            nat_ret: false,
            schemes: NO_SCHEMES,
            strings: NO_SLICES,
            string_fields: NO_SLICES,
        }
    }
    /// This context with the enum-variant map attached (REQ-9 — variant-pattern
    /// qualification). Carried through `match`/pattern lowering.
    fn with_variants(mut self, variants: &'a [(&'a str, &'a str)]) -> Ctx<'a> {
        self.variants = variants;
        self
    }
    /// This context marked as a `nat`-returning spec-fn body (REQ-10 — integer
    /// casts coerce to `as nat`).
    fn with_nat_ret(mut self, nat_ret: bool) -> Ctx<'a> {
        self.nat_ret = nat_ret;
        self
    }
    /// This context with the recursion-scheme bindings in scope (REQ-6 — scheme
    /// CALL lowering). Carried into the spec-fn body so `lower_expr` rewrites a
    /// scheme call to a call of the generated `fold_<e>`.
    fn with_schemes(mut self, schemes: &'a [SchemeBinding]) -> Ctx<'a> {
        self.schemes = schemes;
        self
    }
    /// This context with the `String`-named values in scope (REQ-4 — a `String`
    /// receiver's spec-position `.len()`/`.byte_at(i)` rewrite). Carried into the
    /// signature `requires`/`ensures` lowering so a contract over a `String` param
    /// or `result` names the wrapper's spec fns.
    fn with_strings(mut self, strings: &'a [&'a str]) -> Ctx<'a> {
        self.strings = strings;
        self
    }
    /// This context with the program-wide `String`-typed FIELD names in scope
    /// (REQ-4 — a `String` FIELD receiver's spec-position `.len()`/`.byte_at(i)`
    /// rewrite). The field analog of [`Ctx::with_strings`].
    fn with_string_fields(mut self, string_fields: &'a [&'a str]) -> Ctx<'a> {
        self.string_fields = string_fields;
        self
    }
    /// True if `name` denotes a `String` value in scope (drives the spec-position
    /// `.len()`→`.spec_len()` / `.byte_at(i)`→`.spec_byte_at(i as int)` rewrite).
    fn is_string(&self, name: &str) -> bool {
        self.strings.contains(&name)
    }
    /// True if `name` is a program field whose type reaches `String` (drives the
    /// spec-position `<x>.<field>.len()`→`<x>.<field>.spec_len()` rewrite). REQ-4.
    fn is_string_field(&self, name: &str) -> bool {
        self.string_fields.contains(&name)
    }
    /// The in-scope scheme binding for a callee `name` (REQ-6), or `None` if
    /// `name` is not a scheme call resolved for the current fn.
    fn scheme_binding(&self, name: &str) -> Option<&'a SchemeBinding> {
        self.schemes.iter().find(|b| b.scheme_name == name)
    }
    fn is_spec(&self) -> bool {
        self.pos == Pos::Spec
    }
    /// True if `name` is an in-scope slice-typed parameter (gets `@` in spec).
    fn is_slice(&self, name: &str) -> bool {
        self.slices.contains(&name)
    }
    /// True if `name` is a `nat`-returning spec fn (drives `as nat` coercion).
    fn is_nat_fn(&self, name: &str) -> bool {
        self.nat_fns.contains(&name)
    }
    /// The enum name a user variant belongs to (REQ-9), or `None` if `name` is not
    /// a declared user variant (`Some`/`None`/a binding/literal — left unqualified).
    fn enum_of_variant(&self, name: &str) -> Option<&'a str> {
        self.variants
            .iter()
            .find(|(v, _)| *v == name)
            .map(|(_, e)| *e)
    }
    /// A clone of this spec context keeping its name sets (for recursing).
    fn keep(&self) -> Ctx<'a> {
        *self
    }
}

/// A span pointing at the very start of the source, used when an AST node we are
/// lowering does not itself carry a `Span` (the emitter recurses into spanless
/// sub-`Expr` nodes; the enclosing item's span is the best locus we have, and is
/// threaded down). Errors prefer the nearest enclosing span the caller passes.
fn zero_span() -> Span {
    Span::new(0, 0)
}

/// Lower a whole `Program` to a single Verus source file (REQ-1). Emits the
/// fixed prelude, a `verus! { .. }` block holding (1) the `spec fn` definitions
/// of every combinator the program's contracts reference, (2) the lowered items
/// in source order with their shape-derived proof aids, and (3) a trailing
/// `fn main() {}`.
pub fn lower(program: &Program) -> Result<String, LowerError> {
    let mut out = String::new();
    out.push_str("use vstd::prelude::*;\n");
    out.push_str("verus! {\n");

    // (1) combinator spec-fn definitions used anywhere in the program (REQ-6).
    let combinator_defs = emit_combinator_defs(program)?;
    out.push_str(&combinator_defs);

    // (1b) Basis Stage 2 (`.design/basis/02-recursion-schemes.md` REQ-6/REQ-7):
    // the GENERATED per-(ADT, scheme) Verus recursive `spec fn`s
    // (`fold_<e>`/`for_all_<e>`/…) + the structural measure `<e>_len` + the
    // induction-discharged-once law `fold_bound_<e>`, materialized ONCE BEFORE
    // their first use (a scheme call lowers to a CALL of `fold_<e>`). EMPTY when
    // the program uses no scheme (byte-stable for the non-scheme corpus).
    let scheme_defs = emit_scheme_defs(program)?;
    out.push_str(&scheme_defs);

    // (1c) Basis Stage 4 (`.design/basis/04-collections.md` REQ-5): the
    // bounded-`Vec` wrapper struct + its verified `len`/`spec_get`/`get`/`push`
    // impl, materialized ONCE per element type the program uses (a `Vec<u64>`
    // param/return → `TVecU64`), BEFORE any fn references it. EMPTY when the
    // program uses no `Vec` (byte-stable for the existing corpus — no regression).
    // The GROUNDED `BVec`-over-`vstd::vec::Vec<u64>` form (verus `verified, 0
    // errors`): the `well_formed` capacity invariant, the no-OOB `get`, the
    // capacity-preserving `push` with the `final(self)` &mut postcondition.
    let vec_wrappers = emit_vec_wrappers(program)?;
    out.push_str(&vec_wrappers);

    // (1d) Basis Stage 7 (`.design/basis/07-strings.md` REQ-4): the bounded
    // `String` wrapper struct `TString` over `vstd::vec::Vec<u8>` + its verified
    // `well_formed`/`spec_len`/`len`/`spec_byte_at`/`byte_at`/`concat`/`slice`
    // impl, materialized ONCE when the program uses `String`, BEFORE any fn
    // references it. EMPTY when the program uses no `String` (byte-stable for the
    // existing corpus — no regression). The GROUNDED `TString`-over-
    // `vstd::vec::Vec<u8>` form (verus `verified, 0 errors`): the `well_formed`
    // capacity invariant, the no-OOB `byte_at` (`req i < len`), the bounded
    // `concat`/`slice` with the `final`-free owned-value construction.
    let string_wrapper = emit_string_wrapper(program)?;
    out.push_str(&string_wrapper);

    // The program-wide set of `nat`-returning spec fns (the head-fold-sum shape,
    // OQ-1) — SHAPE-derived, used to coerce `u64`/`nat` equalities (`as nat`). An
    // ADT match-fold-sum spec fn (`sum_list`, REQ-10) joins this set: it too
    // returns `nat` so its integer arithmetic stays `nat` (no overflow obligation
    // in spec context), exactly as the slice head-fold does.
    // Basis Stage 2 (`.design/basis/02-recursion-schemes.md` REQ-6): a `fold`
    // scheme-CALL instance (the only `nat`-result scheme — `Accumulator`) also
    // returns `nat`, so it joins the `nat_fns` set exactly as a hand-written
    // ADT-fold-sum does (an `Eq` against it coerces `as nat`). Detected by SHAPE:
    // the body tail is a `Call` whose callee path resolves to the `fold` scheme.
    let nat_fns: Vec<&str> = program
        .items
        .iter()
        .filter_map(|item| match item {
            Item::SpecFn(s)
                if is_head_fold_sum(&s.body)
                    || is_adt_fold_sum(&s.body)
                    || is_fold_scheme_call_body(&s.body) =>
            {
                Some(s.name.as_str())
            }
            _ => None,
        })
        .collect();

    // The program-wide set of `struct` names that carry a type-invariant (REQ-8,
    // OQ-3 automatic threading): every `fn` taking or returning such a struct gets
    // the `<param>.well_formed()` / `result.well_formed()` conjunct woven into its
    // `requires`/`ensures` so Verus enforces the invariant at construction + use.
    let inv_structs: Vec<&str> = program
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Struct(s) if s.inv.is_some() => Some(s.name.as_str()),
            _ => None,
        })
        .collect();

    // Basis Stage 7 (`.design/basis/07-strings.md` REQ-4): the program-wide set of
    // FIELD names whose declared type reaches `String` — the editor core's `Buf {
    // text: String, .. }`. A contract reading `b.text.len()` / `result.text.len()`
    // (a String FIELD access receiver) must rewrite `.len()`/`.byte_at(i)` to the
    // wrapper SPEC fns `.spec_len()`/`.spec_byte_at(i as int)` (the exec `len`/
    // `byte_at` return `u64` and cannot be named in a contract — the same rule the
    // bare-`String`-value rewrite applies). Threaded into every fn's spec `Ctx`
    // (sorted+deduped for determinism, R-CODE-5). A field name is keyed alone (no
    // struct qualifier): v0.1 has no field-name overload across a `String` field and
    // a non-`String` field of the same name in scope, and the rewrite is inert
    // unless the method is `len`/`byte_at`.
    let mut string_field_names: Vec<&str> = program
        .items
        .iter()
        .flat_map(|item| -> Box<dyn Iterator<Item = &str>> {
            match item {
                Item::Struct(s) => Box::new(
                    s.fields
                        .iter()
                        .filter(|fd| ty_reaches_string(&fd.ty))
                        .map(|fd| fd.name.as_str()),
                ),
                Item::Enum(e) => Box::new(e.variants.iter().flat_map(|v| {
                    let fields: &[thermite_syntax::ast::FieldDef] = match &v.shape {
                        thermite_syntax::ast::VariantShape::Struct(fds) => fds,
                        _ => &[],
                    };
                    fields
                        .iter()
                        .filter(|fd| ty_reaches_string(&fd.ty))
                        .map(|fd| fd.name.as_str())
                })),
                _ => Box::new(std::iter::empty()),
            }
        })
        .collect();
    string_field_names.sort_unstable();
    string_field_names.dedup();

    // The program-wide `(variant_name, enum_name)` map (REQ-9): drives the
    // ENUM-QUALIFIED `Enum::Variant` lowering of a `match` arm / pattern over a
    // user enum value (verus rejects a bare `Nil`/`Circle`). Built once, threaded
    // through every `fn`/`spec fn` body's match lowering.
    let variants: Vec<(&str, &str)> = program
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Enum(e) => Some(e),
            _ => None,
        })
        .flat_map(|e| {
            e.variants
                .iter()
                .map(move |v| (v.name.as_str(), e.name.as_str()))
        })
        .collect();

    // (2) the lowered items, in source order (determinism, §5.3). A `fn` whose
    // loop carries an accumulator-fold invariant pulls in the auto-generated
    // push lemma for the folded spec fn (REQ-7 template a); the lemma def is
    // emitted at file scope right before the `fn` that uses it, deduped.
    let mut emitted_lemmas: Vec<String> = Vec::new();
    for item in &program.items {
        let item_src = match item {
            Item::SpecFn(s) => lower_spec_fn(s, &variants, program)?,
            Item::Fn(f) if f.boundary.is_some() || f.slag.is_some() => {
                // A boundary/slag fn is woven as a `#[verifier::external_body]`
                // signature (its body is never lowered, REQ-1), so it needs no
                // accumulator-fold push lemmas — skip the lemma collection that a
                // fully-proved fn body drives.
                lower_fn(f, &nat_fns, &inv_structs, &string_field_names, &variants)?
            }
            Item::Fn(f) => {
                for lemma_def in push_lemma_defs_for_fn(f)? {
                    let name_line = lemma_def.lines().next().unwrap_or("").to_string();
                    if emitted_lemmas.iter().any(|n| n == &name_line) {
                        continue;
                    }
                    out.push('\n');
                    out.push_str(&lemma_def);
                    out.push('\n');
                    emitted_lemmas.push(name_line);
                }
                lower_fn(f, &nat_fns, &inv_structs, &string_field_names, &variants)?
            }
            // Basis Stage 1c (`.design/basis/01-adts.md` REQ-8/REQ-10): a
            // `struct` lowers to a Verus `pub struct` + the `well_formed`
            // type-invariant predicate (REQ-8); a (recursive) `enum` lowers to a
            // Verus `enum` with `Box<T>` at the recursive occurrence (REQ-10).
            Item::Struct(s) => lower_struct(s)?,
            Item::Enum(e) => lower_enum(e)?,
        };
        out.push('\n');
        out.push_str(&item_src);
        out.push('\n');
    }

    out.push_str("\n}\nfn main() {}\n");
    Ok(out)
}

// ---------------------------------------------------------------------------
// REQ-8/REQ-9/REQ-10: ADT item lowering (struct, enum, recursive enum).
// ---------------------------------------------------------------------------

/// Lower a `StructItem` to a Verus `pub struct` plus, when it carries an `inv`
/// clause, the `well_formed` type-invariant predicate (REQ-8). The GROUNDED form
/// (`.design/basis/01-adts.md` "Struct + type invariant", verus `0 errors`):
///
/// ```verus
/// pub struct Account { pub balance: u64 }
/// impl Account {
///     pub open spec fn well_formed(&self) -> bool { self.balance <= 1000000 }
/// }
/// ```
///
/// VISIBILITY TIER (the recorded finding, REQ-8): a `pub open spec fn` body may
/// refer only to `pub` items, so the struct, ITS FIELDS, and the predicate are
/// all emitted `pub` — otherwise verus rejects with `field expression for a
/// non-visible datatype`. The `inv` expression is lowered with bare field-name
/// paths rewritten to `self.<field>` (the predicate's receiver), the
/// data-invariant the corpus `inv balance <= 1_000_000` denotes.
fn lower_struct(s: &thermite_syntax::ast::StructItem) -> Result<String, LowerError> {
    let mut out = String::new();
    writeln!(out, "pub struct {} {{", s.name).ok();
    for field in &s.fields {
        let ty = lower_type(&field.ty)?;
        writeln!(out, "    pub {}: {ty},", field.name).ok();
    }
    out.push_str("}\n");

    // The type-invariant predicate (REQ-8), when an `inv` clause is present. A
    // struct WITHOUT an invariant is a plain `pub struct` (no predicate, nothing
    // to thread — the OQ-3 threading in `lower_fn_signature` keys on `inv_structs`
    // which is exactly the invariant-bearing set).
    if let Some(inv) = &s.inv {
        let field_names: Vec<&str> = s.fields.iter().map(|f| f.name.as_str()).collect();
        // The subset of fields whose type reaches `String` (REQ-4): a `String`
        // field's `<field>.len()` / `<field>.byte_at(i)` inside the spec-position
        // `well_formed` predicate must name the wrapper SPEC fns
        // `.spec_len()`/`.spec_byte_at(i as int)` (the exec `len`/`byte_at` return
        // `u64` and cannot be named in a contract — the same rule the fn-signature
        // String rewrite applies). The editor core `inv cursor <= text.len()`.
        let string_fields: Vec<&str> = s
            .fields
            .iter()
            .filter(|f| ty_reaches_string(&f.ty))
            .map(|f| f.name.as_str())
            .collect();
        let body = lower_inv_expr(&inv.expr, &field_names, &string_fields, 0, s.span)?;
        writeln!(out, "\nimpl {} {{", s.name).ok();
        out.push_str("    pub open spec fn well_formed(&self) -> bool {\n");
        writeln!(out, "        {body}").ok();
        out.push_str("    }\n}\n");
    }
    Ok(out)
}

/// Lower an `inv` expression to the `well_formed(&self)` predicate body (REQ-8):
/// a bare single-segment path that names a declared field is rewritten to
/// `self.<field>` (the invariant `balance <= 1_000_000` is about `self.balance`).
/// Everything else lowers in spec position via the shared `lower_expr` — but the
/// field rewrite must happen on the AST, so this walks the expression itself.
fn lower_inv_expr(
    expr: &Expr,
    field_names: &[&str],
    string_fields: &[&str],
    depth: usize,
    span: Span,
) -> Result<String, LowerError> {
    if depth >= MAX_EMIT_DEPTH {
        return Err(LowerError::TooDeep {
            limit: MAX_EMIT_DEPTH,
            span,
        });
    }
    let d = depth + 1;
    match expr {
        // A bare field-name path becomes a `self.<field>` field access; any other
        // single/multi-segment path lowers normally (a `spec const` like a CAP,
        // or a `::`-qualified path, stays as written).
        Expr::Path(segs) => {
            if segs.len() == 1 && field_names.contains(&segs[0].as_str()) {
                Ok(format!("self.{}", segs[0]))
            } else {
                Ok(segs.join("::"))
            }
        }
        Expr::Binary { op, lhs, rhs } => {
            let l = lower_inv_operand(lhs, *op, true, field_names, string_fields, d, span)?;
            let r = lower_inv_operand(rhs, *op, false, field_names, string_fields, d, span)?;
            Ok(format!("{l} {} {r}", binop(*op)))
        }
        Expr::Field { receiver, name } => {
            let r = lower_inv_expr(receiver, field_names, string_fields, d, span)?;
            Ok(format!("{r}.{name}"))
        }
        // Basis Stage 7 (`.design/basis/07-strings.md` REQ-4): a method call inside
        // the spec-position `well_formed` predicate — the editor core `inv cursor <=
        // text.len()`. The receiver's bare field name is rewritten to `self.<field>`
        // (recursively, so a nested field receiver works too). When the receiver is a
        // `String`-typed field, `.len()`/`.byte_at(i)` rewrite to the wrapper SPEC fns
        // `.spec_len()`/`.spec_byte_at(i as int)` — the exec `len`/`byte_at` return
        // `u64` and CANNOT be named in a contract (the same rule the fn-signature
        // String rewrite applies; `lower_expr` MethodCall spec arm). A non-`String`
        // field's method call (e.g. a `Vec` field's `.len()`, whose wrapper spec fn IS
        // `len`) keeps the method name unchanged. Without this arm `text.len()` fell to
        // the catch-all `lower_expr`, which lowered the bare receiver `text` with NO
        // `self.` rewrite (`error[E0425]: cannot find value text`).
        Expr::MethodCall {
            receiver,
            name,
            args,
        } => {
            let r = lower_inv_expr(receiver, field_names, string_fields, d, span)?;
            let recv_is_string_field = matches!(
                receiver.as_ref(),
                Expr::Path(segs) if segs.len() == 1 && string_fields.contains(&segs[0].as_str())
            );
            if recv_is_string_field {
                if name == "len" && args.is_empty() {
                    return Ok(format!("{r}.spec_len()"));
                }
                if name == "byte_at" && args.len() == 1 {
                    // `spec_byte_at(i: int)`: an integer literal flows into the `int`
                    // parameter directly (Verus coerces a literal); a non-literal
                    // index gets the explicit `as int` Verus requires in spec
                    // position (the same split as the fn-signature byte_at rewrite).
                    let idx = if matches!(&args[0], Expr::IntLit { .. }) {
                        lower_inv_expr(&args[0], field_names, string_fields, d, span)?
                    } else {
                        lower_index_arg(&args[0], Ctx::spec_seq(), d, span)?
                    };
                    return Ok(format!("{r}.spec_byte_at({idx})"));
                }
            }
            let mut parts = Vec::with_capacity(args.len());
            for a in args {
                parts.push(lower_inv_expr(a, field_names, string_fields, d, span)?);
            }
            Ok(format!("{r}.{name}({})", parts.join(", ")))
        }
        // A literal / other leaf lowers exactly as the shared spec lowering would
        // (the field rewrite only matters for bare paths and their parents).
        _ => lower_expr(expr, Ctx::spec_seq(), depth, span),
    }
}

/// Parenthesize an `inv` binary operand the same way `lower_binary_operand` does,
/// but recursing through `lower_inv_expr` so nested field-name paths are rewritten
/// (REQ-8). Mirrors the precedence discipline of the exec/spec operand lowering.
fn lower_inv_operand(
    operand: &Expr,
    parent: BinOp,
    is_left: bool,
    field_names: &[&str],
    string_fields: &[&str],
    depth: usize,
    span: Span,
) -> Result<String, LowerError> {
    let s = lower_inv_expr(operand, field_names, string_fields, depth, span)?;
    if let Expr::Binary { op: child, .. } = operand {
        let pp = precedence(parent);
        let cp = precedence(*child);
        let needs = cp < pp || (!is_left && cp == pp);
        if needs {
            return Ok(format!("({s})"));
        }
    }
    Ok(s)
}

/// Lower an `EnumItem` to a Verus `enum` (REQ-9), recursive `Box<T>` and all
/// (REQ-10). The GROUNDED forms (`.design/basis/01-adts.md`, verus `0 errors`):
///
/// ```verus
/// enum Shape { Circle(u64), Rect { w: u64, h: u64 } }
/// enum List { Nil, Cons(u64, Box<List>) }
/// ```
///
/// A unit variant is the bare name; a tuple variant `Name(T, …)`; a struct
/// variant `Name { field: T, … }`. The recursive occurrence is a `Box<List>`
/// (`lower_type` emits `Box<…>` for `Type::Box`), the heap indirection Verus
/// dereferences with `*` (REQ-10). An enum is emitted WITHOUT `pub`: the corpus
/// `Shape`/`List` are used only from `fn`/`spec fn` in the same module, and a bare
/// `enum` matches the GROUNDED verified form (no `pub open spec fn` refers to it,
/// so no visibility-tier obligation as the struct invariant has).
fn lower_enum(e: &EnumItem) -> Result<String, LowerError> {
    let mut out = String::new();
    writeln!(out, "enum {} {{", e.name).ok();
    for variant in &e.variants {
        match &variant.shape {
            VariantShape::Unit => {
                writeln!(out, "    {},", variant.name).ok();
            }
            VariantShape::Tuple(tys) => {
                let mut parts = Vec::with_capacity(tys.len());
                for ty in tys {
                    parts.push(lower_type(ty)?);
                }
                writeln!(out, "    {}({}),", variant.name, parts.join(", ")).ok();
            }
            VariantShape::Struct(fields) => {
                let mut parts = Vec::with_capacity(fields.len());
                for field in fields {
                    parts.push(format!("{}: {}", field.name, lower_type(&field.ty)?));
                }
                writeln!(out, "    {} {{ {} }},", variant.name, parts.join(", ")).ok();
            }
        }
    }
    out.push_str("}\n");
    Ok(out)
}

// ---------------------------------------------------------------------------
// REQ-6: combinator Verus(L3) definitions, sourced from the #2 registry seam.
// ---------------------------------------------------------------------------

/// Collect (in deterministic source order, deduped) the combinator names the
/// program references anywhere in a contract/spec position, and emit each one's
/// frozen `verus_l3` `spec fn` definition from the `thermite-spec` registry
/// (REQ-6; closes the OQ-2 seam — this is the registry's #4 consumer per
/// R-DEFER-1). A referenced name with no registry entry is `UnknownCombinator`.
fn emit_combinator_defs(program: &Program) -> Result<String, LowerError> {
    let mut names: Vec<(String, Span)> = Vec::new();
    for item in &program.items {
        match item {
            Item::Fn(f) => {
                collect_combinators_in_expr(&f.contract.req.expr, f.span, &mut names);
                for ens in &f.contract.ens {
                    collect_combinators_in_expr(&ens.expr, f.span, &mut names);
                }
                // A boundary fn (ffi-boundary.md REQ-2) has `body: None` — its
                // `req`/`ens` combinators are collected above; no body to scan.
                if let Some(body) = &f.body {
                    collect_combinators_in_block_specs(body, f.span, &mut names);
                }
            }
            Item::SpecFn(s) => {
                collect_combinators_in_expr(&s.dec.expr, s.span, &mut names);
                collect_combinators_in_block_specs(&s.body, s.span, &mut names);
            }
            // Basis Stage 1a (`.design/basis/01-adts.md`): a `struct`/`enum`
            // item carries no contract clauses, so it references no combinators
            // — the neutral value for this collector is a no-op. (The item is
            // gated at the validator anyway; this arm is dead-in-1a.)
            Item::Struct(_) | Item::Enum(_) => {}
        }
    }

    let mut out = String::new();
    let mut emitted: Vec<&str> = Vec::new();
    for (name, span) in &names {
        if emitted.iter().any(|e| e == name) {
            continue;
        }
        let sig = thermite_spec::lookup(name).ok_or_else(|| LowerError::UnknownCombinator {
            name: name.clone(),
            span: *span,
        })?;
        out.push('\n');
        out.push_str(sig.verus_l3);
        out.push('\n');
        emitted.push(sig.name);
    }
    Ok(out)
}

/// Walk an expression collecting any callee path whose head segment is a
/// registered combinator name. Combinator calls are plain `Expr::Call` with a
/// `Path` callee (the frontend is registry-free — `ast.rs` module doc).
fn collect_combinators_in_expr(expr: &Expr, span: Span, acc: &mut Vec<(String, Span)>) {
    match expr {
        Expr::Call { callee, args } => {
            if let Expr::Path(segs) = callee.as_ref() {
                if let Some(last) = segs.last() {
                    if thermite_spec::lookup(last).is_some() {
                        acc.push((last.clone(), span));
                    }
                }
            }
            collect_combinators_in_expr(callee, span, acc);
            for a in args {
                collect_combinators_in_expr(a, span, acc);
            }
        }
        Expr::MethodCall { receiver, args, .. } => {
            collect_combinators_in_expr(receiver, span, acc);
            for a in args {
                collect_combinators_in_expr(a, span, acc);
            }
        }
        Expr::Field { receiver, .. } => collect_combinators_in_expr(receiver, span, acc),
        Expr::Closure { body, .. } => collect_combinators_in_expr(body, span, acc),
        Expr::Match { scrutinee, arms } => {
            collect_combinators_in_expr(scrutinee, span, acc);
            for arm in arms {
                collect_combinators_in_expr(&arm.body, span, acc);
            }
        }
        Expr::If { cond, then, else_ } => {
            collect_combinators_in_expr(cond, span, acc);
            collect_combinators_in_block_specs(then, span, acc);
            collect_combinators_in_block_specs(else_, span, acc);
        }
        Expr::Binary { lhs, rhs, .. } => {
            collect_combinators_in_expr(lhs, span, acc);
            collect_combinators_in_expr(rhs, span, acc);
        }
        Expr::Index { base, index } => {
            collect_combinators_in_expr(base, span, acc);
            match index {
                IndexArg::Single(e) | IndexArg::RangeTo(e) | IndexArg::RangeFrom(e) => {
                    collect_combinators_in_expr(e, span, acc)
                }
                IndexArg::Range(a, b) => {
                    collect_combinators_in_expr(a, span, acc);
                    collect_combinators_in_expr(b, span, acc);
                }
            }
        }
        Expr::Cast { expr, .. } | Expr::Ref { expr, .. } => {
            collect_combinators_in_expr(expr, span, acc)
        }
        // Basis Stage 1a (`.design/basis/01-adts.md`): the ADT expressions are
        // dead-in-1a (gated at the validator), but the honest collector value
        // is to descend into their sub-expressions — a combinator could in
        // principle appear in a struct-literal field value, an `is` scrutinee,
        // or a deref operand — so no referenced combinator is silently dropped.
        Expr::StructLit { fields, .. } => {
            for (_, value) in fields {
                collect_combinators_in_expr(value, span, acc);
            }
        }
        Expr::Is { scrutinee, .. } => collect_combinators_in_expr(scrutinee, span, acc),
        Expr::Deref(inner) => collect_combinators_in_expr(inner, span, acc),
        // The prefix `!` (#92): descend into the operand so a combinator inside
        // `!forall_in(...)` is still collected (recurse like the other unary arms).
        Expr::Unary { expr, .. } => collect_combinators_in_expr(expr, span, acc),
        // A string literal (`.design/basis/07-strings.md` REQ-1) references no
        // combinator — a value-carrying leaf, like an int/bool literal.
        Expr::IntLit { .. } | Expr::BoolLit(_) | Expr::Path(_) | Expr::StrLit(_) => {}
    }
}

/// Walk a block collecting combinators referenced in its SPEC positions: loop
/// `inv`/`dec` clauses. Body exec expressions never reference combinators in the
/// corpus, but loop invariants do (`binary_search`'s `forall_below`/`forall_from`).
fn collect_combinators_in_block_specs(block: &Block, span: Span, acc: &mut Vec<(String, Span)>) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Loop(l) => {
                for inv in &l.invs {
                    collect_combinators_in_expr(&inv.expr, span, acc);
                }
                collect_combinators_in_expr(&l.dec.expr, span, acc);
                collect_combinators_in_block_specs(&l.body, span, acc);
            }
            Stmt::If { then, else_, .. } => {
                collect_combinators_in_block_specs(then, span, acc);
                if let Some(e) = else_ {
                    collect_combinators_in_block_specs(e, span, acc);
                }
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Basis Stage 2 (`.design/basis/02-recursion-schemes.md` REQ-6/REQ-7): the
// GENERATED per-(ADT, scheme) Verus recursive `spec fn`s + the discharged-once
// law. A scheme call `fold(l, init, step)` lowers to a CALL of the generated
// `fold_<e>`, materialized here ONCE before its first use (the OQ-1 (b)
// MATERIALIZED-items resolution — shared across instances, in the audit surface).
// ---------------------------------------------------------------------------

/// A single (recursion scheme, recursive ADT) pair the program uses, resolved by
/// SHAPE: a scheme call whose scrutinee path resolves to a `spec fn` parameter of
/// a declared `enum` type. The lowerer materializes the scheme's generated
/// `spec fn` over this ADT (`fold_<e>`/`for_all_<e>`/…) + the structural measure +
/// the `fold_bound_<e>` law (REQ-6/REQ-7).
struct SchemeUse {
    scheme: &'static thermite_spec::SchemeSig,
    /// The declared `enum` the scheme folds over (the scrutinee's type).
    enum_name: String,
    /// The element type of the ADT's recursive variant (the first non-`Box`
    /// field — `u64` for `Cons(u64, Box<List>)`), the step's element parameter
    /// type. The GROUNDED forms are all `u64`-element.
    elem_ty: String,
    /// The recursive variant's name (`Cons`) and the base (unit/value) variant(s),
    /// resolved from the enum decl, so the generated `match` is ENUM-QUALIFIED and
    /// recurses through the `Box`-deref'd recursive field.
    enum_item: EnumItem,
}

/// Emit the GENERATED scheme definitions for every (scheme, ADT) pair the program
/// uses (REQ-6/REQ-7), in a DETERMINISTIC order (R-CODE-5), deduped. For each ADT
/// that ANY scheme folds over, the structural measure `<e>_len` is emitted once;
/// for each used scheme over that ADT the recursive `spec fn` (`fold_<e>`/
/// `for_all_<e>`/…); and for each `fold` over that ADT the induction law
/// `fold_bound_<e>`. EMPTY when the program uses no scheme (the non-scheme corpus
/// is byte-stable — no regression). The forms reproduce the GROUNDED Verus
/// (`9 verified, 0 errors`) of `.design/basis/02-recursion-schemes.md`.
fn emit_scheme_defs(program: &Program) -> Result<String, LowerError> {
    let uses = collect_scheme_uses(program)?;
    if uses.is_empty() {
        return Ok(String::new());
    }

    let mut out = String::new();

    // (a) the structural measure `<e>_len` once per ADT any scheme folds over —
    // the `fold_bound_<e>` law's `len_<e>(l) * b` bound references it. Deduped by
    // enum name, deterministic source-order traversal.
    let mut measured: Vec<String> = Vec::new();
    for u in &uses {
        if measured.iter().any(|m| m == &u.enum_name) {
            continue;
        }
        out.push('\n');
        out.push_str(&emit_len_measure(u)?);
        out.push('\n');
        measured.push(u.enum_name.clone());
    }

    // (b) each used scheme's generated recursive `spec fn`, deduped by the
    // generated name (`fold_list`/`for_all_list`/…).
    let mut emitted: Vec<String> = Vec::new();
    for u in &uses {
        let name = u.scheme.generated_fn_name(&u.enum_name);
        if emitted.iter().any(|e| e == &name) {
            continue;
        }
        out.push('\n');
        out.push_str(&emit_scheme_spec_fn(u)?);
        out.push('\n');
        emitted.push(name);
    }

    // (c) the `fold_bound_<e>` induction-discharged-once law for each ADT a `fold`
    // folds over (REQ-7). Deduped by enum name.
    let mut lawed: Vec<String> = Vec::new();
    for u in &uses {
        if u.scheme.name != "fold" {
            continue;
        }
        if lawed.iter().any(|m| m == &u.enum_name) {
            continue;
        }
        out.push('\n');
        out.push_str(&emit_fold_bound_law(u)?);
        out.push('\n');
        lawed.push(u.enum_name.clone());
    }

    Ok(out)
}

/// Collect every (scheme, ADT) pair the program uses (REQ-6). A scheme call is an
/// `Expr::Call` whose callee `Path` resolves via `thermite_spec::schemes::lookup`;
/// the ADT is the type of its scrutinee (first) argument, resolved against the
/// enclosing `spec fn`/`fn`'s parameter types (the AST is untyped — OQ-3 — so the
/// scrutinee path → param-type resolution is the SHAPE-decidable mapping). A
/// scheme over a value whose type is not a declared `enum` is `Unsupported`
/// (REQ-9 — never a panic).
fn collect_scheme_uses(program: &Program) -> Result<Vec<SchemeUse>, LowerError> {
    // The declared enums, by name.
    let enums: std::collections::BTreeMap<&str, &EnumItem> = program
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Enum(e) => Some((e.name.as_str(), e)),
            _ => None,
        })
        .collect();

    let mut uses: Vec<SchemeUse> = Vec::new();
    for item in &program.items {
        let (params, body, span) = match item {
            Item::SpecFn(s) => (&s.params, &s.body, s.span),
            // A scheme in an exec `fn` body is the MONOMORPHIZED exec form (OQ-2);
            // the v0.1 corpus is spec-only, so an exec scheme is out of the
            // grounded path — collect spec-fn scheme uses only here.
            _ => continue,
        };
        collect_scheme_uses_in_block(body, params, &enums, span, &mut uses)?;
    }
    Ok(uses)
}

/// Walk a `spec fn` body block collecting scheme uses (REQ-6).
fn collect_scheme_uses_in_block(
    block: &Block,
    params: &[Param],
    enums: &std::collections::BTreeMap<&str, &EnumItem>,
    span: Span,
    uses: &mut Vec<SchemeUse>,
) -> Result<(), LowerError> {
    for stmt in &block.stmts {
        if let Stmt::Expr(e) | Stmt::Return(Some(e)) | Stmt::Let { init: e, .. } = stmt {
            collect_scheme_uses_in_expr(e, params, enums, span, uses)?;
        }
    }
    if let Some(tail) = &block.tail {
        collect_scheme_uses_in_expr(tail, params, enums, span, uses)?;
    }
    Ok(())
}

/// Walk an expression collecting scheme uses (REQ-6). A scheme call's scrutinee
/// path is resolved to an enclosing parameter's `enum` type.
fn collect_scheme_uses_in_expr(
    expr: &Expr,
    params: &[Param],
    enums: &std::collections::BTreeMap<&str, &EnumItem>,
    span: Span,
    uses: &mut Vec<SchemeUse>,
) -> Result<(), LowerError> {
    if let Expr::Call { callee, args } = expr {
        if let Expr::Path(segs) = callee.as_ref() {
            if let Some(name) = segs.last() {
                if let Some(scheme) = thermite_spec::schemes::lookup(name) {
                    let enum_item = resolve_scheme_adt(scheme, args, params, enums, span)?;
                    let elem_ty = recursive_elem_type(&enum_item, span)?;
                    if !uses
                        .iter()
                        .any(|u| u.scheme.name == scheme.name && u.enum_name == enum_item.name)
                    {
                        uses.push(SchemeUse {
                            scheme,
                            enum_name: enum_item.name.clone(),
                            elem_ty,
                            enum_item,
                        });
                    }
                }
            }
        }
    }
    // Recurse sub-expressions (a scheme call may be nested in arithmetic — though
    // the validator caps the step to a flat closure, the instance body itself can
    // wrap the call, e.g. `fold(...) + 0`). The depth is bounded by the source
    // structure; the validator already enforced the contract limit.
    each_subexpr(expr, &mut |e| {
        collect_scheme_uses_in_expr(e, params, enums, span, uses)
    })
}

/// Apply `f` to each immediate sub-expression of `expr` (a shared structural
/// walk), short-circuiting on the first `Err`. Used by the scheme-use collector.
fn each_subexpr(
    expr: &Expr,
    f: &mut impl FnMut(&Expr) -> Result<(), LowerError>,
) -> Result<(), LowerError> {
    match expr {
        Expr::Call { callee, args } => {
            f(callee)?;
            for a in args {
                f(a)?;
            }
        }
        Expr::MethodCall { receiver, args, .. } => {
            f(receiver)?;
            for a in args {
                f(a)?;
            }
        }
        Expr::Field { receiver, .. } => f(receiver)?,
        Expr::Closure { body, .. } => f(body)?,
        Expr::Match { scrutinee, arms } => {
            f(scrutinee)?;
            for arm in arms {
                f(&arm.body)?;
            }
        }
        Expr::If { cond, then, else_ } => {
            f(cond)?;
            each_block_subexpr(then, f)?;
            each_block_subexpr(else_, f)?;
        }
        Expr::Binary { lhs, rhs, .. } => {
            f(lhs)?;
            f(rhs)?;
        }
        Expr::Index { base, index } => {
            f(base)?;
            match index {
                IndexArg::Single(e) | IndexArg::RangeTo(e) | IndexArg::RangeFrom(e) => f(e)?,
                IndexArg::Range(a, b) => {
                    f(a)?;
                    f(b)?;
                }
            }
        }
        Expr::Cast { expr, .. } | Expr::Ref { expr, .. } | Expr::Deref(expr) => f(expr)?,
        // The prefix `!` (#92): descend into its single operand.
        Expr::Unary { expr, .. } => f(expr)?,
        Expr::StructLit { fields, .. } => {
            for (_, v) in fields {
                f(v)?;
            }
        }
        Expr::Is { scrutinee, .. } => f(scrutinee)?,
        // A string literal (`.design/basis/07-strings.md` REQ-1) is a value-
        // carrying leaf with no sub-expression — like an int/bool literal.
        Expr::IntLit { .. } | Expr::BoolLit(_) | Expr::Path(_) | Expr::StrLit(_) => {}
    }
    Ok(())
}

/// Apply `f` to each sub-expression of a block (for `each_subexpr`'s `If` arms).
fn each_block_subexpr(
    block: &Block,
    f: &mut impl FnMut(&Expr) -> Result<(), LowerError>,
) -> Result<(), LowerError> {
    for stmt in &block.stmts {
        if let Stmt::Expr(e) | Stmt::Return(Some(e)) | Stmt::Let { init: e, .. } = stmt {
            f(e)?;
        }
    }
    if let Some(tail) = &block.tail {
        f(tail)?;
    }
    Ok(())
}

/// Resolve the declared `enum` a scheme call folds over (REQ-6): the scrutinee
/// (first) argument is a bare path naming a parameter whose type is `Named(E)`
/// for a declared `enum E`. An un-resolvable scrutinee (non-path, unknown param,
/// non-enum type) is `Unsupported` (REQ-9 — a scheme over a non-ADT value).
fn resolve_scheme_adt(
    scheme: &thermite_spec::SchemeSig,
    args: &[Expr],
    params: &[Param],
    enums: &std::collections::BTreeMap<&str, &EnumItem>,
    span: Span,
) -> Result<EnumItem, LowerError> {
    let scrutinee = args.first().ok_or_else(|| LowerError::Unsupported {
        what: format!(
            "recursion scheme `{}` with no scrutinee argument",
            scheme.name
        ),
        span,
    })?;
    let Expr::Path(segs) = scrutinee else {
        return Err(LowerError::Unsupported {
            what: format!(
                "recursion scheme `{}` scrutinee must be a bare value path",
                scheme.name
            ),
            span,
        });
    };
    let pname = segs.last().map(|s| s.as_str()).unwrap_or_default();
    let ty = params
        .iter()
        .find(|p| p.name == pname)
        .map(|p| &p.ty)
        .ok_or_else(|| LowerError::Unsupported {
            what: format!(
                "recursion scheme `{}` scrutinee `{pname}` is not a parameter",
                scheme.name
            ),
            span,
        })?;
    let Type::Named(enum_name) = ty else {
        return Err(LowerError::Unsupported {
            what: format!(
                "recursion scheme `{}` scrutinee `{pname}` is not a declared `enum` value",
                scheme.name
            ),
            span,
        });
    };
    enums
        .get(enum_name.as_str())
        .map(|e| (*e).clone())
        .ok_or_else(|| LowerError::Unsupported {
            what: format!(
                "recursion scheme `{}` over `{enum_name}`, which is not a declared `enum`",
                scheme.name
            ),
            span,
        })
}

/// The element type of an ADT's recursive variant — the first NON-`Box` field of
/// the variant that carries a `Box<Self>` recursive occurrence (`u64` for
/// `Cons(u64, Box<List>)`). The step's element parameter type (REQ-6). An enum
/// with no recursive `Box` variant is not a recursion-scheme target → `Unsupported`.
fn recursive_elem_type(e: &EnumItem, span: Span) -> Result<String, LowerError> {
    for variant in &e.variants {
        if let VariantShape::Tuple(tys) = &variant.shape {
            let has_box = tys.iter().any(|t| matches!(t, Type::Box(_)));
            if has_box {
                if let Some(elem) = tys.iter().find(|t| !matches!(t, Type::Box(_))) {
                    return lower_type(elem);
                }
            }
        }
    }
    Err(LowerError::Unsupported {
        what: format!(
            "recursion scheme over `{}`: no recursive `Box<{}>` variant with an element field",
            e.name, e.name
        ),
        span,
    })
}

/// The recursive variant (the one carrying a `Box<Self>`) and the variant set, so
/// the generated `match` is ENUM-QUALIFIED and recurses through the deref'd field.
/// Returns `(base_variants, recursive_variant_name)`. The base variants are every
/// non-recursive variant (unit `Nil` / value `Leaf(v)`).
fn enum_variant_split(e: &EnumItem) -> (Vec<&VariantDef>, Option<&VariantDef>) {
    let mut base = Vec::new();
    let mut recursive = None;
    for variant in &e.variants {
        let is_rec = matches!(&variant.shape, VariantShape::Tuple(tys) if tys.iter().any(|t| matches!(t, Type::Box(_))));
        if is_rec {
            recursive = Some(variant);
        } else {
            base.push(variant);
        }
    }
    (base, recursive)
}

/// Emit the structural measure `<e>_len(l: E) -> nat decreases l` (REQ-6/REQ-7):
/// the `len_list`-shaped count the `fold_bound_<e>` law multiplies. The recursive
/// arm counts `1 + <e>_len(*tail)`; each base arm contributes `0`. GROUNDED
/// (`list_len`, part of the `9 verified` run). The generated name is `<e>_len`
/// (e.g. `list_len`), DISTINCT from any surface `len_<e>` fold instance so the
/// two never collide (the corpus `len_list` is a `fold` instance).
fn emit_len_measure(u: &SchemeUse) -> Result<String, LowerError> {
    let e = &u.enum_item;
    let lname = format!("{}_len", e.name.to_ascii_lowercase());
    let (base, recursive) = enum_variant_split(e);
    let rec = recursive.ok_or_else(|| LowerError::Unsupported {
        what: format!("ADT `{}` has no recursive variant for a measure", e.name),
        span: zero_span(),
    })?;
    let mut out = String::new();
    writeln!(out, "spec fn {lname}(l: {}) -> nat", e.name).map_err(|_| fmt_err())?;
    out.push_str("    decreases l,\n{\n    match l {\n");
    for b in &base {
        writeln!(out, "        {}::{} => 0,", e.name, base_variant_pattern(b))
            .map_err(|_| fmt_err())?;
    }
    writeln!(
        out,
        "        {}::{}(x, tail) => 1 + {lname}(*tail),",
        e.name, rec.name
    )
    .map_err(|_| fmt_err())?;
    out.push_str("    }\n}\n");
    Ok(out)
}

/// The arm pattern for a BASE variant in a generated `match` (REQ-6): a unit
/// variant is its bare name (`Nil`); a value-carrying tuple base binds its field
/// (`Leaf(v)`). v0.1 grounded corpus is unit-base (`Nil`); the value-base shape
/// is supported for the `Tree` generalization.
fn base_variant_pattern(v: &VariantDef) -> String {
    match &v.shape {
        VariantShape::Unit => v.name.clone(),
        VariantShape::Tuple(tys) => {
            let binds: Vec<String> = (0..tys.len()).map(|i| format!("v{i}")).collect();
            format!("{}({})", v.name, binds.join(", "))
        }
        VariantShape::Struct(_) => v.name.clone(),
    }
}

/// Emit the generated recursive scheme `spec fn` over the ADT (REQ-6): `fold_<e>`
/// (`-> nat`, `decreases l`, applies the passed `spec_fn` step at each recursive
/// node), `for_all_<e>`/`exists_<e>`/`traverse_<e>` (`-> bool`), or `map_<e>`
/// (`-> E`, `Box::new`-reconstructing). GROUNDED forms (`fold_list`/`for_all_list`/
/// `map_list`, `decreases l`, `*tail`).
fn emit_scheme_spec_fn(u: &SchemeUse) -> Result<String, LowerError> {
    use thermite_spec::{SchemeResult, StepShape};
    let e = &u.enum_item;
    let elem = &u.elem_ty;
    let fname = u.scheme.generated_fn_name(&e.name);
    let (base, recursive) = enum_variant_split(e);
    let rec = recursive.ok_or_else(|| LowerError::Unsupported {
        what: format!(
            "ADT `{}` has no recursive variant for scheme `{}`",
            e.name, u.scheme.name
        ),
        span: zero_span(),
    })?;

    // The step `spec_fn` type + the seed/return type, by the scheme's result kind.
    let (step_ty, ret_ty, seed_param) = match (u.scheme.step_shape, u.scheme.result) {
        (StepShape::ElementAcc, SchemeResult::Accumulator) => (
            format!("spec_fn({elem}, nat) -> nat"),
            "nat".to_string(),
            Some(("init", "nat")),
        ),
        (StepShape::ElementAcc, SchemeResult::Bool) => (
            format!("spec_fn({elem}, bool) -> bool"),
            "bool".to_string(),
            Some(("init", "bool")),
        ),
        (StepShape::Element, SchemeResult::Bool) => {
            (format!("spec_fn({elem}) -> bool"), "bool".to_string(), None)
        }
        (StepShape::Element, SchemeResult::SameAdt) => {
            (format!("spec_fn({elem}) -> {elem}"), e.name.clone(), None)
        }
        // The remaining (shape, result) combinations are not in the frozen scheme
        // set (the registry pairs each shape with one result); unreachable for a
        // registered scheme, surfaced structurally rather than panicking.
        (shape, result) => {
            return Err(LowerError::Unsupported {
                what: format!(
                "scheme `{}` has an unmodeled (step-shape {shape:?}, result {result:?}) pairing",
                u.scheme.name
            ),
                span: zero_span(),
            })
        }
    };

    let mut out = String::new();
    write!(out, "spec fn {fname}(l: {}", e.name).map_err(|_| fmt_err())?;
    if let Some((sn, st)) = seed_param {
        write!(out, ", {sn}: {st}").map_err(|_| fmt_err())?;
    }
    let step_name = step_param_name(u.scheme);
    writeln!(out, ", {step_name}: {step_ty}) -> {ret_ty}").map_err(|_| fmt_err())?;
    out.push_str("    decreases l,\n{\n    match l {\n");

    // Base arm(s) + the recursive arm, per scheme.
    for b in &base {
        let value = scheme_base_value(u.scheme, b, seed_param.map(|(n, _)| n));
        writeln!(
            out,
            "        {}::{} => {value},",
            e.name,
            base_variant_pattern(b)
        )
        .map_err(|_| fmt_err())?;
    }
    let rec_arm = scheme_recursive_arm(
        u.scheme,
        &e.name,
        &rec.name,
        &fname,
        step_name,
        seed_param.map(|(n, _)| n),
    );
    writeln!(
        out,
        "        {}::{}(x, tail) => {rec_arm},",
        e.name, rec.name
    )
    .map_err(|_| fmt_err())?;
    out.push_str("    }\n}\n");
    Ok(out)
}

/// The step parameter name in the generated scheme `spec fn` (REQ-6): `f` for the
/// accumulator schemes (`fold`/`traverse`), `g` for `map`, `p` for the predicates
/// (`for_all`/`exists`) — matching the GROUNDED forms.
fn step_param_name(scheme: &thermite_spec::SchemeSig) -> &'static str {
    match scheme.name {
        "fold" | "traverse" => "f",
        "map" => "g",
        _ => "p",
    }
}

/// The base-arm value for a generated scheme `spec fn` (REQ-6): `fold` → the seed
/// `init`; `for_all`/`traverse` → `true`; `exists` → `false`; `map` → the empty
/// ADT (the unit base reconstructed, `E::Nil`).
fn scheme_base_value(
    scheme: &thermite_spec::SchemeSig,
    base: &VariantDef,
    seed: Option<&str>,
) -> String {
    match scheme.name {
        "fold" | "traverse" => seed.unwrap_or("init").to_string(),
        "exists" => "false".to_string(),
        "map" => base.name.clone(),
        // for_all (and any predicate base) is the identity `true`.
        _ => "true".to_string(),
    }
}

/// The recursive-arm body for a generated scheme `spec fn` (REQ-6), applying the
/// step at each `Cons`/`Node`. GROUNDED forms:
/// - `fold`: `f(x, fold_<e>(*tail, init, f))`
/// - `for_all`: `p(x) && for_all_<e>(*tail, p)`
/// - `exists`: `p(x) || exists_<e>(*tail, p)`
/// - `traverse`: `f(x, traverse_<e>(*tail, init, f))`
/// - `map`: `E::Cons(g(x), Box::new(map_<e>(*tail, g)))`
fn scheme_recursive_arm(
    scheme: &thermite_spec::SchemeSig,
    enum_name: &str,
    rec_variant: &str,
    fname: &str,
    step_name: &str,
    seed: Option<&str>,
) -> String {
    let seed = seed.unwrap_or("init");
    match scheme.name {
        "fold" | "traverse" => {
            format!("{step_name}(x, {fname}(*tail, {seed}, {step_name}))")
        }
        "for_all" => format!("{step_name}(x) && {fname}(*tail, {step_name})"),
        "exists" => format!("{step_name}(x) || {fname}(*tail, {step_name})"),
        "map" => format!(
            "{enum_name}::{rec_variant}({step_name}(x), Box::new({fname}(*tail, {step_name})))"
        ),
        // Unreachable for a registered scheme (the 5 are matched above); the
        // identity for safety.
        _ => format!("{fname}(*tail, {step_name})"),
    }
}

/// Emit the `fold_bound_<e>` induction-discharged-once law (REQ-7) — the
/// multiplier. A `proof fn` parametric in the step `f` + a per-node premise,
/// carrying the SINGLE `decreases l` structural induction, proving
/// `fold_<e>(l, init, f) <= <e>_len(l) * b` for `init == 0` and a per-node bound.
/// GROUNDED (`fold_bound_list`, single induction, `9 verified, 0 errors`; the
/// per-node-premise-removed negative control FAILS). Emitted ONLY for an ADT a
/// `fold` folds over.
fn emit_fold_bound_law(u: &SchemeUse) -> Result<String, LowerError> {
    let e = &u.enum_item;
    let elem = &u.elem_ty;
    let foldn = u.scheme.generated_fn_name(&e.name); // fold_<e>
    let lenn = format!("{}_len", e.name.to_ascii_lowercase());
    let lawn = format!("fold_bound_{}", e.name.to_ascii_lowercase());
    let (base, recursive) = enum_variant_split(e);
    let rec = recursive.ok_or_else(|| LowerError::Unsupported {
        what: format!("ADT `{}` has no recursive variant for the fold law", e.name),
        span: zero_span(),
    })?;

    let mut out = String::new();
    writeln!(
        out,
        "proof fn {lawn}(l: {}, init: nat, f: spec_fn({elem}, nat) -> nat, b: nat)",
        e.name
    )
    .map_err(|_| fmt_err())?;
    out.push_str("    requires\n        init == 0,\n");
    writeln!(
        out,
        "        forall|x: {elem}, acc: nat| #[trigger] f(x, acc) <= acc + b,"
    )
    .map_err(|_| fmt_err())?;
    writeln!(out, "    ensures").map_err(|_| fmt_err())?;
    writeln!(out, "        {foldn}(l, init, f) <= {lenn}(l) * b,").map_err(|_| fmt_err())?;
    out.push_str("    decreases l,\n{\n    match l {\n");
    for b in &base {
        writeln!(
            out,
            "        {}::{} => {{}}",
            e.name,
            base_variant_pattern(b)
        )
        .map_err(|_| fmt_err())?;
    }
    writeln!(out, "        {}::{}(x, tail) => {{", e.name, rec.name).map_err(|_| fmt_err())?;
    writeln!(out, "            {lawn}(*tail, init, f, b);").map_err(|_| fmt_err())?;
    writeln!(
        out,
        "            assert(({lenn}(*tail) + 1) * b == {lenn}(*tail) * b + b) by(nonlinear_arith);"
    )
    .map_err(|_| fmt_err())?;
    out.push_str("        }\n    }\n}\n");
    Ok(out)
}

// ---------------------------------------------------------------------------
// REQ-1: signature lowering.
// ---------------------------------------------------------------------------

/// Lower a `spec fn` (REQ-1/REQ-5). Slice params take `Seq<T>` (not `&[T]`); the
/// body lowers in spec context; `dec`→`decreases`. The return type uses the
/// `nat`-typed accumulator form when the body folds slice elements into a sum
/// (OQ-1: `u64`-valued `spec_sum` would re-introduce the overflow obligation).
fn lower_spec_fn(
    s: &SpecFnItem,
    variants: &[(&str, &str)],
    program: &Program,
) -> Result<String, LowerError> {
    // Basis Stage 2 (`.design/basis/02-recursion-schemes.md` REQ-6): the
    // recursion-scheme bindings in scope for THIS spec fn — its scrutinee params
    // resolved to the generated `fold_<e>`/`for_all_<e>`. EMPTY for a non-scheme
    // fn (byte-stable for the existing corpus).
    let scheme_bindings = spec_fn_scheme_bindings(s, program)?;

    let mut out = String::new();
    // The return type: a scheme-CALL fold body returns the scheme's result kind
    // (`nat` for `fold`, `bool` for `for_all`/`exists`/`traverse`, the ADT for
    // `map`); else the existing head/ADT-fold-sum or declared-type lowering.
    let ret = lower_spec_fn_ret_with_schemes(&s.ret, &s.body, &scheme_bindings);
    write!(out, "spec fn {}(", s.name).ok();
    emit_params(&mut out, &s.params, Pos::Spec)?;

    // The DEC NUANCE (`.design/basis/02-recursion-schemes.md` step-lowering
    // resolution): a scheme-CALL instance body (`fold_list(l, 0, f)`) is
    // NON-recursive — the recursion lives in the GENERATED `fold_<e>`, which
    // carries its own `decreases l`. The surface instance still parses with a
    // mandatory `dec l`, but emitting `decreases l` on this non-recursive body is
    // spurious. We SUPPRESS it for a scheme-call body. (A hand-written recursive
    // `spec fn` — the `is_adt_fold_sum`/head-fold path — keeps its `decreases`.)
    if is_scheme_call_body(&s.body, &scheme_bindings) {
        writeln!(out, ") -> {ret}").ok();
    } else {
        write!(
            out,
            ") -> {ret}\n    decreases {}\n",
            spec_dec(&s.dec, &s.params)
        )
        .ok();
    }
    out.push_str(&lower_spec_fn_body_with_schemes(
        &s.body,
        &s.params,
        &ret,
        variants,
        &scheme_bindings,
    )?);
    Ok(out)
}

/// The recursion-scheme bindings in scope for a `spec fn` (REQ-6): for each
/// distinct scheme its body CALLS, the resolved generated fn name + element/result
/// types (from the scrutinee param's `enum` type). EMPTY for a non-scheme fn.
fn spec_fn_scheme_bindings(
    s: &SpecFnItem,
    program: &Program,
) -> Result<Vec<SchemeBinding>, LowerError> {
    let enums: std::collections::BTreeMap<&str, &EnumItem> = program
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Enum(e) => Some((e.name.as_str(), e)),
            _ => None,
        })
        .collect();
    let mut uses: Vec<SchemeUse> = Vec::new();
    collect_scheme_uses_in_block(&s.body, &s.params, &enums, s.span, &mut uses)?;
    Ok(uses
        .into_iter()
        .map(|u| SchemeBinding {
            scheme_name: u.scheme.name,
            gen_name: u.scheme.generated_fn_name(&u.enum_name),
            elem_ty: u.elem_ty,
            result: u.scheme.result,
        })
        .collect())
}

/// True if the spec fn's body tail is a scheme CALL resolved by an in-scope
/// binding (REQ-6) — the instance whose `decreases` is suppressed (the recursion
/// lives in the generated `fold_<e>`).
fn is_scheme_call_body(body: &Block, bindings: &[SchemeBinding]) -> bool {
    scheme_call_result(body, bindings).is_some()
}

/// The result kind of a scheme-CALL body tail (REQ-6), or `None` if the body is
/// not a scheme call. Used to pick the instance's return type + suppress the
/// spurious `decreases`.
fn scheme_call_result(
    body: &Block,
    bindings: &[SchemeBinding],
) -> Option<thermite_spec::SchemeResult> {
    let tail = body.tail.as_ref()?;
    if let Expr::Call { callee, .. } = tail.as_ref() {
        if let Expr::Path(segs) = callee.as_ref() {
            if let Some(name) = segs.last() {
                return bindings
                    .iter()
                    .find(|b| b.scheme_name == name)
                    .map(|b| b.result);
            }
        }
    }
    None
}

/// The return type of a scheme-CALL instance spec fn (REQ-6): `nat` for `fold`,
/// `bool` for the predicate schemes, the ADT element-or-name for `map`. Falls
/// back to the existing `lower_spec_fn_ret` (head/ADT-fold-sum or declared type)
/// when the body is not a scheme call.
fn lower_spec_fn_ret_with_schemes(ret: &Type, body: &Block, bindings: &[SchemeBinding]) -> String {
    use thermite_spec::SchemeResult;
    match scheme_call_result(body, bindings) {
        Some(SchemeResult::Accumulator) => "nat".to_string(),
        Some(SchemeResult::Bool) => "bool".to_string(),
        Some(SchemeResult::SameAdt) => {
            // `map` returns the same ADT; the surface `ret` already names it.
            lower_type(ret).unwrap_or_else(|_| "bool".to_string())
        }
        None => lower_spec_fn_ret(ret, body),
    }
}

/// Lower a spec-fn body with the recursion-scheme bindings in scope (REQ-6) — a
/// scheme call in the body lowers to a call of the generated `fold_<e>`. Delegates
/// to the existing `lower_spec_fn_body` for the non-scheme paths (head-fold-sum,
/// ADT-fold-sum), threading the bindings through the spec context.
fn lower_spec_fn_body_with_schemes(
    body: &Block,
    params: &[Param],
    ret: &str,
    variants: &[(&str, &str)],
    bindings: &[SchemeBinding],
) -> Result<String, LowerError> {
    if bindings.is_empty() {
        return lower_spec_fn_body(body, params, ret, variants);
    }
    // A scheme-call fn body lowers directly in spec context with the scheme
    // bindings (and variants) attached. The head/ADT-fold-sum shape predicates do
    // not match a scheme-call body (its tail is a `Call`, not a `Match`), so the
    // existing special-case lowering is bypassed — the scheme call is handled in
    // `lower_expr`'s `Call` arm via `lower_scheme_call`.
    let ctx = Ctx::spec_seq()
        .with_variants(variants)
        .with_nat_ret(ret == "nat")
        .with_schemes(bindings);
    let mut out = String::from("{\n");
    let b = lower_block_inner(body, ctx, 1, zero_span())?;
    out.push_str(&b);
    out.push_str("}\n");
    Ok(out)
}

/// The slice-typed parameter names of an item (the SHAPE-derived set whose bare
/// paths get `@` in spec position — REQ-5).
fn slice_param_names(params: &[Param]) -> Vec<&str> {
    params
        .iter()
        .filter_map(|p| match &p.ty {
            Type::Ref { inner, .. } if matches!(inner.as_ref(), Type::Slice(_)) => {
                Some(p.name.as_str())
            }
            _ => None,
        })
        .collect()
}

/// The `String`-named values in scope for a fn's contract (REQ-4): every
/// `String`/`&String` parameter, plus the synthetic `result` when the return type
/// is `String`. A contract over a `String` names the wrapper's spec fns
/// (`.spec_len()`/`.spec_byte_at(i as int)`); this is the SHAPE-derived set the
/// `Ctx::is_string` rewrite keys on. A `&String` (the `str`-view role) is a
/// `String` value too — a read-only param's `.len()`/`.byte_at` are the same spec
/// fns. EMPTY for a non-`String` fn (byte-stable for the existing corpus).
fn string_value_names(f: &FnItem) -> Vec<&str> {
    fn ty_is_string(ty: &Type) -> bool {
        match ty {
            Type::String => true,
            Type::Ref { inner, .. } => ty_is_string(inner),
            _ => false,
        }
    }
    let mut names: Vec<&str> = f
        .params
        .iter()
        .filter(|p| ty_is_string(&p.ty))
        .map(|p| p.name.as_str())
        .collect();
    if ty_is_string(&f.ret) {
        names.push("result");
    }
    names
}

/// Lower a `fn` (REQ-1). `-> (result: RET)` binder so `ens` can mention
/// `result`; `req`→`requires`, each `ens`→`ensures`, `fx pure`→nothing.
///
/// THE BOUNDARY/SLAG COMPOSITION ARM (`.design/lower/boundary-composition.md`
/// REQ-1, §9/§8): when `f.boundary.is_some() || f.slag.is_some()` the fn is a
/// DECLARED trust boundary — a `#[boundary]` fn has a FOREIGN body (`body: None`)
/// and a `#[slag]` fn a fiat-trusted body, both body-UNPROVEN by §8/§9. As a
/// woven dependency of a caller's sub-program it is emitted as a
/// `#[verifier::external_body]` SIGNATURE — its `requires`/`ensures` lowered
/// exactly as a regular fn's (no weakening), with the body SUPPRESSED to a
/// synthetic `{ unimplemented!() }` verus never checks — so the caller's proof
/// resolves the callee and discharges against its ASSUMED `ensures`. The
/// exemption is gated STRICTLY on the syntactic `#[boundary]`/`#[slag]` flag
/// (the honesty gate, `goal.md` R-DEFER-9): a REGULAR fn (neither flag) ALWAYS
/// takes the fully-proved-body path below — a lying regular body is CAUGHT.
/// True iff the fn's effect row contains `diverge` (§4.1: "divergence requires
/// `fx diverge` in the row"). Keyed on the SHAPE of the effect row — a `pure`
/// row never diverges; a `Set` row diverges iff it lists [`Effect::Diverge`].
/// This is the SINGLE source of truth for the §4.1 termination exemption (the
/// fn attribute in [`lower_fn`] and the loop-`decreases` suppression in
/// [`lower_loop`] both gate on it), so the exemption is applied uniformly and
/// ONLY to a diverge fn (a non-diverge loop still proves termination).
fn fn_is_diverge(f: &FnItem) -> bool {
    use thermite_syntax::ast::{Effect, EffectRow};
    matches!(&f.contract.fx, EffectRow::Set(es) if es.contains(&Effect::Diverge))
}

fn lower_fn(
    f: &FnItem,
    nat_fns: &[&str],
    inv_structs: &[&str],
    string_fields: &[&str],
    variants: &[(&str, &str)],
) -> Result<String, LowerError> {
    // THE HONESTY GATE: external_body iff a declared trust boundary
    // (`#[boundary]`/`#[slag]`), NEVER a regular fn. Emitted only into a CALLER's
    // sub-program as a woven dependency (forge's `item_subprogram`). The 2-bool
    // decision is DELEGATED to the Verus-verified `should_emit_external_body`
    // (epic #60, REQ-9 / Target C, mechanism (c)): its `ensures` proves the
    // disjunction AND the §9 soundness corollary `(!boundary && !slag) ==> !r` —
    // a regular fn is NEVER laundered into an assumed-L3 external_body signature
    // (`goal.md` R-DEFER-9). `boundary_gate_verified.rs` anchors this OBSERVABLE
    // dispatch (the emitted `#[verifier::external_body]` substring) to the proof.
    if thermite_verified::should_emit_external_body(f.boundary.is_some(), f.slag.is_some()) {
        return lower_external_body_fn(f, nat_fns, inv_structs, string_fields);
    }

    let mut out = String::new();
    // §4.1: "Termination is proved by default; divergence requires `fx diverge`."
    // A `fx diverge` fn (an event loop, `examples/editor/editor.th`'s `run`) is
    // honestly NON-terminating: its loop's `decreases` is suppressed below
    // (`lower_loop`), and Verus would then DEMAND a termination proof for the
    // bare loop unless the fn carries this exemption. The attribute scopes the
    // exemption to THIS fn (it does not weaken termination for any other fn): a
    // diverge fn proves PARTIAL correctness (the loop INVARIANTS) only, which is
    // the honest L1 verdict — termination is not claimed. A non-diverge fn never
    // emits this, so the termination default stands unweakened (gap-3 is
    // diverge-ONLY; a normal loop without `dec` still fails to verify).
    if fn_is_diverge(f) {
        out.push_str("#[verifier::exec_allows_no_decreases_clause]\n");
    }
    out.push_str(&lower_fn_signature(f, nat_fns, inv_structs, string_fields)?);
    // `fx pure` emits no annotation (Verus `fn` is pure by default; §4.1).

    // Body, with shape-derived proof aids threaded through the loop lowering. The
    // variant map flows into the exec body so an enum `match` (e.g. `is_circle`'s)
    // lowers to ENUM-QUALIFIED arms (REQ-9).
    let body = lower_fn_body(f, nat_fns, string_fields, variants)?;
    out.push_str(&body);
    Ok(out)
}

/// Emit a `fn`'s signature up to and including its `requires`/`ensures` block
/// (everything before the body): `fn name(<params>) -> (result: RET)` then
/// `requires <req>,` (omitted when literal-`true`) and each `ens` in source
/// order. Shared by the regular fully-proved arm ([`lower_fn`]) and the
/// boundary/slag external_body arm ([`lower_external_body_fn`]) so the contract
/// lowering is IDENTICAL across both (REQ-1 — the assumed signature carries the
/// exact unweakened contract).
fn lower_fn_signature(
    f: &FnItem,
    nat_fns: &[&str],
    inv_structs: &[&str],
    string_fields: &[&str],
) -> Result<String, LowerError> {
    let mut out = String::new();
    let ret = lower_type(&f.ret)?;
    write!(out, "fn {}(", f.name).ok();
    emit_params(&mut out, &f.params, Pos::Exec)?;
    writeln!(out, ") -> (result: {ret})").ok();

    let slices = slice_param_names(&f.params);
    // Basis Stage 7 (`.design/basis/07-strings.md` REQ-4): the `String`-named
    // values in scope for this fn's contract — every `String`/`&String` param plus
    // `result` when the return is `String`. A `String` receiver's spec-position
    // `.len()`/`.byte_at(i)` rewrites to `.spec_len()`/`.spec_byte_at(i as int)`.
    let strings = string_value_names(f);
    let spec = Ctx::spec(&slices, nat_fns)
        .with_strings(&strings)
        .with_string_fields(string_fields);

    // requires: the single `req` clause (REQ-1), plus the woven `well_formed()`
    // conjunct for every parameter whose type is an invariant-bearing `struct`
    // (REQ-8, OQ-3 automatic threading) so Verus has the type-invariant of each
    // incoming value in scope. The author writes neither conjunct: the invariant
    // is a property of the TYPE, implicit at every use.
    let mut woven_reqs: Vec<String> = Vec::new();
    for p in &f.params {
        if let Type::Named(name) = &p.ty {
            if inv_structs.contains(&name.as_str()) {
                woven_reqs.push(format!("{}.well_formed()", p.name));
            }
        }
    }
    let req = lower_expr(&f.contract.req.expr, spec, 0, f.span)?;
    if woven_reqs.is_empty() {
        // No woven invariant conjunct: keep the existing single-line
        // `requires <req>,` form byte-for-byte (no golden churn for the non-ADT
        // corpus — `sum`/`binary_search` lower UNCHANGED). Omit a literal-`true`.
        if req != "true" {
            writeln!(out, "    requires {req},").ok();
        }
    } else {
        // An invariant-bearing struct param weaves its `well_formed()` conjunct;
        // emit the multi-line `requires` block (the woven conjuncts first, then
        // the author's `req` unless it is literal-`true`).
        out.push_str("    requires\n");
        for r in &woven_reqs {
            writeln!(out, "        {r},").ok();
        }
        if req != "true" {
            writeln!(out, "        {req},").ok();
        }
    }

    // ensures: the woven `result.well_formed()` conjunct FIRST when the return
    // type is an invariant-bearing struct (REQ-8 — Verus proves the constructed
    // return value satisfies the invariant), then every `ens` clause in source
    // order (no weakening — R-DEFER-9).
    out.push_str("    ensures\n");
    if let Type::Named(name) = &f.ret {
        if inv_structs.contains(&name.as_str()) {
            out.push_str("        result.well_formed(),\n");
        }
    }
    for ens in &f.contract.ens {
        let e = lower_expr(&ens.expr, spec, 0, f.span)?;
        writeln!(out, "        {e},").ok();
    }
    Ok(out)
}

/// Lower a `#[boundary]`/`#[slag]` fn as a `#[verifier::external_body]` assumable
/// SIGNATURE (`.design/lower/boundary-composition.md` REQ-1, §9/§8). The verus
/// `#[verifier::external_body]` attribute makes the body OPAQUE: verus assumes
/// the `requires`/`ensures` at every call site and NEVER checks the body
/// (grounded harness (1): a caller proves L3 through the assumed `ensures`). The
/// signature + contract are lowered by the SAME [`lower_fn_signature`] a regular
/// fn uses (no weakening — REQ-1), and the body is a synthetic
/// `{ unimplemented!() }` verus never examines (the foreign/fiat body the caller
/// trusts by declaration; §8/§9).
///
/// This is THE HONEST modeling of a foreign function, not a proof cheat
/// (`goal.md` R-DEFER-9): it is emitted ONLY for a fn ALREADY classified
/// `#[boundary]`/`#[slag]` (the §16/§8 `gate_fn` L1 path) and woven into a
/// CALLER's sub-program — the caller still proves its OWN body and discharges
/// the callee's `req` at the call site (harnesses (2)/(3)). The
/// `#[verifier::external_body]` lives in the lowered verus STRING (a generated
/// artifact describing a foreign function), NEVER in the toolchain's own `.rs`
/// source — categorically distinct from the gate-forbidden `#[verifier::external]`
/// proof-dodge of code we wrote (the doc's emitted-verus vs our-Rust distinction).
fn lower_external_body_fn(
    f: &FnItem,
    nat_fns: &[&str],
    inv_structs: &[&str],
    string_fields: &[&str],
) -> Result<String, LowerError> {
    let mut out = String::new();
    out.push_str("#[verifier::external_body]\n");
    out.push_str(&lower_fn_signature(f, nat_fns, inv_structs, string_fields)?);
    // The body is SUPPRESSED: verus does not check an external_body body, so the
    // synthetic `{ unimplemented!() }` stands in for the foreign/fiat body the
    // caller trusts by declaration (§8/§9). The real `f.body` (None for a
    // boundary fn, a fiat body for slag) is NEVER lowered here — re-lowering a
    // slag body would re-introduce the obligation §8 exempts (OQ-2).
    out.push_str("{\n    unimplemented!()\n}\n");
    Ok(out)
}

/// Emit the comma-separated parameter list. In spec context a slice param is the
/// `Seq` view (REQ-5); in exec context it is the plain `&[T]`.
fn emit_params(out: &mut String, params: &[Param], pos: Pos) -> Result<(), LowerError> {
    for (i, p) in params.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        let ty = if pos == Pos::Spec {
            spec_param_type(&p.ty)?
        } else {
            lower_type(&p.ty)?
        };
        write!(out, "{}: {ty}", p.name).ok();
    }
    Ok(())
}

/// A `spec fn` parameter type: a `&[T]` slice becomes `Seq<T>` (REQ-5 — the
/// naive `&[u32]` form fails `verus` with `the trait bound &[u32]: Integer is
/// not satisfied`). Other types lower normally.
fn spec_param_type(ty: &Type) -> Result<String, LowerError> {
    if let Type::Ref { inner, .. } = ty {
        if let Type::Slice(elem) = inner.as_ref() {
            let e = lower_type(elem)?;
            return Ok(format!("Seq<{e}>"));
        }
    }
    lower_type(ty)
}

/// The return type of a `spec fn`. A slice-folding spec fn (one whose body sums
/// `elem as TY` over the slice — the `spec_sum` shape) returns `nat` so the
/// fold cannot overflow the spec relation (OQ-1). Detected by SHAPE: a `Match`
/// or `if/else` whose recursive arm adds a cast slice head to a recursive call.
fn lower_spec_fn_ret(ret: &Type, body: &Block) -> String {
    if is_head_fold_sum(body) || is_adt_fold_sum(body) {
        return "nat".to_string();
    }
    lower_type(ret).unwrap_or_else(|_| "bool".to_string())
}

/// Detect the GENERAL ADT structural-fold shape (`.design/basis/01-adts.md`
/// REQ-10 + the recorded structural-recursion finding): a `spec fn` over a
/// recursive ADT value whose body `match`es that value and whose arm(s) RECURSE
/// on the dereferenced recursive field — `f(*t)`, the `Box`-deref of REQ-3.
///
/// This is a STRUCTURAL predicate, NOT fitted to a base-arm shape (the #69
/// divergence): it does NOT require a literal-`0` unit base. Both folds detect:
///
/// - `sum_list`/`len` — literal-`0` unit base (`Nil => 0`) + a cons arm
///   `Cons(h, t) => <cast h> + f(*t)`;
/// - `tree_sum` — a VALUE-carrying base (`Leaf(v) => v as u64`) + a
///   binary-recursive arm `Node(l, r) => f(*l) + f(*r)`.
///
/// The single distinguishing signal is the presence of a recursive `f(*x)`
/// call ANYWHERE in some arm body (`expr_has_deref_call_arg`, a full-tree walk),
/// over a `match` of the function's `dec` value. Such a fold is lowered with a
/// `nat` return so EVERY arm's integer arithmetic stays `nat` and the arms
/// type-check uniformly (the GROUNDED form; without the `nat` return a base arm
/// like `v as u64` is `u64` while the recursive arm is `int`, and verus rejects
/// the `match` with `match arms have incompatible types`).
/// True if a spec-fn body is a `fold` SCHEME CALL (`.design/basis/02-recursion-
/// schemes.md` REQ-6): the body tail is an `Expr::Call` whose callee path is the
/// `fold` scheme. Such an instance returns `nat` (the `Accumulator` result), so
/// it joins `nat_fns` exactly as a hand-written ADT-fold-sum does. SHAPE check
/// (the callee path is `fold` and `fold` is a registered scheme), never a name
/// check; only `fold` is the `nat`-result scheme.
fn is_fold_scheme_call_body(body: &Block) -> bool {
    let Some(tail) = &body.tail else { return false };
    let Expr::Call { callee, .. } = tail.as_ref() else {
        return false;
    };
    let Expr::Path(segs) = callee.as_ref() else {
        return false;
    };
    segs.last().map(|s| s.as_str()) == Some("fold")
        && thermite_spec::schemes::lookup("fold").is_some()
}

fn is_adt_fold_sum(body: &Block) -> bool {
    let Some(tail) = &body.tail else { return false };
    let Expr::Match { arms, .. } = tail.as_ref() else {
        return false;
    };
    // GENERAL: a recursive structural fold has at least one arm that recurses
    // through a `Box`-deref'd field (`f(*x)`). The base arm(s) are whatever
    // remains — a literal `0`, a value-carrying `Leaf(v) => v`, etc. — and are
    // coerced to `nat` uniformly with the recursive arm by the `nat` return.
    arms.iter().any(|arm| expr_has_deref_call_arg(&arm.body))
}

/// True if a recursive structural-fold call `f(*x)` (a `Call` with a `Deref`
/// argument, the `*t` of REQ-3's `Box`-deref recursion) appears ANYWHERE in the
/// expression tree of an arm body — not just at its top level. A full-tree walk
/// so `Node(l, r) => f(*l) + f(*r)` (the recursive call nested under an `Add`)
/// and `Cons(h, t) => h as T + f(*t)` (nested under an `Add` rhs) are both
/// detected. SHAPE check, never a name check.
fn expr_has_deref_call_arg(expr: &Expr) -> bool {
    match expr {
        Expr::Call { callee, args } => {
            args.iter().any(|a| matches!(a, Expr::Deref(_)))
                || expr_has_deref_call_arg(callee)
                || args.iter().any(expr_has_deref_call_arg)
        }
        Expr::MethodCall { receiver, args, .. } => {
            expr_has_deref_call_arg(receiver) || args.iter().any(expr_has_deref_call_arg)
        }
        Expr::Field { receiver, .. } => expr_has_deref_call_arg(receiver),
        Expr::Binary { lhs, rhs, .. } => {
            expr_has_deref_call_arg(lhs) || expr_has_deref_call_arg(rhs)
        }
        Expr::Cast { expr, .. } => expr_has_deref_call_arg(expr),
        Expr::Ref { expr, .. } | Expr::Deref(expr) => expr_has_deref_call_arg(expr),
        Expr::Index { base, .. } => expr_has_deref_call_arg(base),
        Expr::Is { scrutinee, .. } => expr_has_deref_call_arg(scrutinee),
        Expr::Closure { body, .. } => expr_has_deref_call_arg(body),
        Expr::Match { scrutinee, arms } => {
            expr_has_deref_call_arg(scrutinee)
                || arms.iter().any(|a| expr_has_deref_call_arg(&a.body))
        }
        Expr::StructLit { fields, .. } => fields.iter().any(|(_, v)| expr_has_deref_call_arg(v)),
        // The prefix `!` (#92): a deref'd recursive call could sit under `!`,
        // so descend into the operand (the honest full-tree walk).
        Expr::Unary { expr, .. } => expr_has_deref_call_arg(expr),
        Expr::IntLit { .. }
        | Expr::BoolLit(_)
        | Expr::Path(_)
        | Expr::StrLit(_)
        | Expr::If { .. } => false,
    }
}

// ---------------------------------------------------------------------------
// REQ-5: spec-fn body lowering — the slice match → Seq recursion.
// ---------------------------------------------------------------------------

/// Detect the head-fold-sum shape (`spec_sum`): a `match xs { [] => 0,
/// [head, ..t] => head as T + f(t) }` — an empty-slice base case of `0` and a
/// cons arm adding the (cast) head to a recursive call on the tail. This is a
/// SHAPE predicate over the AST, not a name check.
fn is_head_fold_sum(body: &Block) -> bool {
    let Some(tail) = &body.tail else { return false };
    let Expr::Match { arms, .. } = tail.as_ref() else {
        return false;
    };
    let mut has_empty_zero = false;
    let mut has_cons_add = false;
    for arm in arms {
        match &arm.pattern {
            Pattern::Slice(pats) if pats.is_empty() => {
                if matches!(&arm.body, Expr::IntLit { value: 0, .. }) {
                    has_empty_zero = true;
                }
            }
            Pattern::Slice(pats) if is_head_rest(pats) => {
                if let Expr::Binary { op: BinOp::Add, .. } = &arm.body {
                    has_cons_add = true;
                }
            }
            _ => {}
        }
    }
    has_empty_zero && has_cons_add
}

/// `[head, ..t]` shape: a binding then a rest.
fn is_head_rest(pats: &[SlicePat]) -> bool {
    matches!(
        pats,
        [SlicePat::Pat(Pattern::Binding(_)), SlicePat::Rest(_)]
    )
}

/// Lower a `spec fn` body. For the head-fold-sum shape, emit the verified `Seq`
/// recursion `if xs.len() == 0 { 0 } else { xs[0] as nat + f(xs.drop_first()) }`
/// (REQ-5). The recursion is reconstructed from the match arms' SHAPE: the base
/// arm's value, the head-element cast, and the recursive callee name.
fn lower_spec_fn_body(
    body: &Block,
    params: &[Param],
    ret: &str,
    variants: &[(&str, &str)],
) -> Result<String, LowerError> {
    if is_head_fold_sum(body) {
        if let Some(slice) = first_slice_param(params) {
            if let Some(tail) = &body.tail {
                if let Expr::Match { arms, .. } = tail.as_ref() {
                    return seq_fold_body(slice, arms, ret);
                }
            }
        }
    }
    // Fallback: lower the block in spec context directly. An ADT fold (`sum_list`,
    // REQ-10) flows through HERE — its `match l { … }` lowers via `lower_match`
    // with the enum-variant map attached (ENUM-QUALIFIED arms) and, when the
    // return is `nat`, with `nat_ret` set so integer casts coerce to `as nat`
    // (the GROUNDED form's `h as nat + sum_list(*t)`).
    let ctx = Ctx::spec_seq()
        .with_variants(variants)
        .with_nat_ret(ret == "nat");
    let mut out = String::from("{\n");
    let b = lower_block_inner(body, ctx, 1, zero_span())?;
    out.push_str(&b);
    out.push_str("}\n");
    Ok(out)
}

/// The name of the first slice (`&[T]`) parameter, used as the `Seq` recursion
/// subject.
fn first_slice_param(params: &[Param]) -> Option<&str> {
    params.iter().find_map(|p| match &p.ty {
        Type::Ref { inner, .. } if matches!(inner.as_ref(), Type::Slice(_)) => {
            Some(p.name.as_str())
        }
        _ => None,
    })
}

/// Build the `Seq` head-fold body from the match arms (REQ-5). `[] => B` becomes
/// `if xs.len() == 0 { B }`; `[head, ..t] => head as T + rec(t)` becomes
/// `else { xs[0] as nat + rec(xs.drop_first()) }`.
fn seq_fold_body(slice: &str, arms: &[MatchArm], ret: &str) -> Result<String, LowerError> {
    let mut base = String::from("0");
    let mut rec_name = String::new();
    let head_cast: String = if ret == "nat" {
        "nat".to_string()
    } else {
        ret.to_string()
    };
    for arm in arms {
        match &arm.pattern {
            Pattern::Slice(pats) if pats.is_empty() => {
                base = lower_expr(&arm.body, Ctx::spec_seq(), 0, zero_span())?;
            }
            Pattern::Slice(pats) if is_head_rest(pats) => {
                // The cons arm is `head as T + rec(t)`: pull the recursive callee.
                if let Expr::Binary { rhs, .. } = &arm.body {
                    if let Expr::Call { callee, .. } = rhs.as_ref() {
                        if let Expr::Path(segs) = callee.as_ref() {
                            if let Some(last) = segs.last() {
                                rec_name = last.clone();
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    if rec_name.is_empty() {
        return Err(LowerError::Unsupported {
            what: "head-fold spec fn without a recursive tail call".to_string(),
            span: zero_span(),
        });
    }
    Ok(format!(
        "{{\n    if {slice}.len() == 0 {{ {base} }} else {{ {slice}[0] as {head_cast} + {rec_name}({slice}.drop_first()) }}\n}}\n"
    ))
}

// ---------------------------------------------------------------------------
// Basis Stage 2 (`.design/basis/02-recursion-schemes.md` REQ-6): scheme-CALL
// lowering — a scheme call → a call of the generated `fold_<e>`.
// ---------------------------------------------------------------------------

/// Lower a recursion-scheme CALL to a CALL of the generated scheme `spec fn`
/// (REQ-6): `fold(l, 0, |x, acc| x + acc)` → `fold_list(l, 0, |x: u64, acc: nat|
/// (x + acc) as nat)`; `for_all(l, |x| x > 0)` → `for_all_list(l, |x: u64| x >
/// 0)`. The scrutinee/seed args lower plainly; the trailing STEP closure is
/// lowered to a TYPED Verus `spec_fn` — element parameter `x: <elem>`, the
/// accumulator parameter `acc: <acc-ty>` for `fold`/`traverse`, and for an
/// accumulator (`fold`) the step body is coerced `as nat` (a `u64`/`nat` mixed
/// body is `int` in spec; the GROUNDED step is `(x + acc) as nat`). The validator
/// (Stage 2b) has already checked the call arity + the flat step.
fn lower_scheme_call(
    binding: &SchemeBinding,
    args: &[Expr],
    ctx: Ctx,
    depth: usize,
    span: Span,
) -> Result<String, LowerError> {
    // The trailing argument is the step closure; everything before it is a
    // scrutinee/seed argument lowered plainly. The validator guaranteed the
    // closure shape, but be defensive (no panic, REQ-9): a missing step is
    // `Unsupported`.
    let Some((step, head_args)) = args.split_last() else {
        return Err(LowerError::Unsupported {
            what: format!(
                "recursion scheme `{}` with no arguments",
                binding.scheme_name
            ),
            span,
        });
    };

    let mut parts: Vec<String> = Vec::with_capacity(args.len());
    for a in head_args {
        parts.push(lower_expr(a, ctx, depth, span)?);
    }

    let Expr::Closure { params, body } = step else {
        return Err(LowerError::Unsupported {
            what: format!(
                "recursion scheme `{}` step must be a closure",
                binding.scheme_name
            ),
            span,
        });
    };

    // Lower the step body in spec context. For an accumulator scheme the body is
    // coerced `as nat` (the GROUNDED `(x + acc) as nat`); for a predicate / map
    // scheme the body stays as written (the closure result type already matches).
    let lowered_body = lower_expr(body, ctx.keep(), depth, span)?;
    let step_src = lower_step_closure(binding, params, &lowered_body, span)?;
    parts.push(step_src);

    Ok(format!("{}({})", binding.gen_name, parts.join(", ")))
}

/// Lower a scheme STEP closure to a typed Verus `spec_fn` literal (REQ-6). The
/// element parameter is typed `<elem>`; an accumulator scheme adds the `acc: nat`
/// (or `acc: bool` for `traverse`) parameter and coerces the body `as nat`. The
/// parameter NAMES are the surface closure's (`x`/`acc`), so the body's path
/// references resolve.
fn lower_step_closure(
    binding: &SchemeBinding,
    params: &[String],
    lowered_body: &str,
    span: Span,
) -> Result<String, LowerError> {
    use thermite_spec::SchemeResult;
    let elem = &binding.elem_ty;
    match binding.result {
        // fold: `|x: <elem>, acc: nat| (<body>) as nat`
        SchemeResult::Accumulator => {
            let (x, acc) = two_params(params, binding, span)?;
            Ok(format!("|{x}: {elem}, {acc}: nat| ({lowered_body}) as nat"))
        }
        // traverse: `|x: <elem>, acc: bool| <body>` (the body is already `bool`)
        SchemeResult::Bool if binding.scheme_name == "traverse" => {
            let (x, acc) = two_params(params, binding, span)?;
            Ok(format!("|{x}: {elem}, {acc}: bool| {lowered_body}"))
        }
        // for_all/exists: `|x: <elem>| <body>` (the body is already `bool`)
        SchemeResult::Bool => {
            let x = one_param(params, binding, span)?;
            Ok(format!("|{x}: {elem}| {lowered_body}"))
        }
        // map: `|x: <elem>| <body>` (the body returns `<elem>`)
        SchemeResult::SameAdt => {
            let x = one_param(params, binding, span)?;
            Ok(format!("|{x}: {elem}| {lowered_body}"))
        }
    }
}

/// The two step-closure parameter names for an accumulator scheme (REQ-6),
/// defensively erroring (no panic) if the validator's shape check was bypassed.
fn two_params<'p>(
    params: &'p [String],
    binding: &SchemeBinding,
    span: Span,
) -> Result<(&'p str, &'p str), LowerError> {
    match params {
        [x, acc] => Ok((x.as_str(), acc.as_str())),
        _ => Err(LowerError::Unsupported {
            what: format!(
                "recursion scheme `{}` step must have 2 parameters (`|x, acc|`)",
                binding.scheme_name
            ),
            span,
        }),
    }
}

/// The single step-closure parameter name for an element scheme (REQ-6).
fn one_param<'p>(
    params: &'p [String],
    binding: &SchemeBinding,
    span: Span,
) -> Result<&'p str, LowerError> {
    match params {
        [x] => Ok(x.as_str()),
        _ => Err(LowerError::Unsupported {
            what: format!(
                "recursion scheme `{}` step must have 1 parameter (`|x|`)",
                binding.scheme_name
            ),
            span,
        }),
    }
}

// ---------------------------------------------------------------------------
// REQ-2: type lowering.
// ---------------------------------------------------------------------------

/// Lower a `Type` to its Verus/Rust spelling (REQ-2). No lifetimes (§4.4).
fn lower_type(ty: &Type) -> Result<String, LowerError> {
    match ty {
        Type::Prim(PrimType::U32) => Ok("u32".to_string()),
        Type::Prim(PrimType::U64) => Ok("u64".to_string()),
        Type::Prim(PrimType::Usize) => Ok("usize".to_string()),
        Type::Prim(PrimType::Bool) => Ok("bool".to_string()),
        Type::Unit => Ok("()".to_string()),
        Type::Ref { mutable, inner } => {
            let i = lower_type(inner)?;
            if *mutable {
                Ok(format!("&mut {i}"))
            } else {
                Ok(format!("&{i}"))
            }
        }
        Type::Slice(inner) => {
            let i = lower_type(inner)?;
            Ok(format!("[{i}]"))
        }
        Type::Generic { name, arg } => {
            let a = lower_type(arg)?;
            Ok(format!("{name}<{a}>"))
        }
        // Basis Stage 1c (`.design/basis/01-adts.md` REQ-1/REQ-2/REQ-10): a
        // user-defined `struct`/`enum` type is its bare name (`Account`, `Shape`,
        // `List`) — the type-side complement of the lowered `Item::Struct`/
        // `Item::Enum`. `Box<T>` is the heap-indirection primitive emitted as a
        // Verus `Box<…>` (the recursive occurrence `Box<List>`, REQ-10), which
        // Verus models natively for a recursive datatype.
        Type::Named(name) => Ok(name.clone()),
        Type::Box(inner) => {
            let i = lower_type(inner)?;
            Ok(format!("Box<{i}>"))
        }
        // Basis Stage 4 (`.design/basis/04-collections.md` REQ-5): a bounded
        // `Vec<T>` lowers to the Thermite-runtime newtype `TVec<elem>` over
        // `vstd::vec::Vec<T>` (the GROUNDED `BVec`-over-`Vec<u64>` form). The
        // wrapper struct + its verified `len`/`spec_get`/`get`/`push` impl are
        // materialized ONCE per element type by `emit_vec_wrappers`; this arm
        // names the type (`Vec<u64>` → `TVecU64`). The wrapper carries the
        // `well_formed` capacity invariant + the no-OOB `get` + capacity-preserving
        // `push`, the §4.2-decidable bounded structure. BACKING-AGNOSTIC SURFACE
        // (#62): the surface contract names `len`/`get`/`push` over `v@`, never
        // `vstd::vec::Vec`; v1 IMPLEMENTS it by wrapping vstd's verified `Vec`.
        Type::Vec(inner) => Ok(tvec_name(inner)?),
        // Basis Stage 7 (`.design/basis/07-strings.md` REQ-2/REQ-4): a bounded
        // `String` lowers to the Thermite-runtime newtype `TString` over
        // `vstd::vec::Vec<u8>` (the GROUNDED `TString`-over-`Vec<u8>` form,
        // `verified, 0 errors`). The wrapper struct + its verified
        // `well_formed`/`len`/`byte_at`/`concat`/`slice` impl are materialized
        // ONCE by `emit_string_wrapper`; this arm names the type. The element
        // type is FIXED to `u8` (the char model is bytes for v1), so — unlike
        // `Type::Vec(elem)` — there is no per-element monomorphization. BACKING-
        // AGNOSTIC SURFACE (#62): the surface contract names `len`/`byte_at` over
        // the byte view `s@`, never `vstd::vec::Vec<u8>`; v1 IMPLEMENTS it by
        // wrapping vstd's verified `Vec<u8>`.
        Type::String => Ok("TString".to_string()),
    }
}

/// The generated wrapper struct name for `Vec<elem>` — `TVec` plus an
/// UpperCamelCase suffix derived from the element type's Verus spelling
/// (`Vec<u64>` → `TVecU64`, `Vec<u32>` → `TVecU32`, `Vec<usize>` → `TVecUsize`)
/// (`.design/basis/04-collections.md` REQ-5). A per-element-type concrete
/// wrapper (not a generic `TVec<T>`) is the GROUNDED form: vstd's `Vec<T>` index
/// `self.data[i]` moves the element out, which requires `T: Copy` — so the
/// verified `get` is monomorphized per (Copy) element type, exactly as the design's
/// GROUNDED `BVec` over `Vec<u64>` is. A non-primitive / nested-collection element
/// is `Unsupported` (the v1 corpus is `Vec<u64>`; a richer element joins when a
/// corpus program needs it — never speculatively, REQ-1 frozen-set discipline).
fn tvec_name(elem: &Type) -> Result<String, LowerError> {
    let suffix = match elem {
        Type::Prim(PrimType::U32) => "U32",
        Type::Prim(PrimType::U64) => "U64",
        Type::Prim(PrimType::Usize) => "Usize",
        Type::Prim(PrimType::Bool) => "Bool",
        other => {
            return Err(LowerError::Unsupported {
                what: format!(
                    "Vec element type {:?} (v1 wraps a Copy primitive element; \
                     the GROUNDED form is Vec<u64>)",
                    lower_type(other).unwrap_or_else(|_| "<unlowerable>".to_string())
                ),
                span: zero_span(),
            });
        }
    };
    Ok(format!("TVec{suffix}"))
}

// ---------------------------------------------------------------------------
// Basis Stage 4 (`.design/basis/04-collections.md` REQ-5): the bounded-`Vec`
// wrapper emission. A Thermite `Vec<T>` lowers to a newtype `TVec<elem>` over
// `vstd::vec::Vec<T>` with the verified `len`/`spec_get`/`get`/`push` impl — the
// GROUNDED `BVec`-over-`Vec<u64>` form. Materialized ONCE per element type, so a
// program using `Vec<u64>` in many fns emits a single `TVecU64`.
// ---------------------------------------------------------------------------

/// The bounded-`Vec` capacity constant `CAP` (`.design/basis/04-collections.md`
/// REQ-5 / the GROUNDED `BVec` `spec const CAP`): the SAME `1_000_000` bound the
/// corpus idiom uses (`conformance/sum.th` `req xs.len() <= 1_000_000`;
/// `conformance/vec_demo.th` `push_one` `req v.len() < 1_000_000`). A `Vec` is
/// bounded by design so the §4.2 cage never sees an unbounded sequence.
const VEC_CAP: u64 = 1_000_000;

/// Collect, in deterministic source order and deduped, the element type of every
/// `Vec<T>` the program references in a `fn`/`spec fn` parameter or return
/// position (REQ-5). The wrapper struct is materialized once per element type.
fn collect_vec_elem_types(program: &Program) -> Vec<Type> {
    let mut elems: Vec<Type> = Vec::new();
    let note = |ty: &Type, elems: &mut Vec<Type>| {
        if let Type::Vec(inner) = ty {
            let e = (**inner).clone();
            if !elems.contains(&e) {
                elems.push(e);
            }
        }
    };
    for item in &program.items {
        let (params, ret) = match item {
            Item::Fn(f) => (&f.params, &f.ret),
            Item::SpecFn(s) => (&s.params, &s.ret),
            Item::Struct(_) | Item::Enum(_) => continue,
        };
        for p in params {
            note(&p.ty, &mut elems);
        }
        note(ret, &mut elems);
    }
    elems
}

/// Emit the `TVec<elem>` wrapper struct + its verified `len`/`spec_get`/`get`/
/// `push` impl for every element type the program uses (REQ-5), in deterministic
/// order. EMPTY when the program uses no `Vec` (byte-stable for the non-Vec
/// corpus). The emitted form is EXACTLY the GROUNDED `BVec` over `vstd::vec::Vec`
/// (`verified, 0 errors`):
///
/// ```verus
/// pub struct TVecU64 { pub data: Vec<u64> }
/// impl TVecU64 {
///     pub open spec fn well_formed(&self) -> bool { self.data.len() <= 1000000 }
///     pub open spec fn len(&self) -> nat { self.data.len() as nat }
///     pub open spec fn spec_get(&self, i: int) -> u64 { self.data@[i] }
///     pub fn get(&self, i: usize) -> (result: u64)
///         requires i < self.data.len(),
///         ensures result == self.data@[i as int],
///     { self.data[i] }
///     pub fn push(&mut self, x: u64)
///         requires old(self).well_formed(), old(self).data.len() < 1000000,
///         ensures
///             final(self).well_formed(),
///             final(self).data.len() == old(self).data.len() + 1,
///             final(self).data@[old(self).data.len() as int] == x,
///     { self.data.push(x) }
/// }
/// ```
///
/// THE `final(self)` FINDING (REQ-5 / the design's recorded migration note): verus
/// 0.2026.05.24 requires `final(self)` (NOT bare `self`) to disambiguate a `&mut`
/// receiver in a `push` postcondition. The `well_formed` capacity invariant + the
/// no-OOB `get` (`req i < len`) + the capacity-preserving `push` (`req len < CAP`)
/// are the Thermite-level additions threaded over vstd's verified `Vec::push`/
/// `Vec::index`/`Vec::len` (which carry the heap proof) — NO `assume`/`external_body`
/// (R-DEFER-9; the broken unguarded forms FAIL verus, the non-vacuity proof).
fn emit_vec_wrappers(program: &Program) -> Result<String, LowerError> {
    let elems = collect_vec_elem_types(program);
    if elems.is_empty() {
        return Ok(String::new());
    }
    let mut out = String::new();
    for elem in &elems {
        let name = tvec_name(elem)?;
        let ety = lower_type(elem)?;
        out.push('\n');
        writeln!(out, "pub struct {name} {{ pub data: Vec<{ety}> }}").ok();
        writeln!(out, "impl {name} {{").ok();
        writeln!(
            out,
            "    pub open spec fn well_formed(&self) -> bool {{ self.data.len() <= {VEC_CAP} }}"
        )
        .ok();
        out.push_str("    pub open spec fn len(&self) -> nat { self.data.len() as nat }\n");
        writeln!(
            out,
            "    pub open spec fn spec_get(&self, i: int) -> {ety} {{ self.data@[i] }}"
        )
        .ok();
        // The no-OOB exec accessor `get` (REQ-5): `req i < len`, `ens result ==
        // v@[i]`. The verified vstd index `self.data[i]`.
        writeln!(out, "    pub fn get(&self, i: usize) -> (result: {ety})").ok();
        out.push_str("        requires i < self.data.len(),\n");
        out.push_str("        ensures result == self.data@[i as int],\n");
        out.push_str("    { self.data[i] }\n");
        // The capacity-preserving exec mutator `push` (REQ-5): `req well_formed &&
        // len < CAP`, `ens final(self).well_formed() && len' == len+1 &&
        // v@[old_len] == x`. The `final(self)` &mut postcondition (the grounding
        // finding). The verified vstd `self.data.push(x)`.
        writeln!(out, "    pub fn push(&mut self, x: {ety})").ok();
        out.push_str("        requires old(self).well_formed(), old(self).data.len() < ");
        writeln!(out, "{VEC_CAP},").ok();
        out.push_str("        ensures\n");
        out.push_str("            final(self).well_formed(),\n");
        out.push_str("            final(self).data.len() == old(self).data.len() + 1,\n");
        out.push_str("            final(self).data@[old(self).data.len() as int] == x,\n");
        out.push_str("    { self.data.push(x) }\n");
        out.push_str("}\n");
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Basis Stage 7 (`.design/basis/07-strings.md` REQ-4): the bounded-`String`
// wrapper emission. A Thermite `String` lowers to a newtype `TString` over
// `vstd::vec::Vec<u8>` with the verified `well_formed`/`len`/`byte_at`/`concat`/
// `slice` impl — the GROUNDED `TString`-over-`Vec<u8>` form. Materialized ONCE
// when the program references `String` (the element type is FIXED to `u8`, so —
// unlike the per-element `Vec` wrapper — there is exactly one `TString`).
// ---------------------------------------------------------------------------

/// True if the `String` type (`Type::String`) is REACHABLE anywhere in `ty` —
/// directly, or nested under a `Ref`/`Slice`/`Vec`/`Box`/`Generic` constructor
/// (a `&String` view, a `Vec<String>`, a `Box<String>`). The whole type-constructor
/// closure is walked so no String-bearing type position is missed (REQ-4).
fn ty_reaches_string(ty: &Type) -> bool {
    match ty {
        Type::String => true,
        Type::Ref { inner, .. }
        | Type::Slice(inner)
        | Type::Vec(inner)
        | Type::Box(inner)
        | Type::Generic { arg: inner, .. } => ty_reaches_string(inner),
        Type::Prim(_) | Type::Unit | Type::Named(_) => false,
    }
}

/// True if the program references the `String` type in ANY reachable type
/// position — a `fn`/`spec fn` parameter or return, a `struct`/`enum`-variant
/// FIELD, or a `fn`-body local `let` annotation — OR uses a string literal
/// anywhere (REQ-4). Every such position needs the `TString` wrapper in scope (a
/// struct field `text: String` lowers to `pub text: TString`; a literal
/// materializes a `TString`). The wrapper is emitted once iff this holds (EMPTY
/// otherwise — byte-stable for the non-`String` corpus).
///
/// CRITICAL for the per-item sub-program weave (forge `#86`): a `forge check`
/// per-item sub-program may be a STRUCT decl alone (`struct Buf { text: String,
/// cursor: u64 }`) whose only `String` reference is a FIELD type — so the struct
/// and enum field arms below are load-bearing, not a `continue`. Mirrors the way
/// `reachable_adt_deps` weaves the struct decls a String-bearing item reaches.
fn program_uses_string(program: &Program) -> bool {
    for item in &program.items {
        match item {
            Item::Fn(f) => {
                if f.params.iter().any(|p| ty_reaches_string(&p.ty)) || ty_reaches_string(&f.ret) {
                    return true;
                }
                if let Some(b) = &f.body {
                    if block_has_str_lit(b) || block_has_string_local(b) {
                        return true;
                    }
                }
            }
            Item::SpecFn(s) => {
                if s.params.iter().any(|p| ty_reaches_string(&p.ty))
                    || ty_reaches_string(&s.ret)
                    || block_has_str_lit(&s.body)
                    || block_has_string_local(&s.body)
                {
                    return true;
                }
            }
            Item::Struct(s) => {
                if s.fields.iter().any(|fd| ty_reaches_string(&fd.ty)) {
                    return true;
                }
            }
            Item::Enum(e) => {
                if e.variants.iter().any(variant_reaches_string) {
                    return true;
                }
            }
        }
    }
    false
}

/// True if any field/payload type of an enum variant reaches `String` (REQ-4).
fn variant_reaches_string(v: &thermite_syntax::ast::VariantDef) -> bool {
    match &v.shape {
        thermite_syntax::ast::VariantShape::Unit => false,
        thermite_syntax::ast::VariantShape::Tuple(tys) => tys.iter().any(ty_reaches_string),
        thermite_syntax::ast::VariantShape::Struct(fields) => {
            fields.iter().any(|fd| ty_reaches_string(&fd.ty))
        }
    }
}

/// True if a block contains a `let` whose type annotation reaches `String`
/// (REQ-4) — a `let s: String = …` local needs the `TString` wrapper even when no
/// param/return/field is typed `String`. Walks nested `if`/`loop` blocks.
fn block_has_string_local(block: &Block) -> bool {
    block.stmts.iter().any(stmt_has_string_local)
}

fn stmt_has_string_local(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Let { ty, .. } => ty.as_ref().map(ty_reaches_string).unwrap_or(false),
        Stmt::If { then, else_, .. } => {
            block_has_string_local(then)
                || else_.as_ref().map(block_has_string_local).unwrap_or(false)
        }
        Stmt::Loop(l) => block_has_string_local(&l.body),
        Stmt::Assign { .. } | Stmt::Return(_) | Stmt::Expr(_) => false,
    }
}

/// True if a block contains a string-literal expression anywhere (REQ-1) — a
/// literal materializes a `TString`, so the wrapper must be emitted even when no
/// parameter/return is typed `String` (e.g. `literal_len()`'s `"hello".len()`).
fn block_has_str_lit(block: &Block) -> bool {
    block.stmts.iter().any(stmt_has_str_lit)
        || block.tail.as_deref().map(expr_has_str_lit).unwrap_or(false)
}

fn stmt_has_str_lit(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Let { init, .. } => expr_has_str_lit(init),
        Stmt::Assign { target, value } => expr_has_str_lit(target) || expr_has_str_lit(value),
        Stmt::Return(opt) => opt.as_ref().map(expr_has_str_lit).unwrap_or(false),
        Stmt::If {
            cond, then, else_, ..
        } => {
            expr_has_str_lit(cond)
                || block_has_str_lit(then)
                || else_.as_ref().map(block_has_str_lit).unwrap_or(false)
        }
        Stmt::Loop(l) => block_has_str_lit(&l.body),
        Stmt::Expr(e) => expr_has_str_lit(e),
    }
}

/// True if a string literal appears anywhere in `expr` (a full-tree walk reusing
/// `each_subexpr`'s structural cases). REQ-1.
fn expr_has_str_lit(expr: &Expr) -> bool {
    if matches!(expr, Expr::StrLit(_)) {
        return true;
    }
    let mut found = false;
    let _ = each_subexpr(expr, &mut |e| {
        if expr_has_str_lit(e) {
            found = true;
        }
        Ok(())
    });
    found
}

/// Emit the `TString` wrapper struct + its verified `well_formed`/`spec_len`/
/// `len`/`spec_byte_at`/`byte_at`/`concat`/`slice` impl when the program uses
/// `String` (REQ-4), in deterministic order. EMPTY otherwise. The emitted form is
/// EXACTLY the GROUNDED `TString` over `vstd::vec::Vec<u8>` (`verified, 0
/// errors`):
///
/// ```verus
/// pub struct TString { pub data: Vec<u8> }
/// impl TString {
///     pub open spec fn well_formed(&self) -> bool { self.data.len() <= 1000000 }
///     pub open spec fn spec_len(&self) -> nat { self.data.len() as nat }
///     pub fn len(&self) -> (result: u64) ensures result == self.data.len(),
///         { self.data.len() as u64 }
///     pub open spec fn spec_byte_at(&self, i: int) -> u64 { self.data@[i] as u64 }
///     pub fn byte_at(&self, i: usize) -> (result: u64)
///         requires i < self.data.len(), ensures result == self.data@[i as int],
///         { self.data[i] as u64 }
///     pub fn concat(&self, b: TString) -> (result: TString) { … two-loop append … }
///     pub fn slice(&self, lo: usize, hi: usize) -> (result: TString) { … bounded copy … }
/// }
/// ```
///
/// THE BYTE CHAR MODEL (REQ-2): `byte_at` returns `u64` (the corpus oracle's
/// `first_byte -> u64` shape; a byte zero-extends into `u64`); the exec `len`
/// returns `u64` (the corpus `greeting_len -> u64`) while the SPEC fn is `spec_len`
/// (a contract names `spec_len`, the exec `len` cannot be named in a contract). The
/// no-OOB `byte_at` (`req i < self.data.len()`) is the editor's core safety — the
/// unguarded form FAILS verus (`0 verified, 1 errors`, the L0 demonstration,
/// R-DEFER-9). `concat`/`slice` carry the bounded length identity; `slice` requires
/// `self.well_formed()` so the copied run stays `<= CAP`. NO `assume`/`external_body`
/// (R-DEFER-9) — every Thermite-level contract is threaded over vstd's verified
/// `Vec<u8>::push`/`index`/`len` (which carry the heap proof).
fn emit_string_wrapper(program: &Program) -> Result<String, LowerError> {
    if !program_uses_string(program) {
        return Ok(String::new());
    }
    let cap = VEC_CAP;
    let mut out = String::new();
    out.push('\n');
    out.push_str("pub struct TString { pub data: Vec<u8> }\n");
    out.push_str("impl TString {\n");
    writeln!(
        out,
        "    pub open spec fn well_formed(&self) -> bool {{ self.data.len() <= {cap} }}"
    )
    .ok();
    out.push_str("    pub open spec fn spec_len(&self) -> nat { self.data.len() as nat }\n");
    out.push_str("    pub fn len(&self) -> (result: u64)\n");
    out.push_str("        ensures result == self.data.len(),\n");
    out.push_str("    { self.data.len() as u64 }\n");
    out.push_str(
        "    pub open spec fn spec_byte_at(&self, i: int) -> u64 { self.data@[i] as u64 }\n",
    );
    // The no-OOB exec accessor `byte_at` (REQ-4): `req i < len`, `ens result ==
    // self.data@[i as int]`. The verified vstd index `self.data[i]` zero-extended
    // to `u64` (the corpus `first_byte -> u64`).
    out.push_str("    pub fn byte_at(&self, i: usize) -> (result: u64)\n");
    out.push_str("        requires i < self.data.len(),\n");
    out.push_str("        ensures result == self.data@[i as int],\n");
    out.push_str("    { self.data[i] as u64 }\n");
    // The bounded constructing `concat` (REQ-4): a two-loop append, `req
    // self.well_formed() && b.well_formed() && len_a + len_b <= CAP`, `ens
    // result.well_formed() && result.len() == len_a + len_b`. `b` is by value to
    // match the corpus `a.concat(b)` (no `&` insertion needed). An owned-value
    // construction — no `&mut`/`final(self)` (the result is a fresh value).
    out.push_str("    pub fn concat(&self, b: TString) -> (result: TString)\n");
    out.push_str("        requires self.well_formed(), b.well_formed(),\n");
    writeln!(
        out,
        "                 self.data.len() + b.data.len() <= {cap},"
    )
    .ok();
    out.push_str("        ensures result.well_formed(),\n");
    out.push_str("                result.data.len() == self.data.len() + b.data.len(),\n");
    out.push_str("    {\n");
    out.push_str("        let mut out: Vec<u8> = Vec::new();\n");
    out.push_str("        let mut i: usize = 0;\n");
    out.push_str("        while i < self.data.len()\n");
    out.push_str("            invariant i <= self.data.len(), out.len() == i,\n");
    writeln!(
        out,
        "                      self.data.len() + b.data.len() <= {cap},"
    )
    .ok();
    out.push_str("            decreases self.data.len() - i,\n");
    out.push_str("        { out.push(self.data[i]); i = i + 1; }\n");
    out.push_str("        let mut j: usize = 0;\n");
    out.push_str("        while j < b.data.len()\n");
    out.push_str("            invariant j <= b.data.len(), out.len() == self.data.len() + j,\n");
    writeln!(
        out,
        "                      self.data.len() + b.data.len() <= {cap},"
    )
    .ok();
    out.push_str("            decreases b.data.len() - j,\n");
    out.push_str("        { out.push(b.data[j]); j = j + 1; }\n");
    out.push_str("        TString { data: out }\n");
    out.push_str("    }\n");
    // The bounded substring `slice` (REQ-4): a bounded copy, `req self.well_formed()
    // && lo <= hi && hi <= len`, `ens result.well_formed() && result.len() == hi -
    // lo`. The owned-copy form (OQ-4 RESOLVED — owned, not a borrowed view, so no
    // region/lifetime reasoning §4.4 defers). `self.well_formed()` keeps the copied
    // run <= CAP (the invariant carries the CAP bound).
    out.push_str("    pub fn slice(&self, lo: usize, hi: usize) -> (result: TString)\n");
    out.push_str("        requires self.well_formed(), lo <= hi, hi <= self.data.len(),\n");
    out.push_str("        ensures result.well_formed(), result.data.len() == hi - lo,\n");
    out.push_str("    {\n");
    out.push_str("        let mut out: Vec<u8> = Vec::new();\n");
    out.push_str("        let mut i: usize = lo;\n");
    out.push_str("        while i < hi\n");
    writeln!(
        out,
        "            invariant lo <= i, i <= hi, hi <= self.data.len(), self.data.len() <= {cap}, out.len() == i - lo,"
    )
    .ok();
    out.push_str("            decreases hi - i,\n");
    out.push_str("        { out.push(self.data[i]); i = i + 1; }\n");
    out.push_str("        TString { data: out }\n");
    out.push_str("    }\n");
    out.push_str("}\n");
    Ok(out)
}

// ---------------------------------------------------------------------------
// REQ-3/REQ-5: expression lowering (exec vs spec).
// ---------------------------------------------------------------------------

/// Lower an `Expr` in the given context (REQ-3 exec / REQ-5 spec). `depth`
/// bounds recursion (REQ-9). `span` is the nearest enclosing item span for error
/// loci.
fn lower_expr(expr: &Expr, ctx: Ctx, depth: usize, span: Span) -> Result<String, LowerError> {
    if depth >= MAX_EMIT_DEPTH {
        return Err(LowerError::TooDeep {
            limit: MAX_EMIT_DEPTH,
            span,
        });
    }
    let d = depth + 1;
    match expr {
        // Emit the numeric `value`, NOT `raw` (#37): the lowered output stays
        // byte-identical (`1_000_000` lowers to `1000000`); no golden churn.
        Expr::IntLit { value, .. } => Ok(value.to_string()),
        Expr::BoolLit(b) => Ok(b.to_string()),
        // Basis Stage 7 (`.design/basis/07-strings.md` REQ-1/REQ-4): a string
        // literal `"hello"` materializes into an owned `TString` whose bytes are
        // the literal's UTF-8, constructed by pushing each byte — the GROUNDED
        // `lit_hello` form (`{ let mut data = Vec::new(); data.push(104u8); …
        // TString { data } }`, `verified, 0 errors`). Emitted as an INLINE block
        // expression so it composes as a receiver (`"hello".len()` →
        // `({ … TString { data } }).len()`). It is a CONSTRUCTING op (it
        // allocates), so the enclosing fn carries `fx alloc` (the Stage-1
        // `Effect::Alloc`, accepted by effect-subsumption — `push` is an
        // intrinsic, no declared callee to subsume). The byte sequence is the
        // literal's UTF-8 (`str::as_bytes`), so a multi-byte codepoint pushes each
        // of its bytes (v1 indexes bytes, REQ-2 char model).
        Expr::StrLit(s) => {
            let mut block = String::from("({ let mut data: Vec<u8> = Vec::new();");
            for b in s.as_bytes() {
                write!(block, " data.push({b}u8);").ok();
            }
            block.push_str(" TString { data } })");
            Ok(block)
        }
        Expr::Path(segs) => {
            // A plain path emits its segments joined by `::`. The slice→`xs@`
            // view (REQ-5) is applied at the point of USE (a spec-fn / combinator
            // argument position — `lower_spec_arg`), NOT here, because an `Index`
            // base must stay bare (`lower_index` appends the `@`) to avoid `xs@@`.
            Ok(segs.join("::"))
        }
        Expr::Call { callee, args } => {
            // Basis Stage 2 (`.design/basis/02-recursion-schemes.md` REQ-6): a
            // scheme CALL `fold(l, 0, |x, acc| …)` lowers to a CALL of the
            // generated `fold_<e>` with the step closure lowered to a typed Verus
            // `spec_fn`. Resolved through the in-scope scheme bindings (the
            // current spec fn's `with_schemes`); a non-scheme call falls through.
            if let Expr::Path(segs) = callee.as_ref() {
                if let Some(name) = segs.last() {
                    if let Some(binding) = ctx.scheme_binding(name) {
                        return lower_scheme_call(binding, args, ctx, d, span);
                    }
                }
            }
            let c = lower_expr(callee, ctx, d, span)?;
            // In spec position, a bare slice-param argument to a spec fn or a
            // combinator is passed as its `Seq` view `xs@` (REQ-5). Keyed on the
            // in-scope slice-param SHAPE set (`ctx.is_slice`), not on names. A
            // combinator `Index`-kind argument (per the registry `arg_kinds`)
            // that is a bare `usize` var is cast `as int` (the registry spec-fn
            // param is `int`) — keyed on the registry kind, not on the name.
            let arg_kinds = combinator_arg_kinds(callee);
            let mut parts = Vec::new();
            for (i, a) in args.iter().enumerate() {
                let is_index = arg_kinds
                    .map(|ks| ks.get(i).copied() == Some(thermite_spec::ArgKind::Index))
                    .unwrap_or(false);
                if is_index && ctx.is_spec() {
                    parts.push(lower_index_arg(a, ctx, d, span)?);
                } else {
                    parts.push(lower_spec_arg(a, ctx, d, span)?);
                }
            }
            Ok(format!("{c}({})", parts.join(", ")))
        }
        Expr::MethodCall {
            receiver,
            name,
            args,
        } => {
            // The receiver lowers plainly: a slice `.len()` in spec position is
            // accepted by Verus on the slice (`haystack.len()`), as the golden
            // references confirm; the `@` view is only needed where a `Seq`
            // operation (`subrange`/index) is required (handled in `lower_index`).
            let r = lower_expr(receiver, ctx, d, span)?;
            // Basis Stage 4 (`.design/basis/04-collections.md` REQ-5): in SPEC
            // position the bounded-`Vec` accessor `v.get(i)` (a contract naming the
            // accessed element, `ens result == v.get(i)`) lowers to the wrapper's
            // SPEC accessor `v.spec_get(i as int)` — the exec `get` returns `T` but
            // a contract needs the spec function (`self.data@[i]`), and a Verus spec
            // index is `int`. `v.len()` in spec position lowers to the wrapper's
            // `spec fn len(&self) -> nat` unchanged (`r.len()`). Keyed on the method
            // NAME `get` in spec position only; exec `get`/`push`/`len` (a fn body)
            // lower verbatim to the verified vstd-backed exec methods. The index
            // cast `as int` is appended exactly as `lower_index_arg` does for a
            // combinator index, avoiding a double-cast on an already-`as int` arg.
            if ctx.is_spec() && name == "get" && args.len() == 1 {
                let idx = lower_index_arg(&args[0], ctx, d, span)?;
                return Ok(format!("{r}.spec_get({idx})"));
            }
            // Basis Stage 7 (`.design/basis/07-strings.md` REQ-4): in SPEC position
            // a `String` receiver's `.len()` / `.byte_at(i)` lowers to the wrapper's
            // SPEC fns `.spec_len()` / `.spec_byte_at(i as int)` — the exec `len`/
            // `byte_at` return `u64` and cannot be NAMED in a contract (a contract
            // needs the spec function), and a Verus spec index is `int`. Keyed on
            // the receiver being a `String`-named bare path (`ctx.is_string`) so a
            // `Vec` receiver's `.len()` (whose wrapper spec fn IS `len`) is
            // UNCHANGED — the rewrite is `String`-specific. A `String` `result`
            // (a `String`-returning fn) is in the set too, so `result.len()` in an
            // `ens` rewrites the same way. The receiver path lowered to `r`.
            // The receiver is a `String` either as a bare value PATH (`s`,
            // `result` — `ctx.is_string`) or as a struct FIELD access (`b.text`,
            // `result.text` — `Expr::Field` whose `name` is a `String` field,
            // `ctx.is_string_field`). The editor core `ens result.text.len() ==
            // t.len()` / `b.text.len()` exercises the field form; the corpus
            // `greeting_len` the bare form. Both rewrite `.len()`/`.byte_at(i)` the
            // SAME way (the spec accessors); only the receiver-classification
            // differs, so the whole `String`-receiver class is covered (no field
            // sibling left for a critic to re-pin).
            let recv_is_string = ctx.is_spec()
                && match receiver.as_ref() {
                    Expr::Path(segs) => segs.len() == 1 && ctx.is_string(&segs[0]),
                    Expr::Field { name, .. } => ctx.is_string_field(name),
                    _ => false,
                };
            if recv_is_string {
                if name == "len" && args.is_empty() {
                    return Ok(format!("{r}.spec_len()"));
                }
                if name == "byte_at" && args.len() == 1 {
                    // The spec accessor `spec_byte_at(i: int)` takes an `int` index.
                    // An integer LITERAL (`s.byte_at(0)` — the corpus `first_byte`
                    // ens) flows into the `int` parameter directly (Verus coerces a
                    // literal), so it is emitted plainly, matching the GROUNDED
                    // golden `tests/golden/lower/string_demo.verus.rs`
                    // (`s.spec_byte_at(0)`, `11 verified, 0 errors`). A non-literal
                    // index (a `usize`-typed variable) needs the explicit `as int`
                    // cast Verus requires (no implicit `usize`->`int` in spec
                    // position), so it goes through `lower_index_arg`.
                    let idx = if matches!(&args[0], Expr::IntLit { .. }) {
                        lower_expr(&args[0], ctx, d, span)?
                    } else {
                        lower_index_arg(&args[0], ctx, d, span)?
                    };
                    return Ok(format!("{r}.spec_byte_at({idx})"));
                }
            }
            // Basis Stage 7 (`.design/basis/07-strings.md` REQ-4): in EXEC position
            // the `String` wrapper's index accessors `byte_at(i: usize)` and
            // `slice(lo: usize, hi: usize)` take `usize` parameters (the `vstd::vec::Vec`
            // index type), but a Thermite surface index is commonly a `u64` (the
            // `Buf { cursor: u64 }` editor core, `s.slice(0, k)` with `k: u64`).
            // Verus performs NO implicit `u64 -> usize` narrowing, so each index
            // argument is coerced with an explicit `as usize` cast — the same
            // intrinsic-index coercion `byte_at`'s `usize` accessor needs, applied
            // uniformly across BOTH string index intrinsics so the whole op family
            // (no single triggering site left for a sibling to re-pin). Keyed on the
            // reserved built-in method NAME (`byte_at`/`slice` — there are no
            // user-defined methods in v0.1, so no misfire) and only on the positional
            // index args (`concat`'s by-value `TString` arg is NOT an index). An
            // integer LITERAL (`s.byte_at(0)`, `s.slice(0, ..)`) flows into the
            // `usize` parameter directly (Verus coerces a literal — the GROUNDED
            // golden `string_demo.verus.rs` `{ s.byte_at(0) }` stays byte-identical),
            // and an argument already written `as usize` is left as-is (no
            // double-cast); only a non-literal `u64`/`u32` index needs the explicit
            // narrowing. EXEC-only — the spec-position `.byte_at`/`.spec_*` rewrite
            // above handles a contract index.
            let coerce_usize = !ctx.is_spec() && matches!(name.as_str(), "byte_at" | "slice");
            let mut parts = Vec::new();
            for a in args {
                let lowered = lower_expr(a, ctx, d, span)?;
                if coerce_usize && !matches!(a, Expr::IntLit { .. }) && !is_usize_cast(a) {
                    parts.push(format!("{lowered} as usize"));
                } else {
                    parts.push(lowered);
                }
            }
            Ok(format!("{r}.{name}({})", parts.join(", ")))
        }
        Expr::Field { receiver, name } => {
            let r = lower_expr(receiver, ctx, d, span)?;
            Ok(format!("{r}.{name}"))
        }
        Expr::Closure { params, body } => {
            // Verus `spec_fn` literal `|x: u32| <body>` (REQ-6). The corpus
            // closures are all `u32`-typed slice-element predicates.
            let b = lower_expr(body, ctx.keep(), d, span)?;
            let ps: Vec<String> = params.iter().map(|p| format!("{p}: u32")).collect();
            Ok(format!("|{}| {b}", ps.join(", ")))
        }
        Expr::Match { scrutinee, arms } => lower_match(scrutinee, arms, ctx, d, span),
        Expr::If { cond, then, else_ } => {
            let c = lower_expr(cond, ctx, d, span)?;
            let t = lower_block_inner(then, ctx, d, span)?;
            let e = lower_block_inner(else_, ctx, d, span)?;
            Ok(format!("if {c} {{ {} }} else {{ {} }}", t.trim(), e.trim()))
        }
        Expr::Binary { op, lhs, rhs } => {
            // OQ-1 nat/u64 coercion: an `Eq` where one side calls a `nat`-typed
            // spec fn forces an `as nat` cast on the other (a `u64`-valued
            // scalar) side, since `nat != u64` in Verus. Keyed on the SHAPE
            // (a call to a known nat-spec-fn), not on names. Only in spec
            // position and only when the scalar side is not already a cast.
            if *op == BinOp::Eq && ctx.is_spec() {
                if let Some(s) = lower_nat_equality(lhs, rhs, ctx, d, span)? {
                    return Ok(s);
                }
            }
            // Precedence-preserving parenthesization: a child binary of strictly
            // lower precedence is wrapped (so `lo + (hi - lo) / 2` survives the
            // round-trip rather than degrading to `lo + hi - lo / 2`). The AST
            // already encodes grouping in its nesting; we only add the parens.
            let l = lower_binary_operand(lhs, *op, true, ctx, d, span)?;
            let r = lower_binary_operand(rhs, *op, false, ctx, d, span)?;
            Ok(format!("{l} {} {r}", binop(*op)))
        }
        Expr::Unary { op, expr: inner } => {
            // The prefix `!` (#92, ast.md REQ-10): Verus's `!` is TYPE-DIRECTED —
            // logical-not on `bool`, bitwise-not on an integer — so the lowering
            // emits the bare `!` and Verus resolves the meaning from the operand
            // type (ast.md OQ-4; the GROUNDED `!flag`/`!bits` both certify). The
            // operand is parenthesized when it is itself a binary (or another
            // unary) so the prefix binds only the operand: `!(a & b)` for a
            // grouped binary inner, never `!a & b`. A bare path/literal/call needs
            // no parens.
            let UnaryOp::Not = op;
            let inner_src = lower_expr(inner, ctx, d, span)?;
            let needs_parens = matches!(inner.as_ref(), Expr::Binary { .. });
            if needs_parens {
                Ok(format!("!({inner_src})"))
            } else {
                Ok(format!("!{inner_src}"))
            }
        }
        Expr::Index { base, index } => lower_index(base, index, ctx, d, span),
        Expr::Cast { expr, ty } => {
            let e = lower_expr(expr, ctx, d, span)?;
            // REQ-10: inside a `nat`-returning ADT-fold spec fn body, an integer
            // cast (`h as u64`) coerces to `as nat` so the fold's arithmetic stays
            // `nat` (the GROUNDED `sum_list` form `h as nat + sum_list(*t)`; a
            // `u64`-typed arm body is `int` in spec and verus rejects the match).
            // Keyed on the SHAPE (nat-ret spec body + an integer-target cast),
            // never a name. A `bool`/`()` cast is left as written.
            let t = if ctx.nat_ret && is_int_type(ty) {
                "nat".to_string()
            } else {
                lower_type(ty)?
            };
            Ok(format!("{e} as {t}"))
        }
        Expr::Ref { mutable, expr } => {
            // In spec position `&xs[..i]` becomes `xs@.subrange(..)` (handled in
            // lower_index when the inner is an Index); a bare `&e` keeps the `&`.
            if ctx.is_spec() {
                if let Expr::Index { base, index } = expr.as_ref() {
                    return lower_index(base, index, ctx.keep(), d, span);
                }
            }
            let e = lower_expr(expr, ctx, d, span)?;
            if *mutable {
                Ok(format!("&mut {e}"))
            } else {
                Ok(format!("&{e}"))
            }
        }
        // Basis Stage 1c (`.design/basis/01-adts.md` REQ-8/REQ-9/REQ-10).
        Expr::StructLit { path, fields } => {
            // A struct / struct-variant construction `Path { field: val, … }`
            // (REQ-2/REQ-8): the struct name (or ENUM-QUALIFIED variant) followed
            // by `field: <value>` initializers in source order. The GROUNDED form
            // `Account { balance: a.balance + amount }`. A struct path stays as
            // written; a single-segment user enum VARIANT is qualified.
            let head = qualify_variant_path(path, ctx);
            let mut parts = Vec::with_capacity(fields.len());
            for (name, value) in fields {
                let v = lower_expr(value, ctx, d, span)?;
                parts.push(format!("{name}: {v}"));
            }
            Ok(format!("{head} {{ {} }}", parts.join(", ")))
        }
        Expr::Is { scrutinee, variant } => {
            // A variant-discrimination test `SCRUTINEE is Variant` (REQ-6/REQ-9):
            // Verus-native variant discrimination `<scrutinee> is <Variant>` (the
            // GROUNDED `s is Circle`). The variant name is emitted UNQUALIFIED —
            // Verus's `is` operator takes the bare variant identifier (the
            // scrutinee's type fixes the enum), confirmed by the grounding.
            let s = lower_expr(scrutinee, ctx, d, span)?;
            let v = variant.last().cloned().unwrap_or_default();
            Ok(format!("({s} is {v})"))
        }
        Expr::Deref(inner) => {
            // A `Box` dereference `*EXPR` (REQ-3/REQ-10): the recursive-occurrence
            // read `*tail` Verus follows transparently through the `Box`. Lowers to
            // `*<inner>` in both contexts (the GROUNDED `sum_list(*t)`).
            let e = lower_expr(inner, ctx, d, span)?;
            Ok(format!("*{e}"))
        }
    }
}

/// True if `ty` is an integer primitive (`u32`/`u64`/`usize`) — the cast targets
/// that coerce to `nat` inside a `nat`-returning ADT-fold spec fn (REQ-10). A
/// `bool`/`()`/reference/slice/user type is NOT coerced.
fn is_int_type(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Prim(PrimType::U32) | Type::Prim(PrimType::U64) | Type::Prim(PrimType::Usize)
    )
}

/// True if `expr` is already an `as usize` cast — so the Stage-7 string index
/// coercion (`.design/basis/07-strings.md` REQ-4, the `byte_at`/`slice` `usize`
/// accessors) does NOT double-cast an argument the source already wrote as
/// `... as usize`.
fn is_usize_cast(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Cast {
            ty: Type::Prim(PrimType::Usize),
            ..
        }
    )
}

/// Lower a spec-position call/combinator argument (REQ-5). A bare slice-param
/// path `xs` is passed as its `Seq` view `xs@`; everything else lowers normally.
/// Keyed on the in-scope slice SHAPE set, not on names.
fn lower_spec_arg(arg: &Expr, ctx: Ctx, depth: usize, span: Span) -> Result<String, LowerError> {
    if ctx.is_spec() {
        if let Expr::Path(segs) = arg {
            if let Some(name) = segs.last() {
                if segs.len() == 1 && ctx.is_slice(name) {
                    return Ok(format!("{name}@"));
                }
            }
        }
    }
    lower_expr(arg, ctx, depth, span)
}

/// The registry `arg_kinds` of a call whose callee path names a combinator, or
/// `None` if the callee is not a combinator. Used to apply `as int` to
/// `Index`-kind arguments in spec position (REQ-5/REQ-6).
fn combinator_arg_kinds(callee: &Expr) -> Option<&'static [thermite_spec::ArgKind]> {
    if let Expr::Path(segs) = callee {
        if let Some(name) = segs.last() {
            return thermite_spec::lookup(name).map(|sig| sig.arg_kinds);
        }
    }
    None
}

/// Lower a combinator `Index`-kind argument in spec position: a bare `usize`
/// path is cast `as int` (the registry spec fn takes `int`). A compound index
/// expression lowers normally then is cast. Keyed on the registry kind.
fn lower_index_arg(arg: &Expr, ctx: Ctx, depth: usize, span: Span) -> Result<String, LowerError> {
    let lowered = lower_expr(arg, ctx, depth, span)?;
    // Avoid double-casting if the surface already wrote `as int`.
    if lowered.ends_with("as int") {
        Ok(lowered)
    } else {
        Ok(format!("{lowered} as int"))
    }
}

/// OQ-1 `nat`/`u64` coercion for an `Eq`: if one operand is a call to a
/// `nat`-returning spec fn (`ctx.is_nat_fn`) and the other is a `u64`-valued
/// scalar (a plain path like `acc`/`result`), emit `<scalar> as nat == <call>`.
/// Returns `None` when neither side is a nat-spec-fn call (so the caller falls
/// back to the plain binary lowering). Keyed on SHAPE.
fn lower_nat_equality(
    lhs: &Expr,
    rhs: &Expr,
    ctx: Ctx,
    depth: usize,
    span: Span,
) -> Result<Option<String>, LowerError> {
    let lhs_nat = is_nat_fn_call(lhs, ctx);
    let rhs_nat = is_nat_fn_call(rhs, ctx);
    // Exactly one side is a nat-spec-fn call: coerce the OTHER (scalar) side.
    let (scalar, call) = match (lhs_nat, rhs_nat) {
        (false, true) => (lhs, rhs),
        (true, false) => (rhs, lhs),
        _ => return Ok(None),
    };
    // Only coerce a bare scalar path (`acc`, `result`); leave compound exprs.
    if let Expr::Path(_) = scalar {
        let s = lower_expr(scalar, ctx, depth, span)?;
        let c = lower_expr(call, ctx, depth, span)?;
        return Ok(Some(format!("{s} as nat == {c}")));
    }
    Ok(None)
}

/// True if `expr` is a direct call to a `nat`-returning spec fn (SHAPE check).
fn is_nat_fn_call(expr: &Expr, ctx: Ctx) -> bool {
    if let Expr::Call { callee, .. } = expr {
        if let Expr::Path(segs) = callee.as_ref() {
            if let Some(name) = segs.last() {
                return ctx.is_nat_fn(name);
            }
        }
    }
    false
}

/// Lower an `Index` expression across the four `IndexArg` forms (REQ-3/REQ-5).
/// In spec context: `xs[i]`→`xs@[i as int]`, `&xs[..i]`→`xs@.subrange(0, i as
/// int)`, `xs[i..]`→`xs@.subrange(i as int, xs@.len() as int)`,
/// `xs[i..j]`→`xs@.subrange(i as int, j as int)`. In exec context, plain Rust.
fn lower_index(
    base: &Expr,
    index: &IndexArg,
    ctx: Ctx,
    depth: usize,
    span: Span,
) -> Result<String, LowerError> {
    let b = lower_expr(base, ctx, depth, span)?;
    match (ctx.pos, index) {
        (Pos::Spec, IndexArg::Single(i)) => {
            let idx = lower_expr(i, ctx, depth, span)?;
            Ok(format!("{b}@[{idx} as int]"))
        }
        (Pos::Spec, IndexArg::RangeTo(i)) => {
            let idx = lower_expr(i, ctx, depth, span)?;
            Ok(format!("{b}@.subrange(0, {idx} as int)"))
        }
        (Pos::Spec, IndexArg::RangeFrom(i)) => {
            let idx = lower_expr(i, ctx, depth, span)?;
            Ok(format!("{b}@.subrange({idx} as int, {b}@.len() as int)"))
        }
        (Pos::Spec, IndexArg::Range(i, j)) => {
            let lo = lower_expr(i, ctx, depth, span)?;
            let hi = lower_expr(j, ctx, depth, span)?;
            Ok(format!("{b}@.subrange({lo} as int, {hi} as int)"))
        }
        (Pos::Exec, IndexArg::Single(i)) => {
            let idx = lower_expr(i, ctx, depth, span)?;
            Ok(format!("{b}[{idx}]"))
        }
        (Pos::Exec, IndexArg::RangeTo(i)) => {
            let idx = lower_expr(i, ctx, depth, span)?;
            Ok(format!("{b}[..{idx}]"))
        }
        (Pos::Exec, IndexArg::RangeFrom(i)) => {
            let idx = lower_expr(i, ctx, depth, span)?;
            Ok(format!("{b}[{idx}..]"))
        }
        (Pos::Exec, IndexArg::Range(i, j)) => {
            let lo = lower_expr(i, ctx, depth, span)?;
            let hi = lower_expr(j, ctx, depth, span)?;
            Ok(format!("{b}[{lo}..{hi}]"))
        }
    }
}

/// Lower a `match` (REQ-3). Used in `ens` (the `binary_search` `Option` match)
/// and in spec-fn bodies (the `sum` slice match, handled separately).
fn lower_match(
    scrutinee: &Expr,
    arms: &[MatchArm],
    ctx: Ctx,
    depth: usize,
    span: Span,
) -> Result<String, LowerError> {
    let s = lower_expr(scrutinee, ctx, depth, span)?;
    let mut out = format!("match {s} {{\n");
    for arm in arms {
        let pat = lower_pattern(&arm.pattern, ctx, depth, span)?;
        let body = lower_expr(&arm.body, ctx, depth, span)?;
        writeln!(out, "            {pat} => {body},").ok();
    }
    out.push_str("        }");
    Ok(out)
}

/// Lower a pattern (REQ-7/REQ-9 node set). A user enum-variant pattern is
/// ENUM-QUALIFIED (`Circle(r)`→`Shape::Circle(r)`, `Nil`→`List::Nil`) via the
/// `ctx.variants` map (verus rejects a bare variant); `Some(i)`/`None` and
/// bindings/wildcards/literals are NOT in the map, so they lower unqualified.
/// `Pattern::Struct` (`Rect { w, h }` / `Rect { .. }`) is REQ-4's struct-variant
/// destructuring (REQ-9 lowering).
fn lower_pattern(pat: &Pattern, ctx: Ctx, depth: usize, span: Span) -> Result<String, LowerError> {
    if depth >= MAX_EMIT_DEPTH {
        return Err(LowerError::TooDeep {
            limit: MAX_EMIT_DEPTH,
            span,
        });
    }
    match pat {
        Pattern::Wildcard => Ok("_".to_string()),
        Pattern::Binding(name) => Ok(name.clone()),
        Pattern::Literal(e) => lower_expr(e, Ctx::spec_seq(), depth + 1, span),
        Pattern::Enum { path, fields } => {
            let head = qualify_variant_path(path, ctx);
            if fields.is_empty() {
                Ok(head)
            } else {
                let mut fs = Vec::new();
                for f in fields {
                    fs.push(lower_pattern(f, ctx, depth + 1, span)?);
                }
                Ok(format!("{head}({})", fs.join(", ")))
            }
        }
        Pattern::Struct { path, fields, rest } => {
            // `Rect { w, h }` / `Rect { .. }` (REQ-4/REQ-9): an ENUM-QUALIFIED
            // struct-variant (or struct) destructuring pattern. Each field is
            // `name: <subpat>`; the `rest` flag emits the `..` of `Rect { .. }`.
            let head = qualify_variant_path(path, ctx);
            let mut parts = Vec::with_capacity(fields.len());
            for (name, subpat) in fields {
                let sub = lower_pattern(subpat, ctx, depth + 1, span)?;
                // A field-shorthand `Rect { w, h }` (parsed to `(w, Binding(w))`)
                // lowers to the bare field name; an explicit `field: pat` keeps the
                // `name: pat` form.
                if matches!(subpat, Pattern::Binding(b) if b == name) {
                    parts.push(name.clone());
                } else {
                    parts.push(format!("{name}: {sub}"));
                }
            }
            if *rest {
                parts.push("..".to_string());
            }
            if parts.is_empty() {
                Ok(format!("{head} {{}}"))
            } else {
                Ok(format!("{head} {{ {} }}", parts.join(", ")))
            }
        }
        Pattern::Slice(_) => Err(LowerError::Unsupported {
            what: "slice pattern outside a head-fold spec fn".to_string(),
            span,
        }),
    }
}

/// ENUM-QUALIFY a variant-pattern path (REQ-9): a single-segment user variant
/// `["Circle"]` becomes `Shape::Circle` via the `ctx.variants` map; an already
/// `::`-qualified path, a built-in (`Some`/`None`), or an unknown name is joined
/// as-written (verus knows `Option`; a user variant must be qualified or it is
/// rejected). Keyed on map membership, never on a name pattern.
fn qualify_variant_path(path: &[String], ctx: Ctx) -> String {
    if path.len() == 1 {
        if let Some(enum_name) = ctx.enum_of_variant(&path[0]) {
            return format!("{enum_name}::{}", path[0]);
        }
    }
    path.join("::")
}

/// The Verus/Rust operator for a `BinOp` (REQ-3).
fn binop(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        // #92 integer operators → their Verus-native operators. `%`/`<<`/`>>` carry
        // the divide-by-zero / shift-bound PROOF obligation Verus raises at the
        // operator site (ast.md REQ-11); the lowering emits the BARE operator and
        // MUST NOT suppress it (no `external`/`assume` — R-DEFER-9).
        BinOp::Rem => "%",
        BinOp::Shl => "<<",
        BinOp::Shr => ">>",
        BinOp::BitAnd => "&",
        BinOp::BitOr => "|",
        BinOp::BitXor => "^",
        BinOp::Eq => "==",
        BinOp::Ne => "!=",
        BinOp::Lt => "<",
        BinOp::Le => "<=",
        BinOp::Gt => ">",
        BinOp::Ge => ">=",
        BinOp::And => "&&",
        BinOp::Or => "||",
    }
}

/// Binding-power tier of a binary operator (higher binds tighter). Mirrors the
/// pinned standard-Rust precedence (`surface-grammar.md` REQ-10) closely enough to
/// decide parenthesization of nested binaries during emission (REQ-3 — preserve
/// the AST's grouping). The #92 tiers (modulo at `* /`, shifts, `&`, `^`, `|`)
/// slot between `+ -` and comparison exactly as the parser threads them.
fn precedence(op: BinOp) -> u8 {
    match op {
        BinOp::Or => 1,
        BinOp::And => 2,
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => 3,
        BinOp::BitOr => 4,
        BinOp::BitXor => 5,
        BinOp::BitAnd => 6,
        BinOp::Shl | BinOp::Shr => 7,
        BinOp::Add | BinOp::Sub => 8,
        BinOp::Mul | BinOp::Div | BinOp::Rem => 9,
    }
}

/// Lower an operand of a binary expression, wrapping it in parens when a child
/// binary's precedence is lower than (or, for the right child of a
/// left-associative operator, equal to) the parent's — so the AST's grouping is
/// preserved verbatim (`lo + (hi - lo) / 2`, not `lo + hi - lo / 2`). `is_left`
/// distinguishes the two children for associativity.
fn lower_binary_operand(
    operand: &Expr,
    parent: BinOp,
    is_left: bool,
    ctx: Ctx,
    depth: usize,
    span: Span,
) -> Result<String, LowerError> {
    let s = lower_expr(operand, ctx, depth, span)?;
    if let Expr::Binary { op: child, .. } = operand {
        let pp = precedence(parent);
        let cp = precedence(*child);
        let needs = cp < pp || (!is_left && cp == pp);
        if needs {
            return Ok(format!("({s})"));
        }
    }
    Ok(s)
}

// ---------------------------------------------------------------------------
// REQ-4: statement, block and loop lowering (exec body).
// ---------------------------------------------------------------------------

/// Lower a `fn` body, threading the shape-derived proof aids (REQ-7). The body
/// is emitted between `{` and `}`; the loop lowering injects per-loop aids and
/// the extensionality assert at exit.
fn lower_fn_body(
    f: &FnItem,
    nat_fns: &[&str],
    string_fields: &[&str],
    variants: &[(&str, &str)],
) -> Result<String, LowerError> {
    let mut out = String::from("{\n");
    // A boundary fn (ffi-boundary.md REQ-2/OQ-3) has `body: None` and is NEVER
    // lowered to Verus — `forge`'s `check.rs` routes it to the L1 boundary path
    // BEFORE `lower` ever sees it (the foreign body cannot be proved). Reaching
    // here with no body is a structured error (R-CODE-2), never an unwrap.
    let body = f.body.as_ref().ok_or_else(|| LowerError::Unsupported {
        what: "lower (L3 Verus) reached a bodyless (boundary) fn; a boundary fn \
               certifies at L1 and is never lowered to Verus (ffi-boundary.md OQ-3)"
            .to_string(),
        span: f.span,
    })?;
    let inner = lower_block_with_fn_aids(body, f, nat_fns, string_fields, variants, 1)?;
    out.push_str(&inner);
    out.push_str("}\n");
    Ok(out)
}

/// Lower a block with the enclosing `fn`'s contract in scope, so loop lowering
/// can lift immutable preconditions and emit accumulator/coverage aids (REQ-7).
/// The exec context carries the enum-variant map (REQ-9) so a `match` over a user
/// enum (e.g. `is_circle`'s body) lowers to ENUM-QUALIFIED arms.
fn lower_block_with_fn_aids(
    block: &Block,
    f: &FnItem,
    nat_fns: &[&str],
    string_fields: &[&str],
    variants: &[(&str, &str)],
    indent: usize,
) -> Result<String, LowerError> {
    let pad = "    ".repeat(indent);
    let exec = Ctx::exec().with_variants(variants);
    let mut out = String::new();
    for stmt in &block.stmts {
        match stmt {
            Stmt::Loop(l) => {
                out.push_str(&lower_loop(l, f, nat_fns, string_fields, variants, indent)?);
            }
            other => {
                out.push_str(&lower_stmt(other, exec, indent)?);
            }
        }
    }
    if let Some(tail) = &block.tail {
        let t = lower_expr(tail, exec, 0, f.span)?;
        writeln!(out, "{pad}{t}").ok();
    }
    Ok(out)
}

/// Lower a plain block (no fn-level aids) in the given context.
fn lower_block_inner(
    block: &Block,
    ctx: Ctx,
    depth: usize,
    span: Span,
) -> Result<String, LowerError> {
    let mut out = String::new();
    for stmt in &block.stmts {
        out.push_str(&lower_stmt(stmt, ctx, depth + 1)?);
    }
    if let Some(tail) = &block.tail {
        let t = lower_expr(tail, ctx, depth, span)?;
        writeln!(out, "    {t}").ok();
    }
    Ok(out)
}

/// Lower a single statement (REQ-4).
fn lower_stmt(stmt: &Stmt, ctx: Ctx, indent: usize) -> Result<String, LowerError> {
    let pad = "    ".repeat(indent);
    match stmt {
        Stmt::Let {
            mutable,
            name,
            ty,
            init,
        } => {
            let kw = if *mutable { "let mut" } else { "let" };
            let init_s = lower_expr(init, ctx, 0, zero_span())?;
            if let Some(t) = ty {
                let ts = lower_type(t)?;
                Ok(format!("{pad}{kw} {name}: {ts} = {init_s};\n"))
            } else {
                Ok(format!("{pad}{kw} {name} = {init_s};\n"))
            }
        }
        Stmt::Assign { target, value } => {
            let t = lower_expr(target, ctx, 0, zero_span())?;
            let v = lower_expr(value, ctx, 0, zero_span())?;
            Ok(format!("{pad}{t} = {v};\n"))
        }
        Stmt::Return(e) => match e {
            Some(e) => {
                let s = lower_expr(e, ctx, 0, zero_span())?;
                Ok(format!("{pad}return {s};\n"))
            }
            None => Ok(format!("{pad}return;\n")),
        },
        Stmt::If { cond, then, else_ } => {
            let c = lower_expr(cond, ctx, 0, zero_span())?;
            let t = lower_block_inner(then, ctx, indent, zero_span())?;
            let mut out = format!("{pad}if {c} {{\n{t}{pad}}}");
            if let Some(e) = else_ {
                let es = lower_block_inner(e, ctx, indent, zero_span())?;
                write!(out, " else {{\n{es}{pad}}}").ok();
            }
            out.push('\n');
            Ok(out)
        }
        Stmt::Expr(e) => {
            let s = lower_expr(e, ctx, 0, zero_span())?;
            Ok(format!("{pad}{s};\n"))
        }
        Stmt::Loop(_) => Err(LowerError::Unsupported {
            what: "nested loop without fn-aid context".to_string(),
            span: zero_span(),
        }),
    }
}

// ---------------------------------------------------------------------------
// REQ-7: shape-keyed proof-aid templates. The hard part.
// ---------------------------------------------------------------------------

/// Lower a loop (REQ-4) with its shape-derived proof aids (REQ-7). Emits every
/// `inv`→`invariant`, the `dec`→`decreases`, and:
///  - template (b): every immutable-param precondition of the enclosing `fn`
///    that the loop does not already restate is lifted into the invariants;
///  - template (c)+(a): if an invariant has the accumulator shape
///    `acc as nat == specfn(slice@.subrange(0, idx as int))`, emit + call the
///    auto-generated push lemma for `specfn`;
///  - template (overflow): if the body assigns `acc = acc + slice[idx] ...` and
///    an invariant bounds `acc <= idx * BOUND`, emit the `by(nonlinear_arith)`
///    overflow discharge;
///  - template (e): if a `None`/false-postcondition `forall_in(s, p)` is
///    provable from `forall_below(s,k,p1)` + `forall_from(s,k',p2)`, emit the
///    loop-exit coverage case-split inside the `if lo == hi` branch;
///  - template (d): if an accumulator invariant uses `subrange(0, idx)` and the
///    loop exits when `idx == len`, emit the `=~=` extensionality after the loop.
fn lower_loop(
    l: &thermite_syntax::ast::LoopNode,
    f: &FnItem,
    nat_fns: &[&str],
    string_fields: &[&str],
    variants: &[(&str, &str)],
    indent: usize,
) -> Result<String, LowerError> {
    use thermite_syntax::ast::LoopKind;
    let pad = "    ".repeat(indent);
    let ipad = "    ".repeat(indent + 1);
    let exec = Ctx::exec().with_variants(variants);
    let mut out = String::new();

    // Loop header.
    match &l.kind {
        LoopKind::Loop => writeln!(out, "{pad}loop").map_err(|_| fmt_err())?,
        LoopKind::While(c) => {
            let cs = lower_expr(c, exec, 0, f.span)?;
            writeln!(out, "{pad}while {cs}").map_err(|_| fmt_err())?;
        }
    };

    let slices = slice_param_names(&f.params);
    let strings = string_value_names(f);
    let spec = Ctx::spec(&slices, nat_fns)
        .with_strings(&strings)
        .with_string_fields(string_fields);

    // Invariants: the loop's own `inv`s, then lifted immutable preconditions
    // (template b) not already present.
    out.push_str(&format!("{ipad}invariant\n"));
    let mut inv_strings: Vec<String> = Vec::new();
    for inv in &l.invs {
        inv_strings.push(lower_expr(&inv.expr, spec, 0, f.span)?);
    }
    let lifted = lift_immutable_preconds(f, spec, &inv_strings)?;
    for inv in inv_strings.iter().chain(lifted.iter()) {
        writeln!(out, "{ipad}    {inv},").map_err(|_| fmt_err())?;
    }

    // decreases (§4.1: "Termination is proved by default"). SUPPRESSED for a
    // `fx diverge` fn: an event loop is non-terminating BY DESIGN, so emitting a
    // `decreases` would force Verus to prove a termination measure that honestly
    // does not exist. The enclosing fn carries `#[verifier::exec_allows_no_
    // decreases_clause]` (see `lower_fn`), so Verus verifies the loop's
    // INVARIANTS (partial correctness) WITHOUT a termination claim — the honest
    // L1 verdict. A non-diverge fn ALWAYS emits its `decreases` (sum/binary_search
    // still prove termination → L3): the exemption is diverge-ONLY and is not a
    // termination-proof escape hatch.
    if !fn_is_diverge(f) {
        let dec = lower_expr(&l.dec.expr, spec, 0, f.span)?;
        writeln!(out, "{ipad}decreases {dec},").map_err(|_| fmt_err())?;
    }

    // Body open.
    writeln!(out, "{pad}{{").map_err(|_| fmt_err())?;

    // template (c)+(a): the push-lemma proof block, emitted before the body if
    // an accumulator invariant of the recursive-fold shape is present.
    let acc_aid = accumulator_aid(f, &l.invs)?;
    if let Some((lemma_call, _)) = &acc_aid {
        writeln!(out, "{ipad}proof {{ {lemma_call} }}").map_err(|_| fmt_err())?;
    }
    // template (overflow): the nonlinear_arith discharge, if the body grows an
    // accumulator bounded by a product invariant.
    if let Some(assert_line) = nonlinear_overflow_assert(f, &l.invs, &l.body)? {
        writeln!(out, "{ipad}{assert_line}").map_err(|_| fmt_err())?;
    }

    // The body statements, with the loop-exit coverage split injected into the
    // matching `if` branch (template e).
    let body_src = lower_loop_body(&l.body, f, &l.invs, variants, indent + 1)?;
    out.push_str(&body_src);

    writeln!(out, "{pad}}}").map_err(|_| fmt_err())?;

    // template (d): extensionality at exit, if an accumulator invariant folds a
    // subrange and the loop is `while idx < len` (exits at idx == len).
    if let Some(ext) = extensionality_at_exit(f, l, &acc_aid)? {
        writeln!(out, "{pad}{ext}").map_err(|_| fmt_err())?;
    }

    Ok(out)
}

fn fmt_err() -> LowerError {
    LowerError::Unsupported {
        what: "string formatting".to_string(),
        span: zero_span(),
    }
}

/// Describes a recursive-fold accumulator invariant matched by SHAPE: an
/// invariant `accvar as nat == specfn(slice@.subrange(0, idxvar as int))`.
struct AccInfo {
    specfn: String,
    slice: String,
    idxvar: String,
}

/// Match the accumulator invariant SHAPE in a loop's `inv`s (template c). Keys on
/// the AST shape `Binary{Eq, Cast{acc, nat-ish}, Call{specfn, [subrange(slice, 0, idx)]}}`
/// — NOT on any name. Returns the spec-fn name, slice name, and index var.
fn match_acc_invariant(invs: &[Clause]) -> Option<AccInfo> {
    for inv in invs {
        if let Expr::Binary {
            op: BinOp::Eq,
            lhs,
            rhs,
        } = &inv.expr
        {
            // lhs is `acc` (possibly cast); rhs is `specfn(&slice[..idx])`.
            if let Expr::Call { callee, args } = rhs.as_ref() {
                if let (Expr::Path(segs), [arg0]) = (callee.as_ref(), args.as_slice()) {
                    if let Some(specfn) = segs.last() {
                        // The single arg must be a `&slice[..idx]` (RangeTo) shape.
                        if let Some((slice, idxvar)) = match_range_to_slice(arg0) {
                            // and lhs must reference a single var (the accumulator).
                            let _ = lhs;
                            return Some(AccInfo {
                                specfn: specfn.clone(),
                                slice,
                                idxvar,
                            });
                        }
                    }
                }
            }
        }
    }
    None
}

/// Match a `&slice[..idx]` expression, returning `(slice, idx)` where both are
/// simple path names. Shape: `Ref{ Index{ base: Path[slice], RangeTo(Path[idx]) } }`
/// or the bare `Index` without the `&`.
fn match_range_to_slice(expr: &Expr) -> Option<(String, String)> {
    let inner = match expr {
        Expr::Ref { expr, .. } => expr.as_ref(),
        other => other,
    };
    if let Expr::Index { base, index } = inner {
        if let (Expr::Path(bsegs), IndexArg::RangeTo(i)) = (base.as_ref(), index) {
            if let (Some(slice), Expr::Path(isegs)) = (bsegs.last(), i.as_ref()) {
                if let Some(idx) = isegs.last() {
                    return Some((slice.clone(), idx.clone()));
                }
            }
        }
    }
    None
}

/// template (b): lift each immutable-param precondition of the `fn`'s `req` into
/// the loop invariants when not already present. Keys on SHAPE: a `req`
/// conjunct that mentions only immutable (slice/param) state — concretely, any
/// `req` conjunct that does not mention a loop-local mutable. Because v0.1 has a
/// single `req` clause and the corpus precondition (`xs.len() <= 1_000_000`)
/// references only the immutable slice, we lift the whole `req` if it is not
/// already among the invariants. A `true` req lifts nothing.
fn lift_immutable_preconds(
    f: &FnItem,
    spec: Ctx,
    existing_invs: &[String],
) -> Result<Vec<String>, LowerError> {
    let req = lower_expr(&f.contract.req.expr, spec, 0, f.span)?;
    if req == "true" {
        return Ok(Vec::new());
    }
    // Only lift conjuncts that reference an immutable param name and NOT a
    // mutated local. We approximate "immutable" by: the conjunct references a
    // fn param. The corpus reqs (`xs.len() <= 1_000_000`, `sorted(haystack)`)
    // reference an immutable slice param and no loop-local. Already-present
    // invariants are skipped. Lowered with the fn's slice ctx so a slice arg
    // gets its `@` view (REQ-5).
    let param_names: Vec<&str> = f.params.iter().map(|p| p.name.as_str()).collect();
    let mut lifted = Vec::new();
    for conj in split_conjuncts(&f.contract.req.expr) {
        let lowered = lower_expr(conj, spec, 0, f.span)?;
        let mentions_param = param_names.iter().any(|p| expr_mentions(conj, p));
        if mentions_param && !existing_invs.iter().any(|e| e == &lowered) {
            lifted.push(lowered);
        }
    }
    Ok(lifted)
}

/// Split an expression into top-level `&&` conjuncts (for precondition lifting).
fn split_conjuncts(expr: &Expr) -> Vec<&Expr> {
    let mut out = Vec::new();
    fn go<'a>(e: &'a Expr, acc: &mut Vec<&'a Expr>) {
        if let Expr::Binary {
            op: BinOp::And,
            lhs,
            rhs,
        } = e
        {
            go(lhs, acc);
            go(rhs, acc);
        } else {
            acc.push(e);
        }
    }
    go(expr, &mut out);
    out
}

/// True if `expr` syntactically mentions identifier `name` anywhere.
fn expr_mentions(expr: &Expr, name: &str) -> bool {
    match expr {
        Expr::Path(segs) => segs.iter().any(|s| s == name),
        Expr::IntLit { .. } | Expr::BoolLit(_) | Expr::StrLit(_) => false,
        Expr::Call { callee, args } => {
            expr_mentions(callee, name) || args.iter().any(|a| expr_mentions(a, name))
        }
        Expr::MethodCall { receiver, args, .. } => {
            expr_mentions(receiver, name) || args.iter().any(|a| expr_mentions(a, name))
        }
        Expr::Field { receiver, .. } => expr_mentions(receiver, name),
        Expr::Closure { body, .. } => expr_mentions(body, name),
        Expr::Match { scrutinee, arms } => {
            expr_mentions(scrutinee, name) || arms.iter().any(|a| expr_mentions(&a.body, name))
        }
        Expr::If { cond, .. } => expr_mentions(cond, name),
        Expr::Binary { lhs, rhs, .. } => expr_mentions(lhs, name) || expr_mentions(rhs, name),
        Expr::Index { base, index } => {
            expr_mentions(base, name)
                || match index {
                    IndexArg::Single(e) | IndexArg::RangeTo(e) | IndexArg::RangeFrom(e) => {
                        expr_mentions(e, name)
                    }
                    IndexArg::Range(a, b) => expr_mentions(a, name) || expr_mentions(b, name),
                }
        }
        Expr::Cast { expr, .. } | Expr::Ref { expr, .. } => expr_mentions(expr, name),
        // Basis Stage 1a (`.design/basis/01-adts.md`): dead-in-1a ADT
        // expressions, but the honest predicate value is to descend — a name
        // could be mentioned in a struct-literal field value, an `is`
        // scrutinee, or a deref operand, so we must not silently answer `false`.
        Expr::StructLit { fields, .. } => fields.iter().any(|(_, v)| expr_mentions(v, name)),
        Expr::Is { scrutinee, .. } => expr_mentions(scrutinee, name),
        Expr::Deref(inner) => expr_mentions(inner, name),
        // The prefix `!` (#92): a name can be mentioned under it (`!done`).
        Expr::Unary { expr, .. } => expr_mentions(expr, name),
    }
}

/// template (c)+(a): if a loop carries an accumulator invariant of the
/// recursive-fold shape, return `(lemma_call, lemma_def)` — the in-loop
/// `proof { lemma_<specfn>_push(slice@, idx as int); }` call and the
/// auto-generated push lemma definition. The lemma definition is emitted at file
/// scope by `lower` via `collect_push_lemmas`. Here we only return the call.
fn accumulator_aid(f: &FnItem, invs: &[Clause]) -> Result<Option<(String, String)>, LowerError> {
    let _ = f;
    if let Some(info) = match_acc_invariant(invs) {
        let call = format!(
            "lemma_{}_push({}@, {} as int);",
            info.specfn, info.slice, info.idxvar
        );
        let def = push_lemma_for(&info.specfn);
        return Ok(Some((call, def)));
    }
    Ok(None)
}

/// Collect the auto-generated push-lemma definitions a `fn` needs: one per loop
/// carrying an accumulator-fold invariant of the recursive-fold shape (REQ-7
/// template a). Keyed on the invariant SHAPE (`match_acc_invariant`), never on
/// the program. Emitted at file scope by `lower` before the `fn`.
fn push_lemma_defs_for_fn(f: &FnItem) -> Result<Vec<String>, LowerError> {
    let mut defs = Vec::new();
    // A boundary fn (ffi-boundary.md REQ-2) has `body: None` — no loop bodies, so
    // no accumulator-fold push lemmas. A boundary fn never reaches L3 anyway
    // (`lower_fn` errors on a bodyless fn); this keeps the collector total.
    if let Some(body) = &f.body {
        collect_push_lemmas_in_block(body, &mut defs);
    }
    Ok(defs)
}

fn collect_push_lemmas_in_block(block: &Block, defs: &mut Vec<String>) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Loop(l) => {
                if let Some(info) = match_acc_invariant(&l.invs) {
                    defs.push(push_lemma_for(&info.specfn));
                }
                collect_push_lemmas_in_block(&l.body, defs);
            }
            Stmt::If { then, else_, .. } => {
                collect_push_lemmas_in_block(then, defs);
                if let Some(e) = else_ {
                    collect_push_lemmas_in_block(e, defs);
                }
            }
            _ => {}
        }
    }
}

/// template (a): the auto-generated push (unfold) induction lemma for a
/// head-fold spec fn `specfn`. It relates `specfn(xs.subrange(0, k+1))` to
/// `specfn(xs.subrange(0, k)) + xs[k]`. Keyed PURELY on the spec-fn name passed
/// in (which itself was derived from the accumulator-invariant SHAPE); the body
/// is the general drop_first induction, identical in structure for any
/// head-fold-sum spec fn. NOT program-specific.
fn push_lemma_for(specfn: &str) -> String {
    format!(
        "proof fn lemma_{specfn}_push(xs: Seq<u32>, k: int)\n    requires 0 <= k < xs.len(),\n    ensures {specfn}(xs.subrange(0, k + 1)) == {specfn}(xs.subrange(0, k)) + xs[k] as nat,\n    decreases k,\n{{\n    if k == 0 {{\n        assert(xs.subrange(0, 1).drop_first() =~= xs.subrange(0, 0));\n    }} else {{\n        lemma_{specfn}_push(xs.drop_first(), k - 1);\n        assert(xs.subrange(0, k + 1).drop_first() =~= xs.drop_first().subrange(0, k));\n        assert(xs.subrange(0, k).drop_first() =~= xs.drop_first().subrange(0, k - 1));\n    }}\n}}\n"
    )
}

/// template (overflow): if the loop body assigns `acc = acc + slice[idx] as T`
/// and an invariant bounds `acc <= idx as T * BOUND`, emit the
/// `by(nonlinear_arith)` discharge with the in-scope invariant/precondition
/// hypotheses as `requires`. Keys on SHAPE: an `Assign` whose value is
/// `acc + (slice[idx] cast)`, plus a product-bound invariant on the same `acc`.
fn nonlinear_overflow_assert(
    f: &FnItem,
    invs: &[Clause],
    body: &Block,
) -> Result<Option<String>, LowerError> {
    // Find `acc = acc + slice[idx] as T;` in the body.
    let Some((accvar, idxvar)) = find_accumulator_growth(body) else {
        return Ok(None);
    };
    // Find the product-bound invariant `acc <= idx as T * BOUND`.
    let Some((bound_factor, bound_ty)) = find_product_bound(invs, &accvar, &idxvar) else {
        return Ok(None);
    };
    // Gather the hypotheses: the product bound, `idx < slice.len()`, and the
    // lifted immutable precondition (all from the loop's own state + req).
    let slice = first_slice_param(&f.params).unwrap_or("xs");
    let req = lower_expr(&f.contract.req.expr, Ctx::spec_seq(), 0, f.span)?;
    let mut hyps = vec![
        format!("{accvar} <= {idxvar} as {bound_ty} * {bound_factor}",),
        format!("{idxvar} < {slice}.len()"),
    ];
    if req != "true" {
        hyps.push(req);
    }
    let line = format!(
        "assert({accvar} + {slice}[{idxvar} as int] as {bound_ty} <= ({idxvar} as {bound_ty} + 1) * {bound_factor}) by(nonlinear_arith)\n        requires {};",
        hyps.join(", ")
    );
    Ok(Some(line))
}

/// Find an accumulator-growth assignment `accvar = accvar + slice[idxvar] as T;`
/// in a block. Returns `(accvar, idxvar)`. SHAPE match only.
fn find_accumulator_growth(block: &Block) -> Option<(String, String)> {
    for stmt in &block.stmts {
        let Stmt::Assign {
            target: Expr::Path(tsegs),
            value:
                Expr::Binary {
                    op: BinOp::Add,
                    lhs,
                    rhs,
                },
        } = stmt
        else {
            continue;
        };
        let Some(accvar) = tsegs.last() else {
            continue;
        };
        // value = accvar + (slice[idx] as T)
        if let Expr::Path(lsegs) = lhs.as_ref() {
            if lsegs.last() == Some(accvar) {
                // rhs is `slice[idx] as T` (Cast over Index Single).
                if let Some(idxvar) = index_var_of_cast(rhs) {
                    return Some((accvar.clone(), idxvar));
                }
            }
        }
    }
    None
}

/// Extract the index var of a `slice[idx] as T` expression (or bare `slice[idx]`).
fn index_var_of_cast(expr: &Expr) -> Option<String> {
    let inner = match expr {
        Expr::Cast { expr, .. } => expr.as_ref(),
        other => other,
    };
    if let Expr::Index {
        index: IndexArg::Single(i),
        ..
    } = inner
    {
        if let Expr::Path(segs) = i.as_ref() {
            return segs.last().cloned();
        }
    }
    None
}

/// Find a product-bound invariant `accvar <= idxvar as T * FACTOR`. Returns
/// `(factor_string, T)`. SHAPE match.
fn find_product_bound(invs: &[Clause], accvar: &str, idxvar: &str) -> Option<(String, String)> {
    for inv in invs {
        if let Expr::Binary {
            op: BinOp::Le,
            lhs,
            rhs,
        } = &inv.expr
        {
            if let Expr::Path(lsegs) = lhs.as_ref() {
                if lsegs.last().map(|s| s == accvar).unwrap_or(false) {
                    // rhs = (idxvar as T) * FACTOR
                    if let Expr::Binary {
                        op: BinOp::Mul,
                        lhs: ml,
                        rhs: mr,
                    } = rhs.as_ref()
                    {
                        if let Expr::Cast { expr, ty } = ml.as_ref() {
                            if let Expr::Path(isegs) = expr.as_ref() {
                                if isegs.last().map(|s| s == idxvar).unwrap_or(false) {
                                    let t = lower_type(ty).ok()?;
                                    let factor =
                                        lower_expr(mr, Ctx::spec_seq(), 0, zero_span()).ok()?;
                                    return Some((factor, t));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// template (d): the `=~=` extensionality assert after a `while idx < slice.len()`
/// loop carrying an accumulator over `slice@.subrange(0, idx)` — at exit
/// `idx == len`, so `subrange(0, len) =~= slice@`. Keys on: an accumulator-aid
/// loop whose `while` condition is `idx < slice.len()`.
fn extensionality_at_exit(
    f: &FnItem,
    l: &thermite_syntax::ast::LoopNode,
    acc_aid: &Option<(String, String)>,
) -> Result<Option<String>, LowerError> {
    use thermite_syntax::ast::LoopKind;
    if acc_aid.is_none() {
        return Ok(None);
    }
    let Some(info) = match_acc_invariant(&l.invs) else {
        return Ok(None);
    };
    // Confirm the loop is `while idx < slice.len()` for this idx/slice.
    let LoopKind::While(cond) = &l.kind else {
        return Ok(None);
    };
    if let Expr::Binary {
        op: BinOp::Lt,
        lhs,
        rhs,
    } = cond.as_ref()
    {
        let lhs_is_idx = matches!(lhs.as_ref(), Expr::Path(s) if s.last().map(|x| x == &info.idxvar).unwrap_or(false));
        let rhs_is_len = matches!(rhs.as_ref(), Expr::MethodCall { receiver, name, .. }
            if name == "len" && matches!(receiver.as_ref(), Expr::Path(s) if s.last().map(|x| x == &info.slice).unwrap_or(false)));
        if lhs_is_idx && rhs_is_len {
            let _ = f;
            return Ok(Some(format!(
                "assert({s}@.subrange(0, {s}.len() as int) =~= {s}@);",
                s = info.slice
            )));
        }
    }
    Ok(None)
}

/// Lower a loop body, injecting the complementary-coverage case-split (template
/// e) into the `if <exit-cond>` branch that returns the negative/None result.
fn lower_loop_body(
    body: &Block,
    f: &FnItem,
    invs: &[Clause],
    variants: &[(&str, &str)],
    indent: usize,
) -> Result<String, LowerError> {
    // Pre-compute the coverage split, if this loop's invariants + the fn's
    // None-postcondition match template (e).
    let coverage = complementary_coverage_split(f, invs)?;
    let exec = Ctx::exec().with_variants(variants);

    let mut out = String::new();
    for stmt in &body.stmts {
        if let (Some(cov), Stmt::If { cond, then, else_ }) = (&coverage, stmt) {
            // Inject the split into the branch whose body `return`s the negative
            // result, when the guard matches the coverage's exit condition.
            if if_is_coverage_exit(cond, &cov.guard) {
                out.push_str(&emit_if_with_split(
                    cond,
                    then,
                    else_,
                    &cov.assert_block,
                    f,
                    variants,
                    indent,
                )?);
                continue;
            }
        }
        out.push_str(&lower_stmt(stmt, exec, indent)?);
    }
    if let Some(tail) = &body.tail {
        let pad = "    ".repeat(indent);
        let t = lower_expr(tail, exec, 0, f.span)?;
        writeln!(out, "{pad}{t}").map_err(|_| fmt_err())?;
    }
    Ok(out)
}

/// Whether an `if` condition is the coverage exit `lo == hi` for the matched
/// guard variables.
fn if_is_coverage_exit(cond: &Expr, guard: &(String, String)) -> bool {
    if let Expr::Binary {
        op: BinOp::Eq,
        lhs,
        rhs,
    } = cond
    {
        let l = matches!(lhs.as_ref(), Expr::Path(s) if s.last().map(|x| x == &guard.0).unwrap_or(false));
        let r = matches!(rhs.as_ref(), Expr::Path(s) if s.last().map(|x| x == &guard.1).unwrap_or(false));
        return l && r;
    }
    false
}

/// Emit the coverage-exit `if` with the case-split assert prepended to its
/// `then` block (template e).
fn emit_if_with_split(
    cond: &Expr,
    then: &Block,
    else_: &Option<Block>,
    split: &str,
    f: &FnItem,
    variants: &[(&str, &str)],
    indent: usize,
) -> Result<String, LowerError> {
    let pad = "    ".repeat(indent);
    let exec = Ctx::exec().with_variants(variants);
    let c = lower_expr(cond, exec, 0, f.span)?;
    let mut out = format!("{pad}if {c} {{\n");
    // The split assert, indented one level in.
    for line in split.lines() {
        if line.is_empty() {
            out.push('\n');
        } else {
            writeln!(out, "{pad}    {line}").map_err(|_| fmt_err())?;
        }
    }
    let then_src = lower_block_inner(then, exec, indent, f.span)?;
    out.push_str(&then_src);
    out.push_str(&format!("{pad}}}"));
    if let Some(e) = else_ {
        let es = lower_block_inner(e, exec, indent, f.span)?;
        write!(out, " else {{\n{es}{pad}}}").map_err(|_| fmt_err())?;
    }
    out.push('\n');
    Ok(out)
}

/// The result of matching template (e): the two guard variables whose equality
/// (`below_var == from_var`, the `lo == hi` exit) triggers the split, plus the
/// emitted `assert(forall_in(...)) by { ... }` case-split block.
struct CoverageSplit {
    guard: (String, String),
    assert_block: String,
}

/// template (e): the complementary-bounded-quantifier coverage case-split. When
/// the `fn`'s `None`/false postcondition is `forall_in(s, p)` and the loop
/// invariants include `forall_below(s, k, p1)` and `forall_from(s, k', p2)` with
/// `k == k'` at loop exit (the `lo == hi` guard), the negative postcondition is
/// provable by a case-split on the index: below `k` use `p1`, from `k'` use
/// `p2`. Keys on the SHAPE of the postcondition + invariants (three combinator
/// calls over the same slice with complementary index bounds), never on the
/// program name.
fn complementary_coverage_split(
    f: &FnItem,
    invs: &[Clause],
) -> Result<Option<CoverageSplit>, LowerError> {
    // 1. Find a `None => forall_in(s, ptarget)` arm in some `ens`.
    let Some((slice, ptarget)) = find_none_forall_in(&f.contract.ens) else {
        return Ok(None);
    };

    // 2. Find `forall_below(slice, below_var, p_below)` and
    //    `forall_from(slice, from_var, p_from)` invariants over the same slice.
    let mut below: Option<(String, String)> = None; // (var, pred)
    let mut from: Option<(String, String)> = None;
    for inv in invs {
        if let Some((s, var, pred)) = match_bounded_combinator(&inv.expr, "forall_below") {
            if s == slice {
                below = Some((var, pred));
            }
        }
        if let Some((s, var, pred)) = match_bounded_combinator(&inv.expr, "forall_from") {
            if s == slice {
                from = Some((var, pred));
            }
        }
    }
    let (Some((below_var, p_below)), Some((from_var, p_from))) = (below, from) else {
        return Ok(None);
    };

    // 3. The guard at exit is `below_var == from_var` (the `lo == hi` shape).
    //    Build the assert: forall k in [0,len): below k -> p_below; else p_from.
    let target = ptarget;
    let split = format!(
        "assert(forall_in({slice}@, {target})) by {{\n    assert forall|k: int| 0 <= k < {slice}@.len()\n        implies ({target})({slice}@[k]) by {{\n        if k < {below_var} as int {{\n            assert(({p_below})({slice}@[k]));\n        }} else {{\n            assert(({p_from})({slice}@[k]));\n        }}\n    }}\n}}",
    );
    Ok(Some(CoverageSplit {
        guard: (below_var, from_var),
        assert_block: split,
    }))
}

/// Find a `match result { ... None => forall_in(slice, pred) ... }` ensures arm,
/// returning `(slice, lowered_pred)`. SHAPE match on the ensures.
fn find_none_forall_in(ens: &[Clause]) -> Option<(String, String)> {
    for clause in ens {
        if let Expr::Match { arms, .. } = &clause.expr {
            for arm in arms {
                let is_none = matches!(&arm.pattern, Pattern::Enum { path, fields }
                    if fields.is_empty() && path.last().map(|p| p == "None").unwrap_or(false));
                if is_none {
                    if let Expr::Call { callee, args } = &arm.body {
                        if let (Expr::Path(segs), [s, p]) = (callee.as_ref(), args.as_slice()) {
                            if segs.last().map(|x| x == "forall_in").unwrap_or(false) {
                                let slice = slice_name(s)?;
                                let pred = lower_expr(p, Ctx::spec_seq(), 0, zero_span()).ok()?;
                                return Some((slice, pred));
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Match `comb(slice, var, pred)` (a `forall_below`/`forall_from` call),
/// returning `(slice, var, lowered_pred)`. SHAPE match.
fn match_bounded_combinator(expr: &Expr, comb: &str) -> Option<(String, String, String)> {
    if let Expr::Call { callee, args } = expr {
        if let (Expr::Path(segs), [s, v, p]) = (callee.as_ref(), args.as_slice()) {
            if segs.last().map(|x| x == comb).unwrap_or(false) {
                let slice = slice_name(s)?;
                let var = match v {
                    Expr::Path(vs) => vs.last()?.clone(),
                    _ => return None,
                };
                let pred = lower_expr(p, Ctx::spec_seq(), 0, zero_span()).ok()?;
                return Some((slice, var, pred));
            }
        }
    }
    None
}

/// The bare name of a slice-shaped argument (a `Path` head).
fn slice_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(segs) => segs.last().cloned(),
        _ => None,
    }
}

/// `dec`/`decreases` lowering for a spec fn: the measure expression in spec
/// context, with slice `.len()` viewed appropriately. The corpus `dec xs.len()`
/// lowers to `xs.len()` (Verus coerces a `Seq` `.len()` here).
fn spec_dec(dec: &Clause, _params: &[Param]) -> String {
    lower_expr(&dec.expr, Ctx::spec_seq(), 0, zero_span()).unwrap_or_else(|_| "0".to_string())
}
