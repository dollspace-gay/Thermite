/-
  Kernel-checked theory steps over ground atoms.

  The external grounder may choose a useful subset of these steps, but it may
  not invent one: every step is decoded into its Horn clause and proved sound
  for every typed interpretation. Arithmetic-specific facts are added later as
  checked QF_LIA/QF_BV leaves rather than being smuggled into this datatype.
-/
import Thermite.Strat.GroundReconstruct

namespace Thermite.Strat.Cls

structure GroundTheoryClause where
  premises : List GroundAtom
  conclusion : GroundAtom
  deriving DecidableEq, Repr, Hashable

def GroundTheoryClause.toFormula (clause : GroundTheoryClause) : GroundFrm :=
  match clause.premises with
  | [] => .atom clause.conclusion
  | _ =>
      GroundFrm.implies
        (GroundFrm.conjoin (clause.premises.map GroundFrm.atom))
        (.atom clause.conclusion)

def GroundTheoryClause.eval (M : Model)
    (interpretation : GroundInterpretation M)
    (clause : GroundTheoryClause) : Bool :=
  !(clause.premises.all (evalGroundAtom M interpretation)) ||
    evalGroundAtom M interpretation clause.conclusion

theorem GroundTheoryClause.eval_toFormula (M : Model)
    (interpretation : GroundInterpretation M)
    (clause : GroundTheoryClause) :
    evalGroundFrm M interpretation clause.toFormula =
      clause.eval M interpretation := by
  cases clause with
  | mk premises conclusion =>
      cases premises <;>
        simp [GroundTheoryClause.toFormula, GroundTheoryClause.eval,
          GroundFrm.implies, evalGroundFrm, evalGroundFrm_conjoin,
          Function.comp_def]

inductive GroundTheoryStep where
  | equalityReflexivity (term : GroundTerm)
  | equalitySymmetry (left right : GroundTerm)
  | equalityTransitivity (left middle right : GroundTerm)
  | functionCongruence (fn : GroundFunction)
      (left right : List GroundTerm)
  | relationCongruence (relation : Rel)
      (left₁ left₂ right₁ right₂ : GroundTerm)
  | sequenceExtensionality (elem : Sort₂)
      (left right : GroundTerm)
  deriving DecidableEq, Repr, Hashable

def equalityPremises (left right : List GroundTerm) : List GroundAtom :=
  List.zipWith (fun first second => .rel .eq first second) left right

def sequenceDiffFunction (elem : Sort₂) : GroundFunction :=
  { kind := .seqDiff elem
    arguments := [.seq elem, .seq elem]
    result := usizeS }

def sequenceDiffTerm (elem : Sort₂)
    (left right : GroundTerm) : GroundTerm :=
  .appList (sequenceDiffFunction elem) [left, right]

def sequenceLengthTerm (elem : Sort₂) (sequence : GroundTerm) :
    GroundTerm :=
  .appList
    { kind := .len, arguments := [.seq elem], result := usizeS }
    [sequence]

def sequenceReadTerm (elem : Sort₂) (sequence index : GroundTerm) :
    GroundTerm :=
  .appList
    { kind := .read elem
      arguments := [.seq elem, usizeS]
      result := elem }
    [sequence, index]

def GroundTheoryStep.clause : GroundTheoryStep → GroundTheoryClause
  | .equalityReflexivity term =>
      ⟨[], .rel .eq term term⟩
  | .equalitySymmetry left right =>
      ⟨[.rel .eq left right], .rel .eq right left⟩
  | .equalityTransitivity left middle right =>
      ⟨[.rel .eq left middle, .rel .eq middle right],
        .rel .eq left right⟩
  | .functionCongruence fn left right =>
      ⟨equalityPremises left right,
        .rel .eq (.appList fn left) (.appList fn right)⟩
  | .relationCongruence relation left₁ left₂ right₁ right₂ =>
      ⟨[.rel .eq left₁ left₂, .rel .eq right₁ right₂,
          .rel relation left₁ right₁],
        .rel relation left₂ right₂⟩
  | .sequenceExtensionality elem left right =>
      let diff := sequenceDiffTerm elem left right
      ⟨[.rel .eq
          (sequenceLengthTerm elem left)
          (sequenceLengthTerm elem right),
        .rel .eq
          (sequenceReadTerm elem left diff)
          (sequenceReadTerm elem right diff)],
        .rel .eq left right⟩

