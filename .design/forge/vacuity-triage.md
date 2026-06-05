# Forge structural vacuity triage

<!--
tier: 3-component
status: draft
governs: forge/src/vacuity.rs
thesis-refs:
  - thermite-design.md §7
  - thermite-design.md §7.1
  - thermite-design.md §4.1
  - thermite-design.md §6
  - thermite-design.md §8
-->

## Summary

`forge/src/vacuity.rs` is the **free, syntactic** layer of the §7 vacuity
battery — step 1, "structural triage" — run as a gate stage inside
`forge check` BEFORE each item's L3 proof. A function "does not certify until
its **contract** certifies" (§7); this component is the cheapest, solver-free
guard on that rule. It rejects the four §7.1 degenerate moves by inspecting the
parsed `Contract` AST alone (`thermite_syntax::Contract { req, ens, fx }`): no
`verus`, no Z3, no solver query. The non-trivial counterparts of these moves
(the SOLVER tautology / unsat-precondition checks, §7 steps 2–3) are
issue #13; mutation scoring (step 4) is #12; strengthening probes (step 5) are
#14 — all OUT of scope here. This is the anti-Goodhart floor (`goal.md`
R-DEFER-9: the battery exists precisely to catch the moves that game the gate).

GREENFIELD — no `vacuity.rs` exists. All REQs NOT-STARTED, blocked on
crosslink issue **#6** ("Implement structural vacuity triage and the `#[slag]`
escape hatch", milestone #1, currently blocked by #3 which has shipped).

## Requirements

- **REQ-1 (ens-is-trivially-true reject — §7.1 (a)):** an item whose
  postcondition is *syntactically* `true` is rejected. Syntactic triviality is:
  (i) **every** `ens` clause's `Clause.expr` is `Expr::BoolLit(true)`; or
  (ii) an `ens` clause is a syntactically-trivial tautology — an
  `Expr::Binary { op: BinOp::Eq, lhs, rhs }` (or `BinOp::Ge`/`BinOp::Le`) whose
  `lhs` and `rhs` are structurally identical expressions (`lhs == rhs` under
  `PartialEq`, e.g. `x == x`). This is FREE and SYNTACTIC: NON-trivial
  tautologies (`a || !a`, `x + 0 == x`) are the SOLVER tautology check, issue
  **#13** — explicitly NOT decided here. Because `ens` is a non-empty `Vec`
  (`ast.rs` `Contract.ens`), "ens simplifies to true" means the *conjunction* is
  trivially true; the conservative syntactic rule rejects only when the contract
  carries no non-trivial conjunct (case (i)) or contains a syntactically-trivial
  identity clause (case (ii)).
  Source: `thermite-design.md` §7.1 ("`ens` simplifies to `true` → reject").
