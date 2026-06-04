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

v0.1 lowers Thermite to **Verus-annotated Rust source** (transpile, then shell out to Verus/Z3), inheriting the borrow checker and the ecosystem. The gate **degrades, it never blocks**: on solver timeout it falls L3 → L2 → L1 and reports honestly. A **vacuity battery** (tautology / unsat-precondition / mutation-kill-ratio checks) runs inside the gate so the mandatory contract can't be gamed into triviality.

See [`thermite-design.md`](./thermite-design.md) for the full design (thesis, surface language, the Forge REPL, the ladder, the vacuity battery, `#[slag]`, FFI, roadmap).

## Status

**Early v0.1 (kernel) — under construction.** This repo is a toolchain being built in Rust, not yet a usable language.

- ✅ Verification harness + conformance corpus (the ACToR agent loop; golden `.th` programs as the cert oracle)
- ✅ Cargo workspace scaffold — five crates: `thermite-syntax`, `thermite-spec`, `thermite-lower`, `forge` (CLI), `thermite-skill`
- 🔭 In progress: `thermite-syntax` (lexer, recovering parser, AST, stable semantic addressing)
- ⬜ Next: SpecTherm combinator registry → lowering to Verus → `forge check` → skill generator

Roadmap: v0.1 kernel → v0.2 Kani-backed L2 + degrade protocol → v0.3 mutation/vacuity battery → v0.4 crates.io FFI boundary → v0.5 background proof-repair. Progress is tracked in crosslink (milestones #1–#5).

## Repository layout

| Path | What |
|---|---|
| `thermite-design.md` | The design document — the product thesis and the v0.1–v0.5 roadmap |
| `goal.md` | The binding contract for autonomous work (the ACToR loop + anti-drift rules) |
| `conformance/` | Golden `.th` programs + expected certificates — the verification oracle |
| `.design/` | Per-component design docs (the contract between the thesis and the code) |
| `tooling/` | The spec-discipline + anti-pattern gates and the route table |
| `thermite-*/`, `forge/` | The toolchain crates (scaffolded; implementation in progress) |

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

`forge check` turns this into a certificate: `L3`, contract non-vacuous, mutants killed 17/18 — the deliverable's trust statement.
