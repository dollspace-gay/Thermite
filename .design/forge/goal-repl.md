# Forge Goal-State REPL — `goal` / `fill` / `edit` / `battery`
<!--
tier: 3-component
status: draft
audited-sha: dff9ae866e3437af272a62e078993e66c1116460 (re-audited 2026-06-12: amended — REQ-2 holes-render + REQ-7 increment-(iii) residue corrected, stale (future) governs markers dropped, #195 shared open_hole_reason noted, #262)
governs: forge/src/goal_repl.rs, forge/src/cli.rs (verb dispatch), thermite-syntax/src/parser.rs (hole token)
thesis-refs:
  - thermite-design.md §5
  - thermite-design.md §5.1
  - thermite-design.md Appendix B
-->

## Summary

The Lean-style incremental goal-state REPL is the LAST unshipped surface of the
v0.1 thesis (issue #21 item 3): `forge goal <item>`, `forge fill <addr> <code>`,
`forge edit <addr> --replace <code>`, and `forge battery [item]`. All four verbs
now SHIP (increments (i)+(ii) in `70a65bdb`+`5b8033ad`; increment (iii) — holes +
`forge fill` — in #193). This doc grounds them as VIEWS over already-SHIPPED
machinery (semantic addressing, per-obligation `forge check` results +
counterexamples, and the vacuity+mutation battery that runs inside the gate) plus
ONE genuinely new research-spike capability: a body-position hole token (`?N`) and
a `fill` that splices code at a hole address and re-checks. The doc adapts to what
exists; it proposes no change to the shipped substrates, only documents the
contract the new verbs satisfy and pins their v1 scope honestly.

> **Amendment 2026-06-12 (doc-freshness re-audit, #262).** Re-verified against the
> current tree (`dff9ae86`; one post-pin commit, #195 `d2d1d445`). Two
> increment-(iii) residues corrected: the REQ-2 row's "Holes (`?N`) NOT rendered
> (increment iii)" clause was already false at the pin commit (which itself shipped
> increment (iii)) — `goal_repl::render_goal_item` renders the §5.1 `holes:` section
> from `FnItem.holes`; and REQ-7's "once increment (iii) lands" qualifier is
> discharged (`pub fn fill_hole in goal_repl.rs` returns `Result<_, ForgeError>`,
> pure splice). Post-pin addition (#195, additive): `pub(crate) fn open_hole_reason
> in goal_repl.rs` is now the SHARED open-hole refusal text — `check.rs`'s per-item
> `OpenHole` reject, `build::build_file`'s pre-lowering refusal, and the TV phases
> all gate on this ONE helper (the #192 single-copy lesson), so REQ-5's refusal
> language cannot drift across paths. No REQ changed status.

## Requirements

- REQ-1 (`forge battery [item]` — standalone battery view): expose the vacuity
  triage (`vacuity::triage`), solver-backed vacuity (`vacuity_solver::solver_vacuity_check`),
  and mutation scoring (`check::mutation_score`) that ALREADY run inside the gate
  as a standalone verb that reports the §7 anti-Goodhart battery for one item or
  the whole file, WITHOUT re-defining any verdict. A thin VIEW over the existing
  per-item pipeline. Derived from `thermite-design.md` §7 + Appendix B
  (`forge battery [item]   run vacuity battery + mutation scoring`).

- REQ-2 (`forge goal <item>` — goal-state render): render the goal state for an
  item as the §5.1 four-part view (given / want / holes / per-obligation status
  with counterexamples), as a VIEW over the existing `forge check` per-item
  `Vec<Certificate>` + `ObligationResult` collection. An item with no holes and a
  clean cert renders `ALL GOALS DISCHARGED`; an item with a counterexample renders
  the failed obligation's concrete witness (never an adjective, §5.1 property 2);
  an item with open holes renders `holes: ?N : <position>`. Derived from §5/§5.1.

- REQ-3 (`forge edit <addr> --replace <code>` — semantic edit by address): locate
  a node by its stable semantic address (`thermite_syntax::address::resolve`),
  splice the replacement SOURCE TEXT at that node's span IN THE FILE, re-emit the
  file, and re-check the affected item, printing the new goal state. Derived from
  §4.3 + Appendix B (`forge edit <addr> --replace <code>   semantic edit by
  address`). The proof cache (§5.3) means an edit to one item cannot invalidate an
  unrelated item's certificate.

- REQ-4 (body-position hole token `?N` — parser): the parser accepts a `?N` token
  in fn-BODY statement position ONLY as a structural hole. A holed item is a
  well-formed AST that carries its open holes; it is NEVER certified (an item with
  any open hole is L0-equivalent until every hole is filled). Derived from §5.1
  (`body = hole ?0`). NEW capability — the parser today has no hole support.

- REQ-5 (open-hole validator — a holed item never certifies): the check pipeline
  reports each open hole as an OPEN GOAL and short-circuits the holed item to a
  non-certifying cert (no lowering, no verus) BEFORE the L3 path, exactly as the
  vacuity gate short-circuits a rejected item. A holed item carries `Level::L0`
  with an `OpenHole` reject cause and the hole's address as the obligation. Derived
  from §5.1 + the existing `gate_fn` short-circuit pattern.

- REQ-6 (`forge fill <addr> <code>` — fill a hole + re-check): a specialization of
  `edit` whose address names a hole (`?N` at a body position). It splices the code
  at the hole address, re-parses, re-checks the item, and prints the new goal state
  (which may surface NEW holes the filled code introduces, per the §5.1 dialogue).
  Derived from §5.1 + Appendix B (`forge fill <hole-addr> <code>   fill a hole;
  returns new goal state`).

- REQ-7 (determinism + Result discipline): every verb is deterministic (R-CODE-5 —
  the goal render is a pure function of the cert collection + AST; the splice is a
  pure function of the span + replacement text) and returns `Result<_, ForgeError>`
  with no panic on a bad address / malformed hole (R-CODE-2; reuse
  `address::AddressError`). Derived from `goal.md` R-CODE-2/R-CODE-5 + §5.3.

## Acceptance criteria

- AC-1 (battery view fidelity): `forge battery conformance/sum.th` reports the SAME
  vacuity verdict and mutation kill-ratio that `forge check conformance/sum.th`
  computes internally — the standalone verb is a VIEW, not a re-derivation. Anchored
  to `conformance/sum.cert.json` (`mutants 17/18`, non-vacuous) — NEVER copied from
  the verb's own output (R-CHAR-3).

- AC-2 (goal render — discharged): `forge goal sum` on a clean `sum.th` renders
  `ALL GOALS DISCHARGED` + the L3 level + the §7 battery line, matching the
  obligations in `conformance/sum.cert.json`.

- AC-3 (goal render — counterexample): `forge goal <item>` on an item with a failed
  obligation renders the concrete witness from the `ObligationResult.diagnostic` +
  `location` (the §5.1 `lo=3, hi=3, mid=3` shape), never a bare "verification
  failed".

- AC-4 (edit by address): `forge edit binary_search.loop#1.inv#2 --replace "<text>"`
  resolves the address via `thermite_syntax::address::resolve` against
  `conformance/binary_search.th`, splices the new clause at that span, and the
  re-emitted file re-parses to the SAME address set with the new `inv#2` text. A
  bad address (`binary_search.loop#9`, in `conformance/address/binary_search.addresses.json`
  `must_error[]`) yields a structured `AddressError`, never a panic.

- AC-5 (hole never certifies): a `.th` file whose fn body is `?0` parses, and
  `forge check` / `forge goal` on it reports the item as L0 with an OPEN GOAL at
  `<fn>.?0` and the project assurance as non-certified. No lowering, no verus.

- AC-6 (the §5.1 dialogue as golden scenario): the full §5.1 `binary_search`
  dialogue is the end-to-end golden conformance scenario — declare with `body = ?0`,
  `fill` the loop skeleton + invariants introducing `?1 ?2`, observe `?1 discharged`
  and `?2 open` with its counterexample, guard the branch via a final `fill`/`edit`,
  observe `ALL GOALS DISCHARGED ✓ binary_search certified L3` with the battery line.
  This scenario is the acceptance oracle for the whole component (golden fixture
  under `conformance/goal/binary_search.dialogue.json`, hand-derived from §5.1 —
  R-CHAR-3, NEVER regenerated from the verbs).

## Architecture

The component is a thin REPL/view layer (`forge/src/goal_repl.rs`-ish) over three
SHIPPED substrates, plus one parser/validator extension.

**Substrate 1 — semantic addressing (SHIPPED).** `thermite_syntax::address` is the
operand layer for `edit`/`fill`. `pub fn resolve in address.rs` maps an address
string to an `AddressEntry { addr, kind, surface_keyword, text }` or a structured
`AddressError` (`Malformed` / `NotFound`), and `pub fn addresses_of in address.rs`
enumerates every address in document order. Addresses are stable under unrelated
edits (a block's address is a function of its position within its enclosing item
only — semantic-addressing.md REQ-5), which is exactly what makes `edit` and the
per-item proof cache (§5.3) sound. The address namespace covers `fn`/`spec fn`
roots and `loop#N`/`inv#M`/`dec` inner nodes; `edit` operates on the `inv`/`dec`
nodes whose `text` field the resolver already returns. The hole address space
(`<fn>.?N`) is NEW (REQ-4) and extends this namespace at body-statement positions.

**Substrate 2 — per-obligation results + counterexamples (SHIPPED).** `forge check`
emits, per item, a `manifest::Certificate` carrying a `Vec<ObligationResult>` where
each `ObligationResult` (`manifest.rs`) is `Discharged` or `Failed` with a
`location` (`file:line:col`) and a `diagnostic` (verus's `error: <clause>` — the
§5.1 "counterexample, not adjective"). `pub fn check_file in check.rs` is the
pipeline; `pub fn check_file_with_options in check.rs` is the configurable entry.
`forge goal` (REQ-2) is a RENDER over this collection: it groups obligations by
item, formats discharged vs failed-with-witness, and lists open holes. It adds NO
verification — `render_human in cli.rs` is the existing precedent for a cert
renderer; `goal` is a goal-state-shaped sibling.

**Substrate 3 — the battery, inside the gate (SHIPPED).** The §7 anti-Goodhart
battery already runs inside the check gate per item: `vacuity::triage` (structural,
in `gate_fn`), `vacuity_solver::solver_vacuity_check` (solver-backed, after the
gate, before L3), and `fn mutation_score in check.rs` (re-lower-and-re-verify each
`mutation::generate` mutant against the same contract, kill-ratio vs
`mutation::MUTATION_FLOOR`). `forge battery [item]` (REQ-1) is a THIN verb that
runs the same per-item pipeline and reports just these verdicts standalone — a view
over `check_file_with_options`, not a new battery.

**Extension — holes + the splice (NEW, the research spike).** A hole is a
body-position-only `?N` token (REQ-4). The minimal v1:
- The lexer emits a `Hole(N)` token for `?<digits>`; the parser accepts it ONLY in
  fn-body statement position (an `Item::Fn` body block statement). A `?N` anywhere
  else (expression position, a spec clause, a `spec fn`) is a parse error. No
  nested holes-in-holes ordering games (a hole's filled code may itself introduce
  new holes, but the v1 parser does not track containment beyond document-order
  numbering).
- A holed `FnItem` carries its open holes; `addresses_of` gains a `<fn>.?N` address
  per hole (REQ-4) so `fill`/`edit` can name them.
- The validator (REQ-5) short-circuits a holed item to a non-certifying L0 cert
  with an `OpenHole` reject cause BEFORE lowering — the SAME short-circuit shape
  `gate_fn` uses for a vacuity rejection (`GateOutcome`). A holed item NEVER reaches
  verus; it can never accidentally certify.
- `forge edit <addr> --replace <code>` (REQ-3) resolves the address, computes its
  source span, splices the replacement text into the FILE IN PLACE, re-parses, and
  re-checks the affected item. `forge fill <addr> <code>` (REQ-6) is `edit` whose
  address is a hole — splice at the `?N` position, re-check, print the new goal
  state. Both operate on the file in place and print the new goal state (REQ-6).

**v1 scope pinned honestly** (the research-spike boundary, §5.1 + R-DEFER-5):
- Holes are fn-BODY STATEMENT position only. No holes in expressions, spec clauses,
  signatures, or `spec fn`.
- No nested-hole ORDERING semantics: holes are numbered in document order; filling
  a hole that introduces new holes re-numbers on re-parse. There is no incremental
  hole-id stability across fills in v1 (the address is re-derived each turn — the
  oracle re-presents, §5.1 property 1).
- `fill`/`edit` mutate the file in place and re-run the WHOLE-ITEM check (the v0.1
  check is whole-item per §13/issue #21); they do not do incremental obligation-
  level re-solving. The proof cache (§5.3) makes the re-check cheap for unaffected
  items.
- `goal`/`battery` add NO verification; they are pure views over the existing cert
  collection and battery verdicts.

## Verification

- `cargo test -p forge` — unit tests for the goal render (discharged / counter-
  example / open-hole shapes), the battery view (equals the in-gate verdict), the
  address-splice (round-trips to the same address set), and the hole short-circuit
  (a holed item is non-certifying, never lowered).
- `cargo test -p thermite-syntax` — the `Hole(N)` lexer token, the body-position-
  only parse acceptance + the expression/clause-position parse REJECTION, and the
  `<fn>.?N` address enumeration.
- Conformance: `conformance/goal/binary_search.dialogue.json` (AC-6, the §5.1
  golden dialogue — hand-derived from the thesis, R-CHAR-3) drives the end-to-end
  scenario; `conformance/sum.cert.json` anchors the battery view (AC-1) and the
  discharged goal render (AC-2); `conformance/address/binary_search.addresses.json`
  `must_error[]` anchors the bad-address path (AC-4).
- Gauntlet (R-DEFER-6): `cargo test -p forge`, `cargo test -p thermite-syntax`,
  `cargo clippy -p forge -p thermite-syntax --all-targets -- -D warnings`,
  `cargo fmt --check`, plus the conformance corpus where `forge`/`thermite-lower`
  is touched.

## Increment plan

The build is sequential (R-DEFER-7); the three increments are ordered by dependency
(views first, then the addressing splice, then the parser/validator spike):

- **(i) `forge battery` + `forge goal`** — VIEWS over existing machinery, smallest.
  Two new verbs in `cli.rs` + a `goal_repl.rs` renderer; no new verification, no
  parser change. Discharges REQ-1, REQ-2 (and AC-1, AC-2, AC-3).
- **(ii) `forge edit`** — addressing + source-text splice. Resolves an `inv`/`dec`/
  loop address, splices the replacement at its span, re-emits + re-checks. No holes
  yet. Discharges REQ-3 (and AC-4).
- **(iii) holes + `forge fill`** — the research spike: the `?N` lexer/parser token
  (thermite-syntax), the open-hole validator short-circuit (forge check), the
  `<fn>.?N` address, and the REPL fill loop. Discharges REQ-4, REQ-5, REQ-6 (and
  AC-5, AC-6 — the §5.1 golden dialogue).

REQ-7 (determinism + Result discipline) is a cross-cutting constraint discharged
across all three increments.

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| Substrate: semantic addressing | SHIPPED | `pub fn resolve` + `pub fn addresses_of` in `address.rs` map address ↔ `AddressEntry`, bad addr → `AddressError` (no panic). Non-test consumer: `forge edit`/`fill` are the design-intended consumers (NOT-STARTED below); addressing is ALREADY consumed by `tests/conformance.rs` against `conformance/address/binary_search.addresses.json`. The operand layer for REQ-3/REQ-6 is present and verified. |
| Substrate: per-obligation results + counterexamples | SHIPPED | `pub struct ObligationResult` + `pub enum ObligationStatus` (`Discharged`/`Failed` with `location` + `diagnostic`) in `manifest.rs`; produced by `pub fn check_file` in `check.rs`. Non-test consumer: `fn run_check` → `fn render_human` in `cli.rs` (renders the cert collection today). The goal render (REQ-2) is a VIEW over this; the substrate is shipped + verified against `conformance/sum.cert.json`. |
| Substrate: the §7 battery inside the gate | SHIPPED | `vacuity::triage` + `vacuity_solver::solver_vacuity_check` + `fn mutation_score` in `check.rs` (kill-ratio vs `mutation::MUTATION_FLOOR`) all run per item inside `pub fn check_file_with_options`. Non-test consumer: `fn run_check` in `cli.rs` (the gate result drives the exit code). `forge battery` (REQ-1) exposes it standalone; the substrate is shipped + anchored to `conformance/sum.cert.json` (`mutants 17/18`). |
| REQ-1 (`forge battery [item]`) | SHIPPED | `Command::Battery` in `cli.rs` → `run_battery` → `goal_repl::render_battery`, a VIEW over each cert's `contract_quality` (the §7 verdicts the gate computed; NO accessor — the cert already carries them separably, AC-1 satisfied as a view). Verified: `forge/tests/goal_repl.rs::battery_view_matches_check_verdicts` — non-vacuous booleans anchored to `conformance/sum.cert.json` (oracle fields), the kill-ratio asserted CROSS-VERB (battery == check, since the ratio is oracle-EXCLUDED per `conformance/README.md` — the golden `17/18` is illustrative, the live tool computes `7/7`; R-CHAR-3). |
| REQ-2 (`forge goal <item>`) | SHIPPED | `Command::Goal` in `cli.rs` → `run_goal` → `goal_repl::render_goal`, the §5.1 four-part render (given/want from the re-parsed contract; per-obligation status + concrete counterexample from `cert.obligations`; a clean L3 cert → `ALL GOALS DISCHARGED`). Open holes ARE rendered: `render_goal_item` emits the §5.1 `holes:` section from `FnItem.holes` (increment (iii); Amendment 2026-06-12). Verified: `forge/tests/goal_repl.rs::goal_render_discharged_for_sum` (AC-2) + the unit `goal_render_counterexample` (AC-3, the §5.1 `lo=3,hi=3,mid=3` witness shape). |
| REQ-3 (`forge edit <addr> --replace`) | SHIPPED | `Command::Edit` in `cli.rs` → `run_edit` → `goal_repl::edit_file`: resolves via `thermite_syntax::address::resolve`, finds the addressed node's byte span (`span_of_address`, mirroring `addresses_of`'s traversal since `AddressEntry` carries no span), splices the replacement SOURCE TEXT at that span, writes the file, re-parses + re-checks the item, prints the new goal state. v1 splices an `inv`/`dec`/`loop`/`fn` span; a bad address → structured `ForgeError::Usage` (no panic). Verified: `forge/tests/goal_repl.rs::edit_splices_clause_and_rechecks` (AC-4 round-trip) + `edit_bad_address_is_honest_error` (the `must_error[]` bad-address path, file left untouched). |
| REQ-4 (body-position hole `?N` — parser) | SHIPPED | The `?N` HOLE token (`lexer::TokKind::Hole(u32)` + `lex_hole` — `?` + a digit run; a bare `?` is a stray-char diagnostic, no `?`-operator §2.3) + parser acceptance in EXEC-fn-body statement position ONLY (`parse_block`'s `TokKind::Hole` arm → `parse_hole`, gated by `Parser.fn_body_depth > 0` incremented around the exec-fn body parse in `parse_fn`, NOT a `spec fn` body) + the AST form (`struct Hole { number, span }` + `FnItem.holes: Vec<Hole>`, document order — PURELY ADDITIVE, NO new `Stmt` variant so the workspace `match Stmt` is untouched) + the `<fn>.?N` address (`address::AddrKind::Hole`, emitted in `addresses_of`, accepted in `validate_segments`). A `?N` in a `spec fn`/expr/clause/signature is a structured `SyntaxError::HoleOutsideFnBody` / unexpected-token error, never a panic. Non-test consumer: `forge::goal_repl::render_goal_item` (the §5.1 `holes:` section) + `span_of_address` (the fill splice target). Verified: `forge/tests/goal_repl_fill.rs::fn_body_hole_parses_clean_and_records_the_hole` (clean holed AST, one `?0`), `holes_in_nested_blocks_are_accepted_in_document_order`, `hole_outside_fn_body_statement_position_is_a_structured_parse_error_not_a_panic` (AC-5 parser half), `hole_address_resolves_and_bad_hole_address_is_structured_error`. |
| REQ-5 (open-hole validator — never certifies) | SHIPPED | `forge::check`'s per-item loop short-circuits a holed `FnItem` (any `f.holes`, via the shared `goal_repl::open_hole_reason` — #195) to a non-certified `Certificate::rejected` (`Level::L0`) with a `RejectReason { cause: "OpenHole", detail: <every `<fn>.?N` address + the first open goal> }` BEFORE the #6 gate / lowering / verus — the SAME short-circuit shape the vacuity gate / mutual-recursion reject uses (`RejectReason` reused, NO new variant needed: cause is a string). A holed item NEVER reaches verus, never certifies. `render_goal` surfaces it as the §5.1 open GOAL. Non-test consumer: the `forge check`/`forge goal` exit path (`cli::run_check`/`run_goal`). Verified: `forge/tests/goal_repl_fill.rs::holed_item_never_certifies_open_hole_l0_no_verus` (AC-5 — `forge check --json` reports L0 `OpenHole`, runs WITHOUT verus since the short-circuit precedes it). |
| REQ-6 (`forge fill <addr> <code>`) | SHIPPED | `Command::Fill` in `cli.rs` → `run_fill` → `goal_repl::fill_hole`: a SPECIALIZATION of `edit_file` whose address names a `?N` hole — resolves `<fn>.?N` (a non-hole address is an honest `ForgeError::Usage` directing to `edit`), splices `code` at the hole's `?N` token span (reusing the increment-(ii) `splice`), re-emits, re-parses, re-checks the item, and prints the new GOAL STATE (which may surface NEW holes the fill introduced — the §5.1 loop). Verified: `forge/tests/goal_repl_fill.rs::fill_introducing_new_holes_re_presents_them` (the §5.1 `fill ?0 … ?1 ?2` step), `fill_on_a_non_hole_address_is_an_honest_error`, `fill_closing_the_hole_certifies_l3` (verus L3 terminal), and `ac6_binary_search_dialogue_structural_oracle` (AC-6 — the full §5.1 dialogue, structural oracle from `conformance/goal/binary_search.dialogue.json`). |
| REQ-7 (determinism + Result discipline) | SHIPPED | All four verbs (`goal`/`battery`/`edit`/`fill`) return `Result<_, ForgeError>`; the renders are pure functions of the cert collection + AST, the splice a pure function of the span + replacement text (R-CODE-5); a bad/unresolvable address is a structured error, never a panic (R-CODE-2 — `goal_repl::address_usage`). `fill_hole` carries the constraint for increment (iii) (Amendment 2026-06-12): `fill_on_a_non_hole_address_is_an_honest_error` pins the structured-error path. |
