/-
  Checked finite instantiation for S₂.0.

  This module turns the normalized prenex formula into a quantifier-free ground
  formula. Universal binders are expanded over the independently checked
  universe. Each existential binder becomes one Skolem function whose arguments
  are exactly the preceding universal binders. Ground atoms retain the stable
  qfree ID from the source bridge, so propositional numbering never depends on
  traversal order after normalization.
-/
import Thermite.Strat.Grounding
import Thermite.Strat.Skolem

namespace Thermite.Strat.Cls

inductive GroundAtom where
  | qfree (id : Nat)
  | rel (relation : Rel) (left right : GroundTerm)
  deriving DecidableEq, Repr, Hashable

/-- A quantifier-free Boolean formula over ground theory atoms. -/
inductive GroundFrm where
  | const (value : Bool)
  | atom (atom : GroundAtom)
  | neg (formula : GroundFrm)
  | conj (left right : GroundFrm)
  | disj (left right : GroundFrm)
  deriving DecidableEq, Repr, Hashable

def GroundAtom.terms : GroundAtom → List GroundTerm
  | .qfree _ => []
  | .rel _ left right => left.subterms ++ right.subterms

def GroundFrm.terms : GroundFrm → List GroundTerm
  | GroundFrm.const _ => []
  | GroundFrm.atom groundAtom => groundAtom.terms
  | GroundFrm.neg formula => formula.terms
  | GroundFrm.conj left right | GroundFrm.disj left right =>
      left.terms ++ right.terms

/-- Sort-check the canonical source IR against its de Bruijn binder context.
    `wfFrm` checks scope only; reconstruction additionally needs the sort
    annotation on each occurrence to agree with the binder it addresses. -/
def wellSortedTm (context : List Sort₂) : Tm → Bool
  | .var sort index => decide (context[index]? = some sort)
  | .const _ _ | .lit _ _ => true
  | .read elem sequence index =>
      decide (sequence.sortOf = .seq elem)
        && decide (index.sortOf = usizeS)
        && wellSortedTm context sequence
        && wellSortedTm context index
  | .len sequence =>
      (match sequence.sortOf with
        | .seq _ => true
        | _ => false)
        && wellSortedTm context sequence
  | .cast _ term | .idxOp term _ =>
      wellSortedTm context term
  | .mul left right =>
      decide (left.sortOf = right.sortOf)
        && wellSortedTm context left
        && wellSortedTm context right
  | .app1 argument _ _ term =>
      decide (term.sortOf = argument)
        && wellSortedTm context term

def wellSortedAtom (context : List Sort₂) : Atom → Bool
  | .qfree _ _ => true
  | .rel _ left right =>
      decide (left.sortOf = right.sortOf)
        && wellSortedTm context left
        && wellSortedTm context right

def wellSortedFrm (context : List Sort₂) : Frm → Bool
  | .atom atom => wellSortedAtom context atom
  | .neg formula => wellSortedFrm context formula
  | .conj left right | .disj left right | .imp left right =>
      wellSortedFrm context left && wellSortedFrm context right
  | .all sort body | .ex sort body =>
      wellSortedFrm (sort :: context) body

def GroundFrm.implies (left right : GroundFrm) : GroundFrm :=
  .disj (.neg left) right

def GroundFrm.conjoin : List GroundFrm → GroundFrm
  | [] => .const true
  | [formula] => formula
  | formula :: next :: rest =>
      .conj formula (GroundFrm.conjoin (next :: rest))

def GroundFrm.disjoin : List GroundFrm → GroundFrm
  | [] => .const false
  | formula :: rest => .disj formula (GroundFrm.disjoin rest)

def groundTermAt (environment : List GroundTerm) (sort : Sort₂)
    (index : Nat) : GroundTerm :=
  environment[index]?.getD (.constant sort .inhabitant)

/-- Reify one source term after all of its binders have been assigned ground
    terms. Interpreted operations remain visible as tagged function symbols so
    theory-clause replay can recognize them. -/
