/-
  Thermite/Strat/CombDeriv.lean — REQ-6, combinator demotion: the eight
  `comb_deriv_*` lemmas that close the v1 SpecTherm combinator set into the
  stratified fragment, proving each combinator's denotation EQUALS its
  raw-quantifier expansion (the six bounded ones) or its definitional aggregate
  form (the two SPIKE-2 census combinators that have no layer-1 quantifier
  spelling).

  Governing design: `.design/stage2-stratified-cage.md` REQ-6 / AC-6 (child of
  `.design/thermite2-program.md`; spec of record: the stage-2 metatheory sketch,
  GH issue #2; gate G2). The combinator registry
  (`thermite-spec/src/combinators.rs`) is the surface-syntax source of record and
  is UNTOUCHED — this module mechanizes the demotion SPIKE-2 hand-derived.

  THE TWO-SYNTAX TARGET (binding, per the REQ-3/4/5 notes). The raw-quantifier
  target is the CLASSIFIER surface `Thermite.Strat.Cls.Frm` (the sort-typed
  `Sort₂`/`Tm`/`Atom`/`Frm` of `Strat/Nnf.lean`), NOT REQ-1's minimal semantic
  spine `Thermite.Strat.Frm`. Each expansion is denoted by REQ-3's STRUCTURAL
  `fdenote` (the `(q : Atom → Bool, dom : List Tm)` denotation), which reads atoms
  through an uninterpreted oracle `q` and ranges binders over a finite domain
  `dom`. So each `comb_deriv_*` proves the demotion at the STRUCTURAL layer,
  parametric in `q` and `dom`.

  WHAT IS PROVEN — AND THE REQ-8 BOUNDARY. For the six bounded combinators the
  lemma is `fdenote q dom (expand) ρ = true ↔ <bounded ∀/∃ characterization over
  dom>` — the genuine "raw-quantifier expansion" content: the Cls.Frm skeleton
  denotes exactly the bounded quantifier `Thermite.denote` already gives the v1
  combinator (`Thermite/Denote.lean` lines 444–465, mirroring the frozen
  `verus_l3` of `combinators.rs`), with the index ranging over the finite carrier
  `dom` instead of `∀ i : Int` with explicit bounds. The atom oracle `q` stays
  ABSTRACT: grounding `q` to the real v1 atom semantics
  (`Read`/`Len`/`Cast`/`rel → Thermite.denote`) is REQ-8's INHERITED OBLIGATION
  (REQ-5 option B, #327 — the structural soundness layer proves only the
  quantifier/boolean skeleton, parametric in `q`). REQ-6 therefore proves the
  SKELETON correspondence — that each combinator HAS the claimed raw-quantifier
  spelling in the classifier surface and that its `fdenote` is exactly the
  intended bounded quantifier — and leaves the per-atom grounding to REQ-8. The
  spine `sdenote` is the eventual concrete `q`; the result holds for every `q`, so
  it specializes to that instance.

  THE SPIKE-2 CENSUS (do not fight it). Two of the eight — `count_where` (a
  recursive `nat` count) and `permutation_of` (a multiset equality) — have NO
  layer-1 raw-quantifier spelling. Their `comb_deriv_*` lemmas demote to the
  DEFINITIONAL aggregate forms: `count_where` to a filter-length count (mirroring
  `Thermite/Denote.lean`'s `countWhereVal`), `permutation_of` to the per-element
  count characterization of multiset equality (mirroring `permEq`). Their
  stratified TV routes through the semantic phase (REQ-8), not the syntactic
  normalizer — so there is no Cls.Frm quantifier expansion to prove sound here.

  PIN. `Thermite/PinCombDeriv.lean` exhibits an off-by-one expansion (`≤ len`
  where `< len` is meant) and shows by `decide` that it diverges from the
  faithful one on a concrete domain.

  Core-Lean-only: imports only `Strat/Nnf.lean` (the classifier surface +
  `fdenote`). No Mathlib. -/
import Thermite.Strat.Nnf

namespace Thermite.Strat.Cls

/-! ## The shared classifier-surface vocabulary of an expansion

    Each raw-quantifier expansion is built from the real sort-typed `Cls` term
    vocabulary (`Strat/Nnf.lean`): a closed sequence term for the slice, the
    `usize` index binder, `len`/`read`/`rel`, and a declared unary spec fn
    (`app1`) for the predicate application — exactly the array-property fragment
    the classifier admits. The slice is modelled as a CLOSED literal term so the
    closing substitution is the identity on it (the structural demotion abstracts
    the slice VALUE — that grounding is REQ-8); the only bound variable is the
    quantified index. -/

/-- The `bool` machine sort (a predicate's result + the literal it is tested against). -/
abbrev boolS : Sort₂ := .mach .bool

/-- A closed sequence-of-`elem` term standing for the primary slice `s`. Closed
    (`lit`), so `substTm ρ (seqA elem) = seqA elem`. -/
def seqA (elem : Sort₂) : Tm := .lit (.seq elem)

/-- A closed sequence-of-`elem` term standing for the second slice `b` (only
    `disjoint`/`permutation_of`). Distinct constructor from `seqA` so the two
    slices are syntactically different terms; also closed. -/
def seqB (elem : Sort₂) : Tm := .idxOp (.lit (.seq elem)) 1

/-- The bound `usize` index variable (de Bruijn 0). -/
def idx0 : Tm := .var usizeS 0
/-- The outer bound `usize` index variable (de Bruijn 1) — the `i` of a two-binder
    expansion (`sorted`, `disjoint`), where the inner `j` is `idx0`. -/
def idx1 : Tm := .var usizeS 1
/-- A closed `usize` literal standing for the scalar index argument `n`
    (`forall_below`/`forall_from`). -/
def nLit : Tm := .lit usizeS

/-- `0 ≤ t` — the lower index bound (`0 <= i` in every `verus_l3` form). -/
def boundLo (t : Tm) : Atom := .rel .le (.lit usizeS) t
/-- `t < (sq).len()` — the upper index bound against a slice term. -/
def boundHi (sq t : Tm) : Atom := .rel .lt t (.len sq)

/-- The predicate `p` applied to the `elem`-element `x`, modelled as the declared
    unary spec fn `f` over `x` tested for truth (`#[trigger] p(s[i])` in the frozen
    `verus_l3`). `app1` is the classifier's "declared unary spec fn" term
    (`Strat/Nnf.lean`); comparing it to the `bool` literal makes a relation atom
    the oracle reads. -/
def predApp (elem : Sort₂) (f : Nat) (x : Tm) : Atom :=
  .rel .eq (.app1 elem boolS f x) (.lit boolS)

/-- The element `s[i]` of the primary slice. -/
def readA (elem : Sort₂) (i : Tm) : Tm := .read elem (seqA elem) i
/-- The element `b[j]` of the second slice. -/
def readB (elem : Sort₂) (j : Tm) : Tm := .read elem (seqB elem) j

/-! ## The six raw-quantifier expansions (bounded combinators)

    Each is a genuine `Cls.Frm` over the classifier surface. The bound shapes
    mirror the frozen `verus_l3` of `thermite-spec/src/combinators.rs` exactly. -/

/-- `forall_in(s, p)` ⤳ `∀ i:usize. 0 ≤ i < s.len() → p(s[i])`. -/
def forallInExp (elem : Sort₂) (f : Nat) : Frm :=
  .all usizeS
    (.imp (.conj (.atom (boundLo idx0)) (.atom (boundHi (seqA elem) idx0)))
          (.atom (predApp elem f (readA elem idx0))))

/-- `exists_in(s, p)` ⤳ `∃ i:usize. 0 ≤ i < s.len() ∧ p(s[i])`. -/
def existsInExp (elem : Sort₂) (f : Nat) : Frm :=
  .ex usizeS
    (.conj (.conj (.atom (boundLo idx0)) (.atom (boundHi (seqA elem) idx0)))
           (.atom (predApp elem f (readA elem idx0))))

/-- `sorted(s)` ⤳ `∀ i j:usize. 0 ≤ i ≤ j < s.len() → s[i] ≤ s[j]`. -/
def sortedExp (elem : Sort₂) : Frm :=
  .all usizeS (.all usizeS
    (.imp (.conj (.atom (boundLo idx1))
            (.conj (.atom (.rel .le idx1 idx0))
                   (.atom (boundHi (seqA elem) idx0))))
          (.atom (.rel .le (readA elem idx1) (readA elem idx0)))))

/-- `forall_below(s, n, p)` ⤳ `∀ i:usize. 0 ≤ i ∧ i < n ∧ i < s.len() → p(s[i])`. -/
def forallBelowExp (elem : Sort₂) (f : Nat) : Frm :=
  .all usizeS
    (.imp (.conj (.atom (boundLo idx0))
            (.conj (.atom (.rel .lt idx0 nLit))
                   (.atom (boundHi (seqA elem) idx0))))
          (.atom (predApp elem f (readA elem idx0))))

/-- `forall_from(s, n, p)` ⤳ `∀ i:usize. n ≤ i ∧ i < s.len() → p(s[i])`. -/
def forallFromExp (elem : Sort₂) (f : Nat) : Frm :=
  .all usizeS
    (.imp (.conj (.atom (.rel .le nLit idx0)) (.atom (boundHi (seqA elem) idx0)))
          (.atom (predApp elem f (readA elem idx0))))

/-- `disjoint(a, b)` ⤳ `∀ i j:usize. (0≤i<a.len() ∧ 0≤j<b.len()) → a[i] ≠ b[j]`. -/
def disjointExp (elem : Sort₂) : Frm :=
  .all usizeS (.all usizeS
    (.imp (.conj (.conj (.atom (boundLo idx1)) (.atom (boundHi (seqA elem) idx1)))
                 (.conj (.atom (boundLo idx0)) (.atom (boundHi (seqB elem) idx0))))
          (.atom (.rel .ne (readA elem idx1) (readB elem idx0)))))

/-! ## The six demotion lemmas

    Each unfolds the structural `fdenote` of the expansion to the genuine bounded
    `∀`/`∃` over the finite carrier `dom`, with the bound element substituted into
    the bound/predicate atoms read by `q`. This is the raw-quantifier content of
    the demotion: the Cls.Frm skeleton denotes EXACTLY the bounded quantifier the
    v1 combinator does (`Thermite/Denote.lean` 444–465), the index ranging over
    `dom` rather than `∀ i:Int` with explicit bounds, parametric in the atom
    oracle `q` (grounding `q` is REQ-8). -/

theorem comb_deriv_forall_in (q : Atom → Bool) (dom : List Tm) (ρ : Subst)
    (elem : Sort₂) (f : Nat) :
    fdenote q dom (forallInExp elem f) ρ = true ↔
      ∀ v ∈ dom,
        (q (boundLo v) = true ∧ q (boundHi (seqA elem) v) = true) →
          q (predApp elem f (readA elem v)) = true := by
  simp only [forallInExp, fdenote, List.all_eq_true, substAtom, substTm, cons,
    boundLo, boundHi, predApp, readA, seqA, idx0, Bool.or_eq_true,
    Bool.not_eq_true']
  constructor
  · intro h v hv hb
    have := h v hv
    simp only [hb.1, hb.2] at this
    simpa using this
  · intro h v hv
    cases hb1 : q (.rel .le (.lit usizeS) v) <;>
      cases hb2 : q (.rel .lt v (.len (.lit (.seq elem)))) <;>
      simp_all

theorem comb_deriv_exists_in (q : Atom → Bool) (dom : List Tm) (ρ : Subst)
    (elem : Sort₂) (f : Nat) :
    fdenote q dom (existsInExp elem f) ρ = true ↔
      ∃ v ∈ dom,
        (q (boundLo v) = true ∧ q (boundHi (seqA elem) v) = true) ∧
          q (predApp elem f (readA elem v)) = true := by
  simp only [existsInExp, fdenote, List.any_eq_true, substAtom, substTm, cons,
    boundLo, boundHi, predApp, readA, seqA, idx0, Bool.and_eq_true]

theorem comb_deriv_sorted (q : Atom → Bool) (dom : List Tm) (ρ : Subst)
    (elem : Sort₂) :
    fdenote q dom (sortedExp elem) ρ = true ↔
      ∀ vi ∈ dom, ∀ vj ∈ dom,
        (q (boundLo vi) = true ∧ q (.rel .le vi vj) = true ∧
            q (boundHi (seqA elem) vj) = true) →
          q (.rel .le (readA elem vi) (readA elem vj)) = true := by
  simp only [sortedExp, fdenote, List.all_eq_true, substAtom, substTm, cons,
    boundLo, boundHi, readA, seqA, idx0, idx1, Bool.or_eq_true,
    Bool.not_eq_true']
  constructor
  · intro h vi hvi vj hvj hb
    have := h vi hvi vj hvj
    simp only [hb.1, hb.2.1, hb.2.2] at this
    simpa using this
  · intro h vi hvi vj hvj
    cases hb1 : q (.rel .le (.lit usizeS) vi) <;>
      cases hb2 : q (.rel .le vi vj) <;>
      cases hb3 : q (.rel .lt vj (.len (.lit (.seq elem)))) <;>
      simp_all

theorem comb_deriv_forall_below (q : Atom → Bool) (dom : List Tm) (ρ : Subst)
    (elem : Sort₂) (f : Nat) :
    fdenote q dom (forallBelowExp elem f) ρ = true ↔
      ∀ v ∈ dom,
        (q (boundLo v) = true ∧ q (.rel .lt v nLit) = true ∧
            q (boundHi (seqA elem) v) = true) →
          q (predApp elem f (readA elem v)) = true := by
  simp only [forallBelowExp, fdenote, List.all_eq_true, substAtom, substTm, cons,
    boundLo, boundHi, predApp, readA, seqA, idx0, nLit, Bool.or_eq_true,
    Bool.not_eq_true']
  constructor
  · intro h v hv hb
    have := h v hv
    simp only [hb.1, hb.2.1, hb.2.2] at this
    simpa using this
  · intro h v hv
    cases hb1 : q (.rel .le (.lit usizeS) v) <;>
      cases hb2 : q (.rel .lt v (.lit usizeS)) <;>
      cases hb3 : q (.rel .lt v (.len (.lit (.seq elem)))) <;>
      simp_all

theorem comb_deriv_forall_from (q : Atom → Bool) (dom : List Tm) (ρ : Subst)
    (elem : Sort₂) (f : Nat) :
    fdenote q dom (forallFromExp elem f) ρ = true ↔
      ∀ v ∈ dom,
        (q (.rel .le nLit v) = true ∧ q (boundHi (seqA elem) v) = true) →
          q (predApp elem f (readA elem v)) = true := by
  simp only [forallFromExp, fdenote, List.all_eq_true, substAtom, substTm, cons,
    boundHi, predApp, readA, seqA, idx0, nLit, Bool.or_eq_true,
    Bool.not_eq_true']
  constructor
  · intro h v hv hb
    have := h v hv
    simp only [hb.1, hb.2] at this
    simpa using this
  · intro h v hv
    cases hb1 : q (.rel .le (.lit usizeS) v) <;>
      cases hb2 : q (.rel .lt v (.len (.lit (.seq elem)))) <;>
      simp_all

theorem comb_deriv_disjoint (q : Atom → Bool) (dom : List Tm) (ρ : Subst)
    (elem : Sort₂) :
    fdenote q dom (disjointExp elem) ρ = true ↔
      ∀ vi ∈ dom, ∀ vj ∈ dom,
        ((q (boundLo vi) = true ∧ q (boundHi (seqA elem) vi) = true) ∧
            (q (boundLo vj) = true ∧ q (boundHi (seqB elem) vj) = true)) →
          q (.rel .ne (readA elem vi) (readB elem vj)) = true := by
  simp only [disjointExp, fdenote, List.all_eq_true, substAtom, substTm, cons,
    boundLo, boundHi, readA, readB, seqA, seqB, idx0, idx1, Bool.or_eq_true,
    Bool.not_eq_true']
  constructor
  · intro h vi hvi vj hvj hb
    have := h vi hvi vj hvj
    simp only [hb.1.1, hb.1.2, hb.2.1, hb.2.2] at this
    simpa using this
  · intro h vi hvi vj hvj
    cases hb1 : q (.rel .le (.lit usizeS) vi) <;>
      cases hb2 : q (.rel .lt vi (.len (.lit (.seq elem)))) <;>
      cases hb3 : q (.rel .le (.lit usizeS) vj) <;>
      cases hb4 : q (.rel .lt vj (.len (.idxOp (.lit (.seq elem)) 1))) <;>
      simp_all

/-! ## The two SPIKE-2 census combinators — definitional aggregate forms

    `count_where` and `permutation_of` have no layer-1 raw-quantifier spelling, so
    their demotion is to definitional aggregate forms over the finite carrier
    `dom` (the element multiset), mirroring `Thermite/Denote.lean`'s `countWhereVal`
    / `permEq`. Their stratified TV routes through the semantic phase (REQ-8). -/

/-- The structural count of `count_where(s, p)` over the carrier `dom`: the number
    of elements `x` whose predicate atom the oracle accepts. Mirrors
    `Thermite/Denote.lean`'s `countWhereVal` recursive `verus_l3` count, over the
    Cls surface (`q` reads `predApp`). Core Lean, no Mathlib, no fuel — the list
    shrinks structurally. -/
def countWhereCls (q : Atom → Bool) (elem : Sort₂) (f : Nat) : List Tm → Nat
  | []      => 0
  | x :: xs => (if q (predApp elem f x) then 1 else 0) + countWhereCls q elem f xs

/-- `comb_deriv_count_where` (definitional form). `count_where` demotes to the
    filter-length aggregate: its structural count equals the length of the
    sub-list of elements the predicate accepts. No quantifier expansion — the
    census form. -/
theorem comb_deriv_count_where (q : Atom → Bool) (elem : Sort₂) (f : Nat)
    (dom : List Tm) :
    countWhereCls q elem f dom
      = (dom.filter (fun x => q (predApp elem f x))).length := by
  induction dom with
  | nil => rfl
  | cons x xs ih =>
    simp only [countWhereCls, List.filter]
    cases hx : q (predApp elem f x) <;> simp [ih, Nat.add_comm]

/-- The recursive `verus_l3` head-then-tail step, made explicit (mirrors
    `Thermite/Denote.lean`'s `countWhereVal_cons`): the demotion is faithful to the
    frozen recursive count, not only to the filter-length closed form. -/
theorem countWhereCls_cons (q : Atom → Bool) (elem : Sort₂) (f : Nat)
    (x : Tm) (xs : List Tm) :
    countWhereCls q elem f (x :: xs)
      = (if q (predApp elem f x) then 1 else 0) + countWhereCls q elem f xs := rfl

/-- The decidable per-element count check of `permutation_of(a, b)`: every value
    occurring in either slice occurs the same number of times in both. Mirrors
    `Thermite/Denote.lean`'s `permEq` count-characterization of multiset equality
    (core `List.count`, not Mathlib's `Multiset`). -/
def permEqCls (a b : List Tm) : Bool :=
  (a ++ b).all (fun x => decide (a.count x = b.count x))

/-- `comb_deriv_permutation_of` (definitional form). `permutation_of` demotes to
    the per-element count characterization of multiset equality: the decidable
    check holds iff EVERY value occurs equally often in both slices — the exact
    `permEq` semantics (multiset, not set: `[1,1,2]`/`[1,2,2]` are refuted). No
    quantifier expansion — the census form. -/
theorem comb_deriv_permutation_of (a b : List Tm) :
    permEqCls a b = true ↔ ∀ x : Tm, a.count x = b.count x := by
  simp only [permEqCls, List.all_eq_true, List.mem_append, decide_eq_true_eq]
  constructor
  · intro h x
    by_cases hx : x ∈ a ∨ x ∈ b
    · exact h x hx
    · have hna : x ∉ a := fun ha => hx (Or.inl ha)
      have hnb : x ∉ b := fun hb => hx (Or.inr hb)
      rw [List.count_eq_zero.mpr hna, List.count_eq_zero.mpr hnb]
  · intro h x _; exact h x

/-! ## In-file axiom probe (AC-6)

    Each `comb_deriv_*` must show a subset of `{propext, Classical.choice,
    Quot.sound}` (Quot.sound via `funext` under the binder folds / `List` recursors),
    zero `sorry` — axiom-clean, core-Lean-only. These modules are also added to
    `scripts/lean-axiom-probe.sh`'s build targets (REQ-9 [1′] surface) so a `sorry`
    or broken proof fails the CI Lean job. -/
#print axioms comb_deriv_forall_in
#print axioms comb_deriv_exists_in
#print axioms comb_deriv_sorted
#print axioms comb_deriv_forall_below
#print axioms comb_deriv_forall_from
#print axioms comb_deriv_disjoint
#print axioms comb_deriv_count_where
#print axioms comb_deriv_permutation_of

end Thermite.Strat.Cls
