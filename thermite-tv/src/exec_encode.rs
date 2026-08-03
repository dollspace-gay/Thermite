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
use std::fmt;

use thermite_syntax::ast::{BinOp, Block, Expr, IndexArg, PrimType, Type, UnaryOp};

/// One exact field declaration for a named aggregate in an exec/body-TV frame.
/// The reference encoder uses it to recover bounded field types in spec position
/// and rejects aggregate construction when no declaration was framed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecStructFieldDecl {
    pub struct_path: String,
    pub field: String,
    pub type_str: String,
}

impl ExecStructFieldDecl {
    pub fn new(
        struct_path: impl Into<String>,
        field: impl Into<String>,
        type_str: impl Into<String>,
    ) -> Self {
        Self {
            struct_path: struct_path.into(),
            field: field.into(),
            type_str: type_str.into(),
        }
    }
}

/// Exact executable signature for a framed Thermite callee. It supplies the
/// bounded parameter types needed when a body reference call appears in Verus spec
/// position, and the return type needed for subsequent field projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecCallDecl {
    pub name: String,
    pub param_types: Vec<String>,
    pub ret_type: String,
}

impl ExecCallDecl {
    pub fn new(
        name: impl Into<String>,
        param_types: Vec<String>,
        ret_type: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            param_types,
            ret_type: ret_type.into(),
        }
    }
}

