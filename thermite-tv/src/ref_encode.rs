//! The INDEPENDENT reference encoder for the SpecTherm contract sublanguage
//! (`.design/verified/contract-tv.md` REQ-1; `thermite-design.md` §4.2).
//!
//! [`ref_contract_pred`] maps a contract-position [`Expr`] to a Verus predicate
//! STRING. It is the small, declarative, human-auditable re-implementation of
//! the contract sublanguage's meaning — authored AGAINST `thermite-design.md`
//! §4.2 + the frozen `thermite_spec::REGISTRY.verus_l3`, NOT against the
//! production lowerer.
//!
//! ## THE INDEPENDENCE BOUNDARY (the whole point — REQ-1 HARD CONSTRAINT)
//!
//! This module MUST NOT call `thermite_lower::lower::lower_expr` or any
//! production lowering symbol — `thermite-tv` does not even depend on
//! `thermite-lower` (the dep graph makes reuse a compile error, AC-6). The check
//! `assert(P_production <==> P_reference)` is N-version differential validation:
//! agreement is EVIDENCE, not proof. If this encoder reused `lower_expr` the
//! check would be vacuous (`assert(X <==> X)` always verifies).
//!
//! - **RE-IMPLEMENTED here (the infidelity surface):** the spec-context rewrites
//!   where production fidelity bugs live — comparison/connective binop map
//!   (the `==`/`<=` distinction, the canonical teeth case F1), the slice→`@`
//!   view, the method→`spec_*` byte-view dispatch keyed on the RECEIVER's shape
//!   (`.byte_at(i)`/`.len()` — the #127 class, F3), and the integer cast→`as
//!   nat`/`as int` with the #122 paren discipline.
//! - **REUSED (the shared frozen ground truth — reuse is correct):**
//!   `thermite_spec::lookup(name).verus_l3` for the 8 combinators. The registry
//!   IS the external combinator spec, not a production artifact; the combinator
//!   ARGUMENT rewrites (the predicate closure, the slice `@`-view) are still
//!   RE-implemented here, so divergence over them is still caught.
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (independent reference encoder) | SHIPPED | `pub fn ref_contract_pred` here; non-test consumer `thermite_tv::obligation::equivalence_obligation` (`obligation.rs`); verified by `thermite-tv/tests/teeth.rs` F1–F4 against real verus (faithful VERIFIES, infidel COUNTEREXAMPLE). Depends on `thermite-syntax` + `thermite-spec` ONLY — no `thermite-lower` (`Cargo.toml`), so the independence is a compile constraint (AC-6). |

use std::collections::BTreeSet;
use std::fmt;

use thermite_syntax::ast::{BinOp, Expr, IndexArg, UnaryOp};

/// An honest failure to encode a construct outside the frozen contract
/// sublanguage (REQ-1). The reference encoder NEVER panics and NEVER silently
/// emits a wrong encoding: an unsupported construct is a real `Err` carrying the
/// offending shape (R-CODE-2 / R-APG-1). A silent wrong encoding would defeat
/// the entire point — TV would compare a wrong reference and either spuriously
/// pass or spuriously fail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefEncodeError {
    /// A construct the contract sublanguage does not admit (a statement, a
    /// match in spec position the encoder does not cover, etc.). Carries a short
    /// description of the offending node so a human can see exactly what bit.
    Unsupported(String),
    /// A combinator call whose callee name is not in the frozen
    /// `thermite_spec::REGISTRY` and is not a plain spec-fn call shape we can
    /// encode. (A non-registry path callee IS encodable as a plain spec-fn call;
    /// this fires only for a shape the encoder genuinely cannot represent.)
    UnknownCallee(String),
}

impl fmt::Display for RefEncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RefEncodeError::Unsupported(what) => {
                write!(f, "ref_encode: unsupported contract construct: {what}")
            }
            RefEncodeError::UnknownCallee(name) => {
                write!(f, "ref_encode: cannot encode callee shape: {name}")
            }
        }
    }
}

impl std::error::Error for RefEncodeError {}

