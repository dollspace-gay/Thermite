/-
  Dependency-preserving finite instantiation.

  Prenex form is useful for normalization proofs, but a flat prefix makes every
  existential appear to depend on every earlier universal. Quantifiers pulled
  from independent Boolean branches do not have those dependencies. This
  module instantiates NNF in place, so each Skolem symbol receives exactly the
  universal variables in its lexical scope.
-/
import Thermite.Strat.GroundReconstruct
import Thermite.Strat.Fragment

namespace Thermite.Strat.Cls

open Classical

def Frm.existentialCount : Frm → Nat
  | .atom _ => 0
  | .neg formula => formula.existentialCount
  | .conj left right | .disj left right | .imp left right =>
      left.existentialCount + right.existentialCount
  | .all _ body => body.existentialCount
  | .ex _ body => body.existentialCount + 1

def Frm.isNnf : Frm → Bool
  | .atom _ => true
  | .neg (.atom _) => true
  | .neg _ => false
  | .conj left right | .disj left right =>
      left.isNnf && right.isNnf
  | .imp _ _ => false
  | .all _ body | .ex _ body => body.isNnf

mutual
  @[simp]
  theorem isNnf_nnf : ∀ formula : Frm, (nnf formula).isNnf = true
    | .atom _ => rfl
    | .neg formula => isNnf_nnfNeg formula
    | .conj left right | .disj left right =>
        by simp [nnf, Frm.isNnf, isNnf_nnf left, isNnf_nnf right]
    | .imp left right =>
        by simp [nnf, Frm.isNnf, isNnf_nnfNeg left, isNnf_nnf right]
    | .all _ body | .ex _ body =>
        by simp [nnf, Frm.isNnf, isNnf_nnf body]

  @[simp]
  theorem isNnf_nnfNeg : ∀ formula : Frm,
      (nnfNeg formula).isNnf = true
    | .atom _ => rfl
    | .neg formula => isNnf_nnf formula
    | .conj left right | .disj left right =>
        by simp [nnfNeg, Frm.isNnf,
          isNnf_nnfNeg left, isNnf_nnfNeg right]
    | .imp left right =>
        by simp [nnfNeg, Frm.isNnf,
          isNnf_nnf left, isNnf_nnfNeg right]
    | .all _ body | .ex _ body =>
        by simp [nnfNeg, Frm.isNnf, isNnf_nnfNeg body]
end

def scopedSkolemFunction (id : Nat)
    (universals : List Sort₂) (result : Sort₂) : GroundFunction :=
  { kind := .skolem id, arguments := universals, result }

/-- Skolem symbols with lexical, rather than prenex-artifact, dependencies. -/
def scopedSkolemFunctions : Frm → List Sort₂ → Nat → List GroundFunction
  | .atom _, _, _ => []
  | .neg formula, universals, next =>
      scopedSkolemFunctions formula universals next
  | .conj left right, universals, next
  | .disj left right, universals, next
  | .imp left right, universals, next =>
      scopedSkolemFunctions left universals next ++
        scopedSkolemFunctions right universals
          (next + left.existentialCount)
  | .all sort body, universals, next =>
      scopedSkolemFunctions body (universals ++ [sort]) next
  | .ex sort body, universals, next =>
      scopedSkolemFunction next universals sort ::
        scopedSkolemFunctions body universals (next + 1)

def structuralSignature (source : Frm) : List GroundFunction :=
  let normalized := nnf source
  (closureFunctionsFrm normalized ++
    scopedSkolemFunctions normalized [] 0).eraseDups

def structuralSeeds (source : Frm) : GroundUniverse :=
  reconstructionSeeds (nnf source)

/-- Instantiate NNF without moving its quantifiers. `universals` is ordered
    outermost first, while `environment` remains the ordinary de Bruijn stack. -/
def instantiateNnf (ground : GroundUniverse) :
    Frm → List GroundTerm → List GroundTerm → Nat → Option GroundFrm
  | .atom atom, environment, _, _ =>
      some (.atom (groundAtom environment atom))
  | .neg (.atom atom), environment, _, _ =>
      some (.neg (.atom (groundAtom environment atom)))
  | .neg _, _, _, _ => none
  | .conj left right, environment, universals, next => do
      let leftGround ← instantiateNnf ground left environment universals next
      let rightGround ← instantiateNnf ground right environment universals
        (next + left.existentialCount)
      pure (.conj leftGround rightGround)
  | .disj left right, environment, universals, next => do
      let leftGround ← instantiateNnf ground left environment universals next
      let rightGround ← instantiateNnf ground right environment universals
        (next + left.existentialCount)
      pure (.disj leftGround rightGround)
  | .imp _ _, _, _, _ => none
  | .all sort body, environment, universals, next =>
      ((termsOf ground sort).mapM fun term =>
          instantiateNnf ground body (term :: environment)
            (universals ++ [term]) next)
        |>.map GroundFrm.conjoin
  | .ex sort body, environment, universals, next =>
      let witness := GroundTerm.appList
        (scopedSkolemFunction next
          (universals.map GroundTerm.sortOf) sort)
        universals
      instantiateNnf ground body (witness :: environment)
        universals (next + 1)

def structuralInstantiate (ground : GroundUniverse)
    (source : Frm) : Option GroundFrm :=
  instantiateNnf ground (nnf source) [] [] 0

