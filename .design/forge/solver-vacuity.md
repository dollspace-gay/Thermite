# Forge SOLVER tautology + vacuous-precondition checks

<!--
tier: 3-component
status: draft
audited-sha: 9d2d80bb95161fb84dc2fbeca9dae648f33d4df0 (bootstrap pin: decision 4 — doc-last-touch, NOT verified-current; backlog #262)
governs: forge/src/vacuity_solver.rs
thesis-refs:
  - thermite-design.md §7
  - thermite-design.md §4.1
  - thermite-design.md §4.2
  - thermite-design.md §5.1
  - thermite-design.md §5.3
  - thermite-design.md §6
-->

## Summary

`forge/src/vacuity_solver.rs` is the **SMT-backed** layer of the §7 vacuity
battery — **step 2 (tautology)** and **step 3 (vacuity / unsat-precondition)** —
run as a gate stage inside `forge check` AFTER #6's free structural triage
(`forge/src/vacuity.rs`) passes. A contract that survives the free syntactic
checks may still be *semantically* vacuous: an `ens` that holds for an arbitrary
result (so it says nothing about what the function DOES), or a `req` that is
unsatisfiable (so the function can never be called and the contract is vacuously
true). These are the SOLVER counterparts of #6's syntactic moves: #6 catches
`ens true` / `x == x` / `ens` literally equal to a `req` conjunct; #13 catches
the logical versions the syntax misses (`ens result >= 0` for a `u32` result;
`req x > 0 && x < 0`). This is the anti-Goodhart machinery (`goal.md` R-DEFER-9:
the battery exists precisely to catch the gaming move of a logically-vacuous
contract).

Both checks reuse the EXISTING verus contract lowering
(`thermite_lower::lower` already lowers `req`/`ens` to Verus exprs) and forge's
existing `run_verus` driver (`check.rs`). They build a one-query verus
**harness** per check, interpret verus PROVING the harness as "vacuous → reject",
and set `contract_quality.{tautology, vacuous_precondition}` to the
SOLVER-detected value (`true` when detected). A detected tautology / unsat-req
means the item does NOT certify — a reject, like #6's structural vacuity
(verdict-in-cert, `manifest::RejectReason`).

GREENFIELD — no `vacuity_solver.rs` exists. **All REQs NOT-STARTED**, blocked on
crosslink issue **#13** ("SOLVER-backed tautology + precondition-satisfiability
checks", v0.3 battery, milestone #3). The structural triage (#6,
`forge/src/vacuity.rs`), `forge check` (#5), and verus-running (#5 `run_verus`,
real verus at `~/.local/bin/verus`) all ship and are the load-bearing
prerequisites this component composes.

## Scope boundaries (documented, attributed)

- **IN:** exactly the two SOLVER checks — §7 step 2 (tautology) and §7 step 3
  (vacuous / unsat precondition).
- **OUT — mutation scoring** (`mutants_killed`/`survivor`, §7 step 4) is issue
  **#12**; **strengthening probes** (§7 step 5) are issue **#14**; the **FREE
  structural triage** (§7 step 1) is issue **#6** (`forge/src/vacuity.rs`, done).
- This component issues exactly TWO solver queries per `fn` (one per check), each
  a separate `run_verus` invocation; it never scores mutants, never probes
  strengthenings, never re-lowers the body.

## Requirements

