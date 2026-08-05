//! The independent exec-position reference encoder
//! (`.design/verified/exec-tv.md` REQ-1; epic crosslink #151, blocker #152;
//! `thermite-design.md` §4.1/§6).
//!
//! [`exec_ref_value`] maps a pure exec-position (body) [`Expr`] to a Verus exec
//! value expression string — at the production value type (the bounded `u64`/
//! `u32`/`usize`/`bool`, not `nat`/`int`-coerced), the dual of the contract
//! reference encoder ([`crate::ref_encode::ref_contract_pred`], which encodes the
//! spec/`nat` semantics for a predicate). It is the small, declarative, human-
//! auditable re-implementation of the exec sublanguage's value semantics —
//! authored against `thermite-design.md` §4.1/§6 + standard Rust/Verus exec
//! semantics, not against the production `lower_expr`.
//!
//! ## The exec-value semantics
//!
//! An exec value is bounded — `u64`/`u32`/`usize`/`bool` with the always-active
//! runtime overflow checks (`thermite-design.md` §6, L1) — not unbounded `nat`/
//! `int`. The reference for `a + b` (source `u64`) is the bounded `u64` `a + b`,
//! which carries the verus overflow obligation. A production that lowered to
//! `a.wrapping_sub(b)`/`a.wrapping_add(b)` (an overflow/wrong-op infidelity)
//! fails the obligation `ensures result == a + b` with a counterexample. A
//! reference that silently coerced to `nat` (`a as nat + b as nat`) would mask
//! the wrap point, the soundness hole to avoid (the dual of the contract-side
//! coercion-soundness concern). So this encoder is never nat-coerced; the
//! comparison is at the production type.
//!
//! ## The independence boundary (REQ-1 constraint)
//!
//! This module must not call `thermite_lower::lower::lower_expr` or any production
//! lowering symbol; `thermite-tv` does not even depend on `thermite-lower` (the
//! dep graph makes reuse a compile error, AC-6). The check `ensures result ==
//! <exec_ref_value(source)>` is N-version differential validation: agreement is
//! evidence, not proof. The cast-paren disciplines (#122 inner-paren on a
//! `Binary`/`Unary` cast inner; #146 outer-paren on a `Cast` left of a `<`-leading
//! op via [`is_lt_leading`]) and the 1-to-1 binop map ([`binop_str`]) are
//! re-stated here independently of `Expr::Cast`/`lower_binary_operand`/
//! `is_lt_leading`/`binop in lower.rs`; re-stating them is the point (an imported
//! map would hide a production paren/binop bug).
//!
//! ## REQ status
//!
//! <!-- generated:reqs view=thermite-tv-exec-encode-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-TV-EXEC-REF-ENCODER | shipped | `thermite-tv/src/exec_encode.rs` | Exec-TV independent reference encoder |  |
//! <!-- /generated:reqs -->

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Write as _};

use thermite_syntax::ast::{ArrayLen, BinOp, Expr, IndexArg, PrimType, Type, UnaryOp};

