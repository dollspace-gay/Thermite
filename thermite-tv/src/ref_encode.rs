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
//! | REQ-1 (independent reference encoder) | SHIPPED | `pub fn ref_contract_pred` here; non-test consumer `thermite_tv::obligation::equivalence_obligation` (`obligation.rs`); verified by `thermite-tv/tests/teeth.rs` F1–F4 against real verus (faithful VERIFIES, infidel COUNTEREXAMPLE). Depends on `thermite-syntax` + `thermite-spec` ONLY — no `thermite-lower` (`Cargo.toml`), so the independence is a compile constraint (AC-6). **#150 coverage extension:** `encode_match`/`encode_pattern` independently encode an `Expr::Match`-in-ens (the C7 payload-in-contract `Some/None/Ok/Err` match, mirroring production's `lower_match` shape); `encode_string_byteview` (keyed on the `string_bound` receiver set) re-implements production's `recv_is_string` byte-view rewrite (`.len()`→`spec_len()`, `.byte_at(i)`→`spec_byte_at(<i>)`); `encode_map_accessor` (keyed on `map_bound`) rewrites `.contains_key(k)`→`spec_contains_key(k)` / `.len()`→`len()`; `encode_len_receiver` keeps a plain slice `.len()` BARE (matching production's un-viewed slice `.len()`). Non-test consumer: `forge::contract_tv::tv_file` (the corpus phase, via `equivalence_obligation`). GROUNDED under real verus: binary_search Option-match-ens + string_demo byte-view + map_kv `contains_key`/`is None` all Checked + Faithful (`forge/tests/contract_tv_conformance.rs`). |

use std::collections::BTreeSet;
use std::fmt;

