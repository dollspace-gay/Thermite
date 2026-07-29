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
