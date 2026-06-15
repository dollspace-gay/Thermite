//! SPIKE-2 — the prototype normalizer probe (`.design/m0-spikes.md` REQ-6).
//!
//! ## What this is, and is not
//!
//! This is an experimental, leaf module: it implements the four layer-1
//! passes of the stage-2 two-phase TV normalizer (metatheory sketch §8.2 layer 1)
//! — NNF, prenex, canonical bound-name / de-Bruijn form, atom ordering — over a
//! tiny raw-quantifier formula language, so SPIKE-2 can measure the
//! syntactic-equality hit rate of a normalizer over the conformance corpus's
//! combinator contracts in raw-quantifier form (risk row 3, fallback F-C). It is
//! exported but referenced by no TV pipeline code path (REQ-6 / AC-6): the
//! only consumers are this module's own `#[cfg(test)]` unit tests and the
//! `tests/strat_probe.rs` hit-rate target. The stage-2 normalizer evolves this
//! module in place once it is wired in behind `nnf_sound`/`prenex_sound` lemmas,
//! which are out of scope for the spike (the spike wants the number, not the
//! soundness proof).
//!
//! ## The four passes (metatheory §8.2 layer 1)
//!
//! 1. NNF ([`Formula::to_nnf`]): eliminate `=>`, push `~` to the atoms via De
//!    Morgan + quantifier duality, and fold the residual atom-negations into the
//!    comparison operator (`~(a < b)` becomes `a >= b`). The result is `Not`-free
//!    and `Implies`-free.
//! 2. Prenex ([`prenex`]): alpha-rename every binder to a globally-fresh name
//!    (so hoisting can never capture), then pull all quantifiers to the front,
//!    leaving a quantifier-free matrix.
//! 3. Canonical bound-name / de-Bruijn form + 4. atom ordering
//!    ([`canonical`]): commutative-associative `&`/`|` are flattened and their
//!    children sorted; symmetric atoms (`=`/`!=`) and the flip-equivalent
//!    comparisons (`>`/`>=` rewritten to `<`/`<=`) are oriented canonically; and
//!    the binders within each maximal same-quantifier block — which commute
//!    (`forall i j ≡ forall j i`) — are renamed to canonical `v0,v1,…` names by
//!    choosing the binder permutation that minimizes the serialized matrix. The
//!    canonical names are de-Bruijn-style position names: two alpha-equivalent
//!    formulas serialize identically.
//!
//! Two formulas are judged equal ([`equivalent`]) iff their [`Formula::normalize`]
//! canonical serializations are byte-identical — the "syntactic equality after
//! normalization" the spike measures. There is no soundness lemma: a hit is
//! evidence the two spellings converge under layer-1 normalization, nothing more.

use std::fmt::Write as _;

// ===========================================================================
// The raw-quantifier formula language (the S₂ matrix shape the normalizer eats)
// ===========================================================================

/// A comparison operator on two integer-valued terms. The six surface
/// comparisons; `>`/`>=` are flip-equivalents of `<`/`<=` and are oriented away
/// in the canonical pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl CmpOp {
    /// The token spelling (used in canonical serialization + parse errors).
    fn token(self) -> &'static str {
        match self {
            CmpOp::Eq => "=",
            CmpOp::Ne => "!=",
            CmpOp::Lt => "<",
            CmpOp::Le => "<=",
            CmpOp::Gt => ">",
            CmpOp::Ge => ">=",
        }
    }

    /// The operator of the negated comparison (`~(a < b) ≡ a >= b`). Used by NNF
    /// to fold an atom-negation into the operator so the result is `Not`-free.
    fn negate(self) -> CmpOp {
        match self {
            CmpOp::Eq => CmpOp::Ne,
            CmpOp::Ne => CmpOp::Eq,
            CmpOp::Lt => CmpOp::Ge,
            CmpOp::Le => CmpOp::Gt,
            CmpOp::Gt => CmpOp::Le,
            CmpOp::Ge => CmpOp::Lt,
        }
    }

    /// The operator with the operands swapped (`a > b ≡ b < a`). Used to orient
    /// `>`/`>=` to `<`/`<=` canonically.
    fn flip(self) -> CmpOp {
        match self {
            CmpOp::Eq => CmpOp::Eq,
            CmpOp::Ne => CmpOp::Ne,
            CmpOp::Lt => CmpOp::Gt,
            CmpOp::Le => CmpOp::Ge,
            CmpOp::Gt => CmpOp::Lt,
            CmpOp::Ge => CmpOp::Le,
        }
    }
}

/// An arithmetic operator on terms (`+`/`-`/`*`). `+`/`*` are commutative — their
/// operands are sorted in the canonical pass; `-` is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ArithOp {
    Add,
    Sub,
    Mul,
}