def groundTm (environment : List GroundTerm) : Tm → GroundTerm
  | .var sort index => groundTermAt environment sort index
  | .const sort id => .constant sort (.source id)
  | .lit sort value => .constant sort (.literal value)
  | .read elem sequence index =>
      let sequence := groundTm environment sequence
      let index := groundTm environment index
      .appList
        { kind := .read elem
          arguments := [sequence.sortOf, index.sortOf]
          result := elem }
        [sequence, index]
  | .len sequence =>
      let sequence := groundTm environment sequence
      .appList
        { kind := .len, arguments := [sequence.sortOf], result := usizeS }
        [sequence]
  | .cast target term =>
      let term := groundTm environment term
      .appList
        { kind := .cast, arguments := [term.sortOf], result := target }
        [term]
  | .idxOp term offset =>
      let term := groundTm environment term
      .appList
        { kind := .offset offset
          arguments := [term.sortOf]
          result := term.sortOf }
        [term]
  | .mul left right =>
      let left := groundTm environment left
      let right := groundTm environment right
      .appList
        { kind := .mul
          arguments := [left.sortOf, right.sortOf]
          result := left.sortOf }
        [left, right]
  | .app1 argument result id term =>
      let term := groundTm environment term
      .appList
        { kind := .source id, arguments := [argument], result }
        [term]

def groundAtom (environment : List GroundTerm) : Atom → GroundAtom
  | .qfree id _ => .qfree id
  | .rel relation left right =>
      .rel relation (groundTm environment left) (groundTm environment right)

/-- Reify the quantifier-free matrix. A binder here is a malformed prenex
    certificate and is rejected instead of being silently ignored. -/
def groundMatrix (environment : List GroundTerm) : Frm → Option GroundFrm
  | .atom atom => some (.atom (groundAtom environment atom))
  | .neg formula => return .neg (← groundMatrix environment formula)
  | .conj left right =>
      return .conj (← groundMatrix environment left)
        (← groundMatrix environment right)
  | .disj left right =>
      return .disj (← groundMatrix environment left)
        (← groundMatrix environment right)
  | .imp left right =>
      return GroundFrm.implies (← groundMatrix environment left)
        (← groundMatrix environment right)
  | .all _ _ | .ex _ _ => none

def skolemFunction (id : Nat) (universalsRev : List GroundTerm)
    (result : Sort₂) : GroundFunction :=
  { kind := .skolem id
    arguments := universalsRev.reverse.map GroundTerm.sortOf
    result }

/-- Expand a prenex prefix. `universalsRev` records precisely the universal
    terms to the left of the current binder; existential terms therefore cannot
    depend on later binders or on preceding existential choices. -/
def instantiatePrefix (ground : GroundUniverse) :
    Prefix → Frm → List GroundTerm → List GroundTerm → Nat → Option GroundFrm
  | [], matrix, environment, _, _ => groundMatrix environment matrix
  | (BinderKind.all, sort) :: rest, matrix,
      environment, universalsRev, nextSkolem =>
      (termsOf ground sort).mapM (fun term =>
        instantiatePrefix ground rest matrix
          (term :: environment) (term :: universalsRev) nextSkolem)
        |>.map GroundFrm.conjoin
  | (BinderKind.ex, sort) :: rest, matrix,
      environment, universalsRev, nextSkolem =>
      let witness := GroundTerm.appList
        (skolemFunction nextSkolem universalsRev sort)
        universalsRev.reverse
      instantiatePrefix ground rest matrix
        (witness :: environment) universalsRev (nextSkolem + 1)

def normalizedPrefix (formula : Frm) : Prefix × Frm :=
  peel (prenex (nnf formula))

def Prefix.toContext (binders : Prefix) : List Sort₂ :=
  (binders.map Prod.snd).reverse

def Frm.qfreeIds : Frm → List Nat
  | .atom (.qfree id _) => [id]
  | .atom (.rel _ _ _) => []
  | .neg formula => formula.qfreeIds
  | .conj left right | .disj left right | .imp left right =>
      left.qfreeIds ++ right.qfreeIds
  | .all _ body | .ex _ body => body.qfreeIds

def instantiate (ground : GroundUniverse) (formula : Frm) : Option GroundFrm :=
  let normalized := normalizedPrefix formula
  instantiatePrefix ground normalized.1 normalized.2 [] [] 0

