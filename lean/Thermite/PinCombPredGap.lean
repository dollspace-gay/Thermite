/-
  PinCombPredGap.lean — CRITIC PIN (Pin F), kernel-checked (re-audit of the #241
  root fix, commit f4ae4ee9; ref #240 #203 #215).

  THE FINDING. `Stabilize.lean`'s NB mirror discipline is stated as "mirrors the
  spine recursion EXACTLY save: a fuel-0 `specCall` → `none`; an unresolved
  `specCall` → `none`; every arm PROPAGATES `none`" (the module header; the
  commit message; `.design/verified/proof-backends.md` §1.2 increment-(ii) block
  "every arm propagates `none`"). There is a THIRD silent divergence: `denoteNB`'s
  Prop-combinator arm none-gates ONLY the slice/`seq2`/`idx` subterms and then
  carries the spine `denote` proposition VERBATIM — the per-element PREDICATE BODY
  is never gated (the arm's own comment acknowledges the exception, but the header,
  the commit message, and the doc state the universal discipline, and §4 claims the
  class "discharges CONVERGENCE of every reachable spec-call" / the hypothesis is
  "a GENUINE precondition a divergent registry cannot forge").

  CONSEQUENCE (this pin): a divergent spec-call reachable ONLY through a
  Prop-combinator predicate body does NOT poison `intValNB`. For

      eDeep := count_where(s, |x| forall_in(s, |y| f(x) > 0)),   f(x) = f(x)

  the inner `f(x)` bottoms the spine `intVal` to the Int-bottom `0` at every fuel,
  so the carried predicate `0 > 0` is FALSE, the count is the bottom-poisoned `0`,
  and `intValNB fuel eDeep envD = some 0` at EVERY fuel — so the DIVERGENT registry
  FORGES `RegistryTerminating envD eDeep` (F.2), the very hypothesis Pin E.2 pins
  as unforgeable on the direct call. `stabilization_exists` then delivers the
  bottom-poisoned stabilized value `0` (F.3) — the Pin E wrong-contract shape
  (`ens: result == 0` certifies against a divergent registry), ONE CONSTRUCTOR
  DEEPER than the cycle-4 finding #241 closed.

  THE PIN (the defect oracle, to be INVERTED by the root fix — the #186 precedent):
  - F.0 `eDeep` is NOT spec-call-free (the divergent `f` is genuinely reachable).
  - F.1 `intValNB fuel eDeep envD = some 0` at every fuel (the NB denotation
    assigns the divergent expression a "genuine" value).
  - F.2 `RegistryTerminating envD eDeep` HOLDS — the forgery (contrast Pin E.2's
    `divergent_registry_fails_the_hypothesis` on the direct call).
  - F.3 the forged hypothesis feeds `stabilization_exists`, and the stabilized
    value is the bottom-poisoned `0`.

  Builds GREEN; the green build IS the demonstration of the defect. Tracking:
  crosslink #242.
-/
import Thermite.Stabilize

namespace Thermite.PinCombPredGap

/-! ## F.0 — the divergent registry (Pin E's, verbatim) + the deep expression -/

def Rdiv : Registry := fun n =>
  if n = "f" then some ⟨["x"], Expr.specCall "f" [Expr.var "x"]⟩ else none

/-- One-element slice so the count genuinely consults the per-element predicate. -/
def envD : Env :=
  { ints := fun _ => 0
    seqs := fun _ => [0]
    optres := fun _ => OptResVal.none_
    specs := Rdiv }

def fCall : Expr := Expr.specCall "f" [Expr.var "x"]

/-- `f(x) > 0` — the per-element predicate body containing the DIVERGENT call. -/
def pBody : Expr := Expr.cmp CmpOp.gt fCall (Expr.intLit 0)

/-- `forall_in(s, |y| f(x) > 0)` — the Prop-combinator wrapping the divergent call. -/
def forallExpr : Expr :=
  Expr.comb CombName.forallIn (Expr.seqVar "s") none none (some (Pred.mk "y" pBody))

/-- `count_where(s, |x| forall_in(s, |y| f(x) > 0))` — a WELL-FORMED INT-sorted
    (`ResultKind::Usize`) expression whose ONLY spec-call is the divergent `f`,
    reachable only through the Prop-combinator predicate body. -/
def eDeep : Expr :=
  Expr.comb CombName.countWhere (Expr.seqVar "s") none none (some (Pred.mk "x" forallExpr))

/-- THE PIN (F.0): `eDeep` is NOT spec-call-free — the divergent `f` is in its
    full-expression-position closure, so the REGISTRY-TERMINATION class governs it
    (this is NOT the trivial `converges_specCallFree` convergence). -/
theorem eDeep_not_specCallFree : specCallFree eDeep = false := by
  simp [eDeep, forallExpr, pBody, fCall, specCallFree, optPredFree, predFree,
    optExprFree]

/-! ## F.1 — the NB denotation assigns the divergent expression a value -/

/-- `bindParams` never touches the registry. -/
theorem bindParams_specs : ∀ (ps : List String) (vs : List Int) (env : Env),
    (Env.bindParams env ps vs).specs = env.specs
  | [], _, _ => rfl
  | _ :: _, [], _ => rfl
  | p :: ps, v :: vs, env => bindParams_specs ps vs (env.bindInt p v)

/-- The SPINE bottoms the divergent call to the Int-bottom `0` at every fuel
    (Pin B's fact, local copy — this is what poisons the carried predicate). -/
theorem divergent_call_bottoms :
    ∀ fuel (env : Env), env.specs = Rdiv → intVal fuel fCall env = 0 := by
  intro fuel
  induction fuel with
  | zero => intro env _; simp only [fCall, intVal]
  | succ n ih =>
      intro env h
      simp only [fCall, intVal, h, Rdiv, if_pos]
      exact ih _ (by rw [bindParams_specs]; exact h)

/-- THE GAP, isolated: `denoteNB` on the Prop-combinator is `some` at EVERY fuel —
    the predicate body's divergent call NEVER poisons it (only slice/`seq2`/`idx`
    are gated; the carried proposition is the spine `denote` verbatim). -/
theorem forall_NB_some (fuel : Nat) (env : Env) :
    denoteNB fuel forallExpr env = some (denote fuel forallExpr env) := by
  unfold forallExpr
  unfold denoteNB
  simp only [seqValNB, Option.bind]

/-- The carried proposition is FALSE — bottom-poisoned, not genuine: at every fuel
    the inner `intVal` of `f(x)` is the bottom `0`, so `f(x) > 0` fails at the
    single element. -/
theorem forall_denote_false (fuel : Nat) (env : Env)
    (h : env.specs = Rdiv) (hs : env.seqs "s" = [0]) :
    ¬ denote fuel forallExpr env := by
  intro hP
  unfold forallExpr at hP
  unfold denote at hP
  simp only [seqVal, hs] at hP
  have h0 := hP 0 (by simp)
  simp only [pBody] at h0
  simp only [denote] at h0
  rw [divergent_call_bottoms fuel _ (by simp [Env.bindInt, h])] at h0
  simp only [intVal] at h0
  omega

/-- THE PIN (F.1): `intValNB fuel eDeep envD = some 0` at EVERY fuel — the
    none-propagating denotation reaches a "genuine" value for an expression whose
    reachable spec-call DIVERGES (the bottom-poisoned count: the predicate is
    `False`'d by the spine bottom, so nothing is counted). Contrast Pin E.1's
    `divergent_call_NB_is_none` on the direct call. -/
theorem eDeep_NB (fuel : Nat) : intValNB fuel eDeep envD = some 0 := by
  unfold eDeep
  unfold intValNB
  have hs : seqValNB fuel (Expr.seqVar "s") envD = some [0] := by
    simp only [seqValNB]; rfl
  rw [hs]
  simp only [Option.bind]
  rw [countWhereValNB_cons]
  rw [forall_NB_some fuel (envD.bindInt "x" 0)]
  simp only [Option.bind, countWhereValNB]
  have hfalse : ¬ denote fuel forallExpr (envD.bindInt "x" 0) :=
    forall_denote_false fuel _ (by simp [Env.bindInt, envD]) (by simp [Env.bindInt, envD])
  rw [if_neg hfalse]
  simp

/-! ## F.2 — the FORGERY: the divergent registry SATISFIES `RegistryTerminating` -/

/-- THE PIN (F.2, load-bearing): the DIVERGENT registry FORGES the
    REGISTRY-TERMINATION hypothesis through the Prop-combinator predicate gap —
    `RegistryTerminating envD eDeep` HOLDS, contradicting §1.2/§4's "a GENUINE
    precondition a divergent registry cannot forge" / "discharges CONVERGENCE of
    every reachable spec-call". The cycle-4 #241 finding, one constructor deeper. -/
theorem divergent_registry_forges_the_hypothesis : RegistryTerminating envD eDeep :=
  ⟨0, 0, fun fuel _ => eDeep_NB fuel⟩

/-! ## F.3 — the forged hypothesis delivers the bottom-poisoned stabilized value -/

/-- THE PIN (F.3): `stabilization_exists` is DISCHARGEABLE on the divergent
    registry (contrast Pin E.3), and the stabilized value it pins is the
    bottom-poisoned `0` — so `ens: result == 0` certifies against a registry the
    class exists to reject (uniqueness makes `0` the ONLY value the §4 form binds). -/
theorem stabilization_reachable_on_divergence : stabilizes eDeep envD 0 :=
  converges_imp_stabilizes ⟨0, fun fuel _ => eDeep_NB fuel⟩

end Thermite.PinCombPredGap