theorem instantiateNnf_exists_of_isNnf (ground : GroundUniverse) :
    ∀ (formula : Frm) (environment universals : List GroundTerm)
      (next : Nat),
      formula.isNnf = true →
      ∃ result,
        instantiateNnf ground formula environment universals next =
          some result := by
  intro formula
  induction formula with
  | atom atom =>
      intro environment universals next _
      exact ⟨.atom (groundAtom environment atom), rfl⟩
  | neg formula ih =>
      intro environment universals next nnf
      cases formula with
      | atom atom =>
          exact ⟨.neg (.atom (groundAtom environment atom)), rfl⟩
      | neg body | conj body _ | disj body _ | imp body _
      | all _ body | ex _ body =>
          simp [Frm.isNnf] at nnf
  | conj left right leftIH rightIH =>
      intro environment universals next nnf
      simp only [Frm.isNnf, Bool.and_eq_true] at nnf
      obtain ⟨leftResult, leftResultEq⟩ :=
        leftIH environment universals next nnf.1
      obtain ⟨rightResult, rightResultEq⟩ :=
        rightIH environment universals
          (next + left.existentialCount) nnf.2
      exact
        ⟨.conj leftResult rightResult,
          by simp [instantiateNnf, leftResultEq, rightResultEq]⟩
  | disj left right leftIH rightIH =>
      intro environment universals next nnf
      simp only [Frm.isNnf, Bool.and_eq_true] at nnf
      obtain ⟨leftResult, leftResultEq⟩ :=
        leftIH environment universals next nnf.1
      obtain ⟨rightResult, rightResultEq⟩ :=
        rightIH environment universals
          (next + left.existentialCount) nnf.2
      exact
        ⟨.disj leftResult rightResult,
          by simp [instantiateNnf, leftResultEq, rightResultEq]⟩
  | imp left right leftIH rightIH =>
      intro environment universals next nnf
      simp [Frm.isNnf] at nnf
  | all sort body ih =>
      intro environment universals next nnf
      simp only [Frm.isNnf] at nnf
      have mapExists :
          ∀ terms : List GroundTerm,
            ∃ results,
              (terms.mapM fun term =>
                  instantiateNnf ground body (term :: environment)
                    (universals ++ [term]) next) =
                some results := by
        intro terms
        induction terms with
        | nil => exact ⟨[], rfl⟩
        | cons term rest restIH =>
            obtain ⟨result, resultEq⟩ :=
              ih (term :: environment) (universals ++ [term]) next nnf
            obtain ⟨results, resultsEq⟩ := restIH
            exact
              ⟨result :: results,
                by simp [resultEq, resultsEq]⟩
      obtain ⟨results, resultsEq⟩ :=
        mapExists (termsOf ground sort)
      exact
        ⟨GroundFrm.conjoin results,
          by simp [instantiateNnf, resultsEq]⟩
  | ex sort body ih =>
      intro environment universals next nnf
      simp only [Frm.isNnf] at nnf
      exact ih
        (GroundTerm.appList
          (scopedSkolemFunction next
            (universals.map GroundTerm.sortOf) sort)
          universals :: environment)
        universals (next + 1) nnf

theorem structuralInstantiate_exists (ground : GroundUniverse)
    (source : Frm) :
    ∃ result, structuralInstantiate ground source = some result := by
  apply instantiateNnf_exists_of_isNnf
  exact isNnf_nnf source

def sequenceDiffWitnesses (formula : GroundFrm) : GroundUniverse :=
  formula.atomsRaw.filterMap fun atom =>
    match atom with
    | .rel relation left right =>
        if relation = .eq ∨ relation = .ne then
          match left.sortOf, right.sortOf with
          | .seq leftElem, .seq rightElem =>
              if leftElem = rightElem then
                if left = right then none
                else
                  some <| GroundTerm.appList
                    {
                      kind := .seqDiff leftElem
                      arguments := [.seq leftElem, .seq leftElem]
                      result := usizeS
                    }
                    [left, right]
              else none
          | _, _ => none
        else none
    | _ => none

/-- Internal extensionality witnesses are targeted seeds, not a function
    closure. Closing `seqDiff` over every pair of sequence terms adds unrelated
    index instances and can turn one array equality into a quadratic ground
    problem. -/
def structuralReconstructionSeeds (source : Frm) : GroundUniverse :=
  let signature := structuralSignature source
  let seeds := structuralSeeds source
  let sorts := signatureSorts signature seeds
  let order := (topologicalOrder? signature seeds).getD []
  let preliminaryGround :=
    buildUniverse signature order (withInhabitants sorts seeds)
  let preliminaryFormula :=
    (structuralInstantiate preliminaryGround source).getD (.const false)
  (seeds ++ sequenceDiffWitnesses preliminaryFormula).eraseDups

def buildStructuralInstantiation (source : Frm) :
    InstantiationCertificate :=
  let signature := structuralSignature source
  let seeds := structuralReconstructionSeeds source
  let sorts := signatureSorts signature seeds
  let order := (topologicalOrder? signature seeds).getD []
  let ground :=
    buildUniverse signature order (withInhabitants sorts seeds)
  let formula :=
    (structuralInstantiate ground source).getD (.const false)
  { grounding := { order, ground }, formula }

@[simp]
theorem structuralInstantiate_buildStructuralInstantiation
    (source : Frm) :
    structuralInstantiate
        (buildStructuralInstantiation source).grounding.ground source =
      some (buildStructuralInstantiation source).formula := by
  obtain ⟨result, instantiated⟩ :=
    structuralInstantiate_exists
      (buildStructuralInstantiation source).grounding.ground source
  change
    structuralInstantiate
        (buildStructuralInstantiation source).grounding.ground source =
      some
        ((structuralInstantiate
          (buildStructuralInstantiation source).grounding.ground source).getD
            (.const false))
  rw [instantiated]
  rfl

