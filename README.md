# Thermite

**A programming language where the code has to prove it works.**

> Thermite is what you get when you add energy to rust. Iron oxide plus aluminum: inert powder until ignited, then it burns at 2,500 °C and cuts through steel. Take Rust's substrate, add the energy budget AI agents bring (compute, patience, token spend), and produce something hot enough to weld trust into software.

## The problem

When an AI writes code for you, how do you know it's right? Today the answer is "read it yourself" or "trust the vibes." Neither scales. Code review by humans is exactly the bottleneck AI was supposed to remove — and "the tests pass" only tells you about the cases somebody thought to test.

Formal verification — *mathematically proving* code correct — has existed for decades. It never caught on, because writing the proofs is miserable for humans. But AI agents don't get bored, don't get tired, and pay for effort in cheap compute instead of expensive attention. **Thermite's bet: agents flip the economics of proof.** Burn the cheap resource (tokens) to buy the expensive one (trust).

So Thermite is deliberately strict in a way no human would tolerate. Humans aren't the user. Humans get the part they're good at: deciding what the software *should* do, and reading the receipts.

## How it works, in plain terms

**Every function must make three promises.** Not as comments — as enforced syntax. Leaving one out is a compile error:

- `req` — *"here's what must be true before you call me"* (e.g. "the list is sorted")
- `ens` — *"here's what I guarantee about my answer"* (e.g. "if I return an index, the item is really there")
- `fx` — *"here's everything I'm allowed to touch"* (e.g. "nothing — I'm pure", or "I may read this one file")

**Every promise gets graded.** The toolchain (`forge`) tries to keep every function at the top rung of a ladder:

| Rung | What it means |
|---|---|
| **L3** | A machine-checked **mathematical proof** that the promise holds for *every possible input*. Not tested — proven. |
| **L2** | Proven for all inputs up to a stated size. |
| **L1** | The promise is checked **while the program runs**; violations stop it on the spot. |
| **L0** | Trusted by fiat. The escape hatch — spelled `#[slag]`, deliberately ugly, so `grep slag` lists every line of code anyone is taking on faith. |

It always aims for L3 and only slides down honestly. One thing never slides: if the prover finds an actual **counterexample** — *"your code is wrong when n = 3"* — that's a hard failure. It is never softened into a lower grade.

**You can't cheat the grade.** A promise that promises nothing (`ens true`) would technically always pass. So every contract is audited for emptiness, and then dozens of deliberately-broken mutant copies of your code are generated — the contract must *catch* them. A contract too weak to notice sabotage is rejected.

**The `fx` promise has teeth at runtime too.** When you build a real binary, Thermite derives an operating-system-level cage from the declared effects. A function that said "I'm pure" and then tries to open a network connection gets killed by the OS mid-syscall. Belt, suspenders, and a tripwire.

**How an agent actually writes it.** Like a conversation. Declare the contract first with a hole where the body goes (literally `?0`). `forge goal` shows what's given and what must be achieved. `forge fill` drops code into the hole and immediately re-checks — failures come back as concrete counterexamples, not vibes. Repeat until: `ALL GOALS DISCHARGED ✓ certified L3`. A program with an unfilled hole physically cannot be built or certified.

