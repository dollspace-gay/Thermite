/-
  Thermite/Soundness.lean — the (T1) soundness theorem for the comparison + logical
  fragment (#170) EXTENDED with the ARITHMETIC operators (#176) and the CASTS (#177);
  epic #169.

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
end

/-- The integer-term meanings coincide (the projection of `refVal_eq` used by the
    `cmp`/`idx`/… cases of `ref_sound`). -/
theorem refIntVal_eq_intVal (e : Expr) (env : Env) :
    refIntVal e env = intVal e env := (refVal_eq e env).1

/-- The sequence-term meanings coincide (the `@`-view/`subrange` projection of
    `refVal_eq`). -/
theorem refSeqVal_eq_seqVal (e : Expr) (env : Env) :
    refSeqVal e env = seqVal e env := (refVal_eq e env).2

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
  -- `Expr` is mutually inductive (with `RangeArg`), so `induction` is unavailable;
  -- proceed by `cases` with RECURSIVE `ref_sound` calls on the predicate subterms
  -- (`logic`/`neg`) — the recursion IS the structural induction Lean checks.
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
    seqs := fun s => if s = "s" then [10, 20, 30] else [] }

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

end Thermite
