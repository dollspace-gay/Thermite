# FFI Boundary Modules — L1-enforced foreign contracts
<!--
tier: 3-component
status: draft
governs: thermite-syntax/src/ast.rs, thermite-syntax/src/parser.rs, thermite-lower/src/l1.rs, forge/src/check.rs, forge/src/manifest.rs
thesis-refs:
  - thermite-design.md §9
  - thermite-design.md §8
  - thermite-design.md §6
  - thermite-design.md §2.2
  - thermite-design.md §4.4
-->

## Summary

A `crates.io` dependency is imported through a **boundary module**: each foreign
function gets a Thermite signature (`req`/`ens`/`fx`) but **no Thermite body** — the
body is the foreign crate's. Because a foreign body cannot be proved, the contract
is **enforced at L1** (runtime checks on every crossing; `thermite-design.md` §9),
and the function certifies at `Level::L1` with a `boundary` flag plus the foreign
target. A boundary fn is the FFI analog of `#[slag]` (§8): an unproven body, a
mandatory contract that is still stated and checked. This component spans three
crates — the surface form (`thermite-syntax`), the L1 wrapper (`thermite-lower`),
and the cert (`forge`). It is **greenfield**: there is no foreign/extern/boundary
form anywhere in the grammar, AST, parser, lowerer, or forge today (verified
below). All REQs are NOT-STARTED, blocked on **crosslink issue #16** (v0.4,
milestone #4).

## Decided scope

Issue #16 = the boundary-fn **declaration form** + its **L1-enforced contract** +
its appearance in the **cert** (a `boundary` flag + the foreign target). The
foreign body itself is trusted-by-fiat (`goal.md` R-DEFER-9: trusted-by-fiat, but
the contract is checked — never *silently* trusted), exactly as a `#[slag]` body
is. Explicitly OUT of #16 (noted as boundaries, never deferred-as-status):

- The "verified to the boundary" vs "verified, period" manifest distinction is
  **#17**. This doc's cert flag is the input that distinction will read; #17 is the
  consumer.
- The full audit manifest v1 (TCB enumeration = slag blocks ∪ boundary contracts ∪
  the toolchain) is **#15**. This doc pins the *hook* #15 reads (the per-cert
  `boundary` flag + target, the boundary analog of the `slag: true` flag the
  existing `slag.md` audit-surface REQ already feeds).

## The surface form (the key design choice)

A boundary fn is a `fn` with a contract but **no Thermite body** — the body is
foreign. The chosen form is the **skill-budget-minimal (a) variant**: a bodyless
`fn` carrying a new `#[boundary("crate::path")]` attribute, terminated by `;`:

```thermite
#[boundary("regex::Regex::is_match")]
fn re_is_match(re: &Regex, hay: &[u32]) -> bool
  req true
  ens true
  fx  pure
;
```

**Why (a), not a new `boundary`/`extern` keyword (`thermite-design.md` §2.2 — the
6,000-token skill budget; pillar §2.3 "one way to do everything"):**

- It reuses the *entire* existing `fn` + contract grammar (`parse_fn`,
  `parse_contract`, `parse_params`, `parse_type`). The skill gains one attribute
  (`#[boundary("…")]`) and one terminator rule (a `;` body), not a new item kind
  with its own keyword, its own clause-order rules, and its own skill paragraph. A
  new keyword roughly doubles the surface a boundary fn adds to the skill; the
  attribute reuses the `#[slag(…)]` shape an agent already knows.
- It is the exact precedent of `#[slag]` (§8): an attribute that marks a fn whose
  body is body-unproven while leaving the contract mandatory. An agent who knows
  `#[slag]` reads `#[boundary]` with zero new mental model — "the body is
  elsewhere/foreign, the contract is still checked at L1."
- The `;`-body unifies cleanly: a Thermite fn's body is *either* `{ … }` (proved at
  L3 / runtime-checked at L1 with a real body) *or* `;` (foreign — only the L1
  wrapper around the contract is emitted). One distinction, structurally encoded.

The `#[boundary("…")]` argument is the **foreign target**: a single string literal
naming the `crate::path::to::foreign_fn` the wrapper calls. (OQ-1 below: whether
the target is one positional string, as `#[slag]` uses `key = "value"` fields — the
minimal form is a single positional string literal, distinct from slag's named
fields, because a boundary has exactly one datum.)

