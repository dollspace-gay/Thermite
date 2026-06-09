/-
  Thermite/Soundness.lean — the (T1) soundness theorem for the comparison + logical
  fragment (#170) EXTENDED with the ARITHMETIC operators (#176), the CASTS (#177), the
  SPEC-CONTEXT REWRITES (#178), the 6 BOUNDED-QUANTIFIER COMBINATORS (#179), and the
  MATCH-IN-ENS / `is` PAYLOAD-IN-CONTRACT forms (#180 / 1g — the C7 class); epic #169.

  THE #180 MATCH/`is` EXTENSION. `ref_sound` gains the `optResVar`/`match_`/`is_` cases; the
  `match_` case threads a MUTUAL `ref_sound_arms` (the arm-walk soundness — each arm body via the
  recursive `ref_sound` IH, the selection-by-variant + payload-binding SHARED with the source).
  NON-VACUOUS: the negative `match_arm_swap_breaks_soundness` (a `Some`/`None` body swap DISAGREES
  at `result := Some 7`) and `is_wrong_variant_breaks_soundness` (`is Some` tested as `is None`
  DISAGREES) bite; the positives `match_faithful_is_sound`/`match_result_faithful_is_sound`/
  `is_faithful_is_sound` confirm the faithful encoder is sound. Scoped to Option/Result (the
  built-in `Some/None/Ok/Err` `encode_pattern` admits); general user ADTs are OUT (the encoder
  honestly `Err`s on them) and DELIBERATELY not embedded.

  Governing design: `.design/verified/thermite-semantics.md` REQ-2 (T1: the
  verified-validator obligation `∀ P, ⟦R(P)⟧ = ⟦P⟧_S`, proved by structural induction)
  + AC-2 (the theorem is NON-VACUOUS: the binop/coercion re-statement is the content)
  + Architecture §"S_C" (the `cast → nat/int` rule: "the paren-drop is a (T1) failure
  (a different parse = a different denotation)"). Field vocabulary
  (formal-methods-sota.md finding #1/#2): the VERIFIED-VALIDATOR soundness step
  (Leroy/CompCert), the kernel-checked core of SEMANTIC PRESERVATION for this fragment.

  T1 (this fragment): `∀ e env, refDenote e env ↔ denote e env`.
  `refDenote` follows the reference ENCODER's operator + cast-target maps
  (`encOp`/`encLog`/`encArith`/`encCast` mirror `ref_encode.rs::binop_str`/
  `cast_target`) and its PARENTHESIZATION (`encode_binary`/`encode_cast` wrap their
  operands/inner); `denote` follows the SOURCE meaning. They are defined
  INDEPENDENTLY (so this is not `rfl`-vacuous) and proved equivalent by induction.

  THE #122/#146 RETIREMENT (cast-paren class). The negative lemma
  `cast_paren_drop_breaks_soundness` shows a faulty encoder that DROPS the cast
  paren — emitting `n - 1 as nat` (which Verus/Rust parse as `n - (1 as nat)`)
  instead of the faithful `(n - 1) as nat` — does NOT satisfy soundness at a
  concrete env. This is the proven retirement of the #122/#146 cast-paren class on
  the CONTRACT side: the faithful `encode_cast` paren is what makes T1 hold.
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
    faithfulness facts — the encoder's DISPATCH CHOICE is the content. -/
theorem byteView_encByteAt (s : List Int) (i : Int) :
    byteView encByteAt s i = seqIdx s i := rfl

theorem byteView_encLen (s : List Int) (i : Int) :
    byteView encLen s i = (s.length : Int) := rfl

/- The COMBINED meaning-coincidence (#178): the encoder's `refIntVal` equals the
   source `intVal` AND `refSeqVal` equals `seqVal`, simultaneously, on every term, with
   a companion lemma threading a `RangeArg`'s bounds. Because `Expr`/`RangeArg` are
   MUTUALLY inductive (the `induction` tactic does not support them), these are proved
   as MUTUAL STRUCTURAL-RECURSIVE theorems (the recursive calls ARE the inductive
   hypotheses; Lean checks the structural decrease). The `@`-view (`seqVar`/`strVar` →
   identity), the `subrange` (→ the same `seqSub`), and the byte-view dispatch
   (`byteView_encByteAt`/`byteView_encLen`) are PROVEN denotation-preserving here; the
   operator/cast round-trips settle #176/#177. -/
mutual
/-- `refIntVal e = intVal e ∧ refSeqVal e = seqVal e`, by mutual structural recursion. -/
theorem refVal_eq (e : Expr) (env : Env) :
    refIntVal e env = intVal e env ∧ refSeqVal e env = seqVal e env := by
  cases e with
  | intLit n => exact ⟨rfl, rfl⟩
  | boolLit b => exact ⟨rfl, rfl⟩
  | var x => exact ⟨rfl, rfl⟩
  | cmp op a b => exact ⟨rfl, rfl⟩
  | logic op a b => exact ⟨rfl, rfl⟩
  | neg e => exact ⟨rfl, rfl⟩
  | arith op a b =>
      refine ⟨?_, rfl⟩
      simp only [refIntVal, intVal, (refVal_eq a env).1, (refVal_eq b env).1,
                 tokArith_encArith]
  | cast inner ty =>
      refine ⟨?_, rfl⟩
      simp only [refIntVal, intVal, (refVal_eq inner env).1, tokCast_encCast]
  | seqVar x => exact ⟨rfl, rfl⟩
  | strVar x => exact ⟨rfl, rfl⟩
  | idx base i =>
      -- `refIntVal (idx ..) = byteView encByteAt (refSeqVal base) (refIntVal i)`;
      -- the base recursion settles the `@`-view (`refSeqVal = seqVal`), the index
      -- recursion the index, and `byteView_encByteAt` the dispatch.
      refine ⟨?_, rfl⟩
      simp only [refIntVal, intVal, byteView_encByteAt,
                 (refVal_eq base env).2, (refVal_eq i env).1]
  | subrange base r =>
      -- Sequence-sorted: the base recursion settles the view, the RANGE-BOUND recursion
      -- (`refRangeVal_eq`) the bounds, and the SAME `seqSub` the head.
      refine ⟨rfl, ?_⟩
      cases r with
      | rangeTo hi =>
          simp only [refSeqVal, seqVal, (refVal_eq base env).2,
                     (refVal_eq hi env).1]
      | range lo hi =>
          simp only [refSeqVal, seqVal, (refVal_eq base env).2,
                     (refVal_eq lo env).1, (refVal_eq hi env).1]
      | rangeFrom lo =>
          simp only [refSeqVal, seqVal, (refVal_eq base env).2,
                     (refVal_eq lo env).1]
  | seqLen base =>
      refine ⟨?_, rfl⟩
      simp only [refIntVal, intVal, byteView_encLen, (refVal_eq base env).2]
  | byteAt base i =>
      -- `byte_at`→`spec_byte_at(i)`: identical to `idx` (the i-th byte of the bytes).
      refine ⟨?_, rfl⟩
      simp only [refIntVal, intVal, byteView_encByteAt,
                 (refVal_eq base env).2, (refVal_eq i env).1]
  | comb c seq seq2 idx pred =>
      -- A combinator is BOOLEAN/`Prop`-sorted: as an INTEGER term it falls to `0` and as
      -- a SEQUENCE term to `[]` in BOTH `refIntVal`/`intVal` and `refSeqVal`/`seqVal` (the
      -- `_` defaults — a combinator is never a comparison operand / a sequence base; its
      -- predicate equivalence is handled by the `comb` case of `ref_sound`, not here).
      exact ⟨rfl, rfl⟩
  -- The #180 match/is forms are OPTION/RESULT- or BOOLEAN/`Prop`-sorted: as an INTEGER term
  -- each falls to `0` and as a SEQUENCE term to `[]` (the `_` defaults — a `match`/`is`/
  -- `optResVar` is never a comparison operand / a sequence base; the match/is equivalence is
  -- the `match_`/`is_` case of `ref_sound`, not here).
  | optResVar x => exact ⟨rfl, rfl⟩
  | match_ scrut arms => exact ⟨rfl, rfl⟩
  | is_ scrut variant => exact ⟨rfl, rfl⟩
end

/-- The integer-term meanings coincide (the projection of `refVal_eq` used by the
    `cmp`/`idx`/… cases of `ref_sound`). -/
theorem refIntVal_eq_intVal (e : Expr) (env : Env) :
    refIntVal e env = intVal e env := (refVal_eq e env).1

/-- The sequence-term meanings coincide (the `@`-view/`subrange` projection of
    `refVal_eq`). -/
theorem refSeqVal_eq_seqVal (e : Expr) (env : Env) :
    refSeqVal e env = seqVal e env := (refVal_eq e env).2

mutual
/--
  **(T1) — verified-validator soundness, comparison/logical/arithmetic/cast/
  spec-context-rewrite fragment.**

  For every contract `Expr` `e` in the fragment and every environment `env`, the
  meaning of the reference encoder's output (`refDenote`, routed through the encoder's
  operator + cast-target maps, its parenthesization, the slice→`@`/`subrange` rewrite,
  and the byte-view dispatch) is LOGICALLY EQUIVALENT to the source denotation
  (`denote`, the standard `S_C` meaning). Proved by structural `induction` on `e`, one
  case per inference rule (`thermite-semantics.md` REQ-2).

  NON-VACUOUS: `refDenote`/`refIntVal`/`refSeqVal` and `denote`/`intVal`/`seqVal` are
  defined in separate modules following different structure (the encoder's binop/cast
  maps + paren + `@`-view + byte-view dispatch vs the source relation/arithmetic/
  element/byte); the `cmp` case discharges the `encOp`/`tokRel` round-trip per operator
  AND the integer-operand equality `refIntVal_eq_intVal` — which itself carries the #176
  arithmetic round-trip, the #177 cast round-trip, and the #178 `@`-view/`subrange`/
  byte-view rewrites (via `refVal_eq`), NOT a definitional collapse. See
  `eq_le_infidelity_*` (the `==`-vs-`<=` teeth), `cast_paren_drop_breaks_soundness` (the
  #122/#146 cast-paren teeth), and `byteview_misdispatch_breaks_soundness` (the #127
  byte-view-dispatch teeth) below.
-/
theorem ref_sound (e : Expr) (env : Env) : refDenote e env ↔ denote e env := by
  -- `Expr` is mutually inductive (with `RangeArg`/`MatchArm`), so `induction` is unavailable;
  -- proceed by `cases` with RECURSIVE `ref_sound` calls on the predicate subterms
  -- (`logic`/`neg`/the `match_` arm bodies via `ref_sound_arms`) — the recursion IS the
  -- structural induction Lean checks.
  cases e with
  | intLit n => simp [refDenote, denote]
  | boolLit b => simp [refDenote, denote]
  | var x => simp [refDenote, denote]
  | cmp op a b =>
      -- Both sides reduce to a relation over the SAME operands (refIntVal = intVal,
      -- now incl. the arith/cast/idx/seqLen/byteAt subterms via `refVal_eq`); the
      -- operator round-trip `tokRel (encOp op)` = the source relation is settled
      -- per-operator by `cases`.
      cases op <;>
        simp [refDenote, denote, encOp, tokRel,
              refIntVal_eq_intVal a env, refIntVal_eq_intVal b env]
  | logic op a b =>
      cases op <;>
        simp [refDenote, denote, encLog, tokConn, ref_sound a env, ref_sound b env]
  | neg e =>
      simp [refDenote, denote, ref_sound e env]
  | arith op a b =>
      -- An arithmetic term is integer-sorted: as a TOP-LEVEL predicate it is not a
      -- well-formed clause, so both `refDenote` and `denote` fall to `True`
      -- (it only ever appears as a comparison operand, handled in the `cmp` case via
      -- `refIntVal_eq_intVal`). The iff is reflexive here.
      simp [refDenote, denote]
  | cast inner ty =>
      -- Likewise a cast term is integer-sorted; top-level it is `True ↔ True`.
      simp [refDenote, denote]
  -- The #178 spec-context-rewrite terms are integer/sequence-sorted: as a TOP-LEVEL
  -- predicate each falls to `True ↔ True` (it only ever appears as a comparison
  -- operand / a sequence base, handled in `cmp` via `refIntVal_eq_intVal`, which itself
  -- routes through `refVal_eq` for the `@`-view/`subrange`/byte-view content).
  | seqVar x => simp [refDenote, denote]
  | strVar x => simp [refDenote, denote]
  | idx base i => simp [refDenote, denote]
  | subrange base r => simp [refDenote, denote]
  | seqLen base => simp [refDenote, denote]
  | byteAt base i => simp [refDenote, denote]
  | comb c seq seq2 idx pred =>
      -- THE 6 BOUNDED-QUANTIFIER COMBINATORS (#179). Both `refDenote` and `denote` expand
      -- to the SAME frozen `verus_l3` quantifier FORM (the body is the shared registry
      -- ground truth — `encode_combinator_call` reuses it verbatim); they differ ONLY in
      -- the per-arg-kind threading: the slice (`refSeqVal` vs `seqVal`), the second slice,
      -- the scalar index (`refIntVal` vs `intVal` — the #145 arg-kind), and the predicate
      -- body (`refDenote` vs `denote`, applied at the i-th element). Establish those agree,
      -- then the quantifier forms are equivalent by congruence.
      have hs : refSeqVal seq env = seqVal seq env := refSeqVal_eq_seqVal seq env
      -- The pointwise predicate-application equivalence: at every element value `v`, the
      -- encoder's predicate body and the source's agree (the recursive IH `ref_sound` on
      -- the FLAT closure body — `body` is a structural subterm of `comb`).
      have hp : ∀ v : Int,
          (match pred with
            | some (Pred.mk bound body) => refDenote body (env.bindInt bound v)
            | none => True) ↔
          (match pred with
            | some (Pred.mk bound body) => denote body (env.bindInt bound v)
            | none => True) := by
        intro v
        cases pred with
        | none => exact Iff.rfl
        | some pr => cases pr with
          | mk bound body => exact ref_sound body (env.bindInt bound v)
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
              simp only [refDenote, denote, hs, refIntVal_eq_intVal e env]
              exact forall_congr' (fun i => imp_congr_right (fun _ => hp _))
      | forallFrom =>
          cases idx with
          | none =>
              simp only [refDenote, denote, hs]
              exact forall_congr' (fun i => imp_congr_right (fun _ => hp _))
          | some e =>
              simp only [refDenote, denote, hs, refIntVal_eq_intVal e env]
              exact forall_congr' (fun i => imp_congr_right (fun _ => hp _))
      | disjoint =>
          cases seq2 with
          | none => simp only [refDenote, denote, hs]
          | some e => simp only [refDenote, denote, hs, refSeqVal_eq_seqVal e env]
  -- An `optResVar` is OPTION/RESULT-sorted: top-level it is not a predicate (it is only ever a
  -- `match`/`is` scrutinee, read by the shared `scrutVal`), so both sides fall to `True`.
  | optResVar x => simp [refDenote, denote]
  -- THE MATCH-IN-ENS form (#180). The scrutinee value `scrutVal scrut env` is a FREE name (the
  -- SAME on both sides — `scrutVal` reads `env.optres`); the arm SELECTION-by-variant is the
  -- shared Verus `match` meaning (`refDenoteArms`/`denoteArms` are STRUCTURALLY identical); the
  -- ONLY difference is each arm BODY (`refDenote` vs `denote`), settled by `ref_sound_arms` (the
  -- recursive IH on the arm bodies, each a structural subterm of `match_`).
  | match_ scrut arms =>
      simp only [refDenote, denote]
      exact ref_sound_arms (scrutVal scrut env) arms env
  -- THE `is`-TEST (#180). Both sides are DEFINITIONALLY `((scrutVal scrut env).isVariant
  -- variant = true)` (the shared Verus `is` discriminant test); the iff is reflexive.
  | is_ scrut variant => simp only [refDenote, denote]

/-- THE MATCH-ARM soundness (#180), MUTUAL with `ref_sound`: the encoder's arm walk
    `refDenoteArms` is equivalent to the source `denoteArms` at the SAME scrutinee value, by
    structural recursion on the arm list. Each step is either the SELECTED arm's body
    (`ref_sound` on the body — a structural subterm of the `match_`, so the recursion is
    well-founded) or the recursive tail. The selection condition (`scrut.variant = variant`) +
    the payload binding (`Env.bindInt … scrut.payload`) are SHARED (identical on both sides — the
    Verus `match` semantics the encoder reuses verbatim), so the only content is the per-body
    `ref_sound`. -/
theorem ref_sound_arms (scrut : OptResVal) (arms : List MatchArm) (env : Env) :
    refDenoteArms scrut arms env ↔ denoteArms scrut arms env := by
  cases arms with
  | nil => simp [refDenoteArms, denoteArms]
  | cons arm rest =>
      cases arm with
      | mk variant binder body =>
          simp only [refDenoteArms, denoteArms]
          by_cases h : scrut.variant = variant
          · simp only [h, if_true]
            cases binder with
            | none => exact ref_sound body env
            | some x => exact ref_sound body (env.bindInt x scrut.payload)
          · simp only [h, if_false]
            exact ref_sound_arms scrut rest env
end

/-- A convenient `Prop`-equality corollary (propositional extensionality) — the
    `⟦R(P)⟧ = ⟦P⟧_S` form (T2's transitivity step composes on this equality, AC-3). -/
theorem ref_sound_eq (e : Expr) (env : Env) : refDenote e env = denote e env :=
  propext (ref_sound e env)

/-! ## Negative sanity lemma 1 — the comparison teeth (`==` ≠ `<=`)

  The #170 teeth, retained: an encoder that mapped `Eq → "<="` (the boss's
  `==`-vs-`<=` infidelity) would NOT satisfy soundness at a concrete `env`. -/

/-- A faulty encoder operator map: `Eq` mis-mapped to the `<=` token (the
    infidelity), every other operator faithful. Mirrors a hypothetical
    `binop_str` bug `Eq => "<="`. -/
def encOpFaulty : CmpOp → VerusCmpTok
  | CmpOp.eq => VerusCmpTok.leTok   -- THE BUG: `==` emitted as `<=`
  | CmpOp.ne => VerusCmpTok.neTok
  | CmpOp.lt => VerusCmpTok.ltTok
  | CmpOp.le => VerusCmpTok.leTok
  | CmpOp.gt => VerusCmpTok.gtTok
  | CmpOp.ge => VerusCmpTok.geTok

/-- `refDenote` with the faulty `Eq→<=` map on a comparison. -/
def refDenoteFaultyCmp (op : CmpOp) (a b : Expr) (env : Env) : Prop :=
  tokRel (encOpFaulty op) (refIntVal a env) (refIntVal b env)

/-- A concrete environment: integer names `a := 1`, `b := 2`, `n := -1` (everything
    else `0`); sequence name `s := [10, 20, 30]` (a `String`'s bytes; everything else
    the empty sequence) — the witness sequence for the #127 byte-view-dispatch teeth
    (its bytes DIFFER at adjacent indices, so a wrong index / wrong method is observable). -/
def envAB : Env :=
  { ints := fun s => if s = "a" then 1 else if s = "b" then 2
                     else if s = "n" then -1 else 0
    seqs := fun s => if s = "s" then [10, 20, 30] else []
    -- The #180 option/result binding: `result := Some 7` (the C7 match/is scrutinee witness —
    -- a `Some`-valued result carrying the integer payload 7; everything else `None`).
    optres := fun s => if s = "result" then OptResVal.some_ 7 else OptResVal.none_ }

/-- **Teeth (negative sanity, the `==`-vs-`<=` case, #170).** At `envAB` the faulty
    `Eq→<=` encoding of `a == b` is TRUE (`1 ≤ 2`) while the source meaning of
    `a == b` is FALSE (`1 ≠ 2`) — so the faulty encoder does NOT satisfy the
    soundness equation. -/
theorem eq_le_infidelity_breaks_soundness :
    ¬ (refDenoteFaultyCmp CmpOp.eq (Expr.var "a") (Expr.var "b") envAB
        ↔ denote (Expr.cmp CmpOp.eq (Expr.var "a") (Expr.var "b")) envAB) := by
  simp [refDenoteFaultyCmp, encOpFaulty, tokRel, denote, intVal, refIntVal, envAB]

/-! ## Negative sanity lemma 2 — the #122/#146 CAST-PAREN teeth (the retired class)

  The dispatch's explicit requirement: demonstrate that an encoder that DROPS the
  cast paren — emitting `(n - 1) as nat` as the bare `n - 1 as nat`, which Verus/Rust
  PARSE as `n - (1 as nat)` (a cast binds tighter than `-`) — does NOT satisfy
  soundness at a concrete env where the precedence changes the value. This is the
  #122 `divergence_cast_paren` / #146 cast-mis-parse bug, now PROVEN to break T1 on
  the contract side: the faithful `encode_cast` paren (`refIntVal`'s `cast` arm casts
  the WHOLE inner) is exactly what makes `ref_sound` hold for casts.

  The source clause: `(n - 1) as nat`, i.e.
    `Expr.cast (Expr.arith ArithOp.sub (Expr.var "n") (Expr.intLit 1)) CastTy.nat`.
  Its FAITHFUL denotation casts the whole subtraction: `((n - 1) : Int).toNat`.
  The PAREN-DROPPED encoder instead binds the cast to only the rightmost atom `1`,
  yielding `n - (1 as nat)` = `n - (1 : Int)` (no cast on `n`, the `-` outside the
  cast). At `n = -1` these differ: faithful `(-1 - 1).toNat = (-2).toNat = 0`;
  paren-dropped `-1 - 1 = -2`. `0 ≠ -2`. -/

/-- The FAITHFUL cast denotation of `(n - 1) as nat` — what the real `encode_cast`
    (its `({inner}) as nat` paren) produces: the cast applies to the WHOLE inner. -/
def castInnerFaithful (env : Env) : Int :=
  refIntVal (Expr.cast (Expr.arith ArithOp.sub (Expr.var "n") (Expr.intLit 1)) CastTy.nat) env

/-- The PAREN-DROPPED cast denotation — the #122 bug. The buggy encoder emits the
    string `n - 1 as nat`, which re-parses as `n - (1 as nat)`: the cast binds only
    the atom `1`, and the subtraction sits OUTSIDE the cast. We model that re-parsed
    AST and take its faithful `refIntVal` (the bug is the ENCODER's missing paren, not
    a second meaning function — the re-parse is what the dropped paren denotes). -/
def castInnerParenDropped (env : Env) : Int :=
  refIntVal
    (Expr.arith ArithOp.sub (Expr.var "n") (Expr.cast (Expr.intLit 1) CastTy.nat)) env

/-- **Teeth (negative sanity, the #122/#146 cast-paren case).** At `envAB` (`n := -1`)
    the faithful `(n - 1) as nat` denotes `0` while the paren-dropped `n - 1 as nat`
    (re-parsed `n - (1 as nat)`) denotes `-2` — they DISAGREE, so a paren-dropping
    encoder does NOT satisfy the soundness equation `refDenote = denote` for this
    clause. This is the Lean-level witness that `ref_sound`'s `cast` case PINS the
    encoder's parenthesization: had `encode_cast` dropped the inner paren, the proof
    of `refIntVal_eq_intVal` (hence `ref_sound`) would have failed exactly here. -/
theorem cast_paren_drop_breaks_soundness :
    castInnerFaithful envAB ≠ castInnerParenDropped envAB := by
  -- faithful: castDenote nat (-1 - 1) = (-2).toNat = 0
  -- dropped:  (-1) - castDenote nat 1 = -1 - 1 = -2
  simp [castInnerFaithful, castInnerParenDropped, refIntVal, tokCast, tokArith,
        encCast, encArith, castDenote, arithDenote, envAB]

/-- The faithful counterpart, for contrast: with the REAL `refIntVal` (the
    parenthesized cast) the `(n - 1) as nat` clause IS sound — it equals the source
    `intVal` (the whole-inner cast), by `refIntVal_eq_intVal`. Confirms the teeth bite
    ONLY the paren-drop, not the faithful encoder. -/
theorem cast_faithful_intval_matches_source :
    refIntVal (Expr.cast (Expr.arith ArithOp.sub (Expr.var "n") (Expr.intLit 1)) CastTy.nat) envAB
      = intVal (Expr.cast (Expr.arith ArithOp.sub (Expr.var "n") (Expr.intLit 1)) CastTy.nat) envAB :=
  refIntVal_eq_intVal _ _

/-- The faithful counterpart for the comparison teeth, retained from #170: with the
    REAL `encOp` the `a == b` clause IS sound (both `1 = 2`, False), by `ref_sound`. -/
theorem eq_faithful_is_sound :
    refDenote (Expr.cmp CmpOp.eq (Expr.var "a") (Expr.var "b")) envAB
      ↔ denote (Expr.cmp CmpOp.eq (Expr.var "a") (Expr.var "b")) envAB :=
  ref_sound _ _

/-! ## Negative sanity lemma 3 — the #127 BYTE-VIEW-DISPATCH teeth (the retired class)

  The dispatch's explicit requirement (the #178 point): demonstrate that an encoder
  that MIS-DISPATCHES the byte-view — the #127 name-collision bug, where the encoder
  picks the WRONG byte-view spec fn — does NOT satisfy soundness at a concrete sequence
  env. Two faulty dispatches, both real instances of the #127 class:

    (A) WRONG INDEX. The faithful `s.byte_at(0)` → `s.spec_byte_at(0)` reads byte 0.
        A buggy encoder emitting `s.spec_byte_at(0 + 1)` (an off-by-one index) reads
        byte 1. At `s := [10, 20, 30]` these are `10` vs `20` — they DISAGREE.
    (B) WRONG RECEIVER-METHOD. The faithful `s.byte_at(i)` dispatches to the i-th-byte
        spec fn (`encByteAt = specByteAt`). A buggy encoder that mis-dispatched
        `byte_at` to the LENGTH spec fn (`specLen` — the #127 name collision: picking
        `spec_len` for a `byte_at` call) reads the length `3` instead of byte 0 (`10`).
        `10 ≠ 3` — they DISAGREE.

  This is the #127 (`divergence_byteview_name_collision`) class, now PROVEN to break
  T1 on the contract side: the faithful `encByteAt`/`encLen` dispatch (`refIntVal`'s
  `idx`/`byteAt`/`seqLen` arms, the `byteView_encByteAt`/`byteView_encLen` round-trips)
  is exactly what makes `ref_sound` hold for the byte-view rewrites. -/

/-- The FAITHFUL byte-view of `s.byte_at(0)` — what the real `encode_string_byteview`
    (its `spec_byte_at(0)` dispatch) produces: the 0-th byte. -/
def byteAtFaithful (env : Env) : Int :=
  refIntVal (Expr.byteAt (Expr.strVar "s") (Expr.intLit 0)) env

/-- THE #127 WRONG-INDEX BUG (instance A): a buggy encoder emits `s.spec_byte_at(0 + 1)`
    for the source `s.byte_at(0)` — an off-by-one byte-view index (the misdispatch reads
    the wrong byte). Modelled as the byte-view at index `0 + 1` (the dispatch is the
    faithful `encByteAt`, but the INDEX is wrong — the #127 misdispatch shape). -/
def byteAtWrongIndex (env : Env) : Int :=
  byteView encByteAt (refSeqVal (Expr.strVar "s") env)
    (refIntVal (Expr.arith ArithOp.add (Expr.intLit 0) (Expr.intLit 1)) env)

/-- THE #127 WRONG-METHOD BUG (instance B): a buggy encoder mis-dispatches the
    `byte_at` call to the LENGTH spec fn (`encLen` = `spec_len`) — the name-collision
    misdispatch. It reads the sequence LENGTH where the source reads a byte. -/
def byteAtWrongMethod (env : Env) : Int :=
  byteView encLen (refSeqVal (Expr.strVar "s") env)
    (refIntVal (Expr.intLit 0) env)

/-- **Teeth (negative sanity, the #127 wrong-index byte-view-dispatch case).** At
    `envAB` (`s := [10, 20, 30]`) the faithful `s.byte_at(0)` denotes byte `10` while
    the off-by-one `s.spec_byte_at(0 + 1)` denotes byte `20` — they DISAGREE, so a
    wrong-INDEX byte-view dispatch does NOT satisfy the soundness equation. -/
theorem byteview_wrong_index_breaks_soundness :
    byteAtFaithful envAB ≠ byteAtWrongIndex envAB := by
  -- faithful: seqIdx [10,20,30] 0 = 10 ; wrong-index: seqIdx [10,20,30] 1 = 20
  simp [byteAtFaithful, byteAtWrongIndex, refIntVal, refSeqVal, byteView, seqIdx,
        encByteAt, encArith, tokArith, arithDenote, envAB]

/-- **Teeth (negative sanity, the #127 wrong-receiver-method byte-view-dispatch
    case).** At `envAB` (`s := [10, 20, 30]`) the faithful `s.byte_at(0)` denotes byte
    `10` while the misdispatched `s.spec_len()` denotes the length `3` — they DISAGREE,
    so a wrong-RECEIVER-METHOD byte-view dispatch (the #127 name-collision) does NOT
    satisfy the soundness equation. This is the proven retirement of the #127 class on
    the contract side: the encoder's byte-view DISPATCH CHOICE is what `ref_sound` pins. -/
theorem byteview_misdispatch_breaks_soundness :
    byteAtFaithful envAB ≠ byteAtWrongMethod envAB := by
  -- faithful: seqIdx [10,20,30] 0 = 10 ; wrong-method: ([10,20,30].length : Int) = 3
  simp [byteAtFaithful, byteAtWrongMethod, refIntVal, refSeqVal, byteView, seqIdx,
        encByteAt, encLen, envAB]

/-- The faithful counterpart, for contrast: with the REAL byte-view dispatch the
    `s.byte_at(0)` clause IS sound — its encoder meaning equals the source `intVal`
    (the 0-th byte), by `refIntVal_eq_intVal`. Confirms the teeth bite ONLY the
    misdispatch, not the faithful encoder. -/
theorem byteat_faithful_intval_matches_source :
    refIntVal (Expr.byteAt (Expr.strVar "s") (Expr.intLit 0)) envAB
      = intVal (Expr.byteAt (Expr.strVar "s") (Expr.intLit 0)) envAB :=
  refIntVal_eq_intVal _ _

/-- A faithful POSITIVE witness for the `@`-view + index + subrange rewrites (#178):
    `(&xs[..2])[1]` — the prefix-then-index — has the encoder meaning EQUAL to the
    source (the 1-st element of the 2-element prefix), by `refIntVal_eq_intVal`. This
    exercises `seqVar`→`@`, `subrange`→`seqSub`, and `idx`→`seqIdx` composed, all proven
    denotation-preserving. -/
theorem subrange_index_faithful_matches_source :
    refIntVal
        (Expr.idx (Expr.subrange (Expr.seqVar "s") (RangeArg.rangeTo (Expr.intLit 2)))
          (Expr.intLit 1)) envAB
      = intVal
        (Expr.idx (Expr.subrange (Expr.seqVar "s") (RangeArg.rangeTo (Expr.intLit 2)))
          (Expr.intLit 1)) envAB :=
  refIntVal_eq_intVal _ _

/-! ## Negative sanity lemma 4 — the WRONG-COMBINATOR teeth (#179)

  The dispatch's explicit requirement (a): demonstrate that an encoder that emitted the
  WRONG combinator — `forall_in` (a bounded `∀`) lowered as `exists_in` (a bounded `∃`)
  — does NOT satisfy soundness at a concrete sequence. The two quantifier forms differ
  (`∀ i, .. → p(s[i])` vs `∃ i, .. ∧ p(s[i])`) precisely when SOME element satisfies the
  predicate and some does not. This is the combinator analogue of the `==`-vs-`<=` teeth:
  the encoder's choice of WHICH frozen `verus_l3` quantifier (`encode_call`'s
  `lookup(name)` dispatch — referencing the RIGHT combinator) is load-bearing.

  Source clause: `forall_in(s, |x| x ≤ 15)`, i.e.
    `Expr.comb forallIn (strVar "s") none none (some (Pred.mk "x" (x ≤ 15)))`.
  At `envAB` (`s := [10, 20, 30]`) the source `∀ i, 0≤i<3 → s[i] ≤ 15` is FALSE (`20 > 15`),
  while the WRONG `exists_in` form `∃ i, 0≤i<3 ∧ s[i] ≤ 15` is TRUE (`10 ≤ 15`). FALSE vs
  TRUE — they DISAGREE, so an encoder that referenced the wrong combinator does NOT satisfy
  the soundness equation. -/

/-- The flat predicate body `x ≤ 15` (the #179 combinator predicate slot's `body`). -/
def predLe15Body : Expr := Expr.cmp CmpOp.le (Expr.var "x") (Expr.intLit 15)

/-- The flat predicate closure `|x| x ≤ 15` (the #179 combinator predicate slot). -/
def predLe15 : Pred := Pred.mk "x" predLe15Body

/-- The source `forall_in(s, |x| x ≤ 15)` clause. -/
def forallInClause : Expr :=
  Expr.comb CombName.forallIn (Expr.strVar "s") none none (some predLe15)

/-- THE WRONG-COMBINATOR BUG: the encoder emits `exists_in` where the source is
    `forall_in` (a bounded `∃` for a bounded `∀` — `encode_call` referencing the wrong
    `lookup(name)` form). Modelled as `refDenote` of the `exists_in` combinator over the
    SAME slice + predicate. -/
def existsInWrong : Expr :=
  Expr.comb CombName.existsIn (Expr.strVar "s") none none (some predLe15)

/-- **Teeth (negative sanity, the wrong-combinator case, #179).** At `envAB`
    (`s := [10, 20, 30]`) the WRONG `exists_in` encoding (`∃ i, 0≤i<3 ∧ s[i] ≤ 15`, TRUE)
    is NOT equivalent to the source `forall_in` meaning (`∀ i, 0≤i<3 → s[i] ≤ 15`, FALSE),
    so an encoder that referenced the wrong combinator does NOT satisfy soundness. -/
theorem wrong_combinator_breaks_soundness :
    ¬ (refDenote existsInWrong envAB ↔ denote forallInClause envAB) := by
  -- refDenote existsInWrong = ∃ i, 0≤i<3 ∧ [10,20,30][i] ≤ 15  (TRUE, witness i = 0)
  -- denote forallInClause   = ∀ i, 0≤i<3 → [10,20,30][i] ≤ 15  (FALSE, counter i = 1)
  intro h
  have hExists : refDenote existsInWrong envAB := by
    refine ⟨0, ⟨by decide, by decide⟩, ?_⟩
    simp [predLe15Body, refDenote, refIntVal, encOp, tokRel,
          Env.bindInt, seqIdx, refSeqVal, envAB]
  have hForall := h.mp hExists
  have hAt1 : (seqIdx (envAB.seqs "s") 1) ≤ 15 := by
    have := hForall 1 ⟨by decide, by decide⟩
    simpa [predLe15Body, denote, intVal, Env.bindInt] using this
  simp [seqIdx, envAB] at hAt1

/-! ## Negative sanity lemma 5 — the #145 ARG-KIND teeth (the retired class)

  The dispatch's explicit requirement (b): demonstrate the #145 (`divergence_index_
  combinator`) bug — `forall_below`/`forall_from`'s `ArgKind::Index` bound `n` (a SCALAR
  `int`) ENCODED AS A SLICE `@`-view instead of a scalar. `encode_combinator_arg`'s `#145`
  fix dispatches `ArgKind::Index → encode_index_value` (the scalar `<n> as int`), NOT
  `encode_slice_arg` (the `@`-view). A buggy encoder that slice-`@`-viewed the index would
  produce `n@` (a Verus type error in production; on the contract side, a DIFFERENT
  quantifier bound — the LENGTH of `n`'s view rather than the scalar `n`).

  Source clause: `forall_below(s, n, |x| x ≤ 15)` with the index `n` a SCALAR. We model the
  #145 bug as the SAME `forall_below` form but with the quantifier bound taken from the
  SLICE-`@`-VIEW length of the index arg (`(refSeqVal n).length`) instead of the scalar
  `intVal n`. At an env where the scalar `n` (= 1) differs from the slice-view length
  (`n@` bound to `[10,20,30]`, length 3) and `s := [10,20,30]` with `|x| x ≤ 15`:
    - faithful bound `n = 1`: `∀ i, 0≤i<1 ∧ i<3 → s[i] ≤ 15` — only `i=0` (`10 ≤ 15`) → TRUE.
    - #145-buggy bound `= 3`: `∀ i, 0≤i<3 ∧ i<3 → s[i] ≤ 15` — `i=1` (`20 ≤ 15`) → FALSE.
  TRUE vs FALSE — they DISAGREE, so slice-`@`-viewing the Index arg breaks T1. This is the
  proven retirement of the #145 arg-kind class on the contract side: `encode_combinator_arg`
  threading `ArgKind::Index` as a SCALAR (not a `@`-view) is exactly what `ref_sound`'s
  `comb` case pins (its `forallBelow` arm uses `refIntVal_eq_intVal` on the SCALAR index). -/

/-- A concrete env for the #145 teeth: the index var `n` is the SCALAR `1`, while `n`'s
    SLICE `@`-view (the buggy reading) is `[10, 20, 30]` (length `3`); `s := [10, 20, 30]`.
    The scalar value (1) and the view-length (3) DIFFER — so a slice-`@`-viewed index is
    observable. -/
def envIdx : Env :=
  { ints := fun nm => if nm = "n" then 1 else 0
    seqs := fun nm => if nm = "s" ∨ nm = "n" then [10, 20, 30] else []
    optres := fun _ => OptResVal.none_ }

/-- The FAITHFUL `forall_below(s, n, |x| x ≤ 15)` source meaning — the `n` bound is the
    SCALAR `intVal n` (= 1), as `encode_index_value` (the #145 fix) threads it. -/
def forallBelowFaithful : Prop :=
  denote
    (Expr.comb CombName.forallBelow (Expr.strVar "s")
      none (some (Expr.var "n")) (some predLe15)) envIdx

/-- THE #145 ARG-KIND BUG: the encoder slice-`@`-views the Index arg `n` instead of
    threading it as a scalar — the quantifier bound becomes `(n@).length` (= 3), NOT the
    scalar `n` (= 1). Modelled as the `forall_below` quantifier with the bound taken from
    the slice-`@`-view length of the index arg (`refSeqVal (seqVar "n")` — the encoder
    WRONGLY dispatching `ArgKind::Index` through `encode_slice_arg`). -/
def forallBelowIndexSliceViewed : Prop :=
  let s := refSeqVal (Expr.strVar "s") envIdx
  let nBad := ((refSeqVal (Expr.seqVar "n") envIdx).length : Int)  -- the #145 `n@.len()`
  ∀ i : Int, (0 ≤ i ∧ i < nBad ∧ i < (s.length : Int)) →
    denote predLe15Body (envIdx.bindInt "x" (seqIdx s i))

/-- **Teeth (negative sanity, the #145 arg-kind case).** At `envIdx` (`n` scalar `= 1`,
    `n@` = `[10,20,30]` length `3`, `s := [10,20,30]`) the FAITHFUL `forall_below` (scalar
    bound `1`) is TRUE (`10 ≤ 15`) while the #145-buggy SLICE-`@`-viewed-index form (bound
    `3`) is FALSE (`20 ≤ 15` fails at `i = 1`) — they DISAGREE, so slice-`@`-viewing the
    `ArgKind::Index` bound breaks T1. The faithful `encode_index_value` SCALAR threading is
    what `ref_sound`'s `comb`/`forallBelow` arm pins. -/
theorem index_argkind_slice_view_breaks_soundness :
    forallBelowFaithful ≠ forallBelowIndexSliceViewed := by
  intro h
  -- forallBelowFaithful is TRUE; forallBelowIndexSliceViewed is FALSE → contradiction.
  have hF : forallBelowFaithful := by
    show ∀ i : Int,
        (0 ≤ i ∧ i < intVal (Expr.var "n") envIdx ∧ i < ((seqVal (Expr.strVar "s") envIdx).length : Int)) →
        denote predLe15Body (envIdx.bindInt "x" (seqIdx (seqVal (Expr.strVar "s") envIdx) i))
    intro i hi
    -- bound n = 1, so 0 ≤ i < 1 forces i = 0; s[0] = 10 ≤ 15.
    obtain ⟨hi0, hi1, _⟩ := hi
    have hi0eq : i = 0 := by
      simp only [intVal, envIdx] at hi1; omega
    subst hi0eq
    simp [predLe15Body, denote, intVal, Env.bindInt, seqIdx, seqVal, envIdx]
  rw [h] at hF
  -- the buggy form (bound 3) fails at i = 1 (s[1] = 20 > 15).
  have hBad := hF 1 ⟨by decide, by decide, by decide⟩
  simp [predLe15Body, denote, intVal, Env.bindInt, seqIdx, refSeqVal, envIdx] at hBad

/-- The faithful POSITIVE counterpart, for contrast: with the REAL combinator dispatch +
    the SCALAR index threading the `forall_below(s, n, |x| x ≤ 15)` clause IS sound — its
    encoder meaning is equivalent to the source, by `ref_sound`. Confirms the #179/#145
    teeth bite ONLY the wrong-combinator / slice-viewed-index, not the faithful encoder. -/
theorem forall_below_faithful_is_sound :
    refDenote
        (Expr.comb CombName.forallBelow (Expr.strVar "s")
          none (some (Expr.var "n")) (some predLe15)) envIdx
      ↔ denote
        (Expr.comb CombName.forallBelow (Expr.strVar "s")
          none (some (Expr.var "n")) (some predLe15)) envIdx :=
  ref_sound _ _

/-! ## Negative sanity lemma 6 — the #180 MATCH-ARM-SWAP teeth (the C7 match-in-ens class)

  The dispatch's explicit requirement (a): demonstrate that an encoder that SWAPPED the match
  arm bodies (the `Some`/`None` bodies exchanged — `encode_match` emitting each arm's body
  under the WRONG pattern) does NOT satisfy soundness at a concrete `OptResVal`. This is the
  match-in-ens analogue of the `==`-vs-`<=` / wrong-combinator teeth: which arm body goes under
  which pattern (`encode_match` pairing `encode_pattern(arm.pattern)` with `encode(arm.body)`) is
  load-bearing.

  Source clause: `match result { Some(v) => v == 7, None => false }`, i.e.
    `Expr.match_ (optResVar "result")
        [MatchArm.mk Some (some "v") (v == 7), MatchArm.mk None none false]`.
  At `envAB` (`result := Some 7`) the source selects the `Some` arm → `7 == 7` → TRUE.
  The SWAPPED encoder emits `match result { Some(v) => false, None => v == 7 }` (the bodies
  exchanged). At `result := Some 7` the swapped clause selects the `Some` arm → `false` → FALSE.
  TRUE vs FALSE — they DISAGREE, so an arm-body-swapping encoder does NOT satisfy soundness. -/

/-- The `Some`-arm body `v == 7` (the payload test the C7 match projects). -/
def someBodyEq7 : Expr := Expr.cmp CmpOp.eq (Expr.var "v") (Expr.intLit 7)

/-- The source `match result { Some(v) => v == 7, None => false }` clause (#180). -/
def matchSomeClause : Expr :=
  Expr.match_ (Expr.optResVar "result")
    [MatchArm.mk Variant.some_ (some "v") someBodyEq7,
     MatchArm.mk Variant.none_ none (Expr.boolLit false)]

/-- THE ARM-SWAP BUG: the encoder emits the `Some`/`None` arm BODIES exchanged — `Some(v) => false,
    None => v == 7` — a real `encode_match` infidelity (pairing each body with the WRONG pattern).
    Modelled as `refDenote` of the swapped-arm `match_` over the SAME scrutinee. -/
def matchArmSwapped : Expr :=
  Expr.match_ (Expr.optResVar "result")
    [MatchArm.mk Variant.some_ (some "v") (Expr.boolLit false),
     MatchArm.mk Variant.none_ none someBodyEq7]

/-- **Teeth (negative sanity, the #180 match-arm-swap case).** At `envAB` (`result := Some 7`)
    the source `match result { Some(v) => v == 7, None => false }` is TRUE (the `Some` arm,
    `7 == 7`) while the arm-SWAPPED encoding `match result { Some(v) => false, None => v == 7 }`
    is FALSE (the `Some` arm, `false`) — they DISAGREE, so an arm-body-swapping encoder does NOT
    satisfy the soundness equation `refDenote = denote` for this clause. This is the Lean-level
    witness that `ref_sound`'s `match_` case PINS the encoder's pattern↔body pairing. -/
theorem match_arm_swap_breaks_soundness :
    ¬ (refDenote matchArmSwapped envAB ↔ denote matchSomeClause envAB) := by
  -- denote matchSomeClause   = (Some 7 selects Some arm) → 7 = 7 → True
  -- refDenote matchArmSwapped = (Some 7 selects Some arm) → False
  simp [matchSomeClause, matchArmSwapped, someBodyEq7, refDenote, denote,
        refDenoteArms, denoteArms, scrutVal, OptResVal.variant, OptResVal.payload,
        Env.bindInt, intVal, envAB]

/-- The faithful POSITIVE counterpart, for contrast: with the REAL `encode_match` (each body under
    its OWN pattern) the `match result { Some(v) => v == 7, None => false }` clause IS sound — its
    encoder meaning is equivalent to the source, by `ref_sound`. Confirms the teeth bite ONLY the
    arm-swap, not the faithful encoder. -/
theorem match_faithful_is_sound :
    refDenote matchSomeClause envAB ↔ denote matchSomeClause envAB :=
  ref_sound _ _

/-! ## Negative sanity lemma 7 — the #180 WRONG-`is`-VARIANT teeth (the C7 `is` class)

  The dispatch's explicit requirement (b): demonstrate that an encoder that emitted the WRONG
  `is`-variant — `result is Some` lowered as `result is None` (`ref_encode.rs`'s `Expr::Is` arm
  emitting the WRONG `variant.join("::")`) — does NOT satisfy soundness at a concrete `OptResVal`.
  Which variant the discriminant tests is load-bearing.

  Source clause: `result is Some`, i.e. `Expr.is_ (optResVar "result") Variant.some_`.
  At `envAB` (`result := Some 7`) the source `is Some` is TRUE; the WRONG `result is None` is
  FALSE. TRUE vs FALSE — they DISAGREE, so an encoder that tested the wrong variant does NOT
  satisfy soundness. -/

/-- The source `result is Some` clause (#180). -/
def isSomeClause : Expr := Expr.is_ (Expr.optResVar "result") Variant.some_

/-- THE WRONG-`is`-VARIANT BUG: the encoder tests `is None` where the source tests `is Some`
    (`Expr::Is` emitting the wrong variant). Modelled as `refDenote` of the `is None` test over
    the SAME scrutinee. -/
def isNoneWrong : Expr := Expr.is_ (Expr.optResVar "result") Variant.none_

/-- **Teeth (negative sanity, the #180 wrong-`is`-variant case).** At `envAB` (`result := Some 7`)
    the WRONG `result is None` encoding (FALSE) is NOT equivalent to the source `result is Some`
    meaning (TRUE), so an encoder that tested the wrong variant does NOT satisfy soundness. This is
    the Lean-level witness that `ref_sound`'s `is_` case PINS the encoder's variant choice. -/
theorem is_wrong_variant_breaks_soundness :
    ¬ (refDenote isNoneWrong envAB ↔ denote isSomeClause envAB) := by
  -- refDenote isNoneWrong = (Some 7).isVariant None = false ; denote isSomeClause = (Some 7).isVariant Some = true
  simp [isSomeClause, isNoneWrong, refDenote, denote, scrutVal,
        OptResVal.isVariant, OptResVal.variant, envAB]

/-- The faithful POSITIVE counterpart, for contrast: with the REAL `is`-variant the `result is
    Some` clause IS sound (both `(Some 7).isVariant Some = true`), by `ref_sound`. Confirms the
    teeth bite ONLY the wrong variant, not the faithful encoder. -/
theorem is_faithful_is_sound :
    refDenote isSomeClause envAB ↔ denote isSomeClause envAB :=
  ref_sound _ _

/-- A faithful POSITIVE witness for the RESULT form (#180): `match result { Ok(v) => v == 7,
    Err(e) => e == 0 }` — the `Ok`/`Err` payload projection — has the encoder meaning EQUAL to the
    source, by `ref_sound`. Exercises the `Ok`/`Err` variant + payload-binding path (the Result
    half of the C7 fragment), confirming both Option and Result are covered. -/
theorem match_result_faithful_is_sound :
    refDenote
        (Expr.match_ (Expr.optResVar "result")
          [MatchArm.mk Variant.ok (some "v") (Expr.cmp CmpOp.eq (Expr.var "v") (Expr.intLit 7)),
           MatchArm.mk Variant.err (some "e") (Expr.cmp CmpOp.eq (Expr.var "e") (Expr.intLit 0))])
        envAB
      ↔ denote
        (Expr.match_ (Expr.optResVar "result")
          [MatchArm.mk Variant.ok (some "v") (Expr.cmp CmpOp.eq (Expr.var "v") (Expr.intLit 7)),
           MatchArm.mk Variant.err (some "e") (Expr.cmp CmpOp.eq (Expr.var "e") (Expr.intLit 0))])
        envAB :=
  ref_sound _ _

end Thermite
