//! The Rust admission classifier — the **ops half** of the stage-2 stratified cage
//! (`.design/stage2-stratified-cage.md` REQ-4 / AC-4). This is a simple
//! re-implementation, in Rust, of the Lean kernel classifier shipped in REQ-3 (#325):
//! `Thermite.Strat.Cls.admitted` (`lean/Thermite/Strat/Fragment.lean`),
//!
//! ```text
//!   admitted φ = finCarrier φ && idxGrammar φ && acyclic (sortGraph (nnf φ))
//! ```
//!
//! over the **sort-typed `Cls` surface syntax** (`Sort₂` + the array-property term
//! vocabulary `Read`/`Len`/`Cast`/`IdxOp`/`Mul`/spec-fn + sorted binders), not REQ-1's
//! minimal semantic-spine `Frm`. The two languages are distinct by design (the #68
//! axiom-probe collision; see `Strat/Nnf.lean`'s header): this module mirrors the
//! `Thermite.Strat.Cls.Frm` classifier surface so the **differential battery**
//! (`thermite-tv`'s generator → both this classifier and `lake env lean --run` on the
//! Lean `admitted`) can hold the two implementations to byte-equal verdicts on every
//! generated formula. Any disagreement is a hard CI failure (audit check [8]); the
//! `unknown`-on-admitted tripwire logs and escalates as classifier-suspect.
//!
//! ## The mirror is exact (so the differential is meaningful)
//!
//! Every `fn` below is a line-for-line transliteration of the Lean definition it names
//! in its doc comment — `fin_sort`/`fin_carrier`, `same_width`/`has_bound_var`/
//! `idx_ok_tm`/`idx_grammar_at`, `nnf`/`nnf_neg`, `edges_tm`/`edges_atom`/`edges_frm`/
//! `sort_graph`, and `admitted`. The one intentional divergence is the acyclicity
//! decision: the Lean kernel uses the exponential Roy–Warshall `reach` recursion
//! (`Strat/Graph.lean`, fine for `decide` on the §3.2 micro-examples), whereas the Rust
//! side computes the same boolean (`acyclic G ⟺ no node reaches itself`) by a
//! polynomial transitive-closure ([`Graph::acyclic`]); the two agree by
//! `acyclic_iff_no_cycle`, and the differential battery is what witnesses the agreement
//! empirically over the generated clause space.
//!
//! ## The frozen rejection vocabulary (REQ-4)
//!
//! A rejection names its reason from the frozen [`RejectReason`] vocabulary
//! (`infinite-carrier`/`seq-quantifier` for (R1), `index-grammar` for (R2), the named
//! `…-cycle` for (R3)). The classifier is total — `classify` always returns a definite
//! [`Verdict::Admitted`] or [`Verdict::Rejected`]; the [`Verdict::Unknown`] arm exists
//! only for the differential battery's tripwire (a formula the classifier could not
//! vouch for while Lean admitted it — escalate, never silently retry).
//!
//! ## REQ status
//!
//! Tracked centrally as **REQ-S2-4** in `.design/reqs/registry.toml` (the stage-2
//! tracking entry, alongside REQ-S2-1/2/3), rendered into `.design/reqs/status.md`;
//! governing design `.design/stage2-stratified-cage.md` REQ-4 / AC-4.

use std::fmt;

// ===========================================================================
// Sorts (mirrors `Strat/Nnf.lean` §1.1 — `Mach` / `Sort₂`)
// ===========================================================================

/// Machine sorts — finite by definition (`Strat/Nnf.lean` `inductive Mach`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mach {
    U8,
    U16,
    U32,
    U64,
    Usize,
    Bool,
}

impl Mach {
    /// Machine bit-width — the width-preserving-cast test (`Strat/Fragment.lean`
    /// `machWidth`). `usize` is 64 (the cage's index width); `bool` is 1.
    fn width(self) -> u32 {
        match self {
            Mach::U8 => 8,
            Mach::U16 => 16,
            Mach::U32 => 32,
            Mach::U64 => 64,
            Mach::Usize => 64,
            Mach::Bool => 1,
        }
    }

    /// The wire token (`Strat/Cls/Wire.lean` mirror).
    fn wire(self) -> &'static str {
        match self {
            Mach::U8 => "u8",
            Mach::U16 => "u16",
            Mach::U32 => "u32",
            Mach::U64 => "u64",
            Mach::Usize => "usize",
            Mach::Bool => "bool",
        }
    }

    fn from_wire(tok: &str) -> Option<Mach> {
        Some(match tok {
            "u8" => Mach::U8,
            "u16" => Mach::U16,
            "u32" => Mach::U32,
            "u64" => Mach::U64,
            "usize" => Mach::Usize,
            "bool" => Mach::Bool,
            _ => return None,
        })
    }
}

/// The stratified sort language (`Strat/Nnf.lean` `inductive Sort₂`): machine sorts,
/// sequences, and user-declared opaque nominal sorts (`Key`/`Value`, identified by a
/// `u32`). `seq` is never itself a quantifier carrier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Sort2 {
    Mach(Mach),
    Seq(Box<Sort2>),
    Opaque(u32),
}

impl Sort2 {
    /// The `usize` index sort — the workhorse of array-property formulas
    /// (`Strat/Nnf.lean` `usizeS`).
    #[must_use]
    pub fn usize_s() -> Sort2 {
        Sort2::Mach(Mach::Usize)
    }
}

impl fmt::Display for Sort2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Sort2::Mach(m) => write!(f, "{}", m.wire()),
            Sort2::Seq(s) => write!(f, "seq<{s}>"),
            Sort2::Opaque(k) => write!(f, "opaque{k}"),
        }
    }
}

// ===========================================================================
// Terms, relations, atoms, formulas (mirrors `Strat/Nnf.lean` §1.2)
// ===========================================================================

/// Terms carry their sort annotations explicitly (`Strat/Nnf.lean` `inductive Tm`); the
/// classifier reads sorts off the syntax rather than re-running a typechecker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScalarValue {
    Int(i128),
    Bool(bool),
}

