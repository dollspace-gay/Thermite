# Forge certificate / manifest schema

<!--
tier: 3-component
status: draft
audited-sha: dff9ae866e3437af272a62e078993e66c1116460 (re-audited 2026-06-12: amended — stale GREENFIELD line, oracle-comparison shape, dead parse_verus_output cite; additive-schema/engine_attribution amendment added, #262)
governs: forge/src/manifest.rs
thesis-refs:
  - thermite-design.md §5.1
  - thermite-design.md §5.3
  - thermite-design.md §6
  - thermite-design.md §7
  - thermite-design.md Appendix A
-->

## Summary

`forge/src/manifest.rs` defines the certificate schema — the STABLE, versioned
data contract that `forge check` emits (`thermite-design.md` §5.1, Appendix A).
It is the deliverable's trust statement (§6). This component owns the
`Certificate` struct, its serde serialization (`serde_json`), the per-obligation
result type, and the FULL field set from Appendix A — including the fields #5
does not yet populate (`contract_quality.*`) and the reserved `suggested_move`
slot — so that downstream issues (#6/#12/#13) FILL fields rather than reshape
the schema. Changing a field is a design amendment (R-SPEC-2/R-SPEC-3), not a
code-local choice.

This component is SHIPPED (`forge/src/manifest.rs`; all REQs SHIPPED — see the
REQ status table). The "two-speed schema" below has since EXECUTED and the
certificate gained a family of ADDITIVE fields — see the Amendment.

> **Amendment 2026-06-12 (doc-freshness re-audit, #262).** Re-verified against the
> current tree (`dff9ae86`, 12 post-pin commits to `manifest.rs`). Corrections and
> post-pin additive surface:
> 1. *The two-speed schema has executed.* The forward-declared producers landed:
>    #6 graduates the §7.1 triage bools (`Certificate::graduate_triage_clean`),
>    #13 sets them solver-confirmed `true` on a reject
>    (`Certificate::rejected_vacuity`), #12 graduates `mutants_killed`/`survivor`
>    (`Certificate::with_mutation_score`, `Certificate::rejected_weak_contract`),
>    and the REQ-4 reserved `suggested_move` is now CONSTRUCTED in production on a
>    timeout cert (`Certificate::timeout`, #11) and a strengthening-probe cert
>    (`Certificate::with_strengthening`, #14). `ContractQuality::forward_declared`
>    remains the honest pre-producer default; `contract_quality` remains
>    oracle-EXCLUDED.
> 2. *Additive schema surface (each governed by its OWN design doc, listed for
>    navigation — all `#[serde(default)]` and/or `skip_serializing_if`, so the
>    frozen golden `conformance/sum.cert.json` deserializes unchanged, R-SPEC-2):*
>    `slag_meta`/`reject` (#6 — `slag.md`/`vacuity-triage.md`), `cached` (#8 —
>    `proof-cache.md`), `solver_profile` (#11 — `solver-profiles.md`),
>    `lowered_assurance`/`degrade_reason` (#10 — `degrade-ladder.md`),
>    `strengthening` (#14 — `strengthening-probes.md`), `boundary`/
>    `boundary_target` (#16 — `.design/boundary/ffi-boundary.md`),
>    `assurance_scope` (#17 — `e2e-vs-boundary.md`), and `engine_attribution`
>    (#247 — `.design/verified/proof-backends.md` REQ-4): `pub engine_attribution:
>    Option<crate::engine::EngineAttribution>` with `#[serde(default,
>    skip_serializing_if = "Option::is_none")]`, set by
>    `Certificate::with_engine_attribution` ONLY when a non-default engine (Lean)
>    discharges (`check::lean_proven_cert`/`lean_interactive_proven_cert` on the
>    `--engine lean` path); the default Verus path leaves it `None`, goldens
>    byte-unchanged; oracle-EXCLUDED (diagnostic).
>    `manifest.rs` also gained the project-level aggregate
>    (`AssuranceManifest`/`ProjectAssurance`/`ProjectScope`/`FunctionAssurance`,
>    `pub fn cert_certifies`) — governed by `degrade-ladder.md` REQ-5/REQ-6 and
>    `e2e-vs-boundary.md` REQ-4, not re-specified here.
> 3. *Oracle-comparison shape corrected.* The shipped comparator is
>    `pub fn oracle_subset in manifest.rs` returning the tuple
>    `(&str, Level, &[String], bool, bool, bool)` = (`item`, `level`, `effects`,
>    `slag`, `boundary`, `end_to_end`) — `boundary` (#16) and the
>    `scope_is_end_to_end`-NORMALIZED `assurance_scope` bit (#17) JOINED the
>    subset; per-obligation outcomes are NOT part of the tuple (this doc's
>    original "+ per-obligation outcomes" wording was wrong of the shipped
>    comparator); `contract_quality.*` and `solver_time_ms` stay structurally
>    excluded.
> 4. *Dead symbol cite.* `check::parse_verus_output` no longer exists — the verus
>    output path is `check::classify_verus_outcome` (the #11 three-way
>    proved/timeout/counterexample classifier) feeding `check::assemble_certificate`,
>    which populates `obligations`. REQ-5's status row updated.

## Requirements

- REQ-1 (stable certificate schema, Appendix A): the `Certificate` struct
  mirrors the Appendix A certificate exactly — `item: String`, `level: Level`
  (an enum `L0`/`L1`/`L2`/`L3` serialized as `"L3"` etc., §6),
  `solver_time_ms: u64`, `contract_quality: ContractQuality`,
  `effects: Vec<String>`, `slag: bool`. Field names and JSON shape match
  Appendix A's excerpt byte-for-byte where present. This is a CONTRACT: a field
  add/rename/remove is a design amendment.
  Source: `thermite-design.md` Appendix A; `goal.md` R-SPEC-2 ("certificate
  fields ... match the design (§6, §7, §8, Appendix A). The certificate IS the
  deliverable; its shape is a contract").
- REQ-2 (which fields #5 produces NOW): in #5 the certificate carries real,
  derived values for `item`, `level`, `effects`, `slag`, and the per-obligation
  results. `item` is the checked item's name; `level` is L3 iff verus reports 0
  errors (`.design/forge/check.md` REQ-5); `effects` is the item's `fx` row
  (lowercased combinator-free strings, e.g. `["pure"]`); `slag` is `false` in
  #5 (`#[slag]` handling is issue #6 / §8).
  Source: `conformance/sum.cert.json` (`item`/`level`/`effects`/`slag` are the
  present, non-battery fields); `conformance/README.md` ("the cert it CAN
  produce").
- REQ-3 (forward-declared fields — defined, not stubbed): `contract_quality`
  (`tautology: bool`, `vacuous_precondition: bool`, `mutants_killed: String`,
  `survivor: Option<String>`) is part of the schema but is FORWARD-DECLARED:
  #5 does not run the vacuity battery (`tautology`/`vacuous_precondition`,
  issue #6/#13) or the mutation scorer (`mutants_killed`/`survivor`, issue #12).
  These fields are present in the type with honest #5 values — they are NOT
  asserted against the oracle in #5 and become LIVE assertions when their
  producing component lands. The schema reserves the slot; the value is filled
  later, never fabricated.
  Source: `conformance/README.md` ("Forward-declared fields ... compares only
  the fields the toolchain actually emits"); `thermite-design.md` §7 (battery
  produces these); `goal.md` scope (battery fields are #6/#12/#13).
- REQ-4 (`suggested_move` slot reserved, not stubbed): §5.1 reserves a
  `suggested_move` slot "populated by deterministic heuristics." In #5 it is
  DEFINED as an empty/`None` value (`Option<...>` serializing to `null`/omitted)
  — a reserved, honest absence, NOT a placeholder string and NOT a `todo!()`.
  The heuristic population (missing-invariant patterns, overflow-guard
  templates, trigger hints) is later work; the schema slot exists now so adding
  it is not a breaking change.
  Source: `thermite-design.md` §5.1 ("reserves a `suggested_move` slot").
- REQ-5 (per-obligation results, counterexamples): the certificate carries the
  per-obligation results that `check.rs` parses from verus (§5.1
  "per-obligation results"; "counterexamples, not adjectives"). Each result
  records the obligation identity, its source location, its status
  (discharged/failed), and — on failure — the concrete witness/diagnostic from
  verus, never a bare "failed" string. The exact field name(s) for this list
  are pinned here as the stable schema (see OQ-2 — `sum.cert.json` does not
  enumerate a per-obligation array, so this list's JSON key is a #5 schema
  decision recorded here).
  Source: `thermite-design.md` §5.1.
- REQ-6 (`solver_time_ms` excluded from the oracle): `solver_time_ms` is a
  schema field (Appendix A) but is NON-DETERMINISTIC (wall-clock solver timing,
  §5.3) and is EXCLUDED from the cert-oracle comparison. It is present in the
  emitted certificate but never asserted against a golden cert; the determinism
  contract (R-CODE-5) applies to every OTHER field.
  Source: `conformance/README.md` ("Deterministic subset only ...
  `solver_time_ms` ... is not asserted"); `thermite-design.md` §5.3.
- REQ-7 (serialization — serde_json, deterministic): `Certificate` derives
  `serde::Serialize`/`Deserialize`; `serde_json` is the serializer. Field
  ordering and formatting are stable/deterministic (R-CODE-5) so the JSON is
  bit-reproducible given identical inputs and seed. `Level` serializes as the
  string form (`"L3"`), matching the golden cert.
  Source: `goal.md` R-CODE-5, R-SPEC-3; `conformance/sum.cert.json` (the target
  JSON shape).

## Acceptance criteria

- AC-1 (schema matches Appendix A): a `cargo test -p forge` unit serializes a
  hand-built `Certificate` and asserts every Appendix A key is present with the
  documented type, and `Level::L3` serializes to the string `"L3"`. Expected
  keys/values trace to `thermite-design.md` Appendix A (R-CHAR-3), not to
  `forge`'s own output.
- AC-2 (deterministic subset round-trips the golden cert): the present,
  deterministic fields of `conformance/sum.cert.json`
  (`item`/`level`/`effects`/`slag`) deserialize into a `Certificate` and
  re-serialize to a value equal on those fields — proving the schema is a
  faithful superset of the golden cert and the comparison subset is
  well-defined.
- AC-3 (forward-declared fields excluded from the live oracle): the cert-oracle
  comparison used by `check.rs` ignores `contract_quality.*` and
  `solver_time_ms`; a test asserts that two certificates differing ONLY in those
  fields compare equal under the oracle's deterministic-subset comparison
  (REQ-3, REQ-6).
- AC-4 (`suggested_move` is a reserved absence): a freshly assembled #5
  certificate serializes with `suggested_move` as `null`/omitted (its `Option`
  is `None`), never a placeholder string and never via `todo!()`/`unimplemented!()`
  (anti-pattern gate, R-APG-1).
- AC-5 (per-obligation results present): a certificate for a verified item
  carries a non-empty per-obligation result list, each entry discharged; a
  certificate for a failed item carries at least one failed entry with a
  source-location-bearing diagnostic (the §5.1 counterexample payload), asserted
  against the `check.rs` broken-contract fixture (`.design/forge/check.md`
  AC-3).
- AC-6 (determinism): serializing the same `Certificate` twice yields
  byte-identical JSON (stable field order, R-CODE-5).

## Architecture

`manifest.rs` is data + serde, no I/O. The core type:

- `Certificate { item, level, solver_time_ms, contract_quality, effects, slag,
  obligations, suggested_move }` — the §5.1 / Appendix A schema. `level: Level`
  (enum L0–L3, §6, string-serialized). `contract_quality: ContractQuality`
  (REQ-3, forward-declared). `obligations: Vec<ObligationResult>` (REQ-5).
  `suggested_move: Option<SuggestedMove>` (REQ-4, reserved `None` in #5).
- `Level` — `L0`/`L1`/`L2`/`L3`, serializing to `"L0".."L3"` to match the
  golden cert's `"level": "L3"`.
- `ContractQuality { tautology, vacuous_precondition, mutants_killed,
  survivor }` — matches the Appendix A `contract_quality` object. In #5 these
  carry honest non-asserted values; they go LIVE when #6 (tautology/vacuity) and
  #12 (mutation) land. Excluded from the #5 oracle comparison.
- `ObligationResult` — per-obligation identity, source location, status, and an
  optional failure witness/diagnostic (the §5.1 "counterexamples, not
  adjectives" payload). `check.rs` (`.design/forge/check.md` REQ-4) populates
  this from verus's JSON summary + stderr spans.

**The two-speed schema.** The schema is fixed NOW at its full Appendix A shape;
the PRODUCERS arrive over several issues. `conformance/README.md` codifies this:
the golden `sum.cert.json` asserts fields no shipped component produces yet
(`mutants_killed: "17/18"`), and the cert-oracle compares only the fields the
toolchain emits, each becoming a live assertion as its producer lands. Defining
the full struct now (REQ-1/REQ-3/REQ-4) means #6/#12 FILL fields without a
schema migration — honoring R-SPEC-2 (the certificate shape is a contract;
changing a field is a design amendment).

**Determinism boundary (REQ-6).** Every field except `solver_time_ms` is
deterministic given the toolchain version + pinned seed (§5.3, R-CODE-5).
`solver_time_ms` is the lone wall-clock field and is structurally excluded from
the oracle comparison — present in the emitted cert (Appendix A shows it), never
asserted (`conformance/README.md`).

**The oracle comparison** (as amended 2026-06-12). This component defines the
deterministic-subset comparison the cert-oracle uses (`check.rs` AC-1 /
`goal.md` model (B)): the shipped `Certificate::oracle_subset` tuple is
(`item`, `level`, `effects`, `slag`, `boundary`, `end_to_end`) — `boundary`
(#16) and the normalized §9 scope bit (#17, `scope_is_end_to_end`: `None` ==
`Some(EndToEnd)`) are verdict-relevant and INCLUDED; per-obligation outcomes,
`contract_quality.*`, `solver_time_ms`, and every diagnostic additive field
(`slag_meta`, `boundary_target`, `solver_profile`, `degrade_reason`,
`strengthening`, `cached`, `engine_attribution`) are EXCLUDED. The comparator
lives here because it is a property OF the schema (which fields are
oracle-stable), consumed by `check.rs`'s conformance tests.

**Scope (OUT of #5).** Populating `contract_quality.tautology`/
`vacuous_precondition` (issue #6/#13), `mutants_killed`/`survivor` (#12), the
`suggested_move` heuristics (later), and `slag: true` cases (#6/§8) are NOT this
component's job in #5. The SCHEMA carries the slots; the VALUES come later.

## Verification

- `cargo test -p forge` (`tests/manifest.rs` + unit tests): schema-shape
  assertion against Appendix A (AC-1); deserialize/re-serialize the golden
  `sum.cert.json` deterministic subset (AC-2); forward-declared-field exclusion
  in the oracle comparator (AC-3); `suggested_move` reserved-absence (AC-4);
  per-obligation list present for pass/fail (AC-5); double-serialize byte
  equality (AC-6).
- Consumed by `check.rs`'s conformance integration: `forge check
  conformance/sum.th`'s cert deterministic subset == `sum.cert.json`
  (`.design/forge/check.md` AC-1).
- `cargo clippy -p forge --all-targets -- -D warnings`, `cargo fmt --check`,
  anti-pattern gate (no `todo!`/placeholder in the reserved slots).

Expected JSON keys/values trace to `thermite-design.md` Appendix A and
`conformance/sum.cert.json` — NEVER copied from `forge`'s own output
(R-CHAR-3).

## Open questions

- OQ-1: `mutants_killed` is typed `String` (`"17/18"`) to match the Appendix A
  golden cert verbatim, not a structured `{killed, total}`. Recorded as a
  schema decision; revisit when #12 lands the scorer (it produces the value, so
  the type may want amending then — a design amendment, not a code-local
  change).
- OQ-2: `sum.cert.json` does not enumerate a per-obligation array, so the JSON
  key and entry shape for `obligations` (REQ-5) is a #5 schema decision pinned
  in this doc rather than read off the golden cert. The golden cert asserts only
  the item-level summary fields; the per-obligation list is additive schema
  surface §5.1 mandates. Surfaced so the builder/critic treat the key name as a
  deliberate contract choice, not an accident.

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (stable schema, Appendix A) | SHIPPED | `struct Certificate { item, level, solver_time_ms, contract_quality, effects, slag, obligations, suggested_move }` in `manifest.rs` mirrors Appendix A field order; consumed by `check::assemble_certificate`. Test `schema_matches_appendix_a`. |
| REQ-2 (fields #5 produces now) | SHIPPED | `Certificate::new` + `fn effects_of` set real `item`/`level`/`effects`/`slag`/`obligations`; consumer `check::assemble_certificate`; live oracle `sum_cert_matches_golden_deterministic_subset` against `conformance/sum.cert.json`. |
| REQ-3 (forward-declared fields) | SHIPPED | `ContractQuality::forward_declared` returns honest unscored values (`mutants_killed="0/0"`, not the golden `"17/18"`); excluded from `Certificate::oracle_subset`. Since superseded IN PART: the producers landed (#6 `graduate_triage_clean`, #13 `rejected_vacuity`, #12 `with_mutation_score` — Amendment 2026-06-12 item 1). Test `oracle_ignores_forward_declared_and_time`. |
| REQ-4 (`suggested_move` reserved) | SHIPPED | `Certificate::new` sets `suggested_move: None` (serialized as omitted, not a placeholder); `struct SuggestedMove` reserves the slot — now CONSTRUCTED in production on timeout (#11 `Certificate::timeout`) and strengthening (#14 `with_strengthening`) certs (Amendment 2026-06-12 item 1). Test `suggested_move_is_reserved_absence`. |
| REQ-5 (per-obligation results) | SHIPPED | `struct ObligationResult { name, status, location, diagnostic }` + `enum ObligationStatus`; `obligations` field; populated by `check::assemble_certificate` from `check::classify_verus_outcome` (the former `parse_verus_output` cite is dead — Amendment 2026-06-12 item 4), rendered by `cli::render_human`. Test `obligation_results_present`. |
| REQ-6 (`solver_time_ms` excluded) | SHIPPED | `solver_time_ms: u64` present (`#[serde(default)]` so the golden subset deserializes); `Certificate::oracle_subset` omits it. Test `golden_deterministic_subset_round_trips`. |
| REQ-7 (serde_json serialization) | SHIPPED | `#[derive(Serialize, Deserialize)]`; `Level` → `"L0".."L3"`; serialized via `cli::run_check`'s `serde_json::to_string_pretty`; deterministic field order. Test `serialization_is_deterministic`. |
