/-
  PinRegistryTerminating.lean — CRITIC PIN (Pin E), RE-PINNED to the RESOLVED
  truth (crosslink #241, ref #240 #203 #215; the #186 precedent — when a finding
  is resolved AT THE ROOT, the pin updates to the resolved-truth oracle and stays
  the regression oracle BOTH directions).

  THE ORIGINAL FINDING (cycle-4, commit f7d288ef). `lean/Thermite/Stabilize.lean`'s
  `RegistryTerminating env e := ∃ v, stabilizes e env v` was an IDENTITY HYPOTHESIS,
  and `stabilization_exists` was the definitional identity over it — ZERO proof
  content. The bug: the BOTTOMING `intVal` cannot distinguish "stabilized to a
  genuine value" from "stuck at the Int-bottom 0 because it diverged", so the
  DIVERGENT registry `f(x) = f(x)` (whose `intVal` is constantly the bottom 0)
  CLEARED the hypothesis — the very registry §1.2's REGISTRY-TERMINATION class
  exists to REJECT satisfied it, and a wrong contract certified at the bottom.

  THE ROOT FIX (#241, in `Stabilize.lean`). A SECOND, BOTTOM-DISTINGUISHING
  none-propagating denotation `intValNB`/`denoteNB` mirrors the spine recursion
  EXACTLY save: a fuel-0 `specCall` → `none`; an unresolved `specCall` → `none`;
  every arm PROPAGATES `none`. `Converges e env v := ∃ N, ∀ fuel ≥ N, intValNB
  fuel e env = some v` reaches a GENUINE value, which the divergent call NEVER does
  (it bottoms to `none` at EVERY fuel). The AGREEMENT LEMMA `converges_imp_
  stabilizes` (`intValNB_agrees`) carries that genuine content into `stabilizes`,
  and `RegistryTerminating` is REDEFINED `∃ v, Converges e env v` — no longer an
  identity hypothesis.

  THIS PIN, RE-PINNED (the resolved-truth oracle, both directions):
  - E.1 the DIVERGENT call is `none` at every fuel (`divergent_call_NB_is_none`).
  - E.2 the DIVERGENT registry now FAILS the hypothesis
    (`divergent_registry_fails_the_hypothesis`, `¬ RegistryTerminating envD fCall`)
    — the registry §1.2 mandates the class reject NO LONGER clears it, closing the
    cycle-4 divergence. This is the load-bearing reversal.
  - E.3 `stabilization_exists` has NON-IDENTITY content: it is the agreement-lemma
    composite, not `id` (`stabilization_exists_is_not_identity` — there is no
    `RegistryTerminating` term to feed it on the divergent registry).
  - E.4 a GENUINE (convergent) registry — `g(x) = 5` — still CONVERGES and
    STABILIZES to 5 (`genuine_registry_converges` / `genuine_registry_stabilizes`),
    so the fix did not over-reject: a real dec-valid item still discharges.

  Builds GREEN; the green build IS the demonstration of the resolution. Tracking:
  crosslink #241.
-/
import Thermite.Stabilize

namespace Thermite.PinRegTerm

/-! ## E.0 — the divergent registry (Pin B's, verbatim) -/

def Rdiv : Registry := fun n =>
  if n = "f" then some ⟨["x"], Expr.specCall "f" [Expr.var "x"]⟩ else none

def envD : Env :=
  { ints := fun _ => 0
    seqs := fun _ => []
    optres := fun _ => OptResVal.none_
    specs := Rdiv }

def fCall : Expr := Expr.specCall "f" [Expr.var "x"]

/-! ## E.1 — the DIVERGENT call is `none` at EVERY fuel (the bottom-distinguishing
    denotation REFUSES to assign it a value: it never converges). -/

/-- The denoted args of `f(x)` are `some [x-value]` at any fuel (the args are
    spec-call-free, so they converge trivially). -/
theorem divergent_args_NB (fuel : Nat) (env : Env) :
    intValArgsNB fuel [Expr.var "x"] env = some [env.ints "x"] := by
  simp only [intValArgsNB, intValNB, Option.bind]

/-- THE PIN (E.1): at EVERY fuel the divergent call's none-propagating denotation is
    `none` — it NEVER reaches a genuine value. Contrast Pin B's
    `divergent_call_is_const_bottom`: the BOTTOMING `intVal` is constantly `0`, but
    the BOTTOM-DISTINGUISHING `intValNB` is `none`, which is exactly the distinction
    the #241 fix introduces. -/
