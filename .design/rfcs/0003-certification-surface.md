---
rfc: 3
title: The certification surface — from a ladder to a coordinate system
status: draft
supersedes: []
introduces: []
discussion: https://github.com/dollspace-gay/Thermite/issues/119
---

# RFC-2: The certification surface — from a ladder to a coordinate system

| | |
|---|---|
| **Status** | Draft for discussion — not routed, not content-pinned |
| **Supersedes** | RFC-1 §2 (the ladder), §9 (certificates), §11 (the trust base) — *presentation only* |
| **Preserves** | RFC-1 §3–§5, §10, §13 unchanged. No mechanism, engine, or admission rule changes. |
| **Baseline** | `dollspace-gay/Thermite @ 84d276e7` (gates G1–G4 shipped) |
| **Companion** | The assurance-order metatheory (§4) — a separate document, per the RFC-1 pattern |

**Contents:** [§0 Problem](#0-the-problem) · [§1 Coordinates](#1-the-four-coordinates) · [§2 Rendering](#2-the-rendering) · [§3 The lattice](#3-the-lattice) · [§4 Aggregation is deferred](#4-aggregation-is-deferred) · [§5 Dual labels](#5-dual-labels-and-the-collapse-policy) · [§6 Removing Lx](#6-removing-lx) · [§7 Increments](#7-increments) · [§8 Unchanged](#8-what-rfc-2-does-not-change) · [§9 Open questions](#9-open-questions)

---

## 0. The problem

RFC-1 graded every obligation on a five-rung scalar ladder and stated the rungs mean **refutation quality** ("rank means refutation quality", §2). That was true when written, and it stopped being true the moment two mechanisms shared a rung.

Today `Level::L3` denotes both:

- a general Verus/Z3 result — proven for all inputs, refutation *incomplete* (Z3 may return `unknown`), trust base ~500 kLOC of solver; and
- a Lean forge result — proven for all inputs, refutation *absent* (a failed proof is a stuck goal; the covenant is empirical), trust base a small kernel plus a per-item axiom gate.

These differ on both axes RFC-1 cares about, and they differ in *opposite directions*. Verus refutes better; Lean is trusted less. A scalar cannot carry that, so the scalar silently stopped meaning what §2 said it meant.

This is not a defect introduced by any one change — it is what a lossy projection does under load, and the collision predates any recent work. RFC-1 even anticipated it: §12 lists configuration **C4 ("grid ladder — refutation × trust as a product surface")** and *demotes it to metadata* on legibility grounds:

> The 2-D decomposition is true and lives in the `trust:` field and `make audit`;
> the ladder keeps the product's legibility.

RFC-2 revisits that call with the metatheory that did not exist then: the S₂.0 classifier, checked reconstruction at QF_LIA/QF_BV/EPR, the covenant, per-run translation validation, and boundary-closure classification. The decomposition is no longer theoretical — **every coordinate is already computed and stored. Only one of them is rendered.**

**RFC-2's claim: the decomposition won. Render it.**

## 1. The four coordinates

Each already exists in the implementation. None is new work to *compute*.

### 1.1 Scope — what the claim quantifies over

| value | meaning | source |
|---|---|---|
| `all` | holds for every input | Verus/Z3, Lean, cage routes |
| `bounded(n)` | holds for every input up to size *n* | Kani/CBMC |
| `per-exec` | checked at the call site, this execution | runtime contracts |
| `none` | nothing is claimed about the body | `#[slag]` |

### 1.2 Refutation — what a *false* clause yields

This is RFC-1 §9's `falsification_channel`. **It was specified and never shipped** — `manifest.rs` carries `trust` but no falsification field.

| value | meaning | witness |
|---|---|---|
| `complete` | mechanically complete in-fragment | real point · bit pattern · finite structure |
| `incomplete` | a countermodel *when the solver finds one*; `unknown` possible | Verus counterexample |
| `empirical` | no mechanical refutation; a seeded generator attacks the claim | covenant `CovenantRefuted` |
| `trace(n)` | a concrete trace within the bound | Kani trace |
| `abort` | detected in production, at the violating call | L1 abort |
| `none` | — | `#[slag]` |

**Refutation is a property of the fragment, not of the proof.** It classifies the *question*, not the *answer*. A successful `complete` certificate and a successful `empirical` certificate establish the same proposition; they differ in what the system can tell you when the claim is false, or when the code changes. Lx obscured this by looking like a quality score on the artifact.

What decidability buys is precisely the model-finding direction:

> **¬φ satisfiable ⟺ the procedure exhibits a model of ¬φ.**

That biconditional is why `complete` means something: a `valid` answer carries the information *no countermodel exists, because the procedure would have found one*. Outside a decidable fragment there is no such guarantee, and three situations collapse into one observable stuck goal — φ is false, φ is true but unproven, φ is true but unprovable here.

### 1.3 Trust — what discharging this clause asks you to believe

Already shipped as `Certificate::trust` (`forge/src/manifest.rs:266`). Trust is a **set**, not a scalar.

| value | meaning |
|---|---|
| `lean-checked` | Lean re-checked the actual `req → clause` theorem; the solver is a proof *producer* and **leaves** the trusted base |
| `lean-lemma` | Lean proved a *bridge lemma* licensing the route (e.g. `r_relax_sound`); the solver **remains** trusted |
| `solver` | Z3/Verus soundness, per query |
| `fiat` | trusted by declaration |

`inspection` is a **modifier, not a value** (resolving OQ-3): the Rust↔Lean renderers stay inspection-tier *even after* reconstruction — `bv_kernel_checked_trust_profile` carries a residual "renderer correspondence remains inspection-tier" item alongside its `lean-checked` claim.

**Trust splits into residual and discharged.** A naive union is wrong: it makes the relax route `{solver, lean-lemma}` a superset of plain cage `{solver}`, and therefore "worse," when the bridge lemma is not a liability but a *discharged obligation*. Ordering by **residual risk under set inclusion** gives the right answer for free:

| position | residual |
|---|---|
| `lean-checked` | `{inspection, rustc}` |
| `solver`, `solver + lean-lemma` | `{solver, inspection, rustc}` |

`{inspection, rustc} ⊂ {solver, inspection, rustc}`, so **trust is a two-element chain and `lean-checked` strictly dominates** — with no metatheory required. And the relax route comes out *equal* to the unreconstructed cage rather than worse, which matches RFC-1 §12's own characterization of ℝ-relaxation as a **coverage** win rather than an assurance-rank win.

Note the deliberate absence of the word *kernel*: it is claimed by the Lean kernel, the seccomp filter, the OS-kernel target, and the "kernel-grounded" descriptor. See the nomenclature issue.

**The distinction this axis exists to preserve.** Reconstruction is `{lean-checked}` — Z3 is gone entirely (`engine.rs:130`: "the SAT solver and LRAT converter are proof producers only: neither remains in the trusted base"). The relax route keeps Z3. Today the audit **cannot tell them apart**: `KERNEL_CHECKED_TRUST_MARKER` is the substring `"kernel-checked"`, and `r_relax_sound`'s description ends in that substring, so the marker fires for both. That is the same scalar-collapse pathology as the L3 blur, one layer down. (Filed separately per §7.)

### 1.4 Boundary — how far the claim closes

Already shipped (`f78dd664`, 2026-06-05, "end-to-end vs to-the-boundary classification"), typed as `AssuranceScope`.

| value | meaning |
|---|---|
| `e2e` | the claim closes over the item's whole call graph |
| `to_boundary` | closes to declared `#[boundary]` contracts, which are assumed |
| `to_platform(p)` | closes to a named frozen platform registry `p` |

Boundary is **orthogonal to the ladder and always has been**: a clause can be fully proven *and* to-the-boundary — its own contract verified, the whole-program guarantee still resting on a foreign body. Level and boundary answer different questions: *how well is this proven* versus *how far does the proof reach*.

## 2. The rendering

```
scope/refutation/trust@boundary
```

| rendered | what it is |
|---|---|
| `all/complete/lean-checked@e2e` | caged and reconstructed — the strongest position available |
| `all/complete/{solver,lean-lemma}@e2e` | the nlsat relax route |
| `all/complete/solver@e2e` | caged, reconstruction not yet available for the fragment |
| `all/incomplete/solver@e2e` | general Verus/Z3 |
| `all/empirical/lean-checked@e2e` | the Lean forge |
| `all/incomplete/solver@to_platform(x86_64-pc-uefi-smp-v1)` | a kernel core, stated honestly |
| `bounded(8)/trace(8)/solver@e2e` | Kani |
| `per-exec/abort/fiat@e2e` | runtime contracts |
| `none/none/fiat@e2e` | `#[slag]` |

The two rows that render identically as `L3` today — general Verus/Z3 and the Lean forge — are now visibly different objects.

## 3. The lattice

### 3.1 Coherent cells

The coordinate product is mostly empty, and the emptiness is structural. **Refutation is functionally determined by scope everywhere except `all`:**

| scope | admissible refutation | why |
|---|---|---|
| `none` | `none` | nothing is claimed, so nothing to refute |
| `per-exec` | `abort` | a runtime check can only fail at the violating call |
| `bounded(n)` | `trace(n)` | which *is* "complete, relative to the bound" |
| `all` | `complete` · `incomplete` · `empirical` | the only free choice |

Further constraints: `fiat` ⟹ `scope ∈ {none, per-exec}` (claiming all inputs by declaration just *is* `#[slag]`); `incomplete` ⟹ `solver ∈ residual` (if Lean re-checked it, the route was decidable, hence `complete`); `empirical` ⟹ `lean-checked` (empirical refutation is the covenant, a forge construct).

**Incoherent a priori** — a certificate landing in one of these is a schema violation and a real bug detector:

```
all/*/fiat          none/{≠none}/*        bounded(n)/complete/*
per-exec/{complete,incomplete,empirical}/*
all/incomplete/lean-checked                all/empirical/solver
```

**Eight coherent cells**, five of them in `all`:

```
none/none/fiat                        (slag)
per-exec/abort/fiat                   (runtime)
bounded(n)/trace(n)/solver            (Kani)
all/complete/lean-checked             (reconstructed cage)
all/complete/{solver, lean-lemma}     (relax route)
all/complete/solver                   (cage, unreconstructed)
all/incomplete/solver                 (general Verus)
all/empirical/lean-checked            (the forge)
```

### 3.2 Seven order-elements

The relax route and the unreconstructed cage have identical residual, refutation, and scope — they are **order-equivalent**, different routes to one position. So eight cells collapse to **seven distinct elements**.

```
                 A                 all / complete   / lean-checked
                / \
               /   \
              B     \              all / complete   / solver   [≡ relax route]
              |      \
              C       D            all / incomplete / solver
               \     /             all / empirical  / lean-checked
                \   /
                  K                bounded(n) / trace(n) / solver
                  |
                  R                per-exec / abort / fiat
                  |
                  S                none / none / fiat
```

**This is a bounded lattice.** Top `A`, bottom `S`, and both non-trivial pairs have meets and joins:

| pair | join | meet |
|---|---|---|
| `B ∨ D` | `A` | `K` |
| `C ∨ D` | `A` | `K` |

The meets are worth reading: **the greatest common assurance of the forge and any solver route is Kani.** That is the honest answer to "what can I claim about a module mixing forge and cage clauses without appealing to either's distinctive strength."

### 3.3 The obstruction is exactly N₅

The sublattice `{K, C, B, D, A}` is the **pentagon**:

```
        A
       / \
      B   \          K < C < B < A     (chain of length 3)
      |    D         K < D < A         (chain of length 2)
      C   /          D ∥ C,  D ∥ B
       \ /
        K
```

N₅ is the forbidden sublattice for modularity, so **the assurance order is non-modular, hence non-distributive.** That is the formal statement of a fact the implementation kept rediscovering: the axes genuinely interact, and no amount of per-axis reasoning recovers the whole structure.

Two consequences the documents should carry:

**Any total order must invent a comparison.** A linear extension always exists, so RFC-1's scalar ladder was not wrong to be totally ordered — it was wrong to be *silent* about which relation it added. The fiat is `B ∥ D` and `C ∥ D` being forcibly ordered. The L3 blur is `C ∥ D` being forcibly *identified*, which is strictly worse: an invented equality rather than an invented inequality.

**The pentagon names the companion's subject.** `{Kani, general-Verus, cage-solver, forge, reconstructed-cage}`. The companion document's job is to characterize that one obstruction — prove the cell inventory complete, prove N₅ is the only non-modular sublattice, and decide whether the `forge ∥ solver` edge stays incomparable or earns a principled orientation.

### 3.4 Comparison and floors

- Certificates compare by **product order**. Incomparable pairs are reported incomparable, never silently ordered.
- Acceptance gates compare against a **declared set of floor tuples**, accepted on dominance over any member. For a single floor this is identical to per-axis minimums, so the set form costs nothing until needed — and it is already needed: the strict artifact path requires `@e2e` while a platform-image path requires `@to_platform(p)`. Two acceptable positions, not one floor.
- The floor set is **declared in-repo**, versioned under RFC-3's contract, not computed per command. Scattered acceptance constants are how a build path comes to invent its own criterion privately.
- `make audit` reports the **Pareto frontier** plus the weakest link per axis, and the residual-trust statement stays the last thing it prints.

## 4. Aggregation is deferred

Composing clause tuples into item tuples, and item tuples into artifact tuples, is **not** per-axis minimum, and RFC-2 deliberately does not specify it. Three known failures of the naive rule:

1. **Refutation is fibered over scope.** `complete` means complete *relative to the scope claimed*. `bounded(8)/trace(8)` and `all/complete` both say "complete" and mean different strengths; comparing across fibers without normalizing is a category error.
2. **Boundary acts on refutation.** A clause completely refutable `@to_boundary` is completely refutable *modulo the assumption* — a counterexample to the whole-program property can live inside the foreign body, where no channel observes it. Boundary acts *on* the other axes rather than beside them.
3. **Trust is not flat** (§1.3): residual composes, discharged does not.

This follows RFC-1's own precedent. §3 shipped the admission test while deferring its characterization:

> the precise admitted arithmetic–quantifier mix is the stage-2 metatheory
> deliverable (§12)

Same move: ship the surface, name the deferred characterization, make it a numbered deliverable rather than an unstated gap. **Until the companion lands, `forge` reports per-clause tuples and the weakest link on each axis separately, and computes no single composite claim.** The existing aggregation sites keep computing exactly what they compute today.

The companion must also prove its operator *agrees with what `forge check` computes* — the same discipline as `classifier_correct`. Metatheory that merely coexists with the implementation is how drift starts.

## 5. Dual labels and the collapse policy

Every claim is validity **relative to a frame**, and the fragment plus decision procedure are what fix the frame. Different audiences need different amounts of that frame.

So a certificate carries **two labels, both stored, neither derived at read time**:

- **engineer label** — what changes a decision: *"proven for all inputs; a false clause gives you a concrete failing input."*
- **formal label** — the tuple plus frame: fragment, procedure, axioms, residual.

Presentation is progressive disclosure: the engineer label on the front page, the frame behind `--explain`, the lattice position in the audit.

**The rule that makes this safe:**

> Collapsing two formal positions into one engineer label is permitted, and is
> governed by an explicit, versioned **collapse policy**. Collapsing them
> *silently* is a schema violation.

This is the piece that prevents the L3 blur recurring. The blur was never wrong because two mechanisms shared a label — it was wrong because **nothing declared the collapse, on what grounds, for which audience.** A declared collapse is a reviewable design decision. An undeclared one is drift that looks like a decision.

## 6. Removing Lx

**Lx is removed, not retained** — deleted from the certificate schema, audit output, skill, and README during the `2.0.0-beta` line. The tuple becomes the sole certification surface.

A lossy projection kept "for skimming" is a projection someone will eventually gate on, guaranteeing a second source of truth and slow drift back to the collision this RFC exists to fix.

The migration table is a **translation aid for readers of historical certificates**, not a live projection:

| historical Lx | tuple |
|---|---|
| L4 | `all/complete/*` |
| L3 | `all/incomplete/*` **or** `all/empirical/*` — *ambiguous; the collision class* |
| L2 | `bounded(n)/*` |
| L1 | `per-exec/*` |
| L0 | `none/*` |

The L3 row is the point: historical L3 certificates **cannot be mechanically migrated**, because the number never carried enough information to tell the two cases apart. That is the clearest argument for the change, and why removal is cheaper now than later.

**Migration cost, measured.** The checked-in oracle corpus is **12 `.cert.json` files** (10 at L3, 2 at L0), hand-authored and compared as *subsets*. Subset comparison means R2-1…R2-5 do not break them at all — only removing Lx does, and that is 12 files to re-cut by hand. There are no external consumers to migrate: version `0.0.1`, two tags, no releases.

Per RFC-3, removing Lx breaks *both* the certificate schema and the assurance semantics — permitted within the beta line, and the reason `2.0.0-beta` is where it lands.

## 7. Increments

| # | increment | surface | risk |
|---|---|---|---|
| R2-1 | Ship `falsification_channel` per RFC-1 §9 | `manifest.rs`, cert schema v3 | low — additive |
| R2-2 | `trust` as a set; split `lean-checked`/`lean-lemma`; order by residual inclusion | `manifest.rs`, `engine.rs` | low — already the semantics |
| R2-3 | Surface `boundary` in the per-clause record | `manifest.rs` | low — exists at item level |
| R2-4 | Coherence validation: reject certificates in incoherent cells (§3.1) | `manifest.rs` | low — additive, and a bug detector |
| R2-5 | Render the tuple; dual labels + declared collapse policy (§5) | `manifest.rs`, `audit.rs` | medium |
| R2-6 | Product-order comparison, declared floor set, Pareto frontier | `audit.rs`, new policy file | medium |
| R2-7 | Retire the level-as-verdict idiom | 10 call sites | low — arguably a bug fix |
| R2-8 | **Classification certificate** — emit the fragment verdict pre-discharge | `manifest.rs`, `check.rs` | medium |
| R2-9 | **Remove `Lx`** from schema, audit, skill, README | everywhere | **high** — breaking; requires R2-1…R2-8 |

No engine, admission-rule, or solver path is touched by any increment.

### R2-7, sized

`Level` is mentioned 288 times across `forge/src` and `thermite-*/src`, but only **23 are real comparisons**, in five shapes:

| shape | count | what it actually is |
|---|---|---|
| verdict proxy | 10 | `cert.level == Level::L3 && cert.reject.is_none()` — asking *did it prove*, not *where does it sit*. Should read `CertVerdict::Proved`. |
| floor | 4 | `< Level::L3` in the build/image path — the only true floors |
| aggregation | 6 | min-over-clauses; unchanged pending §4 |
| downgrade clamp | 1 | `check.rs:2158` |
| bounded check | 1 | `kani.rs:370` |

**Correction to an earlier assumption:** `G2Checks.g2_flip_permitted` does *not* read a level. It is `declared && checks.all_green()` over four booleans (`axiom_probe`, `doc_drift`, `differential`, `two_phase_tv`). Removing Lx does not touch it.

The verdict-proxy shape is the same scalar-overload pathology one level further down — it is why `L3` kept accreting meanings — and fixing it is arguably a bug fix independent of this RFC.

### R2-8, the classification certificate

The S₂.0 classifier returns `Admitted | Rejected(reason) | Unknown` and drives routing, but **nothing about the fragment is persisted**: `thermite_spec::classifier::classify` is referenced once outside its own crate (`cli.rs:2001`), and `manifest.rs` has no admission field. The frozen `RejectReason` vocabulary dies at the CLI boundary.

Map that onto §3 and the structure is exact:

> **classification determines the refutation coordinate** (which fiber) ·
> **discharge determines the trust coordinate** (position within it)

Two phases, two coordinates — and only the second is recorded. Consequences:

- The best achievable rung for a clause is statically computable and cheap, yet cannot be answered without running the whole pipeline.
- **"Escalate UP, never degrade down" is not independently auditable.** You cannot verify a clause reached the forge because the cage genuinely could not hold it, versus because a cage attempt timed out. RFC-1 §2 requires the classifier "name *why* — a routing reason from a frozen vocabulary, never a bare refusal." That vocabulary exists and is discarded.
- Pre-discharge code has no artifact describing what it logically *is*.

Today's certificate is a **post-mortem**. R2-8 adds the **prognosis**, which is the more fundamental of the two: it determines the coordinate the post-mortem later fills in.

### Out of scope, filed separately

Per the presentation-only boundary: the `KERNEL_CHECKED_TRUST_MARKER` substring conflation; the dual closure classifier (`verified_closure` vs the legacy one); `to_platform_boundary` as a raw string outside `AssuranceScope`. Each is a defect *against* the axes RFC-2 defines, not a change to them.

## 8. What RFC-2 does not change

RFC-1 §3 (cage admission), §4 (numeric routing, `@bv` and the three locks), §5 (the forge, the covenant, the meaning audit), §10 (anti-Goodhart), and §13 (limits) stand unmodified. The seven verdicts stay closed. Covenant-before-burn stays structural. This RFC is about how the system *reports* what it already does.

## 9. Open questions

- **OQ-1** — Does `refutation` need a `reconstructed` value distinct from `complete`? Current view: no — `trust` carries it, and a reconstructed and a solver-trusted cage clause refute identically.
- **OQ-2** — Should the covenant's `falsify` budget appear in the refutation coordinate (`empirical(50000)`)? Argues for; costs schema churn.
- **OQ-3** — ~~Is `inspection` a trust value or a modifier?~~ **Resolved:** a modifier (§1.3).
- **OQ-4** — ~~Does the partial order break `g2_flip_permitted`?~~ **Resolved:** no — it reads four booleans, not a level (§7). The floor question is settled at §3.4: a declared set of floor tuples, dominance over any member.
- **OQ-5** — ~~How do historical L3 certificates migrate?~~ **Resolved:** they do not need to. 12 hand-authored oracles compared as subsets; re-cutting them is a chore, not a design question (§6).
- **OQ-6** — Does the boundary modality preserve the lattice, or does it fail to distribute over the pentagon? Expectation: it does not distribute, which would make it a second obstruction. Companion question.
- **OQ-7** — Who owns the collapse policy (§5), and what is the review bar for changing it? A collapse policy nobody reviews is an undeclared collapse with extra steps.
