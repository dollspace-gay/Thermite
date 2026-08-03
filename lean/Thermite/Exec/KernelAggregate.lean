/-
  Exact fixed-storage aggregate updates and framed executable calls used by the
  Thermite kernel body validator. This companion layer extends the scalar/slice
  `S_B` spine without weakening `ExecVal` or `body_ref_sound`.
-/

import Thermite.Exec.Stmt

namespace Thermite.Exec

/-! ## FixedArray8 exact slot semantics -/

/-- The exact generated storage representation: eight named slots, matching the
    `TFixedArray8*` Verus wrapper emitted by `thermite-lower`. -/
structure FixedArray8Val where
  slot0 : ExecVal
  slot1 : ExecVal
  slot2 : ExecVal
  slot3 : ExecVal
  slot4 : ExecVal
  slot5 : ExecVal
  slot6 : ExecVal
  slot7 : ExecVal
  deriving DecidableEq, Repr

/-- Source meaning of `array.set(index, value)`: update exactly the selected
    slot, or fail the bounds obligation. This definition uses the source
    operation's index dispatch. -/
def fixedArraySetDenote (array : FixedArray8Val) (index : Nat) (value : ExecVal) :
    Option FixedArray8Val :=
  match index with
  | 0 => some { array with slot0 := value }
  | 1 => some { array with slot1 := value }
  | 2 => some { array with slot2 := value }
  | 3 => some { array with slot3 := value }
  | 4 => some { array with slot4 := value }
  | 5 => some { array with slot5 := value }
  | 6 => some { array with slot6 := value }
  | 7 => some { array with slot7 := value }
  | _ => none

/-- Meaning of the independently rendered body reference. The Rust encoder
    reconstructs all eight named slots, selecting the update with an `if` at
    each slot and retaining every collateral slot from the base aggregate. -/
def fixedArraySetRef (array : FixedArray8Val) (index : Nat) (value : ExecVal) :
    Option FixedArray8Val :=
  if index < 8 then
    some {
      slot0 := if index = 0 then value else array.slot0
      slot1 := if index = 1 then value else array.slot1
      slot2 := if index = 2 then value else array.slot2
      slot3 := if index = 3 then value else array.slot3
      slot4 := if index = 4 then value else array.slot4
      slot5 := if index = 5 then value else array.slot5
      slot6 := if index = 6 then value else array.slot6
      slot7 := if index = 7 then value else array.slot7
    }
  else none

/-- The eight-slot reference reconstruction is exactly the source `.set` state
    transition, including its bounds failure. -/
theorem fixed_array_set_ref_sound (array : FixedArray8Val) (index : Nat) (value : ExecVal) :
    fixedArraySetRef array index value = fixedArraySetDenote array index value := by
  cases index with
  | zero => simp [fixedArraySetRef, fixedArraySetDenote]
  | succ index =>
    cases index with
    | zero => simp [fixedArraySetRef, fixedArraySetDenote]
    | succ index =>
      cases index with
      | zero => simp [fixedArraySetRef, fixedArraySetDenote]
      | succ index =>
        cases index with
        | zero => simp [fixedArraySetRef, fixedArraySetDenote]
        | succ index =>
          cases index with
          | zero => simp [fixedArraySetRef, fixedArraySetDenote]
          | succ index =>
            cases index with
            | zero => simp [fixedArraySetRef, fixedArraySetDenote]
            | succ index =>
              cases index with
              | zero => simp [fixedArraySetRef, fixedArraySetDenote]
              | succ index =>
                cases index with
                | zero => simp [fixedArraySetRef, fixedArraySetDenote]
                | succ index => simp [fixedArraySetRef, fixedArraySetDenote]

/-- Source meaning of the generated `.get` operation. -/
def fixedArrayGetDenote (array : FixedArray8Val) (index : Nat) : Option ExecVal :=
  match index with
  | 0 => some array.slot0
  | 1 => some array.slot1
  | 2 => some array.slot2
  | 3 => some array.slot3
  | 4 => some array.slot4
  | 5 => some array.slot5
  | 6 => some array.slot6
  | 7 => some array.slot7
  | _ => none

