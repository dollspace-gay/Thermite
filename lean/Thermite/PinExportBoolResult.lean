/-
  CRITIC PIN H (#240 re-audit) — RESOLVED (#244). proof-backends.md §4 SCOPE: the
  exporter now enforces the pure-contract class boundary on the RESULT SORT. A
  bool-returning fn's body does NOT denote in `intVal` (the §4 scope is "items whose
  body is a PURE EXPRESSION denoting in `intVal` (the `S_C` domain), so the result
  `r` is an `Int`"; the bool-result binding is NAMED increment-(iv) work — §4.1's
  `bindBool` bridge, "since `Env` has no bool sort"). The exporter now REFUSES a
  non-integer result with `ExportRefusal::NonIntResult` (an honest skip).

  THE FIX (`forge/src/lean_export.rs` @ #244): `export_item` checks the declared
  result type via `result_is_int_sorted` — only `u32`/`u64`/`usize`/`int`/`nat` bind
  `result : Int` faithfully; a `bool`/unit/ADT/collection result is refused
  (`NonIntResult`). The Rust-side regression asserts BOTH polarities refuse:
  `forge/src/lean_export.rs::tests::bool_result_item_refuses_export`.

  The degeneracy that USED to be pinned here (both `ens result == true` AND
  `ens result == false` kernel-accepting for body `{ false }`, because `intVal`
  bottoms every bool node to `0`) is now UNREACHABLE: neither item exports.

  This file remains the critic's audit artifact and must keep COMPILING. It no longer
  reproduces the both-prove emissions; instead it pins the SPINE GROUND TRUTH that
  MOTIVATES the result-sort gate — `intVal` cannot distinguish `true` from `false`
  (both bottom to `0`), so a bool-result item carries NO information and MUST be
  refused rather than exported.
-/
import Thermite.Stabilize

namespace Thermite.PinExportBoolResult

/-- The empty registry the (now-refused) bool items WOULD have produced. -/
def R_item : Thermite.Registry := fun _ => none

/-- The spine ground truth the result-sort gate walls off: a boolean literal has NO
    integer meaning — `intVal` bottoms it to the canonical `0` for BOTH polarities,
    so `intVal` cannot distinguish `true` from `false` (`Denote.lean` catch-all
    `| _, _, _ => 0`). Because of THIS, a bool-result item must be REFUSED
    (`NonIntResult`, #244): were it exported, `result` would bind to `0` for ANY
    body and a contract AND its negation would both certify. -/
theorem boolLit_has_no_intVal_meaning (b : Bool) (env : Env) :
    intVal 0 (Expr.boolLit b) env = 0 := by
  simp [Thermite.intVal]

/-- The degeneracy in one line: `true` and `false` denote the SAME `intVal` (`0`), so
    no `ens result == <b>` clause can pin a bool result — exactly why the result-sort
    gate refuses the whole class. -/
theorem true_false_indistinguishable_in_intVal (env : Env) :
    intVal 0 (Expr.boolLit true) env = intVal 0 (Expr.boolLit false) env := by
  simp [Thermite.intVal]

end Thermite.PinExportBoolResult
