//! Canonical source bridge for the S₂.0 reconstruction fragment.
//!
//! The classifier language deliberately records only the formula shape needed
//! for admission. Source quantifiers also name the finite domain they range
//! over, and quantifier-free leaves carry real source expressions. `S2Recon`
//! keeps those pieces beside the classifier formula so classification, replay,
//! hashing, and diagnostics can consume one deterministic record.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use thermite_syntax::{
    BinOp, Block, BvWidth, Clause, Expr, FnItem, IndexArg, Item, MatchArm, Param, Pattern,
    PrimType, Program, Quant, SlicePat, Stmt, Type, UnaryOp,
};

use crate::classifier::{to_wire, Atom, Frm, Mach, Rel, ScalarValue, Sort2, Tm};

/// Bumped whenever the canonical representation or source translation changes.
pub const S2_RECON_VERSION: &str = "s2-recon-v2";

/// The stable source location attached to a reconstructed clause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceAddress {
    pub item: String,
    pub clause: String,
}

/// A free source value used by the classifier formula.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstantDecl {
    pub id: u32,
    pub name: String,
    pub sort: Sort2,
}

/// A declared unary function used by the classifier formula.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionDecl {
    pub id: u32,
    pub name: String,
    pub arg: Sort2,
    pub result: Sort2,
}

/// The source domain associated with one quantifier occurrence.
///
/// `binder` is a pre-order occurrence number. It is independent of de Bruijn
/// indices, which are local to the formula body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinderDomain {
    pub binder: u32,
    pub sort: Sort2,
    pub source_sort: String,
    pub expression: Expr,
    pub canonical: String,
}

/// The checked arithmetic semantics used for a quantifier-free leaf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QFreeFragment {
    Lia,
    Bv(BvWidth),
}

impl QFreeFragment {
    fn for_clause(clause: &Clause) -> Self {
        clause.bv.map_or(Self::Lia, |tag| Self::Bv(tag.width))
    }

    fn wire_name(self) -> &'static str {
        match self {
            Self::Lia => "lia",
            Self::Bv(width) => width.spelling(),
        }
    }
}

/// A real quantifier-free source leaf embedded in the classifier formula.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QFreeAtom {
    pub id: u32,
    pub fragment: QFreeFragment,
    pub expression: Expr,
    pub canonical: String,
}

/// The complete deterministic input to S₂.0 reconstruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct S2Recon {
    pub version: &'static str,
    pub address: SourceAddress,
    pub formula: Frm,
    pub constants: Vec<ConstantDecl>,
    pub functions: Vec<FunctionDecl>,
    pub domains: Vec<BinderDomain>,
    pub qfree_atoms: Vec<QFreeAtom>,
}

impl S2Recon {
    /// Stable, span-free serialization used by evidence hashes and drift pins.
    #[must_use]
    pub fn canonical_wire(&self) -> String {
        let mut out = String::new();
        out.push_str("(s2r ");
        push_name(&mut out, self.version);
        out.push_str(" (addr ");
        push_name(&mut out, &self.address.item);
        out.push(' ');
        push_name(&mut out, &self.address.clause);
        out.push(')');

        out.push_str(" (consts");
        for decl in &self.constants {
            out.push_str(" (");
            out.push_str(&decl.id.to_string());
            out.push(' ');
            push_name(&mut out, &decl.name);
            out.push(' ');
            push_sort(&mut out, &decl.sort);
            out.push(')');
        }
        out.push(')');

        out.push_str(" (fns");
        for decl in &self.functions {
            out.push_str(" (");
            out.push_str(&decl.id.to_string());
            out.push(' ');
            push_name(&mut out, &decl.name);
            out.push(' ');
            push_sort(&mut out, &decl.arg);
            out.push(' ');
            push_sort(&mut out, &decl.result);
            out.push(')');
        }
        out.push(')');

        out.push_str(" (domains");
        for domain in &self.domains {
            out.push_str(" (");
            out.push_str(&domain.binder.to_string());
            out.push(' ');
            push_sort(&mut out, &domain.sort);
            out.push(' ');
            push_name(&mut out, &domain.source_sort);
            out.push(' ');
            push_name(&mut out, &domain.canonical);
            out.push(')');
        }
        out.push(')');

        out.push_str(" (qfree");
        for atom in &self.qfree_atoms {
            out.push_str(" (");
            out.push_str(&atom.id.to_string());
            out.push(' ');
            out.push_str(atom.fragment.wire_name());
            out.push(' ');
            push_name(&mut out, &atom.canonical);
            out.push(')');
        }
        out.push(')');

        out.push_str(" (frm ");
        out.push_str(&to_wire(&self.formula));
        out.push_str("))");
        out
    }
}

/// A source construct that cannot be represented faithfully in `S2Recon`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeError {
    UnknownValue { name: String },
    UnknownFunction { name: String },
    UnsupportedType { context: String, ty: Type },
    UnsupportedTerm { context: &'static str },
    BoundVariableInQFree { context: &'static str },
    NonUnaryFunction { name: String, arity: usize },
    SortMismatch { left: Sort2, right: Sort2 },
    ExpectedSequence { actual: Sort2 },
    ExpectedBoolean { actual: Sort2 },
    IntegerOutOfRange { value: u128 },
    OffsetOutOfRange { value: u128 },
}

impl fmt::Display for BridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BridgeError::UnknownValue { name } => write!(f, "unknown source value `{name}`"),
            BridgeError::UnknownFunction { name } => {
                write!(f, "missing unary spec-function signature for `{name}`")
            }
            BridgeError::UnsupportedType { context, ty } => {
                write!(f, "unsupported S₂.0 type in {context}: {ty:?}")
            }
            BridgeError::UnsupportedTerm { context } => {
                write!(f, "unsupported S₂.0 term: {context}")
            }
            BridgeError::BoundVariableInQFree { context } => {
                write!(
                    f,
                    "{context} uses a bound variable but has no S₂.0 term encoding"
                )
            }
            BridgeError::NonUnaryFunction { name, arity } => {
                write!(
                    f,
                    "spec function `{name}` has arity {arity}; S₂.0 admits unary functions"
                )
            }
            BridgeError::SortMismatch { left, right } => {
                write!(f, "S₂.0 sort mismatch: `{left}` versus `{right}`")
            }
            BridgeError::ExpectedSequence { actual } => {
                write!(f, "indexing requires a sequence, found `{actual}`")
            }
            BridgeError::ExpectedBoolean { actual } => {
                write!(f, "formula position requires `bool`, found `{actual}`")
            }
            BridgeError::IntegerOutOfRange { value } => {
                write!(
                    f,
                    "integer literal `{value}` does not fit the reconstruction value type"
                )
            }
            BridgeError::OffsetOutOfRange { value } => {
                write!(
                    f,
                    "index offset `{value}` does not fit the reconstruction offset type"
                )
            }
        }
    }
}

impl std::error::Error for BridgeError {}

/// Translate one real source clause into the canonical S₂.0 representation.
pub fn from_clause(
    program: &Program,
    item: &FnItem,
    clause: &Clause,
    address: SourceAddress,
) -> Result<S2Recon, BridgeError> {
    let opaque = OpaqueSorts::collect(program, &clause.expr);
    let constants = collect_constants(item, &opaque)?;
    let functions = collect_functions(program, &opaque);

    let constant_by_name = constants
        .iter()
        .map(|decl| (decl.name.clone(), decl.clone()))
        .collect();
    let function_by_name = functions
        .iter()
        .map(|decl| (decl.name.clone(), decl.clone()))
        .collect();
    let mut bridge = Bridge {
        opaque,
        constants: constant_by_name,
        functions: function_by_name,
        binders: Vec::new(),
        domains: Vec::new(),
        qfree_atoms: Vec::new(),
    };
    let formula = bridge.formula(&clause.expr, QFreeFragment::for_clause(clause))?;

    Ok(S2Recon {
        version: S2_RECON_VERSION,
        address,
        formula,
        constants,
        functions,
        domains: bridge.domains,
        qfree_atoms: bridge.qfree_atoms,
    })
}

