/-
  Finite, sort-indexed Herbrand universes for S₂.0 reconstruction.

  Function symbols include declared unary functions, built-in read/len/cast
  operations after reification, and Skolem symbols. A topological sort of their
  sort-flow graph lets `buildUniverse` close each result sort once. The final
  Boolean checker is intentionally independent of the builder: it verifies
  seeds, well-sortedness, non-empty sorts, and exhaustive function closure.
  `complete_of_checkUniverse` then proves that no finitely generated ground term
  can be missing.
-/
import Thermite.Strat.Substitution
import Mathlib.Data.List.Defs

namespace Thermite.Strat.Cls

inductive GroundFunctionKind where
  | source (id : Nat)
  | read (elem : Sort₂)
  | len
  | seqDiff (elem : Sort₂)
  | cast
  | offset (value : Int)
  | mul
  | skolem (id : Nat)
  deriving DecidableEq, Repr, Hashable

structure GroundFunction where
  kind : GroundFunctionKind
  arguments : List Sort₂
  result : Sort₂
  deriving DecidableEq, Repr, Hashable

inductive GroundConstant where
  | source (id : Nat)
  | literal (value : ScalarValue)
  | inhabitant
  deriving DecidableEq, Repr, Hashable

mutual
  inductive GroundTerm where
    | constant (sort : Sort₂) (constant : GroundConstant)
    | app (fn : GroundFunction) (arguments : GroundArguments)
    deriving Repr

  inductive GroundArguments where
    | nil
    | cons (head : GroundTerm) (tail : GroundArguments)
    deriving Repr
end

deriving instance DecidableEq for GroundTerm
deriving instance DecidableEq for GroundArguments
deriving instance Hashable for GroundTerm
deriving instance Hashable for GroundArguments

def GroundArguments.toList : GroundArguments → List GroundTerm
  | .nil => []
  | .cons head tail => head :: tail.toList

def GroundArguments.ofList : List GroundTerm → GroundArguments
  | [] => .nil
  | head :: tail => .cons head (GroundArguments.ofList tail)

@[simp]
theorem GroundArguments.toList_ofList (arguments : List GroundTerm) :
    (GroundArguments.ofList arguments).toList = arguments := by
  induction arguments <;> simp_all [GroundArguments.ofList, GroundArguments.toList]

@[simp]
theorem GroundArguments.ofList_toList :
    ∀ arguments : GroundArguments,
      GroundArguments.ofList arguments.toList = arguments
  | .nil => rfl
  | .cons head tail => by
      simp [GroundArguments.ofList, GroundArguments.toList,
        GroundArguments.ofList_toList tail]

def GroundTerm.appList (fn : GroundFunction)
    (arguments : List GroundTerm) : GroundTerm :=
  .app fn (GroundArguments.ofList arguments)

def GroundTerm.sortOf : GroundTerm → Sort₂
  | .constant sort _ => sort
  | .app fn _ => fn.result

mutual
  /-- A term together with every argument term occurring below it. -/
  def GroundTerm.subterms : GroundTerm → List GroundTerm
    | term@(.constant _ _) => [term]
    | term@(.app _ arguments) => term :: arguments.subterms

  def GroundArguments.subterms : GroundArguments → List GroundTerm
    | .nil => []
    | .cons head tail => head.subterms ++ tail.subterms
end

mutual
  def GroundTerm.depth : GroundTerm → Nat
    | .constant _ _ => 0
    | .app _ arguments => arguments.depth

  def GroundArguments.depth : GroundArguments → Nat
    | .nil => 1
    | .cons head tail => max (head.depth + 1) tail.depth
end

mutual
  def GroundTerm.wellSorted : GroundTerm → Bool
    | .constant _ _ => true
    | .app fn arguments =>
        decide (arguments.toList.map GroundTerm.sortOf = fn.arguments)
          && arguments.wellSorted

  def GroundArguments.wellSorted : GroundArguments → Bool
    | .nil => true
    | .cons head tail => head.wellSorted && tail.wellSorted