/// An failure to encode a construct outside the pure-exec subset (REQ-1).
/// The exec reference encoder never panics and never silently emits a wrong
/// encoding: an unsupported construct is a real `Err` carrying the offending shape
/// (R-CODE-2 / R-APG-1). A silent wrong encoding would compare a wrong reference
/// and either spuriously pass or spuriously fail. Method calls / Vec-String
/// accessors are out of scope for step 2.1 (the #154/#156 territory) → an
/// [`RefEncodeError::Unsupported`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefEncodeError {
    /// A construct the pure-exec subset does not admit (a statement, a `let`, a
    /// loop, a control-flow expression, a method call, a struct literal, …).
    /// Carries a short description of the offending node so a human can see
    /// what bit.
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
/// slice (`&[T]`) param, so an index `xs[i]` over a slice param encodes to the
/// spec-view element `xs[i as int]` (the bounded element value: in an exec fn the
/// production indexes `xs[i]` with `i: usize`, and its value equals the spec view
/// `xs[i as int]` — the obligation's `ensures result == xs[i as int]` is the
/// element-value equality, grounded in `exec-tv.md` AC-5). A name not declared a
/// slice is indexed verbatim.
///
/// This is the exec dual of [`crate::ref_encode::RefCtx`] (which carries the
/// `@`-view / `nat`-coerce sets for spec position). It carries no `nat`-coerce
/// set: the exec reference is bounded-typed, never nat-coerced.
#[derive(Debug, Clone, Default)]
pub struct ExecRefCtx {
    /// Names bound as a slice (`&[T]`) param in the obligation. An `Index` over
    /// such a name encodes to the spec-view element `xs[i as int]` (the bounded
    /// element value the production `xs[i]` computes). Native fixed arrays are
    /// tracked separately and indexed through their finite `@` view.
    slice_bound: BTreeSet<String>,
    /// Names bound as native fixed arrays. Their executable index is compared
    /// through the array's finite `@` view in an `ensures` predicate.
    fixed_array_bound: BTreeSet<String>,
    /// Direct `root.field` paths whose independently parsed field type is a
    /// native fixed array.
    fixed_array_fields: BTreeSet<String>,
    /// Closed-form scalar bindings threaded by aggregate lifecycle TV. A path
    /// in this map denotes the value captured at that source program point.
    value_bindings: BTreeMap<String, String>,
    /// Closed-form direct record-field cells, keyed as `root.field`.
    field_bindings: BTreeMap<String, String>,
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
            fixed_array_bound: BTreeSet::new(),
            fixed_array_fields: BTreeSet::new(),
            value_bindings: BTreeMap::new(),
            field_bindings: BTreeMap::new(),
        }
    }

    /// Add native fixed-array bindings to this reference frame.
    pub fn with_fixed_array_bound<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.fixed_array_bound = names.into_iter().map(Into::into).collect();
        self
    }

    /// Add exact direct record-field paths whose declared value is a native
    /// fixed array.
    pub fn with_fixed_array_fields<I, S>(mut self, paths: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.fixed_array_fields = paths.into_iter().map(Into::into).collect();
        self
    }

    /// Add exact closed-form local bindings for lifecycle state threading.
    pub fn with_value_bindings<I, K, V>(mut self, bindings: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        self.value_bindings = bindings
            .into_iter()
            .map(|(name, value)| (name.into(), value.into()))
            .collect();
        self
    }

    /// Add exact closed-form direct named-record field cells.
    pub fn with_field_bindings<I, K, V>(mut self, bindings: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        self.field_bindings = bindings
            .into_iter()
            .map(|(name, value)| (name.into(), value.into()))
            .collect();
        self
    }

    fn is_slice_bound(&self, name: &str) -> bool {
        self.slice_bound.contains(name)
    }

    fn is_fixed_array_bound(&self, name: &str) -> bool {
        self.fixed_array_bound.contains(name)
    }

    fn is_fixed_array_field(&self, root: &str, field: &str) -> bool {
        self.fixed_array_fields.contains(&format!("{root}.{field}"))
            || self.fixed_array_fields.contains(field)
    }

    fn value_binding(&self, name: &str) -> Option<&str> {
        self.value_bindings.get(name).map(String::as_str)
    }

    fn field_binding(&self, root: &str, field: &str) -> Option<&str> {
        self.field_bindings
            .get(&format!("{root}.{field}"))
            .map(String::as_str)
    }
}

