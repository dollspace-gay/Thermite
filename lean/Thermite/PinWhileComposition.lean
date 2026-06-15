/-
  Critic pin — the while-body composition mis-map (#264, proof-backends.md §4.2.6
  REQ-11.6). The second of the two (v) bridge-divergence pins. A `whileBodyDenote`
  variant that skips the loop segment (prefix → tail directly), or iterates the wrong
  block, certifies a wrong `ens` true only at the entry value, where the faithful
  composition (`whileBodyDenote`, prefix → the shipped `loopDenote` → tail) refutes it
  at the genuine exit value. Both directions are pinned: the poisoned discharge and the
  faithful refutation.

  The witness (§4.2.6, "the L1 fixture's `lo = 0` entry vs `lo = 3` exit is the natural
  witness shape"). The shipped L1 loop (`Exec/Loop.lean`):

      while lo < n  inv lo <= n  dec n - lo  { lo = lo + 1 }      -- `n := 3`

  with an empty prefix and the tail value `lo` (`Expr` `.var "lo"`). The faithful
  `whileBodyDenote` runs the loop (`lo : 0 → 1 → 2 → 3`, `b_loop_iterates`) and the tail
  reads the exit value `lo = 3`. The skip variant (`skipBodyDenote`) drops the loop and
  reads the tail at the entry state, giving `lo = 0`.

  The wrong contract `ens := result == 0` is true at the entry value `0` and false at the
  genuine exit value `3`:

    - The poisoned discharge: the skip composition binds `result = 0`, so
      `ens: result == 0` certifies (`skip_composition_certifies_wrong_ens`). A
      composition that skips or mis-orders the loop launders a wrong contract.
    - The faithful refutation: the genuine `whileBodyConverges` binds the exit value
      `result = 3`, so `ens: result == 0` is refuted
      (`faithful_composition_refutes_wrong_ens`). The faithful loop-exit value (`lo = 3`)
      is the one the contract must be checked against; `while_compose` ties the converged
      result to the genuine `I ∧ ¬cond` exit, never the entry.

  The rule-level teeth (`h_pres` load-bearing, the exit characterization `inv ∧ ¬cond`)
  already exist on the spine (`l2_no_preservation_premise_for_buggy_body`,
  `l3_exit_overclaim_refuted`, `Exec/Loop.lean`) and are not re-pinned. This file is the
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

/-- The body's tail value: `lo` (`Expr.var "lo"`), read in the final (exit) state by the
    faithful composition, at the entry state by the skip variant. -/
def loTail : ExecExpr := .var "lo"

/-! ## The faithful composition — runs the loop, reads the exit value `lo = 3` -/

/-- The faithful `whileBodyDenote` converges to the exit value `lo = 3`. With the empty
    prefix, the shipped L1 loop (`l1Cond`/`l1Body`, `l1State` `lo := 0, n := 3`) exits at
    fuel `4` with `lo = 3` (`b_loop_iterates`), and the tail `lo` reads that exit value. -/
theorem faithful_converges_to_exit :
    whileBodyDenote noPrefix l1Cond l1Body loTail 4 l1State = some (.int ⟨.usize, 3⟩) := by
  -- the prefix is the identity; the loop result's `lo` cell is `3` (`b_loop_iterates`).
  have hmap := b_loop_iterates
  rw [Option.map_eq_some_iff] at hmap
  obtain ⟨stf, hrun, hlo⟩ := hmap
  simp only [whileBodyDenote, noPrefix, blockThread, bind, Option.bind, hrun, loTail,
    execDenote, hlo]

/-- The faithful exit value `3`, bound through the convergence relation (the #214 form). -/
theorem faithful_whileBodyConverges :
    whileBodyConverges noPrefix l1Cond l1Body loTail l1State (.int ⟨.usize, 3⟩) :=
  ⟨4, faithful_converges_to_exit⟩

/-! ## The skip variant — drops the loop, reads the entry value `lo = 0` -/

/-- The composition mis-map: a `whileBodyDenote` variant that skips the loop segment.
    The prefix's state flows directly into the tail, and the genuine `loopDenote`
    iteration never runs. (The "prefix → tail directly" infidelity of §4.2.6.) -/
def skipBodyDenote (prefixB : Block) (tail : ExecExpr) (st : State) : Option ExecVal := do
  let st₁ ← blockThread prefixB st
  execDenote tail st₁.env      -- bug: the loop segment is dropped; the tail reads the entry

/-- The skip variant reads the entry value `lo = 0`. With the empty prefix, the tail
    `lo` is read at `l1State` (`lo := 0`); the loop never ran. -/
theorem skip_reads_entry :
    skipBodyDenote noPrefix loTail l1State = some (.int ⟨.usize, 0⟩) := by
  simp only [skipBodyDenote, noPrefix, blockThread, bind, Option.bind, loTail, execDenote,
    l1State, if_neg (by decide : ¬ ("lo" = "n"))]

/-! ## The two directions: the poisoned discharge and the faithful refutation -/

/-- The wrong contract `ens := result == 0`: true at the entry value `0`, false at the
    genuine exit value `3`. -/
def ensWrong : Expr := Expr.cmp CmpOp.eq (Expr.var "result") (Expr.intLit 0)

/-- Direction 1 (the poisoned discharge): the skip composition certifies the wrong
    `ens`. The skip variant binds `result = 0` (the entry value), at which `ens: result
    == 0` denotes true. A composition that skips or mis-orders the loop launders a wrong
    contract, the divergence the faithful composition must refuse. -/
theorem skip_composition_certifies_wrong_ens :
    skipBodyDenote noPrefix loTail l1State = some (.int ⟨.usize, 0⟩)
    ∧ denote 0 ensWrong (bindResult baseEnv (.int ⟨.usize, 0⟩)) := by
  refine ⟨skip_reads_entry, ?_⟩
  simp only [ensWrong, denote, intVal, bindResult, baseEnv, Env.bindInt, if_pos]

/-- Direction 2 (the faithful refutation): the genuine composition refutes the wrong
    `ens`. The faithful `whileBodyConverges` binds the exit value `result = 3`
    (`faithful_whileBodyConverges`), at which `ens: result == 0` denotes false (`3 ≠ 0`).
    The faithful loop-exit value is the one the contract is checked against (`while_compose`
    ties the result to the genuine `I ∧ ¬cond` exit, never the entry). So the skip
    discharge above is the loop-skip artifact, which the faithful form catches. -/
theorem faithful_composition_refutes_wrong_ens :
    whileBodyConverges noPrefix l1Cond l1Body loTail l1State (.int ⟨.usize, 3⟩)
    ∧ ¬ denote 0 ensWrong (bindResult baseEnv (.int ⟨.usize, 3⟩)) := by
  refine ⟨faithful_whileBodyConverges, ?_⟩
  simp only [ensWrong, denote, intVal, bindResult, baseEnv, Env.bindInt]
  decide

/-- The composition-mis-map oracle: the skip variant certifies the wrong contract at the
    entry value, while the faithful composition refutes it at the genuine exit value. The
    faithful `whileBodyDenote` (prefix → the shipped `loopDenote` → tail) is the one that
    must reach the certificate, and a loop-skipping or mis-ordered composition is caught. -/
theorem composition_mismap_caught :
    (skipBodyDenote noPrefix loTail l1State = some (.int ⟨.usize, 0⟩)
      ∧ denote 0 ensWrong (bindResult baseEnv (.int ⟨.usize, 0⟩)))
    ∧ (whileBodyConverges noPrefix l1Cond l1Body loTail l1State (.int ⟨.usize, 3⟩)
      ∧ ¬ denote 0 ensWrong (bindResult baseEnv (.int ⟨.usize, 3⟩))) :=
  ⟨skip_composition_certifies_wrong_ens, faithful_composition_refutes_wrong_ens⟩

end Thermite.PinWhileComposition
