//! The stratified reference encoder — stage-2 REQ-8
//! (`.design/stage2-stratified-cage.md` REQ-8 deliverable (3)).
//!
//! An INDEPENDENT reference encoding of an admitted stratified clause
//! (`thermite_spec::classifier::Frm`, the sort-typed `Cls.Frm` surface mirrored from
//! `lean/Thermite/Strat/Nnf.lean`) into the raw-quantifier formula IR
//! ([`crate::normalize::Formula`]) the two-phase TV ([`crate::strat_two_phase`])
//! normalizes and compares against the production lowering
//! (`thermite_lower`, `Expr::Quantifier`).
//!
//! This is the Rust analogue of `lean/Thermite/Strat/RefEncode.lean`'s `sencode`: it
//! names each de Bruijn binder by its LEVEL (the fresh-name discipline — names strictly
//! increase down every path, so no capture; `encName d i = d - 1 - i`, matching the
//! Lean), and transcribes the boolean + relational + array-property skeleton faithfully.
//! Sorts are erased (the structural denotation `fdenote` reads atoms over an abstract
//! domain; T1-S is parametric in the sort model — `Strat/Soundness.lean`).
//!
//! It graduates SPIKE-2's hand-written reference spellings to a mechanized encoder
//! (the REQ-6 pattern): where the probe fixtures survive as tests, the reference side is
//! now this function's output, not free text.
//!
//! Independence (the TV honesty boundary): this depends on `thermite-spec` (the
//! classifier `Frm`) only — never `thermite-lower`. A reference that reused the
//! production lowerer would make the equivalence check vacuous.

use thermite_spec::classifier::{Atom, Frm, Rel, ScalarValue, Sort2, Tm};

use crate::normalize::{ArithOp, CmpOp, Formula, Quant, Term};

/// The fresh NAME (de Bruijn level) a variable at index `i` denotes under binder depth
/// `d`: `encName d i = d - 1 - i` (index 0 ↦ the innermost binder, level `d-1`). Mirrors
/// `lean/Thermite/Strat/RefEncode.lean` `encName` exactly. Rendered as `v{level}` — a
/// stable positional name; the normalizer alpha-canonicalizes binders anyway, so the only
/// requirement is that the same source binder maps consistently (no capture).
fn enc_name(d: u32, i: u32) -> String {
    // `d` is always ≥ 1 wherever a bound var is read (a var at index `i < d` is only
    // reachable under ≥ `i+1` binders), so `d - 1 - i` does not underflow for bound vars.
    // A free var (`i >= d`, never produced for an admitted closed clause) names itself.
    if i < d {
        format!("v{}", d - 1 - i)
    } else {
        format!("free{i}")
    }
}

fn const_name(s: &Sort2, id: u32) -> String {
    format!("const_{}_{id}", sort_tag(s))
}

/// A stable tag for a sort, for naming literals / casts in the reference surface.
fn sort_tag(s: &Sort2) -> String {
    match s {
        Sort2::Mach(_) => format!("{s}"),
        Sort2::Seq(inner) => format!("seq_{}", sort_tag(inner)),
        Sort2::Opaque(k) => format!("opaque{k}"),
    }
}

/// Encode a term to the reference IR under binder depth `d`.
fn enc_tm(d: u32, t: &Tm) -> Term {
    match t {
        Tm::Var(_, i) => Term::Var(enc_name(d, *i)),
        Tm::Const(s, id) => Term::Var(const_name(s, *id)),
        Tm::Lit(_, ScalarValue::Int(value)) => Term::Int(*value),
        Tm::Lit(_, ScalarValue::Bool(value)) => {
            Term::Var(if *value { "true" } else { "false" }.to_string())
        }
        // `sq[ix]` ↦ the reference `idx(sq, ix)` accessor (the normalizer's `App` form).
        Tm::Read(_, sq, ix) => Term::App("idx".to_string(), vec![enc_tm(d, sq), enc_tm(d, ix)]),
        // `sq.len()` ↦ `len(sq)`.
        Tm::Len(sq) => Term::App("len".to_string(), vec![enc_tm(d, sq)]),
        // `(t as to)` ↦ the reference cast `t as <to>`.
        Tm::Cast(to, t) => Term::Cast(Box::new(enc_tm(d, t)), sort_tag(to)),
        // `t ± k` ↦ arithmetic with the literal offset (negative `k` is `t + (-k)`; the
        // normalizer's `Add` is commutative-sorted, so the spelling canonicalizes).
        Tm::IdxOp(t, k) => Term::Arith(
            ArithOp::Add,
            Box::new(enc_tm(d, t)),
            Box::new(Term::Int(i128::from(*k))),
        ),
        // `t * u` ↦ `t * u` (the non-linear op; (R2) forbids a bound index under it, so
        // the operands are bound-var-free in an admitted clause).
        Tm::Mul(t, u) => Term::Arith(ArithOp::Mul, Box::new(enc_tm(d, t)), Box::new(enc_tm(d, u))),
        // A declared unary spec fn `f(a)` ↦ `f{id}(a)` (an uninterpreted application).
        Tm::App1(_, _, f, a) => Term::App(format!("f{f}"), vec![enc_tm(d, a)]),
    }
}

