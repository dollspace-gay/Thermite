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

use thermite_syntax::ast::Expr;

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
