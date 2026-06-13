# Trust

## The runtime cage

`fx` is enforced at runtime as well as statically. When you build a binary,
Thermite derives a syscall filter (seccomp-BPF, the kernel mechanism Docker and
Chrome use) from the declared effects. A function that declared `pure` and then
opens a network connection is killed by the OS mid-syscall. The static effect
check and the runtime cage come from the same `fx` declaration.

## Re-deriving the trust chain

`make audit` re-derives the chain on your machine:

```sh
make audit        # the full re-derivation (minutes)
make audit-fast   # a 60-second demonstration (one program, one injected bug)
```

The fast version: a correct program certifies; the same program with one
injected bug is refused with a counterexample; and the emitted proof re-verifies
under an independent copy of the prover with the Thermite tooling excluded.

The full version re-checks every link: your own Lean proof checker re-verifies
the central theorem, the translation cross-checks re-run over the test corpus
(thousands of live proof obligations), and the sabotage tests re-run to confirm
the prover catches each known class of translation bug. It then prints the list
of what remains trusted. A run with a skipped check prints `INCONCLUSIVE` and
exits nonzero.

## What remains trusted

After a clean run, five items remain trusted:

1. The Lean kernel and its three standard axioms (`propext`, `Classical.choice`,
   `Quot.sound`).
2. Z3/Verus soundness (a kernel-replay proof-of-concept already covers the
   scalar-linear fragment).
3. The gap between the formal specification and the author's intent — answered
   by reading the declarative spec layer, not closed by machine.
4. The pinned Rust↔Lean correspondence inspection.
5. rustc and LLVM.

Everything else is re-derived on your machine. A trust statement is only useful
if it enumerates its assumptions, which is what this list is.