impl ArithOp {
    fn token(self) -> &'static str {
        match self {
            ArithOp::Add => "+",
            ArithOp::Sub => "-",
            ArithOp::Mul => "*",
        }
    }

    fn is_commutative(self) -> bool {
        matches!(self, ArithOp::Add | ArithOp::Mul)
    }
}

/// An integer-valued term. The vocabulary the bounded-quantifier combinator
/// expansions need: variables (free vars stay verbatim, bound vars are
/// canonicalized), literals, the sequence accessors `len`/`idx`, generic spec-fn
/// applications, arithmetic, and the spec casts (`x as u32`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Term {
    /// A variable reference (`i`, `xs`, `needle`). A binder-bound name is renamed
    /// by the canonical pass; a free name is preserved.
    Var(String),
    /// An integer literal.
    Int(i128),
    /// A function-like application — `len(s)`, `idx(s, i)`, or a named spec-fn
    /// call `f(a, b)`. `name` is the head symbol; `args` the (ordered) operands.
    App(String, Vec<Term>),
    /// `lhs <op> rhs` arithmetic.
    Arith(ArithOp, Box<Term>, Box<Term>),
    /// A spec cast `inner as ty` (`x as u32`). `ty` is the target spelling.
    Cast(Box<Term>, String),
}

/// An atom: a single comparison between two terms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Atom {
    pub op: CmpOp,
    pub lhs: Term,
    pub rhs: Term,
}

/// The quantifier kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quant {
    Forall,
    Exists,
}

/// A raw-quantifier formula. Binary `&`/`|` are flattened to n-ary by the
/// canonical pass; quantifiers are single-binder (a multi-binder `forall i j`
/// desugars to nested `forall i. forall j.` at parse time).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Formula {
    Atom(Atom),
    Not(Box<Formula>),
    And(Box<Formula>, Box<Formula>),
    Or(Box<Formula>, Box<Formula>),
    Implies(Box<Formula>, Box<Formula>),
    Quantified(Quant, String, Box<Formula>),
}

// ===========================================================================
// Parser — a small recursive-descent reader of the fixture surface syntax
// ===========================================================================
//
// Grammar (precedence low → high):
//   formula  := ('forall'|'exists') ident+ '.' formula
//             | implies
//   implies  := or ('=>' implies)?                 (right-assoc)
//   or       := and ('|' and)*
//   and      := unary ('&' unary)*
//   unary    := '~' unary | '(' formula ')' | atom
//   atom     := term (cmpop term)+                 (chains desugar to a conjunction)
//   term     := add ('as' ident)?
//   add      := mul (('+'|'-') mul)*
//   mul      := postfix ('*' postfix)*
//   postfix  := ident '(' term (',' term)* ')'     (application)
//             | ident                              (variable)
//             | int
//             | '(' term ')'

/// A parse error with a human-readable reason (the probe surfaces these in test
/// failures rather than panicking on a malformed fixture).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError(pub String);

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "parse error: {}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Tok {
    Ident(String),
    Int(i128),
    LParen,
    RParen,
    Comma,
    Dot,
    Amp,
    Pipe,
    Tilde,
    Implies,
    Cmp(CmpOp),
    Plus,
    Minus,
    Star,
}