/// Terms carry their sort annotations explicitly (`Strat/Nnf.lean` `inductive Tm`); the
/// classifier reads sorts off the syntax rather than re-running a typechecker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tm {
    /// `var s i` — a de Bruijn variable carrying its sort.
    Var(Sort2, u32),
    /// A free source constant. `id` is assigned deterministically by the canonical
    /// source bridge and distinguishes constants of the same sort.
    Const(Sort2, u32),
    /// A source literal with its value preserved for reconstruction.
    Lit(Sort2, ScalarValue),
    /// `read elem sq ix` — `sq[ix] : Read SeqS elem × usize → elem`.
    Read(Sort2, Box<Tm>, Box<Tm>),
    /// `len sq` — `sq.len() : SeqS _ → usize`.
    Len(Box<Tm>),
    /// `cast to t` — `(t as to)`.
    Cast(Sort2, Box<Tm>),
    /// `idxOp t k` — `t ± literal k` (the (R2)-admissible offset; `k` is inert to
    /// classification but carried for round-trip fidelity with the Lean `Int`).
    IdxOp(Box<Tm>, i64),
    /// `mul t u` — a non-linear op ((R2) forbids a quantified index var under it).
    Mul(Box<Tm>, Box<Tm>),
    /// `app1 arg res f a` — a declared unary spec fn (E2 edge `arg → res`).
    App1(Sort2, Sort2, u32, Box<Tm>),
}

impl Tm {
    /// The sort of a term, read straight off its annotations (`Strat/Nnf.lean`
    /// `Tm.sortOf`).
    fn sort_of(&self) -> Sort2 {
        match self {
            Tm::Var(s, _) => s.clone(),
            Tm::Const(s, _) | Tm::Lit(s, _) => s.clone(),
            Tm::Read(elem, _, _) => elem.clone(),
            Tm::Len(_) => Sort2::usize_s(),
            Tm::Cast(to, _) => to.clone(),
            Tm::IdxOp(t, _) => t.sort_of(),
            Tm::Mul(t, _) => t.sort_of(),
            Tm::App1(_, res, _, _) => res.clone(),
        }
    }
}

/// Relations on machine / opaque sorts (`Strat/Nnf.lean` `inductive Rel`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rel {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl Rel {
    fn wire(self) -> &'static str {
        match self {
            Rel::Eq => "eq",
            Rel::Ne => "ne",
            Rel::Lt => "lt",
            Rel::Le => "le",
            Rel::Gt => "gt",
            Rel::Ge => "ge",
        }
    }

    fn from_wire(tok: &str) -> Option<Rel> {
        Some(match tok {
            "eq" => Rel::Eq,
            "ne" => Rel::Ne,
            "lt" => Rel::Lt,
            "le" => Rel::Le,
            "gt" => Rel::Gt,
            "ge" => Rel::Ge,
            _ => return None,
        })
    }
}

/// Atoms (`Strat/Nnf.lean` `inductive Atom`): a relation between two terms, or a whole
/// v1 quantifier-free formula embedded — opaque to the classifier (it contributes no
/// sorts and no graph edges, exactly the metatheory §1.2 `QFree φ₀` leaf). The Lean
/// `qfree` carries a `Thermite.Expr`; the classifier never inspects it. The Rust
/// mirror carries the canonical bridge's stable leaf ID so reconstruction cannot
/// accidentally associate a normalized leaf with a different source expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Atom {
    Rel(Rel, Tm, Tm),
    QFree(u32),
}

/// The stratified formula language with sorted binders and `⇒` (eliminated by NNF)
/// (`Strat/Nnf.lean` `inductive Frm`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frm {
    Atom(Atom),
    Neg(Box<Frm>),
    Conj(Box<Frm>, Box<Frm>),
    Disj(Box<Frm>, Box<Frm>),
    Imp(Box<Frm>, Box<Frm>),
    All(Sort2, Box<Frm>),
    Ex(Sort2, Box<Frm>),
}

// ===========================================================================
// (R1) finite carriers — binder sorts only (mirrors `Strat/Fragment.lean`)
// ===========================================================================

/// Is a sort an admissible (finite) quantifier carrier? (`Strat/Fragment.lean`
/// `finSort`.) The only non-finite `Sort₂` is `seq` — `nat`/`int` are not `Sort₂`s, so
/// the check is exactly "no `seq` binder".
fn fin_sort(s: &Sort2) -> bool {
    match s {
        Sort2::Mach(_) => true,
        Sort2::Opaque(_) => true,
        Sort2::Seq(_) => false,
    }
}

/// Every binder ranges over a finite carrier (`Strat/Fragment.lean` `finCarrier`).
/// Returns the offending binder sort on the first violation (left-to-right), so the
/// classifier can name the (R1) rejection reason.
fn fin_carrier(phi: &Frm) -> Result<(), Sort2> {
    match phi {
        Frm::Atom(_) => Ok(()),
        Frm::Neg(p) => fin_carrier(p),
        Frm::Conj(p, q) | Frm::Disj(p, q) | Frm::Imp(p, q) => {
            fin_carrier(p)?;
            fin_carrier(q)
        }
        Frm::All(s, p) | Frm::Ex(s, p) => {
            if fin_sort(s) {
                fin_carrier(p)
            } else {
                Err(s.clone())
            }
        }
    }
}

// ===========================================================================
// (R2) the Bradley–Manna–Sipma index grammar (mirrors `Strat/Fragment.lean`)
// ===========================================================================

/// Do two sorts have the same machine width — a width-preserving cast?
/// (`Strat/Fragment.lean` `sameWidth`.)
fn same_width(a: &Sort2, b: &Sort2) -> bool {
    match (a, b) {
        (Sort2::Mach(m), Sort2::Mach(m2)) => m.width() == m2.width(),
        _ => false,
    }
}

/// Does a term mention a bound variable (de Bruijn index `< depth`)?
/// (`Strat/Fragment.lean` `hasBoundVar`.)
fn has_bound_var(depth: u32, t: &Tm) -> bool {
    match t {
        Tm::Var(_, i) => *i < depth,
        Tm::Const(_, _) | Tm::Lit(_, _) => false,
        Tm::Read(_, sq, ix) => has_bound_var(depth, sq) || has_bound_var(depth, ix),
        Tm::Len(sq) => has_bound_var(depth, sq),
        Tm::Cast(_, t) => has_bound_var(depth, t),
        Tm::IdxOp(t, _) => has_bound_var(depth, t),
        Tm::Mul(t, u) => has_bound_var(depth, t) || has_bound_var(depth, u),
        Tm::App1(_, _, _, t) => has_bound_var(depth, t),
    }
}

