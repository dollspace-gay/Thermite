//! `forge/src/lean_export.rs` — the Thermite→Lean OBLIGATION EXPORTER
//! (`.design/verified/proof-backends.md` REQ-6/REQ-7; increment (ii-b), the
//! `#240` chain, ref #203).
//!
//! Serializes a checked PURE-CONTRACT item (the §4 scope: a `fn`/`spec fn` whose
//! body is a PURE EXPRESSION denoting in `intVal` — the `S_C` domain) into a
//! self-contained Lean file that INSTANTIATES the SHIPPED spine encodings
//! (`lean/Thermite/Ast.lean`'s `Expr`/`CmpOp`/`LogOp`/`ArithOp`/`CastTy`/`CombName`/
//! `Pred`/`Variant`/`MatchArm`/`RangeArg`/`SpecFn`/`Registry` + `Denote.lean`'s
//! `Env`/`OptResVal`/`stabilizes`/`stabilizesProp` + `Stabilize.lean`'s tier-(a)
//! fuel-free keys). The exporter does NOT define a new semantics — it targets the
//! already-kernel-proven `S` (REQ-6 EXP: arm-by-arm + the drift tripwire).
//!
//! ## The three load-bearing pieces (§4)
//!
//! - **The AST encoder** ([`encode_expr`]): each Thermite `Expr` (the frozen-subset
//!   constructs the spine models) → its Lean constructor term. An OUT-of-spine
//!   construct (Field / TupleProj / user-ADT match / holes / a non-pure body /
//!   etc.) → a structured export [`ExportRefusal`] (the item is NOT Lean-exportable;
//!   an honest skip, NEVER a silent omission).
//! - **`R_item`** ([`build_registry`]): `def R_item : Thermite.Registry := fun name
//!   => match name with | "f" => some ⟨[params], body⟩ | … | _ => none`, populated by
//!   the TRANSITIVE `req ∪ ens ∪ body ∪ dec` full-expression-position closure (the
//!   #226 set — REUSING the SAME `called_spec_fns` the [`crate::obligation::
//!   Obligation`] env carries, the ONE-closure #192 lesson). **THE HARD GATE:** the
//!   export REFUSES (a structured error) if any reachable name is missing a
//!   definition (`calledSpecFns(item) ⊄ dom(R_item)`). Per-name resolution lemmas
//!   `example : R_item "f" ≠ none := by decide` are emitted ALONGSIDE (belt +
//!   suspenders, §4 mechanism 2).
//! - **The theorem** ([`emit_theorem`]), THREE-tier per §6.1:
//!   - Tier (a) — `req`/`ens`/body ALL specCall-free: the FUEL-FREE form
//!     `denote 0 …` / `intVal 0 … = r` via the `stabilizes_iff_intVal_zero` /
//!     `stabilizesProp_iff_denote_zero` corollaries (cited in the emitted comment).
//!     Auto tactic battery `first | decide | (intro …; simp_all; omega) | …`.
//!   - Tier (b) — spec-calls present, registry NON-recursive (a finite DAG): the
//!     exporter STATICALLY UNFOLDS the calls to finite depth into a fuel-free goal,
//!     then tier (a)'s battery.
//!   - Tier (c) — RECURSIVE registry: the §4 stabilized `∀ r, stabilizes body env r
//!     → reqStable → ensStable@r` form, marked INTERACTIVE-ONLY — [`export_item`]
//!     returns the file (for increment-(iii) use) but [`crate::engine::LeanEngine`]
//!     does NOT invoke lake (it returns `Unknown("interactive-only")`).
//!
//! This module is NOT wired into the default `check` path (Verus stays the sole
//! default engine — byte-identical); the [`crate::engine::LeanEngine`] constructs
//! it directly, and the `--engine` surface is increment (iii).
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-6 (the Lean exporter — emit source instantiating `S`; the hard gate; EXP arm-by-arm) | SHIPPED (pure-contract class, tiers (a)/(b)/(c)) | `pub fn export_item` emits a self-contained Lean file: `encode_expr` maps each frozen-subset `Expr` arm-by-arm to its `Ast.lean` constructor (OUT-of-spine → `ExportRefusal::OutOfFragment`); `build_registry` populates `R_item` from the `called_spec_fns` closure with the HARD GATE (`ExportRefusal::IncompleteRegistry` when a reached name is undefined) + the per-name `decide` resolution lemmas; `emit_theorem` emits the §4 stabilized form (tier (c)) or the fuel-free tier-(a)/(b) form. Non-test consumer: `engine::LeanEngine::{fragment,discharge}` calls `export_item` + `tier_of` on the live discharge path. The arms table is pinned in `.design/verified/rust-lean-correspondence.md` Table 4 (the EXP drift tripwire). |
//! | REQ-7 (Lean discharge modes — AUTO tiers (a)/(b) fuel-free; tier (c) interactive) | SHIPPED (AUTO tiers (a)/(b); tier (c) interactive-marked) | `tier_of` classifies the obligation by registry shape (`ExportTier::{FuelFreeAuto,StaticUnfoldAuto,RecursiveInteractive}`); tiers (a)/(b) emit a fuel-free goal + the `decide`/`simp`/`omega` battery (`auto_tactic_battery`); tier (c) emits the `∃N∀fuel` stabilized form marked interactive (the engine returns `Unknown`, NOT lake-invoked). The recursive-registry detection is `registry_is_recursive`. |

use crate::obligation::Obligation;
use std::collections::BTreeMap;
use thermite_syntax::{
    BinOp, Block, Expr, IndexArg, Item, Pattern, PrimType, Program, SpecFnItem, Stmt, Type, UnaryOp,
};

/// Why an item is NOT Lean-exportable (`.design/verified/proof-backends.md` §4 —
/// "OUT-of-spine constructs → a structured export REFUSAL; the item is not
/// Lean-exportable; honest skip"). A refusal is NEVER a silent omission and NEVER a
/// proof cheat — it is the exporter saying "this construct is outside `S`'s frozen
/// subset / outside the pure-contract class, so I decline to emit". The
/// [`crate::engine::LeanEngine`] maps a refusal to "the fragment does not admit this
/// obligation" (a skip), never to `Proven`/`Refuted`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportRefusal {
    /// An `Expr` construct outside the frozen spine subset (Field / TupleProj /
    /// StructLit / Deref / a user-ADT match arm / an unsupported cast target / a
    /// closure outside a combinator predicate / etc.). Carries a human description
    /// of the offending construct.
    OutOfFragment(String),
    /// The body is NOT a pure expression denoting in `intVal` (the §4.1 exec-body
    /// bridge is increment (iv)): a `let`/`assign`/`return`/`loop`/`if`-statement
    /// body, or a block with no tail expression. The pure-contract class is scoped
    /// to a body that is a single tail `Expr` over `S_C` (REQ-6 §4 SCOPE).
    NotPureContract(String),
    /// THE HARD GATE (§4 mechanism 1): a spec-fn reachable from
    /// `req ∪ ens ∪ body ∪ dec(item)` is MISSING a definition
    /// (`calledSpecFns(item) ⊄ dom(R_item)`). The export refuses to emit rather
    /// than emit an incomplete `R_item` whose unresolved `specCall` would bottom to
    /// the `intVal` Int-`0` and self-certify (Pin B / Pin C / Pin G). Carries the
    /// missing name(s) — INCLUDING an UNDEFINED callee with no in-program definition
    /// at all (the `mystery(x)` Pin G: `calledSpecFns` is collected WITHOUT the
    /// defined-names filter, so an undefined symbol is a missing name, not invisible).
    IncompleteRegistry(Vec<String>),
    /// THE RESULT-SORT GATE (§4 SCOPE / §4.1 — the Pin H bool-result class): the
    /// item's body does NOT denote in `intVal` (its declared result type is not an
    /// integer sort — `bool`/unit/ADT). The §4 pure-contract class is scoped to a
    /// body denoting in `intVal` (the result `r : Int`); `Denote.lean`'s `intVal`
    /// bottoms EVERY non-integer-sorted node to the canonical `0`, so a `-> bool`
    /// item would have `result` bound to `0` for ANY body and a contract AND its
    /// negation would BOTH certify. The bool/unit/ADT-result binding is the
    /// increment-(iv) `bindBool` bridge; until then a non-integer result REFUSES
    /// (an honest skip). Carries the offending result type.
    NonIntResult(String),
    /// The item carries an open body hole (`?N`) — short-circuited L0 before any
    /// engine (§8 OUT set); not exportable.
    OpenHole(String),
}

impl std::fmt::Display for ExportRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExportRefusal::OutOfFragment(d) => {
                write!(
                    f,
                    "out-of-fragment construct (not in S's frozen subset): {d}"
                )
            }
            ExportRefusal::NotPureContract(d) => {
                write!(f, "not a pure-contract item (the §4 scope): {d}")
            }
            ExportRefusal::IncompleteRegistry(names) => write!(
                f,
                "incomplete registry (the §4 hard gate): reachable spec-fn(s) {names:?} \
                 have no definition — calledSpecFns ⊄ dom(R_item)"
            ),
            ExportRefusal::NonIntResult(ty) => write!(
                f,
                "non-integer result sort (the §4 pure-contract scope is intVal-denoting \
                 bodies; bool/unit/ADT is the increment-(iv) bindBool bridge): {ty}"
            ),
            ExportRefusal::OpenHole(d) => write!(f, "open body hole (L0, no engine): {d}"),
        }
    }
}

/// The export TIER (`.design/verified/proof-backends.md` §6.1, the #216
/// three-tier story). The AUTO tiers (a)/(b) emit a FUEL-FREE shallow goal the
/// `decide`/`simp`/`omega` battery discharges (matching the z3-demotion grounding);
/// the INTERACTIVE tier (c) emits the `∃N∀fuel` stabilized form (a recursive
/// registry's per-env `∃N` witness needs induction — NOT auto-dischargeable).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportTier {
    /// Tier (a): `req`/`ens`/body are ALL `specCallFree` — emit `denote 0 …` /
    /// `intVal 0 … = r` (fuel-free, via `stabilizes_iff_intVal_zero`).
    FuelFreeAuto,
    /// Tier (b): spec-calls present, registry NON-recursive (a finite DAG) —
    /// statically unfold to finite depth, then tier (a).
    StaticUnfoldAuto,
    /// Tier (c): RECURSIVE registry — the `∃N∀fuel` stabilized form, INTERACTIVE
    /// only (the engine does NOT invoke lake; it returns `Unknown`).
    RecursiveInteractive,
}

impl ExportTier {
    /// Is this an AUTO tier (the engine invokes lake)? Tiers (a)/(b) are auto; tier
    /// (c) is interactive-only.
    #[must_use]
    pub fn is_auto(self) -> bool {
        matches!(
            self,
            ExportTier::FuelFreeAuto | ExportTier::StaticUnfoldAuto
        )
    }

    /// A stable tag for diagnostics / the engine's `Unknown` reason.
    #[must_use]
    pub fn tag(self) -> &'static str {
        match self {
            ExportTier::FuelFreeAuto => "fuel-free-auto",
            ExportTier::StaticUnfoldAuto => "static-unfold-auto",
            ExportTier::RecursiveInteractive => "recursive-interactive",
        }
    }
}

