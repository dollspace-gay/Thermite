/-
  CRITIC PIN (cycle 6) — proof-backends.md §4/#224 closure vs §1.2
  REGISTRY-TERMINATION: `dec`-MEASURE-position spec-calls escape the
  `req ∪ ens ∪ body` reachability set, and a measure denoted against the
  closure-populated `R_item` can AFFIRM well-founded descent the SOURCE
  measure does not have — falsely discharging the very class that guards
  Pin B's divergent-registry bottom-poisoning.

  Authority chain:
  - §4 hard gate (#224): "`calledSpecFns(item)` is defined as every spec-fn
    name reachable from `req ∪ ens ∪ body` — TRANSITIVELY: the set is closed
    under 'a reachable spec-fn's OWN body may call further spec-fns'". The
    seed excludes the item's `dec`, and the closure step is over spec-fn
    BODIES only — a spec-fn's own `dec` clause is in NO covered position.
    `R_item` is "exactly calledSpecFns(item)" and the per-name `decide`
    lemmas "range over calledSpecFns" — so a spec-fn called ONLY from a
    `dec` measure is absent from `R_item` and gets NO resolution lemma.
  - §1.2 REQ-1.2 introduces per-spec-fn obligations ABOUT those `dec`
    measures ("EVERY spec-fn in `R_item` carries a per-spec-fn obligation
    that its `dec` measure is VALID"), with the Lean discharge path (b)
    "a `termination_by`/`decreasing_by`-shaped obligation over the encoded
    `R_item`" — and the doc claims categorically (§1.2 closure / §3.1 note)
    that a divergent spec-fn "FAILS this class on BOTH paths" / "cannot
    satisfy" a Lean well-foundedness proof. REQ-1/REQ-1.2 bind the class to
    "the SAME reachability set the §4 hard gate uses" — a set that
    structurally cannot cover the expressions the class is ABOUT.
  - Grammar ground truth: `SpecFnItem.dec : Clause` (`thermite-syntax/src/
    ast.rs`, `Clause` wraps a full `Expr`) — nothing in thermite-design.md
    §4.1/§4.2 restricts a `dec` measure to be specCall-free (`dec
    spec_size(t)` is a natural tree measure). The SHIPPED forge closure has
    the same omission: `reachable_spec_fn_deps` / `collect_block_spec_fn_calls`
    (`forge/src/check.rs`) walk `decl.body` only, never `decl.dec`.
  - Spine ground truth (`Denote.lean`): an unresolved `specCall` bottoms in
    `intVal` at EVERY fuel (`| none => 0` at fuel+1, catch-all at fuel 0),
    while `arith`/`var` are fuel-transparent — so a measure containing an
    out-of-closure spec-call denotes a DIFFERENT measure, stably.

  The pin: measure `x - t(x)` with true source registry `t(x) = x`.
  - REAL measure: `x - x = 0` — CONSTANT, never strictly descends: the
    classic non-well-founded measure of a divergent spec-fn.
  - Measure as denoted against the #224-closure `R_item` (t escapes
    `req ∪ ens ∪ body`, so t ∉ R_item): `t(x)` bottoms to 0, the measure
    denotes `x - 0 = x` — which STRICTLY DESCENDS on the recursive call
    `x → x - 1`. A `decreasing_by`-shaped check stated over the encoded
    `R_item` (REQ-1.2(b), the only registry the exported artifact carries)
    AFFIRMS descent; REGISTRY-TERMINATION discharges; the stabilization
    soundness lemma's dec-VALID hypothesis is falsely satisfied; and Pin B's
    `divergent_contract_certifies` (PinStabilization.lean) becomes reachable
    at the certificate level again — one position to the left of #224.

  Fix direction (the generator's, not mine): either extend the closure seed
  and closure step to include `dec` clauses (item's `dec` + each reached
  spec-fn's `dec`), or normatively restrict `dec` measures to specCall-free
  and gate that — the doc currently does neither while claiming the class
  fails "on BOTH paths".

  Tracking: see the crosslink issue cited in the doc-audit report. This file
  is the audit artifact; like PinIntBottom.lean / PinStabilization.lean /
  PinBodyRegistry.lean it must keep compiling.
-/
import Thermite.Denote

namespace Thermite.PinDecMeasure

/-- §4's stabilization relation, verbatim from the doc. -/
def stabilizes (e : Expr) (env : Env) (v : Int) : Prop :=
  ∃ N, ∀ fuel, fuel ≥ N → intVal fuel e env = v

/-- The dec measure `x - t(x)` — a spec-call in MEASURE position. -/
def decMeasure : Expr :=
  Expr.arith ArithOp.sub (Expr.var "x") (Expr.specCall "t" [Expr.var "x"])

/-- The #224-closure registry: `t` is called from NO `req`/`ens`/body
    position (only from the `dec` clause), so `t ∉ calledSpecFns(item)` and
    the exported `R_item` ("exactly calledSpecFns(item)") lacks it. -/
def Rclosure : Registry := fun _ => none

/-- The TRUE source registry: `t(x) = x`. -/
def Rtrue : Registry := fun n =>
  if n = "t" then some ⟨["x"], Expr.var "x"⟩ else none

/-- The measure's env at recursion argument `x`, under the closure registry. -/
def envC (x : Int) : Env :=
  { ints := fun n => if n = "x" then x else 0
    seqs := fun _ => []
    optres := fun _ => OptResVal.none_
    specs := Rclosure }

/-- The same valuation under the TRUE registry. -/
def envT (x : Int) : Env :=
  { ints := fun n => if n = "x" then x else 0
    seqs := fun _ => []
    optres := fun _ => OptResVal.none_
    specs := Rtrue }

/-- Against the closure `R_item` the measure denotes `x - 0 = x` at EVERY
    fuel: the dec-position spec-call is unresolved and bottoms stably. -/
theorem closure_measure_denotes_x :
    ∀ fuel x, intVal fuel decMeasure (envC x) = x := by
  intro fuel x
  cases fuel with
  | zero => simp [decMeasure, intVal, arithDenote, envC]
  | succ n => simp [decMeasure, intVal, arithDenote, envC, Rclosure]

/-- Against the TRUE registry the measure denotes `x - x = 0` at every
    resolving fuel: the REAL measure is the constant 0. -/
theorem true_measure_denotes_zero :
    ∀ fuel x, intVal (fuel + 1) decMeasure (envT x) = 0 := by
  intro fuel x
  simp [decMeasure, intVal, intValArgs, arithDenote, envT, Rtrue,
        Env.bindParams, Env.bindInt]

/-- So the closure-denoted measure STABILIZES to `x` … -/
theorem closure_measure_stabilizes_to_x (x : Int) :
    stabilizes decMeasure (envC x) x :=
  ⟨0, fun fuel _ => closure_measure_denotes_x fuel x⟩

/-- … and uniqueness (the doc's own #214 lever) forces that value. -/
theorem closure_measure_forces (x : Int) :
    ∀ v, stabilizes decMeasure (envC x) v → v = x := by
  rintro v ⟨N, h⟩
  have := h N (Nat.le_refl N)
  rw [closure_measure_denotes_x N x] at this
  exact this.symm

/-- The TRUE measure stabilizes to the constant 0 … -/
theorem true_measure_stabilizes_to_zero (x : Int) :
    stabilizes decMeasure (envT x) 0 := by
  refine ⟨1, ?_⟩
  intro fuel h
  obtain ⟨k, rfl⟩ : ∃ k, fuel = k + 1 := ⟨fuel - 1, by omega⟩
  exact true_measure_denotes_zero k x

/-- … and uniqueness forces 0. -/
theorem true_measure_forces (x : Int) :
    ∀ v, stabilizes decMeasure (envT x) v → v = 0 := by
  rintro v ⟨N, h⟩
  obtain ⟨M, h0⟩ := true_measure_stabilizes_to_zero x
  have h1 := h (max N M) (Nat.le_max_left N M)
  have h2 := h0 (max N M) (Nat.le_max_right N M)
  omega

/-- THE PIN, direction 1: the `decreasing_by`-shaped descent check, stated
    over the closure-populated `R_item` (REQ-1.2(b)'s "obligation over the
    encoded `R_item`"), AFFIRMS strict descent on the recursive call
    `x → x - 1` — the poisoned measure looks VALID. -/
theorem closure_measure_strictly_descends (x : Int) :
    ∀ vc vp,
      stabilizes decMeasure (envC x) vc →
      stabilizes decMeasure (envC (x - 1)) vp →
      vp < vc := by
  intro vc vp hc hp
  have h1 := closure_measure_forces x vc hc
  have h2 := closure_measure_forces (x - 1) vp hp
  omega

/-- THE PIN, direction 2: the SOURCE measure NEVER strictly descends
    (constant 0 → 0) — it is exactly the non-well-founded measure of a
    divergent spec-fn, the case REGISTRY-TERMINATION exists to REJECT. So
    "dec-validity as denoted through the #224 closure" diverges from source
    dec-validity, and the §1.2/§3.1 claim that a divergent spec-fn "FAILS
    this class on BOTH paths" is unjustified for spec-call-bearing measures. -/
theorem true_measure_never_descends (x : Int) :
    ∀ vc vp,
      stabilizes decMeasure (envT x) vc →
      stabilizes decMeasure (envT (x - 1)) vp →
      ¬ vp < vc := by
  intro vc vp hc hp
  have h1 := true_measure_forces x vc hc
  have h2 := true_measure_forces (x - 1) vp hp
  omega

end Thermite.PinDecMeasure
