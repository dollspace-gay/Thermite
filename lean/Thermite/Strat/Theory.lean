/-
  Kernel-justified ground theory clauses.

  A `TheoryClause` is the Horn clause `premises → conclusion`; its CNF form is
  the negation of every premise followed by the positive conclusion. These
  constructors cover equality, relation/function congruence, reads, lengths,
  casts, offsets, multiplication, and sequence extensionality. Arithmetic
  leaves are intentionally routed to the existing QF_LIA/QF_BV replay modules.
-/
import Thermite.Strat.Grounding
import Mathlib.Data.List.GetD

namespace Thermite.Strat.Cls

structure TheoryClause where
  premises : List Atom
  conclusion : Atom
  deriving Repr

def TheoryClause.eval (M : Model) (ρ : Valuation M)
    (clause : TheoryClause) : Bool :=
  !(clause.premises.all (evalAtom M ρ)) || evalAtom M ρ clause.conclusion

theorem TheoryClause.sound_of (M : Model) (ρ : Valuation M)
    (clause : TheoryClause)
    (sound : clause.premises.all (evalAtom M ρ) = true →
      evalAtom M ρ clause.conclusion = true) :
    clause.eval M ρ = true := by
  unfold TheoryClause.eval
  cases premisesTrue : clause.premises.all (evalAtom M ρ)
  · rfl
  · simp only [Bool.not_true, Bool.false_or]
    exact sound premisesTrue

def equalityReflexivity (term : Tm) : TheoryClause :=
  ⟨[], .rel .eq term term⟩

theorem equalityReflexivity_sound (M : Model) (ρ : Valuation M) (term : Tm) :
    (equalityReflexivity term).eval M ρ = true := by
  simp [TheoryClause.eval, equalityReflexivity, evalAtom_eq_self]

def equalitySymmetry (left right : Tm) : TheoryClause :=
  ⟨[.rel .eq left right], .rel .eq right left⟩

theorem equalitySymmetry_sound (M : Model) (ρ : Valuation M)
    (left right : Tm) :
    (equalitySymmetry left right).eval M ρ = true := by
  apply TheoryClause.sound_of
  simp only [equalitySymmetry, List.all_cons, List.all_nil, Bool.and_true]
  intro equal
  have tagged := (valueEqTagged_eq_true_iff M _ _).mp equal
  exact (valueEqTagged_eq_true_iff M _ _).mpr tagged.symm

def equalityTransitivity (left middle right : Tm) : TheoryClause :=
  ⟨[.rel .eq left middle, .rel .eq middle right], .rel .eq left right⟩

theorem equalityTransitivity_sound (M : Model) (ρ : Valuation M)
    (left middle right : Tm) :
    (equalityTransitivity left middle right).eval M ρ = true := by
  apply TheoryClause.sound_of
  simp only [equalityTransitivity, List.all_cons, List.all_nil,
    Bool.and_true, Bool.and_eq_true]
  rintro ⟨leftMiddle, middleRight⟩
  have first := (valueEqTagged_eq_true_iff M _ _).mp leftMiddle
  have second := (valueEqTagged_eq_true_iff M _ _).mp middleRight
  exact (valueEqTagged_eq_true_iff M _ _).mpr (first.trans second)

def unaryCongruence (inputLeft inputRight outputLeft outputRight : Tm) :
    TheoryClause :=
  ⟨[.rel .eq inputLeft inputRight], .rel .eq outputLeft outputRight⟩

def app1Congruence (arg result : Sort₂) (fn : Nat)
    (left right : Tm) : TheoryClause :=
  unaryCongruence left right
    (.app1 arg result fn left) (.app1 arg result fn right)

theorem app1Congruence_sound (M : Model) (ρ : Valuation M)
    (arg result : Sort₂) (fn : Nat) (left right : Tm) :
    (app1Congruence arg result fn left right).eval M ρ = true := by
  apply TheoryClause.sound_of
  simp only [app1Congruence, unaryCongruence, List.all_cons, List.all_nil,
    Bool.and_true, evalAtom]
  intro equal
  have tagged := (valueEqTagged_eq_true_iff M _ _).mp equal
  have outputs :
      evalTm M ρ (.app1 arg result fn left) =
        evalTm M ρ (.app1 arg result fn right) := by
    simp only [evalTm]
    rw [tagged]
  exact (valueEqTagged_eq_true_iff M _ _).mpr outputs

