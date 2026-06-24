/-
  Thermite.lean — the library root for the Lean 4 side of the Thermite toolchain
  (the verified-validator metatheory; `.design/verified/thermite-semantics.md`
  REQ-6, increment (a), #170; epic #169).

  This increment proves (T1) soundness of the contract-TV reference encoder on the
  comparison + logical fragment (#170) extended through arithmetic + coercions (#176/
  #177), the spec-context rewrites (#178), the 6 bounded-quantifier combinators (#179),
  the C7 match-in-ens / `is` forms (#180), and the named spec-fn calls including
  well-founded recursion (#181), the kernel-checked opening move of the universal lowering
  semantic-preservation proof. The remaining deferred constructs (the 2 recursive
  combinators `count_where`/`permutation_of` #182, general user-ADT match/is) are the
  future sub-increments, listed in `Ast.lean` (not embedded-then-`sorry`).

  Layer 2 (the exec side) is open: increment 2a (#171) mechanizes the
  exec-expression bounded-value denotation `S_E` (`Thermite.Exec`) and proves (T1)
  `∀ pure exec Expr P, ⟦exec_ref_value(P)⟧ = ⟦P⟧_{S_E}` (`Thermite.Exec.exec_ref_sound`).
  `S_E` is a different semantics from `S_C`: a bounded `u64`/`u32`/`usize`/`bool` value
  (never nat-coerced), with arithmetic overflow carried as a proof obligation (the value
  is the mathematical result given no overflow; an overflowing op has no value). The
  exec-body is mechanized (2b #172): the big-step state transformer `S_B` over
  straight-line blocks + the (T1) soundness proof for `body_ref_state`
  (`Thermite.Exec.body_ref_sound`). Loops (2c #163) remain kernel-gated.
-/
import Thermite.Ast
import Thermite.Denote
import Thermite.RefEncode
import Thermite.Soundness
-- Layer 2 (the exec side, increment 2a, #171): the exec-expression bounded-value
-- denotation `S_E` + the (T1) soundness proof for `exec_ref_value`. Separate namespace
-- `Thermite.Exec` (bounded, overflow-as-obligation, never nat-coerced; `S_E ≠ S_C`).
import Thermite.Exec
-- Layer 2 (the exec side, increment 2b, #172): the exec-body big-step state
-- transformer `S_B` over straight-line blocks + the (T1) soundness proof for
-- `body_ref_state` (`Thermite.Exec.body_ref_sound`). Builds on 2a's `ExecExpr`/
-- `execDenote`/`ExecVal`/`ExecEnv` for every per-RHS / condition / tail value; adds
-- the state threading / scalar-mutation rebind / branch composition / tail
-- projection. Loops remain out (2c #163, kernel-gated).
import Thermite.Exec.Stmt
-- Layer 2 (the exec side, increment 2c, #163): the v1 `while`-loop extension of `S_B`.
-- The fuel-indexed iteration semantics `loopDenote` (iterating the shipped `blockThread`),
-- the partial-correctness while-rule `while_rule` (premises ⟹ after-loop = inv ∧ ¬cond,
-- by fuel induction), its TV meta-theorem `tv_meta_loop`, the L1 non-vacuity witness +
-- the L2/L3 negative lemmas. A separate `WhileLoop` around the proven `blockThread`
-- (faithful to the Rust `loop_ref_obligations` separate-form treatment; `Exec/Stmt.lean`
-- unchanged). Partial correctness: termination is the per-run Verus `decreases` residual.
import Thermite.Exec.Loop
-- Layer 2 (the exec side, increment (v-a), #264; `.design/verified/proof-backends.md`
-- §4.2.2): the while-body composition layer, the first `S_B`×`S_Loop` composition
-- artifact. `whileBodyDenote` (prefix `blockThread` → the shipped `loopDenote` → tail
-- `execDenote`, `Option`-monad composed) + the ∃-fuel `whileBodyConverges` (result bound
-- through it, the #214 discipline) + `loopDenote_fuel_mono`/`whileBodyConverges_unique`
-- (the `stabilizes_unique` mirror), the loop-exit-to-ens composition `while_compose`
-- (the shipped partial-correctness `while_rule` lifted through the prefix/tail segments),
-- and the termination bridge `loopDenote_exits_of_dec` (dec-validity + progress ⟹ the
-- exit witness, the REQ-1.2 mirror, by strong induction on `(μ st).toNat`). Composes
-- around `Exec/Stmt.lean` + `Exec/Loop.lean` (unchanged). The (v-b) exporter targets it.
import Thermite.Exec.WhileBody
-- Layer 3 (compose), increments (d) #174 + 3b #183: the translation-validation
-- meta-theorem capstone. Composes the three proven (T1) theorems (`ref_sound_eq`,
-- `Exec.exec_ref_sound`, `Exec.body_ref_sound`) with the per-run TV result (the
-- Z3-discharged `h_tv` premise) into the (T2) universal semantic-preservation guarantee
-- `∀ P passing TV, ⟦lower(P)⟧ = ⟦P⟧_S`, the existential → universal conversion, the
-- verified-validator architecture's conclusion. Per-layer `tv_meta_{contract,exec,body}`
-- + the composed whole-program `lowering_faithful`, relative to {Z3, S = intended
-- meaning, the Lean kernel}. `h_tv` is the Z3-trusted premise (not Lean-proven; #184
-- demotes Z3). Loops (#163) + the Rust↔Lean correspondence (#185) are named residuals.
import Thermite.Faithfulness
-- Layer 4 (trust-shrink), increment 4a (#184): the Z3-demotion proof-of-concept. Wires
-- Lean-SMT (cvc5 proof reconstruction) so a per-run TV equivalence obligation
-- (`P_production ⟺ P_reference`, `thermite-tv/src/obligation.rs`) is kernel-checked by the
-- `smt` tactic rather than Z3-trusted, the route to demote the `h_tv` premise of
-- `Thermite.lowering_faithful`. Tier 3 reached: two real TV equivalence obligations
-- (hand-translated, the gap an exporter closes) discharged by `smt` and kernel-checked,
-- `#print axioms` = [propext, Classical.choice, Quot.sound] (standard only; the cvc5 proof
-- is replayed, not oracle-trusted). The walls (toolchain v4.29.0 + full Mathlib +
-- vendored cvc5 1.3.2; the hand-translation residual; the BitVec-reconstruction `sorry`
-- excluding bitwise obligations; Verus/Z3 not emitting reconstructable certificates) are in
-- `.design/verified/z3-demotion.md`.
import Thermite.SmtDemo
-- Layer 4 (trust-shrink), stage-3 increment REQ-7 (#349; `.design/stage3-bv-reconstruction.md`
-- REQ-7 / AC-8): the AUTOMATED Rust→Lean obligation exporter's output. Where `SmtDemo`
-- hand-translated two TV obligations, `forge/src/lean_smt_export.rs` now EMITS the
-- `(P_prod) ⟺ (P_ref)` Lean goals — one QF_LIA scalar clause + two QF_BV `@bv` clauses
-- (the bounded-integer machine-model, since lean-smt's literal BitVec reconstruction
-- bit-blasts through an upstream `sorry`) — each discharged by `smt` and kernel-checked,
-- `#print axioms` ⊆ {propext, Classical.choice, Quot.sound}. The file is the exporter's
-- verbatim output (pinned by `golden_file_matches_exporter`); building it here makes the
-- default `lake build` kernel-check the AC-8 reconstruction (the Smt toolchain already
-- enters the graph via `SmtDemo`, so this adds no dependency).
import Thermite.SmtExport
-- Layer 4 (trust-shrink), stage-3 REQ-7/REQ-8 (#356, "Path B"): the bit-vector ⟷
-- bounded-integer model FAITHFULNESS metatheorem. The exporter renders a `@bvN` clause
-- over the bounded-integer machine-model (not `BitVec N`, whose `smt` reconstruction
-- bit-blasts through an upstream `sorry`). `Thermite.BvModel` proves — KERNEL-CHECKED,
-- core-only, `#print axioms` ⊆ {propext, Classical.choice, Quot.sound} — that the two
-- denotations agree (`frmInt_iff_frmBV`), so the exporter's `by smt`-discharged int-model
-- `↔` certifies the genuine bit-vector clause (`tv_equiv_faithful`). This discharges the
-- REQ-8 `render_bv_prop` faithfulness obligation for the renderable fragment IN OUR OWN
-- SPINE — no dependency on lean-smt's (stalled) literal QF_BV reconstruction.
import Thermite.BvModel
-- The stabilization spine prerequisite (increment (ii), #240, ref #203;
-- `.design/verified/proof-backends.md` §4/§6.1): the `stabilizes`/`stabilizesProp`
-- relations (the §4 stabilized-form keys, not a raw fuel index), `stabilizes_unique`
-- (the #214 result-binding lever), the `specCallFree` predicate + the fuel-irrelevance
-- lemma `intVal_fuel_irrelevant`/`denote_fuel_irrelevant` (the #216 normalization bridge,
-- the fuel-free tier-(a) export keys `stabilizes_iff_intVal_zero` /
-- `stabilizesProp_iff_denote_zero`), and `stabilization_exists` (the design's
-- `stabilization_exists_for_dec_bounded`, shipped in the `RegistryTerminating` hypothesis
-- form; the per-item registry-termination obligation discharges it). The exporter
-- targets the §4 form against these; the four critic pins keep their own local copies.
import Thermite.Stabilize
-- The real-relaxation spine (increment 0, REQ-8a; metatheory §7;
-- `.design/stage1-forge-tier.md` REQ-8 / Q-NLSAT). A single Mathlib-importing ISLAND
-- (`Thermite.Relax`) carrying the two relax-route soundness lemmas — `rencode_sound`
-- (the ℤ→ℝ polynomial encoding is a ring hom) and `r_relax_sound` (the real relaxation
-- discharges the integer clause). Mathlib already enters the build graph via `SmtDemo`,
-- so this adds no dependency and keeps the core denotation path Mathlib-free; the audit
-- axiom probe (`scripts/audit.sh` check [1]) is extended to cover both lemmas.
import Thermite.Relax
