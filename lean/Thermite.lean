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

  LAYER 2 (the exec side) is now OPEN: increment 2a (#171) mechanizes the
  EXEC-EXPRESSION bounded-value denotation `S_E` (`Thermite.Exec`) and proves (T1)
  `∀ pure exec Expr P, ⟦exec_ref_value(P)⟧ = ⟦P⟧_{S_E}` (`Thermite.Exec.exec_ref_sound`).
  `S_E` is a DIFFERENT semantics from `S_C`: a BOUNDED `u64`/`u32`/`usize`/`bool` value
  (NEVER nat-coerced), with arithmetic OVERFLOW carried as a PROOF OBLIGATION (the value
  is the mathematical result GIVEN no overflow; an overflowing op has NO value). The
  exec-BODY is now mechanized (2b #172): the big-step STATE TRANSFORMER `S_B` over
  straight-line blocks + the (T1) soundness proof for `body_ref_state`
  (`Thermite.Exec.body_ref_sound`). LOOPS (2c #163) remain kernel-gated.
-/
import Thermite.Ast
import Thermite.Denote
import Thermite.RefEncode
import Thermite.Soundness
-- LAYER 2 (the exec side, increment 2a, #171): the exec-expression bounded-value
-- denotation `S_E` + the (T1) soundness proof for `exec_ref_value`. SEPARATE namespace
-- `Thermite.Exec` (bounded, overflow-as-obligation, NEVER nat-coerced — `S_E ≠ S_C`).
import Thermite.Exec
-- LAYER 2 (the exec side, increment 2b, #172): the exec-BODY big-step STATE
-- TRANSFORMER `S_B` over straight-line blocks + the (T1) soundness proof for
-- `body_ref_state` (`Thermite.Exec.body_ref_sound`). Builds on 2a's `ExecExpr`/
-- `execDenote`/`ExecVal`/`ExecEnv` for every per-RHS / condition / tail value; adds
-- ONLY the state threading / scalar-mutation rebind / branch composition / tail
-- projection. LOOPS remain OUT (2c #163, kernel-gated).
import Thermite.Exec.Stmt