/// The reference-encoding context (REQ-1). Carries which free names are bound in
/// the obligation as a `Seq<_>` ALREADY (so the slice→`@` view is the identity:
/// a param bound as `xs: Seq<u32>` IS its own view — emitting `xs@` would be a
/// type error / a spurious coercion mismatch). This is the load-bearing answer
/// to THE COERCION RISK flagged by the doc author: the reference must infer the
/// SAME `@`-view shape the faithful production column shows. In the per-clause
/// obligation, slice params are bound directly as `Seq`, so the encoder treats
/// them as already-viewed and emits the bare name (matching the faithful
/// `spec_sum(xs)` rather than a spurious `spec_sum(xs@)`).
#[derive(Debug, Clone, Default)]
pub struct RefCtx {
    /// Names that are bound in the obligation directly as a `Seq<_>` view. For
    /// such a name the `@`-view rewrite is the identity (the name IS the view).
    /// A name NOT in this set that is used in a slice position gets the explicit
    /// `@` suffix (the `&[T]`→`Seq` view at use sites).
    seq_bound: BTreeSet<String>,
    /// Names that are a bounded integer (`u64`/`u32`/`usize`) and MUST be coerced
    /// `as nat` when they appear as a top-level operand of a comparison against a
    /// `nat`-valued term (a `nat`-returning spec-fn call). This RE-implements,
    /// independently and declaratively, production's `lower_nat_equality` shape
    /// coercion (the golden `ens result == spec_sum(xs)` lowers to `result as nat
    /// == spec_sum(xs@)`). THE COERCION FIX (the doc author's #1 flagged risk):
    /// the reference must infer the SAME `as nat` coercion the faithful column
    /// shows, else the faithful obligation fails on a coercion mismatch (a
    /// spurious counterexample) rather than a meaning bug. Inferring it here
    /// (rather than importing production's rule) keeps independence — a
    /// production coercion bug would still be caught.
    nat_coerce: BTreeSet<String>,
}

impl RefCtx {
    /// A context in which the named free vars are bound directly as `Seq<_>`
    /// views (so their slice→`@` rewrite is the identity). This mirrors the
    /// obligation's parameter binding (the F1/F3 frames bind `xs`/`s` as `Seq`).
    pub fn with_seq_bound<I, S>(names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        RefCtx {
            seq_bound: names.into_iter().map(Into::into).collect(),
            nat_coerce: BTreeSet::new(),
        }
    }

    /// Declare bounded-int names that must be coerced `as nat` when compared
    /// against a `nat`-valued term (THE COERCION FIX — see [`RefCtx::nat_coerce`]).
    pub fn with_nat_coerce<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.nat_coerce = names.into_iter().map(Into::into).collect();
        self
    }

    fn is_seq_bound(&self, name: &str) -> bool {
        self.seq_bound.contains(name)
    }

    fn needs_nat_coerce(&self, name: &str) -> bool {
        self.nat_coerce.contains(name)
    }
}

/// Encode a contract-position [`Expr`] to a Verus predicate STRING, independently
/// of the production lowerer (REQ-1). Covers exactly the frozen contract
/// sublanguage of `thermite-design.md` §4.2:
///
/// - comparisons + logical connectives ([`Expr::Binary`] over the `Eq`/`Ne`/`Lt`/
///   `Le`/`Gt`/`Ge`/`And`/`Or` ops, plus arithmetic in subterms) — a faithful
///   1-to-1 binop map (`binop_str`), RE-stated independently so a production
///   binop bug (`==`→`<=`, F1) is caught;
/// - [`Expr::Unary`] `!` (logical/bitwise-not);
/// - [`Expr::Path`] vars (incl. `result`) + `old(x)`;
/// - named `spec fn` calls ([`Expr::Call`] with a path callee → `name(args)`);
/// - the 8 frozen combinators ([`Expr::Call`] whose callee name is in
///   `thermite_spec::lookup` → emitted as a call to the registry `verus_l3`
///   `spec fn`, with the arguments RE-encoded here — the predicate closure +
///   slice `@`-view independently, F2);
/// - [`Expr::MethodCall`] with the spec-context rewrite dispatched on the
///   RECEIVER's shape NOT the name (`.len()`→`.len()`, the byte-view
///   `.byte_at(i)`→`recv[i]` over a `Seq<u8>` view — the #127 class, F3);
/// - [`Expr::Index`] (`a[i]` / `a[..i]`→`a.subrange(0, i as int)`);
/// - [`Expr::Cast`]→`(inner) as nat`/`as int` with the #122 paren discipline.
///
/// Anything else is an honest [`RefEncodeError`] (NEVER a panic, NEVER a silent
/// wrong encoding).
pub fn ref_contract_pred(expr: &Expr, ctx: &RefCtx) -> Result<String, RefEncodeError> {
    encode(expr, ctx)
}

