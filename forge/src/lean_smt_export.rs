//! `forge/src/lean_smt_export.rs` — the automated Rust→Lean obligation exporter
//! for the `smt`-tactic (cvc5-reconstruction) discharge path
//! (`.design/stage3-bv-reconstruction.md` REQ-7 / AC-8).
//!
//! This module closes the Tier-3 hand-translation gap that
//! `lean/Thermite/SmtDemo.lean` left open ("the hand-translation step is the gap an
//! automated Rust→Lean exporter would close"). Given a per-clause equivalence
//! obligation — the translation-validation shape `(P_production) ⟺ (P_reference)`
//! that `thermite-tv/src/obligation.rs::equivalence_obligation` asserts — it renders
//! BOTH predicate ASTs into Lean `Prop`s over a typed env and emits a self-contained
//! Lean theorem discharged by the lean-smt `smt` tactic (pinned @ `7d1d8239`,
//! `lean/lakefile.toml`, vendored cvc5 over FFI), followed by a `#print axioms` probe.
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
//! - **QF_BV** ([`SmtFragment::Bv`]): the fixed-width machine-semantics clauses
//!   [`crate::bitvector::BitVectorEngine`] produces (REQ-2). Rendered over the
//!   **range-bounded integer machine-model** rather than Lean `BitVec N`. See the
//!   decision note below.
//!
//! ## Why QF_BV is rendered over a bounded-integer model, not `BitVec N`
//!
//! `.design/verified/z3-demotion.md` records (and this increment empirically
//! reconfirmed at the pinned lean-smt rev) that lean-smt's QF_BV proof
//! reconstruction bit-blasts through `Smt/Reconstruct/BitVec/Bitblast.lean`, which
//! itself `uses 'sorry'`. Every `BitVec`-typed `by smt` goal therefore pulls
//! `sorryAx` into its `#print axioms` — it is NOT kernel-clean. (Even a pure
//! unsigned-comparison goal over `BitVec N` bit-blasts and routes through the hole.)
//!
//! A `@bvN` clause's fixed-width semantics are faithfully captured over `Int` by the
//! standard machine-model: each `N`-bit variable `x` is an integer with
//! `0 ≤ x < 2^N`, each wrapping arithmetic operation `a ⊕ b` is `(a ⊕ b) % 2^N`
//! (Lean's `Int.emod` lands in `[0, 2^N)` for the positive modulus, matching
//! `bvadd`/`bvsub`/`bvmul`), and each unsigned comparison is the integer comparison
//! on the bounded operands. This goal is QF_LIA — exactly the fragment lean-smt
//! reconstructs kernel-clean (the proven path) — so a `@bv` clause obligation rendered
//! this way discharges within `{propext, Classical.choice, Quot.sound}`. The literal
//! `BitVec N` render (`crate::bitvector::render_bv_prop`, the SMT-LIB2 query
//! `--engine bv` runs) is the artifact lean-smt's literal QF_BV replay would consume,
//! and remains blocked on the upstream bit-blasting `sorry`.
//!
//! **Faithfulness of the integer model is kernel-proven** (`lean/Thermite/BvModel.lean`,
//! issue #356): the term/proposition fragment [`render_term`] / [`render_prop`] emit here
//! corresponds arm-for-arm to `Thermite.BvModel.{Tm, Frm}`, and the metatheorem
//! `frmInt_iff_frmBV` proves the bounded-integer denotation agrees with the genuine
//! `BitVec N` denotation (`#print axioms` ⊆ the standard set, Mathlib/Smt-free). So with
//! the exporter's `by smt` int-model `↔` reconstructed in the kernel AND that faithfulness
//! theorem, a `@bv` clause's truth is kernel-grounded end to end — discharging the REQ-8
//! `render_bv_prop` faithfulness obligation in our own spine, with no solver in the trust
//! base for the renderable fragment and no dependency on lean-smt's literal bv
//! reconstruction. The Rust-emitter ⟷ Lean-AST correspondence is inspection-tier (as for
//! the whole exporter — `.design/verified/exporter-surface-correspondence.md`).
//!
//! ## Out of the renderable fragment
//!
//! Bitwise / shift operators (`&`/`|`/`^`/`<<`/`>>`) and integer division/remainder
//! are an honest [`SmtExportError::OutOfFragment`] skip — they are the bit-blasting
//! / division-by-zero territory the integer model does not faithfully and cleanly
//! capture (the `z3-demotion.md` bitwise wall). A skip is never a silent wrong
//! encoding; it names the offending construct.

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
    /// QF_BV at a fixed width, rendered over the range-bounded integer machine-model
    /// (`0 ≤ x < 2^N` per variable, wrapping arithmetic as `% 2^N`).
    Bv(BvWidth),
}

