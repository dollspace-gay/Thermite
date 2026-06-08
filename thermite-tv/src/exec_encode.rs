//! The INDEPENDENT exec-position reference encoder
//! (`.design/verified/exec-tv.md` REQ-1; epic crosslink #151, blocker #152;
//! `thermite-design.md` §4.1/§6).
//!
//! [`exec_ref_value`] maps a PURE exec-position (body) [`Expr`] to a Verus EXEC
//! VALUE expression STRING — at the production VALUE TYPE (the BOUNDED `u64`/
//! `u32`/`usize`/`bool`, NEVER `nat`/`int`-coerced), the dual of the CONTRACT
//! reference encoder ([`crate::ref_encode::ref_contract_pred`], which encodes the
//! spec/`nat` semantics for a predicate). It is the small, declarative, human-
//! auditable re-implementation of the EXEC sublanguage's VALUE semantics —
//! authored AGAINST `thermite-design.md` §4.1/§6 + standard Rust/Verus exec
//! semantics, NOT against the production `lower_expr`.
//!
//! ## THE EXEC-VALUE SEMANTICS (the load-bearing concern)
//!
//! An exec value is BOUNDED — `u64`/`u32`/`usize`/`bool` with the always-active
//! runtime overflow checks (`thermite-design.md` §6, L1) — NOT unbounded `nat`/
//! `int`. The reference for `a + b` (source `u64`) is the bounded `u64` `a + b`,
//! which CARRIES the verus overflow obligation. A production that lowered to
//! `a.wrapping_sub(b)`/`a.wrapping_add(b)` (an overflow/wrong-op infidelity)
//! FAILS the obligation `ensures result == a + b` with a counterexample. A
//! reference that silently coerced to `nat` (`a as nat + b as nat`) would MASK
//! the wrap point — exactly the soundness hole to avoid (the dual of the
//! contract-side coercion-soundness concern). So this encoder is NEVER
//! nat-coerced; the comparison is at the production type.
//!
//! ## THE INDEPENDENCE BOUNDARY (the whole point — REQ-1 HARD CONSTRAINT)
//!
//! This module MUST NOT call `thermite_lower::lower::lower_expr` or any production
//! lowering symbol — `thermite-tv` does not even depend on `thermite-lower` (the
//! dep graph makes reuse a compile error, AC-6). The check `ensures result ==
//! <exec_ref_value(source)>` is N-version differential validation: agreement is
//! EVIDENCE, not proof. The cast-paren disciplines (#122 inner-paren on a
//! `Binary`/`Unary` cast inner; #146 outer-paren on a `Cast` left of a `<`-leading
//! op via [`is_lt_leading`]) and the 1-to-1 binop map ([`binop_str`]) are
//! RE-STATED here INDEPENDENTLY of `Expr::Cast`/`lower_binary_operand`/
//! `is_lt_leading`/`binop in lower.rs` — re-stating them is the point (an imported
//! map would hide a production paren/binop bug).
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (exec-expr reference encoder — independent, EXEC semantics) | SHIPPED | `pub fn exec_ref_value` here; non-test consumer `thermite_tv::obligation::exec_equivalence_obligation` (`obligation.rs`); verified by `thermite-tv/tests/exec_teeth.rs` E1–E4 against real verus (faithful VERIFIES, infidel CAUGHT). Bounded-typed (no `nat`/`int` coercion — the cast target is the source cast's target, the arithmetic stays at the operand type), independent of `lower_expr` (deps `thermite-syntax` + `thermite-spec` ONLY, `Cargo.toml`, AC-6). |

use std::collections::BTreeSet;
use std::fmt;

use thermite_syntax::ast::{BinOp, Expr, IndexArg, PrimType, Type, UnaryOp};

/// An honest failure to encode a construct outside the pure-exec subset (REQ-1).
/// The exec reference encoder NEVER panics and NEVER silently emits a wrong
/// encoding: an unsupported construct is a real `Err` carrying the offending shape
/// (R-CODE-2 / R-APG-1). A silent wrong encoding would defeat the entire point —
/// TV would compare a wrong reference and either spuriously pass or spuriously
/// fail. Method calls / Vec-String accessors are OUT OF SCOPE for step 2.1 (the
/// #154/#156 territory) → an honest [`RefEncodeError::Unsupported`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefEncodeError {
    /// A construct the pure-exec subset does not admit (a statement, a `let`, a
    /// loop, a control-flow expression, a method call, a struct literal, …).
    /// Carries a short description of the offending node so a human can see
    /// exactly what bit.
    Unsupported(String),
}

