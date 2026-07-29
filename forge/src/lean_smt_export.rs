//! Automated Rust→Lean export for per-clause translation-validation obligations
//! (`.design/stage3-bv-reconstruction.md` REQ-7 / AC-8).
//!
//! This module closes the Tier-3 hand-translation gap that
//! `lean/Thermite/SmtDemo.lean` left open ("the hand-translation step is the gap an
//! automated Rust→Lean exporter would close"). Given a per-clause equivalence
//! obligation — the translation-validation shape `(P_production) ⟺ (P_reference)`
//! that `thermite-tv/src/obligation.rs::equivalence_obligation` asserts — it renders
//! both predicate ASTs into Lean `Prop`s over a typed environment and emits a
//! self-contained theorem followed by a `#print axioms` probe. QF_LIA uses the
//! lean-smt `smt` tactic (pinned @ `ee6d36b`, with cvc5 over FFI). QF_BV is rendered
//! as literal `BitVec N` expressions and its normalization equivalence is proved with
//! kernel-checked library lemmas.
//! A theorem whose `#print axioms` stays within `{propext, Classical.choice,
//! Quot.sound}` (no `Smt`-internal oracle, no `sorryAx`, no `Lean.ofReduceBool`) is
//! a kernel-checked reconstruction — that axiom-cleanliness is what makes a fragment
//! count as *reconstruction-supported* (the signal REQ-8's trust migration consumes).
//!
//! ## The two fragments (REQ-7)
//!
//! - **QF_LIA scalar** ([`SmtFragment::Lia`]): comparisons + boolean connectives +
//!   linear integer arithmetic over `Int`. This is the PoC-proven shape
//!   (`SmtDemo.lean`'s Tier-2/Tier-3 theorems), the contract sublanguage
//!   `gen_comparison`/`gen_int` space. Rendered directly over Lean `Int`.
//! - **QF_BV** ([`SmtFragment::Bv`]): the full fixed-width term fragment used by
//!   [`crate::bitvector::BitVectorEngine`] (REQ-2), rendered directly over Lean
//!   `BitVec N`.
//!
//! ## Literal QF_BV reconstruction
//!
//! The exported theorem is an equivalence between the production predicate and
//! [`reference_normalize`]. That normalization uses only total-order duals,
//! `≠` expansion, and commutativity of addition and multiplication. Lean proves
//! those facts directly with `simp`, including when they occur below bitwise, shift,
//! division, or remainder operations. This covers the complete QF_BV term surface
//! without asking lean-smt to reconstruct cvc5's bit-blasting proof and without adding
//! `bv_decide`'s native-reflection axiom.
//!
//! The Rust-emitter ↔ Lean-syntax correspondence remains inspection-tier, as described
//! in `.design/verified/exporter-surface-correspondence.md`.

use std::collections::BTreeSet;

use thermite_syntax::{BinOp, BvWidth, Expr, Item, Program, UnaryOp};

/// The SMT fragment a per-clause obligation is exported into
/// (`.design/stage3-bv-reconstruction.md` REQ-7). The fragment fixes both the Lean
/// sort the free variables are rendered at and the operator semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmtFragment {
    /// QF_LIA scalar: comparisons + boolean connectives + linear arithmetic over
    /// `Int` (the `SmtDemo.lean` PoC shape).
    Lia,
    /// QF_BV at a fixed width, rendered directly over `BitVec N`.
    Bv(BvWidth),
}

/// An out-of-fragment refusal (`.design/stage3-bv-reconstruction.md` REQ-7). Mirrors
/// the skip discipline of [`crate::bitvector::render_bv_prop`]: a construct
/// outside the renderable fragment is named, never silently mis-encoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmtExportError {
    /// An `Expr` construct outside the selected fragment, such as a multi-segment
    /// path, method call, or arithmetic operator at proposition position. Carries a
    /// description of the offending construct.
    OutOfFragment(String),
}

impl std::fmt::Display for SmtExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SmtExportError::OutOfFragment(desc) => {
                write!(f, "out of the smt-export fragment: {desc}")
            }
        }
    }
}

/// One per-clause translation-validation equivalence obligation to export
/// (`.design/stage3-bv-reconstruction.md` REQ-7). The exported theorem is the TV
/// shape `(P_production) ⟺ (P_reference)` over the obligation's free variables — the
/// same logical content `thermite-tv`'s `equivalence_obligation` discharges through
/// Verus/Z3, here discharged by a kernel-checked Lean proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmtEquivObligation {
    /// The item / clause name the theorem is named after (sanitized to a Lean ident).
    pub item: String,
    /// The free variables, in binder order. QF_LIA uses `Int`; QF_BV uses `BitVec N`.
    pub vars: Vec<String>,
    /// The production-lowered predicate (the artifact under test).
    pub prod: Expr,
    /// The reference-lowered predicate (the independent encoding).
    pub reference: Expr,
    /// The fragment (QF_LIA or QF_BV at a width).
    pub fragment: SmtFragment,
}