- **REQ-2 (ens-omits-result reject — §7.1 (b), §4.1):** an item whose return
  type is NOT `()` (`ast.rs` `Type::Unit`) and whose `ens` never mentions
  `result` is rejected. The check walks every `ens` `Clause.expr` for an
  `Expr::Path` whose first segment is the identifier `"result"` (the parser
  builds `result` as `Expr::Path(["result"])` — grounded). The parser already
  enforces `ens` *presence* but NOT "mentions result"; §4.1 states this is
  "structurally enforced — see §7", and THIS is that check. A `Type::Unit`
  return is EXEMPT (§4.1: "Must mention `result` unless the return type is
  `()`").
  Source: `thermite-design.md` §4.1, §7.1 ("`ens` does not mention `result`
  (non-unit return) → reject").
- **REQ-3 (ens-syntactically-implied-by-req reject — §7.1 (c)):** an item is
  rejected when its `ens` is *syntactically* implied by `req` alone, defined as:
  every `ens` clause's `Clause.expr` is structurally identical (`PartialEq`) to
  either (i) the whole `req` `Clause.expr`, or (ii) one of the conjuncts of
  `req` when `req` is an `&&` chain. The `&&` chain is LEFT-ASSOCIATIVE (grounded:
  `a && b && c` parses to `Binary{And, Binary{And, a, b}, c}`), so the conjunct
  set is collected by recursively flattening `Expr::Binary { op: BinOp::And, .. }`
  along both arms. This is FREE and SYNTACTIC: the SOLVER question "is `ens`
  provable from `req` + types WITHOUT the body" (§7 step 2) is issue **#13** —
  NOT this check.
  Source: `thermite-design.md` §7.1 ("`ens` is syntactically implied by `req`
  alone → reject").
- **REQ-4 (maximal-fx-without-slag reject — §7.1 (d)):** an item whose effect
  row is *maximal* and which is NOT `#[slag]` is rejected. There is NO `fx *`
  surface form in the grammar (grounded: `parse_effect_row` accepts only `pure`
  or a comma-separated `Set`; the lexer's `Star` token is never consumed in an
  effect row). So **maximal** is DEFINED as an `EffectRow::Set` that contains at
  least one occurrence of every `Effect` *variant kind* — `Read`, `Write`, `Net`
  (each regardless of its `Ident` argument), `Alloc`, `Time`, `Rand`, `Panic`,
  `Diverge` (all 8 kinds present). An `EffectRow::Pure` is never maximal; a
  partial `Set` is never maximal. A maximal row is admissible ONLY on a
  `#[slag]` item (`FnItem.slag.is_some()`): slag is the only thing that justifies
  it (§8; the `slag.md` interaction). Maximal-`fx` with no slag → reject.
  Source: `thermite-design.md` §7.1 ("Effect row is maximal (`fx *`) without
  `#[slag]` justification → reject"), §8.
- **REQ-5 (`VacuityVerdict` + typed reject cause):** triage returns a structured
  verdict that names WHICH of (a)–(d) fired, with a clause-level diagnostic
  (which `ens` index / the offending effect). A reject is a `forge check`
  contract-certification failure: the item does NOT proceed to `verus`. The
  reject is surfaced through a `ForgeError` variant (or a verdict consumed by
  `check.rs` and rendered as a non-L3 reported failure carrying the cause), never
  a bare boolean, never a panic (`goal.md` R-CODE-2). The verdict is `pub` so
  `check.rs` consumes it; the surface form (a new `ForgeError::Vacuity` variant
  vs. a `VacuityVerdict::Rejected` mapped at the call site) is OQ-1.
  Source: `thermite-design.md` §7 ("a function does not certify until its
  contract certifies"; "reject with the proof as the explanation" — here the
  explanation is the syntactic cause).
- **REQ-6 (forge-check gate integration; the `contract_quality` field #6 sets):**
  the triage runs as a gate stage in `check::check_file`, on each item's
  `Contract`, BEFORE that item is lowered/verified. A contract failing triage is
  rejected before `verus` runs. On a PASS, the structural-triage verdict fills
  the `Certificate.contract_quality.tautology` field (set `true` only on a
  §7.1 (a) syntactic-`true`/identity ens reject — but since a reject does not
  produce an L3 cert, the LIVE semantics are: a non-`#[slag]` item that passes
  triage carries `tautology = false` and `vacuous_precondition = false` as
  ASSERTED (#6-live) values, no longer forward-declared placeholders). The
  SOLVER-derived truth of these fields (a genuine tautology, an unsat
  precondition) remains forward-declared for #13; the mutation fields
  (`mutants_killed`/`survivor`) stay forward-declared for #12. NO frozen schema
  field is added or renamed (R-SPEC-2); #6 only changes which producer sets the
  two existing `bool`s and when they go live. Any need for a *new* field
  (e.g. a distinct `structural_reject_cause`) is flagged OQ-2 and is a design
  amendment, not a code-local choice.
  Source: `thermite-design.md` §7; `.design/forge/certificate-manifest.md`
  REQ-3 (the forward-declared `contract_quality.*`); `goal.md` R-SPEC-2.
- **REQ-7 (slag exempts proving, never stating; triage still applies):** a
  `#[slag]` item is exempt from REQ-4 (maximal `fx` is justified by slag) but is
  STILL subject to REQ-1/REQ-2/REQ-3 — a slag function with a vacuous,
  result-omitting, or req-implied contract is rejected exactly like any other.
  Slag exempts PROVING (the L3 obligation, `slag.md`), never STATING or checking
  the contract (§8: "slag exempts you from *proving*, never from *stating and
  checking*"; `goal.md` R-DEFER-9).
  Source: `thermite-design.md` §8, §7.

## Acceptance criteria

ACs tie to a `conformance/vacuity/` oracle (authored by the orchestrator, NOT
this component): a reject fixture per (a)–(d) plus the corpus accept fixtures.
Each fixture is PARSE-VERIFIED below (it parses clean today; the listed AST is
the grounded `thermite_syntax::parse` output).

- **AC-1 (accept: the corpus is non-vacuous):** `conformance/sum.th`'s `sum` and
  `conformance/binary_search.th`'s `binary_search` both PASS triage (all four
  checks). Grounded: `sum`'s `ens result == spec_sum(xs)` and
  `ens result <= xs.len() as u64 * u32::MAX as u64` both mention `result`,
  neither is `BoolLit(true)` nor an identity, `req xs.len() <= 1_000_000` does
  not syntactically imply either `ens`, and `fx pure` is not maximal.
  `binary_search`'s `ens match result { … }` mentions `result`. The `spec fn`
  `spec_sum` has no `req`/`ens`/`fx` (`ast.rs` `SpecFnItem`) — it carries no
  contract, so triage does not apply to it.
- **AC-2 (reject (a) — ens is true):** `conformance/vacuity/ens_true.th` — a
  non-unit `fn` with `ens true` (sole clause `Expr::BoolLit(true)`) → rejected
  with cause (a). Companion `ens_eq_self.th` — `ens x == x`
  (`Binary{Eq, Path(["x"]), Path(["x"])}`) → rejected with cause (a).
- **AC-3 (reject (b) — ens omits result):** `conformance/vacuity/no_result.th` —
  a `fn f(x: u32) -> u32` whose only `ens` is `x <= 100` (no `result` path) →
  rejected with cause (b). A unit-return companion (`-> ()` with the same `ens`)
  PASSES (b) (the `Type::Unit` exemption), demonstrating the boundary.
- **AC-4 (reject (c) — ens implied by req):** `conformance/vacuity/ens_eq_req.th`
  — `req x <= 10` / `ens x <= 10` (identical `Clause.expr`) → rejected with
  cause (c). Companion `ens_conjunct_req.th` — `req x <= 10 && result == x` /
  `ens x <= 10` (the `ens` is a flattened conjunct of `req`) → rejected with
  cause (c).
- **AC-5 (reject (d) — maximal fx without slag):**
  `conformance/vacuity/maximal_fx.th` — a non-slag `fn` whose row is
  `fx read(a), write(b), net(c), alloc, time, rand, panic, diverge`
  (`EffectRow::Set` with all 8 variant kinds) → rejected with cause (d). The
  slag-justified counterpart (`conformance/slag/`’s maximal-row case) PASSES (d).
- **AC-6 (verdict names the cause):** each reject AC asserts the structured
  verdict identifies the SPECIFIC §7.1 cause (a/b/c/d) and, for (a)/(b)/(c), the
  offending `ens` clause index — a unit test against `vacuity.rs`'s public API,
  not against `verus`.
- **AC-7 (cert-quality field on a passing item):** a non-slag item that passes
  triage emits `contract_quality.tautology == false` and
  `vacuous_precondition == false` as ASSERTED #6-live values (REQ-6), verified
  against the corpus oracle once #6 lands (these two fields graduate from the
  `certificate-manifest.md` forward-declared set to live).

## Architecture

`vacuity.rs` is **pure, syntactic, solver-free** — it imports only
`thermite_syntax` AST types and produces a verdict. It is a new `mod vacuity;`
in `forge/src/main.rs`/`lib.rs`, consumed by `check.rs`.

The public entry is `pub fn triage(item: &FnItem) -> VacuityVerdict` (a
`spec fn` carries no contract — `ast.rs` `SpecFnItem` has no `req`/`ens`/`fx` —
so triage applies only to `FnItem`s; `check.rs` skips `Item::SpecFn`). Each
§7.1 rule is a private predicate over the `Contract`:

1. **(a) `ens_is_trivially_true`** — `BoolLit(true)` over every clause, or a
   syntactically-identical `Eq`/`Le`/`Ge` operand pair in any clause (REQ-1).
2. **(b) `ens_omits_result`** — `item.ret != Type::Unit` AND no `ens` clause's
   `Expr` tree contains an `Expr::Path` whose first segment is `"result"`
   (REQ-2). A recursive `Expr` walker visits `Call`/`MethodCall`/`Field`/
   `Binary`/`Index`/`Cast`/`Ref`/`Match`/`If`/`Closure` children.
3. **(c) `ens_implied_by_req`** — collect the conjunct set of `req` by flattening
   the left-associative `&&` tree (`flatten_and`); reject if every `ens` clause
   equals `req` whole or a member of that set (REQ-3).
4. **(d) `fx_maximal_without_slag`** — `item.slag.is_none()` AND
   `effect_row_is_maximal(&fx)` where maximal = an `EffectRow::Set` covering all
   8 `Effect` variant kinds (REQ-4).

The order is the §7.1 listing order (a, b, c, d); the first firing rule is the
reported cause (cheapest-first within the free tier — all four are O(AST size)).

**Gate integration (REQ-6, `.design/forge/check.md`).** In
`check::check_file`, after `validate`/`check_effects` and the per-item
sub-program split, `triage` runs on each `Item::Fn` BEFORE
`thermite_lower::lower` + `run_verus`. A `VacuityVerdict::Rejected` short-circuits
that item: no lowering, no `verus`, the certificate records a non-L3
contract-certification failure naming the §7.1 cause. A
`VacuityVerdict::Passed` lets the item proceed AND fixes
`contract_quality.{tautology, vacuous_precondition}` to asserted `false` for
the cert (the two `bool`s that §7 steps 2–3 (#13) will later be able to set
`true` on a SOLVER-confirmed tautology/unsat — #6 only owns the syntactic
verdict). This is the "starts filling `contract_quality`" the orchestrator
noted; `mutants_killed`/`survivor` stay forward-declared for #12.

**Slag interaction (REQ-7).** `triage` reads `item.slag` only for rule (d): a
`#[slag]` item skips (d) but runs (a)/(b)/(c). The slag field-VALIDATION
(non-empty `reason`/`owner`/`review`) is `slag.rs`'s job
(`.design/forge/slag.md`), run as a sibling gate stage; the two compose in
`check.rs` (slag-validate → triage → either L1-certify (slag) or proceed to L3).

**Scope boundaries (documented, attributed).** The SOLVER tautology check (§7
step 2) and the SOLVER vacuity / unsat-precondition check (§7 step 3) are issue
**#13**; mutation scoring + `mutants_killed`/`survivor` (§7 step 4) are issue
**#12**; strengthening probes (§7 step 5) are issue **#14**. #6 is the FREE
structural gate ONLY — it never issues a solver query.

## Verification

- `cargo test -p forge` — unit tests over `triage`'s public API: one reject test
  per §7.1 cause (a/b/c/d) and the `Type::Unit` / partial-`fx` / slag-justified
  boundary cases (AC-2..AC-6). Expected verdicts trace to `thermite-design.md`
  §7.1 and the hand-authored `conformance/vacuity/` fixtures (R-CHAR-3), never
  to `forge`'s own output.
- Conformance integration (`goal.md` model (B); the `conformance/vacuity` route
  reference): `forge check conformance/vacuity/<reject>.th` → non-L3 reported
  contract-certification failure naming the cause; `forge check
  conformance/sum.th` / `binary_search.th` still certify (AC-1 — triage does not
  regress the corpus). The accept side reuses the existing corpus; the reject
  fixtures are the new `conformance/vacuity/` oracle.
- `cargo clippy -p forge --all-targets -- -D warnings`, `cargo fmt --check`,
  anti-pattern gate.

## Exact `conformance/vacuity/` fixture programs (PARSE-VERIFIED)

All parse clean under `thermite_syntax::parse` today (verified by direct probe);
the grounded AST is noted per fixture. These are REJECT fixtures the orchestrator
authors; the ACCEPT side is the existing `conformance/sum.th` /
`binary_search.th`.

**`ens_true.th`** — reject (a):
```thermite
fn f(x: u32) -> u32
  req true
  ens true
  fx  pure
{ x }
```
Grounded: `ens#0.expr = BoolLit(true)`.

**`ens_eq_self.th`** — reject (a), identity form:
```thermite
fn f(x: u32) -> u32
  req true
  ens x == x
  fx  pure
{ x }
```
Grounded: `ens#0.expr = Binary { op: Eq, lhs: Path(["x"]), rhs: Path(["x"]) }`.

**`no_result.th`** — reject (b):
```thermite
fn f(x: u32) -> u32
  req true
  ens x <= 100
  fx  pure
{ x }
```
Grounded: `ret = Prim(U32)`, `ens#0.expr = Binary { op: Le, lhs: Path(["x"]),
rhs: IntLit(100) }` — no `result` path.

**`unit_ok.th`** — ACCEPTS (b) (the `Type::Unit` exemption boundary):
```thermite
fn f(x: u32) -> ()
  req true
  ens x <= 100
  fx  pure
{ }
```
Grounded: `ret = Unit` — exempt from (b). (Note: this fixture is rejected by (a)?
No — `x <= 100` is not trivially true; it passes (a), (c), (d) too, so it is a
clean ACCEPT demonstrating the unit exemption.)

**`ens_eq_req.th`** — reject (c), identical clause:
```thermite
fn f(x: u32) -> u32
  req x <= 10
  ens x <= 10
  fx  pure
{ x }
```
Grounded: `req.expr == ens#0.expr == Binary { op: Le, lhs: Path(["x"]),
rhs: IntLit(10) }`. (Also omits `result`, so (b) would fire too — the reported
cause is (b) under listing order; an alternate ens-with-result form that still
mirrors a req conjunct isolates (c): see `ens_conjunct_req.th`.)

**`ens_conjunct_req.th`** — reject (c), conjunct form (isolates (c): `ens`
mentions `result` via the req, so (b) does not fire on the req side, but the
`ens` clause itself is a req conjunct):
```thermite
fn f(x: u32) -> u32
  req x <= 10 && result == x
  ens x <= 10
  fx  pure
{ x }
```
Grounded: `req.expr = Binary { op: And, lhs: Binary{Le, Path(["x"]),
IntLit(10)}, rhs: Binary{Eq, Path(["result"]), Path(["x"])} }`; `ens#0.expr =
Binary{Le, Path(["x"]), IntLit(10)}` — a flattened conjunct of `req`. (This
`ens` still omits `result`, so to fully isolate (c) from (b) the orchestrator
may prefer an `ens result == x` clause that duplicates the second req conjunct;
both forms are valid (c) rejects — flagged OQ-3.)

**`maximal_fx.th`** — reject (d):
```thermite
fn f(x: u32) -> u32
  req true
  ens result == x
  fx  read(a), write(b), net(c), alloc, time, rand, panic, diverge
{ x }
```
Grounded: `fx = Set([Read("a"), Write("b"), Net("c"), Alloc, Time, Rand, Panic,
Diverge])` — all 8 variant kinds, `slag = None` → maximal without slag. (The
`ens result == x` is chosen so ONLY (d) fires, isolating the rule.)

## Open questions

- **OQ-1 (reject surface form):** does a triage reject become a new
  `ForgeError::Vacuity { item, cause }` variant, or a
  `VacuityVerdict::Rejected` that `check.rs` maps to a non-L3 certificate with
  the cause in the obligation diagnostic? The latter keeps a contract-quality
  failure inside the *certificate* (consistent with §7 "a function does not
  certify until its contract certifies" — a reported result, not an environment
  error). Leaning toward the verdict-in-certificate form; pinned for the builder
  + critic. A new `ForgeError` variant is a `cli.rs` change (R-SPEC-3 schema).
- **OQ-2 (cert field for the cause):** §7.1 (a) maps cleanly onto the existing
  `contract_quality.tautology` bool, but (b)/(c)/(d) have no dedicated frozen
  field (Appendix A's `contract_quality` is `{tautology, vacuous_precondition,
  mutants_killed, survivor}`). Options: (i) reuse `tautology`/
  `vacuous_precondition` for the closest §7 step and carry (b)/(d) only in the
  obligation diagnostic; (ii) propose a new `contract_quality.structural`
  sub-field — a design amendment (R-SPEC-2), NOT code-local. Default: option (i)
  (no schema change); surface (ii) as the amendment if the critic wants the
  cause machine-readable in the cert.
- **OQ-3 (isolating (c) from (b)):** the cleanest (c)-only fixture is one whose
  `ens` mentions `result` AND duplicates a `req` conjunct (so (b) cannot fire);
  the orchestrator picks the exact fixture. Noted so the reject ACs are not
  conflated.
- **OQ-4 (`ens` conjunction semantics for (a)):** the conservative rule rejects
  on (a) only when EVERY clause is trivially true (or any clause is a syntactic
  identity). A contract `ens true` + `ens result == x` is NOT (a)-rejected (it
  carries a real conjunct). Confirmed as the intended reading of "ens simplifies
  to true"; flagged in case the critic reads §7.1 (a) as "any clause is true".

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (ens-is-true reject (a)) | SHIPPED | `ens_is_trivially_true` in `vacuity.rs` (`BoolLit(true)` over every clause, or any `identity_clause` `Eq`/`Le`/`Ge` with `PartialEq`-equal operands); consumer `triage` → `check::gate_fn`. Verified: `vacuity::tests::{ens_literal_true_rejected_a, ens_identity_rejected_a, identity_class_covers_le_ge_not_lt_ne}` + `tests/vacuity_slag_conformance.rs::triage_rejects_match_oracle_cause` (cause `EnsIsTrivial`). |
| REQ-2 (ens-omits-result reject (b)) | SHIPPED | `ens_omits_result` + bounded `expr_mentions_result` (whole-`Expr`/`Stmt`/`Block` walk, `MAX_EXPR_DEPTH`) with the `Type::Unit` exemption; consumer `triage`. Verified: `vacuity::tests::{ens_omits_result_rejected_b, unit_return_exempt_from_b, nested_result_mention_passes_b}` + the conformance `EnsOmitsResult` reject + `unit_omits_result_ok` accept. |
| REQ-3 (ens-implied-by-req reject (c)) | SHIPPED | `ens_implied_by_req` + `flatten_and` (left-associative `&&` flatten, bounded); consumer `triage`. Verified: `vacuity::tests::{ens_eq_req_rejected_c, ens_conjunct_req_rejected_c}` + conformance `EnsImpliedByReq` rejects. |
| REQ-4 (maximal-fx-without-slag reject (d)) | SHIPPED | `fx_maximal_without_slag` + `effect_row_is_maximal` (all 8 `Effect` kinds in a `Set`) gated on `slag.is_none()`; consumer `triage`. Verified: `vacuity::tests::{maximal_fx_no_slag_rejected_d, maximal_fx_with_slag_passes_d, partial_fx_is_not_maximal}` + conformance `MaximalFxWithoutSlag`. |
| REQ-5 (`VacuityVerdict` + typed cause) | SHIPPED | `pub enum VacuityVerdict { Passed, Rejected { cause } }` + `pub enum VacuityCause` with `tag`/`detail`; mapped to `manifest::RejectReason` (verdict-in-cert, OQ-1 resolved — NOT a `ForgeError`). Consumed by `check::gate_fn`. |
| REQ-6 (forge-check gate + `contract_quality` field) | SHIPPED | `check::gate_fn` runs `triage` BEFORE `lower`/`run_verus`; a reject short-circuits to `Certificate::rejected`; a pass calls `Certificate::graduate_triage_clean` so `contract_quality.{tautology,vacuous_precondition}` are #6-LIVE `false`. Verified: `corpus_sum_still_l3_and_matches_golden` asserts both bools == golden `false`. |
| REQ-7 (slag exempts proving, not stating) | SHIPPED | `triage(item)` reads `item.slag` and gates ONLY rule (d); (a)/(b)/(c) always run. Verified: `vacuity::tests::slag_does_not_excuse_vacuous_ens` + conformance `slag_vacuous` → `EnsIsTrivial`. |
