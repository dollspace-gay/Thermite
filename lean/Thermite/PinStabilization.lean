/-
  CRITIC PIN (cycle 4) — proof-backends.md §4 "the stabilized form": two soundness
  gaps in the #213-revised obligation form, kernel-checked against the shipped spine
  (`lean/Thermite/Denote.lean`). `stabilizes`/`stabilizesProp` are copied VERBATIM
  from the doc's §4 definition block (they are not yet in the spine — the named
  increment-(ii) prerequisite), so these theorems pin the DOC's form, not a strawman.

  PIN A — the unbound `rbody` (the result-binding hole).
  The §4 exported theorem displays `stabilizesProp ens (Env.bindInt env "result"
  rbody)` with `rbody` bound by NOTHING — no `∀ r, stabilizes body env r →`
  hypothesis; the only prose is "binds via `Env.bindInt` after stabilization" and the
  only computational story in §4 is the fuel₀ exporter HINT. Registry: f(x) = g(x),
  g(x) = 5 (dec-bounded, complete — every §4 gate passes). The item's body `f(x)`
  stabilizes to 5 (`body_stabilizes_to_5`); at the hint fuel 1 it is the BOTTOM 0
  (`rbody_at_hint_fuel_is_bottom`). For the WRONG contract `ens: result == 0` (which
  the real item, result 5, VIOLATES — `wrong_contract_fails_at_true_value`), the §4
  obligation with `rbody` rendered at the hint fuel DISCHARGES
  (`wrong_contract_certifies_with_underfuelled_rbody`): the #213 Int-bottom
  unsoundness RECURS on the body side. The form must quantify the result value
  (`∀ r, stabilizes body env r → …`); the doc does not say so.

  PIN B — the non-dec registry is NOT safely-unprovable; it is bottom-poisoned.
  §4's soundness argument is scoped "for a DEC-MEASURED (terminating) registry", but
  nothing on the Lean path discharges that hypothesis: the parser enforces dec
  PRESENCE only (`SpecFnItem::dec` mandatory); dec VALIDITY (the measure actually
  decreases) is proven ONLY by Verus — and the Lean rung sits exactly downstream of
  a Verus `Unknown` (REQ-3.1 remaps a witness-less failure, e.g. a failed spec-fn
  termination proof, to `Unknown` → degrade → Lean attempts). For the divergent
  registry f(x) = f(x), the fuel denotation is CONSTANTLY the bottom 0
  (`divergent_call_is_const_bottom`), so `stabilizes` HOLDS with the bottom value
  (`divergent_registry_stabilizes_to_bottom`) and the contract `ens: result == f(x)`
  STABILIZES TO TRUE at result = 0 (`divergent_contract_certifies`) — the obligation
  is provable with a bottom-poisoned meaning, not unprovable. The doc neither names
  who guarantees dec-validity reaches the exporter nor assigns registry-termination
  as a mandatory obligation class (REQ-7's TERMINATION covers the item's own
  loop/recursion via `while_rule`, not the registry).

  Tracking: see the cycle-4 crosslink issues referenced in the commit. This file is
  the audit artifact; like PinIntBottom.lean it must keep compiling.
-/
import Thermite.Denote

namespace Thermite.PinStab

/-- §4's stabilization relation, verbatim from the doc (the increment-(ii)
    spine prerequisite, not yet in the spine). -/
def stabilizes (e : Expr) (env : Env) (v : Int) : Prop :=
  ∃ N, ∀ fuel, fuel ≥ N → intVal fuel e env = v

/-- §4's Prop-side analogue, verbatim from the doc. -/
def stabilizesProp (e : Expr) (env : Env) : Prop :=
  ∃ N, ∀ fuel, fuel ≥ N → denote fuel e env

/-! ## PIN A — the unbound `rbody` -/

/-- f(x) = g(x); g(x) = 5. Real-bodied, complete, dec-bounded — every §4 gate passes. -/
def Ra : Registry := fun n =>
  if n = "f" then some ⟨["x"], Expr.specCall "g" [Expr.var "x"]⟩
  else if n = "g" then some ⟨["x"], Expr.intLit 5⟩
  else none

/-- The PURE-CONTRACT item's body: `f(x)` (unfolding depth 2). -/
def fCall : Expr := Expr.specCall "f" [Expr.var "x"]

def envA : Env :=
  { ints := fun _ => 0
    seqs := fun _ => []
    optres := fun _ => OptResVal.none_
    specs := Ra }