/// Encode a pure exec-position [`Expr`] to a Verus exec-value expression string at
/// the production value type, independently of the production lowerer (REQ-1).
/// Covers the pure-exec subset of `thermite-design.md` §4.1 (no statements, `let`,
/// loops, mutation, or control flow — those are step 2.2):
///
/// - arithmetic ([`Expr::Binary`] over `Add`/`Sub`/`Mul`/`Div`/`Rem`/shifts/bitops
///   at the bounded operand type — the bounded `u64`/`u32`/`usize` value carrying
///   the verus overflow obligation, not `nat`/`int`) — a faithful 1-to-1 binop map
///   ([`binop_str`]), re-stated independently so a production wrong-op/overflow bug
///   (`+` → `wrapping_sub`, E3) is caught;
/// - comparisons ([`Expr::Binary`] over `Eq`/`Ne`/`Lt`/`Le`/`Gt`/`Ge` → `bool`);
/// - casts ([`Expr::Cast`] at the cast target, with the #122 inner-paren for a
///   `Binary`/`Unary` inner and the #146 outer-paren when a `Cast` is the left
///   operand of a `<`-leading op — [`is_lt_leading`], re-implemented independently);
/// - calls ([`Expr::Call`] with a path callee — the exec callee verbatim);
/// - indexing ([`Expr::Index`] single-element over a slice param → `xs[i as int]`,
///   the bounded element value).
///
/// Anything else (a method call other than borrowed-slice/fixed-array `.len()` or
/// fixed-array `.array_eq(other)`, a Vec/String accessor, a struct literal, an `if`/`match`, a closure, …) is an
/// [`RefEncodeError::Unsupported`] (never a panic, never a silent wrong encoding —
/// #154/#156 territory).
pub fn exec_ref_value(expr: &Expr, ctx: &ExecRefCtx) -> Result<String, RefEncodeError> {
    encode(expr, ctx)
}

fn encode(expr: &Expr, ctx: &ExecRefCtx) -> Result<String, RefEncodeError> {
    match expr {
        Expr::IntLit { value, .. } => Ok(value.to_string()),
        Expr::BoolLit(b) => Ok(b.to_string()),
        Expr::Array(elements) => {
            let elements = elements
                .iter()
                .map(|element| encode(element, ctx))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(format!("[{}]", elements.join(", ")))
        }
        Expr::ArrayRepeat { value, len } => {
            let value = encode(value, ctx)?;
            Ok(format!("[{value}; {}]", encode_array_len(len)))
        }
        Expr::Path(segments) => {
            if let [name] = segments.as_slice() {
                if let Some(value) = ctx.value_binding(name) {
                    return Ok(value.to_string());
                }
            }
            encode_path(segments)
        }
        Expr::Binary { op, lhs, rhs } => encode_binary(*op, lhs, rhs, ctx),
        Expr::Unary { op, expr } => encode_unary(*op, expr, ctx),
        Expr::Call { callee, args } => encode_call(callee, args, ctx),
        Expr::MethodCall {
            receiver,
            name,
            args,
        } => encode_method_call(receiver, name, args, ctx),
        Expr::Field { receiver, name } => {
            if let Expr::Path(path) = receiver.as_ref() {
                if let [root] = path.as_slice() {
                    if let Some(value) = ctx.field_binding(root, name) {
                        return Ok(value.to_string());
                    }
                }
            }
            let receiver = encode(receiver, ctx)?;
            Ok(format!("{receiver}.{name}"))
        }
        Expr::StructLit { path, fields } => {
            if path.is_empty() {
                return Err(RefEncodeError::Unsupported(
                    "struct literal with an empty type path".to_string(),
                ));
            }
            let fields = fields
                .iter()
                .map(|(name, value)| Ok(format!("{name}: {}", encode(value, ctx)?)))
                .collect::<Result<Vec<_>, RefEncodeError>>()?;
            Ok(format!("({} {{ {} }})", path.join("::"), fields.join(", ")))
        }
        Expr::Tuple(elements) => {
            let elements = elements
                .iter()
                .map(|element| encode(element, ctx))
                .collect::<Result<Vec<_>, _>>()?;
            let trailing = if elements.len() == 1 { "," } else { "" };
            Ok(format!("({}{trailing})", elements.join(", ")))
        }
        Expr::TupleProj { receiver, index } => {
            let receiver = encode(receiver, ctx)?;
            Ok(format!("({receiver}).{index}"))
        }
        Expr::Ref {
            mutable: false,
            expr,
        } => {
            let value = encode(expr, ctx)?;
            Ok(format!("&({value})"))
        }
        Expr::Ref { mutable: true, .. } => Err(RefEncodeError::Unsupported(
            "mutable reference construction requires an exact call-effect state frame".to_string(),
        )),
        Expr::Deref(inner) => {
            let inner = encode(inner, ctx)?;
            Ok(format!("*({inner})"))
        }
        Expr::If { cond, then, else_ } => {
            if !then.stmts.is_empty() || !else_.stmts.is_empty() {
                return Err(RefEncodeError::Unsupported(
                    "if expression with branch statements requires body-state threading"
                        .to_string(),
                ));
            }
            let then_value = then.tail.as_deref().ok_or_else(|| {
                RefEncodeError::Unsupported("if expression then-branch has no value".to_string())
            })?;
            let else_value = else_.tail.as_deref().ok_or_else(|| {
                RefEncodeError::Unsupported("if expression else-branch has no value".to_string())
            })?;
            Ok(format!(
                "if {} {{ {} }} else {{ {} }}",
                encode(cond, ctx)?,
                encode(then_value, ctx)?,
                encode(else_value, ctx)?
            ))
        }
        Expr::Index { base, index } => encode_index(base, index, ctx),
        Expr::Cast { expr, ty } => encode_cast(expr, ty, ctx),
        other => Err(RefEncodeError::Unsupported(node_kind(other))),
    }
}

