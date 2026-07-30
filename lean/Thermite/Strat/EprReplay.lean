/-
  Combined EPR ground/theory/LRAT replay.

  A successful check binds the finite instantiation to the normalized source,
  accepts only kernel-sound theory steps, recomputes the full propositional
  formula and Tseitin CNF, and checks its LRAT refutation.
-/
import Thermite.Strat.GroundTheory
import Thermite.Strat.StructuralInstantiation

namespace Thermite.Strat.Cls

open Std.Tactic.BVDecide

structure EprReplayCertificate where
  instantiation : InstantiationCertificate
  theory : List GroundTheoryStep
  lrat : Array Std.Tactic.BVDecide.LRAT.IntAction

/-- A small declaration boundary for generated reconstruction theorems. The
    sole field is the ordinary model-semantic implication. A structure keeps
    elaboration from normalizing a large concrete formula before replay starts,
    while `.semantic` exposes the exact proposition to consumers. -/
structure EprClaim (premise conclusion : Frm) : Prop where
  semantic :
    ∀ (M : Model) (ρ : Valuation M),
      evalFrm M premise ρ = true → evalFrm M conclusion ρ = true

theorem EprClaim.ofSemantic {premise conclusion : Frm}
    (proof :
      ∀ (M : Model) (ρ : Valuation M),
        evalFrm M premise ρ = true →
          evalFrm M conclusion ρ = true) :
    EprClaim premise conclusion :=
  ⟨proof⟩

/-- The exact term set visible to theory replay: the checked Herbrand closure
    plus all subterms in the recomputed finite instantiation. The latter matters
    for fixed-depth operations such as offsets, which are not globally closed
    as function symbols because doing so would create an infinite same-sort
    term tower. -/
def eprGround (certificate : EprReplayCertificate) : GroundUniverse :=
  certificate.instantiation.formula.terms.eraseDups

/-- Build the deterministic grounding and exhaustive equality/congruence
    closure. The LRAT field is filled by the external proof-producing SAT step;
    every other field is independently recomputed by `verifyEprReplay`. -/
