//! L2 emission: compile a validated `thermite-syntax` `Program` into a single,
//! self-contained Rust source `String` carrying a **Kani proof harness** for
//! every `fn` — the L2 rung of the ladder (`.design/lower/l2-kani.md`;
//! `thermite-design.md` §6/§5.1/§13 v0.2). Where `lower.rs` emits Verus
//! annotations for an SMT *proof* (L3, "holds for all inputs") and `l1.rs` emits
//! Rust that *runs* the contract at runtime (L1), `l2.rs` emits a
//! `#[kani::proof] #[kani::unwind(K)]` fn that creates BOUNDED SYMBOLIC inputs
//! (`kani::any()` + `kani::assume` bounds), `assume`s the `req`, calls the
//! *executable* body, binds `result`, and `assert!`s each `ens` — Kani then
//! bounded-model-checks the contract for ALL inputs UP TO the bound (`< L3`).
//!
//! Kani verifies executable Rust (CBMC over the compiled body), NOT SMT spec
//! annotations, so the harness reuses the **L1 executable lowering** (`l1.rs`):
//! the real recursive `spec_sum` (`lower_spec_fn_l1`), the real combinator loops
//! (`emit_combinator_l1_defs`), the real expression/statement forms
//! (`lower_expr_exec`/`lower_stmt_l1`). The only NEW surface `l2.rs` adds is the
//! harness wrapper (symbolic inputs + `assume`/`assert`) and the type-driven
//! bound inference. The contract checks `l1.rs` weaves into the body
//! (`thermite_check!` per `req`/`ens`/`inv`) are NOT emitted here: Kani derives
//! the loop bound from the `#[kani::unwind]`, and `req`/`ens` become the harness's
//! `kani::assume`/`assert!` (the §6 "bounded check", not a runtime check).
//!
//! Governing design: `.design/lower/l2-kani.md`.
//! Reference (real `cargo kani 0.67.0` runs, hand-grounded): `conformance/sum.th`
//! → `VERIFICATION:- SUCCESSFUL` at `N = 4`/`unwind(5)`; `conformance/binary_search.th`
//! → `VERIFICATION:- SUCCESSFUL` at `N = 4`/`unwind(6)`.
//!
//! ## The bound is type-driven, not name-driven (REQ-2 — the #9 headline)
//!
//! The symbolic bound for each parameter is inferred from its TYPE (SHAPE-keyed),
//! never from the program name. A `&[T]`/`&mut [T]` parameter becomes a symbolic
//! `[T; N]` array of fixed capacity [`SLICE_BOUND`] plus a symbolic `len` with
//! `kani::assume(len <= N)`, sliced `&data[..len]`; an integer/`bool` scalar
//! becomes a full-range `kani::any()` (the `req` then prunes it). The slice
//! scaffolding is emitted purely from seeing a `&[T]` parameter, identical for
//! `sum` and `binary_search` (AC-4 — no `if name == ...`).
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (L2 harness-emission entry point) | SHIPPED | `pub fn lower_l2` emits, per `FnItem`, a `#[kani::proof]` harness reusing the L1 spec-fn (`lower_spec_fn_l1`) + combinator (`emit_combinator_l1_defs`) + body lowering; consumer `forge::kani::run_kani` (`forge/src/kani.rs`) + `l2_conformance::sum_harness_verifies_to_bound`. |
//! | REQ-2 (type-driven bound inference) | SHIPPED | `infer_symbolic_input` keys on `struct Param`'s `struct Type` (`&[T]`→array+len, scalar→`kani::any()`); `l2_conformance::bound_is_type_derived_not_name_derived` (AC-4). |
//! | REQ-3 (unwind bounds for loops/recursion) | SHIPPED | `unwind_bound` derives `K` from [`SLICE_BOUND`] SHAPE-keyed on the loop kind (`while`→`N+1`, unconditional `loop`→`N+2`); emitted as `#[kani::unwind(K)]`; `l2_conformance::sum_harness_verifies_to_bound`/`binary_search_harness_verifies_to_bound`. |
//! | REQ-6 (the "up to bound" caveat) | SHIPPED | `bound_string` records `slice ≤ N, unwind K`; `forge::kani::run_kani` carries it onto the `Level::L2` cert (`forge/src/kani.rs`); the L2 cert is programmatically distinct from L3 (`manifest::Level::L2`). |
//! | REQ-9 (determinism) | SHIPPED | [`SLICE_BOUND`] is a fixed `const`; `unwind_bound` is a pure function of the AST; no wall-clock — a pure function of the `Program` bytes (R-CODE-5). |
//! | REQ-1 (no panics; `LowerError`) | SHIPPED | `lower_l2` returns `Result<String, LowerError>` (shared with `lower.rs`); an un-lowerable construct is an `Err`, never an `unwrap`/`panic!`; `l2_conformance::unlowerable_is_err_not_panic` (AC-9). |