/// (R2), per term: no bound index var under `mul` or a width-changing `cast`
/// (`Strat/Fragment.lean` `idxOkTm`).
fn idx_ok_tm(depth: u32, t: &Tm) -> bool {
    match t {
        Tm::Var(_, _) | Tm::Const(_, _) | Tm::Lit(_, _) => true,
        Tm::Read(_, sq, ix) => idx_ok_tm(depth, sq) && idx_ok_tm(depth, ix),
        Tm::Len(sq) => idx_ok_tm(depth, sq),
        Tm::Cast(to, t) => {
            (!has_bound_var(depth, t) || same_width(&t.sort_of(), to)) && idx_ok_tm(depth, t)
        }
        Tm::IdxOp(t, _) => idx_ok_tm(depth, t),
        Tm::Mul(t, u) => {
            !has_bound_var(depth, t)
                && !has_bound_var(depth, u)
                && idx_ok_tm(depth, t)
                && idx_ok_tm(depth, u)
        }
        Tm::App1(_, _, _, t) => idx_ok_tm(depth, t),
    }
}

/// (R2), per formula, tracking binder depth (`Strat/Fragment.lean` `idxGrammarAt`).
/// `true` iff the whole formula satisfies the index grammar.
fn idx_grammar_at(depth: u32, phi: &Frm) -> bool {
    match phi {
        Frm::Atom(Atom::Rel(_, t, u)) => idx_ok_tm(depth, t) && idx_ok_tm(depth, u),
        Frm::Atom(Atom::QFree(_)) => true,
        Frm::Neg(p) => idx_grammar_at(depth, p),
        Frm::Conj(p, q) | Frm::Disj(p, q) | Frm::Imp(p, q) => {
            idx_grammar_at(depth, p) && idx_grammar_at(depth, q)
        }
        Frm::All(_, p) | Frm::Ex(_, p) => idx_grammar_at(depth + 1, p),
    }
}

/// (R2) at top level (`Strat/Fragment.lean` `idxGrammar`).
fn idx_grammar(phi: &Frm) -> bool {
    idx_grammar_at(0, phi)
}

// ===========================================================================
// Negation-normal form (mirrors `Strat/Nnf.lean` `nnf`/`nnfNeg`)
// ===========================================================================

/// NNF: push every negation inward to the atoms and eliminate `⇒`, so every `all`/`ex`
/// carries its true polarity syntactically — what the sort graph reads
/// (`Strat/Nnf.lean` `nnf`).
fn nnf(phi: &Frm) -> Frm {
    match phi {
        Frm::Atom(a) => Frm::Atom(a.clone()),
        Frm::Neg(p) => nnf_neg(p),
        Frm::Conj(p, q) => Frm::Conj(Box::new(nnf(p)), Box::new(nnf(q))),
        Frm::Disj(p, q) => Frm::Disj(Box::new(nnf(p)), Box::new(nnf(q))),
        Frm::Imp(p, q) => Frm::Disj(Box::new(nnf_neg(p)), Box::new(nnf(q))),
        Frm::All(s, p) => Frm::All(s.clone(), Box::new(nnf(p))),
        Frm::Ex(s, p) => Frm::Ex(s.clone(), Box::new(nnf(p))),
    }
}

/// `nnf_neg φ` computes the NNF of `¬φ` (`Strat/Nnf.lean` `nnfNeg`).
fn nnf_neg(phi: &Frm) -> Frm {
    match phi {
        Frm::Atom(a) => Frm::Neg(Box::new(Frm::Atom(a.clone()))),
        Frm::Neg(p) => nnf(p),
        Frm::Conj(p, q) => Frm::Disj(Box::new(nnf_neg(p)), Box::new(nnf_neg(q))),
        Frm::Disj(p, q) => Frm::Conj(Box::new(nnf_neg(p)), Box::new(nnf_neg(q))),
        Frm::Imp(p, q) => Frm::Conj(Box::new(nnf(p)), Box::new(nnf_neg(q))),
        Frm::All(s, p) => Frm::Ex(s.clone(), Box::new(nnf_neg(p))),
        Frm::Ex(s, p) => Frm::All(s.clone(), Box::new(nnf_neg(p))),
    }
}

// ===========================================================================
// The sort graph (E1 ∪ E2) (mirrors `Strat/Graph.lean`)
// ===========================================================================

/// A directed graph over sorts (`Strat/Graph.lean` `structure Graph`) — an explicit
/// node list and edge list (kept with duplicates, as the Lean `nodesOf` builds
/// them, so the node set matches; reachability is insensitive to the duplication).
struct Graph {
    nodes: Vec<Sort2>,
    edges: Vec<(Sort2, Sort2)>,
}

impl Graph {
    /// Acyclicity (`Strat/Graph.lean` `acyclic`): no node reaches itself. The Lean
    /// kernel decides this with the exponential Roy–Warshall `reach`; this is the
    /// polynomial transitive-closure equivalent — `acyclic ⟺ no node has a length-≥1
    /// walk back to itself`. Returns the offending (self-reaching) node on a cycle, so
    /// the classifier can name the (R3) rejection reason. The node returned is the
    /// first in `nodes` order that reaches itself (deterministic, R-CODE-5).
    fn acyclic(&self) -> Result<(), Sort2> {
        // Reachability one step at a time, from each node, following edges. A node `s`
        // is on a cycle iff a ≥1-step walk returns to `s`. Standard DFS over the edge
        // relation; the node set may carry duplicates, so we scan distinct nodes.
        for s in &self.nodes {
            if self.reaches_self(s) {
                return Err(s.clone());
            }
        }
        Ok(())
    }

    /// Is there a length-≥1 walk `s → … → s`? DFS from each direct successor of `s`,
    /// stopping if we reach `s`. Visited-set bounds it to polynomial time.
    fn reaches_self(&self, s: &Sort2) -> bool {
        let mut stack: Vec<&Sort2> = Vec::new();
        let mut visited: Vec<&Sort2> = Vec::new();
        // Seed with the direct successors of `s` (the ≥1-step requirement).
        for (a, b) in &self.edges {
            if a == s {
                stack.push(b);
            }
        }
        while let Some(node) = stack.pop() {
            if node == s {
                return true;
            }
            if visited.contains(&node) {
                continue;
            }
            visited.push(node);
            for (a, b) in &self.edges {
                if a == node {
                    stack.push(b);
                }
            }
        }
        false
    }
}

