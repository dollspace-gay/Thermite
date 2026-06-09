//! The per-clause Z3 equivalence obligation builder
//! (`.design/verified/contract-tv.md` REQ-2; `thermite-design.md` §6, L3 = the
//! verus-derived SMT proof).
//!
//! [`equivalence_obligation`] emits a SELF-CONTAINED Verus program text whose
//! single proof obligation is `assert((P_production) <==> (P_reference))`:
//!
//! ```text
//! use vstd::prelude::*;
//! verus! {
//!     <frame.spec_defs — the in-scope spec fn / combinator verus_l3 defs>
//!     proof fn tv_check(<frame.params>) requires <frame.req> {
//!         assert((<p_production>) <==> (<ref_contract_pred(source)>));
//!     }
//! }
//! fn main() {}
//! ```
//!
//! VERIFIED ⟺ the production predicate is logically equivalent to the reference
//! for ALL inputs (Z3) ⟺ faithful. A COUNTEREXAMPLE (a concrete input on which
//! they differ) ⟺ infidelity — a witness of the lowering bug
//! (`thermite-design.md` §5.1 "counterexamples, not adjectives").
//!
//! `thermite-tv` does NOT run verus itself: it emits the obligation TEXT. The
//! teeth-test (`tests/teeth.rs`, REQ-4) and the future forge plug-in (REQ-5,
//! `forge/src/contract_tv.rs`) discharge it through the existing
//! `forge::check::run_verus` path.
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-2 (per-clause Z3 equivalence obligation + discharge) | SHIPPED | `pub fn equivalence_obligation` + `pub struct ObligationFrame`/`ParamDecl` here; non-test consumer `thermite_tv::lib` re-export → `tests/teeth.rs` discharges F1–F4 through real verus (faithful VERIFIES, infidel COUNTEREXAMPLE). The reference side is `ref_encode::ref_contract_pred` (REQ-1); the production side is the verbatim `p_production` argument (the artifact under test). |
//!
//! ## EXEC-position extension (`.design/verified/exec-tv.md` REQ-2; epic #151)
//!
//! [`exec_equivalence_obligation`] is the EXEC dual: it emits the EXEC-FN-wrapped
//! `fn tv_exec_wrap(..) requires <req>, ensures result == <exec_ref_value(source)>
//! { <p_production> }` form (NOT the proof-fn `<==>` — an exec VALUE is not a
//! predicate). Verus reasons about the exec fn's value through its `ensures`:
//! VERIFIED ⟺ faithful; a `postcondition not satisfied` / `E0308` / parse error ⟺
//! infidelity (the #122/#146/overflow/off-by-one classes, `exec-tv.md` E1–E4).
//! The reference side is `exec_encode::exec_ref_value` (the BOUNDED exec value,
//! REQ-1); the production side is the verbatim `p_production`
//! (`thermite_lower::lower_exec_expr`, the artifact under test).
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-2 (exec-fn-wrapped equivalence obligation + discharge) | SHIPPED | `pub fn exec_equivalence_obligation` + `pub struct ExecObligationFrame`/`ExecParamDecl` here; non-test consumer `thermite_tv::lib` re-export → `tests/exec_teeth.rs` discharges E1–E4 through real verus (faithful VERIFIES, infidel CAUGHT). Emits the EXEC-FN form (`exec-tv.md` Architecture), discharged through the existing verus path. |

use thermite_syntax::ast::{Block, Expr};

use crate::exec_encode::{exec_ref_value, ExecRefCtx, RefEncodeError as ExecRefEncodeError};
use crate::exec_stmt_encode::{body_ref_state_ensures, BodyRefCtx};
use crate::ref_encode::{ref_contract_pred, RefCtx, RefEncodeError};

/// One obligation parameter declaration: a Verus `name: type` binding for a
/// clause free var (REQ-2). The clause's referenced slice/scalar params, plus
/// `result` (when the return is non-unit) and each `old(x)` value (bound as a
/// distinct `old_x` param), are declared here. The `type_str` is the Verus
/// spelling (`u64`, `Seq<u32>`, `int`, …). A param declared as `Seq<_>` should
/// also be named in [`ObligationFrame::seq_params`] so the reference encoder
/// treats its `@`-view as the identity (THE COERCION FIX).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamDecl {
    /// The parameter name as it appears in the obligation signature and the
    /// predicate text.
    pub name: String,
    /// The Verus type spelling (`u64` / `Seq<u32>` / `int` / …).
    pub type_str: String,
}

