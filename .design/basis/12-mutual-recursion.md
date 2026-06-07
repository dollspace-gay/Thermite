# Basis Cluster C11 — Mutual Recursion (the C9 deferral)
<!--
tier: 3-component
status: draft
governs: forge/src/check.rs
governs: thermite-lower/src/lower.rs
thesis-refs:
  - thermite-design.md §4.1
  - thermite-design.md §4.2
  - thermite-design.md §2.3
  - thermite-design.md §7
-->

## Summary

This cluster (crosslink #113) COMPLETES the deferral pinned in
`.design/basis/10-recursion-tuples.md` REQ-6 + OQ-3: **mutual recursion**. C9
shipped direct self-recursion (a `fn` calling itself with a `dec` measure →
Verus `decreases` → L3) and cleanly **L0-rejected** any mutual cycle (`a -> b ->
a`, `a -> b -> c -> a`, …) via `forge::check`'s `mutual_recursion_cycle_fns`
(#110 — the clean-cert-not-crash fix, `RejectReason` cause
`MutualRecursionUnsupported`, rejecting ALL cycle members regardless of `dec`).

C11 REFINES that blanket rejection: a mutual cycle where **EVERY member carries
a `dec` measure** is now ALLOWED. Each cycle fn's `dec` lowers (unchanged, via
the C9 REQ-3 `lower_fn` `decreases` emission) to a Verus `decreases`, and Verus's
own recursive-group termination check PROVES the group terminates across the
cycle (each cross-call strictly decreases the shared / lexicographic measure) →
**L3**. A cycle where some member LACKS `dec` (or the group's measures do not
decrease across the cycle) is STILL cleanly L0 — but the `MutualRecursionUn-
supported` blanket reject becomes CONDITIONAL: it fires only on the missing-`dec`
case, with a refined "missing/insufficient decreases" message (not "unsupported").

This doc ADAPTS to the existing code: the C9 lowering path already emits a valid
Verus mutual group (GROUNDED below — Verus discovers the recursive SCC across the
whole `verus!` block, source-order, NO adjacency requirement), so the ONLY code
change is in `forge::check` (`mutual_recursion_cycle_fns` → conditional). The
full mutual path was GROUNDED with real `verus 0.2026.05.24.ecee80a` (see
Verification) before this contract was pinned.

## The ground truth this doc adapts to

- **`lower.rs` ALREADY emits a valid Verus mutual group.** `lower` (in
  `lower.rs`) emits every `Item::Fn` in **source order inside one `verus! { }`
  block**, and `lower_fn` ALREADY emits `decreases <spec_dec(f.dec, f.params)>`
  for ANY `fn` with `f.dec.is_some()` (C9 REQ-3 — `decreases` after the
  `requires`/`ensures`, before the body). GROUNDED: Verus discovers the recursive
  group (the mutual SCC) **regardless of source order or adjacency** — a probe
  with an unrelated `fn` sitting BETWEEN the two cycle members still certifies
  (`3 verified, 0 errors`). So a mutual cycle whose members each carry `dec`
  ALREADY lowers to the exact Verus form Verus accepts as a mutual-decreases
  group. **No `lower.rs` change is required for the mutual group itself** (REQ-3
  below records this; the lowering is GRANDFATHERED-correct).

- **`forge::check`'s `mutual_recursion_cycle_fns` UNCONDITIONALLY rejects.** The
  current `mutual_recursion_cycle_fns` (in `check.rs`) returns EVERY in-file `fn`
  that sits in a call-graph cycle of size ≥ 2 (`f` reachable from itself through
  a distinct `g`, via `closure::reachable_in_file_fns`), excluding only
  `fn_is_diverge` members. The per-item loop in `check_file_with_rlimit` then
  emits a `Certificate::rejected` (`Level::L0`, cause `MutualRecursionUn-
  supported`) for each such member, `continue`-ing BEFORE lowering / verus —
  regardless of whether the member carries `dec`. THIS is the #110 behavior C11
  refines.

