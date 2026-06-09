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

/-- The integer-term meanings coincide: the encoder's `refIntVal` equals the source
    `intVal` on every integer-sorted term. Now PROVED BY INDUCTION (the `arith`/`cast`
    cases are recursive — the inductive hypotheses settle the operands/inner, and the
    operator/cast-target round-trips `tokArith_encArith`/`tokCast_encCast` settle the
    head). A standalone lemma so the comparison case of `ref_sound` can rewrite
    operands. -/
theorem refIntVal_eq_intVal (e : Expr) (env : Env) :
    refIntVal e env = intVal e env := by
  induction e with
  | intLit n => rfl
  | boolLit b => rfl
  | var x => rfl
  | cmp op a b _ _ => rfl
  | logic op a b _ _ => rfl
  | neg e _ => rfl
  | arith op a b iha ihb =>
      -- `refIntVal (arith ..) = tokArith (encArith op) (refIntVal a) (refIntVal b)`;
      -- rewrite operands by the IHs, then the operator round-trip.
      simp only [refIntVal, intVal, iha, ihb, tokArith_encArith]
  | cast inner ty ih =>
      -- `refIntVal (cast ..) = tokCast (encCast ty) (refIntVal inner)`; rewrite the
      -- inner by the IH (THIS is where the faithful WHOLE-inner paren matters), then
      -- the cast-target round-trip.
      simp only [refIntVal, intVal, ih, tokCast_encCast]

/--
  **(T1) — verified-validator soundness, comparison/logical/arithmetic/cast fragment.**

  For every contract `Expr` `e` in the fragment and every environment `env`, the
  meaning of the reference encoder's output (`refDenote`, routed through the encoder's
  operator + cast-target maps and its parenthesization) is LOGICALLY EQUIVALENT to the
  source denotation (`denote`, the standard `S_C` meaning). Proved by structural
  `induction` on `e`, one case per inference rule (`thermite-semantics.md` REQ-2).

  NON-VACUOUS: `refDenote`/`refIntVal` and `denote`/`intVal` are defined in separate
  modules following different structure (the encoder's binop/cast maps + paren vs the
  source relation/arithmetic); the `cmp` case discharges the `encOp`/`tokRel`
  round-trip per operator AND the integer-operand equality `refIntVal_eq_intVal` —
  which itself carries the #176 arithmetic round-trip and the #177 cast round-trip,
  NOT a definitional collapse. See `eq_le_infidelity_*` (the `==`-vs-`<=` teeth) and
  `cast_paren_drop_breaks_soundness` (the #122/#146 cast-paren teeth) below.
-/
theorem ref_sound (e : Expr) (env : Env) : refDenote e env ↔ denote e env := by
  induction e with
  | intLit n => simp [refDenote, denote]
  | boolLit b => simp [refDenote, denote]
  | var x => simp [refDenote, denote]
  | cmp op a b =>
      -- Both sides reduce to a relation over the SAME operands (refIntVal = intVal,
      -- now incl. the arith/cast subterms); the operator round-trip
      -- `tokRel (encOp op)` = the source relation is settled per-operator by `cases`.
      cases op <;>
        simp [refDenote, denote, encOp, tokRel,
              refIntVal_eq_intVal a env, refIntVal_eq_intVal b env]
  | logic op a b iha ihb =>
      cases op <;> simp [refDenote, denote, encLog, tokConn, iha, ihb]
  | neg e ih =>
      simp [refDenote, denote, ih]
  | arith op a b _ _ =>
      -- An arithmetic term is integer-sorted: as a TOP-LEVEL predicate it is not a
      -- well-formed clause, so both `refDenote` and `denote` fall to `True`
      -- (it only ever appears as a comparison operand, handled in the `cmp` case via
      -- `refIntVal_eq_intVal`). The iff is reflexive here.
      simp [refDenote, denote]
  | cast inner ty _ =>
      -- Likewise a cast term is integer-sorted; top-level it is `True ↔ True`.
      simp [refDenote, denote]

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

/-- A concrete environment: `a := 1`, `b := 2`, `n := -1`, everything else `0`. -/
def envAB : Env := fun s => if s = "a" then 1 else if s = "b" then 2
                            else if s = "n" then -1 else 0

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

end Thermite
