/-
  Thermite/Strat/Carrier.lean — the carrier assignment with a HAND-ROLLED
  finiteness witness, and the de Bruijn environment algebra.

  Governing design: `.design/stage2-stratified-cage.md` REQ-1 / AC-1. The
  metatheory sketch's `CarrierAssign` wrote Mathlib's `Fintype`, contradicting
  the sketch's own core-Lean-only-hot-path claim; SPIKE-1 resolved that tension
  (`.design/strat/substkit-conventions.md` §6 "the carrier verdict"): the opaque
  sort carries its finiteness as PLAIN DATA — an enumeration `List` + a
  completeness proof + a `DecidableEq` — and the denotation core stays
  Mathlib-free. This module is the production input that verdict specified.

  No Mathlib: this file imports nothing (the struct is parametric over a bare
  `C : Type`; `DecidableEq` is core Lean, carried as a field, NOT `Fintype`).
-/

namespace Thermite.Strat

/-! ## The carrier assignment

    A `CarrierAssign` bundles one opaque carrier sort `C` with the data that
    makes it a finite, decidable domain — without Mathlib's `Fintype`:

    * `deq`      — `DecidableEq C`, core Lean, carried as DATA (the SPIKE-1 §6
                   carry-as-data convention: because `C` is a *field* of a bundled
                   value, instance synthesis cannot recover `DecidableEq 𝓒.C` from
                   `𝓒` alone, so `Bool` equality routes through `deq` explicitly).
    * `enum`     — the finiteness enumeration.
    * `complete` — the completeness witness `∀ x, x ∈ enum`, the hand-rolled
                   replacement for `Fintype.complete`. This is the (R1)
                   load-bearing datum: it is what upgrades the `List.all`/`List.any`
                   binder folds to genuine `∀`/`∃` (`Strat/Denote.lean`
                   `sdenote_all_iff`/`sdenote_ex_iff`); `PinFiniteEscape` exhibits
                   the soundness escape that dropping it would permit.

    A whole stratified program assigns one `CarrierAssign` per opaque index/key
    sort; the sort-graph that collects them (and the (R1) finite-carrier
    admission check over them) is the classifier's concern (REQ-3). -/
structure CarrierAssign where
  /-- The opaque carrier sort. -/
  C : Type
  /-- Core-Lean `DecidableEq` on the sort, carried as data (SPIKE-1 §6). -/
  deq : DecidableEq C
  /-- The finiteness enumeration. -/
  enum : List C
  /-- The finiteness completeness witness — the hand-rolled replacement for
      `Fintype.complete`. -/
  complete : ∀ x : C, x ∈ enum

/-- Boolean equality on a carrier, routed through the carried `DecidableEq`
    (no instance synthesis over the value `𝓒`; SPIKE-1 §6). -/
def CarrierAssign.beq (𝓒 : CarrierAssign) (a b : 𝓒.C) : Bool := @decide (a = b) (𝓒.deq a b)

@[simp] theorem CarrierAssign.beq_self (𝓒 : CarrierAssign) (a : 𝓒.C) : 𝓒.beq a a = true := by
  simp [CarrierAssign.beq]

theorem CarrierAssign.beq_eq_true {𝓒 : CarrierAssign} {a b : 𝓒.C} :
    𝓒.beq a b = true ↔ a = b := by
  simp [CarrierAssign.beq]

/-! ## Environments: total valuations `Nat → C`

    The de Bruijn environment algebra (SPIKE-1 §2), parametric over a bare carrier
    sort `C`. Total functions, so index lookups are total and the binder lemmas
    (REQ-2) stay unconditional (no `getD` partiality side condition). -/

/-- A semantic environment / valuation over a carrier sort: total. -/
abbrev Env (C : Type) := Nat → C

/-- Push `v` as the new index-0 (binder introduction); shift everything else up
    by one. (SPIKE-1 §2.) -/
def cons {C : Type} (v : C) (ρ : Env C) : Env C :=
  fun i => match i with
    | 0 => v
    | i + 1 => ρ i

/-- Insert `v` at de Bruijn position `c`, shifting indices `≥ c` up by one.
    Structural recursion on the cutoff; `insert 0 = cons` definitionally. The
    SubstKit lemmas (REQ-2) are stated over this `insert`. (SPIKE-1 §2.) -/
def insert {C : Type} : Nat → C → Env C → Env C
  | 0,     v, ρ => cons v ρ
  | c + 1, v, ρ => cons (ρ 0) (insert c v (fun i => ρ (i + 1)))

end Thermite.Strat
