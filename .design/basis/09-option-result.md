# Built-in `Option` / `Result` + payload-in-contract projection (Basis Cluster C7)
<!--
tier: 3-component
status: draft
governs: thermite-syntax/src/ast.rs
governs: thermite-syntax/src/parser.rs
governs: thermite-spec/src/validator.rs
governs: thermite-lower/src/lower.rs
thesis-refs:
  - thermite-design.md §4
  - thermite-design.md §4.1
  - thermite-design.md §4.2
  - thermite-design.md §4.4
  - thermite-design.md §6
-->

## Summary

Cluster **C7** (crosslink **#95**) adds the two foundational error-handling
primitives the rest of the basis already leans on: a **built-in `Option<T>`**
(constructible `Some(v)` / `None`, not just matchable) and a **built-in
`Result<T, E>`** (the two-type-arg type + `Ok(v)` / `Err(e)` constructors +
matching), PLUS the hard part — **payload-in-contract projection**: the ability
for an `ens`/`req` to refer to the PAYLOAD a `Some`/`Ok` carries (`ens result is
Some ==> <payload> == parse_be(s)`). This is the precise blocker on
`parse_u64` (`.design/basis/07-strings.md` REQ-9, the C4 #94 deferral): the
verus probe proves the round-trip contract verifies, but the Thermite surface had
no spelling for the payload in a contract.

Today (probe-confirmed against the shipped toolchain): `Option` is **matchable**
(`match o { Some(x) => …, None => … }` lowers) but **NOT constructible** —
`Some(v)` / `None` in expression position reach the validator's variant check and
reject (`"Some is not a declared variant"`, `SpecError::UnknownVariant`).
`Result<T, E>` does not even **parse**: `parser::parse_type` takes exactly ONE
type argument after a generic name (`Generic { name, arg }`), so `Result<u64,
ParseErr>` dies at the comma (`expected >, found ,`). And there is **no
enum-payload projection** in the spec sublanguage at all: `Expr::Field` is
struct-field-only; there is no `result->Some_0` surface node and a `match` was
never admitted as a contract-position projection of a built-in.

**The surface form DECIDED here (GROUNDED, below): the spec-`match`-in-`ens`.**
A contract projects a `Some`/`Ok` payload by `match`-ing the result:

```thermite
fn parse_u64(s: &String) -> Option<u64>
  req s.well_formed()
  ens match result { Some(v) => all_digits(s) && parse_be(s) == v, None => true }
  fx  pure
{ … }
```

This was chosen over a dedicated projection operator (`result->Some_0`) because it
**reuses the already-SHIPPED `Expr::Match`** (`.design/basis/01-adts.md` REQ-4,
admitted as a flat built-in inside the §4.2 cage by 01-adts REQ-7), adds **NO new
AST node** (so no match-arm/skill ripple), and stays inside the §4.2 cage as a
flat `match` whose arms are flat predicates. Both forms verify identically in
verus (`5 verified, 0 errors` on `parse_u64`); the spec-match form is the one that
costs zero new surface. See the Decision section.

## Decision: the payload-in-contract form — spec-`match`-in-`ens`, not a projection operator

Two surface forms project an enum-variant payload in a contract. Both were
**GROUNDED with the real `verus 0.2026.05.24` binary** during authoring
(Verification, below) and both verify `parse_u64` at `5 verified, 0 errors`:

- **(a) spec-`match`-in-`ens`** — `ens match result { Some(v) => <flat pred over
  v>, None => true }`. The payload `v` is bound by the `match` arm exactly as in
  an exec `match`; each arm body is a flat `bool` predicate.
- **(b) a projection operator** — `result->Some_0` (Verus's native field-of-variant
  projection), guarded by `result is Some ==>`. Needs a NEW AST `Expr` node
  (`Expr::Project { scrutinee, variant, index }` or similar) and a NEW validator
  cage rule admitting it.

**DECIDED: form (a), the spec-`match`-in-`ens`.** The decisive reasons:

1. **Zero new AST surface.** `Expr::Match` already exists and already lowers to a
   Verus `match` (`.design/basis/01-adts.md` REQ-4/REQ-9, SHIPPED). Form (b) would
   add a brand-new `Expr` variant, breaking every exhaustive `match Expr` across
   the workspace (lower/l1/effects/validator/mutation/vacuity/closure/review —
   the exact match-arm ripple `ast.md` REQ-10/REQ-12 documents) AND requiring a
   new `THERMITE.skill.md` fragment. Form (a) costs none of this.
2. **It is already in the §4.2 cage.** `.design/basis/01-adts.md` REQ-7 (SHIPPED)
   admits `Expr::Match` as a FLAT built-in inside a contract / combinator-closure
   body — the validator's `walk_expr_inner` `Match` arm recurses operands without
   resolving as a combinator. A spec-`match` over `result` with flat arm bodies is
   already an accepted flat predicate; C7 only needs to admit the BUILT-IN
   `Some`/`None`/`Ok`/`Err` arm patterns (REQ-3), not a new cage rule.
3. **One way to do everything (§2.3 / §4.4).** A `match` is already the one
   destructuring form; making it the contract-position projection too keeps a
   single spelling. A `->` operator would be a second, parallel surface.

Form (b) is not *wrong* — `result->Some_0` verifies — but it buys nothing the
spec-`match` does not, at the cost of a new node + ripple + skill arm. The
spec-`match`-in-`ens` is the lower-surface-cost, cage-native choice. (If a future
cluster finds the `match` form too verbose for a deeply-nested projection, a
projection operator is a clean additive follow-up — recorded, not v1.)

## Decision: `Option` / `Result` REUSE the enum machinery; `Result<T,E>` needs a two-arg `Type`

`Option<T>` and `Result<T, E>` are **NOT new AST `Type`/`Expr` forms** wherever the
existing enum/match machinery already serves:

- **Matching** — `match o { Some(x) => …, None => … }` and `match r { Ok(v) => …,
  Err(e) => … }` already lower (`Expr::Match` + `Pattern::Enum`, SHIPPED). The
  arms reuse `Pattern::Enum { path: [Some], fields: [Binding(x)] }` verbatim.
- **Construction** — `Some(v)` / `Ok(v)` / `Err(e)` are `Expr::Call { callee:
  Path([Some]), args: [v] }`; `None` is `Expr::Path([None])`. These EXISTING
  expression nodes (`ast.md` REQ-6) need no reshape. The ONLY gap is the
  validator's variant registry: `Some`/`None`/`Ok`/`Err` must be **registered as
  built-in variants** so construction (and the `match`/`is` over them) is NOT
  rejected as `UnknownVariant` (REQ-1).
- **`is` discrimination** — `result is Some` / `result is Ok` is the existing
  `Expr::Is` (`.design/basis/01-adts.md` REQ-6, SHIPPED), again gated only by the
  built-in-variant registry.

**The ONE genuine AST/parser change: a two-type-argument `Type` for `Result<T,
E>`.** `parser::parse_type` today parses `NAME<T>` into `Type::Generic { name,
arg: Box<Type> }` — exactly ONE arg — so `Result<u64, ParseErr>` fails at the
comma. C7 needs `Result<T, E>` to parse. Two shapes (OQ-1): a dedicated
`Type::Result(Box<Type>, Box<Type>)` node (clearest — `Result` is a built-in, the
lowerer keys on the node kind, mirroring the `Type::Vec`/`Type::Box`/`Type::String`
dedicated-node precedent), OR a generalized `Type::Generic { name, args:
Vec<Type> }` (a *reshape* of the existing single-arg `Generic` — a wider ripple,
since `Option<usize>` is `Generic` today). **RECOMMEND the dedicated
`Type::Result` node** (additive, keys on node kind, no `Generic` reshape) — and a
dedicated `Type::Option(Box<Type>)` too, so `Option<u64>` stops being a
string-named `Generic` and the lowerer/validator key Option on the node kind
consistently with `Result`. This is the load-bearing parser/AST change of C7 and
the least-confident point (see OQ-1).

## The §4.2 cage + handled-or-loud (the Result error arms scream)

A spec-`match`-in-`ens` is a FLAT `match` (§4.2 cage, `.design/basis/01-adts.md`
REQ-7): its scrutinee is `result`, its arms bind a payload and evaluate a flat
`bool` predicate (comparisons / arithmetic / named `spec fn` calls — never an
anonymous nested quantifier). The cage is UNCHANGED; C7 admits the built-in
`Some`/`Ok`/`Err`/`None` arm patterns, nothing more.

`Option`/`Result` ARE the data-side incarnation of the toolchain's
**handled-or-loud** law (`.design/basis/01-adts.md` "the unifying principle";
`.design/basis/06-provenance-and-sinks.md`): a partial operation (`parse_u64`)
MODELS its failure outcomes as the `None` / `Err(e)` arm, and the consumer's
exhaustive `match` (the COMPILE-TIME tooth, 01-adts REQ-5/REQ-12, SHIPPED) forces
every arm to be HANDLED or an explicit `Wildcard` to SCREAM. `Result`'s `Err(e)`
carries a *typed, named* reason — louder than `Option`'s bare `None`. The success
arm's round-trip `ens` (`parse_be(s) == v`) is the §6 L1 contract that aborts
(exit 101) on a lie at runtime — GROUNDED non-vacuous: a broken `Some(0)` /
`Ok(0)` FAILS verus (the error arm bites).

## Requirements

### Surface + AST (governs `thermite-syntax/src/ast.rs`, `parser.rs`)

- **REQ-1 (built-in `Option<T>` — `Some`/`None` construct + match + `is`):** The
  surface admits `Some(v)` / `None` in EXPRESSION position (construction) — today
  matchable but not constructible. `Some(v)` is `Expr::Call { callee:
  Path([Some]), args: [v] }`; `None` is `Expr::Path([None])` — both EXISTING
  expression nodes (`ast.md` REQ-6), no reshape. `Option<T>` parses to a dedicated
  `Type::Option(Box<Type>)` node (OQ-1; mirrors the `Type::Vec`/`Box`/`String`
  dedicated-node precedent so the lowerer keys on the node kind). The validator
  registers `Some`/`None` as **built-in variants** so construction / `match` / `is`
  is not rejected as `UnknownVariant` (REQ-3). Derived from §4 (the surface), §4.1
  (`match result { Some(i) … }` postconditions), §4.4 (closed built-in interface
  set), and the GROUNDED `Some(5)`/`None` construct + verify.

- **REQ-2 (built-in `Result<T, E>` — type + `Ok`/`Err` construct + match + `is`):**
  The surface admits the two-type-arg type `Result<T, E>` and its constructors
  `Ok(v)` / `Err(e)`. `parser::parse_type` gains the two-arg parse — DECIDED a
  dedicated `Type::Result(Box<Type>, Box<Type>)` node (the load-bearing AST/parser
  change of C7; OQ-1). `Ok(v)` / `Err(e)` are `Expr::Call { callee: Path([Ok]) /
  Path([Err]), args }` (EXISTING nodes); `match r { Ok(v) => …, Err(e) => … }`
  reuses `Expr::Match` + `Pattern::Enum`; `r is Ok` reuses `Expr::Is`. The
  validator registers `Ok`/`Err` as built-in variants (REQ-3). Derived from §4,
  §4.1, §4.4, and the GROUNDED `Result<u64, ParseErr>` construct + match + payload
  verify.

### Validator / the SpecTherm cage (governs `thermite-spec/src/validator.rs`)

- **REQ-3 (built-in-variant registry + spec-`match`-in-`ens` payload projection in
  the cage):** The validator's declaration pre-pass (`Validator::new`, which today
  builds `enums` / `variant_to_enum` from `Item::Enum`,
  `.design/basis/01-adts.md` REQ-5/REQ-6 SHIPPED) SEEDS the built-in variants
  `Some`/`None` (→ enum `Option`) and `Ok`/`Err` (→ enum `Result`) into the same
  registry, so construction, `match` arms, and `Expr::Is` over them are ACCEPTED
  (not `UnknownVariant`), and a `match` over them is exhaustive iff both arms (or a
  `Wildcard`) are present (REQ-5 of 01-adts, unchanged). The **payload-in-contract
  projection** is the spec-`match`-in-`ens`: a `match result { Some(v) => <flat
  pred over v>, None => true }` in a `req`/`ens`/`inv` clause is admitted as a FLAT
  built-in (`.design/basis/01-adts.md` REQ-7, SHIPPED — `walk_expr_inner`'s `Match`
  arm recurses operands as a flat built-in, NOT a combinator). The bound payload
  `v` is in scope for the arm body; the arm body is a flat predicate (comparisons,
  arithmetic, named `spec fn` calls — never an anonymous nested quantifier, §4.2).
  C7 adds NO new cage walk — it adds the built-in variants to the registry and
  confirms the spec-`match` is admitted. Derived from §4.2 (the cage),
  `.design/basis/01-adts.md` REQ-5/REQ-6/REQ-7, and the GROUNDED spec-`match`-in-
  `ens` verify.

### Verus lowering (governs `thermite-lower/src/lower.rs`)

- **REQ-4 (`Option`/`Result` → the Verus-native types; constructors / match / `is`
  / spec-`match` lower):** `Type::Option(T)` lowers to Verus `Option<T>` and
  `Type::Result(T, E)` to `Result<T, E>` (`lower_type`). `Some(v)`/`None`/`Ok(v)`/
  `Err(e)` lower to the bare Verus constructors — Verus's prelude carries them, so
  `qualify_variant_path` must NOT enum-qualify a built-in variant (it qualifies
  only USER-declared enum variants via `enum_of_variant` today; `Some`/`Ok`/etc.
  fall through to the bare name, which is exactly what Verus wants — GROUNDED). A
  `match` over `Option`/`Result` lowers to a Verus `match` with bare arm patterns;
  `result is Some`/`result is Ok` lowers to the Verus-native `is` discriminant
  (`.design/basis/01-adts.md` REQ-9, SHIPPED for user enums — the same path). The
  spec-`match`-in-`ens` lowers to a Verus `match` expression in the `ensures`
  clause (GROUNDED `5 verified, 0 errors`). The L1 mirror (`l1.rs`) lowers a
  built-in `match`/`is` exactly as the user-enum form (`matches!`). Derived from §3
  (transpile to Verus), §4.4, §6, and the GROUNDED `Option`/`Result`/`parse_u64`
  verify.

- **REQ-5 (`parse_u64` — `String`→`u64`, the C4 REQ-9 payoff, ships under C7):**
  With REQ-1..REQ-4 landed, `parse_u64(s: &String) -> Option<u64>` ships: the
  Horner-accumulate loop (`acc = acc*10 + digit`), the three handled-or-loud `None`
  arms (empty / non-digit / overflow — each screams BEFORE corrupting `acc`), and
  the round-trip success contract written as the spec-`match`-in-`ens`:
  ```thermite
  ens match result { Some(v) => all_digits(s) && s.len() >= 1 && parse_be(s) == v, None => true }
  ```
  The lowerer emits the `parse_be`/`all_digits`/`is_digit` spec fns (seeded into
  `GENERATED_SPEC_FNS` alongside the C4 `parse_le`/`pow10`, so the contract
  validates inside the cage as named `spec fn` calls — the C4 #94 precedent) plus
  the loop invariant + `decreases s.len() - i` + the subrange ghost glue. This is
  PURE (`fx pure` — `parse_u64` reads bytes, allocates nothing). **GROUNDED `5
  verified, 0 errors`; the broken `Some(0)` FAILS `3 verified, 1 errors`** — the
  round-trip ens has teeth. Derived from §4.2 (partiality, the cage),
  `.design/basis/07-strings.md` REQ-9 (the deferred C4 contract), the
  handled-or-loud principle, and the GROUNDED `parse_u64` verify.

- **REQ-6 (`LowerError`/`SpecError` extension, no panics):** The C7 constructs
  reuse the EXISTING `thermite-lower::LowerError` (`Unsupported`/`TooDeep`) and
  `thermite-spec::SpecError` (no new validator reject mode beyond admitting the
  built-in variants — a still-`UnknownVariant` name like `Smoe` continues to
  reject). No new variant is expected; if a C7-specific failure surfaces it is a
  span-bearing variant on the existing enums. No `unwrap`/`expect`/`panic!` in
  production (R-CODE-2 / R-APG-1). Derived from R-CODE-2 and the existing
  error-enum discipline.

## Combinators / `?` — v1 vs DEFERRED (honest)

The CORE C7 ships **construct + match + `is` + payload-in-contract** — exactly
what `parse_u64` (and `find → Option`, C5; the calculator acceptance program)
need. The ERGONOMIC layer is **DEFERRED** out of C7, honestly:

- **Combinators (`map` / `unwrap_or` / `and_then`):** DEFERRED to a follow-up.
  They are convenience over `match` — every one desugars to a `match` the user can
  already write. Per §4.4 ("one desugaring, always explicit") and the
  build-leaves-first discipline, C7 does not ship them; the calculator/parse_u64
  path uses explicit `match`. A future cluster may add them as built-in methods
  (the `BUILTIN_METHODS` precedent), each with a GROUNDED contract.
- **The `?` operator:** DEFERRED, and graded HARD. `?` is early-return-on-`Err`/
  `None` sugar — it desugars to a `match` that `return`s the `Err`/`None` arm.
  This interacts with the FUNCTION's contract (the `ens` must hold on the
  early-return path too) and with the effect row / loop `decreases` — it is NOT a
  local rewrite. It needs its own grounding pass (the early-return `ens`
  obligation) and likely a new statement/expr node + a match-arm ripple. C7 does
  NOT ship `?`; the explicit `match … { Err(e) => return Err(e), Ok(v) => v }` is
  the v1 spelling. Pinned as a future cluster, not a C7 REQ. (Honest note: `?`
  feasibility is the least-certain item in this doc — see Report.)

## Acceptance criteria

The orchestrator authors a NEW corpus program — `conformance/option_result.th`
(`Some`/`None`/`Ok`/`Err` construct + match + `is` + a payload-in-contract `ens`,
certifying L3) and extends the C4 string corpus with `conformance/parse_u64.th`
(the `String`→`Option<u64>` parser, certifying L3 pure). Their golden lowerings
live at `tests/golden/lower/option_result.verus.rs` / `tests/golden/lower/
parse_u64.verus.rs`, hand-authored from the GROUNDED forms below and confirmed to
pass `verus`. The cert goldens live at `conformance/option_result.cert.json` /
`conformance/parse_u64.cert.json`.

- **AC-1 (`Option` construct + payload-in-contract certifies L3):** `fn f() ->
  Option<u64> ens match result { Some(v) => v == 5, None => true } { Some(5) }`
  parses (`Some(5)` → `Expr::Call`, `Option<u64>` → `Type::Option`), validates
  (the built-in variant registry accepts `Some`; the spec-`match` is a flat
  built-in), lowers to a Verus `Option<u64>` + the spec-`match`-in-`ens`, and the
  real `verus` binary exits 0 with `N verified, 0 errors`. A `None`-returning fn
  with `ens result is None ==> true` also certifies. **GROUNDED `4 verified, 0
  errors`.** (REQ-1, REQ-3, REQ-4.)

- **AC-2 (`Result<T,E>` parses + construct + match + payload certifies L3):**
  `Result<u64, ParseErr>` PARSES (the two-arg `Type::Result` — the change this AC
  pins); `Ok(7)`/`Err(ParseErr::NotDigit)` construct; `match r { Ok(v) => …, Err(_)
  => … }` validates exhaustively and lowers; `ens match result { Ok(v) => v == 7,
  Err(_) => true }` certifies L3. **GROUNDED `3 verified, 0 errors`** (`ok7`,
  `ok7b` via `result->Ok_0`, `errpath`). (REQ-2, REQ-3, REQ-4.)

- **AC-3 (the error arms BITE — non-vacuity):** A broken `f` returning `Some(0)`
  under `ens match result { Some(v) => v == 5, None => true }` FAILS `verus`, and a
  broken `Ok(0)` under the analogous `Result` ens FAILS — the payload contract is
  real, not vacuous (R-DEFER-9). **GROUNDED: `Some(0)` → `1 verified, 1 errors`;
  `Ok(0)` → `4 verified, 1 errors`.** (REQ-3, REQ-4.)

- **AC-4 (`parse_u64` ships, certifies L3 pure, the C4 REQ-9 payoff):**
  `parse_u64(s: &String) -> Option<u64>` parses, validates (the spec-`match`-in-
  `ens` round-trip is a flat built-in; `parse_be`/`all_digits` are seeded spec
  fns), lowers to the Horner loop + the three `None` arms + the round-trip `ens`,
  and the real `verus` binary exits 0 with `N verified, 0 errors`. The broken
  `parse_u64` returning `Some(0)` unconditionally FAILS. **GROUNDED `5 verified, 0
  errors`; broken → `3 verified, 1 errors`.** `.design/basis/07-strings.md` REQ-9's
  status flips NOT-STARTED → SHIPPED once this lands. (REQ-1, REQ-3, REQ-4, REQ-5.)
  **LANDED #100 (the external cert/golden oracle):** the corpus program
  `conformance/parse_u64.th` (`parse_valid` — a valid in-range digit string PROVES
  `result is Some` via parse_u64's STRENGTHENED caller-usable contract) certifies L3
  matching `conformance/parse_u64.cert.json`; the golden lowering
  `tests/golden/lower/parse_u64.verus.rs` is `34 verified, 0 errors`. The forced-None
  refusal demo is NOT in the corpus (under `req !all_digits` the real body provably
  returns None, so an always-None mutant is behaviorally EQUIVALENT and the §7 gate —
  which lacks equivalent-mutant exclusion — cannot distinguish it: a tracked §7
  limitation, #101, not a parse_u64 defect).

- **AC-5 (existing corpus unchanged — no regression):** `conformance/sum.th`,
  `binary_search.th`, the ADT corpus (`bank_account`/`shape`/`list_sum`), the
  collections / string corpus, and their `.cert.json` / `tests/golden/lower/
  *.verus.rs` goldens are UNCHANGED — they parse, validate, lower byte-stable, and
  certify L3. C7 is purely additive: the new `Type::Option`/`Type::Result` nodes,
  the built-in-variant registry seeding, and the bare-constructor lowering touch no
  existing node shape or user-enum path. The `match`/`is`/spec-`match` over USER
  enums (01-adts) is unchanged (built-in variants fall through `qualify_variant_path`
  to the bare name; user variants still enum-qualify). Mechanically: `cargo test -p
  thermite-syntax -p thermite-spec -p thermite-lower` and the conformance corpus
  pass with 0 mismatches. (All REQs; C7 must not break the kernel.)

## Architecture

C7 spans three crates, additively, mirroring the 01-adts layer split:

- **`thermite-syntax`** — `enum Type` gains `Option(Box<Type>)` and `Result(Box<Type>,
  Box<Type>)` (the two-arg node, the load-bearing parser change); `parse_type`'s
  `Ident` arm gains `Option`/`Result` contextual-ident arms (mirroring the
  `Vec`/`Box`/`String` arms) — `Result` parses `<T, E>` (a comma + second type +
  `>`), the FIRST two-arg type in the grammar. Construction (`Some`/`None`/`Ok`/
  `Err`), `match`, and `is` reuse the EXISTING `Expr::Call`/`Path`/`Match`/`Is`
  nodes (no reshape) and `Pattern::Enum` (no reshape).

- **`thermite-spec`** — `validator.rs`'s declaration pre-pass (`Validator::new`)
  seeds the built-in variants `Some`/`None` (enum `Option`), `Ok`/`Err` (enum
  `Result`) into `enums` / `variant_to_enum`, so construction / `match` / `is` over
  them is accepted (not `UnknownVariant`) and exhaustiveness (01-adts REQ-5)
  applies. The caged-flat walk (`walk_expr_inner`'s `Match` arm, 01-adts REQ-7) is
  UNCHANGED — a spec-`match`-in-`ens` is already an admitted flat built-in.
  `parse_be`/`all_digits`/`is_digit` join `GENERATED_SPEC_FNS` (the C4 precedent)
  for `parse_u64`'s contract.

- **`thermite-lower`** — `lower.rs`'s `lower_type` gains `Type::Option` →
  `Option<T>` and `Type::Result` → `Result<T, E>`; `qualify_variant_path` leaves
  built-in `Some`/`Ok`/`Err`/`None` UNQUALIFIED (bare names Verus's prelude
  carries) — only user-enum variants enum-qualify. `lower_match`/`lower_pattern`/
  the `Expr::Is` arm carry the built-in variant arms through unchanged. The
  spec-`match`-in-`ens` lowers as a Verus `match` expression in `ensures`. `lower`
  emits `parse_u64` (the Horner loop + the `parse_be`/`all_digits`/`is_digit` spec
  fns) and the `option_result` demo. The L1 mirror (`l1.rs`) lowers a built-in
  `match`/`is` exactly as the user-enum form.

Symbol anchors: `enum Type` (`Option`/`Result`), `enum Expr` (`Call`/`Path`/`Match`/
`Is`), `enum Pattern` (`Enum`) in `ast.rs`; `fn parse_type` in `parser.rs`;
`fn validate` + `Validator::new` + `GENERATED_SPEC_FNS` in `validator.rs`;
`fn lower` / `lower_type` / `lower_match` / `lower_pattern` / `qualify_variant_path`
in `lower.rs`.

### The verified Verus forms (GROUNDED — the lowering contract, not guesses)

Produced by the real `verus 0.2026.05.24` binary during authoring (Verification).
These are the seed for the golden files.

**Option construct + payload-in-contract (REQ-1/REQ-3/REQ-4).** Both projection
forms verify (`4 verified, 0 errors`):

```verus
fn f() -> (result: Option<u64>)
    ensures match result { Some(v) => v == 5, None => true },   // spec-match-in-ens (DECIDED form)
{ Some(5) }

fn g() -> (result: Option<u64>)
    ensures result is Some ==> result->Some_0 == 5,             // projection-operator (rejected form, verifies but costs a new node)
{ Some(5) }

fn h() -> (result: Option<u64>)
    ensures result is None ==> true,
{ None }
```

**Result construct + match + payload (REQ-2/REQ-3/REQ-4).** Verifies (`3 verified,
0 errors`; the broken `Ok(0)` arm FAILS):

```verus
enum ParseErr { NotDigit, Overflow, Empty }       // the E parameter — a user error enum

fn ok7() -> (result: Result<u64, ParseErr>)
    ensures match result { Ok(v) => v == 7, Err(_) => true },
{ Ok(7) }

fn errpath(bad: bool) -> (result: Result<u64, ParseErr>)
    ensures bad ==> result is Err,
{ if bad { Err(ParseErr::NotDigit) } else { Ok(1) } }
```

**`parse_u64` — the C4 REQ-9 payoff (REQ-5).** Verifies `5 verified, 0 errors`;
the broken `Some(0)` FAILS `3 verified, 1 errors`:

```verus
pub open spec fn is_digit(b: u8) -> bool { 48 <= b && b <= 57 }
pub open spec fn all_digits(s: Seq<u8>) -> bool
{ forall|i: int| 0 <= i < s.len() ==> is_digit(#[trigger] s[i]) }
pub open spec fn parse_be(s: Seq<u8>) -> nat decreases s.len()
{ if s.len() == 0 { 0 }
  else { parse_be(s.subrange(0, (s.len()-1) as int)) * 10 + ((s[s.len()-1] - 48) as nat) } }

pub fn parse_u64(s: &TString) -> (result: Option<u64>)
    requires s.well_formed(),
    ensures match result {
        Some(v) => all_digits(s.data@) && s.data.len() >= 1 && parse_be(s.data@) == v as nat,
        None => true,
    },
{
    if s.data.len() == 0 { return None; }                       // empty → LOUD None
    let mut acc: u64 = 0; let mut i: usize = 0;
    while i < s.data.len()
        invariant i <= s.data.len(),
                  all_digits(s.data@.subrange(0, i as int)),
                  parse_be(s.data@.subrange(0, i as int)) == acc as nat,
        decreases s.data.len() - i,
    {
        let b: u8 = s.data[i];
        if b < 48 || b > 57 { return None; }                    // non-digit → LOUD None
        let digit: u64 = (b - 48) as u64;
        if acc > (u64::MAX - digit) / 10 { return None; }       // overflow → LOUD None (before acc*10 wraps)
        let ghost old_i = i as int;
        assert(s.data@.subrange(0, (i+1) as int).subrange(0, old_i) == s.data@.subrange(0, old_i));
        assert(s.data@.subrange(0, (i+1) as int)[old_i] == b);
        acc = acc * 10 + digit; i = i + 1;
    }
    assert(s.data@.subrange(0, i as int) == s.data@);
    Some(acc)
}
```

**RECORDED FINDING (the C7 stack is end-to-end feasible, the C4 blocker clears).**
The payload-in-contract projection is expressible as a flat spec-`match`-in-`ens`
with zero new AST surface; `Option` construct, `Result<T,E>` construct + match +
payload, and `parse_u64`'s round-trip all verify NON-VACUOUSLY (every broken-body
companion FAILS). The ONLY surface change beyond the validator registry seeding is
the two-type-arg `Type::Result` (and the recommended dedicated `Type::Option`) in
the parser/AST — every other C7 construct reuses an EXISTING node. `parse_u64`
(`.design/basis/07-strings.md` REQ-9) ships under C7, pure, at L3.

## Verification

- **Mandatory Verus grounding (DONE during authoring — real `verus
  0.2026.05.24`):**
  - Option construct + both payload forms (`f`/`g`/`h`) → `4 verified, 0 errors`.
  - Broken `Some(0)` under the spec-`match` ens → `1 verified, 1 errors`
    (postcondition not satisfied — non-vacuous).
  - `Result<u64, ParseErr>` construct + match + `result->Ok_0` + `is Err`
    (`ok7`/`ok7b`/`errpath`) → `3 verified, 0 errors`; broken `Ok(0)` →
    `4 verified, 1 errors`.
  - `parse_u64` spec-`match`-in-`ens` form → `5 verified, 0 errors`; the
    `result->Some_0` form → `5 verified, 0 errors` (both surfaces equivalent);
    broken `Some(0)` → `3 verified, 1 errors`.
  - Cheat-token grep (`assume`/`external_body`/`admit`/`verifier::external`) over
    every probe: NONE.
- **AC-1/AC-2/AC-4:** `cargo test -p thermite-syntax -p thermite-spec -p
  thermite-lower`, plus a harness shelling the real `verus` binary on the emitted
  lowering of `option_result.th` / `parse_u64.th` asserting exit 0 + `N verified, 0
  errors` (R-CODE-4), plus `forge check` matching the cert goldens.
- **AC-3:** the broken-body negatives (a `Some(0)`/`Ok(0)` whose emitted lowering
  FAILS verus), pinning non-vacuity (R-DEFER-9; GROUNDED).
- **AC-5:** the existing `tests/golden/lower/*.verus.rs` + `*.cert.json` assertions
  stay green (no regression).

Gauntlet (R-DEFER-6, per crate): `cargo test -p <crate>`, `cargo clippy -p <crate>
--all-targets -- -D warnings`, `cargo fmt --check`.

## Routes to add (orchestrator)

C7 adds NEW concerns to files that already carry routes; add these routes to
`tooling/spec-routes.toml` pointing at THIS doc (a file may carry multiple
governing docs — the `lower.rs` precedent):

```
[[route]]  crate_pattern = "thermite-syntax/src/ast.rs"      design = ".design/basis/09-option-result.md"  reference = ["conformance/option_result.th", "conformance/parse_u64.th"]
[[route]]  crate_pattern = "thermite-syntax/src/parser.rs"   design = ".design/basis/09-option-result.md"  reference = ["conformance/option_result.th", "conformance/parse_u64.th"]
[[route]]  crate_pattern = "thermite-spec/src/validator.rs"  design = ".design/basis/09-option-result.md"  reference = ["conformance/option_result.th"]
[[route]]  crate_pattern = "thermite-lower/src/lower.rs"     design = ".design/basis/09-option-result.md"  reference = ["tests/golden/lower/option_result.verus.rs", "tests/golden/lower/parse_u64.verus.rs"]
```

The corpus programs, their `.cert.json` goldens, and the `tests/golden/lower/
*.verus.rs` lowerings are authored by the orchestrator from this doc before the
builder runs (R-CHAR-3), seeded from the GROUNDED forms above.

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (built-in `Option<T>` — `Some`/`None` construct + match + `is`) | SHIPPED | #95 C7. `enum Type` gains `Option(Box<Type>)` (`thermite-syntax/src/ast.rs`, OQ-1 dedicated node — `Option` STOPS being a string-named `Generic`); `parse_type`'s `"Option"` arm builds it (`parser.rs`). `Some(v)`/`None` reuse the EXISTING `Expr::Call`/`Path` nodes; the validator's `Validator::new` SEEDS `Some`/`None` (enum `Option`) into `enums`/`variant_to_enum` so construction/`match`/`is` ACCEPT (no `UnknownVariant`). Consumer: `lower` (`lower_type` `Type::Option` arm → Verus `Option<T>`). Verified: `forge/tests/option_result_conformance.rs::ac1_option_construct_payload_in_contract_certifies_l3` (real verus L3, `Some(5)` + the payload `ens`). |
| REQ-2 (built-in `Result<T, E>` — type + `Ok`/`Err` construct + match + `is`) | SHIPPED | #95 C7. `enum Type` gains `Result(Box<Type>, Box<Type>)` (the FIRST two-type-arg node; `ast.rs`); `parse_type`'s `"Result"` arm parses `<T, E>` (the comma + second type, `parser.rs`). `Ok(v)`/`Err(e)` reuse `Expr::Call`; `Validator::new` seeds `Ok`/`Err` (enum `Result`). Consumer: `lower` (`lower_type` `Type::Result` arm → Verus `Result<T, E>`). Verified: `forge/tests/option_result_conformance.rs::ac2_result_two_arg_type_construct_payload_certifies_l3` (real verus L3, `Result<u64, ParseErr>` parses + `Ok(7)` + payload `ens`). |
| REQ-3 (built-in-variant registry + spec-`match`-in-`ens` payload projection) | SHIPPED | #95 C7. `Validator::new` (`thermite-spec/src/validator.rs`) seeds the built-in variants `Some`/`None`→`Option`, `Ok`/`Err`→`Result` into `enums`/`variant_to_enum` (order `[Some, None]`/`[Ok, Err]` pins the exhaustiveness `missing` set), AFTER the user pre-pass (a user re-decl wins). The spec-`match`-in-`ens` needs NO new cage rule — `walk_expr_inner`'s `Match` arm already admits a flat `match` as a built-in (01-adts REQ-7), so `match result { Some(v) => <flat pred>, None => true }` in an `ens` is an accepted flat predicate once the variants are registered. `GENERATED_SPEC_FNS` += `all_digits`/`is_digit` for `parse_u64`'s witness. Consumer: `pub fn validate`. Verified: `forge/tests/option_result_conformance.rs` (AC-1/AC-2 L3) + `ac3_broken_some_under_payload_ens_is_rejected` (the payload constrains — non-vacuous). |
| REQ-4 (`Option`/`Result` → Verus types; construct/match/`is`/spec-match lower) | SHIPPED | #95 C7. `lower_type` (`thermite-lower/src/lower.rs`) gains `Type::Option(T)` → `Option<T>` and `Type::Result(T, E)` → `Result<T, E>` (the Verus-native generics, no wrapper); `qualify_variant_path` leaves the built-in `Some`/`Ok`/`Err`/`None` UNQUALIFIED (they are not in the user `variants` map, so they fall through to the bare name Verus's prelude carries — GROUNDED). The L1 mirror (`l1.rs::lower_type`) lowers them as the native Rust generics; `l2.rs::type_label` labels them. The spec-`match`-in-`ens` lowers via the EXISTING `lower_expr` `Match` arm (`lower_match`). Consumer: `lower`. Verified: AC-1/AC-2/AC-4 (real verus L3 / `5 verified, 0 errors`). |
| REQ-5 (`parse_u64` — `String`→`u64`, the C4 REQ-9 payoff, ships under C7) | SHIPPED | #95 C7, STRENGTHENED #100. `lower.rs::emit_parse_defs` emits the `is_digit`/`all_digits`/`parse_be` spec fns + the new monotonicity lemma `lemma_parse_be_prefix_le` (`parse_be(s.subrange(0,k)) <= parse_be(s)`, induction on the suffix + `by(nonlinear_arith)`) + the `parse_u64(s: &TString) -> Option<u64>` exec fn (the Horner-accumulate loop, the BE partial-value invariant + all-digits prefix witness + `decreases s.data.len() - i`, the three handled-or-loud `None` arms — empty / non-digit / overflow, each screaming BEFORE corrupting `acc`). The contract is now CALLER-USABLE (#100): (1) `(all_digits(s.data@) && s.data.len() >= 1 && parse_be(s.data@) <= u64::MAX) ==> result is Some` (a caller with that `req` discharges `ens result is Some`), (2) the round-trip success `Some(v) => all_digits(s.data@) && s.data.len() >= 1 && parse_be(s.data@) == v as nat`, (3) the refusal `result is None ==> (!all_digits || s.data.len() == 0 || parse_be > u64::MAX)` (the overflow arm lifts the prefix witness to the whole input via `lemma_parse_be_prefix_le`). The EXEC borrow-rewrite: an owned-`String` arg to `parse_u64` (which takes `&TString`) lowers to `parse_u64(&s)` (`Ctx::owned_strings`/`is_owned_string`/`with_owned_strings` + `owned_string_value_names`). Materialized when `program_uses_parse`; `parse_be` deduped against the numfmt emission. NO `assume`/`external_body`/`admit` (R-DEFER-9). Consumer: `lower`. Verified: the EXTERNAL cert oracle `forge/tests/check_conformance.rs::parse_valid_cert_matches_golden_deterministic_subset` (`forge check conformance/parse_u64.th` → `parse_valid` L3, stable subset == `conformance/parse_u64.cert.json`) + the golden lowering `tests/golden/lower/parse_u64.verus.rs` (`34 verified, 0 errors`: the strengthened contract + the lemma + the `parse_valid`/`parse_rejects_nondigit` callers) + `forge/tests/option_result_conformance.rs` (broken `Some(0)` FAILS, non-vacuous). **BUILD-SIDE L1-EXEC-TWIN (#104):** `forge build` lowers EVERY fn to L1 (`thermite-design.md` §6), so a contract NAMING `is_digit`/`all_digits`/`parse_be` or a body calling the free `parse_u64` needs a runnable EXEC twin to evaluate the runtime `thermite_check!` — these now exist in `thermite-lower::l1::emit_string_runtime_l1` (the C7 block gated on `program_uses_parse`), each computing the same value as its spec body over the runtime `TString` (`Vec<u8>`), NO verus proof (the L1 path is runtime-checked). The calculator acceptance program `add` (its `req`/`ens` name `all_digits`/`parse_be`, body calls `parse_u64`) now `forge build`s + RUNS end-to-end (`forge/tests/acceptance_programs.rs::calculator_string_parse_builds_and_runs_end_to_end`, 2+3→Some(5), 100+200→Some(300)); `forge check` is UNCHANGED (L3 — #104 touched only the L1/exec mirror). |
| REQ-6 (`LowerError`/`SpecError` extension, no panics) | SHIPPED | #95 C7. The C7 constructs reuse the EXISTING `LowerError` (`Unsupported`/`TooDeep`) and `SpecError` (no new validator reject mode beyond the registry seeding — a genuinely undeclared variant like `Smoe` still rejects `UnknownVariant` via `check_variant_ref`). No new variant was needed; no `unwrap`/`expect`/`panic!` in production (R-CODE-2 / R-APG-1 — the anti-pattern gate is clean over every C7 edit). Consumer: the existing error paths. Verified: `cargo clippy --workspace --all-targets -- -D warnings` clean + the gauntlet. |

## Open questions (for the orchestrator before the builder runs)

- **OQ-1 (least-confident: the two-type-arg `Type` shape for `Result<T,E>` — and
  `Option`):** REQ-2 needs `Result<T, E>` to parse, which the single-arg
  `Type::Generic { name, arg }` cannot. Two shapes: a dedicated
  `Type::Result(Box<Type>, Box<Type>)` + `Type::Option(Box<Type>)` (RECOMMENDED —
  additive, the lowerer/validator key on the node kind, mirrors the
  `Type::Vec`/`Box`/`String` precedent, no reshape of the existing `Generic`), OR a
  generalized `Type::Generic { name, args: Vec<Type> }` (a *reshape* — `Option<usize>`
  is `Generic` today, so this ripples every `match Type` site reading `Generic { arg
  }`). The dedicated-node form is recommended and is the load-bearing AST/parser
  change of C7 — the highest-judgment, least-confident part: the dedicated form is
  cleaner but means `Option<T>` STOPS being a `Generic` (a small ripple at every
  `Type::Generic { name: "Option", .. }` reader, if any exist). The builder should
  grep for `Generic` readers before choosing. Not a blocker; pinned for the builder.

- **OQ-2 (the spec-`match`-in-`ens` `None`/`Err` arm is `true` — vacuity-gate
  interaction):** The payload contract's failure arm is `None => true` / `Err(_) =>
  true`. The §7 vacuity battery (which rejects `ens true`) must NOT flag the WHOLE
  spec-`match` as vacuous just because one arm is `true` — the contract is
  non-vacuous overall (the `Some`/`Ok` arm constrains the payload, GROUNDED: a
  broken body FAILS). The builder/critic should confirm the vacuity triage reasons
  over the `match` as a whole (the non-`true` arm carries teeth), not per-arm. Not a
  blocker (GROUNDED non-vacuous), but a real check for the vacuity component.

- **OQ-3 (combinators / `?` — confirmed DEFERRED):** the doc pins `map`/`unwrap_or`/
  `and_then` and `?` OUT of C7 (Combinators section). The orchestrator confirms C7
  ships the CORE only (construct + match + `is` + payload-in-contract + `parse_u64`).
  `?` is graded hard (early-return `ens` obligation + a likely new node + ripple) and
  is the least-certain future item. Recorded; not a C7 blocker.