/// A successfully exported Lean obligation (`.design/verified/proof-backends.md`
/// REQ-6). Carries the self-contained Lean SOURCE (instantiating the spine), the
/// [`ExportTier`] (so the engine knows whether to invoke lake), and the registry
/// names it populated `R_item` with (the EXP registry-faithfulness inspection
/// surface).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportedObligation {
    /// The self-contained Lean file source (a `import Thermite.Stabilize` + the
    /// `R_item` def + per-name resolution lemmas + the theorem).
    pub source: String,
    /// The export tier (REQ-7 / §6.1).
    pub tier: ExportTier,
    /// The spec-fn names `R_item` was populated with (the full-expression-position
    /// closure; the EXP registry-faithfulness inspection surface).
    pub registry_names: Vec<String>,
}

/// Encode a Thermite [`Expr`] into its Lean `Thermite.Expr` constructor TERM
/// (`.design/verified/proof-backends.md` REQ-6 EXP — arm-by-arm). Each frozen-subset
/// construct maps to the `Ast.lean` constructor whose denotation `Denote.lean`
/// assigns it; an OUT-of-spine construct is a structured [`ExportRefusal`]. The
/// arms here MIRROR `lean/Thermite/RefEncode.lean`'s `encode` recursion (the EXP
/// drift tripwire — pinned in `rust-lean-correspondence.md` Table 4).
///
/// The [`EncodeCtx`] coercion frame sorts a `Path` name into `seqVar` (a slice
/// param), `strVar` (a `String` param), `optResVar` (an Option/Result param), or
/// `var` (an integer free name) — the same dispatch the Rust reference encoder does.
fn encode_expr(e: &Expr, ctx: &EncodeCtx) -> Result<String, ExportRefusal> {
    match e {
        Expr::IntLit { value, .. } => Ok(format!("(Thermite.Expr.intLit {value})")),
        Expr::BoolLit(b) => Ok(format!("(Thermite.Expr.boolLit {b})")),
        Expr::Path(segs) => {
            if segs.len() != 1 {
                return Err(ExportRefusal::OutOfFragment(format!(
                    "qualified path {segs:?} (only single-segment names are in S_C)"
                )));
            }
            let name = &segs[0];
            // Sort the free name by the env's coercion frame (the same dispatch the
            // Rust reference encoder does — slice→@-view/seqVar, String→strVar,
            // Option/Result→optResVar, else the integer var).
            if ctx.seq_params.contains(name) {
                Ok(format!("(Thermite.Expr.seqVar {})", lean_str(name)))
            } else if ctx.string_params.contains(name) {
                Ok(format!("(Thermite.Expr.strVar {})", lean_str(name)))
            } else if ctx.optres_params.contains(name) {
                Ok(format!("(Thermite.Expr.optResVar {})", lean_str(name)))
            } else {
                Ok(format!("(Thermite.Expr.var {})", lean_str(name)))
            }
        }
        Expr::Binary { op, lhs, rhs } => encode_binary(*op, lhs, rhs, ctx),
        Expr::Unary { op, expr } => match op {
            // `!` on a Prop subterm is logical negation (`Expr.neg`); on an integer
            // it is bitwise-not, which `S_C` does NOT model (the spine has no
            // bitwise-not Expr arm) — `Expr.neg`'s denotation is `¬ denote e`, so a
            // bool operand is faithful; an integer `!` is rejected downstream by the
            // sort (the spine has no integer `neg`), an honest residual.
            UnaryOp::Not => Ok(format!("(Thermite.Expr.neg {})", encode_expr(expr, ctx)?)),
        },
        Expr::Cast { expr, ty } => {
            let cast_ty = encode_cast_target(ty)?;
            Ok(format!(
                "(Thermite.Expr.cast {} {cast_ty})",
                encode_expr(expr, ctx)?
            ))
        }
        Expr::Index { base, index } => encode_index(base, index, ctx),
        Expr::MethodCall {
            receiver,
            name,
            args,
        } => encode_method_call(receiver, name, args, ctx),
        Expr::Call { callee, args } => encode_call(callee, args, ctx),
        Expr::Match { scrutinee, arms } => encode_match(scrutinee, arms, ctx),
        Expr::Is { scrutinee, variant } => {
            let v = encode_variant(variant)?;
            Ok(format!(
                "(Thermite.Expr.is_ {} {v})",
                encode_expr(scrutinee, ctx)?
            ))
        }
        Expr::Ref { expr, .. } => {
            // A `&xs[..i]` range borrow is `Ref(Index(range))` — routed through the
            // index encoder, which produces the `subrange` form. A bare `&e` of a
            // non-index is the identity on the value in S_C; encode the inner.
            match expr.as_ref() {
                Expr::Index { base, index } => encode_index(base, index, ctx),
                other => encode_expr(other, ctx),
            }
        }
        // OUT of S_C's frozen subset (honest refusal — §4 / §8 OUT set):
        Expr::Field { name, .. } => Err(ExportRefusal::OutOfFragment(format!(
            "field access `.{name}` (S_C has no struct-field projection)"
        ))),
        Expr::TupleProj { index, .. } => Err(ExportRefusal::OutOfFragment(format!(
            "tuple projection `.{index}` (S_C has no tuple projection)"
        ))),
        Expr::StructLit { path, .. } => Err(ExportRefusal::OutOfFragment(format!(
            "struct literal {path:?} (S_C has no ADT construction)"
        ))),
        Expr::Deref(_) => Err(ExportRefusal::OutOfFragment(
            "box dereference `*e` (S_C has no Box)".to_string(),
        )),
        Expr::StrLit(_) => Err(ExportRefusal::OutOfFragment(
            "string literal (S_C has no in-contract string literal Expr)".to_string(),
        )),
        Expr::Tuple(_) => Err(ExportRefusal::OutOfFragment(
            "tuple construction (S_C has no tuple value)".to_string(),
        )),
        Expr::If { .. } => Err(ExportRefusal::OutOfFragment(
            "if-expression in contract position (S_C has no if-Expr)".to_string(),
        )),
        Expr::Closure { .. } => Err(ExportRefusal::OutOfFragment(
            "bare closure (S_C closures appear ONLY as a combinator predicate)".to_string(),
        )),
    }
}

/// Encode a binary `Expr` — a comparison (`CmpOp`), a logical connective (`LogOp`),
/// or an arithmetic op (`ArithOp`) — to the matching `Ast.lean` constructor (REQ-6
/// EXP; mirrors `RefEncode.lean`'s `encOp`/`encLog`/`encArith` dispatch, #176).
fn encode_binary(
    op: BinOp,
    lhs: &Expr,
    rhs: &Expr,
    ctx: &EncodeCtx,
) -> Result<String, ExportRefusal> {
    let l = encode_expr(lhs, ctx)?;
    let r = encode_expr(rhs, ctx)?;
    let (ctor, op_name): (&str, &str) = match op {
        BinOp::Eq => ("cmp", "Thermite.CmpOp.eq"),
        BinOp::Ne => ("cmp", "Thermite.CmpOp.ne"),
        BinOp::Lt => ("cmp", "Thermite.CmpOp.lt"),
        BinOp::Le => ("cmp", "Thermite.CmpOp.le"),
        BinOp::Gt => ("cmp", "Thermite.CmpOp.gt"),
        BinOp::Ge => ("cmp", "Thermite.CmpOp.ge"),
        BinOp::And => ("logic", "Thermite.LogOp.and"),
        BinOp::Or => ("logic", "Thermite.LogOp.or"),
        BinOp::Add => ("arith", "Thermite.ArithOp.add"),
        BinOp::Sub => ("arith", "Thermite.ArithOp.sub"),
        BinOp::Mul => ("arith", "Thermite.ArithOp.mul"),
        BinOp::Div => ("arith", "Thermite.ArithOp.div"),
        BinOp::Rem => ("arith", "Thermite.ArithOp.rem"),
        BinOp::Shl => ("arith", "Thermite.ArithOp.shl"),
        BinOp::Shr => ("arith", "Thermite.ArithOp.shr"),
        BinOp::BitAnd => ("arith", "Thermite.ArithOp.bitAnd"),
        BinOp::BitOr => ("arith", "Thermite.ArithOp.bitOr"),
        BinOp::BitXor => ("arith", "Thermite.ArithOp.bitXor"),
    };
    Ok(format!("(Thermite.Expr.{ctor} {op_name} {l} {r})"))
}

/// Encode a cast target to a `Thermite.CastTy` (REQ-6 EXP; mirrors
/// `RefEncode.lean`'s `cast_target`, #177). A cast to anything outside
/// `u64`/`u32`/`usize`/`nat`/`int` is OUT of S_C.
fn encode_cast_target(ty: &Type) -> Result<&'static str, ExportRefusal> {
    match ty {
        Type::Prim(PrimType::U64) => Ok("Thermite.CastTy.u64"),
        Type::Prim(PrimType::U32) => Ok("Thermite.CastTy.u32"),
        Type::Prim(PrimType::Usize) => Ok("Thermite.CastTy.usize"),
        Type::Named(n) if n == "nat" => Ok("Thermite.CastTy.nat"),
        Type::Named(n) if n == "int" => Ok("Thermite.CastTy.int"),
        other => Err(ExportRefusal::OutOfFragment(format!(
            "cast target {other:?} (S_C casts only to u64/u32/usize/nat/int)"
        ))),
    }
}

/// Encode an index `base[idx]` / range borrow (REQ-6 EXP; mirrors
/// `RefEncode.lean`'s `idx`/`subrange` arms, #178).
fn encode_index(base: &Expr, index: &IndexArg, ctx: &EncodeCtx) -> Result<String, ExportRefusal> {
    let b = encode_expr(base, ctx)?;
    match index {
        IndexArg::Single(i) => Ok(format!("(Thermite.Expr.idx {b} {})", encode_expr(i, ctx)?)),
        IndexArg::RangeTo(hi) => Ok(format!(
            "(Thermite.Expr.subrange {b} (Thermite.RangeArg.rangeTo {}))",
            encode_expr(hi, ctx)?
        )),
        IndexArg::Range(lo, hi) => Ok(format!(
            "(Thermite.Expr.subrange {b} (Thermite.RangeArg.range {} {}))",
            encode_expr(lo, ctx)?,
            encode_expr(hi, ctx)?
        )),
        IndexArg::RangeFrom(lo) => Ok(format!(
            "(Thermite.Expr.subrange {b} (Thermite.RangeArg.rangeFrom {}))",
            encode_expr(lo, ctx)?
        )),
    }
}

/// Encode a method call — the `len`/`byte_at` byte-view dispatch (REQ-6 EXP;
/// mirrors `RefEncode.lean`'s `seqLen`/`byteAt` arms, #178/#127). Any other method
/// is OUT of S_C.
fn encode_method_call(
    receiver: &Expr,
    name: &str,
    args: &[Expr],
    ctx: &EncodeCtx,
) -> Result<String, ExportRefusal> {
    let recv = encode_expr(receiver, ctx)?;
    match (name, args) {
        ("len", []) => Ok(format!("(Thermite.Expr.seqLen {recv})")),
        ("byte_at", [i]) => Ok(format!(
            "(Thermite.Expr.byteAt {recv} {})",
            encode_expr(i, ctx)?
        )),
        _ => Err(ExportRefusal::OutOfFragment(format!(
            "method call `.{name}(..)` with {} args (S_C admits only `.len()` / `.byte_at(i)`)",
            args.len()
        ))),
    }
}

