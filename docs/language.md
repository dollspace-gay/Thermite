# Language

## Three promises

Every function carries three promises, enforced as syntax. Leaving one out is a
compile error:

- `req` — what must hold before the function is called (e.g. "the list is sorted").
- `ens` — what the result guarantees (e.g. "if it returns an index, the element is there").
- `fx` — what the function may touch (e.g. "nothing — pure", or "may read this file").

Loops add `inv` (the loop invariant) and `dec` (the termination measure).

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

## The specification language

Contract-position expressions stay inside a small fragment: a fixed set of
bounded quantifier combinators with frozen SMT triggers, and no raw `forall`.
The fragment is small enough to make the machine-checked soundness proof
([Verification](verification.md)) feasible, and to keep the whole surface
teachable to a model within a fixed token budget. `THERMITE.skill.md` is the
generated, budget-checked language reference.

## How a function is written

The workflow is incremental. Declare the contract with a hole where the body
goes (`?0`, a typed hole). `forge goal` shows what is given and what must hold;
`forge fill` puts code in the hole and re-checks, returning a counterexample on
failure. Repeat until `forge check` reports `ALL GOALS DISCHARGED ✓ certified
L3`. An item with an unfilled hole cannot be built or certified.

`forge build` compiles a certified program to a native binary, with the
`fx`-derived syscall cage enabled ([Trust](trust.md)).