abbrev GroundCnfLiteral := GroundAtom × Bool
abbrev GroundCnfClause := List GroundCnfLiteral

def GroundTheoryClause.cnfClause
    (clause : GroundTheoryClause) : GroundCnfClause :=
  clause.premises.map (·, false) ++ [(clause.conclusion, true)]

def GroundTheoryStep.structurallyValid : GroundTheoryStep → Bool
  | .equalityReflexivity _ | .equalitySymmetry _ _
    | .equalityTransitivity _ _ _ => true
  | .functionCongruence fn left right =>
      decide (left.map GroundTerm.sortOf = fn.arguments)
        && decide (right.map GroundTerm.sortOf = fn.arguments)
  | .relationCongruence _ _ _ _ _ => true
  | .sequenceExtensionality elem left right =>
      decide (left.sortOf = .seq elem)
        && decide (right.sortOf = .seq elem)

def GroundTheoryStep.terms : GroundTheoryStep → List GroundTerm
  | .equalityReflexivity term => [term]
  | .equalitySymmetry left right => [left, right]
  | .equalityTransitivity left middle right => [left, middle, right]
  | .functionCongruence fn left right =>
      left ++ right ++ [.appList fn left, .appList fn right]
  | .relationCongruence _ left₁ left₂ right₁ right₂ =>
      [left₁, left₂, right₁, right₂]
  | .sequenceExtensionality elem left right =>
      let diff := sequenceDiffTerm elem left right
      [left, right, diff,
        sequenceLengthTerm elem left,
        sequenceLengthTerm elem right,
        sequenceReadTerm elem left diff,
        sequenceReadTerm elem right diff]

def GroundTheoryStep.valid (ground : GroundUniverse)
    (step : GroundTheoryStep) : Bool :=
  step.structurallyValid
    && (match step with
      | .sequenceExtensionality _ left right => [left, right]
      | _ => step.terms).all fun term =>
      term ∈ ground && term.wellSorted

theorem equality_true_iff (M : Model)
    (interpretation : GroundInterpretation M) (left right : GroundTerm) :
    evalGroundAtom M interpretation (.rel .eq left right) = true ↔
      evalGroundTerm M interpretation left =
        evalGroundTerm M interpretation right :=
  valueEqTagged_eq_true_iff M _ _

theorem argument_values_equal (M : Model)
    (interpretation : GroundInterpretation M) :
    ∀ (left right : List GroundTerm),
      left.length = right.length →
      (equalityPremises left right).all
        (evalGroundAtom M interpretation) = true →
      left.map (evalGroundTerm M interpretation) =
        right.map (evalGroundTerm M interpretation)
  | [], [], _, _ => rfl
  | [], _ :: _, lengths, _ => by simp at lengths
  | _ :: _, [], lengths, _ => by simp at lengths
  | left :: leftTail, right :: rightTail, lengths, premises => by
      simp only [equalityPremises, List.zipWith, List.all_cons,
        Bool.and_eq_true] at premises
      simp only [List.map_cons, List.cons.injEq]
      exact ⟨(equality_true_iff M interpretation left right).mp premises.1,
        argument_values_equal M interpretation leftTail rightTail
          (by simpa using lengths) premises.2⟩

