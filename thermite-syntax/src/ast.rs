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
//! | REQ-6 — VALUE (expression nodes incl. `IntLit` value) | SHIPPED | `enum Expr` with `Call`/`MethodCall`/`Field`/`Path`/... and `IntLit { value, .. }` carrying the numeric value; built by `parse_expr_bp`; lowered by `Expr::IntLit { value, .. } => value.to_string()`. |
//! | REQ-6 — RAW (`IntLit` verbatim raw on the Expr, #37) | SHIPPED | `Expr::IntLit { value: u128, raw: String }` (struct variant) in `ast.rs`; built by `parse_primary`/pattern-literal in `parser.rs` from `TokKind::Int { value, raw }`; `1_000_000` parses to `{ value: 1000000, raw: "1_000_000" }` (test `int_literal_preserves_value_and_raw`). Lowering still emits `value` (no golden churn). |
//! | REQ-7 (pattern/type/effect nodes) | SHIPPED | `enum Pattern`/`enum Type`/`enum EffectRow`; built by `parse_pattern`/`parse_type`. |
//! | REQ-8 (addressable nodes) | SHIPPED | `Item`/`LoopNode`/`Clause` carry source order; numbered by `address.rs`. |
//! | REQ-9 (spans + boundary stability) | SHIPPED | `Span` on `Item`/`LoopNode`/`Clause`; clauses also keep verbatim `text` for addressing. |
//! | REQ-6 — CHAR/HEX/BIN reuse `IntLit` (#91/#92) | SHIPPED | `'A'`/`0x1b`/`0b101` lex into `TokKind::Int` and `parse_primary` builds `Expr::IntLit { value, raw }` — NO new variant, ZERO match-arm churn; lowering emits the decimal `value`. Test `tests/operators_parse.rs::char_hex_binary_parse_to_intlit_no_new_variant`. |
//! | REQ-10 (binary + unary operator set, #92) | SHIPPED | `enum BinOp` += `Rem`/`Shl`/`Shr`/`BitAnd`/`BitOr`/`BitXor`; `enum UnaryOp { Not }` + `Expr::Unary { op, expr }` (the prefix `!`). Built by the `parser.rs` precedence ladder; the workspace match-arm ripple (lower/l1/effects/validator/mutation/vacuity/closure/review/check/strengthen/skill) is closed (no `_`/panic). GROUNDED L3 for all 7 forms (`forge/tests/operators_conformance.rs`). |
//! | REQ-11 (partial-operator obligations, #92) | SHIPPED | `lower.rs`/`l1.rs` `binop` emit the BARE Verus `%`/`<<`/`>>` (no `external`/`assume`, R-DEFER-9); Verus raises the div-by-zero / shift-bound obligation. GROUNDED: `%`/`<<` WITH their `req` → L3, WITHOUT → L0 (`forge/tests/operators_conformance.rs`). |
//!
//! ## Basis Stage 1a — ADT SURFACE AST nodes (`.design/basis/01-adts.md`)
//!
//! SURFACE-only (parse-into-the-right-AST); the VALIDATOR rules (1b) and Verus
//! LOWERING (1c) are NOT in this crate.
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 SURFACE (struct items + `inv` clause) | SHIPPED | `Item::Struct(StructItem)` with `StructItem { name, fields: Vec<FieldDef>, inv: Option<Clause>, span }` + `FieldDef { name, ty }`; built by `parse_struct` in `parser.rs`; asserted by `tests/adt_parse.rs` against `conformance/parse/bank_account.facts.json`. |
//! | REQ-2 SURFACE (enum items + struct-lit construction) | SHIPPED | `Item::Enum(EnumItem)` with `EnumItem { name, variants: Vec<VariantDef>, span }`, `VariantDef { name, shape }`, `VariantShape::{Unit,Tuple(Vec<Type>),Struct(Vec<FieldDef>)}`, and `Expr::StructLit { path, fields }`; built by `parse_enum`/`parse_struct_lit`; asserted by `tests/adt_parse.rs` (shape/bank_account facts). |
//! | REQ-3 SURFACE (recursive `Box<T>` type) | SHIPPED | `Type::Box(Box<Type>)` (OQ-1 RESOLVED — dedicated node, not `Generic`); built by `parser::parse_type` on the contextual `Box` ident; the `Cons(u64, Box<List>)` self-ref parses (`tests/adt_parse.rs` list_sum). The `alloc` effect / subsumption is stage 1c. |
//! | REQ-4 SURFACE (`match` over enum/struct patterns + binding) | SHIPPED | `Pattern::Struct { path, fields: Vec<(Ident, Pattern)>, rest }` added; the existing `Pattern::Enum` covers tuple/unit variants; `parse_match`/`parse_pattern` bind payloads (`Circle(r)`, `Rect { w, h }`, `Cons(h, t)`); asserted by `tests/adt_parse.rs` (shape + list_sum 2-arm matches). The exhaustiveness CHECK is stage 1b. |
//! | REQ-6 SURFACE (`Expr::Is` + `is` operator) | SHIPPED | `Expr::Is { scrutinee: Box<Expr>, variant: Vec<Ident> }`; built by the postfix `is` parse in `parser::parse_postfix`; `result == (s is Circle)` parses (`tests/adt_parse.rs` shape). The VALIDATOR rule (accept only declared variants) is stage 1b. |
//! | deref `*t` (REQ-3/REQ-4 surface) | SHIPPED | `Expr::Deref(Box<Expr>)` (new prefix-`*` unary; no existing node fit); built by `parser::parse_ref`; `sum_list(*t)` parses (`tests/adt_parse.rs` list_sum). Its SEMANTICS are stage 1c. |
//!
//! ## Basis Stage 4 — bounded-collection SURFACE AST (`.design/basis/04-collections.md`)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 SURFACE (`Vec<T>` type node) | SHIPPED | `Type::Vec(Box<Type>)` (OQ-2 RESOLVED — dedicated node, mirroring `Type::Box`, NOT `Generic`); built by `parser::parse_type` on the contextual `Vec` ident; `v: Vec<u64>` parses (`conformance/vec_demo.th`, asserted by `thermite-lower/tests/collections_conformance.rs`). The `push`/`pop`/`get`/`len` operations reuse `Expr::MethodCall` (no new node). The vstd-`Vec` wrapper + capacity invariant + `fx alloc` are Stage 4 lowering (`lower.rs`). |
//! | REQ-2 (`Map<K,V>` type) | NOT-STARTED | epic **#62** Stage 4 (OQ-3 thin-first-cut); `Map` deferred to a Stage-4 follow-up — `enum Type` has no `Map` node; the single-arg `Generic`/`Vec`/`Box` shapes do not carry a key+value. |

