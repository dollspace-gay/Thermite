/-
  Thermite/SmtDemo.lean — the Z3-DEMOTION proof-of-concept (increment 4a, #184; epic #169).

  Governing design: `.design/verified/thermite-semantics.md` REQ-5 (the COMMITTED
  Lean-SMT tooling decision + its TCB-shrink rationale: cvc5 proof reconstruction is the
  route to demote Z3 from a TRUSTED component to a KERNEL-CHECKED one) and
  `.design/verified/z3-demotion.md` (THIS increment's investigation + the precise wall).

  ─────────────────────────────────────────────────────────────────────────────────────
  WHAT THIS MODULE DEMONSTRATES — and WHAT IT DOES NOT.
  ─────────────────────────────────────────────────────────────────────────────────────
  The per-run TV obligation (`thermite-tv/src/obligation.rs::equivalence_obligation`) is
  the Verus assertion `assert((P_production) <==> (P_reference))`. By Verus's logic
  soundness, a VERIFIED such obligation MEANS the denotational equality `h_tv` that
  `Thermite.lowering_faithful` (`Faithfulness.lean`) takes as its Z3-DISCHARGED premise.
  Increment 4a's goal is to DEMOTE that premise: reconstruct the cvc5 proof of the SMT
  query into a KERNEL-CHECKED Lean proof, so `h_tv` is no longer Z3-trusted.

  This module ships the FURTHEST tier that genuinely works under our toolchain:
    - TIER 2: a TOY-but-genuine equivalence discharged by the Lean-SMT `smt` tactic and
      KERNEL-CHECKED (the `#print axioms` reported in `z3-demotion.md`).
    - TIER 3: ONE REAL TV equivalence obligation (the `(P_prod) ⟺ (P_ref)` shape that
      `equivalence_obligation` asserts for a concrete contract clause), HAND-TRANSLATED
      into Lean (the hand-translation step IS the gap an automated Rust→Lean exporter
      would close — future work, #185-adjacent, NOT built here) and discharged by `smt`,
      KERNEL-CHECKED.

  HONESTY CRUX (R-DEFER-9 — no laundering). The `#print axioms` at the bottom of this
  module records EXACTLY what each `smt`-discharged theorem's reconstruction depends on.
  If it pulls a trust axiom (e.g. a `Smt`-internal `sorry`/oracle, `Lean.ofReduceBool`,
  or `Lean.trustCompiler`), the demotion is PARTIAL and `z3-demotion.md` says so. The
  goal is the STANDARD Lean axiom set `{propext, Classical.choice, Quot.sound}` — i.e.
  the cvc5 proof is genuinely REPLAYED in the kernel, not asserted.
-/
import Smt

namespace Thermite.SmtDemo

/-! ## TIER 2 — a TOY-but-genuine equivalence discharged by the `smt` tactic.

  These are linear-integer-arithmetic (QF_LIA-shaped) equivalences — exactly the fragment
  the per-run TV obligation lives in for the SCALAR contract sublanguage (comparisons +
  logical connectives over `int`). Each is a goal a HUMAN can see is true, dispatched to
  cvc5 and reconstructed in the kernel by `smt`. -/

/-- A trivial linear-arith equivalence: `a < b ↔ ¬ (b ≤ a)` over the integers. This is
    the shape of a `<`-vs-`≥` comparison-faithfulness obligation (the kind the contract
    encoder produces). Discharged by `smt` (cvc5 reconstruction). -/
theorem toy_lt_iff_not_ge (a b : Int) : a < b ↔ ¬ (b ≤ a) := by
  smt

/-- A linear-arith EQUIVALENCE of two syntactically-different but logically-equal
    predicates — the canonical TV shape `P_production ⟺ P_reference` where the two sides
    are written differently (here `a + 1 ≤ b` vs `a < b`, the off-by-one-safe rewrite a
    lowerer might emit). `smt` proves the `↔` via cvc5 and reconstructs in the kernel. -/
theorem toy_tv_equiv_shape (a b : Int) : (a + 1 ≤ b) ↔ (a < b) := by
  smt

/-! ## TIER 3 — ONE REAL TV equivalence obligation, hand-translated + `smt`-discharged.

  The per-run obligation `equivalence_obligation` emits, for a contract clause, the Verus
  `assert((P_production) <==> (P_reference))`. We take a CONCRETE clause from the contract
  sublanguage and hand-translate BOTH sides into Lean `Prop`s over `Int` variables, then
  discharge the `↔` with `smt`.

  THE CLAUSE. A comparison clause over the scalar fragment — the `gen.rs` generator's
  `gen_comparison`/`gen_int` space — e.g. the contract predicate

      `(a - b) <= c`        (the SOURCE clause, an arithmetic comparison)

  Two FAITHFUL but syntactically-DIFFERENT lowerings (both are predicates a
  correct-but-differently-shaped encoder could emit; the obligation checks they agree):

    - `P_production` :  `a - b <= c`        (the production lowerer's direct emission)
    - `P_reference`  :  `a <= c + b`        (an algebraically-rearranged reference form)

  These are EQUAL as `int` predicates (`a - b ≤ c  ↔  a ≤ c + b`), but NOT syntactically —
  exactly the situation the TV obligation discharges via Z3 today. Here `smt` discharges
  it via cvc5 and KERNEL-CHECKS the result.

  THE HAND-TRANSLATION GAP (future work, explicit). Production emits the predicate as a
  Verus SOURCE STRING; the reference encoder (`thermite-tv/src/ref_encode.rs`) emits
  another. To make this fully automated, a Rust→Lean exporter would parse both emitted
  predicate strings into these Lean `Prop`s (over the same typed env the obligation frame
  declares). That exporter is NOT built in this increment (it is the #185-adjacent
  correspondence-bridge work); the translation below is done BY HAND, and that hand step
  is the documented residual gap an exporter closes. The LOGICAL CONTENT discharged — the
  `↔` between the two predicate denotations — is exactly the per-run TV obligation. -/

/-- **(Tier 3) A REAL per-run TV equivalence obligation, kernel-checked.**

    The contract clause `(a - b) <= c` lowered two faithful-but-syntactically-different
    ways (`a - b <= c` the production form; `a <= c + b` the reference form). The per-run
    TV obligation asserts these are logically equivalent for ALL inputs — `P_prod ⟺ P_ref`.
    `smt` discharges the `↔` through cvc5 and reconstructs the proof in the Lean kernel:
    this is the obligation DEMOTED from Z3-trusted to kernel-checked (relative to the
    hand-translation gap above + whatever `#print axioms` reports below). -/
theorem tv_obligation_arith_cmp (a b c : Int) :
    (a - b ≤ c) ↔ (a ≤ c + b) := by
  smt

/-- **(Tier 3, second witness) A comparison + logical-connective TV obligation.**

    The clause `(a == b) || (a < b)` (the source) vs its faithful rearrangement `a <= b`
    (the reference) — a `==`/`<`-disjunction lowered to a single `<=`. The TV obligation
    `P_prod ⟺ P_ref` is `(a = b ∨ a < b) ↔ a ≤ b`, discharged by `smt` (cvc5) and
    kernel-checked. This exercises the logical-connective + comparison surface the
    contract encoder produces (`gen.rs`'s `gen_bool`/`gen_comparison`). -/
theorem tv_obligation_or_le (a b : Int) :
    (a = b ∨ a < b) ↔ (a ≤ b) := by
  smt

/-! ## THE HONESTY CRUX — `#print axioms` on the `smt`-discharged theorems.

  Uncomment-equivalent: run `#print axioms <thm>` (below) and READ the output. The
  reported axiom set is transcribed VERBATIM into `.design/verified/z3-demotion.md`. If it
  is the standard `{propext, Classical.choice, Quot.sound}`, the cvc5 proof was genuinely
  replayed in the kernel (full demotion of THIS obligation). If it includes a `Smt`/cvc5
  oracle/trust axiom or `Lean.ofReduceBool`, the demotion is PARTIAL and the doc says so. -/
#print axioms toy_lt_iff_not_ge
#print axioms toy_tv_equiv_shape
#print axioms tv_obligation_arith_cmp
#print axioms tv_obligation_or_le

end Thermite.SmtDemo
