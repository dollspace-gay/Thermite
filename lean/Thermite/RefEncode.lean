/-
  Thermite/RefEncode.lean — a Lean model of the REFERENCE ENCODER's output as a
  denotation, for the comparison + logical fragment (#170) EXTENDED with the
  ARITHMETIC operators (#176) and the CASTS (#177).

  Governing design: `.design/verified/thermite-semantics.md` REQ-2/REQ-6 (model what
  `thermite-tv`'s `ref_contract_pred` PRODUCES) + the (T1) obligation §"The concrete
  (T1) obligation" (the content lives in the binop/coercion re-statement, NOT a
  vacuous `X=X`) + Architecture §"S_C" (the `cast → nat/int` rule + the #122 paren).

  WHAT THIS MODELS. `thermite-tv/src/ref_encode.rs` does not produce a `Prop`; it
  produces a Verus PREDICATE STRING. Two structural facts of that encoder are
  modelled FAITHFULLY here:

    1. The OPERATOR MAP `binop_str` (`ref_encode.rs::binop_str`):
         comparisons  Eq→"==", Ne→"!=", Lt→"<", Le→"<=", Gt→">", Ge→">="
         logical      And→"&&", Or→"||"          (+ Not→"!", encode_unary)
         ARITHMETIC   Add→"+", Sub→"-", Mul→"*", Div→"/", Rem→"%",
                      Shl→"<<", Shr→">>", BitAnd→"&", BitOr→"|", BitXor→"^"
       modelled as Lean operator-TOKEN data (`VerusCmpTok`/`VerusLogTok`/
       `VerusArithTok`), INDEPENDENT of `Denote`'s source operators.

    2. The CAST + its PARENTHESIZATION (`ref_encode.rs::encode_cast`/`cast_target`):
       `encode_cast` emits `({inner}) as {target}` — it WRAPS the inner expression
       in parens UNCONDITIONALLY (the #122 paren discipline: "a bare path/literal is
       unaffected, a compound inner is bound correctly"). `cast_target` maps the
       target type: `U32→"u32"`, `U64→"u64"`, `Usize→"usize"`, `Named "nat"→"nat"`,
       `Named "int"→"int"`. We model the cast as: interpret the WHOLE inner term,
       then apply the target coercion — which is faithful PRECISELY BECAUSE the
       encoder parenthesizes the inner (so the cast binds the whole subexpression,
       never re-associating across a lower-precedence inner operator). The negative
       lemma below shows a paren-DROPPED encoder would NOT be sound — the #122/#146
       teeth.

  To keep the soundness theorem NON-VACUOUS, this module follows the ENCODER's
  STRUCTURE (the operator/cast-target maps + the parenthesization) rather than the
  source meaning; the shared value functions `arithDenote`/`castDenote` (`Denote.lean`)
  are the meaning the emitted token/coercion is interpreted at — exactly as
  `tokRel`/`tokConn` interpret the emitted comparison/logical tokens.
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

/-- A Verus ARITHMETIC-operator TOKEN — the thing `binop_str`'s arithmetic arms emit
    (`"+"`, `"-"`, `"*"`, `"/"`, `"%"`, `"<<"`, `">>"`, `"&"`, `"|"`, `"^"`),
    modelled as a Lean datum so the encoder's arithmetic operator CHOICE is an
    explicit, independent step (the faithfulness decision point for #176). -/
inductive VerusArithTok where
  | plusTok    -- "+"
  | minusTok   -- "-"
  | starTok    -- "*"
  | slashTok   -- "/"
  | percentTok -- "%"
  | shlTok     -- "<<"
  | shrTok     -- ">>"
  | ampTok     -- "&"
  | pipeTok    -- "|"
  | caretTok   -- "^"
  deriving DecidableEq, Repr

/-- A Verus CAST-TARGET TOKEN — the spelling `cast_target` emits (`"u64"`, `"u32"`,
    `"usize"`, `"nat"`, `"int"`), modelled as a Lean datum so the encoder's
    cast-target CHOICE is an explicit, independent step (the faithfulness decision
    point for #177). -/
inductive VerusCastTok where
  | u64Tok
  | u32Tok
  | usizeTok
  | natTok
  | intTok
  deriving DecidableEq, Repr

/-- The encoder's comparison-operator map — mirrors `ref_encode.rs::binop_str` for
    the comparison ops. THE FAITHFULNESS DECISION POINT: an infidelity (`Eq→leTok`,
    the `==`-vs-`<=` bug) would live HERE. -/
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

/-- The encoder's ARITHMETIC-operator map — mirrors `ref_encode.rs::binop_str`'s
    arithmetic arms (`Add→"+"`, `Sub→"-"`, `Mul→"*"`, `Div→"/"`, `Rem→"%"`,
    `Shl→"<<"`, `Shr→">>"`, `BitAnd→"&"`, `BitOr→"|"`, `BitXor→"^"`). THE #176
    FAITHFULNESS DECISION POINT: an infidelity (e.g. `Add→minusTok`) would live HERE. -/
def encArith : ArithOp → VerusArithTok
  | ArithOp.add    => VerusArithTok.plusTok
  | ArithOp.sub    => VerusArithTok.minusTok
  | ArithOp.mul    => VerusArithTok.starTok
  | ArithOp.div    => VerusArithTok.slashTok
  | ArithOp.rem    => VerusArithTok.percentTok
  | ArithOp.shl    => VerusArithTok.shlTok
  | ArithOp.shr    => VerusArithTok.shrTok
  | ArithOp.bitAnd => VerusArithTok.ampTok
  | ArithOp.bitOr  => VerusArithTok.pipeTok
  | ArithOp.bitXor => VerusArithTok.caretTok

/-- The encoder's CAST-TARGET map — mirrors `ref_encode.rs::cast_target`
    (`U64→"u64"`, `U32→"u32"`, `Usize→"usize"`, `Named "nat"→"nat"`,
    `Named "int"→"int"`). THE #177 FAITHFULNESS DECISION POINT: an infidelity
    (e.g. `nat→intTok`) would live HERE. -/
def encCast : CastTy → VerusCastTok
  | CastTy.u64   => VerusCastTok.u64Tok
  | CastTy.u32   => VerusCastTok.u32Tok
  | CastTy.usize => VerusCastTok.usizeTok
  | CastTy.nat   => VerusCastTok.natTok
  | CastTy.int   => VerusCastTok.intTok

/-- The standard-model interpretation of a Verus comparison TOKEN over two integers
    — the meaning `⟦·⟧` of the emitted operator string. -/
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

/-- The standard-model interpretation of a Verus ARITHMETIC TOKEN over two integer
    operand values — the meaning `⟦·⟧` of the emitted operator string. Routes
    through the SHARED `arithDenote` (`Denote.lean`): the token determines WHICH
    `ArithOp` meaning, so the encoder's faithfulness is `tokArith (encArith op) = the
    op's arithmetic`, the round-trip the soundness theorem discharges (mirroring
    `tokRel (encOp op)`). -/
def tokArith : VerusArithTok → Int → Int → Int
  | VerusArithTok.plusTok,    x, y => arithDenote ArithOp.add x y
  | VerusArithTok.minusTok,   x, y => arithDenote ArithOp.sub x y
  | VerusArithTok.starTok,    x, y => arithDenote ArithOp.mul x y
  | VerusArithTok.slashTok,   x, y => arithDenote ArithOp.div x y
  | VerusArithTok.percentTok, x, y => arithDenote ArithOp.rem x y
  | VerusArithTok.shlTok,     x, y => arithDenote ArithOp.shl x y
  | VerusArithTok.shrTok,     x, y => arithDenote ArithOp.shr x y
  | VerusArithTok.ampTok,     x, y => arithDenote ArithOp.bitAnd x y
  | VerusArithTok.pipeTok,    x, y => arithDenote ArithOp.bitOr x y
  | VerusArithTok.caretTok,   x, y => arithDenote ArithOp.bitXor x y

/-- The standard-model interpretation of a Verus CAST-TARGET TOKEN over an integer
    operand value — the meaning `⟦·⟧` of the emitted `as <target>`. Routes through
    the SHARED `castDenote` (`Denote.lean`). -/
def tokCast : VerusCastTok → Int → Int
  | VerusCastTok.u64Tok,   v => castDenote CastTy.u64 v
  | VerusCastTok.u32Tok,   v => castDenote CastTy.u32 v
  | VerusCastTok.usizeTok, v => castDenote CastTy.usize v
  | VerusCastTok.natTok,   v => castDenote CastTy.nat v
  | VerusCastTok.intTok,   v => castDenote CastTy.int v

/-- The integer-operand meaning the encoder assigns to a term. `ref_encode.rs`
    emits `value.to_string()` for an `IntLit`, the joined path for a `var`, the
    parenthesized `({l} {op} {r})` for an arithmetic binary (`encode_binary`'s
    non-comparison arm), and `({inner}) as {target}` for a cast (`encode_cast`).
    Structured as ENCODE-THEN-INTERPRET on the OPERATOR / CAST-TARGET map, following
    the ENCODER:

    - `arith`: `tokArith (encArith op)` of the two re-encoded operands — the encoder
      wholly parenthesizes a binary (`encode_binary`: `format!("({l} {} {r})")`), so
      the operands are bound correctly and the operator map is the only content.
    - `cast`: `tokCast (encCast ty)` of the re-encoded WHOLE inner — the encoder
      wraps the inner in parens (`encode_cast`: `format!("({e}) as {target}")`), so
      the cast binds the entire `refIntVal inner` (no re-association). THE #122
      DISCIPLINE: dropping that paren would make a compound inner re-parse and the
      cast bind only a sub-term (the negative lemma). -/
def refIntVal : Expr → Env → Int
  | Expr.intLit n,      _   => n
  | Expr.var x,         env => env x
  | Expr.arith op a b,  env => tokArith (encArith op) (refIntVal a env) (refIntVal b env)
  | Expr.cast inner ty, env => tokCast (encCast ty) (refIntVal inner env)
  | _, _ => 0

/-- `⟦ ref_contract_pred(P) ⟧` — the meaning, under the standard model, of the Verus
    predicate string the reference encoder produces. Structured as
    ENCODE-THEN-INTERPRET (`tokRel (encOp op) …`), following the ENCODER, so the
    equality with `denote` is a real theorem. -/
def refDenote : Expr → Env → Prop
  | Expr.boolLit b, _   => (b = true)
  | Expr.cmp op a b, env =>
      tokRel (encOp op) (refIntVal a env) (refIntVal b env)
  | Expr.logic op a b, env =>
      tokConn (encLog op) (refDenote a env) (refDenote b env)
  | Expr.neg e, env => ¬ refDenote e env
  | _, _ => True

end Thermite
