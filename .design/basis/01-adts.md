# Algebraic Data Types + Pattern Matching (Basis Stage 1)
<!--
tier: 3-component
status: draft
governs: thermite-syntax/src/ast.rs
governs: thermite-syntax/src/parser.rs
governs: thermite-spec/src/validator.rs
governs: thermite-lower/src/lower.rs
thesis-refs:
  - thermite-design.md §3
  - thermite-design.md §4
  - thermite-design.md §4.1
  - thermite-design.md §4.2
  - thermite-design.md §4.4
  - thermite-design.md §6
-->

## Summary

Stage 1 of the universal verified primitive basis (crosslink epic **#62**) adds
**product types (`struct`)**, **sum types (`enum`)**, **recursive types**, and
**`match`** to the Thermite surface — the DATA half of "program anything,
verified." Product + sum + recursion is, by construction, every finite algebraic
data type; joined with the structural recursion Stage 2 layers on top, that is
total functional completeness. Verus models ADTs + structural induction natively
(`decreases` orders datatype values directly — verified during authoring, see
Verification), so expressiveness and provability arrive together.

This is the **keystone** stage: the data representation pinned here is consumed
by Stage 2 (recursion schemes fold over these recursive ADTs), Stage 4
(collections reuse the `Box`/`alloc` heap primitive established here), and
Stage 5 (composition reasons over ADT contracts). The recursive-type
representation decision (below) is therefore load-bearing for the whole buildout.

This doc is GREENFIELD / FORWARD-LOOKING. Thermite v0.1 today admits exactly
`u32`/`u64`/`usize`, `bool`, and `&[T]` (`thermite-syntax/src/ast.rs` `enum
PrimType` + `enum Type`); there is no `struct`, `enum`, `match` in item position,
or heap. **Every REQ below is NOT-STARTED**, tracked under epic **#62** (no
separate blocker is filed — #62 owns this stage; gaps that need an independent
blocker are noted with a fresh `#`).

## The unifying principle — handled-or-loud (the COMPILE-TIME tooth)

