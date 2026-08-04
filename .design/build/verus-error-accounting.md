# Whole-crate Verus error-count accounting

<!--
tier: 3-component
status: shipped
audited-content-sha256: cd2818ed3657a5f5f7f538a8a946eebd33fc244efabde25d3a907480f8ca9042
decision: preserve verifier counts when present and represent frontend counts as unknown
issue: github:dollspace-gay/Thermite#111
governs:
  - forge/src/verified_build.rs
  - forge/src/verified_build/composition.rs
  - forge/tests/verus_error_accounting.rs
thesis-refs:
  - thermite-design.md §6
  - thermite-design.md §8
  - thermite-design.md §9
-->

## Summary

Strict whole-crate builds report a numeric Verus error count only when the
machine-readable `verification-results.errors` field is present. A normal
frontend rejection may set `success: false` and `encountered-vir-error: true`
while omitting `errors`; Forge records that count as unknown and omits the
`(errors=N)` suffix from the rejection diagnostic. It never fabricates a count
from a sentinel or by scraping human-readable compiler output.

This changes diagnostics only. A missing count can never authorize an artifact:
strict success still requires a successful process, `success: true`, and an
explicit machine-readable zero error count.

## Root cause

The pinned Verus frontend emits this summary for unsupported array literals
under `--no-vstd`:

```json
{
  "verification-results": {
    "encountered-error": true,
    "encountered-vir-error": true,
    "success": false
  }
}
```

The previous parser represented a missing summary or missing `errors` field as
`u64::MAX`. That internal sentinel leaked through the composition rejection
formatter as `errors=18446744073709551615`. It was not a real count and was
indistinguishable from an absurd but syntactically valid numeric result.

## Selected representation

`VerusEvidence.errors` is `Option<u64>` while retaining the serialized field
name `errors`:

- `Some(N)` means Verus supplied an unsigned `errors` value;
- `None` means the JSON was missing, malformed, or omitted the field.

Existing successful receipt evidence remains byte-shaped as `"errors": 0`:
Serde serializes `Some(0)` as the same JSON number, and old numeric evidence
deserializes as `Some(N)`. A missing field is accepted as `None` for defensive
backward parsing, but successful bundle validation still requires `Some(0)`.

Text diagnostics are not a counting interface. A Rust/Verus diagnostic can
contain child errors, repeated rendered messages, JSON diagnostics, or a final
“aborting due to” line. Counting `error:` prefixes would couple receipt meaning
to presentation details and localization. Forge therefore preserves stderr for
the operator but makes no numeric claim when the structured field is absent.

## Decision table

| Process and summary | Evidence | Build result | Diagnostic count |
|---|---:|---|---|
| exit 0, `success: true`, `errors: 0` | `Some(0)` | accept if every other strict gate passes | none |
| nonzero or `success: false`, `errors: N` | `Some(N)` | reject | `(errors=N)` |
| nonzero or `success: false`, no `errors` | `None` | reject | omitted |
| exit 0 and `success: true`, no `errors` | `None` | reject closed | omitted |
| missing or malformed JSON | `None` | reject closed | omitted |

The same formatter and acceptance predicate serve ordinary and composition
builds, preventing the two paths from drifting.

## Acceptance

- Unit coverage parses zero, a positive count, a frontend summary without a
  count, and malformed output without ever producing a sentinel.
- Unit coverage preserves `(errors=N)` for known counts and omits the suffix for
  unknown counts.
- The exact issue reproduction reaches the pinned Verus frontend, reports the
  array-literal rejection without `u64::MAX` or any fabricated numeric suffix,
  exits nonzero, and publishes no bundle.
- Existing verified-build, composition, receipt validation, and replay tests
  remain green with successful evidence encoded as numeric zero.

## Requirements

<!-- generated:reqs view=forge-verus-error-accounting-status -->
Source: `.design/reqs/registry.toml`

| ID | Status | Owner | Title | Follow-up |
|---|---|---|---|---|
| REQ-VERUSERR-1 | shipped | `.design/build/verus-error-accounting.md` | Structured optional Verus error counts |  |
| REQ-VERUSERR-2 | shipped | `.design/build/verus-error-accounting.md` | No fabricated frontend diagnostic count |  |
| REQ-VERUSERR-3 | shipped | `.design/build/verus-error-accounting.md` | Unknown counts remain fail-closed |  |
<!-- /generated:reqs -->