def verifyStructuralInstantiation (source : Frm)
    (certificate : InstantiationCertificate) : Bool :=
  let normalized := nnf source
  wellSortedFrm [] source
    && normalized.isNnf
    && decide normalized.qfreeIds.Nodup
    && admitted source
    && verifyGrounding (structuralSignature source)
      (structuralReconstructionSeeds source) certificate.grounding
    && decide
      (structuralInstantiate certificate.grounding.ground source =
        some certificate.formula)

/-- The facts needed by the soundness theorem itself. Full grounding
    completeness is checked separately by `verifyStructuralInstantiation`; a
    refutation of any exactly bound finite instance set is already sound for
    the quantified source. Keeping this replay check small avoids normalizing
    the cubic completeness audit inside every generated theorem. -/
def verifyStructuralBinding (source : Frm)
    (certificate : InstantiationCertificate) : Bool :=
  wellSortedFrm [] source
    && decide (nnf source).qfreeIds.Nodup
    && decide
      (structuralInstantiate certificate.grounding.ground source =
        some certificate.formula)

theorem verifyStructuralBinding_buildStructuralInstantiation
    (source : Frm)
    (wellSorted : wellSortedFrm [] source = true)
    (qfreeNodup : (nnf source).qfreeIds.Nodup) :
    verifyStructuralBinding source
        (buildStructuralInstantiation source) = true := by
  simp [verifyStructuralBinding, wellSorted, qfreeNodup,
    structuralInstantiate_buildStructuralInstantiation]

/-- A satisfying strategy for NNF. Conjunction retains both strategies;
    disjunction records the successful branch; universal nodes branch on every
    typed value; existential nodes retain one enumerated witness. -/
inductive NnfStrategy (M : Model) where
  | leaf
  | both (left right : NnfStrategy M)
  | chooseLeft (left : NnfStrategy M)
  | chooseRight (right : NnfStrategy M)
  | all (next : Value M → NnfStrategy M)
  | ex (value : Value M)
      (member :
        match value with
        | ⟨sort, element⟩ => element ∈ M.enum sort)
      (next : NnfStrategy M)

def NnfStrategy.wins (M : Model) :
    NnfStrategy M → Frm → Valuation M → Bool
  | .leaf, .atom atom, ρ => evalAtom M ρ atom
  | .leaf, .neg (.atom atom), ρ => !evalAtom M ρ atom
  | .both leftTree rightTree, .conj left right, ρ =>
      leftTree.wins M left ρ && rightTree.wins M right ρ
  | .chooseLeft leftTree, .disj left _, ρ =>
      leftTree.wins M left ρ
  | .chooseRight rightTree, .disj _ right, ρ =>
      rightTree.wins M right ρ
  | .all next, .all sort body, ρ =>
      (M.enum sort).all fun value =>
        (next ⟨sort, value⟩).wins M body
          (Valuation.cons M sort value ρ)
  | .ex ⟨actual, value⟩ _ next, .ex sort body, ρ =>
      if same : actual = sort then
        next.wins M body
          (Valuation.cons M sort (same ▸ value) ρ)
      else
        false
  | _, _, _ => false

