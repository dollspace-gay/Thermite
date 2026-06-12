# Composition Experiment — contract-carrying vs agent-plus-tests (the thesis-level experiment)
<!--
tier: research
status: draft
governs: (no code yet — pins the PLANNED harness under experiments/composition/; spec-routes
         entries are added when the harness lands, not before)
thesis-refs:
  - thermite-design.md §1
  - thermite-design.md §2 (pillar 6)
  - thermite-design.md §5.2
  - thermite-design.md §7
  - thermite-design.md §9
related:
  - .design/basis/05-composition.md (the SHIPPED compositional machinery this experiment exercises)
  - goal.md R-CHAR-3, R-HONEST-3 (the measurement-independence and symmetric-honesty constraints)
issue: #273 (outside review, item 9)
-->

## Summary

A minimal controlled experiment testing the Thermite thesis at the COMPOSITION level: AI
agents integrate pre-verified components drawn from the four shipped example programs
(`examples/editor`, `examples/formatter`, `examples/calculator`, `examples/parser`), in two
arms — **contract-carrying** (arm A: the agent sees only the components' contracts and must
produce a `forge check`-certifying composition) versus **agent-plus-tests** (arm B: the same
components stripped to bare signatures, conventional Rust glue plus the agent's own tests).
Correctness is judged by **held-out acceptance suites hand-authored in advance** — never by
either arm's own gate (R-CHAR-3). The doc is the pre-registration: hypothesis, tasks, arms,
measures, confounds, stopping rule, and the falsification statement are fixed here before
any artifact is built or any session is run.

## What claim is under test (the thesis, quoted)

The experiment operationalizes three specific claims:

1. **§9 (the composition rule):** "if `g` calls `f` only through `f`'s contract, then `g`'s
   certificate is valid independent of `f`'s body. Trust is invariant under composition
   instead of decaying multiplicatively — which is the property that matters once
   unsupervised agents start building large systems."
2. **§2 pillar 6 (the contract is the interface):** "Once a function verifies, no caller
   (human or agent) ever needs to read its body again."
3. **§1 (the failure-mode match):** agents' "failure mode (locally plausible, globally
   wrong) is exactly the failure mode machine verification catches."

