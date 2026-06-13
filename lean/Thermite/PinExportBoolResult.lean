/-
  Critic pin H (#240 re-audit) — resolved (#244). proof-backends.md §4 scope: the
  exporter now enforces the pure-contract class boundary on the result sort. A
  bool-returning fn's body does not denote in `intVal` (the §4 scope is "items whose
  body is a pure expression denoting in `intVal` (the `S_C` domain), so the result
  `r` is an `Int`"; the bool-result binding is named increment-(iv) work, the §4.1
  `bindBool` bridge, "since `Env` has no bool sort"). The exporter refuses a
  non-integer result with `ExportRefusal::NonIntResult` (a skip).

  The fix (`forge/src/lean_export.rs` @ #244): `export_item` checks the declared
  result type via `result_is_int_sorted`; only `u32`/`u64`/`usize`/`int`/`nat` bind
  `result : Int` faithfully, and a `bool`/unit/ADT/collection result is refused
  (`NonIntResult`). The Rust-side regression asserts both polarities refuse:
  `forge/src/lean_export.rs::tests::bool_result_item_refuses_export`.

  The degeneracy this file once pinned (both `ens result == true` and
  `ens result == false` kernel-accepting for body `{ false }`, because `intVal`
  bottoms every bool node to `0`) is now unreachable: neither item exports.

  This file remains the critic's audit artifact and must keep compiling. It no longer
  reproduces the both-prove emissions; it pins the spine ground truth that
  motivates the result-sort gate: `intVal` cannot distinguish `true` from `false`
  (both bottom to `0`), so a bool-result item carries no information and must be
  refused rather than exported.
-/
import Thermite.Stabilize

namespace Thermite.PinExportBoolResult

/-- The empty registry the (now-refused) bool items would have produced. -/
def R_item : Thermite.Registry := fun _ => none

/-- The spine ground truth the result-sort gate walls off: a boolean literal has no
    integer meaning. `intVal` bottoms it to the canonical `0` for both polarities,
    so `intVal` cannot distinguish `true` from `false` (`Denote.lean` catch-all
    `| _, _, _ => 0`). A bool-result item must therefore be refused
    (`NonIntResult`, #244): were it exported, `result` would bind to `0` for any
    body and a contract and its negation would both certify. -/
theorem boolLit_has_no_intVal_meaning (b : Bool) (env : Env) :
    intVal 0 (Expr.boolLit b) env = 0 := by
  simp [Thermite.intVal]

/-- The degeneracy in one line: `true` and `false` denote the same `intVal` (`0`), so
    no `ens result == <b>` clause can pin a bool result, which is why the result-sort
    gate refuses the whole class. -/
theorem true_false_indistinguishable_in_intVal (env : Env) :
    intVal 0 (Expr.boolLit true) env = intVal 0 (Expr.boolLit false) env := by
  simp [Thermite.intVal]

end Thermite.PinExportBoolResult