use crate::lexer::Span;

/// An identifier (a single name segment).
pub type Ident = String;

/// A whole parsed program: the recovered top-level items, in source order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    pub items: Vec<Item>,
}

/// A top-level item. v0.1 admits `fn` and `spec fn`; the basis ADT stage
/// (`.design/basis/01-adts.md` REQ-1/REQ-2) adds `struct` (product types) and
/// `enum` (sum types) item kinds. These are PURELY ADDITIVE: existing
/// `Item::Fn`/`Item::SpecFn` consumers are unchanged in shape — but exhaustive
/// `match`es over `Item` downstream (thermite-spec/thermite-lower/forge) gain
/// the validate/lower arms in basis stages 1b/1c.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    Fn(FnItem),
    SpecFn(SpecFnItem),
    /// A `struct NAME { field: TYPE, … } [inv <expr>]` product type
    /// (`.design/basis/01-adts.md` REQ-1).
    Struct(StructItem),
    /// An `enum NAME { Variant, Variant(TYPE, …), Variant { field: TYPE, … } }`
    /// sum type (`.design/basis/01-adts.md` REQ-2).
    Enum(EnumItem),
}

impl Item {
    /// The item name — the root segment of every semantic address. For a
    /// `struct`/`enum` this is the type name.
    pub fn name(&self) -> &str {
        match self {
            Item::Fn(f) => &f.name,
            Item::SpecFn(s) => &s.name,
            Item::Struct(s) => &s.name,
            Item::Enum(e) => &e.name,
        }
    }
}