theorem evalFrm_iff_nnfStrategy (M : Model) :
    ∀ (formula : Frm), formula.isNnf = true →
      ∀ ρ : Valuation M,
        evalFrm M formula ρ = true ↔
          ∃ strategy : NnfStrategy M,
            strategy.wins M formula ρ = true
  | .atom atom, _, ρ => by
      constructor
      · intro holds
        exact ⟨.leaf, holds⟩
      · rintro ⟨strategy, holds⟩
        cases strategy <;> simp_all [NnfStrategy.wins, evalFrm]
  | .neg (.atom atom), _, ρ => by
      constructor
      · intro holds
        exact ⟨.leaf, holds⟩
      · rintro ⟨strategy, holds⟩
        cases strategy <;> simp_all [NnfStrategy.wins, evalFrm]
  | .neg (.neg formula), nnf, ρ => by
      simp [Frm.isNnf] at nnf
  | .neg (.conj left right), nnf, ρ => by
      simp [Frm.isNnf] at nnf
  | .neg (.disj left right), nnf, ρ => by
      simp [Frm.isNnf] at nnf
  | .neg (.imp left right), nnf, ρ => by
      simp [Frm.isNnf] at nnf
  | .neg (.all sort body), nnf, ρ => by
      simp [Frm.isNnf] at nnf
  | .neg (.ex sort body), nnf, ρ => by
      simp [Frm.isNnf] at nnf
  | .conj left right, nnf, ρ => by
      simp only [Frm.isNnf, Bool.and_eq_true] at nnf
      rw [evalFrm, Bool.and_eq_true,
        evalFrm_iff_nnfStrategy M left nnf.1 ρ,
        evalFrm_iff_nnfStrategy M right nnf.2 ρ]
      constructor
      · rintro ⟨⟨leftTree, leftWins⟩, ⟨rightTree, rightWins⟩⟩
        exact ⟨.both leftTree rightTree,
          by simp [NnfStrategy.wins, leftWins, rightWins]⟩
      · rintro ⟨strategy, wins⟩
        cases strategy with
        | both leftTree rightTree =>
            simp only [NnfStrategy.wins, Bool.and_eq_true] at wins
            exact ⟨⟨leftTree, wins.1⟩, ⟨rightTree, wins.2⟩⟩
        | leaf | chooseLeft _ | chooseRight _ | all _ | ex _ _ _ =>
            simp [NnfStrategy.wins] at wins
  | .disj left right, nnf, ρ => by
      simp only [Frm.isNnf, Bool.and_eq_true] at nnf
      rw [evalFrm, Bool.or_eq_true,
        evalFrm_iff_nnfStrategy M left nnf.1 ρ,
        evalFrm_iff_nnfStrategy M right nnf.2 ρ]
      constructor
      · intro holds
        rcases holds with leftHolds | rightHolds
        · rcases leftHolds with ⟨leftTree, leftWins⟩
          exact ⟨.chooseLeft leftTree, leftWins⟩
        · rcases rightHolds with ⟨rightTree, rightWins⟩
          exact ⟨.chooseRight rightTree, rightWins⟩
      · rintro ⟨strategy, wins⟩
        cases strategy with
        | chooseLeft leftTree => exact Or.inl ⟨leftTree, wins⟩
        | chooseRight rightTree => exact Or.inr ⟨rightTree, wins⟩
        | leaf | both _ _ | all _ | ex _ _ _ =>
            simp [NnfStrategy.wins] at wins
  | .imp left right, nnf, ρ => by
      simp [Frm.isNnf] at nnf
  | .all sort body, nnf, ρ => by
      simp only [Frm.isNnf] at nnf
      rw [evalFrm, List.all_eq_true]
      constructor
      · intro holds
        let next : Value M → NnfStrategy M :=
          fun tagged =>
            match tagged with
            | ⟨actual, value⟩ =>
                if same : actual = sort then
                  Classical.choose <|
                    (evalFrm_iff_nnfStrategy M body nnf
                      (Valuation.cons M sort (same ▸ value) ρ)).mp
                      (holds (same ▸ value)
                        (M.enum_complete sort (same ▸ value)))
                else
                  .leaf
        refine ⟨.all next, ?_⟩
        simp only [NnfStrategy.wins, List.all_eq_true]
        intro value member
        simpa [next] using
          Classical.choose_spec <|
            (evalFrm_iff_nnfStrategy M body nnf
              (Valuation.cons M sort value ρ)).mp
              (holds value member)
      · rintro ⟨strategy, wins⟩
        cases strategy with
        | all next =>
            intro value member
            apply (evalFrm_iff_nnfStrategy M body nnf
              (Valuation.cons M sort value ρ)).mpr
            exact ⟨next ⟨sort, value⟩,
              List.all_eq_true.mp wins value member⟩
        | leaf | both _ _ | chooseLeft _ | chooseRight _ | ex _ _ _ =>
            simp [NnfStrategy.wins] at wins
  | .ex sort body, nnf, ρ => by
      simp only [Frm.isNnf] at nnf
      rw [evalFrm, List.any_eq_true]
      constructor
      · rintro ⟨value, member, bodyTrue⟩
        rcases (evalFrm_iff_nnfStrategy M body nnf
          (Valuation.cons M sort value ρ)).mp bodyTrue with
          ⟨next, nextWins⟩
        exact ⟨.ex ⟨sort, value⟩ member next,
          by simpa [NnfStrategy.wins] using nextWins⟩
      · rintro ⟨strategy, wins⟩
        cases strategy with
        | @ex tagged member next =>
            rcases tagged with ⟨actual, value⟩
            simp only [NnfStrategy.wins] at wins
            split at wins
            · rename_i same
              cases same
              exact ⟨value, member,
                (evalFrm_iff_nnfStrategy M body nnf
                  (Valuation.cons M _ value ρ)).mpr
                  ⟨next, wins⟩⟩
            · simp at wins
        | leaf | both _ _ | chooseLeft _ | chooseRight _ | all _ =>
            simp [NnfStrategy.wins] at wins

/-- Resolve a Skolem occurrence relative to the current formula subtree.
    `arguments` contains values for universal nodes still below this subtree;
    universal choices already traversed are represented by the selected
    strategy node itself. -/
def NnfStrategy.lookup (M : Model) :
    NnfStrategy M → Frm → Nat → List (Value M) →
      (result : Sort₂) → M.Carrier result
  | .both leftTree rightTree, .conj left right, id, arguments, result =>
      if id < left.existentialCount then
        leftTree.lookup M left id arguments result
      else
        rightTree.lookup M right
          (id - left.existentialCount) arguments result
  | .chooseLeft leftTree, .disj left _, id, arguments, result =>
      if id < left.existentialCount then
        leftTree.lookup M left id arguments result
      else
        M.default result
  | .chooseRight rightTree, .disj left right, id, arguments, result =>
      if id < left.existentialCount then
        M.default result
      else
        rightTree.lookup M right
          (id - left.existentialCount) arguments result
  | .all next, .all sort body, id, arguments, result =>
      match arguments with
      | [] => M.default result
      | ⟨actual, value⟩ :: rest =>
          if same : actual = sort then
            (next ⟨sort, same ▸ value⟩).lookup M body id rest result
          else
            M.default result
  | .ex ⟨actual, value⟩ _ next, .ex sort body, id, arguments, result =>
      match id with
      | 0 =>
          if sameSort : actual = sort then
            if sameResult : sort = result then
              sameResult ▸ (sameSort ▸ value)
            else
              M.default result
          else
            M.default result
      | id + 1 => next.lookup M body id arguments result
  | _, _, _, _, result => M.default result

def GroundInterpretation.ofNnfStrategy (M : Model)
    (formula : Frm) (strategy : NnfStrategy M) :
    GroundInterpretation M :=
  { qfree := qfreeValue M formula
    skolem := strategy.lookup M formula }

/-- The external interpretation agrees with a selected strategy on exactly the
    Skolem IDs owned by the current subtree. -/
def NnfSkolemAgreement (M : Model)
    (interpretation : GroundInterpretation M)
    (strategy : NnfStrategy M) (formula : Frm)
    (universalValues : List (Value M)) (nextSkolem : Nat) : Prop :=
  ∀ id, id < formula.existentialCount →
    ∀ rest result,
      interpretation.skolem (nextSkolem + id)
          (universalValues ++ rest) result =
        strategy.lookup M formula id rest result

