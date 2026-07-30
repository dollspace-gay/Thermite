/-
  Typed preservation proofs for S₂.0 normalization.

  `Nnf.lean` proves the transformations against the older sort-erased
  structural oracle. These theorems establish the same facts for `evalFrm`, so
  reconstruction can normalize the formula that the typed source model means.
-/
import Thermite.Strat.Model

namespace Thermite.Strat.Cls

theorem not_all {α : Type} (values : List α) (predicate : α → Bool) :
    (!values.all predicate) = values.any (fun value => !predicate value) := by
  induction values with
  | nil => rfl
  | cons head tail ih => simp [List.all_cons, List.any_cons, Bool.not_and, ih]

theorem not_any {α : Type} (values : List α) (predicate : α → Bool) :
    (!values.any predicate) = values.all (fun value => !predicate value) := by
  induction values with
  | nil => rfl
  | cons head tail ih => simp [List.all_cons, List.any_cons, Bool.not_or, ih]

mutual
theorem eval_nnf (M : Model) :
    ∀ (formula : Frm) (ρ : Valuation M),
      evalFrm M (nnf formula) ρ = evalFrm M formula ρ
  | .atom atom, ρ => by simp [nnf, evalFrm]
  | .neg formula, ρ => by
      simp only [nnf, evalFrm, eval_nnfNeg M formula ρ]
  | .conj left right, ρ => by
      simp only [nnf, evalFrm, eval_nnf M left ρ, eval_nnf M right ρ]
  | .disj left right, ρ => by
      simp only [nnf, evalFrm, eval_nnf M left ρ, eval_nnf M right ρ]
  | .imp left right, ρ => by
      simp only [nnf, evalFrm, eval_nnfNeg M left ρ, eval_nnf M right ρ]
  | .all sort body, ρ => by
      simp only [nnf, evalFrm]
      apply congrArg (List.all (M.enum sort))
      funext value
      exact eval_nnf M body (Valuation.cons M sort value ρ)
  | .ex sort body, ρ => by
      simp only [nnf, evalFrm]
      apply congrArg (List.any (M.enum sort))
      funext value
      exact eval_nnf M body (Valuation.cons M sort value ρ)

theorem eval_nnfNeg (M : Model) :
    ∀ (formula : Frm) (ρ : Valuation M),
      evalFrm M (nnfNeg formula) ρ = !evalFrm M formula ρ
  | .atom atom, ρ => by simp [nnfNeg, evalFrm]
  | .neg formula, ρ => by
      simp only [nnfNeg, evalFrm, eval_nnf M formula ρ, Bool.not_not]
  | .conj left right, ρ => by
      simp only [nnfNeg, evalFrm, eval_nnfNeg M left ρ,
        eval_nnfNeg M right ρ, Bool.not_and]
  | .disj left right, ρ => by
      simp only [nnfNeg, evalFrm, eval_nnfNeg M left ρ,
        eval_nnfNeg M right ρ, Bool.not_or]
  | .imp left right, ρ => by
      simp only [nnfNeg, evalFrm, eval_nnf M left ρ,
        eval_nnfNeg M right ρ, Bool.not_or, Bool.not_not]
  | .all sort body, ρ => by
      simp only [nnfNeg, evalFrm, not_all]
      apply congrArg (List.any (M.enum sort))
      funext value
      exact eval_nnfNeg M body (Valuation.cons M sort value ρ)
  | .ex sort body, ρ => by
      simp only [nnfNeg, evalFrm, not_any]
      apply congrArg (List.all (M.enum sort))
      funext value
      exact eval_nnfNeg M body (Valuation.cons M sort value ρ)
end