fn encode_method_call(
    receiver: &Expr,
    name: &str,
    args: &[Expr],
    ctx: &ExecRefCtx,
) -> Result<String, RefEncodeError> {
    let is_fixed_array_value = |expr: &Expr| {
        let is_bound_array = matches!(expr, Expr::Path(segments)
        if segments.len() == 1 && ctx.is_fixed_array_bound(&segments[0]));
        let is_array_value = matches!(expr, Expr::Array(_) | Expr::ArrayRepeat { .. })
            || matches!(expr, Expr::Call { callee, .. }
            if matches!(callee.as_ref(), Expr::Path(path)
                if path.join("::") == "vstd::array::spec_array_update"));
        is_bound_array || is_array_value
    };
    let is_slice_value = |expr: &Expr| {
        matches!(expr, Expr::Path(segments)
            if segments.len() == 1 && ctx.is_slice_bound(&segments[0]))
    };

    if args.len() == 1 && matches!(name, "bit_test" | "bit_set" | "bit_clear") {
        let word = encode(receiver, ctx)?;
        let offset = encode(&args[0], ctx)?;
        return Ok(encode_u64_bit_reference(&word, &offset, name));
    }
    if args.len() == 2
        && matches!(
            name,
            "bit_set_preserves_other" | "bit_clear_preserves_other"
        )
    {
        let word = encode(receiver, ctx)?;
        let changed = encode(&args[0], ctx)?;
        let observed = encode(&args[1], ctx)?;
        return Ok(encode_u64_bit_preservation_reference(
            &word, &changed, &observed, name,
        ));
    }

    match name {
        "len" if args.is_empty() && is_slice_value(receiver) => {
            let slice = encode(receiver, ctx)?;
            Ok(format!("({slice}@.len() as usize)"))
        }
        "len" if args.is_empty() && is_fixed_array_value(receiver) => {
            let array = encode(receiver, ctx)?;
            Ok(format!("({array}@.len() as usize)"))
        }
        "array_eq"
            if args.len() == 1
                && is_fixed_array_value(receiver)
                && is_fixed_array_value(&args[0]) =>
        {
            let left = encode(receiver, ctx)?;
            let right = encode(&args[0], ctx)?;
            Ok(format!("(({left})@ =~= ({right})@)"))
        }
        "array_same_except"
            if args.len() == 2
                && is_fixed_array_value(receiver)
                && is_fixed_array_value(&args[0]) =>
        {
            let left = encode(receiver, ctx)?;
            let right = encode(&args[0], ctx)?;
            let except = encode(&args[1], ctx)?;
            Ok(format!(
                "(forall|__thermite_i: int| 0 <= __thermite_i < ({left})@.len() && __thermite_i != ({except}) as int ==> ({left})@[__thermite_i] == ({right})@[__thermite_i])"
            ))
        }
        _ => Err(RefEncodeError::Unsupported(format!(
            "exec method `.{name}()` outside the borrowed-slice/fixed-array \
             `.len()` / fixed-array relation subset, or with an \
             unsupported operand"
        ))),
    }
}

