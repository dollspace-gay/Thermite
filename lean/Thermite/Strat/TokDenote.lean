/-
  Thermite/Strat/TokDenote.lean — the trigger-free MBQI SMT token surface (`Tok`)
  and its denotation (`tokDenote`), the target of the reference encoder
  (`Strat/RefEncode.lean`).

  Governing design: `.design/stage2-stratified-cage.md` REQ-5 / AC-5 (child of
  `.design/thermite2-program.md`; spec of record: the stage-2 metatheory sketch,
  GH issue #2; gate G2). Builds on REQ-3's classifier surface
  (`Strat/Nnf.lean`, namespace `Thermite.Strat.Cls`).

  THE TWO-SYNTAX BRIDGE (REQ-5 is the bridge — read the design REQ-3/4/5 notes).
  Stage 2 carries two formula languages:
    (a) `Thermite.Strat.Frm` — REQ-1's minimal de Bruijn SEMANTIC spine (a single
        opaque carrier, unsorted `all`/`ex`, `Tm.var` only, `qf` atoms opaque),
        with `sdenote` + the SubstKit.
    (b) `Thermite.Strat.Cls.Frm` — REQ-3's RICH sort-typed CLASSIFIER surface
        (`Sort₂` + the array-property term vocabulary `Read`/`Len`/`Cast`/`IdxOp`/
        `app1` + sorted binders) the classifier's `admitted`/`classifier_correct`
        operate on.

  REQ-5 reconciles them. The bridging mechanism is DECIDED and DOCUMENTED here
  (the design offered two options; this is the principled choice between them):

    * Option A — a translation `Cls.Frm → Strat.Frm` + a denotation-correspondence
      lemma — is ILL-DEFINED. The spine's `Tm` is `var`-only: it cannot hold the
      `Read`/`Len`/`Cast`/`IdxOp` term vocabulary, and the spine's `Atom.eq`
      equates only `Tm.var`s while `Atom.qf` is carrier-CLOSED (its free names are
      v1 env names, never carrier binders). An admitted formula like `∀i. a[i] ≤
      a[j]` simply has no image in the minimal spine. So there is no total,
      meaning-preserving translation of the admitted fragment into `Strat.Frm`.
      (This is a genuine architectural finding, surfaced on the issue, not papered
      over — there is no `sorry` and no second forked syntax below.)

    * Option B — encode `Cls.Frm` DIRECTLY against a sound denotation — is the
      taken route. The sound denotation is REQ-3's STRUCTURAL `fdenote`
      (`Strat/Nnf.lean`): the binders range over a finite domain of closed terms
      `dom : List Tm` (sorts erased) and atoms are read by an oracle `q : Atom →
      Bool`. The design itself records that `fdenote` is STRONGER than any single
      per-sort model — `nnf_sound`/`prenex_sound` hold for EVERY `(q, dom)`, so a
      property proved against `fdenote` holds under the eventual per-sort `sdenote`
      too (that is one instance). Encoder soundness against `fdenote` is therefore
      exactly the right strength for an SMT surface, which must agree with the
      source in every model the solver may pick.

  THE TOKEN SURFACE. `Tok` is the SMT/MBQI image of an admitted `Cls.Frm`: the
  same propositional + relational body, but with NAMED binders (a `Nat` name per
  quantifier) in place of de Bruijn indices — what an SMT-LIB emitter produces.
  Two encoder choices are made load-bearing (and refuted in their broken neighbours
  by the pins):

    * the FRESH-NAME discipline — `RefEncode.sencode` names each binder by its de
      Bruijn LEVEL, so names strictly increase down every path and never capture
      (`PinStratCapture` exhibits a name-reusing encoder that captures and is
      unsound);
    * the TRIGGER-FREE (MBQI) surface — the quantifier carries a `triggerFree`
      flag (no instantiation-restricting pattern), set by the encoder; a flipped
      quantifier kind is unsound (`PinStratFlip`).

  `tokDenote` interprets the token surface using a NAMED environment update
  (`upd`): entering `all s n _ φ` re-binds the name `n`. Because names are
  load-bearing here, a capturing (name-reusing) encoder is genuinely refutable.
  Function symbols / relations in the body reuse the Cls `Atom` and are read by the
  SAME oracle `q` as `fdenote`, so the soundness content is concentrated in the
  binder bookkeeping — exactly the fresh-name discipline T1-S certifies.

  Core-Lean-only: the only import is `Strat/Nnf.lean` (the Cls surface + `fdenote`
  + `substAtom`/`substTm` + the generic `cons`), all Mathlib-free. No `Fintype`,
  no Mathlib.
-/
import Thermite.Strat.Nnf

namespace Thermite.Strat

open Thermite.Strat.Cls

/-! ## The named-binder token surface

    Structurally the Cls formula language, except `all`/`ex` carry an explicit
    `name : Nat` (the SMT variable name the encoder emits) and a `triggerFree`
    flag (the MBQI marker). The atoms reuse `Cls.Atom`, whose `Tm.var`s — after
    encoding — reference NAMES rather than de Bruijn indices. -/
inductive Tok where
  | atom (a : Atom) : Tok
  | neg  (φ : Tok) : Tok
  | conj (φ ψ : Tok) : Tok
  | disj (φ ψ : Tok) : Tok
  | imp  (φ ψ : Tok) : Tok
  | all  (s : Sort₂) (name : Nat) (triggerFree : Bool) (φ : Tok) : Tok
  | ex   (s : Sort₂) (name : Nat) (triggerFree : Bool) (φ : Tok) : Tok

/-! ## The named environment update

    A `Subst` (`Nat → Tm`, REQ-3's closing substitution) doubles as the NAMED
    environment for `tokDenote`: lookups are by name. Entering a binder named `n`
    re-binds `n` to the chosen domain element; a capturing encoder that reuses a
    name therefore shadows an outer binder — the soundness escape `PinStratCapture`
    exhibits. -/
def upd (σ : Subst) (n : Nat) (v : Tm) : Subst :=
  fun m => if m = n then v else σ m

/-! ## The token denotation

    `Bool`-valued and computable (the pins `decide` through it). Binders fold the
    finite domain `dom` with `List.all`/`List.any`, re-binding the binder's NAME;
    atoms are read by the SAME oracle `q` as `fdenote`, on the named substitution.
    Connectives mirror `fdenote` (including `imp` as `¬φ ∨ ψ`). -/
def tokDenote (q : Atom → Bool) (dom : List Tm) : Tok → Subst → Bool
  | .atom a,      σ => q (substAtom σ a)
  | .neg φ,       σ => !tokDenote q dom φ σ
  | .conj φ ψ,    σ => tokDenote q dom φ σ && tokDenote q dom ψ σ
  | .disj φ ψ,    σ => tokDenote q dom φ σ || tokDenote q dom ψ σ
  | .imp φ ψ,     σ => !tokDenote q dom φ σ || tokDenote q dom ψ σ
  | .all _ n _ φ, σ => dom.all (fun v => tokDenote q dom φ (upd σ n v))
  | .ex _ n _ φ,  σ => dom.any (fun v => tokDenote q dom φ (upd σ n v))

end Thermite.Strat
