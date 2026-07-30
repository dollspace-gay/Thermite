/-
  Faithfulness of the stratified encoder over typed source semantics.

  Stage 2 checked only the quantifier skeleton and left relation/array atoms as
  an arbitrary Boolean oracle. `Strat.Model` closes that gap: terms evaluate in
  their declared carrier, binders enumerate their own finite sort, equality is
  real equality, and reads, lengths, casts, arithmetic, and unary functions come
  from one typed model. Embedded qfree expressions are tied to
  `Thermite.denote` by `SourceModel`.
-/
import Thermite.Strat.Model

namespace Thermite.Strat

open Thermite.Strat.Cls
open Classical

/-- The production stratified encoding preserves the actual typed formula
    meaning. There is no free relation model in this statement. -/
theorem strat_lowering_faithful (source : SourceModel) (formula : Frm)
    (ρ σ : Valuation source.toModel) (wellFormed : wfFrm 0 formula = true) :
    evalTok source.toModel (sencode formula) σ =
      evalFrm source.toModel formula ρ :=
  typed_ref_sound_sentence source.toModel formula wellFormed ρ σ

/-- A qfree leaf in the encoded surface has exactly its v1 source meaning. -/
theorem strat_lowering_faithful_qfree_iff (source : SourceModel)
    (id : Nat) (expr : Thermite.Expr) (σ : Valuation source.toModel) :
    evalTok source.toModel (sencode (.atom (.qfree id expr))) σ = true
      ↔ Thermite.denote 0 expr source.venv := by
  change source.qfree id expr = true ↔ Thermite.denote 0 expr source.venv
  rw [source.qfree_source id expr]
  simp only [decide_eq_true_eq]

#print axioms strat_lowering_faithful
#print axioms strat_lowering_faithful_qfree_iff

end Thermite.Strat
