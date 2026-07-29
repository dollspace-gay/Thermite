/-
  Thermite/PinNNFPolarity.lean — the pre-NNF polarity pin (stage-2 pin battery,
  `.design/stage2-stratified-cage.md` REQ-3 / AC-3 / REQ-10).

  What it guards: `Strat/Fragment.lean`'s `admitted` computes the sort graph on the
  NEGATION NORMAL FORM (`acyclic (sortGraph (nnf φ))`), NOT on the raw formula. NNF is
  load-bearing precisely because `¬∀ = ∃` and `¬∃ = ∀` (metatheory §3.1): a quantifier
  under a negation has the OPPOSITE polarity from its syntactic kind, so the E1
  alternation edges (`∀S → ∃T`) are only correct once negations are pushed to the
  leaves. This pin exhibits the broken neighbour that builds the graph on the RAW
  formula (`acyclic (sortGraph φ)`, no `nnf`) and shows it ADMITS a formula whose true
  (post-NNF) sort graph has a `Key ⇄ Value` alternation cycle — a soundness break: a
  formula equivalent to an unstratifiable `(∀k.∃v…) ∧ (∀v.∃k…)` would slip into the cage.

  The witness is `(¬∃k:Key. ∀v:Value. φ₁) ∧ (¬∃v:Value. ∀k:Key. φ₂)`. RAW, the binders
  read syntactically as `∃k.∀v` and `∃v.∀k` (an existential outside a universal gives
  NO E1 edge), so the raw graph is empty → acyclic → the pre-NNF neighbour admits.
  After NNF the negations push in and FLIP both: `¬∃k.∀v = ∀k.∃v` and `¬∃v.∀k = ∀v.∃k`,
  contributing E1 edges `Key → Value` and `Value → Key` — a cycle, so the real
  `admitted` rejects. The (R1)/(R2) conjuncts pass either way, isolating the polarity.

  Style model: the in-tree `lean/Thermite/Pin*.lean` battery (`PinStratFlip`,
  `PinFiniteEscape`) — a small concrete `Frm` + `decide`-checked theorems; `decide`
  (kernel), never `native_decide`. Core-Lean-only (imports only `Strat/Fragment.lean`).
-/
import Thermite.Strat.Fragment

namespace Thermite.PinNNFPolarity

open Thermite.Strat.Cls

/-! ## The pre-NNF admission classifier (the broken neighbour) -/

/-- The broken admission classifier: identical to `admitted` except it builds the sort
    graph on the RAW formula instead of its NNF — so a quantifier hidden under a `¬`
    contributes its SYNTACTIC (wrong) polarity to the E1 edges. -/
def admittedPreNnf (φ : Frm) : Bool :=
  finCarrier φ && idxGrammar φ && acyclic (sortGraph φ)

/-! ## The witness — two negated `∃…∀` blocks that NNF flips into a `Key ⇄ Value` cycle -/

/-- `Key = opaque 0`, `Value = opaque 1` (the §3.2 kv-cycle sorts). -/
def keyS : Sort₂ := .opaque 0
def valueS : Sort₂ := .opaque 1

/-- `body₁` under `∃k. ∀v.`: de Bruijn `v = 0`, `k = 1` — both opaque vars, so it
    contributes no E2 edges and passes (R2) (`idxOkTm` on a `var` is `true`). -/
def body1 : Frm := .atom (.rel .eq (.var valueS 0) (.var keyS 1))
/-- `body₂` under `∃v. ∀k.`: de Bruijn `k = 0`, `v = 1`. -/
def body2 : Frm := .atom (.rel .eq (.var keyS 0) (.var valueS 1))

/-- `(¬∃k:Key. ∀v:Value. body₁) ∧ (¬∃v:Value. ∀k:Key. body₂)`. raw polarity hides the
    alternation; NNF reveals the `Key ⇄ Value` cycle. -/
def phiHiddenCycle : Frm :=
  .conj (.neg (.ex keyS (.all valueS body1)))
        (.neg (.ex valueS (.all keyS body2)))

/-! ## The pin -/

/-- The concrete counterexample: the pre-NNF classifier admits the hidden-cycle formula
    (its raw graph is empty → acyclic), while the real `admitted` REJECTS it (the
    post-NNF graph has the `Key ⇄ Value` alternation cycle). -/
theorem nnfPolarity_counterexample :
    admittedPreNnf phiHiddenCycle = true ∧ admitted phiHiddenCycle = false := by decide

/-- The pin: admission is unsound for the pre-NNF classifier — it accepts a formula
    (`phiHiddenCycle`) the real `admitted` rejects, because it reads quantifier polarity
    syntactically rather than after NNF. Computing acyclicity before NNF is therefore
    not a safe refactor — NNF is load-bearing. -/
theorem preNnf_breaks_admission :
    ¬ ∀ φ : Frm, admittedPreNnf φ = admitted φ := by
  intro h
  have hc := h phiHiddenCycle
  rw [nnfPolarity_counterexample.1, nnfPolarity_counterexample.2] at hc
  exact absurd hc (by decide)

end Thermite.PinNNFPolarity
