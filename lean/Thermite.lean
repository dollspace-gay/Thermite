/-
  Thermite.lean — the library root for the Lean 4 side of the Thermite toolchain
  (the verified-validator metatheory; `.design/verified/thermite-semantics.md`
  REQ-6, increment (a), #170; epic #169).

  This increment proves (T1) SOUNDNESS of the contract-TV reference encoder on the
  COMPARISON + LOGICAL fragment (#170) EXTENDED through arithmetic + coercions (#176/
  #177), the spec-context rewrites (#178), the 6 bounded-quantifier combinators (#179),
  the C7 match-in-ens / `is` forms (#180), and the NAMED SPEC-FN CALLS incl. well-founded
  RECURSION (#181) — the kernel-checked opening move of the universal lowering
  semantic-preservation proof. The remaining deferred constructs (the 2 recursive
  combinators `count_where`/`permutation_of` #182, general user-ADT match/is) are the
  future sub-increments, listed in `Ast.lean` (NOT embedded-then-`sorry`).
-/
import Thermite.Ast
import Thermite.Denote
import Thermite.RefEncode
import Thermite.Soundness