/// `2^width` as a `u128`. `width ≤ 64`, so `1u128 << 64` stays well inside `u128`.
#[must_use]
fn modulus(width: u32) -> u128 {
    1u128 << width
}

/// Sanitize an item name into a Lean identifier tail (`.design/stage3-bv-reconstruction.md`
/// REQ-7). Non-`[A-Za-z0-9_]` characters become `_`; a leading digit is prefixed with
/// `_` so the result is a legal Lean ident. Deterministic (R-CODE-5).
#[must_use]
fn lean_ident(item: &str) -> String {
    let mut out = String::with_capacity(item.len());
    for ch in item.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        out.push('_');
    } else if out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}

/// Render an arithmetic or bitwise term in the selected Lean fragment.
fn render_term(e: &Expr, fragment: SmtFragment) -> Result<String, SmtExportError> {
    match e {
        Expr::IntLit { value, .. } => match fragment {
            SmtFragment::Lia => Ok(format!("({value} : Int)")),
            SmtFragment::Bv(w) => Ok(format!("({}#{})", value % modulus(w.bits()), w.bits())),
        },
        Expr::Path(segs) if segs.len() == 1 => Ok(segs[0].clone()),
        Expr::Binary { op, lhs, rhs } => {
            let sym = match (fragment, op) {
                (_, BinOp::Add) => "+",
                (_, BinOp::Sub) => "-",
                (_, BinOp::Mul) => "*",
                (SmtFragment::Bv(_), BinOp::Div) => "/",
                (SmtFragment::Bv(_), BinOp::Rem) => "%",
                (SmtFragment::Bv(_), BinOp::Shl) => "<<<",
                (SmtFragment::Bv(_), BinOp::Shr) => ">>>",
                (SmtFragment::Bv(_), BinOp::BitAnd) => "&&&",
                (SmtFragment::Bv(_), BinOp::BitOr) => "|||",
                (SmtFragment::Bv(_), BinOp::BitXor) => "^^^",
                (_, other) => {
                    let supported = match fragment {
                        SmtFragment::Lia => "`+`, `-`, or `*`",
                        SmtFragment::Bv(_) => {
                            "arithmetic, division, remainder, bitwise, or shift operators"
                        }
                    };
                    return Err(SmtExportError::OutOfFragment(format!(
                        "`{other:?}` is not a term operator in this fragment; expected {supported}"
                    )));
                }
            };
            let l = render_term(lhs, fragment)?;
            let r = render_term(rhs, fragment)?;
            if matches!((fragment, op), (SmtFragment::Bv(_), BinOp::Div)) {
                let SmtFragment::Bv(w) = fragment else {
                    unreachable!("the branch fixed the fragment")
                };
                // SMT-LIB defines bvudiv-by-zero as all ones, while Lean's BitVec
                // division returns zero. Spell out the SMT case so the two renderers
                // agree even before Thermite's nonzero-divisor obligation is applied.
                let zero = format!("(0#{})", w.bits());
                Ok(format!(
                    "(if {r} = {zero} then (~~~{zero}) else ({l} / {r}))"
                ))
            } else {
                Ok(format!("({l} {sym} {r})"))
            }
        }
        Expr::Unary {
            op: UnaryOp::Not,
            expr,
        } if matches!(fragment, SmtFragment::Bv(_)) => {
            Ok(format!("(~~~{})", render_term(expr, fragment)?))
        }
        // Casts in this fragment preserve the selected fixed-width representation.
        Expr::Cast { expr, .. } => render_term(expr, fragment),
        other => Err(SmtExportError::OutOfFragment(format!(
            "`{other:?}` is outside the renderable term fragment (only integer \
             literals, single-segment variables, fragment operators, and casts)"
        ))),
    }
}

/// Render a proposition to a Lean `Prop` in the given fragment
/// (`.design/stage3-bv-reconstruction.md` REQ-7). `BitVec` comparisons are unsigned,
/// matching the scalar types and the SMT-LIB QF_BV renderer.
fn render_prop(e: &Expr, fragment: SmtFragment) -> Result<String, SmtExportError> {
    match e {
        Expr::BoolLit(b) => Ok(if *b { "True" } else { "False" }.to_string()),
        Expr::Binary { op, lhs, rhs } => match op {
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                let l = render_term(lhs, fragment)?;
                let r = render_term(rhs, fragment)?;
                let rel = match op {
                    BinOp::Eq => "=",
                    BinOp::Ne => "≠",
                    BinOp::Lt => "<",
                    BinOp::Le => "≤",
                    BinOp::Gt => ">",
                    BinOp::Ge => "≥",
                    _ => unreachable!("the outer match fixed the comparison set"),
                };
                Ok(format!("({l} {rel} {r})"))
            }
            BinOp::And => Ok(format!(
                "({} ∧ {})",
                render_prop(lhs, fragment)?,
                render_prop(rhs, fragment)?
            )),
            BinOp::Or => Ok(format!(
                "({} ∨ {})",
                render_prop(lhs, fragment)?,
                render_prop(rhs, fragment)?
            )),
            other => Err(SmtExportError::OutOfFragment(format!(
                "`{other:?}` is an arithmetic/bitwise operator, not a proposition — a \
                 clause must be a comparison or a boolean connective at its root"
            ))),
        },
        Expr::Unary {
            op: UnaryOp::Not,
            expr,
        } => Ok(format!("(¬ {})", render_prop(expr, fragment)?)),
        other => Err(SmtExportError::OutOfFragment(format!(
            "`{other:?}` is outside the renderable proposition fragment (a comparison \
             / boolean connective over the term fragment)"
        ))),
    }
}