theorem GroundTheoryStep.sound (M : Model)
    (interpretation : GroundInterpretation M)
    (ground : GroundUniverse) (step : GroundTheoryStep)
    (valid : step.valid ground = true) :
    step.clause.eval M interpretation = true := by
  have structurallyValid : step.structurallyValid = true := by
    simp only [GroundTheoryStep.valid, Bool.and_eq_true] at valid
    exact valid.1
  unfold GroundTheoryClause.eval
  cases step with
  | equalityReflexivity term =>
      simp [GroundTheoryStep.clause, evalGroundAtom,
        valueEqTagged_self]
  | equalitySymmetry left right =>
      simp only [GroundTheoryStep.clause, List.all_cons, List.all_nil,
        Bool.and_true]
      cases equal : evalGroundAtom M interpretation (.rel .eq left right)
      · rfl
      · simp only [Bool.not_true, Bool.false_or]
        exact (equality_true_iff M interpretation right left).mpr
          ((equality_true_iff M interpretation left right).mp equal).symm
  | equalityTransitivity left middle right =>
      simp only [GroundTheoryStep.clause, List.all_cons, List.all_nil,
        Bool.and_true]
      cases first : evalGroundAtom M interpretation (.rel .eq left middle)
      · rfl
      · cases second : evalGroundAtom M interpretation (.rel .eq middle right)
        · rfl
        · simp only [Bool.true_and, Bool.not_true, Bool.false_or]
          exact (equality_true_iff M interpretation left right).mpr <|
            ((equality_true_iff M interpretation left middle).mp first).trans
              ((equality_true_iff M interpretation middle right).mp second)
  | functionCongruence fn left right =>
      simp only [GroundTheoryStep.structurallyValid,
        Bool.and_eq_true] at structurallyValid
      have leftSorted := of_decide_eq_true structurallyValid.1
      have rightSorted := of_decide_eq_true structurallyValid.2
      have lengths : left.length = right.length := by
        have leftLength : left.length = fn.arguments.length := by
          simpa using congrArg List.length leftSorted
        have rightLength : right.length = fn.arguments.length := by
          simpa using congrArg List.length rightSorted
        exact leftLength.trans rightLength.symm
      cases premises :
          (equalityPremises left right).all
            (evalGroundAtom M interpretation)
      · simp [GroundTheoryStep.clause, premises]
      · simp only [GroundTheoryStep.clause, premises, Bool.not_true,
          Bool.false_or]
        apply (equality_true_iff M interpretation _ _).mpr
        simp only [GroundTerm.appList, evalGroundTerm]
        rw [evalGroundArguments_ofList, evalGroundArguments_ofList,
          argument_values_equal M interpretation left right lengths premises]
  | relationCongruence relation left₁ left₂ right₁ right₂ =>
      simp only [GroundTheoryStep.clause, List.all_cons, List.all_nil,
        Bool.and_true]
      cases leftEqual :
          evalGroundAtom M interpretation (.rel .eq left₁ left₂)
      · rfl
      · cases rightEqual :
          evalGroundAtom M interpretation (.rel .eq right₁ right₂)
        · rfl
        · cases relationTrue :
            evalGroundAtom M interpretation (.rel relation left₁ right₁)
          · rfl
          · simp only [Bool.true_and, Bool.not_true, Bool.false_or]
            have leftValues :=
              (equality_true_iff M interpretation left₁ left₂).mp leftEqual
            have rightValues :=
              (equality_true_iff M interpretation right₁ right₂).mp rightEqual
            cases relation <;>
              simp only [evalGroundAtom] at relationTrue ⊢ <;>
              rw [← leftValues, ← rightValues] <;>
              exact relationTrue
  | sequenceExtensionality elem left right =>
      simp only [GroundTheoryStep.structurallyValid,
        Bool.and_eq_true] at structurallyValid
      have leftSort := of_decide_eq_true structurallyValid.1
      have rightSort := of_decide_eq_true structurallyValid.2
      simp only [GroundTheoryStep.clause, List.all_cons, List.all_nil,
        Bool.and_true]
      cases sameLength :
          evalGroundAtom M interpretation
            (.rel .eq
              (sequenceLengthTerm elem left)
              (sequenceLengthTerm elem right))
      · rfl
      · cases sameRead :
          evalGroundAtom M interpretation
            (.rel .eq
              (sequenceReadTerm elem left
                (sequenceDiffTerm elem left right))
              (sequenceReadTerm elem right
                (sequenceDiffTerm elem left right)))
        · rfl
        · simp only [Bool.true_and, Bool.not_true, Bool.false_or]
          apply (equality_true_iff M interpretation left right).mpr
          cases leftValue :
              evalGroundTerm M interpretation left with
          | mk leftActual leftCarrier =>
              have leftActualEq : leftActual = .seq elem := by
                calc
                  leftActual =
                      (evalGroundTerm M interpretation left).fst := by
                        rw [leftValue]
                  _ = left.sortOf :=
                    evalGroundTerm_sortOf M interpretation left
                  _ = .seq elem := leftSort
              subst leftActual
              cases rightValue :
                  evalGroundTerm M interpretation right with
              | mk rightActual rightCarrier =>
                  have rightActualEq : rightActual = .seq elem := by
                    calc
                      rightActual =
                          (evalGroundTerm M interpretation right).fst := by
                            rw [rightValue]
                      _ = right.sortOf :=
                        evalGroundTerm_sortOf M interpretation right
                      _ = .seq elem := rightSort
                  subst rightActual
                  have lengths :
                      M.len (.seq elem) leftCarrier =
                        M.len (.seq elem) rightCarrier := by
                    have tagged :=
                      (equality_true_iff M interpretation
                        (sequenceLengthTerm elem left)
                        (sequenceLengthTerm elem right)).mp sameLength
                    simp only [sequenceLengthTerm, GroundTerm.appList,
                      GroundArguments.ofList, evalGroundTerm,
                      evalGroundArguments, evalGroundFunction] at tagged
                    rw [leftValue, rightValue] at tagged
                    simpa using tagged
                  have reads :
                      M.read elem (.seq elem) usizeS leftCarrier
                          (M.seqDiff elem leftCarrier rightCarrier) =
                        M.read elem (.seq elem) usizeS rightCarrier
                          (M.seqDiff elem leftCarrier rightCarrier) := by
                    have tagged :=
                      (equality_true_iff M interpretation
                        (sequenceReadTerm elem left
                          (sequenceDiffTerm elem left right))
                        (sequenceReadTerm elem right
                          (sequenceDiffTerm elem left right))).mp sameRead
                    simp only [sequenceReadTerm, sequenceDiffTerm,
                      sequenceDiffFunction, GroundTerm.appList,
                      GroundArguments.ofList, evalGroundTerm,
                      evalGroundArguments, evalGroundFunction] at tagged
                    rw [leftValue, rightValue] at tagged
                    simpa using tagged
                  have equal :=
                    M.seq_ext_at_diff elem leftCarrier rightCarrier
                      lengths reads
                  simpa [leftValue, rightValue] using
                    congrArg (fun value => (⟨.seq elem, value⟩ : Value M))
                      equal