fn encode(expr: &Expr, ctx: &RefCtx) -> Result<String, RefEncodeError> {
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
        Expr::Index { base, index } => encode_index(base, index, ctx),
        Expr::Cast { expr, ty } => encode_cast(expr, ty, ctx),
        Expr::Field { receiver, name } => {
            let r = encode(receiver, ctx)?;
            Ok(format!("{r}.{name}"))
        }
        Expr::TupleProj { receiver, index } => {
            let r = encode(receiver, ctx)?;
            Ok(format!("{r}.{index}"))
        }
        Expr::Is { scrutinee, variant } => {
            let s = encode(scrutinee, ctx)?;
            Ok(format!("({s} is {})", variant.join("::")))
        }
        other => Err(RefEncodeError::Unsupported(node_kind(other))),
    }
}

/// A path reference: a var (incl. `result`), a `::`-qualified name, or the
/// special `old(x)` (parsed as a `Call` of the path `old`; a bare `old` path is
/// never a clause leaf). `result`/`old(_)`-bound values are treated as free vars
/// — they are bound as distinct obligation params (REQ-2).
fn encode_path(segments: &[String]) -> Result<String, RefEncodeError> {
    if segments.is_empty() {
        return Err(RefEncodeError::Unsupported("empty path".to_string()));
    }
    Ok(segments.join("::"))
}

/// The faithful 1-to-1 binary-operator map (`thermite-design.md` §4.2). RE-stated
/// here independently of the production `binop in lower.rs`: the `==`-vs-`<=`
/// distinction is the canonical teeth case (F1) — if this imported production's
/// map, a production binop bug would be invisible.
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

fn encode_binary(
    op: BinOp,
    lhs: &Expr,
    rhs: &Expr,
    ctx: &RefCtx,
) -> Result<String, RefEncodeError> {
    // THE COERCION FIX (declarative `lower_nat_equality` re-implementation): in a
    // COMPARISON where one operand is a `nat`-valued term (a nat-returning
    // spec-fn call) and the other is a bounded-int name declared in
    // `nat_coerce`, coerce the int operand `as nat` (matching the golden `result
    // as nat == spec_sum(xs@)`). This only fires for a comparison op AND only
    // when the other side is nat-valued — an int-vs-int comparison (F4's `a ==
    // b`) is left bare.
    if is_comparison(op) {
        let lhs_nat = is_nat_valued(lhs);
        let rhs_nat = is_nat_valued(rhs);
        let l = encode_comparison_operand(lhs, rhs_nat, ctx)?;
        let r = encode_comparison_operand(rhs, lhs_nat, ctx)?;
        return Ok(format!("({l} {} {r})", binop_str(op)));
    }
    let l = encode(lhs, ctx)?;
    let r = encode(rhs, ctx)?;
    // Parenthesize the whole binary so precedence is explicit at every level
    // (the #122 paren discipline generalized: a sub-predicate never silently
    // re-associates). Z3 sees the SAME term regardless of nesting.
    Ok(format!("({l} {} {r})", binop_str(op)))
}

/// Is `op` a comparison (the operators whose operands may need a `nat`
/// coercion)?
fn is_comparison(op: BinOp) -> bool {
    matches!(
        op,
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge
    )
}

/// Is `expr` a `nat`-valued term — i.e. a call to a `nat`-returning spec fn? The
/// frozen `nat`-returning forms are the recursive `spec fn … -> nat` (e.g.
/// `spec_sum`, `count_where`). We detect it structurally: a call whose callee is
/// a path. (A combinator-`count_where`/spec-fn call is the nat term in a
/// comparison.) This is the declarative side of the coercion inference.
fn is_nat_valued(expr: &Expr) -> bool {
    matches!(expr, Expr::Call { callee, .. } if matches!(callee.as_ref(), Expr::Path(_)))
}