/// The frozen combinator names → their `CombName` constructor (REQ-6 EXP; the 8
/// frozen combinators, #179/#182). A free-call callee that is one of these encodes
/// to `Expr.comb`; otherwise it is a spec-fn call (`encode_call`).
fn combinator_name(callee: &str) -> Option<&'static str> {
    match callee {
        "forall_in" => Some("Thermite.CombName.forallIn"),
        "exists_in" => Some("Thermite.CombName.existsIn"),
        "sorted" => Some("Thermite.CombName.sorted"),
        "forall_below" => Some("Thermite.CombName.forallBelow"),
        "forall_from" => Some("Thermite.CombName.forallFrom"),
        "disjoint" => Some("Thermite.CombName.disjoint"),
        "count_where" => Some("Thermite.CombName.countWhere"),
        "permutation_of" => Some("Thermite.CombName.permutationOf"),
        _ => None,
    }
}

/// Encode a free call `f(args)` — a combinator call (`Expr.comb`) or a named
/// spec-fn call (`Expr.specCall`) (REQ-6 EXP; mirrors `RefEncode.lean`'s
/// `encode_call` cases #179/#181). `old(x)` is the pre-state free name (→ `Expr.var
/// "old(x)"`). A qualified / non-`Path` callee is OUT.
fn encode_call(callee: &Expr, args: &[Expr], ctx: &EncodeCtx) -> Result<String, ExportRefusal> {
    let name = match callee {
        Expr::Path(segs) if segs.len() == 1 => segs[0].clone(),
        other => {
            return Err(ExportRefusal::OutOfFragment(format!(
                "call with non-simple callee {other:?}"
            )))
        }
    };
    if name == "old" {
        // `old(x)` — a free pre-state integer name (the encoder treats it as a
        // distinct free name; `RefEncode.lean`'s `old(_)` arm).
        if let [Expr::Path(p)] = args {
            if p.len() == 1 {
                return Ok(format!(
                    "(Thermite.Expr.var {})",
                    lean_str(&format!("old({})", p[0]))
                ));
            }
        }
        return Err(ExportRefusal::OutOfFragment(
            "old(_) of a non-simple argument".to_string(),
        ));
    }
    if let Some(comb) = combinator_name(&name) {
        return encode_combinator(comb, &name, args, ctx);
    }
    // A named spec-fn call: `Expr.specCall name [args]` (#181). The HARD GATE
    // (`build_registry`) guarantees the name is in `R_item`; here we only encode the
    // call form. The args are S_C exprs (re-encoded).
    let arg_terms = args
        .iter()
        .map(|a| encode_expr(a, ctx))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(format!(
        "(Thermite.Expr.specCall {} [{}])",
        lean_str(&name),
        arg_terms.join(", ")
    ))
}

/// Encode a combinator call to `Expr.comb` (REQ-6 EXP; #179/#182). Each combinator
/// populates exactly the fields its `arg_kinds` declares; the others are `none`.
fn encode_combinator(
    comb: &str,
    surface: &str,
    args: &[Expr],
    ctx: &EncodeCtx,
) -> Result<String, ExportRefusal> {
    // Encode a predicate closure `|x| body` → `some (Pred.mk "x" body)` (the bound
    // element var is an integer name in the body).
    let encode_pred = |e: &Expr| -> Result<String, ExportRefusal> {
        match e {
            Expr::Closure { params, body } if params.len() == 1 => {
                let mut inner = ctx.clone();
                inner.bind_int(&params[0]);
                Ok(format!(
                    "(some (Thermite.Pred.mk {} {}))",
                    lean_str(&params[0]),
                    encode_expr(body, &inner)?
                ))
            }
            other => Err(ExportRefusal::OutOfFragment(format!(
                "combinator predicate is not a single-param closure: {other:?}"
            ))),
        }
    };
    let none_e = "none".to_string();
    let none_p = "none".to_string();
    // (seq, seq2, idx, pred) per the combinator's arg_kinds.
    let (seq, seq2, idx, pred): (String, String, String, String) = match (surface, args) {
        ("forall_in", [s, p]) | ("exists_in", [s, p]) | ("count_where", [s, p]) => (
            encode_expr(s, ctx)?,
            none_e.clone(),
            none_e.clone(),
            encode_pred(p)?,
        ),
        ("sorted", [s]) => (
            encode_expr(s, ctx)?,
            none_e.clone(),
            none_e.clone(),
            none_p.clone(),
        ),
        ("disjoint", [a, b]) | ("permutation_of", [a, b]) => (
            encode_expr(a, ctx)?,
            format!("(some {})", encode_expr(b, ctx)?),
            none_e.clone(),
            none_p.clone(),
        ),
        ("forall_below", [s, n, p]) | ("forall_from", [s, n, p]) => (
            encode_expr(s, ctx)?,
            none_e.clone(),
            format!("(some {})", encode_expr(n, ctx)?),
            encode_pred(p)?,
        ),
        _ => {
            return Err(ExportRefusal::OutOfFragment(format!(
                "combinator `{surface}` with {} args (arity mismatch vs the frozen registry)",
                args.len()
            )))
        }
    };
    Ok(format!(
        "(Thermite.Expr.comb {comb} {seq} {seq2} {idx} {pred})"
    ))
}

/// Encode a contract-position `match scrut { arms }` to `Expr.match_` (REQ-6 EXP;
/// #180). Only the C7 built-in Option/Result 2-arm forms are in S_C; a user-ADT arm
/// / a guard / a wildcard is OUT.
fn encode_match(
    scrutinee: &Expr,
    arms: &[thermite_syntax::MatchArm],
    ctx: &EncodeCtx,
) -> Result<String, ExportRefusal> {
    let scrut = encode_expr(scrutinee, ctx)?;
    let mut arm_terms = Vec::new();
    for arm in arms {
        if arm.guard.is_some() {
            return Err(ExportRefusal::OutOfFragment(
                "guarded match arm (out of the C7 fragment)".to_string(),
            ));
        }
        let (variant, binder, inner) = match &arm.pattern {
            Pattern::Enum { path, fields } if path.len() == 1 => {
                let v = encode_variant(path)?;
                match fields.as_slice() {
                    [] => (v, "none".to_string(), ctx.clone()),
                    [Pattern::Binding(b)] => {
                        let mut inner = ctx.clone();
                        inner.bind_int(b);
                        (v, format!("(some {})", lean_str(b)), inner)
                    }
                    _ => {
                        return Err(ExportRefusal::OutOfFragment(
                            "match arm with a non-binding/non-empty payload pattern".to_string(),
                        ))
                    }
                }
            }
            other => {
                return Err(ExportRefusal::OutOfFragment(format!(
                    "match arm pattern {other:?} (only built-in Some/None/Ok/Err in S_C)"
                )))
            }
        };
        arm_terms.push(format!(
            "(Thermite.MatchArm.mk {variant} {binder} {})",
            encode_expr(&arm.body, &inner)?
        ));
    }
    Ok(format!(
        "(Thermite.Expr.match_ {scrut} [{}])",
        arm_terms.join(", ")
    ))
}

/// Encode a built-in variant name to `Thermite.Variant` (REQ-6 EXP; #180). A user
/// variant is OUT of S_C.
fn encode_variant(path: &[String]) -> Result<&'static str, ExportRefusal> {
    let last = path.last().map(String::as_str).unwrap_or("");
    match last {
        "Some" => Ok("Thermite.Variant.some_"),
        "None" => Ok("Thermite.Variant.none_"),
        "Ok" => Ok("Thermite.Variant.ok"),
        "Err" => Ok("Thermite.Variant.err"),
        other => Err(ExportRefusal::OutOfFragment(format!(
            "variant `{other}` (only built-in Some/None/Ok/Err in S_C)"
        ))),
    }
}