def sequenceLengthEquality (elem : Sort₂)
    (left right : GroundTerm) : GroundAtom :=
  .rel .eq
    (sequenceLengthTerm elem left)
    (sequenceLengthTerm elem right)

def sequenceEquality (left right : GroundTerm) : GroundAtom :=
  .rel .eq left right

def sequenceLowerBound (elem : Sort₂)
    (left right : GroundTerm) : GroundAtom :=
  .rel .le
    (.constant usizeS (.literal (.int 0)))
    (sequenceDiffTerm elem left right)

def sequenceUpperBound (elem : Sort₂)
    (left right : GroundTerm) : GroundAtom :=
  .rel .lt
    (sequenceDiffTerm elem left right)
    (sequenceLengthTerm elem left)

def GroundTheoryStep.cnfClauses : GroundTheoryStep → List GroundCnfClause
  | step@(.sequenceExtensionality elem left right) =>
      let lengthEqual := sequenceLengthEquality elem left right
      let sequencesEqual := sequenceEquality left right
      [step.clause.cnfClause,
        [(lengthEqual, false), (sequencesEqual, true),
          (sequenceLowerBound elem left right, true)],
        [(lengthEqual, false), (sequencesEqual, true),
          (sequenceUpperBound elem left right, true)]]
  | step => [step.clause.cnfClause]

def evalGroundCnfLiteral (M : Model)
    (interpretation : GroundInterpretation M)
    (literal : GroundCnfLiteral) : Bool :=
  evalGroundAtom M interpretation literal.1 == literal.2

def evalGroundCnfClause (M : Model)
    (interpretation : GroundInterpretation M)
    (clause : GroundCnfClause) : Bool :=
  clause.any (evalGroundCnfLiteral M interpretation)

