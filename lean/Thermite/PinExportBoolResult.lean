/-
  CRITIC PIN H (#240 re-audit) — proof-backends.md §4 SCOPE: the exporter does
  NOT enforce the pure-contract class boundary on the RESULT SORT. A
  bool-returning fn's body does NOT denote in `intVal` (the §4 scope is "items
  whose body is a PURE EXPRESSION denoting in `intVal` (the `S_C` domain), so
  the result `r` is an `Int`"; the bool-result binding is NAMED increment-(iv)
  work — §4.1: "the bool-result binding (a `bindBool` spine addition — an
  increment-(iv) prerequisite, since `Env` has no bool sort)"). The required
  behavior is a structured `ExportRefusal::NotPureContract` (an honest skip).

  The toolchain divergence (`forge/src/lean_export.rs` @ d4871ded, fn
  `export_item`): the pure-contract check is `pure_tail_of_block` ONLY — the
  SHAPE of the body (single tail expr), never its SORT. A `-> bool` fn with body
  `{ false }` exports tier (a), and `Denote.lean`'s `intVal` bottoms EVERY
  boolean-sorted node to the canonical `0` (the catch-all `| _, _, _ => 0`,
  documented "a boolean-sorted node never appears as a comparison operand in a
  well-formed clause"). So `result` is bound to `0` REGARDLESS of the body's
  boolean value, and `true`/`false` literals in the `ens` ALSO denote `0`.

  THE DEGENERACY PINNED BELOW (kernel-accepted, both): for the SAME item

      fn always(a: u32) -> bool req true ens result == true  fx pure { false }
      fn always(a: u32) -> bool req true ens result == false fx pure { false }

  the exporter emits the two theorems below (VERBATIM, live-reproduced on the
  real `lean_export.rs` — both `Ok`/`FuelFreeAuto`, NOT `NotPureContract`), and
  BOTH kernel-accept: a contract and its NEGATION both certify `Proven` on the
  live engine-#2 path (`ens result == true` is semantically FALSE for the body
  `{ false }` — Verus refutes it; the LeanEngine PROVES it → a REQ-5
  Proven⊕Refuted engine-disagreement on a frozen-subset item). This is the
  wrong-certificate class: the exported theorem is about the wrong program
  (every bool collapses to the Int-bottom 0).

  Tracking: the crosslink issue filed with this pin. This file is the critic's
  audit artifact and must keep compiling — the fixed exporter must REFUSE
  (NotPureContract) the items whose emissions are pinned here.
-/
import Thermite.Stabilize

namespace Thermite.PinExportBoolResult

/-- The exporter's emission, VERBATIM: no spec-fns → the empty registry. -/
def R_item : Thermite.Registry := fun _ => none

/-- The spine ground truth: a boolean literal has NO integer meaning — `intVal`
    bottoms it to the canonical `0` for BOTH polarities, so `intVal` cannot
    distinguish `true` from `false` (`Denote.lean` catch-all `| _, _, _ => 0`). -/
theorem boolLit_has_no_intVal_meaning (b : Bool) (env : Env) :
    intVal 0 (Expr.boolLit b) env = 0 := by
  simp [Thermite.intVal]

/-- The exporter's emitted theorem, VERBATIM, for `ens result == true` with body
    `{ false }` — a semantically FALSE contract. It PROVES (the false
    certification): both sides bottom to `0`. -/
theorem thermite_obligation_always_ens_true (v : Thermite.Env) :
    Thermite.denote 0 (Thermite.Expr.boolLit true) { v with specs := R_item } →
    Thermite.denote 0 (Thermite.Expr.cmp Thermite.CmpOp.eq (Thermite.Expr.var "result") (Thermite.Expr.boolLit true))
      ((({ v with specs := R_item } : Thermite.Env)).bindInt "result"
        (Thermite.intVal 0 (Thermite.Expr.boolLit false) { v with specs := R_item })) := by
  intro hreq
  simp [Thermite.Env.bindInt, Thermite.intVal, Thermite.denote] at hreq ⊢

/-- The exporter's emitted theorem for the NEGATED contract `ens result == false`
    with the SAME body `{ false }`. It ALSO proves — a contract and its negation
    both certify, so the emitted obligation carries NO information about the
    bool result (the encoding is degenerate outside `intVal`'s domain). -/
theorem thermite_obligation_always_ens_false (v : Thermite.Env) :
    Thermite.denote 0 (Thermite.Expr.boolLit true) { v with specs := R_item } →
    Thermite.denote 0 (Thermite.Expr.cmp Thermite.CmpOp.eq (Thermite.Expr.var "result") (Thermite.Expr.boolLit false))
      ((({ v with specs := R_item } : Thermite.Env)).bindInt "result"
        (Thermite.intVal 0 (Thermite.Expr.boolLit false) { v with specs := R_item })) := by
  intro hreq
  simp [Thermite.Env.bindInt, Thermite.intVal, Thermite.denote] at hreq ⊢

end Thermite.PinExportBoolResult
