# Thermite Semantic Addressing (stable block addresses)
<!--
tier: 3-component
status: draft
audited-sha: 32533e4bfe375a008ee049f5401249c99b7747fe (bootstrap pin: decision 4 — doc-last-touch, NOT verified-current; backlog #262)
governs: thermite-syntax/src/address.rs
thesis-refs:
  - thermite-design.md §4.3
  - thermite-design.md §2 (pillar 5 locality)
  - thermite-design.md §5.3 (per-item content-addressed proof cache)
references:
  - conformance/binary_search.th
  - conformance/sum.th
  - conformance/address/ (address-resolution fixtures)
-->

## Summary

Every item and addressable block in a Thermite program has a **stable semantic
address** — a deterministic, positional path like `binary_search.loop#1.inv#2`
(§4.3). Addresses are the operands of `forge edit <addr>` / `forge insert-after`
and the keys of the per-item proof cache (§5.3), so they must be **stable under
unrelated edits**: editing one function must not renumber another's blocks
(pillar 5: "a proof must not break because of an unrelated edit"). This doc pins
the EXACT numbering scheme and gives the full address list for both corpus
programs as the oracle.

This doc is GREENFIELD / FORWARD-LOOKING: no `address.rs` exists. Every REQ is
**NOT-STARTED**, blocked on issue #3. One numbering reading (the corpus oracle
string vs verbatim source) is additionally pending blocker **#26** (OQ-1).

## Requirements

- **REQ-1 (address grammar):** An address is a `.`-separated path of segments.
  The root segment is the **function name** (`binary_search`, `sum`, `spec_sum`).
  Subsequent segments are `KIND#N` where `KIND ∈ {loop, inv, dec}` and `N` is a
  1-based ordinal (except `dec`, which is unique within its loop and carries no
  ordinal). Derived from §4.3 (`binary_search`, `binary_search.loop#1`,
  `binary_search.loop#1.inv#2`).

- **REQ-2 (loop/while numbering — 1-based, source order, per-function):** Within
  a function body, every `loop` and `while` is numbered `loop#N`, `N` starting at
  1, in source-text order, counting `loop` and `while` in the same sequence
  (a `while` is `loop#1` if it is the first loop construct — see the `sum`
  worked example). Numbering is scoped to the enclosing function and is
  INDEPENDENT of any other item. Derived from §4.3 + corpus.

- **REQ-3 (inv numbering — 1-based, source order, per-loop):** Within a loop, the
  `inv` clauses are numbered `inv#M`, `M` starting at 1, in source-text order.
  Scoped to the enclosing loop only. Derived from §4.3 (`...loop#1.inv#2` is the
  second invariant) + corpus. The exact resolution of the oracle string
  `binary_search.loop#1.inv#2` is pending blocker **#26** (OQ-1).

- **REQ-4 (dec address — unique per loop):** Each loop's single `dec` clause is
  addressed `<loop-addr>.dec` (no ordinal — exactly one `dec` per loop, §4.1).
  Derived from §4.3 + §4.1.

