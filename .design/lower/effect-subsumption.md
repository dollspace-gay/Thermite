# Compile-Time Effect-Row Subsumption (`fx`)
<!--
tier: 3-component
status: draft
governs: thermite-lower/src/effects.rs
thesis-refs:
  - thermite-design.md §4.1
  - thermite-design.md §9
  - thermite-design.md §11
-->

## Summary

`thermite-lower::effects` enforces the **effect-row subsumption rule** at compile
time: *a caller's `fx` row must subsume every callee's row* (`thermite-design.md
§4.1`: "Effect rows compose: a caller's row must subsume every callee's row,
checked at compile time"). `fx pure` permits nothing; a `pure` function that
calls an effectful one is a compile-time rejection. In v0.1 this is the
**compile-time check ONLY** — the runtime syscall sandbox (§4.1 "killed at the
syscall boundary") is DEFERRED to issue #21 (`goal.md` "EXCLUDED from the
kernel: runtime effect sandbox (compile-time `fx` subsumption only in v0.1)").
This component is the static half, fully implemented (R-SPEC-5: implement the
v0.1 form fully, do not stub the deferred form).

This doc is GREENFIELD / FORWARD-LOOKING. Only the empty
`thermite-lower/src/lib.rs` scaffold root exists (no `effects.rs`). Every REQ is
**NOT-STARTED**, blocked on issue **#4**. The corpus programs (`sum`,
`binary_search`) are both `fx pure` with no internal calls to effectful
functions, so they are the ACCEPT baseline; reject cases are crafted fixtures.

## Requirements

- **REQ-1 (the effect lattice):** The effects form a lattice over the powerset of
  the atomic effect set the AST models (`thermite-syntax/src/ast.rs` `enum
  Effect`): `{ Read(path), Write(path), Net(domain), Alloc, Time, Rand, Panic,
  Diverge }`. The ordering is subset inclusion: `{}` (≡ `pure`) is the bottom;
  any set subsumes its subsets. `EffectRow::Pure` (`ast.rs` `enum EffectRow`) is
  the empty set `{}` — the bottom element that permits nothing. The join of two
  rows is set union. Derived from §4.1 (the effect set + "Effect rows compose").

- **REQ-2 (the subsumption rule — the accept relation):** A row `R_caller`
  **subsumes** `R_callee` iff `effects(R_callee) ⊆ effects(R_caller)`, where
  `effects(Pure) = {}` and `effects(Set(v)) = v`. `Read`/`Write`/`Net` carry a
  path/domain argument; v0.1 subsumption is at the **atom level** (a `Write(p)`
  caller subsumes a `Write(q)` callee for any paths — path-granular subsumption is
  a later refinement, see OQ-1), so subsumption reduces to: every atomic effect
  *kind* in the callee's row is present in the caller's row. `pure ⊆ pure` holds;
  `pure` subsumes only `pure`. Derived from §4.1 ("a caller's row must subsume
  every callee's row").

- **REQ-3 (the check entry point + call graph):** `check_effects(program,
  fn_rows) -> Result<(), Vec<LowerError>>` walks every `FnItem` body, and for each
  `Expr::Call`/`Expr::MethodCall` whose callee resolves to another declared
  `FnItem` (by name), checks that the *caller's* `Contract.fx` subsumes the
  *callee's* `Contract.fx` (REQ-2). It accumulates one structured error per
  violation (crisp feedback, §2.4) rather than failing on the first. A
  `spec fn` carries no `fx` (`ast.rs` `struct SpecFnItem` has no `fx`) and is pure
  by construction (§4.2 spec sublanguage is total/effect-free) — calls to a
  `spec fn` from any row are always permitted. Calls to combinators are likewise
  pure. Derived from §4.1 + `ast.rs` (`FnItem.contract.fx`, `SpecFnItem` has no
  `fx`).

- **REQ-4 (structured rejection, `LowerError`):** A subsumption violation returns
  a span-bearing `LowerError::EffectNotSubsumed { caller, callee, missing:
  Vec<Effect>, span }` (sharing `lower.rs`'s `LowerError`, REQ-9 there) naming the
  caller, the callee, and the atomic effects the callee has that the caller's row
  lacks — so the diagnostic tells the agent exactly which effect to add to the
  caller's row or remove from the callee (§2.4 actionable feedback). NEVER panics
  (R-CODE-2 / R-APG-1). Derived from §2.4, R-CODE-2.

- **REQ-5 (the maximal-row / `#[slag]` interaction — recorded boundary):** A
  maximal effect row without `#[slag]` justification is a vacuity-battery reject
  (§7.1 "Effect row is maximal (`fx *`) without `#[slag]` justification →
  reject") — but that structural triage is FORGE's vacuity stage (#6,
  `.design/forge/vacuity-triage.md`), NOT this component. `effects.rs` enforces
  subsumption only; it does not judge whether a row is "too broad." This REQ
  records the boundary so the critic does not expect maximal-row triage here.
  Derived from §7.1 + issue #6 ownership.

- **REQ-6 (runtime sandbox deferred to #21 — recorded boundary):** The runtime
  enforcement of `fx` (§4.1 "killed at the syscall boundary, not trusted at the
  type level alone") is OUT of v0.1 scope (issue #21). v0.1 ships compile-time
  subsumption only; this component emits NO runtime sandbox and inserts NO
  syscall interception. R-SPEC-5: the v0.1 form (compile-time check) is
  implemented FULLY, not stubbed; the deferred form (sandbox) is not built.
  Derived from `goal.md` scope + §4.1 + issue #21.

## Acceptance criteria

- **AC-1 (lattice + subsumption unit law):** `subsumes(R_a, R_b)` is reflexive
  (`subsumes(R, R)` for every row), and `subsumes(Pure, R)` iff `R == Pure`, and
  `subsumes(Set(all_atoms), R)` for every `R`. Mechanically: a table-driven test
  over hand-chosen rows asserts the relation against hand-derived expected bools
  (R-CHAR-3). (REQ-1, REQ-2)

- **AC-2 (corpus accepts):** `check_effects` over the parsed `conformance/sum.th`
  and `conformance/binary_search.th` returns `Ok(())` — both are `fx pure`, call
  only the pure `spec_sum` / combinators, and so trivially subsume. (REQ-2, REQ-3)

- **AC-3 (crafted accept cases):** Hand-crafted fixtures accept: a `fx {alloc}`
  caller calling a `fx {alloc}` callee; a `fx {read(x), write(y)}` caller calling
  a `fx {read(x)}` callee; any row calling a `spec fn`. Each expected `Ok`
  hand-derived. (REQ-2, REQ-3)

- **AC-4 (crafted reject cases — the right missing-effect set):** Hand-crafted
  fixtures reject with `EffectNotSubsumed` naming the right `missing` atoms: a
  `fx pure` caller calling a `fx {alloc}` callee → `missing: [Alloc]`; a
  `fx {read(x)}` caller calling a `fx {read(x), net(d)}` callee → `missing:
  [Net(d)]`; a `fx pure` caller calling a `fx {panic}` callee → `missing:
  [Panic]`. Each fixture's expected variant + missing set is hand-derived
  (R-CHAR-3). (REQ-4)

- **AC-5 (no panic; never swallows):** `check_effects` over malformed / deeply
  nested / unresolved-callee inputs returns `Ok`/`Err`, never panics
  (R-APG-1); an unresolved callee (a name that is neither a declared `FnItem`,
  `SpecFnItem`, nor a combinator) is handled as a no-op for subsumption (it is the
  validator's #2 job to reject unknown names), not a panic. (REQ-3, REQ-4)

- **AC-6 (no runtime sandbox emitted):** The component emits no syscall-sandbox
  code and the lowered output (from `lower.rs`/`l1.rs`) contains no effect-runtime
  scaffolding; `effects.rs` is a pure compile-time check returning `Result`.
  Mechanically: `effects.rs` has no codegen path, only a checking path. (REQ-6)

## Architecture

`thermite-lower/src/effects.rs`: a compile-time analysis over the
`thermite-syntax` AST, sibling to `lower.rs` / `l1.rs`, sharing the `LowerError`
enum. Symbol anchors: `enum EffectRow { Pure, Set(Vec<Effect>) }` and `enum
Effect { Read, Write, Net, Alloc, Time, Rand, Panic, Diverge }` in
`thermite-syntax/src/ast.rs`; `struct FnItem` (`.contract.fx`), `struct
SpecFnItem` (no `fx`); `fn lookup` in `thermite-spec/src/combinators.rs` (to
classify a callee as a pure combinator).

### The effect lattice (REQ-1)

```
            {Read, Write, Net, Alloc, Time, Rand, Panic, Diverge}   (top)
                              … powerset …
   {Alloc}      {Read}      {Write}    …    {Panic}                 (atoms)
                              \  |  /
                               { }  ≡  pure                          (bottom)
```

- Order: `R_a ⊑ R_b ⇔ effects(R_a) ⊆ effects(R_b)`.
- Bottom: `EffectRow::Pure` ≡ `{}` — permits nothing, subsumed by everything,
  subsumes only itself.
- Join: set union (used when computing the effective row of a body that makes
  several effectful calls — its required caller row is the union of callee rows).
- The atomic set is exactly `enum Effect`'s eight variants. `Diverge` is the
  termination-escape effect (§4.1 "divergence requires `fx diverge`"); it sits in
  the lattice like any other atom for subsumption purposes.

### The subsumption check (REQ-2/REQ-3)

```
effects(Pure)     = {}                 // the bottom
effects(Set(v))   = { kind(e) | e ∈ v } // atom kinds, path-insensitive in v0.1

subsumes(caller, callee)  ⇔  effects(callee) ⊆ effects(caller)
```

`check_effects` first builds a name→`Contract.fx` map over the program's
`FnItem`s (and notes `SpecFnItem` names as pure). It then walks each `FnItem`
body's `Expr` tree; for every `Call`/`MethodCall` whose callee path resolves to a
declared `FnItem`, it asserts `subsumes(caller.fx, callee.fx)`, emitting
`EffectNotSubsumed { missing = effects(callee) \ effects(caller) }` on failure.
Callees that resolve to a `SpecFnItem` or a registry combinator
(`thermite-spec::lookup`) are pure ⇒ always subsumed. Unresolved callees are a
no-op (the #2 validator owns unknown-name rejection — REQ-3 / AC-5).

The walk reuses the bounded-recursion discipline the parser and validator
established (`guard_recursion` / `MAX_RECURSION_DEPTH` in
`thermite-syntax/src/parser.rs`; mirrored in `thermite-spec` per
`.design/spec/spectherm-combinators.md` REQ-5) so a pathological body returns a
structured error, never a stack overflow.

### Path granularity (v0.1 decision)

`Read`/`Write`/`Net` carry a path/domain (`ast.rs` `Effect::Read(Ident)`). v0.1
subsumption is **atom-kind level**: a `Write(_)` caller subsumes any `Write(_)`
callee. Path-granular subsumption (`write("/tmp/a")` does NOT subsume
`write("/etc/passwd")`) is a refinement that needs a path lattice the v0.1 kernel
does not build (OQ-1). This keeps the v0.1 check honest about what it enforces
and matches §4.1's prose, which states the composition rule at the row level
without specifying path-granular ordering.

### Corpus baseline

Both corpus programs are `fx pure` (`conformance/sum.th` line 14
`fx  pure`; `conformance/binary_search.th` line 7 `fx  pure`) and call only the
pure `spec_sum` and the combinators (`sorted`, `forall_in`, `forall_below`,
`forall_from`) — all pure. So `check_effects` returns `Ok(())` for the entire
corpus (AC-2); the reject cases are necessarily crafted fixtures (AC-4), as the
v0.1 corpus has no effectful program.

## Verification

`cargo test -p thermite-lower` (this route has no golden `reference` in
`tooling/spec-routes.toml` — the checks are unit-level over crafted rows):

- **AC-1:** table-driven `subsumes` law test (reflexive; `Pure` subsumes only
  `Pure`; top subsumes all) with hand-derived expected bools (R-CHAR-3).
- **AC-2:** `check_effects` over the parsed corpus returns `Ok(())`.
- **AC-3:** crafted accept fixtures (`{alloc}`→`{alloc}`, `{read,write}`→`{read}`,
  any→`spec fn`) return `Ok`.
- **AC-4:** crafted reject fixtures assert `EffectNotSubsumed` + the exact
  `missing` atom set, hand-derived.
- **AC-5:** malformed / deep / unresolved-callee inputs return `Result`, no panic.
- **AC-6:** confirm `effects.rs` has only a checking path, no codegen / sandbox.

Gauntlet (R-DEFER-6): `cargo test -p thermite-lower`,
`cargo clippy -p thermite-lower --all-targets -- -D warnings`,
`cargo fmt --check`.

There is no golden Verus / corpus-cert reference for effect subsumption (it is a
static check, not a lowering or a certificate field in v0.1). The reject cases
are crafted unit fixtures, hand-derived from §4.1 (R-CHAR-3).

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (effect lattice) | SHIPPED | `enum EffectKind` (the 8 atoms) + `fn effects` (powerset projection of `EffectRow`) in `effects.rs`; consumer `subsumes`/`missing_atoms`; asserted by `tests/effects.rs::lattice_law_*` (AC-1). |
| REQ-2 (subsumption accept relation) | SHIPPED | `pub fn subsumes` in `effects.rs` (`effects(callee) ⊆ effects(caller)`; `Pure` subsumes only `Pure`); consumer `check_effects::check_call`; asserted by `lattice_law_table` + `crafted_accepts` (AC-1/AC-3). |
| REQ-3 (check entry point + call graph) | SHIPPED | `pub fn check_effects` in `effects.rs` builds a name→`fx` map over `FnItem`s (`SpecFnItem`/`thermite_spec::lookup` combinators noted pure) and walks each body's `Expr` tree (`check_block`/`check_expr`) per `Call`/`MethodCall`; consumer `tests/effects.rs` + the `pub use` lowering-pipeline surface; asserted by `corpus_accepts` (AC-2) + `crafted_rejects` (AC-4). |
| REQ-4 (structured rejection, `LowerError`) | SHIPPED | `LowerError::EffectNotSubsumed { caller, callee, missing, span }` in `lower.rs` (`Display` arm + `effect_atom_name`); produced by `check_call`; `missing` = `effects(callee) \ effects(caller)`; asserted by `reject_*` (AC-4). |
| REQ-5 (maximal-row / slag boundary) | SHIPPED | boundary recorded — `effects.rs` enforces subsumption only; no maximal-row judgement in the file (that is forge's vacuity stage #6). |
| REQ-6 (runtime sandbox deferred to #21) | SHIPPED | boundary recorded — `effects.rs` returns `Result<(), Vec<LowerError>>` with NO codegen / NO syscall-sandbox path (AC-6); the sandbox stays deferred to #21 (R-SPEC-5). |

## Open questions (for the orchestrator before the builder runs)

- **OQ-1 (path/domain granularity):** v0.1 subsumption is atom-kind level
  (`Write(_)` subsumes any `Write(_)`). Path-granular subsumption (a `write`
  caller scoped to `/tmp` not subsuming a callee writing `/etc`) needs a path
  lattice and matches the eventual runtime sandbox's granularity (§4.1 mentions
  `write(path)`). Deferring it keeps v0.1 honest; recorded as a refinement, not a
  v0.1 kernel item. Not a blocker.

- **OQ-2 (transitive vs. direct subsumption):** §4.1 says a caller subsumes
  "every callee's row." The v0.1 check is DIRECT (each call site checked against
  the immediate callee's declared row). Because every callee's declared row must
  already subsume ITS callees (checked when that callee is analyzed), direct
  checking composes to transitive correctness — the same locality property §9
  ("Trust is invariant under composition") relies on. Recorded so the critic does
  not expect a transitive-closure walk; the per-function check suffices. Not a
  blocker.

- **OQ-3 (no effectful corpus program — reject coverage):** The v0.1 corpus is
  entirely `fx pure`, so the ACCEPT path (AC-2) has corpus coverage but the
  REJECT path (AC-4) is covered only by crafted fixtures. This is acceptable
  (the fixtures are hand-derived from §4.1, R-CHAR-3), but it means the
  subsumption-reject logic has no conformance-corpus anchor until an effectful
  corpus program exists. Flagged as the thinnest external-truth coverage in this
  component (the lattice/subsumption logic is simple, so confidence stays high).
  Not a blocker.