/-- Function symbols appearing below a binder. Unlike the old classifier's
    universal-only occurrence test, this predicate also sees existential
    variables: after Skolemization they may carry an earlier universal
    dependency through another function symbol. -/
def hasBoundVariable : Tm → Bool
  | .var _ _ => true
  | .const _ _ | .lit _ _ => false
  | .read _ sequence index =>
      hasBoundVariable sequence || hasBoundVariable index
  | .len sequence => hasBoundVariable sequence
  | .cast _ term | .idxOp term _ | .app1 _ _ _ term =>
      hasBoundVariable term
  | .mul left right => hasBoundVariable left || hasBoundVariable right

def closureFunctionsTm : Tm → List GroundFunction
  | .var _ _ | .const _ _ | .lit _ _ => []
  | .read elem sequence index =>
      let nested := closureFunctionsTm sequence ++ closureFunctionsTm index
      if hasBoundVariable index then
        { kind := .read elem
          arguments := [sequence.sortOf, index.sortOf]
          result := elem } :: nested
      else nested
  | .len sequence =>
      let nested := closureFunctionsTm sequence
      if hasBoundVariable sequence then
        { kind := .len, arguments := [sequence.sortOf], result := usizeS } :: nested
      else nested
  | .cast target term =>
      let nested := closureFunctionsTm term
      if hasBoundVariable term then
        { kind := .cast, arguments := [term.sortOf], result := target } :: nested
      else nested
  | .idxOp term _ => closureFunctionsTm term
  | .mul left right => closureFunctionsTm left ++ closureFunctionsTm right
  | .app1 argument result id term =>
      let nested := closureFunctionsTm term
      if hasBoundVariable term then
        { kind := .source id, arguments := [argument], result } :: nested
      else nested

def closureFunctionsAtom : Atom → List GroundFunction
  | .qfree _ _ => []
  | .rel _ left right => closureFunctionsTm left ++ closureFunctionsTm right

def closureFunctionsFrm : Frm → List GroundFunction
  | .atom atom => closureFunctionsAtom atom
  | .neg formula => closureFunctionsFrm formula
  | .conj left right | .disj left right | .imp left right =>
      closureFunctionsFrm left ++ closureFunctionsFrm right
  | .all _ body | .ex _ body => closureFunctionsFrm body

def skolemFunctions : Prefix → List GroundTerm → Nat → List GroundFunction
  | [], _, _ => []
  | (BinderKind.all, sort) :: rest, universalsRev, next =>
      skolemFunctions rest
        (.constant sort .inhabitant :: universalsRev) next
  | (BinderKind.ex, sort) :: rest, universalsRev, next =>
      skolemFunction next universalsRev sort ::
        skolemFunctions rest universalsRev (next + 1)

def seedTermsTm : Tm → List GroundTerm
  | .var _ _ => []
  | .const sort id => [.constant sort (.source id)]
  | .lit sort value => [.constant sort (.literal value)]
  | .read _ sequence index => seedTermsTm sequence ++ seedTermsTm index
  | .len sequence | .cast _ sequence | .idxOp sequence _
    | .app1 _ _ _ sequence => seedTermsTm sequence
  | .mul left right => seedTermsTm left ++ seedTermsTm right

def seedTermsAtom : Atom → List GroundTerm
  | .qfree _ _ => []
  | .rel _ left right => seedTermsTm left ++ seedTermsTm right

def seedTermsFrm : Frm → List GroundTerm
  | .atom atom => seedTermsAtom atom
  | .neg formula => seedTermsFrm formula
  | .conj left right | .disj left right | .imp left right =>
      seedTermsFrm left ++ seedTermsFrm right
  | .all _ body | .ex _ body => seedTermsFrm body

/-- Binder sorts are explicit seeds. Without them, a formula containing only a
    quantified variable could produce an empty `signatureSorts` list and turn a
    universal into a vacuous empty conjunction. -/
def binderSeeds : Frm → List GroundTerm
  | .atom _ => []
  | .neg formula => binderSeeds formula
  | .conj left right | .disj left right | .imp left right =>
      binderSeeds left ++ binderSeeds right
  | .all sort body | .ex sort body =>
      .constant sort .inhabitant :: binderSeeds body