/// Render the equivalence GOAL `(P_prod) ↔ (P_ref)` for an obligation
/// (`.design/stage3-bv-reconstruction.md` REQ-7) — the body of the exported theorem,
/// without the binders or the tactic.
pub fn render_goal(o: &SmtEquivObligation) -> Result<String, SmtExportError> {
    // `render_prop` already fully parenthesizes each side, so the `↔` binds the whole
    // predicates with no precedence surprise — no extra wrapping needed.
    let prod = render_prop(&o.prod, o.fragment)?;
    let reference = render_prop(&o.reference, o.fragment)?;
    Ok(format!("{prod} ↔ {reference}"))
}

/// Lemmas needed for the commutative rewrites performed by [`reference_normalize`].
/// Order and comparison rewrites are already handled by `simp`.
fn bv_normalization_lemmas(e: &Expr) -> Vec<&'static str> {
    fn walk(e: &Expr, add: &mut bool, mul: &mut bool) {
        match e {
            Expr::Binary { op, lhs, rhs } => {
                *add |= *op == BinOp::Add;
                *mul |= *op == BinOp::Mul;
                walk(lhs, add, mul);
                walk(rhs, add, mul);
            }
            Expr::Unary { expr, .. } | Expr::Cast { expr, .. } => walk(expr, add, mul),
            _ => {}
        }
    }

    let mut add = false;
    let mut mul = false;
    walk(e, &mut add, &mut mul);
    let mut lemmas = Vec::new();
    if add {
        lemmas.push("BitVec.add_comm");
    }
    if mul {
        lemmas.push("BitVec.mul_comm");
    }
    lemmas
}

/// Export one obligation as a self-contained Lean theorem followed by its
/// `#print axioms` probe (`.design/stage3-bv-reconstruction.md`
/// REQ-7 / AC-8). The theorem is named `thermite_smt_<item>`.
///
/// - [`SmtFragment::Lia`]: `theorem T (a b … : Int) : (P_prod) ↔ (P_ref) := by smt`.
/// - [`SmtFragment::Bv`]: `theorem T (a b … : BitVec N) :
///   (P_prod) ↔ (P_ref) := by simp [BitVec.add_comm, BitVec.mul_comm]`.
pub fn export_theorem(o: &SmtEquivObligation) -> Result<String, SmtExportError> {
    let name = format!("thermite_smt_{}", lean_ident(&o.item));
    let goal = render_goal(o)?;

    match o.fragment {
        SmtFragment::Lia => {
            let binder = if o.vars.is_empty() {
                String::new()
            } else {
                format!(" ({} : Int)", o.vars.join(" "))
            };
            Ok(format!(
                "theorem {name}{binder} :\n    {goal} := by smt\n#print axioms {name}\n"
            ))
        }
        SmtFragment::Bv(w) => {
            let binder = if o.vars.is_empty() {
                String::new()
            } else {
                format!(" ({} : BitVec {})", o.vars.join(" "), w.bits())
            };
            let lemmas = bv_normalization_lemmas(&o.prod);
            let tactic = if lemmas.is_empty() {
                "by\n  simp".to_string()
            } else {
                format!("by\n  simp [{}]", lemmas.join(", "))
            };
            Ok(format!(
                "theorem {name}{binder} :\n    {goal} := {tactic}\n#print axioms {name}\n"
            ))
        }
    }
}

/// The header of an exported Lean file (`.design/stage3-bv-reconstruction.md`
/// REQ-7). A standing banner naming the generator, so the committed artifact is
/// self-describing as automated output (not hand-translation), plus the `import Smt`
/// the `smt` tactic needs.
const FILE_HEADER: &str = "\
/-
  Thermite/SmtExport.lean — AUTO-GENERATED by `forge/src/lean_smt_export.rs`
  (`.design/stage3-bv-reconstruction.md` REQ-7 / AC-8). DO NOT EDIT BY HAND.

  Each theorem is a per-clause translation-validation obligation `(P_prod) ⟺ (P_ref)`
  emitted by the automated Rust→Lean exporter. QF_LIA uses lean-smt/cvc5. QF_BV is
  rendered directly as `BitVec N` and proved from kernel-checked normalization lemmas.
  The `#print axioms` after each theorem must report a subset of
  {propext, Classical.choice, Quot.sound}.

  The literal QF_BV renderer covers wrapping arithmetic, unsigned comparisons,
  bitwise operations, shifts, unsigned division, and remainder. Regenerate via the
  `golden_file_matches_exporter` test with THERMITE_REGEN_SMT_EXPORT=1.