/// A Lean string literal (escaping `\` and `"`; Thermite idents never contain them,
/// but `old(x)` parens are fine inside a Lean string). Deterministic (R-CODE-5).
fn lean_str(s: &str) -> String {
    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

/// The encoding context — the env coercion frame that sorts a `Path` free name into
/// `seqVar`/`strVar`/`optResVar`/`var` (the same dispatch the Rust reference encoder
/// does from the [`Obligation`] env). Cloned + narrowed when a closure / match arm
/// binds an integer element/payload var.
#[derive(Debug, Clone, Default)]
struct EncodeCtx {
    seq_params: Vec<String>,
    string_params: Vec<String>,
    optres_params: Vec<String>,
}

impl EncodeCtx {
    /// Bind a name as an INTEGER name in this scope (a closure element var / a match
    /// payload binder): it shadows any outer seq/string/optres sort.
    fn bind_int(&mut self, name: &str) {
        self.seq_params.retain(|n| n != name);
        self.string_params.retain(|n| n != name);
        self.optres_params.retain(|n| n != name);
    }
}

/// The spec-fn DECLARATIONS in scope (a name→`SpecFnItem` map), for `R_item`
/// population + the recursive-registry detection. Built once from the program.
fn spec_decls(program: &Program) -> BTreeMap<String, SpecFnItem> {
    program
        .items
        .iter()
        .filter_map(|i| match i {
            Item::SpecFn(s) => Some((s.name.clone(), s.clone())),
            _ => None,
        })
        .collect()
}

/// Build the `R_item : Thermite.Registry` definition + the per-name resolution
/// lemmas (`.design/verified/proof-backends.md` §4 — the EXPORTER-SIDE HARD GATE).
/// `R_item` is populated by `called` (the full-expression-position closure the
/// [`Obligation`] env carries — the ONE-closure #192 lesson). **THE HARD GATE:**
/// returns [`ExportRefusal::IncompleteRegistry`] if any name in `called` has no
/// in-program definition (`calledSpecFns ⊄ dom(R_item)`).
///
/// Each entry encodes the spec-fn's REAL body (EXP registry-faithfulness, §4): a
/// WRONG body would be an unsound certification the gate cannot catch — so the body
/// is `encode_expr`'d from the spec-fn's actual tail expression, arm-by-arm.
fn build_registry(
    called: &[String],
    decls: &BTreeMap<String, SpecFnItem>,
) -> Result<(String, Vec<String>), ExportRefusal> {
    // THE HARD GATE: every reachable name must resolve to a definition.
    let missing: Vec<String> = called
        .iter()
        .filter(|n| !decls.contains_key(*n))
        .cloned()
        .collect();
    if !missing.is_empty() {
        return Err(ExportRefusal::IncompleteRegistry(missing));
    }

    let mut arms = Vec::new();
    let mut lemmas = Vec::new();
    for name in called {
        let decl = &decls[name];
        let body_expr = spec_fn_body_expr(decl)?;
        // The spec-fn's params are bound to the call args; the body is encoded with
        // the params sorted by their declared type (a spec fn is closed over its
        // params, §4.2).
        let ctx = ctx_for_params(&decl.params);
        let body = encode_expr(body_expr, &ctx)?;
        let params: Vec<String> = decl.params.iter().map(|p| lean_str(&p.name)).collect();
        arms.push(format!(
            "  | {} => some ⟨[{}], {}⟩",
            lean_str(name),
            params.join(", "),
            body
        ));
        // §4 mechanism 2: the per-name resolution lemma. If the exporter ever omits a
        // called spec-fn from `R_item`, this lemma FAILS to compile.
        lemmas.push(format!(
            "example : R_item {} ≠ none := by decide",
            lean_str(name)
        ));
    }
    let r_item = if arms.is_empty() {
        // No reached spec-fns → the EMPTY registry (`fun _ => none`). The §4 hard
        // gate passed vacuously (`∅ ⊆ dom(R_item)`), which is SOUND here precisely
        // because a tier-(a) specCall-free item NEVER denotes a `specCall` (the
        // Pin-B/Pin-C bottom-poisoning needs a reachable-but-omitted call — here
        // there is none, the closure IS empty).
        "def R_item : Thermite.Registry := fun _ => none".to_string()
    } else {
        format!(
            "def R_item : Thermite.Registry := fun name =>\n  match name with\n{}\n  | _ => none",
            arms.join("\n")
        )
    };
    let block = if lemmas.is_empty() {
        r_item
    } else {
        format!("{r_item}\n\n{}", lemmas.join("\n"))
    };
    Ok((block, called.to_vec()))
}

/// The pure tail expression of a spec-fn body, or a refusal if the body is not a
/// single pure tail expr (the pure-contract scope; §4.1's exec-body bridge is
/// increment (iv)).
fn spec_fn_body_expr(s: &SpecFnItem) -> Result<&Expr, ExportRefusal> {
    pure_tail_of_block(&s.body).ok_or_else(|| {
        ExportRefusal::NotPureContract(format!(
            "spec fn `{}` body is not a single pure tail expression",
            s.name
        ))
    })
}

/// A block's pure tail expression: `Some(&expr)` iff the block is a single tail expr
/// with NO statements (the pure-contract class); `None` for any
/// let/assign/return/loop/if statement body or a statement-only block.
fn pure_tail_of_block(b: &Block) -> Option<&Expr> {
    if b.stmts.is_empty() {
        b.tail.as_deref()
    } else {
        None
    }
}

/// The [`EncodeCtx`] for a parameter list — sorts each param into the seq / string /
/// optres / integer coercion frame by its declared [`Type`].
fn ctx_for_params(params: &[thermite_syntax::Param]) -> EncodeCtx {
    let mut ctx = EncodeCtx::default();
    for p in params {
        sort_param(&p.name, &p.ty, &mut ctx);
    }
    ctx
}

/// Sort one param into the coercion frame by its type (a slice `&[u32]` → seq; a
/// `String` → string; an `Option<_>`/`Result<_,_>` → optres; else integer).
fn sort_param(name: &str, ty: &Type, ctx: &mut EncodeCtx) {
    match ty {
        Type::Ref { inner, .. } => sort_param(name, inner, ctx),
        Type::Slice(_) => ctx.seq_params.push(name.to_string()),
        Type::Named(n) if n == "String" => ctx.string_params.push(name.to_string()),
        Type::Generic { name: g, .. } if g == "Option" || g == "Result" => {
            ctx.optres_params.push(name.to_string())
        }
        _ => {}
    }
}

/// Is the registry RECURSIVE? (`.design/verified/proof-backends.md` §6.1 tier (c)
/// vs (b).) A registry is recursive iff some reached spec-fn (transitively) calls
/// itself — i.e. the call graph over `called` has a cycle. A NON-recursive (DAG)
/// registry is tier (b) (statically unfoldable); a recursive one is tier (c)
/// (interactive). Deterministic DFS cycle detection (R-CODE-5).
fn registry_is_recursive(called: &[String], decls: &BTreeMap<String, SpecFnItem>) -> bool {
    fn callees(s: &SpecFnItem, decls: &BTreeMap<String, SpecFnItem>) -> Vec<String> {
        let mut out = Vec::new();
        collect_block_calls(&s.body, decls, &mut out);
        collect_expr_calls(&s.dec.expr, decls, &mut out);
        out
    }
    // DFS with a "currently on the stack" set → a back edge is a cycle.
    fn has_cycle(
        name: &str,
        decls: &BTreeMap<String, SpecFnItem>,
        on_stack: &mut Vec<String>,
        done: &mut Vec<String>,
    ) -> bool {
        if done.iter().any(|n| n == name) {
            return false;
        }
        if on_stack.iter().any(|n| n == name) {
            return true;
        }
        on_stack.push(name.to_string());
        if let Some(decl) = decls.get(name) {
            for c in callees(decl, decls) {
                if decls.contains_key(&c) && has_cycle(&c, decls, on_stack, done) {
                    return true;
                }
            }
        }
        on_stack.retain(|n| n != name);
        done.push(name.to_string());
        false
    }
    let mut on_stack = Vec::new();
    let mut done = Vec::new();
    called
        .iter()
        .any(|n| has_cycle(n, decls, &mut on_stack, &mut done))
}

/// Collect the in-program spec-fn names a `Block` calls (for cycle detection).
fn collect_block_calls(b: &Block, decls: &BTreeMap<String, SpecFnItem>, out: &mut Vec<String>) {
    for stmt in &b.stmts {
        if let Stmt::Let { init, .. } = stmt {
            collect_expr_calls(init, decls, out);
        }
    }
    if let Some(tail) = &b.tail {
        collect_expr_calls(tail, decls, out);
    }
}

/// Collect the in-program spec-fn names an `Expr` calls (for cycle detection).
fn collect_expr_calls(e: &Expr, decls: &BTreeMap<String, SpecFnItem>, out: &mut Vec<String>) {
    match e {
        Expr::Call { callee, args } => {
            if let Expr::Path(segs) = callee.as_ref() {
                if segs.len() == 1 && decls.contains_key(&segs[0]) {
                    out.push(segs[0].clone());
                }
            }
            for a in args {
                collect_expr_calls(a, decls, out);
            }
        }
        Expr::Binary { lhs, rhs, .. } => {
            collect_expr_calls(lhs, decls, out);
            collect_expr_calls(rhs, decls, out);
        }
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::Ref { expr, .. } => {
            collect_expr_calls(expr, decls, out)
        }
        Expr::MethodCall { receiver, args, .. } => {
            collect_expr_calls(receiver, decls, out);
            for a in args {
                collect_expr_calls(a, decls, out);
            }
        }
        Expr::Index { base, index } => {
            collect_expr_calls(base, decls, out);
            match index {
                IndexArg::Single(i) | IndexArg::RangeTo(i) | IndexArg::RangeFrom(i) => {
                    collect_expr_calls(i, decls, out)
                }
                IndexArg::Range(a, b) => {
                    collect_expr_calls(a, decls, out);
                    collect_expr_calls(b, decls, out);
                }
            }
        }
        Expr::Match { scrutinee, arms } => {
            collect_expr_calls(scrutinee, decls, out);
            for arm in arms {
                collect_expr_calls(&arm.body, decls, out);
            }
        }
        Expr::Is { scrutinee, .. } => collect_expr_calls(scrutinee, decls, out),
        Expr::Closure { body, .. } => collect_expr_calls(body, decls, out),
        Expr::If { cond, then, else_ } => {
            collect_expr_calls(cond, decls, out);
            collect_block_calls(then, decls, out);
            collect_block_calls(else_, decls, out);
        }
        _ => {}
    }
}

/// Collect EVERY spec-call-position callee name in an `Expr` — WITHOUT the
/// defined-names filter (the §4 mechanism-1 fix for the Pin G undefined-callee:
/// `collect_expr_calls` filters through `decls.contains_key`, so an UNDEFINED callee
/// `mystery(x)` is invisible to the hard gate and the emitted goal silently carries
/// `Expr.specCall "mystery"` at fuel 0, bottoming to `0` and self-certifying). A
/// callee is a spec-call position iff it is a simple single-segment `Path` that is
/// NEITHER a frozen combinator (`forall_in`/…) NOR `old` — i.e. EXACTLY the calls
/// `encode_call` emits as `Expr.specCall`. This is the full-expression-position
/// principle (§4 mechanism 1): every expression the export denotes against `R_item`
/// contributes its spec-calls, defined or not.
fn collect_all_call_names(e: &Expr, out: &mut Vec<String>) {
    match e {
        Expr::Call { callee, args } => {
            if let Expr::Path(segs) = callee.as_ref() {
                if segs.len() == 1 && segs[0] != "old" && combinator_name(&segs[0]).is_none() {
                    out.push(segs[0].clone());
                }
            }
            for a in args {
                collect_all_call_names(a, out);
            }
        }
        Expr::Binary { lhs, rhs, .. } => {
            collect_all_call_names(lhs, out);
            collect_all_call_names(rhs, out);
        }
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::Ref { expr, .. } => {
            collect_all_call_names(expr, out)
        }
        Expr::MethodCall { receiver, args, .. } => {
            collect_all_call_names(receiver, out);
            for a in args {
                collect_all_call_names(a, out);
            }
        }
        Expr::Index { base, index } => {
            collect_all_call_names(base, out);
            match index {
                IndexArg::Single(i) | IndexArg::RangeTo(i) | IndexArg::RangeFrom(i) => {
                    collect_all_call_names(i, out)
                }
                IndexArg::Range(a, b) => {
                    collect_all_call_names(a, out);
                    collect_all_call_names(b, out);
                }
            }
        }
        Expr::Match { scrutinee, arms } => {
            collect_all_call_names(scrutinee, out);
            for arm in arms {
                collect_all_call_names(&arm.body, out);
            }
        }
        Expr::Is { scrutinee, .. } => collect_all_call_names(scrutinee, out),
        Expr::Closure { body, .. } => collect_all_call_names(body, out),
        Expr::If { cond, then, else_ } => {
            collect_all_call_names(cond, out);
            collect_all_block_call_names(then, out);
            collect_all_block_call_names(else_, out);
        }
        _ => {}
    }
}

/// Collect EVERY spec-call-position callee name in a `Block` (unfiltered; the Pin G
/// companion of [`collect_all_call_names`] over a spec-fn body's statements + tail).
fn collect_all_block_call_names(b: &Block, out: &mut Vec<String>) {
    for stmt in &b.stmts {
        if let Stmt::Let { init, .. } = stmt {
            collect_all_call_names(init, out);
        }
    }
    if let Some(tail) = &b.tail {
        collect_all_call_names(tail, out);
    }
}

/// Does a declared result [`Type`] denote in `intVal` (the §4 pure-contract scope —
/// the Pin H result-sort gate)? An integer sort (`u32`/`u64`/`usize` or the spec
/// `int`/`nat`) binds `result : Int` faithfully; a `bool`/unit/ADT/collection result
/// does NOT (`Denote.lean`'s `intVal` bottoms it to `0`, so a contract and its
/// negation both certify). Only the integer sorts are admitted; everything else is
/// the increment-(iv) `bindBool`/ADT bridge → [`ExportRefusal::NonIntResult`].
fn result_is_int_sorted(ty: &Type) -> bool {
    match ty {
        Type::Prim(PrimType::U32 | PrimType::U64 | PrimType::Usize) => true,
        Type::Named(n) => n == "int" || n == "nat",
        _ => false,
    }
}

/// STATICALLY UNFOLD every in-program spec-fn call in an `Expr` to its body, with
/// the params substituted by the call args (`.design/verified/proof-backends.md`
/// §6.1(b) — "the exporter STATICALLY UNFOLDS every spec-fn call to its FINITE
/// depth at export time, producing a specCall-free `Expr`, then apply tier (a)").
/// Iterated until no in-registry call remains (terminates because the registry is a
/// finite DAG — tier (b)'s precondition; a recursive registry is tier (c) and is
/// NOT unfolded). The unfolded `Expr` must equal the spec-fn's real body substituted
/// arm-by-arm — itself part of EXP (a wrong unfolding is an unsound export the
/// inspection tier catches). Bounded iteration (a guard) keeps it total even if the
/// DAG assumption is violated (it then leaves residual calls → the fuel-free goal
/// would carry a `specCall`, which the encoder rejects as still-present, an honest
/// non-proof, NEVER a silent self-cert).
fn unfold_spec_calls(
    e: &Expr,
    decls: &BTreeMap<String, SpecFnItem>,
) -> Result<Expr, ExportRefusal> {
    // The DAG depth is bounded by the number of distinct spec-fns; iterate that many
    // times + 1 as the safety bound (a recursive registry never reaches here — tier
    // (c) — but the bound keeps the function total regardless).
    let bound = decls.len() + 1;
    let mut cur = e.clone();
    for _ in 0..bound {
        if !expr_has_spec_call(&cur, decls) {
            break;
        }
        cur = unfold_once(&cur, decls)?;
    }
    Ok(cur)
}

/// One unfolding pass: replace each in-program spec-fn `Call(f, args)` with `f`'s
/// body (a pure tail expr) with `f.params` substituted by the UNFOLDED args. Returns
/// an [`ExportRefusal::OutOfFragment`] (capture-unsafe) if the §6.1(b) substitution
/// would CAPTURE a caller free var under a body binder (the Pin I fix; see
/// [`substitute`]).
fn unfold_once(e: &Expr, decls: &BTreeMap<String, SpecFnItem>) -> Result<Expr, ExportRefusal> {
    Ok(match e {
        Expr::Call { callee, args } => {
            let unfolded_args: Vec<Expr> = args
                .iter()
                .map(|a| unfold_once(a, decls))
                .collect::<Result<_, _>>()?;
            if let Expr::Path(segs) = callee.as_ref() {
                if segs.len() == 1 {
                    if let Some(decl) = decls.get(&segs[0]) {
                        if let Some(body) = pure_tail_of_block(&decl.body) {
                            // Substitute params → unfolded args, then keep unfolding
                            // the substituted body (handled by the outer iteration).
                            let mut subst: BTreeMap<String, Expr> = BTreeMap::new();
                            for (p, a) in decl.params.iter().zip(unfolded_args.iter()) {
                                subst.insert(p.name.clone(), a.clone());
                            }
                            return substitute(body, &subst);
                        }
                    }
                }
            }
            Expr::Call {
                callee: callee.clone(),
                args: unfolded_args,
            }
        }
        Expr::Binary { op, lhs, rhs } => Expr::Binary {
            op: *op,
            lhs: Box::new(unfold_once(lhs, decls)?),
            rhs: Box::new(unfold_once(rhs, decls)?),
        },
        Expr::Unary { op, expr } => Expr::Unary {
            op: *op,
            expr: Box::new(unfold_once(expr, decls)?),
        },
        Expr::Cast { expr, ty } => Expr::Cast {
            expr: Box::new(unfold_once(expr, decls)?),
            ty: ty.clone(),
        },
        Expr::Ref { mutable, expr } => Expr::Ref {
            mutable: *mutable,
            expr: Box::new(unfold_once(expr, decls)?),
        },
        Expr::MethodCall {
            receiver,
            name,
            args,
        } => Expr::MethodCall {
            receiver: Box::new(unfold_once(receiver, decls)?),
            name: name.clone(),
            args: args
                .iter()
                .map(|a| unfold_once(a, decls))
                .collect::<Result<_, _>>()?,
        },
        Expr::Index { base, index } => Expr::Index {
            base: Box::new(unfold_once(base, decls)?),
            index: unfold_index(index, decls)?,
        },
        Expr::Match { scrutinee, arms } => Expr::Match {
            scrutinee: Box::new(unfold_once(scrutinee, decls)?),
            arms: arms
                .iter()
                .map(|a| {
                    Ok(thermite_syntax::MatchArm {
                        pattern: a.pattern.clone(),
                        guard: a.guard.clone(),
                        body: unfold_once(&a.body, decls)?,
                    })
                })
                .collect::<Result<_, _>>()?,
        },
        Expr::Is { scrutinee, variant } => Expr::Is {
            scrutinee: Box::new(unfold_once(scrutinee, decls)?),
            variant: variant.clone(),
        },
        Expr::Closure { params, body } => Expr::Closure {
            params: params.clone(),
            body: Box::new(unfold_once(body, decls)?),
        },
        other => other.clone(),
    })
}

/// Unfold spec-calls inside an [`IndexArg`]'s bounds.
fn unfold_index(
    index: &IndexArg,
    decls: &BTreeMap<String, SpecFnItem>,
) -> Result<IndexArg, ExportRefusal> {
    Ok(match index {
        IndexArg::Single(i) => IndexArg::Single(Box::new(unfold_once(i, decls)?)),
        IndexArg::RangeTo(i) => IndexArg::RangeTo(Box::new(unfold_once(i, decls)?)),
        IndexArg::RangeFrom(i) => IndexArg::RangeFrom(Box::new(unfold_once(i, decls)?)),
        IndexArg::Range(a, b) => IndexArg::Range(
            Box::new(unfold_once(a, decls)?),
            Box::new(unfold_once(b, decls)?),
        ),
    })
}

/// Collect the FREE single-segment `Path` names of an `Expr` (the caller-var
/// support of a substituted argument; the Pin I capture test). A closure / match-arm
/// binder REMOVES its bound name from the free set of its body (it is bound there),
/// matching the same scoping [`substitute`] respects.
fn free_path_names(
    e: &Expr,
    bound: &mut Vec<String>,
    out: &mut std::collections::BTreeSet<String>,
) {
    match e {
        Expr::Path(segs) if segs.len() == 1 && !bound.iter().any(|b| b == &segs[0]) => {
            out.insert(segs[0].clone());
        }
        Expr::Call { callee, args } => {
            free_path_names(callee, bound, out);
            for a in args {
                free_path_names(a, bound, out);
            }
        }
        Expr::Binary { lhs, rhs, .. } => {
            free_path_names(lhs, bound, out);
            free_path_names(rhs, bound, out);
        }
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::Ref { expr, .. } => {
            free_path_names(expr, bound, out)
        }
        Expr::MethodCall { receiver, args, .. } => {
            free_path_names(receiver, bound, out);
            for a in args {
                free_path_names(a, bound, out);
            }
        }
        Expr::Index { base, index } => {
            free_path_names(base, bound, out);
            match index {
                IndexArg::Single(i) | IndexArg::RangeTo(i) | IndexArg::RangeFrom(i) => {
                    free_path_names(i, bound, out)
                }
                IndexArg::Range(a, b) => {
                    free_path_names(a, bound, out);
                    free_path_names(b, bound, out);
                }
            }
        }
        Expr::Match { scrutinee, arms } => {
            free_path_names(scrutinee, bound, out);
            for arm in arms {
                let mut inner = bound.clone();
                if let Pattern::Enum { fields: fs, .. } = &arm.pattern {
                    if let [Pattern::Binding(b)] = fs.as_slice() {
                        inner.push(b.clone());
                    }
                }
                free_path_names(&arm.body, &mut inner, out);
            }
        }
        Expr::Is { scrutinee, .. } => free_path_names(scrutinee, bound, out),
        Expr::Closure { params, body } => {
            let mut inner = bound.clone();
            inner.extend(params.iter().cloned());
            free_path_names(body, &mut inner, out);
        }
        _ => {}
    }
}

/// The set of free names a substitution `subst` would INTRODUCE (the union of the
/// free names of every substituted argument) — the names a body binder must NOT
/// shadow, on pain of CAPTURE (the Pin I test).
fn subst_free_names(subst: &BTreeMap<String, Expr>) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    for arg in subst.values() {
        let mut bound = Vec::new();
        free_path_names(arg, &mut bound, &mut out);
    }
    out
}

