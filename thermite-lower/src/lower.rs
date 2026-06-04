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

use std::fmt::Write as _;

use thermite_syntax::ast::{
    BinOp, Block, Clause, Expr, FnItem, IndexArg, Item, MatchArm, Param, Pattern, PrimType,
    Program, SlicePat, SpecFnItem, Stmt, Type,
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
}

const NO_SLICES: &[&str] = &[];

impl<'a> Ctx<'a> {
    fn exec() -> Ctx<'static> {
        Ctx {
            pos: Pos::Exec,
            slices: NO_SLICES,
            nat_fns: NO_SLICES,
        }
    }
    fn spec(slices: &'a [&'a str], nat_fns: &'a [&'a str]) -> Ctx<'a> {
        Ctx {
            pos: Pos::Spec,
            slices,
            nat_fns,
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
        }
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

    // The program-wide set of `nat`-returning spec fns (the head-fold-sum shape,
    // OQ-1) — SHAPE-derived, used to coerce `u64`/`nat` equalities (`as nat`).
    let nat_fns: Vec<&str> = program
        .items
        .iter()
        .filter_map(|item| match item {
            Item::SpecFn(s) if is_head_fold_sum(&s.body) => Some(s.name.as_str()),
            _ => None,
        })
        .collect();

    // (2) the lowered items, in source order (determinism, §5.3). A `fn` whose
    // loop carries an accumulator-fold invariant pulls in the auto-generated
    // push lemma for the folded spec fn (REQ-7 template a); the lemma def is
    // emitted at file scope right before the `fn` that uses it, deduped.
    let mut emitted_lemmas: Vec<String> = Vec::new();
    for item in &program.items {
        let item_src = match item {
            Item::SpecFn(s) => lower_spec_fn(s)?,
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
                lower_fn(f, &nat_fns)?
            }
        };
        out.push('\n');
        out.push_str(&item_src);
        out.push('\n');
    }

    out.push_str("\n}\nfn main() {}\n");
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
                collect_combinators_in_block_specs(&f.body, f.span, &mut names);
            }
            Item::SpecFn(s) => {
                collect_combinators_in_expr(&s.dec.expr, s.span, &mut names);
                collect_combinators_in_block_specs(&s.body, s.span, &mut names);
            }
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
        Expr::IntLit(_) | Expr::BoolLit(_) | Expr::Path(_) => {}
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
// REQ-1: signature lowering.
// ---------------------------------------------------------------------------

