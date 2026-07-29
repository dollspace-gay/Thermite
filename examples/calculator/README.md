# Verified calculator

[`calc.th`](calc.th) combines the verified decimal parser with
`Option<u64>`, contract-position pattern matching, and checked addition.

## Verified behavior

```thermite
fn add(a: String, b: String) -> Option<u64>
  req all_digits(a) && a.len() >= 1 && parse_be(a) <= 9223372036854775807
   && all_digits(b) && b.len() >= 1 && parse_be(b) <= 9223372036854775807
  ens result is Some
  ens match result { Some(v) => v == parse_be(a) + parse_be(b), None => true }
  fx  pure
{
  match parse_u64(a) {
    Some(x) => match parse_u64(b) { Some(y) => Some(x + y), None => None },
    None => None,
  }
}
```

`forge check` certifies `add` at L3. For valid in-range decimal inputs, the
result is `Some(parse_be(a) + parse_be(b))`. Returning `None` or the wrong sum
violates the postcondition.

```sh
cargo run -q -p forge -- check examples/calculator/calc.th
```

## Run the arithmetic demo

```sh
cargo run -q -p forge -- build examples/calculator/calc.th --entry add_2_3 --out ./calc
./calc
```

Available zero-argument entries:

```text
add_2_3()     = Some(5)
add_100_200() = Some(300)
```

## Current build limitation

Building the full string-parsing entry currently fails because the L1 runtime
lowerer does not emit all of the C7 parsing helpers:

```text
error[E0425]: cannot find function `all_digits` in this scope
error[E0425]: cannot find function `parse_be` in this scope
error[E0425]: cannot find function `parse_u64` in this scope
```

This affects `forge build`, which turns contracts into runtime checks. It does
not affect the L3 proof. The missing work is the L1 implementation of the C7
specification functions described in
`.design/basis/09-option-result.md` and `.design/basis/07-strings.md`.

The behavior is covered in `forge/tests/acceptance_programs.rs` by the
calculator certification, runnable-core, and missing-L1-helper tests.
