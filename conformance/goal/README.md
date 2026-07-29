# Goal-state dialogue fixture

`binary_search.dialogue.json` is the acceptance fixture for `forge goal`,
`forge fill`, `forge edit`, and `forge battery`. It is based on the workflow in
`thermite-design.md` §5.1 and `.design/forge/goal-repl.md`.

## What the fixture checks

The runner asserts that:

- goal output includes the relevant `given` and `want` clauses;
- an item with open holes is not certified and reports those holes;
- filling a hole removes it and reports any new holes introduced by the fill;
- an open failed obligation includes a concrete diagnostic;
- closing all holes in a correct body produces `ALL GOALS DISCHARGED`, an L3
  certificate, and a non-vacuous contract result.

## Illustrative values

Several values in the design dialogue are examples rather than stable oracle
data:

- solver time varies by machine and is not compared;
- the mutation ratio depends on the live mutant set;
- holes may be renumbered after each parse;
- the solver may return a different concrete counterexample.

The fixture therefore checks structure and state transitions, not those literal
values. It was derived from the design rather than recorded from the goal-state
implementation (goal.md R-CHAR-3).

`forge/tests/goal_repl_fill.rs` drives the dialogue and applies these assertions.
