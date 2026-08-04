//! The per-clause Z3 equivalence obligation builder
//! (`.design/verified/contract-tv.md` REQ-2; `thermite-design.md` §6, L3 = the
//! verus-derived SMT proof).
//!
//! [`equivalence_obligation`] emits a self-contained Verus program text whose
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
//! Verified iff the production predicate is logically equivalent to the reference
//! for all inputs (Z3), i.e. faithful. A counterexample (a concrete input on which
//! they differ) is infidelity: a witness of the lowering bug
//! (`thermite-design.md` §5.1 "counterexamples, not adjectives").
//!
//! `thermite-tv` does not run verus itself: it emits the obligation text. The
//! negative test (`tests/teeth.rs`, REQ-4) and the future forge plug-in (REQ-5,
//! `forge/src/contract_tv.rs`) discharge it through the existing
//! `forge::check::run_verus` path.
//!
//! ## REQ status
//!
//! <!-- generated:reqs view=thermite-tv-obligation-contract-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-TV-CONTRACT-OBLIGATION | shipped | `thermite-tv/src/obligation.rs` | Contract-TV per-clause Z3 equivalence obligation |  |
//! <!-- /generated:reqs -->
//!
//! ## exec-position extension (`.design/verified/exec-tv.md` REQ-2; epic #151)
//!
//! [`exec_equivalence_obligation`] is the exec dual: it emits the exec-fn-wrapped
//! `fn tv_exec_wrap(..) requires <req>, ensures result == <exec_ref_value(source)>
//! { <p_production> }` form, not the proof-fn `<==>` (an exec value is not a
//! predicate). Verus reasons about the exec fn's value through its `ensures`:
//! verified iff faithful; a `postcondition not satisfied` / `E0308` / parse error
//! is infidelity (the #122/#146/overflow/off-by-one classes, `exec-tv.md` E1–E4).
//! The reference side is `exec_encode::exec_ref_value` (the bounded exec value,
//! REQ-1); the production side is the verbatim `p_production`
//! (`thermite_lower::lower_exec_expr`, the artifact under test).
//!
//! <!-- generated:reqs view=thermite-tv-obligation-exec-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-TV-EXEC-OBLIGATION | shipped | `thermite-tv/src/obligation.rs` | Exec-TV fn-wrapped equivalence obligation |  |
//! <!-- /generated:reqs -->
//!
//! ## LOOP-position extension — step 2.2.2-i (`.design/verified/loop-tv.md`; epic #169)
//!
//! [`loop_entry_obligation`] / [`loop_preservation_obligation`] /
//! [`loop_exit_obligation`] are the three per-run loop obligations (`loop-tv.md`
//! REQ-2), siblings to [`body_equivalence_obligation`]. They consume
//! [`crate::exec_stmt_encode::loop_ref_obligations`] (the v1-frozen-subset recognizer +
//! the three reference pieces) and emit the self-contained Verus units the existing
//! `forge::check::run_verus` discharges: entry (`proof fn` asserting `inv` on the
//! pre-loop entry state, where a wrong pre-loop init fails), preservation (`fn` with
//! `requires inv && cond`, `ensures result.i == <body_ref_state step_i> && inv_at_step`,
//! where a per-iteration body infidelity or a broken invariant fails `postcondition not
//! satisfied`), and exit (`proof fn` with `requires inv && !cond` asserting the claimed
//! after-loop characterization, where an over-claim stronger than `inv ∧ ¬cond` fails).
//! The single-iteration step reuses the shipped `body_ref_state` (no new body machinery,
//! AC-5); a loop out of v1 is an `Unsupported` from `loop_ref_obligations`
//! (Skipped, never silently Faithful, R-HONEST-3).
//!
//! <!-- generated:reqs view=thermite-tv-obligation-loop-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-TV-LOOP-OBLIGATIONS | shipped | `thermite-tv/src/obligation.rs` | Loop-TV per-run obligation emitters |  |
//! <!-- /generated:reqs -->

use thermite_syntax::ast::{Block, Expr};

use crate::exec_encode::{exec_ref_value, ExecRefCtx, RefEncodeError as ExecRefEncodeError};
use crate::exec_stmt_encode::{
    body_ref_state_ensures, loop_ref_obligations, negate_condition, BodyRefCtx,
};
pub use crate::ref_encode::StateViewKind;
use crate::ref_encode::{ref_contract_pred, RefCtx, RefEncodeError};