The machinery these claims rest on is SHIPPED and documented in
`.design/basis/05-composition.md`: REQ-1 ("CONTRACT composition — a caller `g` ... proves
AGAINST `f`'s contract (`requires`/`ensures`), never `f`'s body", via
`lower_external_body_fn in thermite-lower/src/lower.rs` + `item_subprogram in
forge/src/check.rs`), REQ-2 ("ASSURANCE aggregation — project = min over parts", via
`AssuranceManifest::aggregate in forge/src/manifest.rs`, the min core verus-verified), and
REQ-3 (TCB = union of parts' boundary/slag ∪ toolchain, via `Tcb::from_certificates in
forge/src/audit.rs`). The experiment does not re-verify that machinery — it tests whether
the discipline that machinery enforces **buys compositional correctness for agent-built
software in practice**, against the obvious cheaper alternative (an agent plus conventional
tests). That is the thesis-level question: the mechanism being sound (§9, proven) does not
by itself establish that the mechanism catches the defects agents actually introduce at
integration seams.

## Hypothesis (pre-registered, falsifiable)

> **H1.** When agents integrate pre-verified components into a composed program under a
> fixed effort budget, the contract-carrying arm (A) ships FEWER integration-seam defects
> to the held-out suite than the agent-plus-tests arm (B) — in a majority of task×agent
> cells — AND at least one defect class is caught at compose time by arm A's contracts
> (a `forge check` counterexample) that arm B ships to held-out, i.e. D(A→B) ≠ ∅.

> **H2 (the honesty secondary — §7's residue, measured).** Some arm-A artifacts will
> certify (L3, project = min) yet still fail held-out vectors — the spec-intent gap ("are
> these the contracts you wanted?", §1). H2 quantifies that residue: the certified-yet-
> failed rate. It is reported with the same prominence as H1 whichever way it goes.

**What would FALSIFY the thesis claim (stated up front, R-HONEST-3 — an experiment that
cannot fail is propaganda):**

- Arm B matches or beats arm A on held-out seam-defect rate across the majority of
  task×agent cells at comparable effort; **or**
- D(A→B) = ∅ while D(B→A) ≠ ∅ — the contracts caught nothing the baseline's tests missed,
  while the baseline's tests caught real defects the certificates waved through; **or**
- Arm A's certified-yet-failed rate (H2) is comparable to arm B's overall held-out failure
  rate — certification added gate cost without adding held-out correctness.

Any of these outcomes is reported as **"the compositional-correctness claim is unsupported
at v0.1 scale"** — not reframed, not buried. The symmetric metric (both D(A→B) and D(B→A))
is mandatory in the report.

## Tasks (grounded in the real exports of `examples/`)

Three integration tasks. Each composes components from ≥2 example programs, and each has a
**designed seam** — a place where the components' real contracts impose an obligation that
conventional gluing plausibly gets wrong. The seams below are read off the actual shipped
contracts (symbol anchors into the example sources); the held-out suites target them.

### T1 — `csv_sum`: the CSV parser feeding the calculator feeding the formatter

Compose `fn fields(s: String, sep: u64) -> Vec<String>` (`examples/parser/parse_lines.th`,
ens `result.len() == 1 + count_sep(s, sep)`) → the calculator's parse/add front
(`fn add(a: String, b: String) -> Option<u64>` in `examples/calculator/calc.th`, req
`all_digits(a) && a.len() >= 1 && parse_be(a) <= 9223372036854775807 && ...`, ens pinning
`Some(parse_be(a) + parse_be(b))`) → `fn format(n: u64) -> String`
(`examples/formatter/format.th`, ens `parse_be(result) == n`). **Task:** given a two-field
digit CSV line (`"2,3"`), return the formatted sum (`"5"`); malformed input (wrong field
count, empty field, non-digit field, out-of-range operand) must be rejected loudly per the
task statement.

Designed seams: (a) the **separator representation mismatch** — `fields` takes the
separator as a **byte** (`sep: u64`, `','` = 44) while `has_sep(s: &String, sep: &String)`
takes it as a **String**; (b) `add`'s `req` chain (`all_digits` excludes the empty field
`"2,,3"` only via `a.len() >= 1`; the `parse_be(_) <= i64::MAX` bound excludes overflow);
(c) the exact-field-count obligation derived from `fields`' count ens.

### T2 — `calc_insert`: the calculator embedded in the editor

Compose the editor's verified edit core (`fn insert_str(b: Buffer, ins: String) -> Buffer`
in `examples/editor/editor.th`, req `b.text.len() + ins.len() <= 1_000_000`, ens exact
text-growth and cursor-advance; `struct Buffer ... inv cursor <= text.len() && text.len()
<= 1_000_000`) with the calculator's `add_vals` (req each operand `<= 9223372036854775807`,
ens pins the sum and forbids `None`) and the formatter's decimal emission. **Task:** given
a `Buffer` and two `u64` operands, insert the formatted decimal sum at the cursor,
preserving the `Buffer` invariant; reject (no-op or loud error per the task statement) when
the insertion would exceed capacity.

Designed seams: (a) the **capacity obligation** — `insert_str`'s req needs an UPPER bound
on the formatted sum's length, but `format`'s ens gives only a length FLOOR
(`result.len() >= 1`); the ≤20-digit u64 bound lives in the C4 `u64_to_string` contract
(the `render_frame` discharge precedent, blocker #105) and the composer must find and
thread it; (b) the near-capacity boundary (`text.len() = 999_981` + a 20-digit sum); (c)
the operand-range reqs.

### T3 — `status_line`: the editor's nav core feeding the formatter

Compose `fn cursor_row(b: &Buffer) -> u64` (ens `result <= b.cursor`), `fn cursor_col(b:
&Buffer) -> u64` (ens `result == b.cursor - spec_line_start(&b.text, 0, b.cursor, 0)`),
`fn to_1based(x: u64) -> u64` (req `x < 1_000_000`, ens `result == x + 1`) and the C4
decimal formatting into a `"Ln <r>, Col <c>"` status string (1-based, the ANSI/editor
convention). **Task:** produce the status string for any valid `Buffer`.

Designed seams: (a) the **0-based/1-based off-by-one**, pinned by `to_1based`'s exact ens;
(b) the **strict-bound boundary** — `to_1based` requires `x < 1_000_000`, but
`cursor_row`'s ens gives only `result <= b.cursor` and the `Buffer` invariant allows
`cursor == text.len() == 1_000_000` (an all-newline buffer makes the row hit exactly
1_000_000), so the composition's precondition is NOT dischargeable for every valid
`Buffer` — the composer must either narrow its own req or clamp, and a counterexample at
the exact boundary is what `forge check` surfaces; (c) col-after-newline (col resets to 0
at a line start — `spec_line_start` jumps to `i + 1` past a newline).

Two tasks would under-sample the seam space; four exceeds the budget. T1 exercises
pure-pipeline composition (the `05-composition.md` REQ-1 shape `h(g(f(x)))`), T2 exercises
struct-invariant-preserving composition, T3 exercises a precondition that is genuinely
non-dischargeable without caller-side narrowing. Each is checkable end-to-end with shipped
machinery (no Stage-3 effect boundary required — the editor's `#[boundary]` os seam is
deliberately excluded so neither arm touches trusted I/O).

