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
//! | REQ-1 (L1 check-emission entry point) | SHIPPED | `pub fn lower_l1`; emits each `FnItem` with `req` on entry, loop `inv` per iteration, `ens` against bound `result` on exit; verified by `sum_l1_compiles_and_runs` (compile+run via `rustc`). #88 blocker 2 (ens-after-move CLASS): `lower_fn_l1` snapshots each non-Copy param an `ens` references (`type_is_non_copy_l1` + `expr_references_ident`) into a `<p>__pre` `.clone()` BEFORE the body, and lowers the `ens` against the snapshot (`rename_params_in_expr`) — so a param moved into the result and then named in `ens` (`move_left`'s `ens result.text.len() == b.text.len()`, `insert_str`'s `+ ins.len()`) no longer rustc-`E0382`s. `TString`/struct/enum all `#[derive(Clone)]`. #88 blocker 3 (empty-literal → `TString`): `lower_expr_exec`'s `Expr::StrLit` materializes an owned `TString` (the `Vec<u8>` byte-push, mirroring `lower.rs`'s L3 form, #82) NOT a Rust `&str`, so `Buffer { text: "", .. }` in struct-lit field position no longer rustc-`E0308`s. Verified: `forge/tests/editor_runs.rs` (the editor builds + runs with piped keystrokes). |
//! | REQ-2 (always-active check primitive) | SHIPPED | `emit_check_macro` writes the `thermite_check!` macro + `thermite_contract_violation` handler (NOT `debug_assert!`); `no_debug_assert_in_emission` (AC-2) + `negative_fixture_fires_violation`. |
//! | REQ-3 (combinator L1 executable forms) | SHIPPED | `emit_combinator_l1_defs` reads `thermite_spec::CombinatorSig.l1`; a combinator call lowers via `lower_expr_exec`; unit-tested by `combinator_l1_forms_run` (AC-3). |
//! | REQ-4 (`spec fn` → executable fn) | SHIPPED | `lower_spec_fn_l1`/`slice_fold_body_l1` emit the slice-length-branch recursion over `&[u32]`; verified by `sum_l1_compiles_and_runs` (AC-4: `spec_sum(&[1,2,3]) == 6` in the positive harness). |
//! | REQ-5 (`dec`/termination L1 scope) | SHIPPED | `lower_loop_l1` emits `inv` checks only, no `dec` runtime check (OQ-3); `no_syscall_sandbox_and_no_dec_guarantee` (AC-5). |
//! | REQ-6 (golden L1 contract) | SHIPPED | `tests/golden/l1/sum.l1.rs` compiles+runs; emitted output is execution-equivalent (compiles, runs, `sum(&[1,2,3])==6`, checks fire) — `sum_l1_compiles_and_runs`. |
//! | REQ-7 (`fx`/effect at L1 deferred to #21) | SHIPPED | no `fx` runtime check emitted; `no_syscall_sandbox_and_no_dec_guarantee` (AC-5) confirms no sandbox scaffolding. |
//!
//! ## Basis Stage 1c ADT arm (`.design/basis/01-adts.md`)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-8 (struct → L1 struct + always-active invariant check) | SHIPPED | `lower_struct_l1` emits a plain Rust `struct` + `fn well_formed(&self) -> bool` (the inv via `lower_inv_expr_l1`, `self.field`); `lower_fn_l1` weaves `<param>.well_formed()` (req-class) + `result.well_formed()` (ens-class) `thermite_check!`s for invariant-bearing struct params/returns — handled-or-loud at run time (§6 L1 rung). Consumer: `lower_l1`. Verified: `tests/adt_lower_conformance.rs::bank_account_l1_compiles_and_runs` (rustc compile+run, positive 150/0) + `bank_account_l1_req_check_fires` (an overflowing deposit ABORTS at the `[req]` check). |
//! | REQ-9 (enum/match/`is` → L1 enum/match/`matches!`) | SHIPPED | `lower_enum_l1` emits a plain Rust `enum`; `lower_match_exec`/`lower_pattern_exec` emit ENUM-QUALIFIED arms (`qualify_variant_path_l1`) incl. `Pattern::Struct`; `Expr::Is`→`matches!(s, Enum::Variant { .. })`. Consumer: `lower_l1`/`lower_fn_l1`. Verified: `shape_l1_compiles_and_runs` (Circle→true/Rect→false) + `shape_l1_ens_check_fires_on_a_lying_body` (a lying body ABORTS at the `[ens]` check). |
//! | REQ-10 (recursive type → L1 `Box`; deref) | SHIPPED | `lower_type` emits `Box<List>`; `Expr::Deref`→`*t`; an ADT-fold `spec fn` lowers to a real recursive Rust fn through the fallback `lower_block_inner`. Consumer: `lower_l1` (`Item::SpecFn`/`Item::Enum`). Verified by the workspace `cargo test` (the `list_sum` L1 lowering compiles in the corpus pipeline). |
//! | REQ-11 (`LowerError`/no panics) | SHIPPED | the L1 ADT arms reuse `LowerError`; no `unwrap`/`expect`/`panic!` added (the emitted program's `thermite_contract_violation` panic is the GENERATED program's intended L1 abort, not a toolchain panic). |
//!
//! ## C5/C7 build-side exec twins (`.design/basis/07-strings.md` REQ-9/REQ-13..16, `09-option-result.md` REQ-5; issue #104)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | C5/C7 contract spec-fn EXEC twins (the build/runtime mirror of the L3 spec fns) | SHIPPED | `forge build` lowers EVERY fn to L1 (always-active runtime `thermite_check!`s), so a contract NAMING a generated spec fn needs a runnable exec twin to evaluate the check. `emit_string_runtime_l1` emits the C5 twins (`occurs_at`/`contains_sub`/`count_sep`/`sep_free`, gated on `program_uses_string_search`) + the C7 twins (`is_digit`/`all_digits`/the free `parse_u64`/`parse_be`, gated on `program_uses_parse`, `parse_be` deduped against the C4 numfmt round-trip), each computing the SAME value as its spec body over the runtime `TString` (`Vec<u8>`) — NO verus proof (the L1 path is runtime-checked, not verified). `lower_expr_exec`'s `Expr::Call` arm `.clone()`s the leading string args (`string_arg_count_l1`, the by-value/`&`-snapshot ambiguity). The `Vec<String>` (`TVecTString`) exec `len() -> u64` is emitted by `emit_vec_runtime_l1` (every `TVec*` wrapper). Consumer: `lower_l1`. Verified: `forge/tests/acceptance_programs.rs::calculator_string_parse_builds_and_runs_end_to_end` (the full `calc.th` builds + RUNS, 2+3→Some(5), 100+200→Some(300)) + `parser_builds_and_runs_end_to_end` (the full `parse_lines.th` builds + RUNS, a,b,c→3 pieces); the formatter (C4) is unaffected + `forge check` is unchanged (L3). |

use std::fmt::Write as _;

use thermite_syntax::ast::{
    BinOp, Block, EnumItem, Expr, FnItem, IndexArg, Item, LoopKind, LoopNode, MatchArm, Param,
    Pattern, PrimType, Program, SlicePat, SpecFnItem, Stmt, StructItem, Type, UnaryOp,
    VariantShape,
};
use thermite_syntax::lexer::Span;

use crate::lower::{
    collect_vec_elem_types, elem_is_copy, is_vec_new, program_uses_parse,
    program_uses_string_search, tvec_name, LowerError,
};

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

    // Basis Stage 8 (`.design/basis/08-runnable-effect-link.md` REQ-1/REQ-3): the
    // BUILD-emitted crate must DEFINE `TString` whenever a `String`-typed value is
    // present, because the runnable effect-link wrappers
    // (`forge/src/effect_wrappers.rs`: `os::print`/`os::write`/`os::read_line`)
    // reference `super::TString`, and `lower_type` lowers a `String`-typed boundary
    // signature to the bare name `TString` (`Type::String => "TString"`). Unlike the
    // L3/Verus lowering (`lower.rs::emit_string_wrapper`, a `verus!` form over
    // `vstd::vec::Vec<u8>`), L1 is ENTIRELY exec — so this emits a PLAIN-Rust
    // `TString` over `std::vec::Vec<u8>` (no `Seq`/`@`/`spec`/`requires`), plus
    // `use TString as String;` so the surface name `String` (e.g. `String::new()` in
    // a body) resolves to the same emitted type as a `String`-typed signature
    // (`07-strings.md` REQ-4: the surface `String` IS `TString`). Without this the
    // build crate `rustc`-fails `error[E0425]: cannot find type \`TString\``. Emitted
    // ONCE, only when the program uses `String` (the non-`String` corpus is
    // byte-unaffected, matching `lower.rs::program_uses_string`'s gate).
    out.push_str(&emit_string_runtime_l1(program));

    // Cluster C6 (`.design/basis/04-collections.md` REQ-5/REQ-8/REQ-9, issue #98):
    // the BUILD-emitted crate must DEFINE the per-element `TVec<elem>` runtime
    // wrapper whenever a `Vec<T>` is REACHABLE — `lower_type` lowers `Type::Vec` to
    // the wrapper name (`Vec<u64>` → `TVecU64`), and the surface ops (`push`/`get`/
    // `last`/`pop_last`/`insert`/`remove`/`contains`/`len`) resolve to its methods.
    // Unlike the L3/Verus lowering (`lower.rs::emit_vec_wrappers`, a `verus!` form
    // over `vstd::vec::Vec<T>` with `requires`/`ensures`/`Seq`), L1 is ENTIRELY exec
    // — so this emits PLAIN-Rust methods with the capacity/no-OOB guards as
    // always-active `thermite_check!`s (§6 L1 handled-or-loud: an over-cap push or an
    // OOB get ABORTS loudly rather than UB). A non-Copy element's `get`/`last` return
    // a BORROW `&T` (the L1 mirror of the REQ-9 borrow-`get`), so a non-Copy element
    // is never moved out of the backing run. Emitted ONCE per element type, only when
    // the program uses `Vec` (the non-`Vec` corpus is byte-unaffected, matching
    // `lower.rs::collect_vec_elem_types`'s reachability gate).
    out.push_str(&emit_vec_runtime_l1(program)?);

    // The program-wide `(variant, enum)` map (REQ-9) — drives the ENUM-QUALIFIED
    // `Enum::Variant` of an L1 `match` arm / pattern / `is` `matches!` — and the
    // invariant-bearing `struct` set (REQ-8) whose `well_formed()` check is woven
    // into a producing fn (handled-or-loud at run time, §6 L1 rung). Built once.
    let variants = variant_map(program);
    let inv_structs = invariant_struct_names(program);

    // (3) + (4) the lowered items, in source order (determinism, §5.3).
    for item in &program.items {
        let item_src = match item {
            Item::SpecFn(s) => lower_spec_fn_l1(s, &variants)?,
            // A boundary fn (ffi-boundary.md REQ-4) lowers to the L1 wrapper: a
            // `req`-check, a call to the FOREIGN target binding `result`, then the
            // `ens`-checks — the foreign body is NOT lowered/verified. An
            // in-language fn lowers with its real body.
            Item::Fn(f) if f.boundary.is_some() => lower_boundary_fn_l1(f, &variants)?,
            Item::Fn(f) => lower_fn_l1(f, &variants, &inv_structs)?,
            // Basis Stage 1c (`.design/basis/01-adts.md` REQ-8/REQ-9): a `struct`
            // lowers to a plain Rust `struct` + a `well_formed` method (the
            // always-active invariant predicate); an `enum` to a plain Rust `enum`.
            Item::Struct(s) => lower_struct_l1(s)?,
            Item::Enum(e) => lower_enum_l1(e)?,
        };
        out.push('\n');
        out.push_str(&item_src);
        out.push('\n');
    }

    Ok(out)
}

/// The fixed empty variant map for the non-ADT lowering paths (mirrors
/// `lower.rs::NO_VARIANTS`): an enum-variant pattern is qualified only when its
/// name is in the program's map, so a `Some`/`None`/binding lowers unqualified.
pub(crate) const NO_VARIANTS: &[(&str, &str)] = &[];

/// The program's `(variant_name, enum_name)` map (REQ-9). A user enum-variant
/// pattern / `is` test lowers ENUM-QUALIFIED via this map; `Some`/`None`/bindings
/// are absent and lower unqualified. Built once in `lower_l1`, threaded down.
fn variant_map(program: &Program) -> Vec<(&str, &str)> {
    program
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Enum(e) => Some(e),
            _ => None,
        })
        .flat_map(|e| {
            e.variants
                .iter()
                .map(move |v| (v.name.as_str(), e.name.as_str()))
        })
        .collect()
}

/// The program's invariant-bearing `struct` names (REQ-8): a fn taking/returning
/// one gets its `well_formed()` check woven as an always-active L1 contract check.
fn invariant_struct_names(program: &Program) -> Vec<&str> {
    program
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Struct(s) if s.inv.is_some() => Some(s.name.as_str()),
            _ => None,
        })
        .collect()
}

/// The enum name a user variant belongs to (REQ-9), or `None` if `name` is not a
/// declared user variant. Shared by the L1 pattern / `is` qualification.
fn enum_of_variant<'a>(variants: &[(&'a str, &'a str)], name: &str) -> Option<&'a str> {
    variants.iter().find(|(v, _)| *v == name).map(|(_, e)| *e)
}