use thermite_syntax::ast::{BinOp, Expr, IndexArg, MatchArm, Pattern, UnaryOp};

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
    /// Names bound in the obligation as the `String` wrapper (`&TString`/`TString`)
    /// — a `String`/`&String` param (#150 gap #2). For such a receiver the
    /// byte-view dispatch is the wrapper's SPEC fns, NOT a `Seq<u8>` index:
    /// `.len()`→`.spec_len()`, `.byte_at(i)`→`.spec_byte_at(i as int)`. This
    /// RE-implements production's `String`-receiver spec-position rewrite
    /// (`lower.rs`: `recv_is_string` → `r.spec_len()` / `r.spec_byte_at(i as int)`)
    /// INDEPENDENTLY, so a misdispatch (a wrong index, the #127 class) is caught.
    /// A `String`-bound receiver is emitted BARE (`s`, NOT `s@`) — the wrapper
    /// spec fns take `&self`, not a `Seq` view.
    string_bound: BTreeSet<String>,
    /// Names bound in the obligation as the `Map` wrapper (`TMap…`) — a
    /// `Map<K,V>`/`&Map<K,V>` param/result (#150 gap #3). For such a receiver the
    /// membership accessor rewrites to the wrapper SPEC fn: `.contains_key(k)`→
    /// `.spec_contains_key(k)`, `.len()`→`.len()` (the wrapper `spec fn len -> nat`).
    /// This RE-implements production's `m.contains_key(k)`→`m.spec_contains_key(k)`
    /// spec rewrite (`lower.rs`) INDEPENDENTLY (the wrapper spec fns are the shared
    /// frozen ground truth, in the preamble). The receiver is emitted BARE.
    map_bound: BTreeSet<String>,
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
            string_bound: BTreeSet::new(),
            map_bound: BTreeSet::new(),
            nat_coerce: BTreeSet::new(),
        }
    }

    /// Declare names bound as the `Map` wrapper (`TMap…`) — a `Map<K,V>` param/
    /// result whose spec-position membership accessor dispatches to the wrapper SPEC
    /// fn (`.contains_key(k)`→`.spec_contains_key(k)`), MATCHING production (#150 gap
    /// #3; see [`RefCtx::map_bound`]).
    pub fn with_map_bound<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.map_bound = names.into_iter().map(Into::into).collect();
        self
    }

    /// Declare names bound as the `String` wrapper (`&TString`/`TString`) — a
    /// `String`/`&String` param whose spec-position byte-view dispatches to the
    /// wrapper SPEC fns (`.spec_len()`/`.spec_byte_at(i as int)`), NOT a `Seq<u8>`
    /// index (#150 gap #2; see [`RefCtx::string_bound`]).
    pub fn with_string_bound<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.string_bound = names.into_iter().map(Into::into).collect();
        self
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

    fn is_string_bound(&self, name: &str) -> bool {
        self.string_bound.contains(name)
    }

    fn is_map_bound(&self, name: &str) -> bool {
        self.map_bound.contains(name)
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
        // A reference `&e` in spec position (REQ-1; the `&xs[..i]` slice-range
        // borrow). Production lowers a spec-context `&xs[..i]` to the SUBRANGE of
        // the base's `@`-view (`xs@.subrange(0, i as int)`) — the `&`/`[..]` is the
        // slice→`Seq`-subrange rewrite, NOT a Verus reference (Verus `Seq`s are
        // value types, `&` over a spec slice is meaningless). We RE-implement that
        // shape INDEPENDENTLY: a `&`-of-`Index` is encoded EXACTLY as the inner
        // `Index` (the subrange), and a bare `&e` (`&xs` → `xs@`) drops the `&`
        // to the base's view. This MATCHES production's `Expr::Ref` spec arm
        // (which delegates `&xs[..i]` to its `lower_index`) without calling it.
        Expr::Ref { expr: inner, .. } => encode_ref(inner, ctx),
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
        Expr::Match { scrutinee, arms } => encode_match(scrutinee, arms, ctx),
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
    // THE COERCION FIX (declarative `lower_nat_equality` re-implementation): in an
    // `==` COMPARISON where one operand is a `nat`-valued term (a nat-returning
    // spec-fn call) and the other is a bounded-int name declared in `nat_coerce`,
    // coerce the int operand `as nat` (matching the golden `result as nat ==
    // spec_sum(xs@)`).
    //
    // CRITICAL — production's coercion is `Eq`-ONLY (`lower.rs`: `lower_nat_equality`
    // fires only `if *op == BinOp::Eq`). A NON-`Eq` comparison of a bounded int to a
    // `nat` term (`acc <= spec_sum(xs)`, `i < count_where(..)`, `acc != spec_sum`) is
    // lowered BARE — production emits `acc <= spec_sum(xs)`, NEVER `acc as nat <=
    // spec_sum(xs)` (verus accepts the mixed `u64`/`nat` comparison directly). So the
    // reference MUST coerce ONLY on `Eq` too; coercing on `<=`/`<`/`>`/`>=`/`!=` would
    // emit `acc as nat <= spec_sum` and DIVERGE from production's bare form (a spurious
    // counterexample, NOT a meaning bug). This RE-implements production's Eq-only rule
    // INDEPENDENTLY (a production coercion bug — coercing the wrong op — is still
    // caught: the reference and production would then differ).
    if is_comparison(op) {
        let coerce = op == BinOp::Eq;
        let lhs_nat = coerce && is_nat_valued(rhs);
        let rhs_nat = coerce && is_nat_valued(lhs);
        let l = encode_comparison_operand(lhs, lhs_nat, op, true, ctx)?;
        let r = encode_comparison_operand(rhs, rhs_nat, op, false, ctx)?;
        return Ok(format!("({l} {} {r})", binop_str(op)));
    }
    let l = encode_binary_operand(lhs, op, true, ctx)?;
    let r = encode_binary_operand(rhs, op, false, ctx)?;
    // Parenthesize the whole binary so precedence is explicit at every level
    // (the #122 paren discipline generalized: a sub-predicate never silently
    // re-associates). Z3 sees the SAME term regardless of nesting.
    Ok(format!("({l} {} {r})", binop_str(op)))
}

