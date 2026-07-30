/-
  Typed semantics for the S₂.0 classifier language.

  The old stage-2 theorem treated every relation and array atom as a free
  `Atom → Bool`. That was enough to check binder bookkeeping, but it did not say
  what reads, lengths, equality, casts, or declared functions meant. This module
  gives every term a value in the carrier named by `Tm.sortOf` and gives every
  formula a per-sort finite interpretation.
-/
import Thermite.Strat.RefEncode
import Thermite.Denote

namespace Thermite.Strat.Cls

open Classical

/-- A typed, finite interpretation of the S₂.0 term language. Operations accept
    their syntax-level input sorts explicitly, so evaluation remains total even
    for a malformed term. The source bridge and reconstruction gate separately
    require well-sorted input. -/
structure Model where
  Carrier : Sort₂ → Type
  default : (s : Sort₂) → Carrier s
  decEq : (s : Sort₂) → DecidableEq (Carrier s)
  enum : (s : Sort₂) → List (Carrier s)
  enum_complete : ∀ (s : Sort₂) (x : Carrier s), x ∈ enum s

  constant : (s : Sort₂) → Nat → Carrier s
  literal : (s : Sort₂) → ScalarValue → Carrier s

  seqView : (elem : Sort₂) → Carrier (.seq elem) → List (Carrier elem)
  index : (s : Sort₂) → Carrier s → Nat
  embedNat : Nat → Carrier usizeS
  read : (elem base index : Sort₂) → Carrier base → Carrier index → Carrier elem
  len : (base : Sort₂) → Carrier base → Carrier usizeS
  seqDiff : (elem : Sort₂) →
    Carrier (.seq elem) → Carrier (.seq elem) → Carrier usizeS
  cast : (source target : Sort₂) → Carrier source → Carrier target
  idxOffset : (s : Sort₂) → Carrier s → Int → Carrier s
  mul : (left right : Sort₂) → Carrier left → Carrier right → Carrier left
  app1 : (arg result actual : Sort₂) → Nat → Carrier actual → Carrier result
  order : Rel → (left right : Sort₂) → Carrier left → Carrier right → Bool
  qfree : Nat → Thermite.Expr → Bool

  read_seq : ∀ (elem indexSort : Sort₂) (sq : Carrier (.seq elem))
      (i : Carrier indexSort),
    read elem (.seq elem) indexSort sq i =
      (seqView elem sq).getD (index indexSort i) (default elem)
  len_seq : ∀ (elem : Sort₂) (sq : Carrier (.seq elem)),
    len (.seq elem) sq = embedNat (seqView elem sq).length
  index_embedNat_seq : ∀ (elem : Sort₂) (sq : Carrier (.seq elem)) (indexValue : Nat),
    indexValue < (seqView elem sq).length →
      index usizeS (embedNat indexValue) = indexValue
  embedNat_seqLength_injective : ∀ (elem : Sort₂)
      (left right : Carrier (.seq elem)),
    embedNat (seqView elem left).length =
      embedNat (seqView elem right).length →
    (seqView elem left).length = (seqView elem right).length
  seq_ext_at_diff : ∀ (elem : Sort₂)
      (left right : Carrier (.seq elem)),
    len (.seq elem) left = len (.seq elem) right →
    read elem (.seq elem) usizeS left (seqDiff elem left right) =
      read elem (.seq elem) usizeS right (seqDiff elem left right) →
    left = right
  seqDiff_lower_bound : ∀ (elem : Sort₂)
      (left right : Carrier (.seq elem)),
    len (.seq elem) left = len (.seq elem) right →
    left ≠ right →
    order .le usizeS usizeS (literal usizeS (.int 0))
      (seqDiff elem left right) = true
  seqDiff_upper_bound : ∀ (elem : Sort₂)
      (left right : Carrier (.seq elem)),
    len (.seq elem) left = len (.seq elem) right →
    left ≠ right →
    order .lt usizeS usizeS (seqDiff elem left right)
      (len (.seq elem) left) = true
  seq_ext : ∀ (elem : Sort₂) (left right : Carrier (.seq elem)),
    seqView elem left = seqView elem right → left = right

/-- A model tied to the actual meaning of embedded quantifier-free source
    expressions. Relation and array terms are interpreted by `model`; qfree
    leaves are not another oracle. -/