/// ENUM-QUALIFY a variant path (REQ-9, the L1 mirror of `lower.rs`): a single
/// user-variant segment becomes `Enum::Variant`; an already-qualified path or a
/// built-in/unknown name is joined as-written.
fn qualify_variant_path_l1(path: &[String], variants: &[(&str, &str)]) -> String {
    if path.len() == 1 {
        if let Some(enum_name) = enum_of_variant(variants, &path[0]) {
            return format!("{enum_name}::{}", path[0]);
        }
    }
    path.join("::")
}

// ---------------------------------------------------------------------------
// REQ-8/REQ-9: ADT item lowering (struct + well_formed method, enum).
// ---------------------------------------------------------------------------

/// Lower a `StructItem` to a plain Rust `struct` plus, when it carries an `inv`
/// clause, a `well_formed(&self) -> bool` method (REQ-8) — the always-active
/// invariant predicate a producing fn checks at run time (handled-or-loud, §6 L1
/// rung). The `inv` body rewrites bare field-name paths to `self.<field>`.
fn lower_struct_l1(s: &StructItem) -> Result<String, LowerError> {
    let mut out = String::new();
    // `#[derive(Clone)]`: the L1 ens-check snapshots a non-Copy struct parameter
    // BEFORE the body consumes it (`lower_fn_l1`'s `<p>__pre` snapshot) so a field
    // moved into the result and then named in an `ens` (e.g. `move_left`'s `ens
    // result.text.len() == b.text.len()`, `b.text` moved into `Buffer { text:
    // b.text, .. }`) no longer triggers rustc `error[E0382]` (#88 blocker 2). The
    // derive is the whole CLASS — every invariant/plain struct can be a moved-then-
    // named ens param.
    out.push_str("#[derive(Clone)]\n");
    out.push_str("#[allow(dead_code)]\n");
    writeln!(out, "struct {} {{", s.name).ok();
    for field in &s.fields {
        let ty = lower_type(&field.ty)?;
        writeln!(out, "    {}: {ty},", field.name).ok();
    }
    out.push_str("}\n");
    if let Some(inv) = &s.inv {
        let field_names: Vec<&str> = s.fields.iter().map(|f| f.name.as_str()).collect();
        let body = lower_inv_expr_l1(&inv.expr, &field_names, 0, s.span)?;
        writeln!(out, "\nimpl {} {{", s.name).ok();
        writeln!(out, "    #[allow(dead_code)]").ok();
        writeln!(out, "    fn well_formed(&self) -> bool {{ {body} }}").ok();
        out.push_str("}\n");
    }
    Ok(out)
}

/// Lower an `inv` expression to the L1 `well_formed(&self)` method body (REQ-8):
/// a bare field-name path becomes `self.<field>` (mirrors `lower.rs::lower_inv_expr`,
/// exec spelling — no `Seq`/`@`).
fn lower_inv_expr_l1(
    expr: &Expr,
    field_names: &[&str],
    depth: usize,
    span: Span,
) -> Result<String, LowerError> {
    if depth >= MAX_EMIT_DEPTH {
        return Err(LowerError::TooDeep {
            limit: MAX_EMIT_DEPTH,
            span,
        });
    }
    let d = depth + 1;
    match expr {
        Expr::Path(segs) => {
            if segs.len() == 1 && field_names.contains(&segs[0].as_str()) {
                Ok(format!("self.{}", segs[0]))
            } else {
                Ok(segs.join("::"))
            }
        }
        Expr::Binary { op, lhs, rhs } => {
            let l = lower_inv_expr_l1(lhs, field_names, d, span)?;
            let r = lower_inv_expr_l1(rhs, field_names, d, span)?;
            Ok(format!("{l} {} {r}", binop(*op)))
        }
        Expr::Field { receiver, name } => {
            let r = lower_inv_expr_l1(receiver, field_names, d, span)?;
            Ok(format!("{r}.{name}"))
        }
        // A method-call receiver/args (`cursor <= text.len()`'s `text.len()`,
        // `b.text.slice(0, b.cursor)`) must have the field-path rewrite recurse
        // THROUGH the receiver AND every argument — a bare field name `text` is a
        // `self.text` in the emitted `well_formed`, so an unqualified `text.len()`
        // would emit `text.len()` (rustc E0425: cannot find value `text`). Recurse
        // the rewrite so the WHOLE call tree is qualified (the editor's struct
        // inv `cursor <= text.len()`).
        Expr::MethodCall {
            receiver,
            name,
            args,
        } => {
            let r = lower_inv_expr_l1(receiver, field_names, d, span)?;
            let mut parts = Vec::with_capacity(args.len());
            for a in args {
                parts.push(lower_inv_expr_l1(a, field_names, d, span)?);
            }
            Ok(format!("{r}.{name}({})", parts.join(", ")))
        }
        // A call's args also carry the field-path rewrite (a `spec fn`/combinator
        // call over struct fields inside an `inv` — the same CLASS as the method
        // receiver above, so the whole call family is covered, not just the one
        // triggering site).
        Expr::Call { callee, args } => {
            let c = lower_inv_expr_l1(callee, field_names, d, span)?;
            let mut parts = Vec::with_capacity(args.len());
            for a in args {
                parts.push(lower_inv_expr_l1(a, field_names, d, span)?);
            }
            Ok(format!("{c}({})", parts.join(", ")))
        }
        _ => lower_expr_exec(expr, depth, span, NO_VARIANTS),
    }
}