/// Is `op` a comparison (the operators whose operands may carry a `nat`
/// coercion / a cast-`<` paren)?
fn is_comparison(op: BinOp) -> bool {
    matches!(
        op,
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge
    )
}

/// Is `op` a `<`-LEADING operator (`<`, `<=`, `<<`)? A `Cast` LEFT operand of such
/// an op MUST be wholly parenthesized — `x as u32 < 33` mis-parses the `u32 <` as
/// the start of a generic-argument list (the #146/#148 cast-paren ambiguity, a HARD
/// parse error in both Verus and Rust). This is the EXACT dual of production's
/// `is_lt_leading in lower.rs` (`Lt | Le | Shl`), RE-stated INDEPENDENTLY here so
/// the reference parenthesizes the same class — without it the reference would emit
/// an un-parseable `as nat <`/`as u32 <` and the obligation would be Unverifiable
/// (not a faithfulness verdict). `>`/`>=`/`>>`/`==`/`!=` do NOT trigger the generic
/// ambiguity (excluded — keeps the non-`<` casts paren-minimal).
fn is_lt_leading(op: BinOp) -> bool {
    matches!(op, BinOp::Lt | BinOp::Le | BinOp::Shl)
}

/// Is `expr` a `nat`-valued term — i.e. a call to a `nat`-returning spec fn? The
/// frozen `nat`-returning forms are the recursive `spec fn … -> nat` (e.g.
/// `spec_sum`, `count_where`). We detect it structurally: a call whose callee is
/// a path. (A combinator-`count_where`/spec-fn call is the nat term in a
/// comparison.) This is the declarative side of the coercion inference.
fn is_nat_valued(expr: &Expr) -> bool {
    matches!(expr, Expr::Call { callee, .. } if matches!(callee.as_ref(), Expr::Path(_)))
}

/// Encode a comparison operand: apply the `as nat` coercion when `coerce_nat` (the
/// `Eq`-only nat-coerce decided by [`encode_binary`]) AND the operand is a
/// bounded-int name in `nat_coerce`; OTHERWISE encode the sub-expression and apply
/// the cast-`<`-leading paren ([`encode_binary_operand`]) so a `Cast` left operand
/// of a `<`-leading op is wholly parenthesized (the #146/#148 discipline).
fn encode_comparison_operand(
    operand: &Expr,
    coerce_nat: bool,
    op: BinOp,
    is_left: bool,
    ctx: &RefCtx,
) -> Result<String, RefEncodeError> {
    if coerce_nat {
        if let Expr::Path(segments) = operand {
            if segments.len() == 1 && ctx.needs_nat_coerce(&segments[0]) {
                // The coerced operand is itself an `as nat` cast — when it is the
                // LEFT operand of a `<`-leading op it must be wholly parenthesized
                // (`(acc as nat) <= …`), exactly as production parenthesizes a
                // source-level `(result as nat) <= …` cast (#146/#148).
                let cast = format!("{} as nat", segments[0]);
                if is_left && is_lt_leading(op) {
                    return Ok(format!("({cast})"));
                }
                return Ok(cast);
            }
        }
    }
    encode_binary_operand(operand, op, is_left, ctx)
}