A boundary fn `req`/`ens`/`fx` remain **mandatory** (the §4.1 rule is unchanged):
the parser's existing `parse_contract` already rejects a missing `req`/`ens`/`fx`,
so a bodyless fn with no contract is a parse error for free.

### Exact `ast.rs` / `parser.rs` additions (greenfield — verified absent)

Empirically confirmed against the current crate (a probe of
`thermite_syntax::parse`):

- A bodyless `fn foo(x: u32) -> u32 req true ens result == x fx pure ;` → parse
  error `Unexpected { expected: "`{`", found: "`;`" }` — `parse_fn` calls
  `parse_block` in `parser.rs`, which `consume`s a `{`.
- `#[boundary("…")] fn …` → parse error `Unexpected { expected: "`slag`", found:
  "identifier `boundary`" }` — `parse_slag` in `parser.rs` hardcodes `name ==
  "slag"` and `parse_item` only routes `HashBracket` into `parse_slag`.
- A normal `fn … { x }` parses clean (the control).

So the boundary form needs (all in `thermite-syntax`, governed by
`.design/syntax/ast.md` + `.design/syntax/parser.md` jointly with this doc):

1. **`ast.rs`** — a `struct BoundaryAttr { target: String, span: Span }` (the
   foreign-target string, mirroring the existing `struct SlagAttr`), and a field
   on `FnItem` to mark a boundary fn. The minimal, type-honest choice is
   `FnItem.boundary: Option<BoundaryAttr>` (a fn is a boundary fn iff `Some`),
   paralleling the existing `FnItem.slag: Option<SlagAttr>`. The body must become
   optional for a foreign fn: `FnItem.body: Option<Block>` (a foreign fn has
   `None`; an in-language fn has `Some`). A `boundary.is_some()` fn MUST have
   `body == None` and an in-language fn MUST have `body == Some` — a structural
   invariant the parser upholds (an `Item` distinction is NOT needed; the
   `Option<BoundaryAttr>` + `Option<Block>` pair on `FnItem` suffices and keeps the
   downstream `Item::Fn` match arms intact). `BoundaryAttr` is exported from
   `lib.rs` like `SlagAttr`.

2. **`parser.rs`** — three small extensions:
   - `parse_attribute` (generalize `parse_slag`): after `#[` read the attribute
     name; `slag` → the existing `SlagAttr` path; `boundary` → read a single
     `(` STRING `)` and build `BoundaryAttr { target }`. `parse_item` routes the
     parsed attribute to `parse_fn`. A `#[boundary]` on a `spec fn` is an error
     (parallel to the existing `#[slag]`-on-`spec`-fn error).
   - `parse_fn` gains a bodyless path: after `parse_contract`, if the next token is
     `Semi` (the lexer already has `TokKind::Semi`, `;`), `consume` it and set
     `body = None`; else `parse_block` and set `body = Some`. A `#[boundary]`
     attribute REQUIRES the `;` form (a foreign fn with a `{ … }` body is an
     error — there is no Thermite body to prove); a fn with NO `#[boundary]`
     REQUIRES the `{ … }` form (a non-boundary bodyless fn is an error — the §4.1
     "body-second" rule).
   - Per-item recovery (`resync_to_item_boundary`) is unaffected: `#[` is already a
     resync boundary token, and the `;` terminator gives a clean item end, so a
     malformed boundary fn cannot bleed into the next item (pillar §2.5 locality).
     OQ-2 below records the one error-recovery interaction to confirm.

## L1 wrapper lowering (thermite-lower)

A boundary fn lowers to an **L1 wrapper** in `l1.rs` (`thermite-design.md` §9 "L1,
runtime checks on every crossing"; §6 L1 = always-active runtime checks). The
wrapper reuses `l1.rs`'s existing executable machinery exactly:

1. Emit the `fn <name>(<params>) -> <ret>` head (`emit_params`, `lower_type` —
   existing).
2. Check `req` on entry via the always-active `thermite_check!` macro
   (`emit_check`, `lower_expr_exec` — existing; the same `if !(cond) {
   thermite_contract_violation(…) }` the proved-body L1 path uses).
3. **Call the foreign function** named by `BoundaryAttr.target`, binding its return
   to `result`. This replaces the `let result = { <lowered body> }` of a normal
   L1 fn: `let result = <target>(<args>);`. The foreign body is **NOT** lowered,
   NOT verified, NOT proved — it is the unproven crossing (§9), exactly as a
   `#[slag]` body's *body* is exempt from proving.
4. Check each `ens` on exit against the bound `result` (`emit_check` over
   `f.contract.ens` — existing).
5. `fx` emits no runtime sandbox in v0.1 (deferred to #21, R-SPEC-5), identical to
   the proved-body L1 path.

The wrapper IS "the runtime checks on every crossing": `req` before the foreign
call, `ens` after. The new `thermite-lower` code is a `lower_boundary_fn_l1` arm in
`lower_l1`/`lower_fn_l1` that, when `f.boundary.is_some()` (equivalently `f.body ==
None`), emits steps 1–2-4-5 with step 3 in place of the body lowering. (OQ-3: a
boundary fn is NOT lowered to Verus by `lower.rs` at all — there is no body to
prove, so the L3 path skips it, mirroring how `check.rs` skips verus for a
`#[slag]` item.)

## forge cert (forge/src/check.rs + forge/src/manifest.rs)

A boundary fn certifies at **`Level::L1`** with a `boundary` flag + the foreign
target — NOT L3 (the foreign body is unproven), precisely mirroring the existing
`#[slag]` `Level::L1` + `slag: true` precedent (`slag.md` REQ-2;
`certificate-manifest.md`). The cert path:

- **`manifest.rs`** — an additive `boundary: bool` field on `Certificate` (default
  `false`, `#[serde(default)]` so the frozen golden `conformance/sum.cert.json`
  still deserializes — R-SPEC-2), plus an additive `boundary_target:
  Option<String>` (the foreign `crate::path`, `#[serde(default,
  skip_serializing_if = "Option::is_none")]`). A `Certificate::boundary_l1(item,
  effects, target)` constructor (modeled on the existing `Certificate::slag_l1`):
  `Level::L1`, `boundary: true`, `boundary_target: Some(target)`, a single
  discharged obligation recording "contract enforced at L1 (boundary); foreign body
  trusted by fiat" (no verus run), and `graduate_triage_clean()` (a boundary fn
  still passes §7.1 (a)/(b)/(c) triage). The `boundary` flag is VERDICT-relevant
  (it qualifies the L1 as "to-the-boundary, body unproven") and feeds #15/#17, so
  it joins `slag` in `oracle_subset` — the oracle subset becomes
  `(item, level, effects, slag, boundary)`. `boundary_target` is diagnostic and
  oracle-EXCLUDED (a prose path).

- **`check.rs`** — `gate_fn`/`check_file` gains a `boundary.is_some()` branch
  alongside the existing `slag.is_some()` branch: validate the target is non-empty,
  run §7.1 (a)/(b)/(c) triage (a boundary contract is still subject to triage —
  slag-adjacent, §9), then `Certificate::boundary_l1` WITHOUT invoking verus (no
  body to prove). This is the §9 composition rule made operational: `g` calling a
  boundary fn `f` sees only `f`'s contract, so `g`'s certificate is valid
  independent of `f`'s (foreign) body.

- **#15 TCB-enumeration hook** — the per-cert `boundary: true` + `boundary_target`
  is the boundary analog of `slag: true` + `slag_meta` that the existing audit
  surface enumerates. #15's audit manifest reads `slag ∪ boundary ∪ toolchain`;
  this doc supplies the `boundary` half. #17's "verified to the boundary vs
  verified, period" distinction reads the same `boundary` flag (a transitive
  closure with any `boundary: true` cert is "to the boundary", else "period").

## Requirements

- **REQ-1 (surface form — bodyless boundary fn)**: a boundary fn parses as a
  `#[boundary("crate::path")] fn NAME(params) -> ret req … ens … fx … ;` — a `fn`
  with a mandatory contract, a `#[boundary]` attribute naming the foreign target,
  and a `;` body. Derived from `thermite-design.md` §9 (boundary module = a foreign
  fn given a Thermite signature) + §2.2 (skill-budget-minimal surface) + §4.4
  (attributes).
- **REQ-2 (AST shape)**: the AST represents a boundary fn as `FnItem { boundary:
  Some(BoundaryAttr { target }), body: None, .. }` with the contract mandatory and
  the body absent. Derived from §9 + the §8 `#[slag]` attribute precedent.
- **REQ-3 (parser extension)**: `parse_attribute` accepts `#[boundary("…")]`,
  `parse_fn` accepts the `;` body, and the invariants hold (`#[boundary]` ⇒ `;`
  body, no `#[boundary]` ⇒ `{ }` body, `#[boundary]` not on `spec fn`); per-item
  recovery is preserved. Derived from §9 + pillar §2.5 (locality / per-item
  recovery).
- **REQ-4 (L1 wrapper lowering)**: a boundary fn lowers to an L1 wrapper —
  `thermite_check!` on `req` → call the foreign target binding `result` →
  `thermite_check!` on each `ens`; the foreign body is NOT lowered or verified.
  Derived from §9 ("L1, runtime checks on every crossing") + §6 (L1 = always-active
  runtime checks).
- **REQ-5 (forge cert — L1 + boundary flag + target)**: a boundary fn certifies at
  `Level::L1` with `boundary: true` + `boundary_target` (NOT L3), via
  `Certificate::boundary_l1`, after triage (a)/(b)/(c), with no verus run. Derived
  from §9 (slag-adjacent, contract-enforced-at-L1) + §6/§8 (the `L1 + slag: true`
  precedent).
- **REQ-6 (#15 TCB-enumeration hook)**: the per-cert `boundary` flag + target is
  the enumerable-TCB input #15's audit manifest reads (slag ∪ boundary ∪
  toolchain); a boundary fn is visible in `forge audit`. Derived from §9 ("the
  trusted computing base is enumerable").
- **REQ-7 (composition independence)**: a fn `g` that calls a boundary fn `f` only
  through `f`'s contract certifies independent of `f`'s foreign body — `g`'s
  certificate does not change if `f`'s foreign target changes. Derived from §9 (the
  composition rule).

## Acceptance criteria

ACs tie to a `conformance/boundary/` oracle the orchestrator authors (a
`boundary.th` example program + a `boundary.cert.json` golden cert + a parse oracle
entry). The exact example program:

```thermite
#[boundary("ext::foreign_id")]
fn foreign_id(x: u32) -> u32
  req x <= 1000
  ens result == x
  fx  pure
;

fn caller(x: u32) -> u32
  req x <= 1000
  ens result == x
  fx  pure
{
  foreign_id(x)
}
```

(`foreign_id` is a boundary fn; `caller` is a pure-Thermite fn whose cert is valid
through `foreign_id`'s contract alone — the §9 composition witness. After the
parser extension lands this PARSES; it does NOT parse today — verified above.)

- **AC-1 (parses)**: `thermite_syntax::parse` of `boundary.th` returns
  `errors.is_empty()` and a `Program` whose first item is `FnItem { boundary:
  Some(BoundaryAttr { target: "ext::foreign_id" }), body: None, .. }`. (Oracle:
  `conformance/parse` boundary entry. Today: FAILS — `expected "{"` / `expected
  "slag"`, verified above.)
- **AC-2 (certifies L1 + boundary, NOT L3)**: `forge check` of `boundary.th` emits
  for `foreign_id` a cert with `level == "L1"`, `boundary == true`,
  `boundary_target == "ext::foreign_id"`, `slag == false`, and an oracle subset
  matching `conformance/boundary/boundary.cert.json`. The cert is NOT `L3` (no
  verus run on a foreign body). (Oracle: the golden boundary cert.)
- **AC-3 (L1 wrapper checks req/ens on the crossing)**: `thermite_lower::lower_l1`
  of `boundary.th` emits, for `foreign_id`, a `thermite_check!("req", …, x <= 1000)`
  before a call to `ext::foreign_id(x)`, then `thermite_check!("ens", …, result ==
  x)` against the bound `result`; the foreign body is absent from the output.
  (Oracle: a `tests/golden/l1/boundary.l1.rs` golden the orchestrator authors,
  hand-derived, compiling under `rustc` with a stub `ext::foreign_id`.)
- **AC-4 (composition independence)**: `caller`'s cert is unchanged whether
  `foreign_id`'s `BoundaryAttr.target` is `"ext::foreign_id"` or any other path —
  `caller` certifies through the contract, not the body (§9). (Oracle: `caller`'s
  golden cert is byte-identical across two boundary targets.)
- **AC-5 (corpus unaffected)**: the existing corpus (`sum`, `binary_search`, the
  slag/vacuity/parse oracles — none of which contain a boundary fn) emits an
  IDENTICAL cert before and after #16; the additive `boundary` field defaults
  `false` and the frozen golden `conformance/sum.cert.json` still deserializes
  (R-SPEC-2). (Oracle: the unchanged existing golden certs.)

## Architecture

The component threads three crates in dependency order (`goal.md` R-DEFER-7):
`thermite-syntax` (form) → `thermite-lower` (wrapper) → `forge` (cert).

- **Surface (`thermite-syntax`).** `struct BoundaryAttr` mirrors `struct SlagAttr`
  in `ast.rs`; `FnItem` gains `boundary: Option<BoundaryAttr>` (mirroring
  `FnItem.slag: Option<SlagAttr>`) and `body: Option<Block>`. `parse_attribute`
  (generalizing `parse_slag` in `parser.rs`) dispatches on the attribute name;
  `parse_fn` gains the `Semi`-terminated bodyless path (the lexer already produces
  `TokKind::Semi`). The frontend stays registry-free and the §4.1 mandatory-contract
  rule is enforced by the unchanged `parse_contract`.

- **L1 wrapper (`thermite-lower`).** `lower_l1` routes a `f.boundary.is_some()`
  `FnItem` to a `lower_boundary_fn_l1` arm reusing `emit_check`, `lower_expr_exec`,
  `emit_params`, `lower_type`, and the `thermite_check!` macro from
  `emit_check_macro` — the wrapper is `req`-check → `let result = <target>(args);`
  → `ens`-checks. `lower.rs` (the L3 Verus path) skips a boundary fn (no body to
  prove), mirroring `check.rs`'s slag skip.

- **Cert (`forge`).** `manifest.rs` adds the additive `boundary: bool` +
  `boundary_target: Option<String>` to `Certificate`, a `Certificate::boundary_l1`
  constructor (modeled on `Certificate::slag_l1`), and extends `oracle_subset` to
  `(item, level, effects, slag, boundary)`. `check.rs`'s `gate_fn`/`check_file`
  adds a `boundary.is_some()` branch (triage a/b/c, then `boundary_l1`, no verus),
  alongside the existing `slag.is_some()` branch. The `boundary` flag is the #15
  audit hook + the #17 to-the-boundary input.

## Verification

- **AC-1**: `cargo test -p thermite-syntax` — a parse test of `boundary.th`
  asserting `errors.is_empty()` and the `FnItem { boundary: Some(_), body: None }`
  shape; a `conformance/parse` boundary entry.
- **AC-2 / AC-4 / AC-5**: the conformance corpus — `forge check` of `boundary.th`
  diffs the emitted cert against `conformance/boundary/boundary.cert.json` (the
  cert oracle, `goal.md` verification model (B)); the existing corpus diffs
  unchanged (AC-5). `cargo test -p forge`.
- **AC-3**: `cargo test -p thermite-lower` — diff `lower_l1(boundary.th)` against
  `tests/golden/l1/boundary.l1.rs` (the golden lowering, `goal.md` verification
  model (A) + R-CHAR-3 — hand-authored from this design, never regenerated), and a
  compile+run check (the wrapper compiles under `rustc` with a stub foreign target,
  and a contract violation fires `thermite_contract_violation`).
- **Gauntlet (each crate):** `cargo test -p <crate>`, `cargo clippy -p <crate>
  --all-targets -- -D warnings`, `cargo fmt --check` (`goal.md` R-DEFER-6).

## Open questions

- **OQ-1 (target spelling):** is the foreign target a single positional string
  (`#[boundary("crate::foo")]`, the minimal form this doc pins) or a named field
  (`#[boundary(target = "crate::foo")]`, exactly the `#[slag]` `key = "value"`
  shape)? The positional string is fewer tokens (§2.2); the named field is one
  fewer grammar rule (reuses the slag field-list parser verbatim). LEANING
  positional — a boundary has exactly one datum, so a field name is noise. The
  builder should ratify against the skill budget when #16 starts.
- **OQ-2 (error-recovery interaction — the one to confirm):** does the new `;` body
  path interact badly with per-item recovery? A bodyless fn ends at `;`, which is
  NOT currently an item-boundary resync token (`resync_to_item_boundary` resyncs on
  `Fn`/`Spec`/`HashBracket`). A malformed boundary fn (e.g. missing `;`) recovers
  to the NEXT `fn`/`#[`, which is correct; a stray `;` mid-fn is already a parse
  error in the existing grammar. The risk is a non-boundary fn that the agent wrote
  bodyless by mistake — it must error clearly ("a non-`#[boundary]` fn requires a
  `{ }` body"), not silently parse as a boundary fn. The parser must gate the `;`
  body on `boundary.is_some()`. LOW risk, but this is the load-bearing recovery
  interaction the builder must test.
- **OQ-3 (Verus-path skip):** confirmed-intended — `lower.rs` (L3) does NOT lower a
  boundary fn (no body), and `check.rs` does NOT run verus on it, mirroring the
  `#[slag]` skip. The builder should assert no Verus invocation occurs for a
  boundary fn (parallel to the slag no-verus assertion).
- **OQ-4 (effect-row crossing):** does a boundary fn's `fx` row constrain the
  foreign call (e.g. a `fx pure` boundary fn calling a foreign fn that allocates)?
  In v0.1 `fx` is compile-time-subsumption-only (no runtime sandbox, #21,
  R-SPEC-5), so the `fx` row is checked at the Thermite call site exactly as for a
  proved fn; the foreign body's actual effects are trusted-by-fiat (the §9/§8
  honesty: the row is *stated*, the body is trusted). No new mechanism in #16.

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (surface form) | SHIPPED | `#[boundary("crate::path")] fn NAME(..) -> ret req .. ens .. fx .. ;` parses via `parse_attribute` + the `Semi`-body path in `parse_fn` (`thermite-syntax/src/parser.rs`); verified by `boundary_fn_parses_with_target_and_no_body` in `thermite-syntax/tests/boundary_parse.rs`. |
| REQ-2 (AST shape) | SHIPPED | `struct BoundaryAttr { target, span }` + `FnItem.boundary: Option<BoundaryAttr>` + `FnItem.body: Option<Block>` in `thermite-syntax/src/ast.rs` (exported from `lib.rs`); a boundary fn is `boundary: Some`, `body: None` — asserted by `boundary_fn_parses_with_target_and_no_body`. |
| REQ-3 (parser extension) | SHIPPED | `parse_attribute` dispatches on the `#[` name (`slag`→`SlagAttr`, `boundary`→`BoundaryAttr`); `parse_fn`'s `Semi`-body path is GATED on `boundary.is_some()` (OQ-2: a bodyless non-`#[boundary]` fn is a `SyntaxError`, a `#[boundary]` fn with `{ }` is a `SyntaxError`, `#[boundary]` on a `spec fn` is a `SyntaxError`). Verified by `bodyless_fn_without_boundary_is_a_parse_error`, `boundary_fn_with_brace_body_is_a_parse_error`, `boundary_on_spec_fn_is_a_parse_error`. |
| REQ-4 (L1 wrapper lowering) | SHIPPED | `lower_boundary_fn_l1` in `thermite-lower/src/l1.rs` emits `req`-check → `let result = <target>(args);` (the foreign call; body NOT lowered) → `ens`-checks; routed by the `f.boundary.is_some()` guard in `lower_l1`. Consumer: `forge`'s `ladder_for_timeout`/the L1 recording path + the boundary cert. |
| REQ-5 (forge cert L1 + boundary flag) | SHIPPED | `Certificate.boundary: bool` + `boundary_target: Option<String>` + `Certificate::boundary_l1` (`Level::L1`, `boundary: true`, target, no verus, `graduate_triage_clean`) in `forge/src/manifest.rs`; `oracle_subset` is now `(item, level, effects, slag, boundary)`. `check::gate_fn` detects `f.boundary.is_some()` FIRST, validates a non-empty target, runs (a)/(b)/(c) triage, then `boundary_l1`. Verified by `foreign_id_certifies_l1_boundary_not_l3` + `boundary_vacuous_contract_is_rejected` in `forge/tests/boundary_conformance.rs`. |
| REQ-6 (#15 TCB hook) | SHIPPED | the per-cert `boundary: bool` + `boundary_target` is the enumerable hook (joins `slag` in `oracle_subset`, rendered by `cli::render_human`); a boundary fn's cert carries `boundary: true` + the foreign target for #15's `slag ∪ boundary ∪ toolchain` audit. |
| REQ-7 (composition independence) | SHIPPED | a boundary fn is gated to the L1 path in `check::gate_fn` BEFORE any L3/L2/mutation/strengthen stage, so a caller `g` lowers/certifies through `f`'s contract alone (its foreign body never enters `g`'s sub-program); the `caller`-through-`foreign_id` example certifies independent of the foreign target. |