- **The validator's self-call rule (`block_calls_name`) is unchanged.** The
  `thermite-spec` validator (`validator.rs` `run`'s `Item::Fn` arm) flags only a
  DIRECT self-call missing `dec` (`MissingDecreases`); a mutual pair (neither
  calls itself directly) is NOT flagged there. C11 does NOT move the mutual
  missing-`dec` diagnostic into the validator — it stays a `forge::check` cert
  verdict (the cleanest place, where the call graph is already computed). REQ-2
  refines the `forge::check` reject, not the validator.

## Requirements

- **REQ-1 (mutual cycle with `dec` on every member → L3):** A call-graph cycle
  of in-file exec `fn`s (`a -> b -> a`, `a -> b -> c -> a`, …, size ≥ 2) where
  **every member carries a `dec` clause** is ALLOWED: each member lowers
  (unchanged) with its Verus `decreases`, the whole group is emitted in one
  `verus!` block, and Verus's recursive-group termination check proves the cycle
  terminates → the members reach the normal L3 ladder (each member certifies at
  its own level exactly as a single recursive `fn` does). This REQUIRES
  `forge::check`'s `mutual_recursion_cycle_fns` to STOP rejecting dec-complete
  cycles. Derived from §4.1 ("Termination is proved by default") + the C9 REQ-3
  `decreases` lowering (extended from a self-edge to an SCC) + the GROUNDED Verus
  mutual-decreases group (Verification).

- **REQ-2 (the #110 reject becomes CONDITIONAL — missing/insufficient `dec` → L0):**
  `mutual_recursion_cycle_fns` is refined so a cycle is rejected at
  `forge::check` (a `Certificate::rejected`, `Level::L0`, no lowering / no verus)
  **only if at least one cycle member LACKS `dec`** (and is not `fx diverge`).
  The reject cause is RENAMED / the detail REFINED from "mutual recursion is not
  supported in v1" to a missing-decreases diagnostic — e.g. cause
  `MutualRecursionMissingDecreases`, detail naming the offending member(s) and
  the rule "every member of a mutual-recursion cycle must carry a `dec` measure
  (a Verus mutual-`decreases` group), or declare `fx diverge`". A cycle whose
  members ALL carry `dec` but whose measures do NOT decrease across the cycle is
  NOT caught here — it reaches Verus and is rejected as `could not prove
  termination` (a clean L0, the SAME shape as the single-fn non-decreasing L0,
  C9 REQ-4 / AC-2). The `fx diverge` exemption is PRESERVED (the #88 honesty
  exemption — a diverge cycle member lowers with
  `#[verifier::exec_allows_no_decreases_clause]` and is L1-capped, never reaching
  the termination check). Derived from §4.1 ("divergence requires `fx diverge`")
  + §7 (the battery's teeth) + R-DEFER-9 (no proof cheat — a missing-`dec` cycle
  cannot be laundered to L3) + the #110 reject (refined, not removed).

- **REQ-3 (the mutual-group lowering — GRANDFATHERED-correct, no change):** The
  Verus mutual-recursion group is the C9 REQ-3 `lower_fn` `decreases` emission,
  unchanged: `lower` emits every `Item::Fn` in source order inside the single
  `verus! { }` block, and each cycle member with `f.dec.is_some()` carries its
  `decreases <spec_dec(f.dec, f.params)>`. GROUNDED: Verus accepts the group with
  NO adjacency requirement (members may be non-contiguous in source / the verus
  block) and NO grouping syntax (no `verus!`-internal group keyword, no `mod`
  wrapping) — the recursive SCC is discovered automatically. So `lower.rs`
  requires NO change for the mutual group; this REQ records that the existing
  emission IS the contract (any future `lower.rs` change that breaks the
  source-order single-`verus!`-block emission would regress this REQ). The
  cross-call inside a member's body lowers as an ordinary `Expr::Call` (no special
  node), exactly as a self-call does (C9 REQ-3). Derived from §4.1 + the existing
  `lower`/`lower_fn` emission + the GROUNDED no-adjacency Verus behavior.

- **REQ-4 (v1 measure scope — full lexicographic, n-cycles, per-fn `dec`):** v1
  ships the FULL Verus mutual-decreases capability, NOT a restricted form:
  - **n-cycles, not pairs-only.** GROUNDED: a 3-cycle `a -> b -> c -> a` where
    every cross-call decreases the measure certifies L3 (`3 verified, 0 errors`).
    The reject test (REQ-2) and the allow path are both phrased over a cycle of
    any size ≥ 2 (`mutual_recursion_cycle_fns` already computes the whole SCC).
  - **Per-fn `dec` is the measure-supply mechanism.** The user supplies the
    mutual measure the SAME way as a self-recursive fn: each member writes its
    own `dec <measure>` clause. There is NO new "shared measure" surface keyword
    — Verus derives the group's well-foundedness from the per-fn `decreases` (a
    cross-call `f(args) -> g(args')` must show `g`'s `decreases` value, evaluated
    at `args'`, is `<` `f`'s `decreases` value at `args`, in Verus's lexicographic
    order).
  - **Lexicographic measures are supported.** GROUNDED: a pair with comma-list
    measures `decreases n, 1` / `decreases n, 0` (a `(level, tag)` lexicographic
    tuple where the cross-call keeps `n` and drops the tag) certifies L3
    (`2 verified, 0 errors`). Since Thermite's `dec` clause lowers a single
    measure expression today (C9 `spec_dec`), the lexicographic *surface* (a
    comma-separated `dec`) is a SEPARATE surface concern (OQ-2) — but the v1
    common case (each member decreases a shared structural / numeric measure on
    the cross-call, e.g. `is_even`/`is_odd` on `n`, or `eval_node`/`eval_list`
    on the structural ADT value) is FULLY covered by a single per-fn `dec`.
  Derived from §2.3 (one `dec` form, lifted to the cycle) + the GROUNDED 3-cycle
  + lexicographic probes (Verification).