/// One obligation parameter declaration: a Verus `name: type` binding for a
/// clause free var (REQ-2). The clause's referenced slice/scalar params, plus
/// `result` (when the return is non-unit) and each `old(x)` value (bound as a
/// distinct `old_x` param), are declared here. The `type_str` is the Verus
/// spelling (`u64`, `Seq<u32>`, `int`, …). A param declared as `Seq<_>` should
/// also be named in [`ObligationFrame::seq_params`] so the reference encoder
/// treats its `@`-view as the identity (the coercion fix).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamDecl {
    /// The parameter name as it appears in the obligation signature and the
    /// predicate text.
    pub name: String,
    /// The Verus type spelling (`u64` / `Seq<u32>` / `int` / …).
    pub type_str: String,
}

/// A source state-view call reified as an ordinary, universally quantified
/// obligation parameter. Contract TV runs in a `proof fn`, where Verus's
/// `old(..)`/`final(..)` operators are not available; binding the snapshots
/// independently also checks the predicate for every possible transition rather
/// than only for the no-op transition of a synthetic executable body.
/// This declaration identifies one exact source state view and its symbolic
/// obligation binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateViewDecl {
    /// `old` or `final`.
    pub kind: StateViewKind,
    /// The source parameter named inside the state-view call.
    pub source_name: String,
    /// The fresh parameter name used in the proof obligation.
    pub binding_name: String,
    /// Whether the binding is a `Seq<T>` standing for a borrowed slice's `@`
    /// view. In that case the production `old(x)@`/`final(x)@` spelling is
    /// reified to the bare sequence binding.
    pub sequence_view: bool,
}