Under the hood, Thermite translates to Rust (annotated for the [Verus](https://github.com/verus-lang/verus) prover, which uses the Z3 logic engine), so it inherits Rust's compiler, optimizer, and ecosystem. The full design rationale lives in [`thermite-design.md`](./thermite-design.md).

## The proof it isn't a toy

We built a **working text editor** in Thermite — a real one, like a tiny nano: you run it in a terminal and type. Its editing logic, line navigation, and cursor math are all *proven correct for every input* (L3), and it runs inside the syscall cage, holding only the handful of permissions its `fx` declares. ([`examples/editor/`](examples/) — plus a formatter, a calculator, and a CSV parser, all proven, all runnable.)

Here's what the language looks like — a function that sums a list, with its three promises:

```thermite
fn sum(xs: &[u32]) -> u64
  req xs.len() <= 1_000_000        // what I need
  ens result == spec_sum(xs)       // what I guarantee
  fx  pure                         // what I touch: nothing
{
  let mut acc: u64 = 0;
  let mut i: usize = 0;
  while i < xs.len()
    inv acc == spec_sum(&xs[..i])  // why this loop is right
    dec xs.len() - i               // why this loop ends
  {
    acc = acc + xs[i] as u64;
    i = i + 1;
  }
  acc
}
```

`forge check` turns that into a certificate: proven for all inputs, promise non-empty, mutants killed. `forge build` turns it into a runnable binary with the cage on.

## Don't trust us — audit it

Every claim above is the kind of thing a liar could also type. So the repo ships an audit that **re-derives the trust chain on your machine**:

```sh
make audit        # the full re-derivation (slow — minutes)
make audit-fast   # the 60-second demonstration (one program, one injected bug)
```

The fast version shows you the essentials with your own eyes: a correct program certifies; the *same program with one bug injected* is refused with a counterexample; and the emitted proof re-verifies under an independent copy of the prover with our tooling removed from the loop entirely.

The deep version re-checks every link of the actual guarantee — including having **your own copy of the Lean proof checker re-verify our central theorem** (more on that below), re-running the translation cross-checks on every program in the test corpus (thousands of proof obligations, live), and re-running the forty-odd sabotage tests that prove the prover catches each known class of translation bug. At the end it prints the honest list of what you are *still* trusting (five items, mostly industry-standard tools), because a trust statement that hides its assumptions isn't one. If any tool is missing it says so loudly and refuses to claim success.

<details>
<summary><b>The six checks, precisely</b> (click to expand)</summary>

- **[1] The universal theorem, re-verified by <i>your</i> Lean kernel.** Builds the [`lean/`](lean/) proof spine from source and checks the axiom footprint of the five load-bearing theorems (`lowering_faithful`, `ref_sound`, `exec_ref_sound`, `body_ref_sound`, `while_rule`) — pass only if each depends on nothing beyond `{propext, Classical.choice, Quot.sound}`: no `sorry`, no custom axioms. Skips loudly (and downgrades the verdict) if Lean isn't installed.
- **[2] Full-corpus translation validation.** Runs `forge tv` / `exec-tv` / `body-tv` over every program in [`conformance/`](conformance/); requires zero divergences across thousands of live Z3-checked obligations; prints skip reasons honestly.
- **[3] The falsification battery.** Runs the live "teeth" suites that inject known classes of translation bugs (wrong operator, dropped parenthesis, mis-dispatched method, swapped match arms, dropped statements, broken loop invariants…) and asserts Z3 catches every one — plus one visible end-to-end mutant for legibility.
- **[4] Correspondence drift tripwire.** The Rust encoders are tied to their Lean models by an arm-by-arm [inspection audit](.design/verified/rust-lean-correspondence.md) that pins the exact commits inspected; this check fails if the code has drifted past its audit.
- **[5] Third-party prover re-check.** The committed proof re-verifies under your own Verus/Z3 with `forge` excluded.
- **[6] The residual-trust statement.** Pass requires 1–5; then it prints exactly what remains trusted: the Lean kernel + its three standard axioms; Z3/Verus soundness (with a [kernel-replay proof-of-concept](.design/verified/z3-demotion.md) already covering part of it); the gap between formal spec and human intent; the pinned inspection audit; rustc/LLVM. *Everything else was just re-derived on your machine.*

A run with a skipped guarantee prints **INCONCLUSIVE** and exits nonzero — it cannot be mistaken for a pass.
</details>

## "But how do you know the *translation* is honest?"

The sharpest possible objection: Thermite translates your code into the prover's language — so a buggy translator could prove the wrong statement. Promise `=`, prove `≤`, certificate says L3, everyone goes home happy and wrong.

Two answers, both machine-checked:

1. **Every program, every run:** a second, independent translator (forbidden by the build system from sharing code with the first) re-translates your contracts and bodies, and Z3 must prove both translations equivalent — on *your* program, *every* check. A mistranslation can't slip through quietly on any run.
2. **All programs, once and forever:** that independent translator is small enough that we **proved it correct in Lean** — a machine-checked theorem ([`lean/`](lean/), `Thermite.lowering_faithful`) saying that *every* program passing the cross-check was translated meaning-for-meaning. Quantified over all programs, checked by Lean's kernel, re-checkable by yours (audit check [1]). Every translation bug we ever caught by testing is now individually *refuted by a theorem* — that class of mistake can't silently come back.

This is the [verified-validator architecture](.design/research/formal-methods-sota.md) from the compiler-verification literature (the CompCert lineage), and it has a useful consequence: Thermite's *meaning* is defined by the Lean semantics, not by Verus. Verus is the first proof engine, proven faithful — not the foundation.

## What works today

The short version: **the language is complete enough to write real programs, and the whole pipeline — prove, build, run, cage — works end to end.**

- A general-purpose surface: integers, strings, vectors, maps, structs/enums with invariants, pattern matching, tuples, recursion (including mutual), `for`/`while` loops, `Option`/`Result` — all provable to L3.
- Four finished example programs, all proven and runnable — including the sandboxed text editor.
- The conversational workflow (`forge goal` / `fill` / `edit`) with holes, for incremental agent-driven development.
- A `--target kernel` build mode that emits freestanding, OS-less libraries (the road toward a verified microkernel) — code that needs ambient OS access is refused at compile time.
- The toolchain audits *itself*: its soundness-critical core is Verus-verified, and the translation layer carries the Lean theorem above.
- Built almost entirely by AI agents, adversarially: every component was audited by an independent critic agent whose job is to break it; every divergence found (dozens) was pinned by a failing test and fixed — never skipped.

<details>
<summary><b>The full component inventory</b> (click to expand — dense, for the technically inclined)</summary>

- ✅ **Frontend** (`thermite-syntax`) — lexer, recovering per-item parser, AST (literals keep verbatim text), stable semantic addressing
- ✅ **SpecTherm** (`thermite-spec`) — the frozen bounded-combinator registry + the cage validator (no anonymous nested quantifiers; closure bodies are flat predicates)
- ✅ **Lowering** (`thermite-lower`) — Thermite → Verus (L3), Kani harnesses (L2), executable Rust + always-active runtime checks (L1); compile-time effect-row subsumption; a `req`-bounded `var*var` overflow proof aid discharges multiplication overflow from declared bounds
- ✅ **Forge** (`forge`) — `check` (per-item certificate with content-addressed proof caching), the goal-state REPL (`goal`/`fill`/`edit`/`battery` over `?N` holes — a holed item never certifies), `build` (native binary with runtime checks + the fx-derived seccomp sandbox; `--target kernel` emits a freestanding `no_std`+`alloc` rlib and refuses ambient-syscall `fx`), `audit`, `review`, `repair`, and the translation-validation phases `tv`/`exec-tv`/`body-tv` (four-way `Faithful`/`Divergent`/`Unverifiable`/`Skipped`, skips honest, divergences loud). Automatic L3 → L2 → L1 degrade; a counterexample never degrades.
- ✅ **Anti-Goodhart battery** — structural vacuity triage, solver tautology/unsat-precondition checks, mutation scoring with a kill-ratio floor (excluding only prover-proved-equivalent mutants), strengthening probes
- ✅ **Boundaries** — crates.io FFI + `#[slag]` modules, L1-enforced and runtime-confined to their declared `fx`; the manifest distinguishes *verified-to-the-boundary* from *verified, period*
- ✅ **Self-verification** (`thermite-verified`) — the soundness-critical pure core is itself Verus-verified (`--no-cheating`, no `assume`/`external_body`): effect subsumption, the degrade anti-cheat, the seccomp allowlist, the boundary honesty gate, project aggregation, the mutation floor
- ✅ **Universal lowering soundness** (`thermite-tv` + [`lean/`](lean/)) — per-run translation validation by an independent reference encoder + the kernel-checked Lean proof spine (`ref_sound` over all 8 contract construct classes, `exec_ref_sound`, `body_ref_sound`, the loop `while_rule`, composed into `lowering_faithful`); 13+ negative lemmas machine-refute the historical infidelity classes; arm-by-arm Rust↔Lean [correspondence audit](.design/verified/rust-lean-correspondence.md); a [Lean-SMT/cvc5 PoC](.design/verified/z3-demotion.md) kernel-replays the Z3 step for the QF-linear fragment
- ✅ **The verified primitive basis** (Stages 1–8) + the **primitive-completeness campaign** (C1–C12) — recursive ADTs with invariants, recursion schemes with prove-once induction laws, the contracted effect stdlib, bounded collections, compositional reasoning (callers verify *through* callee contracts; assurance aggregates as the honest min), security-by-construction marked types (SQL injection is *untypeable*), strings, and the full ergonomics layer (destructuring, `for`, guards, or-patterns, `if let`, `Map<K,V>`)
- ✅ **`THERMITE.skill.md`** — the entire language in ≤ 6,000 tokens, regenerated from the compiler's own definitions (a new construct without a skill entry is a compile error), CI-gated
- 🔭 Deferred (tracked): direct MIR-level lowering (transpile-to-Verus stands in, #21); the contract-TV parallel seam (#166); the basis v1.1 layer; and the named proof-spine residuals — full Z3 demotion (upstream-gated), the Lean→Rust extraction bridge, user-ADT `match`/`is` in the proven fragment

Roadmap v0.1 → v0.5: all shipped (milestones #1–#5 closed).
</details>

## Repository layout

| Path | What |
|---|---|
| [`thermite-design.md`](./thermite-design.md) | The design document — why Thermite exists and how it's meant to work |
| [`goal.md`](./goal.md) | The binding contract for the AI agents that build Thermite (the ACToR loop + anti-drift rules) |
| `conformance/` | Golden test programs with hand-certified expected results — the oracle the toolchain is checked against |
| [`examples/`](examples/) | The proven, runnable programs (editor, formatter, calculator, parser) + how to build and run each |
| `.design/` | Per-component design docs — each part of the toolchain answers to one |
| `tooling/` | The enforcement gates (no editing a component without reading its design; no stubs/TODOs) |
| [`lean/`](lean/) | The kernel-checked Lean proof spine (`ref_sound` → … → `lowering_faithful`) + the Lean-SMT demotion PoC |
| `.claude/agents/` | The four ACToR sub-agents that build this repo — auto-discovered by Claude Code (see below) |
| `thermite-*/`, `forge/` | The toolchain itself: `thermite-syntax`, `thermite-spec`, `thermite-lower`, `thermite-tv` (translation validation), `thermite-verified`, `forge` (the CLI), `thermite-skill` |

## Working on Thermite as an agent — the ACToR loop

Thermite is built by AI agents using a four-role adversarial loop, and everything an
agent needs to work the repo **ships in the repo**:

- **`goal.md`** — the binding contract: the full ACToR loop, the verification model
  (corpus + golden oracles), **R-CHAR-3** (the toolchain never authors its own
  oracle), and the anti-drift rules. Read it first, every session.
- **`THERMITE.skill.md`** — the entire surface language + toolchain in ≤ 6,000 tokens,
  generated from the registry (CI-gated). Load it as context before writing any `.th`.
- **`.claude/agents/acto-*.md`** — the four sub-agents below, auto-discovered by Claude
  Code as the `acto-doc-author` / `acto-builder` / `acto-critic` / `acto-fixer`
  subagent types (dispatch them with the Task/Agent tool — no setup needed).
- **`tooling/`** (tracked — ships + fires on a fresh clone) — the enforcement layer:
  the **spec-discipline gate** (`spec-discipline.py`: a routed file can't be edited
  until its `.design/` doc exists), the **anti-pattern gate** (`anti-pattern-gate.py`:
  no stubs/TODOs), and the **route table** (`spec-routes.toml`: which file maps to
  which design doc). `.claude/settings.json` (also tracked) wires these into Claude
  Code's `PreToolUse`/`PostToolUse` events, so they enforce automatically — no setup.
- **`.claude/hooks/`** (gitignored — environment infra, *not* project source) — the
  crosslink issue-tracking + session machinery (`work-check.py` = an active issue is
  required before any edit, plus session/heartbeat/prompt hooks). These are regenerated
  by `crosslink init` and depend on the `crosslink` CLI + the `.crosslink/` DB, so they
  ship with the *harness*, not the repo. Every hook in `settings.json` is guarded with
  `if [ -f "$HOOK" ]` — so a clone **without** crosslink degrades them to no-ops while
  the `tooling/` gates still fire.

### Setting up a fresh clone

The verification gates work immediately (`tooling/` + `settings.json` are tracked). To
also get the crosslink issue-tracking discipline (the `acto-*` agents assume it), install
the `crosslink` CLI and run `crosslink init` at the repo root —
it regenerates `.claude/hooks/` + the `.crosslink/` DB and leaves the tracked
`settings.json` (with its `tooling/` wiring) intact. Without it, you can still build and
verify; you just won't get the issue-before-edit enforcement.

### The four roles

| Agent | Does | Never |
|---|---|---|
| **acto-doc-author** | Writes `.design/<area>/<doc>.md` grounded in real code + the thesis; classifies each REQ **SHIPPED** or **NOT-STARTED** (+ a blocker #). | Touches toolchain code. |
| **acto-builder** | Ships a missing component against a pre-declared ≤ ~10-file manifest; tests + production in one commit. | Widens scope mid-dispatch. |
| **acto-critic** | Adversarially audits a "done" claim; pins each divergence as a **failing test** + files a `-l blocker`. | Fixes anything. |
| **acto-fixer** | Minimal fix for **one** pinned divergence, root cause in the owning crate. | Bundles fixes / refactors. |

The loop: **doc-author → (orchestrator hand-authors the R-CHAR-3 oracle) → builder →
critic → (GENERATOR MUST FIX) → fixer → critic → … until clean.** Every builder/fixer
on novel code is followed by a critic. A "divergence" is anchored to two things the
toolchain may not author for itself — the **conformance corpus**
(`conformance/<name>.cert.json`) and the **Verus golden lowerings**
(`tests/golden/lower/<name>.verus.rs`).

### Session start (any Claude on this machine)

1. Read `goal.md`.
2. Load the language reference. Install it once as a user-level skill so every future
   session auto-discovers it (or just read `THERMITE.skill.md` directly):
   ```sh
   mkdir -p ~/.claude/skills/thermite
   { printf -- '---\nname: thermite\ndescription: Thermite language + Forge toolchain reference.\n---\n\n'; cat THERMITE.skill.md; } > ~/.claude/skills/thermite/SKILL.md
   ```
3. Work the loop. The orchestrator stays hands-on-the-wheel: load context, hand off
   implementation with a clear manifest, and **verify every result** — read the diff,
   re-run the gauntlet, cross-check the critic. Agents are capable and also make
   mistakes; the loop's adversarial structure is what keeps them honest.

Every change must pass the gauntlet (also the CI gate in `.github/workflows/ci.yml`):

```sh
cargo build --workspace
cargo test --workspace          # with `verus` on PATH for the L3 tier
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo run -p thermite-skill -- --check-budget
```