This stage instantiates, in DATA, the unifying law of the whole toolchain
(crosslink **#62** design-refinement pass): **for every outcome a program MODELS,
it must either (a) HANDLE it — a path whose correctness is *proven* (L3) or
*checked* (L1) — or (b) SCREAM — an explicit, typed, greppable refusal. Silently
doing the wrong thing is structurally impossible.** Verification's claim is not
"nothing fails"; it is "every modeled outcome resolves to *correct* or *loud*, and
the unmodeled remainder is enumerated in the manifest." The law has three teeth,
escalating in loudness:

- **Compile-time scream (loudest) — THIS stage.** The exhaustive `match` (REQ-5)
  is the COMPILE-TIME enforcement: a missed variant/outcome is REJECTED by the
  validator *before the program ships* (`SpecError::NonExhaustiveMatch`). Either
  every variant is handled, or an explicit `Wildcard` catch-arm screams. There is
  no third "fell through silently" path — a non-exhaustive `match` does not
  compile. This is the tooth `01-adts.md` owns.
- **Runtime scream (always-live) — the §6 ladder's L1 rung.** Every contract is
  L1-checked in EVERY build profile (§6: "Violations detected at the call site, in
  every build profile"); a violated `[ens]`/`[req]` aborts with the diagnostic
  (exit 101), never returns a wrong value. An ADT-shaped contract (`well_formed`,
  `is Variant`, a `match`-postcondition) rides this rung exactly as a scalar one,
  so even a `#[slag]`/boundary ADT producer screams at runtime if it lies (§6 L1 +
  `slag` flag). The data half this stage pins is therefore handled-or-loud at BOTH
  compile time (exhaustiveness) and run time (the L1 contract).
- **Kill scream (effect-confinement) — the #57 seccomp sandbox.** Code exceeding
  its declared `fx` is `SIGSYS`-killed at the syscall boundary
  (`.design/forge/runtime-sandbox.md`). The `Alloc` effect this stage first
  exercises (REQ-3) participates: a constructing `fn` is confined to its declared
  `alloc` allowlist.

The fiat/verified line is a KNOB, not a fixed frontier: whatever outcome you NAME
(model as a variant), you must handle-or-scream it; whatever you leave UNMODELED is
the enumerated trusted remainder the manifest reports. An ADT models its outcome
set as its variant set, so "exhaustive `match`" is precisely "every modeled
outcome is handled-or-loud" made mechanical at validation time. This is why
exhaustiveness is mandatory (REQ-5), not a convenience — it is the data-side
enforcement of the toolchain's core safety law.

## Decision: the recursive-type representation — `Box<T>` on the `Alloc` effect

A recursive `enum` needs indirection at the recursive occurrence (Rust/Verus
reject an infinitely-sized inline type). Thermite v0.1 has NO heap. Three options
were considered:

- **(a) `Box<T>`** — introduce `Box<T>` as the first heap primitive, tied to the
  EXISTING `Alloc` effect (`thermite-syntax/src/ast.rs` `Effect::Alloc`; the
  `fx alloc` row of §4.1). Verus models recursive ADTs with `Box` natively.
- **(b) bounded-depth inline types** — no indirection; a fixed maximum depth.
- **(c) arena / index representation** — store nodes in a `Vec`, recurse through
  `usize` indices.

**DECIDED: option (a), `Box<T>`.** It is the only option that (1) Verus accepts
end-to-end for an unbounded recursive type with a `decreases`-terminating fold
(GROUNDED below — `enum List { Nil, Cons(u64, Box<List>) }` with `len`/`sum_list`
verified `0 errors`), (2) reuses the effect lattice the design already pins
rather than inventing a new mechanism (the `Alloc` effect exists in the AST
today, unexercised by the corpus), and (3) sets up Stage 4 — `Vec`/`Map` are the
same heap primitive generalized. Option (b) caps expressiveness below "any finite
data structure" (it cannot represent an unbounded list), failing the basis's
completeness claim. Option (c) requires Stage 4's `Vec` and so cannot be the
keystone (it inverts the dependency order — R-DEFER-7).

**Effect-row consequence (the load-bearing rule this stage pins).** Constructing
a `Box<T>` allocates, so any `fn` that **constructs** a boxed value carries
`fx alloc` (§4.1: "`alloc`" is a member of the effect set; "a caller's row must
subsume every callee's row"). A `spec fn` (e.g. `len`, `sum_list`) carries NO
effect row — it never constructs, only `match`-destructures, and runs in spec
context where `Box<T>` is a transparent wrapper Verus dereferences with `*`
(GROUNDED: the `decreases` recursion uses `*tail`, no `alloc`). The exec-position
construction of a `List`/`Tree` is what carries `fx alloc`; the spec-position
reasoning over one is `pure`. This is the first non-`pure` corpus program and the
first exercise of `Effect::Alloc`.

OQ-1 records the one residual ambiguity (whether `Box` is a first-class `Type`
node `Box(Box<Type>)` or a `Generic { name: "Box", arg }` reusing the existing
`Option<usize>` machinery).

## Requirements

### Surface + AST (governs `thermite-syntax/src/ast.rs`, `parser.rs`)

- **REQ-1 (struct items — product types):** The surface admits a top-level
  `struct NAME { field: TYPE, … }` item, optionally carrying a **type-invariant
  clause** (`inv <expr>`) Verus enforces at construction and use. The AST gains an
  `Item::Struct(StructItem)` where `StructItem { name, fields: Vec<FieldDef>, inv:
  Option<Clause>, span }` and `FieldDef { name, ty: Type }`. Field access in BOTH
  exec position (`a.balance`) and contract position lowers to a Verus field
  expression. Derived from §4 (the surface), §4.4 (closed type set), and the
  Verus data-invariant model (GROUNDED). `Expr::Field` already exists in
  `ast.rs` — this REQ extends WHAT a `Field` receiver may be (a struct value),
  not the node shape.

- **REQ-2 (enum items + variant construction — sum types):** The surface admits a
  top-level `enum NAME { Variant, Variant(TYPE, …), Variant { field: TYPE, … }, …
  }` item — tuple variants (`Circle(u64)`), struct variants (`Rect { w: u64, h:
  u64 }`), and unit variants (`Nil`). The AST gains `Item::Enum(EnumItem)` where
  `EnumItem { name, variants: Vec<VariantDef>, span }` and `VariantDef { name,
  shape: VariantShape }` with `VariantShape` one of `Unit`, `Tuple(Vec<Type>)`,
  `Struct(Vec<FieldDef>)`. Variant CONSTRUCTION reuses the existing expression
  nodes: a unit/path variant is `Expr::Path` (`Shape::Circle` is a callee path;
  `List::Nil` a path), a tuple-variant construction is `Expr::Call { callee:
  Path, args }`, a struct-variant construction is a NEW `Expr::StructLit { path:
  Vec<Ident>, fields: Vec<(Ident, Expr)> }` (also used to construct a `struct`).
  Derived from §4, the existing `enum Expr` (`Path`/`Call`), and the Verus enum
  model (GROUNDED).

  **Variant names MUST be UpperCamelCase (uppercase-initial).** The validator
  rejects a lowercase-initial variant declaration with a structured
  `SpecError::InvalidVariantCasing { name, span }`. This is load-bearing for
  soundness, not style: the parser disambiguates a single-segment arm pattern by
  first-letter case (`Pattern::Enum` if uppercase-initial, `Pattern::Binding`
  otherwise). Forbidding lowercase variants makes that split SOUND — a
  lowercase ident in a pattern is *unambiguously* a binding, because no lowercase
  variant can exist, so a non-exhaustive `match` can never be silently masked by a
  variant-looking name being read as a catch-all binding (the #66 bypass).
  Field names, by contrast, are unconstrained. Derived from §4.4 (always-explicit,
  one desugaring) + the #66 audit.

- **REQ-3 (recursive types via `Box<T>` + the `alloc` effect):** A type may refer
  to itself through `Box<T>`: `enum List { Nil, Cons(u64, Box<List>) }`, `enum
  Tree { Leaf(u64), Node(Box<Tree>, Box<Tree>) }`. The AST `enum Type` gains the
  `Box` indirection (OQ-1: a dedicated `Type::Box(Box<Type>)` or `Generic { name:
  "Box", arg }`). A `fn` constructing a boxed value carries `fx alloc`
  (`Effect::Alloc`, already in `ast.rs`); the effect-subsumption check
  (`.design/lower/effect-subsumption.md`) must accept `alloc` in a caller's row
  when a callee constructs. Derived from §4.1 (the `alloc` effect; row
  subsumption), the Decision section, and the GROUNDED recursive-`List` Verus
  proof.

- **REQ-4 (`match` in expression + statement position, exhaustive, with
  binding):** `match SCRUTINEE { PATTERN => EXPR, … }` over an enum value (and
  struct destructuring `let`/`match` of a `struct`). The AST `Expr::Match`
  already exists (`scrutinee`, `arms: Vec<MatchArm>`); this REQ extends the
  `Pattern` set so enum-variant patterns bind variant payloads — `Pattern::Enum {
  path, fields: Vec<Pattern> }` already covers `Some(i)`/`None`; struct-variant
  patterns (`Rect { w, h }`) and struct destructuring add a `Pattern::Struct {
  path, fields: Vec<(Ident, Pattern)>, rest: bool }` (the `rest` flag is the `..`
  of `Rect { .. }`). Match binding introduces names in scope for the arm body.
  Derived from §4.1 (the `match result { … }` postcondition), §4.4 ("`match`
  ergonomics special cases → One desugaring, always explicit"), Appendix A's
  slice `match`, and the GROUNDED enum `match`.

### Validator / the SpecTherm cage (governs `thermite-spec/src/validator.rs`)

- **REQ-5 (exhaustiveness checking — the validator rejects a non-exhaustive
  `match`):** The validator, knowing the declared `enum`'s variant set (collected
  from `Item::Enum`), rejects a `match` over an enum value whose arms do not
  cover every variant (and is not closed by a `Wildcard` arm), with a span-bearing
  structured `SpecError::NonExhaustiveMatch { missing: Vec<Ident>, span }`.
  Conversely a redundant / unreachable arm (a variant matched twice, or an arm
  after a `Wildcard`) is a `SpecError::UnreachableArm { span }`. This is the
  "validator rejects non-exhaustive matches" rule of the decided scope. Derived
  from §4.4 ("One desugaring, always explicit" — exhaustiveness is mandatory, not
  defaulted), §2.4 (crisp structured feedback), and `validator.rs`'s existing
  `SpecError` discipline (`.design/spec/spectherm-combinators.md` REQ-4). **This REQ IS the compile-time tooth of the handled-or-loud principle** (above): the validator rejecting a non-exhaustive `match` is exactly "every modeled outcome (variant) is handled, or an explicit `Wildcard` catch screams" enforced *before the program ships* — silently dropping an unhandled variant is structurally impossible. The runtime tooth (the always-live §6 L1 contract) and the kill tooth (#57 confinement of the `alloc` effect, REQ-3) are the escalating complements.

- **REQ-6 (well-formed field / variant access + variant discrimination `is`):**
  The validator rejects access to a field a `struct`/struct-variant does not
  declare (`SpecError::UnknownField { name, span }`) and a variant a `enum` does
  not declare (`SpecError::UnknownVariant { name, span }`). In CONTRACT position,
  variant discrimination is written `SCRUTINEE is Variant` (a `bool`-valued
  expression — e.g. `result is Circle`); the AST gains an `Expr::Is { scrutinee:
  Box<Expr>, variant: Vec<Ident> }`, and the validator accepts `is` only against
  a declared variant of the scrutinee's enum. Field access `x.balance` in a
  contract is the existing `Expr::Field`, accepted against a declared field.
  Derived from §4.1 (contracts mention structured `result`), the decided scope
  ("variant discrimination in contracts (`is Circle`)"), and the GROUNDED
  `s is Circle` proof.

- **REQ-7 (ADT predicates fit the SpecTherm cage — flat predicates, no anonymous
  nested quantifiers):** An ADT-shaped contract obeys §4.2's cage exactly as a
  scalar one. Field access (`x.balance`), variant tests (`r is Circle`), and a
  `match`/`if` over an ADT are FLAT built-ins admitted inside a combinator's
  predicate-closure body (`.design/spec/spectherm-combinators.md` REQ-6
  "caged-flat" accept set already lists `Match`/`If`/`Field`). A property that
  must quantify over a RECURSIVE structure's elements (e.g. "every node of a
  `Tree` is `< CAP`") is NOT an anonymous nested quantifier — it is written as a
  NAMED `spec fn` carrying its own `dec` measure (the structural recursion of
  Stage 2; §4.2 "composition happens only through named `spec fn`s"). The
  validator's caged-flat walk (REQ-6 of the combinator doc) is UNCHANGED by ADTs:
  a `match`/`Field`/`is` inside a closure body stays flat; a recursive
  `spec fn` call stays a named-composition accept. Derived from §4.2 (the cage)
  + `.design/spec/spectherm-combinators.md` REQ-6.

### Verus lowering (governs `thermite-lower/src/lower.rs`)

- **REQ-8 (struct → Verus struct; type-invariant → enforced predicate):** A
  `StructItem` lowers to a Verus `struct` with the same fields; its `inv` clause
  lowers to a `pub open spec fn well_formed(&self) -> bool { <inv> }` (the
  data-invariant predicate), referenced in the `requires`/`ensures` of every `fn`
  that takes or returns the struct (so Verus enforces it at construction and use).
  Field access lowers to a Verus field expression in both contexts. **GROUNDED**:
  this is the exact verified pattern (`Account { balance }`, `well_formed(&self) {
  balance <= CAP }`, `open_account`/`deposit` enforcing it). Visibility must be
  consistent — a `pub open spec fn` body may only refer to `pub` fields and `pub`
  consts (recorded finding, see Architecture). Derived from §3 ("transpile to
  Verus"), §6 (L3), and the GROUNDED struct-invariant proof.

- **REQ-9 (enum → Verus enum; `match` → Verus `match`; `is` → variant test):** An
  `EnumItem` lowers to a Verus `enum` with the same variants; a `match` lowers to
  a Verus `match` (one desugaring, §4.4); `Expr::Is { variant }` lowers to the
  Verus `scrutinee is Variant` discriminant test. **GROUNDED**: `enum Shape {
  Circle(u64), Rect { w, h } }`, `shape_area` via `match`, and `is_circle`
  `ensures result == (s is Circle)` verified `0 errors`. Derived from §3, §4.4,
  and the GROUNDED enum/match/`is` proof.

- **REQ-10 (recursive type → Verus recursive enum; `Box` → `Box`; structural
  `decreases`):** A recursive `EnumItem` lowers to a Verus recursive `enum` with
  `Box<T>` at the recursive occurrence; a `spec fn` over it carries `decreases
  <value>` (the value itself, NOT a derived measure — Verus orders datatype values
  structurally) and recurses through the `Box` with `*`. **GROUNDED**: `enum List
  { Nil, Cons(u64, Box<List>) }`, `spec fn len(l: List) -> nat decreases l { match
  l { Nil => 0, Cons(_, tail) => 1 + len(*tail) } }` and `sum_list`, plus a
  `proof fn` by structural induction, verified `0 errors`. The exec-position
  construction of a boxed value lowers to `Box::new(..)` and the owning `fn`
  emits no Verus effect annotation but carries `fx alloc` at the Thermite layer
  (REQ-3). Derived from §3, §4.1 (`alloc`, termination by default), §6, and the
  GROUNDED recursive-`List` proof.

- **REQ-11 (`LowerError`/`SpecError` extension, no panics):** The new ADT
  constructs extend the EXISTING `thermite-lower::LowerError` and
  `thermite-spec::SpecError` enums with span-bearing variants for the new failure
  modes (REQ-5/REQ-6 reject cases; an un-lowerable ADT construct), reusing
  `thermite_syntax::lexer::Span`. No `unwrap`/`expect`/`panic!` in production
  (R-CODE-2 / R-APG-1). Derived from R-CODE-2, the existing error-enum discipline
  in `validator.rs` / `lower.rs`.

- **REQ-12 (handled-or-loud is mechanically enforced on every ADT outcome — the
  compile-time tooth):** every outcome an ADT MODELS (each declared variant of an
  `enum`; each arm of a returned sum type) is, at the point a `match` consumes it,
  either HANDLED by an arm or covered by an explicit `Wildcard` catch that screams
  — the validator REJECTS any other shape (`SpecError::NonExhaustiveMatch`, REQ-5)
  *at validation, before the program ships*. There is no silent fall-through: a
  non-exhaustive `match` does not compile. This is the COMPILE-TIME tooth of the
  toolchain's unifying handled-or-loud law (the principle section above); the
  runtime tooth is the always-live §6 L1 contract (a violated `[ens]`/`[req]`
  aborts exit 101, never returns a wrong value — an ADT contract rides it exactly
  as a scalar one), and the kill tooth is #57's seccomp confinement of the `alloc`
  effect (REQ-3). Derived from §4.4 ("`match` … One desugaring, always explicit"),
  §6 (the L1 rung active in every profile), the #57 sandbox
  (`.design/forge/runtime-sandbox.md`), and the **#62** design-refinement
  unifying-principle decision. REQ-12 is the named statement of the law; REQ-5 is
  its concrete validator mechanism.

## Acceptance criteria

The orchestrator authors a NEW corpus program — call it
`conformance/bank_account.th` (a `struct Account` with a `balance <= CAP`
invariant) and a NEW recursive program `conformance/list_sum.th` (`enum List {
Nil, Cons(u64, Box<List>) }` + a `spec fn sum_list` and an exec `fn` that
constructs and folds a list, the terminating fold-precursor Stage 2 builds on).
Their golden lowerings live at `tests/golden/lower/bank_account.verus.rs` /
`tests/golden/lower/list_sum.verus.rs`, hand-authored from this doc and confirmed
to pass `verus` (the forms below are the verified seed). The certificate goldens
live at `conformance/bank_account.cert.json` / `conformance/list_sum.cert.json`.

- **AC-1 (struct + invariant parses, validates, lowers, certifies L3):** Parsing
  `bank_account.th` yields an `Item::Struct` with an `inv` clause; the validator
  accepts its contracts (field access well-formed, `req`/`ens` in the cage); the
  lowerer emits a Verus `struct` + `well_formed` predicate; running the real
  `verus` binary on the emitted output exits 0 with `N verified, 0 errors`; the
  emitted certificate matches `bank_account.cert.json` (L3, non-vacuous).
  (REQ-1, REQ-8, R-DEFER-9 no weakening.)

- **AC-2 (enum + exhaustive `match` parses, validates, lowers, certifies L3):**
  Parsing the enum program yields an `Item::Enum`; the validator ACCEPTS an
  exhaustive `match` and REJECTS a non-exhaustive one with
  `SpecError::NonExhaustiveMatch` (a crafted negative fixture under
  `conformance/parse` or a validator reject fixture); the lowerer emits a Verus
  `enum` + `match`; `verus` certifies L3. (REQ-2, REQ-4, REQ-5, REQ-9.)

- **AC-3 (recursive `List` + terminating fold parses, validates, lowers,
  certifies L3):** Parsing `list_sum.th` yields a recursive `Item::Enum` with a
  `Box` recursive occurrence; the constructing exec `fn` carries `fx alloc` and
  passes effect-subsumption; the `spec fn sum_list` carries a `dec` and the
  lowerer emits `decreases l` over the datatype value with `*tail` recursion;
  `verus` certifies L3 (`N verified, 0 errors`). (REQ-3, REQ-4, REQ-10.)

- **AC-4 (`is` discrimination in a contract):** A `fn` whose `ens` mentions
  `result is Circle` (or a struct-variant test) parses to `Expr::Is`, validates
  against the declared variant set (REQ-6), and lowers to the Verus `is`
  discriminant — `verus` certifies it. A crafted `result is Nonexistent` rejects
  with `SpecError::UnknownVariant`. (REQ-6, REQ-9.)

- **AC-5 (exhaustiveness + well-formedness reject cases):** Crafted negatives
  reject with the right structured variant: a `match` missing a variant →
  `NonExhaustiveMatch { missing }`; an arm after a `Wildcard` → `UnreachableArm`;
  `x.no_such_field` → `UnknownField`; `r is NoSuchVariant` → `UnknownVariant`.
  Hand-derived expectations (R-CHAR-3), never read back from the validator's
  output. (REQ-5, REQ-6, REQ-11.)

- **AC-6 (the existing corpus still works — no regression):** Parsing,
  validating, and lowering `conformance/sum.th` and `conformance/binary_search.th`
  is UNCHANGED — they still parse to the same AST, validate clean, lower to the
  byte-stable `tests/golden/lower/{sum,binary_search}.verus.rs`, and certify L3.
  The ADT additions are purely additive (new `Item`/`Expr`/`Pattern`/`Type`
  variants, new `SpecError`/`LowerError` variants); no existing node reshapes.
  Mechanically: `cargo test -p thermite-syntax -p thermite-spec -p thermite-lower`
  and the conformance corpus pass with 0 mismatches. (All REQs; the keystone
  must not break the kernel.)

## Architecture

The component spans three crates, all additively:

- **`thermite-syntax`** — `enum Item` gains `Struct(StructItem)` and
  `Enum(EnumItem)` (`thermite-syntax/src/ast.rs`); `enum Expr` gains `StructLit`
  and `Is`; `enum Pattern` gains `Struct`; `enum Type` gains the `Box`
  indirection (OQ-1). `parser.rs` gains `parse_struct`/`parse_enum`/`parse_match`
  (the last already partially present for the slice `match` of Appendix A) and
  struct/enum-pattern parsing. The mandatory-contract discipline of `Contract`
  (`ast.md` REQ-2) is unchanged — a `struct`/`enum` item carries no `req`/`ens`/
  `fx`; only `fn` does.

- **`thermite-spec`** — `validator.rs` gains the enum-variant-set collection (a
  pass over `Item::Enum` mirroring the existing `spec fn` name collection,
  `.design/spec/spectherm-combinators.md` REQ-3), the exhaustiveness/redundancy
  check (REQ-5), and the field/variant well-formedness + `is` checks (REQ-6),
  emitting new `SpecError` variants. The caged-flat walk (that doc's REQ-6) is
  UNCHANGED: `Match`/`If`/`Field` are already flat built-ins; `Expr::Is` joins
  them as a flat `bool` built-in. ADT properties that quantify over a recursive
  structure are NAMED `spec fn`s (REQ-7) — composition stays named, never
  anonymous, so the §4.2 cage is preserved.

- **`thermite-lower`** — `lower.rs` gains `lower_struct`/`lower_enum`/
  `lower_match`/`lower_is` and the `well_formed`-predicate emission (REQ-8). The
  two lowering contexts (exec vs spec, `.design/lower/verus-lowering.md`) extend
  to ADTs: a `struct`/`enum` value is the same spelling in both; the
  data-invariant predicate is a `spec fn` referenced from `requires`/`ensures`.
  Symbol anchors: `struct StructItem`/`struct EnumItem`/`enum Expr` in `ast.rs`;
  `pub fn validate` in `validator.rs`; `pub fn lower` / `lower_expr` in `lower.rs`.

### The verified Verus forms (GROUNDED — the lowering contract, not guesses)

These were produced by running the real `verus 0.2026.05.24` binary during
authoring (Verification). They are the seed for the golden files.

**Struct + type invariant (REQ-8).** The data-invariant unlock:

```verus
pub spec const CAP: u64 = 1_000_000;

pub struct Account {
    pub balance: u64,
}

impl Account {
    pub open spec fn well_formed(&self) -> bool {
        self.balance <= CAP
    }
}

fn open_account(initial: u64) -> (result: Account)
    requires initial <= CAP,
    ensures result.well_formed(), result.balance == initial,
{ Account { balance: initial } }

fn deposit(a: Account, amount: u64) -> (result: Account)
    requires a.well_formed(), amount <= CAP - a.balance,
    ensures result.well_formed(),
{ Account { balance: a.balance + amount } }
```

**RECORDED FINDING (visibility — load-bearing for the lowerer).** A `pub open
spec fn` body may only refer to `pub` items: the `well_formed` invariant must be
`pub open`, the `struct` and its fields `pub`, and any referenced `spec const`
(`CAP`) `pub` — otherwise Verus rejects with `field expression for a non-visible
datatype` / `cannot refer to private const item`. The lowerer must emit a
consistent visibility tier for the invariant predicate, the struct, its fields,
and any constant the invariant references. The non-vacuity of the invariant was
confirmed: a guard-less `deposit` (omitting `amount <= CAP - a.balance`)
correctly FAILS to verify — the invariant is not trivially true.

**Enum + `match` + `is` (REQ-9).**

```verus
enum Shape { Circle(u64), Rect { w: u64, h: u64 } }

spec fn shape_area(s: Shape) -> nat {
    match s {
        Shape::Circle(r) => 3nat * (r as nat) * (r as nat),
        Shape::Rect { w, h } => (w as nat) * (h as nat),
    }
}

fn is_circle(s: &Shape) -> (result: bool)
    ensures result == (s is Circle),
{ match s { Shape::Circle(_) => true, Shape::Rect { .. } => false } }
```

`s is Circle` is Verus-native variant discrimination — the lowering of REQ-6's
`Expr::Is`. The struct-variant `match` arm `Rect { w, h }` and the `..` rest
pattern (`Rect { .. }`) are REQ-4's `Pattern::Struct`.

**Recursive type via `Box` + structural `decreases` (REQ-10).** The keystone
proof:

```verus
enum List { Nil, Cons(u64, Box<List>) }

spec fn len(l: List) -> nat
    decreases l,
{
    match l {
        List::Nil => 0,
        List::Cons(_, tail) => 1 + len(*tail),
    }
}

spec fn sum_list(l: List) -> nat
    decreases l,
{
    match l {
        List::Nil => 0,
        List::Cons(x, tail) => (x as nat) + sum_list(*tail),
    }
}

proof fn len_nonneg(l: List)
    ensures len(l) >= 0,
    decreases l,
{
    match l {
        List::Nil => {}
        List::Cons(_, tail) => { len_nonneg(*tail); }
    }
}
```

**RECORDED FINDING (the structural-recursion stack is end-to-end feasible).**
`decreases l` (the datatype VALUE, not a derived `usize` measure) terminates the
recursion — Verus has a built-in structural order on datatype values, so a fold
over a `Box`-recursive ADT needs NO manual measure. The recursive occurrence is
dereferenced with `*tail`. A `Tree` (`Node(Box<Tree>, Box<Tree>)`) with a
`tree_sum` fold was also confirmed to verify. This is the foundation Stage 2's
recursion schemes (`fold`/`map`) and Stage 4's collections build on — both
expressiveness (recursive ADTs representable) and provability (structural
induction native) arrive together, exactly the basis thesis.

## Dependency hooks (for the rest of epic #62)

- **Stage 2 (recursion schemes — fold/map):** consumes the recursive ADTs of
  REQ-3/REQ-10. A `fold` is a `spec fn` with `decreases l` over a `Box`-recursive
  enum (the GROUNDED `sum_list`/`len` are the fold-precursors); REQ-7's named
  `spec fn` composition is exactly how a fold quantifies over a structure's
  elements inside the SpecTherm cage. Stage 1 must land the recursive type +
  `decreases l` lowering (REQ-10) before any scheme can be written.

- **Stage 4 (collections — Vec/Map):** consumes the `Box`/`alloc` heap primitive
  of REQ-3. `Vec`/`Map` are the same `Alloc`-effect heap generalized; the
  effect-row rule (a constructing `fn` carries `fx alloc`) and the
  effect-subsumption acceptance of `alloc` (REQ-3) are reused verbatim. Option (c)
  for recursive types (arena/index) was REJECTED precisely because it would invert
  this dependency (collections-before-keystone).

- **Stage 5 (composition law):** reasons over the ADT contracts pinned here — the
  struct type-invariant (`well_formed`, REQ-8) and enum-variant discrimination
  (`is`, REQ-6) are the contract surface a composition law quantifies the
  data-half over. The §9 composition rule ("if `g` calls `f` only through `f`'s
  contract …") applies to ADT-valued contracts unchanged.

## Verification

- **Mandatory Verus grounding (DONE during authoring — real `verus
  0.2026.05.24`).** A single `verus!{}` file containing the struct-with-invariant
  (`Account`/`well_formed`/`open_account`/`deposit`), the enum + `match` + `is`
  (`Shape`/`shape_area`/`is_circle`), and the recursive `List` + `decreases`-
  terminating `len`/`sum_list` + structural-induction `proof fn` verified:

  ```
  verus --no-cheating /tmp/adt_ground.rs
  verification results:: 8 verified, 0 errors
  ```

  Cheat-token grep (`assume`/`external_body`/`admit`/`verifier::external`) over
  the file: NONE. Non-vacuity confirmed by a companion run where a guard-less
  `deposit` and a `Tree` fold were checked: the unguarded write FAILS
  (`3 verified, 1 errors`) — the invariant is real, the `Tree` recursion (via
  `Box`) verifies. This proves the ADT + recursive-type + `match` +
  structural-recursion stack is Verus-feasible end to end — the foundation for
  Stages 2 and 4.

- **AC-1/AC-2/AC-3/AC-4:** `cargo test -p thermite-syntax -p thermite-spec -p
  thermite-lower`, plus a harness that shells the real `verus` binary on the
  emitted lowering of `bank_account.th` / `list_sum.th` / the enum program and
  asserts exit 0 + `N verified, 0 errors` (R-CODE-4: subprocess status checked,
  never swallowed), plus `forge check` matching the golden certificates
  (`conformance/{bank_account,list_sum}.cert.json`).
- **AC-5:** validator reject fixtures (hand-derived expectations, R-CHAR-3).
- **AC-6:** the existing `tests/golden/lower/{sum,binary_search}.verus.rs` and
  `conformance/sum.cert.json` assertions stay green (no regression).

Gauntlet (R-DEFER-6, per crate): `cargo test -p <crate>`, `cargo clippy -p
<crate> --all-targets -- -D warnings`, `cargo fmt --check`.

## Routes to add (orchestrator)

This stage adds NEW concerns to files that already have routes; the orchestrator
adds these routes to `tooling/spec-routes.toml` pointing at THIS doc (a file may
carry multiple governing docs — the §52 `lower.rs` precedent):

```
[[route]]  crate_pattern = "thermite-syntax/src/ast.rs"        design = ".design/basis/01-adts.md"   reference = ["conformance/bank_account.th", "conformance/list_sum.th"]
[[route]]  crate_pattern = "thermite-syntax/src/parser.rs"     design = ".design/basis/01-adts.md"   reference = ["conformance/bank_account.th", "conformance/list_sum.th"]
[[route]]  crate_pattern = "thermite-spec/src/validator.rs"    design = ".design/basis/01-adts.md"   reference = ["conformance/bank_account.th"]
[[route]]  crate_pattern = "thermite-lower/src/lower.rs"       design = ".design/basis/01-adts.md"   reference = ["tests/golden/lower/bank_account.verus.rs", "tests/golden/lower/list_sum.verus.rs"]
```

The corpus programs `conformance/bank_account.th`, `conformance/list_sum.th`,
their `.cert.json` goldens, and the `tests/golden/lower/*.verus.rs` lowerings are
authored by the orchestrator from this doc before the builder runs (R-CHAR-3).

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (struct items + type invariant) | NOT-STARTED | epic **#62** Stage 1. No `Item::Struct`/`StructItem` in `thermite-syntax/src/ast.rs` (`enum Item` is `Fn`/`SpecFn` only); no `struct` parse path in `parser.rs`. GROUNDED-feasible (verus `8 verified, 0 errors`), not yet implemented. |
| REQ-2 (enum items + variant construction) | NOT-STARTED | epic **#62** Stage 1. No `Item::Enum`/`EnumItem`/`Expr::StructLit` in `ast.rs`; the surface admits no `enum` item today. |
| REQ-3 (recursive types via `Box` + `alloc`) | NOT-STARTED | epic **#62** Stage 1. `enum Type` (`ast.rs`) has no `Box` indirection; `Effect::Alloc` exists but is unexercised; no corpus program is non-`pure`. Representation DECIDED (`Box`+`alloc`, GROUNDED), not implemented. |
| REQ-4 (`match` exhaustive + binding, struct destructuring) | NOT-STARTED | epic **#62** Stage 1. `Expr::Match` + `Pattern::Enum` exist (slice-`match` of Appendix A) but no `Pattern::Struct` and no enum-item `match` validation/lowering. |
| REQ-5 (exhaustiveness checking in the validator) | SHIPPED | epic **#62** Stage 1b (#65). `thermite-spec/src/validator.rs`: the declaration pre-pass `Validator::new` collects `enums` (name → variant order) + `variant_to_enum`; `check_match_exhaustiveness` (reached from the caged `walk_expr_inner` `Match` arm AND the exec-body `scan_expr_for_loops` `Match` arm) emits `SpecError::NonExhaustiveMatch { missing }` (declaration order), `SpecError::UnreachableArm` (variant twice / arm after wildcard), and `SpecError::UnknownVariant` (undeclared variant in a pattern); a slice/`Option` `match` is inert (no regression). Consumer: `pub fn validate`. Verification: `thermite-spec/tests/adt_validate.rs` over `conformance/adt-validate/cases.json` — `non_exhaustive_match` → `missing:[Rect]`, `unreachable_redundant_arm` → `UnreachableArm`, `unknown_variant_pattern` → `UnknownVariant{Square}`; `shape`/`list_sum` accept. The Verus LOWERING of `match` (REQ-9) stays Stage 1c. |
| REQ-6 (well-formed field/variant access + `is`) | SHIPPED | epic **#62** Stage 1b (#65). `validator.rs`: the pre-pass collects `struct_fields` (every `struct`/struct-variant field); `check_field` (on `Expr::Field` + `Expr::StructLit` field names, both walks) → `SpecError::UnknownField` (inert when no struct declared); `check_variant_ref` (on `Expr::Is`, both walks) → `SpecError::UnknownVariant` for an undeclared variant. Consumer: `pub fn validate`. Verification: `tests/adt_validate.rs` — `unknown_field` → `UnknownField{bogus}`, `unknown_variant_is` → `UnknownVariant{Triangle}`; `bank_account`/`shape` accept. The Verus LOWERING of `is` (REQ-9) stays Stage 1c. |
| REQ-7 (ADT predicates fit the SpecTherm cage) | SHIPPED | epic **#62** Stage 1b (#65). The caged-flat walk (`.design/spec/spectherm-combinators.md` REQ-6) shipped under #40; in `validator.rs`'s `walk_expr_inner`, `Expr::Match`/`Field`/`Is`/`Deref` recurse operands WITHOUT setting `in_combinator_closure` and WITHOUT resolving as combinators, so they are admitted as FLAT built-ins inside a combinator predicate-closure body unchanged. No recursive scheme exists yet to nest in a closure (forward-declared; schemes are Stage 2). Verification: the combinator cage tests (`tests/combinators_conformance.rs`, `divergence_nesting.rs`) stay green. |
| REQ-8 (struct → Verus struct; invariant → predicate) | SHIPPED | epic **#62** Stage 1c (#67). `thermite-lower/src/lower.rs`: `lower_struct` emits a `pub struct` + `pub` fields + `impl { pub open spec fn well_formed(&self) -> bool { <inv with self.field> } }` (`lower_inv_expr`); **OQ-3 RESOLVED — automatic threading**: `lower_fn_signature` weaves `<param>.well_formed()` / `result.well_formed()` into `requires`/`ensures` for every invariant-bearing struct param/return (the `inv_structs` set built in `lower`). The `pub` visibility tier is the recorded grounding finding. L1 mirror: `l1.rs::lower_struct_l1` emits the `well_formed` method + `lower_fn_l1` weaves the always-active `thermite_check!`. Consumer: `pub fn lower` / `pub fn lower_l1`. Verification: real verus `1 verified, 0 errors` on the emitted `bank_account` lowering + cert oracle (`conformance/bank_account.cert.json` L3/pure/non-vacuous) — `thermite-lower/tests/adt_lower_conformance.rs::bank_account_lowers_struct_invariant_and_verifies_l3`, `deposit_matches_cert_oracle_stable_subset`, `bank_account_l1_compiles_and_runs`, `bank_account_l1_req_check_fires`. |
| REQ-9 (enum → Verus enum; `match` → `match`; `is`) | SHIPPED | epic **#62** Stage 1c (#67). `lower.rs`: `lower_enum` emits a Verus `enum` (unit/tuple/struct variants); `lower_match`/`lower_pattern` emit ENUM-QUALIFIED arms via the program `(variant,enum)` map (`qualify_variant_path`) incl. `Pattern::Struct` (`Rect { w, h }`/`..`); `Expr::Is`→`(s is Circle)` the Verus-native discriminant. L1 mirror: `l1.rs` `lower_enum_l1`/`lower_match_exec`/`lower_pattern_exec` + `Expr::Is`→`matches!(s, Shape::Circle { .. })`. Consumer: `pub fn lower`/`pub fn lower_l1`. Verification: real verus `1 verified, 0 errors` on the emitted `shape` lowering + cert oracle (`conformance/shape.cert.json` L3/pure/non-vacuous) — `shape_lowers_enum_match_is_and_verifies_l3`, `is_circle_matches_cert_oracle_stable_subset`, `shape_l1_compiles_and_runs`, `shape_l1_ens_check_fires_on_a_lying_body`. |
| REQ-10 (recursive type → Verus recursive enum; `Box`; structural `decreases`) | SHIPPED | epic **#62** Stage 1c (#67). `lower.rs`: `lower_enum` emits `Cons(u64, Box<List>)` (`lower_type` `Type::Box`→`Box<…>`); a `spec fn` of the ADT-fold-sum shape (`is_adt_fold_sum`) lowers `-> nat` with `decreases l` over the datatype VALUE (Verus's built-in structural order) and `Expr::Deref`→`*t`, integer casts coerced `as nat` (`Ctx::nat_ret`). Consumer: `pub fn lower`. Verification: real verus `1 verified, 0 errors` on the emitted `list_sum` lowering (the recursive spec fn terminates + totals) — `list_sum_lowers_recursive_box_and_verifies_l3`. The `fx alloc` effect-row for an exec `Box`-constructor is forward-ready (`effects.rs` accepts `alloc`); the corpus `list_sum` is spec-fn-only (`pure`), so no exec `alloc` is exercised this stage. |
| REQ-11 (`LowerError`/`SpecError` extension, no panics) | SHIPPED | epic **#62** Stage 1c (#67). The lowering reuses the existing `LowerError` (`Unsupported`/`TooDeep`) — no new lower variant was needed (the validator #65 already owns the `SpecError` reject cases REQ-5/REQ-6); no `unwrap`/`expect`/`panic!` added in `lower.rs`/`l1.rs`/`l2.rs`. Verification: `cargo clippy --workspace --all-targets -- -D warnings` PASS + the anti-pattern-gate. |
| REQ-12 (handled-or-loud — compile-time tooth via exhaustive `match`) | SHIPPED | epic **#62** Stage 1c (#67). The named compile-time tooth is the #65 validator's `SpecError::NonExhaustiveMatch` (REQ-5, SHIPPED); Stage 1c's L3/L1 lowering of an ACCEPTED `match` preserves it — every arm is emitted, a non-exhaustive `match` never reaches the lowerer (it dies at the validator), and the runtime tooth (the §6 L1 contract) rides an ADT contract exactly as a scalar one (`shape_l1_ens_check_fires_on_a_lying_body`: a lying ADT body ABORTS, observable). The kill tooth is #57 (`.design/forge/runtime-sandbox.md`, SHIPPED). |

## Open questions (for the orchestrator before the builder runs)

- **OQ-1 (`Box` as a dedicated `Type` node vs. `Generic`):** REQ-3 needs the
  recursive occurrence to be representable in `enum Type`. Two shapes: a dedicated
  `Type::Box(Box<Type>)` (clearest — `Box` is the heap primitive, distinct from
  the closed generic set), or reuse the existing `Generic { name: "Box", arg }`
  machinery (`Option<usize>`'s path; less new surface). The lowering is identical
  (`Box<T>` either way). RECOMMEND the dedicated `Type::Box` so the
  effect-subsumption check (REQ-3) can key on the node kind rather than a string
  name match. Not a blocker; pinned for the builder.

- **OQ-2 (closed type set vs. user `struct`/`enum` — the §4.4 tension):** §4.4
  removed the full trait system and user-defined abstractions to fit the 6k-token
  skill budget, but it did NOT forbid user data types — Appendix A and §4.1 use
  structured values (`Option`, slices, `match result { Some(i) … }`) freely. This
  stage admits user `struct`/`enum` as DATA (no methods, no traits — the closed
  built-in interface set of §4.4 still applies). The skill-budget consequence
  (§10, #7) — the ADT grammar must fit the 6k-token `THERMITE.skill.md` — is real
  and is the orchestrator's check at Stage 1's skill regeneration. Flagged
  because it is the one place the basis buildout touches a hard design budget;
  the grammar is small (struct/enum/match/`is`/`Box`) and expected to fit, but
  the §10 gate is the arbiter. Not a blocker.

- **OQ-3 (least-confident: the `well_formed` invariant emission mechanism).** The
  GROUNDED struct invariant is enforced by THREADING `requires
  a.well_formed()` / `ensures result.well_formed()` through every `fn` touching
  the struct (Verus has no automatic struct-invariant-at-construction in the form
  used here — the predicate is referenced explicitly). The open question is
  whether the lowerer threads it automatically (every param/return of an
  invariant-bearing struct gets the `well_formed` conjunct) or whether the
  Thermite surface requires the author to write it. RECOMMEND automatic threading
  (the invariant is a property of the TYPE, so it should be implicit at every use
  — the "data-invariant unlock" of the decided scope). This is the
  highest-judgment, least-confident part of REQ-8: the GROUNDED proof writes the
  conjuncts BY HAND, so the automatic-threading lowering is designed-but-unproven
  end-to-end. Verus's `#[verifier::type_invariant]` attribute is an alternative
  the builder should evaluate against the explicit-threading form. Not a blocker
  for the corpus (the orchestrator's `bank_account.th` golden pins the verified
  output); a real design call for the builder.

- **OQ-4 (`fx alloc` as the first non-`pure` corpus program):** REQ-3 makes a
  list-constructing `fn` the first `fx alloc` corpus entry, exercising
  `Effect::Alloc` and effect-subsumption (`.design/lower/effect-subsumption.md`)
  for the first time. If the orchestrator prefers to keep Stage 1's corpus
  `pure`, the constructing `fn` can be a `spec fn`-only fold over a list built in
  the spec layer (no `alloc`), deferring the exec `alloc` exercise — but that
  weakens the "program anything" exec story. RECOMMEND shipping the `fx alloc`
  exec constructor so the heap primitive is exercised end-to-end at the keystone.
  Not a blocker.
