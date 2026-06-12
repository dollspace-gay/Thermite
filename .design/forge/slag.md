# Forge `#[slag]` escape hatch

<!--
tier: 3-component
status: draft
audited-sha: 838374d1d8126c781789b48b2b876827a3bd3ab6 (bootstrap pin: decision 4 — doc-last-touch, NOT verified-current; backlog #262)
governs: forge/src/slag.rs
thesis-refs:
  - thermite-design.md §8
  - thermite-design.md §6
  - thermite-design.md §7
  - thermite-design.md §4.1
  - thermite-design.md Appendix A
-->

## Summary

`forge/src/slag.rs` implements the §8 escape hatch: the only sanctioned way to
ship a function whose body is NOT machine-proved. The parser already builds the
attribute node (`thermite_syntax::FnItem.slag: Option<SlagAttr { reason, owner,
review, span }>`, `ast.rs`); this component supplies the FORGE-side semantics
the parser deferred "downstream/forge":
(1) **validate** the three mandatory fields are present AND non-empty;
(2) **L3-exempt / L1-enforced** certification — a valid `#[slag]` item is NOT
sent to `verus`; it certifies at **L1** (the runtime-contract rung, §6) with
`slag: true` and its metadata in the certificate;
(3) it is the ONLY justification for a maximal `fx` row (the §7.1 (d)
interaction, `.design/forge/vacuity-triage.md`);
(4) it is VISIBLE in the audit surface (the certificate carries `slag: true` +
the reason/owner/review). The polarity inversion is the point (§8):
non-verification costs MORE keystrokes, metadata, and visibility — and slag
exempts you from *proving*, NEVER from *stating and checking* the contract.

GREENFIELD — no `slag.rs` exists; `manifest.rs` always emits `slag: false`
today (`certificate-manifest.md`). All REQs NOT-STARTED, blocked on crosslink
issue **#6** ("Implement structural vacuity triage and the `#[slag]` escape
hatch", milestone #1).

## Requirements

- **REQ-1 (mandatory-field validation — present AND non-empty):** a `#[slag]`
  item's `SlagAttr` must carry `reason`, `owner`, and `review` all present
  (`Option::Some`) and NON-EMPTY (the trimmed string is not `""`). The parser
  stores each field as `Option<String>` and explicitly leaves non-emptiness
  "downstream/forge" (`ast.rs` `SlagAttr` doc + grounded: an omitted field
  parses to `None`, `reason = ""` parses to `Some("")`). A missing field
  (`None`) or an empty/whitespace-only field → reject with a structured cause
  naming the offending field. No panic, no `unwrap` (`goal.md` R-CODE-2).
  Source: `thermite-design.md` §8 ("`reason`, `owner`, and `review` fields are
  mandatory and non-empty (checked)").
- **REQ-2 (slag semantics — L3-exempt, L1-enforced, `slag: true`):** a VALID
  `#[slag]` item is exempt from the L3 proof obligation: `forge check` does NOT
  invoke `verus` on it. Its contract is STILL mandatory and is enforced at **L1**
  (§6: "Runtime contract checks ... in every build profile"; §8: "enforced at L1
  (runtime) — slag exempts you from *proving*, never from *stating and
  checking*"). The certificate level is `Level::L1` (NOT `L3`, NOT skipped) with
  `slag: true`. This is the ONE place a v0.1 certificate carries `L1`: it is a
  deliberate down-rung, not a degrade (the L3→L2→L1 degrade ladder is #10).
  Source: `thermite-design.md` §8, §6.
- **REQ-3 (slag justifies a maximal `fx` row — the §7.1 (d) interaction):** slag
  is the ONLY thing that justifies a maximal effect row. The structural triage
  rule (d) (`.design/forge/vacuity-triage.md` REQ-4) rejects a maximal
  `EffectRow::Set` (all 8 `Effect` variant kinds) on a NON-slag item; on a
  `#[slag]` item (with valid fields, REQ-1) the maximal row is admissible. This
  component owns the "slag present ⇒ rule (d) skipped" half; `vacuity.rs` reads
  `FnItem.slag.is_some()` for it. A slag item is STILL subject to triage rules
  (a)/(b)/(c) — slag does not excuse a vacuous, result-omitting, or req-implied
  contract (§8; `goal.md` R-DEFER-9).
  Source: `thermite-design.md` §7.1 ("Effect row is maximal ... without
  `#[slag]` justification → reject"), §8.
- **REQ-4 (audit visibility — cert carries `slag: true` + metadata):** a slag
  item is visible in the certificate (the trust statement, §6): the existing
  `Certificate.slag: bool` is set `true`, and the certificate carries the slag
  metadata (`reason`/`owner`/`review`) so a reviewer can audit the fiat-trusted
  block. The frozen Appendix A certificate shows `slag: false` as a bare bool and
  does NOT enumerate the metadata fields; carrying the metadata is an ADDITIVE
  schema decision pinned here (OQ-1: a `Certificate.slag_meta:
  Option<SlagMeta>`, serialized only when present — a faithful superset of
  Appendix A, like `obligations`/`suggested_move` already are). `grep slag` over
  the codebase remains the complete inventory (§8); the full `forge audit`
  inventory (Appendix B) is a SEPARATE issue (#15 audit manifest v1) — OUT of
  scope here, noted as a boundary.
  Source: `thermite-design.md` §8 ("Every slag block appears in the build
  manifest ... `grep slag` over a codebase is the complete inventory"), §6;
  `.design/forge/certificate-manifest.md` (additive-schema convention).
- **REQ-5 (typed verdict; forge-check integration):** validation returns a
  structured result (`SlagVerdict`/`Result<SlagMeta, SlagError>`) consumed by
  `check::check_file`. The slag gate runs PER ITEM: if `FnItem.slag.is_some()`,
  validate fields (REQ-1) → on failure, reject (a contract-certification failure,
  the item does not certify); on success, run the structural triage rules
  (a)/(b)/(c) (`vacuity.rs`, REQ-3 here) → if those pass, SHORT-CIRCUIT the L3
  proof and emit an `L1` `slag: true` certificate (REQ-2/REQ-4). A non-slag item
  (`slag.is_none()`) is untouched by this component and proceeds to the normal L3
  path. No panic; errors are structured `ForgeError`/verdict values.
  Source: `thermite-design.md` §8, §7; `.design/forge/check.md` (the
  `check_file` pipeline this slots into).

## Acceptance criteria

ACs tie to a `conformance/slag/` oracle (authored by the orchestrator). Each
fixture is PARSE-VERIFIED below. Grammar limits hit while authoring (noted): no
`%` operator exists; `spec fn` requires a mandatory `dec`; effect rows use
COMMA separators (no `+`, no `fx *`); slag field values are double-quoted string
literals.

- **AC-1 (valid slag → L1, `slag: true`):** `conformance/slag/simd_sum.th` —
  §8's `simd_sum` with all three non-empty fields and a real contract — certifies
  `level == "L1"`, `slag == true`, and carries the reason/owner/review metadata;
  `verus` is NOT invoked for it. (Because a slag item certifies L1 by fiat, this
  AC is a LIVE cert assertion that does not require the `verus` binary — distinct
  from `sum`'s L3 path which does.)
- **AC-2 (invalid slag, empty field → reject):**
  `conformance/slag/empty_reason.th` — a `#[slag]` item with `reason = ""` →
  rejected with a cause naming `reason` (the item does NOT certify, NOT L1 and
  NOT L3). A `missing_owner.th` companion (`owner` field omitted entirely,
  parses to `None`) → rejected naming `owner`.
- **AC-3 (slag still subject to vacuity (a)/(b)/(c)):**
  `conformance/slag/slag_vacuous.th` — a valid-fielded `#[slag]` item with
  `ens true` → rejected by triage rule (a) (`.design/forge/vacuity-triage.md`),
  demonstrating slag exempts proving but not stating (REQ-3, §8).
- **AC-4 (slag justifies maximal `fx`):** `conformance/slag/maximal_fx.th` — a
  valid-fielded `#[slag]` item whose row is all 8 `Effect` kinds and whose `ens`
  mentions `result` non-trivially → certifies L1 with `slag: true` (passes (d)
  because slag is present; passes (a)/(b)/(c)). The non-slag counterpart is
  `conformance/vacuity/maximal_fx.th`, which is REJECTED (d).
- **AC-5 (field-validation unit coverage):** unit tests over `slag.rs`'s public
  validator assert: all-present-non-empty → `Ok`; each of `None` / `Some("")` /
  whitespace-only per field → the matching `SlagError`; expected verdicts trace
  to §8 (R-CHAR-3), not to `forge`'s output.

## Architecture

`slag.rs` is a new `mod slag;` in `forge/src/main.rs`/`lib.rs`, consumed by
`check.rs`. It imports `thermite_syntax::{FnItem, SlagAttr}` and the
`manifest::{Certificate, Level}` schema.

- `pub fn validate(slag: &SlagAttr) -> Result<SlagMeta, SlagError>` (REQ-1):
  checks each of `reason`/`owner`/`review` is `Some` and, after `trim`, non-empty;
  returns a `SlagMeta { reason, owner, review }` of validated owned strings, or a
  `SlagError::MissingField { field }` / `SlagError::EmptyField { field }`.
- `SlagMeta` is the validated metadata carried into the certificate (REQ-4).
- The slag CERT helper builds the `L1` `slag: true` certificate for a validated,
  triage-passing item (REQ-2): `Level::L1`, `effects` from the item's `fx` row
  (`manifest::effects_of`), `slag: true`, the metadata, and a single discharged
  obligation noting "contract enforced at L1 (slag); proof exempt by fiat" — NOT
  a `verus` obligation (no proof was run). The L1 RUNTIME-CHECK compilation of
  the contract is `thermite-lower`'s `l1.rs` job (`.design/lower/l1-runtime-checks.md`);
  this component records that the item's assurance IS L1, it does not generate
  the runtime checks.

**Forge-check integration (REQ-5, `.design/forge/check.md`).** In
`check::check_file`'s per-item loop, for an `Item::Fn` with `slag.is_some()`:

```text
slag::validate(slag)  ──Err──▶ reject (contract-certification failure; no cert L1/L3)
        │ Ok(meta)
        ▼
vacuity::triage(item) (rules a/b/c only; d skipped because slag present)
        │ Rejected ──▶ reject (slag exempts proving, not stating — §8)
        │ Passed
        ▼
emit L1 certificate { level: L1, slag: true, slag_meta: meta, effects } ── NO verus run
```

A non-slag `Item::Fn` skips this entirely: it runs `triage` (all four rules)
then the normal `lower → verus → L3` path (`check.md`). So slag and vacuity
COMPOSE: slag-validate first (it gates whether rule (d) is skipped), then triage,
then the L1-vs-L3 fork.

**Why L1, not L0 or "skipped" (REQ-2).** The frozen `Level` enum has
`L0 = #[slag]/unverified` (§6 table) AND `L1 = runtime contracts`. §8 is
explicit that a slag contract is "still mandatory and is enforced at L1
(runtime)" — so the slag item is L1 (its contract IS checked at runtime), not L0
(nothing checked) and not absent from the cert. The §6 ladder table lists
`L0 #[slag]` as the rung where "Nothing. Trusted by fiat" — that is the BODY's
proof status; the CONTRACT's enforcement is L1. The certificate records the
item's assurance as L1 with `slag: true`, which captures both facts: the
contract is runtime-enforced (L1) and the body is fiat-trusted (slag flag). This
reading is flagged OQ-2 in case the critic reads §6's `L0 #[slag]` row as
mandating `Level::L0`.

**Scope boundaries.** The full `forge audit` inventory (Appendix B —
slag ∪ boundary contracts ∪ assurance) is issue **#15** (audit manifest v1) —
OUT of scope; this component only sets the per-item certificate's `slag: true` +
metadata, which IS the `grep slag`-equivalent inventory at the cert level (§8).
The L1 runtime-check CODE generation is `thermite-lower` (`l1.rs`); CI policy
hooks that cap slag count / require second-party sign-off (§8) are a later
policy layer, not v0.1.

## Verification

- `cargo test -p forge` — unit tests over `slag::validate` (AC-5: present /
  missing / empty / whitespace per field) and the slag-certificate helper
  (AC-1/AC-4 cert shape: `Level::L1`, `slag == true`, metadata present).
  Expected verdicts/levels trace to `thermite-design.md` §8 and the
  `conformance/slag/` fixtures (R-CHAR-3), never to `forge`'s output.
- Conformance integration (`goal.md` model (B); the `conformance/slag` route
  reference): `forge check conformance/slag/simd_sum.th` → `L1`, `slag: true`
  (AC-1, no `verus` needed); `conformance/slag/empty_reason.th` → reject (AC-2);
  `conformance/slag/slag_vacuous.th` → reject by triage (AC-3);
  `conformance/slag/maximal_fx.th` → `L1`, `slag: true` (AC-4). The corpus
  `sum`/`binary_search` (non-slag) keep their L3 path.
- `cargo clippy -p forge --all-targets -- -D warnings`, `cargo fmt --check`,
  anti-pattern gate.

## Exact `conformance/slag/` fixture programs (PARSE-VERIFIED)

All parse clean under `thermite_syntax::parse` today (verified by direct probe);
grounded AST noted. The `simd_sum` body uses `0` as a placeholder (the body is
proof-exempt, so its content is irrelevant to slag certification); a real
`spec_sum` reference parses (it is an ordinary `Expr::Call`).

**`simd_sum.th`** — valid slag → L1 (AC-1). §8's example, adapted to the v0.1
grammar (the design comments are dropped; `u32::MAX as usize` parses as a
`Path`+`Cast`):
```thermite
#[slag(reason = "vendored SIMD intrinsics; contract checked at boundary by L1 wrapper",
       owner  = "agent:forge-7/session-2026-06-04",
       review = "required")]
fn simd_sum(xs: &[u32]) -> u64
  req xs.len() <= 1000000
  ens result == spec_sum(xs)
  fx  pure
{ 0 }
```
Grounded: `slag = Some(SlagAttr { reason: Some("vendored SIMD ..."), owner:
Some("agent:forge-7/session-2026-06-04"), review: Some("required") })`;
`ens#0.expr = Binary{Eq, Path(["result"]), Call{Path(["spec_sum"]),
[Path(["xs"])]}}` (mentions `result`, non-trivial). (Grammar note: `1_000_000`
also parses — digit separators are lexed — but `1000000` is used to avoid the
observation #37 separator caveat.)

**`empty_reason.th`** — invalid (empty `reason`) → reject (AC-2):
```thermite
#[slag(reason = "", owner = "agent:forge-7", review = "required")]
fn f(xs: &[u32]) -> u64
  req true
  ens result == 0
  fx  pure
{ 0 }
```
Grounded: `slag = Some(SlagAttr { reason: Some(""), owner: Some("agent:forge-7"),
review: Some("required") })` → `reason` present but empty → `SlagError::EmptyField
{ field: "reason" }`.

**`missing_owner.th`** — invalid (omitted `owner`) → reject (AC-2):
```thermite
#[slag(reason = "x", review = "required")]
fn f(xs: &[u32]) -> u64
  req true
  ens result == 0
  fx  pure
{ 0 }
```
Grounded: `slag = Some(SlagAttr { reason: Some("x"), owner: None, review:
Some("required") })` → `owner` is `None` → `SlagError::MissingField
{ field: "owner" }`.

**`slag_vacuous.th`** — valid fields but vacuous contract → reject by triage (a)
(AC-3):
```thermite
#[slag(reason = "x", owner = "y", review = "required")]
fn f(xs: &[u32]) -> u64
  req true
  ens true
  fx  pure
{ 0 }
```
Grounded: fields all `Some(non-empty)`; `ens#0.expr = BoolLit(true)` → passes
slag validation, then triage rule (a) rejects (`.design/forge/vacuity-triage.md`
REQ-1). Demonstrates slag exempts proving, not stating (§8).

**`maximal_fx.th`** — valid slag justifying a maximal row → L1 (AC-4):
```thermite
#[slag(reason = "vendored hardware path", owner = "agent:forge-7", review = "required")]
fn f(x: u32) -> u32
  req true
  ens result == x
  fx  read(a), write(b), net(c), alloc, time, rand, panic, diverge
{ x }
```
Grounded: `slag = Some(...)`; `fx = Set([Read("a"), Write("b"), Net("c"), Alloc,
Time, Rand, Panic, Diverge])` (all 8 kinds); `ens#0.expr = Binary{Eq,
Path(["result"]), Path(["x"])}`. Passes slag validation, triage (a)/(b)/(c), and
(d)-is-skipped (slag present) → L1, `slag: true`.

## Open questions

- **OQ-1 (cert metadata field):** Appendix A's certificate has `slag: bool`
  only — no reason/owner/review. Carrying the metadata (REQ-4) requires an
  ADDITIVE field, e.g. `Certificate.slag_meta: Option<SlagMeta>` serialized only
  when `slag == true` (a faithful Appendix A superset, mirroring how
  `obligations`/`suggested_move` were added in `certificate-manifest.md`). This
  is a design amendment to `certificate-manifest.md` (R-SPEC-2), NOT a
  code-local choice — flagged for the doc-author/critic to ratify before the
  builder adds the field. Alternative: carry the metadata only in the human/JSON
  audit rendering, leaving the cert struct at `slag: bool`; weaker for
  `forge audit` (#15). Default recommendation: the additive `slag_meta` field.
- **OQ-2 (L1 vs L0 for the slag rung):** §6's ladder table labels the `L0` row
  "`#[slag]` — Nothing. Trusted by fiat", while §8 says the slag contract "is
  enforced at L1 (runtime)". This doc resolves it as `Level::L1` + `slag: true`
  (the contract IS runtime-checked; the slag flag records the body is
  fiat-trusted). If the critic reads §6 as mandating `Level::L0` for a slag
  item, that is a thesis-level ambiguity (escalate per R-SPEC-4); the DECIDED
  scope here (slag certifies L1, not skipped, not L3) is the binding
  interpretation pending that escalation.
- **OQ-3 (does L1 certification need `thermite-lower`'s `l1.rs`?):** REQ-2 says
  a slag item's assurance IS L1 because its contract is runtime-enforced. Whether
  `forge check` must invoke `thermite-lower`'s L1 runtime-check generation
  (`l1.rs`) to EMIT those checks as part of slag certification, or whether #6
  only records the L1 LEVEL (with `l1.rs` generation wired later), is a
  sequencing question. Leaning: #6 records `Level::L1` + `slag: true` (the
  certificate-level fact); the actual runtime-check codegen is `l1.rs`'s
  existing responsibility, invoked by the build, not re-implemented here.

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (mandatory-field validation) | SHIPPED | `pub fn validate(&SlagAttr) -> Result<SlagMeta, SlagError>` in `slag.rs` (`validate_field`: `None` → `MissingField`, empty-after-`trim` → `EmptyField`); consumer `check::gate_fn`. Verified: `slag::tests::{all_present_non_empty_ok, empty_reason_rejected, missing_owner_rejected, whitespace_only_is_empty, validated_fields_are_trimmed, reason_checked_first}` + conformance `empty_reason`/`missing_owner`. |
| REQ-2 (L3-exempt / L1-enforced / `slag: true`) | SHIPPED | `Certificate::slag_l1` (manifest.rs) emits `Level::L1` + `slag: true` + a fiat-trusted obligation; `check::gate_fn` builds it WITHOUT invoking `lower`/`run_verus`. Verified: `slag_accepts_certify_l1_slag_true` (`simd_sum_l1`, `slag_justifies_maximal_fx` → L1, slag:true, no verus). |
| REQ-3 (slag justifies maximal `fx`) | SHIPPED | `vacuity::triage` skips rule (d) when `item.slag.is_some()`; `slag::validate` (run first in `gate_fn`) gates whether that skip is honored. Verified: `vacuity::tests::maximal_fx_with_slag_passes_d` + conformance `slag_justifies_maximal_fx` accept vs. non-slag `maximal_fx_no_slag` reject. |
| REQ-4 (audit visibility — cert metadata) | SHIPPED | `SlagMeta { reason, owner, review }` (manifest.rs) carried via the additive `Certificate.slag_meta: Option<SlagMeta>` (OQ-1 ratified — `skip_serializing_if`, golden still deserializes); `cli::render_human` prints it. Verified: `manifest::tests::slag_l1_cert_shape` + `slag_accepts` asserts `slag_meta` present. |
| REQ-5 (typed verdict + check integration) | SHIPPED | `SlagError` + `Result<SlagMeta, SlagError>`; `check::gate_fn` composes validate → triage(a/b/c) → `slag_l1` short-circuit; a `slag.is_none()` item takes the normal L3 path. Verified: the slag conformance tests + the corpus L3 tests (non-slag untouched). |