## Acceptance criteria

- **AC-1 (mutual pair with `dec` → L3 — GROUNDED):** an exec mutual pair
  `is_even(n)` / `is_odd(n)`, each calling the other on `n - 1`, each carrying
  `dec n`, each with a NON-VACUOUS `ens` tied to a recursive spec twin (`ens r ==
  s_even(n as nat)`) certifies L3 (Verus `4 verified, 0 errors`). `forge check`
  on this program emits L3 certs for both members (NOT the
  `MutualRecursionUnsupported`/`...MissingDecreases` reject). (REQ-1, REQ-3)

- **AC-2 (mutual cycle, measure does NOT decrease across the cycle → L0 —
  GROUNDED):** a mutual pair where a cross-call does NOT decrease the measure
  (`ping(n)` calls `pong(n)` and vice versa, both `dec n`, trivial `ens r == r`
  so termination is the SOLE obligation) is L0: Verus `could not prove
  termination` (`0 verified, 2 errors`). This cycle is NOT caught by
  `mutual_recursion_cycle_fns` (every member HAS `dec`) — it reaches Verus and is
  rejected there as a clean L0 cert, the SAME shape as the single-fn
  non-decreasing L0 (C9 AC-2). (REQ-2, REQ-4)

- **AC-3 (mutual cycle, a member LACKS `dec` → clean L0 reject — GROUNDED + the
  refined message):** a mutual pair where one member has NO `dec` (and is not `fx
  diverge`) is rejected at `forge::check` as a `Certificate::rejected`
  (`Level::L0`, the REFINED missing-decreases cause), BEFORE lowering / verus —
  so it is a parseable cert verdict, never the raw Verus VIR-error abort
  (GROUNDED: the raw form is `error: recursive function must have a decreases
  clause`, exit-2, empty `--json`; `forge::check` must catch it as a cert). The
  detail names the missing member + the "every cycle member needs `dec` or `fx
  diverge`" rule. (REQ-2)

- **AC-4 (`fx diverge` cycle member → L1-capped, not rejected — the #88
  exemption preserved):** a mutual cycle where a member is `fx diverge` is EXEMPT
  from the missing-`dec` reject (the member is honestly non-terminating, lowers
  with `#[verifier::exec_allows_no_decreases_clause]`, and is L1-capped) — the
  SAME exemption `mutual_recursion_cycle_fns` already applies (`fn_is_diverge`
  members are skipped). A diverge member is NEVER L0-rejected for missing `dec`.
  (REQ-2)

- **AC-5 (3-cycle with `dec` on every member → L3 — GROUNDED):** a 3-cycle `a ->
  b -> c -> a`, every cross-call decreasing the measure, every member carrying
  `dec n`, certifies L3 (Verus `3 verified, 0 errors`). v1 is n-cycles, not
  pairs-only. (REQ-1, REQ-4)