structure SourceModel extends Model where
  venv : Thermite.Env
  qfree_source : ∀ id e, qfree id e = decide (Thermite.denote 0 e venv)

/-- A value tagged by its carrier sort. Environments use tagged values because
    one de Bruijn stack contains binders of several sorts. -/
abbrev Value (M : Model) := Sigma M.Carrier

abbrev Valuation (M : Model) := Nat → Value M

def Model.valueAt (M : Model) (ρ : Valuation M) (expected : Sort₂) (i : Nat) :
    M.Carrier expected :=
  match ρ i with
  | ⟨actual, value⟩ =>
      if h : actual = expected then h ▸ value else M.default expected

def Valuation.cons (M : Model) (s : Sort₂) (value : M.Carrier s)
    (ρ : Valuation M) : Valuation M
  | 0 => ⟨s, value⟩
  | i + 1 => ρ i

def Valuation.upd (M : Model) (ρ : Valuation M) (name : Nat)
    (s : Sort₂) (value : M.Carrier s) : Valuation M :=
  fun other => if other = name then ⟨s, value⟩ else ρ other

/-- Typed evaluation returns a carrier value paired with its sort. Every model
    operation still consumes and produces values in indexed `Carrier` types. -/
def evalTm (M : Model) (ρ : Valuation M) : Tm → Value M
  | .var s i => ⟨s, M.valueAt ρ s i⟩
  | .const s id => ⟨s, M.constant s id⟩
  | .lit s value => ⟨s, M.literal s value⟩
  | .read elem sq ix =>
      match evalTm M ρ sq, evalTm M ρ ix with
      | ⟨baseSort, base⟩, ⟨indexSort, index⟩ =>
          ⟨elem, M.read elem baseSort indexSort base index⟩
  | .len sq =>
      match evalTm M ρ sq with
      | ⟨baseSort, base⟩ => ⟨usizeS, M.len baseSort base⟩
  | .cast to term =>
      match evalTm M ρ term with
      | ⟨source, value⟩ => ⟨to, M.cast source to value⟩
  | .idxOp term offset =>
      match evalTm M ρ term with
      | ⟨sort, value⟩ => ⟨sort, M.idxOffset sort value offset⟩
  | .mul left right =>
      match evalTm M ρ left, evalTm M ρ right with
      | ⟨leftSort, leftValue⟩, ⟨rightSort, rightValue⟩ =>
          ⟨leftSort, M.mul leftSort rightSort leftValue rightValue⟩
  | .app1 arg result fn term =>
      match evalTm M ρ term with
      | ⟨actual, value⟩ => ⟨result, M.app1 arg result actual fn value⟩

def valueEq (M : Model) {left right : Sort₂}
    (x : M.Carrier left) (y : M.Carrier right) : Bool :=
  if h : left = right then
    @decide ((h ▸ x) = y) (M.decEq right (h ▸ x) y)
  else
    false

def valueEqTagged (M : Model) : Value M → Value M → Bool
  | ⟨_, left⟩, ⟨_, right⟩ => valueEq M left right

def orderTagged (M : Model) (relation : Rel) : Value M → Value M → Bool
  | ⟨leftSort, left⟩, ⟨rightSort, right⟩ =>
      M.order relation leftSort rightSort left right

def evalAtom (M : Model) (ρ : Valuation M) : Atom → Bool
  | .qfree id expr => M.qfree id expr
  | .rel .eq left right => valueEqTagged M (evalTm M ρ left) (evalTm M ρ right)
  | .rel .ne left right => !valueEqTagged M (evalTm M ρ left) (evalTm M ρ right)
  | .rel relation left right =>
      orderTagged M relation (evalTm M ρ left) (evalTm M ρ right)

/-- Formula evaluation uses the enumeration belonging to each binder's sort.
    `enum_complete` also makes every carrier non-empty. -/
def evalFrm (M : Model) : Frm → Valuation M → Bool
  | .atom atom, ρ => evalAtom M ρ atom
  | .neg formula, ρ => !evalFrm M formula ρ
  | .conj left right, ρ => evalFrm M left ρ && evalFrm M right ρ
  | .disj left right, ρ => evalFrm M left ρ || evalFrm M right ρ
  | .imp left right, ρ => !evalFrm M left ρ || evalFrm M right ρ
  | .all sort body, ρ =>
      (M.enum sort).all fun value => evalFrm M body (Valuation.cons M sort value ρ)
  | .ex sort body, ρ =>
      (M.enum sort).any fun value => evalFrm M body (Valuation.cons M sort value ρ)

