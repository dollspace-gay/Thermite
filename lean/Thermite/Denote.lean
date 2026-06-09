/-
  Thermite/Denote.lean — the SOURCE denotation `⟦·⟧_{S_C}` for the comparison +
  logical contract fragment (increment (a), #170) EXTENDED with the ARITHMETIC
  operators (#176), the CASTS (#177), and the SPEC-CONTEXT REWRITES (#178 — slice→
  `@`/subrange, indexing, the `String` byte-view length/byte-at).

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

/-- The denotation environment: a valuation of free names. Integer names (params,
    `result`, `old(x)`) map to `Int`; SEQUENCE names (a `&[u32]` slice / a `String`'s
    bytes — #178) map to `List Int`. `S_C`'s `Env` extended to the sequence domain.
    The two maps are independent (a name is bound at exactly one sort by the
    obligation's parameter binding). -/
structure Env where
  /-- The integer-valued free names (params / `result` / `old(x)`). -/
  ints : String → Int
  /-- The sequence-valued free names (slice params, `String` byte sequences). The
      `@`-view is the IDENTITY on this value — a slice and its `@`-view are the same
      `List Int` (#178). -/
  seqs : String → List Int

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

/-- `⟦·⟧_{S_C}` — the SOURCE meaning of a contract predicate as a Lean `Prop`.
    Each comparison/logical/negation denotes the STANDARD mathematical relation
    (the `S_C` inference rules), defined HERE following the SOURCE meaning — to be
    proved equal to `RefEncode.refDenote` (which follows the ENCODER's structure),
    so the soundness theorem has content. -/
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
  -- An integer-sorted leaf/term (`intLit`/`var`/`arith`/`cast`) is not a predicate
  -- on its own; in a well-formed clause it only appears as a comparison operand
  -- (handled by `intVal` above). As a top-level predicate it denotes `True`
  -- vacuously — never reached by the soundness theorem (whose top-level `Expr`s are
  -- always cmp/logic/neg/bool). Keeps `denote` TOTAL with no `sorry`.
  | _, _ => True

end Thermite