/// Independent finite semantics for the total packed-`u64` bit methods. This
/// intentionally spells the 64 masks from the surface meaning rather than
/// importing the production helper generator.
fn encode_u64_bit_reference(word: &str, offset: &str, method: &str) -> String {
    let mut out = format!("(match ({offset}) {{ ");
    for bit in 0..64usize {
        let mask = 1u64 << bit;
        let value = match method {
            "bit_test" => format!("({word}) & {mask}u64 != 0u64"),
            "bit_set" => format!("({word}) | {mask}u64"),
            "bit_clear" => format!("({word}) & !{mask}u64"),
            _ => unreachable!("caller restricts the frozen bit method"),
        };
        write!(out, "{bit} => {value}, ").ok();
    }
    let fallback = if method == "bit_test" {
        "false".to_string()
    } else {
        format!("({word})")
    };
    write!(out, "_ => {fallback} }})").ok();
    out
}

fn encode_u64_bit_preservation_reference(
    word: &str,
    changed: &str,
    observed: &str,
    method: &str,
) -> String {
    let update = if method == "bit_set_preserves_other" {
        "bit_set"
    } else {
        "bit_clear"
    };
    let updated = encode_u64_bit_reference(word, changed, update);
    let after = encode_u64_bit_reference(&updated, observed, "bit_test");
    let before = encode_u64_bit_reference(word, observed, "bit_test");
    format!(
        "(({changed}) < 64usize && ({observed}) < 64usize && ({changed}) != ({observed}) && (({after}) == ({before})))"
    )
}

/// A path reference: a var or a `::`-qualified name. A pure exec value path is a
/// free var (a body param) or a constant path.
fn encode_path(segments: &[String]) -> Result<String, RefEncodeError> {
    if segments.is_empty() {
        return Err(RefEncodeError::Unsupported("empty path".to_string()));
    }
    Ok(segments.join("::"))
}

fn encode_array_len(len: &ArrayLen) -> String {
    match len {
        ArrayLen::Literal { value, .. } => value.to_string(),
        ArrayLen::Const(name) => name.clone(),
    }
}

/// The faithful 1-to-1 binary-operator map (`thermite-design.md` §4.1). Re-stated
/// here independently of the production `binop in lower.rs`: a production wrong-op
/// bug (`+` → `wrapping_sub`, E3) is caught only because this map is the
/// independent ground truth. The exec arithmetic ops emit the bounded operator
/// (`+`/`-`/`*`/…), which in an exec fn carries the verus overflow obligation,
/// not a `wrapping_*`/`nat` form.
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

/// Is `op` a `<`-leading operator (`<`, `<=`, `<<`)? A `Cast` left operand of such
/// an op must be wholly parenthesized: `x as u32 < 33` mis-parses the `u32 <` as
/// the start of a generic-argument list (the #146/#148 cast-paren ambiguity, a
/// parse error in both Verus and Rust, E2). This is the dual of production's
/// `is_lt_leading in lower.rs` (`Lt | Le | Shl`), re-stated independently here so
/// the reference parenthesizes the same class. Without it the reference would emit
/// an un-parseable `as u32 <` and the obligation would be Unverifiable (not a
/// faithfulness verdict). `>`/`>=`/`>>`/`==`/`!=` do not trigger the generic
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
    // re-associates). The bounded operand type is preserved: Verus/Z3 see the
    // same bounded term regardless of nesting, so the overflow obligation (E3) is
    // carried, not coerced away.
    Ok(format!("({l} {} {r})", binop_str(op)))
}

/// Encode a binary operand, applying the cast-`<`-leading paren (#146/#148): a
/// `Cast` that is the left operand of a `<`-leading op (`<`/`<=`/`<<`) is wholly
/// parenthesized — `(x as u32) < 33`, never the ambiguous `x as u32 < 33` (E2).
/// This is the dual of production's `lower_binary_operand`'s `is_lt_leading` guard,
/// re-stated independently. Every other operand is the plain [`encode`] (its own
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

