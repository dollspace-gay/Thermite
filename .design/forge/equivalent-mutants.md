# Forge equivalent-mutant exclusion (the §7 kill-ratio denominator fix)

<!--
tier: 3-component
status: draft
governs: forge/src/mutation.rs
thesis-refs:
  - thermite-design.md §7
  - thermite-design.md §6
  - thermite-design.md §9
amended: 2026-06-12 (#269) — the CALL-BEARING-BODY gap: the shipped #101 probe
  spec-renders a call-bearing real body into an ILLEGAL self-contained spec fn
  (`spec fn equiv_real_caller(x: u32) -> u32 { (ext_id(x)) as u32 }`, no callee
  decl), so a genuinely-equivalent F-IDENT survivor of a §9 composition caller
  is never excluded and the caller falsely gates WeakContract. THE RULING
  (encoded as REQ-7/REQ-8/REQ-9, NOT-STARTED, blocker #269): the probe's
  equivalence notion is equivalence IN THE VERIFICATION SEMANTICS — MODULO
  CALLEE CONTRACTS; the call-bearing obligation is an EXEC-position proof
  harness with the callee closure woven exactly as `item_subprogram` weaves it
  for the caller's own L3 proof.
  R-SPEC-4 RECORD: `.design/forge/mutation-scoring.md`'s *Cert/oracle impact +
  landing order* (#269/#270) enumerated `editor_runs.rs`,
  `equivalent_mutants_conformance.rs`, `conformance/mutation/cases.json`,
  `divergence_mutation.rs`, the frozen goldens, and the Lean tallies — and
  MISSED `forge/tests/composition_conformance.rs` (F-IDENT on the call-bearing
  `caller`/`g`/`h` flips its two L3 assertions red). Its REQ-13 claimed the
  #101 rule "applies unchanged" with the honest limit pinned only for
  NON-SCALAR returns; the call-bearing SCALAR body was the unenumerated case.
  The correction note belongs in mutation-scoring.md (out of this dispatch's
  scope, per the #269 dispatch); it is recorded here instead, citing
  mutation-scoring.md REQ-13.
-->

## Summary

This component refines **§7 step 4** (`.design/forge/mutation-scoring.md`): a
SURVIVING mutant that Verus PROVES is observably-equivalent to the real body
*under the precondition* is dropped from the kill-ratio DENOMINATOR rather than
counted as a survivor. The bug it fixes (crosslink **#101**): the mutation gate
counts a mutant that is PROVABLY EQUIVALENT to the real body as a survivor,
falsely flagging an honest contract `WeakContract`. The textbook case is a
forced-output refusal — under a precondition that pins an output, the real body
and an "early-`return <that output>`" mutant are behaviorally identical, so no
input distinguishes them, yet the equivalent mutant survives and depresses the
ratio. The exclusion is **sound-but-incomplete**: a mutant is dropped ONLY on a
Verus PROOF of observable equivalence under `req`; a mutant Verus cannot prove
equivalent (a distinguishing input exists, or the proof times out)
conservatively STAYS counted. A genuinely-distinguishing survivor — the symptom
of a real `WeakContract` — is never excluded, so the exclusion cannot launder a
weak contract (`goal.md` R-DEFER-9).

This doc governs the per-survivor equivalence check + the denominator drop in
`forge/src/mutation.rs`; the gate-wiring change is in `check.rs`
(`mutation_score`) and is threaded through the existing `MutationScore` shape.

**Amendment (#269):** the shipped probe handles CALL-FREE scalar bodies only.
For a CALL-BEARING body (a §9 composition caller — `fn caller(x) { ext_id(x) }`)
the shipped spec-render is an illegal Verus form, so a genuinely-equivalent
survivor is never excluded and the honest caller falsely gates. REQ-7/REQ-8/
REQ-9 below extend the probe's equivalence notion to **equivalence modulo
callee contracts** — the same Hoare-style semantics the main L3 proof path
already uses (`.design/basis/05-composition.md` law 1: the caller proves
THROUGH the callee's contract, never its body). See *Amendment (#269)* below.

## Scope boundaries (documented, attributed)

- **IN:** for each mutant classified a SURVIVOR by §7 step 4 (Verus PROVED it
  against the unchanged contract), run ONE additional Verus EQUIVALENCE query —
  "under `req`, does the mutant body produce the same observable result as the
  real body for all inputs?". A PROVED-equivalent mutant is removed from BOTH the
  survivor set AND the `scored` denominator; an unproven one stays a counted
  survivor. The denominator/kill-ratio arithmetic + the `MutationScore` field
  carrying the excluded count; the `CHECK_SCHEMA_VERSION` bump (the gate verdict
  changes for forced-output fns). *(#269 widens IN: the equivalence query for a
  CALL-BEARING body, REQ-7 — the query assumes each callee's contract, exactly
  as the caller's own L3 sub-program does.)*
- **OUT — the killed/survived classification itself** (§7 step 4, REQ-4 of
  `.design/forge/mutation-scoring.md`). A KILLED mutant is never equivalence-
  checked (Verus already rejected it against the contract — by definition it is
  distinguished). Only SURVIVORS are candidates for exclusion.
- **OUT — mutating the contract / the mutator set** (`mutation-scoring.md`
  REQ-1/REQ-3). The equivalence check reads the SAME real body and the SAME
  mutant body the §7 gate produced; it adds no mutant and touches no contract.
  The F-IDENT/F-STRUCT-ZERO families themselves are `mutation-scoring.md`
  REQ-9..REQ-13's contract, not this doc's.
- **OUT — strengthening probes** (§7 step 5, `.design/forge/strengthening-
  probes.md`). The probe runs only on a met-floor cert; this exclusion changes
  whether the floor is met for a forced-output fn, but it adds no suggestion.

## Requirements

- **REQ-1 (per-survivor Verus equivalence check — §7):** for each mutant the §7
  gate classifies a SURVIVOR (`MutantOutcome::Survived` — Verus proved it against
  the unchanged contract), the gate issues ONE further Verus query asking whether,
  *under the function's `req`*, the mutant body's observable result equals the
  real body's observable result for ALL inputs. The query reuses the EXISTING
  Verus driver (`check::run_verus`-class invocation): the equivalence obligation
  is `ensures mutant_result == real_result`, discharged under `requires <the
  fn's req>` (and the fn's parameter types). It is a new CALLER of the existing
  prover path, not a new prover. Source: `thermite-design.md` §7 ("re-verifies
  each against the contract" — this is the dual: re-verify the survivor against
  the real body); `goal.md` R-CODE-4 (an environment/VIR failure surfaces a
  `ForgeError`, never a silent equivalence).

- **REQ-2 (PROVED-equivalent → dropped from the denominator; sound-but-
  incomplete):** if the equivalence query VERIFIES (Verus proves the mutant
  observably equal to the real body under `req`), the mutant is a TRUE equivalent
  mutant — not a contract weakness — and is removed from the kill-ratio
  DENOMINATOR (it does not count as a survivor, and it does not count as scored).
  If the query does NOT verify — Verus finds a distinguishing input
  (counterexample) OR the query times out — the mutant STAYS a counted survivor
  (the conservative, sound reading: exclude ONLY on a proof). This mirrors the
  existing OQ-5 / OQ-4 polarity in `mutation-scoring.md` (an un-lowerable mutant
  is already dropped from the denominator; an un-proved mutant is already
  conservatively the strict reading). Source: `thermite-design.md` §7; `goal.md`
  R-DEFER-9.

- **REQ-3 (the soundness line — a distinguishing mutant is NEVER excluded):** the
  exclusion is gated on a Verus PROOF of equivalence, so a mutant that DIFFERS
  from the real body under `req` (a genuinely-distinguishing survivor — exactly
  the symptom of a contract too weak to pin the behavior) fails the equivalence
  query and is NEVER dropped. The kill-ratio denominator for a genuinely-weak
  contract is therefore unchanged by this component; a weak contract still gates
  `WeakContract`. The exclusion narrows the denominator ONLY by mutants the
  prover certifies are indistinguishable from the truth. Source: `goal.md`
  R-DEFER-9 ("never discharge an obligation by weakening it"); `thermite-design.md`
  §7 (the battery's anti-Goodhart purpose).

- **REQ-4 (the `MutationScore` denominator change + the `K/N` cert):**
  `mutation::MutationScore` records the proved-equivalent exclusions so that
  `scored` (the denominator) is the count of mutants that lowered, ran, AND were
  NOT proved equivalent. `kill_ratio = killed / scored` and the Appendix A
  `mutants_killed = "killed/scored"` string both reflect the REDUCED denominator.
  The `0/0` backstop (`mutation-scoring.md` REQ-5: a `scored == 0` score is below
  floor, gated `WeakContract`) is UNCHANGED — if every scored mutant is killed or
  proved-equivalent, leaving a non-empty killed set, the ratio certifies; if
  exclusion empties the denominator entirely (every mutant proved equivalent and
  none killed), the `0/0` backstop STILL gates (a contract that cannot be
  mutation-validated has not met the §7 bar). Source: `thermite-design.md`
  Appendix A (`contract_quality.mutants_killed`); `mutation-scoring.md` REQ-5/REQ-6.

- **REQ-5 (`CHECK_SCHEMA_VERSION` bump — cache invalidation):** because the gate
  verdict CHANGES for forced-output fns (a `0/1` `WeakContract` becomes a
  certifying score once the equivalent mutant is excluded), the check logic is
  no longer the same function of its inputs. `cache::CHECK_SCHEMA_VERSION` is
  bumped (the on-disk cache key input, `cache.rs` `cache_key`), so stale cached
  verdicts for forced-output fns are invalidated and re-scored under the new
  logic. Source: `.design/forge/proof-cache.md` (the schema-version cache-key
  input, blocker #49); `cache.rs` `CHECK_SCHEMA_VERSION`.

- **REQ-6 (determinism + bounded cost — R-CODE-5):** the equivalence check is a
  Verus proof obligation — a deterministic function of the (mutant body, real
  body, `req`, parameter types) under a pinned seed + rlimit + toolchain, so a
  proved-equivalent exclusion is DETERMINISTIC (the same fn excludes the same
  mutants every run; `mutants_killed` stays deterministic, `mutation-scoring.md`
  REQ-8). The cost is ONE extra Verus run PER SURVIVOR (not per mutant): survivors
  are the few mutants the contract failed to kill, so the added cost is bounded by
  the survivor count, itself bounded by `MUTANT_CAP`. Each equivalence query is
  content-addressed through the SAME proof cache (#8), so re-runs are cheap.
  Source: `goal.md` R-CODE-5; `mutation-scoring.md` REQ-7/REQ-8.

### Amendment #269 — the call-bearing-body requirements (NOT-STARTED)

- **REQ-7 (call-bearing equivalence obligation — equivalence MODULO CALLEE
  CONTRACTS, the exec-harness form):** the probe's equivalence notion for a
  call-bearing body is **equivalence in the verification semantics**: assuming
  each callee's contract at its call site — the SAME modular rule the caller's
  own L3 sub-program uses (`.design/basis/05-composition.md` law 1, locality
  theorem: "a caller's proof references only the callees' CONTRACTS … never
  their bodies") — is `mutant_result == real_result` provable for all inputs
  under `req`? The obligation is rendered NOT as the shipped self-contained
  spec-fn pair but as an **EXEC-position proof harness**:

  ```verus
  // callee closure woven EXACTLY as item_subprogram weaves f's own sub-program:
  //   boundary/slag callee -> lower_external_body_fn's #[verifier::external_body]
  //   signature (requires/ensures verbatim, `{ unimplemented!() }` body);
  //   regular in-file callee -> its full lower_fn def (its contract governs the
  //   call site identically — modular verification).
  fn equiv_check_<name>(<params>) -> (eq: bool)
      requires <req>,
      ensures eq,
  {
      let real: <ret> = { <real body, rendered as an exec block value> };
      let mutant: <ret> = { <mutant body, rendered as an exec block value> };
      real == mutant
  }
  ```

  A VERIFIED harness (`ensures eq` proved, `0 errors`) is a PROOF that no input
  satisfying `req` distinguishes the mutant from the real body *given the callee
  contracts* → exclude (REQ-2's polarity unchanged). The callee closure is the
  caller's existing `reachable_fn_deps` set (`check.rs`) — a mutant introduces no
  NEW call, so `f`'s closure covers the mutant body too. Each body renders
  through the EXISTING body-as-value rules (`render_body_as_spec_value`'s
  leading-early-return / immutable-let-chain-plus-tail shapes) generalized to an
  EXEC block context so a call is legal; the scalar param/return gate
  (`scalar_obligation_type`) is retained for v1. The shipped CALL-FREE spec-fn
  form stays as-is (grounded, byte-stable, cache-warm); the harness is the
  call-bearing arm. Landing bumps `CHECK_SCHEMA_VERSION` per REQ-5's rule (the
  verdict changes for call-bearing fns). TRUST BASE: a proved-modulo-contracts
  exclusion assumes only that callees honor their contracts — exactly the trust
  base of the caller's own L3 cert (`to_boundary` scope, §9); the exclusion adds
  NO new trust. Source: `thermite-design.md` §9; `.design/basis/05-composition.md`
  law 1; `.design/lower/boundary-composition.md`; the #269 orchestrator ruling.
  **NOT-STARTED — blocker #269.**

- **REQ-8 (conservatism under a weak callee contract — never a false
  exclusion):** when the callee's contract is TOO WEAK to prove equivalence
  (e.g. callee `ens result <= 100` cannot pin `real == mutant`), the harness's
  `ensures eq` is unprovable → the probe returns NOT-equivalent → the survivor
  STAYS counted and the item still gates. The exclusion fires ONLY on a Verus
  PROOF of the harness (REQ-2/REQ-3's polarity, extended verbatim): a
  counterexample, a timeout, an unprovable-through-contracts query, or a
  mutant-side call whose `requires` cannot be discharged under the harness's
  `req` ALL keep the survivor counted. Semantic justification: a mutant the
  callee contracts CANNOT prove equivalent is either genuinely distinguishable
  or unknown — and the sound reading of unknown is "counted" (`goal.md`
  R-DEFER-9). Conversely a mutant proved equivalent modulo contracts cannot be
  killed by ANY caller-side `ens` the verifier could check (the verifier never
  sees past the callee contract, §9), so counting it punishes the caller for the
  callee's contract, not its own — the precise #101 false-gating pattern.
  Source: `goal.md` R-DEFER-9; `.design/basis/05-composition.md` (locality);
  `thermite-design.md` §7. **NOT-STARTED — blocker #269.**

- **REQ-9 (probe failure → structured `Unsupported`, never a silent
  exclusion — R-HONEST-3):** a probe FAILURE — an out-of-scope body shape, an
  ill-formed emitted obligation, a verus compile-level rejection, a solver
  error — must NEVER exclude the survivor, and must never be SILENTLY swallowed.
  The shipped consumer collapses every render error to bare `Ok(false)`
  (`equivalence_proves_equal`: `Err(_) => return Ok(false)`); under this REQ the
  call-bearing arm (a) keeps the not-excluded polarity (a failed probe is NO
  proof → the survivor stays counted) and (b) graduates the failure to a
  STRUCTURED form: `lower_equivalence_obligation`'s `LowerError::Unsupported`
  reason (or the verus failure class) is carried to the score's transparency
  surface (the survivor record / diagnostic note), so an operator can
  distinguish "proved distinguishing" from "the probe could not ask the
  question". The polarity table: VERIFIED → exclude; counterexample/timeout →
  counted; `Unsupported`/illegal form/solver error → counted + recorded reason;
  environment/VIR failure → `ForgeError` (R-CODE-4, unchanged). Source: `goal.md`
  R-HONEST-3 (no silent discharge), R-CODE-4; the #269 orchestrator ruling.
  **NOT-STARTED — blocker #269.**

## Amendment (#269): the call-bearing-body gap — grounding

### The live failure (the Arc-1 build, uncommitted at amendment time)

`mutation-scoring.md` REQ-9's F-IDENT family (built, uncommitted) generates the
identity mutant `return x` for the §9 composition fixture
(`conformance/composition/cases.json`, `verifies_to_boundary` /
`direct_boundary_caller`, quoted verbatim):

```text
#[boundary("ext::ext_id")] fn ext_id(x: u32) -> u32 req x < 100 ens result == x fx pure ;
fn caller(x: u32) -> u32 req x < 100 ens result == x fx pure { ext_id(x) }
```

The mutant `return x` PROVES against `caller`'s contract (`result == x` holds
trivially) → SURVIVOR. It is GENUINELY equivalent to the real body *under
`ext_id`'s assumed contract* (`ens result == x` pins `ext_id(x) == x`). But the
shipped probe cannot prove it: `lower_equivalence_obligation`
(`thermite-lower/src/lower.rs`) renders BOTH bodies via
`render_body_as_spec_value`, whose tail arm lowers the call through `lower_expr`
(the generic `Expr::Call` render — no callee-existence gate) and emits, by its
`writeln!` template (hand-derived from the symbol, not a pasted run):

```verus
use vstd::prelude::*;
verus! {
spec fn equiv_real_caller(x: u32) -> u32 { (ext_id(x)) as u32 }
spec fn equiv_mut_caller(x: u32) -> u32 { (x) as u32 }
proof fn equiv_check_caller(x: u32)
    requires x < 100,
    ensures equiv_mut_caller(x) == equiv_real_caller(x),
{}
}
fn main() {}
```

`ext_id` is UNDECLARED in the unit — the consumer's own doc comment pins the
omission: "The obligation is SELF-CONTAINED … so no §9/ADT composition deps are
woven" (`check::equivalence_proves_equal`). Verus rejects the unit (an undeclared
callee in spec position — an illegal spec form), `run_verus` yields a non-Proved
outcome, `mutant_outcome_is_survivor` is false, and the probe returns
`Ok(false)`: CONSERVATIVE (no laundering — the R-DEFER-9 line held) but FALSE-
GATING — the survivor stays counted, `caller`'s ratio falls below the floor,
`caller` gates L0 `WeakContract`, and TWO `forge/tests/composition_conformance.rs`
tests go red (`direct_boundary_caller_verifies_through_the_contract` and
`transitive_boundary_caller_weaves_real_and_external_body_deps` — the latter via
the same identity survivors on `g { ext_id(x) }` and `h { g(x) }` in the
`transitive` fixture).

This is the #101 bug pattern resurfacing one level up: an honest contract
falsely flagged `WeakContract` by an equivalent mutant the probe cannot
recognize — except here the probe's QUESTION (verbatim spec-render) is malformed
for the body class, not merely unproved.

### The decided obligation shape (the ruling, grounded)

The machinery to reuse exists and is SHIPPED: `check::item_subprogram` weaves
"a `#[boundary]`/`#[slag]` reachable fn … as a `#[verifier::external_body]`
signature" and a regular reachable fn "with its REAL body (fully lowered +
proved)" into the caller's sub-program; `lower_external_body_fn`
(`thermite-lower/src/lower.rs`) emits the assumable signature (unweakened
`requires`/`ensures`, `{ unimplemented!() }` body verus never checks). The
REQ-7 harness puts the SAME woven closure in front of an exec-position
comparison fn, so verus answers the equivalence question with the SAME call-site
semantics the caller's own L3 proof used. For the direct fixture the harness is
(hand-derived to the REQ-7 template; GROUNDING against real verus is the
builder's landing obligation, R-CHAR-3):

```verus
use vstd::prelude::*;
verus! {
#[verifier::external_body]
fn ext_id(x: u32) -> (result: u32)
    requires x < 100,
    ensures result == x,
{ unimplemented!() }

fn equiv_check_caller(x: u32) -> (eq: bool)
    requires x < 100,
    ensures eq,
{
    let real: u32 = { ext_id(x) };
    let mutant: u32 = { x };
    real == mutant
}
}
fn main() {}
```

`ext_id`'s assumed `ensures` pins `real == x` at the call site → `eq` proves →
the identity survivor is a TRUE equivalent (modulo the contract the whole §9
edifice already trusts) → excluded → `caller` certifies L3 again. Under the
WEAK-callee variant (`ens result <= 100`) nothing pins `real == x` → `eq`
unprovable → counted survivor → still gates (REQ-8).

### v1 scope (decided)

v1 covers EXACTLY what the composition fixtures need:

- **IN:** call-bearing bodies whose calls resolve to IN-FILE fns —
  boundary/slag callees woven as `lower_external_body_fn` external_body
  signatures, REGULAR in-file callees woven with their full lowered defs (their
  contracts govern the harness call site identically; verus is modular — the
  call site sees only the `ensures` either way). TRANSITIVE chains are in
  scope by construction: the closure is `reachable_fn_deps`, the same walk the
  L3 path uses (`h → g → ext_id` weaves `g` real + `ext_id` external_body, as
  `item_subprogram` already does for `h` itself). Scalar params/returns and the
  existing body-shape gates are retained.
- **OUT (counted-survivor fallback via REQ-9):** calls to generated free fns /
  combinators / string-method receivers inside the compared bodies (OQ-4);
  non-scalar params/returns (OQ-1, unchanged); effectful bodies (OQ-2,
  unchanged).

## Acceptance criteria

ACs tie to a `conformance/mutation/` oracle (authored by the orchestrator);
expected verdicts are hand-derived from §7 (R-CHAR-3), GROUNDED against real
Verus (`0.2026.05.24.ecee80a`) below in *Ground the path*. AC-6..AC-9 (#269)
are hand-derived from §7+§9; their real-verus grounding is the landing
obligation.

- **AC-1 (a forced-output fn → the equivalent mutant excluded → certifies; was
  depressed):** the fixture `clamp_zero` (`req x == 0`, `ens result == 0`,
  body `{ let y: u64 = x + 0; y }`) scores `1/3` BEFORE this component (the
  early-`return 0` and the `x - 0` binop-flip survivors are both proved equal to
  the real body under `x == 0`; only the `x + 1` off-by-one is killed) →
  `WeakContract`. With exclusion, both proved-equivalent survivors drop from the
  denominator → `1/1 = 1.0 >= 0.60` → CERTIFIES L3 with `mutants_killed = "1/1"`.
  The verdict FLIP from `WeakContract` to certify is the #101 fix.

- **AC-2 (a genuinely-weak contract → STILL `WeakContract`; not laundered):** the
  fixture `loose` (`req x <= 100`, `ens result <= 1000`, body `{ let y: u64 =
  x + 0; y }`) has a SURVIVING early-`return 0` mutant that is NOT equivalent to
  the real body (under `x <= 100`, `0 != x` for `x = 5`), so its equivalence
  query FAILS → it STAYS counted → the score remains below floor → `WeakContract`
  (`mutants_killed = "0/2"`, survivor reported). (The `x - 0` arithmetic-flip
  mutant IS proved-equivalent — `x - 0 == x + 0` for all `x`, independent of
  `req` — and is soundly excluded, dropping the denominator 3 → 2; the
  DISTINGUISHING `return 0` survivor is what keeps the verdict `WeakContract`.)
  The exclusion does NOT launder it (R-DEFER-9).

- **AC-3 (the exclusion is Verus-PROVED, no heuristic):** a mutant is excluded
  ONLY when the equivalence query VERIFIES (`success: true, errors: 0`); a
  counterexample or timeout never excludes. A unit/conformance test asserts the
  exclusion decision is the Verus verdict, not a syntactic shape match: the
  `return 0` mutant is excluded ONLY under `req x == 0` (which makes `0 == x`)
  and STAYS counted under the looser `req x <= 100` (AC-2), whereas the `x - 0`
  flip is excluded under ANY `req` (it equals `x + 0` for all `x`) — the verdict
  tracks provable equivalence per precondition, not the mutant's shape.

- **AC-4 (determinism — R-CODE-5):** scoring the SAME forced-output fixture twice
  yields the byte-identical reduced `mutants_killed` and the same exclusion set.

- **AC-5 (the `0/0` backstop survives exclusion):** a forced-output fn ALL of
  whose mutants are proved equivalent and none killed (the degenerate `refuse(x)
  req x == 0 ens result == 0 { x }`, whose sole early-`return 0` mutant is its only
  scored mutant and is proved equivalent) reduces to `0/0` → the #48 backstop
  STILL gates it (`kill_ratio == 0.0` < floor) — exclusion never opens a vacuous
  `1.0` pass for a fn the battery could not exercise.

- **AC-6 (#269 — the direct composition caller restored):** under the F-IDENT
  battery, the `direct_boundary_caller` fixture's `caller` has its identity
  survivor `return x` PROVED equivalent THROUGH `ext_id`'s assumed contract
  (REQ-7 harness) → excluded (`MutationScore.equivalent` rises; the exact
  killed/scored tally is tool-computed at landing, mutation-scoring.md OQ-10's
  stance) → `caller` certifies L3 with `assurance_scope = to_boundary` →
  `forge/tests/composition_conformance.rs::
  direct_boundary_caller_verifies_through_the_contract` is GREEN again.

- **AC-7 (#269 — the transitive chain restored):** the `transitive_boundary_caller`
  fixture's `g` (body `{ ext_id(x) }`, probe weaves `ext_id` external_body) and
  `h` (body `{ g(x) }`, probe weaves `g` real + `ext_id` external_body — the
  `reachable_fn_deps` closure) BOTH have their identity survivors proved
  equivalent through the contracts → excluded → both L3 →
  `transitive_boundary_caller_weaves_real_and_external_body_deps` GREEN. This is
  the regular-callee arm of REQ-7 (g's contract governs h's harness call site
  the same way ext_id's governs g's).

- **AC-8 (#269 — the WEAK-callee conservatism fixture, REQ-8):** the new fixture
  `wcaller` — `#[boundary("ext::ext_weak")] fn ext_weak(x: u32) -> u32 req
  x < 100 ens result <= 100 fx pure ; fn wcaller(x: u32) -> u32 req x < 100 ens
  result <= 100 fx pure { ext_weak(x) }` — has an identity survivor `return x`
  (it proves: `x < 100 ⟹ x <= 100`). Its harness's `eq` is UNPROVABLE
  (`ext_weak`'s `ens result <= 100` does not pin `real == x`) → NOT excluded →
  the survivor STAYS counted and is reported → `wcaller` gates `WeakContract`
  (the contract genuinely fails to pin the behavior through the callee's weak
  contract). Never a false exclusion.

- **AC-9 (#269 — structured `Unsupported`, REQ-9):** a survivor whose body the
  obligation renderer cannot ask about (an out-of-scope shape) is (a) NEVER
  excluded and (b) carries the structured reason to the transparency surface —
  a test asserts the not-excluded polarity AND that the reason is recorded, not
  silently collapsed into the proved-distinguishing bucket.

## Architecture

The equivalence check is a new seam in `forge/src/mutation.rs` (the equivalence-
obligation formulation) consumed by `check::mutation_score` in `check.rs` (it
weaves the obligation as a per-item sub-program, lowers via the EXISTING
`thermite_lower::lower`, content-addresses through `cache.rs`, and runs the
EXISTING `run_verus`). It depends on `thermite_syntax` (the `FnItem` whose `req`
+ params + return type frame the obligation, the real `body`, and the survivor's
`body`), `thermite_lower::lower` (reused), the `check.rs` Verus driver (reused),
and `cache.rs` (the `CHECK_SCHEMA_VERSION` bump + per-query content addressing).

### The equivalence obligation (the formulation)

Given the function `f` (its `req`, params, return type), its real body `B_real`,
and a survivor mutant's body `B_mut`, the equivalence obligation asks Verus to
prove that, under `req`, `B_mut`'s observable result equals `B_real`'s for all
inputs. **Call-free bodies (SHIPPED, #101):** hand-derived to spec form (the
GROUNDED query below): two `spec fn`s — one per body — over the same parameters,
with a `proof fn` that `requires <req>` and `ensures B_mut(params..) ==
B_real(params..)`. **Call-bearing bodies (NOT-STARTED, REQ-7, #269):** the
exec-position harness with the `item_subprogram`-style woven callee closure —
the equivalence notion is modulo callee contracts (§9), because the verification
semantics itself never sees past a callee's contract. A VERIFIED result is a
PROOF of observable equivalence (REQ-2 → exclude); a postcondition-not-satisfied
result is a distinguishing input (REQ-3 → stays counted); a timeout, an
unprovable-through-contracts query, or an un-askable obligation is the
conservative stay-counted reading (REQ-2/REQ-8/REQ-9).

### Data flow (the refined §7 step 4)

```text
mutation_score, per mutant Verus-classified SURVIVED:
  build the equivalence obligation (B_mut, B_real, f.req, f.params)        (REQ-1)
    call-free  -> the spec-fn pair (SHIPPED #101 form)
    call-bearing -> the exec harness + reachable_fn_deps woven closure     (REQ-7, NOT-STARTED)
  lower -> cache::cache_key                                                (REQ-1/REQ-6, reuse)
  load? else run_verus + store                                             (REQ-6, #8 cache)
    VERIFIED        -> PROVED equivalent -> drop from denominator           (REQ-2: scored -= 1, not a survivor)
    counterexample  -> distinguishing    -> STAYS a counted survivor        (REQ-3)
    timeout / weak-callee-unprovable -> unproven -> STAYS counted           (REQ-2/REQ-8, conservative)
    Unsupported / illegal form / solver error -> STAYS counted + reason     (REQ-9, R-HONEST-3)
  kill_ratio = killed / scored   (scored is the REDUCED denominator)        (REQ-4)
```

### Why this cannot launder a weak contract (the soundness line)

A `WeakContract` verdict means a mutant survived that the contract should have
killed — i.e. a body DIFFERENT from the real body that the `ens` nonetheless
admits. That mutant DIFFERS from the real body, so its equivalence query has a
distinguishing input and FAILS to prove (REQ-3, GROUNDED below: the `x + 1`
off-by-one and the `loose` early-`return 0` both fail equivalence). The exclusion
can only ever remove mutants the prover certifies are indistinguishable from the
truth — those were never evidence of weakness. The denominator a genuinely-weak
contract is scored against is unchanged (`goal.md` R-DEFER-9). The #269 modulo-
contracts extension preserves the line: a proof through callee contracts assumes
ONLY what the caller's own L3 cert already assumes (§9 — the callee honors its
contract, boundary-trusted or L3-proved), and an unprovable query — including
one unprovable BECAUSE the callee's contract is weak — always keeps the survivor
(REQ-8).

## Verification

- `cargo test -p forge` — unit tests for the equivalence-obligation formulation
  (the spec-fn pair + the `requires req / ensures B_mut == B_real` shape), the
  exclude/keep decision over synthetic Verus verdicts (VERIFIED → exclude,
  counterexample → keep, timeout → keep, AC-3), and the determinism property
  (AC-4). *(#269: plus the harness form — the woven-closure shape, the
  block-value body render, the REQ-9 structured-reason carry — expectations
  derived from this doc, never from the seam's output.)*
- `forge/tests/mutation_conformance.rs` (extended) — the conformance oracle over
  `conformance/mutation/` runs real scoring (real Verus): the forced-output
  `accept` fixture (`clamp_zero` → certifies `1/1`, AC-1), the genuinely-weak
  `reject` fixture (`loose` → `WeakContract` `0/2`, AC-2), and the `0/0`-backstop
  `reject` fixture (`refuse` → `WeakContract` `0/0`, AC-5). Expected verdicts are
  hand-derived from §7 (R-CHAR-3).
- *(#269)* `forge/tests/composition_conformance.rs` — the two §9 oracles GREEN
  again under the F-IDENT battery (AC-6/AC-7); the orchestrator-authored
  WEAK-callee fixture (AC-8) and the structured-`Unsupported` assert (AC-9) land
  in the same arc as the REQ-7 builder change (`mutation-scoring.md`'s
  mandatory one-arc landing order — never a red-main window).
- `cargo clippy -p forge --all-targets -- -D warnings`, `cargo fmt --check`.

## Route to add (orchestrator, NOT this component)

`forge/src/mutation.rs` already routes to `.design/forge/mutation-scoring.md`.
This refinement co-governs the same file; the orchestrator adds the additional
design path to the existing route (the spec-discipline hook reads each route's
`design`):

```toml
[[route]]
crate_pattern = "forge/src/mutation.rs"
design = ".design/forge/mutation-scoring.md"
# add:
# design = ".design/forge/equivalent-mutants.md"
reference = ["conformance/mutation"]
```

## Ground the path (real Verus, `0.2026.05.24.ecee80a`; real `forge check`)

The full #101 path was ground end-to-end against the real `forge` binary and the
real `verus` binary. (Scratch fixtures were under `/tmp`; removed.)

### 1. The bug — a forced-output fn is falsely `WeakContract` (BEFORE)

`refuse(x: u64) -> u64  req x == 0  ens result == 0  { x }` — `forge check`:

```text
item: refuse
level: L0
reject: WeakContract — §7 step 4 ... mutation kill ratio 0/1 is below the floor;
        mutant `insert early `return 0` at body head` survived ...
```

`clamp_zero(x: u64) -> u64  req x == 0  ens result == 0  { let y: u64 = x + 0; y }`:

```text
item: clamp_zero
level: L0
reject: WeakContract — ... mutation kill ratio 1/3 is below the floor;
        mutant `insert early `return 0` at body head` survived ...
```

Both honest (the `ens` pins `result == 0`, which the real body provably satisfies
under `req x == 0`), both falsely gated.

### 2. The equivalence proof — the survivors ARE equivalent (Verus VERIFIED)

`clamp_zero`'s three mutants under `req x == 0`: `x + 1` (off-by-one) is KILLED;
the early-`return 0` and the `x - 0` (binop flip) SURVIVE. Both survivors' bodies
are observably equal to the real body `x + 0` under `x == 0`:

```rust
spec fn real_body(x: u64) -> u64 { (x + 0) as u64 }
spec fn mut_early(x: u64) -> u64 { 0 }
spec fn mut_sub(x: u64) -> u64 { (x - 0) as u64 }
proof fn equiv_early(x: u64) requires x == 0, ensures mut_early(x) == real_body(x) {}
proof fn equiv_sub(x: u64)   requires x == 0, ensures mut_sub(x)   == real_body(x) {}
```
```text
verification results:: 2 verified, 0 errors
```

Both survivors are PROVED equivalent → excluded from the denominator (REQ-2). The
`refuse` sole survivor likewise verifies:

```rust
spec fn real_body(x: u64) -> u64 { x }
spec fn mutant_body(x: u64) -> u64 { 0 }
proof fn equivalence_under_req(x: u64) requires x == 0, ensures mutant_body(x) == real_body(x) {}
```
```text
verification results:: 1 verified, 0 errors
```

### 3. AFTER exclusion — the honest ratio certifies

- `clamp_zero`: `1/3` → exclude the 2 proved-equivalent survivors → `1/1 = 1.0`
  `>= 0.60` → CERTIFIES L3, `mutants_killed = "1/1"` (AC-1).
- `refuse`: `0/1` → exclude the sole proved-equivalent survivor → `0/0` → the #48
  backstop STILL gates `WeakContract` (AC-5 — exclusion never opens a vacuous
  pass; the fn is genuinely unscoreable).

### 4. The soundness line — a distinguishing survivor STAYS counted (NOT laundered)

The KILLED `x + 1` mutant is NOT equivalent (the equivalence query FAILS):

```rust
spec fn real_body(x: u64) -> u64 { (x + 0) as u64 }
spec fn mut_offbyone(x: u64) -> u64 { (x + 1) as u64 }
proof fn equiv_offbyone(x: u64) requires x == 0, ensures mut_offbyone(x) == real_body(x) {}
```
```text
verification results:: 0 verified, 1 errors        (postcondition not satisfied)
```

A genuinely-WEAK contract stays `WeakContract`. `loose(x: u64) -> u64  req x <= 100
ens result <= 1000  { let y: u64 = x + 0; y }` — `forge check`:

```text
item: loose
level: L0
reject: WeakContract — ... mutation kill ratio 0/2 is below the floor;
        mutant `insert early `return 0` at body head` survived ...
        (the `x - 0` flip was proved-equivalent and excluded, 3 → 2)
```

Its early-`return 0` survivor's equivalence query FAILS under the looser `req`
(the SAME mutant that was excludable under `x == 0` is NOT excludable under
`x <= 100`, AC-3 — the decision is the Verus verdict, not a syntactic match):

```rust
spec fn real_body(x: u64) -> u64 { (x + 0) as u64 }
spec fn mut_early(x: u64) -> u64 { 0 }
proof fn equiv_early(x: u64) requires x <= 100, ensures mut_early(x) == real_body(x) {}
```
```text
verification results:: 0 verified, 1 errors        (postcondition not satisfied)
```

So `loose` stays `0/2` → STILL `WeakContract` (the distinguishing `return 0`
survivor keeps it below floor; only the genuinely-equivalent `x - 0` flip was
excluded). The exclusion narrows the denominator ONLY by prover-certified-
indistinguishable mutants (REQ-3, R-DEFER-9).

## Open questions

- **OQ-1 (equivalence over richer return types):** the GROUNDED cases are scalar
  (`u64`) returns where observable equality is value equality. For
  reference/slice/`Vec`/`String` returns (the #48/#74/#80 early-return classes)
  the observable-equivalence obligation is structural equality of the returned
  value (`==` over the lowered wrapper); the formulation generalizes (the spec-fn
  pair returns the wrapper type and `ensures` its `==`), but only scalar returns
  are GROUNDED here. **(Least confident — see report.)**
- **OQ-2 (effectful bodies):** v0.1 mutation scores `fx pure` exec bodies; an
  effectful body's "observable result" would also include its effect trace, not
  just the return value. The corpus forced-output fns are `pure`, so value
  equality is the full observable. A non-pure forced-output fn is out of v0.1
  scope (effects subsume at compile time, §4).
- **OQ-3 (the per-survivor cost vs. the cap):** the cost is one Verus run per
  SURVIVOR, bounded by `MUTANT_CAP` and the proof cache. For a contract with many
  survivors (a very weak contract) the equivalence sweep runs once per survivor —
  but a very weak contract gates `WeakContract` regardless of the sweep's outcome
  (the survivors that fail equivalence keep it below floor), so the sweep is
  wasted work only on a pathologically-weak contract. Bounded, deterministic,
  acceptable (§11 accepts slow verification).
- **OQ-4 (#269 — non-fn-item calls in a compared body):** v1 REQ-7 weaves only
  the in-file `Item::Fn` closure (`reachable_fn_deps` — boundary/slag external
  sigs + regular defs). A body whose value depends on a GENERATED free fn
  (`parse_u64`-class), a combinator/scheme call, or a string/Vec method receiver
  needs the corresponding lowered defs woven (the full `item_subprogram`
  ingredients: spec-fn deps, ADT decls, generated defs). DECISION DEFERRED: v1
  returns structured `Unsupported` for those shapes (REQ-9 — counted survivor,
  recorded reason); the composition fixtures need none of them.
- **OQ-5 (#269 — mutant-side calls whose `req` fails under the harness):** a
  mutated call argument (`ext_id(x + 1)`-class, families 2–4) may fail the
  callee's `requires` inside the harness even when the mutant SURVIVED its own
  contract check (the mutant's L3 run had the same call — if it survived, the
  req discharged; but the harness's `req` is `f`'s, identical, so this should be
  unreachable). If verus nonetheless reports a precondition failure inside the
  harness, the polarity is already safe (non-Proved → counted); whether to
  classify it as `Unsupported`-with-reason instead is a builder call under
  REQ-9's table.

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (per-survivor Verus equivalence check) | SHIPPED | The seam `thermite_lower::lower_equivalence_obligation` (`thermite-lower/src/lower.rs`, exported in `lib.rs`) renders `f`'s real body + a survivor's body into the GROUNDED `spec fn equiv_real_<n>` / `spec fn equiv_mut_<n>` + `proof fn equiv_check_<n> requires <req> ensures mut == real {}` Verus unit, REUSING the L3 exec coercions (`lower_expr` + the `(expr) as <ret>` bounded-arith coercion — a naive spec render of `x + 0` over `u64` fails `verus` with `expected u64, found int`, R-CHAR-3 no hand-emit). Consumer: `check::equivalence_proves_equal` (`check.rs`), called per SURVIVOR from `check::mutation_score`. Verified: `thermite-lower/tests/equivalence_obligation.rs` (real verus — equivalent body VERIFIES, distinguishing `x + 1` / `loose` early-return FAIL, non-scalar → `Unsupported`) + `forge/tests/equivalent_mutants_conformance.rs`. **#269 caveat (scope, not status):** the shipped form is sound ONLY for CALL-FREE bodies — a call-bearing scalar body is rendered into an ILL-FORMED self-contained unit (`render_body_as_spec_value`'s tail arm lowers a call with no callee decl woven; "no §9/ADT composition deps are woven" per `equivalence_proves_equal`'s doc), which verus rejects → `Ok(false)` → conservative but FALSE-GATING for a genuinely-equivalent survivor. RESOLVED by REQ-7/REQ-9 (the call-bearing arm is now the exec harness — the `lower_equivalence_obligation` signature gained `callee_deps: &[Item]`; a CALL-FREE body still takes this self-contained spec-fn path UNCHANGED). |
| REQ-2 (PROVED-equivalent → drop from denominator; sound-but-incomplete) | SHIPPED | `check::mutation_score` runs `equivalence_proves_equal` on each SURVIVOR; a VERIFIED query (`mutant_outcome_is_survivor`/`mutant_cert_is_survivor` true) `continue`s WITHOUT incrementing `scored` (the survivor drops from the denominator) and bumps `MutationScore.equivalent`; an unproven query (counterexample/timeout/un-renderable) increments `scored` (stays counted). Verified: `forge/tests/equivalent_mutants_conformance.rs::ac1_forced_output_excludes_equivalents_and_certifies` (`clamp_zero` 1/3 → 1/1 L3, real verus). |
| REQ-3 (soundness line — distinguishing mutant never excluded) | SHIPPED | Exclusion is gated on `equivalence_proves_equal == Ok(true)` (a Verus PROOF, `0 errors`); a counterexample/timeout/`Unsupported` returns `Ok(false)` → the survivor stays counted. Verified: `forge/tests/equivalent_mutants_conformance.rs::ac2_weak_contract_survivor_stays_counted` (`loose`'s distinguishing early-`return 0` FAILS the query → STILL `WeakContract`, NOT laundered) + the seam test's `distinguishing_offbyone_fails` / `loose_early_return_stays_distinguishing`. |
| REQ-4 (`MutationScore` denominator + `K/N` cert) | SHIPPED | `mutation::MutationScore` gains `equivalent: usize` (the proved-equivalent exclusion count); `scored` is now NET of proved-equivalents, so `kill_ratio = killed / scored` and `mutants_killed_string` reflect the REDUCED denominator. The `0/0` backstop in `kill_ratio` is unchanged (`scored == 0 ⟹ 0.0`). Verified: `clamp_zero` cert `mutants_killed = "1/1"` (AC-1); `refuse` `"0/0"` (AC-3); `loose` below floor (AC-2). |
| REQ-5 (`CHECK_SCHEMA_VERSION` bump) | SHIPPED | `cache::CHECK_SCHEMA_VERSION` bumped `4 → 5` (`cache.rs`, with the schema-history note): the verdict-changing exclusion invalidates stale forced-output verdicts so a `WeakContract` cached under schema 4 is re-scored under the new logic. *(REQ-7's landing bumps again per the same rule — the verdict changes for call-bearing fns.)* |
| REQ-6 (determinism + bounded per-survivor cost) | SHIPPED | `equivalence_proves_equal` content-addresses the obligation through the SAME `cache::cache_key`/`load`/`store` (#8) and runs the deterministic pinned-seed/rlimit `run_verus`; ONE extra run PER SURVIVOR (a killed mutant is never queried — `mutation_score` `continue`s before the query). Verified: `forge/tests/equivalent_mutants_conformance.rs::req6_exclusion_is_deterministic` (byte-identical reduced `mutants_killed` across runs). |
| REQ-7 (call-bearing obligation — equivalence modulo callee contracts, the exec harness) | SHIPPED | `thermite_lower::lower_equivalence_obligation(f, mutant_body, callee_deps)` (`thermite-lower/src/lower.rs`) routes a NON-EMPTY `callee_deps` (a call-bearing body) to `lower_call_bearing_equivalence_obligation`, which emits the EXEC-position harness `fn equiv_check_<n>(<params>) -> (eq: bool) requires <req>, ensures eq { let real_v: <ret> = { <real> }; let mutant_v: <ret> = { <mutant> }; real_v == mutant_v }` with the callee closure woven via the EXISTING `lower` dispatch (a boundary/slag dep → `lower_external_body_fn`'s `#[verifier::external_body]` signature, a regular dep → its full `lower_fn` def — the SAME weave `item_subprogram` drives, modulo callee contracts §9). Each compared body renders as an exec block VALUE via `render_body_as_exec_value` (leading early-return → its returned expr, bare tail, or immutable let-chain-plus-tail; a call is LEGAL — the closure declares every callee). Binders are `real_v`/`mutant_v` (NOT `real`, a vstd-imported type name). Consumer: `check::equivalence_proves_equal` threads the SAME `fn_deps` closure `mutation_score` weaves into each mutant's `item_subprogram`. Verified: `thermite-lower/tests/equivalence_obligation.rs::{call_bearing_obligation_emits_the_woven_exec_harness, call_bearing_identity_through_strong_contract_verifies}` (real verus — the woven ext_id external_body + the identity harness VERIFIES) + `forge/tests/composition_conformance.rs::{direct_boundary_caller_verifies_through_the_contract, transitive_boundary_caller_weaves_real_and_external_body_deps}` (AC-6/AC-7 GREEN — `caller`'s `return x` excluded → L3). |
| REQ-8 (conservatism — weak callee ⟹ NOT equivalent; never a false exclusion) | SHIPPED | The harness `ensures eq` is provable ONLY when a callee contract pins `real == mutant`. `check::equivalence_proves_equal` excludes on `EquivOutcome::Proved` ALONE (a verus PROOF of the harness); a weak callee (`ens result <= 100`) leaves `eq` unprovable → `EquivOutcome::NotProved` → the survivor STAYS counted, the item gates. Verified: `thermite-lower/tests/equivalence_obligation.rs::call_bearing_identity_through_weak_contract_fails` (the `ext_weak` harness FAILS verus) + `forge/tests/composition_conformance.rs::weak_callee_identity_survivor_stays_counted_and_gates` (`wcaller` gates `WeakContract 0/2` — BOTH the identity and zero-return survivors counted, never falsely excluded). |
| REQ-9 (structured `Unsupported` / probe-failure transparency — R-HONEST-3) | SHIPPED | The consumer no longer collapses every render error to bare `Ok(false)`: `check::equivalence_proves_equal` returns `EquivOutcome::{Proved, NotProved, Unsupported(String)}`, and `mutation_score` carries an `Unsupported` reason to the survivor transparency surface (`survivor = "<desc> (equivalence probe Unsupported — survivor COUNTED, not excluded: <reason>)"`) so an operator distinguishes "proved distinguishing" from "the probe could not ask". EXCLUSION fires on `Proved` ALONE; an out-of-scope shape (`LowerError::Unsupported` from `lower_equivalence_obligation`) keeps the survivor COUNTED with the reason recorded. Verified: `thermite-lower/tests/equivalence_obligation.rs::non_scalar_return_is_unsupported` (the seam returns structured `Unsupported`, no panic) + the `EquivOutcome::Unsupported` arm in `check.rs`. |
