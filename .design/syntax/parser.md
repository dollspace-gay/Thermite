# Thermite Parser (recovering recursive descent)
<!--
tier: 3-component
status: draft
governs: thermite-syntax/src/parser.rs
thesis-refs:
  - thermite-design.md §4.1
  - thermite-design.md §4.3
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
combinator calls (`forall_in`, `sorted`) as generic call expressions and never
consults thermite-spec (`goal.md`: the fixed-combinator-set rule is a downstream
*semantic* check, not a grammar rule).

This doc is GREENFIELD / FORWARD-LOOKING: no `parser.rs` exists. Every REQ is
**NOT-STARTED**, blocked on issue #3.

## Requirements

- **REQ-1 (recursive-descent over the surface grammar):** The parser implements
  exactly the productions of `surface-grammar.md` as recursive-descent functions
  (one per non-terminal family), with the expression-precedence ladder
  (`OrExpr`→…→`Postfix`→`Primary`) and non-associative comparison from that
  grammar. It accepts both corpus programs in full and rejects the constructs
  §4.4 removes. Derived from §4.3 + `surface-grammar.md`.

- **REQ-2 (mandatory-clause enforcement is a parse error):** Parsing a `fn`
  emits a `SyntaxError` if any of `req`, `ens` (≥1), or `fx` is absent or
  out-of-order; parsing a `loop`/`while` errors if `inv` (≥1) or `dec` (exactly
  one) is absent. The parser enforces clause PRESENCE, ORDER, and CARDINALITY
  only — it does NOT check that `ens` mentions `result`, that `req` is
  satisfiable, or that combinators are registered (those are forge/thermite-spec,
  §7/§4.2). Derived from §4.1 ("absence is always a parse error, never an
  implicit default") + the scope boundary in `surface-grammar.md`.

- **REQ-3 (per-item recovery, no cascade):** On a syntax error inside an item,
  the parser (a) records a `SyntaxError` diagnostic with the error span, (b)
  **resyncs** by discarding tokens up to the next top-level item-boundary token
  (`fn`, `spec`, or `#[` beginning a `#[slag]` attribute) or EOF, and (c)
  resumes parsing the next item. A malformed item produces an error-marked /
  absent `Item` but the following well-formed items still parse to correct AST
  nodes. Items are parsed independently — recovery is per-item by construction
  (§4.3: "a syntax error inside one item cannot cascade into the next … recovery
  is per-item by construction"). Derived from §4.3 + pillar 5 (locality: "an
  edit's blast radius is its block").

- **REQ-4 (Result / diagnostics-bearing return, no panics):** The parser returns
  a structure bearing the parsed `Program` (the recovered items) AND the full
  `Vec<SyntaxError>` diagnostics — e.g. `ParseResult { program, errors }` or
  `Result<Program, Vec<SyntaxError>>` such that even on error the recovered items
  are recoverable for tooling. No `unwrap`/`expect`/`panic!` in production
  (`goal.md` R-CODE-2; `thermite-syntax`'s `SyntaxError` per scaffold REQ-3).
  Every diagnostic carries a span (from `lexer.md` REQ-7) and an actionable
  message (pillar 4). Derived from R-CODE-2 + §2 pillar 4.

- **REQ-5 (round-trip / AST-shape fidelity):** Parsing `conformance/sum.th` and
  `conformance/binary_search.th` produces the AST shapes pinned in `ast.md`
  (AC-1, AC-3) with zero diagnostics. The `conformance/parse/` fixtures are the
  oracle: a round-trip (parse → AST-shape assertion, and where a formatter exists,
  parse → print → parse stability) over both programs. Derived from `goal.md`
  (corpus is the parser's verification oracle) + `conformance/README.md`.

- **REQ-6 (one call syntax disambiguation):** The parser resolves the postfix
  `.` form to a `MethodCall` (when followed by `( … )`) or `Field` (otherwise),
  free `name(args)` to a `Call`, and `::`-segmented names to a `Path`
  (`u32::MAX`, `Some`) — never treating `::` as method dispatch
  (`surface-grammar.md` REQ-6; §4.4 "one call syntax"). Derived from §4.4 + the
  corpus (`xs.len()` vs `spec_sum(t)` vs `u32::MAX`).

- **REQ-7 (addressing substrate available):** The parser produces AST nodes from
  which `semantic-addressing.md`'s deterministic numbering is computable — loops
  in source order within their function, `inv` in source order within their loop
  — so `address.rs` (governed by `semantic-addressing.md`) can resolve
  `binary_search.loop#1.inv#2`. The parser does not itself implement the address
  string syntax; it guarantees the structural order addressing relies on. Derived
  from §4.3 + `ast.md` REQ-8.

## Acceptance criteria

- **AC-1 (corpus round-trips):** `conformance/parse/` fixtures: parsing
  `sum.th` and `binary_search.th` yields the `ast.md`-pinned shapes with zero
  diagnostics. Specifically, parsing `binary_search.th` yields an item addressed
  `binary_search` whose `binary_search.loop#1.inv#2` resolves to
  `forall_from(haystack, hi, |x| x > needle)` (a `Call` to `forall_from` with a
  closure arg). (REQ-1, REQ-5, REQ-6, REQ-7)
- **AC-2 (missing clause = diagnostic):** A `conformance/parse/` negative
  fixture per case — `sum.th` with the `req` line removed, with an `ens` removed
  (leaving zero), with `fx` removed, with a `loop`/`while` `inv` removed, with
  `dec` removed, and with `req`/`ens`/`fx` reordered — each produces a
  `SyntaxError` (no silent default). (REQ-2)
- **AC-3 (per-item recovery, no cascade):** A `conformance/parse/` fixture
  (`recover_per_item`, per `tooling/spec-routes.toml conformance_ops`) with two
  items where the FIRST is malformed (e.g. a broken `fn` body) and the SECOND is
  well-formed: the parser emits ≥1 diagnostic for item one AND parses item two to
  its correct AST node. The error does not consume or corrupt item two. (REQ-3)
- **AC-4 (no panic):** No input — including the negative and recovery fixtures —
  causes a panic; all failures surface as `SyntaxError` diagnostics in the
  returned structure. (REQ-4)

## Architecture

A hand-written recursive-descent parser in `thermite-syntax/src/parser.rs` with a
cursor over the `Vec<Token>` from `lexer.md`. One function per grammar family
(`parse_item`, `parse_fn`, `parse_contract`, `parse_block`, `parse_stmt`,
`parse_loop`, `parse_expr_bp` for the precedence ladder, `parse_pattern`,
`parse_type`). The precedence ladder follows `surface-grammar.md` exactly
(non-associative comparison, postfix for `.`/`[]`/`(`).

**Mandatory clauses (REQ-2).** `parse_contract` requires `req` then `ens`+ then
`fx` in order; a missing or misordered keyword is a `SyntaxError` at that span.
`parse_loop`/`parse_while` require `inv`+ then exactly one `dec`. Because the AST
`Contract`/`Loop` types are non-optional in those fields (`ast.md` REQ-2/REQ-5),
the parser physically cannot build the node without them — the type system backs
the rule (§4.1).

**Per-item recovery (REQ-3).** The top-level loop is:
`while not EOF { match parse_item() { Ok(item) => push; Err(e) => { record e;
resync_to_item_boundary(); } } }`. `resync_to_item_boundary` discards tokens
until it sees `fn` / `spec` / `#[` / EOF (the item-start tokens), so a broken
item's tokens never bleed into the next. This realizes §4.3's "items are parsed
independently — recovery is per-item by construction" and pillar 5 locality
(blast radius = the block). The token spans (`lexer.md` REQ-7) make the resync
boundary precise.

**Registry-free (REQ-6, the corrected dependency note in `goal.md`).** The parser
has NO dependency on thermite-spec. `forall_in`, `forall_below`, `forall_from`,
`sorted` parse as ordinary `Call`/path expressions; the parser does not know they
are combinators. The "fixed combinator set" rule (§4.2) is enforced later, in
thermite-spec / forge, as a semantic check — confirmed by the absence of a
`thermite-spec/src/grammar.rs` route in `tooling/spec-routes.toml`.

**Result discipline (REQ-4).** Parsing returns a diagnostics-bearing structure
carrying both the recovered `Program` and `Vec<SyntaxError>`; no panics
(R-CODE-2). Errors are `thermite_syntax::SyntaxError` (scaffold REQ-3 — the first
fallible code in the toolchain introduces its own error type).

**Addressing substrate (REQ-7).** The parser guarantees structural source order
of loops within a function and `inv` within a loop; `semantic-addressing.md` /
`address.rs` compute the address strings over that order. The parser owns the
order; addressing owns the numbering.

## Verification

`cargo test -p thermite-syntax` against `conformance/parse/`:
- round-trip / AST-shape fixtures for `sum.th` and `binary_search.th` (AC-1),
  including the `binary_search.loop#1.inv#2 → forall_from(...)` assertion;
- one negative fixture per missing/misordered-clause case (AC-2);
- the `recover_per_item` fixture (AC-3) asserting item-two parses despite a
  broken item-one;
- a no-panic sweep over all negative inputs (AC-4).

Expected ASTs / diagnostics are hand-derived from `surface-grammar.md` + the
corpus, NEVER copied from the parser's own output (R-CHAR-3). This is the oracle
`goal.md` "verification model (A)" assigns to thermite-syntax.

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (recursive descent) | SHIPPED | `parser.rs` has one fn per grammar family + the precedence ladder (`parse_or`→…→`parse_postfix`→`parse_primary`); `sum`/`binary_search` parse facts pass. |
| REQ-2 (mandatory-clause enforcement) | SHIPPED | `parse_contract` requires `req`→`ens`+→`fx` in order; `parse_loop` requires `inv`+→one `dec`; absence/misorder → `SyntaxError` (test `negative_inputs_never_panic`, `recover_per_item`). |
| REQ-3 (per-item recovery) | SHIPPED | `parse_program` + `resync_to_item_boundary`; test `recover_per_item` confirms `ok` parses despite broken `broken`. |
| REQ-4 (Result / no panic) | SHIPPED | `pub fn parse → ParseResult { program, errors }`; `enum SyntaxError`; test `negative_inputs_never_panic`. |
| REQ-5 (round-trip fidelity) | SHIPPED | `tests/conformance.rs` asserts both corpus programs' facts with 0 diagnostics against `conformance/parse/`. |
| REQ-6 (one call syntax) | SHIPPED | `parse_postfix` → `MethodCall`/`Field`, free `f(args)` → `Call`, `parse_path_expr` `::` → `Path` (corpus `xs.len()`/`u32::MAX`). |
| REQ-7 (addressing substrate) | SHIPPED | loops/`inv`s kept in source order in the AST; `address.rs` numbers them (address oracle passes). |

## Open questions (for the orchestrator)

- **OQ-1 (resync token set):** REQ-3 resyncs to `fn`/`spec`/`#[`/EOF. If a future
  item kind is added (none in v0.1), the resync set grows. For v0.1 these three
  are the complete item-start set (`surface-grammar.md` `Item`). Recorded; not a
  blocker.
- **OQ-2 (return-shape: ParseResult vs Result):** REQ-4 allows either a
  `ParseResult { program, errors }` (always returns recovered items + diagnostics)
  or `Result<Program, Vec<SyntaxError>>`. The former is preferable for per-item
  recovery (tooling wants the surviving items even on partial failure); the
  builder picks one, the critic checks recovery is observable either way. Not a
  blocker.
