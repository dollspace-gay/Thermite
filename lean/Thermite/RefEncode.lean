/-
  Thermite/RefEncode.lean — a Lean model of the REFERENCE ENCODER's output as a
  denotation, for the comparison + logical fragment (increment (a), #170).

  Governing design: `.design/verified/thermite-semantics.md` REQ-2/REQ-6 (model what
  `thermite-tv`'s `ref_contract_pred` PRODUCES) + the (T1) obligation §"The concrete
  (T1) obligation" (the content lives in the binop re-statement, NOT a vacuous `X=X`).

  WHAT THIS MODELS. `thermite-tv/src/ref_encode.rs` does not produce a `Prop`; it
  produces a Verus PREDICATE STRING via the 1-to-1 operator map `binop_str`:
      Eq→"==", Ne→"!=", Lt→"<", Le→"<=", Gt→">", Ge→">=", And→"&&", Or→"||"  (+ Not→"!")
  and `⟦·⟧` of that string is its meaning under the standard Verus/vstd model.

  To keep the soundness theorem NON-VACUOUS (the dispatch's explicit requirement:
  `refDenote` and `denote` are defined INDEPENDENTLY and PROVED equal, NOT identical
  by definition / `rfl`), this module follows the ENCODER's STRUCTURE rather than the
  source meaning:

    1. `encOp` / `encLog` mirror `ref_encode.rs::binop_str` — the encoder's
       OPERATOR CHOICE, modelled as a Lean datum (`VerusCmpTok` / `VerusLogTok`),
       INDEPENDENT of `Denote.denote`'s source relations. THIS is the
       `==`-vs-`<=` decision point at the Lean level.
    2. `tokRel` / `tokConn` interpret a Verus operator TOKEN under the standard
       model — the meaning `⟦·⟧` assigns to the emitted string.
    3. `refDenote` = "encode the operator (1), then interpret the token (2)".

  So `refDenote` routes through the encoder's binop map; `denote` routes through the
  source relation. They COINCIDE only because the map is faithful — which is exactly
  what `Soundness.ref_sound` proves (and what the negative `==`-vs-`<=` lemma shows
  would FAIL if the map were `Eq→<=`).
-/
import Thermite.Ast
import Thermite.Denote

namespace Thermite

/-- A Verus comparison-operator TOKEN — the thing `binop_str` emits into the
    predicate string (`"=="`, `"<="`, …), modelled as a Lean datum so the encoder's
    operator CHOICE is an explicit, independent step. -/
inductive VerusCmpTok where
  | eqTok   -- "=="
  | neTok   -- "!="
  | ltTok   -- "<"
  | leTok   -- "<="
  | gtTok   -- ">"
  | geTok   -- ">="
  deriving DecidableEq, Repr

/-- A Verus logical-connective TOKEN (`"&&"`, `"||"`). -/
inductive VerusLogTok where
  | andTok  -- "&&"
  | orTok   -- "||"
  deriving DecidableEq, Repr

/-- The encoder's comparison-operator map — mirrors `ref_encode.rs::binop_str` for
    the comparison ops (`Eq→"=="`, `Ne→"!="`, `Lt→"<"`, `Le→"<="`, `Gt→">"`,
    `Ge→">="`). THE FAITHFULNESS DECISION POINT: an infidelity (`Eq→leTok`, the
    `==`-vs-`<=` bug) would live HERE. -/
def encOp : CmpOp → VerusCmpTok
  | CmpOp.eq => VerusCmpTok.eqTok
  | CmpOp.ne => VerusCmpTok.neTok
  | CmpOp.lt => VerusCmpTok.ltTok
  | CmpOp.le => VerusCmpTok.leTok
  | CmpOp.gt => VerusCmpTok.gtTok
  | CmpOp.ge => VerusCmpTok.geTok

/-- The encoder's logical-connective map (`And→"&&"`, `Or→"||"`). -/
def encLog : LogOp → VerusLogTok
  | LogOp.and => VerusLogTok.andTok
  | LogOp.or  => VerusLogTok.orTok

/-- The standard-model interpretation of a Verus comparison TOKEN over two integers
    — the meaning `⟦·⟧` of the emitted operator string. Defined independently of
    `CmpOp` (it ranges over the TOKEN, the encoder's output alphabet). -/
def tokRel : VerusCmpTok → Int → Int → Prop
  | VerusCmpTok.eqTok, x, y => x = y
  | VerusCmpTok.neTok, x, y => x ≠ y
  | VerusCmpTok.ltTok, x, y => x < y
  | VerusCmpTok.leTok, x, y => x ≤ y
  | VerusCmpTok.gtTok, x, y => x > y
  | VerusCmpTok.geTok, x, y => x ≥ y

/-- The standard-model interpretation of a Verus connective TOKEN. -/
def tokConn : VerusLogTok → Prop → Prop → Prop
  | VerusLogTok.andTok, p, q => p ∧ q
  | VerusLogTok.orTok,  p, q => p ∨ q

/-- The integer-operand meaning the encoder assigns to a leaf. `ref_encode.rs`
    emits `value.to_string()` for an `IntLit` and the joined path for a `var` — i.e.
    the operand string is the SAME integer leaf the source denotes. Modelled directly
    (the leaf encoding is the identity on integer leaves; the fragment's content is in
    the OPERATOR map, not the leaves). -/
def refIntVal : Expr → Env → Int
  | Expr.intLit n, _   => n
  | Expr.var x,    env => env x
  | _, _ => 0

/-- `⟦ ref_contract_pred(P) ⟧` — the meaning, under the standard model, of the Verus
    predicate string the reference encoder produces, for the comparison/logical
    fragment. Structured as ENCODE-THEN-INTERPRET (`tokRel (encOp op) …`), following
    the ENCODER, so the equality with `denote` is a real theorem. -/
def refDenote : Expr → Env → Prop
  | Expr.boolLit b, _   => (b = true)
  | Expr.cmp op a b, env =>
      tokRel (encOp op) (refIntVal a env) (refIntVal b env)
  | Expr.logic op a b, env =>
      tokConn (encLog op) (refDenote a env) (refDenote b env)
  | Expr.neg e, env => ¬ refDenote e env
  | _, _ => True

end Thermite