/// Translate the counterexample query `premise ∧ ¬conclusion`.
///
/// Refuting this formula is exactly a proof of `premise → conclusion`.
/// Production reconstruction uses this entry point for `req → ens#k`; keeping
/// the polarity conversion here prevents routing from classifying one formula
/// and replaying another.
pub fn from_obligation(
    program: &Program,
    item: &FnItem,
    premise: &Clause,
    conclusion: &Clause,
    address: SourceAddress,
) -> Result<S2Recon, BridgeError> {
    let opaque = OpaqueSorts::collect_for(program, &[&premise.expr, &conclusion.expr]);
    let constants = collect_constants(item, &opaque)?;
    let functions = collect_functions(program, &opaque);
    let constant_by_name = constants
        .iter()
        .map(|decl| (decl.name.clone(), decl.clone()))
        .collect();
    let function_by_name = functions
        .iter()
        .map(|decl| (decl.name.clone(), decl.clone()))
        .collect();
    let mut bridge = Bridge {
        opaque,
        constants: constant_by_name,
        functions: function_by_name,
        binders: Vec::new(),
        domains: Vec::new(),
        qfree_atoms: Vec::new(),
    };
    let premise = bridge.formula(&premise.expr, QFreeFragment::for_clause(premise))?;
    let conclusion = bridge.formula(&conclusion.expr, QFreeFragment::for_clause(conclusion))?;

    Ok(S2Recon {
        version: S2_RECON_VERSION,
        address,
        formula: Frm::Conj(Box::new(premise), Box::new(Frm::Neg(Box::new(conclusion)))),
        constants,
        functions,
        domains: bridge.domains,
        qfree_atoms: bridge.qfree_atoms,
    })
}

#[derive(Debug, Clone)]
struct Bound {
    name: String,
    sort: Sort2,
}

struct Bridge {
    opaque: OpaqueSorts,
    constants: BTreeMap<String, ConstantDecl>,
    functions: BTreeMap<String, FunctionDecl>,
    binders: Vec<Bound>,
    domains: Vec<BinderDomain>,
    qfree_atoms: Vec<QFreeAtom>,
}

impl Bridge {
    fn formula(&mut self, expr: &Expr, fragment: QFreeFragment) -> Result<Frm, BridgeError> {
        match expr {
            Expr::Binary {
                op: BinOp::And,
                lhs,
                rhs,
            } => Ok(Frm::Conj(
                Box::new(self.formula(lhs, fragment)?),
                Box::new(self.formula(rhs, fragment)?),
            )),
            Expr::Binary {
                op: BinOp::Or,
                lhs,
                rhs,
            } => Ok(Frm::Disj(
                Box::new(self.formula(lhs, fragment)?),
                Box::new(self.formula(rhs, fragment)?),
            )),
            Expr::Binary { op, lhs, rhs } if relation(*op).is_some() => {
                match self.relation_atom(relation(*op).expect("guarded"), lhs, rhs) {
                    Ok(atom) => Ok(Frm::Atom(atom)),
                    Err(err)
                        if qfree_fallback_allowed(&err)
                            && !uses_bound(expr, &self.binders)
                            && qfree_formula(expr, &self.constants, fragment) =>
                    {
                        self.qfree(expr, fragment)
                    }
                    Err(err) => Err(err),
                }
            }
            Expr::Unary {
                op: UnaryOp::Not,
                expr,
            } => Ok(Frm::Neg(Box::new(self.formula(expr, fragment)?))),
            Expr::Quantifier {
                quant,
                var,
                sort,
                domain,
                body,
            } => {
                let binder_sort = self.opaque.sort_name(sort);
                let domain_term = self.term(domain, None)?;
                if !matches!(term_sort(&domain_term), Sort2::Seq(_)) {
                    return Err(BridgeError::ExpectedSequence {
                        actual: term_sort(&domain_term),
                    });
                }
                let domain_id =
                    u32::try_from(self.domains.len()).expect("S₂.0 binder count exceeds u32");
                self.domains.push(BinderDomain {
                    binder: domain_id,
                    sort: binder_sort.clone(),
                    source_sort: sort.clone(),
                    expression: (**domain).clone(),
                    canonical: canonical_expr(domain),
                });
                self.binders.push(Bound {
                    name: var.clone(),
                    sort: binder_sort.clone(),
                });
                let lowered = self.formula(body, fragment);
                self.binders.pop();
                let lowered = lowered?;
                let variable = Tm::Var(binder_sort.clone(), 0);
                let zero = Tm::Lit(binder_sort.clone(), ScalarValue::Int(0));
                let lifted_domain = lift_tm(&domain_term, 0);
                let lower_bound = Frm::Atom(Atom::Rel(Rel::Le, zero, variable.clone()));
                let upper_bound = Frm::Atom(Atom::Rel(
                    Rel::Lt,
                    variable,
                    Tm::Len(Box::new(lifted_domain)),
                ));
                let guard = Frm::Conj(Box::new(lower_bound), Box::new(upper_bound));
                Ok(match quant {
                    Quant::Forall => Frm::All(
                        binder_sort,
                        Box::new(Frm::Imp(Box::new(guard), Box::new(lowered))),
                    ),
                    Quant::Exists => Frm::Ex(
                        binder_sort,
                        Box::new(Frm::Conj(Box::new(guard), Box::new(lowered))),
                    ),
                })
            }
            Expr::BoolLit(_) if !uses_bound(expr, &self.binders) => self.qfree(expr, fragment),
            _ => match self.term(expr, Some(&Sort2::Mach(Mach::Bool))) {
                Ok(term) => {
                    let actual = term_sort(&term);
                    if actual != Sort2::Mach(Mach::Bool) {
                        return Err(BridgeError::ExpectedBoolean { actual });
                    }
                    Ok(Frm::Atom(Atom::Rel(
                        Rel::Eq,
                        term,
                        Tm::Lit(Sort2::Mach(Mach::Bool), ScalarValue::Bool(true)),
                    )))
                }
                Err(err)
                    if qfree_fallback_allowed(&err)
                        && !uses_bound(expr, &self.binders)
                        && qfree_formula(expr, &self.constants, fragment) =>
                {
                    self.qfree(expr, fragment)
                }
                Err(err) => Err(err),
            },
        }
    }

    fn relation_atom(&self, rel: Rel, lhs: &Expr, rhs: &Expr) -> Result<Atom, BridgeError> {
        let left_hint = self.infer_sort(lhs)?;
        let right_hint = self.infer_sort(rhs)?;
        let chosen = reconcile_hints(left_hint.as_ref(), right_hint.as_ref())?
            .unwrap_or_else(Sort2::usize_s);
        let left = self.term(lhs, Some(&chosen))?;
        let right = self.term(rhs, Some(&chosen))?;
        let left_sort = term_sort(&left);
        let right_sort = term_sort(&right);
        if left_sort != right_sort {
            return Err(BridgeError::SortMismatch {
                left: left_sort,
                right: right_sort,
            });
        }
        Ok(Atom::Rel(rel, left, right))
    }

    fn qfree(&mut self, expr: &Expr, fragment: QFreeFragment) -> Result<Frm, BridgeError> {
        if uses_bound(expr, &self.binders) {
            return Err(BridgeError::BoundVariableInQFree {
                context: "quantifier-free leaf",
            });
        }
        let canonical = canonical_expr(expr);
        if let Some(existing) = self
            .qfree_atoms
            .iter()
            .find(|atom| atom.fragment == fragment && atom.canonical == canonical)
        {
            return Ok(Frm::Atom(Atom::QFree(existing.id)));
        }
        let id = u32::try_from(self.qfree_atoms.len()).expect("S₂.0 qfree atom count exceeds u32");
        self.qfree_atoms.push(QFreeAtom {
            id,
            fragment,
            expression: expr.clone(),
            canonical,
        });
        Ok(Frm::Atom(Atom::QFree(id)))
    }

