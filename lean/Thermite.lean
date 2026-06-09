/-
  Thermite.lean — the library root for the Lean 4 side of the Thermite toolchain
  (the verified-validator metatheory; `.design/verified/thermite-semantics.md`
  REQ-6, increment (a), #170; epic #169).

  This increment proves (T1) SOUNDNESS of the contract-TV reference encoder on the
  COMPARISON + LOGICAL fragment — the kernel-checked opening move of the
  universal lowering semantic-preservation proof. The deferred constructs
  (arithmetic + coercions, the 8 combinators, spec-fn calls, method/slice/byte-view
  rewrites, match/is) are the #171+ sub-increments, listed in `Ast.lean`.
-/
import Thermite.Ast
import Thermite.Denote
import Thermite.RefEncode
import Thermite.Soundness