def readCongruence (elem : Sort₂)
    (leftSeq rightSeq leftIndex rightIndex : Tm) : TheoryClause :=
  ⟨[.rel .eq leftSeq rightSeq, .rel .eq leftIndex rightIndex],
    .rel .eq (.read elem leftSeq leftIndex) (.read elem rightSeq rightIndex)⟩

theorem readCongruence_sound (M : Model) (ρ : Valuation M)
    (elem : Sort₂) (leftSeq rightSeq leftIndex rightIndex : Tm) :
    (readCongruence elem leftSeq rightSeq leftIndex rightIndex).eval M ρ = true := by
  apply TheoryClause.sound_of
  simp only [readCongruence, List.all_cons, List.all_nil,
    Bool.and_true, Bool.and_eq_true, evalAtom]
  rintro ⟨seqEqual, indexEqual⟩
  have seqTagged := (valueEqTagged_eq_true_iff M _ _).mp seqEqual
  have indexTagged := (valueEqTagged_eq_true_iff M _ _).mp indexEqual
  have outputs :
      evalTm M ρ (.read elem leftSeq leftIndex) =
        evalTm M ρ (.read elem rightSeq rightIndex) := by
    simp only [evalTm]
    rw [seqTagged, indexTagged]
  exact (valueEqTagged_eq_true_iff M _ _).mpr outputs

def lenCongruence (left right : Tm) : TheoryClause :=
  unaryCongruence left right (.len left) (.len right)

theorem lenCongruence_sound (M : Model) (ρ : Valuation M)
    (left right : Tm) :
    (lenCongruence left right).eval M ρ = true := by
  apply TheoryClause.sound_of
  simp only [lenCongruence, unaryCongruence, List.all_cons, List.all_nil,
    Bool.and_true, evalAtom]
  intro equal
  have tagged := (valueEqTagged_eq_true_iff M _ _).mp equal
  have outputs : evalTm M ρ (.len left) = evalTm M ρ (.len right) := by
    simp only [evalTm]
    rw [tagged]
  exact (valueEqTagged_eq_true_iff M _ _).mpr outputs

def castCongruence (target : Sort₂) (left right : Tm) : TheoryClause :=
  unaryCongruence left right (.cast target left) (.cast target right)

theorem castCongruence_sound (M : Model) (ρ : Valuation M)
    (target : Sort₂) (left right : Tm) :
    (castCongruence target left right).eval M ρ = true := by
  apply TheoryClause.sound_of
  simp only [castCongruence, unaryCongruence, List.all_cons, List.all_nil,
    Bool.and_true, evalAtom]
  intro equal
  have tagged := (valueEqTagged_eq_true_iff M _ _).mp equal
  have outputs :
      evalTm M ρ (.cast target left) = evalTm M ρ (.cast target right) := by
    simp only [evalTm]
    rw [tagged]
  exact (valueEqTagged_eq_true_iff M _ _).mpr outputs

def offsetCongruence (offset : Int) (left right : Tm) : TheoryClause :=
  unaryCongruence left right (.idxOp left offset) (.idxOp right offset)

theorem offsetCongruence_sound (M : Model) (ρ : Valuation M)
    (offset : Int) (left right : Tm) :
    (offsetCongruence offset left right).eval M ρ = true := by
  apply TheoryClause.sound_of
  simp only [offsetCongruence, unaryCongruence, List.all_cons, List.all_nil,
    Bool.and_true, evalAtom]
  intro equal
  have tagged := (valueEqTagged_eq_true_iff M _ _).mp equal
  have outputs :
      evalTm M ρ (.idxOp left offset) = evalTm M ρ (.idxOp right offset) := by
    simp only [evalTm]
    rw [tagged]
  exact (valueEqTagged_eq_true_iff M _ _).mpr outputs

def mulCongruence (left₁ left₂ right₁ right₂ : Tm) : TheoryClause :=
  ⟨[.rel .eq left₁ left₂, .rel .eq right₁ right₂],
    .rel .eq (.mul left₁ right₁) (.mul left₂ right₂)⟩

