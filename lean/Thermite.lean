/-
  Library root for Thermite's Lean models and proof spine.

  The core modules define the contract and executable semantics, their reference
  encoders, and the soundness theorems used by translation validation. Later
  imports add loop composition, solver replay, and the real-relaxation bridge.
-/
import Thermite.Ast
import Thermite.Denote
import Thermite.RefEncode
import Thermite.Soundness

-- Executable expressions, statements, and partial-correctness loop rules.
import Thermite.Exec
import Thermite.Exec.Stmt
import Thermite.Exec.KernelAggregate
import Thermite.Exec.Loop
import Thermite.Exec.WhileBody

-- The composed translation-validation theorem.
import Thermite.Faithfulness

-- Solver-replay examples, generated exporter fixtures, and BitVec models.
import Thermite.SmtDemo
import Thermite.SmtExport
import Thermite.BvModel
import Thermite.PinReconstruction

-- Stabilization and the integer-to-real relaxation theorem.
import Thermite.Stabilize
import Thermite.Relax

-- Finite EPR reconstruction for admitted S₂.0 relation and sequence clauses.
import Thermite.Strat.Model
import Thermite.Strat.Normalize
import Thermite.Strat.Substitution
import Thermite.Strat.Skolem
import Thermite.Strat.Grounding
import Thermite.Strat.Instantiation
import Thermite.Strat.GroundReconstruct
import Thermite.Strat.GroundTheory
import Thermite.Strat.StructuralInstantiation
import Thermite.Strat.EprReplay
import Thermite.PinSubstitutionCapture
import Thermite.PinSkolemDependencies
import Thermite.PinGroundingCompleteness
import Thermite.PinInstantiationOmission
import Thermite.PinStructuralSkolemScopes
import Thermite.PinEprReplay
