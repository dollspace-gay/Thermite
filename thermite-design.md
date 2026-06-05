# Thermite

**A hyper-strict, verification-mandatory programming language for AI agents, built on Rust.**

> Thermite is what you get when you add energy to rust. Iron oxide plus aluminum: inert powder until ignited, then it burns at 2,500°C and cuts through steel. The name is the thesis — take Rust's substrate, add the energy budget agents bring (compute, patience, token spend), and produce something hot enough to weld trust into software.

Version 0.1 — Design Document
Status: Draft for discussion

---

## 1. Thesis

Every verification-heavy language in history died of human ergonomics. Annotating contracts on a ten-line function feels, to a human, like filing paperwork to open a window. Humans pay for verification in attention — their scarcest resource — so verification has remained a niche luxury reserved for kernels, hypervisors, and avionics.

AI agents invert the economics:

- **The annotation cost is paid in tokens and compute**, which are cheap, parallelizable, and falling in price every year.
- **The cost of misplaced trust in autonomously generated code** is rising every year, because agents are writing more code with less supervision.
- **Agents have infinite patience** for proof-obligation loops, and their failure mode (locally plausible, globally wrong) is exactly the failure mode machine verification catches.

Thermite is the arbitrage: burn the cheap resource (tokens) to buy the expensive one (trust). It is deliberately insufferable for humans to write. Humans are not the user.

### What "trustable" means, precisely

A Thermite artifact ships with a certificate that says: *the implementation satisfies these contracts, the contracts are machine-certified non-vacuous, and they kill X% of generated mutants.* The residual trust question — "are these the contracts you wanted?" — is answered by reading a short declarative spec layer, not by mentally executing code. Trust is relocated twice (code → spec → spec-intent alignment) and each relocation shrinks the residue and makes it more legible. Trust that a skeptical third party can audit in minutes, without trusting the agent or anyone's vibes, is the product.

---

## 2. Design Pillars

1. **Verification is the floor, not the ceiling.** Every function carries a contract. Every contract is checked. Unverified code requires loud, greppable ceremony; verified code requires none.
2. **The whole language fits in a skill.** The complete surface syntax and semantics must be teachable to an LLM in ≤ 6,000 tokens. This is a hard budget, enforced in CI against the canonical `THERMITE.skill.md`. Any feature that doesn't fit doesn't ship.
3. **One way to do everything.** No expressiveness for its own sake. No style decisions. Exactly one canonical formatting (the formatter has zero configuration options). Predictability over elegance.
4. **Feedback is always crisp.** Every toolchain response is structured, machine-readable, and actionable. A timeout is never the final answer; the gate degrades, it does not block.
5. **Locality.** Every block is independently parseable, independently checkable, and addressable by name. An edit's blast radius is its block. A proof must not break because of an unrelated edit.
6. **The contract is the interface.** Once a function verifies, no caller (human or agent) ever needs to read its body again. Specs double as the compressed representation of the codebase — a context-window subsidy that grows with project size.

---

## 3. The Stack

```
┌────────────────────────────────────────────────┐
│  Thermite surface language                     │   small, regular, contract-mandatory
├────────────────────────────────────────────────┤
│  Forge (toolchain + goal-state REPL)           │   the agent's actual interface
├────────────────────────────────────────────────┤
│  Verification ladder                           │
│   L3  SMT proof          (Verus/Z3)            │
│   L2  bounded check      (Kani/CBMC)           │
│   L1  runtime contracts  (active all profiles) │
│   L0  unverified         (#[slag] only)        │
├────────────────────────────────────────────────┤
│  Rust (MIR-level lowering)                     │   semantics, borrow checker, ecosystem
├────────────────────────────────────────────────┤
│  LLVM                                          │   codegen
└────────────────────────────────────────────────┘
```

Thermite lowers to Rust MIR, inheriting the borrow checker, the optimizer, and `crates.io` interop (through a contract-wrapped FFI boundary — see §9). Verification reuses the Verus and Kani toolchains rather than reinventing solvers.

---

## 4. Surface Language

### 4.1 Functions: contract-first, body-second

```thermite
fn binary_search(haystack: &[u32], needle: u32) -> Option<usize>
  req sorted(haystack)
  ens match result {
        Some(i) => i < haystack.len() && haystack[i] == needle,
        None    => forall_in(haystack, |x| x != needle),
      }
  fx  pure
{
  let mut lo: usize = 0;
  let mut hi: usize = haystack.len();
  loop
    inv lo <= hi && hi <= haystack.len()
    inv forall_below(haystack, lo, |x| x < needle)
    inv forall_from(haystack, hi, |x| x > needle)
    dec hi - lo
  {
    if lo == hi { return None; }
    let mid = lo + (hi - lo) / 2;
    if haystack[mid] == needle { return Some(mid); }
    if haystack[mid] < needle  { lo = mid + 1; } else { hi = mid; }
  }
}
```

