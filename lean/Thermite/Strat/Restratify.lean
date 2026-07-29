/-
  Thermite/Strat/Restratify.lean — the restratification rewrite and its T4-R
  metatheory: `restrat` (the cycle-breaking rewrite), `Side` (the implication
  side obligation), and the four theorems `restrat_conservative`,
  `restrat_admits`, `restrat_complete`, `side_admitted`.

  Governing design: `.design/stage2-stratified-cage.md` REQ-7 / AC-7 (spec of record:
  the stage-2 metatheory sketch, GH issue #2, §6 "restratification"). Builds on REQ-3's
  classifier surface `Thermite.Strat.Cls.Frm` and its STRUCTURAL denotation `fdenote`
  (`Strat/Nnf.lean`) + the admission classifier `admitted` (`Strat/Fragment.lean`).

  THE REWRITE (metatheory §6, the kv-alternation repair).  The motivating inadmissible
  formula is the kv alternation cycle (`Strat/Fragment.lean` `ex_kvCycle`):

      φ  =  (∀k:Key. ∃v:Value. v = k)  ∧  (∀v:Value. ∃k:Key. k = v)
            └──────────  A  ──────────┘     └──────────  B  ──────────┘

  whose sort graph has the E1 edges `Key → Value` (from A) AND `Value → Key` (from B),
  a cycle — so `admitted φ = false`.  The cycle exists ONLY because both edges sit in the
  same formula.  `restrat` breaks it by EXCISING the cycle-closing conjunct B, replacing
  it with a fresh OPAQUE boolean abstraction `p` (a `qfree` leaf — opaque to the
  classifier, contributing no sorts and no edges, the §1.2 `QFree φ₀` leaf):

      φ' = restrat φ  =  A  ∧  p          -- only the `Key → Value` edge ⇒ acyclic ⇒ ADMITTED

  and emits the IMPLICATION SIDE OBLIGATION that the abstraction soundly stands for B:

      Side(φ', φ)  =  p ⇒ B               -- only the `Value → Key` edge ⇒ acyclic ⇒ ADMITTED

  Neither φ' nor Side carries BOTH edges, so each is admissible on its own — yet together
  they re-establish φ:  `(A ∧ p) ∧ (p ⇒ B)  ⊢  A ∧ B = φ`  (modus ponens on `p`).

  R-SIDE-1 (the load-bearing soundness discipline).  A certificate of φ' ALONE never
  counts for φ: `p` is a FRESH, UNCONSTRAINED abstraction, so `A ∧ p` is satisfied
  trivially by `p := true` WITHOUT B holding.  Only a SEPARATELY DISCHARGED `Side`
  (`p ⇒ B`, itself in-cage) licenses the inference to φ.  `restrat_conservative` is
  exactly the bridge that consumes BOTH; `PinRestratDropSide` exhibits the
  mis-certification that dropping `Side` would permit.

  Core-Lean-only, axiom-clean: `restrat_conservative` is a Boolean tautology over the
  three structural denotations; the admissibility theorems are `decide` (kernel), never
  `native_decide`.
-/
import Thermite.Strat.Fragment

namespace Thermite.Strat.Cls

/-! ## The rewrite, the abstraction, and the side obligation -/

/-- The fresh opaque boolean abstraction leaf standing in for an excised sub-formula.
    A `qfree` atom is opaque to the classifier (no sorts, no graph edges — `edgesAtom`
    returns `[]`), so substituting it for a cycle-closing conjunct deletes that
    conjunct's edges from the sort graph. -/
def absLeaf (e : Thermite.Expr) : Frm := .atom (.qfree e)

/-- The restratify rewrite (metatheory §6).  On a conjunction `A ∧ B` whose right
    conjunct `B` closes the alternation cycle, excise `B` and replace it with the fresh
    abstraction `p = absLeaf e` — yielding the admissible `A ∧ p`.  Any other shape is
    returned unchanged (the kv repair is the §6 worked instance; the classifier reports
    a cycle whose witnessing conjunct selects `e`/the split). -/
def restrat (e : Thermite.Expr) : Frm → Frm
  | .conj A _ => .conj A (absLeaf e)
  | φ         => φ

/-- The implication side obligation `Side(φ', φ)` (R-side-1).  Parameterised by the
    abstraction token `e` introduced into φ' and the ORIGINAL φ (from which the excised
    conjunct `B` is read): the obligation that the abstraction `p = absLeaf e` soundly
    stands for `B`, i.e. `p ⇒ B`.  Discharging it IN-CAGE (it is admissible —
    `side_admitted`) is what licenses a φ'-certificate to count for φ. -/
def Side (e : Thermite.Expr) : Frm → Frm
  | .conj _ B => .imp (absLeaf e) B
  | φ         => φ

/-! ## T4-R — conservativity (R-side-1): the certificate bridge

    For EVERY model `(q, dom)` and environment `ρ`, the rewritten formula φ' TOGETHER
    WITH a discharged `Side` re-establishes the original φ.  This is the soundness of
    using restratify: certifying φ' and discharging `Side` (both in-cage) certifies φ.
    The proof is a Boolean tautology — modus ponens on the abstraction's denotation. -/

/-- **T4-R conservativity.**  `restrat_conservative` consumes both φ' and `Side`:
    `fdenote φ' ∧ fdenote Side ⇒ fdenote φ`, for all `(q, dom, ρ)`.  Dropping either
    hypothesis breaks it — the `Side` hypothesis is precisely what `PinRestratDropSide`
    shows is load-bearing. -/
theorem restrat_conservative (e : Thermite.Expr) (q : Atom → Bool) (dom : List Tm)
    (A B : Frm) (ρ : Subst) :
    fdenote q dom (restrat e (.conj A B)) ρ = true →
    fdenote q dom (Side e (.conj A B)) ρ = true →
    fdenote q dom (.conj A B) ρ = true := by
  simp only [restrat, Side, absLeaf, fdenote, substAtom, Bool.and_eq_true, Bool.or_eq_true,
    Bool.not_eq_true']
  rintro ⟨hA, hp⟩ hside
  refine ⟨hA, ?_⟩
  -- hp : q (.qfree e) = true ; hside : q (.qfree e) = false ∨ fdenote q dom B ρ = true
  rcases hside with hpf | hB
  · exact absurd (hp.symm.trans hpf) (by decide)
  · exact hB

/-! ## T4-R — admissibility: φ' and `Side` are both in-cage

    The whole point of the split: the originally-rejected φ becomes TWO in-cage
    obligations.  Demonstrated on the §6 kv worked example. -/

/-- The canonical abstraction token for the kv repair (the value is irrelevant to the
    classifier — `qfree` is opaque; `boolLit true` is the Wire-format placeholder). -/
def kvAbs : Thermite.Expr := .boolLit true

/-- **T4-R, the rewritten formula is admitted.**  `restrat` takes the rejected kv cycle
    `ex_kvCycle` to an ADMITTED formula (the `Value → Key` edge is gone with B). -/
theorem restrat_admits : admitted (restrat kvAbs ex_kvCycle) = true := by decide

/-- For contrast: the original kv cycle is rejected (`Strat/Fragment.lean`
    `ex_kvCycle_rejected`), so `restrat` genuinely moves it into the cage. -/
theorem restrat_moves_into_cage :
    admitted ex_kvCycle = false ∧ admitted (restrat kvAbs ex_kvCycle) = true := by decide

/-- **T4-R `side_admitted`.**  The side obligation `Side(φ', φ)` is itself admitted —
    so it can be DISCHARGED IN-CAGE (it carries only the `Value → Key` edge from B, no
    cycle).  This is what makes the restratify split usable: both products are in-cage. -/
theorem side_admitted : admitted (Side kvAbs ex_kvCycle) = true := by decide

/-! ## T4-R — completeness: no validity is lost

    Restratify loses nothing.  Under the WITNESS oracle that assigns the fresh
    abstraction `p` the truth of the excised conjunct `B` (`q (.qfree e) = ⟦B⟧`), every
    model of φ is a model of φ' AND of `Side` — so any valid φ remains provable through
    the restratified pair.  (Conservativity holds for ALL oracles; completeness needs the
    canonical witness, exactly as Skolem-style repairs do.) -/

/-- **T4-R completeness.**  Under the witness oracle (`q (.qfree e) = fdenote q dom B ρ`),
    a model of φ is a model of both φ' and `Side`: the rewrite admits the original. -/
theorem restrat_complete (e : Thermite.Expr) (q : Atom → Bool) (dom : List Tm)
    (A B : Frm) (ρ : Subst)
    (hwit : q (.qfree e) = fdenote q dom B ρ) :
    fdenote q dom (.conj A B) ρ = true →
      fdenote q dom (restrat e (.conj A B)) ρ = true
        ∧ fdenote q dom (Side e (.conj A B)) ρ = true := by
  simp only [restrat, Side, absLeaf, fdenote, substAtom, Bool.and_eq_true, Bool.or_eq_true,
    Bool.not_eq_true', hwit]
  rintro ⟨hA, hB⟩
  exact ⟨⟨hA, hB⟩, Or.inr hB⟩

/-! ## Axiom hygiene (documented in-file, the REQ-2/REQ-6 convention)

    `restrat_conservative` is additionally added to the axiom-gated THEOREMS list in
    `scripts/lean-axiom-probe.sh` (the REQ-9 [1′] extension, brought forward for AC-7,
    as REQ-5 did for `strat_ref_sound`). -/
section AxiomHygiene
-- All four T4-R theorems are core-Lean-only (⊆ {propext, Classical.choice, Quot.sound}).
end AxiomHygiene

end Thermite.Strat.Cls
