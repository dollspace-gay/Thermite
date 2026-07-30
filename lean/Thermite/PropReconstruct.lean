/-
  Generic propositional reconstruction through AIG, Tseitin CNF, and LRAT.

  The stage-3 checker was intentionally specialized to reflected bit-vectors.
  Ground EPR formulas need the same checked SAT boundary without pretending to
  be bit-vector expressions. This module compiles `BoolExpr Nat` directly,
  proves the compiler semantics, recomputes the standard-library Tseitin CNF,
  and turns a successful LRAT check into unsatisfiability of the original
  propositional formula.
-/
import Thermite.Reconstruct
import Mathlib.Data.List.Basic
import Mathlib.Data.List.Nodup
import Mathlib.Tactic.Sat.FromLRAT
import Std.Data.HashMap.Lemmas
import Std.Sat.AIG.CachedGatesLemmas
import Std.Sat.CNF.Dimacs
import Std.Tactic.BVDecide.Bitblast.BoolExpr
import Lean.Meta.Tactic.Cbv
import Lean.Meta.Tactic.Grind.Util
import Lean.Meta.Tactic.SplitIf
import Lean.Meta.Reduce
import Lean.Util.ForEachExpr
import Lean.Util.Sorry

open Std
open Std.Sat
open Std.Sat.AIG
open Std.Tactic.BVDecide
open Lean
open Lean.Meta
open Lean.Elab
open Lean.Elab.Tactic

namespace Thermite.PropReconstruct

variable {α : Type} [Hashable α] [DecidableEq α]

/-- An entrypoint whose declaration array extends `base` without changing its
    existing gates. Keeping the full prefix fact makes compiler composition
    semantic, not merely a size calculation. -/