use std::fmt::Write as _;

use thermite_syntax::ast::{
    Block, FnItem, Item, LoopKind, LoopNode, Param, PrimType, Program, Stmt, Type,
};
use thermite_syntax::lexer::Span;

use crate::l1::{
    emit_combinator_l1_defs, emit_params, lower_expr_exec, lower_spec_fn_l1, lower_stmt_l1,
    lower_type, zero_span,
};
use crate::lower::LowerError;

/// The fixed default slice bound `N` (REQ-2 / `.design/lower/l2-kani.md` §"The
/// slice bound `N`"). A `&[T]` parameter is modelled by a symbolic `[T; N]` array
/// plus a symbolic `len <= N`. `N = 4` verifies both corpus programs in under a
/// second (the grounded value; the design's §5.2 illustration is `8`, and the
/// `4`–`8` range is documented). A FIXED constant — the L2 result is
/// reproducible given the same bound (determinism, R-CODE-5 / §5.3); the bound is
/// stated on the certificate so L2 is never oversold as a proof (REQ-6).
pub(crate) const SLICE_BOUND: usize = 4;

/// Lower a whole `Program` to a single self-contained Kani-harness Rust source
/// file (REQ-1). Emits, in deterministic source order: (1) the L1 runnable forms
/// of every combinator the program references (REQ-1, reusing `l1.rs`), (2) every
/// `spec fn` as an executable Rust fn (reusing `l1.rs`), (3) every `fn` as a
/// CHECK-FREE executable body plus a `#[kani::proof] #[kani::unwind(K)]` harness
/// that builds bounded symbolic inputs, `assume`s the `req`, calls the body, and
/// `assert!`s each `ens`.
///
/// The harness is gated behind `#[cfg(kani)]` so the file still compiles under a
/// plain `rustc` (kani injects the `kani` cfg + the `kani` crate). The body and
/// spec fns are NOT cfg-gated (kani compiles and reasons over them).
pub fn lower_l2(program: &Program) -> Result<String, LowerError> {
    let mut out = String::new();
    out.push_str("// L2 Kani-harness lowering (.design/lower/l2-kani.md). Self-contained; the\n");
    out.push_str(
        "// `#[kani::proof]` harnesses bounded-model-check each contract UP TO the bound\n",
    );
    out.push_str("// (slices <= N, loops/recursion unwound <= K) — L2, strictly below L3.\n");

    // (1) the L1 runnable forms of every referenced combinator (REQ-1, reused).
    out.push_str(&emit_combinator_l1_defs(program)?);

    // (2) + (3) the lowered items, in source order (determinism, §5.3).
    for item in &program.items {
        match item {
            // A `spec fn` is the reused L1 executable recursion (Kani reasons over
            // it as the `ens` reference, e.g. `result == spec_sum(xs)`).
            Item::SpecFn(s) => {
                out.push('\n');
                out.push_str(&lower_spec_fn_l1(s)?);
                out.push('\n');
            }
            // A `fn` is a check-free executable body PLUS its Kani harness.
            Item::Fn(f) => {
                out.push('\n');
                out.push_str(&lower_fn_body_exec(f)?);
                out.push('\n');
                out.push_str(&emit_harness(f)?);
                out.push('\n');
            }
            // Basis Stage 1a (`.design/basis/01-adts.md`): a `struct`/`enum`
            // item is UNREACHABLE — an ADT program dies at the validator gate
            // before L2 lowering. Honest neutral value: the existing
            // `Unsupported` error at the item span (1c lowers ADTs). NOT a panic.
            Item::Struct(s) => {
                return Err(LowerError::Unsupported {
                    what: "struct item (ADT L2 lowering lands in basis Stage 1c)".to_string(),
                    span: s.span,
                })
            }
            Item::Enum(e) => {
                return Err(LowerError::Unsupported {
                    what: "enum item (ADT L2 lowering lands in basis Stage 1c)".to_string(),
                    span: e.span,
                })
            }
        }
    }

    Ok(out)
}