theorem eval_liftTm (M : Model) (cutoff : Nat) (ρ : Valuation M) (term : Tm) :
    evalTm M ρ (liftTm cutoff term) =
      evalTm M (fun index => ρ (bumpIdx cutoff index)) term := by
  induction term generalizing cutoff with
  | var sort index => rfl
  | const sort id => rfl
  | lit sort value => rfl
  | read elem seq index seqIH indexIH =>
      simp only [liftTm, evalTm]
      rw [seqIH, indexIH]
  | len seq ih =>
      simp only [liftTm, evalTm]
      rw [ih]
  | cast target term ih =>
      simp only [liftTm, evalTm]
      rw [ih]
  | idxOp term offset ih =>
      simp only [liftTm, evalTm]
      rw [ih]
  | mul left right leftIH rightIH =>
      simp only [liftTm, evalTm]
      rw [leftIH, rightIH]
  | app1 arg result fn term ih =>
      simp only [liftTm, evalTm]
      rw [ih]

theorem eval_liftAtom (M : Model) (cutoff : Nat) (ρ : Valuation M) (atom : Atom) :
    evalAtom M ρ (liftAtom cutoff atom) =
      evalAtom M (fun index => ρ (bumpIdx cutoff index)) atom := by
  cases atom with
  | qfree expr => rfl
  | rel relation left right =>
      cases relation <;>
        simp only [liftAtom, evalAtom, eval_liftTm M cutoff ρ left,
          eval_liftTm M cutoff ρ right]

