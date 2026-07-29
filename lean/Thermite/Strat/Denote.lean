/-
  Thermite/Strat/Denote.lean — the stratified `Bool`-valued denotation, with the
  finiteness-witness upgrade and the v1 deferral at QFree atoms.

  Governing design: `.design/stage2-stratified-cage.md` REQ-1 / AC-1. Three
  obligations:

  1. `sdenote` is `Bool`-valued: the carrier-sort binders fold the carrier
     enumeration with `List.all`/`List.any` (SPIKE-1 §3). The denotation is
     parametric in a QFree ORACLE `q : QOracle` (the QFree atoms' truth), so it
     stays computable — the `Pin`s and the REQ-3 worked examples can `decide`
     through it.

  2. `sdenote_all_iff` / `sdenote_ex_iff` upgrade the finite folds to genuine
     `∀`/`∃`, CONSUMING the carrier's `complete` witness (the (R1) datum). This
     is the only place the finiteness witness is load-bearing in the semantics;
     `PinFiniteEscape` exhibits the escape that dropping it would permit.

  3. "Defer to the v1 denotation at QFree atoms" (design Architecture §"Lean":
     the v1 arithmetic / cast / byte-view layers are CONSUMED, not re-proven).
     `canonicalQOracle` builds the oracle from the existing `Thermite.denote`,
     and `sdenote_qf_canonical` is the bridge: under that oracle a `qf` atom's
     `Bool` value agrees with the v1 `Prop` meaning. This is the concrete seam
     the encoder (REQ-5) and the SubstKit (REQ-2) build on.

  No Mathlib on the denote path: the imports are `Strat/Syntax`, `Strat/Carrier`,
  and the v1 `Thermite.Denote` — all Mathlib-free (the root `Thermite` pulls
  Mathlib only via the `SmtDemo`/`Relax` islands, which this path never touches).
  `canonicalQOracle` is `noncomputable` only because the v1 `Thermite.denote` is
  (it is `Prop`-valued); `sdenote` itself is computable in its oracle argument.
-/
import Thermite.Strat.Syntax
import Thermite.Strat.Carrier
import Thermite.Denote

namespace Thermite.Strat

/-! ## The denotation -/

/-- A QFree oracle: the `Bool` truth of an embedded v1 quantifier-free atom.
    Threaded through `sdenote` as fixed context so the binder algebra (REQ-2)
    is unaffected; the canonical instance defers to v1 (`canonicalQOracle`). -/
abbrev QOracle := Thermite.Expr → Bool

/-- Term denotation: a variable reads the carrier environment. (SPIKE-1.) -/
def tdenote (𝓒 : CarrierAssign) : Tm → Env 𝓒.C → 𝓒.C
  | .var i, ρ => ρ i

/-- Atom denotation into `Bool`. The `eq` atom compares two carrier terms via the
    carried `DecidableEq`; the `qf` atom DEFERS to the oracle `q` (and is
    independent of the carrier environment — it is closed w.r.t. the binders). -/
def adenote (𝓒 : CarrierAssign) (q : QOracle) : Atom → Env 𝓒.C → Bool
  | .eq t u, ρ => 𝓒.beq (tdenote 𝓒 t ρ) (tdenote 𝓒 u ρ)
  | .qf e,  _ => q e

/-- Formula denotation into `Bool`. The binders fold the hand-rolled enumeration
    with `List.all` (`all`) / `List.any` (`ex`) — the only place the finiteness
    witness is consumed by the computation (SPIKE-1 §3). -/
def sdenote (𝓒 : CarrierAssign) (q : QOracle) : Frm → Env 𝓒.C → Bool
  | .atom a,   ρ => adenote 𝓒 q a ρ
  | .neg φ,    ρ => !sdenote 𝓒 q φ ρ
  | .conj φ ψ, ρ => sdenote 𝓒 q φ ρ && sdenote 𝓒 q ψ ρ
  | .disj φ ψ, ρ => sdenote 𝓒 q φ ρ || sdenote 𝓒 q ψ ρ
  | .all φ,    ρ => 𝓒.enum.all (fun x => sdenote 𝓒 q φ (cons x ρ))
  | .ex φ,     ρ => 𝓒.enum.any (fun x => sdenote 𝓒 q φ (cons x ρ))

/-! ## The finiteness-witness upgrade (the (R1) lemmas)

    `sdenote (all φ)` computes `enum.all`; the `complete` witness is what upgrades
    that finite fold to `∀ x : C`. These two lemmas are the ONLY consumers of the
    finiteness datum in the semantics. They would FAIL if the carrier were not
    finite (or if `enum` were incomplete); `PinFiniteEscape` pins exactly that. -/

/-- The `all` fold upgrades to a `∀` through the completeness witness. -/
theorem sdenote_all_iff (𝓒 : CarrierAssign) (q : QOracle) (φ : Frm) (ρ : Env 𝓒.C) :
    sdenote 𝓒 q (Frm.all φ) ρ = true ↔ ∀ x : 𝓒.C, sdenote 𝓒 q φ (cons x ρ) = true := by
  simp only [sdenote, List.all_eq_true]
  constructor
  · intro h x; exact h x (𝓒.complete x)
  · intro h x _; exact h x

/-- The `ex` fold upgrades to a `∃` through the completeness witness. -/
theorem sdenote_ex_iff (𝓒 : CarrierAssign) (q : QOracle) (φ : Frm) (ρ : Env 𝓒.C) :
    sdenote 𝓒 q (Frm.ex φ) ρ = true ↔ ∃ x : 𝓒.C, sdenote 𝓒 q φ (cons x ρ) = true := by
  simp only [sdenote, List.any_eq_true]
  constructor
  · rintro ⟨x, _, hx⟩; exact ⟨x, hx⟩
  · rintro ⟨x, hx⟩; exact ⟨x, 𝓒.complete x, hx⟩

/-! ## The v1 deferral at QFree atoms

    The canonical oracle reads the embedded `Thermite.Expr`'s truth straight off
    the v1 `Thermite.denote` (fuel 0 — quantifier-free atoms never reach the
    fuel-indexed `specCall` recursion). `noncomputable` because v1 `denote` is
    `Prop`-valued; `decide` is via `Classical.propDecidable`. The bridge lemmas
    state that `sdenote`, at a `qf` leaf under this oracle, faithfully reflects
    the v1 meaning — the v1 layer is consumed, not re-proven. -/

open Classical in
/-- The canonical QFree oracle: the v1 `Thermite.denote` truth of the atom, in a
    fixed v1 environment `venv`. -/
noncomputable def canonicalQOracle (venv : Thermite.Env) : QOracle :=
  fun e => decide (Thermite.denote 0 e venv)

theorem canonicalQOracle_iff (venv : Thermite.Env) (e : Thermite.Expr) :
    canonicalQOracle venv e = true ↔ Thermite.denote 0 e venv := by
  simp only [canonicalQOracle, decide_eq_true_eq]

/-- The QFree-deferral bridge: a `qf` atom, denoted under the canonical oracle,
    is `true` iff the embedded expression holds in v1. (`sdenote` consumes
    `Thermite.denote`; the carrier environment is irrelevant at a `qf` leaf.) -/
theorem sdenote_qf_canonical (𝓒 : CarrierAssign) (venv : Thermite.Env)
    (e : Thermite.Expr) (ρ : Env 𝓒.C) :
    sdenote 𝓒 (canonicalQOracle venv) (Frm.atom (Atom.qf e)) ρ = true
      ↔ Thermite.denote 0 e venv := by
  simp only [sdenote, adenote, canonicalQOracle_iff]

end Thermite.Strat
