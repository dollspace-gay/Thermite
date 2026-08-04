//! Thermite AST node shapes — the structured output of the parser and the
//! boundary type consumed downstream by thermite-lower (#4) and forge (#5/#6).
//!
//! Governing design: `.design/syntax/ast.md`. The node set mirrors
//! `.design/syntax/surface-grammar.md` one-for-one. The mandatory-contract
//! rule (§4.1) is encoded in the types: `Contract.req`/`Contract.fx` are
//! non-`Option`, `Contract.ens` is a non-empty `Vec`, and `LoopNode` carries a
//! non-empty `invs` plus a single `dec`, so an ill-formed contract is
//! unrepresentable (ast.md REQ-2/REQ-5). The frontend is registry-free:
//! combinator calls (`forall_in`, `sorted`) are ordinary `Expr::Call` nodes.
//!
//! ## REQ status
//!
//! <!-- generated:reqs view=thermite-syntax-ast-core-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-SYNTAX-AST-ADDRESSABLE | shipped | `thermite-syntax/src/ast.rs` | AST addressability hooks |  |
//! | REQ-SYNTAX-AST-BLOCK-STMT | shipped | `thermite-syntax/src/ast.rs` | Block and statement AST nodes |  |
//! | REQ-SYNTAX-AST-CONTRACT | shipped | `thermite-syntax/src/ast.rs` | Mandatory contract AST node |  |
//! | REQ-SYNTAX-AST-EXPRS | shipped | `thermite-syntax/src/ast.rs` | Expression AST nodes |  |
//! | REQ-SYNTAX-AST-INTLIT-RAW | shipped | `thermite-syntax/src/ast.rs` | Integer literal value and raw spelling |  |
//! | REQ-SYNTAX-AST-ITEMS | shipped | `thermite-syntax/src/ast.rs` | Function item AST nodes |  |
//! | REQ-SYNTAX-AST-LOOPS | shipped | `thermite-syntax/src/ast.rs` | Addressable loop AST nodes |  |
//! | REQ-SYNTAX-AST-OPERATORS | shipped | `thermite-syntax/src/ast.rs` | Binary and unary operator AST set |  |
//! | REQ-SYNTAX-AST-PARTIAL-OPS | shipped | `thermite-syntax/src/ast.rs` | Partial operator proof obligations |  |
//! | REQ-SYNTAX-AST-PATTERN-TYPE-EFFECT | shipped | `thermite-syntax/src/ast.rs` | Pattern, type, and effect AST nodes |  |
//! | REQ-SYNTAX-AST-SLAG | shipped | `thermite-syntax/src/ast.rs` | Slag attribute AST node |  |
//! | REQ-SYNTAX-AST-SPANS | shipped | `thermite-syntax/src/ast.rs` | Span-bearing AST boundary stability |  |
//! <!-- /generated:reqs -->
//!
//! ## #16 boundary-fn additive schema (FFI boundary modules, `.design/boundary/ffi-boundary.md`)
//!
//! <!-- generated:reqs view=thermite-syntax-ast-ffi-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-SYNTAX-AST-BOUNDARY | shipped | `thermite-syntax/src/ast.rs` | Boundary function AST shape |  |
//! <!-- /generated:reqs -->
//!
//! ## #193 body-position holes (`.design/forge/goal-repl.md` REQ-4)
//!
//! <!-- generated:reqs view=thermite-syntax-ast-goal-repl-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-SYNTAX-AST-HOLES | shipped | `thermite-syntax/src/ast.rs` | Goal REPL body hole AST nodes |  |
//! <!-- /generated:reqs -->
//!
//! ## Basis Stage 1a — ADT surface AST nodes (`.design/basis/01-adts.md`)
//!
//! Surface-only (parse-into-the-right-AST); the validator rules (1b) and Verus
//! lowering (1c) are not in this crate.
//!
//! <!-- generated:reqs view=thermite-syntax-ast-adt-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-SYNTAX-ADT-BOX | shipped | `thermite-syntax/src/ast.rs` | Recursive Box type surface AST |  |
//! | REQ-SYNTAX-ADT-DEREF | shipped | `thermite-syntax/src/ast.rs` | Box dereference surface AST |  |
//! | REQ-SYNTAX-ADT-ENUM | shipped | `thermite-syntax/src/ast.rs` | ADT enum and struct literal surface AST |  |
//! | REQ-SYNTAX-ADT-IS | shipped | `thermite-syntax/src/ast.rs` | ADT `is` operator surface AST |  |
//! | REQ-SYNTAX-ADT-MATCH | shipped | `thermite-syntax/src/ast.rs` | ADT match pattern surface AST |  |
//! | REQ-SYNTAX-ADT-STRUCT | shipped | `thermite-syntax/src/ast.rs` | ADT struct surface AST |  |
//! <!-- /generated:reqs -->
//!
//! ## Basis Stage 4 — bounded-collection surface AST (`.design/basis/04-collections.md`)
//!
//! <!-- generated:reqs view=thermite-syntax-ast-collections-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-SYNTAX-COLLECTIONS-VEC | shipped | `thermite-syntax/src/ast.rs` | Vec type surface AST |  |
//! | REQ-SYNTAX-MAP-TYPE | shipped | `thermite-syntax/src/ast.rs` | Map type surface AST |  |
//! <!-- /generated:reqs -->
//!
//! ## Cluster C7 — built-in Option/Result surface AST (`.design/basis/09-option-result.md`, #95)
//!
//! <!-- generated:reqs view=thermite-syntax-ast-option-result-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-SYNTAX-OPTRES-OPTION | shipped | `thermite-syntax/src/ast.rs` | Option type surface AST |  |
//! | REQ-SYNTAX-OPTRES-RESULT | shipped | `thermite-syntax/src/ast.rs` | Result type surface AST |  |
//! <!-- /generated:reqs -->
//!
//! ## Cluster C9-A — plain-`fn` recursion AST (`.design/basis/10-recursion-tuples.md`, #108)
//!
//! <!-- generated:reqs view=thermite-syntax-ast-recursion-tuples-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-SYNTAX-RECURSION-DEC | shipped | `thermite-syntax/src/ast.rs` | Plain function decreases clause AST |  |
//! | REQ-SYNTAX-TUPLES-ARITY | shipped | `thermite-syntax/src/ast.rs` | Tuple arity disambiguation |  |
//! | REQ-SYNTAX-TUPLES-NODES | shipped | `thermite-syntax/src/ast.rs` | Tuple type, expression, and projection AST |  |
//! <!-- /generated:reqs -->
//!
//! ## Cluster C9-B — tuples AST (`.design/basis/10-recursion-tuples.md`, #109)
//!
//! Tuple status references are generated in the C9-A section above.
//!
//! ## Cluster C12 — bounded verified `Map<K,V>` surface AST (`.design/basis/13-map.md`, #123)
//!
//! <!-- generated:reqs view=thermite-syntax-ast-map-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-SYNTAX-MAP-METHODS | shipped | `thermite-syntax/src/ast.rs` | Map operations as method calls |  |
//! | REQ-SYNTAX-MAP-TYPE | shipped | `thermite-syntax/src/ast.rs` | Map type surface AST |  |
//! <!-- /generated:reqs -->
//!
//! ## Cluster C10 — binding/control-flow ergonomics AST (`.design/basis/11-ergonomics.md`, #112)
//!
//! <!-- generated:reqs view=thermite-syntax-ast-ergonomics-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-SYNTAX-ERGONOMICS-FOR | shipped | `thermite-syntax/src/parser.rs` | For-loop desugar |  |
//! | REQ-SYNTAX-ERGONOMICS-IF-WHILE-LET | shipped | `thermite-syntax/src/parser.rs` | If-let and while-let desugars |  |
//! | REQ-SYNTAX-ERGONOMICS-MATCH-GUARD | shipped | `thermite-syntax/src/ast.rs` | Match guard surface AST |  |
//! | REQ-SYNTAX-ERGONOMICS-OR-PATTERN | shipped | `thermite-syntax/src/ast.rs` | Or-pattern surface AST |  |
//! | REQ-SYNTAX-ERGONOMICS-TUPLE-DESTRUCTURE | shipped | `thermite-syntax/src/parser.rs` | Tuple destructuring desugar |  |
//! <!-- /generated:reqs -->

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
/// `enum` (sum types) item kinds. These are additive: existing
/// `Item::Fn`/`Item::SpecFn` consumers are unchanged in shape, while exhaustive
/// `match`es over `Item` downstream (thermite-spec/thermite-lower/forge) gain
/// the validate/lower arms in basis stages 1b/1c.
#[derive(Debug, Clone, PartialEq, Eq)]
// C9-A (#108): adding `FnItem.dec: Option<Clause>` (the recursive-fn termination
// measure) grew `Item::Fn` past clippy's `large_enum_variant` threshold (Fn ~560
// bytes vs SpecFn ~256). Boxing `Item::Fn(Box<FnItem>)` would ripple a `Box` deref
// to every exhaustive `match Item` across thermite-spec/thermite-lower/forge
// (dozens of value-pattern sites), a churn far beyond this clusters's scope, and
// `Item` is a value enum threaded by-value through the whole pipeline by design.
// The size asymmetry is benign (an `Item` vec holds few items; no hot copy path),
// so a per-item allow (R-CODE-3 / R-APG-3, not a module-root `#![allow]`) is the
// response here. crosslink observation: #108 builder.
#[allow(
    clippy::large_enum_variant,
    reason = "Item is a by-value pipeline enum; boxing Fn would churn every match Item site (#108)"
)]
pub enum Item {
    Fn(FnItem),
    SpecFn(SpecFnItem),
    /// A `struct NAME { field: type, … } [inv <expr>]` product type
    /// (`.design/basis/01-adts.md` REQ-1).
    Struct(StructItem),
    /// An `enum NAME { Variant, Variant(type, …), Variant { field: type, … } }`
    /// sum type (`.design/basis/01-adts.md` REQ-2).
    Enum(EnumItem),
    /// A Stage-1 forge-tier item (`.design/stage1-forge-tier.md` REQ-3): one of the
    /// proof-tier surface forms parsed beside `fn` — `prop fn`, `lemma`,
    /// `proof for`, `witness`. Grouped under one `Item` variant (with the kind
    /// distinguished by the inner [`ForgeItem`]) so the v1 downstream consumers
    /// (`thermite-spec` validation, `thermite-lower` lowering, `forge check`)
    /// dispatch every forge-tier item through a SINGLE match arm: they have no v1
    /// semantic consumer yet — the covenant engine is increment 2b (REQ-4), the
    /// tactic battery 2c (REQ-5), the proof view 2e (REQ-7), the lemma library 3
    /// (REQ-9). Increment 2a ships the PARSE + AST + ADDRESS + hole-gating surface
    /// (each kind has parse/address/round-trip tests); the consumers arrive next.
    Forge(ForgeItem),
}

