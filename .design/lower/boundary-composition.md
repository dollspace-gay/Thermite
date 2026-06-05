# Boundary/Slag Composition Lowering — verify-through-the-contract
<!--
tier: 3-component
status: draft
governs: thermite-lower/src/lower.rs, forge/src/check.rs
thesis-refs:
  - thermite-design.md §9
  - thermite-design.md §8
  - thermite-design.md §6
-->

## Summary

`thermite-design.md` §9 promises composition: "if `g` calls `f` only through
`f`'s contract, then `g`'s certificate is valid independent of `f`'s body. Trust
is invariant under composition." Today that promise is a LABEL, not a STATUS: a
pure-Thermite `g` whose body calls a `#[boundary]` (foreign body, #16) or
`#[slag]` (fiat-trusted body, §8) fn `f` is lowered with `f` UNDEFINED — `forge
check`'s per-item sub-program (`item_subprogram in check.rs`) weaves in only the
file's `spec fn`s, never the referenced `#[boundary]`/`#[slag]` siblings — so
`verus` cannot resolve the foreign call, the run errors, and `g` lands L0. #17's
classification has already labelled `g` `ToBoundary { via: f }` (scope ⊥ level),
but `g`'s level can never reach L3.

This component closes that gap: the L3 lowering emits each referenced
`#[boundary]`/`#[slag]` fn as a **verus-assumable signature** — its contract
(`requires`/`ensures`) under `#[verifier::external_body]`, with NO body — so `g`'s
proof resolves `f` and discharges against `f`'s ensures. `g` reaches **L3** (its
own body proves against the contract); `g`'s `assurance_scope` stays
`to_boundary` (#17 — the guarantee depends on `f` honoring its contract,
L1-enforced at the crossing). `f` itself is UNCHANGED — `Level::L1` +
boundary/slag flag, its own body never proved (the §16/§8 path).

This doc is **FORWARD-LOOKING / GREENFIELD**. No `external_body` emission exists
anywhere in `lower.rs` today, and `item_subprogram` weaves no boundary/slag
siblings (verified below). Every REQ is **NOT-STARTED**, blocked on **crosslink
issue #52** (found during #17). #52 already tracks this — file no new blocker.

## The verus mechanism (GROUNDED — `verus 0.2026.05.24`)

Verus's `#[verifier::external_body]` is the exact primitive: the function's BODY
is NOT checked, while its `requires`/`ensures` are ASSUMED at every call site.
This is verus's first-class way to model a function whose body it cannot (or must
not) prove — precisely the §9/§8 foreign/fiat residue. Authoring harnesses (run
against the real binary; scratch removed):

**(1) The caller proves THROUGH the contract.** An `external_body` `ext_id` with
`requires x < 100, ensures r == x` and an `unimplemented!()` body, plus a caller
`g` that calls it:

```rust
#[verifier::external_body]
fn ext_id(x: u32) -> (r: u32) requires x < 100, ensures r == x, { unimplemented!() }
fn caller(x: u32) -> (r: u32) requires x < 100, ensures r == x, { ext_id(x) }
```

→ `verus --output-json`: `"success": true, "verified": 1, "errors": 0` (exit 0).
`caller` PROVES at L3 using `ext_id`'s ASSUMED ensures, with no `--no-cheating`
or any cheat flag — verus's default mode. The body of `ext_id` is never examined.

**(2) Soundness — the caller must still honor `f`'s `req`.** A caller with
`requires true` (NOT establishing `x < 100`) calling `ext_id(x)`:

```
error: precondition not satisfied
 --> ...:17:5    |  6 | requires x < 100,  failed precondition
 ...  17 |     ext_id(x)
verification results:: 0 verified, 1 errors      (exit 1)
```

A COUNTEREXAMPLE, not a false L3. The external_body assumes `f`'s ensures, but
the caller is still obliged to ESTABLISH `f`'s requires at the call site.

