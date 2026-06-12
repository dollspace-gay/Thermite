/-
  CRITIC PIN — the WHILE-BODY COMPOSITION MIS-MAP (#264, proof-backends.md §4.2.6
  REQ-11.6). The second of the TWO (v) bridge-divergence pins. A `whileBodyDenote`
  variant that SKIPS the loop segment (prefix → tail directly) — or iterates the WRONG
  block — certifies a wrong `ens` true ONLY at the ENTRY value, where the FAITHFUL
  composition (`whileBodyDenote`, prefix → the SHIPPED `loopDenote` → tail) refutes it
  at the genuine EXIT value. Both directions are pinned: the POISONED discharge AND the
  FAITHFUL refutation.

  THE WITNESS (§4.2.6 — "the L1 fixture's `lo = 0` entry vs `lo = 3` exit is the natural
  witness shape"). The SHIPPED L1 loop (`Exec/Loop.lean`):

      while lo < n  inv lo <= n  dec n - lo  { lo = lo + 1 }      -- `n := 3`

  with an EMPTY prefix and the tail value `lo` (`Expr` `.var "lo"`). The FAITHFUL
  `whileBodyDenote` runs the loop (`lo : 0 → 1 → 2 → 3`, `b_loop_iterates`) and the tail
  reads the EXIT value `lo = 3`. The SKIP variant (`skipBodyDenote`) drops the loop and
  reads the tail at the ENTRY state, giving `lo = 0`.

  The wrong contract `ens := result == 0` is TRUE at the entry value `0` and FALSE at the
  genuine exit value `3`:

    - THE POISONED DISCHARGE — the SKIP composition binds `result = 0`, so
      `ens: result == 0` CERTIFIES (`skip_composition_certifies_wrong_ens`). A
      composition that skips/mis-orders the loop launders a wrong contract.
    - THE FAITHFUL REFUTATION — the genuine `whileBodyConverges` binds the EXIT value
      `result = 3`, so `ens: result == 0` is REFUTED
      (`faithful_composition_refutes_wrong_ens`). The faithful loop-exit value (`lo = 3`)
      is the one the contract must be checked against — `while_compose` ties the converged
      result to the genuine `I ∧ ¬cond` exit, never the entry.

  The rule-level teeth (`h_pres` load-bearing, the exit characterization exactly
  `inv ∧ ¬cond`) already EXIST on the spine (`l2_no_preservation_premise_for_buggy_body`,
  `l3_exit_overclaim_refuted`, `Exec/Loop.lean`) and are NOT re-pinned. This file is the
  audit artifact; like the other pins it must keep compiling (kernel-checked, standard
  axioms only).
-/
import Thermite.Stabilize
import Thermite.Exec.WhileBody

namespace Thermite.PinWhileComposition

open Thermite Thermite.Exec

/-- A contract env with nothing bound (the `ens` reads only `result`, bound by the bridge). -/
def baseEnv : Env :=
  { ints := fun _ => 0
    seqs := fun _ => []
    optres := fun _ => OptResVal.none_
    specs := fun _ => none }

/-- The empty no-op prefix (the loop is the whole body before the tail). -/
def noPrefix : Block := .mk [] none

/-- The body's tail value: `lo` (`Expr.var "lo"`) — read in the FINAL (exit) state by the
    faithful composition, at the ENTRY state by the skip variant. -/
def loTail : ExecExpr := .var "lo"

/-! ## The FAITHFUL composition — runs the loop, reads the EXIT value `lo = 3` -/

/-- **The faithful `whileBodyDenote` converges to the EXIT value `lo = 3`.** With the empty
    prefix, the SHIPPED L1 loop (`l1Cond`/`l1Body`, `l1State` `lo := 0, n := 3`) exits at
    fuel `4` with `lo = 3` (`b_loop_iterates`), and the tail `lo` reads that exit value. -/
theorem faithful_converges_to_exit :
    whileBodyDenote noPrefix l1Cond l1Body loTail 4 l1State = some (.int ⟨.usize, 3⟩) := by
  -- the prefix is the identity; the loop result's `lo` cell is `3` (`b_loop_iterates`).
  have hmap := b_loop_iterates
  rw [Option.map_eq_some_iff] at hmap
  obtain ⟨stf, hrun, hlo⟩ := hmap
  simp only [whileBodyDenote, noPrefix, blockThread, bind, Option.bind, hrun, loTail,
    execDenote, hlo]

/-- The faithful exit value `3`, bound THROUGH the convergence relation (the #214 form). -/
theorem faithful_whileBodyConverges :
    whileBodyConverges noPrefix l1Cond l1Body loTail l1State (.int ⟨.usize, 3⟩) :=
  ⟨4, faithful_converges_to_exit⟩

/-! ## The SKIP variant — drops the loop, reads the ENTRY value `lo = 0` -/

/-- THE COMPOSITION MIS-MAP: a `whileBodyDenote` variant that SKIPS the loop segment —
    the prefix's state flows DIRECTLY into the tail, the genuine `loopDenote` iteration
    never runs. (The "prefix → tail directly" infidelity of §4.2.6.) -/
def skipBodyDenote (prefixB : Block) (tail : ExecExpr) (st : State) : Option ExecVal := do
  let st₁ ← blockThread prefixB st
  execDenote tail st₁.env      -- BUG: the loop segment is dropped; the tail reads the ENTRY

/-- **The SKIP variant reads the ENTRY value `lo = 0`.** With the empty prefix, the tail
    `lo` is read at `l1State` (`lo := 0`) — the loop never ran. -/
theorem skip_reads_entry :
    skipBodyDenote noPrefix loTail l1State = some (.int ⟨.usize, 0⟩) := by
  simp only [skipBodyDenote, noPrefix, blockThread, bind, Option.bind, loTail, execDenote,
    l1State, if_neg (by decide : ¬ ("lo" = "n"))]

/-! ## The two directions: the poisoned discharge AND the faithful refutation -/

/-- The wrong contract `ens := result == 0` — TRUE at the entry value `0`, FALSE at the
    genuine exit value `3`. -/
def ensWrong : Expr := Expr.cmp CmpOp.eq (Expr.var "result") (Expr.intLit 0)

/-- **Direction 1 (the POISONED discharge) — the SKIP composition CERTIFIES the wrong
    `ens`.** The skip variant binds `result = 0` (the entry value), at which `ens: result
    == 0` denotes TRUE. A composition that skips/mis-orders the loop launders a wrong
    contract — exactly the divergence the faithful composition must refuse. -/
theorem skip_composition_certifies_wrong_ens :
    skipBodyDenote noPrefix loTail l1State = some (.int ⟨.usize, 0⟩)
    ∧ denote 0 ensWrong (bindResult baseEnv (.int ⟨.usize, 0⟩)) := by
  refine ⟨skip_reads_entry, ?_⟩
  simp only [ensWrong, denote, intVal, bindResult, baseEnv, Env.bindInt, if_pos]

/-- **Direction 2 (the FAITHFUL refutation) — the genuine composition REFUTES the wrong
    `ens`.** The faithful `whileBodyConverges` binds the EXIT value `result = 3`
    (`faithful_whileBodyConverges`), at which `ens: result == 0` denotes FALSE (`3 ≠ 0`).
    The faithful loop-exit value is the one the contract is checked against (`while_compose`
    ties the result to the genuine `I ∧ ¬cond` exit, never the entry). So the skip
    discharge above is purely the loop-skip artifact — the faithful form catches it. -/
theorem faithful_composition_refutes_wrong_ens :
    whileBodyConverges noPrefix l1Cond l1Body loTail l1State (.int ⟨.usize, 3⟩)
    ∧ ¬ denote 0 ensWrong (bindResult baseEnv (.int ⟨.usize, 3⟩)) := by
  refine ⟨faithful_whileBodyConverges, ?_⟩
  simp only [ensWrong, denote, intVal, bindResult, baseEnv, Env.bindInt]
  decide

/-- The composition-mis-map oracle: the SKIP variant certifies the wrong contract at the
    ENTRY value, while the FAITHFUL composition refutes it at the genuine EXIT value. The
    faithful `whileBodyDenote` (prefix → the SHIPPED `loopDenote` → tail) is the one that
    must reach the certificate — a loop-skipping / mis-ordered composition is caught. -/
theorem composition_mismap_caught :
    (skipBodyDenote noPrefix loTail l1State = some (.int ⟨.usize, 0⟩)
      ∧ denote 0 ensWrong (bindResult baseEnv (.int ⟨.usize, 0⟩)))
    ∧ (whileBodyConverges noPrefix l1Cond l1Body loTail l1State (.int ⟨.usize, 3⟩)
      ∧ ¬ denote 0 ensWrong (bindResult baseEnv (.int ⟨.usize, 3⟩))) :=
  ⟨skip_composition_certifies_wrong_ens, faithful_composition_refutes_wrong_ens⟩

end Thermite.PinWhileComposition