/// Encode a comparison operand, applying the `as nat` coercion when the operand
/// is a bounded-int name in `nat_coerce` AND the OTHER operand is `nat`-valued
/// (THE COERCION FIX). Otherwise an ordinary sub-expression.
fn encode_comparison_operand(
    operand: &Expr,
    other_is_nat: bool,
    ctx: &RefCtx,
) -> Result<String, RefEncodeError> {
    if other_is_nat {
        if let Expr::Path(segments) = operand {
            if segments.len() == 1 && ctx.needs_nat_coerce(&segments[0]) {
                return Ok(format!("{} as nat", segments[0]));
            }
        }
    }
    encode(operand, ctx)
}

fn encode_unary(op: UnaryOp, inner: &Expr, ctx: &RefCtx) -> Result<String, RefEncodeError> {
    let i = encode(inner, ctx)?;
    match op {
        UnaryOp::Not => Ok(format!("(!{i})")),
    }
}

/// A free-form call `f(args)`. Three cases, dispatched on the callee:
///
/// 1. `old(x)` — the prev-state reference. Bound as a distinct obligation param
///    `old_x` (REQ-2), so we emit `old_x` (NOT a Verus `old(_)`, which is only
///    valid on a `&mut` param — the obligation binds the value directly).
/// 2. a FROZEN combinator (callee name in `thermite_spec::lookup`) — emit a call
///    to the registry `verus_l3` `spec fn` by name, with the args RE-encoded
///    here (the slice `@`-view + the predicate closure independently, F2).
/// 3. a named spec-fn call — `name(encoded args)`.
fn encode_call(callee: &Expr, args: &[Expr], ctx: &RefCtx) -> Result<String, RefEncodeError> {
    let Expr::Path(segments) = callee else {
        return Err(RefEncodeError::UnknownCallee(node_kind(callee)));
    };
    let name = segments.join("::");

    // (1) old(x) — a single-arg `old` call.
    if name == "old" {
        if args.len() != 1 {
            return Err(RefEncodeError::Unsupported(format!(
                "old/{} (expected exactly 1 arg)",
                args.len()
            )));
        }
        let inner = encode(&args[0], ctx)?;
        // The prev-state value is bound as a distinct param `old_<name>`; a bare
        // `old(x)` over a path `x` binds `old_x`.
        let mangled = inner.replace("::", "_");
        return Ok(format!("old_{mangled}"));
    }

    // (2) a frozen combinator — REUSE the registry name (its verus_l3 def is the
    // shared ground truth), RE-encode the args here.
    if thermite_spec::lookup(&name).is_some() {
        return encode_combinator_call(&name, args, ctx);
    }

    // (3) a named spec-fn call.
    let encoded_args = args
        .iter()
        .map(|a| encode_call_arg(a, ctx))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(format!("{name}({})", encoded_args.join(", ")))
}

/// Encode a frozen-combinator call (F2). The combinator's `verus_l3` body is the
/// SHARED frozen ground truth (`thermite_spec::lookup(name).verus_l3`), reused by
/// BOTH production and this reference — sharing it is NOT a loss of independence
/// (the registry is the external spec). What this RE-implements independently is
/// the combinator's ARGUMENTS: the slice `@`-view and the predicate closure
/// (`|x| <body>`) — exactly where a combinator-argument fidelity bug lives.
fn encode_combinator_call(
    name: &str,
    args: &[Expr],
    ctx: &RefCtx,
) -> Result<String, RefEncodeError> {
    let encoded_args = args
        .iter()
        .map(|a| encode_call_arg(a, ctx))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(format!("{name}({})", encoded_args.join(", ")))
}

