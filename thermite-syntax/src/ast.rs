//! Thermite AST node shapes — the structured output of the parser and the
//! boundary type consumed downstream by thermite-lower (#4) and forge (#5/#6).
//!
//! Governing design: `.design/syntax/ast.md`. The node set mirrors
//! `.design/syntax/surface-grammar.md` one-for-one. The mandatory-contract
//! rule (§4.1) is encoded in the TYPES: `Contract.req`/`Contract.fx` are
//! non-`Option`, `Contract.ens` is a non-empty `Vec`, and `LoopNode` carries a
//! non-empty `invs` plus a single `dec` — so an ill-formed contract is
//! unrepresentable (ast.md REQ-2/REQ-5). The frontend is REGISTRY-FREE:
//! combinator calls (`forall_in`, `sorted`) are ordinary `Expr::Call` nodes.
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (item nodes) | SHIPPED | `enum Item { Fn, SpecFn }`; consumer `parse_item` in `parser.rs`, asserted by `tests/conformance.rs`. |
//! | REQ-2 (contract node, mandatory fields) | SHIPPED | `struct Contract { req: Expr, ens: Vec<Expr>, fx: EffectRow }` — non-`Option`; built only in `parse_contract` after presence checks. |
//! | REQ-3 (slag attribute node) | SHIPPED | `struct SlagAttr` + `Fn.slag: Option<SlagAttr>`; parsed by `parse_slag` in `parser.rs`. |
//!
//! ## #16 boundary-fn additive schema (FFI boundary modules, `.design/boundary/ffi-boundary.md`)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | ffi REQ-2 (AST shape) | SHIPPED | `struct BoundaryAttr { target, span }` (mirrors `SlagAttr`) + `FnItem.boundary: Option<BoundaryAttr>` + `FnItem.body: Option<Block>` (a boundary fn is `boundary: Some`, `body: None`; an in-language fn is `boundary: None`, `body: Some`). Built by `parse_attribute`/`parse_fn` in `parser.rs`; consumed by `thermite_lower::l1::lower_l1` (the boundary L1 wrapper) and `forge`'s `check::gate_fn` (the `boundary_l1` cert). |
//! | REQ-4 (block + statement nodes) | SHIPPED | `struct Block`, `enum Stmt`; built by `parse_block`/`parse_stmt` in `parser.rs`. |
//! | REQ-5 (loop nodes, addressable) | SHIPPED | `struct LoopNode { kind, invs, dec, .. }`; addressed by `address.rs`. |
//! | REQ-6 (expression nodes) | SHIPPED | `enum Expr` with `Call`/`MethodCall`/`Field`/`Path`/... ; built by `parse_expr_bp`. |
//! | REQ-7 (pattern/type/effect nodes) | SHIPPED | `enum Pattern`/`enum Type`/`enum EffectRow`; built by `parse_pattern`/`parse_type`. |
//! | REQ-8 (addressable nodes) | SHIPPED | `Item`/`LoopNode`/`Clause` carry source order; numbered by `address.rs`. |
//! | REQ-9 (spans + boundary stability) | SHIPPED | `Span` on `Item`/`LoopNode`/`Clause`; clauses also keep verbatim `text` for addressing. |

use crate::lexer::Span;

/// An identifier (a single name segment).
pub type Ident = String;

/// A whole parsed program: the recovered top-level items, in source order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    pub items: Vec<Item>,
}

/// A top-level item. v0.1 admits exactly `fn` and `spec fn` (ast.md REQ-1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    Fn(FnItem),
    SpecFn(SpecFnItem),
}

impl Item {
    /// The function name — the root segment of every semantic address.
    pub fn name(&self) -> &str {
        match self {
            Item::Fn(f) => &f.name,
            Item::SpecFn(s) => &s.name,
        }
    }
}

/// A `fn` item with its mandatory contract and body (ast.md REQ-1/REQ-2/REQ-3;
/// ffi-boundary.md REQ-2).
///
/// A structural invariant the parser upholds: `boundary.is_some()` IFF
/// `body.is_none()`. A FOREIGN (boundary) fn carries a `#[boundary("crate::path")]`
/// attribute and NO Thermite body (`body: None`) — its body is the foreign
/// crate's, enforced at L1 (`.design/boundary/ffi-boundary.md` §"surface form").
/// An IN-LANGUAGE fn carries `boundary: None` and a real `body: Some(Block)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FnItem {
    pub slag: Option<SlagAttr>,
    /// The `#[boundary("crate::path")]` attribute marking a FOREIGN fn (ffi
    /// REQ-2). `Some` iff this is a boundary fn (and then `body` is `None`).
    pub boundary: Option<BoundaryAttr>,
    pub name: Ident,
    pub params: Vec<Param>,
    pub ret: Type,
    pub contract: Contract,
    /// The Thermite body — `Some(Block)` for an in-language fn, `None` for a
    /// boundary fn (the body is foreign; ffi REQ-2).
    pub body: Option<Block>,
    pub span: Span,
}

/// A `spec fn` item: carries only a `dec` measure, no `req`/`ens`/`fx`
/// (ast.md REQ-1; §4.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecFnItem {
    pub name: Ident,
    pub params: Vec<Param>,
    pub ret: Type,
    pub dec: Clause,
    pub body: Block,
    pub span: Span,
}

/// A `#[slag(reason=..., owner=..., review=...)]` attribute (ast.md REQ-3, §8).
/// Fields are stored verbatim; required-field-presence is a downstream check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlagAttr {
    pub reason: Option<String>,
    pub owner: Option<String>,
    pub review: Option<String>,
    pub span: Span,
}