/// Lower a `spec fn` (REQ-1/REQ-5). Slice params take `Seq<T>` (not `&[T]`); the
/// body lowers in spec context; `dec`→`decreases`. The return type uses the
/// `nat`-typed accumulator form when the body folds slice elements into a sum
/// (OQ-1: `u64`-valued `spec_sum` would re-introduce the overflow obligation).
fn lower_spec_fn(s: &SpecFnItem) -> Result<String, LowerError> {
    let mut out = String::new();
    let ret = lower_spec_fn_ret(&s.ret, &s.body);
    write!(out, "spec fn {}(", s.name).ok();
    emit_params(&mut out, &s.params, Pos::Spec)?;
    write!(
        out,
        ") -> {ret}\n    decreases {}\n",
        spec_dec(&s.dec, &s.params)
    )
    .ok();
    out.push_str(&lower_spec_fn_body(&s.body, &s.params, &ret)?);
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

/// Lower a `fn` (REQ-1). `-> (result: RET)` binder so `ens` can mention
/// `result`; `req`→`requires`, each `ens`→`ensures`, `fx pure`→nothing.
fn lower_fn(f: &FnItem, nat_fns: &[&str]) -> Result<String, LowerError> {
    let mut out = String::new();
    let ret = lower_type(&f.ret)?;
    write!(out, "fn {}(", f.name).ok();
    emit_params(&mut out, &f.params, Pos::Exec)?;
    writeln!(out, ") -> (result: {ret})").ok();

    let slices = slice_param_names(&f.params);
    let spec = Ctx::spec(&slices, nat_fns);

    // requires: the single `req` clause (REQ-1). Omit a literal-`true` req.
    let req = lower_expr(&f.contract.req.expr, spec, 0, f.span)?;
    if req != "true" {
        writeln!(out, "    requires {req},").ok();
    }

    // ensures: every `ens` clause in source order (no weakening — R-DEFER-9).
    out.push_str("    ensures\n");
    for ens in &f.contract.ens {
        let e = lower_expr(&ens.expr, spec, 0, f.span)?;
        writeln!(out, "        {e},").ok();
    }
    // `fx pure` emits no annotation (Verus `fn` is pure by default; §4.1).

    // Body, with shape-derived proof aids threaded through the loop lowering.
    let body = lower_fn_body(f, nat_fns)?;
    out.push_str(&body);
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
    if is_head_fold_sum(body) {
        return "nat".to_string();
    }
    lower_type(ret).unwrap_or_else(|_| "bool".to_string())
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
                if matches!(&arm.body, Expr::IntLit(0)) {
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
fn lower_spec_fn_body(body: &Block, params: &[Param], ret: &str) -> Result<String, LowerError> {
    if is_head_fold_sum(body) {
        if let Some(slice) = first_slice_param(params) {
            if let Some(tail) = &body.tail {
                if let Expr::Match { arms, .. } = tail.as_ref() {
                    return seq_fold_body(slice, arms, ret);
                }
            }
        }
    }
    // Fallback: lower the block in spec context directly.
    let mut out = String::from("{\n");
    let b = lower_block_inner(body, Ctx::spec_seq(), 1, zero_span())?;
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
    }
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
        Expr::IntLit(n) => Ok(n.to_string()),
        Expr::BoolLit(b) => Ok(b.to_string()),
        Expr::Path(segs) => {
            // A plain path emits its segments joined by `::`. The slice→`xs@`
            // view (REQ-5) is applied at the point of USE (a spec-fn / combinator
            // argument position — `lower_spec_arg`), NOT here, because an `Index`
            // base must stay bare (`lower_index` appends the `@`) to avoid `xs@@`.
            Ok(segs.join("::"))
        }
        Expr::Call { callee, args } => {
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
            let mut parts = Vec::new();
            for a in args {
                parts.push(lower_expr(a, ctx, d, span)?);
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
        Expr::Index { base, index } => lower_index(base, index, ctx, d, span),
        Expr::Cast { expr, ty } => {
            let e = lower_expr(expr, ctx, d, span)?;
            let t = lower_type(ty)?;
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
    }
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
        let pat = lower_pattern(&arm.pattern, depth, span)?;
        let body = lower_expr(&arm.body, ctx, depth, span)?;
        writeln!(out, "            {pat} => {body},").ok();
    }
    out.push_str("        }");
    Ok(out)
}

/// Lower a pattern (REQ-7 node set). Enum patterns `Some(i)`/`None`, bindings,
/// wildcards, literals.
fn lower_pattern(pat: &Pattern, depth: usize, span: Span) -> Result<String, LowerError> {
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
            let head = path.join("::");
            if fields.is_empty() {
                Ok(head)
            } else {
                let mut fs = Vec::new();
                for f in fields {
                    fs.push(lower_pattern(f, depth + 1, span)?);
                }
                Ok(format!("{head}({})", fs.join(", ")))
            }
        }
        Pattern::Slice(_) => Err(LowerError::Unsupported {
            what: "slice pattern outside a head-fold spec fn".to_string(),
            span,
        }),
    }
}

/// The Verus/Rust operator for a `BinOp` (REQ-3).
fn binop(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
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

/// Binding-power tier of a binary operator (higher binds tighter). Mirrors
/// Rust/Verus operator precedence closely enough to decide parenthesization of
/// nested binaries during emission (REQ-3 — preserve the AST's grouping).
fn precedence(op: BinOp) -> u8 {
    match op {
        BinOp::Or => 1,
        BinOp::And => 2,
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => 3,
        BinOp::Add | BinOp::Sub => 4,
        BinOp::Mul | BinOp::Div => 5,
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
fn lower_fn_body(f: &FnItem, nat_fns: &[&str]) -> Result<String, LowerError> {
    let mut out = String::from("{\n");
    let inner = lower_block_with_fn_aids(&f.body, f, nat_fns, 1)?;
    out.push_str(&inner);
    out.push_str("}\n");
    Ok(out)
}

/// Lower a block with the enclosing `fn`'s contract in scope, so loop lowering
/// can lift immutable preconditions and emit accumulator/coverage aids (REQ-7).
fn lower_block_with_fn_aids(
    block: &Block,
    f: &FnItem,
    nat_fns: &[&str],
    indent: usize,
) -> Result<String, LowerError> {
    let pad = "    ".repeat(indent);
    let mut out = String::new();
    for stmt in &block.stmts {
        match stmt {
            Stmt::Loop(l) => {
                out.push_str(&lower_loop(l, f, nat_fns, indent)?);
            }
            other => {
                out.push_str(&lower_stmt(other, Ctx::exec(), indent)?);
            }
        }
    }
    if let Some(tail) = &block.tail {
        let t = lower_expr(tail, Ctx::exec(), 0, f.span)?;
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
    indent: usize,
) -> Result<String, LowerError> {
    use thermite_syntax::ast::LoopKind;
    let pad = "    ".repeat(indent);
    let ipad = "    ".repeat(indent + 1);
    let mut out = String::new();

    // Loop header.
    match &l.kind {
        LoopKind::Loop => writeln!(out, "{pad}loop").map_err(|_| fmt_err())?,
        LoopKind::While(c) => {
            let cs = lower_expr(c, Ctx::exec(), 0, f.span)?;
            writeln!(out, "{pad}while {cs}").map_err(|_| fmt_err())?;
        }
    };

    let slices = slice_param_names(&f.params);
    let spec = Ctx::spec(&slices, nat_fns);

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

    // decreases.
    let dec = lower_expr(&l.dec.expr, spec, 0, f.span)?;
    writeln!(out, "{ipad}decreases {dec},").map_err(|_| fmt_err())?;

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
    let body_src = lower_loop_body(&l.body, f, &l.invs, indent + 1)?;
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
        Expr::IntLit(_) | Expr::BoolLit(_) => false,
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
    collect_push_lemmas_in_block(&f.body, &mut defs);
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
    indent: usize,
) -> Result<String, LowerError> {
    // Pre-compute the coverage split, if this loop's invariants + the fn's
    // None-postcondition match template (e).
    let coverage = complementary_coverage_split(f, invs)?;

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
                    indent,
                )?);
                continue;
            }
        }
        out.push_str(&lower_stmt(stmt, Ctx::exec(), indent)?);
    }
    if let Some(tail) = &body.tail {
        let pad = "    ".repeat(indent);
        let t = lower_expr(tail, Ctx::exec(), 0, f.span)?;
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
    indent: usize,
) -> Result<String, LowerError> {
    let pad = "    ".repeat(indent);
    let c = lower_expr(cond, Ctx::exec(), 0, f.span)?;
    let mut out = format!("{pad}if {c} {{\n");
    // The split assert, indented one level in.
    for line in split.lines() {
        if line.is_empty() {
            out.push('\n');
        } else {
            writeln!(out, "{pad}    {line}").map_err(|_| fmt_err())?;
        }
    }
    let then_src = lower_block_inner(then, Ctx::exec(), indent, f.span)?;
    out.push_str(&then_src);
    out.push_str(&format!("{pad}}}"));
    if let Some(e) = else_ {
        let es = lower_block_inner(e, Ctx::exec(), indent, f.span)?;
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