    fn term(&self, expr: &Expr, expected: Option<&Sort2>) -> Result<Tm, BridgeError> {
        match expr {
            Expr::IntLit { value, .. } => {
                let value = i128::try_from(*value)
                    .map_err(|_| BridgeError::IntegerOutOfRange { value: *value })?;
                Ok(Tm::Lit(
                    expected.cloned().unwrap_or_else(Sort2::usize_s),
                    ScalarValue::Int(value),
                ))
            }
            Expr::BoolLit(value) => Ok(Tm::Lit(Sort2::Mach(Mach::Bool), ScalarValue::Bool(*value))),
            Expr::Path(path) if path.len() == 1 => {
                let name = &path[0];
                if let Some((index, bound)) = self
                    .binders
                    .iter()
                    .rev()
                    .enumerate()
                    .find(|(_, bound)| &bound.name == name)
                {
                    return Ok(Tm::Var(
                        bound.sort.clone(),
                        u32::try_from(index).expect("S₂.0 binder depth exceeds u32"),
                    ));
                }
                self.constants
                    .get(name)
                    .map(|decl| Tm::Const(decl.sort.clone(), decl.id))
                    .ok_or_else(|| BridgeError::UnknownValue { name: name.clone() })
            }
            Expr::Index { base, index } => {
                let IndexArg::Single(index) = index else {
                    return Err(BridgeError::UnsupportedTerm {
                        context: "slice ranges are not scalar reads",
                    });
                };
                let base = self.term(base, None)?;
                let Sort2::Seq(elem) = term_sort(&base) else {
                    return Err(BridgeError::ExpectedSequence {
                        actual: term_sort(&base),
                    });
                };
                let index = self.term(index, None)?;
                Ok(Tm::Read(*elem, Box::new(base), Box::new(index)))
            }
            Expr::MethodCall {
                receiver,
                name,
                args,
            } if name == "len" && args.is_empty() => {
                Ok(Tm::Len(Box::new(self.term(receiver, None)?)))
            }
            Expr::Cast { expr, ty } => {
                let to = self.opaque.type_sort(ty, "cast target")?;
                Ok(Tm::Cast(to, Box::new(self.term(expr, None)?)))
            }
            Expr::Binary {
                op: BinOp::Add,
                lhs,
                rhs,
            } => {
                if let Some(k) = literal_offset(rhs)? {
                    return Ok(Tm::IdxOp(Box::new(self.term(lhs, expected)?), k));
                }
                if let Some(k) = literal_offset(lhs)? {
                    return Ok(Tm::IdxOp(Box::new(self.term(rhs, expected)?), k));
                }
                Err(BridgeError::UnsupportedTerm {
                    context: "S₂.0 addition requires one literal offset",
                })
            }
            Expr::Binary {
                op: BinOp::Sub,
                lhs,
                rhs,
            } => {
                let Some(k) = literal_offset(rhs)? else {
                    return Err(BridgeError::UnsupportedTerm {
                        context: "S₂.0 subtraction requires a literal offset",
                    });
                };
                Ok(Tm::IdxOp(Box::new(self.term(lhs, expected)?), -k))
            }
            Expr::Binary {
                op: BinOp::Mul,
                lhs,
                rhs,
            } => Ok(Tm::Mul(
                Box::new(self.term(lhs, expected)?),
                Box::new(self.term(rhs, expected)?),
            )),
            Expr::Call { callee, args } => {
                let Expr::Path(path) = callee.as_ref() else {
                    return Err(BridgeError::UnsupportedTerm {
                        context: "function callee is not a path",
                    });
                };
                let Some(name) = path.last() else {
                    return Err(BridgeError::UnsupportedTerm {
                        context: "empty function path",
                    });
                };
                let Some(decl) = self.functions.get(name) else {
                    return Err(BridgeError::UnknownFunction { name: name.clone() });
                };
                if args.len() != 1 {
                    return Err(BridgeError::NonUnaryFunction {
                        name: name.clone(),
                        arity: args.len(),
                    });
                }
                let arg = self.term(&args[0], Some(&decl.arg))?;
                let actual = term_sort(&arg);
                if actual != decl.arg {
                    return Err(BridgeError::SortMismatch {
                        left: actual,
                        right: decl.arg.clone(),
                    });
                }
                Ok(Tm::App1(
                    decl.arg.clone(),
                    decl.result.clone(),
                    decl.id,
                    Box::new(arg),
                ))
            }
            _ => Err(BridgeError::UnsupportedTerm {
                context: expression_kind(expr),
            }),
        }
    }

    fn infer_sort(&self, expr: &Expr) -> Result<Option<Sort2>, BridgeError> {
        match expr {
            Expr::IntLit { .. } => Ok(None),
            Expr::BoolLit(_) => Ok(Some(Sort2::Mach(Mach::Bool))),
            Expr::Path(path) if path.len() == 1 => {
                let name = &path[0];
                if let Some(bound) = self.binders.iter().rev().find(|bound| &bound.name == name) {
                    return Ok(Some(bound.sort.clone()));
                }
                self.constants
                    .get(name)
                    .map(|decl| Some(decl.sort.clone()))
                    .ok_or_else(|| BridgeError::UnknownValue { name: name.clone() })
            }
            Expr::Index { base, .. } => {
                let Some(sort) = self.infer_sort(base)? else {
                    return Err(BridgeError::UnsupportedTerm {
                        context: "sequence base has no inferred sort",
                    });
                };
                let Sort2::Seq(elem) = sort else {
                    return Err(BridgeError::ExpectedSequence { actual: sort });
                };
                Ok(Some(*elem))
            }
            Expr::MethodCall { name, args, .. } if name == "len" && args.is_empty() => {
                Ok(Some(Sort2::usize_s()))
            }
            Expr::Cast { ty, .. } => Ok(Some(self.opaque.type_sort(ty, "cast target")?)),
            Expr::Binary {
                op: BinOp::Add | BinOp::Sub | BinOp::Mul,
                lhs,
                rhs,
            } => {
                let left = self.infer_sort(lhs)?;
                let right = self.infer_sort(rhs)?;
                reconcile_hints(left.as_ref(), right.as_ref())
            }
            Expr::Call { callee, .. } => {
                let Expr::Path(path) = callee.as_ref() else {
                    return Ok(None);
                };
                Ok(path
                    .last()
                    .and_then(|name| self.functions.get(name))
                    .map(|decl| decl.result.clone()))
            }
            _ => Ok(None),
        }
    }
}

fn reconcile_hints(
    left: Option<&Sort2>,
    right: Option<&Sort2>,
) -> Result<Option<Sort2>, BridgeError> {
    match (left, right) {
        (Some(left), Some(right)) if left != right => Err(BridgeError::SortMismatch {
            left: left.clone(),
            right: right.clone(),
        }),
        (Some(sort), _) | (_, Some(sort)) => Ok(Some(sort.clone())),
        (None, None) => Ok(None),
    }
}

fn relation(op: BinOp) -> Option<Rel> {
    Some(match op {
        BinOp::Eq => Rel::Eq,
        BinOp::Ne => Rel::Ne,
        BinOp::Lt => Rel::Lt,
        BinOp::Le => Rel::Le,
        BinOp::Gt => Rel::Gt,
        BinOp::Ge => Rel::Ge,
        _ => return None,
    })
}

/// The scalar source grammar shared by the QF_LIA and QF_BV replay paths.
///
/// Qfree fallback is deliberately syntax-directed. A failed EPR term
/// translation is not permission to hide a call, field, collection operation,
/// or other unsupported source node behind an opaque atom.
fn qfree_formula(
    expr: &Expr,
    constants: &BTreeMap<String, ConstantDecl>,
    fragment: QFreeFragment,
) -> bool {
    match expr {
        Expr::BoolLit(_) => true,
        Expr::Binary { op, lhs, rhs } if relation(*op).is_some() => {
            qfree_term(lhs, constants, fragment) && qfree_term(rhs, constants, fragment)
        }
        Expr::Binary {
            op: BinOp::And | BinOp::Or,
            lhs,
            rhs,
        } => qfree_formula(lhs, constants, fragment) && qfree_formula(rhs, constants, fragment),
        Expr::Unary {
            op: UnaryOp::Not,
            expr,
        } => qfree_formula(expr, constants, fragment),
        _ => false,
    }
}

fn qfree_fallback_allowed(error: &BridgeError) -> bool {
    matches!(error, BridgeError::UnsupportedTerm { .. })
}