theorem Model.enum_ne_nil (M : Model) (s : Sort₂) : M.enum s ≠ [] := by
  intro empty
  have member := M.enum_complete s (M.default s)
  rw [empty] at member
  simp at member

theorem valueEq_self (M : Model) {s : Sort₂} (x : M.Carrier s) :
    valueEq M x x = true := by
  simp [valueEq]

theorem valueEqTagged_self (M : Model) (x : Value M) :
    valueEqTagged M x x = true := by
  cases x with
  | mk sort value => exact valueEq_self M value

theorem valueEqTagged_eq_true_iff (M : Model) (left right : Value M) :
    valueEqTagged M left right = true ↔ left = right := by
  cases left with
  | mk leftSort leftValue =>
      cases right with
      | mk rightSort rightValue =>
          simp only [valueEqTagged, valueEq]
          split
          next sameSort =>
            subst sameSort
            simp
          next differentSort =>
            simp only [Bool.false_eq_true, false_iff]
            intro tagged
            injection tagged with sortEquality
            exact differentSort sortEquality

theorem evalAtom_eq_self (M : Model) (ρ : Valuation M) (term : Tm) :
    evalAtom M ρ (.rel .eq term term) = true :=
  valueEqTagged_self M (evalTm M ρ term)

/-! The named token semantics. This is the semantic target of the production
    encoder; unlike `tokDenote`, it keeps the binder sort and evaluates atoms. -/

def evalTok (M : Model) : Thermite.Strat.Tok → Valuation M → Bool
  | .atom atom, ρ => evalAtom M ρ atom
  | .neg formula, ρ => !evalTok M formula ρ
  | .conj left right, ρ => evalTok M left ρ && evalTok M right ρ
  | .disj left right, ρ => evalTok M left ρ || evalTok M right ρ
  | .imp left right, ρ => !evalTok M left ρ || evalTok M right ρ
  | .all sort name _ body, ρ =>
      (M.enum sort).all fun value =>
        evalTok M body (Valuation.upd M ρ name sort value)
  | .ex sort name _ body, ρ =>
      (M.enum sort).any fun value =>
        evalTok M body (Valuation.upd M ρ name sort value)

end Thermite.Strat.Cls

namespace Thermite.Strat

open Thermite.Strat.Cls

def TypedAgree (M : Model) (depth : Nat) (ρ σ : Valuation M) : Prop :=
  ∀ i, i < depth → σ (encName depth i) = ρ i

theorem TypedAgree_upd (M : Model) (depth : Nat) (ρ σ : Valuation M)
    (sort : Sort₂) (value : M.Carrier sort)
    (agree : TypedAgree M depth ρ σ) :
    TypedAgree M (depth + 1) (Valuation.cons M sort value ρ)
      (Valuation.upd M σ depth sort value) := by
  intro i hi
  cases i with
  | zero =>
      have name : encName (depth + 1) 0 = depth := by simp [encName]
      rw [name]
      simp [Valuation.upd, Valuation.cons]
  | succ i =>
      have hi' : i < depth := Nat.lt_of_succ_lt_succ hi
      have name : encName (depth + 1) (i + 1) = encName depth i := by
        simp [encName]
        omega
      have distinct : encName depth i ≠ depth := by
        simp [encName]
        omega
      rw [name]
      simp only [Valuation.upd, distinct, ↓reduceIte, Valuation.cons]
      exact agree i hi'