## Arms

**Arm A — contract-carrying (the Thermite discipline).** The agent receives:
`THERMITE.skill.md` (the §10 skill — the language onboarding IS part of the system under
test), the task statement, and the **contract pack**: each needed component's signature +
`req`/`ens`/`fx` + the named spec fns its contracts mention (`parse_be`, `all_digits`,
`count_sep`, `contains_sub`, `spec_line_start`, the C4 length bound) — **bodies withheld**
(this operationalizes pillar §2.6 literally: the caller composes through the contract,
never the body). The agent writes the composed Thermite `fn`(s); the harness concatenates
them with the real component sources (v0.1 is single-file — no module system) and runs
`forge check`. The arm's gate: the composed items certify, project assurance = min over
parts (`05-composition.md` REQ-2). Compose-time defects = the per-obligation failures /
counterexamples the agent iterates through (forge's structured output, §5.1, is the log).

**Arm B — agent-plus-tests (the baseline).** The agent receives: the SAME task statement
verbatim, and the **signature pack**: the same components as an opaque pre-compiled Rust
library (built by the harness from the SAME lowered component sources, with the L1 runtime
contract checks COMPILED OUT — the baseline must not silently inherit contract enforcement;
this is the "normal crates.io dependency" condition) with bare Rust signatures + one-line
doc comments (no `req`/`ens`/`fx`). The agent writes conventional Rust glue plus its own
`#[cfg(test)]` suite. The arm's gate: its own tests green. Compose-time defects = its own
test failures during the session.

**Symmetry constraints (frozen in the harness):** identical task statements; prompts differ
ONLY in the contract-pack vs signature-pack material (mechanically diffable, AC-2);
identical effort budget per session (fixed turn cap + token cap, pinned in the harness
config); neither arm ever sees the held-out suite (R-CHAR-3); the same agent models run
both arms.

## Measures (per cell = task × arm × agent × repetition)

| # | Measure | Arm A operationalization | Arm B operationalization |
|---|---|---|---|
| M1 | Held-out correctness | held-out suite verdict on the composed artifact (pass fraction of vectors + properties) | same suite, same vectors, via the arm-B adapter |
| M2 | Seam-defect count at held-out | failures classified by the defect taxonomy below | same |
| M3 | Compose-time catches | forge obligations failed → fixed during the session (counterexamples, from the structured per-obligation log) | own-test failures → fixed during the session |
| M4 | Effort | turns to first gate-green artifact; total tokens; solver seconds | turns to gate-green; total tokens; test seconds |
| M5 | Gate honesty | final certificate level (min over parts; any L2/L1 downgrades) | final self-reported test pass state |

**Defect taxonomy (fixed in advance):** precondition violation at a seam · representation
mismatch (byte-vs-String separator) · arithmetic overflow · off-by-one (0/1-based) ·
capacity-bound violation · dropped/duplicated field · vacuous success (the arm's own gate
green but held-out red).

**The key metric (symmetric honesty):**

- **D(A→B)** — defect classes arm A's contracts caught at compose time (M3) that arm B
  shipped to held-out (M2). Evidence the contract discipline catches what tests miss.
- **D(B→A)** — defects arm B's own tests caught that arm A shipped to held-out DESPITE
  certifying (the H2 spec-intent residue). Evidence in the other direction.

Both are reported per task, with the concrete defect instances named. Reporting only
D(A→B) would be propaganda; the symmetry is the design.

## Confounds (named) and mitigations

1. **Agent familiarity asymmetry.** Models have trained on oceans of Rust and near-zero
   Thermite. Mitigation: the §10 skill-as-spec claim is itself part of the thesis (arm A
   gets the skill, as the design intends); effort (M4) is reported separately from
   correctness (M1/M2) so a slower-but-correcter A is visible as exactly that; ≥2 distinct
   agent models cross the arms. The confound is REPORTED, not eliminated — stated as a
   v0.1-scale limit on external validity.
2. **Sunk contract-authoring cost.** Arm A's component contracts were already authored and
   verified (the examples buildout paid it). The experiment measures COMPOSITION cost and
   correctness only; the comparison explicitly excludes component-authoring cost. Contract
   LOC per component is reported as context so the reader can weigh it.
3. **Held-out suite coverage.** The suite itself can miss defects both arms shipped — both
   arms' M1 is an upper bound on true correctness. Mitigation: suites are authored
   adversarially AT the designed seams (the seam analysis above is the checklist), frozen
   and hash-pinned before any session (AC-1), never amended after a run, and validated by
   seeded-defect references (≥1 hand-written defective composition per targeted defect
   class per task; the suite must kill every seed).
4. **Single-run variance.** Agent sessions are stochastic. n = 3 repetitions per cell;
   full grid = 3 tasks × 2 arms × 2 agents × 3 reps = 36 sessions; per-cell distributions
   reported, no cherry-picking (the grid is the pre-registration).
5. **Prompt asymmetry.** Both arm prompts are generated from one template and frozen
   before runs; their diff must contain only the contract-pack/signature-pack material
   (AC-2, mechanically checked).
6. **Judge contamination (R-CHAR-3, extended).** goal.md R-CHAR-3 forbids expected values
   "literal-copied from the toolchain's own output"; here the rule extends to the agents:
   the held-out suites are hand-authored from THIS doc's task statements and seam analysis
   by the orchestrator/human, before any session, and are never derived from either arm's
   artifacts or from forge output. Neither arm's agent sees them.

## Harness: build vs reuse

**Must be BUILT (the REQ rows below; everything lands under `experiments/composition/`):**

- The **task pack**: 3 task statements + arm-A contract packs + arm-B signature packs,
  frozen (REQ-E1).
- The **held-out suites**: per task, input→expected vectors + property checks + the two
  per-arm adapters (arm A: drive the assembled `.th` through `forge check` + execute the
  lowered Rust composition directly via a thin Rust driver — sidestepping the v0.1
  zero-arg-entry limit and the documented C5/C7 no-L1-emission gap, see OQ-2; arm B:
  `cargo test` against the glue + opaque library), plus the seeded-defect validation
  references (REQ-E2).
- The **arm-B component library build**: lower the same component sources, strip the L1
  contract checks, compile to an rlib; plus a behavioral-equivalence spot-check against
  arm A's components (REQ-E3).
- The **session runner**: per-cell dispatch with the frozen prompt, transcript/turn/token
  capture, artifact collection, assembly + gate execution, per-cell JSON record (REQ-E4).
- The **scorer**: defect-taxonomy classification, D(A→B)/D(B→A) attribution, the grid
  report (REQ-E5).

**Already EXISTS in the repo (reused, not rebuilt):**

- `forge check` / `forge build` / `forge audit` — arm A's entire gate, including the
  composition machinery (`05-composition.md` REQ-1/2/3, SHIPPED) and the structured
  per-obligation output (§5.1) that doubles as the compose-time defect log.
- `THERMITE.skill.md` + the CI token-budget gate — arm A's onboarding (§10).
- The four example programs — the verified component library itself, with contracts
  already battle-tested (`forge check` L3 per `examples/README.md`).
- The conformance-corpus pattern (`conformance/<name>.cert.json` golden certificates) —
  the format precedent for the held-out goldens and the per-cell record schema.
- The ACToR/crosslink dispatch machinery (`.crosslink/acto-*.md` agent definitions, the
  kickoff pattern) — the session-runner substrate REQ-E4 wraps rather than reinvents.

## Stopping rule (pre-registered)

Run the full 36-session grid. No early stopping on results in either direction. A harness
defect (runner crash, suite bug found via a seeded reference, prompt-symmetry violation)
stops the affected cells, is fixed and logged, and the cells are re-run from scratch; the
log is part of the report. After the grid: score, report every cell, and state the H1
verdict (supported / unsupported) and the H2 residue using the falsification criteria
verbatim from this doc. The experiment ends there — no extension runs "to get a cleaner
result" without a new pre-registered amendment to this doc.

## Requirements

All NOT-STARTED — this doc is the pre-registration; nothing is built. Blocker: **#273**.

- **REQ-E1 (task pack):** the 3 task statements (T1/T2/T3) + arm-A contract packs
  (extracted from the cited example contracts) + arm-B signature packs, frozen under
  `experiments/composition/tasks/`. Derived from §9 + pillar §2.6 (the contract pack IS
  the interface) + the seam analysis above.
- **REQ-E2 (held-out suites):** per task, hand-authored vectors + properties + per-arm
  adapters + seeded-defect references, frozen and hash-pinned BEFORE any session
  (R-CHAR-3; confound 3/6 mitigations).
- **REQ-E3 (arm-B component library):** the contract-stripped opaque rlib built from the
  same lowered component sources + the behavioral-equivalence spot-check (arms differ in
  CONTRACTS, never in component semantics).
- **REQ-E4 (session runner):** per-cell dispatch, frozen prompts, budget caps,
  transcript/turn/token capture, gate execution, schema-valid per-cell JSON records;
  wraps the existing ACToR dispatch machinery.
- **REQ-E5 (scorer + report):** taxonomy classification, the symmetric D(A→B)/D(B→A)
  attribution, the grid report with the H1/H2 verdicts stated per the falsification
  criteria (R-HONEST-3).
- **REQ-E6 (the run):** the 36-session grid executed under the stopping rule, all records
  committed, the verdict stated whichever way it goes.

## Acceptance criteria

- **AC-1 (suite freeze):** sha256 of each `experiments/composition/tasks/<t>/holdout/`
  tree is recorded in-repo before the first session, and every seeded-defect reference
  (≥1 per targeted defect class per task) FAILS the suite. (REQ-E2.)
- **AC-2 (prompt symmetry):** the mechanical diff of the arm-A vs arm-B prompt for each
  task contains only contract-pack vs signature-pack material. (REQ-E1, REQ-E4.)
- **AC-3 (component equivalence):** the arm-B rlib and arm A's lowered components agree on
  the equivalence spot-check vectors — the arms differ only in contracts. (REQ-E3.)
- **AC-4 (grid completeness):** 36 schema-valid per-cell records, no missing cells, each
  carrying M1–M5. (REQ-E4, REQ-E6.)
- **AC-5 (symmetric verdict):** the report states D(A→B) AND D(B→A) with named defect
  instances, the H2 certified-yet-failed count, and the H1 verdict in the falsification
  criteria's own words — regardless of direction. (REQ-E5, REQ-E6.)

## Open questions

- **OQ-1 (single-file composition representativeness).** v0.1 has no module system, so
  "integration" is concatenation into one file. The §9 claim is about call-through-contract
  regardless of file layout, and the contract pack still withholds bodies — but the
  external-validity caveat (real integration crosses crate/module boundaries) goes in the
  report.
- **OQ-2 (the C5/C7 L1-emission gap).** `all_digits`/`parse_be`/`count_sep` have no L1
  runnable emission (the documented forcing-function finding in
  `examples/calculator/calc.th` and `examples/parser/parse_lines.th` — contract-bearing
  fns naming them `forge check` but do not `forge build`). The arm-A held-out adapter
  therefore drives the LOWERED Rust composition directly rather than relying on
  `forge build --entry` zero-arg wrappers. If the gap closes upstream before the run, the
  adapter simplifies; either way the adapter choice is fixed before the first session.
- **OQ-3 (agent selection and budget calibration).** Which two models, and what turn/token
  cap, are pinned in the harness config when REQ-E4 lands — calibrated by ONE throwaway
  pilot session per arm on a fourth, non-grid warm-up task (so no grid task is contaminated
  by calibration runs).
- **OQ-4 (does arm A get mutation scores?).** `forge check`'s §7 battery output (kill
  ratios, surviving mutants) is part of the shipped gate; arm A sees it as a matter of
  course. It is part of the system under test, not a leak — but the report notes it as part
  of arm A's information advantage, mirrored by arm B's freedom to write arbitrary tests.

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-E1 (task pack) | NOT-STARTED | blocker #273. No `experiments/composition/` tree exists; the task statements and seam analyses exist only in this doc. |
| REQ-E2 (held-out suites) | NOT-STARTED | blocker #273. No suites, adapters, or seeded-defect references authored; AC-1's freeze must precede the first session. |
| REQ-E3 (arm-B component library) | NOT-STARTED | blocker #273. No contract-stripped build mode exists; the harness must produce the rlib from the lowered example sources and prove behavioral equivalence (AC-3). |
| REQ-E4 (session runner) | NOT-STARTED | blocker #273. The ACToR dispatch substrate exists (`.crosslink/acto-*.md`) but no per-cell runner, prompt freezing, budget caps, or record schema. |
| REQ-E5 (scorer + report) | NOT-STARTED | blocker #273. Taxonomy and symmetric-metric definitions are pinned above; no scoring code or report template exists. |
| REQ-E6 (the run) | NOT-STARTED | blocker #273. Blocked on REQ-E1..E5; the stopping rule and falsification criteria above govern it when it runs. |
