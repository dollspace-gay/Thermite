# Thermite Lexer (tokenization)
<!--
tier: 3-component
status: draft
governs: thermite-syntax/src/lexer.rs
thesis-refs:
  - thermite-design.md §4.3
  - thermite-design.md §4.4
  - thermite-design.md §8
  - thermite-design.md §2 (pillar 3)
-->

## Summary

The lexer turns Thermite source text into a flat token stream consumed by the
recursive-descent parser (`parser.md`). Thermite has **no significant
whitespace** (§4.3); whitespace and `//` comments are insignificant separators.
The token set is exactly what the surface grammar (`surface-grammar.md`) needs
— no more (pillar 3, §2). The lexer is the first stage; it does not enforce
clause presence or any grammar structure (that is the parser's job).

This doc is GREENFIELD / FORWARD-LOOKING: no lexer code exists. Every REQ is
**NOT-STARTED**, blocked on issue #3.

## Requirements

- **REQ-1 (token set):** The lexer produces exactly these token kinds and no
  others: keywords, identifiers, integer literals, the two bool literals,
  punctuation/operators, the `#[slag(...)]` attribute tokens, and end-of-file.
  No string literals occur outside `#[slag]` field values (see REQ-4); no float,
  char, byte, or lifetime tokens exist (§4.4 removes lifetimes; the corpus has
  no floats/chars). Derived from `surface-grammar.md` terminals and §4.4.

- **REQ-2 (keywords, fixed closed set):** The reserved keywords are exactly:
  `fn`, `spec`, `req`, `ens`, `fx`, `inv`, `dec`, `pure`, `let`, `mut`,
  `return`, `if`, `else`, `loop`, `while`, `match`, `as`. Each is lexed as a
  distinct keyword token, never as an identifier. Effect-row names (`read`,
  `write`, `net`, `alloc`, `time`, `rand`, `panic`, `diverge`) and `slag`,
  `reason`, `owner`, `review` are lexed as **identifiers**; the parser
  recognizes them contextually (they are not reserved, so they may also name
  user values). Derived from §4.1 (the mandatory keywords), §4.2 (`spec`), and
  `surface-grammar.md`.

- **REQ-3 (integer literals with `_` separators):** An integer literal is a
  run of ASCII digits with optional interior `_` separators; `1_000_000` lexes
  to one integer-literal token whose numeric value is `1000000` (the `_` are
  removed). A trailing/leading `_` adjacent to the digit run is not part of the
  literal. No type suffix is lexed (the corpus uses `as u64` casts, never
  `0u64`). Derived from Appendix A (`1_000_000`) and §4.4 ("All conversions
  explicit").

- **REQ-4 (`#[slag(...)]` attribute tokenization):** The attribute introducer
  `#[`, the inner `slag`/`reason`/`owner`/`review` identifiers, `=`, the
  string-literal field values, `,`, `)`, and `]` are lexed as ordinary tokens;
  string literals (double-quoted, used ONLY as `#[slag]` field values) are a
  token kind that carries the unescaped string content. The lexer does not
  validate field names or required-field presence — that is the parser/forge
  (§8). Derived from §8 (the `#[slag(reason=…, owner=…, review=…)]` example).

- **REQ-5 (comments + whitespace insignificant):** `//` to end-of-line is a
  comment; comments and any run of spaces/tabs/newlines are skipped as
  separators and never produced as tokens. There is no significant whitespace
  and no block-comment form (one comment syntax — pillar 3). Derived from §4.3
  ("no significant whitespace") and the corpus inline comments
  (`// overflow: discharged …`).

- **REQ-6 (punctuation / multi-char operators, maximal munch):** Operators and
  punctuation are lexed by maximal munch so multi-char tokens win over their
  prefixes: `->`, `=>`, `==`, `!=`, `<=`, `>=`, `&&`, `||`, `::`, `..`, `#[`,
  and the single-char `{ } ( ) [ ] , ; : . = < > + - * / & | !`. E.g. `<=` is
  one token, not `<` then `=`; `..` is one token, not two `.`; `&mut` lexes as
  `&` then the `mut` keyword. Derived from `surface-grammar.md` operators.

- **REQ-7 (spans for diagnostics + addressing):** Every token carries a source
  span (byte offset + length, or line/col) so the parser can attach spans to
  AST nodes for diagnostics (`parser.md`, R-CODE-2 crisp feedback) and so
  per-item recovery can resync to a token boundary. Derived from §2 pillar 4
  ("Feedback is always crisp") and §4.3 (per-item recovery needs token
  boundaries).

- **REQ-8 (Result discipline — no panics):** The lexer returns a `Result` /
  diagnostics-bearing structure; an unrecognized character (e.g. a stray `@`)
  produces a `SyntaxError` token-level diagnostic, never a panic
  (`goal.md` R-CODE-2; `thermite-syntax` owns the new `SyntaxError` type per
  `.design/scaffold/workspace.md` REQ-3). Derived from R-CODE-2 and the scaffold
  error-architecture decision.

## Acceptance criteria

- **AC-1 (corpus lexes clean):** Lexing `conformance/sum.th` and
  `conformance/binary_search.th` produces a token stream with zero error
  diagnostics; the inline comments and all whitespace are absent from the
  stream. (REQ-1, REQ-5)
- **AC-2 (`1_000_000` value):** The integer literal in `sum.th` lexes to a
  single integer-literal token with numeric value `1000000`. (REQ-3)
- **AC-3 (maximal munch):** In `binary_search.th`, `<=`, `==`, `->`, `=>`,
  `&&`, `::` (`u32::MAX` in `sum.th`), and `..` (`&xs[..i]` in `sum.th`,
  `[head, ..t]` in `sum.th`) each lex as exactly one token. (REQ-6)
- **AC-4 (keyword vs identifier):** `fn`/`spec`/`req`/`ens`/`fx`/`inv`/`dec`/
  `loop`/`while`/`match`/`let`/`mut`/`return`/`if`/`else`/`as`/`pure` lex as
  keywords; `read`/`forall_in`/`sorted`/`len`/`result`/`haystack` lex as
  identifiers. (REQ-2)
- **AC-5 (slag tokenizes):** A `#[slag(reason="x", owner="y", review="required")]`
  attribute lexes to `#[`, ident `slag`, `(`, ident/`=`/string triples, `]`,
  with string tokens carrying `x`/`y`/`required`. Tied to `conformance/slag`
  fixtures. (REQ-4)
- **AC-6 (no panic on bad char):** Lexing a source with a stray `@` yields a
  `SyntaxError` diagnostic and continues, never panicking. (REQ-8)

## Architecture

A single-pass, hand-written scanner over the source `&str` producing
`Vec<Token>` (or an iterator), each `Token { kind, span }`. No significant
whitespace means the scanner skips `[ \t\r\n]+` and `//`-to-EOL between tokens
and emits nothing for them (REQ-5). Maximal munch (REQ-6) is implemented by
checking the longest operator prefix first at each position. Integer literals
strip `_` while accumulating the value (REQ-3). The scanner is the lexical layer
of `surface-grammar.md`; its keyword set is exactly that grammar's keyword
terminals (REQ-2). It is registry-free — it does not know SpecTherm combinators
exist; `forall_in`, `sorted`, `len` are all plain identifiers (`goal.md`: the
parser/frontend is REGISTRY-FREE).

Spans (REQ-7) feed both diagnostics and `parser.md`'s per-item resync, which
seeks the next top-level-item-boundary token (`fn`/`spec`/`#[`) after an error.
Errors are `SyntaxError` values (REQ-8), the crate's own error type
(`.design/scaffold/workspace.md` REQ-3: "thermite-syntax introduces its OWN
`SyntaxError` since this is the first fallible code").

## Verification

`cargo test -p thermite-syntax` over lexer unit fixtures derived from the corpus:
token-stream snapshots for `sum.th` / `binary_search.th` (AC-1, AC-3, AC-4), the
`1_000_000` value assertion (AC-2), a `#[slag]` fixture (AC-5, `conformance/slag`),
and a stray-character negative fixture (AC-6). Expected token streams are
hand-derived from the grammar / corpus, never copied from the lexer's own output
(R-CHAR-3).

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (token set) | SHIPPED | `enum TokKind` in `lexer.rs` enumerates exactly keywords/ident/int/bool/str/`#[`/punct/eof; consumed by `parser.rs`. |
| REQ-2 (keywords closed set) | SHIPPED | `keyword_kind` in `lexer.rs` maps the 17 reserved words; effect/slag names fall through to `Ident` (test `int_literal`/parse facts). |
| REQ-3 (int literals with `_`) | SHIPPED | `lex_int` strips `_`; test `int_literal_underscores_strip_to_value` asserts `1_000_000` → `Int(1000000)`. |
| REQ-4 (`#[slag]` tokenization) | SHIPPED | `HashBracket` token + `lex_string`; consumed by `parse_slag` in `parser.rs`. |
| REQ-5 (comments + whitespace insignificant) | SHIPPED | `skip_trivia` in `lexer.rs` drops `[ \t\r\n]+` and `//`-EOL; corpus parses clean (0 errors). |
| REQ-6 (maximal munch operators) | SHIPPED | `lex_punct` tries 2-char operators first; corpus `<=`/`==`/`->`/`=>`/`::`/`..` lex as single tokens (parse facts pass). |
| REQ-7 (spans) | SHIPPED | every `Token` carries `Span { start, len }`; consumed by parser diagnostics (`SyntaxError::span`) + `address.rs`. |
| REQ-8 (Result discipline) | SHIPPED | `tokenize` returns `(Vec<Token>, Vec<SyntaxError>)`; `SyntaxError` enum in `parser.rs`; test `stray_char_is_diagnostic_not_panic`. |

## Open questions (for the orchestrator)

- **OQ-1 (effect/slag names not reserved):** REQ-2 lexes `read`/`write`/`net`/
  `alloc`/`time`/`rand`/`panic`/`diverge` and `slag`/`reason`/`owner`/`review`
  as identifiers (contextual keywords), not reserved words, so they can also be
  ordinary identifiers. This keeps the reserved set minimal (pillar 3) and is
  consistent with the corpus (which never uses them as identifiers). Recorded;
  not a blocker.