impl ParamDecl {
    /// Construct a parameter declaration.
    pub fn new(name: impl Into<String>, type_str: impl Into<String>) -> Self {
        ParamDecl {
            name: name.into(),
            type_str: type_str.into(),
        }
    }
}

/// The frame carrying everything the self-contained obligation program needs
/// besides the two predicates (REQ-2): the in-scope spec-fn / combinator
/// `verus_l3` definitions, the parameter declarations for the clause's free vars
/// (+ `result`/`old(_)`), an optional enclosing `requires`, and the set of
/// params bound directly as a `Seq<_>` view (so the reference encoder matches the
/// faithful `@`-coercion shape).
#[derive(Debug, Clone, Default)]
pub struct ObligationFrame {
    /// The Verus `spec fn` / combinator `verus_l3` definitions the clause depends
    /// on, emitted verbatim into the `verus! { … }` frame BEFORE `tv_check`. For
    /// a combinator clause these come from `thermite_spec::lookup(name).verus_l3`
    /// (the shared frozen ground truth); for a spec-fn clause, the spec fn's def.
    pub spec_defs: Vec<String>,
    /// The obligation parameter declarations (the clause free vars + `result` +
    /// `old(_)` values), in signature order.
    pub params: Vec<ParamDecl>,
    /// The optional enclosing `requires` predicate (the clause's enclosing `req`,
    /// or a well-formedness precondition such as `s.len() >= 1` for an index).
    /// `None` emits no `requires`.
    pub req: Option<String>,
    /// The names of params bound directly as a `Seq<_>` view — their slice→`@`
    /// rewrite is the identity in the reference encoder (THE COERCION FIX, so a
    /// faithful `spec_sum(xs)` is NOT spuriously encoded as `spec_sum(xs@)`).
    pub seq_params: Vec<String>,
    /// The names of bounded-int params (`result`, an `old_acc`, …) that must be
    /// coerced `as nat` when compared against a `nat`-valued spec-fn call — the
    /// declarative `lower_nat_equality` re-implementation (THE COERCION FIX, the
    /// doc author's #1 flagged risk). For F1 this is `["result"]` so the
    /// reference encodes `result as nat == spec_sum(xs)` (matching the faithful
    /// column) and the faithful obligation VERIFIES rather than failing on a
    /// spurious coercion mismatch.
    pub nat_coerce_params: Vec<String>,
    /// The names of params bound as the `String` wrapper (`&TString`/`TString`) —
    /// a `String`/`&String` param whose spec-position byte-view dispatches to the
    /// wrapper SPEC fns (`.spec_len()`/`.spec_byte_at(i as int)`), NOT a `Seq<u8>`
    /// index (#150 gap #2). The reference encoder reads this set (via
    /// [`RefCtx::with_string_bound`]) so a `String`-param `s.byte_at(0)` encodes to
    /// `s.spec_byte_at(0)`, MATCHING production's `recv_is_string` rewrite under the
    /// same `&TString` binding.
    pub string_params: Vec<String>,
    /// The names of params bound as the `Map` wrapper (`TMap…`) — a `Map<K,V>`
    /// param/result whose spec-position membership accessor dispatches to the
    /// wrapper SPEC fn (`.contains_key(k)`→`.spec_contains_key(k)`), MATCHING
    /// production (#150 gap #3). Read by the reference encoder via
    /// [`RefCtx::with_map_bound`].
    pub map_params: Vec<String>,
}

impl ObligationFrame {
    /// Build the [`RefCtx`] the reference encoder uses for this frame: the
    /// `seq_params` are the names whose `@`-view is the identity; the
    /// `string_params` are the `&TString`-bound names whose byte-view dispatches to
    /// the wrapper spec fns (#150 gap #2); the `map_params` are the `TMap`-bound
    /// names whose membership accessor dispatches to the wrapper spec fn (#150 gap
    /// #3).
    fn ref_ctx(&self) -> RefCtx {
        RefCtx::with_seq_bound(self.seq_params.iter().cloned())
            .with_nat_coerce(self.nat_coerce_params.iter().cloned())
            .with_string_bound(self.string_params.iter().cloned())
            .with_map_bound(self.map_params.iter().cloned())
    }