Anatomy:

- **`req`** — precondition. Mandatory keyword; `req true` must be written explicitly if there is genuinely no precondition, so absence is always a parse error, never an implicit default.
- **`ens`** — postcondition. Mandatory. Must mention `result` unless the return type is `()` (structurally enforced — see §7).
- **`fx`** — effect row. Mandatory. One of `pure`, or a set drawn from `{read(path), write(path), net(domain), alloc, time, rand, panic}`. The runtime enforces the row as a sandbox: a function declared `fx pure` that attempts I/O is killed at the syscall boundary, not trusted at the type level alone. Effect rows compose: a caller's row must subsume every callee's row, checked at compile time.
- **`inv` / `dec`** — loop invariants and a decreases-measure. Mandatory on every `loop`/`while`. Termination is proved by default; divergence requires `fx diverge` in the row.

### 4.2 The spec sublanguage is deliberately weak

Contracts are written in **SpecTherm**, a restricted total language:

- **No general quantifiers.** Quantification is only available through a fixed library of bounded combinators (`forall_in`, `forall_below`, `exists_in`, `count_where`, `sorted`, `permutation_of`, `disjoint`, …), each with hand-tuned, frozen SMT triggers. Undecidability lives in quantifier instantiation; Thermite locks the cage.
- **Closure bodies are flat predicates.** A combinator's predicate-closure body (`|x| …`) may use comparisons, arithmetic, boolean/logical operators, field/index access, and calls to *named* `spec fn`s — but it may **not** contain another combinator. This is what makes "locks the cage" precise: there are no *anonymous* nested quantifiers, so the frozen trigger on a combinator's predicate application fully controls its instantiation. Genuine nested quantification is written as a named `spec fn` — which may itself quantify (boundedly), but carries its own `dec` measure and appears by name in the audit surface. Stated exactly: *every quantifier is a bounded combinator with a frozen trigger; composition happens only through named `spec fn`s, never anonymous nested quantifiers.*
- **Spec functions are executable.** Every `spec fn` is total, terminating (checked), and compilable to a runtime check. This guarantees the L1 fallback rung always exists for every contract, and it keeps the solver on predictable ground.
- **No spec-level recursion without a `dec` measure**, same as code.

The expressiveness ceiling is a feature. A contract you can't write in SpecTherm is a contract the solver was going to choke on; the language refuses the bet upfront instead of letting the agent discover the timeout three hours in.

### 4.3 Named blocks and semantic addressing

Every item and every block has a stable address:

```
binary_search                  — the function
binary_search.loop#1           — its first loop
binary_search.loop#1.inv#2     — the second invariant
```

The toolchain's edit operations take addresses, not string matches:

```
forge edit binary_search.loop#1.inv#2 --replace "forall_from(haystack, hi, |x| x > needle)"
forge insert-after binary_search --item "fn linear_search ..."
```

This eliminates the fragile-string-match editing problem at the language level. Blocks are delimited by explicit `{ }` with no significant whitespace; a syntax error inside one item cannot cascade into the next (items are parsed independently — recovery is per-item by construction).

### 4.4 What Thermite removes from Rust

To fit the 6k-token skill budget and keep agent-written code formulaic:

| Removed | Replacement |
|---|---|
| Explicit lifetimes | Region inference + arena defaults; the three blessed ownership patterns (owned, `&`, `&mut`) cover the surface; anything fancier lives behind FFI |
| Trait system (full) | Closed set of built-in interfaces (`Eq`, `Ord`, `Hash`, `Iter`, `Display`); no user-defined traits in v0.1 |
| Macros | None. Code generation is the agent's job |
| `unsafe` | `#[slag]` (§8), which is harder to write and louder to read |
| Method syntax vs UFCS choice | One call syntax |
| `match` ergonomics special cases | One desugaring, always explicit |
| Implicit integer widening rules | All conversions explicit; arithmetic overflow is a proof obligation, not a runtime panic |

The result reads like a boring, regular, slightly verbose Rust dialect — exactly the register agents reproduce most reliably.

---

## 5. Forge: the Goal-State REPL

Agents don't "write a file and run it." The primary interface is a Lean-style dialogue where the unit of progress is **discharging a goal**, and every step is cheap and fully verified.

### 5.1 The loop