fn lex(src: &str) -> Result<Vec<Tok>, ParseError> {
    let bytes = src.as_bytes();
    let mut i = 0;
    let mut out = Vec::new();
    while i < bytes.len() {
        let c = bytes[i] as char;
        match c {
            ' ' | '\t' | '\n' | '\r' => {
                i += 1;
            }
            '(' => {
                out.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                out.push(Tok::RParen);
                i += 1;
            }
            ',' => {
                out.push(Tok::Comma);
                i += 1;
            }
            '.' => {
                out.push(Tok::Dot);
                i += 1;
            }
            '&' => {
                out.push(Tok::Amp);
                i += 1;
            }
            '|' => {
                out.push(Tok::Pipe);
                i += 1;
            }
            '~' => {
                out.push(Tok::Tilde);
                i += 1;
            }
            '+' => {
                out.push(Tok::Plus);
                i += 1;
            }
            '*' => {
                out.push(Tok::Star);
                i += 1;
            }
            '-' => {
                out.push(Tok::Minus);
                i += 1;
            }
            '=' => {
                if bytes.get(i + 1) == Some(&b'>') {
                    out.push(Tok::Implies);
                    i += 2;
                } else {
                    out.push(Tok::Cmp(CmpOp::Eq));
                    i += 1;
                }
            }
            '!' => {
                if bytes.get(i + 1) == Some(&b'=') {
                    out.push(Tok::Cmp(CmpOp::Ne));
                    i += 2;
                } else {
                    return Err(ParseError("bare '!' (use '~' for not, '!=' for ne)".into()));
                }
            }
            '<' => {
                if bytes.get(i + 1) == Some(&b'=') {
                    out.push(Tok::Cmp(CmpOp::Le));
                    i += 2;
                } else {
                    out.push(Tok::Cmp(CmpOp::Lt));
                    i += 1;
                }
            }
            '>' => {
                if bytes.get(i + 1) == Some(&b'=') {
                    out.push(Tok::Cmp(CmpOp::Ge));
                    i += 2;
                } else {
                    out.push(Tok::Cmp(CmpOp::Gt));
                    i += 1;
                }
            }
            _ if c.is_ascii_digit() => {
                let start = i;
                while i < bytes.len() && (bytes[i] as char).is_ascii_digit() {
                    i += 1;
                }
                let n: i128 = src[start..i]
                    .parse()
                    .map_err(|_| ParseError(format!("bad integer '{}'", &src[start..i])))?;
                out.push(Tok::Int(n));
            }
            _ if c.is_ascii_alphabetic() || c == '_' => {
                let start = i;
                while i < bytes.len()
                    && ((bytes[i] as char).is_ascii_alphanumeric() || bytes[i] == b'_')
                {
                    i += 1;
                }
                out.push(Tok::Ident(src[start..i].to_string()));
            }
            other => return Err(ParseError(format!("unexpected character '{other}'"))),
        }
    }
    Ok(out)
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn bump(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn eat(&mut self, t: &Tok) -> bool {
        if self.peek() == Some(t) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, t: &Tok, what: &str) -> Result<(), ParseError> {
        if self.eat(t) {
            Ok(())
        } else {
            Err(ParseError(format!(
                "expected {what}, found {:?}",
                self.peek()
            )))
        }
    }

    fn parse_formula(&mut self) -> Result<Formula, ParseError> {
        if let Some(Tok::Ident(kw)) = self.peek() {
            let q = match kw.as_str() {
                "forall" => Some(Quant::Forall),
                "exists" => Some(Quant::Exists),
                _ => None,
            };
            if let Some(q) = q {
                self.bump();
                let mut names = Vec::new();
                while let Some(Tok::Ident(n)) = self.peek() {
                    names.push(n.clone());
                    self.bump();
                }
                if names.is_empty() {
                    return Err(ParseError("quantifier with no binders".into()));
                }
                self.expect(&Tok::Dot, "'.' after quantifier binders")?;
                let body = self.parse_formula()?;
                // `forall i j . body` desugars to `forall i. forall j. body`.
                let mut f = body;
                for n in names.into_iter().rev() {
                    f = Formula::Quantified(q, n, Box::new(f));
                }
                return Ok(f);
            }
        }
        self.parse_implies()
    }

    fn parse_implies(&mut self) -> Result<Formula, ParseError> {
        let lhs = self.parse_or()?;
        if self.eat(&Tok::Implies) {
            let rhs = self.parse_implies()?;
            Ok(Formula::Implies(Box::new(lhs), Box::new(rhs)))
        } else {
            Ok(lhs)
        }
    }

    fn parse_or(&mut self) -> Result<Formula, ParseError> {
        let mut lhs = self.parse_and()?;
        while self.eat(&Tok::Pipe) {
            let rhs = self.parse_and()?;
            lhs = Formula::Or(Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_and(&mut self) -> Result<Formula, ParseError> {
        let mut lhs = self.parse_unary()?;
        while self.eat(&Tok::Amp) {
            let rhs = self.parse_unary()?;
            lhs = Formula::And(Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> Result<Formula, ParseError> {
        if self.eat(&Tok::Tilde) {
            return Ok(Formula::Not(Box::new(self.parse_unary()?)));
        }
        if self.peek() == Some(&Tok::LParen) {
            // A leading `(` is ambiguous: a FORMULA group `(0 <= i & …)` or a
            // parenthesized TERM that starts an atom `(5 - n) <= i`. Try the
            // formula group first; if it does not parse and close, backtrack
            // and read the `(` as the first term of an atom.
            let save = self.pos;
            self.pos += 1; // consume the '('
            if let Ok(f) = self.parse_formula() {
                if self.eat(&Tok::RParen) {
                    return Ok(f);
                }
            }
            self.pos = save; // backtrack — it was a term, not a formula group
        }
        self.parse_atom()
    }

    fn parse_atom(&mut self) -> Result<Formula, ParseError> {
        let first = self.parse_term()?;
        let mut chain: Vec<(CmpOp, Term)> = Vec::new();
        while let Some(Tok::Cmp(op)) = self.peek() {
            let op = *op;
            self.bump();
            let t = self.parse_term()?;
            chain.push((op, t));
        }
        if chain.is_empty() {
            return Err(ParseError(
                "expected a comparison operator to form an atom".into(),
            ));
        }
        // A chain `t0 op1 t1 op2 t2` desugars to `(t0 op1 t1) & (t1 op2 t2)`.
        let mut prev = first;
        let mut acc: Option<Formula> = None;
        for (op, t) in chain {
            let atom = Formula::Atom(Atom {
                op,
                lhs: prev,
                rhs: t.clone(),
            });
            acc = Some(match acc {
                None => atom,
                Some(a) => Formula::And(Box::new(a), Box::new(atom)),
            });
            prev = t;
        }
        Ok(acc.expect("chain non-empty"))
    }

    fn parse_term(&mut self) -> Result<Term, ParseError> {
        let inner = self.parse_add()?;
        if let Some(Tok::Ident(kw)) = self.peek() {
            if kw == "as" {
                self.bump();
                if let Some(Tok::Ident(ty)) = self.bump() {
                    return Ok(Term::Cast(Box::new(inner), ty));
                }
                return Err(ParseError("expected a type after 'as'".into()));
            }
        }
        Ok(inner)
    }

    fn parse_add(&mut self) -> Result<Term, ParseError> {
        let mut lhs = self.parse_mul()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Plus) => ArithOp::Add,
                Some(Tok::Minus) => ArithOp::Sub,
                _ => break,
            };
            self.bump();
            let rhs = self.parse_mul()?;
            lhs = Term::Arith(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_mul(&mut self) -> Result<Term, ParseError> {
        let mut lhs = self.parse_postfix()?;
        while self.eat(&Tok::Star) {
            let rhs = self.parse_postfix()?;
            lhs = Term::Arith(ArithOp::Mul, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_postfix(&mut self) -> Result<Term, ParseError> {
        match self.bump() {
            Some(Tok::Int(n)) => Ok(Term::Int(n)),
            Some(Tok::LParen) => {
                let t = self.parse_add()?;
                self.expect(&Tok::RParen, "')'")?;
                Ok(t)
            }
            Some(Tok::Ident(name)) => {
                if self.eat(&Tok::LParen) {
                    let mut args = Vec::new();
                    if self.peek() != Some(&Tok::RParen) {
                        loop {
                            args.push(self.parse_term()?);
                            if !self.eat(&Tok::Comma) {
                                break;
                            }
                        }
                    }
                    self.expect(&Tok::RParen, "')' to close application")?;
                    Ok(Term::App(name, args))
                } else {
                    Ok(Term::Var(name))
                }
            }
            other => Err(ParseError(format!("expected a term, found {other:?}"))),
        }
    }
}

/// Parse the raw-quantifier surface syntax into a [`Formula`]. The grammar is
/// documented above; chains (`0 <= i <= j`) desugar to conjunctions and
/// multi-binder quantifiers (`forall i j`) desugar to nested single binders.
pub fn parse(src: &str) -> Result<Formula, ParseError> {
    let toks = lex(src)?;
    let mut p = Parser { toks, pos: 0 };
    let f = p.parse_formula()?;
    if p.pos != p.toks.len() {
        return Err(ParseError(format!(
            "trailing tokens after formula: {:?}",
            &p.toks[p.pos..]
        )));
    }
    Ok(f)
}

// ===========================================================================
// Pass 1 — NNF
// ===========================================================================

impl Formula {
    /// Negation normal form (metatheory §8.2 layer-1 pass 1): eliminate `=>`, push
    /// `~` inward via De Morgan + quantifier duality, fold atom-negations into the
    /// comparison operator. The result has no `Not` and no `Implies` node.
    pub fn to_nnf(self) -> Formula {
        self.nnf_inner(false)
    }

    /// `neg == true` means "the negation of `self`" is wanted (the `~` has been
    /// pushed down to this point). This single recursion both eliminates `=>` and
    /// drives the polarity to the atoms.
    fn nnf_inner(self, neg: bool) -> Formula {
        match self {
            Formula::Atom(a) => Formula::Atom(if neg {
                Atom {
                    op: a.op.negate(),
                    ..a
                }
            } else {
                a
            }),
            Formula::Not(inner) => inner.nnf_inner(!neg),
            Formula::And(l, r) => {
                let l = l.nnf_inner(neg);
                let r = r.nnf_inner(neg);
                if neg {
                    // ~(l & r) = ~l | ~r
                    Formula::Or(Box::new(l), Box::new(r))
                } else {
                    Formula::And(Box::new(l), Box::new(r))
                }
            }
            Formula::Or(l, r) => {
                let l = l.nnf_inner(neg);
                let r = r.nnf_inner(neg);
                if neg {
                    Formula::And(Box::new(l), Box::new(r))
                } else {
                    Formula::Or(Box::new(l), Box::new(r))
                }
            }
            Formula::Implies(l, r) => {
                // a => b  ≡  ~a | b. Under an outer negation: ~(a=>b) ≡ a & ~b.
                if neg {
                    let l = l.nnf_inner(false);
                    let r = r.nnf_inner(true);
                    Formula::And(Box::new(l), Box::new(r))
                } else {
                    let l = l.nnf_inner(true);
                    let r = r.nnf_inner(false);
                    Formula::Or(Box::new(l), Box::new(r))
                }
            }
            Formula::Quantified(q, name, body) => {
                let q = if neg {
                    match q {
                        Quant::Forall => Quant::Exists,
                        Quant::Exists => Quant::Forall,
                    }
                } else {
                    q
                };
                Formula::Quantified(q, name, Box::new(body.nnf_inner(neg)))
            }
        }
    }
}

// ===========================================================================
// Pass 2 — prenex (alpha-rename to fresh, then hoist quantifiers)
// ===========================================================================

/// Rename every binder in `f` to a globally-unique fresh name `__qN`, so a later
/// hoist can never capture a free occurrence. Free variables are untouched.
fn freshen(f: Formula, counter: &mut usize, env: &mut Vec<(String, String)>) -> Formula {
    match f {
        Formula::Atom(a) => Formula::Atom(Atom {
            op: a.op,
            lhs: subst_term(a.lhs, env),
            rhs: subst_term(a.rhs, env),
        }),
        Formula::Not(b) => Formula::Not(Box::new(freshen(*b, counter, env))),
        Formula::And(l, r) => {
            let l = freshen(*l, counter, env);
            let r = freshen(*r, counter, env);
            Formula::And(Box::new(l), Box::new(r))
        }
        Formula::Or(l, r) => {
            let l = freshen(*l, counter, env);
            let r = freshen(*r, counter, env);
            Formula::Or(Box::new(l), Box::new(r))
        }
        Formula::Implies(l, r) => {
            let l = freshen(*l, counter, env);
            let r = freshen(*r, counter, env);
            Formula::Implies(Box::new(l), Box::new(r))
        }
        Formula::Quantified(q, name, body) => {
            let fresh = format!("__q{counter}");
            *counter += 1;
            env.push((name, fresh.clone()));
            let body = freshen(*body, counter, env);
            env.pop();
            Formula::Quantified(q, fresh, Box::new(body))
        }
    }
}

/// Apply the binder renaming `env` (innermost binding wins) to a term's variable
/// occurrences. A name not in `env` is free and preserved verbatim.
fn subst_term(t: Term, env: &[(String, String)]) -> Term {
    match t {
        Term::Var(name) => {
            let bound = env.iter().rev().find(|(from, _)| *from == name);
            Term::Var(bound.map(|(_, to)| to.clone()).unwrap_or(name))
        }
        Term::Int(n) => Term::Int(n),
        Term::App(name, args) => {
            Term::App(name, args.into_iter().map(|a| subst_term(a, env)).collect())
        }
        Term::Arith(op, l, r) => Term::Arith(
            op,
            Box::new(subst_term(*l, env)),
            Box::new(subst_term(*r, env)),
        ),
        Term::Cast(inner, ty) => Term::Cast(Box::new(subst_term(*inner, env)), ty),
    }
}

/// Pull all quantifiers to the front (metatheory §8.2 layer-1 pass 2). Assumes
/// the input is in NNF (no `Not`/`Implies`) and has been [`freshen`]ed (all
/// binders globally unique, so hoisting across `&`/`|` cannot capture). Returns
/// the quantifier prefix (outermost first) and the quantifier-free matrix.
fn prenex(f: Formula) -> (Vec<(Quant, String)>, Formula) {
    match f {
        Formula::Atom(_) => (Vec::new(), f),
        Formula::Quantified(q, name, body) => {
            let (mut prefix, matrix) = prenex(*body);
            prefix.insert(0, (q, name));
            (prefix, matrix)
        }
        Formula::And(l, r) => {
            let (mut pl, ml) = prenex(*l);
            let (pr, mr) = prenex(*r);
            pl.extend(pr);
            (pl, Formula::And(Box::new(ml), Box::new(mr)))
        }
        Formula::Or(l, r) => {
            let (mut pl, ml) = prenex(*l);
            let (pr, mr) = prenex(*r);
            pl.extend(pr);
            (pl, Formula::Or(Box::new(ml), Box::new(mr)))
        }
        Formula::Not(_) | Formula::Implies(..) => {
            unreachable!("prenex requires NNF (no Not/Implies)")
        }
    }
}

// ===========================================================================
// Passes 3 & 4 — canonical bound-name form + atom ordering
// ===========================================================================

/// A canonicalized matrix node: `&`/`|` are flattened to a sorted n-ary list.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum CMatrix {
    Atom(CmpOp, CTerm, CTerm),
    And(Vec<CMatrix>),
    Or(Vec<CMatrix>),
}

/// A canonicalized term (a plain mirror of [`Term`] with commutative arithmetic
/// operands sorted). `Ord` drives the deterministic child ordering.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum CTerm {
    Var(String),
    Int(i128),
    App(String, Vec<CTerm>),
    Arith(ArithOp, Box<CTerm>, Box<CTerm>),
    Cast(Box<CTerm>, String),
}

fn canon_term(t: &Term, rename: &dyn Fn(&str) -> String) -> CTerm {
    match t {
        Term::Var(name) => CTerm::Var(rename(name)),
        Term::Int(n) => CTerm::Int(*n),
        Term::App(name, args) => CTerm::App(
            name.clone(),
            args.iter().map(|a| canon_term(a, rename)).collect(),
        ),
        Term::Arith(op, l, r) => {
            let mut cl = canon_term(l, rename);
            let mut cr = canon_term(r, rename);
            if op.is_commutative() && cr < cl {
                std::mem::swap(&mut cl, &mut cr);
            }
            CTerm::Arith(*op, Box::new(cl), Box::new(cr))
        }
        Term::Cast(inner, ty) => CTerm::Cast(Box::new(canon_term(inner, rename)), ty.clone()),
    }
}

/// Build the canonical matrix from a quantifier-free [`Formula`], applying
/// `rename` to every variable, orienting atoms, flattening + sorting `&`/`|`.
fn canon_matrix(f: &Formula, rename: &dyn Fn(&str) -> String) -> CMatrix {
    match f {
        Formula::Atom(a) => {
            let mut op = a.op;
            let mut l = canon_term(&a.lhs, rename);
            let mut r = canon_term(&a.rhs, rename);
            // Orient `>`/`>=` to their `<`/`<=` flips, and sort the operands of
            // the symmetric `=`/`!=`. `<`/`<=` keep their operand order.
            match op {
                CmpOp::Gt | CmpOp::Ge => {
                    op = op.flip();
                    std::mem::swap(&mut l, &mut r);
                }
                CmpOp::Eq | CmpOp::Ne => {
                    if r < l {
                        std::mem::swap(&mut l, &mut r);
                    }
                }
                CmpOp::Lt | CmpOp::Le => {}
            }
            CMatrix::Atom(op, l, r)
        }
        Formula::And(_, _) => {
            let mut kids = Vec::new();
            flatten_and(f, rename, &mut kids);
            kids.sort();
            kids.dedup();
            if kids.len() == 1 {
                kids.pop().unwrap()
            } else {
                CMatrix::And(kids)
            }
        }
        Formula::Or(_, _) => {
            let mut kids = Vec::new();
            flatten_or(f, rename, &mut kids);
            kids.sort();
            kids.dedup();
            if kids.len() == 1 {
                kids.pop().unwrap()
            } else {
                CMatrix::Or(kids)
            }
        }
        Formula::Not(_) | Formula::Implies(..) | Formula::Quantified(..) => {
            unreachable!("canon_matrix requires a quantifier-free NNF matrix")
        }
    }
}

fn flatten_and(f: &Formula, rename: &dyn Fn(&str) -> String, out: &mut Vec<CMatrix>) {
    match f {
        Formula::And(l, r) => {
            flatten_and(l, rename, out);
            flatten_and(r, rename, out);
        }
        other => out.push(canon_matrix(other, rename)),
    }
}

fn flatten_or(f: &Formula, rename: &dyn Fn(&str) -> String, out: &mut Vec<CMatrix>) {
    match f {
        Formula::Or(l, r) => {
            flatten_or(l, rename, out);
            flatten_or(r, rename, out);
        }
        other => out.push(canon_matrix(other, rename)),
    }
}

/// Produce the canonical serialization of a prenex `(prefix, matrix)` (passes 3 &
/// 4). The binders within each maximal same-quantifier block commute, so we try
/// every permutation of binders within each block, rename to positional
/// `v0,v1,…` names, serialize, and keep the lexicographically smallest result —
/// a true alpha- and binder-order-invariant canonical form.
fn canonical(prefix: &[(Quant, String)], matrix: &Formula) -> String {
    // Partition the prefix into maximal same-quantifier blocks (block order is
    // fixed — different quantifiers do not commute past each other in general).
    let mut blocks: Vec<(Quant, Vec<String>)> = Vec::new();
    for (q, name) in prefix {
        match blocks.last_mut() {
            Some((bq, names)) if bq == q => names.push(name.clone()),
            _ => blocks.push((*q, vec![name.clone()])),
        }
    }

    // The flat fresh-name order produced by a given choice of per-block
    // permutations, mapped to positional names v0,v1,…
    let block_perms: Vec<Vec<Vec<usize>>> = blocks
        .iter()
        .map(|(_, names)| permutations((0..names.len()).collect()))
        .collect();

    let mut best: Option<String> = None;
    for choice in cartesian(&block_perms) {
        // Build the fresh→canonical map for this permutation choice.
        let mut map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        let mut k = 0usize;
        for (bi, (_, names)) in blocks.iter().enumerate() {
            for &idx in &choice[bi] {
                map.insert(names[idx].clone(), format!("v{k}"));
                k += 1;
            }
        }
        let rename =
            |name: &str| -> String { map.get(name).cloned().unwrap_or_else(|| name.to_string()) };
        let cmatrix = canon_matrix(matrix, &rename);

        // Serialize the prefix (canonical names in v-order) + the matrix.
        let mut s = String::new();
        let mut k = 0usize;
        for (q, names) in &blocks {
            for _ in names {
                let qt = match q {
                    Quant::Forall => "A",
                    Quant::Exists => "E",
                };
                let _ = write!(s, "{qt} v{k}. ");
                k += 1;
            }
        }
        serialize_matrix(&cmatrix, &mut s);
        match &best {
            Some(b) if *b <= s => {}
            _ => best = Some(s),
        }
    }
    best.unwrap_or_default()
}

fn serialize_matrix(m: &CMatrix, out: &mut String) {
    match m {
        CMatrix::Atom(op, l, r) => {
            out.push('(');
            serialize_term(l, out);
            let _ = write!(out, " {} ", op.token());
            serialize_term(r, out);
            out.push(')');
        }
        CMatrix::And(kids) => {
            out.push_str("(&");
            for k in kids {
                out.push(' ');
                serialize_matrix(k, out);
            }
            out.push(')');
        }
        CMatrix::Or(kids) => {
            out.push_str("(|");
            for k in kids {
                out.push(' ');
                serialize_matrix(k, out);
            }
            out.push(')');
        }
    }
}

fn serialize_term(t: &CTerm, out: &mut String) {
    match t {
        CTerm::Var(name) => out.push_str(name),
        CTerm::Int(n) => {
            let _ = write!(out, "{n}");
        }
        CTerm::App(name, args) => {
            out.push_str(name);
            out.push('(');
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                serialize_term(a, out);
            }
            out.push(')');
        }
        CTerm::Arith(op, l, r) => {
            out.push('(');
            serialize_term(l, out);
            let _ = write!(out, " {} ", op.token());
            serialize_term(r, out);
            out.push(')');
        }
        CTerm::Cast(inner, ty) => {
            out.push('(');
            serialize_term(inner, out);
            let _ = write!(out, " as {ty})");
        }
    }
}

/// All permutations of a small `Vec<usize>` (the binder indices within one
/// same-quantifier block — at most 2 in every spike shape, so the factorial cost
/// is trivially bounded).
fn permutations(items: Vec<usize>) -> Vec<Vec<usize>> {
    if items.len() <= 1 {
        return vec![items];
    }
    let mut out = Vec::new();
    for i in 0..items.len() {
        let mut rest = items.clone();
        let head = rest.remove(i);
        for mut p in permutations(rest) {
            p.insert(0, head);
            out.push(p);
        }
    }
    out
}

/// The cartesian product of per-block permutation choices: one [`Vec<usize>`] per
/// block, all combinations.
fn cartesian(per_block: &[Vec<Vec<usize>>]) -> Vec<Vec<Vec<usize>>> {
    let mut acc: Vec<Vec<Vec<usize>>> = vec![Vec::new()];
    for choices in per_block {
        let mut next = Vec::new();
        for prefix in &acc {
            for choice in choices {
                let mut p = prefix.clone();
                p.push(choice.clone());
                next.push(p);
            }
        }
        acc = next;
    }
    acc
}

impl Formula {
    /// The full layer-1 normalization (all four passes), returning the canonical
    /// serialization. Two formulas are [`equivalent`] iff these strings match.
    pub fn normalize(self) -> String {
        let nnf = self.to_nnf();
        let mut counter = 0;
        let mut env = Vec::new();
        let freshened = freshen(nnf, &mut counter, &mut env);
        let (prefix, matrix) = prenex(freshened);
        canonical(&prefix, &matrix)
    }
}

/// Are two raw-quantifier formulas syntactically equal after layer-1
/// normalization? This is the per-pair predicate the spike's hit rate counts.
pub fn equivalent(a: &Formula, b: &Formula) -> bool {
    a.clone().normalize() == b.clone().normalize()
}

/// Convenience: parse both surface spellings and report whether they normalize
/// to the same canonical form. Returns the two canonical strings alongside the
/// verdict so the hit-rate target can print a divergence.
pub fn pair_hits(production: &str, reference: &str) -> Result<PairResult, ParseError> {
    let pf = parse(production)?;
    let rf = parse(reference)?;
    let pn = pf.normalize();
    let rn = rf.normalize();
    Ok(PairResult {
        hit: pn == rn,
        production_canonical: pn,
        reference_canonical: rn,
    })
}

/// The outcome of normalizing one production/reference fixture pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairResult {
    /// Whether the two normalized forms are byte-identical.
    pub hit: bool,
    /// The production spelling's canonical normal form.
    pub production_canonical: String,
    /// The reference spelling's canonical normal form.
    pub reference_canonical: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn norm(src: &str) -> String {
        parse(src).expect("parse").normalize()
    }

    // ---- parser ----

    #[test]
    fn parses_and_roundtrips_a_quantified_formula() {
        let f = parse("forall i . (0 <= i & i < len(xs)) => idx(xs, i) < needle").unwrap();
        // The chained-free body is a single forall with an Implies under it.
        match f {
            Formula::Quantified(Quant::Forall, _, _) => {}
            other => panic!("expected forall, got {other:?}"),
        }
    }

    #[test]
    fn chained_comparison_desugars_to_conjunction() {
        // `0 <= i <= j` ≡ `0 <= i & i <= j`.
        assert_eq!(norm("0 <= i <= j"), norm("0 <= i & i <= j"));
    }

    #[test]
    fn leading_parenthesized_term_is_not_misread_as_a_formula_group() {
        // `(5 - n) <= i` — the leading `(` wraps a TERM, not a formula group. The
        // backtracking `parse_unary` must read it as an atom whose lhs is `(5 - n)`.
        assert_eq!(
            norm("(5 - n) <= i < len(xs)"),
            norm("5 - n <= i & i < len(xs)")
        );
    }

    #[test]
    fn rejects_trailing_garbage() {
        assert!(parse("0 <= i )").is_err());
        assert!(parse("forall . i < 3").is_err());
    }

    // ---- Pass 1: NNF ----

    #[test]
    fn nnf_eliminates_implies() {
        // `a => b` ≡ `~a | b`.
        assert_eq!(norm("i < 3 => j < 4"), norm("~(i < 3) | j < 4"));
    }

    #[test]
    fn nnf_pushes_negation_to_atoms_via_de_morgan() {
        // `~(a & b)` ≡ `~a | ~b`, and `~(i < 3)` ≡ `i >= 3`.
        assert_eq!(norm("~(i < 3 & j < 4)"), norm("i >= 3 | j >= 4"));
    }

    #[test]
    fn nnf_negates_quantifier_duality() {
        // `~(forall i. i < 3)` ≡ `exists i. i >= 3`.
        assert_eq!(norm("~(forall i . i < 3)"), norm("exists i . i >= 3"));
    }

    #[test]
    fn nnf_double_negation() {
        assert_eq!(norm("~(~(i < 3))"), norm("i < 3"));
    }

    // ---- Pass 2: prenex ----

    #[test]
    fn prenex_hoists_quantifier_out_of_conjunction() {
        // `(forall i. p(i)) & q` ≡ `forall i. (p(i) & q)` (i not free in q).
        assert_eq!(
            norm("(forall i . i < len(xs)) & needle < 9"),
            norm("forall i . (i < len(xs) & needle < 9)")
        );
    }

    #[test]
    fn prenex_avoids_capture_via_alpha_rename() {
        // Two binders both named `i` in independent scopes must not be conflated:
        // `(forall i. i < a) & (forall i. i < b)` keeps two distinct binders.
        let n = norm("(forall i . i < a) & (forall i . i < b)");
        // Two `A`-quantifiers in the prefix.
        assert_eq!(n.matches("A v").count(), 2, "{n}");
    }

    // ---- Passes 3 & 4: canonical bound names + atom ordering ----

    #[test]
    fn alpha_equivalent_formulas_normalize_equal() {
        assert_eq!(
            norm("forall i . i < len(xs)"),
            norm("forall k . k < len(xs)")
        );
    }

    #[test]
    fn conjunct_order_is_canonicalized() {
        assert_eq!(norm("i < 3 & j < 4 & k < 5"), norm("k < 5 & i < 3 & j < 4"));
    }

    #[test]
    fn comparison_orientation_is_canonicalized() {
        // `a > b` ≡ `b < a`; `a >= b` ≡ `b <= a`.
        assert_eq!(norm("needle > idx(xs, i)"), norm("idx(xs, i) < needle"));
        assert_eq!(norm("needle >= idx(xs, i)"), norm("idx(xs, i) <= needle"));
    }

    #[test]
    fn symmetric_atom_operands_are_sorted() {
        assert_eq!(norm("idx(xs, i) = needle"), norm("needle = idx(xs, i)"));
        assert_eq!(norm("idx(xs, i) != needle"), norm("needle != idx(xs, i)"));
    }

    #[test]
    fn commutative_quantifier_block_order_is_canonicalized() {
        // `forall i j. i < j` ≡ `forall j i. i < j` — same-quantifier binders
        // commute, so binder order must not matter.
        assert_eq!(
            norm("forall i j . idx(xs, i) <= idx(xs, j)"),
            norm("forall j i . idx(xs, i) <= idx(xs, j)")
        );
    }

    #[test]
    fn commutative_arithmetic_operands_are_sorted() {
        assert_eq!(norm("i < a + b"), norm("i < b + a"));
        // Subtraction is not commutative.
        assert_ne!(norm("i < a - b"), norm("i < b - a"));
    }

    #[test]
    fn genuinely_different_formulas_do_not_collide() {
        assert_ne!(
            norm("forall i . i < len(xs)"),
            norm("exists i . i < len(xs)")
        );
        assert_ne!(norm("i < 3"), norm("i < 4"));
        assert_ne!(norm("i < len(xs)"), norm("i < len(ys)"));
    }
}