theorem GroundInterpretation.ofNnfStrategy_agrees
    (M : Model) (formula : Frm) (strategy : NnfStrategy M) :
    NnfSkolemAgreement M
      (GroundInterpretation.ofNnfStrategy M formula strategy)
      strategy formula [] 0 := by
  intro id inRange rest result
  simp [GroundInterpretation.ofNnfStrategy]

mutual
  theorem wellSorted_nnf (context : List Sort₂) :
      ∀ formula : Frm,
        wellSortedFrm context formula = true →
          wellSortedFrm context (nnf formula) = true
    | .atom _, sorted => sorted
    | .neg formula, sorted =>
        wellSorted_nnfNeg context formula
          (by simpa [wellSortedFrm] using sorted)
    | .conj left right, sorted | .disj left right, sorted =>
        by
          simp only [nnf, wellSortedFrm, Bool.and_eq_true] at sorted ⊢
          exact ⟨wellSorted_nnf context left sorted.1,
            wellSorted_nnf context right sorted.2⟩
    | .imp left right, sorted =>
        by
          simp only [nnf, wellSortedFrm, Bool.and_eq_true] at sorted ⊢
          exact ⟨wellSorted_nnfNeg context left sorted.1,
            wellSorted_nnf context right sorted.2⟩
    | .all sort body, sorted | .ex sort body, sorted =>
        by
          simpa only [nnf, wellSortedFrm] using
            wellSorted_nnf (sort :: context) body
              (by simpa only [wellSortedFrm] using sorted)

  theorem wellSorted_nnfNeg (context : List Sort₂) :
      ∀ formula : Frm,
        wellSortedFrm context formula = true →
          wellSortedFrm context (nnfNeg formula) = true
    | .atom _, sorted => by simpa [nnfNeg, wellSortedFrm] using sorted
    | .neg formula, sorted =>
        wellSorted_nnf context formula
          (by simpa [wellSortedFrm] using sorted)
    | .conj left right, sorted | .disj left right, sorted =>
        by
          simp only [nnfNeg, wellSortedFrm, Bool.and_eq_true] at sorted ⊢
          exact ⟨wellSorted_nnfNeg context left sorted.1,
            wellSorted_nnfNeg context right sorted.2⟩
    | .imp left right, sorted =>
        by
          simp only [nnfNeg, wellSortedFrm, Bool.and_eq_true] at sorted ⊢
          exact ⟨wellSorted_nnf context left sorted.1,
            wellSorted_nnfNeg context right sorted.2⟩
    | .all sort body, sorted | .ex sort body, sorted =>
        by
          simpa only [nnfNeg, wellSortedFrm] using
            wellSorted_nnfNeg (sort :: context) body
              (by simpa only [wellSortedFrm] using sorted)
end