    /// The Verus parameter list `name: type, …`.
    fn param_list(&self) -> String {
        self.params
            .iter()
            .map(|p| format!("{}: {}", p.name, p.type_str))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Build the self-contained Verus equivalence obligation for one contract clause
/// (REQ-2). `source` is the clause's parsed [`Expr`] (encoded independently to
/// the reference predicate via [`ref_contract_pred`]); `p_production` is the
/// VERBATIM production-lowered predicate text (the artifact under test); `frame`
/// carries the spec-fn / combinator defs, the param decls, and the optional
/// `requires`.
///
/// Returns the obligation PROGRAM TEXT (`thermite-tv` does not run verus — the
/// teeth-test / forge plug-in discharge it). Returns [`RefEncodeError`] if the
/// source clause is outside the frozen contract sublanguage (an honest error,
/// never a panic / silent wrong encoding).
pub fn equivalence_obligation(
    source: &Expr,
    p_production: &str,
    frame: &ObligationFrame,
) -> Result<String, RefEncodeError> {
    let p_reference = ref_contract_pred(source, &frame.ref_ctx())?;

    let mut out = String::new();
    out.push_str("use vstd::prelude::*;\n");
    out.push_str("verus! {\n");

    for def in &frame.spec_defs {
        out.push('\n');
        out.push_str(def);
        out.push('\n');
    }

    out.push_str("\nproof fn tv_check(");
    out.push_str(&frame.param_list());
    out.push(')');
    if let Some(req) = &frame.req {
        out.push_str("\n    requires ");
        out.push_str(req);
        out.push(',');
    }
    out.push_str("\n{\n");
    // The obligation: the production predicate is logically equivalent to the
    // INDEPENDENT reference encoding for all inputs. VERIFIED ⟺ faithful; a
    // counterexample ⟺ infidelity. Both sides are parenthesized so the `<==>`
    // binds the whole predicates (no precedence surprise).
    out.push_str(&format!(
        "    assert(({p_production}) <==> ({p_reference}));\n"
    ));
    out.push_str("}\n");

    out.push_str("\n}\nfn main() {}\n");
    Ok(out)
}

/// One exec-obligation parameter declaration: a Verus `name: type` binding for a
/// body-position expr's free var (REQ-2). The `type_str` is the EXEC value-type
/// spelling — the BOUNDED `u64`/`u32`/`usize`/`bool` or a slice `&[u32]` (NEVER
/// `nat`/`int`: the exec obligation reasons at the production VALUE TYPE so an
/// overflow/wrapping infidelity is caught, not coerced away — `exec-tv.md` the
/// EXEC-value-semantics concern). A param declared as a slice (`&[u32]`) should
/// also be named in [`ExecObligationFrame::slice_params`] so the exec reference
/// encoder indexes it as the spec-view element value (`xs[i as int]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecParamDecl {
    /// The parameter name as it appears in the obligation signature and the body.
    pub name: String,
    /// The Verus EXEC value-type spelling (`u64` / `usize` / `&[u32]` / `bool`).
    pub type_str: String,
}

impl ExecParamDecl {
    /// Construct an exec-obligation parameter declaration.
    pub fn new(name: impl Into<String>, type_str: impl Into<String>) -> Self {
        ExecParamDecl {
            name: name.into(),
            type_str: type_str.into(),
        }
    }
}

/// The frame carrying everything the self-contained EXEC-FN obligation program
/// needs besides the production body + the reference (REQ-2): the param
/// declarations for the body expr's free vars (at their EXEC types), the return
/// type (the exec value's type), an optional enclosing `requires` (the expr's
/// well-formedness frame — `n >= 1`, `i < xs.len()`, `a + b <= 0xFFFF`), and the
/// set of params bound as a slice (`&[T]`) so the exec reference encoder indexes
/// them as the spec-view element value.
///
/// This is the EXEC dual of [`ObligationFrame`] (which frames the CONTRACT
/// predicate obligation). It deliberately carries NO `nat_coerce`/`@`-view sets —
/// the exec obligation is bounded-typed.
#[derive(Debug, Clone, Default)]
pub struct ExecObligationFrame {
    /// The Verus `spec fn` definitions the body / its `requires` depend on,
    /// emitted verbatim into the `verus! { … }` frame BEFORE `tv_exec_wrap`.
    /// Usually EMPTY for a pure scalar exec expr (the common case is scalar
    /// arithmetic with no spec-fn dependency).
    pub spec_defs: Vec<String>,
    /// The obligation parameter declarations (the body expr free vars), in
    /// signature order, at their EXEC value types.
    pub params: Vec<ExecParamDecl>,
    /// The return type spelling (the exec value's type — `u8`/`u32`/`u64`/`usize`/
    /// `bool`). This is the cast TARGET for a top-level cast expr, the comparison
    /// `bool` for a comparison, or the operand type for arithmetic.
    pub ret_type: String,
    /// The optional enclosing `requires` predicate (the body expr's well-formedness
    /// frame — `n >= 1`, `i < xs.len()`, `a + b <= 0xFFFF`). `None` emits no
    /// `requires`. The requires is emitted VERBATIM (it is the obligation's own
    /// precondition, authored from the source's `req`/index-bound, not lowered
    /// here — `exec-tv.md` REQ-2).
    pub req: Option<String>,
    /// The names of params bound as a slice (`&[T]`) — their index encodes to the
    /// spec-view element value (`xs[i as int]`) in the exec reference encoder
    /// (`exec-tv.md` AC-5). Read by [`ExecRefCtx::with_slice_bound`].
    pub slice_params: Vec<String>,
}

impl ExecObligationFrame {
    /// Build the [`ExecRefCtx`] the exec reference encoder uses for this frame: the
    /// `slice_params` are the names indexed as the spec-view element value.
    fn exec_ref_ctx(&self) -> ExecRefCtx {
        ExecRefCtx::with_slice_bound(self.slice_params.iter().cloned())
    }

