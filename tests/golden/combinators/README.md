# SpecTherm combinator fixtures

These files are the external fixtures for `thermite-spec`, governed by
`.design/spec/spectherm-combinators.md`.

- `registry.json` defines the v0.1 combinator names, arities, argument kinds,
  and result kinds consumed by the validator.
- `accept.json` contains valid contract expressions covering all eight
  combinators and a specification-function call.
- `reject.json` contains parseable programs that the validator must reject,
  together with the expected error category.

The expected values are derived from `thermite-design.md` §4.2 and the fixture
programs. They are not generated from `thermite-spec` output (goal.md
R-CHAR-3).

The error names in `reject.json` follow the variants in the current design.
Tests should check the documented cause rather than a brittle string when an
intentional variant rename keeps the same behavior.
