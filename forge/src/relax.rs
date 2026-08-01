//! `forge/src/relax.rs` — the **relaxable** syntactic fragment (REQ-8b/8c;
//! `.design/stage1-forge-tier.md` REQ-8 / AC-12, increment 2f). The classifier the
//! nlsat relax route gates on, plus the two pure helpers that route consumes: the
//! real-relaxation SMT-LIB(QF_NRA) encoding (the query nlsat answers) and the
//! integer evaluator (the integrality check, Q8).
//!
//! ## The relaxable fragment (REQ-8b)
//!
//! A `fn`'s contract is **relaxable** iff it is a universally-quantified statement
//! over integer-scalar parameters whose clause atoms are *polynomial* — built only
//! from variables, integer literals, `+`/`-`/`*`, the comparison relations, and the
//! boolean connectives. Crucially it contains **no div/mod/shifts/casts** (REQ-8b):
//! those are not polynomial over ℝ (`/`, `%`), are bit-level rather than arithmetic
//! (`<<`/`>>`/`&`/`|`/`^`), or change the carrier (`as`), so the real relaxation
//! `∀ x : ℝ, …` would not faithfully encode the integer clause and the relax spine
//! lemmas (`rencode_sound` / `r_relax_sound`, `lean/Thermite/Relax.lean`) would not
//! apply. The quantifier is *implicit*: a `fn` contract is `∀ params, req → ⋀ ens`,
//! so the relaxable check is over the contract's clause expressions, with the
//! parameters (and `result`) confirmed integer-scalar.
//!
//! ## What the route does with a relaxable contract (REQ-8c)
//!
//! The route hands nlsat the **negation** `∃ vars : ℝ, req ∧ ¬(⋀ ens)` ([`negated_contract_query`]):
//! - `unsat` → no real counterexample → the relaxation `∀ x : ℝ, …` holds → by the
//!   kernel-checked `r_relax_sound` the integer clause holds → certify at **L4**.
//! - `sat` → a real countermodel → the **integrality check** ([`eval_contract_negation_over_ints`]):
//!   round the real point into a radius-2 ℤⁿ box and test whether any integer point
//!   falsifies the integer clause. An integer falsifier is a real
//!   `Counterexample`; if none does, the countermodel is real-only (true over ℤ,
//!   false over ℝ) → a `RealWitness` escalation (never a `Counterexample`).
//!
//! All of this module is pure (no z3, no I/O) so it unit-tests without the solver;
//! the [`crate::engine::NlsatEngine`] (the z3-invoking layer) consumes it.

use std::collections::BTreeMap;

use thermite_syntax::{BinOp, Clause, Expr, FnItem, PrimType, Type, UnaryOp};

/// The relaxable-classification verdict (`.design/stage1-forge-tier.md` REQ-8 /
/// AC-12). [`Relaxable`](RelaxVerdict::Relaxable) means the whole contract is in the
/// relax fragment (the nlsat route may attempt it); [`NotRelaxable`](RelaxVerdict::NotRelaxable)
/// names the first construct that put it out of fragment (a div/mod/shift/cast atom,
/// a non-integer parameter, a non-polynomial call), so the route's skip is explicit and
/// the auditor sees *why* (R-CODE-4 — never a bare boolean).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelaxVerdict {
    /// Every clause is a relaxable polynomial proposition over integer-scalar vars.
    Relaxable,
    /// Out of fragment; the string names the disqualifying construct.
    NotRelaxable(String),
}

impl RelaxVerdict {
    /// `true` iff relaxable (the fragment-gate predicate the route keys on).
    #[must_use]
    pub fn is_relaxable(&self) -> bool {
        matches!(self, RelaxVerdict::Relaxable)
    }
}

/// Is this Thermite type an integer scalar (`u32`/`u64`/`usize`)? Only these carry
/// the ℤ-valued, ℝ-relaxable reading the route relies on; `bool`, `Unit`, and every
/// aggregate (`Vec`/`String`/`Option`/tuple/ADT/…) are out of fragment.
fn is_integer_scalar(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Prim(PrimType::U8)
            | Type::Prim(PrimType::U16)
            | Type::Prim(PrimType::U32)
            | Type::Prim(PrimType::U64)
            | Type::Prim(PrimType::Usize)
    )
}