/// Lower an `EnumItem` to a plain Rust `enum` (REQ-9): unit/tuple/struct variants
/// in their plain Rust spelling (the L1 mirror of `lower.rs::lower_enum`).
fn lower_enum_l1(e: &EnumItem) -> Result<String, LowerError> {
    let mut out = String::new();
    // `#[derive(Clone)]` mirrors `lower_struct_l1` (the whole non-Copy-param
    // class): an enum-typed param named in an `ens` after the body moves it is
    // snapshot-cloned by `lower_fn_l1` (#88 blocker 2).
    out.push_str("#[derive(Clone)]\n");
    out.push_str("#[allow(dead_code)]\n");
    writeln!(out, "enum {} {{", e.name).ok();
    for variant in &e.variants {
        match &variant.shape {
            VariantShape::Unit => {
                writeln!(out, "    {},", variant.name).ok();
            }
            VariantShape::Tuple(tys) => {
                let mut parts = Vec::with_capacity(tys.len());
                for ty in tys {
                    parts.push(lower_type(ty)?);
                }
                writeln!(out, "    {}({}),", variant.name, parts.join(", ")).ok();
            }
            VariantShape::Struct(fields) => {
                let mut parts = Vec::with_capacity(fields.len());
                for field in fields {
                    parts.push(format!("{}: {}", field.name, lower_type(&field.ty)?));
                }
                writeln!(out, "    {} {{ {} }},", variant.name, parts.join(", ")).ok();
            }
        }
    }
    out.push_str("}\n");
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
            // Basis Stage 1a (`.design/basis/01-adts.md`): a `struct`/`enum`
            // item carries no contract clauses → references no combinator; the
            // collector's neutral value is a no-op. (Dead-in-1a: gated at the
            // validator.)
            Item::Struct(_) | Item::Enum(_) => {}
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
        // Basis Stage 1a (`.design/basis/01-adts.md`): dead-in-1a ADT
        // expressions, but the honest collector descends into their
        // sub-expressions so no referenced combinator is silently dropped.
        Expr::StructLit { fields, .. } => {
            for (_, value) in fields {
                collect_combinators_in_expr(value, span, acc);
            }
        }
        Expr::Is { scrutinee, .. } => collect_combinators_in_expr(scrutinee, span, acc),
        Expr::Deref(inner) => collect_combinators_in_expr(inner, span, acc),
        // The prefix `!` (#92): descend into the operand.
        Expr::Unary { expr, .. } => collect_combinators_in_expr(expr, span, acc),
        // A string literal is a LEAF (`.design/basis/07-strings.md` REQ-1): no
        // sub-expressions, so it references no combinator — the no-op leaf arm
        // alongside `IntLit`/`BoolLit`.
        Expr::IntLit { .. } | Expr::BoolLit(_) | Expr::Path(_) | Expr::StrLit(_) => {}
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
pub(crate) fn lower_spec_fn_l1(
    s: &SpecFnItem,
    variants: &[(&str, &str)],
) -> Result<String, LowerError> {
    let ret = lower_type(&s.ret)?;
    let mut out = String::new();
    write!(out, "fn {}(", s.name).ok();
    emit_params(&mut out, &s.params)?;
    writeln!(out, ") -> {ret} {{").ok();
    out.push_str(&lower_spec_fn_body_l1(s, &ret, variants)?);
    out.push_str("}\n");
    Ok(out)
}

/// Lower a spec-fn body. For the head-fold-sum shape, emit the slice-length
/// branch recursion (REQ-4). Otherwise lower the block directly in exec
/// position (an ADT fold `sum_list` flows here — its `match l { … }` lowers with
/// the enum-variant map for ENUM-QUALIFIED arms + `*t` Box-deref, REQ-9/REQ-10).
/// The recursion is reconstructed from the match arms' SHAPE.
fn lower_spec_fn_body_l1(
    s: &SpecFnItem,
    ret: &str,
    variants: &[(&str, &str)],
) -> Result<String, LowerError> {
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
    lower_block_inner(&s.body, 1, s.span, variants)
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
                base = lower_expr_exec(&arm.body, 0, zero_span(), NO_VARIANTS)?;
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
fn lower_fn_l1(
    f: &FnItem,
    variants: &[(&str, &str)],
    inv_structs: &[&str],
) -> Result<String, LowerError> {
    let ret = lower_type(&f.ret)?;
    let mut out = String::new();
    write!(out, "fn {}(", f.name).ok();
    emit_params(&mut out, &f.params)?;
    writeln!(out, ") -> {ret} {{").ok();

    // req on entry (REQ-1/REQ-2). Omit a literal-`true` req (the empty contract).
    let req_cond = lower_expr_exec(&f.contract.req.expr, 0, f.span, variants)?;
    if req_cond != "true" {
        out.push_str(&emit_check("req", &f.contract.req.text, &req_cond, 1));
    }
    // REQ-8 (handled-or-loud, run-time tooth): a parameter whose type is an
    // invariant-bearing `struct` gets its `well_formed()` check woven as an
    // always-active `req`-class check — the type-invariant is verified on entry.
    for p in &f.params {
        if let Type::Named(name) = &p.ty {
            if inv_structs.contains(&name.as_str()) {
                let cond = format!("{}.well_formed()", p.name);
                out.push_str(&emit_check("req", &cond, &cond, 1));
            }
        }
    }

    // #88 blocker 2 (the ens-after-move CLASS): an `ens` may name a non-Copy
    // parameter (`String`/`TString`, an invariant/plain `struct`, `Vec`/`Box`)
    // that the BODY then MOVES into the `result` — e.g. `move_left`'s
    // `ens result.text.len() == b.text.len()` where the body is
    // `Buffer { text: b.text, .. }` (`b.text` moved), or `insert_str`'s
    // `ens ... + ins.len()` where `ins` is moved into `head.concat(ins)`. Reading
    // the param in the ens AFTER the body consumed it is rustc
    // `error[E0382]: borrow of moved value`. So we SNAPSHOT each non-Copy param
    // into a `<p>__pre` CLONE on entry (before the body runs) and lower every ens
    // against the snapshot (`rename_params_in_expr`). The snapshot is taken AFTER
    // the `req`/well_formed checks (which read the live param) and BEFORE the body
    // (which may move it). A Copy param (`u64`/`bool`) is left live (no snapshot,
    // no rename) so the common arithmetic ens is byte-unchanged for the corpus.
    // Snapshot ONLY a non-Copy param that some `ens` actually references (else the
    // emitted `let <p>__pre = ..` would be an unused binding → clippy `-D warnings`
    // and a needless clone). A Copy param is never snapshot.
    let snap_params: Vec<&Param> = f
        .params
        .iter()
        .filter(|p| {
            type_is_non_copy_l1(&p.ty)
                && f.contract
                    .ens
                    .iter()
                    .any(|ens| expr_references_ident(&ens.expr, &p.name))
        })
        .collect();
    let rename: Vec<(String, String)> = snap_params
        .iter()
        .map(|p| (p.name.clone(), snapshot_name_l1(&p.name)))
        .collect();
    for p in &snap_params {
        // A deep `clone()` of the non-Copy param (the L1 `TString`/struct/enum all
        // `#[derive(Clone)]`) preserves it for the ens check after the body moves
        // the original.
        writeln!(
            out,
            "    let {} = {}.clone();",
            snapshot_name_l1(&p.name),
            p.name
        )
        .ok();
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
    out.push_str(&lower_fn_body_l1(body, f, variants, 2)?);
    writeln!(out, "    }};").ok();

    // ens on exit, in source order, against the bound `result` (REQ-1/REQ-2). A
    // reference to a snapshot non-Copy param is rewritten to its `<p>__pre` clone
    // (#88 blocker 2) so the check never borrows a value the body moved.
    for ens in &f.contract.ens {
        let expr = rename_params_in_expr(&ens.expr, &rename);
        let cond = lower_expr_exec(&expr, 0, f.span, variants)?;
        out.push_str(&emit_check("ens", &ens.text, &cond, 1));
    }
    // REQ-8 (handled-or-loud): a fn returning an invariant-bearing `struct` checks
    // `result.well_formed()` on exit — the constructed value satisfies the
    // type-invariant or the always-active check SCREAMS (the L1 mirror of the L3
    // `ensures result.well_formed()`).
    if let Type::Named(name) = &f.ret {
        if inv_structs.contains(&name.as_str()) {
            out.push_str(&emit_check(
                "ens",
                "result.well_formed()",
                "result.well_formed()",
                1,
            ));
        }
    }

    writeln!(out, "    result").ok();
    out.push_str("}\n");
    Ok(out)
}

/// The `<p>__pre` snapshot binding name for a non-Copy parameter `p` (#88
/// blocker 2). Deterministic; a single fixed suffix so the renamer and the
/// emitter agree. `__pre` cannot collide with a surface identifier (the lexer
/// rejects a leading `_`-run as an ident start in user code paths, and no
/// surface name carries this exact suffix in the corpus).
fn snapshot_name_l1(param: &str) -> String {
    format!("{param}__pre")
}

/// True iff a parameter of type `ty` is NON-Copy in the emitted L1 source — so the
/// ens-check must snapshot it before the body may move it (#88 blocker 2). The
/// owning types are the ADT/collection/text types: a `String` (`TString`
/// newtype), a `Vec`/`Box` (owning heap), and a user `Named` `struct`/`enum`. A
/// `Prim` (`u32`/`u64`/`usize`/`bool`), `Unit`, a `&[T]`/`&T` `Ref` (a Copy
/// shared borrow), a `Slice`, or a `Generic` are left LIVE (no snapshot) so the
/// common arithmetic ens (`sum`/`binary_search`) lowers byte-unchanged.
fn type_is_non_copy_l1(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Named(_) | Type::String | Type::Vec(_) | Type::Box(_)
    )
}

/// Rewrite every reference to a snapshot parameter in `expr` to its `<p>__pre`
/// clone (#88 blocker 2). `renames` maps a param name to its snapshot name. The
/// rewrite is a structural deep copy of the `ens` `Expr` that, at every `Path`
/// whose SINGLE segment is a renamed param, swaps in the snapshot segment — so a
/// `b.text.len()` (`MethodCall` over `Field` over `Path(["b"])`) becomes
/// `b__pre.text.len()` and a bare `ins` (`Path(["ins"])`) becomes `ins__pre`. The
/// bound `result` is never a param, so it is untouched. A multi-segment / non-param
/// path is left as-is. This recurses through EVERY `Expr` variant (the whole class,
/// no node left un-renamed) so an arbitrary ens shape is handled.
fn rename_params_in_expr(expr: &Expr, renames: &[(String, String)]) -> Expr {
    let rec = |e: &Expr| Box::new(rename_params_in_expr(e, renames));
    match expr {
        Expr::Path(segs) => {
            if segs.len() == 1 {
                if let Some((_, to)) = renames.iter().find(|(from, _)| from == &segs[0]) {
                    return Expr::Path(vec![to.clone()]);
                }
            }
            Expr::Path(segs.clone())
        }
        Expr::IntLit { value, raw } => Expr::IntLit {
            value: *value,
            raw: raw.clone(),
        },
        Expr::BoolLit(b) => Expr::BoolLit(*b),
        Expr::StrLit(s) => Expr::StrLit(s.clone()),
        Expr::Call { callee, args } => Expr::Call {
            callee: rec(callee),
            args: args
                .iter()
                .map(|a| rename_params_in_expr(a, renames))
                .collect(),
        },
        Expr::MethodCall {
            receiver,
            name,
            args,
        } => Expr::MethodCall {
            receiver: rec(receiver),
            name: name.clone(),
            args: args
                .iter()
                .map(|a| rename_params_in_expr(a, renames))
                .collect(),
        },
        Expr::Field { receiver, name } => Expr::Field {
            receiver: rec(receiver),
            name: name.clone(),
        },
        Expr::Closure { params, body } => Expr::Closure {
            params: params.clone(),
            body: rec(body),
        },
        Expr::Match { scrutinee, arms } => Expr::Match {
            scrutinee: rec(scrutinee),
            arms: arms
                .iter()
                .map(|arm| MatchArm {
                    pattern: arm.pattern.clone(),
                    body: rename_params_in_expr(&arm.body, renames),
                })
                .collect(),
        },
        Expr::If { cond, then, else_ } => Expr::If {
            cond: rec(cond),
            then: then.clone(),
            else_: else_.clone(),
        },
        Expr::Binary { op, lhs, rhs } => Expr::Binary {
            op: *op,
            lhs: rec(lhs),
            rhs: rec(rhs),
        },
        Expr::Index { base, index } => Expr::Index {
            base: rec(base),
            index: rename_params_in_index(index, renames),
        },
        Expr::Cast { expr, ty } => Expr::Cast {
            expr: rec(expr),
            ty: ty.clone(),
        },
        Expr::Ref { mutable, expr } => Expr::Ref {
            mutable: *mutable,
            expr: rec(expr),
        },
        Expr::StructLit { path, fields } => Expr::StructLit {
            path: path.clone(),
            fields: fields
                .iter()
                .map(|(n, v)| (n.clone(), rename_params_in_expr(v, renames)))
                .collect(),
        },
        Expr::Is { scrutinee, variant } => Expr::Is {
            scrutinee: rec(scrutinee),
            variant: variant.clone(),
        },
        Expr::Deref(inner) => Expr::Deref(rec(inner)),
        // The prefix `!` (#92): rebuild faithfully, recursing the operand.
        Expr::Unary { op, expr } => Expr::Unary {
            op: *op,
            expr: rec(expr),
        },
    }
}

/// True iff `expr` references the bare single-segment identifier `ident`
/// anywhere — used to decide whether an `ens` needs a `<p>__pre` snapshot of a
/// non-Copy param (#88 blocker 2). Recurses the whole `Expr` class; a `Path`
/// whose single segment equals `ident` is the hit.
fn expr_references_ident(expr: &Expr, ident: &str) -> bool {
    let any = |es: &[Expr]| es.iter().any(|e| expr_references_ident(e, ident));
    match expr {
        Expr::Path(segs) => segs.len() == 1 && segs[0] == ident,
        Expr::IntLit { .. } | Expr::BoolLit(_) | Expr::StrLit(_) => false,
        Expr::Call { callee, args } => expr_references_ident(callee, ident) || any(args),
        Expr::MethodCall { receiver, args, .. } => {
            expr_references_ident(receiver, ident) || any(args)
        }
        Expr::Field { receiver, .. } => expr_references_ident(receiver, ident),
        Expr::Closure { params, body } => {
            // A closure param that SHADOWS `ident` rebinds it inside the body, so
            // the body's use is not the param. Conservative: if shadowed, the outer
            // param is not referenced through this closure.
            if params.iter().any(|p| p == ident) {
                false
            } else {
                expr_references_ident(body, ident)
            }
        }
        Expr::Match { scrutinee, arms } => {
            expr_references_ident(scrutinee, ident)
                || arms.iter().any(|a| expr_references_ident(&a.body, ident))
        }
        Expr::If { cond, then, else_ } => {
            expr_references_ident(cond, ident)
                || block_references_ident(then, ident)
                || block_references_ident(else_, ident)
        }
        Expr::Binary { lhs, rhs, .. } => {
            expr_references_ident(lhs, ident) || expr_references_ident(rhs, ident)
        }
        Expr::Index { base, index } => {
            expr_references_ident(base, ident) || index_references_ident(index, ident)
        }
        Expr::Cast { expr, .. } => expr_references_ident(expr, ident),
        Expr::Ref { expr, .. } => expr_references_ident(expr, ident),
        Expr::StructLit { fields, .. } => {
            fields.iter().any(|(_, v)| expr_references_ident(v, ident))
        }
        Expr::Is { scrutinee, .. } => expr_references_ident(scrutinee, ident),
        Expr::Deref(inner) => expr_references_ident(inner, ident),
        // The prefix `!` (#92): an ident can be referenced under it (`!done`).
        Expr::Unary { expr, .. } => expr_references_ident(expr, ident),
    }
}

/// [`expr_references_ident`] for a `Block` (its statements' inits + tail).
fn block_references_ident(block: &Block, ident: &str) -> bool {
    let stmt_hit = block.stmts.iter().any(|s| match s {
        Stmt::Let { init, .. } => expr_references_ident(init, ident),
        Stmt::Assign { target, value } => {
            expr_references_ident(target, ident) || expr_references_ident(value, ident)
        }
        Stmt::Return(e) => e.as_ref().is_some_and(|e| expr_references_ident(e, ident)),
        Stmt::If { cond, then, else_ } => {
            expr_references_ident(cond, ident)
                || block_references_ident(then, ident)
                || else_
                    .as_ref()
                    .is_some_and(|b| block_references_ident(b, ident))
        }
        Stmt::Expr(e) => expr_references_ident(e, ident),
        Stmt::Loop(l) => block_references_ident(&l.body, ident),
        // break/continue carry no sub-expression (#93): reference nothing.
        Stmt::Break | Stmt::Continue => false,
    });
    stmt_hit
        || block
            .tail
            .as_ref()
            .is_some_and(|t| expr_references_ident(t, ident))
}

/// [`expr_references_ident`] for an `IndexArg`.
fn index_references_ident(index: &IndexArg, ident: &str) -> bool {
    match index {
        IndexArg::Single(i) | IndexArg::RangeTo(i) | IndexArg::RangeFrom(i) => {
            expr_references_ident(i, ident)
        }
        IndexArg::Range(i, j) => expr_references_ident(i, ident) || expr_references_ident(j, ident),
    }
}

/// Rename snapshot params inside an `IndexArg` (the index sub-expressions), the
/// `Index` companion to [`rename_params_in_expr`] (#88 blocker 2).
fn rename_params_in_index(index: &IndexArg, renames: &[(String, String)]) -> IndexArg {
    match index {
        IndexArg::Single(i) => IndexArg::Single(Box::new(rename_params_in_expr(i, renames))),
        IndexArg::RangeTo(i) => IndexArg::RangeTo(Box::new(rename_params_in_expr(i, renames))),
        IndexArg::RangeFrom(i) => IndexArg::RangeFrom(Box::new(rename_params_in_expr(i, renames))),
        IndexArg::Range(i, j) => IndexArg::Range(
            Box::new(rename_params_in_expr(i, renames)),
            Box::new(rename_params_in_expr(j, renames)),
        ),
    }
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
fn lower_boundary_fn_l1(f: &FnItem, variants: &[(&str, &str)]) -> Result<String, LowerError> {
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
    let req_cond = lower_expr_exec(&f.contract.req.expr, 0, f.span, variants)?;
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
        let cond = lower_expr_exec(&ens.expr, 0, f.span, variants)?;
        out.push_str(&emit_check("ens", &ens.text, &cond, 1));
    }

    writeln!(out, "    result").ok();
    out.push_str("}\n");
    Ok(out)
}

/// Lower a `fn` body block, threading the loop `inv`-check injection. The body's
/// statements are emitted, then its tail expression (the block's value) — both
/// inside the `let result = { .. }` binder the caller opened. The variant map
/// flows into the exec lowering so an enum `match` (`is_circle`'s body) is
/// ENUM-QUALIFIED (REQ-9).
fn lower_fn_body_l1(
    block: &Block,
    f: &FnItem,
    variants: &[(&str, &str)],
    indent: usize,
) -> Result<String, LowerError> {
    let pad = "    ".repeat(indent);
    let mut out = String::new();
    for stmt in &block.stmts {
        match stmt {
            Stmt::Loop(l) => out.push_str(&lower_loop_l1(l, variants, indent)?),
            other => out.push_str(&lower_stmt_l1(other, indent, variants)?),
        }
    }
    if let Some(tail) = &block.tail {
        let t = lower_expr_exec(tail, 0, f.span, variants)?;
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
fn lower_loop_l1(
    l: &LoopNode,
    variants: &[(&str, &str)],
    indent: usize,
) -> Result<String, LowerError> {
    let pad = "    ".repeat(indent);
    let ipad = "    ".repeat(indent + 1);
    let mut out = String::new();
    match &l.kind {
        LoopKind::Loop => writeln!(out, "{pad}loop {{").ok(),
        LoopKind::While(c) => {
            let cs = lower_expr_exec(c, 0, zero_span(), variants)?;
            writeln!(out, "{pad}while {cs} {{").ok()
        }
    };
    // inv checks at the top of each iteration (REQ-1). NO dec check (REQ-5).
    for inv in &l.invs {
        let cond = lower_expr_exec(&inv.expr, 0, zero_span(), variants)?;
        out.push_str(&emit_check("inv", &inv.text, &cond, indent + 1));
    }
    // Loop body statements (a nested loop recurses through `lower_loop_l1`).
    for stmt in &l.body.stmts {
        match stmt {
            Stmt::Loop(inner) => out.push_str(&lower_loop_l1(inner, variants, indent + 1)?),
            other => out.push_str(&lower_stmt_l1(other, indent + 1, variants)?),
        }
    }
    if let Some(tail) = &l.body.tail {
        let t = lower_expr_exec(tail, 0, zero_span(), variants)?;
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
fn lower_block_inner(
    block: &Block,
    indent: usize,
    span: Span,
    variants: &[(&str, &str)],
) -> Result<String, LowerError> {
    let pad = "    ".repeat(indent);
    let mut out = String::new();
    for stmt in &block.stmts {
        match stmt {
            Stmt::Loop(l) => out.push_str(&lower_loop_l1(l, variants, indent)?),
            other => out.push_str(&lower_stmt_l1(other, indent, variants)?),
        }
    }
    if let Some(tail) = &block.tail {
        let t = lower_expr_exec(tail, 0, span, variants)?;
        writeln!(out, "{pad}{t}").ok();
    }
    Ok(out)
}

/// Lower a single statement in exec position. `variants` flows to the expression
/// lowering for ENUM-QUALIFIED match/struct/`is` forms (REQ-9); it is the last
/// parameter so `l2.rs`'s reuse passes `NO_VARIANTS` explicitly.
pub(crate) fn lower_stmt_l1(
    stmt: &Stmt,
    indent: usize,
    variants: &[(&str, &str)],
) -> Result<String, LowerError> {
    let pad = "    ".repeat(indent);
    match stmt {
        Stmt::Let {
            mutable,
            name,
            ty,
            init,
        } => {
            let kw = if *mutable { "let mut" } else { "let" };
            // Cluster C6 (`.design/basis/04-collections.md` REQ-5/REQ-11): the L1
            // mirror of the L3 `Vec::new()`-init rewrite. A `Vec`-typed `let` whose
            // init is the no-param constructor `Vec::new()` lowers the init to the
            // wrapper constructor `<TVec>::new()` (`emit_vec_runtime_l1` emits a
            // `fn new()`), because the bounded-`Vec` wrapper is a newtype — a bare
            // `Vec::new()` cannot inhabit the `TVec` type (rustc `E0308`). The element
            // type comes from the `let`'s `Type::Vec(elem)` annotation. Keyed on a
            // `Type::Vec` annotation AND a `Vec::new()` init; any other init passes
            // through.
            let init_s = if let (Some(Type::Vec(elem)), true) = (ty, is_vec_new(init)) {
                let wname = tvec_name(elem.as_ref())?;
                format!("{wname}::new()")
            } else {
                lower_expr_exec(init, 0, zero_span(), variants)?
            };
            if let Some(t) = ty {
                let ts = lower_type(t)?;
                Ok(format!("{pad}{kw} {name}: {ts} = {init_s};\n"))
            } else {
                Ok(format!("{pad}{kw} {name} = {init_s};\n"))
            }
        }
        Stmt::Assign { target, value } => {
            let t = lower_expr_exec(target, 0, zero_span(), variants)?;
            let v = lower_expr_exec(value, 0, zero_span(), variants)?;
            Ok(format!("{pad}{t} = {v};\n"))
        }
        Stmt::Return(e) => match e {
            Some(e) => {
                let s = lower_expr_exec(e, 0, zero_span(), variants)?;
                Ok(format!("{pad}return {s};\n"))
            }
            None => Ok(format!("{pad}return;\n")),
        },
        Stmt::If { cond, then, else_ } => {
            let c = lower_expr_exec(cond, 0, zero_span(), variants)?;
            let t = lower_block_inner(then, indent + 1, zero_span(), variants)?;
            let mut out = format!("{pad}if {c} {{\n{t}{pad}}}");
            if let Some(e) = else_ {
                let es = lower_block_inner(e, indent + 1, zero_span(), variants)?;
                write!(out, " else {{\n{es}{pad}}}").ok();
            }
            out.push('\n');
            Ok(out)
        }
        Stmt::Expr(e) => {
            let s = lower_expr_exec(e, 0, zero_span(), variants)?;
            Ok(format!("{pad}{s};\n"))
        }
        // `break;` / `continue;` — the L1 runtime-check form is the same native
        // loop-control statement as L3 (#93); the L1 form has no decreases/
        // invariant to weaken (runtime checks, not a proof). Mirror of `lower.rs`.
        Stmt::Break => Ok(format!("{pad}break;\n")),
        Stmt::Continue => Ok(format!("{pad}continue;\n")),
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
pub(crate) fn lower_expr_exec(
    expr: &Expr,
    depth: usize,
    span: Span,
    variants: &[(&str, &str)],
) -> Result<String, LowerError> {
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
        // A string literal materializes an OWNED `TString` (NOT a Rust `&str`)
        // (`.design/basis/07-strings.md` REQ-1) — the L1 exec mirror of `lower.rs`'s
        // L3 `Expr::StrLit` form (#82). Without this an `Expr::StrLit("")` in a
        // struct-literal field position (`Buffer { text: "", cursor: 0 }`) emits a
        // bare `""` where a `TString` field is expected → rustc `error[E0308]:
        // mismatched types (expected `TString`, found `&str`)` (#88 blocker 3). The
        // L1 `TString` is the `Vec<u8>` newtype (`emit_string_runtime_l1`), so the
        // literal's UTF-8 bytes are pushed one-by-one into a fresh `data` vec (the
        // empty literal yields the empty `TString`). Emitted as an inline block so
        // it composes as a receiver (`"hi".len()`), exactly like the L3 form.
        Expr::StrLit(s) => {
            let mut block = String::from("({ let mut data: Vec<u8> = Vec::new();");
            for b in s.as_bytes() {
                write!(block, " data.push({b}u8);").ok();
            }
            block.push_str(" TString { data } })");
            Ok(block)
        }
        Expr::Path(segs) => {
            // Cluster C4 (`.design/basis/07-strings.md` REQ-7, issue #94): the L1
            // mirror of `lower.rs`'s `String::`→`TString::` rewrite — a
            // `String::from_byte(b)` associated call names the surface `String`,
            // which the L1 runtime emits as `TString` (`emit_string_runtime_l1`).
            // Without the rewrite the build crate resolves `String` to the
            // `use TString as String;` alias for a TYPE, but an associated-fn PATH
            // `String::from_byte` needs the wrapper name; rewriting the leading
            // segment keeps it on the emitted `TString` impl.
            if segs.len() >= 2 && segs[0] == "String" {
                let mut out = String::from("TString");
                for seg in &segs[1..] {
                    out.push_str("::");
                    out.push_str(seg);
                }
                return Ok(out);
            }
            Ok(segs.join("::"))
        }
        Expr::Call { callee, args } => {
            let c = lower_expr_exec(callee, d, span, variants)?;
            // Cluster C4 (`.design/basis/07-strings.md` REQ-8, issue #94): the L1
            // runnable `parse_be`/`parse_le` (emitted by `emit_string_runtime_l1`) take
            // their byte sequence by REFERENCE (`&TString`) so the ens check does not
            // MOVE the bound `result` it also returns. Borrow the sole arg of a
            // `parse_be`/`parse_le` call (the round-trip ens `parse_be(result) == n`).
            // Keyed on the generated callee name; an arg already written `&x` is
            // left as-is.
            let borrow_arg = matches!(callee.as_ref(), Expr::Path(segs)
                if segs.len() == 1 && (segs[0] == "parse_be" || segs[0] == "parse_le"));
            // Cluster C5/C7 (`.design/basis/07-strings.md` REQ-13..16 / REQ-9, issue
            // #104): the L1 exec twins of the C5/C7 CONTRACT spec fns (`all_digits` /
            // `is_digit` / `parse_u64` / `count_sep` / `sep_free` / `contains_sub` /
            // `occurs_at`, emitted by `emit_string_runtime_l1`) take their `TString`
            // byte-sequence argument(s) BY VALUE. A contract names them over a
            // `String`-typed param that may be a by-value snapshot (`s__pre`, a
            // `TString` value) OR a `&String` reference param (`s`, a `&TString`),
            // and the two surface shapes lower to textually-indistinguishable
            // identifiers — so the call uniformly `.clone()`s the leading string
            // argument(s) (a deep `Vec<u8>` copy; `(&TString).clone()` and
            // `TString.clone()` both yield an owned `TString`), which is always valid
            // and never moves the source. The trailing scalar args (the `sep` /
            // offset byte, a surface `u64`) pass through unchanged. The COUNT of
            // leading string args per twin is fixed by its spec signature
            // (`lower.rs::emit_string_search_defs`/`emit_parse_defs`).
            let string_args = match callee.as_ref() {
                Expr::Path(segs) if segs.len() == 1 => string_arg_count_l1(&segs[0]),
                _ => 0,
            };
            let mut parts = Vec::new();
            for (idx, a) in args.iter().enumerate() {
                let lowered = lower_expr_exec(a, d, span, variants)?;
                if idx < string_args {
                    parts.push(format!("{lowered}.clone()"));
                } else if borrow_arg && !lowered.starts_with('&') {
                    parts.push(format!("&{lowered}"));
                } else {
                    parts.push(lowered);
                }
            }
            Ok(format!("{c}({})", parts.join(", ")))
        }
        Expr::MethodCall {
            receiver,
            name,
            args,
        } => {
            let r = lower_expr_exec(receiver, d, span, variants)?;
            // Cluster C4 (`.design/basis/07-strings.md` REQ-8, issue #94): the
            // `u64`→decimal-`String` method `n.to_string()` lowers to a CALL of the
            // generated free fn `u64_to_string(n)` (emitted by
            // `emit_string_runtime_l1` when the program uses `to_string`). The L1
            // mirror of the L3 `lower.rs` rewrite — the surface method spelling, the
            // free-fn call the runnable form. The generated fn RUNS the digit loop +
            // reverses to the MSB-first decimal.
            if name == "to_string" && args.is_empty() {
                return Ok(format!("u64_to_string({r})"));
            }
            // Basis Stage 7 (`.design/basis/07-strings.md` REQ-4): the emitted L1
            // `String` wrapper's index accessors `byte_at(i: usize)` /
            // `slice(lo: usize, hi: usize)` (see `emit_string_runtime_l1`) take
            // `usize`, but a Thermite surface index is commonly a `u64` (the
            // editor's `b.text.slice(0, b.cursor)` with `cursor: u64`). Rust does
            // NO implicit `u64 -> usize` narrowing, so each index arg is coerced
            // with an explicit `as usize` (rustc E0308 otherwise). This MIRRORS the
            // CHECK-path coercion in `lower.rs::lower_expr` (#86): keyed on the
            // reserved built-in method NAME (`byte_at`/`slice` — no user methods in
            // v0.1) across BOTH string index intrinsics (the whole op family, no
            // sibling left to re-pin). An integer LITERAL flows in directly (Rust
            // coerces a literal to `usize`) and an arg already `as usize` is left
            // as-is (no double-cast). L1 is entirely exec, so no spec guard needed.
            let coerce_usize = matches!(name.as_str(), "byte_at" | "slice");
            // Cluster C5 (`.design/basis/07-strings.md` REQ-15, #102): the L1
            // `split(sep: u8)` mirror takes a `u8` separator; the surface `sep` is a
            // `u64` (the `byte_at -> u64` convention). Rust does no implicit `u64 ->
            // u8` narrowing, so coerce the `split` arg `as u8` (the L1 mirror of the
            // L3 call-site coercion in `lower.rs`). A literal / already-`as u8` arg
            // passes through.
            let coerce_u8 = name == "split";
            let mut parts = Vec::with_capacity(args.len());
            for a in args {
                let lowered = lower_expr_exec(a, d, span, variants)?;
                if coerce_usize && !matches!(a, Expr::IntLit { .. }) && !is_usize_cast_l1(a) {
                    parts.push(format!("{lowered} as usize"));
                } else if coerce_u8
                    && !matches!(a, Expr::IntLit { .. })
                    && !lowered.ends_with("as u8")
                {
                    parts.push(format!("{lowered} as u8"));
                } else {
                    parts.push(lowered);
                }
            }
            Ok(format!("{r}.{name}({})", parts.join(", ")))
        }
        Expr::Field { receiver, name } => {
            let r = lower_expr_exec(receiver, d, span, variants)?;
            Ok(format!("{r}.{name}"))
        }
        Expr::Closure { params, body } => {
            // A real Rust closure (REQ-3); the corpus closures are `u32`-typed
            // slice-element predicates (matching the registry `l1` `impl Fn(u32)
            // -> bool` parameter).
            let b = lower_expr_exec(body, d, span, variants)?;
            let ps: Vec<String> = params.iter().map(|p| format!("{p}: u32")).collect();
            Ok(format!("|{}| {b}", ps.join(", ")))
        }
        Expr::Match { scrutinee, arms } => lower_match_exec(scrutinee, arms, d, span, variants),
        Expr::If { cond, then, else_ } => {
            let c = lower_expr_exec(cond, d, span, variants)?;
            let t = lower_block_inner(then, 0, span, variants)?;
            let e = lower_block_inner(else_, 0, span, variants)?;
            Ok(format!("if {c} {{ {} }} else {{ {} }}", t.trim(), e.trim()))
        }
        Expr::Binary { op, lhs, rhs } => {
            let l = lower_binary_operand(lhs, *op, true, d, span, variants)?;
            let r = lower_binary_operand(rhs, *op, false, d, span, variants)?;
            Ok(format!("{l} {} {r}", binop(*op)))
        }
        Expr::Unary { op, expr: inner } => {
            // The prefix `!` (#92, L1 exec mirror of `lower.rs`): Rust's `!` is
            // type-directed (logical-not on `bool`, bitwise-not on an integer),
            // exactly like Verus's, so the bare `!` lowers per the operand type. A
            // binary operand is parenthesized so the prefix binds only the operand.
            let UnaryOp::Not = op;
            let inner_src = lower_expr_exec(inner, d, span, variants)?;
            if matches!(inner.as_ref(), Expr::Binary { .. }) {
                Ok(format!("!({inner_src})"))
            } else {
                Ok(format!("!{inner_src}"))
            }
        }
        Expr::Index { base, index } => lower_index_exec(base, index, d, span, variants),
        Expr::Cast { expr, ty } => {
            let e = lower_expr_exec(expr, d, span, variants)?;
            let t = lower_type(ty)?;
            Ok(format!("{e} as {t}"))
        }
        Expr::Ref { mutable, expr } => {
            let e = lower_expr_exec(expr, d, span, variants)?;
            if *mutable {
                Ok(format!("&mut {e}"))
            } else {
                Ok(format!("&{e}"))
            }
        }
        // Basis Stage 1c (`.design/basis/01-adts.md` REQ-8/REQ-9/REQ-10).
        Expr::StructLit { path, fields } => {
            // A struct / struct-variant construction (REQ-2/REQ-8): the struct
            // name (or ENUM-QUALIFIED variant) + `field: value` initializers (the
            // `Account { balance: a.balance + amount }` of `deposit`).
            let head = qualify_variant_path_l1(path, variants);
            let mut parts = Vec::with_capacity(fields.len());
            for (name, value) in fields {
                let v = lower_expr_exec(value, d, span, variants)?;
                parts.push(format!("{name}: {v}"));
            }
            Ok(format!("{head} {{ {} }}", parts.join(", ")))
        }
        Expr::Is { scrutinee, variant } => {
            // A variant-discrimination test `SCRUTINEE is Variant` (REQ-6/REQ-9):
            // at L1 there is no Verus `is`, so it is the always-true-or-false Rust
            // `matches!(<scrutinee>, Enum::Variant { .. })` (the `{ .. }` form
            // works for unit/tuple/struct variants). The enum is resolved from the
            // variant map so the discriminant test is ENUM-QUALIFIED.
            let s = lower_expr_exec(scrutinee, d, span, variants)?;
            let v = variant.last().cloned().unwrap_or_default();
            let qualified = qualify_variant_path_l1(variant, variants);
            // If the variant is not in the map (an unknown/qualified name), fall
            // back to the bare variant; otherwise use the qualified path.
            let pat_path = if qualified == v && variant.len() == 1 {
                v.clone()
            } else {
                qualified
            };
            Ok(format!("matches!({s}, {pat_path} {{ .. }})"))
        }
        Expr::Deref(inner) => {
            // A `Box` dereference `*EXPR` (REQ-3/REQ-10): the recursive read
            // `*tail`. At L1 `Box<T>` is a real heap box; `*t` derefs it (moving
            // the owned `T` out — the recursive `sum_list(*t)` consumes the box).
            let e = lower_expr_exec(inner, d, span, variants)?;
            Ok(format!("*{e}"))
        }
    }
}

/// True if `expr` is already an `as usize` cast — so the L1 `byte_at`/`slice`
/// `u64 -> usize` index coercion (mirroring `lower.rs::is_usize_cast`, #86) does
/// NOT double-cast an argument the source already wrote `... as usize`.
fn is_usize_cast_l1(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Cast {
            ty: Type::Prim(PrimType::Usize),
            ..
        }
    )
}

/// Lower an `Index` expression in exec position (plain Rust): `xs[i]`,
/// `&xs[..i]` (as `xs[..i]` since a `Ref` wraps it), `xs[i..]`, `xs[i..j]`.
fn lower_index_exec(
    base: &Expr,
    index: &IndexArg,
    depth: usize,
    span: Span,
    variants: &[(&str, &str)],
) -> Result<String, LowerError> {
    let b = lower_expr_exec(base, depth, span, variants)?;
    match index {
        IndexArg::Single(i) => {
            let idx = lower_expr_exec(i, depth, span, variants)?;
            Ok(format!("{b}[{idx}]"))
        }
        IndexArg::RangeTo(i) => {
            let idx = lower_expr_exec(i, depth, span, variants)?;
            Ok(format!("{b}[..{idx}]"))
        }
        IndexArg::RangeFrom(i) => {
            let idx = lower_expr_exec(i, depth, span, variants)?;
            Ok(format!("{b}[{idx}..]"))
        }
        IndexArg::Range(i, j) => {
            let lo = lower_expr_exec(i, depth, span, variants)?;
            let hi = lower_expr_exec(j, depth, span, variants)?;
            Ok(format!("{b}[{lo}..{hi}]"))
        }
    }
}

/// Lower a `match` in exec position (e.g. `binary_search`'s `Option` ens match,
/// `is_circle`'s enum match, `sum_list`'s ADT fold). Arm patterns are
/// ENUM-QUALIFIED via the variant map (REQ-9).
fn lower_match_exec(
    scrutinee: &Expr,
    arms: &[MatchArm],
    depth: usize,
    span: Span,
    variants: &[(&str, &str)],
) -> Result<String, LowerError> {
    let s = lower_expr_exec(scrutinee, depth, span, variants)?;
    let mut out = format!("match {s} {{ ");
    for arm in arms {
        let pat = lower_pattern_exec(&arm.pattern, depth, span, variants)?;
        let body = lower_expr_exec(&arm.body, depth, span, variants)?;
        write!(out, "{pat} => {body}, ").ok();
    }
    out.push('}');
    Ok(out)
}

/// Lower a pattern in exec position (REQ-7/REQ-9). A user enum-variant pattern is
/// ENUM-QUALIFIED via the variant map (`Circle(r)`→`Shape::Circle(r)`); `Some`/
/// `None`/bindings lower unqualified. `Pattern::Struct` (`Rect { w, h }`) is the
/// struct-variant destructuring (REQ-4/REQ-9). A slice pattern outside a head-fold
/// spec fn is unsupported at L1 (mirrors `lower.rs`).
fn lower_pattern_exec(
    pat: &Pattern,
    depth: usize,
    span: Span,
    variants: &[(&str, &str)],
) -> Result<String, LowerError> {
    if depth >= MAX_EMIT_DEPTH {
        return Err(LowerError::TooDeep {
            limit: MAX_EMIT_DEPTH,
            span,
        });
    }
    match pat {
        Pattern::Wildcard => Ok("_".to_string()),
        Pattern::Binding(name) => Ok(name.clone()),
        Pattern::Literal(e) => lower_expr_exec(e, depth + 1, span, variants),
        Pattern::Enum { path, fields } => {
            let head = qualify_variant_path_l1(path, variants);
            if fields.is_empty() {
                Ok(head)
            } else {
                let mut fs = Vec::new();
                for f in fields {
                    fs.push(lower_pattern_exec(f, depth + 1, span, variants)?);
                }
                Ok(format!("{head}({})", fs.join(", ")))
            }
        }
        Pattern::Struct { path, fields, rest } => {
            let head = qualify_variant_path_l1(path, variants);
            let mut parts = Vec::with_capacity(fields.len());
            for (name, subpat) in fields {
                let sub = lower_pattern_exec(subpat, depth + 1, span, variants)?;
                if matches!(subpat, Pattern::Binding(b) if b == name) {
                    parts.push(name.clone());
                } else {
                    parts.push(format!("{name}: {sub}"));
                }
            }
            if *rest {
                parts.push("..".to_string());
            }
            if parts.is_empty() {
                Ok(format!("{head} {{}}"))
            } else {
                Ok(format!("{head} {{ {} }}", parts.join(", ")))
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
    variants: &[(&str, &str)],
) -> Result<String, LowerError> {
    let s = lower_expr_exec(operand, depth, span, variants)?;
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
        // #92 integer operators (the L1 exec mirror of `lower.rs::binop`).
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

/// Binding-power tier of a binary operator (higher binds tighter). Mirrors
/// `lower.rs::precedence` (the pinned standard-Rust precedence, #92).
fn precedence(op: BinOp) -> u8 {
    match op {
        BinOp::Or => 1,
        BinOp::And => 2,
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => 3,
        BinOp::BitOr => 4,
        BinOp::BitXor => 5,
        BinOp::BitAnd => 6,
        BinOp::Shl | BinOp::Shr => 7,
        BinOp::Add | BinOp::Sub => 8,
        BinOp::Mul | BinOp::Div | BinOp::Rem => 9,
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
        // Basis Stage 1c (`.design/basis/01-adts.md` REQ-1/REQ-2/REQ-10): a user
        // `struct`/`enum` type is its bare name; `Box<T>` is a real heap box (the
        // L1 mirror of `lower.rs::lower_type` — plain Rust, no `Seq`).
        Type::Named(name) => Ok(name.clone()),
        Type::Box(inner) => {
            let i = lower_type(inner)?;
            Ok(format!("Box<{i}>"))
        }
        // Basis Stage 4 / Cluster C6 (`.design/basis/04-collections.md`
        // REQ-5/REQ-8/REQ-9, issue #98): the L1 exec mirror of a bounded `Vec<T>`
        // is the per-element runtime wrapper `TVec<elem>` (`Vec<u64>` → `TVecU64`,
        // `Vec<String>` → `TVecTString`, `Vec<Point>` → `TVecPoint`, nested
        // `Vec<Vec<u64>>` → `TVecTVecU64`), the SAME name as the L3 lowering
        // (`lower.rs::tvec_name`). The wrapper is DEFINED by `emit_vec_runtime_l1`
        // with the surface ops (`push`/`get`/`last`/`pop_last`/`insert`/`remove`/
        // `contains`/`len`) as plain-Rust methods carrying the capacity/no-OOB
        // guards as always-active `thermite_check!`s (§6 L1 handled-or-loud). The
        // earlier bare `Vec<T>` form could not back the value-`get`/`pop_last`/
        // borrow-`get` surface ops (native `Vec::get` returns `Option`); the wrapper
        // makes the whole op family runnable.
        Type::Vec(inner) => tvec_name(inner),
        // Cluster C7 (`.design/basis/09-option-result.md` REQ-4): the L1 exec mirror
        // of the built-in `Option<T>` / `Result<T, E>` is the Verus-/Rust-NATIVE
        // generic — Rust's prelude carries them and their constructors
        // `Some`/`None`/`Ok`/`Err`, so the L1 runnable form is the bare native type
        // (no wrapper), exactly as the L3 lowering (`lower.rs::lower_type`). The
        // element/error types lower recursively. This keeps L1 total over `Type`
        // (no panic, REQ-6 / REQ-5).
        Type::Option(inner) => {
            let i = lower_type(inner)?;
            Ok(format!("Option<{i}>"))
        }
        Type::Result(ok, err) => {
            let o = lower_type(ok)?;
            let e = lower_type(err)?;
            Ok(format!("Result<{o}, {e}>"))
        }
        // Basis Stage 7 (`.design/basis/07-strings.md` REQ-4): the bounded owned
        // text primitive lowers to the newtype `TString` over `vstd::vec::Vec<u8>`
        // (the byte char model). The L1 exec mirror is the SAME wrapper name as the
        // L3 lowering (`lower.rs::lower_type` -> `"TString"`), so its
        // `len`/`byte_at`/`concat` method calls resolve to the emitted `TString`
        // ops. The v1 corpus exercises L3 only; this arm keeps L1 total over `Type`
        // (no panic, REQ-7 / REQ-5).
        Type::String => Ok("TString".to_string()),
    }
}

// ---------------------------------------------------------------------------
// Basis Stage 8 (`.design/basis/08-runnable-effect-link.md` REQ-1/REQ-3): the
// plain-Rust `TString` definition the BUILD-emitted crate needs when a
// `String`-typed value is present (so the `os::<name>` Write/Read-line wrappers'
// `super::TString` and `lower_type`'s bare `TString` boundary signatures resolve).
// ---------------------------------------------------------------------------

/// The bounded-string capacity (`.design/basis/07-strings.md` §4.2 cage; the
/// `lower.rs` L3 `VEC_CAP` idiom `1_000_000`). At L1 (runtime checks, not an SMT
/// proof) the bound is enforced by an always-active `thermite_check!` rather than a
/// `requires`/`invariant`.
const STRING_CAP_L1: usize = 1_000_000;

/// The number of LEADING `TString`-typed arguments a C5/C7 contract spec-fn twin
/// takes (issue #104), used by `lower_expr_exec`'s `Expr::Call` arm to `.clone()`
/// exactly those arguments at a call site (the by-value/`&` snapshot-shape
/// ambiguity, see that arm). The counts mirror the spec signatures in
/// `lower.rs::emit_string_search_defs`/`emit_parse_defs`: `all_digits(s)` /
/// `parse_u64(s)` / `count_sep(s, sep)` / `sep_free(s, sep)` take one leading
/// string; `contains_sub(s, needle)` / `occurs_at(s, needle, at)` take two;
/// `is_digit(b)` takes none (a `u8`/`u64` byte). A name not in this set is not a
/// string-arg twin (0). `parse_be`/`parse_le` are EXCLUDED — they keep the C4
/// `&TString` borrow form (the `borrow_arg` gate), not the clone form.
fn string_arg_count_l1(name: &str) -> usize {
    match name {
        "all_digits" | "parse_u64" | "count_sep" | "sep_free" => 1,
        "contains_sub" | "occurs_at" => 2,
        _ => 0,
    }
}

/// True iff the program references the `String` type in any `fn`/`spec fn`
/// parameter or return position, OR materializes a string literal anywhere — both
/// require the build-crate `TString` definition (a literal lowers to a constructed
/// `TString`, a `String`-typed boundary signature lowers to `TString`). Mirrors
/// `lower.rs::program_uses_string`'s gate so the emission is byte-stable for the
/// non-`String` corpus (no `TString` emitted when nothing uses it).
fn program_uses_string_l1(program: &Program) -> bool {
    fn ty_is_string(ty: &Type) -> bool {
        match ty {
            Type::String => true,
            Type::Ref { inner, .. } => ty_is_string(inner),
            Type::Slice(inner) | Type::Box(inner) | Type::Vec(inner) => ty_is_string(inner),
            Type::Generic { arg, .. } => ty_is_string(arg),
            // Cluster C7 (`.design/basis/09-option-result.md` REQ-4): a `String`
            // nested under an `Option<String>` / `Result<String, E>` is reached
            // through the type argument(s), exactly as a `Box`/`Vec` inner is.
            Type::Option(inner) => ty_is_string(inner),
            Type::Result(ok, err) => ty_is_string(ok) || ty_is_string(err),
            Type::Prim(_) | Type::Unit | Type::Named(_) => false,
        }
    }
    for item in &program.items {
        let (params, ret, body): (&[Param], &Type, Option<&Block>) = match item {
            Item::Fn(f) => (&f.params, &f.ret, f.body.as_ref()),
            Item::SpecFn(s) => (&s.params, &s.ret, Some(&s.body)),
            Item::Struct(_) | Item::Enum(_) => continue,
        };
        if params.iter().any(|p| ty_is_string(&p.ty)) || ty_is_string(ret) {
            return true;
        }
        if let Some(b) = body {
            if block_has_str_lit_l1(b) {
                return true;
            }
        }
    }
    false
}

/// True if a block contains a string-literal expression anywhere (a literal lowers
/// to a constructed `TString`). The L1 mirror of `lower.rs::block_has_str_lit`.
fn block_has_str_lit_l1(block: &Block) -> bool {
    block.stmts.iter().any(stmt_has_str_lit_l1)
        || block
            .tail
            .as_deref()
            .map(expr_has_str_lit_l1)
            .unwrap_or(false)
}

fn stmt_has_str_lit_l1(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Let { init, .. } => expr_has_str_lit_l1(init),
        Stmt::Assign { target, value } => expr_has_str_lit_l1(target) || expr_has_str_lit_l1(value),
        Stmt::Return(opt) => opt.as_ref().map(expr_has_str_lit_l1).unwrap_or(false),
        Stmt::If {
            cond, then, else_, ..
        } => {
            expr_has_str_lit_l1(cond)
                || block_has_str_lit_l1(then)
                || else_.as_ref().map(block_has_str_lit_l1).unwrap_or(false)
        }
        Stmt::Loop(l) => block_has_str_lit_l1(&l.body),
        Stmt::Expr(e) => expr_has_str_lit_l1(e),
        // break/continue carry no sub-expression (#93): no string literal.
        Stmt::Break | Stmt::Continue => false,
    }
}

/// True if a string literal appears anywhere in `expr` (a full structural walk).
fn expr_has_str_lit_l1(expr: &Expr) -> bool {
    match expr {
        Expr::StrLit(_) => true,
        Expr::IntLit { .. } | Expr::BoolLit(_) | Expr::Path(_) => false,
        Expr::Call { callee, args } => {
            expr_has_str_lit_l1(callee) || args.iter().any(expr_has_str_lit_l1)
        }
        Expr::MethodCall { receiver, args, .. } => {
            expr_has_str_lit_l1(receiver) || args.iter().any(expr_has_str_lit_l1)
        }
        Expr::Field { receiver, .. } => expr_has_str_lit_l1(receiver),
        Expr::Closure { body, .. } => expr_has_str_lit_l1(body),
        Expr::Match { scrutinee, arms } => {
            expr_has_str_lit_l1(scrutinee) || arms.iter().any(|a| expr_has_str_lit_l1(&a.body))
        }
        Expr::If { cond, then, else_ } => {
            expr_has_str_lit_l1(cond) || block_has_str_lit_l1(then) || block_has_str_lit_l1(else_)
        }
        Expr::Binary { lhs, rhs, .. } => expr_has_str_lit_l1(lhs) || expr_has_str_lit_l1(rhs),
        Expr::Index { base, index } => {
            expr_has_str_lit_l1(base)
                || match index {
                    IndexArg::Single(e) | IndexArg::RangeTo(e) | IndexArg::RangeFrom(e) => {
                        expr_has_str_lit_l1(e)
                    }
                    IndexArg::Range(lo, hi) => expr_has_str_lit_l1(lo) || expr_has_str_lit_l1(hi),
                }
        }
        Expr::Cast { expr, .. } | Expr::Ref { expr, .. } | Expr::Deref(expr) => {
            expr_has_str_lit_l1(expr)
        }
        Expr::StructLit { fields, .. } => fields.iter().any(|(_, v)| expr_has_str_lit_l1(v)),
        Expr::Is { scrutinee, .. } => expr_has_str_lit_l1(scrutinee),
        // The prefix `!` (#92): a string literal could sit under it; descend.
        Expr::Unary { expr, .. } => expr_has_str_lit_l1(expr),
    }
}

/// Emit the PLAIN-Rust `TString` definition + the `use TString as String;` surface
/// alias the BUILD-emitted crate needs whenever the program uses `String`
/// (`.design/basis/08-runnable-effect-link.md` REQ-1/REQ-3), EMPTY otherwise (the
/// non-`String` corpus is byte-unaffected).
///
/// This is the L1 exec mirror of `lower.rs::emit_string_wrapper`'s L3/Verus form:
/// the SAME `TString { data: Vec<u8> }` shape (matching the wrappers' `super::TString
/// { data: s.into_bytes() }` construction and `&s.data` field access in
/// `forge/src/effect_wrappers.rs`) and the SAME method surface (`new`/`len`/`byte_at`/
/// `concat`/`slice`), but as ordinary runnable Rust — no `vstd`, no `Seq`/`@`, no
/// `spec`/`requires`/`ensures`/`invariant`/`decreases` (L1 is entirely exec; §6 L1
/// rung). The bounds the L3 form proves (`i < len`, `lo <= hi <= len`, `len <= CAP`)
/// are enforced at run time by the always-active `thermite_check!` (so a no-OOB /
/// over-cap violation ABORTS loudly rather than being a silent panic; the §6 L1
/// "handled-or-loud" discipline). `#[derive(Debug)]` so a `String`-returning
/// `--entry` runner's `println!("… = {r:?}")` compiles (`build.rs`
/// `synthesize_entry_main`). `#[allow(dead_code)]` because a program may name only
/// a subset of the methods (the wrapper/lowering keeps the full surface available).
///
/// The `use TString as String;` alias makes the surface name `String` (in
/// expression position, e.g. a body's `String::new()`) resolve to the SAME emitted
/// type as a `String`-typed signature lowered by `lower_type` (`07-strings.md`
/// REQ-4: the surface `String` IS the bounded `TString`, not `std::string::String`).
fn emit_string_runtime_l1(program: &Program) -> String {
    if !program_uses_string_l1(program) {
        return String::new();
    }
    let cap = STRING_CAP_L1;
    let mut out = String::new();
    out.push('\n');
    // `Clone` (alongside `Debug`): the L1 ens-check snapshots a non-Copy
    // parameter BEFORE the body consumes it (`lower_fn_l1`'s `<p>__pre` snapshot),
    // so a `String`/`TString` param named in an `ens` after being moved into the
    // result no longer triggers rustc `error[E0382]: borrow of moved value` (#88
    // blocker 2). A `TString` is a `Vec<u8>` newtype, so the derive is a deep byte
    // copy.
    out.push_str("#[derive(Debug, Clone)]\n");
    out.push_str("#[allow(dead_code)]\n");
    out.push_str("struct TString { data: Vec<u8> }\n");
    out.push_str("#[allow(dead_code)]\n");
    out.push_str("use TString as String;\n");
    out.push_str("#[allow(dead_code)]\n");
    out.push_str("impl TString {\n");
    out.push_str("    fn new() -> TString { TString { data: Vec::new() } }\n");
    out.push_str("    fn len(&self) -> u64 { self.data.len() as u64 }\n");
    // The no-OOB `byte_at` accessor (the editor's core safety): the L3 form PROVES
    // `req i < len`; L1 enforces it loudly at run time (the always-active check),
    // then returns the byte zero-extended to `u64` (the corpus `first_byte -> u64`).
    out.push_str("    fn byte_at(&self, i: usize) -> u64 {\n");
    writeln!(
        out,
        "        thermite_check!(\"req\", \"i < self.len()\", i < self.data.len());"
    )
    .ok();
    out.push_str("        self.data[i] as u64\n");
    out.push_str("    }\n");
    // The bounded constructing `concat` (a two-loop append). The L3 form PROVES the
    // `len_a + len_b <= CAP` cage; L1 enforces it loudly, then appends.
    out.push_str("    fn concat(&self, b: TString) -> TString {\n");
    writeln!(
        out,
        "        thermite_check!(\"req\", \"self.len() + b.len() <= CAP\", self.data.len() + b.data.len() <= {cap});"
    )
    .ok();
    out.push_str("        let mut out: Vec<u8> = Vec::new();\n");
    out.push_str("        out.extend_from_slice(&self.data);\n");
    out.push_str("        out.extend_from_slice(&b.data);\n");
    out.push_str("        TString { data: out }\n");
    out.push_str("    }\n");
    // The bounded substring `slice` (an owned byte copy). The L3 form PROVES `lo <=
    // hi <= len`; L1 enforces it loudly, then copies the run.
    out.push_str("    fn slice(&self, lo: usize, hi: usize) -> TString {\n");
    writeln!(
        out,
        "        thermite_check!(\"req\", \"lo <= hi && hi <= self.len()\", lo <= hi && hi <= self.data.len());"
    )
    .ok();
    out.push_str("        TString { data: self.data[lo..hi].to_vec() }\n");
    out.push_str("    }\n");
    // Cluster C4 (`.design/basis/07-strings.md` REQ-7, issue #94): the L1 exec
    // mirror of the verified byte-builder (`lower.rs::emit_string_wrapper`'s
    // `from_byte`/`push_byte`). `from_byte(b)` builds a 1-byte `String`; `push_byte(b)`
    // appends one byte returning a FRESH owned `String` (the owned-result form). The
    // surface byte is a `u64` (the SAME zero-extension as `byte_at -> u64`), narrowed
    // to the `u8` backing element. The L3 form PROVES `len < CAP`; L1 enforces it
    // loudly at run time (the always-active check) so an over-cap push ABORTS rather
    // than silently — §6 L1 handled-or-loud.
    out.push_str("    fn from_byte(b: u64) -> TString {\n");
    out.push_str("        let mut data: Vec<u8> = Vec::new();\n");
    out.push_str("        data.push(b as u8);\n");
    out.push_str("        TString { data }\n");
    out.push_str("    }\n");
    out.push_str("    fn push_byte(&self, b: u64) -> TString {\n");
    writeln!(
        out,
        "        thermite_check!(\"req\", \"self.len() < CAP\", self.data.len() < {cap});"
    )
    .ok();
    out.push_str("        let mut out: Vec<u8> = Vec::new();\n");
    out.push_str("        out.extend_from_slice(&self.data);\n");
    out.push_str("        out.push(b as u8);\n");
    out.push_str("        TString { data: out }\n");
    out.push_str("    }\n");
    // Cluster C5 (`.design/basis/07-strings.md` REQ-13..16, issue #102): the L1 exec
    // mirror of the verified string search/transform ops (`lower.rs::emit_string_-
    // search_methods`). Entirely runnable Rust — no `verus!`/`Seq`/`requires`; the L3
    // proof carries the contracts, L1 RUNS the same byte scans. `find` returns a
    // native `Option<u64>`; `split` returns the C6 `TVecTString` runtime wrapper
    // (woven by `emit_vec_runtime_l1` because `collect_vec_elem_types` notes the
    // `Vec<String>` element when a C5 op is used). Emitted only when the program uses
    // a C5 op (byte-stable for the non-C5 corpus). `contains` here is the SUBSTRING op
    // (the `TString` receiver); the C6 `TVec::contains` membership op is a DISTINCT
    // inherent method on the `TVec*` impl (receiver-type dispatch, no clobber).
    if program_uses_string_search(program) {
        out.push_str("    fn matches_at(&self, p: &TString, at: usize) -> bool {\n");
        out.push_str("        let mut k: usize = 0;\n");
        out.push_str("        while k < p.data.len() {\n");
        out.push_str("            if self.data[at + k] != p.data[k] { return false; }\n");
        out.push_str("            k = k + 1;\n");
        out.push_str("        }\n");
        out.push_str("        true\n");
        out.push_str("    }\n");
        out.push_str("    fn starts_with(&self, p: &TString) -> bool {\n");
        out.push_str("        if p.data.len() > self.data.len() { return false; }\n");
        out.push_str("        self.matches_at(p, 0)\n");
        out.push_str("    }\n");
        out.push_str("    fn ends_with(&self, p: &TString) -> bool {\n");
        out.push_str("        if p.data.len() > self.data.len() { return false; }\n");
        out.push_str("        self.matches_at(p, self.data.len() - p.data.len())\n");
        out.push_str("    }\n");
        out.push_str("    fn contains(&self, p: &TString) -> bool {\n");
        out.push_str("        if p.data.len() > self.data.len() { return false; }\n");
        out.push_str("        let last: usize = self.data.len() - p.data.len();\n");
        out.push_str("        let mut at: usize = 0;\n");
        out.push_str("        while at <= last {\n");
        out.push_str("            if self.matches_at(p, at) { return true; }\n");
        out.push_str("            at = at + 1;\n");
        out.push_str("        }\n");
        out.push_str("        false\n");
        out.push_str("    }\n");
        out.push_str("    fn find(&self, p: &TString) -> Option<u64> {\n");
        out.push_str("        if p.data.len() > self.data.len() { return None; }\n");
        out.push_str("        let last: usize = self.data.len() - p.data.len();\n");
        out.push_str("        let mut at: usize = 0;\n");
        out.push_str("        while at <= last {\n");
        out.push_str("            if self.matches_at(p, at) { return Some(at as u64); }\n");
        out.push_str("            at = at + 1;\n");
        out.push_str("        }\n");
        out.push_str("        None\n");
        out.push_str("    }\n");
        out.push_str("    fn split(&self, sep: u8) -> TVecTString {\n");
        out.push_str("        let mut pieces: Vec<TString> = Vec::new();\n");
        out.push_str("        let mut cur: Vec<u8> = Vec::new();\n");
        out.push_str("        let mut i: usize = 0;\n");
        out.push_str("        while i < self.data.len() {\n");
        out.push_str("            let b: u8 = self.data[i];\n");
        out.push_str("            if b == sep {\n");
        out.push_str("                pieces.push(TString { data: cur });\n");
        out.push_str("                cur = Vec::new();\n");
        out.push_str("            } else {\n");
        out.push_str("                cur.push(b);\n");
        out.push_str("            }\n");
        out.push_str("            i = i + 1;\n");
        out.push_str("        }\n");
        out.push_str("        pieces.push(TString { data: cur });\n");
        out.push_str("        TVecTString { data: pieces }\n");
        out.push_str("    }\n");
        out.push_str("    fn trim(&self) -> TString {\n");
        out.push_str("        let n: usize = self.data.len();\n");
        out.push_str("        let mut lo: usize = 0;\n");
        out.push_str(
            "        while lo < n && { let c = self.data[lo]; c == 32 || c == 9 || c == 10 || c == 13 } { lo = lo + 1; }\n",
        );
        out.push_str("        let mut hi: usize = n;\n");
        out.push_str(
            "        while hi > lo && { let c = self.data[hi - 1]; c == 32 || c == 9 || c == 10 || c == 13 } { hi = hi - 1; }\n",
        );
        out.push_str("        TString { data: self.data[lo..hi].to_vec() }\n");
        out.push_str("    }\n");
    }
    out.push_str("}\n");
    // Cluster C5 (`.design/basis/07-strings.md` REQ-13..16, issue #104): the L1 EXEC
    // twins of the C5 CONTRACT spec fns (`occurs_at`/`contains_sub`/`count_sep`/
    // `sep_free`). At L3 these are `spec fn`s carrying the PROOF; `forge build` lowers
    // EVERY fn to its always-active runtime `thermite_check!`, so a contract naming a
    // C5 spec fn (the parser's `ens result.len() == 1 + count_sep(s, sep)` /
    // `ens result == contains_sub(s, sep)`) becomes a RUN-time check that must resolve
    // the named fn as runnable Rust. Each twin computes the SAME value as its spec body
    // over the runtime `TString` (`Vec<u8>`) — the byte scans are the exec mirror of
    // the `Seq<u8>` definitions in `lower.rs::emit_string_search_defs`. They carry NO
    // verus proof (the L1 path is runtime-checked, not verified — the SPEC twins + the
    // §7 proofs already discharge the check path). String args are taken BY VALUE
    // (the call site `.clone()`s — `string_arg_count_l1`). Emitted only when the
    // program uses a C5 op (`program_uses_string_search` — byte-stable for the non-C5
    // corpus), `#[allow(dead_code)]` because a program may name only a subset.
    if program_uses_string_search(program) {
        out.push('\n');
        // occurs_at: the needle occurs at byte offset `at` (a bounded forward compare).
        // The exec mirror of the `Seq<u8>` `0 <= at && at + needle.len() <= s.len() &&
        // forall|k| s[at+k] == needle[k]`. `at` is the surface `u64` offset.
        out.push_str("#[allow(dead_code)]\n");
        out.push_str("fn occurs_at(s: TString, needle: TString, at: u64) -> bool {\n");
        out.push_str("    let at_u: usize = at as usize;\n");
        out.push_str("    if at_u + needle.data.len() > s.data.len() { return false; }\n");
        out.push_str("    let mut k: usize = 0;\n");
        out.push_str("    while k < needle.data.len() {\n");
        out.push_str("        if s.data[at_u + k] != needle.data[k] { return false; }\n");
        out.push_str("        k = k + 1;\n");
        out.push_str("    }\n");
        out.push_str("    true\n");
        out.push_str("}\n");
        // contains_sub: some offset `at` at which the needle occurs (the bounded
        // existential, scanned left-to-right). Mirror of `exists|at| occurs_at(..)`.
        out.push_str("#[allow(dead_code)]\n");
        out.push_str("fn contains_sub(s: TString, needle: TString) -> bool {\n");
        out.push_str("    if needle.data.len() > s.data.len() { return false; }\n");
        out.push_str("    let last: usize = s.data.len() - needle.data.len();\n");
        out.push_str("    let mut at: usize = 0;\n");
        out.push_str("    while at <= last {\n");
        out.push_str(
            "        if occurs_at(s.clone(), needle.clone(), at as u64) { return true; }\n",
        );
        out.push_str("        at = at + 1;\n");
        out.push_str("    }\n");
        out.push_str("    false\n");
        out.push_str("}\n");
        // count_sep: the number of bytes equal to `sep` — split's piece count is
        // `1 + count_sep`. The exec mirror of the recursive `Seq<u8>` count. The surface
        // `sep` is a `u64` (the `byte_at -> u64` convention), narrowed `as u8`.
        out.push_str("#[allow(dead_code)]\n");
        out.push_str("fn count_sep(s: TString, sep: u64) -> u64 {\n");
        out.push_str("    let sep_b: u8 = sep as u8;\n");
        out.push_str("    let mut n: u64 = 0;\n");
        out.push_str("    let mut i: usize = 0;\n");
        out.push_str("    while i < s.data.len() {\n");
        out.push_str("        if s.data[i] == sep_b { n = n + 1; }\n");
        out.push_str("        i = i + 1;\n");
        out.push_str("    }\n");
        out.push_str("    n\n");
        out.push_str("}\n");
        // sep_free: no byte equals `sep` (each split piece). Mirror of the `forall`.
        out.push_str("#[allow(dead_code)]\n");
        out.push_str("fn sep_free(s: TString, sep: u64) -> bool {\n");
        out.push_str("    let sep_b: u8 = sep as u8;\n");
        out.push_str("    let mut i: usize = 0;\n");
        out.push_str("    while i < s.data.len() {\n");
        out.push_str("        if s.data[i] == sep_b { return false; }\n");
        out.push_str("        i = i + 1;\n");
        out.push_str("    }\n");
        out.push_str("    true\n");
        out.push_str("}\n");
    }
    // Cluster C7 (`.design/basis/09-option-result.md` REQ-4 / `07-strings.md` REQ-9,
    // issue #104): the L1 EXEC twins of the C7 parse CONTRACT spec fns (`is_digit`/
    // `all_digits`/the free `parse_u64`/`parse_be`). The calculator's `add` names
    // `all_digits(a)` / `parse_be(a)` in its `req`/`ens` and calls the free
    // `parse_u64(a)` in its body — all become RUN-time `thermite_check!`/exec calls
    // under `forge build`, so each needs a runnable form computing the SAME value as
    // its spec body over the runtime `TString`. `parse_be` is SHARED with the numfmt
    // round-trip (`program_uses_numfmt_l1`); emit it HERE only when numfmt did not
    // (dedup — a program using BOTH `to_string` and `parse_u64`/`parse_be` must not
    // define `parse_be` twice). `parse_be` keeps the C4 `&TString` borrow form (the
    // round-trip ens borrows `result`); the C7 string-arg twins take `TString` by
    // value (the call site `.clone()`s). Emitted only when the program uses a parse op
    // (`program_uses_parse` — byte-stable otherwise), `#[allow(dead_code)]`.
    if program_uses_parse(program) {
        out.push('\n');
        // is_digit: the ASCII decimal-digit predicate (the surface byte is a `u64`).
        out.push_str("#[allow(dead_code)]\n");
        out.push_str("fn is_digit(b: u64) -> bool { 48 <= b && b <= 57 }\n");
        // all_digits: every byte is a decimal digit (the `forall` witness).
        out.push_str("#[allow(dead_code)]\n");
        out.push_str("fn all_digits(s: TString) -> bool {\n");
        out.push_str("    let mut i: usize = 0;\n");
        out.push_str("    while i < s.data.len() {\n");
        out.push_str("        let b: u8 = s.data[i];\n");
        out.push_str("        if b < 48 || b > 57 { return false; }\n");
        out.push_str("        i = i + 1;\n");
        out.push_str("    }\n");
        out.push_str("    true\n");
        out.push_str("}\n");
        // parse_be — the MSB-first decimal value (a left-to-right Horner accumulate),
        // SHARED with the numfmt round-trip. Emit only when numfmt did not (dedup),
        // identical bytes to the numfmt form so L1 stays byte-stable either way.
        if !program_uses_numfmt_l1(program) {
            out.push_str("#[allow(dead_code)]\n");
            out.push_str("fn parse_be(s: &TString) -> u64 {\n");
            out.push_str("    let mut acc: u64 = 0;\n");
            out.push_str("    let mut i: usize = 0;\n");
            out.push_str("    while i < s.data.len() {\n");
            out.push_str("        acc = acc * 10 + (s.data[i] as u64 - 48);\n");
            out.push_str("        i = i + 1;\n");
            out.push_str("    }\n");
            out.push_str("    acc\n");
            out.push_str("}\n");
        }
        // parse_u64 — the free `String -> Option<u64>` partial parse. The exec mirror
        // of `lower.rs::emit_parse_defs`'s Horner loop with the three handled-or-loud
        // `None` arms (empty / non-digit / would-overflow), computing the SAME value
        // as the L3 form. The L3 PROOF lives in `emit_parse_defs`; this just RUNS the
        // loop. Takes `TString` by value (the call site `.clone()`s).
        out.push_str("#[allow(dead_code)]\n");
        out.push_str("fn parse_u64(s: TString) -> Option<u64> {\n");
        out.push_str("    if s.data.len() == 0 { return None; }\n");
        out.push_str("    let mut acc: u64 = 0;\n");
        out.push_str("    let mut i: usize = 0;\n");
        out.push_str("    while i < s.data.len() {\n");
        out.push_str("        let b: u8 = s.data[i];\n");
        out.push_str("        if b < 48 || b > 57 { return None; }\n");
        out.push_str("        let digit: u64 = (b - 48) as u64;\n");
        out.push_str("        if acc > (u64::MAX - digit) / 10 { return None; }\n");
        out.push_str("        acc = acc * 10 + digit;\n");
        out.push_str("        i = i + 1;\n");
        out.push_str("    }\n");
        out.push_str("    Some(acc)\n");
        out.push_str("}\n");
    }
    // Cluster C4 (`.design/basis/07-strings.md` REQ-8, issue #94): the L1 exec
    // form of the generated `u64`→decimal-`String` round-trip. The L3 form PROVES
    // the round-trip `parse_le(result.data@) == n` via the divide/mod-by-10 digit
    // loop + the `lemma_parse_push` append lemma; L1 is ENTIRELY exec, so this is
    // ordinary runnable Rust — the SAME digit-extraction loop (LSB-first push of
    // `(m % 10) + 48`, then `m /= 10`), reversed at the end to the human-readable
    // MSB-first decimal (the construction is LSB-first; the display reverses — the
    // L3 `parse_be(reverse(s)) == parse_le(s)` bridge). Emitted only when the program
    // uses `n.to_string()` (the non-numfmt corpus is byte-unaffected). `n == 0` yields
    // the single byte "0" (the loop body runs once before the reverse). The method
    // `n.to_string()` lowers to a CALL of this free fn (`lower_expr_exec`).
    if program_uses_numfmt_l1(program) {
        out.push('\n');
        out.push_str("#[allow(dead_code)]\n");
        out.push_str("fn u64_to_string(n: u64) -> TString {\n");
        out.push_str("    let mut data: Vec<u8> = Vec::new();\n");
        out.push_str("    let mut m: u64 = n;\n");
        out.push_str("    if m == 0 { data.push(48u8); }\n");
        out.push_str("    while m > 0 {\n");
        out.push_str("        data.push((m % 10) as u8 + 48u8);\n");
        out.push_str("        m = m / 10;\n");
        out.push_str("    }\n");
        // Reverse the LSB-first construction buffer to the human-readable MSB-first
        // display order (REQ-8, blocker #96) — the BUILT binary prints "42", not "24".
        // Mirrors the L3 reverse loop in `lower.rs::emit_numfmt_defs`, so L1 and L3
        // agree byte-for-byte (both now reverse to MSB-first).
        out.push_str("    data.reverse();\n");
        out.push_str("    TString { data }\n");
        out.push_str("}\n");
        // The L1 runnable form of the round-trip spec fns (`parse_be`/`parse_le`/
        // `pow10`). At L3 these are `spec fn`s carrying the PROOF; at L1 a contract
        // `ens parse_be(result) == n` becomes an always-active runtime CHECK, so the
        // named parse fn must be a real runnable fn. `u64_to_string` now reverses to
        // MSB-first display order (REQ-8, blocker #96), so the surface round-trip is
        // `parse_be` — the MSB-first parse: a left-to-right Horner accumulate
        // (`acc = acc*10 + (s[i]-48)`, `data[0]` most significant), matching the L3
        // spec `parse_be`. `parse_le` (the LSB-first parse, `data[0]` least
        // significant) is ALSO emitted: it is the construction-order value the bridge
        // carries the proof through, and a contract may still name it. Both take
        // `&TString` so the ens check borrows (never moves) the bound `result`. L1 and
        // L3 agree byte-for-byte (both reverse to MSB-first — no display divergence).
        // `pow10` is the decimal weight (emitted for completeness; a contract may name it).
        out.push_str("#[allow(dead_code)]\n");
        out.push_str("fn pow10(k: u64) -> u64 {\n");
        out.push_str("    let mut p: u64 = 1;\n");
        out.push_str("    let mut i: u64 = 0;\n");
        out.push_str("    while i < k { p = p * 10; i = i + 1; }\n");
        out.push_str("    p\n");
        out.push_str("}\n");
        out.push_str("#[allow(dead_code)]\n");
        out.push_str("fn parse_be(s: &TString) -> u64 {\n");
        out.push_str("    let mut acc: u64 = 0;\n");
        out.push_str("    let mut i: usize = 0;\n");
        out.push_str("    while i < s.data.len() {\n");
        out.push_str("        acc = acc * 10 + (s.data[i] as u64 - 48);\n");
        out.push_str("        i = i + 1;\n");
        out.push_str("    }\n");
        out.push_str("    acc\n");
        out.push_str("}\n");
        out.push_str("#[allow(dead_code)]\n");
        out.push_str("fn parse_le(s: &TString) -> u64 {\n");
        out.push_str("    let mut acc: u64 = 0;\n");
        out.push_str("    let mut i: usize = s.data.len();\n");
        out.push_str("    while i > 0 {\n");
        out.push_str("        i = i - 1;\n");
        out.push_str("        acc = acc * 10 + (s.data[i] as u64 - 48);\n");
        out.push_str("    }\n");
        out.push_str("    acc\n");
        out.push_str("}\n");
    }
    out
}

/// The bounded-`Vec` capacity (`.design/basis/04-collections.md` REQ-5; the
/// `lower.rs` L3 `VEC_CAP` idiom `1_000_000`). At L1 (runtime checks, not an SMT
/// proof) the bound is enforced by an always-active `thermite_check!`.
const VEC_CAP_L1: usize = 1_000_000;

/// Emit the per-element `TVec<elem>` runtime wrapper(s) for every `Vec<T>` the
/// program reaches (Cluster C6 #98, `.design/basis/04-collections.md`
/// REQ-5/REQ-8/REQ-9). The L1 EXEC mirror of `lower.rs::emit_vec_wrappers`: PLAIN
/// Rust (no `verus!`/`Seq`/`requires`), with the capacity / no-OOB guards as
/// always-active `thermite_check!`s so a violated contract ABORTS loudly (§6 L1
/// handled-or-loud) rather than UB. EMPTY when the program uses no `Vec`
/// (byte-stable for the non-`Vec` corpus). The element wrapper(s) are emitted
/// inner-first (a nested `Vec<Vec<u64>>` emits `TVecU64` before `TVecTVecU64`) via
/// `collect_vec_elem_types`'s ordering, so the outer wrapper's field type resolves.
fn emit_vec_runtime_l1(program: &Program) -> Result<String, LowerError> {
    let elems = collect_vec_elem_types(program);
    if elems.is_empty() {
        return Ok(String::new());
    }
    let mut out = String::new();
    for elem in &elems {
        let name = tvec_name(elem)?;
        let ety = lower_type(elem)?;
        let copy = elem_is_copy(elem);
        let cap = VEC_CAP_L1;
        out.push('\n');
        // `Clone`: the L1 ens-check snapshots a non-Copy parameter before the body
        // consumes it (`lower_fn_l1`'s `<p>__pre` snapshot), so a `TVec*` param named
        // in an `ens` after a move no longer rustc-`E0382`s. A `TVec*` over a
        // `Clone` element is a deep copy.
        out.push_str("#[derive(Debug, Clone)]\n");
        out.push_str("#[allow(dead_code)]\n");
        writeln!(out, "struct {name} {{ data: Vec<{ety}> }}").ok();
        out.push_str("#[allow(dead_code)]\n");
        writeln!(out, "impl {name} {{").ok();
        writeln!(
            out,
            "    fn new() -> {name} {{ {name} {{ data: Vec::new() }} }}"
        )
        .ok();
        out.push_str("    fn len(&self) -> u64 { self.data.len() as u64 }\n");
        // `get`: no-OOB guard, then index. Copy → by value; non-Copy → borrow `&T`
        // (the L1 mirror of the REQ-9 borrow-`get` — never move a non-Copy element
        // out of the backing run).
        if copy {
            writeln!(out, "    fn get(&self, i: usize) -> {ety} {{").ok();
            out.push_str(
                "        thermite_check!(\"req\", \"i < self.len()\", i < self.data.len());\n",
            );
            out.push_str("        self.data[i]\n");
            out.push_str("    }\n");
        } else {
            writeln!(out, "    fn get(&self, i: usize) -> &{ety} {{").ok();
            out.push_str(
                "        thermite_check!(\"req\", \"i < self.len()\", i < self.data.len());\n",
            );
            out.push_str("        &self.data[i]\n");
            out.push_str("    }\n");
        }
        // `push`: capacity guard, then append (consumes the owned element).
        writeln!(out, "    fn push(&mut self, x: {ety}) {{").ok();
        writeln!(
            out,
            "        thermite_check!(\"req\", \"self.len() < CAP\", self.data.len() < {cap});"
        )
        .ok();
        out.push_str("        self.data.push(x);\n");
        out.push_str("    }\n");
        // `pop_last`: len>0 guard, then drop the last (REQ-8).
        out.push_str("    fn pop_last(&mut self) {\n");
        out.push_str(
            "        thermite_check!(\"req\", \"self.len() > 0\", self.data.len() > 0);\n",
        );
        out.push_str("        self.data.pop();\n");
        out.push_str("    }\n");
        // `last`: len>0 guard, then read the last. Copy → value; non-Copy → borrow.
        if copy {
            writeln!(out, "    fn last(&self) -> {ety} {{").ok();
            out.push_str(
                "        thermite_check!(\"req\", \"self.len() > 0\", self.data.len() > 0);\n",
            );
            out.push_str("        self.data[self.data.len() - 1]\n");
            out.push_str("    }\n");
        } else {
            writeln!(out, "    fn last(&self) -> &{ety} {{").ok();
            out.push_str(
                "        thermite_check!(\"req\", \"self.len() > 0\", self.data.len() > 0);\n",
            );
            out.push_str("        &self.data[self.data.len() - 1]\n");
            out.push_str("    }\n");
        }
        // `insert`: i<=len && len<CAP guard, then splice (REQ-8).
        writeln!(out, "    fn insert(&mut self, i: usize, x: {ety}) {{").ok();
        writeln!(
            out,
            "        thermite_check!(\"req\", \"i <= self.len() && self.len() < CAP\", i <= self.data.len() && self.data.len() < {cap});"
        )
        .ok();
        out.push_str("        self.data.insert(i, x);\n");
        out.push_str("    }\n");
        // `remove`: i<len guard, then delete (REQ-8).
        out.push_str("    fn remove(&mut self, i: usize) {\n");
        out.push_str(
            "        thermite_check!(\"req\", \"i < self.len()\", i < self.data.len());\n",
        );
        out.push_str("        self.data.remove(i);\n");
        out.push_str("    }\n");
        // `contains`: a linear scan (Copy element `==` only — the L3 form omits a
        // non-Copy `contains` too, REQ-9).
        if copy {
            writeln!(out, "    fn contains(&self, x: {ety}) -> bool {{").ok();
            out.push_str("        let mut i: usize = 0;\n");
            out.push_str("        while i < self.data.len() {\n");
            out.push_str("            if self.data[i] == x { return true; }\n");
            out.push_str("            i += 1;\n");
            out.push_str("        }\n");
            out.push_str("        false\n");
            out.push_str("    }\n");
        }
        out.push_str("}\n");
    }
    Ok(out)
}

/// True if the L1 program uses `n.to_string()` anywhere (the L1 mirror of
/// `lower.rs::program_uses_numfmt`): a `to_string` `MethodCall` requires the
/// generated `u64_to_string` runnable form emitted. EMPTY otherwise (byte-stable
/// for the non-numfmt corpus). A contract `parse_le`/`pow10` reference is L3-only
/// (a spec fn never runs as exec code at L1), so only the `to_string` method drives
/// the L1 emission — the round-trip PROOF lives at L3, the L1 fn just RUNS the loop.
fn program_uses_numfmt_l1(program: &Program) -> bool {
    program.items.iter().any(|item| match item {
        Item::Fn(f) => f.body.as_ref().map(block_has_to_string).unwrap_or(false),
        Item::SpecFn(s) => block_has_to_string(&s.body),
        Item::Struct(_) | Item::Enum(_) => false,
    })
}

/// True if a block calls `n.to_string()` anywhere (REQ-8 L1) — a full-tree walk
/// over the body statements + tail, mirroring `block_has_str_lit_l1`.
fn block_has_to_string(block: &Block) -> bool {
    block.stmts.iter().any(stmt_has_to_string)
        || block
            .tail
            .as_deref()
            .map(expr_has_to_string)
            .unwrap_or(false)
}

fn stmt_has_to_string(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Let { init, .. } => expr_has_to_string(init),
        Stmt::Assign { target, value } => expr_has_to_string(target) || expr_has_to_string(value),
        Stmt::Return(opt) => opt.as_ref().map(expr_has_to_string).unwrap_or(false),
        Stmt::If {
            cond, then, else_, ..
        } => {
            expr_has_to_string(cond)
                || block_has_to_string(then)
                || else_.as_ref().map(block_has_to_string).unwrap_or(false)
        }
        Stmt::Loop(l) => block_has_to_string(&l.body),
        Stmt::Expr(e) => expr_has_to_string(e),
        Stmt::Break | Stmt::Continue => false,
    }
}

/// True if a `to_string` method call appears anywhere in `expr` (REQ-8 L1) — a
/// full structural walk mirroring `expr_has_str_lit_l1`.
fn expr_has_to_string(expr: &Expr) -> bool {
    match expr {
        Expr::MethodCall {
            receiver,
            name,
            args,
        } => {
            name == "to_string"
                || expr_has_to_string(receiver)
                || args.iter().any(expr_has_to_string)
        }
        Expr::IntLit { .. } | Expr::BoolLit(_) | Expr::Path(_) | Expr::StrLit(_) => false,
        Expr::Call { callee, args } => {
            expr_has_to_string(callee) || args.iter().any(expr_has_to_string)
        }
        Expr::Field { receiver, .. } => expr_has_to_string(receiver),
        Expr::Closure { body, .. } => expr_has_to_string(body),
        Expr::Match { scrutinee, arms } => {
            expr_has_to_string(scrutinee) || arms.iter().any(|a| expr_has_to_string(&a.body))
        }
        Expr::If { cond, then, else_ } => {
            expr_has_to_string(cond) || block_has_to_string(then) || block_has_to_string(else_)
        }
        Expr::Binary { lhs, rhs, .. } => expr_has_to_string(lhs) || expr_has_to_string(rhs),
        Expr::Index { base, index } => {
            expr_has_to_string(base)
                || match index {
                    IndexArg::Single(e) | IndexArg::RangeTo(e) | IndexArg::RangeFrom(e) => {
                        expr_has_to_string(e)
                    }
                    IndexArg::Range(lo, hi) => expr_has_to_string(lo) || expr_has_to_string(hi),
                }
        }
        Expr::Cast { expr, .. } | Expr::Ref { expr, .. } | Expr::Deref(expr) => {
            expr_has_to_string(expr)
        }
        Expr::StructLit { fields, .. } => fields.iter().any(|(_, v)| expr_has_to_string(v)),
        Expr::Is { scrutinee, .. } => expr_has_to_string(scrutinee),
        Expr::Unary { expr, .. } => expr_has_to_string(expr),
    }
}
