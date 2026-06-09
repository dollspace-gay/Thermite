# Loop Translation Validation — step 2.2.2 (the LOOP extension of body-state TV + the WHILE-RULE)

<!--
tier: 3-component
status: draft
governs: thermite-tv/src/exec_stmt_encode.rs, thermite-tv/src/obligation.rs,
         lean/Thermite/Exec/Loop.lean (new), forge/src/body_tv.rs
thesis-refs:
  - thermite-design.md §4.1 (contract-first functions — the exec BODY they guard: while/inv/dec)
  - thermite-design.md §6 (the verification ladder; L3 = Verus-derived SMT proof; the loop invariant + decreases)
  - thermite-design.md §5.1 (counterexamples, not adjectives)
  - thermite-design.md §13 (v0.1 kernel scope; the verified-microkernel convergence — loops are kernel-exec)
epic: crosslink #169 (the lowering-soundness arc); #163 (this component)
forge-seam: crosslink #162 (forge body-tv check phase — the four-way Faithful/Divergent/Unverifiable/Skipped)
starting-frame: .design/verified/exec-stmt-tv.md REQ-4 + "Step 2.2.2 horizon"
unifies-into: .design/verified/thermite-semantics.md (S_B → S_Loop; the (T2) capstone pattern this EXTENDS)
prior-arc:
  - .design/verified/exec-stmt-tv.md (#158 — the STRAIGHT-LINE body state-transformer R_B, SHIPPED + PROVEN)
  - lean/Thermite/Exec/Stmt.lean (#172 — S_B mechanized + body_ref_sound, the SINGLE-ITERATION reuse target)
  - lean/Thermite/Faithfulness.lean (#174/#183 — the h_tv capstone pattern this EXTENDS, not forks)
-->

## Summary

This is the LOOP extension of the body-state translation-validation arc: it takes loops from "FRAMED,
not designed" (`exec-stmt-tv.md` REQ-4) to a buildable increment. The straight-line body TV is SHIPPED
+ PROVEN (`body_ref_state in exec_stmt_encode.rs`, `body_ref_sound in Exec/Stmt.lean`); a `Stmt::Loop`
is HONESTLY rejected as `RefEncodeError::Unsupported`. This doc designs the per-run loop obligations
(Rust `thermite-tv`) and the mechanized iteration semantics + WHILE-RULE soundness theorem (Lean) that
together upgrade a faithful `while inv .. dec ..` loop from "Skipped honestly" to VERIFIED — without
ever re-deriving the loop's closed form. The architecture EXTENDS the existing `h_tv` capstone pattern
(`Faithfulness.lean`): per-run Z3-discharged premises + a universally-proven Lean rule = the universal
guarantee. Everything here is NOT-STARTED with this doc as the design prerequisite, except the
single-iteration body transformer it reuses (SHIPPED).

## Trust model (unchanged — N-version differential validation + the verified-validator meta-theorem)

Loop-TV checks `production-loop-lowering ≡ independent-loop-reference` over the per-iteration STATE STEP
and the after-loop characterization. Agreement is EVIDENCE per run (Z3-discharged); the Lean WHILE-RULE
is the universally-proven piece that makes the per-run evidence a universal guarantee (the
`Faithfulness.lean` `h_tv` pattern). The independence boundary is UNCHANGED and HARD (`exec-stmt-tv.md`
REQ-2 / AC-6): the reference MUST NOT call any `thermite_lower::lower::*` symbol; `thermite-tv` keeps NO
`thermite-lower` dependency. The loop reference reuses the SHIPPED `body_ref_state` for the
single-iteration step (the loop body is ITSELF a straight-line block) and adds ONLY the three loop
obligations — the new, small, auditable part.

## The chosen architecture — a variant of (a), single-iteration step-TV + invariant, NOT (b) unrolling

The two candidates `#163` names: **(a)** TV the single-iteration state-step + rely on the production
invariant; **(b)** bounded unrolling for a small bound. The architecture, the established
single-iteration body TV (`body_ref_sound` already covers the per-iteration block), and the
`Faithfulness.lean` capstone pattern all converge on a **variant of (a)**: the loop's per-run obligations
are three Z3-discharged premises (`h_tv`-shaped) — invariant-holds-on-entry, invariant-preserved-by-one-
iteration, and after-loop = invariant ∧ ¬condition — and a Lean-proven WHILE-RULE supplies the universal
piece (IF those premises hold THEN the after-loop characterization is sound w.r.t. the genuine iteration
semantics). The single-iteration body IS a straight-line block, so the ALREADY-PROVEN `body_ref_sound`
(`Exec/Stmt.lean`) covers the step with NO new body machinery — that reuse is the whole reason (a) is
tractable and is why it slots cleanly into the existing `h_tv` capstone. Approach (b) bounded unrolling
is **DROPPED for v1**: it is sound only up to its bound (a bounded-model-check / L2 flavour,
`thermite-design.md` §13 v0.2; Alive2's "sound-for-reported-violations, incomplete" in the
`thermite-semantics.md` field map), so it would NOT discharge the after-loop refinement for the
arbitrary-iteration-count corpus loops (`binary_search`'s `while`/`loop` runs an input-dependent number
of iterations). What (b) WOULD add — a Kani-style L2 fallback for invariant-FREE loops — is recorded as
the honest degraded path: a loop WITHOUT a usable `inv`/`dec` is `Skipped` in v1 (it cannot enter the
(a) rule), and bounded unrolling is the future v0.2 L2 mechanism for those (a separate blocker, not v1).
The case for (a) is exactly Leroy's verified-validator economy: reuse the SHIPPED, PROVEN single-step
validator and lean on the EXISTING Verus-checked invariant rather than re-deriving the loop's fixpoint.

## REQ-1 — the loop surface + the v1 frozen loop subset

The loop AST is SHIPPED (`thermite-syntax/src/ast.rs`):

```rust
pub enum Stmt {
    // ...
    Loop(LoopNode),
    Break,
    Continue,
    // ...
}

pub struct LoopNode {
    pub kind: LoopKind,        // Loop | While(Box<Expr>)
    pub invs: Vec<Clause>,     // non-empty (structurally encoding §4.1 "inv is mandatory")
    pub dec: Clause,           // a single clause (the `decreases` measure)
    pub body: Block,
    pub span: Span,
}

pub enum LoopKind { Loop, While(Box<Expr>) }
```

`LoopNode` STRUCTURALLY guarantees what v1 needs: `invs` is non-empty and `dec` is present (the parser
enforces §4.1's mandatory `inv`/`dec`). The production lowerer emits the Verus-native form (`lower_loop
in lower.rs`): a `while <cond>` (or `loop`) header, then `invariant <inv>,` per `inv` clause (plus
lifted immutable preconditions), then `decreases <dec>,` (suppressed ONLY for `fx diverge`, where the
loop is non-terminating by design and the fn carries `#[verifier::exec_allows_no_decreases_clause]`),
then the body. The `decreases` is REAL (no proof cheat, R-DEFER-9): a non-decreasing measure → Verus L0.

**The canonical corpus loop** (`conformance/binary_search.th`, quoted verbatim — the subset is DERIVED
from what the corpus actually uses):

```
  let mut lo: usize = 0;
  let mut hi: usize = haystack.len();
  loop
    inv lo <= hi && hi <= haystack.len()
    inv forall_below(haystack, lo, |x| x < needle)
    inv forall_from(haystack, hi, |x| x > needle)
    dec hi - lo
  {
    if lo == hi { return None; }
    let mid = lo + (hi - lo) / 2;
    if haystack[mid] == needle { return Some(mid); }
    if haystack[mid] < needle  { lo = mid + 1; } else { hi = mid; }
  }
```

This loop is the v1 design target's STRESS case (it has early `return`s in the body, an `Option` result,
and `forall_*` invariants) — but it also shows exactly where v1 must FREEZE: those mid-body `return`s are
a multi-exit form. **The v1 frozen loop subset** (the buildable increment, deliberately narrower than the
full corpus loop, derived from `exec-stmt-tv.md` REQ-1 + the `LoopNode` surface):

**IN (the v1 loop subset — the buildable first slice):**

| Construct | AST | v1 role |
|---|---|---|
| a single `while <cond>` with declared `inv`+`dec` | `Stmt::Loop(LoopNode { kind: While(c), invs, dec, .. })` | the after-loop state is characterized by `inv ∧ ¬c` |
| a STRAIGHT-LINE loop body | `LoopNode.body` is the `exec-stmt-tv.md` REQ-1 IN set (let/assign/if/seq, scalar cells) | the per-iteration step reuses the SHIPPED `body_ref_state` |
| scalar mutable loop variables | `usize`/`u64`/`u32` cells the body mutates (`lo`/`hi`) | the per-iteration `(state, cond) → state'` transformer |
| the loop `inv` clauses + the `dec` measure | `LoopNode.invs` (non-empty) + `LoopNode.dec` | the per-run obligations + the (Verus-checked) termination premise |

**OUT (explicitly NOT in v1 — honest boundary, each → `Skipped`):**

- **`loop` (the infinite-`kind`) and `break`/`continue`.** v1 admits ONLY `while <cond>` with a single
  exit at the head (the `inv ∧ ¬c` characterization). The corpus `binary_search` uses `loop { if lo == hi
  { return None; } .. }` — its `loop`-kind + mid-body `return None`/`return Some(mid)` are a MULTI-EXIT
  CPS form, OUT of v1 (a multi-exit after-loop characterization needs per-exit invariant conjuncts — a
  v2 extension). A `loop`-kind / a body with `break`/`continue` / a mid-body `return` is `Skipped`.
- **Nested loops.** A loop body containing another `Stmt::Loop` is OUT of v1 (the inner loop's after
  state is itself a fixpoint inside the outer body-step; `Skipped`).
- **Non-scalar loop state** (Vec/Map/String mutation in the body) — inherited OUT from `exec-stmt-tv.md`
  REQ-1 (the v1 state is bounded scalar cells); `Skipped`.
- **A loop without a usable `inv`/`dec`.** Structurally `LoopNode` always carries them, but a TRIVIALLY
  weak invariant (e.g. `inv true`) cannot enter the (a) rule (the after-loop characterization `true ∧ ¬c`
  is vacuous); such a loop is `Skipped` HONESTLY (R-HONEST-3) — never silently `Faithful`. (This is the
  invariant-free case where bounded unrolling (b) is the future v0.2 L2 fallback.)

This frozen subset is the design-pinned contract the Lean iteration semantics is authored against (the
`exec-stmt-tv.md` moving-target argument: the loop semantics is futile against a growing target; v1 pins
single-`while` straight-line-body). Derived from `thermite-design.md` §4.1 + the `LoopNode`/`LoopKind`
surface + `conformance/binary_search.th`'s loop shape.

## REQ-2 — the Rust TV extension (the three per-run loop obligations)

The loop TV adds three Z3-discharged obligations to the existing obligation machinery
(`obligation.rs`, sibling to `body_equivalence_obligation`). Each is a self-contained Verus unit
discharged through the EXISTING `forge::check::run_verus` (the same path `body_equivalence_obligation`
uses). The single-iteration body is encoded by REUSING the SHIPPED `body_ref_state` (the loop body is a
straight-line `Block`). The new encoder entry is `loop_ref_obligations(loop: &LoopNode, frame:
&LoopObligationFrame) -> Result<LoopObligations>` in `exec_stmt_encode.rs` (sibling to `body_ref_state`),
emitting the three Verus units below; the per-RHS / condition / inv VALUE encoding REUSES `exec_ref_value`
(independence preserved). The three obligations:

1. **Entry (invariant-holds-on-entry).** The loop is reached with the body's pre-loop straight-line
   state (the `let mut lo = 0; let mut hi = haystack.len();` prefix, encoded by `body_ref_state`); the
   obligation is `req <enclosing req> ensures <inv>` over that entry state — a Z3 IMPLICATION
   `entry-state ⟹ inv`. VERIFIED ⟺ the invariant genuinely holds on entry; a counterexample ⟺ the
   entry state violates the claimed invariant (a wrong pre-loop initialization).

2. **Preservation (invariant-preserved-by-one-iteration).** The single iteration body IS a straight-line
   block, so its state step is `body_ref_state(loop.body)` — the ALREADY-SHIPPED, ALREADY-PROVEN
   transformer. The obligation wraps that step as `fn tv_loop_step(<state-cells>) requires <inv> && <cond>,
   ensures <inv-at-the-stepped-state>, { <production loop-body lowering> }` — a Z3 check that the
   reference single-step state transformer carries `inv ∧ cond` to `inv`. VERIFIED ⟺ one faithful
   iteration preserves the invariant; a `postcondition not satisfied` ⟺ a per-iteration state-lowering
   infidelity (the SAME teeth `body_ref_sound`'s `wrong_var_assign` / `sequencing_order` /
   `mutation_not_applied` negative lemmas bite — a dropped/reordered/wrong-cell body mutation that breaks
   preservation). This is the obligation that REUSES the entire 2.2.1 body machinery for the step.

3. **Exit-characterization (after-loop = invariant ∧ ¬condition).** The obligation pins the after-loop
   symbolic state: on exit the loop guarantees `inv ∧ ¬cond`. The obligation is `req <inv> && !(<cond>)
   ensures <after-loop characterization>` — a Z3 EQUIVALENCE that the production's after-loop
   continuation reads the cells as `inv ∧ ¬cond`-constrained (not as a concrete closed form). VERIFIED ⟺
   the statements FOLLOWING the loop see exactly the `inv ∧ ¬cond` state; a counterexample ⟺ a wrong
   after-loop characterization (an over-strong claim about the exit state).

**How the after-loop state threads (the design's load-bearing question for the statements FOLLOWING the
loop).** A straight-line body's `body_ref_state` env maps each cell → a closed-form VALUE `Expr` in the
inputs. A loop CANNOT produce a closed form (it is a fixpoint). So at the loop boundary the env threading
changes shape: each cell the loop mutates (`lo`/`hi`) becomes an **opaque-but-invariant-constrained**
symbolic value — a FRESH Verus immutable binding (e.g. `let lo_post: usize; let hi_post: usize;`) whose
ONLY known facts are `assume(inv[cells := *_post] && !(cond[cells := *_post]))`. The statements following
the loop then thread `body_ref_state` over those opaque cells (the per-RHS value reference is unchanged —
it operates on `lo_post`/`hi_post` as free inputs constrained by the assumed invariant). This is the
honest analogue of how Verus itself models a loop's after-state (the cells are havocked + re-constrained
to the invariant). v1's after-loop continuation is in scope ONLY when the loop is the LAST statement
before the tail (the `binary_search` shape where the loop is the whole body); a loop followed by further
straight-line mutation is a v1.1 extension (the opaque-cell threading is designed here but its end-to-end
discharge is the increment's last slice).

**The four-way reporting (R-HONEST-3, the `forge::body_tv` seam #162).** The `forge body-tv` phase
(`exec-stmt-tv.md` REQ-5, blocker #162) reports each loop DISTINCTLY:
- **Faithful** — all three obligations VERIFY (entry holds, one iteration preserves, after-loop = inv ∧ ¬cond).
- **Divergent** — any obligation's production side fails `postcondition not satisfied` (a counterexample:
  a per-iteration infidelity or a wrong after-loop claim).
- **Unverifiable** — a Verus/Z3 timeout on an obligation (the ladder degrades, R-CODE-4; never a silent pass).
- **Skipped** — a loop OUTSIDE the v1 frozen subset (a `loop`-kind, `break`/`continue`, a mid-body
  `return`, a nested loop, non-scalar state, or a trivially-weak `inv`) → `RefEncodeError::Unsupported`
  surfaced as `Skipped` with a reason. NEVER `Faithful` (the honest 2.2.2 boundary in the certificate).

## REQ-3 — the Lean extension (the iteration semantics + the WHILE-RULE)

A new `lean/Thermite/Exec/Loop.lean` (namespace `Thermite.Exec`, importing `Thermite.Exec.Stmt`) extends
`S_B` with the `while` form. It does NOT fork the existing development — it reuses 2b's
`Stmt`/`Block`/`blockThread`/`bodyDenote`/`State` (`Exec/Stmt.lean`) for the loop body step.

**The while form + the iteration semantics (fuel-indexed iteration of the SHIPPED block transformer).**
Reuse the `Denote.lean` fuel pattern (`#181` well-founded recursion; the module header documents it is
NOT a fuel-cap vacuity dodge — the soundness theorem is ∀-fuel). A loop iteration is the SHIPPED
`blockThread` applied to the body once; the loop denotation iterates it:

```lean
/-- Fuel-indexed iteration of the SHIPPED single-step body transformer `blockThread`.
    `none` when fuel exhausts (faithful to nontermination-as-obligation: the genuine
    termination is the Verus-checked `decreases`, NOT proved here — see trust accounting). -/
def loopDenote (cond : ExecExpr) (body : Block) : Nat → State → Option State
  | 0,        _  => none                       -- fuel exhausted: no result (NOT a fixpoint claim)
  | fuel + 1, st => do
      let c ← asBool (← execDenote cond st.env)
      if c then
        let st' ← blockThread body st          -- ONE iteration = the SHIPPED transformer
        loopDenote cond body fuel st'           -- consume one fuel, iterate
      else
        some st                                 -- exit: the after-loop state
```

This is the genuine iteration semantics (big-step, fuel/WF on the iteration count). `none` when fuel
exhausts is faithful to nontermination-as-obligation: termination is the Verus-checked `decreases`
measure, deliberately NOT a Lean premise of the partial-correctness rule (see REQ-4).

**The WHILE-RULE theorem (the universal piece — the `h_tv` pattern's universally-proven rule).** Stated
against the genuine `loopDenote` iteration semantics, with the per-run obligations as hypotheses:

```lean
/-- (T2-loop) PARTIAL-CORRECTNESS WHILE-RULE. If the invariant `I` holds on entry,
    is preserved by one iteration of the SHIPPED body step (`blockThread body`), then
    on exit (whenever the loop terminates with some fuel) the after-loop state satisfies
    `I ∧ ¬cond`. The per-run premises are the Z3-discharged REQ-2 obligations. -/
theorem while_rule
    (cond : ExecExpr) (body : Block) (I : State → Prop)
    (h_pres : ∀ st, I st → (asBool <$> execDenote cond st.env) = some true →
                ∀ st', blockThread body st = some st' → I st')
    (fuel : Nat) (st stf : State)
    (h_entry : I st)                                   -- entry obligation (#REQ-2.1)
    (h_run   : loopDenote cond body fuel st = some stf) :
    I stf ∧ (asBool <$> execDenote cond stf.env) = some false :=
  -- by induction on `fuel`, using `h_pres` per iteration and the exit branch for `¬cond`.
  sorry  -- the universally-proven Lean rule (the build proves it; this doc states it)
```

The `h_pres` premise is exactly REQ-2 obligation 2 (preservation), discharged per-run by Z3 against the
SHIPPED `blockThread`/`body_ref_sound` step; `h_entry` is REQ-2 obligation 1; the conclusion `I stf ∧
¬cond` is REQ-2 obligation 3 (the after-loop characterization). The proof is induction on `fuel`: each
`fuel+1` step either exits (`¬cond` → conclusion directly) or iterates (`h_pres` re-establishes `I`,
recurse). The single-iteration step is `blockThread body` — the EXACTLY ALREADY-PROVEN transformer; the
WHILE-RULE carries the induction over the iteration count.

**The negative-lemma plan (the teeth — mirroring `Exec/Stmt.lean`'s three negative lemmas).** A
non-preserved invariant admits a state the rule would WRONGLY certify if `h_pres` were dropped:
- `non_preserved_invariant_admits_bad_after_state` — exhibit a `body`/`I`/witness state where one
  iteration BREAKS `I` (`blockThread body st = some st'` but `¬ I st'`); show that WITHOUT `h_pres` the
  after-loop conclusion `I stf` is FALSE at a reachable `stf`. This is the loop analogue of
  `mutation_not_applied_breaks_soundness` — it pins that the WHILE-RULE genuinely CONSUMES `h_pres`
  (a vacuous rule would certify the broken loop).
- `wrong_after_loop_characterization_breaks` — exhibit a loop whose true exit state satisfies `inv ∧ ¬c`
  but a buggy obligation claims a STRONGER post-condition; show the stronger claim is false at the
  genuine `loopDenote` exit (the loop analogue of the swapped-branch / wrong-cell teeth).
- `b_loop_iterates` — a POSITIVE witness: a concrete `while i < 3 { i = i + 1 }` from `i := 0` runs to
  `i = 3` under `loopDenote` at sufficient fuel (the iteration is REAL, not vacuous `none`).

## REQ-4 — the trust accounting (partial correctness is the honest v1)

| Piece | Trust class | Where |
|---|---|---|
| Entry / preservation / exit obligations | **per-run Z3** (the `h_tv` premises) | `loop_ref_obligations` → `forge::check::run_verus` |
| The single-iteration step | **Lean-proven** (REUSED, SHIPPED) | `body_ref_sound in Exec/Stmt.lean` (#172) |
| The iteration semantics `loopDenote` | **Lean-proven** (new) | `Exec/Loop.lean` |
| The WHILE-RULE (premises ⟹ after-loop = inv ∧ ¬cond) | **Lean-proven** (new, universal) | `Exec/Loop.lean::while_rule` |
| **Termination** (the `decreases` measure → the loop EXITS) | **per-run Z3/Verus (RESIDUAL)** | `lower_loop`'s emitted `decreases`, Verus-checked; NOT a Lean premise |
| Z3 soundness; S = intended meaning | inherited trusted (the `thermite-semantics.md` floor) | unchanged |

**The termination decision (DECIDED explicitly, R-HONEST-3).** The WHILE-RULE proves **PARTIAL
CORRECTNESS** only: *after-loop holds IF the loop exits* (the `h_run : loopDenote .. fuel st = some stf`
hypothesis carries the "exits at this fuel"). It deliberately does NOT prove total correctness /
termination. Termination is the Verus-checked `decreases` measure per run (`lower_loop` emits
`decreases <dec>`, Verus discharges it; a non-decreasing measure → L0, the SHIPPED behaviour). So
termination stays a per-run trusted premise (Verus/Z3), exactly as Verus's own loop-termination check.
This is the honest v1 boundary: the Lean WHILE-RULE is the UNIVERSAL partial-correctness piece; total
correctness rides on the SHIPPED, per-run `decreases` discharge. Making termination a Lean premise (a
well-founded measure inside `Exec/Loop.lean`) is a v2 strengthening (a separate increment) — v1 records
it as the named residual, never machine-closed.

## REQ-5 — the increment plan (build order + manifests + acceptance criteria)

The build order follows the arc's invariant: **the Rust obligation is the thing the Lean must be
faithful TO** (the `exec-stmt-tv.md` / `thermite-semantics.md` order: encoder first, then the soundness
proof, then the forge seam). Three increments, each a future blocker under #163:

**Increment 2.2.2-i — the Rust loop arm + the three obligations (the production-side reference).**
- Manifest: `thermite-tv/src/exec_stmt_encode.rs` (`loop_ref_obligations` + the opaque-cell after-loop
  threading; the v1-subset honest-`Unsupported` for `loop`-kind / break / continue / mid-body return /
  nested / weak-inv), `thermite-tv/src/obligation.rs` (`LoopObligationFrame` + the three obligation
  emitters, sibling to `body_equivalence_obligation`), `thermite-tv/tests/loop_teeth.rs` (the L1–L4
  conformance pins against real verus).
- AC: a faithful `while` loop's three obligations VERIFY (`verified: 1, errors: 0`); a broken-invariant
  mutant (preservation fails) → `postcondition not satisfied`; a wrong-after-loop-state mutant → CAUGHT;
  a `loop`-kind / break / mid-body-return loop → `RefEncodeError::Unsupported` (Skipped honestly).

**Increment 2.2.2-ii — the Lean iteration semantics + the WHILE-RULE (the universal soundness piece).**
- Manifest: `lean/Thermite/Exec/Loop.lean` (`loopDenote` + `while_rule` + the three negative lemmas +
  the positive `b_loop_iterates`), `lean/Thermite/Faithfulness.lean` (EXTEND `FnTvWitness` with a loop
  layer + a `tv_meta_loop` meta-theorem composing the WHILE-RULE with the per-run `h_tv`, sibling to
  `tv_meta_body`).
- AC: `while_rule` builds (`lake build` green, `#print axioms` = propext/Quot.sound/Classical.choice
  standard only, NO `sorry` in the shipped rule); the three negative lemmas PROVE the rule consumes
  `h_pres` / the exit characterization (the teeth bite); `tv_meta_loop` composes `h_tv.trans (while_rule)`.

**Increment 2.2.2-iii — the forge `body_tv` loop wiring (the #162 seam).**
- Manifest: `forge/src/body_tv.rs` (the loop branch of the four-way report — Faithful/Divergent/
  Unverifiable/Skipped — reusing the #162 phase scaffold), `forge/tests/loop_tv_conformance.rs`.
- AC: `binary_search.th`'s loop reaches the phase as `Skipped` (it is a `loop`-kind with mid-body
  returns — OUT of v1, honestly); a v1-subset faithful `while` corpus fixture → `Faithful`; a
  broken-invariant fixture → `Divergent` with a counterexample. (#162 is the prerequisite seam.)

## Acceptance criteria

- **AC-1 (faithful `while` loop → VERIFIED)** — a v1-subset `while c inv I dec d { straight-line body }`
  whose entry / preservation / exit obligations are faithful discharges all three as `verified: 1,
  errors: 0`.
- **AC-2 (broken-invariant mutant → CAUGHT)** — a loop whose body breaks preservation (a dropped/wrong-
  cell mutation, the `body_ref_sound` negative-lemma classes) fails the preservation obligation with
  `postcondition not satisfied`.
- **AC-3 (wrong-after-loop-state mutant → CAUGHT)** — a loop whose after-loop characterization over-claims
  (stronger than `inv ∧ ¬c`) fails the exit obligation with a counterexample.
- **AC-4 (loop-without-usable-inv → Skipped honestly)** — a `loop`-kind, a `break`/`continue` body, a
  mid-body `return`, a nested loop, a non-scalar-state loop, or a trivially-weak `inv` reaches
  `forge::body_tv` as `Skipped` (with a reason), NEVER `Faithful` (R-HONEST-3).
- **AC-5 (the single-iteration step REUSES the SHIPPED transformer)** — the preservation obligation's
  per-iteration body is encoded by `body_ref_state` (no new body machinery); a per-iteration RHS value
  infidelity is caught by the SAME obligation (inherited from `body_ref_sound`).
- **AC-6 (the WHILE-RULE is partial-correctness, termination is the residual)** — `while_rule` carries
  `h_run : loopDenote .. fuel st = some stf` (the loop EXITS) as a hypothesis; termination is the
  per-run Verus `decreases` discharge, NOT a Lean premise. The decision is recorded (REQ-4).
- **AC-7 (independence is structural — unchanged)** — `thermite-tv` keeps NO `thermite-lower` dependency;
  `loop_ref_obligations` references no `lower_loop`/`lower_stmt` symbol (`cargo tree -p thermite-tv` =
  syntax + spec only).

## Verification

When built, GROUNDED end-to-end against the real `verus` binary (exactly as 2.1 / 2.2.1): the three
obligations replayed through `forge::check::run_verus` (`thermite-tv/tests/loop_teeth.rs` L1–L4); the
Lean WHILE-RULE + negative lemmas under `lake build` (`lean/Thermite/Exec/Loop.lean`); the forge phase
under `forge/tests/loop_tv_conformance.rs`. Crate gauntlet: `cargo test -p thermite-tv`, `cargo test -p
forge`, `cargo clippy -p thermite-tv -p forge --all-targets -- -D warnings`, `cargo fmt --check`; `lake
build` green for the Lean increment.

## How Verus while-invariants work (training knowledge — marked as such)

(The following is from training knowledge of Verus, not a Read of a Thermite file; it grounds the
obligation shapes above.) Verus verifies a `while c invariant I decreases d { B }` loop by three
proof obligations, which the v1 loop-TV mirrors as its three per-run obligations: (1) the invariant
holds on loop ENTRY (`I` is implied by the pre-loop state); (2) the invariant is PRESERVED — assuming
`I ∧ c` at the loop head, the body `B` re-establishes `I` (Verus havocs the mutated cells and assumes
`I`, so the body proof is over a SINGLE arbitrary iteration — exactly why the single-iteration step-TV
slots in); (3) the `decreases` measure `d` strictly decreases each iteration and is bounded below
(TERMINATION). After the loop, Verus assumes `I ∧ ¬c` for the continuation — which is precisely the
v1 after-loop characterization (REQ-2 obligation 3) and the WHILE-RULE conclusion. The v1 architecture
deliberately mirrors Verus's own obligation structure: obligations (1)/(2)/the-after-state are the
per-run Z3 checks; the Lean WHILE-RULE proves the universal "(1) ∧ (2) ⟹ after = I ∧ ¬c" partial-
correctness shape; obligation (3) termination stays Verus's per-run `decreases` discharge (the residual).

## The honest boundary (loops in the certificate)

- **v1 loop subset (single `while` + declared inv/dec + straight-line scalar body):** the three per-run
  obligations + the Lean WHILE-RULE certify PARTIAL CORRECTNESS of the loop's after-state characterization
  (`inv ∧ ¬cond`), with termination riding on Verus's per-run `decreases`. A reader must NOT read v1
  loop-TV as TOTAL correctness (termination is the residual) and must NOT read it as covering `loop`-kind
  / multi-exit / nested / non-scalar loops (those are Skipped).
- **OUT of v1 (Skipped, the future v0.2 path):** invariant-free loops get bounded unrolling (b) as an
  L2/Kani fallback (a separate increment, not v1); multi-exit (`loop`+`break`/mid-body-`return`, the
  `binary_search` shape) gets per-exit invariant conjuncts (v2). A v1 certificate marks these `Skipped`
  honestly — never `Faithful`.

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (loop surface + v1 frozen loop subset) | SHIPPED (increment 2.2.2-i) | the v1 SUBSET is now PINNED IN CODE: `thermite_tv::exec_stmt_encode::loop_ref_obligations` (+ `recognize_v1_loop`/`reject_out_of_subset_body`/`collect_assigned_cells`/`invariant_is_vacuous`) ADMITS exactly a single `while <cond>` with non-empty `invs` + `dec` + a straight-line scalar body as the LAST statement before the tail, and HONESTLY REJECTS (an `Unsupported` `Err`) the `loop`-kind, `break`/`continue`, a mid-body `return`, a NESTED loop, a non-scalar assignment target, and a trivially-weak `inv true`. The loop AST is SHIPPED (`struct LoopNode`/`enum LoopKind`/`Stmt::Loop` in `thermite-syntax/src/ast.rs`; lowered by `lower_loop in lower.rs`). Non-test consumer `thermite_tv::obligation::{loop_entry_obligation, loop_preservation_obligation, loop_exit_obligation}`; verified by `thermite-tv/tests/loop_teeth.rs` L0 (the hand-derived reference pieces) + L1 (faithful in-subset VERIFIES) + L4 (five OUT forms each Skipped). |
| REQ-2 (Rust loop TV — the three per-run obligations) | SHIPPED (increment 2.2.2-i) | `pub fn loop_ref_obligations` (returns `struct LoopObligations`) in `thermite-tv/src/exec_stmt_encode.rs` builds the three per-run reference pieces (`entry_pred` = `inv[cells:=entry]`; `cond` + `inv` for the preservation `requires`; `step_cells` = the SHIPPED `body_ref_state` single-iteration step + `inv_at_step` for the preservation `ensures`); the three Verus-unit emitters are `pub fn loop_entry_obligation`/`loop_preservation_obligation`/`loop_exit_obligation` + `pub struct LoopObligationFrame`/`LoopParamDecl` in `thermite-tv/src/obligation.rs` (siblings to `body_equivalence_obligation`). ENTRY = `proof fn { assert(inv[cells:=entry]); }`; PRESERVATION = `fn tv_loop_step(cells, inputs) requires inv && cond, ensures result.i == <step_i>, <inv_at_step> { <p_production> }` (REUSES `body_ref_state` for the step — AC-5); EXIT = `proof fn tv_loop_exit(cells, inputs) requires inv && (!cond) { assert(<claimed_after_loop>); }` (the opaque-but-invariant-constrained after-loop cells). The cond/inv VALUE encoding REUSES `exec_ref_value` (independence preserved — `thermite-tv` keeps NO `thermite-lower` dep, AC-7). Non-test consumer `thermite_tv::lib` re-export → `tests/loop_teeth.rs` L1–L4 discharge through real verus (`Verus 0.2026.05.24`): L1 all three obligations `1 verified, 0 errors`; L2 broken-preservation `postcondition not satisfied`; L3 wrong-after-loop `assertion failed`; L4 OUT-of-v1 Skipped. The Lean WHILE-RULE (increment 2.2.2-ii, REQ-3) + the forge `body_tv` loop wiring (increment 2.2.2-iii, REQ-5) are SEPARATE future increments. |
| REQ-3 (Lean iteration semantics + WHILE-RULE) | SHIPPED (increment 2.2.2-ii) | `lean/Thermite/Exec/Loop.lean` (namespace `Thermite.Exec`, imports `Thermite.Exec.Stmt`) mechanizes the v1 `while` form as a SEPARATE `structure WhileLoop { cond : ExecExpr, body : Block, inv : State → Prop }` AROUND the SHIPPED `blockThread` (the Rust-faithful separate-form treatment — `Exec/Stmt.lean` UNCHANGED, no new `Stmt` arm); `def loopDenote (cond) (body) : Nat → State → Option State` is the GENUINE fuel-indexed iteration of `blockThread` (`0,_ => none` = fuel exhausted; `fuel+1 => if ¬cond then some st else loopDenote .. (blockThread body st)`, the obligation-`none` propagating). `theorem while_rule (cond body I) (h_pres : ∀ st, I st → condBool cond st = some true → ∀ st', blockThread body st = some st' → I st') (fuel) : ∀ st stf, I st → loopDenote cond body fuel st = some stf → I stf ∧ condBool cond stf = some false` — PROVED by induction on `fuel` (the exit branch gives `¬cond` directly; the continue branch re-establishes `I` via `h_pres` over the SHIPPED step + the IH), NO `sorry`. `theorem tv_meta_loop (L : WhileLoop) (h_pres) (fuel st stf) (h_entry : L.inv st) (h_run : loopDenote L.cond L.body fuel st = some stf) : L.inv stf ∧ condBool L.cond stf = some false` — the capstone composition (the three per-run Z3 premises + `while_rule`), the loop sibling of `tv_meta_body`. The premises MATCH the REQ-2 obligations: `h_entry`↔ENTRY (`inv[cells:=entry]`), `h_pres`↔PRESERVATION (one `blockThread` step keeps `inv` under `inv ∧ cond` — the havoc+frame shape), the conclusion `inv ∧ ¬cond`↔EXIT. NEGATIVE lemmas (the teeth): `l2_non_preserved_invariant_admits_bad_step` + `l2_no_preservation_premise_for_buggy_body` (the `lo + 2` body — preservation FAILS at `lo=2,n=3 → lo=4>n`, so `h_pres` is genuinely load-bearing; the L2 mirror) and `l3_exit_overclaim_refuted` (from `inv ∧ ¬cond` only `lo == n` follows — the over-claim `lo > n` is refuted at the genuine exit; the L3 mirror). Non-vacuity: `b_loop_iterates` (the L1 fixture `while lo < n inv lo ≤ n { lo := lo + 1 }` at `n=3` GENUINELY iterates `lo ↦ 1 ↦ 2 ↦ 3` to exit, fuel consumed, decide-backed) + `l1_entry_holds`/`l1_preservation`/`l1_while_rule_certifies_exit` (the rule FIRES, certifying `lo ≤ 3 ∧ ¬(lo < 3)` hence `lo = 3` = the L1 `lo == n` claim). `lake build` GREEN (full project incl. the spine + SmtDemo); `#print axioms while_rule`/`tv_meta_loop`/`loopDenote`/the negatives = `[propext, Quot.sound]` (STANDARD only, NO `sorryAx`/custom axiom/`Classical.choice`). CORE Lean only (the `2^64` `usize` bound facts via `Int.pow_pos` + a `decide`-computed `two_pow_64_val`; NO Mathlib, NO Lean-SMT, NO `native_decide`). `Thermite.lean` imports the new module. |
| REQ-4 (trust accounting — partial correctness, termination residual) | SHIPPED (increment 2.2.2-ii) | The DECISION is RECORDED above (partial correctness for v1; termination stays the per-run Verus `decreases` discharge, `lower_loop`'s SHIPPED `decreases <dec>` emission) AND now MECHANIZED: `while_rule`/`tv_meta_loop` (`Exec/Loop.lean`) carry the `h_run : loopDenote .. fuel st = some stf` hypothesis as the "the loop EXITS at this fuel" premise — the Verus `decreases` residual, deliberately NOT a Lean premise-to-prove (the module doc states the correspondence explicitly: `loopDenote`'s fuel-`0` `none` is NOT a fixpoint claim and `while_rule` is ∀-fuel, so termination is the named per-run residual, never machine-closed in v1). A reader must NOT read v1 loop-TV as TOTAL correctness (the honest boundary in the certificate). |
| REQ-5 (increment plan + forge `body_tv` loop wiring) | NOT-STARTED | open prereq blocker #163; the forge seam is blocker #162 (the `body_tv` phase scaffold — itself NOT-STARTED, no `forge/src/body_tv.rs`). The build order (Rust obligations → Lean WHILE-RULE → forge wiring) + the per-increment manifests + ACs are pinned above; unbuilt. |