end

abbrev GroundUniverse := List GroundTerm

def termsOf (ground : GroundUniverse) (sort : Sort₂) : List GroundTerm :=
  ground.filter (fun term => decide (term.sortOf = sort))

/-- All sort-correct tuples over the current ground. -/
def argumentTuples (ground : GroundUniverse) :
    List Sort₂ → List (List GroundTerm)
  | [] => [[]]
  | sort :: rest =>
      (termsOf ground sort).flatMap fun head =>
        (argumentTuples ground rest).map (head :: ·)

def functionInstances (fn : GroundFunction)
    (ground : GroundUniverse) : List GroundTerm :=
  (argumentTuples ground fn.arguments).map (GroundTerm.appList fn)

def closeSort (signature : List GroundFunction) (sort : Sort₂)
    (ground : GroundUniverse) : GroundUniverse :=
  (ground ++
    (signature.filter (fun fn => decide (fn.result = sort))).flatMap
      (fun fn => functionInstances fn ground)).eraseDups

/-- Topological closure terminates structurally on the finite sort order. -/
def buildUniverse (signature : List GroundFunction)
    (order : List Sort₂) (seeds : GroundUniverse) : GroundUniverse :=
  order.foldl (fun ground sort => closeSort signature sort ground)
    seeds.eraseDups

def signatureSorts (signature : List GroundFunction)
    (seeds : GroundUniverse) : List Sort₂ :=
  ((signature.flatMap fun fn => fn.result :: fn.arguments)
    ++ seeds.map GroundTerm.sortOf).eraseDups

def withSequenceDiffFunctions (signature : List GroundFunction)
    (seeds : GroundUniverse) : List GroundFunction :=
  let sequenceElements :=
    (signatureSorts signature seeds).filterMap fun sort =>
      match sort with
      | .seq elem => some elem
      | _ => none
  (signature ++ sequenceElements.map (fun elem =>
    ({ kind := .seqDiff elem
       arguments := [.seq elem, .seq elem]
       result := usizeS } : GroundFunction))).eraseDups

/-- `left` occurs strictly before `right`. -/
def before (left right : Sort₂) : List Sort₂ → Bool
  | [] => false
  | head :: tail =>
      if head = left then decide (right ∈ tail) else before left right tail

def orderValid (signature : List GroundFunction) (sorts order : List Sort₂) : Bool :=
  decide order.Nodup
    && sorts.all (· ∈ order)
    && order.all (· ∈ sorts)
    && signature.all (fun fn =>
      fn.arguments.all fun argument => before argument fn.result order)

/-- A deterministic, brute-force topological order. S₂.0 signatures are small;
    replay values simplicity and auditability over a second graph algorithm. -/
def topologicalOrder? (signature : List GroundFunction)
    (seeds : GroundUniverse) : Option (List Sort₂) :=
  let sorts := signatureSorts signature seeds
  sorts.permutations.find? (orderValid signature sorts)

def withInhabitants (sorts : List Sort₂)
    (seeds : GroundUniverse) : GroundUniverse :=
  (seeds ++ sorts.map (fun sort =>
    GroundTerm.constant sort GroundConstant.inhabitant)).eraseDups

/-- The finite grounding data emitted by the untrusted builder. Replay does not
    trust either field: it checks the sort order, recomputes the universe, and
    independently checks closure. -/
structure GroundingCertificate where
  order : List Sort₂
  ground : GroundUniverse
  deriving Repr

/-- Recheck a candidate ground from first principles. This is the boundary
    consumed by reconstruction; a builder bug becomes `false`, never a proof. -/
def checkUniverse (signature : List GroundFunction) (sorts : List Sort₂)
    (seeds ground : GroundUniverse) : Bool :=
  seeds.all (· ∈ ground)
    && ground.all GroundTerm.wellSorted
    && sorts.all (fun sort => !(termsOf ground sort).isEmpty)
    && signature.all (fun fn =>
      (functionInstances fn ground).all (· ∈ ground))

