/-
  Semantic replay for a checked ground formula.

  Interpreted source operations are evaluated directly in the typed model.
  Only Skolem functions and qfree IDs are supplied by the interpretation.
  The generic propositional compiler then turns an LRAT refutation into a
  theorem that the checked ground formula is false in every such
  interpretation.
-/
import Thermite.Strat.Instantiation
import Thermite.PropReconstruct
import Mathlib.Data.List.Basic
import Mathlib.Data.List.Nodup

open Std.Tactic.BVDecide
open Std.Sat

namespace Thermite.Strat.Cls

structure GroundInterpretation (M : Model) where
  qfree : Nat → Bool
  skolem : (id : Nat) → (arguments : List (Value M)) →
    (result : Sort₂) → M.Carrier result

def SkolemTree.lookup (M : Model) {binders : Prefix}
    (tree : SkolemTree M binders) (id : Nat)
    (arguments : List (Value M)) (result : Sort₂) :
    M.Carrier result :=
  match tree with
  | .done => M.default result
  | @SkolemTree.all _ sort _ next =>
      match arguments with
      | [] => M.default result
      | ⟨actual, value⟩ :: rest =>
          if same : actual = sort then
            (next (same ▸ value)).lookup M id rest result
          else M.default result
  | @SkolemTree.ex _ sort _ value _ next =>
      match id with
      | 0 =>
          if same : sort = result then
            same ▸ value
          else M.default result
      | id + 1 => next.lookup M id arguments result

mutual
  def evalGroundTerm (M : Model) (interpretation : GroundInterpretation M) :
      GroundTerm → Value M
    | .constant sort (.source id) => ⟨sort, M.constant sort id⟩
    | .constant sort (.literal value) => ⟨sort, M.literal sort value⟩
    | .constant sort .inhabitant => ⟨sort, M.default sort⟩
    | .app fn arguments =>
        let values := evalGroundArguments M interpretation arguments
        ⟨fn.result, evalGroundFunction M interpretation fn values⟩

  def evalGroundArguments (M : Model)
      (interpretation : GroundInterpretation M) :
      GroundArguments → List (Value M)
    | .nil => []
    | .cons head tail =>
        evalGroundTerm M interpretation head ::
          evalGroundArguments M interpretation tail

  def evalGroundFunction (M : Model)
      (interpretation : GroundInterpretation M)
      (fn : GroundFunction) (arguments : List (Value M)) :
      M.Carrier fn.result :=
    match fn.kind, arguments with
    | .source id, [⟨actual, value⟩] =>
        M.app1 (fn.arguments.head?.getD actual) fn.result actual id value
    | .read elem, [⟨baseSort, base⟩, ⟨indexSort, index⟩] =>
        if same : elem = fn.result then
          same ▸ M.read elem baseSort indexSort base index
        else M.default fn.result
    | .len, [⟨baseSort, base⟩] =>
        if same : usizeS = fn.result then
          same ▸ M.len baseSort base
        else M.default fn.result
    | .seqDiff elem,
        [⟨.seq leftElem, left⟩, ⟨.seq rightElem, right⟩] =>
        if leftSame : leftElem = elem then
          if rightSame : rightElem = elem then
            if resultSame : usizeS = fn.result then
              resultSame ▸ M.seqDiff elem
                (leftSame ▸ left) (rightSame ▸ right)
            else M.default fn.result
          else M.default fn.result
        else M.default fn.result
    | .cast, [⟨source, value⟩] =>
        M.cast source fn.result value
    | .offset offset, [⟨source, value⟩] =>
        if same : source = fn.result then
          same ▸ M.idxOffset source value offset
        else M.default fn.result
    | .mul, [⟨leftSort, left⟩, ⟨rightSort, right⟩] =>
        if same : leftSort = fn.result then
          same ▸ M.mul leftSort rightSort left right
        else M.default fn.result
    | .skolem id, values =>
        interpretation.skolem id values fn.result
    | _, _ => M.default fn.result
end

@[simp]
theorem evalGroundTerm_sortOf (M : Model)
    (interpretation : GroundInterpretation M) (term : GroundTerm) :
    (evalGroundTerm M interpretation term).fst = term.sortOf := by
  cases term with
  | constant sort constant => cases constant <;> rfl
  | app fn arguments => rfl

theorem evalGroundArguments_ofList (M : Model)
    (interpretation : GroundInterpretation M) (arguments : List GroundTerm) :
    evalGroundArguments M interpretation (GroundArguments.ofList arguments) =
      arguments.map (evalGroundTerm M interpretation) := by
  induction arguments with
  | nil => rfl
  | cons head tail ih =>
      simp [GroundArguments.ofList, evalGroundArguments, ih]

/-- The ground terms assigned to the current de Bruijn environment denote the
    same typed values as the source valuation. Stating the invariant through
    `groundTermAt` makes the out-of-scope fallback explicit. -/