/// Encode a call argument. A predicate closure `|x| <body>` (a combinator's
/// `Pred` slot) is RE-encoded to a Verus closure `|x: u32| <body>` — the body is
/// encoded by the SAME independent recursion (so a closure-predicate infidelity,
/// F2's `x <= 10` vs `x < 10`, is caught). Everything else is an ordinary
/// sub-expression (the slice gets its `@`-view via [`encode`]'s path/var rules).
fn encode_call_arg(arg: &Expr, ctx: &RefCtx) -> Result<String, RefEncodeError> {
    match arg {
        Expr::Closure { params, body } => {
            if params.len() != 1 {
                return Err(RefEncodeError::Unsupported(format!(
                    "predicate closure with {} params (expected 1)",
                    params.len()
                )));
            }
            // The combinator predicate closures range over the slice element
            // type, which is `u32` for the frozen `Seq<u32>` combinators (§4.2 /
            // `REGISTRY.verus_l3`). Bind the closure param at `u32` so the
            // emitted closure matches the registry `spec_fn(u32) -> bool` shape.
            let body_s = encode(body, ctx)?;
            Ok(format!("|{}: u32| {body_s}", params[0]))
        }
        other => encode_slice_arg(other, ctx),
    }
}

/// Encode a slice-position argument with the slice→`@` view rewrite (§4.2). A
/// bare `Expr::Path` that is NOT already bound as a `Seq` in the obligation gets
/// the explicit `@` suffix (the `&[T]`→`Seq` view); a `Seq`-bound name is the
/// identity (THE COERCION FIX — emitting `xs` not `xs@` when `xs: Seq` is bound).
fn encode_slice_arg(arg: &Expr, ctx: &RefCtx) -> Result<String, RefEncodeError> {
    if let Expr::Path(segments) = arg {
        if segments.len() == 1 && !ctx.is_seq_bound(&segments[0]) {
            // A slice param NOT bound as a Seq → take its `@`-view at the use
            // site. (Bound-as-Seq → identity, the obligation case.)
            return Ok(format!("{}@", segments[0]));
        }
    }
    encode(arg, ctx)
}

/// A method call in spec position (`thermite-design.md` §4.2; REQ-1). The
/// dispatch is keyed on the RECEIVER's SHAPE not the method name (the #127
/// class): for a `String`/byte receiver, `.byte_at(i)` is the byte-view index
/// `recv[i]` and `.len()` is `recv.len()` over the `@`-view. This RE-implements
/// the spec-context method→`spec_*` byte-view dispatch independently of
/// production, so a misdispatch (a wrong index, F3) is caught.
fn encode_method_call(
    receiver: &Expr,
    name: &str,
    args: &[Expr],
    ctx: &RefCtx,
) -> Result<String, RefEncodeError> {
    let recv = encode_receiver(receiver, ctx)?;
    match name {
        // The byte-view accessor (#127): `s.byte_at(i)` is the i-th byte of the
        // sequence view — `recv[i]`. F3's teeth bite here: a production
        // misdispatch to index `1` for source index `0` differs from this.
        "byte_at" => {
            if args.len() != 1 {
                return Err(RefEncodeError::Unsupported(format!(
                    "byte_at/{} (expected exactly 1 arg)",
                    args.len()
                )));
            }
            let idx = encode_index_value(&args[0], ctx)?;
            Ok(format!("{recv}[{idx}]"))
        }
        // The length accessor: `s.len()` over the sequence view.
        "len" => {
            if !args.is_empty() {
                return Err(RefEncodeError::Unsupported(
                    "len with arguments".to_string(),
                ));
            }
            Ok(format!("{recv}.len()"))
        }
        // A sub-slice view `s.slice(lo, hi)` → `recv.subrange(lo as int, hi as int)`.
        "slice" => {
            if args.len() != 2 {
                return Err(RefEncodeError::Unsupported(format!(
                    "slice/{} (expected exactly 2 args)",
                    args.len()
                )));
            }
            let lo = encode_index_value(&args[0], ctx)?;
            let hi = encode_index_value(&args[1], ctx)?;
            Ok(format!("{recv}.subrange({lo}, {hi})"))
        }
        other => Err(RefEncodeError::Unsupported(format!(
            "spec method `.{other}()` (not in the frozen byte-view set)"
        ))),
    }
}

