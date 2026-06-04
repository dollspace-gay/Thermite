# Parse oracle (`conformance/parse/`)

Hand-derived expected structural facts for parsing the corpus programs — the
external anchor for `thermite-syntax/src/parser.rs` (issue #3), per
`.design/syntax/parser.md`.

## Contract

Each `<name>.facts.json` describes parsing `<name>.th` at a
representation-agnostic level (counts and kinds the parser must produce, NOT a
specific AST encoding — so the oracle does not over-constrain the AST shape):

- `parses_ok` / `error_count` — whether the program parses cleanly.
- `items[]` — per top-level item: `name`, `kind` (`fn` / `spec fn`), `params`
  (name + type as written), `ret`, and for `fn`s the mandatory-clause counts
  (`req_count`, `ens_count`, `fx`) and `loops[]` (each with its `loop#N` addr,
  surface keyword, `inv_count`, `has_dec`). `spec fn`s carry `has_dec` and no
  contract clauses (§4.2).

`recover_per_item.th` + `.facts.json` pin **per-item recovery** (§4.3): the
first item is malformed (omits the mandatory `ens` clause — a parse error per
§4.1), and the parser must report that error yet still recover and parse the
well-formed second item (`recovered_items` / `recovered_item_facts`).

## R-CHAR-3

Expected values are derived from the verbatim `.th` source + `thermite-design.md`
§4. They are NEVER produced by `parser.rs`. The parser is the artifact under
test; this is the truth it is tested against. A builder implementing the parser
must MATCH these fixtures, never edit them to match its output.