// ---------------------------------------------------------------------------
// REQ-1: the check-free executable `fn` body (Kani checks executable Rust).
// ---------------------------------------------------------------------------

/// Lower a `fn` to a plain executable Rust fn WITHOUT the L1 contract checks
/// (REQ-1). Kani derives the loop bound from `#[kani::unwind]` (not from the
/// `inv`), and the `req`/`ens` are checked by the HARNESS's `kani::assume`/
/// `assert!`, so the body here is the bare computation — identical to the L1
/// lowering sans the per-iteration `inv` / entry-`req` / exit-`ens` checks
/// (`.design/lower/l2-kani.md` §"Why reuse L1, not L3"). Reuses `l1.rs`'s
/// expression/statement/type lowering.
fn lower_fn_body_exec(f: &FnItem) -> Result<String, LowerError> {
    let ret = lower_type(&f.ret)?;
    let mut out = String::new();
    write!(out, "fn {}(", f.name).ok();
    emit_params(&mut out, &f.params)?;
    writeln!(out, ") -> {ret} {{").ok();
    // A boundary fn (ffi-boundary.md REQ-2) has `body: None` and is NEVER lowered
    // to an L2 kani harness — `forge`'s `check.rs` routes it to L1 BEFORE any L2
    // attempt (the foreign body cannot be bounded-checked). A `None` here is a
    // structured error (R-CODE-2), never an unwrap.
    let body = f.body.as_ref().ok_or_else(|| LowerError::Unsupported {
        what: "lower_l2 (kani) reached a bodyless (boundary) fn; a boundary fn \
               certifies at L1 and is never bounded-checked (ffi-boundary.md OQ-3)"
            .to_string(),
        span: f.span,
    })?;
    out.push_str(&lower_block_exec(body, 1, f.span)?);
    out.push_str("}\n");
    Ok(out)
}

/// Lower a block in exec position WITHOUT contract checks (REQ-1). A loop routes
/// through the check-free `lower_loop_exec`; every other statement reuses
/// `l1.rs`'s `lower_stmt_l1` (which carries no checks for non-loop statements).
fn lower_block_exec(block: &Block, indent: usize, span: Span) -> Result<String, LowerError> {
    let pad = "    ".repeat(indent);
    let mut out = String::new();
    for stmt in &block.stmts {
        match stmt {
            Stmt::Loop(l) => out.push_str(&lower_loop_exec(l, indent)?),
            other => out.push_str(&lower_stmt_l1(other, indent)?),
        }
    }
    if let Some(tail) = &block.tail {
        let t = lower_expr_exec(tail, 0, span)?;
        writeln!(out, "{pad}{t}").ok();
    }
    Ok(out)
}