def reconstructionSeeds (formula : Frm) : GroundUniverse :=
  (seedTermsFrm formula ++ binderSeeds formula).eraseDups

def reconstructionSignature (formula : Frm) : List GroundFunction :=
  let normalized := normalizedPrefix formula
  let seeds := reconstructionSeeds formula
  withSequenceDiffFunctions
    (closureFunctionsFrm normalized.2 ++
      skolemFunctions normalized.1 [] 0).eraseDups
    seeds

structure InstantiationCertificate where
  grounding : GroundingCertificate
  formula : GroundFrm
  deriving Repr

/-- Deterministic untrusted-side builder. Replay still recomputes every field in
    `verifyInstantiation`; the fallbacks deliberately produce a certificate
    that fails that check when no topological order or instantiation exists. -/
def buildInstantiation (source : Frm) : InstantiationCertificate :=
  let signature := reconstructionSignature source
  let seeds := reconstructionSeeds source
  let sorts := signatureSorts signature seeds
  let order := (topologicalOrder? signature seeds).getD []
  let ground :=
    buildUniverse signature order (withInhabitants sorts seeds)
  let formula := (instantiate ground source).getD (.const false)
  { grounding := { order, ground }, formula }

/-- Replay first verifies and recomputes the universe, then recomputes every
    quantifier instance and the complete quantifier-free ground formula. -/
def verifyInstantiation (source : Frm)
    (certificate : InstantiationCertificate) : Bool :=
  let signature := reconstructionSignature source
  let seeds := reconstructionSeeds source
  let normalized := normalizedPrefix source
  wellSortedFrm [] source
    && wellSortedFrm normalized.1.toContext normalized.2
    && decide normalized.2.qfreeIds.Nodup
    && verifyGrounding signature seeds certificate.grounding
    && decide
      (instantiate certificate.grounding.ground source =
        some certificate.formula)

theorem instantiation_eq_of_verify
    {source : Frm} {certificate : InstantiationCertificate}
    (verified : verifyInstantiation source certificate = true) :
    instantiate certificate.grounding.ground source =
      some certificate.formula := by
  simp only [verifyInstantiation, Bool.and_eq_true] at verified
  exact of_decide_eq_true verified.2

theorem wellSorted_of_verifyInstantiation
    {source : Frm} {certificate : InstantiationCertificate}
    (verified : verifyInstantiation source certificate = true) :
    wellSortedFrm [] source = true := by
  simp only [verifyInstantiation, Bool.and_eq_true] at verified
  exact verified.1.1.1.1

theorem normalizedWellSorted_of_verifyInstantiation
    {source : Frm} {certificate : InstantiationCertificate}
    (verified : verifyInstantiation source certificate = true) :
    let normalized := normalizedPrefix source
    wellSortedFrm normalized.1.toContext normalized.2 = true := by
  simp only [verifyInstantiation, Bool.and_eq_true] at verified
  exact verified.1.1.1.2

theorem qfreeIds_nodup_of_verifyInstantiation
    {source : Frm} {certificate : InstantiationCertificate}
    (verified : verifyInstantiation source certificate = true) :
    (normalizedPrefix source).2.qfreeIds.Nodup := by
  simp only [verifyInstantiation, Bool.and_eq_true] at verified
  exact of_decide_eq_true verified.1.1.2

theorem completeGrounding_of_verifyInstantiation
    {source : Frm} {certificate : InstantiationCertificate}
    (verified : verifyInstantiation source certificate = true) :
    CompleteUniverse (reconstructionSignature source)
      (signatureSorts (reconstructionSignature source)
        (reconstructionSeeds source))
      (withInhabitants
        (signatureSorts (reconstructionSignature source)
          (reconstructionSeeds source))
        (reconstructionSeeds source))
      certificate.grounding.ground := by
  apply complete_of_verifyGrounding
  simp only [verifyInstantiation, Bool.and_eq_true] at verified
  exact verified.1.2

#print axioms instantiation_eq_of_verify
#print axioms wellSorted_of_verifyInstantiation
#print axioms normalizedWellSorted_of_verifyInstantiation
#print axioms qfreeIds_nodup_of_verifyInstantiation
#print axioms completeGrounding_of_verifyInstantiation

end Thermite.Strat.Cls