impl StateViewDecl {
    /// Construct one state-view binding.
    pub fn new(
        kind: StateViewKind,
        source_name: impl Into<String>,
        binding_name: impl Into<String>,
        sequence_view: bool,
    ) -> Self {
        Self {
            kind,
            source_name: source_name.into(),
            binding_name: binding_name.into(),
            sequence_view,
        }
    }
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
    /// on, emitted verbatim into the `verus! { … }` frame before `tv_check`. For
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
    /// rewrite is the identity in the reference encoder (the coercion fix, so a
    /// faithful `spec_sum(xs)` is not spuriously encoded as `spec_sum(xs@)`).
    pub seq_params: Vec<String>,
    /// The names of bounded-int params (`result`, an `old_acc`, …) that must be
    /// coerced `as nat` when compared against a `nat`-valued spec-fn call — the
    /// declarative `lower_nat_equality` re-implementation (the coercion fix, the
    /// doc author's #1 flagged risk). For F1 this is `["result"]` so the
    /// reference encodes `result as nat == spec_sum(xs)` (matching the faithful
    /// column) and the faithful obligation verifies rather than failing on a
    /// spurious coercion mismatch.
    pub nat_coerce_params: Vec<String>,
    /// The names of params bound as the `String` wrapper (`&TString`/`TString`) —
    /// a `String`/`&String` param whose spec-position byte-view dispatches to the
    /// wrapper spec fns (`.spec_len()`/`.spec_byte_at(i as int)`), not a `Seq<u8>`
    /// index (#150 gap #2). The reference encoder reads this set (via
    /// [`RefCtx::with_string_bound`]) so a `String`-param `s.byte_at(0)` encodes to
    /// `s.spec_byte_at(0)`, matching production's `recv_is_string` rewrite under the
    /// same `&TString` binding.
    pub string_params: Vec<String>,
    /// The names of params bound as the `Map` wrapper (`TMap…`) — a `Map<K,V>`
    /// param/result whose spec-position membership accessor dispatches to the
    /// wrapper spec fn (`.contains_key(k)`→`.spec_contains_key(k)`), matching
    /// production (#150 gap #3). Read by the reference encoder via
    /// [`RefCtx::with_map_bound`].
    pub map_params: Vec<String>,
    /// Native fixed-array parameters/results whose specification meaning exposes
    /// the finite `@` view for the independently encoded length operation.
    pub fixed_array_params: Vec<String>,
    /// User `spec fn` names and zero-based argument positions declared as slice
    /// views. Named calls apply `@` only at these positions.
    pub spec_call_slice_args: Vec<(String, Vec<usize>)>,
    /// Arbitrary pre/post-state snapshots used to reify `old(..)` and
    /// `final(..)` outside an executable Verus postcondition. The production
    /// predicate is rewritten only at exact emitted state-view calls; the
    /// independent reference encoder resolves the source AST through the same
    /// symbolic bindings.
    pub state_views: Vec<StateViewDecl>,
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
            .with_fixed_array_bound(self.fixed_array_params.iter().cloned())
            .with_spec_call_slice_args(self.spec_call_slice_args.clone())
            .with_state_views(self.state_views.iter().map(|view| {
                (
                    view.kind,
                    view.source_name.clone(),
                    view.binding_name.clone(),
                )
            }))
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
/// verbatim production-lowered predicate text (the artifact under test); `frame`
/// carries the spec-fn / combinator defs, the param decls, and the optional
/// `requires`.
///
/// Returns the obligation program text (`thermite-tv` does not run verus — the
/// negative test and forge plug-in discharge it). Returns [`RefEncodeError`] if the
/// State-view calls in that exact production text are reified to the arbitrary
/// snapshot bindings declared by the frame; all other production text remains
/// unchanged. Returns [`RefEncodeError`] if the source clause is outside the frozen contract sublanguage (an error,
/// never a panic / silent wrong encoding).
pub fn equivalence_obligation(
    source: &Expr,
    p_production: &str,
    frame: &ObligationFrame,
) -> Result<String, RefEncodeError> {
    let p_reference = ref_contract_pred(source, &frame.ref_ctx())?;
    let p_production = reify_production_state_views(p_production, &frame.state_views);

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
    // independent reference encoding for all inputs. Verified iff faithful; a
    // counterexample is infidelity. Both sides are parenthesized so the `<==>`
    // binds the whole predicates (no precedence surprise).
    out.push_str(&format!(
        "    assert(({p_production}) <==> ({p_reference}));\n"
    ));
    out.push_str("}\n");

    out.push_str("\n}\nfn main() {}\n");
    Ok(out)
}

/// Replace only the canonical state-view calls emitted by the production
/// lowerer with their arbitrary snapshot parameters. A wrong operator, argument,
/// or view shape therefore remains different (or ill-typed) in the obligation.
fn reify_production_state_views(production: &str, views: &[StateViewDecl]) -> String {
    let mut reified = production.to_string();
    for view in views {
        let operator = match view.kind {
            StateViewKind::Old => "old",
            StateViewKind::Final => "final",
        };
        let call = format!("{operator}({})", view.source_name);
        if view.sequence_view {
            reified = reified.replace(&format!("{call}@"), &view.binding_name);
        }
        reified = reified.replace(&call, &view.binding_name);
    }
    reified
}

/// One exec-obligation parameter declaration: a Verus `name: type` binding for a
/// body-position expr's free var (REQ-2). The `type_str` is the exec value-type
/// spelling — the bounded `u64`/`u32`/`usize`/`bool` or a slice `&[u32]` (never
/// `nat`/`int`: the exec obligation reasons at the production value type so an
/// overflow/wrapping infidelity is caught, not coerced away — `exec-tv.md` the
/// exec-value-semantics concern). A param declared as a slice (`&[u32]`) should
/// also be named in [`ExecObligationFrame::slice_params`] so the exec reference
/// encoder indexes it as the spec-view element value (`xs[i as int]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecParamDecl {
    /// The parameter name as it appears in the obligation signature and the body.
    pub name: String,
    /// The Verus exec value-type spelling (`u64` / `usize` / `&[u32]` / `bool`).
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

/// The frame carrying everything the self-contained exec-fn obligation program
/// needs besides the production body + the reference (REQ-2): the param
/// declarations for the body expr's free vars (at their exec types), the return
/// type (the exec value's type), an optional enclosing `requires` (the expr's
/// well-formedness frame — `n >= 1`, `i < xs.len()`, `a + b <= 0xFFFF`), and the
/// set of params bound as a slice (`&[T]`) so the exec reference encoder indexes
/// them as the spec-view element value.
///
/// This is the exec dual of [`ObligationFrame`] (which frames the contract
/// predicate obligation). It carries no `nat_coerce`/`@`-view sets: the exec
/// obligation is bounded-typed.
#[derive(Debug, Clone, Default)]
pub struct ExecObligationFrame {
    /// The Verus `spec fn` definitions the body / its `requires` depend on,
    /// emitted verbatim into the `verus! { … }` frame before `tv_exec_wrap`.
    /// Usually empty for a pure scalar exec expr (the common case is scalar
    /// arithmetic with no spec-fn dependency).
    pub spec_defs: Vec<String>,
    /// The obligation parameter declarations (the body expr free vars), in
    /// signature order, at their exec value types.
    pub params: Vec<ExecParamDecl>,
    /// The return type spelling (the exec value's type — `u8`/`u32`/`u64`/`usize`/
    /// `bool`). This is the cast target for a top-level cast expr, the comparison
    /// `bool` for a comparison, or the operand type for arithmetic.
    pub ret_type: String,
    /// The optional enclosing `requires` predicate (the body expr's well-formedness
    /// frame — `n >= 1`, `i < xs.len()`, `a + b <= 0xFFFF`). `None` emits no
    /// `requires`. The requires is emitted verbatim (it is the obligation's own
    /// precondition, authored from the source's `req`/index-bound, not lowered
    /// here — `exec-tv.md` REQ-2).
    pub req: Option<String>,
    /// The names of params bound as a slice (`&[T]`) — their index encodes to the
    /// spec-view element value (`xs[i as int]`) in the exec reference encoder
    /// (`exec-tv.md` AC-5). Read by [`ExecRefCtx::with_slice_bound`].
    pub slice_params: Vec<String>,
    /// Native fixed-array parameters, indexed through their finite `@` views.
    pub fixed_array_params: Vec<String>,
    /// Whether the result is a native fixed array and therefore compared
    /// extensionally through its `@` view.
    pub result_is_fixed_array: bool,
}

impl ExecObligationFrame {
    /// Build the [`ExecRefCtx`] the exec reference encoder uses for this frame: the
    /// `slice_params` are the names indexed as the spec-view element value.
    fn exec_ref_ctx(&self) -> ExecRefCtx {
        ExecRefCtx::with_slice_bound(self.slice_params.iter().cloned())
            .with_fixed_array_bound(self.fixed_array_params.iter().cloned())
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

/// Build the self-contained Verus exec-fn equivalence obligation for one
/// body-position exec expr (REQ-2; `.design/verified/exec-tv.md`). `source` is the
/// body expr's parsed [`Expr`] (encoded independently to the exec reference value
/// via [`exec_ref_value`]); `p_production` is the verbatim production exec-lowered
/// expression text (the artifact under test — `thermite_lower::lower_exec_expr`);
/// `frame` carries the param decls (at exec types), the return type, the optional
/// `requires`, and the slice-param set.
///
/// The emitted shape is the exec-fn form (`exec-tv.md` REQ-2 / Architecture), not
/// the contract `proof fn { assert(_ <==> _); }` form (an exec value is not a
/// predicate):
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
/// Verus reasons about the exec fn's value through its `ensures`. Verified
/// (`verified: 1, errors: 0`) iff the production exec-lowering computes the
/// reference value for all inputs, i.e. faithful. A `postcondition not satisfied`
/// (production typechecks but computes the wrong value — the E3 wrapping case, the
/// E4 off-by-one), an `E0308`/type error (the #122 paren-drop makes the production
/// ill-typed, E1), or a parse error (the #146 cast-`<` mis-parse, E2) is
/// infidelity. The always-active runtime overflow checks are live (it is an exec
/// `fn`, not a `proof fn`), so an overflow infidelity raises the obligation
/// (`exec-tv.md` AC-4): the structural reason the obligation is an exec fn.
///
/// Returns the obligation program text (`thermite-tv` does not run verus — the
/// negative test and forge plug-in discharge it). Returns [`ExecRefEncodeError`] if the
/// source body expr is outside the pure-exec subset (an error, never a
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
    // The obligation: the production exec value equals the independent exec
    // reference value for all inputs (Z3), at the bounded production type. Verified
    // iff faithful; a postcondition counterexample / type / parse error is infidelity.
    if frame.result_is_fixed_array {
        out.push_str("\n    ensures result@ == (");
        out.push_str(&reference);
        out.push_str(")@");
    } else {
        out.push_str("\n    ensures result == ");
        out.push_str(&reference);
    }
    out.push_str(",\n{\n    ");
    out.push_str(p_production);
    out.push_str("\n}\n");

    out.push_str("\n}\nfn main() {}\n");
    Ok(out)
}

/// One body-obligation parameter declaration: a Verus `name: type` binding for a
/// straight-line body's free var (REQ-3). The `type_str` is the exec value-type
/// spelling — the bounded `u64`/`u32`/`usize`/`bool` or a slice `&[u32]` (never
/// `nat`/`int`: the body obligation reasons at the production value type so an
/// overflow / wrong-state infidelity is caught, not coerced away). Identical in
/// shape to [`ExecParamDecl`] (the per-expr param); a distinct type keeps the body
/// obligation's surface self-documenting (a body input vs a single-expr input).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyParamDecl {
    /// The parameter name as it appears in the obligation signature and the body.
    pub name: String,
    /// The Verus exec value-type spelling (`u64` / `usize` / `&[u32]` / `bool`).
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

/// The frame carrying everything the self-contained body-state-refinement obligation
/// program needs besides the production body + the reference state-denotation
/// (REQ-3; `.design/verified/exec-stmt-tv.md`): the param declarations for the
/// body's free vars (at their exec types), the result type (the final-state
/// projection's type — a scalar `u64`/`bool` for a single-cell body, a tuple
/// `(u64, u64)` for a multi-cell body), an optional enclosing `requires` (the body's
/// well-formedness / no-overflow frame — `x <= 1000`), and the slice-param set.
///
/// This is the body analogue of [`ExecObligationFrame`] (which frames a single exec
/// expression's value obligation): where the exec frame's obligation compares one
/// value, the body frame's obligation compares the body's final state (the
/// `body_ref_state` denotation). It carries no `nat`-coerce set: the body obligation
/// is bounded-typed (the same as step 2.1).
#[derive(Debug, Clone, Default)]
pub struct BodyObligationFrame {
    /// The Verus `spec fn` definitions the body / its `requires` depend on, emitted
    /// verbatim into the `verus! { ... }` frame before `tv_body_wrap`. Usually empty
    /// for a scalar straight-line body (B1-B4 carry none).
    pub spec_defs: Vec<String>,
    /// The obligation parameter declarations (the body's free vars), in signature
    /// order, at their exec value types.
    pub params: Vec<BodyParamDecl>,
    /// The result type spelling — the body's final-state projection type: a scalar
    /// (`u64`/`bool`/`usize`) for a single-cell body (B1/B2/B3), a tuple
    /// (`(u64, u64)`) for a multi-cell body (B4).
    pub ret_type: String,
    /// The optional enclosing `requires` predicate (the body's well-formedness /
    /// no-overflow frame — `x <= 1000`). `None` emits no `requires`. Emitted
    /// verbatim (the obligation's own precondition, authored from the source frame,
    /// not lowered here — `exec-stmt-tv.md` REQ-3).
    pub req: Option<String>,
    /// The names of params bound as a slice (`&[T]`) — their index in any RHS / tail
    /// encodes to the spec-view element value (`xs[i as int]`) in the reference
    /// state-denotation. Read by [`BodyRefCtx::with_slice_bound`].
    pub slice_params: Vec<String>,
    /// Parameters borrowed exclusively as `&mut [T]` or `&mut [T; N]`.
    /// Body TV observes their complete post-state through `final(param)@`.
    pub mutable_indexed_params: Vec<String>,
    /// Native fixed-array parameters, indexed through their finite `@` views.
    pub fixed_array_params: Vec<String>,
    /// Whether the result type is a native fixed array. Array results are
    /// compared extensionally through `result@`.
    pub result_is_fixed_array: bool,
}

impl BodyObligationFrame {
    /// Build the [`BodyRefCtx`] the reference state-denotation uses for this frame:
    /// the `slice_params` are the names indexed as the spec-view element value.
    fn body_ref_ctx(&self) -> BodyRefCtx {
        BodyRefCtx::with_slice_bound(self.slice_params.iter().cloned())
            .with_mutable_indexed_bound(self.mutable_indexed_params.iter().cloned())
            .with_fixed_array_bound(self.fixed_array_params.iter().cloned())
            .with_fixed_array_result(self.result_is_fixed_array)
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

/// Build the self-contained Verus body-state-refinement obligation for one
/// straight-line exec body (REQ-3; `.design/verified/exec-stmt-tv.md`). `body` is
/// the source straight-line [`Block`] (encoded independently to the reference final
/// state via [`body_ref_state`]); `p_production` is the verbatim production
/// body-lowered text (the artifact under test — `thermite_lower::lower_exec_body`);
/// `frame` carries the param decls (at exec types), the result type, the optional
/// `requires`, and the slice-param set.
///
/// This is the state analogue of [`exec_equivalence_obligation`] (which compares a
/// single value): the emitted shape wraps the production body as the body of an exec
/// fn whose `ensures` compares the fn result (the body's final-state projection — the
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
/// Verified (`verified: 1, errors: 0`) iff the production body's state
/// transformation produces the reference final state for all inputs (Z3), i.e.
/// faithful. A `postcondition not satisfied` counterexample is a
/// state-transformation infidelity — a dropped statement, a reordered mutation, a
/// swapped `if`-branch (each changes the final state while every sub-expression stays
/// value-faithful, which is the state-sequencing failure that per-expression step-2.1 TV cannot
/// see). The production body is an exec `fn` (not `proof`/`spec`), so the
/// always-active runtime overflow checks are live (the same structural reason the
/// step-2.1 obligation is an exec fn).
///
/// Returns the obligation program text (`thermite-tv` does not run verus — the
/// negative test and the future forge plug-in discharge it). Returns
/// [`ExecRefEncodeError`] if the source body is outside the frozen straight-line
/// subset (a loop / mid-branch early return / non-scalar mutation / re-shadow — an
/// error, never a panic / silent wrong encoding).
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
    // The obligation: the production body's result (its final-state projection) equals
    // the independent reference final state for all inputs (Z3), at the bounded
    // production type. For a single-cell body this is `result == <ref>`; for a
    // multi-cell tuple body it is the per-projection conjunction `result.0 == <c0> &&
    // result.1 == <c1>` (Verus has no SpecEq on a `(u64,u64)` vs a `(int,int)` tuple
    // literal — the per-projection compare is element-wise at the bounded type, B4).
    // Verified iff faithful state transformation; a postcondition counterexample is
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

// =============================================================================
// Loop obligations — step 2.2.2-i (`.design/verified/loop-tv.md` REQ-2)
// =============================================================================

/// One loop-obligation parameter declaration: a Verus `name: type` binding for a
/// loop's free var — a fn input (`n: usize`, a slice `&[u32]`) or a mutated cell
/// (`lo: usize`/`hi: usize`) at its bounded exec type (never `nat`/`int`: the loop
/// obligation reasons at the production value type so an overflow / wrong-state
/// infidelity is caught, not coerced away). Identical in shape to [`BodyParamDecl`];
/// a distinct type keeps the loop obligation surface self-documenting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopParamDecl {
    /// The parameter name (as it appears in the signature and the predicates).
    pub name: String,
    /// The Verus exec value-type spelling (`usize` / `u64` / `&[u32]` / `bool`).
    pub type_str: String,
}

impl LoopParamDecl {
    /// Construct a loop-obligation parameter declaration.
    pub fn new(name: impl Into<String>, type_str: impl Into<String>) -> Self {
        LoopParamDecl {
            name: name.into(),
            type_str: type_str.into(),
        }
    }
}

/// The frame carrying everything the three per-run loop obligations need besides the
/// reference pieces (`loop-tv.md` REQ-2): the spec-fn defs, the fn input params (at
/// their exec types — the slices / scalars the entry state + the inv/cond reference),
/// the mutated cell params (at their exec types — `lo: usize`/`hi: usize`, declared in
/// the same sorted order [`crate::exec_stmt_encode::loop_ref_obligations`] uses), the
/// enclosing fn `requires` (the well-formedness frame the entry obligation discharges
/// `inv` under), and the slice-param set (so an index in the inv/cond/cell encodes to
/// the spec-view element value).
///
/// This is the loop analogue of [`BodyObligationFrame`]. The cells are distinguished
/// from the inputs because they play a structurally different role: in the entry
/// obligation the cells are substituted away (the entry-state closed form in the
/// inputs), while in the preservation + exit obligations they are free params (the
/// loop-step's arbitrary-iteration state — havocked + invariant-constrained, the
/// design's opaque-but-invariant-constrained after-loop cells).
#[derive(Debug, Clone, Default)]
pub struct LoopObligationFrame {
    /// The Verus `spec fn` defs the inv / cond / body depend on, emitted verbatim
    /// before the obligation fn. Usually empty for a scalar-comparison-invariant loop.
    pub spec_defs: Vec<String>,
    /// The fn input params (the slices / scalars the entry state + the inv/cond
    /// reference), at their exec types, in signature order. Does not include the
    /// mutated cells (those are [`Self::cells`]).
    pub inputs: Vec<LoopParamDecl>,
    /// The mutated cell params (`lo: usize`/`hi: usize`), at their exec types, in the
    /// sorted order `loop_ref_obligations` reports them (so a `result.i` projection in
    /// the preservation `ensures` lines up with `step_cells[i]`).
    pub cells: Vec<LoopParamDecl>,
    /// The enclosing fn `requires` (the well-formedness / no-overflow frame the entry
    /// obligation discharges `inv` under — `n <= 1000`). `None` emits no `requires`.
    pub req: Option<String>,
    /// The names of params bound as a slice (`&[T]`) — their index in the inv / cond /
    /// cell encodes to the spec-view element value (`xs[i as int]`). Read by
    /// [`BodyRefCtx::with_slice_bound`].
    pub slice_params: Vec<String>,
}

impl LoopObligationFrame {
    /// Build the [`BodyRefCtx`] the reference state-denotation + predicate encoder use
    /// for this frame: the `slice_params` are the names indexed as the spec-view
    /// element value.
    fn body_ref_ctx(&self) -> BodyRefCtx {
        BodyRefCtx::with_slice_bound(self.slice_params.iter().cloned())
    }

