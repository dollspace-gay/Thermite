/-
  Thermite/Denote.lean — the SOURCE denotation `⟦·⟧_{S_C}` for the comparison +
  logical contract fragment (increment (a), #170).

  Governing design: `.design/verified/thermite-semantics.md` Architecture §"S_C —
  the spec/contract sublanguage", REQ-1. This is `S_C` RESTRICTED to the fragment
  `Ast.lean` embeds: comparisons/logical denote the corresponding math relation
  (the STANDARD meaning), `Eq → =`, `Le → ≤`, `And → ∧`, `Not → ¬`, etc.

  An `Env` maps each free name (a param / `result` / `old(x)`) to an `Int` — exactly
  the per-clause obligation binding `ref_encode.rs` describes (each free var, incl.
  `result`/`old(_)`, is a distinct param). The denotation is a TOTAL structural
  recursion (a clause is a pure predicate; no state, no loops — `contract-tv.md`).
-/
import Thermite.Ast

namespace Thermite

/-- The denotation environment: a valuation of free integer names (params,
    `result`, `old(x)`) to `Int`. `S_C`'s `Env := name → Int`. -/
abbrev Env := String → Int

/-- `⟦·⟧_{S_C}` on the INTEGER-valued leaves (the operands of a comparison): a
    literal denotes itself, a variable denotes its environment value. These are the
    "literals / refs denote themselves" rules of `S_C`. -/
def intVal : Expr → Env → Int
  | Expr.intLit n, _   => n
  | Expr.var x,    env => env x
  -- A boolean-sorted node never appears as a comparison operand in a well-formed
  -- clause; it has no integer meaning, so it denotes the canonical `0`. (The
  -- soundness theorem only ever evaluates `intVal` on `intLit`/`var` subterms —
  -- the operands `cmp` builds — so this default is never observed there; it keeps
  -- `intVal` TOTAL without a `sorry`/partial annotation.)
  | _, _ => 0

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
  -- An integer-sorted leaf (`intLit`/`var`) is not a predicate on its own; in a
  -- well-formed clause it only appears as a comparison operand (handled by
  -- `intVal` above). As a top-level predicate it denotes `True` vacuously — never
  -- reached by the soundness theorem (whose `Expr`s are always cmp/logic/neg/bool
  -- at the top). Keeps `denote` TOTAL with no `sorry`.
  | _, _ => True

end Thermite
