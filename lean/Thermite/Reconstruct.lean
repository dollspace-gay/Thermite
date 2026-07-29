/-
  QF_BV reconstruction with an LRAT certificate.

  `bv_reconstruct` uses Lean's bit-blaster, SAT solver, LRAT parser, and LRAT
  soundness theorem. The kernel evaluates the final certificate check, avoiding
  the native-evaluation axiom used by `bv_decide`.
-/
import Lean.Elab.Tactic.BVDecide.Frontend
import Lean.Meta.Tactic.Cbv
import Lean.Meta.Tactic.Grind.Util
import Std.Tactic.BVDecide

open Lean
open Lean.Meta
open Lean.Elab
open Lean.Elab.Tactic
open Std.Sat
open Std.Tactic.BVDecide
open Std.Tactic.BVDecide.Reflect

namespace Thermite.Reconstruct

open Lean.Elab.Tactic.BVDecide
open Lean.Elab.Tactic.BVDecide.Frontend

def verifyActions
    (bv : BVLogicalExpr)
    (certificate : Array LRAT.IntAction) : Bool :=
  LRAT.check certificate (AIG.toCNF bv.bitblast.relabelNat)

theorem unsat_of_verifyActions
    (bv : BVLogicalExpr)
    (certificate : Array LRAT.IntAction)
    (checked : verifyActions bv certificate = true) :
    bv.Unsat := by
  apply BVLogicalExpr.unsat_of_bitblast
  rw [← AIG.Entrypoint.relabelNat_unsat_iff]
  rw [← AIG.toCNF_equisat]
  apply LRAT.check_sound
  exact checked

private def cloneBVExpr : BVExpr w → BVExpr w
  | .var idx => .var idx
  | .const value => .const value
  | .extract start len expr => .extract start len (cloneBVExpr expr)
  | .bin lhs op rhs => .bin (cloneBVExpr lhs) op (cloneBVExpr rhs)
  | .un op operand => .un op (cloneBVExpr operand)
  | .append lhs rhs h => .append (cloneBVExpr lhs) (cloneBVExpr rhs) h
  | .replicate n expr h => .replicate n (cloneBVExpr expr) h
  | .shiftLeft lhs rhs => .shiftLeft (cloneBVExpr lhs) (cloneBVExpr rhs)
  | .shiftRight lhs rhs => .shiftRight (cloneBVExpr lhs) (cloneBVExpr rhs)
  | .arithShiftRight lhs rhs => .arithShiftRight (cloneBVExpr lhs) (cloneBVExpr rhs)

private def cloneBVPred : BVPred → BVPred
  | .bin lhs op rhs => .bin (cloneBVExpr lhs) op (cloneBVExpr rhs)
  | .getLsbD expr idx => .getLsbD (cloneBVExpr expr) idx

private def cloneBVLogicalExpr : BVLogicalExpr → BVLogicalExpr
  | .literal pred => .literal (cloneBVPred pred)
  | .const value => .const value
  | .not expr => .not (cloneBVLogicalExpr expr)
  | .gate op lhs rhs => .gate op (cloneBVLogicalExpr lhs) (cloneBVLogicalExpr rhs)
  | .ite discr thenBranch elseBranch =>
      .ite
        (cloneBVLogicalExpr discr)
        (cloneBVLogicalExpr thenBranch)
        (cloneBVLogicalExpr elseBranch)

private def addAuxDecl (name : Name) (value type : Expr) : CoreM Unit :=
  withOptions (fun options => options.set `compiler.extract_closed false) do
    addAndCompile <| .defnDecl {
      name
      levelParams := []
      type
      value
      hints := .abbrev
      safety := .safe
    }

private def kernelReflectionProof
    (cert : LratCert)
    (ctx : TacticContext)
    (bvExpr : BVLogicalExpr) : MetaM Expr := do
  addAuxDecl ctx.exprDef (toExpr bvExpr) (mkConst ``BVLogicalExpr)
  let actions ← IO.ofExcept <| LRAT.parseLRATProof cert.toUTF8
  let actionType := mkApp (mkConst ``Array [.zero]) (mkConst ``LRAT.IntAction)
  addAuxDecl ctx.certDef (toExpr actions) actionType

  let reflectedExpr := mkConst ctx.exprDef
  let certExpr := mkConst ctx.certDef
  let verification := mkApp2 (mkConst ``verifyActions) reflectedExpr certExpr
  let equality :=
    mkApp3 (mkConst ``Eq [1]) (mkConst ``Bool) verification (mkConst ``Bool.true)
  let equality ← Lean.Meta.Grind.foldProjs (← Lean.Meta.Sym.unfoldReducible equality)
  let proof ← mkFreshExprSyntheticOpaqueMVar equality
  Lean.Meta.Tactic.Cbv.cbvDecideGoal proof.mvarId!
  let proof ← instantiateMVars proof
  unless !proof.hasExprMVar do
    throwError "kernel LRAT verification left an unsolved equality"
  pure <| mkApp3
    (mkConst ``unsat_of_verifyActions)
    reflectedExpr
    certExpr
    proof

private def kernelLratBitblaster
    (goal : MVarId)
    (ctx : TacticContext)
    (reflectionResult : ReflectionResult)
    (atomsAssignment : Std.HashMap Nat (Nat × Expr × Bool)) :
    MetaM (Except CounterExample UnsatProver.Result) := do
  -- Rebuild the tree so runtime pointer sharing cannot change AIG numbering
  -- between certificate generation and kernel replay.
  let bvExpr := cloneBVLogicalExpr reflectionResult.bvExpr
  let entry ← IO.lazyPure (fun _ => bvExpr.bitblast)
  let aigSize := entry.aig.decls.size
  let (cnf, map) ← IO.lazyPure (fun _ =>
    let (entry, map) := entry.relabelNat'
    (AIG.toCNF entry, map))
  let result ← runExternal
    cnf
    ctx.solver
    ctx.lratPath
    ctx.config.trimProofs
    ctx.config.timeout
    ctx.config.binaryProofs
    ctx.config.solverMode
  match result with
  | .ok cert =>
      unless Reflect.verifyCert cnf cert do
        throwError "the generated LRAT certificate failed an eager self-check"
      let proof ← kernelReflectionProof cert ctx bvExpr
      pure <| .ok ⟨proof, cert⟩
  | .error assignment =>
      let equations := reconstructCounterExample map assignment aigSize atomsAssignment
      pure <| .error {
        goal
        unusedHypotheses := reflectionResult.unusedHypotheses
        equations
      }

private def reconstructGoal (goal : MVarId) (ctx : TacticContext) : MetaM Unit := do
  let normalized? ← Normalize.bvNormalize goal ctx.config
  let some goal := normalized? | return
  let prover : UnsatProver := fun goal reflection atoms =>
    kernelLratBitblaster goal ctx reflection atoms
  match ← closeWithBVReflection goal prover with
  | .ok _ => pure ()
  | .error counterexample =>
      counterexample.goal.withContext do
        throwError "the QF_BV validity goal is false; SAT produced a counterexample"

syntax (name := bvReconstruct) "bv_reconstruct" Lean.Parser.Tactic.optConfig : tactic

@[tactic bvReconstruct]
def evalBvReconstruct : Tactic := fun
  | `(tactic| bv_reconstruct $config:optConfig) => do
      let config ← elabBVDecideConfig config
      IO.FS.withTempFile fun _ lratFile => do
        let context ← TacticContext.new lratFile config
        liftMetaFinishingTactic fun goal => reconstructGoal goal context
  | _ => throwUnsupportedSyntax

end Thermite.Reconstruct
