# Forge SOLVER tautology + vacuous-precondition checks

<!--
tier: 3-component
status: draft
audited-sha: dff9ae866e3437af272a62e078993e66c1116460 (re-audited 2026-06-12: amended — shipped-status Summary + the #53 scratch-dir cleanup note, #262; amended 2026-06-12 — the SPEC-TIGHTNESS SIGNAL REQ family (REQ-8..REQ-13, NOT-STARTED, #271) + the #275 ADT-harness grounded finding)
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

SHIPPED (#13) — `forge/src/vacuity_solver.rs` implements both harness
builders, the interpretation table, and the gate entry
`vacuity_solver::solver_vacuity_check`; the REQ-status table below is the
per-REQ evidence (re-audited against the tree at the pinned SHA, #262). The
structural triage (#6, `forge/src/vacuity.rs`), `forge check` (#5), and the
verus driver (#5 `run_verus`, real verus at `~/.local/bin/verus`) are the
load-bearing prerequisites this component composes.

**AMENDMENT (#271, NOT-STARTED) — the SPEC-TIGHTNESS SIGNAL, the third member
of this query family.** An outside review (item 6) surfaced the gap the gate
checks cannot see: a contract can be non-tautological, non-vacuous, mutation-
scored — and still admit MANY results per input. Motivating finding:
`examples/editor/editor.th::move_up` is L3-proven against
`ens result.text.len() == b.text.len() && result.cursor <= b.cursor`, which the
IDENTITY body also satisfies (`result.cursor` admits both the true up-target and
`b.cursor` itself for any row ≥ 1 input — and `result.text` admits any same-
length bytes). The tightness signal is ONE extra solver query per checked `fn`
asking "can TWO DISTINCT results satisfy `ens` for the SAME `req`-satisfying
input?" — and its answer is a **REPORT-ONLY certificate field, NEVER a gate**:
many specs are intentionally relational/bounds-shaped (`binary_search` returning
ANY matching index is correct-by-design), so looseness is a review signal for
the §7 residue ("is this what I meant?"), not a defect verdict. REQ-8..REQ-13
below; all NOT-STARTED, blocker #271.

## Scope boundaries (documented, attributed)

- **IN (shipped, #13):** exactly the two SOLVER gate checks — §7 step 2
  (tautology) and §7 step 3 (vacuous / unsat precondition).
- **IN (this amendment, #271, NOT-STARTED):** the SPEC-TIGHTNESS SIGNAL — one
  additional REPORT-ONLY solver query per checked `fn` (the third member of the
  same harness/driver family), surfaced as an additive certificate field. It
  NEVER rejects, never changes a `Level`, never gates.
- **OUT — mutation scoring** (`mutants_killed`/`survivor`, §7 step 4) is issue
  **#12**; **strengthening probes** (§7 step 5) are issue **#14**; the **FREE
  structural triage** (§7 step 1) is issue **#6** (`forge/src/vacuity.rs`, done).
- The shipped component issues exactly TWO solver queries per `fn` (one per gate
  check); with #271 the family grows to AT MOST THREE (the tightness query is
  skipped on a #6/#13 reject and on a unit return). It never scores mutants,
  never probes strengthenings, never re-lowers the body.

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

### The SPEC-TIGHTNESS SIGNAL (#271 — all NOT-STARTED, blocker #271)

Provenance: outside review item 6; the motivating finding is the L3-proven
`move_up` whose ens the identity body satisfies. Thesis anchor: `thermite-design.md`
§7 — the battery's residue paragraph ("what the battery cannot check — whether
the contract is the property the *user* wanted — is exactly the residue surfaced
for review"). Tightness is a SURFACED-FOR-REVIEW signal in precisely that sense:
it tells the §7 spec-intent reviewer *how much freedom* the postcondition leaves
the implementation, adjacent in spirit to §7 step 5's "which behavior the
contract fails to constrain" — but unlike steps 1–4 it carries NO verdict
authority, because a loose spec is often the INTENDED spec (relational /
bounds-shaped contracts).

- **REQ-8 (TIGHTNESS harness builder — the ∃-two-results query):** per checked
  `fn`, build ONE verus harness deciding the satisfiability of
  `∃ input, r1, r2. req(input) ∧ ens(input, r1) ∧ ens(input, r2) ∧ r1 ≠ r2`,
  posed through the SAME prove-the-negation trick as the REQ-2 vacuity harness
  (verus proves validity, so SAT is checked by failing to prove the negation).
  The grounded encoding (see *Ground the tightness harness*):

  ```text
  proof fn tight_check(<lowered params>, r1: <RET>, r2: <RET>)
      requires
          <lowered req>,
          <lowered ens with result := r1>,
          <lowered ens with result := r2>,
  { assert(r1 == r2); }      // structs: assert(r1 =~= r2)
  ```

  verus PROVING the assert means the postcondition pins a UNIQUE result for every
  `req`-satisfying input (the spec is FUNCTIONAL → `tight`); a genuine
  `assertion failed` counterexample is a witness input admitting two distinct
  results (→ `loose`). The `<lowered ens with result := rN>` copies are the
  VERBATIM `extract_lowered_fn`-extracted ens lines with the `result` identifier
  renamed at identifier boundaries (`result` is the reserved ens binder; an
  occurrence preceded by `.` is a FIELD access and is NOT renamed — OQ-7). The
  harness reuses the REQ-1/REQ-2 extraction, frame, spec-fn weaving, and the
  `run_harness` scratch-dir/seed/rlimit discipline unchanged. Source: outside
  review item 6; `thermite-design.md` §7 (residue).
- **REQ-9 (polarity + the honest third value, R-HONEST-3):** the tightness
  outcome maps FOUR ways, and — unlike the REQ-3 gate table — the inconclusive
  classes land in an explicit `undetermined` value rather than a `ForgeError`,
  because the signal is advisory and `undetermined` IS its honest surface:
  PROVED (`success && errors == 0`) → **`tight`**; a genuine assertion-failed
  counterexample (`!success && errors >= 1`, no solver-resource report on
  stderr) → **`loose`**; a TIMEOUT (an error WITH a `profile::parse_profile`
  resource report, the `check::classify_verus_outcome` `Timeout` vocabulary) →
  **`undetermined`** (a hard query is NOT evidence of looseness); a COMPILE-class
  non-verdict (`!success && errors == 0` — the grounded E0425 signature, see
  #275) or a VIR error → **`undetermined`**, never `tight`, never `loose` (the
  R-CODE-4 spirit: a non-verdict outcome must not masquerade as a verdict —
  here the field's own third value is the handled surface). Spawn/parse
  environment failures (`VerusAbsent`, unparseable `--output-json`) keep the
  family's existing `ForgeError` path — verus is already a hard prerequisite of
  the surrounding L3 run. Source: `goal.md` R-HONEST-3, R-CODE-4.
- **REQ-10 (REPORT-ONLY cert field — additive serde, goldens unchanged):** the
  verdict lands in a NEW additive `Certificate` field:

  ```rust
  #[derive(..., Serialize, Deserialize)]
  #[serde(rename_all = "snake_case")]
  pub enum SpecTightness { Tight, Loose, Undetermined }

  // on Certificate:
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub spec_tightness: Option<SpecTightness>,
  ```

  — exactly the `engine_attribution` / `solver_profile` / `assurance_scope`
  additive precedent (`pub engine_attribution: Option<EngineAttribution>` with
  `#[serde(default, skip_serializing_if = "Option::is_none")]` in `manifest.rs`),
  so the frozen goldens (`conformance/sum.cert.json`, `bank_account.cert.json`,
  …) deserialize unchanged (default `None`) and a `None` cert serializes
  byte-identically (R-SPEC-2; the O-D-class requirement). `None` means "no
  tightness query ran": a #6/#13 reject cert, a unit-return `fn`, a v1
  out-of-scope return type (REQ-11), or a pre-amendment cached cert. The field
  is REPORT-ONLY: it never feeds `Level`, never produces a `RejectReason`, and
  is EXCLUDED from `Certificate::oracle_subset` (the diagnostic-field precedent
  — verdict authority stays with the gate fields). NO opt-out / acknowledgement
  annotation in v1 (OQ-5 — resolved-by-default NO): a relational-by-design spec
  simply reports `loose` and certifies as before. Source: `manifest.rs` additive
  precedents; `thermite-design.md` §5.1, Appendix A (additive, non-frozen).
- **REQ-11 (v1 result-type scope + struct distinctness):** in scope — integer
  scalars + `bool` (`assert(r1 == r2)`), `Option<T>` / `Result` / user `enum`s /
  tuples of in-scope types (datatype `==`, grounded on `Option<usize>`), and
  STRUCTS whose fields are in-scope types, where `r1 ≠ r2` means ANY FIELD
  DIFFERS — encoded as `assert(r1 =~= r2)` (verus extensional equality; grounded
  PROVING on a fully-pinned struct and FAILING on the `move_up`-shaped bounds
  ens). Out of scope v1 (field stays `None`): unit returns (single inhabitant —
  the query is skipped, not reported `tight`, to keep the signal meaningful) and
  `String`/`Vec`/slice-valued returns (the view-equality encoding `r1@ =~= r2@`
  is OQ-6). The struct path REQUIRES weaving the reachable ADT decls
  (`reachable_adt_deps`, as `check::check_file` already computes for the L3
  sub-program) into the harness — the shipped #13 extraction weaves SpecFns ONLY,
  and the grounded consequence is blocker **#275** (an ADT-returning harness hits
  `E0425` and its compile error reads as a verdict); the tightness builder MUST
  NOT inherit that gap (its AC-7 fixture is a struct return, so the AC
  mechanically forces the weave). Source: grounded probes below; #275.
- **REQ-12 (wiring + cost + cache — rides the existing pass):** the tightness
  query runs INSIDE the proof-cache MISS branch of `check::check_file`'s
  per-item loop, immediately after `solver_vacuity_check` returns `Clean` (so it
  never runs on a known-degenerate contract — an unsat `req` would spuriously
  prove the tightness assert, the same false-premise hazard the CHECK-ORDER pin
  documents) and before the L3 proof; the verdict is attached to WHATEVER cert
  the L3 path produces (`Certificate::with_spec_tightness`, builder-style like
  `with_engine_attribution`) — a proved-L3 cert, a counterexample cert, or a
  timeout/ladder cert all carry it, because the signal is about the CONTRACT,
  not the body verdict. Cost: AT MOST ONE extra verus query per `fn`, on a cache
  MISS only (skipped on rejects + unit returns; ZERO on a cache HIT — the field
  is part of the stored cert, preserving the cache-hit verus-free invariant,
  `proof-cache.md` AC-1). Cache-key interaction: the verdict is a deterministic
  function of EXACTLY the existing 5-input key (the item's lowered source —
  which fixes `req`/`ens` — the pinned seed, the verus version, the thermite
  version, and `CHECK_SCHEMA_VERSION`), so NO new key input is needed; the
  builder BUMPS the module-internal `CHECK_SCHEMA_VERSION` (the #49 precedent in
  `cache.rs`) so every pre-amendment cache entry (which would deserialize with
  `spec_tightness: None`) universally MISSES and refreshes with the field
  populated. Source: `.design/forge/proof-cache.md` REQ-1 (the key composition +
  the `CHECK_SCHEMA_VERSION` amendment); the #13 GATE-PLACEMENT pin.
- **REQ-13 (human surface — `forge check` rendering):** `cli::render_human`
  prints the field when present, explicitly labelled as non-gating so a reader
  cannot mistake it for a verdict field — e.g.
  `spec_tightness: loose (report-only; a relational/bounds-shaped spec may be intended — review signal, never a gate)`
  — below the oracle fields, in the `solver_time_ms`-style labelled-diagnostic
  position. The `--json` surface is the REQ-10 field itself (additive, absent
  when `None`). Surfacing in the `forge audit` manifest (a `FunctionRow` copy,
  like the existing `contract_quality` copy) is OQ-8 — recommended YES but it
  touches `audit-manifest.md`'s schema, so it is a follow-on amendment there,
  not silently here. Source: `thermite-design.md` §5.1/§5.2 (the certificate as
  the displayed trust statement); `cli.rs` `render_human`.

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

### Tightness ACs (#271 — discharge REQ-8..REQ-13)

Expected verdicts hand-derived (R-CHAR-3) + grounded on real verus (the probes
in *Ground the tightness harness*); the corpus fixtures named all certify L3
TODAY, which is load-bearing for the never-a-gate claims.

- **AC-7 (the motivating LOOSE struct — `move_up`):** `forge check
  examples/editor/editor.th` reports `spec_tightness: "loose"` for `move_up`
  AND `move_up` STILL certifies L3 exactly as today (the report changes no
  verdict field). Grounded: the `move_up`-shaped harness (length-pinned text,
  upper-bounded cursor, struct result, `assert(r1 =~= r2)`) FAILS on real verus
  (`errors: 1`, "assertion failed") — for any row ≥ 1 input, `r1.cursor` = the
  true up-target and `r2.cursor = b.cursor` both satisfy the ens (and the text
  bytes are free up to length). Because `Buffer` is a struct return, this AC
  mechanically forces the REQ-11 ADT weave (a builder inheriting #275 would
  report `undetermined`, failing the AC).
- **AC-8 (the TIGHTENED struct reports tight — `deposit`):**
  `conformance/bank_account.th`'s `deposit` (L3 today; ens
  `result.balance == a.balance + amount` fully pins the single field) reports
  `spec_tightness: "tight"`. Grounded: the fully-pinned-struct harness PROVES
  `assert(r1 =~= r2)` on real verus (`success: true, errors: 0`). This is the
  "tightened move_up" shape: same struct-return machinery, ens pins every field.
- **AC-9 (the scalar FUNCTIONAL spec reports tight — `sum`):**
  `conformance/sum.th`'s `sum` reports `spec_tightness: "tight"`. Grounded: the
  harness with `r1 as nat == spec_sum(xs@)` and `r2 as nat == spec_sum(xs@)` as
  premises PROVES `assert(r1 == r2)` (`success: true, verified: 2, errors: 0`).
- **AC-10 (relational-by-design reports loose and STILL certifies L3):**
  `conformance/binary_search.th`'s `binary_search` reports
  `spec_tightness: "loose"` AND still certifies L3 with both gate bools `false`
  — the never-a-gate property asserted on a CORPUS item. Hand-derived: under
  `req sorted(haystack)` with duplicates allowed (`[3, 3]` is sorted),
  `Some(0)` and `Some(1)` both satisfy the `Some(i) ⇒ haystack[i] == needle`
  ens for `needle = 3`. Grounded on the simplified Option-result shape: the
  any-matching-index harness FAILS `assert(r1 == r2)` (`errors: 1`).
- **AC-11 (the honest third value):** a unit test over the tightness
  interpretation: a synthetic timeout summary (error + a solver-resource
  report) → `Undetermined`; the grounded compile-class signature
  (`success: false, errors: 0`) → `Undetermined`; NEITHER maps to `tight` or
  `loose` (REQ-9, R-HONEST-3). A reject cert and a unit-return `fn` carry
  `spec_tightness: None` (no query ran).
- **AC-12 (goldens + cache, O-D-class):** the frozen golden
  `conformance/sum.cert.json` (which omits the field) still deserializes; a
  `None`-field cert serializes byte-identically to today (`skip_serializing_if`);
  `Certificate::oracle_subset` is UNCHANGED by the field. A pre-amendment cache
  entry is never served (the `CHECK_SCHEMA_VERSION` bump → universal MISS); a
  post-amendment HIT serves the stored `spec_tightness` with ZERO extra verus
  spawns (the cache-hit verus-free invariant).

## Architecture

`vacuity_solver.rs` is a new `mod vacuity_solver;` in `forge/src/lib.rs`,
consumed by `check.rs`. It depends on `thermite_lower::lower` (the existing
contract lowering) and reuses forge's `run_verus`-class invocation
(`check.rs`'s verus driver). It owns NO new schema (it sets the two existing
`manifest::ContractQuality` bools and produces a `manifest::RejectReason`).
The #271 amendment adds the ONE additive `Certificate.spec_tightness` field
(REQ-10) — produced here, owned by `manifest.rs` per its additive-field
precedent.

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

### The tightness harness (#271, NOT-STARTED — third member, same machinery)

```rust
proof fn tight_check(<lowered params>, r1: <RET>, r2: <RET>)
    requires
        <lowered req>,
        <ens lines, result := r1>,
        <ens lines, result := r2>,
{ assert(r1 == r2); }    // struct RET: assert(r1 =~= r2)
```

The ∃-query `req ∧ ens(r1) ∧ ens(r2) ∧ r1 ≠ r2` is posed as the FAILURE of its
negation, exactly the vacuity-harness trick: the premises assume one input
vector and two ens-satisfying results; the assert claims they coincide. PROVED →
the spec is functional (`tight`); a genuine assert-failed counterexample → a
witness of two distinct admissible results (`loose`); inconclusive →
`undetermined` (REQ-9 — note the polarity table is RICHER than the gate's,
because for a report a misread timeout would be a WRONG `loose` claim, not a
missed detection). Construction deltas vs. REQ-1/REQ-2: (a) TWO result binders
instead of one, via the same `append_result_param` shape; (b) the ens lines move
from `ensures` to `requires` position, duplicated under the `result → r1/r2`
identifier-boundary rename (OQ-7); (c) the equality form is type-directed
(`==` scalar/datatype, `=~=` struct — REQ-11); (d) the sub-program weave must
include `reachable_adt_deps` (NOT just spec fns — the #275 lesson). The
`run_harness` scratch-dir/seed/rlimit/JSON-parse path is reused verbatim; only
the OUTCOME mapping differs (a dedicated interpretation, not the gate's
`interpret_summary`, because timeout and compile-class outcomes must land in
`Undetermined` rather than `Failed`-as-clean or `ForgeError`).

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

**Tightness interpretation (REQ-9 — the report-only column):**

| verus run | tightness harness |
|---|---|
| PROVED (`success && errors == 0`) | `tight` (the ens is functional) |
| FAILED, genuine counterexample (`errors >= 1`, no resource report) | `loose` (two admissible results witnessed) |
| TIMEOUT (`errors >= 1` WITH a `profile::parse_profile` resource report) | `undetermined` — a hard query is not evidence of looseness |
| COMPILE-class (`!success && errors == 0`, the grounded E0425 signature) / VIR | `undetermined` — a non-verdict never masquerades as a verdict |
| spawn / unparseable JSON | the family's existing `ForgeError` path |

### Gate wiring (REQ-5, `.design/forge/check.md`)

In `check::check_file`'s per-item loop, the order becomes:

```text
gate_fn (#6 structural triage)  ──Rejected──▶ Certificate::rejected  (no solver)
   │ ProceedToL3
   ▼
#13 vacuity check (run_verus on vacuity harness)  ──proved──▶ reject (VacuousPrecondition)
   │ failed (clean)
   ▼
#13 tautology check (run_verus on taut harness)   ──proved──▶ reject (SemanticTautology)
   │ failed (clean)
   ▼
#271 tightness query (report-only; verdict held, never a reject)        [NOT-STARTED]
   ▼
L3 proof of the real item (existing lower + run_verus)  ──▶ Certificate (graduate both bools
                                                            + attach spec_tightness)
```

The two SOLVER checks run in the SAME gate as #6 (`check_file`), AFTER the free
`gate_fn` triage passes (a contract that survives the syntactic checks may still
be semantically vacuous — the §7 ordering, cheapest-first) and before the item's
own L3 proof. A detected reject short-circuits to a non-certified cert
(verdict-in-cert), so no L3 proof runs on a known-degenerate contract (and no
tightness query runs either — its premises would be false). On a clean
pass through both, the item proceeds to the existing L3 path and the cert
graduates `contract_quality.{tautology, vacuous_precondition}` to the
SOLVER-confirmed `false` (REQ-6) — a strengthening of #6's syntactic-`false` —
and (#271) carries the held tightness verdict (REQ-12).

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
  calls `spec_sum`/`sorted` still lowers and resolves. CAVEAT (#275, grounded):
  the shipped weave passes SpecFns ONLY — an ADT-returning fn's harness omits the
  `struct`/`enum` decls and dies at `E0425`, which the gate's `interpret_summary`
  misreads as CLEAN. The #271 builder must weave `reachable_adt_deps` (REQ-11);
  the #275 fix for the two GATE harnesses is its own blocker, not this amendment.

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
- (#271, NOT-STARTED) tightness: unit tests over the tightness harness builder
  (the two-binder signature, the `result → r1/r2` rename, the struct `=~=` form,
  the ADT weave) and the REQ-9 interpretation (AC-11's synthetic
  timeout/compile-class summaries → `Undetermined`); conformance assertions on
  the corpus certs — `sum` `tight` (AC-9), `deposit` `tight` (AC-8),
  `binary_search` `loose` + L3 (AC-10), `move_up` `loose` + L3 (AC-7, verus-gated
  like `editor_runs.rs`); golden/serde stability (AC-12: `sum.cert.json`
  deserializes; `oracle_subset` unchanged; `CHECK_SCHEMA_VERSION` bumped).
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

### Ground the tightness harness (#271 — REAL verus, same binary/version, 2026-06-12)

**TIGHT on the functional scalar spec (`sum`'s ens — AC-9):**

```rust
spec fn spec_sum(xs: Seq<u32>) -> nat decreases xs.len()
{ if xs.len() == 0 { 0 } else { xs[0] as nat + spec_sum(xs.drop_first()) } }
proof fn tight_check(xs: &[u32], r1: u64, r2: u64)
    requires xs.len() <= 1_000_000,
        r1 as nat == spec_sum(xs@),
        r2 as nat == spec_sum(xs@),
{ assert(r1 == r2); }
```
`{ "success": true, "verified": 2, "errors": 0, "encountered-vir-error": false }`
→ PROVED → **tight**.

**LOOSE on the `move_up`-shaped struct bounds ens (AC-7):**

```rust
pub struct Buffer { pub text: Seq<u8>, pub cursor: u64 }
proof fn tight_check(b: Buffer, r1: Buffer, r2: Buffer)
    requires
        r1.text.len() == b.text.len(), r1.cursor <= b.cursor,
        r2.text.len() == b.text.len(), r2.cursor <= b.cursor,
{ assert(r1 =~= r2); }
```
stderr `error: assertion failed`;
`{ "success": false, "verified": 0, "errors": 1, "encountered-vir-error": false }`
→ FAILED → **loose** (the identity result and the true up-target both satisfy the
ens; even the text BYTES are free up to length).

**TIGHT on the fully-pinned struct ens (the `deposit` shape — AC-8 + the REQ-11
`=~=` any-field-differs decision):**

```rust
pub struct P { pub x: u64, pub y: u64 }
proof fn tight_check(p: P, r1: P, r2: P)
    requires r1.x == p.x, r1.y == p.y, r2.x == p.x, r2.y == p.y,
{ assert(r1 =~= r2); }
```
`{ "success": true, "verified": 1, "errors": 0, "encountered-vir-error": false }`
→ PROVED → **tight** (extensional struct equality proves when every field is
pinned; conversely a single free field makes it fail — the any-field-differs
semantics).

**LOOSE on the relational `Option` spec (the `binary_search` shape — AC-10 + the
REQ-11 datatype-`==` decision):**

```rust
proof fn tight_check(xs: &[u32], needle: u32, r1: Option<usize>, r2: Option<usize>)
    requires
        match r1 { Some(i) => i < xs@.len() && xs@[i as int] == needle, None => true },
        match r2 { Some(i) => i < xs@.len() && xs@[i as int] == needle, None => true },
{ assert(r1 == r2); }
```
stderr `error: assertion failed`;
`{ "success": false, "verified": 0, "errors": 1, "encountered-vir-error": false }`
→ FAILED → **loose** (any matching index is admissible — relational by design;
the report says so and the gate stays silent).

**The COMPILE-class signature (the #275 grounded finding — feeds REQ-9/REQ-11):**
a harness referencing an UNDECLARED type (the shipped SpecFn-only weave on a
struct-returning fn, e.g. `proof fn taut_check(p: P, result: P)` with no
`struct P` decl) yields stderr `error[E0425]: cannot find type 'P' in this
scope` and
`{ "encountered-error": true, "encountered-vir-error": false, "success": false, "verified": 0, "errors": 0 }`
— note `errors: 0` with `success: false`: NOT a counterexample, NOT a VIR error.
The shipped gate `interpret_summary` maps this to `Failed` → CLEAN (blocker
#275: both #13 gate checks are silent no-ops on every ADT-returning fn today,
verified end-to-end — `forge check` on a minimal `struct P` + `fn keep(p: P) ->
P` fixture sails past the #13 stage to step-4 mutation scoring). The tightness
interpretation maps this signature to `undetermined` (REQ-9) and the tightness
builder weaves ADT decls so it does not arise (REQ-11; AC-7 forces it).

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

(#271 deltas for the tightness harness: two trailing binders `r1`/`r2`; the ens
lines duplicated into `requires` position under the `result → rN` rename;
type-directed equality in the body; the weave extended with
`reachable_adt_deps` — REQ-8/REQ-11.)

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

(#271 — NO new reject fixtures: the tightness ACs assert REPORT fields on the
EXISTING corpus (`sum` tight, `bank_account::deposit` tight, `binary_search`
loose-and-L3) plus `examples/editor/editor.th::move_up` (loose-and-L3); a
dedicated relational fixture would only duplicate `binary_search`.)

## Route to add (orchestrator, NOT this component)

`tooling/spec-routes.toml`:
```toml
[[route]]
crate_pattern = "forge/src/vacuity_solver.rs"
design = ".design/forge/solver-vacuity.md"
reference = ["conformance/solver-vacuity", "conformance/sum.th", "conformance/binary_search.th"]
conformance_ops = ["tautology", "vacuous", "corpus_sum", "corpus_binary_search"]
```

(#271 follow-on once shipped: `conformance_ops` grows tightness entries — e.g.
`tight_sum`, `tight_deposit`, `loose_binary_search` — and `reference` gains
`conformance/bank_account.th`.)

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
- **OQ-5 (#271 — relational-by-design opt-out annotation):** should a spec author
  be able to acknowledge intended looseness (e.g. a future `#[relational]`
  attribute) so reviews can distinguish "loose, acknowledged" from "loose,
  unexamined"? RECOMMENDED: **NO annotation in v1** — the field is a pure report
  and an annotation would add grammar surface + a new gaming vector (slap
  `#[relational]` on everything) before the signal has earned its keep. Revisit
  only if review practice shows `loose` is too noisy on the real corpus.
- **OQ-6 (#271 — `String`/`Vec` result equality encoding):** v1 excludes
  `String`/`Vec`/slice-valued returns (field `None`). The candidate encoding is
  view equality (`r1@ =~= r2@`) but the exec-type binder + view interaction in a
  `proof fn` param position is ungrounded; needs its own verus probe before the
  scope grows (e.g. the editor's `render_frame -> String` would then report
  loose, which is true and useful).
- **OQ-7 (#271 — the `result → rN` rename edge):** the rename is an
  identifier-boundary textual substitution over the VERBATIM lowered ens lines;
  an occurrence preceded by `.` is a FIELD access (a user struct field may be
  named `result`) and must NOT be renamed. The cleaner long-term form is a
  binder-name parameter on the lowering `Ctx` (emit the ens directly under the
  `rN` name) — builder's call; the textual form is acceptable v1 because
  `result` is a reserved contract binder in the grammar (no param can shadow it).
- **OQ-8 (#271 — `forge audit` surfacing):** copy `spec_tightness` into the
  audit manifest's `FunctionRow` (next to its existing `contract_quality` copy)
  so the §8 audit story shows spec freedom alongside slag/boundary? RECOMMENDED
  yes, but it amends `audit-manifest.md`'s schema — a follow-on amendment THERE,
  not a silent ride-along here.

## Resolved during implementation (#13)

- **OQ-3 / OQ-4 (resolved):** a verus TIMEOUT / non-success-without-VIR-error on a
  harness maps to `Failed` → CLEAN (the conservative reading — an inconclusive
  query never rejects, never reads as a tautology); a verus-absent / unparseable /
  VIR error surfaces a `ForgeError` (never a silent clean). The arbitrary-`result`
  binder is sound for `u32`, `u64`, and `Option<usize>` (binary_search) — all
  confirmed PROVING/FAILING on real verus as grounded. CAVEAT discovered during
  the #271 grounding: the non-success-without-VIR-error class ALSO absorbs a
  COMPILE error (`errors: 0`, the E0425 signature) — see the #275 row note below.
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
  causes are unchanged. Pinned in `vacuity_solver::solver_vacuity_check`. The same
  precedence extends to the #271 tightness query (an unsat `req` would spuriously
  prove `assert(r1 == r2)`), which is why it runs only after a `Clean` (REQ-12).
- **GATE-PLACEMENT (resolved; OQ-2-adjacent):** the two queries run INSIDE the
  proof-cache MISS branch (after the cache lookup, before the L3 proof), so the
  deterministic #13 verdict (reject or clean) is CACHED with the item. A later
  cache HIT serves the stored cert WITHOUT re-spawning verus — preserving the
  proof-cache cache-hit verus-free invariant (`proof-cache.md` AC-1). A #13 reject
  cert is cached like a counterexample cert (a settled, deterministic verdict).

## Post-pin amendments (re-audited 2026-06-12, #262)

Four commits touched `vacuity_solver.rs` after the bootstrap pin `9d2d80bb`; the
cumulative diff (`git diff 9d2d80bb..HEAD -- forge/src/vacuity_solver.rs`) is
the #53 hygiene fix only:

- **#53 (`5746a9b6`) — the harness temp-binary leak is closed.** `run_harness`
  now writes the harness `.rs` into a per-run scratch DIRECTORY and spawns verus
  with `current_dir` set there; a `check::ScratchDir` Drop guard removes the
  directory WHOLESALE on every exit path (success, a clean FAILED, or a `?`
  early-return on an environment/IO error). The leak: verus compiles a
  SUCCEEDING harness — exactly the rejected tautology/unsat cases this gate
  exists to catch — into a ~4.3M sibling binary the old per-file
  `remove_file` cleanup orphaned. Removal is best-effort (`Drop` does a
  `let _ = remove_dir_all`), never a panic (R-CODE-2).
- #16 / #108 / #193 appear in the file's commit log but contribute no surviving
  behavioral change to this module; the gate placement (inside the cache-miss
  branch), the check order (vacuity BEFORE tautology), the harness shapes, and
  the interpretation table were all re-verified current.

## Grounded gap discovered during the #271 audit (blocker #275)

`extract_lowered_fn` builds the harness sub-program from `spec_items` (SpecFn
items only — `check::check_file` passes its SpecFn-filtered list) plus the fn;
it does NOT weave `reachable_adt_deps`. For a struct/enum-returning (or
-accepting) fn the harness therefore references an undeclared Verus type, verus
dies at `error[E0425]` — and the resulting summary
(`success: false, errors: 0, encountered-vir-error: false`) is mapped by
`interpret_summary` to `Failed` → CLEAN. Verified end-to-end: a minimal
`struct P` + `fn keep(p: P) -> P` fixture (and by the same token every
`examples/editor/editor.th` Buffer-returning fn) sails through the #13 stage to
step-4 mutation scoring with both bools reported solver-confirmed-`false`,
without any real solver query having run. Both gate checks are silent no-ops on
ADT-signature fns today — an R-CODE-4 violation in effect (a non-verdict outcome
read as clean). Filed as blocker **#275**; the doc records the code as it IS
(the REQ-1/REQ-3/REQ-6 rows below carry the caveat) and the #271 tightness REQs
are specified not to inherit it (REQ-9/REQ-11).

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (tautology harness builder) | SHIPPED | `vacuity_solver::build_tautology_harness` lowers the real `FnItem` (+ spec fns) via `thermite_lower::lower` and rebuilds `proof fn taut_check(<params>, result: <RET>) requires ..; ensures ..; { }` (`extract_lowered_fn` reuses the verbatim `requires`/`ensures`). Consumer: `check::check_file`. Grounded: PROVES on `result >= 0`/`u32`, FAILS on `sum`'s ens. CAVEAT #275: the weave is SpecFn-only — an ADT-signature fn's harness hits `E0425` (no real query runs). |
| REQ-2 (vacuity harness builder) | SHIPPED | `vacuity_solver::build_vacuity_harness` reuses the same extraction → `proof fn vac_check(<params>) requires ..; { assert(false); }`. Consumer: `check::check_file`. Grounded: PROVES on `x>5 && x<3`, FAILS on `sum`'s `req`. CAVEAT #275: same SpecFn-only weave. |
| REQ-3 (verdict interpretation, R-CODE-4) | SHIPPED | `vacuity_solver::interpret_summary`: PROVED (`success && errors==0`) → DETECTED; FAILED → CLEAN; VIR error → `ForgeError::VerusOutput`; `run_harness` surfaces verus-absent / unparseable as `ForgeError`, never a silent clean. CAVEAT #275 (grounded): the FAILED arm also absorbs the COMPILE-error class (`!success && errors==0`, E0425) as CLEAN — a non-verdict read as a verdict on ADT-signature fns. |
| REQ-4 (value-add over #6) | SHIPPED | the `semantic_tautology` / `vacuous_precondition` fixtures PASS `vacuity::triage` (no #6 syntactic cause) yet `solver_vacuity_check` rejects them with the SOLVER causes — asserted by `forge/tests/solver_vacuity_conformance.rs` against `conformance/solver-vacuity/cases.json`. |
| REQ-5 (gate wiring, verdict-in-cert) | SHIPPED | `check::check_file` calls `vacuity_solver::solver_vacuity_check` after #6 `gate_fn` returns `ProceedToL3` (inside the cache-miss branch, before L3); a `Detected` → `Certificate::rejected_vacuity` (`Level::L0` + cause), a `Clean` proceeds to L3. |
| REQ-6 (graduate the two bools to solver-confirmed) | SHIPPED | `Certificate::rejected_vacuity` sets `contract_quality.tautology`/`vacuous_precondition = true` on the matching detection; a `Clean` reaches the L3 path whose `graduate_triage_clean` keeps both live-`false`, now solver-confirmed. CAVEAT #275: for ADT-signature fns the "solver-confirmed" strength is overstated today (the queries were E0425 no-ops). |
| REQ-7 (determinism + one query/check) | SHIPPED | `run_harness` passes the pinned `seed` + `rlimit`; exactly two verus queries per `fn` (vacuity then tautology), short-circuiting on the first detection. |
| REQ-8 (tightness harness builder — ∃-two-results) | NOT-STARTED | open blocker #271. No `build_tightness_harness` exists in `vacuity_solver.rs`; the query shape + the `result → r1/r2` rename and the grounded verus verdicts are pinned above for the builder. |
| REQ-9 (tightness polarity + honest `undetermined`) | NOT-STARTED | open blocker #271. No `SpecTightness`-valued interpretation exists; the gate's `interpret_summary` is NOT reusable as-is (it maps timeout AND the grounded compile-class to `Failed`, which here would be a WRONG `loose`/clean reading — the four-way table above is the contract). |
| REQ-10 (`Certificate.spec_tightness` additive field) | NOT-STARTED | open blocker #271. `manifest.rs` has no such field (`grep spec_tightness forge/src/` is empty); the additive-serde shape mirrors the shipped `engine_attribution: Option<EngineAttribution>` precedent (`#[serde(default, skip_serializing_if = "Option::is_none")]`, oracle-EXCLUDED). |
| REQ-11 (v1 type scope + struct `=~=` + ADT weave) | NOT-STARTED | open blockers #271, #275. The struct path needs the `reachable_adt_deps` weave the shipped extraction lacks (the #275 root cause); the `==`/`=~=` split is grounded on real verus above. |
| REQ-12 (wiring + cache + `CHECK_SCHEMA_VERSION` bump) | NOT-STARTED | open blocker #271. `check::check_file`'s cache-miss branch runs only the two gate queries today; no `with_spec_tightness` attach point, no schema-version bump. |
| REQ-13 (`forge check` human rendering) | NOT-STARTED | open blocker #271. `cli::render_human` prints no tightness line (it renders the oracle subset + the labelled diagnostics; the field does not exist yet). |
| — substrate: harness frame + scratch/seed/rlimit discipline | SHIPPED | `vacuity_solver::run_harness` — "writes the harness to a `<stem>.rs` file … INSIDE a per-run scratch DIRECTORY, spawns verus there with the pinned `seed` + `rlimit` + `--output-json`" with the `check::ScratchDir` Drop guard (#53); the tightness query reuses it verbatim (REQ-8). Consumer: `solver_vacuity_check`. |
| — substrate: verbatim contract extraction | SHIPPED | `vacuity_solver::extract_lowered_fn` / `parse_lowered_fn` — "the harness's contract text is the SAME bytes the real L3 proof sees"; `append_result_param` is the binder-append shape REQ-8 doubles. Consumer: both shipped builders. |
| — substrate: additive cert-field precedent | SHIPPED | `manifest::Certificate.engine_attribution` — "`#[serde(default, skip_serializing_if = "Option::is_none")]` … so the frozen golden" deserializes unchanged; `Certificate::with_engine_attribution` is the builder-style attach REQ-12 mirrors. Consumer: `check::check_file` / the engine path. |
| — substrate: timeout vocabulary for REQ-9 | SHIPPED | `check::classify_verus_outcome` — "`Timeout` (an error WITH a `profile::parse_profile` report present on stderr)" vs `Counterexample` (an error WITHOUT one); the tightness interpretation reuses exactly this split to keep a hard query out of `loose`. |
| — substrate: 5-input cache key | SHIPPED | `cache::cache_key(lowered_src, seed, verus_version, thermite_version)` + the module-internal `CHECK_SCHEMA_VERSION` (`proof-cache.md` REQ-1, the #49 amendment) — the tightness verdict is a function of exactly these inputs, so REQ-12 adds NO key input, only the version bump. Consumer: `check::check_file`. |