theorem eval_instantiateNnf_of_wins (M : Model)
    (interpretation : GroundInterpretation M)
    (ground : GroundUniverse) :
    ∀ (formula : Frm) (strategy : NnfStrategy M)
      (context : List Sort₂) (environment universals : List GroundTerm)
      (universalValues : List (Value M)) (nextSkolem : Nat)
      (ρ : Valuation M) (groundFormula : GroundFrm),
      formula.isNnf = true →
      wellSortedFrm context formula = true →
      strategy.wins M formula ρ = true →
      GroundEnvironmentAgrees M interpretation context environment ρ →
      QfreeAgreementFor M interpretation formula →
      NnfSkolemAgreement M interpretation strategy formula
        universalValues nextSkolem →
      evalGroundArguments M interpretation
          (GroundArguments.ofList universals) = universalValues →
      instantiateNnf ground formula environment universals nextSkolem =
          some groundFormula →
      evalGroundFrm M interpretation groundFormula = true := by
  intro formula
  induction formula with
  | atom atom =>
      intro strategy context environment universals universalValues
        nextSkolem ρ groundFormula nnf sorted wins environmentAgrees
        qfreeAgrees skolemAgrees argumentsAgree instantiated
      cases strategy with
      | leaf =>
          simp only [instantiateNnf, Option.some.injEq] at instantiated
          subst groundFormula
          simp only [evalGroundFrm]
          rw [evalGroundAtom_groundAtom M interpretation context
            environment ρ environmentAgrees atom sorted qfreeAgrees]
          exact wins
      | both _ _ | chooseLeft _ | chooseRight _ | all _ | ex _ _ _ =>
          simp [NnfStrategy.wins] at wins
  | neg formula ih =>
      intro strategy context environment universals universalValues
        nextSkolem ρ groundFormula nnf sorted wins environmentAgrees
        qfreeAgrees skolemAgrees argumentsAgree instantiated
      cases formula with
      | atom atom =>
          cases strategy with
          | leaf =>
              simp only [instantiateNnf, Option.some.injEq] at instantiated
              subst groundFormula
              simp only [evalGroundFrm]
              rw [evalGroundAtom_groundAtom M interpretation context
                environment ρ environmentAgrees atom
                (by simpa [wellSortedFrm] using sorted)]
              · exact wins
              · intro id expression occurs
                exact qfreeAgrees id expression (.neg occurs)
          | both _ _ | chooseLeft _ | chooseRight _ | all _ | ex _ _ _ =>
              simp [NnfStrategy.wins] at wins
      | neg body | conj body _ | disj body _ | imp body _
        | all _ body | ex _ body =>
          simp [Frm.isNnf] at nnf
  | conj left right leftIH rightIH =>
      intro strategy context environment universals universalValues
        nextSkolem ρ groundFormula nnf sorted wins environmentAgrees
        qfreeAgrees skolemAgrees argumentsAgree instantiated
      simp only [Frm.isNnf, Bool.and_eq_true] at nnf
      simp only [wellSortedFrm, Bool.and_eq_true] at sorted
      cases strategy with
      | both leftTree rightTree =>
          simp only [NnfStrategy.wins, Bool.and_eq_true] at wins
          simp only [instantiateNnf] at instantiated
          cases leftValue :
              instantiateNnf ground left environment universals nextSkolem with
          | none => simp [leftValue] at instantiated
          | some leftGround =>
              cases rightValue :
                  instantiateNnf ground right environment universals
                    (nextSkolem + left.existentialCount) with
              | none => simp [leftValue, rightValue] at instantiated
              | some rightGround =>
                  simp [leftValue, rightValue] at instantiated
                  subst groundFormula
                  simp only [evalGroundFrm, Bool.and_eq_true]
                  constructor
                  · apply leftIH leftTree context environment universals
                      universalValues nextSkolem ρ leftGround
                      nnf.1 sorted.1 wins.1 environmentAgrees
                    · intro id expression occurs
                      exact qfreeAgrees id expression (.conjLeft occurs)
                    · intro id inRange rest result
                      have parent := skolemAgrees id
                        (Nat.lt_add_right _ inRange) rest result
                      simpa [NnfStrategy.lookup, inRange] using parent
                    · exact argumentsAgree
                    · exact leftValue
                  · apply rightIH rightTree context environment universals
                      universalValues
                      (nextSkolem + left.existentialCount) ρ rightGround
                      nnf.2 sorted.2 wins.2 environmentAgrees
                    · intro id expression occurs
                      exact qfreeAgrees id expression (.conjRight occurs)
                    · intro id inRange rest result
                      have inParent :
                          left.existentialCount + id <
                            (Frm.conj left right).existentialCount := by
                        simp only [Frm.existentialCount]
                        omega
                      have notLeft :
                          ¬ left.existentialCount + id <
                            left.existentialCount := by omega
                      have notLeft' :
                          ¬ id + left.existentialCount <
                            left.existentialCount := by omega
                      have parent := skolemAgrees
                        (left.existentialCount + id)
                        inParent rest result
                      simpa [NnfStrategy.lookup, notLeft, notLeft',
                        Nat.add_assoc, Nat.add_comm, Nat.add_left_comm]
                        using parent
                    · exact argumentsAgree
                    · exact rightValue
      | leaf | chooseLeft _ | chooseRight _ | all _ | ex _ _ _ =>
          simp [NnfStrategy.wins] at wins
  | disj left right leftIH rightIH =>
      intro strategy context environment universals universalValues
        nextSkolem ρ groundFormula nnf sorted wins environmentAgrees
        qfreeAgrees skolemAgrees argumentsAgree instantiated
      simp only [Frm.isNnf, Bool.and_eq_true] at nnf
      simp only [wellSortedFrm, Bool.and_eq_true] at sorted
      simp only [instantiateNnf] at instantiated
      cases leftValue :
          instantiateNnf ground left environment universals nextSkolem with
      | none => simp [leftValue] at instantiated
      | some leftGround =>
          cases rightValue :
              instantiateNnf ground right environment universals
                (nextSkolem + left.existentialCount) with
          | none => simp [leftValue, rightValue] at instantiated
          | some rightGround =>
              simp [leftValue, rightValue] at instantiated
              subst groundFormula
              cases strategy with
              | chooseLeft leftTree =>
                  simp only [evalGroundFrm, Bool.or_eq_true]
                  left
                  apply leftIH leftTree context environment universals
                    universalValues nextSkolem ρ leftGround
                    nnf.1 sorted.1 wins environmentAgrees
                  · intro id expression occurs
                    exact qfreeAgrees id expression (.disjLeft occurs)
                  · intro id inRange rest result
                    have parent := skolemAgrees id
                      (Nat.lt_add_right _ inRange) rest result
                    simpa [NnfStrategy.lookup, inRange] using parent
                  · exact argumentsAgree
                  · exact leftValue
              | chooseRight rightTree =>
                  simp only [evalGroundFrm, Bool.or_eq_true]
                  right
                  apply rightIH rightTree context environment universals
                    universalValues
                    (nextSkolem + left.existentialCount) ρ rightGround
                    nnf.2 sorted.2 wins environmentAgrees
                  · intro id expression occurs
                    exact qfreeAgrees id expression (.disjRight occurs)
                  · intro id inRange rest result
                    have inParent :
                        left.existentialCount + id <
                          (Frm.disj left right).existentialCount := by
                      simp only [Frm.existentialCount]
                      omega
                    have notLeft :
                        ¬ left.existentialCount + id <
                          left.existentialCount := by omega
                    have notLeft' :
                        ¬ id + left.existentialCount <
                          left.existentialCount := by omega
                    have parent := skolemAgrees
                      (left.existentialCount + id)
                      inParent rest result
                    simpa [NnfStrategy.lookup, notLeft, notLeft',
                      Nat.add_assoc, Nat.add_comm, Nat.add_left_comm]
                      using parent
                  · exact argumentsAgree
                  · exact rightValue
              | leaf | both _ _ | all _ | ex _ _ _ =>
                  simp [NnfStrategy.wins] at wins
  | imp left right leftIH rightIH =>
      intro strategy context environment universals universalValues
        nextSkolem ρ groundFormula nnf
      simp [Frm.isNnf] at nnf
  | all sort body ih =>
      intro strategy context environment universals universalValues
        nextSkolem ρ groundFormula nnf sorted wins environmentAgrees
        qfreeAgrees skolemAgrees argumentsAgree instantiated
      simp only [Frm.isNnf] at nnf
      simp only [wellSortedFrm] at sorted
      cases strategy with
      | all next =>
          simp only [NnfStrategy.wins] at wins
          simp only [instantiateNnf] at instantiated
          have mapTrue :
              ∀ (terms : List GroundTerm),
                (∀ term, term ∈ terms → term ∈ termsOf ground sort) →
                ∀ formulas : List GroundFrm,
                  terms.mapM (fun term =>
                    instantiateNnf ground body (term :: environment)
                      (universals ++ [term]) nextSkolem) =
                      some formulas →
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
                    cases headValue :
                        instantiateNnf ground body (term :: environment)
                          (universals ++ [term]) nextSkolem with
                    | none => simp [headValue] at mapped
                    | some headFormula =>
                        cases tailValue :
                            terms.mapM (fun candidate =>
                              instantiateNnf ground body
                                (candidate :: environment)
                                (universals ++ [candidate])
                                nextSkolem) with
                        | none => simp [headValue, tailValue] at mapped
                        | some tailFormulas =>
                            simp [headValue, tailValue] at mapped
                            subst formulas
                            simp only [List.all_cons, Bool.and_eq_true]
                            constructor
                            · apply ih (next ⟨sort, value⟩)
                                (sort :: context) (term :: environment)
                                (universals ++ [term])
                                (universalValues ++ [⟨sort, value⟩])
                                nextSkolem
                                (Valuation.cons M sort value ρ)
                                headFormula nnf sorted
                              · exact List.all_eq_true.mp wins value
                                  (M.enum_complete sort value)
                              · exact environmentAgrees.cons M interpretation
                                  evaluated
                              · intro id expression occurs
                                exact qfreeAgrees id expression (.all occurs)
                              · intro id inRange rest result
                                have parent := skolemAgrees id inRange
                                  (⟨sort, value⟩ :: rest) result
                                simpa [NnfStrategy.lookup,
                                  List.append_assoc] using parent
                              · rw [evalGroundArguments_ofList]
                                  at argumentsAgree ⊢
                                simp [List.map_append, argumentsAgree,
                                  evaluated]
                              · exact headValue
                            · apply tailIH
                              · intro candidate member
                                exact contained candidate (by simp [member])
                              · exact tailValue
          cases mapped :
              (termsOf ground sort).mapM fun term =>
                instantiateNnf ground body (term :: environment)
                  (universals ++ [term]) nextSkolem with
          | none => simp [mapped] at instantiated
          | some formulas =>
              simp [mapped] at instantiated
              subst groundFormula
              rw [evalGroundFrm_conjoin]
              exact mapTrue (termsOf ground sort) (by simp)
                formulas mapped
      | leaf | both _ _ | chooseLeft _ | chooseRight _ | ex _ _ _ =>
          simp [NnfStrategy.wins] at wins
  | ex sort body ih =>
      intro strategy context environment universals universalValues
        nextSkolem ρ groundFormula nnf sorted wins environmentAgrees
        qfreeAgrees skolemAgrees argumentsAgree instantiated
      simp only [Frm.isNnf] at nnf
      simp only [wellSortedFrm] at sorted
      cases strategy with
      | @ex tagged member next =>
          rcases tagged with ⟨actual, value⟩
          simp only [NnfStrategy.wins] at wins
          split at wins
          · rename_i same
            cases same
            let witness := GroundTerm.appList
              (scopedSkolemFunction nextSkolem
                (universals.map GroundTerm.sortOf) sort)
              universals
            have witnessEvaluates :
                evalGroundTerm M interpretation witness =
                  ⟨sort, value⟩ := by
              have current := skolemAgrees 0 (by
                simp [Frm.existentialCount]) [] sort
              have currentValue :
                  interpretation.skolem nextSkolem universalValues sort =
                    value := by
                simpa [NnfStrategy.lookup] using current
              rw [evalGroundArguments_ofList] at argumentsAgree
              simp only [witness, GroundTerm.appList, evalGroundTerm,
                evalGroundArguments_ofList, scopedSkolemFunction,
                evalGroundFunction]
              rw [argumentsAgree, currentValue]
            apply ih next (sort :: context) (witness :: environment)
              universals universalValues (nextSkolem + 1)
              (Valuation.cons M sort value ρ) groundFormula
              nnf sorted wins
            · exact environmentAgrees.cons M interpretation witnessEvaluates
            · intro id expression occurs
              exact qfreeAgrees id expression (.ex occurs)
            · intro id inRange rest result
              have parent := skolemAgrees (id + 1)
                (by simp [Frm.existentialCount]; omega) rest result
              simpa [NnfStrategy.lookup, Nat.add_assoc, Nat.add_comm,
                Nat.add_left_comm] using parent
            · exact argumentsAgree
            · simpa [instantiateNnf, witness] using instantiated
          · simp at wins
      | leaf | both _ _ | chooseLeft _ | chooseRight _ | all _ =>
          simp [NnfStrategy.wins] at wins