```
agent → forge: declare fn signature + contract (body = hole `?0`)
forge → agent: GOAL STATE
                given:  sorted(haystack)
                want:   match result { Some(i) => ..., None => ... }
                holes:  ?0 : body
agent → forge: fill ?0 with loop skeleton, invariants, two new holes ?1 ?2
forge → agent: GOAL STATE
                ?1 discharged ✓ (L3, 0.4s)
                ?2 open — obligation: lo <= hi preserved across `lo = mid + 1`
                          counterexample: lo=3, hi=3, mid=3 → lo=4 > hi=3
agent → forge: guard the branch
forge → agent: ALL GOALS DISCHARGED ✓  binary_search certified L3
                contract score: non-vacuous ✓, mutants killed 23/24 (§7)
```

Three properties make this loop work for LLMs:

1. **The oracle does the remembering.** Goal state is recomputed and re-presented every turn; the agent never has to maintain global correctness in its head, only propose the next local move.
2. **Counterexamples, not adjectives.** When an obligation fails, Forge returns a concrete witness from the solver (or from Kani's bounded search) — never just "verification failed."
3. **Every message is a prompt.** All Forge output is structured (JSON with a stable schema, rendered to readable text), includes the relevant source inline, and reserves a `suggested_move` slot populated by deterministic heuristics (missing-invariant patterns, overflow-guard templates, trigger hints).

### 5.2 Solver tantrum protocol

The discontinuity of SMT solvers is the central engineering risk (§6). Forge's contract with the agent: **the gate degrades, it never blocks.**

- Every obligation gets a fixed solver budget (default 10s, portfolio of Z3 + cvc5 seeds in parallel).
- On timeout, Forge automatically attempts **L2** (Kani bounded check, default bound from type-driven heuristics) and reports honestly: `certified L2 (bound: slices ≤ 8, ints full range)`.
- If even L2 times out, the obligation drops to **L1**: the SpecTherm contract compiles to runtime checks, the function is certified L1, and a `lowered-assurance` flag is attached to the build manifest.
- Forge emits a **solver profile** on every timeout — which combinator's triggers blew up, which assertion consumed the budget — so "maybe" becomes "here's where I got lost," which is actionable for proof repair.
- The whole-project assurance level is the min over functions, displayed on every build. Driving L1s and L2s back up to L3 is a background task agents can run unattended (proof repair is a local, checkable move — the task shape LLMs are best at).

### 5.3 Determinism

Builds, formatting, codegen, and check results are bit-reproducible given the same toolchain version and solver seeds (seeds are pinned in the lockfile). The agent can reason about behavior without re-running things, and a proof that passed yesterday passes today unless something semantically relevant changed. Proof results are content-addressed and cached per item: an edit to `f` cannot invalidate `g`'s certificate unless `g`'s contract references `f`'s contract.

---

## 6. The Verification Ladder

| Level | Mechanism | Guarantee | Termination of the check |
|---|---|---|---|
| **L3** | SMT proof (Verus-derived) | Contract holds for **all** inputs | Not guaranteed → budget + downgrade |
| **L2** | Bounded model check (Kani-derived) | Contract holds for all inputs **up to bound** | Guaranteed |
| **L1** | Runtime contract checks | Violations **detected at the call site**, in every build profile (not just debug) | Guaranteed |
| **L0** | `#[slag]` | Nothing. Trusted by fiat | — |

Rules:

- L3 is the default target for every function.
- Downgrades are automatic, logged, and surfaced in the build manifest; upgrades are a standing background task.
- The certificate attached to a build artifact lists every function's level, every `#[slag]` block, and the contract-quality scores from §7. This manifest **is** the deliverable's trust statement.
- **`#[slag]`, the L0 row, and L1 enforcement.** The L0 row measures assurance about the *body*: nothing is proved about the implementation — it is trusted by fiat. But a `#[slag]` function's *contract* is still mandatory and is enforced at runtime (§8), so its certificate carries level **L1** with a `slag: true` flag — `L1` because the contract is L1-checked at the call site, `slag` because the body is unproven. The L0 row therefore names the body-proof aspect (recorded by the `slag` flag), never an unchecked contract: slag exempts *proving*, never *stating and checking*. (`fx` effect rows are likewise enforced — at compile time in v0.1, at the syscall boundary later — independent of the proof level.)

---

## 7. The Vacuity Battery: anti-Goodhart by construction

A mandatory gate creates pressure to game it: weaken the postcondition until it's trivially true. Thermite makes the degenerate moves mechanically detectable, and runs the battery inside the gate itself. A function does not certify until its **contract** certifies.

Deterministic checks, in order of cost:

1. **Structural triage** (free):
   - `ens` simplifies to `true` → reject.
   - `ens` does not mention `result` (non-unit return) → reject.
   - `ens` is syntactically implied by `req` alone → reject.
   - Effect row is maximal (`fx *`) without `#[slag]` justification → reject.
2. **Tautology check** (one solver query per contract): is `ens` provable from `req` + types **without the function body**? If yes, the contract says nothing about the implementation → reject with the proof as the explanation.
3. **Vacuity check** (one query): is `req` satisfiable? An unsatisfiable precondition verifies everything about the empty set → reject with the unsat core.
4. **Mutation scoring** (parallel, budgeted): Forge generates N mutants of the body (operator flips, off-by-ones, early returns, branch swaps — fixed deterministic mutator set, seeded from the lockfile) and re-verifies each against the contract. The **kill ratio** is recorded in the certificate. A configurable floor (default 60%) gates certification; below it, Forge reports exactly which mutants survived, which tells the agent *which behavior the contract fails to constrain* — a precise prompt for strengthening.
5. **Strengthening probes** (budgeted): template-based tightenings of `ens` (tighter bounds, added conjuncts from the SpecTherm combinator library) are tried automatically; if a strictly stronger contract proves with no body change, Forge suggests it.

What the battery cannot check — whether the contract is the property the *user* wanted — is exactly the residue surfaced for review: the certificate includes the full spec layer (typically a few percent of total line count), pre-screened to be non-vacuous, non-trivially-weak, and mutation-scored. The reviewer's job is reduced to reading declarative statements and asking "is this what I meant?" That review slot is pluggable: a human, or a critic model whose only question is spec-intent alignment.

---

## 8. `#[slag]`: the escape hatch

Slag is the waste product of a thermite burn — the part that didn't become weld. Unverified code is named accordingly.

```thermite
#[slag(reason = "vendored SIMD intrinsics; contract checked at boundary by L1 wrapper",
       owner  = "agent:forge-7/session-2026-06-04",
       review = "required")]
fn simd_sum(xs: &[u32]) -> u64
  req xs.len() <= u32::MAX as usize
  ens result == spec_sum(xs)          // contract still mandatory — enforced at L1
  fx  pure
{ ... }
```

Rules:

- `reason`, `owner`, and `review` fields are mandatory and non-empty (checked).
- The contract is **still mandatory** and is enforced at L1 (runtime) — slag exempts you from *proving*, never from *stating and checking*.
- Every slag block appears in the build manifest and in `forge audit` output. `grep slag` over a codebase is the complete inventory of fiat-trusted code.
- CI policy hooks can cap slag count or require a second-party sign-off per block.

The polarity inversion is the point: in every mainstream language, verification is the exotic add-on; in Thermite, *non*-verification is the exotic add-on, and it costs more keystrokes, more metadata, and more visibility.

---

## 9. Interop and the FFI boundary

Thermite gets Rust's ecosystem, but never silently inherits its trust assumptions.

- A `crates.io` dependency is imported through a **boundary module**: each foreign function used must be given a Thermite signature with `req`/`ens`/`fx`, enforced at L1 (runtime checks on every crossing) since the foreign body can't be proved.
- Boundary modules are slag-adjacent: they appear in the audit manifest with per-function contracts, so the trusted computing base is enumerable — it is exactly (slag blocks ∪ boundary contracts ∪ the toolchain itself).
- Pure-Thermite transitive closures can be certified end-to-end; the manifest distinguishes "verified to the boundary" from "verified, period."

Composition rule (the reason any of this scales): if `g` calls `f` only through `f`'s contract, then `g`'s certificate is valid independent of `f`'s body. Trust is invariant under composition instead of decaying multiplicatively — which is the property that matters once unsupervised agents start building large systems.

---

## 10. The Skill is the Spec

The canonical language definition is `THERMITE.skill.md`: the complete surface grammar, the SpecTherm combinator library with one example each, the Forge command set, the ladder semantics, and the slag rules — budgeted at **≤ 6,000 tokens, enforced in CI** (the skill is regenerated from the grammar and combinator registry; if it exceeds budget, the feature that pushed it over is reverted).

Consequences:

- **No cold-start corpus problem.** An agent that has never seen Thermite reads the skill at session start and holds the *entire* language in context — no half-remembered semantics, no hallucinated stdlib, because the stdlib surface is in the skill too.
- **No version skew.** The skill is versioned with the toolchain; Forge serves the matching skill on `forge skill`, so the agent's mental model and the checker are never out of sync.
- **Uniform training signal.** When models do eventually train on Thermite code, the one-way-to-do-everything property means the corpus is stylistically uniform — maximum signal per token.

---

## 11. Anti-Goals

Thermite explicitly does not try to be:

- **Pleasant for humans to write.** Humans review the spec layer and the audit manifest; they should rarely write bodies.
- **Expressive.** No metaprogramming, no clever abstractions, no embedded DSLs. If a pattern is common, it becomes a stdlib function or a SpecTherm combinator through the (slow, budget-gated) RFC process — never a user-level abstraction mechanism.
- **A proof assistant.** Full functional-correctness proofs of deep mathematics are out of scope; that's Lean's job. Thermite's L3 targets the contracts ordinary systems code needs: bounds, overflow, state-machine invariants, frame conditions, termination.
- **Fast to compile.** Verification time is an accepted cost, mitigated by per-item caching, parallel solver portfolios, and the degrade-don't-block protocol — never by weakening the gate.

---

## 12. Risks and Mitigations

| Risk | Mitigation |
|---|---|
| SMT discontinuity: small edits flip proofs to timeouts | Quantifier-locked SpecTherm; frozen triggers; per-item proof caching; portfolio solving; automatic L2/L1 degrade; solver profiles as repair prompts |
| Goodhart on the gate: vacuous/weak contracts | §7 battery inside the gate; mutation kill-ratio floor; strengthening probes; spec layer surfaced for the one irreducible judgment |
| Spec-intent gap: right proof, wrong property | Spec layer kept small, declarative, pre-screened; pluggable critic-model/human review slot; contracts as the *only* artifact that needs intent review |
| Ecosystem gravity: nobody adopts a new language | Skill-as-spec removes the training-corpus barrier; Rust lowering gives ecosystem access from day one; the target user (agents) adopts by config flag, not by community persuasion |
| Proof brittleness across edits | Locality by construction: certificates are per-item and content-addressed; cross-item invalidation only through contract references |
| Bounded checks oversold as proofs | Manifest states bounds explicitly; L2 and L3 are visually and programmatically distinct everywhere they appear |

---

## 13. Roadmap

- **v0.1 (kernel):** surface language §4, lowering to Rust MIR, L3/L1 rungs via Verus passthrough, structural vacuity checks, `forge` CLI with goal-state output, skill generator + CI budget.
- **v0.2 (ladder):** Kani-backed L2 with type-driven bound inference, automatic degrade protocol, solver profiles, proof cache.
- **v0.3 (battery):** mutation scoring, strengthening probes, tautology/vacuity solver checks in the gate, audit manifest format v1.
- **v0.4 (boundary):** crates.io boundary modules, L1-enforced FFI contracts, end-to-end vs to-the-boundary certification in the manifest.
- **v0.5 (repair):** background proof-repair agent loop (L1→L2→L3 upgrades), critic-model spec-review integration, multi-agent Forge sessions.

---

## Appendix A — A complete tiny program

```thermite
spec fn spec_sum(xs: &[u32]) -> u64
  dec xs.len()
{
  match xs {
    []          => 0,
    [head, ..t] => head as u64 + spec_sum(t),
  }
}

fn sum(xs: &[u32]) -> u64
  req xs.len() <= 1_000_000
  ens result == spec_sum(xs)
  ens result <= xs.len() as u64 * u32::MAX as u64
  fx  pure
{
  let mut acc: u64 = 0;
  let mut i: usize = 0;
  while i < xs.len()
    inv i <= xs.len()
    inv acc == spec_sum(&xs[..i])
    inv acc <= i as u64 * u32::MAX as u64
    dec xs.len() - i
  {
    acc = acc + xs[i] as u64;   // overflow: discharged from inv#3 + req
    i = i + 1;
  }
  acc
}
```

Certificate (excerpt of build manifest):

```json
{
  "item": "sum",
  "level": "L3",
  "solver_time_ms": 612,
  "contract_quality": {
    "tautology": false,
    "vacuous_precondition": false,
    "mutants_killed": "17/18",
    "survivor": "mutant#11: `i = i + 1` → `i = i + 2` survives ens but killed by inv#2"
  },
  "effects": ["pure"],
  "slag": false
}
```

## Appendix B — Forge command surface (v0.1)

```
forge new <name>                      create project (manifest, lockfile, skill pin)
forge goal <item>                     print goal state for an item
forge fill <hole-addr> <code>         fill a hole; returns new goal state
forge edit <addr> --replace <code>    semantic edit by address
forge check [item]                    run the ladder; returns per-obligation results
forge battery [item]                  run vacuity battery + mutation scoring
forge audit                           full slag + boundary + assurance inventory
forge skill                           emit the canonical THERMITE.skill.md for this toolchain
forge repair [item]                   background L1/L2 → L3 upgrade loop
```