/// Classify a `fn` against the relaxable fragment (`.design/stage1-forge-tier.md`
/// REQ-8b / AC-12). Relaxable iff (a) every parameter is an integer scalar, (b) the
/// return type is an integer scalar (so `result` is a relaxable variable), and (c)
/// every contract clause — the `req` and each `ens` — is a relaxable polynomial
/// proposition (no div/mod/shifts/casts in any atom). A boundary `fn` (no body) is
/// out of fragment (the relax route discharges in-language contracts). The implicit
/// `∀ params` is the universal quantifier the fragment names; this confirms its
/// domain is integer-scalar.
#[must_use]
pub fn classify_fn(f: &FnItem) -> RelaxVerdict {
    if f.boundary.is_some() {
        return RelaxVerdict::NotRelaxable(
            "a boundary `fn` has no in-language contract to relax".to_string(),
        );
    }
    for p in &f.params {
        if !is_integer_scalar(&p.ty) {
            return RelaxVerdict::NotRelaxable(format!(
                "parameter `{}` is not an integer scalar (the relax fragment is \
                 universally quantified over integer-scalar variables only)",
                p.name
            ));
        }
    }
    if !is_integer_scalar(&f.ret) {
        return RelaxVerdict::NotRelaxable(
            "the return type is not an integer scalar, so `result` is not a relaxable \
             variable"
                .to_string(),
        );
    }
    if let Err(reason) = classify_prop(&f.contract.req.expr) {
        return RelaxVerdict::NotRelaxable(format!(
            "the `req` clause is out of fragment: {reason}"
        ));
    }
    for (k, ens) in f.contract.ens.iter().enumerate() {
        if let Err(reason) = classify_prop(&ens.expr) {
            return RelaxVerdict::NotRelaxable(format!("`ens#{k}` is out of fragment: {reason}"));
        }
    }
    RelaxVerdict::Relaxable
}

/// Classify a single clause's expression as a relaxable proposition
/// (`.design/stage1-forge-tier.md` REQ-8b / AC-12) — the per-clause helper the
/// `relaxable admits/rejects` unit tests exercise directly. The production fragment
/// gate is the whole-contract [`classify_fn`]; this is the per-clause projection.
#[allow(
    dead_code,
    reason = "REQ-8b/AC-12 per-clause relaxable check: exercised by the relax unit tests \
              (admits an isqrt ens, rejects a div clause); classify_fn is the production gate"
)]
#[must_use]
pub fn classify_clause(c: &Clause) -> RelaxVerdict {
    match classify_prop(&c.expr) {
        Ok(()) => RelaxVerdict::Relaxable,
        Err(reason) => RelaxVerdict::NotRelaxable(reason),
    }
}

/// Is `e` a relaxable proposition (a boolean-valued clause expression)? A
/// comparison of polynomial terms, a boolean connective of relaxable propositions,
/// or a boolean literal. Anything else (notably a div/mod/shift/cast/bitwise atom,
/// reached through a term) is out of fragment, with the reason named.
fn classify_prop(e: &Expr) -> Result<(), String> {
    match e {
        Expr::BoolLit(_) => Ok(()),
        Expr::Binary { op, lhs, rhs } => match op {
            // Comparison relations: both sides are polynomial terms.
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                classify_term(lhs)?;
                classify_term(rhs)
            }
            // Boolean connectives: both sides are propositions.
            BinOp::And | BinOp::Or => {
                classify_prop(lhs)?;
                classify_prop(rhs)
            }
            // Div/mod/shift/bitwise at proposition position is not a boolean
            // connective — out of fragment (REQ-8b "no div/mod/shifts").
            BinOp::Div => Err("`/` (division) is not relaxable (REQ-8b)".to_string()),
            BinOp::Rem => Err("`%` (modulo) is not relaxable (REQ-8b)".to_string()),
            BinOp::Shl | BinOp::Shr => {
                Err("a shift (`<<`/`>>`) is not relaxable (REQ-8b)".to_string())
            }
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor => {
                Err("a bitwise operator is not a polynomial atom (REQ-8b)".to_string())
            }
            BinOp::Add | BinOp::Sub | BinOp::Mul => Err(
                "an arithmetic term cannot stand as a proposition (expected a \
                     comparison or boolean connective)"
                    .to_string(),
            ),
        },
        // `!p` is a logical negation over a relaxable proposition.
        Expr::Unary {
            op: UnaryOp::Not,
            expr,
        } => classify_prop(expr),
        other => Err(format!(
            "`{}` is not a relaxable proposition (expected a comparison, a boolean \
             connective, or a boolean literal)",
            expr_kind(other)
        )),
    }
}