/// Lower a loop WITHOUT the `inv` checks (REQ-1/REQ-3). The header (`while`/
/// `loop`) is preserved; NO `thermite_check!("inv", ..)` is emitted (Kani bounds
/// the loop via `#[kani::unwind]`, not via the invariant), NO `dec` check. A
/// nested loop recurses. Mirrors `l1.rs::lower_loop_l1` minus the check weaving.
fn lower_loop_exec(l: &LoopNode, indent: usize) -> Result<String, LowerError> {
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
    for stmt in &l.body.stmts {
        match stmt {
            Stmt::Loop(inner) => out.push_str(&lower_loop_exec(inner, indent + 1)?),
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

// ---------------------------------------------------------------------------
// REQ-1/REQ-2/REQ-3: the Kani proof harness.
// ---------------------------------------------------------------------------

/// Emit the `#[cfg(kani)] #[kani::proof] #[kani::unwind(K)]` harness for `f`
/// (REQ-1). Body order (`.design/lower/l2-kani.md` §"Harness shape"):
/// 1. build symbolic inputs from the parameter TYPES (REQ-2, type-driven);
/// 2. `kani::assume` the `req` (after the symbolic construction, so a `req` that
///    further bounds a value prunes the search — REQ-2);
/// 3. call the executable body, binding `result`;
/// 4. `assert!` each `ens` against the bound `result`.
fn emit_harness(f: &FnItem) -> Result<String, LowerError> {
    // A boundary fn (ffi-boundary.md REQ-2) has `body: None` and never reaches an
    // L2 harness — its body is foreign, so there is nothing to bounded-check. A
    // `None` here is a structured error (R-CODE-2), never an unwrap.
    let body = f.body.as_ref().ok_or_else(|| LowerError::Unsupported {
        what: "lower_l2 (kani) reached a bodyless (boundary) fn (ffi-boundary.md OQ-3)".to_string(),
        span: f.span,
    })?;
    let unwind = unwind_bound(body);
    let mut out = String::new();
    out.push_str("#[cfg(kani)]\n");
    out.push_str("#[kani::proof]\n");
    writeln!(out, "#[kani::unwind({unwind})]").ok();
    writeln!(out, "fn check_{}() {{", f.name).ok();

    // (1) symbolic inputs, type-driven (REQ-2). A slice declares the bound `N`.
    if f.params.iter().any(|p| is_slice_param(&p.ty)) {
        writeln!(out, "    const N: usize = {SLICE_BOUND};").ok();
    }
    let mut call_args: Vec<String> = Vec::with_capacity(f.params.len());
    for p in &f.params {
        let (decl, arg) = infer_symbolic_input(p)?;
        out.push_str(&decl);
        call_args.push(arg);
    }

    // (2) assume the `req` (omit a literal-`true` empty contract).
    let req = lower_expr_exec(&f.contract.req.expr, 0, f.span)?;
    if req != "true" {
        writeln!(out, "    kani::assume({req});").ok();
    }

    // (3) call the executable body, binding `result`.
    writeln!(
        out,
        "    let result = {}({});",
        f.name,
        call_args.join(", ")
    )
    .ok();

    // (4) assert each `ens` against `result`, in source order (REQ-1, no
    // weakening — R-DEFER-9).
    for ens in &f.contract.ens {
        let cond = lower_expr_exec(&ens.expr, 0, f.span)?;
        writeln!(out, "    assert!({cond});").ok();
    }

    out.push_str("}\n");
    Ok(out)
}

/// Infer the symbolic-input declaration + the call argument for one parameter
/// from its TYPE (REQ-2 — the type-driven bound inference, SHAPE-keyed, never
/// name-keyed). Returns `(declaration_lines, call_argument_expression)`:
///
/// | Param type | Construction | Bound |
/// |---|---|---|
/// | `&[T]` / `&mut [T]` | `let len = kani::any(); kani::assume(len <= N); let mut data: [T; N] = kani::any(); let xs = &data[..len];` | `len ≤ N` |
/// | `u32` / `u64` / `usize` / `bool` | `let x: T = kani::any();` | full symbolic range (the `req` narrows it) |
///
/// The slice scaffolding is emitted purely from seeing a `&[T]` parameter (AC-4).
fn infer_symbolic_input(p: &Param) -> Result<(String, String), LowerError> {
    let name = &p.name;
    if let Some(elem) = slice_elem(&p.ty) {
        let elem_ty = lower_type(elem)?;
        let len_name = format!("{name}_len");
        let data_name = format!("{name}_data");
        let mut decl = String::new();
        writeln!(decl, "    let {len_name}: usize = kani::any();").ok();
        writeln!(decl, "    kani::assume({len_name} <= N);").ok();
        writeln!(decl, "    let {data_name}: [{elem_ty}; N] = kani::any();").ok();
        let mutability = if is_mut_slice(&p.ty) { "&mut " } else { "&" };
        writeln!(
            decl,
            "    let {name} = {mutability}{data_name}[..{len_name}];"
        )
        .ok();
        Ok((decl, name.clone()))
    } else {
        // A scalar (integer or bool): a full-range symbolic value (REQ-2). The
        // `req` (assumed after) narrows it. Every scalar shape lowers via the
        // shared `lower_type` so the whole closed `PrimType` set is covered.
        match &p.ty {
            Type::Prim(_) => {
                let ty = lower_type(&p.ty)?;
                let decl = format!("    let {name}: {ty} = kani::any();\n");
                Ok((decl, name.clone()))
            }
            other => Err(LowerError::Unsupported {
                what: format!(
                    "L2 symbolic-input inference for parameter `{name}` of type `{}`",
                    type_label(other)
                ),
                span: zero_span(),
            }),
        }
    }
}

/// Derive the CBMC unwind bound `K` from the loop SHAPE (REQ-3). Every corpus
/// loop is slice-length-bounded by [`SLICE_BOUND`] `N`, so a `while`-over-slice
/// loop runs at most `N` iterations and needs `K = N + 1` (the extra iteration
/// lets CBMC prove the loop exits — the "unwinding assertion"). An unconditional
/// `loop` (e.g. `binary_search`'s) is set one higher, `K = N + 2`, conservatively
/// above the slice length (the grounded `unwind(6)` for `binary_search`'s `N = 4`
/// slice). A too-small `K` is NOT a false pass: Kani reports an explicit
/// `unwinding assertion loop 0` failure which `run_kani` parses as non-L2 (AC-5).
/// SHAPE-keyed on the deepest loop kind, never name-keyed; a fixed function of
/// the AST (determinism, R-CODE-5).
fn unwind_bound(body: &Block) -> usize {
    // The unwind annotation applies to the whole harness, so pick the bound the
    // LOOSEST (most permissive) loop in the body needs: an unconditional `loop`
    // (`N + 2`) dominates a `while` (`N + 1`); recursion over the slice is itself
    // `N + 1`. The base, if the body has no loop, is `N + 1` (the spec-fn
    // recursion the `ens` may call, e.g. `spec_sum`, recurses `N` times).
    let mut k = SLICE_BOUND + 1;
    if has_unconditional_loop(body) {
        k = SLICE_BOUND + 2;
    }
    k
}

/// True if `body` (recursively) contains an unconditional `loop` (vs a `while`).
/// An unconditional `loop`'s exit is data-driven (a `return`/`break`), so CBMC
/// needs a slightly higher unwind than a `while`-over-slice (REQ-3).
fn has_unconditional_loop(body: &Block) -> bool {
    body.stmts.iter().any(stmt_has_unconditional_loop)
}

/// Whether a single statement contains an unconditional `loop` (recursing into
/// nested loops, `if` arms, and loop bodies). Bounded by the AST depth, which
/// `thermite-syntax` caps at 64 (no separate guard needed — this is a finite walk
/// over the already-parsed tree).
fn stmt_has_unconditional_loop(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Loop(l) => matches!(l.kind, LoopKind::Loop) || has_unconditional_loop(&l.body),
        Stmt::If { then, else_, .. } => {
            has_unconditional_loop(then)
                || else_.as_ref().map(has_unconditional_loop).unwrap_or(false)
        }
        _ => false,
    }
}

/// The bound caveat string recorded on the L2 certificate (REQ-6 / AC-6): the L2
/// result holds for all inputs UP TO this bound, not for all inputs (that is L3).
/// A reader sees `slice ≤ N, unwind K` and knows the L2 caveat. A pure function
/// of the program (determinism, R-CODE-5).
pub fn bound_string(program: &Program) -> String {
    let unwind = program
        .items
        .iter()
        .filter_map(|i| match i {
            // A boundary fn (ffi-boundary.md REQ-2) has `body: None` — no loop to
            // bound, so it contributes no unwind requirement.
            Item::Fn(f) => f.body.as_ref().map(unwind_bound),
            Item::SpecFn(_) => None,
            // Basis Stage 1a (`.design/basis/01-adts.md`): a `struct`/`enum`
            // item has no loop body to unwind-bound → contributes nothing
            // (neutral value `None`). Dead-in-1a (gated at the validator).
            Item::Struct(_) | Item::Enum(_) => None,
        })
        .max()
        .unwrap_or(SLICE_BOUND + 1);
    format!("slice <= {SLICE_BOUND}, unwind {unwind}")
}

// ---------------------------------------------------------------------------
// Type SHAPE helpers (REQ-2 — type-driven, never name-driven).
// ---------------------------------------------------------------------------

/// The element type of a `&[T]` / `&mut [T]` parameter, or `None` if `ty` is not
/// a slice reference. The SHAPE that drives the symbolic-array scaffolding.
fn slice_elem(ty: &Type) -> Option<&Type> {
    match ty {
        Type::Ref { inner, .. } => match inner.as_ref() {
            Type::Slice(elem) => Some(elem),
            _ => None,
        },
        _ => None,
    }
}

/// True if `ty` is a slice reference `&[T]` / `&mut [T]`.
fn is_slice_param(ty: &Type) -> bool {
    slice_elem(ty).is_some()
}

/// True if `ty` is a MUTABLE slice reference `&mut [T]`.
fn is_mut_slice(ty: &Type) -> bool {
    matches!(ty, Type::Ref { mutable: true, inner } if matches!(inner.as_ref(), Type::Slice(_)))
}

/// A short human label for a `Type` in an `Unsupported` diagnostic (no panic).
fn type_label(ty: &Type) -> String {
    match ty {
        Type::Prim(PrimType::U32) => "u32".to_string(),
        Type::Prim(PrimType::U64) => "u64".to_string(),
        Type::Prim(PrimType::Usize) => "usize".to_string(),
        Type::Prim(PrimType::Bool) => "bool".to_string(),
        Type::Unit => "()".to_string(),
        Type::Ref { mutable, .. } => {
            if *mutable {
                "&mut _".to_string()
            } else {
                "&_".to_string()
            }
        }
        Type::Slice(_) => "[_]".to_string(),
        Type::Generic { name, .. } => format!("{name}<_>"),
        // Basis Stage 1a (`.design/basis/01-adts.md` REQ-1/REQ-2/REQ-3): a
        // descriptive label for a user `Named` type or a `Box<T>`. This fn is
        // purely a human label INSIDE an `Unsupported` diagnostic — a
        // descriptive name (NOT a panic) is the honest neutral value here; the
        // type is dead-in-1a (gated at the validator).
        Type::Named(name) => name.clone(),
        Type::Box(_) => "Box<_>".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> Program {
        let parsed = thermite_syntax::parse(src);
        assert!(parsed.is_clean(), "parse: {:?}", parsed.errors);
        parsed.program
    }

    // REQ-2 / AC-4: the slice scaffolding + the unbounded scalar are TYPE-derived
    // (a synthetic `fn f(xs: &[u32], k: u32)`), not name-derived. The same
    // `kani::any()`/`assume(len <= N)` slice scaffolding the corpus uses appears,
    // plus a bare `kani::any()` for `k` — no `if name == ...`.
    #[test]
    fn bound_is_type_derived_not_name_derived() {
        let p = parse(
            "fn f(xs: &[u32], k: u32) -> u64\n  req k < 10\n  ens result == k as u64\n  fx pure\n{\n  k as u64\n}\n",
        );
        let out = lower_l2(&p).expect("lower_l2");
        assert!(out.contains("const N: usize = 4;"), "slice bound N:\n{out}");
        assert!(
            out.contains("let xs_len: usize = kani::any();")
                && out.contains("kani::assume(xs_len <= N);")
                && out.contains("let xs_data: [u32; N] = kani::any();")
                && out.contains("let xs = &xs_data[..xs_len];"),
            "type-driven slice scaffolding:\n{out}"
        );
        assert!(
            out.contains("let k: u32 = kani::any();"),
            "type-driven unbounded scalar:\n{out}"
        );
        // The `req` is assumed AFTER the symbolic construction (REQ-2).
        assert!(out.contains("kani::assume(k < 10);"), "req assumed:\n{out}");
    }

    // REQ-3: a `while`-over-slice fn gets unwind N+1; an unconditional `loop` fn
    // gets N+2. SHAPE-keyed on the loop kind. Grounded: sum→5, binary_search→6.
    #[test]
    fn unwind_bound_is_shape_keyed() {
        let while_fn = parse(
            "fn g(xs: &[u32]) -> u64\n  req true\n  ens result >= 0\n  fx pure\n{\n  let mut a: u64 = 0;\n  let mut i: usize = 0;\n  while i < xs.len()\n    inv i <= xs.len()\n    dec xs.len() - i\n  {\n    a = a + xs[i] as u64;\n    i = i + 1;\n  }\n  a\n}\n",
        );
        if let Item::Fn(f) = &while_fn.items[0] {
            if let Some(body) = f.body.as_ref() {
                assert_eq!(unwind_bound(body), SLICE_BOUND + 1, "while → N+1");
            }
        }
        let loop_fn = parse(
            "fn h(xs: &[u32]) -> usize\n  req true\n  ens result <= xs.len()\n  fx pure\n{\n  let mut i: usize = 0;\n  loop\n    inv i <= xs.len()\n    dec xs.len() - i\n  {\n    if i == xs.len() { return i; }\n    i = i + 1;\n  }\n}\n",
        );
        if let Item::Fn(f) = &loop_fn.items[0] {
            if let Some(body) = f.body.as_ref() {
                assert_eq!(unwind_bound(body), SLICE_BOUND + 2, "loop → N+2");
            }
        }
    }

    // REQ-6 / AC-6: the bound string names the slice bound + unwind so the L2
    // caveat is explicit. binary_search has an unconditional loop → unwind 6.
    #[test]
    fn bound_string_states_the_caveat() {
        let p = parse(&std::fs::read_to_string(corpus("binary_search")).expect("read"));
        assert_eq!(bound_string(&p), "slice <= 4, unwind 6");
    }

    // REQ-1: `sum` lowers to a harness reusing the L1 `spec_sum` + a check-free
    // `sum` body + the `#[kani::proof] #[kani::unwind(5)]` wrapper.
    #[test]
    fn sum_harness_shape() {
        let p = parse(&std::fs::read_to_string(corpus("sum")).expect("read"));
        let out = lower_l2(&p).expect("lower_l2");
        // Reused L1 spec fn (real recursion, no Seq/@).
        assert!(
            out.contains("fn spec_sum(xs: &[u32]) -> u64 {") && out.contains("spec_sum(&xs[1..])"),
            "reused L1 spec_sum:\n{out}"
        );
        // The harness, unwind N+1 = 5 (while-over-slice).
        assert!(out.contains("#[kani::proof]"), "proof attr:\n{out}");
        assert!(out.contains("#[kani::unwind(5)]"), "unwind 5:\n{out}");
        assert!(out.contains("fn check_sum() {"), "harness name:\n{out}");
        // ens become assert!s; the body has NO thermite_check! (check-free).
        assert!(
            out.contains("assert!(result == spec_sum(xs));"),
            "ens#1 assert:\n{out}"
        );
        assert!(
            !out.contains("thermite_check!"),
            "the L2 body is check-free (Kani checks symbolically):\n{out}"
        );
    }

    // REQ-1 / AC-9: an un-lowerable parameter shape is an Err, not a panic. A
    // `&u32` (reference-to-scalar, not a slice and not a prim) has no L2 symbolic
    // inference, so it is `Unsupported`.
    #[test]
    fn unlowerable_is_err_not_panic() {
        let p =
            parse("fn f(p: &u32) -> u32\n  req true\n  ens result == 0\n  fx pure\n{\n  0\n}\n");
        let r = lower_l2(&p);
        assert!(
            matches!(r, Err(LowerError::Unsupported { .. })),
            "ref-to-scalar param is Unsupported, not a panic: {r:?}"
        );
    }

    fn corpus(name: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("conformance")
            .join(format!("{name}.th"))
    }
}
