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

This doc is GREENFIELD / FORWARD-LOOKING. **Every REQ below is NOT-STARTED**,
tracked under **#79** (no separate blocker is filed — #79 owns this stage; a gap
needing an independent blocker is noted with a fresh `#`). Thermite today
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

Symbol anchors: `enum Expr` (`StrLit`), `enum Type` (`String`), `enum Effect`
(`Alloc`) in `ast.rs`; `fn parse_primary` / `fn parse_type` in `parser.rs`;
`pub fn validate` + `BUILTIN_METHODS` in `validator.rs`; `pub fn lower` /
`lower_expr` in `lower.rs`.

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
| REQ-1 (`Expr::StrLit` — string literal as a primary expr) | NOT-STARTED | #79 Stage 7. `enum Expr` (`thermite-syntax/src/ast.rs`) has `IntLit`/`BoolLit` but no `StrLit`; `parse_primary` (`thermite-syntax/src/parser.rs`) has no `TokKind::Str` arm — `let s = "hello"` dies at the catch-all `_ => Err(self.unexpected("an expression"))`. The literal LEXES (`TokKind::Str(String)` in `lexer.rs`, consumed by `parse_slag`/`parse_attribute` only). GROUNDED-feasible (the literal lowers + verifies, `lit_hello` `4 verified, 0 errors`); not yet accepted as an `Expr`. |
| REQ-2 (`String` type + len/byte_at/slice/concat/`==` surface) | NOT-STARTED | #79 Stage 7. `enum Type` (`ast.rs`) has `Prim`/`Slice`/`Vec`/`Box`/`Named`/`Generic` but no `String` node; `parse_type` has no `String` contextual-ident dispatch. The operations would reuse `Expr::MethodCall`/`Expr::Binary` (no new node), but no `String`-typed value parses today. Char model DECIDED (bytes/`u8`), `String`-owned / `str`-as-`&String` DECIDED; not implemented. |
| REQ-3 (string contracts fit the §4.2 cage — no-OOB index, length, bounded slice/concat, `==`) | NOT-STARTED | #79 Stage 7. `byte_at` is not in `BUILTIN_METHODS` (`thermite-spec/src/validator.rs`); no string contract validates today. The accept path it reuses (the Stage-4 `get` no-OOB accessor in `BUILTIN_METHODS`, the caged-flat walk) is SHIPPED, so the validator extension is mechanical. GROUNDED-feasible (the no-OOB `byte_at` certifies, the unguarded form FAILS `0 verified, 1 errors`); not implemented. |
| REQ-4 (`String` → `vstd::vec::Vec<u8>` wrapper; len/byte_at/slice/concat/`==`; `fx alloc`; literal lowering; BACKING-AGNOSTIC surface) | NOT-STARTED | #79 Stage 7. `lower.rs` has no `String`/`Type::String` lowering and no string-literal materialization. The wrap-vstd path it reuses (the Stage-4 `TVec` over `vstd::vec::Vec`, the `well_formed` predicate, the no-OOB exec accessor, `fx alloc` subsumption, the `final(self)` finding) is SHIPPED (#73), so the extension to `Vec<u8>` is mechanical. GROUNDED-feasible (`TString` over `vstd::vec::Vec<u8>`: `well_formed`/`len`/`byte_at`/`concat` `6 verified, 0 errors`; literal `lit_hello` `4 verified, 0 errors`); not implemented. |
| REQ-5 (`LowerError`/`SpecError` extension, no panics) | NOT-STARTED | #79 Stage 7. No string lowering exists yet to surface a failure mode; the existing `LowerError::Unsupported` / validator reject path is expected to suffice (Stage 4 needed no new variant). No code added — NOT-STARTED until the string path lands. No `unwrap`/`expect`/`panic!` will be introduced (R-CODE-2 / R-APG-1). |

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