/-- Recompute every verdict-bearing part of a grounding certificate. The
    equality check binds the certificate to the deterministic builder; the
    second check deliberately re-establishes closure without appealing to that
    builder's implementation. -/
def verifyGrounding (signature : List GroundFunction)
    (seeds : GroundUniverse) (certificate : GroundingCertificate) : Bool :=
  let sorts := signatureSorts signature seeds
  let seeded := withInhabitants sorts seeds
  orderValid signature sorts certificate.order
    && decide
      (certificate.ground =
        buildUniverse signature certificate.order seeded)
    && checkUniverse signature sorts seeded certificate.ground

theorem mem_termsOf_of_mem (ground : GroundUniverse) (term : GroundTerm)
    (member : term ∈ ground) :
    term ∈ termsOf ground term.sortOf := by
  simp [termsOf, member]

theorem mem_argumentTuples_of (ground : GroundUniverse) :
    ∀ (sorts : List Sort₂) (arguments : List GroundTerm),
      arguments.map GroundTerm.sortOf = sorts →
      (∀ term ∈ arguments, term ∈ ground) →
      arguments ∈ argumentTuples ground sorts := by
  intro sorts
  induction sorts with
  | nil =>
      intro arguments sorted members
      have empty : arguments = [] := by
        cases arguments <;> simp_all
      subst empty
      simp [argumentTuples]
  | cons sort rest ih =>
      intro arguments sorted members
      cases arguments with
      | nil => simp at sorted
      | cons head tail =>
          simp only [List.map_cons, List.cons.injEq] at sorted
          apply List.mem_flatMap.mpr
          refine ⟨head, ?_, ?_⟩
          · simp [termsOf, members head (by simp), sorted.1]
          · apply List.mem_map.mpr
            refine ⟨tail, ih tail sorted.2 ?_, rfl⟩
            intro term member
            exact members term (by simp [member])

theorem app_mem_functionInstances_of (fn : GroundFunction)
    (ground : GroundUniverse) (arguments : List GroundTerm)
    (sorted : arguments.map GroundTerm.sortOf = fn.arguments)
    (members : ∀ term ∈ arguments, term ∈ ground) :
    GroundTerm.appList fn arguments ∈ functionInstances fn ground := by
  apply List.mem_map.mpr
  exact ⟨arguments, mem_argumentTuples_of ground fn.arguments arguments sorted members, rfl⟩

structure CompleteUniverse (signature : List GroundFunction)
    (sorts : List Sort₂) (seeds ground : GroundUniverse) : Prop where
  seed_mem : ∀ term ∈ seeds, term ∈ ground
  wellSorted : ∀ term ∈ ground, term.wellSorted = true
  inhabited : ∀ sort ∈ sorts, ∃ term ∈ ground, term.sortOf = sort
  closed : ∀ fn ∈ signature, ∀ arguments : List GroundTerm,
    arguments.map GroundTerm.sortOf = fn.arguments →
    (∀ term ∈ arguments, term ∈ ground) →
    GroundTerm.appList fn arguments ∈ ground

theorem completeUniverse_of_check {signature : List GroundFunction}
    {sorts : List Sort₂} {seeds ground : GroundUniverse}
    (checked : checkUniverse signature sorts seeds ground = true) :
    CompleteUniverse signature sorts seeds ground := by
  simp only [checkUniverse, Bool.and_eq_true, List.all_eq_true] at checked
  refine {
    seed_mem := fun term member =>
      of_decide_eq_true (checked.1.1.1 term member)
    wellSorted := checked.1.1.2
    inhabited := ?_
    closed := ?_
  }
  · intro sort sortMember
    have nonempty := checked.1.2 sort sortMember
    have notNil : termsOf ground sort ≠ [] := by
      intro empty
      simp [empty] at nonempty
    obtain ⟨term, member⟩ :=
      List.exists_mem_of_ne_nil (termsOf ground sort) notNil
    exact ⟨term, (List.mem_filter.mp member).1,
      of_decide_eq_true (List.mem_filter.mp member).2⟩
  · intro fn fnMember arguments sorted members
    have allInstances := checked.2 fn fnMember
    exact of_decide_eq_true <|
      allInstances _ (app_mem_functionInstances_of fn ground arguments sorted members)