impl Item {
    /// The item name — the root segment of every semantic address. For a
    /// `struct`/`enum` this is the type name; for a forge-tier item it is the
    /// [`ForgeItem::name`] (the prop/lemma name, the `proof for` target, or
    /// `"witness"`).
    pub fn name(&self) -> &str {
        match self {
            Item::Fn(f) => &f.name,
            Item::SpecFn(s) => &s.name,
            Item::Struct(s) => &s.name,
            Item::Enum(e) => &e.name,
            Item::Forge(forge) => forge.name(),
        }
    }
}

/// A Stage-1 forge-tier item (`.design/stage1-forge-tier.md` REQ-3, increment 2a).
/// The four proof-tier surface forms parsed beside `fn`. This is surface syntax
/// only: the AST faithfully represents each form (with `?pN` proof holes captured
/// in proof blocks), but the semantic consumers are later increments and are not
/// built here (REQ-4 covenant / REQ-5 battery / REQ-7 proof view / REQ-9 library).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForgeItem {
    /// `prop fn NAME(params) -> type { body }` — a proposition (logical predicate)
    /// definition, like a `spec fn` but a forge-tier proposition.
    PropFn(PropFnItem),
    /// `lemma NAME(params) req … ens … proof { … }` — a named lemma carrying a
    /// req/ens statement and a proof block.
    Lemma(LemmaItem),
    /// `proof for f { ens#k by { … } }` — a proof item discharging specific
    /// contract clauses (`ens#k`) of an existing function `f`.
    Proof(ProofItem),
    /// `witness { inhabit (…); falsify N; }` — a covenant witness block (the
    /// covenant logic is increment 2b; here parsed + represented only).
    Witness(WitnessBlock),
}

