# Thermite conformance corpus

This directory is the **cert oracle**: the external truth the `acto-critic`
anchors divergence claims to, in place of the upstream a translation fork
would have (see `goal.md` → "Why the critic still has teeth without an
upstream").

## Layout

```
conformance/
  <name>.th          a Thermite program (the input to `forge check`)
  <name>.cert.json   the GOLDEN certificate `forge check <name>.th` must emit
  README.md          this file
```

A future `tests/golden/lower/<name>.verus.rs` holds the golden **lowering**
(the exact Verus source `thermite-lower` must emit). Those are NOT authored
yet — see "Forward-declared" below.

## The cert-oracle contract

For each `<name>.th` with a golden `<name>.cert.json`, `forge check` must
emit a certificate whose fields **match the golden cert**, with two rules:

1. **Deterministic subset only.** Non-deterministic fields (e.g.
   `solver_time_ms`) are EXCLUDED from comparison. The design's Appendix A
   shows `solver_time_ms: 612` as illustrative; it is not asserted. Builds
   are bit-reproducible given a pinned seed (design §5.3), but wall-clock
   solver timing is not, so it never appears in a golden cert.

2. **Forward-declared fields.** A golden cert may assert a field that no
   shipped component produces yet (e.g. `mutants_killed`, produced by the
   mutation scorer, issue #12 / v0.3). Until that component ships, the
   cert-oracle compares only the fields the toolchain actually emits. Each
   field becomes a LIVE assertion when its producing component lands. The
   golden cert is the target; the toolchain grows into it.

Expected values trace to `thermite-design.md` or are hand-derived from it —
**never** copied from `forge`'s own output (`goal.md` R-CHAR-3). A test that
asserts the toolchain's output equals itself is itself a divergence.

## Current entries

| Program | Source | Golden cert | Provenance |
|---|---|---|---|
| `sum.th` | verbatim | `sum.cert.json` | `thermite-design.md` Appendix A (program + certificate excerpt) |
| `binary_search.th` | verbatim | — (not yet) | `thermite-design.md` §4.1 (program) |

`binary_search` has no golden cert yet: the design gives its program but not
a certificate, and its `mutants_killed` value is not specified. Authoring a
fabricated cert would create a false anchor (`goal.md` R-CHAR-3), so its
golden cert is deferred until the certificate-manifest contract
(`.design/forge/certificate-manifest.md`) and the mutation scorer (#12) pin
the missing fields. Until then `binary_search.th` is used as a parser /
lowering fixture only.

## Forward-declared (do not fabricate)

- `tests/golden/lower/<name>.verus.rs` — exact Verus lowering. Authored
  alongside `.design/lower/verus-lowering.md` (issue #4), not guessed now.
- Full `*.cert.json` schema — pinned by `.design/forge/certificate-manifest.md`.
- `binary_search.cert.json` — pending the above + mutation scorer (#12).

## How the corpus is consumed

- **thermite-syntax / parser (#3)** — `*.th` are parse fixtures (round-trip,
  per-item recovery, semantic addressing).
- **thermite-lower (#4)** — `*.th` lower to the golden Verus files (once
  authored).
- **forge check (#5)** — `forge check <name>.th` emits a cert compared to
  `<name>.cert.json` under the contract above. This is the gate referenced
  by `goal.md` R-DEFER-6.
