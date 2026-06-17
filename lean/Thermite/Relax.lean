/-
  Thermite/Relax.lean — the real-relaxation spine lemmas (REQ-8a, metatheory §7;
  `.design/stage1-forge-tier.md` REQ-8 / Q-NLSAT; epic G1).

  Normative home: the relaxable-clause fragment, the L4 kernel-grounded rung, and the
  `RealWitness` escalation are stated normatively in `thermite2-semantics.md` §3.2 and
  §1; that doc is the authority a reader consults. This header carries the two spine
  lemmas' statements and the Mathlib-island justification.

  Governing design: `.design/stage1-forge-tier.md` REQ-8 (the nlsat real-relaxation
  route) and its Q-NLSAT decision (a direct Z3 `nlsat`-tactic QF_NRA query). The relax
  route narrows a universally-quantified polynomial clause `∀ n : ℤ, 0 ≤ e(n)` to its
  real relaxation `∀ x : ℝ, 0 ≤ e(x)` and hands THAT to nlsat. Two facts make the route
  sound, and they are the two lemmas this module kernel-checks:

    rencode_sound  — the real ENCODING of an integer polynomial is faithful: evaluating
                     the encoded expression over ℝ at a cast integer point equals casting
                     the integer evaluation. (The encoding `R(·)` from ℤ-arithmetic into
                     ℝ-arithmetic is a ring homomorphism on the relax fragment — variables,
                     integer literals, `+`, `*`.) This is what lets the integrality check
                     read a real countermodel back as an integer countermodel.

    r_relax_sound  — relaxation soundness: if the relaxation `∀ x : ℝ, 0 ≤ e(x)` holds
                     (what nlsat discharges), then the integer clause `∀ a : ℤⁿ, 0 ≤ e(a)`
                     holds, because ℤ embeds in ℝ order-preservingly. This is the direction
                     the discharge relies on; the CONVERSE fails (a clause true over ℤ but
                     false over ℝ), which is exactly the `RealWitness` escalation — nlsat's
                     real countermodel is not integral and no nearby integer falsifies, so
                     the clause routes UP to the forge rather than down to `Counterexample`.

  ─────────────────────────────────────────────────────────────────────────────────────
  Why this is a Mathlib-importing ISLAND.
  ─────────────────────────────────────────────────────────────────────────────────────
  The lemmas are stated over `ℝ` and lean on Mathlib's `Int.cast` ring-hom and ordered-ring
  lemmas. Full Mathlib is already pinned via the `lean-smt` dependency (toolchain v4.29.0)
  and already enters the build graph through `Thermite.SmtDemo`, so this island reuses the
  existing toolchain — no new dependency, no Mathlib on the core denotation path
  (`Ast`/`Denote`/`RefEncode`/`Soundness`/`Exec`), which stays Mathlib-free. The audit
  axiom probe (`scripts/audit.sh` check [1], the §12 [1′] pattern) is extended to both
  lemmas; their footprint must stay ⊆ {propext, Classical.choice, Quot.sound}.
-/
import Mathlib.Data.Real.Basic

namespace Thermite.Relax

universe u

/-- A polynomial expression over `ν` integer variables — the relax fragment's atom shape:
    variables, integer literals, `+`, `*` (the QF_NRA-relaxable polynomial atoms; no
    div/mod/shift/cast, per REQ-8's `relaxable` predicate). Generic over the carrier so
    the SAME syntax denotes over ℤ (the clause's real meaning) and over ℝ (the nlsat
    query) — the two readings the relax route compares. -/
inductive PExpr (ν : Type u) where
  | var : ν → PExpr ν
  | lit : ℤ → PExpr ν
  | add : PExpr ν → PExpr ν → PExpr ν
  | mul : PExpr ν → PExpr ν → PExpr ν

/-- Denote a `PExpr` into any commutative ring `R` under a variable assignment `a`. With
    `R := ℤ` this is the clause's intended (integer) meaning; with `R := ℝ` it is the
    relaxed query nlsat sees. Integer literals embed via the canonical `ℤ → R` cast. -/
def PExpr.eval {ν : Type u} {R : Type*} [CommRing R] (a : ν → R) : PExpr ν → R
  | .var v => a v
  | .lit k => (k : R)
  | .add e₁ e₂ => e₁.eval a + e₂.eval a
  | .mul e₁ e₂ => e₁.eval a * e₂.eval a

/-- **Real-encoding soundness.** Evaluating a polynomial atom over ℝ at a cast integer
    assignment equals casting its integer evaluation: the encoding `ℤ-arithmetic → ℝ`
    commutes with evaluation. This is the homomorphism the integrality check trusts when
    it reads a real countermodel back over ℤ. -/
theorem rencode_sound {ν : Type u} (e : PExpr ν) (a : ν → ℤ) :
    e.eval (fun v => (a v : ℝ)) = ((e.eval a : ℤ) : ℝ) := by
  induction e with
  | var v => rfl
  | lit k => simp [PExpr.eval]
  | add e₁ e₂ ih₁ ih₂ => simp only [PExpr.eval, ih₁, ih₂, Int.cast_add]
  | mul e₁ e₂ ih₁ ih₂ => simp only [PExpr.eval, ih₁, ih₂, Int.cast_mul]

/-- **Relaxation soundness.** If the real relaxation `∀ x : ℝ, 0 ≤ e(x)` holds (what the
    nlsat QF_NRA query discharges), then the integer clause `∀ a : ℤⁿ, 0 ≤ e(a)` holds —
    because ℤ embeds order-preservingly into ℝ. This is the soundness direction the relax
    route's discharge relies on. The converse is false (a clause true over ℤ, false over
    ℝ), which is the `RealWitness` escalation rather than a `Counterexample`. -/
theorem r_relax_sound {ν : Type u} (e : PExpr ν)
    (h : ∀ x : ν → ℝ, 0 ≤ e.eval x) (a : ν → ℤ) : 0 ≤ e.eval a := by
  have hr : (0 : ℝ) ≤ e.eval (fun v => (a v : ℝ)) := h _
  rw [rencode_sound e a] at hr
  exact_mod_cast hr

/-! ## Trust accounting — `#print axioms` on the relax spine lemmas.

  The canonical gate is `scripts/audit.sh` check [1] (and the Lean CI job), which parses
  these axiom lists and PASSES iff each ⊆ {propext, Classical.choice, Quot.sound}. These
  in-module `#print axioms` mirror that for developer visibility (the `SmtDemo` pattern). -/
#print axioms rencode_sound
#print axioms r_relax_sound

end Thermite.Relax