/// An out-of-fragment refusal (`.design/stage3-bv-reconstruction.md` REQ-7). Mirrors
/// the honest-skip discipline of [`crate::bitvector::render_bv_prop`]: a construct
/// outside the renderable fragment is named, never silently mis-encoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmtExportError {
    /// An `Expr` construct outside the renderable QF_LIA / QF_BV fragment (a bitwise
    /// or shift operator, division/remainder, a multi-segment path, a method call,
    /// an arithmetic operator at proposition position, …). Carries a human
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
/// Verus/Z3, here discharged by the kernel-checked `smt` tactic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmtEquivObligation {
    /// The item / clause name the theorem is named after (sanitized to a Lean ident).
    pub item: String,
    /// The free variables, in binder order. All are rendered at the fragment's sort
    /// (`Int` for both QF_LIA and the QF_BV integer machine-model).
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

/// Render an arithmetic TERM to a Lean `Int` expression in the given fragment
/// (`.design/stage3-bv-reconstruction.md` REQ-7). In [`SmtFragment::Lia`] the
/// operators are the plain integer operators; in [`SmtFragment::Bv`] each wrapping
/// operation is reduced `% 2^N` to model fixed-width wraparound and a literal is
/// reduced into `[0, 2^N)`.
fn render_term(e: &Expr, fragment: SmtFragment) -> Result<String, SmtExportError> {
    match e {
        Expr::IntLit { value, .. } => match fragment {
            SmtFragment::Lia => Ok(format!("({value} : Int)")),
            SmtFragment::Bv(w) => Ok(format!("({} : Int)", value % modulus(w.bits()))),
        },
        Expr::Path(segs) if segs.len() == 1 => Ok(segs[0].clone()),
        Expr::Binary { op, lhs, rhs } => {
            let sym = match op {
                BinOp::Add => "+",
                BinOp::Sub => "-",
                BinOp::Mul => "*",
                other => {
                    return Err(SmtExportError::OutOfFragment(format!(
                        "`{other:?}` is not a renderable arithmetic term operator (only \
                         `+`/`-`/`*` lower to the integer model; `/`/`%`/`&`/`|`/`^`/`<<`/`>>` \
                         are the bit-blasting / division residual)"
                    )))
                }
            };
            let l = render_term(lhs, fragment)?;
            let r = render_term(rhs, fragment)?;
            match fragment {
                SmtFragment::Lia => Ok(format!("({l} {sym} {r})")),
                // Model fixed-width wraparound: the N-bit operation is the integer
                // operation reduced modulo 2^N (Lean `Int.emod` lands in [0, 2^N)).
                SmtFragment::Bv(w) => Ok(format!("(({l} {sym} {r}) % {})", modulus(w.bits()))),
            }
        }
        // A cast leaves the integer value unchanged in the model (the spine view; the
        // `crate::bitvector::render_bv_term` precedent renders the inner term).
        Expr::Cast { expr, .. } => render_term(expr, fragment),
        other => Err(SmtExportError::OutOfFragment(format!(
            "`{other:?}` is outside the renderable term fragment (only integer \
             literals, single-segment variables, `+`/`-`/`*`, and casts)"
        ))),
    }
}

/// Render a PROPOSITION to a Lean `Prop` in the given fragment
/// (`.design/stage3-bv-reconstruction.md` REQ-7). Comparisons map to the integer
/// relations (faithful for the unsigned bit-vector relations on the bounded
/// machine-model operands), the connectives to `∧`/`∨`/`¬`. `Err` names the
/// out-of-fragment construct.
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