def GroundEnvironmentAgrees (M : Model)
    (interpretation : GroundInterpretation M)
    (context : List Sort₂) (environment : List GroundTerm)
    (ρ : Valuation M) : Prop :=
  ∀ sort index,
    context[index]? = some sort →
    evalGroundTerm M interpretation
        (groundTermAt environment sort index) =
      ⟨sort, M.valueAt ρ sort index⟩

theorem GroundEnvironmentAgrees.nil (M : Model)
    (interpretation : GroundInterpretation M) (ρ : Valuation M) :
    GroundEnvironmentAgrees M interpretation [] [] ρ := by
  intro expected index inScope
  simp at inScope

theorem GroundEnvironmentAgrees.cons (M : Model)
    (interpretation : GroundInterpretation M)
    {context : List Sort₂} {environment : List GroundTerm}
    {ρ : Valuation M} {sort : Sort₂} {term : GroundTerm}
    {value : M.Carrier sort}
    (agrees :
      GroundEnvironmentAgrees M interpretation context environment ρ)
    (evaluates :
      evalGroundTerm M interpretation term = ⟨sort, value⟩) :
    GroundEnvironmentAgrees M interpretation (sort :: context)
      (term :: environment) (Valuation.cons M sort value ρ) := by
  intro expected index inScope
  cases index with
  | zero =>
      simp only [List.getElem?_cons_zero, Option.some.injEq] at inScope
      subst expected
      simpa [groundTermAt, Valuation.cons, Model.valueAt] using evaluates
  | succ index =>
      simp only [List.getElem?_cons_succ] at inScope
      simpa [groundTermAt, Valuation.cons] using
        agrees expected index inScope

theorem evalGroundTerm_groundTm (M : Model)
    (interpretation : GroundInterpretation M)
    (context : List Sort₂) (environment : List GroundTerm)
    (ρ : Valuation M)
    (agrees :
      GroundEnvironmentAgrees M interpretation context environment ρ) :
    ∀ term : Tm,
      wellSortedTm context term = true →
      evalGroundTerm M interpretation (groundTm environment term) =
        evalTm M ρ term := by
  intro term
  induction term with
  | var sort index =>
      intro sorted
      apply agrees
      exact of_decide_eq_true sorted
  | const sort id => intro _; rfl
  | lit sort value => intro _; rfl
  | read elem sequence index sequenceIH indexIH =>
      intro sorted
      simp only [wellSortedTm, Bool.and_eq_true] at sorted
      simp only [groundTm, GroundTerm.appList, GroundArguments.ofList,
        evalGroundTerm, evalGroundArguments, evalGroundFunction, evalTm]
      rw [sequenceIH sorted.1.2, indexIH sorted.2]
      simp
  | len sequence ih =>
      intro sorted
      simp only [wellSortedTm, Bool.and_eq_true] at sorted
      simp only [groundTm, GroundTerm.appList, GroundArguments.ofList,
        evalGroundTerm, evalGroundArguments, evalGroundFunction, evalTm]
      rw [ih sorted.2]
      simp
  | cast target term ih =>
      intro sorted
      simp only [wellSortedTm] at sorted
      simp only [groundTm, GroundTerm.appList, GroundArguments.ofList,
        evalGroundTerm, evalGroundArguments, evalGroundFunction, evalTm]
      rw [ih sorted]
  | idxOp term offset ih =>
      intro sorted
      simp only [wellSortedTm] at sorted
      have sortEq :
          (groundTm environment term).sortOf =
            (evalTm M ρ term).fst := by
        rw [← evalGroundTerm_sortOf M interpretation,
          ih sorted]
      simp only [groundTm, GroundTerm.appList, GroundArguments.ofList,
        evalGroundTerm, evalGroundArguments, evalGroundFunction, evalTm]
      rw [ih sorted]
      rw [sortEq]
      simp
  | mul left right leftIH rightIH =>
      intro sorted
      simp only [wellSortedTm, Bool.and_eq_true] at sorted
      have leftSortEq :
          (groundTm environment left).sortOf =
            (evalTm M ρ left).fst := by
        rw [← evalGroundTerm_sortOf M interpretation,
          leftIH sorted.1.2]
      simp only [groundTm, GroundTerm.appList, GroundArguments.ofList,
        evalGroundTerm, evalGroundArguments, evalGroundFunction, evalTm]
      rw [leftIH sorted.1.2, rightIH sorted.2]
      rw [leftSortEq]
      simp
  | app1 argument result id term ih =>
      intro sorted
      simp only [wellSortedTm, Bool.and_eq_true] at sorted
      simp only [groundTm, GroundTerm.appList, GroundArguments.ofList,
        evalGroundTerm, evalGroundArguments, evalGroundFunction, evalTm]
      rw [ih sorted.2]
      simp

