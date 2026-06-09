# Thermite

**A hyper-strict, verification-mandatory programming language for AI agents, built on Rust.**

> Thermite is what you get when you add energy to rust. Iron oxide plus aluminum: inert powder until ignited, then it burns at 2,500 °C and cuts through steel. Take Rust's substrate, add the energy budget agents bring (compute, patience, token spend), and produce something hot enough to weld trust into software.

## What we're making

Every function in Thermite carries a machine-checked **contract** (`req` / `ens` / `fx`). Verification is the floor, not the ceiling: verified code needs no ceremony, while *un*verified code (`#[slag]`) is the loud, greppable exception. A Thermite artifact ships with a **certificate** that says: the implementation satisfies its contracts, the contracts are machine-certified non-vacuous, and they kill X% of generated mutants. A skeptical third party can audit that trust in minutes — without trusting the agent or anyone's vibes.

The bet: verification has always died of *human* ergonomics — annotating contracts feels like paperwork. AI agents invert the economics. Annotation cost is paid in tokens and compute (cheap, falling); the cost of misplaced trust in autonomously-generated code is rising. Thermite is the arbitrage: **burn the cheap resource (tokens) to buy the expensive one (trust).** It is deliberately insufferable for humans to write. Humans are not the user — they review the spec layer and the audit manifest.

## How it works

```
Thermite surface language     small, regular, contract-mandatory
        │
Forge (toolchain + goal-state REPL)     the agent's actual interface
        │
Verification ladder
  L3  SMT proof        (Verus/Z3)        holds for all inputs
  L2  bounded check    (Kani/CBMC)       holds up to a bound
  L1  runtime contracts                  violations caught at the call site
  L0  #[slag]                            trusted by fiat, loud and greppable
        │
Rust (MIR-level lowering) → LLVM
```

Thermite **transpiles to Rust** — `rustc` is the backend, so there is no separate compiler: `forge check` lowers to Verus-annotated Rust and shells out to Verus/Z3 (L3) or Kani/CBMC (L2); `forge build` lowers to executable Rust + the always-active runtime checks and shells out to `rustc` to produce a native binary. The gate **degrades, it never blocks**: on solver timeout it falls L3 → L2 → L1 and reports honestly (a *counterexample* is never softened to a degrade). A **vacuity battery** (structural triage, solver tautology/unsat-precondition checks, mutation kill-ratio, strengthening probes) runs inside the gate so the mandatory contract can't be gamed into triviality. A built binary's declared `fx` is enforced at runtime by a **seccomp sandbox** — code that exceeds its effects is killed at the syscall boundary, not trusted at the type level alone.

See [`thermite-design.md`](./thermite-design.md) for the full design (thesis, surface language, the Forge REPL, the ladder, the vacuity battery, `#[slag]`, FFI, roadmap).

## Auditing the claim (don't trust the label)

"L3" is only worth what an outsider can independently re-derive. **`make audit`** does exactly that — it trusts neither the agent nor the `L3` label, builds the toolchain from source, and runs three checks on a real program (binary search):

```sh
make audit          # requires the Verus/Z3 prover (VERUS_BIN, on PATH, or ~/.local/bin/verus)
```

- **(A)** the faithful program **certifies L3** (proven for all inputs);
- **(B)** the *same* program with one line changed to return the wrong index is **REFUSED** — the prover reports `postcondition not satisfied` and hard-fails (a rubber stamp would still say L3 here; this is the proof the prover has teeth);
- **(D)** the emitted proof file **re-verifies under third-party Verus with `forge` excluded** (`2 verified, 0 errors`).

A passing audit means trust reduces to a small *named* set — `{ Z3/Verus soundness, the Thermite→Verus lowering }` — and every other step reproduces on the auditor's own machine. (See [`scripts/audit.sh`](./scripts/audit.sh); the lowering link is what the translation-validation work narrows.)

## Status

**v0.1–v0.5 + the universal verified primitive basis (Stages 1–8) + the primitive-completeness campaign (C1–C12) — the language composes into real programs that run, effect-confined.** A Thermite program goes from source to a verified, **runnable, contract-checked, seccomp-sandboxed native binary**. Beyond the basis, the surface now has **general literals + integer operators, `break`/`continue`, `u64`↔`String` (round-trip proven), string search/transform (`split`/`find`/`contains`/`trim`), `Vec` completeness incl. non-Copy `Vec<String>`, built-in `Option`/`Result` with payload-in-contract, plain-`fn` and mutual recursion, tuples + tuple destructuring, `for`-loops / match guards / or-patterns / `if let`, and a bounded verified `Map<K, V>`** — and **all four acceptance programs certify L3 *and* build+run**: a **verified multi-line text editor that runs under the seccomp sandbox** (editing, line-navigation, and cursor-layout logic all L3; only the raw syscalls trusted), a number formatter, a calculator, and a line/CSV parser. Every cluster was grounded in real Verus and adversarially verified by the ACToR critic loop (every divergence it surfaced — ~20 across the campaign — caught and fixed, never skipped), and the toolchain's soundness-critical core is **itself Verus-verified** (`thermite-verified`), down to a mutation gate that **excludes only prover-proved-equivalent mutants** so an honest contract is never falsely flagged weak.