def buildEprSkeleton (source : Frm) : EprReplayCertificate :=
  let instantiation := buildStructuralInstantiation source
  let relevantTerms := instantiation.formula.terms.eraseDups
  let theory :=
    checkedRelevantTheory relevantTerms instantiation.formula
  { instantiation, theory, lrat := #[] }

@[simp]
theorem verifyTheory_buildEprSkeleton (source : Frm) :
    verifyTheory (eprGround (buildEprSkeleton source))
      (buildEprSkeleton source).theory = true := by
  simp [buildEprSkeleton, eprGround]

theorem verifyStructuralBinding_buildEprSkeleton
    (source : Frm)
    (wellSorted : wellSortedFrm [] source = true)
    (qfreeNodup : (nnf source).qfreeIds.Nodup) :
    verifyStructuralBinding source
        (buildEprSkeleton source).instantiation = true := by
  simpa only [buildEprSkeleton] using
    verifyStructuralBinding_buildStructuralInstantiation
      source wellSorted qfreeNodup

def eprProblem (certificate : EprReplayCertificate) : GroundFrm :=
  .conj certificate.instantiation.formula
    (theoryFormula certificate.theory)

def GroundAtom.eprBase : GroundAtom → GroundAtom
  | .rel .ne left right => .rel .eq left right
  | atom => atom

def eprTheoryGroundClauses
    (certificate : EprReplayCertificate) : List GroundCnfClause :=
  certificate.theory.flatMap GroundTheoryStep.cnfClauses

def eprTheoryAtoms
    (certificate : EprReplayCertificate) : List GroundAtom :=
  (eprTheoryGroundClauses certificate).flatMap fun clause =>
    clause.map Prod.fst

def eprRawAtoms (certificate : EprReplayCertificate) : List GroundAtom :=
  certificate.instantiation.formula.atomsRaw ++
    eprTheoryAtoms certificate

/-- The source and theory clauses share one atom table. Disequality is stored
    as the corresponding equality variable and encoded with reversed
    polarity. -/
def eprAtoms (certificate : EprReplayCertificate) : List GroundAtom :=
  let raw := eprRawAtoms certificate
  (raw ++ raw.map GroundAtom.eprBase).eraseDups

def GroundAtom.eprEncodeWith (atoms : List GroundAtom) :
    GroundAtom → BoolExpr Nat
  | .rel .ne left right =>
      .not (.literal (atoms.idxOf (.rel .eq left right)))
  | atom => .literal (atoms.idxOf atom.eprBase)

def GroundFrm.eprEncodeWith (atoms : List GroundAtom) :
    GroundFrm → BoolExpr Nat
  | .const value => .const value
  | .atom groundAtom => groundAtom.eprEncodeWith atoms
  | .neg formula => .not (formula.eprEncodeWith atoms)
  | .conj left right =>
      .gate .and (left.eprEncodeWith atoms)
        (right.eprEncodeWith atoms)
  | .disj left right =>
      .gate .or (left.eprEncodeWith atoms)
        (right.eprEncodeWith atoms)

/-- Only the grounded source gets Tseitin gates. Theory facts are appended as
    direct clauses below. -/
def eprFormula (certificate : EprReplayCertificate) : BoolExpr Nat :=
  certificate.instantiation.formula.eprEncodeWith
    (eprAtoms certificate)

def GroundCnfLiteral.toCnfLiteral (atoms : List GroundAtom)
    (literal : GroundCnfLiteral) : Std.Sat.Literal Nat :=
  match literal.1 with
  | .rel .ne left right =>
      (atoms.idxOf (.rel .eq left right), !literal.2)
  | atom => (atoms.idxOf atom.eprBase, literal.2)

def GroundCnfClause.toCnfClause (atoms : List GroundAtom)
    (clause : GroundCnfClause) : Std.Sat.CNF.Clause Nat :=
  clause.map (GroundCnfLiteral.toCnfLiteral atoms)

def eprTheoryClauses (certificate : EprReplayCertificate) :
    List (Std.Sat.CNF.Clause Nat) :=
  (eprTheoryGroundClauses certificate).map
    (GroundCnfClause.toCnfClause (eprAtoms certificate))

/-- Production EPR CNF. Only the grounded source formula receives Tseitin
    gates; checked equality and relation implications stay as flat Horn
    clauses. -/
def eprCnf (certificate : EprReplayCertificate) : Std.Sat.CNF Nat :=
  Thermite.PropReconstruct.tseitinCnfWith
    (eprFormula certificate) (eprTheoryClauses certificate)

def verifyEprActions (certificate : EprReplayCertificate) : Bool :=
  LRAT.check certificate.lrat (eprCnf certificate)

def evalGroundAtomAt (M : Model)
    (interpretation : GroundInterpretation M)
    (atoms : List GroundAtom) (index : Nat) : Bool :=
  match atoms[index]? with
  | some atom => evalGroundAtom M interpretation atom
  | none => false

@[simp]
theorem evalGroundAtomAt_idxOf (M : Model)
    (interpretation : GroundInterpretation M)
    (atoms : List GroundAtom) (atom : GroundAtom)
    (member : atom ∈ atoms) :
    evalGroundAtomAt M interpretation atoms (atoms.idxOf atom) =
      evalGroundAtom M interpretation atom := by
  simp [evalGroundAtomAt, List.getElem?_idxOf member]

theorem eval_groundCnfLiteral_toCnfLiteral (M : Model)
    (interpretation : GroundInterpretation M)
    (atoms : List GroundAtom) (literal : GroundCnfLiteral)
    (member : literal.1.eprBase ∈ atoms) :
    let encoded := literal.toCnfLiteral atoms
    (evalGroundAtomAt M interpretation atoms encoded.1 ==
        encoded.2) =
      evalGroundCnfLiteral M interpretation literal := by
  rcases literal with ⟨atom, polarity⟩
  cases atom with
  | qfree id =>
      simp only [GroundCnfLiteral.toCnfLiteral,
        GroundAtom.eprBase] at member ⊢
      rw [evalGroundAtomAt_idxOf M interpretation atoms
        (.qfree id) member]
      rfl
  | rel relation left right =>
      cases relation <;> cases polarity <;>
        simp only [GroundCnfLiteral.toCnfLiteral,
          GroundAtom.eprBase] at member ⊢ <;>
        rw [evalGroundAtomAt_idxOf M interpretation atoms _ member] <;>
        simp [evalGroundCnfLiteral, evalGroundAtom]

theorem eval_groundCnfClause_toCnfClause (M : Model)
    (interpretation : GroundInterpretation M)
    (atoms : List GroundAtom) (clause : GroundCnfClause)
    (contained :
      ∀ literal ∈ clause, literal.1.eprBase ∈ atoms) :
    Std.Sat.CNF.Clause.eval
        (evalGroundAtomAt M interpretation atoms)
        (clause.toCnfClause atoms) =
      evalGroundCnfClause M interpretation clause := by
  induction clause with
  | nil => rfl
  | cons literal rest ih =>
      simp only [GroundCnfClause.toCnfClause, List.map_cons,
        Std.Sat.CNF.Clause.eval_cons, evalGroundCnfClause,
        List.any_cons]
      rw [eval_groundCnfLiteral_toCnfLiteral M interpretation atoms
        literal (contained literal List.mem_cons_self)]
      have restEvaluated :=
        ih (fun item member =>
          contained item (List.mem_cons_of_mem literal member))
      simp only [GroundCnfClause.toCnfClause,
        evalGroundCnfClause] at restEvaluated
      rw [restEvaluated]

theorem eval_eprAtomWith (M : Model)
    (interpretation : GroundInterpretation M)
    (atoms : List GroundAtom) (atom : GroundAtom)
    (member : atom.eprBase ∈ atoms) :
    BoolExpr.eval (evalGroundAtomAt M interpretation atoms)
        (atom.eprEncodeWith atoms) =
      evalGroundAtom M interpretation atom := by
  cases atom with
  | qfree id =>
      simp only [GroundAtom.eprBase] at member
      simp only [GroundAtom.eprEncodeWith, GroundAtom.eprBase,
        BoolExpr.eval]
      rw [evalGroundAtomAt_idxOf M interpretation atoms
        (.qfree id) member]
  | rel relation left right =>
      cases relation <;>
        simp only [GroundAtom.eprBase] at member <;>
        simp only [GroundAtom.eprEncodeWith, GroundAtom.eprBase,
          BoolExpr.eval, evalGroundAtom] <;>
        rw [evalGroundAtomAt_idxOf M interpretation atoms _ member] <;>
        rfl

theorem eval_eprEncodeWith (M : Model)
    (interpretation : GroundInterpretation M)
    (atoms : List GroundAtom) :
    ∀ formula : GroundFrm,
      (∀ atom, atom ∈ formula.atomsRaw →
        atom.eprBase ∈ atoms) →
      BoolExpr.eval (evalGroundAtomAt M interpretation atoms)
          (formula.eprEncodeWith atoms) =
        evalGroundFrm M interpretation formula
  | .const _, _ => rfl
  | .atom groundAtom, contains => by
      apply eval_eprAtomWith M interpretation atoms
      exact contains groundAtom
        (by simp [GroundFrm.atomsRaw])
  | .neg formula, contains => by
      simp only [GroundFrm.eprEncodeWith, BoolExpr.eval,
        evalGroundFrm]
      rw [eval_eprEncodeWith M interpretation atoms formula]
      intro atom member
      exact contains atom
        (by simpa [GroundFrm.atomsRaw] using member)
  | .conj left right, contains => by
      simp only [GroundFrm.eprEncodeWith, BoolExpr.eval, Gate.eval,
        evalGroundFrm]
      rw [eval_eprEncodeWith M interpretation atoms left,
        eval_eprEncodeWith M interpretation atoms right]
      · intro atom member
        exact contains atom (by simp [GroundFrm.atomsRaw, member])
      · intro atom member
        exact contains atom (by simp [GroundFrm.atomsRaw, member])
  | .disj left right, contains => by
      simp only [GroundFrm.eprEncodeWith, BoolExpr.eval, Gate.eval,
        evalGroundFrm]
      rw [eval_eprEncodeWith M interpretation atoms left,
        eval_eprEncodeWith M interpretation atoms right]
      · intro atom member
        exact contains atom (by simp [GroundFrm.atomsRaw, member])
      · intro atom member
        exact contains atom (by simp [GroundFrm.atomsRaw, member])

theorem eprBase_mem_eprAtoms_of_raw
    (certificate : EprReplayCertificate) (atom : GroundAtom)
    (member : atom ∈ eprRawAtoms certificate) :
    atom.eprBase ∈ eprAtoms certificate := by
  simp only [eprAtoms, List.mem_eraseDups, List.mem_append]
  exact Or.inr (List.mem_map.mpr ⟨atom, member, rfl⟩)

theorem eval_eprFormula (M : Model)
    (interpretation : GroundInterpretation M)
    (certificate : EprReplayCertificate) :
    BoolExpr.eval
        (evalGroundAtomAt M interpretation (eprAtoms certificate))
        (eprFormula certificate) =
      evalGroundFrm M interpretation
        certificate.instantiation.formula := by
  apply eval_eprEncodeWith
  intro atom member
  apply eprBase_mem_eprAtoms_of_raw
  exact List.mem_append_left (eprTheoryAtoms certificate) member

theorem eval_eprTheoryClauses (M : Model)
    (interpretation : GroundInterpretation M)
    (certificate : EprReplayCertificate)
    (verified :
      verifyTheory (eprGround certificate)
        certificate.theory = true) :
    (eprTheoryClauses certificate).all
        (Std.Sat.CNF.Clause.eval
          (evalGroundAtomAt M interpretation
            (eprAtoms certificate))) = true := by
  have groundValid :
      (eprTheoryGroundClauses certificate).all
        (evalGroundCnfClause M interpretation) = true := by
    have proveSteps :
        ∀ steps : List GroundTheoryStep,
          steps.all
              (GroundTheoryStep.valid (eprGround certificate)) = true →
          (steps.flatMap GroundTheoryStep.cnfClauses).all
              (evalGroundCnfClause M interpretation) = true := by
      intro steps stepsValid
      induction steps with
      | nil => rfl
      | cons step rest ih =>
          simp only [List.all_cons, Bool.and_eq_true] at stepsValid
          simp only [List.flatMap_cons, List.all_append,
            Bool.and_eq_true]
          exact
            ⟨step.eval_cnfClauses M interpretation
                (eprGround certificate) stepsValid.1,
              ih stepsValid.2⟩
    apply proveSteps certificate.theory
    exact verified
  have clauseContained :
      ∀ clause ∈ eprTheoryGroundClauses certificate,
        ∀ literal ∈ clause,
          literal.1.eprBase ∈ eprAtoms certificate := by
    intro clause clauseMember literal literalMember
    apply eprBase_mem_eprAtoms_of_raw
    apply List.mem_append_right
    simp only [eprTheoryAtoms, List.mem_flatMap]
    exact
      ⟨clause, clauseMember,
        List.mem_map.mpr ⟨literal, literalMember, rfl⟩⟩
  have encodeClauses :
      ∀ clauses : List GroundCnfClause,
        (∀ clause ∈ clauses,
          ∀ literal ∈ clause,
            literal.1.eprBase ∈ eprAtoms certificate) →
        clauses.all (evalGroundCnfClause M interpretation) = true →
        (clauses.map
            (GroundCnfClause.toCnfClause (eprAtoms certificate))).all
          (Std.Sat.CNF.Clause.eval
            (evalGroundAtomAt M interpretation
              (eprAtoms certificate))) = true := by
    intro clauses contained clausesValid
    induction clauses with
    | nil => rfl
    | cons clause rest ih =>
        simp only [List.all_cons, Bool.and_eq_true] at clausesValid
        simp only [List.map_cons, List.all_cons, Bool.and_eq_true]
        constructor
        · rw [eval_groundCnfClause_toCnfClause]
          · exact clausesValid.1
          · exact contained clause List.mem_cons_self
        · apply ih
          · intro item itemMember
            exact contained item
              (List.mem_cons_of_mem clause itemMember)
          · exact clausesValid.2
  exact encodeClauses (eprTheoryGroundClauses certificate)
    clauseContained groundValid

def verifyEprReplay (source : Frm)
    (certificate : EprReplayCertificate) : Bool :=
  verifyInstantiation source certificate.instantiation
    && verifyTheory (eprGround certificate)
      certificate.theory
    && verifyEprActions certificate

/-- Production replay uses dependency-preserving structural instantiation.
    The older prenex verifier remains available for its focused normalization
    pins, but automatic routing calls this boundary. -/
def verifyStructuralEprReplay (source : Frm)
    (certificate : EprReplayCertificate) : Bool :=
  verifyStructuralInstantiation source certificate.instantiation
    && verifyTheory (eprGround certificate) certificate.theory
    && verifyEprActions certificate

/-- Checked theory clauses cannot be the source of the contradiction. -/
theorem ground_false_of_verifyEprReplay
    {source : Frm} {certificate : EprReplayCertificate}
    (verified : verifyEprReplay source certificate = true)
    (M : Model) (interpretation : GroundInterpretation M) :
    evalGroundFrm M interpretation certificate.instantiation.formula = false := by
  simp only [verifyEprReplay, Bool.and_eq_true] at verified
  have cnfUnsat : (eprCnf certificate).Unsat :=
    LRAT.check_sound certificate.lrat (eprCnf certificate)
      verified.2
  cases evaluated :
      evalGroundFrm M interpretation
        certificate.instantiation.formula with
  | false => rfl
  | true =>
      have cnfSat :=
        Thermite.PropReconstruct.tseitinCnfWith_sat_of_eval
          (evalGroundAtomAt M interpretation (eprAtoms certificate))
          (eprFormula certificate) (eprTheoryClauses certificate)
          (by
            rw [eval_eprFormula]
            exact evaluated)
          (eval_eprTheoryClauses M interpretation certificate
            verified.1.2)
      change
        Std.Sat.CNF.eval
            (Thermite.PropReconstruct.tseitinAssignmentWith
              (evalGroundAtomAt M interpretation
                (eprAtoms certificate))
              (eprFormula certificate)
              (eprTheoryClauses certificate))
            (eprCnf certificate) =
          true at cnfSat
      have contradicted :=
        cnfUnsat <|
          Thermite.PropReconstruct.tseitinAssignmentWith
            (evalGroundAtomAt M interpretation
              (eprAtoms certificate))
            (eprFormula certificate) (eprTheoryClauses certificate)
      rw [cnfSat] at contradicted
      exact contradicted

/-- The semantic half of production replay, split from source-instantiation
    checking so generated files can bind the structural certificate and LRAT
    result independently. -/
theorem ground_false_of_epr_actions
    {certificate : EprReplayCertificate}
    (theoryVerified :
      verifyTheory (eprGround certificate)
        certificate.theory = true)
    (actionsVerified : verifyEprActions certificate = true)
    (M : Model) (interpretation : GroundInterpretation M) :
    evalGroundFrm M interpretation
        certificate.instantiation.formula = false := by
  have cnfUnsat : (eprCnf certificate).Unsat :=
    LRAT.check_sound certificate.lrat (eprCnf certificate)
      actionsVerified
  cases evaluated :
      evalGroundFrm M interpretation
        certificate.instantiation.formula with
  | false => rfl
  | true =>
      have cnfSat :=
        Thermite.PropReconstruct.tseitinCnfWith_sat_of_eval
          (evalGroundAtomAt M interpretation (eprAtoms certificate))
          (eprFormula certificate) (eprTheoryClauses certificate)
          (by
            rw [eval_eprFormula]
            exact evaluated)
          (eval_eprTheoryClauses M interpretation certificate
            theoryVerified)
      change
        Std.Sat.CNF.eval
            (Thermite.PropReconstruct.tseitinAssignmentWith
              (evalGroundAtomAt M interpretation
                (eprAtoms certificate))
              (eprFormula certificate)
              (eprTheoryClauses certificate))
            (eprCnf certificate) =
          true at cnfSat
      have contradicted :=
        cnfUnsat <|
          Thermite.PropReconstruct.tseitinAssignmentWith
            (evalGroundAtomAt M interpretation
              (eprAtoms certificate))
            (eprFormula certificate) (eprTheoryClauses certificate)
      rw [cnfSat] at contradicted
      exact contradicted

/-- Variant for a term-producing LRAT replay. This is useful when reducing the
    Boolean checker itself would exceed the symbolic evaluator's budget. -/
theorem ground_false_of_epr_cnf_unsat
    {certificate : EprReplayCertificate}
    (theoryVerified :
      verifyTheory (eprGround certificate)
        certificate.theory = true)
    (cnfUnsat : (eprCnf certificate).Unsat)
    (M : Model) (interpretation : GroundInterpretation M) :
    evalGroundFrm M interpretation
        certificate.instantiation.formula = false := by
  cases evaluated :
      evalGroundFrm M interpretation
        certificate.instantiation.formula with
  | false => rfl
  | true =>
      have cnfSat :=
        Thermite.PropReconstruct.tseitinCnfWith_sat_of_eval
          (evalGroundAtomAt M interpretation (eprAtoms certificate))
          (eprFormula certificate) (eprTheoryClauses certificate)
          (by
            rw [eval_eprFormula]
            exact evaluated)
          (eval_eprTheoryClauses M interpretation certificate
            theoryVerified)
      change
        Std.Sat.CNF.eval
            (Thermite.PropReconstruct.tseitinAssignmentWith
              (evalGroundAtomAt M interpretation
                (eprAtoms certificate))
              (eprFormula certificate)
              (eprTheoryClauses certificate))
            (eprCnf certificate) =
          true at cnfSat
      have contradicted :=
        cnfUnsat <|
          Thermite.PropReconstruct.tseitinAssignmentWith
            (evalGroundAtomAt M interpretation
              (eprAtoms certificate))
            (eprFormula certificate) (eprTheoryClauses certificate)
      rw [cnfSat] at contradicted
      exact contradicted

/-- Proof-producing replay endpoint. Generated files use the term-producing
    LRAT tactic to supply `problemUnsat`; the executable verifier remains the
    routing and tamper-detection gate. -/
theorem ground_false_of_problem_unsat
    {certificate : EprReplayCertificate}
    (theoryVerified :
      verifyTheory (eprGround certificate)
        certificate.theory = true)
    (problemUnsat :
      BoolExpr.Unsat (eprProblem certificate).toBoolExprNat)
    (M : Model) (interpretation : GroundInterpretation M) :
    evalGroundFrm M interpretation
        certificate.instantiation.formula = false := by
  have unsat :=
    problemUnsat
      (evalIndexedAtom M interpretation (eprProblem certificate))
  rw [eval_toBoolExprNat] at unsat
  have theoryTrue :=
    eval_theoryFormula M interpretation
      (eprGround certificate) certificate.theory
      theoryVerified
  simp only [eprProblem, evalGroundFrm, theoryTrue, Bool.and_true] at unsat
  exact unsat

/-- The generated replay keeps its concrete propositional refutation behind
    `UnsatPin`. The separately checked equality binds the emitted ground
    problem to the certificate without forcing elaboration to normalize the
    whole Boolean formula in a theorem signature. -/
theorem ground_false_of_pinned_problem
    {certificate : EprReplayCertificate}
    (theoryVerified :
      verifyTheory (eprGround certificate)
        certificate.theory = true)
    (groundProblem : GroundFrm)
    (problemBound : eprProblem certificate = groundProblem)
    (problemUnsat :
      Thermite.PropReconstruct.UnsatPin groundProblem.toBoolExprNat)
    (M : Model) (interpretation : GroundInterpretation M) :
    evalGroundFrm M interpretation
        certificate.instantiation.formula = false := by
  have unsat :=
    problemUnsat.proof
      (evalIndexedAtom M interpretation groundProblem)
  rw [eval_toBoolExprNat] at unsat
  rw [← problemBound] at unsat
  have theoryTrue :=
    eval_theoryFormula M interpretation
      (eprGround certificate) certificate.theory
      theoryVerified
  simp only [eprProblem, evalGroundFrm, theoryTrue, Bool.and_true] at unsat
  exact unsat

/-- Variant used by generated files: the LRAT theorem is packed locally so its
    declaration has a constant-size target, then two small equalities bind the
    packed Boolean formula to the checked ground problem. -/
theorem ground_false_of_packed_problem
    {certificate : EprReplayCertificate}
    (theoryVerified :
      verifyTheory (eprGround certificate)
        certificate.theory = true)
    (groundProblem : GroundFrm)
    (groundProblemBound : eprProblem certificate = groundProblem)
    (propositionalProblem : BoolExpr Nat)
    (propositionalProblemBound :
      propositionalProblem = groundProblem.toBoolExprNat)
    (problemUnsat : Thermite.PropReconstruct.PackedUnsat)
    (packedFormulaBound :
      problemUnsat.formula = propositionalProblem)
    (M : Model) (interpretation : GroundInterpretation M) :
    evalGroundFrm M interpretation
        certificate.instantiation.formula = false := by
  have unsat :=
    problemUnsat.proof
      (evalIndexedAtom M interpretation groundProblem)
  rw [packedFormulaBound, propositionalProblemBound,
    eval_toBoolExprNat] at unsat
  rw [← groundProblemBound] at unsat
  have theoryTrue :=
    eval_theoryFormula M interpretation
      (eprGround certificate) certificate.theory
      theoryVerified
  simp only [eprProblem, evalGroundFrm, theoryTrue, Bool.and_true] at unsat
  exact unsat

theorem instantiation_bound_of_verifyEprReplay
    {source : Frm} {certificate : EprReplayCertificate}
    (verified : verifyEprReplay source certificate = true) :
    instantiate certificate.instantiation.grounding.ground source =
      some certificate.instantiation.formula := by
  apply instantiation_eq_of_verify
  simp only [verifyEprReplay, Bool.and_eq_true] at verified
  exact verified.1.1

theorem source_false_of_verifyEprReplay
    {source : Frm} {certificate : EprReplayCertificate}
    (verified : verifyEprReplay source certificate = true)
    (M : Model) (ρ : Valuation M) :
    evalFrm M source ρ = false := by
  apply source_false_of_verifiedInstantiation
  · simp only [verifyEprReplay, Bool.and_eq_true] at verified
    exact verified.1.1
  · intro candidate interpretation
    exact ground_false_of_verifyEprReplay verified candidate interpretation

theorem source_false_of_problem_unsat
    {source : Frm} {certificate : EprReplayCertificate}
    (instantiationVerified :
      verifyInstantiation source certificate.instantiation = true)
    (theoryVerified :
      verifyTheory (eprGround certificate)
        certificate.theory = true)
    (problemUnsat :
      BoolExpr.Unsat (eprProblem certificate).toBoolExprNat)
    (M : Model) (ρ : Valuation M) :
    evalFrm M source ρ = false := by
  apply source_false_of_verifiedInstantiation instantiationVerified
  intro candidate interpretation
  exact ground_false_of_problem_unsat theoryVerified problemUnsat
    candidate interpretation

theorem implication_of_counterexample_false (M : Model)
    (premise conclusion : Frm) (ρ : Valuation M)
    (counterexampleFalse :
      evalFrm M (.conj premise (.neg conclusion)) ρ = false) :
    evalFrm M premise ρ = true → evalFrm M conclusion ρ = true := by
  intro premiseTrue
  cases conclusionValue : evalFrm M conclusion ρ with
  | true => rfl
  | false =>
      simp [evalFrm, premiseTrue, conclusionValue] at counterexampleFalse

theorem checked_implication_of_problem_unsat
    {premise conclusion : Frm} {certificate : EprReplayCertificate}
    (instantiationVerified :
      verifyInstantiation (.conj premise (.neg conclusion))
        certificate.instantiation = true)
    (theoryVerified :
      verifyTheory (eprGround certificate)
        certificate.theory = true)
    (problemUnsat :
      BoolExpr.Unsat (eprProblem certificate).toBoolExprNat)
    (M : Model) (ρ : Valuation M) :
    evalFrm M premise ρ = true → evalFrm M conclusion ρ = true := by
  apply implication_of_counterexample_false M premise conclusion ρ
  exact source_false_of_problem_unsat instantiationVerified theoryVerified
    problemUnsat M ρ

theorem source_false_of_structural_problem_unsat
    {source : Frm} {certificate : EprReplayCertificate}
    (instantiationVerified :
      verifyStructuralInstantiation source certificate.instantiation = true)
    (theoryVerified :
      verifyTheory (eprGround certificate) certificate.theory = true)
    (problemUnsat :
      BoolExpr.Unsat (eprProblem certificate).toBoolExprNat)
    (M : Model) (ρ : Valuation M) :
    evalFrm M source ρ = false := by
  apply source_false_of_verifiedStructuralInstantiation
    instantiationVerified
  intro candidate interpretation
  exact ground_false_of_problem_unsat theoryVerified problemUnsat
    candidate interpretation

theorem checked_structural_implication_of_problem_unsat
    {premise conclusion : Frm} {certificate : EprReplayCertificate}
    (instantiationVerified :
      verifyStructuralInstantiation (.conj premise (.neg conclusion))
        certificate.instantiation = true)
    (theoryVerified :
      verifyTheory (eprGround certificate) certificate.theory = true)
    (problemUnsat :
      BoolExpr.Unsat (eprProblem certificate).toBoolExprNat)
    (M : Model) (ρ : Valuation M) :
    evalFrm M premise ρ = true → evalFrm M conclusion ρ = true := by
  apply implication_of_counterexample_false M premise conclusion ρ
  exact source_false_of_structural_problem_unsat
    instantiationVerified theoryVerified problemUnsat M ρ

theorem checked_structural_binding_implication_of_problem_unsat
    {premise conclusion : Frm} {certificate : EprReplayCertificate}
    (bindingVerified :
      verifyStructuralBinding (.conj premise (.neg conclusion))
        certificate.instantiation = true)
    (theoryVerified :
      verifyTheory (eprGround certificate) certificate.theory = true)
    (problemUnsat :
      BoolExpr.Unsat (eprProblem certificate).toBoolExprNat)
    (M : Model) (ρ : Valuation M) :
    evalFrm M premise ρ = true → evalFrm M conclusion ρ = true := by
  apply implication_of_counterexample_false M premise conclusion ρ
  apply source_false_of_verifiedStructuralBinding bindingVerified
  intro candidate interpretation
  exact ground_false_of_problem_unsat theoryVerified problemUnsat
    candidate interpretation

/-- Production EPR endpoint: a checked structural binding, checked direct
    theory clauses, and a kernel-replayed LRAT certificate establish the actual
    `premise → conclusion` semantics. -/
theorem checked_structural_binding_claim_of_epr_actions
    {premise conclusion : Frm} {certificate : EprReplayCertificate}
    (bindingVerified :
      verifyStructuralBinding (.conj premise (.neg conclusion))
        certificate.instantiation = true)
    (theoryVerified :
      verifyTheory (eprGround certificate) certificate.theory = true)
    (actionsVerified : verifyEprActions certificate = true) :
    EprClaim premise conclusion := by
  apply EprClaim.ofSemantic
  intro M ρ
  apply implication_of_counterexample_false M premise conclusion ρ
  apply source_false_of_verifiedStructuralBinding bindingVerified
  intro candidate interpretation
  exact ground_false_of_epr_actions theoryVerified actionsVerified
    candidate interpretation

/-- Production endpoint for term-producing LRAT replay over the compact EPR
    CNF. -/
theorem checked_structural_binding_claim_of_epr_cnf_unsat
    {premise conclusion : Frm} {certificate : EprReplayCertificate}
    (bindingVerified :
      verifyStructuralBinding (.conj premise (.neg conclusion))
        certificate.instantiation = true)
    (theoryVerified :
      verifyTheory (eprGround certificate) certificate.theory = true)
    (cnfUnsat : (eprCnf certificate).Unsat) :
    EprClaim premise conclusion := by
  apply EprClaim.ofSemantic
  intro M ρ
  apply implication_of_counterexample_false M premise conclusion ρ
  apply source_false_of_verifiedStructuralBinding bindingVerified
  intro candidate interpretation
  exact ground_false_of_epr_cnf_unsat theoryVerified cnfUnsat
    candidate interpretation

theorem checked_structural_binding_implication_of_pinned_problem
    {premise conclusion : Frm} {certificate : EprReplayCertificate}
    (bindingVerified :
      verifyStructuralBinding (.conj premise (.neg conclusion))
        certificate.instantiation = true)
    (theoryVerified :
      verifyTheory (eprGround certificate) certificate.theory = true)
    (groundProblem : GroundFrm)
    (problemBound : eprProblem certificate = groundProblem)
    (problemUnsat :
      Thermite.PropReconstruct.UnsatPin groundProblem.toBoolExprNat)
    (M : Model) (ρ : Valuation M) :
    evalFrm M premise ρ = true → evalFrm M conclusion ρ = true := by
  apply implication_of_counterexample_false M premise conclusion ρ
  apply source_false_of_verifiedStructuralBinding bindingVerified
  intro candidate interpretation
  exact ground_false_of_pinned_problem theoryVerified groundProblem
    problemBound problemUnsat candidate interpretation

theorem checked_structural_binding_implication_of_packed_problem
    {premise conclusion : Frm} {certificate : EprReplayCertificate}
    (bindingVerified :
      verifyStructuralBinding (.conj premise (.neg conclusion))
        certificate.instantiation = true)
    (theoryVerified :
      verifyTheory (eprGround certificate) certificate.theory = true)
    (groundProblem : GroundFrm)
    (groundProblemBound : eprProblem certificate = groundProblem)
    (propositionalProblem : BoolExpr Nat)
    (propositionalProblemBound :
      propositionalProblem = groundProblem.toBoolExprNat)
    (problemUnsat : Thermite.PropReconstruct.PackedUnsat)
    (packedFormulaBound :
      problemUnsat.formula = propositionalProblem)
    (M : Model) (ρ : Valuation M) :
    evalFrm M premise ρ = true → evalFrm M conclusion ρ = true := by
  apply implication_of_counterexample_false M premise conclusion ρ
  apply source_false_of_verifiedStructuralBinding bindingVerified
  intro candidate interpretation
  exact ground_false_of_packed_problem theoryVerified groundProblem
    groundProblemBound propositionalProblem propositionalProblemBound
    problemUnsat packedFormulaBound candidate interpretation

theorem checked_structural_binding_claim_of_packed_problem
    {premise conclusion : Frm} {certificate : EprReplayCertificate}
    (bindingVerified :
      verifyStructuralBinding (.conj premise (.neg conclusion))
        certificate.instantiation = true)
    (theoryVerified :
      verifyTheory (eprGround certificate) certificate.theory = true)
    (groundProblem : GroundFrm)
    (groundProblemBound : eprProblem certificate = groundProblem)
    (propositionalProblem : BoolExpr Nat)
    (propositionalProblemBound :
      propositionalProblem = groundProblem.toBoolExprNat)
    (problemUnsat : Thermite.PropReconstruct.PackedUnsat)
    (packedFormulaBound :
      problemUnsat.formula = propositionalProblem) :
    EprClaim premise conclusion := by
  apply EprClaim.ofSemantic
  intro M ρ
  exact checked_structural_binding_implication_of_packed_problem
    bindingVerified theoryVerified groundProblem groundProblemBound
    propositionalProblem propositionalProblemBound problemUnsat
    packedFormulaBound M ρ

#print axioms ground_false_of_verifyEprReplay
#print axioms ground_false_of_problem_unsat
#print axioms instantiation_bound_of_verifyEprReplay
#print axioms source_false_of_verifyEprReplay
#print axioms source_false_of_problem_unsat
#print axioms checked_implication_of_problem_unsat
#print axioms checked_structural_implication_of_problem_unsat
#print axioms checked_structural_binding_implication_of_problem_unsat
#print axioms checked_structural_binding_implication_of_pinned_problem
#print axioms checked_structural_binding_implication_of_packed_problem
#print axioms checked_structural_binding_claim_of_packed_problem

end Thermite.Strat.Cls
