# Bounded Strings — String / str + literals + core operations (Basis Stage 7)
<!--
tier: 3-component
status: draft
governs: thermite-syntax/src/ast.rs
governs: thermite-syntax/src/parser.rs
governs: thermite-spec/src/validator.rs
governs: thermite-lower/src/lower.rs
thesis-refs:
  - thermite-design.md §4
  - thermite-design.md §4.2
  - thermite-design.md §4.4
  - thermite-design.md §6
-->

## Summary

Stage 7 of the universal verified primitive basis (crosslink **#79**) adds
**text** to the Thermite surface: a bounded **`String`** type (owned, growable),
**string-literal expressions** (`let s = "hello"`), and the v1 core operations —
**`len()`**, **byte access** (`byte_at(i)`, the no-OOB accessor), **bounded
`slice(lo, hi)` / substring**, **`concat` / `+`** (bounded by a `CAP`), and
**equality (`==`)** — lowering to a verified model. This is the biggest practical
unlock of the basis: text is the substrate of editors, parsers, formatters, and
most real programs. A read-only operation (`len`/`byte_at`/`==`) is `pure`; a
string-CONSTRUCTING operation (`concat`, a literal materialized into an owned
`String`) allocates and carries **`fx alloc`** — the Stage-1 `Alloc` effect, the
SAME rule as `Box`/`Vec` construction.

**Cluster C4 (#94) extends this** with the verified `u64`↔`String` conversions the
editor (ANSI cursor coords) and a number formatter / calculator need, on a new PURE
byte-builder: **`push_byte`/`from_byte`** (REQ-7), **`u64_to_string`** with a
GROUNDED gold-standard ROUND-TRIP contract (`parse_le(result@) == n`, REQ-8), and
**`parse_u64`** (REQ-9, PARTIAL / handled-or-loud) — the last blocked on **C7
(#95)**, the built-in `Option`/`Result` + payload-in-contract surface, so REQ-7/REQ-8
ship now under #94 and REQ-9 lands after C7.

**SHIPPED** (commits `b8c3bf7` + `2f5535a`, #79, critic-clean): `string_demo.th`
certifies — `greeting_len`/`first_byte` L3 pure, `join`/`literal_len` L3 alloc,
the no-`req` OOB access → L0. The per-REQ prose below is the original pre-build
feasibility analysis (retained for the grounding record; each row's status cell now
reads SHIPPED). Originally GREENFIELD / FORWARD-LOOKING — every REQ below WAS
NOT-STARTED, tracked under **#79**. Thermite today
**lexes** a string literal — `TokKind::Str(String)` (`thermite-syntax/src/lexer.rs`)
is produced and consumed by `parse_slag`/`parse_attribute` for `#[slag]` /
`#[boundary]` field values — but a string literal is **rejected as an expression**:
`parse_primary` (`thermite-syntax/src/parser.rs`) has no `TokKind::Str` arm, so
`let s = "hello"` dies at the catch-all `_ => Err(self.unexpected("an
expression"))`. There is no `String`/`str` TYPE in `enum Type`
(`thermite-syntax/src/ast.rs` — `Prim`/`Unit`/`Ref`/`Slice`/`Generic`/`Named`/
`Box`/`Vec`), and no string operations anywhere. The GAP is the expression, the
type, and the operations — NOT the lexer.

This stage REUSES, verbatim, the Stage-4 `Vec` machinery and its grounding finding:
a `String` is a bounded run of bytes, the EXACT shape of the verified bounded `Vec`
(`.design/basis/04-collections.md` REQ-5, `TVecU64` over `vstd::vec::Vec`). The
Stage-4 finding — a GENERIC element type failed Verus because `vstd` index moves a
non-`Copy` element, forcing per-element-type monomorphization — **does not bite
here**: the v1 model is `Vec<u8>` (bytes are `Copy`), grounded `6 verified, 0
errors` below.

## Decision: a bounded `String` over `vstd::vec::Vec<u8>` (UTF-8 bytes); `str` is `&String`

### The char model — bytes (`u8`), not codepoints

Three char models were considered, and **all three were GROUNDED with real
`verus 0.2026.05.24`** during authoring (Verification):

- **(a) bytes — a `String` over `vstd::vec::Vec<u8>`.** The model is `Seq<u8>`
  (`v@`); `byte_at(i) -> u8`; the length is the byte length. UTF-8 is the
  encoding; v1 treats a string as its byte sequence (no normalization, no
  codepoint decoding). **GROUNDED `6 verified, 0 errors`** — `well_formed`/`len`/
  `byte_at`/`greeting_len`/bounded `concat`.
- **(b) codepoints — a `String` over `Seq<char>`.** `char` is `Copy`, so `Seq<char>`
  indexes cleanly (GROUNDED `2 verified, 0 errors` — `char_at(s, i) -> char`,
  `s[i]`). `vstd` also exposes a verified `&str` whose view `s@` IS a `Seq<char>`,
  with `unicode_len()`/`get_char(i)` (GROUNDED `2 verified, 0 errors`).
- **(c) a dedicated `char` type fronting `u32`** — a codepoint scalar.

**DECIDED: option (a), bytes (`u8`) over `vstd::vec::Vec<u8>`.** The decisive
reasons:

1. **It is the EXACT Stage-4 `Vec<u8>` machinery, generalized by naming.** A
   `String` is a `TVec` of `u8` with the SAME `well_formed` capacity invariant
   (`len() <= CAP`), the SAME no-OOB exec `get` (here `byte_at`, `req i < len`),
   the SAME capacity-preserving `push`, and the SAME `fx alloc` boundary
   (`.design/basis/04-collections.md` REQ-5). The lowerer reuses the `TVecU64`
   wrapper-emission path almost verbatim, parameterized to `u8`. The Stage-4
   `final(self)`-for-`&mut` finding carries over.
2. **`u8` is `Copy`, so the Stage-4 non-`Copy` generic failure does not recur.**
   `self.data[i]` (a `u8`) copies out of the `vstd::vec::Vec` cleanly — exactly
   why Stage 4 monomorphized to `Vec<u64>` rather than a generic `T`. Bytes are
   the most conservative choice on that axis.
3. **It is the minimum that makes the no-OOB safety claim — the editor's core.**
   The load-bearing v1 contract is `byte_at`'s `req i < len` (no-OOB read);
   GROUNDED that the bounded form verifies and the unguarded form FAILS (`0
   verified, 1 errors`, the L0 demonstration below). Codepoint decoding,
   normalization, and UTF-8 validation add proof surface that the no-OOB / length
   claims do not need.

Options (b)/(c) are not rejected as wrong — `Seq<char>` and `vstd`'s `&str`
verify, and a future codepoint-aware `chars()`/`char_at` is a clean follow-up over
the SAME byte backing (decode on demand). They are deferred because v1's claim is
**bounded text with no-OOB access + length + bounded concat/slice + equality**,
which the byte model discharges with the least new proof surface and maximum reuse
of the SHIPPED `Vec` path. **A "char" in v1 is therefore a `u8` byte**; `byte_at`
returns `u8`. (The naming is `byte_at`, not `char_at`, to be honest that v1
indexes bytes, not Unicode scalar values — `char_at` is reserved for the
codepoint follow-up.)

### `String` vs `str`

**v1 ships `String` (owned, growable) as the first-class type; `str` is the
borrowed view `&String`.** This mirrors the `Vec<T>` / `&[T]` split exactly
(`.design/basis/04-collections.md`: a `Vec<T>` owns a growable run, `&[T]` is the
read-only borrowed view). A `String` parameter passed read-only is taken by
reference (`&String`, the `str`-view role); an owned/constructed/concatenated
`String` is the owning value that carries `fx alloc`. v1 does NOT introduce a
distinct unsized `str` `Type` node — the `Ref { inner: String }` machinery already
in `enum Type` (`thermite-syntax/src/ast.rs`) supplies the borrowed view, the same
way `&[T]` is `Ref` of `Slice`. (A dedicated unsized `str` is a future refinement;
v1's borrowed-view-is-`&String` keeps the type set minimal per §4.4.)

## The §4.2 cage: a string is bounded (`len() <= CAP`)

Per §4.2 the spec sublanguage is deliberately weak and the structures it reasons
over are BOUNDED so the solver stays decidable. A `String` is bounded by design:
`well_formed(&self) -> bool { self.data.len() <= CAP }`, the SAME `CAP` constant
idiom (`1_000_000`) as `conformance/sum.th` (`req xs.len() <= 1_000_000`) and the
Stage-4 `Vec` capacity bound. The cage never sees an unbounded sequence. A property
quantifying over a string's bytes is the EXISTING bounded combinator
`forall_in(s@, |b| …)` (the slice/`Vec` form, now over the byte `Seq` `s@`), whose
closure body is flat; a deeper property is a NAMED `spec fn` — never an anonymous
nested quantifier (§4.2 "composition happens only through named `spec fn`s"). The
validator's caged-flat walk is UNCHANGED: `s@`-indexing, `s.len()`, and `s == t`
are flat built-ins.

## Requirements

### Surface + AST (governs `thermite-syntax/src/ast.rs`, `parser.rs`)

- **REQ-1 (`Expr::StrLit` — a string literal as a primary expression):** The
  surface admits a string literal in expression position: `let s = "hello"`. The
  AST `enum Expr` gains `StrLit(String)` (the decoded literal text, mirroring
  `Expr::IntLit { value, raw }`'s value-carrying shape and `Expr::BoolLit(bool)`);
  `parse_primary` (`thermite-syntax/src/parser.rs`) gains a `TokKind::Str(s) =>
  Ok(Expr::StrLit(s))` arm BEFORE the catch-all `_ => Err(self.unexpected("an
  expression"))`. The literal LEXES today (`TokKind::Str(String)` in
  `thermite-syntax/src/lexer.rs` `enum TokKind`); the only addition is accepting it
  as an `Expr`. The `Str` token's existing `parse_slag`/`parse_attribute` consumers
  are UNCHANGED (a `#[slag(reason = "…")]` field value is still a token-level
  string, not an `Expr`). Derived from §4 (the surface), §4.4 (closed type set,
  one spelling), and the existing `IntLit`/`BoolLit` literal precedent.

- **REQ-2 (`String` type + the operation surface):** The surface admits a `String`
  type and its bounded operations `len()`, `byte_at(i)`, `slice(lo, hi)`,
  `concat`/`+`, and `==`. The AST `enum Type` gains a dedicated `Type::String`
  node (a nullary node — no element-type indirection, unlike `Type::Vec(Box<Type>)`
  — because the element type is fixed to `u8`), mirroring the existing `Type::Vec`
  decision (a dedicated first-class node so the lowerer keys the wrapper +
  capacity-invariant + `fx alloc` emission on the NODE KIND, not a string-name
  match). `parser::parse_type` parses the contextual `String` ident to
  `Type::String` (the SAME contextual-ident dispatch `parse_type` already uses for
  `Vec`/`Box`). Operations are ordinary calls — `s.len()`, `s.byte_at(i)`,
  `s.slice(lo, hi)`, `s.concat(t)` reuse `Expr::MethodCall`; `==` is the existing
  `Expr::Binary { op: BinOp::Eq }`; `+` (concat sugar) is `Expr::Binary { op:
  BinOp::Add }` over two `String`s (lowered to `concat`). No new expression node
  for the operations. The borrowed `str`-view is `Ref { inner: String }` (the
  decision above). Derived from §4.4 (one call syntax, closed built-in interface
  set — `String` is a built-in, not a user type) and the `Type::Vec` precedent.

- **REQ-6 (string-literal escape table — control/hex bytes; crosslink #91 cluster
  1):** A string literal decodes a closed escape set to the BYTES it materializes
  (REQ-2's byte char model). The escape table is: `\n` → 10 (LF), `\t` → 9 (TAB),
  `\r` → 13 (CR), `\0` → 0 (NUL), `\"` → 34 (the quote), `\\` → 92 (the backslash),
  and `\xNN` (exactly two hex digits) → the byte value `0xNN`. The `\r`/`\0`/`\xNN`
  forms are the ANSI/control bytes a terminal editor needs (e.g. `\x1b` → 27, the
  ANSI ESC introducer; `"\x1b".byte_at(0) == 27`). The decoded byte flows through
  the EXISTING `Expr::StrLit` lowering (`thermite-lower::lower` `lower_expr` —
  byte-`push` of `s.as_bytes()`), so no lowering change is needed: a control byte
  is just another byte in the materialized `TString`. **v1 byte-model bound:**
  `\xNN` is admitted for `0x00..=0x7F` (a single UTF-8 byte, byte-faithful); a value
  `>= 0x80` is NOT single-byte-representable in the v1 `String` content (a Rust
  UTF-8 `String`), so it is a STRUCTURED lex diagnostic, NOT a silent
  mis-materialization to two bytes — faithful byte indexing (REQ-2) is the
  load-bearing claim; a high-byte `\xNN` awaits a future `Vec<u8>` string-content
  reshape. An UNKNOWN escape (`\z`) and a MALFORMED `\x` (`\xZZ`, truncated) are
  STRUCTURED `SyntaxError` diagnostics (the v0.1 `lex_string` catch-all
  `other => other as char` SILENTLY swallowed these — the bug this REQ closes),
  never a panic (`.design/syntax/lexer.md` REQ-8; the lexer recovers past the
  string's closing quote). This REQ extends the existing `lex_string` escape table
  in `thermite-syntax/src/lexer.rs` (consistent with `.design/syntax/lexer.md`
  REQ-4, which says a string token carries "the unescaped string content" without
  enumerating the escape SET — this REQ enumerates it). Derived from §4.4 (a closed
  surface), REQ-2 (the byte char model), and the ANSI-editor unblock (#91).


### `u64`↔`String` + the byte-builder (crosslink #94, cluster C4 — GROUNDED)

Cluster C4 adds the verified `u64`↔`String` conversions the **editor** (ANSI
cursor coordinates — `ESC[<row>;<col>H` needs `u64`→decimal text) and a number
formatter / calculator need, plus the **byte-builder** that constructs them in
PURE Thermite (replacing the trusted `os::key_str` glue the editor used). All three
were GROUNDED end-to-end with the real `verus 0.2026.05.24` binary during authoring
(Verification, below) — non-vacuous contracts, the §7 gate's floor cleared, no
`assume`/`admit`/`external_body`. These extend the SHIPPED `TString`-over-
`vstd::vec::Vec<u8>` machinery (REQ-4): `push_byte`/`from_byte` are the verified
byte-construction building block the other two stand on.

- **REQ-7 (`push_byte` / `from_byte` — the verified byte-builder; `fx alloc`):**
  The surface admits byte construction of a `String`: `s.push_byte(b)` (append one
  byte, returning a fresh owned `String`) and `String::from_byte(b)` (build a 1-byte
  `String`). Both are CONSTRUCTING ops (they allocate), so a fn using them carries
  **`fx alloc`** (the Stage-1 `Effect::Alloc`, accepted by effect-subsumption since
  `push`/`Vec::new` are intrinsics — the SAME rule as `concat`/the literal
  materialization, REQ-4). `push_byte` is an `Expr::MethodCall` (`s.push_byte(b)`,
  ADDED to `BUILTIN_METHODS` so its `ens` validates inside the cage); `from_byte`
  is an associated constructor call `String::from_byte(b)` (an `Expr::Call` on the
  `String::from_byte` path — the SAME path-call shape as a free op). The GROUNDED
  contracts (`4 verified, 0 errors`, no cheat tokens):

  ```verus
  // from_byte: a 1-byte String whose sole byte is b.
  pub fn from_byte(b: u8) -> (result: TString)
      ensures result.well_formed(), result.data.len() == 1, result.data@[0] == b,
  { let mut data: Vec<u8> = Vec::new(); data.push(b); TString { data } }

  // push_byte: append b, returning a fresh String (owned construction, NO &mut).
  pub fn push_byte(&self, b: u8) -> (result: TString)
      requires self.well_formed(), self.data.len() < CAP,         // the §4.2 cage
      ensures
          result.well_formed(),
          result.data.len() == self.data.len() + 1,                // length identity
          result.data@[self.data.len() as int] == b,               // the new byte
          forall|j: int| 0 <= j < self.data.len()                  // element frame
              ==> result.data@[j] == self.data@[j],
  { let mut out: Vec<u8> = Vec::new(); let mut i: usize = 0;
    while i < self.data.len()
        invariant i <= self.data.len(), out.len() == i, self.data.len() < CAP,
                  forall|j: int| 0 <= j < i ==> #[trigger] out@[j] == self.data@[j],
        decreases self.data.len() - i,
    { out.push(self.data[i]); i = i + 1; }
    out.push(b); TString { data: out } }
  ```

  The contract is NON-VACUOUS: the length identity, the new-byte placement
  (`result@[old_len] == b`), AND the element frame (every prior byte is preserved)
  are all proved over vstd's verified `Vec::push`. The copy loop carries the standard
  loop invariant (`out.len() == i`, the element-frame `forall`) + `decreases`. v1
  returns a FRESH owned value (the `&self`/owned-result form, NOT a `&mut self`
  in-place mutate — so no `final(self)` is needed; consistent with `concat`'s owned
  result, REQ-4). Derived from §4.1 (the `alloc` effect; row subsumption), §4.2 (the
  cage — `len < CAP`), §6 (L3), the GROUNDED `from_byte`/`push_byte` proofs, and the
  Stage-4 capacity-preserving-`push` precedent (`.design/basis/04-collections.md`
  REQ-5).

- **REQ-8 (`u64_to_string` — decimal formatting with the ROUND-TRIP contract;
  `fx alloc`):** The surface admits `u64`→decimal-`String`: a method
  `n.to_string()` on a `u64` (the chosen spelling — an `Expr::MethodCall` `to_string`
  ADDED to `BUILTIN_METHODS`; it lowers to the generated `u64_to_string` exec fn).
  It is a CONSTRUCTING op (`fx alloc`). The **CONTRACT is the round-trip — the GOLD
  STANDARD, and it PROVES**: the produced byte sequence parses back to exactly `n`.
  GROUNDED (`9 verified, 0 errors`, no cheat tokens):

  ```verus
  // pow10 and the LSB-first digit value (data[0] least significant — the
  // construction order of the divide/mod-by-10 loop). The DISPLAY string reverses
  // to MSB-first; parse_be(reverse(s)) == parse_le(s) is separately proved (4/0).
  pub open spec fn pow10(k: nat) -> nat decreases k
  { if k == 0 { 1 } else { 10 * pow10((k - 1) as nat) } }
  pub open spec fn parse_le(s: Seq<u8>) -> nat decreases s.len()
  { if s.len() == 0 { 0 }
    else { ((s[0] - 48) as nat) + 10 * parse_le(s.subrange(1, s.len() as int)) } }

  // The append lemma (proved by induction, 4/0): appending a digit at the end
  // adds (d-48)*pow10(len) to the value.
  proof fn lemma_parse_push(s: Seq<u8>, d: u8)
      ensures parse_le(s.push(d)) == parse_le(s) + ((d - 48) as nat) * pow10(s.len()),
      decreases s.len(), { /* base: subrange(1,1)==empty, pow10(0)==1;
        step: subrange recurse + pow10(s.len())==10*pow10(t.len()) + nonlinear_arith */ }

  pub fn u64_to_string(n: u64) -> (result: Vec<u8>)
      ensures parse_le(result@) == n as nat,                       // THE ROUND-TRIP
  { let mut data: Vec<u8> = Vec::new(); let mut m: u64 = n;
    proof { /* parse_le([]) + n*pow10(0) == n: pow10(0)==1, n*1==n by nonlinear */ }
    while m > 0
        invariant parse_le(data@) + (m as nat) * pow10(data.len() as nat) == n as nat,
        decreases m,
    { let d: u8 = (m % 10) as u8 + 48u8;                           // the C2 `%`/`/` by 10
      let ghost old_data = data@; let ghost old_m = m as nat;
      let ghost old_len = data.len() as nat;
      data.push(d);
      proof { lemma_parse_push(old_data, d);
              assert((m as nat) == 10 * ((m / 10) as nat) + ((m % 10) as nat)) by(nonlinear_arith);
              assert(pow10((old_len + 1) as nat) == 10 * pow10(old_len)); }
      m = m / 10;
      proof { assert(old_m * pow10(old_len)
          == ((d - 48) as nat) * pow10(old_len) + (m as nat) * pow10((old_len + 1) as nat))
          by(nonlinear_arith)
          requires old_m == 10 * (m as nat) + ((d - 48) as nat),
                   pow10((old_len + 1) as nat) == 10 * pow10(old_len); } }
    data }
  ```

  **THE DIGIT-EXTRACTION LOOP (divide/mod by 10 — the C2 `%`/`/` shipped):** the loop
  invariant is the round-trip *partial accumulator* —
  `parse_le(data@) + m * pow10(data.len()) == n` (the digits built so far plus the
  un-emitted remainder `m`, scaled by `pow10` of the digit count, equal `n`); the
  `decreases m` is the strictly-shrinking remainder (`m / 10 < m` while `m > 0`). The
  per-iteration step is discharged by the `lemma_parse_push` append lemma + a
  `by(nonlinear_arith)` step (`m == 10*(m/10) + m%10`). **This is the strongest
  contract — NOT the floor.** (The HONEST FLOOR — length `>= 1`, `<= 20` (u64 max is
  20 digits, proved via `pow10(20) > u64::MAX` with `reveal_with_fuel`), and
  `all_ascii_digits` (every byte is `'0'..'9'`, 48..=57) — ALSO independently GROUNDED
  `8 verified, 0 errors`; the round-trip SUBSUMES the digit-correctness half of it.)
  The surface emits the human-readable MSB-first decimal (the construction is
  LSB-first; the display form reverses — `parse_be(reverse(s)) == parse_le(s)` proved
  `4 verified, 0 errors`, so the displayed bytes round-trip against a big-endian
  parse). Derived from §3 (transpile to Verus), §4.1 (`alloc`), §6 (L3), the C2 `%`/`/`
  primitives, and the GROUNDED round-trip proof.

- **REQ-9 (`parse_u64` — `String`→`u64`, PARTIAL / handled-or-loud; DEPENDS-ON-C7
  for the surface return type):** The surface admits `String`→`u64` parsing:
  `parse_u64(s) -> Option<u64>` (v1 form) — PARTIAL, with the **handled-or-loud
  teeth**: a non-digit byte, an overflowing value, or an empty string takes the LOUD
  error arm (`None`), NEVER a wrong value or a panic (`.design/basis/06-provenance-
  and-sinks.md` "handled-or-loud, the COMPILE-TIME tooth"; §4.2 partiality). The
  CONTRACT is the round-trip on the success arm: `Some(v)` implies the string is
  all-digits, non-empty, and `parse_be(s) == v`. GROUNDED (`5 verified, 0 errors`, no
  cheat tokens; the verus probe used vstd's `Option` + `result is Some` / `result->
  Some_0`):

  ```verus
  pub open spec fn is_digit(b: u8) -> bool { 48 <= b && b <= 57 }
  pub open spec fn all_digits(s: Seq<u8>) -> bool
  { forall|i: int| 0 <= i < s.len() ==> is_digit(#[trigger] s[i]) }
  pub open spec fn parse_be(s: Seq<u8>) -> nat decreases s.len()       // big-endian (read order)
  { if s.len() == 0 { 0 }
    else { parse_be(s.subrange(0, (s.len()-1) as int)) * 10 + ((s[s.len()-1] - 48) as nat) } }

  pub fn parse_u64(s: &TString) -> (result: Option<u64>)
      requires s.well_formed(),
      ensures result is Some ==> (all_digits(s.data@) && s.data.len() >= 1
                                  && parse_be(s.data@) == result->Some_0 as nat),
  { if s.data.len() == 0 { return None; }                              // empty → LOUD None
    let mut acc: u64 = 0; let mut i: usize = 0;
    while i < s.data.len()
        invariant i <= s.data.len(),
                  all_digits(s.data@.subrange(0, i as int)),
                  parse_be(s.data@.subrange(0, i as int)) == acc as nat,
        decreases s.data.len() - i,
    { let b: u8 = s.data[i];
      if b < 48 || b > 57 { return None; }                            // non-digit → LOUD None
      let digit: u64 = (b - 48) as u64;
      if acc > (u64::MAX - digit) / 10 { return None; }               // overflow → LOUD None
      /* subrange/index ghost glue */
      acc = acc * 10 + digit; i = i + 1; }
    Some(acc) }
  ```

  **THE PARSE LOOP (Horner accumulate — `acc = acc*10 + digit`):** the invariant is
  the BE partial value over the prefix consumed so far (`parse_be(s[0..i]) == acc`)
  plus the all-digits prefix witness; the `decreases s.len() - i`. The three partial
  cases each take the `None` arm BEFORE corrupting `acc`: the overflow guard
  (`acc > (u64::MAX - digit) / 10`) screams BEFORE the `acc*10 + digit` would wrap
  (the C2 partial-`+`/`*` obligation, handled-or-loud). **NON-VACUITY CONFIRMED:** a
  broken `parse_u64` returning `Some(0)` unconditionally FAILS verus (`2 verified, 1
  errors`, "postcondition not satisfied") — the round-trip ens is real teeth, the
  error arm bites.

  **DEPENDS-ON-C7 (the honest dependency — `parse_u64` does NOT ship under #94):**
  the verus probe expresses the contract with vstd's built-in `Option` + the
  `result is Some` discriminant + the `result->Some_0` PAYLOAD PROJECTION in the
  `ensures`. The Thermite surface today has user-defined `enum`s + `Expr::Is` +
  `match` + tuple-variant constructors (`.design/basis/01-adts.md` SHIPPED), but it
  has **NO built-in `Option`/`Result` type AND no enum-PAYLOAD projection in the spec
  sublanguage** — `Expr::Field` is struct-field only; there is no `result->Some_0`
  surface, and a `match`-in-contract over a tuple variant is not admitted by the
  §4.2 cage. Naming `parse_be(s) == <payload>` in an `ens` therefore needs the
  Result/Option-built-in-with-payload-in-contract work — pinned as **C7** in prereq
  **blocker #95**. Per the build-leaves-first discipline (R-DEFER-7, R-LOOP-3):
  **REQ-7 (`push_byte`/`from_byte`) and REQ-8 (`u64_to_string`) ship NOW under #94**
  (they need no new return type); **REQ-9 (`parse_u64`) is NOT-STARTED, blocked on
  C7 (#95)**, then lands. The GROUNDING above PROVES `parse_u64` is feasible the
  instant C7 lands (the contract verifies `5/0`); the gap is purely the surface
  spelling of the partial return type, NOT the verification. Derived from §4.2
  (partiality, the cage), the handled-or-loud principle
  (`.design/basis/06-provenance-and-sinks.md`), the C2 partial-operator obligations,
  and the GROUNDED `parse_u64` proof.

### Validator / the SpecTherm cage (governs `thermite-spec/src/validator.rs`)

- **REQ-3 (string contracts fit the §4.2 cage — flat, no-OOB index, length,
  bounded slice/concat, equality):** The string operation contracts are written
  with FLAT, named predicates inside the cage. The capacity bound (`s.len() <=
  CAP`) is a flat comparison; `byte_at`'s `req i < len` and `ens result ==
  s@[i]` is the no-OOB accessor (the editor's core safety) admitted as a flat
  built-in (`byte_at` ADDED to `BUILTIN_METHODS` in `thermite-spec/src/validator.rs`,
  alongside the Stage-4 `get`, so `ens result == s.byte_at(i)` validates inside the
  cage); `len` returns the length (`ens result == s.len()`); `slice`'s `req lo <= hi
  && hi <= len` is two flat comparisons with `ens result.len() == hi - lo`;
  `concat`'s `req a.len() + b.len() <= CAP, ens result.len() == a.len() + b.len()`
  is a flat length identity; `==` is the existing equality built-in over the byte
  view. `push`/`concat` are EXEC-position (never in a contract). A property over
  the bytes is `forall_in(s@, |b| …)` — the same frozen-trigger combinator as over
  a slice/`Vec`. The caged-flat walk (`.design/spec/spectherm-combinators.md`
  REQ-6) is UNCHANGED. Derived from §4.2 (the cage), the GROUNDED `byte_at`/`concat`
  contracts, and the Stage-4 `BUILTIN_METHODS` precedent.

### Verus lowering (governs `thermite-lower/src/lower.rs`)

- **REQ-4 (`String` → `vstd::vec::Vec<u8>` wrapper; `len`/`byte_at`/`slice`/
  `concat`/`==` → verified ops; the `alloc` effect):** A Thermite `String` lowers
  to a newtype over `vstd::vec::Vec<u8>` — `pub struct TString { pub data: Vec<u8> }`
  — with the capacity bound as `pub open spec fn well_formed(&self) -> bool {
  self.data.len() <= CAP }` threaded through `requires`/`ensures` (the SAME
  data-invariant-threading as Stage-1 `Account::well_formed` and Stage-4 `TVec`).
  `len` lowers to `self.data.len()` (spec `len`/exec); `byte_at` to the no-OOB exec
  accessor `req i < self.data.len(), ens result == self.data@[i as int]` (`{
  self.data[i] }`); `slice(lo, hi)` to a bounded copy with `req lo <= hi && hi <=
  self.data.len(), ens final result.data.len() == hi - lo`; `concat` to the
  bounded two-loop append with `req a.data.len() + b.data.len() <= CAP, ens
  result.well_formed() && result.data.len() == a.data.len() + b.data.len()`; `==`
  to `self.data@ == other.data@` (sequence equality over the byte view). A string
  LITERAL `"hello"` lowers to a constructed `TString` whose bytes are the literal's
  UTF-8 (`{ let mut data = Vec::new(); data.push(104u8); … TString { data } }`)
  with `ens result.data.len() == <byte-length>` — GROUNDED `4 verified, 0 errors`.
  A `fn` CONSTRUCTING a `String` (materializing a literal into an owned value, or
  `concat`-ing) allocates, so it carries `fx alloc` (`Effect::Alloc`,
  `thermite-syntax/src/ast.rs` `enum Effect` `Alloc`, already present) — the SAME
  effect-row rule and subsumption acceptance as Stage-1 `Box` / Stage-4 `Vec`
  construction; a read-only op (`len`/`byte_at`/`==` over a `&String`) is `pure`.
  The lowerer must emit `final(...)` for `&mut`-mutating string-op `ensures` (the
  Stage-4 `final(self)` grounding finding for this `verus` version). Derived from
  §3 (transpile to Verus), §4.1 (the `alloc` effect; row subsumption), §6 (L3), and
  the GROUNDED `TString` proof. **BACKING-AGNOSTIC SURFACE CONTRACT** (the
  #62/Stage-4 resolution applied to strings): the Thermite-surface `String`
  contract names the operation guarantees over the byte view `s@`, NEVER
  `vstd::vec::Vec<u8>` itself; v1 IMPLEMENTS that contract by wrapping
  `vstd::vec::Vec<u8>` (`vstd` is version-pinned alongside Verus). A later decouple
  to a custom byte store, or a codepoint follow-up, swaps the lowering target
  without changing the surface contract or user `.th` code (§6/§9 "the contract is
  the interface").

- **REQ-5 (`LowerError`/`SpecError` extension, no panics):** The new string
  constructs reuse the EXISTING `thermite-lower::LowerError` (an un-lowerable
  string construct → `LowerError::Unsupported`, exactly as the Stage-4 `Vec` path
  reuses it) and the validator's existing reject path (a forbidden method in a
  contract), reusing `thermite_syntax::lexer::Span`. No new variant is expected to
  be required (Stage 4 needed none); if a string-specific failure mode surfaces, it
  is a span-bearing variant on the existing enums. No `unwrap`/`expect`/`panic!` in
  production (R-CODE-2 / R-APG-1). Derived from R-CODE-2 and the existing
  error-enum discipline in `validator.rs` / `lower.rs`.

## The LAYER MAP

The component lands in three layers across three crates, all additively, mirroring
the Stage-1/Stage-4 layer split:

- **7a — surface (`thermite-syntax`).** `enum Expr` gains `StrLit(String)`;
  `parse_primary` accepts `TokKind::Str` as a primary expr (REQ-1). `enum Type`
  gains the nullary `Type::String` node; `parse_type` parses the `String` ident
  (REQ-2). The operations parse as `Expr::MethodCall` (`len`/`byte_at`/`slice`/
  `concat`) and `Expr::Binary` (`==`, `+`) — no new operation node. The
  borrowed-view `str` is `Ref { inner: String }`.
- **7b — validator (`thermite-spec`).** `validate` accepts the string operation
  contracts as FLAT built-ins inside the §4.2 cage (REQ-3): the no-OOB `byte_at`
  accessor (`req i < len`), the `len` identity, the bounded `slice` (`req lo <= hi
  && hi <= len`), the bounded `concat` (`req a.len() + b.len() <= CAP`), and `==`
  over the byte view. `byte_at` joins `BUILTIN_METHODS`. The cage / bounds: a
  `String` is bounded (`well_formed`: `len() <= CAP`); a property over its bytes is
  `forall_in(s@, |b| …)`, never an anonymous nested quantifier.
- **7c — lowering (`thermite-lower`).** `lower` / `lower_expr` gain the `String`
  lowering path (REQ-4): the `TString` newtype over `vstd::vec::Vec<u8>`, the
  `well_formed` capacity predicate, the no-OOB `byte_at` accessor, bounded
  `slice`/`concat`, `==` over `s@`, and the string-literal → byte-`push` sequence.
  A constructing op carries `fx alloc`; a read-only op is `pure`. `final(...)` is
  emitted for `&mut`-mutating `ensures`.
- **C4 — the byte-builder + `u64`↔`String` (#94, layered across 7b/7c).** *7b
  (`thermite-spec`):* `push_byte` and `to_string` ADDED to `BUILTIN_METHODS`
  (alongside `byte_at`/`concat`/`slice`) so their `ens` validates inside the cage
  (REQ-7/REQ-8). *7c (`thermite-lower`):* `emit_string_wrapper` gains the
  `from_byte`/`push_byte` constructor methods (REQ-7); `lower` emits the generated
  `u64_to_string` exec fn + the `pow10`/`parse_le` spec fns + the `lemma_parse_push`
  proof fn (the divide/mod-by-10 digit-extraction loop with its round-trip `inv` +
  `dec m`, REQ-8). All carry `fx alloc` (constructing). **`parse_u64` (REQ-9) is
  NOT in this layer map — it is blocked on C7 (#95):** it needs the built-in
  `Option`/`Result` return + the `result is Some` / payload-in-contract surface that
  the §4.2-cage spec sublanguage does not yet admit; once C7 lands, 7c gains the
  `parse_u64` Horner-accumulate loop + the `parse_be`/`all_digits` spec fns + the
  `None`-arm handled-or-loud error path.

Symbol anchors: `enum Expr` (`StrLit`), `enum Type` (`String`), `enum Effect`
(`Alloc`) in `ast.rs`; `fn parse_primary` / `fn parse_type` in `parser.rs`;
`pub fn validate` + `BUILTIN_METHODS` in `validator.rs`; `pub fn lower` /
`lower_expr` + `emit_string_wrapper` in `lower.rs`. C4 adds (#94): `push_byte`/
`to_string` in `BUILTIN_METHODS` (`validator.rs`); the `from_byte`/`push_byte`
methods in `emit_string_wrapper` + the generated `u64_to_string` / `pow10` /
`parse_le` / `lemma_parse_push` in `lower.rs`.

### The verified Verus form (GROUNDED — the lowering contract, not guesses)

Produced by the real `verus 0.2026.05.24` binary during authoring (Verification).
This is the seed for the `string_demo.th` golden lowering.

```verus
pub spec const CAP: usize = 1_000_000;

pub struct TString { pub data: Vec<u8> }      // wraps vstd::vec::Vec<u8>

impl TString {
    pub open spec fn well_formed(&self) -> bool { self.data.len() <= CAP }
    pub open spec fn len(&self) -> nat { self.data.len() as nat }

    pub fn byte_at(&self, i: usize) -> (result: u8)   // the no-OOB accessor
        requires i < self.data.len(),                 // req i < len — the safety
        ensures result == self.data@[i as int],       // result == s@[i]
    { self.data[i] }
}

pub fn greeting_len(s: &TString) -> (result: usize)   // len, pure
    requires s.well_formed(),
    ensures result == s.data.len(),
{ s.data.len() }

pub fn lit_hello() -> (result: TString)               // a string literal "hello"
    ensures result.well_formed(), result.data.len() == 5,
{
    let mut data: Vec<u8> = Vec::new();
    data.push(104u8); data.push(101u8); data.push(108u8);
    data.push(108u8); data.push(111u8);               // h e l l o
    TString { data }
}

pub fn concat(a: &TString, b: &TString) -> (result: TString)   // bounded concat
    requires a.well_formed(), b.well_formed(),
             a.data.len() + b.data.len() <= CAP,               // the §4.2 cage
    ensures  result.well_formed(),
             result.data.len() == a.data.len() + b.data.len(), // length identity
{
    let mut out: Vec<u8> = Vec::new();
    let mut i: usize = 0;
    while i < a.data.len()
        invariant i <= a.data.len(), out.len() == i,
                  a.data.len() + b.data.len() <= CAP,
        decreases a.data.len() - i,
    { out.push(a.data[i]); i = i + 1; }
    let mut j: usize = 0;
    while j < b.data.len()
        invariant j <= b.data.len(), out.len() == a.data.len() + j,
                  a.data.len() + b.data.len() <= CAP,
        decreases b.data.len() - j,
    { out.push(b.data[j]); j = j + 1; }
    TString { data: out }
}
```

**RECORDED FINDING (the bounded-string stack is end-to-end feasible).** The
`well_formed` capacity invariant (`len() <= CAP`), the no-OOB `byte_at` (`req i <
len`), the length (`greeting_len`), the string-LITERAL lowering (`lit_hello`,
constructed by byte-`push`, `ens len == 5`), and the bounded `concat` (`req a.len()
+ b.len() <= CAP, ens result.len() == a.len() + b.len()`) all verify — the
literal+len+byte_at file `4 verified, 0 errors`, the type+len+byte_at+concat file
`6 verified, 0 errors`. Cheat-token grep (`assume`/`external_body`/`admit`/
`verifier::external`): NONE. **Non-vacuity / the L0 demonstration confirmed:** a
companion `byte_at` dropping the `req i < self.data.len()` correctly FAILS — `0
verified, 1 errors` (`note: failed precondition`) — proving the no-OOB bound is
load-bearing, not vacuous. **`char` model cross-check:** `Seq<char>` indexing and
`vstd`'s `&str` (`s@: Seq<char>`, `unicode_len()`, `get_char(i)`) ALSO verify (`2
verified, 0 errors` each) — the codepoint follow-up is feasible over the same
backing; v1 ships bytes (`u8`, `Copy`, the Stage-4-safe choice). The verified
`TString` over `vstd::vec::Vec<u8>` is the exact wrap-vstd form REQ-4 lowers to;
`vstd`'s verified `Vec::push`/`Vec::index`/`Vec::len` carry the heap proof, the
capacity bound and length identities are the Thermite-level additions.

## Acceptance criteria

The orchestrator authors a NEW corpus program — `conformance/string_demo.th` (a
string literal + `len` + a no-OOB `byte_at` + a bounded `concat`, certifying L3,
with a non-`pure` constructing `fn` exercising `fx alloc`). Its golden lowering
lives at `tests/golden/lower/string_demo.verus.rs`, hand-authored from the
GROUNDED form above and confirmed to pass `verus`. The certificate golden lives at
`conformance/string_demo.cert.json`. The EXACT corpus pinned (the shape the builder
implements against):

```thermite
fn greeting_len(s: &String) -> usize
  req s.len() <= 1_000_000
  ens result == s.len()
  fx  pure
{ s.len() }

fn first_byte(s: &String, i: usize) -> u8
  req i < s.len()
  ens result == s.byte_at(i)
  fx  pure
{ s.byte_at(i) }

fn join(a: &String, b: &String) -> String
  req a.len() + b.len() <= 1_000_000
  ens result.len() == a.len() + b.len()
  fx  alloc
{ a.concat(b) }
```

Plus a crafted negative `conformance/parse` / lower-reject fixture: a `byte_at`
without the `req i < s.len()` bound — its emitted lowering FAILS `verus` (`0
verified, 1 errors`, the L0 demonstration), pinning the no-OOB contract's
non-vacuity (R-DEFER-9).

- **AC-1 (string literal as an expression parses):** Parsing `let s = "hello";`
  yields `Expr::StrLit("hello")` (REQ-1); `parse_primary` accepts `TokKind::Str`;
  the existing `#[slag(reason = "…")]` / `#[boundary]` field-value parsing is
  UNCHANGED (no regression in `tests/sealed_parse.rs` / `tests/boundary_parse.rs`).
  (REQ-1.)

- **AC-2 (bounded `String` len + no-OOB `byte_at` parses, validates, lowers,
  certifies L3/pure):** Parsing `string_demo.th` yields `String`-typed values
  (REQ-2); the validator accepts the `len` identity and the no-OOB `byte_at` (`req
  i < len, ens result == s.byte_at(i)`) inside the §4.2 cage (REQ-3); the lowerer
  emits the `TString` over `vstd::vec::Vec<u8>` + `well_formed` + `len`/`byte_at`
  (REQ-4); running the real `verus` binary on the emitted output exits 0 with `N
  verified, 0 errors`; `forge check` certifies `greeting_len`/`first_byte` L3 with
  `effects: [pure]`, matching `string_demo.cert.json`. (REQ-2, REQ-3, REQ-4.)

- **AC-3 (string literal lowers to bytes + bounded `concat` certifies L3/alloc):**
  The lowerer materializes a string literal into a constructed `TString` (byte
  `push` sequence) and lowers `concat` to the bounded two-loop append with `ens
  result.len() == a.len() + b.len()`; the constructing `fn join` carries `fx alloc`
  and passes effect-subsumption; `verus` certifies L3 (`N verified, 0 errors`);
  `forge check` certifies `join` L3 with `effects: [alloc]`. (REQ-1, REQ-2, REQ-4.)

- **AC-4 (the no-OOB negative FAILS — non-vacuity):** The crafted `byte_at` without
  the `req i < s.len()` bound emits a lowering that FAILS `verus` (`0 verified, 1
  errors`, `failed precondition`) — the no-OOB contract is real, not vacuous
  (R-DEFER-9; GROUNDED). The validator/lowerer surfaces this through the ladder as a
  proof failure (L0/drop), never a lowerer panic (REQ-5). (REQ-3, REQ-4, REQ-5.)

- **AC-5 (existing corpus unchanged — no regression):** `conformance/sum.th`,
  `conformance/binary_search.th`, `conformance/vec_demo.th`, the ADT corpus
  (`bank_account.th`/`shape.th`/`list_sum.th`), and their `.cert.json` /
  `tests/golden/lower/*.verus.rs` goldens are UNCHANGED — they still parse,
  validate, lower byte-stable, and certify L3. The string additions are purely
  additive (one new `Expr` variant, one new `Type` variant, the `String` lowering
  path, `byte_at` in `BUILTIN_METHODS`); no existing node reshapes. Mechanically:
  `cargo test -p thermite-syntax -p thermite-spec -p thermite-lower` and the
  conformance corpus pass with 0 mismatches. (All REQs; Stage 7 must not break the
  kernel.) (REQ-1–REQ-5.)

### C4 acceptance criteria (#94 — `u64`↔`String` + the byte-builder, GROUNDED)

The orchestrator authors a NEW corpus program — `conformance/numfmt_demo.th` (the
byte-builder + `u64_to_string`, certifying L3 with `fx alloc`) — its golden lowering
at `tests/golden/lower/numfmt_demo.verus.rs` (hand-authored from the GROUNDED forms
above, confirmed to pass `verus`) and cert golden at
`conformance/numfmt_demo.cert.json`. (The `parse_u64` corpus entry is authored only
once C7 / #95 lands — AC-8 below is gated.)

- **AC-6 (`push_byte`/`from_byte` build a `String` byte-by-byte, certify L3/alloc):**
  `from_byte(b)` lowers to the 1-byte constructor (`ens len == 1 && data@[0] == b`)
  and `s.push_byte(b)` to the copy-then-append (`ens len == old + 1 && data@[old] == b`
  + the element frame); the constructing fn carries `fx alloc` and passes
  effect-subsumption; the real `verus` binary on the emitted output exits 0 with
  `N verified, 0 errors` (the GROUNDED `4 verified, 0 errors`); `forge check` certifies
  L3 with `effects: [alloc]`. (REQ-7.)

- **AC-7 (`u64_to_string` certifies L3 with the ROUND-TRIP contract):** `n.to_string()`
  lowers to the generated `u64_to_string` (the divide/mod-by-10 digit loop + the
  `pow10`/`parse_le` spec fns + the `lemma_parse_push` append lemma); the emitted
  output passes the real `verus` binary `N verified, 0 errors` with the round-trip
  ens `parse_le(result@) == n` (the GROUNDED `9 verified, 0 errors`); the constructing
  fn carries `fx alloc`; `forge check` certifies L3, `effects: [alloc]`, NON-VACUOUS.
  A crafted broken `u64_to_string` (e.g. dropping the loop step or returning a fixed
  byte) FAILS to verify (R-DEFER-9 non-vacuity). (REQ-8.)

- **AC-8 (`parse_u64` — GATED ON C7/#95 — the error arm BITES):** once C7 lands the
  built-in `Option`/`Result` + payload-in-contract surface, `parse_u64(s)` lowers to
  the Horner accumulate loop with the round-trip ens `result is Some ==> parse_be(s)
  == <payload>`; `verus` certifies L3 (`5 verified, 0 errors`); a non-digit /
  overflowing / empty input takes the `None` arm (not a wrong value, not a panic —
  handled-or-loud); a crafted broken `parse_u64` returning `Some(0)` unconditionally
  FAILS to verify (GROUNDED `2 verified, 1 errors` — non-vacuity, R-DEFER-9). UNTIL
  C7 lands this AC is NOT exercised — REQ-9 is NOT-STARTED. (REQ-9; blocked on #95.)


## Architecture

The component spans three crates, all additively:

- **`thermite-syntax`** — `enum Expr` (`thermite-syntax/src/ast.rs`) gains
  `StrLit(String)` (REQ-1, the value-carrying literal mirroring `IntLit`/
  `BoolLit`); `enum Type` gains the nullary `Type::String` node (REQ-2, a dedicated
  first-class node mirroring `Type::Vec`/`Type::Box` so the lowerer keys on node
  kind). `parser.rs` `parse_primary` gains the `TokKind::Str` arm; `parse_type`
  parses the `String` contextual ident. The lexer is UNCHANGED — `TokKind::Str` is
  already produced (`lexer.rs`); the change is accepting it as an `Expr`. The
  mandatory-contract discipline of `Contract` is unchanged.

- **`thermite-spec`** — `validator.rs` (`pub fn validate`) accepts the string
  operation contracts as FLAT built-ins (REQ-3): `byte_at` joins `BUILTIN_METHODS`
  alongside the Stage-4 `get`; `len`/`slice`/`concat`/`==` are flat length/equality
  built-ins. The caged-flat walk (`.design/spec/spectherm-combinators.md` REQ-6) is
  UNCHANGED: `s@`-indexing, `s.len()`, `s == t`, and `forall_in(s@, …)` are the same
  flat-built-in / frozen-trigger-combinator forms as over a slice/`Vec`. A
  string is bounded (`well_formed`: `len() <= CAP`) so the §4.2 cage never sees an
  unbounded sequence.

- **`thermite-lower`** — `lower.rs` (`pub fn lower` / `lower_expr`) gains the
  `String` lowering path (REQ-4): the `TString` newtype over `vstd::vec::Vec<u8>`
  (reusing the Stage-4 `TVec` wrapper-emission path, parameterized to `u8`), the
  `well_formed` capacity predicate, the no-OOB `byte_at`, bounded `slice`/`concat`,
  `==` over `s@`, and the string-literal → byte-`push` materialization. The two
  lowering contexts (exec vs spec, `.design/lower/verus-lowering.md`) extend:
  `s.concat(t)` / a constructed literal are exec position (carry `fx alloc`);
  `s.byte_at(i)` / `s.len()` / `s@[i]` are spec/read position (`pure`). `final(...)`
  is emitted for `&mut`-mutating `ensures` (the Stage-4 finding).

## Dependency hooks (for the rest of the basis)

- **Stage 4 (collections — `Vec`/`alloc` — CONSUMED):** Stage 7 IS Stage-4's
  bounded `Vec` machinery applied to `u8`. The `TVec` wrapper-emission, the
  `well_formed` capacity invariant, the no-OOB exec accessor, the
  capacity-preserving `push`, the `fx alloc` effect-row rule + subsumption
  acceptance, and the `final(self)`-for-`&mut` grounding finding
  (`.design/basis/04-collections.md` REQ-5, SHIPPED #73) are REUSED. The Stage-4
  non-`Copy` generic finding is the reason v1 picks `u8` (Copy) bytes.
- **Stage 1 (ADTs — `Box`/`alloc`, type invariants — CONSUMED):** the `fx alloc`
  effect for a constructing op and the `well_formed`-threading mechanism reuse the
  Stage-1 keystone (`.design/basis/01-adts.md` REQ-3/REQ-8).
- **A codepoint follow-up (FUTURE, OUT of v1):** `chars()` / `char_at(i) -> char`
  (decode UTF-8 on demand over the same byte backing), `format!`/interpolation,
  full UTF-8 validation, normalization, and regex are explicitly OUT. The
  GROUNDED `Seq<char>` / `vstd` `&str` cross-check shows the codepoint path is
  feasible over the byte model; the backing-agnostic surface contract (REQ-4) keeps
  the migration clean.

## Verification

- **Mandatory Verus grounding (DONE during authoring — real `verus
  0.2026.05.24`).** Two `verus!{}` files were run:
  - The type + read ops + bounded concat (`TString` over `vstd::vec::Vec<u8>`:
    `well_formed`/`len`/`byte_at`/`greeting_len`/`concat`):
    ```
    verus --no-cheating /tmp/strchk.rs
    verification results:: 6 verified, 0 errors
    ```
  - The string-literal lowering + no-OOB safe accessor (`lit_hello` constructed by
    byte-`push` with `ens len == 5`, plus the bounded `byte_at`):
    ```
    verus --no-cheating /tmp/strlit.rs
    verification results:: 4 verified, 0 errors
    ```
  Cheat-token grep (`assume`/`external_body`/`admit`/`verifier::external`) over both
  files: NONE. **Non-vacuity / L0 confirmed:** a companion `byte_at` dropping the
  `req i < self.data.len()` correctly FAILS — `0 verified, 1 errors` (`failed
  precondition`). **char model cross-check:** `Seq<char>` indexing and `vstd`'s
  `&str` (`s@: Seq<char>`, `unicode_len()`, `get_char(i)`) verify `2 verified, 0
  errors` each — the codepoint follow-up is feasible; v1 ships bytes. This proves
  the bounded-`String` + capacity-invariant + no-OOB-`byte_at` + length +
  bounded-`concat` + string-literal-lowering stack is Verus-feasible end to end.
  (Scratch cleaned per #53 — no stray `*.rlib`/`*.d` left.)

- **C4 Verus grounding (DONE during authoring — real `verus 0.2026.05.24`, #94).**
  Five `verus!{}` probes were run; ALL cheat-free (grep `assume`/`admit`/
  `external_body`/`verifier::external`: NONE):
  - `from_byte` + `push_byte` (byte-builder over `vstd::vec::Vec<u8>`, the
    copy-then-append loop with the element-frame invariant): `4 verified, 0 errors`.
  - `u64_to_string` — **the GOLD-STANDARD round-trip** (`ens parse_le(result@) == n`),
    the divide/mod-by-10 digit loop with invariant
    `parse_le(data@) + m*pow10(data.len()) == n` + `decreases m` + the
    `lemma_parse_push` append lemma (proved by induction) + `by(nonlinear_arith)`
    steps: `9 verified, 0 errors`.
  - `u64_to_string` — the honest FLOOR (`len >= 1`, `<= 20` via `pow10(20) > u64::MAX`
    with `reveal_with_fuel`, `all_ascii_digits`): `8 verified, 0 errors` (independently;
    the round-trip subsumes its digit-correctness half).
  - `parse_be(reverse(s)) == parse_le(s)` (the display-form bridge — the loop builds
    LSB-first, the displayed decimal reverses to MSB-first): `4 verified, 0 errors`.
  - `parse_u64 -> Option<u64>` (the Horner-accumulate loop, the round-trip success
    ens `result is Some ==> parse_be(s) == result->Some_0`, the non-digit/overflow/
    empty `None` arms): `5 verified, 0 errors`. **Non-vacuity:** a broken `parse_u64`
    returning `Some(0)` unconditionally FAILS — `2 verified, 1 errors` (postcondition
    not satisfied) — the error arm bites. `parse_u64`'s SURFACE return type is the C7
    dependency (#95); the VERIFICATION is proved feasible here.
  This proves the C4 stack (byte-builder + the gold-standard `u64`→`String`
  round-trip + the partial `String`→`u64` parse) is Verus-feasible end to end; the
  digit-extraction and Horner loops both verify with a real invariant + `decreases`.
  (Scratch cleaned per #53 — no stray `*.rs`/`*.rlib`/`*.d` left.)

- **Toolchain path grounded:** `./target/debug/forge check conformance/vec_demo.th`
  exits 0 emitting L3 certs with `effects: [pure]` (read-only `checked_get`) and
  `effects: [alloc]` (constructing `push_one`) — the exact cert shape
  `string_demo`'s `greeting_len`/`first_byte` (pure) and `join` (alloc) will match
  (`conformance/string_demo.cert.json`).

- **AC-1–AC-4:** `cargo test -p thermite-syntax -p thermite-spec -p thermite-lower`,
  plus a harness that shells the real `verus` binary on the emitted lowering of
  `string_demo.th` and asserts exit 0 + `N verified, 0 errors` (R-CODE-4:
  subprocess status checked, never swallowed), plus `forge check` matching
  `conformance/string_demo.cert.json`. The no-OOB negative must FAIL to verify
  (R-DEFER-9).
- **AC-5:** the existing `tests/golden/lower/*.verus.rs` and `*.cert.json`
  assertions stay green (no regression); the existing `#[slag]`/`#[boundary]`
  string-token parsing stays green.

Gauntlet (R-DEFER-6, per crate): `cargo test -p <crate>`, `cargo clippy -p
<crate> --all-targets -- -D warnings`, `cargo fmt --check`.

## Routes to add (orchestrator)

This stage adds NEW concerns to files that already carry routes; the orchestrator
adds these routes to `tooling/spec-routes.toml` pointing at THIS doc (a file may
carry multiple governing docs — the `lower.rs` precedent):

```
[[route]]  crate_pattern = "thermite-syntax/src/ast.rs"        design = ".design/basis/07-strings.md"   reference = ["conformance/string_demo.th"]
[[route]]  crate_pattern = "thermite-syntax/src/parser.rs"     design = ".design/basis/07-strings.md"   reference = ["conformance/string_demo.th"]
[[route]]  crate_pattern = "thermite-spec/src/validator.rs"    design = ".design/basis/07-strings.md"   reference = ["conformance/string_demo.th"]
[[route]]  crate_pattern = "thermite-lower/src/lower.rs"       design = ".design/basis/07-strings.md"   reference = ["tests/golden/lower/string_demo.verus.rs"]
```

The corpus program `conformance/string_demo.th`, its `.cert.json` golden, and the
`tests/golden/lower/string_demo.verus.rs` lowering are authored by the orchestrator
from this doc (and the GROUNDED `TString` seed) before the builder runs (R-CHAR-3).

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (`Expr::StrLit` — string literal as a primary expr) | SHIPPED | #79 Stage 7. `enum Expr` (`thermite-syntax/src/ast.rs`) has `IntLit`/`BoolLit` but no `StrLit`; `parse_primary` (`thermite-syntax/src/parser.rs`) has no `TokKind::Str` arm — `let s = "hello"` dies at the catch-all `_ => Err(self.unexpected("an expression"))`. The literal LEXES (`TokKind::Str(String)` in `lexer.rs`, consumed by `parse_slag`/`parse_attribute` only). GROUNDED-feasible (the literal lowers + verifies, `lit_hello` `4 verified, 0 errors`); not yet accepted as an `Expr`. |
| REQ-2 (`String` type + len/byte_at/slice/concat/`==` surface) | SHIPPED | #79 Stage 7. `enum Type` (`ast.rs`) has `Prim`/`Slice`/`Vec`/`Box`/`Named`/`Generic` but no `String` node; `parse_type` has no `String` contextual-ident dispatch. The operations would reuse `Expr::MethodCall`/`Expr::Binary` (no new node), but no `String`-typed value parses today. Char model DECIDED (bytes/`u8`), `String`-owned / `str`-as-`&String` DECIDED; not implemented. |
| REQ-3 (string contracts fit the §4.2 cage — no-OOB index, length, bounded slice/concat, `==`) | SHIPPED | #79 Stage 7. `byte_at` is not in `BUILTIN_METHODS` (`thermite-spec/src/validator.rs`); no string contract validates today. The accept path it reuses (the Stage-4 `get` no-OOB accessor in `BUILTIN_METHODS`, the caged-flat walk) is SHIPPED, so the validator extension is mechanical. GROUNDED-feasible (the no-OOB `byte_at` certifies, the unguarded form FAILS `0 verified, 1 errors`); not implemented. |
| REQ-4 (`String` → `vstd::vec::Vec<u8>` wrapper; len/byte_at/slice/concat/`==`; `fx alloc`; literal lowering; BACKING-AGNOSTIC surface) | SHIPPED | #79 Stage 7. `lower.rs` has no `String`/`Type::String` lowering and no string-literal materialization. The wrap-vstd path it reuses (the Stage-4 `TVec` over `vstd::vec::Vec`, the `well_formed` predicate, the no-OOB exec accessor, `fx alloc` subsumption, the `final(self)` finding) is SHIPPED (#73), so the extension to `Vec<u8>` is mechanical. GROUNDED-feasible (`TString` over `vstd::vec::Vec<u8>`: `well_formed`/`len`/`byte_at`/`concat` `6 verified, 0 errors`; literal `lit_hello` `4 verified, 0 errors`); not implemented. |
| REQ-5 (`LowerError`/`SpecError` extension, no panics) | SHIPPED | #79 Stage 7. No string lowering exists yet to surface a failure mode; the existing `LowerError::Unsupported` / validator reject path is expected to suffice (Stage 4 needed no new variant). No code added — NOT-STARTED until the string path lands. No `unwrap`/`expect`/`panic!` will be introduced (R-CODE-2 / R-APG-1). |
| REQ-6 (string-literal escape table — control/hex bytes, #91 cluster 1) | SHIPPED | #91. `lex_string` in `thermite-syntax/src/lexer.rs` decodes `\n`/`\t`/`\r`/`\0`/`\"`/`\\` to their bytes and `\xNN` (two hex digits, `0x00..=0x7F`) to the byte value via `parse_hex_escape`/`hex_digit`; an unknown/malformed/high-byte escape is a STRUCTURED `SyntaxError::StrayChar` (recovering past the close-quote via `resume_past_string`), never the old silent `other as char` swallow and never a panic. Consumer: the decoded byte flows through the EXISTING `Expr::StrLit` lowering (`thermite-lower::lower` `lower_expr`, byte-`push` of `s.as_bytes()`) — no new variant. Verified: `thermite-syntax/tests/string_escapes.rs` (9 decode/diagnostic tests) + `forge/tests/literal_layer.rs` grounds `"\x1b".byte_at(0) == 27` / `\r` == 13 / `\0` == 0 at L3 against real verus (non-vacuous, §7 battery), wrong-code NOT L3. |

| REQ-7 (`push_byte`/`from_byte` — verified byte-builder; `fx alloc`) | SHIPPED | #94 cluster C4. `push_byte` ADDED to `BUILTIN_METHODS` (`thermite-spec/src/validator.rs`, now `["len","get","byte_at","concat","slice","push_byte","to_string"]`); `from_byte`/`push_byte` methods ADDED to `emit_string_wrapper` (`thermite-lower/src/lower.rs`) — `from_byte(b: u64) -> TString` (`ens len==1 && data@[0]==b as u8`) + `push_byte(&self, b: u64) -> TString` (`req len < CAP`, `ens len==old+1 && data@[old]==b as u8` + the element frame `forall|j| 0 <= j < old ==> result@[j]==self@[j]`); the surface byte is `u64` (the `byte_at -> u64` zero-extension convention), cast to the `u8` backing. `String::from_byte(b)` (a path call) lowers to `TString::from_byte(b)` (the `lower_expr` `Path` arm `String::`→`TString::` rewrite); `fx alloc` via effect-subsumption (the REQ-4 `concat` rule). Owned-result form (no `&mut`/`final`). GROUNDED `verified, 0 errors` (reuses vstd's verified `Vec::push`). Consumer: `lower`. Verified: `forge/tests/string_format_conformance.rs::ac6_byte_builder_certifies_l3_alloc` (real verus L3 / `effects: [alloc]`). |
| REQ-8 (`u64_to_string` — decimal formatting, ROUND-TRIP contract; `fx alloc`) | SHIPPED | #94 cluster C4. `to_string` ADDED to `BUILTIN_METHODS`; the GENERATED `parse_le`/`pow10` seeded into `Validator::spec_fns` (`GENERATED_SPEC_FNS`) so `ens parse_le(result) == n` validates inside the §4.2 cage. `lower.rs::emit_numfmt_defs` emits the `pow10`/`parse_le` spec fns + the `lemma_parse_push` append lemma + the `u64_to_string(n) -> TString` exec fn (the divide/mod-by-10 digit loop with the round-trip invariant `parse_le(data@) + m*pow10(data.len()) == n` + `decreases m` + `by(nonlinear_arith)` + `=~=` extensionality), materialized when the program uses `n.to_string()` / names `parse_le` (`program_uses_numfmt`). `n.to_string()` lowers to `u64_to_string(n)` (`lower_expr` MethodCall exec arm); `parse_le(result)` lowers to `parse_le(result.data@)` (`lower_spec_arg` String byte-view rule) with the `as nat` coercion (`nat_fns += parse_le`). The round-trip `ens parse_le(result.data@) == n as nat` is the GOLD STANDARD — GROUNDED `16 verified, 0 errors` end-to-end (the wrapper + numfmt + the surface `show`), no `assume`/`external_body`/`admit`; a WRONG digit (`+49` instead of `+48`) FAILS verus `15 verified, 1 errors` (non-vacuous, R-DEFER-9). v1 builds LSB-first (the proven form); the human MSB-first display reversal is the design's noted `parse_be(reverse(s)) == parse_le(s)` bridge (follow-up). Consumer: `lower`. Verified: `forge/tests/string_format_conformance.rs` — `ac7_to_string_round_trip_certifies_l3` (L3, mutants 1/1, non-vacuous), `ac7_overclaimed_round_trip_is_rejected` (an overclaimed `== n+1` REJECTED, never L3), `ac7_formatter_builds_and_prints_decimal` (the formatter builds + RUNS + prints the decimal digits of 42). |
| REQ-9 (`parse_u64` — `String`→`u64`, PARTIAL / handled-or-loud) | NOT-STARTED | #94 cluster C4, **DEPENDS-ON-C7 — prereq blocker #95**. The Thermite surface has user `enum`s + `Expr::Is` + `match` + tuple-variant constructors (`.design/basis/01-adts.md` SHIPPED) but NO built-in `Option`/`Result` AND no enum-payload projection in the §4.2-cage spec sublanguage (`Expr::Field` is struct-only; no `result->Some_0`; no `match`-in-contract over a tuple variant) — so the success-arm round-trip `ens result is Some ==> parse_be(s) == <payload>` cannot be spelled on the surface today. GROUNDED-feasible (real `verus`, vstd `Option`: the Horner-accumulate parse loop with the BE partial-value invariant + the overflow/non-digit/empty `None` arms, `5 verified, 0 errors`; the broken `Some(0)` FAILS `2 verified, 1 errors` — the error arm bites, non-vacuous). REQ-7/REQ-8 ship now under #94; REQ-9 lands after C7 (#95). Builder's layer (post-C7): the C7 built-in `Option`/`Result` return type + the `result is Some`/payload-in-contract surface, THEN `parse_u64` in `lower.rs` (the accumulate loop) + the `parse_be`/`all_digits` spec fns + the `None`-arm error path (handled-or-loud, no panic — R-CODE-2). |

## Open questions (for the orchestrator before the builder runs)

- **OQ-1 (least-confident: the char model — bytes vs. codepoints).** v1 DECIDED
  bytes (`u8` over `vstd::vec::Vec<u8>`) — `Copy` (dodges the Stage-4 non-`Copy`
  failure), maximum reuse of the SHIPPED `Vec` path, minimum proof surface for the
  no-OOB/length claims. The residual risk: a byte string is NOT codepoint-aware —
  `byte_at(i)` returns a UTF-8 byte, not a Unicode scalar, and `slice(lo, hi)` can
  cut a multi-byte codepoint (v1 does NOT validate UTF-8 boundaries). This is
  HONEST about v1's claim (bounded bytes with no-OOB access), and the GROUNDED
  `Seq<char>` / `vstd` `&str` cross-check (both `2 verified, 0 errors`) shows the
  codepoint follow-up is feasible over the same backing. The decision is named
  `byte_at` (not `char_at`) precisely so the surface does not over-claim Unicode
  awareness. RECOMMEND bytes for v1; `char_at`/`chars()` is a follow-up. The
  least-confident axis is whether the v1 "string" should be codepoint-aware from the
  start (option (b), `Seq<char>` / `vstd` `&str`) — `vstd`'s `&str` verifies and
  would give true Unicode `len`/index, at the cost of leaving the SHIPPED `Vec<u8>`
  reuse path. Pinned bytes; flagged for the orchestrator's call.

- **OQ-2 (the string-literal lowering — byte-`push` sequence vs. a `vstd` literal
  constructor).** v1 lowers `"hello"` to a constructed `TString` built by a
  byte-`push` sequence (GROUNDED `lit_hello`, `4 verified, 0 errors`). This is the
  most conservative, fully-grounded form, but it makes a literal a CONSTRUCTING op
  (`fx alloc`) — a `let s = "hello"` in a `fx pure` fn would NOT type-check unless
  the literal is treated as a `&str`-view constant (no allocation). The open
  question: is a bare string literal in a read-only position a borrowed `&String`
  constant (`pure`, no alloc — the common editor case `s == "needle"`), or always
  an owned construction (`fx alloc`)? RECOMMEND: a literal compared/read (`s ==
  "x"`, passed as `&String`) is a `pure` `&str`-view constant; a literal BOUND to an
  owned `String` (`let s: String = "x"`) or concatenated is `fx alloc`. This is the
  second least-confident decision (the GROUNDED form proves the owned-construction
  path; the `pure` view-constant path is designed-but-needs-grounding against a
  `vstd` `&str` literal). Not a blocker; flagged.

- **OQ-3 (`String` as a dedicated nullary `Type` node — confirmed):** the byte
  element type is FIXED (`u8`), so `Type::String` is nullary (no `Box<Type>` arg),
  unlike `Type::Vec(Box<Type>)`. This is the clearest shape (the lowerer keys the
  `Vec<u8>` wrapper on the node kind). RECOMMEND the dedicated nullary node;
  consistent with the `Type::Vec`/`Type::Box` dedicated-node precedent (OQ-2 of
  Stage 4, RESOLVED). Not a blocker; pinned for the builder.

- **OQ-4 (`slice` ownership — owned copy vs. borrowed view):** `slice(lo, hi)` can
  return an owned `String` (a bounded byte copy, `fx alloc`) or a borrowed `&str`
  view into the source (`pure`, no copy). v1 RECOMMENDS the owned-copy form
  (`fx alloc`, `ens result.len() == hi - lo`) — it is the §4.2-cage-clean bounded
  construction and reuses the `concat` loop machinery; a zero-copy borrowed slice
  needs region/lifetime reasoning §4.4 defers. Not a blocker; flagged so the
  builder does not over-scope `slice` to a borrowed view.
```