/// Is `e` a relaxable polynomial TERM (an integer-valued atom)? A variable, an
/// integer literal, or a `+`/`-`/`*` of relaxable terms. Every disqualifying
/// construct — `/`, `%`, `<<`/`>>`, the bitwise operators, and a cast `as` — is
/// rejected here (REQ-8b "no div/mod/shifts/casts in atoms"), as are calls, indexing,
/// and the like (non-polynomial).
fn classify_term(e: &Expr) -> Result<(), String> {
    match e {
        Expr::IntLit { .. } => Ok(()),
        // A single-segment path is a variable (a parameter or `result`); a
        // `::`-segmented path is not a relax variable.
        Expr::Path(segs) if segs.len() == 1 => Ok(()),
        Expr::Path(_) => Err("a `::`-segmented path is not a relax-fragment variable".to_string()),
        Expr::Binary { op, lhs, rhs } => match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul => {
                classify_term(lhs)?;
                classify_term(rhs)
            }
            BinOp::Div => Err("`/` (division) is not a polynomial atom (REQ-8b)".to_string()),
            BinOp::Rem => Err("`%` (modulo) is not a polynomial atom (REQ-8b)".to_string()),
            BinOp::Shl | BinOp::Shr => {
                Err("a shift (`<<`/`>>`) is not a polynomial atom (REQ-8b)".to_string())
            }
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor => {
                Err("a bitwise operator is not a polynomial atom (REQ-8b)".to_string())
            }
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                Err("a comparison cannot stand as an arithmetic term".to_string())
            }
            BinOp::And | BinOp::Or => {
                Err("a boolean connective cannot stand as an arithmetic term".to_string())
            }
        },
        Expr::Cast { .. } => {
            Err("a cast (`as`) is not allowed in a relaxable atom (REQ-8b)".to_string())
        }
        other => Err(format!(
            "`{}` is not a relaxable polynomial term (expected a variable, an integer \
             literal, or a `+`/`-`/`*` of terms)",
            expr_kind(other)
        )),
    }
}

/// A short kind tag for an [`Expr`] (diagnostics only — names the offending
/// construct in a `NotRelaxable` reason). Deterministic (R-CODE-5).
fn expr_kind(e: &Expr) -> &'static str {
    match e {
        Expr::IntLit { .. } => "integer literal",
        Expr::BoolLit(_) => "boolean literal",
        Expr::Path(_) => "path",
        Expr::Call { .. } => "call",
        Expr::MethodCall { .. } => "method call",
        Expr::Field { .. } => "field access",
        Expr::Closure { .. } => "closure",
        Expr::Match { .. } => "match",
        Expr::If { .. } => "if-expression",
        Expr::Binary { .. } => "binary expression",
        Expr::Unary { .. } => "unary expression",
        Expr::Index { .. } => "index",
        Expr::Cast { .. } => "cast",
        Expr::Ref { .. } => "reference",
        Expr::StructLit { .. } => "struct literal",
        Expr::Is { .. } => "`is` test",
        Expr::Deref(_) => "dereference",
        Expr::StrLit(_) => "string literal",
        Expr::Tuple(_) => "tuple",
        Expr::TupleProj { .. } => "tuple projection",
        // A raw quantifier (`.design/stage2-stratified-cage.md` REQ-0): a kind label
        // for the relax classifier; stratified relax routing is stage-2 (REQ-8).
        Expr::Quantifier { .. } => "quantifier",
    }
}

