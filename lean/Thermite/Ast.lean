/-
  Thermite/Ast.lean — the contract-sublanguage AST as a Lean `inductive`, for the
  COMPARISON + LOGICAL fragment (increment (a), #170) EXTENDED with the ARITHMETIC
  operators (increment (b), #176), the CASTS (increment (c), #177), and the
  SPEC-CONTEXT REWRITES (increment (f), #178 — slice→`@`/subrange, indexing, and the
  method→`spec_*` byte-view DISPATCH, the #127 class); epic #169.

  Governing design: `.design/verified/thermite-semantics.md` REQ-1/REQ-6 (the
  `S_C` denotation domain; the Lean module layout) + AC-1 (S is stated over the
  exact frozen subset the encoders admit) + Architecture §"S_C" (the binop map +
  the `cast → nat/int` coercion rule, "the #122 class is a property of the
  production STRING ... the (T1) obligation is precisely 'does the production
  string PARSE to an AST whose denotation matches'").

  This mirrors the relevant `thermite-syntax/src/ast.rs` `Expr` / `BinOp` /
  `Type` / `PrimType` variants for THIS fragment:
    - integer literals          (`Expr::IntLit { value, .. }`)
    - bool literals             (`Expr::BoolLit(b)`)
    - variables / `result` / `old(x)`  (`Expr::Path` + the `old(_)` call form —
      all denote as free names of type `Int`, per `S_C`'s "literals/refs denote
      themselves" rule; the obligation binds each as a distinct param)
    - comparison binops         (`BinOp::{Eq,Ne,Lt,Le,Gt,Ge}`)
    - logical binops + negation (`BinOp::{And,Or}`, `UnaryOp::Not`)
    - ARITHMETIC binops (#176)   (`BinOp::{Add,Sub,Mul,Div,Rem,Shl,Shr,BitAnd,
                                  BitOr,BitXor}` — integer/`int` arithmetic over
                                  the values; NO wraparound — overflow is an
                                  EXEC-side obligation, increment #171, not here)
    - CASTS (#177)               (`Expr::Cast { expr, ty }` to
                                  `u64`/`u32`/`usize`/`nat`/`int` — value coercions)

  PARTIALITY (#176): `Div`/`Rem`/`Shl`/`Shr` are PARTIAL in the source — a zero
  divisor / a zero shift is rejected as a precondition (an L0 obligation discharged
  OUTSIDE the contract clause, `ast.rs` `BinOp::Rem` "PARTIAL: requires a nonzero
  divisor"). The denotation models them with Lean's TOTAL `Int` operations under
  the divisor-≠0 convention; because `denote` and `refDenote` use the SAME total
  operation, T1 holds regardless of the guard (the guard is the source-side
  precondition, not part of the binop's meaning when the precondition holds —
  Euclidean-consistent, see `Denote.lean`).

  THE SPEC-CONTEXT REWRITES (#178). In contract position a slice/`String` param does
  NOT denote a scalar — it denotes a SEQUENCE, and the encoder REWRITES the use sites:
    - a slice param `xs`            → `xs@` (the `Seq` view; the identity on the value)
    - `xs[i]`                       → `xs@[i]` (the i-th element)
    - `&xs[..i]` / `&xs[a..b]`      → `xs@.subrange(0, i as int)` / `xs@.subrange(a, b)`
    - a `String` receiver `s.byte_at(i)` → `s.spec_byte_at(i)` (the byte-view DISPATCH)
    - `s.len()`                     → `s.spec_len()`             (the byte-view DISPATCH)
  The THEOREM (`ref_sound`): these rewrites PRESERVE MEANING (`@`/subrange/`spec_*` is
  the identity-on-meaning coercion from the exec slice/`String` to its spec sequence).
  The #127 NEGATIVE lemma shows a WRONG dispatch (wrong index / wrong receiver-method)
  FAILS soundness. To model this `Ast.lean` gains:
    - `SeqVar`             — a free SEQUENCE name (a `&[u32]` slice / a `String`'s bytes)
    - `Expr.idx`           — `xs[i]` over a sequence var at an integer index
    - `RangeArg` + `Expr.subrange` — the `&xs[..i]`/`&xs[a..b]`/`&xs[a..]` range borrow
    - `Expr.seqLen`        — `xs.len()` / `s.len()` (the sequence length, → `spec_len`)
    - `Expr.byteAt`        — `s.byte_at(i)` (the i-th byte, → `spec_byte_at`)
  These are integer-valued (an element / a byte / a length) except `subrange`, which is
  sequence-valued and feeds another `idx`/`seqLen`/`byteAt` (so the prefix's meaning is
  observed through a later element/length read — exactly how a contract clause uses it).

  THE 6 BOUNDED-QUANTIFIER COMBINATORS (#179, increment 1d-i). In contract position a
  combinator CALL `Call(C, args)` denotes its FROZEN `verus_l3` quantifier form
  (`thermite-spec/src/combinators.rs`), with each argument threaded PER ITS REGISTRY
  ARG-KIND (`CombinatorSig.arg_kinds`/`ArgKind`):
    - `forall_in(s, p)`    = `∀ i, 0 ≤ i < s.len() → p(s[i])`
    - `exists_in(s, p)`    = `∃ i, 0 ≤ i < s.len() ∧ p(s[i])`
    - `sorted(s)`          = `∀ i j, 0 ≤ i ≤ j < s.len() → s[i] ≤ s[j]`
    - `forall_below(s,n,p)`= `∀ i, 0 ≤ i < n ∧ i < s.len() → p(s[i])`
    - `forall_from(s,n,p)` = `∀ i, n ≤ i < s.len() → p(s[i])`
    - `disjoint(a, b)`     = `∀ i j, (0 ≤ i < a.len() ∧ 0 ≤ j < b.len()) → a[i] ≠ b[j]`
  To embed these `Ast.lean` gains:
    - `CombName`           — the 6 frozen names (the closed combinator set, sans the 2
      RECURSIVE combinators `count_where`/`permutation_of`, which are #182 / 1d-ii and
      DELIBERATELY ABSENT — no embed-then-`sorry`).
    - `Pred`               — a FLAT predicate closure `|x| <body>` (`ArgKind::Pred`): a
      bound element-var name + a body `Expr` over the comparison/logical/arithmetic
      fragment (§4.2 "no anonymous nested quantifiers" — so the body REUSES the existing
      `Expr`). `p(s[i])` denotes as the body with the bound var ↦ the i-th element.
    - `Expr.comb`          — a combinator call carrying: the name `c`; a primary slice
      `seq` (`ArgKind::Slice` → the `@`-view, #178/1f); an OPTIONAL second slice `seq2`
      (only `disjoint`); an OPTIONAL scalar index `idx` (only `forall_below`/`forall_from`
      — `ArgKind::Index`, a SCALAR `int`, NOT a slice `@`-view: THE #145 BUG CLASS); and
      an OPTIONAL predicate `pred` (`ArgKind::Pred`). Each combinator populates exactly
      the fields its `arg_kinds` declares (the others `none`).

  DEFERRED — NOT embedded here, and DELIBERATELY NOT (no `sorry`-behind-a-variant;
  embedding-then-`sorry` is forbidden). These are the remaining sub-increments:
    - the 2 RECURSIVE / aggregate combinators (`count_where`/`permutation_of` — their
      well-founded-recursive / multiset `verus_l3` forms; Mathlib) (#182 / 1d-ii)
    - named spec-fn calls (the well-founded recursive `S_C` fixpoint) (#181)
    - `Expr::Match` / `Expr::Is` in contract position (#180)
  Each is a real future inductive case, listed (not stubbed) so the deferral is honest.
-/

namespace Thermite

/-- The comparison operators of the frozen contract sublanguage — mirrors the
    `BinOp::{Eq,Ne,Lt,Le,Gt,Ge}` arms of `thermite-syntax/src/ast.rs`. These relate
    two integer operands and denote a `Prop`. The `==`-vs-`<=` faithfulness (the
    showcase case) is the distinction between `CmpOp.Eq` and `CmpOp.Le`. -/
inductive CmpOp where
  | eq
  | ne
  | lt
  | le
  | gt
  | ge
  deriving DecidableEq, Repr

/-- The binary logical connectives — mirrors `BinOp::{And,Or}`. -/
inductive LogOp where
  | and
  | or
  deriving DecidableEq, Repr

/-- The ARITHMETIC binary operators of the frozen contract sublanguage (#176) —
    mirrors the `BinOp::{Add,Sub,Mul,Div,Rem,Shl,Shr,BitAnd,BitOr,BitXor}` arms of
    `thermite-syntax/src/ast.rs`. In CONTRACT position these are integer (`int`)
    arithmetic over the operand VALUES — NO wraparound (the unbounded-`int` spec
    domain; overflow is an exec obligation, not modelled here). `Div`/`Rem`/`Shl`/
    `Shr` are PARTIAL (divisor/shift ≠ 0 precondition); see `Denote.lean` for the
    total-operation-under-the-convention modelling. These take two `Int` operands
    and produce an `Int` (unlike `CmpOp`, which produces a `Prop`). -/
inductive ArithOp where
  | add
  | sub
  | mul
  | div
  | rem
  | shl
  | shr
  | bitAnd
  | bitOr
  | bitXor
  deriving DecidableEq, Repr

/-- The 6 BOUNDED-QUANTIFIER combinator names (#179) — the frozen `verus_l3` forms of
    `thermite-spec/src/combinators.rs` whose denotation is a BOUNDED `∀`/`∃` over a
    slice. The 2 RECURSIVE / aggregate combinators (`count_where`/`permutation_of`) are
    #182 (1d-ii) and DELIBERATELY ABSENT — their `verus_l3` is a well-founded recursion /
    a multiset equality, not a bounded quantifier, so embedding them here would force a
    `sorry` (forbidden). This enum is therefore the CLOSED bounded-quantifier subset. -/
inductive CombName where
  | forallIn      -- `forall_in(s, p)`    = `∀ i, 0 ≤ i < s.len() → p(s[i])`
  | existsIn      -- `exists_in(s, p)`    = `∃ i, 0 ≤ i < s.len() ∧ p(s[i])`
  | sorted        -- `sorted(s)`          = `∀ i j, 0 ≤ i ≤ j < s.len() → s[i] ≤ s[j]`
  | forallBelow   -- `forall_below(s,n,p)`= `∀ i, 0 ≤ i < n ∧ i < s.len() → p(s[i])`
  | forallFrom    -- `forall_from(s,n,p)` = `∀ i, n ≤ i < s.len() → p(s[i])`
  | disjoint      -- `disjoint(a, b)`     = `∀ i j, (0≤i<a.len() ∧ 0≤j<b.len()) → a[i]≠b[j]`
  deriving DecidableEq, Repr

/-- The cast targets of the frozen contract sublanguage (#177) — mirrors the
    cast-admitting `Type`/`PrimType` arms `ref_encode.rs::cast_target` accepts:
    the bounded prims `u64`/`u32`/`usize` (`PrimType::{U64,U32,Usize}`) and the
    spec arithmetic ladder `nat`/`int` (`Type::Named "nat"`/`"int"`). A cast to
    `bool` or any other type is `Unsupported` in the encoder, so it is OUT of `S_C`
    and absent here. -/
inductive CastTy where
  | u64
  | u32
  | usize
  | nat
  | int
  deriving DecidableEq, Repr

/- The contract-sublanguage expression. Four syntactic sorts are distinguished by
   the inductive shape: `intLit`/`var`/`arith`/`cast`/`idx`/`seqLen`/`byteAt` build
   INTEGER terms (operands of a comparison / an element / a byte / a length);
   `cmp`/`logic`/`neg`/`boolLit` build BOOLEAN/`Prop` terms; `seqVar`/`subrange`
   build SEQUENCE terms (the slice→`@`-view + the `subrange` borrow, observed through
   a later `idx`/`seqLen`/`byteAt`). A `var` carries a `String` name (a param,
   `result`, or the `old(x)` pre-state binding — all free integer names, per `S_C`);
   a `seqVar`/`strVar` carries a free SEQUENCE name (#178).

   `RangeArg` (the slice-borrow range) is a MUTUAL inductive with `Expr` because its
   bounds are integer-valued `Expr`s (cast `as int` by the encoder). -/
mutual
/-- The contract-sublanguage expression (the integer/bool/sequence terms). -/
inductive Expr where
  /-- An integer literal `IntLit { value }`. Lean models the value as `Int`
      (the spec numeric domain `S_C` denotes into is unbounded `int`). -/
  | intLit (value : Int)
  /-- A boolean literal `BoolLit(b)`. -/
  | boolLit (value : Bool)
  /-- A free integer variable: a param, `result`, or `old(x)` (all bound as distinct
      obligation params of type `Int`). Mirrors `Expr::Path([name])` and the
      `old(_)` form, which `ref_encode.rs` likewise treats as free names. -/
  | var (name : String)
  /-- A comparison `a <op> b` over two integer subterms (`Expr::Binary` with a
      comparison `BinOp`). -/
  | cmp (op : CmpOp) (lhs rhs : Expr)
  /-- A logical connective `a <op> b` over two boolean subterms (`Expr::Binary`
      with `BinOp::{And,Or}`). -/
  | logic (op : LogOp) (lhs rhs : Expr)
  /-- Logical negation `!a` (`Expr::Unary { op := UnaryOp::Not, .. }`). -/
  | neg (e : Expr)
  /-- An ARITHMETIC binary `a <op> b` over two integer subterms (#176; `Expr::Binary`
      with an arithmetic `BinOp`). Builds an INTEGER term (a comparison operand). -/
  | arith (op : ArithOp) (lhs rhs : Expr)
  /-- A CAST `inner as ty` (#177; `Expr::Cast { expr := inner, ty }`). Builds an
      INTEGER term (the coerced value). The PARENTHESIZATION the encoder applies to
      `inner` (the #122/#146 discipline) is modelled in `RefEncode.lean`: because
      the cast wraps a WHOLE subexpression as its operand, dropping the paren would
      re-parse a compound `inner` and change the denotation (the negative lemma). -/
  | cast (inner : Expr) (ty : CastTy)
  /-- A free SEQUENCE variable: a `&[u32]` slice param (#178). In contract position
      the encoder REWRITES it to its `@`-view (`xs` → `xs@`), which is the identity on
      the value (the same sequence of elements). Mirrors `encode_slice_arg`'s
      `Expr::Path` arm. SEQUENCE-sorted — observed only through `idx`/`subrange`/
      `seqLen` (a bare sequence is not a `Prop`). -/
  | seqVar (name : String)
  /-- A free `String` variable whose BYTES are the sequence (#178; the #127 byte-view
      class). The encoder dispatches its `.len()`/`.byte_at(i)` to the wrapper SPEC
      fns `spec_len`/`spec_byte_at` (`encode_string_byteview`); the BYTES it denotes
      over are the same sequence. SEQUENCE-sorted (a `List` of byte values, modelled
      as `Int`). -/
  | strVar (name : String)
  /-- `xs[i]` — a single-element index (#178; `Expr::Index { index := Single(i) }`).
      The encoder rewrites to `xs@[i as int]` (`encode_index`'s `Single` arm over the
      receiver's `@`-view); the meaning is the i-th element. INTEGER-sorted. `base` is
      a SEQUENCE term (a `seqVar` or a `subrange`), `idx` an integer term. -/
  | idx (base : Expr) (index : Expr)
  /-- `&xs[..i]` / `&xs[a..b]` / `&xs[a..]` — a slice-range borrow (#178;
      `Expr::Ref` of an `Expr::Index` of a range, routed through `encode_ref`→
      `encode_index`). The encoder rewrites to `xs@.subrange(lo, hi)`; the meaning is
      the corresponding contiguous SUB-sequence. SEQUENCE-sorted (`base` a sequence
      term, the range an integer-bounded `RangeArg`). -/
  | subrange (base : Expr) (range : RangeArg)
  /-- `xs.len()` / `s.len()` — the sequence length (#178; `Expr::MethodCall` `len`).
      For a slice it rewrites to `xs@.len()` (`encode_method_call`'s `len` arm); for a
      `String` to `s.spec_len()` (`encode_string_byteview`'s `len` arm). The meaning is
      the length of the sequence. INTEGER-sorted. -/
  | seqLen (base : Expr)
  /-- `s.byte_at(i)` — the i-th byte of a `String`'s byte sequence (#178; the #127
      byte-view DISPATCH; `Expr::MethodCall` `byte_at`). The encoder rewrites to
      `s.spec_byte_at(i)` (`encode_string_byteview`'s `byte_at` arm). The meaning is
      the i-th byte. INTEGER-sorted. `base` is a `String`-sequence term, `index` an
      integer term. THE #127 CLASS lives here: a wrong index / a wrong receiver-method
      is a DIFFERENT meaning (the negative lemma). -/
  | byteAt (base : Expr) (index : Expr)
  /-- A BOUNDED-QUANTIFIER combinator call (#179) — `Call(C, args)` for `C` in the 6
      frozen bounded combinators. Denotes its FROZEN `verus_l3` quantifier form
      (`combinators.rs`) with each argument threaded PER ITS REGISTRY ARG-KIND. The
      fields carry the per-kind args; each combinator populates exactly the fields its
      `CombinatorSig.arg_kinds` declares (`none` otherwise):
        - `seq`  : the primary slice (`ArgKind::Slice` → the `@`-view; SEQUENCE-sorted —
                   a `seqVar`/`strVar`/`subrange`).
        - `seq2` : the SECOND slice (only `disjoint`'s `b`; `ArgKind::Slice`).
        - `idx`  : the SCALAR index bound (only `forall_below`/`forall_from`'s `n`;
                   `ArgKind::Index` → a scalar `int`, NOT a slice `@`-view — THE #145 BUG
                   CLASS lives in this arg-kind's dispatch).
        - `pred` : the predicate closure (`ArgKind::Pred`; absent for `sorted`/`disjoint`).
      BOOLEAN/`Prop`-sorted (a combinator result is `bool`). -/
  | comb (c : CombName) (seq : Expr) (seq2 : Option Expr) (idx : Option Expr)
         (pred : Option Pred)
  /-- A FLAT predicate closure `|x| <body>` (#179; `ArgKind::Pred`; `Expr::Closure` with
      one param over the frozen `Seq<u32>` element type `u32`). `bound` is the element
      var name the closure binds (`encode_pred_arg`'s `params[0]`); `body` is the
      predicate over that element, REUSING the comparison/logical/arithmetic `Expr`
      fragment (§4.2 "no anonymous nested quantifiers" — the body is FLAT, so the
      structural recursion terminates). `p(s[i])` denotes as `body` with `bound ↦` the
      i-th element. MUTUAL with `Expr` (the body IS an `Expr`). -/
inductive Pred where
  | mk (bound : String) (body : Expr)
  /-- A range argument of a spec-context slice borrow (#178) — mirrors the
      `thermite-syntax::ast::IndexArg` arms `encode_index`/`encode_ref` accept:
      `RangeTo(i)` (`&xs[..i]`), `Range(a, b)` (`&xs[a..b]`), `RangeFrom(a)`
      (`&xs[a..]`). A `Single(i)` is NOT here — a single-index borrow is the element
      form `Expr.idx` (`encode_index`'s `IndexArg::Single` arm), not a subrange. Each
      bound is an integer-valued `Expr` (cast `as int` by the encoder,
      `encode_index_value`). -/
inductive RangeArg where
  /-- `..i` (`&xs[..i]`) → `xs@.subrange(0, i as int)` (`encode_index`'s `RangeTo`). -/
  | rangeTo (hi : Expr)
  /-- `a..b` (`&xs[a..b]`) → `xs@.subrange(a, b)` (`encode_index`'s `Range`). -/
  | range (lo hi : Expr)
  /-- `a..` (`&xs[a..]`) → `xs@.subrange(a, xs@.len())` (`encode_index`'s `RangeFrom`). -/
  | rangeFrom (lo : Expr)
end

deriving instance Repr for Expr
deriving instance Repr for Pred
deriving instance Repr for RangeArg

end Thermite
