# End-to-End vs To-the-Boundary Certification
<!--
tier: 3-component
status: draft
governs: forge/src/closure.rs
thesis-refs:
  - thermite-design.md §9
  - thermite-design.md §6
  - thermite-design.md §8
-->

## Summary

`thermite-design.md` §9 promises: "Pure-Thermite transitive closures can be
certified **end-to-end**; the manifest distinguishes 'verified to the boundary'
from 'verified, period.'" This component computes, for each `fn` in a file, its
**transitive call closure** within the file and classifies the function's
*assurance scope*: **END-TO-END** ("verified, period") when nothing in the
closure is a `#[boundary]` (foreign, unproven body, #16) or `#[slag]` (trusted-
by-fiat body, #6) function, and **TO-THE-BOUNDARY** ("verified to the boundary")
when the closure transitively reaches one — meaning the whole-program guarantee
depends on a foreign/unproven body at the crossing even though the function's own
contract is verified. It records the result as an additive per-`fn` certificate
field and aggregates it into the project-level `AssuranceManifest` (#10): the
project is END-TO-END iff every function is, otherwise TO-THE-BOUNDARY (listing
the crossings).

This is the §9 manifest distinction. It is **greenfield**: no call-graph walk,
no transitive-closure analysis, and no assurance-scope field exist anywhere in
`forge` today (verified below — the only existing `boundary`/`slag` machinery is
the per-fn `Certificate.boundary` / `slag` flags from #16/#6, which are this
component's *inputs*). All REQs are **NOT-STARTED**, tracked by **crosslink issue
#17** (v0.4, milestone #4).

## Decided scope

Issue #17 = the **e2e-vs-boundary classification** (the transitive-call-closure
analysis + the END-TO-END / TO-THE-BOUNDARY rule) + the **per-fn cert field** +
the **project-level claim** in the assurance manifest. Explicitly OUT (these are
inputs/boundaries, never deferred-as-status):

- The boundary-fn declaration form, its L1-enforced contract, and the per-cert
  `boundary: bool` / `boundary_target: Option<String>` flags are **#16**
  (`.design/boundary/ffi-boundary.md`, SHIPPED). This component *reads* those
  flags; it does not add them.
- The `#[slag]` body-unproven flag (`Certificate.slag: bool`) is **#6**
  (`.design/forge/slag.md`, SHIPPED). This component *reads* it.
- The project-level `AssuranceManifest` / `ProjectAssurance` aggregate and the
  `Level` min-over-functions headline are **#10** (`.design/forge/degrade-ladder.md`,
  SHIPPED). #17 EXTENDS that aggregate with the project-level assurance-scope
  claim; it does not author the manifest.
- The full audit-manifest v1 / TCB enumeration (slag ∪ boundary ∪ toolchain) is
  **#15**. The §9 "TCB = slag ∪ boundary ∪ toolchain" sentence motivates *why*
  the closure treats slag and boundary identically as crossings, but the audit
  surface itself is #15's job.

## The classification (the core rule)

For each `Item::Fn` `f` in a file, compute the **transitive call closure**
`closure(f)`: the set of in-file functions `f` calls, transitively. Then:

- **END-TO-END** ("verified, period"): `f` AND every function in `closure(f)` is
  pure-Thermite — there is **no** `#[boundary]` fn and **no** `#[slag]` fn
  anywhere in the closure. Every link is a proved (or `spec fn` / combinator)
  body. The whole-program guarantee rests only on the toolchain.
- **TO-THE-BOUNDARY** ("verified to the boundary"): `closure(f)` transitively
  reaches a `#[boundary]` fn (foreign, unproven body) or a `#[slag]` fn (trusted-
  by-fiat body). `f`'s own contract is verified, but the closure crosses into a
  foreign/unproven body, so the end-to-end guarantee does not hold (`goal.md`
  R-DEFER-9: the manifest must HONESTLY mark a guarantee that depends on an
  unproven foreign body — never claim end-to-end when a boundary is reached).

The **composition rule** (§9) is exactly what makes this a *closure* and not a
whole-program reverification: "if `g` calls `f` only through `f`'s contract,
`g`'s certificate is valid independent of `f`'s body." The classification never
re-proves anything — it is a pure structural property of the call graph layered
on top of the per-fn certificates `forge check` already produces (the
verification driver, `.design/forge/check.md`).

### The call graph (what is PURE, what is a CROSSING)

- **Nodes** are the file's `Item::Fn` and `Item::SpecFn` (`thermite-syntax`
  `enum Item in ast.rs`).