impl ForgeItem {
    /// The forge-tier item's address root: the prop/lemma name, the `proof for`
    /// target function name, or the literal `"witness"` (witness blocks are
    /// numbered `witness#N` by `address.rs`, mirroring `loop#N`).
    pub fn name(&self) -> &str {
        match self {
            ForgeItem::PropFn(p) => &p.name,
            ForgeItem::Lemma(l) => &l.name,
            ForgeItem::Proof(p) => &p.target,
            ForgeItem::Witness(_) => "witness",
        }
    }
}

/// A `prop fn NAME(params) -> type { body }` proposition definition
/// (`.design/stage1-forge-tier.md` REQ-3). A proposition is a logical predicate —
/// it mirrors [`SpecFnItem`] (params, return type, an expression body) but is a
/// forge-tier definition. `dec` is the optional termination measure for a
/// recursive proposition (a [`Clause`], the same `dec <measure>` /
/// `dec lex(…)` / `dec wf <rel>` surface the other `dec` positions accept).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropFnItem {
    pub name: Ident,
    pub params: Vec<Param>,
    pub ret: Type,
    pub dec: Option<Clause>,
    pub body: Block,
    pub span: Span,
}

/// A `lemma NAME(params) req … ens … proof { … }` item
/// (`.design/stage1-forge-tier.md` REQ-3). A lemma states a req/ens proposition
/// over its parameters and discharges it with a proof block. Unlike a `fn` it
/// carries no effect row (`fx`): a lemma is pure proof. `req` is the (single)
/// hypothesis clause; `ens` is the non-empty conclusion list; `proof` is the
/// proof block (which may carry open `?pN` proof holes). The proof MECHANICS
/// (citation resolution, dedup-on-burn) are increment 3 (REQ-9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LemmaItem {
    pub name: Ident,
    pub params: Vec<Param>,
    pub req: Clause,
    pub ens: Vec<Clause>,
    pub proof: ProofBlock,
    pub span: Span,
}

/// A `proof for f { ens#k by { … } }` item (`.design/stage1-forge-tier.md`
/// REQ-3). A proof item discharges one or more specific contract clauses of an
/// existing function `target` (`f`), each named by a [`ClauseSelector`] (`ens#k`)
/// and proved by a `by { … }` proof block. The clauses are resolved against `f`'s
/// contract by the proof view (increment 2e, REQ-7); here the surface is parsed
/// and addressed (`f.proof.ens#k`) only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofItem {
    /// The target function name `f` (the `proof for f` head). The address root.
    pub target: Ident,
    pub obligations: Vec<ProofObligation>,
    pub span: Span,
}

/// One `clause by { … }` obligation inside a [`ProofItem`]
/// (`.design/stage1-forge-tier.md` REQ-3): a [`ClauseSelector`] (`ens#k`) plus the
/// proof block discharging it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofObligation {
    pub clause: ClauseSelector,
    pub proof: ProofBlock,
    pub span: Span,
}

/// A reference to a specific contract clause of a function, e.g. `ens#k`
/// (`.design/stage1-forge-tier.md` REQ-3). `keyword` is the clause family
/// (`"ens"`/`"req"`/`"inv"`); `index` is the `#k` ordinal, or `None` for an
/// unindexed family (`req`). The surface spelling of the `f.proof.ens#k` address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClauseSelector {
    pub keyword: Ident,
    pub index: Option<u32>,
}

/// A forge-tier proof block — the `{ … }` body of a `lemma`/`proof` form
/// (`.design/stage1-forge-tier.md` REQ-3). The block's tactic content is not
/// structurally parsed here (the frozen tactic battery is increment 2c, REQ-5):
/// the block is captured as the verbatim source `text` plus the open proof holes
/// (`?pN`) it carries, in document order, so the proof view (2e) and the battery
/// (2c) can consume it next. `holes` carry [`HoleContext::Proof`]; a body hole
/// `?N` inside a proof block is a structured parse error
/// (`SyntaxError::BodyHoleInProofBlock`). An open proof hole blocks build and
/// certification (AC-7) once the forge consumers land.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofBlock {
    /// The verbatim source text between (and excluding) the proof block's braces.
    pub text: String,
    /// The open proof holes (`?pN`) in the block, in document order (each with
    /// [`HoleContext::Proof`]).
    pub holes: Vec<Hole>,
    pub span: Span,
}

/// A `witness { inhabit (…); falsify N; }` covenant witness block
/// (`.design/stage1-forge-tier.md` REQ-3/REQ-4). The covenant logic — type-checking
/// and executing `inhabit` witnesses against `req`, running the `falsify`
/// generator — is increment 2b (REQ-4); here the surface is parsed + represented +
/// round-tripped only (no execution, no covenant record produced).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitnessBlock {
    pub inhabits: Vec<Inhabit>,
    pub falsifies: Vec<Falsify>,
    pub span: Span,
}

