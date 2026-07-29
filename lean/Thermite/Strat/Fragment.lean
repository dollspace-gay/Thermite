/-
  Thermite/Strat/Fragment.lean — the admission classifier kernel: the three side
  conditions (R1) `finCarrier`, (R2) `idxGrammar`, (R3) acyclicity, the executable
  `admitted`, the declarative fragment `Frag`, and the coincidence theorem T3-C
  `classifier_correct : ∀ φ, admitted φ = true ↔ Frag φ`.

  Governing design: `.design/stage2-stratified-cage.md` REQ-3 / AC-3 (spec of record:
  the stage-2 metatheory sketch, GH issue #2, §3 "the admission predicate", §5 "T3-C").

      admitted φ = finCarrier φ && idxGrammar φ && acyclic (sortGraph (nnf φ))

  Fragment version **S₂.0** — the narrow (R2) index grammar of the re-pass (no S₂.1
  widening pressure pre-build); every later widening is a NEW grammar clause + new pins,
  never a silent loosening (metatheory §3.3 "v2.0 grammar conservatism").

  THE FOUR §3.2 WORKED MICRO-EXAMPLES are `decide`-checked at the bottom with their
  expected admit/reject outcomes: `a[a[i]]` (E2 self-loop → reject), the cast cycle
  (E2 `usize → u64 → usize` → reject), the kv alternation cycle (E1 `Key ⇄ Value` →
  reject), and sortedness (E2 `usize → u32` only → admit).

  Core-Lean-only, axiom-clean (`#print axioms classifier_correct` ⊆ {propext,
  Classical.choice, Quot.sound}); the examples are `decide` (kernel), never
  `native_decide` (which would inject `Lean.ofReduceBool`).
-/
import Thermite.Strat.Graph

namespace Thermite.Strat.Cls

/-! ## (R1) finite carriers — binder sorts only

    Machine and opaque sorts denote finite views of machine data; `seq` is never a
    quantifier sort. An unbounded-int binder would be `infinite-carrier` forge-routed —
    but `nat`/`int` are not even `Sort₂`s, so the check is exactly "no `seq` binder". -/

/-- Is a sort an admissible quantifier carrier (finite)? -/
def finSort : Sort₂ → Bool
  | .mach _   => true
  | .opaque _ => true
  | .seq _    => false

/-- Every binder ranges over a finite carrier. -/
def finCarrier : Frm → Bool
  | .atom _   => true
  | .neg φ    => finCarrier φ
  | .conj φ ψ => finCarrier φ && finCarrier ψ
  | .disj φ ψ => finCarrier φ && finCarrier ψ
  | .imp φ ψ  => finCarrier φ && finCarrier ψ
  | .all s φ  => finSort s && finCarrier φ
  | .ex s φ   => finSort s && finCarrier φ

/-! ## (R2) the Bradley–Manna–Sipma index grammar

    A quantified (bound) index variable may appear as a `Read`/`Len` argument, in a
    `Rel`, and under `IdxOp` with a literal offset — but NOT under multiplication
    (`mul`) or a width-changing `Cast`. (The cast restriction is the deliberate
    redundancy with the sort graph, §1.2 / §3.2.) -/

/-- Machine bit-width (used for the width-preserving-cast test). -/
def machWidth : Mach → Nat
  | .u8 => 8 | .u16 => 16 | .u32 => 32 | .u64 => 64 | .usize => 64 | .bool => 1

/-- Do two sorts have the same machine width (a width-preserving cast)? -/
def sameWidth : Sort₂ → Sort₂ → Bool
  | .mach m, .mach m' => decide (machWidth m = machWidth m')
  | _,       _        => false

/-- Does a term mention a bound variable (de Bruijn index `< depth`)? -/
def hasBoundVar (depth : Nat) : Tm → Bool
  | .var _ i      => decide (i < depth)
  | .lit _        => false
  | .read _ sq ix => hasBoundVar depth sq || hasBoundVar depth ix
  | .len sq       => hasBoundVar depth sq
  | .cast _ t     => hasBoundVar depth t
  | .idxOp t _    => hasBoundVar depth t
  | .mul t u      => hasBoundVar depth t || hasBoundVar depth u
  | .app1 _ _ _ t => hasBoundVar depth t

/-- (R2), per term: no bound index var under `mul` or a width-changing `cast`. -/
def idxOkTm (depth : Nat) : Tm → Bool
  | .var _ _      => true
  | .lit _        => true
  | .read _ sq ix => idxOkTm depth sq && idxOkTm depth ix
  | .len sq       => idxOkTm depth sq
  | .cast to t    => (!hasBoundVar depth t || sameWidth t.sortOf to) && idxOkTm depth t
  | .idxOp t _    => idxOkTm depth t
  | .mul t u      => !hasBoundVar depth t && !hasBoundVar depth u && idxOkTm depth t && idxOkTm depth u
  | .app1 _ _ _ t => idxOkTm depth t

/-- (R2), per formula, tracking binder depth. -/
def idxGrammarAt (depth : Nat) : Frm → Bool
  | .atom (.rel _ t u) => idxOkTm depth t && idxOkTm depth u
  | .atom (.qfree _)   => true
  | .neg φ             => idxGrammarAt depth φ
  | .conj φ ψ          => idxGrammarAt depth φ && idxGrammarAt depth ψ
  | .disj φ ψ          => idxGrammarAt depth φ && idxGrammarAt depth ψ
  | .imp φ ψ           => idxGrammarAt depth φ && idxGrammarAt depth ψ
  | .all _ φ           => idxGrammarAt (depth + 1) φ
  | .ex _ φ            => idxGrammarAt (depth + 1) φ

/-- (R2) at top level. -/
def idxGrammar (φ : Frm) : Bool := idxGrammarAt 0 φ

