/-
  Thermite/PinStratFlip.lean — the quantifier-flip encoder pin (stage-2 pin
  battery, `.design/stage2-stratified-cage.md` REQ-5 / AC-5 / REQ-10).

  What it guards: `Strat/Soundness.lean`'s `strat_ref_sound` (T1-S) holds because
  `Strat/RefEncode.lean`'s `sencode` emits the SAME quantifier kind as the source
  (`all → Tok.all`, `ex → Tok.ex`). This pin exhibits the broken neighbour that
  SWAPS the kinds (`all → Tok.ex`, `ex → Tok.all`) and shows it FALSIFIES
  encoder soundness on a concrete 2-element domain, discharged by `decide`.

  The witness is the sentence `∃x. x = c0`, true over the domain `{c0, c1}` (take
  `x = c0`); flipped to `∀x. x = c0` it is false (`c1 ≠ c0`). So the flipped
  encoder's token denotes `false` where the source denotes `true`. The correct
  `sencode` agrees with the source on the same witness (via the proven
  `strat_ref_sound`), confirming the divergence is solely the swapped kind.

  Style model: the in-tree `lean/Thermite/Pin*.lean` battery (`PinFiniteEscape`,
  `PinBrokenLift`) — a small concrete carrier + `decide`-checked theorems. Like
  those, this file must keep compiling; `decide` (kernel), never `native_decide`.
-/
import Thermite.Strat.Soundness

namespace Thermite.PinStratFlip

open Thermite.Strat
open Thermite.Strat.Cls

/-! ## A concrete 2-element domain + an equality oracle -/

/-- Two distinct closed carrier terms (`c0 ≠ c1`: different constructors). -/
def c0 : Tm := .lit usizeS
def c1 : Tm := .idxOp (.lit usizeS) 1
/-- The finite quantifier domain. -/
def dom : List Tm := [c0, c1]

/-- The oracle interpreting `rel eq` as structural term equality (the only atom
    shape the witness uses); anything else is `false`. -/
def qEq : Atom → Bool
  | .rel .eq t u => decide (t = u)
  | _            => false

/-- An ambient substitution (unused — the witness is a closed sentence). -/
def σ0 : Subst := fun _ => c0

/-! ## The flipped encoder (the broken neighbour) -/

/-- The flip-broken encoder: identical to `sencodeAt` except it SWAPS the
    quantifier kinds. -/
def sencodeFlipAt (d : Nat) : Frm → Tok
  | .atom a   => .atom (encAtom d a)
  | .neg φ    => .neg (sencodeFlipAt d φ)
  | .conj φ ψ => .conj (sencodeFlipAt d φ) (sencodeFlipAt d ψ)
  | .disj φ ψ => .disj (sencodeFlipAt d φ) (sencodeFlipAt d ψ)
  | .imp φ ψ  => .imp (sencodeFlipAt d φ) (sencodeFlipAt d ψ)
  | .all s φ  => .ex s d true (sencodeFlipAt (d + 1) φ)   -- BUG: all → ex
  | .ex s φ   => .all s d true (sencodeFlipAt (d + 1) φ)  -- BUG: ex → all

def sencodeFlip (φ : Frm) : Tok := sencodeFlipAt 0 φ

/-- The witness sentence `∃x:usize. x = c0` — true over `dom` (`c0 ∈ dom`). -/
def phiFlip : Frm := .ex usizeS (.atom (.rel .eq (.var usizeS 0) c0))

/-! ## The pin -/

/-- The concrete counterexample: the flipped token denotes `false` while the source
    denotes `true`. -/
theorem flip_counterexample :
    tokDenote qEq dom (sencodeFlip phiFlip) σ0 = false
      ∧ fdenote qEq dom phiFlip σ0 = true := by decide

/-- The pin: encoder soundness is false for the flipped encoder — there is a
    formula on a concrete domain where its token disagrees with the source. -/
theorem flip_breaks_soundness :
    ¬ ∀ (φ : Frm) (σ ρ : Subst),
        tokDenote qEq dom (sencodeFlip φ) σ = fdenote qEq dom φ ρ := by
  intro h
  have hc := h phiFlip σ0 σ0
  rw [flip_counterexample.1, flip_counterexample.2] at hc
  exact absurd hc (by decide)

/-- For contrast: the CORRECT `sencode` agrees with the source on the same witness,
    via the proven `strat_ref_sound` (`phiFlip` is a well-scoped sentence). The
    divergence is solely the swapped quantifier kind. -/
theorem correct_sound_on_phiFlip (ρ σ : Subst) :
    tokDenote qEq dom (sencode phiFlip) σ = fdenote qEq dom phiFlip ρ :=
  strat_ref_sound qEq dom phiFlip 0 ρ σ (fun i hi => absurd hi (Nat.not_lt_zero i)) (by decide)

end Thermite.PinStratFlip