/// A binder context entry: `(is_universal, sort)`. The list head is the innermost
/// binder (de Bruijn level 0), matching the Lean `ctx : List (Bool × Sort₂)`.
type Ctx = Vec<(bool, Sort2)>;

/// Does a term mention a variable bound by the current prefix?
///
/// Existential occurrences count: after Skolemization they can carry an
/// earlier universal dependency through the surrounding function.
fn has_scoped_var(ctx: &Ctx, t: &Tm) -> bool {
    match t {
        Tm::Var(_, i) => (*i as usize) < ctx.len(),
        Tm::Const(_, _) | Tm::Lit(_, _) => false,
        Tm::Read(_, sq, ix) => has_scoped_var(ctx, sq) || has_scoped_var(ctx, ix),
        Tm::Len(sq) => has_scoped_var(ctx, sq),
        Tm::Cast(_, t) => has_scoped_var(ctx, t),
        Tm::IdxOp(t, _) => has_scoped_var(ctx, t),
        Tm::Mul(t, u) => has_scoped_var(ctx, t) || has_scoped_var(ctx, u),
        Tm::App1(_, _, _, t) => has_scoped_var(ctx, t),
    }
}

/// The E2 edges contributed by a term (`Strat/Graph.lean` `edgesTm`): a function
/// occurrence whose `S`-position argument contains a universally bound variable, plus
/// the edges of its subterms.
fn edges_tm(ctx: &Ctx, t: &Tm, out: &mut Vec<(Sort2, Sort2)>) {
    match t {
        Tm::Var(_, _) | Tm::Const(_, _) | Tm::Lit(_, _) => {}
        Tm::Read(elem, sq, ix) => {
            if has_scoped_var(ctx, ix) {
                out.push((Sort2::usize_s(), elem.clone()));
            }
            edges_tm(ctx, sq, out);
            edges_tm(ctx, ix, out);
        }
        Tm::Len(sq) => {
            if has_scoped_var(ctx, sq) {
                out.push((sq.sort_of(), Sort2::usize_s()));
            }
            edges_tm(ctx, sq, out);
        }
        Tm::Cast(to, t) => {
            if has_scoped_var(ctx, t) {
                out.push((t.sort_of(), to.clone()));
            }
            edges_tm(ctx, t, out);
        }
        Tm::IdxOp(t, _) => edges_tm(ctx, t, out),
        Tm::Mul(t, u) => {
            edges_tm(ctx, t, out);
            edges_tm(ctx, u, out);
        }
        Tm::App1(arg, res, _, t) => {
            if has_scoped_var(ctx, t) {
                out.push((arg.clone(), res.clone()));
            }
            edges_tm(ctx, t, out);
        }
    }
}

/// The E2 edges of an atom (`Strat/Graph.lean` `edgesAtom`); the `qfree` leaf is opaque.
fn edges_atom(ctx: &Ctx, a: &Atom, out: &mut Vec<(Sort2, Sort2)>) {
    match a {
        Atom::Rel(_, t, u) => {
            edges_tm(ctx, t, out);
            edges_tm(ctx, u, out);
        }
        Atom::QFree(_) => {}
    }
}

/// The universal sorts currently in scope (`Strat/Graph.lean` `univSorts`).
fn univ_sorts(ctx: &Ctx) -> Vec<Sort2> {
    ctx.iter()
        .filter(|(u, _)| *u)
        .map(|(_, s)| s.clone())
        .collect()
}

/// The full edge set (E1 ∪ E2) of a formula under a binder context (`Strat/Graph.lean`
/// `edgesFrm`). The `ex` case adds the E1 alternation edges `S → T` for every enclosing
/// universal sort `S`. The context cons (`(univ, s) :: ctx`) prepends at the head.
fn edges_frm(ctx: &Ctx, phi: &Frm, out: &mut Vec<(Sort2, Sort2)>) {
    match phi {
        Frm::Atom(a) => edges_atom(ctx, a, out),
        Frm::Neg(p) => edges_frm(ctx, p, out),
        Frm::Conj(p, q) | Frm::Disj(p, q) | Frm::Imp(p, q) => {
            edges_frm(ctx, p, out);
            edges_frm(ctx, q, out);
        }
        Frm::All(s, p) => {
            let mut c2 = Vec::with_capacity(ctx.len() + 1);
            c2.push((true, s.clone()));
            c2.extend_from_slice(ctx);
            edges_frm(&c2, p, out);
        }
        Frm::Ex(s, p) => {
            for big_s in univ_sorts(ctx) {
                out.push((big_s, s.clone()));
            }
            let mut c2 = Vec::with_capacity(ctx.len() + 1);
            c2.push((false, s.clone()));
            c2.extend_from_slice(ctx);
            edges_frm(&c2, p, out);
        }
    }
}

/// The sort graph of a formula (`Strat/Graph.lean` `sortGraph`). The node set is every
/// endpoint of every edge (so the graph is closed under its own endpoints — `Wf`).
fn sort_graph(phi: &Frm) -> Graph {
    let mut edges: Vec<(Sort2, Sort2)> = Vec::new();
    edges_frm(&Vec::new(), phi, &mut edges);
    let mut nodes: Vec<Sort2> = Vec::with_capacity(edges.len() * 2);
    for (a, b) in &edges {
        nodes.push(a.clone());
        nodes.push(b.clone());
    }
    Graph { nodes, edges }
}

// ===========================================================================
// The classifier verdict + the frozen rejection vocabulary
// ===========================================================================

/// Why the classifier rejected a formula — the frozen vocabulary (REQ-4 / AC-4). One
/// member per admission gate; every rejection names exactly one. The headline three the
/// design names (`infinite-carrier`/`seq-quantifier`, the named cycle) are (R1) and
/// (R3); `index-grammar` is the (R2) member (a bound index var under `mul` or a
/// width-changing cast — the one rejection class with no graph-cycle witness).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RejectReason {
    /// (R1) `finCarrier` — a binder ranges over a non-finite carrier. The only such
    /// `Sort₂` is `seq` (nat/int are not `Sort₂`), so this is reported as the frozen
    /// `seq-quantifier` reason, naming the offending carrier sort.
    SeqQuantifier { sort: Sort2 },
    /// (R2) `idxGrammar` — a quantified index variable appears under `mul` (non-linear)
    /// or a width-changing `cast`. The frozen `index-grammar` reason.
    IndexGrammar,
    /// (R3) acyclicity — the sort graph `sortGraph(nnf φ)` has a cycle. The frozen
    /// "named cycle" reason; `cycle` is the sort that reaches itself.
    SortGraphCycle { cycle: Sort2 },
}