/// An `inhabit (e, …);` directive inside a [`WitnessBlock`]
/// (`.design/stage1-forge-tier.md` REQ-4): an author-stated witness tuple of
/// expressions the covenant engine (2b) will type-check + execute against `req`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inhabit {
    pub args: Vec<Expr>,
    pub span: Span,
}

/// A `falsify N;` directive inside a [`WitnessBlock`]
/// (`.design/stage1-forge-tier.md` REQ-4): the `falsify` generator budget (the
/// number of random inputs to try; Q3 default 50_000 when unstated — applied by the
/// covenant engine, 2b). `budget` is the verbatim integer as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Falsify {
    pub budget: u64,
    pub span: Span,
}

/// A `struct NAME { field: type, … }` product-type item, optionally carrying a
/// type-invariant `inv <expr>` clause (`.design/basis/01-adts.md` REQ-1). The
/// `inv` reuses the existing [`Clause`] (verbatim text + parsed expr); it is
/// `None` when the struct declares no invariant. Stage 1b validates field
/// access against `fields`; stage 1c lowers the `inv` to a Verus `well_formed`
/// predicate.
///
/// `sealed` carries the `#[sealed]` abstraction-barrier attribute
/// (`.design/basis/06-provenance-and-sinks.md` REQ-8): a `#[sealed]` struct is a
/// door-only-mintable clean/capability type. The validator rejects any
/// `Expr::StructLit` of a sealed struct (`SpecError::SealedConstruction`), so the
/// only way to obtain one is through its `#[boundary]` door's return value (the
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
/// A structural invariant the parser upholds: `boundary.is_some()` iff
/// `body.is_none()`. A foreign (boundary) fn carries a `#[boundary("crate::path")]`
/// attribute and no Thermite body (`body: None`); its body is the foreign
/// crate's, enforced at L1 (`.design/boundary/ffi-boundary.md` §"surface form").
/// An in-language fn carries `boundary: None` and a real `body: Some(Block)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FnItem {
    pub slag: Option<SlagAttr>,
    /// The `#[boundary("crate::path")]` attribute marking a foreign fn (ffi
    /// REQ-2). `Some` iff this is a boundary fn (and then `body` is `None`).
    pub boundary: Option<BoundaryAttr>,
    pub name: Ident,
    pub params: Vec<Param>,
    pub ret: Type,
    pub contract: Contract,
    /// The optional `dec <measure>` termination clause of a recursive exec `fn`
    /// (`.design/basis/10-recursion-tuples.md` REQ-1, C9-A). Mirrors
    /// [`SpecFnItem::dec`] (a spec fn's `dec` is mandatory; an exec fn's is
    /// optional, with a non-recursive `fn` carrying `dec = None`). When `Some`, the
    /// lowerer emits a `decreases <measure>` on the Verus `fn` (the same measure
    /// position the spec-fn / loop `decreases` use) so Verus proves termination of
    /// the self-recursion; a self-calling `fn` without this (and not `fx diverge`)
    /// is a validator error (REQ-2). The clause parses after `fx` (REQ-1, OQ-4,
    /// keeping the `req`/`ens`/`fx` parse byte-stable), mirroring the loop order
    /// where `dec` follows the `inv`s.
    pub dec: Option<Clause>,
    /// The Thermite body — `Some(Block)` for an in-language fn, `None` for a
    /// boundary fn (the body is foreign; ffi REQ-2).
    pub body: Option<Block>,
    /// The open body holes (`?N`) this fn carries (`.design/forge/goal-repl.md`
    /// REQ-4, #193), in document (source) order: the order their `<fn>.?N`
    /// addresses are numbered (`semantic-addressing.md` / `address.rs`) and the
    /// order `forge goal` lists them. Empty for every hole-free fn (the entire
    /// pre-#193 corpus), so this is an additive field: a non-hole `FnItem`
    /// literal sets `holes: Vec::new()`, mirroring the `dec: None` additive
    /// precedent (C9-A). A fn with any hole never certifies; `forge check`
    /// short-circuits it to a non-certified L0 cert with an `OpenHole` cause before
    /// lowering (`.design/forge/goal-repl.md` REQ-5; the same short-circuit shape
    /// the vacuity gate uses), so a hole is recorded here, not threaded into the
    /// statement stream. It never lowers, so the `Stmt` enum and every exhaustive
    /// `match Stmt` stay untouched. The parser records a hole here when it sees a
    /// `?N` in fn-body statement position (`parser.md` REQ-11).
    pub holes: Vec<Hole>,
    /// transient refinement-type sugar (`.design/stage1-forge-tier.md` REQ-3): the
    /// `x: T{P}` parameter refinements and the `-> T{P}` return refinement the
    /// parser captured on this fn, before the post-parse desugar pass folds them
    /// into the contract. The pass [`crate::desugar::desugar_refinements`] runs at
    /// the end of [`crate::parse`] and (a) folds each parameter refinement into the
    /// `req` clause (`req && P` — so a caller automatically owes the refinement as a
    /// call-site obligation, Verus-checked), (b) appends each return refinement as
    /// an `ens` clause, then (c) CLEARS this vec. So in every `parse()` output this
    /// is empty — downstream stages (`thermite-spec` validation, lowering) see only
    /// the v1 `req`/`ens` clause shapes (REQ-3 "downstream sees only v1 clause
    /// shapes plus the new item kinds"). It is `Vec::new()` on every non-refined
    /// `FnItem` literal, mirroring the `holes: Vec::new()` / `dec: None` additive
    /// precedent.
    pub refinements: Vec<Refinement>,
    pub span: Span,
}

/// A refinement-type sugar predicate captured on a [`FnItem`]
/// (`.design/stage1-forge-tier.md` REQ-3), before the post-parse desugar pass folds
/// it into the contract. A `x: T{P}` parameter refinement targets the parameter;
/// a `-> T{P}` return refinement targets the result. transient: present only
/// between parsing and [`crate::desugar::desugar_refinements`], which folds it into
/// `req`/`ens` and clears it (so it never reaches downstream stages).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refinement {
    pub target: RefinementTarget,
    /// The refinement predicate `P` (a contract-position expression over the
    /// parameter, or over `result` for a return refinement), as a [`Clause`].
    pub pred: Clause,
}

