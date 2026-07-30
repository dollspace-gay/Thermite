/-
  Thermite/PinCombDeriv.lean — the off-by-one combinator-demotion pin (stage-2 pin
  battery, `.design/stage2-stratified-cage.md` REQ-6 / AC-6 / REQ-10).

  What it guards: `Strat/CombDeriv.lean`'s six bounded `comb_deriv_*` lemmas hold
  because each raw-quantifier expansion uses the FAITHFUL bound shape from the
  frozen `verus_l3` — in particular the STRICT upper bound `i < s.len()`. This pin
  exhibits the broken neighbour that uses `i ≤ s.len()` (the classic off-by-one:
  it lets the index `i = len` into range, reading one past the end) and shows it
  DIVERGES from the faithful expansion on a concrete 2-element domain, discharged
  by `decide`.

  The witness is `forall_in(s, p)` on a domain `{c0, c1}` where, under the oracle:
  `c0` is strictly in bounds (`c0 < len`) and satisfies the predicate; `c1` is the
  boundary index (`c1 = len`, so `¬(c1 < len)` but `c1 ≤ len`) and FAILS the
  predicate. The faithful expansion is TRUE (c0 in-bounds ⇒ pred holds; c1
  out-of-bounds ⇒ vacuous). The off-by-one expansion is FALSE (c1 now counts as
  in-bounds via `≤`, and its predicate fails). So the off-by-one demotion would
  certify `forall_in` true where the faithful one says false — a soundness break.

  Style model: the in-tree `lean/Thermite/Pin*.lean` battery (`PinStratFlip`,
  `PinFiniteEscape`) — a small concrete carrier + `decide`-checked theorems. Like
  those, this file must keep compiling; `decide` (kernel), never `native_decide`.
-/
import Thermite.Strat.CombDeriv

namespace Thermite.PinCombDeriv

open Thermite.Strat.Cls

/-! ## A concrete 2-element index domain + the off-by-one neighbour -/

/-- The element sort the witness quantifies a slice over. -/
def elem : Sort₂ := .opaque 0
/-- The declared unary predicate spec-fn id. -/
def f : Nat := 0

/-- The strictly-in-bounds index (`c0 < len`, predicate holds). Note `c0` is the
    `usize` literal, so it also shares the `boundLo` first-argument shape. -/
def c0 : Tm := .lit usizeS (.int 0)
/-- The boundary index (`c1 = len`: `¬(c1 < len)` but `c1 ≤ len`, predicate fails). -/
def c1 : Tm := .idxOp (.lit usizeS (.int 0)) 1
/-- The finite quantifier domain. -/
def dom : List Tm := [c0, c1]
/-- An ambient substitution (unused — the expansions are closed under their binder). -/
def σ0 : Subst := fun _ => c0

/-- The oracle realizing the witness: `0 ≤ i` always; `i ≤ len` always (the WRONG
    upper bound, true even at the boundary `c1`); `i < len` only for `c0` (the
    FAITHFUL upper bound, false at the boundary); `p(s[i])` only for `c0`. The
    `boundLo` shape (`le` with a `usize` literal on the left) is matched first, so
    the `i ≤ len` arm only catches the genuine off-by-one upper bound. -/
def qPin : Atom → Bool
  | .rel .le (.lit (.mach .usize) _) _            => true                  -- 0 ≤ i (boundLo)
  | .rel .le _ (.len _)                           => true                  -- i ≤ len (WRONG)
  | .rel .lt i (.len _)                           => decide (i = c0)       -- i < len (faithful)
  | .rel .eq (.app1 _ _ _ (.read _ _ i)) (.lit _ _) => decide (i = c0)     -- p(s[i])
  | _                                             => false

/-! ## The off-by-one expansion (the broken neighbour)

    Identical to `Strat/CombDeriv.lean`'s `forallInExp` except the upper bound is
    `i ≤ s.len()` (`.rel .le`) instead of the faithful `i < s.len()` (`.rel .lt`). -/

/-- `0 ≤ i` (the faithful lower bound; reused verbatim from `CombDeriv`). -/
def boundLoP (t : Tm) : Atom := .rel .le (.lit usizeS (.int 0)) t
/-- `i ≤ len` — the OFF-BY-one upper bound (`.rel .le` where `.rel .lt` is meant). -/
def boundHiWrong (sq t : Tm) : Atom := .rel .le t (.len sq)

/-- The off-by-one `forall_in` expansion. -/
def forallInExpWrong (elem : Sort₂) (f : Nat) : Frm :=
  .all usizeS
    (.imp (.conj (.atom (boundLoP idx0)) (.atom (boundHiWrong (seqA elem) idx0)))
          (.atom (predApp elem f (readA elem idx0))))

/-! ## The pin -/

/-- The concrete counterexample: the faithful expansion denotes `true` while the
    off-by-one neighbour denotes `false`, on the same domain and oracle. -/
theorem offbyone_counterexample :
    fdenote qPin dom (forallInExp elem f) σ0 = true
      ∧ fdenote qPin dom (forallInExpWrong elem f) σ0 = false := by decide

/-- The pin: the off-by-one upper bound BREAKS the demotion — there is a concrete
    model where the off-by-one expansion disagrees with the faithful one, so it is
    NOT a sound raw-quantifier spelling of `forall_in`. -/
theorem offbyone_breaks_demotion :
    fdenote qPin dom (forallInExpWrong elem f) σ0
      ≠ fdenote qPin dom (forallInExp elem f) σ0 := by decide

/-- For contrast: the faithful expansion's truth matches the bounded `∀`
    characterization `comb_deriv_forall_in` derives — the divergence is solely the
    off-by-one upper bound, not any other part of the expansion. -/
theorem faithful_matches_bounded_forall :
    fdenote qPin dom (forallInExp elem f) σ0 = true ↔
      ∀ v ∈ dom,
        (qPin (boundLo v) = true ∧ qPin (boundHi (seqA elem) v) = true) →
          qPin (predApp elem f (readA elem v)) = true :=
  comb_deriv_forall_in qPin dom σ0 elem f

end Thermite.PinCombDeriv
