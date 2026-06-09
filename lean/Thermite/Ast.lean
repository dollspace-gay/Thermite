/-
  Thermite/Ast.lean — the contract-sublanguage AST as a Lean `inductive`, for the
  COMPARISON + LOGICAL fragment (increment (a), #170) EXTENDED with the ARITHMETIC
  operators (increment (b), #176) and the CASTS (increment (c), #177); epic #169.

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

  DEFERRED — NOT embedded here, and DELIBERATELY NOT (no `sorry`-behind-a-variant;
  embedding-then-`sorry` is forbidden). These are the #178+ sub-increments:
    - the 8 frozen combinators (`forall_in`/`sorted`/… — their `verus_l3` forms),
      incl. RECURSIVE / quantified bodies
    - named spec-fn calls (the well-founded recursive `S_C` fixpoint)
    - method calls / the slice→`@` view / the `spec_*` byte-view (the #127 class)
    - `Expr::Match` / `Expr::Is` in contract position
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

/-- The contract-sublanguage expression. Three syntactic sorts are distinguished by
    the inductive shape: `intLit`/`var`/`arith`/`cast` build INTEGER terms (operands
    of a comparison); `cmp`/`logic`/`neg`/`boolLit` build BOOLEAN/`Prop` terms. A
    `var` carries a `String` name (a param, `result`, or the `old(x)` pre-state
    binding — all free integer names, per `S_C`). -/
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
  deriving Repr

end Thermite
