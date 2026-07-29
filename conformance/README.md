# Conformance corpus

The conformance corpus provides hand-authored inputs and expected results for
the Thermite toolchain. Tests compare implementation output with these fixtures;
they do not regenerate the expected values from the code under test.

## Layout

The root contains representative `.th` programs. Depending on the component,
their expected results live in one of several forms:

- `<name>.cert.json` records the stable subset of a `forge check` certificate.
- `parse/*.facts.json` records parser-level structure.
- `address/*.addresses.json` records semantic addresses and error cases.
- Subdirectories such as `build/`, `mutation/`, and `sandbox/` contain focused
  JSON case sets.
- `tests/golden/lower/` contains expected Verus or L1 lowering output.

## Fixture rules

Expected values come from the language and component designs, or are derived by
hand from the source program. They must not be copied from the implementation's
current output (goal.md R-CHAR-3).

Golden certificates contain deterministic fields only. Measurements such as
`solver_time_ms` are excluded. A fixture may describe a field before its
producer ships; tests begin comparing that field when the component is
implemented.

Not every source program has every kind of fixture. For example,
`binary_search.th` is useful to the parser and lowerer without a committed
certificate for every toolchain stage.

## Consumers

- `thermite-syntax` uses source programs, parse facts, and address fixtures.
- `thermite-spec` uses focused accept/reject case sets.
- `thermite-lower` compares generated code with the golden lowerings.
- `forge` compares certificates and component behavior with the relevant JSON
  fixtures.

When behavior changes intentionally, update the governing design first and then
update the fixture from that design. A failing test should not be fixed by
copying newly emitted output into the oracle.
