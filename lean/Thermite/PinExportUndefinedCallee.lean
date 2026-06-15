/-
  Critic pin G (#240 re-audit) — resolved (#243). proof-backends.md §4 mechanism 1:
  the exporter's hard gate now fires on a contract spec-call whose callee has no
  definition anywhere in the program. The bottom-poisoned wrong-contract theorem
  this file once pinned (the exporter emitting `Expr.specCall "mystery"` against an
  empty registry, kernel-accepting it at the fuel-0 Int-bottom) is now unreachable:
  `export_item` refuses with `ExportRefusal::IncompleteRegistry(["mystery"])` before
  any file is emitted.

  Authority chain:
  - proof-backends.md §4 mechanism 1: "The exporter computes `calledSpecFns(item)`
    and FAILS the export — refuses to write the Lean file — if
    `calledSpecFns(item) ⊄ dom(R_item)`", under the full expression-position
    principle: "EVERY expression the export denotes against `R_item` ... contributes
    its spec-calls". A `specCall "mystery"` in the `ens` is denoted against `R_item`,
    so `mystery ∈ calledSpecFns(item)` and `dom(R_item) = ∅ ⊉ {mystery}`, so the export
    must refuse.

  The fix (`forge/src/lean_export.rs` @ #243): the gate collects called names via
  `collect_all_call_names`, unfiltered (not through `decls.contains_key`), which was
  the shared blind spot that made `mystery` invisible. Any present name ∉ `decls` is
  the `IncompleteRegistry` refusal. An independent re-check walks the emitted
  Lean theorem string for `Expr.specCall "NAME"` occurrences and demands each
  `NAME ∈ dom(R_item)`. The Rust-side regression asserts the refusal:
  `forge/src/lean_export.rs::tests::undefined_callee_refuses_export`.

  This file remains the critic's audit artifact and must keep compiling. It no longer
  reproduces the divergence (the exporter cannot emit it); it pins the spine
  ground truth the gate exists to wall off: an unresolved/fuel-0 `specCall` bottoms
  to the Int-`0`, which is why an undefined callee must be refused rather
  than emitted (if it were emitted, the wrong contract would self-certify at `0 = 0`).
-/
import Thermite.Stabilize

namespace Thermite.PinExportUndefinedCallee

/-- The empty registry an undefined callee would have produced (the bug). The fixed
    exporter never reaches an emission with this registry plus a `specCall "mystery"`;
    it refuses at the hard gate. -/
def R_item : Thermite.Registry := fun _ => none

/-- The spine ground truth the gate walls off: an unresolved/fuel-0 `specCall`
    bottoms to the Int-`0` (`Denote.lean` `intVal` fuel-0 catch-all
    `| _, _, _ => 0`). An undefined callee must therefore be refused at export
    rather than emitted: were it emitted, the wrong contract `result == mystery(x) as u64`
    over a body `{ 0 }` would self-certify at `0 = 0`. The #243 fix makes that
    emission unreachable. -/
theorem unresolved_call_bottoms_at_fuel_zero (env : Env) (args : List Expr) :
    intVal 0 (Expr.specCall "mystery" args) env = 0 := by
  simp [Thermite.intVal]

end Thermite.PinExportUndefinedCallee
