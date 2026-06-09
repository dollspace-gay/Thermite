/-
  Thermite/Soundness.lean — the (T1) soundness theorem for the comparison + logical
  contract fragment (increment (a), #170; epic #169).

  Governing design: `.design/verified/thermite-semantics.md` REQ-2 (T1: the
  verified-validator obligation `∀ P, ⟦R(P)⟧ = ⟦P⟧_S`, proved by structural induction)
  + AC-2 (the theorem is NON-VACUOUS: the `==`-vs-`<=` content is the obligation's
  content). Field vocabulary (formal-methods-sota.md finding #1/#2): this is the
  VERIFIED-VALIDATOR soundness step (Leroy/CompCert), the kernel-checked core of
  SEMANTIC PRESERVATION for this fragment.

  T1 (this fragment): `∀ e env, refDenote e env ↔ denote e env`.
  `refDenote` follows the reference ENCODER's operator map (`encOp`/`encLog` mirror
  `ref_encode.rs::binop_str`); `denote` follows the SOURCE meaning. They are defined
  INDEPENDENTLY (so this is not `rfl`-vacuous) and proved equivalent by induction.
-/
import Thermite.Ast
import Thermite.Denote
import Thermite.RefEncode

namespace Thermite

/-- The integer-operand meanings coincide: the encoder's `refIntVal` equals the
    source `intVal` on every leaf (both are the identity on integer leaves). A
    standalone lemma so the comparison case of `ref_sound` can rewrite operands. -/
theorem refIntVal_eq_intVal (e : Expr) (env : Env) :
    refIntVal e env = intVal e env := by
  cases e <;> rfl

/--
  **(T1) — verified-validator soundness, comparison/logical fragment.**

  For every contract `Expr` `e` in the fragment and every environment `env`, the
  meaning of the reference encoder's output (`refDenote`, routed through the encoder's
  operator map) is LOGICALLY EQUIVALENT to the source denotation (`denote`, the
  standard `S_C` meaning). Proved by structural `induction` on `e`, one case per
  inference rule (`Thermite-semantics.md` REQ-2).

  NON-VACUOUS: `refDenote` and `denote` are defined in separate modules following
  different structure (the encoder's binop map vs the source relation); the
  `cmp`/`logic` cases discharge the `encOp`/`tokRel` round-trip per operator (the
  `==`-vs-`<=` content), NOT a definitional collapse. See `eq_le_infidelity_*` below
  for the teeth.
-/
theorem ref_sound (e : Expr) (env : Env) : refDenote e env ↔ denote e env := by
  induction e with
  | intLit n => simp [refDenote, denote]
  | boolLit b => simp [refDenote, denote]
  | var x => simp [refDenote, denote]
  | cmp op a b =>
      -- Both sides reduce to a relation over the SAME operands (refIntVal = intVal);
      -- the operator round-trip `tokRel (encOp op)` = the source relation is settled
      -- per-operator by `cases op`.
      cases op <;>
        simp [refDenote, denote, encOp, tokRel,
              refIntVal_eq_intVal a env, refIntVal_eq_intVal b env]
  | logic op a b iha ihb =>
      cases op <;> simp [refDenote, denote, encLog, tokConn, iha, ihb]
  | neg e ih =>
      simp [refDenote, denote, ih]

/-- A convenient `Prop`-equality corollary (propositional extensionality) — the
    `⟦R(P)⟧ = ⟦P⟧_S` form (T2's transitivity step composes on this equality, AC-3). -/
theorem ref_sound_eq (e : Expr) (env : Env) : refDenote e env = denote e env :=
  propext (ref_sound e env)

/-! ## The negative sanity lemma — the theorem has teeth (`==` ≠ `<=`)

  The dispatch's explicit requirement: demonstrate that an encoder that mapped
  `Eq → "<="` (the boss's `==`-vs-`<=` infidelity) would NOT satisfy the soundness
  equation. We exhibit a concrete `env` where `(a ≤ b)` and `(a = b)` disagree, so
  the `Eq`-encoded predicate interpreted with the WRONG (`leTok`) token differs from
  the source `Eq` meaning. This proves `ref_sound` genuinely PINS the operator —
  swapping the map breaks the theorem. -/

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

/-- `refDenote` with the faulty `Eq→<=` map on a comparison — the meaning the buggy
    encoder would give a `cmp Eq` clause. -/
def refDenoteFaultyCmp (op : CmpOp) (a b : Expr) (env : Env) : Prop :=
  tokRel (encOpFaulty op) (refIntVal a env) (refIntVal b env)

/-- A concrete environment: `a := 1`, `b := 2`, everything else `0`. -/
def envAB : Env := fun n => if n = "a" then 1 else if n = "b" then 2 else 0

/-- **Teeth (negative sanity, the `==`-vs-`<=` case).** At `envAB` the faulty
    `Eq→<=` encoding of `a == b` is TRUE (`1 ≤ 2`) while the source meaning of
    `a == b` is FALSE (`1 ≠ 2`) — so the faulty encoder does NOT satisfy the
    soundness equation. This is the Lean-level witness that `ref_sound` pins
    the operator: had the real `encOp` mapped `Eq` to `leTok`, the proof would
    have failed exactly here. -/
theorem eq_le_infidelity_breaks_soundness :
    ¬ (refDenoteFaultyCmp CmpOp.eq (Expr.var "a") (Expr.var "b") envAB
        ↔ denote (Expr.cmp CmpOp.eq (Expr.var "a") (Expr.var "b")) envAB) := by
  -- Faulty side: `1 ≤ 2` (True). Source side: `1 = 2` (False). They cannot be iff.
  simp [refDenoteFaultyCmp, encOpFaulty, tokRel, denote, intVal, refIntVal, envAB]

/-- The faithful counterpart, for contrast: with the REAL `encOp` the same clause
    IS sound (the `Eq` token gives `1 = 2`, matching the source `1 = 2`) — both
    False, so the iff holds. This is `ref_sound` specialized; it confirms the teeth
    bite ONLY the infidelity, not the faithful map. -/
theorem eq_faithful_is_sound :
    refDenote (Expr.cmp CmpOp.eq (Expr.var "a") (Expr.var "b")) envAB
      ↔ denote (Expr.cmp CmpOp.eq (Expr.var "a") (Expr.var "b")) envAB :=
  ref_sound _ _

end Thermite
