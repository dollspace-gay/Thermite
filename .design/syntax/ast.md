# Thermite AST (node shapes)
<!--
tier: 3-component
status: draft
governs: thermite-syntax/src/ast.rs
thesis-refs:
  - thermite-design.md §4.1
  - thermite-design.md §4.2
  - thermite-design.md §4.3
  - thermite-design.md §4.4
  - thermite-design.md §8
  - thermite-design.md Appendix A
-->

## Summary

The AST is the structured output of the parser (`parser.md`) and the **boundary
type** consumed downstream by thermite-lower (issue #4, AST → Verus source) and
forge (issues #5/#6, the ladder + vacuity battery). Its node set mirrors the
surface grammar (`surface-grammar.md`) one-for-one. Certain nodes are
**addressable** — they carry a stable semantic address (`semantic-addressing.md`,
e.g. `binary_search.loop#1.inv#2`) so `forge edit`/`forge insert-after` and the
per-item proof cache key off structure, not string matches (§4.3).

This doc is GREENFIELD / FORWARD-LOOKING: no `ast.rs` exists. Every REQ is
**NOT-STARTED**, blocked on issue #3.

## Requirements

- **REQ-1 (item nodes):** An `Item` is one of `Fn` (with optional `#[slag]`
  attribute), or `SpecFn`. `Fn` holds: name, params, return type, the contract
  (`Contract`), and a body `Block`. `SpecFn` holds: name, params, return type,
  the `dec` measure expression, and a body `Block` (no `Contract`; spec fns
  carry only `dec`). Derived from §4.1, §4.2 (Appendix A `spec_sum`), §8.

- **REQ-2 (contract node, mandatory fields):** `Contract` is a struct with a
  `req: Expr`, a non-empty `ens: Vec<Expr>` (one-or-more), and an `fx: EffectRow`
  — all **non-optional fields** in the type (the parser cannot construct a `Fn`
  without them; absence is a parse error per §4.1, not an `Option`). The AST type
  thus structurally encodes the mandatory-contract rule. Derived from §4.1
  ("Mandatory keyword … absence is always a parse error").

- **REQ-3 (slag attribute node):** A `Fn` may carry `slag: Option<SlagAttr>`
  where `SlagAttr { reason, owner, review }` holds the three field strings. The
  AST stores the parsed fields verbatim; required-field-presence and
  non-emptiness checks are downstream (§8 / forge). A non-slag `fn` has
  `slag = None`. Derived from §8.

- **REQ-4 (block + statement nodes):** `Block { stmts: Vec<Stmt>, tail:
  Option<Expr> }`. `Stmt` is one of `Let { mutable: bool, name, ty: Type, init:
  Expr }`, `Assign { target: Expr, value: Expr }`, `Return(Option<Expr>)`,
  `If { cond, then: Block, else_: Option<Block> }` (statement form), or
  `Expr(Expr)`. The `tail` is the block's trailing value expression
  (`sum`'s final `acc`). Derived from §4.3 + corpus bodies.

- **REQ-5 (loop nodes, addressable):** `Loop { invs: Vec<Expr>, dec: Expr,
  body: Block }` and `While { cond: Expr, invs: Vec<Expr>, dec: Expr, body:
  Block }`. `invs` is non-empty and `dec` is a single `Expr` (structurally
  encoding §4.1's mandatory inv*+one-dec). These are ADDRESSABLE nodes (REQ-8).
  In v0.1 a loop appears as a statement/expression position within a body.
  Derived from §4.1 + corpus `loop`/`while`.

- **REQ-6 (expression nodes):** `Expr` covers exactly: `IntLit(u128)` (value
  with `_` already stripped), `BoolLit(bool)`, `Path(Vec<Ident>)` (`u32::MAX`,
  `Some`, `None`, `lo`), `Call { callee: Box<Expr>, args: Vec<Expr> }` (free
  call `f(args)`), `MethodCall { receiver: Box<Expr>, name: Ident, args:
  Vec<Expr> }` (the one call syntax, `xs.len()`), `Field { receiver, name }`
  (`x.name` with no args), `Closure { params: Vec<Ident>, body: Box<Expr> }`,
  `Match { scrutinee, arms: Vec<MatchArm> }`, `If { cond, then, else_ }`
  (expression form), `Binary { op, lhs, rhs }`, `Index { base, index: IndexArg }`
  (`a[i]` / `a[..i]`), `Cast { expr, ty: Type }` (`e as T`), `Ref { mutable:
  bool, expr }` (`&e` / `&mut e`). Derived from §4.4 + both corpus programs.

- **REQ-7 (pattern + type + effect nodes):** `Pattern` covers `Wildcard`,
  `Literal`, `Binding(Ident)`, `Slice(Vec<SlicePat>)` where `SlicePat` is a
  sub-pattern or `Rest(Ident)` (`..t`), and `Enum { path, fields:
  Vec<Pattern> }` (`Some(i)`, `None`). `Type` covers `Prim(u32|u64|usize|bool)`,
  `Ref { mutable, inner }`, `Slice(inner)` (`&[u32]` is `Ref` of `Slice`), and
  `Generic { name, arg }` (`Option<usize>`). `EffectRow` is `Pure` or
  `Set(Vec<Effect>)`. Derived from §4.1, §4.2, §4.4, Appendix A.

- **REQ-8 (addressable nodes carry an address):** The node types that
  `semantic-addressing.md` numbers — `Item` (root = function name), `Loop`/
  `While` (`loop#N`), and the `inv`/`dec` clauses — are addressable: either they
  carry an `Address`/positional index field, or the address is derivable from
  their position in the parent (the structural-numbering scheme in
  `semantic-addressing.md`). The AST is the substrate addresses are computed
  over; `ast.md` defines WHICH nodes are addressable, `semantic-addressing.md`
  defines the EXACT numbering. Derived from §4.3.

- **REQ-9 (spans + boundary-type stability):** Every node carries the source
  span of the tokens it was built from (from `lexer.md` REQ-7) for diagnostics.
  The AST is the stable boundary type consumed by thermite-lower (#4) and forge
  (#5/#6) — its shape is a contract; changing a node is a design-doc amendment
  (R-SPEC-3 spirit). Derived from §2 pillar 4 + the authority chain (`goal.md`:
  AST is what thermite-lower lowers).

## Acceptance criteria

- **AC-1 (corpus AST shapes):** Parsing `conformance/sum.th` yields two `Item`s:
  a `SpecFn` named `spec_sum` (with a `dec` and a `Match` body whose two arms are
  a `Slice([])` pattern and a `Slice([Binding(head), Rest(t)])` pattern) and a
  `Fn` named `sum` whose `Contract` has `req`, two `ens`, and `fx = Pure`, and
  whose body contains a `While` with three `invs` and a `dec`. Tied to
  `conformance/parse/` AST-shape fixtures. (REQ-1, REQ-2, REQ-5, REQ-7)
- **AC-2 (mandatory fields are non-optional):** The `Contract` type has
  `req: Expr` (not `Option`), `ens: Vec<Expr>` with a non-empty invariant
  enforced at construction, and `fx: EffectRow` (not `Option`) — a `Fn` cannot
  be constructed without them. (REQ-2)
- **AC-3 (one call syntax distinction):** In `binary_search.th`,
  `haystack.len()` parses to a `MethodCall { name: "len", args: [] }`,
  `forall_in(haystack, |x| ...)` to a `Call`, and `u32::MAX` (in `sum.th`) to a
  `Path(["u32","MAX"])` — never a `MethodCall`. (REQ-6)
- **AC-4 (addressable nodes resolve):** The `While` in `sum` and the `Loop` in
  `binary_search` are addressable, and their `inv`/`dec` clauses are numbered per
  `semantic-addressing.md` (`sum.loop#1.inv#1..#3`, `binary_search.loop#1.inv#2`
  resolving to `forall_from(haystack, hi, |x| x > needle)`). Tied to
  `conformance/address` fixtures. (REQ-8)

## Architecture

The AST is a tree of plain Rust enums/structs in `thermite-syntax/src/ast.rs`,
one node family per grammar production (`surface-grammar.md`). The
mandatory-contract rule (§4.1) is encoded in the **types**: `Contract.req` and
`Contract.fx` are non-`Option`, `Contract.ens` is a `Vec` with a non-empty
construction invariant, and `Loop`/`While` carry a non-empty `invs` and a single
`dec` — so an ill-formed contract is unrepresentable, and the parser surfaces
the absence as a `SyntaxError` before ever building the node.

`Expr` distinguishes `Call` (free `f(args)`), `MethodCall` (postfix `recv.m(...)`),
and `Field` (postfix `recv.m`) to encode the **one call syntax** decision
(`surface-grammar.md` REQ-6): `xs.len()` is a `MethodCall`, `spec_sum(t)` is a
`Call`, `u32::MAX` is a `Path`. The frontend remains REGISTRY-FREE: combinator
calls (`forall_in`, `sorted`) are ordinary `Call` nodes — the AST does not mark
them special; the fixed-combinator-set rule is a downstream semantic check
(§4.2; `goal.md`).

Addressability (REQ-8) is the bridge to `semantic-addressing.md`: `Item`, `Loop`/
`While`, and their `inv`/`dec` clauses are the addressed nodes. Whether the
address is a stored field or computed on demand from structural position is an
implementation choice the builder makes; either way it must be the deterministic,
edit-stable scheme `semantic-addressing.md` pins. The AST is the consumed
boundary type for thermite-lower (#4) and forge (#5/#6) — every span and node is
chosen so lowering can map `req`→`requires`, `ens`→`ensures`, `inv`/`dec`→loop
clauses, and `spec fn`→a Verus spec function.

## Verification

`cargo test -p thermite-syntax` over AST-shape fixtures derived from the corpus
(`conformance/parse/`): structural assertions on the two-item shape of `sum.th`
and `binary_search.th` (AC-1, AC-3), a compile-time / construction check that
`Contract` fields are non-optional (AC-2), and address-resolution fixtures
(`conformance/address`, AC-4). Expected shapes are hand-derived from the grammar
+ corpus, never copied from the parser's output (R-CHAR-3).

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (item nodes) | SHIPPED | `enum Item { Fn(FnItem), SpecFn(SpecFnItem) }` in `ast.rs`; built by `parse_item`, asserted by `tests/conformance.rs` (sum/binary_search facts). |
| REQ-2 (contract node, mandatory fields) | SHIPPED | `struct Contract { req: Clause, ens: Vec<Clause>, fx: EffectRow }` — non-`Option`; built only in `parse_contract` after presence checks. |
| REQ-3 (slag attribute node) | SHIPPED | `struct SlagAttr` + `FnItem.slag: Option<SlagAttr>`; parsed by `parse_slag`. |
| REQ-4 (block + statement nodes) | SHIPPED | `struct Block { stmts, tail }`, `enum Stmt`; built by `parse_block`/`parse_stmt`. |
| REQ-5 (loop nodes, addressable) | SHIPPED | `struct LoopNode { kind, invs, dec, .. }` (non-empty `invs`, single `dec`); addressed by `address.rs`. |
| REQ-6 (expression nodes) | SHIPPED | `enum Expr` with `Call`/`MethodCall`/`Field`/`Path`/`Closure`/`Match`/`Binary`/`Index`/`Cast`/`Ref`; built by the precedence ladder. |
| REQ-7 (pattern/type/effect nodes) | SHIPPED | `enum Pattern`/`enum Type`/`enum EffectRow`; `Slice`/`Enum` patterns + `&[u32]`/`Option<usize>` types verified by parse facts. |
| REQ-8 (addressable nodes) | SHIPPED | `Item`/`LoopNode`/`Clause` retain source order; numbered by `address.rs` (address oracle passes). |
| REQ-9 (spans + boundary stability) | SHIPPED | `Span` on `FnItem`/`SpecFnItem`/`LoopNode`/`SlagAttr`/`Clause`; clauses also keep verbatim `text` for addressing. |

## Open questions (for the orchestrator)

- **OQ-1 (loop as statement vs expression):** `binary_search`'s `loop` is in
  statement position (it can `return` out); `sum`'s `while` is also a statement.
  In v0.1 the corpus never uses a loop's value, so REQ-5 models `Loop`/`While`
  as statement-position nodes. If a future need arises for `let x = loop {...}`,
  the node moves into `Expr`. Recorded as a v0.1 restriction; not a blocker.
- **OQ-2 (address stored vs computed):** REQ-8 leaves "address field vs computed
  from position" to the builder, requiring only that the result match
  `semantic-addressing.md`. Flagged so the critic checks the *scheme*, not the
  representation. Not a blocker.