/// What a [`Refinement`] constrains (`.design/stage1-forge-tier.md` REQ-3): a named
/// parameter (`x: T{P}` → folds into `req`) or the function result (`-> T{P}` →
/// folds into `ens`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefinementTarget {
    Param(Ident),
    Result,
}

/// An open body hole `?N` (`.design/forge/goal-repl.md` REQ-4, #193). A hole is a
/// structural placeholder the agent fills via `forge fill <fn>.?N <code>`. It
/// carries the verbatim hole number as written (`number`, the surface ordinal:
/// `?0` → `0`) and the source span of the `?N` token so `forge fill` can splice
/// replacement source text at that position (`goal_repl::fill_hole`). A
/// hole is not a `Stmt` (it never lowers; a holed item short-circuits at
/// `forge check`, REQ-5) and is not separately addressable beyond its `<fn>.?N`
/// address (`address.rs`). The address ordinal is the hole's document-order index
/// among the fn's holes (`AddrKind::Hole`), which may differ from the surface
/// `number` if the agent reuses or skips numbers (the oracle re-presents the
/// addresses every turn, §5.1 property 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hole {
    /// The verbatim hole number as the agent wrote it (`?0`/`?p0` → `0`).
    pub number: u32,
    /// The source span of the `?N`/`?pN` token (the splice target for `forge fill`).
    pub span: Span,
    /// Which surface sigil this hole was written with — a body hole `?N` or a
    /// proof hole `?pN` (`.design/stage1-forge-tier.md` REQ-3). The forge routes
    /// the two differently (a body hole is a code goal, a proof hole a proof goal),
    /// and they are addressed differently (`<fn>.?N` vs `<item>.proof.…`), so the
    /// context rides the node rather than being re-derived from position.
    pub context: HoleContext,
}

/// The surface sigil a [`Hole`] was written with (`.design/stage1-forge-tier.md`
/// REQ-3). `Body` is the body-position hole `?N` (#193, exec-fn-body statement
/// position only); `Proof` is the proof hole `?pN` (the forge tier, valid only
/// inside a proof block). A hole in the wrong position for its context is a
/// structured parse error, never a silent reclassification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoleContext {
    /// A body hole `?N` (#193): an open code goal in exec-fn-body statement
    /// position, filled by `forge fill <fn>.?N <code>`.
    Body,
    /// A proof hole `?pN` (the forge tier, REQ-3): an open proof goal inside a
    /// proof block, filled by `forge fill <item>.proof.…?pN <proof>`.
    Proof,
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
/// body-unproven (here, foreign) while leaving the contract mandatory. The single
/// positional `target` string names the foreign `crate::path` the L1 wrapper calls
/// (OQ-1: a boundary has one datum, so a positional string, not the named
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

/// The fixed bit-width of a `@bv` machine-semantics clause tag
/// (`.design/stage3-bv-reconstruction.md` REQ-1). Exactly the four widths the
/// RFC commits to (RFC-1 §4); any other `bvN` spelling is a parse error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BvWidth {
    W8,
    W16,
    W32,
    W64,
}

impl BvWidth {
    /// The width in bits (`8`/`16`/`32`/`64`).
    pub fn bits(self) -> u32 {
        match self {
            BvWidth::W8 => 8,
            BvWidth::W16 => 16,
            BvWidth::W32 => 32,
            BvWidth::W64 => 64,
        }
    }

    /// The canonical surface spelling (`bv8`/`bv16`/`bv32`/`bv64`).
    pub fn spelling(self) -> &'static str {
        match self {
            BvWidth::W8 => "bv8",
            BvWidth::W16 => "bv16",
            BvWidth::W32 => "bv32",
            BvWidth::W64 => "bv64",
        }
    }
}

/// A fixed-width tag on a postcondition, invariant, or lemma conclusion.
/// `nowrap` adds a no-overflow side obligation. The parser accepts the tag only
/// when the `bv` feature includes its certificate plumbing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BvTag {
    pub width: BvWidth,
    /// Whether the clause requests a no-overflow side obligation.
    pub nowrap: bool,
    /// The source span of the whole tag, from `@` through the closing `)` (or the
    /// width token when no `(nowrap)` follows).
    pub span: Span,
}

/// A clause carrying its parsed expression and the verbatim source text it was
/// built from. The `text` is the oracle string `address.rs` resolves an
/// `inv`/`dec` address to (semantic-addressing.md AC-1/AC-2).
///
/// `bv` is present on a tagged postcondition, invariant, or lemma conclusion.
/// The tag sits outside `text`, so semantic addresses remain unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clause {
    pub expr: Expr,
    pub text: String,
    pub span: Span,
    pub bv: Option<BvTag>,
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
    /// Terminal-control effect (`fx term`, issue #106): the boundary issues the
    /// `ioctl` syscall (termios `tcgetattr`/`tcsetattr`). A bare atom (no path
    /// arg), like `time`/`rand`. Its runtime-sandbox grant is `{ioctl:16}`
    /// (runtime-sandbox.md REQ-7); it carries no proof obligation (only the
    /// syscall grant + the §4.1 row-subsumption every atom is subject to).
    Term,
    /// A privileged operation in one frozen kernel platform domain
    /// (`.design/build/kernel-primitives.md` REQ-KPRIM-1). The domain
    /// is closed by [`PlatformDomain`]; a source program cannot mint an
    /// unregistered platform effect by spelling an arbitrary identifier.
    Platform(PlatformDomain),
}