theorem evalGroundTheoryClause_cnfClause (M : Model)
    (interpretation : GroundInterpretation M)
    (clause : GroundTheoryClause) :
    evalGroundCnfClause M interpretation clause.cnfClause =
      clause.eval M interpretation := by
  cases clause with
  | mk premises conclusion =>
      induction premises with
      | nil =>
          simp [GroundTheoryClause.cnfClause,
            evalGroundCnfClause, evalGroundCnfLiteral,
            GroundTheoryClause.eval]
      | cons premise rest ih =>
          simp only [GroundTheoryClause.cnfClause, List.map_cons,
            List.cons_append, evalGroundCnfClause, List.any_cons,
            evalGroundCnfLiteral, GroundTheoryClause.eval,
            List.all_cons]
          change
            (evalGroundAtom M interpretation premise == false ||
                evalGroundCnfClause M interpretation
                  (rest.map (·, false) ++ [(conclusion, true)])) =
              (!(evalGroundAtom M interpretation premise &&
                  rest.all (evalGroundAtom M interpretation)) ||
                evalGroundAtom M interpretation conclusion)
          have tail :=
            show
              evalGroundCnfClause M interpretation
                  (rest.map (·, false) ++ [(conclusion, true)]) =
                (⟨rest, conclusion⟩ : GroundTheoryClause).eval
                  M interpretation by
              simpa [GroundTheoryClause.cnfClause] using ih
          rw [tail]
          simp only [GroundTheoryClause.eval]
          cases evalGroundAtom M interpretation premise <;>
            cases rest.all (evalGroundAtom M interpretation) <;>
            cases evalGroundAtom M interpretation conclusion <;>
            rfl

def GroundTheoryStep.formula : GroundTheoryStep → GroundFrm
  | step@(.sequenceExtensionality elem left right) =>
      let lengthEqual := GroundFrm.atom <|
        sequenceLengthEquality elem left right
      let sequencesEqual := GroundFrm.atom <|
        sequenceEquality left right
      let lower := GroundFrm.atom <|
        sequenceLowerBound elem left right
      let upper := GroundFrm.atom <|
        sequenceUpperBound elem left right
      .conj
        (GroundFrm.implies lengthEqual
          (.disj sequencesEqual lower))
        (.conj
          (GroundFrm.implies lengthEqual
            (.disj sequencesEqual upper))
          step.clause.toFormula)
  | step => step.clause.toFormula