abbrev PrefixEntrypoint (base : AIG α) :=
  { entry : Entrypoint α // IsPrefix base.decls entry.aig.decls }

omit [Hashable α] [DecidableEq α] in
theorem isPrefix_trans {first second third : Array (AIG.Decl α)}
    (left : IsPrefix first second) (right : IsPrefix second third) :
    IsPrefix first third := by
  apply IsPrefix.of
  · intro index inFirst
    rw [right.idx_eq index (Nat.lt_of_lt_of_le inFirst left.size_le)]
    exact left.idx_eq index inFirst
  · exact Nat.le_trans left.size_le right.size_le

/-- Compile a generic Boolean formula into an AIG extending the supplied graph.
    Cached gates make the produced CNF deterministic for identical formulas. -/
def compileFrom (aig : AIG α) : BoolExpr α → PrefixEntrypoint aig
  | .literal atom =>
      let result := aig.mkAtomCached atom
      ⟨result, LawfulOperator.isPrefix_aig (f := mkAtomCached) aig atom⟩
  | .const value =>
      ⟨⟨aig, aig.mkConstCached value⟩, IsPrefix.rfl⟩
  | .not formula =>
      let inner := compileFrom aig formula
      let result := inner.val.aig.mkNotCached inner.val.ref
      ⟨result, isPrefix_trans inner.property
        (LawfulOperator.isPrefix_aig inner.val.aig inner.val.ref)⟩
  | .gate gate left right =>
      let leftResult := compileFrom aig left
      let rightResult := compileFrom leftResult.val.aig right
      let leftRef := leftResult.val.ref.cast rightResult.property.size_le
      let input : BinaryInput rightResult.val.aig :=
        ⟨leftRef, rightResult.val.ref⟩
      match gate with
      | .and =>
          let result := rightResult.val.aig.mkAndCached input
          ⟨result, isPrefix_trans leftResult.property <|
            isPrefix_trans rightResult.property <|
              LawfulOperator.isPrefix_aig rightResult.val.aig input⟩
      | .xor =>
          let result := rightResult.val.aig.mkXorCached input
          ⟨result, isPrefix_trans leftResult.property <|
            isPrefix_trans rightResult.property <|
              LawfulOperator.isPrefix_aig rightResult.val.aig input⟩
      | .beq =>
          let result := rightResult.val.aig.mkBEqCached input
          ⟨result, isPrefix_trans leftResult.property <|
            isPrefix_trans rightResult.property <|
              LawfulOperator.isPrefix_aig rightResult.val.aig input⟩
      | .or =>
          let result := rightResult.val.aig.mkOrCached input
          ⟨result, isPrefix_trans leftResult.property <|
            isPrefix_trans rightResult.property <|
              LawfulOperator.isPrefix_aig rightResult.val.aig input⟩
  | .ite discr thenBranch elseBranch =>
      let discrResult := compileFrom aig discr
      let thenResult := compileFrom discrResult.val.aig thenBranch
      let elseResult := compileFrom thenResult.val.aig elseBranch
      let discrRef := discrResult.val.ref.cast <|
        Nat.le_trans thenResult.property.size_le elseResult.property.size_le
      let thenRef := thenResult.val.ref.cast elseResult.property.size_le
      let input : TernaryInput elseResult.val.aig :=
        ⟨discrRef, thenRef, elseResult.val.ref⟩
      let result := elseResult.val.aig.mkIfCached input
      ⟨result, isPrefix_trans discrResult.property <|
        isPrefix_trans thenResult.property <|
          isPrefix_trans elseResult.property <|
            LawfulOperator.isPrefix_aig elseResult.val.aig input⟩

/- `PrefixEntrypoint` carries every declaration-prefix equality needed by the
   semantic proof. Those proofs are intentionally absent from the executable
   compiler below: reducing them inside a closed LRAT check makes kernel
   evaluation needlessly deep. A size witness is enough to transport refs
   during construction, and proof irrelevance identifies this result with the
   fully proved compiler afterward. -/
abbrev SizedEntrypoint (base : AIG α) :=
  { entry : Entrypoint α // base.decls.size ≤ entry.aig.decls.size }

def compileCheckedFrom (aig : AIG α) :
    BoolExpr α → SizedEntrypoint aig
  | .literal atom =>
      let result := aig.mkAtomCached atom
      ⟨result, LawfulOperator.le_size (f := mkAtomCached) aig atom⟩
  | .const value =>
      ⟨⟨aig, aig.mkConstCached value⟩, Nat.le_refl _⟩
  | .not formula =>
      let inner := compileCheckedFrom aig formula
      let result := inner.val.aig.mkNotCached inner.val.ref
      ⟨result, Nat.le_trans inner.property
        (LawfulOperator.le_size (f := mkNotCached)
          inner.val.aig inner.val.ref)⟩
  | .gate gate left right =>
      let leftResult := compileCheckedFrom aig left
      let rightResult := compileCheckedFrom leftResult.val.aig right
      let leftRef := leftResult.val.ref.cast rightResult.property
      let input : BinaryInput rightResult.val.aig :=
        ⟨leftRef, rightResult.val.ref⟩
      match gate with
      | .and =>
          let result := rightResult.val.aig.mkAndCached input
          ⟨result, Nat.le_trans leftResult.property <|
            Nat.le_trans rightResult.property <|
              LawfulOperator.le_size (f := mkAndCached)
                rightResult.val.aig input⟩
      | .xor =>
          let result := rightResult.val.aig.mkXorCached input
          ⟨result, Nat.le_trans leftResult.property <|
            Nat.le_trans rightResult.property <|
              LawfulOperator.le_size (f := mkXorCached)
                rightResult.val.aig input⟩
      | .beq =>
          let result := rightResult.val.aig.mkBEqCached input
          ⟨result, Nat.le_trans leftResult.property <|
            Nat.le_trans rightResult.property <|
              LawfulOperator.le_size (f := mkBEqCached)
                rightResult.val.aig input⟩
      | .or =>
          let result := rightResult.val.aig.mkOrCached input
          ⟨result, Nat.le_trans leftResult.property <|
            Nat.le_trans rightResult.property <|
              LawfulOperator.le_size (f := mkOrCached)
                rightResult.val.aig input⟩
  | .ite discr thenBranch elseBranch =>
      let discrResult := compileCheckedFrom aig discr
      let thenResult :=
        compileCheckedFrom discrResult.val.aig thenBranch
      let elseResult :=
        compileCheckedFrom thenResult.val.aig elseBranch
      let discrRef := discrResult.val.ref.cast <|
        Nat.le_trans thenResult.property elseResult.property
      let thenRef := thenResult.val.ref.cast elseResult.property
      let input : TernaryInput elseResult.val.aig :=
        ⟨discrRef, thenRef, elseResult.val.ref⟩
      let result := elseResult.val.aig.mkIfCached input
      ⟨result, Nat.le_trans discrResult.property <|
        Nat.le_trans thenResult.property <|
          Nat.le_trans elseResult.property <|
            LawfulOperator.le_size (f := mkIfCached)
              elseResult.val.aig input⟩

theorem compileCheckedFrom_prefix (aig : AIG α)
    (formula : BoolExpr α) :
    IsPrefix aig.decls
      (compileCheckedFrom aig formula).val.aig.decls := by
  induction formula generalizing aig with
  | literal atom =>
      exact LawfulOperator.isPrefix_aig
        (f := mkAtomCached) aig atom
  | const value =>
      exact IsPrefix.rfl
  | not formula ih =>
      exact isPrefix_trans (ih aig) <|
        LawfulOperator.isPrefix_aig
          (compileCheckedFrom aig formula).val.aig
          (compileCheckedFrom aig formula).val.ref
  | gate gate left right leftIH rightIH =>
      let leftResult := compileCheckedFrom aig left
      let rightResult :=
        compileCheckedFrom leftResult.val.aig right
      let leftRef :=
        leftResult.val.ref.cast rightResult.property
      let input : BinaryInput rightResult.val.aig :=
        ⟨leftRef, rightResult.val.ref⟩
      apply isPrefix_trans (leftIH aig)
      apply isPrefix_trans (rightIH leftResult.val.aig)
      cases gate with
      | and =>
          exact LawfulOperator.isPrefix_aig
            (f := mkAndCached) rightResult.val.aig input
      | xor =>
          exact LawfulOperator.isPrefix_aig
            (f := mkXorCached) rightResult.val.aig input
      | beq =>
          exact LawfulOperator.isPrefix_aig
            (f := mkBEqCached) rightResult.val.aig input
      | or =>
          exact LawfulOperator.isPrefix_aig
            (f := mkOrCached) rightResult.val.aig input
  | ite discr thenBranch elseBranch discrIH thenIH elseIH =>
      let discrResult := compileCheckedFrom aig discr
      let thenResult :=
        compileCheckedFrom discrResult.val.aig thenBranch
      let elseResult :=
        compileCheckedFrom thenResult.val.aig elseBranch
      let discrRef := discrResult.val.ref.cast <|
        Nat.le_trans thenResult.property elseResult.property
      let thenRef := thenResult.val.ref.cast elseResult.property
      let input : TernaryInput elseResult.val.aig :=
        ⟨discrRef, thenRef, elseResult.val.ref⟩
      exact isPrefix_trans (discrIH aig) <|
        isPrefix_trans (thenIH discrResult.val.aig) <|
          isPrefix_trans (elseIH thenResult.val.aig) <|
            LawfulOperator.isPrefix_aig
              (f := mkIfCached) elseResult.val.aig input

theorem denote_compileCheckedFrom (aig : AIG α)
    (formula : BoolExpr α) (assign : α → Bool) :
    ⟦(compileCheckedFrom aig formula).val, assign⟧ =
      BoolExpr.eval assign formula := by
  induction formula generalizing aig with
  | literal atom =>
      simp [compileCheckedFrom, BoolExpr.eval]
  | const value =>
      simp [compileCheckedFrom, BoolExpr.eval]
  | not formula ih =>
      simp [compileCheckedFrom, BoolExpr.eval, ih]
  | gate gate left right leftIH rightIH =>
      cases gate <;>
        simp only [compileCheckedFrom, BoolExpr.eval, Gate.eval,
          denote_mkAndCached, denote_mkXorCached, denote_mkBEqCached,
          denote_mkOrCached, Ref.cast_eq, rightIH]
      all_goals
        rw [AIG.denote.eq_of_isPrefix
          (compileCheckedFrom aig left).val
          (compileCheckedFrom
            (compileCheckedFrom aig left).val.aig right).val.aig
          (compileCheckedFrom_prefix
            (compileCheckedFrom aig left).val.aig right)]
        rw [leftIH]
  | ite discr thenBranch elseBranch discrIH thenIH elseIH =>
      simp only [compileCheckedFrom, BoolExpr.eval,
        denote_mkIfCached, Ref.cast_eq, elseIH]
      rw [AIG.denote.eq_of_isPrefix
        (compileCheckedFrom
          (compileCheckedFrom aig discr).val.aig thenBranch).val
        (compileCheckedFrom
          (compileCheckedFrom
            (compileCheckedFrom aig discr).val.aig thenBranch).val.aig
          elseBranch).val.aig
        (compileCheckedFrom_prefix
          (compileCheckedFrom
            (compileCheckedFrom aig discr).val.aig thenBranch).val.aig
          elseBranch)]
      rw [thenIH]
      rw [AIG.denote.eq_of_isPrefix
        (compileCheckedFrom aig discr).val
        (compileCheckedFrom
          (compileCheckedFrom
            (compileCheckedFrom aig discr).val.aig thenBranch).val.aig
          elseBranch).val.aig
        (isPrefix_trans
          (compileCheckedFrom_prefix
            (compileCheckedFrom aig discr).val.aig thenBranch)
          (compileCheckedFrom_prefix
            (compileCheckedFrom
              (compileCheckedFrom aig discr).val.aig thenBranch).val.aig
            elseBranch))]
      rw [discrIH]
      cases BoolExpr.eval assign discr <;> rfl

/-- The AIG compiler evaluates exactly like its input formula. -/
theorem denote_compileFrom (aig : AIG α) (formula : BoolExpr α)
    (assign : α → Bool) :
    ⟦(compileFrom aig formula).val, assign⟧ =
      BoolExpr.eval assign formula := by
  induction formula generalizing aig with
  | literal atom =>
      simp [compileFrom, BoolExpr.eval]
  | const value =>
      simp [compileFrom, BoolExpr.eval]
  | not formula ih =>
      simp [compileFrom, BoolExpr.eval, ih]
  | gate gate left right leftIH rightIH =>
      cases gate <;>
        simp only [compileFrom, BoolExpr.eval, Gate.eval,
          denote_mkAndCached, denote_mkXorCached, denote_mkBEqCached,
          denote_mkOrCached, Ref.cast_eq, rightIH]
      all_goals
        rw [AIG.denote.eq_of_isPrefix
          (compileFrom aig left).val
          (compileFrom (compileFrom aig left).val.aig right).val.aig
          (compileFrom (compileFrom aig left).val.aig right).property]
        rw [leftIH]
  | ite discr thenBranch elseBranch discrIH thenIH elseIH =>
      simp only [compileFrom, BoolExpr.eval,
        denote_mkIfCached, Ref.cast_eq, elseIH]
      rw [AIG.denote.eq_of_isPrefix
        (compileFrom (compileFrom aig discr).val.aig thenBranch).val
        (compileFrom
          (compileFrom (compileFrom aig discr).val.aig thenBranch).val.aig
          elseBranch).val.aig
        (compileFrom
          (compileFrom (compileFrom aig discr).val.aig thenBranch).val.aig
          elseBranch).property]
      rw [thenIH]
      rw [AIG.denote.eq_of_isPrefix
        (compileFrom aig discr).val
        (compileFrom
          (compileFrom (compileFrom aig discr).val.aig thenBranch).val.aig
          elseBranch).val.aig
        (isPrefix_trans
          (compileFrom (compileFrom aig discr).val.aig thenBranch).property
          (compileFrom
            (compileFrom (compileFrom aig discr).val.aig thenBranch).val.aig
            elseBranch).property)]
      rw [discrIH]
      cases BoolExpr.eval assign discr <;> rfl

def compile (formula : BoolExpr α) : Entrypoint α :=
  (compileCheckedFrom AIG.empty formula).val

theorem denote_compile (formula : BoolExpr α) (assign : α → Bool) :
    ⟦compile formula, assign⟧ = BoolExpr.eval assign formula :=
  denote_compileCheckedFrom AIG.empty formula assign

deriving instance DecidableEq for Gate
deriving instance Repr for Gate
deriving instance DecidableEq for BoolExpr
deriving instance Repr for BoolExpr

inductive TseitinVar where
  | source (index : Nat)
  | node (formula : BoolExpr Nat)
  deriving DecidableEq, Repr

def TseitinVar.eval (assign : Nat → Bool) : TseitinVar → Bool
  | .source index => assign index
  | .node formula => BoolExpr.eval assign formula

@[simp] theorem TseitinVar.eval_source
    (assign : Nat → Bool) (index : Nat) :
    TseitinVar.eval assign (.source index) = assign index := rfl

@[simp] theorem TseitinVar.eval_node
    (assign : Nat → Bool) (formula : BoolExpr Nat) :
    TseitinVar.eval assign (.node formula) =
      BoolExpr.eval assign formula := rfl

def tseitinOutput : BoolExpr Nat → TseitinVar
  | .literal index => .source index
  | formula => .node formula

def negateLiteral {β : Type} (literal : Literal β) :
    Literal β :=
  (literal.1, !literal.2)

def positive {β : Type} (atom : β) : Literal β :=
  (atom, true)

def negative {β : Type} (atom : β) : Literal β :=
  (atom, false)

def gateClauses (output : TseitinVar) (gate : Gate)
    (left right : Literal TseitinVar) :
    List (CNF.Clause TseitinVar) :=
  let out := positive output
  let notOut := negative output
  let notLeft := negateLiteral left
  let notRight := negateLiteral right
  match gate with
  | .and =>
      [[notOut, left], [notOut, right], [out, notLeft, notRight]]
  | .or =>
      [[out, notLeft], [out, notRight], [notOut, left, right]]
  | .xor =>
      [[left, right, notOut], [left, notRight, out],
        [notLeft, right, out], [notLeft, notRight, notOut]]
  | .beq =>
      [[left, right, out], [left, notRight, notOut],
        [notLeft, right, notOut], [notLeft, notRight, out]]

def iteClauses (output : TseitinVar)
    (discr thenBranch elseBranch : Literal TseitinVar) :
    List (CNF.Clause TseitinVar) :=
  let out := positive output
  let notOut := negative output
  let notDiscr := negateLiteral discr
  let notThen := negateLiteral thenBranch
  let notElse := negateLiteral elseBranch
  [[notDiscr, notOut, thenBranch],
    [notDiscr, out, notThen],
    [discr, notOut, elseBranch],
    [discr, out, notElse]]

def tseitinClauses : BoolExpr Nat →
    List (CNF.Clause TseitinVar)
  | .literal _ => []
  | formula@(.const value) =>
      [[(tseitinOutput formula, value)]]
  | formula@(.not child) =>
      tseitinClauses child ++
        [[negative (tseitinOutput formula),
            negative (tseitinOutput child)],
          [positive (tseitinOutput formula),
            positive (tseitinOutput child)]]
  | formula@(.gate gate left right) =>
      tseitinClauses left ++ tseitinClauses right ++
        gateClauses (tseitinOutput formula) gate
          (positive (tseitinOutput left))
          (positive (tseitinOutput right))
  | formula@(.ite discr thenBranch elseBranch) =>
      tseitinClauses discr ++ tseitinClauses thenBranch ++
        tseitinClauses elseBranch ++
          iteClauses (tseitinOutput formula)
            (positive (tseitinOutput discr))
            (positive (tseitinOutput thenBranch))
            (positive (tseitinOutput elseBranch))

def tseitinRawClauses (formula : BoolExpr Nat) :
    List (CNF.Clause TseitinVar) :=
  tseitinClauses formula ++ [[positive (tseitinOutput formula)]]

@[simp] theorem eval_tseitinOutput (assign : Nat → Bool)
    (formula : BoolExpr Nat) :
    (tseitinOutput formula).eval assign =
      BoolExpr.eval assign formula := by
  cases formula <;> rfl

theorem eval_gateClauses (assign : Nat → Bool)
    (gate : Gate) (left right : BoolExpr Nat) :
    (gateClauses (.node (.gate gate left right)) gate
      (positive (tseitinOutput left))
      (positive (tseitinOutput right))).all
        (CNF.Clause.eval (TseitinVar.eval assign)) = true := by
  cases gate <;>
    cases leftValue : BoolExpr.eval assign left <;>
    cases rightValue : BoolExpr.eval assign right <;>
    simp [gateClauses, CNF.Clause.eval, positive, negative,
      negateLiteral, TseitinVar.eval_node, BoolExpr.eval, Gate.eval,
      eval_tseitinOutput, leftValue, rightValue]

theorem eval_iteClauses (assign : Nat → Bool)
    (discr thenBranch elseBranch : BoolExpr Nat) :
    (iteClauses (.node (.ite discr thenBranch elseBranch))
      (positive (tseitinOutput discr))
      (positive (tseitinOutput thenBranch))
      (positive (tseitinOutput elseBranch))).all
        (CNF.Clause.eval (TseitinVar.eval assign)) = true := by
  cases discrValue : BoolExpr.eval assign discr <;>
    cases thenValue : BoolExpr.eval assign thenBranch <;>
    cases elseValue : BoolExpr.eval assign elseBranch <;>
    simp [iteClauses, CNF.Clause.eval, positive, negative,
      negateLiteral, TseitinVar.eval_node, BoolExpr.eval,
      eval_tseitinOutput, discrValue, thenValue, elseValue]

theorem eval_tseitinClauses (assign : Nat → Bool) :
    ∀ formula : BoolExpr Nat,
      (tseitinClauses formula).all
        (CNF.Clause.eval (TseitinVar.eval assign)) = true
  | .literal _ => by rfl
  | .const value => by
      simp [tseitinClauses, tseitinOutput, CNF.Clause.eval,
        TseitinVar.eval, BoolExpr.eval]
  | .not child => by
      cases childValue : BoolExpr.eval assign child <;>
        simp [tseitinClauses, eval_tseitinClauses assign child,
          CNF.Clause.eval, positive, negative,
          BoolExpr.eval, childValue,
          eval_tseitinOutput]
  | .gate gate left right => by
      simp only [tseitinClauses, List.all_append,
        eval_tseitinClauses assign left,
        eval_tseitinClauses assign right, Bool.true_and]
      simpa only [tseitinOutput] using
        eval_gateClauses assign gate left right
  | .ite discr thenBranch elseBranch => by
      simp only [tseitinClauses, List.all_append,
        eval_tseitinClauses assign discr,
        eval_tseitinClauses assign thenBranch,
        eval_tseitinClauses assign elseBranch, Bool.true_and]
      simpa only [tseitinOutput] using
        eval_iteClauses assign discr thenBranch elseBranch

theorem eval_tseitinRawClauses (assign : Nat → Bool)
    (formula : BoolExpr Nat)
    (evaluated : BoolExpr.eval assign formula = true) :
    (tseitinRawClauses formula).all
      (CNF.Clause.eval (TseitinVar.eval assign)) = true := by
  simp [tseitinRawClauses, eval_tseitinClauses,
    CNF.Clause.eval, positive, eval_tseitinOutput, evaluated]

/-- All variables occurring in the structural Tseitin clauses, in their first
    occurrence order. Repeated occurrences are harmless: `idxOf` consistently
    selects the first one, while avoiding a second duplicate-removal pass in
    the trusted encoder. -/
def tseitinVariables (formula : BoolExpr Nat) : List TseitinVar :=
  (tseitinRawClauses formula).flatMap fun clause =>
    clause.map Prod.fst

def tseitinRelabelLiteral (formula : BoolExpr Nat)
    (literal : Literal TseitinVar) : Literal Nat :=
  (tseitinVariables formula |>.idxOf literal.1, literal.2)

def tseitinRelabelClause (formula : BoolExpr Nat)
    (clause : CNF.Clause TseitinVar) : CNF.Clause Nat :=
  clause.map (tseitinRelabelLiteral formula)

/-- The production generic Tseitin CNF. Formula nodes, rather than AIG
    declarations, are the auxiliary variables; this keeps both recomputation
    and proof binding structurally recursive. -/
def tseitinCnf (formula : BoolExpr Nat) : CNF Nat where
  clauses :=
    (tseitinRawClauses formula).map
      (tseitinRelabelClause formula) |>.toArray

/-- Regard a clause over source variables as a clause over the structural
    Tseitin variable space. -/
def liftSourceClause (clause : CNF.Clause Nat) :
    CNF.Clause TseitinVar :=
  clause.map fun literal => (.source literal.1, literal.2)

/-- Structural Tseitin clauses followed by already-CNF source clauses. Keeping
    the source clauses flat is important for EPR theory reconstruction: a Horn
    congruence fact should remain one clause, not grow another Boolean syntax
    tree and a layer of Tseitin gates. -/
def tseitinRawClausesWith (formula : BoolExpr Nat)
    (sourceClauses : List (CNF.Clause Nat)) :
    List (CNF.Clause TseitinVar) :=
  tseitinRawClauses formula ++ sourceClauses.map liftSourceClause

def tseitinVariablesWith (formula : BoolExpr Nat)
    (sourceClauses : List (CNF.Clause Nat)) : List TseitinVar :=
  (tseitinRawClausesWith formula sourceClauses).flatMap fun clause =>
    clause.map Prod.fst

def tseitinRelabelLiteralWith (formula : BoolExpr Nat)
    (sourceClauses : List (CNF.Clause Nat))
    (literal : Literal TseitinVar) : Literal Nat :=
  (tseitinVariablesWith formula sourceClauses |>.idxOf literal.1,
    literal.2)

def tseitinRelabelClauseWith (formula : BoolExpr Nat)
    (sourceClauses : List (CNF.Clause Nat))
    (clause : CNF.Clause TseitinVar) : CNF.Clause Nat :=
  clause.map (tseitinRelabelLiteralWith formula sourceClauses)

/-- Tseitin CNF for a formula, extended with direct clauses over the formula's
    source-variable namespace. Variables are relabelled once across both
    portions, so atoms that occur only in an EPR theory clause remain usable. -/
def tseitinCnfWith (formula : BoolExpr Nat)
    (sourceClauses : List (CNF.Clause Nat)) : CNF Nat where
  clauses :=
    (tseitinRawClausesWith formula sourceClauses).map
      (tseitinRelabelClauseWith formula sourceClauses) |>.toArray

def tseitinAssignmentWith (assign : Nat → Bool)
    (formula : BoolExpr Nat)
    (sourceClauses : List (CNF.Clause Nat))
    (index : Nat) : Bool :=
  if inBounds :
      index < (tseitinVariablesWith formula sourceClauses).length then
    ((tseitinVariablesWith formula sourceClauses)[index]).eval assign
  else
    false

theorem variable_mem_tseitinVariablesWith
    {formula : BoolExpr Nat}
    {sourceClauses : List (CNF.Clause Nat)}
    {clause : CNF.Clause TseitinVar}
    {literal : Literal TseitinVar}
    (clauseMember :
      clause ∈ tseitinRawClausesWith formula sourceClauses)
    (literalMember : literal ∈ clause) :
    literal.1 ∈ tseitinVariablesWith formula sourceClauses := by
  simp only [tseitinVariablesWith, List.mem_flatMap, List.mem_map]
  exact ⟨clause, clauseMember, literal, literalMember, rfl⟩

@[simp] theorem tseitinAssignmentWith_idxOf
    (assign : Nat → Bool) (formula : BoolExpr Nat)
    (sourceClauses : List (CNF.Clause Nat))
    (atom : TseitinVar)
    (member : atom ∈ tseitinVariablesWith formula sourceClauses) :
    tseitinAssignmentWith assign formula sourceClauses
        (tseitinVariablesWith formula sourceClauses |>.idxOf atom) =
      atom.eval assign := by
  simp [tseitinAssignmentWith,
    List.idxOf_lt_length_iff.mpr member, List.getElem_idxOf]

theorem eval_liftSourceClause (assign : Nat → Bool)
    (clause : CNF.Clause Nat) :
    CNF.Clause.eval (TseitinVar.eval assign)
        (liftSourceClause clause) =
      CNF.Clause.eval assign clause := by
  induction clause with
  | nil => rfl
  | cons literal rest ih =>
      simp only [liftSourceClause, List.map_cons,
        CNF.Clause.eval_cons, TseitinVar.eval_source]
      change
        (assign literal.1 == literal.2 ||
            CNF.Clause.eval (TseitinVar.eval assign)
              (liftSourceClause rest)) =
          (assign literal.1 == literal.2 ||
            CNF.Clause.eval assign rest)
      rw [ih]

theorem eval_tseitinRelabelClauseWith
    (assign : Nat → Bool) (formula : BoolExpr Nat)
    (sourceClauses : List (CNF.Clause Nat))
    (clause : CNF.Clause TseitinVar)
    (contained :
      ∀ literal ∈ clause,
        literal.1 ∈
          tseitinVariablesWith formula sourceClauses) :
    CNF.Clause.eval
        (tseitinAssignmentWith assign formula sourceClauses)
        (tseitinRelabelClauseWith formula sourceClauses clause) =
      CNF.Clause.eval (TseitinVar.eval assign) clause := by
  induction clause with
  | nil => rfl
  | cons literal rest ih =>
      simp only [tseitinRelabelClauseWith, List.map_cons,
        tseitinRelabelLiteralWith, CNF.Clause.eval_cons]
      rw [tseitinAssignmentWith_idxOf assign formula sourceClauses
        literal.1 (contained literal List.mem_cons_self)]
      simpa only [tseitinRelabelClauseWith] using
        congrArg
          (fun tail =>
            (TseitinVar.eval assign literal.1 == literal.2) || tail)
          (ih (fun restLiteral restMember =>
            contained restLiteral
              (List.mem_cons_of_mem literal restMember)))

theorem eval_tseitinRawClausesWith
    (assign : Nat → Bool) (formula : BoolExpr Nat)
    (sourceClauses : List (CNF.Clause Nat))
    (evaluated : BoolExpr.eval assign formula = true)
    (sourceSatisfied :
      sourceClauses.all (CNF.Clause.eval assign) = true) :
    (tseitinRawClausesWith formula sourceClauses).all
        (CNF.Clause.eval (TseitinVar.eval assign)) = true := by
  have liftedSatisfied :
      (sourceClauses.map liftSourceClause).all
          (CNF.Clause.eval (TseitinVar.eval assign)) = true := by
    rw [List.all_eq_true]
    intro lifted liftedMember
    simp only [List.mem_map] at liftedMember
    rcases liftedMember with ⟨clause, clauseMember, rfl⟩
    rw [eval_liftSourceClause]
    exact (List.all_eq_true.mp sourceSatisfied) clause clauseMember
  simp only [tseitinRawClausesWith, List.all_append,
    eval_tseitinRawClauses assign formula evaluated, Bool.true_and]
  exact liftedSatisfied

theorem tseitinCnfWith_sat_of_eval
    (assign : Nat → Bool) (formula : BoolExpr Nat)
    (sourceClauses : List (CNF.Clause Nat))
    (evaluated : BoolExpr.eval assign formula = true)
    (sourceSatisfied :
      sourceClauses.all (CNF.Clause.eval assign) = true) :
    (tseitinCnfWith formula sourceClauses).Sat
      (tseitinAssignmentWith assign formula sourceClauses) := by
  have rawSatisfied :=
    eval_tseitinRawClausesWith assign formula sourceClauses
      evaluated sourceSatisfied
  simp only [CNF.Sat, CNF.eval, tseitinCnfWith,
    List.all_toArray, List.all_map, Function.comp_apply,
    List.all_eq_true]
  intro clause clauseMember
  rw [eval_tseitinRelabelClauseWith assign formula sourceClauses
    clause (fun literal literalMember =>
      variable_mem_tseitinVariablesWith clauseMember literalMember)]
  exact (List.all_eq_true.mp rawSatisfied) clause clauseMember

def tseitinAssignment (assign : Nat → Bool)
    (formula : BoolExpr Nat) (index : Nat) : Bool :=
  if inBounds : index < (tseitinVariables formula).length then
    ((tseitinVariables formula)[index]).eval assign
  else
    false

theorem variable_mem_tseitinVariables
    {formula : BoolExpr Nat}
    {clause : CNF.Clause TseitinVar}
    {literal : Literal TseitinVar}
    (clauseMember : clause ∈ tseitinRawClauses formula)
    (literalMember : literal ∈ clause) :
    literal.1 ∈ tseitinVariables formula := by
  simp only [tseitinVariables, List.mem_flatMap, List.mem_map]
  exact ⟨clause, clauseMember, literal, literalMember, rfl⟩

@[simp] theorem tseitinAssignment_idxOf
    (assign : Nat → Bool) (formula : BoolExpr Nat)
    (atom : TseitinVar)
    (member : atom ∈ tseitinVariables formula) :
    tseitinAssignment assign formula
        (tseitinVariables formula |>.idxOf atom) =
      atom.eval assign := by
  simp [tseitinAssignment, List.idxOf_lt_length_iff.mpr member,
    List.getElem_idxOf]

theorem eval_tseitinRelabelClause
    (assign : Nat → Bool) (formula : BoolExpr Nat)
    (clause : CNF.Clause TseitinVar)
    (contained :
      ∀ literal ∈ clause,
        literal.1 ∈ tseitinVariables formula) :
    CNF.Clause.eval (tseitinAssignment assign formula)
        (tseitinRelabelClause formula clause) =
      CNF.Clause.eval (TseitinVar.eval assign) clause := by
  induction clause with
  | nil => rfl
  | cons literal rest ih =>
      simp only [tseitinRelabelClause, List.map_cons,
        tseitinRelabelLiteral, CNF.Clause.eval_cons]
      rw [tseitinAssignment_idxOf assign formula literal.1
        (contained literal (List.mem_cons_self))]
      simpa only [tseitinRelabelClause] using
        congrArg
          (fun tail =>
            (TseitinVar.eval assign literal.1 == literal.2) || tail)
          (ih (fun restLiteral restMember =>
            contained restLiteral
              (List.mem_cons_of_mem literal restMember)))

theorem tseitinCnf_sat_of_eval
    (assign : Nat → Bool) (formula : BoolExpr Nat)
    (evaluated : BoolExpr.eval assign formula = true) :
    (tseitinCnf formula).Sat
      (tseitinAssignment assign formula) := by
  have rawSatisfied :=
    eval_tseitinRawClauses assign formula evaluated
  simp only [CNF.Sat, CNF.eval, tseitinCnf, List.all_toArray,
    List.all_map, Function.comp_apply, List.all_eq_true]
  intro clause clauseMember
  rw [eval_tseitinRelabelClause assign formula clause
    (fun literal literalMember =>
      variable_mem_tseitinVariables clauseMember literalMember)]
  exact (List.all_eq_true.mp rawSatisfied) clause clauseMember

/-- Unsatisfiability of the recomputed structural Tseitin CNF refutes the
    original generic propositional formula. -/
theorem unsat_of_tseitinCnf (formula : BoolExpr Nat)
    (cnfUnsat : (tseitinCnf formula).Unsat) :
    BoolExpr.Unsat formula := by
  intro assign
  cases evaluated : BoolExpr.eval assign formula with
  | false => rfl
  | true =>
      have cnfSat :=
        tseitinCnf_sat_of_eval assign formula evaluated
      have contradicted :=
        cnfUnsat (tseitinAssignment assign formula)
      rw [cnfSat] at contradicted
      exact contradicted

def verifyActions (formula : BoolExpr Nat)
    (certificate : Array LRAT.IntAction) : Bool :=
  LRAT.check certificate (tseitinCnf formula)

def formulaCnf (formula : BoolExpr Nat) : CNF Nat :=
  tseitinCnf formula

def verifyCnf (formula : BoolExpr Nat) (cnf : CNF Nat) : Bool :=
  decide ((formulaCnf formula).clauses = cnf.clauses)

theorem cnf_eq_of_verifyCnf (formula : BoolExpr Nat) (cnf : CNF Nat)
    (checked : verifyCnf formula cnf = true) :
    formulaCnf formula = cnf := by
  cases cnf
  apply congrArg CNF.mk
  exact of_decide_eq_true checked

theorem unsat_of_verifyCnf (formula : BoolExpr Nat) (cnf : CNF Nat)
    (checked : verifyCnf formula cnf = true)
    (unsat : cnf.Unsat) :
    (formulaCnf formula).Unsat := by
  rw [cnf_eq_of_verifyCnf formula cnf checked]
  exact unsat

theorem formulaCnf_unsat_of_eq
    (left right : BoolExpr Nat) (bound : left = right)
    (unsat : (formulaCnf right).Unsat) :
    (formulaCnf left).Unsat := by
  subst bound
  exact unsat

theorem boolExpr_unsat_of_eq
    (left right : BoolExpr Nat) (bound : left = right)
    (unsat : BoolExpr.Unsat right) :
    BoolExpr.Unsat left := by
  subst bound
  exact unsat

theorem cnf_unsat_of_eq (left right : CNF Nat)
    (bound : left = right) (unsat : right.Unsat) :
    left.Unsat := by
  subst bound
  exact unsat

def toProofLiteral : Std.Sat.Literal Nat → _root_.Sat.Literal
  | (index, true) => .pos index
  | (index, false) => .neg index

def toProofClause (clause : CNF.Clause Nat) : _root_.Sat.Clause :=
  clause.map toProofLiteral

def toProofFormula (cnf : CNF Nat) : _root_.Sat.Fmla :=
  cnf.clauses.toList.map toProofClause

def proofValuation (assign : Nat → Bool) : _root_.Sat.Valuation :=
  fun index => assign index = true

theorem satisfies_proofClause_of_eval
    (assign : Nat → Bool) (clause : CNF.Clause Nat)
    (evaluated : clause.eval assign = true) :
    (proofValuation assign).satisfies (toProofClause clause) := by
  induction clause with
  | nil =>
      simp at evaluated
  | cons literal rest ih =>
      rcases literal with ⟨index, polarity⟩
      simp only [CNF.Clause.eval_cons] at evaluated
      cases polarity with
      | false =>
          cases value : assign index with
          | false =>
              simp [toProofClause, toProofLiteral, proofValuation,
                _root_.Sat.Valuation.satisfies,
                _root_.Sat.Valuation.neg, value]
          | true =>
              have restEvaluated :
                  CNF.Clause.eval assign rest = true := by
                simpa [value] using evaluated
              simpa [toProofClause, toProofLiteral, proofValuation,
                _root_.Sat.Valuation.satisfies,
                _root_.Sat.Valuation.neg, value] using ih restEvaluated
      | true =>
          cases value : assign index with
          | false =>
              have restEvaluated :
                  CNF.Clause.eval assign rest = true := by
                simpa [value] using evaluated
              simpa [toProofClause, toProofLiteral, proofValuation,
                _root_.Sat.Valuation.satisfies,
                _root_.Sat.Valuation.neg, value] using ih restEvaluated
          | true =>
              simp [toProofClause, toProofLiteral, proofValuation,
                _root_.Sat.Valuation.satisfies,
                _root_.Sat.Valuation.neg, value]

theorem satisfies_proofFormula_of_eval
    (assign : Nat → Bool) (cnf : CNF Nat)
    (evaluated : cnf.eval assign = true) :
    (proofValuation assign).satisfies_fmla (toProofFormula cnf) := by
  constructor
  intro proofClause member
  simp only [toProofFormula, List.mem_map] at member
  rcases member with ⟨clause, member, rfl⟩
  apply satisfies_proofClause_of_eval
  simp only [CNF.eval, Array.all_eq_true] at evaluated
  rw [Array.mem_toList_iff] at member
  rw [Array.mem_iff_getElem] at member
  rcases member with ⟨index, inBounds, rfl⟩
  exact evaluated index inBounds

/-- Mathlib's term-producing LRAT replay proves the empty proof clause from a
    reified formula. Binding that formula to the exact recomputed Std CNF gives
    the same unsatisfiability fact as the Boolean checker, without relying on
    the latter's deeply recursive CBV execution path. -/
theorem cnf_unsat_of_proof
    (cnf : CNF Nat) (proofFormula : _root_.Sat.Fmla)
    (bound : proofFormula = toProofFormula cnf)
    (refutation : proofFormula.proof _root_.Sat.Clause.nil) :
    cnf.Unsat := by
  intro assign
  cases evaluated : cnf.eval assign with
  | false => rfl
  | true =>
      exfalso
      exact refutation (proofValuation assign)
        (bound ▸ satisfies_proofFormula_of_eval assign cnf evaluated)

/-- A checked LRAT refutation of the recomputed Tseitin CNF refutes the original
    generic propositional formula. -/
theorem unsat_of_verifyActions (formula : BoolExpr Nat)
    (certificate : Array LRAT.IntAction)
    (checked : verifyActions formula certificate = true) :
    BoolExpr.Unsat formula := by
  apply unsat_of_tseitinCnf formula
  exact LRAT.check_sound certificate (tseitinCnf formula) checked

/-- The reconstruction endpoint used by generated EPR proofs. Refuting
    `premise ∧ ¬conclusion` establishes the source obligation itself, rather
    than merely certifying an unrelated unsatisfiable Boolean expression. -/
theorem implication_of_verifyActions
    (premise conclusion : BoolExpr Nat)
    (certificate : Array LRAT.IntAction)
    (checked :
      verifyActions
        (.gate .and premise (.not conclusion)) certificate = true)
    (assign : Nat → Bool)
    (premiseTrue : BoolExpr.eval assign premise = true) :
    BoolExpr.eval assign conclusion = true := by
  have unsat := unsat_of_verifyActions
    (.gate .and premise (.not conclusion)) certificate checked assign
  simp only [BoolExpr.eval, Gate.eval, premiseTrue, Bool.true_and] at unsat
  cases conclusionValue : BoolExpr.eval assign conclusion <;>
    simp_all

theorem verifyActions_eq_of_bound
    (formula : BoolExpr Nat)
    (certificate checkedCertificate : Array LRAT.IntAction)
    (cnf : CNF Nat) (expected : Bool)
    (certificateBound : certificate = checkedCertificate)
    (cnfBound : tseitinCnf formula = cnf)
    (checked : LRAT.check checkedCertificate cnf = expected) :
    verifyActions formula certificate = expected := by
  simp only [verifyActions]
  rw [certificateBound, cnfBound]
  exact checked

theorem lratCheck_eq_of_bound
    (certificate checkedCertificate : Array LRAT.IntAction)
    (cnf checkedCnf : CNF Nat) (expected : Bool)
    (certificateBound : certificate = checkedCertificate)
    (cnfBound : cnf = checkedCnf)
    (checked : LRAT.check checkedCertificate checkedCnf = expected) :
    LRAT.check certificate cnf = expected := by
  rw [certificateBound, cnfBound]
  exact checked

theorem lratCheck_eq_true_of_internal
    (certificate : Array LRAT.IntAction) (cnf : CNF Nat)
    (internal : LRAT.Internal.DefaultFormula (cnf.numLiterals + 1))
    (internalBound : LRAT.Internal.CNF.convertLRAT cnf = internal)
    (checked :
      LRAT.Internal.compactLratChecker internal certificate =
        .success) :
    LRAT.check certificate cnf = true := by
  simp [LRAT.check, internalBound, checked]

theorem lratCheck_eq_true_of_stages
    (certificate : Array LRAT.IntAction) (cnf : CNF Nat)
    (lifted :
      CNF (LRAT.Internal.PosFin (cnf.numLiterals + 1)))
    (converted :
      Array (Option
        (LRAT.Internal.DefaultClause (cnf.numLiterals + 1))))
    (internal :
      LRAT.Internal.DefaultFormula (cnf.numLiterals + 1))
    (liftedBound : LRAT.Internal.CNF.lift cnf = lifted)
    (convertedBound :
      LRAT.Internal.CNF.convertLRAT' lifted = converted)
    (internalBound :
      LRAT.Internal.DefaultFormula.ofArray
        (#[none] ++ converted) = internal)
    (checked :
      LRAT.Internal.compactLratChecker internal certificate =
        .success) :
    LRAT.check certificate cnf = true := by
  simp [LRAT.check, LRAT.Internal.CNF.convertLRAT,
    liftedBound, convertedBound, internalBound, checked]

theorem of_platform_numBits_cases {proposition : Prop}
    (case32 : System.Platform.numBits = 32 → proposition)
    (case64 : System.Platform.numBits = 64 → proposition) :
    proposition :=
  System.Platform.numBits_eq.elim case32 case64

theorem getNumBits_value_eq_of_numBits_eq {width : Nat}
    (bound : System.Platform.numBits = width) :
    (System.Platform.getNumBits ()).val = width :=
  bound

theorem of_getNumBits_cases
    (motive :
      {width : Nat // width = 32 ∨ width = 64} → Prop)
    (case32 : motive ⟨32, Or.inl rfl⟩)
    (case64 : motive ⟨64, Or.inr rfl⟩) :
    motive (System.Platform.getNumBits ()) := by
  rcases (System.Platform.getNumBits ()).property with h | h
  · have actual :
        System.Platform.getNumBits () =
          ⟨32, Or.inl rfl⟩ :=
      Subtype.ext h
    rw [actual]
    exact case32
  · have actual :
        System.Platform.getNumBits () =
          ⟨64, Or.inr rfl⟩ :=
      Subtype.ext h
    rw [actual]
    exact case64

attribute [local irreducible, local cbv_opaque]
  System.Platform.numBits

omit [Hashable α] [DecidableEq α] in
theorem array_foldl_nil
    (folder : β → α → β) (initial : β) :
    (#[] : Array α).foldl folder initial = initial := by
  rfl

omit [Hashable α] [DecidableEq α] in
theorem array_foldl_one
    (folder : β → α → β) (initial : β) (first : α) :
    (#[first] : Array α).foldl folder initial =
      folder initial first := by
  rfl

omit [Hashable α] [DecidableEq α] in
theorem array_foldl_two
    (folder : β → α → β) (initial : β) (first second : α) :
    (#[first, second] : Array α).foldl folder initial =
      folder (folder initial first) second := by
  rfl

omit [Hashable α] [DecidableEq α] in
theorem array_foldl_three
    (folder : β → α → β) (initial : β)
    (first second third : α) :
    (#[first, second, third] : Array α).foldl folder initial =
      folder (folder (folder initial first) second) third := by
  rfl

omit [Hashable α] [DecidableEq α] in
theorem listToArray_foldl_nil
    (folder : β → α → β) (initial : β) :
    ([] : List α).toArray.foldl folder initial = initial := by
  rfl

omit [Hashable α] [DecidableEq α] in
theorem listToArray_foldl_one
    (folder : β → α → β) (initial : β) (first : α) :
    ([first] : List α).toArray.foldl folder initial =
      folder initial first := by
  rfl

omit [Hashable α] [DecidableEq α] in
theorem listToArray_foldl_two
    (folder : β → α → β) (initial : β) (first second : α) :
    ([first, second] : List α).toArray.foldl folder initial =
      folder (folder initial first) second := by
  rfl

omit [Hashable α] [DecidableEq α] in
theorem listToArray_foldl_three
    (folder : β → α → β) (initial : β)
    (first second third : α) :
    ([first, second, third] : List α).toArray.foldl folder initial =
      folder (folder (folder initial first) second) third := by
  rfl

@[simp]
theorem rawExpandIfNecessary_eq_self
    (map : Std.DHashMap.Internal.Raw₀ α (fun _ => β))
    (capacity :
      map.1.size * 4 / 3 ≤ map.1.buckets.size) :
    Std.DHashMap.Internal.Raw₀.expandIfNecessary map = map := by
  rcases map with ⟨⟨size, buckets⟩, positive⟩
  change size * 4 / 3 ≤ buckets.size at capacity
  change
    (if size * 4 / 3 ≤ buckets.size then
      (⟨⟨size, buckets⟩, positive⟩ :
        Std.DHashMap.Internal.Raw₀ α (fun _ => β))
    else _) =
      (⟨⟨size, buckets⟩, positive⟩ :
        Std.DHashMap.Internal.Raw₀ α (fun _ => β))
  rw [if_pos capacity]

/-- Hash-map bucket masks are formed by converting the bucket count to
    `USize`, subtracting one, and converting the result back to `Nat`.  This
    lemma keeps that platform-dependent representation behind two elementary
    bounds, so concrete reconstruction only has to discharge those bounds. -/
theorem usizeOfNatSubOneToNat {n : Nat}
    (positive : 0 < n) (fits : n < USize.size) :
    (OfNat.ofNat n - 1 : USize).toNat = n - 1 := by
  change (USize.ofNat n - USize.ofNat 1).toNat = n - 1
  rw [USize.toNat_sub_of_le]
  · simp [USize.toNat_ofNat_of_lt' fits]
  · rw [USize.le_iff_toNat_le]
    simp [USize.toNat_ofNat_of_lt' fits]
    simpa using positive

def defaultClauseOfNodupKeys
    (clause :
      CNF.Clause (LRAT.Internal.PosFin n))
    (keysNodup : (clause.map Prod.fst).Nodup) :
    LRAT.Internal.DefaultClause n where
  clause := clause
  nodupkey := by
    intro literal
    by_cases positive : (literal, true) ∈ clause
    · right
      intro negative
      have pairEqual :=
        List.inj_on_of_nodup_map keysNodup
          positive negative rfl
      simp at pairEqual
    · exact Or.inl positive
  nodup := List.Nodup.of_map Prod.fst keysNodup

def defaultClauseOfHashMap
    (map : HashMap (LRAT.Internal.PosFin n) Bool) :
    LRAT.Internal.DefaultClause n :=
  have keysNodup :
      (map.toList.map Prod.fst).Nodup := by
    rw [map.map_fst_toList_eq_keys]
    exact map.nodup_keys
  defaultClauseOfNodupKeys map.toList keysNodup

theorem someDefaultClause_eq_of_clause_eq
    (left right : LRAT.Internal.DefaultClause n)
    (clauses : left.clause = right.clause) :
    (some left : Option (LRAT.Internal.DefaultClause n)) =
      some right :=
  congrArg some (LRAT.Internal.DefaultClause.ext clauses)

theorem someDefaultClauseOfNodupKeys_eq
    (left right :
      CNF.Clause (LRAT.Internal.PosFin n))
    (leftNodup : (left.map Prod.fst).Nodup)
    (rightNodup : (right.map Prod.fst).Nodup)
    (clauses : left = right) :
    (some (defaultClauseOfNodupKeys left leftNodup) :
      Option (LRAT.Internal.DefaultClause n)) =
      some (defaultClauseOfNodupKeys right rightNodup) := by
  subst right
  rfl

/-- Convert a list-backed clause through `List.foldl` instead of exposing the
    implementation loop behind `Array.foldl`. Generated LRAT clauses are
    reified as lists, so this keeps every intermediate map expression closed
    and makes the subsequent kernel reconstruction independent of array-loop
    binders. -/
theorem defaultClauseOfArray_toArray
    (clause : CNF.Clause (LRAT.Internal.PosFin n)) :
    LRAT.Internal.DefaultClause.ofArray clause.toArray =
      let mapOption :=
        clause.foldl
          LRAT.Internal.DefaultClause.ofArray.folder
          (some (HashMap.emptyWithCapacity clause.length))
      match mapOption with
      | none => none
      | some map =>
          some (defaultClauseOfHashMap map) := by
  unfold LRAT.Internal.DefaultClause.ofArray
  rw [← Array.foldl_toList]
  simp only [List.size_toArray]
  split
  · simp_all
  · simp_all [defaultClauseOfHashMap]
    apply LRAT.Internal.DefaultClause.ext
    rfl

theorem defaultClauseFolder_none
    (literal : Literal (LRAT.Internal.PosFin n)) :
    LRAT.Internal.DefaultClause.ofArray.folder none literal =
      none := by
  rw [LRAT.Internal.DefaultClause.ofArray.folder.eq_def]

theorem defaultClauseFolder_eq
    (acc : Option (HashMap (LRAT.Internal.PosFin n) Bool))
    (literal : Literal (LRAT.Internal.PosFin n)) :
    LRAT.Internal.DefaultClause.ofArray.folder acc literal =
      match acc with
      | none => none
      | some map =>
          let (value?, updated) :=
            map.getThenInsertIfNew? literal.1 literal.2
          if let some previous := value? then
            if literal.2 != previous then
              none
            else
              some updated
          else
            some updated := by
  rw [LRAT.Internal.DefaultClause.ofArray.folder.eq_def]
  rfl

theorem defaultClauseFolder_some
    (map : HashMap (LRAT.Internal.PosFin n) Bool)
    (literal : Literal (LRAT.Internal.PosFin n)) :
    LRAT.Internal.DefaultClause.ofArray.folder
        (some map) literal =
      let (value?, updated) :=
        map.getThenInsertIfNew? literal.1 literal.2
      if let some previous := value? then
        if literal.2 != previous then none else some updated
      else
        some updated := by
  rw [LRAT.Internal.DefaultClause.ofArray.folder.eq_def]
  rfl

theorem eq_of_platform_numBits_cases {α : Type}
    (left right : α)
    (case32 :
      ∀ h : System.Platform.numBits = 32,
        (⟨h, left⟩ :
          PSigma fun _ : System.Platform.numBits = 32 => α) =
        ⟨h, right⟩)
    (case64 :
      ∀ h : System.Platform.numBits = 64,
        (⟨h, left⟩ :
          PSigma fun _ : System.Platform.numBits = 64 => α) =
        ⟨h, right⟩) :
    left = right := by
  rcases System.Platform.numBits_eq with h | h
  · exact congrArg
      (fun pair :
        PSigma fun _ : System.Platform.numBits = 32 => α =>
          pair.2)
      (case32 h)
  · exact congrArg
      (fun pair :
        PSigma fun _ : System.Platform.numBits = 64 => α =>
          pair.2)
      (case64 h)

/-- Keep a concrete unsatisfiability result behind a proposition boundary.
    Generated EPR files use this wrapper so elaborating a theorem statement
    does not normalize a large reflected formula before LRAT reconstruction. -/
structure UnsatPin (formula : BoolExpr Nat) : Prop where
  proof : BoolExpr.Unsat formula

/-- An existentially packaged reflected formula and its refutation. This form
    gives generated theorems a constant-size elaboration target; a local
    definition then exposes the exact formula to the generic replay theorem. -/
structure PackedUnsat where
  formula : BoolExpr Nat
  proof : BoolExpr.Unsat formula

/-- Kernel-evaluate a concrete `verifyActions … = true/false` goal using the
    same projection-folding boundary as `bv_reconstruct`. This is deliberately
    not native evaluation: the produced proof is checked by the kernel and has
    only the standard axiom footprint. -/
syntax (name := kernelLratCheck) "kernel_lrat_check" : tactic
syntax (name := kernelLratCnfCheck)
  "kernel_lrat_cnf_check " str " with " str : tactic
syntax (name := kernelLratCnfUnsat)
  "kernel_lrat_cnf_unsat " str " with " str : tactic
syntax (name := kernelBoolCheck) "kernel_bool_check" : tactic
syntax (name := kernelLratUnsat)
  "kernel_lrat_unsat " str " with " str : tactic
syntax (name := kernelLratPacked)
  "kernel_lrat_packed " str " with " str : term
syntax (name := kernelLratChecked)
  "kernel_lrat_checked " str " with " str : term
syntax (name := kernelLratPackedDecl)
  "kernel_lrat_packed_decl " ident " from " str " with " str : command
syntax (name := kernelLratTextDecl)
  "kernel_lrat_text_decl " ident " from " str : command

private def addAuxDecl (name : Name) (value type : Expr) : CoreM Unit :=
  withOptions (fun options => options.set `compiler.extract_closed false) do
    addAndCompile <| .defnDecl {
      name
      levelParams := []
      type
      value
      hints := .abbrev
      safety := .safe
    }

private def addKernelAuxDecl
    (name : Name) (value type : Expr) : CoreM Unit :=
  addDecl <| .defnDecl {
    name
    levelParams := []
    type
    value
    hints := .abbrev
    safety := .safe
  }

/-- Parse an LRAT text literal during elaboration and declare the resulting
    action array as transparent data. The parser does not create a proof: the
    generated array must still pass `LRAT.check` in the kernel. Keeping parsing
    outside the theorem term avoids asking kernel reduction to execute the
    parser's partial recursive loop. -/
@[command_elab kernelLratTextDecl]
unsafe def elabKernelLratTextDecl :
    Lean.Elab.Command.CommandElab := fun stx => do
  let `(kernel_lrat_text_decl $output:ident from $certificate:str) := stx
    | throwUnsupportedSyntax
  let declName := (← getCurrNamespace) ++ output.getId
  if (← getEnv).contains declName then
    throwError "declaration `{declName}` already exists"
  let actions ←
    match LRAT.parseLRATProof certificate.getString.toUTF8 with
    | .ok actions => pure actions
    | .error error =>
        throwError "invalid LRAT certificate: {error}"
  let certificateType :=
    mkApp (mkConst ``Array [.zero]) (mkConst ``LRAT.IntAction)
  Lean.Elab.Command.liftCoreM <|
    withOptions
        (fun options => options.set `compiler.extract_closed false) do
      addAndCompile <| .defnDecl {
        name := declName
        levelParams := []
        type := certificateType
        value := toExpr actions
        hints := .abbrev
        safety := .safe
      }

private def actionsToLratText
    (actions : Array LRAT.IntAction) : Except String String :=
  pure (LRAT.lratProofToString actions)

private def cloneBoolExpr : BoolExpr Nat → BoolExpr Nat
  | .literal atom => .literal atom
  | .const value => .const value
  | .not formula => .not (cloneBoolExpr formula)
  | .gate gate left right =>
      .gate gate (cloneBoolExpr left) (cloneBoolExpr right)
  | .ite discr thenBranch elseBranch =>
      .ite (cloneBoolExpr discr)
        (cloneBoolExpr thenBranch) (cloneBoolExpr elseBranch)

/- Reduce a concrete Boolean equality with Lean's proof-producing symbolic
   evaluator. The resulting expression is checked by the kernel. -/
private unsafe def cbvReconstructionGoal
    (goal : MVarId) (fuel := 64) : MetaM Unit := do
  let rec loop (current : MVarId) : Nat → MetaM Unit
    | 0 => do
        throwError m!"kernel reconstruction left a residual equality:\n\
          {← current.getType}"
    | fuel + 1 => do
        match ← Lean.Meta.Tactic.Cbv.cbvGoalCore current with
        | none => pure ()
        | some residual => loop residual fuel
  loop goal fuel

private unsafe def cbvPlatformReconstructionGoal
    (goal : MVarId) : MetaM Unit := do
  let target ← goal.getType
  let some (_, left, right) := target.eq?
    | throwError
        "platform kernel reconstruction expects an equality"
  let solveCase (caseGoal : MVarId) (widthBound : Expr) :
      MetaM Unit := do
    let caseTarget ← caseGoal.getType
    let some (_, caseLeft, caseRight) := caseTarget.eq?
      | throwError
          "platform case reconstruction expects an equality"
    let addWidthBound
        (theoremSets : SimpTheoremsArray) :
        MetaM SimpTheoremsArray := do
      let theoremSets ← theoremSets.addTheorem
        (.other `thermitePlatformWidth) widthBound
      let getNumBitsProof ← mkAppM
        ``Thermite.PropReconstruct.getNumBits_value_eq_of_numBits_eq
        #[widthBound]
      theoremSets.addTheorem
        (.other `thermitePlatformGetNumBits) getNumBitsProof
    let simplifyWithDefinitions
        (input : Simp.Result) (definitions : Array Name)
        (useGlobal := true) : MetaM Simp.Result := do
      let mut localTheorems : SimpTheorems := {}
      for definition in definitions do
        localTheorems ←
          localTheorems.addDeclToUnfold definition
      let globalTheorems ← getSimpTheorems
      let theoremSets : SimpTheoremsArray :=
        if useGlobal then
          #[globalTheorems, localTheorems]
        else
          #[localTheorems]
      let theoremSets ← addWidthBound theoremSets
      let context ← Simp.mkContext
        { failIfUnchanged := false, maxSteps := 1_000_000 }
        (simpTheorems := theoremSets)
      let (next, _) ← Lean.Meta.simp input.expr context
      input.mkEqTrans next
    let rec proveClosedProposition
        (goal : MVarId) (branchFacts : Array Expr) :
        Nat → MetaM Unit
      | 0 => do
          throwError m!"kernel reconstruction exhausted case splits while proving:\n\
            {← goal.getType}"
      | fuel + 1 => goal.withContext do
          let mut theoremSets : SimpTheoremsArray :=
            #[← getSimpTheorems]
          theoremSets ← addWidthBound theoremSets
          for fact in branchFacts do
            theoremSets ← theoremSets.addTheorem
              (.other `thermiteMapCapacityCase) fact
          let context ← Simp.mkContext
            { failIfUnchanged := false, maxSteps := 1_000_000 }
            (simpTheorems := theoremSets)
          let (remaining?, _) ←
            Lean.Meta.simpTarget goal context
          match remaining? with
          | none => pure ()
          | some remaining =>
              match ← Lean.Meta.splitIfTarget? remaining with
              | some (positive, negative) => do
                  proveClosedProposition positive.mvarId
                    (branchFacts.push (mkFVar positive.fvarId)) fuel
                  proveClosedProposition negative.mvarId
                    (branchFacts.push (mkFVar negative.fvarId)) fuel
              | none => do
                  let residualTarget ← remaining.getType
                  let residualProof ← mkDecideProof residualTarget
                  Lean.Meta.checkWithKernel residualProof
                  remaining.assign residualProof
    let normalizeInsert (insert : Expr) :
        MetaM Simp.Result := do
      let groups : Array (Array Name × Bool) := #[
        (#[
          ``Std.HashMap.insertIfNew,
          ``Std.HashMap.emptyWithCapacity
        ], true),
        (#[
          ``Std.DHashMap.insertIfNew,
          ``Std.DHashMap.emptyWithCapacity
        ], true),
        (#[
          ``Std.DHashMap.Raw.emptyWithCapacity
        ], true),
        (#[
          ``Std.DHashMap.Internal.Raw₀.insertIfNew,
          ``Std.DHashMap.Internal.Raw₀.emptyWithCapacity
        ], true),
        (#[
          ``Std.DHashMap.Internal.mkIdx
        ], true),
        (#[
          ``UInt64.toUSize
        ], true)
      ]
      let mut result : Simp.Result := { expr := insert }
      let mut stage := 0
      for (definitions, useGlobal) in groups do
        IO.eprintln s!"kernel_lrat_cnf_check: insertion simplification stage {stage}"
        stage := stage + 1
        result ← simplifyWithDefinitions
          result definitions useGlobal
      let rewriteResult
          (whole : Simp.Result) (source normalized proof : Expr) :
          MetaM Simp.Result := do
        let equality ← mkEq source normalized
        let proof ← mkExpectedTypeHint proof equality
        let mut rewriteTheorems : SimpTheoremsArray := #[]
        rewriteTheorems ← rewriteTheorems.addTheorem
          (.other `thermiteNormalizedMapSubexpression) proof
        let context ← Simp.mkContext
          { failIfUnchanged := true, maxSteps := 100_000 }
          (simpTheorems := rewriteTheorems)
        let (next, _) ← Lean.Meta.simp whole.expr context
        whole.mkEqTrans next
      for _ in List.range 4 do
        let some capacityCall := result.expr.find? fun expression =>
            match expression.getAppFn.constName? with
            | some name =>
                name.toString.contains
                  "numBucketsForCapacity"
            | none => false
          | break
        let value ← evalExpr Nat (mkConst ``Nat) capacityCall
        let normalized := toExpr value
        let equality ← mkEq capacityCall normalized
        let proof ← mkDecideProof equality
        result ← rewriteResult result capacityCall
          normalized proof
      for _ in List.range 8 do
        let some usizeCall := result.expr.find? fun expression =>
            expression.isAppOfArity ``USize.ofNat 1
          | break
        let normalized ← simplifyWithDefinitions
          { expr := usizeCall } #[``USize.ofNat] false
        if normalized.expr == usizeCall then
          break
        let proof ←
          match normalized.proof? with
          | some proof => pure proof
          | none => mkEqRefl usizeCall
        result ← rewriteResult result usizeCall
          normalized.expr proof
      for (functionName, arity) in #[
          (``Hashable.hash, 3),
          (``Std.DHashMap.Internal.scrambleHash, 1)
        ] do
        for _ in List.range 8 do
          let some call := result.expr.find? fun expression =>
              expression.isAppOfArity functionName arity
            | break
          let value ← evalExpr UInt64
            (mkConst ``UInt64) call
          let normalized := toExpr value
          let equality ← mkEq call normalized
          let proof ← mkDecideProof equality
          result ← rewriteResult result call normalized proof
      for expansionIndex in List.range 8 do
        let dependentMap :=
          mkProj ``Std.HashMap 0 result.expr
        let rawMap :=
          mkProj ``Std.DHashMap 0 dependentMap
        let rawMap ← whnf rawMap
        let rec collectExpansions
            (expression : Expr) (found : Array Expr) :
            MetaM (Array Expr) := do
          if expression.hasLooseBVars then
            return found
          if ← isProof expression then
            return found
          let found :=
            if expression.isAppOfArity
                ``Std.DHashMap.Internal.Raw₀.expandIfNecessary 5 then
              found.push expression
            else
              found
          match expression with
          | .app function argument => do
              let found ← collectExpansions function found
              collectExpansions argument found
          | .lam _ type body _ =>
              return ← collectExpansions body
                (← collectExpansions type found)
          | .forallE _ type body _ =>
              return ← collectExpansions body
                (← collectExpansions type found)
          | .letE _ type value body _ =>
              return ← collectExpansions body
                (← collectExpansions value
                  (← collectExpansions type found))
          | .mdata _ body =>
              collectExpansions body found
          | .proj _ _ projected =>
              collectExpansions projected found
          | _ => pure found
        let expansions ← collectExpansions rawMap #[]
        if expansions.isEmpty then
          break
        let mut expansion := expansions[0]!
        let mut largestSize := 0
        for candidate in expansions do
          let candidateMap := candidate.getAppArgs.back!
          let candidateRaw ← mkAppM ``Subtype.val #[candidateMap]
          let candidateSize :=
            mkProj ``Std.DHashMap.Raw 0 candidateRaw
          let reduced ←
            Lean.Meta.Tactic.Cbv.cbvEntry candidateSize
          let sizeExpression ←
            match reduced with
            | .rfl _ => withTransparency .all do
                whnf candidateSize
            | .step expression _ _ => pure expression
          let size := sizeExpression.rawNatLit?.getD 0
          if largestSize ≤ size then
            largestSize := size
            expansion := candidate
        IO.eprintln s!"kernel_lrat_cnf_check: normalizing map expansion {expansionIndex}"
        let map := expansion.getAppArgs.back!
        let rawMap ← mkAppM ``Subtype.val #[map]
        let mapSize :=
          mkProj ``Std.DHashMap.Raw 0 rawMap
        let buckets :=
          mkProj ``Std.DHashMap.Raw 1 rawMap
        let bucketCount ← mkAppM ``Array.size #[buckets]
        let scaled :=
          mkApp2 (mkConst ``Nat.mul) mapSize (toExpr 4)
        let threshold :=
          mkApp2 (mkConst ``Nat.div) scaled (toExpr 3)
        let capacityTarget :=
          mkApp2 (mkConst ``Nat.le) threshold bucketCount
        let capacityProofGoal ←
          mkFreshExprMVar (some capacityTarget)
        proveClosedProposition capacityProofGoal.mvarId! #[] 32
        let capacityProof ←
          instantiateMVars capacityProofGoal
        unless !capacityProof.hasExprMVar do
          throwError
            "kernel reconstruction left map capacity unresolved"
        Lean.Meta.checkWithKernel capacityProof
        let expansionProof ← mkAppM
          ``Thermite.PropReconstruct.rawExpandIfNecessary_eq_self
          #[map, capacityProof]
        result ← rewriteResult result expansion map
          expansionProof
      result ← simplifyWithDefinitions result #[] true
      pure result
    let isHashInsert (expression : Expr) : Bool :=
      expression.isAppOfArity
        ``Std.HashMap.insertIfNew 7
    let rec findComputational
        (expression : Expr) (predicate : Expr → Bool) :
        MetaM (Option Expr) := do
      if expression.hasLooseBVars then
        return none
      if ← isProof expression then
        return none
      if predicate expression then
        return some expression
      match expression with
      | .app function argument =>
          if let some found ←
              findComputational argument predicate then
            return some found
          findComputational function predicate
      | .lam _ type body _ =>
          if let some found ←
              findComputational type predicate then
            return some found
          findComputational body predicate
      | .forallE _ type body _ =>
          if let some found ←
              findComputational type predicate then
            return some found
          findComputational body predicate
      | .letE _ type value body _ =>
          if let some found ←
              findComputational type predicate then
            return some found
          if let some found ←
              findComputational value predicate then
            return some found
          findComputational body predicate
      | .mdata _ body =>
          findComputational body predicate
      | .proj _ _ projected =>
          findComputational projected predicate
      | _ => pure none
    let rec findInnermostComputational
        (expression : Expr) (predicate : Expr → Bool) :
        MetaM (Option Expr) := do
      if expression.hasLooseBVars then
        return none
      if ← isProof expression then
        return none
      let childResult ←
        match expression with
        | .app function argument => do
            if let some found ←
                findInnermostComputational argument predicate then
              return some found
            findInnermostComputational function predicate
        | .lam _ type body _ => do
            if let some found ←
                findInnermostComputational type predicate then
              return some found
            findInnermostComputational body predicate
        | .forallE _ type body _ => do
            if let some found ←
                findInnermostComputational type predicate then
              return some found
            findInnermostComputational body predicate
        | .letE _ type value body _ => do
            if let some found ←
                findInnermostComputational type predicate then
              return some found
            if let some found ←
                findInnermostComputational value predicate then
              return some found
            findInnermostComputational body predicate
        | .mdata _ body =>
            findInnermostComputational body predicate
        | .proj _ _ projected =>
            findInnermostComputational projected predicate
        | _ => pure none
      if childResult.isSome then
        return childResult
      if predicate expression then
        return some expression
      pure none
    let rec findInnermostComputationalM
        (expression : Expr)
        (predicate : Expr → MetaM Bool) :
        MetaM (Option Expr) := do
      if expression.hasLooseBVars then
        return none
      if ← isProof expression then
        return none
      let childResult ←
        match expression with
        | .app function argument => do
            if let some found ←
                findInnermostComputationalM argument predicate then
              return some found
            findInnermostComputationalM function predicate
        | .lam _ type body _ => do
            if let some found ←
                findInnermostComputationalM type predicate then
              return some found
            findInnermostComputationalM body predicate
        | .forallE _ type body _ => do
            if let some found ←
                findInnermostComputationalM type predicate then
              return some found
            findInnermostComputationalM body predicate
        | .letE _ type value body _ => do
            if let some found ←
                findInnermostComputationalM type predicate then
              return some found
            if let some found ←
                findInnermostComputationalM value predicate then
              return some found
            findInnermostComputationalM body predicate
        | .mdata _ body =>
            findInnermostComputationalM body predicate
        | .proj _ _ projected =>
            findInnermostComputationalM projected predicate
        | _ => pure none
      if childResult.isSome then
        return childResult
      if ← predicate expression then
        return some expression
      pure none
    let mut simplified : Simp.Result := { expr := caseLeft }
    let mut normalizedInsertionCount := 0
    for insertionIndex in List.range 64 do
      let some insert ← findComputational simplified.expr fun expression =>
          isHashInsert expression &&
            !(expression.getAppArgs.any fun argument =>
              (argument.find? isHashInsert).isSome)
        | break
      IO.eprintln s!"kernel_lrat_cnf_check: normalizing map insertion {insertionIndex}"
      normalizedInsertionCount := normalizedInsertionCount + 1
      let normalized ← normalizeInsert insert
      if normalized.expr == insert then
        throwError m!"kernel reconstruction could not normalize a map insertion:\n\
          {insert}"
      let rawNormalizationProof ←
        match normalized.proof? with
        | some proof => pure proof
        | none => mkEqRefl insert
      let equality ← mkEq insert normalized.expr
      let normalizationProof ←
        mkExpectedTypeHint rawNormalizationProof equality
      let mut rewriteTheorems : SimpTheoremsArray := #[]
      rewriteTheorems ← rewriteTheorems.addTheorem
        (.other `thermiteNormalizedMapInsertion)
        normalizationProof
      let rewriteContext ← Simp.mkContext
        { failIfUnchanged := true, maxSteps := 100_000 }
        (simpTheorems := rewriteTheorems)
      let (next, _) ←
        Lean.Meta.simp simplified.expr rewriteContext
      simplified ← simplified.mkEqTrans next
    simplified ← simplifyWithDefinitions simplified
      #[``Thermite.PropReconstruct.defaultClauseOfHashMap] true
    let finalGroups : Array (Array Name) := #[
      #[``Std.HashMap.toList],
      #[``Std.DHashMap.Const.toList],
      #[``Std.DHashMap.Raw.Const.toList],
      #[``Std.DHashMap.Raw.Internal.foldRev]
    ]
    let mut finalStage := 0
    for definitions in finalGroups do
      IO.eprintln s!"kernel_lrat_cnf_check: map traversal stage {finalStage}"
      finalStage := finalStage + 1
      simplified ← simplifyWithDefinitions
        simplified definitions true
    let currentClauseValue
        (whole : Expr) : MetaM (Option Expr) := do
      let some clauseConstructor :=
          whole.findExt? fun expression =>
            if expression.getAppFn.isConstOf
                ``LRAT.Internal.DefaultClause.mk then
              .found
            else
              .visit
        | return some whole
      let arguments := clauseConstructor.getAppArgs
      if arguments.size < 2 then
        return some whole
      pure (some arguments[1]!)
    let rewriteClauseSubexpression
        (whole : Simp.Result)
        (source normalized proof : Expr) :
        MetaM Simp.Result := do
      let equality ← mkEq source normalized
      let proof ← mkExpectedTypeHint proof equality
      let mut rewriteTheorems : SimpTheoremsArray := #[]
      rewriteTheorems ← rewriteTheorems.addTheorem
        (.other `thermiteNormalizedClauseSubexpression) proof
      let context ← Simp.mkContext
        { failIfUnchanged := true, maxSteps := 100_000 }
        (simpTheorems := rewriteTheorems)
      let (next, _) ← Lean.Meta.simp whole.expr context
      whole.mkEqTrans next
    for (functionName, arity) in #[
        (``Hashable.hash, 3),
        (``Std.DHashMap.Internal.scrambleHash, 1)
      ] do
      for _ in List.range 8 do
        let some clauseValue ← currentClauseValue simplified.expr
          | break
        let some call ← findComputational clauseValue fun expression =>
            expression.isAppOfArity functionName arity
          | break
        let value ← evalExpr UInt64 (mkConst ``UInt64) call
        let normalized := toExpr value
        let equality ← mkEq call normalized
        let proof ← mkDecideProof equality
        simplified ← rewriteClauseSubexpression
          simplified call normalized proof
    for _ in List.range 16 do
      let some clauseValue ← currentClauseValue simplified.expr
        | break
      let some call ← findComputational clauseValue fun expression =>
          match expression.getAppFn.constName? with
          | some name =>
              name.toString.contains "nextPowerOfTwo"
          | none => false
        | break
      let value ← evalExpr Nat (mkConst ``Nat) call
      let normalized := toExpr value
      let equality ← mkEq call normalized
      let proof ← mkDecideProof equality
      simplified ← rewriteClauseSubexpression
        simplified call normalized proof
    for maskIndex in List.range 16 do
      let some clauseValue ← currentClauseValue simplified.expr
        | break
      let some call ← findInnermostComputational clauseValue fun expression =>
          expression.isAppOfArity ``USize.toNat 1 &&
            expression.getAppArgs.back!.getAppFn.isConstOf
              ``HSub.hSub
        | break
      IO.eprintln "kernel_lrat_cnf_check: normalizing bucket mask"
      let subtraction := call.getAppArgs.back!
      let subtractionArguments := subtraction.getAppArgs
      unless subtractionArguments.size ≥ 2 do
        throwError m!"kernel reconstruction found a malformed USize mask:\n\
          {call}"
      let convertedCount :=
        subtractionArguments[subtractionArguments.size - 2]!
      let convertedArguments := convertedCount.getAppArgs
      let bucketCount ←
        if convertedCount.getAppFn.isConstOf ``OfNat.ofNat then
          unless convertedArguments.size ≥ 2 do
            throwError m!"kernel reconstruction found a malformed bucket count:\n\
              {convertedCount}"
          pure convertedArguments[1]!
        else if convertedCount.getAppFn.isConstOf ``USize.ofNat then
          unless !convertedArguments.isEmpty do
            throwError m!"kernel reconstruction found a malformed bucket count:\n\
              {convertedCount}"
          pure convertedArguments.back!
        else
          throwError m!"kernel reconstruction found a non-canonical bucket count:\n\
            {convertedCount}"
      let powerCall? ← findComputational call fun expression =>
        expression.getAppFn.isConstOf ``Nat.nextPowerOfTwo
      let bucketValue? ← getNatValue? bucketCount
      if powerCall?.isNone && bucketValue?.isNone then
        let mut normalizedBucket : Simp.Result := {
          expr := bucketCount
        }
        for _ in List.range 8 do
          let previous := normalizedBucket.expr
          let reduced ←
            Lean.Meta.Tactic.Cbv.cbvEntry previous
          match reduced with
          | .rfl _ => pure ()
          | .step expression proof _ =>
              normalizedBucket ← normalizedBucket.mkEqTrans {
                expr := expression
                proof? := some proof
              }
          normalizedBucket ← simplifyWithDefinitions
            normalizedBucket #[] true
          if normalizedBucket.expr == previous then
            break
        unless (← getNatValue? normalizedBucket.expr).isSome do
          throwError m!"kernel reconstruction could not normalize a bucket count:\n\
            {bucketCount}\n\nresidual:\n{normalizedBucket.expr}"
        let bucketProof ←
          match normalizedBucket.proof? with
          | some proof => pure proof
          | none => mkEqRefl bucketCount
        simplified ← rewriteClauseSubexpression
          simplified bucketCount normalizedBucket.expr bucketProof
        continue
      let powerProof? ←
        match powerCall? with
        | none => pure none
        | some powerCall => do
            let powerValue ←
              evalExpr Nat (mkConst ``Nat) powerCall
            let normalizedPower := toExpr powerValue
            let powerEquality ←
              mkEq powerCall normalizedPower
            pure (some (← mkDecideProof powerEquality))
      let mut localTheorems : SimpTheorems := {}
      localTheorems ← localTheorems.addDeclToUnfold
        ``USize.size
      let mut theoremSets : SimpTheoremsArray :=
        #[← getSimpTheorems, localTheorems]
      theoremSets ← addWidthBound theoremSets
      if let some powerProof := powerProof? then
        theoremSets ← theoremSets.addTheorem
          (.other `thermiteConcreteBucketCount) powerProof
      let context ← Simp.mkContext
        { failIfUnchanged := true, maxSteps := 100_000 }
        (simpTheorems := theoremSets)
      let proveConcrete (proposition : Expr) : MetaM Expr := do
        let (normalized, _) ← Lean.Meta.simp proposition context
        let normalizedProof ← mkDecideProof normalized.expr
        normalized.mkEqMPR normalizedProof
      let positiveTarget :=
        mkApp2 (mkConst ``Nat.lt) (toExpr 0) bucketCount
      let fitsTarget :=
        mkApp2 (mkConst ``Nat.lt) bucketCount
          (mkConst ``USize.size)
      let positiveProof ← proveConcrete positiveTarget
      let fitsProof ← proveConcrete fitsTarget
      let maskValue :=
        mkApp2 (mkConst ``Nat.sub) bucketCount (toExpr 1)
      let rawMaskProof ← mkAppM
        ``Thermite.PropReconstruct.usizeOfNatSubOneToNat
        #[positiveProof, fitsProof]
      let maskEquality ← mkEq call maskValue
      let maskProof ←
        mkExpectedTypeHint rawMaskProof maskEquality
      let (reducedMask, _) ← Lean.Meta.simp maskValue context
      let normalized ←
        ({ expr := maskValue, proof? := some maskProof } :
          Simp.Result).mkEqTrans reducedMask
      if maskIndex == 0 then
        IO.eprintln s!"kernel_lrat_cnf_check: bucket mask {call} -> {normalized.expr}"
      let proof := normalized.proof?.getD maskProof
      simplified ← rewriteClauseSubexpression
        simplified call normalized.expr proof
    for _ in List.range 64 do
      let some clauseValue ← currentClauseValue simplified.expr
        | break
      let some call ← findComputational clauseValue fun expression =>
          expression.isAppOfArity ``HAnd.hAnd 6 &&
            expression.getAppArgs[2]!.isConstOf ``Nat
        | break
      let value ← evalExpr Nat (mkConst ``Nat) call
      let normalized := toExpr value
      let equality ← mkEq call normalized
      let proof ← mkDecideProof equality
      simplified ← rewriteClauseSubexpression
        simplified call normalized proof
    for _ in List.range 128 do
      let some clauseValue ← currentClauseValue simplified.expr
        | break
      let some call ← findInnermostComputationalM clauseValue
          fun expression => do
            if expression.isConstOf ``Bool.true ||
                expression.isConstOf ``Bool.false then
              return false
            let type ← whnf (← inferType expression)
            pure (type.isConstOf ``Bool)
        | break
      let mut normalized : Simp.Result := { expr := call }
      for _ in List.range 8 do
        let previous := normalized.expr
        let reduced ←
          Lean.Meta.Tactic.Cbv.cbvEntry previous
        match reduced with
        | .rfl _ => pure ()
        | .step expression proof _ =>
            normalized ← normalized.mkEqTrans {
              expr := expression
              proof? := some proof
            }
        normalized ← simplifyWithDefinitions
          normalized #[] true
        if normalized.expr == previous then
          break
      unless normalized.expr.isConstOf ``Bool.true ||
          normalized.expr.isConstOf ``Bool.false do
        throwError m!"kernel reconstruction could not normalize a Boolean \
          map computation:\n{call}\n\nresidual:\n{normalized.expr}"
      let proof ←
        match normalized.proof? with
        | some proof => pure proof
        | none => mkEqRefl call
      simplified ← rewriteClauseSubexpression
        simplified call normalized.expr proof
    for _ in List.range 32 do
      let some clauseValue ← currentClauseValue simplified.expr
        | break
      let some call ← findInnermostComputationalM clauseValue
          fun expression => do
            if expression.getAppFn.isConstOf ``Option.some ||
                expression.getAppFn.isConstOf ``Option.none then
              return false
            let type ← whnf (← inferType expression)
            let arguments := type.getAppArgs
            pure (type.getAppFn.isConstOf ``Option &&
              arguments.size == 1 &&
              arguments[0]!.isConstOf ``Bool)
        | break
      let mut normalized : Simp.Result := { expr := call }
      for _ in List.range 8 do
        let previous := normalized.expr
        let reduced ←
          Lean.Meta.Tactic.Cbv.cbvEntry previous
        match reduced with
        | .rfl _ => pure ()
        | .step expression proof _ =>
            normalized ← normalized.mkEqTrans {
              expr := expression
              proof? := some proof
            }
        normalized ← simplifyWithDefinitions
          normalized #[] true
        if normalized.expr == previous then
          break
      unless normalized.expr.getAppFn.isConstOf ``Option.some ||
          normalized.expr.getAppFn.isConstOf ``Option.none do
        throwError m!"kernel reconstruction could not normalize a map lookup:\n\
          {call}\n\nresidual:\n{normalized.expr}"
      let proof ←
        match normalized.proof? with
        | some proof => pure proof
        | none => mkEqRefl call
      simplified ← rewriteClauseSubexpression
        simplified call normalized.expr proof
    simplified ← simplifyWithDefinitions simplified #[] true
    for definitions in finalGroups do
      simplified ← simplifyWithDefinitions
        simplified definitions true
    unless simplified.expr.getAppFn.isConstOf ``Option.some &&
        caseRight.getAppFn.isConstOf ``Option.some do
      throwError m!"kernel reconstruction expected concrete clauses:\n\
        {simplified.expr}\n\nexpected:\n{caseRight}"
    let sourceClause := simplified.expr.getAppArgs.back!
    let expectedClause := caseRight.getAppArgs.back!
    unless sourceClause.getAppFn.isConstOf
          ``Thermite.PropReconstruct.defaultClauseOfNodupKeys &&
        expectedClause.getAppFn.isConstOf
          ``Thermite.PropReconstruct.defaultClauseOfNodupKeys do
      throwError m!"kernel reconstruction expected reified clause wrappers:\n\
        {sourceClause}\n\nexpected:\n{expectedClause}"
    let sourceArguments := sourceClause.getAppArgs
    let expectedArguments := expectedClause.getAppArgs
    unless sourceArguments.size ≥ 2 &&
        expectedArguments.size ≥ 2 do
      throwError
        "kernel reconstruction found malformed clause wrappers"
    let sourceLiterals :=
      sourceArguments[sourceArguments.size - 2]!
    let sourceNodup := sourceArguments.back!
    let expectedLiterals :=
      expectedArguments[expectedArguments.size - 2]!
    let expectedNodup := expectedArguments.back!
    let literalTarget ← mkEq sourceLiterals expectedLiterals
    let mut normalizedLiterals : Simp.Result := {
      expr := sourceLiterals
    }
    for _ in List.range 8 do
      let previous := normalizedLiterals.expr
      let reduced ←
        Lean.Meta.Tactic.Cbv.cbvEntry previous
      match reduced with
      | .rfl _ => pure ()
      | .step expression proof _ =>
          normalizedLiterals ← normalizedLiterals.mkEqTrans {
            expr := expression
            proof? := some proof
          }
      normalizedLiterals ← simplifyWithDefinitions
        normalizedLiterals #[] true
      if normalizedLiterals.expr == previous then
        break
    unless ← isDefEq normalizedLiterals.expr expectedLiterals do
      throwError m!"kernel reconstruction left a clause-list residual:\n\
        {normalizedLiterals.expr}\n\nexpected:\n{expectedLiterals}"
    let rawLiteralProof ←
      match normalizedLiterals.proof? with
      | some proof => pure proof
      | none => mkEqRefl sourceLiterals
    let literalProof ←
      mkExpectedTypeHint rawLiteralProof literalTarget
    let optionProof ← mkAppM
      ``Thermite.PropReconstruct.someDefaultClauseOfNodupKeys_eq
      #[sourceLiterals, expectedLiterals,
        sourceNodup, expectedNodup, literalProof]
    let proof ←
      match simplified.proof? with
      | some prefixProof => mkEqTrans prefixProof optionProof
      | none => pure optionProof
    caseGoal.assign
      (← mkExpectedTypeHint proof caseTarget)
  let makeCase (width : Nat) : MetaM Expr := do
    let widthTarget ←
      mkEq (mkConst ``System.Platform.numBits) (toExpr width)
    withLocalDeclD `platformWidth widthTarget fun widthBound => do
      let caseProof ← mkFreshExprMVar (some target)
      IO.eprintln s!"kernel_lrat_cnf_check: checking {width}-bit platform case"
      solveCase caseProof.mvarId! widthBound
      let caseProof ← instantiateMVars caseProof
      mkLambdaFVars #[widthBound] caseProof
  IO.eprintln "kernel_lrat_cnf_check: splitting platform width"
  let case32 ← makeCase 32
  let case64 ← makeCase 64
  let proof ← mkAppM
    ``Thermite.PropReconstruct.of_platform_numBits_cases
    #[case32, case64]
  goal.assign (← mkExpectedTypeHint proof target)
  /-
  let definitionGroups : Array (Array Name) := #[
    #[
      ``Std.HashMap.toList,
      ``Std.HashMap.insertIfNew,
      ``Std.HashMap.emptyWithCapacity
    ],
    #[
      ``Std.DHashMap.Const.toList,
      ``Std.DHashMap.insertIfNew,
      ``Std.DHashMap.emptyWithCapacity
    ],
    #[
      ``Std.DHashMap.Raw.Const.toList,
      ``Std.DHashMap.Raw.emptyWithCapacity
    ],
    #[
      ``Std.DHashMap.Internal.Raw₀.insertIfNew,
      ``Std.DHashMap.Internal.Raw₀.emptyWithCapacity,
      ``Std.DHashMap.Internal.mkIdx
    ],
    #[
      ``UInt64.toUSize,
      ``USize.ofNat,
      ``System.Platform.numBits
    ]
  ]
  let mut simplified : Simp.Result := { expr := left }
  for definitions in definitionGroups do
    let mut theorems : SimpTheorems := {}
    for definition in definitions do
      theorems ← theorems.addDeclToUnfold definition
    let context ← Simp.mkContext
      { failIfUnchanged := false, maxSteps := 100_000 }
      (simpTheorems := #[theorems])
    let (next, _) ← Lean.Meta.simp simplified.expr context
    simplified ← simplified.mkEqTrans next
    if simplified.expr.find? (fun expression =>
        expression.isAppOfArity
          ``System.Platform.getNumBits 1) |>.isSome then
      break
  let afterExposure := simplified.expr
  if ← isDefEq afterExposure right then
    let proof ←
      match simplified.proof? with
      | some proof => pure proof
      | none => mkEqRefl left
    goal.assign (← mkExpectedTypeHint proof target)
    return
  let residualTarget ← mkEq afterExposure right
  let some platformCall := residualTarget.find? fun expression =>
      expression.isAppOfArity
        ``System.Platform.getNumBits 1
    | throwError m!"kernel reconstruction did not expose the platform value:\n\
        {afterExposure}\n\nexpected:\n{right}"
  let platformType ← inferType platformCall
  withLocalDeclD `platformValue platformType fun platformValue => do
    let replacedTarget := residualTarget.replace fun expression =>
      if expression == platformCall then
        some platformValue
      else
        none
    let abstractedTarget :=
      replacedTarget.abstract #[platformValue]
    unless abstractedTarget.hasLooseBVar 0 do
      throwError
        "kernel reconstruction could not abstract the platform value"
    let motive :=
      mkLambda `platform .default platformType
        abstractedTarget
    let natType := mkConst ``Nat
    let widthVariable := mkBVar 0
    let equality (lhs rhs : Expr) :=
      mkApp3 (mkConst ``Eq [1]) natType lhs rhs
    let platformPredicate :=
      mkLambda `width .default natType <|
        mkApp2 (mkConst ``Or)
          (equality widthVariable (toExpr 32))
          (equality widthVariable (toExpr 64))
    let width32 := toExpr 32
    let width64 := toExpr 64
    let proof32 :=
      mkApp3 (mkConst ``Or.inl)
        (equality width32 width32)
        (equality width32 width64)
        (← mkEqRefl width32)
    let proof64 :=
      mkApp3 (mkConst ``Or.inr)
        (equality width64 width32)
        (equality width64 width64)
        (← mkEqRefl width64)
    let platform32 ← mkAppOptM ``Subtype.mk
      #[some natType, some platformPredicate,
        some width32, some proof32]
    let platform64 ← mkAppOptM ``Subtype.mk
      #[some natType, some platformPredicate,
        some width64, some proof64]
    let case32Type ← whnf <| mkApp motive platform32
    let case64Type ← whnf <| mkApp motive platform64
    let case32 ← mkFreshExprMVar case32Type
    let case64 ← mkFreshExprMVar case64Type
    let residualProof ← mkAppM
      ``Thermite.PropReconstruct.of_getNumBits_cases
      #[motive, case32, case64]
    IO.eprintln "kernel_lrat_cnf_check: checking 32-bit platform case"
    cbvReconstructionGoal case32.mvarId!
    IO.eprintln "kernel_lrat_cnf_check: checking 64-bit platform case"
    cbvReconstructionGoal case64.mvarId!
    let residualProof ← instantiateMVars residualProof
    let proof ←
      match simplified.proof? with
      | some prefixProof => mkEqTrans prefixProof residualProof
      | none => pure residualProof
    goal.assign (← mkExpectedTypeHint proof target)
  -/

private unsafe def simpPlatformReconstructionGoal
    (goal : MVarId) (constants : Array Name)
    (facts : Array Expr) (directClause : Bool := false) :
    MetaM Unit := do
  if directClause then
    cbvReconstructionGoal goal
    return
  let target ← goal.getType
  let some (_, left, right) := target.eq?
    | throwError
        "platform kernel reconstruction expects an equality"
  let mut conversionTheorems : SimpTheorems := {}
  conversionTheorems ← conversionTheorems.addDeclToUnfold
    ``LRAT.Internal.CNF.Clause.convertLRAT'
  let conversionContext ← Simp.mkContext
    { failIfUnchanged := false, maxSteps := 100_000 }
    (simpTheorems := #[conversionTheorems])
  let (conversionResult, _) ←
    Lean.Meta.simp left conversionContext
  let mut simplified : Simp.Result := {
    expr := left
  }
  simplified ← simplified.mkEqTrans conversionResult
  let mut listFoldTheorems : SimpTheorems := {}
  listFoldTheorems ← listFoldTheorems.addConst
    ``Thermite.PropReconstruct.defaultClauseOfArray_toArray
  let listFoldContext ← Simp.mkContext
    { failIfUnchanged := true, maxSteps := 100_000 }
    (simpTheorems := #[listFoldTheorems])
  let (listFoldResult, _) ←
    Lean.Meta.simp simplified.expr listFoldContext
  simplified ← simplified.mkEqTrans listFoldResult
  let mut listFoldEvaluationTheorems : SimpTheorems := {}
  listFoldEvaluationTheorems ←
    listFoldEvaluationTheorems.addDeclToUnfold ``List.foldl
  let listFoldEvaluationContext ← Simp.mkContext
    { failIfUnchanged := true, maxSteps := 100_000 }
    (simpTheorems := #[listFoldEvaluationTheorems])
  let (listFoldEvaluationResult, _) ←
    Lean.Meta.simp simplified.expr
      listFoldEvaluationContext
  simplified ← simplified.mkEqTrans
    listFoldEvaluationResult
  let isClauseFolder (expression : Expr) : Bool :=
    expression.isAppOfArity
      ``LRAT.Internal.DefaultClause.ofArray.folder 3
  for _ in List.range 64 do
    let some folder := simplified.expr.find? fun expression =>
        isClauseFolder expression &&
          !(expression.getAppArgs.any fun argument =>
            (argument.find? isClauseFolder).isSome)
      | break
    let arguments := folder.getAppArgs
    let folderProof ← mkAppM
      ``Thermite.PropReconstruct.defaultClauseFolder_eq
      #[arguments[arguments.size - 2]!,
        arguments[arguments.size - 1]!]
    let folderEquality ← inferType folderProof
    let some (_, folderLeft, _) := folderEquality.eq?
      | throwError "malformed clause-folder expansion theorem"
    unless ← isDefEq folderLeft folder do
      throwError
        "clause-folder expansion did not bind its source"
    let rewritten ← goal.rewrite simplified.expr folderProof
    unless rewritten.mvarIds.isEmpty do
      throwError
        "clause-folder expansion created unresolved goals"
    if rewritten.eNew == simplified.expr then
      throwError
        "clause-folder expansion did not change the expression"
    simplified ← simplified.mkEqTrans {
      expr := rewritten.eNew
      proof? := some rewritten.eqProof
    }
  let mut theoremSet : SimpTheorems := {}
  for theoremName in #[
      ``Thermite.PropReconstruct.array_foldl_nil,
      ``Thermite.PropReconstruct.array_foldl_one,
      ``Thermite.PropReconstruct.array_foldl_two,
      ``Thermite.PropReconstruct.array_foldl_three,
      ``Thermite.PropReconstruct.listToArray_foldl_nil,
      ``Thermite.PropReconstruct.listToArray_foldl_one,
      ``Thermite.PropReconstruct.listToArray_foldl_two,
      ``Thermite.PropReconstruct.listToArray_foldl_three
    ] do
    theoremSet ← theoremSet.addConst theoremName
  for constant in constants do
    theoremSet ← theoremSet.addDeclToUnfold constant
  let simpContext ← Simp.mkContext
    { failIfUnchanged := false, maxSteps := 1_000_000 }
    (simpTheorems := #[theoremSet])
  IO.eprintln "kernel_lrat_cnf_check: simplifying clause conversion"
  for _ in List.range 1 do
    let (next, _) ←
      Lean.Meta.simp simplified.expr simpContext
    simplified ← simplified.mkEqTrans next
  let mut hashMapTheorems : SimpTheorems := {}
  for theoremName in #[
      ``Std.HashMap.getThenInsertIfNew?_fst,
      ``Std.HashMap.getThenInsertIfNew?_snd,
      ``Std.HashMap.getElem?_emptyWithCapacity,
      ``Std.HashMap.getElem?_insertIfNew,
      ``Std.HashMap.not_mem_emptyWithCapacity,
      ``Std.HashMap.toList_emptyWithCapacity
    ] do
    hashMapTheorems ← hashMapTheorems.addConst theoremName
  let mut hashMapTheoremSets : SimpTheoremsArray :=
    #[hashMapTheorems]
  for fact in facts do
    hashMapTheoremSets ← hashMapTheoremSets.addTheorem
      (.other `thermiteConcreteClauseFact) fact
  let hashMapContext ← Simp.mkContext
    { failIfUnchanged := false, maxSteps := 100_000 }
    (simpTheorems := hashMapTheoremSets)
  let (hashMapSimplified, _) ←
    Lean.Meta.simp simplified.expr hashMapContext
  simplified ← simplified.mkEqTrans hashMapSimplified
  for _ in List.range 3 do
    let (unfolded, _) ←
      Lean.Meta.simp simplified.expr simpContext
    simplified ← simplified.mkEqTrans unfolded
    let (apiSimplified, _) ←
      Lean.Meta.simp simplified.expr hashMapContext
    simplified ← simplified.mkEqTrans apiSimplified
  for _ in List.range 8 do
    let some comparison := simplified.expr.find? fun expression =>
        expression.getAppFn.isConstOf ``BEq.beq
      | break
    let value ← evalExpr Bool (mkConst ``Bool)
      comparison
    let comparisonTarget ← mkEq comparison (toExpr value)
    let comparisonProof ←
      mkDecideProof comparisonTarget
    let mut comparisonTheorems : SimpTheoremsArray :=
      #[← getSimpTheorems]
    comparisonTheorems ← comparisonTheorems.addTheorem
      (.other `thermiteConcreteComparison)
      comparisonProof
    let comparisonContext ← Simp.mkContext
      { failIfUnchanged := false, maxSteps := 100_000 }
      (simpTheorems := comparisonTheorems)
    let (comparisonSimplified, _) ←
      Lean.Meta.simp simplified.expr comparisonContext
    simplified ← simplified.mkEqTrans
      comparisonSimplified
  let mut logicalTheorems : SimpTheorems := {}
  logicalTheorems ← logicalTheorems.addConst
    ``Subtype.ext_iff
  logicalTheorems ← logicalTheorems.addDeclToUnfold
    ``LRAT.Internal.PosFin
  let mut logicalTheoremSets : SimpTheoremsArray :=
    #[← getSimpTheorems, logicalTheorems]
  for fact in facts do
    logicalTheoremSets ← logicalTheoremSets.addTheorem
      (.other `thermiteConcreteClauseFact) fact
  let defaultContext ← Simp.mkContext
    { failIfUnchanged := false, maxSteps := 100_000 }
    (simpTheorems := logicalTheoremSets)
  for _ in List.range 3 do
    let (defaultSimplified, _) ←
      Lean.Meta.simp simplified.expr defaultContext
    simplified ← simplified.mkEqTrans defaultSimplified
  let residualTarget ← mkEq simplified.expr right
  let residualProof ← mkFreshExprMVar (some residualTarget)
  cbvPlatformReconstructionGoal residualProof.mvarId!
  let residualProof ← instantiateMVars residualProof
  let proof ←
    match simplified.proof? with
    | none => pure residualProof
    | some simplification =>
        mkEqTrans simplification residualProof
  goal.assign (← mkExpectedTypeHint proof target)

@[tactic kernelBoolCheck]
unsafe def evalKernelBoolCheck : Tactic := fun
  | `(tactic| kernel_bool_check) => do
      liftMetaFinishingTactic fun goal => goal.withContext do
        let target ← goal.getType
        let some (_, left, right) := target.eq?
          | throwError
              "kernel_bool_check expects a Boolean equality"
        let boolType := mkConst ``Bool
        unless ← isDefEq (← inferType left) boolType do
          throwError
            "kernel_bool_check expects a Boolean left-hand side"
        let value ← evalExpr Bool boolType left
        unless value do
          throwError "kernel_bool_check evaluated the left-hand side to false"
        unless ← withTransparency .all <| isDefEq right (mkConst ``Bool.true) do
          throwError
            "kernel_bool_check expects the right-hand side to be true"
        unless ← withTransparency .all <| isDefEq left right do
          throwError
            "kernel_bool_check could not bind evaluation to kernel reduction"
        goal.assign (← mkEqRefl left)
  | _ => throwUnsupportedSyntax

private unsafe def reconstructLratCheck
    (formulaExpr certificateExpr expected : Expr)
    (formulaDef certificateDef cnfDef : Name) : MetaM Expr := do
  IO.eprintln "kernel_lrat_check: reflecting inputs"
  let formulaType := mkApp (mkConst ``BoolExpr) (mkConst ``Nat)
  let certificateType :=
    mkApp (mkConst ``Array [.zero]) (mkConst ``LRAT.IntAction)
  let formula ← evalExpr (BoolExpr Nat) formulaType formulaExpr
  let formula := cloneBoolExpr formula
  let certificate ←
    evalExpr (Array LRAT.IntAction) certificateType certificateExpr
  IO.eprintln "kernel_lrat_check: building CNF"
  let cnf := tseitinCnf formula
  let cnfType := mkApp (mkConst ``CNF [.zero]) (mkConst ``Nat)
  let cnfValue :=
    mkApp2 (mkConst ``CNF.mk [.zero])
      (mkConst ``Nat) (toExpr cnf.clauses)
  addAuxDecl formulaDef (toExpr formula) formulaType
  addAuxDecl certificateDef (toExpr certificate) certificateType
  addAuxDecl cnfDef cnfValue cnfType
  IO.eprintln "kernel_lrat_check: checking LRAT"
  let compactVerification ← mkAppM
    ``LRAT.check #[mkConst certificateDef, mkConst cnfDef]
  let compactTarget :=
    mkApp3 (mkConst ``Eq [1]) (mkConst ``Bool)
      compactVerification expected
  let compactTarget ← Lean.Meta.Grind.foldProjs
    (← Lean.Meta.Sym.unfoldReducible compactTarget)
  let proof ← mkFreshExprMVar (some compactTarget)
  cbvReconstructionGoal proof.mvarId!
  IO.eprintln "kernel_lrat_check: binding verification"
  let proof ← instantiateMVars proof
  unless !proof.hasExprMVar do
    throwError "kernel LRAT evaluation left an unsolved equality"
  if proof.hasSorry then
    throwError "kernel LRAT evaluation produced a sorry-bearing proof"
  let sourceCnf :=
    mkApp
      (mkConst ``Thermite.PropReconstruct.tseitinCnf)
      formulaExpr
  unless ← withTransparency .all <|
      isDefEq sourceCnf (mkConst cnfDef) do
    throwError
      "recomputed propositional CNF did not bind to LRAT input"
  unless ← withTransparency .all <|
      isDefEq certificateExpr (mkConst certificateDef) do
    throwError
      "LRAT certificate did not bind to its checked auxiliary"
  let certificateBound ← mkEqRefl certificateExpr
  let cnfBound ← mkEqRefl sourceCnf
  let result ← mkAppM
    ``Thermite.PropReconstruct.verifyActions_eq_of_bound
    #[formulaExpr, certificateExpr, mkConst certificateDef,
      mkConst cnfDef, expected, certificateBound, cnfBound, proof]
  let result ← instantiateMVars result
  unless !result.hasExprMVar do
    throwError "kernel LRAT reconstruction left unresolved metavariables"
  if result.hasSorry then
    throwError "kernel LRAT reconstruction produced a sorry-bearing term"
  IO.eprintln "kernel_lrat_check: done"
  pure result

private def reifyPosFin (bound value : Nat) : MetaM Expr := do
  let natType := mkConst ``Nat
  let boundExpr := toExpr bound
  let valueExpr := toExpr value
  let variableExpr := mkBVar 0
  let natLt (left right : Expr) :=
    mkAppN (mkConst ``LT.lt [.zero])
      #[natType, mkConst ``instLTNat, left, right]
  let positive := natLt (toExpr 0) variableExpr
  let belowBound := natLt variableExpr boundExpr
  let predicate :=
    mkLambda `value .default natType <|
      mkApp2 (mkConst ``And) positive belowBound
  let positiveValue := natLt (toExpr 0) valueExpr
  let belowBoundValue := natLt valueExpr boundExpr
  let positiveProof ← mkDecideProof positiveValue
  let belowBoundProof ← mkDecideProof belowBoundValue
  let conjunctionProof ←
    mkAppM ``And.intro #[positiveProof, belowBoundProof]
  let membershipProof ←
    mkExpectedTypeHint conjunctionProof
      (mkApp predicate valueExpr)
  mkAppOptM ``Subtype.mk
    #[some natType, some predicate, some valueExpr,
      some membershipProof]

private def reifyDefaultClause {bound : Nat}
    (clause : CNF.Clause (LRAT.Internal.PosFin bound)) :
    MetaM Expr := do
  let boundExpr := toExpr bound
  let posFinType :=
    mkApp (mkConst ``LRAT.Internal.PosFin) boundExpr
  let literalType :=
    mkApp2 (mkConst ``Prod [.zero, .zero])
      posFinType (mkConst ``Bool)
  let literalExprs ← clause.mapM fun literal => do
    let variableExpr ← reifyPosFin bound literal.1.1
    pure <| mkAppN (mkConst ``Prod.mk [.zero, .zero])
      #[posFinType, mkConst ``Bool, variableExpr, toExpr literal.2]
  let literalList ← mkListLit literalType literalExprs
  let literalFst :=
    mkApp2 (mkConst ``Prod.fst [.zero, .zero])
      posFinType (mkConst ``Bool)
  let keys ← mkAppM ``List.map
    #[literalFst, literalList]
  let nodupTarget ← mkAppM ``List.Nodup #[keys]
  let nodupProof ← mkDecideProof nodupTarget
  mkAppM
    ``Thermite.PropReconstruct.defaultClauseOfNodupKeys
    #[literalList, nodupProof]

private def reifyAssignment : LRAT.Internal.Assignment → Expr
  | .pos => mkConst ``LRAT.Internal.Assignment.pos
  | .neg => mkConst ``LRAT.Internal.Assignment.neg
  | .both => mkConst ``LRAT.Internal.Assignment.both
  | .unassigned => mkConst ``LRAT.Internal.Assignment.unassigned

private def reifyPosFinClause {bound : Nat}
    (clause : CNF.Clause (LRAT.Internal.PosFin bound)) :
    MetaM Expr := do
  let boundExpr := toExpr bound
  let posFinType :=
    mkApp (mkConst ``LRAT.Internal.PosFin) boundExpr
  let literalType :=
    mkApp2 (mkConst ``Prod [.zero, .zero])
      posFinType (mkConst ``Bool)
  let literals ← clause.mapM fun literal => do
    let variableExpr ← reifyPosFin bound literal.1.1
    pure <| mkAppN (mkConst ``Prod.mk [.zero, .zero])
      #[posFinType, mkConst ``Bool, variableExpr,
        toExpr literal.2]
  mkListLit literalType literals

private def reifyPosFinCnf {bound : Nat}
    (cnf : CNF (LRAT.Internal.PosFin bound)) : MetaM Expr := do
  let boundExpr := toExpr bound
  let posFinType :=
    mkApp (mkConst ``LRAT.Internal.PosFin) boundExpr
  let literalType :=
    mkApp2 (mkConst ``Prod [.zero, .zero])
      posFinType (mkConst ``Bool)
  let clauseType :=
    mkApp (mkConst ``List [.zero]) literalType
  let clauses ← cnf.clauses.toList.mapM reifyPosFinClause
  let clausesExpr ← mkArrayLit clauseType clauses
  pure <| mkApp2 (mkConst ``CNF.mk [.zero])
    posFinType clausesExpr

private def reifyDefaultClauseArray {bound : Nat}
    (clauses :
      Array (Option (LRAT.Internal.DefaultClause bound))) :
    MetaM Expr := do
  let clauseType :=
    mkApp (mkConst ``LRAT.Internal.DefaultClause) (toExpr bound)
  let optionClauseType :=
    mkApp (mkConst ``Option [.zero]) clauseType
  let clauses ← clauses.toList.mapM fun clause? =>
    match clause? with
    | none => mkNone clauseType
    | some clause => do
        let clauseExpr ← reifyDefaultClause clause.clause
        mkSome clauseType clauseExpr
  mkArrayLit optionClauseType clauses

private unsafe def reifyDefaultFormula {bound : Nat}
    (formula : LRAT.Internal.DefaultFormula bound) : MetaM Expr := do
  let boundExpr := toExpr bound
  let clausesExpr ← reifyDefaultClauseArray formula.clauses
  let literalType :=
    mkApp2 (mkConst ``Prod [.zero, .zero])
      (mkApp (mkConst ``LRAT.Internal.PosFin) boundExpr)
      (mkConst ``Bool)
  let emptyLiterals ← mkArrayLit literalType []
  let assignments ← mkArrayLit
    (mkConst ``LRAT.Internal.Assignment)
    (formula.assignments.toList.map reifyAssignment)
  pure <| mkAppN (mkConst ``LRAT.Internal.DefaultFormula.mk)
    #[boundExpr, clausesExpr, emptyLiterals, emptyLiterals, assignments]

private unsafe def reconstructCnfLratCheck
    (cnfExpr certificateExpr expected : Expr)
    (certificateDef cnfDef liftedDef convertedDef internalDef : Name) :
    MetaM Expr := do
  IO.eprintln "kernel_lrat_cnf_check: reflecting inputs"
  let certificateType :=
    mkApp (mkConst ``Array [.zero]) (mkConst ``LRAT.IntAction)
  let cnfType := mkApp (mkConst ``CNF [.zero]) (mkConst ``Nat)
  let certificate ←
    evalExpr (Array LRAT.IntAction) certificateType certificateExpr
  let cnf ← evalExpr (CNF Nat) cnfType cnfExpr
  let cnfValue :=
    mkApp2 (mkConst ``CNF.mk [.zero])
      (mkConst ``Nat) (toExpr cnf.clauses)
  addAuxDecl certificateDef (toExpr certificate) certificateType
  addAuxDecl cnfDef cnfValue cnfType
  unless ← isDefEq expected (mkConst ``Bool.true) do
    throwError "split kernel CNF LRAT replay expects `true`"
  let numVars := cnf.numLiterals + 1
  let numVarsExpr := toExpr numVars
  let posFinType :=
    mkApp (mkConst ``LRAT.Internal.PosFin) numVarsExpr
  let liftedType :=
    mkApp (mkConst ``CNF [.zero]) posFinType
  let liftedSource ← mkAppM
    ``LRAT.Internal.CNF.lift #[mkConst cnfDef]
  let lifted ←
    evalExpr (CNF (LRAT.Internal.PosFin numVars))
      liftedType liftedSource
  IO.eprintln "kernel_lrat_cnf_check: reifying lifted CNF"
  let liftedValue ← reifyPosFinCnf lifted
  addKernelAuxDecl liftedDef liftedValue liftedType
  IO.eprintln "kernel_lrat_cnf_check: binding lifted CNF"
  let liftedTarget ← mkEq liftedSource (mkConst liftedDef)
  let liftedProof ← mkFreshExprMVar (some liftedTarget)
  cbvReconstructionGoal liftedProof.mvarId!
  let liftedProof ← instantiateMVars liftedProof
  unless !liftedProof.hasExprMVar do
    throwError "kernel CNF lift left an unsolved equality"
  if liftedProof.hasSorry then
    throwError "kernel CNF lift produced a sorry-bearing proof"
  let defaultClauseType :=
    mkApp (mkConst ``LRAT.Internal.DefaultClause) numVarsExpr
  let optionClauseType :=
    mkApp (mkConst ``Option [.zero]) defaultClauseType
  let convertedType :=
    mkApp (mkConst ``Array [.zero]) optionClauseType
  let convertedSource ← mkAppM
    ``LRAT.Internal.CNF.convertLRAT' #[mkConst liftedDef]
  let converted := LRAT.Internal.CNF.convertLRAT' lifted
  IO.eprintln "kernel_lrat_cnf_check: reifying LRAT clauses"
  let convertedValue ← reifyDefaultClauseArray converted
  addKernelAuxDecl convertedDef convertedValue convertedType
  IO.eprintln "kernel_lrat_cnf_check: binding LRAT clauses"
  let mut clauseProofs : Array Expr := #[]
  for clause in lifted.clauses do
    IO.eprintln s!"kernel_lrat_cnf_check: binding clause of size {clause.length}"
    let clauseExpr ← reifyPosFinClause clause
    let clauseSource ← mkAppM
      ``LRAT.Internal.CNF.Clause.convertLRAT' #[clauseExpr]
    let clauseResult :=
      LRAT.Internal.CNF.Clause.convertLRAT' clause
    if clause.length == 3 then
      IO.eprintln s!"kernel_lrat_cnf_check: ternary clause \
        {reprStr (clause.map fun literal => (literal.1.1, literal.2))}; \
        discarded={clauseResult.isNone}"
    let clauseResultExpr ←
      match clauseResult with
      | none => mkNone defaultClauseType
      | some result => do
          let resultExpr ← reifyDefaultClause result.clause
          mkSome defaultClauseType resultExpr
    let clauseTarget ← mkEq clauseSource clauseResultExpr
    let clauseProof ← mkFreshExprMVar (some clauseTarget)
    let mut clauseFacts : Array Expr := #[]
    for first in clause do
      for second in clause do
        let firstExpr ←
          reifyPosFin numVars first.1.1
        let secondExpr ←
          reifyPosFin numVars second.1.1
        let compared ← mkAppM ``BEq.beq
          #[firstExpr, secondExpr]
        if first.1.1 == second.1.1 then
          let equalityType ← mkEq firstExpr secondExpr
          clauseFacts := clauseFacts.push
            (← mkDecideProof equalityType)
          let comparisonType ←
            mkEq compared (mkConst ``Bool.true)
          clauseFacts := clauseFacts.push
            (← mkDecideProof comparisonType)
        else
          let factType :=
            mkNot (← mkEq firstExpr secondExpr)
          clauseFacts := clauseFacts.push
            (← mkDecideProof factType)
          let comparisonType ←
            mkEq compared (mkConst ``Bool.false)
          clauseFacts := clauseFacts.push
            (← mkDecideProof comparisonType)
    simpPlatformReconstructionGoal clauseProof.mvarId! #[
      ``LRAT.Internal.CNF.Clause.convertLRAT'
    ] clauseFacts (clause.isEmpty || clauseResult.isNone)
    let clauseProof ← instantiateMVars clauseProof
    unless !clauseProof.hasExprMVar do
      throwError
        "kernel LRAT clause conversion left an unsolved equality"
    if clauseProof.hasSorry then
      throwError
        "kernel LRAT clause conversion produced a sorry-bearing proof"
    clauseProofs := clauseProofs.push clauseProof
  let conversionTheoremsHead ←
    ({} : SimpTheorems).addDeclToUnfold
    ``LRAT.Internal.CNF.convertLRAT'
  let conversionTheoremsHead ←
    conversionTheoremsHead.addConst ``List.filterMap_toArray
  let conversionTheoremsHead ←
    conversionTheoremsHead.addDeclToUnfold ``List.filterMap
  let mut conversionTheorems : SimpTheoremsArray :=
    #[conversionTheoremsHead]
  let liftedValueTarget ←
    mkEq (mkConst liftedDef) liftedValue
  let liftedValueProof ← mkExpectedTypeHint
    (← mkEqRefl (mkConst liftedDef)) liftedValueTarget
  for proof in clauseProofs do
    conversionTheorems ← conversionTheorems.addTheorem
      (.other `thermiteClauseConversion) proof
  let conversionContext ← Simp.mkContext
    { failIfUnchanged := false, maxSteps := 1_000_000 }
    (simpTheorems := conversionTheorems)
  let convertedValueSource :=
    mkApp convertedSource.appFn! liftedValue
  let convertedSourceValueProof ←
    mkCongrArg convertedSource.appFn! liftedValueProof
  let (convertedResult, _) ←
    Lean.Meta.simp convertedValueSource conversionContext
  unless ← isDefEq convertedResult.expr (mkConst convertedDef) do
    throwError m!"kernel LRAT clause conversions did not bind the converted array:\n\
      {convertedResult.expr}\n\nexpected:\n{mkConst convertedDef}"
  let convertedProof ←
    match convertedResult.proof? with
    | some proof => mkEqTrans convertedSourceValueProof proof
    | none => pure convertedSourceValueProof
  let internalType :=
    mkApp (mkConst ``LRAT.Internal.DefaultFormula) numVarsExpr
  let noneClause ← mkNone defaultClauseType
  let clausePrefix ← mkArrayLit optionClauseType [noneClause]
  let prefixed ← mkAppM ``Array.append
    #[clausePrefix, mkConst convertedDef]
  let internalSource ← mkAppM
    ``LRAT.Internal.DefaultFormula.ofArray #[prefixed]
  let initialClauses :
      Array (Option (LRAT.Internal.DefaultClause numVars)) :=
    #[none]
  let internal :=
    LRAT.Internal.DefaultFormula.ofArray
      (initialClauses ++ converted)
  IO.eprintln "kernel_lrat_cnf_check: reifying converted CNF"
  let internalValue ← reifyDefaultFormula internal
  addKernelAuxDecl internalDef internalValue internalType
  IO.eprintln "kernel_lrat_cnf_check: binding converted CNF"
  let conversionTarget ←
    mkEq internalSource (mkConst internalDef)
  let conversionProof ← mkFreshExprMVar (some conversionTarget)
  cbvReconstructionGoal conversionProof.mvarId!
  let conversionProof ← instantiateMVars conversionProof
  unless !conversionProof.hasExprMVar do
    throwError "kernel CNF conversion left an unsolved equality"
  if conversionProof.hasSorry then
    throwError "kernel CNF conversion produced a sorry-bearing proof"
  IO.eprintln "kernel_lrat_cnf_check: checking LRAT actions"
  let checker ← mkAppM
    ``LRAT.Internal.compactLratChecker
      #[mkConst internalDef, mkConst certificateDef]
  let checkerTarget ←
    mkEq checker (mkConst ``LRAT.Internal.Result.success)
  let checkerProof ← mkFreshExprMVar (some checkerTarget)
  cbvReconstructionGoal checkerProof.mvarId!
  let checkerProof ← instantiateMVars checkerProof
  unless !checkerProof.hasExprMVar do
    throwError "kernel LRAT checker left an unsolved equality"
  if checkerProof.hasSorry then
    throwError "kernel LRAT checker produced a sorry-bearing proof"
  IO.eprintln "kernel_lrat_cnf_check: binding checked result"
  let checked ← mkAppM
    ``Thermite.PropReconstruct.lratCheck_eq_true_of_stages
      #[mkConst certificateDef, mkConst cnfDef,
        mkConst liftedDef, mkConst convertedDef,
        mkConst internalDef, liftedProof, convertedProof,
        conversionProof, checkerProof]
  let certificateTarget ←
    mkEq certificateExpr (mkConst certificateDef)
  let certificateBound ←
    if ← withTransparency .all <|
        isDefEq certificateExpr (mkConst certificateDef) then
      mkExpectedTypeHint
        (← mkEqRefl certificateExpr) certificateTarget
    else do
      let proof ← mkFreshExprMVar (some certificateTarget)
      cbvReconstructionGoal proof.mvarId! 4096
      instantiateMVars proof
  unless !certificateBound.hasExprMVar do
    throwError
      "LRAT certificate binding left an unsolved equality"
  if certificateBound.hasSorry then
    throwError
      "LRAT certificate binding produced a sorry-bearing proof"
  let cnfTarget ← mkEq cnfExpr (mkConst cnfDef)
  let cnfBound ←
    if ← withTransparency .all <|
        isDefEq cnfExpr (mkConst cnfDef) then
      mkExpectedTypeHint
        (← mkEqRefl cnfExpr) cnfTarget
    else do
      let proof ← mkFreshExprMVar (some cnfTarget)
      cbvReconstructionGoal proof.mvarId! 4096
      instantiateMVars proof
  unless !cnfBound.hasExprMVar do
    throwError "CNF binding left an unsolved equality"
  if cnfBound.hasSorry then
    throwError "CNF binding produced a sorry-bearing proof"
  let result ← mkAppM
    ``Thermite.PropReconstruct.lratCheck_eq_of_bound
    #[certificateExpr, mkConst certificateDef, cnfExpr,
      mkConst cnfDef, expected, certificateBound, cnfBound, checked]
  let result ← instantiateMVars result
  unless !result.hasExprMVar do
    throwError "kernel CNF LRAT reconstruction left unresolved metavariables"
  if result.hasSorry then
    throwError "kernel CNF LRAT reconstruction produced a sorry-bearing term"
  IO.eprintln "kernel_lrat_cnf_check: done"
  pure result

@[tactic kernelLratCnfCheck]
unsafe def evalKernelLratCnfCheck : Tactic := fun
  | `(tactic| kernel_lrat_cnf_check $cnfName:str with
      $certificateName:str) => do
      let certificateDef ← Lean.Elab.Term.mkAuxName `_cnf_cert_def
      let cnfDef ← Lean.Elab.Term.mkAuxName `_cnf_def
      let liftedDef ← Lean.Elab.Term.mkAuxName `_cnf_lifted_def
      let convertedDef ←
        Lean.Elab.Term.mkAuxName `_cnf_converted_def
      let internalDef ← Lean.Elab.Term.mkAuxName `_cnf_internal_def
      liftMetaFinishingTactic fun goal => goal.withContext do
        let target ← goal.getType
        let some (_, _, expected) := target.eq?
          | throwError
              "kernel_lrat_cnf_check expects a Boolean equality"
        let cnfExpr := mkConst cnfName.getString.toName
        let certificateExpr := mkConst certificateName.getString.toName
        let proof ← reconstructCnfLratCheck
          cnfExpr certificateExpr expected certificateDef cnfDef
            liftedDef convertedDef internalDef
        unless ← isDefEq (← inferType proof) target do
          throwError
            "kernel_lrat_cnf_check result does not match its goal"
        goal.assign proof
  | _ => throwUnsupportedSyntax

@[command_elab kernelLratPackedDecl]
unsafe def elabKernelLratPackedDecl :
    Lean.Elab.Command.CommandElab := fun stx => do
  let `(kernel_lrat_packed_decl $output:ident from
      $formulaName:str with $certificateName:str) := stx
    | throwUnsupportedSyntax
  let declName := (← getCurrNamespace) ++ output.getId
  logInfo m!"kernel_lrat_packed_decl: declaring {declName}"
  if (← getEnv).contains declName then
    throwError "declaration `{declName}` already exists"
  let formulaName := formulaName.getString.toName
  let certificateName := certificateName.getString.toName
  let formulaDef := declName ++ `_prop_expr_def
  let certificateDef := declName ++ `_prop_cert_def
  let cnfDef := declName ++ `_prop_cnf_def
  let packed ←
    try
      Lean.Elab.Command.liftTermElabM do
        Lean.Elab.Term.withDeclName declName do
          let formulaExpr := mkConst formulaName
          let certificateExpr := mkConst certificateName
          let checked ← reconstructLratCheck formulaExpr certificateExpr
            (mkConst ``Bool.true) formulaDef certificateDef cnfDef
          let proof ← mkAppM
            ``Thermite.PropReconstruct.unsat_of_verifyActions
            #[formulaExpr, certificateExpr, checked]
          let packed ← mkAppM ``PackedUnsat.mk #[formulaExpr, proof]
          let packed ← instantiateMVars packed
          unless !packed.hasExprMVar do
            throwError
              "kernel_lrat_packed_decl left unresolved metavariables"
          if packed.hasSorry then
            throwError
              "kernel_lrat_packed_decl produced a sorry-bearing term"
          pure packed
    catch exception =>
      logException exception
      throw exception
  logInfo "kernel_lrat_packed_decl: proof reconstructed"
  try
    Lean.Elab.Command.liftCoreM <|
      withOptions
        (fun options => options.set `compiler.extract_closed false) do
        addAndCompile <| .defnDecl {
          name := declName
          levelParams := []
          type := mkConst ``PackedUnsat
          value := packed
          hints := .opaque
          safety := .safe
        }
  catch exception =>
    logException exception
    throw exception
  logInfo m!"kernel_lrat_packed_decl: present={((← getEnv).contains declName)}"

@[term_elab kernelLratChecked]
unsafe def elabKernelLratChecked : Lean.Elab.Term.TermElab :=
    fun stx expectedType? => do
  let `(kernel_lrat_checked $formulaName:str with
      $certificateName:str) := stx
    | throwUnsupportedSyntax
  let formulaExpr := mkConst formulaName.getString.toName
  let certificateExpr := mkConst certificateName.getString.toName
  let formulaDef ← Lean.Elab.Term.mkAuxName `_prop_expr_def
  let certificateDef ← Lean.Elab.Term.mkAuxName `_prop_cert_def
  let cnfDef ← Lean.Elab.Term.mkAuxName `_prop_cnf_def
  IO.eprintln "kernel_lrat_packed: auxiliary names allocated"
  let proof ← reconstructLratCheck formulaExpr certificateExpr
    (mkConst ``Bool.true) formulaDef certificateDef cnfDef
  if let some expected := expectedType? then
    unless ← isDefEq (← inferType proof) expected do
      throwError
        "kernel_lrat_checked does not match the expected verification equality"
  Lean.Elab.Term.synthesizeSyntheticMVarsNoPostponing
  let proof ← instantiateMVars proof
  unless !proof.hasExprMVar do
    throwError "kernel_lrat_checked left unresolved metavariables"
  pure proof

@[tactic kernelLratCheck]
unsafe def evalKernelLratCheck : Tactic := fun
  | `(tactic| kernel_lrat_check) => do
      let formulaDef ← Lean.Elab.Term.mkAuxName `_prop_expr_def
      let certificateDef ← Lean.Elab.Term.mkAuxName `_prop_cert_def
      let cnfDef ← Lean.Elab.Term.mkAuxName `_prop_cnf_def
      liftMetaFinishingTactic fun goal => goal.withContext do
        let target ← goal.getType
        let some (_, verification, expected) := target.eq?
          | throwError
              "kernel_lrat_check expects a Boolean verification equality"
        let arguments := verification.getAppArgs
        unless verification.getAppFn.constName? ==
            some ``Thermite.PropReconstruct.verifyActions do
          throwError
            "kernel_lrat_check expects verifyActions formula certificate = value"
        unless arguments.size >= 2 do
          throwError "malformed verifyActions application"
        let result ← reconstructLratCheck
          arguments[arguments.size - 2]!
          arguments[arguments.size - 1]!
          expected formulaDef certificateDef cnfDef
        goal.assign result
        unless ← goal.isAssigned do
          throwError "kernel_lrat_check failed to assign the goal"
  | _ => throwUnsupportedSyntax

private unsafe def reconstructLratUnsat
    (formulaExpr certificateExpr : Expr)
    (formulaDef cnfDef replayName : Name) : MetaM Expr := do
  IO.eprintln "kernel_lrat_packed: reflecting formula"
  let formulaType := mkApp (mkConst ``BoolExpr) (mkConst ``Nat)
  let certificateType :=
    mkApp (mkConst ``Array [.zero]) (mkConst ``LRAT.IntAction)
  let formula ← evalExpr (BoolExpr Nat) formulaType formulaExpr
  let actions ←
    evalExpr (Array LRAT.IntAction) certificateType certificateExpr
  IO.eprintln "kernel_lrat_packed: computing CNF"
  let cnf := tseitinCnf formula
  let lrat ← IO.ofExcept (actionsToLratText actions)
  let cnfType := mkApp (mkConst ``CNF [.zero]) (mkConst ``Nat)
  let cnfValue :=
    mkApp2 (mkConst ``CNF.mk [.zero])
      (mkConst ``Nat) (toExpr cnf.clauses)
  addAuxDecl formulaDef (toExpr formula) formulaType
  addAuxDecl cnfDef cnfValue cnfType
  IO.eprintln "kernel_lrat_packed: reconstructing LRAT"
  let (_, proofFormula, _, refutation) ←
    Mathlib.Tactic.Sat.fromLRATAux cnf.dimacs lrat replayName
  IO.eprintln "kernel_lrat_packed: binding proof"
  let cnfExpr := mkConst cnfDef
  let expectedFormula :=
    mkApp (mkConst ``Thermite.PropReconstruct.toProofFormula) cnfExpr
  unless ← withTransparency .all <|
      isDefEq proofFormula expectedFormula do
    throwError
      "LRAT CNF did not bind to the recomputed propositional CNF"
  let bound ← mkEqRefl proofFormula
  let result ← mkAppM
    ``Thermite.PropReconstruct.cnf_unsat_of_proof
    #[cnfExpr, proofFormula, bound, refutation]
  let sourceCnf :=
    mkApp
      (mkConst ``Thermite.PropReconstruct.formulaCnf)
      (mkConst formulaDef)
  unless ← withTransparency .all <| isDefEq sourceCnf cnfExpr do
    throwError
      "recomputed propositional CNF did not bind to LRAT input"
  let cnfBound ← mkEqRefl sourceCnf
  let sourceUnsat ← mkAppM
    ``Thermite.PropReconstruct.cnf_unsat_of_eq
    #[sourceCnf, cnfExpr, cnfBound, result]
  let formulaUnsat ← mkAppM
    ``Thermite.PropReconstruct.unsat_of_tseitinCnf
    #[mkConst formulaDef, sourceUnsat]
  unless ← isDefEq formulaExpr (mkConst formulaDef) do
    throwError
      "reflected formula did not bind to its compiled auxiliary"
  let formulaBound ← mkEqRefl formulaExpr
  let proof ← mkAppM
    ``Thermite.PropReconstruct.boolExpr_unsat_of_eq
    #[formulaExpr, mkConst formulaDef, formulaBound, formulaUnsat]
  IO.eprintln "kernel_lrat_packed: done"
  pure proof

private unsafe def reconstructNamedLratUnsat
    (formulaName certificateName formulaDef cnfDef replayName : Name) :
    MetaM Expr := do
  reconstructLratUnsat
    (mkConst formulaName) (mkConst certificateName)
    formulaDef cnfDef replayName

private unsafe def reconstructCnfUnsat
    (cnfExpr certificateExpr : Expr)
    (cnfDef replayName : Name) : MetaM Expr := do
  let cnfType := mkApp (mkConst ``CNF [.zero]) (mkConst ``Nat)
  let certificateType :=
    mkApp (mkConst ``Array [.zero]) (mkConst ``LRAT.IntAction)
  let cnf ← evalExpr (CNF Nat) cnfType cnfExpr
  let actions ←
    evalExpr (Array LRAT.IntAction) certificateType certificateExpr
  let lrat ← IO.ofExcept (actionsToLratText actions)
  let cnfValue :=
    mkApp2 (mkConst ``CNF.mk [.zero])
      (mkConst ``Nat) (toExpr cnf.clauses)
  addAuxDecl cnfDef cnfValue cnfType
  let (_, proofFormula, _, refutation) ←
    Mathlib.Tactic.Sat.fromLRATAux cnf.dimacs lrat replayName
  let checkedCnf := mkConst cnfDef
  let expectedFormula :=
    mkApp (mkConst ``Thermite.PropReconstruct.toProofFormula) checkedCnf
  unless ← withTransparency .all <|
      isDefEq proofFormula expectedFormula do
    throwError "LRAT proof did not bind to the reflected CNF"
  let proofFormulaBound ← mkEqRefl proofFormula
  let checkedUnsat ← mkAppM
    ``Thermite.PropReconstruct.cnf_unsat_of_proof
    #[checkedCnf, proofFormula, proofFormulaBound, refutation]
  let cnfBound ←
    if ← withTransparency .all <| isDefEq cnfExpr checkedCnf then
      mkEqRefl cnfExpr
    else
      let reduced ← reduceAll cnfExpr
      unless ← withTransparency .all <| isDefEq reduced checkedCnf do
        throwError "source CNF did not bind to its reflected auxiliary"
      mkEqRefl cnfExpr
  let result ← mkAppM
    ``Thermite.PropReconstruct.cnf_unsat_of_eq
    #[cnfExpr, checkedCnf, cnfBound, checkedUnsat]
  let result ← instantiateMVars result
  unless !result.hasExprMVar do
    throwError "kernel CNF LRAT proof left unresolved metavariables"
  if result.hasSorry then
    throwError "kernel CNF LRAT proof contains a sorry"
  pure result

@[tactic kernelLratCnfUnsat]
unsafe def evalKernelLratCnfUnsat : Tactic := fun
  | `(tactic| kernel_lrat_cnf_unsat $cnfName:str with
      $certificateName:str) => do
      let cnfDef ← Lean.Elab.Term.mkAuxName `_cnf_unsat_def
      let replayName ← Lean.Elab.Term.mkAuxName `_cnf_lrat
      liftMetaFinishingTactic fun goal => goal.withContext do
        let proof ← reconstructCnfUnsat
          (mkConst cnfName.getString.toName)
          (mkConst certificateName.getString.toName)
          cnfDef replayName
        unless ← isDefEq (← inferType proof) (← goal.getType) do
          throwError "kernel_lrat_cnf_unsat result does not match its goal"
        goal.assign proof
  | _ => throwUnsupportedSyntax

@[term_elab kernelLratPacked]
unsafe def elabKernelLratPacked : Lean.Elab.Term.TermElab :=
    fun stx expectedType? => do
  IO.eprintln "kernel_lrat_packed: entered fast elaborator"
  let `(kernel_lrat_packed $formulaName:str with
      $certificateName:str) := stx
    | throwUnsupportedSyntax
  let expected := mkConst ``PackedUnsat
  if let some type := expectedType? then
    unless ← isDefEq type expected do
      throwError "kernel_lrat_packed must elaborate as PackedUnsat"
  IO.eprintln "kernel_lrat_packed: expected type bound"
  let formulaExpr := mkConst formulaName.getString.toName
  let certificateExpr := mkConst certificateName.getString.toName
  let formulaDef ← Lean.Elab.Term.mkAuxName `_prop_expr_def
  let certificateDef ← Lean.Elab.Term.mkAuxName `_prop_cert_def
  let cnfDef ← Lean.Elab.Term.mkAuxName `_prop_cnf_def
  IO.eprintln "kernel_lrat_packed: auxiliary names allocated"
  let checked ←
    try
      reconstructLratCheck formulaExpr certificateExpr
        (mkConst ``Bool.true) formulaDef certificateDef cnfDef
    catch exception =>
      logException exception
      throw exception
  IO.eprintln "kernel_lrat_packed: verification reconstructed"
  let proof ← mkAppM
    ``Thermite.PropReconstruct.unsat_of_verifyActions
    #[formulaExpr, certificateExpr, checked]
  let packed ← mkAppM ``PackedUnsat.mk #[formulaExpr, proof]
  Lean.Elab.Term.synthesizeSyntheticMVarsNoPostponing
  let packed ← instantiateMVars packed
  unless !packed.hasExprMVar do
    throwError "kernel_lrat_packed left unresolved metavariables"
  if packed.hasSorry then
    throwError "kernel_lrat_packed produced a sorry-bearing term"
  IO.eprintln "kernel_lrat_packed: returning packed proof"
  pure packed

@[tactic kernelLratUnsat]
unsafe def evalKernelLratUnsat : Tactic := fun
  | `(tactic| kernel_lrat_unsat $formulaName:str with
        $certificateName:str) => do
      let formulaDef ← Lean.Elab.Term.mkAuxName `_prop_expr_def
      let cnfDef ← Lean.Elab.Term.mkAuxName `_prop_cnf_def
      let replayName ← Lean.Elab.Term.mkAuxName `_prop_lrat
      liftMetaFinishingTactic fun goal => goal.withContext do
        let formulaExpr := mkConst formulaName.getString.toName
        let finalProof ← reconstructNamedLratUnsat
          formulaName.getString.toName certificateName.getString.toName
          formulaDef cnfDef replayName
        let target ← goal.getType
        if target.getAppFn.constName? == some ``UnsatPin then
          goal.assign (← mkAppM ``UnsatPin.mk #[finalProof])
        else if target.getAppFn.constName? == some ``PackedUnsat then
          goal.assign (← mkAppM ``PackedUnsat.mk #[formulaExpr, finalProof])
        else
          goal.assign finalProof
  | _ => throwUnsupportedSyntax

#print axioms denote_compile
#print axioms unsat_of_verifyActions
#print axioms implication_of_verifyActions

end Thermite.PropReconstruct