/// The closed authority/effect domains available to consumer-supplied kernel
/// platform layers (`.design/build/kernel-primitives.md` REQ-KPRIM-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PlatformDomain {
    Boot,
    Memory,
    Mmio,
    Pio,
    Irq,
    Cpu,
    Atomic,
    Smp,
    Dma,
    Clock,
    Entropy,
    Power,
}

impl PlatformDomain {
    /// Every frozen kernel-platform domain, in canonical surface order.
    pub const ALL: [Self; 12] = [
        Self::Boot,
        Self::Memory,
        Self::Mmio,
        Self::Pio,
        Self::Irq,
        Self::Cpu,
        Self::Atomic,
        Self::Smp,
        Self::Dma,
        Self::Clock,
        Self::Entropy,
        Self::Power,
    ];

    /// Parse the one canonical surface spelling of a platform domain. The
    /// parser is the production consumer of this closed mapping.
    pub fn from_surface(surface: &str) -> Option<Self> {
        match surface {
            "boot" => Some(Self::Boot),
            "memory" => Some(Self::Memory),
            "mmio" => Some(Self::Mmio),
            "pio" => Some(Self::Pio),
            "irq" => Some(Self::Irq),
            "cpu" => Some(Self::Cpu),
            "atomic" => Some(Self::Atomic),
            "smp" => Some(Self::Smp),
            "dma" => Some(Self::Dma),
            "clock" => Some(Self::Clock),
            "entropy" => Some(Self::Entropy),
            "power" => Some(Self::Power),
            _ => None,
        }
    }

    /// The canonical effect-row spelling used by manifests and lowering.
    pub fn surface(self) -> &'static str {
        match self {
            Self::Boot => "boot",
            Self::Memory => "memory",
            Self::Mmio => "mmio",
            Self::Pio => "pio",
            Self::Irq => "irq",
            Self::Cpu => "cpu",
            Self::Atomic => "atomic",
            Self::Smp => "smp",
            Self::Dma => "dma",
            Self::Clock => "clock",
            Self::Entropy => "entropy",
            Self::Power => "power",
        }
    }
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

/// A `loop`/`while` node, addressable (ast.md REQ-5). `invs` is non-empty and
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

/// A match arm `Pattern [if GUARD] => Expr` (ast.md REQ-6;
/// `.design/basis/11-ergonomics.md` REQ-3). The optional `guard` is the C10
/// match-guard `pat if cond => …`: a `bool`-valued [`Expr`] evaluated in the
/// arm's binding scope, lowered to the Verus-native guarded arm
/// (`pat if <guard> => body`). Per REQ-3 (GROUNDED): a guard does not
/// complete a match. The validator's exhaustiveness check treats a guarded arm
/// as covering none of its pattern's cases (the guard may fail), as
/// Rust/Verus does. `None` is an unguarded arm (the entire pre-C10 corpus).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchArm {
    pub pattern: Pattern,
    /// The optional `if <cond>` match guard (`.design/basis/11-ergonomics.md`
    /// REQ-3). `Some(cond)` is a guarded arm `pat if cond => body`; `None` is an
    /// unguarded arm. A guarded arm covers no cases for exhaustiveness.
    pub guard: Option<Expr>,
    pub body: Expr,
}

/// A binary operator (ast.md REQ-6/REQ-10; §4.4). The arithmetic/comparison/
/// logical core is the v0.1 base; `Rem`/`Shl`/`Shr`/`BitAnd`/`BitOr`/`BitXor` are
/// the #92 integer-operator additions (their precedence is pinned in
/// `surface-grammar.md` REQ-10). `Rem` (`%`) inherits `Div`'s divide-by-zero proof
/// obligation; `Shl`/`Shr` raise a shift-bound obligation. Both are Verus-native
/// (ast.md REQ-11), discharged at L3, not a parse/lowering check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    /// `%` — remainder (#92). Partial: requires a nonzero divisor (ast.md REQ-11).
    Rem,
    /// `<<` — left shift (#92). Partial: requires a bounded shift amount.
    Shl,
    /// `>>` — right shift (#92). Partial: requires a bounded shift amount.
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

/// A unary (prefix) operator (ast.md REQ-10, #92). There is one `UnaryOp::Not`
/// for the prefix `!`: its meaning is per the operand type (logical-not on
/// `bool`, bitwise-not on an integer), resolved downstream (validator/lower) by
/// Verus's type-directed `!`, not by a syntactic split (§2.3 "one way to do
/// everything"; ast.md OQ-4). Prefix `!` binds tighter than every binary operator
/// (`surface-grammar.md` REQ-10), so `!a & b` parses as `(!a) & b`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
}