/// Encode a binary operand, applying the cast-`<`-leading paren (#146/#148): a
/// `Cast` that is the LEFT operand of a `<`-leading op (`<`/`<=`/`<<`) is wholly
/// parenthesized — `(x as u32) < 33`, never the ambiguous `x as u32 < 33`. This is
/// the dual of production's `lower_binary_operand`'s `is_lt_leading` guard, RE-stated
/// INDEPENDENTLY. Every other operand is the plain [`encode`] (its own
/// parenthesization is already explicit per the #122 discipline).
fn encode_binary_operand(
    operand: &Expr,
    op: BinOp,
    is_left: bool,
    ctx: &RefCtx,
) -> Result<String, RefEncodeError> {
    let s = encode(operand, ctx)?;
    if is_left && matches!(operand, Expr::Cast { .. }) && is_lt_leading(op) {
        return Ok(format!("({s})"));
    }
    Ok(s)
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
/// the combinator's ARGUMENTS, dispatched PER REGISTRY ARG-KIND
/// (`CombinatorSig.arg_kinds`, `thermite_spec::combinators`) — exactly where a
/// combinator-argument fidelity bug lives.
///
/// Each i-th argument is encoded by its frozen kind (`thermite-design.md` §4.2):
///
/// - [`ArgKind::Slice`] → the slice→`@` view ([`encode_slice_arg`]): a `Seq<u32>`
///   view of a slice param (F2's `xs` → `xs@`/`xs`).
/// - [`ArgKind::Index`] → a SCALAR `int` ([`encode_index_value`]: `<path> as int`,
///   a bare literal stays bare) — NEVER the `@`-view. This is the #145 fix:
///   `forall_below`/`forall_from`'s `n: int` index bound is a scalar, and `n@`
///   is a Verus type error (`no method view for int`).
/// - [`ArgKind::Pred`] → the predicate closure `|x: u32| <body>`, body re-encoded
///   by the SAME independent recursion (so F2's `x <= 10` vs `x < 10` is caught).
/// - [`ArgKind::Value`] → the value as-is ([`encode`]).
///
/// The arity + arg-kinds are already validated by the registry/validator, so we
/// index `arg_kinds` by position; a mismatch is an honest `Err` (never a panic).
fn encode_combinator_call(
    name: &str,
    args: &[Expr],
    ctx: &RefCtx,
) -> Result<String, RefEncodeError> {
    // The registry entry is the frozen ground truth for the arg KINDS. The caller
    // (`encode_call`) only reaches here when `lookup(name).is_some()`.
    let sig = thermite_spec::lookup(name)
        .ok_or_else(|| RefEncodeError::UnknownCallee(name.to_string()))?;
    if args.len() != sig.arg_kinds.len() {
        return Err(RefEncodeError::Unsupported(format!(
            "combinator `{name}` arity mismatch (got {} args, registry declares {})",
            args.len(),
            sig.arg_kinds.len()
        )));
    }

    let encoded_args = args
        .iter()
        .zip(sig.arg_kinds.iter())
        .map(|(arg, kind)| encode_combinator_arg(arg, *kind, ctx))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(format!("{name}({})", encoded_args.join(", ")))
}

/// Encode a single combinator argument BY ITS REGISTRY KIND (`#145`). This is the
/// per-kind dispatch the frozen `CombinatorSig.arg_kinds` mandates; it replaces
/// the old slice-`@`-view-everything path that mis-encoded the `int` Index arg.
fn encode_combinator_arg(
    arg: &Expr,
    kind: thermite_spec::ArgKind,
    ctx: &RefCtx,
) -> Result<String, RefEncodeError> {
    use thermite_spec::ArgKind;
    match kind {
        // A slice param → its `Seq` `@`-view (identity when bound as `Seq`).
        ArgKind::Slice => encode_slice_arg(arg, ctx),
        // An `int` index BOUND (`forall_below`/`forall_from`'s `n: int`) → a
        // SCALAR `int`, NEVER the `@`-view. THE #145 FIX.
        ArgKind::Index => encode_index_value(arg, ctx),
        // The predicate closure slot.
        ArgKind::Pred => encode_pred_arg(arg, ctx),
        // A plain scalar value, as-is.
        ArgKind::Value => encode(arg, ctx),
    }
}

/// Encode a combinator `Pred`-kind argument: a predicate closure `|x| <body>` is
/// RE-encoded to a Verus closure `|x: u32| <body>` — the body is encoded by the
/// SAME independent recursion (so a closure-predicate infidelity, F2's `x <= 10`
/// vs `x < 10`, is caught). A non-closure in a `Pred` slot is an honest `Err`
/// (the registry says this slot MUST be a closure).
fn encode_pred_arg(arg: &Expr, ctx: &RefCtx) -> Result<String, RefEncodeError> {
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
        other => Err(RefEncodeError::Unsupported(format!(
            "combinator predicate slot expects a closure, got {}",
            node_kind(other)
        ))),
    }
}