/// Export one obligation as a self-contained Lean theorem discharged by `smt`,
/// followed by its `#print axioms` probe (`.design/stage3-bv-reconstruction.md`
/// REQ-7 / AC-8). The theorem is named `thermite_smt_<item>`.
///
/// - [`SmtFragment::Lia`]: `theorem T (a b … : Int) : (P_prod) ↔ (P_ref) := by smt`.
/// - [`SmtFragment::Bv`]: each variable additionally carries its machine-domain
///   range hypotheses `0 ≤ x` / `x < 2^N`, passed to `smt` so the bounded-integer
///   model is sound. `theorem T (a … : Int) (h0lo : 0 ≤ a) (h0hi : a < 2^N) … :
///   (P_prod) ↔ (P_ref) := by smt [h0lo, h0hi, …]`.
pub fn export_theorem(o: &SmtEquivObligation) -> Result<String, SmtExportError> {
    let name = format!("thermite_smt_{}", lean_ident(&o.item));
    let goal = render_goal(o)?;

    let var_binder = if o.vars.is_empty() {
        String::new()
    } else {
        format!(" ({} : Int)", o.vars.join(" "))
    };

    match o.fragment {
        SmtFragment::Lia => Ok(format!(
            "theorem {name}{var_binder} :\n    {goal} := by smt\n#print axioms {name}\n"
        )),
        SmtFragment::Bv(w) => {
            let m = modulus(w.bits());
            let mut hyp_binders = String::new();
            let mut hyp_names: Vec<String> = Vec::new();
            for (i, v) in o.vars.iter().enumerate() {
                let lo = format!("h{i}lo");
                let hi = format!("h{i}hi");
                hyp_binders.push_str(&format!(" ({lo} : 0 ≤ {v}) ({hi} : {v} < {m})"));
                hyp_names.push(lo);
                hyp_names.push(hi);
            }
            let tactic = if hyp_names.is_empty() {
                "by smt".to_string()
            } else {
                format!("by smt [{}]", hyp_names.join(", "))
            };
            Ok(format!(
                "theorem {name}{var_binder}{hyp_binders} :\n    {goal} := {tactic}\n#print axioms {name}\n"
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
  emitted by the automated Rust→Lean exporter — the step `Thermite.SmtDemo` performed
  by hand. It is discharged by the lean-smt `smt` tactic (cvc5 reconstruction, pinned
  @ 7d1d8239) and KERNEL-CHECKED; the `#print axioms` after each must report a subset
  of {propext, Classical.choice, Quot.sound} (no Smt oracle, no sorryAx, no
  Lean.ofReduceBool) for the fragment to count as reconstruction-supported.

  QF_BV clauses are rendered over the range-bounded integer machine-model (bv var ->
  Int with 0 ≤ x < 2^N, wraparound op -> `% 2^N`, unsigned cmp -> Int cmp): lean-smt's
  literal BitVec reconstruction bit-blasts through an upstream `sorry`
  (.design/verified/z3-demotion.md), so the integer model is the reconstruction-
  supported QF_BV encoding. Regenerate via the `golden_file_matches_exporter` test
  with THERMITE_REGEN_SMT_EXPORT=1.
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
/// over BOTH the QF_LIA `Int` model and the QF_BV bounded-integer machine-model
/// (the unsigned comparison flips hold for a total order; `+`/`*` commute for both
/// `Int` and the wraparound `% 2^N` operations):
///
/// - `a ≤ b` → `¬ (b < a)`, `a < b` → `¬ (b ≤ a)` (the comparison-faithfulness flip);
/// - `a ≥ b` → `b ≤ a`, `a > b` → `b < a`;
/// - `a ≠ b` → `¬ (a = b)`;
/// - `a + b` → `b + a`, `a * b` → `b * a` (commutation);
/// - `=`/`∧`/`∨`/`¬`/`-`/… recurse into normalized children, operator kept.
///
/// Deterministic, total (R-CODE-5): a leaf or an unhandled construct is returned
/// unchanged, so a clause the renderer later refuses is refused honestly downstream,
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

/// Is a per-clause predicate inside the RECONSTRUCTION-SUPPORTED fragment
/// (`.design/stage3-bv-reconstruction.md` REQ-8 / AC-9 — the fragment-support check
/// REQ-8's trust migration keys on)? `true` iff the exporter renders this clause's
/// `(P_prod) ⟺ (P_ref)` translation-validation goal WITHOUT [`SmtExportError`] in the
/// given fragment — i.e. exactly the QF_LIA scalar + arithmetic/comparison QF_BV subset
/// (`+`/`-`/`*`, unsigned `=`/`≠`/`<`/`≤`/`>`/`≥`, logical connectives). The
/// bitwise/shift/rotate QF_BV subset (`^`/`&`/`|`/`<<`/`>>`, rotate) and
/// division/remainder are the [`SmtExportError::OutOfFragment`] refusal — the
/// bit-blasting wall (`.design/verified/z3-demotion.md`) — and read `false` here, so
/// those clauses stay solver-trusted (the F-J residual the audit names).
///
/// Renderability IS the reconstruction-support signal: a clause this returns `true` for
/// is rendered over the bounded-integer machine-model that lean-smt reconstructs
/// kernel-clean (`#print axioms ⊆ {propext, Classical.choice, Quot.sound}`, the AC-8
/// committed `lean/Thermite/SmtExport.lean` proof + the kernel-checked
/// `BvModel.frmInt_iff_frmBV` faithfulness metatheorem) — so the renderable fragment's
/// axiom-cleanliness is discharged by REQ-7 once, statically, not re-run per clause.
/// Total + deterministic (R-CODE-5): a leaf the renderer would refuse is refused here
/// too, never a silent claim of support.
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
/// QF_BV `@bv` clauses (a comparison subfragment clause at bv64, a modular-arithmetic
/// clause at bv8), each paired with its [`reference_normalize`] reference. The `@bv`
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

    vec![
        obligation_for_predicate("lia_arith_cmp", &lia, SmtFragment::Lia),
        obligation_for_predicate("bv64_le_not_lt", &bv_cmp, SmtFragment::Bv(BvWidth::W64)),
        obligation_for_predicate("bv8_add_comm", &bv_arith, SmtFragment::Bv(BvWidth::W8)),
    ]
}

/// Build the export obligations for every renderable contract `ens` clause of a
/// parsed program (`.design/stage3-bv-reconstruction.md` REQ-7 — the file-driven
/// exporter). A clause carrying a `@bvN` tag (only present in a `bv`-feature build)
/// exports in [`SmtFragment::Bv`]; an untagged clause exports in [`SmtFragment::Lia`].
/// A clause outside the renderable fragment is an honest skip, named in the returned
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
    /// demo obligations are built from REAL parsed Thermite predicates (not
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

    // REQ-8 / AC-9: the per-clause fragment-support check keys on renderability — the
    // arithmetic/comparison subset is reconstruction-supported (migrates trust), the
    // bitwise/shift/rotate subset is refused (stays solver-trusted).
    #[test]
    fn clause_reconstruction_supported_keys_on_the_renderable_fragment() {
        // The mix64 split: `a + b == b + a` (arith) is supported; `a ^ b ^ b == a` (xor) is not.
        let add = ens_expr(
            "fn p(a: u64, b: u64) -> u64 req true ens a + b == b + a fx pure { a }",
            "p",
        );
        assert!(
            clause_reconstruction_supported(&add, SmtFragment::Bv(BvWidth::W64)),
            "wraparound-add commutativity is the reconstruction-supported QF_BV subset"
        );
        let xor = ens_expr(
            "fn p(a: u64, b: u64) -> u64 req true ens a ^ b ^ b == a fx pure { a }",
            "p",
        );
        assert!(
            !clause_reconstruction_supported(&xor, SmtFragment::Bv(BvWidth::W64)),
            "xor is the bitwise subset the exporter refuses — stays solver-trusted (F-J)"
        );
        // A QF_LIA scalar comparison is supported too.
        let lia = ens_expr(
            "fn p(a: u64, b: u64) -> u64 req true ens a <= b fx pure { a }",
            "p",
        );
        assert!(clause_reconstruction_supported(&lia, SmtFragment::Lia));
        // A shift clause (the rotl1 lemma shape) is refused.
        let shift = ens_expr(
            "fn p(a: u64, b: u64) -> u64 req true ens (a << b) == a fx pure { a }",
            "p",
        );
        assert!(!clause_reconstruction_supported(
            &shift,
            SmtFragment::Bv(BvWidth::W64)
        ));
    }

    // REQ-7: the file-driven exporter mints a QF_LIA obligation per renderable `ens`
    // clause and honestly names a non-renderable (bitwise) clause in the skip list.
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

    // REQ-7: the QF_BV integer machine-model reduces literals and wraps arithmetic
    // `% 2^N` while comparisons stay integer comparisons on the bounded operands.
    #[test]
    fn bv_renders_modular_arithmetic() {
        let p = ens_expr(
            "fn p(a: u64, b: u64, c: u64) -> u64 req true ens a + b == c fx pure { a }",
            "p",
        );
        assert_eq!(
            render_prop(&p, SmtFragment::Bv(BvWidth::W8)).unwrap(),
            "(((a + b) % 256) = c)"
        );
        // A literal is reduced into [0, 2^N): `300` at width 8 is `300 % 256 = 44`.
        let lit = ens_expr(
            "fn p(a: u64) -> u64 req true ens a == 300 fx pure { a }",
            "p",
        );
        assert_eq!(
            render_prop(&lit, SmtFragment::Bv(BvWidth::W8)).unwrap(),
            "(a = (44 : Int))"
        );
    }

    // REQ-7: bitwise / shift / division operators are an honest OutOfFragment skip
    // (the bit-blasting / division residual), never a silent wrong encoding.
    #[test]
    fn bitwise_and_division_are_out_of_fragment() {
        for (src, clause) in [
            (
                "fn p(a: u64, b: u64) -> u64 req true ens (a & b) == a fx pure { a }",
                "&",
            ),
            (
                "fn p(a: u64, b: u64) -> u64 req true ens (a | b) == a fx pure { a }",
                "|",
            ),
            (
                "fn p(a: u64, b: u64) -> u64 req true ens (a ^ b) == a fx pure { a }",
                "^",
            ),
            (
                "fn p(a: u64, b: u64) -> u64 req true ens (a / b) == a fx pure { a }",
                "/",
            ),
            (
                "fn p(a: u64, b: u64) -> u64 req true ens (a << b) == a fx pure { a }",
                "<<",
            ),
        ] {
            let p = ens_expr(src, "p");
            assert!(
                matches!(
                    render_prop(&p, SmtFragment::Bv(BvWidth::W64)),
                    Err(SmtExportError::OutOfFragment(_))
                ),
                "the `{clause}` operator must be an OutOfFragment skip"
            );
        }
    }

    // REQ-7 / AC-8: the QF_LIA theorem shape is the SmtDemo `by smt` form with a
    // `#print axioms` probe; the QF_BV theorem additionally carries the machine-domain
    // range hypotheses, passed to `smt`.
    #[test]
    fn theorem_shapes_are_well_formed() {
        let obs = reconstruction_demo_obligations();
        let lia = export_theorem(&obs[0]).unwrap();
        assert!(lia.contains("theorem thermite_smt_lia_arith_cmp (a b c : Int) :"));
        assert!(lia.contains("((a - b) ≤ c) ↔ (¬ (c < (a - b)))"));
        assert!(lia.contains(":= by smt\n"));
        assert!(lia.contains("#print axioms thermite_smt_lia_arith_cmp"));

        let bv = export_theorem(&obs[1]).unwrap();
        assert!(bv.contains("(h0lo : 0 ≤ a) (h0hi : a < 18446744073709551616)"));
        assert!(bv.contains(":= by smt [h0lo, h0hi, h1lo, h1hi]"));
        assert!(bv.contains("#print axioms thermite_smt_bv64_le_not_lt"));
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
            checked, 3,
            "all three demo obligations must be axiom-checked"
        );
    }
}
