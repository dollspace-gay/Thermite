# Verified `u64` decimal formatter

[`format.th`](format.th) wraps the verified `u64::to_string` primitive. Its
contract states that parsing the emitted decimal bytes returns the original
number.

## Verified behavior

```thermite
fn format(n: u64) -> String
  req true
  ens parse_be(result) == n
  ens result.len() >= 1
  fx  alloc
{ n.to_string() }
```

`forge check` certifies `format` at L3 for every `u64`. The round-trip
postcondition catches an omitted or incorrect digit, while the length
postcondition rules out an empty representation of zero.

```sh
cargo run -q -p forge -- check examples/formatter/format.th
```

## Run it

```sh
cargo run -q -p forge -- build examples/formatter/format.th --entry format_42 --out ./fmt
./fmt
```

Other entries exercise zero and a longer value:

```sh
cargo run -q -p forge -- build examples/formatter/format.th --entry format_0
cargo run -q -p forge -- build examples/formatter/format.th --entry format_1000000
```

The executable prints the underlying byte representation:

```text
format_42()      = TString { data: [52, 50] }
format_0()       = TString { data: [48] }
format_1000000() = TString { data: [49, 48, 48, 48, 48, 48, 48] }
```

The implementation builds digits in least-significant-first order and reverses
them for display. The zero-argument entry functions supply fixed inputs because
the deterministic runner does not synthesize `String` or `u64` arguments.

Certification and executable behavior are tested in
`forge/tests/acceptance_programs.rs`.
