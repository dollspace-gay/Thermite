/-
  CRITIC PIN G (#240 re-audit) — proof-backends.md §4 mechanism 1: the exporter's
  HARD GATE does NOT fire on a contract spec-call whose callee has NO definition
  anywhere in the program, and the emitted fuel-free theorem then CERTIFIES a
  bottom-poisoned wrong contract — kernel-accepted, live (`lake env lean` PROVES
  this file's `thermite_obligation_f`, which is byte-for-byte the exporter's
  emission).

  Authority chain:
  - proof-backends.md §4 mechanism 1: "The exporter computes `calledSpecFns(item)`
    and FAILS the export — refuses to write the Lean file — if
    `calledSpecFns(item) ⊄ dom(R_item)`", where `calledSpecFns(item)` is "the FULL
    EXPRESSION-POSITION PRINCIPLE: EVERY expression the export denotes against
    `R_item` ... contributes its spec-calls". A `specCall "mystery"` in the `ens`
    IS denoted against `R_item` (it resolves `env.specs "mystery"` → `none` →
    the Int-bottom `0`), so `mystery ∈ calledSpecFns(item)` and
    `dom(R_item) = ∅ ⊉ {mystery}` — the export MUST refuse with
    `IncompleteRegistry(["mystery"])`.
  - The exporter's own doc (`lean_export.rs`, `ExportRefusal::IncompleteRegistry`):
    "The export refuses to emit rather than emit an incomplete `R_item` whose
    unresolved `specCall` would bottom to the `intVal` Int-`0` and self-certify
    (Pin B / Pin C)." — EXACTLY the discharge pinned below.

  The toolchain divergence (`forge/src/lean_export.rs` @ d4871ded, fns
  `collect_expr_calls` / `export_item`): BOTH gate directions filter candidate
  names through `decls.contains_key(&segs[0])` — a call to a name with NO
  in-program definition is invisible to (i) the export-time coverage re-check,
  (ii) `build_registry`'s `calledSpecFns ⊆ dom(R_item)` check, AND (iii)
  `expr_has_spec_call`, so `tier_of` classifies the item tier (a)
  "specCall-free" (FuelFreeAuto) while the emitted goal CONTAINS
  `Expr.specCall "mystery"` at fuel 0.

  LIVE REPRODUCTION (the exporter run on the real `lean_export.rs`): for

      fn f(x: u64) -> u64 req true
        ens result == mystery(x as int) as u64
        fx pure { 0 }

  `export_item` returns Ok { tier: FuelFreeAuto, registry_names: [] } — NOT
  `Err(IncompleteRegistry(["mystery"]))` — and emits the `R_item` + theorem
  below VERBATIM. `LeanEngine::discharge` then invokes lake, which kernel-accepts
  it (this file), i.e. the engine returns Verdict::Proven for a contract whose
  RHS is an undefined symbol: `intVal 0 (specCall "mystery" …) = 0` (the fuel-0
  catch-all `| _, _, _ => 0`, Denote.lean), the body `0` binds `result := 0`,
  and the wrong contract `result == mystery(x) as u64` certifies at `0 = 0` —
  the Pin B/Pin C bottom-poisoning REACHED through the gate, on the live
  engine-#2 path. (Contrast the Verus path, where an undefined callee fails
  CLOSED with a Verus resolution error — check.rs's E0425 note.)

  Tracking: the crosslink issue filed with this pin. This file is the critic's
  audit artifact; like PinIntBottom.lean / PinBodyRegistry.lean it must keep
  compiling — `thermite_obligation_f` discharging is the documented divergence
  the gate fix must make UNREACHABLE (the fixed exporter must refuse to emit
  this file's content).
-/
import Thermite.Stabilize

namespace Thermite.PinExportUndefinedCallee

/-- The exporter's emission, VERBATIM: the EMPTY registry — `mystery` is not in
    `registry_names` and gets NO per-name `decide` lemma, so §4 mechanism 2 is
    silent too. -/
def R_item : Thermite.Registry := fun _ => none

/-- The spine ground truth the gate exists to wall off: an UNRESOLVED/fuel-0
    `specCall` bottoms to the Int-`0` (`Denote.lean` `intVal` fuel-0 catch-all
    `| _, _, _ => 0`). -/
theorem unresolved_call_bottoms_at_fuel_zero (env : Env) (args : List Expr) :
    intVal 0 (Expr.specCall "mystery" args) env = 0 := by
  simp [Thermite.intVal]

/-- The exporter's emitted theorem, VERBATIM (tier (a) fuel-free form, item `f`,
    wrong contract `ens result == mystery(x as int) as u64`, body `{ 0 }`).
    It PROVES — the wrong contract certifies through the bottom: the engine
    returns `Proven` for a contract about an UNDEFINED symbol. The proof below
    is the head of the exporter's own `auto_tactic_battery`. -/
theorem thermite_obligation_f (v : Thermite.Env) :
    Thermite.denote 0 (Thermite.Expr.boolLit true) { v with specs := R_item } →
    Thermite.denote 0 (Thermite.Expr.cmp Thermite.CmpOp.eq (Thermite.Expr.var "result") (Thermite.Expr.cast (Thermite.Expr.specCall "mystery" [(Thermite.Expr.cast (Thermite.Expr.var "x") Thermite.CastTy.int)]) Thermite.CastTy.u64))
      ((({ v with specs := R_item } : Thermite.Env)).bindInt "result"
        (Thermite.intVal 0 (Thermite.Expr.intLit 0) { v with specs := R_item })) := by
  intro hreq
  simp [Thermite.Env.bindInt, Thermite.intVal, Thermite.denote,
    Thermite.castDenote] at hreq ⊢

end Thermite.PinExportUndefinedCallee
