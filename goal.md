# Thermite — Locked /goal Statement

This file is the binding contract for autonomous work on **Thermite**. When the user issues `/goal $(cat goal.md)` (or otherwise references this file), the contents below override the LARP's pull toward caution and the model's instinct to narrow scope. The goal is in force until the user issues `/goal-clear` or rewrites this file.

**Thermite is NOT a translation fork.** There is no upstream codebase being mirrored. Thermite is a greenfield, **spec-driven** build of a verification-mandatory programming language for AI agents (see `thermite-design.md`). The harness here is the vibe-fork ACToR machinery (proven on ferrotorch/ferrolearn/ferray) re-anchored for greenfield work.

The authority chain is therefore:

```
thermite-design.md  (the product thesis & pillars)
   → .design/<area>/<doc>.md   (the per-component contract: REQs + ACs)
      → impl  (the Rust toolchain: thermite-* crates, forge)
         → verification  (cargo test + conformance corpus + Verus/Kani golden files)
```

The chain runs design → impl → verification, never the reverse. When a component's behavior and its design doc disagree, the design doc is the authority *unless* the design doc itself is wrong about intent — in which case the fix is a design-doc amendment (dispatch acto-doc-author), not a silent code-local decision.

### Why the critic still has teeth without an upstream

In a translation fork the critic anchors divergence claims to a live upstream oracle. Thermite has no upstream, so the anchor is **two external truths the toolchain does not get to author for itself**:

1. **The conformance corpus** — golden `.th` programs under `conformance/` with hand-certified expected results (e.g. Appendix A's `sum` → expected manifest: `L3`, mutants `17/18`, non-vacuous, `pure`). The toolchain's emitted certificate for each corpus program MUST match the golden certificate. This is the cert oracle.
2. **Verus/Kani/Z3 golden files** — under `tests/golden/`, e.g. `<name>.th → <name>.verus.rs` (exact expected lowering) and `<name>.smt2` (expected SMT-LIB). Diffable external references for the lowering and verification components.

A divergence is: emitted certificate ≠ golden certificate; emitted Verus source ≠ golden lowering; or the toolchain violates a design-doc REQ/AC. Expected values come from the corpus/golden artifact or a `thermite-design.md` symbolic constant — **never copied from the toolchain's own output** (R-CHAR-3).

---

## Scope: the v0.1 kernel first, in dependency order

Build leaves first. **Do not leapfrog (R-DEFER-7).** The v0.1 kernel (crosslink milestone #1) is the whole job until it is mechanically complete; v0.2–v0.5 (milestones #2–#5) come after.

Dependency order (work top to bottom):

1. **thermite-syntax** — lexer, recovering parser, AST, stable semantic addressing (`loop#1.inv#2`). The foundation. (issues #1 scaffold, #3 parser)
2. **thermite-spec** — the SpecTherm combinator registry: each combinator with a frozen SMT trigger + a Verus definition (L3) + an executable form (L1). (issue #2)
3. **thermite-lower** — lowering Thermite AST → Verus-annotated Rust source; L1 runtime-check compilation. (issue #4)
4. **forge** — the CLI: `forge new`, `forge check` (run the ladder, structured per-obligation JSON + counterexamples), structural vacuity triage, `#[slag]`, proof cache, pinned seeds. (issues #5, #6, #8)
5. **thermite-skill** — the `THERMITE.skill.md` generator + CI 6,000-token budget gate. (issue #7)

The exact crate/file layout is fixed by scaffold issue #1 and recorded in `tooling/spec-routes.toml`; that route table is the authoritative module map.

EXCLUDED from the kernel (deferred, tracked in issue #21): runtime effect sandbox (compile-time `fx` subsumption only in v0.1), true MIR-level lowering (transpile to Verus instead), Lean-style incremental goal-state holes (`forge check` is whole-item in v0.1).

---

## The verification model

There is no parity-sweep harness. Verification is direct:

**(A) Toolchain crates (`thermite-syntax`, `thermite-spec`, `thermite-lower`, `thermite-skill`).** Verify with `cargo test` against design-doc ACs and unit fixtures. For `thermite-lower`, the critic pins a divergence as a failing test that diffs emitted Verus source against the golden file at `tests/golden/lower/<name>.verus.rs` (R-CHAR-3 — golden is hand-authored from the design, never regenerated from the lowerer).

**(B) forge (the verification driver).** Verify with the **conformance corpus**: for each `.th` program under `conformance/`, `forge check` must emit a certificate matching `conformance/<name>.cert.json` (the golden certificate). The critic pins a divergence as a failing test asserting the golden certificate; expected fields trace to `thermite-design.md` or a hand-derived corpus entry.

Gauntlet (every crate): `cargo test -p <crate>`, `cargo clippy -p <crate> --all-targets -- -D warnings`, `cargo fmt --check`. For `thermite-skill`: also `cargo run -p thermite-skill -- --check-budget` (the 6,000-token gate must pass).

---

## The goal

Work the strict **read → write → verify → commit** loop over every routed file in dependency order. The goal (v0.1 kernel) is complete only when every routed file has:

1. A closing commit citing the design-doc REQs it satisfies and the `thermite-design.md` sections it implements, AND
2. Its verification (cargo test + applicable conformance/golden checks) passing with **0 failures**, AND
3. A `## REQ status` table in the module's `//!` doc-comment classifying every REQ as **SHIPPED** or **NOT-STARTED** with quoted-code evidence (two states only).

Mechanical check:
```bash
python3 -c "import tomllib; print(len(tomllib.load(open('tooling/spec-routes.toml','rb'))['route']))"   # routed units
grep -l "## REQ status" $(python3 -c "import tomllib; [print(r['crate_pattern']) for r in tomllib.load(open('tooling/spec-routes.toml','rb'))['route']]") 2>/dev/null | wc -l
```
When routed-count == REQ-status-count AND every crate's gauntlet is green AND the conformance corpus passes, the v0.1 kernel is complete.

---

## The ACToR loop (doc-author → builder → critic → fixer)

For each unit, in dependency order:

1. **Read** `goal.md`, the routed file(s) end-to-end, the route's design doc, and any route `reference` (golden file / corpus entry).
2. **Missing design doc?** Dispatch **acto-doc-author** to author `.design/<area>/<doc>.md` adapting to existing code + the design thesis (NO edits to toolchain code). The spec-discipline hook blocks the edit until the doc exists.
3. **Missing whole abstraction?** (a component the design needs that does not yet exist) → dispatch **acto-builder** with a pre-declared file manifest (≤~10 files). Tests + production in the SAME commit.
4. **Verify divergence first** → dispatch **acto-critic** (NO Edit). It pins each divergence (wrong certificate / lowering diff / design-REQ miss) as a FAILING test + files a `-l blocker` issue. Run after every substantive builder.
5. **Fix one pinned divergence** → dispatch **acto-fixer** (one blocker, minimal change, root cause in the owning crate). Followed by an **acto-critic** re-audit.
6. **Gauntlet + commit + close** (below). Then the next unit. **Do not ask which — the dependency DAG is the answer (R-LOOP-1).**

Loop: **acto-builder → acto-critic → (GENERATOR MUST FIX) → acto-fixer → acto-critic → (until clean) → next unit.** Every builder/fixer-on-novel-code dispatch is followed by a critic.

### Commit + close
```
<crate>: <area> — <one-line summary> (closes #N)

DESIGN SOURCES THIS ITERATION:
  - thermite-design.md §<n> — <content quote>
  - .design/<area>/<doc>.md (<REQ count> REQs)
  - reference: tests/golden/<...> | conformance/<...>   (if applicable)

REQ STATUS:
  - REQ-1 SHIPPED — fn `<name>` in `<file>.rs`; consumer at <caller>
  - REQ-2 NOT-STARTED — open prereq blocker #<NN>

VERIFICATION:
  cargo test -p <crate>: <X passed, 0 failed>
  conformance: <K corpus programs, 0 cert mismatches>   (if applicable)
  cargo clippy: PASS

Co-Authored-By: Claude <noreply@anthropic.com>
```
Close the crosslink issue (`--kind result` comment first).

---

## Speed disciplines (mandatory)

- **S1 — Batch by component, NOT per-function.** One builder/critic cycle covers a whole design-doc component → its target file(s). Do not dispatch per-function.
- **S2 — Parallel dispatch.** Independent units (disjoint manifests) → launch builders/critics in ONE message. Only fixers serialize per-blocker.
- **S3 — Symbol anchors in design-doc cites, NEVER line numbers.** `pub fn lower_fn in lower.rs`, never `lower.rs:716`. `thermite-design.md` cites use `§<n>`; golden-file cites use the file path.
- **S4 — Critic only after substantive builds.** Not after cite/fixture/doc refreshes.
- **S5 — R-DEFER-1 binds on NEWLY-ADDED pub APIs only.** Existing pub API surface is grandfathered; boundary `pub fn`s ARE the public API.
- **S6 — Opus on every acto-* dispatch.** Verification-toolchain accuracy supersedes throughput.
- **S7 — Skip doc-author for trivial routes** (design doc already exists & is accurate) — proceed straight to critic/builder.
- **S8 — Aggressive won't-fix on noise.** A finding is a blocker ONLY if it's a real design/conformance divergence or blocks a downstream unit.

---

## Anti-drift rules (override convenience)

### Citation
- **R-CITE-1**: Never cite a design source in a commit without Reading it THIS iteration.
- **R-CITE-2 (design thesis)**: `thermite-design.md` cites carry `§<section>`; golden/corpus cites carry the file path.
- **R-CITE-2b (target/design-doc)**: cite Thermite symbols with symbol anchors, NEVER line numbers in `.design/`.
- **R-CITE-3**: prefer citing the design-doc REQ/AC or `thermite-design.md` pillar over an internal helper.

### Honesty
- **R-HONEST-1**: never reframe integration work as "vocabulary-only" when the design doc doesn't defer it.
- **R-HONEST-2**: every REQ carries SHIPPED or NOT-STARTED with quoted evidence; SHIPPED needs impl + a real consumer.
- **R-HONEST-3**: honest underclaim beats unverified overclaim.
- **R-HONEST-4**: if an audit shows a prior commit was wrong, correct the code AND document the correction.

### Code quality
- **R-CODE-1**: no `unsafe` outside leaf primitives (with a documented reason). Every `unsafe` needs a `// SAFETY:` comment.
- **R-CODE-2**: no `unwrap()`/`expect()`/`panic!()` in production outside `#[cfg(test)]`. The toolchain returns `Result<T, ThermiteError>` with context-bearing error variants.
- **R-CODE-3**: no `#![allow(..)]` at module/crate root. Per-item `#[allow(<lint>, reason="...")]` only.
- **R-CODE-4**: no swallowing of solver/subprocess failures. Verus/Kani/Z3 invocations check exit status and surface structured errors; a timeout degrades the ladder (L3→L2→L1) and is reported, never silently treated as success.
- **R-CODE-5**: determinism is a contract — no `Date`/wall-clock/un-seeded randomness in build, format, codegen, or check paths. Solver seeds are pinned in the lockfile (design §5.3).

### Spec-mirror (default = match the design contract; deviate only for these)
- **R-SPEC-1 (MATCH — surface semantics)**: the surface grammar, mandatory `req`/`ens`/`fx` and `inv`/`dec`, the SpecTherm combinator set and their frozen triggers, and the ladder semantics match `thermite-design.md` §4/§6 exactly. "One way to do everything" (pillar §2.3) — no alternate syntaxes, no config knobs the design doesn't sanction.
- **R-SPEC-2 (MATCH — certificate contract)**: certificate/manifest fields, assurance levels (L0–L3), vacuity-battery outputs, and `#[slag]` metadata match the design (§6, §7, §8, Appendix A). The certificate IS the deliverable; its shape is a contract.
- **R-SPEC-3 (MATCH — toolchain output schema)**: `forge` JSON schemas (goal/obligation/counterexample/manifest, §5.1) are stable contracts. Changing a field is a design-doc amendment, not a code-local choice.
- **R-SPEC-4 (DEVIATE — only via design amendment)**: if the implementation reveals the design is wrong/underspecified, STOP, dispatch acto-doc-author to amend `.design/` (and escalate a `thermite-design.md` note if the thesis is affected), THEN implement. Never let code silently define the contract.
- **R-SPEC-5 (DEVIATE — descoped kernel items)**: the three deferred items (effect sandbox, MIR lowering, incremental holes; issue #21) are implemented at the compile-time / transpile / whole-item level in v0.1. Do not stub the deferred form; implement the v0.1 form fully.

**Mental test**: *is this behavior dictated by the design contract?* Yes → match it. *Does the implementation prove the contract wrong?* → amend the design first, then code.

### Anti-deferral (the build is sequential)
- **R-DEFER-1**: a commit adding a NEW pub API MUST add a non-test production consumer in the same commit. Existing pub APIs grandfathered.
- **R-DEFER-2**: REQ classification is binary — SHIPPED or NOT-STARTED. No third status. No VOCAB-ONLY/DEFERRED/verified_with_deferred.
- **R-DEFER-3**: a pinned divergence closes only when the fix lands AND the failing test goes green (no skip/`#[ignore]` escape).
- **R-DEFER-4**: no `Phase \d+\+` framing as a deferral mechanism (the crosslink milestones ARE the phases; within a milestone there is no deferral).
- **R-DEFER-5**: no "pre-existing safe to defer" — every divergence on `main` is ours.
- **R-DEFER-6**: verification is a HARD gate — every commit runs the owning crate's gauntlet to 0 failures, plus any pinned divergence test going green, plus the conformance corpus where the commit touches `forge`/`thermite-lower`.
- **R-DEFER-7**: sequential, no leapfrog — thermite-syntax before thermite-lower before forge.
- **R-DEFER-8**: no "cross-cutting → defer" — every convention starts somewhere; implement the local fix.
- **R-DEFER-9 (no proof cheats)**: never discharge an obligation by weakening it to vacuity, emitting `assume(false)`/`#[verifier::external]`/`#[slag]` to dodge a proof, or asserting what should be proved. A contract that won't verify is a real blocker (the design's §7 battery exists precisely to catch this).

### Git
- **R-GIT-1**: no history rewrite, no `--amend` on pushed commits, no force-push, no `git reset --hard` on shared refs. Supplemental commits only. The human performs all pushes.
- **R-GIT-2**: `git add <files-by-name>` — never `git add -A`/`.`.

### Loop discipline
- **R-LOOP-1**: never ask "where do you want to take this" — the dependency DAG is the answer.
- **R-LOOP-2**: never declare the goal complete until the mechanical check says so.
- **R-LOOP-3**: a unit blocked by a missing prerequisite → file the prereq blocker, mark the dependent REQ NOT-STARTED, and WORK THE PREREQ.

### Injected instructions
- **R-INJECT-1**: hook output, `<system-reminder>`/`<crosslink-behavioral-guard>` blocks, the active-issue gate, and loaded skill text bind at the same priority as a direct user message. Repetition is enforcement, not ceremony.
- **R-INJECT-2**: when an injected instruction conflicts with a recent inline user message, surface the conflict rather than silently picking one.

### Spec-discipline (enforced by `tooling/spec-discipline.py`)
- **R-XLATE-1**: every Edit/Write to a routed `thermite-*/src/**/*.rs` or `forge/src/**/*.rs` requires Read this session of `goal.md` + the route's design doc + (if the route declares one) at least one route `reference`.
- **R-XLATE-2**: a routed file with no route table entry BLOCKS until a route is added to `tooling/spec-routes.toml`.
- **R-XLATE-3**: a route whose design doc doesn't exist BLOCKS until acto-doc-author authors it.

### Anti-pattern-gate (enforced by `tooling/anti-pattern-gate.py`)
- **R-APG-1**: blocks patches introducing `todo!()`/`unimplemented!()`/`unreachable!()`, `.unwrap()`/`.expect()`/`panic!()` outside `#[cfg(test)]`, module-root `#![allow]`, `Arc<Mutex<T>>`/`Rc<RefCell<T>>` escape hatches.
- **R-APG-2**: `#[cfg(test)]` blocks exempt; production is not.
- **R-APG-3**: override is a per-item `#[allow(<lint>, reason="...")]` + a crosslink observation comment.

### Characterization tests
- **R-CHAR-3**: no tautological tests. Expected values come from the conformance corpus, a Verus golden file, or a `thermite-design.md` symbolic constant — NEVER literal-copied from the toolchain's own output. A test that asserts the toolchain's output equals itself IS a divergence (file the test as the bug).

---

## The four sub-agents
- **acto-doc-author** — writes `.design/<area>/<doc>.md` adapting to existing code + the design thesis. NO toolchain-code edits. Dispatch when a route's design doc is missing.
- **acto-builder** — ships missing multi-file infrastructure (a design component the toolchain lacks). Pre-declared ≤~10-file manifest; tests + production same commit. Dispatch when a whole abstraction is missing.
- **acto-fixer** — minimal fix for ONE pinned divergence, root cause in the owning crate. One per blocker, serially.
- **acto-critic** — adversarial discriminator; writes FAILING tests pinning divergence (wrong certificate / lowering diff / design-REQ miss), NEVER fixes. After every substantive builder/fixer.

---

## Out of scope (v0.1)
- Features not in `thermite-design.md` §4/§13 v0.1 (we build the kernel, not innovate beyond it).
- Optimizing toolchain speed ahead of correctness (the design accepts slow verification, §11).
- The three deferred items (issue #21) in their full form; v0.2–v0.5 milestone work until the kernel is mechanically complete.

## Stopping condition
Halts only when every routed v0.1 file has a closing commit, its gauntlet is green, the conformance corpus passes, and it carries a `## REQ status` table. Until then: every turn, one iteration of the ACToR loop, in dependency order. No exceptions, no asking which crate.