/// A `struct NAME { field: TYPE, … }` product-type item, optionally carrying a
/// type-invariant `inv <expr>` clause (`.design/basis/01-adts.md` REQ-1). The
/// `inv` reuses the existing [`Clause`] (verbatim text + parsed expr); it is
/// `None` when the struct declares no invariant. Stage 1b validates field
/// access against `fields`; stage 1c lowers the `inv` to a Verus `well_formed`
/// predicate.
///
/// `sealed` carries the `#[sealed]` ABSTRACTION-BARRIER attribute
/// (`.design/basis/06-provenance-and-sinks.md` REQ-8): a `#[sealed]` struct is a
/// door-only-mintable clean/capability type — the validator REJECTS any
/// `Expr::StructLit` of a sealed struct (`SpecError::SealedConstruction`), so the
/// ONLY way to obtain one is through its `#[boundary]` door's return value (the
/// door body is foreign/`external_body`, with no in-language `StructLit`). It is
/// `false` for an ordinary struct (the parser sets it `true` only on `#[sealed]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructItem {
    pub name: Ident,
    pub fields: Vec<FieldDef>,
    pub inv: Option<Clause>,
    pub sealed: bool,
    pub span: Span,
}

/// A named, typed field of a `struct` or a struct-shaped enum variant
/// (`.design/basis/01-adts.md` REQ-1/REQ-2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldDef {
    pub name: Ident,
    pub ty: Type,
}

/// An `enum NAME { … }` sum-type item (`.design/basis/01-adts.md` REQ-2). Its
/// `variants` are the declared outcome set the exhaustive-`match` check (REQ-5,
/// stage 1b) and `is`-discrimination (REQ-6) key off.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumItem {
    pub name: Ident,
    pub variants: Vec<VariantDef>,
    pub span: Span,
}

/// One declared variant of an `enum` (`.design/basis/01-adts.md` REQ-2): a name
/// plus its payload [`VariantShape`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariantDef {
    pub name: Ident,
    pub shape: VariantShape,
}

/// The payload shape of an enum variant (`.design/basis/01-adts.md` REQ-2):
/// `Unit` (`Nil`), `Tuple` (`Circle(u64)`, `Cons(u64, Box<List>)`), or `Struct`
/// (`Rect { w: u64, h: u64 }`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VariantShape {
    Unit,
    Tuple(Vec<Type>),
    Struct(Vec<FieldDef>),
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
    /// `break;` — the loop-control statement (ast.md REQ-12, #93). Payload-less
    /// and value-less (no loop label, no `break expr` — §2.3). Lowers to the
    /// Verus-native `break;` (`verus-lowering.md` REQ-12). Valid only inside a
    /// loop body (the parser enforces the in-loop rule — `parser.md` REQ-10).
    Break,
    /// `continue;` — the loop-control statement (ast.md REQ-12, #93).
    /// Payload-less and value-less. Lowers to the Verus-native `continue;`; a
    /// `continue` is a loop back-edge owing the invariant + `decreases`
    /// obligations (Verus-checked — `verus-lowering.md` REQ-12).
    Continue,
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

/// A binary operator (ast.md REQ-6/REQ-10; §4.4). The arithmetic/comparison/
/// logical core is the v0.1 base; `Rem`/`Shl`/`Shr`/`BitAnd`/`BitOr`/`BitXor` are
/// the #92 integer-operator additions (their precedence is pinned in
/// `surface-grammar.md` REQ-10). `Rem` (`%`) inherits `Div`'s divide-by-zero PROOF
/// obligation; `Shl`/`Shr` raise a shift-bound obligation — both are Verus-native
/// (ast.md REQ-11), discharged at L3, NOT a parse/lowering check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    /// `%` — remainder (#92). PARTIAL: requires a nonzero divisor (ast.md REQ-11).
    Rem,
    /// `<<` — left shift (#92). PARTIAL: requires a bounded shift amount.
    Shl,
    /// `>>` — right shift (#92). PARTIAL: requires a bounded shift amount.
    Shr,
    /// `&` — bitwise and (#92).
    BitAnd,
    /// `|` — bitwise or (#92).
    BitOr,
    /// `^` — bitwise xor (#92).
    BitXor,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