/// A free-form call `f(args)` (REQ-1). In exec position the callee is emitted
/// verbatim (the exec call lowers to the exec fn by name; the value semantics are
/// the callee's own contract). A non-path callee is outside the pure-exec subset
/// (an `Err`). Arguments are encoded by the same independent recursion.
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
        .enumerate()
        .map(|(index, arg)| {
            if name == "vstd::array::spec_array_update" && index == 1 {
                encode_index_value(arg, ctx)
            } else {
                encode(arg, ctx)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(format!("{name}({})", encoded_args.join(", ")))
}

/// `xs[i]` in exec position (REQ-1). A single index over a slice-bound base is the
/// bounded element value — the spec view `xs[i as int]` (in an exec fn the
/// production indexes `xs[i]` with `i: usize`, whose value equals the spec view
/// `xs[i as int]`; the obligation `ensures result == xs[i as int]` is the
/// element-value equality, grounded `exec-tv.md` AC-5/E4). A `RangeTo`/`RangeFrom`/
/// `Range` slice index produces a sub-slice (not a scalar value), outside the
/// pure-exec scalar-value subset of step 2.1 → an `Err`. Native fixed arrays
/// use their finite `@` view; every other non-slice base is unsupported.
fn encode_index(base: &Expr, index: &IndexArg, ctx: &ExecRefCtx) -> Result<String, RefEncodeError> {
    let IndexArg::Single(i) = index else {
        return Err(RefEncodeError::Unsupported(
            "slice-range index in exec position (a sub-slice is not a scalar \
             exec value — step 2.2 territory)"
                .to_string(),
        ));
    };
    // Slice and fixed-array bindings both expose a finite sequence view in the
    // obligation. Keep the historical slice spelling stable; native arrays use
    // their explicit `@` view.
    if let Expr::Path(segments) = base {
        if segments.len() == 1 && ctx.is_slice_bound(&segments[0]) {
            let idx = encode_index_value(i, ctx)?;
            return Ok(format!("{}[{idx}]", segments[0]));
        }
        if segments.len() == 1 && ctx.is_fixed_array_bound(&segments[0]) {
            let idx = encode_index_value(i, ctx)?;
            return Ok(format!("{}@[{idx}]", segments[0]));
        }
    }
    if let Expr::Field { receiver, name } = base {
        if let Expr::Path(segments) = receiver.as_ref() {
            if let [root] = segments.as_slice() {
                if ctx.is_fixed_array_field(root, name) {
                    let array = encode(base, ctx)?;
                    let idx = encode_index_value(i, ctx)?;
                    return Ok(format!("({array})@[{idx}]"));
                }
            }
        }
    }
    // State threading substitutes a local fixed array with its initializer or
    // an exact `spec_array_update` expression. Index those values through their
    // native array view as well.
    if matches!(base, Expr::Array(_) | Expr::ArrayRepeat { .. })
        || matches!(base, Expr::Call { callee, .. }
            if matches!(callee.as_ref(), Expr::Path(path)
                if path.join("::") == "vstd::array::spec_array_update"))
    {
        let array = encode(base, ctx)?;
        let idx = encode_index_value(i, ctx)?;
        return Ok(format!("({array})@[{idx}]"));
    }
    Err(RefEncodeError::Unsupported(format!(
        "index over a non-slice / non-fixed-array base ({}) — the frozen exec \
         index subset is `xs[i]` over a slice or native fixed-array binding",
        node_kind(base)
    )))
}

/// Encode a slice index value `i` as `<i> as int` (a Verus `Seq` index is `int`).
/// A bare integer literal stays bare (Verus coerces it); a bare path (`i: usize`)
/// is cast `<i> as int`; a compound index is parenthesized then cast (the #122
/// paren discipline). This is the index of the spec element-value view `xs[i as
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
/// inner is parenthesized when it is a `Binary`/`Unary` so the cast binds the whole
/// inner — `(n - 1) as u8`, not `n - 1 as u8` which parses as `n - (1 as u8)`,
/// the E1 infidelity). The cast target is the bounded prim (`u8`/`u32`/`u64`/`usize`),
/// not `nat`/`int` (the exec value semantics: a narrowing cast wraps at the
/// bounded type, and the obligation catches a wrong-paren/wrong-target wrap). This
/// is the dual of the contract encoder's `as nat`/`as int` cast.
fn encode_cast(inner: &Expr, ty: &Type, ctx: &ExecRefCtx) -> Result<String, RefEncodeError> {
    let e = encode(inner, ctx)?;
    let target = cast_target(ty)?;
    // The #122 inner-paren discipline. A `Binary`/`Unary` inner is already wholly
    // parenthesized by [`encode_binary`]/[`encode_unary`] (which wrap every binary/
    // unary), so the cast binds the whole inner — `(n - 1) as u8`, never the E1
    // mis-bind `n - 1 as u8` (= `n - (1 as u8)`). We therefore do not re-wrap a
    // `Binary`/`Unary` inner (that would emit the cosmetically-redundant
    // `((n - 1)) as u8`); a bare path/literal/index inner never mis-binds, so it is
    // cast bare. This matches production's minimal-paren cast output
    // (`thermite-lower/src/lower.rs::exec_expr_tests` pins `(n - 1) as u8`).
    Ok(format!("{e} as {target}"))
}

/// The Verus cast-target spelling for an exec cast (`thermite-design.md` §4.1). The
/// exec sublanguage casts to the bounded prims (`u8`/`u16`/`u32`/`u64`/`usize`),
/// not the spec `nat`/`int` (that is the contract encoder's target). A `bool`
/// cast is not an arithmetic cast (an `Err`). The narrower bounded targets
/// (`u8`/`u16`) the fixture E1 needs are accepted alongside the prim-type set so a
/// narrowing/wrapping cast (the #122 surface) is encodable.
fn cast_target(ty: &Type) -> Result<String, RefEncodeError> {
    match ty {
        Type::Prim(PrimType::U8) => Ok("u8".to_string()),
        Type::Prim(PrimType::U16) => Ok("u16".to_string()),
        Type::Prim(PrimType::U32) => Ok("u32".to_string()),
        Type::Prim(PrimType::U64) => Ok("u64".to_string()),
        Type::Prim(PrimType::Usize) => Ok("usize".to_string()),
        Type::Prim(PrimType::Bool) => Err(RefEncodeError::Unsupported(
            "cast to bool (not an arithmetic exec cast)".to_string(),
        )),
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
        Expr::Array(_) => "array literal".to_string(),
        Expr::ArrayRepeat { .. } => "array repeat initializer".to_string(),
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
        // A raw quantifier `forall`/`exists` (`.design/stage2-stratified-cage.md`
        // REQ-0) is a spec-only formula with no exec encoding; this descriptor keeps
        // the exec node-kind report exhaustive.
        Expr::Quantifier { .. } => "quantifier (spec-only)".to_string(),
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
    /// `u8` target (never `nat`). The reference means the faithful production form.
    #[test]
    fn e1_cast_inner_paren() {
        let e = cast(bin(BinOp::Sub, path("n"), int(1)), Type::Prim(PrimType::U8));
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

    /// E3: `a + b` → bounded `u64` add (not nat-coerced, not `wrapping_add`), so
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

    /// A borrowed slice's executable length is related directly to the length of
    /// its mathematical sequence view (used by total export guards).
    #[test]
    fn borrowed_slice_len_uses_finite_view() {
        let e = Expr::MethodCall {
            receiver: Box::new(path("xs")),
            name: "len".to_string(),
            args: vec![],
        };
        let ctx = ExecRefCtx::with_slice_bound(["xs"]);
        assert_eq!(exec_ref_value(&e, &ctx).unwrap(), "(xs@.len() as usize)");
    }

    /// An unclassified method call (exec / Vec-String accessor) is out of scope →
    /// an `Err`, never a silent wrong encoding (REQ-1 / R-CODE-2).
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

    /// A bare index over a non-slice, non-fixed-array base has no scalar-value
    /// denotation in the frozen subset → an `Err`.
    #[test]
    fn non_slice_index_is_unsupported() {
        let e = Expr::Index {
            base: Box::new(path("xs")),
            index: IndexArg::Single(Box::new(path("i"))),
        };
        // `xs` not declared a slice → unsupported.
        assert!(matches!(
            exec_ref_value(&e, &ExecRefCtx::default()),
            Err(RefEncodeError::Unsupported(_))
        ));
    }
}