def evalGroundAtom (M : Model) (interpretation : GroundInterpretation M) :
    GroundAtom → Bool
  | .qfree id => interpretation.qfree id
  | .rel .eq left right =>
      valueEqTagged M
        (evalGroundTerm M interpretation left)
        (evalGroundTerm M interpretation right)
  | .rel .ne left right =>
      !valueEqTagged M
        (evalGroundTerm M interpretation left)
        (evalGroundTerm M interpretation right)
  | .rel relation left right =>
      orderTagged M relation
        (evalGroundTerm M interpretation left)
        (evalGroundTerm M interpretation right)

def evalGroundFrm (M : Model) (interpretation : GroundInterpretation M) :
    GroundFrm → Bool
  | .const value => value
  | .atom atom => evalGroundAtom M interpretation atom
  | .neg formula => !evalGroundFrm M interpretation formula
  | .conj left right =>
      evalGroundFrm M interpretation left &&
        evalGroundFrm M interpretation right
  | .disj left right =>
      evalGroundFrm M interpretation left ||
        evalGroundFrm M interpretation right

theorem evalGroundFrm_conjoin (M : Model)
    (interpretation : GroundInterpretation M) (formulas : List GroundFrm) :
    evalGroundFrm M interpretation (GroundFrm.conjoin formulas) =
      formulas.all (evalGroundFrm M interpretation) := by
  induction formulas with
  | nil => rfl
  | cons formula rest ih =>
      cases rest <;>
        simp_all [GroundFrm.conjoin, evalGroundFrm]

inductive QfreeOccurs (id : Nat) (expression : Thermite.Expr) : Frm → Prop
  | here : QfreeOccurs id expression (.atom (.qfree id expression))
  | neg {formula} :
      QfreeOccurs id expression formula →
      QfreeOccurs id expression (.neg formula)
  | conjLeft {left right} :
      QfreeOccurs id expression left →
      QfreeOccurs id expression (.conj left right)
  | conjRight {left right} :
      QfreeOccurs id expression right →
      QfreeOccurs id expression (.conj left right)
  | disjLeft {left right} :
      QfreeOccurs id expression left →
      QfreeOccurs id expression (.disj left right)
  | disjRight {left right} :
      QfreeOccurs id expression right →
      QfreeOccurs id expression (.disj left right)
  | impLeft {left right} :
      QfreeOccurs id expression left →
      QfreeOccurs id expression (.imp left right)
  | impRight {left right} :
      QfreeOccurs id expression right →
      QfreeOccurs id expression (.imp left right)
  | all {sort body} :
      QfreeOccurs id expression body →
      QfreeOccurs id expression (.all sort body)
  | ex {sort body} :
      QfreeOccurs id expression body →
      QfreeOccurs id expression (.ex sort body)

theorem qfreeId_mem_of_occurs {id : Nat} {expression : Thermite.Expr}
    {formula : Frm} (occurs : QfreeOccurs id expression formula) :
    id ∈ formula.qfreeIds := by
  induction occurs <;> simp_all [Frm.qfreeIds]

/-- Evaluate the qfree leaf addressed by `id` directly from the normalized
    source matrix. Unique IDs make this lookup independent of traversal order. -/
def qfreeValue (M : Model) : Frm → Nat → Bool
  | .atom (.qfree candidate expression), id =>
      if id = candidate then M.qfree expression else false
  | .atom (.rel _ _ _), _ => false
  | .neg formula, id => qfreeValue M formula id
  | .conj left right, id | .disj left right, id
    | .imp left right, id =>
      if id ∈ left.qfreeIds then qfreeValue M left id
      else qfreeValue M right id
  | .all _ body, id | .ex _ body, id => qfreeValue M body id

theorem qfreeValue_of_occurs (M : Model) {id : Nat}
    {expression : Thermite.Expr} {formula : Frm}
    (occurs : QfreeOccurs id expression formula)
    (unique : formula.qfreeIds.Nodup) :
    qfreeValue M formula id = M.qfree expression := by
  induction occurs with
  | here => simp [qfreeValue]
  | neg occurs ih =>
      exact ih unique
  | conjLeft occurs ih | disjLeft occurs ih | impLeft occurs ih =>
      have member := qfreeId_mem_of_occurs occurs
      simp only [Frm.qfreeIds] at unique
      simp [qfreeValue, member, ih (List.Nodup.of_append_left unique)]
  | conjRight occurs ih | disjRight occurs ih | impRight occurs ih =>
      have rightMember := qfreeId_mem_of_occurs occurs
      simp only [Frm.qfreeIds] at unique
      have separate := List.disjoint_of_nodup_append unique
      simp only [qfreeValue]
      split
      · rename_i leftMember
        exact False.elim
          (List.disjoint_left.mp separate leftMember rightMember)
      · exact ih (List.Nodup.of_append_right unique)
  | all occurs ih | ex occurs ih =>
      exact ih unique