/// The integer-scalar variables of a relaxable `fn`, in a deterministic order
/// (parameters in signature order, then `result`). The relax encoding declares each
/// as a `Real`; the integrality check assigns each an integer. Pre: `f` is relaxable
/// (`classify_fn(f).is_relaxable()`), so every name here is an integer
/// variable.
#[must_use]
pub fn integer_vars(f: &FnItem) -> Vec<String> {
    let mut vars: Vec<String> = f.params.iter().map(|p| p.name.clone()).collect();
    vars.push("result".to_string());
    vars
}

// ─────────────────────────────────────────────────────────────────────────────
// The real-relaxation SMT-LIB(QF_NRA) encoding (REQ-8c, the Q-NLSAT query).
// ─────────────────────────────────────────────────────────────────────────────

/// Render a relaxable polynomial TERM to an SMT-LIB(QF_NRA) `Real` expression. Pre:
/// `classify_term` accepted `e`. Returns `None` only if `e` is out of fragment
/// (defensive — the route classifies first). Integer literals render as decimals
/// (e.g. `2.0`) so they take the `Real` sort in the QF_NRA query.
#[must_use]
pub fn render_term_smt(e: &Expr) -> Option<String> {
    match e {
        Expr::IntLit { value, .. } => Some(format!("{value}.0")),
        Expr::Path(segs) if segs.len() == 1 => Some(segs[0].clone()),
        Expr::Binary { op, lhs, rhs } => {
            let l = render_term_smt(lhs)?;
            let r = render_term_smt(rhs)?;
            let sym = match op {
                BinOp::Add => "+",
                BinOp::Sub => "-",
                BinOp::Mul => "*",
                _ => return None,
            };
            Some(format!("({sym} {l} {r})"))
        }
        _ => None,
    }
}

/// Render a relaxable proposition to an SMT-LIB(QF_NRA) `Bool` expression. Pre:
/// `classify_prop` accepted `e`. `≠` renders as `(not (= …))`; the comparisons map to
/// their SMT relations; the connectives to `and`/`or`/`not`.
#[must_use]
pub fn render_prop_smt(e: &Expr) -> Option<String> {
    match e {
        Expr::BoolLit(b) => Some(if *b {
            "true".to_string()
        } else {
            "false".to_string()
        }),
        Expr::Binary { op, lhs, rhs } => match op {
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                let l = render_term_smt(lhs)?;
                let r = render_term_smt(rhs)?;
                let rel = match op {
                    BinOp::Eq | BinOp::Ne => "=",
                    BinOp::Lt => "<",
                    BinOp::Le => "<=",
                    BinOp::Gt => ">",
                    BinOp::Ge => ">=",
                    _ => unreachable!(),
                };
                let cmp = format!("({rel} {l} {r})");
                Some(if matches!(op, BinOp::Ne) {
                    format!("(not {cmp})")
                } else {
                    cmp
                })
            }
            BinOp::And => Some(format!(
                "(and {} {})",
                render_prop_smt(lhs)?,
                render_prop_smt(rhs)?
            )),
            BinOp::Or => Some(format!(
                "(or {} {})",
                render_prop_smt(lhs)?,
                render_prop_smt(rhs)?
            )),
            _ => None,
        },
        Expr::Unary {
            op: UnaryOp::Not,
            expr,
        } => Some(format!("(not {})", render_prop_smt(expr)?)),
        _ => None,
    }
}