    /// The Verus parameter list `name: type, …`.
    fn param_list(&self) -> String {
        self.params
            .iter()
            .map(|p| format!("{}: {}", p.name, p.type_str))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Build the self-contained Verus EXEC-FN equivalence obligation for one
/// body-position exec expr (REQ-2; `.design/verified/exec-tv.md`). `source` is the
/// body expr's parsed [`Expr`] (encoded INDEPENDENTLY to the exec reference VALUE
/// via [`exec_ref_value`]); `p_production` is the VERBATIM production exec-lowered
/// expression text (the artifact under test — `thermite_lower::lower_exec_expr`);
/// `frame` carries the param decls (at EXEC types), the return type, the optional
/// `requires`, and the slice-param set.
///
/// The emitted shape is NOT the contract `proof fn { assert(_ <==> _); }` form (an
/// exec value is not a predicate) but the EXEC-FN form (`exec-tv.md` REQ-2 /
/// Architecture):
///
/// ```text
/// use vstd::prelude::*;
/// verus! {
///     <frame.spec_defs>
///     fn tv_exec_wrap(<frame.params>) -> (result: <frame.ret_type>)
///         requires <frame.req>,
///         ensures result == <exec_ref_value(source)>,
///     {
///         <p_production>
///     }
/// }
/// fn main() {}
/// ```
///
/// Verus reasons about the exec fn's VALUE through its `ensures`. VERIFIED
/// (`verified: 1, errors: 0`) ⟺ the production exec-lowering computes the
/// reference VALUE for ALL inputs ⟺ faithful. A `postcondition not satisfied`
/// (production typechecks but computes the wrong value — the E3 wrapping case, the
/// E4 off-by-one), an `E0308`/type error (the #122 paren-drop makes the production
/// ill-typed, E1), or a parse error (the #146 cast-`<` mis-parse, E2) ⟺
/// infidelity. The always-active runtime overflow checks are LIVE (it is an EXEC
/// `fn`, not a `proof fn`), so an overflow infidelity raises the obligation
/// (`exec-tv.md` AC-4) — the structural reason the obligation is an exec fn.
///
/// Returns the obligation PROGRAM TEXT (`thermite-tv` does not run verus — the
/// teeth-test / forge plug-in discharge it). Returns [`ExecRefEncodeError`] if the
/// source body expr is outside the pure-exec subset (an honest error, never a
/// panic / silent wrong encoding).
pub fn exec_equivalence_obligation(
    source: &Expr,
    p_production: &str,
    frame: &ExecObligationFrame,
) -> Result<String, ExecRefEncodeError> {
    let reference = exec_ref_value(source, &frame.exec_ref_ctx())?;

    let mut out = String::new();
    out.push_str("use vstd::prelude::*;\n");
    out.push_str("verus! {\n");

    for def in &frame.spec_defs {
        out.push('\n');
        out.push_str(def);
        out.push('\n');
    }

    out.push_str("\nfn tv_exec_wrap(");
    out.push_str(&frame.param_list());
    out.push_str(") -> (result: ");
    out.push_str(&frame.ret_type);
    out.push(')');
    if let Some(req) = &frame.req {
        out.push_str("\n    requires ");
        out.push_str(req);
        out.push(',');
    }
    // The obligation: the production exec value EQUALS the INDEPENDENT exec
    // reference VALUE for all inputs (Z3), at the BOUNDED production type. VERIFIED
    // ⟺ faithful; a postcondition counterexample / type / parse error ⟺ infidelity.
    out.push_str("\n    ensures result == ");
    out.push_str(&reference);
    out.push_str(",\n{\n    ");
    out.push_str(p_production);
    out.push_str("\n}\n");

    out.push_str("\n}\nfn main() {}\n");
    Ok(out)
}

/// One BODY-obligation parameter declaration: a Verus `name: type` binding for a
/// straight-line body's free var (REQ-3). The `type_str` is the EXEC value-type
/// spelling — the BOUNDED `u64`/`u32`/`usize`/`bool` or a slice `&[u32]` (NEVER
/// `nat`/`int`: the body obligation reasons at the production VALUE TYPE so an
/// overflow / wrong-state infidelity is caught, not coerced away). Identical in
/// SHAPE to [`ExecParamDecl`] (the per-expr param); a distinct type keeps the body
/// obligation's surface self-documenting (a body input vs a single-expr input).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyParamDecl {
    /// The parameter name as it appears in the obligation signature and the body.
    pub name: String,
    /// The Verus EXEC value-type spelling (`u64` / `usize` / `&[u32]` / `bool`).
    pub type_str: String,
}

impl BodyParamDecl {
    /// Construct a body-obligation parameter declaration.
    pub fn new(name: impl Into<String>, type_str: impl Into<String>) -> Self {
        BodyParamDecl {
            name: name.into(),
            type_str: type_str.into(),
        }
    }
}

/// The frame carrying everything the self-contained BODY-state-refinement obligation
/// program needs besides the production body + the reference state-denotation
/// (REQ-3; `.design/verified/exec-stmt-tv.md`): the param declarations for the
/// body's free vars (at their EXEC types), the RESULT type (the final-state
/// projection's type — a scalar `u64`/`bool` for a single-cell body, a tuple
/// `(u64, u64)` for a multi-cell body), an optional enclosing `requires` (the body's
/// well-formedness / no-overflow frame — `x <= 1000`), and the slice-param set.
///
/// This is the BODY analogue of [`ExecObligationFrame`] (which frames a single exec
/// EXPRESSION's value obligation): where the exec frame's obligation compares one
/// VALUE, the body frame's obligation compares the body's FINAL STATE (the
/// `body_ref_state` denotation). It deliberately carries NO `nat`-coerce set — the
/// body obligation is bounded-typed (the same as step 2.1).
#[derive(Debug, Clone, Default)]
pub struct BodyObligationFrame {
    /// The Verus `spec fn` definitions the body / its `requires` depend on, emitted
    /// verbatim into the `verus! { ... }` frame BEFORE `tv_body_wrap`. Usually EMPTY
    /// for a scalar straight-line body (B1-B4 carry none).
    pub spec_defs: Vec<String>,
    /// The obligation parameter declarations (the body's free vars), in signature
    /// order, at their EXEC value types.
    pub params: Vec<BodyParamDecl>,
    /// The RESULT type spelling — the body's final-state projection type: a scalar
    /// (`u64`/`bool`/`usize`) for a single-cell body (B1/B2/B3), a tuple
    /// (`(u64, u64)`) for a multi-cell body (B4).
    pub ret_type: String,
    /// The optional enclosing `requires` predicate (the body's well-formedness /
    /// no-overflow frame — `x <= 1000`). `None` emits no `requires`. Emitted
    /// VERBATIM (the obligation's own precondition, authored from the source frame,
    /// not lowered here — `exec-stmt-tv.md` REQ-3).
    pub req: Option<String>,
    /// The names of params bound as a slice (`&[T]`) — their index in any RHS / tail
    /// encodes to the spec-view element value (`xs[i as int]`) in the reference
    /// state-denotation. Read by [`BodyRefCtx::with_slice_bound`].
    pub slice_params: Vec<String>,
}

impl BodyObligationFrame {
    /// Build the [`BodyRefCtx`] the reference state-denotation uses for this frame:
    /// the `slice_params` are the names indexed as the spec-view element value.
    fn body_ref_ctx(&self) -> BodyRefCtx {
        BodyRefCtx::with_slice_bound(self.slice_params.iter().cloned())
    }

