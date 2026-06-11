/-
  CRITIC PIN I (#240 re-audit) — RESOLVED (#245). proof-backends.md §6.1(b): the
  tier-(b) static unfolding is now CAPTURE-AVOIDING. §6.1(b) requires "the unfolded
  `Expr` must equal the spec-fn's real body substituted, arm-by-arm; a wrong
  unfolding is an unsound export". `substitute` now DETECTS a collision — a body
  binder (a closure element var / a match payload binder) that equals a FREE name of
  a still-live substituted argument — and REFUSES tier (b) for that item
  (`ExportRefusal::OutOfFragment`, capture-unsafe; the item routes to the tier-(c)
  interactive path). No silent capture is ever emitted.

  THE FIX (`forge/src/lean_export.rs` @ #245): `substitute`/`unfold_once`/
  `unfold_spec_calls` now return `Result`; at each closure / match-arm binder the
  `check_no_capture` guard compares the binder against `subst_free_names` (the free
  vars of the live substituted args, via `free_path_names`). A collision refuses; the
  shadowing direction (no live arg uses the binder name) is still handled soundly by
  removing the key. The Rust-side regression asserts the refusal AND the
  over-refusal guard (a non-capturing tier-(b) item still exports):
  `forge/src/lean_export.rs::tests::{capture_unsafe_unfolding_refuses,
  non_capturing_unfolding_still_exports}`.

  The captured-tautology emission that USED to be pinned here
  (`thermite_obligation_f3`, both sides the identical `|k| k as int == k as int`) is
  now UNREACHABLE for `f3` (the exporter refuses tier (b)).

  This file remains the critic's audit artifact and must keep COMPILING. It no longer
  reproduces the captured emission; instead it pins the MEANING GAP that MOTIVATES the
  capture guard — the captured form and the faithful (alpha-renamed) form denote
  DIFFERENTLY against the spine, so capturing would change meaning and MUST be
  refused.
-/
import Thermite.Stabilize

namespace Thermite.PinExportCapture

/-- What `unfold_once`/`substitute` WOULD have produced for `cntk(xs, k as int)`
    without the capture guard: the binder `k` captures the substituted caller `k` —
    the predicate becomes the tautology `|k| k as int == k as int`. The fixed
    exporter REFUSES this unfolding (tier (c)), never emitting it. -/
def capturedPred : Pred :=
  Pred.mk "k" (Expr.cmp CmpOp.eq (Expr.cast (Expr.var "k") CastTy.int)
                                 (Expr.cast (Expr.var "k") CastTy.int))

/-- The FAITHFUL substitution §6.1(b) requires: the binder alpha-renamed (any
    fresh name), the caller's `k` kept FREE in the predicate body. -/
def faithfulPred : Pred :=
  Pred.mk "elem" (Expr.cmp CmpOp.eq (Expr.cast (Expr.var "elem") CastTy.int)
                                    (Expr.cast (Expr.var "k") CastTy.int))

/-- The probe env: `xs = [0, 1]`, caller `k = 0`, empty registry (the unfolded
    forms are specCall-free, so the registry is irrelevant here). -/
def envI : Env :=
  { ints := fun _ => 0
    seqs := fun n => if n = "xs" then [0, 1] else []
    optres := fun _ => OptResVal.none_
    specs := fun _ => none }

/-- The CAPTURED unfolding counts ALL elements: `2` on `[0, 1]`. -/
theorem captured_unfolding_counts_all :
    intVal 0 (Expr.comb CombName.countWhere (Expr.seqVar "xs") none none
      (some capturedPred)) envI = 2 := by
  simp [Thermite.intVal, Thermite.seqVal, Thermite.denote, Thermite.Env.bindInt,
    Thermite.castDenote, capturedPred, envI, Thermite.countWhereVal]

/-- The FAITHFUL substitution counts the elements EQUAL to the caller's `k = 0`:
    `1` on `[0, 1]`. -/
theorem faithful_substitution_counts_k :
    intVal 0 (Expr.comb CombName.countWhere (Expr.seqVar "xs") none none
      (some faithfulPred)) envI = 1 := by
  simp [Thermite.intVal, Thermite.seqVal, Thermite.denote, Thermite.Env.bindInt,
    Thermite.castDenote, faithfulPred, envI, Thermite.countWhereVal]

/-- The §6.1(b) meaning gap the capture guard walls off: the captured Expr does NOT
    equal the real body substituted — their denotations differ at `envI`. Because of
    THIS, a capture-unsafe unfolding must be REFUSED (#245), never emitted. -/
theorem capture_changes_meaning :
    intVal 0 (Expr.comb CombName.countWhere (Expr.seqVar "xs") none none
      (some capturedPred)) envI
    ≠ intVal 0 (Expr.comb CombName.countWhere (Expr.seqVar "xs") none none
      (some faithfulPred)) envI := by
  rw [captured_unfolding_counts_all, faithful_substitution_counts_k]
  decide

end Thermite.PinExportCapture
