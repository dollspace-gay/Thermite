# `conformance/goal/` — the §5.1 goal-state dialogue oracle

This directory holds the acceptance oracle for the Lean-style goal-state REPL
(`forge goal` / `forge fill` / `forge edit` / `forge battery` — design
`.design/forge/goal-repl.md`, thesis `thermite-design.md` §5/§5.1, Appendix B).

## The dialogue golden

`binary_search.dialogue.json` is the **AC-6** acceptance oracle: the verbatim
§5.1 `binary_search` dialogue (declare with `body = ?0` → fill the loop skeleton
introducing `?1 ?2` → one discharged / one open-with-counterexample → guard the
branch → `ALL GOALS DISCHARGED ✓ binary_search certified L3`).

It is **hand-derived from `thermite-design.md` §5.1's hand-written dialogue text +
the design doc's AC-6** — NEVER regenerated from running the verbs (`goal.md`
R-CHAR-3). A fabricated golden copied from the tool's own output would be a false
anchor (the same rule the cert-oracle README states).

## Structural-oracle vs illustrative (R-CHAR-3 honesty)

`thermite-design.md` §5.1 is an ILLUSTRATIVE narrative — its concrete numbers are
not all assertable against the real prover. The golden's `oracle_kind` field and
its `expect_structure` / `illustrative_not_asserted` blocks pin the split:

**STRUCTURAL ORACLE (asserted — the acceptance criteria):**
- the **given/want** lines are present and carry the contract's `req`/`ens` text
  (turn 1: `given` contains `sorted(haystack)`);
- a holed item shows its **open holes** as the `holes:` section and is
  **NOT CERTIFIED** (`Level::L0`, `reject_cause: OpenHole`) — it never reaches
  verus (REQ-5);
- `forge fill` at a hole **closes that hole**, and a fill whose code introduces
  new holes **re-presents the new open holes** (the §5.1 fill loop);
- an open obligation carries a **concrete counterexample** (a non-empty
  diagnostic), never a bare adjective (§5.1 property 2);
- once **every hole is closed** and the bodies are correct, the item renders
  `ALL GOALS DISCHARGED` and certifies **L3** with a **non-vacuous** battery line.

**ILLUSTRATIVE (NOT asserted — the design's narrative numbers):**
- `solver_time_ms` (§5.1 `0.4s`) — wall-clock, non-deterministic, EXCLUDED from
  the oracle (the cert-oracle contract, `conformance/README.md`);
- the **mutant kill ratio** (§5.1 `23/24`) — the live tool computes its own ratio
  for the actual mutant set; the asserted fact is `non-vacuous`, not the ratio;
- the **exact `?N` numbers across turns** — v1 re-numbers holes on each re-parse
  (no incremental hole-id stability — §5.1 property 1, "the oracle re-presents"),
  so the asserted fact is the open/discharged **transition + count**, not literal
  `?N` identity across turns;
- the **exact counterexample witness** (§5.1 `lo=3, hi=3, mid=3`) — the design's
  illustrative witness; the asserted fact is that the witness is CONCRETE.

The runner `forge/tests/goal_repl_fill.rs` drives the dialogue against the real
toolchain (real verus where available) and asserts only the structural oracle.
