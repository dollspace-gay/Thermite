//! `thermite-syntax` — the Thermite surface-syntax foundation: lexer, recovering
//! parser, AST, and stable semantic addressing (`loop#1.inv#2`).
//!
//! This is the leaf crate of the v0.1 kernel DAG (workspace REQ-2): it has no
//! intra-workspace dependencies. The four modules below are the executable form
//! of the surface grammar (`.design/syntax/`):
//!
//! - [`lexer`] — `.design/syntax/lexer.md`: source `&str` -> `Vec<Token>`.
//! - [`ast`] — `.design/syntax/ast.md`: the boundary AST consumed by
//!   thermite-lower (#4) and forge (#5/#6).
//! - [`parser`] — `.design/syntax/parser.md`: recursive descent with per-item
//!   recovery and mandatory-clause enforcement; owns [`SyntaxError`].
//! - [`address`] — `.design/syntax/semantic-addressing.md`: stable positional
//!   block addresses.
//!
//! Per the scaffold contract (`.design/scaffold/workspace.md` REQ-3), the
//! crate's own error type [`SyntaxError`] lands here with this, the first
//! fallible code in the toolchain. No shared `ThermiteError` is created.
//!
//! The public surface ([`parse`], the AST node types, [`addresses_of`],
//! [`resolve`]) is the boundary API thermite-lower/forge consume; it is
//! exercised by `tests/conformance.rs` against the read-only oracle fixtures
//! under `conformance/` (R-CHAR-3).
//!
//! ## REQ status (scaffold REQs this crate root materializes)
//!
//! <!-- generated:reqs view=thermite-syntax-scaffold-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-SCAFFOLD-SYNTAX-COMPILE | shipped | `thermite-syntax/src/lib.rs` | Syntax clean compile |  |
//! | REQ-SCAFFOLD-SYNTAX-DAG | shipped | `thermite-syntax/src/lib.rs` | Syntax dependency DAG |  |
//! | REQ-SCAFFOLD-SYNTAX-RESULT | shipped | `thermite-syntax/src/lib.rs` | Syntax result discipline |  |
//! | REQ-SCAFFOLD-SYNTAX-WORKSPACE | shipped | `thermite-syntax/src/lib.rs` | Syntax workspace topology |  |
//! <!-- /generated:reqs -->
//!
//! The lexer/parser/AST/addressing component REQs (`.design/syntax/*.md`) carry
//! their own `## REQ status` tables in each module's `//!` doc-comment.

pub mod address;
pub mod ast;
pub mod desugar;
pub mod lexer;
pub mod parser;

pub use address::{addresses_of, resolve, AddrKind, AddressEntry, AddressError};
pub use ast::{
    BinOp, Block, BoundaryAttr, BvTag, BvWidth, Clause, ClauseSelector, Contract, Effect,
    EffectRow, EnumItem, Expr, Falsify, FieldDef, FnItem, ForgeItem, Hole, HoleContext, IndexArg,
    Inhabit, Item, LemmaItem, LoopKind, LoopNode, MatchArm, Param, Pattern, PrimType, Program,
    ProofBlock, ProofItem, ProofObligation, PropFnItem, Quant, Refinement, RefinementTarget,
    SlagAttr, SlicePat, SpecFnItem, Stmt, StructItem, Type, UnaryOp, VariantDef, VariantShape,
    WitnessBlock,
};
pub use lexer::{tokenize, Span, TokKind, Token};
pub use parser::{parse, ParseResult, SyntaxError};
