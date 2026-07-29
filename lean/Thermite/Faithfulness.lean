/-
  Thermite/Faithfulness.lean — the (T2) translation-validation meta-theorem capstone
  (increments (d) #174 + 3b #183; epic #169). The existential → universal conversion.

  This module is the CompCert-class keystone of the lowering-soundness arc. It composes
  the three already-proven (T1) validator-soundness theorems with the per-run
  translation-validation (TV) result and discharges the (T2) universal semantic-
  preservation guarantee `∀ P passing TV, ⟦lower(P)⟧ = ⟦P⟧_S`, relative to the named
  trust base `{Z3 soundness, S = the intended meaning, the Lean kernel}`. It is the
  verified-validator architecture's conclusion (Leroy/CompCert, finding #1: a verified
  validator ∘ an unverified compiler gives a verified-compiler-strength guarantee).

  Governing design: `.design/verified/thermite-semantics.md`
    - REQ-3 (T2, semantic preservation, stated as a forward simulation; the relation
      `∼`, observable = the caged contract fragment + the `fx` rows; finding #2),
    - REQ-7 (the verified-validator architecture, Leroy finding #1 / Necula finding #4:
      `R` plus the per-run Z3 check is the validator; (T1) is what makes it verified),
    - the soundness-theorem section "(T2) — semantic preservation": the one-line
      modus-ponens proof `⟦lower(P)⟧ = ⟦R(P)⟧ = ⟦P⟧_S` and its `{Z3, S}` relativity,
    - AC-3 (the composition is `h_tv.trans (T1)`), AC-4 (the relativity plus the reduced-
      trusted-base enumeration, Leroy finding #3).

  ─────────────────────────────────────────────────────────────────────────────────────
  The trust boundary: `h_tv` is Z3-discharged, not Lean-proven.
  ─────────────────────────────────────────────────────────────────────────────────────
  The per-run TV does the following. For the program `P` under check, Z3 proves
  `lower(P) ⟺ ref(P)` (the production-emitted Verus predicate is logically equivalent
  to the reference encoder's output). By Verus's logic soundness, that Z3 result
  gives `⟦lower(P)⟧ = ⟦ref(P)⟧` (denotational equality of the two Verus forms). In this
  module that per-run Z3 attestation is modelled as an explicit hypothesis

      `h_tv : ⟦lower(P)⟧ = ⟦ref(P)⟧`   (the contract layer's `↔` form, or the `=` form).

  This is the trust boundary. `h_tv` is the external oracle's premise; Z3 discharges it
  per run. Lean does not prove `h_tv`. The capstone theorems take it as a hypothesis and
  compose it with the kernel-proven (T1). Increment 4a (#184) is the route to demote Z3:
  reconstruct the Z3/cvc5 proof into a kernel-checked Lean proof (Lean-SMT, finding #8).
  Until then `h_tv` is the Z3-trusted premise, and (T2) is relative to Z3 soundness.

  To make `h_tv` a load-bearing premise rather than `True` or a trivially satisfiable
  one, `⟦lower(P)⟧` is modelled as an arbitrary value `lowered` of the denotation type (a
  universally-quantified parameter standing for the Z3-attested meaning of whatever the
  unverified production lowerer emitted). For an arbitrary `lowered`, `h_tv` is false
  unless Z3 in fact attested it, so the theorem consumes the TV result; it does not invent
  faithfulness for a `lowered` that disagrees with the reference. The `∀ P` (and
  `∀ lowered`) quantification is therefore load-bearing: the conclusion is the faithfulness
  equality for every program, with the only per-`P` input being the Z3-supplied `h_tv`.
  That is the existential → universal conversion: the claim is not "there exists a faithful
  `P`" but "every `P` passing TV is faithful".

  ─────────────────────────────────────────────────────────────────────────────────────
  The (T1) theorems composed (all shipped and critic-clean; this module does not touch them).
  ─────────────────────────────────────────────────────────────────────────────────────
    - `Thermite.ref_sound_eq` (S_C, `Thermite/Soundness.lean`):
        `refDenote fuel e env = denote fuel e env` — the reference contract encoder is
        denotation-faithful over all 8 contract construct classes.
    - `Thermite.Exec.exec_ref_sound` (S_E, `Thermite/Exec.lean`):
        `execRefValue e env = execDenote e env` — the exec-expression encoder is faithful
        (bounded value, overflow-as-obligation, never nat-coerced).
    - `Thermite.Exec.body_ref_sound` (S_B, `Thermite/Exec/Stmt.lean`):
        `bodyRefState b st = bodyDenote b st` — the straight-line exec-body encoder is
        faithful (loops #163 are out of scope, kernel-gated).

  ─────────────────────────────────────────────────────────────────────────────────────
  The full trust base and the named residuals (enumerated below the theorems, REQ-3 AC-4;
  reduced-trusted-base framing, Leroy finding #3). `#print axioms lowering_faithful`
  shows the Lean-side trust (propext/Quot.sound/Classical.choice — standard); the trusted
  components (Z3, S = intended meaning, Verus VC-gen, rustc/LLVM) and the residuals
  (loops #163, Z3-demotion #184, the Rust↔Lean correspondence #185) are documented, not
  hidden and not machine-closed.
-/
import Thermite.Soundness
import Thermite.Exec
import Thermite.Exec.Stmt

namespace Thermite

/-! ## The TV-hypothesis abstraction (`h_tv`): the Z3-discharged premise per layer

  `TvHyp` records, per encoder layer, that the per-run Z3 check attested
  `⟦lower(P)⟧ = ⟦ref(P)⟧`. The left side `lowered` is the Z3-attested meaning of the
  production lowerer's output (modelled as an arbitrary denotation value; the unverified
  lowerer is a black box, and only its Z3-checked meaning enters here). The right side is
  the reference encoder's meaning under `S`. The field is the trust boundary: it is
  supplied by Z3, not proved in Lean. (#184 demotes the supplier to a kernel-checked
  proof.) -/

/-- A whole-function TV witness: the per-run Z3 attestations for a function, namely a
    contract clause (`S_C`) plus an exec body (`S_B`, which itself composes `S_E`). Both
    fields are Z3-discharged premises; bundling them is the whole-program composition
    (`lowering_faithful` consumes a `FnTvWitness`). Each field is a genuine equality the
    production lowering must satisfy, attested per run; it is not `True`. -/
structure FnTvWitness where
  /-- The fuel at which the contract clause is denoted (shared by source + encoder; T1 is
      fuel-uniform, so any fuel works). -/
  fuel : Nat
  /-- The function's `req`/`ens` contract clause (a contract `Expr`, denoted under `S_C`). -/
  contract : Expr
  /-- The environment the contract clause is denoted in. -/
  contractEnv : Env
  /-- The Z3-attested meaning of the production lowering of `contract` (an arbitrary `Prop`:
      the unverified lowerer's output, known only through its Z3-checked meaning). -/
  loweredContract : Prop
  /-- `h_tv` for the contract layer: Z3 attested `⟦lower(contract)⟧ = ⟦ref(contract)⟧`.
      The trust boundary; supplied by Z3, not proved in Lean. -/
  h_tv_contract : loweredContract = refDenote fuel contract contractEnv
  /-- The function's straight-line exec body (a `Thermite.Exec.Block`, denoted under `S_B`). -/
  body : Thermite.Exec.Block
  /-- The state the body is denoted in. -/
  bodyState : Thermite.Exec.State
  /-- The Z3-attested meaning of the production lowering of `body` (an arbitrary
      `Option ExecVal`: the unverified lowerer's output, known only through Z3). -/
  loweredBody : Option Thermite.Exec.ExecVal
  /-- `h_tv` for the body layer: Z3 attested `⟦lower(body)⟧ = ⟦ref(body)⟧`.
      The trust boundary; supplied by Z3, not proved in Lean. -/
  h_tv_body : loweredBody = Thermite.Exec.bodyRefState body bodyState

/-! ## The per-layer (T2) meta-theorems: `h_tv ∧ T1 ⟹ ⟦lower(P)⟧ = ⟦P⟧_S`

  Each is the one-line modus-ponens (AC-3): `h_tv.trans (T1 P)`. The statement is the
  deliverable: the universal `∀ P` faithfulness relative to `{Z3, S}`. The proof is short
  because (T1) is already kernel-checked; the content is the composition direction, that
  the Z3-attested `lowered` equals the source meaning under `S`, given only the per-run
  `h_tv`. -/

/--
  (T2) — the contract-layer translation-validation meta-theorem (`S_C`).

  For every contract clause `e` (and every `fuel`/`env`), and for every `lowered : Prop`
  standing for the Z3-attested meaning of the production lowering of `e`: if the per-run
  TV attested `lowered = ⟦ref(e)⟧` (`h_tv`, the Z3-discharged premise), then the lowering
  is faithful to the source meaning under `S_C`: `lowered = ⟦e⟧_{S_C}`.

  Proof: `h_tv.trans (ref_sound_eq …)`, the per-run TV `⟦lower⟧ = ⟦ref⟧` composed with
  the kernel-proven (T1) `⟦ref⟧ = ⟦e⟧_S`. The `∀ e` (and `∀ lowered`) is load-bearing, and
  `h_tv` is the Z3-discharged premise (for an arbitrary `lowered` it is false unless Z3
  attested it, so the conclusion is not a tautology). This is the contract-side existential
  → universal conversion: every contract clause passing TV is faithfully lowered, relative
  to `{Z3, S_C}`. -/
theorem tv_meta_contract
    (fuel : Nat) (e : Expr) (env : Env) (lowered : Prop)
    (h_tv : lowered = refDenote fuel e env) :
    lowered = denote fuel e env :=
  h_tv.trans (ref_sound_eq fuel e env)

/--
  (T2) — the exec-expression-layer translation-validation meta-theorem (`S_E`).

  For every pure exec expression `e` (and every `env`), and for every
  `lowered : Option ExecVal` standing for the Z3-attested meaning of the production
  lowering of `e`: if the per-run TV attested `lowered = ⟦ref(e)⟧` (`h_tv`), then the
  lowering is faithful to the source bounded-value meaning under `S_E`: `lowered = ⟦e⟧_{S_E}`.

  Proof: `h_tv.trans (exec_ref_sound …)`. The bounded / overflow-as-obligation / never-
  nat-coerced content of `S_E` is carried through `exec_ref_sound` (the kernel-proven T1);
  here it is composed with the per-run TV. `h_tv` is the Z3-discharged premise. -/
theorem tv_meta_exec
    (e : Thermite.Exec.ExecExpr) (env : Thermite.Exec.ExecEnv)
    (lowered : Option Thermite.Exec.ExecVal)
    (h_tv : lowered = Thermite.Exec.execRefValue e env) :
    lowered = Thermite.Exec.execDenote e env :=
  h_tv.trans (Thermite.Exec.exec_ref_sound e env)

/--
  (T2) — the exec-body-layer translation-validation meta-theorem (`S_B`, straight-line).

  For every straight-line exec body `b` (and every state `st`), and for every
  `lowered : Option ExecVal` standing for the Z3-attested meaning of the production
  lowering of `b`: if the per-run TV attested `lowered = ⟦ref(b)⟧` (`h_tv`), then the
  lowering is faithful to the source state-transformer meaning under `S_B`:
  `lowered = ⟦b⟧_{S_B}`.

  Proof: `h_tv.trans (body_ref_sound …)`. The big-step state-transformer content (the
  threading / scalar-mutation rebind / branch composition / tail projection, with the
  obligation-`none` propagating) is carried through `body_ref_sound` (the kernel-proven
  T1). Loops are out of scope (#163, kernel-gated); `b` ranges over the straight-line
  `Block` fragment exactly as `body_ref_sound` does. `h_tv` is the Z3-discharged premise. -/
theorem tv_meta_body
    (b : Thermite.Exec.Block) (st : Thermite.Exec.State)
    (lowered : Option Thermite.Exec.ExecVal)
    (h_tv : lowered = Thermite.Exec.bodyRefState b st) :
    lowered = Thermite.Exec.bodyDenote b st :=
  h_tv.trans (Thermite.Exec.body_ref_sound b st)

/-! ## The composed whole-program (T2) meta-theorem: `lowering_faithful`

  A Thermite function is a contract (`S_C`) plus a straight-line body (`S_B`/`S_E`). Given
  the per-encoder TV witnesses (the Z3-discharged `h_tv` for each layer, bundled in a
  `FnTvWitness`) and the composed (T1), the whole lowering of the function is faithful: its
  lowered contract equals its source contract under `S_C`, and its lowered body equals its
  source body under `S_B`. This is the verified-validator architecture's conclusion for the
  whole straight-line frozen subset. -/

/--
  (T2) — `lowering_faithful`: the composed whole-program semantic-preservation capstone.

  For every Thermite function (a contract clause plus a straight-line exec body) and every
  per-run TV witness `w` (the bundled Z3-discharged `h_tv` for the contract layer and the
  body layer): the whole lowering is faithful to the source meaning under `S = S_C ⊔ S_B`:

      `w.loweredContract = ⟦w.contract⟧_{S_C}`   ∧   `w.loweredBody = ⟦w.body⟧_{S_B}`.

  That is, `S ≈ C` for this function: the forward-simulation relation `∼` holds between the
  Thermite state and the emitted Verus-Rust target state, preserving the observable effects
  (the caged contract fragment + the `fx` rows; finding #2). The denotational equality this
  theorem establishes is the relation `∼`.

  Proof: the conjunction of `tv_meta_contract` (the contract layer) and `tv_meta_body` (the
  body layer), each the `h_tv.trans (T1)` composition. The `∀ w` quantification is load-
  bearing: the theorem holds for any function, with the only per-function input being the
  Z3-supplied TV witness `w`. This is the existential → universal conversion: the claim is
  not "there exists a faithfully-lowered function" but "every function passing TV is
  faithfully lowered", relative to `{Z3 soundness, S = the intended meaning, the Lean
  kernel}`. -/
theorem lowering_faithful (w : FnTvWitness) :
    w.loweredContract = denote w.fuel w.contract w.contractEnv
    ∧ w.loweredBody = Thermite.Exec.bodyDenote w.body w.bodyState :=
  ⟨tv_meta_contract w.fuel w.contract w.contractEnv w.loweredContract w.h_tv_contract,
   tv_meta_body w.body w.bodyState w.loweredBody w.h_tv_body⟩

/-! ## Non-vacuity of the capstone: `h_tv` is a required premise, the `∀` ranges freely

  These witnesses show that `lowering_faithful` (and its per-layer pieces) is not a
  trivial tautology. The conclusion is the faithfulness equality, and `h_tv` is a premise
  the conclusion consumes: a `lowered` that disagrees with the reference does not get
  certified. The theorem refuses it, because `h_tv` would be false. -/

/-- `h_tv` is not trivially satisfiable: there is a `lowered` (here `False`) and a contract
    clause whose Z3-attestation `h_tv` is false, so `tv_meta_contract` cannot be invoked
    for it. This pins `h_tv` as a load-bearing premise (a vacuous/`True` premise would always
    hold). The witness clause `a == b` at `envAB` (`a:=1, b:=2`) has source meaning the false
    `1 = 2`; the encoder meaning is likewise `refDenote … = (1 = 2)`. Taking the bogus
    `lowered := (2 = 2)` (true: a hypothetical lowerer that emitted a faithful-looking but
    semantically wrong predicate), `h_tv : (2 = 2) = (refDenote …)` is false, since a true
    `Prop` is not equal to the false encoder meaning. So no `h_tv` is available and the
    theorem does not fire, showing it depends on the Z3 attestation. -/
theorem h_tv_is_genuine_premise :
    ¬ ((2 = 2) = refDenote 0 (Expr.cmp CmpOp.eq (Expr.var "a") (Expr.var "b")) envAB) := by
  -- refDenote of `a == b` at envAB is the false proposition `1 = 2`; `(2 = 2)` is true.
  -- A true Prop equal to a false Prop is absurd.
  intro h
  have hRef : ¬ refDenote 0 (Expr.cmp CmpOp.eq (Expr.var "a") (Expr.var "b")) envAB := by
    simp [refDenote, encOp, tokRel, refIntVal, envAB]
  rw [← h] at hRef
  exact hRef rfl

/-- The faithful positive direction (non-vacuity, the other side): when the production
    lowering is faithful, i.e. Z3 attested `lowered = ⟦ref(e)⟧`, the meta-theorem
    concludes faithfulness to `S`. Here a lowerer whose meaning is the reference meaning of
    `a == b` (`lowered := refDenote …`) satisfies `h_tv` by `rfl`, and `tv_meta_contract`
    then derives `lowered = ⟦a == b⟧_{S_C}` (the source `1 = 2`). This confirms the theorem
    fires on a TV pass; it is not vacuously unusable. -/
theorem tv_meta_contract_fires_on_faithful_lowering :
    refDenote 0 (Expr.cmp CmpOp.eq (Expr.var "a") (Expr.var "b")) envAB
      = denote 0 (Expr.cmp CmpOp.eq (Expr.var "a") (Expr.var "b")) envAB :=
  tv_meta_contract 0 (Expr.cmp CmpOp.eq (Expr.var "a") (Expr.var "b")) envAB
    (refDenote 0 (Expr.cmp CmpOp.eq (Expr.var "a") (Expr.var "b")) envAB) rfl

end Thermite