- **Edges** come from walking each `fn` body's expressions for call forms:
  `Expr::Call { callee, .. }` and `Expr::MethodCall { name, .. }` (`enum Expr in
  ast.rs`). A boundary fn has `body: None` (`struct FnItem in ast.rs`) and so has
  **no out-edges** — it is a leaf crossing.
- **Callee resolution** resolves a call to an in-file node by name (the leading
  `Path` segment for `Expr::Call`, the method `name` for `Expr::MethodCall`):
  - resolves to an in-file `Item::SpecFn` → **PURE** (a `spec fn` is total,
    terminating, body-Thermite-verified; §4.2 — it is never a crossing, even when
    transitively self-recursive like `spec_sum`);
  - resolves to a registry combinator (`forall_in`, `sorted`, …; the
    `thermite-spec` combinator set, §4.2) → **PURE** (frozen-trigger library, a
    proved definition, never a crossing);
  - resolves to an in-file `Item::Fn` `g` → recurse: `g` is a CROSSING iff `g` is
    `#[boundary]`/`#[slag]`, else inherit `g`'s closure classification;
  - resolves to nothing in-file and is not a combinator (an out-of-file callee) →
    see OQ-1 (the cross-file-resolution open question).
- **Bounded, cycle-safe walk.** A function is in its own closure (self/mutual
  recursion: `spec_sum` calls `spec_sum`). The walk maintains a visited set keyed
  by function name and never re-enters a visited node, so a cycle terminates
  after each node is touched once — the walk is O(nodes + edges), DETERMINISTIC
  (R-CODE-5: a pure function of the parsed `Program`, no wall-clock / unordered
  iteration in the verdict).

## The record (per-fn field + project claim)

- **Per-fn cert field** (additive, the #16/#10 additive-field precedent in
  `manifest.rs`): an `assurance_scope` field on `Certificate`. The shape under
  consideration (OQ-2): an enum `AssuranceScope { EndToEnd, ToBoundary { via:
  String } }` where `via` names the boundary/slag fn the closure reached (the
  first crossing, deterministically). It MUST be additive (`#[serde(default,
  skip_serializing_if)]`) so the frozen golden `conformance/sum.cert.json` (which
  omits it) still deserializes — the R-SPEC-2 rule every prior additive field
  (`slag_meta`, `cached`, `solver_profile`, `boundary`) followed.
- **Project-level claim** (extend `AssuranceManifest::aggregate(&[Certificate])`,
  `manifest.rs`): a project-level assurance scope alongside the existing
  `ProjectAssurance` headline. The project is **END-TO-END iff every function is
  END-TO-END**; otherwise it is **TO-THE-BOUNDARY**, listing the crossings (the
  boundary/slag fns reached). This is a *render-time aggregate* of the per-fn
  scopes (the `aggregate` OQ-4 (b) reading the existing manifest uses), computed
  from the cert collection — NOT a separately-materialized schema object.

### How scope coexists with the verification level

The assurance scope is **orthogonal** to the assurance `Level` (L0–L3) and to the
`boundary`/`slag` flags, and must not be conflated with them:

- `Level` answers "how strongly is THIS fn's own contract established?" (L3 SMT
  proof, L2 bounded, L1 runtime). A TO-THE-BOUNDARY fn can be `Level::L3`: a pure-
  Thermite `g` that calls a `#[boundary]` `f` is itself SMT-proved (L3) *against
  f's contract* — its own body is fully verified — yet its whole-program closure
  reaches a foreign body, so its scope is TO-THE-BOUNDARY (`via: f`). The `via`
  fn `f` is itself `Level::L1` + `boundary: true`. Scope and level are reported
  together, never merged (OQ-3).
- A `#[boundary]`/`#[slag]` fn is trivially TO-THE-BOUNDARY (it IS the crossing;
  `via` is itself).
- A pure-Thermite leaf or a fn whose entire closure is pure is END-TO-END.

## Architecture

The analysis is a new pure module, expected at `forge/src/closure.rs` (the route
the orchestrator must add — see Verification). It depends ONLY on the parsed
`Program` (`thermite-syntax`) plus the per-fn certificate collection
(`manifest.rs`): it owns NO prover invocation and changes NO verdict — it LAYERS
a structural classification on top of the verdicts `check::check_file` already
produced (the §9 composition rule made operational).

Data flow (the §9 distinction, end to end):

```text
parsed Program (thermite-syntax: Item::{Fn,SpecFn}, Expr::{Call,MethodCall})
      │
      ▼
build call graph  (resolve callees: spec fn / combinator = PURE; #[boundary]/#[slag] fn = CROSSING)
      │
      ▼
transitive closure per fn  (cycle-safe visited set; bounded walk)
      │
      ▼
AssuranceScope per fn  (EndToEnd | ToBoundary { via })  ── additive Certificate field
      │
      ▼
AssuranceManifest aggregate  (project END-TO-END iff every fn is; else TO-THE-BOUNDARY + crossings)
```

