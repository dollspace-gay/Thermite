/-
  Thermite/Ast.lean — the contract-sublanguage AST as a Lean `inductive`, for the
  COMPARISON + LOGICAL fragment only (increment (a), #170; epic #169).

  Governing design: `.design/verified/thermite-semantics.md` REQ-1/REQ-6 (the
  `S_C` denotation domain; the Lean module layout) + AC-1 (S is stated over the
  exact frozen subset the encoders admit).

  This mirrors the relevant `thermite-syntax/src/ast.rs` `Expr` / `BinOp` variants
  for THIS fragment ONLY:
    - integer literals          (`Expr::IntLit { value, .. }`)
    - bool literals             (`Expr::BoolLit(b)`)
    - variables / `result` / `old(x)`  (`Expr::Path` + the `old(_)` call form —
      all denote as free names of type `Int`, per `S_C`'s "literals/refs denote
      themselves" rule; the obligation binds each as a distinct param)
    - comparison binops         (`BinOp::{Eq,Ne,Lt,Le,Gt,Ge}`)
    - logical binops + negation (`BinOp::{And,Or}`, `UnaryOp::Not`)

  DEFERRED — NOT embedded here, and DELIBERATELY NOT (no `sorry`-behind-a-variant;
  embedding-then-`sorry` is forbidden). These are the #171+ sub-increments:
    - arithmetic binops (`Add/Sub/Mul/Div/Rem/Shl/Shr/BitAnd/BitOr/BitXor`)
      and the `as nat`/`as int` coercions (the #122 paren / coercion class)
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

/-- The contract-sublanguage expression, restricted to the comparison + logical
    fragment (#170). Two syntactic sorts are distinguished by the inductive shape:
    `intLit`/`var` build INTEGER terms (operands of a comparison); `cmp`/`logic`/`neg`
    /`boolLit` build BOOLEAN/`Prop` terms. A `var` carries a `String` name (a param,
    `result`, or the `old(x)` pre-state binding — all free integer names, per `S_C`). -/
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
  deriving Repr

end Thermite