- ✅ **Frontend** (`thermite-syntax`) — lexer, recovering per-item parser, AST (literals keep verbatim text), stable semantic addressing
- ✅ **SpecTherm** (`thermite-spec`) — the frozen bounded-combinator registry + the cage validator (no anonymous nested quantifiers; closure bodies are flat predicates)
- ✅ **Lowering** (`thermite-lower`) — Thermite → Verus (L3), Kani harnesses (L2), executable Rust + always-active runtime checks (L1); compile-time effect-row subsumption
- ✅ **Forge** (`forge`) — `check` (parse → validate → effect-check → lower → Verus, per-item certificate with content-addressed proof caching), `build` (→ `rustc` → a runnable binary whose contract checks fire at runtime, with an **fx-derived seccomp sandbox**), `audit` (the trust manifest + enumerable TCB), `review` (pluggable spec-intent slot), `repair` (background L1/L2 → L3 upgrades). Automatic **L3 → L2 → L1 degrade**; a counterexample never degrades and is never "repaired."
- ✅ **Anti-Goodhart battery** — structural vacuity triage, solver tautology/unsat-precondition checks, mutation scoring (kill-ratio floor), strengthening probes
- ✅ **Boundaries** — crates.io FFI + `#[slag]` modules, L1-enforced and runtime-confined to their declared `fx`; the manifest distinguishes *verified-to-the-boundary* from *verified, period*; a caller verifies **through** a boundary's contract (composition)
- ✅ **`THERMITE.skill.md`** — the whole language in ≤ 6,000 tokens, regenerated from the registry, CI-gated; concurrency-safe multi-agent sessions
- ✅ **Self-verification** (`thermite-verified`) — the soundness-critical pure core is itself **Verus-verified** (`verus --no-cheating`, no `assume`/`external_body` on the core): effect subsumption, the degrade anti-cheat (a counterexample never degrades), the seccomp allowlist (pure → no I/O + monotonicity), the boundary honesty gate (a regular fn is never laundered to L3), project-level aggregation (no over-claim), and the mutation 0/0 floor. Each is anchored to its production consumer by exhaustive/observable equivalence. The toolchain shrinks its own TCB.
- ✅ **Universal verified primitive basis** (`.design/basis/`, Stages 1–8) — the surface grew from bounded slice algorithms to a *composable* basis, each stage certifying **L3 end-to-end** in real Verus:
  1. **Recursive ADTs** — `struct` (with `inv` type-invariants), `enum`, recursive types via `Box<T>`, and exhaustive `match` (a missed variant is a compile-time reject — the "handled-or-loud" tooth).
  2. **Recursion schemes** — `fold`/`map`/`for_all`/`exists` over the ADTs, with the `fold_bound` *prove-once* induction law: an instance proves its bound by citing the law, no fresh induction.
  3. **Effect-primitive stdlib** — each effect atom (`read`/`write`/`net`/`time`/…) a contracted, seccomp-confined `#[boundary]` primitive; outcome-coverage forces every `Result` arm handled.
  4. **Bounded collections** — `Vec<T>` over verified vstd, with a no-OOB `get` (`req i < len`) and a capacity-preserving `push`.
  5. **Compositional reasoning** — a caller verifies *through* its callees' contracts (no re-proof); project assurance aggregates as the honest **min** over parts; the TCB is enumerable.
  6. **Security-by-construction (IFC)** — `Tainted`/`Secret`/`Authorized` marked types + `#[sealed]` clean types whose **only** producer is their `#[boundary]` door (`parameterize`/`declassify`/`authorize`). SQL injection, secret leaks, and missing-authorization are **un-typeable — the careless path does not compile.**
  7. **Strings** — `String` as a bounded owned run of `u8` bytes (the verified `Vec<u8>` shape), with a borrowed `&str` view (`Ref` of `String`); `len`/`byte_at`/`slice`/`concat` are ordinary method calls and `==`/`+` ordinary binary ops; constructing or concatenating carries `fx alloc`.
  8. **The runnable effect link** — `forge build` lowers a verified item to executable Rust + always-active runtime contract checks and shells out to `rustc` for a **native binary that RUNS and does real I/O**; its declared `fx` row is enforced at runtime by an `fx`-derived **seccomp syscall sandbox** (code that exceeds its effects is killed at the syscall boundary), closing the loop from proof to a running, effect-confined executable.