/// A `#[boundary("crate::path::to::foreign_fn")]` attribute (ffi-boundary.md
/// REQ-1/REQ-2, §9). Mirrors `struct SlagAttr`: it marks a `fn` whose body is
/// body-unproven (here, FOREIGN) while leaving the contract mandatory. The single
/// positional `target` string names the foreign `crate::path` the L1 wrapper calls
/// (OQ-1: a boundary has exactly one datum, so a positional string, not the named
/// `key = "value"` fields `#[slag]` uses).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryAttr {
    /// The foreign target: a `crate::path` naming the foreign fn the L1 wrapper
    /// calls. Stored verbatim; non-emptiness is a downstream (forge) check.
    pub target: String,
    pub span: Span,
}

/// A function parameter `name: Type`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    pub name: Ident,
    pub ty: Type,
}

/// The mandatory contract of a `fn` (ast.md REQ-2). All three fields are
/// non-optional: `ens` is a `Vec` the parser only ever fills with ≥1 element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contract {
    pub req: Clause,
    pub ens: Vec<Clause>,
    pub fx: EffectRow,
}

/// A clause carrying its parsed expression AND the verbatim source text it was
/// built from. The `text` is the oracle string `address.rs` resolves an
/// `inv`/`dec` address to (semantic-addressing.md AC-1/AC-2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clause {
    pub expr: Expr,
    pub text: String,
    pub span: Span,
}

/// An effect row (ast.md REQ-7; §4.1). The corpus uses only `pure`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectRow {
    Pure,
    Set(Vec<Effect>),
}

/// A single effect in a non-`pure` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    Read(Ident),
    Write(Ident),
    Net(Ident),
    Alloc,
    Time,
    Rand,
    Panic,
    Diverge,
}

/// A `{ ... }` block: statements plus an optional trailing tail expression
/// (ast.md REQ-4). The `tail` is the block's value (`sum`'s final `acc`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub tail: Option<Box<Expr>>,
}

/// A statement (ast.md REQ-4). A loop appears in statement position (ast.md
/// OQ-1: the corpus never uses a loop's value).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    Let {
        mutable: bool,
        name: Ident,
        ty: Option<Type>,
        init: Expr,
    },
    Assign {
        target: Expr,
        value: Expr,
    },
    Return(Option<Expr>),
    If {
        cond: Expr,
        then: Block,
        else_: Option<Block>,
    },
    Loop(LoopNode),
    Expr(Expr),
}

/// A `loop`/`while` node — ADDRESSABLE (ast.md REQ-5). `invs` is non-empty and
/// `dec` is a single clause (structurally encoding §4.1). `while` and `loop`
/// share the `loop#N` namespace (semantic-addressing.md REQ-2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopNode {
    pub kind: LoopKind,
    pub invs: Vec<Clause>,
    pub dec: Clause,
    pub body: Block,
    pub span: Span,
}

/// The surface keyword of a loop (`loop` vs `while EXPR`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoopKind {
    Loop,
    While(Box<Expr>),
}

impl LoopKind {
    /// The surface keyword as written, for address/fact reporting.
    pub fn surface_keyword(&self) -> &'static str {
        match self {
            LoopKind::Loop => "loop",
            LoopKind::While(_) => "while",
        }
    }
}

/// A match arm `Pattern => Expr` (ast.md REQ-6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Expr,
}

/// A binary operator (ast.md REQ-6; §4.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

/// An index argument: `a[i]`, `a[..i]`, `a[i..]`, `a[i..j]` (ast.md REQ-6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexArg {
    Single(Box<Expr>),
    RangeTo(Box<Expr>),
    RangeFrom(Box<Expr>),
    Range(Box<Expr>, Box<Expr>),
}

/// An expression (ast.md REQ-6). `Call` is the free form `f(args)`,
/// `MethodCall` is the postfix `recv.m(args)`, `Field` is `recv.m` — the one
/// call syntax (surface-grammar.md REQ-6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    IntLit(u128),
    BoolLit(bool),
    Path(Vec<Ident>),
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    MethodCall {
        receiver: Box<Expr>,
        name: Ident,
        args: Vec<Expr>,
    },
    Field {
        receiver: Box<Expr>,
        name: Ident,
    },
    Closure {
        params: Vec<Ident>,
        body: Box<Expr>,
    },
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<MatchArm>,
    },
    If {
        cond: Box<Expr>,
        then: Block,
        else_: Block,
    },
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Index {
        base: Box<Expr>,
        index: IndexArg,
    },
    Cast {
        expr: Box<Expr>,
        ty: Type,
    },
    Ref {
        mutable: bool,
        expr: Box<Expr>,
    },
}

/// A pattern (ast.md REQ-7). Slice patterns `[]`/`[head, ..t]` and enum
/// patterns `Some(i)`/`None` per Appendix A + §4.1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pattern {
    Wildcard,
    Literal(Expr),
    Binding(Ident),
    Slice(Vec<SlicePat>),
    Enum {
        path: Vec<Ident>,
        fields: Vec<Pattern>,
    },
}

/// A sub-pattern inside a slice pattern, or a rest binding `..t` (ast.md REQ-7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlicePat {
    Pat(Pattern),
    Rest(Ident),
}

/// A primitive type name (ast.md REQ-7; §4.4 — no lifetimes, closed set).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimType {
    U32,
    U64,
    Usize,
    Bool,
}

/// A type (ast.md REQ-7). `&[u32]` is `Ref` of `Slice`; `Option<usize>` is a
/// single-arg `Generic`. `Unit` is the `()` type — the ONE sanctioned unit
/// spelling, written explicitly in a return position (surface-grammar.md
/// decision 4; §4.4 "All conversions explicit").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Prim(PrimType),
    Unit,
    Ref { mutable: bool, inner: Box<Type> },
    Slice(Box<Type>),
    Generic { name: Ident, arg: Box<Type> },
}
