/-
  PinRegistryTerminating.lean — CRITIC PIN (Pin E, the #240 audit; ref #203 #215).

  FINDING: `lean/Thermite/Stabilize.lean`'s `RegistryTerminating env e := ∃ v,
  stabilizes e env v` is NOT the REGISTRY-TERMINATION obligation class of
  `.design/verified/proof-backends.md` §1.2, and `stabilization_exists` (shipped
  under the design's name `stabilization_exists_for_dec_bounded`) is the
  definitional identity — zero proof content.

  The authority, two clauses:
  - §1.2 "The class": every spec-fn in `R_item` carries an obligation that its
    `dec` is a VALID well-founded measure (strict descent); discharge is (a)
    Verus's dec-check or (b) a Lean `decreasing_by`-shaped descent proof.
  - §1.2 "The closure": "a divergent spec-fn `f(x) = f(x)` FAILS this class on
    BOTH paths" — the class EXISTS to reject the divergent registry.

  The shipped hypothesis VIOLATES the second clause: for Pin B's divergent
  registry the fuel denotation is constantly the Int-bottom `0`, so
  `stabilizes (f x) envD 0` HOLDS, so `RegistryTerminating envD (f x)` HOLDS —
  the canonical registry the class must REJECT *clears the shipped hypothesis*
  (`divergent_registry_clears_the_hypothesis` below). And the first clause's
  artifacts (a descent fact) are not of type `∃ v, stabilizes e env v`: NOTHING
  in the spine connects dec-validity to stabilization — the load-bearing §4
  implication ("the source dec measure makes every spec-fn's recursion
  well-founded … so the stabilized value EXISTS and equals S's intended
  meaning") is now proven NOWHERE, while the build-blockers REQ row says
  SHIPPED. The amended doc's claim "the hypothesis is EXACTLY what the per-item
  REGISTRY-TERMINATION obligation class REQ-1.2 discharges" is therefore false
  in both directions:
    (i)  class ⇒ hypothesis is the REAL unproven theorem (the doc-author's
         flagged least-confident assertion — descent ⇒ per-env finite
         unfolding ⇒ stabilization), and
    (ii) hypothesis ⇒ class is kernel-REFUTED here: the divergent registry
         satisfies the hypothesis (`hypothesis_is_satisfied_by_divergence`)
         and `stabilization_exists` then AFFIRMS a "stabilized value" for it —
         the bottom-poisoned `0` (`the_affirmed_value_is_the_bottom`), which
         Stabilize.lean's own docstring concedes is "NOT a genuine stabilized
         value". If increment (ii) wires the exporter to take a
         `RegistryTerminating` term as the REGISTRY-TERMINATION discharge,
         the bottom witness supplies it FOR FREE on a divergent registry and
         Pin B's `divergent_contract_certifies` path re-opens THROUGH the very
         hypothesis named to block it.

  This pin is the critic's kernel-checked audit artifact (builds GREEN; the
  green build IS the demonstration of the divergence). Tracking: crosslink #241.
-/
import Thermite.Stabilize

namespace Thermite.PinRegTerm

/-! ## E.0 — the shipped lemma has zero proof content

`RegistryTerminating env e` unfolds DEFINITIONALLY to `stabilization_exists`'s
conclusion: the hypothesis IS the conclusion, by `rfl`. The design's named
supporting lemma (`stabilization_exists_for_dec_bounded`, §4: dec-valid ⇒ the
stabilized value exists *and equals S's intended meaning*) carries content;
the shipped form carries none. -/
theorem hypothesis_is_conclusion (env : Env) (e : Expr) :
    RegistryTerminating env e = (∃ v, stabilizes e env v) := rfl

/-! ## E.1 — the divergent registry CLEARS the shipped hypothesis

§1.2 "The closure": the divergent `f(x) = f(x)` must FAIL the
REGISTRY-TERMINATION class on BOTH discharge paths. Pin B's registry,
reproduced verbatim. -/

def Rdiv : Registry := fun n =>
  if n = "f" then some ⟨["x"], Expr.specCall "f" [Expr.var "x"]⟩ else none

def envD : Env :=
  { ints := fun _ => 0
    seqs := fun _ => []
    optres := fun _ => OptResVal.none_
    specs := Rdiv }

def fCall : Expr := Expr.specCall "f" [Expr.var "x"]

/-- At EVERY fuel the divergent call denotes the Int-bottom `0` (Pin B's
    `divergent_call_is_const_bottom`, against the spine defs). -/
theorem divergent_call_is_const_bottom :
    ∀ fuel (env : Env), env.specs = Rdiv → intVal fuel fCall env = 0 := by
  intro fuel
  induction fuel with
  | zero => intro env _; simp [fCall, intVal]
  | succ n ih =>
      intro env h
      simp only [fCall, intVal, h, Rdiv, if_pos, intValArgs]
      exact ih _ (by simp [Env.bindParams, Env.bindInt, h])

/-- THE PIN (E, part ii): the DIVERGENT registry SATISFIES the shipped
    `RegistryTerminating` — the hypothesis the doc claims is "EXACTLY" the
    REQ-1.2 class is cleared by the exact registry §1.2 mandates the class
    reject. -/
theorem divergent_registry_clears_the_hypothesis :
    RegistryTerminating envD fCall :=
  ⟨0, 0, fun fuel _ => divergent_call_is_const_bottom fuel envD rfl⟩

/-- `stabilization_exists` (the design's `stabilization_exists_for_dec_bounded`)
    AFFIRMS a stabilized value for the canonical NON-dec-valid registry. -/
theorem stabilization_exists_affirms_the_divergent_registry :
    ∃ v, stabilizes fCall envD v :=
  stabilization_exists divergent_registry_clears_the_hypothesis

/-- … and the value it affirms is the BOTTOM-POISONED `0` (by uniqueness), not
    any "intended meaning" — the #213/#214/#215 trap value, supplied as the
    `∀ r, stabilizes body env r →` premise's unique `r`. -/
theorem the_affirmed_value_is_the_bottom :
    ∀ v, stabilizes fCall envD v → v = 0 := fun _ hv =>
  stabilizes_unique hv
    ⟨0, fun fuel _ => divergent_call_is_const_bottom fuel envD rfl⟩

end Thermite.PinRegTerm