def QfreeAgreementFor (M : Model)
    (interpretation : GroundInterpretation M) (formula : Frm) : Prop :=
  ∀ id expression,
    QfreeOccurs id expression formula →
    interpretation.qfree id = M.qfree expression

theorem qfreeAgreementFor_value (M : Model) (formula : Frm)
    (skolem : (id : Nat) → List (Value M) →
      (result : Sort₂) → M.Carrier result)
    (unique : formula.qfreeIds.Nodup) :
    QfreeAgreementFor M
      { qfree := qfreeValue M formula, skolem } formula := by
  intro id expression occurs
  exact qfreeValue_of_occurs M occurs unique

def SkolemAgreement (M : Model) {binders : Prefix}
    (tree : SkolemTree M binders)
    (interpretation : GroundInterpretation M)
    (universalValues : List (Value M)) (nextSkolem : Nat) : Prop :=
  match tree with
  | .done => True
  | .all next =>
      ∀ value,
        SkolemAgreement M (next value) interpretation
          (universalValues ++ [⟨_, value⟩]) nextSkolem
  | .ex value _ next =>
      interpretation.skolem nextSkolem universalValues _ = value
        ∧ SkolemAgreement M next interpretation universalValues
          (nextSkolem + 1)

theorem skolemAgreement_of_lookup (M : Model) {binders : Prefix}
    (tree : SkolemTree M binders)
    (interpretation : GroundInterpretation M)
    (universalValues : List (Value M)) (nextSkolem : Nat)
    (lookup :
      ∀ id rest result,
        interpretation.skolem (nextSkolem + id)
            (universalValues ++ rest) result =
          tree.lookup M id rest result) :
    SkolemAgreement M tree interpretation universalValues nextSkolem := by
  induction tree generalizing universalValues nextSkolem with
  | done => trivial
  | @all sort rest next ih =>
      intro value
      apply ih value
      intro id tail result
      simpa [SkolemTree.lookup, List.append_assoc] using
        lookup id (⟨sort, value⟩ :: tail) result
  | @ex sort rest value member next ih =>
      constructor
      · have current := lookup 0 [] sort
        simpa [SkolemTree.lookup] using current
      · apply ih
        intro id tail result
        have later := lookup (id + 1) tail result
        simpa [SkolemTree.lookup, Nat.add_assoc, Nat.add_comm,
          Nat.add_left_comm] using later

def GroundInterpretation.ofTree (M : Model) {binders : Prefix}
    (matrix : Frm) (tree : SkolemTree M binders) :
    GroundInterpretation M :=
  { qfree := qfreeValue M matrix
    skolem := tree.lookup M }

theorem GroundInterpretation.ofTree_skolemAgreement
    (M : Model) {binders : Prefix} (matrix : Frm)
    (tree : SkolemTree M binders) :
    SkolemAgreement M tree (GroundInterpretation.ofTree M matrix tree) [] 0 := by
  apply skolemAgreement_of_lookup
  intro id rest result
  simp [GroundInterpretation.ofTree]

theorem evalGroundAtom_groundAtom (M : Model)
    (interpretation : GroundInterpretation M)
    (context : List Sort₂) (environment : List GroundTerm)
    (ρ : Valuation M)
    (environmentAgrees :
      GroundEnvironmentAgrees M interpretation context environment ρ) :
    ∀ atom : Atom,
      wellSortedAtom context atom = true →
      QfreeAgreementFor M interpretation (.atom atom) →
      evalGroundAtom M interpretation (groundAtom environment atom) =
        evalAtom M ρ atom := by
  intro atom sorted qfreeAgrees
  cases atom with
  | qfree id expression =>
      exact qfreeAgrees id expression QfreeOccurs.here
  | rel relation left right =>
      simp only [wellSortedAtom, Bool.and_eq_true] at sorted
      cases relation <;>
        simp only [groundAtom, evalGroundAtom, evalAtom,
          evalGroundTerm_groundTm M interpretation context environment ρ
            environmentAgrees left sorted.1.2,
          evalGroundTerm_groundTm M interpretation context environment ρ
            environmentAgrees right sorted.2]

