# SpecTherm combinator oracle (`tests/golden/combinators/`)

The hand-derived external anchor for `thermite-spec` (issue #2), per
`.design/spec/spectherm-combinators.md`. Referenced by the
`thermite-spec/src/combinators.rs` route in `tooling/spec-routes.toml`.

## Files

- **`registry.json`** — the FROZEN v0.1 SpecTherm combinator set (name, arity,
  arg-kinds, result). The registry the validator consumes. SMT triggers and
  Verus(L3)/executable(L1) forms are deferred to issue #4.
- **`accept.json`** — Thermite programs that PARSE and the validator must
  ACCEPT (valid SpecTherm). Covers all 8 combinators in contract positions +
  a spec-fn call in a contract.
- **`reject.json`** — Thermite programs that PARSE but the validator must
  REJECT, each with the expected `SpecError` cause (unknown combinator, wrong
  arity, wrong arg-kind). These step outside the §4.2 cage.

## R-CHAR-3

Every expected value here is derived from `thermite-design.md` §4.2 + the
verbatim corpus — never from `thermite-spec`'s own output. The validator is
the artifact under test; this is the truth it is tested against. A builder
implementing the validator must MATCH these fixtures, never edit them to fit
its output.

## Note on the reject `expected` variant names

`reject.json`'s `expected` strings mirror the `SpecError` variants named in
`.design/spec/spectherm-combinators.md` REQ-4. The builder MAY choose
different variant identifiers, but the REJECT outcome and the documented cause
must hold; the conformance test should assert rejection + the cause, not a
brittle exact string if the design doc and code agree on a rename.
