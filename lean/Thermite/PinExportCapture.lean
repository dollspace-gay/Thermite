/-
  CRITIC PIN I (#240 re-audit) — proof-backends.md §6.1(b): the tier-(b) static
  unfolding (`forge/src/lean_export.rs` @ d4871ded, fns `unfold_once` /
  `substitute`) is NOT capture-avoiding. §6.1(b) requires "the unfolded `Expr`
  must equal the spec-fn's real body substituted, arm-by-arm; a wrong unfolding
  is an unsound export". `substitute` removes a SHADOWED key at a closure binder
  (the param-shadowing direction) but never RENAMES a binder when the
  substituted ARGUMENT carries a free name equal to that binder — so a caller
  variable named like a predicate-closure element var is CAPTURED.

  LIVE REPRODUCTION (the exporter run on the real `lean_export.rs`): for

      spec fn cntk(xs: &[u32], v: int) -> int dec xs.len()
        { count_where(xs, |k| k as int == v) }
      spec fn cntall(xs: &[u32]) -> int dec xs.len()
        { count_where(xs, |k| k as int == k as int) }
      fn f3(xs: &[u32], k: u32) -> u64 req true
        ens cntk(xs, k as int) == cntall(xs) fx pure { 0 }

  unfolding `cntk(xs, k as int)` substitutes `v ↦ (k as int)` UNDER the binder
  `|k| …`, producing `|k| k as int == k as int` — the CALLER's `k` is captured
  and the count-of-elements-EQUAL-TO-k becomes count-of-ALL. The source `ens`
  is semantically FALSE (at `xs = [0, 1], k = 0` it says `1 == 2`), but the
  emitted theorem (`thermite_obligation_f3` below, VERBATIM) renders BOTH sides
  as the identical captured Expr and PROVES — kernel-accepted in this file.
  (Today `LeanEngine` happens to return Unknown on this item only because the
  shipped `auto_tactic_battery` errors with "No goals to be solved" after its
  leading `simp only` closes the goal — an accident of battery structure, not a
  soundness gate. The export ARTIFACT is the divergence: a kernel-acceptable
  theorem about the WRONG program.)

  The meaning gap, kernel-checked against the spine (`Denote.lean`'s
  `intVal`/`countWhereVal`): at the env `xs = [0, 1], k = 0`,
  - the CAPTURED unfolding denotes `2` (`captured_unfolding_counts_all`),
  - the FAITHFUL substitution (binder alpha-renamed, the caller's `k` kept
    free) denotes `1` (`faithful_substitution_counts_k`),
  so the unfolded Expr ≠ the real body substituted (`capture_changes_meaning`)
  — the §6.1(b) EXP requirement is violated.

  Tracking: the crosslink issue filed with this pin. This file is the critic's
  audit artifact and must keep compiling — the fixed (capture-avoiding)
  unfolder must never emit `capturedPred` for this program.
-/
import Thermite.Stabilize

namespace Thermite.PinExportCapture

/-- The exporter's emission, VERBATIM: `R_item` with both spec-fns, real-bodied
    (the registry itself is fine — the divergence is in the UNFOLDED theorem). -/
def R_item : Thermite.Registry := fun name =>
  match name with
  | "cntall" => some ⟨["xs"], (Thermite.Expr.comb Thermite.CombName.countWhere (Thermite.Expr.seqVar "xs") none none (some (Thermite.Pred.mk "k" (Thermite.Expr.cmp Thermite.CmpOp.eq (Thermite.Expr.cast (Thermite.Expr.var "k") Thermite.CastTy.int) (Thermite.Expr.cast (Thermite.Expr.var "k") Thermite.CastTy.int)))))⟩
  | "cntk" => some ⟨["xs", "v"], (Thermite.Expr.comb Thermite.CombName.countWhere (Thermite.Expr.seqVar "xs") none none (some (Thermite.Pred.mk "k" (Thermite.Expr.cmp Thermite.CmpOp.eq (Thermite.Expr.cast (Thermite.Expr.var "k") Thermite.CastTy.int) (Thermite.Expr.var "v")))))⟩
  | _ => none

/-- What `unfold_once`/`substitute` ACTUALLY produce for `cntk(xs, k as int)`:
    the binder `k` captures the substituted caller `k` — the predicate is the
    tautology `|k| k as int == k as int`. -/
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

/-- The §6.1(b) violation: the unfolded Expr does NOT equal the real body
    substituted — their denotations differ at `envI`. -/
theorem capture_changes_meaning :
    intVal 0 (Expr.comb CombName.countWhere (Expr.seqVar "xs") none none
      (some capturedPred)) envI
    ≠ intVal 0 (Expr.comb CombName.countWhere (Expr.seqVar "xs") none none
      (some faithfulPred)) envI := by
  rw [captured_unfolding_counts_all, faithful_substitution_counts_k]
  decide

/-- The exporter's emitted theorem, VERBATIM (tier (b), item `f3`): the
    semantically FALSE contract `ens cntk(xs, k as int) == cntall(xs)` rendered
    with BOTH sides as the identical captured Expr — it PROVES (the false
    contract's obligation is kernel-acceptable). The proof is the battery's
    leading simp step (trimmed to the used lemmas). -/
theorem thermite_obligation_f3 (v : Thermite.Env) :
    Thermite.denote 0 (Thermite.Expr.boolLit true) { v with specs := R_item } →
    Thermite.denote 0 (Thermite.Expr.cmp Thermite.CmpOp.eq (Thermite.Expr.comb Thermite.CombName.countWhere (Thermite.Expr.seqVar "xs") none none (some (Thermite.Pred.mk "k" (Thermite.Expr.cmp Thermite.CmpOp.eq (Thermite.Expr.cast (Thermite.Expr.var "k") Thermite.CastTy.int) (Thermite.Expr.cast (Thermite.Expr.var "k") Thermite.CastTy.int))))) (Thermite.Expr.comb Thermite.CombName.countWhere (Thermite.Expr.seqVar "xs") none none (some (Thermite.Pred.mk "k" (Thermite.Expr.cmp Thermite.CmpOp.eq (Thermite.Expr.cast (Thermite.Expr.var "k") Thermite.CastTy.int) (Thermite.Expr.cast (Thermite.Expr.var "k") Thermite.CastTy.int))))))
      ((({ v with specs := R_item } : Thermite.Env)).bindInt "result"
        (Thermite.intVal 0 (Thermite.Expr.intLit 0) { v with specs := R_item })) := by
  intro hreq
  simp [Thermite.Env.bindInt, Thermite.intVal, Thermite.denote,
    Thermite.castDenote] at hreq ⊢

end Thermite.PinExportCapture
