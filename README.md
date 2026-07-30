# Thermite

Thermite is a programming language for code that must explain why it is
correct. Functions state:

- `req`: what callers must establish;
- `ens`: what the function guarantees;
- `fx`: what the function may affect.

Forge checks those contracts, returns concrete counterexamples when they fail,
and records the result in an assurance manifest. The long-term goal is simple:
compose a small set of known primitives in known-safe ways.

Thermite is experimental research software. The complete build and proof stack
is currently tested on x86-64 Linux, including Ubuntu and WSL2. The Rust crates
may build elsewhere, but the Verus distribution, Lean/CVC5 bridge, and
seccomp-backed runtime path are Linux-oriented.

## A small example

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
    acc = acc + xs[i] as u64;
    i = i + 1;
  }
  acc
}
```

`forge check` proves the contract or reports why it could not. `forge build`
lowers the program to Rust with runtime contract checks; hosted executables are
confined by an `fx`-derived seccomp filter unless the sandbox is explicitly
disabled.

## Install

There are two useful installation levels:

1. A Rust-only build compiles Forge and the supporting crates.
2. The full proof stack also installs Verus, Lean, Z3, and the pinned
   reconstruction tools. Use this if you want meaningful `forge check`, the
   complete test suite, or the trust-chain audit.

The commands below match the versions used by CI. Allow several gigabytes of
disk for Rust, Verus, Lean, Mathlib, and solver caches.

### 1. System packages

On Ubuntu or Debian:

```sh
sudo apt-get update
sudo apt-get install --yes \
  build-essential ca-certificates clang curl git \
  libc++-dev libc++abi-dev python3 unzip util-linux z3
```

What these provide:

| Package | Used for |
|---|---|
| `git`, `curl`, `ca-certificates`, `unzip` | Fetching pinned Rust, Lean, Verus, CVC5, and solver artifacts |
| `build-essential`, `clang`, `libc++-dev`, `libc++abi-dev` | Building the Stage 4 SAT tools and Lean's CVC5 bridge |
| `python3` | Repository gates and proof-artifact checks |
| `util-linux` | `prlimit`, used by the memory-bounded G4 gate |
| `z3` | Normal reconstruction and G4 checks; Verus also ships its matching private Z3 |

You do not need a system CVC5 package. Lean's pinned dependency downloads the
matching CVC5 distribution and builds its bridge.

### 2. Rust

Install rustup if necessary:

```sh
curl --proto '=https' --tlsv1.2 -sSf \
  https://sh.rustup.rs -o /tmp/rustup-init.sh
sh /tmp/rustup-init.sh -y
. "$HOME/.cargo/env"
```

The repository's [`rust-toolchain.toml`](rust-toolchain.toml) selects Rust
1.95.0 and installs `rustfmt` and Clippy automatically.

For a Rust-only build:

```sh
cargo build --release -p forge
```

To put Forge on your path:

```sh
mkdir -p "$HOME/.local/bin"
install -m 0755 target/release/forge "$HOME/.local/bin/forge"
export PATH="$HOME/.local/bin:$PATH"
```

Add that `PATH` export to your shell profile if `~/.local/bin` is not already
present.

### 3. Verus

Thermite's L3 path invokes `verus` from `PATH`. Install the exact CI release:

```sh
VERUS_VERSION="0.2026.05.24.ecee80a"
VERUS_ROOT="$HOME/.local/share/thermite/verus-$VERUS_VERSION"

mkdir -p "$VERUS_ROOT" "$HOME/.local/bin"
curl -fsSL -o /tmp/verus.zip \
  "https://github.com/verus-lang/verus/releases/download/release/$VERUS_VERSION/verus-$VERUS_VERSION-x86-linux.zip"
unzip -q /tmp/verus.zip -d "$VERUS_ROOT"
VERUS_BIN="$(find "$VERUS_ROOT" -type f -name verus -print -quit)"
test -n "$VERUS_BIN"
ln -sf "$VERUS_BIN" "$HOME/.local/bin/verus"
verus --version
```

Keep the Verus directory intact: its binary expects the bundled Rust toolchain
and Z3 beside it.

### 4. Lean and the proof spine

Install elan, then let the repository select Lean 4.29.0:

```sh
curl -fsSL \
  https://raw.githubusercontent.com/leanprover/elan/master/elan-init.sh \
  -o /tmp/elan-init.sh
bash /tmp/elan-init.sh -y --default-toolchain none
export PATH="$HOME/.elan/bin:$PATH"