theorem GroundTheoryStep.eval_formula (M : Model)
    (interpretation : GroundInterpretation M)
    (ground : GroundUniverse) (step : GroundTheoryStep)
    (valid : step.valid ground = true) :
    evalGroundFrm M interpretation step.formula = true := by
  have clauseTrue := step.sound M interpretation ground valid
  cases step with
  | equalityReflexivity term
  | equalitySymmetry left right
  | equalityTransitivity left middle right
  | functionCongruence fn left right
  | relationCongruence relation left₁ left₂ right₁ right₂ =>
      simpa [GroundTheoryStep.formula,
        GroundTheoryClause.eval_toFormula] using clauseTrue
  | sequenceExtensionality elem left right =>
      have structurallyValid :
          (GroundTheoryStep.sequenceExtensionality
            elem left right).structurallyValid = true := by
        simp only [GroundTheoryStep.valid, Bool.and_eq_true] at valid
        exact valid.1
      simp only [GroundTheoryStep.structurallyValid,
        Bool.and_eq_true] at structurallyValid
      have leftSort := of_decide_eq_true structurallyValid.1
      have rightSort := of_decide_eq_true structurallyValid.2
      cases lengthValue :
          evalGroundAtom M interpretation
            (sequenceLengthEquality elem left right)
      · simp [GroundTheoryStep.formula, GroundFrm.implies,
          evalGroundFrm, lengthValue,
          GroundTheoryClause.eval_toFormula, clauseTrue]
      · cases equalityValue :
          evalGroundAtom M interpretation
            (sequenceEquality left right)
        · cases leftValue :
              evalGroundTerm M interpretation left with
          | mk leftActual leftCarrier =>
              have leftActualEq : leftActual = .seq elem := by
                calc
                  leftActual =
                      (evalGroundTerm M interpretation left).fst := by
                        rw [leftValue]
                  _ = left.sortOf :=
                    evalGroundTerm_sortOf M interpretation left
                  _ = .seq elem := leftSort
              subst leftActual
              cases rightValue :
                  evalGroundTerm M interpretation right with
              | mk rightActual rightCarrier =>
                  have rightActualEq : rightActual = .seq elem := by
                    calc
                      rightActual =
                          (evalGroundTerm M interpretation right).fst := by
                            rw [rightValue]
                      _ = right.sortOf :=
                        evalGroundTerm_sortOf M interpretation right
                      _ = .seq elem := rightSort
                  subst rightActual
                  have lengths :
                      M.len (.seq elem) leftCarrier =
                        M.len (.seq elem) rightCarrier := by
                    have tagged :=
                      (equality_true_iff M interpretation
                        (sequenceLengthTerm elem left)
                        (sequenceLengthTerm elem right)).mp lengthValue
                    simp only [sequenceLengthTerm, GroundTerm.appList,
                      GroundArguments.ofList, evalGroundTerm,
                      evalGroundArguments, evalGroundFunction] at tagged
                    rw [leftValue, rightValue] at tagged
                    simpa using tagged
                  have different : leftCarrier ≠ rightCarrier := by
                    intro equal
                    have tagged :
                        evalGroundTerm M interpretation left =
                          evalGroundTerm M interpretation right := by
                      rw [leftValue, rightValue, equal]
                    have atomTrue :=
                      (equality_true_iff M interpretation left right).mpr
                        tagged
                    simp only [sequenceEquality] at equalityValue
                    rw [equalityValue] at atomTrue
                    contradiction
                  have lower :=
                    M.seqDiff_lower_bound elem leftCarrier rightCarrier
                      lengths different
                  have upper :=
                    M.seqDiff_upper_bound elem leftCarrier rightCarrier
                      lengths different
                  have lowerEvaluated :
                      evalGroundAtom M interpretation
                        (sequenceLowerBound elem left right) = true := by
                    simp only [sequenceLowerBound, evalGroundAtom,
                      orderTagged, sequenceDiffTerm,
                      sequenceDiffFunction, GroundTerm.appList,
                      GroundArguments.ofList, evalGroundTerm,
                      evalGroundArguments, evalGroundFunction]
                    rw [leftValue, rightValue]
                    simpa [evalGroundFunction] using lower
                  have upperEvaluated :
                      evalGroundAtom M interpretation
                        (sequenceUpperBound elem left right) = true := by
                    simp only [sequenceUpperBound, evalGroundAtom,
                      orderTagged, sequenceDiffTerm,
                      sequenceDiffFunction, sequenceLengthTerm,
                      GroundTerm.appList, GroundArguments.ofList,
                      evalGroundTerm, evalGroundArguments,
                      evalGroundFunction]
                    rw [leftValue, rightValue]
                    simpa [evalGroundFunction] using upper
                  simp [GroundTheoryStep.formula, GroundFrm.implies,
                    evalGroundFrm, lengthValue, equalityValue,
                    lowerEvaluated, upperEvaluated,
                    GroundTheoryClause.eval_toFormula, clauseTrue]
        · simp [GroundTheoryStep.formula, GroundFrm.implies,
            evalGroundFrm, lengthValue, equalityValue,
            GroundTheoryClause.eval_toFormula, clauseTrue]

theorem GroundTheoryStep.eval_cnfClauses (M : Model)
    (interpretation : GroundInterpretation M)
    (ground : GroundUniverse) (step : GroundTheoryStep)
    (valid : step.valid ground = true) :
    step.cnfClauses.all
      (evalGroundCnfClause M interpretation) = true := by
  cases step with
  | equalityReflexivity term
  | equalitySymmetry left right
  | equalityTransitivity left middle right
  | functionCongruence fn left right
  | relationCongruence relation left₁ left₂ right₁ right₂ =>
      have clauseTrue :=
        GroundTheoryStep.sound M interpretation ground _ valid
      simpa [GroundTheoryStep.cnfClauses,
        evalGroundTheoryClause_cnfClause] using clauseTrue
  | sequenceExtensionality elem left right =>
      have clauseTrue :=
        GroundTheoryStep.sound M interpretation ground
          (.sequenceExtensionality elem left right) valid
      have mainTrue :
          evalGroundCnfClause M interpretation
              ((GroundTheoryStep.sequenceExtensionality
                elem left right).clause.cnfClause) = true := by
        rw [evalGroundTheoryClause_cnfClause]
        exact clauseTrue
      simp only [GroundTheoryStep.cnfClauses, List.all_cons,
        List.all_nil, Bool.and_true]
      rw [mainTrue]
      have formulaTrue :=
        GroundTheoryStep.eval_formula M interpretation ground
          (.sequenceExtensionality elem left right) valid
      cases lengthValue :
        evalGroundAtom M interpretation
          (sequenceLengthEquality elem left right)
      · simp [evalGroundCnfClause, evalGroundCnfLiteral,
          lengthValue]
      · cases equalityValue :
          evalGroundAtom M interpretation
            (sequenceEquality left right)
        · have bounds :
              evalGroundAtom M interpretation
                    (sequenceLowerBound elem left right) = true ∧
                evalGroundAtom M interpretation
                    (sequenceUpperBound elem left right) = true := by
            simpa [GroundTheoryStep.formula, GroundFrm.implies,
              evalGroundFrm, lengthValue, equalityValue,
              GroundTheoryClause.eval_toFormula, clauseTrue] using
                formulaTrue
          simp [evalGroundCnfClause, evalGroundCnfLiteral,
            lengthValue, equalityValue, bounds.1, bounds.2]
        · simp [evalGroundCnfClause, evalGroundCnfLiteral,
            lengthValue, equalityValue]