/// An failure to encode a construct outside the pure-exec subset (REQ-1).
/// The exec reference encoder never panics and never silently emits a wrong
/// encoding: an unsupported construct is a real `Err` carrying the offending shape
/// (R-CODE-2 / R-APG-1). A silent wrong encoding would compare a wrong reference
/// and either spuriously pass or spuriously fail. The one frozen method form is
/// parameter-slice `.len()`; other method calls / Vec-String accessors are out of
/// scope for step 2.1 (the #154/#156 territory) and produce an
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
    /// element value the production `xs[i]` computes); a non-slice base is indexed
    /// verbatim.
    slice_bound: BTreeSet<String>,
    /// Bare exec parameter names whose source type is a named enum/ADT. The
    /// independent `is` reference uses this declaration to qualify a bare variant;
    /// it never guesses a type from the variant spelling.
    named_bound: BTreeMap<String, String>,
    /// Exact aggregate path -> field name -> emitted bounded type.
    struct_fields: BTreeMap<String, BTreeMap<String, String>>,
    /// Exact callee name -> executable signature.
    calls: BTreeMap<String, ExecCallDecl>,
    /// Exact return type of the surrounding obligation, when framed.
    result_type: Option<String>,
    /// Named aggregate returned by the surrounding obligation, when any.
    result_struct: Option<String>,
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
            named_bound: BTreeMap::new(),
            struct_fields: BTreeMap::new(),
            calls: BTreeMap::new(),
            result_type: None,
            result_struct: None,
        }
    }

    /// A context carrying both slice parameters and exact bare-name → named-type
    /// bindings for enum discriminant tests.
    pub fn with_bounds<I, S, J, N, T>(slices: I, named: J) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
        J: IntoIterator<Item = (N, T)>,
        N: Into<String>,
        T: Into<String>,
    {
        ExecRefCtx {
            slice_bound: slices.into_iter().map(Into::into).collect(),
            named_bound: named
                .into_iter()
                .map(|(name, ty)| (name.into(), ty.into()))
                .collect(),
            struct_fields: BTreeMap::new(),
            calls: BTreeMap::new(),
            result_type: None,
            result_struct: None,
        }
    }

    /// Add the aggregate declarations available in this exact obligation frame.
    pub fn with_struct_fields<I>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = ExecStructFieldDecl>,
    {
        for field in fields {
            self.struct_fields
                .entry(field.struct_path)
                .or_default()
                .insert(field.field, field.type_str);
        }
        self
    }

    /// Add exact executable call signatures available in this obligation frame.
    pub fn with_calls<I>(mut self, calls: I) -> Self
    where
        I: IntoIterator<Item = ExecCallDecl>,
    {
        self.calls = calls
            .into_iter()
            .map(|call| (call.name.clone(), call))
            .collect();
        self
    }

    /// Record the surrounding result type. Numeric references are narrowed back to
    /// this bounded type in spec position; a framed aggregate is projected.
    pub fn with_result_type(mut self, result: impl Into<String>) -> Self {
        let result = result.into();
        self.result_type = Some(result.clone());
        if self.struct_fields.contains_key(&result) {
            self.result_struct = Some(result);
        }
        self
    }

    fn is_slice_bound(&self, name: &str) -> bool {
        self.slice_bound.contains(name)
    }

    fn named_type_bound(&self, name: &str) -> Option<&str> {
        self.named_bound.get(name).map(String::as_str)
    }

    fn struct_field_type(&self, path: &str, field: &str) -> Option<&str> {
        self.struct_fields
            .get(path)
            .and_then(|fields| fields.get(field))
            .map(String::as_str)
    }

    fn result_struct_fields(&self) -> Option<&BTreeMap<String, String>> {
        self.result_struct
            .as_ref()
            .and_then(|name| self.struct_fields.get(name))
    }

    fn expr_type<'a>(&'a self, expr: &Expr) -> Option<&'a str> {
        match expr {
            Expr::Path(path) if path.len() == 1 => self.named_type_bound(&path[0]),
            Expr::StructLit { path, .. } => {
                let path = path.join("::");
                self.struct_fields.contains_key(&path).then_some(())?;
                self.struct_fields
                    .get_key_value(&path)
                    .map(|(name, _)| name.as_str())
            }
            Expr::Field { receiver, name } => {
                let receiver_ty = self.expr_type(receiver)?;
                self.struct_field_type(receiver_ty, name)
            }
            Expr::Call { callee, .. } => {
                let Expr::Path(path) = callee.as_ref() else {
                    return None;
                };
                self.calls
                    .get(&path.join("::"))
                    .map(|call| call.ret_type.as_str())
            }
            _ => None,
        }
    }

    fn numeric_result_type(&self) -> Option<&str> {
        match self.result_type.as_deref() {
            Some(ty @ ("u8" | "u16" | "u32" | "u64" | "usize")) => Some(ty),
            _ => None,
        }
    }

    fn for_value_type(&self, ty: &str) -> Self {
        let mut nested = self.clone();
        nested.result_type = Some(ty.to_string());
        nested.result_struct = nested
            .struct_fields
            .contains_key(ty)
            .then(|| ty.to_string());
        nested
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
/// - `.len()` on a parameter bound as a slice in [`ExecRefCtx`];
/// - pure `if` expressions whose two branch blocks contain only a tail value;
/// - indexing ([`Expr::Index`] single-element over a slice param → `xs[i as int]`,
///   the bounded element value).
///
/// Anything else (another method call, a Vec/String accessor, a struct literal, a
/// statement-bearing `if`, a `match`, a closure, …) is an
/// [`RefEncodeError::Unsupported`] (never a
/// panic, never a silent wrong encoding — #154/#156 territory).
pub fn exec_ref_value(expr: &Expr, ctx: &ExecRefCtx) -> Result<String, RefEncodeError> {
    encode(expr, ctx)
}

/// Encode an exact result relation for an exec expression. Aggregate construction
/// is compared field-by-field, while another aggregate-valued expression is
/// projected through every declared result field.
pub fn exec_ref_ensures(
    expr: &Expr,
    result_name: &str,
    ctx: &ExecRefCtx,
) -> Result<String, RefEncodeError> {
    if let Expr::StructLit { path, fields } = expr {
        let head = path.join("::");
        let mut relations = Vec::new();
        for (name, value) in encode_struct_fields(path, fields, ctx)? {
            let ty = ctx.struct_field_type(&head, &name).ok_or_else(|| {
                RefEncodeError::Unsupported(format!(
                    "struct literal field `{head}.{name}` has no exact aggregate frame declaration"
                ))
            })?;
            relations.extend(aggregate_field_relations(
                &format!("{result_name}.{name}"),
                &value,
                ty,
            ));
        }
        return Ok(relations.join(" && "));
    }
    let reference = encode(expr, ctx)?;
    Ok(exec_ref_ensures_value(&reference, result_name, ctx))
}

/// Relate a result to an already encoded reference value. This is used by body-TV,
/// whose state-threading produces a reference string rather than one source node.
pub fn exec_ref_ensures_value(reference: &str, result_name: &str, ctx: &ExecRefCtx) -> String {
    if let Some(fields) = ctx.result_struct_fields() {
        let mut relations = Vec::new();
        for (field, ty) in fields {
            relations.extend(aggregate_field_relations(
                &format!("{result_name}.{field}"),
                &format!("({reference}).{field}"),
                ty,
            ));
        }
        return relations.join(" && ");
    }
    let reference = match ctx.result_type.as_deref() {
        Some(ty @ ("u8" | "u16" | "u32" | "u64" | "usize")) => {
            format!("(({reference}) as {ty})")
        }
        _ => format!("({reference})"),
    };
    format!("{result_name} == {reference}")
}

fn aggregate_field_relations(lhs: &str, rhs: &str, ty: &str) -> Vec<String> {
    if ty.starts_with("TFixedArray8") {
        return (0..8)
            .map(|index| format!("{lhs}.spec_get({index}) == ({rhs}).spec_get({index})"))
            .collect();
    }
    vec![format!("{lhs} == {rhs}")]
}

fn encode(expr: &Expr, ctx: &ExecRefCtx) -> Result<String, RefEncodeError> {
    match expr {
        Expr::IntLit { value, .. } => Ok(value.to_string()),
        Expr::BoolLit(b) => Ok(b.to_string()),
        Expr::Path(segments) => encode_path(segments),
        Expr::Binary { op, lhs, rhs } => encode_binary(*op, lhs, rhs, ctx),
        Expr::Unary { op, expr } => encode_unary(*op, expr, ctx),
        Expr::Call { callee, args } => encode_call(callee, args, ctx),
        Expr::MethodCall {
            receiver,
            name,
            args,
        } => encode_method_call(receiver, name, args, ctx),
        Expr::If { cond, then, else_ } => encode_pure_if(cond, then, else_, ctx),
        Expr::Index { base, index } => encode_index(base, index, ctx),
        Expr::Cast { expr, ty } => encode_cast(expr, ty, ctx),
        Expr::Is { scrutinee, variant } => encode_is(scrutinee, variant, ctx),
        Expr::Field { receiver, name } => {
            let receiver = encode(receiver, ctx)?;
            Ok(format!("{receiver}.{name}"))
        }
        Expr::StructLit { path, fields } => encode_struct_literal(path, fields, ctx),
        other => Err(RefEncodeError::Unsupported(node_kind(other))),
    }
}

/// Independently encode a named struct/struct-variant construction. Field order and
/// names are preserved exactly, and every initializer is recursively encoded by the
/// bounded exec reference. A missing type path is rejected rather than guessed.
fn encode_struct_literal(
    path: &[String],
    fields: &[(String, Expr)],
    ctx: &ExecRefCtx,
) -> Result<String, RefEncodeError> {
    let head = path.join("::");
    let fields = encode_struct_fields(path, fields, ctx)?
        .into_iter()
        .map(|(name, value)| format!("{name}: {value}"))
        .collect::<Vec<_>>();
    Ok(format!("{head} {{ {} }}", fields.join(", ")))
}

fn encode_struct_fields(
    path: &[String],
    fields: &[(String, Expr)],
    ctx: &ExecRefCtx,
) -> Result<Vec<(String, String)>, RefEncodeError> {
    if path.is_empty() {
        return Err(RefEncodeError::Unsupported(
            "struct literal with an empty type path".to_string(),
        ));
    }
    let head = path.join("::");
    fields
        .iter()
        .map(|(name, value)| {
            let ty = ctx.struct_field_type(&head, name).ok_or_else(|| {
                RefEncodeError::Unsupported(format!(
                    "struct literal field `{head}.{name}` has no exact aggregate frame declaration"
                ))
            })?;
            let value_ctx = ctx.for_value_type(ty);
            let value = bounded_field_value(&encode(value, &value_ctx)?, ty);
            Ok((name.clone(), value))
        })
        .collect()
}

fn bounded_field_value(value: &str, ty: &str) -> String {
    match ty {
        "u8" | "u16" | "u32" | "u64" | "usize" => format!("({value}) as {ty}"),
        _ => value.to_string(),
    }
}

/// Independently encode an exec-position enum discriminant. Production lowers
/// `order is Release` to `matches!(order, Ordering::Release { .. })`; the reference
/// reconstructs that pattern only when the scrutinee is a bare parameter with an
/// exact named-type binding in the obligation frame. A qualified source variant is
/// preserved; a bare one is qualified by the declared scrutinee type.
fn encode_is(
    scrutinee: &Expr,
    variant: &[String],
    ctx: &ExecRefCtx,
) -> Result<String, RefEncodeError> {
    let Expr::Path(segments) = scrutinee else {
        return Err(RefEncodeError::Unsupported(
            "is-test over a non-bare scrutinee".to_string(),
        ));
    };
    if segments.len() != 1 || variant.is_empty() {
        return Err(RefEncodeError::Unsupported(
            "is-test without a bare framed scrutinee and non-empty variant".to_string(),
        ));
    }
    let scrutinee_name = &segments[0];
    let head = if variant.len() > 1 {
        variant.join("::")
    } else {
        let ty = ctx.named_type_bound(scrutinee_name).ok_or_else(|| {
            RefEncodeError::Unsupported(format!(
                "is-test scrutinee `{scrutinee_name}` has no named-type frame binding"
            ))
        })?;
        format!("{ty}::{}", variant[0])
    };
    Ok(format!("matches!({scrutinee_name}, {head} {{ .. }})"))
}

/// Independently encode the value semantics of a pure `if` expression. Branch
/// blocks with statements remain outside the per-expression subset: their state
/// sequencing belongs to `exec_stmt_encode`, while this encoder admits exactly a
/// condition and two recursively encoded tail values.
fn encode_pure_if(
    cond: &Expr,
    then: &Block,
    else_: &Block,
    ctx: &ExecRefCtx,
) -> Result<String, RefEncodeError> {
    let c = encode(cond, ctx)?;
    let t = pure_branch_tail(then, "then")?;
    let e = pure_branch_tail(else_, "else")?;
    let mut t = encode(t, ctx)?;
    let mut e = encode(e, ctx)?;
    if let Some(ty) = ctx.numeric_result_type() {
        t = bounded_field_value(&t, ty);
        e = bounded_field_value(&e, ty);
    }
    Ok(format!("(if {c} {{ {t} }} else {{ {e} }})"))
}

fn pure_branch_tail<'a>(block: &'a Block, branch: &str) -> Result<&'a Expr, RefEncodeError> {
    if !block.stmts.is_empty() {
        return Err(RefEncodeError::Unsupported(format!(
            "statement-bearing `{branch}` branch in if expression"
        )));
    }
    block.tail.as_deref().ok_or_else(|| {
        RefEncodeError::Unsupported(format!("value-less `{branch}` branch in if expression"))
    })
}