    /// The Verus parameter list `name: type, ...`.
    fn param_list(&self) -> String {
        self.params
            .iter()
            .map(|p| format!("{}: {}", p.name, p.type_str))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Build the self-contained Verus BODY-state-refinement obligation for one
/// straight-line exec body (REQ-3; `.design/verified/exec-stmt-tv.md`). `body` is
/// the source straight-line [`Block`] (encoded INDEPENDENTLY to the reference FINAL
/// STATE via [`body_ref_state`]); `p_production` is the VERBATIM production
/// body-lowered text (the artifact under test — `thermite_lower::lower_exec_body`);
/// `frame` carries the param decls (at EXEC types), the result type, the optional
/// `requires`, and the slice-param set.
///
/// This is the STATE analogue of [`exec_equivalence_obligation`] (which compares a
/// single VALUE): the emitted shape wraps the production body as the BODY of an exec
/// fn whose `ensures` compares the fn RESULT (the body's final-state projection — the
/// tail value a single-exit straight-line body returns) to the reference
/// state-denotation:
///
/// ```text
/// use vstd::prelude::*;
/// verus! {
///     <frame.spec_defs>
///     fn tv_body_wrap(<frame.params>) -> (result: <frame.ret_type>)
///         requires <frame.req>,
///         ensures result == <body_ref_state(body)>,
///     {
///         <p_production>
///     }
/// }
/// fn main() {}
/// ```
///
/// VERIFIED (`verified: 1, errors: 0`) <=> the production body's STATE
/// TRANSFORMATION produces the reference FINAL STATE for ALL inputs (Z3) <=>
/// faithful. A `postcondition not satisfied` counterexample <=> a
/// state-transformation infidelity — a DROPPED statement, a REORDERED mutation, a
/// SWAPPED `if`-branch (each changes the final state while every sub-expression stays
/// value-faithful — the STATE-SEQUENCING teeth that per-EXPRESSION step-2.1 TV cannot
/// see). The production body is an EXEC `fn` (not `proof`/`spec`), so the
/// always-active runtime overflow checks are LIVE (the same structural reason the
/// step-2.1 obligation is an exec fn).
///
/// Returns the obligation PROGRAM TEXT (`thermite-tv` does not run verus — the
/// teeth-test / the future forge plug-in discharge it). Returns
/// [`ExecRefEncodeError`] if the source body is outside the frozen straight-line
/// subset (a loop / mid-branch early return / non-scalar mutation / re-shadow — an
/// honest error, never a panic / silent wrong encoding).
pub fn body_equivalence_obligation(
    body: &Block,
    p_production: &str,
    frame: &BodyObligationFrame,
) -> Result<String, ExecRefEncodeError> {
    let ensures_pred = body_ref_state_ensures(body, "result", &frame.body_ref_ctx())?;

    let mut out = String::new();
    out.push_str("use vstd::prelude::*;\n");
    out.push_str("verus! {\n");

    for def in &frame.spec_defs {
        out.push('\n');
        out.push_str(def);
        out.push('\n');
    }

    out.push_str("\nfn tv_body_wrap(");
    out.push_str(&frame.param_list());
    out.push_str(") -> (result: ");
    out.push_str(&frame.ret_type);
    out.push(')');
    if let Some(req) = &frame.req {
        out.push_str("\n    requires ");
        out.push_str(req);
        out.push(',');
    }
    // The obligation: the production body's RESULT (its final-state projection) EQUALS
    // the INDEPENDENT reference FINAL STATE for all inputs (Z3), at the BOUNDED
    // production type. For a single-cell body this is `result == <ref>`; for a
    // multi-cell tuple body it is the per-projection conjunction `result.0 == <c0> &&
    // result.1 == <c1>` (Verus has no SpecEq on a `(u64,u64)` vs a `(int,int)` tuple
    // literal — the per-projection compare is element-wise at the bounded type, B4).
    // VERIFIED <=> faithful state transformation; a postcondition counterexample <=>
    // a state-transformation infidelity (dropped stmt / reordered mutation / swapped
    // branch / wrong cell).
    out.push_str("\n    ensures ");
    out.push_str(&ensures_pred);
    out.push_str(",\n{\n");
    out.push_str(p_production);
    out.push_str("}\n");

    out.push_str("\n}\nfn main() {}\n");
    Ok(out)
}