/// Substitute free `Path` names by their bound exprs (the spec-fn param→arg
/// substitution of static unfolding, §6.1(b)). A `Path([name])` that is a key in
/// `subst` is replaced; everything else recurses structurally. A bound closure/
/// match-arm binder SHADOWS a param of the same name (the spec-fn body is closed
/// over its params, §4.2, so a shadowed name is the closure's element, not the
/// param — kept correct by removing the shadowed key in that scope).
///
/// CAPTURE-SAFETY (the Pin I fix, §6.1(b) "the unfolded `Expr` MUST equal the real
/// body substituted"): a body binder (a closure element var / a match payload
/// binder) that EQUALS a free name of a STILL-LIVE substituted argument would
/// CAPTURE that caller var — silently changing meaning (`cntk(xs, k as int)` with
/// `|k| … == v` becomes the tautology `|k| … == k`). This is detected and the
/// substitution REFUSES (`ExportRefusal::OutOfFragment`, capture-unsafe) → the item
/// is NOT tier-(b)-unfoldable and the engine SKIPs it to the tier-(c) interactive
/// path (an honest skip, NEVER a silent capture, NEVER a wrong-program proof). The
/// shadowing direction (where NO live arg uses the binder name) is still handled by
/// removing the shadowed key — that is sound, not a capture.
fn substitute(e: &Expr, subst: &BTreeMap<String, Expr>) -> Result<Expr, ExportRefusal> {
    Ok(match e {
        Expr::Path(segs) if segs.len() == 1 => {
            subst.get(&segs[0]).cloned().unwrap_or_else(|| e.clone())
        }
        Expr::Call { callee, args } => Expr::Call {
            callee: Box::new(substitute(callee, subst)?),
            args: args
                .iter()
                .map(|a| substitute(a, subst))
                .collect::<Result<_, _>>()?,
        },
        Expr::Binary { op, lhs, rhs } => Expr::Binary {
            op: *op,
            lhs: Box::new(substitute(lhs, subst)?),
            rhs: Box::new(substitute(rhs, subst)?),
        },
        Expr::Unary { op, expr } => Expr::Unary {
            op: *op,
            expr: Box::new(substitute(expr, subst)?),
        },
        Expr::Cast { expr, ty } => Expr::Cast {
            expr: Box::new(substitute(expr, subst)?),
            ty: ty.clone(),
        },
        Expr::Ref { mutable, expr } => Expr::Ref {
            mutable: *mutable,
            expr: Box::new(substitute(expr, subst)?),
        },
        Expr::MethodCall {
            receiver,
            name,
            args,
        } => Expr::MethodCall {
            receiver: Box::new(substitute(receiver, subst)?),
            name: name.clone(),
            args: args
                .iter()
                .map(|a| substitute(a, subst))
                .collect::<Result<_, _>>()?,
        },
        Expr::Index { base, index } => Expr::Index {
            base: Box::new(substitute(base, subst)?),
            index: match index {
                IndexArg::Single(i) => IndexArg::Single(Box::new(substitute(i, subst)?)),
                IndexArg::RangeTo(i) => IndexArg::RangeTo(Box::new(substitute(i, subst)?)),
                IndexArg::RangeFrom(i) => IndexArg::RangeFrom(Box::new(substitute(i, subst)?)),
                IndexArg::Range(a, b) => IndexArg::Range(
                    Box::new(substitute(a, subst)?),
                    Box::new(substitute(b, subst)?),
                ),
            },
        },
        Expr::Match { scrutinee, arms } => {
            let scrut = Box::new(substitute(scrutinee, subst)?);
            let mut new_arms = Vec::with_capacity(arms.len());
            for a in arms {
                // The arm payload binder SHADOWS a param of the same name.
                let mut inner = subst.clone();
                if let Pattern::Enum { fields: fs, .. } = &a.pattern {
                    if let [Pattern::Binding(b)] = fs.as_slice() {
                        check_no_capture(b, &inner)?;
                        inner.remove(b);
                    }
                }
                new_arms.push(thermite_syntax::MatchArm {
                    pattern: a.pattern.clone(),
                    guard: a.guard.clone(),
                    body: substitute(&a.body, &inner)?,
                });
            }
            Expr::Match {
                scrutinee: scrut,
                arms: new_arms,
            }
        }
        Expr::Is { scrutinee, variant } => Expr::Is {
            scrutinee: Box::new(substitute(scrutinee, subst)?),
            variant: variant.clone(),
        },
        Expr::Closure { params, body } => {
            // The closure element var SHADOWS a param of the same name.
            let mut inner = subst.clone();
            for p in params {
                check_no_capture(p, &inner)?;
                inner.remove(p);
            }
            Expr::Closure {
                params: params.clone(),
                body: Box::new(substitute(body, &inner)?),
            }
        }
        other => other.clone(),
    })
}

