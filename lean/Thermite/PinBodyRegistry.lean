/-
  CRITIC PIN (cycle 5, #224) — proof-backends.md §4: the registry HARD GATE
  excludes the BODY's spec-calls, and the #214 ∀r result-binding makes that
  omission unsound. Kernel-checked against the shipped spine
  (`lean/Thermite/Denote.lean`); `stabilizes`/`stabilizesProp` are copied
  VERBATIM from the doc's §4 definition block.

  Authority chain:
  - §4 "Registry population is an EXPORTER-SIDE HARD GATE": "The exporter
    computes `calledSpecFns(item)` (every spec-fn name reachable from
    `req ∪ ens`)" — the BODY is NOT in the gate's reachability set; and the
    exported `R_item` is "exactly calledSpecFns(item)" (the §4 display), so a
    spec-fn called ONLY by the body is omitted from `R_item` BY CONSTRUCTION,
    the gate `calledSpecFns ⊆ dom(R_item)` passes vacuously, and ZERO per-name
    `decide` lemmas are emitted for it (they too range over calledSpecFns).
  - §4 the #214 form: the theorem now binds the result THROUGH
    `∀ r, stabilizes body_expr env r → … ensStable(at r)` — so the BODY's
    denotation is newly load-bearing in the exported obligation, and the §4
    soundness paragraph itself says "each `specCall` reachable from
    `req`/`ens`/THE BODY has a FINITE unfolding depth" — assuming a body
    coverage the gate does not deliver. (§6.1 tier (a) likewise tests
    "no spec-fn appears in `req`/`ens`/body" — the author's own enumeration
    includes the body everywhere EXCEPT the gate.)
  - Spine ground truth: an UNRESOLVED `specCall` bottoms in `intVal` at EVERY
    fuel (`| none => 0` at fuel+1; catch-all `| _, _, _ => 0` at fuel 0) — so
    omission is not "fails to stabilize": it STABILIZES to the bottom, and the
    doc's own uniqueness-of-stabilization lever FORCES `r = 0`.

  The pin: item body `h(x)` with the true source registry `h(x) = 5`; `ens`
  mentions no spec-fn, so calledSpecFns = ∅ and R_item = the EMPTY registry —
  every §4 gate passes. Then:
  - `body_bottoms_at_every_fuel` / `omitted_body_stabilizes_to_bottom`:
    the body stabilizes to 0, not 5, under the omission;
  - `omission_forces_r_zero`: uniqueness forces r = 0;
  - `wrong_contract_certifies_under_body_omission`: the §4 exported obligation
    (the #214 ∀r form, req = true) DISCHARGES for the WRONG contract
    `ens: result == 0` — an unsound certification (the real item's result is 5);
  - `body_stabilizes_to_5_with_full_registry` / `full_registry_forces_r_five` /
    `wrong_contract_fails_with_full_registry`: with the body's spec-fn PRESENT
    the same obligation is REFUTED — so the discharge above is purely the
    gate's req∪ens-only scope. REGISTRY-TERMINATION (REQ-1.2) does not close
    this: with R_item = ∅ the class is not even ASSIGNED ("non-empty registry"),
    and dec-validity of PRESENT fns says nothing about ABSENT ones.

  The fix direction (the generator's, not mine): the gate's reachability set —
  and R_item's population and the per-name lemmas — must include the body's
  spec-calls (req ∪ ens ∪ body_expr) for the PURE-CONTRACT class.

  Tracking: crosslink #224. This file is the audit artifact; like
  PinIntBottom.lean / PinStabilization.lean it must keep compiling.
-/
import Thermite.Denote

namespace Thermite.PinBodyReg

/-- §4's stabilization relation, verbatim from the doc. -/
def stabilizes (e : Expr) (env : Env) (v : Int) : Prop :=
  ∃ N, ∀ fuel, fuel ≥ N → intVal fuel e env = v

/-- §4's Prop-side analogue, verbatim from the doc. -/
def stabilizesProp (e : Expr) (env : Env) : Prop :=
  ∃ N, ∀ fuel, fuel ≥ N → denote fuel e env

/-- The §4 R_item for this item: `ens`/`req` mention NO spec-fn, so
    calledSpecFns(item) = ∅ and R_item = "exactly calledSpecFns(item)" = ∅.
    The gate `∅ ⊆ dom(R_item)` passes vacuously; no `decide` lemmas emitted. -/
def Rempty : Registry := fun _ => none

/-- The TRUE registry (the source's spec-fn defs): h(x) = 5. -/
def Rfull : Registry := fun n =>
  if n = "h" then some ⟨["x"], Expr.intLit 5⟩ else none

/-- The PURE-CONTRACT item's body: `h(x)` — a spec-call reachable from the
    BODY only, not from req ∪ ens. -/
def hCall : Expr := Expr.specCall "h" [Expr.var "x"]

def envO : Env :=
  { ints := fun _ => 0
    seqs := fun _ => []
    optres := fun _ => OptResVal.none_
    specs := Rempty }

def envF : Env :=
  { ints := fun _ => 0
    seqs := fun _ => []
    optres := fun _ => OptResVal.none_
    specs := Rfull }

/-- Under the omitted registry the body bottoms to 0 at EVERY fuel: the
    unresolved name hits `| none => 0` at fuel+1 and the catch-all at fuel 0. -/
theorem body_bottoms_at_every_fuel :
    ∀ fuel, intVal fuel hCall envO = 0 := by
  intro fuel
  cases fuel with
  | zero => simp [hCall, intVal]
  | succ n => simp [hCall, intVal, envO, Rempty]

/-- So the omitted body STABILIZES — to the bottom, not the true value.
    Omission is not "fails to stabilize". -/
theorem omitted_body_stabilizes_to_bottom : stabilizes hCall envO 0 :=
  ⟨0, fun fuel _ => body_bottoms_at_every_fuel fuel⟩

/-- The doc's own #214 lever (uniqueness of stabilization) FORCES r = 0
    under the omission. -/
theorem omission_forces_r_zero : ∀ r, stabilizes hCall envO r → r = 0 := by
  rintro r ⟨N, h⟩
  have := h N (Nat.le_refl N)
  rw [body_bottoms_at_every_fuel N] at this
  exact this.symm

/-- `ens: result == 0` — a contract the REAL item (h(x) = 5, result 5) VIOLATES. -/
def ensWrong : Expr := Expr.cmp CmpOp.eq (Expr.var "result") (Expr.intLit 0)

/-- THE PIN: the §4 exported obligation (the #214 ∀r form, req = true),
    instantiated at the gate-passing omitted-body registry, DISCHARGES for the
    WRONG contract — an unsound certification. -/
theorem wrong_contract_certifies_under_body_omission :
    ∀ r : Int, stabilizes hCall envO r →
      stabilizesProp (Expr.boolLit true) envO →
      stabilizesProp ensWrong (envO.bindInt "result" r) := by
  intro r hr _
  have hz := omission_forces_r_zero r hr
  subst hz
  refine ⟨0, fun fuel _ => ?_⟩
  simp [ensWrong, denote, intVal, envO, Env.bindInt]

/-- Contrast: with the body's spec-fn PRESENT the body stabilizes to 5 … -/
theorem body_stabilizes_to_5_with_full_registry : stabilizes hCall envF 5 := by
  refine ⟨1, ?_⟩
  intro fuel h
  obtain ⟨k, rfl⟩ : ∃ k, fuel = k + 1 := ⟨fuel - 1, by omega⟩
  simp [hCall, intVal, intValArgs, envF, Rfull, Env.bindParams, Env.bindInt]

/-- … uniqueness forces r = 5 … -/
theorem full_registry_forces_r_five : ∀ r, stabilizes hCall envF r → r = 5 := by
  rintro r ⟨N, h⟩
  obtain ⟨M, h5⟩ := body_stabilizes_to_5_with_full_registry
  have h1 := h (max N M) (Nat.le_max_left N M)
  have h2 := h5 (max N M) (Nat.le_max_right N M)
  omega

/-- … and the SAME wrong contract is REFUTED at the true value — the
    omission discharge above is purely the gate's req∪ens-only scope. -/
theorem wrong_contract_fails_with_full_registry :
    ¬ stabilizesProp ensWrong (envF.bindInt "result" 5) := by
  rintro ⟨N, h⟩
  have := h N (Nat.le_refl N)
  simp [ensWrong, denote, intVal, envF, Env.bindInt] at this

end Thermite.PinBodyReg