**(3) Soundness — the caller must still prove its OWN `ens`.** A caller claiming
`ensures r == x + 1` (stronger than what `ext_id`'s `ensures r == x` delivers):

```
error: postcondition not satisfied
 --> ...:15:13   |  15 | ensures r == x + 1,  failed this postcondition
verification results:: 0 verified, 1 errors      (exit 1)
```

Again a counterexample. The external_body assumes ONLY `f`'s ensures — nothing
more; the caller cannot manufacture a guarantee `f`'s contract does not deliver.

### THE HONESTY ARGUMENT (R-DEFER-9 — pinned hard)

`#[verifier::external_body]` makes the body opaque. A `external_body` fn whose
body LIES (`{ x + 1 }` under `ensures r == x`) still "verifies"
(`0 verified, 0 errors`) — the body is never checked. The IDENTICAL body in a
REGULAR fn (no `external_body`) is CHECKED and FAILS (`postcondition not
satisfied`, verified). So external_body is a body-proof EXEMPTION.

That exemption is HONEST **iff** it is emitted ONLY for a DECLARED trust boundary:

- A `#[boundary]` fn (§9) has a FOREIGN body (`body: None`, ffi-boundary.md
  REQ-2) — there is genuinely no Thermite body to prove; its contract is enforced
  at runtime by the L1 wrapper (`thermite_lower::l1`, ffi-boundary.md REQ-4).
- A `#[slag]` fn (§8) has a fiat-trusted body — proving exempted by declaration,
  contract enforced at L1 (slag.md, §6 "L1 because the contract is L1-checked").

These ARE the trusted-by-fiat residue the §9 TCB sentence enumerates ("slag
blocks ∪ boundary contracts ∪ the toolchain", #15). Emitting their contracts as
`external_body` signatures in the lowered verus STRING is the HONEST modeling of
a foreign function — not a proof cheat — because:

1. It is emitted **only** for a fn ALREADY classified `#[boundary]`/`#[slag]` and
   ALREADY certified `Level::L1` (the §16/§8 path); NEVER for a regular Thermite
   fn (which must be FULLY proved — harness (3)'s contrast).
2. The caller still proves its OWN body and discharges `f`'s `req` (harnesses 2,
   3) — composition is SOUND, not a free pass.
3. The crossing is L1-enforced at runtime: if `f` violates its contract, the L1
   wrapper detects it at the boundary (§6/§9). The L3 proof is honestly scoped
   `to_boundary` (#17): "verified, GIVEN `f` honors its contract."

**emitted-verus vs our-Rust-src distinction (load-bearing).** The anti-pattern
gate (`tooling/anti-pattern-gate.py`, R-DEFER-9) forbids `#[verifier::external]`
(note: `external`, not `external_body`) appearing in OUR toolchain's `.rs`
production source — that would dodge a proof of code we wrote. #52 emits
`#[verifier::external_body]` (a different attribute) INTO the lowered verus
STRING `lower` produces for a `#[boundary]`/`#[slag]` fn — a generated artifact
describing a foreign function, not a proof-dodge of our own code. The gate
operates on `.rs` files in the repo; `lower`'s output is a `String` handed to the
`verus` binary, never an `.rs` file in the tree. The two are categorically
distinct, and the doc/builder MUST keep them so.

## The lowering change (where #52 touches)

The fix is at the boundary between two existing components — the doc governs the
two files #52 must touch:

- **`forge/src/check.rs` (`item_subprogram`).** Today `item_subprogram(item,
  spec_items)` builds a `fn`'s isolated sub-`Program` as `spec_items + [item]`
  (verified — only `Item::SpecFn` is woven in via the `spec_items` filter in
  `check_file_with_options`). It does NOT include the `#[boundary]`/`#[slag]`
  siblings the `fn`'s body references, so the lowered verus has an UNDEFINED
  callee. #52 extends the woven set: for a caller `fn`, also include each
  IN-FILE `#[boundary]`/`#[slag]` `Item::Fn` the body transitively references (the
  #17 `closure.rs` call-graph already computes exactly this reachability — the
  natural seam to reuse), so `lower` sees the referenced foreign fns. This is the
  PRIMARY change site (the sub-program composition is forge's job).
- **`thermite-lower/src/lower.rs` (`lower` / `lower_fn`).** `lower` must emit a
  woven `#[boundary]`/`#[slag]` `FnItem` (which has `body: None` for boundary, or
  a fiat body for slag) NOT as a normal `fn` (today `lower_fn_body` returns
  `LowerError::Unsupported` on a `body: None` boundary fn — verified) but as a
  `#[verifier::external_body]` SIGNATURE: the `requires <req>` + `ensures <ens>`
  lowered from its `Contract` (the existing `lower_fn` requires/ensures emission),
  with a synthetic `{ unimplemented!() }` (or verus's no-body external_body form)
  in place of a real body. The `requires`/`ensures` lowering is UNCHANGED (same
  spec-context machinery, REQ-5 of verus-lowering.md); only the attribute + the
  body-suppression is new. This is the SECONDARY change site (the emission shape
  is the lowerer's job). The spec-routes entry for `lower.rs` already points at
  `verus-lowering.md`; #52 adds this doc as a second governing route OR amends
  `verus-lowering.md` — the orchestrator decides the route shape (see OQ-3).

The `#[boundary]`/`#[slag]` fn's OWN certificate is produced by check.rs's
`gate_fn` EARLY (the §16/§8 `BoundaryL1`/`SlagL1` short-circuit — verified at
`check.rs` `gate_fn`), entirely BEFORE the L3 path, and #52 does not touch it: it
stays `Level::L1` + flag, body never proved. The external_body signature is a
COPY of its contract woven into the CALLER's sub-program only — the boundary fn
is never itself lowered to a real verus body.

## Requirements

- **REQ-1 (assumable-signature emission, boundary/slag only):** the L3 lowering
  emits each referenced `#[boundary]`/`#[slag]` fn into a CALLER's sub-program as
  a `#[verifier::external_body]` verus signature — `requires <req>` + `ensures
  <ens>` lowered from its `Contract`, with NO checked body — so the caller's proof
  resolves the callee and uses its assumed `ensures`. `external_body` is emitted
  ONLY for a fn with `boundary.is_some() || slag.is_some()`; a regular Thermite
  `Item::Fn` is ALWAYS lowered to a fully-proved body (never external_body). The
  emission keys on the syntactic `#[boundary]`/`#[slag]` flag (`FnItem.boundary` /
  `FnItem.slag` in `ast.rs`), never on a name. Derived from §9 (the composition
  rule) + §8 (the fiat-trusted body) + `goal.md` R-DEFER-9 (the honest foreign
  model, NOT a proof cheat).
- **REQ-2 (boundary-caller reaches L3 + scope `to_boundary`):** a pure-Thermite
  fn `g` whose body calls a `#[boundary]`/`#[slag]` fn `f`, and which honors `f`'s
  `req` at the call site + proves its own `ens`, certifies at `Level::L3` (its own
  body SMT-proves against `f`'s contract) AND records `assurance_scope =
  ToBoundary { via: f }` (#17, already shipped). Scope ⊥ level: L3 and
  to-the-boundary coexist. Derived from §9 (composition independence) + §6 (the
  per-fn body-proof level) + the #17 `scope ⊥ level` decision.
- **REQ-3 (the boundary/slag fn itself is unchanged — L1 + flag):** the
  `#[boundary]`/`#[slag]` fn `f` ITSELF stays `Level::L1` + `boundary`/`slag`
  flag, its own body NEVER proved (the §16/§8 `gate_fn` `BoundaryL1`/`SlagL1`
  short-circuit, before the L3 path). #52 emits `f`'s contract as an external_body
  signature into the CALLER's sub-program only; it never lowers `f`'s real body,
  and it never changes `f`'s own certificate. Derived from §8/§6 (the L0/L1 +
  flag certificate) + ffi-boundary.md REQ-5.
- **REQ-4 (soundness — violation is a counterexample, not a false L3):** a caller
  that does NOT establish `f`'s `req` at the call site, or that asserts an `ens`
  stronger than `f`'s contract delivers, FAILS verification (a verus
  counterexample → a non-L3 cert with a witness, the existing #5 failure path) —
  NOT a false L3. The external_body assumes ONLY `f`'s `ensures`; the caller's
  own obligations stand. Derived from §9 (the contract, not the body, is the
  interface) + §7/R-DEFER-9 (no obligation is discharged by vacuity) + the
  grounded harnesses (2)/(3).

## Acceptance criteria

ACs tie to a `conformance/composition/` oracle (a hand-derived cases file, the
`conformance/e2e/cases.json` precedent — authored by the ORCHESTRATOR, not this
doc; R-CHAR-3, expected values hand-derived from the call graph + verus
semantics, never copied from forge output). The #17 `conformance/e2e/cases.json`
`to_boundary` fixtures (`direct_boundary_caller`, `transitive_boundary_caller`,
`slag_caller`) are the SAME programs whose scope #17 pinned; #52's oracle pins
their LEVEL (which today is L0 and #52 makes L3). The exact fixtures:

- **AC-1 (direct boundary caller → L3 + to_boundary):** the #17
  `direct_boundary_caller` program — `#[boundary("ext::ext_id")] fn ext_id(x:
  u32) -> u32 req x < 100 ens result == x fx pure ;` + `fn caller(x: u32) -> u32
  req x < 100 ens result == x fx pure { ext_id(x) }` — under `forge check`: the
  `caller` cert is `level == "L3"` (TODAY: `L0`, verus errors on undefined
  `ext_id`) AND `assurance_scope == ToBoundary { via: "ext_id" }`. `ext_id`'s own
  cert is UNCHANGED: `level == "L1"`, `boundary == true`,
  `boundary_target == "ext::ext_id"`. (Oracle: a `conformance/composition`
  cases entry; the #16 `boundary.cert.json` precedent for `ext_id`'s L1 cert.)
- **AC-2 (transitive caller → L3 + to_boundary):** the #17
  `transitive_boundary_caller` program — `ext_id` (boundary) ← `g` ← `h`, each
  `req x < 100 ens result == x fx pure` — under `forge check`: BOTH `g` and `h`
  certify `level == "L3"` (TODAY: L0) AND `assurance_scope == ToBoundary { via:
  "ext_id" }` (the transitive crossing). The sub-program for `h` weaves in `g`
  (proved, real body) AND `ext_id` (external_body signature).
- **AC-3 (req-violating caller → counterexample, NOT L3):** a fixture with the
  SAME `ext_id` boundary but a caller whose `req` is `true` (does not establish
  `x < 100`): `fn bad(x: u32) -> u32 req true ens result == x fx pure { ext_id(x)
  }` → `bad`'s cert is NOT `L3`; it is the failure path with a `precondition not
  satisfied`-class witness (the grounded harness 2). A caller asserting `ens
  result == x + 1` likewise fails (`postcondition not satisfied`, harness 3).
  This is the anti-cheat AC: composition does not let a caller dodge `f`'s req or
  manufacture an `ens`.
- **AC-4 (corpus unaffected → L3 unchanged):** the existing pure corpus (`sum`,
  `binary_search` — no `#[boundary]`/`#[slag]` reference anywhere in their
  closures, #17 `end_to_end`) certifies `level == "L3"` with an IDENTICAL cert
  before and after #52, and `assurance_scope` END-TO-END. No external_body is
  emitted for any of their sub-programs (they reference only `spec fn`s /
  combinators). The frozen golden `conformance/sum.cert.json` is byte-stable.
  (Oracle: the unchanged existing golden certs.)

## Architecture

The change layers on the SHIPPED §16/§17 machinery; #52 adds only the composition
weaving + the external_body emission shape.

```text
forge check <file>
  │
  ├─ gate_fn: a #[boundary]/#[slag] fn -> BoundaryL1/SlagL1 cert (L1 + flag) [§16/§8, SHIPPED, UNCHANGED]
  │
  ├─ for a caller fn g (ProceedToL3):
  │     item_subprogram(g, spec_items)  ── #52: ALSO weave each in-file
  │        │                                  #[boundary]/#[slag] fn g's body
  │        │                                  (transitively) references
  │        ▼                                  (reuse closure.rs reachability)
  │     thermite_lower::lower(sub)  ── #52: emit each woven boundary/slag fn as a
  │        │                              #[verifier::external_body] signature
  │        │                              (requires/ensures from its Contract, no body)
  │        ▼
  │     run_verus(sub, lowered)  ── g PROVES through f's assumed ensures -> L3
  │        │                          (or COUNTEREXAMPLE if g violates f's req / its own ens)
  │        ▼
  │     Certificate (Level::L3, no reject)
  │
  └─ closure::classify(program)  ── attach g.assurance_scope = ToBoundary { via: f } [§17, SHIPPED]
```

- **`check.rs` `item_subprogram`** is the PRIMARY seam: it already isolates each
  `fn` with the file's `spec fn`s (`§5.3` per-item caching); #52 widens the woven
  set to include the referenced boundary/slag siblings. The transitive
  reachability is exactly the `closure.rs` `CallGraph`/`reach_crossing` walk
  (`pub fn classify in closure.rs`) #17 already ships — the natural reuse.
- **`lower.rs`** is the SECONDARY seam: `lower` iterates `program.items` and calls
  `lower_fn`; #52 routes an `f.boundary.is_some() || f.slag.is_some()` item to a
  new external_body-signature emission (the `requires`/`ensures` reuse the
  existing spec-context lowering; the body is suppressed). This replaces today's
  `LowerError::Unsupported` on a `body: None` boundary fn (`lower_fn_body` in
  `lower.rs`) WHEN the fn appears as a woven dependency of a caller (a top-level
  boundary fn is still never lowered — it certifies L1 in `gate_fn`, never
  reaching `lower`).
- The boundary/slag fn's OWN cert (`Certificate::boundary_l1` /
  `Certificate::slag_l1` in `manifest.rs`) and #17's `assurance_scope`
  attachment are UNCHANGED — #52 changes only the caller's LEVEL (L0→L3) by giving
  verus a resolvable, assumable callee.

## Verification

- **Route (orchestrator, not this doc):** add/extend `[[route]]` entries in
  `tooling/spec-routes.toml` so `forge/src/check.rs` and `thermite-lower/src/lower.rs`
  map to this doc (in ADDITION to their existing `check.md` / `verus-lowering.md`
  routes — a file may carry multiple governing docs), with `reference =
  ["conformance/composition"]`. The spec-discipline hook (R-XLATE-2/R-XLATE-3)
  blocks the builder until both the route and this doc exist (this doc satisfies
  the latter). See OQ-3.
- **Oracle (orchestrator-authored):** a `conformance/composition/cases.json`
  hand-derived fixture file (the `conformance/e2e/cases.json` precedent) carrying
  AC-1..AC-4's programs and their expected per-fn `level` + `assurance_scope` +
  (for `ext_id`) `boundary`/`boundary_target`. The cert-oracle test
  (`forge/tests/`) runs `forge check` over each and asserts the emitted cert's
  oracle subset against this golden file.
- **Golden lowering (R-CHAR-3):** a `tests/golden/lower/composition.verus.rs`
  hand-authored from THIS design — the `caller`+`ext_id` program lowered, showing
  the `#[verifier::external_body] fn ext_id(...) requires x < 100, ensures result
  == x, { unimplemented!() }` signature woven before `caller` — and which MUST
  itself pass the real `verus` with 0 errors (the load-bearing external truth,
  `goal.md` verification model (A); the grounded harness (1) is the existence
  proof). A `thermite-lower` test diffs `lower(composition.th)` against it.
- **Soundness test (AC-3):** a `forge` test asserting the req-violating /
  ens-overclaiming callers emit a NON-L3 cert with a counterexample (grounded
  harnesses 2/3), never a false L3 (R-DEFER-9 anti-cheat).
- **Crate gauntlets (`goal.md` R-DEFER-6):** `cargo test -p forge`, `cargo test
  -p thermite-lower`, `cargo clippy -p <crate> --all-targets -- -D warnings`,
  `cargo fmt --check`, plus the conformance corpus (`forge check` over
  `conformance/` — `sum`/`binary_search` stay L3 + END-TO-END, AC-4; the
  composition fixtures reach L3 + to_boundary).

## Open questions

- **OQ-1 (the honesty boundary — least confident):** external_body is emitted
  ONLY for a fn with `boundary.is_some() || slag.is_some()`. The risk the builder
  + critic MUST pin: a regression that emits external_body for a regular Thermite
  fn (dodging its proof) is a R-DEFER-9 cheat. The mechanical guard: a test
  asserting that for the pure corpus (`sum`/`binary_search`) the lowered string
  contains NO `external_body` substring (every dependency is a `spec fn` /
  combinator, fully defined), and that an `external_body` line in the lowered
  output appears IFF the woven dependency carries `#[boundary]`/`#[slag]`. The
  honesty argument rests entirely on this gate; it is the load-bearing invariant.
- **OQ-2 (slag body — emit external_body or the fiat body?):** a `#[slag]` fn has
  a REAL (fiat-trusted) body, unlike a boundary fn (`body: None`). For
  composition, the caller verifies through the slag fn's CONTRACT (not its body —
  §8 "slag exempts proving"), so the slag fn should ALSO be woven as an
  external_body signature (contract assumed, body NOT lowered/checked), identical
  to a boundary fn. LEANING: treat boundary and slag IDENTICALLY at the
  composition seam (both are external_body signatures) — the §9 TCB unifies them
  as crossings (#17 already treats them identically). The builder should NOT lower
  the slag fn's fiat body into the caller's sub-program (that would re-introduce
  an obligation §8 exempts). Confirm against `conformance/composition`'s
  `slag_caller` fixture.
- **OQ-3 (change site — `item_subprogram` vs `lower.rs`; route shape):** the
  weaving (WHICH fns enter the caller's sub-program) is `item_subprogram`'s job in
  `check.rs`; the emission SHAPE (external_body signature) is `lower.rs`'s job.
  Both files change. LEANING: the primary fix is `item_subprogram` (the
  sub-program is forge's composition concern, reusing `closure.rs` reachability),
  and `lower.rs` gains the external_body emission arm. The route question for the
  orchestrator: does `lower.rs` get a SECOND route to this doc, or is the
  external_body emission folded into `verus-lowering.md` as a new REQ? Either is
  R-XLATE-1-compliant; this doc governs the composition contract regardless. The
  builder must NOT split the two changes across separate commits if a single
  fixture (AC-1) cannot go green without both (R-DEFER-6).
- **OQ-4 (effect-row crossing through the boundary):** `check_effects` (whole-
  program, §4.1) already runs BEFORE the per-item split and validates that the
  caller's `fx` row subsumes the boundary fn's stated row (the boundary fn's `fx`
  is its STATED row, ffi-boundary.md OQ-4 — the foreign body's actual effects are
  trusted-by-fiat). #52 changes nothing here: the external_body signature carries
  no `fx` annotation (verus `fn` is pure by default; the Thermite-level `fx`
  subsumption is the existing compile-time check). Confirm the composition
  fixtures all use `fx pure` so no effect interaction is exercised in v0.1.

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (assumable-signature emission, boundary/slag only) | SHIPPED | `lower_fn in thermite-lower/src/lower.rs` dispatches a `f.boundary.is_some() \|\| f.slag.is_some()` fn to `lower_external_body_fn`, which emits `#[verifier::external_body]` + the SHARED `lower_fn_signature` (unweakened `requires`/`ensures`) + a synthetic `{ unimplemented!() }` body verus never checks. THE HONESTY GATE: external_body iff the syntactic `#[boundary]`/`#[slag]` flag — a regular fn ALWAYS takes the fully-proved-body arm. Consumer: `check::item_subprogram` weaves a boundary/slag dep through this arm. Grounded `verus 0.2026.05.24`: the emitted `#[verifier::external_body] fn ext_id(..) requires x<100, ensures result==x { unimplemented!() }` + caller verifies `success: true, verified: 1, errors: 0`. Verified by `forge`'s `composition_conformance::direct_boundary_caller_verifies_through_the_contract` + `lying_regular_fn_is_caught_never_laundered_to_l3`. |
| REQ-2 (boundary-caller reaches L3 + scope `to_boundary`) | SHIPPED | `check::item_subprogram(item, spec_items, fn_deps)` weaves the transitively-reachable in-file fns (`check::reachable_fn_deps` → `closure::reachable_in_file_fns`, the reused #17 walk) into a caller's §5.3 sub-program — regular fns with their real body, boundary/slag fns via the `lower` external_body arm — so `verus` resolves the callee and the caller proves THROUGH its contract. `direct_boundary_caller`'s `caller` and `transitive_boundary_caller`'s `h` certify `Level::L3` (was `L0`) AND `assurance_scope = ToBoundary { via: ext_id }` (#17, unchanged). Verified by `composition_conformance::{direct_boundary_caller_verifies_through_the_contract, transitive_boundary_caller_weaves_real_and_external_body_deps}`. |
| REQ-3 (boundary/slag fn itself unchanged — L1 + flag) | SHIPPED | #52 left the §16/§8 path UNTOUCHED: `gate_fn in check.rs` still short-circuits a `f.boundary.is_some()` item to `GateOutcome::BoundaryL1` (`Certificate::boundary_l1`, `Level::L1`, `boundary: true`, no verus) before the L3 path; the external_body signature is woven only into a CALLER's sub-program, never `f`'s own cert. Verified: `composition_conformance::direct_boundary_caller_verifies_through_the_contract` asserts `ext_id`'s cert stays `level == "L1"`, `boundary == true`. |
| REQ-4 (soundness — violation is a counterexample, not a false L3) | SHIPPED | The external_body assumes ONLY `f`'s `ensures`; the caller must still establish `f`'s `req`. The `req_violating_caller`'s `bad` (req `true`, not establishing `ext_id`'s `x < 100`) certifies NON-L3 with a failed-obligation witness (`precondition not satisfied`), NOT a false L3. Grounded `verus 0.2026.05.24`: a regular fn with a lying body (`ens result == x + 1` body `x`) FAILS `postcondition not satisfied` (the external_body exemption is boundary/slag-only). Verified by `composition_conformance::{req_violating_caller_is_a_counterexample_not_a_false_l3, lying_regular_fn_is_caught_never_laundered_to_l3}`. |
