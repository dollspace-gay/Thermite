/-
  CRITIC PIN G (#240 re-audit) — RESOLVED (#243). proof-backends.md §4 mechanism 1:
  the exporter's HARD GATE now FIRES on a contract spec-call whose callee has NO
  definition anywhere in the program. The bottom-poisoned wrong-contract theorem
  this file used to PIN (the exporter emitting `Expr.specCall "mystery"` against an
  EMPTY registry, kernel-accepting it at the fuel-0 Int-bottom) is now UNREACHABLE:
  `export_item` REFUSES with `ExportRefusal::IncompleteRegistry(["mystery"])` BEFORE
  any file is emitted.

  Authority chain:
  - proof-backends.md §4 mechanism 1: "The exporter computes `calledSpecFns(item)`
    and FAILS the export — refuses to write the Lean file — if
    `calledSpecFns(item) ⊄ dom(R_item)`", under the FULL EXPRESSION-POSITION
    PRINCIPLE: "EVERY expression the export denotes against `R_item` ... contributes
    its spec-calls". A `specCall "mystery"` in the `ens` IS denoted against `R_item`,
    so `mystery ∈ calledSpecFns(item)` and `dom(R_item) = ∅ ⊉ {mystery}` — the export
    MUST refuse.

  THE FIX (`forge/src/lean_export.rs` @ #243): the gate collects called names via
  `collect_all_call_names` — UNFILTERED (NOT through `decls.contains_key`), which was
  the shared blind spot that made `mystery` invisible. Any present name ∉ `decls` is
  the `IncompleteRegistry` refusal. A GENUINELY INDEPENDENT re-check walks the EMITTED
  Lean theorem string for `Expr.specCall "NAME"` occurrences and demands each
  `NAME ∈ dom(R_item)`. The Rust-side regression asserts the refusal:
  `forge/src/lean_export.rs::tests::undefined_callee_refuses_export`.

  This file remains the critic's audit artifact and must keep COMPILING. It no longer
  reproduces the divergence (the exporter cannot emit it); instead it pins the SPINE
  GROUND TRUTH the gate exists to wall off — an unresolved/fuel-0 `specCall` bottoms
  to the Int-`0`, which is PRECISELY why an undefined callee must be refused rather
  than emitted (if it were emitted, the wrong contract would self-certify at `0 = 0`).
-/
import Thermite.Stabilize

namespace Thermite.PinExportUndefinedCallee

/-- The empty registry an undefined callee WOULD have produced (the bug). The fixed
    exporter never reaches an emission with this registry + a `specCall "mystery"` —
    it refuses at the hard gate. -/
def R_item : Thermite.Registry := fun _ => none

/-- The spine ground truth the gate walls off: an UNRESOLVED/fuel-0 `specCall`
    bottoms to the Int-`0` (`Denote.lean` `intVal` fuel-0 catch-all
    `| _, _, _ => 0`). Because of THIS, an undefined callee must be REFUSED at export
    (not emitted) — were it emitted, the wrong contract `result == mystery(x) as u64`
    over a body `{ 0 }` would self-certify at `0 = 0`. The #243 fix makes that
    emission unreachable. -/
theorem unresolved_call_bottoms_at_fuel_zero (env : Env) (args : List Expr) :
    intVal 0 (Expr.specCall "mystery" args) env = 0 := by
  simp [Thermite.intVal]

end Thermite.PinExportUndefinedCallee
