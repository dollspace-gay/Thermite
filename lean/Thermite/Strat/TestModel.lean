/-
  A two-element model used by the reconstruction pins.

  Every syntactic sort receives a separate `Bool` carrier. The uniform
  implementation keeps small counterexamples reducible while still exercising
  the dependent carrier interface.
-/
import Thermite.Strat.Model

namespace Thermite.Strat.Cls

def boolSeqView (value : Bool) : List Bool :=
  if value then [true] else []

def searchedBoolSeqView (seed : Nat) (value : Bool) : List Bool :=
  if seed.testBit 0 then [value] else boolSeqView value

def boolIndex (value : Bool) : Nat :=
  if value then 1 else 0

def boolEmbedNat (value : Nat) : Bool :=
  value % 2 == 1

def boolLiteral : ScalarValue → Bool
  | .bool value => value
  | .int value => value % 2 == 1

def searchedBoolConstant (seed id : Nat) : Bool :=
  seed.testBit (id % 5 + 1)

def searchedBoolQfree (seed id : Nat) : Bool :=
  seed.testBit (id % 5 + 6)

def searchedBoolApp (seed fn : Nat) (value : Bool) : Bool :=
  match seed.testBit (fn % 2 * 2 + 11),
      seed.testBit (fn % 2 * 2 + 12) with
  | false, false => false
  | false, true => true
  | true, false => value
  | true, true => !value

def relationCode : Rel → Nat
  | .eq => 0
  | .ne => 1
  | .lt => 2
  | .le => 3
  | .gt => 4
  | .ge => 5

def searchedBoolOrder (seed : Nat) (relation : Rel)
    (left right : Bool) : Bool :=
  seed.testBit <|
    (relationCode relation * 4 +
      (if left then 2 else 0) +
      (if right then 1 else 0)) % 16

def boolModel : Model where
  Carrier := fun _ => Bool
  default := fun _ => false
  decEq := fun _ => inferInstance
  enum := fun _ => [false, true]
  enum_complete := by
    intro sort value
    cases value <;> simp
  constant := fun _ id => id % 2 == 1
  literal := fun _ value => boolLiteral value
  seqView := fun _ value => boolSeqView value
  index := fun _ value => boolIndex value
  embedNat := boolEmbedNat
  read := fun _ _ _ base index =>
    (boolSeqView base).getD (boolIndex index) false
  len := fun _ base => boolEmbedNat (boolSeqView base).length
  seqDiff := fun _ _ _ => false
  cast := fun _ _ value => value
  idxOffset := fun _ value _ => value
  mul := fun _ _ left right => left && right
  app1 := fun _ _ _ _ value => value
  order := fun relation _ _ left right =>
    match relation with
    | .eq => left == right
    | .ne => left != right
    | .lt => !left && right
    | .le => !left || right
    | .gt => left && !right
    | .ge => left || !right
  qfree := fun _ _ => false
  read_seq := by
    intro elem indexSort seq index
    rfl
  len_seq := by
    intro elem seq
    rfl
  index_embedNat_seq := by
    intro elem seq indexValue inBounds
    cases seq with
    | false => simp [boolSeqView] at inBounds
    | true =>
        have : indexValue = 0 := by
          simp [boolSeqView] at inBounds
          omega
        subst indexValue
        rfl
  embedNat_seqLength_injective := by
    intro elem left right equal
    cases left <;> cases right <;> simp [boolSeqView, boolEmbedNat] at equal ⊢
  seq_ext_at_diff := by
    intro elem left right sameLength sameRead
    cases left <;> cases right <;>
      simp [boolSeqView, boolEmbedNat, boolIndex] at sameLength sameRead ⊢
  seqDiff_lower_bound := by
    intros
    rfl
  seqDiff_upper_bound := by
    intro elem left right sameLength different
    cases left <;> cases right <;>
      simp [boolSeqView, boolEmbedNat] at sameLength different ⊢
  seq_ext := by
    intro elem left right equal
    cases left <;> cases right <;> simp [boolSeqView] at equal ⊢

/-- A small family of typed models used to realize SAT results as actual Lean
    countermodels. The seed selects source constants, unary functions, qfree
    leaves, ordering tables, and one of two injective sequence encodings.
    Reconstruction searches this family outside the kernel, then asks Lean to
    check the selected seed against the original source formula. -/