theorem divergent_call_NB_is_none :
    ∀ fuel (env : Env), env.specs = Rdiv → intValNB fuel fCall env = none := by
  intro fuel
  induction fuel with
  | zero => intro env _; simp only [fCall, intValNB]
  | succ n ih =>
      intro env h
      simp only [fCall, intValNB, h, Rdiv, if_pos]
      rw [divergent_args_NB]
      simp only [Option.bind]
      exact ih _ (by simp [Env.bindParams, Env.bindInt, h])

/-! ## E.2 — the DIVERGENT registry FAILS the new hypothesis (the load-bearing
    reversal: the cycle-4 divergence is CLOSED). -/

/-- THE PIN (E.2): the DIVERGENT registry does NOT satisfy `RegistryTerminating` —
    no value Converges, because `intValNB` is `none` at every fuel (so it cannot be
    `some v` at any fuel, let alone all fuel ≥ N). The registry §1.2 mandates the
    class REJECT now FAILS the hypothesis named to discharge the class. -/
theorem divergent_registry_fails_the_hypothesis :
    ¬ RegistryTerminating envD fCall := by
  rintro ⟨v, N, hN⟩
  have hsome := hN N (Nat.le_refl N)
  rw [divergent_call_NB_is_none N envD rfl] at hsome
  exact absurd hsome (by simp)

/-- The divergent registry ALSO does not Converge to ANY value (the per-value form,
    the explicit negative the class needs). -/
theorem divergent_registry_no_convergence :
    ∀ v, ¬ Converges fCall envD v := by
  intro v hc
  exact divergent_registry_fails_the_hypothesis ⟨v, hc⟩

/-! ## E.3 — `stabilization_exists` now has NON-IDENTITY content. -/

/-- THE PIN (E.3): `stabilization_exists` is NOT discharge-able on the divergent
    registry — there is NO `RegistryTerminating envD fCall` term to feed it (E.2).
    Contrast the cycle-4 form, where the bottom witness `⟨0, 0, …⟩` supplied the
    hypothesis FOR FREE and `stabilization_exists` then affirmed the bottom-poisoned
    `0`. The resolved `stabilization_exists` carries the agreement lemma's content:
    its hypothesis is genuine CONVERGENCE, which the divergent registry lacks. -/
theorem stabilization_exists_unreachable_on_divergence :
    ¬ ∃ (h : RegistryTerminating envD fCall), True := by
  rintro ⟨h, _⟩
  exact divergent_registry_fails_the_hypothesis h

/-! ## E.4 — a GENUINE (convergent) registry still discharges (no over-rejection):
    `g(x) = 5` converges to 5 and stabilizes to 5. -/

def Rgood : Registry := fun n =>
  if n = "g" then some ⟨["x"], Expr.intLit 5⟩ else none

def envG : Env :=
  { ints := fun _ => 0
    seqs := fun _ => []
    optres := fun _ => OptResVal.none_
    specs := Rgood }

def gCall : Expr := Expr.specCall "g" [Expr.var "x"]

/-- The genuine call's none-propagating denotation reaches `some 5` at every fuel ≥ 1
    (the body `5` is spec-call-free, so it converges immediately on resolution). -/
theorem genuine_call_NB (fuel : Nat) :
    intValNB (fuel + 1) gCall envG = some 5 := by
  have hargs : intValArgsNB (fuel + 1) [Expr.var "x"] envG = some [envG.ints "x"] := by
    simp only [intValArgsNB, intValNB, Option.bind]
  have hres : envG.specs "g" = some ⟨["x"], Expr.intLit 5⟩ := by
    simp only [envG, Rgood, if_pos]
  simp only [gCall, intValNB, hres, hargs, Option.bind, intValNB]

/-- THE PIN (E.4a): the GENUINE registry CONVERGES to 5 — the fix did NOT
    over-reject; a real dec-valid item still supplies `RegistryTerminating`. -/
theorem genuine_registry_converges : Converges gCall envG 5 :=
  ⟨1, fun fuel hfuel => by
    obtain ⟨k, rfl⟩ := Nat.exists_eq_add_of_le hfuel
    rw [Nat.add_comm]; exact genuine_call_NB k⟩

/-- THE PIN (E.4b): so the GENUINE registry SATISFIES `RegistryTerminating`, and
    `stabilization_exists` discharges it to a GENUINE stabilized value (5, by the
    agreement lemma) — NOT a bottom-poisoned artifact. -/
theorem genuine_registry_stabilizes : stabilizes gCall envG 5 :=
  converges_imp_stabilizes genuine_registry_converges

/-- THE PIN (E.4c): the genuine value is 5, by uniqueness — the contract
    `ens: result == g(x)` is now genuinely about 5, not a bottom. -/
theorem genuine_registry_value_is_five :
    ∀ v, stabilizes gCall envG v → v = 5 := fun _ hv =>
  stabilizes_unique hv genuine_registry_stabilizes

end Thermite.PinRegTerm