fn qfree_term(
    expr: &Expr,
    constants: &BTreeMap<String, ConstantDecl>,
    fragment: QFreeFragment,
) -> bool {
    match expr {
        Expr::IntLit { .. } => true,
        Expr::Path(path) => {
            path.len() == 1
                && constants.get(&path[0]).is_some_and(|constant| {
                    matches!(
                        constant.sort,
                        Sort2::Mach(Mach::U8 | Mach::U16 | Mach::U32 | Mach::U64 | Mach::Usize)
                    )
                })
        }
        Expr::Binary {
            op: BinOp::Add | BinOp::Sub,
            lhs,
            rhs,
        } => qfree_term(lhs, constants, fragment) && qfree_term(rhs, constants, fragment),
        Expr::Binary {
            op: BinOp::Mul,
            lhs,
            rhs,
        } => {
            (matches!(fragment, QFreeFragment::Bv(_))
                || matches!(lhs.as_ref(), Expr::IntLit { .. })
                || matches!(rhs.as_ref(), Expr::IntLit { .. }))
                && qfree_term(lhs, constants, fragment)
                && qfree_term(rhs, constants, fragment)
        }
        Expr::Binary {
            op:
                BinOp::Div
                | BinOp::Rem
                | BinOp::Shl
                | BinOp::Shr
                | BinOp::BitAnd
                | BinOp::BitOr
                | BinOp::BitXor,
            lhs,
            rhs,
        } if matches!(fragment, QFreeFragment::Bv(_)) => {
            qfree_term(lhs, constants, fragment) && qfree_term(rhs, constants, fragment)
        }
        Expr::Unary {
            op: UnaryOp::Not,
            expr,
        } if matches!(fragment, QFreeFragment::Bv(_)) => qfree_term(expr, constants, fragment),
        Expr::Cast { expr, .. } => qfree_term(expr, constants, fragment),
        _ => false,
    }
}

fn literal_offset(expr: &Expr) -> Result<Option<i64>, BridgeError> {
    let Expr::IntLit { value, .. } = expr else {
        return Ok(None);
    };
    i64::try_from(*value)
        .map(Some)
        .map_err(|_| BridgeError::OffsetOutOfRange { value: *value })
}

fn term_sort(term: &Tm) -> Sort2 {
    match term {
        Tm::Var(sort, _) | Tm::Const(sort, _) | Tm::Lit(sort, _) => sort.clone(),
        Tm::Read(elem, _, _) => elem.clone(),
        Tm::Len(_) => Sort2::usize_s(),
        Tm::Cast(to, _) => to.clone(),
        Tm::IdxOp(term, _) | Tm::Mul(term, _) => term_sort(term),
        Tm::App1(_, result, _, _) => result.clone(),
    }
}

fn lift_tm(term: &Tm, cutoff: u32) -> Tm {
    match term {
        Tm::Var(sort, index) => Tm::Var(
            sort.clone(),
            if *index < cutoff { *index } else { index + 1 },
        ),
        Tm::Const(sort, id) => Tm::Const(sort.clone(), *id),
        Tm::Lit(sort, value) => Tm::Lit(sort.clone(), value.clone()),
        Tm::Read(elem, base, index) => Tm::Read(
            elem.clone(),
            Box::new(lift_tm(base, cutoff)),
            Box::new(lift_tm(index, cutoff)),
        ),
        Tm::Len(base) => Tm::Len(Box::new(lift_tm(base, cutoff))),
        Tm::Cast(target, inner) => Tm::Cast(target.clone(), Box::new(lift_tm(inner, cutoff))),
        Tm::IdxOp(inner, offset) => Tm::IdxOp(Box::new(lift_tm(inner, cutoff)), *offset),
        Tm::Mul(left, right) => Tm::Mul(
            Box::new(lift_tm(left, cutoff)),
            Box::new(lift_tm(right, cutoff)),
        ),
        Tm::App1(arg, result, function, inner) => Tm::App1(
            arg.clone(),
            result.clone(),
            *function,
            Box::new(lift_tm(inner, cutoff)),
        ),
    }
}

fn collect_constants(
    item: &FnItem,
    opaque: &OpaqueSorts,
) -> Result<Vec<ConstantDecl>, BridgeError> {
    let mut values: BTreeMap<String, Type> = item
        .params
        .iter()
        .map(|Param { name, ty }| (name.clone(), ty.clone()))
        .collect();
    values.insert("result".to_string(), item.ret.clone());
    values
        .into_iter()
        .enumerate()
        .map(|(id, (name, ty))| {
            Ok(ConstantDecl {
                id: u32::try_from(id).expect("S₂.0 constant count exceeds u32"),
                sort: opaque.type_sort(&ty, &format!("constant `{name}`"))?,
                name,
            })
        })
        .collect()
}

fn collect_functions(program: &Program, opaque: &OpaqueSorts) -> Vec<FunctionDecl> {
    let mut signatures = BTreeMap::new();
    for item in &program.items {
        let Item::SpecFn(spec) = item else {
            continue;
        };
        if spec.params.len() != 1 {
            continue;
        }
        let Ok(arg) = opaque.type_sort(&spec.params[0].ty, "spec-function argument") else {
            continue;
        };
        let Ok(result) = opaque.type_sort(&spec.ret, "spec-function result") else {
            continue;
        };
        signatures.insert(spec.name.clone(), (arg, result));
    }
    signatures
        .into_iter()
        .enumerate()
        .map(|(id, (name, (arg, result)))| FunctionDecl {
            id: u32::try_from(id).expect("S₂.0 function count exceeds u32"),
            name,
            arg,
            result,
        })
        .collect()
}

#[derive(Debug, Clone)]
struct OpaqueSorts {
    ids: BTreeMap<String, u32>,
}

impl OpaqueSorts {
    fn collect(program: &Program, clause: &Expr) -> Self {
        Self::collect_for(program, &[clause])
    }

    fn collect_for(program: &Program, clauses: &[&Expr]) -> Self {
        let mut names = BTreeSet::new();
        for item in &program.items {
            match item {
                Item::Fn(function) => {
                    for param in &function.params {
                        collect_type_names(&param.ty, &mut names);
                    }
                    collect_type_names(&function.ret, &mut names);
                }
                Item::SpecFn(function) => {
                    for param in &function.params {
                        collect_type_names(&param.ty, &mut names);
                    }
                    collect_type_names(&function.ret, &mut names);
                }
                Item::Struct(item) => {
                    names.insert(item.name.clone());
                    for field in &item.fields {
                        collect_type_names(&field.ty, &mut names);
                    }
                }
                Item::Enum(item) => {
                    names.insert(item.name.clone());
                }
                Item::Forge(_) => {}
            }
        }
        for clause in clauses {
            collect_quantifier_sorts(clause, &mut names);
        }
        let ids = names
            .into_iter()
            .filter(|name| mach_name(name).is_none())
            .enumerate()
            .map(|(id, name)| {
                (
                    name,
                    u32::try_from(id).expect("S₂.0 opaque-sort count exceeds u32"),
                )
            })
            .collect();
        Self { ids }
    }

    fn sort_name(&self, name: &str) -> Sort2 {
        mach_name(name)
            .map(Sort2::Mach)
            .unwrap_or_else(|| Sort2::Opaque(self.ids[name]))
    }

    fn type_sort(&self, ty: &Type, context: &str) -> Result<Sort2, BridgeError> {
        match ty {
            Type::Prim(PrimType::U8) => Ok(Sort2::Mach(Mach::U8)),
            Type::Prim(PrimType::U16) => Ok(Sort2::Mach(Mach::U16)),
            Type::Prim(PrimType::U32) => Ok(Sort2::Mach(Mach::U32)),
            Type::Prim(PrimType::U64) => Ok(Sort2::Mach(Mach::U64)),
            Type::Prim(PrimType::Usize) => Ok(Sort2::Mach(Mach::Usize)),
            Type::Prim(PrimType::Bool) => Ok(Sort2::Mach(Mach::Bool)),
            Type::Ref { inner, .. } => self.type_sort(inner, context),
            Type::Slice(inner) | Type::Vec(inner) => {
                Ok(Sort2::Seq(Box::new(self.type_sort(inner, context)?)))
            }
            Type::String => Ok(Sort2::Seq(Box::new(Sort2::Mach(Mach::U8)))),
            Type::Named(name) => Ok(self.sort_name(name)),
            _ => Err(BridgeError::UnsupportedType {
                context: context.to_string(),
                ty: ty.clone(),
            }),
        }
    }
}