def theoryFormula (steps : List GroundTheoryStep) : GroundFrm :=
  GroundFrm.conjoin (steps.map GroundTheoryStep.formula)

def verifyTheory (ground : GroundUniverse)
    (steps : List GroundTheoryStep) : Bool :=
  steps.all (GroundTheoryStep.valid ground)

/-- Equality and congruence instances whose conclusions occur in the grounded
    problem. Premises may use any matching ground term, but building rules for
    conclusions that no source or theory formula can observe only creates a
    cubic amount of dead CNF. -/
def exhaustiveTheory (ground : GroundUniverse)
    (formula : GroundFrm) : List GroundTheoryStep :=
  let atoms :=
    formula.atoms.flatMap fun atom =>
      match atom with
      | .rel .ne left right =>
          [atom, .rel .eq left right]
      | _ => [atom]
  let equalities :=
    atoms.filterMap fun atom =>
      match atom with
      | .rel .eq left right => some (left, right)
      | _ => none
  let hasEquality := fun left right =>
    decide (.rel .eq left right ∈ atoms) ||
      decide (.rel .eq right left ∈ atoms)
  let reflexivity :=
    equalities.filterMap fun (left, right) =>
      if left = right then
        some <| GroundTheoryStep.equalityReflexivity left
      else
        none
  let symmetry :=
    equalities.map fun (left, right) =>
      GroundTheoryStep.equalitySymmetry right left
  let transitivity :=
    equalities.flatMap fun (left, right) =>
      ground.filterMap fun middle =>
        if hasEquality left middle &&
            hasEquality middle right then
          some <| GroundTheoryStep.equalityTransitivity
            left middle right
        else
          none
  let functionCongruence :=
    equalities.filterMap fun (left, right) =>
      match left, right with
      | .app fn leftArguments, .app rightFn rightArguments =>
          if rightFn = fn &&
              (equalityPremises leftArguments.toList
                rightArguments.toList).all
                (fun premise => decide (premise ∈ atoms)) then
            some <| GroundTheoryStep.functionCongruence fn
              leftArguments.toList rightArguments.toList
          else
            none
      | _, _ => none
  let relations :=
    atoms.filterMap fun atom =>
      match atom with
      | .qfree _ => none
      | .rel .ne _ _ => none
      | .rel relation left right => some (relation, left, right)
  let relationCongruence :=
    relations.flatMap fun (relation, left₂, right₂) =>
      relations.filterMap fun (other, left₁, right₁) =>
        if other = relation &&
            hasEquality left₁ left₂ &&
            hasEquality right₁ right₂ &&
            decide (left₁ ≠ left₂ ∨ right₁ ≠ right₂) then
          some <| GroundTheoryStep.relationCongruence relation
            left₁ left₂ right₁ right₂
        else
          none
  let sequenceExtensionality :=
    equalities.filterMap fun (left, right) =>
      match left.sortOf, right.sortOf with
      | .seq elem, .seq rightElem =>
          if rightElem = elem then
            some <| GroundTheoryStep.sequenceExtensionality
              elem left right
          else
            none
      | _, _ => none
  reflexivity ++ symmetry ++ transitivity ++
    functionCongruence ++ relationCongruence ++
      sequenceExtensionality