    /// The Verus parameter list for the entry obligation: the fn inputs only (the
    /// cells are substituted away into the entry-state closed form).
    fn input_param_list(&self) -> String {
        self.inputs
            .iter()
            .map(|p| format!("{}: {}", p.name, p.type_str))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// The Verus parameter list for the preservation + exit obligations: the mutated
    /// cells first (the loop-step's free state), then the fn inputs (the slices /
    /// scalars the inv/cond reference).
    fn cell_and_input_param_list(&self) -> String {
        self.cells
            .iter()
            .chain(self.inputs.iter())
            .map(|p| format!("{}: {}", p.name, p.type_str))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// The result-tuple type for the preservation step's `(cell0', cell1', …)` return:
    /// a single cell is the bare type, multiple cells a tuple `(T0, T1)`.
    fn cell_tuple_type(&self) -> String {
        if self.cells.len() == 1 {
            self.cells[0].type_str.clone()
        } else {
            format!(
                "({})",
                self.cells
                    .iter()
                    .map(|p| p.type_str.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    }
}

/// Build the entry loop obligation (`loop-tv.md` REQ-2.1): the loop is reached with
/// the pre-loop straight-line entry state; the obligation asserts the invariant holds
/// there (`entry-state ⟹ inv`). `block` is the enclosing straight-line `Block` whose
/// last statement is the v1-frozen-subset `while` loop (the prefix establishes the
/// entry state); `frame` carries the input params + the enclosing `requires`.
///
/// Emitted shape (a proof fn — the entry state is a closed form, no exec body needed):
///
/// ```text
/// use vstd::prelude::*;
/// verus! {
///     <frame.spec_defs>
///     proof fn tv_loop_entry(<frame.inputs>)
///         requires <frame.req>,
///     {
///         assert(<inv[cells := entry-state]>);
///     }
/// }
/// fn main() {}
/// ```
///
/// Verified iff the invariant holds on entry; an `assertion failed`
/// counterexample means the entry state violates the claimed invariant (a wrong
/// pre-loop initialization). Returns [`ExecRefEncodeError`] if the loop is outside
/// the v1 frozen subset (an Skipped, never a panic / silent wrong encoding).
pub fn loop_entry_obligation(
    block: &Block,
    frame: &LoopObligationFrame,
) -> Result<String, ExecRefEncodeError> {
    let obs = loop_ref_obligations(block, &frame.body_ref_ctx())?;

    let mut out = String::new();
    out.push_str("use vstd::prelude::*;\n");
    out.push_str("verus! {\n");
    for def in &frame.spec_defs {
        out.push('\n');
        out.push_str(def);
        out.push('\n');
    }
    out.push_str("\nproof fn tv_loop_entry(");
    out.push_str(&frame.input_param_list());
    out.push(')');
    if let Some(req) = &frame.req {
        out.push_str("\n    requires ");
        out.push_str(req);
        out.push(',');
    }
    // The entry obligation: the invariant (cells substituted by their pre-loop entry
    // values) holds under the enclosing `requires`. Verified iff the invariant holds
    // on entry; a counterexample means a wrong pre-loop initialization.
    out.push_str("\n{\n    assert(");
    out.push_str(&obs.entry_pred);
    out.push_str(");\n}\n");
    out.push_str("\n}\nfn main() {}\n");
    Ok(out)
}

/// Build the preservation loop obligation (`loop-tv.md` REQ-2.2): one straight-line
/// iteration of the loop body carries `inv ∧ cond` to `inv`. The single-iteration body
/// is a straight-line `Block`, so its state step reuses the shipped
/// [`crate::exec_stmt_encode::body_ref_state`] (no new body machinery, AC-5).
/// `p_production` is the verbatim production loop-body lowering shaped to mutate the
/// cell shadows and return the stepped cells as a `(cell0', cell1', …)` tuple (the
/// artifact under test); `frame` carries the cell + input params.
///
/// Emitted shape:
///
/// ```text
/// use vstd::prelude::*;
/// verus! {
///     <frame.spec_defs>
///     fn tv_loop_step(<cells>, <inputs>) -> (result: (<cell types>))
///         requires <inv> && <cond>,
///         ensures
///             result.0 == <step_cells[0]>, result.1 == <step_cells[1]>,  // body-TV (AC-5)
///             <inv[cells := step_cells]>,                                 // preservation (AC-2)
///     {
///         <p_production — returns (cell0', cell1')>
///     }
/// }
/// fn main() {}
/// ```
///
/// Verified iff one faithful iteration preserves the invariant (and production computes
/// the reference step); a `postcondition not satisfied` is a per-iteration state-lowering
/// infidelity (a dropped, reordered, or wrong-cell body mutation covered by
/// `body_ref_sound`'s negative lemmas) or a broken-invariant body (the source step
/// does not re-establish `inv`). Returns [`ExecRefEncodeError`] if the loop is outside
/// the v1 frozen subset (an Skipped).
pub fn loop_preservation_obligation(
    block: &Block,
    p_production: &str,
    frame: &LoopObligationFrame,
) -> Result<String, ExecRefEncodeError> {
    let obs = loop_ref_obligations(block, &frame.body_ref_ctx())?;

    let mut out = String::new();
    out.push_str("use vstd::prelude::*;\n");
    out.push_str("verus! {\n");
    for def in &frame.spec_defs {
        out.push('\n');
        out.push_str(def);
        out.push('\n');
    }
    out.push_str("\nfn tv_loop_step(");
    out.push_str(&frame.cell_and_input_param_list());
    out.push_str(") -> (result: ");
    out.push_str(&frame.cell_tuple_type());
    out.push(')');
    // The loop-step's frame is the loop-head assumption `inv ∧ cond` (not the
    // enclosing fn `requires` — Verus havocs the cells and assumes the invariant, so
    // the body proof is over a single arbitrary iteration).
    out.push_str("\n    requires ");
    out.push_str(&obs.inv);
    out.push_str(" && ");
    out.push_str(&obs.cond);
    out.push(',');
    // The `ensures`: (a) production's result equals the reference single-step state
    // (the body-TV reuse of `body_ref_state` — a per-iteration infidelity is caught
    // here, AC-5); (b) the invariant at the stepped state (the preservation conjunct —
    // a broken-invariant body is caught here, AC-2).
    out.push_str("\n    ensures\n");
    let proj = |i: usize| {
        if frame.cells.len() == 1 {
            "result".to_string()
        } else {
            format!("result.{i}")
        }
    };
    for (i, step) in obs.step_cells.iter().enumerate() {
        out.push_str(&format!("        {} == {step},\n", proj(i)));
    }
    out.push_str(&format!("        {},\n", obs.inv_at_step));
    out.push_str("{\n");
    out.push_str(p_production);
    out.push_str("}\n");
    out.push_str("\n}\nfn main() {}\n");
    Ok(out)
}

/// Build the exit loop obligation (`loop-tv.md` REQ-2.3): on exit the loop guarantees
/// `inv ∧ ¬cond`; the obligation pins that the production's after-loop characterization
/// `claimed_after_loop` (how the statements following the loop read the opaque cells)
/// follows from `inv ∧ ¬cond`. `claimed_after_loop` is the verbatim
/// production after-loop characterization (the artifact under test); `frame` carries
/// the cell + input params.
///
/// Emitted shape (a proof fn — the cells are the opaque-but-invariant-constrained
/// after-loop state, havocked + re-constrained to `inv ∧ ¬cond`, the way Verus
/// itself models a loop's after-state):
///
/// ```text
/// use vstd::prelude::*;
/// verus! {
///     <frame.spec_defs>
///     proof fn tv_loop_exit(<cells>, <inputs>)
///         requires <inv> && (!(<cond>)),    // the after-loop facts (assumed)
///     {
///         assert(<claimed_after_loop>);     // the production's after-loop claim
///     }
/// }
/// fn main() {}
/// ```
///
/// Verified iff the after-loop continuation reads the `inv ∧ ¬cond` state (the
/// claim follows); an `assertion failed` counterexample means a wrong after-loop
/// characterization (an over-strong claim about the exit state — stronger than
/// `inv ∧ ¬cond`). Returns [`ExecRefEncodeError`] if the loop is outside the v1 frozen
/// subset (an Skipped).
pub fn loop_exit_obligation(
    block: &Block,
    claimed_after_loop: &str,
    frame: &LoopObligationFrame,
) -> Result<String, ExecRefEncodeError> {
    let obs = loop_ref_obligations(block, &frame.body_ref_ctx())?;

    let mut out = String::new();
    out.push_str("use vstd::prelude::*;\n");
    out.push_str("verus! {\n");
    for def in &frame.spec_defs {
        out.push('\n');
        out.push_str(def);
        out.push('\n');
    }
    out.push_str("\nproof fn tv_loop_exit(");
    out.push_str(&frame.cell_and_input_param_list());
    out.push(')');
    // The after-loop facts Verus assumes for the continuation: `inv ∧ ¬cond` over the
    // opaque (havocked) cells — the analogue of how Verus models a loop's after-state.
    out.push_str("\n    requires ");
    out.push_str(&obs.inv);
    out.push_str(" && ");
    out.push_str(&negate_condition(&obs.cond));
    out.push(',');
    // The exit obligation: the production's after-loop characterization claim follows
    // from `inv ∧ ¬cond`. Verified iff the continuation reads the `inv ∧ ¬cond` state; a
    // counterexample means an over-claim (stronger than `inv ∧ ¬cond`).
    out.push_str("\n{\n    assert(");
    out.push_str(claimed_after_loop);
    out.push_str(");\n}\n");
    out.push_str("\n}\nfn main() {}\n");
    Ok(out)
}