-/
import Smt

namespace Thermite.SmtExport
";

/// Export a batch of obligations into one self-contained Lean file
/// (`.design/stage3-bv-reconstruction.md` REQ-7 / AC-8). The file imports `Smt`,
/// opens the `Thermite.SmtExport` namespace, and emits each obligation's theorem +
/// `#print axioms`. Deterministic in the input order (R-CODE-5).
pub fn export_file(obligations: &[SmtEquivObligation]) -> Result<String, SmtExportError> {
    let mut out = String::from(FILE_HEADER);
    for o in obligations {
        out.push('\n');
        out.push_str(&export_theorem(o)?);
    }
    out.push_str("\nend Thermite.SmtExport\n");
    Ok(out)
}

// ─────────────────────────────────────────────────────────────────────────────
// The reference encoding + obligation minting (the consumers of the renderers).
// ─────────────────────────────────────────────────────────────────────────────

/// A single-segment variable `Expr`.
fn var(name: &str) -> Expr {
    Expr::Path(vec![name.to_string()])
}

/// A binary `Expr`.
fn bin(op: BinOp, lhs: Expr, rhs: Expr) -> Expr {
    Expr::Binary {
        op,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
}

/// A logical-negation `Expr`.
fn not_expr(e: Expr) -> Expr {
    Expr::Unary {
        op: UnaryOp::Not,
        expr: Box::new(e),
    }
}

/// Produce an independent REFERENCE encoding of a production predicate — a
/// syntactically-different but logically-equivalent rewrite
/// (`.design/stage3-bv-reconstruction.md` REQ-7). This plays the role
/// `thermite-tv/src/ref_encode.rs` plays for the Verus obligation: the second,
/// independent rendering whose agreement with production the translation-validation
/// obligation `(P_prod) ⟺ (P_ref)` checks. Each rewrite is equivalence-preserving
/// over QF_LIA `Int` and QF_BV `BitVec N`: the comparison flips hold for their total
/// orders, while addition and multiplication commute in both representations.
///
/// - `a ≤ b` → `¬ (b < a)`, `a < b` → `¬ (b ≤ a)` (the comparison-faithfulness flip);
/// - `a ≥ b` → `b ≤ a`, `a > b` → `b < a`;
/// - `a ≠ b` → `¬ (a = b)`;
/// - `a + b` → `b + a`, `a * b` → `b * a` (commutation);
/// - `=`/`∧`/`∨`/`¬`/`-`/… recurse into normalized children, operator kept.
///
/// Deterministic, total (R-CODE-5): a leaf or an unhandled construct is returned
/// unchanged, so a clause the renderer later refuses is refused downstream,
/// not mangled here.
#[must_use]
pub fn reference_normalize(e: &Expr) -> Expr {
    match e {
        Expr::Binary { op, lhs, rhs } => {
            let l = reference_normalize(lhs);
            let r = reference_normalize(rhs);
            match op {
                BinOp::Le => not_expr(bin(BinOp::Lt, r, l)),
                BinOp::Lt => not_expr(bin(BinOp::Le, r, l)),
                BinOp::Ge => bin(BinOp::Le, r, l),
                BinOp::Gt => bin(BinOp::Lt, r, l),
                BinOp::Ne => not_expr(bin(BinOp::Eq, l, r)),
                BinOp::Add | BinOp::Mul => bin(*op, r, l),
                other => bin(*other, l, r),
            }
        }
        Expr::Unary {
            op: UnaryOp::Not,
            expr,
        } => not_expr(reference_normalize(expr)),
        Expr::Cast { expr, ty } => Expr::Cast {
            expr: Box::new(reference_normalize(expr)),
            ty: ty.clone(),
        },
        other => other.clone(),
    }
}

/// Collect the single-segment free variables of a predicate, sorted and de-duplicated
/// (`.design/stage3-bv-reconstruction.md` REQ-7 — the obligation's binder set).
/// `BTreeSet` gives a deterministic order (R-CODE-5).
#[must_use]
pub fn free_vars(e: &Expr) -> Vec<String> {
    fn walk(e: &Expr, acc: &mut BTreeSet<String>) {
        match e {
            Expr::Path(segs) if segs.len() == 1 => {
                acc.insert(segs[0].clone());
            }
            Expr::Binary { lhs, rhs, .. } => {
                walk(lhs, acc);
                walk(rhs, acc);
            }
            Expr::Unary { expr, .. } | Expr::Cast { expr, .. } => walk(expr, acc),
            _ => {}
        }
    }
    let mut acc = BTreeSet::new();
    walk(e, &mut acc);
    acc.into_iter().collect()
}

/// Is a per-clause predicate inside the RECONSTRUCTION-supported fragment
/// (`.design/stage3-bv-reconstruction.md` REQ-8 / AC-9 — the fragment-support check
/// REQ-8's trust migration keys on)? `true` iff the exporter renders this clause's
/// `(P_prod) ⟺ (P_ref)` translation-validation goal without [`SmtExportError`] in the
/// given fragment. QF_LIA covers its scalar arithmetic subset. QF_BV covers the full
/// fixed-width term surface: wrapping arithmetic, bitwise operators, shifts, unsigned
/// division and remainder, comparisons, and logical connectives.
///
/// Renderability IS the reconstruction-support signal: a clause this returns `true` for
/// has an axiom-clean proof shape in the committed `lean/Thermite/SmtExport.lean`.
/// Total + deterministic (R-CODE-5): a leaf the renderer would refuse is refused here.
#[must_use]
pub fn clause_reconstruction_supported(prod: &Expr, fragment: SmtFragment) -> bool {
    render_goal(&obligation_for_predicate("clause", prod, fragment)).is_ok()
}

/// Mint an equivalence obligation for a production predicate by pairing it with its
/// [`reference_normalize`] encoding (`.design/stage3-bv-reconstruction.md` REQ-7).
/// The binder set is the predicate's [`free_vars`]; the fragment is the caller's.
#[must_use]
pub fn obligation_for_predicate(
    item: &str,
    prod: &Expr,
    fragment: SmtFragment,
) -> SmtEquivObligation {
    SmtEquivObligation {
        item: item.to_string(),
        vars: free_vars(prod),
        prod: prod.clone(),
        reference: reference_normalize(prod),
        fragment,
    }
}

/// The canonical reconstruction-supported obligation set
/// (`.design/stage3-bv-reconstruction.md` REQ-7 / AC-8) — the batch the committed
/// `lean/Thermite/SmtExport.lean` is generated from. One QF_LIA scalar clause and two
/// QF_BV `@bv` clauses covering comparison, wrapping arithmetic, and the complete
/// bitwise/shift/division term surface, each paired with its [`reference_normalize`]
/// reference. The `@bv`
/// fragments are assigned explicitly (not parsed from a `@bvN` tag) so this set is
/// available in the default build, where the `bv` parse feature is off (REQ-1's
/// structural lock).
#[must_use]
pub fn reconstruction_demo_obligations() -> Vec<SmtEquivObligation> {
    // QF_LIA: `(a - b) <= c` — the SmtDemo Tier-3 contract-clause shape.
    let lia = bin(BinOp::Le, bin(BinOp::Sub, var("a"), var("b")), var("c"));
    // QF_BV comparison subfragment (bv64): `a <= b`.
    let bv_cmp = bin(BinOp::Le, var("a"), var("b"));
    // QF_BV modular arithmetic (bv8): `a + b == c`.
    let bv_arith = bin(BinOp::Eq, bin(BinOp::Add, var("a"), var("b")), var("c"));
    // QF_BV full term surface (bv8). The nested multiply is commuted in the
    // independent reference encoding, so this is not a reflexivity-only fixture.
    let bv_full = bin(
        BinOp::Ne,
        bin(
            BinOp::Rem,
            bin(
                BinOp::Div,
                bin(
                    BinOp::Shr,
                    bin(
                        BinOp::Shl,
                        bin(
                            BinOp::BitOr,
                            bin(BinOp::BitAnd, not_expr(var("a")), var("b")),
                            bin(BinOp::BitXor, var("a"), var("b")),
                        ),
                        var("c"),
                    ),
                    var("b"),
                ),
                var("c"),
            ),
            var("b"),
        ),
        bin(BinOp::Add, bin(BinOp::Mul, var("a"), var("b")), var("c")),
    );

    vec![
        obligation_for_predicate("lia_arith_cmp", &lia, SmtFragment::Lia),
        obligation_for_predicate("bv64_le_not_lt", &bv_cmp, SmtFragment::Bv(BvWidth::W64)),
        obligation_for_predicate("bv8_add_comm", &bv_arith, SmtFragment::Bv(BvWidth::W8)),
        obligation_for_predicate("bv8_full_terms", &bv_full, SmtFragment::Bv(BvWidth::W8)),
    ]
}

/// Build the export obligations for every renderable contract `ens` clause of a
/// parsed program (`.design/stage3-bv-reconstruction.md` REQ-7 — the file-driven
/// exporter). A clause carrying a `@bvN` tag (only present in a `bv`-feature build)
/// exports in [`SmtFragment::Bv`]; an untagged clause exports in [`SmtFragment::Lia`].
/// A clause outside the renderable fragment is a skip, named in the returned
/// skip list (never a silent drop). Deterministic in source order (R-CODE-5).
#[must_use]
pub fn obligations_for_program(program: &Program) -> (Vec<SmtEquivObligation>, Vec<String>) {
    let mut obligations = Vec::new();
    let mut skipped = Vec::new();
    for item in &program.items {
        let Item::Fn(f) = item else { continue };
        for (idx, clause) in f.contract.ens.iter().enumerate() {
            let fragment = bv_fragment(clause).unwrap_or(SmtFragment::Lia);
            let name = format!("{}_ens{idx}", f.name);
            let obligation = obligation_for_predicate(&name, &clause.expr, fragment);
            match render_goal(&obligation) {
                Ok(_) => obligations.push(obligation),
                Err(e) => skipped.push(format!("{name}: {e}")),
            }
        }
    }
    (obligations, skipped)
}

/// The QF_BV fragment of a clause carrying a `@bvN` tag, when the `bv` parse feature
/// is compiled in (`.design/stage3-bv-reconstruction.md` REQ-1/REQ-7). Without the
/// feature a `Clause` carries no `bv` field, so this is always `None` and every
/// clause exports as QF_LIA — the structural lock is honored at the exporter too.
#[cfg(feature = "bv")]
fn bv_fragment(clause: &thermite_syntax::Clause) -> Option<SmtFragment> {
    clause.bv.map(|tag| SmtFragment::Bv(tag.width))
}

/// Without the `bv` feature there is no clause-level tag, so every clause is QF_LIA.
#[cfg(not(feature = "bv"))]
fn bv_fragment(_clause: &thermite_syntax::Clause) -> Option<SmtFragment> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::process::Command;
    use thermite_syntax::{Item, Program};

    fn parse_one(src: &str) -> Program {
        let parsed = thermite_syntax::parse(src);
        assert!(parsed.is_clean(), "fixture must parse: {:?}", parsed.errors);
        parsed.program
    }

    /// Extract the parsed `ens` predicate `Expr` of a fn `name` from `src`. The
    /// demo obligations are built from real parsed Thermite predicates (not
    /// hand-built ASTs), so the exporter is exercised on the same `thermite-syntax`
    /// nodes the production obligation carries.
    fn ens_expr(src: &str, name: &str) -> Expr {
        let p = parse_one(src);
        p.items
            .iter()
            .find_map(|i| match i {
                Item::Fn(f) if f.name == name => Some(f.contract.ens[0].expr.clone()),
                _ => None,
            })
            .expect("fn with an ens clause present")
    }

    // REQ-7: `reference_normalize` produces an equivalence-preserving but
    // syntactically-different reference encoding (the comparison-faithfulness flip +
    // arithmetic commutation) — the second rendering the TV obligation checks.
    #[test]
    fn reference_normalize_flips_and_commutes() {
        // `a <= b`  →  `¬ (b < a)`.
        let le = ens_expr(
            "fn p(a: u64, b: u64) -> u64 req true ens a <= b fx pure { a }",
            "p",
        );
        assert_eq!(
            render_prop(&reference_normalize(&le), SmtFragment::Lia).unwrap(),
            "(¬ (b < a))"
        );
        // `a + b == c`  →  `(b + a) == c` (the Add commutes; the `=` is kept).
        let add = ens_expr(
            "fn p(a: u64, b: u64, c: u64) -> u64 req true ens a + b == c fx pure { a }",
            "p",
        );
        assert_eq!(
            render_prop(&reference_normalize(&add), SmtFragment::Lia).unwrap(),
            "((b + a) = c)"
        );
        // A pure variable / literal leaf is returned unchanged (totality).
        assert_eq!(reference_normalize(&var("x")), var("x"));
    }

    // REQ-8 / AC-9: the per-clause support check covers the complete QF_BV term
    // surface, while QF_LIA retains its smaller scalar fragment.
    #[test]
    fn clause_reconstruction_supported_keys_on_the_renderable_fragment() {
        let add = ens_expr(
            "fn p(a: u64, b: u64) -> u64 req true ens a + b == b + a fx pure { a }",
            "p",
        );
        assert!(
            clause_reconstruction_supported(&add, SmtFragment::Bv(BvWidth::W64)),
            "wrapping arithmetic is reconstruction-supported"
        );
        let xor = ens_expr(
            "fn p(a: u64, b: u64) -> u64 req true ens a ^ b ^ b == a fx pure { a }",
            "p",
        );
        assert!(
            clause_reconstruction_supported(&xor, SmtFragment::Bv(BvWidth::W64)),
            "bitwise terms are reconstructed literally"
        );
        // A QF_LIA scalar comparison is supported too.
        let lia = ens_expr(
            "fn p(a: u64, b: u64) -> u64 req true ens a <= b fx pure { a }",
            "p",
        );
        assert!(clause_reconstruction_supported(&lia, SmtFragment::Lia));
        // Shift terms use a BitVec shift amount, matching SMT-LIB `bvshl`.
        let shift = ens_expr(
            "fn p(a: u64, b: u64) -> u64 req true ens (a << b) == a fx pure { a }",
            "p",
        );
        assert!(clause_reconstruction_supported(
            &shift,
            SmtFragment::Bv(BvWidth::W64)
        ));
    }

    // REQ-7: the file-driven exporter mints a QF_LIA obligation per renderable `ens`
    // clause and names a non-renderable (bitwise) clause in the skip list.
    #[test]
    fn program_export_skips_out_of_fragment_clauses() {
        let p = parse_one(
            "fn ok(a: u64, b: u64) -> u64 req true ens a <= b fx pure { a }\n\
             fn bad(a: u64, b: u64) -> u64 req true ens (a & b) == a fx pure { a }",
        );
        let (obligations, skipped) = obligations_for_program(&p);
        assert_eq!(
            obligations.len(),
            1,
            "only the renderable clause is exported"
        );
        assert_eq!(obligations[0].item, "ok_ens0");
        assert_eq!(obligations[0].fragment, SmtFragment::Lia);
        assert_eq!(skipped.len(), 1, "the bitwise clause is a named skip");
        assert!(skipped[0].starts_with("bad_ens0:"));
    }

    // REQ-7: the QF_LIA term/prop renderer maps the contract sublanguage to Lean
    // `Int` syntax (the SmtDemo Tier-3 shape).
    #[test]
    fn lia_renders_arith_comparison() {
        let prod = ens_expr(
            "fn p(a: u64, b: u64, c: u64) -> u64 req true ens a - b <= c fx pure { a }",
            "p",
        );
        assert_eq!(
            render_prop(&prod, SmtFragment::Lia).unwrap(),
            "((a - b) ≤ c)"
        );
    }

    // REQ-7: the QF_LIA disjunction + comparison shape (tv_obligation_or_le surface).
    #[test]
    fn lia_renders_or_of_comparisons() {
        let p = ens_expr(
            "fn p(a: u64, b: u64) -> u64 req true ens a == b || a < b fx pure { a }",
            "p",
        );
        assert_eq!(
            render_prop(&p, SmtFragment::Lia).unwrap(),
            "((a = b) ∨ (a < b))"
        );
    }

    // REQ-7: QF_BV terms render directly as Lean `BitVec N` expressions.
    #[test]
    fn bv_renders_literal_bitvec_arithmetic() {
        let p = ens_expr(
            "fn p(a: u64, b: u64, c: u64) -> u64 req true ens a + b == c fx pure { a }",
            "p",
        );
        assert_eq!(
            render_prop(&p, SmtFragment::Bv(BvWidth::W8)).unwrap(),
            "((a + b) = c)"
        );
        // Numeric syntax carries the width and reduces the value modulo 2^N.
        let lit = ens_expr(
            "fn p(a: u64) -> u64 req true ens a == 300 fx pure { a }",
            "p",
        );
        assert_eq!(
            render_prop(&lit, SmtFragment::Bv(BvWidth::W8)).unwrap(),
            "(a = (44#8))"
        );
    }

    // REQ-7: the literal renderer covers every QF_BV term operator used by the
    // production SMT-LIB renderer.
    #[test]
    fn bv_renders_bitwise_shift_division_and_remainder() {
        for (src, lean_op) in [
            (
                "fn p(a: u64, b: u64) -> u64 req true ens (a & b) == a fx pure { a }",
                "&&&",
            ),
            (
                "fn p(a: u64, b: u64) -> u64 req true ens (a | b) == a fx pure { a }",
                "|||",
            ),
            (
                "fn p(a: u64, b: u64) -> u64 req true ens (a ^ b) == a fx pure { a }",
                "^^^",
            ),
            (
                "fn p(a: u64, b: u64) -> u64 req true ens (a / b) == a fx pure { a }",
                "/",
            ),
            (
                "fn p(a: u64, b: u64) -> u64 req true ens (a % b) == a fx pure { a }",
                "%",
            ),
            (
                "fn p(a: u64, b: u64) -> u64 req true ens (a << b) == a fx pure { a }",
                "<<<",
            ),
            (
                "fn p(a: u64, b: u64) -> u64 req true ens (a >> b) == a fx pure { a }",
                ">>>",
            ),
        ] {
            let p = ens_expr(src, "p");
            let rendered = render_prop(&p, SmtFragment::Bv(BvWidth::W64))
                .expect("the complete QF_BV term surface renders");
            assert!(
                rendered.contains(lean_op),
                "expected Lean operator `{lean_op}` in {rendered}"
            );
        }

        let not = ens_expr(
            "fn p(a: u64) -> u64 req true ens !a == a fx pure { a }",
            "p",
        );
        assert_eq!(
            render_prop(&not, SmtFragment::Bv(BvWidth::W8)).unwrap(),
            "((~~~a) = a)"
        );

        let div = ens_expr(
            "fn p(a: u64, b: u64) -> u64 req true ens (a / b) == a fx pure { a }",
            "p",
        );
        assert_eq!(
            render_prop(&div, SmtFragment::Bv(BvWidth::W8)).unwrap(),
            "((if b = (0#8) then (~~~(0#8)) else (a / b)) = a)",
            "Lean and SMT-LIB use different bvudiv-by-zero defaults, so the zero case \
             must be explicit"
        );
    }

    // REQ-7 / AC-8: QF_LIA uses `smt`; QF_BV uses literal `BitVec N` binders and
    // kernel-checked normalization lemmas.
    #[test]
    fn theorem_shapes_are_well_formed() {
        let obs = reconstruction_demo_obligations();
        let lia = export_theorem(&obs[0]).unwrap();
        assert!(lia.contains("theorem thermite_smt_lia_arith_cmp (a b c : Int) :"));
        assert!(lia.contains("((a - b) ≤ c) ↔ (¬ (c < (a - b)))"));
        assert!(lia.contains(":= by smt\n"));
        assert!(lia.contains("#print axioms thermite_smt_lia_arith_cmp"));

        let bv = export_theorem(&obs[1]).unwrap();
        assert!(bv.contains("(a b : BitVec 64)"));
        assert!(bv.contains(":= by\n  simp\n"));
        assert!(bv.contains("#print axioms thermite_smt_bv64_le_not_lt"));

        let full = export_theorem(&obs[3]).unwrap();
        assert!(full.contains("simp [BitVec.add_comm, BitVec.mul_comm]"));
    }

    // REQ-7: the committed `lean/Thermite/SmtExport.lean` IS the exporter's automated
    // output for the AC-8 batch — the proof the hand-translation gap is closed (the
    // file is generated, not authored). Set THERMITE_REGEN_SMT_EXPORT=1 to regenerate.
    #[test]
    fn golden_file_matches_exporter() {
        let generated =
            export_file(&reconstruction_demo_obligations()).expect("the demo batch exports");
        let golden_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("lean")
            .join("Thermite")
            .join("SmtExport.lean");
        if std::env::var_os("THERMITE_REGEN_SMT_EXPORT").is_some() {
            std::fs::write(&golden_path, &generated).expect("regenerate the golden file");
            return;
        }
        let committed = std::fs::read_to_string(&golden_path).unwrap_or_else(|e| {
            panic!(
                "read committed {}: {e} (regenerate with THERMITE_REGEN_SMT_EXPORT=1)",
                golden_path.display()
            )
        });
        assert_eq!(
            generated, committed,
            "the committed SmtExport.lean must be the exporter's verbatim output \
             (regenerate with THERMITE_REGEN_SMT_EXPORT=1)"
        );
    }

    fn lake_binary() -> Option<PathBuf> {
        if let Ok(home) = std::env::var("HOME") {
            let elan = PathBuf::from(home).join(".elan/bin/lake");
            if elan.exists() {
                return Some(elan);
            }
        }
        if Command::new("lake")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Some(PathBuf::from("lake"));
        }
        None
    }

    /// AC-8 (live): `lake build` the exporter-generated module and assert every
    /// theorem's `#print axioms` report is a subset of `{propext, Classical.choice,
    /// Quot.sound}` — no `sorryAx`, no `Smt` oracle, no `Lean.ofReduceBool`. Gated on
    /// `lake` (the cvc5-FFI build): a shard without it SKIPs rather than fails (the
    /// `engine.rs` live-test precedent). Run requires the SmtDemo toolchain
    /// (toolchain v4.29.0 + Mathlib + vendored cvc5) already materialized.
    #[test]
    fn ac8_exported_obligations_discharge_axiom_clean() {
        let Some(lake) = lake_binary() else {
            eprintln!("SKIP: lake not available — the AC-8 axiom-clean check is not run");
            return;
        };
        let lean_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("lean");
        // Build the committed exporter output module (kept in sync by
        // `golden_file_matches_exporter`). `#print axioms` reports surface as `info:`
        // lines in the build output.
        let out = Command::new(&lake)
            .arg("build")
            .arg("Thermite.SmtExport")
            .current_dir(&lean_root)
            .output()
            .expect("spawn lake build");
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            out.status.success(),
            "lake build of Thermite.SmtExport must succeed (the `smt` tactic discharged \
             every obligation):\n{combined}"
        );
        // Every exported theorem's axiom report must be clean. The names are the
        // `thermite_smt_<item>` theorems of the demo batch.
        let allow = ["propext", "Classical.choice", "Quot.sound"];
        let mut checked = 0usize;
        for o in reconstruction_demo_obligations() {
            let thm = format!("thermite_smt_{}", lean_ident(&o.item));
            let anchor = format!("'Thermite.SmtExport.{thm}'");
            let line = combined
                .lines()
                .find(|l| l.contains(&anchor) && l.contains("depends on axioms:"))
                .unwrap_or_else(|| {
                    panic!("no `#print axioms` report for {thm} in lake output:\n{combined}")
                });
            assert!(
                !line.to_ascii_lowercase().contains("sorry"),
                "{thm} pulled a sorryAx (NOT kernel-clean): {line}"
            );
            let list = line
                .split_once('[')
                .and_then(|(_, rest)| rest.split_once(']'))
                .map(|(inside, _)| inside)
                .unwrap_or("");
            for ax in list.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                assert!(
                    allow.contains(&ax),
                    "{thm} depends on a non-standard axiom `{ax}` (outside {allow:?}): {line}"
                );
            }
            checked += 1;
        }
        assert_eq!(
            checked, 4,
            "all four demo obligations must be axiom-checked"
        );
    }
}