def searchedBoolModel (seed : Nat) : Model where
  Carrier := fun _ => Bool
  default := fun _ => false
  decEq := fun _ => inferInstance
  enum := fun _ => [false, true]
  enum_complete := by
    intro sort value
    cases value <;> simp
  constant := fun _ id => searchedBoolConstant seed id
  literal := fun _ value => boolLiteral value
  seqView := fun _ value => searchedBoolSeqView seed value
  index := fun _ value => boolIndex value
  embedNat := boolEmbedNat
  read := fun _ _ _ base index =>
    (searchedBoolSeqView seed base).getD (boolIndex index) false
  len := fun _ base =>
    boolEmbedNat (searchedBoolSeqView seed base).length
  seqDiff := fun _ _ _ => false
  cast := fun _ _ value => value
  idxOffset := fun _ value _ => value
  mul := fun _ _ left right => left && right
  app1 := fun _ _ _ fn value => searchedBoolApp seed fn value
  order := fun relation _ _ left right =>
    match relation with
    | .eq => left == right
    | .ne => left != right
    | .lt => !left && right
    | .le => !left || right
    | .gt => left && !right
    | .ge => left || !right
  qfree := fun id _ => searchedBoolQfree seed id
  read_seq := by
    intro elem indexSort seq index
    rfl
  len_seq := by
    intro elem seq
    rfl
  index_embedNat_seq := by
    intro elem seq indexValue inBounds
    unfold searchedBoolSeqView at inBounds
    split at inBounds
    · have : indexValue = 0 := by
        simp at inBounds
        omega
      subst indexValue
      rfl
    · cases seq with
      | false => simp [boolSeqView] at inBounds
      | true =>
          have : indexValue = 0 := by
            simp [boolSeqView] at inBounds
            omega
          subst indexValue
          rfl
  embedNat_seqLength_injective := by
    intro elem left right equal
    by_cases profile : seed.testBit 0
    · simp [searchedBoolSeqView, profile]
    · cases left <;> cases right <;>
        simp [searchedBoolSeqView, profile, boolSeqView,
          boolEmbedNat] at equal ⊢
  seq_ext_at_diff := by
    intro elem left right sameLength sameRead
    by_cases profile : seed.testBit 0
    · cases left <;> cases right <;>
        simp [searchedBoolSeqView, profile,
          boolEmbedNat, boolIndex] at sameLength sameRead ⊢
    · cases left <;> cases right <;>
        simp [searchedBoolSeqView, profile, boolSeqView,
          boolEmbedNat, boolIndex] at sameLength sameRead ⊢
  seqDiff_lower_bound := by
    intros
    rfl
  seqDiff_upper_bound := by
    intro elem left right sameLength different
    cases profile : seed.testBit 0 with
    | false =>
      cases left <;> cases right <;>
        simp [searchedBoolSeqView, profile, boolSeqView,
          boolEmbedNat] at sameLength different ⊢
    | true =>
      cases left <;> cases right <;>
        simp [searchedBoolSeqView, profile, boolEmbedNat] at sameLength different ⊢
  seq_ext := by
    intro elem left right equal
    unfold searchedBoolSeqView at equal
    split at equal
    · simpa using equal
    · cases left <;> cases right <;>
        simp [boolSeqView] at equal ⊢

/-- Override only the canonical QFree assignment while retaining the searched
    constants, functions, relations, and sequence operations. Production
    countermodel replay uses this after independently checking the QF_LIA/QF_BV
    leaf values. -/
def searchedBoolModelWithQfree (seed : Nat) (values : Nat → Bool) : Model :=
  { searchedBoolModel seed with qfree := fun id _ => values id }

def emptyBoolValuation : Valuation boolModel :=
  fun _ => ⟨.mach .bool, false⟩

def emptySearchedBoolValuation (seed : Nat) :
    Valuation (searchedBoolModel seed) :=
  fun _ => ⟨.mach .bool, false⟩

def emptySearchedBoolValuationWithQfree (seed : Nat) (values : Nat → Bool) :
    Valuation (searchedBoolModelWithQfree seed values) :=
  fun _ => ⟨.mach .bool, false⟩

end Thermite.Strat.Cls
