# Verified line and CSV parser

[`parse_lines.th`](parse_lines.th) combines the verified string `split`
operation, `Vec<String>`, and the `contains` predicate.

## Verified behavior

```thermite
fn fields(s: String, sep: u64) -> Vec<String>
  req true
  ens result.len() == 1 + count_sep(s, sep)
  fx  alloc
{ s.split(sep) }

fn has_sep(s: &String, sep: &String) -> bool
  req true
  ens result == contains_sub(s, sep)
  fx  pure
{ s.contains(sep) }
```

`has_sep` certifies at L3 through the normal `forge check` path. The emitted
Verus lowering for `fields` also verifies at L3 (`7 verified, 0 errors`).
Because `fields` delegates directly to `split`, it has no scoreable scalar body
mutant; its proof is checked at the lowering level.

```sh
cargo run -q -p forge -- check examples/parser/parse_lines.th
```

## Run the split demo

```sh
cargo run -q -p forge -- build examples/parser/parse_lines.th --entry split_abc --out ./parse
./parse
```

The result is a three-element `TVecTString` containing `a`, `b`, and `c`.
`split_abc` uses the runtime-checkable postcondition `result.len() >= 1`.

## Current build limitation

Building the complete program currently fails because two C5 specification
helpers lack L1 runtime forms:

```text
error[E0425]: cannot find function `count_sep` in this scope
error[E0425]: cannot find function `contains_sub` in this scope
```

The `split` and `contains` methods already have runnable implementations. The
missing work is limited to runtime forms for contract helpers such as
`count_sep`, `sep_free`, `occurs_at`, and `contains_sub`, described in
`.design/basis/07-strings.md`.

The certification, runnable core, and build limitation are covered in
`forge/tests/acceptance_programs.rs`.