/// A unary (prefix) operator (ast.md REQ-10, #92). There is ONE `UnaryOp::Not`
/// for the prefix `!`: its meaning is per the OPERAND TYPE — logical-not on
/// `bool`, bitwise-not on an integer — resolved DOWNSTREAM (validator/lower) by
/// Verus's type-directed `!`, NOT by a syntactic split (§2.3 "one way to do
/// everything"; ast.md OQ-4). Prefix `!` binds tighter than every binary operator
/// (`surface-grammar.md` REQ-10), so `!a & b` parses as `(!a) & b`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
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
    /// An integer literal carrying BOTH the numeric `value` (with `_`
    /// separators stripped — ast.md REQ-6 VALUE, the original semantics,
    /// UNCHANGED) and the verbatim source `raw` (separators included — ast.md
    /// REQ-6 RAW, #37). `1_000_000` parses to `{ value: 1000000, raw:
    /// "1_000_000" }`. CRITICAL: lowering/mutation/vacuity consume `value`, NOT
    /// `raw` (no golden churn); `raw` is AST-fidelity / round-trip only.
    IntLit {
        value: u128,
        raw: String,
    },
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
    /// A unary prefix application `!EXPR` (ast.md REQ-10, #92). The single
    /// `UnaryOp::Not` whose meaning is per the operand type (logical-not on
    /// `bool`, bitwise-not on an integer) — resolved downstream by Verus's
    /// type-directed `!`, NOT by a syntactic split (§2.3). Prefix `!` binds tighter
    /// than every binary, so `!a & b` is `(!a) & b`.
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
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
    /// A struct / struct-variant construction `Path { field: val, … }`
    /// (`.design/basis/01-adts.md` REQ-2): the literal that builds an
    /// `Account { balance: … }` or a struct-shaped enum variant. The `path` is
    /// the (possibly `::`-segmented) type/variant name; `fields` are the
    /// `name: value` initializers in source order. A unit/tuple variant is
    /// constructed via the existing `Path`/`Call` nodes (REQ-2) — only the
    /// brace-initializer form is new.
    StructLit {
        path: Vec<Ident>,
        fields: Vec<(Ident, Expr)>,
    },
    /// A variant-discrimination test `SCRUTINEE is Variant`
    /// (`.design/basis/01-adts.md` REQ-6): a `bool`-valued contract expression
    /// (`result is Circle`). The `variant` is the (possibly `::`-segmented)
    /// variant name. Stage 1b validates it against the scrutinee's declared
    /// variant set; stage 1c lowers it to the Verus `is` discriminant test.
    Is {
        scrutinee: Box<Expr>,
        variant: Vec<Ident>,
    },
    /// A dereference of a boxed value `*EXPR` (`.design/basis/01-adts.md` REQ-3,
    /// the recursive call `sum_list(*t)`). A new unary node (no existing node
    /// fits — `Ref` is its inverse); its SEMANTICS (the `Box` deref Verus reads
    /// transparently with `*`) are stage 1c. Surface-only here.
    Deref(Box<Expr>),
    /// A string literal in expression position `"hello"`
    /// (`.design/basis/07-strings.md` REQ-1): the decoded literal text, mirroring
    /// the value-carrying [`Expr::IntLit`] / [`Expr::BoolLit`] literal precedent.
    /// The literal LEXES today (`TokKind::Str(String)` in `lexer.rs`, consumed by
    /// `parse_slag`/`parse_attribute` for `#[slag]`/`#[boundary]` field values);
    /// this node is the addition of accepting it as an `Expr` (`parse_primary`).
    /// A `String` literal lowers to an owned `TString` materialized by pushing
    /// each UTF-8 byte (the char model is `u8` for v1 — stage 7c, `lower.rs`); it
    /// is a CONSTRUCTING op carrying `fx alloc`.
    StrLit(String),
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
    /// A struct / struct-variant destructuring pattern `Path { field: pat, … }`
    /// or `Path { .. }` (`.design/basis/01-adts.md` REQ-4): binds the named
    /// fields of a `struct` or struct-shaped enum variant (`Rect { w, h }`). The
    /// `rest` flag is the `..` of `Rect { .. }`. A `field` shorthand `Rect { w,
    /// h }` is sugar the parser expands to `(w, Pattern::Binding("w"))`.
    Struct {
        path: Vec<Ident>,
        fields: Vec<(Ident, Pattern)>,
        rest: bool,
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
    Ref {
        mutable: bool,
        inner: Box<Type>,
    },
    Slice(Box<Type>),
    Generic {
        name: Ident,
        arg: Box<Type>,
    },
    /// A bare user-defined type name — a `struct`/`enum` declared in the program
    /// (`.design/basis/01-adts.md` REQ-1/REQ-2): `Account`, `Shape`, `List`. A
    /// parameter `a: Account`, a return type `-> Shape`, and the recursive
    /// occurrence `Box<List>`'s inner `List` are all `Type::Named`. Without this
    /// node a user type could not appear in any type position (no ADT program
    /// would parse); it is the type-side complement of the `struct`/`enum` items.
    /// Distinct from `Generic` (which REQUIRES `<arg>`, e.g. `Option<usize>`).
    Named(Ident),
    /// The heap-indirection primitive `Box<T>` (`.design/basis/01-adts.md`
    /// REQ-3, OQ-1 RESOLVED: a dedicated first-class `Type` node, NOT a
    /// `Generic { name: "Box", .. }`, so the effect-subsumption check keys on the
    /// node kind rather than a string match). The recursive occurrence of a
    /// recursive `enum` (`Cons(u64, Box<List>)`); constructing a boxed value
    /// carries `fx alloc` (stage 1c).
    Box(Box<Type>),
    /// The bounded growable-collection primitive `Vec<T>`
    /// (`.design/basis/04-collections.md` REQ-1, OQ-2 RESOLVED: a dedicated
    /// first-class node mirroring [`Type::Box`], NOT a `Generic { name: "Vec",
    /// .. }`, so the lowerer keys the vstd-`Vec` wrapper + capacity invariant +
    /// `fx alloc` emission on the node KIND rather than a string-name match). A
    /// `Vec<T>` is the GROWTH generalization of the read-only [`Type::Slice`]: a
    /// `&[T]` is a borrowed read-only view, a `Vec<T>` owns a growable backing run
    /// whose `Seq` view is `v@`. Its bounded operations `push`/`pop`/`get`/`len`
    /// are ordinary [`Expr::MethodCall`]s (no new expression node — the one call
    /// syntax, §4.4). Constructing / `push`-ing a `Vec` allocates, so the fn
    /// carries `fx alloc` (the Stage-1 [`Effect::Alloc`] heap, generalized; REQ-5).
    Vec(Box<Type>),
    /// The bounded owned text primitive `String` (`.design/basis/07-strings.md`
    /// REQ-2, OQ-3 RESOLVED: a dedicated NULLARY node — no element-type
    /// indirection, unlike [`Type::Vec(Box<Type>)`], because the element type is
    /// FIXED to `u8` (the char model is bytes for v1). Mirrors the [`Type::Vec`]/
    /// [`Type::Box`] dedicated-node decision so the lowerer keys the `TString`
    /// wrapper + capacity invariant + `fx alloc` emission on the NODE KIND rather
    /// than a string-name match. A `String` is a bounded run of `u8` bytes, the
    /// EXACT shape of the verified bounded [`Type::Vec`] over `u8`. Its operations
    /// `len`/`byte_at`/`slice`/`concat` are ordinary [`Expr::MethodCall`]s and
    /// `==`/`+` are [`Expr::Binary`] (no new expression node — the one call
    /// syntax, §4.4). The borrowed `str`-view is `Ref { inner: String }` (the same
    /// way `&[T]` is `Ref` of `Slice`). Constructing / concatenating a `String`
    /// allocates, so the fn carries `fx alloc` (the Stage-1 [`Effect::Alloc`]).
    String,
}
