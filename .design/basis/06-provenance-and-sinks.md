# Provenance & Sinks — security-by-construction via information-flow control (Basis Stage 6)
<!--
tier: 3-component
status: draft
governs: thermite-spec/src/validator.rs
governs: thermite-syntax/src/ast.rs
governs: thermite-lower/src/lower.rs
thesis-refs:
  - thermite-design.md §1
  - thermite-design.md §4.1
  - thermite-design.md §6
  - thermite-design.md §9
-->

## Summary

Stage 6 of the universal-verified-basis buildout (crosslink epic **#62**) makes
whole CLASSES of security bug **un-typeable**: the careless path does not compile.
It is **security-by-construction** through **information-flow control (IFC)** —
ONE mechanism instantiated on three axes (taint / secret / capability). The
mechanism is: a **marked TYPE** (Stage 1 `Tainted<T>` / `Secret<T>` /
`Authorized<A>` wrappers) + **flow rules** the validator propagates and enforces
+ **doors** (the only mark-changing operations), where the doors are the audited,
greppable security TCB. A tainted value reaching a SINK (SQL/shell/path/HTML/
net-target/log) without passing a declared sanitizer, a `Secret` reaching a
public output without an audited `declassify`, or a protected op called without
its `Authorized` capability is a **compile-time SCREAM** — the loudest tooth of
the toolchain's handled-or-loud law. The SQL-injection program does not compile.

This doc is **GREENFIELD / FORWARD-LOOKING.** There is no IFC mechanism anywhere
in the toolchain today — no `Tainted`/`Secret`/`Authorized` type, no
`parameterize`/`declassify`/`authorize` door, no taint-propagation pass in the
validator (`grep -r "Tainted\|Secret\|Authorized\|declassify\|sanitize"` over the
`.rs` tree returns NONE). **Every REQ below is NOT-STARTED**, tracked under epic
**#62** (no separate blocker is filed — #62 owns this stage; a gap that needs an
independent blocker is noted with a fresh `#`). Stage 6 BUILDS ON, and invents
none of, four substrates: the Stage-1 marked wrapper TYPES (`01-adts.md`
REQ-1/REQ-2/REQ-8 — a newtype/phantom-tag struct carrying its value), the Stage-3
effect-primitive SINKS (`03-effect-stdlib.md` — the SQL/shell/file/net/log
`#[boundary]` primitives whose `req` demands the clean type), the Stage-5
composition law (`05-composition.md` — marks compose through the call graph), and
the audit-manifest door enumeration (`audit-manifest.md` #15 — the doors extend
the TCB).

## The model — IFC, one mechanism, three axes

Most of the security-CVE catalog reduces to ONE mechanism — a marked type whose
mark the validator propagates through dataflow, a set of forbidden flows the
validator REJECTS at compile time, and a small set of doors (the only operations
that change a mark), which are the audited security TCB. Three axes instantiate
it:

### Axis 1 — Integrity / TAINT (`Tainted<T>`)

Data from an untrusted source (user input, network, a file read) carries a
**taint mark**. The mark is a TYPE property: `Tainted<T>` is a Stage-1 wrapper
over the carried value (`01-adts.md` REQ-1 — a struct/newtype). A tainted value
**cannot reach a SINK** without first passing a declared **sanitizer door**. The
sink catalog (each sink's `req` demands a SANITIZED/clean type, never the
raw/tainted one):

| Sink (Stage-3 `#[boundary]` primitive) | Bug class killed | Sanitizer door (the clean-type producer) | Clean type the sink's `req` demands |
|---|---|---|---|
| SQL `query` | SQL injection (SQLi) | `parameterize(Tainted) -> Sql` | `Sql` (parameterized statement) |
| shell / `exec` | command injection | `shell_escape` / structured-args `-> Argv` | `Argv` (structured argument vector) |
| file `open` / path | path traversal | `validate_path(Tainted) -> SafePath` | `SafePath` (canonicalized, allow-rooted) |
| HTML / template output | XSS | `html_escape(Tainted) -> Html` | `Html` (entity-escaped) |
| net target / `connect` | SSRF | `allowlist_host(Tainted) -> Host` | `Host` (allow-listed target) |
| log / `print` / output | log/header injection, also the SECRET sink (Axis 2) | `html_escape` / `sanitize_log -> Clean` | the clean type (and no `Secret`, Axis 2) |

Also killed by the same mechanism: LDAP injection, HTTP-header injection,
unvalidated-deserialization (the deserialized value is `Tainted` until
validated). A tainted value reaching any of these sinks un-sanitized is a
**compile-time reject** (the un-typeable demo, GROUNDED below).

### Axis 2 — Confidentiality / SECRET (`Secret<T>`, the dual)

A secret (password, key, token) carries a **secret mark** (`Secret<T>`, the
Stage-1 dual wrapper). A secret **cannot reach a PUBLIC output** (a log, an error
message, a network response, stdout — the Stage-3 `Write`/`Net`/`print`
boundaries) without an explicit, **AUDITED `declassify` door**. A `Secret`
reaching a `Write`/`Net`/`print` boundary is the confidentiality flow to forbid
(`03-effect-stdlib.md` — the public-output sinks). Kills: logged passwords, keys
in responses, secrets in stack traces. The mark propagates the dual way: a
`Secret` combined with ANYTHING stays secret (a derived value of a secret is
secret), where taint propagates the integrity way (a value derived from tainted
data is tainted).