impl fmt::Display for RejectReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RejectReason::SeqQuantifier { sort } => {
                write!(
                    f,
                    "seq-quantifier (binder over non-finite carrier `{sort}`)"
                )
            }
            RejectReason::IndexGrammar => write!(
                f,
                "index-grammar (a quantified index var under `mul` or a width-changing `cast`)"
            ),
            RejectReason::SortGraphCycle { cycle } => {
                write!(f, "`{cycle}`-cycle (the sort graph is not stratified)")
            }
        }
    }
}

impl RejectReason {
    /// The bare frozen-vocabulary tag (no detail) — the stable token a forge route /
    /// the differential battery keys on.
    #[must_use]
    pub fn tag(&self) -> &'static str {
        match self {
            RejectReason::SeqQuantifier { .. } => "seq-quantifier",
            RejectReason::IndexGrammar => "index-grammar",
            RejectReason::SortGraphCycle { .. } => "sort-graph-cycle",
        }
    }
}

/// The classifier's verdict on a formula (REQ-4). [`Verdict::Admitted`] /
/// [`Verdict::Rejected`] are the total decision; [`Verdict::Unknown`] is the
/// differential battery's tripwire arm — a formula the classifier could not vouch for.
/// `classify` itself never returns `Unknown` (the decision is total); the variant is the
/// type-level home of the "unknown-on-admitted" escalation the battery counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Admitted,
    Rejected(RejectReason),
    Unknown(String),
}

impl Verdict {
    /// `true` iff the verdict is [`Verdict::Admitted`] — the boolean the differential
    /// battery compares against the Lean `admitted`.
    #[must_use]
    pub fn is_admitted(&self) -> bool {
        matches!(self, Verdict::Admitted)
    }
}

/// Classify a formula — the Rust mirror of `Thermite.Strat.Cls.admitted` plus the
/// rejection reason (REQ-4). Checks the three gates in the same order the Lean
/// `admitted` conjunction evaluates — (R1) `finCarrier`, then (R2) `idxGrammar`, then
/// (R3) `acyclic (sortGraph (nnf φ))` — so the boolean `classify(φ).is_admitted()`
/// equals `admitted φ` exactly (the differential invariant), and a rejection names the
/// first failing gate.
#[must_use]
pub fn classify(phi: &Frm) -> Verdict {
    // (R1) finite carriers.
    if let Err(sort) = fin_carrier(phi) {
        return Verdict::Rejected(RejectReason::SeqQuantifier { sort });
    }
    // (R2) the index grammar.
    if !idx_grammar(phi) {
        return Verdict::Rejected(RejectReason::IndexGrammar);
    }
    // (R3) the sort graph (computed on NNF) is acyclic.
    let g = sort_graph(&nnf(phi));
    if let Err(cycle) = g.acyclic() {
        return Verdict::Rejected(RejectReason::SortGraphCycle { cycle });
    }
    Verdict::Admitted
}

/// The bare boolean — the exact mirror of the Lean `admitted φ` (REQ-4). Equal to
/// `classify(φ).is_admitted()`; this is the value the differential battery holds
/// byte-equal to `lake env lean --run` on the Lean `admitted`.
#[must_use]
pub fn admitted(phi: &Frm) -> bool {
    classify(phi).is_admitted()
}

// ===========================================================================
// The wire format (shared with `lean/Thermite/Strat/Cls/Wire.lean`)
// ===========================================================================

/// Serialize a formula to the compact S-expression wire format the differential battery
/// feeds to `lake env lean --run` (REQ-4). The grammar is parenthesized and uniform so
/// the matching Lean parser (`Strat/Cls/Wire.lean`) and this serializer round-trip every
/// `Frm`. See [`parse_frm`] for the inverse and the grammar.
#[must_use]
pub fn to_wire(phi: &Frm) -> String {
    let mut s = String::new();
    write_frm(phi, &mut s);
    s
}

fn write_sort(s: &Sort2, out: &mut String) {
    match s {
        Sort2::Mach(m) => {
            out.push_str("(m ");
            out.push_str(m.wire());
            out.push(')');
        }
        Sort2::Seq(inner) => {
            out.push_str("(s ");
            write_sort(inner, out);
            out.push(')');
        }
        Sort2::Opaque(k) => {
            out.push_str("(o ");
            out.push_str(&k.to_string());
            out.push(')');
        }
    }
}

fn write_tm(t: &Tm, out: &mut String) {
    match t {
        Tm::Var(s, i) => {
            out.push_str("(v ");
            write_sort(s, out);
            out.push(' ');
            out.push_str(&i.to_string());
            out.push(')');
        }
        Tm::Const(s, id) => {
            out.push_str("(c ");
            write_sort(s, out);
            out.push(' ');
            out.push_str(&id.to_string());
            out.push(')');
        }
        Tm::Lit(s, value) => {
            out.push_str("(l ");
            write_sort(s, out);
            match value {
                ScalarValue::Int(value) => {
                    out.push_str(" (i ");
                    out.push_str(&value.to_string());
                    out.push(')');
                }
                ScalarValue::Bool(value) => {
                    out.push_str(" (b ");
                    out.push_str(if *value { "1" } else { "0" });
                    out.push(')');
                }
            }
            out.push(')');
        }
        Tm::Read(elem, sq, ix) => {
            out.push_str("(rd ");
            write_sort(elem, out);
            out.push(' ');
            write_tm(sq, out);
            out.push(' ');
            write_tm(ix, out);
            out.push(')');
        }
        Tm::Len(sq) => {
            out.push_str("(ln ");
            write_tm(sq, out);
            out.push(')');
        }
        Tm::Cast(to, t) => {
            out.push_str("(ct ");
            write_sort(to, out);
            out.push(' ');
            write_tm(t, out);
            out.push(')');
        }
        Tm::IdxOp(t, k) => {
            out.push_str("(ix ");
            write_tm(t, out);
            out.push(' ');
            out.push_str(&k.to_string());
            out.push(')');
        }
        Tm::Mul(t, u) => {
            out.push_str("(ml ");
            write_tm(t, out);
            out.push(' ');
            write_tm(u, out);
            out.push(')');
        }
        Tm::App1(arg, res, f, a) => {
            out.push_str("(a1 ");
            write_sort(arg, out);
            out.push(' ');
            write_sort(res, out);
            out.push(' ');
            out.push_str(&f.to_string());
            out.push(' ');
            write_tm(a, out);
            out.push(')');
        }
    }
}