/// A quantifier binder head (`.design/stage2-stratified-cage.md` REQ-0): the `forall`
/// or `exists` of a raw quantified formula. The two surface keywords [`crate::lexer::TokKind::Forall`]
/// / [`crate::lexer::TokKind::Exists`] map to these; the stratified classifier (REQ-1/REQ-4)
/// keys on the kind. Distinct from the registry-free `forall_in`/`exists_in` COMBINATOR
/// calls, which remain ordinary [`Expr::Call`] nodes (the combinator registry is untouched).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quant {
    Forall,
    Exists,
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
    /// An integer literal carrying both the numeric `value` (with `_`
    /// separators stripped, ast.md REQ-6 value, the original semantics
    /// unchanged) and the verbatim source `raw` (separators included, ast.md
    /// REQ-6 raw, #37). `1_000_000` parses to `{ value: 1000000, raw:
    /// "1_000_000" }`. Lowering/mutation/vacuity consume `value`, not
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
    /// `bool`, bitwise-not on an integer), resolved downstream by Verus's
    /// type-directed `!`, not by a syntactic split (§2.3). Prefix `!` binds tighter
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
    /// constructed via the existing `Path`/`Call` nodes (REQ-2); only the
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
    /// fits; `Ref` is its inverse); its semantics (the `Box` deref Verus reads
    /// transparently with `*`) are stage 1c. Surface-only here.
    Deref(Box<Expr>),
    /// A string literal in expression position `"hello"`
    /// (`.design/basis/07-strings.md` REQ-1): the decoded literal text, mirroring
    /// the value-carrying [`Expr::IntLit`] / [`Expr::BoolLit`] literal precedent.
    /// The literal lexes today (`TokKind::Str(String)` in `lexer.rs`, consumed by
    /// `parse_slag`/`parse_attribute` for `#[slag]`/`#[boundary]` field values);
    /// this node is the addition of accepting it as an `Expr` (`parse_primary`).
    /// A `String` literal lowers to an owned `TString` materialized by pushing
    /// each UTF-8 byte (the char model is `u8` for v1, stage 7c, `lower.rs`); it
    /// is a constructing op carrying `fx alloc`.
    StrLit(String),
    /// An n-tuple construction `(a, b, …)` of arity ≥ 2
    /// (`.design/basis/10-recursion-tuples.md` REQ-5/REQ-7, C9-B): the value form
    /// of [`Type::Tuple`] (`swap`'s body `(b, a)`). The parser distinguishes arity
    /// by the comma: `(e)` is a parenthesised grouping (arity 1, the inner expr),
    /// `(a, b, …)` is `Expr::Tuple` (arity ≥ 2); the empty `()` is not a value form
    /// (v1 surfaces unit only as a return type). Lowers to the Verus-native tuple
    /// `(<e0>, <e1>, …)`. Its effects are the union of its elements' effects (a
    /// tuple construction is otherwise pure).
    Tuple(Vec<Expr>),
    /// A tuple projection `e.0`/`e.1`/… (`.design/basis/10-recursion-tuples.md`
    /// REQ-5/REQ-8, C9-B; OQ-1 resolved to a dedicated node, not an overloaded
    /// [`Expr::Field`] with a string `"0"` name: a tuple index is a `usize`, and a
    /// dedicated node keeps the projection lowering distinct from struct/method
    /// `.field`). The v1 §2.3 "one way" tuple access (destructuring is deferred).
    /// Parsed in the postfix `.` ladder (`parse_postfix`) when the token after `.`
    /// is a numeric literal. Works in both exec and spec/contract position: an
    /// `ens result.0 == b` is the GROUNDED Verus form `r.0 == b`. Lowers to
    /// the Verus-native projection `<recv>.<index>`. A projection is pure (its
    /// effects are its receiver's).
    TupleProj {
        receiver: Box<Expr>,
        index: usize,
    },
    /// A raw quantified formula `forall (x : S) in <dom>. φ` / `exists (x : S) in
    /// <dom>. φ` (`.design/stage2-stratified-cage.md` REQ-0): the surface binder
    /// production the (R2) index grammar admits, over a named sorted carrier. `quant`
    /// is the binder kind; `var` is the bound variable `x`; `sort` names its carrier
    /// sort `S`; `domain` is the `<dom>` expression it ranges over (parsed after the
    /// contextual `in`, e.g. a slice/carrier); `body` is the quantified formula φ.
    ///
    /// This is the foundation increment blocking REQ-1 (the Lean `Strat/Syntax` denote
    /// path) and REQ-4 (the Rust classifier): until raw `forall`/`exists` parse, the
    /// classifier cannot see a quantified formula. It is distinct from the
    /// registry-free `forall_in`/`forall_below`/`forall_from`/`sorted` COMBINATOR calls
    /// — those stay ordinary [`Expr::Call`] nodes and the combinator registry
    /// (`thermite-spec/src/combinators.rs`) is untouched as surface syntax. Surface +
    /// parse only here; the binder denotation/lowering land in the later Strat
    /// increments (REQ-1/REQ-8). The `body` is greedy (lowest precedence): `forall …. a
    /// && b` reads the whole `a && b` as the body; parenthesize to bound it.
    Quantifier {
        quant: Quant,
        var: Ident,
        sort: Ident,
        domain: Box<Expr>,
        body: Box<Expr>,
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
    /// An or-pattern `p0 | p1 | …` (`.design/basis/11-ergonomics.md` REQ-4): a
    /// `|`-joined alternation matching any one of its alternatives, lowered to the
    /// Verus-native or-pattern `p0 | p1 | … => body`. Exhaustiveness (REQ-4,
    /// GROUNDED): an `Or` covers the union of its alternatives' covered
    /// cases. `Some(_) | None` is exhaustive over `Option`, the validator counts
    /// each alternative toward the covered set. v0.1 admits literal/variant
    /// alternatives that bind the same set of names (OQ-3: payload-free
    /// alternatives sidestep Verus's same-bindings rule). Never nested in v0.1
    /// (`(a | b) | c` flattens at the parser).
    Or(Vec<Pattern>),
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
    U8,
    U16,
    U32,
    U64,
    Usize,
    Bool,
}