/-- Meaning of the reference encoder's `.spec_get(index)` projection. -/
def fixedArrayGetRef (array : FixedArray8Val) (index : Nat) : Option ExecVal :=
  if index = 0 then some array.slot0
  else if index = 1 then some array.slot1
  else if index = 2 then some array.slot2
  else if index = 3 then some array.slot3
  else if index = 4 then some array.slot4
  else if index = 5 then some array.slot5
  else if index = 6 then some array.slot6
  else if index = 7 then some array.slot7
  else none

theorem fixed_array_get_ref_sound (array : FixedArray8Val) (index : Nat) :
    fixedArrayGetRef array index = fixedArrayGetDenote array index := by
  cases index with
  | zero => simp [fixedArrayGetRef, fixedArrayGetDenote]
  | succ index =>
    cases index with
    | zero => simp [fixedArrayGetRef, fixedArrayGetDenote]
    | succ index =>
      cases index with
      | zero => simp [fixedArrayGetRef, fixedArrayGetDenote]
      | succ index =>
        cases index with
        | zero => simp [fixedArrayGetRef, fixedArrayGetDenote]
        | succ index =>
          cases index with
          | zero => simp [fixedArrayGetRef, fixedArrayGetDenote]
          | succ index =>
            cases index with
            | zero => simp [fixedArrayGetRef, fixedArrayGetDenote]
            | succ index =>
              cases index with
              | zero => simp [fixedArrayGetRef, fixedArrayGetDenote]
              | succ index =>
                cases index with
                | zero => simp [fixedArrayGetRef, fixedArrayGetDenote]
                | succ index => simp [fixedArrayGetRef, fixedArrayGetDenote]

/-- A representative named kernel aggregate. Other scalar fields are retained
    by structure update while the fixed-storage field is reconstructed exactly. -/
structure KernelAggregateVal where
  slots : FixedArray8Val
  epoch : ExecVal
  deriving DecidableEq, Repr

def aggregateSetDenote (base : KernelAggregateVal) (index : Nat) (value : ExecVal) :
    Option KernelAggregateVal := do
  let slots ← fixedArraySetDenote base.slots index value
  some { base with slots }

def aggregateSetRef (base : KernelAggregateVal) (index : Nat) (value : ExecVal) :
    Option KernelAggregateVal := do
  let slots ← fixedArraySetRef base.slots index value
  some { base with slots }

/-- Exact aggregate reconstruction composes the slot theorem and preserves the
    other named fields. -/
theorem aggregate_set_ref_sound (base : KernelAggregateVal) (index : Nat) (value : ExecVal) :
    aggregateSetRef base index value = aggregateSetDenote base index value := by
  simp [aggregateSetRef, aggregateSetDenote, fixed_array_set_ref_sound]

def fixedArrayWitness : FixedArray8Val :=
  { slot0 := .int ⟨.u8, 0⟩, slot1 := .int ⟨.u8, 1⟩
    slot2 := .int ⟨.u8, 2⟩, slot3 := .int ⟨.u8, 3⟩
    slot4 := .int ⟨.u8, 4⟩, slot5 := .int ⟨.u8, 5⟩
    slot6 := .int ⟨.u8, 6⟩, slot7 := .int ⟨.u8, 7⟩ }

/-- Wrong-index tooth: changing the generated index changes the state. -/
theorem wrong_fixed_array_index_breaks_refinement :
    fixedArraySetDenote fixedArrayWitness 0 (.int ⟨.u8, 99⟩) ≠
      fixedArraySetDenote fixedArrayWitness 1 (.int ⟨.u8, 99⟩) := by
  decide

/-- Wrong-value tooth: changing the generated value changes the state. -/
theorem wrong_fixed_array_value_breaks_refinement :
    fixedArraySetDenote fixedArrayWitness 3 (.int ⟨.u8, 99⟩) ≠
      fixedArraySetDenote fixedArrayWitness 3 (.int ⟨.u8, 42⟩) := by
  decide