fn write_atom(a: &Atom, out: &mut String) {
    match a {
        Atom::Rel(r, t, u) => {
            out.push_str("(r ");
            out.push_str(r.wire());
            out.push(' ');
            write_tm(t, out);
            out.push(' ');
            write_tm(u, out);
            out.push(')');
        }
        Atom::QFree(id) => {
            out.push_str("(qf ");
            out.push_str(&id.to_string());
            out.push(')');
        }
    }
}

fn write_frm(phi: &Frm, out: &mut String) {
    match phi {
        Frm::Atom(a) => {
            out.push_str("(at ");
            write_atom(a, out);
            out.push(')');
        }
        Frm::Neg(p) => {
            out.push_str("(ng ");
            write_frm(p, out);
            out.push(')');
        }
        Frm::Conj(p, q) => {
            out.push_str("(cj ");
            write_frm(p, out);
            out.push(' ');
            write_frm(q, out);
            out.push(')');
        }
        Frm::Disj(p, q) => {
            out.push_str("(dj ");
            write_frm(p, out);
            out.push(' ');
            write_frm(q, out);
            out.push(')');
        }
        Frm::Imp(p, q) => {
            out.push_str("(im ");
            write_frm(p, out);
            out.push(' ');
            write_frm(q, out);
            out.push(')');
        }
        Frm::All(s, p) => {
            out.push_str("(al ");
            write_sort(s, out);
            out.push(' ');
            write_frm(p, out);
            out.push(')');
        }
        Frm::Ex(s, p) => {
            out.push_str("(ex ");
            write_sort(s, out);
            out.push(' ');
            write_frm(p, out);
            out.push(')');
        }
    }
}

/// Parse a formula from the wire format (the inverse of [`to_wire`]; REQ-4). Returns
/// `Err` with a human-readable position-free message on any malformed input (never
/// panics, R-CODE-2). Used by the differential battery's round-trip test and any
/// debugging consumer.
///
/// Grammar (tokens are `(`, `)`, and maximal non-paren non-space runs):
///
/// ```text
/// sort := (m WIDTH) | (s sort) | (o NAT)
/// tm   := (v sort INT) | (l sort) | (rd sort TM TM) | (ln TM)
///       | (ct sort TM) | (ix TM INT) | (ml TM TM) | (a1 sort sort NAT TM)
/// atom := (r REL TM TM) | (qf NAT)
/// frm  := (at ATOM) | (ng FRM) | (cj FRM FRM) | (dj FRM FRM)
///       | (im FRM FRM) | (al sort FRM) | (ex sort FRM)
/// ```
pub fn parse_frm(wire: &str) -> Result<Frm, String> {
    let toks = tokenize(wire);
    let mut p = Parser {
        toks: &toks,
        pos: 0,
    };
    let frm = p.frm()?;
    if p.pos != p.toks.len() {
        return Err(format!("trailing tokens after formula at {}", p.pos));
    }
    Ok(frm)
}