/-- The body STABILIZES to 5 — the real result value of the item. -/
theorem body_stabilizes_to_5 : stabilizes fCall envA 5 := by
  refine ⟨2, ?_⟩
  intro fuel h
  obtain ⟨k, rfl⟩ : ∃ k, fuel = k + 2 := ⟨fuel - 2, by omega⟩
  simp [fCall, intVal, intValArgs, envA, Ra, Env.bindParams, Env.bindInt]

/-- At the exporter's fuel₀ HINT (the static nesting bound under-computed to 1),
    the body's value is the BOTTOM 0, not 5. -/
theorem rbody_at_hint_fuel_is_bottom : intVal 1 fCall envA = 0 := by
  simp [fCall, intVal, intValArgs, envA, Ra, Env.bindParams, Env.bindInt]

/-- `ens: result == 0` — a contract the REAL item (result = 5) VIOLATES. -/
def ensWrong : Expr := Expr.cmp CmpOp.eq (Expr.var "result") (Expr.intLit 0)

/-- THE PIN (A): the §4 displayed obligation, with `rbody` rendered via the only
    computational story §4 gives (the fuel hint), DISCHARGES for the wrong
    contract — an unsound certification. -/
theorem wrong_contract_certifies_with_underfuelled_rbody :
    stabilizesProp ensWrong (envA.bindInt "result" (intVal 1 fCall envA)) := by
  refine ⟨0, fun fuel _ => ?_⟩
  simp [ensWrong, denote, intVal, fCall, intValArgs, envA, Ra,
        Env.bindParams, Env.bindInt]

/-- The honestly-bound result (the stabilized value 5) REFUTES the wrong contract —
    so the discharge above is purely the under-fuelled-rbody artifact. -/
theorem wrong_contract_fails_at_true_value :
    ¬ stabilizesProp ensWrong (envA.bindInt "result" 5) := by
  rintro ⟨N, h⟩
  have := h N (Nat.le_refl N)
  simp [ensWrong, denote, intVal, envA, Env.bindInt] at this

/-! ## PIN B — the non-dec registry stabilizes to the bottom -/

/-- f(x) = f(x): dec PRESENCE is parser-side; this measure does not decrease, and
    only Verus ever proves dec-VALIDITY. Complete and real-bodied — the §4 hard
    gate + the per-name `decide` lemmas all pass. -/
def Rdiv : Registry := fun n =>
  if n = "f" then some ⟨["x"], Expr.specCall "f" [Expr.var "x"]⟩ else none

def envD : Env :=
  { ints := fun _ => 0
    seqs := fun _ => []
    optres := fun _ => OptResVal.none_
    specs := Rdiv }

/-- At EVERY fuel the divergent call denotes the BOTTOM 0: each unfolding
    re-arrives at the same call and the fuel runs out into the catch-all. -/
theorem divergent_call_is_const_bottom :
    ∀ fuel (env : Env), env.specs = Rdiv →
      intVal fuel (Expr.specCall "f" [Expr.var "x"]) env = 0 := by
  intro fuel
  induction fuel with
  | zero => intro env _; simp [intVal]
  | succ n ih =>
      intro env h
      simp only [intVal, h, Rdiv, if_pos, intValArgs]
      exact ih _ (by simp [Env.bindParams, Env.bindInt, h])

/-- THE PIN (B): `stabilizes` ASSIGNS A VALUE (the bottom 0) to the divergent
    call. The form does not become safely-unprovable on a non-dec registry. -/
theorem divergent_registry_stabilizes_to_bottom :
    stabilizes (Expr.specCall "f" [Expr.var "x"]) envD 0 :=
  ⟨0, fun fuel _ => divergent_call_is_const_bottom fuel envD rfl⟩

/-- `ens: result == f(x)` over the divergent registry. -/
def ensDiv : Expr := Expr.cmp CmpOp.eq (Expr.var "result") (Expr.specCall "f" [Expr.var "x"])

/-- THE PIN (B, the certification): the contract over the DIVERGENT spec-fn
    STABILIZES TO TRUE at result = 0 — the §4 obligation is provable with the
    bottom-poisoned meaning `result == 0`, which is not `S`'s intended meaning of
    a divergent call. -/
theorem divergent_contract_certifies :
    stabilizesProp ensDiv (envD.bindInt "result" 0) := by
  refine ⟨0, fun fuel _ => ?_⟩
  simp [ensDiv, denote, intVal, envD, Env.bindInt]
  exact (divergent_call_is_const_bottom fuel _ rfl).symm

end Thermite.PinStab
