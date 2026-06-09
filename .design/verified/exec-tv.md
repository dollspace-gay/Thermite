# Exec-Position (Body) Translation Validation — step 2

<!--
tier: 3-component
status: draft
governs: thermite-tv/src/exec_encode.rs, thermite-tv/src/obligation.rs, thermite-tv/src/gen.rs, forge/src/exec_tv.rs
thesis-refs:
  - thermite-design.md §1 (trust relocated: code → spec → spec-intent)
  - thermite-design.md §4.1 (contract-first functions — and the EXEC body they guard)
  - thermite-design.md §6 (the verification ladder; L3 = Verus-derived SMT proof; L1 runtime checks)
  - thermite-design.md §5.1 (counterexamples, not adjectives)
epic: crosslink #151
step-1-sibling: .design/verified/contract-tv.md (crosslink #139 — CONTRACT-position TV, shipped + total on the corpus)
-->

## Summary

Step-1 contract-TV (`.design/verified/contract-tv.md`) certifies that the emitted Verus *contract*
(`req`/`ens`/`inv`/`dec`) MEANS the same thing as the source contract. It does NOT cover the EXEC
BODY: a function body lowers exec expressions (`lower_expr` under `Ctx::exec()` — real `u64`/`usize`
arithmetic, the exec method calls, slice indexing, the always-active runtime overflow checks, NO
slice→`@`/`nat` rewrites). The #122 (`(n - 1) as nat` paren) and #146 (`x as u32 < 33` cast-`<`
mis-parse) infidelity classes are EXEC-EXPRESSION lowering bugs; in contract position they are caught
by contract-TV, but their general home is the body. This component adds **exec-position TV (step
2.1)**: an INDEPENDENT EXEC-semantics reference denotation of a pure exec expression's VALUE, wrapped
as an exec-fn obligation `fn tv_exec_wrap(..) ensures result == <reference> { <production exec
lowering> }`, discharged through the existing `forge::check::run_verus`. VERIFIED ⟺ the production
exec-lowering produces the reference VALUE for all inputs ⟺ faithful; a `postcondition not satisfied`
(or a verus type/parse error from a malformed production lowering) ⟺ infidelity. This closes the
#122/#146 class GENERALLY (off-corpus, not just in contract position).

**Scope split (the crux — pin it precisely):**
- **Step 2.1 (DESIGNED HERE, doable NOW):** TV for PURE EXEC-POSITION EXPRESSIONS — arithmetic,
  casts, comparisons, calls, indexing — in BODY/exec context (`Ctx::is_spec()` false). NO statements,
  loops, mutation, or control flow. A pure exec expr has a denotation (a `u64`/`usize`/`bool` VALUE)
  with NO state, so an independent reference is a total recursion and the exec-fn obligation is a
  single `ensures`. This is where #122/#146 lived.
- **Step 2.2 (FRAMED ONLY, kernel-gated):** a mechanized OPERATIONAL SEMANTICS for the exec
  sublanguage's STATEMENTS / LOOPS / MUTATION + behavioral refinement of whole fn bodies. The big,
  moving object. NOT designed here (see "Step 2.2 horizon").

## Trust model (same as step 1 — N-version differential validation, not proof)

Exec-TV checks `production-exec-lowering ≡ independent-exec-reference`. Agreement is EVIDENCE, not
PROOF: both encoders could share a wrong assumption. What makes it meaningful is asymmetry of
auditability — the exec reference encoder is a small total recursion over the pure-exec subset of
`Expr` (arithmetic, casts, comparisons, calls, indexing), authored against `thermite-design.md`
§4.1/§6 + standard Rust/Verus exec semantics, independently of `lower_expr in lower.rs`. A human can
certify it by inspection; the production `lower_expr` (~2000 exec-path lines, shape-keyed rewrites)
cannot. The honesty boundary is HARD (REQ-1): the exec reference MUST NOT call
`thermite_lower::lower::lower_expr` or any production lowering symbol — independence is the entire
point (`assert(result == result)`-style vacuity is the failure mode). Independence is enforced as a
COMPILE constraint: the `thermite-tv` crate has NO `thermite-lower` dependency (the step-1 invariant,
`cargo tree -p thermite-tv` = syntax + spec only, AC-6 of contract-tv.md).