### Axis 3 — CAPABILITIES (`Authorized<Action>`)

A protected operation's `req` demands a **proof-carrying capability token**
(`Authorized<Action>`) that ONLY the auth check produces — the op is un-callable
without it. Kills: missing authorization, IDOR (insecure-direct-object-reference).
The capability is the dual of a sink: where a sink's `req` demands the *absence*
of a mark (clean, not tainted), a protected op's `req` demands the *presence* of
a mark (`Authorized`, only the `authorize` door produces it).

### The unifying law — handled-or-loud, the COMPILE-TIME tooth (the loudest)

This stage instantiates, in SECURITY, the toolchain's unifying law (the **#62**
design-refinement principle, stated in `01-adts.md` and `03-effect-stdlib.md`):
**for every outcome a program models it either HANDLES it (a proven/checked path)
or SCREAMS (an explicit, typed, greppable refusal); silently doing the wrong
thing is structurally impossible.** A forbidden flow is HANDLED (routed through a
door — `parameterize`/`declassify`/`authorize`) or it is a **compile-time
SCREAM** (the program does not type-check / does not validate). This is the
LOUDEST tooth (the same rung `01-adts.md` REQ-5/REQ-12 owns for exhaustive
`match`): the dangerous flow is caught *before the program ships*, not at runtime
and not at the syscall. The SQLi program does not compile (GROUNDED). The fiat
line is a KNOB: whatever flow you NAME (mark a source `Tainted`, a value
`Secret`, an op capability-gated) the validator forces handled-or-loud; the doors
you trust are NAMED in the manifest (the §9 TCB). `grep declassify` = every
secret-release; `grep parameterize`/`grep sanitize` = every taint-clearing — the
security TCB is grep-complete (§8).

## The door-as-audited-TCB honesty (the honest ceiling)

A door is a **trusted point**. You trust `html_escape` to actually escape, you
trust `validate_path` to actually canonicalize-and-root, you trust `declassify`
to be an intentional release. The language proves the data CAN'T reach the sink
un-doored; it TRUSTS the door does its job. That trust is made honest exactly the
way Stage 3 makes a syscall honest (`03-effect-stdlib.md` "the door-as-TCB" =
"the boundary-as-TCB"):