- ✅ **Primitive-completeness campaign** (`.design/basis/`, clusters C1–C9) — the general-purpose surface, each cluster grounded in real Verus + critic-audited: **C1** literals (`\x1b`/hex/char), **C2** integer operators (`% << >> & | ^ !`, partiality proven — div/shift-by-zero is L0, not UB), **C3** `break`/`continue` (the invariant holds at `continue`, can't launder termination), **C4** `u64`↔`String` (the digit round-trip `parse_be(to_string(n)) == n` proven), **C5** string search/transform (`split → Vec<String>`, `find → Option`, `contains`/`starts_with`/`trim`), **C6** `Vec` completeness (`pop`/`insert`/`remove`/`contains` + non-Copy `Vec<String>`/`Vec<struct>` via a borrow-`get`), **C7** built-in `Option`/`Result` + payload-in-contract (`ens match result { Some(v) => … }`) + `parse_u64`, **C8/C9** plain-`fn` recursion (a `fn` `dec` measure, termination-proven; mutual recursion cleanly L0-rejected) and tuples (`(T, U)` + `.N` projection).
- ✅ **Acceptance programs** ([`examples/`](examples/)) — the compose-any-program proof: all four certify **L3** *and* `forge build` into a standalone binary that runs. A **verified multi-line interactive text editor** (`examples/editor/` — the editing, line-navigation, cursor-layout, and keystroke-decode logic all L3, only the raw `read`/`write`/`ioctl`/`open` syscalls trusted; `forge build examples/editor/editor.th --entry run --out ./nano` then run `./nano` directly — it self-sets raw mode, runs **under the seccomp sandbox** via a dedicated `fx term` terminal-control effect that grants exactly the termios `ioctl`, no script), a `u64`→decimal **formatter** (the digit round-trip proven), a **calculator** (`parse_u64` + arithmetic), and a line/CSV **parser** (`split` → `Vec<String>`). See [`examples/README.md`](examples/README.md).
- ✅ **Ergonomics + completeness clusters** (C10–C12) — tuple destructuring (`let (x, y) = e`), `for`-loops (auto-`dec`), match guards, or-patterns, `if let`/`while let` (sugar over the shipped `while`/`match`); **mutual recursion** (a dec'd cycle certifies L3 via Verus's mutual-decreases group); and **`Map<K, V>`** (a bounded verified key-value collection). Each grounded in real Verus + critic-audited.
- ✅ **The confined editor + the honest §7 gate** — the verified editor runs **under the seccomp sandbox** via a dedicated `fx term` terminal-control effect (the Verus-proved effect-subsumption bitset widened to carry it, #106); the mutation gate now **excludes provably-equivalent mutants** from the kill-ratio (a per-survivor Verus equivalence proof — sound-but-incomplete, never laundering a weak contract, #101).
- 🔭 Deferred (tracked): direct MIR-level lowering + the incremental goal-state REPL (crosslink #21); and the basis v1.1 layer (dataflow taint-propagation, `Vec` element-invariants, scheme fusion).

Roadmap (all shipped): v0.1 kernel → v0.2 Kani-backed L2 + degrade protocol → v0.3 mutation/vacuity battery → v0.4 crates.io FFI boundary → v0.5 background proof-repair + multi-agent sessions, plus `forge build` (runnable binaries) and the runtime seccomp sandbox. Progress is tracked in crosslink (milestones #1–#5 closed).

## Repository layout

| Path | What |
|---|---|
| `thermite-design.md` | The design document — the product thesis and the v0.1–v0.5 roadmap |
| `goal.md` | The binding contract for autonomous work (the ACToR loop + anti-drift rules) |
| `conformance/` | Golden `.th` programs + expected certificates — the verification oracle |
| `examples/` | Runnable example programs (the verified text editor, formatter, calculator, parser) + how to `forge build` + run each |
| `.design/` | Per-component design docs (the contract between the thesis and the code) |
| `tooling/` | The spec-discipline + anti-pattern gates and the route table |
| `.claude/agents/` | The four ACToR sub-agents (`acto-doc-author`/`builder`/`critic`/`fixer`) — auto-discovered by Claude Code (see below) |
| `thermite-*/`, `forge/` | The toolchain crates: `thermite-syntax`, `thermite-spec`, `thermite-lower`, `forge` (the CLI), `thermite-skill` |

## A taste of the surface language

```thermite
fn sum(xs: &[u32]) -> u64
  req xs.len() <= 1_000_000
  ens result == spec_sum(xs)
  fx  pure
{
  let mut acc: u64 = 0;
  let mut i: usize = 0;
  while i < xs.len()
    inv acc == spec_sum(&xs[..i])
    dec xs.len() - i
  {
    acc = acc + xs[i] as u64;   // overflow: discharged from the invariant + req
    i = i + 1;
  }
  acc
}
```

`forge check` turns this into a certificate: `L3`, contract non-vacuous, mutants killed — the deliverable's trust statement. `forge build --entry sum` compiles it to a native binary whose contract checks fire at runtime and whose `fx pure` is seccomp-enforced.

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