- **AC-6 (no-mutual corpus is byte-stable):** a program with no mutual cycle
  lowers and certifies IDENTICALLY — `mutual_recursion_cycle_fns` returns the
  empty set on a self-recursion-only / acyclic program (the SCC test requires a
  DISTINCT witness `g`), so no existing corpus cert or golden churns. (REQ-2,
  REQ-3)

## Architecture

**The mutual group is the self-recursion `decreases`, lifted from a self-edge to
an SCC.** C9 shipped the per-fn `decreases` emission (`lower_fn`, REQ-3) and the
single-`verus!`-block source-order item emission (`lower`). GROUNDED, these
TOGETHER already produce a valid Verus mutual-decreases group: Verus discovers
the recursive group across the whole block (no adjacency, no grouping syntax —
the probe with an interposed unrelated `fn` certifies). So the verification
MECHANISM for mutual recursion is **already shipped** — C11 does not add a new
lowering. The only thing standing between a dec-complete mutual cycle and L3 is
`forge::check`'s blanket `MutualRecursionUnsupported` reject, which was a
conservative #110 guard (catch the cycle BEFORE Verus so a missing-`dec` mutual
pair is a clean cert, not the raw VIR-error abort). C11 makes that guard
CONDITIONAL on the real failure (a member without `dec`), letting the
dec-complete cycle fall through to the normal lower/verus ladder.