/// Build the full SMT-LIB(QF_NRA) query whose satisfiability nlsat decides
/// (`.design/stage1-forge-tier.md` REQ-8c / Q-NLSAT). It asserts the **negation** of
/// the contract over the reals — `req ∧ ¬(⋀ ens)` — with each integer variable
/// declared `Real`, and asks the nlsat tactic:
///
/// - `unsat` → no real counterexample → `∀ x : ℝ, req → ⋀ ens` holds → (by
///   `r_relax_sound`) the integer clause holds → the route certifies **L4**;
/// - `sat` → a real countermodel `(get-model)` returns, handed to the integrality
///   check.
///
/// Pre: `f` is relaxable. Returns `None` if any clause fails to render (defensive).
#[must_use]
pub fn negated_contract_query(f: &FnItem) -> Option<String> {
    let mut s = String::new();
    s.push_str("(set-logic QF_NRA)\n");
    for v in integer_vars(f) {
        s.push_str(&format!("(declare-const {v} Real)\n"));
    }
    // Domain guard: every relax variable is an UNSIGNED integer scalar (u32/u64/
    // usize), so the faithful real relaxation restricts each to the non-negative
    // reals. This keeps the route sound (ℤ≥0 ⊆ ℝ≥0, so real-validity-on-the-guarded-
    // domain still implies integer-validity per r_relax_sound) and avoids a spurious
    // countermodel at a negative real the unsigned domain never reaches.
    for v in integer_vars(f) {
        s.push_str(&format!("(assert (>= {v} 0.0))\n"));
    }
    // The precondition (the hypothesis of the universally-quantified implication).
    let req = render_prop_smt(&f.contract.req.expr)?;
    s.push_str(&format!("(assert {req})\n"));
    // The negation of the conjoined conclusion: ¬(⋀ ens) = (or ¬ens0 ¬ens1 …).
    let mut neg_ens = Vec::with_capacity(f.contract.ens.len());
    for ens in &f.contract.ens {
        neg_ens.push(format!("(not {})", render_prop_smt(&ens.expr)?));
    }
    let neg_conj = if neg_ens.len() == 1 {
        neg_ens.remove(0)
    } else {
        format!("(or {})", neg_ens.join(" "))
    };
    s.push_str(&format!("(assert {neg_conj})\n"));
    // The direct Z3 nlsat tactic over QF_NRA (Q-NLSAT) — not the default solver.
    s.push_str("(check-sat-using qfnra-nlsat)\n");
    s.push_str("(get-model)\n");
    Some(s)
}

/// The exact input passed to Z3 for a real-relaxation validity check.
#[must_use]
pub fn nlsat_solver_input(f: &FnItem) -> Option<String> {
    negated_contract_query(f).map(|query| format!("(set-option :pp.decimal true)\n{query}"))
}

// ─────────────────────────────────────────────────────────────────────────────
// The integer evaluator (REQ-8c, the integrality check Q8).
// ─────────────────────────────────────────────────────────────────────────────