/// CAPTURE GUARD (the Pin I fix, §6.1(b)): a body binder `binder` may safely shadow
/// a param key in `subst` ONLY if NO still-live substituted argument carries
/// `binder` as a free name — otherwise removing the key would let the binder CAPTURE
/// that caller var. On a collision the tier-(b) unfolding REFUSES (capture-unsafe).
fn check_no_capture(binder: &str, subst: &BTreeMap<String, Expr>) -> Result<(), ExportRefusal> {
    if subst_free_names(subst).contains(binder) {
        return Err(ExportRefusal::OutOfFragment(format!(
            "tier-(b) static unfolding would CAPTURE caller variable `{binder}` under a \
             body binder (§6.1(b) capture-unsafe substitution); refusing tier (b) — the \
             item routes to the tier-(c) interactive path (an honest skip, never a silent \
             capture / wrong-program proof)"
        )));
    }
    Ok(())
}

/// Does an `Expr` contain a `specCall` reachable to an in-program spec-fn? (The §6.1
/// `specCallFree` test, over the SAME positions the closure walks.) A combinator
/// callee (`forall_in`/…) is NOT a spec-call; `old(_)` is not; only a named in-program
/// spec-fn is.
fn expr_has_spec_call(e: &Expr, decls: &BTreeMap<String, SpecFnItem>) -> bool {
    let mut out = Vec::new();
    collect_expr_calls(e, decls, &mut out);
    !out.is_empty()
}

/// Classify the export TIER (`.design/verified/proof-backends.md` §6.1). Tier (a) if
/// `req`/`ens`/body/dec are ALL specCall-free; else tier (b) if the registry is
/// NON-recursive (a finite DAG); else tier (c) (recursive).
fn tier_of(
    req: Option<&Expr>,
    ens: &[Expr],
    body: &Expr,
    dec: Option<&Expr>,
    called: &[String],
    decls: &BTreeMap<String, SpecFnItem>,
) -> ExportTier {
    let any_spec_call = req.is_some_and(|e| expr_has_spec_call(e, decls))
        || ens.iter().any(|e| expr_has_spec_call(e, decls))
        || expr_has_spec_call(body, decls)
        || dec.is_some_and(|e| expr_has_spec_call(e, decls));
    if !any_spec_call {
        ExportTier::FuelFreeAuto
    } else if registry_is_recursive(called, decls) {
        ExportTier::RecursiveInteractive
    } else {
        ExportTier::StaticUnfoldAuto
    }
}

/// The AUTO tactic battery (`.design/verified/proof-backends.md` §6.1(a) / REQ-7) —
/// `first | decide | (intros; simp_all; omega) | …`. Tried in order so a closed-form
/// QF goal is `decide`d, a linear-arith goal falls to `simp_all; omega` (the
/// z3-demotion battery), etc. Emitted as the proof of the tier-(a)/(b) theorem.
fn auto_tactic_battery() -> &'static str {
    "  intro hreq\n  \
     simp only [Thermite.Env.bindInt, Thermite.intVal, Thermite.denote, Thermite.arithDenote, \
     Thermite.castDenote, Thermite.seqIdx, Thermite.seqSub, Thermite.scrutVal, \
     Thermite.OptResVal.isVariant, Thermite.OptResVal.variant] at hreq ⊢\n  \
     first\n    \
     | decide\n    \
     | omega\n    \
     | simp_all\n    \
     | exact hreq\n    \
     | (revert hreq; decide)\n    \
     | (revert hreq; omega)"
}

/// Emit the obligation THEOREM (`.design/verified/proof-backends.md` §4/§6.1). Tier
/// (a)/(b): the FUEL-FREE `∀ v, denote 0 req env → denote 0 ens (bindInt env "result"
/// rbody)` goal (sound via `stabilizes_iff_intVal_zero` /
/// `stabilizesProp_iff_denote_zero`, cited), discharged by the auto battery. Tier
/// (c): the §4 `∀ r, stabilizes body env r → stabilizesProp req env → stabilizesProp
/// ens (bindInt env "result" r)` stabilized form, marked INTERACTIVE (a skeleton the
/// engine does NOT lake-check).
fn emit_theorem(
    thm_name: &str,
    req: Option<&Expr>,
    ens: &[Expr],
    body: &Expr,
    tier: ExportTier,
    ctx: &EncodeCtx,
) -> Result<String, ExportRefusal> {
    let req_term = match req {
        Some(r) => encode_expr(r, ctx)?,
        None => "(Thermite.Expr.boolLit true)".to_string(),
    };
    let ens_terms = ens
        .iter()
        .map(|e| encode_expr(e, ctx))
        .collect::<Result<Vec<_>, _>>()?;
    let ens_term = conjoin(&ens_terms);
    let body_term = encode_expr(body, ctx)?;

    match tier {
        ExportTier::FuelFreeAuto | ExportTier::StaticUnfoldAuto => {
            // FUEL-FREE shallow goal (§6.1(a)/(b)): `denote 0` / `intVal 0`. The
            // `stabilizes_iff_intVal_zero` / `stabilizesProp_iff_denote_zero`
            // corollaries (`Stabilize.lean`) prove this EQUIVALENT to the §4
            // stabilized form for a specCall-free `e` (tier a) or a statically-
            // unfolded one (tier b) — cited so the EXP inspector sees the bridge.
            Ok(format!(
                "/-- {thm_name}: the FUEL-FREE tier-({}) obligation (§6.1).\n    \
                 Sound via `Thermite.stabilizes_iff_intVal_zero` /\n    \
                 `Thermite.stabilizesProp_iff_denote_zero` (Stabilize.lean): for a\n    \
                 specCall-free / statically-unfolded `e`, `stabilizesProp e env ↔\n    \
                 denote 0 e env`, so the fuel-free goal is equivalent to the §4\n    \
                 stabilized form but is a SHALLOW QF goal the auto battery chews. -/\n\
                 theorem {thm_name} (v : Thermite.Env) :\n    \
                 Thermite.denote 0 {req_term} {{ v with specs := R_item }} →\n    \
                 Thermite.denote 0 {ens_term}\n      \
                 ((({{ v with specs := R_item }} : Thermite.Env)).bindInt \"result\"\n        \
                 (Thermite.intVal 0 {body_term} {{ v with specs := R_item }})) := by\n\
                 {}",
                tier.tag(),
                auto_tactic_battery()
            ))
        }
        ExportTier::RecursiveInteractive => {
            // The §4 STABILIZED form (interactive only): `∀ r, stabilizes body env r
            // → reqStable → ensStable@r`. The result is BOUND THROUGH `stabilizes`
            // (the #214 fix — NO concrete export-time value; uniqueness of
            // stabilization forces `r` to the body's true stabilized value). The
            // engine does NOT lake-check this (RecursiveInteractive); it is emitted
            // for increment-(iii) interactive use, marked as a skeleton.
            Ok(format!(
                "/-- {thm_name}: the §4 STABILIZED form (tier (c), INTERACTIVE only).\n    \
                 The result `r` is BOUND THROUGH `Thermite.stabilizes body env r` (the\n    \
                 #214 fix — no concrete export-time value; uniqueness of stabilization\n    \
                 (`Thermite.stabilizes_unique`) forces `r` to the body's true stabilized\n    \
                 value). Sound for a DEC-VALID registry by\n    \
                 `Thermite.stabilization_exists` (the REGISTRY-TERMINATION class\n    \
                 discharges the `Thermite.RegistryTerminating` hypothesis). A recursive\n    \
                 registry's per-env `∃N` witness needs INDUCTION — reserved for the\n    \
                 interactive path (REQ-7(ii)); the auto battery does NOT attempt it. -/\n\
                 theorem {thm_name} (v : Thermite.Env) (r : Int) :\n    \
                 Thermite.stabilizes {body_term} {{ v with specs := R_item }} r →\n    \
                 Thermite.stabilizesProp {req_term} {{ v with specs := R_item }} →\n    \
                 Thermite.stabilizesProp {ens_term}\n      \
                 ((({{ v with specs := R_item }} : Thermite.Env)).bindInt \"result\" r) := by\n  \
                 sorry  -- INTERACTIVE: a human/agent authors the induction (REQ-7(ii))"
            ))
        }
    }
}

/// Conjoin a list of encoded ens terms into a single `Expr` (`a ∧ b ∧ …`). One ens
/// is itself; several fold with `Expr.logic LogOp.and`. An empty list (no ens — the
/// parser never produces this) is `boolLit true`.
fn conjoin(terms: &[String]) -> String {
    match terms {
        [] => "(Thermite.Expr.boolLit true)".to_string(),
        [single] => single.clone(),
        [first, rest @ ..] => {
            let tail = conjoin(rest);
            format!("(Thermite.Expr.logic Thermite.LogOp.and {first} {tail})")
        }
    }
}

