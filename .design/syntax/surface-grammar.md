# Thermite Surface Grammar (the canonical anchor)
<!--
tier: 3-component
status: draft
governs: thermite-syntax (the whole surface grammar; the parser is its executable form)
thesis-refs:
  - thermite-design.md §4.1
  - thermite-design.md §4.2
  - thermite-design.md §4.3
  - thermite-design.md §4.4
  - thermite-design.md §2 (pillar 3, "one way to do everything")
  - thermite-design.md §8
  - thermite-design.md Appendix A
-->

## Summary

This is the canonical EBNF for the Thermite v0.1 surface language: exactly the
constructs in the two conformance programs (`conformance/sum.th`,
`conformance/binary_search.th`) and §4, plus the primitive-completeness additions
(#91/#92: char/hex/binary literals and the integer operators `% << >> & | ^ !`),
and *no more* (pillar 3, "one way to do everything", §2). It is the shared anchor
that `lexer.md`, `ast.md`, `parser.md`, and `semantic-addressing.md` all
reference. The parser is the executable form of this grammar.

This doc is GREENFIELD / FORWARD-LOOKING: no parser code exists. Every REQ is
**NOT-STARTED**, blocked on issue #3.

## Scope boundary (grammar vs. semantics)

The grammar enforces **clause PRESENCE and structure only**. It does NOT enforce:

- that `ens` mentions `result` (a §7 structural-vacuity check; forge, issue #6),
- that combinators come from the fixed SpecTherm set (a §4.2 *semantic* check),
- that `req`/`inv` are non-vacuous, that types resolve, that effects subsume,
- **the partiality obligations of `/`/`%`/`<<`/`>>`** (`ast.md` REQ-11): the
  grammar parses `a / b` regardless; the divide-by-zero / shift-bound obligation
  is a §7 PROOF obligation discharged at verification, not a parse rule.

The grammar's job: a `fn` without all three of `req`/`ens`/`fx`, or a `loop`/
`while` missing `inv`/`dec`, is a **parse error** (§4.1). A combinator call like
`forall_in(haystack, |x| ...)` parses as a generic call expression.

## Requirements

- **REQ-1 (item grammar):** Exactly three top-level item forms — `fn`,
  `spec fn`, and `#[slag(...)] fn` (plus the basis-stage `struct`/`enum`) — and
  nothing else (no `impl`, `trait`, `use`, `mod`, `macro` in v0.1). Derived from
  §4.1, §4.2, §8, and the corpus.

- **REQ-2 (mandatory contract clauses, fixed order):** A `fn` signature is
  `fn NAME ( PARAMS ) -> TYPE` followed by, in this exact order, `req EXPR`,
  one-or-more `ens EXPR`, then exactly one `fx EFFECTROW`. A `spec fn` carries
  exactly one `dec EXPR`. Absence of a required clause is a parse error. Derived
  from §4.1, §4.2, and the corpus verbatim.

- **REQ-3 (loop/while with mandatory inv* + exactly one dec):** Both `loop { }`
  and `while EXPR { }` carry one-or-more `inv EXPR` clauses then exactly one
  `dec EXPR`, then the body. Missing `inv` or missing/duplicate `dec` is a parse
  error. Derived from §4.1 and the corpus loops.

- **REQ-4 (statement grammar):** A block `{ }` is a sequence of statements with
  an optional trailing tail expression. Statements: `let mut? NAME : TYPE = EXPR
  ;`, assignment `LVALUE = EXPR ;`, `return EXPR? ;`, the `if`/`else` statement,
  and `EXPR ;`. Derived from §4.3 and the corpus bodies.

- **REQ-5 (expression grammar):** Expressions cover exactly: integer literals
  (decimal `1_000_000`, **hexadecimal `0x1b`, binary `0b101`** — #92; all the
  SAME integer value as the equivalent decimal), **char literals `'A'`** (#91/#92;
  a byte-valued integer literal, `'A'` == 65), `bool` literals, paths (`lo`,
  `u32::MAX`, `Some`, `None`), call `f(args)`, the ONE call syntax for member
  access (REQ-6), closure `|params| EXPR`, `match EXPR { ARMS }`, `if EXPR { }
  else { }` as an expression, **binary arithmetic `+ - * /`, modulo `%`, shifts
  `<< >>`, bitwise `& | ^`** (#92), comparison (`== != < <= > >=`), logical
  (`&& ||`), **the unary prefix `!`** (#92 — bitwise-not on integers, logical-not
  on `bool`; one operator, meaning per type), indexing `a[i]` / `a[..i]`, cast
  `EXPR as TYPE`, references `&EXPR` / `&mut EXPR`, and parenthesized grouping.
  Derived from §4.4 and both corpus programs.

  > **AMENDMENT (#91/#92, supersedes the prior expr-grammar wording).** The prior
  > REQ-5 admitted only `+ - * /` arithmetic, `== != < <= > >=`, and `&& ||` and
  > only DECIMAL integer literals. It now ADDS: hex/binary integer literals and
  > char literals (all lexing into the SAME integer-literal form — `lexer.md`
  > REQ-3/REQ-9, `ast.md` REQ-6, NO new Expr variant), the binary operators
  > `% << >> & | ^`, and the unary prefix `!`. These complete "compose any
  > program" for integer/bit work (#91/#92). The precedence is pinned below.

- **REQ-6 (one call syntax — DECISION):** Member/associated access uses one call
  syntax: postfix `.` selects a field or a method call. `xs.len()` is a method
  call; `spec_sum(t)`/`forall_in(...)` are free calls; `u32::MAX` is a path
  (never method-dispatch sugar). No UFCS. Derived from §4.4.

- **REQ-7 (pattern grammar):** Patterns cover literal patterns (including a
  char/hex/binary literal, which is the SAME integer-literal pattern — `ast.md`
  REQ-7), binding patterns, wildcard `_`, slice patterns `[]`/`[head, ..t]`, and
  enum/tuple-struct patterns `Some(i)`/`None`. Derived from §4.1 + Appendix A.

- **REQ-8 (type grammar):** Types cover primitive names (`u32`, `u64`, `usize`,
  `bool`), `&[T]`, `&T`/`&mut T`, and one generic application `NAME<T>`. No
  lifetimes (§4.4). A char literal is `u8`-typed (`lexer.md` REQ-9 / OQ-2); `u8`
  is therefore an accepted primitive in a char/byte context. Derived from the
  corpus signatures and §4.4.

- **REQ-9 (effect-row grammar):** An `fx` row is `pure` or a set drawn from
  `{read(path), write(path), net(domain), alloc, time, rand, panic, diverge}`.
  Derived from §4.1.

- **REQ-10 (operator precedence — PINNED, #92):** The binary-operator precedence
  is the standard Rust precedence (tightest → loosest):

  | Tier | Operators | Associativity |
  |---|---|---|
  | 1 (tightest) | `*` `/` `%` | left |
  | 2 | `+` `-` | left |
  | 3 | `<<` `>>` | left |
  | 4 | `&` | left |
  | 5 | `^` | left |
  | 6 | `\|` | left |
  | 7 | `==` `!=` `<` `<=` `>` `>=` | **non-associative** (≤ 1, unchanged) |
  | 8 | `&&` | left |
  | 9 (loosest) | `\|\|` | left |

  The unary prefix `!` binds TIGHTER than every binary operator (it is a prefix
  operator at the `RefExpr` tier, alongside `&`/`&mut`/`*`). So `!a & b` parses
  as `(!a) & b`, and `a % b + 1` parses as `(a % b) + 1` (GROUNDED: this group
  certifies under verus). This is the SINGLE canonical precedence; there are no
  precedence knobs (§2.3). Comparison stays non-associative (`a < b < c` is a
  parse error — unchanged). Derived from §4.4 (the Rust-dialect register: "reads
  like a boring, regular Rust dialect") and §2.3.

## Acceptance criteria

- **AC-1 (both corpus programs accept):** The grammar accepts `sum.th` and
  `binary_search.th` in full. (REQ-1..REQ-9)
- **AC-2 (missing clause rejects):** Removing `req`/`ens`/`fx`/`inv`/`dec`
  yields a parse error. (REQ-2, REQ-3)
- **AC-3 (no extra constructs):** No production for `struct`-as-Rust-generics,
  `impl`, `trait`, `use`, `mod`, macro, `for`, `unsafe`, an explicit lifetime
  token, or a UFCS method call. (REQ-1, REQ-6, REQ-8)
- **AC-4 (one call syntax):** `xs.len()` → method call, `spec_sum(t)` → free
  call, `u32::MAX` → path. (REQ-6)
- **AC-5 (new literals parse — NEW, #91/#92):** `0x1b`, `0b101`, and `'A'` each
  parse as an integer-literal expression (the same node as a decimal literal);
  `0xFF_FF` parses (interior `_`); `''`, `0x` with no digit, and a non-ASCII
  `'é'` are parse/lex errors. (REQ-5)
- **AC-6 (new operators parse with the pinned precedence — NEW, #92):** `a % b`,
  `a << k`, `a >> k`, `a & b`, `a | b`, `a ^ b`, `!a` each parse to the expected
  `Binary`/`Unary` node; `a % b + 1` groups as `(a % b) + 1`; `!a & b` as
  `(!a) & b`; `a + b << c` as `(a + b) << c` (shifts below `+ -`). (REQ-5,
  REQ-10)
- **AC-7 (partiality is a §7 obligation, not a parse rule — GROUNDED):** `a / b`
  / `a % b` parse regardless of whether `b` can be zero; the divide-by-zero
  obligation is discharged (or fails) at verification. GROUNDED: `a % b` with
  `req b != 0` certifies L3; without it, L0. (Scope boundary; `ast.md` REQ-11.)

## Architecture

EBNF (informative; `parser.md` is the operational contract). Symbol anchors,
never line numbers (R-CITE-2b).

```ebnf
Program     ::= Item*

Item        ::= Attr? FnItem | SpecFnItem
Attr        ::= '#[' 'slag' '(' SlagField (',' SlagField)* ')' ']'   ; §8
SlagField   ::= ('reason' | 'owner' | 'review') '=' StringLit

FnItem      ::= 'fn' Ident '(' Params? ')' RetType ReqClause EnsClause+ FxClause Block
SpecFnItem  ::= 'spec' 'fn' Ident '(' Params? ')' RetType DecClause Block

Params      ::= Param (',' Param)*
Param       ::= Ident ':' Type
RetType     ::= '->' Type

ReqClause   ::= 'req' Expr                          ; mandatory (§4.1)
EnsClause   ::= 'ens' Expr                          ; >=1, mandatory (§4.1)
FxClause    ::= 'fx' EffectRow                      ; mandatory (§4.1)
DecClause   ::= 'dec' Expr                          ; mandatory on spec fn + loops

EffectRow   ::= 'pure' | Effect (',' Effect)*
Effect      ::= 'read' '(' PathArg ')' | 'write' '(' PathArg ')'
              | 'net' '(' PathArg ')' | 'alloc' | 'time' | 'rand'
              | 'panic' | 'diverge'

Block       ::= '{' Stmt* TailExpr? '}'
TailExpr    ::= Expr
Stmt        ::= LetStmt | AssignStmt | ReturnStmt | IfStmt | ExprStmt
LetStmt     ::= 'let' 'mut'? Ident ':' Type '=' Expr ';'
AssignStmt  ::= LValue '=' Expr ';'
ReturnStmt  ::= 'return' Expr? ';'
IfStmt      ::= 'if' Expr Block ('else' Block)?
ExprStmt    ::= Expr ';'
LValue      ::= Ident | IndexExpr | FieldExpr

LoopExpr    ::= 'loop'  InvClause+ DecClause Block
WhileExpr   ::= 'while' Expr InvClause+ DecClause Block
InvClause   ::= 'inv' Expr

; Expression precedence (loosest -> tightest), the SINGLE canonical ladder
; (§2.3). #92 inserts the modulo/shift/bitwise tiers between '+ -' and
; comparison, matching standard Rust precedence (surface-grammar.md REQ-10).
Expr        ::= OrExpr
OrExpr      ::= AndExpr ('||' AndExpr)*
AndExpr     ::= CmpExpr ('&&' CmpExpr)*
CmpExpr     ::= BitOrExpr (CmpOp BitOrExpr)?        ; non-associative comparison
CmpOp       ::= '==' | '!=' | '<' | '<=' | '>' | '>='
BitOrExpr   ::= BitXorExpr ('|' BitXorExpr)*        ; #92
BitXorExpr  ::= BitAndExpr ('^' BitAndExpr)*        ; #92
BitAndExpr  ::= ShiftExpr ('&' ShiftExpr)*          ; #92
ShiftExpr   ::= AddExpr (('<<' | '>>') AddExpr)*    ; #92
AddExpr     ::= MulExpr (('+' | '-') MulExpr)*
MulExpr     ::= CastExpr (('*' | '/' | '%') CastExpr)*   ; '%' is #92
CastExpr    ::= UnaryExpr ('as' Type)*
UnaryExpr   ::= '!' UnaryExpr | RefExpr             ; '!' prefix is #92
RefExpr     ::= '&' 'mut'? RefExpr | '*' RefExpr | Postfix
Postfix     ::= Primary PostfixOp*
PostfixOp   ::= '.' Ident ('(' Args? ')')?          ; field OR method — ONE call syntax
              | '[' IndexArg ']'
              | '(' Args? ')'
Primary     ::= Literal | Path | Closure | MatchExpr | IfExpr | '(' Expr ')'

Closure     ::= '|' ClosureParams? '|' Expr
ClosureParams ::= Ident (',' Ident)*
MatchExpr   ::= 'match' Expr '{' MatchArm (',' MatchArm)* ','? '}'
MatchArm    ::= Pattern '=>' Expr
IfExpr      ::= 'if' Expr Block 'else' Block

Path        ::= Ident ('::' Ident)*
IndexArg    ::= Expr | '..' Expr | Expr '..' Expr | Expr '..'
Args        ::= Expr (',' Expr)*

Pattern     ::= '_' | Literal | Ident
              | '[' (SlicePat (',' SlicePat)*)? ']'
              | Path ('(' Pattern (',' Pattern)* ')')?
SlicePat    ::= Pattern | '..' Ident

Type        ::= 'u32' | 'u64' | 'usize' | 'bool'
              | '&' 'mut'? Type
              | '&' '[' Type ']'
              | Ident '<' Type '>'

; #92: a single IntLit terminal covers decimal/hex/binary AND the char form —
; one integer-literal node (lexer.md REQ-3/REQ-9, ast.md REQ-6).
Literal     ::= IntLit | BoolLit
IntLit      ::= DecLit | HexLit | BinLit | CharLit
DecLit      ::= Digit ('_'? Digit)*                  ; 1_000_000
HexLit      ::= ('0x' | '0X') HexDigit ('_'? HexDigit)*   ; 0x1b, 0xFF_FF  (== decimal value)
BinLit      ::= ('0b' | '0B') BinDigit ('_'? BinDigit)*   ; 0b101          (== decimal value)
CharLit     ::= "'" (AsciiChar | Escape) "'"         ; 'A' == 65 (byte value); ASCII only in v1
```

**Key design decisions (one-way-to-do-everything, §2.3):**

1. **One call syntax (REQ-6).** Unchanged.
2. **`if` is both a statement and an expression.** Unchanged.
3. **Comparison is non-associative.** Unchanged (`a < b < c` is a parse error).
4. **`()` return type written explicitly.** Unchanged.
5. **One integer-literal node for all radices AND char (#91/#92).** Decimal,
   hex, binary, and char literals are ONE `IntLit` node carrying the integer
   value — the radix/char spelling is surface-only, never a distinct type or
   downstream node (no Expr-variant break). A char is `u8`-typed (`'A'` == 65).
6. **Standard Rust operator precedence, single canonical ladder (REQ-10, #92).**
   No precedence config; `*` `/` `%` tightest among binaries, then `+` `-`, then
   shifts, then `&`, `^`, `|`, then comparison, then `&&`, then `||`. Prefix `!`
   binds tighter than all binaries.
7. **Partial operators are §7 obligations, not parse rules.** `a / b` / `a % b`
   / `a << k` parse unconditionally; divide-by-zero / shift-bound is proven (or
   fails) at verification (`ast.md` REQ-11; scope boundary above).

## Verification

The grammar is verified through `parser.md`'s oracle, `conformance/parse/`:
round-trip / AST-shape fixtures (AC-1, AC-4), missing-clause negatives (AC-2),
removed-construct negatives (AC-3), and NEW fixtures for the literal forms
(AC-5) and the operator precedence (AC-6). The partiality / value semantics
(AC-7, and the radix/char value equalities) are GROUNDED end-to-end through
`forge`/`thermite-lower` certifying real Verus (see `ast.md` Verification — the
`% / << >> & | ^ !` and `'A'`/`0x1b`/`0b101` probes, all certifying L3 with
non-vacuous `ens`, the partials failing L0 without their obligation). No
standalone grammar binary; the parser is the executable grammar.

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (item grammar) | SHIPPED | `parse_item` in `parser.rs` admits `fn`/`spec fn`/`#[slag] fn` (+ basis `struct`/`enum`). |
| REQ-2 (mandatory contract clauses) | SHIPPED | `parse_contract`/`parse_spec_fn` enforce `req`→`ens`+→`fx` and spec-fn `dec`. |
| REQ-3 (loop/while + inv* + one dec) | SHIPPED | `parse_loop` requires `inv`+ then one `dec`. |
| REQ-4 (statement grammar) | SHIPPED | `parse_block`/`parse_let`/`parse_return`/`parse_if_stmt` + tail expr. |
| REQ-5 — base expr grammar | SHIPPED | precedence ladder `parse_or`→…→`parse_postfix`→`parse_primary` in `parser.rs`; corpus exprs round-trip. |
| REQ-5 — char/hex/binary literals (#91/#92) | SHIPPED | the lexer emits the SAME `TokKind::Int` for `'A'`/`0x1b`/`0b101` (`lexer.rs` `lex_char`/`lex_int`); `parse_primary`'s literal arm is UNCHANGED (it consumes any `Int`), so all radices+char build `Expr::IntLit` (test `char_hex_binary_parse_to_intlit_no_new_variant`). `''`/`0x`/`'é'` are lex errors (`malformed_literals_are_structured_diagnostics_not_panic`). |
| REQ-5 — operators `% << >> & \| ^ !` (#92) | SHIPPED | `parser.rs` threads `parse_mul`(+`%`)→`parse_shift`→`parse_bitand`→`parse_bitxor`→`parse_bitor` between comparison and addition, with `is` above the bitwise tiers (OQ-3) and a `parse_unary` (`!` prefix) above `parse_ref`. Each builds the `ast.md` REQ-10 node (`each_new_operator_parses_to_its_binop_node`). |
| REQ-6 (one call syntax) | SHIPPED | `parse_postfix` + `parse_path_expr` (`::`→`Path`). |
| REQ-7 (pattern grammar) | SHIPPED | `parse_pattern`/`parse_slice_pattern`/`parse_path_pattern`. |
| REQ-8 (type grammar) | SHIPPED | `parse_type` covers prims/`&T`/`&mut T`/`&[T]`/`Name<T>`. |
| REQ-9 (effect-row grammar) | SHIPPED | `parse_effect_row`/`parse_effect`. |
| REQ-10 (operator precedence pinned, #92) | SHIPPED | the ladder realizes the pinned standard-Rust precedence: `* / %` > `+ -` > `<< >>` > `&` > `^` > `\|` > comparison > `&&` > `\|\|`, with prefix `!` tighter than all binaries. Tests `modulo_binds_tighter_than_add`, `shift_binds_looser_than_add`, `not_binds_tighter_than_bitand`, `bitand_binds_tighter_than_bitor` (`thermite-syntax/tests/operators_parse.rs`). GROUNDED: `a % b + 1` groups `(a%b)+1`, verus-certified (`forge/tests/operators_conformance.rs::precedence_rem_binds_tighter_than_add`). |

## Open questions (for the orchestrator before the builder runs)

- **OQ-1 (UFCS exclusion):** unchanged; `Type::const` paths stay, UFCS method
  dispatch dropped. Not a blocker.
- **OQ-2 (typed closure params):** unchanged; untyped only in v0.1. Not a
  blocker.
- **OQ-3 (if-expression else-mandatory):** unchanged. Not a blocker.
- **OQ-4 (`!` precedence tier, #92):** REQ-10 places prefix `!` at the unary
  (`RefExpr`-adjacent) tier — tighter than every binary, so `!a & b` is
  `(!a) & b`. This matches Rust. The builder may implement it as a dedicated
  `parse_unary` between `parse_cast` and `parse_ref`. GROUNDED via the verus
  probes. Recorded; not a blocker.
- **OQ-5 (char model is byte/`u8`, #91/#92):** a char literal is a byte (`'A'`
  == 65), ASCII-only in v1 (`lexer.md` REQ-9). Non-ASCII chars await the
  `Vec<u8>` reshape. Recorded; not a blocker.