/// Encode a call argument for a NAMED spec-fn call (not a combinator — combinator
/// args dispatch per registry kind via [`encode_combinator_arg`]). A predicate
/// closure is RE-encoded to a Verus closure `|x: u32| <body>`; everything else is
/// an ordinary sub-expression (a slice gets its `@`-view via [`encode_slice_arg`]).
fn encode_call_arg(arg: &Expr, ctx: &RefCtx) -> Result<String, RefEncodeError> {
    match arg {
        Expr::Closure { .. } => encode_pred_arg(arg, ctx),
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
    // A `String`/`&String` receiver (#150 gap #2): the byte-view dispatch is the
    // wrapper SPEC fns (`.spec_len()` / `.spec_byte_at(i as int)`), keyed on the
    // RECEIVER being a `string_bound` bare path — MIRRORING production's
    // `recv_is_string` arm (`lower.rs`), which a `String`-param `s.byte_at(0)` /
    // `s.len()` in an `ens` reaches. The receiver is emitted BARE (`s`), not `s@`:
    // the wrapper spec fns take `&self`.
    if let Expr::Path(segs) = receiver {
        if segs.len() == 1 && ctx.is_string_bound(&segs[0]) {
            return encode_string_byteview(&segs[0], name, args, ctx);
        }
        if segs.len() == 1 && ctx.is_map_bound(&segs[0]) {
            return encode_map_accessor(&segs[0], name, args, ctx);
        }
    }

    match name {
        // The byte-view accessor (#127): `s.byte_at(i)` is the i-th byte of the
        // sequence view — `recv[i]`. F3's teeth bite here: a production
        // misdispatch to index `1` for source index `0` differs from this. This is
        // the `Seq<u8>`-bound byte-view (a #127/#147 `Seq`-receiver), distinct from
        // the `String`/TString wrapper byte-view above (#150 gap #2).
        "byte_at" => {
            if args.len() != 1 {
                return Err(RefEncodeError::Unsupported(format!(
                    "byte_at/{} (expected exactly 1 arg)",
                    args.len()
                )));
            }
            let recv = encode_receiver(receiver, ctx)?;
            let idx = encode_index_value(&args[0], ctx)?;
            Ok(format!("{recv}[{idx}]"))
        }
        // The length accessor `s.len()`. A `Seq`-bound receiver views as `recv.len()`
        // (the `Seq::len()`); a plain SLICE-param receiver (`&[T]`, NOT seq-bound)
        // emits the BARE `recv.len()` — matching production, which keeps a slice
        // `.len()` un-viewed in spec position (`lower.rs`: "a slice `.len()` in spec
        // position is accepted by Verus on the slice (`haystack.len()`); the `@` view
        // is only needed where a `Seq` operation is required"). So the receiver here
        // is the BARE path (no `@` suffix), NOT `encode_receiver`'s viewed form.
        "len" => {
            if !args.is_empty() {
                return Err(RefEncodeError::Unsupported(
                    "len with arguments".to_string(),
                ));
            }
            let recv = encode_len_receiver(receiver, ctx)?;
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
            let recv = encode_receiver(receiver, ctx)?;
            let lo = encode_index_value(&args[0], ctx)?;
            let hi = encode_index_value(&args[1], ctx)?;
            Ok(format!("{recv}.subrange({lo}, {hi})"))
        }
        other => Err(RefEncodeError::Unsupported(format!(
            "spec method `.{other}()` (not in the frozen byte-view set)"
        ))),
    }
}

/// Encode a `String`/`&String`-receiver byte-view method (#150 gap #2). The
/// receiver `s` is a `string_bound` name (bound `&TString`/`TString` in the
/// obligation), so its spec-position byte-view rewrites to the wrapper SPEC fns —
/// EXACTLY production's `recv_is_string` arm (`lower.rs`):
///
/// - `.len()` → `s.spec_len()` (the `nat`-valued spec length; the exec `len`
///   returns `u64` and cannot be named in a contract).
/// - `.byte_at(i)` → `s.spec_byte_at(<i>)` where an integer LITERAL stays bare
///   (Verus coerces it into the `int` param, matching the golden
///   `string_demo.verus.rs` `s.spec_byte_at(0)`) and a non-literal index is cast
///   `as int` (no implicit `usize`→`int` in spec position) — the SAME literal/cast
///   split production applies.
///
/// `.slice(..)` over a `String` is NOT in the frozen contract byte-view set (the
/// `TString` wrapper has no `spec_slice` — `slice` is an EXEC constructor, never
/// named in a contract; no corpus clause uses it) → an honest [`RefEncodeError`],
/// never a silent wrong encoding.
fn encode_string_byteview(
    recv: &str,
    name: &str,
    args: &[Expr],
    ctx: &RefCtx,
) -> Result<String, RefEncodeError> {
    match name {
        "len" => {
            if !args.is_empty() {
                return Err(RefEncodeError::Unsupported(
                    "String len with arguments".to_string(),
                ));
            }
            Ok(format!("{recv}.spec_len()"))
        }
        "byte_at" => {
            if args.len() != 1 {
                return Err(RefEncodeError::Unsupported(format!(
                    "String byte_at/{} (expected exactly 1 arg)",
                    args.len()
                )));
            }
            // An integer literal flows into the `int` param directly (Verus coerces
            // it), matching production's golden `s.spec_byte_at(0)`; a non-literal
            // index is cast `as int` via `encode_index_value`.
            let idx = match &args[0] {
                Expr::IntLit { value, .. } => value.to_string(),
                other => encode_index_value(other, ctx)?,
            };
            Ok(format!("{recv}.spec_byte_at({idx})"))
        }
        other => Err(RefEncodeError::Unsupported(format!(
            "spec method `.{other}()` on a String receiver (the frozen String \
             byte-view is `.len()`/`.byte_at(i)`; `.slice(..)` is an exec \
             constructor, not a contract spec fn)"
        ))),
    }
}

/// Encode a `Map`-receiver spec-position accessor (#150 gap #3). The receiver `m`
/// is a `map_bound` name (bound `TMap…` in the obligation), so its membership/length
/// accessor rewrites to the wrapper SPEC fns — EXACTLY production's `lower.rs` Map
/// arm:
///
/// - `.contains_key(k)` → `m.spec_contains_key(k)` — the `exists|j| data@[j].0 == k`
///   membership. The key arg lowers PLAINLY (a Copy key value, NO `as int` cast —
///   `spec_contains_key` takes the surface key type), matching production.
/// - `.len()` → `m.len()` — the wrapper `spec fn len(&self) -> nat`, unchanged.
///
/// `.get(_)`/`.insert(_)` are NOT spec-rewritten (production names `get` only via a
/// `match`-in-`ens` over the result, and `insert` is exec) → an honest
/// [`RefEncodeError`], never a silent wrong encoding.
fn encode_map_accessor(
    recv: &str,
    name: &str,
    args: &[Expr],
    ctx: &RefCtx,
) -> Result<String, RefEncodeError> {
    match name {
        "contains_key" => {
            if args.len() != 1 {
                return Err(RefEncodeError::Unsupported(format!(
                    "contains_key/{} (expected exactly 1 arg)",
                    args.len()
                )));
            }
            let arg = encode(&args[0], ctx)?;
            Ok(format!("{recv}.spec_contains_key({arg})"))
        }
        "len" => {
            if !args.is_empty() {
                return Err(RefEncodeError::Unsupported(
                    "Map len with arguments".to_string(),
                ));
            }
            Ok(format!("{recv}.len()"))
        }
        other => Err(RefEncodeError::Unsupported(format!(
            "spec method `.{other}()` on a Map receiver (the frozen Map spec \
             accessors are `.contains_key(k)`/`.len()`; `.get`/`.insert` are not \
             contract spec-fn rewrite targets)"
        ))),
    }
}

/// Encode a `.len()` receiver: a `Seq`-bound name is its own view (`recv.len()`),
/// and a plain SLICE-param bare path (`&[T]`, not seq-bound, not string-bound)
/// emits the BARE name — production keeps a slice `.len()` UN-viewed in spec
/// position (`haystack.len()`, NOT `haystack@.len()`), applying `@` only at a `Seq`
/// op (index/subrange/combinator-arg). This is the dual of production's slice
/// `.len()` rule. A non-path / non-slice receiver falls back to [`encode`].
fn encode_len_receiver(receiver: &Expr, ctx: &RefCtx) -> Result<String, RefEncodeError> {
    if let Expr::Path(segments) = receiver {
        if segments.len() == 1 {
            return encode_path(segments);
        }
    }
    encode(receiver, ctx)
}

/// Encode an `Expr::Match` in contract position (#150 gap #1; the C7 payload-in-
/// contract `ens match result { Some(v) => <pred(v)>, None => <pred> }`, and the
/// `Ok`/`Err` Result form). Production lowers a spec-context `match` to a Verus
/// `match` EXPRESSION in the `ensures` (`tests/golden/lower/binary_search.verus.rs`,
/// `option_result.verus.rs`):
///
/// ```text
/// match result {
///     Some(i) => i < haystack.len() && haystack@[i as int] == needle,
///     None => forall_in(haystack@, |x: u32| x != needle),
/// }
/// ```
///
/// We RE-implement that shape INDEPENDENTLY: the scrutinee is encoded by the same
/// recursion, each arm's PATTERN is encoded ([`encode_pattern`]) and each arm's
/// BODY is encoded by the SAME independent recursion (so a payload-predicate
/// infidelity — a wrong arm body, a swapped `Some`/`None` — is caught). The
/// pattern-bound payload var (`i`/`v`/`e`) is in scope in the body exactly as
/// production binds it, so `haystack[i]` encodes to `haystack@[i as int]` (the
/// pattern var as an `int` index) MATCHING production. The brace/arm layout
/// mirrors production's `lower_match`. A guard arm emits `pat if <guard> => body`
/// (the C10 form). NEVER a panic / silent wrong encoding.
fn encode_match(
    scrutinee: &Expr,
    arms: &[MatchArm],
    ctx: &RefCtx,
) -> Result<String, RefEncodeError> {
    let s = encode(scrutinee, ctx)?;
    let mut out = format!("match {s} {{\n");
    for arm in arms {
        let pat = encode_pattern(&arm.pattern)?;
        let body = encode(&arm.body, ctx)?;
        match &arm.guard {
            Some(guard) => {
                let g = encode(guard, ctx)?;
                out.push_str(&format!("            {pat} if {g} => {body},\n"));
            }
            None => {
                out.push_str(&format!("            {pat} => {body},\n"));
            }
        }
    }
    out.push_str("        }");
    Ok(out)
}

/// Encode a contract-position match PATTERN independently of production's
/// `lower_pattern` (#150 gap #1). The frozen contract-`match` covers the C7
/// payload-in-contract patterns: the built-in `Option`/`Result` variants
/// (`Some(x)`/`None`/`Ok(x)`/`Err(e)`, unqualified — Verus knows `Option`/`Result`,
/// exactly as production's `qualify_variant_path` leaves a built-in unqualified),
/// a binding (`x`), and a wildcard (`_`). A nested/struct/slice/or pattern, or a
/// USER enum variant (which production would enum-qualify via its `variants` map —
/// the reference has no such map, so qualifying it would risk a silent wrong
/// encoding) is an honest [`RefEncodeError`].
fn encode_pattern(pat: &Pattern) -> Result<String, RefEncodeError> {
    match pat {
        Pattern::Wildcard => Ok("_".to_string()),
        Pattern::Binding(name) => Ok(name.clone()),
        Pattern::Enum { path, fields } => {
            let head = path.join("::");
            // Only the built-in Option/Result variants are encodable unqualified
            // (production leaves a built-in unqualified; a user variant would need
            // the enum-qualification map we deliberately do not import).
            if !is_builtin_variant(&head) {
                return Err(RefEncodeError::Unsupported(format!(
                    "match pattern over the user/non-built-in variant `{head}` \
                     (the frozen contract-`match` covers the built-in \
                     Some/None/Ok/Err payload patterns)"
                )));
            }
            if fields.is_empty() {
                Ok(head)
            } else {
                let mut fs = Vec::with_capacity(fields.len());
                for f in fields {
                    fs.push(encode_pattern(f)?);
                }
                Ok(format!("{head}({})", fs.join(", ")))
            }
        }
        other => Err(RefEncodeError::Unsupported(format!(
            "match pattern {other:?} (the frozen contract-`match` covers the \
             built-in Some/None/Ok/Err payload patterns + bindings/wildcards)"
        ))),
    }
}

/// Is `head` a built-in `Option`/`Result` variant constructor (unqualified in
/// Verus)? These are the only variants a contract-position `match` patterns over
/// in the frozen sublanguage (#150 gap #1).
fn is_builtin_variant(head: &str) -> bool {
    matches!(head, "Some" | "None" | "Ok" | "Err")
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

/// Encode a reference `&inner` in spec position (REQ-1; the `&xs[..i]` /
/// `&xs[a..b]` slice-range borrow + the bare `&xs`). A spec `Seq` is a value
/// type — there is no Verus `&Seq` borrow in the contract sublanguage — so a `&`
/// in spec position is ALWAYS the slice→`Seq` view/subrange rewrite, never a
/// literal `&`. This RE-implements production's `Expr::Ref` spec arm
/// (`lower.rs`: in spec position a `&`-of-`Index` delegates to `lower_index`, a
/// bare `&e` over a slice param views it) INDEPENDENTLY:
///
/// - `&xs[..i]` / `&xs[a..b]` / `&xs[a..]` — the inner is an [`Expr::Index`]
///   range; encode EXACTLY the inner `Index` (the `.subrange(..)` over the base
///   view), so `&xs[..i]` and `xs[..i]` encode identically (matching production,
///   which routes both through `lower_index`).
/// - `&xs[i]` — a single-element borrow is the indexed element (the inner
///   `Index`), same as above.
/// - a bare `&xs` (a slice param) — the base's `@`-view ([`encode_slice_arg`]:
///   `xs@` when not seq-bound, the identity `xs` when bound as `Seq`).
///
/// A `&` over anything else (a scalar, a non-slice expr) is outside the frozen
/// contract sublanguage → an honest [`RefEncodeError`] (never a silent wrong
/// encoding).
fn encode_ref(inner: &Expr, ctx: &RefCtx) -> Result<String, RefEncodeError> {
    match inner {
        // `&xs[..i]` / `&xs[a..b]` / `&xs[i]` — the slice-range/element borrow is
        // the inner index/subrange itself (production routes `&`-of-`Index`
        // straight through `lower_index`).
        Expr::Index { base, index } => encode_index(base, index, ctx),
        // A bare `&xs` over a slice param → its `Seq` `@`-view (the slice→`@`
        // rewrite, the same dispatch `encode_slice_arg` applies at use sites).
        Expr::Path(_) => encode_slice_arg(inner, ctx),
        other => Err(RefEncodeError::Unsupported(format!(
            "reference `&{}` (a spec `&` is only the slice→Seq view/subrange — \
             over a slice index/range or a bare slice param)",
            node_kind(other)
        ))),
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
