/-
  Thermite/BvModel.lean — the bit-vector ⟷ bounded-integer model faithfulness
  metatheorem (`.design/stage3-bv-reconstruction.md` REQ-7/REQ-8; issue #356).

  ─────────────────────────────────────────────────────────────────────────────
  What this proves.
  ─────────────────────────────────────────────────────────────────────────────
  This module relates `BitVec N` to the bounded-integer machine model used by an
  earlier exporter path: each variable is an `Int` in `[0, 2^N)`, wrapping arithmetic
  reduces modulo `2^N`, and unsigned comparisons use the integer order.

  The main result is a kernel-checked proof that the two denotations agree. The active
  exporter now emits literal `BitVec N`, so this is a supplementary semantic cross-check
  rather than a bridge in the reconstruction path.

  Mathlib-free / Smt-free: rests only on Lean-core `BitVec` lemmas, so it builds in the
  core spine and is covered by `scripts/lean-axiom-probe.sh` (CI, no cvc5 needed) — a
  small independent check beside the `Smt`-importing `Thermite.SmtExport`.

  The fragment covers wrapping `+`/`-`/`*`, unsigned comparisons, and boolean
  connectives.
-/

namespace Thermite.BvModel

/-- The arithmetic term fragment: variables, literals, and wrapping `+`/`-`/`*`. -/
inductive Tm where
  | var (i : Nat)
  | lit (n : Nat)
  | add (a b : Tm)
  | sub (a b : Tm)
  | mul (a b : Tm)
  deriving Repr

/-- Comparisons and boolean connectives over the arithmetic term fragment. -/
inductive Frm where
  | tt
  | ff
  | eq (a b : Tm)
  | ne (a b : Tm)
  | lt (a b : Tm)
  | le (a b : Tm)
  | gt (a b : Tm)
  | ge (a b : Tm)
  | and (a b : Frm)
  | or (a b : Frm)
  | not (a : Frm)
  deriving Repr

variable {w : Nat}

/-! ## The fixed-width bit-vector semantics (what a `@bvN` clause means, REQ-2).

  Every operator is its `BitVec w` (2's-complement / unsigned) machine counterpart; the
  comparisons are the unsigned bit-vector relations (`BitVec`'s `<`/`≤` are `ult`/`ule`). -/

def tmBV (ρ : Nat → BitVec w) : Tm → BitVec w
  | .var i => ρ i
  | .lit n => BitVec.ofNat w n
  | .add a b => tmBV ρ a + tmBV ρ b
  | .sub a b => tmBV ρ a - tmBV ρ b
  | .mul a b => tmBV ρ a * tmBV ρ b

def frmBV (ρ : Nat → BitVec w) : Frm → Prop
  | .tt => True
  | .ff => False
  | .eq a b => tmBV ρ a = tmBV ρ b
  | .ne a b => tmBV ρ a ≠ tmBV ρ b
  | .lt a b => tmBV ρ a < tmBV ρ b
  | .le a b => tmBV ρ a ≤ tmBV ρ b
  | .gt a b => tmBV ρ a > tmBV ρ b
  | .ge a b => tmBV ρ a ≥ tmBV ρ b
  | .and a b => frmBV ρ a ∧ frmBV ρ b
  | .or a b => frmBV ρ a ∨ frmBV ρ b
  | .not a => ¬ frmBV ρ a

/-! ## The bounded-integer machine model.

  Each variable is an `Int` (range `[0, 2^w)` in the emitted goal's hypotheses); each
  wrapping operation reduces `% 2^w` (Lean `Int.emod` lands in `[0, 2^w)`); comparisons
  are integer comparisons. -/

/-- `2^w` as the positive `Int` modulus the emitted goal uses. -/
def M (w : Nat) : Int := (2 ^ w : Nat)

/-- The modulus `w` is explicit here: unlike [`tmBV`] (whose `w` is fixed by the
    `BitVec w` valuation), the integer model carries no `w`-typed argument, so the width
    is threaded directly. -/
def tmInt (w : Nat) (σ : Nat → Int) : Tm → Int
  | .var i => σ i
  | .lit n => (n : Int) % M w
  | .add a b => (tmInt w σ a + tmInt w σ b) % M w
  | .sub a b => (tmInt w σ a - tmInt w σ b) % M w
  | .mul a b => (tmInt w σ a * tmInt w σ b) % M w

def frmInt (w : Nat) (σ : Nat → Int) : Frm → Prop
  | .tt => True
  | .ff => False
  | .eq a b => tmInt w σ a = tmInt w σ b
  | .ne a b => tmInt w σ a ≠ tmInt w σ b
  | .lt a b => tmInt w σ a < tmInt w σ b
  | .le a b => tmInt w σ a ≤ tmInt w σ b
  | .gt a b => tmInt w σ a > tmInt w σ b
  | .ge a b => tmInt w σ a ≥ tmInt w σ b
  | .and a b => frmInt w σ a ∧ frmInt w σ b
  | .or a b => frmInt w σ a ∨ frmInt w σ b
  | .not a => ¬ frmInt w σ a

/-! ## Faithfulness — the two denotations agree under the `toNat` valuation. -/

/-- The valuation the faithfulness theorem instantiates: each `Int` variable is the
    `toNat` of the corresponding bit-vector (automatically in `[0, 2^w)`). -/
@[reducible] def toNatσ (ρ : Nat → BitVec w) : Nat → Int := fun i => ((ρ i).toNat : Int)

/-- `M w` is positive (the modulus is a power of two). -/
theorem M_pos : 0 < M w := by
  unfold M
  exact_mod_cast Nat.two_pow_pos w

/-- The modular identity behind the subtraction case: over `Int`, `m - q + p` and
    `p - q` differ by exactly the modulus `m`, so they agree mod `m`. (`omega` does not
    reason about a symbolic modulus, so this is discharged via `Int.add_emod_right`.) -/
theorem emod_sub_bridge (p q m : Int) : (m - q + p) % m = (p - q) % m := by
  rw [show m - q + p = (p - q) + m by omega]
  exact Int.add_emod_right (p - q) m

/-- TERM faithfulness: the bounded-integer model of a term, evaluated at the `toNat`
    valuation, equals the `toNat` of its bit-vector denotation. The crux; each wrapping
    operation matches the corresponding `BitVec.toNat_*` modular identity. -/
theorem tmInt_eq_toNat (ρ : Nat → BitVec w) (t : Tm) :
    tmInt w (toNatσ ρ) t = ((tmBV ρ t).toNat : Int) := by
  induction t with
  | var i => rfl
  | lit n =>
    -- ↑n % 2^w  =  ↑((BitVec.ofNat w n).toNat)  =  ↑(n % 2^w)  (natCast_emod, rfl)
    simp only [tmInt, tmBV, BitVec.toNat_ofNat, M, Int.natCast_emod]
  | add a b iha ihb =>
    -- (↑p + ↑q) % 2^w  =  ↑((p + q) % 2^w)   via BitVec.toNat_add + the cast-distributes
    simp only [tmInt, tmBV, BitVec.toNat_add, iha, ihb, M, Int.natCast_emod, Int.natCast_add]
  | mul a b iha ihb =>
    simp only [tmInt, tmBV, BitVec.toNat_mul, iha, ihb, M, Int.natCast_emod, Int.natCast_mul]
  | sub a b iha ihb =>
    -- BitVec.toNat_sub: (x - y).toNat = ((2^w - q) + p) % 2^w. The integer model's
    -- (↑p - ↑q) differs from ((2^w - ↑q) + ↑p) by exactly the modulus 2^w, so the two
    -- agree mod 2^w. `q ≤ 2^w` lets the Nat subtraction cast; positivity feeds omega's
    -- modular reasoning.
    have hb : (tmBV ρ b).toNat ≤ 2 ^ w := Nat.le_of_lt (tmBV ρ b).isLt
    simp only [tmInt, tmBV, BitVec.toNat_sub, iha, ihb, M, Int.natCast_emod, Int.natCast_add,
      Int.ofNat_sub hb]
    -- goal: (↑p - ↑q) % ↑(2^w) = (↑(2^w) - ↑q + ↑p) % ↑(2^w)
    exact (emod_sub_bridge _ _ _).symm

/-- Proposition faithfulness: at the `toNat` valuation, the bounded-integer model
    agrees with the bit-vector denotation. -/
theorem frmInt_iff_frmBV (ρ : Nat → BitVec w) (f : Frm) :
    frmInt w (toNatσ ρ) f ↔ frmBV ρ f := by
  induction f with
  | tt => simp [frmInt, frmBV]
  | ff => simp [frmInt, frmBV]
  | eq a b =>
    simp only [frmInt, frmBV, tmInt_eq_toNat, BitVec.toNat_eq]
    omega
  | ne a b =>
    simp only [frmInt, frmBV, tmInt_eq_toNat, BitVec.toNat_eq, ne_eq]
    omega
  | lt a b =>
    simp only [frmInt, frmBV, tmInt_eq_toNat, BitVec.lt_def]
    omega
  | le a b =>
    simp only [frmInt, frmBV, tmInt_eq_toNat, BitVec.le_def]
    omega
  | gt a b =>
    simp only [frmInt, frmBV, tmInt_eq_toNat, BitVec.lt_def, GT.gt]
    omega
  | ge a b =>
    simp only [frmInt, frmBV, tmInt_eq_toNat, BitVec.le_def, GE.ge]
    omega
  | and a b iha ihb => simp only [frmInt, frmBV]; rw [iha, ihb]
  | or a b iha ihb => simp only [frmInt, frmBV]; rw [iha, ihb]
  | not a ih => simp only [frmInt, frmBV]; rw [ih]

/-- A bounded-integer production/reference equivalence transfers to `BitVec`. -/
theorem tv_equiv_faithful (ρ : Nat → BitVec w) (prod ref : Frm)
    (h : frmInt w (toNatσ ρ) prod ↔ frmInt w (toNatσ ρ) ref) :
    frmBV ρ prod ↔ frmBV ρ ref := by
  rw [← frmInt_iff_frmBV ρ prod, ← frmInt_iff_frmBV ρ ref]
  exact h

/-! ## Trust accounting — in-file `#print axioms` (the SubstKit/SPIKE-1 convention).

  The faithfulness theorems must rest only on the standard axiom set
  `{propext, Classical.choice, Quot.sound}` — no `sorryAx`, no custom axiom. This is
  an independent check of the bounded-integer reference model. Probed in-file and built
  by `scripts/lean-axiom-probe.sh`; not added to the fixed universal-pillar theorem list. -/
#print axioms tmInt_eq_toNat
#print axioms frmInt_iff_frmBV
#print axioms tv_equiv_faithful

end Thermite.BvModel
