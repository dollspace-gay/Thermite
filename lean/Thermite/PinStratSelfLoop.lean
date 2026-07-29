/-
  Thermite/PinStratSelfLoop.lean — the sort-graph self-loop pin (stage-2 pin
  battery, `.design/stage2-stratified-cage.md` REQ-3 / AC-3 / REQ-10).

  What it guards: `Strat/Fragment.lean`'s `admitted` (and the coincidence theorem
  T3-C `classifier_correct`) REJECTS the nested-read trap `a[a[i]]` because the
  inner `Read` over a universally bound index contributes the E2 SELF-LOOP edge
  `usize → usize` to the sort graph, and `acyclic` (the Roy–Warshall `reach`) treats
  a self-edge as a length-1 cycle (`reach [] s s = hasEdge G s s`). This pin exhibits
  the broken neighbour that STRIPS reflexive edges from the graph before the acyclicity
  check (`stripSelf`, the classic "a cycle must have length > 1" bug) and shows it
  ADMITS the `a[a[i]]` self-loop that the real `admitted` rejects — a soundness break:
  the stratification guarantee (no quantifier-instantiation cycle) is exactly what the
  self-edge denies, so dropping it would let an unstratifiable formula into the cage.

  The witness is `Strat/Fragment.lean`'s own §3.2 micro-example `ex_selfLoop`
  (`∀ i:usize. a[a[i]] = a[a[i]]`, `a : SeqS usize`), whose sort graph is the single
  self-edge `usize → usize`. `admitted ex_selfLoop = false` is already proven there
  (`ex_selfLoop_rejected`); this pin shows the self-edge is load-bearing by exhibiting
  the strip-self neighbour that wrongly admits it.

  Style model: the in-tree `lean/Thermite/Pin*.lean` battery (`PinStratFlip`,
  `PinFiniteEscape`) — reuse a concrete `Frm` + `decide`-checked theorems; `decide`
  (kernel), never `native_decide`. Core-Lean-only (imports only `Strat/Fragment.lean`).
-/
import Thermite.Strat.Fragment

namespace Thermite.PinStratSelfLoop

open Thermite.Strat.Cls

/-! ## The strip-self acyclicity (the broken neighbour) -/

/-- Remove every reflexive (self-loop) edge from a graph. This is the broken
    neighbour's one change: it discards exactly the length-1 cycles, modelling the
    "a cycle must have length > 1" off-by-one in an acyclicity check. -/
def stripSelf (G : Graph) : Graph :=
  { nodes := G.nodes, edges := G.edges.filter (fun p => decide (p.1 ≠ p.2)) }

/-- The broken admission classifier: identical to `admitted` except it strips reflexive
    edges before the acyclicity check, so it can never see a self-loop. -/
def admittedNoSelf (φ : Frm) : Bool :=
  finCarrier φ && idxGrammar φ && acyclic (stripSelf (sortGraph (nnf φ)))

/-! ## The pin -/

/-- The concrete counterexample: the strip-self classifier admits the `a[a[i]]`
    self-loop (`ex_selfLoop`), while the real `admitted` REJECTS it — the self-edge,
    which `stripSelf` discards, is the entire reason for the rejection. -/
theorem selfLoop_counterexample :
    admittedNoSelf ex_selfLoop = true ∧ admitted ex_selfLoop = false := by decide

/-- The pin: admission is unsound for the strip-self classifier — it accepts a formula
    (`ex_selfLoop`) that the real `admitted` rejects, so the two classifiers disagree.
    Stripping reflexive edges is therefore not a safe refactor of `acyclic`. -/
theorem stripSelf_breaks_admission :
    ¬ ∀ φ : Frm, admittedNoSelf φ = admitted φ := by
  intro h
  have hc := h ex_selfLoop
  rw [selfLoop_counterexample.1, selfLoop_counterexample.2] at hc
  exact absurd hc (by decide)

/-- For contrast: the real `admitted` rejects the same witness (`Strat/Fragment.lean`'s
    `ex_selfLoop_rejected`), confirming the divergence is solely the dropped self-edge. -/
theorem real_rejects_selfLoop : admitted ex_selfLoop = false := ex_selfLoop_rejected

end Thermite.PinStratSelfLoop
