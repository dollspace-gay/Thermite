/-
  Thermite/Strat/Faithfulness.lean — T2-S, the stratified lowering faithfulness, and
  the ATOM-GROUNDING that turns REQ-5's structural soundness into a guarantee over the
  real source meaning. This is the SOUNDNESS-CRUX increment of stage 2 (REQ-8).

  Governing design: `.design/stage2-stratified-cage.md` REQ-8 / AC-8 (child of
  `.design/thermite2-program.md`; spec of record: the stage-2 metatheory sketch, GH
  issue #2; gate G2). Inherits the obligation REQ-5 (#327, option B) deferred here.

  ────────────────────────────────────────────────────────────────────────────────
  THE INHERITED OBLIGATION (REQ-5 option B).  REQ-5's T1-S (`strat_ref_sound`,
  `Strat/Soundness.lean`) proved only the STRUCTURAL/skeleton layer: the encoder
  `sencode` transcribes the quantifier+boolean skeleton of a classifier formula
  (`Cls.Frm`) to the trigger-free MBQI token surface (`Tok`) faithfully, PARAMETRIC
  in an uninterpreted atom oracle `q : Atom → Bool` and ∀-quantified over every
  oracle and domain.  Atoms were never interpreted there.  REQ-8 OWNS closing the
  atom-grounding: instantiate `q` to the REAL v1 / program atom semantics and show
  the encoded SMT surface denotes the real source meaning, not merely a structural
  image.  Only when this loop closes is cage L4 'proven over source meaning' rather
  than structural-only (the G2 gate constraint, REQ-9).

  ────────────────────────────────────────────────────────────────────────────────
  HOW THE LOOP CLOSES — AND THE HONEST BOUNDARY IT RESPECTS.  A `Cls.Atom` is one of
  two leaves (`Strat/Nnf.lean`):

    * `qfree e`  — an embedded v1 quantifier-free `Thermite.Expr`.  This atom HAS a
      real source meaning: the existing v1 `Thermite.denote`.  It is GROUNDED here,
      exactly mirroring the spine seam REQ-1 already proved sound — `canonicalQOracle
      venv e := decide (Thermite.denote 0 e venv)` and `sdenote_qf_canonical`
      (`Strat/Denote.lean`).  This module specializes REQ-5's structural result to the
      same v1-deferral instance, on the classifier atom set.  This is the substantive
      new content over REQ-5: the SMT surface of a `qfree` atom denotes EXACTLY the v1
      `Thermite.denote` truth in the requirement frame (`strat_lowering_faithful`,
      second conjunct).

    * `rel ρ t u` — a relation over the array-property term vocabulary
      (`read`/`len`/`cast`/`idxOp`/`mul`/`app1`; `Strat/Nnf.lean`).  Its meaning is the
      SMT ARRAY/INTEGER THEORY — i.e. the model the solver picks.  This is NOT papered
      over with a fabricated Lean evaluator: a closed `rel` atom CANNOT be evaluated to a
      v1 value, because `Cls.Tm.lit s` carries NO value (`Strat/Nnf.lean`, "a machine
      literal (value irrelevant here)") and the structural domain `dom : List Tm` is
      sort-erased.  So `rel` atoms stay MODEL-RELATIVE: the faithfulness theorem holds
      for EVERY oracle that grounds `qfree` to v1, leaving `rel` free.  This is the
      honest scope — exactly an SMT translation-validation posture (the array theory is
      the solver's), and exactly the parametricity T1-S already certifies.  The
      grounding adds real meaning where real meaning EXISTS in Lean (`qfree → v1`) and
      records, rather than fakes, where it lives in the solver (`rel → theory`).  This
      boundary was surfaced on #330 as a decision, not hidden.

  WHY THIS IS NOT A SILENT WEAKENING.  The faithfulness theorem is INHABITED: for any
  requirement frame `venv`, any solver relation model `relModel`, and any non-empty
  carrier `dom`, `SFnTvWitness.canonical` builds a grounded witness (`grounds` holds by
  `rfl`).  So `strat_lowering_faithful` is not vacuously true over an empty hypothesis;
  it is the genuine, instantiable faithfulness statement.

  Axiom discipline (AC-8 / REQ-9 [1′]): `strat_lowering_faithful` is `#print axioms`-d
  in-file and added to `scripts/lean-axiom-probe.sh`'s THEOREMS, and must show a subset
  of `{propext, Classical.choice, Quot.sound}` (Classical via the `decide` over the
  `Prop`-valued v1 `Thermite.denote`), zero `sorry`.

  Core-Lean path: imports REQ-5's `Strat/Soundness.lean` (the Cls encoder + T1-S) and
  the v1 `Thermite.Denote` (the grounding seam), both Mathlib-free.  It deliberately
  does NOT import the spine `Strat/Denote.lean`: that would pull the spine's
  `Thermite.Strat.{Tm,Atom,Frm}` into scope and re-introduce the #68 two-syntax
  axiom-probe collision the `.Cls` split fixed.  The v1 deferral seam is reproduced on
  the Cls atom set directly (faithful to the spine's `canonicalQOracle`, by design).
-/
import Thermite.Strat.Soundness
import Thermite.Denote

namespace Thermite.Strat

open Thermite.Strat.Cls
open Classical

/-! ## The grounding oracle (the v1 deferral on the classifier atom set)

    `groundQ venv relModel` is the concrete atom oracle that turns REQ-5's parametric
    soundness into a statement over real source meaning:

      * a `qfree e` atom defers to the v1 `Thermite.denote` (the requirement frame
        `venv`) — exactly the spine's `canonicalQOracle` seam (`Strat/Denote.lean`),
        reproduced on the `Cls.Atom` set;
      * a `rel …` atom defers to the solver's relation model `relModel` — the SMT
        array/integer theory, which is model-relative by design (see the header).

    `noncomputable` because v1 `denote` is `Prop`-valued (`decide` via
    `Classical.propDecidable`); the encoder + token denotation themselves stay
    computable in their oracle argument. -/
noncomputable def groundQ (venv : Thermite.Env) (relModel : Atom → Bool) : Atom → Bool
  | .qfree e   => decide (Thermite.denote 0 e venv)
  | .rel r t u => relModel (.rel r t u)

/-- A `groundQ` atom oracle reads a `qfree` atom as exactly the v1 `Thermite.denote`
    truth in the requirement frame — the grounding seam, the `Cls` analogue of the
    spine's `canonicalQOracle_iff` / `sdenote_qf_canonical`. -/
theorem groundQ_qfree (venv : Thermite.Env) (relModel : Atom → Bool) (e : Thermite.Expr) :
    groundQ venv relModel (.qfree e) = decide (Thermite.denote 0 e venv) := rfl

/-! ## `SFnTvWitness` — the faithfulness witness with explicit req-frame conditioning

    The witness bundles the REQUIREMENT FRAME the faithfulness is conditioned on:

      * `venv`     — the v1 environment the clause's preconditions establish (the
                     "req-frame": `qfree` atoms read their real meaning here);
      * `q`        — the atom oracle (the model the SMT surface is read in);
      * `dom`      — the finite carrier domain the binders range over;
      * `hdom`     — the carrier is inhabited (the standard non-empty side condition);
      * `grounds`  — the GROUNDING CONDITION: `q` agrees with the v1 deferral on every
                     `qfree` atom.  This is what makes the witness "grounded" rather than
                     a bare REQ-5 instance — it ties `q`'s `qfree` behaviour to real v1
                     source meaning, while leaving `q`'s `rel` behaviour free (the
                     solver's array theory).

    Conditioning on a witness (rather than a bare `q`) is the "explicit req-frame
    conditioning" T2-S adds over T1-S: faithfulness is asserted RELATIVE TO a stated
    requirement frame and grounding, never unconditionally over an arbitrary model. -/
structure SFnTvWitness where
  /-- The requirement frame: the v1 environment the clause is read in. -/
  venv : Thermite.Env
  /-- The atom oracle (the model the encoded SMT surface is interpreted in). -/
  q : Atom → Bool
  /-- The finite carrier domain the stratified binders range over. -/
  dom : List Tm
  /-- The carrier is inhabited (the non-empty side condition, true of every S₂ sort). -/
  hdom : dom ≠ []
  /-- The grounding condition: the oracle reads every `qfree` atom as its real v1
      `Thermite.denote` truth in the requirement frame. -/
  grounds : ∀ e, q (.qfree e) = decide (Thermite.denote 0 e venv)

/-- The canonical grounded witness for a requirement frame `venv`, a solver relation
    model `relModel`, and a non-empty carrier `dom`.  Its existence proves
    `strat_lowering_faithful` is INHABITED (not vacuously true over an empty
    hypothesis): every frame + solver model + non-empty carrier yields a grounded
    witness, with `grounds` discharged by `rfl` (`groundQ_qfree`). -/
noncomputable def SFnTvWitness.canonical (venv : Thermite.Env) (relModel : Atom → Bool)
    (dom : List Tm) (hdom : dom ≠ []) : SFnTvWitness where
  venv := venv
  q := groundQ venv relModel
  dom := dom
  hdom := hdom
  grounds := fun e => groundQ_qfree venv relModel e

/-! ## T2-S — the stratified lowering is faithful over source meaning

    `strat_lowering_faithful` is the conjunction that closes the atom-grounding loop:

      (1) STRUCTURAL FAITHFULNESS (T1-S, specialized to the witness's grounded oracle):
          the encoded MBQI token surface `sencode φ` denotes exactly the source
          classifier formula `φ` under the witness's model — the quantifier+boolean
          skeleton is transcribed faithfully (the production lowering, validated equal
          to `sencode` by the two-phase TV, inherits this).

      (2) ATOM-GROUNDING (the new content over REQ-5): the encoded SMT surface of a
          `qfree` atom denotes EXACTLY the v1 `Thermite.denote` truth in the requirement
          frame `venv` — the encoded atom carries real source meaning, not an
          uninterpreted symbol.

    Together: the SMT surface denotes the source skeleton with its `qfree` atoms grounded
    to real v1 truth — faithfulness "over source meaning", in the requirement frame.  The
    `rel` atoms are interpreted by the witness's `q` (the solver's array theory), which is
    model-relative by design (header).  `φ` is a well-scoped SENTENCE (`wfFrm 0`), the
    shape of a top-level admitted clause. -/
theorem strat_lowering_faithful (W : SFnTvWitness) (φ : Frm) (ρ σ : Subst)
    (hwf : wfFrm 0 φ = true) :
    tokDenote W.q W.dom (sencode φ) σ = fdenote W.q W.dom φ ρ
    ∧ (∀ e, tokDenote W.q W.dom (sencode (.atom (.qfree e))) σ
              = decide (Thermite.denote 0 e W.venv)) := by
  refine ⟨strat_ref_sound_sentence W.q W.dom φ hwf ρ σ, ?_⟩
  intro e
  -- `sencode (atom (qfree e)) = atom (qfree e)`; `tokDenote` reads it via `W.q`
  -- on the (passed-through) `qfree` atom; the witness grounds that to v1 `denote`.
  simp only [sencode, sencodeAt, encAtom, tokDenote, substAtom]
  exact W.grounds e

/-- The grounding corollary in `↔` form, the `Cls` analogue of the spine's
    `sdenote_qf_canonical`: under a grounded witness the encoded SMT surface of a
    `qfree` atom is `true` iff the embedded expression holds in v1. -/
theorem strat_lowering_faithful_qfree_iff (W : SFnTvWitness) (e : Thermite.Expr) (σ : Subst) :
    tokDenote W.q W.dom (sencode (.atom (.qfree e))) σ = true
      ↔ Thermite.denote 0 e W.venv := by
  simp only [sencode, sencodeAt, encAtom, tokDenote, substAtom, W.grounds e,
    decide_eq_true_eq]

/-! ## In-file axiom probe (AC-8)

    Must show a subset of `{propext, Classical.choice, Quot.sound}` — zero `sorry`.
    `strat_lowering_faithful` is additionally gated in CI via the THEOREMS list of
    `scripts/lean-axiom-probe.sh` (the REQ-9 [1′] extension, brought forward for AC-8,
    as REQ-5 did for `strat_ref_sound`). -/
#print axioms strat_lowering_faithful
#print axioms strat_lowering_faithful_qfree_iff

end Thermite.Strat