impl fmt::Display for RefEncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RefEncodeError::Unsupported(what) => {
                write!(f, "exec_encode: unsupported exec construct: {what}")
            }
        }
    }
}

impl std::error::Error for RefEncodeError {}

/// The exec-reference-encoding context (REQ-1). Carries which free names denote a
/// SLICE (`&[T]`) param, so an index `xs[i]` over a slice param encodes to the
/// spec-view element `xs[i as int]` (the bounded element VALUE: in an EXEC fn the
/// production indexes `xs[i]` with `i: usize`, and its value EQUALS the spec view
/// `xs[i as int]` — the obligation's `ensures result == xs[i as int]` is the
/// element-value equality, GROUNDED in `exec-tv.md` AC-5). A name NOT declared a
/// slice is indexed verbatim.
///
/// This is the EXEC dual of [`crate::ref_encode::RefCtx`] (which carries the
/// `@`-view / `nat`-coerce sets for SPEC position). It deliberately carries NO
/// `nat`-coerce set — the exec reference is bounded-typed, never nat-coerced.
#[derive(Debug, Clone, Default)]
pub struct ExecRefCtx {
    /// Names bound as a slice (`&[T]`) param in the obligation. An `Index` over
    /// such a name encodes to the spec-view element `xs[i as int]` (the bounded
    /// element value the production `xs[i]` computes); a non-slice base is indexed
    /// verbatim.
    slice_bound: BTreeSet<String>,
}

impl ExecRefCtx {
    /// A context in which the named free vars are bound as slice (`&[T]`) params,
    /// so an index over them encodes to the spec-view element `xs[i as int]`
    /// (mirroring the obligation's `xs: &[u32]` binding; `exec-tv.md` AC-5).
    pub fn with_slice_bound<I, S>(names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        ExecRefCtx {
            slice_bound: names.into_iter().map(Into::into).collect(),
        }
    }

    fn is_slice_bound(&self, name: &str) -> bool {
        self.slice_bound.contains(name)
    }
}

/// Encode a pure exec-position [`Expr`] to a Verus EXEC-VALUE expression STRING at
/// the production VALUE TYPE, independently of the production lowerer (REQ-1).
/// Covers exactly the pure-exec subset of `thermite-design.md` §4.1 (NO
/// statements, `let`, loops, mutation, or control flow — those are step 2.2):
///
/// - arithmetic ([`Expr::Binary`] over `Add`/`Sub`/`Mul`/`Div`/`Rem`/shifts/bitops
///   at the BOUNDED operand type — the bounded `u64`/`u32`/`usize` value carrying
///   the verus overflow obligation, NOT `nat`/`int`) — a faithful 1-to-1 binop map
///   ([`binop_str`]), RE-stated independently so a production wrong-op/overflow bug
///   (`+` → `wrapping_sub`, E3) is caught;
/// - comparisons ([`Expr::Binary`] over `Eq`/`Ne`/`Lt`/`Le`/`Gt`/`Ge` → `bool`);
/// - casts ([`Expr::Cast`] at the cast target, with the #122 inner-paren for a
///   `Binary`/`Unary` inner and the #146 outer-paren when a `Cast` is the LEFT
///   operand of a `<`-leading op — [`is_lt_leading`], re-implemented independently);
/// - calls ([`Expr::Call`] with a path callee — the exec callee verbatim);
/// - indexing ([`Expr::Index`] single-element over a slice param → `xs[i as int]`,
///   the bounded element VALUE).
///
/// Anything else (a method call, a Vec/String accessor, a struct literal, an `if`/
/// `match`, a closure, …) is an honest [`RefEncodeError::Unsupported`] (NEVER a
/// panic, NEVER a silent wrong encoding — #154/#156 territory).
pub fn exec_ref_value(expr: &Expr, ctx: &ExecRefCtx) -> Result<String, RefEncodeError> {
    encode(expr, ctx)
}

