/-
  Thermite/Stabilize.lean — the STABILIZATION SPINE PREREQUISITE for the Lean
  obligation exporter (increment (ii), the #213/#214/#216 fixes; crosslink #240,
  ref #203).

  Governing design: `.design/verified/proof-backends.md` §4 ("the stabilized form",
  the increment-(ii) build-blocker note) + §6.1 (the three-tier normalization story).
  The §4 exported obligation is stated against a STABILIZATION RELATION over the
  fuel-indexed `Denote.lean` denotation — NOT a raw fuel index — and the result value
  is bound THROUGH that relation. This module lands the named Lean additions §4 says
  increment (ii) MUST have in the spine BEFORE the exporter can target the form:

    - `stabilizes` / `stabilizesProp` — the per-env ∃N∀fuel≥N relations (§4 "the form:
      stabilization"), matching `Denote.lean`'s `intVal`/`denote` signatures/universes.
    - `stabilizes_unique` — the uniqueness-of-stabilization fact (the #214 lever: binding
      the result THROUGH the relation pins ONE value, by the overlap-at-max argument).
    - `specCallFree` — a predicate over the FULL mutual AST (Expr/Pred/MatchArm/RangeArg,
      traversing closure bodies, match arms, range bounds) marking the spec-call-free
      fragment (the §6.1(a) auto-tier key, the #216 predicate).
    - `intVal_fuel_irrelevant` / `denote_fuel_irrelevant` — the FUEL-IRRELEVANCE lemma:
      a spec-call-free `e` denotes identically at every fuel (ONLY the two `fuel+1,
      specCall` arms consume fuel; the cycle-7 critic verified the lemma's truth). Proved
      by the mutual well-founded recursion with `env` generalized. The corollaries
      `stabilizesProp e env ↔ denote 0 e env` and `stabilizes e env v ↔ intVal 0 e env = v`
      for spec-call-free `e` are the tier-(a) FUEL-FREE export keys (§4 "the normalization
      story").
    - the BOTTOM-DISTINGUISHING none-propagating denotation `intValNB`/`denoteNB` (+ the
      `seqVal`/`args`/`arms`/`countWhere` companions) — the #241 root fix. It mirrors
      `Denote.lean`'s recursion EXACTLY save: a fuel-0 `specCall` → `none`; an unresolved
      `specCall` → `none`; every arm PROPAGATES `none`. `intValNB f e env = some v` means
      `e` reached a GENUINE value WITHOUT bottoming — what the bottoming `intVal` cannot
      witness. `Converges e env v := ∃N∀fuel≥N, intValNB fuel e env = some v`.
    - THE AGREEMENT LEMMA `converges_imp_stabilizes` (via `intValNB_agrees`/`denoteNB_agrees`):
      `Converges e env v → stabilizes e env v` — where NB is `some v`, no bottom arm was
      taken, so the bottoming denotation runs identically and stabilizes to the SAME `v`.
    - `RegistryTerminating env e := ∃ v, Converges e env v` (the #241 fix — NO LONGER the
      identity hypothesis `∃ v, stabilizes` that a divergent registry satisfied at the
      Int-bottom 0) and `stabilization_exists` (the design's
      `stabilization_exists_for_dec_bounded`) carrying the agreement lemma's GENUINE
      content. The divergent registry `f(x)=f(x)` now FAILS the hypothesis
      (`PinRegistryTerminating.lean`'s `divergent_registry_fails_the_hypothesis`:
      `intValNB` is `none` at every fuel, so no `Converges` witness); spec-call-free exprs
      Converge unconditionally (`converges_specCallFree`).

  This module does NOT redefine the spine semantics — `intValNB`/`denoteNB` are a SECOND
  (auxiliary) bottom-distinguishing denotation stated OVER the already-kernel-proven
  `Denote.lean` spine, and the agreement lemmas tie it back to that spine. The four other
  critic pins (PinIntBottom / PinStabilization / PinBodyRegistry / PinDecMeasure) keep
  their OWN local copies of `stabilizes`/`stabilizesProp` in their own namespaces; they are
  NOT touched and stay green (they import `Thermite.Denote`, not this module).
-/
import Thermite.Denote

namespace Thermite

/-! ## The stabilization relations (§4 "the form: stabilization")

`stabilizes e env v` says: there is a per-env threshold `N` beyond which `intVal`
has stopped changing and equals `v`. `stabilizesProp e env` says: there is a per-env
`N` beyond which `denote` is `True`. The threshold `N` is PER-ENV — NOT a global
`fuel₀` — which is exactly what fixes the value-dependent-depth counterexample of #213
(an env with a large unfolding depth simply has a large `N`). The relations match
`Denote.lean`'s `intVal : Nat → Expr → Env → Int` / `denote : Nat → Expr → Env → Prop`
signatures and universes verbatim; the four pins carry the SAME shape. -/

/-- The INT-position stabilized value (§4): `intVal` reaches `v` and stays there beyond
    some per-env `N`. Determines `v` uniquely (`stabilizes_unique`). -/
def stabilizes (e : Expr) (env : Env) (v : Int) : Prop :=
  ∃ N, ∀ fuel, fuel ≥ N → intVal fuel e env = v

/-- The Prop-position analogue (§4): `denote` stabilizes to `True` beyond some per-env
    `N`. The exported obligation's `reqStable`/`ensStable` are this relation on the
    clauses. -/
def stabilizesProp (e : Expr) (env : Env) : Prop :=
  ∃ N, ∀ fuel, fuel ≥ N → denote fuel e env

/-! ## Uniqueness of stabilization (the #214 lever)

Binding the result THROUGH the relation (rather than at a concrete export-time value)
is well-defined precisely because `stabilizes e env v` pins ONE `v`. The argument is
the overlap-at-max: at any `fuel ≥ max N₁ N₂` both thresholds are cleared, so
`v₁ = intVal fuel e env = v₂`. -/

/-- Uniqueness of INT-stabilization (the #214 lever): the stabilized value is unique, so
    `∀ r, stabilizes body env r → …` binds `r` to the body's ONE true stabilized value
    with no export-time computation. -/
theorem stabilizes_unique {e : Expr} {env : Env} {v₁ v₂ : Int}
    (h₁ : stabilizes e env v₁) (h₂ : stabilizes e env v₂) : v₁ = v₂ := by
  obtain ⟨N₁, h₁⟩ := h₁
  obtain ⟨N₂, h₂⟩ := h₂
  have e₁ := h₁ (max N₁ N₂) (Nat.le_max_left N₁ N₂)
  have e₂ := h₂ (max N₁ N₂) (Nat.le_max_right N₁ N₂)
  rw [e₁] at e₂
  exact e₂

/-! ## `specCallFree` — the spec-call-free fragment (§6.1(a), the #216 key)

`specCallFree e` means `e` (traversed over the FULL mutual AST — closure bodies, match
arms, range bounds) contains NO `specCall`. It is a structurally-recursive Bool predicate
over `Expr`/`Pred`/`MatchArm`/`RangeArg` (the exporter computes it — DECIDABLE by
construction), so it traverses EXACTLY the positions `intVal`/`denote`/`seqVal`/
`denoteArms` denote against. ONLY the two `fuel+1, specCall` arms of `intVal`/`denote`
consume fuel, so for a `specCallFree` `e` the denotation does NOT depend on fuel
(`*_fuel_irrelevant` below). -/

mutual
/-- `e : Expr` contains no `specCall` (over the full AST). `false` exactly at a `specCall`
    node and propagated through every sub-position. -/
def specCallFree : Expr → Bool
  | Expr.intLit _ => true
  | Expr.boolLit _ => true
  | Expr.var _ => true
  -- THE #253 BOOL-VAR: a free name, no subterms — spec-call-free (`true`).
  | Expr.boolVar _ => true
  | Expr.seqVar _ => true
  | Expr.strVar _ => true
  | Expr.optResVar _ => true
  | Expr.cmp _ a b => specCallFree a && specCallFree b
  | Expr.logic _ a b => specCallFree a && specCallFree b
  | Expr.neg a => specCallFree a
  | Expr.arith _ a b => specCallFree a && specCallFree b
  | Expr.cast inner _ => specCallFree inner
  | Expr.idx base index => specCallFree base && specCallFree index
  | Expr.subrange base range => specCallFree base && rangeFree range
  | Expr.seqLen base => specCallFree base
  | Expr.byteAt base index => specCallFree base && specCallFree index
  | Expr.comb _ seq seq2 idx pred =>
      specCallFree seq && optExprFree seq2 && optExprFree idx && optPredFree pred
  | Expr.match_ scrut arms => specCallFree scrut && armsFree arms
  | Expr.is_ scrut _ => specCallFree scrut
  -- THE ONLY `false` node: a spec-call is, by definition, NOT spec-call-free.
  | Expr.specCall _ _ => false
  termination_by e => sizeOf e

/-- A `Pred` closure body is spec-call-free. -/
def predFree : Pred → Bool
  | Pred.mk _ body => specCallFree body
  termination_by p => sizeOf p

/-- A `RangeArg`'s integer bounds are spec-call-free. -/
def rangeFree : RangeArg → Bool
  | RangeArg.rangeTo hi => specCallFree hi
  | RangeArg.range lo hi => specCallFree lo && specCallFree hi
  | RangeArg.rangeFrom lo => specCallFree lo
  termination_by r => sizeOf r

/-- A `List MatchArm`: every arm body is spec-call-free. -/
def armsFree : List MatchArm → Bool
  | [] => true
  | MatchArm.mk _ _ body :: rest => specCallFree body && armsFree rest
  termination_by arms => sizeOf arms

/-- An `Option Expr` field (`seq2`/`idx`): `none` vacuously, `some e` when `e` is. -/
def optExprFree : Option Expr → Bool
  | none => true
  | some e => specCallFree e
  termination_by o => sizeOf o

/-- An `Option Pred` field (the combinator predicate). -/
def optPredFree : Option Pred → Bool
  | none => true
  | some p => predFree p
  termination_by o => sizeOf o
end

/-- `countWhereVal` is congruent in its predicate (a pointwise-`Iff` of `p`/`q` gives an
    equal count). A local copy of `Soundness.countWhereVal_predCongr` over the `Denote.lean`
    `countWhereVal` — reproduced here so this module does NOT depend on `Soundness.lean`;
    used by the `countWhere` arm of `intVal_fuel_irrelevant` (the per-element predicate at
    fuels `f`/`g` is pointwise-equivalent by `denote_fuel_irrelevant`). -/
theorem countWhereVal_predCongr (p q : Int → Prop) (hpq : ∀ x, p x ↔ q x) :
    ∀ s : List Int, countWhereVal p s = countWhereVal q s
  | [] => rfl
  | x :: xs => by
      rw [countWhereVal_cons, countWhereVal_cons, countWhereVal_predCongr p q hpq xs]
      rw [propext (hpq x)]

/-! ## The FUEL-IRRELEVANCE lemma (§4 "the normalization story", the #216 bridge)

For a `specCallFree` `e`, `intVal`/`denote` do NOT depend on fuel — ONLY the two
`fuel+1, specCall` arms consume fuel, and a spec-call-free `e` reaches neither. Proved by
a MUTUAL well-founded recursion over the five `Denote.lean` denotation functions
(`intVal`/`seqVal`/`intValArgs`/`denote`/`denoteArms`), with `env` and the two fuels `f g`
universally quantified inside (the combinator/arm/closure arms rebind via
`Env.bindInt`/`Env.bindParams`, so the IH must hold at every env). The recursion measure
is `sizeOf e` ALONE — fuel never decrements on the spec-call-free fragment, so the
recursion is purely structural (this is the cycle-7 critic's "only the two specCall arms
consume fuel" observation made into the termination argument). -/

mutual
/-- FUEL-IRRELEVANCE on the INT side: a spec-call-free `e`'s `intVal` is the same at every
    fuel. The §6.1(a) auto-tier key (with the `denote` analogue). -/
theorem intVal_fuel_irrelevant : ∀ (e : Expr) (env : Env) (f g : Nat),
    specCallFree e = true → intVal f e env = intVal g e env
  | Expr.intLit n, env, f, g, _ => by simp only [intVal]
  | Expr.boolLit b, env, f, g, _ => by simp only [intVal]
  | Expr.var x, env, f, g, _ => by simp only [intVal]
  -- THE #253 BOOL-VAR: bottoms to `intVal`'s `0` catch-all at every fuel (no integer meaning).
  | Expr.boolVar x, env, f, g, _ => by simp only [intVal]
  | Expr.seqVar x, env, f, g, _ => by simp only [intVal]
  | Expr.strVar x, env, f, g, _ => by simp only [intVal]
  | Expr.optResVar x, env, f, g, _ => by simp only [intVal]
  | Expr.cmp op a b, env, f, g, _ => by simp only [intVal]
  | Expr.logic op a b, env, f, g, _ => by simp only [intVal]
  | Expr.neg a, env, f, g, _ => by simp only [intVal]
  | Expr.match_ scrut arms, env, f, g, _ => by simp only [intVal]
  | Expr.is_ scrut v, env, f, g, _ => by simp only [intVal]
  | Expr.arith op a b, env, f, g, h => by
      simp only [specCallFree, Bool.and_eq_true] at h
      simp only [intVal,
        intVal_fuel_irrelevant a env f g h.1,
        intVal_fuel_irrelevant b env f g h.2]
  | Expr.cast inner ty, env, f, g, h => by
      simp only [specCallFree] at h
      simp only [intVal, intVal_fuel_irrelevant inner env f g h]
  | Expr.idx base i, env, f, g, h => by
      simp only [specCallFree, Bool.and_eq_true] at h
      simp only [intVal,
        seqVal_fuel_irrelevant base env f g h.1,
        intVal_fuel_irrelevant i env f g h.2]
  | Expr.seqLen base, env, f, g, h => by
      simp only [specCallFree] at h
      simp only [intVal, seqVal_fuel_irrelevant base env f g h]
  | Expr.byteAt base i, env, f, g, h => by
      simp only [specCallFree, Bool.and_eq_true] at h
      simp only [intVal,
        seqVal_fuel_irrelevant base env f g h.1,
        intVal_fuel_irrelevant i env f g h.2]
  | Expr.comb c seq seq2 idx pred, env, f, g, h => by
      -- Only `countWhere` reads `intVal` non-trivially (the others bottom to `0`).
      simp only [specCallFree, Bool.and_eq_true] at h
      have hseq : specCallFree seq = true := h.1.1.1
      have hpred : optPredFree pred = true := h.2
      cases c with
      | countWhere =>
          -- countWhere: slice agrees + the per-element closure-body `denote` agrees.
          simp only [intVal, seqVal_fuel_irrelevant seq env f g hseq]
          cases pred with
          | none => rfl
          | some pr =>
              cases pr with
              | mk bound body =>
                  have hbody : specCallFree body = true := by
                    simp only [optPredFree, predFree] at hpred; exact hpred
                  apply countWhereVal_predCongr
                  intro x
                  exact denote_fuel_irrelevant body (env.bindInt bound x) f g hbody
      -- The 6 bounded combinators + permutationOf are Prop-sorted: their `intVal`
      -- bottoms to the `0` catch-all at every fuel.
      | forallIn => simp only [intVal]
      | existsIn => simp only [intVal]
      | sorted => simp only [intVal]
      | forallBelow => simp only [intVal]
      | forallFrom => simp only [intVal]
      | disjoint => simp only [intVal]
      | permutationOf => simp only [intVal]
  | Expr.subrange base r, env, f, g, _ => by simp only [intVal]
  | Expr.specCall name args, _, _, _, h => by
      simp only [specCallFree] at h
      exact absurd h Bool.false_ne_true
  termination_by e => sizeOf e

/-- FUEL-IRRELEVANCE on the SEQUENCE side: a spec-call-free `e`'s `seqVal` is the same at
    every fuel (needed for `idx`/`seqLen`/`byteAt`/`countWhere`, whose operands are
    sequences). -/
theorem seqVal_fuel_irrelevant : ∀ (e : Expr) (env : Env) (f g : Nat),
    specCallFree e = true → seqVal f e env = seqVal g e env
  | Expr.seqVar x, env, f, g, _ => by simp only [seqVal]
  | Expr.strVar x, env, f, g, _ => by simp only [seqVal]
  | Expr.subrange base r, env, f, g, h => by
      simp only [specCallFree, Bool.and_eq_true] at h
      have hbase : specCallFree base = true := h.1
      have hrange : rangeFree r = true := h.2
      cases r with
      | rangeTo hi =>
          have hhi : specCallFree hi = true := by
            simp only [rangeFree] at hrange; exact hrange
          simp only [seqVal,
            seqVal_fuel_irrelevant base env f g hbase,
            intVal_fuel_irrelevant hi env f g hhi]
      | range lo hi =>
          have hlh : specCallFree lo = true ∧ specCallFree hi = true := by
            simp only [rangeFree, Bool.and_eq_true] at hrange; exact hrange
          simp only [seqVal,
            seqVal_fuel_irrelevant base env f g hbase,
            intVal_fuel_irrelevant lo env f g hlh.1,
            intVal_fuel_irrelevant hi env f g hlh.2]
      | rangeFrom lo =>
          have hlo : specCallFree lo = true := by
            simp only [rangeFree] at hrange; exact hrange
          simp only [seqVal,
            seqVal_fuel_irrelevant base env f g hbase,
            intVal_fuel_irrelevant lo env f g hlo]
  -- Every non-sequence `Expr` denotes `[]` on the `seqVal` side at every fuel.
  | Expr.intLit n, _, f, g, _ => by simp only [seqVal]
  | Expr.boolLit b, _, f, g, _ => by simp only [seqVal]
  | Expr.var x, _, f, g, _ => by simp only [seqVal]
  -- THE #253 BOOL-VAR: denotes `[]` on the seq side at every fuel.
  | Expr.boolVar x, _, f, g, _ => by simp only [seqVal]
  | Expr.optResVar x, _, f, g, _ => by simp only [seqVal]
  | Expr.cmp op a b, _, f, g, _ => by simp only [seqVal]
  | Expr.logic op a b, _, f, g, _ => by simp only [seqVal]
  | Expr.neg a, _, f, g, _ => by simp only [seqVal]
  | Expr.arith op a b, _, f, g, _ => by simp only [seqVal]
  | Expr.cast inner ty, _, f, g, _ => by simp only [seqVal]
  | Expr.idx base i, _, f, g, _ => by simp only [seqVal]
  | Expr.seqLen base, _, f, g, _ => by simp only [seqVal]
  | Expr.byteAt base i, _, f, g, _ => by simp only [seqVal]
  | Expr.comb c seq seq2 idx pred, _, f, g, _ => by simp only [seqVal]
  | Expr.match_ scrut arms, _, f, g, _ => by simp only [seqVal]
  | Expr.is_ scrut v, _, f, g, _ => by simp only [seqVal]
  | Expr.specCall name args, _, _, _, h => by
      simp only [specCallFree] at h
      exact absurd h Bool.false_ne_true
  termination_by e => sizeOf e

/-- FUEL-IRRELEVANCE on the Prop side: a spec-call-free `e`'s `denote` is logically the
    same at every fuel. The §6.1(a) auto-tier key for `req`/`ens` clauses. -/
theorem denote_fuel_irrelevant : ∀ (e : Expr) (env : Env) (f g : Nat),
    specCallFree e = true → (denote f e env ↔ denote g e env)
  | Expr.boolLit b, env, f, g, _ => by simp only [denote]
  | Expr.is_ scrut v, env, f, g, _ => by simp only [denote]
  | Expr.cmp op a b, env, f, g, h => by
      simp only [specCallFree, Bool.and_eq_true] at h
      cases op <;>
        simp only [denote,
          intVal_fuel_irrelevant a env f g h.1,
          intVal_fuel_irrelevant b env f g h.2]
  | Expr.logic op a b, env, f, g, h => by
      simp only [specCallFree, Bool.and_eq_true] at h
      cases op <;>
        simp only [denote,
          denote_fuel_irrelevant a env f g h.1,
          denote_fuel_irrelevant b env f g h.2]
  | Expr.neg a, env, f, g, h => by
      simp only [specCallFree] at h
      simp only [denote, denote_fuel_irrelevant a env f g h]
  | Expr.match_ scrut arms, env, f, g, h => by
      simp only [specCallFree, Bool.and_eq_true] at h
      simp only [denote]
      exact denoteArms_fuel_irrelevant (scrutVal scrut env) arms env f g h.2
  | Expr.comb c seq seq2 idx pred, env, f, g, h => by
      -- Mirrors `Soundness.ref_sound`'s combinator case (the same `denote`-congruence
      -- proof, here between fuels `f`/`g` rather than encoder/source). The slice agrees by
      -- `seqVal_fuel_irrelevant`; the per-element predicate by `denote_fuel_irrelevant` on
      -- the closure body; the optional `idx`/`seq2` are resolved by `cases` IN-branch (as in
      -- `ref_sound`, avoiding `match`-motive pollution).
      -- Resolve `pred` FIRST (eliminating the `pred` variable, so no `pred`-mentioning hyp
      -- survives to pollute the `denote`-equation `match pred` motive); then the per-element
      -- predicate iff `hp` mentions NO `pred`. The optional `idx`/`seq2` are resolved by
      -- `cases` IN-branch (as in `Soundness.ref_sound`'s combinator case).
      simp only [specCallFree, Bool.and_eq_true] at h
      have hseq : specCallFree seq = true := h.1.1.1
      have hseq2 : optExprFree seq2 = true := h.1.1.2
      have hidx : optExprFree idx = true := h.1.2
      have hpred : optPredFree pred = true := h.2
      have hs : seqVal f seq env = seqVal g seq env :=
        seqVal_fuel_irrelevant seq env f g hseq
      -- The per-element predicate iff: `cases pred` is performed at the LEAF (under the
      -- congruence binders), so the goal's `match pred` is resolved to a concrete arm before
      -- the iff is closed — no `match pred` motive is ever built over the in-scope `hpred`.
      cases c with
      | forallIn =>
          simp only [denote, hs]
          refine forall_congr' (fun i => imp_congr_right (fun _ => ?_))
          exact comb_pred_fuel_iff pred env f g hpred (seqIdx (seqVal g seq env) i)
      | existsIn =>
          simp only [denote, hs]
          refine exists_congr (fun i => and_congr_right (fun _ => ?_))
          exact comb_pred_fuel_iff pred env f g hpred (seqIdx (seqVal g seq env) i)
      | sorted =>
          simp only [denote, hs]
      | forallBelow =>
          cases idx with
          | none =>
              simp only [denote, hs]
              refine forall_congr' (fun i => imp_congr_right (fun _ => ?_))
              exact comb_pred_fuel_iff pred env f g hpred (seqIdx (seqVal g seq env) i)
          | some e =>
              have he : specCallFree e = true := by
                simp only [optExprFree] at hidx; exact hidx
              simp only [denote, hs, intVal_fuel_irrelevant e env f g he]
              refine forall_congr' (fun i => imp_congr_right (fun _ => ?_))
              exact comb_pred_fuel_iff pred env f g hpred (seqIdx (seqVal g seq env) i)
      | forallFrom =>
          cases idx with
          | none =>
              simp only [denote, hs]
              refine forall_congr' (fun i => imp_congr_right (fun _ => ?_))
              exact comb_pred_fuel_iff pred env f g hpred (seqIdx (seqVal g seq env) i)
          | some e =>
              have he : specCallFree e = true := by
                simp only [optExprFree] at hidx; exact hidx
              simp only [denote, hs, intVal_fuel_irrelevant e env f g he]
              refine forall_congr' (fun i => imp_congr_right (fun _ => ?_))
              exact comb_pred_fuel_iff pred env f g hpred (seqIdx (seqVal g seq env) i)
      | disjoint =>
          cases seq2 with
          | none => simp only [denote, hs]
          | some e =>
              have he : specCallFree e = true := by
                simp only [optExprFree] at hseq2; exact hseq2
              simp only [denote, hs, seqVal_fuel_irrelevant e env f g he]
      | permutationOf =>
          cases seq2 with
          | none => simp only [denote, hs]
          | some e =>
              have he : specCallFree e = true := by
                simp only [optExprFree] at hseq2; exact hseq2
              simp only [denote, hs, seqVal_fuel_irrelevant e env f g he]
      | countWhere =>
          -- countWhere is value-sorted; its `denote` arm bottoms to `True` at every fuel.
          simp only [denote]
  | Expr.specCall name args, _, _, _, h => by
      simp only [specCallFree] at h
      exact absurd h Bool.false_ne_true
  -- Integer-sorted leaves denote `True` on the Prop side at every fuel.
  | Expr.intLit n, env, f, g, _ => by simp only [denote]
  | Expr.var x, env, f, g, _ => by simp only [denote]
  -- THE #253 BOOL-VAR: denotes `env.bools x = true` (fuel-free) — the iff is reflexive.
  | Expr.boolVar x, env, f, g, _ => by simp only [denote]
  | Expr.seqVar x, env, f, g, _ => by simp only [denote]
  | Expr.strVar x, env, f, g, _ => by simp only [denote]
  | Expr.optResVar x, env, f, g, _ => by simp only [denote]
  | Expr.arith op a b, env, f, g, _ => by simp only [denote]
  | Expr.cast inner ty, env, f, g, _ => by simp only [denote]
  | Expr.idx base i, env, f, g, _ => by simp only [denote]
  | Expr.subrange base r, env, f, g, _ => by simp only [denote]
  | Expr.seqLen base, env, f, g, _ => by simp only [denote]
  | Expr.byteAt base i, env, f, g, _ => by simp only [denote]
  termination_by e => sizeOf e

/-- FUEL-IRRELEVANCE on the match-arm side (the `denote`-mutual helper). -/
theorem denoteArms_fuel_irrelevant : ∀ (scrut : OptResVal) (arms : List MatchArm)
    (env : Env) (f g : Nat),
    armsFree arms = true → (denoteArms f scrut arms env ↔ denoteArms g scrut arms env)
  | scrut, [], env, f, g, _ => by rw [denoteArms.eq_def, denoteArms.eq_def]
  | scrut, MatchArm.mk variant binder body :: rest, env, f, g, h => by
      simp only [armsFree, Bool.and_eq_true] at h
      have hbody : specCallFree body = true := h.1
      have hrest : armsFree rest = true := h.2
      rw [denoteArms.eq_def, denoteArms.eq_def]
      by_cases hv : scrut.variant = variant
      · simp only [hv, if_true]
        cases binder with
        | some x => exact denote_fuel_irrelevant body (env.bindInt x scrut.payload) f g hbody
        | none => exact denote_fuel_irrelevant body env f g hbody
      · simp only [hv, if_false]
        exact denoteArms_fuel_irrelevant scrut rest env f g hrest
  termination_by _ arms _ _ _ => sizeOf arms

/-- The per-element predicate of a combinator is fuel-irrelevant (the `denote`-mutual
    helper that keeps the comb arm free of a `pred`-mentioning hypothesis). A STANDALONE
    helper over `pred` so `intVal_fuel_irrelevant`/`denote_fuel_irrelevant`'s combinator
    cases obtain `hp` WITHOUT a context hyp polluting the `match pred` motive. -/
theorem comb_pred_fuel_iff : ∀ (pred : Option Pred) (env : Env) (f g : Nat),
    optPredFree pred = true →
    ∀ v : Int,
      (match pred with
        | some (Pred.mk bound body) => denote f body (env.bindInt bound v)
        | none => True) ↔
      (match pred with
        | some (Pred.mk bound body) => denote g body (env.bindInt bound v)
        | none => True)
  | none, env, f, g, _, v => Iff.rfl
  | some (Pred.mk bound body), env, f, g, hpred, v => by
      have hbody : specCallFree body = true := by
        simp only [optPredFree, predFree] at hpred; exact hpred
      exact denote_fuel_irrelevant body (env.bindInt bound v) f g hbody
  termination_by pred _ _ _ _ => sizeOf pred
end

/-! ## The FUEL-FREE export keys (§4 "the normalization story", tier (a))

For a spec-call-free `e`, the `∃N∀fuel≥N` relation collapses to the fuel-0 value: the
witness `N = 0` works because the value is CONSTANT in fuel. So the exporter can emit the
FUEL-FREE shallow statement `denote 0 e env` / `intVal 0 e env = v` — exactly the QF shape
the z3-demotion PoC discharges (§6 tier (a)). -/

/-- Tier-(a) key (Prop side): for spec-call-free `e`, `stabilizesProp e env ↔ denote 0 e env`.
    The fuel-free shallow statement the auto battery actually chews. -/
theorem stabilizesProp_iff_denote_zero {e : Expr} {env : Env} (h : specCallFree e = true) :
    stabilizesProp e env ↔ denote 0 e env := by
  constructor
  · rintro ⟨N, hN⟩
    exact (denote_fuel_irrelevant e env N 0 h).mp (hN N (Nat.le_refl N))
  · intro h0
    exact ⟨0, fun fuel _ => (denote_fuel_irrelevant e env 0 fuel h).mp h0⟩

/-- Tier-(a) key (INT side): for spec-call-free `e`, `stabilizes e env v ↔ intVal 0 e env = v`.
    The fuel-free shallow value the auto battery actually chews. -/
theorem stabilizes_iff_intVal_zero {e : Expr} {env : Env} {v : Int}
    (h : specCallFree e = true) : stabilizes e env v ↔ intVal 0 e env = v := by
  constructor
  · rintro ⟨N, hN⟩
    rw [intVal_fuel_irrelevant e env 0 N h]
    exact hN N (Nat.le_refl N)
  · intro h0
    exact ⟨0, fun fuel _ => by rw [intVal_fuel_irrelevant e env fuel 0 h]; exact h0⟩

/-! ## The BOTTOM-DISTINGUISHING none-propagating denotation (the #241 root fix)

THE PROBLEM (the critic's Pin E, `PinRegistryTerminating.lean`). The cycle-4 form defined
`RegistryTerminating env e := ∃ v, stabilizes e env v` and `stabilization_exists` as the
DEFINITIONAL identity over it. That is an IDENTITY HYPOTHESIS, and it is SATISFIED by a
DIVERGENT registry: for `f(x) = f(x)`, `Denote.lean`'s fuel-indexed `intVal` is CONSTANTLY
the Int-bottom `0` (a fuel-0 / unresolved `specCall` bottoms to `0`), so `stabilizes (f x)
env 0` HOLDS — the registry the REGISTRY-TERMINATION class (§1.2) exists to REJECT clears
the hypothesis, and a wrong contract certifies at the bottom-poisoned `0` (Pin E, commit
`f7d288ef`). The bottoming `intVal` CANNOT distinguish "stabilized to a genuine value" from
"stuck at the bottom because it diverged".

THE FIX (a BOTTOM-DISTINGUISHING denotation). We add a SECOND, NONE-PROPAGATING denotation
`intValNB`/`denoteNB` (+ the `seqVal`/`args`/`arms`/`countWhere` companions) that mirrors
`Denote.lean`'s recursion EXACTLY save THREE points: a fuel-0 `specCall` yields `none`; an
unresolved `specCall` yields `none`; and EVERY arm PROPAGATES `none` monadically (an
operand that bottoms poisons the whole node). So `intValNB f e env = some v` means "`e`
reached a GENUINE value at fuel `f` WITHOUT EVER hitting a bottom arm" — exactly what the
bottoming `intVal` cannot witness. `Converges` (below) is "some `v` is reached at all
sufficiently large fuel", and the AGREEMENT LEMMA proves that whenever NB is `some v`, the
bottoming `intVal` agrees (no bottom arm was taken, so the two run identically) — so
`Converges → stabilizes`, giving the §4 supporting lemma GENUINE content. The divergent
registry now FAILS: `intValNB fuel (f x) envD = none` at EVERY fuel, so there is NO
`Converges` witness (`PinRegistryTerminating.lean` re-pins this as the resolved truth). -/

/-- The none-propagating COUNT for the `countWhere` combinator (the #182 arm): mirrors
    `Denote.lean`'s `countWhereVal` but takes a per-element predicate that may BOTTOM
    (`p : Int → Option Prop`) and propagates `none` if ANY element's predicate bottoms.
    Structural recursion on the slice (core, no fuel — exactly like `countWhereVal`). -/
noncomputable def countWhereValNB (p : Int → Option Prop) : List Int → Option Int
  | []      => some 0
  | x :: xs => (p x).bind (fun px =>
      (countWhereValNB p xs).bind (fun rest =>
        some ((@ite _ px (Classical.propDecidable _) (1 : Int) 0) + rest)))

/-- `countWhereValNB`'s head-then-tail step (the defining equation, by `rfl`). -/
theorem countWhereValNB_cons (p : Int → Option Prop) (x : Int) (xs : List Int) :
    countWhereValNB p (x :: xs)
      = (p x).bind (fun px =>
          (countWhereValNB p xs).bind (fun rest =>
            some ((@ite _ px (Classical.propDecidable _) (1 : Int) 0) + rest))) := rfl

/-- The PER-ELEMENT PREDICATE GATE for the six Prop-combinators (the #242 root fix): given a
    per-element predicate `p : Int → Option Prop` that may BOTTOM and the slice, yields
    `some ()` iff EVERY element's predicate evaluates to `some` (any `none` poisons). The
    bottom-distinguishing companion to `denote`'s `p`-position for forall_in/exists_in/
    forall_below/forall_from: a divergent spec-call inside the predicate body is `none` at
    that element, so the gate is `none` and the comb arm does NOT forge a genuine value.
    Carried alongside the (reflexively-correct) spine `denote` proposition — the gate decides
    `some`/`none`, never WHICH proposition is carried (keeping `denoteNB_agrees` reflexive).
    `sorted`/`disjoint`/`permutationOf` carry no predicate, so the gate is `some ()` for them
    (`pred = none`) and only the slice/`seq2`/`idx` subterms gate, as before. Structural
    recursion on the slice (core, no fuel — exactly like `countWhereValNB`). -/
noncomputable def predGateNB (p : Int → Option Prop) : List Int → Option Unit
  | []      => some ()
  | x :: xs => (p x).bind (fun _ => predGateNB p xs)

/-- `predGateNB`'s head-then-tail step (the defining equation, by `rfl`). -/
theorem predGateNB_cons (p : Int → Option Prop) (x : Int) (xs : List Int) :
    predGateNB p (x :: xs) = (p x).bind (fun _ => predGateNB p xs) := rfl

/-- When `predGateNB p s = some ()`, EVERY element's predicate is `some` (the witness the
    agreement lemma needs to discharge the per-element predicate at each slice member). -/
theorem predGateNB_all_some {p : Int → Option Prop} :
    ∀ {s : List Int}, predGateNB p s = some () → ∀ x ∈ s, ∃ px, p x = some px
  | [], _, x, hx => by simp at hx
  | y :: ys, h, x, hx => by
      rw [predGateNB_cons] at h
      cases hpy : p y with
      | none => rw [hpy] at h; simp at h
      | some py =>
          rw [hpy] at h; simp only [Option.bind] at h
          rcases List.mem_cons.mp hx with hxy | hxys
          · subst hxy; exact ⟨py, hpy⟩
          · exact predGateNB_all_some h x hxys

/-- When every element's predicate is `some`, the gate is `some ()` (the converse direction:
    the auto fragment's gate never poisons). PARAMETRIZED by the per-element `some`-ness `hp`
    (NOT a forward reference into the spec-call-free mutual block — `denoteNB_specCallFree`'s
    comb arm supplies `hp`, exactly as `countWhereValNB_eq_of` is supplied). -/
theorem predGateNB_some_of (p : Int → Option Prop) :
    ∀ (s : List Int), (∀ x, ∃ px, p x = some px) → predGateNB p s = some ()
  | [], _ => rfl
  | x :: xs, hp => by
      rw [predGateNB_cons]
      obtain ⟨px, hpx⟩ := hp x
      rw [hpx]; simp only [Option.bind]
      exact predGateNB_some_of p xs hp

mutual
/-- The none-propagating SEQUENCE denotation (the `seqVal` mirror): `some s` iff `e`
    reaches a genuine sequence at fuel `f` with no bottom arm. Threads fuel UNCHANGED to
    the base/bound subterms (mirrors `seqVal`); a non-sequence node is `some []` (matching
    `seqVal`'s `[]` catch-all — a genuine, non-bottomed value). A `specCall` cannot appear
    on this side (`seqVal` has no `specCall` arm), so the `[]` catch-all is the only default
    and it is GENUINE, never a bottom. -/
noncomputable def seqValNB : Nat → Expr → Env → Option (List Int)
  | _,    Expr.seqVar x, env => some (env.seqs x)
  | _,    Expr.strVar x, env => some (env.seqs x)
  | fuel, Expr.subrange base r, env =>
      (seqValNB fuel base env).bind (fun s =>
        match r with
        | RangeArg.rangeTo hi    =>
            (intValNB fuel hi env).bind (fun h => some (seqSub s 0 h))
        | RangeArg.range lo hi   =>
            (intValNB fuel lo env).bind (fun l =>
              (intValNB fuel hi env).bind (fun h => some (seqSub s l h)))
        | RangeArg.rangeFrom lo  =>
            (intValNB fuel lo env).bind (fun l => some (seqSub s l (s.length : Int))))
  | _, _, _ => some []
  termination_by fuel e _ => (fuel, sizeOf e)

/-- The none-propagating INT denotation (the `intVal` mirror, the #241 root). `some v` iff
    `e` reaches a GENUINE integer value at fuel `f` with NO bottom arm taken. THE THREE
    bottom-distinguishing points vs `intVal`: a fuel-0 `specCall` → `none`; an unresolved
    `specCall` → `none`; every operand bind PROPAGATES `none`. The integer-sorted leaves
    (`intLit`/`var`) and the bool-sorted catch-all (`some 0`) match `intVal`'s GENUINE
    values exactly — only the `specCall` bottoms become `none`. -/
noncomputable def intValNB : Nat → Expr → Env → Option Int
  | fuel, Expr.arith op a b,  env =>
      (intValNB fuel a env).bind (fun x =>
        (intValNB fuel b env).bind (fun y => some (arithDenote op x y)))
  | fuel, Expr.cast inner ty, env =>
      (intValNB fuel inner env).bind (fun x => some (castDenote ty x))
  | fuel, Expr.idx base i,    env =>
      (seqValNB fuel base env).bind (fun s =>
        (intValNB fuel i env).bind (fun iv => some (seqIdx s iv)))
  | fuel, Expr.seqLen base,   env =>
      (seqValNB fuel base env).bind (fun s => some (s.length : Int))
  | fuel, Expr.byteAt base i, env =>
      (seqValNB fuel base env).bind (fun s =>
        (intValNB fuel i env).bind (fun iv => some (seqIdx s iv)))
  -- THE bottom-distinguishing `specCall` arm: a RESOLVED call at fuel+1 unfolds the body
  -- at the consumed fuel (none-propagating through args + body); an UNRESOLVED name is
  -- `none` (NOT `intVal`'s `0`).
  | fuel+1, Expr.specCall name args, env =>
      match env.specs name with
      | some fn =>
          (intValArgsNB (fuel+1) args env).bind (fun vs =>
            intValNB fuel fn.body (env.bindParams fn.params vs))
      | none    => none
  -- THE bottom-distinguishing fuel-0 `specCall`: `none` (NOT `intVal`'s `0` catch-all).
  | 0, Expr.specCall _ _, _ => none
  -- The #182 `countWhere` value-combinator: the none-propagating recursive count.
  | fuel, Expr.comb CombName.countWhere seq _ _ pred, env =>
      (seqValNB fuel seq env).bind (fun s =>
        countWhereValNB
          (fun x => match pred with
            | some (Pred.mk bound body) => denoteNB fuel body (env.bindInt bound x)
            | none => some True) s)
  | _,    Expr.intLit n,      _   => some n
  | _,    Expr.var x,         env => some (env.ints x)
  -- The bool-sorted catch-all: GENUINE `some 0` (matches `intVal`'s `0`; the `specCall`
  -- arms above already peeled off BOTH fuels, so no `specCall` reaches here).
  | _, _, _ => some 0
  termination_by fuel e _ => (fuel, sizeOf e)

/-- The none-propagating denoted ARG VALUES (the `intValArgs` mirror): `some vs` iff every
    arg reaches a genuine value (any arg bottoming poisons the list). -/
noncomputable def intValArgsNB : Nat → List Expr → Env → Option (List Int)
  | _,    [],        _   => some []
  | fuel, a :: rest, env =>
      (intValNB fuel a env).bind (fun v =>
        (intValArgsNB fuel rest env).bind (fun vs => some (v :: vs)))
  termination_by fuel args _ => (fuel, sizeOf args)

/-- The none-propagating PROP denotation (the `denote` mirror): `some P` iff `e` reaches a
    GENUINE proposition at fuel `f` with no bottom arm; the carried `P` is exactly the
    `denote` form (the agreement lemma proves `denote f e env ↔ P`). THE bottom-
    distinguishing `specCall` arm: fuel-0 / unresolved → `none`. Every sub-position
    propagates `none`. The 6 bounded combinators + permutationOf MIRROR `denote` exactly
    (the inline per-element predicate `p` is `denote`'s own — agreement is reflexivity),
    propagating `none` only from the slice/`seq2`/`idx` subterms. -/
noncomputable def denoteNB : Nat → Expr → Env → Option Prop
  | _,    Expr.boolLit b, _   => some (b = true)
  | fuel, Expr.cmp op a b, env =>
      (intValNB fuel a env).bind (fun x =>
        (intValNB fuel b env).bind (fun y =>
          some (match op with
            | CmpOp.eq => x = y
            | CmpOp.ne => x ≠ y
            | CmpOp.lt => x < y
            | CmpOp.le => x ≤ y
            | CmpOp.gt => x > y
            | CmpOp.ge => x ≥ y)))
  | fuel, Expr.logic op a b, env =>
      (denoteNB fuel a env).bind (fun pa =>
        (denoteNB fuel b env).bind (fun pb =>
          some (match op with
            | LogOp.and => pa ∧ pb
            | LogOp.or  => pa ∨ pb)))
  | fuel, Expr.neg e, env =>
      (denoteNB fuel e env).bind (fun pe => some (¬ pe))
  -- THE #253 BOOL-VAR: an EXPLICIT arm carrying `denote`'s `env.bools x = true` proposition
  -- (it must NOT fall to the `some True` catch-all, which would break `denoteNB_agrees`'s
  -- carried-proposition agreement at `env.bools x = false`). A free name never bottoms → `some`.
  | _,    Expr.boolVar x, env => some (env.bools x = true)
  | fuel, Expr.match_ scrut arms, env =>
      denoteArmsNB fuel (scrutVal scrut env) arms env
  | _,    Expr.is_ scrut variant, env =>
      some ((scrutVal scrut env).isVariant variant = true)
  -- The comb arm propagates `none` from the slice/`seq2`/`idx` subterms (where a `specCall`
  -- could bottom) AND from the PER-ELEMENT PREDICATE BODY over the slice (the #242 root fix:
  -- a divergent spec-call reachable only through a Prop-combinator predicate body now poisons
  -- this arm, via `predGateNB`). The gate evaluates the predicate body through `denoteNB` at
  -- each slice element (`env.bindInt bound x`) — exactly the positions the spine `denote`'s
  -- `p i = denote fuel body (env.bindInt bound (seqIdx s i))` reads where the index guard
  -- holds; any element bottoming makes the gate `none`. `sorted`/`disjoint`/`permutationOf`
  -- carry `pred = none`, so the gate is vacuously `some ()` (they gate only slice/`seq2`/
  -- `idx`, as before). The carried proposition is still EXACTLY the spine `denote` form —
  -- the gate decides `some`/`none`, never WHICH proposition is carried — so `denoteNB_agrees`
  -- stays reflexive (`P = denote (comb …)`), while a divergent predicate no longer forges a
  -- `some` (`PinCombPredGap.lean`'s F.2 inverts).
  | fuel, Expr.comb c seq seq2 idx pred, env =>
      (seqValNB fuel seq env).bind (fun s =>
        ((match seq2 with | some e => seqValNB fuel e env | none => some []).bind (fun _ =>
          ((match idx with | some e => intValNB fuel e env | none => some 0).bind (fun _ =>
            (predGateNB
              (fun x => match pred with
                | some (Pred.mk bound body) => denoteNB fuel body (env.bindInt bound x)
                | none => some True) s).bind (fun _ =>
              some (denote fuel (Expr.comb c seq seq2 idx pred) env)))))))
  | fuel+1, Expr.specCall name args, env =>
      match env.specs name with
      | some fn =>
          (intValArgsNB (fuel+1) args env).bind (fun vs =>
            denoteNB fuel fn.body (env.bindParams fn.params vs))
      | none    => none
  | 0, Expr.specCall _ _, _ => none
  | _, _, _ => some True
  termination_by fuel e _ => (fuel, sizeOf e)

/-- The none-propagating match-arm SELECTION (the `denoteArms` mirror). -/
noncomputable def denoteArmsNB : Nat → OptResVal → List MatchArm → Env → Option Prop
  | _,    _,     [], _ => some True
  | fuel, scrut, MatchArm.mk variant binder body :: rest, env =>
      if scrut.variant = variant then
        match binder with
        | some x => denoteNB fuel body (env.bindInt x scrut.payload)
        | none   => denoteNB fuel body env
      else
        denoteArmsNB fuel scrut rest env
  termination_by fuel _ arms _ => (fuel, sizeOf arms)
end

/-- The `countWhere` NB-count agreement, PARAMETRIZED by the per-element predicate
    agreement `hp` (so it is NOT a forward reference into the mutual agreement block). When
    `countWhereValNB pNB s = some v`, every element's NB predicate is `some` and (by `hp`)
    carries the SAME proposition as the spine predicate, so the two `ite`-sums coincide. -/
theorem countWhereVal_agrees_of (pNB : Int → Option Prop) (pSp : Int → Prop)
    (hp : ∀ x px, pNB x = some px → (pSp x ↔ px)) :
    ∀ (s : List Int) (v : Int), countWhereValNB pNB s = some v → countWhereVal pSp s = v
  | [], v, h => by
      simp only [countWhereValNB, Option.some.injEq] at h
      simp only [countWhereVal]; omega
  | x :: xs, v, h => by
      rw [countWhereValNB_cons] at h
      cases hpx : pNB x with
      | none => rw [hpx] at h; simp at h
      | some px =>
          rw [hpx] at h; simp only [Option.bind] at h
          cases hrest : countWhereValNB pNB xs with
          | none => rw [hrest] at h; simp at h
          | some rv =>
              rw [hrest] at h; simp only [Option.some.injEq] at h
              rw [countWhereVal_cons, countWhereVal_agrees_of pNB pSp hp xs rv hrest]
              rw [show (@ite _ (pSp x) (Classical.propDecidable _) (1 : Int) 0)
                  = (@ite _ px (Classical.propDecidable _) (1 : Int) 0) from by
                rw [propext (hp x px hpx)]]
              omega

/-- A bind whose continuation is the CONSTANT `fun _ => some k`: if the whole bind equals
    `some P`, then `k = P` (the gate/scrutinee `Option` value is irrelevant to the carried
    proposition). The #242 comb-arm agreement key — the predicate gate decides `some`/`none`
    but never WHICH proposition is carried. -/
theorem Option.bind_const_eq_some_inj {α : Type} {k P : α} :
    ∀ {o : Option Unit}, o.bind (fun _ => some k) = some P → k = P
  | none, h => by simp only [Option.bind] at h; exact absurd h (by simp)
  | some _, h => by simp only [Option.bind, Option.some.injEq] at h; exact h

/-! ## The AGREEMENT LEMMAS (the real content #241 demands)

Where the none-propagating denotation yields `some v` (resp. `some P`), the BOTTOMING
denotation yields the SAME `v` (resp. an equivalent `P`): no bottom arm was taken, so the
two run IDENTICALLY. Proved by the SAME mutual well-founded recursion as the spine
(`(fuel, sizeOf e)`). This is what turns `Converges` into genuine `stabilizes` content — a
divergent registry has NO `some` witness, so the agreement is VACUOUS there, exactly
right. -/

mutual
/-- AGREEMENT (SEQUENCE side): `seqValNB f e env = some s → seqVal f e env = s`. -/
theorem seqValNB_agrees : ∀ (f : Nat) (e : Expr) (env : Env) (s : List Int),
    seqValNB f e env = some s → seqVal f e env = s
  | f, Expr.seqVar x, env, s, h => by
      simp only [seqValNB, Option.some.injEq] at h; simp only [seqVal]; exact h
  | f, Expr.strVar x, env, s, h => by
      simp only [seqValNB, Option.some.injEq] at h; simp only [seqVal]; exact h
  | f, Expr.subrange base r, env, s, h => by
      cases r with
      | rangeTo hi =>
          simp only [seqValNB] at h
          cases hb : seqValNB f base env with
          | none => rw [hb] at h; simp at h
          | some sb =>
              rw [hb] at h; simp only [Option.bind] at h
              cases hhi : intValNB f hi env with
              | none => rw [hhi] at h; simp at h
              | some hv =>
                  rw [hhi] at h; simp only [Option.some.injEq] at h
                  simp only [seqVal, seqValNB_agrees f base env sb hb,
                    intValNB_agrees f hi env hv hhi]; exact h
      | range lo hi =>
          simp only [seqValNB] at h
          cases hb : seqValNB f base env with
          | none => rw [hb] at h; simp at h
          | some sb =>
              rw [hb] at h; simp only [Option.bind] at h
              cases hlo : intValNB f lo env with
              | none => rw [hlo] at h; simp at h
              | some lv =>
                  rw [hlo] at h
                  cases hhi : intValNB f hi env with
                  | none => rw [hhi] at h; simp at h
                  | some hv =>
                      rw [hhi] at h; simp only [Option.some.injEq] at h
                      simp only [seqVal, seqValNB_agrees f base env sb hb,
                        intValNB_agrees f lo env lv hlo,
                        intValNB_agrees f hi env hv hhi]; exact h
      | rangeFrom lo =>
          simp only [seqValNB] at h
          cases hb : seqValNB f base env with
          | none => rw [hb] at h; simp at h
          | some sb =>
              rw [hb] at h; simp only [Option.bind] at h
              cases hlo : intValNB f lo env with
              | none => rw [hlo] at h; simp at h
              | some lv =>
                  rw [hlo] at h; simp only [Option.some.injEq] at h
                  simp only [seqVal, seqValNB_agrees f base env sb hb,
                    intValNB_agrees f lo env lv hlo]; exact h
  | f, Expr.intLit n, env, s, h => by simp only [seqValNB, Option.some.injEq] at h; simp only [seqVal]; exact h
  | f, Expr.boolLit b, env, s, h => by simp only [seqValNB, Option.some.injEq] at h; simp only [seqVal]; exact h
  | f, Expr.var x, env, s, h => by simp only [seqValNB, Option.some.injEq] at h; simp only [seqVal]; exact h
  | f, Expr.boolVar x, env, s, h => by simp only [seqValNB, Option.some.injEq] at h; simp only [seqVal]; exact h
  | f, Expr.optResVar x, env, s, h => by simp only [seqValNB, Option.some.injEq] at h; simp only [seqVal]; exact h
  | f, Expr.cmp op a b, env, s, h => by simp only [seqValNB, Option.some.injEq] at h; simp only [seqVal]; exact h
  | f, Expr.logic op a b, env, s, h => by simp only [seqValNB, Option.some.injEq] at h; simp only [seqVal]; exact h
  | f, Expr.neg a, env, s, h => by simp only [seqValNB, Option.some.injEq] at h; simp only [seqVal]; exact h
  | f, Expr.arith op a b, env, s, h => by simp only [seqValNB, Option.some.injEq] at h; simp only [seqVal]; exact h
  | f, Expr.cast inner ty, env, s, h => by simp only [seqValNB, Option.some.injEq] at h; simp only [seqVal]; exact h
  | f, Expr.idx base i, env, s, h => by simp only [seqValNB, Option.some.injEq] at h; simp only [seqVal]; exact h
  | f, Expr.seqLen base, env, s, h => by simp only [seqValNB, Option.some.injEq] at h; simp only [seqVal]; exact h
  | f, Expr.byteAt base i, env, s, h => by simp only [seqValNB, Option.some.injEq] at h; simp only [seqVal]; exact h
  | f, Expr.comb c seq seq2 idx pred, env, s, h => by simp only [seqValNB, Option.some.injEq] at h; simp only [seqVal]; exact h
  | f, Expr.match_ scrut arms, env, s, h => by simp only [seqValNB, Option.some.injEq] at h; simp only [seqVal]; exact h
  | f, Expr.is_ scrut v, env, s, h => by simp only [seqValNB, Option.some.injEq] at h; simp only [seqVal]; exact h
  | f, Expr.specCall name args, env, s, h => by simp only [seqValNB, Option.some.injEq] at h; simp only [seqVal]; exact h
  termination_by f e _ _ => (f, sizeOf e)

/-- AGREEMENT (INT side, the #241 core): `intValNB f e env = some v → intVal f e env = v`. -/
theorem intValNB_agrees : ∀ (f : Nat) (e : Expr) (env : Env) (v : Int),
    intValNB f e env = some v → intVal f e env = v
  | f, Expr.arith op a b, env, v, h => by
      simp only [intValNB] at h
      cases ha : intValNB f a env with
      | none => rw [ha] at h; simp at h
      | some av =>
          rw [ha] at h; simp only [Option.bind] at h
          cases hb : intValNB f b env with
          | none => rw [hb] at h; simp at h
          | some bv =>
              rw [hb] at h; simp only [Option.some.injEq] at h
              simp only [intVal, intValNB_agrees f a env av ha, intValNB_agrees f b env bv hb]
              exact h
  | f, Expr.cast inner ty, env, v, h => by
      simp only [intValNB] at h
      cases hi : intValNB f inner env with
      | none => rw [hi] at h; simp at h
      | some iv =>
          rw [hi] at h; simp only [Option.bind, Option.some.injEq] at h
          simp only [intVal, intValNB_agrees f inner env iv hi]; exact h
  | f, Expr.idx base i, env, v, h => by
      simp only [intValNB] at h
      cases hs : seqValNB f base env with
      | none => rw [hs] at h; simp at h
      | some sv =>
          rw [hs] at h; simp only [Option.bind] at h
          cases hi : intValNB f i env with
          | none => rw [hi] at h; simp at h
          | some iv =>
              rw [hi] at h; simp only [Option.some.injEq] at h
              simp only [intVal, seqValNB_agrees f base env sv hs, intValNB_agrees f i env iv hi]
              exact h
  | f, Expr.seqLen base, env, v, h => by
      simp only [intValNB] at h
      cases hs : seqValNB f base env with
      | none => rw [hs] at h; simp at h
      | some sv =>
          rw [hs] at h; simp only [Option.bind, Option.some.injEq] at h
          simp only [intVal, seqValNB_agrees f base env sv hs]; exact h
  | f, Expr.byteAt base i, env, v, h => by
      simp only [intValNB] at h
      cases hs : seqValNB f base env with
      | none => rw [hs] at h; simp at h
      | some sv =>
          rw [hs] at h; simp only [Option.bind] at h
          cases hi : intValNB f i env with
          | none => rw [hi] at h; simp at h
          | some iv =>
              rw [hi] at h; simp only [Option.some.injEq] at h
              simp only [intVal, seqValNB_agrees f base env sv hs, intValNB_agrees f i env iv hi]
              exact h
  | f+1, Expr.specCall name args, env, v, h => by
      simp only [intValNB] at h
      cases hr : env.specs name with
      | none => rw [hr] at h; simp at h
      | some fn =>
          rw [hr] at h
          cases hargs : intValArgsNB (f+1) args env with
          | none => rw [hargs] at h; simp at h
          | some vs =>
              rw [hargs] at h; simp only [Option.bind] at h
              simp only [intVal, hr, intValArgsNB_agrees (f+1) args env vs hargs]
              exact intValNB_agrees f fn.body (env.bindParams fn.params vs) v h
  | 0, Expr.specCall name args, env, v, h => by simp only [intValNB] at h; exact absurd h (by simp)
  | f, Expr.comb CombName.countWhere seq seq2 idx pred, env, v, h => by
      simp only [intValNB] at h
      cases hs : seqValNB f seq env with
      | none => rw [hs] at h; simp at h
      | some sv =>
          rw [hs] at h; simp only [Option.bind] at h
          simp only [intVal, seqValNB_agrees f seq env sv hs]
          refine countWhereVal_agrees_of _ _ ?_ sv v h
          intro x px hpx
          cases pred with
          | none => simp only at hpx ⊢; rw [← (Option.some.injEq _ _).mp hpx]
          | some pr =>
              cases pr with
              | mk bound body =>
                  simp only at hpx ⊢
                  exact denoteNB_agrees f body (env.bindInt bound x) px hpx
  | f, Expr.comb CombName.forallIn seq seq2 idx pred, env, v, h => by
      simp only [intValNB, Option.some.injEq] at h; simp only [intVal]; exact h
  | f, Expr.comb CombName.existsIn seq seq2 idx pred, env, v, h => by
      simp only [intValNB, Option.some.injEq] at h; simp only [intVal]; exact h
  | f, Expr.comb CombName.sorted seq seq2 idx pred, env, v, h => by
      simp only [intValNB, Option.some.injEq] at h; simp only [intVal]; exact h
  | f, Expr.comb CombName.forallBelow seq seq2 idx pred, env, v, h => by
      simp only [intValNB, Option.some.injEq] at h; simp only [intVal]; exact h
  | f, Expr.comb CombName.forallFrom seq seq2 idx pred, env, v, h => by
      simp only [intValNB, Option.some.injEq] at h; simp only [intVal]; exact h
  | f, Expr.comb CombName.disjoint seq seq2 idx pred, env, v, h => by
      simp only [intValNB, Option.some.injEq] at h; simp only [intVal]; exact h
  | f, Expr.comb CombName.permutationOf seq seq2 idx pred, env, v, h => by
      simp only [intValNB, Option.some.injEq] at h; simp only [intVal]; exact h
  | f, Expr.intLit n, env, v, h => by
      simp only [intValNB, Option.some.injEq] at h; simp only [intVal]; exact h
  | f, Expr.var x, env, v, h => by
      simp only [intValNB, Option.some.injEq] at h; simp only [intVal]; exact h
  | f, Expr.boolVar x, env, v, h => by
      simp only [intValNB, Option.some.injEq] at h; simp only [intVal]; exact h
  | f, Expr.boolLit b, env, v, h => by
      simp only [intValNB, Option.some.injEq] at h; simp only [intVal]; exact h
  | f, Expr.seqVar x, env, v, h => by
      simp only [intValNB, Option.some.injEq] at h; simp only [intVal]; exact h
  | f, Expr.strVar x, env, v, h => by
      simp only [intValNB, Option.some.injEq] at h; simp only [intVal]; exact h
  | f, Expr.optResVar x, env, v, h => by
      simp only [intValNB, Option.some.injEq] at h; simp only [intVal]; exact h
  | f, Expr.cmp op a b, env, v, h => by
      simp only [intValNB, Option.some.injEq] at h; simp only [intVal]; exact h
  | f, Expr.logic op a b, env, v, h => by
      simp only [intValNB, Option.some.injEq] at h; simp only [intVal]; exact h
  | f, Expr.neg a, env, v, h => by
      simp only [intValNB, Option.some.injEq] at h; simp only [intVal]; exact h
  | f, Expr.subrange base r, env, v, h => by
      simp only [intValNB, Option.some.injEq] at h; simp only [intVal]; exact h
  | f, Expr.match_ scrut arms, env, v, h => by
      simp only [intValNB, Option.some.injEq] at h; simp only [intVal]; exact h
  | f, Expr.is_ scrut vv, env, v, h => by
      simp only [intValNB, Option.some.injEq] at h; simp only [intVal]; exact h
  termination_by f e _ _ => (f, sizeOf e)

/-- AGREEMENT (ARG side): `intValArgsNB f args env = some vs → intValArgs f args env = vs`. -/
theorem intValArgsNB_agrees : ∀ (f : Nat) (args : List Expr) (env : Env) (vs : List Int),
    intValArgsNB f args env = some vs → intValArgs f args env = vs
  | f, [], env, vs, h => by
      simp only [intValArgsNB, Option.some.injEq] at h; simp only [intValArgs]; exact h
  | f, a :: rest, env, vs, h => by
      simp only [intValArgsNB] at h
      cases ha : intValNB f a env with
      | none => rw [ha] at h; simp at h
      | some av =>
          rw [ha] at h; simp only [Option.bind] at h
          cases hrest : intValArgsNB f rest env with
          | none => rw [hrest] at h; simp at h
          | some rvs =>
              rw [hrest] at h; simp only [Option.some.injEq] at h
              simp only [intValArgs, intValNB_agrees f a env av ha,
                intValArgsNB_agrees f rest env rvs hrest]
              exact h
  termination_by f args _ _ => (f, sizeOf args)

/-- AGREEMENT (PROP side): `denoteNB f e env = some P → (denote f e env ↔ P)`. -/
theorem denoteNB_agrees : ∀ (f : Nat) (e : Expr) (env : Env) (P : Prop),
    denoteNB f e env = some P → (denote f e env ↔ P)
  | f, Expr.boolLit b, env, P, h => by
      simp only [denoteNB, Option.some.injEq] at h; simp only [denote]; rw [h]
  | f, Expr.cmp op a b, env, P, h => by
      simp only [denoteNB] at h
      cases ha : intValNB f a env with
      | none => rw [ha] at h; simp at h
      | some av =>
          rw [ha] at h; simp only [Option.bind] at h
          cases hb : intValNB f b env with
          | none => rw [hb] at h; simp at h
          | some bv =>
              rw [hb] at h; simp only [Option.some.injEq] at h
              rw [← h]
              cases op <;>
                simp only [denote, intValNB_agrees f a env av ha, intValNB_agrees f b env bv hb]
  | f, Expr.logic op a b, env, P, h => by
      simp only [denoteNB] at h
      cases ha : denoteNB f a env with
      | none => rw [ha] at h; simp at h
      | some pa =>
          rw [ha] at h; simp only [Option.bind] at h
          cases hb : denoteNB f b env with
          | none => rw [hb] at h; simp at h
          | some pb =>
              rw [hb] at h; simp only [Option.some.injEq] at h
              have iha := denoteNB_agrees f a env pa ha
              have ihb := denoteNB_agrees f b env pb hb
              rw [← h]
              cases op <;> simp only [denote] <;>
                first
                  | exact and_congr iha ihb
                  | exact or_congr iha ihb
  | f, Expr.neg e, env, P, h => by
      simp only [denoteNB] at h
      cases he : denoteNB f e env with
      | none => rw [he] at h; simp at h
      | some pe =>
          rw [he] at h; simp only [Option.bind, Option.some.injEq] at h
          have ih := denoteNB_agrees f e env pe he
          rw [← h]; simp only [denote]; exact not_congr ih
  | f, Expr.match_ scrut arms, env, P, h => by
      simp only [denoteNB] at h
      simp only [denote]
      exact denoteArmsNB_agrees f (scrutVal scrut env) arms env P h
  | f, Expr.is_ scrut variant, env, P, h => by
      simp only [denoteNB, Option.some.injEq] at h; simp only [denote]; rw [h]
  | f, Expr.comb c seq seq2 idx pred, env, P, h => by
      -- The comb NB arm carries EXACTLY the spine `denote` proposition once the slice/`seq2`/
      -- `idx` binds AND the per-element predicate gate succeed, so the agreement is reflexive
      -- after extracting `P = denote …` (the gate decides `some`/`none`, never the carried
      -- proposition).
      unfold denoteNB at h
      cases hs : seqValNB f seq env with
      | none => rw [hs] at h; simp at h
      | some sv =>
          rw [hs] at h; simp only [Option.bind] at h
          cases hs2 : (match seq2 with | some e => seqValNB f e env | none => some []) with
          | none => rw [hs2] at h; simp at h
          | some s2v =>
              rw [hs2] at h
              cases hn : (match idx with | some e => intValNB f e env | none => some 0) with
              | none => rw [hn] at h; simp at h
              | some nv =>
                  rw [hn] at h
                  -- The gate's VALUE is irrelevant to the carried proposition (the gate only
                  -- decides `some`/`none`): the bind's continuation is the CONSTANT
                  -- `fun _ => some (denote …)`, so `h` forces `P = denote …` regardless of the
                  -- gate, and the agreement is reflexive. (A divergent predicate makes the gate
                  -- `none`, so the `some P` equation cannot hold there at all —
                  -- `PinCombPredGap.lean` F.2 inverts.)
                  exact iff_of_eq (Option.bind_const_eq_some_inj h)
  | f+1, Expr.specCall name args, env, P, h => by
      simp only [denoteNB] at h
      cases hr : env.specs name with
      | none => rw [hr] at h; simp at h
      | some fn =>
          rw [hr] at h
          cases hargs : intValArgsNB (f+1) args env with
          | none => rw [hargs] at h; simp at h
          | some vs =>
              rw [hargs] at h; simp only [Option.bind] at h
              simp only [denote, hr, intValArgsNB_agrees (f+1) args env vs hargs]
              exact denoteNB_agrees f fn.body (env.bindParams fn.params vs) P h
  | 0, Expr.specCall name args, env, P, h => by simp only [denoteNB] at h; exact absurd h (by simp)
  | f, Expr.intLit n, env, P, h => by
      simp only [denoteNB, Option.some.injEq] at h; simp only [denote]; rw [h]
  | f, Expr.var x, env, P, h => by
      simp only [denoteNB, Option.some.injEq] at h; simp only [denote]; rw [h]
  -- THE #253 BOOL-VAR: NB carries the SAME `env.bools x = true` proposition `denote` does
  -- (the explicit arm), so the agreement is `denote ↔ P` after rewriting `P` from `h`.
  | f, Expr.boolVar x, env, P, h => by
      simp only [denoteNB, Option.some.injEq] at h; simp only [denote]; rw [h]
  | f, Expr.seqVar x, env, P, h => by
      simp only [denoteNB, Option.some.injEq] at h; simp only [denote]; rw [h]
  | f, Expr.strVar x, env, P, h => by
      simp only [denoteNB, Option.some.injEq] at h; simp only [denote]; rw [h]
  | f, Expr.optResVar x, env, P, h => by
      simp only [denoteNB, Option.some.injEq] at h; simp only [denote]; rw [h]
  | f, Expr.arith op a b, env, P, h => by
      simp only [denoteNB, Option.some.injEq] at h; simp only [denote]; rw [h]
  | f, Expr.cast inner ty, env, P, h => by
      simp only [denoteNB, Option.some.injEq] at h; simp only [denote]; rw [h]
  | f, Expr.idx base i, env, P, h => by
      simp only [denoteNB, Option.some.injEq] at h; simp only [denote]; rw [h]
  | f, Expr.subrange base r, env, P, h => by
      simp only [denoteNB, Option.some.injEq] at h; simp only [denote]; rw [h]
  | f, Expr.seqLen base, env, P, h => by
      simp only [denoteNB, Option.some.injEq] at h; simp only [denote]; rw [h]
  | f, Expr.byteAt base i, env, P, h => by
      simp only [denoteNB, Option.some.injEq] at h; simp only [denote]; rw [h]
  termination_by f e _ _ => (f, sizeOf e)

/-- AGREEMENT (match-arm side): `denoteArmsNB f scrut arms env = some P → (denoteArms … ↔ P)`. -/
theorem denoteArmsNB_agrees : ∀ (f : Nat) (scrut : OptResVal) (arms : List MatchArm)
    (env : Env) (P : Prop),
    denoteArmsNB f scrut arms env = some P → (denoteArms f scrut arms env ↔ P)
  | f, scrut, [], env, P, h => by
      simp only [denoteArmsNB, Option.some.injEq] at h
      rw [denoteArms.eq_def]; rw [h]
  | f, scrut, MatchArm.mk variant binder body :: rest, env, P, h => by
      unfold denoteArmsNB at h
      rw [denoteArms.eq_def]
      by_cases hv : scrut.variant = variant
      · simp only [hv, if_true] at h ⊢
        cases binder with
        | some x => exact denoteNB_agrees f body (env.bindInt x scrut.payload) P h
        | none => exact denoteNB_agrees f body env P h
      · simp only [hv, if_false] at h ⊢
        exact denoteArmsNB_agrees f scrut rest env P h
  termination_by f _ arms _ _ => (f, sizeOf arms)
end

/-! ## `Converges` — the bottom-distinguishing analogue of `stabilizes`

`Converges e env v` says the NONE-PROPAGATING denotation reaches `some v` at all
sufficiently large fuel — i.e. `e` GENUINELY computes `v` per env, never bottoming. Unlike
`stabilizes` (which a divergent registry satisfies at the Int-bottom `0`), `Converges`
requires `intValNB` to be `some` — which a divergent `specCall` NEVER is. The agreement
lemma `converges_imp_stabilizes` carries `Converges`'s genuine content into `stabilizes`. -/

/-- `Converges e env v`: the none-propagating `intValNB` reaches `some v` at all fuel ≥
    some per-env `N`. The BOTTOM-DISTINGUISHING analogue of `stabilizes` — a divergent
    registry has NO witness (its `intValNB` is `none` at every fuel). -/
def Converges (e : Expr) (env : Env) (v : Int) : Prop :=
  ∃ N, ∀ fuel, fuel ≥ N → intValNB fuel e env = some v

/-- THE AGREEMENT LEMMA (the real #241 content): convergence implies stabilization. When
    the none-propagating denotation yields `some v` at all large fuel, the bottoming
    `intVal` yields the SAME `v` there (`intValNB_agrees`) — so `e` genuinely STABILIZES to
    `v`, NOT to a bottom-poisoned artifact. -/
theorem converges_imp_stabilizes {e : Expr} {env : Env} {v : Int}
    (h : Converges e env v) : stabilizes e env v := by
  obtain ⟨N, hN⟩ := h
  exact ⟨N, fun fuel hfuel => intValNB_agrees fuel e env v (hN fuel hfuel)⟩

/-! ## The spec-call-free fragment CONVERGES unconditionally (the tier-(a) companion)

A spec-call-free `e` never reaches a `specCall` arm, so `intValNB` is TOTAL-`some` and
equals the bottoming `intVal` at EVERY fuel — and `intVal` is fuel-irrelevant there
(`intVal_fuel_irrelevant`). So such `e` CONVERGES with witness `N = 0` and NO registry-
termination hypothesis: the auto-fragment's convergence is free. -/

/-- The `countWhere` NB count, when the per-element NB predicate is TOTAL-`some` (`hp`),
    equals `some` of the spine count. PARAMETRIZED by `hp` (not a forward reference into the
    spec-call-free mutual block — `intValNB_specCallFree`'s countWhere arm supplies `hp`
    from `denoteNB_specCallFree`). -/
theorem countWhereValNB_eq_of (pNB : Int → Option Prop) (pSp : Int → Prop)
    (hp : ∀ x, pNB x = some (pSp x)) :
    ∀ s : List Int, countWhereValNB pNB s = some (countWhereVal pSp s)
  | [] => by simp only [countWhereValNB, countWhereVal]
  | x :: xs => by
      rw [countWhereValNB_cons, countWhereVal_cons, countWhereValNB_eq_of pNB pSp hp xs, hp x]
      simp only [Option.bind]

mutual
/-- On the spec-call-free fragment, `seqValNB` is total-`some` and equals `seqVal`. -/
theorem seqValNB_specCallFree : ∀ (e : Expr) (env : Env) (f : Nat),
    specCallFree e = true → seqValNB f e env = some (seqVal f e env)
  | Expr.seqVar x, env, f, _ => by simp only [seqValNB, seqVal]
  | Expr.strVar x, env, f, _ => by simp only [seqValNB, seqVal]
  | Expr.subrange base r, env, f, hf => by
      simp only [specCallFree, Bool.and_eq_true] at hf
      have hbase : specCallFree base = true := hf.1
      have hrange : rangeFree r = true := hf.2
      cases r with
      | rangeTo hi =>
          have hhi : specCallFree hi = true := by
            simp only [rangeFree] at hrange; exact hrange
          simp only [seqValNB, seqVal, seqValNB_specCallFree base env f hbase,
            intValNB_specCallFree hi env f hhi, Option.bind]
      | range lo hi =>
          have hlh : specCallFree lo = true ∧ specCallFree hi = true := by
            simp only [rangeFree, Bool.and_eq_true] at hrange; exact hrange
          simp only [seqValNB, seqVal, seqValNB_specCallFree base env f hbase,
            intValNB_specCallFree lo env f hlh.1, intValNB_specCallFree hi env f hlh.2,
            Option.bind]
      | rangeFrom lo =>
          have hlo : specCallFree lo = true := by
            simp only [rangeFree] at hrange; exact hrange
          simp only [seqValNB, seqVal, seqValNB_specCallFree base env f hbase,
            intValNB_specCallFree lo env f hlo, Option.bind]
  | Expr.intLit n, env, f, _ => by simp only [seqValNB, seqVal]
  | Expr.boolLit b, env, f, _ => by simp only [seqValNB, seqVal]
  | Expr.var x, env, f, _ => by simp only [seqValNB, seqVal]
  | Expr.boolVar x, env, f, _ => by simp only [seqValNB, seqVal]
  | Expr.optResVar x, env, f, _ => by simp only [seqValNB, seqVal]
  | Expr.cmp op a b, env, f, _ => by simp only [seqValNB, seqVal]
  | Expr.logic op a b, env, f, _ => by simp only [seqValNB, seqVal]
  | Expr.neg a, env, f, _ => by simp only [seqValNB, seqVal]
  | Expr.arith op a b, env, f, _ => by simp only [seqValNB, seqVal]
  | Expr.cast inner ty, env, f, _ => by simp only [seqValNB, seqVal]
  | Expr.idx base i, env, f, _ => by simp only [seqValNB, seqVal]
  | Expr.seqLen base, env, f, _ => by simp only [seqValNB, seqVal]
  | Expr.byteAt base i, env, f, _ => by simp only [seqValNB, seqVal]
  | Expr.comb c seq seq2 idx pred, env, f, _ => by simp only [seqValNB, seqVal]
  | Expr.match_ scrut arms, env, f, _ => by simp only [seqValNB, seqVal]
  | Expr.is_ scrut v, env, f, _ => by simp only [seqValNB, seqVal]
  | Expr.specCall name args, env, f, hf => by
      simp only [specCallFree] at hf; exact absurd hf Bool.false_ne_true
  termination_by e => sizeOf e

/-- On the spec-call-free fragment, `intValNB` is total-`some` and equals `intVal`. -/
theorem intValNB_specCallFree : ∀ (e : Expr) (env : Env) (f : Nat),
    specCallFree e = true → intValNB f e env = some (intVal f e env)
  | Expr.arith op a b, env, f, hf => by
      simp only [specCallFree, Bool.and_eq_true] at hf
      simp only [intValNB, intVal, intValNB_specCallFree a env f hf.1,
        intValNB_specCallFree b env f hf.2, Option.bind]
  | Expr.cast inner ty, env, f, hf => by
      simp only [specCallFree] at hf
      simp only [intValNB, intVal, intValNB_specCallFree inner env f hf, Option.bind]
  | Expr.idx base i, env, f, hf => by
      simp only [specCallFree, Bool.and_eq_true] at hf
      simp only [intValNB, intVal, seqValNB_specCallFree base env f hf.1,
        intValNB_specCallFree i env f hf.2, Option.bind]
  | Expr.seqLen base, env, f, hf => by
      simp only [specCallFree] at hf
      simp only [intValNB, intVal, seqValNB_specCallFree base env f hf, Option.bind]
  | Expr.byteAt base i, env, f, hf => by
      simp only [specCallFree, Bool.and_eq_true] at hf
      simp only [intValNB, intVal, seqValNB_specCallFree base env f hf.1,
        intValNB_specCallFree i env f hf.2, Option.bind]
  | Expr.comb c seq seq2 idx pred, env, f, hf => by
      simp only [specCallFree, Bool.and_eq_true] at hf
      cases c with
      | countWhere =>
          have hseq : specCallFree seq = true := hf.1.1.1
          have hpred : optPredFree pred = true := hf.2
          simp only [intValNB, intVal, seqValNB_specCallFree seq env f hseq, Option.bind]
          refine countWhereValNB_eq_of _ _ ?_ (seqVal f seq env)
          intro x
          cases pred with
          | none => simp only
          | some pr =>
              cases pr with
              | mk bound body =>
                  have hbody : specCallFree body = true := by
                    simp only [optPredFree, predFree] at hpred; exact hpred
                  simp only [denoteNB_specCallFree body (env.bindInt bound x) f hbody]
      | forallIn => simp only [intValNB, intVal]
      | existsIn => simp only [intValNB, intVal]
      | sorted => simp only [intValNB, intVal]
      | forallBelow => simp only [intValNB, intVal]
      | forallFrom => simp only [intValNB, intVal]
      | disjoint => simp only [intValNB, intVal]
      | permutationOf => simp only [intValNB, intVal]
  | Expr.intLit n, env, f, _ => by simp only [intValNB, intVal]
  | Expr.var x, env, f, _ => by simp only [intValNB, intVal]
  | Expr.boolVar x, env, f, _ => by simp only [intValNB, intVal]
  | Expr.boolLit b, env, f, _ => by simp only [intValNB, intVal]
  | Expr.seqVar x, env, f, _ => by simp only [intValNB, intVal]
  | Expr.strVar x, env, f, _ => by simp only [intValNB, intVal]
  | Expr.optResVar x, env, f, _ => by simp only [intValNB, intVal]
  | Expr.cmp op a b, env, f, _ => by simp only [intValNB, intVal]
  | Expr.logic op a b, env, f, _ => by simp only [intValNB, intVal]
  | Expr.neg a, env, f, _ => by simp only [intValNB, intVal]
  | Expr.subrange base r, env, f, _ => by simp only [intValNB, intVal]
  | Expr.match_ scrut arms, env, f, _ => by simp only [intValNB, intVal]
  | Expr.is_ scrut v, env, f, _ => by simp only [intValNB, intVal]
  | Expr.specCall name args, env, f, hf => by
      simp only [specCallFree] at hf; exact absurd hf Bool.false_ne_true
  termination_by e => sizeOf e

/-- The predicate GATE is `some ()` on the spec-call-free fragment (the #242 auto-fragment
    companion): when the predicate option is spec-call-free, every element's predicate body is
    spec-call-free, so `denoteNB body` is total-`some` and the gate never poisons. A STANDALONE
    member of the spec-call-free mutual block (supplies the comb arm's `hgate` with NO
    `pred`-mentioning context hyp polluting the lambda motive — the same discipline as
    `comb_pred_fuel_iff`). -/
theorem predGateNB_specCallFree : ∀ (pred : Option Pred) (env : Env) (f : Nat),
    optPredFree pred = true → ∀ s : List Int,
      predGateNB
        (fun x => match pred with
          | some (Pred.mk bound body) => denoteNB f body (env.bindInt bound x)
          | none => some True) s = some ()
  | none, _, _, _, s => by
      refine predGateNB_some_of _ s (fun x => ?_); exact ⟨True, rfl⟩
  | some (Pred.mk bound body), env, f, hpred, s => by
      have hbody : specCallFree body = true := by
        simp only [optPredFree, predFree] at hpred; exact hpred
      refine predGateNB_some_of _ s (fun x => ?_)
      exact ⟨denote f body (env.bindInt bound x),
        denoteNB_specCallFree body (env.bindInt bound x) f hbody⟩
  termination_by pred _ _ _ => sizeOf pred

/-- On the spec-call-free fragment, `denoteNB` is total-`some` and carries exactly `denote`. -/
theorem denoteNB_specCallFree : ∀ (e : Expr) (env : Env) (f : Nat),
    specCallFree e = true → denoteNB f e env = some (denote f e env)
  | Expr.boolLit b, env, f, _ => by simp only [denoteNB, denote]
  | Expr.is_ scrut v, env, f, _ => by simp only [denoteNB, denote]
  | Expr.cmp op a b, env, f, hf => by
      simp only [specCallFree, Bool.and_eq_true] at hf
      cases op <;>
        simp only [denoteNB, denote, intValNB_specCallFree a env f hf.1,
          intValNB_specCallFree b env f hf.2, Option.bind]
  | Expr.logic op a b, env, f, hf => by
      simp only [specCallFree, Bool.and_eq_true] at hf
      cases op <;>
        simp only [denoteNB, denote, denoteNB_specCallFree a env f hf.1,
          denoteNB_specCallFree b env f hf.2, Option.bind]
  | Expr.neg a, env, f, hf => by
      simp only [specCallFree] at hf
      simp only [denoteNB, denote, denoteNB_specCallFree a env f hf, Option.bind]
  | Expr.match_ scrut arms, env, f, hf => by
      simp only [specCallFree, Bool.and_eq_true] at hf
      simp only [denoteNB, denote]
      exact denoteArmsNB_specCallFree (scrutVal scrut env) arms env f hf.2
  | Expr.comb c seq seq2 idx pred, env, f, hf => by
      simp only [specCallFree, Bool.and_eq_true] at hf
      have hseq : specCallFree seq = true := hf.1.1.1
      have hseq2 : optExprFree seq2 = true := hf.1.1.2
      have hidx : optExprFree idx = true := hf.1.2
      have hpred : optPredFree pred = true := hf.2
      have hs2 : (match seq2 with | some e => seqValNB f e env | none => some [])
          = some (match seq2 with | some e => seqVal f e env | none => []) := by
        cases seq2 with
        | none => rfl
        | some e =>
            have : specCallFree e = true := by simp only [optExprFree] at hseq2; exact hseq2
            simp only [seqValNB_specCallFree e env f this]
      have hn : (match idx with | some e => intValNB f e env | none => some 0)
          = some (match idx with | some e => intVal f e env | none => (0 : Int)) := by
        cases idx with
        | none => rfl
        | some e =>
            have : specCallFree e = true := by simp only [optExprFree] at hidx; exact hidx
            simp only [intValNB_specCallFree e env f this]
      -- The predicate gate is `some ()` on the spec-call-free fragment (the #242 auto-fragment
      -- companion) — supplied by the standalone `predGateNB_specCallFree`, so no
      -- `pred`-mentioning context hyp pollutes the lambda motive.
      unfold denoteNB
      simp only [seqValNB_specCallFree seq env f hseq, hs2, hn, Option.bind]
      rw [predGateNB_specCallFree pred env f hpred (seqVal f seq env)]
  | Expr.specCall name args, env, f, hf => by
      simp only [specCallFree] at hf; exact absurd hf Bool.false_ne_true
  | Expr.intLit n, env, f, _ => by simp only [denoteNB, denote]
  | Expr.var x, env, f, _ => by simp only [denoteNB, denote]
  -- THE #253 BOOL-VAR: NB and `denote` are the IDENTICAL explicit `env.bools x = true` arm.
  | Expr.boolVar x, env, f, _ => by simp only [denoteNB, denote]
  | Expr.seqVar x, env, f, _ => by simp only [denoteNB, denote]
  | Expr.strVar x, env, f, _ => by simp only [denoteNB, denote]
  | Expr.optResVar x, env, f, _ => by simp only [denoteNB, denote]
  | Expr.arith op a b, env, f, _ => by simp only [denoteNB, denote]
  | Expr.cast inner ty, env, f, _ => by simp only [denoteNB, denote]
  | Expr.idx base i, env, f, _ => by simp only [denoteNB, denote]
  | Expr.subrange base r, env, f, _ => by simp only [denoteNB, denote]
  | Expr.seqLen base, env, f, _ => by simp only [denoteNB, denote]
  | Expr.byteAt base i, env, f, _ => by simp only [denoteNB, denote]
  termination_by e => sizeOf e

/-- On the spec-call-free fragment, `denoteArmsNB` is total-`some` and equals `denoteArms`. -/
theorem denoteArmsNB_specCallFree : ∀ (scrut : OptResVal) (arms : List MatchArm)
    (env : Env) (f : Nat),
    armsFree arms = true → denoteArmsNB f scrut arms env = some (denoteArms f scrut arms env)
  | scrut, [], env, f, _ => by simp only [denoteArmsNB]; rw [denoteArms.eq_def]
  | scrut, MatchArm.mk variant binder body :: rest, env, f, hf => by
      simp only [armsFree, Bool.and_eq_true] at hf
      have hbody : specCallFree body = true := hf.1
      have hrest : armsFree rest = true := hf.2
      unfold denoteArmsNB; rw [denoteArms.eq_def]
      by_cases hv : scrut.variant = variant
      · simp only [hv, if_true]
        cases binder with
        | some x => exact denoteNB_specCallFree body (env.bindInt x scrut.payload) f hbody
        | none => exact denoteNB_specCallFree body env f hbody
      · simp only [hv, if_false]
        exact denoteArmsNB_specCallFree scrut rest env f hrest
  termination_by _ arms _ _ => sizeOf arms
end

/-! ## `stabilization_exists` (the design's `stabilization_exists_for_dec_bounded`),
    REDEFINED on `Converges` (the #241 root fix)

`RegistryTerminating env e := ∃ v, Converges e env v` is no longer an identity hypothesis:
it asserts the NONE-PROPAGATING denotation CONVERGES, which a divergent registry CANNOT
satisfy (`PinRegistryTerminating.lean`'s `divergent_registry_fails_the_hypothesis`).
`stabilization_exists` now carries the AGREEMENT LEMMA's content: the hypothesis is genuine
convergence, the conclusion genuine stabilization (`converges_imp_stabilizes`). -/

/-- `RegistryTerminating env e` (the discharge target of the REGISTRY-TERMINATION class,
    REQ-1.2), REDEFINED on `Converges` (the #241 root fix). `∃ v, Converges e env v` — the
    none-propagating denotation reaches a GENUINE value, which a divergent registry CANNOT
    do (it bottoms to `none` at every fuel). This is NOT the cycle-4 identity hypothesis a
    divergent registry cleared at the Int-bottom `0`; it is exactly the convergence the
    per-item dec-validity proof supplies (a dec-valid registry's `specCall` resolves within
    a finite per-env fuel, so `intValNB` is `some`). -/
def RegistryTerminating (env : Env) (e : Expr) : Prop :=
  ∃ v, Converges e env v

/-- `stabilization_exists` (the design's `stabilization_exists_for_dec_bounded`), NOW with
    GENUINE content (the #241 fix). Under the REGISTRY-TERMINATION hypothesis (`∃ v,
    Converges e env v` — the none-propagating denotation CONVERGES, which the per-item
    dec-validity proof supplies and a divergent registry CANNOT), the BOTTOMING `intVal`
    STABILIZES to the SAME value (`converges_imp_stabilizes`) — NOT a bottom-poisoned
    artifact. UNIQUE (`stabilizes_unique`), so the §4 `∀ r, stabilizes body env r → …`
    premise is inhabited by exactly one genuine `r`. -/
theorem stabilization_exists {env : Env} {e : Expr}
    (h : RegistryTerminating env e) : ∃ v, stabilizes e env v := by
  obtain ⟨v, hv⟩ := h
  exact ⟨v, converges_imp_stabilizes hv⟩

/-- A spec-call-FREE `Expr` CONVERGES WITHOUT any registry-termination hypothesis: `intValNB`
    is total-`some` and fuel-irrelevant on it (`intValNB_specCallFree` + the spine
    `intVal_fuel_irrelevant`), so the witness is the fuel-0 value. The companion to
    `stabilization_exists_specCallFree` — the tier-(a) auto fragment's CONVERGENCE is free. -/
theorem converges_specCallFree {env : Env} {e : Expr} (h : specCallFree e = true) :
    Converges e env (intVal 0 e env) :=
  ⟨0, fun fuel _ => by
    rw [intValNB_specCallFree e env fuel h, intVal_fuel_irrelevant e env fuel 0 h]⟩

/-- A spec-call-FREE `Expr` stabilizes WITHOUT any registry-termination hypothesis: the
    witness is the fuel-0 value, by fuel-irrelevance. The half of `stabilization_exists`
    that holds in core Lean with NO hypothesis. -/
theorem stabilization_exists_specCallFree {env : Env} {e : Expr}
    (h : specCallFree e = true) : ∃ v, stabilizes e env v :=
  ⟨intVal 0 e env, (stabilizes_iff_intVal_zero h).mpr rfl⟩

end Thermite
