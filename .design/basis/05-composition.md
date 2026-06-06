# Compositional Reasoning — verification is LOCAL not global (Basis Stage 5, the CAPSTONE)
<!--
tier: 3-component
status: draft
governs: thermite-lower/src/lower.rs
governs: forge/src/check.rs
governs: forge/src/audit.rs
governs: forge/src/manifest.rs
thesis-refs:
  - thermite-design.md §9
  - thermite-design.md §1
  - thermite-design.md §6
  - thermite-design.md §5.2
  - thermite-design.md §5.3
-->

## Summary

Stage 5 is the **capstone** of the universal-verified-basis buildout (crosslink
epic **#62**): the **composition law** that makes verification LOCAL, not global.
A program assembled from verified primitives (Stages 1–4) is verified by the
ASSEMBLY — you never re-prove the world. This is the mechanism by which a FINITE
primitive basis reaches the INFINITE space of programs: prove each primitive
ONCE, and each composition step discharges LOCALLY against contracts alone.

The thesis pins this as the scaling property (§9): "if `g` calls `f` only through
`f`'s contract, then `g`'s certificate is valid independent of `f`'s body. Trust
is invariant under composition instead of decaying multiplicatively." Much of the
machinery already SHIPS: contract composition (#52, `boundary-composition.md`),
assurance aggregation (#15, `audit-manifest.md`), and the verus-verified core
(#60). This doc DOCUMENTS those shipped mechanisms as the four composition laws,
states the LOCALITY theorem and the whole-program honest-assurance ceiling, and
PINS the EXTENSIONS the basis needs so Stages 1–4 compose cleanly. Every
extension REQ is **NOT-STARTED** under epic **#62** (no separate blocker — #62
owns this stage); the laws backed by shipped #52/#15/#60 mechanisms are
**SHIPPED** with cited evidence.

## The four composition laws (the load-bearing theorems)

Each is the mechanism by which "verified parts ⟹ verified whole." Three already
ship (the v0.1 kernel proved them on the scalar/boundary corpus); the basis
EXTENDS each to the Stage 1–4 vocabulary.

1. **CONTRACT composition (sequential, `g∘f`).** If `f` ensures `P` and `g`
   requires `P`, then `g∘f` discharges `g`'s precondition from `f`'s
   postcondition — and the caller's proof uses `f`'s CONTRACT, never `f`'s body.
   SHIPPED (#52): `lower_external_body_fn in thermite-lower/src/lower.rs` emits a
   boundary/slag callee as a `#[verifier::external_body]` assumable signature
   (req→requires, ens→ensures, no checked body); `item_subprogram in
   forge/src/check.rs` weaves the transitively-reachable callees so verus
   resolves the call and the caller proves THROUGH the contract (REQ-1).

2. **RECURSION-SCHEME composition (fusion).** A proven scheme verifies all its
   instances: `fold ∘ map = fold` (fusion), and a `fold`/`unfold` proved once
   over a `Box`-recursive ADT (Stage 1 keystone, `01-adts.md` REQ-10) carries its
   `decreases`-termination and its element-property to every instantiation — you
   do not re-prove termination per call. EXTENSION (NOT-STARTED): whether
   `item_subprogram`'s weaving and the cache (§5.3) correctly key a
   SCHEME-INSTANTIATED proof so a fused pipeline `fold(map(...))` discharges
   against the scheme's contract without re-lowering the fused body (REQ-5).

3. **ASSURANCE aggregation (project level = min over parts).** The whole is no
   stronger than its weakest part. SHIPPED (#15/#60):
   `AssuranceManifest::aggregate in forge/src/manifest.rs` computes the
   `ProjectAssurance` headline as the MIN level over functions (else `Failed`),
   the `ProjectScope` as END-TO-END iff every part is (else TO-THE-BOUNDARY
   listing the crossings), and `Tcb::from_certificates in forge/src/audit.rs`
   enumerates the TCB as the UNION of the parts' boundaries/slag ∪ the toolchain.
   #60 verus-verified the bit-level min/subset core (`thermite-verified`,
   `8 verified, 0 errors`) so the no-over-claim is itself machine-checked
   (REQ-2/REQ-3).

4. **INVARIANT composition (conjunction).** A verified collection (Stage 4) of
   verified ADTs (Stage 1) carries the CONJOINED invariant: a `Vec<Account>`
   where each `Account` has `well_formed()` (`01-adts.md` REQ-8) carries
   "every element well-formed" without re-proving each element. EXTENSION
   (NOT-STARTED): the lowering/validator support for THREADING a nested
   structure's element-invariant as a conjoined `spec fn` over the container so
   verus discharges the conjunction structurally (REQ-6) — the data-half analogue
   of contract composition.

## The locality theorem (the scaling claim)

> **Locality.** Verification cost is PER-PRIMITIVE + PER-COMPOSITION-STEP, never
> global. You prove each primitive once (its contract, against its body, at L3),
> and each composition step discharges LOCALLY: a caller's proof references only
> the callees' CONTRACTS (their `requires`/`ensures`), never their bodies. No
> assembly re-proves the transitive closure.

This is precisely §5.3's content-addressed per-item caching ("an edit to `f`
cannot invalidate `g`'s certificate unless `g`'s contract references `f`'s
contract") and §2.5 Locality ("an edit's blast radius is its block"). It is WHY a
finite basis reaches infinite programs: the primitive set (Stages 1–4) is finite
and each is proved once; the space of compositions is infinite but each step is a
local, checkable discharge. Trust is INVARIANT under composition (§9) rather than
decaying multiplicatively — a 1000-function program is not 1000× less trustworthy
than a 1-function one; it is exactly as trustworthy as its weakest part plus the
enumerated TCB.

**Precedent (the verified-systems lineage).** This is the seL4 / CompCert pattern
made the default: verified components + COMPOSITIONAL reasoning (assume the
callee's spec, discharge the caller locally, aggregate honestly) is how those
projects scaled machine-checked proof past the size where a monolithic proof is
tractable. Thermite makes that the floor (§2.1) rather than a heroic one-off:
every function carries a contract, so every call site is a composition boundary
with a local discharge.

## The whole-program honest-assurance statement (the verify-anything ceiling)

A REAL program in the basis is a **verified pure core** (Stages 1/2/4 — ADTs,
recursion schemes, collections, all L3 end-to-end) ORCHESTRATING **verified
effect-primitives** (Stage 3 — the world-interaction surface: `read`/`write`/
`net`/`alloc`/`time`/`rand`, each a contracted, sandboxed boundary). The audit
manifest (#15) gives the HONEST whole-program trust statement:

> **everything proven except the enumerated, contracted, sandboxed effect TCB.**

The §9 TCB is *exactly* (slag blocks ∪ boundary contracts ∪ the toolchain
itself), enumerated by `Tcb::from_certificates`. The residual trust is therefore
EXACTLY the irreducible world-interaction — Stage 3's effect primitives — and it
is MINIMIZED (only genuine crossings) and ENUMERATED (grep-complete, §8). This is
the theoretical-maximum "verify anything": the only thing you trust by fiat is
the part that interacts with a world the prover cannot model, and that part is
named, contracted, and L1-enforced at the crossing. A pure-Thermite closure with
NO Stage-3 primitive certifies END-TO-END ("verified, period"); the moment it
touches the world, the manifest says "verified to the boundary" and lists the
exact crossing. The honesty is mechanical (R-DEFER-9): the manifest never claims
a stronger state than the per-part certs support (#60 verus-verified the min).

## Requirements

### Laws backed by shipped mechanisms (SHIPPED)

- **REQ-1 (CONTRACT composition — `g∘f` discharges through the contract):** a
  caller `g` whose body calls a callee `f` proves AGAINST `f`'s contract
  (`requires`/`ensures`), never `f`'s body. For a pure callee this is verus's
  native modular reasoning (the woven real-body sub-program); for a
  boundary/slag callee it is the `#[verifier::external_body]` assumable signature.
  The caller still discharges `f`'s `req` at the call site and proves its own
  `ens` — composition is SOUND, not a free pass. Derived from §9 (the composition
  rule), §1 (trust relocation: code→contract), and `boundary-composition.md`
  REQ-1/REQ-2/REQ-4. SHIPPED via #52.

- **REQ-2 (ASSURANCE aggregation — project = min over parts, scope = end_to_end
  iff all parts):** the project-level assurance is the MIN level over functions
  (else `Failed`); the project scope is END-TO-END iff every part is, else
  TO-THE-BOUNDARY listing the crossings. The whole is no stronger than its
  weakest part (`goal.md` R-DEFER-9). Derived from §5.2 ("whole-project assurance
  is the min over functions") + §9 (end-to-end vs to-the-boundary). SHIPPED via
  #15/#60 (`AssuranceManifest::aggregate`, the min/subset core verus-verified).

- **REQ-3 (TCB aggregation — whole-program TCB = ∪ of parts' boundaries/slag ∪
  toolchain):** the whole-program trusted computing base is the UNION over parts
  of each part's slag blocks and boundary contracts, plus the toolchain identity
  — enumerated completely (nothing fiat-trusted omitted, R-DEFER-9). Derived from
  §9 ("the TCB is exactly (slag blocks ∪ boundary contracts ∪ the toolchain
  itself)") + §8 (`grep slag` is the complete inventory). SHIPPED via #15
  (`Tcb::from_certificates`).

### Extensions the basis needs (NOT-STARTED — the concrete gaps)

- **REQ-4 (compose ADT/collection invariants across the basis vocabulary —
  contract composition reaches Stage 1–4 values):** the #52 weaving and the
  caller's proof must compose contracts whose `requires`/`ensures` mention
  Stage-1 ADT predicates (`x.well_formed()`, `r is Variant`, `01-adts.md`
  REQ-6/REQ-8) and Stage-4 collection invariants. The shipped #52 mechanism is
  proven on SCALAR/boundary contracts; whether `item_subprogram` correctly weaves
  the `spec fn` data-invariant definitions (the `well_formed` predicates) into a
  caller's sub-program so an ADT-valued contract composes is unverified
  end-to-end. The gap: the woven-set computation (`reachable_fn_deps in
  check.rs`) must also pull in the `spec fn`/`struct`/`enum` definitions an
  ADT-valued contract transitively references, not just the called `fn`s. Derived
  from §9 (the composition rule applies to ADT-valued contracts unchanged,
  `01-adts.md` Stage-5 hook) + §4.2 (composition through named `spec fn`s).

- **REQ-5 (compose recursion-scheme contracts — a proven scheme verifies its
  instances / fusion):** a recursion scheme (Stage 2 — `fold`/`map`/`unfold`)
  proved once over a `Box`-recursive ADT must compose so that an INSTANTIATION
  (`fold(map(f, l))`, fusion `fold∘map = fold`) discharges against the scheme's
  contract WITHOUT re-lowering or re-proving the fused body. The open gap: does
  the aggregate/cache (§5.3 content-addressing) key a scheme-instantiated proof
  correctly, and does `item_subprogram` weave the scheme's `decreases`-bearing
  `spec fn` so verus reuses the termination proof rather than re-deriving it per
  call? Derived from §9 + §5.3 (per-item caching) + `02-recursion-schemes.md`
  (the scheme-fusion law — Stage 2, in parallel; reference by stage).

- **REQ-6 (compose nested invariants — invariant CONJUNCTION for nested
  ADTs/collections):** a verified collection (Stage 4) of verified ADTs (Stage 1)
  carries the conjoined element-invariant ("every element well-formed") as a
  composed property, threaded as a `spec fn` over the container so verus
  discharges the conjunction structurally — without re-proving each element. The
  gap: the lowering/validator support for the container-element invariant
  conjunction (the data-half analogue of REQ-1's contract composition). Derived
  from §9 + `04-collections.md` (Stage 4 collection invariants — in parallel) +
  `01-adts.md` REQ-8 (the per-ADT `well_formed`).

- **REQ-7 (aggregate assurance across a DEEP call graph through the effect
  stdlib — the whole-program honest-assurance ceiling):** the manifest must
  aggregate assurance across a deep, multi-stage call graph (a verified pure core
  orchestrating Stage-3 effect primitives), producing the honest whole-program
  statement: END-TO-END for the pure closure, TO-THE-BOUNDARY listing exactly the
  Stage-3 crossings, with the TCB = the union of those primitives' contracts. The
  shipped #15 aggregate is proven on a SHALLOW corpus (direct boundary callers);
  whether `ProjectScope` correctly accumulates crossings through a DEEP transitive
  closure that passes through the effect stdlib is the gap (the `closure.rs`
  reachability must reach Stage-3 primitives transitively, not just direct
  callees). Derived from §9 (verified-to-the-boundary vs verified-period) + §5.2
  (min over functions) + `03-effect-stdlib.md` (Stage 3 — in parallel).

## Acceptance criteria

ACs tie to a NEW corpus program — a multi-stage pipeline using ADTs (Stage 1) +
a recursion scheme (Stage 2) + an effect primitive (Stage 3) — authored by the
ORCHESTRATOR from this doc (R-CHAR-3; expected values hand-derived from §9 +
verus semantics, never copied from forge output). Call it
`conformance/composition/pipeline.th` with golden lowering
`tests/golden/lower/composition_pipeline.verus.rs` and cert/manifest goldens
under `conformance/composition/`.

- **AC-1 (the pipeline certifies TO-THE-BOUNDARY, discharging locally):** a
  pipeline `h(g(f(x)))` where `f` is a Stage-3 effect primitive (boundary/slag,
  `external_body` signature) and `g`/`h` are verified pure steps over Stage-1
  ADTs — `forge check` certifies `g`/`h`/the pipeline at `Level::L3` proving
  ONLY against each step's contract (no re-proof across the boundary), with the
  pipeline's `assurance_scope = ToBoundary { via: f }`. GROUNDED below: the exact
  chain verifies `4 verified, 0 errors` (default mode, with `f` as external_body)
  and `5 verified, 0 errors` (`--no-cheating`, with `f` as a proved pure step —
  the composition law itself carries NO cheat). (REQ-1, REQ-4, REQ-5.)

- **AC-2 (the audit manifest aggregates per-part assurance + enumerates the TCB
  honestly):** `forge audit conformance/composition/pipeline.th` emits a manifest
  whose `project_assurance` is the MIN over parts (TO-THE-BOUNDARY, listing the
  `f` crossing) and whose `tcb` enumerates `f`'s boundary/slag contract ∪ the
  toolchain — nothing omitted (R-DEFER-9). A purely-pure variant (no Stage-3
  primitive) certifies END-TO-END with an empty-but-toolchain TCB. (REQ-2, REQ-3,
  REQ-7.) SHIPPED-mechanism-backed (#15/#60); the deep-call-graph variant is the
  REQ-7 extension.

- **AC-3 (composition discharges LOCALLY — no global re-proof; the anti-cheat):**
  the pipeline's proof uses only the callees' contracts: a counterexample
  demonstrates that a pipeline OVER-CLAIMING its `ensures` FAILS (postcondition
  not satisfied), and one that drops an intermediate step's `requires` FAILS
  (precondition not satisfied) — composition does not let the caller dodge a
  callee's `req` or manufacture an `ens`. GROUNDED below (`3 verified, 1 errors`
  over-claim; `2 verified, 1 errors` req-violation). (REQ-1.)

- **AC-4 (invariant conjunction over a nested structure):** a `Vec<Account>` (or
  Stage-1 ADT in a Stage-4 collection) whose element-invariant is the conjoined
  `well_formed()` certifies the container-level property WITHOUT a per-element
  re-proof — the invariant composes. (REQ-6.) NOT-STARTED extension; the
  orchestrator's corpus pins the verified form once Stage 4 lands.

- **AC-5 (the existing corpus is unaffected):** `conformance/sum.th` /
  `binary_search.th` certify `level == "L3"`, project END-TO-END, byte-stable
  goldens — the composition extensions are additive. (All REQs; the capstone must
  not regress the kernel.)

## Architecture

Stage 5 LAYERS on the shipped #52/#15/#60 machinery; the basis extensions widen
the woven set and the aggregate's reach. No new prover invocation — composition
is verus's native modular reasoning plus a pure aggregate.

```text
forge check <pipeline.th>
  │
  ├─ gate_fn: Stage-3 effect primitive f (#[boundary]/#[slag]) -> BoundaryL1/SlagL1 (L1+flag) [#52, SHIPPED]
  │
  ├─ for each pure step g/h/pipeline (Stage 1/2/4):
  │     item_subprogram(item, spec_items, fn_deps)  ── #52 weaves transitively-reachable callees
  │        │                                            (regular -> real body; boundary/slag -> external_body sig)
  │        │   EXTENSION REQ-4: also weave the ADT/collection spec-fn invariant defs
  │        │                    an ADT-valued contract references
  │        │   EXTENSION REQ-5: weave the scheme's decreases-bearing spec fn (reuse termination proof)
  │        ▼
  │     run_verus(sub)  ── each step proves THROUGH the next step's CONTRACT (LOCAL discharge)
  │        ▼
  │     Certificate (Level::L3) + assurance_scope = ToBoundary { via: f }  [#17, SHIPPED]
  │
  └─ AssuranceManifest::aggregate(&certs)  ── project = min over parts; scope = end_to_end iff all [#15/#60, SHIPPED]
        │   EXTENSION REQ-7: accumulate crossings through the DEEP closure (effect stdlib)
        ▼
     audit::AuditManifest::from_certificates  ── tcb = ∪ parts' boundary/slag ∪ toolchain [#15, SHIPPED]
```

- **`thermite-lower/src/lower.rs`** (`lower_external_body_fn`, `lower`) is the
  contract-composition emission seam (#52, SHIPPED). The basis extensions
  (REQ-4/REQ-5/REQ-6) widen WHAT is lowered/woven, not the external_body shape.
- **`forge/src/check.rs`** (`item_subprogram`, `reachable_fn_deps`) is the
  weaving seam (#52, SHIPPED). REQ-4/REQ-5/REQ-7 widen the reachability to ADT
  spec-fn defs, scheme spec-fns, and deep transitive crossings.
- **`forge/src/manifest.rs`** (`AssuranceManifest::aggregate`) is the aggregation
  seam (#15/#60, SHIPPED). REQ-7 extends the scope accumulation across a deep
  graph.
- **`forge/src/audit.rs`** (`Tcb::from_certificates`,
  `AuditManifest::from_certificates`) is the whole-program trust deliverable (#15,
  SHIPPED) — the union TCB and the honest assurance ceiling.

Symbol anchors: `lower_external_body_fn` / `lower` in `lower.rs`;
`item_subprogram` / `reachable_fn_deps` / `gate_fn` in `check.rs`;
`AssuranceManifest::aggregate` in `manifest.rs`; `Tcb::from_certificates` /
`AuditManifest::from_certificates` in `audit.rs`; `reachable_in_file_fns` /
`classify` in `closure.rs`.

### The verified composition chain (GROUNDED — real `verus 0.2026.05.24`)

The capstone mechanism — multi-step composition discharging through contracts
alone — was run against the real binary during authoring (scratch removed, no
stray `*.rlib`). The pipeline:

```text
f : requires x < 100            ensures r == x + 1     (external_body effect primitive)
g : requires 1 <= y <= 100      ensures r == 2 * y     (verified pure step)
h : requires z >= 2             ensures r == z - 1     (verified pure step)
pipeline(x), x < 99 : ensures r == 2*(x+1) - 1
```

`pipeline` discharges each step's `requires` from the PREVIOUS step's `ensures`
alone — `f.req x<100` from `x<99`; `g.req 1<=a<=100` from `a==x+1, x<99`;
`h.req b>=2` from `b==2*a, a>=1` — and proves its final goal `2*(x+1)-1` using
ONLY the contracts, never the body of `f`/`g`/`h`. A second-order consumer
`use_pipeline()` proves `pipeline(0) == 1` through `pipeline`'s contract (locality
one level up).

1. **Composition through contracts (the §9/#52 mechanism), `f` an
   `external_body` effect primitive:**

   ```
   verus compose_ground.rs
   verification results:: 4 verified, 0 errors      (exit 0)
   ```

   `g`, `h`, `pipeline`, `use_pipeline` all prove; `f`'s body is verus-exempt
   (the §9 honesty: external_body iff a declared crossing). The assumed contract
   composes like any other.

2. **The composition law itself is CHEAT-FREE** — the identical pipeline with `f`
   given a real proved body (no exemption) passes the strict gate:

   ```
   verus --no-cheating compose_nocheat.rs
   verification results:: 5 verified, 0 errors      (exit 0)
   ```

   Only the genuine world-interaction primitive needs the external_body exemption
   (`--no-cheating` rejects external_body entirely — confirming it is the ONE
   load-bearing trust admission, not a pervasive cheat). The pure-core composition
   carries none.

3. **SOUNDNESS — composition does not launder (the anti-cheat, AC-3):**
   - a pipeline OVER-CLAIMING its `ensures` (dropping the `-1` that `h` applies)
     FAILS: `postcondition not satisfied`, `3 verified, 1 errors` (exit 1).
   - a caller VIOLATING an intermediate `requires` (widening its own pre to
     `x < 200` so `f.req x<100` is no longer established) FAILS: `precondition
     not satisfied`, `2 verified, 1 errors` (exit 1).

   Both are COUNTEREXAMPLES, not false L3s. The caller must discharge each
   callee's `req` and prove its own `ens` — locality is sound.

This grounds the capstone: a multi-step chain verifies END-TO-END using only each
step's contract (no global re-proof), one step is an opaque effect primitive whose
assumed contract composes like any other, and the discharge is local + sound.

## Verification

- **Routes to add (orchestrator, not this doc):** the composition machinery lives
  in files that already carry governing routes; add `[[route]]` entries to
  `tooling/spec-routes.toml` pointing the composition/aggregation files at THIS
  doc (a file may carry multiple governing docs — the #52 `lower.rs` precedent):

  ```
  [[route]]  crate_pattern = "thermite-lower/src/lower.rs"  design = ".design/basis/05-composition.md"  reference = ["conformance/composition"]
  [[route]]  crate_pattern = "forge/src/check.rs"           design = ".design/basis/05-composition.md"  reference = ["conformance/composition"]
  [[route]]  crate_pattern = "forge/src/manifest.rs"        design = ".design/basis/05-composition.md"  reference = ["conformance/composition"]
  [[route]]  crate_pattern = "forge/src/audit.rs"           design = ".design/basis/05-composition.md"  reference = ["conformance/composition"]
  ```

- **Oracle (orchestrator-authored):** a `conformance/composition/pipeline.th`
  multi-stage program (ADT + recursion scheme + effect primitive) + its expected
  per-fn `level` / `assurance_scope` and the aggregate manifest projection
  (project level = min, scope = to-the-boundary listing the crossing, TCB = the
  primitive's contract ∪ toolchain), hand-derived (R-CHAR-3). The existing
  `conformance/composition/` directory (the #52 cases) is the seam.
- **Golden lowering (R-CHAR-3):** `tests/golden/lower/composition_pipeline.verus.rs`
  hand-authored from this doc — the `f` external_body signature woven before the
  pure steps — which MUST itself pass the real `verus` with 0 errors (the
  GROUNDED chain above is the verified seed).
- **Soundness test (AC-3):** a `forge` test asserting the over-claiming /
  req-violating pipelines emit NON-L3 certs with counterexamples (the grounded
  runs), never a false L3 (R-DEFER-9 anti-cheat).
- **Crate gauntlets (`goal.md` R-DEFER-6):** `cargo test -p forge`, `cargo test
  -p thermite-lower`, `cargo clippy -p <crate> --all-targets -- -D warnings`,
  `cargo fmt --check`, plus the conformance corpus (the pure corpus stays L3 +
  END-TO-END, AC-5).

## Open questions

- **OQ-1 (least confident — REQ-4/REQ-5 real gap vs already-covered by #52):**
  the #52 weaving (`reachable_fn_deps`) already pulls in transitively-reachable
  in-file `fn`s and `spec fn`s. It is UNCLEAR whether it ALSO weaves the `struct`/
  `enum`/`spec fn` definitions an ADT-VALUED contract references but does not
  CALL (e.g. `g`'s `ens` mentions `result.well_formed()` — is `well_formed`'s def
  woven?). If `item_subprogram` already includes all `spec fn` defs (the
  `01-adts.md` REQ-7 named-composition path), REQ-4 may largely reduce to a
  conformance test rather than a code change. The builder/critic must measure this
  against the Stage-1 ADT corpus before treating REQ-4 as a code gap — over-
  claiming it NOT-STARTED when #52 already covers it would mislead. This is the
  REQ I am LEAST confident is a real extension.
- **OQ-2 (REQ-5 fusion — is `fold∘map = fold` a LOWERING concern or purely a
  proof the agent writes?):** fusion may be a property the agent proves with a
  `proof fn` (no toolchain change — composition is just verus reasoning), OR it
  may need cache/weaving support so the scheme's termination proof is reused. The
  scope boundary depends on Stage 2's (`02-recursion-schemes.md`) chosen scheme
  representation, which is in parallel. Pinned for the Stage-2 author + the
  builder; do not implement REQ-5 before Stage 2 lands.
- **OQ-3 (REQ-7 deep-graph scope accumulation — does `closure.rs` already reach
  transitively?):** `closure::reachable_in_file_fns` reuses the `CallGraph` DFS,
  which IS transitive, so `ProjectScope` accumulation across a deep closure may
  already be correct (#52/#17 proved the transitive boundary-caller case `h ←
  g ← ext_id`). REQ-7 may therefore be a CONFORMANCE gap (a deep-pipeline-through-
  the-effect-stdlib fixture) rather than a code gap. The genuine unknown is the
  Stage-3 effect-stdlib boundary (`03-effect-stdlib.md`, in parallel): whether a
  Stage-3 primitive is modeled as a `#[boundary]`/`#[slag]` fn (then #52/#15
  already aggregate it) or a new construct. Confirm against Stage 3.
- **OQ-4 (effect-row composition through the chain):** §4.1 — "a caller's row
  must subsume every callee's row," checked at compile time by `check_effects`
  (verified by #60's `subsumes` core). The composition law's effect-half is the
  effect-row SUBSUMPTION (a separate shipped mechanism), not the contract
  discharge. The grounded chain uses `fx pure` throughout; the basis's effect
  composition (a pipeline whose row is the union of its Stage-3 primitives' rows)
  is the §4.1 subsumption check, already SHIPPED-and-verus-verified (#60) — this
  doc's REQ-7 aggregates the ASSURANCE, `check_effects` composes the EFFECTS. The
  two are orthogonal; do not conflate.

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (CONTRACT composition — `g∘f` through the contract) | SHIPPED | `lower_external_body_fn in thermite-lower/src/lower.rs` emits a boundary/slag callee as a `#[verifier::external_body]` assumable signature (req→requires, ens→ensures, no checked body); `item_subprogram in forge/src/check.rs` weaves the transitively-reachable callees (via `reachable_fn_deps` → `closure::reachable_in_file_fns`) so verus resolves the call and the caller proves THROUGH the contract. Non-test consumer: `check::check_file_with_options` drives `item_subprogram` per fn. Soundness: a caller must discharge `f`'s `req` + prove its own `ens` (#52). GROUNDED `verus 0.2026.05.24`: the multi-step chain `h(g(f(x)))` verifies `4 verified, 0 errors` (default, `f` external_body) / `5 verified, 0 errors` (`--no-cheating`, `f` proved); over-claim → `3 verified, 1 errors`, req-violation → `2 verified, 1 errors`. Verified by `composition_conformance::direct_boundary_caller_verifies_through_the_contract` (#52). |
| REQ-2 (ASSURANCE aggregation — project = min over parts; scope end_to_end iff all) | SHIPPED | `AssuranceManifest::aggregate in forge/src/manifest.rs` computes `ProjectAssurance` = MIN level over functions (else `Failed`) + `ProjectScope` = END-TO-END iff every part is, else TO-THE-BOUNDARY listing crossings. The min/subset core is verus-verified (#60: `thermite-verified`, `verus --no-cheating` `8 verified, 0 errors`; a broken impl → `7 verified, 1 errors`, non-vacuous). Non-test consumer: `audit::AuditManifest::from_certificates` (`forge/src/audit.rs`) embeds it; `cli::run_audit` emits it. Verified by `audit_conformance.rs::corpus_empty_tcb` (L3/end-to-end headline) + #60 `tests/verus_verify.rs`. |
| REQ-3 (TCB aggregation — whole = ∪ parts' boundary/slag ∪ toolchain) | SHIPPED | `Tcb::from_certificates in forge/src/audit.rs` enumerates every `cert.slag` → `SlagBlock` (reason/owner/review) ∪ every `cert.boundary` → `BoundaryContract` (target + req/ens/fx) ∪ `Toolchain` (always present) — nothing fiat-trusted omitted (R-DEFER-9). Non-test consumer: `AuditManifest::from_certificates` → `cli::run_audit`. Verified by `audit_conformance.rs::slag_boundary_tcb` (both slag + boundary enumerated) + `corpus_empty_tcb` (empty-but-toolchain pure state) (#15). |
| REQ-4 (compose ADT/collection invariants — contract composition reaches Stage 1–4) | NOT-STARTED | epic **#62** Stage 5. #52 composes SCALAR/boundary contracts; whether `reachable_fn_deps in check.rs` weaves the `struct`/`enum`/`spec fn` invariant defs an ADT-valued contract references (e.g. `result.well_formed()`) is unverified end-to-end and depends on Stage 1 (`01-adts.md`, NOT-STARTED). May partially reduce to a conformance test if #52 already weaves all `spec fn` defs (OQ-1, least confident). |
| REQ-5 (compose recursion-scheme contracts — fusion / proven scheme verifies instances) | NOT-STARTED | epic **#62** Stage 5. No scheme representation exists yet — Stage 2 (`02-recursion-schemes.md`) is in parallel. Whether the §5.3 cache + `item_subprogram` reuse a scheme's `decreases`-termination proof across instantiations (`fold∘map = fold`) is unresolved (OQ-2); blocked on Stage 2 landing. |
| REQ-6 (compose nested invariants — conjunction for nested ADTs/collections) | NOT-STARTED | epic **#62** Stage 5. No lowering/validator support for threading a container-element invariant (a `Vec<Account>` carrying conjoined `well_formed()`) as a composed `spec fn`. Depends on Stage 1 (`01-adts.md` REQ-8) + Stage 4 (`04-collections.md`), both NOT-STARTED. |
| REQ-7 (aggregate assurance across a DEEP call graph through the effect stdlib) | NOT-STARTED | epic **#62** Stage 5. #15's aggregate is proven on a SHALLOW corpus (direct boundary callers); whether `ProjectScope` accumulates crossings through a deep transitive closure passing through Stage-3 effect primitives is unverified — depends on Stage 3 (`03-effect-stdlib.md`, in parallel) modeling primitives as `#[boundary]`/`#[slag]` (then #52/#15 already aggregate them, OQ-3). |