**Where the dec-complete cycle terminates: the per-fn `dec`.** §4.1's "termination
is proved by default" applies to the cycle exactly as to a self-recursive fn:
each member's `dec` measure must strictly decrease on every cross-call (in Verus's
lexicographic order over the comma-list `decreases`). The user supplies the
measure as a per-fn `dec` — the SAME surface as a self-recursive fn (§2.3 "one way
to do everything"; there is no separate "mutual-group measure" keyword). The
common case (a shared numeric/structural measure decremented on each cross-call:
`is_even`/`is_odd` on `n`; an `eval_node`/`eval_list` AST walk on the structural
value) is one `dec` per member. Verus's lexicographic support (GROUNDED) means a
future lexicographic surface (a comma-separated `dec`, OQ-2) is a measure-supply
extension, not a new mechanism.

**Why `forge::check`, not the validator (REQ-2 placement).** The missing-`dec`
mutual diagnostic stays in `forge::check`'s `mutual_recursion_cycle_fns`, NOT the
`thermite-spec` validator. The validator's `block_calls_name` rule (C9 REQ-2)
catches a DIRECT self-call missing `dec`; a mutual cycle is a call-GRAPH property
(an SCC), and `forge::check` already computes the in-file call graph
(`closure::reachable_in_file_fns`) and owns the cycle-detection. Moving the
diagnostic to the validator would duplicate the call-graph walk in `thermite-spec`
(which has no `closure` module) — so REQ-2 refines the EXISTING `forge::check`
reject (the minimal, single-site change) rather than relocating it. The reject is
still a clean cert verdict (verdict-in-cert, §5.1 / R-SPEC-3), never a crash.

**The no-cheat guarantee (R-DEFER-9).** A mutual cycle cannot be laundered to L3:
a member without `dec` is rejected at `forge::check` (REQ-2) — it never reaches
the ladder; a dec-complete cycle whose measures don't decrease is rejected by
Verus (`could not prove termination`, AC-2) — the `decreases` is the ONLY thing
between the cycle and L0, exactly as for a self-recursive fn (C9 REQ-4). The
`ens` clauses are NON-VACUOUS (the §7 vacuity gate, which rejects `ens true`, is
respected — the AC-1 grounding ties `r` to a recursive spec twin).

## Verification

`cargo test -p forge` for the `mutual_recursion_cycle_fns` refinement (the
conditional reject: a dec-complete cycle is NOT in the rejected set; a
missing-`dec` cycle IS, with the refined cause) and the END-TO-END conformance
probes lowering each form to Verus and certifying (AC-1..AC-5). Expected cert
fields are hand-derived (R-CHAR-3), never copied from the toolchain. The
conformance probe is a `forge/tests/mutual_recursion_conformance.rs` exercising
`is_even`/`is_odd` (L3), the non-decreasing pair (L0), the missing-`dec` pair
(refined reject), the `fx diverge` member (L1), and the 3-cycle (L3).

GROUNDED with real `verus 0.2026.05.24.ecee80a` on the exact Verus forms the
lowerer emits (each probe a `use vstd::prelude::*; verus! { .. } fn main(){}`
frame, run `verus --no-cheating`):

```
(A) mutual pair WITH dec on both — the dec-complete cycle
  spec is_even/is_odd (decreases n, cross-call n-1)              -> 3 verified, 0 errors  (L3)
  EXEC is_even/is_odd (u64, decreases n, cross n-1,
    ens r == s_even(n as nat) — NON-VACUOUS spec twin)           -> 4 verified, 0 errors  (L3, AC-1)
  3-cycle a->b->c->a (each decreases n, cross n-1)               -> 3 verified, 0 errors  (L3, AC-5)
  lexicographic pair (decreases n,1 / decreases n,0;
    cross keeps n, drops tag)                                    -> 2 verified, 0 errors  (Verus
                                                                     supports full lexicographic, REQ-4)

(B) non-decreasing across the cycle — termination BITES
  ping/pong, both decreases n, cross-call n (NOT n-1),
    trivial ens r==r (termination the SOLE obligation)           -> 0 verified, 2 errors
                                                                     "could not prove termination"  (L0, AC-2)
  3-cycle a->b->c->a, two cross-calls do NOT decrease n          -> 1 verified, 2 errors
                                                                     "could not prove termination"  (L0)

(C) a member LACKS dec — the refined missing-dec reject
  is_even (NO decreases) / is_odd (decreases n), mutual          -> error: "recursive function must have a
                                                                     decreases clause"  (raw VIR-error,
                                                                     exit 2, empty --json — which forge::check
                                                                     must catch as a clean cert, AC-3; help
                                                                     names #[verifier::exec_allows_no_decreases_clause]
                                                                     — the fx-diverge exemption, #88)

(D) adjacency — does Verus require the cycle members contiguous?
  is_even, then an UNRELATED fn, then is_odd (non-adjacent)      -> 3 verified, 0 errors  (NO adjacency
                                                                     requirement; Verus discovers the SCC
                                                                     across the whole verus! block — REQ-3:
                                                                     the existing source-order emission
                                                                     ALREADY produces a valid group)
```

The grounding shows: (1) the EXISTING C9 lowering already emits a valid Verus
mutual group (D — no adjacency / grouping syntax needed); (2) the `decreases` is
the ONLY thing between the cycle and L0 (B — weaken it → termination failure; C —
remove it from one member → the recursive-decreases demand); (3) v1 is the FULL
mutual capability — n-cycles and lexicographic measures both certify (A). The
`ens` in AC-1 is NON-VACUOUS (tied to a recursive spec twin), so a wrong
body/measure is rejected — the §7 vacuity gate is respected.

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (mutual cycle with `dec` on every member → L3) | SHIPPED | #121 (epic #113). `mutual_recursion_cycle_fns` (in `check.rs`) is now CONDITIONAL: a `fn` is in the reject set only if its SCC (size ≥ 2) contains a non-`fx diverge` member lacking `dec`. A dec-complete cycle is ABSENT from the set → the per-item loop FALLS THROUGH to `item_subprogram` → `thermite_lower::lower` → `run_verus`. The partner is already woven into each member's §5.3 sub-program by the existing `reachable_fn_deps` (`reachable_in_file_fns` returns the cross-called partner). VERIFIED end-to-end: `forge/tests/mutual_recursion_conformance.rs::dec_complete_mutual_pair_certifies_l3` — `is_even`/`is_odd` each `dec n` cross `n-1`, non-vacuous `ens result == (n % 2 == 0/1)` → BOTH L3 (real verus). Non-test consumer: `cli::run` → `check_file`. |
| REQ-2 (the #110 reject becomes conditional — missing/insufficient `dec` → L0) | SHIPPED | #121 (epic #113). `mutual_recursion_cycle_fns` rejects ONLY when a cycle member `f.dec.is_none() && !fn_is_diverge(f)` (scanning the whole SCC); the per-item loop emits `Certificate::rejected` (`Level::L0`, cause RENAMED `MutualRecursionMissingDecreases` + a member-naming "every member must carry a `dec` measure, or declare `fx diverge`" detail). The `fx diverge` exemption (`fn_is_diverge` skip) is PRESERVED. A dec-complete cycle whose measures don't decrease reaches Verus and is rejected there (`could not prove termination`, the single-fn non-decreasing L0 shape). VERIFIED: `mutual_recursion_conformance.rs::mutual_cycle_missing_dec_is_rejected_l0` (missing-dec → L0 `MutualRecursionMissingDecreases`) + `nondecreasing_mutual_cycle_is_l0` (verus termination L0) + `diverge_mutual_cycle_is_l1` (#88 exemption); `divergence_mutual_recursion.rs` (the #110 no-dec pin, still a clean non-L3 cert). |
| REQ-3 (the mutual-group lowering — grandfathered-correct, no change) | SHIPPED | C9 #108, CONFIRMED unchanged by #121. `lower` (in `lower.rs`) emits every `Item::Fn` in source order inside one `verus! { }` block; `lower_fn` emits `decreases <spec_dec(f.dec, f.params)>` for any `f.dec.is_some()`. CONFIRMED no `lower.rs` change needed: `item_subprogram`'s `Item::Fn` arm weaves `fn_deps` (= `reachable_fn_deps`, the partner) THEN the item, so each cycle member's per-item sub-program already contains its partner — Verus discovers the SCC. Non-test consumer: `forge::check` (`item_subprogram` → `thermite_lower::lower` → `run_verus`). VERIFIED L3 end-to-end via the REQ-1 conformance (the existing emission IS the contract). |
| REQ-4 (v1 measure scope — full lexicographic, n-cycles, per-fn `dec`) | SHIPPED | #121 (epic #113). n-cycles: `mutual_recursion_cycle_fns` computes the whole SCC, so any size ≥ 2 cycle is handled. VERIFIED: `mutual_recursion_conformance.rs::dec_complete_three_cycle_certifies_l3` — a 3-cycle `step_a -> step_b -> step_c -> step_a`, each `dec n` cross `n-1` → all three L3 (real verus). Per-fn `dec` is the measure-supply mechanism (each member writes its own `dec`); the common single-measure case is fully covered. The lexicographic comma-`dec` SURFACE remains OQ-2 (Verus has the capability; Thermite's `dec` lowers a single measure today via `spec_dec` — a separate surface concern, not a v1 REQ). |

## Open questions (for the orchestrator)

- **OQ-1 (reject cause rename — `MutualRecursionUnsupported` →
  `MutualRecursionMissingDecreases`):** REQ-2 renames the `RejectReason.cause`
  for the refined (missing-`dec`) reject. Because the certificate cause string is
  a contract surface (R-SPEC-2 — the cert IS the deliverable), the rename is a
  cert-schema-visible change; any conformance cert pinning the old
  `MutualRecursionUnsupported` string must update. The builder MAY instead keep
  the cause string and refine ONLY the `detail` (less churny, but the cause then
  no longer reads true for a dec-complete cycle — which is no longer rejected, so
  the only remaining reject IS missing-`dec`, making a rename the honest choice).
  Not a blocker for the contract.
- **OQ-2 (lexicographic comma-`dec` surface):** Verus supports a comma-separated
  lexicographic `decreases` (GROUNDED), and Thermite's `dec` clause is a single
  measure expression today (C9 `spec_dec`). A future lexicographic `dec`
  (`dec n, tag`) would extend the `dec` grammar + `spec_dec` to a comma-list — a
  separate surface concern, NOT required for the v1 common case (a shared
  numeric/structural measure, one `dec` per member). A design amendment, not a
  v1 REQ. Not a blocker.
- **OQ-3 (the missing-`dec` mutual diagnostic placement — `forge::check` vs
  validator):** REQ-2 keeps the diagnostic in `forge::check` (where the call
  graph is computed). A future move into `thermite-spec` (so the editor surfaces
  it earlier) would need the call-graph walk in `thermite-spec` (which has no
  `closure` module). The `forge::check` placement is the minimal, single-site v1
  choice. Not a blocker.
