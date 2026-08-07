# Record-state loop lifecycle primitives

<!--
tier: 3-component
status: shipped
decision: a single while-loop over one typed recursively finite record cell is validated by exact entry, recursive leaf-preservation, abstract exit, and full generated-result obligations; the authored invariant is the explicit fixpoint summary
governs:
  - thermite-lower/src/lower.rs
  - thermite-lower/src/lib.rs
  - thermite-lower/tests/function_context_loop_body.rs
  - thermite-tv/src/exec_stmt_encode.rs
  - thermite-tv/src/obligation.rs
  - thermite-tv/src/lib.rs
  - thermite-tv/tests/record_state_loop_tv.rs
  - forge/src/body_tv.rs
  - forge/tests/verified_build.rs
  - conformance/verified-build/record_state_loop.th
audited-content-sha256: 87da3fc6701636547da86f68208116144864f94ef66490b56f926bc77d2d4353 (re-pinned 2026-08-07 after source-oriented Forge commands resolved canonical packages through one shared front door; existing single-file behavior remains regression-covered)
extends:
  - .design/verified/loop-tv.md
  - .design/build/nested-aggregate-lifecycle.md
  - .design/build/kernel-primitives.md
-->

## Decision

Thermite admits a record-state loop when its complete verification frame is
derivable from source:

- the loop is one terminating `while` with non-vacuous `inv` clauses and `dec`;
- its state is one explicitly typed local whose type belongs to the recursively
  finite structural-record closure;
- the loop body contains straight-line assignments and conditionals over exact
  record field paths, including nested fields and an optional terminal fixed-array
  index already admitted by aggregate lifecycle TV;
- the loop is the last statement and the body tail returns that record cell; and
- the invariant explicitly summarizes every mutated or collateral property a
  caller expects after an arbitrary number of iterations.

This is an allocation-free language primitive. It does not provide a scheduler,
allocator, device model, boot path, or any other kernel policy.

## Independent semantics

The loop recognizer collects the root of each field assignment rather than
rejecting every non-scalar target. Prefix threading records the exact nominal
type of the local record. One arbitrary iteration begins with that record as a
symbolic input, then reuses the independent nested-aggregate reconstruction:
each changed field is installed at its precise typed path and every enclosing
sibling is copied from the immediately preceding record value.

Preservation compares the production result with the independent step at every
recursively finite leaf. Scalar leaves use bounded equality. Fixed-array leaves
use extensional sequence-view equality. Nested records recurse through their
complete field inventory. Consequently a dropped write, wrong nested value,
dependent-write reorder, wrong field, or collateral sibling mutation changes a
real Verus postcondition.

The invariant remains the explicit fixpoint interface. TV does not fabricate a
closed form for an unbounded loop and does not silently infer a collateral
property absent from `inv`.

## Four obligations

Record-state loop TV discharges four independent obligations:

1. **Entry:** the source prefix establishes every invariant clause.
2. **Preservation:** under `inv && cond`, the exact generated one-step body equals
   the independently reconstructed record at every leaf and re-establishes `inv`.
3. **Abstract exit:** opaque loop-head state satisfying `inv && !cond` entails the
   stated exit claim.
4. **Full result:** the exact production prefix, annotated `while`, decreases
   measure, and tail execute inside a wrapper whose actual result must satisfy
   independently encoded `inv[result] && !cond[result]`.

The fourth obligation closes the former assurance gap where the three while-rule
premises could pass without exercising the complete generated loop body. The
production column uses `lower_exec_body_in_function`, which calls the same
function-context lowering implementation as normal L3 emission, including loop
invariants, decreases, and shape-derived proof aids. A correspondence test pins
that extracted body byte-for-byte inside the complete production output.

## Assurance and acceptance

The focused real-Verus battery proves faithful nested record entry, step, exit,
and result obligations. It rejects wrong nested values, collateral changes,
dependent-write reordering, a dropped loop, a wrong tail, and a missing collateral
invariant.

The policy-free `record_state_loop.th` fixture builds for the generic kernel
target at strict L3, replays its receipt, links a codegen-pinned downstream
consumer, and executes the generated loop. Receipt evidence requires every
contract, exec, loop, and wrapper row to be faithful and every reachable member
to achieve L3. Bound-source tampering invalidates verification.

No bodyful application primitive is below L3 or L4. This increment introduces no
bodyless boundary declaration and therefore no new L1 exception.

## Residual boundary

The frozen subset still excludes multiple record cells in one loop result,
record mutation through a mutable-reference callee inside the loop theory, loops
over exclusive borrowed records, a loop followed by additional stateful
statements, enum-payload lvalues,
index-then-field aliases, multi-exit `break`/`continue`/early-return control, and a
quantified closed-form summary for dynamically updated fixed arrays. Those remain
separate aggregate-effect, aliasing, and quantified-framing increments.