- **A door is a `#[boundary]`/`#[slag]` with a contract.** A sanitizer
  (`parameterize`, `html_escape`, `validate_path`, `allowlist_host`) is a
  `#[boundary]` fn whose contract STATES what it guarantees (e.g.
  `parameterize`'s `ens` that the result is a parameterized statement) and whose
  body is the trusted escaper. `declassify` and `authorize` likewise. The door's
  contract is L1-ENFORCED at the crossing (`ffi-boundary.md` REQ-4, the
  `lower_boundary_fn_l1` wrapper) — a door that violates its stated contract is
  caught at the boundary, not a free pass. This is the SAME legitimate-
  `external_body` distinction Stage 3 pins (`03-effect-stdlib.md` REQ-7,
  `boundary-composition.md` HONESTY ARGUMENT): the door is a declared trust
  boundary, NOT a `--no-cheating` core-logic cheat (R-DEFER-9).
- **Every door is enumerated in the audit manifest.** The doors are the security
  TCB — exactly where you trusted a sanitizer or released a secret. The
  `AuditManifest.tcb` `boundary_contracts` section (`audit-manifest.md` REQ-3,
  `Tcb::from_certificates`) enumerates each reached door: name + contract +
  foreign target + effect. `declassify` ESPECIALLY is audited — a skeptical third
  party reads every secret-release in the manifest in minutes (§1 trust
  relocation). This is the honest ceiling: not "no secret ever leaks" (you cannot
  prove the escaper), but "every secret-release passes a NAMED, contracted,
  enumerated door, and there are exactly THESE doors" (§9, the enumerable TCB).

The triple that makes a one-line door an honest TCB member is the Stage-3 triple
specialized to IFC: the door's guarantee is **stated** (its contract), the flow
through it is **typed** (the mark changes only at the door), and the door is
**enumerated** (the manifest names it). A door without a contract, or a
mark-change OUTSIDE a declared door, is the gap the validator (REQ-4) rejects.

## What is verus-checkable NOW vs the new validator-dataflow engine

This is the load-bearing honesty of this stage — be explicit about the line:

- **The SINK CONTRACT slice is verus-checkable NOW (GROUNDED below).** That a sink
  `fn query(q: Sql)` accepts ONLY a `parameterize`-produced `Sql`, that the
  `parameterize(Tainted) -> Sql` door is the only producer, and that a caller
  passing raw tainted input to the sink FAILS to type-check — this is a pure TYPE
  / `req` property, grounded in the real `verus` binary today (`3 verified, 0
  errors` for the doored path; the careless path is an `E0308` type error). The
  marked type + the sink's clean-type `req` + the door's `ens` is exactly the
  Stage-1-ADT + Stage-3-boundary + Stage-5-composition machinery, already
  GROUNDED in their docs and re-grounded here for IFC.
- **The MARK-PROPAGATION engine is NEW validator-dataflow work, NOT SMT.** That a
  tainted value flowing into a *derived* value STAYS tainted (`let y = f(x)` where
  `x: Tainted` makes `y` tainted), that a `Secret` *combined* with anything stays
  secret, that the mark propagates through assignment / function calls / ADT
  construction & destructuring / arithmetic — this is a DATAFLOW / type-
  propagation pass in `thermite-spec/src/validator.rs`, not a solver query. It is
  the CORE NEW WORK of this stage (more validator than SMT). The validator
  PROPAGATES the mark through the program and REJECTS the forbidden flows at the
  point a marked value reaches a sink/output/protected-op without passing a door.

The grounded slice proves the *shape* the validator-dataflow engine must produce
(a sink whose `req`/type only the door satisfies); the engine itself — tracking
which values carry which mark through the program — is the NOT-STARTED validator
work this doc pins.

## Requirements

### The marked types + the doors (governs `thermite-syntax/src/ast.rs`)

- **REQ-1 (the three marked types — `Tainted<T>` / `Secret<T>` /
  `Authorized<A>`):** the IFC mechanism is three Stage-1 marked wrapper types
  (`01-adts.md` REQ-1/REQ-2/REQ-8 — a newtype/phantom-tag struct over the carried
  value, the mark a TYPE property). `Tainted<T>` (integrity, untrusted source),
  `Secret<T>` (confidentiality, its dual), and `Authorized<A>` (a proof-carrying
  capability token). v1 is these THREE fixed axes — NOT a full lattice with
  arbitrary security levels (OUT, OQ-3). Derived from §1 (trust relocation: the
  mark is the legible trust statement) + `01-adts.md` REQ-1/REQ-2 (the wrapper
  types) + the **#62** IFC decision.

- **REQ-2 (the doors — the only mark-changing operations, each a contracted
  `#[boundary]`/`#[slag]`):** a mark changes ONLY through a declared door: the
  SANITIZERS (`parameterize`, `shell_escape`, `validate_path`, `html_escape`,
  `allowlist_host`, `sanitize_log` — `Tainted<T> -> Clean`), the `declassify` door
  (`Secret<T> -> Public`), and the `authorize` door (auth-check `-> Authorized<A>`,
  the ONLY `Authorized` producer). Each door is a `#[boundary]`/`#[slag]` fn with
  a contract (`FnItem.boundary.is_some() || FnItem.slag.is_some()` in `ast.rs`,
  the SHIPPED form), L1-enforced at the crossing (`ffi-boundary.md` REQ-4). No
  mark-change exists outside a door — a value's mark is fixed at construction and
  changeable only by passing a door. Derived from §9 (the boundary contract is the
  interface) + §8 (the door is greppable/enumerable) + `03-effect-stdlib.md` REQ-1
  (the door is the Stage-3 `#[boundary]` form specialized to a mark-change) +
  `boundary-composition.md` (the door's contract composes).

### The sink catalog + the flow rules (governs `thermite-syntax/src/ast.rs`,
`thermite-spec/src/validator.rs`)

- **REQ-3 (the sink catalog — every sink's `req` demands the CLEAN type):** each
  security sink is a Stage-3 effect-primitive `#[boundary]` (`03-effect-stdlib.md`)
  whose `req` (or parameter type) demands the SANITIZED/clean type, never the
  raw/tainted one: the SQL sink demands `Sql` (only `parameterize` produces it),
  the shell sink `Argv`, the path sink `SafePath`, the HTML sink `Html`, the net
  sink `Host`, the public-output sinks demand "not a `Secret`" (Axis 2). The
  protected-op sink (Axis 3) inverts it: its `req` demands the PRESENCE of
  `Authorized<A>` (only `authorize` produces it). The sink demanding the clean
  type is just a boundary contract the caller verifies THROUGH
  (`boundary-composition.md` REQ-1 — the sink's `req` is discharged at the call
  site, exactly as any boundary `req`). **GROUNDED** (the typed-sink slice
  below): a `query(s: Sql)` sink accepts only a `parameterize`-produced `Sql`;
  raw `Tainted<u64>` to `query` is a type error. Derived from §4.1 (the effect
  `req`/`ens` row) + §9 + `03-effect-stdlib.md` (the sinks are boundary
  primitives) + `boundary-composition.md` REQ-1.

- **REQ-4 (the validator mark-PROPAGATION + REJECTION engine — the core new
  work):** the validator (`thermite-spec/src/validator.rs`) PROPAGATES each mark
  through dataflow and REJECTS the forbidden flows at compile time. Propagation
  rules: a value derived from a `Tainted` value is `Tainted` (through assignment,
  function call return, ADT construction/destructuring, arithmetic, field/index
  access); a value combining a `Secret` is `Secret`; the mark is cleared/changed
  ONLY by a door (REQ-2). Rejection rules (the forbidden flows): a `Tainted` value
  reaching a sink (REQ-3) un-doored → `SpecError::TaintReachesSink { sink, span }`;
  a `Secret` reaching a public output un-`declassify`'d →
  `SpecError::SecretReachesPublic { sink, span }`; a protected op called without
  `Authorized` → `SpecError::MissingCapability { op, span }`. This is the
  DATAFLOW / type-propagation engine (more validator than SMT), the SHAPE of which
  the grounded sink contract proves but whose mark-through-the-program tracking is
  NEW. Derived from §4.1 (the validator enforces the row at compile time) + §2.4
  (crisp structured feedback) + `01-adts.md` REQ-5 (the validator's `SpecError`
  reject discipline) + the **#62** IFC-dataflow decision. **This REQ IS the
  compile-time tooth of handled-or-loud for security** (the loudest, §01-adts.md
  principle): a forbidden flow is rejected before the program ships, or it is
  routed through a door.

### Lowering + honesty (governs `thermite-lower/src/lower.rs`,
`forge/src/audit.rs` — via the SHIPPED #15 path)

- **REQ-5 (marks lower to Stage-1 wrapper types; doors lower to
  `external_body`):** a marked type lowers to its Stage-1 Verus wrapper
  (`01-adts.md` REQ-8/REQ-9 — a `struct`/`enum`); the sink's clean-type `req` and
  the door's `ens` lower to the existing Verus `requires`/`ensures`
  (`verus-lowering.md`), and the door (a `#[boundary]`/`#[slag]`) lowers to a
  `#[verifier::external_body]` signature woven into the caller's sub-program
  (`boundary-composition.md` REQ-1, `lower_external_body_fn in lower.rs`) — so the
  caller proves THROUGH the door's contract and the door's trusted body is never
  proved. **GROUNDED**: the typed-sink + door + caller pattern verifies `verus
  0.2026.05.24` `3 verified, 0 errors`. Derived from §3 (transpile to Verus) +
  `01-adts.md` REQ-8/REQ-9 + `boundary-composition.md` REQ-1 + the GROUNDED slice.

- **REQ-6 (the doors are the security TCB — enumerated in the audit manifest):**
  every door a program reaches is enumerated in the `AuditManifest.tcb`
  `boundary_contracts`/`slag_blocks` section (`audit-manifest.md` REQ-3,
  `Tcb::from_certificates in forge/src/audit.rs`) — name + contract + foreign
  target. `declassify` especially is audited: every secret-release is a named,
  contracted, enumerated door. A program with no door reaching a sink is verified
  end-to-end (no IFC flow); a program routing through doors is verified-to-the-
  boundary listing exactly the doors (`e2e-vs-boundary.md` #17, `05-composition.md`
  REQ-7). The manifest NEVER claims "no leak, period" — it claims "every flow
  passes THESE enumerated doors" (R-DEFER-9, honest enumeration of the entire
  fiat-trusted base). Derived from §1 (the auditable residue) + §9 (the enumerable
  TCB) + §8 (`grep declassify`/`grep sanitize` is the complete inventory) +
  `audit-manifest.md` REQ-3 + `05-composition.md` REQ-3/REQ-7.

- **REQ-7 (marks compose through the call graph — the Stage-5 hook):** a mark
  propagates through a multi-step call graph exactly as a contract composes
  (`05-composition.md` REQ-1/REQ-4): a caller `g` calling a sink `f` discharges
  `f`'s clean-type `req` from its own (doored) value's type, and a value's mark
  flows through the transitive closure the #52 weave already computes
  (`reachable_fn_deps in check.rs`). The whole-program honest-assurance statement
  (`05-composition.md`) holds: the verified pure core orchestrates the IFC doors
  (the world-interaction + trust-change surface), and the manifest aggregates the
  door TCB across the deep graph (`05-composition.md` REQ-7). Derived from §9 (the
  composition rule) + `05-composition.md` REQ-1/REQ-4/REQ-7 (marks compose like
  any contract) + the **#62** Stage-5 weaving.

## Acceptance criteria

ACs tie to a NEW `conformance/provenance/` oracle the ORCHESTRATOR authors (a
hand-derived cases file, the `conformance/composition/cases.json` /
`conformance/effect-stdlib/cases.json` precedents — R-CHAR-3, expected values
hand-derived from the flow rules + verus/type semantics, NEVER copied from
toolchain output). The CENTERPIECE is the un-typeable demo: a program that passes
user input to a SQL sink **fails to compile**, and the same program routed through
`parameterize` certifies.

- **AC-1 (the SQLi program does NOT compile — the centerpiece):** a corpus
  program `conformance/provenance/sqli.th` that reads user input as `Tainted<T>`
  and passes it RAW to the SQL sink `query(s: Sql)` is REJECTED — the validator
  emits `SpecError::TaintReachesSink { sink: "query" }` (and the lowered Verus is
  a TYPE error: `Tainted<T>` is not `Sql`). The SAME program routed through
  `parameterize(input)` first (`conformance/provenance/sqli_safe.th`) VALIDATES,
  lowers, and certifies `Level::L3` — running the real `verus` binary on the
  emitted output exits 0 with `N verified, 0 errors`. **GROUNDED** (`verus
  0.2026.05.24`): the doored path `3 verified, 0 errors`; the careless path is an
  `E0308` mismatched-types reject. Each is a compile-time handled-or-loud scream.
  (REQ-1, REQ-3, REQ-4, REQ-5.)

- **AC-2 (a `Secret` reaching `print` does NOT compile; declassified-then-printed
  does + shows in the manifest):** a program passing a `Secret<T>` to the
  `print`/log sink is REJECTED (`SpecError::SecretReachesPublic`); the same value
  passed through `declassify(secret)` first VALIDATES and certifies; and `forge
  audit` of the safe program enumerates `declassify` in the `tcb`
  `boundary_contracts` (REQ-6 — every secret-release is in the manifest).
  **GROUNDED**: the `declassify`-then-`emit` pattern verifies (part of the `6
  verified, 0 errors` secret/capability run); the direct-leak path is an `E0308`
  reject. (REQ-1, REQ-4, REQ-6.)

- **AC-3 (a protected op called without `Authorized` does NOT compile):** a
  program calling `protected_op` without the `Authorized<A>` capability is
  REJECTED (`SpecError::MissingCapability`); the same op called with
  `authorize(user)`'s token VALIDATES and certifies (the op's `req cap.ok`
  discharges from `authorize`'s `ens a.ok`). **GROUNDED**: the `authorize`-then-
  `protected_op` pattern verifies (part of the `6 verified, 0 errors` run). (REQ-1,
  REQ-3, REQ-4.)

- **AC-4 (mark propagation through a derived value rejects):** a tainted value
  flowed into a DERIVED value (`let y = f(x); query(y)` where `x: Tainted`) is
  REJECTED — the validator propagates the taint to `y` (REQ-4 propagation rule)
  and rejects `y` at the sink, even though `y` is not syntactically the tainted
  source. A `Secret` combined into a derived value stays secret and rejects at a
  public output. Hand-derived expectations (R-CHAR-3). This is the
  validator-dataflow engine's load-bearing behavior (not the contract slice).
  (REQ-4.)

- **AC-5 (the doors are enumerated as the security TCB):** `forge audit` of the
  doored programs (sqli_safe, the declassify program, the authorize program) emits
  an `AuditManifest` whose `tcb` enumerates `parameterize` / `declassify` /
  `authorize` (name + contract + target) as `boundary_contracts`; the pure logic
  appears as L3 + `to_boundary`; nothing fiat-trusted is omitted (R-DEFER-9).
  `grep declassify`/`grep parameterize` over the corpus = the manifest's door list.
  (REQ-2, REQ-6.)

- **AC-6 (the existing corpus is unaffected — no regression):** the existing pure
  corpus (`sum`, `binary_search`) and the prior stages' corpora certify
  IDENTICAL certs before and after Stage 6 — no marked type appears, no IFC flow
  is checked, byte-stable goldens. The IFC additions are purely additive (new
  `SpecError` variants, new marked-type wrappers, a new validator pass that is a
  no-op on mark-free programs). (All REQs; the security layer must not regress the
  kernel.)

## Architecture

Stage 6 owns ONE new mechanism — the validator mark-propagation/rejection engine
(REQ-4) — and otherwise instantiates SHIPPED machinery: the Stage-1 wrapper types,
the Stage-3 `#[boundary]` doors/sinks, the #52 compose-through, and the #15 door
enumeration. The component spans three crates, additively:

- **`thermite-syntax/src/ast.rs`** — the three marked types are Stage-1
  `struct`/`enum` wrappers (`01-adts.md` REQ-1/REQ-2); the doors are the SHIPPED
  `#[boundary]`/`#[slag]` form (`FnItem.boundary` / `FnItem.slag`, ALREADY in
  `ast.rs` per `struct BoundaryAttr`/`struct SlagAttr`). No new node SHAPE is
  required for the doors — they reuse the boundary surface. The marked types may
  need a phantom-tag generic; the v1 line is the three fixed wrappers (OQ-1).
- **`thermite-spec/src/validator.rs`** — the NEW mark-propagation/rejection pass
  (`pub fn validate` extended): collect the marked-type set, propagate the mark
  through the dataflow of each `fn` body (assignment / call / ADT
  construct-destruct / arithmetic / field-index), and reject the forbidden flows
  with the new `SpecError` variants (`TaintReachesSink` / `SecretReachesPublic` /
  `MissingCapability`). This is the CORE new work — a dataflow pass, NOT a solver
  query. The caged-flat walk (`spectherm-combinators.md` REQ-6) is UNCHANGED.
- **`thermite-lower/src/lower.rs`** — the marked types lower to their Stage-1
  wrappers (`01-adts.md` REQ-8/REQ-9, `lower_struct`/`lower_enum`); the doors lower
  to `#[verifier::external_body]` signatures via the SHIPPED `lower_external_body_fn`
  (`boundary-composition.md` REQ-1). No new emission SHAPE — the door is a boundary.
- **`forge/src/audit.rs`** — the doors are enumerated by the SHIPPED
  `Tcb::from_certificates` (`audit-manifest.md` REQ-3) — no change, the doors are
  boundaries the existing TCB enumeration already lists.

Symbol anchors: `struct BoundaryAttr` / `struct SlagAttr` / `enum Effect` in
`ast.rs` (the SHIPPED door substrate); `pub fn validate` in `validator.rs` (the
mark-propagation pass extends it); `lower_external_body_fn` / `lower_struct` in
`lower.rs`; `Tcb::from_certificates` in `audit.rs`.

### The verified typed-sink slice (GROUNDED — real `verus 0.2026.05.24`)

The CONTRACT slice of the taint→sink rejection — a sink whose type/`req` ONLY a
sanitizer-door-produced value satisfies — was run against the real `verus` binary
during authoring (scratch removed: no stray `*.rlib`, no `/tmp` leftovers). This
is the seed for the golden lowering; it proves the *shape* the validator-dataflow
engine (REQ-4) must produce.

**The SQL sink + the `parameterize` door + the safe caller (verified):**

```rust
// A taint mark is a TYPE property. `Sql` is the CLEAN sink-acceptable type;
// ONLY the `parameterize` door produces it.
pub struct Tainted<T> { pub raw: T }
pub struct Sql { pub q: u64 }

fn parameterize(t: Tainted<u64>) -> (s: Sql) ensures s.q == t.raw, { Sql { q: t.raw } }  // the DOOR
fn query(s: Sql) -> (r: u64) ensures r == s.q, { s.q }                                   // the SINK: req demands Sql

fn safe_path(input: Tainted<u64>) -> (r: u64) {     // routes user input THROUGH the door
    let clean = parameterize(input);
    query(clean)
}
```

```
verus ifc_ground.rs
verification results:: 3 verified, 0 errors      (exit 0)
```

**The SQLi program (the careless path) — does NOT compile (the un-typeable
demo):**

```rust
fn careless_path(input: Tainted<u64>) -> (r: u64) {
    query(input)   // pass raw tainted input straight to the sink, no door
}
```

```
error[E0308]: mismatched types
  --> ifc_neg.rs:18:11
   |    query(input)
   |          ^^^^^ expected `Sql`, found `Tainted<u64>`
error: aborting due to 1 previous error      (exit 1)
```

**RECORDED FINDING.** The careless SQLi path is rejected by the TYPE SYSTEM
before any proof runs — `Tainted<u64>` is not `Sql`, and only `parameterize`
produces `Sql`. This is the un-typeable demo at the contract level: the sink's
parameter type IS the flow rule. The doored path verifies. What the GROUNDED
slice does NOT show — and is the NEW validator work — is that a value DERIVED from
tainted input (`let y = munge(input); query(parameterize_lookalike(y))`) carries
the taint through the program; that mark-PROPAGATION (REQ-4) is the dataflow pass,
not this type-level slice. The slice proves the *destination* (the sink rejects
the mark); the engine proves the *journey* (the mark reaches the sink).

**The SECRET and CAPABILITY axes (verified — the same mechanism):** a
`declassify(Secret) -> Public` door + an `emit(Public)` public-output sink + a
safe caller, AND an `authorize(user) -> Authorized` door + a `protected_op(cap:
Authorized) req cap.ok` whose `req` demands the token, with safe callers for both:

```
verus ifc_secret_cap.rs
verification results:: 6 verified, 0 errors      (exit 0)
```

The negatives are type errors exactly as the SQLi case: `emit(secret)` (a `Secret`
straight to the public sink) is `E0308 expected Public, found Secret<u64>`. The
three axes are ONE mechanism — a marked type, a door, a sink whose `req`/type
encodes the flow rule — confirmed end to end against the real binary.

## Dependency hooks (the Stage 1 / 3 / 5 wiring)

- **Stage 1 (marked types — `01-adts.md`):** `Tainted<T>` / `Secret<T>` /
  `Authorized<A>` ARE Stage-1 wrapper ADTs (REQ-1/REQ-2 — a `struct`/newtype over
  the carried value; the mark a TYPE property). The marked type lowers via the
  Stage-1 `lower_struct`/`lower_enum` (REQ-8/REQ-9). Stage 6 cannot land before
  Stage 1 ships the wrapper types.
- **Stage 3 (the sinks — `03-effect-stdlib.md`):** the SINKS are the Stage-3
  effect-primitive `#[boundary]`s (the SQL/shell/file/net/log primitives); each
  sink's `req` demands the CLEAN type, never the raw/tainted one. A `Secret`
  reaching a `Write`/`Net`/`print` boundary is the confidentiality flow to forbid.
  The doors are ALSO Stage-3-form `#[boundary]`s (a sanitizer is a boundary whose
  contract states the escape). Stage 6 reuses the Stage-3 boundary form verbatim
  (REQ-2/REQ-3).
- **Stage 5 (marks compose — `05-composition.md`):** a mark composes through the
  call graph exactly as a contract composes (REQ-1/REQ-4 — the #52 weave); the
  door TCB aggregates across a deep graph (REQ-7). Stage 6's REQ-7 IS the IFC
  instantiation of the Stage-5 composition law: the sink's clean-type `req` is a
  boundary `req` the caller verifies through, and the marks flow through the same
  transitive closure (`reachable_fn_deps`).

## Verification

- **Mandatory Verus grounding (DONE during authoring — real `verus
  0.2026.05.24`, scratch removed).** The typed-sink CONTRACT slice — the `Sql`
  sink + `parameterize` door + safe caller (`3 verified, 0 errors`), the careless
  SQLi path (`E0308`, exit 1), and the secret/capability axes
  (`declassify`/`emit`, `authorize`/`protected_op` — `6 verified, 0 errors`; the
  direct-leak negatives `E0308`) — all verified against the real binary. This
  proves the typed-sink-rejects-tainted SHAPE is verus-feasible. The
  mark-PROPAGATION engine (REQ-4) is the NOT-STARTED validator-dataflow work the
  grounding does NOT cover (it is not an SMT property).
- **AC-1/AC-2/AC-3/AC-5:** `cargo test -p thermite-spec -p thermite-lower`, plus a
  harness shelling the real `verus` binary on the emitted lowering of the doored
  programs (assert exit 0 + `N verified, 0 errors`, R-CODE-4) and asserting the
  careless programs REJECT with the right `SpecError` variant + an `E0308`-class
  lowering type error, plus `forge audit` enumerating the doors in the TCB.
- **AC-4:** validator-dataflow reject fixtures (hand-derived expectations,
  R-CHAR-3) exercising mark propagation through a derived value.
- **AC-6:** the existing `conformance/sum`/`binary_search` certs stay byte-stable.
- **Gauntlet (R-DEFER-6, per crate):** `cargo test -p <crate>`, `cargo clippy -p
  <crate> --all-targets -- -D warnings`, `cargo fmt --check`, plus the conformance
  corpus.

## Routes to add (orchestrator)

This stage adds NEW concerns to files that already carry routes; the orchestrator
adds these routes to `tooling/spec-routes.toml` pointing at THIS doc (a file may
carry multiple governing docs — the #52 `lower.rs` precedent):

```toml
[[route]]  crate_pattern = "thermite-spec/src/validator.rs"  design = ".design/basis/06-provenance-and-sinks.md"  reference = ["conformance/provenance"]
[[route]]  crate_pattern = "thermite-syntax/src/ast.rs"       design = ".design/basis/06-provenance-and-sinks.md"  reference = ["conformance/provenance"]
[[route]]  crate_pattern = "thermite-lower/src/lower.rs"      design = ".design/basis/06-provenance-and-sinks.md"  reference = ["tests/golden/lower/sqli_safe.verus.rs"]
```

The orchestrator authors `conformance/provenance/cases.json` (the oracle this
doc's ACs cite), the `conformance/provenance/{sqli,sqli_safe,secret_leak,...}.th`
programs, their `.cert.json` goldens, and the `tests/golden/lower/sqli_safe.verus.rs`
golden (hand-authored from the GROUNDED slice, confirmed to pass `verus`), BEFORE
the builder runs (R-CHAR-3). This doc does NOT author the oracle, the goldens, or
the routes (R-DOC-1).

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (the three marked types — `Tainted`/`Secret`/`Authorized`) | NOT-STARTED | epic **#62** Stage 6. No `Tainted`/`Secret`/`Authorized` type anywhere in the tree (`grep -r "Tainted\|Secret\|Authorized"` over `.rs` returns NONE). Depends on Stage 1 wrapper types (`01-adts.md` REQ-1/REQ-2, NOT-STARTED). GROUNDED-feasible (`verus 0.2026.05.24`: the `Tainted<T>`/`Sql` slice `3 verified, 0 errors`), not implemented. |
| REQ-2 (the doors — only mark-changing ops, contracted `#[boundary]`/`#[slag]`) | NOT-STARTED | epic **#62** Stage 6. No `parameterize`/`declassify`/`authorize` door exists (`grep -r "declassify\|sanitize"` returns NONE). The SHIPPED door substrate (`struct BoundaryAttr`/`struct SlagAttr` in `ast.rs`, `FnItem.boundary`/`.slag`) is the form, but no door is declared against it. |
| REQ-3 (the sink catalog — every sink's `req` demands the CLEAN type) | NOT-STARTED | epic **#62** Stage 6. No security sink exists — depends on Stage 3 effect primitives (`03-effect-stdlib.md`, NOT-STARTED) whose `req` would demand the clean type. The sink-demands-clean-type SHAPE is GROUNDED (`query(s: Sql)` rejects raw `Tainted<u64>` with `E0308`; the doored path `3 verified, 0 errors`), not implemented. |
| REQ-4 (validator mark-PROPAGATION + REJECTION engine — the core new work) | NOT-STARTED | epic **#62** Stage 6. `thermite-spec/src/validator.rs` has no taint/secret/capability propagation pass and no `TaintReachesSink`/`SecretReachesPublic`/`MissingCapability` `SpecError` variant. This is the NEW dataflow engine (NOT SMT) — the grounded slice proves only the sink-contract SHAPE, not the mark-through-the-program tracking. Compile-time tooth of handled-or-loud for security. |
| REQ-5 (marks lower to Stage-1 wrappers; doors lower to `external_body`) | NOT-STARTED | epic **#62** Stage 6. `lower.rs` has no marked-type lowering. The mechanism is SHIPPED (`lower_external_body_fn` for the door, `boundary-composition.md` REQ-1; `lower_struct`/`lower_enum` for the wrapper, `01-adts.md` REQ-8/REQ-9 NOT-STARTED), but no marked type or door is lowered through it. GROUNDED (`3 verified, 0 errors`). |
| REQ-6 (the doors are the security TCB — enumerated in the manifest) | NOT-STARTED | epic **#62** Stage 6. The SHIPPED `Tcb::from_certificates in forge/src/audit.rs` (`audit-manifest.md` REQ-3) enumerates boundary contracts as the TCB, but no IFC door reaches it — there is no door to enumerate. `grep declassify`/`grep sanitize` = the door list once doors exist. |
| REQ-7 (marks compose through the call graph — the Stage-5 hook) | NOT-STARTED | epic **#62** Stage 6. The SHIPPED #52 compose-through (`reachable_fn_deps in check.rs`, `05-composition.md` REQ-1) + the #15 deep-graph TCB aggregation (`05-composition.md` REQ-7, NOT-STARTED) are the mechanism, but no marked value composes through a call graph — there is no sink/door program. Depends on REQ-3/REQ-4 + Stage 5. |

## Open questions (for the orchestrator before the builder runs)

- **OQ-1 (marked type as a phantom-tag generic vs three concrete wrappers — and
  the §10 skill budget).** REQ-1's marks could be a generic phantom-tag
  (`Marked<Tag, T>` with `Tainted`/`Secret`/`Authorized` as `Tag`s) or three
  concrete Stage-1 wrappers. The generic is fewer types but more surface; the
  three concrete wrappers fit the "one way" pillar (§2.3) and the closed-type-set
  discipline (§4.4). LEANING: three concrete wrappers (the v1 fixed-axis line,
  OQ-3). Either lowers identically. The §10 6k-token skill must hold the IFC
  grammar (the marks + the door verbs + the sink catalog) — a real budget check at
  Stage 6's skill regeneration (#7); the surface is small (three types + a handful
  of door verbs) and expected to fit, but §10 is the arbiter. Not a blocker.

- **OQ-2 (least-confident: the mark-propagation engine's REACH — implicit flows,
  marks through arithmetic/ADTs).** REQ-4 is the highest-judgment, least-confident
  part. The EXPLICIT-flow slice is clear (a tainted value passed to a sink). The
  open reach: (a) IMPLICIT flows — a `Secret` that influences a CONTROL path (`if
  secret > 0 { print("hi") }` leaks one bit) — v1 LEANS to tracking EXPLICIT
  data-flow only (the value reaching the sink), NOT implicit/control-flow leaks
  (a much harder non-interference property, noted as future work like
  constant-time below); (b) marks through ARITHMETIC and ADT
  construct/destructure — `Tainted(a) + b` is tainted, `match t { ... }` on a
  tainted scrutinee taints the bindings — these are tractable explicit-flow rules
  the engine must pin precisely; (c) where the v1 LINE is — explicit data-flow
  propagation through assignment/call/arith/ADT, rejecting at sinks, is v1;
  implicit/control-flow and full lattice IFC are OUT. The builder must pin the
  propagation rules mechanically (a fixture per rule, AC-4). This is the REQ I am
  LEAST confident is fully specified — the dataflow engine's exact reach is a real
  design call the builder refines against the corpus.

- **OQ-3 (the v1 fixed-axis line vs full lattice IFC — and the OUT-of-scope future
  axes).** v1 is the THREE FIXED axes (tainted/clean, secret/public, the
  capability set) — NOT a full lattice IFC with arbitrary user-defined security
  levels (OUT, harder and unneeded for the CVE catalog). Explicitly noted as OUT,
  do-not-build: (a) **constant-time crypto / side-channels** — a harder RELATIONAL
  property (non-interference over timing), a FUTURE axis, not v1; (b) **TOCTOU /
  concurrency** — out; (c) **full lattice IFC** — out, v1 is the three fixed axes.
  These are named so the builder does not over-reach; they are future work, not
  Stage-6 gaps. Not a blocker.

- **OQ-4 (the door's L1 enforcement vs the type-level guarantee — the honesty
  ceiling).** The language proves the data CAN'T reach the sink un-doored (a TYPE
  property, GROUNDED); it TRUSTS the door does its job (the escaper actually
  escapes). The door's contract is L1-enforced at the crossing (`ffi-boundary.md`
  REQ-4), but that L1 check verifies the door's STATED contract, which for a
  sanitizer is itself a trust statement (you cannot prove `html_escape` escapes
  all XSS vectors — that is the fiat the manifest enumerates, REQ-6). The open
  question: how strong is a sanitizer's STATED contract (a shape claim, like
  Stage-3's syscall contracts, or a stronger property)? LEANING: a shape claim
  ("the result is the `Html` clean type") + the door enumerated in the TCB — the
  same honest ceiling as Stage 3 (`03-effect-stdlib.md` REQ-3, outcome-coverage).
  The honesty is "every flow passes a NAMED door," not "the door is provably
  correct." Not a blocker; the builder pins the door-contract strength against the
  `conformance/provenance` oracle.