The inputs already ship: `Certificate.boundary` / `Certificate.boundary_target`
(set by `Certificate::boundary_l1`, consumed in `check::gate_fn`) and
`Certificate.slag` (set by `Certificate::slag_l1`). The `via` crossing is the
boundary/slag fn the closure reached; `boundary_target` (the foreign `crate::path`)
is the audit-surface detail #15 reads, but the classification itself keys on the
in-file `#[boundary]`/`#[slag]` *node*, not its foreign target.

## Requirements

- **REQ-1 (transitive call closure, cycle-safe + bounded):** for each
  `Item::Fn`, compute the set of in-file functions it calls transitively by
  walking `Expr::Call`/`Expr::MethodCall` and resolving callees to in-file
  `Item::Fn`/`Item::SpecFn`. A `spec fn` and a registry combinator are PURE
  (never a crossing). The walk is cycle-safe (a visited set; recursion does not
  loop) and bounded (each node touched once). Derived from `thermite-design.md`
  §9 (the composition rule) + §4.2 (`spec fn`s are total/terminating).
- **REQ-2 (END-TO-END vs TO-THE-BOUNDARY rule):** classify a fn END-TO-END iff
  its closure contains NO `#[boundary]` and NO `#[slag]` fn; otherwise
  TO-THE-BOUNDARY, recording the reached crossing (`via`). Derived from §9
  ("verified to the boundary" vs "verified, period") + `goal.md` R-DEFER-9 (never
  claim end-to-end when a boundary is reached).
- **REQ-3 (per-fn additive cert field):** record the per-fn assurance scope as an
  additive `Certificate` field (R-SPEC-2: `#[serde(default, skip_serializing_if)]`
  so the frozen golden cert still deserializes), following the #16/#10 additive-
  field precedent. Derived from §9 (the manifest distinguishes the two) + §6 (the
  certificate is the trust statement).
- **REQ-4 (project-level claim — END-TO-END iff all fns are):** extend the
  `AssuranceManifest` aggregate with the project assurance scope: END-TO-END iff
  every fn is END-TO-END, else TO-THE-BOUNDARY listing the crossings. Derived from
  §9 (the manifest distinction) + §5.2 (the project-level aggregate is over
  functions).
- **REQ-5 (scope ⊥ level):** the assurance scope is recorded ALONGSIDE the
  `Level` and the `boundary`/`slag` flags, never merged: a TO-THE-BOUNDARY fn may
  be `Level::L3` (its own contract proved) while its closure crosses a foreign
  body. Derived from §9 (the composition rule keeps `g`'s certificate valid
  independent of `f`'s body) + §6 (the ladder level is the per-fn body-proof
  strength).
- **REQ-6 (determinism):** the classification is a pure, deterministic function
  of the parsed `Program` + cert collection — no wall-clock, no unordered
  iteration affecting the verdict; the `via` crossing is chosen deterministically
  (e.g. the first reached in a fixed traversal order). Derived from §5.3 +
  `goal.md` R-CODE-5.

## Acceptance criteria

ACs tie to a `conformance/e2e/` oracle (a hand-derived JSON cases file, the
`conformance/boundary/cases.json` precedent — authored by the orchestrator, NOT
this doc; R-CHAR-3, expected values hand-derived, never copied from forge output).

- **AC-1 (pure corpus → END-TO-END):** `sum` (`conformance/sum.th`) — `sum` calls
  only `spec_sum` (a `spec fn`) — and `binary_search`
  (`conformance/binary_search.th`) — which calls only combinators (`sorted`,
  `forall_in`, `forall_below`, `forall_from`) — classify **END-TO-END**. (These
  are the existing golden pure-Thermite programs; their closures contain no
  crossing.)
- **AC-2 (direct boundary caller → TO-THE-BOUNDARY via that boundary):** a fixture
  with a `#[boundary("ext::foreign_id")]` fn `foreign_id` (the
  `conformance/boundary/cases.json` `foreign_id` program) and an in-language
  caller `g` whose body calls `foreign_id` → `g` classifies **TO-THE-BOUNDARY**
  with `via: "foreign_id"`; `foreign_id` itself is TO-THE-BOUNDARY (the crossing).
- **AC-3 (transitive boundary chain → TO-THE-BOUNDARY):** a fixture `h → g →
  foreign_id` (a pure-Thermite `h` calling a pure-Thermite `g` calling the
  boundary `foreign_id`) → `h` classifies **TO-THE-BOUNDARY** (the closure reaches
  `foreign_id` transitively through `g`); so does `g`.
