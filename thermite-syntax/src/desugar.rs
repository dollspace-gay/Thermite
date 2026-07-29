//! Post-parse desugaring passes (`.design/stage1-forge-tier.md` REQ-3, the forge
//! tier). This is the new post-parse pass the refinement-type sugar resolution
//! (Q-DECWF note) calls for: none existed in `thermite-syntax` before. It runs at
//! the end of [`crate::parse`], after the recursive-descent parser has built the
//! `Program`, and rewrites the surface sugar into the v1 clause shapes so every
//! downstream stage (`thermite-spec` validation, `thermite-lower` lowering, `forge`)
//! sees only the v1 contract grammar plus the new forge-tier item kinds.
//!
//! ## Refinement-type sugar (`x: T{P}` / `-> T{P}`)
//!
//! A refined parameter `x: T{P}` says "the argument is a `T` satisfying `P`"; a
//! refined return `-> T{P}` says "the result is a `T` satisfying `P`". The parser
//! captures the predicates on [`crate::ast::FnItem::refinements`] (transient). This
//! pass folds them into the function's mandatory v1 contract:
//!
//! - a PARAMETER refinement `P` becomes a `req` conjunct (`req` ← `req && P`). The
//!   precondition is the function's own assumption and — because Verus enforces a
//!   callee's `req` at every call site — the refinement automatically becomes the
//!   caller's proof obligation (the "call-site obligations" of REQ-3), with no
//!   separate mechanism.
//! - a return refinement `P` becomes a new `ens` clause (the result postcondition).
//!
//! After folding, `refinements` is cleared, so the sugar is invisible downstream.

use crate::ast::{BinOp, Clause, Expr, Item, Program, Refinement, RefinementTarget};

/// Run every post-parse desugaring pass over `program` in place. Called once by
/// [`crate::parse`] after parsing. Currently: refinement-type sugar.
pub fn desugar(program: &mut Program) {
    desugar_refinements(program);
}

/// Fold the refinement-type sugar captured on each `fn` into its v1 contract
/// (`.design/stage1-forge-tier.md` REQ-3), then clear the transient store. A
/// parameter refinement becomes a `req` conjunct; a return refinement becomes an
/// `ens` clause. Idempotent on an already-desugared program (no `refinements` left
/// to fold). Deterministic (R-CODE-5): refinements are folded in source order.
pub fn desugar_refinements(program: &mut Program) {
    for item in &mut program.items {
        let Item::Fn(f) = item else { continue };
        if f.refinements.is_empty() {
            continue;
        }
        // Take the refinements out so we can fold them by value (and so the field
        // is left empty — the post-condition downstream relies on).
        let refinements = std::mem::take(&mut f.refinements);
        for Refinement { target, pred } in refinements {
            match target {
                // A parameter refinement strengthens the precondition: `req && P`.
                RefinementTarget::Param(_) => {
                    f.contract.req = conjoin(f.contract.req.clone(), pred);
                }
                // A return refinement is a new postcondition clause.
                RefinementTarget::Result => {
                    f.contract.ens.push(pred);
                }
            }
        }
    }
}

/// Conjoin two clauses into `a && b` (a fresh [`Clause`] whose `expr` is the
/// `BinOp::And` of the two, whose `text` is the parenthesized concatenation, and
/// whose `span` covers both). Used to fold a parameter refinement into `req`.
fn conjoin(a: Clause, b: Clause) -> Clause {
    let text = format!("({}) && ({})", a.text, b.text);
    let span = a.span.to(b.span);
    // Preserve a `@bv` tag if either side carried one (a refinement predicate
    // never does, so in practice this keeps the original clause's tag, REQ-1).
    let bv = a.bv.or(b.bv);
    Clause {
        expr: Expr::Binary {
            op: BinOp::And,
            lhs: Box::new(a.expr),
            rhs: Box::new(b.expr),
        },
        text,
        span,
        bv,
    }
}
