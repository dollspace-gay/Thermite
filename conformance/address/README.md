# Address-resolution oracle (`conformance/address/`)

The hand-derived expected semantic addresses for the corpus programs — the
external anchor for `thermite-syntax/src/address.rs` (issue #3), per
`.design/syntax/semantic-addressing.md`.

## Contract

Each `<name>.addresses.json` lists, for `conformance/<name>.th`:

- `addresses[]` — every valid address in the program, in document order. For
  `inv`/`dec` nodes, `text` is the source text the address must resolve to.
- `must_error[]` — address strings that MUST resolve to a structured error
  (never a panic — R-CODE-2), e.g. out-of-range ordinals or unknown names.

The address scheme (1-based, source-order, structural/positional within the
enclosing item) is defined in `.design/syntax/semantic-addressing.md`. The
`inv#2`/`inv#3` resolutions encode the resolution of blocker #26: **`inv#2` is
`forall_below`, `inv#3` is `forall_from`** (source order; the thesis §4.3
`forge edit` example mislabels this and is an erratum).

## R-CHAR-3

Expected values here are derived from `thermite-design.md` §4.3 + the verbatim
`.th` source. They are NEVER copied from `address.rs`'s output. A test that
asserts `address.rs` agrees with itself is itself a divergence.