## The mechanism (how to Z3-check exec-expr faithfulness)

**The hard part vs step 1.** A CONTRACT expr lowers to a Verus PREDICATE (a `bool`), directly
Z3-comparable as `assert((P_prod) <==> (P_ref))` (`obligation::equivalence_obligation`). An EXEC expr
lowers to Rust exec CODE producing a VALUE (`u64`/`usize`/`bool`) — NOT a predicate. So the
equivalence cannot be a `<==>` over two predicates.

**The exec-fn-wrapped obligation (the solution, GROUNDED below).** Wrap the source exec expr's
PRODUCTION exec-lowering as the BODY of an exec `fn`, with `ensures result == <independent exec
reference denotation>`, and let Verus discharge it. Verus reasons about an exec fn's value through
its `ensures`, so if the production exec-lowering computes the wrong value (the #122/#146 bug), the
`ensures` fails with a counterexample:

```verus
fn tv_exec_wrap(<free vars: u64/usize/&[u32]/...>) -> (result: <ret>)
    requires <enclosing req>,
    ensures result == <exec reference denotation of the source expr>,
{
    <production exec-lowering of the source expr — the artifact under test>
}
```

VERIFIED (`success: true, errors: 0`) ⟺ production's exec value equals the reference for ALL inputs
(Z3) ⟺ faithful. A counterexample is an input on which they differ. THREE failure shapes, all
grounded below: (a) `postcondition not satisfied` (production typechecks but computes the wrong
value — the value-distinguishing #122 wrapping case, the overflow infidelity, the off-by-one index);
(b) `E0308 mismatched types` (the #122 paren-drop makes the production text ill-typed — production
fails to typecheck ⟹ not faithful); (c) `error: expected ','` (the #146 cast-`<` mis-parse — the
production text does not even parse). All three are HARD verus failures the obligation surfaces;
none is a silent pass.

**The EXEC-value semantics (the load-bearing concern — the dual of step-1's coercion soundness).**
The exec value is BOUNDED — `u64`/`usize`/`u32` with the always-active runtime overflow checks
(`thermite-design.md` §6, L1) — NOT unbounded `nat`/`int`. The reference denotation MUST capture the
EXEC value semantics, at the production VALUE TYPE, so an overflow-distinguishing infidelity is
CAUGHT (not coerced away):
- The reference for `a + b` (source `u64`) is the bounded `u64` `a + b` — which CARRIES the verus
  overflow obligation. A production that lowers to `a.wrapping_add(b)` (an overflow infidelity)
  FAILS the obligation (`postcondition not satisfied`), GROUNDED below. A reference that silently
  coerced to `nat` (`a as nat + b as nat`) would mask the wrap point — exactly the soundness hole
  to avoid. So the exec reference is NEVER nat-coerced; the comparison is at the production type.
- A cast `(e) as u32` is a NARROWING/wrapping operation in exec position; the reference re-states it
  at the cast target with the #122 inner-paren discipline (`Expr::Cast in lower.rs`) and the #146
  cast-`<` outer-paren discipline (`lower_binary_operand`/`is_lt_leading in lower.rs`) — INDEPENDENTLY
  (the reference does NOT import production's paren logic).
- Indexing `xs[i]` (source `i: usize`, `xs: &[u32]`) has the exec reference `xs[i as int]` (the spec
  view of the same element value) — GROUNDED: faithful VERIFIES, an off-by-one production
  (`xs[i + 1]`) FAILS `postcondition not satisfied`.

**The home (where it lives).** Reuse the step-1 architecture; extend, do not fork:
- `thermite-tv/src/exec_encode.rs` — the NEW exec-expr reference encoder (REQ-1), sibling to
  `ref_encode.rs` (which stays CONTRACT-only). Independent EXEC semantics.
- `thermite-tv/src/obligation.rs` — a NEW `exec_equivalence_obligation` (REQ-2) alongside the
  existing contract `equivalence_obligation`. It emits the exec-fn-wrapped `ensures` form, NOT the
  proof-fn `<==>` form.
- `thermite-tv/src/gen.rs` — extend with `gen_exec_exprs` (REQ-3): typed EXEC-position `Expr` trees.
- `forge/src/exec_tv.rs` — the NEW forge check phase (REQ-5), sibling to `contract_tv.rs`. The
  production side is `thermite_lower::lower_contract_expr` called in EXEC context (the per-expr
  production entry; a `lower_exec_expr` exec-context variant is the #152-noted prerequisite if the
  existing `lower_contract_expr` spec-context entry cannot be reused). Discharged through
  `forge::check::run_verus` / the `ScratchDir` (#53) cleanup, exactly as `contract_tv::discharge`.

## Requirements

- **REQ-1 (exec-expr reference encoder — independent, EXEC semantics)** —
  `thermite_tv::exec_encode::ref_exec_expr(expr: &Expr, &ExecRefCtx) -> Result<String>` maps a pure
  exec-position `Expr` to a Verus EXEC-VALUE expression STRING, covering arithmetic (`Expr::Binary`
  with `Add`/`Sub`/`Mul`/`Div`/`Rem`/shifts/bitops at the BOUNDED `u64`/`u32`/`usize` type — NOT
  `nat`/`int`), comparisons (`Eq`/`Ne`/`Lt`/`Le`/`Gt`/`Ge` → `bool`), casts (`Expr::Cast` at the
  target type, with the #122 inner-paren for a `Binary`/`Unary` inner and the #146 outer-paren when a
  `Cast` is the LEFT operand of a `<`-leading op), calls (`Expr::Call` — the exec callee verbatim),
  and indexing (`Expr::Index` — `xs[i as int]` for the spec view of the exec element value). Derived
  from `thermite-design.md` §4.1/§6. **HARD CONSTRAINT (R-CHAR-3 / trust model):** MUST NOT call
  `thermite_lower::lower::lower_expr` or any production lowering symbol; the `thermite-tv` crate keeps
  NO `thermite-lower` dependency.
- **REQ-2 (exec-fn-wrapped equivalence obligation + discharge path)** —
  `thermite_tv::obligation::exec_equivalence_obligation(source: &Expr, p_production: &str, frame:
  &ExecObligationFrame) -> Result<String>` emits a self-contained `fn tv_exec_wrap(<params>)
  requires <req>, ensures result == <ref_exec_expr output>, { <p_production> }` Verus unit (NOT the
  proof-fn `<==>` form — an exec value is not a predicate). Discharged through the EXISTING
  `forge::check::run_verus`. VERIFIED ⟺ faithful; a `postcondition not satisfied`/type/parse error ⟺
  infidelity. Derived from `thermite-design.md` §6. The production side reuses the per-expr exec
  lowering verbatim (the artifact under test); the reference side is REQ-1.
- **REQ-3 (generator extension — exec-position exprs)** — `thermite_tv::gen::gen_exec_exprs(seed,
  budget) -> impl Iterator<Item = ExecClause>` produces well-typed EXEC-position `Expr` trees over
  the pure-exec subset: `u64`/`usize` arithmetic, narrowing/widening casts, the cast-`<` form
  (`n as u32 < k` — the #146 surface), comparisons, indexing of a `&[u32]` param. Seeded +
  deterministic (R-CODE-5). This un-bounds the #122/#146 fidelity check off-corpus (the step-1
  generator is contract-position only). Derived from `thermite-design.md` §1.
- **REQ-4 (the teeth — R-CHAR-3)** — a conformance test (`thermite-tv/tests/exec_teeth.rs`)
  asserting (a) FAITHFUL exec exprs VERIFY and (b) each injected exec infidelity produces a verus
  FAILURE: the #122 cast-paren drop (`E0308`/value-distinguishing wrapping → `postcondition not
  satisfied`), the #146 cast-`<` mis-parse (`error: expected ','`), and an overflow-distinguishing
  infidelity (`wrapping_add` for source checked `+` → `postcondition not satisfied`). GROUNDED below.
  Expected values trace to `thermite-design.md` §4.1/§6 + the #122/#146 fixes, never the lowerer's
  output.
- **REQ-5 (forge plug-in point)** — a new `forge::exec_tv` check phase runs the exec-expr TV over the
  pure exec exprs of each checked item's body, exposed as `forge tv <file> --exec` (the non-test
  consumer is `cli::run_tv`). An exec-TV counterexample is a per-expr DIVERGENT verdict (a body
  meaning-mismatch finding). Derived from `thermite-design.md` §6.

## Acceptance criteria

- **AC-1 (faithful exec expr → verified)** — the exec-fn obligation for `(n - 1) as u64` under
  `req n >= 1`, production `(n - 1) as u64`, reference `(n - 1) as u64`, discharges as `success:
  true, verified: 1, errors: 0`. GROUNDED below.
- **AC-2 (#122 cast-paren infidelity → caught)** — (a) the paren-drop form `n - 1 as u8` for source
  `(n - 1) as u8` (`n: u64`, target `u8`) fails verus with `E0308 mismatched types`; (b) the
  value-distinguishing wrapping form fails with `postcondition not satisfied`. GROUNDED below.
- **AC-3 (#146 cast-`<` mis-parse → caught; faithful → verified)** — production `x as u32 < 33` for
  source `(x as u32) < 33` fails verus with `error: expected ','`; the faithful `(x as u32) < 33`
  VERIFIES (`verified: 1, errors: 0`). GROUNDED below.
- **AC-4 (overflow-distinguishing infidelity → caught)** — production `a.wrapping_add(b)` against the
  bounded `u64` reference `a + b` fails with `postcondition not satisfied`; the faithful bounded
  checked-add (`requires a + b <= u64::MAX`) VERIFIES. GROUNDED below.
- **AC-5 (usize indexing exec expr discharges)** — `xs[i] as u64` (`i: usize`, `xs: &[u32]`, `req i <
  xs.len()`) against reference `xs[i as int] as u64` VERIFIES; an off-by-one production (`xs[i + 1]`)
  FAILS `postcondition not satisfied`. GROUNDED below.
- **AC-6 (independence is structural)** — `thermite-tv` keeps NO `thermite-lower` dependency; a
  `cargo tree -p thermite-tv` audit shows `exec_encode.rs` references no `lower_expr` symbol.
- **AC-7 (off-corpus coverage)** — `gen_exec_exprs` produces ≥ N exec exprs (incl. ≥1 cast-`<` and ≥1
  arithmetic-overflow-surface form) and the exec-TV obligation runs (and VERIFIES for the faithful
  lowerer) on each; a seeded run is reproducible.

## Architecture

**Reuse vs re-implement (the independence boundary).**
- **RE-IMPLEMENTED by the exec reference encoder (the infidelity surface):** the exec-context
  rewrites where fidelity bugs live — the cast paren discipline (#122 inner paren on a `Binary`/
  `Unary` cast inner; #146 outer paren on a `Cast` left of a `<`-leading op), the bounded-int
  arithmetic at the source type, the exec index form. Authored against `thermite-design.md` §4.1/§6 +
  standard Rust/Verus exec semantics, INDEPENDENTLY of `Expr::Cast in lower.rs` /
  `lower_binary_operand in lower.rs` / `lower_index in lower.rs`. The `binop(BinOp) in lower.rs` 1-to-1
  operator map is re-stated independently (re-stating it is the point — an imported map would hide a
  production binop bug).
- **REUSED (correct):** `thermite_spec::lookup(name)` for any spec-fn / combinator a call resolves to
  (the frozen shared ground truth, same as step 1) — but in EXEC position the corpus body exprs are
  arithmetic/cast/index/exec-call, so the combinator path is rare; the common case is pure scalar
  exec arithmetic with no registry lookup.

**The pure-exec subset is small (why the reference is auditable).** A pure exec expr is the
arithmetic/cast/comparison/call/index subset of `Expr` — NO statements, NO `let`, NO loops, NO
mutation, NO control flow (those are step 2.2). The reference encoder is a total recursion over that
subset, NOT a re-implementation of `lower_block`/`lower_stmt`/the loop machinery.

**The exec-fn obligation shape (REQ-2).** For a source exec expr with free vars (the body params it
reads — `u64`/`usize`/`&[u32]`/...) + a return type (the exec value's type), under the enclosing
`req` as a `requires`:

```verus
fn tv_exec_wrap(<params>) -> (result: <ret>)
    requires <enclosing_req>,
    ensures result == <ref_exec_expr(source)>,
{
    <production exec lowering of source>
}
```

Verus discharges the exec fn's value via its `ensures`. The production body is an EXEC fn body
(`fn`, not `proof fn`/`spec fn`), so the always-active runtime overflow checks (`thermite-design.md`
§6, L1) are LIVE — an overflowing arithmetic in production raises the obligation, matching the
bounded reference (AC-4). This is the structural reason the obligation is an exec fn, not a proof fn:
the exec-value/overflow semantics must be the ones under test.

## Verification

GROUNDED end-to-end against the real `verus` binary during authoring (the `forge`/`verus` toolchain
this dispatch had). The conformance test (REQ-4) replays these through `forge::check::run_verus`.

**AC-1 (faithful `(n - 1) as u64` → verified).** `fn tv_exec_wrap(n: u64) -> (result: u64) requires
n >= 1, ensures result == (n - 1) as u64, { (n - 1) as u64 }`:

```
"verification-results": { "success": true, "verified": 1, "errors": 0 }
```

**AC-2a (#122 paren-drop → E0308).** Source `(n - 1) as u8` (`n: u64`, target `u8`); production drops
the inner paren → `n - 1 as u8` == `n - (1 as u8)` (a `u64 - u8` mix):

```
error[E0308]: mismatched types
   |
   |     n - 1 as u8
   |     ^ no implementation for `u64 - u8`
error: aborting due to previous error
```

(Production fails to typecheck ⟹ not faithful — the obligation surfaces it as a hard verus failure.)

**AC-2b (#122 value-distinguishing → counterexample).** A wrapping cast where the paren position
changes the WRAP point (reference: widen-add-narrow `(a as u64 + b as u64) as u8`; production wraps
each first `(a as u8).wrapping_add(b as u8)`):

```
error: postcondition not satisfied
  --> ...:10:13
   |
10 |     ensures result == ((a as u64 + b as u64) as u8),
   |             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ failed this postcondition
"verification-results": { "success": false, "verified": 0, "errors": 1 }
```

**AC-3 (#146 cast-`<`).** Faithful `(x as u32) < 33` (`x: u64`, `req x <= 1000`, reference `(x as
u32) < 33`):

```
"verification-results": { "success": true, "verified": 1, "errors": 0 }
```

Production drops the outer paren → `x as u32 < 33` (the `u32 <` mis-parses as a generic-arg list):

```
error: expected `,`
error: aborting due to 1 previous error
```

**AC-4 (overflow-distinguishing infidelity → counterexample).** Reference at the bounded `u64` type
`result == a + b` (carries the verus overflow obligation); production `a.wrapping_add(b)`:

```
error: postcondition not satisfied
  --> ...:8:13
 8 |     ensures result == a + b,
   |             ^^^^^^^^^^^^^^^ failed this postcondition
error: aborting due to 1 previous error
```

The faithful bounded checked-add (`requires a + b <= u64::MAX`, production `a + b`):

```
"verification-results": { "success": true, "verified": 1, "errors": 0 }
```

This is the EXEC-value-semantics teeth: a `nat`-coerced reference would mask the wrap point; the
bounded `u64` reference catches it.

**AC-5 (usize indexing).** Faithful `xs[i] as u64` (`i: usize`, `xs: &[u32]`, `req i < xs.len()`,
reference `xs[i as int] as u64`) → `verified: 1, errors: 0`. Off-by-one production (`xs[i + 1] as
u64`, `req i + 1 < xs.len()`) → `error: postcondition not satisfied`.

These confirm the load-bearing feasibility questions: (1) Verus DOES discharge `ensures result ==
<reference>` for a production-lowered EXEC expr wrapped as an exec fn (AC-1/3/4/5 verify; the
infidelities fail); (2) the exec-value/overflow semantics match at the production type (AC-4), so an
overflow infidelity is caught rather than coerced away; (3) the #122/#146 class bites in EXEC
position (AC-2/AC-3), closing it generally off-corpus.

**Crate gauntlet (when built):** `cargo test -p thermite-tv`, `cargo test -p forge` (`exec_tv`
conformance), `cargo clippy -p thermite-tv -p forge --all-targets -- -D warnings`, `cargo fmt
--check`. Scratch/verus temp cleaned per the `ScratchDir` Drop guard (blocker #53).

## Step 2.2 horizon — operational semantics for statements/loops/mutation (kernel-gated, FRAMED not designed)

**The object.** Step 2.1 covers PURE EXEC EXPRESSIONS (a value, no state). It does NOT cover
control-flow/mutation faithfulness: `let`/assignment, `while`/loop bodies + invariants, `if`/`match`
as STATEMENTS, and the through-body refinement of a whole fn (does the lowered body's STATE TRACE
match the source's?). That requires a mechanized OPERATIONAL SEMANTICS for the exec sublanguage's
statements/loops/mutation + a behavioral-refinement obligation over whole fn bodies — a much larger
object than a per-expression value equality.

**WHY it is kernel-gated.** An operational semantics is a definition of meaning for EVERY construct
in the exec sublanguage. If the sublanguage is still GROWING (new statement forms, new loop shapes,
new mutation patterns land as the kernel is built), the semantics chases a moving target: every new
construct invalidates the refinement proof and the reference operational model. The semantics is only
sound + maintainable once the set of exec STATEMENT/CONTROL-FLOW constructs is FROZEN. Step 2.1's
expression subset is ALREADY effectively frozen (arithmetic/cast/comparison/call/index — the §4.1
expression grammar is stable), which is exactly why 2.1 is doable now and 2.2 is not.

**The frozen-subset PREREQUISITE.** Before 2.2: a FROZEN KERNEL EXEC-STATEMENT SUBSET — an
enumerated, design-pinned list of the exec statement/control-flow/mutation forms (the `lower_stmt` /
`lower_block` / loop-lowering construct set), declared stable (no new forms admitted without a design
amendment). 2.2's operational reference is authored against that frozen list. Until the v0.1 kernel's
exec-body construct set is mechanically complete (goal.md stopping condition) and pinned, 2.2 stays
framed.

**Honest scope boundary (state in the certificate).** Exec-TV (2.1) certifies that a pure exec
EXPRESSION's lowered VALUE matches the source. It does NOT certify control-flow/mutation/loop body
faithfulness — that is 2.2, kernel-gated. A reader must not read exec-expr TV as whole-body
faithfulness.

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (exec-expr reference encoder) | SHIPPED | `thermite_tv::exec_encode::exec_ref_value` (`thermite-tv/src/exec_encode.rs`) — the independent BOUNDED exec-VALUE encoder (`u64`/`u32`/`usize`/`bool`, NEVER `nat`/`int`): arithmetic at the operand type (the overflow obligation carried), comparisons → `bool`, casts with the #122 inner-paren (`(n - 1) as u8`) + the #146 cast-`<` outer-paren (`(x as u32) < 33`, independent `is_lt_leading`), calls verbatim, and the slice index → the spec-view element value `xs[i as int]`. Non-test consumer `obligation::exec_equivalence_obligation`. Verified by `thermite-tv/tests/exec_teeth.rs` E1–E4 under real verus (the `exec_ref_value_matches_faithful_meaning` unit + the four faithful obligations VERIFY). Deps `thermite-syntax` + `thermite-spec` ONLY — no `thermite-lower` (`cargo tree -p thermite-tv` = syntax + spec; AC-6). Out-of-scope (method calls / Vec-String accessors) → honest `RefEncodeError::Unsupported` (#154/#156 territory), never silent-wrong. |
| REQ-2 (exec-fn-wrapped equivalence obligation + discharge) | SHIPPED | `thermite_tv::obligation::exec_equivalence_obligation` + `ExecObligationFrame`/`ExecParamDecl` (`thermite-tv/src/obligation.rs`) emits the self-contained `fn tv_exec_wrap(<params>) requires <req>, ensures result == <exec_ref_value(source)>, { <p_production> }` EXEC-FN form (NOT the proof-fn `<==>`). The production side reuses `thermite_lower::lower_exec_expr` (the per-expr EXEC lowering, `thermite-lower/src/lower.rs` — re-enters `lower_expr` in `Ctx::exec()`, the standalone exec `Ctx` IS reachable for a pure expr; the #152 feasibility unknown RESOLVED). Discharged through real verus by `tests/exec_teeth.rs`: all four faithful VERIFY (`1 verified, 0 errors`), all four infidel CAUGHT (E1 `E0308 mismatched types`, E2 `error: expected ','`, E3/E4 `postcondition not satisfied`). The bounded reference catches the E3 wrap (NOT coerced away). GROUNDED in Verification above. |
| REQ-3 (off-corpus generator — exec exprs) | SHIPPED | `thermite_tv::gen::gen_exec_exprs` + `gen::ExecClause` (`thermite-tv/src/gen.rs`) — a DETERMINISTIC (SplitMix64-seeded, no `rand`/clock, R-CODE-5) generator of WELL-FRAMED exec-position `Expr`s over the bounded exec sublanguage: `u64`/`usize` arithmetic (`+`/`-`/`*`), shifts, bitwise, narrowing/widening casts (`as u8`/`u16`/`u32`/`u64`/`usize`), the cast-`<` surface (`x as u32 < k` — the #146 guard), and slice indexing (`xs[i]`). EACH `ExecClause` carries an ADEQUATE FRAME (every base scalar `<= 1000` + an index `< xs.len()`) so the FAITHFUL lowering VERIFIES (the overflow obligation does not spuriously fire). The frame-adequacy disciplines: arithmetic operands are PROVABLY-BOUNDED (no bitwise/shift/index result fed to `+`/`*`), and `*` scales by a small LITERAL only (a product of two unknowns is NONLINEAR — verus cannot bound it). Non-test consumer `forge::exec_tv::run_generated`. Determinism + construct coverage + self-framing in `gen::tests` (`exec_deterministic_and_seed_sensitive`, `exec_diverse_construct_coverage`, `exec_clauses_are_self_framed`) + the 200-expr all-faithful run in `forge/tests/exec_tv_conformance.rs`. Deps `thermite-syntax` + `thermite-spec` ONLY — no `thermite-lower` (AC-6). |
| REQ-4 (the teeth — R-CHAR-3) | SHIPPED | `thermite-tv/tests/exec_teeth.rs` — E1 (#122 cast-paren), E2 (#146 cast-`<`), E3 (wrong-op/overflow), E4 (off-by-one index): each FAITHFUL `p_production` (the exact `lower_exec_expr` output, pinned in `thermite-lower/src/lower.rs::exec_expr_tests` — the cross-crate bridge, since `thermite-tv` has no `thermite-lower` dep) VERIFIES + each INFIDEL is CAUGHT with the precise catch shape asserted (`CatchShape::Compile`/`Postcondition`). Expected values trace to the fixtures + §4.1/§6, never the lowerer's output. Skip-loudly if verus absent. |
| REQ-5 (forge plug-in point) | SHIPPED | `forge::exec_tv::run_generated` (the off-corpus exec run — PRIMARY, the #122/#146 regression guard) + `forge::exec_tv::exec_tv_file` (the corpus body-expr check — best-effort) (`forge/src/exec_tv.rs`); both compute `P_production` via `thermite_lower::lower_exec_expr` (CLOSING the consumer loop — R-DEFER-1), build the obligation via `thermite_tv::exec_equivalence_obligation`, and discharge it through `verus` (the `discharge` helper, reusing `crate::check::ScratchDir`/#53 cleanup). Non-test consumer `cli::run_exec_tv` (the `forge exec-tv <file>` subcommand). The FOUR-WAY classification — Faithful / Divergent / Unverifiable / Skipped — is REPORTED DISTINCTLY (Unverifiable/Skipped never mask an infidelity, R-HONEST-3): an inadequate body-expr overflow frame is Unverifiable, a statement/loop/non-derivable-frame/Unsupported is Skipped, a non-compiling production / postcondition counterexample is Divergent. Verified by `forge/tests/exec_tv_conformance.rs` under real verus: the 200-expr generated run is all-faithful (0 divergent/unverifiable/skipped) with the cast-`<`/arith/cast/index coverage non-vacuous; the corpus body-expr check is faithful-where-checked + the loop skipped HONESTLY (out-of-scope step 2.2). |