/// Export a checked PURE-CONTRACT item to a self-contained Lean file
/// (`.design/verified/proof-backends.md` REQ-6/REQ-7 — the top-level exporter).
///
/// `obligation` is the backend-neutral [`Obligation`] (its `env.spec_defs` is the
/// full-expression-position called-spec-fn closure — the ONE closure, #192);
/// `program` is the parsed source (for the spec-fn definitions `R_item` is populated
/// from + the recursive-registry detection); `item` is the source item (for the body
/// + the dec measure + the type-sorted params).
///
/// Returns the [`ExportedObligation`] (Lean source + tier + registry names) or a
/// structured [`ExportRefusal`] (an OUT-of-fragment construct, a non-pure-contract
/// body, the HARD-GATE incomplete-registry refusal, or an open hole). NEVER a panic.
pub fn export_item(
    obligation: &Obligation,
    program: &Program,
    item: &Item,
) -> Result<ExportedObligation, ExportRefusal> {
    let decls = spec_decls(program);

    // The pure-contract body, req, ens, dec — sorted by item kind.
    let (req, ens, body, dec, params): (
        Option<Expr>,
        Vec<Expr>,
        Expr,
        Option<Expr>,
        Vec<thermite_syntax::Param>,
    ) = match item {
        Item::Fn(f) => {
            if !f.holes.is_empty() {
                return Err(ExportRefusal::OpenHole(format!(
                    "fn `{}` carries {} open hole(s)",
                    f.name,
                    f.holes.len()
                )));
            }
            let body_block = f.body.as_ref().ok_or_else(|| {
                ExportRefusal::NotPureContract(format!(
                    "fn `{}` is a boundary fn (foreign body, no in-language body)",
                    f.name
                ))
            })?;
            let body = pure_tail_of_block(body_block)
                .ok_or_else(|| {
                    ExportRefusal::NotPureContract(format!(
                        "fn `{}` body is not a single pure tail expression (the §4.1 \
                         exec-body bridge is increment (iv))",
                        f.name
                    ))
                })?
                .clone();
            // THE RESULT-SORT GATE (§4 SCOPE / Pin H): the pure-contract class is
            // intVal-denoting bodies (result `r : Int`). A `-> bool`/unit/ADT result
            // bottoms to `0` in `intVal` (a contract AND its negation both certify) —
            // refuse (the increment-(iv) bindBool bridge).
            if !result_is_int_sorted(&f.ret) {
                return Err(ExportRefusal::NonIntResult(format!(
                    "fn `{}` returns {:?}",
                    f.name, f.ret
                )));
            }
            (
                Some(f.contract.req.expr.clone()),
                f.contract.ens.iter().map(|c| c.expr.clone()).collect(),
                body,
                f.dec.as_ref().map(|c| c.expr.clone()),
                f.params.clone(),
            )
        }
        Item::SpecFn(s) => {
            let body = pure_tail_of_block(&s.body)
                .ok_or_else(|| {
                    ExportRefusal::NotPureContract(format!(
                        "spec fn `{}` body is not a single pure tail expression",
                        s.name
                    ))
                })?
                .clone();
            // THE RESULT-SORT GATE (§4 SCOPE / Pin H): a spec fn's body must denote
            // in `intVal` (the degenerate ens `result == body` binds `result : Int`).
            // A `-> bool`/ADT spec fn would bottom to `0` — refuse.
            if !result_is_int_sorted(&s.ret) {
                return Err(ExportRefusal::NonIntResult(format!(
                    "spec fn `{}` returns {:?}",
                    s.name, s.ret
                )));
            }
            // A spec fn has no req/ens; its certification obligation is its body
            // characterization. We export a degenerate ens `result == body` so the
            // same theorem shape applies (the body's stabilized value IS the result).
            let ens = vec![Expr::Binary {
                op: BinOp::Eq,
                lhs: Box::new(Expr::Path(vec!["result".to_string()])),
                rhs: Box::new(body.clone()),
            }];
            (None, ens, body, Some(s.dec.expr.clone()), s.params.clone())
        }
        Item::Struct(_) | Item::Enum(_) => {
            return Err(ExportRefusal::OutOfFragment(
                "ADT item (no in-language certification obligation in v1)".to_string(),
            ))
        }
    };

    // The env coercion frame (sorts free names).
    let ctx = ctx_for_params(&params);

    // THE HARD GATE (§4 mechanism 1), in TWO independent directions, BEFORE any
    // encoding:
    //
    // (i) every spec-call ACTUALLY appearing in `req ∪ ens ∪ body ∪ dec` must be in
    //     the obligation's `called` closure AND must RESOLVE to a definition. This
    //     catches a BUGGY/omitting closure (the Pin B/C/E/F bottom-poisoning: an
    //     omitted body- or measure-called spec-fn would bottom to the Int-`0` and
    //     self-certify) AND an UNDEFINED callee (the Pin G `mystery(x)`: a callee with
    //     NO in-program definition). The collection here is UNFILTERED
    //     (`collect_all_call_names`, NOT `collect_expr_calls`) — the defined-names
    //     filter was the shared blind spot that made `mystery` invisible to the gate
    //     (§4 full-expression-position principle: EVERY spec-call position the export
    //     denotes against `R_item` contributes its name, defined or not). The exporter
    //     does NOT trust the closure blindly — it RE-CHECKS coverage against the exprs
    //     it is about to denote (the #192 lesson is "ONE closure builds `R_item`", not
    //     "skip the coverage check").
    // (ii) every name in `called` must resolve to a definition (`build_registry`'s
    //      `calledSpecFns ⊆ dom(R_item)` check).
    let called = &obligation.env.spec_defs;
    let mut direct: Vec<String> = Vec::new();
    if let Some(r) = &req {
        collect_all_call_names(r, &mut direct);
    }
    for e in &ens {
        collect_all_call_names(e, &mut direct);
    }
    collect_all_call_names(&body, &mut direct);
    if let Some(d) = &dec {
        collect_all_call_names(d, &mut direct);
    }
    let mut present: std::collections::BTreeSet<String> = direct.into_iter().collect();
    // Close over each reached DEFINED spec-fn's body+dec (the transitive set the
    // measure/body-position calls reach — the #226 full-expression-position closure).
    // The closure walk is UNFILTERED too, so a transitively-reached UNDEFINED callee
    // is also a present name the resolution check below catches.
    let mut worklist: Vec<String> = present.iter().cloned().collect();
    while let Some(n) = worklist.pop() {
        if let Some(d) = decls.get(&n) {
            let mut sub = Vec::new();
            collect_all_block_call_names(&d.body, &mut sub);
            collect_all_call_names(&d.dec.expr, &mut sub);
            for c in sub {
                if present.insert(c.clone()) {
                    worklist.push(c);
                }
            }
        }
    }
    // An UNDEFINED present name (no in-program definition — the Pin G `mystery`) → the
    // hard-gate refusal (`calledSpecFns ⊄ dom(R_item)`): its emitted `specCall` would
    // bottom to `0` and self-certify.
    let undefined: Vec<String> = present
        .iter()
        .filter(|n| !decls.contains_key(*n))
        .cloned()
        .collect();
    if !undefined.is_empty() {
        return Err(ExportRefusal::IncompleteRegistry(undefined));
    }
    // A DEFINED present name OMITTED from the closure → refuse (the Pin B/C/E/F
    // mirror: the bottom-poisoning of a reachable-but-unregistered call).
    let omitted: Vec<String> = present
        .iter()
        .filter(|n| !called.iter().any(|c| c == *n))
        .cloned()
        .collect();
    if !omitted.is_empty() {
        return Err(ExportRefusal::IncompleteRegistry(omitted));
    }
    let (registry_block, registry_names) = build_registry(called, &decls)?;

    // The tier (registry shape — §6.1).
    let tier = tier_of(req.as_ref(), &ens, &body, dec.as_ref(), called, &decls);

    // For tier (b) (STATIC-UNFOLD AUTO) the exporter UNFOLDS every spec-call to its
    // finite DAG depth, producing specCall-FREE exprs the fuel-free tier-(a) form is
    // sound for (§6.1(b)). Tier (a) is already specCall-free; tier (c) keeps the
    // calls (the `∃N∀fuel` interactive form denotes against `R_item`).
    // The unfolding is CAPTURE-SAFE (Pin I): a substitution that would capture a
    // caller var under a body binder REFUSES tier (b) here (`?` propagates the
    // `ExportRefusal::OutOfFragment` — an honest skip to the tier-(c) interactive
    // path, never a silent capture / wrong-program proof).
    let (req_e, ens_e, body_e): (Option<Expr>, Vec<Expr>, Expr) =
        if tier == ExportTier::StaticUnfoldAuto {
            (
                match &req {
                    Some(r) => Some(unfold_spec_calls(r, &decls)?),
                    None => None,
                },
                ens.iter()
                    .map(|e| unfold_spec_calls(e, &decls))
                    .collect::<Result<_, _>>()?,
                unfold_spec_calls(&body, &decls)?,
            )
        } else {
            (req.clone(), ens.clone(), body.clone())
        };

    // Encode ALL exprs FIRST (so an OUT-of-fragment construct refuses before we emit
    // the file — an honest skip, never a partial file). For tier (b) the unfolded
    // exprs are encoded (specCall-free); the theorem below uses the SAME unfolded
    // exprs, so the emitted goal is the fuel-free shallow shape.
    if let Some(r) = &req_e {
        encode_expr(r, &ctx)?;
    }
    for e in &ens_e {
        encode_expr(e, &ctx)?;
    }
    encode_expr(&body_e, &ctx)?;
    if let Some(d) = &dec {
        encode_expr(d, &ctx)?;
    }

    // The theorem (over the per-tier exprs: unfolded for tier (b), as-is otherwise).
    let thm_name = format!("thermite_obligation_{}", sanitize(&obligation.item));
    let theorem = emit_theorem(&thm_name, req_e.as_ref(), &ens_e, &body_e, tier, &ctx)?;

    let source = format!(
        "/- AUTO-GENERATED by `forge` (lean_export.rs) — the Thermite→Lean obligation\n   \
         exporter (proof-backends.md REQ-6/REQ-7). Item: `{item}`, tier: {tier}.\n   \
         Instantiates the kernel-proven spine (`lean/Thermite/`); do NOT edit by hand. -/\n\
         import Thermite.Stabilize\n\n\
         {registry_block}\n\n\
         {theorem}\n",
        item = obligation.item,
        tier = tier.tag(),
    );

    // THE INDEPENDENT RE-CHECK (§4 mechanism 1, the Pin G blind-spot fix): walk the
    // EMITTED Lean THEOREM TERMS for `Expr.specCall "NAME"` occurrences and demand
    // each `NAME ∈ dom(R_item)` (= `registry_names`). This is GENUINELY independent of
    // the source-side gate above — it inspects the bytes actually handed to the
    // kernel, NOT the Thermite AST — so a future encoder bug that emitted a `specCall`
    // for a name the gate never saw (the shared `decls.contains_key` blind spot the
    // critic showed) is caught here, NEVER kernel-accepted with an unresolved bottom.
    // Only the theorem is scanned (the `R_item` def legitimately matches names in its
    // own body); a residual `specCall` in the goal whose name is not registered means
    // the registry does NOT cover the denoted term → refuse.
    for name in emitted_spec_call_names(&theorem) {
        if !registry_names.iter().any(|r| r == &name) {
            return Err(ExportRefusal::IncompleteRegistry(vec![name]));
        }
    }

    Ok(ExportedObligation {
        source,
        tier,
        registry_names,
    })
}

/// Scan an emitted Lean term string for `Thermite.Expr.specCall "NAME"` occurrences
/// and return the `NAME`s (the GENUINELY-INDEPENDENT re-check support, Pin G). This
/// inspects the RENDERED bytes the kernel sees — NOT the Thermite AST — so it has no
/// shared blind spot with the AST-side gate. Deterministic (R-CODE-5).
fn emitted_spec_call_names(term: &str) -> Vec<String> {
    const MARKER: &str = "Thermite.Expr.specCall \"";
    let mut out = Vec::new();
    let mut rest = term;
    while let Some(pos) = rest.find(MARKER) {
        let after = &rest[pos + MARKER.len()..];
        if let Some(end) = after.find('"') {
            out.push(after[..end].to_string());
            rest = &after[end + 1..];
        } else {
            break;
        }
    }
    out
}