fn mach_name(name: &str) -> Option<Mach> {
    Some(match name {
        "u8" => Mach::U8,
        "u16" => Mach::U16,
        "u32" => Mach::U32,
        "u64" => Mach::U64,
        "usize" => Mach::Usize,
        "bool" => Mach::Bool,
        _ => return None,
    })
}

fn collect_type_names(ty: &Type, out: &mut BTreeSet<String>) {
    match ty {
        Type::Named(name) => {
            out.insert(name.clone());
        }
        Type::Ref { inner, .. }
        | Type::Slice(inner)
        | Type::Generic { arg: inner, .. }
        | Type::Box(inner)
        | Type::Vec(inner)
        | Type::Option(inner) => collect_type_names(inner, out),
        Type::Result(ok, err) | Type::Map(ok, err) => {
            collect_type_names(ok, out);
            collect_type_names(err, out);
        }
        Type::Tuple(types) => {
            for ty in types {
                collect_type_names(ty, out);
            }
        }
        Type::Prim(_) | Type::Unit | Type::String => {}
    }
}

fn collect_quantifier_sorts(expr: &Expr, out: &mut BTreeSet<String>) {
    walk_expr(expr, &mut |expr| {
        if let Expr::Quantifier { sort, .. } = expr {
            out.insert(sort.clone());
        }
    });
}

fn uses_bound(expr: &Expr, binders: &[Bound]) -> bool {
    let names: BTreeSet<&str> = binders.iter().map(|bound| bound.name.as_str()).collect();
    let mut found = false;
    walk_expr(expr, &mut |expr| {
        if let Expr::Path(path) = expr {
            found |= path.len() == 1 && names.contains(path[0].as_str());
        }
    });
    found
}

fn walk_expr(expr: &Expr, f: &mut impl FnMut(&Expr)) {
    f(expr);
    match expr {
        Expr::Call { callee, args } => {
            walk_expr(callee, f);
            for arg in args {
                walk_expr(arg, f);
            }
        }
        Expr::MethodCall { receiver, args, .. } => {
            walk_expr(receiver, f);
            for arg in args {
                walk_expr(arg, f);
            }
        }
        Expr::Field { receiver, .. }
        | Expr::Unary { expr: receiver, .. }
        | Expr::Cast { expr: receiver, .. }
        | Expr::Ref { expr: receiver, .. }
        | Expr::Deref(receiver)
        | Expr::TupleProj { receiver, .. } => walk_expr(receiver, f),
        Expr::Closure { body, .. } => walk_expr(body, f),
        Expr::Match { scrutinee, arms } => {
            walk_expr(scrutinee, f);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    walk_expr(guard, f);
                }
                walk_expr(&arm.body, f);
            }
        }
        Expr::If { cond, then, else_ } => {
            walk_expr(cond, f);
            walk_block(then, f);
            walk_block(else_, f);
        }
        Expr::Binary { lhs, rhs, .. } => {
            walk_expr(lhs, f);
            walk_expr(rhs, f);
        }
        Expr::Index { base, index } => {
            walk_expr(base, f);
            match index {
                IndexArg::Single(expr) | IndexArg::RangeTo(expr) | IndexArg::RangeFrom(expr) => {
                    walk_expr(expr, f)
                }
                IndexArg::Range(lo, hi) => {
                    walk_expr(lo, f);
                    walk_expr(hi, f);
                }
            }
        }
        Expr::StructLit { fields, .. } => {
            for (_, value) in fields {
                walk_expr(value, f);
            }
        }
        Expr::Is { scrutinee, .. } => walk_expr(scrutinee, f),
        Expr::Tuple(values) => {
            for value in values {
                walk_expr(value, f);
            }
        }
        Expr::Quantifier { domain, body, .. } => {
            walk_expr(domain, f);
            walk_expr(body, f);
        }
        Expr::IntLit { .. } | Expr::BoolLit(_) | Expr::Path(_) | Expr::StrLit(_) => {}
    }
}

fn walk_block(block: &Block, f: &mut impl FnMut(&Expr)) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Let { init, .. } => walk_expr(init, f),
            Stmt::Assign { target, value } => {
                walk_expr(target, f);
                walk_expr(value, f);
            }
            Stmt::Return(Some(expr)) | Stmt::Expr(expr) => walk_expr(expr, f),
            Stmt::Return(None) | Stmt::Break | Stmt::Continue => {}
            Stmt::If { cond, then, else_ } => {
                walk_expr(cond, f);
                walk_block(then, f);
                if let Some(else_) = else_ {
                    walk_block(else_, f);
                }
            }
            Stmt::Loop(loop_) => {
                if let thermite_syntax::LoopKind::While(cond) = &loop_.kind {
                    walk_expr(cond, f);
                }
                for inv in &loop_.invs {
                    walk_expr(&inv.expr, f);
                }
                walk_expr(&loop_.dec.expr, f);
                walk_block(&loop_.body, f);
            }
        }
    }
    if let Some(tail) = &block.tail {
        walk_expr(tail, f);
    }
}

fn push_name(out: &mut String, value: &str) {
    out.push_str(&value.len().to_string());
    out.push(':');
    out.push_str(value);
}

fn push_sort(out: &mut String, sort: &Sort2) {
    match sort {
        Sort2::Mach(mach) => {
            out.push_str("(m ");
            out.push_str(match mach {
                Mach::U8 => "u8",
                Mach::U16 => "u16",
                Mach::U32 => "u32",
                Mach::U64 => "u64",
                Mach::Usize => "usize",
                Mach::Bool => "bool",
            });
            out.push(')');
        }
        Sort2::Seq(inner) => {
            out.push_str("(s ");
            push_sort(out, inner);
            out.push(')');
        }
        Sort2::Opaque(id) => {
            out.push_str("(o ");
            out.push_str(&id.to_string());
            out.push(')');
        }
    }
}

fn canonical_expr(expr: &Expr) -> String {
    let mut out = String::new();
    write_expr(expr, &mut out);
    out
}

/// Stable, span-free source-expression serialization shared by reconstruction
/// evidence and the canonical S₂.0 bridge.
#[must_use]
pub fn canonical_source_expr(expr: &Expr) -> String {
    canonical_expr(expr)
}

