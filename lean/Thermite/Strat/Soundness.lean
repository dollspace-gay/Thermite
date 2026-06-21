/-
  Thermite/Strat/Soundness.lean — T1-S, the stratified encoder soundness: the
  trigger-free MBQI token surface `sencode φ` denotes exactly the source classifier
  formula `φ` (`strat_ref_sound`), and `sencode`'s output obeys the fresh-name +
  MBQI discipline (`strat_ref_wf`).

  Governing design: `.design/stage2-stratified-cage.md` REQ-5 / AC-5 (child of
  `.design/thermite2-program.md`; spec of record: the stage-2 metatheory sketch,
  GH issue #2; gate G2). See `Strat/TokDenote.lean` for the two-syntax bridge
  decision (option B: encode `Cls.Frm` directly against the structural `fdenote`).

  THE SOUNDNESS THEOREM (T1-S). `strat_ref_sound`:

      tokDenote q dom (sencodeAt d φ) σ = fdenote q dom φ ρ

  for EVERY oracle `q` and domain `dom`, whenever the named environment `σ` AGREES
  with the de Bruijn environment `ρ` at depth `d` (`Agree d ρ σ`) and `φ` is
  well-scoped under `d` (`wfFrm d φ`). The closed-sentence form (`strat_ref_sound`
  at `d = 0`, `Agree 0` vacuous) is the usable statement: an admitted clause
  encodes to a token that denotes the same in every model, for any environments —
  exactly encoder soundness for an SMT surface.

  THE FRESH-NAME ↔ de BRUIJN CORRESPONDENCE. The single load-bearing fact is the
  agreement invariant `Agree d ρ σ := ∀ i < d, σ (encName d i) = ρ i`: under the
  level-naming discipline, the named environment reproduces the de Bruijn one.
  `Agree_upd` shows it is MAINTAINED across a binder (`cons` on the de Bruijn side,
  `upd` at the fresh name `d` on the named side); `encTm_subst` lifts it to terms
  (a relabelled term, read under `σ`, equals the original read under `ρ`); the main
  induction lifts it to formulas. The well-scopedness hypothesis is what makes the
  variable case go through (no underflowed level, no name collision).

  Axiom discipline (AC-5): the `#print axioms` lines at the end probe `strat_ref_sound`
  and `strat_ref_wf` in-file; each must show a subset of `{propext,
  Classical.choice, Quot.sound}` (Quot.sound via `funext` under the binder folds),
  zero `sorry`. `strat_ref_sound` is also added to `scripts/lean-axiom-probe.sh`'s
  axiom-gated THEOREMS so CI enforces the same (the REQ-9 [1′] extension, brought
  forward for AC-5).

  Core-Lean-only: imports only `Strat/RefEncode.lean`. No Mathlib.
-/
import Thermite.Strat.RefEncode

namespace Thermite.Strat

open Thermite.Strat.Cls

/-! ## The agreement invariant -/

/-- The named environment `σ` agrees with the de Bruijn environment `ρ` at depth
    `d`: every in-scope index `i < d` reads, through its level name `encName d i`,
    the same value. -/
def Agree (d : Nat) (ρ σ : Subst) : Prop := ∀ i, i < d → σ (encName d i) = ρ i

/-- Agreement is maintained across a binder: pushing `v` (de Bruijn `cons`) lines
    up with binding the fresh name `d` (named `upd σ d v`). -/
theorem Agree_upd (d : Nat) (ρ σ : Subst) (v : Tm) (h : Agree d ρ σ) :
    Agree (d + 1) (cons v ρ) (upd σ d v) := by
  intro i hi
  cases i with
  | zero =>
    -- encName (d+1) 0 = d; upd σ d v d = v; cons v ρ 0 = v
    have hname : encName (d + 1) 0 = d := by simp [encName]
    rw [hname]
    simp [upd, cons]
  | succ k =>
    have hk : k < d := Nat.lt_of_succ_lt_succ hi
    -- encName (d+1) (k+1) = d - 1 - k = encName d k, and d-1-k ≠ d
    have hname : encName (d + 1) (k + 1) = encName d k := by simp [encName]; omega
    have hne : encName d k ≠ d := by simp [encName]; omega
    rw [hname]
    have hlk : upd σ d v (encName d k) = σ (encName d k) := by simp [upd, hne]
    rw [hlk, h k hk]
    -- cons v ρ (k+1) = ρ k
    rfl

/-! ## Term-level soundness -/

/-- A relabelled term, read under the named env `σ`, equals the original read under
    the de Bruijn env `ρ` — given agreement and well-scopedness. -/
theorem encTm_subst (d : Nat) (ρ σ : Subst) (h : Agree d ρ σ) :
    ∀ (t : Tm), wfTm d t = true → substTm σ (encTm d t) = substTm ρ t := by
  intro t
  induction t with
  | var s i =>
    intro hw
    have hi : i < d := by simp only [wfTm, decide_eq_true_eq] at hw; exact hw
    simp only [encTm, substTm]
    exact h i hi
  | lit s => intro _; rfl
  | read e sq ix ihsq ihix =>
    intro hw
    simp only [wfTm, Bool.and_eq_true] at hw
    simp only [encTm, substTm, ihsq hw.1, ihix hw.2]
  | len sq ih => intro hw; simp only [encTm, substTm, ih hw]
  | cast to t ih => intro hw; simp only [encTm, substTm, ih hw]
  | idxOp t k ih => intro hw; simp only [encTm, substTm, ih hw]
  | mul t u iht ihu =>
    intro hw
    simp only [wfTm, Bool.and_eq_true] at hw
    simp only [encTm, substTm, iht hw.1, ihu hw.2]
  | app1 a r f t ih => intro hw; simp only [encTm, substTm, ih hw]

/-- Atom-level companion of `encTm_subst`. -/
theorem encAtom_subst (d : Nat) (ρ σ : Subst) (h : Agree d ρ σ) (a : Atom)
    (hw : wfAtom d a = true) : substAtom σ (encAtom d a) = substAtom ρ a := by
  cases a with
  | rel r t u =>
    simp only [wfAtom, Bool.and_eq_true] at hw
    simp only [encAtom, substAtom, encTm_subst d ρ σ h t hw.1, encTm_subst d ρ σ h u hw.2]
  | qfree e => rfl

/-! ## T1-S — the encoder is sound

    The formula induction. `q` and `dom` are fixed context; the formula is the
    induction target with `d`, `ρ`, `σ` quantified after it, so each binder case
    can re-instantiate `d → d+1`, `ρ → cons v ρ`, `σ → upd σ d v`. The binder cases
    line up the two folds with `congrArg`/`funext` and discharge each point by the
    IH under `Agree_upd`. -/
theorem strat_ref_sound (q : Atom → Bool) (dom : List Tm) (φ : Frm) :
    ∀ (d : Nat) (ρ σ : Subst), Agree d ρ σ → wfFrm d φ = true →
      tokDenote q dom (sencodeAt d φ) σ = fdenote q dom φ ρ := by
  induction φ with
  | atom a =>
    intro d ρ σ hag hw
    simp only [sencodeAt, tokDenote, fdenote, encAtom_subst d ρ σ hag a hw]
  | neg φ ih =>
    intro d ρ σ hag hw
    simp only [sencodeAt, tokDenote, fdenote, ih d ρ σ hag hw]
  | conj φ ψ ihφ ihψ =>
    intro d ρ σ hag hw
    simp only [wfFrm, Bool.and_eq_true] at hw
    simp only [sencodeAt, tokDenote, fdenote, ihφ d ρ σ hag hw.1, ihψ d ρ σ hag hw.2]
  | disj φ ψ ihφ ihψ =>
    intro d ρ σ hag hw
    simp only [wfFrm, Bool.and_eq_true] at hw
    simp only [sencodeAt, tokDenote, fdenote, ihφ d ρ σ hag hw.1, ihψ d ρ σ hag hw.2]
  | imp φ ψ ihφ ihψ =>
    intro d ρ σ hag hw
    simp only [wfFrm, Bool.and_eq_true] at hw
    simp only [sencodeAt, tokDenote, fdenote, ihφ d ρ σ hag hw.1, ihψ d ρ σ hag hw.2]
  | all s φ ih =>
    intro d ρ σ hag hw
    simp only [sencodeAt, tokDenote, fdenote]
    apply congrArg (List.all dom)
    funext v
    exact ih (d + 1) (cons v ρ) (upd σ d v) (Agree_upd d ρ σ v hag) hw
  | ex s φ ih =>
    intro d ρ σ hag hw
    simp only [sencodeAt, tokDenote, fdenote]
    apply congrArg (List.any dom)
    funext v
    exact ih (d + 1) (cons v ρ) (upd σ d v) (Agree_upd d ρ σ v hag) hw

/-- The closed-sentence corollary: a well-scoped SENTENCE (`wfFrm 0`) encodes to a
    token whose denotation equals the source's, for ANY environments and in EVERY
    model `(q, dom)` — the usable form of T1-S. -/
theorem strat_ref_sound_sentence (q : Atom → Bool) (dom : List Tm) (φ : Frm)
    (hw : wfFrm 0 φ = true) (ρ σ : Subst) :
    tokDenote q dom (sencode φ) σ = fdenote q dom φ ρ :=
  strat_ref_sound q dom φ 0 ρ σ (fun i hi => absurd hi (Nat.not_lt_zero i)) hw

/-! ## strat_ref_wf — the encoder output obeys the fresh-name + MBQI discipline -/

/-- `sencodeAt d φ` is `tokWf d`: every binder is named by its level (`≥ d`, body
    `tokWf (name+1)`, so names strictly increase down each path — no capture) and
    trigger-free. -/
theorem tokWf_sencodeAt (φ : Frm) : ∀ (d : Nat), tokWf d (sencodeAt d φ) = true := by
  induction φ with
  | atom a => intro d; rfl
  | neg φ ih => intro d; simp only [sencodeAt, tokWf, ih d]
  | conj φ ψ ihφ ihψ => intro d; simp only [sencodeAt, tokWf, ihφ d, ihψ d, Bool.and_self]
  | disj φ ψ ihφ ihψ => intro d; simp only [sencodeAt, tokWf, ihφ d, ihψ d, Bool.and_self]
  | imp φ ψ ihφ ihψ => intro d; simp only [sencodeAt, tokWf, ihφ d, ihψ d, Bool.and_self]
  | all s φ ih =>
    intro d
    have hle : decide (d ≤ d) = true := by simp
    simp only [sencodeAt, tokWf, hle, ih (d + 1), Bool.and_true]
  | ex s φ ih =>
    intro d
    have hle : decide (d ≤ d) = true := by simp
    simp only [sencodeAt, tokWf, hle, ih (d + 1), Bool.and_true]

/-- `strat_ref_wf` (AC-5): the encoder's output obeys the fresh-name + trigger-free
    (MBQI) discipline — binder names strictly increase down every path (so are
    pairwise distinct: no capture) and every quantifier is trigger-free. -/
theorem strat_ref_wf (φ : Frm) : tokWf 0 (sencode φ) = true :=
  tokWf_sencodeAt φ 0

/-! ## In-file axiom probe (AC-5)

    Each must show a subset of `{propext, Classical.choice, Quot.sound}` — zero
    `sorry`. `strat_ref_sound` is additionally gated in CI via the THEOREMS list of
    `scripts/lean-axiom-probe.sh`. -/
#print axioms strat_ref_sound
#print axioms strat_ref_wf

end Thermite.Strat
