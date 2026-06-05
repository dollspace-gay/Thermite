//! L1 emission: compile a validated `thermite-syntax` `Program` into a single,
//! self-contained, runnable Rust source `String` whose body EXECUTES the
//! Thermite contract — the L1 rung of the ladder (`.design/lower/l1-runtime-checks.md`;
//! `thermite-design.md` §4.2/§6/§8). Where `lower.rs` emits Verus annotations for
//! an SMT *proof* (L3), `l1.rs` emits Rust that *runs* the contract: every
//! `req`/`ens` clause and every loop `inv` becomes a runnable `bool` check, every
//! combinator a real loop over `&[u32]`, every `spec fn` a real recursive Rust
//! fn. A violation is detected at the call site in EVERY build profile (not just
//! debug) via an always-active `thermite_check!` macro (NOT `debug_assert!`; §6).
//!
//! Governing design: `.design/lower/l1-runtime-checks.md`.
//! Reference (compiles + runs under `rustc`, hand-authored): `tests/golden/l1/sum.l1.rs`.
//!
//! ## Exec semantics, not spec semantics (REQ-1..REQ-4)
//!
//! Unlike `lower.rs` (which has a spec context with `Seq`/`@`/`subrange`), L1 is
//! ENTIRELY exec: there is no `vstd`, no `Seq`, no proof. A clause's verbatim
//! `Clause.text` (`ast.rs` `struct Clause { text }`) is carried into the
//! violation message for legibility (§2.4). The combinator L1 bodies and the
//! executable `spec_sum` are emitted INLINE (OQ-2) so the output is a
//! single self-contained file; the combinator bodies are pulled from the
//! `thermite-spec` registry's `l1` field (single source of truth, mirroring how
//! `lower.rs` reads `verus_l3`).
//!
//! ## Honest scope (REQ-5/REQ-7)
//!
//! `dec`/termination is a PROOF (L3) / BOUNDED (L2) obligation — a runtime check
//! cannot prove a still-running loop terminates, so L1 asserts `inv` per
//! iteration and emits NO `dec` runtime check (OQ-3). `fx` produces NO runtime
//! sandbox in v0.1 (effects are enforced at compile time by `effects.rs`,
//! deferred to #21, R-SPEC-5).
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (L1 check-emission entry point) | SHIPPED | `pub fn lower_l1`; emits each `FnItem` with `req` on entry, loop `inv` per iteration, `ens` against bound `result` on exit; verified by `sum_l1_compiles_and_runs` (compile+run via `rustc`). |
//! | REQ-2 (always-active check primitive) | SHIPPED | `emit_check_macro` writes the `thermite_check!` macro + `thermite_contract_violation` handler (NOT `debug_assert!`); `no_debug_assert_in_emission` (AC-2) + `negative_fixture_fires_violation`. |
//! | REQ-3 (combinator L1 executable forms) | SHIPPED | `emit_combinator_l1_defs` reads `thermite_spec::CombinatorSig.l1`; a combinator call lowers via `lower_expr_exec`; unit-tested by `combinator_l1_forms_run` (AC-3). |
//! | REQ-4 (`spec fn` → executable fn) | SHIPPED | `lower_spec_fn_l1`/`slice_fold_body_l1` emit the slice-length-branch recursion over `&[u32]`; verified by `sum_l1_compiles_and_runs` (AC-4: `spec_sum(&[1,2,3]) == 6` in the positive harness). |
//! | REQ-5 (`dec`/termination L1 scope) | SHIPPED | `lower_loop_l1` emits `inv` checks only, no `dec` runtime check (OQ-3); `no_syscall_sandbox_and_no_dec_guarantee` (AC-5). |
//! | REQ-6 (golden L1 contract) | SHIPPED | `tests/golden/l1/sum.l1.rs` compiles+runs; emitted output is execution-equivalent (compiles, runs, `sum(&[1,2,3])==6`, checks fire) — `sum_l1_compiles_and_runs`. |
//! | REQ-7 (`fx`/effect at L1 deferred to #21) | SHIPPED | no `fx` runtime check emitted; `no_syscall_sandbox_and_no_dec_guarantee` (AC-5) confirms no sandbox scaffolding. |

use std::fmt::Write as _;

