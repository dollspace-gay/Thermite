/-
  CRITIC PIN — the env→State MIS-MAP (#253, proof-backends.md §4.1.4/§4.1.6). The fourth
  of the FOUR bridge-divergence pins. A kernel-checked DIVERGENCE ORACLE: a `stateOf` that
  DROPS the `seqs → slices` map (the exec body reads `slices xs = []` while the contract's
  `xs.len()`/`xs[i]` reads `v.seqs xs`) makes a RIGHT body FAIL to converge at a witness,
  while the faithful map agrees — and the per-param correspondence `rfl`-lemma the exporter
  emits (§4.1.4) is the COMPILE-TIME tripwire this pin motivates. The design (§4.1.6) noted
  the env→State pin "may be exporter-side" but is authored HERE because it is expressible
  against the spine: `stateOf` is a generator-emitted DEFINITION, and a mis-mapped one is a
  concrete Lean function whose divergence from the faithful one is a kernel fact.

  THE WITNESS. A contract valuation `v` with the slice `v.seqs "xs" = [7]`. A
  straight-line body `{ xs[0] }` (an exec `index` reading element 0 of the slice param).

    - FAITHFUL `stateOf` (maps `v.seqs "xs"` into the exec `slices`): the body converges
      to the genuine element `7` — `bodyConverges` holds AND the per-param correspondence
      `((stateOf v).env.slices "xs").map BVal.value = v.seqs "xs"` is `rfl`.
    - DROPPED-MAP `stateOf` (the bug — `slices := fun _ => []`): the body reads `xs[0]`
      out of range (`slices "xs" = []`), so `bodyDenote = none` — a RIGHT body FAILS to
      converge, spuriously failing the OVERFLOW class; AND the correspondence lemma is
      `[] ≠ [7]`, so the emitted `rfl`-lemma FAILS TO COMPILE (the §4.1.4 tripwire).

  Both directions are pinned: the faithful agreement AND the mis-map divergence.
-/
import Thermite.Stabilize
import Thermite.Exec.Stmt

namespace Thermite.PinExecStateMisMap

open Thermite Thermite.Exec

/-- The contract valuation `v` carrying the slice `xs := [7]` (a single `u32`-width elem). -/
def v : Env :=
  { ints := fun _ => 0
    seqs := fun s => if s = "xs" then [7] else []
    optres := fun _ => OptResVal.none_
    specs := fun _ => none }

/-- THE FAITHFUL `stateOf` (§4.1.4): the slice param `xs` maps `v.seqs "xs"` into the exec
    `slices` at its element width (`u32`), element-wise. Params are free inputs (`scope`
    all `false`). -/
def stateOfFaithful : State :=
  { env := { vars := fun _ => .int ⟨.u64, 0⟩
             slices := fun s => if s = "xs" then (v.seqs s).map (fun n => ⟨.u32, n⟩) else [] }
    scope := fun _ => false }

/-- THE DROPPED-MAP `stateOf` (the bug): the `seqs → slices` map is DROPPED — the exec
    `slices` is empty for every name, so `xs[i]` reads out of range. -/
def stateOfDropped : State :=
  { env := { vars := fun _ => .int ⟨.u64, 0⟩
             slices := fun _ => [] }
    scope := fun _ => false }

/-- The exec body `{ xs[0] }` (index element 0 of the slice param `xs`). -/
def idxBody : Block := .mk [] (some (.index "xs" (.intLit .u64 0)))

/-- **Faithful — the body converges to the genuine element `7`.** With the slice mapped,
    `xs[0]` reads element `0` of `[⟨u32,7⟩]` = `7`, so `bodyConverges` holds. -/
theorem faithful_body_converges :
    bodyConverges idxBody stateOfFaithful (.int ⟨.u32, 7⟩) := by
  simp [bodyConverges, idxBody, bodyDenote, blockThread, Block.blkTail, stateOfFaithful, v,
    execDenote, asInt, IntTy.bound, IntTy.width]

/-- **Teeth — the dropped-map body FAILS to converge.** With `slices "xs" = []`, the
    `xs[0]` index is out of range, so `bodyDenote = none` — a RIGHT body produces no value,
    spuriously failing its OVERFLOW class (`isSome = false`). -/
theorem dropped_map_body_no_result :
    bodyDenote idxBody stateOfDropped = none := by
  simp only [idxBody, bodyDenote, blockThread, Block.blkTail, stateOfDropped, execDenote,
    asInt, IntTy.bound, IntTy.width]
  decide

/-- The divergence in one statement: at the SAME contract valuation the faithful `stateOf`
    converges (`some (.int ⟨.u32, 7⟩)`) while the dropped-map `stateOf` does NOT (`none`).
    A `stateOf` that drops the `seqs → slices` map is therefore UNSOUND (it changes whether
    a right item certifies); the faithful element-wise map is what keeps the env→State
    correspondence faithful. -/
theorem dropped_map_diverges_from_faithful :
    bodyDenote idxBody stateOfFaithful ≠ bodyDenote idxBody stateOfDropped := by
  rw [dropped_map_body_no_result]
  have h : bodyConverges idxBody stateOfFaithful (.int ⟨.u32, 7⟩) := faithful_body_converges
  rw [bodyConverges] at h
  rw [h]; simp

/-- **The per-param correspondence `rfl`-lemma (§4.1.4) — the FAITHFUL map agrees.** The
    exec slice's element values equal the contract slice (`((stateOf v).env.slices "xs").map
    BVal.value = v.seqs "xs"`). For the faithful `stateOf` this is `rfl`-discharged — the
    compile-time tripwire the exporter emits alongside. -/
theorem faithful_slice_correspondence :
    (stateOfFaithful.env.slices "xs").map BVal.value = v.seqs "xs" := by
  decide

/-- **The same correspondence lemma FAILS for the dropped map** (`[] ≠ [7]`): were the
    exporter to emit a dropped-map `stateOf`, this `rfl`-lemma would FAIL TO COMPILE — the
    §4.1.4 tripwire that catches the mis-map independent of inspection. -/
theorem dropped_slice_correspondence_fails :
    (stateOfDropped.env.slices "xs").map BVal.value ≠ v.seqs "xs" := by
  simp only [stateOfDropped, v, List.map]
  decide

end Thermite.PinExecStateMisMap
