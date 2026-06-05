# Forge strengthening probes (suggest tighter contracts)

<!--
tier: 3-component
status: draft
governs: forge/src/strengthen.rs
thesis-refs:
  - thermite-design.md §7
  - thermite-design.md §4.1
  - thermite-design.md §4.2
  - thermite-design.md §5.1
-->

## Summary

`forge/src/strengthen.rs` is **§7 step 5** of the vacuity battery: given a `fn`
whose REAL body already proved **L3** but whose contract is **WEAK** (mutation
scoring, #12, found one or more SURVIVORS — behavior the `ens` does not pin), it
generates a FROZEN, DETERMINISTIC, BOUNDED set of CANDIDATE stronger `ens`
clauses, VERIFIES each against the real body by reusing the existing verus driver
(`check::run_verus`), and SURFACES the candidates that (a) VERIFY against the body
and (b) are strictly STRONGER than the current `ens` as adoptable SUGGESTIONS
(*"consider strengthening `ens` with `<clause>` — it holds for your body and would
kill survivor `<M>`"*). This is `thermite-design.md` §7's "template-based
tightenings of `ens` … if a strictly stronger contract proves with no body change,
Forge suggests it" (§7 step 5), and the made-concrete form of §7 line 224's
"a precise prompt for strengthening" / line 227's "the residue surfaced for
review".

Strengthening probes are **ADVISORY, not a gate**. Unlike #12's kill-ratio floor,
a probe NEVER changes the certification verdict: a `fn` that certifies L3 + meets
the mutation floor STILL certifies, with the suggestions surfaced in an ADDITIVE
cert field / the structured output (the `suggested_move` slot and a new additive
`strengthening` field, §5.1 "every message is a prompt"). A suggestion is emitted
ONLY when it both VERIFIES (so it is adoptable with no body change) and is strictly
stronger than the current `ens` (so it actually narrows the allowed outputs / would
have killed a #12 survivor). This is the anti-Goodhart escape hatch (`goal.md`
R-DEFER-9): the probe helps the agent climb OUT of a weak-but-true contract toward
one that pins behavior — it never lets it certify a weaker one.

GREENFIELD — no `strengthen.rs` exists. **All REQs NOT-STARTED**, blocked on
crosslink issue **#14** ("§7 step 5 strengthening probes", v0.3 battery, milestone
#3). The load-bearing prerequisites all ship and are what this component composes:
`forge check` (#5, `check::check_file_with_options`), the per-item verus driver
(#5, `check::run_verus` + `classify_verus_outcome`, both `private` in `check.rs`),
mutation scoring (#12, `mutation::MutationScore` / `mutation::generate` — the
SURVIVORS are the input), the proof cache (#8, `cache::cache_key`/`load`/`store`),
the SpecTherm clause AST (`thermite_syntax::{Expr, Clause, Contract, BinOp}`), the
lowerer (#4, `thermite_lower::lower`), and the cert schema with the reserved
`manifest::SuggestedMove` slot. Real verus is at `~/.local/bin/verus`
(`0.2026.05.24.ecee80a`); the GROUNDING below ran against it and PROVES the
candidate-verify-against-body mechanism is mechanical, not vaporware.

## Scope boundaries (documented, attributed)

- **IN:** exactly §7 step 5 — given a `fn` that proved L3 (and, primarily, whose
  #12 score reports a survivor), generate a frozen bounded candidate set of
  stronger `ens` clauses, verify each against the real body via `run_verus`, keep
  the ones that VERIFY and are strictly STRONGER, and surface them as advisory
  suggestions. Tie a suggestion to the #12 survivor it would kill where derivable.
- **OUT — mutation scoring** (§7 step 4, #12, `mutation.rs`) is the INPUT: it
  REPORTS which mutants survived (the `MutationScore.survivor` "precise prompt").
  #14 turns that prompt into adoptable CLAUSE suggestions. #14 never re-scores; it
  consumes #12's already-computed survivors.
- **OUT — changing the verdict.** A probe is ADVISORY. It does NOT gate
  certification, does NOT degrade a level, and does NOT turn a passing cert into a
  reject. (The kill-ratio FLOOR — the gate — is #12's job, not this component's.)
- **OUT — mutating the body or the `req`/`fx`.** A probe only proposes a stronger
  `ens` CONJUNCT for the SAME body and the SAME `req`/`fx`. "Strictly stronger"
  means added/tightened postcondition; weakening `req` or touching the body is out.
- **OUT — the background proof-repair loop** (#18, v0.5) and the **critic-model
  spec-intent review** (#19, v0.5). A probe SURFACES a suggestion; it does not
  auto-adopt it, does not re-run a repair loop, and does not judge spec INTENT
  ("is this what the user meant?" stays the §7 line 227 review slot's job).
- **OUT — general clause synthesis.** Candidates come from a FROZEN template
  (below), deterministic + capped. No search, no solver-guided synthesis, no
  LLM generation — those would break determinism (R-CODE-5) and the ≤6k skill
  budget's "one way to do everything".

## Requirements

- **REQ-1 (frozen, deterministic, bounded candidate-clause template).** A pure
  function of the `FnItem` (+ the file's `spec fn`s + the #12 survivors) produces a
  DETERMINISTIC, ORDERED, CAPPED list of candidate `ens` clauses (`Expr`s over
  `result` and the parameters). The template families (the mechanical core, all
  derived from what is IN SCOPE — no invented vocabulary) are, in a FIXED family
  order:
  1. **spec-fn equality** — for each `spec fn` `s` in scope whose parameter types
     match the `fn`'s parameters (in order) and whose return type matches the
     `fn`'s return type, the clause `result == s(<params>)` (e.g. `result ==
     spec_sum(xs)`). This is the §4.2 "spec functions are executable" pinning form.
  2. **result-equals-input-expression** — `result == <e>` for each small
     in-scope expression `e` of the return type drawn from a FROZEN expression
     grammar over the parameters: a bare parameter `p`; a binary combination of two
     scalar parameters under the frozen `{+, -, *}` set with the result type
     (`result == a + b`); a `len()` of a slice parameter for an integer return
     (`result == xs.len()`). The grammar is bounded to depth 1 (one operator), so
     the candidate count is bounded by the parameter count.
  3. **tighter bound derived from the survivor** — when a #12 survivor is a
     scalar early-return (`return 0` / `return false` / `return None`) and the
     current `ens` is a loose inequality `result <= N` (or `result >= N`), the
     candidate that the survivor VIOLATES: a tighter equality/inequality the real
     body satisfies but the early-return value does not (e.g. survivor `return 0`
     against `result <= 1000000` → candidate `result == a + b`, which `0` fails).
  - The list is bounded by a documented `pub const CANDIDATE_CAP` (OQ-2). When the
    candidate count exceeds the cap, the first `CANDIDATE_CAP` candidates in the
    fixed family order are taken (the same order-prefix selection #12's
    `MUTANT_CAP` uses). A pure function of the inputs ⇒ the same ordered list every
    run (REQ-6).

- **REQ-2 (verify each candidate against the REAL body, reusing `run_verus`).**
  Each candidate `ens` is woven into a COPY of the `fn` (the body UNCHANGED, the
  `req`/`fx` UNCHANGED, the candidate clause REPLACING-or-ADDED-TO the `ens`),
  lowered via `thermite_lower::lower` of the same per-item sub-program shape
  (`check::item_subprogram`), and re-verified through the EXISTING verus driver
  (`check::run_verus`). A candidate that verus PROVES against the real body HOLDS
  (it is adoptable with NO body change — §7 step 5's "proves with no body change").
  A candidate that verus does NOT prove (counterexample / timeout) is DISCARDED —
  never suggested (no unadoptable/over-strong suggestions, R-DEFER-1). A candidate
  that fails to LOWER is DISCARDED (parallel to #12's drop-from-denominator). An
  ENVIRONMENT / VIR failure surfaces a `ForgeError` (R-CODE-4), never a silent
  discard masquerading as a verdict.

- **REQ-3 (strictly-stronger filter).** A verifying candidate is suggested ONLY if
  it is strictly STRONGER than the current `ens` — it narrows the allowed outputs.
  v0.1 establishes "strictly stronger" mechanically via the verus driver: the
  candidate `C` is strictly stronger than the current conjunction `E` iff
  `E && body-facts ⊬ C` is NOT already implied (i.e. `C` adds a constraint `E`
  did not), confirmed by the survivor link in REQ-1 family 3 (`C` REJECTS the
  survivor body that `E` accepted — so `C` is provably stronger on that witness),
  OR by the structural test that `C` is `result == <e>` while the current `ens`
  is only an inequality / does not mention `result` as an equality (an equality
  strictly narrows a satisfiable range). A candidate that is logically EQUAL to or
  WEAKER than the current `ens` (it would not have killed any survivor and does not
  add an equality) is DISCARDED — not suggested.

- **REQ-4 (advisory placement — additive cert field / output, NOT a gate).** A
  surviving-the-filter candidate is rendered into the certificate as an ADDITIVE,
  oracle-EXCLUDED `strengthening` field (a list of `Suggestion { clause, kills_survivor }`)
  and into the reserved `suggested_move` slot (§5.1 "trigger hints" / "every
  message is a prompt"). The cert's `level`, `reject`, and `contract_quality`
  oracle subset are UNCHANGED by a probe (a probe is advisory): a `fn` that
  certified L3 still certifies L3 with the same oracle subset, now carrying
  suggestions. An item with NO surviving candidate carries NO suggestion (an honest
  absence, not a placeholder — mirrors the `suggested_move: None` precedent).

- **REQ-5 (consumes #12 survivors; runs only on a settled L3 + scored item).** The
  probe runs in `check::check_file_with_options`'s per-item L3 path AFTER mutation
  scoring (#12), ONLY when the item is `Level::L3` with `reject.is_none()` AND
  mutation scoring produced a `MutationScore` (so there is something to strengthen
  toward). A `WeakContract` REJECT (#12 sub-floor) is NOT a probe target in v0.1 —
  a rejected item already carries the survivor prompt; the probe targets a
  certified-but-improvable contract (the §7 step-5 "strictly stronger contract
  proves" case). The survivors feed REQ-1 family 3 candidate generation: a
  survivor `desc` (e.g. `"insert early `return 0` at body head"`) maps to the
  tighter-bound candidate that survivor violates.

- **REQ-6 (determinism — same fn ⇒ same suggestions).** `generate_candidates` is a
  pure function of the AST + the file's `spec fn`s + the #12 survivor set + the
  frozen template, and each candidate's verdict is the SAME deterministic verus run
  (pinned seed + rlimit) the L3 path + proof cache rely on (each candidate
  content-addressed via `cache::cache_key`, #8). So the emitted suggestion list is
  byte-stable across runs (R-CODE-5). The suggestions are DIAGNOSTIC and
  verus-version-sensitive, so they are oracle-EXCLUDED (parallel to
  `solver_profile` / `mutants_killed`, OQ-3).

## Acceptance criteria

The orchestrator authors a `conformance/strengthening/` oracle (cases.json,
QUALITATIVE per R-CHAR-3 — the exact suggested clause set is tool/verus-version-
sensitive, so the checkable property is the PARSE-VERIFIED presence/absence of an
adoptable suggestion, not a byte-exact clause string). The fixtures + expected
outcomes below are the AC anchors.

- **AC-1 (a weak contract with a survivor → ≥1 adoptable strengthening
  suggestion).** For the EXACT weak fixture
  `fn f(a: u32, b: u32) -> u32 req a <= 10 && b <= 10 ens result <= 1000000 fx pure { a + b }`,
  the probe emits ≥1 suggestion whose clause VERIFIES against the body and is
  strictly stronger. The EXPECTED (PARSE-VERIFIED) suggestion is
  **`ens result == a + b`** — it VERIFIES against `{ a + b }` (grounded below) and
  would have killed the early-return-0 survivor (`0 == a + b` is false). Mechanical
  check: the suggestion list contains a clause that (i) `thermite_syntax::parse`s as
  an `ens` expression, (ii) when verified against the real body yields `Proved`,
  (iii) when verified against the `return 0` survivor body yields a non-`Proved`
  outcome (the kill link).
- **AC-2 (the corpus `sum` → NO suggestion — already tight).** For
  `conformance/sum.th` (`ens result == spec_sum(xs)`, the output fully pinned), the
  probe emits NO strengthening suggestion: every template candidate either fails to
  verify or is not strictly stronger than the existing exact-equality `ens` (an
  equality on `result` to the canonical spec fn cannot be strengthened by another
  output constraint). Mechanical check: the `strengthening` field is empty / absent
  for `sum`.
- **AC-3 (a candidate that does not verify → NOT suggested — no false/unadoptable
  suggestions).** A candidate clause that does not hold for the body (e.g. the
  over-strong `ens result == a + b + 1` against `{ a + b }`, grounded below to
  produce a verus FAILURE) is DISCARDED and never appears in the suggestion list.
  Mechanical check: a candidate whose against-body verus outcome is non-`Proved`
  contributes nothing to the output.
- **AC-4 (advisory — the verdict is unchanged).** Running the probe on the AC-1
  weak fixture (after it has CERTIFIED at a low `--mutation-floor` so it reaches the
  probe as an L3-certified item) leaves the cert's oracle subset
  `(item, level, effects, slag)` and `reject` IDENTICAL to the pre-probe cert; only
  the additive `strengthening` field / `suggested_move` slot is populated.
- **AC-5 (determinism).** `generate_candidates` over the AC-1 fixture twice yields
  the byte-identical ordered candidate-clause list (a pure function of the AST +
  survivors + template); the same fn yields the same emitted suggestions.
- **AC-6 (bounded).** `generate_candidates` over any fixture returns ≤
  `CANDIDATE_CAP` candidates.

## Architecture

```text
check::check_file_with_options (per-item L3 path)
   │  item proved L3, reject.is_none(), mutation_score met floor
   ▼
mutation::MutationScore { survivor: Some(M), .. }      ── #12 (INPUT)
   │
   ▼
strengthen::generate_candidates(f, spec_items, &score) ── REQ-1 (frozen template)
   │   ordered, capped Vec<CandidateClause>
   ▼  for each candidate (deterministic order)
strengthen::verify_candidate                            ── REQ-2
   │   weave candidate ens into a COPY of f (body UNCHANGED)
   │   item_subprogram → thermite_lower::lower → cache::load? → run_verus
   │   Proved  → HOLDS (adoptable)        ── §7 step 5 "proves with no body change"
   │   else    → DISCARD                  ── no unadoptable suggestion (R-DEFER-1)
   ▼
strengthen::is_strictly_stronger                        ── REQ-3
   │   kills a #12 survivor, OR adds an equality the current ens lacks
   ▼
strengthen::Suggestion { clause, kills_survivor }       ── REQ-4 (ADVISORY)
   │
   ▼
Certificate (level UNCHANGED) + additive `strengthening` + `suggested_move`
```

The component owns `strengthen.rs` (the candidate template + the verify/filter
pipeline) and a `manifest::Suggestion` additive cert type; `check.rs` is the
consumer (one new call in the per-item L3 path, after `mutation_score`). The
candidate `ens` `Expr`s are built from `thermite_syntax::{Expr, BinOp, Clause}`
(the same nodes the parser produces, so a candidate round-trips through the
lowerer unchanged — `lower_fn` in `lower.rs` emits the `ens` from `Contract.ens`).
The verify step reuses `check::run_verus` and `check::item_subprogram` verbatim
(per-item isolation, §5.3) — the probe introduces NO new prover invocation path,
only a new caller of the existing one.

### Why the suggestion must VERIFY against the real body (the adoptability invariant)

The §7 step-5 promise is "if a strictly stronger contract **proves with no body
change**, Forge suggests it". The verify-against-body step (REQ-2) is what makes a
suggestion ADOPTABLE rather than vaporware (`goal.md` R-DEFER-1: a suggestion must
be a REAL adoptable clause): the agent can paste the suggested `ens` into the
function and it will still certify L3, because the probe ALREADY proved it does.
A candidate that does not verify is precisely a candidate the agent could NOT adopt
without changing the body — so it is discarded, never surfaced (AC-3). This is the
asymmetry that keeps the probe honest: it only ever proposes contracts it has
itself proven hold.

### Why it is ADVISORY, not a gate (anti-Goodhart, but the right polarity)

#12's floor GATES: a weak contract does not certify. #14 SUGGESTS: a contract that
certified can be made stronger. The two compose without #14 ever moving the
verdict. The reason #14 is advisory and not a second gate: the strongest provable
`ens` is not always the one the USER wanted (the spec-intent residue, §7 line 227 /
#19). Auto-tightening could narrow a contract past intent. So the probe surfaces the
adoptable tightening as a PROMPT (§5.1 "every message is a prompt") and leaves
adoption to the agent / the review slot. This is the same trust relocation §1
describes: the probe shrinks the residue (here is a stronger contract that holds)
without making the irreducible intent call.

### Determinism and the oracle (REQ-6)

`generate_candidates` is a pure function (AST + spec fns + survivor set + frozen
template), so the candidate LIST is deterministic. Each candidate's verus verdict is
the same pinned-seed/rlimit run the L3 path uses, content-addressed via the #8 cache,
so the SET of verifying candidates is deterministic too. The emitted suggestion
clauses are DIAGNOSTIC + verus-version-sensitive (a future verus might prove a
candidate today's cannot), so they are oracle-EXCLUDED from `Certificate::oracle_subset`
(parallel to `solver_profile` and `mutants_killed`). The cert-oracle compares
`(item, level, effects, slag)` — which a probe never changes (AC-4) — so the corpus
`sum.cert.json` is unperturbed whether or not `sum` gets a suggestion (it does not,
AC-2).

## Verification

- `cargo test -p forge` — `strengthen.rs` unit tests: the frozen candidate template
  + order + cap (REQ-1, AC-6, anchored to the template families, R-CHAR-3); the
  strictly-stronger filter logic (REQ-3); determinism (AC-5); the advisory
  placement leaves the oracle subset unchanged (REQ-4, AC-4).
- `conformance/strengthening/` oracle (orchestrator-authored, R-CHAR-3 QUALITATIVE):
  AC-1 (weak fixture → adoptable suggestion that verifies + kills the survivor),
  AC-2 (`sum` → no suggestion), AC-3 (non-verifying candidate not suggested). The
  expected outcomes are FLOOR-relative / presence-absence (the exact clause string
  is verus-version-sensitive, so it is oracle-excluded; the PARSE-VERIFIED property
  is the anchor).
- The conformance cert-oracle (`tests/check_conformance.rs`) MUST still pass: a
  probe changes no oracle field, so `sum.cert.json` matches unchanged (AC-4).
- `cargo clippy -p forge --all-targets -- -D warnings`, `cargo fmt --check`.

## Route to add (orchestrator, NOT this component)

`tooling/spec-routes.toml` gains, under the `forge` block:

```toml
[[route]]
crate_pattern = "forge/src/strengthen.rs"
design = ".design/forge/strengthening-probes.md"
reference = ["conformance/strengthening", "conformance/sum.th"]
conformance_ops = ["weak_loose_bound", "corpus_sum"]
```

This component does NOT edit the route table (R-XLATE-2 is the orchestrator's
gate); the route + the `conformance/strengthening/` oracle are authored before the
builder edits `strengthen.rs`.

## Ground the probe (real verus, `0.2026.05.24.ecee80a`)

Run against `~/.local/bin/verus` to prove the candidate-verify mechanism is
MECHANICAL. The harness mirrors what `thermite_lower::lower` emits for a `fn`
(`fn f(...) -> (result: T) requires ...; ensures ...; { body }` inside `verus! {}`).

### Adoptable candidate: `ens result == a + b` VERIFIES against `{ a + b }`

The weak fixture's body is `{ a + b }`. The candidate strengthens the loose
`result <= 1000000` to the exact `result == a + b`. Harness (the candidate ADDED
as a conjunct, body unchanged):

```rust
fn f(a: u32, b: u32) -> (result: u32)
    requires a <= 10, b <= 10,
    ensures result <= 1000000, result == a + b,
{ a + b }
```

`verus --output-json --rlimit 30` → `"success": true, "verified": 1, "errors": 0`.
The candidate HOLDS for the real body ⇒ it is an ADOPTABLE suggestion (REQ-2).

### The candidate would KILL the early-return-0 survivor

The #12 survivor for this fixture is the early-return-0 mutant `{ return 0; }`,
which the loose `result <= 1000000` PROVES (`0 <= 1000000`). Verifying the
strengthened `ens result == a + b` against that survivor body:

```rust
fn f(a: u32, b: u32) -> (result: u32)
    requires a <= 10, b <= 10,
    ensures result == a + b,
{ return 0; }
```

→ `"success": false, "errors": 1`. The strengthened clause REJECTS the survivor
(`0 == a + b` is not provable) ⇒ the suggestion would have killed survivor
`"insert early `return 0` at body head"` (REQ-1 family 3 link, REQ-3 strictly-
stronger witness).

### Non-verifying candidate: `ens result == a + b + 1` is NOT suggested

An over-strong / wrong candidate that does NOT hold for the body:

```rust
fn f(a: u32, b: u32) -> (result: u32)
    requires a <= 10, b <= 10,
    ensures result == a + b + 1,
{ a + b }
```

→ `"success": false, "errors": 1`. The candidate does NOT verify against the real
body ⇒ it is DISCARDED, never surfaced (REQ-2, AC-3). This is the guard against
unadoptable / over-strong suggestions.

### `sum` (already `ens result == spec_sum(xs)`) → NO suggestion

`conformance/sum.th`'s `ens result == spec_sum(xs)` pins the output exactly. The
family-1 candidate (`result == spec_sum(xs)`) is ALREADY the contract → not
strictly stronger (REQ-3 discards it). Family-2/3 candidates over a slice-return
fold either fail to verify (no `result == xs.len()` etc. holds for a sum) or are
not strictly stronger than an exact equality. So `sum` carries NO suggestion
(AC-2): there is nothing to strengthen toward when the output is already pinned.
(Note: `sum`'s golden cert reports a `survivor` — mutant#11 `i = i + 2`, killed by
`inv#2`, above the floor — but that survivor is killed by an INVARIANT, not the
`ens`; no stronger ENS candidate both verifies and is strictly stronger, so the
probe still emits nothing. The probe targets `ens` strength, which is already
maximal here.)

### Established: the deterministic template + cap

From the grounding, the FROZEN candidate template (REQ-1) is the three families
above, generated in fixed order from in-scope material (spec fns, parameters,
the survivor), depth-1 / order-prefix bounded by `CANDIDATE_CAP` (OQ-2). The
verify step is `run_verus` verbatim; the filter is "verus-Proved against the body
AND strictly stronger (kills a survivor / adds an equality)". This is mechanical:
no search, no synthesis beyond the frozen grammar.

## Open questions

- **OQ-1 (additive cert field shape).** REQ-4 adds a `strengthening:
  Vec<Suggestion>` field (each `Suggestion { clause: String, kills_survivor:
  Option<String> }`) alongside reusing `suggested_move`. The exact field name /
  whether suggestions live ONLY in `suggested_move` vs a dedicated list is a
  schema decision for the builder + orchestrator to ratify (R-SPEC-2 — additive,
  `#[serde(default, skip_serializing_if)]` so the golden `sum.cert.json` still
  deserializes). LEANING: a dedicated `strengthening` list (a single fn can have
  multiple adoptable tightenings; `suggested_move` is a single hint) + populate
  `suggested_move` with the first suggestion as the headline.
- **OQ-2 (`CANDIDATE_CAP` value).** §7 says "budgeted" without a number. Each
  candidate is a verus run (cheap on a cache hit, #8), so the cap bounds cost.
  LEANING: a small documented `const` (≈ 16) — the corpus `fn`s produce a handful
  of candidates (one spec-fn-equality per matching spec fn + a depth-1 grammar over
  ≤ 4 params), comfortably under it. Ratified with the builder against real
  candidate counts (parallel to `mutation::MUTANT_CAP`).
- **OQ-3 (oracle exclusion of suggestions).** The grounding shows a candidate's
  verdict is verus-version-sensitive (a future verus might prove one today's
  cannot), so suggestions are oracle-EXCLUDED. This is the same call #12 made for
  `mutants_killed` and #11 for `solver_profile`. Confirmed direction; the builder
  wires `oracle_subset` to omit the field.
- **OQ-4 (does the probe target a #12 WeakContract REJECT too?).** v0.1 scopes the
  probe to L3-CERTIFIED items (REQ-5): a rejected item already surfaces the survivor
  prompt via the `WeakContract` cert. Extending the probe to ALSO suggest a clause
  on a rejected item (turning the reject's survivor into an adoptable clause) is a
  natural follow-on but is deferred to keep v0.1 advisory-on-certified only; the
  orchestrator ratifies whether #14 covers the reject case or a later issue does.
- **OQ-5 (strictly-stronger via a third verus query vs structural test).** REQ-3
  establishes "strictly stronger" via the survivor-kill witness OR a structural
  equality test, NOT a separate `E ⊬ C` solver query (which would double the prover
  cost). LEANING: the survivor-kill + structural test is sufficient for the
  template families (every family-3 candidate kills a survivor; every family-1/2
  equality strictly narrows a non-equality `ens`). A dedicated implication query is
  a possible future precision upgrade; ratified with the builder.

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (frozen deterministic bounded candidate template) | SHIPPED | `pub fn generate_candidates` in `strengthen.rs` is a pure function of the `FnItem` + the file's `spec fn`s + the #12 survivor, producing an ORDERED list in the fixed family order (family 1 spec-fn equality `result == s(<params>)`, family 2 result-equals-input-expression `result == p` / `result == a OP b` / `result == xs.len()`, family 3 survivor-derived kill link), capped by `pub const CANDIDATE_CAP = 16`. Consumer: `pub fn probe` + `check::strengthen_certificate`. Verified by `strengthen::tests::{weak_fixture_generates_result_eq_a_plus_b, spec_fn_equality_candidate_for_matching_signature, candidates_bounded_by_cap}`. |
| REQ-2 (verify candidate vs real body, reuse `run_verus`) | SHIPPED | `pub fn probe` weaves each candidate `ens` into a COPY of `f` (`candidate_fn`, body UNCHANGED) and calls the threaded verify closure; `check::strengthen_certificate` implements that closure as `item_subprogram` → `thermite_lower::lower` → `cache::load`? → `check::run_verus` (the EXISTING driver, content-addressed via #8). A non-`Proved` / un-lowerable candidate is DISCARDED; a `ForgeError` propagates (R-CODE-4). Verified live against real verus by `strengthening_conformance::weak_contract_emits_verifying_strictly_stronger_suggestion` (the surfaced `result == a + b` PROVES against `{ a + b }`). |
| REQ-3 (strictly-stronger filter) | SHIPPED | `pub fn is_strictly_stronger` keeps a verifying candidate only if it KILLS the survivor (the survivor-body verify witness — `candidate_fn` over the #12 survivor body does NOT verify the candidate) OR adds a `result ==` equality the current `ens` lacks (`current_ens_pins_result` false). No extra implication solver query (OQ-5). Verified by `strengthen::tests::{equality_is_stronger_than_non_pinning_ens, not_stronger_when_ens_already_pins_result}` + the live `result == a + b` kill of the `return 0` survivor. |
| REQ-4 (advisory placement — additive cert field, not a gate) | SHIPPED | `manifest::Suggestion` + the additive `#[serde(default, skip_serializing_if = "Vec::is_empty")] Certificate.strengthening: Vec<Suggestion>` field (oracle-EXCLUDED); `Certificate::with_strengthening` attaches them + populates `suggested_move` with the headline. `level`/`reject`/`oracle_subset` UNTOUCHED. Verified by `strengthening_conformance::{probe_never_changes_the_verdict, corpus_sum_emits_no_suggestion_and_certifies_l3}` (the golden `sum.cert.json` oracle subset is unperturbed; `check_conformance` still green). |
| REQ-5 (consumes #12 survivors; runs on a settled L3 + scored item) | SHIPPED | `check::check_file_with_options` calls `check::strengthen_certificate` AFTER `mutation_score`, ONLY in the `score.meets_floor` branch (the item is `Level::L3`, `reject.is_none()`, a `MutationScore` produced). The `MutationScore.survivor` resolves the survivor body via `mutation::generate` (same frozen mutator) for the family-3 kill witness. |
| REQ-6 (determinism — same fn ⇒ same suggestions) | SHIPPED | `generate_candidates` is a pure function of the AST + spec fns + survivor + frozen template (byte-stable list, `strengthen::tests::generate_candidates_is_deterministic`); each candidate's verus verdict is the same pinned-seed/rlimit run + #8 cache the L3 path uses. Suggestions are DIAGNOSTIC + verus-version-sensitive → oracle-EXCLUDED. Verified live by `strengthening_conformance::suggestions_are_deterministic_across_two_runs`. |
