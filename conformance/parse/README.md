# Parser fixtures

Each `<name>.facts.json` file records representation-independent facts about a
corresponding Thermite program. The fixtures test
`thermite-syntax/src/parser.rs` without fixing the parser's internal AST layout.

Recorded facts include:

- whether the program parses and how many diagnostics it produces;
- top-level item names, kinds, parameters, and return types;
- `req`, `ens`, and `fx` clause counts for functions;
- loop addresses, surface forms, invariant counts, and `dec` presence;
- `dec` presence for specification functions.

`recover_per_item.th` checks parser recovery. Its first item is malformed, while
the second is valid. The parser must report the first error and still recover
the second item.

Expected values are derived from the source text and `thermite-design.md` §4.
They are not generated from the parser under test (goal.md R-CHAR-3).
