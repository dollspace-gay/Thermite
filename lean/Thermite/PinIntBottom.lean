/-
  CRITIC PIN (#213) — proof-backends.md §4 "Soundness argument" divergence.

  Authority: lean/Thermite/Denote.lean (the shipped spine). `intVal` bottoms an
  Int-POSITION `specCall` to `0` (the `| none => 0` arm + the fuel-0 catch-all
  `| _, _, _ => 0`) — NOT to `True`. The doc claims bottoming "can only make the
  obligation EASIER (more vacuously true) by bottoming a `specCall` to `True`,
  never harder" and that an under-computed fuel₀ "only adds a TRIVIALLY-TRUE
  conjunct". Both claims are refuted here, kernel-checked:

  Registry: f(x) = g(x), g(x) = x — real-bodied and COMPLETE (the §4 export-time
  hard gate passes). ens = `result == f(x)` (the canonical contract shape — a
  specCall in comparison-operand / Int position, cf. `result as nat ==
  spec_sum(xs)`). env: x = 1, result = 1 — the CORRECT item (body = identity).

  - `ens_holds_at_fuel_2`: at adequate fuel the obligation carries the real
    content (`result = x`) and HOLDS.
  - `ens_FALSE_at_fuel_1`: at fuel 1 the inner call bottoms — `intVal` returns
    `0`, the conjunct is `result = 0`, FALSE for the correct item. The
    under-fuelled conjunct is NOT trivially true.
  - `obligation_form_is_false`: the §4 `∀ fuel ≥ fuel₀ → req → ens` form,
    instantiated with an under-computed fuel₀ = 1, is FALSE as a whole for this
    CORRECT item. (Corollary, value-dependent recursion: for
    `result == spec_f(xs)` with unfolding depth |xs| and the env ∀-quantified,
    every finite fuel admits a falsifying env — no fuel₀ computation fixes it.)

  Tracking: crosslink #213. This file is the audit artifact; it must keep
  compiling (it pins the spine's actual bottom values), and the §4 form must be
  revised before increment (ii) builds the exporter against it.
-/
import Thermite.Denote

namespace Thermite.Pin

/-- f(x) = g(x); g(x) = x. Real-bodied, complete — the §4 hard gate passes. -/
def Rpin : Registry := fun n =>
  if n = "f" then some ⟨["x"], Expr.specCall "g" [Expr.var "x"]⟩
  else if n = "g" then some ⟨["x"], Expr.var "x"⟩
  else none

/-- ens := `result == f(x)` — a specCall in `intVal` (Int) position. -/
def ensPin : Expr := Expr.cmp CmpOp.eq (Expr.var "result") (Expr.specCall "f" [Expr.var "x"])

/-- x = 1, result = 1 — the CORRECT item (body = identity, f(x) = x = 1). -/
def envPin : Env :=
  { ints := fun s => if s = "x" then 1 else if s = "result" then 1 else 0
    seqs := fun _ => []
    optres := fun _ => OptResVal.none_
    specs := Rpin }

/-- At adequate fuel the obligation carries the REAL content and HOLDS. -/
theorem ens_holds_at_fuel_2 : denote 2 ensPin envPin := by
  simp [ensPin, denote, intVal, intValArgs, envPin, Rpin, Env.bindParams, Env.bindInt]

/-- THE PIN: at fuel 1 the inner `g` call bottoms — `intVal` returns `0`, NOT
    `True` — so the conjunct is `result = 0`: FALSE at the correct env. -/
theorem ens_FALSE_at_fuel_1 : ¬ denote 1 ensPin envPin := by
  simp [ensPin, denote, intVal, intValArgs, envPin, Rpin, Env.bindParams, Env.bindInt]

/-- The §4 obligation form (under-computed fuel₀ = 1, req = true) is FALSE for
    this CORRECT item — not padded with trivially-true conjuncts. -/
theorem obligation_form_is_false :
    ¬ (∀ (fuel : Nat), fuel ≥ 1 →
        denote fuel (Expr.boolLit true) envPin → denote fuel ensPin envPin) := by
  intro h
  exact ens_FALSE_at_fuel_1 (h 1 (Nat.le_refl 1) (by simp [denote]))

end Thermite.Pin