theorem valuation_cons_bump (M : Model) (cutoff : Nat) (sort : Sort₂)
    (value : M.Carrier sort) (ρ : Valuation M) :
    (fun index =>
      Valuation.cons M sort value ρ (bumpIdx (cutoff + 1) index)) =
    Valuation.cons M sort value (fun index => ρ (bumpIdx cutoff index)) := by
  funext index
  cases index with
  | zero => rfl
  | succ index =>
      simp only [Valuation.cons, bumpIdx]
      by_cases below : index < cutoff
      · have below' : index + 1 < cutoff + 1 := Nat.succ_lt_succ below
        simp [below, below']
      · have below' : ¬index + 1 < cutoff + 1 := by omega
        simp [below, below']

theorem eval_liftFrm (M : Model) :
    ∀ (formula : Frm) (cutoff : Nat) (ρ : Valuation M),
      evalFrm M (liftFrm cutoff formula) ρ =
        evalFrm M formula (fun index => ρ (bumpIdx cutoff index)) := by
  intro formula
  induction formula with
  | atom atom =>
      intro cutoff ρ
      simp only [liftFrm, evalFrm, eval_liftAtom]
  | neg formula ih =>
      intro cutoff ρ
      simp only [liftFrm, evalFrm, ih]
  | conj left right leftIH rightIH =>
      intro cutoff ρ
      simp only [liftFrm, evalFrm, leftIH, rightIH]
  | disj left right leftIH rightIH =>
      intro cutoff ρ
      simp only [liftFrm, evalFrm, leftIH, rightIH]
  | imp left right leftIH rightIH =>
      intro cutoff ρ
      simp only [liftFrm, evalFrm, leftIH, rightIH]
  | all sort body ih =>
      intro cutoff ρ
      simp only [liftFrm, evalFrm]
      apply congrArg (List.all (M.enum sort))
      funext value
      rw [ih, valuation_cons_bump]
  | ex sort body ih =>
      intro cutoff ρ
      simp only [liftFrm, evalFrm]
      apply congrArg (List.any (M.enum sort))
      funext value
      rw [ih, valuation_cons_bump]

theorem eval_cons_liftFrm (M : Model) (sort : Sort₂) (value : M.Carrier sort)
    (formula : Frm) (ρ : Valuation M) :
    evalFrm M (liftFrm 0 formula) (Valuation.cons M sort value ρ) =
      evalFrm M formula ρ := by
  rw [eval_liftFrm]
  congr

theorem any_const_false_typed {α : Type} (values : List α) :
    values.any (fun _ => false) = false := by
  induction values <;> simp_all

theorem all_const_true_typed {α : Type} (values : List α) :
    values.all (fun _ => true) = true := by
  induction values <;> simp_all

theorem all_and_distrib_typed {α : Type} (values : List α) (nonempty : values ≠ [])
    (predicate : α → Bool) (constant : Bool) :
    values.all (fun value => predicate value && constant) =
      (values.all predicate && constant) := by
  cases constant with
  | true => simp
  | false =>
      cases values with
      | nil => exact absurd rfl nonempty
      | cons head tail => simp [List.all_cons]

theorem any_or_distrib_typed {α : Type} (values : List α) (nonempty : values ≠ [])
    (predicate : α → Bool) (constant : Bool) :
    values.any (fun value => predicate value || constant) =
      (values.any predicate || constant) := by
  cases constant with
  | false => simp
  | true =>
      cases values with
      | nil => exact absurd rfl nonempty
      | cons head tail => simp [List.any_cons]

theorem any_and_distrib_typed {α : Type} (values : List α)
    (predicate : α → Bool) (constant : Bool) :
    values.any (fun value => predicate value && constant) =
      (values.any predicate && constant) := by
  cases constant <;> simp [any_const_false_typed]

theorem all_or_distrib_typed {α : Type} (values : List α)
    (predicate : α → Bool) (constant : Bool) :
    values.all (fun value => predicate value || constant) =
      (values.all predicate || constant) := by
  cases constant <;> simp [all_const_true_typed]

set_option linter.unusedVariables false in
theorem eval_mergeConj (M : Model) :
    ∀ (left right : Frm) (ρ : Valuation M),
      evalFrm M (mergeConj left right) ρ =
        (evalFrm M left ρ && evalFrm M right ρ) := by
  intro left right
  induction left, right using mergeConj.induct with
  | case1 sort left right ih =>
      intro ρ
      simp only [mergeConj, evalFrm]
      have body :
          (fun value => evalFrm M (mergeConj left (liftFrm 0 right))
            (Valuation.cons M sort value ρ)) =
          (fun value => evalFrm M left (Valuation.cons M sort value ρ)
            && evalFrm M right ρ) :=
        funext fun value => by rw [ih, eval_cons_liftFrm]
      rw [body, all_and_distrib_typed _ (M.enum_ne_nil sort)]
  | case2 sort left right ih =>
      intro ρ
      simp only [mergeConj, evalFrm]
      have body :
          (fun value => evalFrm M (mergeConj left (liftFrm 0 right))
            (Valuation.cons M sort value ρ)) =
          (fun value => evalFrm M left (Valuation.cons M sort value ρ)
            && evalFrm M right ρ) :=
        funext fun value => by rw [ih, eval_cons_liftFrm]
      rw [body, any_and_distrib_typed]
  | case3 left sort right _ _ ih =>
      intro ρ
      simp only [mergeConj, evalFrm]
      have body :
          (fun value => evalFrm M (mergeConj (liftFrm 0 left) right)
            (Valuation.cons M sort value ρ)) =
          (fun value => evalFrm M right (Valuation.cons M sort value ρ)
            && evalFrm M left ρ) :=
        funext fun value => by
          rw [ih, eval_cons_liftFrm, Bool.and_comm]
      rw [body, all_and_distrib_typed _ (M.enum_ne_nil sort), Bool.and_comm]
  | case4 left sort right _ _ ih =>
      intro ρ
      simp only [mergeConj, evalFrm]
      have body :
          (fun value => evalFrm M (mergeConj (liftFrm 0 left) right)
            (Valuation.cons M sort value ρ)) =
          (fun value => evalFrm M right (Valuation.cons M sort value ρ)
            && evalFrm M left ρ) :=
        funext fun value => by
          rw [ih, eval_cons_liftFrm, Bool.and_comm]
      rw [body, any_and_distrib_typed, Bool.and_comm]
  | case5 left right _ _ _ _ =>
      intro ρ
      simp only [mergeConj, evalFrm]

set_option linter.unusedVariables false in
theorem eval_mergeDisj (M : Model) :
    ∀ (left right : Frm) (ρ : Valuation M),
      evalFrm M (mergeDisj left right) ρ =
        (evalFrm M left ρ || evalFrm M right ρ) := by
  intro left right
  induction left, right using mergeDisj.induct with
  | case1 sort left right ih =>
      intro ρ
      simp only [mergeDisj, evalFrm]
      have body :
          (fun value => evalFrm M (mergeDisj left (liftFrm 0 right))
            (Valuation.cons M sort value ρ)) =
          (fun value => evalFrm M left (Valuation.cons M sort value ρ)
            || evalFrm M right ρ) :=
        funext fun value => by rw [ih, eval_cons_liftFrm]
      rw [body, all_or_distrib_typed]
  | case2 sort left right ih =>
      intro ρ
      simp only [mergeDisj, evalFrm]
      have body :
          (fun value => evalFrm M (mergeDisj left (liftFrm 0 right))
            (Valuation.cons M sort value ρ)) =
          (fun value => evalFrm M left (Valuation.cons M sort value ρ)
            || evalFrm M right ρ) :=
        funext fun value => by rw [ih, eval_cons_liftFrm]
      rw [body, any_or_distrib_typed _ (M.enum_ne_nil sort)]
  | case3 left sort right _ _ ih =>
      intro ρ
      simp only [mergeDisj, evalFrm]
      have body :
          (fun value => evalFrm M (mergeDisj (liftFrm 0 left) right)
            (Valuation.cons M sort value ρ)) =
          (fun value => evalFrm M right (Valuation.cons M sort value ρ)
            || evalFrm M left ρ) :=
        funext fun value => by
          rw [ih, eval_cons_liftFrm, Bool.or_comm]
      rw [body, all_or_distrib_typed, Bool.or_comm]
  | case4 left sort right _ _ ih =>
      intro ρ
      simp only [mergeDisj, evalFrm]
      have body :
          (fun value => evalFrm M (mergeDisj (liftFrm 0 left) right)
            (Valuation.cons M sort value ρ)) =
          (fun value => evalFrm M right (Valuation.cons M sort value ρ)
            || evalFrm M left ρ) :=
        funext fun value => by
          rw [ih, eval_cons_liftFrm, Bool.or_comm]
      rw [body, any_or_distrib_typed _ (M.enum_ne_nil sort), Bool.or_comm]
  | case5 left right _ _ _ _ =>
      intro ρ
      simp only [mergeDisj, evalFrm]

theorem eval_prenex (M : Model) :
    ∀ (formula : Frm) (ρ : Valuation M),
      evalFrm M (prenex formula) ρ = evalFrm M formula ρ
  | .atom atom, ρ => by simp [prenex]
  | .neg formula, ρ => by
      simp only [prenex, evalFrm, eval_prenex M formula ρ]
  | .conj left right, ρ => by
      simp only [prenex, evalFrm]
      rw [eval_mergeConj M, eval_prenex M left ρ, eval_prenex M right ρ]
  | .disj left right, ρ => by
      simp only [prenex, evalFrm]
      rw [eval_mergeDisj M, eval_prenex M left ρ, eval_prenex M right ρ]
  | .imp left right, ρ => by
      simp only [prenex, evalFrm, eval_prenex M left ρ, eval_prenex M right ρ]
  | .all sort body, ρ => by
      simp only [prenex, evalFrm]
      apply congrArg (List.all (M.enum sort))
      funext value
      exact eval_prenex M body (Valuation.cons M sort value ρ)
  | .ex sort body, ρ => by
      simp only [prenex, evalFrm]
      apply congrArg (List.any (M.enum sort))
      funext value
      exact eval_prenex M body (Valuation.cons M sort value ρ)

#print axioms eval_nnf
#print axioms eval_prenex

end Thermite.Strat.Cls