theorem eval_groundMatrix (M : Model)
    (interpretation : GroundInterpretation M)
    (context : List Sort₂) (environment : List GroundTerm)
    (ρ : Valuation M)
    (environmentAgrees :
      GroundEnvironmentAgrees M interpretation context environment ρ) :
    ∀ (formula : Frm) (groundFormula : GroundFrm),
      wellSortedFrm context formula = true →
      groundMatrix environment formula = some groundFormula →
      QfreeAgreementFor M interpretation formula →
      evalGroundFrm M interpretation groundFormula =
        evalFrm M formula ρ := by
  intro formula
  induction formula with
  | atom atom =>
      intro groundFormula sorted grounded qfreeAgrees
      simp only [groundMatrix, Option.some.injEq] at grounded
      subst grounded
      exact evalGroundAtom_groundAtom M interpretation context environment ρ
        environmentAgrees atom sorted qfreeAgrees
  | neg formula ih =>
      intro groundFormula sorted grounded qfreeAgrees
      cases value : groundMatrix environment formula with
      | none => simp [groundMatrix, value] at grounded
      | some result =>
          simp [groundMatrix, value] at grounded
          subst groundFormula
          simp only [evalGroundFrm, evalFrm]
          rw [ih result sorted value (fun id expression occurs =>
            qfreeAgrees id expression (.neg occurs))]
  | conj left right leftIH rightIH =>
      intro groundFormula sorted grounded qfreeAgrees
      simp only [wellSortedFrm, Bool.and_eq_true] at sorted
      cases leftValue : groundMatrix environment left with
      | none => simp [groundMatrix, leftValue] at grounded
      | some leftGround =>
          cases rightValue : groundMatrix environment right with
          | none => simp [groundMatrix, leftValue, rightValue] at grounded
          | some rightGround =>
              simp [groundMatrix, leftValue, rightValue] at grounded
              subst groundFormula
              simp only [evalGroundFrm, evalFrm]
              rw [leftIH leftGround sorted.1 leftValue
                    (fun id expression occurs =>
                      qfreeAgrees id expression (.conjLeft occurs)),
                rightIH rightGround sorted.2 rightValue
                    (fun id expression occurs =>
                      qfreeAgrees id expression (.conjRight occurs))]
  | disj left right leftIH rightIH =>
      intro groundFormula sorted grounded qfreeAgrees
      simp only [wellSortedFrm, Bool.and_eq_true] at sorted
      cases leftValue : groundMatrix environment left with
      | none => simp [groundMatrix, leftValue] at grounded
      | some leftGround =>
          cases rightValue : groundMatrix environment right with
          | none => simp [groundMatrix, leftValue, rightValue] at grounded
          | some rightGround =>
              simp [groundMatrix, leftValue, rightValue] at grounded
              subst groundFormula
              simp only [evalGroundFrm, evalFrm]
              rw [leftIH leftGround sorted.1 leftValue
                    (fun id expression occurs =>
                      qfreeAgrees id expression (.disjLeft occurs)),
                rightIH rightGround sorted.2 rightValue
                    (fun id expression occurs =>
                      qfreeAgrees id expression (.disjRight occurs))]
  | imp left right leftIH rightIH =>
      intro groundFormula sorted grounded qfreeAgrees
      simp only [wellSortedFrm, Bool.and_eq_true] at sorted
      cases leftValue : groundMatrix environment left with
      | none => simp [groundMatrix, leftValue] at grounded
      | some leftGround =>
          cases rightValue : groundMatrix environment right with
          | none => simp [groundMatrix, leftValue, rightValue] at grounded
          | some rightGround =>
              simp [groundMatrix, leftValue, rightValue] at grounded
              subst groundFormula
              simp only [GroundFrm.implies, evalGroundFrm, evalFrm]
              rw [leftIH leftGround sorted.1 leftValue
                    (fun id expression occurs =>
                      qfreeAgrees id expression (.impLeft occurs)),
                rightIH rightGround sorted.2 rightValue
                    (fun id expression occurs =>
                      qfreeAgrees id expression (.impRight occurs))]
  | all sort body ih =>
      intro groundFormula sorted grounded qfreeAgrees
      simp [groundMatrix] at grounded
  | ex sort body ih =>
      intro groundFormula sorted grounded qfreeAgrees
      simp [groundMatrix] at grounded

/-- A winning finite Skolem strategy makes every checked ground instance true.
    Universal instances range over ground terms, whose denotations are ordinary
    model values; existential instances use the strategy lookup fixed above. -/