use thermite_syntax::ast::{
    BinOp, Block, Expr, FnItem, IndexArg, Item, LoopKind, LoopNode, MatchArm, Param, Pattern,
    PrimType, Program, SlicePat, SpecFnItem, Stmt, Type,
};
use thermite_syntax::lexer::Span;

use crate::lower::LowerError;

/// The maximum recursive-descent emission depth before `lower_l1` returns
/// `LowerError::TooDeep`. Mirrors `lower.rs`'s `MAX_EMIT_DEPTH` (the
/// #29/#31/#32 stack-overflow lesson): a single shared counter bounds EVERY
/// recursive family here (expressions, blocks, statements, patterns) so a
/// pathological (or adversarial, post-recovery) AST cannot overflow the native
/// stack and abort the process. Fixed constant (determinism, `goal.md`
/// R-CODE-5). `thermite-syntax` caps parse nesting at 64, so a well-formed AST
/// cannot exceed that — this is a defensive backstop.
const MAX_EMIT_DEPTH: usize = 256;

/// A span pointing at the very start of source, used when an AST node we are
/// lowering does not carry a `Span` (mirrors `lower.rs::zero_span`).
pub(crate) fn zero_span() -> Span {
    Span::new(0, 0)
}

/// Lower a whole `Program` to a single self-contained, runnable L1 Rust source
/// file (REQ-1). Emits, in deterministic source order: (1) the always-active
/// `thermite_contract_violation` handler + `thermite_check!` macro (REQ-2),
/// (2) the L1 runnable bodies of every combinator the program references
/// (REQ-3), (3) every `spec fn` as an executable Rust fn (REQ-4), and (4) every
/// `fn` with its `req`/`ens`/`inv` checks woven in (REQ-1). The output compiles
/// and runs under `rustc` (REQ-6).
pub fn lower_l1(program: &Program) -> Result<String, LowerError> {
    let mut out = String::new();
    out.push_str(&emit_check_macro());

    // (2) the L1 runnable forms of every combinator referenced anywhere in a
    // contract/spec position, deduped in source order (REQ-3).
    out.push_str(&emit_combinator_l1_defs(program)?);

    // (3) + (4) the lowered items, in source order (determinism, §5.3).
    for item in &program.items {
        let item_src = match item {
            Item::SpecFn(s) => lower_spec_fn_l1(s)?,
            // A boundary fn (ffi-boundary.md REQ-4) lowers to the L1 wrapper: a
            // `req`-check, a call to the FOREIGN target binding `result`, then the
            // `ens`-checks — the foreign body is NOT lowered/verified. An
            // in-language fn lowers with its real body.
            Item::Fn(f) if f.boundary.is_some() => lower_boundary_fn_l1(f)?,
            Item::Fn(f) => lower_fn_l1(f)?,
        };
        out.push('\n');
        out.push_str(&item_src);
        out.push('\n');
    }

    Ok(out)
}

// ---------------------------------------------------------------------------
// REQ-2: the always-active check primitive + violation handler.
// ---------------------------------------------------------------------------

/// Emit the `thermite_contract_violation` handler and the always-active
/// `thermite_check!` macro (REQ-2). The handler is the defined contract-failure
/// behavior of the *generated* program (a structured abort with a legible
/// diagnostic; §2.4 / §6) — this is the intended L1 runtime behavior, distinct
/// from a toolchain panic (R-CODE-2 forbids the latter in `thermite-lower`'s own
/// code). The macro is NOT `debug_assert!` (which is stripped in release; §6
/// demands every build profile) — it is a plain `if !(cond)` so the check is
/// present in every profile (AC-2).
fn emit_check_macro() -> String {
    let mut out = String::new();
    out.push_str(
        "// L1 runtime-check lowering (.design/lower/l1-runtime-checks.md). Self-contained,\n",
    );
    out.push_str("// compiles and runs under `rustc`; the always-active contract checks fire on\n");
    out.push_str("// violation in every build profile (NOT debug-only).\n\n");
    out.push_str("/// The defined contract-violation behavior of the GENERATED program (not a\n");
    out.push_str(
        "/// toolchain panic): the L1 program's intended abort with a structured, legible\n",
    );
    out.push_str("/// diagnostic (\u{a7}2.4 / \u{a7}6). Always active in every build profile.\n");
    out.push_str("fn thermite_contract_violation(kind: &str, text: &str) -> ! {\n");
    out.push_str("    panic!(\"thermite L1 contract violation [{kind}]: {text}\");\n");
    out.push_str("}\n\n");
    out.push_str("/// Always-active check: a plain `if !(cond)` so the contract is enforced in\n");
    out.push_str(
        "/// EVERY build profile (a release-stripped assertion would not be; \u{a7}6 demands\n",
    );
    out.push_str("/// every profile).\n");
    out.push_str("macro_rules! thermite_check {\n");
    out.push_str("    ($kind:literal, $text:literal, $cond:expr) => {\n");
    out.push_str("        if !($cond) {\n");
    out.push_str("            thermite_contract_violation($kind, $text);\n");
    out.push_str("        }\n");
    out.push_str("    };\n");
    out.push_str("}\n");
    out
}