- **AC-4 (slag in closure → TO-THE-BOUNDARY):** a fixture with a `#[slag(...)]`
  fn `s` and a caller `g` calling `s` → `g` classifies **TO-THE-BOUNDARY** via the
  slag crossing (a `#[slag]` body is unproven, identical to a boundary for the
  whole-program guarantee; §9 "TCB = slag ∪ boundary ∪ toolchain").
- **AC-5 (project END-TO-END iff all fns are):** a project of only pure-Thermite
  fns → project **END-TO-END**; a project with ANY TO-THE-BOUNDARY fn → project
  **TO-THE-BOUNDARY**, listing the crossing(s).
- **AC-6 (cycle-safe):** a fixture with recursion (a fn calling itself, or mutual
  recursion `a → b → a`) classifies WITHOUT non-termination, and a recursive pure-
  Thermite fn is END-TO-END (recursion is not a crossing).
- **AC-7 (determinism):** classifying the same program twice yields byte-identical
  scopes and an identical `via` choice (R-CODE-5).

## Verification

- **Route to add (orchestrator, not this doc):** add a `[[route]]` to
  `tooling/spec-routes.toml` mapping `forge/src/closure.rs` → this doc, with
  `reference = ["conformance/e2e"]` and `conformance_ops = ["sum", "binary_search",
  "foreign_id", "transitive_chain", "slag_in_closure"]`. The spec-discipline hook
  (R-XLATE-2/R-XLATE-3) blocks the builder's edit until both the route and this
  doc exist.
- **Oracle (orchestrator-authored):** a `conformance/e2e/cases.json` hand-derived
  fixture file (the `conformance/boundary/cases.json` precedent) carrying the
  fixtures AC-1..AC-6 reference and their expected per-fn scopes + the project
  claim. The cert-oracle test (`forge/tests/`) asserts the emitted per-fn
  `assurance_scope` + the aggregate project scope against this golden file.
- **Crate gauntlet (the kernel discipline):** `cargo test -p forge`, `cargo clippy
  -p forge --all-targets -- -D warnings`, `cargo fmt --check`, plus the
  conformance corpus (`forge check` over `conformance/` programs — the
  existing pure programs must stay END-TO-END, the boundary/slag fixtures
  TO-THE-BOUNDARY). The corpus golden `sum.cert.json` must still deserialize after
  the additive `assurance_scope` field is added (R-SPEC-2).

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (transitive call closure, cycle-safe + bounded) | NOT-STARTED | crosslink issue #17 (open, v0.4 milestone #4). No call-graph walk exists in `forge`: `forge/src/closure.rs` is absent; no module walks `Expr::Call`/`Expr::MethodCall` to build an in-file call graph. The AST source nodes exist (`enum Expr { Call, MethodCall, .. } in ast.rs`, `enum Item { Fn, SpecFn } in ast.rs`) but nothing consumes them for closure analysis. |
| REQ-2 (END-TO-END vs TO-THE-BOUNDARY rule) | NOT-STARTED | crosslink issue #17. No assurance-scope classification exists. The crossing *inputs* ship (`Certificate.boundary` set by `Certificate::boundary_l1`; `Certificate.slag` set by `Certificate::slag_l1`, both in `manifest.rs`), but no code reads them to classify a closure as verified-to-the-boundary vs verified-period. |
| REQ-3 (per-fn additive cert field) | NOT-STARTED | crosslink issue #17. No `assurance_scope` (or equivalent) field on `struct Certificate in manifest.rs`. The additive-field pattern is established (the #16 `boundary` / #10 `lowered_assurance` fields use `#[serde(default)]`), but the scope field does not exist. |
| REQ-4 (project-level claim — END-TO-END iff all fns are) | NOT-STARTED | crosslink issue #17. `AssuranceManifest::aggregate in manifest.rs` computes the `ProjectAssurance` min-over-levels headline (#10) but has NO project assurance-scope: it never inspects `boundary`/`slag` across the cert collection to decide END-TO-END vs TO-THE-BOUNDARY, and `struct AssuranceManifest` carries no scope field. |
| REQ-5 (scope ⊥ level) | NOT-STARTED | crosslink issue #17. Without REQ-2/REQ-3 there is no scope to record alongside `Level`; the orthogonality (an L3 fn that is TO-THE-BOUNDARY) is unrepresented. |
| REQ-6 (determinism) | NOT-STARTED | crosslink issue #17. The analysis does not exist, so its determinism is unverified. (The aggregate it extends, `AssuranceManifest::aggregate`, is already a deterministic pure function of the cert collection per #10 REQ-7 — the new scope logic must preserve that.) |
