/-
  Thermite/Soundness.lean — the (T1) soundness theorem for the comparison + logical
  fragment (#170) extended with the arithmetic operators (#176), the casts (#177), the
  spec-context rewrites (#178), the 6 bounded-quantifier combinators (#179), and the
  match-in-ens / `is` payload-in-contract forms (#180 / 1g — the C7 class); epic #169.

  The #180 match/`is` extension. `ref_sound` gains the `optResVar`/`match_`/`is_` cases; the
  `match_` case threads a mutual `ref_sound_arms` (the arm-walk soundness — each arm body via the
  recursive `ref_sound` IH, the selection-by-variant + payload-binding shared with the source).
  Non-vacuous: the negative `match_arm_swap_breaks_soundness` (a `Some`/`None` body swap disagrees
  at `result := Some 7`) and `is_wrong_variant_breaks_soundness` (`is Some` tested as `is None`
  disagrees) bite; the positives `match_faithful_is_sound`/`match_result_faithful_is_sound`/
  `is_faithful_is_sound` confirm the faithful encoder is sound. Scoped to Option/Result (the
  built-in `Some/None/Ok/Err` `encode_pattern` admits); general user ADTs are out of scope (the
  encoder `Err`s on them) and not embedded.

  Governing design: `.design/verified/thermite-semantics.md` REQ-2 (T1: the
  verified-validator obligation `∀ P, ⟦R(P)⟧ = ⟦P⟧_S`, proved by structural induction)
  + AC-2 (the theorem is non-vacuous: the binop/coercion re-statement is the content)
  + Architecture §"S_C" (the `cast → nat/int` rule: "the paren-drop is a (T1) failure
  (a different parse = a different denotation)"). Field vocabulary
  (formal-methods-sota.md finding #1/#2): the verified-validator soundness step
  (Leroy/CompCert), the kernel-checked core of semantic preservation for this fragment.

  T1 (this fragment): `∀ e env, refDenote e env ↔ denote e env`.
  `refDenote` follows the reference encoder's operator + cast-target maps
  (`encOp`/`encLog`/`encArith`/`encCast` mirror `ref_encode.rs::binop_str`/
  `cast_target`) and its parenthesization (`encode_binary`/`encode_cast` wrap their
  operands/inner); `denote` follows the source meaning. They are defined
  independently (so this is not `rfl`-vacuous) and proved equivalent by induction.

  The #122/#146 retirement (cast-paren class). The negative lemma
  `cast_paren_drop_breaks_soundness` shows a faulty encoder that drops the cast
  paren — emitting `n - 1 as nat` (which Verus/Rust parse as `n - (1 as nat)`)
  instead of the faithful `(n - 1) as nat` — does not satisfy soundness at a
  concrete env. This is the proven retirement of the #122/#146 cast-paren class on
  the contract side: the faithful `encode_cast` paren is what makes T1 hold.
-/
import Thermite.Ast
import Thermite.Denote
import Thermite.RefEncode

namespace Thermite

/-- The arithmetic-token round-trip is faithful: interpreting the encoded token
    `encArith op` equals the shared `arithDenote op`. A per-operator `cases`
    discharges the `encArith`/`tokArith` round-trip (the #176 content). -/
theorem tokArith_encArith (op : ArithOp) (x y : Int) :
    tokArith (encArith op) x y = arithDenote op x y := by
  cases op <;> rfl

/-- The cast-token round-trip is faithful: interpreting the encoded target
    `encCast ty` equals the shared `castDenote ty` (the #177 content). -/
theorem tokCast_encCast (ty : CastTy) (v : Int) :
    tokCast (encCast ty) v = castDenote ty v := by
  cases ty <;> rfl

/-- The byte-view dispatch round-trips are faithful (#178/#127): interpreting the
    encoder's `encByteAt` dispatch (the `byte_at`→`spec_byte_at` choice) over a sequence
    and index equals the source i-th-byte access `seqIdx`; the `encLen` dispatch (the
    `len`→`spec_len`/`.len()` choice) equals the source length. These are the #127
    faithfulness facts — the encoder's dispatch choice is the content. -/
theorem byteView_encByteAt (s : List Int) (i : Int) :
    byteView encByteAt s i = seqIdx s i := rfl

theorem byteView_encLen (s : List Int) (i : Int) :
    byteView encLen s i = (s.length : Int) := rfl

/-- `count_where`'s count depends on the predicate only through its truth at each element (#182):
    two pointwise-equivalent predicates (`∀ x, p x ↔ q x`) yield the same `countWhereVal`. This is
    the congruence the `count_where` soundness case needs — the encoder and source predicates
    (`refDenote body` vs `denote body`) are pointwise-equivalent by the recursive `ref_sound` IH on
    the flat closure body, so the two counts coincide. Proved by structural recursion on the list,
    rewriting each `if p x` to `if q x` via `propext`. -/
theorem countWhereVal_congr (p q : Int → Prop) (hpq : ∀ x, p x ↔ q x) :
    ∀ s : List Int, countWhereVal p s = countWhereVal q s
  | [] => rfl
  | x :: xs => by
      rw [countWhereVal_cons, countWhereVal_cons, countWhereVal_congr p q hpq xs]
      have hx : (p x) = (q x) := propext (hpq x)
      rw [hx]

/- The combined meaning-coincidence (#178): the encoder's `refIntVal` equals the
   source `intVal` and `refSeqVal` equals `seqVal`, simultaneously, on every term, with
   a companion lemma threading a `RangeArg`'s bounds. Because `Expr`/`RangeArg` are
   mutually inductive (the `induction` tactic does not support them), these are proved
   as mutual structural-recursive theorems (the recursive calls are the inductive
   hypotheses; Lean checks the structural decrease). The `@`-view (`seqVar`/`strVar` →
   identity), the `subrange` (→ the same `seqSub`), and the byte-view dispatch
   (`byteView_encByteAt`/`byteView_encLen`) are proven denotation-preserving here; the
   operator/cast round-trips settle #176/#177. -/
mutual
/-- `refIntVal fuel e = intVal fuel e ∧ refSeqVal fuel e = seqVal fuel e`, by well-founded
    recursion on `(fuel, sizeOf e)` (#181). For the non-spec-fn fragment the recursion is
    structural (fuel unchanged, `sizeOf` of the subterm smaller). The #181 `specCall` case (at
    `fuel+1`): the args agree (`refIntValArgs_eq` — the same-fuel arg-list IH, the args smaller by
    `sizeOf`) and the resolved body agrees (`refVal_eq fuel body …` — smaller fuel); since both
    sides resolve `name` in the same `Env.specs` and bind the same (now-equal) arg values, the two
    `intVal`/`refIntVal` of the call coincide. At fuel `0` both bottom to `0`/`[]` identically. -/
theorem refVal_eq : ∀ (fuel : Nat) (e : Expr) (env : Env),
    refIntVal fuel e env = intVal fuel e env ∧ refSeqVal fuel e env = seqVal fuel e env
  | fuel, Expr.intLit n, env => ⟨by simp [refIntVal, intVal], by simp [refSeqVal, seqVal]⟩
  | fuel, Expr.boolLit b, env => ⟨by simp [refIntVal, intVal], by simp [refSeqVal, seqVal]⟩
  | fuel, Expr.var x, env => ⟨by simp [refIntVal, intVal], by simp [refSeqVal, seqVal]⟩
  -- The #253 bool-var: int-sorted bottoms to `0` (the `| _, _, _ => 0` catch-all) on both
  -- sides, seq projection `[]` — both bottom identically (it has no integer/sequence meaning).
  | fuel, Expr.boolVar x, env => ⟨by simp [refIntVal, intVal], by simp [refSeqVal, seqVal]⟩
  | fuel, Expr.cmp op a b, env => ⟨by simp [refIntVal, intVal], by simp [refSeqVal, seqVal]⟩
  | fuel, Expr.logic op a b, env => ⟨by simp [refIntVal, intVal], by simp [refSeqVal, seqVal]⟩
  | fuel, Expr.neg e0, env => ⟨by simp [refIntVal, intVal], by simp [refSeqVal, seqVal]⟩
  | fuel, Expr.arith op a b, env => by
      refine ⟨?_, by simp [refSeqVal, seqVal]⟩
      simp only [refIntVal, intVal, (refVal_eq fuel a env).1, (refVal_eq fuel b env).1,
                 tokArith_encArith]
  | fuel, Expr.cast inner ty, env => by
      refine ⟨?_, by simp [refSeqVal, seqVal]⟩
      simp only [refIntVal, intVal, (refVal_eq fuel inner env).1, tokCast_encCast]
  | fuel, Expr.seqVar x, env => ⟨by simp [refIntVal, intVal], by simp [refSeqVal, seqVal]⟩
  | fuel, Expr.strVar x, env => ⟨by simp [refIntVal, intVal], by simp [refSeqVal, seqVal]⟩
  | fuel, Expr.idx base i, env => by
      refine ⟨?_, by simp [refSeqVal, seqVal]⟩
      simp only [refIntVal, intVal, byteView_encByteAt,
                 (refVal_eq fuel base env).2, (refVal_eq fuel i env).1]
  | fuel, Expr.subrange base r, env => by
      refine ⟨by simp [refIntVal, intVal], ?_⟩
      cases r with
      | rangeTo hi =>
          simp only [refSeqVal, seqVal, (refVal_eq fuel base env).2,
                     (refVal_eq fuel hi env).1]
      | range lo hi =>
          simp only [refSeqVal, seqVal, (refVal_eq fuel base env).2,
                     (refVal_eq fuel lo env).1, (refVal_eq fuel hi env).1]
      | rangeFrom lo =>
          simp only [refSeqVal, seqVal, (refVal_eq fuel base env).2,
                     (refVal_eq fuel lo env).1]
  | fuel, Expr.seqLen base, env => by
      refine ⟨?_, by simp [refSeqVal, seqVal]⟩
      simp only [refIntVal, intVal, byteView_encLen, (refVal_eq fuel base env).2]
  | fuel, Expr.byteAt base i, env => by
      refine ⟨?_, by simp [refSeqVal, seqVal]⟩
      simp only [refIntVal, intVal, byteView_encByteAt,
                 (refVal_eq fuel base env).2, (refVal_eq fuel i env).1]
  -- The combinators. The 6 bounded combinators + `permutationOf` are `Prop`-sorted: their
  -- `refIntVal`/`intVal` both bottom to `0` and `refSeqVal`/`seqVal` to `[]`. The #182 `countWhere`
  -- is value-sorted: its `refIntVal`/`intVal` are `countWhereVal` over the `@`-view, equal by
  -- `countWhereVal_congr` (the slice agrees via `(refVal_eq fuel seq env).2`; the per-element
  -- predicate agrees via the recursive `ref_sound fuel body` IH on the flat closure body, smaller by
  -- `sizeOf` at the same fuel — the well-founded decrease).
  | fuel, Expr.comb c seq seq2 idx pred, env => by
      cases c with
      | countWhere =>
          refine ⟨?_, by simp [refSeqVal, seqVal]⟩
          simp only [refIntVal, intVal]
          have hs : refSeqVal fuel seq env = seqVal fuel seq env := (refVal_eq fuel seq env).2
          rw [hs]
          cases pred with
          | none => rfl
          | some pr => cases pr with
            | mk bound body =>
                apply countWhereVal_congr
                intro x
                exact ref_sound fuel body (env.bindInt bound x)
      | forallIn => exact ⟨by simp [refIntVal, intVal], by simp [refSeqVal, seqVal]⟩
      | existsIn => exact ⟨by simp [refIntVal, intVal], by simp [refSeqVal, seqVal]⟩
      | sorted => exact ⟨by simp [refIntVal, intVal], by simp [refSeqVal, seqVal]⟩
      | forallBelow => exact ⟨by simp [refIntVal, intVal], by simp [refSeqVal, seqVal]⟩
      | forallFrom => exact ⟨by simp [refIntVal, intVal], by simp [refSeqVal, seqVal]⟩
      | disjoint => exact ⟨by simp [refIntVal, intVal], by simp [refSeqVal, seqVal]⟩
      | permutationOf => exact ⟨by simp [refIntVal, intVal], by simp [refSeqVal, seqVal]⟩
  | fuel, Expr.optResVar x, env => ⟨by simp [refIntVal, intVal], by simp [refSeqVal, seqVal]⟩
  | fuel, Expr.match_ scrut arms, env => ⟨by simp [refIntVal, intVal], by simp [refSeqVal, seqVal]⟩
  | fuel, Expr.is_ scrut variant, env => ⟨by simp [refIntVal, intVal], by simp [refSeqVal, seqVal]⟩
  -- The #181 call at fuel `0`: integer-sorted; both bottom to `0`; sequence projection `[]`.
  | 0, Expr.specCall name args, env => ⟨by simp [refIntVal, intVal], by simp [refSeqVal, seqVal]⟩
  -- The #181 call at `fuel+1` (fuel matched in the header, so the well-founded measure sees the
  -- concrete `n+1`): both resolve `name` in the same `Env.specs`; on `some fn` the args agree
  -- (`refIntValArgs_eq` at the same `n+1`, the args smaller by `sizeOf` than `specCall`) so the
  -- bound env is identical, and the body agrees (`refVal_eq n` at smaller fuel); on `none` both `0`.
  | n+1, Expr.specCall name args, env => by
      refine ⟨?_, by simp [refSeqVal, seqVal]⟩
      simp only [refIntVal, intVal]
      cases h : env.specs name with
      | none => rfl
      | some fn =>
          rw [refIntValArgs_eq (n+1) args env]
          exact (refVal_eq n fn.body
            (env.bindParams fn.params (intValArgs (n+1) args env))).1
  termination_by fuel e _ => (fuel, sizeOf e)
  decreasing_by
    all_goals simp_wf
    all_goals first
      | (apply Prod.Lex.left; omega)
      | (apply Prod.Lex.right; omega)

/-- The encoder-denoted and source-denoted spec-call arg lists coincide (#181): `refIntValArgs fuel
    args = intValArgs fuel args`, by structural recursion on the arg list (each arg agrees by
    `refVal_eq` at the same fuel — the args are smaller by `sizeOf`). The args being equal is the
    call-site soundness content (the body + registry are shared). Mutual with `refVal_eq`. -/
theorem refIntValArgs_eq : ∀ (fuel : Nat) (args : List Expr) (env : Env),
    refIntValArgs fuel args env = intValArgs fuel args env
  | _,    [],        _   => by simp [refIntValArgs, intValArgs]
  | fuel, a :: rest, env => by
      simp only [refIntValArgs, intValArgs, (refVal_eq fuel a env).1,
                 refIntValArgs_eq fuel rest env]
  termination_by fuel args _ => (fuel, sizeOf args)
  decreasing_by
    all_goals simp_wf
    all_goals first
      | (apply Prod.Lex.left; omega)
      | (apply Prod.Lex.right; omega)

/--
  **(T1) — verified-validator soundness, comparison/logical/arithmetic/cast/
  spec-context-rewrite fragment.**

  For every contract `Expr` `e` in the fragment and every environment `env`, the
  meaning of the reference encoder's output (`refDenote`, routed through the encoder's
  operator + cast-target maps, its parenthesization, the slice→`@`/`subrange` rewrite,
  and the byte-view dispatch) is logically equivalent to the source denotation
  (`denote`, the standard `S_C` meaning). Proved by structural `induction` on `e`, one
  case per inference rule (`thermite-semantics.md` REQ-2).

  Non-vacuous: `refDenote`/`refIntVal`/`refSeqVal` and `denote`/`intVal`/`seqVal` are
  defined in separate modules following different structure (the encoder's binop/cast
  maps + paren + `@`-view + byte-view dispatch vs the source relation/arithmetic/
  element/byte); the `cmp` case discharges the `encOp`/`tokRel` round-trip per operator
  and the integer-operand equality `refIntVal_eq_intVal`, which itself carries the #176
  arithmetic round-trip, the #177 cast round-trip, and the #178 `@`-view/`subrange`/
  byte-view rewrites (via `refVal_eq`), not a definitional collapse. See
  `eq_le_infidelity_*` (the `==`-vs-`<=` teeth), `cast_paren_drop_breaks_soundness` (the
  #122/#146 cast-paren teeth), and `byteview_misdispatch_breaks_soundness` (the #127
  byte-view-dispatch teeth) below.
-/
theorem ref_sound : ∀ (fuel : Nat) (e : Expr) (env : Env), refDenote fuel e env ↔ denote fuel e env
  -- `Expr` is mutually inductive (with `RangeArg`/`MatchArm`), and #181 adds a `specCall` that
  -- decreases fuel, so the recursion is well-founded on `(fuel, sizeOf e)` (the fuel is matched in
  -- the header for `specCall`, so the measure sees the concrete `n+1`/`n`-decrease). The structural
  -- predicate subterms (`logic`/`neg`/the `comb` body/the `match_` arm bodies via `ref_sound_arms`)
  -- recurse at the same fuel; a resolved spec-fn body at smaller fuel. The recursion is the
  -- well-founded induction Lean checks.
  | fuel, Expr.intLit n, env => by simp [refDenote, denote]
  | fuel, Expr.boolLit b, env => by simp [refDenote, denote]
  | fuel, Expr.var x, env => by simp [refDenote, denote]
  -- The #253 bool-var: both `refDenote`/`denote` are the identical `env.bools x = true` arm, so
  -- the soundness equation re-establishes definitionally (`Iff.rfl` after unfolding).
  | fuel, Expr.boolVar x, env => by simp [refDenote, denote]
  | fuel, Expr.cmp op a b, env => by
      -- Both sides reduce to a relation over the same operands (refIntVal = intVal at this fuel);
      -- the operator round-trip `tokRel (encOp op)` = the source relation is settled per-operator.
      cases op <;>
        simp [refDenote, denote, encOp, tokRel,
              (refVal_eq fuel a env).1, (refVal_eq fuel b env).1]
  | fuel, Expr.logic op a b, env => by
      cases op <;>
        simp [refDenote, denote, encLog, tokConn, ref_sound fuel a env, ref_sound fuel b env]
  | fuel, Expr.neg e0, env => by
      simp [refDenote, denote, ref_sound fuel e0 env]
  | fuel, Expr.arith op a b, env => by simp [refDenote, denote]
  | fuel, Expr.cast inner ty, env => by simp [refDenote, denote]
  | fuel, Expr.seqVar x, env => by simp [refDenote, denote]
  | fuel, Expr.strVar x, env => by simp [refDenote, denote]
  | fuel, Expr.idx base i, env => by simp [refDenote, denote]
  | fuel, Expr.subrange base r, env => by simp [refDenote, denote]
  | fuel, Expr.seqLen base, env => by simp [refDenote, denote]
  | fuel, Expr.byteAt base i, env => by simp [refDenote, denote]
  | fuel, Expr.comb c seq seq2 idx pred, env => by
      -- The 6 bounded-quantifier combinators (#179). Both sides expand to the same frozen
      -- quantifier form; they differ only in the per-arg-kind threading. Establish those agree, then
      -- the quantifier forms are equivalent by congruence.
      have hs : refSeqVal fuel seq env = seqVal fuel seq env := (refVal_eq fuel seq env).2
      have hp : ∀ v : Int,
          (match pred with
            | some (Pred.mk bound body) => refDenote fuel body (env.bindInt bound v)
            | none => True) ↔
          (match pred with
            | some (Pred.mk bound body) => denote fuel body (env.bindInt bound v)
            | none => True) := by
        intro v
        cases pred with
        | none => exact Iff.rfl
        | some pr => cases pr with
          | mk bound body => exact ref_sound fuel body (env.bindInt bound v)
      cases c with
      | forallIn =>
          simp only [refDenote, denote, hs]
          exact forall_congr' (fun i => imp_congr_right (fun _ => hp _))
      | existsIn =>
          simp only [refDenote, denote, hs]
          exact exists_congr (fun i => and_congr_right (fun _ => hp _))
      | sorted =>
          simp only [refDenote, denote, hs]
      | forallBelow =>
          cases idx with
          | none =>
              simp only [refDenote, denote, hs]
              exact forall_congr' (fun i => imp_congr_right (fun _ => hp _))
          | some e =>
              simp only [refDenote, denote, hs, (refVal_eq fuel e env).1]
              exact forall_congr' (fun i => imp_congr_right (fun _ => hp _))
      | forallFrom =>
          cases idx with
          | none =>
              simp only [refDenote, denote, hs]
              exact forall_congr' (fun i => imp_congr_right (fun _ => hp _))
          | some e =>
              simp only [refDenote, denote, hs, (refVal_eq fuel e env).1]
              exact forall_congr' (fun i => imp_congr_right (fun _ => hp _))
      | disjoint =>
          cases seq2 with
          | none => simp only [refDenote, denote, hs]
          | some e => simp only [refDenote, denote, hs, (refVal_eq fuel e env).2]
      -- The #182 `permutation_of(a, b)` (`Prop`-sorted, multiset equality `permEq`). Both sides reduce
      -- to `permEq` over the two `@`-views; the views agree (`hs` for `a`, `(refVal_eq …).2` for `b`),
      -- so the two `permEq`s are definitionally the same `Prop` once the views are rewritten.
      | permutationOf =>
          cases seq2 with
          | none => simp only [refDenote, denote, hs]
          | some e => simp only [refDenote, denote, hs, (refVal_eq fuel e env).2]
      -- The #182 `count_where` as a top-level predicate: value-sorted, so both `refDenote`/`denote`
      -- bottom to the shared `True` (it is read on the `intVal` side, settled by `refVal_eq`). `Iff.rfl`.
      | countWhere => simp only [refDenote, denote]
  | fuel, Expr.optResVar x, env => by simp [refDenote, denote]
  -- The match-in-ens form (#180). Threads `ref_sound_arms` at the same fuel.
  | fuel, Expr.match_ scrut arms, env => by
      simp only [refDenote, denote]
      exact ref_sound_arms fuel (scrutVal scrut env) arms env
  | fuel, Expr.is_ scrut variant, env => by simp only [refDenote, denote]
  -- The #181 spec-fn call at fuel `0`: both `refDenote`/`denote` bottom to the shared default
  -- `True` → `Iff.rfl`.
  | 0, Expr.specCall name args, env => by simp only [refDenote, denote]
  -- The #181 spec-fn call at `fuel+1` (fuel matched in the header — the measure sees the concrete
  -- `n+1`/`n`): both resolve `name` in the same `Env.specs`: on `none` both are `True`; on `some fn`
  -- both denote the same body — at the smaller fuel `n` (the well-founded decrease) and in the same
  -- bound env (the args agree by `refIntValArgs_eq`, so `bindParams` produces the identical env) —
  -- settled by the recursive `ref_sound n fn.body`. This is the generic call-site theorem: args
  -- sound (the IH) + the same registry resolves the same body (the smaller-fuel IH). The fuel is
  -- shared with the source, so T1 is fuel-uniform (it holds at every fuel, not a fuel-cap dodge).
  | n+1, Expr.specCall name args, env => by
      simp only [refDenote, denote]
      cases h : env.specs name with
      | none => exact Iff.rfl
      | some fn =>
          rw [refIntValArgs_eq (n+1) args env]
          exact ref_sound n fn.body
            (env.bindParams fn.params (intValArgs (n+1) args env))
  termination_by fuel e _ => (fuel, sizeOf e)
  decreasing_by
    all_goals simp_wf
    all_goals first
      | (apply Prod.Lex.left; omega)
      | (apply Prod.Lex.right; omega)

/-- The match-arm soundness (#180), mutual with `ref_sound`, fuel-indexed (#181): the encoder's arm
    walk `refDenoteArms` is equivalent to the source `denoteArms` at the same scrutinee value + fuel,
    by structural recursion on the arm list. Each step is either the selected arm's body (`ref_sound`
    at the same fuel on the body) or the recursive tail. The selection condition + payload binding
    are shared, so the only content is the per-body `ref_sound`. -/
theorem ref_sound_arms : ∀ (fuel : Nat) (scrut : OptResVal) (arms : List MatchArm) (env : Env),
    refDenoteArms fuel scrut arms env ↔ denoteArms fuel scrut arms env
  | fuel, scrut, arms, env => by
    cases arms with
    | nil => rw [refDenoteArms.eq_def, denoteArms.eq_def]
    | cons arm rest =>
        cases arm with
        | mk variant binder body =>
            rw [refDenoteArms.eq_def, denoteArms.eq_def]
            by_cases h : scrut.variant = variant
            · simp only [h, if_true]
              cases binder with
              | none => exact ref_sound fuel body env
              | some x => exact ref_sound fuel body (env.bindInt x scrut.payload)
            · simp only [h, if_false]
              exact ref_sound_arms fuel scrut rest env
  termination_by fuel _ arms _ => (fuel, sizeOf arms)
  decreasing_by
    all_goals simp_wf
    all_goals first
      | (apply Prod.Lex.left; omega)
      | (apply Prod.Lex.right; omega)
end

/-- The integer-term meanings coincide (the projection of `refVal_eq` used by the teeth/positive
    lemmas below). -/
theorem refIntVal_eq_intVal (fuel : Nat) (e : Expr) (env : Env) :
    refIntVal fuel e env = intVal fuel e env := (refVal_eq fuel e env).1

/-- The sequence-term meanings coincide (the `@`-view/`subrange` projection of `refVal_eq`). -/
theorem refSeqVal_eq_seqVal (fuel : Nat) (e : Expr) (env : Env) :
    refSeqVal fuel e env = seqVal fuel e env := (refVal_eq fuel e env).2

/-- A convenient `Prop`-equality corollary (propositional extensionality) — the
    `⟦R(P)⟧ = ⟦P⟧_S` form (T2's transitivity step composes on this equality, AC-3). -/
theorem ref_sound_eq (fuel : Nat) (e : Expr) (env : Env) : refDenote fuel e env = denote fuel e env :=
  propext (ref_sound fuel e env)

/-! ## Negative sanity lemma 1 — the comparison teeth (`==` ≠ `<=`)

  The #170 teeth, retained: an encoder that mapped `Eq → "<="` (the
  `==`-vs-`<=` infidelity) would not satisfy soundness at a concrete `env`. -/

/-- A faulty encoder operator map: `Eq` mis-mapped to the `<=` token (the
    infidelity), every other operator faithful. Mirrors a hypothetical
    `binop_str` bug `Eq => "<="`. -/
def encOpFaulty : CmpOp → VerusCmpTok
  | CmpOp.eq => VerusCmpTok.leTok   -- the bug: `==` emitted as `<=`
  | CmpOp.ne => VerusCmpTok.neTok
  | CmpOp.lt => VerusCmpTok.ltTok
  | CmpOp.le => VerusCmpTok.leTok
  | CmpOp.gt => VerusCmpTok.gtTok
  | CmpOp.ge => VerusCmpTok.geTok

/-- `refDenote` with the faulty `Eq→<=` map on a comparison (fuel-indexed; the comparison
    operands are non-spec-fn terms so the fuel is immaterial — `0` suffices). -/
def refDenoteFaultyCmp (fuel : Nat) (op : CmpOp) (a b : Expr) (env : Env) : Prop :=
  tokRel (encOpFaulty op) (refIntVal fuel a env) (refIntVal fuel b env)

/-- A concrete environment: integer names `a := 1`, `b := 2`, `n := -1` (everything
    else `0`); sequence name `s := [10, 20, 30]` (a `String`'s bytes; everything else
    the empty sequence) — the witness sequence for the #127 byte-view-dispatch teeth
    (its bytes differ at adjacent indices, so a wrong index / wrong method is observable). -/
def envAB : Env :=
  { ints := fun s => if s = "a" then 1 else if s = "b" then 2
                     else if s = "n" then -1 else 0
    seqs := fun s => if s = "s" then [10, 20, 30] else []
    -- The #180 option/result binding: `result := Some 7` (the C7 match/is scrutinee witness:
    -- a `Some`-valued result carrying the integer payload 7; everything else `None`).
    optres := fun s => if s = "result" then OptResVal.some_ 7 else OptResVal.none_
    -- The spec-fn registry slot (#181): `envAB` carries no spec fn (the comparison/cast/byte-view/
    -- combinator/match teeth do not call one). The #181 spec-fn teeth use `envSpec` below.
    specs := fun _ => none }

/-- **Teeth (negative sanity, the `==`-vs-`<=` case, #170).** At `envAB` the faulty
    `Eq→<=` encoding of `a == b` is true (`1 ≤ 2`) while the source meaning of
    `a == b` is false (`1 ≠ 2`), so the faulty encoder does not satisfy the
    soundness equation. -/
theorem eq_le_infidelity_breaks_soundness :
    ¬ (refDenoteFaultyCmp 0 CmpOp.eq (Expr.var "a") (Expr.var "b") envAB
        ↔ denote 0 (Expr.cmp CmpOp.eq (Expr.var "a") (Expr.var "b")) envAB) := by
  simp [refDenoteFaultyCmp, encOpFaulty, tokRel, denote, intVal, refIntVal, envAB]

/-! ## Negative sanity lemma 2 — the #122/#146 cast-paren teeth (the retired class)

  The dispatch's explicit requirement: demonstrate that an encoder that drops the
  cast paren — emitting `(n - 1) as nat` as the bare `n - 1 as nat`, which Verus/Rust
  parse as `n - (1 as nat)` (a cast binds tighter than `-`) — does not satisfy
  soundness at a concrete env where the precedence changes the value. This is the
  #122 `divergence_cast_paren` / #146 cast-mis-parse bug, now proven to break T1 on
  the contract side: the faithful `encode_cast` paren (`refIntVal`'s `cast` arm casts
  the whole inner) is what makes `ref_sound` hold for casts.

  The source clause: `(n - 1) as nat`, i.e.
    `Expr.cast (Expr.arith ArithOp.sub (Expr.var "n") (Expr.intLit 1)) CastTy.nat`.
  Its faithful denotation casts the whole subtraction: `((n - 1) : Int).toNat`.
  The paren-dropped encoder instead binds the cast to only the rightmost atom `1`,
  yielding `n - (1 as nat)` = `n - (1 : Int)` (no cast on `n`, the `-` outside the
  cast). At `n = -1` these differ: faithful `(-1 - 1).toNat = (-2).toNat = 0`;
  paren-dropped `-1 - 1 = -2`. `0 ≠ -2`. -/

/-- The faithful cast denotation of `(n - 1) as nat` — what the real `encode_cast`
    (its `({inner}) as nat` paren) produces: the cast applies to the whole inner. -/
noncomputable def castInnerFaithful (env : Env) : Int :=
  refIntVal 0 (Expr.cast (Expr.arith ArithOp.sub (Expr.var "n") (Expr.intLit 1)) CastTy.nat) env

/-- The paren-dropped cast denotation — the #122 bug. The buggy encoder emits the
    string `n - 1 as nat`, which re-parses as `n - (1 as nat)`: the cast binds only
    the atom `1`, and the subtraction sits outside the cast. We model that re-parsed
    AST and take its faithful `refIntVal` (the bug is the encoder's missing paren, not
    a second meaning function — the re-parse is what the dropped paren denotes). -/
noncomputable def castInnerParenDropped (env : Env) : Int :=
  refIntVal 0
    (Expr.arith ArithOp.sub (Expr.var "n") (Expr.cast (Expr.intLit 1) CastTy.nat)) env

/-- **Teeth (negative sanity, the #122/#146 cast-paren case).** At `envAB` (`n := -1`)
    the faithful `(n - 1) as nat` denotes `0` while the paren-dropped `n - 1 as nat`
    (re-parsed `n - (1 as nat)`) denotes `-2` — they disagree, so a paren-dropping
    encoder does not satisfy the soundness equation `refDenote = denote` for this
    clause. This is the Lean-level witness that `ref_sound`'s `cast` case pins the
    encoder's parenthesization: had `encode_cast` dropped the inner paren, the proof
    of `refIntVal_eq_intVal` (hence `ref_sound`) would have failed here. -/
theorem cast_paren_drop_breaks_soundness :
    castInnerFaithful envAB ≠ castInnerParenDropped envAB := by
  -- faithful: castDenote nat (-1 - 1) = (-2).toNat = 0
  -- dropped:  (-1) - castDenote nat 1 = -1 - 1 = -2
  simp [castInnerFaithful, castInnerParenDropped, refIntVal, tokCast, tokArith,
        encCast, encArith, castDenote, arithDenote, envAB]

/-- The faithful counterpart, for contrast: with the real `refIntVal` (the
    parenthesized cast) the `(n - 1) as nat` clause is sound — it equals the source
    `intVal` (the whole-inner cast), by `refIntVal_eq_intVal`. Confirms the teeth bite
    only the paren-drop, not the faithful encoder. -/
theorem cast_faithful_intval_matches_source :
    refIntVal 0 (Expr.cast (Expr.arith ArithOp.sub (Expr.var "n") (Expr.intLit 1)) CastTy.nat) envAB
      = intVal 0 (Expr.cast (Expr.arith ArithOp.sub (Expr.var "n") (Expr.intLit 1)) CastTy.nat) envAB :=
  refIntVal_eq_intVal _ _ _

/-- The faithful counterpart for the comparison teeth, retained from #170: with the
    real `encOp` the `a == b` clause is sound (both `1 = 2`, false), by `ref_sound`. -/
theorem eq_faithful_is_sound :
    refDenote 0 (Expr.cmp CmpOp.eq (Expr.var "a") (Expr.var "b")) envAB
      ↔ denote 0 (Expr.cmp CmpOp.eq (Expr.var "a") (Expr.var "b")) envAB :=
  ref_sound _ _ _

/-! ## Negative sanity lemma 3 — the #127 byte-view-dispatch teeth (the retired class)

  The dispatch's explicit requirement (the #178 point): demonstrate that an encoder
  that mis-dispatches the byte-view — the #127 name-collision bug, where the encoder
  picks the wrong byte-view spec fn — does not satisfy soundness at a concrete sequence
  env. Two faulty dispatches, both real instances of the #127 class:

    (A) Wrong index. The faithful `s.byte_at(0)` → `s.spec_byte_at(0)` reads byte 0.
        A buggy encoder emitting `s.spec_byte_at(0 + 1)` (an off-by-one index) reads
        byte 1. At `s := [10, 20, 30]` these are `10` vs `20` — they disagree.
    (B) Wrong receiver-method. The faithful `s.byte_at(i)` dispatches to the i-th-byte
        spec fn (`encByteAt = specByteAt`). A buggy encoder that mis-dispatched
        `byte_at` to the length spec fn (`specLen` — the #127 name collision: picking
        `spec_len` for a `byte_at` call) reads the length `3` instead of byte 0 (`10`).
        `10 ≠ 3` — they disagree.

  This is the #127 (`divergence_byteview_name_collision`) class, now proven to break
  T1 on the contract side: the faithful `encByteAt`/`encLen` dispatch (`refIntVal`'s
  `idx`/`byteAt`/`seqLen` arms, the `byteView_encByteAt`/`byteView_encLen` round-trips)
  is what makes `ref_sound` hold for the byte-view rewrites. -/

/-- The faithful byte-view of `s.byte_at(0)` — what the real `encode_string_byteview`
    (its `spec_byte_at(0)` dispatch) produces: the 0-th byte. -/
noncomputable def byteAtFaithful (env : Env) : Int :=
  refIntVal 0 (Expr.byteAt (Expr.strVar "s") (Expr.intLit 0)) env

/-- The #127 wrong-index bug (instance A): a buggy encoder emits `s.spec_byte_at(0 + 1)`
    for the source `s.byte_at(0)` — an off-by-one byte-view index (the misdispatch reads
    the wrong byte). Modelled as the byte-view at index `0 + 1` (the dispatch is the
    faithful `encByteAt`, but the index is wrong — the #127 misdispatch shape). -/
noncomputable def byteAtWrongIndex (env : Env) : Int :=
  byteView encByteAt (refSeqVal 0 (Expr.strVar "s") env)
    (refIntVal 0 (Expr.arith ArithOp.add (Expr.intLit 0) (Expr.intLit 1)) env)

/-- The #127 wrong-method bug (instance B): a buggy encoder mis-dispatches the
    `byte_at` call to the length spec fn (`encLen` = `spec_len`) — the name-collision
    misdispatch. It reads the sequence length where the source reads a byte. -/
noncomputable def byteAtWrongMethod (env : Env) : Int :=
  byteView encLen (refSeqVal 0 (Expr.strVar "s") env)
    (refIntVal 0 (Expr.intLit 0) env)

/-- **Teeth (negative sanity, the #127 wrong-index byte-view-dispatch case).** At
    `envAB` (`s := [10, 20, 30]`) the faithful `s.byte_at(0)` denotes byte `10` while
    the off-by-one `s.spec_byte_at(0 + 1)` denotes byte `20` — they disagree, so a
    wrong-index byte-view dispatch does not satisfy the soundness equation. -/
theorem byteview_wrong_index_breaks_soundness :
    byteAtFaithful envAB ≠ byteAtWrongIndex envAB := by
  -- faithful: seqIdx [10,20,30] 0 = 10 ; wrong-index: seqIdx [10,20,30] 1 = 20
  simp [byteAtFaithful, byteAtWrongIndex, refIntVal, refSeqVal, byteView, seqIdx,
        encByteAt, encArith, tokArith, arithDenote, envAB]

/-- **Teeth (negative sanity, the #127 wrong-receiver-method byte-view-dispatch
    case).** At `envAB` (`s := [10, 20, 30]`) the faithful `s.byte_at(0)` denotes byte
    `10` while the misdispatched `s.spec_len()` denotes the length `3` — they disagree,
    so a wrong-receiver-method byte-view dispatch (the #127 name-collision) does not
    satisfy the soundness equation. This is the proven retirement of the #127 class on
    the contract side: the encoder's byte-view dispatch choice is what `ref_sound` pins. -/
theorem byteview_misdispatch_breaks_soundness :
    byteAtFaithful envAB ≠ byteAtWrongMethod envAB := by
  -- faithful: seqIdx [10,20,30] 0 = 10 ; wrong-method: ([10,20,30].length : Int) = 3
  simp [byteAtFaithful, byteAtWrongMethod, refIntVal, refSeqVal, byteView, seqIdx,
        encByteAt, encLen, envAB]

/-- The faithful counterpart, for contrast: with the real byte-view dispatch the
    `s.byte_at(0)` clause is sound — its encoder meaning equals the source `intVal`
    (the 0-th byte), by `refIntVal_eq_intVal`. Confirms the teeth bite only the
    misdispatch, not the faithful encoder. -/
theorem byteat_faithful_intval_matches_source :
    refIntVal 0 (Expr.byteAt (Expr.strVar "s") (Expr.intLit 0)) envAB
      = intVal 0 (Expr.byteAt (Expr.strVar "s") (Expr.intLit 0)) envAB :=
  refIntVal_eq_intVal _ _ _

/-- A faithful positive witness for the `@`-view + index + subrange rewrites (#178):
    `(&xs[..2])[1]` — the prefix-then-index — has the encoder meaning equal to the
    source (the 1-st element of the 2-element prefix), by `refIntVal_eq_intVal`. This
    exercises `seqVar`→`@`, `subrange`→`seqSub`, and `idx`→`seqIdx` composed, all proven
    denotation-preserving. -/
theorem subrange_index_faithful_matches_source :
    refIntVal 0
        (Expr.idx (Expr.subrange (Expr.seqVar "s") (RangeArg.rangeTo (Expr.intLit 2)))
          (Expr.intLit 1)) envAB
      = intVal 0
        (Expr.idx (Expr.subrange (Expr.seqVar "s") (RangeArg.rangeTo (Expr.intLit 2)))
          (Expr.intLit 1)) envAB :=
  refIntVal_eq_intVal _ _ _

/-! ## Negative sanity lemma 4 — the wrong-combinator teeth (#179)

  The dispatch's explicit requirement (a): demonstrate that an encoder that emitted the
  wrong combinator — `forall_in` (a bounded `∀`) lowered as `exists_in` (a bounded `∃`)
  — does not satisfy soundness at a concrete sequence. The two quantifier forms differ
  (`∀ i, .. → p(s[i])` vs `∃ i, .. ∧ p(s[i])`) precisely when some element satisfies the
  predicate and some does not. This is the combinator analogue of the `==`-vs-`<=` teeth:
  the encoder's choice of which frozen `verus_l3` quantifier (`encode_call`'s
  `lookup(name)` dispatch, referencing the right combinator) is load-bearing.

  Source clause: `forall_in(s, |x| x ≤ 15)`, i.e.
    `Expr.comb forallIn (strVar "s") none none (some (Pred.mk "x" (x ≤ 15)))`.
  At `envAB` (`s := [10, 20, 30]`) the source `∀ i, 0≤i<3 → s[i] ≤ 15` is false (`20 > 15`),
  while the wrong `exists_in` form `∃ i, 0≤i<3 ∧ s[i] ≤ 15` is true (`10 ≤ 15`). False vs
  true — they disagree, so an encoder that referenced the wrong combinator does not satisfy
  the soundness equation. -/

/-- The flat predicate body `x ≤ 15` (the #179 combinator predicate slot's `body`). -/
def predLe15Body : Expr := Expr.cmp CmpOp.le (Expr.var "x") (Expr.intLit 15)

/-- The flat predicate closure `|x| x ≤ 15` (the #179 combinator predicate slot). -/
def predLe15 : Pred := Pred.mk "x" predLe15Body

/-- The source `forall_in(s, |x| x ≤ 15)` clause. -/
def forallInClause : Expr :=
  Expr.comb CombName.forallIn (Expr.strVar "s") none none (some predLe15)

/-- The wrong-combinator bug: the encoder emits `exists_in` where the source is
    `forall_in` (a bounded `∃` for a bounded `∀` — `encode_call` referencing the wrong
    `lookup(name)` form). Modelled as `refDenote` of the `exists_in` combinator over the
    same slice + predicate. -/
def existsInWrong : Expr :=
  Expr.comb CombName.existsIn (Expr.strVar "s") none none (some predLe15)

/-- **Teeth (negative sanity, the wrong-combinator case, #179).** At `envAB`
    (`s := [10, 20, 30]`) the wrong `exists_in` encoding (`∃ i, 0≤i<3 ∧ s[i] ≤ 15`, true)
    is not equivalent to the source `forall_in` meaning (`∀ i, 0≤i<3 → s[i] ≤ 15`, false),
    so an encoder that referenced the wrong combinator does not satisfy soundness. -/
theorem wrong_combinator_breaks_soundness :
    ¬ (refDenote 0 existsInWrong envAB ↔ denote 0 forallInClause envAB) := by
  -- refDenote existsInWrong = ∃ i, 0≤i<3 ∧ [10,20,30][i] ≤ 15  (true, witness i = 0)
  -- denote forallInClause   = ∀ i, 0≤i<3 → [10,20,30][i] ≤ 15  (false, counter i = 1)
  intro h
  have hExists : refDenote 0 existsInWrong envAB := by
    rw [existsInWrong, refDenote.eq_def]
    simp only [refSeqVal, predLe15]
    refine ⟨0, ⟨by decide, by simp [envAB]⟩, ?_⟩
    simp [predLe15Body, refDenote, refIntVal, encOp, tokRel,
          Env.bindInt, seqIdx, envAB]
  have hForall := h.mp hExists
  rw [forallInClause, denote.eq_def] at hForall
  simp only [seqVal, predLe15] at hForall
  have hAt1 : (seqIdx (envAB.seqs "s") 1) ≤ 15 := by
    have := hForall 1 ⟨by decide, by simp [envAB]⟩
    simpa [predLe15Body, denote, intVal, Env.bindInt, seqVal] using this
  simp [seqIdx, envAB] at hAt1

/-! ## Negative sanity lemma 5 — the #145 arg-kind teeth (the retired class)

  The dispatch's explicit requirement (b): demonstrate the #145 (`divergence_index_
  combinator`) bug — `forall_below`/`forall_from`'s `ArgKind::Index` bound `n` (a scalar
  `int`) encoded as a slice `@`-view instead of a scalar. `encode_combinator_arg`'s `#145`
  fix dispatches `ArgKind::Index → encode_index_value` (the scalar `<n> as int`), not
  `encode_slice_arg` (the `@`-view). A buggy encoder that slice-`@`-viewed the index would
  produce `n@` (a Verus type error in production; on the contract side, a different
  quantifier bound — the length of `n`'s view rather than the scalar `n`).

  Source clause: `forall_below(s, n, |x| x ≤ 15)` with the index `n` a scalar. We model the
  #145 bug as the same `forall_below` form but with the quantifier bound taken from the
  slice-`@`-view length of the index arg (`(refSeqVal n).length`) instead of the scalar
  `intVal n`. At an env where the scalar `n` (= 1) differs from the slice-view length
  (`n@` bound to `[10,20,30]`, length 3) and `s := [10,20,30]` with `|x| x ≤ 15`:
    - faithful bound `n = 1`: `∀ i, 0≤i<1 ∧ i<3 → s[i] ≤ 15` — only `i=0` (`10 ≤ 15`) → true.
    - #145-buggy bound `= 3`: `∀ i, 0≤i<3 ∧ i<3 → s[i] ≤ 15` — `i=1` (`20 ≤ 15`) → false.
  True vs false — they disagree, so slice-`@`-viewing the Index arg breaks T1. This is the
  proven retirement of the #145 arg-kind class on the contract side: `encode_combinator_arg`
  threading `ArgKind::Index` as a scalar (not a `@`-view) is what `ref_sound`'s
  `comb` case pins (its `forallBelow` arm uses `refIntVal_eq_intVal` on the scalar index). -/

/-- A concrete env for the #145 teeth: the index var `n` is the scalar `1`, while `n`'s
    slice `@`-view (the buggy reading) is `[10, 20, 30]` (length `3`); `s := [10, 20, 30]`.
    The scalar value (1) and the view-length (3) differ, so a slice-`@`-viewed index is
    observable. -/
def envIdx : Env :=
  { ints := fun nm => if nm = "n" then 1 else 0
    seqs := fun nm => if nm = "s" ∨ nm = "n" then [10, 20, 30] else []
    optres := fun _ => OptResVal.none_
    specs := fun _ => none }

/-- The faithful `forall_below(s, n, |x| x ≤ 15)` source meaning — the `n` bound is the
    scalar `intVal n` (= 1), as `encode_index_value` (the #145 fix) threads it. -/
def forallBelowFaithful : Prop :=
  denote 0
    (Expr.comb CombName.forallBelow (Expr.strVar "s")
      none (some (Expr.var "n")) (some predLe15)) envIdx

/-- The #145 arg-kind bug: the encoder slice-`@`-views the Index arg `n` instead of
    threading it as a scalar — the quantifier bound becomes `(n@).length` (= 3), not the
    scalar `n` (= 1). Modelled as the `forall_below` quantifier with the bound taken from
    the slice-`@`-view length of the index arg (`refSeqVal (seqVar "n")` — the encoder
    wrongly dispatching `ArgKind::Index` through `encode_slice_arg`). -/
def forallBelowIndexSliceViewed : Prop :=
  let s := refSeqVal 0 (Expr.strVar "s") envIdx
  let nBad := ((refSeqVal 0 (Expr.seqVar "n") envIdx).length : Int)  -- the #145 `n@.len()`
  ∀ i : Int, (0 ≤ i ∧ i < nBad ∧ i < (s.length : Int)) →
    denote 0 predLe15Body (envIdx.bindInt "x" (seqIdx s i))

/-- **Teeth (negative sanity, the #145 arg-kind case).** At `envIdx` (`n` scalar `= 1`,
    `n@` = `[10,20,30]` length `3`, `s := [10,20,30]`) the faithful `forall_below` (scalar
    bound `1`) is true (`10 ≤ 15`) while the #145-buggy slice-`@`-viewed-index form (bound
    `3`) is false (`20 ≤ 15` fails at `i = 1`) — they disagree, so slice-`@`-viewing the
    `ArgKind::Index` bound breaks T1. The faithful `encode_index_value` scalar threading is
    what `ref_sound`'s `comb`/`forallBelow` arm pins. -/
theorem index_argkind_slice_view_breaks_soundness :
    forallBelowFaithful ≠ forallBelowIndexSliceViewed := by
  intro h
  -- forallBelowFaithful is true; forallBelowIndexSliceViewed is false → contradiction.
  have hF : forallBelowFaithful := by
    rw [forallBelowFaithful, denote.eq_def]
    simp only [seqVal, predLe15]
    intro i hi
    -- bound n = 1, so 0 ≤ i < 1 forces i = 0; s[0] = 10 ≤ 15.
    obtain ⟨hi0, hi1, _⟩ := hi
    have hi0eq : i = 0 := by
      simp only [intVal, envIdx] at hi1; omega
    subst hi0eq
    simp [predLe15Body, denote, intVal, Env.bindInt, seqIdx, envIdx]
  rw [h] at hF
  -- the buggy form (bound 3) fails at i = 1 (s[1] = 20 > 15).
  rw [forallBelowIndexSliceViewed] at hF
  simp only [refSeqVal, envIdx] at hF
  have hBad := hF 1 ⟨by decide, by decide, by decide⟩
  simp [predLe15Body, denote, intVal, Env.bindInt, seqIdx] at hBad

/-- The faithful positive counterpart, for contrast: with the real combinator dispatch +
    the scalar index threading the `forall_below(s, n, |x| x ≤ 15)` clause is sound — its
    encoder meaning is equivalent to the source, by `ref_sound`. Confirms the #179/#145
    teeth bite only the wrong-combinator / slice-viewed-index, not the faithful encoder. -/
theorem forall_below_faithful_is_sound :
    refDenote 0
        (Expr.comb CombName.forallBelow (Expr.strVar "s")
          none (some (Expr.var "n")) (some predLe15)) envIdx
      ↔ denote 0
        (Expr.comb CombName.forallBelow (Expr.strVar "s")
          none (some (Expr.var "n")) (some predLe15)) envIdx :=
  ref_sound _ _ _

/-! ## Negative sanity lemma 6 — the #180 match-arm-swap teeth (the C7 match-in-ens class)

  The dispatch's explicit requirement (a): demonstrate that an encoder that swapped the match
  arm bodies (the `Some`/`None` bodies exchanged — `encode_match` emitting each arm's body
  under the wrong pattern) does not satisfy soundness at a concrete `OptResVal`. This is the
  match-in-ens analogue of the `==`-vs-`<=` / wrong-combinator teeth: which arm body goes under
  which pattern (`encode_match` pairing `encode_pattern(arm.pattern)` with `encode(arm.body)`) is
  load-bearing.

  Source clause: `match result { Some(v) => v == 7, None => false }`, i.e.
    `Expr.match_ (optResVar "result")
        [MatchArm.mk Some (some "v") (v == 7), MatchArm.mk None none false]`.
  At `envAB` (`result := Some 7`) the source selects the `Some` arm → `7 == 7` → true.
  The swapped encoder emits `match result { Some(v) => false, None => v == 7 }` (the bodies
  exchanged). At `result := Some 7` the swapped clause selects the `Some` arm → `false` → false.
  True vs false — they disagree, so an arm-body-swapping encoder does not satisfy soundness. -/

/-- The `Some`-arm body `v == 7` (the payload test the C7 match projects). -/
def someBodyEq7 : Expr := Expr.cmp CmpOp.eq (Expr.var "v") (Expr.intLit 7)

/-- The source `match result { Some(v) => v == 7, None => false }` clause (#180). -/
def matchSomeClause : Expr :=
  Expr.match_ (Expr.optResVar "result")
    [MatchArm.mk Variant.some_ (some "v") someBodyEq7,
     MatchArm.mk Variant.none_ none (Expr.boolLit false)]

/-- The arm-swap bug: the encoder emits the `Some`/`None` arm bodies exchanged — `Some(v) => false,
    None => v == 7` — a real `encode_match` infidelity (pairing each body with the wrong pattern).
    Modelled as `refDenote` of the swapped-arm `match_` over the same scrutinee. -/
def matchArmSwapped : Expr :=
  Expr.match_ (Expr.optResVar "result")
    [MatchArm.mk Variant.some_ (some "v") (Expr.boolLit false),
     MatchArm.mk Variant.none_ none someBodyEq7]

/-- **Teeth (negative sanity, the #180 match-arm-swap case).** At `envAB` (`result := Some 7`)
    the source `match result { Some(v) => v == 7, None => false }` is true (the `Some` arm,
    `7 == 7`) while the arm-swapped encoding `match result { Some(v) => false, None => v == 7 }`
    is false (the `Some` arm, `false`) — they disagree, so an arm-body-swapping encoder does not
    satisfy the soundness equation `refDenote = denote` for this clause. This is the Lean-level
    witness that `ref_sound`'s `match_` case pins the encoder's pattern↔body pairing. -/
theorem match_arm_swap_breaks_soundness :
    ¬ (refDenote 0 matchArmSwapped envAB ↔ denote 0 matchSomeClause envAB) := by
  -- denote matchSomeClause   = (Some 7 selects Some arm) → 7 = 7 → true
  -- refDenote matchArmSwapped = (Some 7 selects Some arm) → false
  simp [matchSomeClause, matchArmSwapped, someBodyEq7, refDenote, denote,
        refDenoteArms, denoteArms, scrutVal, OptResVal.variant, OptResVal.payload,
        Env.bindInt, intVal, envAB]

/-- The faithful positive counterpart, for contrast: with the real `encode_match` (each body under
    its own pattern) the `match result { Some(v) => v == 7, None => false }` clause is sound — its
    encoder meaning is equivalent to the source, by `ref_sound`. Confirms the teeth bite only the
    arm-swap, not the faithful encoder. -/
theorem match_faithful_is_sound :
    refDenote 0 matchSomeClause envAB ↔ denote 0 matchSomeClause envAB :=
  ref_sound _ _ _

/-! ## Negative sanity lemma 7 — the #180 wrong-`is`-variant teeth (the C7 `is` class)

  The dispatch's explicit requirement (b): demonstrate that an encoder that emitted the wrong
  `is`-variant — `result is Some` lowered as `result is None` (`ref_encode.rs`'s `Expr::Is` arm
  emitting the wrong `variant.join("::")`) — does not satisfy soundness at a concrete `OptResVal`.
  Which variant the discriminant tests is load-bearing.

  Source clause: `result is Some`, i.e. `Expr.is_ (optResVar "result") Variant.some_`.
  At `envAB` (`result := Some 7`) the source `is Some` is true; the wrong `result is None` is
  false. True vs false — they disagree, so an encoder that tested the wrong variant does not
  satisfy soundness. -/

/-- The source `result is Some` clause (#180). -/
def isSomeClause : Expr := Expr.is_ (Expr.optResVar "result") Variant.some_

/-- The wrong-`is`-variant bug: the encoder tests `is None` where the source tests `is Some`
    (`Expr::Is` emitting the wrong variant). Modelled as `refDenote` of the `is None` test over
    the same scrutinee. -/
def isNoneWrong : Expr := Expr.is_ (Expr.optResVar "result") Variant.none_

/-- **Teeth (negative sanity, the #180 wrong-`is`-variant case).** At `envAB` (`result := Some 7`)
    the wrong `result is None` encoding (false) is not equivalent to the source `result is Some`
    meaning (true), so an encoder that tested the wrong variant does not satisfy soundness. This is
    the Lean-level witness that `ref_sound`'s `is_` case pins the encoder's variant choice. -/
theorem is_wrong_variant_breaks_soundness :
    ¬ (refDenote 0 isNoneWrong envAB ↔ denote 0 isSomeClause envAB) := by
  -- refDenote isNoneWrong = (Some 7).isVariant None = false ; denote isSomeClause = (Some 7).isVariant Some = true
  simp [isSomeClause, isNoneWrong, refDenote, denote, scrutVal,
        OptResVal.isVariant, OptResVal.variant, envAB]

/-- The faithful positive counterpart, for contrast: with the real `is`-variant the `result is
    Some` clause is sound (both `(Some 7).isVariant Some = true`), by `ref_sound`. Confirms the
    teeth bite only the wrong variant, not the faithful encoder. -/
theorem is_faithful_is_sound :
    refDenote 0 isSomeClause envAB ↔ denote 0 isSomeClause envAB :=
  ref_sound _ _ _

/-- A faithful positive witness for the Result form (#180): `match result { Ok(v) => v == 7,
    Err(e) => e == 0 }` — the `Ok`/`Err` payload projection — has the encoder meaning equal to the
    source, by `ref_sound`. Exercises the `Ok`/`Err` variant + payload-binding path (the Result
    half of the C7 fragment), confirming both Option and Result are covered. -/
theorem match_result_faithful_is_sound :
    refDenote 0
        (Expr.match_ (Expr.optResVar "result")
          [MatchArm.mk Variant.ok (some "v") (Expr.cmp CmpOp.eq (Expr.var "v") (Expr.intLit 7)),
           MatchArm.mk Variant.err (some "e") (Expr.cmp CmpOp.eq (Expr.var "e") (Expr.intLit 0))])
        envAB
      ↔ denote 0
        (Expr.match_ (Expr.optResVar "result")
          [MatchArm.mk Variant.ok (some "v") (Expr.cmp CmpOp.eq (Expr.var "v") (Expr.intLit 7)),
           MatchArm.mk Variant.err (some "e") (Expr.cmp CmpOp.eq (Expr.var "e") (Expr.intLit 0))])
        envAB :=
  ref_sound _ _ _

/-! ## Negative sanity lemma 8 — the #181 wrong-arg-order teeth (the spec-fn-call class)

  The dispatch's explicit requirement (a): demonstrate that an encoder that emitted a spec-fn call's
  args in the wrong order — `foo(a, b)` lowered as `foo(b, a)` for a non-commutative body — does not
  satisfy soundness. The spec-fn-call analogue of the `==`-vs-`<=` teeth: the per-arg `encode_call_arg`
  pairing (which encoded arg goes to which param position) is load-bearing.

  Registry: `sub_fn(p, q) -> int { p - q }` — a non-commutative body (`p - q ≠ q - p` in general).
  Source clause `sub_fn(a, b)` at `a := 1, b := 2` denotes `1 - 2 = -1`; the wrong `sub_fn(b, a)`
  denotes `2 - 1 = 1`. `-1 ≠ 1` — they disagree, so an arg-order-swapping encoder breaks T1. -/

/-- The non-commutative spec fn `sub_fn(p, q) = p - q` (the #181 witness body — its non-commutativity
    is what makes the arg order observable). -/
def subFn : SpecFn := SpecFn.mk ["p", "q"] (Expr.arith ArithOp.sub (Expr.var "p") (Expr.var "q"))

/-- A second spec fn `add_fn(p, q) = p + q` — used for the wrong-resolution teeth (a call that
    resolves to `add_fn` where the source resolves to `sub_fn` is a different meaning). -/
def addFn : SpecFn := SpecFn.mk ["p", "q"] (Expr.arith ArithOp.add (Expr.var "p") (Expr.var "q"))

/-- A spec fn `g(p) = sub_fn(p, 1)` whose body itself contains a `specCall` (`sub_fn`) — the #181
    nested-resolution witness: denoting `g(x)` recurses through two registry entries (`g` then
    `sub_fn`), exercising the well-founded recursive descent at a fuel that unfolds (not the
    fuel-`0` bottom — the non-vacuity of the recursive denotation). -/
def gFn : SpecFn := SpecFn.mk ["p"]
  (Expr.specCall "sub_fn" [Expr.var "p", Expr.intLit 1])

/-- The shared spec-fn registry env (#181): `sub_fn`/`add_fn`/`g` resolve here; `a := 1`, `b := 2`,
    `p := 5` (the nested-resolution witness arg). Shared between `denote` and `refDenote` — the
    load-bearing fact for the call-site soundness (the same registry resolves the same body). -/
def envSpec : Env :=
  { ints := fun s => if s = "a" then 1 else if s = "b" then 2 else if s = "p" then 5 else 0
    seqs := fun _ => []
    optres := fun _ => OptResVal.none_
    specs := fun nm =>
      if nm = "sub_fn" then some subFn
      else if nm = "add_fn" then some addFn
      else if nm = "g" then some gFn
      else none }

/-- The faithful `sub_fn(a, b)` source meaning — args in source order (`p ↦ a = 1`, `q ↦ b = 2`),
    body `p - q = -1`. -/
noncomputable def subCallFaithful : Int :=
  intVal 1 (Expr.specCall "sub_fn" [Expr.var "a", Expr.var "b"]) envSpec

/-- The #181 wrong-arg-order bug: the encoder emits `sub_fn(b, a)` for the source `sub_fn(a, b)` —
    the args swapped (`encode_call_arg` pairing each arg with the wrong param position). Modelled as
    the encoder meaning of the swapped-arg call. -/
noncomputable def subCallArgSwapped : Int :=
  refIntVal 1 (Expr.specCall "sub_fn" [Expr.var "b", Expr.var "a"]) envSpec

/-- **Teeth (negative sanity, the #181 wrong-arg-order case).** At `envSpec` (`a := 1`, `b := 2`,
    `sub_fn(p,q) = p - q`) the faithful `sub_fn(a, b)` denotes `1 - 2 = -1` while the arg-swapped
    `sub_fn(b, a)` denotes `2 - 1 = 1` — they disagree, so an arg-order-swapping encoder does not
    satisfy the soundness equation for this spec-fn call. This pins `encode_call_arg`'s arg→param
    pairing (the recursive `ref_sound`/`refIntValArgs_eq` thread the args in order). -/
theorem specfn_arg_order_breaks_soundness :
    subCallFaithful ≠ subCallArgSwapped := by
  -- faithful: bind p↦1, q↦2 → 1 - 2 = -1 ; swapped: bind p↦2, q↦1 → 2 - 1 = 1
  simp [subCallFaithful, subCallArgSwapped, intVal, refIntVal, refIntValArgs, intValArgs,
        envSpec, subFn, Env.bindParams, Env.bindInt, arithDenote, tokArith, encArith]

/-! ## Negative sanity lemma 9 — the #181 wrong-registry-resolution teeth (the spec-fn-call class)

  The dispatch's explicit requirement (b): demonstrate that an encoder that resolved a spec-fn call
  to the wrong spec fn — emitting `add_fn(a, b)` where the source calls `sub_fn(a, b)` (the
  `encode_call` callee name wrong) — does not satisfy soundness. Which registry entry the call name
  resolves to is load-bearing (the encoder emits `{name}(args)`; the name is the content).

  At `envSpec` (`a := 1`, `b := 2`): the source `sub_fn(a, b)` = `1 - 2 = -1`; the wrong
  `add_fn(a, b)` = `1 + 2 = 3`. `-1 ≠ 3` — they disagree. -/

/-- The #181 wrong-resolution bug: the encoder emits `add_fn(a, b)` where the source resolves to
    `sub_fn(a, b)` — the callee name mis-resolved in the registry. Modelled as the encoder meaning of
    the `add_fn`-named call (resolving the wrong registry entry). -/
noncomputable def addCallWrongResolution : Int :=
  refIntVal 1 (Expr.specCall "add_fn" [Expr.var "a", Expr.var "b"]) envSpec

/-- **Teeth (negative sanity, the #181 wrong-registry-resolution case).** At `envSpec` (`a := 1`,
    `b := 2`, `sub_fn = p-q`, `add_fn = p+q`) the faithful `sub_fn(a, b)` denotes `-1` while the
    wrong-resolution `add_fn(a, b)` denotes `1 + 2 = 3` — they disagree, so an encoder that resolved
    the call to the wrong spec fn does not satisfy soundness. This pins the call's name resolution
    (`ref_sound`'s `specCall` case resolves `name` in the same `Env.specs` on both sides). -/
theorem specfn_wrong_resolution_breaks_soundness :
    subCallFaithful ≠ addCallWrongResolution := by
  -- faithful sub_fn: 1 - 2 = -1 ; wrong add_fn: 1 + 2 = 3
  simp [subCallFaithful, addCallWrongResolution, intVal, refIntVal, refIntValArgs, intValArgs,
        envSpec, subFn, addFn, Env.bindParams, Env.bindInt, arithDenote, tokArith, encArith]

/-- **The #181 nested-resolution witness (the recursive denotation unfolds, not a fuel-`0` bottom).**
    `g(p) = sub_fn(p, 1)` — denoting `g(5)` recurses through two registry entries (`g` then `sub_fn`),
    so at fuel `2` the denotation unfolds to `5 - 1 = 4` (not the shared fuel-`0` default `0`,
    which would be a vacuity dodge). This exercises the well-founded recursive descent at a fuel that
    fires; the value `4` (not `0`) shows the recursive denotation is non-vacuous. -/
theorem specfn_nested_resolution_value :
    intVal 2 (Expr.specCall "g" [Expr.var "p"]) envSpec = 4 := by
  simp [intVal, intValArgs, envSpec, gFn, subFn, Env.bindParams, Env.bindInt, arithDenote]

/-- **(T1) for the #181 spec-fn-call fragment — the generic call-site soundness, at every fuel.** For
    every fuel, the encoder meaning of a spec-fn call equals the source meaning — `ref_sound`
    specialized to a `specCall`. Stated `∀ fuel` (the fuel-uniform statement, not a fuel-cap dodge):
    at the nested-resolution witness `g(p)` it holds at fuel `2` (where it unfolds to `4`) and at
    fuel `0` (where both bottom to `True`), because the source and encoder share the fuel + registry.
    Confirms the faithful spec-fn-call encoder (args in order, name resolved correctly) is sound; the
    teeth above bite only the arg-swap / wrong-resolution. -/
theorem specfn_call_faithful_is_sound (fuel : Nat) :
    refDenote fuel (Expr.specCall "g" [Expr.var "p"]) envSpec
      ↔ denote fuel (Expr.specCall "g" [Expr.var "p"]) envSpec :=
  ref_sound _ _ _

/-! ## Negative sanity lemma 10 — the #182 `count_where` teeth (wrong-predicate + off-by-one count)

  The dispatch's explicit requirement (a): demonstrate that a `count_where` encoded with a wrong
  predicate or an off-by-one count fails soundness. `count_where` is a value-combinator (`intVal`),
  so the teeth are an inequality of counts (not an `Iff` of `Prop`s).

  Source clause: `count_where(s, |x| x ≤ 15)` at `envAB` (`s := [10, 20, 30]`) — exactly one element
  (`10`) is ≤ 15, so the faithful count is `1`.
    (A) Wrong predicate: `count_where(s, |x| x ≤ 25)` counts two (`10`, `20`) → `2 ≠ 1`.
    (B) Off-by-one: a buggy encoder whose count is `count_where(..) + 1` reads `2 ≠ 1`. -/

/-- The source `count_where(s, |x| x ≤ 15)` clause (#182) — a value-combinator (the `pred` slot is
    `|x| x ≤ 15`; `seq` is `s`). At `envAB` (`s := [10,20,30]`) its faithful count is `1`. -/
def countWhereClause : Expr :=
  Expr.comb CombName.countWhere (Expr.strVar "s") none none (some predLe15)

/-- The #182 wrong-predicate bug: `count_where(s, |x| x ≤ 25)` where the source predicate is
    `|x| x ≤ 15` — the closure body infidelity (`encode_pred_arg` re-encoding the wrong body). At
    `envAB` (`s := [10,20,30]`) it counts `2` (`10`, `20`) ≠ the faithful `1`. -/
def countWhereWrongPred : Expr :=
  Expr.comb CombName.countWhere (Expr.strVar "s") none none
    (some (Pred.mk "x" (Expr.cmp CmpOp.le (Expr.var "x") (Expr.intLit 25))))

/-- **Teeth (negative sanity, the #182 wrong-predicate `count_where` case).** At `envAB`
    (`s := [10,20,30]`) the faithful `count_where(s, |x| x ≤ 15)` = `1` while the wrong-predicate
    `count_where(s, |x| x ≤ 25)` = `2` — they disagree, so a `count_where` with a corrupted predicate
    does not satisfy the soundness equation. This pins the `count_where` predicate encoding (the
    `count_where` case of `refVal_eq` threads the body via the recursive `ref_sound` IH). -/
theorem count_where_wrong_pred_breaks_soundness :
    intVal 0 countWhereClause envAB ≠ intVal 0 countWhereWrongPred envAB := by
  -- faithful: count {10,20,30 | ≤15} = 1 ; wrong: count {10,20,30 | ≤25} = 2
  simp [countWhereClause, countWhereWrongPred, predLe15, predLe15Body, intVal, seqVal,
        countWhereVal, countWhereVal_cons, denote, Env.bindInt, envAB]

/-- **Teeth (negative sanity, the #182 off-by-one `count_where` case).** The faithful count of
    `count_where(s, |x| x ≤ 15)` at `envAB` is `1`; an off-by-one encoder reading `count + 1` would
    yield `2`. `1 ≠ 2` — an off-by-one count breaks soundness. (The faithful count is fixed to the
    concrete `1`, so any encoder emitting a different integer, including `+1`, disagrees.) -/
theorem count_where_off_by_one_breaks_soundness :
    intVal 0 countWhereClause envAB ≠ intVal 0 countWhereClause envAB + 1 := by
  have h : intVal 0 countWhereClause envAB = 1 := by
    simp [countWhereClause, predLe15, predLe15Body, intVal, seqVal,
          countWhereVal, countWhereVal_cons, denote, Env.bindInt, envAB]
  rw [h]; decide

/-- The faithful positive counterpart, for contrast (#182): with the real `count_where` encoding the
    `count_where(s, |x| x ≤ 15)` clause's encoder meaning equals the source count (`1`), by
    `refIntVal_eq_intVal`. Confirms the teeth bite only the corrupted predicate / off-by-one, not the
    faithful encoder; and that the recursive count is non-vacuous (the value `1`, not a bottom). -/
theorem count_where_faithful_intval_matches_source :
    refIntVal 0 countWhereClause envAB = intVal 0 countWhereClause envAB :=
  refIntVal_eq_intVal _ _ _

/-- The faithful count is `1` (the recursive `countWhereVal` fires over
    `[10,20,30]`, not a vacuous `0`) — the non-vacuity of the `count_where` recursion mechanism. -/
theorem count_where_value_is_one :
    intVal 0 countWhereClause envAB = 1 := by
  simp [countWhereClause, predLe15, predLe15Body, intVal, seqVal,
        countWhereVal, countWhereVal_cons, denote, Env.bindInt, envAB]

/-! ## Negative sanity lemma 11 — the #182 `permutation_of` multiset-vs-set teeth (the key fidelity)

  The dispatch's explicit requirement (b): demonstrate that `permutation_of` mis-modelled as set
  equality (membership) instead of multiset (counts) fails. The canonical witness: `a := [1,1,2]`,
  `b := [1,2,2]` have the same set `{1,2}` but different multisets (`count 1` is `2` in `a` vs `1` in
  `b`; `count 2` is `1` vs `2`). So `permutation_of(a, b)` is false (the faithful multiset model
  `permEq` — `a.to_multiset() ≠ b.to_multiset()`), while a set-based model wrongly says true. This is
  the fidelity check that `permutation_of`'s `verus_l3` is `to_multiset()` equality, not set equality. -/

/-- A fresh env for the `permutation_of` multiset-vs-set teeth (#182): `a := [1,1,2]`, `b := [1,2,2]`
    (the canonical same-set / different-multiset witness). Everything else empty / `None` / no spec. -/
def envPerm : Env :=
  { ints := fun _ => 0
    seqs := fun nm => if nm = "a" then [1, 1, 2] else if nm = "b" then [1, 2, 2] else []
    optres := fun _ => OptResVal.none_
    specs := fun _ => none }

theorem envPerm_a : envPerm.seqs "a" = [1, 1, 2] := rfl
theorem envPerm_b : envPerm.seqs "b" = [1, 2, 2] := rfl

/-- The source `permutation_of(a, b)` clause (#182) — a `Prop`-combinator (two slice args, `seq` = `a`,
    `seq2` = `some b`, no predicate). The faithful multiset model `permEq` over the `@`-views. -/
def permClause : Expr :=
  Expr.comb CombName.permutationOf (Expr.seqVar "a") (some (Expr.seqVar "b")) none none

/-- The (wrong) set-equality model of `permutation_of` — membership, not counts: `∀ x, (x ∈ a ↔ x ∈ b)`.
    This is the infidelity the dispatch names: modelling `to_multiset()` equality as set equality. At
    `a := [1,1,2]`, `b := [1,2,2]` it is true (both have set `{1,2}`), whereas the faithful `permEq`
    (counts) is false — the multiset-vs-set teeth. -/
def permSetModel (a b : List Int) : Prop :=
  ∀ x : Int, (x ∈ a ↔ x ∈ b)

/-- **Teeth (negative sanity, the #182 permutation_of multiset-vs-set case — the key fidelity check).**
    At `a := [1,1,2]`, `b := [1,2,2]` the set model (`permSetModel`) is true (same set `{1,2}`) while
    the source `permutation_of` (the faithful multiset `permEq`) is false (`count 1` is `2` vs `1`).
    True vs false — they disagree, so an encoder that modelled `permutation_of` as set equality does
    not satisfy soundness. This proves `permutation_of`'s `verus_l3` is `to_multiset()` equality (the
    count-characterization), not set/membership equality. -/
theorem permutation_set_model_breaks_soundness :
    ¬ (permSetModel (envPerm.seqs "a") (envPerm.seqs "b")
        ↔ denote 0 permClause envPerm) := by
  intro h
  -- The set model is true: [1,1,2] and [1,2,2] have the same membership (both = `x = 1 ∨ x = 2`).
  have hSet : permSetModel (envPerm.seqs "a") (envPerm.seqs "b") := by
    intro x
    rw [envPerm_a, envPerm_b]
    simp only [List.mem_cons, List.not_mem_nil, or_false]
    omega
  -- So `permutation_of` (the faithful multiset `permEq`) would have to be true — but it is false:
  -- `count 1` is `2` in `[1,1,2]` vs `1` in `[1,2,2]`.
  have hPerm := h.mp hSet
  rw [permClause, denote.eq_def] at hPerm
  simp only [seqVal, permEq] at hPerm
  have h1 := hPerm 1
  rw [envPerm_a, envPerm_b] at h1
  simp at h1

/-- The faithful positive counterpart, for contrast (#182): with the real `permutation_of` (the
    multiset `permEq`) the clause is sound — its encoder meaning is equivalent to the source, by
    `ref_sound`. Confirms the teeth bite only the set-model infidelity, not the faithful encoder. -/
theorem permutation_faithful_is_sound :
    refDenote 0 permClause envPerm ↔ denote 0 permClause envPerm :=
  ref_sound _ _ _

/-- A faithful positive witness that `permutation_of` is non-vacuous and is satisfied by a genuine
    permutation (#182): `[1,2,3]` is a permutation of `[3,1,2]` (same multiset) — the source
    `permutation_of` is true here (every count agrees), showing `permEq` is not trivially false. -/
def envPermTrue : Env :=
  { ints := fun _ => 0
    seqs := fun nm => if nm = "a" then [1, 2, 3] else if nm = "b" then [3, 1, 2] else []
    optres := fun _ => OptResVal.none_
    specs := fun _ => none }

theorem envPermTrue_a : envPermTrue.seqs "a" = [1, 2, 3] := rfl
theorem envPermTrue_b : envPermTrue.seqs "b" = [3, 1, 2] := rfl

theorem permutation_true_on_real_permutation :
    denote 0 permClause envPermTrue := by
  rw [permClause, denote.eq_def]
  simp only [seqVal, permEq]
  intro x
  rw [envPermTrue_a, envPermTrue_b]
  -- `[1,2,3]` is a permutation of `[3,1,2]` (`decide`), so per-element counts agree.
  exact (by decide : ([1, 2, 3] : List Int).Perm [3, 1, 2]).count_eq x

end Thermite