fn encode(expr: &Expr, ctx: &ExecRefCtx) -> Result<String, RefEncodeError> {
    match expr {
        Expr::IntLit { value, .. } => Ok(value.to_string()),
        Expr::BoolLit(b) => Ok(b.to_string()),
        Expr::Path(segments) => encode_path(segments),
        Expr::Binary { op, lhs, rhs } => encode_binary(*op, lhs, rhs, ctx),
        Expr::Unary { op, expr } => encode_unary(*op, expr, ctx),
        Expr::Call { callee, args } => encode_call(callee, args, ctx),
        Expr::Index { base, index } => encode_index(base, index, ctx),
        Expr::Cast { expr, ty } => encode_cast(expr, ty, ctx),
        other => Err(RefEncodeError::Unsupported(node_kind(other))),
    }
}

/// A path reference: a var or a `::`-qualified name. A pure exec value path is a
/// free var (a body param) or a constant path.
fn encode_path(segments: &[String]) -> Result<String, RefEncodeError> {
    if segments.is_empty() {
        return Err(RefEncodeError::Unsupported("empty path".to_string()));
    }
    Ok(segments.join("::"))
}

/// The faithful 1-to-1 binary-operator map (`thermite-design.md` §4.1). RE-stated
/// here INDEPENDENTLY of the production `binop in lower.rs`: a production wrong-op
/// bug (`+` → `wrapping_sub`, E3) is caught only because this map is the
/// independent ground truth. The exec arithmetic ops emit the BOUNDED operator
/// (`+`/`-`/`*`/…), which in an exec fn carries the verus overflow obligation —
/// NOT a `wrapping_*`/`nat` form.
fn binop_str(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
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

/// Is `op` a `<`-LEADING operator (`<`, `<=`, `<<`)? A `Cast` LEFT operand of such
/// an op MUST be wholly parenthesized — `x as u32 < 33` mis-parses the `u32 <` as
/// the start of a generic-argument list (the #146/#148 cast-paren ambiguity, a HARD
/// parse error in both Verus and Rust, E2). This is the EXACT dual of production's
/// `is_lt_leading in lower.rs` (`Lt | Le | Shl`), RE-stated INDEPENDENTLY here so
/// the reference parenthesizes the same class — without it the reference would emit
/// an un-parseable `as u32 <` and the obligation would be Unverifiable (not a
/// faithfulness verdict). `>`/`>=`/`>>`/`==`/`!=` do NOT trigger the generic
/// ambiguity (excluded — keeps the non-`<` casts paren-minimal).
fn is_lt_leading(op: BinOp) -> bool {
    matches!(op, BinOp::Lt | BinOp::Le | BinOp::Shl)
}

fn encode_binary(
    op: BinOp,
    lhs: &Expr,
    rhs: &Expr,
    ctx: &ExecRefCtx,
) -> Result<String, RefEncodeError> {
    let l = encode_binary_operand(lhs, op, true, ctx)?;
    let r = encode_binary_operand(rhs, op, false, ctx)?;
    // Parenthesize the whole binary so precedence is explicit at every level (the
    // #122 paren discipline generalized: a sub-expression never silently
    // re-associates). The bounded operand type is preserved — Verus/Z3 see the
    // SAME bounded term regardless of nesting, so the overflow obligation (E3) is
    // carried, not coerced away.
    Ok(format!("({l} {} {r})", binop_str(op)))
}

/// Encode a binary operand, applying the cast-`<`-leading paren (#146/#148): a
/// `Cast` that is the LEFT operand of a `<`-leading op (`<`/`<=`/`<<`) is wholly
/// parenthesized — `(x as u32) < 33`, never the ambiguous `x as u32 < 33` (E2).
/// This is the dual of production's `lower_binary_operand`'s `is_lt_leading` guard,
/// RE-stated INDEPENDENTLY. Every other operand is the plain [`encode`] (its own
/// parenthesization is already explicit per the #122 discipline).
fn encode_binary_operand(
    operand: &Expr,
    op: BinOp,
    is_left: bool,
    ctx: &ExecRefCtx,
) -> Result<String, RefEncodeError> {
    let s = encode(operand, ctx)?;
    if is_left && matches!(operand, Expr::Cast { .. }) && is_lt_leading(op) {
        return Ok(format!("({s})"));
    }
    Ok(s)
}

fn encode_unary(op: UnaryOp, inner: &Expr, ctx: &ExecRefCtx) -> Result<String, RefEncodeError> {
    let i = encode(inner, ctx)?;
    match op {
        UnaryOp::Not => Ok(format!("(!{i})")),
    }
}

/// A free-form call `f(args)` (REQ-1). In EXEC position the callee is emitted
/// VERBATIM (the exec call lowers to the exec fn by name; the value semantics are
/// the callee's own contract). A non-path callee is outside the pure-exec subset
/// (an honest `Err`). Arguments are encoded by the SAME independent recursion.
fn encode_call(callee: &Expr, args: &[Expr], ctx: &ExecRefCtx) -> Result<String, RefEncodeError> {
    let Expr::Path(segments) = callee else {
        return Err(RefEncodeError::Unsupported(format!(
            "call with a non-path callee ({})",
            node_kind(callee)
        )));
    };
    let name = segments.join("::");
    let encoded_args = args
        .iter()
        .map(|a| encode(a, ctx))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(format!("{name}({})", encoded_args.join(", ")))
}

/// `xs[i]` in exec position (REQ-1). A SINGLE index over a SLICE-bound base is the
/// bounded element VALUE — the spec view `xs[i as int]` (in an EXEC fn the
/// production indexes `xs[i]` with `i: usize`, whose value EQUALS the spec view
/// `xs[i as int]`; the obligation `ensures result == xs[i as int]` is the
/// element-value equality, GROUNDED `exec-tv.md` AC-5/E4). A `RangeTo`/`RangeFrom`/
/// `Range` slice index produces a sub-SLICE (not a scalar value) — outside the
/// pure-exec scalar-value subset of step 2.1 → an honest `Err`. A non-slice base
/// index is also unsupported (no scalar-value denotation in the frozen subset).
fn encode_index(base: &Expr, index: &IndexArg, ctx: &ExecRefCtx) -> Result<String, RefEncodeError> {
    let IndexArg::Single(i) = index else {
        return Err(RefEncodeError::Unsupported(
            "slice-range index in exec position (a sub-slice is not a scalar \
             exec value — step 2.2 territory)"
                .to_string(),
        ));
    };
    // Only a slice-bound base has the spec-view element-value denotation; a
    // non-slice base index is outside the frozen pure-exec value subset.
    if let Expr::Path(segments) = base {
        if segments.len() == 1 && ctx.is_slice_bound(&segments[0]) {
            let idx = encode_index_value(i, ctx)?;
            return Ok(format!("{}[{idx}]", segments[0]));
        }
    }
    Err(RefEncodeError::Unsupported(format!(
        "index over a non-slice base ({}) — the frozen exec index subset is \
         `xs[i]` over a slice param",
        node_kind(base)
    )))
}

/// Encode a slice index VALUE `i` as `<i> as int` (a Verus `Seq` index is `int`).
/// A bare integer literal stays bare (Verus coerces it); a bare path (`i: usize`)
/// is cast `<i> as int`; a compound index is parenthesized then cast (the #122
/// paren discipline). This is the index of the spec ELEMENT-VALUE view `xs[i as
/// int]` — the bounded element the production exec `xs[i]` computes.
fn encode_index_value(expr: &Expr, ctx: &ExecRefCtx) -> Result<String, RefEncodeError> {
    match expr {
        Expr::IntLit { value, .. } => Ok(value.to_string()),
        Expr::Path(segments) => {
            let p = encode_path(segments)?;
            Ok(format!("{p} as int"))
        }
        other => {
            let e = encode(other, ctx)?;
            Ok(format!("({e}) as int"))
        }
    }
}

/// An integer cast `e as T` → `(e) as T` with the #122 inner-paren discipline (the
/// inner is parenthesized when it is a `Binary`/`Unary` so the cast binds the WHOLE
/// inner — `(n - 1) as u8`, NEVER `n - 1 as u8` which parses as `n - (1 as u8)`,
/// the E1 infidelity). The cast TARGET is the BOUNDED prim (`u8`/`u32`/`u64`/`usize`)
/// — NEVER `nat`/`int` (the exec value semantics: a narrowing cast wraps at the
/// bounded type, and the obligation catches a wrong-paren/wrong-target wrap). This
/// is the dual of the contract encoder's `as nat`/`as int` cast.
fn encode_cast(inner: &Expr, ty: &Type, ctx: &ExecRefCtx) -> Result<String, RefEncodeError> {
    let e = encode(inner, ctx)?;
    let target = cast_target(ty)?;
    // The #122 inner-paren discipline. A `Binary`/`Unary` inner is ALREADY wholly
    // parenthesized by [`encode_binary`]/[`encode_unary`] (which wrap every binary/
    // unary), so the cast binds the WHOLE inner — `(n - 1) as u8`, never the E1
    // mis-bind `n - 1 as u8` (= `n - (1 as u8)`). We therefore do NOT re-wrap a
    // `Binary`/`Unary` inner (that would emit the cosmetically-redundant
    // `((n - 1)) as u8`); a bare path/literal/index inner never mis-binds, so it is
    // cast bare. This matches production's minimal-paren cast output exactly
    // (`thermite-lower/src/lower.rs::exec_expr_tests` pins `(n - 1) as u8`).
    Ok(format!("{e} as {target}"))
}

/// The Verus cast-target spelling for an EXEC cast (`thermite-design.md` §4.1). The
/// exec sublanguage casts to the BOUNDED prims (`u8`/`u16`/`u32`/`u64`/`usize`) —
/// NEVER the spec `nat`/`int` (that is the CONTRACT encoder's target). A `bool`
/// cast is not an arithmetic cast (an honest `Err`). The narrower bounded targets
/// (`u8`/`u16`) the fixture E1 needs are accepted alongside the prim-type set so a
/// narrowing/wrapping cast (the #122 surface) is encodable.
fn cast_target(ty: &Type) -> Result<String, RefEncodeError> {
    match ty {
        Type::Prim(PrimType::U32) => Ok("u32".to_string()),
        Type::Prim(PrimType::U64) => Ok("u64".to_string()),
        Type::Prim(PrimType::Usize) => Ok("usize".to_string()),
        Type::Prim(PrimType::Bool) => Err(RefEncodeError::Unsupported(
            "cast to bool (not an arithmetic exec cast)".to_string(),
        )),
        // The narrower bounded byte/half targets a narrowing cast (#122) uses
        // (`(n - 1) as u8`). They are not surface `PrimType`s but ARE valid Verus
        // exec cast targets; spelled as the bounded Rust int names. Keyed on the
        // exact named-type spelling so a non-bounded named type is rejected.
        Type::Named(n) if matches!(n.as_str(), "u8" | "u16") => Ok(n.clone()),
        other => Err(RefEncodeError::Unsupported(format!(
            "exec cast to unsupported type {other:?} (the exec cast targets are \
             the bounded `u8`/`u16`/`u32`/`u64`/`usize`, NEVER `nat`/`int`)"
        ))),
    }
}

/// A short human-readable tag for an `Expr` variant, for error messages.
fn node_kind(e: &Expr) -> String {
    match e {
        Expr::IntLit { .. } => "int literal".to_string(),
        Expr::BoolLit(_) => "bool literal".to_string(),
        Expr::Path(_) => "path".to_string(),
        Expr::Call { .. } => "call".to_string(),
        Expr::MethodCall { .. } => "method call (exec method / Vec-String accessor \
             — step 2.2 / #154/#156 territory)"
            .to_string(),
        Expr::Field { .. } => "field access".to_string(),
        Expr::Closure { .. } => "closure".to_string(),
        Expr::Match { .. } => "match expression".to_string(),
        Expr::If { .. } => "if expression".to_string(),
        Expr::Binary { .. } => "binary".to_string(),
        Expr::Unary { .. } => "unary".to_string(),
        Expr::Index { .. } => "index".to_string(),
        Expr::Cast { .. } => "cast".to_string(),
        Expr::Ref { .. } => "reference".to_string(),
        Expr::StructLit { .. } => "struct literal".to_string(),
        Expr::Is { .. } => "is-test".to_string(),
        Expr::Deref(_) => "deref".to_string(),
        Expr::StrLit(_) => "string literal".to_string(),
        Expr::Tuple(_) => "tuple".to_string(),
        Expr::TupleProj { .. } => "tuple projection".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(name: &str) -> Expr {
        Expr::Path(vec![name.to_string()])
    }
    fn int(value: u128) -> Expr {
        Expr::IntLit {
            value,
            raw: value.to_string(),
        }
    }
    fn bin(op: BinOp, lhs: Expr, rhs: Expr) -> Expr {
        Expr::Binary {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }
    }
    fn cast(inner: Expr, ty: Type) -> Expr {
        Expr::Cast {
            expr: Box::new(inner),
            ty,
        }
    }

    /// E1: `(n - 1) as u8` → the #122 inner-paren on the `Binary` inner, bounded
    /// `u8` target (never `nat`). The reference MEANS the faithful production form.
    #[test]
    fn e1_cast_inner_paren() {
        let e = cast(
            bin(BinOp::Sub, path("n"), int(1)),
            Type::Named("u8".to_string()),
        );
        assert_eq!(
            exec_ref_value(&e, &ExecRefCtx::default()).unwrap(),
            "(n - 1) as u8"
        );
    }

    /// E2: `x as u32 < 33` → the #146 outer-paren on the `Cast` left of `<`.
    #[test]
    fn e2_cast_lt_outer_paren() {
        let e = bin(
            BinOp::Lt,
            cast(path("x"), Type::Prim(PrimType::U32)),
            int(33),
        );
        assert_eq!(
            exec_ref_value(&e, &ExecRefCtx::default()).unwrap(),
            "((x as u32) < 33)"
        );
    }

    /// E3: `a + b` → bounded `u64` add (NOT nat-coerced, NOT `wrapping_add`), so
    /// the obligation carries the overflow obligation.
    #[test]
    fn e3_bounded_add() {
        let e = bin(BinOp::Add, path("a"), path("b"));
        assert_eq!(
            exec_ref_value(&e, &ExecRefCtx::default()).unwrap(),
            "(a + b)"
        );
    }

    /// E4: `xs[i]` over a slice param → the spec-view element value `xs[i as int]`.
    #[test]
    fn e4_slice_index_element_value() {
        let e = Expr::Index {
            base: Box::new(path("xs")),
            index: IndexArg::Single(Box::new(path("i"))),
        };
        let ctx = ExecRefCtx::with_slice_bound(["xs"]);
        assert_eq!(exec_ref_value(&e, &ctx).unwrap(), "xs[i as int]");
    }

    /// A method call (exec / Vec-String accessor) is OUT OF SCOPE for step 2.1 →
    /// an honest `Err`, NEVER a silent wrong encoding (REQ-1 / R-CODE-2).
    #[test]
    fn method_call_is_unsupported_not_panic() {
        let e = Expr::MethodCall {
            receiver: Box::new(path("v")),
            name: "len".to_string(),
            args: vec![],
        };
        assert!(matches!(
            exec_ref_value(&e, &ExecRefCtx::default()),
            Err(RefEncodeError::Unsupported(_))
        ));
    }

    /// A bare index over a non-slice base has no scalar-value denotation in the
    /// frozen subset → an honest `Err`.
    #[test]
    fn non_slice_index_is_unsupported() {
        let e = Expr::Index {
            base: Box::new(path("xs")),
            index: IndexArg::Single(Box::new(path("i"))),
        };
        // `xs` NOT declared a slice → unsupported.
        assert!(matches!(
            exec_ref_value(&e, &ExecRefCtx::default()),
            Err(RefEncodeError::Unsupported(_))
        ));
    }
}