/-- Terms generated within `fuel` applications. This executable definition is
    the finite induction measure behind Herbrand completeness. -/
def generatedWithin (signature : List GroundFunction)
    (seeds : GroundUniverse) : Nat → GroundTerm → Bool
  | 0, _ => false
  | fuel + 1, term =>
      decide (term ∈ seeds) ||
        match term with
        | .constant _ _ => false
        | .app fn packed =>
            let arguments := packed.toList
            decide (fn ∈ signature)
              && decide (arguments.map GroundTerm.sortOf = fn.arguments)
              && arguments.all (generatedWithin signature seeds fuel)

theorem generated_mem_of_complete
    {signature : List GroundFunction} {sorts : List Sort₂}
    {seeds ground : GroundUniverse}
    (complete : CompleteUniverse signature sorts seeds ground) :
    ∀ fuel term,
      generatedWithin signature seeds fuel term = true →
      term ∈ ground := by
  intro fuel
  induction fuel with
  | zero =>
      intro term generated
      simp [generatedWithin] at generated
  | succ fuel ih =>
      intro term generated
      simp only [generatedWithin, Bool.or_eq_true] at generated
      rcases generated with seed | built
      · exact complete.seed_mem term (of_decide_eq_true seed)
      · cases term with
        | constant sort constant => simp at built
        | app fn packed =>
            let arguments := packed.toList
            simp only [Bool.and_eq_true, List.all_eq_true] at built
            have members : ∀ argument ∈ arguments, argument ∈ ground := by
              intro argument member
              exact ih argument (built.2 argument member)
            have closed := complete.closed fn (of_decide_eq_true built.1.1)
              arguments (of_decide_eq_true built.1.2) members
            simpa [GroundTerm.appList, arguments] using closed

/-- Main no-omission theorem. Every finite well-sorted term generated from the
    seeds and admitted functions occurs in a checked ground. -/
theorem complete_of_checkUniverse
    {signature : List GroundFunction} {sorts : List Sort₂}
    {seeds ground : GroundUniverse}
    (checked : checkUniverse signature sorts seeds ground = true)
    (term : GroundTerm)
    (generated :
      generatedWithin signature seeds (term.depth + 1) term = true) :
    term ∈ ground :=
  generated_mem_of_complete (completeUniverse_of_check checked)
    (term.depth + 1) term generated

/-- A verified certificate is bound to the deterministic topological build and
    yields the independent no-omission invariant used by instantiation. -/
theorem complete_of_verifyGrounding
    {signature : List GroundFunction} {seeds : GroundUniverse}
    {certificate : GroundingCertificate}
    (verified : verifyGrounding signature seeds certificate = true) :
    CompleteUniverse signature (signatureSorts signature seeds)
      (withInhabitants (signatureSorts signature seeds) seeds)
      certificate.ground := by
  simp only [verifyGrounding, Bool.and_eq_true] at verified
  exact completeUniverse_of_check verified.2

/-- Tampering with the recorded ground cannot be hidden behind a closure check:
    successful replay identifies it with the ground recomputed from the recorded
    topological order. -/
theorem ground_eq_build_of_verifyGrounding
    {signature : List GroundFunction} {seeds : GroundUniverse}
    {certificate : GroundingCertificate}
    (verified : verifyGrounding signature seeds certificate = true) :
    certificate.ground =
      buildUniverse signature certificate.order
        (withInhabitants (signatureSorts signature seeds) seeds) := by
  simp only [verifyGrounding, Bool.and_eq_true] at verified
  exact of_decide_eq_true verified.1.2

#print axioms complete_of_checkUniverse
#print axioms complete_of_verifyGrounding
#print axioms ground_eq_build_of_verifyGrounding

end Thermite.Strat.Cls