/// A type (ast.md REQ-7). `&[u32]` is `Ref` of `Slice`; `Option<usize>` is a
/// single-arg `Generic`. `Unit` is the `()` type, the one sanctioned unit
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
    /// Distinct from `Generic` (which requires `<arg>`, e.g. `Option<usize>`).
    Named(Ident),
    /// The heap-indirection primitive `Box<T>` (`.design/basis/01-adts.md`
    /// REQ-3, OQ-1 resolved: a dedicated first-class `Type` node, not a
    /// `Generic { name: "Box", .. }`, so the effect-subsumption check keys on the
    /// node kind rather than a string match). The recursive occurrence of a
    /// recursive `enum` (`Cons(u64, Box<List>)`); constructing a boxed value
    /// carries `fx alloc` (stage 1c).
    Box(Box<Type>),
    /// The bounded growable-collection primitive `Vec<T>`
    /// (`.design/basis/04-collections.md` REQ-1, OQ-2 resolved: a dedicated
    /// first-class node mirroring [`Type::Box`], not a `Generic { name: "Vec",
    /// .. }`, so the lowerer keys the vstd-`Vec` wrapper + capacity invariant +
    /// `fx alloc` emission on the node kind rather than a string-name match). A
    /// `Vec<T>` is the growth generalization of the read-only [`Type::Slice`]: a
    /// `&[T]` is a borrowed read-only view, a `Vec<T>` owns a growable backing run
    /// whose `Seq` view is `v@`. Its bounded operations `push`/`pop`/`get`/`len`
    /// are ordinary [`Expr::MethodCall`]s (no new expression node, the one call
    /// syntax, §4.4). Constructing / `push`-ing a `Vec` allocates, so the fn
    /// carries `fx alloc` (the Stage-1 [`Effect::Alloc`] heap, generalized; REQ-5).
    Vec(Box<Type>),
    /// The bounded owned text primitive `String` (`.design/basis/07-strings.md`
    /// REQ-2, OQ-3 resolved: a dedicated nullary node with no element-type
    /// indirection, unlike [`Type::Vec(Box<Type>)`], because the element type is
    /// fixed to `u8` (the char model is bytes for v1). Mirrors the [`Type::Vec`]/
    /// [`Type::Box`] dedicated-node decision so the lowerer keys the `TString`
    /// wrapper + capacity invariant + `fx alloc` emission on the node kind rather
    /// than a string-name match. A `String` is a bounded run of `u8` bytes, the
    /// shape of the verified bounded [`Type::Vec`] over `u8`. Its operations
    /// `len`/`byte_at`/`slice`/`concat` are ordinary [`Expr::MethodCall`]s and
    /// `==`/`+` are [`Expr::Binary`] (no new expression node, the one call
    /// syntax, §4.4). The borrowed `str`-view is `Ref { inner: String }` (the same
    /// way `&[T]` is `Ref` of `Slice`). Constructing / concatenating a `String`
    /// allocates, so the fn carries `fx alloc` (the Stage-1 [`Effect::Alloc`]).
    String,
    /// The built-in optional primitive `Option<T>`
    /// (`.design/basis/09-option-result.md` REQ-1, OQ-1 resolved: a dedicated
    /// `Type::Option(Box<Type>)` node, not a `Generic { name: "Option", .. }`,
    /// so the lowerer/validator key `Option` on the node kind, mirroring the
    /// [`Type::Vec`]/[`Type::Box`]/[`Type::String`] dedicated-node precedent. This
    /// makes `Option` stop being a string-named `Generic` (the OQ-1 ripple: every
    /// `Generic { name: "Option", .. }` reader is updated to read this node). Its
    /// constructors `Some(v)`/`None` reuse the existing [`Expr::Call`]/[`Expr::Path`]
    /// nodes (no reshape); `match`/`is` reuse [`Expr::Match`]/[`Expr::Is`]. Lowers
    /// to the Verus-native `Option<T>` (the `lower_type` `Option` arm).
    Option(Box<Type>),
    /// The built-in fallible primitive `Result<T, E>`
    /// (`.design/basis/09-option-result.md` REQ-2, OQ-1 resolved: a dedicated
    /// two-type-argument node, the first two-arg type in the grammar). The
    /// single-arg [`Type::Generic`] cannot parse
    /// `Result<u64, ParseErr>` (it dies at the comma). `Ok(v)`/`Err(e)` reuse the
    /// existing [`Expr::Call`] node; `match`/`is` reuse [`Expr::Match`]/[`Expr::Is`].
    /// Lowers to the Verus-native `Result<T, E>` (the `lower_type` `Result` arm).
    /// The `E` parameter is an ordinary user error enum (a [`Type::Named`]).
    Result(Box<Type>, Box<Type>),
    /// The built-in bounded verified key-value primitive `Map<K, V>`
    /// (`.design/basis/13-map.md` REQ-1, C12: the second two-type-argument node,
    /// mirroring [`Type::Result`]: a dedicated node, not a generalized multi-arg
    /// `Generic`, so the lowerer/validator key the `TMap` Vec-of-pairs wrapper +
    /// the spec abstraction view + the capacity/no-OOB contracts on the node kind.
    /// The single-arg [`Type::Generic`] cannot parse `Map<u64, u64>` (it dies at
    /// the comma, the C7 finding). The first arg is the key type, the second the
    /// value type. Its `insert`/`get`/`contains_key`/`len` ops are ordinary
    /// [`Expr::MethodCall`]s (no new expression node, the one call syntax, §4.4);
    /// `get` returns the C7 [`Type::Option`] (the no-OOB accessor,
    /// absent key → `None`). Lowers to a `TMap<K,V>` newtype over a
    /// `vstd::vec::Vec<(K, V)>`-of-pairs backing + a spec abstraction view
    /// (`spec_contains_key`/`spec_dom`); constructing / `insert`-ing a `Map`
    /// allocates, so the fn carries `fx alloc` (the Stage-1 [`Effect::Alloc`]).
    Map(Box<Type>, Box<Type>),
    /// An n-tuple type `(T, U, …)` of arity ≥ 2
    /// (`.design/basis/10-recursion-tuples.md` REQ-5/REQ-7, C9-B). The
    /// multiple-return / pair primitive: `fn swap(a, b: u64) -> (u64, u64)`. The
    /// parser distinguishes arity by the comma: `()` stays [`Type::Unit`] (arity
    /// 0), `(T)` is a parenthesised grouping (arity 1, the inner type), and
    /// `(T, U, …)` is `Type::Tuple` (arity ≥ 2). Lowers to the Verus-native tuple
    /// type `(<t0>, <t1>, …)` (the `lower_type` `Tuple` arm); Verus tuples are
    /// native and GROUNDED at arity 2 and 3. Its elements are accessed by the
    /// projection [`Expr::TupleProj`] (`.0`/`.1`/…), the v1 §2.3 "one way" tuple
    /// access (destructuring is deferred, REQ-9/OQ-2).
    Tuple(Vec<Type>),
}
