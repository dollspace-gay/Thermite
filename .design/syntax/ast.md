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
  `If { cond, then: Block, else_: Option<Block> }` (statement form),
  **`Break` and `Continue` (NEW, #93 — the loop-control statements, REQ-12)**,
  or `Expr(Expr)`. The `tail` is the block's trailing value expression
  (`sum`'s final `acc`). Derived from §4.3 + corpus bodies.

- **REQ-5 (loop nodes, addressable):** `Loop { invs: Vec<Expr>, dec: Expr,
  body: Block }` and `While { cond: Expr, invs: Vec<Expr>, dec: Expr, body:
  Block }`. `invs` is non-empty and `dec` is a single `Expr` (structurally
  encoding §4.1's mandatory inv*+one-dec). These are ADDRESSABLE nodes (REQ-8).
  In v0.1 a loop appears as a statement/expression position within a body.
  Derived from §4.1 + corpus `loop`/`while`.

- **REQ-6 (expression nodes):** `Expr` covers exactly:
  `IntLit { value: u128, raw: String }`, `BoolLit(bool)`, `Path(Vec<Ident>)`
  (`u32::MAX`, `Some`, `None`, `lo`), `Call { callee: Box<Expr>, args:
  Vec<Expr> }` (free call `f(args)`), `MethodCall { receiver: Box<Expr>, name:
  Ident, args: Vec<Expr> }` (the one call syntax, `xs.len()`), `Field {
  receiver, name }` (`x.name` with no args), `Closure { params: Vec<Ident>,
  body: Box<Expr> }`, `Match { scrutinee, arms: Vec<MatchArm> }`, `If { cond,
  then, else_ }` (expression form), `Binary { op, lhs, rhs }`, `Unary { op,
  expr }` (NEW, #92 — the prefix `!`), `Index { base, index: IndexArg }`
  (`a[i]` / `a[..i]`), `Cast { expr, ty: Type }` (`e as T`), `Ref { mutable:
  bool, expr }` (`&e` / `&mut e`). Derived from §4.4 + both corpus programs.

  **`IntLit` — value AND verbatim raw (#37 amendment).** `IntLit` is a
  **struct variant** `IntLit { value: u128, raw: String }`:
  - **value** — the numeric value with `_` separators stripped and the radix
    applied (`1000000` for `1_000_000`, `27` for hex `0x1b`, `5` for binary
    `0b101`, `65` for the char literal `'A'`). The original value-only semantics,
    carried from the lexer's `TokKind::Int.value` (`lexer.md` REQ-3/REQ-9) —
    **UNCHANGED in shape**.
  - **raw** — the verbatim source literal, prefix/separators/quotes included
    (`"1_000_000"`, `"0x1b"`, `"0b101"`, `"'A'"`), carried from
    `TokKind::Int.raw`. Round-trip / display fidelity only.

  **CHAR / HEX / BINARY LITERALS REUSE `Expr::IntLit` — NO new Expr variant
  (#91/#92).** A char literal `'A'`, a hex literal `0x1b`, and a binary literal
  `0b101` are ALL `Expr::IntLit { value, raw }` — the value carries the byte /
  radix-applied integer, the raw carries the verbatim spelling. This is a
  DELIBERATE pin: the alternative (a `CharLit` Expr variant) would break every
  exhaustive `match` over `Expr` across the workspace (lower/l1/effects/validator/
  mutation/vacuity/closure/review) AND require a new skill arm. Reusing `IntLit`
  costs ZERO match-arm/skill churn — the literal flows through every existing
  `Expr::IntLit` consumer unchanged. (A char literal is `u8`-typed; the
  validator/lower track that in a char context — see `lexer.md` OQ-2; owned by
  #92, NOT this struct-shape contract.)

  **CRITICAL — lowering & semantics consume `value`, not `raw`.** The lowering
  (`lower`/`lower_l1`/`lower_l2`) emits the numeric `value` (e.g. `1000000`, `27`,
  `5`, `65`), NOT the raw — so a hex/binary/char literal lowers to the SAME Verus
  decimal as the equivalent decimal literal, and the `tests/golden/lower/*` files
  stay byte-stable (no golden churn). Mutation reconstructs `IntLit { value: n±1,
  raw: (n±1).to_string() }` (plain decimal). The `raw` is AST-fidelity only.

- **REQ-7 (pattern + type + effect nodes):** `Pattern` covers `Wildcard`,
  `Literal`, `Binding(Ident)`, `Slice(Vec<SlicePat>)` where `SlicePat` is a
  sub-pattern or `Rest(Ident)` (`..t`), and `Enum { path, fields:
  Vec<Pattern> }` (`Some(i)`, `None`). `Type` covers `Prim(u32|u64|usize|bool)`,
  `Ref { mutable, inner }`, `Slice(inner)` (`&[u32]` is `Ref` of `Slice`), and
  `Generic { name, arg }` (`Option<usize>`). `EffectRow` is `Pure` or
  `Set(Vec<Effect>)`. Note `Pattern::Literal` wraps an `Expr` (e.g.
  `Expr::IntLit`), so a literal pattern carries the same value+raw shape as an
  expression literal — and a char/hex/binary literal pattern is the SAME
  `Expr::IntLit` (no new pattern node). Derived from §4.1, §4.2, §4.4, Appendix A.

- **REQ-8 (addressable nodes carry an address):** The node types that
  `semantic-addressing.md` numbers — `Item` (root = function name), `Loop`/
  `While` (`loop#N`), and the `inv`/`dec` clauses — are addressable. The AST is
  the substrate addresses are computed over. Derived from §4.3.

- **REQ-9 (spans + boundary-type stability):** Every node carries the source
  span of the tokens it was built from (from `lexer.md` REQ-7) for diagnostics.
  The AST is the stable boundary type consumed by thermite-lower (#4) and forge
  (#5/#6) — its shape is a contract; changing a node is a design-doc amendment
  (R-SPEC-3 spirit). Derived from §2 pillar 4 + the authority chain.

- **REQ-10 (binary + unary operator set — NEW, #92):** `BinOp` covers exactly:
  `Add` `Sub` `Mul` `Div` `Rem` (`%`) `Shl` (`<<`) `Shr` (`>>`) `BitAnd` (`&`)
  `BitOr` (`|`) `BitXor` (`^`) `Eq` `Ne` `Lt` `Le` `Gt` `Ge` `And` (`&&`)
  `Or` (`||`). The variants `Rem`/`Shl`/`Shr`/`BitAnd`/`BitOr`/`BitXor` are NEW
  (#92). `UnaryOp` is a NEW enum covering `Not` (the prefix `!` — **bitwise-not
  on an integer type, logical-not on `bool`; the meaning is per the operand
  type, resolved downstream by the validator/lower, NOT by a syntactic
  distinction**: there is ONE `!` token and ONE `UnaryOp::Not`, matching §2.3
  "one way to do everything"). The expression node `Unary { op: UnaryOp, expr:
  Box<Expr> }` carries it.

  **These ARE new exhaustive-match-breaking variants.** Unlike the
  char/hex/binary literals (which reuse `IntLit`), the new `BinOp` variants and
  the new `UnaryOp`/`Unary` node break every exhaustive `match` over `BinOp`/
  `Expr` across the workspace — see Architecture → operator ripple. This is the
  load-bearing cost the builder pays for #92; it is pinned here so the critic
  checks every site. Derived from §4.4 (the arithmetic register; "arithmetic
  overflow is a proof obligation, not a runtime panic" — the same model carries
  to division-by-zero and shift-bounds as PROOF OBLIGATIONS, REQ-11) and §2.3.

- **REQ-11 (partial-operator obligations — handled-or-loud, GROUNDED, #92):**
  `/` (`Div`), `%` (`Rem`), `<<` (`Shl`), and `>>` (`Shr`) are PARTIAL
  operations whose well-definedness is a PROOF OBLIGATION, never UB and never a
  silent runtime trap (§4.4: "arithmetic overflow is a proof obligation, not a
  runtime panic" — the same teeth):
  - **`/` and `%` require a NONZERO divisor.** The lowering emits the bare
    Verus `/`/`%`, and Verus AUTOMATICALLY raises a "possible division by zero"
    obligation at the operator site. The caller discharges it with `req divisor
    != 0` (or a proven-nonzero context, e.g. the literal `2` in `(hi - lo) / 2`).
    A `/`/`%` whose divisor cannot be proven nonzero is L0 (fails verification) —
    the obligation BITES, it is not optional.
  - **`<<` and `>>` require a BOUNDED shift amount** (`< bit width`). Verus
    AUTOMATICALLY raises a "possible bit shift underflow/overflow" obligation;
    the caller discharges it with `req amount < 64` (for `u64`, the relevant
    width). An unbounded shift is L0.

  The AST itself carries no obligation field — the obligation is INHERENT to the
  Verus operator the lowering emits (REQ-10). This REQ pins that the lowering
  MUST emit the partial operators in a position where Verus raises the
  obligation (i.e. NOT wrap them in `#[verifier::external]` or `assume`, which
  would be a proof cheat — `goal.md` R-DEFER-9). GROUNDED (real verus): see
  Verification.

- **REQ-12 (`Break` / `Continue` statement nodes — NEW, #93):** `Stmt` gains two
  variants `Break` and `Continue` (REQ-4) carrying NO payload (labelless, value-
  less — `break;` / `continue;`; Thermite has no loop labels and no `break expr`,
  matching §2.3). They are the AST forms of the loop-control statements lexed in
  `lexer.md` REQ-10 and parsed in `parser.md` REQ-10. They are NOT addressable
  (no `semantic-addressing.md` number — they are body statements, not clauses).

  **These ARE new exhaustive-match-breaking variants.** Like the #92 `BinOp`
  variants (and UNLIKE the char/hex/binary literals which reuse `IntLit`),
  `Stmt::Break` and `Stmt::Continue` break every exhaustive `match` over `Stmt`
  across the workspace — see Architecture → the `Stmt` ripple. This is the
  load-bearing cost #93 pays; it is pinned here so the critic checks every site.

  **The verification semantics live downstream, NOT in the AST.** The AST only
  records "here is a `break`/`continue`". WHETHER a `continue` preserves the loop
  invariant, respects the `decreases`, or sits in a `fx diverge` loop is a
  VERUS-checked property of the LOWERED loop (`verus-lowering.md` #93, REQ-11
  there), not an AST field. The parser additionally enforces that `break`/
  `continue` appear only INSIDE a loop body (`parser.md` REQ-10) — a structural,
  not a verification, rule. Derived from §4.1 (the loop model; termination is
  proved by default), R-DEFER-9 (break/continue must not launder the invariant /
  decreases obligation), and `surface-grammar.md` REQ-11.

## Acceptance criteria

- **AC-1 (corpus AST shapes):** Parsing `conformance/sum.th` yields a `SpecFn`
  `spec_sum` and a `Fn` `sum` whose `Contract` has `req`, two `ens`, `fx = Pure`,
  and a `While` with three `invs` + a `dec`. (REQ-1, REQ-2, REQ-5, REQ-7)
- **AC-1b (`1_000_000` parses to value + raw — #37):** parses to
  `Expr::IntLit { value: 1000000, raw: "1_000_000" }`; lowering still emits
  `1000000` (no golden churn). (REQ-6)
- **AC-1c (char/hex/binary parse to `IntLit` — #91/#92):** `'A'` parses to
  `Expr::IntLit { value: 65, raw: "'A'" }`, `0x1b` to `{ value: 27, raw:
  "0x1b" }`, `0b101` to `{ value: 5, raw: "0b101" }` — all `Expr::IntLit`, NO
  new variant. Lowering emits the decimal `value` (`65`/`27`/`5`). GROUNDED:
  each certifies its `ens result == <decimal>` at L3 (Verification). (REQ-6)
- **AC-2 (mandatory fields non-optional):** `Contract.req: Expr` (not `Option`),
  `ens: Vec<Expr>` non-empty, `fx: EffectRow` (not `Option`). (REQ-2)
- **AC-3 (one call syntax distinction):** `haystack.len()` → `MethodCall`,
  `forall_in(...)` → `Call`, `u32::MAX` → `Path`. (REQ-6)
- **AC-4 (addressable nodes resolve):** the corpus loops + `inv`/`dec` clauses
  number per `semantic-addressing.md`. (REQ-8)
- **AC-5 (operator set is exhaustive — #92):** `BinOp` has the 18 variants
  of REQ-10 (incl. `Rem`/`Shl`/`Shr`/`BitAnd`/`BitOr`/`BitXor`); `UnaryOp` has
  `Not`. A fn `a % b` parses to `Binary { op: BinOp::Rem, .. }`, `a << k` to
  `Shl`, `a & b` to `BitAnd`, `!a` to `Unary { op: UnaryOp::Not, .. }`. (REQ-10)
- **AC-6 (partial-operator obligations bite — GROUNDED, #92):** A fn
  `fn modulo(a,b)->u64 req b != 0 ens result == a % b { a % b }` certifies L3;
  the SAME fn WITHOUT `req b != 0` is L0 ("possible division by zero"). A fn
  `a << k req k < 64` certifies L3; without the bound it is L0 ("possible bit
  shift underflow/overflow"). (REQ-11)
- **AC-7 (`Break`/`Continue` statement nodes — NEW, #93):** A `while` body
  containing `break;` parses to a `Stmt::Break` in the body's `stmts`; `continue;`
  to `Stmt::Continue`. Both carry no payload. A `break;` OUTSIDE any loop body
  is a `SyntaxError` (the structural in-loop rule — `parser.md` REQ-10). (REQ-12)

## Architecture

The AST is a tree of plain Rust enums/structs in `thermite-syntax/src/ast.rs`,
one node family per grammar production. The mandatory-contract rule (§4.1) is
encoded in the types.

**`IntLit` consumer ripple (#37, unchanged by #91/#92).** Every match/construct
site reads `value` where it used the old `u128`; `raw` is ignored except for
verbatim display. Sites: `parser.rs` (construct), `lower.rs`/`l1.rs`/`effects.rs`
(lower/walk), `validator.rs`, `mutation.rs`, `strengthen.rs`, `vacuity.rs`,
`closure.rs`, `review.rs`. Char/hex/binary literals add NOTHING to this ripple —
they ARE `IntLit`, so they flow through the existing arms verbatim.

**Operator ripple (#92 — the load-bearing match-arm cost).** Adding the `BinOp`
variants `Rem`/`Shl`/`Shr`/`BitAnd`/`BitOr`/`BitXor` and the new `UnaryOp`/`Unary`
node breaks every exhaustive `match BinOp` / `match Expr` in the workspace. The
sites the builder MUST extend (non-test production):
- `thermite-syntax/src/parser.rs` — the precedence ladder: a new tier for `%`
  (alongside `*` `/` in `parse_mul`), new tiers for shifts / `&` / `^` / `|`
  (between `+ -` and comparison, per the precedence in REQ-10 / surface-grammar.md),
  and a `!` prefix arm in `parse_ref` (or a sibling) building `Unary`.
- `thermite-lower/src/lower.rs` — `binop` (operator string for each new variant:
  `Rem`→`%`, `Shl`→`<<`, `Shr`→`>>`, `BitAnd`→`&`, `BitOr`→`|`, `BitXor`→`^`) +
  `precedence` (the new tiers) + a `Unary`/`UnaryOp::Not` emit arm (`!`) + the
  `Expr` walk/leaf arms gain `Unary`.
- `thermite-lower/src/l1.rs` — the mirror `binop_str` + `precedence` + the
  `Unary` walk/emit arms (the L1 runtime-check form).
- `forge/src/mutation.rs` — `mutate_op`/`op_str`/`negate` gain arms for the new
  binops (a sound mutant for each, e.g. `Shl`↔`Shr`, `BitAnd`↔`BitOr`) and the
  `Unary` walk arm.
- `forge/src/vacuity.rs` — the `Expr` walk gains a `Unary` leaf-descent arm.
- `thermite-skill/src/generate.rs` — a `SkillFragment` for each new binop AND a
  fragment for `UnaryOp::Not` (the operator vocabulary the skill teaches).
- `thermite-spec/src/validator.rs`, `forge/src/strengthen.rs`,
  `forge/src/closure.rs`, `forge/src/review.rs` — any exhaustive `Expr` match
  gains a `Unary` arm.

**The `Stmt` ripple (#93 — the load-bearing match-arm cost for break/continue).**
Adding `Stmt::Break` and `Stmt::Continue` breaks every exhaustive `match Stmt` in
the workspace (the existing arms are `Let`/`Assign`/`Return`/`If`/`Loop`/`Expr` —
131 `Stmt::` arm references across production today). The sites the builder MUST
extend (non-test production), each adding a `Break`/`Continue` arm (almost always
a no-op / leaf arm — break/continue carry no sub-expression to walk):
- `thermite-syntax/src/parser.rs` — `parse_block`'s statement dispatch gains
  `TokKind::Break`/`Continue` arms building `Stmt::Break`/`Continue` (`parser.md`
  REQ-10), plus the in-loop structural check.
- `thermite-syntax/src/address.rs` — the statement walk (loops/clauses numbered)
  gains leaf `Break`/`Continue` arms (they carry no addressable child).
- `thermite-lower/src/lower.rs` — `lower_stmt`/`lower_loop_body` gain
  `Break`→`break;` / `Continue`→`continue;` emit arms (Verus has NATIVE
  `break`/`continue`; `verus-lowering.md` REQ-12).
- `thermite-lower/src/l1.rs` and `l2.rs` — the mirror statement-walk/emit arms.
- `thermite-lower/src/effects.rs` — the `Stmt` effect-walk gains leaf
  `Break`/`Continue` arms (a loop-control statement contributes NO effect).
- `forge/src/mutation.rs` — the `Stmt` walk gains leaf arms (a `break`/`continue`
  is not a mutation target in v0.1; recorded so the critic confirms no mutant is
  silently dropped — see `verus-lowering.md` OQ-4).
- `forge/src/vacuity.rs`, `forge/src/closure.rs`, `forge/src/review.rs`,
  `forge/src/check.rs` — any exhaustive `Stmt` match gains leaf `Break`/`Continue`
  arms.
- `thermite-spec/src/validator.rs` — the `Stmt` walk gains leaf arms.
- `thermite-skill/src/generate.rs` — a `SkillFragment` teaching `break`/`continue`
  in a loop (the loop-control vocabulary the skill teaches — the skill layer
  ripple).

This is the SAME class of ripple as #92's operator variants — pinned here so the
critic checks every `match Stmt` site for a missing arm (no `_` wildcard / panic
fallthrough — `goal.md` R-APG-1).

**Partial-operator obligations (REQ-11).** The lowering emits the bare Verus
`/`/`%`/`<<`/`>>`; Verus raises the obligation automatically at the operator
site. The lowering MUST NOT suppress it (no `external`/`assume` — R-DEFER-9). No
AST field encodes the obligation; it is a property of the emitted Verus operator.

**Break/continue verification semantics (REQ-12) live in the LOWERING, not the
AST.** The AST is shape-only. The invariant-at-`continue`, `decreases`
interaction, break-exit reasoning, and `fx diverge` cases are all VERUS-checked
properties of the lowered loop, owned + GROUNDED in `verus-lowering.md` (#93). No
AST field carries them.

## Verification

`cargo test -p thermite-syntax` over AST-shape fixtures (`conformance/parse/`):
the two-item shape of `sum.th`/`binary_search.th` (AC-1, AC-3), the `1_000_000`
value+raw assertion (AC-1b), char/hex/binary→`IntLit` assertions (AC-1c),
the non-optional-`Contract` check (AC-2), address resolution (AC-4), operator-
shape assertions that `a % b`/`a << k`/`a & b`/`!a` parse to the right
`BinOp`/`UnaryOp` nodes (AC-5), and NEW `Stmt::Break`/`Continue` shape assertions
(AC-7). The lowering goldens stay UNCHANGED. Expected shapes are hand-derived
(R-CHAR-3).

The END-TO-END operator + obligation grounding (AC-1c, AC-6) is discharged by
`forge`/`thermite-lower` conformance probes lowering each form to Verus and
certifying. GROUNDED with real `verus 0.2026.05.24` (the #92 amendment):

```
% with `req b != 0`, `ens result == a % b`     -> 1 verified, 0 errors  (L3)
% WITHOUT the req                               -> 0 verified, 1 errors  ("possible division by zero", L0)
/ with `req b != 0`, `ens result == a / b`     -> 1 verified, 0 errors  (L3)
/ WITHOUT the req                               -> 0 verified, 1 errors  ("possible division by zero", L0)
<< with `req k < 64`, `ens result == a << k`   -> 1 verified, 0 errors  (L3)
<< WITHOUT the bound                            -> 0 verified, 1 errors  ("possible bit shift underflow/overflow", L0)
>> with `req k < 64`  / & / | / ^ / !u64 / !bool -> 6 verified, 0 errors (L3)
char 'A'==65 / hex 0x1b==27 / bin 0b101==5      -> 3 verified, 0 errors  (L3)
'A'==66 (wrong code)                            -> 0 verified, 1 errors  (non-vacuous, L0)
```

The `ens` clauses are NON-VACUOUS (`result == <expr>`/`result == <code>`), so a
wrong value is rejected — the §7 vacuity gate (which rejects `ens true`) is
respected.

The `break`/`continue` END-TO-END verification semantics (AC-7's downstream
meaning) are owned + GROUNDED in `verus-lowering.md` (#93): a terminating
`continue` that preserves the invariant + decreases certifies L3; a `continue`
that breaks the invariant or fails to decrease the measure is L0; a `break`
early-exit with the loop `ensures` certifies L3; a `fx diverge` loop with
`break`/`continue` certifies (no decreases, capped at L1 by the #88 gate).

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (item nodes) | SHIPPED | `enum Item { Fn, SpecFn, Struct, Enum }` in `ast.rs`; built by `parse_item` in `parser.rs`, asserted by `tests/conformance.rs`. |
| REQ-2 (contract node, mandatory fields) | SHIPPED | `struct Contract { req: Clause, ens: Vec<Clause>, fx: EffectRow }` — non-`Option`; built only in `parse_contract`. |
| REQ-3 (slag attribute node) | SHIPPED | `struct SlagAttr` + `FnItem.slag: Option<SlagAttr>`; parsed by `parse_slag`. |
| REQ-4 (block + statement nodes) | SHIPPED | `struct Block`, `enum Stmt` in `ast.rs` (`Let`/`Assign`/`Return`/`If`/`Loop`/`Expr`); built by `parse_block`/`parse_stmt`. The `Break`/`Continue` additions are REQ-12 (#93, NOT-STARTED). |
| REQ-5 (loop nodes, addressable) | SHIPPED | `struct LoopNode { kind, invs, dec, .. }`; addressed by `address.rs`. |
| REQ-6 — VALUE (`IntLit` value) | SHIPPED | `enum Expr` with `IntLit { value, .. }` in `ast.rs`; built by `parse_primary`; lowered by `IntLit { value, .. } => value.to_string()` in `lower.rs`/`l1.rs`. |
| REQ-6 — RAW (`IntLit` verbatim raw, #37) | SHIPPED | `Expr::IntLit { value: u128, raw: String }` in `ast.rs`; built from `TokKind::Int { value, raw }`; test `int_literal_preserves_value_and_raw`. |
| REQ-6 — CHAR/HEX/BIN reuse `IntLit` (#91/#92) | SHIPPED | `'A'`/`0x1b`/`0b101` lex into `TokKind::Int { value, raw }` (`lexer.rs` `lex_char`/`lex_int`, REQ-3/REQ-9) and `parse_primary` in `parser.rs` builds them into `Expr::IntLit { value, raw }` — NO new Expr variant, ZERO match-arm churn (test `char_hex_binary_parse_to_intlit_no_new_variant` in `thermite-syntax/tests/operators_parse.rs`). Lowering emits the decimal `value` (`lower.rs`/`l1.rs` `Expr::IntLit { value, .. }`). GROUNDED L3 (`forge/tests/operators_conformance.rs`). |
| REQ-7 (pattern/type/effect nodes) | SHIPPED | `enum Pattern`/`enum Type`/`enum EffectRow` in `ast.rs`; built by `parse_pattern`/`parse_type`. |
| REQ-8 (addressable nodes) | SHIPPED | `Item`/`LoopNode`/`Clause` keep source order; numbered by `address.rs`. |
| REQ-9 (spans + boundary stability) | SHIPPED | `Span` on `FnItem`/`SpecFnItem`/`LoopNode`/`SlagAttr`/`Clause`. |
| REQ-10 (binary + unary operator set, #92) | SHIPPED | `enum BinOp` in `ast.rs` gains `Rem`/`Shl`/`Shr`/`BitAnd`/`BitOr`/`BitXor`; the NEW `enum UnaryOp { Not }` + `Expr::Unary { op, expr }` node carry the prefix `!`. Built by the `parser.rs` precedence ladder (`parse_mul`+`%`, `parse_shift`/`parse_bitand`/`parse_bitxor`/`parse_bitor`, `parse_unary`); the match-arm ripple is closed across lower/l1/effects/validator/mutation/vacuity/closure/review/check/strengthen/skill (no `_`/panic — see commit). Tests `each_new_operator_parses_to_its_binop_node` (parser). GROUNDED L3 for all 7 forms (`forge/tests/operators_conformance.rs`). |
| REQ-11 (partial-operator obligations, #92) | SHIPPED | `binop` in `lower.rs`/`l1.rs` emits the BARE Verus `%`/`<<`/`>>` (no `external`/`assume` — R-DEFER-9), so Verus raises the div-by-zero / shift-bound obligation at the operator site. GROUNDED (real verus): `a % b` WITH `req b != 0` → L3, WITHOUT → L0; `a << k` WITH `req k < 64` → L3, unbounded → L0 (`forge/tests/operators_conformance.rs::rem_with_nonzero_req_certifies_l3` / `rem_without_nonzero_req_is_l0` / `shifts_and_bitwise_certify_l3` / `shift_without_bound_is_l0`). The existing `/` already bit; `%`/shifts inherit the same Verus-native obligation. |
| REQ-12 (`Break`/`Continue` statement nodes, #93) | NOT-STARTED | open prereq blocker #93. `enum Stmt` in `ast.rs` currently has NO `Break`/`Continue` variant (the arms are `Let`/`Assign`/`Return`/`If`/`Loop`/`Expr`), so a `break;` parses as a `Stmt::Expr(Expr::Path(["break"]))` once `break` lexes as an identifier — a silent no-op expr (the editor's quit-flag workaround). #93 must add `Stmt::Break`/`Stmt::Continue` (two new exhaustive-match-breaking variants — the `Stmt` ripple across lower/l1/l2/effects/address/validator/mutation/vacuity/closure/review/check/skill), the parser arms (`parser.md` REQ-10), and the Verus-native lowering + verification semantics (`verus-lowering.md` REQ-12, GROUNDED there). |

## Open questions (for the orchestrator)

- **OQ-1 (loop as statement vs expression):** unchanged; v0.1 loops are
  statement-position. Not a blocker.
- **OQ-2 (address stored vs computed):** unchanged; builder's choice. Not a
  blocker.
- **OQ-3 (`IntLit` raw redundancy with span, #37):** unchanged; the `{ value,
  raw }` shape is the contract. Not a blocker.
- **OQ-4 (`!` meaning is per-type, #92):** REQ-10 pins ONE `UnaryOp::Not` token
  whose meaning (bitwise-not on integers, logical-not on `bool`) is resolved by
  the operand type DOWNSTREAM (validator/lower), not by a syntactic split — the
  one-way-to-do-everything rule (§2.3). GROUNDED: `!a` on `u64` and `!a` on
  `bool` both certify under Verus's type-directed `!`. The validator must reject
  `!` on a non-integer / non-bool operand (e.g. `&[u32]`) — flagged for the
  builder, owned by #92. Not a blocker for the AST shape.
- **OQ-5 (`Break`/`Continue` payload-less, #93):** REQ-12 pins both as
  payload-less `Stmt` variants (no loop label, no `break expr`) — §2.3 "one way
  to do everything". A future labelled-break would be a NEW design amendment, not
  a v0.1 concern. Recorded; not a blocker for the AST shape.