theorem eval_encTm (M : Model) (depth : Nat) (ρ σ : Valuation M)
    (agree : TypedAgree M depth ρ σ) :
    ∀ term : Tm, wfTm depth term = true →
      evalTm M σ (encTm depth term) = evalTm M ρ term := by
  intro term
  induction term with
  | var sort index =>
      intro wellFormed
      have inScope : index < depth := by
        simpa [wfTm] using wellFormed
      simp only [encTm, evalTm, Model.valueAt]
      rw [agree index inScope]
  | const sort id => intro _; rfl
  | lit sort value => intro _; rfl
  | read elem seq index seqIH indexIH =>
      intro wellFormed
      simp only [wfTm, Bool.and_eq_true] at wellFormed
      simp only [encTm, evalTm]
      rw [seqIH wellFormed.1, indexIH wellFormed.2]
  | len seq ih =>
      intro wellFormed
      simp only [encTm, evalTm]
      rw [ih wellFormed]
  | cast to term ih =>
      intro wellFormed
      simp only [encTm, evalTm]
      rw [ih wellFormed]
  | idxOp term offset ih =>
      intro wellFormed
      simp only [encTm, evalTm]
      rw [ih wellFormed]
  | mul left right leftIH rightIH =>
      intro wellFormed
      simp only [wfTm, Bool.and_eq_true] at wellFormed
      simp only [encTm, evalTm]
      rw [leftIH wellFormed.1, rightIH wellFormed.2]
  | app1 arg result fn term ih =>
      intro wellFormed
      simp only [encTm, evalTm]
      rw [ih wellFormed]

theorem eval_encAtom (M : Model) (depth : Nat) (ρ σ : Valuation M)
    (agree : TypedAgree M depth ρ σ) (atom : Atom)
    (wellFormed : wfAtom depth atom = true) :
    evalAtom M σ (encAtom depth atom) = evalAtom M ρ atom := by
  cases atom with
  | qfree id expr => rfl
  | rel relation left right =>
      simp only [wfAtom, Bool.and_eq_true] at wellFormed
      cases relation <;>
        simp only [encAtom, evalAtom, eval_encTm M depth ρ σ agree left wellFormed.1,
          eval_encTm M depth ρ σ agree right wellFormed.2]

/-- The classifier encoder is faithful in the typed model. Relations, arrays,
    functions, and literals are evaluated here; no atom oracle remains. -/
theorem typed_ref_sound (M : Model) (formula : Frm) :
    ∀ (depth : Nat) (ρ σ : Valuation M),
      TypedAgree M depth ρ σ → wfFrm depth formula = true →
      evalTok M (sencodeAt depth formula) σ = evalFrm M formula ρ := by
  induction formula with
  | atom atom =>
      intro depth ρ σ agree wellFormed
      exact eval_encAtom M depth ρ σ agree atom wellFormed
  | neg formula ih =>
      intro depth ρ σ agree wellFormed
      simp only [sencodeAt, evalTok, evalFrm, ih depth ρ σ agree wellFormed]
  | conj left right leftIH rightIH =>
      intro depth ρ σ agree wellFormed
      simp only [wfFrm, Bool.and_eq_true] at wellFormed
      simp only [sencodeAt, evalTok, evalFrm,
        leftIH depth ρ σ agree wellFormed.1, rightIH depth ρ σ agree wellFormed.2]
  | disj left right leftIH rightIH =>
      intro depth ρ σ agree wellFormed
      simp only [wfFrm, Bool.and_eq_true] at wellFormed
      simp only [sencodeAt, evalTok, evalFrm,
        leftIH depth ρ σ agree wellFormed.1, rightIH depth ρ σ agree wellFormed.2]
  | imp left right leftIH rightIH =>
      intro depth ρ σ agree wellFormed
      simp only [wfFrm, Bool.and_eq_true] at wellFormed
      simp only [sencodeAt, evalTok, evalFrm,
        leftIH depth ρ σ agree wellFormed.1, rightIH depth ρ σ agree wellFormed.2]
  | all sort body ih =>
      intro depth ρ σ agree wellFormed
      simp only [sencodeAt, evalTok, evalFrm]
      apply congrArg (List.all (M.enum sort))
      funext value
      exact ih (depth + 1) _ _
        (TypedAgree_upd M depth ρ σ sort value agree) wellFormed
  | ex sort body ih =>
      intro depth ρ σ agree wellFormed
      simp only [sencodeAt, evalTok, evalFrm]
      apply congrArg (List.any (M.enum sort))
      funext value
      exact ih (depth + 1) _ _
        (TypedAgree_upd M depth ρ σ sort value agree) wellFormed

theorem typed_ref_sound_sentence (M : Model) (formula : Frm)
    (wellFormed : wfFrm 0 formula = true) (ρ σ : Valuation M) :
    evalTok M (sencode formula) σ = evalFrm M formula ρ :=
  typed_ref_sound M formula 0 ρ σ
    (fun index inScope => absurd inScope (Nat.not_lt_zero index)) wellFormed

#print axioms typed_ref_sound
#print axioms typed_ref_sound_sentence

end Thermite.Strat
