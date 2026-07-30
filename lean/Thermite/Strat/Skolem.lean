/-
  Skolemization for the finite S₂.0 prefix.

  A `SkolemTree` is the finite graph of Skolem choices. Universal nodes branch
  over their whole typed carrier; existential nodes store one witness. Because
  an existential node occurs below exactly the universal nodes to its left, its
  witness may depend on every earlier universal and on no later binder. This is
  the dependency condition that a flat list of witness constants tends to lose.
-/
import Thermite.Strat.Normalize

namespace Thermite.Strat.Cls

open Classical

inductive BinderKind where
  | all
  | ex
  deriving DecidableEq, Repr

abbrev Prefix := List (BinderKind × Sort₂)

/-- Remove the leading quantifier prefix. `prenex` is run before this operation
    in the reconstruction pipeline. -/
def peel : Frm → Prefix × Frm
  | .all sort body =>
      let result := peel body
      ((BinderKind.all, sort) :: result.1, result.2)
  | .ex sort body =>
      let result := peel body
      ((BinderKind.ex, sort) :: result.1, result.2)
  | formula => ([], formula)

def evalPrefix (M : Model) : Prefix → Frm → Valuation M → Bool
  | [], matrix, ρ => evalFrm M matrix ρ
  | (BinderKind.all, sort) :: rest, matrix, ρ =>
      (M.enum sort).all fun value =>
        evalPrefix M rest matrix (Valuation.cons M sort value ρ)
  | (BinderKind.ex, sort) :: rest, matrix, ρ =>
      (M.enum sort).any fun value =>
        evalPrefix M rest matrix (Valuation.cons M sort value ρ)

theorem evalPrefix_peel (M : Model) (formula : Frm) (ρ : Valuation M) :
    evalPrefix M (peel formula).1 (peel formula).2 ρ =
      evalFrm M formula ρ := by
  induction formula generalizing ρ with
  | all sort body ih =>
      simp only [peel, evalPrefix, evalFrm]
      apply congrArg (List.all (M.enum sort))
      funext value
      exact ih (Valuation.cons M sort value ρ)
  | ex sort body ih =>
      simp only [peel, evalPrefix, evalFrm]
      apply congrArg (List.any (M.enum sort))
      funext value
      exact ih (Valuation.cons M sort value ρ)
  | atom atom => rfl
  | neg formula ih => rfl
  | conj left right leftIH rightIH => rfl
  | disj left right leftIH rightIH => rfl
  | imp left right leftIH rightIH => rfl

/-- A finite Skolem strategy. The membership proof on an existential witness
    prevents an empty- or out-of-carrier choice from masquerading as a model. -/
inductive SkolemTree (M : Model) : Prefix → Type
  | done : SkolemTree M []
  | all {sort rest}
      (next : M.Carrier sort → SkolemTree M rest) :
      SkolemTree M ((BinderKind.all, sort) :: rest)
  | ex {sort rest}
      (value : M.Carrier sort)
      (member : value ∈ M.enum sort)
      (next : SkolemTree M rest) :
      SkolemTree M ((BinderKind.ex, sort) :: rest)

def SkolemTree.wins (M : Model) {binders : Prefix}
    (tree : SkolemTree M binders) (matrix : Frm) (ρ : Valuation M) : Bool :=
  match tree with
  | .done => evalFrm M matrix ρ
  | .all next =>
      (M.enum _).all fun value =>
        (next value).wins M matrix (Valuation.cons M _ value ρ)
  | .ex value _ next =>
      next.wins M matrix (Valuation.cons M _ value ρ)

/-- Finite typed Skolemization is equisatisfiable with the original prefix.
    Classical choice only assembles the per-universal successful subtrees into
    one strategy; every chosen value already comes from a finite enumeration. -/
theorem evalPrefix_iff_skolemTree (M : Model) :
    ∀ (binders : Prefix) (matrix : Frm) (ρ : Valuation M),
      evalPrefix M binders matrix ρ = true ↔
        ∃ tree : SkolemTree M binders, tree.wins M matrix ρ = true := by
  intro binders
  induction binders with
  | nil =>
      intro matrix ρ
      constructor
      · intro holds
        exact ⟨SkolemTree.done, holds⟩
      · rintro ⟨tree, holds⟩
        cases tree
        exact holds
  | cons binder rest ih =>
      cases binder with
      | mk kind sort =>
          cases kind with
          | all =>
              intro matrix ρ
              constructor
              · intro holds
                have each :
                    ∀ value ∈ M.enum sort,
                      evalPrefix M rest matrix
                        (Valuation.cons M sort value ρ) = true :=
                  List.all_eq_true.mp holds
                let next : M.Carrier sort → SkolemTree M rest :=
                  fun value => Classical.choose
                    ((ih matrix (Valuation.cons M sort value ρ)).mp
                      (each value (M.enum_complete sort value)))
                refine ⟨SkolemTree.all next, ?_⟩
                apply List.all_eq_true.mpr
                intro value member
                exact Classical.choose_spec
                  ((ih matrix (Valuation.cons M sort value ρ)).mp
                    (each value member))
              · rintro ⟨tree, wins⟩
                cases tree with
                | all next =>
                    apply List.all_eq_true.mpr
                    intro value member
                    apply (ih matrix (Valuation.cons M sort value ρ)).mpr
                    exact ⟨next value, List.all_eq_true.mp wins value member⟩
          | ex =>
              intro matrix ρ
              constructor
              · intro holds
                obtain ⟨value, member, tail⟩ := List.any_eq_true.mp holds
                obtain ⟨tree, wins⟩ :=
                  (ih matrix (Valuation.cons M sort value ρ)).mp tail
                exact ⟨SkolemTree.ex value member tree, wins⟩
              · rintro ⟨tree, wins⟩
                cases tree with
                | ex value member next =>
                    apply List.any_eq_true.mpr
                    exact ⟨value, member,
                      (ih matrix (Valuation.cons M sort value ρ)).mpr
                        ⟨next, wins⟩⟩

/-- End-to-end normalization and Skolemization theorem for an arbitrary S₂.0
    formula. The strategy is built for the prefix of `prenex (nnf formula)`. -/
theorem skolemization_equisatisfiable (M : Model) (formula : Frm)
    (ρ : Valuation M) :
    evalFrm M formula ρ = true ↔
      ∃ tree : SkolemTree M (peel (prenex (nnf formula))).1,
        tree.wins M (peel (prenex (nnf formula))).2 ρ = true := by
  rw [← eval_nnf M formula ρ]
  rw [← eval_prenex M (nnf formula) ρ]
  rw [← evalPrefix_peel M (prenex (nnf formula)) ρ]
  exact evalPrefix_iff_skolemTree M _ _ ρ

#print axioms evalPrefix_iff_skolemTree
#print axioms skolemization_equisatisfiable

end Thermite.Strat.Cls