/-! ## The executable classifier and the declarative fragment -/

/-- The admission classifier (metatheory §3.1). Computed on the negation NORMAL FORM so
    every binder's polarity — and hence the E1 edges — is syntactic (`Strat/Nnf.lean`). -/
def admitted (φ : Frm) : Bool :=
  finCarrier φ && idxGrammar φ && acyclic (sortGraph (nnf φ))

/-- The declarative fragment S₂.0 (metatheory §5, "kernel half"): the (R1)/(R2)
    syntactic side conditions, and the genuinely semantic stratification (R3) stated
    declaratively as the ABSENCE OF A SORT-GRAPH CYCLE (not the `acyclic` Bool). -/
def Frag (φ : Frm) : Prop :=
  finCarrier φ = true ∧ idxGrammar φ = true ∧ ¬ HasCycle (sortGraph (nnf φ))

/-- **T3-C — classifier coincidence.** The executable `admitted` decides exactly the
    declarative fragment `Frag`. The substance is the graph-side lemma
    `acyclic_iff_no_cycle` (consuming `sortGraph_complete`'s well-formedness); the
    (R1)/(R2) conjuncts are syntactic and pass through by Boolean reflection. -/
theorem classifier_correct (φ : Frm) : admitted φ = true ↔ Frag φ := by
  simp only [admitted, Frag, Bool.and_eq_true,
    acyclic_iff_no_cycle (sortGraph_complete (nnf φ)), and_assoc]

/-! ## The four §3.2 worked micro-examples (decide-checked)

    Each is a concrete `Frm`; the `decide`-checked theorem pins its expected admit/reject
    outcome. A non-quantified array parameter is modelled as a closed `lit` of its
    sequence sort (it is not a bound variable, so it contributes no edges itself). -/

/-- `a : SeqS usize`. -/
def aSeqUsize : Tm := .lit (.seq usizeS)
/-- `a, b : SeqS u64`. -/
def aSeqU64 : Tm := .lit (.seq (.mach .u64))
def bSeqU64 : Tm := .lit (.seq (.mach .u64))
/-- `a : SeqS u32`. -/
def aSeqU32 : Tm := .lit (.seq (.mach .u32))

/-- **Example 1 — nested reads `a[a[i]]`** (`a : SeqS usize`): the inner `Read` gives the
    E2 self-loop `usize → usize`. `∀ i:usize. a[a[i]] = a[a[i]]`. Expected: REJECT. -/
def ex_selfLoop : Frm :=
  let inner : Tm := .read usizeS aSeqUsize (.var usizeS 0)        -- a[i]
  let outer : Tm := .read usizeS aSeqUsize inner                   -- a[a[i]]
  .all usizeS (.atom (.rel .eq outer outer))

theorem ex_selfLoop_rejected : admitted ex_selfLoop = false := by decide

/-- **Example 2 — nested reads across sorts with a (width-preserving) cast**
    `b[(a[i] as usize)]` (`a, b : SeqS u64`): `Read_a : usize → u64`, `Cast : u64 → usize`
    — the cycle `usize → u64 → usize`.  The cast is width-preserving (64-bit), so (R2)
    PASSES and the rejection is purely the sort-graph cycle.  Expected: REJECT. -/
def ex_castCycle : Frm :=
  let ai : Tm := .read (.mach .u64) aSeqU64 (.var usizeS 0)        -- a[i] : u64
  let cast_ai : Tm := .cast usizeS ai                              -- (a[i] as usize)
  let outer : Tm := .read (.mach .u64) bSeqU64 cast_ai             -- b[…] : u64
  .all usizeS (.atom (.rel .eq outer outer))

theorem ex_castCycle_rejected : admitted ex_castCycle = false := by decide
/-- (R2) itself accepts the width-preserving cast — the rejection is the graph alone. -/
theorem ex_castCycle_idxGrammar_ok : idxGrammar ex_castCycle = true := by decide

/-- **Example 3 — the kv alternation cycle** `(∀k:Key. ∃v:Value. …) ∧ (∀v:Value. ∃k:Key. …)`
    (Key = `opaque 0`, Value = `opaque 1`): E1 gives `Key → Value` and `Value → Key`.
    Expected: REJECT (restratification, §6, repairs it). -/
def keyS : Sort₂ := .opaque 0
def valueS : Sort₂ := .opaque 1
def ex_kvCycle : Frm :=
  let body1 : Frm := .atom (.rel .eq (.var valueS 0) (.var keyS 1))   -- mentions v, k
  let body2 : Frm := .atom (.rel .eq (.var keyS 0) (.var valueS 1))   -- mentions k, v
  .conj (.all keyS (.ex valueS body1)) (.all valueS (.ex keyS body2))

theorem ex_kvCycle_rejected : admitted ex_kvCycle = false := by decide

/-- **Example 4 — sortedness** `∀ i j : usize. i ≤ j ⇒ a[i] ≤ a[j]` (`a : SeqS u32`): E2
    `usize → u32` only, acyclic. Expected: ADMIT (raw, no triggers). -/
def ex_sortedness : Frm :=
  -- de Bruijn under `∀i ∀j`: j = var 0, i = var 1
  let i : Tm := .var usizeS 1
  let j : Tm := .var usizeS 0
  let hyp : Frm := .atom (.rel .le i j)                            -- i ≤ j
  let concl : Frm := .atom (.rel .le (.read (.mach .u32) aSeqU32 i)
                                      (.read (.mach .u32) aSeqU32 j))  -- a[i] ≤ a[j]
  .all usizeS (.all usizeS (.imp hyp concl))

theorem ex_sortedness_admitted : admitted ex_sortedness = true := by decide

end Thermite.Strat.Cls