theorem mulCongruence_sound (M : Model) (ρ : Valuation M)
    (left₁ left₂ right₁ right₂ : Tm) :
    (mulCongruence left₁ left₂ right₁ right₂).eval M ρ = true := by
  apply TheoryClause.sound_of
  simp only [mulCongruence, List.all_cons, List.all_nil,
    Bool.and_true, Bool.and_eq_true, evalAtom]
  rintro ⟨leftEqual, rightEqual⟩
  have leftTagged := (valueEqTagged_eq_true_iff M _ _).mp leftEqual
  have rightTagged := (valueEqTagged_eq_true_iff M _ _).mp rightEqual
  have outputs :
      evalTm M ρ (.mul left₁ right₁) = evalTm M ρ (.mul left₂ right₂) := by
    simp only [evalTm]
    rw [leftTagged, rightTagged]
  exact (valueEqTagged_eq_true_iff M _ _).mpr outputs

/-- Congruence for every relation, including equality and disequality. -/
def relationCongruence (relation : Rel)
    (left₁ left₂ right₁ right₂ : Tm) : TheoryClause :=
  ⟨[.rel .eq left₁ left₂, .rel .eq right₁ right₂,
      .rel relation left₁ right₁],
    .rel relation left₂ right₂⟩

def evalRelationValues (M : Model) (relation : Rel) (left right : Value M) : Bool :=
  match relation with
  | .eq => valueEqTagged M left right
  | .ne => !valueEqTagged M left right
  | .lt => orderTagged M .lt left right
  | .le => orderTagged M .le left right
  | .gt => orderTagged M .gt left right
  | .ge => orderTagged M .ge left right

theorem evalAtom_relation (M : Model) (ρ : Valuation M)
    (relation : Rel) (left right : Tm) :
    evalAtom M ρ (.rel relation left right) =
      evalRelationValues M relation (evalTm M ρ left) (evalTm M ρ right) := by
  cases relation <;> rfl

theorem relationCongruence_sound (M : Model) (ρ : Valuation M)
    (relation : Rel) (left₁ left₂ right₁ right₂ : Tm) :
    (relationCongruence relation left₁ left₂ right₁ right₂).eval M ρ = true := by
  apply TheoryClause.sound_of
  simp only [relationCongruence, List.all_cons, List.all_nil,
    Bool.and_true, Bool.and_eq_true]
  intro premises
  have leftEqual := premises.1
  have rightEqual := premises.2.1
  have relationTrue := premises.2.2
  have leftTagged := (valueEqTagged_eq_true_iff M _ _).mp leftEqual
  have rightTagged := (valueEqTagged_eq_true_iff M _ _).mp rightEqual
  have sameRelation :
      evalAtom M ρ (.rel relation left₁ right₁) =
        evalAtom M ρ (.rel relation left₂ right₂) := by
    rw [evalAtom_relation, evalAtom_relation, leftTagged, rightTagged]
  rw [← sameRelation]
  exact relationTrue

/-- Semantic array extensionality used to justify the finite read clause family.
    The length premise fixes the common finite bound, and every read below that
    bound fixes the sequence view pointwise. -/
theorem sequence_extensionality_sound (M : Model) (elem : Sort₂)
    (left right : M.Carrier (.seq elem))
    (sameLength : M.len (.seq elem) left = M.len (.seq elem) right)
    (sameReads : ∀ index : Nat, index < (M.seqView elem left).length →
      M.read elem (.seq elem) usizeS left (M.embedNat index) =
        M.read elem (.seq elem) usizeS right (M.embedNat index)) :
    left = right := by
  apply M.seq_ext
  have lengths :
      (M.seqView elem left).length = (M.seqView elem right).length := by
    apply M.embedNat_seqLength_injective elem left right
    rw [← M.len_seq elem left, ← M.len_seq elem right]
    exact sameLength
  apply List.ext_getElem lengths
  intro index leftBound rightBound
  have reads := sameReads index leftBound
  rw [M.read_seq, M.read_seq,
    M.index_embedNat_seq elem left index leftBound] at reads
  rw [List.getD_eq_getElem (l := M.seqView elem left)
      (d := M.default elem) leftBound,
    List.getD_eq_getElem (l := M.seqView elem right)
      (d := M.default elem) rightBound] at reads
  exact reads

#print axioms equalityTransitivity_sound
#print axioms relationCongruence_sound
#print axioms sequence_extensionality_sound

end Thermite.Strat.Cls