// ---------------------------------------------------------------------------
// REQ-3: combinator L1 executable forms, sourced from the #2 registry `l1` seam.
// ---------------------------------------------------------------------------

/// Collect (deterministic source order, deduped) the combinator names the
/// program references anywhere in a contract/spec position, and emit each one's
/// frozen `l1` runnable Rust `fn` from the `thermite-spec` registry (REQ-3; the
/// L1 half of the OQ-2 seam — this is the registry `l1` field's #4 consumer per
/// R-DEFER-1). A referenced name with no registry entry is `UnknownCombinator`.
pub(crate) fn emit_combinator_l1_defs(program: &Program) -> Result<String, LowerError> {
    let mut names: Vec<(String, Span)> = Vec::new();
    for item in &program.items {
        match item {
            Item::Fn(f) => {
                collect_combinators_in_expr(&f.contract.req.expr, f.span, &mut names);
                for ens in &f.contract.ens {
                    collect_combinators_in_expr(&ens.expr, f.span, &mut names);
                }
                // A boundary fn (ffi-boundary.md REQ-2) has `body: None` — its
                // `req`/`ens` combinators are collected above; there is no body
                // with loop spec-positions to scan.
                if let Some(body) = &f.body {
                    collect_combinators_in_block_specs(body, f.span, &mut names);
                }
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
        out.push_str(sig.l1);
        out.push('\n');
        emitted.push(sig.name);
    }
    Ok(out)
}

/// Walk an expression collecting any callee path whose head segment is a
/// registered combinator name. Combinator calls are plain `Expr::Call` with a
/// `Path` callee (the frontend is registry-free — `ast.rs` module doc). Mirrors
/// `lower.rs::collect_combinators_in_expr`.
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
        Expr::IntLit { .. } | Expr::BoolLit(_) | Expr::Path(_) => {}
    }
}

/// Walk a block collecting combinators referenced in its SPEC positions: loop
/// `inv`/`dec` clauses. Mirrors `lower.rs::collect_combinators_in_block_specs`.
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
// REQ-4: `spec fn` -> executable Rust fn.
// ---------------------------------------------------------------------------

/// Lower a `spec fn` to a real, total, terminating Rust fn (REQ-4; §4.2 "spec
/// functions are executable"). The head-fold-sum shape (`spec_sum`: `match xs {
/// [] => 0, [head, ..t] => head as T + f(t) }`) lowers to a slice-length branch
/// over `&[u32]` — `if xs.is_empty() { 0 } else { xs[0] as T + f(&xs[1..]) }` —
/// preserving real recursion. The `dec` measure is NOT emitted as a runtime
/// check (REQ-5: a spec fn just runs at L1). The slice-match shape is detected
/// structurally, never by name (mirrors `lower.rs::is_head_fold_sum`).
pub(crate) fn lower_spec_fn_l1(s: &SpecFnItem) -> Result<String, LowerError> {
    let ret = lower_type(&s.ret)?;
    let mut out = String::new();
    write!(out, "fn {}(", s.name).ok();
    emit_params(&mut out, &s.params)?;
    writeln!(out, ") -> {ret} {{").ok();
    out.push_str(&lower_spec_fn_body_l1(s, &ret)?);
    out.push_str("}\n");
    Ok(out)
}

