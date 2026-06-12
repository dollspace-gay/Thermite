# Thermite Parser (recovering recursive descent)
<!--
tier: 3-component
status: draft
audited-sha: dff9ae866e3437af272a62e078993e66c1116460 (re-audited 2026-06-12: amended — greenfield Summary fixed + post-pin currency note (basis/C7-C12/#193 parse surface, resync set, ladder threading), #262)
governs: thermite-syntax/src/parser.rs
thesis-refs:
  - thermite-design.md §4.1
  - thermite-design.md §4.3
  - thermite-design.md §4.4
  - thermite-design.md §2 (pillar 4 crisp feedback, pillar 5 locality)
references:
  - conformance/sum.th
  - conformance/binary_search.th
  - conformance/parse/ (round-trip / AST-shape / recovery fixtures)
-->

## Summary

The parser is a hand-written **recursive-descent** consumer of the lexer's token
stream (`lexer.md`) producing the AST (`ast.md`). It is the executable form of
`surface-grammar.md`. Two design-mandated properties dominate its contract:
(a) **per-item error recovery** — a syntax error inside one item must NOT cascade
into the next (§4.3, pillar 5 locality); and (b) **mandatory-clause enforcement**
— a `fn` missing `req`/`ens`/`fx`, or a `loop`/`while` missing `inv`/`dec`, is a
parse error, never an implicit default (§4.1). It is **REGISTRY-FREE**: it parses
combinator calls (`forall_in`, `sorted`) as generic call expressions.

This doc's REQs are SHIPPED (`thermite-syntax/src/parser.rs`, issue #3 + the
#92/#93 amendments) — see the REQ status table.

> **AMENDMENT (#262 re-audit, 2026-06-12 — post-pin currency).** The parser has
> since grown ADDITIVE surface owned by OTHER design docs (each contracted in
> `parser.rs`'s module-doc REQ tables — not re-owned here): the dedicated
> `Option`/`Vec`/`Box`/`String` type arms + the two-arg type parses
> `Result<T, E>` / `Map<K, V>` (09-option-result / 13-map / 04-collections /
> 07-strings); the optional exec-fn `dec` clause after `fx` (`parse_fn`,
> 10-recursion-tuples C9-A); tuples (`parse_type_inner`'s `(`-arm
> disambiguation, `Expr::Tuple`/`TupleProj` in `parse_primary`/`parse_postfix` —
> C9-B); the C10 ergonomics desugars (`parse_let` tuple destructuring,
> `parse_for`, `parse_if_let`/`parse_while_let`, match guards, or-patterns —
> 11-ergonomics); the `struct`/`enum`/`#[sealed]` item parses (`parse_struct`/
> `parse_enum`, 01-adts/06-provenance); and the `?N` hole statement arm
> (`parse_block`'s `TokKind::Hole` arm → `parse_hole`,
> `SyntaxError::HoleOutsideFnBody` — `.design/forge/goal-repl.md` REQ-4, #193).
> Two corrections against the current tree: the item-boundary RESYNC set
> (REQ-3 / OQ-1) is now `fn`/`spec`/`#[`/`struct`/`enum`/EOF
> (`resync_to_item_boundary` — the basis Stage-1a item kinds joined it), and the
> precedence ladder threads `parse_cmp` → `parse_is` → `parse_bitor` →
> `parse_bitxor` → `parse_bitand` → `parse_shift` → `parse_add` (OQ-3 RESOLVED:
> `is` sits ABOVE the bitwise tiers, exactly as the REQ-8 status row records).

## Requirements

- **REQ-1 (recursive-descent over the surface grammar):** The parser implements
  exactly the productions of `surface-grammar.md` as recursive-descent functions
  (one per non-terminal family), with the expression-precedence ladder
  (`OrExpr`→`AndExpr`→`CmpExpr`→`BitOrExpr`→`BitXorExpr`→`BitAndExpr`→`ShiftExpr`
  →`AddExpr`→`MulExpr`→`CastExpr`→`UnaryExpr`→`RefExpr`→`Postfix`→`Primary`) and
  non-associative comparison from that grammar. It accepts both corpus programs
  in full and rejects the constructs §4.4 removes. Derived from §4.3 +
  `surface-grammar.md`.

  > **AMENDMENT (#92).** The ladder gains the modulo/shift/bitwise tiers
  > (`BitOrExpr`/`BitXorExpr`/`BitAndExpr`/`ShiftExpr`, and `%` folded into
  > `MulExpr`) and a unary `!` tier (`UnaryExpr`), at the standard-Rust
  > precedence pinned in `surface-grammar.md` REQ-10.

- **REQ-2 (mandatory-clause enforcement is a parse error):** Parsing a `fn`
  emits a `SyntaxError` if any of `req`, `ens` (≥1), or `fx` is absent or
  out-of-order; parsing a `loop`/`while` errors if `inv` (≥1) or `dec` (exactly
  one) is absent. The parser enforces PRESENCE, ORDER, and CARDINALITY only.
  Derived from §4.1 + the scope boundary in `surface-grammar.md`.

- **REQ-3 (per-item recovery, no cascade):** On a syntax error inside an item,
  the parser records a `SyntaxError`, resyncs to the next item-boundary token
  (`fn`, `spec`, `#[`) or EOF, and resumes. A malformed item does not corrupt the
  following well-formed items. Derived from §4.3 + pillar 5.

- **REQ-4 (Result / diagnostics-bearing return, no panics):** The parser returns
  a structure bearing the recovered `Program` AND the full `Vec<SyntaxError>`.
  No `unwrap`/`expect`/`panic!` in production. Every diagnostic carries a span +
  an actionable message. Derived from R-CODE-2 + §2 pillar 4.

- **REQ-5 (round-trip / AST-shape fidelity):** Parsing `conformance/sum.th` and
  `conformance/binary_search.th` produces the AST shapes pinned in `ast.md` with
  zero diagnostics. The `conformance/parse/` fixtures are the oracle. Derived
  from `goal.md` + `conformance/README.md`.

- **REQ-6 (one call syntax disambiguation):** The parser resolves postfix `.` to
  `MethodCall` (when followed by `(…)`) or `Field`, free `name(args)` to `Call`,
  and `::`-segmented names to `Path` (`u32::MAX`). Derived from §4.4.

- **REQ-7 (addressing substrate available):** The parser produces AST nodes from
  which `semantic-addressing.md`'s deterministic numbering is computable. Derived
  from §4.3 + `ast.md` REQ-8.

- **REQ-8 (operator-tier parsing — NEW, #92):** The parser parses the new binary
  operators and the unary `!` into the AST nodes of `ast.md` REQ-10 at the
  precedence of `surface-grammar.md` REQ-10:
  - `%` (`TokKind::Percent`) folds into the `MulExpr` tier alongside `*`/`/` →
    `Binary { op: BinOp::Rem, .. }`;
  - `<<`/`>>` (`TokKind::Shl`/`Shr`) at a NEW `ShiftExpr` tier (below `+ -`) →
    `BinOp::Shl`/`Shr`;
  - `&` (`TokKind::Amp`) at a NEW `BitAndExpr` tier → `BinOp::BitAnd` (the parser
    distinguishes binary `&` here from the prefix `&`/`&mut` reference in
    `RefExpr` by position: a `&` in operator position binds two operands);
  - `^` (`TokKind::Caret`) at a NEW `BitXorExpr` tier → `BinOp::BitXor`;
  - `|` (`TokKind::Pipe`) at a NEW `BitOrExpr` tier → `BinOp::BitOr` (the parser
    distinguishes binary `|` from a closure delimiter `|params|` by context: a
    closure `|` opens a closure in `Primary`, an operator `|` joins two operands
    in `BitOrExpr`);
  - prefix `!` (`TokKind::Bang`) at a NEW `UnaryExpr` tier (tighter than all
    binaries) → `Unary { op: UnaryOp::Not, .. }`.

  The `!` meaning (bitwise vs logical) is per operand type, resolved downstream
  (validator/lower) — the PARSER produces one `UnaryOp::Not` regardless (§2.3,
  `ast.md` OQ-4). Derived from §4.4 + `surface-grammar.md` REQ-10.

- **REQ-9 (partiality is NOT a parse concern — NEW, #92):** The parser parses
  `a / b`, `a % b`, `a << k`, `a >> k` UNCONDITIONALLY — it does NOT check or
  inject the divide-by-zero / shift-bound obligation. That obligation is a §7
  PROOF obligation discharged at verification (`ast.md` REQ-11): the lowering
  emits the bare Verus operator and Verus raises the obligation. The parser's
  only job is to build the `Binary` node. Derived from the scope boundary in
  `surface-grammar.md` + `ast.md` REQ-11.

- **REQ-10 (`break` / `continue` statement parsing + in-loop enforcement — NEW,
  #93):** The parser recognizes the loop-control statements:
  - In `parse_block`'s statement dispatch, a `TokKind::Break` followed by `;`
    builds `Stmt::Break`; a `TokKind::Continue` followed by `;` builds
    `Stmt::Continue` (`ast.md` REQ-12). Both are statement-only, payload-less,
    and value-less (no `break expr`, no loop label — §2.3); a missing `;` is a
    `SyntaxError` (presence/cardinality, like every statement).
  - **In-loop structural enforcement.** `break`/`continue` are valid ONLY inside
    a `loop`/`while` body. The parser tracks loop-nesting depth (a counter
    incremented in `parse_loop_inner` around the body parse) and emits a
    `SyntaxError` for a `break;`/`continue;` parsed at loop-depth 0 (e.g. at a
    function-body top level, outside any loop). This is a STRUCTURAL parse rule,
    not a verification rule — analogous to the mandatory-clause enforcement
    (REQ-2): the parser owns presence/position; Verus owns the invariant/
    decreases semantics (`verus-lowering.md` REQ-12).

  Before #93, `break`/`continue` lexed as identifiers and parsed as a bare
  identifier-expression statement (`Stmt::Expr(Path(["break"]))`) — a silent
  no-op (the editor's quit-flag workaround). #93 makes them real statements.
  Derived from §4.1 (the loop model) + `surface-grammar.md` REQ-11 + `ast.md`
  REQ-12.

## Acceptance criteria

- **AC-1 (corpus round-trips):** parsing `sum.th`/`binary_search.th` yields the
  `ast.md` shapes with zero diagnostics, including
  `binary_search.loop#1.inv#2 → forall_from(...)`. (REQ-1, REQ-5, REQ-6, REQ-7)
- **AC-2 (missing clause = diagnostic):** removing `req`/`ens`/`fx`/`inv`/`dec`
  or reordering `req`/`ens`/`fx` each produces a `SyntaxError`. (REQ-2)
- **AC-3 (per-item recovery, no cascade):** the `recover_per_item` fixture: a
  malformed first item + a well-formed second; ≥1 diagnostic for item one, item
  two parses to its correct node. (REQ-3)
- **AC-4 (no panic):** no input — including negative/recovery fixtures and
  malformed literals (`''`, `0x`, `'é'`) and a `break;` outside a loop — panics.
  (REQ-4)
- **AC-5 (operator tiers + precedence — #92):** `a % b` → `Binary { op:
  Rem }`, `a << k` → `Shl`, `a >> k` → `Shr`, `a & b` → `BitAnd`, `a | b` →
  `BitOr`, `a ^ b` → `BitXor`, `!a` → `Unary { op: Not }`. Precedence:
  `a % b + 1` parses as `(a % b) + 1`, `a + b << c` as `(a + b) << c`,
  `!a & b` as `(!a) & b`, `a & b | c` as `(a & b) | c`. (REQ-8)
- **AC-6 (binary `|` vs closure `|`, binary `&` vs ref `&` — #92):** `|x| x`
  parses as a closure (in `Primary`); `a | b` parses as `BinOp::BitOr`; `&e`
  parses as a `Ref`; `a & b` as `BinOp::BitAnd`. The parser disambiguates by
  position. (REQ-8)
- **AC-7 (char/hex/binary literal parse — #91/#92):** `'A'`/`0x1b`/`0b101`
  each parse to `Expr::IntLit` (the same node as decimal); `let c: u8 = 'A';`
  parses. (REQ-1, ties to `lexer.md` REQ-3/REQ-9.)
- **AC-8 (`break`/`continue` parse + in-loop rule — NEW, #93):** A `while … { …
  break; }` body parses with a `Stmt::Break` in its `stmts`; `continue;` to
  `Stmt::Continue`; both require a trailing `;`. A `break;` or `continue;` parsed
  at the function-body top level (NO enclosing loop) produces a `SyntaxError`
  (`break` outside a loop), never a panic. A `break;` nested inside an `if`
  WITHIN a loop body is accepted (depth > 0). (REQ-10)

## Architecture

A hand-written recursive-descent parser in `thermite-syntax/src/parser.rs` with a
cursor over the `Vec<Token>` from `lexer.md`. One function per grammar family.

**Expression ladder (REQ-1, REQ-8).** The existing ladder
(`parse_or`→`parse_and`→`parse_cmp`→`parse_is`→`parse_add`→`parse_mul`→
`parse_cast`→`parse_ref`→`parse_postfix`→`parse_primary`) GAINS the #92 tiers,
threaded at the pinned precedence between `parse_cmp` and `parse_add`:
`parse_cmp` → `parse_bitor` → `parse_bitxor` → `parse_bitand` → `parse_shift` →
`parse_add`; `parse_mul` adds the `%` arm; and a `parse_unary` (`!` prefix) sits
between `parse_cast` and `parse_ref`. (The `parse_is` tier of the basis ADT stage
stays where it is; the builder reconciles its position with the new tiers,
keeping `is` at its current binding.) Each new tier is a left-folding loop
mirroring `parse_add`/`parse_mul`.

**Disambiguation (REQ-8, AC-6).** `&` in `parse_bitand` is the BINARY operator
(two operands); `&`/`&mut` in `parse_ref` is the PREFIX reference (one operand) —
distinguished by parse position (prefix vs infix). `|` in `parse_bitor` is the
BINARY operator; `|` opening a closure is recognized only in `parse_primary`
(`Closure`) — distinguished by position. `!` in `parse_unary` is the prefix
unary; `!=` (`TokKind::Ne`) is already a distinct maximal-munch token, so a
standalone `!` is unambiguously the unary operator.

**`break`/`continue` statement parsing + in-loop enforcement (REQ-10, AC-8).**
The statement dispatch in `parse_block` (which already branches on
`TokKind::Let`/`Return`/`Loop`/`While`/`If` before falling through to an
expression statement) gains two arms: `TokKind::Break` → consume `break`, consume
`;`, push `Stmt::Break`; `TokKind::Continue` likewise → `Stmt::Continue`. The
in-loop rule is a depth counter: `parse_loop_inner` increments it around the body
parse and decrements after; the `break`/`continue` arms read the counter and emit
a `SyntaxError` if it is 0 (outside any loop). The counter increments per loop, so
a `break` inside an `if` block that is itself inside a loop is depth ≥ 1 and
accepted. The arms are pure parsing — they do NOT inject any invariant/decreases
reasoning (that is Verus's job, `verus-lowering.md` REQ-12). This is a new `Stmt`
construction site; the broader `match Stmt` ripple is pinned in `ast.md`
Architecture (the `Stmt` ripple).

**Mandatory clauses (REQ-2), per-item recovery (REQ-3), registry-free (REQ-6),
Result discipline (REQ-4), addressing substrate (REQ-7)** — unchanged.

**Partiality (REQ-9).** The parser builds `Binary { op: Div/Rem/Shl/Shr, .. }`
with no obligation check; the lowering (`ast.md` REQ-11) emits the bare Verus
operator and Verus raises the divide-by-zero / shift-bound obligation. The
parser MUST NOT inject `req` clauses or guards — that would silently alter the
contract.

## Verification

`cargo test -p thermite-syntax` against `conformance/parse/`:
- round-trip / AST-shape fixtures for the corpus (AC-1);
- missing/misordered-clause negatives (AC-2);
- `recover_per_item` (AC-3);
- a no-panic sweep over negatives incl. malformed literals + a `break;` outside
  a loop (AC-4);
- operator-tier + precedence fixtures (AC-5), `|`/`&` disambiguation (AC-6),
  char/hex/binary literal fixtures (AC-7);
- NEW `break`/`continue` parse fixtures: a `while` body with `break;`/`continue;`
  parses to `Stmt::Break`/`Continue`; a top-level `break;` is a `SyntaxError`; a
  loop-nested-`if` `break;` is accepted (AC-8).

The operator + literal SEMANTICS are GROUNDED end-to-end through `forge`/
`thermite-lower` certifying real Verus (`ast.md` Verification). The `break`/
`continue` END-TO-END verification semantics (invariant-at-continue, decreases
interaction, break-exit, diverge-loop) are owned + GROUNDED in
`verus-lowering.md` (#93). Expected ASTs are hand-derived (R-CHAR-3).

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (recursive descent) | SHIPPED | `parser.rs` has one fn per grammar family + the precedence ladder; corpus parse facts pass. The #92 tier extensions are REQ-8. |
| REQ-2 (mandatory-clause enforcement) | SHIPPED | `parse_contract`/`parse_loop` enforce presence/order/cardinality; recovery test. |
| REQ-3 (per-item recovery) | SHIPPED | `parse_program` + `resync_to_item_boundary`; test `recover_per_item`. |
| REQ-4 (Result / no panic) | SHIPPED | `pub fn parse → ParseResult { program, errors }`; `enum SyntaxError`; test `negative_inputs_never_panic`. |
| REQ-5 (round-trip fidelity) | SHIPPED | `tests/conformance.rs` asserts both corpus programs with 0 diagnostics. |
| REQ-6 (one call syntax) | SHIPPED | `parse_postfix` + `parse_path_expr`. |
| REQ-7 (addressing substrate) | SHIPPED | loops/`inv`s kept in source order; `address.rs` numbers them. |
| REQ-8 (operator tiers `% << >> & \| ^ !`, #92) | SHIPPED | `parse_mul` adds the `Percent`→`Rem` arm; `parse_shift`/`parse_bitand`/`parse_bitxor`/`parse_bitor` are the new left-folding tiers (threaded `parse_is`→`parse_bitor`→…→`parse_add`); `parse_unary` builds the prefix `!`→`Unary { Not }`. Binary `&`/`\|` vs prefix ref `&`/closure `\|` disambiguated by position (tests `each_new_operator_parses_to_its_binop_node`, `binary_pipe_distinct_from_closure_pipe`). GROUNDED L3 for all forms (`forge/tests/operators_conformance.rs`). |
| REQ-9 (partiality not a parse concern, #92) | SHIPPED | `parse_mul`/`parse_shift` build the `Binary { op: Rem/Shl/Shr, .. }` node UNCONDITIONALLY — no `req` injection, no obligation check. The div-by-zero / shift-bound obligation is a §7 proof obligation raised by the bare Verus operator the lowering emits (`ast.md` REQ-11). GROUNDED: `%` without `req b != 0` is L0, `<<` unbounded is L0 (`forge/tests/operators_conformance.rs::rem_without_nonzero_req_is_l0` / `shift_without_bound_is_l0`). |
| REQ-10 (`break`/`continue` parse + in-loop rule, #93) | SHIPPED | `parse_block`'s statement dispatch (`parser.rs`) gains `TokKind::Break`/`Continue` arms calling `parse_break_continue`, which consumes the keyword + mandatory trailing `;` and builds `Stmt::Break`/`Continue`. The in-loop structural rule is the `Parser.loop_depth` counter, incremented around the body parse in `parse_loop_inner` (symmetric decrement on every exit path); a `break;`/`continue;` at depth 0 is a `SyntaxError::BreakContinueOutsideLoop` (a new span-bearing variant, `Display`-able, no panic). A nested-in-`if` break inside a loop is depth > 0 → accepted. Tests (`forge/tests/break_continue_conformance.rs`): `break_or_continue_outside_a_loop_is_a_structured_error_not_a_panic` (top-level break/continue → structured error) + `break_and_continue_inside_a_loop_parse_cleanly_as_stmt_nodes` (loop-nested → `Stmt::Break`/`Continue`). |

## Open questions (for the orchestrator)

- **OQ-1 (resync token set):** unchanged; `fn`/`spec`/`#[`/EOF. Not a blocker.
- **OQ-2 (return-shape):** unchanged; `ParseResult` preferred. Not a blocker.
- **OQ-3 (`parse_is` tier vs new #92 tiers):** REQ-8 inserts the bitwise tiers
  between comparison and addition; the basis-stage `parse_is` currently sits
  between comparison and addition (`parse_cmp` → `parse_is` → `parse_add`). The
  builder must place `is` consistently — recommended: keep `is` ABOVE the
  bitwise/shift tiers (so `a & b is Variant` reads as `(a & b) is Variant`),
  matching `is`'s current near-comparison binding. Flagged for the critic to
  confirm the placement does not regress the ADT `is` facts. Not a blocker for
  the AST shape, but the builder MUST pin a single placement.
- **OQ-4 (binary `|` / closure `|` ambiguity, #92):** REQ-8 resolves it by
  position (closure `|` only in `Primary`, operator `|` in `BitOrExpr`). A
  pathological `|x| x | 1` is `|x| (x | 1)` (the closure body is a full `Expr`).
  Recorded; the builder confirms via an AC-6 fixture. Not a blocker.
- **OQ-5 (in-loop check: parser vs validator, #93):** REQ-10 puts the "break
  outside a loop" check IN THE PARSER (a depth counter) rather than deferring it
  to the validator (#2) — it is a purely structural/syntactic rule (like the
  mandatory-clause enforcement REQ-2), needs no type/effect information, and
  gives the crispest feedback (pillar 4) at parse time. The builder MAY instead
  surface it as a validator diagnostic if the depth counter complicates recovery;
  either is acceptable as long as a top-level `break;` is rejected SOMEWHERE
  before lowering (an un-rejected top-level `break;` would lower to a Verus
  `break` outside a loop, a Verus error — so it is caught no later than verus,
  but pillar-4 wants it earlier). Flagged; not a blocker.
