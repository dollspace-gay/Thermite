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
`conformance/binary_search.th`) and §4, and *no more* (pillar 3, "one way to do
everything", §2). It is the shared anchor that `lexer.md`, `ast.md`,
`parser.md`, and `semantic-addressing.md` all reference. The parser is the
executable form of this grammar; this doc fixes what is and is not accepted.

This doc is GREENFIELD / FORWARD-LOOKING: no parser code exists. Every REQ is
**NOT-STARTED**, blocked on issue #3. The acto-builder satisfies these REQs next.

## Scope boundary (grammar vs. semantics)

The grammar enforces **clause PRESENCE and structure only**. It does NOT enforce:

- that `ens` mentions `result` (a §7 structural-vacuity check; forge, issue #6),
- that combinators come from the fixed SpecTherm set (a §4.2 *semantic* check in
  thermite-spec / forge; `goal.md` "the parser is REGISTRY-FREE … the
  fixed-combinator-set rule is a SEMANTIC check, NOT a grammar rule"),
- that `req`/`inv` are non-vacuous, that types resolve, that effects subsume.

The grammar's job: a `fn` without all three of `req`/`ens`/`fx`, or a `loop`/
`while` missing `inv`/`dec`, is a **parse error** (§4.1: "absence is always a
parse error, never an implicit default"). A combinator call like
`forall_in(haystack, |x| ...)` parses as a generic call expression — the parser
neither knows nor cares whether `forall_in` is a registered combinator.

## Requirements

- **REQ-1 (item grammar):** The grammar admits exactly three top-level item
  forms — `fn`, `spec fn`, and `#[slag(...)] fn` — and nothing else (no `struct`,
  `impl`, `trait`, `use`, `mod`, `macro` in v0.1). Derived from §4.1, §4.2
  (`spec fn`), §8 (`#[slag]`), and the corpus (`sum`, `spec_sum`).

- **REQ-2 (mandatory contract clauses, fixed order):** A `fn` signature is
  `fn NAME ( PARAMS ) -> TYPE` followed by, in this exact order, `req EXPR`,
  then one-or-more `ens EXPR`, then exactly one `fx EFFECTROW`. A `spec fn`
  signature is `spec fn NAME ( PARAMS ) -> TYPE` followed by exactly one
  `dec EXPR` (spec functions carry a decreases-measure, not req/ens/fx — corpus
  `spec_sum` has only `dec xs.len()`). Absence of a required clause is a parse
  error. Derived from §4.1 ("`req`/`ens`/`fx` … Mandatory keyword"), §4.2
  ("No spec-level recursion without a `dec` measure"), and the two corpus
  programs verbatim.

- **REQ-3 (loop/while with mandatory inv* + exactly one dec):** Both `loop { }`
  and `while EXPR { }` carry one-or-more `inv EXPR` clauses followed by exactly
  one `dec EXPR`, then the body block. Missing `inv` or missing/duplicate `dec`
  is a parse error. Derived from §4.1 ("`inv` / `dec` … Mandatory on every
  `loop`/`while`"), corpus `binary_search` (`loop` + 3×`inv` + `dec`) and `sum`
  (`while` + 3×`inv` + `dec`).

- **REQ-4 (statement grammar):** A block `{ }` is a sequence of statements with
  an optional trailing tail expression (no semicolon → the block's value, as in
  `sum`'s final `acc`). Statements: `let mut? NAME : TYPE = EXPR ;`,
  assignment `LVALUE = EXPR ;`, `return EXPR? ;`, the `if`/`else` statement, and
  an expression-statement `EXPR ;`. Derived from §4.3 (explicit `{}` blocks, no
  significant whitespace) and the corpus bodies.

- **REQ-5 (expression grammar):** Expressions cover exactly: integer literals
  with optional `_` separators (`1_000_000`), `bool` literals, paths
  (`lo`, `u32::MAX`, `Some`, `None`), call `f(args)`, the ONE call syntax for
  member access (see REQ-6), closure `|params| EXPR`, `match EXPR { ARMS }`,
  `if EXPR { } else { }` as an expression, binary arithmetic (`+ - * /`),
  comparison (`== != < <= > >=`), logical (`&& ||`), indexing `a[i]` including
  range index `a[..i]`, cast `EXPR as TYPE`, references `&EXPR` / `&mut EXPR`,
  and parenthesized grouping. Derived from §4.4 and both corpus programs.

- **REQ-6 (one call syntax — DECISION):** Member/associated access uses **one
  call syntax**: a postfix `.` selects a field or a zero-or-more-arg method call,
  written `RECEIVER . NAME` (field) or `RECEIVER . NAME ( ARGS )` (method).
  `haystack.len()` and `xs.len()` (both corpus programs) parse as a method call
  on `haystack`/`xs`. The grammar admits the postfix dot form AND the free-call
  form `NAME(ARGS)` (e.g. `spec_sum(t)`, `forall_in(...)`); it does NOT admit a
  UFCS form `Type::method(receiver, ...)` as an alternate spelling of a method
  call (§4.4: "Method syntax vs UFCS choice → One call syntax"). `Type::NAME` is
  a *path* (associated constant / variant constructor, e.g. `u32::MAX`), parsed
  as a path expression, never as a method-dispatch sugar. See OQ-1.

- **REQ-7 (pattern grammar):** Patterns cover exactly: literal patterns, binding
  patterns (`i`, `head`), wildcard `_`, slice patterns `[]` and `[head, ..t]`
  (rest binding), and enum/tuple-struct patterns `Some(i)` / `None`. Derived
  from §4.1 (the `match result { Some(i) => …, None => … }` in `binary_search`)
  and Appendix A (`match xs { [] => …, [head, ..t] => … }`).

- **REQ-8 (type grammar):** Types cover exactly: primitive names (`u32`, `u64`,
  `usize`, `bool`), shared slice `&[T]`, references `&T` / `&mut T`, and one
  generic application `NAME<T>` (`Option<usize>`). No user generics beyond the
  closed built-in set; no lifetimes (§4.4). Derived from the corpus signatures
  (`&[u32]`, `u32`, `Option<usize>`, `u64`, `usize`) and §4.4 (lifetimes removed).

- **REQ-9 (effect-row grammar):** An `fx` row is either the keyword `pure` or a
  brace/paren-free set drawn from `{read(path), write(path), net(domain), alloc,
  time, rand, panic, diverge}`. The corpus uses only `fx pure`. Derived from
  §4.1 (the effect-row enumeration; `diverge` from "divergence requires
  `fx diverge`").

## Acceptance criteria

- **AC-1 (both corpus programs accept):** The grammar accepts
  `conformance/sum.th` (both `spec fn spec_sum` and `fn sum`) and
  `conformance/binary_search.th` in full, with no construct in either program
  unrecognized. Mechanically: the parser (parser.md AC-1) round-trips both.
- **AC-2 (missing clause rejects):** Removing the `req` line from `sum.th`,
  the single `ens` from a `fn`, the `fx` line, any `inv`, or the `dec` from a
  `loop`/`while` yields a parse error (not a silent default). Tied to
  `conformance/parse/` negative fixtures.
- **AC-3 (no extra constructs):** The grammar has no production for `struct`,
  `impl`, `trait`, `use`, `mod`, macro invocation, `for`, `unsafe`, an explicit
  lifetime token, or a UFCS `Type::method(recv, ..)` method call — each is a
  parse error. (REQ-1, REQ-6, REQ-8; §4.4.)
- **AC-4 (one call syntax round-trips both spellings in corpus):** `xs.len()`
  parses as a method-call expression and `spec_sum(t)` / `forall_in(...)` as
  free-call expressions; `u32::MAX` parses as a path (not a method call). Tied
  to `conformance/parse/` AST-shape fixtures.

## Architecture

EBNF (informative; `parser.md` is the operational contract). Symbol anchors,
never line numbers (R-CITE-2b). `'x'` is a terminal; `?` optional; `*` zero+;
`+` one+; `|` alternation.

```ebnf
Program     ::= Item*

Item        ::= Attr? FnItem | SpecFnItem
Attr        ::= '#[' 'slag' '(' SlagField (',' SlagField)* ')' ']'   ; §8
SlagField   ::= ('reason' | 'owner' | 'review') '=' StringLit

FnItem      ::= 'fn' Ident '(' Params? ')' RetType ReqClause EnsClause+ FxClause Block
SpecFnItem  ::= 'spec' 'fn' Ident '(' Params? ')' RetType DecClause Block

Params      ::= Param (',' Param)*
Param       ::= Ident ':' Type
RetType     ::= '->' Type                          ; '()' written explicitly if unit

ReqClause   ::= 'req' Expr                          ; mandatory (§4.1)
EnsClause   ::= 'ens' Expr                          ; >=1, mandatory (§4.1)
FxClause    ::= 'fx' EffectRow                      ; mandatory (§4.1)
DecClause   ::= 'dec' Expr                          ; mandatory on spec fn + loops

EffectRow   ::= 'pure' | Effect (',' Effect)*
Effect      ::= 'read' '(' PathArg ')' | 'write' '(' PathArg ')'
              | 'net' '(' PathArg ')' | 'alloc' | 'time' | 'rand'
              | 'panic' | 'diverge'

Block       ::= '{' Stmt* TailExpr? '}'
TailExpr    ::= Expr                                ; no trailing ';' -> block value
Stmt        ::= LetStmt | AssignStmt | ReturnStmt | IfStmt | ExprStmt
LetStmt     ::= 'let' 'mut'? Ident ':' Type '=' Expr ';'
AssignStmt  ::= LValue '=' Expr ';'
ReturnStmt  ::= 'return' Expr? ';'
IfStmt      ::= 'if' Expr Block ('else' Block)?     ; statement form
ExprStmt    ::= Expr ';'
LValue      ::= Ident | IndexExpr | FieldExpr

LoopExpr    ::= 'loop'  InvClause+ DecClause Block
WhileExpr   ::= 'while' Expr InvClause+ DecClause Block
InvClause   ::= 'inv' Expr

; Expression precedence (loosest -> tightest), one-way-to-do-everything (§2.3):
Expr        ::= OrExpr
OrExpr      ::= AndExpr ('||' AndExpr)*
AndExpr     ::= CmpExpr ('&&' CmpExpr)*
CmpExpr     ::= AddExpr (CmpOp AddExpr)?             ; non-associative comparison
CmpOp       ::= '==' | '!=' | '<' | '<=' | '>' | '>='
AddExpr     ::= MulExpr (('+' | '-') MulExpr)*
MulExpr     ::= CastExpr (('*' | '/') CastExpr)*
CastExpr    ::= RefExpr ('as' Type)*
RefExpr     ::= '&' 'mut'? RefExpr | Postfix
Postfix     ::= Primary PostfixOp*
PostfixOp   ::= '.' Ident ('(' Args? ')')?          ; field OR method — ONE call syntax
              | '[' IndexArg ']'                     ; a[i] / a[..i]
              | '(' Args? ')'                         ; free call  f(args)
Primary     ::= Literal | Path | Closure | MatchExpr | IfExpr | '(' Expr ')'

Closure     ::= '|' ClosureParams? '|' Expr
ClosureParams ::= Ident (',' Ident)*                ; corpus closures are untyped: |x| ...
MatchExpr   ::= 'match' Expr '{' MatchArm (',' MatchArm)* ','? '}'
MatchArm    ::= Pattern '=>' Expr
IfExpr      ::= 'if' Expr Block 'else' Block         ; expression form requires else

Path        ::= Ident ('::' Ident)*                 ; lo, u32::MAX, Some, None
IndexArg    ::= Expr | '..' Expr | Expr '..' Expr | Expr '..'   ; a[i], a[..i]
Args        ::= Expr (',' Expr)*

Pattern     ::= '_' | Literal | Ident                ; wildcard / literal / binding
              | '[' (SlicePat (',' SlicePat)*)? ']'  ; [] , [head, ..t]
              | Path ('(' Pattern (',' Pattern)* ')')? ; Some(i) / None
SlicePat    ::= Pattern | '..' Ident                 ; rest binding ..t

Type        ::= 'u32' | 'u64' | 'usize' | 'bool'
              | '&' 'mut'? Type
              | '&' '[' Type ']'                      ; &[u32]
              | Ident '<' Type '>'                    ; Option<usize>
Literal     ::= IntLit | BoolLit                      ; IntLit allows '_' : 1_000_000
```

**Key design decisions (one-way-to-do-everything, §2.3):**

1. **One call syntax (REQ-6).** Member access is the postfix `.` form only;
   `xs.len()` is a method call, `spec_sum(t)` is a free call. There is no UFCS
   alternate (`<[u32]>::len(xs)`). `::` is reserved for path segments
   (`u32::MAX`, `Some`), never for method dispatch. This is the single
   "Method syntax vs UFCS choice → One call syntax" resolution (§4.4).
2. **`if` is both a statement and an expression.** The corpus bodies use `if` as
   a statement (`if lo == hi { return None; }`) and the design forbids
   `match` ergonomics special-casing (§4.4). The expression form requires an
   `else` (it must have a value); the statement form does not. This is the one
   desugaring, always explicit.
3. **Comparison is non-associative** (`CmpExpr` takes at most one `CmpOp`), so
   `a < b < c` is a parse error — predictability over expressiveness (§2.3).
4. **`()` return type is written explicitly** (§4.4 "All conversions explicit"
   register); the grammar requires `-> Type`. No implicit unit return in a
   signature. (Corpus functions all return non-unit.)

## Verification

The grammar is verified through `parser.md`'s oracle, `conformance/parse/`:
round-trip and AST-shape fixtures derived from `sum.th` / `binary_search.th`
(AC-1, AC-4) plus negative fixtures for each missing-clause case (AC-2) and each
removed construct (AC-3). No standalone grammar binary; the parser is the
executable grammar (`goal.md`: "the parser is the grammar's executable form").

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (item grammar) | SHIPPED | `parse_item` in `parser.rs` admits exactly `fn`/`spec fn`/`#[slag] fn`; `negative_inputs_never_panic` rejects other item starts. |
| REQ-2 (mandatory contract clauses, fixed order) | SHIPPED | `parse_contract`/`parse_spec_fn` enforce `req`→`ens`+→`fx` and spec-fn `dec`; corpus parse facts + recovery test. |
| REQ-3 (loop/while + inv* + one dec) | SHIPPED | `parse_loop` requires `inv`+ then exactly one `dec`; both corpus loops pass facts. |
| REQ-4 (statement grammar) | SHIPPED | `parse_block`/`parse_let`/`parse_return`/`parse_if_stmt` + tail expr; corpus bodies parse clean. |
| REQ-5 (expression grammar) | SHIPPED | precedence ladder `parse_or`→…→`parse_postfix`→`parse_primary`; non-assoc `parse_cmp`; corpus exprs round-trip. |
| REQ-6 (one call syntax) | SHIPPED | `parse_postfix` (`MethodCall`/`Field`/`Call`) + `parse_path_expr` (`::`→`Path`); corpus `xs.len()`/`u32::MAX`. |
| REQ-7 (pattern grammar) | SHIPPED | `parse_pattern`/`parse_slice_pattern`/`parse_path_pattern`; `[]`/`[head, ..t]`/`Some(i)`/`None` parse (sum/binary_search facts). |
| REQ-8 (type grammar) | SHIPPED | `parse_type` covers prims/`&T`/`&mut T`/`&[T]`/`Name<T>`; corpus `&[u32]`/`Option<usize>` verified by param/ret facts. |
| REQ-9 (effect-row grammar) | SHIPPED | `parse_effect_row`/`parse_effect` parse `pure` + the effect set; corpus `fx pure` verified by parse facts. |

## Open questions (for the orchestrator before the builder runs)

- **OQ-1 (UFCS exclusion):** REQ-6 reads "Method syntax vs UFCS choice → One
  call syntax" (§4.4) as: keep the postfix-dot method form, drop UFCS as an
  *alternate spelling of method dispatch*. `Type::const` paths (`u32::MAX`)
  stay (they are paths, not method calls). This is the only reading consistent
  with the corpus (`xs.len()` and `u32::MAX` both appear). Flagged for
  confirmation; not a blocker.
- **OQ-2 (typed closure params):** The corpus closures are all untyped
  (`|x| x != needle`). The grammar admits only untyped closure params. If a
  future combinator needs `|x: T| ...`, the grammar is a superset away; recorded
  as a deliberate v0.1 restriction, not a blocker.
- **OQ-3 (if-expression else-mandatory):** REQ-5 makes the *expression* form of
  `if` require an `else` (it must produce a value); the *statement* form does
  not. The corpus only uses the statement form. Recorded; resolvable from §4.4
  ("one desugaring, always explicit"), not a blocker.