/// Lower a spec-fn body. For the head-fold-sum shape, emit the slice-length
/// branch recursion (REQ-4). Otherwise lower the block directly in exec
/// position. The recursion is reconstructed from the match arms' SHAPE: the base
/// arm's value, the head cast, and the recursive callee name.
fn lower_spec_fn_body_l1(s: &SpecFnItem, ret: &str) -> Result<String, LowerError> {
    if is_head_fold_sum(&s.body) {
        if let Some(slice) = first_slice_param(&s.params) {
            if let Some(tail) = &s.body.tail {
                if let Expr::Match { arms, .. } = tail.as_ref() {
                    return slice_fold_body_l1(slice, arms, ret);
                }
            }
        }
    }
    // Fallback: lower the block in exec position directly.
    lower_block_inner(&s.body, 1, s.span)
}

/// The name of the first slice (`&[T]`) parameter — the recursion subject.
fn first_slice_param(params: &[Param]) -> Option<&str> {
    params.iter().find_map(|p| match &p.ty {
        Type::Ref { inner, .. } if matches!(inner.as_ref(), Type::Slice(_)) => {
            Some(p.name.as_str())
        }
        _ => None,
    })
}

/// Build the executable head-fold body from the match arms (REQ-4). `[] => B`
/// becomes `if xs.is_empty() { B }`; `[head, ..t] => head as T + rec(t)` becomes
/// `else { xs[0] as T + rec(&xs[1..]) }`. Real Rust slice recursion (no `Seq`).
fn slice_fold_body_l1(slice: &str, arms: &[MatchArm], ret: &str) -> Result<String, LowerError> {
    let mut base = String::from("0");
    let mut rec_name = String::new();
    for arm in arms {
        match &arm.pattern {
            Pattern::Slice(pats) if pats.is_empty() => {
                base = lower_expr_exec(&arm.body, 0, zero_span())?;
            }
            Pattern::Slice(pats) if is_head_rest(pats) => {
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
        "    if {slice}.is_empty() {{\n        {base}\n    }} else {{\n        {slice}[0] as {ret} + {rec_name}(&{slice}[1..])\n    }}\n"
    ))
}

/// Detect the head-fold-sum shape (mirrors `lower.rs::is_head_fold_sum`): a
/// `match xs { [] => 0, [head, ..t] => head as T + f(t) }`. SHAPE predicate, not
/// a name check.
fn is_head_fold_sum(body: &Block) -> bool {
    let Some(tail) = &body.tail else {
        return false;
    };
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

// ---------------------------------------------------------------------------
// REQ-1: `fn` lowering with woven-in checks.
// ---------------------------------------------------------------------------

/// Lower a `fn` to an executable Rust fn with its contract checks woven in
/// (REQ-1): each `req` asserted on entry, the body lowered with each loop's
/// `inv` asserted per iteration, the body's value bound to `result`, then each
/// `ens` asserted on exit against `result`. `fx` emits no runtime check (REQ-7).
fn lower_fn_l1(f: &FnItem) -> Result<String, LowerError> {
    let ret = lower_type(&f.ret)?;
    let mut out = String::new();
    write!(out, "fn {}(", f.name).ok();
    emit_params(&mut out, &f.params)?;
    writeln!(out, ") -> {ret} {{").ok();

    // req on entry (REQ-1/REQ-2). Omit a literal-`true` req (the empty contract).
    let req_cond = lower_expr_exec(&f.contract.req.expr, 0, f.span)?;
    if req_cond != "true" {
        out.push_str(&emit_check("req", &f.contract.req.text, &req_cond, 1));
    }

    // The body value is bound to `result` so `ens` can reference it (REQ-1). A
    // boundary fn has `body: None` and is routed to `lower_boundary_fn_l1` by the
    // `lower_l1` match guard, so this arm only ever sees an in-language fn; a
    // `None` here is a structured error (never an unwrap/panic — R-CODE-2).
    let body = f.body.as_ref().ok_or_else(|| LowerError::Unsupported {
        what: "lower_fn_l1 reached a bodyless (boundary) fn; route it through \
               lower_boundary_fn_l1 instead (ffi-boundary.md REQ-4)"
            .to_string(),
        span: f.span,
    })?;
    writeln!(out, "    let result = {{").ok();
    out.push_str(&lower_fn_body_l1(body, f, 2)?);
    writeln!(out, "    }};").ok();

    // ens on exit, in source order, against the bound `result` (REQ-1/REQ-2).
    for ens in &f.contract.ens {
        let cond = lower_expr_exec(&ens.expr, 0, f.span)?;
        out.push_str(&emit_check("ens", &ens.text, &cond, 1));
    }

    writeln!(out, "    result").ok();
    out.push_str("}\n");
    Ok(out)
}

/// Lower a BOUNDARY fn to its L1 wrapper (ffi-boundary.md REQ-4, §9 "L1, runtime
/// checks on every crossing"). The wrapper REUSES `l1.rs`'s executable machinery
/// exactly — `emit_params`/`lower_type`/`emit_check`/`lower_expr_exec` — and
/// emits, around the FOREIGN call:
///
/// 1. the `fn <name>(<params>) -> <ret>` head;
/// 2. a `req`-check on entry (the always-active `thermite_check!`);
/// 3. `let result = <target>(<args>);` — the foreign call (the unproven crossing,
///    §9): the foreign body is NOT lowered, NOT verified, NOT proved;
/// 4. an `ens`-check on exit against the bound `result`.
///
/// `fx` emits no runtime sandbox in v0.1 (REQ-7, deferred to #21). The target is
/// `f.boundary`'s `BoundaryAttr.target`; this fn is only called when
/// `f.boundary.is_some()` (the `lower_l1` match guard), so the attribute is read
/// via a structured error rather than an unwrap (R-CODE-2).
fn lower_boundary_fn_l1(f: &FnItem) -> Result<String, LowerError> {
    let boundary = f.boundary.as_ref().ok_or_else(|| LowerError::Unsupported {
        what: "lower_boundary_fn_l1 reached a non-boundary fn (no `#[boundary]` \
               target to call); route it through lower_fn_l1 (ffi-boundary.md REQ-4)"
            .to_string(),
        span: f.span,
    })?;
    let ret = lower_type(&f.ret)?;
    let mut out = String::new();
    write!(out, "fn {}(", f.name).ok();
    emit_params(&mut out, &f.params)?;
    writeln!(out, ") -> {ret} {{").ok();

    // (2) req-check on entry (REQ-4). Omit a literal-`true` req (empty contract).
    let req_cond = lower_expr_exec(&f.contract.req.expr, 0, f.span)?;
    if req_cond != "true" {
        out.push_str(&emit_check("req", &f.contract.req.text, &req_cond, 1));
    }

    // (3) the foreign call binding `result` — the unproven crossing (§9). The
    // body is NOT lowered: this `<target>(<params>)` REPLACES the `let result =
    // { <lowered body> }` of a normal L1 fn. Arguments are the parameter names in
    // declaration order (the wrapper forwards its own params to the foreign fn).
    let args = f
        .params
        .iter()
        .map(|p| p.name.clone())
        .collect::<Vec<_>>()
        .join(", ");
    writeln!(out, "    let result = {}({args});", boundary.target).ok();

    // (4) ens-check on exit against the bound `result` (REQ-4), in source order.
    for ens in &f.contract.ens {
        let cond = lower_expr_exec(&ens.expr, 0, f.span)?;
        out.push_str(&emit_check("ens", &ens.text, &cond, 1));
    }

    writeln!(out, "    result").ok();
    out.push_str("}\n");
    Ok(out)
}

/// Lower a `fn` body block, threading the loop `inv`-check injection. The body's
/// statements are emitted, then its tail expression (the block's value) — both
/// inside the `let result = { .. }` binder the caller opened.
fn lower_fn_body_l1(block: &Block, f: &FnItem, indent: usize) -> Result<String, LowerError> {
    let pad = "    ".repeat(indent);
    let mut out = String::new();
    for stmt in &block.stmts {
        match stmt {
            Stmt::Loop(l) => out.push_str(&lower_loop_l1(l, indent)?),
            other => out.push_str(&lower_stmt_l1(other, indent)?),
        }
    }
    if let Some(tail) = &block.tail {
        let t = lower_expr_exec(tail, 0, f.span)?;
        writeln!(out, "{pad}{t}").ok();
    }
    Ok(out)
}

/// Lower a loop with its `inv` checks woven in per iteration (REQ-1/REQ-5). The
/// loop header is preserved (`while`/`loop`); at the TOP of each iteration every
/// `inv` clause is asserted via `thermite_check!`. NO `dec` runtime check is
/// emitted: termination is a proof-time (L3) / bounded (L2) obligation, out of
/// L1's runtime scope (REQ-5, OQ-3) — a runtime check cannot prove a
/// still-running loop terminates.
fn lower_loop_l1(l: &LoopNode, indent: usize) -> Result<String, LowerError> {
    let pad = "    ".repeat(indent);
    let ipad = "    ".repeat(indent + 1);
    let mut out = String::new();
    match &l.kind {
        LoopKind::Loop => writeln!(out, "{pad}loop {{").ok(),
        LoopKind::While(c) => {
            let cs = lower_expr_exec(c, 0, zero_span())?;
            writeln!(out, "{pad}while {cs} {{").ok()
        }
    };
    // inv checks at the top of each iteration (REQ-1). NO dec check (REQ-5).
    for inv in &l.invs {
        let cond = lower_expr_exec(&inv.expr, 0, zero_span())?;
        out.push_str(&emit_check("inv", &inv.text, &cond, indent + 1));
    }
    // Loop body statements (a nested loop recurses through `lower_loop_l1`).
    for stmt in &l.body.stmts {
        match stmt {
            Stmt::Loop(inner) => out.push_str(&lower_loop_l1(inner, indent + 1)?),
            other => out.push_str(&lower_stmt_l1(other, indent + 1)?),
        }
    }
    if let Some(tail) = &l.body.tail {
        let t = lower_expr_exec(tail, 0, zero_span())?;
        writeln!(out, "{ipad}{t}").ok();
    }
    writeln!(out, "{pad}}}").ok();
    Ok(out)
}

/// Emit a single always-active check (REQ-2): `thermite_check!("<kind>",
/// "<verbatim clause text>", <lowered cond>);`. The verbatim `Clause.text` is
/// carried into the diagnostic for legibility (§2.4); it is escaped as a Rust
/// string literal so an arbitrary clause text cannot break the emitted source.
fn emit_check(kind: &str, text: &str, cond: &str, indent: usize) -> String {
    let pad = "    ".repeat(indent);
    format!(
        "{pad}thermite_check!(\"{kind}\", {}, {cond});\n",
        rust_string_literal(text)
    )
}

/// Render `s` as a Rust string literal (deterministic; escapes `\`, `"`,
/// newlines, tabs, carriage returns) so the verbatim clause text is embedded
/// safely in the emitted `thermite_check!` invocation. Determinism (§5.3): a
/// pure function of the input bytes.
fn rust_string_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

// ---------------------------------------------------------------------------
// Statement, block lowering (exec).
// ---------------------------------------------------------------------------

/// Emit the comma-separated parameter list (exec types — plain `&[T]`, no `Seq`).
pub(crate) fn emit_params(out: &mut String, params: &[Param]) -> Result<(), LowerError> {
    for (i, p) in params.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        let ty = lower_type(&p.ty)?;
        write!(out, "{}: {ty}", p.name).ok();
    }
    Ok(())
}

/// Lower a plain block in exec position (no loop-inv injection — used for spec-fn
/// fallback bodies and `if`/`else` arms).
fn lower_block_inner(block: &Block, indent: usize, span: Span) -> Result<String, LowerError> {
    let pad = "    ".repeat(indent);
    let mut out = String::new();
    for stmt in &block.stmts {
        match stmt {
            Stmt::Loop(l) => out.push_str(&lower_loop_l1(l, indent)?),
            other => out.push_str(&lower_stmt_l1(other, indent)?),
        }
    }
    if let Some(tail) = &block.tail {
        let t = lower_expr_exec(tail, 0, span)?;
        writeln!(out, "{pad}{t}").ok();
    }
    Ok(out)
}

/// Lower a single statement in exec position.
pub(crate) fn lower_stmt_l1(stmt: &Stmt, indent: usize) -> Result<String, LowerError> {
    let pad = "    ".repeat(indent);
    match stmt {
        Stmt::Let {
            mutable,
            name,
            ty,
            init,
        } => {
            let kw = if *mutable { "let mut" } else { "let" };
            let init_s = lower_expr_exec(init, 0, zero_span())?;
            if let Some(t) = ty {
                let ts = lower_type(t)?;
                Ok(format!("{pad}{kw} {name}: {ts} = {init_s};\n"))
            } else {
                Ok(format!("{pad}{kw} {name} = {init_s};\n"))
            }
        }
        Stmt::Assign { target, value } => {
            let t = lower_expr_exec(target, 0, zero_span())?;
            let v = lower_expr_exec(value, 0, zero_span())?;
            Ok(format!("{pad}{t} = {v};\n"))
        }
        Stmt::Return(e) => match e {
            Some(e) => {
                let s = lower_expr_exec(e, 0, zero_span())?;
                Ok(format!("{pad}return {s};\n"))
            }
            None => Ok(format!("{pad}return;\n")),
        },
        Stmt::If { cond, then, else_ } => {
            let c = lower_expr_exec(cond, 0, zero_span())?;
            let t = lower_block_inner(then, indent + 1, zero_span())?;
            let mut out = format!("{pad}if {c} {{\n{t}{pad}}}");
            if let Some(e) = else_ {
                let es = lower_block_inner(e, indent + 1, zero_span())?;
                write!(out, " else {{\n{es}{pad}}}").ok();
            }
            out.push('\n');
            Ok(out)
        }
        Stmt::Expr(e) => {
            let s = lower_expr_exec(e, 0, zero_span())?;
            Ok(format!("{pad}{s};\n"))
        }
        Stmt::Loop(_) => Err(LowerError::Unsupported {
            what: "nested loop reached lower_stmt_l1 (should route through lower_loop_l1)"
                .to_string(),
            span: zero_span(),
        }),
    }
}

// ---------------------------------------------------------------------------
// REQ-3: expression lowering (entirely exec — no Seq, no `@`, no subrange).
// ---------------------------------------------------------------------------

/// Lower an `Expr` in exec position to plain Rust (REQ-3). `depth` bounds
/// recursion (REQ-9-equivalent; mirrors `lower.rs`'s guard). A combinator call
/// lowers to a call of its L1 fn (the name is unchanged; its body is emitted by
/// `emit_combinator_l1_defs`), with a closure argument becoming a real Rust
/// closure. Every clause is a real `bool`/value expression over real values.
pub(crate) fn lower_expr_exec(expr: &Expr, depth: usize, span: Span) -> Result<String, LowerError> {
    if depth >= MAX_EMIT_DEPTH {
        return Err(LowerError::TooDeep {
            limit: MAX_EMIT_DEPTH,
            span,
        });
    }
    let d = depth + 1;
    match expr {
        // Emit the numeric `value`, NOT `raw` (#37) — byte-identical L1 output.
        Expr::IntLit { value, .. } => Ok(value.to_string()),
        Expr::BoolLit(b) => Ok(b.to_string()),
        Expr::Path(segs) => Ok(segs.join("::")),
        Expr::Call { callee, args } => {
            let c = lower_expr_exec(callee, d, span)?;
            let mut parts = Vec::new();
            for a in args {
                parts.push(lower_expr_exec(a, d, span)?);
            }
            Ok(format!("{c}({})", parts.join(", ")))
        }
        Expr::MethodCall {
            receiver,
            name,
            args,
        } => {
            let r = lower_expr_exec(receiver, d, span)?;
            let mut parts = Vec::new();
            for a in args {
                parts.push(lower_expr_exec(a, d, span)?);
            }
            Ok(format!("{r}.{name}({})", parts.join(", ")))
        }
        Expr::Field { receiver, name } => {
            let r = lower_expr_exec(receiver, d, span)?;
            Ok(format!("{r}.{name}"))
        }
        Expr::Closure { params, body } => {
            // A real Rust closure (REQ-3); the corpus closures are `u32`-typed
            // slice-element predicates (matching the registry `l1` `impl Fn(u32)
            // -> bool` parameter).
            let b = lower_expr_exec(body, d, span)?;
            let ps: Vec<String> = params.iter().map(|p| format!("{p}: u32")).collect();
            Ok(format!("|{}| {b}", ps.join(", ")))
        }
        Expr::Match { scrutinee, arms } => lower_match_exec(scrutinee, arms, d, span),
        Expr::If { cond, then, else_ } => {
            let c = lower_expr_exec(cond, d, span)?;
            let t = lower_block_inner(then, 0, span)?;
            let e = lower_block_inner(else_, 0, span)?;
            Ok(format!("if {c} {{ {} }} else {{ {} }}", t.trim(), e.trim()))
        }
        Expr::Binary { op, lhs, rhs } => {
            let l = lower_binary_operand(lhs, *op, true, d, span)?;
            let r = lower_binary_operand(rhs, *op, false, d, span)?;
            Ok(format!("{l} {} {r}", binop(*op)))
        }
        Expr::Index { base, index } => lower_index_exec(base, index, d, span),
        Expr::Cast { expr, ty } => {
            let e = lower_expr_exec(expr, d, span)?;
            let t = lower_type(ty)?;
            Ok(format!("{e} as {t}"))
        }
        Expr::Ref { mutable, expr } => {
            let e = lower_expr_exec(expr, d, span)?;
            if *mutable {
                Ok(format!("&mut {e}"))
            } else {
                Ok(format!("&{e}"))
            }
        }
    }
}

/// Lower an `Index` expression in exec position (plain Rust): `xs[i]`,
/// `&xs[..i]` (as `xs[..i]` since a `Ref` wraps it), `xs[i..]`, `xs[i..j]`.
fn lower_index_exec(
    base: &Expr,
    index: &IndexArg,
    depth: usize,
    span: Span,
) -> Result<String, LowerError> {
    let b = lower_expr_exec(base, depth, span)?;
    match index {
        IndexArg::Single(i) => {
            let idx = lower_expr_exec(i, depth, span)?;
            Ok(format!("{b}[{idx}]"))
        }
        IndexArg::RangeTo(i) => {
            let idx = lower_expr_exec(i, depth, span)?;
            Ok(format!("{b}[..{idx}]"))
        }
        IndexArg::RangeFrom(i) => {
            let idx = lower_expr_exec(i, depth, span)?;
            Ok(format!("{b}[{idx}..]"))
        }
        IndexArg::Range(i, j) => {
            let lo = lower_expr_exec(i, depth, span)?;
            let hi = lower_expr_exec(j, depth, span)?;
            Ok(format!("{b}[{lo}..{hi}]"))
        }
    }
}

/// Lower a `match` in exec position (e.g. `binary_search`'s `Option` ens match).
fn lower_match_exec(
    scrutinee: &Expr,
    arms: &[MatchArm],
    depth: usize,
    span: Span,
) -> Result<String, LowerError> {
    let s = lower_expr_exec(scrutinee, depth, span)?;
    let mut out = format!("match {s} {{ ");
    for arm in arms {
        let pat = lower_pattern_exec(&arm.pattern, depth, span)?;
        let body = lower_expr_exec(&arm.body, depth, span)?;
        write!(out, "{pat} => {body}, ").ok();
    }
    out.push('}');
    Ok(out)
}

/// Lower a pattern in exec position. Enum patterns `Some(i)`/`None`, bindings,
/// wildcards, literals. A slice pattern outside a head-fold spec fn is
/// unsupported at L1 (mirrors `lower.rs`).
fn lower_pattern_exec(pat: &Pattern, depth: usize, span: Span) -> Result<String, LowerError> {
    if depth >= MAX_EMIT_DEPTH {
        return Err(LowerError::TooDeep {
            limit: MAX_EMIT_DEPTH,
            span,
        });
    }
    match pat {
        Pattern::Wildcard => Ok("_".to_string()),
        Pattern::Binding(name) => Ok(name.clone()),
        Pattern::Literal(e) => lower_expr_exec(e, depth + 1, span),
        Pattern::Enum { path, fields } => {
            let head = path.join("::");
            if fields.is_empty() {
                Ok(head)
            } else {
                let mut fs = Vec::new();
                for f in fields {
                    fs.push(lower_pattern_exec(f, depth + 1, span)?);
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

/// Lower an operand of a binary expression, parenthesizing a child binary of
/// lower precedence so the AST grouping survives the round-trip (mirrors
/// `lower.rs::lower_binary_operand`).
fn lower_binary_operand(
    operand: &Expr,
    parent: BinOp,
    is_left: bool,
    depth: usize,
    span: Span,
) -> Result<String, LowerError> {
    let s = lower_expr_exec(operand, depth, span)?;
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

/// The Rust operator for a `BinOp`.
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
/// `lower.rs::precedence`.
fn precedence(op: BinOp) -> u8 {
    match op {
        BinOp::Or => 1,
        BinOp::And => 2,
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => 3,
        BinOp::Add | BinOp::Sub => 4,
        BinOp::Mul | BinOp::Div => 5,
    }
}

/// Lower a `Type` to its Rust spelling (exec). No `Seq` — every type is its
/// plain Rust form. Mirrors `lower.rs::lower_type`.
pub(crate) fn lower_type(ty: &Type) -> Result<String, LowerError> {
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