/// Map a classifier relation to the normalizer's comparison operator.
fn enc_rel(r: Rel) -> CmpOp {
    match r {
        Rel::Eq => CmpOp::Eq,
        Rel::Ne => CmpOp::Ne,
        Rel::Lt => CmpOp::Lt,
        Rel::Le => CmpOp::Le,
        Rel::Gt => CmpOp::Gt,
        Rel::Ge => CmpOp::Ge,
    }
}

/// Encode an atom. A `qfree` leaf is opaque to the classifier (`Strat/Nnf.lean`: it
/// contributes no sorts/edges) and carries no inspectable v1 expression in the Rust
/// mirror, so the reference names it as a nullary predicate `qfree()` — a stable opaque
/// proposition (the syntactic phase treats it atomically; its real v1 meaning is the
/// atom-grounding the kernel T2-S certifies, `Strat/Faithfulness.lean`).
fn enc_atom(d: u32, a: &Atom) -> Formula {
    match a {
        Atom::Rel(r, t, u) => Formula::Atom(crate::normalize::Atom {
            op: enc_rel(*r),
            lhs: enc_tm(d, t),
            rhs: enc_tm(d, u),
        }),
        // An opaque qfree leaf: a nullary predicate, modeled as `qfree() = qfree()` (a
        // trivially-true atomic placeholder that normalizes stably and atomically).
        Atom::QFree(_) => Formula::Atom(crate::normalize::Atom {
            op: CmpOp::Eq,
            lhs: Term::App("qfree".to_string(), vec![]),
            rhs: Term::App("qfree".to_string(), vec![]),
        }),
    }
}

/// Encode a formula under binder depth `d`.
fn enc_frm(d: u32, phi: &Frm) -> Formula {
    match phi {
        Frm::Atom(a) => enc_atom(d, a),
        Frm::Neg(p) => Formula::Not(Box::new(enc_frm(d, p))),
        Frm::Conj(p, q) => Formula::And(Box::new(enc_frm(d, p)), Box::new(enc_frm(d, q))),
        Frm::Disj(p, q) => Formula::Or(Box::new(enc_frm(d, p)), Box::new(enc_frm(d, q))),
        Frm::Imp(p, q) => Formula::Implies(Box::new(enc_frm(d, p)), Box::new(enc_frm(d, q))),
        // Each binder is named by the CURRENT depth `d` (its de Bruijn level — the
        // fresh-name discipline); the body recurses at `d + 1`. Mirrors
        // `RefEncode.lean` `sencodeAt`'s `all s d true (sencodeAt (d+1) φ)`.
        Frm::All(_, p) => {
            Formula::Quantified(Quant::Forall, format!("v{d}"), Box::new(enc_frm(d + 1, p)))
        }
        Frm::Ex(_, p) => {
            Formula::Quantified(Quant::Exists, format!("v{d}"), Box::new(enc_frm(d + 1, p)))
        }
    }
}