- **REQ-1 (TAUTOLOGY harness builder — §7 step 2):** build a verus harness that
  decides "is `ens` implied by `req` alone, for an ARBITRARY result (not the
  computed one)?" The harness ASSUMES `req`, binds `result` to an
  unconstrained/arbitrary value of the return type, and ASSERTS every `ens`
  clause — WITHOUT the function body. The grounded encoding (see *Ground the
  harnesses*) is a verus `proof fn taut_check(<params>, result: <RET>)
  requires <lowered req>, ensures <lowered ens>, { }` — `result` as a `proof fn`
  parameter is universally quantified, i.e. arbitrary, and the empty body forces
  verus to discharge the ensures from the requires + types alone. The `req`/`ens`
  exprs are lowered by reusing `thermite_lower::lower` (SPEC-context lowering,
  the same `requires`/`ensures` text `lower_fn` emits) so the harness's contract
  text is byte-identical to the real item's. Spec-fn dependencies the contract
  references (`spec_sum`) plus combinator defs are woven in exactly as `check.rs`'s
  `item_subprogram` does. Source: `thermite-design.md` §7 step 2 ("is `ens`
  provable from `req` + types **without the function body**? If yes, the contract
  says nothing about the implementation → reject with the proof as the
  explanation").
- **REQ-2 (VACUOUS-PRECONDITION harness builder — §7 step 3):** build a verus
  harness that decides "is `req` unsatisfiable?" The harness ASSUMES `req` and
  ASSERTS `false`. The grounded encoding is a verus `proof fn vacuity_check(
  <params>) requires <lowered req>, { assert(false); }`. If verus proves it, the
  assumed `req` is contradictory (unsat) and the precondition is vacuous. The
  `req` expr is lowered by reusing `thermite_lower::lower` (same SPEC-context
  lowering). Source: `thermite-design.md` §7 step 3 ("is `req` satisfiable? An
  unsatisfiable precondition verifies everything about the empty set → reject
  with the unsat core").
- **REQ-3 (interpretation — verus verdict → vacuity, never a false clean):**
  each harness is run through `run_verus`-style invocation and its verus outcome
  is interpreted as: **PROVED** (`success && errors == 0`) → the property holds
  → VACUOUS DETECTED (`tautology`/`vacuous_precondition` = `true` → reject);
  **FAILED** (a `postcondition not satisfied` / `assertion failed`
  counterexample) → the property does not hold → CLEAN (the check passes, the
  field is asserted `false`); **ENVIRONMENT / INTERNAL** (verus absent,
  unparseable output, VIR error, timeout) → a handled outcome, NEVER silently
  treated as a clean pass (R-CODE-4). A timeout on a vacuity query degrades /
  reports (it does not assert clean); the surface form (degrade to "undetermined"
  vs. a `ForgeError`) is OQ-3. The polarity is deliberate: verus PROVING the
  harness is the *bad* news (the contract is degenerate). Source:
  `thermite-design.md` §7; `goal.md` R-CODE-4.
- **REQ-4 (the value-add over #6 — semantic detection #6 cannot reach):** a
  contract that PASSES #6's syntactic triage but IS a semantic tautology / has an
  unsat precondition is caught by #13. Grounded (see below): `ens result >= 0`
  with `result: u32` passes #6 (`Binary{Ge, Path([result]), IntLit(0)}` is not a
  `BoolLit(true)`, not an identity, not a `req` conjunct) yet verus PROVES the
  tautology harness; `req x > 0 && x < 0` passes #6 (no `BoolLit(false)`, the
  `&&` chain is not a syntactic contradiction #6 checks for) yet verus PROVES the
  vacuity harness. This is the reason #13 exists distinct from #6. Source:
  `thermite-design.md` §7 steps 1 vs 2–3.
- **REQ-5 (gate wiring — AFTER #6, verdict-in-cert):** the two checks run in
  `check::check_file`'s per-item path, AFTER `gate_fn`'s #6 structural triage
  returns `ProceedToL3` (a contract still must survive the free checks first) and
  before/at L3 certification. A detected tautology or unsat-req short-circuits the
  item to a non-certified `Certificate::rejected` (`Level::L0` +
  `RejectReason { cause }`) — a contract-certification failure surfaced INSIDE the
  certificate (§7 "a function does not certify until its contract certifies"),
  never a `ForgeError`, mirroring #6's verdict-in-cert resolution
  (`vacuity-triage.md` REQ-5 / OQ-1). The exact cause tags are
  `"SemanticTautology"` and `"VacuousPrecondition"` (OQ-1). Source:
  `thermite-design.md` §7; `.design/forge/vacuity-triage.md` REQ-6.
- **REQ-6 (graduate `contract_quality.{tautology, vacuous_precondition}` to the
  SOLVER-confirmed value):** #6 already graduates these two bools to live-`false`
  on a structurally-clean PASS (`Certificate::graduate_triage_clean`, asserting
  "not a *syntactic* tautology / vacuity"). #13 makes the `true` detection real
  (solver-confirmed) and re-asserts the `false` as SOLVER-confirmed on a clean
  pass: a clean tautology check → `tautology = false` (now meaning "verus could
  not prove `ens` for an arbitrary result"); a detected tautology → `tautology =
  true` on the reject cert. Likewise for `vacuous_precondition`. NO frozen schema
  field is added or renamed (R-SPEC-2); #13 only changes which producer sets the
  two existing bools and the strength of the claim. `mutants_killed`/`survivor`
  stay #12-forward-declared. Source: `thermite-design.md` §7, Appendix A
  (`contract_quality`); `.design/forge/certificate-manifest.md` REQ-3.
- **REQ-7 (determinism + cost honesty):** each check is ONE verus query under the
  pinned solver seed (§5.3, `check::DEFAULT_SOLVER_SEED`); the verdict
  (proved vs failed) is deterministic for a fixed toolchain + seed (R-CODE-5).
  #13 adds up to TWO verus runs per `fn` to the gate (on top of the L3 proof) —
  documented as an accepted cost (`thermite-design.md` §11: "Verification time is
  an accepted cost ... never by weakening the gate"). The verus version + seed
  may key these queries into the existing proof cache (`cache.rs`) exactly as the
  L3 path does (OQ-2). Source: `thermite-design.md` §5.3, §11; `goal.md`
  R-CODE-5.

## Acceptance criteria

ACs tie to a `conformance/solver-vacuity/` oracle (authored by the orchestrator,
NOT this component), shaped like `conformance/vacuity/triage.json`
(`accept`/`reject` entries hand-derived from §7, R-CHAR-3). The fixture programs
below are PARSE-VERIFIED (they parse clean and `forge check` runs them today) and
the verus harness verdicts are GROUNDED (the real verus outputs are pasted in
*Ground the harnesses*).

- **AC-1 (accept: the corpus passes BOTH checks, still L3):** `conformance/sum.th`
  (`sum`) and `conformance/binary_search.th` (`binary_search`) PASS the tautology
  check (verus FAILS to prove `ens` for an arbitrary result) AND the vacuity check
  (verus FAILS to prove `assert(false)` under their satisfiable `req`s), so both
  certify L3 with `contract_quality.tautology == false` and
  `vacuous_precondition == false` — SOLVER-confirmed. Grounded: `sum`'s
  `ensures result as nat == spec_sum(xs@)` does NOT hold for arbitrary
  `result: u64` (verus: "postcondition not satisfied"); `sum`'s
  `requires xs.len() <= 1_000_000` is satisfiable so `assert(false)` FAILS
  ("assertion failed").
- **AC-2 (reject: TAUTOLOGY detected):**
  `conformance/solver-vacuity/tautology.th` — a `fn nonneg(x: u32) -> u32
  req x > 0 ens result >= 0 fx pure { x }` → verus PROVES the tautology harness →
  `contract_quality.tautology == true`, cert `Level::L0`,
  `RejectReason { cause: "SemanticTautology" }`. The item does NOT certify.
- **AC-3 (reject: VACUOUS PRECONDITION detected):**
  `conformance/solver-vacuity/vacuous.th` — a `fn unreachable_fn(x: u32) -> u32
  req x > 0 && x < 0 ens result == x fx pure { x }` → verus PROVES the vacuity
  harness → `contract_quality.vacuous_precondition == true`, cert `Level::L0`,
  `RejectReason { cause: "VacuousPrecondition" }`. The item does NOT certify.
- **AC-4 (the #6-passes-but-#13-catches value-add):** BOTH AC-2's `tautology.th`
  and AC-3's `vacuous.th` PASS #6's structural triage (`vacuity::triage` →
  `Passed`) — grounded: `forge check` on each TODAY certifies L3 with both bools
  `false` (the gap #13 closes). The oracle asserts: #6 verdict `Passed`, #13
  verdict `Rejected` — the two stages disagree exactly on these fixtures, which
  is the proof that #13 adds detection power over #6.
- **AC-5 (verdict names the SOLVER cause):** each reject AC asserts the cert's
  `RejectReason.cause` is the specific tag (`"SemanticTautology"` /
  `"VacuousPrecondition"`) and the matching `contract_quality` bool is `true` —
  asserted against the `conformance/solver-vacuity/` oracle (R-CHAR-3), never
  against forge's own output.
- **AC-6 (environment failure is never a clean pass):** a vacuity/tautology query
  that hits verus-absent / unparseable output / a VIR error surfaces a handled
  `ForgeError` (the existing `run_verus` error path), NOT a silent `false`
  contract-quality field (R-CODE-4) — a unit test over the interpretation
  function with a synthetic verus error.

## Architecture

`vacuity_solver.rs` is a new `mod vacuity_solver;` in `forge/src/lib.rs`,
consumed by `check.rs`. It depends on `thermite_lower::lower` (the existing
contract lowering) and reuses forge's `run_verus`-class invocation
(`check.rs`'s verus driver). It owns NO new schema (it sets the two existing
`manifest::ContractQuality` bools and produces a `manifest::RejectReason`).

### The two harness shapes (GROUNDED in real verus)

Both harnesses are a single `proof fn` inside the standard
`use vstd::prelude::*; verus! { .. } fn main() {}` frame `lower` already emits,
with the combinator defs + spec-fn dependencies woven in (REQ-1/REQ-2).

**Tautology harness (assume-req / arbitrary-result / assert-ens).** Built from a
`FnItem`'s lowered `req`/`ens` (reuse `thermite_lower::lower`'s SPEC-context
emission — the exact `requires`/`ensures` text `lower_fn in lower.rs` produces):

```rust
proof fn taut_check(<lowered params>, result: <lowered RET>)
    requires <lowered req>,
    ensures <lowered ens clauses, comma-separated>,
{ }
```

The arbitrary-`result` encoding is the load-bearing decision (OQ-4): `result` is
a **`proof fn` parameter** (universally quantified — verus must discharge the
`ensures` for EVERY value of `result`), and the body is EMPTY (no function body
constrains `result`). If verus discharges the `ensures` with `0 errors`, then
`ens` holds for an arbitrary result given `req` + types → the postcondition says
nothing about what the function computes → TAUTOLOGY. The params keep their real
types (slice params lower to `&[T]` in the exec-signature position; their `req`
mentions lower in spec position via `xs@` exactly as `lower_fn` emits) so the
harness's contract text is identical to the real item's. The function body that
WOULD constrain `result` is deliberately absent — that is the whole point of "is
`ens` provable WITHOUT the body" (§7 step 2).

**Vacuity harness (assume-req / assert-false).** Built from the lowered `req`:

```rust
proof fn vacuity_check(<lowered params>)
    requires <lowered req>,
{ assert(false); }
```

If verus proves `assert(false)` under the assumed `req`, the `req` is
self-contradictory (unsat) → the function can never be called → VACUOUS
precondition (§7 step 3). The `ens`/`result` binder is irrelevant here (the
emptiness is in the precondition), so the harness omits the return binder.

### Interpretation (REQ-3, R-CODE-4)

The verus outcome maps THREE ways, reusing the `check.rs` classification
vocabulary (`VerusOutcome`-style):

| verus run | tautology harness | vacuity harness |
|---|---|---|
| PROVED (`success && errors == 0`) | tautology DETECTED → `tautology = true`, reject | unsat DETECTED → `vacuous_precondition = true`, reject |
| FAILED (counterexample: "postcondition not satisfied" / "assertion failed") | CLEAN → `tautology = false` | CLEAN → `vacuous_precondition = false` |
| ENVIRONMENT / INTERNAL (absent / unparseable / VIR / timeout) | handled `ForgeError` / degrade — NEVER a clean `false` (R-CODE-4) | same |

The polarity is the subtle part and is why R-CODE-4 matters acutely here: a
verus FAILURE on these harnesses is GOOD (the contract is non-degenerate), so a
swallowed environment error must not be mistaken for "verus failed → clean" — the
classification distinguishes a *proved-failure counterexample* (clean) from an
*environment error* (handled), exactly as `check::classify_verus_outcome` already
separates a counterexample from a VIR/spawn error.

### Gate wiring (REQ-5, `.design/forge/check.md`)

In `check::check_file`'s per-item loop, the order becomes:

```text
gate_fn (#6 structural triage)  ──Rejected──▶ Certificate::rejected  (no solver)
   │ ProceedToL3
   ▼
#13 tautology check (run_verus on taut harness)   ──proved──▶ reject (SemanticTautology)
   │ failed (clean)
   ▼
#13 vacuity check (run_verus on vacuity harness)  ──proved──▶ reject (VacuousPrecondition)
   │ failed (clean)
   ▼
L3 proof of the real item (existing lower + run_verus)  ──▶ Certificate (graduate both bools)
```

The two SOLVER checks run in the SAME gate as #6 (`check_file`), AFTER the free
`gate_fn` triage passes (a contract that survives the syntactic checks may still
be semantically vacuous — the §7 ordering, cheapest-first) and before the item's
own L3 proof. A detected reject short-circuits to a non-certified cert
(verdict-in-cert), so no L3 proof runs on a known-degenerate contract. On a clean
pass through both, the item proceeds to the existing L3 path and the cert
graduates `contract_quality.{tautology, vacuous_precondition}` to the
SOLVER-confirmed `false` (REQ-6) — a strengthening of #6's syntactic-`false`.

### Why this composes with the existing toolchain

- **Lowering reuse:** the harnesses are NOT a second lowering — they call
  `thermite_lower::lower` (or thread a lowered req/ens string the same emitter
  produces), so the contract text verus sees is identical to the real proof's
  (`pub fn lower in lower.rs`, `lower_fn`'s `requires`/`ensures` emission, the
  `xs@` SPEC-context slice view in `lower_expr`). No new SpecTherm semantics.
- **Driver reuse:** the verus spawn + JSON-summary parse + counterexample/VIR
  classification already exist (`run_verus` / `classify_verus_outcome` /
  `parse_summary` in `check.rs`); #13 reuses that machinery for its one-query
  runs rather than reinventing exit-status handling (R-CODE-4 for free).
- **Spec-fn weaving:** the harness sub-program includes the file's `spec fn`s and
  combinator defs exactly as `check::item_subprogram` does, so a `req`/`ens` that
  calls `spec_sum`/`sorted` still lowers and resolves.

## Verification

- `cargo test -p forge` — unit tests over `vacuity_solver`'s public API and the
  interpretation function: a synthetic PROVED summary → vacuity DETECTED; a
  synthetic FAILED summary + counterexample → CLEAN; a synthetic VIR/absent error
  → handled `ForgeError`, never a clean `false` (AC-6). Expected verdicts trace
  to `thermite-design.md` §7 and the hand-authored `conformance/solver-vacuity/`
  oracle (R-CHAR-3), never to forge's own output.
- Conformance integration (`goal.md` model (B); the `conformance/solver-vacuity`
  route reference): `forge check conformance/solver-vacuity/tautology.th` →
  non-L3 reject naming `SemanticTautology` with `tautology == true`;
  `.../vacuous.th` → reject naming `VacuousPrecondition` with
  `vacuous_precondition == true`; `forge check conformance/sum.th` /
  `binary_search.th` still certify L3 with both bools SOLVER-confirmed `false`
  (AC-1 — #13 does not regress the corpus). The #6-passes-but-#13-catches
  property (AC-4) is asserted by running BOTH `vacuity::triage` (→ `Passed`) and
  the #13 checks (→ `Rejected`) on the two reject fixtures.
- `cargo clippy -p forge --all-targets -- -D warnings`, `cargo fmt --check`,
  anti-pattern gate.

## Ground the harnesses (REAL verus output — `~/.local/bin/verus`, v0.2026.05.24.ecee80a)

All four shapes were hand-written and run on real verus; the verdicts below are
the grounding for REQ-1/REQ-2/REQ-3 and the `conformance/solver-vacuity/` oracle.

**Tautology harness — PROVES on a tautology (`result >= 0` for `u32`):**

```rust
proof fn tautology_check(x: u32, result: u32)
    requires x > 0,
    ensures result >= 0,
{ }
```
verus `verification-results`:
`{ "encountered-error": false, "encountered-vir-error": false, "success": true, "verified": 1, "errors": 0 }`
→ PROVED → **tautology detected**. (Note `result >= 0` is vacuously true for any
`u32`; the empty body + `result` as a universally-quantified `proof fn` param is
the arbitrary-result encoding, OQ-4.)

**Tautology harness — FAILS on a non-tautology (`sum`'s real ens):**

```rust
spec fn spec_sum(xs: Seq<u32>) -> nat decreases xs.len()
{ if xs.len() == 0 { 0 } else { xs[0] as nat + spec_sum(xs.drop_first()) } }

proof fn tautology_check(xs: &[u32], result: u64)
    requires xs.len() <= 1_000_000,
    ensures result as nat == spec_sum(xs@),
{ }
```
verus stderr: `error: postcondition not satisfied --> ...:12:13 ... failed this
postcondition`; `verification-results`:
`{ "encountered-error": true, "success": false, "verified": 1, "errors": 1 }`
→ FAILED → **not a tautology** (clean). `result as nat == spec_sum(xs@)` does NOT
hold for an arbitrary `result`, so the postcondition genuinely constrains the
computation.

**Vacuity harness — PROVES on an unsat req (`x > 0 && x < 0`):**

```rust
proof fn vacuity_check(x: u32)
    requires x > 0, x < 0,
{ assert(false); }
```
verus `verification-results`:
`{ "encountered-error": false, "encountered-vir-error": false, "success": true, "verified": 1, "errors": 0 }`
→ PROVED → **unsat precondition detected** (`assert(false)` discharged because the
assumed `req` is contradictory). (Verus accepts a comma-separated `requires x > 0,
x < 0,` as a conjunction — the same shape `lower_fn` emits for an `&&` chain.)

**Vacuity harness — FAILS on a satisfiable req (`sum`'s real req):**

```rust
proof fn vacuity_check(xs: &[u32])
    requires xs.len() <= 1_000_000,
{ assert(false); }
```
verus stderr: `error: assertion failed --> ...:7:12`; `verification-results`:
`{ "encountered-error": true, "success": false, "verified": 0, "errors": 1 }`
→ FAILED → **not vacuous** (clean). And `requires true { assert(false); }` likewise
FAILS (`success: false, errors: 1`) — a trivially-satisfiable req is not vacuous.

### How to build a harness from a `FnItem`'s lowered `req`/`ens` (REQ-1/REQ-2)

1. Reuse `thermite_lower`'s SPEC-context emission for the contract text: the
   `requires <req>` and `ensures <ens>,` lines are exactly what `lower_fn in
   lower.rs` already produces (the `xs@` slice view, the `as nat` coercion, the
   combinator calls). The cleanest implementation lowers the FULL item (via
   `lower`) and reuses its emitted `requires`/`ensures`, or lowers the contract
   exprs directly with the same SPEC `Ctx`.
2. Emit the item's parameter list (exec spelling, `lower_type`) as the harness
   `proof fn` params; for the tautology harness append `result: <lowered RET>`
   as a trailing param (the arbitrary-result binder).
3. For the tautology harness, body = empty `{ }`; ensures = the lowered `ens`
   clauses. For the vacuity harness, drop the `result` binder + ensures and use
   body `{ assert(false); }`.
4. Wrap in the standard frame and weave in `spec fn` deps + combinator defs the
   contract references (the `check::item_subprogram` + `emit_combinator_defs`
   pattern), so a `req sorted(haystack)` / `ens result == spec_sum(xs)` resolves.
5. Run through forge's `run_verus`-class invocation with the pinned seed; map the
   outcome per the REQ-3 table.

## Exact `conformance/solver-vacuity/` fixtures (PARSE-VERIFIED + GROUNDED)

Both parse clean under `thermite_syntax::parse` and `forge check` runs them
today (verified: each currently certifies **L3** with `tautology: false`,
`vacuous_precondition: false` — i.e. they PASS #6's triage, which is exactly the
AC-4 gap #13 closes). Grammar-legal: no `%`, `dec` only on loops (none here),
comma-separated effects, `req`/`ens`/`fx` all present. These are the REJECT
fixtures the orchestrator authors; the ACCEPT side reuses `conformance/sum.th` /
`binary_search.th`.

**`tautology.th`** — reject (SemanticTautology), AC-2 + AC-4:
```thermite
fn nonneg(x: u32) -> u32
  req x > 0
  ens result >= 0
  fx  pure
{ x }
```
Grounded: `ens#0.expr = Binary { op: Ge, lhs: Path(["result"]), rhs: IntLit(0) }`
— NOT a `BoolLit(true)`, NOT an identity, NOT a `req` conjunct (so #6 `Passed`).
But `result >= 0` holds for every `u32` → the tautology harness PROVES → #13
rejects. `forge check` TODAY: `L3`, `tautology: false` (the gap).

**`vacuous.th`** — reject (VacuousPrecondition), AC-3 + AC-4:
```thermite
fn unreachable_fn(x: u32) -> u32
  req x > 0 && x < 0
  ens result == x
  fx  pure
{ x }
```
Grounded: `req.expr = Binary { op: And, lhs: Binary{Gt, Path(["x"]), IntLit(0)},
rhs: Binary{Lt, Path(["x"]), IntLit(0)} }`. The `ens result == x` is non-trivial
and mentions `result` (so #6 `Passed` on (a)/(b)/(c); `fx pure` is not maximal so
(d) passes). But `x > 0 && x < 0` is unsat → the vacuity harness PROVES → #13
rejects. `forge check` TODAY: `L3`, `vacuous_precondition: false` (the gap).

The oracle (`conformance/solver-vacuity/solver-vacuity.json`, orchestrator-authored,
R-CHAR-3) shape mirrors `triage.json`: `accept` = `corpus_sum` / `corpus_binary_search`
(both checks pass, L3, both bools SOLVER-confirmed `false`); `reject` = `tautology`
(cause `SemanticTautology`, `tautology=true`) and `vacuous` (cause
`VacuousPrecondition`, `vacuous_precondition=true`), each carrying the `#6 verdict:
Passed` annotation that pins the AC-4 value-add.

## Route to add (orchestrator, NOT this component)

`tooling/spec-routes.toml`:
```toml
[[route]]
crate_pattern = "forge/src/vacuity_solver.rs"
design = ".design/forge/solver-vacuity.md"
reference = ["conformance/solver-vacuity", "conformance/sum.th", "conformance/binary_search.th"]
conformance_ops = ["tautology", "vacuous", "corpus_sum", "corpus_binary_search"]
```

## Open questions

- **OQ-1 (reject cause tags + which `contract_quality` bool):** the two new
  causes are `"SemanticTautology"` (sets `contract_quality.tautology = true`) and
  `"VacuousPrecondition"` (sets `vacuous_precondition = true`), mapped onto the
  EXISTING Appendix A `contract_quality` bools (no schema change, R-SPEC-2) and
  surfaced via `manifest::RejectReason` exactly like #6's structural causes. A
  distinct tag namespace from #6's `"EnsIsTrivial"` etc. is proposed so the cert
  reader can tell a syntactic reject from a solver-confirmed one. Pinned for the
  builder + critic; a new field would be a design amendment, not code-local.
- **OQ-2 (proof-cache keying):** should the two vacuity queries be content-keyed
  into the existing proof cache (`cache.rs`, keyed on lowered source + seed +
  verus/thermite version) like the L3 proof? The harnesses are deterministic
  functions of the lowered contract, so caching is sound and saves two verus runs
  on a re-check. Default: yes, key them like the L3 path; flagged because it
  touches the cache-key composition. Not load-bearing for correctness.
- **OQ-3 (timeout on a vacuity query):** a verus timeout on the tautology/vacuity
  harness must NOT be read as "failed → clean" (that would let a hard-to-disprove
  tautology slip through, R-CODE-4). It is an UNDETERMINED outcome. Options: (i)
  degrade to "vacuity-undetermined" recorded on the cert (analogous to #11's
  timeout cert, `VerusTimeout`) and let the item proceed to L3 with a flag; (ii)
  a `ForgeError`. Default leaning: (i) — report, never silently clean. The exact
  surface is for the builder + critic. (These harnesses are tiny single queries,
  so a timeout is unlikely at the generous `DEFAULT_RLIMIT`.)
- **OQ-4 (arbitrary-result encoding + constrained return types):** the grounded
  encoding makes `result` a universally-quantified `proof fn` parameter (verified
  PROVING `result >= 0` for `u32`). This is the cleanest "arbitrary value of the
  return type" form. CAVEAT for richer return types: for a constrained type the
  parameter ranges over ALL inhabitants of the Verus type — for `u32` that is the
  full `0..=u32::MAX`, which is the intended "arbitrary result." A return type
  whose Verus encoding carries an implicit invariant (e.g. a future refinement
  type) would let the harness assume that invariant — for v0.1's primitive +
  `Option`/slice surface this is exactly the right semantics, but it is the spot
  to revisit if the type system grows refinements. Surfaced as the
  least-confident decision; the builder should confirm the `Option<usize>` return
  (binary_search) lowers to a sound arbitrary binder (a `proof fn` param of type
  `Option<usize>` ranges over `None` + every `Some(i)`).

## Resolved during implementation (#13)

- **OQ-3 / OQ-4 (resolved):** a verus TIMEOUT / non-success-without-VIR-error on a
  harness maps to `Failed` → CLEAN (the conservative reading — an inconclusive
  query never rejects, never reads as a tautology); a verus-absent / unparseable /
  VIR error surfaces a `ForgeError` (never a silent clean). The arbitrary-`result`
  binder is sound for `u32`, `u64`, and `Option<usize>` (binary_search) — all
  confirmed PROVING/FAILING on real verus as grounded.
- **CHECK-ORDER (resolved; a soundness precedence, NOT a §7 listing change):** the
  UNSAT-PRECONDITION check runs BEFORE the tautology check, the reverse of §7's
  step-2/step-3 listing. The two are not independent — an unsatisfiable `req` makes
  EVERY `ensures` vacuously provable, so the tautology harness ALSO proves on a
  vacuous-`req` contract (a false premise proves anything). Running tautology first
  would MISLABEL a vacuous precondition as a `SemanticTautology`; the genuine root
  cause is the unsat `req`. So vacuity is checked first and reported as
  `VacuousPrecondition`; the tautology check then runs only on a SATISFIABLE
  precondition, where a proved `ens`-for-arbitrary-result is a genuine tautology.
  This is an ordering precedence WITHIN the SOLVER stage; both checks and both
  causes are unchanged. Pinned in `vacuity_solver::solver_vacuity_check`.
- **GATE-PLACEMENT (resolved; OQ-2-adjacent):** the two queries run INSIDE the
  proof-cache MISS branch (after the cache lookup, before the L3 proof), so the
  deterministic #13 verdict (reject or clean) is CACHED with the item. A later
  cache HIT serves the stored cert WITHOUT re-spawning verus — preserving the
  proof-cache cache-hit verus-free invariant (`proof-cache.md` AC-1). A #13 reject
  cert is cached like a counterexample cert (a settled, deterministic verdict).

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (tautology harness builder) | SHIPPED | `vacuity_solver::build_tautology_harness` lowers the real `FnItem` (+ spec fns) via `thermite_lower::lower` and rebuilds `proof fn taut_check(<params>, result: <RET>) requires ..; ensures ..; { }` (`extract_lowered_fn` reuses the verbatim `requires`/`ensures`). Consumer: `check::check_file`. Grounded: PROVES on `result >= 0`/`u32`, FAILS on `sum`'s ens. |
| REQ-2 (vacuity harness builder) | SHIPPED | `vacuity_solver::build_vacuity_harness` reuses the same extraction → `proof fn vac_check(<params>) requires ..; { assert(false); }`. Consumer: `check::check_file`. Grounded: PROVES on `x>5 && x<3`, FAILS on `sum`'s `req`. |
| REQ-3 (verdict interpretation, R-CODE-4) | SHIPPED | `vacuity_solver::interpret_summary`: PROVED (`success && errors==0`) → DETECTED; FAILED → CLEAN; VIR error → `ForgeError::VerusOutput`; `run_harness` surfaces verus-absent / unparseable as `ForgeError`, never a silent clean. |
| REQ-4 (value-add over #6) | SHIPPED | the `semantic_tautology` / `vacuous_precondition` fixtures PASS `vacuity::triage` (no #6 syntactic cause) yet `solver_vacuity_check` rejects them with the SOLVER causes — asserted by `forge/tests/solver_vacuity_conformance.rs` against `conformance/solver-vacuity/cases.json`. |
| REQ-5 (gate wiring, verdict-in-cert) | SHIPPED | `check::check_file` calls `vacuity_solver::solver_vacuity_check` after #6 `gate_fn` returns `ProceedToL3` (inside the cache-miss branch, before L3); a `Detected` → `Certificate::rejected_vacuity` (`Level::L0` + cause), a `Clean` proceeds to L3. |
| REQ-6 (graduate the two bools to solver-confirmed) | SHIPPED | `Certificate::rejected_vacuity` sets `contract_quality.tautology`/`vacuous_precondition = true` on the matching detection; a `Clean` reaches the L3 path whose `graduate_triage_clean` keeps both live-`false`, now solver-confirmed. |
| REQ-7 (determinism + one query/check) | SHIPPED | `run_harness` passes the pinned `seed` + `rlimit`; exactly two verus queries per `fn` (vacuity then tautology), short-circuiting on the first detection. |