- **REQ-5 (stability under unrelated edits — STRUCTURAL/POSITIONAL):** An address
  is a function of the node's position WITHIN ITS ENCLOSING ITEM only, never of
  any sibling item or any whole-program offset. Consequences (all mechanically
  testable): (a) adding, removing, or editing function `f` does NOT change any
  address inside function `g`; (b) editing the BODY of `binary_search.loop#1`
  (e.g. changing a statement) does not change `binary_search.loop#1.inv#2`;
  (c) renaming a function changes only its own root segment. Numbering is purely
  structural/positional within the enclosing item (§4.3: "items are parsed
  independently"; pillar 5: "a proof must not break because of an unrelated
  edit"; §5.3: "an edit to `f` cannot invalidate `g`'s certificate"). Derived
  from §4.3 + §5.3 + pillar 5.

- **REQ-6 (deterministic + resolvable both ways):** Address computation is
  deterministic (same AST → same addresses; R-CODE-5) and supports both
  directions: given an `Item`/block, produce its address; given an address
  string, resolve it to the node (or a structured "no such address" error, no
  panic — R-CODE-2). Derived from §4.3 (`forge edit <addr>` resolves an address
  to a node) + R-CODE-2/R-CODE-5.

## Acceptance criteria

- **AC-1 (`binary_search` full address list):** Parsing
  `conformance/binary_search.th` and addressing it yields EXACTLY:
  - `binary_search` — the function
  - `binary_search.loop#1` — the `loop`
  - `binary_search.loop#1.inv#1` → `lo <= hi && hi <= haystack.len()`
  - `binary_search.loop#1.inv#2` → `forall_below(haystack, lo, |x| x < needle)`
  - `binary_search.loop#1.inv#3` → `forall_from(haystack, hi, |x| x > needle)`
  - `binary_search.loop#1.dec` → `hi - lo`

  **NOTE on the corpus oracle string `binary_search.loop#1.inv#2` (blocker #26):**
  the task brief and `tooling/spec-routes.toml`
  (`conformance_ops = ["binary_search.loop#1.inv#2"]`) cite `inv#2` resolving to
  `forall_from(haystack, hi, |x| x > needle)`. In the verbatim corpus source the
  SECOND `inv` is `forall_below(...)` and the THIRD is `forall_from(...)`. This
  doc pins the **scheme** (1-based source order → `inv#2` = `forall_below`,
  `inv#3` = `forall_from`); blocker **#26** holds the discrepancy for the
  orchestrator/critic to pin the `conformance/address/` fixture before the
  builder runs. The AC fixture asserts the scheme against the verbatim source.
  (REQ-1, REQ-2, REQ-3, REQ-4)

- **AC-2 (`sum` full address list):** Parsing `conformance/sum.th` and addressing
  it yields EXACTLY (for `fn sum`; `spec fn spec_sum` has no loops so only the
  root `spec_sum`):
  - `sum` — the function
  - `sum.loop#1` — the `while i < xs.len()` (the `while` is the first loop
    construct, so `loop#1`, per REQ-2)
  - `sum.loop#1.inv#1` → `i <= xs.len()`
  - `sum.loop#1.inv#2` → `acc == spec_sum(&xs[..i])`
  - `sum.loop#1.inv#3` → `acc <= i as u64 * u32::MAX as u64`
  - `sum.loop#1.dec` → `xs.len() - i`
  - `spec_sum` — the spec function (no addressable inner blocks; its `dec` is a
    spec-fn measure, NOT a loop `dec` — see OQ-2)

  Tied to `conformance/address/` fixtures. (REQ-1..REQ-4)

- **AC-3 (stability under unrelated edit):** A `conformance/address/` fixture
  with both corpus functions in one file: deleting/editing `sum` leaves every
  `binary_search.*` address unchanged, and editing the body statements of
  `binary_search.loop#1` leaves `binary_search.loop#1.inv#2` resolving to the same
  expression. (REQ-5)

- **AC-4 (round-trip resolution + no-panic on bad address):** For each address in
  AC-1/AC-2, address-of-node and resolve-node-from-address are inverse;
  resolving a nonexistent address (`binary_search.loop#9`, `sum.loop#1.inv#7`)
  returns a structured error, never a panic. (REQ-6)

## Architecture

`thermite-syntax/src/address.rs` computes addresses over the AST (`ast.md`
REQ-8 marks the addressable nodes: `Item`, `Loop`/`While`, `inv`/`dec` clauses).
The scheme is purely structural:

```
address(Item f)              = f.name
address(Loop/While L in f)   = address(f) + ".loop#" + (1-based index of L
                                 among loop+while constructs in f, source order)
address(inv#M of L)          = address(L) + ".inv#" + M        (1-based, source order)
address(dec of L)            = address(L) + ".dec"
```

The index counters reset at each enclosing scope: loop indices count within a
function, `inv` indices count within a loop. Nothing in the computation reads a
sibling item or a whole-file offset — that is the **stability property** (REQ-5):
the address of a block is a function of its position inside its own item only, so
unrelated edits cannot renumber it. This is exactly what the per-item
content-addressed proof cache needs (§5.3: "an edit to `f` cannot invalidate
`g`'s certificate"); the address is (part of) the cache key.

Resolution (REQ-6) parses an address string into segments and walks the AST:
match the root function by name, then index into its loops, then into the loop's
`inv`/`dec`. A missing segment yields a structured `SyntaxError`/`AddressError`
(no panic, R-CODE-2). Computation is deterministic (R-CODE-5): same AST → same
addresses, every run.

`while` and `loop` share the `loop#N` namespace (REQ-2) — the address abstracts
over the surface keyword, matching §4.3's examples which use `loop#1` generically.
In `sum` the `while` is therefore `sum.loop#1`.

## Verification

`cargo test -p thermite-syntax` against `conformance/address/`:
- the full address list for `binary_search.th` (AC-1) and `sum.th` (AC-2),
  including the `inv#2`/`inv#3` resolutions;
- the stability fixture (AC-3): unrelated-item edit and same-loop body edit leave
  the other addresses fixed;
- the round-trip + bad-address no-panic checks (AC-4).

Expected address lists are hand-derived from §4.3's scheme + the verbatim corpus,
NEVER copied from `address.rs`'s output (R-CHAR-3). The corpus string
`binary_search.loop#1.inv#2` (in `tooling/spec-routes.toml conformance_ops`) is
the external oracle for the scheme; its exact resolution is pinned by blocker #26.

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (address grammar) | SHIPPED | `address.rs` segments = fn name + `loop#N`/`inv#M`/`dec`; `validate_segments` + `AddressEntry`; address oracle passes. |
| REQ-2 (loop numbering) | SHIPPED | `collect_block_loops` numbers loops 1-based source order, `while`+`loop` shared; `sum` `while` = `sum.loop#1` (sum address oracle). |
| REQ-3 (inv numbering) | SHIPPED | `emit_loop` numbers `inv#M` 1-based source order; blocker #26 resolved — test asserts `inv#2`=`forall_below`, `inv#3`=`forall_from`. |
| REQ-4 (dec address) | SHIPPED | `emit_loop` emits `<loop>.dec` (no ordinal); resolves to `hi - lo` / `xs.len() - i` (address oracle). |
| REQ-5 (stability under unrelated edits) | SHIPPED | numbering reads only the enclosing item; test `address_stability_under_unrelated_edit`. |
| REQ-6 (deterministic + bidirectional) | SHIPPED | `addresses_of` (node→addr) + `resolve` (addr→node); `must_error` addresses → `AddressError`, never panic. |

## Open questions (for the orchestrator)

- **OQ-1 (BLOCKER #26 — corpus oracle string vs verbatim source):** The task
  brief and `spec-routes.toml` state `binary_search.loop#1.inv#2` resolves to
  `forall_from(haystack, hi, |x| x > needle)`. Under the 1-based source-order
  scheme (REQ-3) applied to the verbatim `conformance/binary_search.th`, the
  SECOND `inv` is `forall_below(...)` and `forall_from(...)` is `inv#3`. Either
  (a) the oracle string is illustrative and the scheme is authoritative (this
  doc's reading — `inv#2`=`forall_below`, `inv#3`=`forall_from`), or (b) the
  corpus / oracle expects a different ordering. This is a genuine grounding
  ambiguity not resolvable from §4.3 + the corpus alone (§4.3's own
  `binary_search.loop#1.inv#2` example does not say which invariant it is). Filed
  as blocker **#26** so the orchestrator/critic pins the `conformance/address/`
  fixture before the builder runs. AC-1 commits to the scheme reading; if (b) is
  intended, this doc must be amended (R-SPEC-4).

- **OQ-2 (spec-fn `dec` is not a loop `dec`):** `spec_sum` has a top-level
  `dec xs.len()` that is the SPEC-FUNCTION decreases-measure, not a loop `dec`.
  REQ-4 addresses only LOOP `dec`s. Whether a spec-fn measure needs its own
  address (`spec_sum.dec`?) is unspecified by §4.3 (which only shows loop
  addresses). AC-2 currently gives `spec_sum` no inner addresses. Recorded;
  resolvable when forge's edit surface needs it, not a v0.1-parser blocker.

- **OQ-3 (nested loops):** The corpus has no nested loops; REQ-2's "within a
  function" wording means a nested loop would also count in the function-level
  `loop#N` sequence (flat numbering), which matches §4.3's flat examples. If
  nesting needs a hierarchical scheme (`loop#1.loop#1`), §4.3 does not say.
  Recorded as a v0.1 non-issue (no corpus nesting); not a blocker.