theorem structuralInstantiation_eq_of_verify
    {source : Frm} {certificate : InstantiationCertificate}
    (verified :
      verifyStructuralInstantiation source certificate = true) :
    structuralInstantiate certificate.grounding.ground source =
      some certificate.formula := by
  simp only [verifyStructuralInstantiation, Bool.and_eq_true] at verified
  exact of_decide_eq_true verified.2

theorem structuralInstantiation_eq_of_binding
    {source : Frm} {certificate : InstantiationCertificate}
    (verified : verifyStructuralBinding source certificate = true) :
    structuralInstantiate certificate.grounding.ground source =
      some certificate.formula := by
  simp only [verifyStructuralBinding, Bool.and_eq_true] at verified
  exact of_decide_eq_true verified.2

theorem structuralWellSorted_of_verify
    {source : Frm} {certificate : InstantiationCertificate}
    (verified :
      verifyStructuralInstantiation source certificate = true) :
    wellSortedFrm [] source = true := by
  simp only [verifyStructuralInstantiation, Bool.and_eq_true] at verified
  exact verified.1.1.1.1.1

theorem structuralWellSorted_of_binding
    {source : Frm} {certificate : InstantiationCertificate}
    (verified : verifyStructuralBinding source certificate = true) :
    wellSortedFrm [] source = true := by
  simp only [verifyStructuralBinding, Bool.and_eq_true] at verified
  exact verified.1.1

