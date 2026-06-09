/-
  Thermite/Denote.lean — the SOURCE denotation `⟦·⟧_{S_C}` for the comparison +
  logical contract fragment (increment (a), #170) EXTENDED with the ARITHMETIC
  operators (#176), the CASTS (#177), the SPEC-CONTEXT REWRITES (#178 — slice→
  `@`/subrange, indexing, the `String` byte-view length/byte-at), and the 6
  BOUNDED-QUANTIFIER COMBINATORS (#179 / 1d-i — `forall_in`/`exists_in`/`sorted`/
  `forall_below`/`forall_from`/`disjoint`, each denoting its FROZEN `verus_l3`
  quantifier form from `thermite-spec/src/combinators.rs`, with the predicate closure
  `p(s[i])` applied at the i-th element via `Env.bindInt`), and the MATCH-IN-ENS / `is`
  PAYLOAD-IN-CONTRACT forms (#180 / 1g — `match scrut { Some(v) => P(v), None => Q }` /
  `Ok`/`Err`, and `scrut is Variant`: the arm SELECTED by the scrutinee's `OptResVal` variant
  denotes its body with the payload BOUND via `Env.bindInt`; the `is`-test reads the variant).

  THE OPTION/RESULT ENVIRONMENT (#180). A `match`/`is` scrutinee (a param / `result` of a
  built-in `Option`/`Result` type) denotes an `OptResVal` (`none`/`some v`/`ok v`/`err e`, the
  payload an `Int` — the C7 corpus shape). The `Env` gains an `optres` map. The SOURCE meaning:
  `match scrut arms` = the body of the arm whose pattern variant matches `scrut`'s variant,
  denoted with the payload bound (`denoteArms`); `scrut is V` = `scrut`'s variant is `V`.

  THE SEQUENCE ENVIRONMENT (#178). A slice param `xs: &[u32]` and a `String`'s bytes
  denote a SEQUENCE (`List Int`), not a scalar. The `Env` is therefore extended to map
  free SEQUENCE names to `List Int` alongside the integer names. The SOURCE meaning
  (BEFORE the rewrite): `xs[i]` = the i-th element; `&xs[..i]` = the prefix
  `subrange(0,i)`; `s.byte_at(i)` = the i-th byte; `s.len()` = the length.

  THE SEQUENCE-ACCESS PARTIALITY CONVENTION (#178). An index `xs[i]` carries a source
  obligation `0 ≤ i < |xs|` (Verus rejects an out-of-bounds spec index — an L0/source
  precondition, exactly like the div-by-zero of #176). We model the access with the
  TOTAL `List.getD … 0` (out-of-range → `0`). This is sound for `S_C` for the SAME
  reason as the arithmetic partial point: the in-range obligation is a SOURCE-side
  precondition, and — load-bearing for T1 — BOTH `denote`/`intVal` (source) and
  `RefEncode.refDenote`/`refIntVal` (encoder) route the access through the SAME total
  `seqIdx` over the SAME `List` (the `@`-view is the identity on the value), so the
  soundness equation holds regardless of the out-of-range value. The same holds for
  `subrange` (`List.take`/`List.drop`, total) and `seqLen` (`List.length`, total).

  Governing design: `.design/verified/thermite-semantics.md` Architecture §"S_C —
  the spec/contract sublanguage", REQ-1. This is `S_C` RESTRICTED to the fragment
  `Ast.lean` embeds: comparisons/logical denote the corresponding math relation
  (the STANDARD meaning), `Eq → =`, `Le → ≤`, `And → ∧`, `Not → ¬`; ARITHMETIC ops
  denote `int` arithmetic over the operand VALUES (`Add → +`, `Sub → -`, …; the
  doc's `⟦Binary(Add,a,b)⟧ = ⟦a⟧ + ⟦b⟧` rule, "arithmetic in subterms ... at the
  spec-context numeric domain"); CASTS denote the value coercion (the doc's
  `cast → nat/int` rule: "an integer cast denotes the value as an unbounded
  `nat`/`int` (no wrap — spec arithmetic is mathematical)").

  THE PARTIALITY CONVENTION (#176). `Div`/`Rem`/`Shl`/`Shr` are PARTIAL in the
  source (`ast.rs`: `BinOp::Rem` "requires a nonzero divisor"; the zero divisor /
  zero shift is rejected as a SOURCE PRECONDITION / L0 obligation, discharged
  OUTSIDE the clause). We model them with Lean's TOTAL `Int` operations
  (`Int./` is Euclidean-ish / T-division; `Int.%` its companion). This is sound
  for `S_C` because: (i) the divisor-≠0 precondition is a SOURCE-SIDE obligation,
  not part of the binop's contract MEANING; (ii) — load-bearing for T1 — `denote`
  and `refDenote` route the op through the SAME shared `arithDenote` function, so
  whatever total convention is chosen, BOTH sides agree (the soundness theorem is
  about the encoder's operator MAP being faithful, not about the partial-point
  value). The convention is stated here and held consistent across both denotations.

  THE CAST/`nat` CONVENTION (#177). `as nat` is the value injected into the
  naturals: a non-negative `int` maps to itself, a negative `int` maps to `0`
  (Lean `Int.toNat` clamps; in `S_C` an `as nat` always carries a `≥ 0` source
  frame, so the clamp point is never the intended value — the same shape as the
  zero-divisor convention). `as int`/`as u64`/`as u32`/`as usize` are
  value-preserving on the spec `int` domain (the bounded cast carries its
  no-overflow frame as a source obligation, exactly like div-by-zero), so at the
  CONTRACT level they denote the value unchanged. Again the convention is SHARED
  by `denote` and `refDenote` (`castDenote`), so T1 is insensitive to the clamp.

  An `Env` maps each free name (a param / `result` / `old(x)`) to an `Int` — exactly
  the per-clause obligation binding `ref_encode.rs` describes. The denotation is a
  TOTAL structural recursion (a clause is a pure predicate; no state, no loops).
-/
import Thermite.Ast

namespace Thermite

/-- An `Option`/`Result` VALUE (#180, the C7 payload-in-contract fragment): the value an
    `optResVar` scrutinee denotes. `none`/`some v`/`ok v`/`err e` — the payload `v`/`e` an
    `Int` (the C7 corpus payloads are integer-valued; faithful to the corpus, NOT general
    ADTs). The `match`-arm selection reads the variant; the `is`-test reads the variant; the
    payload is bound into the arm's predicate via `Env.bindInt`. -/
inductive OptResVal where
  | none_
  | some_ (v : Int)
  | ok    (v : Int)
  | err   (e : Int)
  deriving DecidableEq, Repr

/-- The denotation environment: a valuation of free names. Integer names (params,
    `result`, `old(x)`) map to `Int`; SEQUENCE names (a `&[u32]` slice / a `String`'s
    bytes — #178) map to `List Int`; OPTION/RESULT names (a `match`/`is` scrutinee — #180)
    map to an `OptResVal`. `S_C`'s `Env` extended to the sequence + option/result domains.
    The maps are independent (a name is bound at exactly one sort by the obligation's
    parameter binding). -/
structure Env where
  /-- The integer-valued free names (params / `result` / `old(x)`). -/
  ints : String → Int
  /-- The sequence-valued free names (slice params, `String` byte sequences). The
      `@`-view is the IDENTITY on this value — a slice and its `@`-view are the same
      `List Int` (#178). -/
  seqs : String → List Int
  /-- The `Option`/`Result`-valued free names (a `match`/`is` scrutinee — #180). -/
  optres : String → OptResVal

/-- Bind an integer name to a value in an environment (#179): used to interpret a
    predicate closure `|x| <body>` at a concrete element — the bound var `x` is set to
    the i-th element while the sequence names are unchanged. The SHARED env-update both
    `denote` (source) and `RefEncode.refDenote` (encoder) route a combinator predicate
    through, so the predicate's denotation is identical on both sides modulo the body's
    own integer/sequence subterms (settled by `refVal_eq`). -/
def Env.bindInt (env : Env) (name : String) (v : Int) : Env :=
  { env with ints := fun s => if s = name then v else env.ints s }

/-- The VARIANT an `OptResVal` is (#180): the discriminant a `match` arm selects on / an
    `is`-test reads. Shared by `denote`/`refDenote` (the Verus `match`/`is` discriminant is the
    SAME meaning on both sides — the encoder reuses Verus's variant semantics verbatim). -/
def OptResVal.variant : OptResVal → Variant
  | OptResVal.none_  => Variant.none_
  | OptResVal.some_ _ => Variant.some_
  | OptResVal.ok _    => Variant.ok
  | OptResVal.err _   => Variant.err

/-- The PAYLOAD an `OptResVal` carries (#180): the `Int` bound into a `Some(v)`/`Ok(v)`/`Err(e)`
    arm's predicate. `None` carries no payload — modelled as `0` (a `None` arm has no binder, so
    the payload is never observed; the `0` keeps the read total). Shared by `denote`/`refDenote`. -/
def OptResVal.payload : OptResVal → Int
  | OptResVal.none_   => 0
  | OptResVal.some_ v => v
  | OptResVal.ok v    => v
  | OptResVal.err e   => e

/-- The `is`-test result (#180): does an `OptResVal` have the tested `Variant`? Shared by
    `denote`/`refDenote` (the Verus `(e is V)` discriminant test is the SAME meaning on both
    sides — `ref_encode.rs`'s `Expr::Is` arm reuses Verus's `is`). -/
def OptResVal.isVariant (v : OptResVal) (target : Variant) : Bool :=
  decide (v.variant = target)

/-- The shared TOTAL sequence-index access (#178): the i-th element, or `0` when the
    index is out of range (negative or ≥ length). The in-range obligation `0 ≤ i < |s|`
    is a SOURCE precondition (L0); the total `0`-default is the analogue of the
    div-by-zero convention, held CONSISTENT between `denote` and `refDenote` (both route
    through `seqIdx`), so T1 is insensitive to the out-of-range point. -/
def seqIdx (s : List Int) (i : Int) : Int :=
  s.getD i.toNat 0

/-- The shared TOTAL `subrange(lo, hi)` (#178): the contiguous sub-sequence of indices
    `[lo, hi)`. Modelled as `drop lo` then `take (hi - lo)` over `List`, total under the
    `0 ≤ lo ≤ hi ≤ |s|` source frame (out-of-range clamps via `List.take`/`List.drop`,
    held consistent across both denotations). `&xs[..i]` = `subrange 0 i` = `take i`. -/
def seqSub (s : List Int) (lo hi : Int) : List Int :=
  (s.drop lo.toNat).take (hi.toNat - lo.toNat)

/-- The SHARED integer meaning of an arithmetic operator over two `Int` operand
    VALUES (#176). Defined ONCE here so BOTH `denote` (source) and
    `RefEncode.refDenote` (encoder) route through it — the soundness content for
    arithmetic is the encoder's OPERATOR-MAP faithfulness (`arith → "+"` etc.),
    mirroring `tokRel (encOp op)` for comparisons, NOT a re-derivation of each
    op's integer value.

    - `add`/`sub`/`mul`/`div`/`rem` use core `Int` arithmetic. `div`/`rem` are the
      TOTAL `Int./`/`Int.%` under the divisor-≠0 convention (the partial point is a
      source precondition; see the module header).
    - `shl`/`shr`/`bitAnd`/`bitOr`/`bitXor` are defined over the `Nat` value of each
      operand (`Int.toNat`) using core `Nat` bit ops, re-injected as `Int`. The
      frozen `S_C` bitwise/shift operands are bounded UNSIGNED values (`u64`/`u32`/
      `usize`), so `Int.toNat` is value-preserving on them (the `≥ 0` frame, the
      same source obligation as the cast `nat` clamp). Core-only — no Mathlib. -/
def arithDenote : ArithOp → Int → Int → Int
  | ArithOp.add,    x, y => x + y
  | ArithOp.sub,    x, y => x - y
  | ArithOp.mul,    x, y => x * y
  | ArithOp.div,    x, y => x / y
  | ArithOp.rem,    x, y => x % y
  | ArithOp.shl,    x, y => (Nat.shiftLeft x.toNat y.toNat : Int)
  | ArithOp.shr,    x, y => (Nat.shiftRight x.toNat y.toNat : Int)
  | ArithOp.bitAnd, x, y => (Nat.land x.toNat y.toNat : Int)
  | ArithOp.bitOr,  x, y => (Nat.lor x.toNat y.toNat : Int)
  | ArithOp.bitXor, x, y => (Nat.xor x.toNat y.toNat : Int)

/-- The SHARED value coercion of a cast target over an `Int` operand value (#177).
    Defined ONCE so BOTH `denote` and `RefEncode.refDenote` route through it — the
    soundness content for casts is the encoder's CAST-TARGET map faithfulness +
    the PARENTHESIZATION of the inner (the #122/#146 class, modelled in
    `RefEncode.lean`), NOT a re-derivation of the coercion.

    - `nat`: inject into the naturals (`Int.toNat`, clamping a negative to `0`; in
      `S_C` an `as nat` carries a `≥ 0` source frame — see the module header).
    - `int`/`u64`/`u32`/`usize`: value-preserving on the spec `int` domain (the
      bounded no-overflow frame is a source obligation, carried like div-by-zero). -/
def castDenote : CastTy → Int → Int
  | CastTy.nat,   v => (v.toNat : Int)
  | CastTy.int,   v => v
  | CastTy.u64,   v => v
  | CastTy.u32,   v => v
  | CastTy.usize, v => v

/- `⟦·⟧_{S_C}` on the SEQUENCE-valued terms (#178): a `seqVar`/`strVar` denotes its
   environment sequence (the `@`-view is the identity on this value); a `subrange`
   denotes the contiguous sub-sequence `seqSub` of its base over the range bounds
   (`&xs[..i]` = `seqSub 0 i`, `&xs[a..b]` = `seqSub a b`, `&xs[a..]` = `seqSub a |xs|`).
   These are the SOURCE meanings (before the rewrite); the encoder's `@`/`subrange`
   rewrite preserves them. A non-sequence node denotes the empty sequence (never
   observed by the soundness theorem, which only evaluates `seqVal` on sequence-sorted
   bases — keeps it TOTAL without a `sorry`). Mutual with `intVal` (`subrange`'s bounds
   are integer terms). -/
mutual
/-- `⟦·⟧_{S_C}` on the SEQUENCE-valued terms (#178). See the block comment above. -/
def seqVal : Expr → Env → List Int
  | Expr.seqVar x, env => env.seqs x
  | Expr.strVar x, env => env.seqs x
  | Expr.subrange base r, env =>
      let s := seqVal base env
      match r with
      | RangeArg.rangeTo hi    => seqSub s 0 (intVal hi env)
      | RangeArg.range lo hi   => seqSub s (intVal lo env) (intVal hi env)
      | RangeArg.rangeFrom lo  => seqSub s (intVal lo env) (s.length : Int)
  | _, _ => []

/-- `⟦·⟧_{S_C}` on the INTEGER-valued terms (the operands of a comparison): a
    literal denotes itself, a variable denotes its environment value, an arithmetic
    term denotes the shared `arithDenote` of its operands, a cast denotes the shared
    `castDenote` of its inner. EXTENDED (#178) with the sequence-reading integer terms:
    `idx` = the i-th element (`seqIdx`), `seqLen` = the length, `byteAt` = the i-th byte
    (`seqIdx` over the `String` byte sequence). These are the integer-term rules of
    `S_C`. Mutual with `seqVal`. -/
def intVal : Expr → Env → Int
  | Expr.intLit n,      _   => n
  | Expr.var x,         env => env.ints x
  | Expr.arith op a b,  env => arithDenote op (intVal a env) (intVal b env)
  | Expr.cast inner ty, env => castDenote ty (intVal inner env)
  | Expr.idx base i,    env => seqIdx (seqVal base env) (intVal i env)
  | Expr.seqLen base,   env => (seqVal base env).length
  | Expr.byteAt base i, env => seqIdx (seqVal base env) (intVal i env)
  -- A boolean-sorted node never appears as a comparison operand in a well-formed
  -- clause; it has no integer meaning, so it denotes the canonical `0`. (The
  -- soundness theorem only ever evaluates `intVal` on integer-sorted subterms —
  -- the operands `cmp`/`arith`/`cast`/`idx`/`seqLen`/`byteAt` build — so this default
  -- is never observed there; it keeps `intVal` TOTAL without a `sorry`/partial
  -- annotation.)
  | _, _ => 0
end

/-- The `OptResVal` a `match`/`is` SCRUTINEE denotes (#180): an `optResVar` reads its env
    `optres` value; any other node is not an `Option`/`Result` scrutinee in a well-formed C7
    clause, so it denotes the canonical `none` (never observed — the soundness theorem only
    evaluates this on an `optResVar` scrutinee; keeps it total without a `sorry`). Shared by
    `denote`/`refDenote` (the scrutinee value is a free name, the SAME on both sides). -/
def scrutVal : Expr → Env → OptResVal
  | Expr.optResVar x, env => env.optres x
  | _, _ => OptResVal.none_

mutual
/-- `⟦·⟧_{S_C}` — the SOURCE meaning of a contract predicate as a Lean `Prop`.
    Each comparison/logical/negation denotes the STANDARD mathematical relation
    (the `S_C` inference rules), defined HERE following the SOURCE meaning — to be
    proved equal to `RefEncode.refDenote` (which follows the ENCODER's structure),
    so the soundness theorem has content. Mutual with `denoteArms` (#180: the `match_`
    arm selection denotes the selected arm's body, an `Expr` subterm). -/
def denote : Expr → Env → Prop
  | Expr.boolLit b, _   => (b = true)
  | Expr.cmp op a b, env =>
      let x := intVal a env
      let y := intVal b env
      match op with
      | CmpOp.eq => x = y
      | CmpOp.ne => x ≠ y
      | CmpOp.lt => x < y
      | CmpOp.le => x ≤ y
      | CmpOp.gt => x > y
      | CmpOp.ge => x ≥ y
  | Expr.logic op a b, env =>
      match op with
      | LogOp.and => denote a env ∧ denote b env
      | LogOp.or  => denote a env ∨ denote b env
  | Expr.neg e, env => ¬ denote e env
  -- The MATCH-IN-ENS form (#180): the arm SELECTED by the scrutinee's variant denotes its body
  -- with the payload BOUND (the C7 `match result { Some(v) => P(v), None => Q }`). `scrutVal`
  -- reads the scrutinee's `OptResVal`; `denoteArms` walks the arms, selecting the one whose
  -- `Variant` matches and binding its payload into the body (`Env.bindInt`). Faithful to
  -- `encode_match`: scrutinee + bodies via the SAME recursion, the pattern's variant+binder via
  -- `encode_pattern`. The selection-by-variant is the Verus `match` meaning (shared, not
  -- re-derived); the soundness content is the scrutinee/body encoding.
  | Expr.match_ scrut arms, env =>
      denoteArms (scrutVal scrut env) arms env
  -- The `is`-test (#180): the variant DISCRIMINANT test `scrut is variant`. True iff the
  -- scrutinee's value is that variant (`OptResVal.isVariant`, the shared Verus `is` meaning).
  | Expr.is_ scrut variant, env =>
      ((scrutVal scrut env).isVariant variant = true)
  -- The 6 BOUNDED-QUANTIFIER combinators (#179): each denotes its FROZEN `verus_l3`
  -- quantifier form (`combinators.rs`), with the slice = `seqVal` of the slice arg, the
  -- index = `intVal` of the scalar index arg, and `p(s[i])` = the predicate body denoted
  -- with the bound element var ↦ the i-th element (`seqIdx s i`). The optional `seq2`/
  -- `idx`/`pred` default to the empty sequence / `0` / a vacuous predicate when a
  -- combinator does not carry that arg (never observed — each combinator populates only
  -- its own arg-kinds; keeps `denote` TOTAL with no `sorry`).
  | Expr.comb c seq seq2 idx pred, env =>
      let s := seqVal seq env
      let s2 := match seq2 with | some e => seqVal e env | none => []
      let n := match idx with | some e => intVal e env | none => 0
      -- Apply the predicate closure body at the i-th element of `s` (the bound var ↦
      -- `seqIdx s i`); a missing predicate (sorted/disjoint) is `True` (unused).
      let p : Int → Prop := fun i =>
        match pred with
        | some (Pred.mk bound body) => denote body (env.bindInt bound (seqIdx s i))
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
  -- An integer-sorted leaf/term (`intLit`/`var`/`arith`/`cast`) is not a predicate
  -- on its own; in a well-formed clause it only appears as a comparison operand
  -- (handled by `intVal` above). As a top-level predicate it denotes `True`
  -- vacuously — never reached by the soundness theorem (whose top-level `Expr`s are
  -- always cmp/logic/neg/bool). Keeps `denote` TOTAL with no `sorry`.
  | _, _ => True

/-- The `match`-arm SELECTION + payload BINDING (#180), SHARED structure for `denote` and
    `RefEncode.refDenote` (the encoder reuses the Verus `match` semantics verbatim — the
    arm selection by variant is the Verus `match` meaning, NOT re-implemented; the soundness
    content is the per-body encoding, threaded by the `match_` case of `ref_sound`).

    Walks the arms in source order; for the FIRST arm whose `Variant` matches the scrutinee
    value's variant, denotes that arm's body with the payload bound (the binder ↦ the scrutinee
    payload via `Env.bindInt`; a `None`/binder-less arm leaves the env unchanged). A
    non-matching arm is skipped. A well-formed C7 `match` is EXHAUSTIVE (the corpus 2-arm
    `Some/None`/`Ok/Err`), so the matching arm always exists; an empty/non-exhaustive remainder
    denotes `True` (out of the fragment — never reached by the soundness theorem's exhaustive
    `match`es). -/
def denoteArms : OptResVal → List MatchArm → Env → Prop
  | _, [], _ => True
  | scrut, MatchArm.mk variant binder body :: rest, env =>
      if scrut.variant = variant then
        match binder with
        | some x => denote body (env.bindInt x scrut.payload)
        | none   => denote body env
      else
        denoteArms scrut rest env
end

end Thermite