/-- Keep exactly the exhaustive equality and congruence steps whose structural
    side conditions hold in the recomputed ground universe. The filter is part
    of the deterministic builder, not an external trust decision. -/
def checkedExhaustiveTheory (ground : GroundUniverse)
    (formula : GroundFrm) : List GroundTheoryStep :=
  (exhaustiveTheory ground formula).filter
    (GroundTheoryStep.valid ground)

@[simp]
theorem verifyTheory_checkedExhaustiveTheory
    (ground : GroundUniverse) (formula : GroundFrm) :
    verifyTheory ground
      (checkedExhaustiveTheory ground formula) = true := by
  simp [verifyTheory, checkedExhaustiveTheory]

/-- Whether a theory rule can add information in the canonical Horn closure.
    Reflexive equalities have a unique direct rule; deriving the same fact via
    symmetry, transitivity, or congruence only expands the dependency cone
    without strengthening the CNF. -/
def GroundTheoryStep.contributes (step : GroundTheoryStep) : Bool :=
  if step.clause.conclusion ∈ step.clause.premises then
    false
  else
    match step, step.clause.conclusion with
    | .equalityReflexivity _, _ => true
    | _, .rel .eq left right => decide (left ≠ right)
    | _, _ => true

/-- One backward dependency pass from theory conclusions that can affect the
    grounded source formula to the premises needed to establish them. -/
def expandRelevantTheoryAtoms (steps : List GroundTheoryStep)
    (atoms : List GroundAtom) : List GroundAtom :=
  steps.foldl
    (fun relevant step =>
      if step.contributes &&
          decide (step.clause.conclusion ∈ relevant) then
        (step.clause.premises ++ relevant).eraseDups
      else
        relevant)
    atoms

def closeRelevantTheoryAtoms (steps : List GroundTheoryStep) :
    Nat → List GroundAtom → List GroundAtom
  | 0, atoms => atoms
  | fuel + 1, atoms =>
      closeRelevantTheoryAtoms steps fuel
        (expandRelevantTheoryAtoms steps atoms)

/-- The sound exhaustive closure sliced to the Horn dependency cone of atoms
    visible in the grounded formula. Rules whose conclusion is already a
    premise are propositional tautologies and cannot contribute to a
    refutation, so they are omitted deterministically. -/
def checkedRelevantTheory (ground : GroundUniverse)
    (formula : GroundFrm) : List GroundTheoryStep :=
  let steps := checkedExhaustiveTheory ground formula
  let formulaAtoms :=
    formula.atoms.flatMap fun atom =>
      match atom with
      | .rel .ne left right =>
          [atom, .rel .eq left right]
      | _ => [atom]
  let relevant :=
    closeRelevantTheoryAtoms steps steps.length formulaAtoms
  steps.filter fun step =>
    step.contributes &&
      decide (step.clause.conclusion ∈ relevant)

@[simp]
theorem verifyTheory_checkedRelevantTheory
    (ground : GroundUniverse) (formula : GroundFrm) :
    verifyTheory ground
      (checkedRelevantTheory ground formula) = true := by
  simp only [verifyTheory, List.all_eq_true]
  intro step stepMember
  simp only [checkedRelevantTheory, List.mem_filter] at stepMember
  exact
    (List.all_eq_true.mp
      (verifyTheory_checkedExhaustiveTheory ground formula))
      step stepMember.1

theorem eval_theoryFormula (M : Model)
    (interpretation : GroundInterpretation M)
    (ground : GroundUniverse) (steps : List GroundTheoryStep)
    (verified : verifyTheory ground steps = true) :
    evalGroundFrm M interpretation (theoryFormula steps) = true := by
  simp only [verifyTheory, List.all_eq_true] at verified
  rw [theoryFormula, evalGroundFrm_conjoin, List.all_map,
    List.all_eq_true]
  intro step member
  exact step.eval_formula M interpretation ground
    (verified step (by simpa using member))

#print axioms GroundTheoryStep.sound
#print axioms GroundTheoryStep.eval_formula
#print axioms eval_theoryFormula

end Thermite.Strat.Cls
