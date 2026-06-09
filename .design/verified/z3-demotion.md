# Z3-demotion (Lean-SMT / cvc5 proof reconstruction) — the investigation + the PoC

<!--
tier: verified
status: investigation + proof-of-concept (increment 4a, crosslink #184; epic #169 Layer 4)
governing: .design/verified/thermite-semantics.md REQ-5 (the COMMITTED Lean-SMT tooling
           decision + its TCB-shrink rationale) and the SOTA finding #8
           (.design/research/formal-methods-sota.md): proof-PRODUCING SMT + reconstruction.
boundary:  the `h_tv` premise of `Thermite.lowering_faithful` (lean/Thermite/Faithfulness.lean)
           — Z3-TRUSTED today; this increment is the route to demote it to KERNEL-CHECKED.
-->

## What this increment is

`Thermite.lowering_faithful` (the T2 capstone, `lean/Thermite/Faithfulness.lean`) is a
KERNEL-CHECKED theorem RELATIVE to a named trust base. The single per-run input it
consumes is `h_tv` — the denotational equality the per-run translation-validation (TV)
check attests. Today `h_tv` is discharged by **Z3** (via Verus): the obligation
`assert((P_production) <==> (P_reference))` that `thermite-tv/src/obligation.rs::equivalence_obligation`
emits is verified by Z3, and by Verus's logic soundness that VERIFIED result MEANS the
`h_tv` equality. So `h_tv` is a **Z3-TRUSTED** premise — Lean does not check Z3's work.

The Z3-demotion goal (finding #8): make the SMT solver **proof-producing**, replay the
proof in the Lean kernel, and thereby demote `h_tv` from *trusted-oracle* to
*kernel-checked*. The live route is **Lean-SMT** (`github.com/ufmg-smite/lean-smt`,
arXiv 2505.15796): its `smt` tactic shells out to **cvc5** and reconstructs the cvc5
proof into native Lean proof terms submitted to the kernel.

This is the **frontier brick**: an honestly-documented wall is an acceptable outcome.
The result below is the FURTHEST tier that genuinely works under our toolchain.

## TIER REACHED — Tier 3 (the prize), with documented residual gaps

**Tier 3 is reached:** TWO REAL per-run TV equivalence obligations — the
`(P_production) ⟺ (P_reference)` shape `equivalence_obligation` asserts — were
hand-translated into Lean and discharged by the `smt` tactic, **kernel-checked**, with the
standard axiom set only. See `lean/Thermite/SmtDemo.lean`:

- `tv_obligation_arith_cmp (a b c : Int) : (a - b ≤ c) ↔ (a ≤ c + b)` — the contract
  clause `(a - b) <= c` lowered two faithful-but-syntactically-different ways (the
  production direct emission vs an algebraically-rearranged reference form). This is a
  genuine `P_prod ⟺ P_ref` over the scalar contract sublanguage.
- `tv_obligation_or_le (a b : Int) : (a = b ∨ a < b) ↔ (a ≤ b)` — a comparison +
  logical-connective clause (`==`/`<` disjunction lowered to a single `<=`), the
  `gen.rs::gen_bool`/`gen_comparison` surface.

Plus two Tier-2 toy witnesses (`toy_lt_iff_not_ge`, `toy_tv_equiv_shape`).

### THE HONESTY CRUX — `#print axioms` (kernel-checked, not oracle-trusted)

```
'Thermite.SmtDemo.toy_lt_iff_not_ge'     depends on axioms: [propext, Classical.choice, Quot.sound]
'Thermite.SmtDemo.toy_tv_equiv_shape'    depends on axioms: [propext, Classical.choice, Quot.sound]
'Thermite.SmtDemo.tv_obligation_arith_cmp' depends on axioms: [propext, Classical.choice, Quot.sound]
'Thermite.SmtDemo.tv_obligation_or_le'   depends on axioms: [propext, Classical.choice, Quot.sound]
```

Every `smt`-discharged theorem — including the two REAL TV obligations — depends on the
**STANDARD Lean axiom set `{propext, Classical.choice, Quot.sound}` ONLY**. There is NO
`sorryAx`, NO cvc5/`Smt` oracle axiom, NO `Lean.ofReduceBool`/`Lean.trustCompiler` (native
decide). For these QF-linear-integer-arithmetic obligations the cvc5 proof is GENUINELY
REPLAYED in the kernel — this is a real (partial-scope) demotion of Z3 to kernel-checked,
NOT laundering (R-DEFER-9). cvc5 IS the solver, but its result is RE-CHECKED, so cvc5 is
not in the trust base for these obligations; only the Lean kernel + the standard axioms are.

## The dependency / toolchain story (Tier 2 — the dep BUILDS, spine stays green)

- **Lean-SMT** has NO release tags; it is pinned by toolchain. `main`
  (`7d1d823`) requires **`leanprover/lean4:v4.29.0`** + full **Mathlib v4.29.0** +
  **lean-auto** (`5c4433f`) + **lean-cvc5** (`abdoo8080/lean-cvc5` @ `4ecae27`).
- **lean-cvc5** downloads a **VENDORED cvc5 1.3.2 static library** (from
  `abdoo8080/cvc5/releases`, Linux-x86_64-static) and builds an FFI binding. It does NOT
  use a system cvc5. (A system `cvc5 1.2.1` IS present at `/usr/sbin/cvc5` here, but
  Lean-SMT uses its own vendored 1.3.2 via the FFI.)
- **Our spine was on `v4.30.0`.** Lean-SMT's nearest supported toolchain is `v4.29.0`.
  Per the manifest's conditional authorization, the `lean/lean-toolchain` was pinned DOWN
  to `v4.29.0` — and verified that **the ENTIRE existing proof spine still builds green on
  v4.29.0** (a clean `lake build` of `Thermite.{Ast,Denote,RefEncode,Soundness,Exec,
  Exec.Stmt,Faithfulness}` — 10 jobs green) BOTH before adding the dependency and after.
  The spine is Lean-core-only (no external imports), so the one-minor-version downgrade is
  inert for it. (Had the spine broken, the toolchain change would have been REVERTED and
  this would stop at Tier 1 — the spine's green is non-negotiable.)
- `lake update` resolved `smt` + mathlib (v4.29.0) + lean-cvc5 + lean-auto + plausible +
  batteries + aesop + proofwidgets + importGraph + LeanSearchClient + Qq into
  `lake-manifest.json`; the Mathlib build-cache (8232 files) downloaded; `lake build Smt`
  built the cvc5 FFI (`libcvc5_cvc5.so`) + the reconstruction library (358 jobs green).
- **The full project builds green** (`lake build`, default target `Thermite`, now incl.
  `Thermite.SmtDemo`).

## The architecture of the demotion (what fragment, is it reconstructable?)

The per-run TV obligation is `lower(P) ⟺ ref(P)` as a Verus/SMT query. For the CAGED
contract sublanguage the relevant SMT fragments are:

| Clause family (`gen.rs`/`obligation.rs`)        | SMT fragment        | Lean-SMT reconstructable? |
|---|---|---|
| scalar comparisons + logical connectives (`==`/`<=`/`&&`/`||`/`!`) | QF_LIA / QF_UF      | **YES** — kernel-clean (the two Tier-3 obligations) |
| integer arithmetic (`+`/`-`/`*`-by-literal, linear) | QF_LIA              | **YES** (linear); nonlinear `*` is solver-incomplete (not a reconstruction wall, a solver one) |
| bounded quantifier combinators (`forall_in`/`sorted`/…) | quantified (UF + LIA + arrays) | PARTIAL — quantifier-instantiation rules have weaker reconstruction coverage (~30% of cvc5 rules overall, finding #8); not exercised here |
| bitwise / shift ops (`&`/`|`/`^`/`<<`)          | QF_BV               | **NO (kernel-clean)** — see the BitVec wall below |
| `spec_sum`/recursive spec fns, `permutation_of` | UF + recursion / multiset | OUT — needs the recursive definition exported as an SMT axiomatization; not a single reconstructable query |

So **the scalar/linear-arithmetic core of the contract obligation is within Lean-SMT's
reconstructable subset TODAY** (demonstrated, kernel-clean). The richer fragments are not.

## THE EXACT WALLS

1. **The BitVec-reconstruction `sorry`.** Building Lean-SMT emits:
   `warning: Smt/Reconstruct/BitVec/Bitblast.lean:36:4: declaration uses 'sorry'`.
   The bit-vector (QF_BV) proof-reconstruction path in Lean-SMT itself contains a `sorry`.
   It is NOT pulled into our integer obligations (their axiom sets are clean — verified by
   `#print axioms`), but it means a **bitwise/shift TV obligation (`&`/`|`/`^`/`<<`, the
   `gen.rs` exec-side surface) would NOT be kernel-clean** — its reconstruction would route
   through the `sorry`. This is a hard wall for the bitwise fragment until upstream closes
   it.
2. **Coverage ~30% of cvc5's proof rules (finding #8).** Quantified obligations (the
   bounded combinators) and theory-lemma-heavy proofs may hit an unreconstructable cvc5
   rule and FAIL (the `smt` tactic errors rather than producing an unsound proof — it does
   not silently trust). The scalar/linear core is comfortably inside the covered subset.
3. **Verus/Z3 do NOT emit reconstructable certificates (finding #8).** This is the
   load-bearing wall for an END-TO-END demotion. Our production TV check runs the obligation
   through **Verus → Z3**, and Z3-via-Verus does not emit a proof certificate that Lean-SMT
   (a cvc5 reconstructor) can replay. The demotion path therefore requires RE-DISCHARGING
   the obligation through **cvc5** (Lean-SMT's solver) instead of trusting the Verus/Z3
   pass — i.e. the obligation is solved a SECOND time by cvc5 and that cvc5 proof is what
   gets kernel-checked. The PoC does exactly this (the `smt` tactic calls cvc5). The Verus/Z3
   pass stays as the production fast path; the Lean-SMT/cvc5 pass is the kernel-checked
   audit of the same `↔`.
4. **The hand-translation gap (Tier-3 residual).** The PoC obligations were HAND-translated
   from the `(P_prod) ⟺ (P_ref)` shape into Lean `Prop`s over `Int`. Production emits both
   predicates as Verus SOURCE STRINGS (`thermite_lower` for `P_prod`,
   `thermite-tv/src/ref_encode.rs` for `P_ref`). An AUTOMATED demotion needs a **Rust→Lean
   exporter** that parses both emitted predicate strings into Lean `Prop`s over the typed
   env the obligation frame declares. That exporter is NOT built in this increment (it is
   the #185-adjacent correspondence-bridge work). The LOGICAL CONTENT discharged is exactly
   the per-run obligation; the residual is the parse/translate step.

## The upstream asks (what would have to change)

- **Lean-SMT:** close the `Smt/Reconstruct/BitVec/Bitblast.lean` `sorry` (kernel-clean
  QF_BV reconstruction) and raise proof-rule coverage above ~30% (esp. quantifier
  instantiation, for the bounded combinators). A release-tagged, toolchain-current
  (`v4.30.0`+) line would also remove our forced downgrade.
- **Verus / Z3:** emit a **reconstructable proof certificate** for a discharged VC (proof
  logging Lean-SMT/SMTCoq can replay). Until then the demotion must RE-SOLVE the obligation
  through cvc5 rather than reuse the Verus/Z3 attestation.
- **Thermite (us):** a Rust→Lean predicate exporter (parse the two emitted Verus predicate
  strings → Lean `Prop`s under the obligation frame's typed env) to remove the
  hand-translation step — future work, NOT this increment.

## Honest assessment — when does FULL demotion become practical?

- **Today (this increment):** the SCALAR / QF-linear-integer core of the contract TV
  obligation can be RE-discharged by cvc5 and kernel-checked with the standard axioms — a
  REAL but PARTIAL-SCOPE demotion (proven, not asserted). The bitwise fragment is blocked
  by an upstream `sorry`; quantified/recursive fragments by coverage; the end-to-end path
  by Verus/Z3 not emitting certificates + the missing exporter.
- **Practical full demotion** needs THREE things to land: (1) Lean-SMT QF_BV `sorry` closed
  + quantifier-rule coverage raised; (2) Verus/Z3 proof-logging OR an accepted policy of
  re-solving every TV obligation through cvc5 (a latency cost — every L3 program's TV runs
  twice); (3) the Rust→Lean exporter. None is fundamental; all are engineering + upstream
  maturation. Realistically this is a multi-cycle, partly-upstream-gated effort — exactly
  why the SOTA flagged it least-confident. The architecture is SOUND and the PoC PROVES the
  kernel-checked path exists for the core fragment; the wall is breadth + the production
  plumbing, not feasibility.

## Trust-base impact (relative to `Faithfulness.lean`'s enumeration)

Until full demotion lands, `Thermite.lowering_faithful`'s `h_tv` REMAINS Z3-trusted and the
trust base still enumerates Z3 + Verus (no overclaim — R-DEFER-9). This increment PROVES the
demotion path is real for the scalar core (kernel-clean `#print axioms`) and pins the exact
walls for the rest. It does NOT change `lowering_faithful`'s status: `h_tv` is not yet
sourced from a kernel-checked proof in production — the PoC is a standalone witness, not a
wired-in replacement.

## Verification

```
cd lean && lake build         # FULL project green (spine + Smt + Thermite.SmtDemo) on v4.29.0
#print axioms (the four smt-discharged theorems) → [propext, Classical.choice, Quot.sound]
cargo build --workspace       # Rust unaffected (the lean/ dir is not a Cargo crate, not routed)
```
