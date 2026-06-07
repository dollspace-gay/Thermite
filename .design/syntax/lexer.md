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
  others: keywords, identifiers, integer literals (decimal/hex/binary AND the
  char-literal form, all carried by ONE `Int` token — REQ-3/REQ-9), the two
  bool literals, punctuation/operators, the `#[slag(...)]` attribute tokens,
  and end-of-file. No string literals occur outside `#[slag]` field values (see
  REQ-4). **No float, byte-string, or lifetime tokens exist** (§4.4 removes
  lifetimes; the corpus has no floats; byte-string literals are out of scope for
  v1). A **char literal `'A'` is NOT a distinct token kind** — it lexes to the
  SAME `TokKind::Int { value, raw }` token as a numeric literal, carrying the
  byte value of the character (`'A'` → value 65) and the verbatim raw `"'A'"`
  (REQ-9, the byte-level `u8` char model, #91/#92). Derived from
  `surface-grammar.md` terminals and §4.4.

  > **AMENDMENT (#91/#92, supersedes the prior "no char tokens exist" wording).**
  > REQ-1 previously forbade char tokens outright. It now ADMITS the char-literal
  > SYNTAX `'c'` but lexes it into the existing integer-literal token (no new
  > token kind, no Expr-variant break — see REQ-9 and `ast.md` REQ-6). Floats,
  > byte-strings, and lifetimes remain forbidden.

- **REQ-2 (keywords, fixed closed set):** The reserved keywords are exactly:
  `fn`, `spec`, `req`, `ens`, `fx`, `inv`, `dec`, `pure`, `let`, `mut`,
  `return`, `if`, `else`, `loop`, `while`, `match`, `as` (plus the basis-stage
  `struct`/`enum`/`is`). Each is lexed as a distinct keyword token, never as an
  identifier. Effect-row names (`read`, `write`, `net`, `alloc`, `time`, `rand`,
  `panic`, `diverge`) and `slag`, `reason`, `owner`, `review` are lexed as
  **identifiers**; the parser recognizes them contextually (they are not
  reserved, so they may also name user values). Derived from §4.1 (the mandatory
  keywords), §4.2 (`spec`), and `surface-grammar.md`.

- **REQ-3 (integer literals — decimal/hex/binary + `_` separators — value AND
  verbatim raw):** An integer literal is one of:
  - a **decimal** run of ASCII digits with optional interior `_` separators
    (`1_000_000`);
  - a **hexadecimal** literal `0x` / `0X` followed by a run of hex digits
    (`0-9a-fA-F`) with optional interior `_` (`0x1b`, `0xFF_FF`);
  - a **binary** literal `0b` / `0B` followed by a run of binary digits (`0`/`1`)
    with optional interior `_` (`0b101`).

  All three produce the SAME token shape `TokKind::Int { value: u128, raw: String }`
  carrying **both** a numeric `value` and the **verbatim raw source slice**:
  - **value** — the numeric value with `_` separators removed and the radix
    applied: `1_000_000` → `1000000`, `0x1b` → `27`, `0b101` → `5`. **A hex /
    binary literal has the SAME integer value as the equivalent decimal** — the
    radix is a surface spelling only, never a distinct downstream type or node.
  - **raw** — the exact source substring the token spans, prefix and separators
    included: `"1_000_000"`, `"0x1b"`, `"0b101"`. This is the verbatim text used
    for Expr-level round-trip / display fidelity (`ast.md` REQ-6). The #37
    verbatim-text rule is PRESERVED: raw equals `source[span.start ..
    span.start+span.len]`.

  A trailing/leading `_` adjacent to the digit run is **not** part of the
  literal (raw ends at the last digit). The `0x`/`0b` prefix REQUIRES at least
  one radix digit; `0x` with no following hex digit is a `SyntaxError` (REQ-8),
  not a `0` followed by an `x` identifier. No type suffix is lexed (the corpus
  uses `as u64` casts, never `0u64`). The decimal value semantics are the
  v0.1-original behavior and are **UNCHANGED**; hex/binary are NEW spellings of
  the same `Int` token (#91/#92). Derived from Appendix A (`1_000_000`) and §4.4
  ("All conversions explicit").

  > **AMENDMENT (#91/#92, supersedes the prior "a run of ASCII digits" wording).**
  > REQ-3 previously defined an integer literal as ONLY a decimal ASCII-digit run.
  > It now ALSO admits `0x`/`0b` radix prefixes, lexed into the SAME `Int` token
  > with the same integer `value`. No new token kind; the value carries the radix
  > already-applied, so every downstream consumer (lowering, mutation, vacuity)
  > sees a plain `u128` exactly as before — no match-arm churn.

- **REQ-4 (`#[slag(...)]` attribute tokenization):** The attribute introducer
  `#[`, the inner `slag`/`reason`/`owner`/`review` identifiers, `=`, the
  string-literal field values, `,`, `)`, and `]` are lexed as ordinary tokens;
  string literals (double-quoted) carry the decoded string content via the
  escape table (`.design/basis/07-strings.md` REQ-6: `\n`/`\t`/`\r`/`\0`/`\"`/
  `\\` + `\xNN` for the ASCII range `0x00..=0x7F`; a high-byte or malformed
  escape is a STRUCTURED `SyntaxError`, not a silent swallow — high-byte awaits
  the `Vec<u8>` content reshape). The lexer does not validate field names or
  required-field presence — that is the parser/forge (§8). Derived from §8.

- **REQ-5 (comments + whitespace insignificant):** `//` to end-of-line is a
  comment; comments and any run of spaces/tabs/newlines are skipped as
  separators and never produced as tokens. There is no significant whitespace
  and no block-comment form (one comment syntax — pillar 3). Derived from §4.3
  ("no significant whitespace") and the corpus inline comments.

- **REQ-6 (punctuation / multi-char operators, maximal munch):** Operators and
  punctuation are lexed by maximal munch so multi-char tokens win over their
  prefixes: `->`, `=>`, `==`, `!=`, `<=`, `>=`, `&&`, `||`, `::`, `..`, `#[`,
  the **shift operators `<<` and `>>`** (NEW, #92), and the single-char
  `{ } ( ) [ ] , ; : . = < > + - * / % & | ^ !` (the `%`, `^` single-chars are
  NEW, #92). E.g. `<<` is one token, not two `<`; `<=` is one token, not `<`
  then `=`; `&mut` lexes as `&` then the `mut` keyword. Derived from
  `surface-grammar.md` operators.

  > **AMENDMENT (#92).** Adds the tokens `%` (Percent), `^` (Caret), `<<` (Shl),
  > `>>` (Shr). Maximal munch must try `<<`/`>>` BEFORE the single `<`/`>` (and
  > before `<=`/`>=`): `>>` must not split into `>` `>`. The single `&`/`|`/`!`
  > tokens already exist (used by `&&`/`||`/`!=`/`&mut`); #92 gives them an
  > expression-operator meaning (`parser.md`).

- **REQ-7 (spans for diagnostics + addressing):** Every token carries a source
  span (byte offset + length) so the parser can attach spans to AST nodes for
  diagnostics and so per-item recovery can resync to a token boundary. Derived
  from §2 pillar 4 ("Feedback is always crisp") and §4.3.

- **REQ-8 (Result discipline — no panics):** The lexer returns a `Result` /
  diagnostics-bearing structure; an unrecognized character (a stray `@`), a
  malformed radix literal (`0x` with no hex digit, `0b2`), or a malformed char
  literal (REQ-9) produces a `SyntaxError` token-level diagnostic, never a panic
  (`goal.md` R-CODE-2). Derived from R-CODE-2 and the scaffold error
  architecture.

- **REQ-9 (char literals — byte value, byte-level `u8` model — NEW, #91/#92):**
  A char literal is `'` followed by a single ASCII character (or one of the
  recognized `\`-escapes `\n`/`\t`/`\r`/`\0`/`\\`/`\'`/`\xNN` with `NN <=
  0x7F`) followed by `'`. It lexes to the SAME `TokKind::Int { value, raw }`
  token as a numeric literal, where:
  - **value** is the **byte value** of the character — `'A'` → 65, `'\n'` → 10,
    `'\x1b'` → 27. The char model is byte-level `u8` (consistent with the
    07-strings byte model: a string is a run of `u8`, and `\xNN` ASCII-range
    escapes already shipped, 07-strings REQ-6).
  - **raw** is the verbatim source including the quotes — `"'A'"`, `"'\\n'"`.

  A char literal whose content is **multi-byte / non-ASCII** (a codepoint `>=
  0x80`, e.g. `'é'`), or is **empty** (`''`), or is **unterminated**, or whose
  `\xNN` escape is `>= 0x80`, is a STRUCTURED `SyntaxError` (REQ-8) — never a
  silent mis-lex, never a panic. (High-byte/non-ASCII char literals await the
  same `Vec<u8>` content reshape that defers high-byte string escapes.)

  **No new token kind and no new Expr variant**: a char literal IS an integer
  literal at the token AND the AST level (it flows through `Expr::IntLit { value,
  raw }` — `ast.md` REQ-6), so it adds NO exhaustive-match arm anywhere in the
  workspace and NO skill arm. Its type is `u8` (the same primitive the byte
  model uses). Derived from §4.4 (all conversions explicit; the byte char model)
  and 07-strings REQ-6.

## Acceptance criteria

- **AC-1 (corpus lexes clean):** Lexing `conformance/sum.th` and
  `conformance/binary_search.th` produces a token stream with zero error
  diagnostics; inline comments and all whitespace are absent. (REQ-1, REQ-5)
- **AC-2 (`1_000_000` value, UNCHANGED):** The integer literal in `sum.th`
  lexes to a single `Int` token whose **value** is `1000000`. The existing test
  `int_literal_underscores_strip_to_value` continues to assert it. (REQ-3)
- **AC-2b (`1_000_000` raw preserved — #37):** The same token carries `raw ==
  "1_000_000"`. (REQ-3)
- **AC-3 (maximal munch):** In `binary_search.th`, `<=`, `==`, `->`, `=>`,
  `&&`, `::`, `..` each lex as one token; ADDITIONALLY `<<` and `>>` (where they
  appear) lex as one token each and are NOT split into `<` `<` / `>` `>`. (REQ-6)
- **AC-4 (keyword vs identifier):** The reserved keywords lex as keywords;
  `read`/`forall_in`/`sorted`/`len`/`result` lex as identifiers. (REQ-2)
- **AC-5 (slag tokenizes):** `#[slag(reason="x", owner="y", review="required")]`
  lexes to `#[`, ident `slag`, `(`, ident/`=`/string triples, `]`. (REQ-4)
- **AC-6 (no panic on bad char):** A stray `@` yields a `SyntaxError` and the
  scan continues; `0x` with no hex digit, `0b2`, `''`, and `'AB'` each yield a
  `SyntaxError`, never a panic. (REQ-8, REQ-9)
- **AC-7 (hex == binary == decimal value — NEW, #92):** `0x1b` lexes to `Int {
  value: 27, raw: "0x1b" }`; `0b101` to `Int { value: 5, raw: "0b101" }`;
  `0xFF_FF` to value `65535`. Each value EQUALS the value of the equivalent
  decimal literal — verified through the lowering: `forge`/`verus` certifies
  `ens result == 27` for a fn returning `0x1b` at L3 (the GROUNDED probe). (REQ-3)
- **AC-8 (char `'A'` == 65 — NEW, #91/#92):** `'A'` lexes to `Int { value: 65,
  raw: "'A'" }`; `'\n'` to value `10`; `'\x1b'` to value `27`. Verified through
  the lowering: a fn returning `'A'` certifies `ens result == 65` at L3 (the
  GROUNDED probe); a fn returning `'A'` with `ens result == 66` is L0. A
  non-ASCII / multi-byte / empty char literal is a `SyntaxError` (AC-6). (REQ-9)

## Architecture

A single-pass, hand-written scanner over the source `&str` producing
`Vec<Token>`, each `Token { kind, span }`. No significant whitespace means the
scanner skips `[ \t\r\n]+` and `//`-to-EOL between tokens (REQ-5). Maximal munch
(REQ-6) checks the longest operator prefix first at each position — the builder
adds `<<`/`>>` to the two-char-first branch in `lex_punct` and `%`/`^` to the
single-char branch.

Integer literals (REQ-3) dispatch on the prefix at the start of a digit run:
`0x`/`0X` → hex scan, `0b`/`0B` → binary scan, otherwise decimal scan. Each
accumulates the numeric `value` (with the radix applied and `_` skipped) and
captures the verbatim raw as the source slice from the start (INCLUDING the
`0x`/`0b` prefix) to the last radix digit. A `0x`/`0b` with no following radix
digit is a `SyntaxError` (REQ-8). All three radices yield the SAME `TokKind::Int
{ value, raw }` token — the radix is a surface spelling, not a token-kind
distinction.

Char literals (REQ-9) are recognized when the scanner sees `'`: it reads one
character (or a `\`-escape decoded by the SAME escape table the string lexer
uses, `.design/basis/07-strings.md` REQ-6) and a closing `'`, producing
`TokKind::Int { value: <byte>, raw: <"'...'"> }`. A multi-byte/non-ASCII/empty/
unterminated char literal, or a `\xNN >= 0x80`, is a `SyntaxError`. Because a
char literal lexes into the integer-literal token (and thus `Expr::IntLit` at
the AST level), it adds NO new token kind, NO new Expr variant, NO exhaustive-
match arm, and NO skill arm — the cheapest possible literal addition.

> **Note on `'` and lifetimes.** §4.4 removes lifetimes, so `'` is unambiguous:
> it ALWAYS begins a char literal, never a lifetime. There is no `'a`-style
> lifetime token to disambiguate against.

Spans (REQ-7) feed both diagnostics and `parser.md`'s per-item resync. Errors
are `SyntaxError` values (REQ-8), the crate's own error type.

## Verification

`cargo test -p thermite-syntax` over lexer unit fixtures derived from the corpus:
token-stream snapshots for `sum.th`/`binary_search.th` (AC-1, AC-3, AC-4), the
`1_000_000` value + raw assertions (AC-2/AC-2b), a `#[slag]` fixture (AC-5),
stray/malformed-literal negative fixtures (AC-6), and NEW radix/char fixtures
asserting `0x1b`→27 / `0b101`→5 / `'A'`→65 with their raws (AC-7, AC-8). The
END-TO-END value grounding (AC-7/AC-8's L3 claims) is discharged by
`forge`/`thermite-lower` conformance probes that lower a fn returning each
literal with a NON-VACUOUS `ens result == <decimal>` and certify at L3 (the §7
vacuity gate rejects `ens true`); a wrong-code `ens` lands L0. Expected token
streams / values are hand-derived from the grammar, never copied from the
lexer's output (R-CHAR-3).

GROUNDED (real `verus 0.2026.05.24`, this amendment):
- char `'A'` → byte 65: `fn char_a() -> (result: u8) ensures result == 65 { 65 }`
  → `1 verified, 0 errors` (L3); `ensures result == 66` → `0 verified, 1 errors`
  (non-vacuous, L0).
- hex `0x1b` → 27: `fn hex_esc() -> (result: u64) ensures result == 27 { 0x1b }`
  → `1 verified, 0 errors` (L3).
- binary `0b101` → 5: `fn bin_five() -> (result: u64) ensures result == 5
  { 0b101 }` → `1 verified, 0 errors` (L3).

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (token set) | SHIPPED | `enum TokKind` in `lexer.rs` enumerates keywords/ident/int/bool/str/`#[`/punct/eof; consumed by `parser.rs`. The char-literal admission (no new kind) is REQ-9. |
| REQ-2 (keywords closed set) | SHIPPED | `keyword_kind` in `lexer.rs` maps the reserved words; effect/slag names fall through to `Ident`. |
| REQ-3 — VALUE (int literals, `_` stripped) | SHIPPED | `lex_int` in `lexer.rs` strips `_` into `value`; test `int_literal_underscores_strip_to_value`. Consumer: `parse_primary`/pattern-literal in `parser.rs` (`TokKind::Int { value, .. } => Expr::IntLit { .. }`). |
| REQ-3 — RAW (verbatim slice on the token, #37) | SHIPPED | `lex_int` captures `source[i..last_digit]` as `raw`; `TokKind::Int { value, raw }`; test `int_literal_preserves_raw`. Consumer: `parse_primary` in `parser.rs`. |
| REQ-3 — HEX/BINARY (radix spellings, #92) | NOT-STARTED | blocker #92. `lex_int` in `lexer.rs` today scans ONLY decimal (`c.is_ascii_digit()` loop, no `0x`/`0b` prefix dispatch); a `0x1b` source currently lexes as `Int { value: 0 }` then ident `x1b`. Builder adds prefix dispatch in `lex_int`. GROUNDED: `0x1b`==27, `0b101`==5 certify L3. |
| REQ-4 (`#[slag]` tokenization) | SHIPPED | `HashBracket` token + `lex_string` (escape table) in `lexer.rs`; consumed by `parse_slag` in `parser.rs`. |
| REQ-5 (comments + whitespace) | SHIPPED | `skip_trivia` in `lexer.rs` drops `[ \t\r\n]+` and `//`-EOL. |
| REQ-6 (maximal-munch operators) | NOT-STARTED | blocker #92. `lex_punct` in `lexer.rs` today has NO `<<`/`>>`/`%`/`^` arms (the two-char match covers `->`/`=>`/`==`/`!=`/`<=`/`>=`/`&&`/`\|\|`/`::`/`..` only; the single-char match has no `%`/`^`). Builder adds `Shl`/`Shr` (two-char-first) + `Percent`/`Caret` (single-char). |
| REQ-7 (spans) | SHIPPED | every `Token` in `lexer.rs` carries `Span { start, len }`; consumed by parser diagnostics + `address.rs`. |
| REQ-8 (Result discipline) | SHIPPED | `tokenize` in `lexer.rs` returns `(Vec<Token>, Vec<SyntaxError>)`; test `stray_char_is_diagnostic_not_panic`. The NEW malformed-radix/char diagnostics extend this (REQ-3/REQ-9, blocker #92). |
| REQ-9 (char literals → byte `u8` via `Int` token, #91/#92) | NOT-STARTED | blocker #92. `tokenize` in `lexer.rs` has NO `'` branch — a `'A'` source currently lexes `'` as a stray-char `SyntaxError`. Builder adds a char-literal branch producing `TokKind::Int { value: <byte>, raw }` (NO new token kind / Expr variant). GROUNDED: `'A'`==65 certifies L3, `==66` L0. |

## Open questions (for the orchestrator)

- **OQ-1 (effect/slag names not reserved):** `read`/`write`/… and
  `slag`/`reason`/`owner`/`review` lex as identifiers (contextual keywords), not
  reserved words. Recorded; not a blocker.
- **OQ-2 (char literal type is `u8`, #91/#92):** REQ-9 pins a char literal's type
  to `u8` (the byte model). A future need for a wider char type (Unicode
  codepoints) is the same `Vec<u8>`/non-ASCII reshape that defers high-byte
  escapes; in v1 a char IS a byte. Recorded; not a blocker. The validator/lower
  (downstream) must treat the resulting `IntLit` as `u8`-typed in a char context
  — flagged for the builder, owned by #92.
