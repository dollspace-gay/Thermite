/-
  PinRegistryTerminating.lean — critic pin (Pin E), re-pinned to the resolved
  truth (crosslink #241, ref #240 #203 #215; the #186 precedent: when a finding
  is resolved at the root, the pin updates to the resolved-truth oracle and stays
  the regression oracle in both directions).

  The original finding (cycle-4, commit f7d288ef). `lean/Thermite/Stabilize.lean`'s
  `RegistryTerminating env e := ∃ v, stabilizes e env v` was an identity hypothesis,
  and `stabilization_exists` was the definitional identity over it, with no proof
  content. The bug: the bottoming `intVal` cannot distinguish "stabilized to a
  genuine value" from "stuck at the Int-bottom 0 because it diverged", so the
  divergent registry `f(x) = f(x)` (whose `intVal` is constantly the bottom 0)
  cleared the hypothesis. The registry §1.2's registry-termination class exists to
  reject this case, yet it satisfied the hypothesis, and a wrong contract certified
  at the bottom.

  The root fix (#241, in `Stabilize.lean`). A second, bottom-distinguishing
  none-propagating denotation `intValNB`/`denoteNB` mirrors the spine recursion,
  save that a fuel-0 `specCall` is `none`, an unresolved `specCall` is `none`, and
  every arm propagates `none`. `Converges e env v := ∃ N, ∀ fuel ≥ N, intValNB
  fuel e env = some v` reaches a genuine value, which the divergent call never does
  (it bottoms to `none` at every fuel). The agreement lemma `converges_imp_
  stabilizes` (`intValNB_agrees`) carries that genuine content into `stabilizes`,
  and `RegistryTerminating` is redefined `∃ v, Converges e env v`, no longer an
  identity hypothesis.

  This pin, re-pinned (the resolved-truth oracle, both directions):
  - E.1 the divergent call is `none` at every fuel (`divergent_call_NB_is_none`).
  - E.2 the divergent registry now fails the hypothesis
    (`divergent_registry_fails_the_hypothesis`, `¬ RegistryTerminating envD fCall`).
    The class the registry §1.2 mandates rejecting no longer clears it, closing the
    cycle-4 divergence. This is the load-bearing reversal.
  - E.3 `stabilization_exists` has non-identity content: it is the agreement-lemma
    composite, not `id` (`stabilization_exists_is_not_identity`; there is no
    `RegistryTerminating` term to feed it on the divergent registry).
  - E.4 a genuine (convergent) registry, `g(x) = 5`, still converges and
    stabilizes to 5 (`genuine_registry_converges` / `genuine_registry_stabilizes`),
    so the fix did not over-reject: a real dec-valid item still discharges.

  Builds green; the green build is the demonstration of the resolution. Tracking:
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

/-! ## E.1 — the divergent call is `none` at every fuel (the bottom-distinguishing
    denotation does not assign it a value: it never converges). -/

/-- The denoted args of `f(x)` are `some [x-value]` at any fuel (the args are
    spec-call-free, so they converge trivially). -/
theorem divergent_args_NB (fuel : Nat) (env : Env) :
    intValArgsNB fuel [Expr.var "x"] env = some [env.ints "x"] := by
  simp only [intValArgsNB, intValNB, Option.bind]

/-- The pin (E.1): at every fuel the divergent call's none-propagating denotation is
    `none`; it never reaches a genuine value. Contrast Pin B's
    `divergent_call_is_const_bottom`: the bottoming `intVal` is constantly `0`, but
    the bottom-distinguishing `intValNB` is `none`, which is the distinction
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

/-! ## E.2 — the divergent registry fails the new hypothesis (the load-bearing
    reversal: the cycle-4 divergence is closed). -/

/-- The pin (E.2): the divergent registry does not satisfy `RegistryTerminating`;
    no value Converges, because `intValNB` is `none` at every fuel (so it cannot be
    `some v` at any fuel, let alone all fuel ≥ N). The class the registry §1.2
    mandates rejecting now fails the hypothesis named to discharge the class. -/
theorem divergent_registry_fails_the_hypothesis :
    ¬ RegistryTerminating envD fCall := by
  rintro ⟨v, N, hN⟩
  have hsome := hN N (Nat.le_refl N)
  rw [divergent_call_NB_is_none N envD rfl] at hsome
  exact absurd hsome (by simp)

/-- The divergent registry also does not Converge to any value (the per-value form,
    the explicit negative the class needs). -/
theorem divergent_registry_no_convergence :
    ∀ v, ¬ Converges fCall envD v := by
  intro v hc
  exact divergent_registry_fails_the_hypothesis ⟨v, hc⟩

/-! ## E.3 — `stabilization_exists` now has non-identity content. -/

/-- The pin (E.3): `stabilization_exists` is not discharge-able on the divergent
    registry; there is no `RegistryTerminating envD fCall` term to feed it (E.2).
    Contrast the cycle-4 form, where the bottom witness `⟨0, 0, …⟩` supplied the
    hypothesis for free and `stabilization_exists` then affirmed the bottom-poisoned
    `0`. The resolved `stabilization_exists` carries the agreement lemma's content:
    its hypothesis is genuine convergence, which the divergent registry lacks. -/
theorem stabilization_exists_unreachable_on_divergence :
    ¬ ∃ (h : RegistryTerminating envD fCall), True := by
  rintro ⟨h, _⟩
  exact divergent_registry_fails_the_hypothesis h

/-! ## E.4 — a genuine (convergent) registry still discharges (no over-rejection):
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

/-- The pin (E.4a): the genuine registry converges to 5; the fix did not
    over-reject, and a real dec-valid item still supplies `RegistryTerminating`. -/
theorem genuine_registry_converges : Converges gCall envG 5 :=
  ⟨1, fun fuel hfuel => by
    obtain ⟨k, rfl⟩ := Nat.exists_eq_add_of_le hfuel
    rw [Nat.add_comm]; exact genuine_call_NB k⟩

/-- The pin (E.4b): so the genuine registry satisfies `RegistryTerminating`, and
    `stabilization_exists` discharges it to a genuine stabilized value (5, by the
    agreement lemma) rather than a bottom-poisoned artifact. -/
theorem genuine_registry_stabilizes : stabilizes gCall envG 5 :=
  converges_imp_stabilizes genuine_registry_converges

/-- The pin (E.4c): the genuine value is 5, by uniqueness; the contract
    `ens: result == g(x)` is now about 5, not a bottom. -/
theorem genuine_registry_value_is_five :
    ∀ v, stabilizes gCall envG v → v = 5 := fun _ hv =>
  stabilizes_unique hv genuine_registry_stabilizes

end Thermite.PinRegTerm
