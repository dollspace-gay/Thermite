/-
  Thermite/RefEncode.lean — a Lean model of the REFERENCE ENCODER's output as a
  denotation, for the comparison + logical fragment (#170) EXTENDED with the
  ARITHMETIC operators (#176), the CASTS (#177), and the SPEC-CONTEXT REWRITES
  (#178 — slice→`@`/subrange, indexing, the `String` byte-view DISPATCH, the #127 class).

  WHAT THE #178 REWRITES MODEL (faithful to `thermite-tv/src/ref_encode.rs`). In
  contract position the encoder REWRITES slice/`String` use sites; this module models
  the MEANING of the rewritten Verus the encoder produces, FAITHFULLY:

    - slice var `xs` → `xs@`  (`encode_slice_arg`'s `Expr::Path` arm): the `@`-view is
      the IDENTITY on the sequence value (`refSeqVal (seqVar x) = env.seqs x`).
    - `xs[i]` → `xs@[i as int]`  (`encode_index`'s `IndexArg::Single` arm over
      `encode_receiver`'s `@`-view): the i-th element — `seqIdx (view) i`.
    - `&xs[..i]` → `xs@.subrange(0, i as int)`  (`encode_ref`→`encode_index`'s `RangeTo`
      arm); `&xs[a..b]` → `.subrange(a, b)` (`Range`); `&xs[a..]` →
      `.subrange(a, xs@.len() as int)` (`RangeFrom`): the contiguous sub-sequence —
      `seqSub`.
    - `s.byte_at(i)` → `s.spec_byte_at(i)`  (`encode_string_byteview`'s `byte_at` arm —
      the #127 byte-view DISPATCH): the i-th byte — `seqIdx (bytes) i`.
    - `s.len()` → `s.spec_len()`  (`encode_string_byteview`'s `len` arm) / `xs@.len()`
      (`encode_method_call`'s `len` arm): the length — `(view).length`.

  THE #127 DISPATCH is modelled as an EXPLICIT STEP (`encByteView` : the method name →
  the byte-view spec fn). THE FAITHFULNESS DECISION POINT for #178 lives there: a
  misdispatch (`byte_at`→a wrong index, OR `byte_at`→the length spec fn) is a DIFFERENT
  meaning — the negative lemma `byteview_misdispatch_breaks_soundness` shows it FAILS
  soundness at a concrete sequence env (mirroring `cast_paren_drop_breaks_soundness`).

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

/-- A Verus BYTE-VIEW SPEC-FN TOKEN — the thing `encode_string_byteview` dispatches a
    `String`-receiver method to (`.spec_byte_at(i)` for the i-th byte, `.spec_len()` for
    the length), modelled as a Lean datum so the encoder's byte-view DISPATCH CHOICE is
    an explicit, independent step. THE #127 FAITHFULNESS DECISION POINT: a misdispatch
    (the name-collision bug — `byte_at` routed to the wrong spec fn / index) lives HERE.
    `specByteAt` reads the i-th byte; `specLen` reads the length (a `slice`/`Seq`
    receiver's `.len()`→`recv.len()` is the same length meaning). -/
inductive VerusByteView where
  | specByteAt  -- "spec_byte_at(i)" — the i-th byte (Seq index over the byte view)
  | specLen     -- "spec_len()" / ".len()" — the length
  deriving DecidableEq, Repr

/-- The encoder's BYTE-VIEW DISPATCH for the `byte_at` method — mirrors
    `encode_string_byteview`'s `"byte_at" => …spec_byte_at(idx)` arm. THE #127
    FAITHFULNESS DECISION POINT: routes `byte_at` to the i-th-byte spec fn. -/
def encByteAt : VerusByteView := VerusByteView.specByteAt

/-- The encoder's BYTE-VIEW DISPATCH for the `len` method — mirrors
    `encode_string_byteview`'s `"len" => …spec_len()` arm (and `encode_method_call`'s
    `"len"` slice arm). Routes `len` to the length spec fn. -/
def encLen : VerusByteView := VerusByteView.specLen

/-- The standard-model interpretation of a Verus BYTE-VIEW spec fn over a sequence and
    an index — the meaning `⟦·⟧` of the emitted `.spec_byte_at(i)` / `.spec_len()`.
    `specByteAt` reads the i-th byte (`seqIdx`, the shared total access); `specLen`
    reads the length (the index argument is ignored). Routes through the SHARED
    `seqIdx`/`List.length` (`Denote.lean`) — the dispatch token determines WHICH read,
    so the encoder's #127 faithfulness is `byteView encByteAt = the byte`/`byteView
    encLen = the length`, the property the soundness theorem discharges. -/
def byteView : VerusByteView → List Int → Int → Int
  | VerusByteView.specByteAt, s, i => seqIdx s i
  | VerusByteView.specLen,    s, _ => (s.length : Int)

/- The integer-operand meaning the encoder assigns to a term (`refIntVal`) + the
   sequence-valued meaning (`refSeqVal`), MUTUAL (#178: `idx`/`subrange` cross sorts).
   `ref_encode.rs` emits `value.to_string()` for an `IntLit`, the joined path for a
   `var`, the parenthesized `({l} {op} {r})` for an arithmetic binary
   (`encode_binary`'s non-comparison arm), `({inner}) as {target}` for a cast
   (`encode_cast`), the indexed `recv[idx]` for `xs[i]` (`encode_index`'s `Single`
   arm), and the byte-view spec fns for the `String` method calls
   (`encode_string_byteview`). Structured as ENCODE-THEN-INTERPRET on the OPERATOR /
   CAST-TARGET / BYTE-VIEW map, following the ENCODER:

   - `arith`: `tokArith (encArith op)` of the two re-encoded operands — the encoder
     wholly parenthesizes a binary (`encode_binary`: `format!("({l} {} {r})")`), so
     the operands are bound correctly and the operator map is the only content.
   - `cast`: `tokCast (encCast ty)` of the re-encoded WHOLE inner — the encoder
     wraps the inner in parens (`encode_cast`: `format!("({e}) as {target}")`), so
     the cast binds the entire `refIntVal inner` (no re-association). THE #122
     DISCIPLINE: dropping that paren would make a compound inner re-parse and the
     cast bind only a sub-term (the negative lemma).
   - `idx`: `byteView encByteAt (refSeqVal base) (refIntVal i)` — `encode_index` emits
     `recv[idx]` over the receiver's `@`-view (the index `as int`); the meaning is the
     i-th element of the view (`refSeqVal` is the same sequence — the `@`-view is the
     identity). #178.
   - `seqLen`: `byteView encLen (refSeqVal base) 0` — the encoder dispatches
     `len`→`spec_len()` / `.len()`; the meaning is the length of the view. #178.
   - `byteAt`: `byteView encByteAt (refSeqVal base) (refIntVal i)` — the encoder
     dispatches `byte_at`→`spec_byte_at(i)`; the i-th byte. THE #127 class: the
     dispatch CHOICE is the content (the negative lemma mis-dispatches). -/
mutual
/-- The integer-operand meaning the encoder assigns to a term. See the block comment. -/
def refIntVal : Expr → Env → Int
  | Expr.intLit n,      _   => n
  | Expr.var x,         env => env.ints x
  | Expr.arith op a b,  env => tokArith (encArith op) (refIntVal a env) (refIntVal b env)
  | Expr.cast inner ty, env => tokCast (encCast ty) (refIntVal inner env)
  | Expr.idx base i,    env => byteView encByteAt (refSeqVal base env) (refIntVal i env)
  | Expr.seqLen base,   env => byteView encLen (refSeqVal base env) 0
  | Expr.byteAt base i, env => byteView encByteAt (refSeqVal base env) (refIntVal i env)
  | _, _ => 0

/-- The encoder's SEQUENCE-valued meaning (#178): a slice var `xs`→`xs@` is the
    IDENTITY on the sequence value (`encode_slice_arg`); a `String` byte receiver is
    emitted bare and its bytes are the same sequence; a `subrange` is the encoder's
    `.subrange(lo, hi)` (`encode_index`'s range arms), `seqSub` over the re-encoded
    base. Routes through the SAME `seqSub` as the source — the rewrite preserves the
    sub-sequence. Mutual with `refIntVal` (`subrange`'s bounds are integer terms). -/
def refSeqVal : Expr → Env → List Int
  | Expr.seqVar x, env => env.seqs x
  | Expr.strVar x, env => env.seqs x
  | Expr.subrange base r, env =>
      let s := refSeqVal base env
      match r with
      | RangeArg.rangeTo hi    => seqSub s 0 (refIntVal hi env)
      | RangeArg.range lo hi   => seqSub s (refIntVal lo env) (refIntVal hi env)
      | RangeArg.rangeFrom lo  => seqSub s (refIntVal lo env) (s.length : Int)
  | _, _ => []
end

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
  -- The 6 BOUNDED-QUANTIFIER combinators (#179). The encoder REUSES the SHARED frozen
  -- `lookup(C).verus_l3` quantifier BODY verbatim (`encode_combinator_call` emits
  -- `name(args)` to the registry `spec fn` — the body is the shared ground truth, NOT
  -- re-implemented), so the quantifier FORM here is structurally identical to `denote`'s.
  -- What the encoder RE-implements (the faithfulness surface, `encode_combinator_arg`) is
  -- the per-arg-kind THREADING:
  --   - `ArgKind::Slice` → `encode_slice_arg`'s `@`-view = `refSeqVal` (the identity on
  --     the sequence value).
  --   - `ArgKind::Index` → `encode_index_value`'s SCALAR `<n> as int` = `refIntVal` of a
  --     scalar — NEVER the slice `@`-view. THE #145 FIX (`forall_below`/`forall_from`'s
  --     `n: int`; `n@` would be a Verus type error / a wrong meaning — the negative lemma).
  --   - `ArgKind::Pred` → `encode_pred_arg`'s `|x: u32| <body>`, the body re-encoded by the
  --     SAME independent `refDenote`/`refIntVal` recursion (so a predicate infidelity is
  --     caught), applied at the i-th element via the SHARED `Env.bindInt`.
  | Expr.comb c seq seq2 idx pred, env =>
      let s := refSeqVal seq env
      let s2 := match seq2 with | some e => refSeqVal e env | none => []
      let n := match idx with | some e => refIntVal e env | none => 0
      let p : Int → Prop := fun i =>
        match pred with
        | some (Pred.mk bound body) => refDenote body (env.bindInt bound (seqIdx s i))
        | none => True
      match c with
      | CombName.forallIn =>
          ∀ i : Int, (0 ≤ i ∧ i < (s.length : Int)) → p i
      | CombName.existsIn =>
          ∃ i : Int, (0 ≤ i ∧ i < (s.length : Int)) ∧ p i
      | CombName.sorted =>
          ∀ i j : Int, (0 ≤ i ∧ i ≤ j ∧ j < (s.length : Int)) → seqIdx s i ≤ seqIdx s j
      | CombName.forallBelow =>
          ∀ i : Int, (0 ≤ i ∧ i < n ∧ i < (s.length : Int)) → p i
      | CombName.forallFrom =>
          ∀ i : Int, (n ≤ i ∧ i < (s.length : Int)) → p i
      | CombName.disjoint =>
          ∀ i j : Int,
            ((0 ≤ i ∧ i < (s.length : Int)) ∧ (0 ≤ j ∧ j < (s2.length : Int))) →
              seqIdx s i ≠ seqIdx s2 j
  | _, _ => True

end Thermite