/// Evaluate a relaxable polynomial TERM over an integer assignment. Returns `None`
/// on an unbound variable or an arithmetic overflow (`i128` checked) — an
/// inconclusive point the integrality check skips rather than mis-reading (R-CODE-4).
#[must_use]
pub fn eval_term_int(e: &Expr, assign: &BTreeMap<String, i128>) -> Option<i128> {
    match e {
        Expr::IntLit { value, .. } => i128::try_from(*value).ok(),
        Expr::Path(segs) if segs.len() == 1 => assign.get(&segs[0]).copied(),
        Expr::Binary { op, lhs, rhs } => {
            let l = eval_term_int(lhs, assign)?;
            let r = eval_term_int(rhs, assign)?;
            match op {
                BinOp::Add => l.checked_add(r),
                BinOp::Sub => l.checked_sub(r),
                BinOp::Mul => l.checked_mul(r),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Evaluate a relaxable proposition over an integer assignment. Returns `None` if any
/// sub-term is inconclusive (unbound / overflow). The comparisons and connectives
/// fold over [`eval_term_int`] / themselves.
#[must_use]
pub fn eval_prop_int(e: &Expr, assign: &BTreeMap<String, i128>) -> Option<bool> {
    match e {
        Expr::BoolLit(b) => Some(*b),
        Expr::Binary { op, lhs, rhs } => match op {
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                let l = eval_term_int(lhs, assign)?;
                let r = eval_term_int(rhs, assign)?;
                Some(match op {
                    BinOp::Eq => l == r,
                    BinOp::Ne => l != r,
                    BinOp::Lt => l < r,
                    BinOp::Le => l <= r,
                    BinOp::Gt => l > r,
                    BinOp::Ge => l >= r,
                    _ => unreachable!(),
                })
            }
            BinOp::And => Some(eval_prop_int(lhs, assign)? && eval_prop_int(rhs, assign)?),
            BinOp::Or => Some(eval_prop_int(lhs, assign)? || eval_prop_int(rhs, assign)?),
            _ => None,
        },
        Expr::Unary {
            op: UnaryOp::Not,
            expr,
        } => Some(!eval_prop_int(expr, assign)?),
        _ => None,
    }
}

/// The integrality check (`.design/stage1-forge-tier.md` REQ-8c / Q8): does the
/// contract negation `req ∧ ¬(⋀ ens)` hold at the integer point `assign`? `true`
/// means `assign` is a **integer counterexample** to the contract (a real
/// `Counterexample`); `false` means the contract holds there. `None` is inconclusive
/// (unbound / overflow). Used to test the radius-2 ℤⁿ box rounded from a real
/// countermodel: if no integer point in the box returns `true`, the real countermodel
/// is real-only (a `RealWitness`, not a `Counterexample`).
#[must_use]
pub fn eval_contract_negation_over_ints(
    f: &FnItem,
    assign: &BTreeMap<String, i128>,
) -> Option<bool> {
    // req ∧ ¬(⋀ ens) = req ∧ (∃k. ¬ens_k).
    if !eval_prop_int(&f.contract.req.expr, assign)? {
        return Some(false);
    }
    let mut any_ens_violated = false;
    for ens in &f.contract.ens {
        if !eval_prop_int(&ens.expr, assign)? {
            any_ens_violated = true;
        }
    }
    Some(any_ens_violated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use thermite_syntax::{Item, Program};

    fn parse_one(src: &str) -> Program {
        let parsed = thermite_syntax::parse(src);
        assert!(parsed.is_clean(), "fixture must parse: {:?}", parsed.errors);
        parsed.program
    }

    fn fn_item<'a>(p: &'a Program, name: &str) -> &'a FnItem {
        p.items
            .iter()
            .find_map(|i| match i {
                Item::Fn(f) if f.name == name => Some(f),
                _ => None,
            })
            .expect("fn present")
    }

    // AC-12: `relaxable` admits the isqrt postconditions — a polynomial contract
    // (`result*result <= n` ∧ `n < (result+1)*(result+1)`), integer-scalar params +
    // result, no div/mod/shifts/casts.
    #[test]
    fn relaxable_admits_isqrt_postconditions() {
        let p = parse_one(
            "fn isqrt(n: u64) -> u64\n  req true\n  \
             ens result * result <= n\n  \
             ens n < (result + 1) * (result + 1)\n  fx pure\n{ n }\n",
        );
        let f = fn_item(&p, "isqrt");
        assert_eq!(
            classify_fn(f),
            RelaxVerdict::Relaxable,
            "the isqrt postconditions are relaxable (polynomial, integer-scalar)"
        );
        // Each individual ens clause is relaxable too.
        for ens in &f.contract.ens {
            assert!(classify_clause(ens).is_relaxable());
        }
    }

    // AC-12: `relaxable` rejects a div-containing clause (`%`, `<<`, casts likewise).
    #[test]
    fn relaxable_rejects_div_mod_shift_cast() {
        let div =
            parse_one("fn g(n: u64) -> u64\n  req true\n  ens result == n / 2\n  fx pure\n{ n }\n");
        let v = classify_fn(fn_item(&div, "g"));
        assert!(!v.is_relaxable(), "a `/` clause is NOT relaxable");
        match v {
            RelaxVerdict::NotRelaxable(r) => assert!(r.contains("division"), "names `/`: {r}"),
            RelaxVerdict::Relaxable => unreachable!(),
        }

        let rem =
            parse_one("fn g(n: u64) -> u64\n  req true\n  ens result == n % 2\n  fx pure\n{ n }\n");
        assert!(
            !classify_fn(fn_item(&rem, "g")).is_relaxable(),
            "`%` rejected"
        );

        let shl = parse_one(
            "fn g(n: u64) -> u64\n  req true\n  ens result == n << 1\n  fx pure\n{ n }\n",
        );
        assert!(
            !classify_fn(fn_item(&shl, "g")).is_relaxable(),
            "`<<` rejected"
        );

        let cast = parse_one(
            "fn g(n: u64) -> u64\n  req true\n  ens result as u32 == n as u32\n  fx pure\n{ n }\n",
        );
        assert!(
            !classify_fn(fn_item(&cast, "g")).is_relaxable(),
            "a cast `as` is rejected"
        );
    }

    // A non-integer-scalar parameter (a slice) is out of fragment — the quantifier
    // domain must be integer-scalar.
    #[test]
    fn relaxable_rejects_non_integer_param() {
        let p = parse_one(
            "fn h(xs: &[u32]) -> u64\n  req true\n  ens result == result\n  fx pure\n{ 0 }\n",
        );
        let v = classify_fn(fn_item(&p, "h"));
        assert!(!v.is_relaxable(), "a slice parameter is not relaxable");
    }

    // The `∀ n. n*n != 2` shape (the true-over-ℤ / false-over-ℝ example, AC-12) is
    // relaxable: a `≠` of polynomial terms.
    #[test]
    fn relaxable_admits_n_squared_ne_two() {
        let p = parse_one("fn sq(n: u64) -> u64\n  req true\n  ens n * n != 2\n  fx pure\n{ n }\n");
        let f = fn_item(&p, "sq");
        assert_eq!(classify_fn(f), RelaxVerdict::Relaxable);
        // It renders to a QF_NRA query negating `n*n != 2` → `(= (* n n) 2.0)`.
        let q = negated_contract_query(f).expect("renders");
        assert!(q.contains("(set-logic QF_NRA)"), "QF_NRA logic: {q}");
        assert!(q.contains("qfnra-nlsat"), "uses the nlsat tactic: {q}");
        assert!(
            q.contains("(= (* n n) 2.0)"),
            "the negation of `n*n != 2` is the equality: {q}"
        );
    }

    // The integer evaluator + the integrality check: at the real point √2 the
    // negation `n*n = 2` holds over ℝ, but no nearby integer (the radius-2 box
    // {-1,0,1,2,3} around 1.41) satisfies `n*n = 2` → real-only (a RealWitness, not a
    // Counterexample).
    #[test]
    fn integrality_check_n_squared_ne_two_is_real_only() {
        let p = parse_one("fn sq(n: u64) -> u64\n  req true\n  ens n * n != 2\n  fx pure\n{ n }\n");
        let f = fn_item(&p, "sq");
        for n in -1..=3i128 {
            let mut a = BTreeMap::new();
            a.insert("n".to_string(), n);
            a.insert("result".to_string(), n);
            assert_eq!(
                eval_contract_negation_over_ints(f, &a),
                Some(false),
                "no integer n in the radius-2 box falsifies `n*n != 2`"
            );
        }
    }

    // An integer counterexample IS caught: `ens result == n + 1` with the body
    // returning n is false at, e.g., result=n → the negation holds over ℤ.
    #[test]
    fn integrality_check_catches_integer_counterexample() {
        let p = parse_one(
            "fn bad(n: u64) -> u64\n  req true\n  ens result == n + 1\n  fx pure\n{ n }\n",
        );
        let f = fn_item(&p, "bad");
        let mut a = BTreeMap::new();
        a.insert("n".to_string(), 5);
        a.insert("result".to_string(), 5); // result == n, but ens wants n+1
        assert_eq!(
            eval_contract_negation_over_ints(f, &a),
            Some(true),
            "result=5, n=5 falsifies `result == n+1` → a genuine integer counterexample"
        );
    }

    // The QF_NRA encoding of the isqrt contract is well-formed: declares the vars,
    // asserts req, asserts the negated conjunction, asks nlsat.
    #[test]
    fn isqrt_query_is_well_formed() {
        let p = parse_one(
            "fn isqrt(n: u64) -> u64\n  req true\n  \
             ens result * result <= n\n  \
             ens n < (result + 1) * (result + 1)\n  fx pure\n{ n }\n",
        );
        let f = fn_item(&p, "isqrt");
        let q = negated_contract_query(f).expect("isqrt renders");
        assert!(q.contains("(declare-const n Real)"));
        assert!(q.contains("(declare-const result Real)"));
        // Two ens clauses → the negation is a disjunction of their negations.
        assert!(
            q.contains("(or (not"),
            "negated conjunction is a disjunction: {q}"
        );
        assert!(q.contains("(check-sat-using qfnra-nlsat)"));
    }
}