cd lean
lake exe cache get
lake build
cd ..
```

`lake exe cache get` is important: without the prebuilt Mathlib cache, a first
build can spend hours rebuilding dependencies.

### 5. Checked BV and EPR reconstruction

Normal release builds include fixed-width `@bvN` syntax and automatic routing.
The finite relation/array reconstruction path additionally needs the exact
CaDiCaL and `drat-trim` revisions pinned in
[`scripts/g4-toolchain.env`](scripts/g4-toolchain.env):

```sh
bash scripts/install-g4-tools.sh
```

They are installed under `target/g4-tools`. Forge finds that directory
automatically when run from this checkout. The G4 gate also exports explicit
paths before it runs:

```sh
bash scripts/g4-gate.sh
```

The gate applies a 6 GiB address-space ceiling and serializes the expensive
work, so it is suitable for smaller development machines.

### 6. Optional: Kani for explicit L2 checks

Kani is only needed for `forge check --level l2` and the live Kani tests:

```sh
cargo install --locked kani-verifier --version 0.67.0
cargo kani setup
```

Kani installs its own nightly Rust and CBMC toolchain. It is not required for a
normal L3/L4 check.

### Dependency check

After installing the full stack:

```sh
rustc --version
cargo --version
verus --version
lake --version
z3 --version
python3 --version

cargo build --release -p forge
forge check conformance/sum.th
```

Missing proof tools are errors in the proof-bearing gates; they are not treated
as successful skips.

## Everyday Forge commands

The complete method list is generated from the same registry the CLI parses:

```sh
forge check program.th
forge goal program.th item
forge fill program.th item.?0 'replacement code'
forge build program.th --entry main --out ./program
forge audit program.th
forge skill
```

Run `forge` with no arguments for the current full synopsis. The primary
workflow is:

1. Write the contract and leave a hole if the body is unfinished.
2. Run `forge check`.
3. Read the counterexample or open goal.
4. Use `goal`, `fill`, or `edit`, then check again.
5. Build only after every required obligation is discharged.

## Assurance levels

The level records refutation quality; each clause separately records its engine
and trust profile.

| Level | Meaning |
|---|---|
| **L4** | An admitted decidable route with checked reconstruction and concrete failures: nonlinear relaxation, fixed-width BV, or finite EPR relation/array clauses |
| **L3** | An all-input machine proof through Verus/Z3 or the Lean engine |
| **L2** | A bounded Kani/CBMC result with the bound recorded |
| **L1** | An always-active runtime contract check |
| **L0** | Body trusted by fiat through the explicit `#[slag]` escape hatch |

Plain `forge check` uses automatic routing, including eligible BV and EPR
reconstruction. A counterexample is a failure, never a downgrade. Timeouts and
unsupported shapes remain named outcomes rather than being laundered into a
proof.

For the detailed trust argument, see [RATIONALE.md](RATIONALE.md) and
[thermite-design.md](thermite-design.md).

## Generated skill and Claude Code

[`THERMITE.skill.md`](THERMITE.skill.md) is the generated language and Forge
reference. Its surface grammar comes from exhaustive compiler matches, its
combinators and methods come from registries, and CI keeps it below 6,000
estimated tokens.

Regenerate or check the canonical copy:

```sh
forge skill --write THERMITE.skill.md
forge skill --check THERMITE.skill.md
```

Install the matching Claude Code skill:

```sh
forge skill --claude --write "$HOME/.claude/skills/thermite/SKILL.md"
forge skill --claude --check "$HOME/.claude/skills/thermite/SKILL.md"
```

The `--claude` form adds the required frontmatter. It is generated from the
same content as the committed reference, so there is no hand-maintained Claude
copy to drift.

The tracked `.claude/agents/` files and repository gates support the ACToR
development workflow. Crosslink is optional issue/session infrastructure; it is
not a Thermite build dependency.

## Tests and audits

Focused development checks:

```sh
cargo test -p thermite-skill
cargo test -p forge
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
python3 tooling/doc-drift.py
tooling/reqs check
```

Run the Rust/control-plane gauntlet with:

```sh
make gauntlet
```

Proof-bearing gates:

```sh
bash scripts/g3-gate.sh
bash scripts/g4-gate.sh
make audit-fast
make audit
```

`make audit` re-derives the trust chain, including the Lean axiom probe,
translation-validation batteries, correspondence drift checks, and independent
Verus replay. A missing guarantee-bearing dependency makes the final result
inconclusive and nonzero.

On a memory-constrained machine, keep Rust compilation and test execution
serial:

```sh
CARGO_BUILD_JOBS=1 cargo test --workspace -- --test-threads=1
```

## Examples and repository map

Runnable examples live under [`examples/`](examples/):

- [`editor`](examples/editor/README.md)
- [`calculator`](examples/calculator/README.md)
- [`formatter`](examples/formatter/README.md)
- [`parser`](examples/parser/README.md)

Important paths:

| Path | Purpose |
|---|---|
| [`THERMITE.skill.md`](THERMITE.skill.md) | Generated language and Forge reference |
| [`thermite-design.md`](thermite-design.md) | Language and verification design |
| [`RATIONALE.md`](RATIONALE.md) | Plain-language trust rationale |
| [`goal.md`](goal.md) | Repository development and anti-drift contract |
| [`conformance/`](conformance/) | Hand-authored programs and certificate oracles |
| [`lean/`](lean/) | Lean proof spine and checked reconstruction |
| [`.design/`](.design/) | Component-level requirements and audit pins |
| [`tooling/`](tooling/) | Documentation, requirement, and anti-pattern gates |
| `thermite-*`, `forge/` | Compiler, verifier, translation validator, and CLI crates |

## License

Thermite is open source under the [MIT License](LICENSE).