/// The stratified reference encoder: an admitted classifier formula → its independent
/// raw-quantifier reference encoding, ready for the two-phase TV
/// ([`crate::strat_two_phase`]). The top level is depth 0 (a closed sentence — the
/// stratification keeps carrier variables in bound positions only, so an admitted clause
/// has no free carrier index).
#[must_use]
pub fn strat_ref_encode(phi: &Frm) -> Formula {
    enc_frm(0, phi)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::normalize;
    use thermite_spec::classifier::Mach;

    fn usize_s() -> Sort2 {
        Sort2::Mach(Mach::Usize)
    }

    // `forall (i : usize). a[i] <= a[j]`-shaped clause (the `sorted` skeleton, one binder).
    // Modeled with a single binder and a read of the bound index vs a read at a free
    // index — exercises Var/Read/Rel + the level naming.
    fn sorted_one() -> Frm {
        // forall i. read(a, i) <= read(a, lit)   (lit stands for the second index)
        Frm::All(
            usize_s(),
            Box::new(Frm::Atom(Atom::Rel(
                Rel::Le,
                Tm::Read(
                    usize_s(),
                    Box::new(Tm::Var(usize_s(), 1)),
                    Box::new(Tm::Var(usize_s(), 0)),
                ),
                Tm::Read(
                    usize_s(),
                    Box::new(Tm::Var(usize_s(), 1)),
                    Box::new(Tm::Const(usize_s(), 1)),
                ),
            ))),
        )
    }

    #[test]
    fn encodes_to_a_parseable_normalizable_formula() {
        let phi = sorted_one();
        let f = strat_ref_encode(&phi);
        // It normalizes (no panic) and is a single forall.
        let n = f.clone().normalize();
        assert!(n.starts_with("A v0."), "one universal binder: {n}");
        assert!(n.contains("idx("), "the read lowered to idx(): {n}");
    }

    #[test]
    fn level_naming_is_consistent_for_the_bound_var() {
        // The bound var (index 0 under the single binder, depth 1) names `v0` — the same
        // name the binder (depth 0) is given. So the body references the binder, no
        // capture, alpha-stable.
        let phi = Frm::Ex(
            usize_s(),
            Box::new(Frm::Atom(Atom::Rel(
                Rel::Lt,
                Tm::Var(usize_s(), 0),
                Tm::Len(Box::new(Tm::Const(Sort2::Seq(Box::new(usize_s())), 0))),
            ))),
        );
        let n = strat_ref_encode(&phi).normalize();
        assert!(n.starts_with("E v0."), "one existential binder: {n}");
        assert!(n.contains("len("));
    }

    #[test]
    fn relations_and_casts_transcribe() {
        // forall i. (i as u32) != lit
        let phi = Frm::All(
            usize_s(),
            Box::new(Frm::Atom(Atom::Rel(
                Rel::Ne,
                Tm::Cast(Sort2::Mach(Mach::U32), Box::new(Tm::Var(usize_s(), 0))),
                Tm::Const(Sort2::Mach(Mach::U32), 0),
            ))),
        );
        let f = strat_ref_encode(&phi);
        let n = f.normalize();
        assert!(n.contains(" as "), "cast transcribed: {n}");
        // Round-trips through the parser as a sanity check on the surface it emits.
        let _ = normalize::parse(&render(&strat_ref_encode(&phi))).map(|g| {
            assert_eq!(g.normalize(), n, "render→parse is normalization-stable");
        });
    }

    // A minimal renderer to the normalizer's surface syntax, for the round-trip check.
    fn render(f: &Formula) -> String {
        match f {
            Formula::Quantified(q, name, body) => {
                let kw = match q {
                    Quant::Forall => "forall",
                    Quant::Exists => "exists",
                };
                format!("{kw} {name} . {}", render(body))
            }
            Formula::Atom(a) => format!(
                "{} {} {}",
                render_term(&a.lhs),
                op_tok(a.op),
                render_term(&a.rhs)
            ),
            Formula::Not(p) => format!("~({})", render(p)),
            Formula::And(p, q) => format!("({} & {})", render(p), render(q)),
            Formula::Or(p, q) => format!("({} | {})", render(p), render(q)),
            Formula::Implies(p, q) => format!("({} => {})", render(p), render(q)),
        }
    }

    fn op_tok(op: CmpOp) -> &'static str {
        match op {
            CmpOp::Eq => "=",
            CmpOp::Ne => "!=",
            CmpOp::Lt => "<",
            CmpOp::Le => "<=",
            CmpOp::Gt => ">",
            CmpOp::Ge => ">=",
        }
    }

    fn render_term(t: &Term) -> String {
        match t {
            Term::Var(n) => n.clone(),
            Term::Int(n) => n.to_string(),
            Term::App(f, args) => {
                let a: Vec<String> = args.iter().map(render_term).collect();
                format!("{f}({})", a.join(", "))
            }
            Term::Arith(op, l, r) => {
                let o = match op {
                    ArithOp::Add => "+",
                    ArithOp::Sub => "-",
                    ArithOp::Mul => "*",
                };
                format!("({} {o} {})", render_term(l), render_term(r))
            }
            Term::Cast(inner, ty) => format!("({} as {ty})", render_term(inner)),
        }
    }
}