/-! ## Exact executable call declarations and prior-local preconditions -/

/-- The semantic content required for a reachable executable call. `arity` is
    the exact framed signature, `pre` is the callee precondition whose proof is
    supplied by the caller's prior-local assumptions, and `run` is its result
    meaning. -/
structure ExactCallDecl where
  arity : Nat
  pre : List ExecVal → Bool
  run : List ExecVal → Option ExecVal

abbrev ExactCallFrame := String → Option ExactCallDecl

/-- Source call meaning. An absent declaration, wrong arity, or unsatisfied
    callee precondition has no executable value. -/
def exactCallDenote (frame : ExactCallFrame) (name : String) (args : List ExecVal) :
    Option ExecVal := do
  let decl ← frame name
  if args.length = decl.arity ∧ decl.pre args then decl.run args else none

/-- The token produced by the exact call encoder only after declaration and
    arity checks. -/
structure ExactCallToken where
  name : String
  args : List ExecVal

def encodeExactCall (frame : ExactCallFrame) (name : String) (args : List ExecVal) :
    Option ExactCallToken := do
  let decl ← frame name
  if args.length = decl.arity then some ⟨name, args⟩ else none

/-- Meaning of the emitted exact-call token. The callee declaration is looked
    up again and its precondition remains a real proof obligation. -/
def denoteExactCallToken (frame : ExactCallFrame) (token : ExactCallToken) : Option ExecVal := do
  let decl ← frame token.name
  if token.args.length = decl.arity ∧ decl.pre token.args then decl.run token.args else none

def exactCallRef (frame : ExactCallFrame) (name : String) (args : List ExecVal) :
    Option ExecVal := do
  let token ← encodeExactCall frame name args
  denoteExactCallToken frame token

/-- Exact call encoding preserves the source call meaning. -/
theorem exact_call_ref_sound (frame : ExactCallFrame) (name : String) (args : List ExecVal) :
    exactCallRef frame name args = exactCallDenote frame name args := by
  cases hdecl : frame name with
  | none => simp [exactCallRef, encodeExactCall, exactCallDenote, hdecl]
  | some decl =>
    by_cases harity : args.length = decl.arity
    · simp [exactCallRef, encodeExactCall, denoteExactCallToken, exactCallDenote,
          hdecl, harity]
    · simp [exactCallRef, encodeExactCall, exactCallDenote, hdecl, harity]

/-- An unframed user call is rejected before a token can be emitted. -/
theorem unframed_call_is_rejected (frame : ExactCallFrame) (name : String)
    (args : List ExecVal) (h : frame name = none) :
    encodeExactCall frame name args = none := by
  simp [encodeExactCall, h]

def advanceDecl : ExactCallDecl :=
  { arity := 1
    pre := fun args => args = [.int ⟨.u64, 0⟩]
    run := fun args => if args = [.int ⟨.u64, 0⟩] then some (.int ⟨.u64, 1⟩) else none }

def advanceFrame : ExactCallFrame := fun name =>
  if name = "advance" then some advanceDecl else none

/-- The caller's exact prior-local fact discharges the callee precondition and
    produces the declared result. -/
theorem prior_local_assumption_enables_exact_call
    (args : List ExecVal) (hlocal : args = [.int ⟨.u64, 0⟩]) :
    exactCallRef advanceFrame "advance" args = some (.int ⟨.u64, 1⟩) := by
  subst args
  decide

/-- Missing the prior-local fact is observable: the same framed call at a state
    outside the callee precondition has no value. A validator cannot silently
    omit the lexical-state premise and still claim a total faithful call. -/
theorem missing_prior_local_assumption_blocks_call :
    exactCallRef advanceFrame "advance" [.int ⟨.u64, 2⟩] = none := by
  decide

/-- Permissively inventing a value for an unframed call disagrees with the exact
    semantics, pinning the fail-closed declaration requirement. -/
theorem permissive_unframed_call_breaks_refinement :
    some (.int ⟨.u64, 0⟩) ≠
      exactCallDenote (fun _ => none) "missing" [] := by
  decide

end Thermite.Exec