fn write_expr(expr: &Expr, out: &mut String) {
    match expr {
        Expr::IntLit { value, raw } => {
            out.push_str("(int ");
            out.push_str(&value.to_string());
            out.push(' ');
            push_name(out, raw);
            out.push(')');
        }
        Expr::BoolLit(value) => out.push_str(if *value { "(bool 1)" } else { "(bool 0)" }),
        Expr::Path(path) => {
            out.push_str("(path");
            for part in path {
                out.push(' ');
                push_name(out, part);
            }
            out.push(')');
        }
        Expr::Call { callee, args } => {
            out.push_str("(call ");
            write_expr(callee, out);
            for arg in args {
                out.push(' ');
                write_expr(arg, out);
            }
            out.push(')');
        }
        Expr::MethodCall {
            receiver,
            name,
            args,
        } => {
            out.push_str("(method ");
            push_name(out, name);
            out.push(' ');
            write_expr(receiver, out);
            for arg in args {
                out.push(' ');
                write_expr(arg, out);
            }
            out.push(')');
        }
        Expr::Field { receiver, name } => {
            out.push_str("(field ");
            push_name(out, name);
            out.push(' ');
            write_expr(receiver, out);
            out.push(')');
        }
        Expr::Closure { params, body } => {
            out.push_str("(closure (");
            for param in params {
                push_name(out, param);
                out.push(' ');
            }
            out.push_str(") ");
            write_expr(body, out);
            out.push(')');
        }
        Expr::Match { scrutinee, arms } => {
            out.push_str("(match ");
            write_expr(scrutinee, out);
            for arm in arms {
                out.push(' ');
                write_arm(arm, out);
            }
            out.push(')');
        }
        Expr::If { cond, then, else_ } => {
            out.push_str("(if ");
            write_expr(cond, out);
            out.push(' ');
            write_block(then, out);
            out.push(' ');
            write_block(else_, out);
            out.push(')');
        }
        Expr::Binary { op, lhs, rhs } => {
            out.push_str("(bin ");
            out.push_str(binop_name(*op));
            out.push(' ');
            write_expr(lhs, out);
            out.push(' ');
            write_expr(rhs, out);
            out.push(')');
        }
        Expr::Unary { op, expr } => {
            out.push_str("(un ");
            out.push_str(match op {
                UnaryOp::Not => "not",
            });
            out.push(' ');
            write_expr(expr, out);
            out.push(')');
        }
        Expr::Index { base, index } => {
            out.push_str("(index ");
            write_expr(base, out);
            out.push(' ');
            write_index(index, out);
            out.push(')');
        }
        Expr::Cast { expr, ty } => {
            out.push_str("(cast ");
            write_type(ty, out);
            out.push(' ');
            write_expr(expr, out);
            out.push(')');
        }
        Expr::Ref { mutable, expr } => {
            out.push_str(if *mutable { "(ref mut " } else { "(ref imm " });
            write_expr(expr, out);
            out.push(')');
        }
        Expr::StructLit { path, fields } => {
            out.push_str("(struct-lit (");
            for part in path {
                push_name(out, part);
                out.push(' ');
            }
            out.push(')');
            for (name, value) in fields {
                out.push_str(" (");
                push_name(out, name);
                out.push(' ');
                write_expr(value, out);
                out.push(')');
            }
            out.push(')');
        }
        Expr::Is { scrutinee, variant } => {
            out.push_str("(is ");
            write_expr(scrutinee, out);
            out.push_str(" (");
            for part in variant {
                push_name(out, part);
                out.push(' ');
            }
            out.push_str("))");
        }
        Expr::Deref(expr) => {
            out.push_str("(deref ");
            write_expr(expr, out);
            out.push(')');
        }
        Expr::StrLit(value) => {
            out.push_str("(str ");
            push_name(out, value);
            out.push(')');
        }
        Expr::Tuple(values) => {
            out.push_str("(tuple");
            for value in values {
                out.push(' ');
                write_expr(value, out);
            }
            out.push(')');
        }
        Expr::TupleProj { receiver, index } => {
            out.push_str("(tuple-proj ");
            out.push_str(&index.to_string());
            out.push(' ');
            write_expr(receiver, out);
            out.push(')');
        }
        Expr::Quantifier {
            quant,
            var,
            sort,
            domain,
            body,
        } => {
            out.push_str(match quant {
                Quant::Forall => "(forall ",
                Quant::Exists => "(exists ",
            });
            push_name(out, var);
            out.push(' ');
            push_name(out, sort);
            out.push(' ');
            write_expr(domain, out);
            out.push(' ');
            write_expr(body, out);
            out.push(')');
        }
    }
}

fn write_index(index: &IndexArg, out: &mut String) {
    match index {
        IndexArg::Single(expr) => {
            out.push_str("(one ");
            write_expr(expr, out);
            out.push(')');
        }
        IndexArg::RangeTo(expr) => {
            out.push_str("(to ");
            write_expr(expr, out);
            out.push(')');
        }
        IndexArg::RangeFrom(expr) => {
            out.push_str("(from ");
            write_expr(expr, out);
            out.push(')');
        }
        IndexArg::Range(lo, hi) => {
            out.push_str("(range ");
            write_expr(lo, out);
            out.push(' ');
            write_expr(hi, out);
            out.push(')');
        }
    }
}

fn write_arm(arm: &MatchArm, out: &mut String) {
    out.push_str("(arm ");
    write_pattern(&arm.pattern, out);
    out.push(' ');
    match &arm.guard {
        Some(guard) => write_expr(guard, out),
        None => out.push_str("(none)"),
    }
    out.push(' ');
    write_expr(&arm.body, out);
    out.push(')');
}

fn write_pattern(pattern: &Pattern, out: &mut String) {
    match pattern {
        Pattern::Wildcard => out.push_str("(wild)"),
        Pattern::Literal(expr) => {
            out.push_str("(literal ");
            write_expr(expr, out);
            out.push(')');
        }
        Pattern::Binding(name) => {
            out.push_str("(bind ");
            push_name(out, name);
            out.push(')');
        }
        Pattern::Slice(parts) => {
            out.push_str("(slice");
            for part in parts {
                out.push(' ');
                match part {
                    SlicePat::Pat(pattern) => write_pattern(pattern, out),
                    SlicePat::Rest(name) => {
                        out.push_str("(rest ");
                        push_name(out, name);
                        out.push(')');
                    }
                }
            }
            out.push(')');
        }
        Pattern::Enum { path, fields } => {
            out.push_str("(enum (");
            for part in path {
                push_name(out, part);
                out.push(' ');
            }
            out.push(')');
            for field in fields {
                out.push(' ');
                write_pattern(field, out);
            }
            out.push(')');
        }
        Pattern::Struct { path, fields, rest } => {
            out.push_str("(struct (");
            for part in path {
                push_name(out, part);
                out.push(' ');
            }
            out.push(')');
            for (name, pattern) in fields {
                out.push_str(" (");
                push_name(out, name);
                out.push(' ');
                write_pattern(pattern, out);
                out.push(')');
            }
            out.push_str(if *rest { " rest)" } else { " closed)" });
        }
        Pattern::Or(patterns) => {
            out.push_str("(or");
            for pattern in patterns {
                out.push(' ');
                write_pattern(pattern, out);
            }
            out.push(')');
        }
    }
}

fn write_block(block: &Block, out: &mut String) {
    out.push_str("(block");
    for stmt in &block.stmts {
        out.push(' ');
        write_stmt(stmt, out);
    }
    out.push(' ');
    match &block.tail {
        Some(tail) => write_expr(tail, out),
        None => out.push_str("(none)"),
    }
    out.push(')');
}

fn write_stmt(stmt: &Stmt, out: &mut String) {
    match stmt {
        Stmt::Let {
            mutable,
            name,
            ty,
            init,
        } => {
            out.push_str(if *mutable { "(let mut " } else { "(let imm " });
            push_name(out, name);
            out.push(' ');
            match ty {
                Some(ty) => write_type(ty, out),
                None => out.push_str("(infer)"),
            }
            out.push(' ');
            write_expr(init, out);
            out.push(')');
        }
        Stmt::Assign { target, value } => {
            out.push_str("(assign ");
            write_expr(target, out);
            out.push(' ');
            write_expr(value, out);
            out.push(')');
        }
        Stmt::Return(value) => {
            out.push_str("(return ");
            match value {
                Some(value) => write_expr(value, out),
                None => out.push_str("(none)"),
            }
            out.push(')');
        }
        Stmt::If { cond, then, else_ } => {
            out.push_str("(stmt-if ");
            write_expr(cond, out);
            out.push(' ');
            write_block(then, out);
            out.push(' ');
            match else_ {
                Some(else_) => write_block(else_, out),
                None => out.push_str("(none)"),
            }
            out.push(')');
        }
        Stmt::Loop(loop_) => {
            out.push_str("(loop ");
            match &loop_.kind {
                thermite_syntax::LoopKind::Loop => out.push_str("(forever)"),
                thermite_syntax::LoopKind::While(cond) => {
                    out.push_str("(while ");
                    write_expr(cond, out);
                    out.push(')');
                }
            }
            for inv in &loop_.invs {
                out.push(' ');
                write_expr(&inv.expr, out);
            }
            out.push(' ');
            write_expr(&loop_.dec.expr, out);
            out.push(' ');
            write_block(&loop_.body, out);
            out.push(')');
        }
        Stmt::Break => out.push_str("(break)"),
        Stmt::Continue => out.push_str("(continue)"),
        Stmt::Expr(expr) => {
            out.push_str("(expr ");
            write_expr(expr, out);
            out.push(')');
        }
    }
}