theorem eval_instantiatePrefix_of_wins (M : Model)
    (interpretation : GroundInterpretation M) (ground : GroundUniverse)
    {binders : Prefix} (tree : SkolemTree M binders)
    (matrix : Frm) :
    ∀ (context : List Sort₂) (environment universalsRev : List GroundTerm)
      (nextSkolem : Nat) (ρ : Valuation M)
      (universalValues : List (Value M)) (groundFormula : GroundFrm),
      wellSortedFrm (binders.toContext ++ context) matrix = true →
      tree.wins M matrix ρ = true →
      GroundEnvironmentAgrees M interpretation context environment ρ →
      QfreeAgreementFor M interpretation matrix →
      SkolemAgreement M tree interpretation universalValues nextSkolem →
      evalGroundArguments M interpretation
          (GroundArguments.ofList universalsRev.reverse) =
        universalValues →
      instantiatePrefix ground binders matrix environment universalsRev
          nextSkolem = some groundFormula →
      evalGroundFrm M interpretation groundFormula = true := by
  induction tree with
  | done =>
      intro context environment universalsRev nextSkolem ρ universalValues
        groundFormula sorted wins environmentAgrees qfreeAgrees
        skolemAgrees argumentsAgree instantiated
      simp only [Prefix.toContext, List.map_nil, List.reverse_nil,
        List.nil_append] at sorted
      simp only [instantiatePrefix] at instantiated
      rw [eval_groundMatrix M interpretation context environment ρ
        environmentAgrees matrix groundFormula sorted instantiated qfreeAgrees]
      exact wins
  | @all sort rest next ih =>
      intro context environment universalsRev nextSkolem ρ universalValues
        groundFormula sorted wins environmentAgrees qfreeAgrees
        skolemAgrees argumentsAgree instantiated
      have restSorted :
          wellSortedFrm (rest.toContext ++ sort :: context) matrix = true := by
        simpa [Prefix.toContext, List.append_assoc] using sorted
      have everyWins :
          ∀ value ∈ M.enum sort,
            (next value).wins M matrix
              (Valuation.cons M sort value ρ) = true :=
        List.all_eq_true.mp wins
      have mapTrue :
          ∀ (terms : List GroundTerm),
            (∀ term, term ∈ terms → term ∈ termsOf ground sort) →
            ∀ formulas : List GroundFrm,
              terms.mapM (fun term =>
                instantiatePrefix ground rest matrix
                  (term :: environment) (term :: universalsRev)
                  nextSkolem) = some formulas →
              formulas.all (evalGroundFrm M interpretation) = true := by
        intro terms contained
        induction terms with
        | nil =>
            intro formulas mapped
            simp at mapped
            subst formulas
            rfl
        | cons term terms tailIH =>
            intro formulas mapped
            have termMember := contained term (by simp)
            have termSort :
                term.sortOf = sort :=
              of_decide_eq_true (List.mem_filter.mp termMember).2
            cases evaluated :
                evalGroundTerm M interpretation term with
            | mk actual value =>
                have actualSort : actual = sort := by
                  calc
                    actual =
                        (evalGroundTerm M interpretation term).fst := by
                          rw [evaluated]
                    _ = term.sortOf :=
                      evalGroundTerm_sortOf M interpretation term
                    _ = sort := termSort
                subst actual
                have branchWins :=
                  everyWins value (M.enum_complete sort value)
                have branchSkolem :
                    SkolemAgreement M (next value) interpretation
                      (universalValues ++ [⟨sort, value⟩]) nextSkolem :=
                  skolemAgrees value
                have branchEnvironment :
                    GroundEnvironmentAgrees M interpretation
                      (sort :: context) (term :: environment)
                      (Valuation.cons M sort value ρ) :=
                  environmentAgrees.cons M interpretation evaluated
                have branchArguments :
                    evalGroundArguments M interpretation
                        (GroundArguments.ofList
                          (term :: universalsRev).reverse) =
                      universalValues ++ [⟨sort, value⟩] := by
                  rw [evalGroundArguments_ofList]
                  simp [List.reverse_cons, List.map_append,
                    evalGroundArguments_ofList] at argumentsAgree ⊢
                  rw [argumentsAgree, evaluated]
                  exact ⟨rfl, rfl⟩
                cases headValue :
                    instantiatePrefix ground rest matrix
                      (term :: environment) (term :: universalsRev)
                      nextSkolem with
                | none =>
                    simp [headValue] at mapped
                | some headFormula =>
                    cases tailValue :
                        terms.mapM (fun candidate =>
                          instantiatePrefix ground rest matrix
                            (candidate :: environment)
                            (candidate :: universalsRev) nextSkolem) with
                    | none =>
                        simp [headValue, tailValue] at mapped
                    | some tailFormulas =>
                        simp [headValue, tailValue] at mapped
                        subst formulas
                        simp only [List.all_cons, Bool.and_eq_true]
                        constructor
                        · exact ih value (sort :: context)
                            (term :: environment) (term :: universalsRev)
                            nextSkolem
                            (Valuation.cons M sort value ρ)
                            (universalValues ++ [⟨sort, value⟩])
                            headFormula restSorted branchWins
                            branchEnvironment qfreeAgrees branchSkolem
                            branchArguments headValue
                        · apply tailIH
                          · intro candidate member
                            exact contained candidate (by simp [member])
                          · exact tailValue
      simp only [instantiatePrefix] at instantiated
      cases mapped :
          (termsOf ground sort).mapM (fun term =>
            instantiatePrefix ground rest matrix
              (term :: environment) (term :: universalsRev)
              nextSkolem) with
      | none =>
          simp [mapped] at instantiated
      | some formulas =>
          simp [mapped] at instantiated
          subst groundFormula
          rw [evalGroundFrm_conjoin]
          exact mapTrue (termsOf ground sort) (by simp) formulas mapped
  | @ex sort rest value member next ih =>
      intro context environment universalsRev nextSkolem ρ universalValues
        groundFormula sorted wins environmentAgrees qfreeAgrees
        skolemAgrees argumentsAgree instantiated
      have restSorted :
          wellSortedFrm (rest.toContext ++ sort :: context) matrix = true := by
        simpa [Prefix.toContext, List.append_assoc] using sorted
      change
        interpretation.skolem nextSkolem universalValues sort = value
          ∧ SkolemAgreement M next interpretation universalValues
            (nextSkolem + 1) at skolemAgrees
      let witness := GroundTerm.appList
        (skolemFunction nextSkolem universalsRev sort)
        universalsRev.reverse
      have witnessEvaluates :
          evalGroundTerm M interpretation witness = ⟨sort, value⟩ := by
        have argumentsAgree' := argumentsAgree
        rw [evalGroundArguments_ofList] at argumentsAgree'
        simp only [witness, GroundTerm.appList, evalGroundTerm,
          evalGroundArguments_ofList, skolemFunction, evalGroundFunction]
        rw [argumentsAgree', skolemAgrees.1]
      exact ih (sort :: context) (witness :: environment) universalsRev
        (nextSkolem + 1) (Valuation.cons M sort value ρ)
        universalValues groundFormula restSorted wins
        (environmentAgrees.cons M interpretation witnessEvaluates)
        qfreeAgrees skolemAgrees.2 argumentsAgree instantiated

def GroundFrm.toBoolExpr : GroundFrm → BoolExpr GroundAtom
  | .const value => .const value
  | .atom groundAtom => .literal groundAtom
  | .neg formula => .not formula.toBoolExpr
  | .conj left right => .gate .and left.toBoolExpr right.toBoolExpr
  | .disj left right => .gate .or left.toBoolExpr right.toBoolExpr

def GroundFrm.atomsRaw : GroundFrm → List GroundAtom
  | .const _ => []
  | .atom groundAtom => [groundAtom]
  | .neg formula => formula.atomsRaw
  | .conj left right | .disj left right =>
      left.atomsRaw ++ right.atomsRaw

def GroundFrm.atoms (formula : GroundFrm) : List GroundAtom :=
  formula.atomsRaw.eraseDups

def GroundFrm.encodeWith (atoms : List GroundAtom) :
    GroundFrm → BoolExpr Nat
  | .const value => .const value
  | .atom groundAtom => .literal (atoms.idxOf groundAtom)
  | .neg formula => .not (formula.encodeWith atoms)
  | .conj left right =>
      .gate .and (left.encodeWith atoms) (right.encodeWith atoms)
  | .disj left right =>
      .gate .or (left.encodeWith atoms) (right.encodeWith atoms)

/-- Production LRAT uses compact natural-number variables. The atom table is
    recomputed from the exact ground formula and first-occurrence order. -/
def GroundFrm.toBoolExprNat (formula : GroundFrm) : BoolExpr Nat :=
  formula.encodeWith formula.atoms

def evalIndexedAtom (M : Model) (interpretation : GroundInterpretation M)
    (formula : GroundFrm) (index : Nat) : Bool :=
  match formula.atoms[index]? with
  | some atom => evalGroundAtom M interpretation atom
  | none => false

theorem eval_encodeWith (M : Model)
    (interpretation : GroundInterpretation M) (atoms : List GroundAtom) :
    ∀ formula : GroundFrm,
      (∀ atom, atom ∈ formula.atomsRaw → atom ∈ atoms) →
      BoolExpr.eval
        (fun index =>
          match atoms[index]? with
          | some atom => evalGroundAtom M interpretation atom
          | none => false)
        (formula.encodeWith atoms) =
        evalGroundFrm M interpretation formula
  | .const _, _ => rfl
  | .atom atom, contains => by
      have member : atom ∈ atoms := contains atom (by simp [GroundFrm.atomsRaw])
      simp [GroundFrm.encodeWith, List.getElem?_idxOf member,
        evalGroundFrm]
  | .neg formula, contains => by
      simp only [GroundFrm.encodeWith, BoolExpr.eval, evalGroundFrm]
      rw [eval_encodeWith M interpretation atoms formula]
      intro atom member
      exact contains atom (by simpa [GroundFrm.atomsRaw] using member)
  | .conj left right, contains => by
      simp only [GroundFrm.encodeWith, BoolExpr.eval, Gate.eval, evalGroundFrm]
      rw [eval_encodeWith M interpretation atoms left,
        eval_encodeWith M interpretation atoms right]
      · intro atom member
        exact contains atom (by simp [GroundFrm.atomsRaw, member])
      · intro atom member
        exact contains atom (by simp [GroundFrm.atomsRaw, member])
  | .disj left right, contains => by
      simp only [GroundFrm.encodeWith, BoolExpr.eval, Gate.eval, evalGroundFrm]
      rw [eval_encodeWith M interpretation atoms left,
        eval_encodeWith M interpretation atoms right]
      · intro atom member
        exact contains atom (by simp [GroundFrm.atomsRaw, member])
      · intro atom member
        exact contains atom (by simp [GroundFrm.atomsRaw, member])

theorem eval_toBoolExprNat (M : Model)
    (interpretation : GroundInterpretation M) (formula : GroundFrm) :
    BoolExpr.eval (evalIndexedAtom M interpretation formula)
        formula.toBoolExprNat =
      evalGroundFrm M interpretation formula := by
  apply eval_encodeWith
  intro atom member
  simp [GroundFrm.atoms, member]

theorem eval_toBoolExpr (M : Model)
    (interpretation : GroundInterpretation M) (formula : GroundFrm) :
    BoolExpr.eval (evalGroundAtom M interpretation) formula.toBoolExpr =
      evalGroundFrm M interpretation formula := by
  induction formula with
  | const value => rfl
  | atom atom => rfl
  | neg formula ih => simp only [GroundFrm.toBoolExpr, BoolExpr.eval, evalGroundFrm, ih]
  | conj left right leftIH rightIH =>
      simp only [GroundFrm.toBoolExpr, BoolExpr.eval, Gate.eval,
        evalGroundFrm, leftIH, rightIH]
  | disj left right leftIH rightIH =>
      simp only [GroundFrm.toBoolExpr, BoolExpr.eval, Gate.eval,
        evalGroundFrm, leftIH, rightIH]

structure GroundReplayCertificate where
  instantiation : InstantiationCertificate
  lrat : Array LRAT.IntAction

def verifyGroundReplay (source : Frm)
    (certificate : GroundReplayCertificate) : Bool :=
  verifyInstantiation source certificate.instantiation
    && Thermite.PropReconstruct.verifyActions
      certificate.instantiation.formula.toBoolExprNat certificate.lrat

/-- The first full checked boundary: Lean recomputes the exact finite
    instantiation and Tseitin CNF, then an LRAT proof establishes that the
    resulting ground theory problem has no interpretation. -/
theorem ground_unsat_of_verifyGroundReplay
    {source : Frm} {certificate : GroundReplayCertificate}
    (verified : verifyGroundReplay source certificate = true)
    (M : Model) (interpretation : GroundInterpretation M) :
    evalGroundFrm M interpretation certificate.instantiation.formula = false := by
  simp only [verifyGroundReplay, Bool.and_eq_true] at verified
  have unsat :=
    Thermite.PropReconstruct.unsat_of_verifyActions
      certificate.instantiation.formula.toBoolExprNat certificate.lrat verified.2
      (evalIndexedAtom M interpretation certificate.instantiation.formula)
  rw [eval_toBoolExprNat] at unsat
  exact unsat

theorem source_false_of_verifiedInstantiation
    {source : Frm} {certificate : InstantiationCertificate}
    (verified : verifyInstantiation source certificate = true)
    (groundFalse :
      ∀ (M : Model) (interpretation : GroundInterpretation M),
        evalGroundFrm M interpretation certificate.formula = false)
    (M : Model) (ρ : Valuation M) :
    evalFrm M source ρ = false := by
  cases sourceValue : evalFrm M source ρ with
  | false => rfl
  | true =>
      obtain ⟨tree, wins⟩ :=
        (skolemization_equisatisfiable M source ρ).mp sourceValue
      let normalized := normalizedPrefix source
      let interpretation :=
        GroundInterpretation.ofTree M normalized.2 tree
      have sorted :
          wellSortedFrm normalized.1.toContext normalized.2 = true := by
        exact normalizedWellSorted_of_verifyInstantiation verified
      have unique : normalized.2.qfreeIds.Nodup := by
        exact qfreeIds_nodup_of_verifyInstantiation verified
      have qfreeAgrees :
          QfreeAgreementFor M interpretation normalized.2 := by
        exact qfreeAgreementFor_value M normalized.2 (tree.lookup M) unique
      have skolemAgrees :
          SkolemAgreement M tree interpretation [] 0 := by
        exact GroundInterpretation.ofTree_skolemAgreement M normalized.2 tree
      have instantiated :
          instantiatePrefix certificate.grounding.ground normalized.1
              normalized.2 [] [] 0 =
            some certificate.formula := by
        simpa [instantiate, normalized] using
          instantiation_eq_of_verify verified
      have groundTrue :
          evalGroundFrm M interpretation certificate.formula = true := by
        exact eval_instantiatePrefix_of_wins M interpretation
          certificate.grounding.ground tree normalized.2
          [] [] [] 0 ρ [] certificate.formula
          (by simpa using sorted) wins
          (GroundEnvironmentAgrees.nil M interpretation ρ)
          qfreeAgrees skolemAgrees rfl instantiated
      have groundFalse' := groundFalse M interpretation
      exact Bool.noConfusion (groundTrue.symm.trans groundFalse')

#print axioms eval_toBoolExpr
#print axioms eval_toBoolExprNat
#print axioms ground_unsat_of_verifyGroundReplay
#print axioms source_false_of_verifiedInstantiation

end Thermite.Strat.Cls