theorem structuralQfreeIds_nodup_of_verify
    {source : Frm} {certificate : InstantiationCertificate}
    (verified :
      verifyStructuralInstantiation source certificate = true) :
    (nnf source).qfreeIds.Nodup := by
  simp only [verifyStructuralInstantiation, Bool.and_eq_true] at verified
  exact of_decide_eq_true verified.1.1.1.2

theorem structuralQfreeIds_nodup_of_binding
    {source : Frm} {certificate : InstantiationCertificate}
    (verified : verifyStructuralBinding source certificate = true) :
    (nnf source).qfreeIds.Nodup := by
  simp only [verifyStructuralBinding, Bool.and_eq_true] at verified
  exact of_decide_eq_true verified.1.2

theorem source_false_of_verifiedStructuralBinding
    {source : Frm} {certificate : InstantiationCertificate}
    (verified : verifyStructuralBinding source certificate = true)
    (groundFalse :
      ∀ (M : Model) (interpretation : GroundInterpretation M),
        evalGroundFrm M interpretation certificate.formula = false)
    (M : Model) (ρ : Valuation M) :
    evalFrm M source ρ = false := by
  cases sourceValue : evalFrm M source ρ with
  | false => rfl
  | true =>
      have normalizedTrue : evalFrm M (nnf source) ρ = true := by
        rw [eval_nnf M source ρ]
        exact sourceValue
      obtain ⟨strategy, wins⟩ :=
        (evalFrm_iff_nnfStrategy M (nnf source)
          (isNnf_nnf source) ρ).mp normalizedTrue
      let interpretation :=
        GroundInterpretation.ofNnfStrategy M (nnf source) strategy
      have groundTrue :
          evalGroundFrm M interpretation certificate.formula = true := by
        apply eval_instantiateNnf_of_wins M interpretation
          certificate.grounding.ground
          (nnf source) strategy [] [] [] [] 0 ρ certificate.formula
          (isNnf_nnf source)
          (wellSorted_nnf [] source
            (structuralWellSorted_of_binding verified))
          wins
        · exact GroundEnvironmentAgrees.nil M interpretation ρ
        · exact qfreeAgreementFor_value M (nnf source)
            (strategy.lookup M (nnf source))
            (structuralQfreeIds_nodup_of_binding verified)
        · exact GroundInterpretation.ofNnfStrategy_agrees
            M (nnf source) strategy
        · rfl
        · exact structuralInstantiation_eq_of_binding verified
      rw [groundFalse M interpretation] at groundTrue
      simp at groundTrue

/-- Dependency-preserving grounding is sound for the original source formula.
    The interpretation used for a hypothetical satisfying source is built from
    its lexical NNF strategy, so independent Boolean branches never acquire
    fake Skolem dependencies. -/
theorem source_false_of_verifiedStructuralInstantiation
    {source : Frm} {certificate : InstantiationCertificate}
    (verified :
      verifyStructuralInstantiation source certificate = true)
    (groundFalse :
      ∀ (M : Model) (interpretation : GroundInterpretation M),
        evalGroundFrm M interpretation certificate.formula = false)
    (M : Model) (ρ : Valuation M) :
    evalFrm M source ρ = false := by
  cases sourceValue : evalFrm M source ρ with
  | false => rfl
  | true =>
      have normalizedTrue : evalFrm M (nnf source) ρ = true := by
        rw [eval_nnf M source ρ]
        exact sourceValue
      obtain ⟨strategy, wins⟩ :=
        (evalFrm_iff_nnfStrategy M (nnf source)
          (isNnf_nnf source) ρ).mp normalizedTrue
      let interpretation :=
        GroundInterpretation.ofNnfStrategy M (nnf source) strategy
      have groundTrue :
          evalGroundFrm M interpretation certificate.formula = true := by
        apply eval_instantiateNnf_of_wins M interpretation
          certificate.grounding.ground
          (nnf source) strategy [] [] [] [] 0 ρ certificate.formula
          (isNnf_nnf source)
          (wellSorted_nnf [] source
            (structuralWellSorted_of_verify verified))
          wins
        · exact GroundEnvironmentAgrees.nil M interpretation ρ
        · exact qfreeAgreementFor_value M (nnf source)
            (strategy.lookup M (nnf source))
            (structuralQfreeIds_nodup_of_verify verified)
        · exact GroundInterpretation.ofNnfStrategy_agrees
            M (nnf source) strategy
        · rfl
        · exact structuralInstantiation_eq_of_verify verified
      rw [groundFalse M interpretation] at groundTrue
      simp at groundTrue

#print axioms source_false_of_verifiedStructuralInstantiation
#print axioms source_false_of_verifiedStructuralBinding

end Thermite.Strat.Cls