fn write_type(ty: &Type, out: &mut String) {
    match ty {
        Type::Prim(prim) => out.push_str(match prim {
            PrimType::U8 => "(u8)",
            PrimType::U16 => "(u16)",
            PrimType::U32 => "(u32)",
            PrimType::U64 => "(u64)",
            PrimType::Usize => "(usize)",
            PrimType::Bool => "(bool)",
        }),
        Type::Unit => out.push_str("(unit)"),
        Type::Ref { mutable, inner } => {
            out.push_str(if *mutable { "(ref mut " } else { "(ref imm " });
            write_type(inner, out);
            out.push(')');
        }
        Type::Slice(inner) => write_unary_type("slice", inner, out),
        Type::Generic { name, arg } => {
            out.push_str("(generic ");
            push_name(out, name);
            out.push(' ');
            write_type(arg, out);
            out.push(')');
        }
        Type::Named(name) => {
            out.push_str("(named ");
            push_name(out, name);
            out.push(')');
        }
        Type::Box(inner) => write_unary_type("box", inner, out),
        Type::Vec(inner) => write_unary_type("vec", inner, out),
        Type::String => out.push_str("(string)"),
        Type::Option(inner) => write_unary_type("option", inner, out),
        Type::Result(ok, err) => write_binary_type("result", ok, err, out),
        Type::Map(key, value) => write_binary_type("map", key, value, out),
        Type::Tuple(types) => {
            out.push_str("(tuple");
            for ty in types {
                out.push(' ');
                write_type(ty, out);
            }
            out.push(')');
        }
    }
}

fn write_unary_type(tag: &str, inner: &Type, out: &mut String) {
    out.push('(');
    out.push_str(tag);
    out.push(' ');
    write_type(inner, out);
    out.push(')');
}

fn write_binary_type(tag: &str, left: &Type, right: &Type, out: &mut String) {
    out.push('(');
    out.push_str(tag);
    out.push(' ');
    write_type(left, out);
    out.push(' ');
    write_type(right, out);
    out.push(')');
}

fn binop_name(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "add",
        BinOp::Sub => "sub",
        BinOp::Mul => "mul",
        BinOp::Div => "div",
        BinOp::Rem => "rem",
        BinOp::Shl => "shl",
        BinOp::Shr => "shr",
        BinOp::BitAnd => "bit-and",
        BinOp::BitOr => "bit-or",
        BinOp::BitXor => "bit-xor",
        BinOp::Eq => "eq",
        BinOp::Ne => "ne",
        BinOp::Lt => "lt",
        BinOp::Le => "le",
        BinOp::Gt => "gt",
        BinOp::Ge => "ge",
        BinOp::And => "and",
        BinOp::Or => "or",
    }
}

fn expression_kind(expr: &Expr) -> &'static str {
    match expr {
        Expr::IntLit { .. } => "integer literal",
        Expr::BoolLit(_) => "boolean literal",
        Expr::Path(_) => "multi-segment path",
        Expr::Call { .. } => "call",
        Expr::MethodCall { .. } => "method call",
        Expr::Field { .. } => "field access",
        Expr::Closure { .. } => "closure",
        Expr::Match { .. } => "match",
        Expr::If { .. } => "if expression",
        Expr::Binary { .. } => "binary operator",
        Expr::Unary { .. } => "unary operator",
        Expr::Index { .. } => "index expression",
        Expr::Cast { .. } => "cast",
        Expr::Ref { .. } => "reference",
        Expr::StructLit { .. } => "struct literal",
        Expr::Is { .. } => "variant test",
        Expr::Deref(_) => "dereference",
        Expr::StrLit(_) => "string literal",
        Expr::Tuple(_) => "tuple",
        Expr::TupleProj { .. } => "tuple projection",
        Expr::Quantifier { .. } => "quantifier",
    }
}

#[cfg(test)]
mod tests {
    use thermite_syntax::{parse, Item};

    use super::*;
    use crate::classifier::{classify, Verdict};

    fn bridge(src: &str, clause: &str) -> Result<S2Recon, BridgeError> {
        let parsed = parse(src);
        assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
        let Item::Fn(function) = &parsed.program.items[1] else {
            panic!("expected the second item to be an exec fn");
        };
        from_clause(
            &parsed.program,
            function,
            &function.contract.req,
            SourceAddress {
                item: function.name.clone(),
                clause: clause.to_string(),
            },
        )
    }

    #[test]
    fn source_quantifier_preserves_values_domains_and_de_bruijn_indices() {
        let recon = bridge(
            "spec fn widen(x: u32) -> u64 dec x { x as u64 }\n\
             fn f(xs: Vec<u32>, needle: u64) -> u64\n\
               req forall (i : usize) in xs. widen(xs[i]) != needle\n\
               ens result == 0 fx pure { 0 }",
            "req",
        )
        .expect("bridge");

        assert_eq!(recon.version, S2_RECON_VERSION);
        assert_eq!(recon.domains.len(), 1);
        assert_eq!(recon.domains[0].source_sort, "usize");
        assert!(recon.qfree_atoms.is_empty());
        assert_eq!(classify(&recon.formula), Verdict::Admitted);

        let wire = recon.canonical_wire();
        assert!(wire.contains("(domains (0 (m usize) 5:usize"));
        assert!(wire.contains("(r le (l (m usize) (i 0)) (v (m usize) 0))"));
        assert!(wire.contains("(r lt (v (m usize) 0) (ln (c (s (m u32))"));
        assert!(wire.contains("(a1 (m u32) (m u64) 0 (rd (m u32)"));
        assert!(wire.contains("(c (m u64) 0)"));
    }

    #[test]
    fn obligation_bridge_binds_the_counterexample_polarity() {
        let parsed = parse(
            "fn f(x: u64) -> u64\n\
             req x > 0\n\
             ens result >= x fx pure { x }",
        );
        assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
        let Item::Fn(function) = &parsed.program.items[0] else {
            panic!("expected function");
        };
        let recon = from_obligation(
            &parsed.program,
            function,
            &function.contract.req,
            &function.contract.ens[0],
            SourceAddress {
                item: "f".to_string(),
                clause: "ens#0".to_string(),
            },
        )
        .expect("obligation bridge");
        assert!(matches!(
            recon.formula,
            Frm::Conj(_, ref negated) if matches!(negated.as_ref(), Frm::Neg(_))
        ));
        let wire = recon.canonical_wire();
        assert!(wire.contains("(addr 1:f 5:ens#0)"));
        assert!(wire.contains("(cj (at (r gt"));
        assert!(wire.contains("(ng (at (r ge"));
    }

    #[test]
    fn literals_are_valued_and_constant_ids_do_not_alias() {
        let recon = bridge(
            "spec fn id(x: u64) -> u64 dec x { x }\n\
             fn f(a: u64, b: u64) -> u64\n\
               req id(a) == b && b != 17\n\
               ens result == 0 fx pure { 0 }",
            "req",
        )
        .expect("bridge");
        let wire = recon.canonical_wire();
        assert!(wire.contains("(c (m u64) 0)"));
        assert!(wire.contains("(c (m u64) 1)"));
        assert!(wire.contains("(l (m u64) (i 17))"));
    }

    #[test]
    fn qfree_leaf_retains_a_deterministic_source_ast() {
        let a = bridge(
            "spec fn id(x: u64) -> u64 dec x { x }\n\
             fn f(x: u64) -> u64 req x + x == 6 ens result == 0 fx pure { 0 }",
            "req",
        )
        .expect("bridge");
        let b = bridge(
            "spec fn id(x: u64) -> u64 dec x { x }\n\
             fn f(x: u64) -> u64 req x + x == 6 ens result == 0 fx pure { 0 }",
            "req",
        )
        .expect("bridge");
        assert_eq!(a.canonical_wire(), b.canonical_wire());
        assert_eq!(a.qfree_atoms.len(), 1);
        assert_eq!(a.qfree_atoms[0].fragment, QFreeFragment::Lia);
        assert!(a.qfree_atoms[0].canonical.contains("(bin add"));
    }

    #[test]
    fn repeated_qfree_source_expressions_share_one_canonical_atom() {
        let parsed = parse(
            "fn f(x: u64) -> u64\n\
             req x + x == 6\n\
             ens x + x == 6 fx pure { x }",
        );
        assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
        let Item::Fn(function) = &parsed.program.items[0] else {
            panic!("expected function");
        };
        let recon = from_obligation(
            &parsed.program,
            function,
            &function.contract.req,
            &function.contract.ens[0],
            SourceAddress {
                item: "f".to_string(),
                clause: "ens#0".to_string(),
            },
        )
        .expect("QF obligation bridge");
        assert_eq!(recon.qfree_atoms.len(), 1);
        let wire = crate::classifier::to_wire(&recon.formula);
        assert_eq!(wire.matches("(qf 0)").count(), 2, "{wire}");
        assert!(!wire.contains("(qf 1)"), "{wire}");
    }

