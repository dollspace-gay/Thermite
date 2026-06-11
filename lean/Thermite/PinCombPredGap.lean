/-
  PinCombPredGap.lean — CRITIC PIN (Pin F), RE-PINNED as the RESOLVED TRUTH after the
  #242 root fix (commit for #242; ref #241 #240 #203 #215). The #186 precedent: the
  defect oracle is INVERTED — the divergent-through-predicate registry now FAILS
  `RegistryTerminating`, and a genuine-registry positive guards over-rejection.

  THE ORIGINAL FINDING (cycle, pre-#242). `Stabilize.lean`'s `denoteNB` Prop-combinator
  arm none-gated ONLY the slice/`seq2`/`idx` subterms and then carried the spine
  `denote` proposition VERBATIM — the per-element PREDICATE BODY was never gated. For

      eDeep := count_where(s, |x| forall_in(s, |y| f(x) > 0)),   f(x) = f(x)

  the inner `f(x)` bottoms the spine `intVal` to the Int-bottom `0` at every fuel, the
  carried predicate `0 > 0` is FALSE, the count is the bottom-poisoned `0`, and
  `intValNB fuel eDeep envD = some 0` at EVERY fuel — so the DIVERGENT registry FORGED
  `RegistryTerminating envD eDeep`, the very hypothesis Pin E.2 pins as unforgeable on
  the direct call. `stabilization_exists` then delivered the bottom-poisoned `0`.

  THE FIX (#242, now landed). `denoteNB`'s Prop-combinator arm routes the per-element
  predicate body through `denoteNB` at each slice element and gates emission on EVERY
  element being `some` (`Stabilize.predGateNB`). The divergent `f(x)` inside the
  predicate body is now `intValNB = none`, so the gate is `none`, so `denoteNB
  forallExpr = none`, so `countWhereValNB` is `none`, so `intValNB eDeep = none` at every
  fuel — the forgery is GONE.

  THE RE-PIN (the resolved truth):
  - F.0 `eDeep` is NOT spec-call-free (the divergent `f` is genuinely reachable) — UNCHANGED.
  - F.1 `intValNB fuel eDeep envD = none` at every fuel (the NB denotation now REFUSES to
    assign the divergent expression a value — the predicate-body gate poisons it). INVERTED
    from the old `some 0`.
  - F.2 `¬ RegistryTerminating envD eDeep` — the divergent registry can no longer FORGE the
    hypothesis through a Prop-combinator predicate body (the F.2 inversion: contrast the old
    `divergent_registry_forges_the_hypothesis`).
  - F.+ (the over-rejection guard, the #241 precedent): the SAME eDeep shape under a
    GENUINE registry `g(x) = 1` CONVERGES — `RegistryTerminating envG eGen` HOLDS and
    `stabilization_exists` delivers a genuine stabilized value — so the gate rejects ONLY
    divergence, not every predicate-body spec-call.

  Builds GREEN with NO `sorry`; the green build IS the demonstration the divergence is
  resolved. Tracking: crosslink #242.
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

/-- THE PIN (F.0, UNCHANGED): `eDeep` is NOT spec-call-free — the divergent `f` is in its
    full-expression-position closure, so the REGISTRY-TERMINATION class governs it
    (this is NOT the trivial `converges_specCallFree` convergence). -/
theorem eDeep_not_specCallFree : specCallFree eDeep = false := by
  simp [eDeep, forallExpr, pBody, fCall, specCallFree, optPredFree, predFree,
    optExprFree]

/-! ## F.1 — the NB denotation now REFUSES to value the divergent expression (INVERTED) -/

/-- `bindParams` never touches the registry. -/
theorem bindParams_specs : ∀ (ps : List String) (vs : List Int) (env : Env),
    (Env.bindParams env ps vs).specs = env.specs
  | [], _, _ => rfl
  | _ :: _, [], _ => rfl
  | p :: ps, v :: vs, env => bindParams_specs ps vs (env.bindInt p v)

/-- The divergent call is `none` under `intValNB` at EVERY fuel (Pin E's fact, local
    copy — the none-propagating denotation REFUSES a value to a divergent specCall). -/
theorem divergent_call_NB_none :
    ∀ fuel (env : Env), env.specs = Rdiv → intValNB fuel fCall env = none := by
  intro fuel
  induction fuel with
  | zero => intro env _; simp only [fCall, intValNB]
  | succ n ih =>
      intro env h
      simp only [fCall, intValNB, h, Rdiv, if_pos]
      rw [show intValArgsNB (n + 1) [Expr.var "x"] env
          = some [env.ints "x"] from by simp only [intValArgsNB, intValNB, Option.bind]]
      simp only [Option.bind]
      exact ih _ (by rw [bindParams_specs]; exact h)

/-- The predicate-body gate over the divergent call is `none`: `pBody = f(x) > 0` reads
    `intValNB f(x)` which is `none`, so the whole `denoteNB pBody` is `none`. -/
theorem pBody_NB_none (fuel : Nat) (env : Env) (h : env.specs = Rdiv) :
    denoteNB fuel pBody env = none := by
  simp only [pBody, denoteNB, divergent_call_NB_none fuel env h, Option.bind]

/-- THE GAP, now CLOSED: `denoteNB` on the Prop-combinator is `none` — the predicate
    body's divergent call POISONS it (the #242 gate routes the body through `denoteNB`
    per element and propagates `none`). Contrast the old `forall_NB_some`. -/
theorem forall_NB_none (fuel : Nat) (env : Env) (h : env.specs = Rdiv)
    (hs : env.seqs "s" = [0]) :
    denoteNB fuel forallExpr env = none := by
  unfold forallExpr
  unfold denoteNB
  simp only [seqValNB, hs, Option.bind, predGateNB_cons]
  rw [pBody_NB_none fuel (env.bindInt "y" 0) (by rw [Env.bindInt]; exact h)]

/-- THE PIN (F.1, INVERTED): `intValNB fuel eDeep envD = none` at EVERY fuel — the
    none-propagating denotation REFUSES a value to an expression whose reachable
    spec-call DIVERGES (the predicate-body gate poisons the count). Contrast the old
    `eDeep_NB : intValNB fuel eDeep envD = some 0`. -/
theorem eDeep_NB_none (fuel : Nat) : intValNB fuel eDeep envD = none := by
  unfold eDeep
  unfold intValNB
  have hs : seqValNB fuel (Expr.seqVar "s") envD = some [0] := by
    simp only [seqValNB]; rfl
  rw [hs]
  simp only [Option.bind]
  rw [countWhereValNB_cons]
  rw [forall_NB_none fuel (envD.bindInt "x" 0) (by rw [Env.bindInt]; rfl) (by rw [Env.bindInt]; rfl)]
  simp only [Option.bind]

/-! ## F.2 — the FORGERY is GONE: the divergent registry FAILS `RegistryTerminating` -/

/-- THE PIN (F.2, the load-bearing inversion): the divergent registry can NO LONGER forge
    the REGISTRY-TERMINATION hypothesis through the Prop-combinator predicate body —
    `¬ RegistryTerminating envD eDeep`, restoring §1.2/§4's "a GENUINE precondition a
    divergent registry cannot forge" / "discharges CONVERGENCE of every reachable
    spec-call". Contrast the old `divergent_registry_forges_the_hypothesis`. -/
theorem divergent_registry_fails_the_hypothesis : ¬ RegistryTerminating envD eDeep := by
  rintro ⟨v, N, hN⟩
  have := hN N (Nat.le_refl N)
  rw [eDeep_NB_none N] at this
  exact absurd this (by simp)

/-! ## F.+ — the OVER-REJECTION GUARD: a GENUINE registry of the same shape CONVERGES -/

/-- A GENUINE (terminating) registry: `g(x) = 1` — the SAME eDeep shape, but the spec-fn
    returns a constant instead of diverging. -/
def Rgen : Registry := fun n =>
  if n = "f" then some ⟨["x"], Expr.intLit 1⟩ else none

def envG : Env :=
  { ints := fun _ => 0
    seqs := fun _ => [0]
    optres := fun _ => OptResVal.none_
    specs := Rgen }

/-- The genuine call resolves to `some 1` at every POSITIVE fuel (`intValNB`). -/
theorem genuine_call_NB_one :
    ∀ fuel (env : Env), env.specs = Rgen → intValNB (fuel + 1) fCall env = some 1 := by
  intro fuel env h
  simp only [fCall, intValNB, h, Rgen, if_pos]
  rw [show intValArgsNB (fuel + 1) [Expr.var "x"] env
      = some [env.ints "x"] from by simp only [intValArgsNB, intValNB, Option.bind]]
  simp only [Option.bind]

/-- The predicate body `f(x) > 0` denotes `some (1 > 0)` under the genuine registry. -/
theorem pBody_NB_genuine (fuel : Nat) (env : Env) (h : env.specs = Rgen) :
    denoteNB (fuel + 1) pBody env = some ((1 : Int) > 0) := by
  simp only [pBody, denoteNB, genuine_call_NB_one fuel env h, Option.bind, intValNB]

/-- The inner `forall_in` NB-denotes to `some` of an EXPLICIT genuine proposition at
    positive fuel — the predicate gate now SUCCEEDS (`some`) because the genuine call
    resolves. The carried proposition is the spine `denote` form (agreement is reflexive). -/
theorem forall_NB_genuine_eq (fuel : Nat) (env : Env) (h : env.specs = Rgen)
    (hs : env.seqs "s" = [0]) :
    denoteNB (fuel + 1) forallExpr env
      = some (denote (fuel + 1) forallExpr env) := by
  unfold forallExpr
  unfold denoteNB
  simp only [seqValNB, hs, Option.bind, predGateNB_cons]
  rw [pBody_NB_genuine fuel (env.bindInt "y" 0) (by rw [Env.bindInt]; exact h)]
  simp only [predGateNB]

/-- The spine `forall_in(s, |y| f(x) > 0)` denotes TRUE under the genuine registry: the
    body is `1 > 0` at the (single) bound element, which holds. -/
theorem forall_denote_genuine (fuel : Nat) (env : Env) (h : env.specs = Rgen)
    (hs : env.seqs "s" = [0]) :
    denote (fuel + 1) forallExpr env := by
  unfold forallExpr
  unfold denote
  simp only [seqVal, hs]
  intro i hi
  simp only [pBody, denote]
  rw [show intVal (fuel + 1) fCall (env.bindInt "y" (seqIdx [0] i)) = 1 from by
    simp only [fCall, intVal, h, Rgen, if_pos, Env.bindInt, intValArgs]]
  simp only [intVal]; omega

/-- THE PIN (F.+, the over-rejection guard): the SAME eDeep shape under the GENUINE
    registry `g(x) = 1` reaches a value under `intValNB` at positive fuel — so
    `RegistryTerminating envG eDeep` HOLDS and `stabilization_exists` delivers a genuine
    stabilized value. The #242 gate rejects DIVERGENCE, not every predicate-body
    spec-call (the #241 precedent: a fix that rejected everything would be unsound the
    other way). The convergent value is the genuine count `1` (the single element's
    `forall_in` holds). -/
theorem genuine_registry_satisfies_the_hypothesis : RegistryTerminating envG eDeep := by
  refine ⟨1, 1, fun fuel hfuel => ?_⟩
  obtain ⟨k, rfl⟩ : ∃ k, fuel = k + 1 := ⟨fuel - 1, by omega⟩
  unfold eDeep intValNB
  have hs : seqValNB (k + 1) (Expr.seqVar "s") envG = some [0] := by
    simp only [seqValNB]; rfl
  rw [hs]
  simp only [Option.bind, countWhereValNB_cons]
  rw [forall_NB_genuine_eq k (envG.bindInt "x" 0) (by rw [Env.bindInt]; rfl)
    (by rw [Env.bindInt]; rfl)]
  simp only [countWhereValNB]
  rw [if_pos (forall_denote_genuine k (envG.bindInt "x" 0) (by rw [Env.bindInt]; rfl)
    (by rw [Env.bindInt]; rfl))]
  simp only [Option.some.injEq]; omega

/-- `stabilization_exists` is DISCHARGEABLE on the genuine registry, delivering a genuine
    stabilized value — the resolved-side counterpart to F.2's rejection of divergence. -/
theorem genuine_stabilization_exists : ∃ v, stabilizes eDeep envG v :=
  stabilization_exists genuine_registry_satisfies_the_hypothesis
