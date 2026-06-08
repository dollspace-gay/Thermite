# Contract-Faithfulness Translation Validation

<!--
tier: 3-component
status: draft
governs: thermite-tv/src/ref_encode.rs, thermite-tv/src/obligation.rs, forge/src/contract_tv.rs
thesis-refs:
  - thermite-design.md §1 (trust relocated twice: code → spec → spec-intent)
  - thermite-design.md §4.1 (contract-first functions: req/ens/fx, inv/dec)
  - thermite-design.md §4.2 (the deliberately-weak SpecTherm sublanguage; the frozen combinator cage)
  - thermite-design.md §6 (the verification ladder; L3 = Verus-derived SMT proof)
  - thermite-design.md §7 (the vacuity battery — and the gap it explicitly does not close)
epic: crosslink #139
-->

## Summary

`forge check` certifies that the **emitted** Verus contract holds for the implementation. It does
NOT certify that the emitted contract *means the same thing* as the source contract the author
wrote. Every existing guard takes the emitted contract as ground truth (verus-on-emitted, the cert
oracle, the vacuity/mutation battery, the critic) or is corpus-bounded (golden files). This
component adds **contract-faithfulness translation validation (TV)**: an INDEPENDENT reference
encoder for the SpecTherm contract sublanguage, plus a per-clause Z3 equivalence obligation
(`assert(P_production <==> P_reference)`) discharged through the existing verus path. A divergence
is a real lowering-fidelity bug (the #122 cast-paren and #127 byte-view-misdispatch classes).
Scope: the **contract/spec sublanguage only** (`req`/`ens`/`inv`/`dec` + spec fns) — NOT exec body
terms (epic #139 step 2, kernel-gated). This is the §1 "code → spec" relocation made mechanical: the
golden-file principle (a hand-certified external reference) generalized from per-program to
language-level.

## Trust model (state it plainly — this is N-version differential validation, not proof)

TV checks `production-lowering ≡ reference-encoding`. Agreement is **evidence**, not **proof**: both
encoders could share the same wrong assumption and agree on a wrong answer. What makes the evidence
meaningful is asymmetry of auditability — the reference encoder is small, declarative, and covers
only the frozen contract sublanguage (comparisons, logical connectives, `result`/`old`, named
spec-fn calls, the 8 frozen combinators, and the spec-context rewrites). A human can certify it by
inspection in minutes; the production `lower_expr` is ~2000 lines threaded with shape-keyed rewrites
and cannot. So "production agrees with an independently-auditable reference, on every clause, for all
inputs (Z3)" relocates faithfulness from *audit the 5000-line lowerer* to *audit the small reference
encoder + trust Z3 finds disagreement*. The reference encoder IS the mechanized contract semantics
in operational form. The honesty boundary is hard (see REQ-1): if the reference reuses production's
`lower_expr in lower.rs`, independence is lost and the check is vacuous (`assert(X <==> X)` always
verifies). The two encoders MUST be authored against the same external spec — `thermite-design.md`
§4.2 + the frozen `REGISTRY.verus_l3` — never against each other.

## Requirements

- **REQ-1 (independent reference encoder)** — `thermite-tv::ref_encode::ref_contract_pred(clause.expr, &RefCtx) -> Result<VerusPredicate>` maps a SpecTherm contract clause's `Expr` (`thermite-syntax::ast::Expr`) to a Verus predicate STRING, covering exactly the contract sublanguage (`thermite-design.md` §4.2): comparisons (`Expr::Binary` with `BinOp::Eq|Ne|Lt|Le|Gt|Ge`), logical connectives (`And`/`Or`/`Unary::Not`), arithmetic in subterms, `result`/`old(x)` references (treated as free vars), named `spec fn` calls (`Expr::Call`), the 8 frozen combinators (resolved via `thermite_spec::lookup(name).verus_l3`), and the spec-context rewrites: slice→`@` / `&xs[..i]`→`xs@.subrange(0, i as int)`, method→`spec_*` (the #127 byte-view dispatch: `.len()`→`.spec_len()`, `.byte_at(i)`→`.spec_byte_at(i as int)`), integer cast→`as nat`/`as int` (the #122 cast-paren class). Derived from `thermite-design.md` §4.2. **HARD CONSTRAINT (R-CHAR-3 / the trust model):** it MUST NOT call `thermite_lower::lower::lower_expr` or any production lowering symbol — independence is the entire point.
- **REQ-2 (per-clause Z3 equivalence obligation + discharge path)** — `thermite-tv::obligation::clause_obligation(clause, production_pred, ref_ctx) -> Result<VerusObligation>` emits a `proof fn tv_<clause-addr>(<free vars + result/old as params>) requires <enclosing req> { assert(P_production <==> P_reference); }` Verus unit, with the clause's spec-fn deps + the referenced combinators' `verus_l3` defs in scope. It is discharged through the EXISTING `forge::check::run_verus(program, lowered, seed, rlimit) -> VerusResult` path; VERIFIED ⟺ faithful, a counterexample ⟺ infidelity. Derived from `thermite-design.md` §6 (L3 = the verus-derived SMT proof). The production side reuses `lower_fn_signature in lower.rs`'s clause output verbatim (the artifact under test); the reference side is REQ-1.
- **REQ-3 (off-corpus generator — the corpus-bound escape)** — `thermite-tv::gen::gen_clauses(seed, budget) -> impl Iterator<Item = Clause>` produces well-typed contract clauses over the frozen sublanguage (typed `Expr` trees: comparisons over slice/scalar/spec-fn/combinator terms, with `result`/`old` and the 8 combinators), and the TV obligation runs on each. This is what un-bounds fidelity checking: golden files are per-corpus; TV-over-generated-clauses is not. Derived from `thermite-design.md` §1 (trust a skeptical third party can audit — here, audit of the *encoder* + machine-checked agreement over an unbounded clause space). Seeded + deterministic (R-CODE-5).
- **REQ-4 (the teeth — R-CHAR-3)** — a conformance test asserting (a) a FAITHFUL clause's obligation VERIFIES and (b) each of three INJECTED infidelities produces a verus COUNTEREXAMPLE: the `==`→`<=` binop swap, a wrong combinator predicate (`|x| x < 10` for source `|x| x <= 10`), and the #127-class wrong byte-view rewrite (`spec_byte_at(1)` for source index `0`). This is the proof TV catches what the vacuity/mutation battery and verus-on-emitted structurally cannot (`thermite-design.md` §7 — the battery takes the emitted contract as ground truth). Expected values trace to `thermite-design.md` §4.2 + the frozen `REGISTRY.verus_l3`, never to the lowerer's output.
- **REQ-5 (forge plug-in point)** — a new `forge::contract_tv` check phase runs the TV obligation over each `req`/`ens`/`inv`/`dec` clause of every checked item, alongside the vacuity/mutation gates (`forge/src/vacuity.rs`, `forge/src/mutation.rs`). The non-test consumer is `forge::check`. A TV counterexample is a hard fidelity failure surfaced in the certificate (it is NOT a contract-too-weak signal — that is mutation; TV is a *meaning-mismatch* signal). Derived from `thermite-design.md` §6/§7 (the gate runs the battery inside itself).

## Acceptance criteria

- **AC-1 (faithful → verified)** — the obligation for a real corpus clause (`sum`'s `ens result as nat == spec_sum(xs@)`, from `conformance/sum.th` / `tests/golden/lower/sum.verus.rs`) with `P_production == P_reference` discharges through `run_verus` as `success: true, errors: 0`. GROUNDED below.
- **AC-2 (==-vs-<= infidelity → counterexample)** — the SAME obligation with the production side weakened to `result as nat <= spec_sum(xs@)` fails verus with `errors: 1` and a counterexample at the `assert(P_production <==> P_reference)` site. GROUNDED below.
- **AC-3 (combinator clause → discharges via REGISTRY.verus_l3)** — a clause using `forall_in(xs, |x| x <= 10)` encodes both sides through the frozen `thermite_spec::lookup("forall_in").verus_l3` body and the faithful obligation VERIFIES; a wrong-predicate variant (`|x| x < 10`) FAILS with a counterexample. GROUNDED below.
- **AC-4 (#127-class wrong rewrite → counterexample)** — a `&String`-param clause whose production side mis-dispatched the byte-view (`spec_byte_at(1 as int)` for source index `0`) FAILS with a counterexample; the faithful index-`0` version VERIFIES. GROUNDED below.
- **AC-5 (result/old + free-var binding discharges)** — an obligation binding `result` and an `old(acc)` value as distinct params, under an enclosing `requires`, discharges. GROUNDED below.
- **AC-6 (independence is structural)** — `thermite-tv` does NOT depend on `thermite-lower` for the reference encoder path; a `cargo tree`/grep audit shows `ref_encode.rs` references no `lower_expr` symbol. (Verification: REQ-1 + a grep guard test.)
- **AC-7 (off-corpus coverage)** — `gen_clauses` produces ≥ N generated clauses and the TV obligation runs (and VERIFIES for the faithful lowerer) on each; a seeded run is reproducible.

## Architecture

**The home (the new artifacts).** Two new units:

1. `thermite-tv` — a NEW workspace crate. `src/ref_encode.rs` is the independent reference encoder
   (REQ-1); `src/obligation.rs` builds the per-clause Verus obligation (REQ-2); `src/gen.rs` is the
   off-corpus generator (REQ-3). The crate depends on `thermite-syntax` (for `ast::Expr`/`Clause`/
   `BinOp`) and `thermite-spec` (for `lookup`/`CombinatorSig.verus_l3` — the frozen shared ground
   truth). It MUST NOT depend on `thermite-lower` (AC-6 — the independence boundary). A separate
   crate is the home (not a module inside `thermite-lower`) precisely so the dependency graph makes
   accidental reuse of `lower_expr in lower.rs` a compile error rather than a temptation.
2. `forge/src/contract_tv.rs` — the check phase (REQ-5). It already has the production-side clause
   text (the `requires`/`ensures` lines `lower_fn_signature in lower.rs` produced), calls
   `thermite_tv::ref_encode::ref_contract_pred` for the reference side, builds the obligation via
   `thermite_tv::obligation::clause_obligation`, and discharges it through the existing
   `forge::check::run_verus`.

**The independence boundary (precisely what is re-implemented vs reused).**

- **RE-IMPLEMENTED by the reference encoder (the infidelity surface):** the spec-context rewrites.
  These are exactly where production fidelity bugs live. The reference encoder authors them AGAINST
  `thermite-design.md` §4.2 directly, independently of `lower_expr in lower.rs`:
  - slice→`@` view at use sites; `&xs[..i]`→`xs@.subrange(0, i as int)`.
  - method→`spec_*` byte-view dispatch (`.len()`→`.spec_len()`, `.byte_at(i)`→`.spec_byte_at(i as int)`)
    — the #127 misdispatch class.
  - integer cast→`as nat`/`as int` with correct parenthesization of a binary/unary inner — the #122
    cast-paren class.
  - `binop(BinOp) in lower.rs` is a 1-to-1 map; the reference re-states the SAME map independently
    (the `==`/`<=` distinction is the canonical teeth case, AC-2). Re-stating it is the point: if the
    reference imported production's `binop`, a production binop bug would be invisible.
- **REUSED (the shared frozen ground truth — reuse is correct here):** `thermite_spec::lookup(name).verus_l3`,
  the frozen Verus `spec fn` body for each of the 8 combinators (e.g. `forall_in` →
  `forall|i: int| 0 <= i < s.len() ==> #[trigger] p(s[i])`, `combinators.rs`). The registry IS the
  combinator spec (`thermite-design.md` §4.2 "frozen SMT triggers"); both production
  (`emit_combinator_defs in lower.rs`, REQ-6) and the reference encoder read it. Sharing the registry
  is NOT a loss of independence — the registry is the external spec, not a production artifact;
  divergence between the two encoders over a *combinator argument rewrite* (predicate lowering, the
  slice→`@` of the combinator's slice arg) is still caught, because that rewrite is RE-implemented
  while only the combinator *body* is shared.

**The contract sublanguage is small + frozen (why the reference is auditable).** Per `thermite-design.md`
§4.2 SpecTherm has no general quantifiers (only the 8 frozen combinators), flat predicate closures
(no anonymous nested quantifiers), and named total `spec fn`s. There are no statements, loops,
mutation, or exec bodies in a clause — a `Clause` holds an ordinary `Expr` (`ast.rs`: `struct Clause
{ expr, text, span }`). The reference encoder is therefore a small total recursion over the
comparison/connective/call/combinator subset of `Expr`, not a re-implementation of all of
`lower_expr`.

**The per-clause obligation shape (REQ-2).** For each clause, with free vars `v1..vn` (the clause's
referenced slice/scalar params) + `result` (when the return is non-unit) + `old(x)` values bound as
distinct params, under the enclosing `req` as a `requires`, and the clause's spec-fn + combinator
`verus_l3` deps in scope:

```
proof fn tv_<addr>(<params>) requires <enclosing_req> {
    let p_production: bool = <production clause text>;
    let p_reference:  bool = <ref_contract_pred output>;
    assert(p_production <==> p_reference);
}
```

VERIFIED ⟺ the two predicates are logically equivalent for ALL inputs (Z3) ⟺ faithful. A
counterexample is an input on which they differ — a concrete witness of the lowering infidelity
(`thermite-design.md` §5.1 "counterexamples, not adjectives").

**Scope boundary (honest, hard).** This is CONTRACT/spec-sublanguage TV: `req`/`ens`/`inv`/`dec` +
the spec fns they call. It is NOT exec-body/term TV. The #122-class body-exec lowering bugs (a wrong
exec rewrite in a function *body*) stay caught by golden-files-on-corpus until epic #139 step 2 (the
kernel-gated body TV) lands. The reason: a clause is a pure predicate with an independent
denotational reference; an exec body has control flow, mutation, and the L1/L2/L3 ladder — its TV
needs a different (operational) reference and is out of scope here. State this in the certificate so
no one reads contract-TV as body-faithfulness.

## Verification

GROUNDED end-to-end against the real `verus` binary (`Verus 0.2026.05.24.ecee80a`) during authoring.
The conformance test (REQ-4) replays these through `forge::check::run_verus`.

**AC-1 (faithful sum `ens` → verified).** Obligation: `proof fn tv_ens_sum(result: u64, xs:
Seq<u32>) { let p_production = result as nat == spec_sum(xs); let p_reference = result as nat ==
spec_sum(xs); assert(p_production <==> p_reference); }` with the frozen `spec_sum` def in scope.
Result:

```
"verification-results": { "success": true, "verified": 2, "errors": 0 }
```

**AC-2 (==-vs-<= infidelity → counterexample).** SAME obligation, production side weakened to
`result as nat <= spec_sum(xs)`:

```
error: assertion failed
  --> teeth_le.rs:15:12
   |
15 |     assert(p_production <==> p_reference);
"verification-results": { "success": false, "verified": 1, "errors": 1 }
```

**AC-3 (combinator clause via REGISTRY.verus_l3).** Using the frozen `forall_in` body verbatim from
`thermite_spec::lookup("forall_in").verus_l3`: the faithful obligation (`forall_in(xs, |x| x <= 10)`
both sides) VERIFIES; the wrong-predicate variant (production `|x| x < 10`, reference `|x| x <= 10`):

```
error: assertion failed
  --> teeth_combinator.rs:22:12   (the wrong-pred obligation)
"verification-results": { "success": false, "verified": 1, "errors": 1 }
```

**AC-4 (#127-class wrong byte-view rewrite → counterexample).** A `&TString`-param clause; production
mis-dispatched `s.byte_at(0)` to `spec_byte_at(1 as int)` (wrong index), reference encoded index 0:

```
error: assertion failed
  --> teeth_rewrite.rs:29:12
"verification-results": { "success": false, "verified": 1, "errors": 1 }
```

(The faithful index-0 version, with `requires s.spec_len() >= 1`, VERIFIES.)

**AC-5 (result/old + free-var binding).** `proof fn tv_old_faithful(result: u64, old_acc: u64)
requires old_acc < u64::MAX { ... result == (old_acc + 1) as u64 ... }` discharges:

```
"verification-results": { "success": true, "verified": 1, "errors": 0 }
```

These confirm the load-bearing feasibility questions: a per-clause obligation with bound free vars +
`result`/`old` + spec-fn deps + combinator `verus_l3` deps DOES discharge in verus, and the teeth
bite on exactly the infidelity classes (==/<=, wrong combinator predicate, wrong byte-view rewrite)
that the vacuity battery and verus-on-emitted cannot see.

**Crate gauntlet (when built):** `cargo test -p thermite-tv`, `cargo test -p forge`
(`contract_tv` conformance), `cargo clippy -p thermite-tv -p forge --all-targets -- -D warnings`,
`cargo fmt --check`. Scratch/verus temp cleaned per the `ScratchDir` Drop guard (blocker #53).

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (independent reference encoder) | SHIPPED | `thermite_tv::ref_encode::ref_contract_pred` (`thermite-tv/src/ref_encode.rs`) — independently encodes the contract sublanguage (binop map, slice→`@`, method→byte-view dispatch on the RECEIVER shape, cast→`nat`/`int`), reusing ONLY `thermite_spec::lookup(name).verus_l3` for the 8 combinators. Non-test consumer: `thermite_tv::obligation::equivalence_obligation`. The crate depends on `thermite-syntax` + `thermite-spec` ONLY (`thermite-tv/Cargo.toml`) — NO `thermite-lower` (`cargo tree -p thermite-tv` shows syntax + spec only), so independence is a COMPILE constraint (AC-6). Verified by `thermite-tv/tests/teeth.rs` F1–F4 under real verus. |
| REQ-2 (per-clause Z3 equivalence obligation + discharge) | SHIPPED | `thermite_tv::obligation::equivalence_obligation` + `ObligationFrame`/`ParamDecl` (`thermite-tv/src/obligation.rs`) — emits a self-contained `proof fn tv_check(<params>) requires <req> { assert((P_production) <==> (P_reference)); }` with the spec-fn/combinator `verus_l3` defs + free vars + `result`/`old(_)` params in scope. Discharged through real verus by `thermite-tv/tests/teeth.rs` (the obligation TEXT's non-test consumer; the forge discharge phase is REQ-5/#144). GROUNDED: F1 faithful → `2 verified, 0 errors`; F1 infidel (`<=`) → `1 verified, 1 errors`. |
| REQ-3 (off-corpus generator) | NOT-STARTED | open prereq blocker #142. No `thermite-tv/src/gen.rs` clause generator exists (next dispatch, not on the builder manifest). This is the corpus-bound escape; v1 grounds on the four teeth fixtures (REQ-4) while the generator is built — REQ-3 delivers the thesis payoff (un-bounding fidelity checking). |
| REQ-4 (the teeth — R-CHAR-3) | SHIPPED | `thermite-tv/tests/teeth.rs` — the four orchestrator fixtures F1 (comparison `==`/`<=`), F2 (combinator predicate `<`/`<=`), F3 (#127 byte-view index `0`/`1`), F4 (structural-drop conjunct): each FAITHFUL p_production VERIFIES (`0 errors`) and each INFIDEL produces a verus COUNTEREXAMPLE at the `assert(P_production <==> P_reference)` site (`errors >= 1`). Skip-loudly if verus absent (mirrors `lower_conformance.rs`). Expected values trace to the fixtures + `thermite-design.md` §4.2 + `REGISTRY.verus_l3` (R-CHAR-3, never the lowerer's output). |
| REQ-5 (forge plug-in point) | NOT-STARTED | open prereq blocker #144. No `forge/src/contract_tv.rs` and no route entry; the phase that runs the obligation over each `req`/`ens`/`inv`/`dec` clause alongside `vacuity.rs`/`mutation.rs` is unbuilt. The consumer-to-be is `forge::check`. |