fn tokenize(wire: &str) -> Vec<String> {
    let mut toks = Vec::new();
    let mut cur = String::new();
    for ch in wire.chars() {
        match ch {
            '(' | ')' => {
                if !cur.is_empty() {
                    toks.push(std::mem::take(&mut cur));
                }
                toks.push(ch.to_string());
            }
            c if c.is_whitespace() => {
                if !cur.is_empty() {
                    toks.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        toks.push(cur);
    }
    toks
}

struct Parser<'a> {
    toks: &'a [String],
    pos: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&str> {
        self.toks.get(self.pos).map(String::as_str)
    }

    fn next(&mut self) -> Result<&str, String> {
        let t = self
            .toks
            .get(self.pos)
            .ok_or_else(|| "unexpected end of wire input".to_string())?;
        self.pos += 1;
        Ok(t.as_str())
    }

    fn expect(&mut self, want: &str) -> Result<(), String> {
        let got = self.next()?;
        if got == want {
            Ok(())
        } else {
            Err(format!("expected `{want}`, found `{got}`"))
        }
    }

    /// The head tag of a `( tag … )` node, leaving `pos` after the tag.
    fn open(&mut self) -> Result<String, String> {
        self.expect("(")?;
        Ok(self.next()?.to_string())
    }

    fn int<T: std::str::FromStr>(&mut self) -> Result<T, String> {
        let t = self.next()?;
        t.parse::<T>()
            .map_err(|_| format!("expected an integer, found `{t}`"))
    }

    fn sort(&mut self) -> Result<Sort2, String> {
        let tag = self.open()?;
        let s = match tag.as_str() {
            "m" => {
                let w = self.next()?;
                Sort2::Mach(Mach::from_wire(w).ok_or_else(|| format!("bad mach width `{w}`"))?)
            }
            "s" => Sort2::Seq(Box::new(self.sort()?)),
            "o" => Sort2::Opaque(self.int()?),
            other => return Err(format!("bad sort tag `{other}`")),
        };
        self.expect(")")?;
        Ok(s)
    }

    fn tm(&mut self) -> Result<Tm, String> {
        let tag = self.open()?;
        let t = match tag.as_str() {
            "v" => {
                let s = self.sort()?;
                Tm::Var(s, self.int()?)
            }
            "c" => {
                let sort = self.sort()?;
                Tm::Const(sort, self.int()?)
            }
            "l" => {
                let sort = self.sort()?;
                let value_tag = self.open()?;
                let value = match value_tag.as_str() {
                    "i" => ScalarValue::Int(self.int()?),
                    "b" => match self.int::<u8>()? {
                        0 => ScalarValue::Bool(false),
                        1 => ScalarValue::Bool(true),
                        other => return Err(format!("bad boolean literal `{other}`")),
                    },
                    other => return Err(format!("bad literal tag `{other}`")),
                };
                self.expect(")")?;
                Tm::Lit(sort, value)
            }
            "rd" => {
                let elem = self.sort()?;
                let sq = self.tm()?;
                let ix = self.tm()?;
                Tm::Read(elem, Box::new(sq), Box::new(ix))
            }
            "ln" => Tm::Len(Box::new(self.tm()?)),
            "ct" => {
                let to = self.sort()?;
                Tm::Cast(to, Box::new(self.tm()?))
            }
            "ix" => {
                let t = self.tm()?;
                Tm::IdxOp(Box::new(t), self.int()?)
            }
            "ml" => {
                let t = self.tm()?;
                let u = self.tm()?;
                Tm::Mul(Box::new(t), Box::new(u))
            }
            "a1" => {
                let arg = self.sort()?;
                let res = self.sort()?;
                let f = self.int()?;
                Tm::App1(arg, res, f, Box::new(self.tm()?))
            }
            other => return Err(format!("bad term tag `{other}`")),
        };
        self.expect(")")?;
        Ok(t)
    }

    fn atom(&mut self) -> Result<Atom, String> {
        let tag = self.open()?;
        let a = match tag.as_str() {
            "r" => {
                let r = Rel::from_wire(self.next()?).ok_or("bad rel")?;
                let t = self.tm()?;
                let u = self.tm()?;
                Atom::Rel(r, t, u)
            }
            "qf" => Atom::QFree(self.int()?),
            other => return Err(format!("bad atom tag `{other}`")),
        };
        self.expect(")")?;
        Ok(a)
    }

    fn frm(&mut self) -> Result<Frm, String> {
        let tag = self.open()?;
        let f = match tag.as_str() {
            "at" => Frm::Atom(self.atom()?),
            "ng" => Frm::Neg(Box::new(self.frm()?)),
            "cj" => {
                let p = self.frm()?;
                Frm::Conj(Box::new(p), Box::new(self.frm()?))
            }
            "dj" => {
                let p = self.frm()?;
                Frm::Disj(Box::new(p), Box::new(self.frm()?))
            }
            "im" => {
                let p = self.frm()?;
                Frm::Imp(Box::new(p), Box::new(self.frm()?))
            }
            "al" => {
                let s = self.sort()?;
                Frm::All(s, Box::new(self.frm()?))
            }
            "ex" => {
                let s = self.sort()?;
                Frm::Ex(s, Box::new(self.frm()?))
            }
            other => return Err(format!("bad formula tag `{other}`")),
        };
        // Some atoms (`(qf)`) consume no closing here because `atom()` already balanced;
        // every other arm leaves the formula's own `)` next.
        let _ = self.peek();
        self.expect(")")?;
        Ok(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usize_s() -> Sort2 {
        Sort2::usize_s()
    }

    // The four §3.2 worked micro-examples — the same concrete formulas the Lean
    // `Strat/Fragment.lean` `decide`-checks (`ex_selfLoop`/`ex_castCycle`/`ex_kvCycle`/
    // `ex_sortedness`), transliterated to the Rust `Frm`. The Rust verdict must match the
    // Lean expected admit/reject outcome — the kernel-anchored end of the differential.

    /// Example 1 — nested reads `a[a[i]]` (`a : SeqS usize`): the E2 self-loop
    /// `usize → usize`. Expected: REJECT (a `usize`-cycle).
    fn ex_self_loop() -> Frm {
        let a_seq = || Tm::Const(Sort2::Seq(Box::new(usize_s())), 0);
        let inner = Tm::Read(
            usize_s(),
            Box::new(a_seq()),
            Box::new(Tm::Var(usize_s(), 0)),
        );
        let outer = Tm::Read(usize_s(), Box::new(a_seq()), Box::new(inner));
        Frm::All(
            usize_s(),
            Box::new(Frm::Atom(Atom::Rel(Rel::Eq, outer.clone(), outer))),
        )
    }

    /// Example 2 — `b[(a[i] as usize)]` (`a, b : SeqS u64`): the cycle
    /// `usize → u64 → usize`; the cast is width-preserving so (R2) PASSES and the
    /// rejection is purely the graph cycle. Expected: REJECT.
    fn ex_cast_cycle() -> Frm {
        let u64s = Sort2::Mach(Mach::U64);
        let a_seq = Tm::Const(Sort2::Seq(Box::new(u64s.clone())), 0);
        let b_seq = Tm::Const(Sort2::Seq(Box::new(u64s.clone())), 1);
        let ai = Tm::Read(
            u64s.clone(),
            Box::new(a_seq),
            Box::new(Tm::Var(usize_s(), 0)),
        );
        let cast_ai = Tm::Cast(usize_s(), Box::new(ai));
        let outer = Tm::Read(u64s, Box::new(b_seq), Box::new(cast_ai));
        Frm::All(
            usize_s(),
            Box::new(Frm::Atom(Atom::Rel(Rel::Eq, outer.clone(), outer))),
        )
    }

    /// Example 3 — the kv alternation cycle `(∀k. ∃v. …) ∧ (∀v. ∃k. …)` (Key = opaque 0,
    /// Value = opaque 1): E1 gives `Key → Value` and `Value → Key`. Expected: REJECT.
    fn ex_kv_cycle() -> Frm {
        let key_s = Sort2::Opaque(0);
        let value_s = Sort2::Opaque(1);
        let body1 = Frm::Atom(Atom::Rel(
            Rel::Eq,
            Tm::Var(value_s.clone(), 0),
            Tm::Var(key_s.clone(), 1),
        ));
        let body2 = Frm::Atom(Atom::Rel(
            Rel::Eq,
            Tm::Var(key_s.clone(), 0),
            Tm::Var(value_s.clone(), 1),
        ));
        Frm::Conj(
            Box::new(Frm::All(
                key_s.clone(),
                Box::new(Frm::Ex(value_s.clone(), Box::new(body1))),
            )),
            Box::new(Frm::All(value_s, Box::new(Frm::Ex(key_s, Box::new(body2))))),
        )
    }

    /// Example 4 — sortedness `∀ i j : usize. i ≤ j ⇒ a[i] ≤ a[j]` (`a : SeqS u32`): E2
    /// `usize → u32` only, acyclic. Expected: ADMIT.
    fn ex_sortedness() -> Frm {
        let u32s = Sort2::Mach(Mach::U32);
        let a_seq = || Tm::Const(Sort2::Seq(Box::new(u32s.clone())), 0);
        let i = Tm::Var(usize_s(), 1);
        let j = Tm::Var(usize_s(), 0);
        let hyp = Frm::Atom(Atom::Rel(Rel::Le, i.clone(), j.clone()));
        let concl = Frm::Atom(Atom::Rel(
            Rel::Le,
            Tm::Read(u32s.clone(), Box::new(a_seq()), Box::new(i)),
            Tm::Read(u32s.clone(), Box::new(a_seq()), Box::new(j)),
        ));
        Frm::All(
            usize_s(),
            Box::new(Frm::All(
                usize_s(),
                Box::new(Frm::Imp(Box::new(hyp), Box::new(concl))),
            )),
        )
    }

    #[test]
    fn micro_examples_match_lean_outcomes() {
        // Self-loop → reject with a cycle reason.
        match classify(&ex_self_loop()) {
            Verdict::Rejected(RejectReason::SortGraphCycle { cycle }) => {
                assert_eq!(cycle, usize_s(), "the self-loop is `usize → usize`");
            }
            other => panic!("ex_selfLoop must reject with a cycle, got {other:?}"),
        }
        assert!(!admitted(&ex_self_loop()));

        // Cast cycle → reject (cycle), and (R2) passes on its own.
        assert!(
            idx_grammar(&ex_cast_cycle()),
            "the width-preserving cast passes (R2) (mirrors ex_castCycle_idxGrammar_ok)"
        );
        assert!(matches!(
            classify(&ex_cast_cycle()),
            Verdict::Rejected(RejectReason::SortGraphCycle { .. })
        ));

        // kv alternation cycle → reject (cycle).
        assert!(matches!(
            classify(&ex_kv_cycle()),
            Verdict::Rejected(RejectReason::SortGraphCycle { .. })
        ));

        // Sortedness → admit.
        assert_eq!(classify(&ex_sortedness()), Verdict::Admitted);
        assert!(admitted(&ex_sortedness()));
    }

    #[test]
    fn seq_binder_is_seq_quantifier() {
        // `∀ x : SeqS u32. qfree#0` — a binder over a sequence sort, the (R1) rejection.
        let phi = Frm::All(
            Sort2::Seq(Box::new(Sort2::Mach(Mach::U32))),
            Box::new(Frm::Atom(Atom::QFree(0))),
        );
        match classify(&phi) {
            Verdict::Rejected(RejectReason::SeqQuantifier { sort }) => {
                assert_eq!(sort, Sort2::Seq(Box::new(Sort2::Mach(Mach::U32))));
            }
            other => panic!("a seq binder must be seq-quantifier, got {other:?}"),
        }
    }

    #[test]
    fn existential_function_flow_closes_the_skolem_cycle() {
        let source = Sort2::Opaque(40);
        let target = Sort2::Opaque(41);
        let formula = Frm::All(
            source.clone(),
            Box::new(Frm::Ex(
                target.clone(),
                Box::new(Frm::Atom(Atom::Rel(
                    Rel::Eq,
                    Tm::App1(
                        target.clone(),
                        source.clone(),
                        0,
                        Box::new(Tm::Var(target, 0)),
                    ),
                    Tm::Var(source, 1),
                ))),
            )),
        );

        assert!(matches!(
            classify(&formula),
            Verdict::Rejected(RejectReason::SortGraphCycle { .. })
        ));
    }

    #[test]
    fn mul_over_bound_var_is_index_grammar() {
        // `∀ i : usize. a[(i * i)] = a[(i * i)]` — a bound index var under `mul`, the
        // (R2) rejection with no graph-cycle witness.
        let u32s = Sort2::Mach(Mach::U32);
        let a_seq = || Tm::Const(Sort2::Seq(Box::new(u32s.clone())), 0);
        let prod = Tm::Mul(
            Box::new(Tm::Var(usize_s(), 0)),
            Box::new(Tm::Var(usize_s(), 0)),
        );
        let rd = Tm::Read(u32s.clone(), Box::new(a_seq()), Box::new(prod));
        let phi = Frm::All(
            usize_s(),
            Box::new(Frm::Atom(Atom::Rel(Rel::Eq, rd.clone(), rd))),
        );
        assert_eq!(
            classify(&phi),
            Verdict::Rejected(RejectReason::IndexGrammar)
        );
    }

    #[test]
    fn wire_round_trips_every_micro_example() {
        for phi in [
            ex_self_loop(),
            ex_cast_cycle(),
            ex_kv_cycle(),
            ex_sortedness(),
        ] {
            let wire = to_wire(&phi);
            let back = parse_frm(&wire).unwrap_or_else(|e| panic!("parse `{wire}`: {e}"));
            assert_eq!(back, phi, "wire round-trip must be identity: {wire}");
            // And the verdict is stable across the round-trip.
            assert_eq!(admitted(&back), admitted(&phi));
        }
    }

    #[test]
    fn wire_qfree_and_all_term_forms_round_trip() {
        // A formula exercising every Tm/Atom/Frm constructor at least once.
        let u64s = Sort2::Mach(Mach::U64);
        let a_seq = Tm::Const(Sort2::Seq(Box::new(u64s.clone())), 0);
        let inner = Tm::IdxOp(Box::new(Tm::Var(usize_s(), 0)), -3);
        let app = Tm::App1(usize_s(), u64s.clone(), 7, Box::new(inner));
        let rd = Tm::Read(u64s.clone(), Box::new(a_seq), Box::new(app));
        let ln = Tm::Len(Box::new(Tm::Const(Sort2::Seq(Box::new(u64s.clone())), 0)));
        let phi = Frm::Ex(
            usize_s(),
            Box::new(Frm::Conj(
                Box::new(Frm::Atom(Atom::Rel(Rel::Ne, rd, ln))),
                Box::new(Frm::Neg(Box::new(Frm::Atom(Atom::QFree(11))))),
            )),
        );
        let wire = to_wire(&phi);
        assert_eq!(parse_frm(&wire).unwrap(), phi, "wire: {wire}");
    }

    #[test]
    fn parse_rejects_malformed_wire() {
        assert!(parse_frm("(at (r eq").is_err(), "truncated");
        assert!(parse_frm("(zz)").is_err(), "bad tag");
        assert!(parse_frm("(at (r eq (v (m usize) 0) (v (m usize) 0))) extra").is_err());
    }
}