/// A Lean-identifier-safe form of an item name (the theorem name). Replaces any
/// non-alphanumeric/underscore char with `_` (deterministic, R-CODE-5).
fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Look up an item by name in a program (the engine resolves the source item for an
/// [`Obligation`] before exporting). `None` if absent.
#[must_use]
pub fn find_item<'a>(program: &'a Program, name: &str) -> Option<&'a Item> {
    program.items.iter().find(|i| i.name() == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_one(src: &str) -> Program {
        let parsed = thermite_syntax::parse(src);
        assert!(parsed.is_clean(), "fixture must parse: {:?}", parsed.errors);
        parsed.program
    }

    // REQ-6 EXP: the arm-by-arm encoder maps a scalar contract clause to its
    // `Ast.lean` constructor. Expected from the design's §4 encoding (R-CHAR-3) —
    // `result >= a` is `Expr.cmp CmpOp.ge (var "result") (var "a")`.
    #[test]
    fn encode_scalar_comparison_arm_by_arm() {
        let p = parse_one("fn max2(a: u32, b: u32) -> u32 req true ens result >= a fx pure { a }");
        let f = match find_item(&p, "max2").unwrap() {
            Item::Fn(f) => f,
            _ => unreachable!(),
        };
        let ctx = ctx_for_params(&f.params);
        let ens = &f.contract.ens[0].expr;
        let encoded = encode_expr(ens, &ctx).expect("scalar comparison encodes");
        assert!(
            encoded.contains("Thermite.Expr.cmp Thermite.CmpOp.ge"),
            "the >= comparison maps to CmpOp.ge: {encoded}"
        );
        assert!(encoded.contains("Thermite.Expr.var \"result\""));
        assert!(encoded.contains("Thermite.Expr.var \"a\""));
    }

    // REQ-6 §4: an out-of-fragment construct (a tuple projection) REFUSES — never a
    // silent omission. Expected from the §4 OUT-of-spine refusal rule (R-CHAR-3).
    #[test]
    fn out_of_fragment_field_refuses() {
        let proj = Expr::TupleProj {
            receiver: Box::new(Expr::Path(vec!["p".to_string()])),
            index: 0,
        };
        let r = encode_expr(&proj, &EncodeCtx::default());
        assert!(matches!(r, Err(ExportRefusal::OutOfFragment(_))), "{r:?}");
    }

    // REQ-6 §4 HARD GATE: an incomplete registry (a reachable name with no
    // definition) REFUSES with `IncompleteRegistry`. Expected from the §4 gate
    // mechanism 1 (R-CHAR-3).
    #[test]
    fn hard_gate_refuses_incomplete_registry() {
        let decls = BTreeMap::new(); // no definitions
        let r = build_registry(&["spec_sum".to_string()], &decls);
        match r {
            Err(ExportRefusal::IncompleteRegistry(names)) => {
                assert_eq!(names, vec!["spec_sum".to_string()])
            }
            other => panic!("expected IncompleteRegistry, got {other:?}"),
        }
    }

    // REQ-7 §6.1: a specCall-free item is tier (a) (FuelFreeAuto). Expected from the
    // §6.1(a) classification (R-CHAR-3).
    #[test]
    fn spec_call_free_is_tier_a() {
        let p = parse_one("fn id(x: u64) -> u64 req true ens result == x fx pure { x }");
        let f = match find_item(&p, "id").unwrap() {
            Item::Fn(f) => f,
            _ => unreachable!(),
        };
        let decls = spec_decls(&p);
        let tier = tier_of(
            Some(&f.contract.req.expr),
            &[f.contract.ens[0].expr.clone()],
            f.body.as_ref().unwrap().tail.as_deref().unwrap(),
            None,
            &[],
            &decls,
        );
        assert_eq!(tier, ExportTier::FuelFreeAuto);
    }

    // REQ-7 §6.1: a NON-recursive registry is tier (b); a recursive one is tier (c).
    // Expected from the §6.1(b)/(c) classification (R-CHAR-3).
    #[test]
    fn recursive_registry_detection() {
        // Non-recursive: g calls nothing.
        let p = parse_one(
            "spec fn g(x: int) -> int dec x { x } \
             fn f(x: u64) -> u64 req true ens result == g(x as int) as u64 fx pure { x }",
        );
        let decls = spec_decls(&p);
        assert!(
            !registry_is_recursive(&["g".to_string()], &decls),
            "g is non-recursive (a DAG)"
        );
        // Recursive: r calls itself.
        let p2 = parse_one(
            "spec fn r(x: int) -> int dec x { r(x) } \
             fn f(x: u64) -> u64 req true ens result == r(x as int) as u64 fx pure { x }",
        );
        let decls2 = spec_decls(&p2);
        assert!(
            registry_is_recursive(&["r".to_string()], &decls2),
            "r is recursive (a self-call cycle)"
        );
    }

    // Build a CONTRACT obligation for a named fn, asserting it is present + a fn (no
    // unwrap/panic — the anti-pattern gate is clean). The default obligation is only
    // returned on the already-failed assert path.
    fn fn_obl(p: &Program, name: &str, called: Vec<String>) -> Obligation {
        let item = find_item(p, name);
        assert!(
            matches!(item, Some(Item::Fn(_))),
            "fn `{name}` must be present, got {item:?}"
        );
        if let Some(Item::Fn(f)) = item {
            Obligation::contract_for_fn(f, called)
        } else {
            // Reached only after the assert above has failed the test; a degenerate
            // obligation keeps the helper total without an unwrap/panic.
            Obligation {
                item: name.to_string(),
                class: crate::obligation::ObligationClass::Contract,
                role: crate::obligation::ObligationRole::Certification,
                ast_slice: crate::obligation::AstSlice::Block(Box::new(Block {
                    stmts: Vec::new(),
                    tail: None,
                })),
                env: crate::obligation::ObligationEnv::default(),
            }
        }
    }

    // #243 / Pin G — THE UNDEFINED-CALLEE HARD GATE: a contract spec-call whose
    // callee has NO in-program definition (`mystery`) must REFUSE the export with
    // `IncompleteRegistry(["mystery"])`, NOT export an empty registry that bottoms to
    // 0 and self-certifies. Expected from §4 mechanism 1 (full-expression-position
    // principle; R-CHAR-3) — the live repro the pin's `thermite_obligation_f` pins.
    #[test]
    fn undefined_callee_refuses_export() {
        let p = parse_one(
            "fn f(x: u64) -> u64 req true ens result == mystery(x as int) as u64 fx pure { 0 }",
        );
        // The closure is EMPTY (mystery is undefined, so no closure could list it).
        let o = fn_obl(&p, "f", vec![]);
        if let Some(item) = find_item(&p, "f") {
            match export_item(&o, &p, item) {
                Err(ExportRefusal::IncompleteRegistry(names)) => assert!(
                    names.contains(&"mystery".to_string()),
                    "the undefined callee is named in the refusal: {names:?}"
                ),
                other => assert!(
                    matches!(other, Err(ExportRefusal::IncompleteRegistry(_))),
                    "an undefined callee must REFUSE (IncompleteRegistry): {other:?}"
                ),
            }
        }
    }

    // #244 / Pin H — THE RESULT-SORT GATE: a `-> bool` item must REFUSE with
    // `NonIntResult` (its body bottoms to 0 in intVal, so a contract AND its negation
    // both certify). Expected from §4 SCOPE / §4.1 (R-CHAR-3) — the pin's
    // `thermite_obligation_always_ens_{true,false}` both-prove degeneracy.
    #[test]
    fn bool_result_item_refuses_export() {
        let p =
            parse_one("fn always(a: u32) -> bool req true ens result == true fx pure { false }");
        let o = fn_obl(&p, "always", vec![]);
        if let Some(item) = find_item(&p, "always") {
            assert!(
                matches!(
                    export_item(&o, &p, item),
                    Err(ExportRefusal::NonIntResult(_))
                ),
                "a bool-returning item must REFUSE (NonIntResult)"
            );
        }
        // The negated contract on the SAME bool item ALSO refuses (the pin's both-prove
        // degeneracy is walled off at export for both polarities).
        let p2 =
            parse_one("fn always(a: u32) -> bool req true ens result == false fx pure { false }");
        let o2 = fn_obl(&p2, "always", vec![]);
        if let Some(item2) = find_item(&p2, "always") {
            assert!(
                matches!(
                    export_item(&o2, &p2, item2),
                    Err(ExportRefusal::NonIntResult(_))
                ),
                "the negated bool contract also refuses"
            );
        }
    }

    // #245 / Pin I — CAPTURE-SAFE UNFOLDING: a tier-(b) item whose unfolding would
    // CAPTURE a caller var under a predicate binder (`cntk(xs, k as int)` with
    // `|k| … == v`) must REFUSE tier (b) (`OutOfFragment`, capture-unsafe), NEVER
    // silently emit the captured tautology. Expected from §6.1(b) (R-CHAR-3) — the
    // pin's `capture_changes_meaning`.
    #[test]
    fn capture_unsafe_unfolding_refuses() {
        let p = parse_one(
            "spec fn cntk(xs: &[u32], v: int) -> int dec xs.len() \
               { count_where(xs, |k| k as int == v) } \
             spec fn cntall(xs: &[u32]) -> int dec xs.len() \
               { count_where(xs, |k| k as int == k as int) } \
             fn f3(xs: &[u32], k: u32) -> u64 req true \
               ens cntk(xs, k as int) == cntall(xs) fx pure { 0 }",
        );
        let o = fn_obl(&p, "f3", vec!["cntk".to_string(), "cntall".to_string()]);
        if let Some(item) = find_item(&p, "f3") {
            match export_item(&o, &p, item) {
                Err(ExportRefusal::OutOfFragment(d)) => assert!(
                    d.contains("CAPTURE") && d.contains('k'),
                    "the capture-unsafe refusal names the captured binder: {d}"
                ),
                other => assert!(
                    matches!(other, Err(ExportRefusal::OutOfFragment(_))),
                    "a capture-unsafe unfolding must REFUSE tier (b): {other:?}"
                ),
            }
        }
    }

    // #245 OVER-REFUSAL GUARD: a tier-(b) item whose unfolding does NOT capture
    // (the arg's free vars are disjoint from the body binder) still UNFOLDS + exports
    // (the capture guard is sound, not a blanket refusal). `dbl(x) = x + x` has no
    // binder; substituting `x ↦ (y as int)` cannot capture.
    #[test]
    fn non_capturing_unfolding_still_exports() {
        let p = parse_one(
            "spec fn dbl(x: int) -> int dec x { x + x } \
             fn g(y: u32) -> u32 req y < 100 ens result as int == dbl(y as int) fx pure { y + y }",
        );
        let o = fn_obl(&p, "g", vec!["dbl".to_string()]);
        if let Some(item) = find_item(&p, "g") {
            match export_item(&o, &p, item) {
                Ok(exported) => {
                    assert_eq!(exported.tier, ExportTier::StaticUnfoldAuto);
                    // The unfolded goal is specCall-free (dbl unfolded away) — the
                    // independent re-check passes (no residual specCall).
                    assert!(!exported.source.contains("Expr.specCall"));
                }
                other => assert!(
                    other.is_ok(),
                    "non-capturing tier-(b) still exports: {other:?}"
                ),
            }
        }
    }

    // REQ-6: a full pure-contract export of a scalar item produces a self-contained
    // file importing the spine, with the fuel-free theorem. Expected shape from §4/§6.1.
    #[test]
    fn full_export_scalar_item_is_self_contained() {
        let p = parse_one(
            "fn max2(a: u32, b: u32) -> u32 req true ens result >= a && result >= b fx pure { a }",
        );
        let item = find_item(&p, "max2").unwrap();
        let f = match item {
            Item::Fn(f) => f,
            _ => unreachable!(),
        };
        let o = Obligation::contract_for_fn(f, vec![]);
        let exported = export_item(&o, &p, item).expect("scalar item exports");
        assert_eq!(exported.tier, ExportTier::FuelFreeAuto);
        assert!(exported.source.contains("import Thermite.Stabilize"));
        assert!(exported.source.contains("def R_item"));
        assert!(exported.source.contains("theorem thermite_obligation_max2"));
        assert!(exported.source.contains("Thermite.denote 0"));
        // No spec-fn deps → no per-name decide lemmas.
        assert!(!exported.source.contains("≠ none := by decide"));
    }
}