/// A path reference: a var or a `::`-qualified name. A pure exec value path is a
/// free var (a body param) or a constant path.
fn encode_path(segments: &[String]) -> Result<String, RefEncodeError> {
    if segments.is_empty() {
        return Err(RefEncodeError::Unsupported("empty path".to_string()));
    }
    Ok(segments.join("::"))
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

/// An exactly framed call `f(args)` (REQ-1). In exec position the callee is emitted
/// verbatim, but its value semantics come from the exact executable signature and
/// contract included in the obligation. An unframed or non-path callee is an
/// `Err`; accepting it would let an undeclared user call acquire guessed argument
/// and result semantics.
fn encode_call(callee: &Expr, args: &[Expr], ctx: &ExecRefCtx) -> Result<String, RefEncodeError> {
    let Expr::Path(segments) = callee else {
        return Err(RefEncodeError::Unsupported(format!(
            "call with a non-path callee ({})",
            node_kind(callee)
        )));
    };
    let name = segments.join("::");
    let signature = ctx.calls.get(&name).ok_or_else(|| {
        RefEncodeError::Unsupported(format!(
            "call `{name}` has no exact executable declaration in the obligation frame"
        ))
    })?;
    if signature.param_types.len() != args.len() {
        return Err(RefEncodeError::Unsupported(format!(
            "call `{name}` has {} arguments but its exact frame declares {}",
            args.len(),
            signature.param_types.len()
        )));
    }
    let encoded_args = args
        .iter()
        .enumerate()
        .map(|(index, argument)| {
            let expected = &signature.param_types[index];
            let argument_ctx = ctx.for_value_type(expected);
            let value = encode(argument, &argument_ctx)?;
            Ok(typed_call_argument(&value, expected))
        })
        .collect::<Result<Vec<_>, RefEncodeError>>()?;
    Ok(format!("{name}({})", encoded_args.join(", ")))
}

fn typed_call_argument(value: &str, ty: &str) -> String {
    match ty {
        "u8" | "u16" | "u32" | "u64" | "usize" => format!("({value}) as {ty}"),
        _ => value.to_string(),
    }
}

/// The sole frozen exec-method reference is `xs.len()` for a name the obligation
/// frame binds as a slice. Its bounded `usize` result is exactly the value returned
/// by the production slice-length call. Receiver shape and arity are checked here;
/// accepting a general `.len()` would silently assign slice semantics to Vec/String
/// or user-defined methods.
fn encode_method_call(
    receiver: &Expr,
    name: &str,
    args: &[Expr],
    ctx: &ExecRefCtx,
) -> Result<String, RefEncodeError> {
    if name == "get"
        && args.len() == 1
        && ctx
            .expr_type(receiver)
            .is_some_and(|ty| ty.starts_with("TFixedArray8"))
    {
        let receiver = encode(receiver, ctx)?;
        let index = encode_index_value(&args[0], ctx)?;
        return Ok(format!("{receiver}.spec_get({index})"));
    }
    if name == "len" && args.is_empty() {
        if let Expr::Path(segments) = receiver {
            if segments.len() == 1 && ctx.is_slice_bound(&segments[0]) {
                return Ok(format!("{}.len()", segments[0]));
            }
        }
    }
    Err(RefEncodeError::Unsupported(
        "method call (the frozen exec subset admits fixed-array `.get(i)` and `.len()` on a parameter slice)".to_string(),
    ))
}

/// `xs[i]` in exec position (REQ-1). A single index over a slice-bound base is the
/// bounded element value — the spec view `xs[i as int]` (in an exec fn the
/// production indexes `xs[i]` with `i: usize`, whose value equals the spec view
/// `xs[i as int]`; the obligation `ensures result == xs[i as int]` is the
/// element-value equality, grounded `exec-tv.md` AC-5/E4). A `RangeTo`/`RangeFrom`/
/// `Range` slice index produces a sub-slice (not a scalar value), outside the
/// pure-exec scalar-value subset of step 2.1 → an `Err`. A non-slice base
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

    #[test]
    fn pure_if_expression_is_encoded_recursively() {
        let e = Expr::If {
            cond: Box::new(bin(BinOp::Lt, path("first"), int(64))),
            then: Block {
                stmts: vec![],
                tail: Some(Box::new(path("first"))),
            },
            else_: Block {
                stmts: vec![],
                tail: Some(Box::new(int(64))),
            },
        };
        assert_eq!(
            exec_ref_value(&e, &ExecRefCtx::default()).unwrap(),
            "(if (first < 64) { first } else { 64 })"
        );
    }

    #[test]
    fn statement_bearing_if_branch_is_unsupported() {
        let e = Expr::If {
            cond: Box::new(path("ready")),
            then: Block {
                stmts: vec![thermite_syntax::ast::Stmt::Expr(int(1))],
                tail: Some(Box::new(int(2))),
            },
            else_: Block {
                stmts: vec![],
                tail: Some(Box::new(int(3))),
            },
        };
        assert!(matches!(
            exec_ref_value(&e, &ExecRefCtx::default()),
            Err(RefEncodeError::Unsupported(_))
        ));
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

    #[test]
    fn parameter_slice_len_has_a_bounded_exec_reference() {
        let e = Expr::MethodCall {
            receiver: Box::new(path("xs")),
            name: "len".to_string(),
            args: vec![],
        };
        let ctx = ExecRefCtx::with_slice_bound(["xs"]);
        assert_eq!(exec_ref_value(&e, &ctx).unwrap(), "xs.len()");
    }

    /// A non-slice method call (exec / Vec-String accessor) is out of scope for
    /// step 2.1 → an `Err`, never a silent wrong encoding (REQ-1 / R-CODE-2).
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
    /// frozen subset → an `Err`.
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

    #[test]
    fn user_call_without_an_exact_declaration_is_unsupported() {
        let call = Expr::Call {
            callee: Box::new(path("advance")),
            args: vec![path("state")],
        };
        assert!(matches!(
            exec_ref_value(&call, &ExecRefCtx::default()),
            Err(RefEncodeError::Unsupported(reason))
                if reason.contains("no exact executable declaration")
        ));
    }

    #[test]
    fn exactly_declared_user_call_is_encoded_at_declared_argument_types() {
        let call = Expr::Call {
            callee: Box::new(path("advance")),
            args: vec![path("state"), int(1)],
        };
        let ctx = ExecRefCtx::default().with_calls([ExecCallDecl::new(
            "advance",
            vec!["Step".to_string(), "u64".to_string()],
            "Step",
        )]);
        assert_eq!(
            exec_ref_value(&call, &ctx).unwrap(),
            "advance(state, (1) as u64)"
        );
    }
}