/// Encode a method-call receiver. A bare slice/string param name takes its
/// `@`-view unless it is bound directly as a `Seq` in the obligation (the same
/// COERCION-matching rule as [`encode_slice_arg`]): F3 binds `s: Seq<u8>`, so the
/// receiver is the bare `s`.
fn encode_receiver(receiver: &Expr, ctx: &RefCtx) -> Result<String, RefEncodeError> {
    if let Expr::Path(segments) = receiver {
        if segments.len() == 1 && !ctx.is_seq_bound(&segments[0]) {
            return Ok(format!("{}@", segments[0]));
        }
    }
    encode(receiver, ctx)
}

/// Encode an index/bound value `i` in a spec position as `<i> as int` (Verus
/// `Seq` indices/subranges are `int`). A bare integer literal stays bare (Verus
/// coerces it); a non-trivial expression is parenthesized then cast (the #122
/// paren discipline).
fn encode_index_value(expr: &Expr, ctx: &RefCtx) -> Result<String, RefEncodeError> {
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

/// `a[i]` / `a[..i]` / `a[i..]` / `a[i..j]` in spec position. A single index is a
/// `Seq` index over the receiver's `@`-view; a range is a `.subrange(..)` (the
/// `&xs[..i]`→`xs@.subrange(0, i as int)` rewrite, §4.2).
fn encode_index(base: &Expr, index: &IndexArg, ctx: &RefCtx) -> Result<String, RefEncodeError> {
    let recv = encode_receiver(base, ctx)?;
    match index {
        IndexArg::Single(i) => {
            let idx = encode_index_value(i, ctx)?;
            Ok(format!("{recv}[{idx}]"))
        }
        IndexArg::RangeTo(hi) => {
            let h = encode_index_value(hi, ctx)?;
            Ok(format!("{recv}.subrange(0, {h})"))
        }
        IndexArg::RangeFrom(lo) => {
            let l = encode_index_value(lo, ctx)?;
            Ok(format!("{recv}.subrange({l}, {recv}.len() as int)"))
        }
        IndexArg::Range(lo, hi) => {
            let l = encode_index_value(lo, ctx)?;
            let h = encode_index_value(hi, ctx)?;
            Ok(format!("{recv}.subrange({l}, {h})"))
        }
    }
}

/// An integer cast `e as T` → `(e) as nat`/`as int`/`as u64`/… with the #122
/// paren discipline (the inner is parenthesized so a binary/unary inner casts as
/// a whole — `a + b as nat` must NOT parse as `a + (b as nat)`).
fn encode_cast(
    inner: &Expr,
    ty: &thermite_syntax::ast::Type,
    ctx: &RefCtx,
) -> Result<String, RefEncodeError> {
    let e = encode(inner, ctx)?;
    let target = cast_target(ty)?;
    // Parenthesize the inner unconditionally (the #122 discipline): a bare path
    // / literal is unaffected, a compound inner is bound correctly.
    Ok(format!("({e}) as {target}"))
}

/// The Verus cast-target spelling. The contract sublanguage casts to the
/// arithmetic ladder `nat`/`int` and the bounded prims (§4.2). `nat`/`int` are
/// spelled bare; a primitive name maps to its Verus spelling.
fn cast_target(ty: &thermite_syntax::ast::Type) -> Result<String, RefEncodeError> {
    use thermite_syntax::ast::{PrimType, Type};
    match ty {
        Type::Prim(PrimType::U32) => Ok("u32".to_string()),
        Type::Prim(PrimType::U64) => Ok("u64".to_string()),
        Type::Prim(PrimType::Usize) => Ok("usize".to_string()),
        Type::Prim(PrimType::Bool) => Err(RefEncodeError::Unsupported(
            "cast to bool (not an arithmetic cast)".to_string(),
        )),
        // `nat`/`int` are surface-spelled as bare named types in cast position.
        Type::Named(n) if n == "nat" || n == "int" => Ok(n.clone()),
        other => Err(RefEncodeError::Unsupported(format!(
            "cast to unsupported type {other:?}"
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
        Expr::MethodCall { .. } => "method call".to_string(),
        Expr::Field { .. } => "field access".to_string(),
        Expr::Closure { .. } => "closure (outside a combinator predicate slot)".to_string(),
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