    #[test]
    fn identical_qfree_text_under_lia_and_bv_keeps_distinct_semantics() {
        use thermite_syntax::{BvTag, Span};

        let parsed = parse(
            "fn f(x: u64) -> u64\n\
             req x + x == 6\n\
             ens x + x == 6 fx pure { x }",
        );
        assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
        let Item::Fn(parsed_function) = &parsed.program.items[0] else {
            panic!("expected function");
        };
        let mut function = parsed_function.clone();
        function.contract.ens[0].bv = Some(BvTag {
            width: BvWidth::W64,
            nowrap: false,
            span: Span::new(0, 0),
        });
        let recon = from_obligation(
            &parsed.program,
            &function,
            &function.contract.req,
            &function.contract.ens[0],
            SourceAddress {
                item: "f".to_string(),
                clause: "ens#0".to_string(),
            },
        )
        .expect("mixed QF obligation bridge");

        assert_eq!(recon.qfree_atoms.len(), 2);
        assert_eq!(recon.qfree_atoms[0].fragment, QFreeFragment::Lia);
        assert_eq!(
            recon.qfree_atoms[1].fragment,
            QFreeFragment::Bv(BvWidth::W64)
        );
        let wire = recon.canonical_wire();
        assert!(wire.contains("(0 lia "), "{wire}");
        assert!(wire.contains("(1 bv64 "), "{wire}");
    }

    #[test]
    fn division_is_a_bv_leaf_not_an_unchecked_lia_leaf() {
        use thermite_syntax::{BvTag, Span};

        let parsed = parse(
            "fn f(x: u64) -> u64\n\
             req x / 2 == 3\n\
             ens result == 0 fx pure { 0 }",
        );
        assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
        let Item::Fn(parsed_function) = &parsed.program.items[0] else {
            panic!("expected function");
        };
        let untagged = from_clause(
            &parsed.program,
            parsed_function,
            &parsed_function.contract.req,
            SourceAddress {
                item: "f".to_string(),
                clause: "req".to_string(),
            },
        );
        assert!(matches!(
            untagged,
            Err(BridgeError::UnsupportedTerm {
                context: "binary operator"
            })
        ));

        let mut function = parsed_function.clone();
        function.contract.req.bv = Some(BvTag {
            width: BvWidth::W32,
            nowrap: false,
            span: Span::new(0, 0),
        });
        let tagged = from_clause(
            &parsed.program,
            &function,
            &function.contract.req,
            SourceAddress {
                item: "f".to_string(),
                clause: "req".to_string(),
            },
        )
        .expect("tagged division uses the QF_BV leaf");
        assert_eq!(tagged.qfree_atoms.len(), 1);
        assert_eq!(
            tagged.qfree_atoms[0].fragment,
            QFreeFragment::Bv(BvWidth::W32)
        );
    }

    #[test]
    fn admitted_bridge_covers_the_complete_formula_relation_and_term_inventory() {
        let recon = bridge(
            "spec fn widen(x: u32) -> u64 dec x { x as u64 }\n\
             fn f(xs: Vec<u32>, x: u64, flag: bool) -> u64\n\
               req (((x == 0 || x != 1) && !(x < 2)) && x <= 3 && x > 4 && \
                 (x as u64) >= 5) && \
                 flag == true && \
                 forall (i : usize) in xs. \
                   ((i + 1 < xs.len()) || \
                    (widen(xs[i]) <= x * 2 && xs[i - 1] == 0))\n\
               ens result == 0 fx pure { 0 }",
            "req",
        )
        .expect("complete constructor bridge");
        assert_eq!(classify(&recon.formula), Verdict::Admitted);
        assert!(recon.qfree_atoms.is_empty());

        let wire = crate::classifier::to_wire(&recon.formula);
        for constructor in [
            "(cj ", "(dj ", "(ng ", "(im ", "(al ", "(r eq ", "(r ne ", "(r lt ", "(r le ",
            "(r gt ", "(r ge ", "(v ", "(c ", "(l ", "(rd ", "(ln ", "(ct ", "(ix ", "(ml ",
            "(a1 ",
        ] {
            assert!(
                wire.contains(constructor),
                "missing `{constructor}` in canonical formula: {wire}"
            );
        }
    }

    #[test]
    fn admitted_bridge_covers_existential_opaque_binders() {
        let recon = bridge(
            "spec fn keep(x: Key) -> Key dec 0 { x }\n\
             fn f(keys: Vec<Key>, needle: Key) -> u64\n\
               req exists (key : Key) in keys. key == needle\n\
               ens result == 0 fx pure { 0 }",
            "req",
        )
        .expect("existential opaque bridge");
        assert_eq!(classify(&recon.formula), Verdict::Admitted);
        let wire = crate::classifier::to_wire(&recon.formula);
        assert!(wire.contains("(ex (o 0)"), "{wire}");
        assert!(
            !wire.contains("(im "),
            "existential guard uses conjunction: {wire}"
        );
    }

    #[test]
    fn non_qfree_source_nodes_are_refused_instead_of_hidden_as_atoms() {
        let non_unary = bridge(
            "spec fn pair(x: u64, y: u64) -> u64 dec x { x }\n\
             fn f(x: u64) -> u64\n\
               req pair(x, x) == x\n\
               ens result == 0 fx pure { 0 }",
            "req",
        )
        .expect_err("a non-unary call is outside both EPR terms and scalar QF replay");
        assert!(matches!(
            non_unary,
            BridgeError::UnknownFunction { .. } | BridgeError::NonUnaryFunction { .. }
        ));

        let field = bridge(
            "spec fn keep(x: u64) -> u64 dec x { x }\n\
             fn f(x: u64) -> u64\n\
               req x.value == 0\n\
               ens result == 0 fx pure { 0 }",
            "req",
        )
        .expect_err("field access is not a scalar QF leaf");
        assert!(matches!(field, BridgeError::UnsupportedTerm { .. }));

        let unknown = bridge(
            "spec fn keep(x: u64) -> u64 dec x { x }\n\
             fn f(x: u64) -> u64\n\
               req ghost / 2 == 0\n\
               ens result == 0 fx pure { 0 }",
            "req",
        )
        .expect_err("QF fallback cannot invent an undeclared source value");
        assert!(matches!(
            unknown,
            BridgeError::UnknownValue { .. } | BridgeError::UnsupportedTerm { .. }
        ));
    }

    #[test]
    fn bound_multiplication_and_width_changing_casts_reach_named_classifier_refusals() {
        let multiplication = bridge(
            "spec fn keep(x: u64) -> u64 dec x { x }\n\
             fn f(xs: Vec<u64>) -> u64\n\
               req forall (i : usize) in xs. i * 2 < xs.len()\n\
               ens result == 0 fx pure { 0 }",
            "req",
        )
        .expect("bound multiplication has a canonical form before classification");
        assert_eq!(
            classify(&multiplication.formula),
            Verdict::Rejected(crate::classifier::RejectReason::IndexGrammar)
        );

        let cast = bridge(
            "spec fn keep(x: u32) -> u32 dec x { x }\n\
             fn f(xs: Vec<u32>) -> u64\n\
               req forall (i : usize) in xs. (i as u32) < xs[i]\n\
               ens result == 0 fx pure { 0 }",
            "req",
        )
        .expect("width-changing cast has a canonical form before classification");
        assert_eq!(
            classify(&cast.formula),
            Verdict::Rejected(crate::classifier::RejectReason::IndexGrammar)
        );
    }

    #[test]
    fn unsupported_bound_term_is_a_named_refusal_not_qfree() {
        let error = bridge(
            "spec fn id(x: usize) -> usize dec x { x }\n\
             fn f(xs: Vec<u64>) -> u64\n\
               req forall (i : usize) in xs. xs[i / 2] == 0\n\
               ens result == 0 fx pure { 0 }",
            "req",
        )
        .expect_err("bound division is outside the S₂.0 term language");
        assert!(matches!(
            error,
            BridgeError::UnsupportedTerm { .. } | BridgeError::BoundVariableInQFree { .. }
        ));
    }
}
