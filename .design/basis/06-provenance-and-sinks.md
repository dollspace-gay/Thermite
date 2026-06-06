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

Stage 6 of the universal-verified-basis buildout (crosslink epic **#62**, issue
**#76**) makes whole CLASSES of security bug **un-typeable**: the careless path
does not compile. It is **security-by-construction** through **information-flow
control (IFC)** — ONE mechanism instantiated on three axes (taint / secret /
capability). The mechanism is: a **marked TYPE** (a Stage-1 wrapper carrying the
value), **typed SINKS** (each demanding the clean/safe type by its parameter
type), and **DOORS** (the only mark-changing operations — the audited, greppable
security TCB). A tainted value reaching a SINK without a sanitizer door, a
`Secret` reaching a public output without an audited `declassify`, or a protected
op called without its `Authorized` capability is a **compile-time SCREAM** — the
loudest tooth of the toolchain's handled-or-loud law. The SQL-injection program
does not compile.

**v1 scope is PINNED (the buildable slice — GROUNDED end-to-end below):**

- **v1 = TYPE-LEVEL enforcement of the three axes at a DIRECT sink call.** The
  marked types are DISTINCT types from the clean types; a sink's parameter type
  demands the clean type; the only door from marked→clean is the declared door
  fn; so passing a raw marked value to a sink is a **TYPE MISMATCH** that the
  full toolchain path (parse → validate → effect-check → lower → verus) REJECTS.
  This slice is **EMERGENT from SHIPPED machinery** — Stage-1 ADTs (the wrapper
  structs, `01-adts.md` REQ-8 SHIPPED), the SHIPPED `#[boundary]` door/sink form
  (`ffi-boundary.md` REQ-2), and the existing lower→verus type-check. It needs
  **NO new validator code** for the direct-sink rejection (the type system is the
  flow rule). The work that remains is the conformance CORPUS + skill grammar +
  the marked-type/door VOCABULARY the skill teaches; the rejection mechanism is in
  place (GROUNDED: the SQLi careless path is L0/FAILED with `E0308`; the doored
  path certifies). REQ-1/REQ-2/REQ-3/REQ-5/REQ-6/REQ-7 are the v1 slice.
- **v1.1 = the DATAFLOW-PROPAGATION engine (REQ-4).** That a mark flows through
  INTERMEDIATE values (`let y = f(x)` where `x` is tainted makes `y` tainted),
  the dual secret-propagation (a value combining a `Secret` stays secret), and
  the reject at the point a *derived* marked value reaches a sink — this is the
  harder NEW validator-dataflow pass in `thermite-spec/src/validator.rs`. It is
  explicitly **v1.1**, NOT v1; the v1 type-level slice does not need it for the
  centerpiece demo (a direct `query(user_input)` is rejected by the type system
  alone — GROUNDED).

This doc is **FORWARD-LOOKING for the IFC vocabulary** (`grep -r
"Tainted\|Secret\|Authorized\|declassify\|sanitize"` over the `.rs` tree returns
NONE — no IFC type or door is declared in the toolchain or corpus today). **Every
REQ below is NOT-STARTED**, tracked under epic **#62** / issue **#76** (no separate
blocker is filed — **#76** owns this stage; a gap needing an independent blocker is
noted with a fresh `#`). But the v1 ENFORCEMENT mechanism is SHIPPED substrate,
re-grounded here against the real `forge`/`verus` binaries (full-path output
below). Stage 6 BUILDS ON, and invents none of, four substrates: the Stage-1
marked wrapper TYPES (`01-adts.md` REQ-1/REQ-8 — a newtype struct carrying its
value, SHIPPED through lowering), the SHIPPED `#[boundary]` SINK/door form
(`ffi-boundary.md` REQ-2; the Stage-3 effect-primitive sinks, `03-effect-stdlib.md`),
the Stage-5 composition law (`05-composition.md` — marks compose through the call
graph, SHIPPED), and the audit-manifest door enumeration (`audit-manifest.md` #15,
SHIPPED — the doors extend the TCB, GROUNDED below).

## The model — IFC, one mechanism, three axes

Most of the security-CVE catalog reduces to ONE mechanism — a marked type, a sink
whose parameter type demands the clean (or capability) type, and a small set of
doors (the only operations that change a mark), which are the audited security
TCB. Three axes instantiate it:

### Axis 1 — Integrity / TAINT (`Tainted`)

Data from an untrusted source (user input, network, a file read) carries a
**taint mark**. The mark is a TYPE property: `Tainted` is a Stage-1 wrapper over
the carried value (`01-adts.md` REQ-1 — a `struct`/newtype). A tainted value
**cannot reach a SINK** without first passing a declared **sanitizer door**. The
sink catalog (each sink's parameter type / `req` demands a SANITIZED/clean type,
never the raw/tainted one):

| Sink (`#[boundary]` primitive) | Bug class killed | Sanitizer door (the clean-type producer) | Clean type the sink demands |
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
**compile-time reject** (the un-typeable demo, GROUNDED below at the TOOLCHAIN
level — `query(input)` is `L0`/`FAILED` with `E0308`).

### Axis 2 — Confidentiality / SECRET (`Secret`, the dual)

A secret (password, key, token) carries a **secret mark** (`Secret`, the Stage-1
dual wrapper). A secret **cannot reach a PUBLIC output** (a log, an error message,
a network response, stdout — the Stage-3 `Write`/`Net`/`print` boundaries) without
an explicit, **AUDITED `declassify` door** (`declassify(Secret) -> Public`). A
`Secret` reaching an `emit`/`print` boundary is the confidentiality flow to forbid
(`03-effect-stdlib.md` — the public-output sinks). Kills: logged passwords, keys
in responses, secrets in stack traces. (At v1.1, the mark propagates the dual way:
a `Secret` combined with ANYTHING stays secret — REQ-4. At v1, a DIRECT
`emit(secret)` is already a type mismatch — GROUNDED.)

### Axis 3 — CAPABILITIES (`Authorized`)

A protected operation's parameter type demands a **proof-carrying capability
token** (`Authorized`) that ONLY the auth check produces — the op is un-callable
without it. Kills: missing authorization, IDOR (insecure-direct-object-reference).
The capability is the dual of a sink: where a sink's parameter type demands the
*absence* of a mark (the clean type, not the tainted one), a protected op's
parameter type demands the *presence* of a mark (`Authorized`, only the
`authorize` door produces it). The op's `req` (e.g. `req c.ok`) discharges from
the door's `ens` (e.g. `ens result.ok`).

### The unifying law — handled-or-loud, the COMPILE-TIME tooth (the loudest)

This stage instantiates, in SECURITY, the toolchain's unifying law (the **#62**
design-refinement principle, stated in `01-adts.md` and `03-effect-stdlib.md`):
**for every outcome a program models it either HANDLES it (a proven/checked path)
or SCREAMS (an explicit, typed, greppable refusal); silently doing the wrong
thing is structurally impossible.** A forbidden flow is HANDLED (routed through a
door — `parameterize`/`declassify`/`authorize`) or it is a **compile-time
SCREAM** (the program does not type-check / does not certify — `L0`/`FAILED`).
This is the LOUDEST tooth (the same rung `01-adts.md` REQ-5/REQ-12 owns for
exhaustive `match`): the dangerous flow is caught *before the program ships*, not
at runtime and not at the syscall. The SQLi program does not compile (GROUNDED
end-to-end). The fiat line is a KNOB: whatever flow you NAME (mark a source
`Tainted`, a value `Secret`, an op capability-gated) the toolchain forces
handled-or-loud; the doors you trust are NAMED in the manifest (the §9 TCB).
`grep declassify` = every secret-release; `grep parameterize`/`grep sanitize` =
every taint-clearing — the security TCB is grep-complete (§8, GROUNDED via `forge
audit`).

## The v1 enforcement mechanism — TYPE-LEVEL, emergent from SHIPPED machinery

The crux scoping decision, RESOLVED and GROUNDED. The centerpiece — "the SQLi
program doesn't compile" — is achievable at the TYPE LEVEL without any dataflow
pass:

1. `Tainted` and `Sql` are **DISTINCT types** (two Stage-1 newtype `struct`s —
   `01-adts.md` REQ-1/REQ-8, SHIPPED).
2. The SQL sink `query(s: Sql)` demands `Sql` **by its parameter type**.
3. The ONLY door from `Tainted` to `Sql` is `parameterize(t: Tainted) -> Sql`
   (a `#[boundary]` fn — `ffi-boundary.md` REQ-2, SHIPPED).
4. Therefore `query(input)` where `input: Tainted` is a **TYPE MISMATCH** — and
   the existing lower→verus type-check REJECTS it (`E0308: expected Sql, found
   Tainted`), so the function fails to certify (`L0`/`FAILED`).
5. The same input routed through the door — `query(parameterize(input))` —
   type-checks and CERTIFIES.

**This needs NO new validator code.** The careless program PASSES parse, validate,
and effect-check unchanged — the validator has no marked-type knowledge and does
not reject it. The rejection happens at the LOWERING/verus type-check, which is
already the engine that rejects every type error. The flow rule IS the sink's
parameter type. The marked types are ordinary Stage-1 structs; the sinks and doors
are ordinary `#[boundary]` fns. **The v1 enforcement is emergent — Stage-1 ADTs +
the boundary form + the type system already produce it** (GROUNDED below; the
v1 deliverable is the CORPUS + the skill grammar that teaches the IFC vocabulary,
not a new toolchain pass).

The harder claim — that a mark flowing into a DERIVED value stays marked and is
rejected at the sink even when the value reaching the sink is not syntactically
the source — is the **v1.1 dataflow-propagation engine (REQ-4)**, the only NEW
validator code this stage describes. The v1 slice proves the *destination* (the
sink's type rejects the raw mark); the v1.1 engine proves the *journey* (the mark
reaches the sink through intermediate values).

## The layer map (6a / 6b / 6c)

- **6a — the marked types + the door/sink DECLARATIONS (the vocabulary).** The
  three marked types are per-axis Stage-1 newtype `struct`s; the doors and sinks
  are `#[boundary]` fns. NO new toolchain code (SHIPPED substrate). Deliverable:
  the corpus `.th` declarations + the skill grammar (§10, #7) that teaches the IFC
  vocabulary (the marks, the door verbs, the sink catalog). REQ-1, REQ-2, REQ-3.
- **6b — the TYPE ENFORCEMENT (reject raw-marked → sink).** The lower→verus
  type-check rejects a raw marked value at a sink; the doored value certifies.
  EMERGENT (SHIPPED). v1 deliverable: the conformance corpus asserting the careless
  path FAILS (`L0`/`E0308`) and the doored path certifies (`L3`/to-boundary). The
  v1.1 dataflow-propagation pass (REQ-4) lands here as NEW validator code. REQ-3
  (v1), REQ-4 (v1.1), REQ-5.
- **6c — the AUDIT TCB enumeration of doors + the centerpiece demo.** `forge audit`
  enumerates every reached door in the manifest `boundary_contracts` (name +
  target + req + ens + fx) — the grep-complete security TCB. SHIPPED
  (`Tcb::from_certificates`, GROUNDED below). REQ-6.

## The door-as-audited-TCB honesty (the honest ceiling)

A door is a **trusted point**. You trust `html_escape` to actually escape, you
trust `validate_path` to actually canonicalize-and-root, you trust `declassify`
to be an intentional release. The language proves the data CAN'T reach the sink
un-doored (a TYPE property, GROUNDED); it TRUSTS the door does its job. That trust
is made honest exactly the way Stage 3 makes a syscall honest (`03-effect-stdlib.md`
"the door-as-TCB" = "the boundary-as-TCB"):

- **A door is a `#[boundary]`/`#[slag]` with a contract.** A sanitizer
  (`parameterize`, `html_escape`, `validate_path`, `allowlist_host`) is a
  `#[boundary]` fn whose contract STATES what it guarantees (e.g. `parameterize`'s
  `ens result.q == t.raw`) and whose body is the trusted escaper. `declassify` and
  `authorize` likewise. The door's contract is L1-ENFORCED at the crossing
  (`ffi-boundary.md` REQ-4, the `lower_boundary_fn_l1` wrapper) — a door that
  violates its stated contract is caught at the boundary, not a free pass. This is
  the SAME legitimate-`external_body` distinction Stage 3 pins
  (`03-effect-stdlib.md` REQ-7, `boundary-composition.md` HONESTY ARGUMENT): the
  door is a declared trust boundary, NOT a `--no-cheating` core-logic cheat
  (R-DEFER-9).
- **Every door is enumerated in the audit manifest.** The doors are the security
  TCB — exactly where you trusted a sanitizer or released a secret. The
  `AuditManifest.tcb` `boundary_contracts` section (`audit-manifest.md` REQ-3,
  `Tcb::from_certificates`) enumerates each reached door: name + contract +
  foreign target + effect. `declassify` ESPECIALLY is audited (GROUNDED: `forge
  audit` of the secret program lists `['declassify', 'emit']`). This is the honest
  ceiling: not "no secret ever leaks" (you cannot prove the escaper), but "every
  secret-release passes a NAMED, contracted, enumerated door, and there are exactly
  THESE doors" (§9, the enumerable TCB).

The triple that makes a one-line door an honest TCB member is the Stage-3 triple
specialized to IFC: the door's guarantee is **stated** (its contract), the flow
through it is **typed** (the mark changes only at the door's return type), and the
door is **enumerated** (the manifest names it). A door without a contract, or a
mark-change OUTSIDE a declared door, is the gap the v1.1 dataflow engine (REQ-4)
rejects.

## How the marked types + doors are REPRESENTED (PINNED)

**The marked types are per-axis concrete newtype `struct`s, NOT generics.**
Thermite has **no user generics** — `StructItem` in `ast.rs` carries `name` +
`fields` + `inv` and NO type parameters (verified). A user cannot write `struct
Tainted<T>`. So the marked types are concrete Stage-1 wrappers (`struct Tainted {
raw: u64 }`, `struct Secret { val: u64 }`, `struct Authorized { ok: bool }`),
exactly the SHIPPED ADT form (`01-adts.md` REQ-1/REQ-8). The `Type::Generic` node
exists (`Option<usize>`) but is for the built-in `Option`; the marked types do not
use it. This is the v1 line (OQ-1): per-T concrete wrappers, the fixed three axes —
NOT a `Marked<Tag, T>` phantom-generic (un-expressible) and NOT a full lattice
(OQ-3). A `Tainted<String>` vs `Tainted<u64>` distinction would be per-element-type
concrete wrappers if needed; the corpus uses `u64` payloads as the grounding
exemplar (the mechanism is identical for any payload type).

**The doors are `#[boundary]` fns (the SHIPPED FFI-boundary form), audited.** A
door is a `#[boundary("ifc::parameterize")] fn parameterize(t: Tainted) -> Sql ens
result.q == t.raw fx pure ;` — a boundary fn (`body: None`, `boundary: Some`,
`FnItem.boundary` in `ast.rs`) whose foreign body is the trusted escaper, whose
contract is L1-enforced, and which `forge audit` enumerates in the TCB
`boundary_contracts`. `#[slag]` is the alternative for an in-language door with a
review reason. NO new node shape — the door reuses `struct BoundaryAttr` /
`struct SlagAttr` (SHIPPED).

**The sink enforcement is TYPE-LEVEL — the parameter type.** A sink demands the
clean type by its PARAMETER TYPE (`fn query(s: Sql)`): a raw `Tainted` argument is
a type mismatch the lower→verus type-check rejects. The capability sink ALSO uses
its `req` (`fn delete(c: Authorized) req c.ok`) which discharges from the door's
`ens result.ok`. The mechanism is the existing type-checking — extended only by
the marked-type/door VOCABULARY at v1, by the dataflow-propagation pass at v1.1.

## What v1 ships emergently vs the v1.1 validator-dataflow engine

This is the load-bearing honesty of this stage — be explicit about the line:

- **The DIRECT-SINK TYPE slice is enforced NOW (GROUNDED end-to-end below).** That
  a sink `fn query(s: Sql)` accepts ONLY a `parameterize`-produced `Sql`, that a
  direct `query(input)` (raw `Tainted`) FAILS to certify, and that
  `query(parameterize(input))` CERTIFIES — this is a pure TYPE property, enforced
  by the SHIPPED full toolchain path against the real `verus` binary today
  (`L3`/to-boundary for the doored path; `L0`/`FAILED` + `E0308` for the careless
  path). It is the Stage-1-ADT + boundary + Stage-5-composition machinery, already
  SHIPPED in their docs and re-grounded here for IFC. **v1 is this slice + the
  corpus + the skill grammar — NO new toolchain pass.**
- **The MARK-PROPAGATION engine is v1.1, NEW validator-dataflow work, NOT SMT.**
  That a tainted value flowing into a *derived* value STAYS tainted (`let y =
  f(x)` where `x: Tainted` makes `y` tainted), that a `Secret` *combined* with
  anything stays secret, that the mark propagates through assignment / function
  calls / ADT construction & destructuring / arithmetic — this is a DATAFLOW /
  type-propagation pass in `thermite-spec/src/validator.rs`, not a solver query. It
  is the CORE NEW WORK of v1.1 (more validator than SMT). The validator PROPAGATES
  the mark through the program and REJECTS the forbidden flows at the point a
  *derived* marked value reaches a sink/output/protected-op without passing a door
  (a fresh `SpecError` variant).

The v1 grounding proves the *shape* the v1.1 dataflow engine must produce (a sink
whose type only the door satisfies); the engine itself — tracking which derived
values carry which mark through the program — is the v1.1 validator work this doc
pins.

## Requirements

### The marked types + the doors (governs `thermite-syntax/src/ast.rs`)

- **REQ-1 (v1 — the three marked types — `Tainted` / `Secret` / `Authorized`):**
  the IFC mechanism is three concrete Stage-1 marked wrapper `struct`s (`01-adts.md`
  REQ-1/REQ-8 — a newtype struct over the carried value, the mark a TYPE property;
  NO user generics — PINNED above). `Tainted` (integrity, untrusted source),
  `Secret` (confidentiality, its dual), and `Authorized` (a proof-carrying
  capability token). v1 is these THREE fixed axes — NOT a full lattice with
  arbitrary security levels (OUT, OQ-3). Derived from §1 (trust relocation: the
  mark is the legible trust statement) + `01-adts.md` REQ-1/REQ-8 (the wrapper
  types, SHIPPED) + the **#62**/**#76** IFC decision. **GROUNDED**: `struct
  Tainted { raw: u64 }` parses, validates, lowers and verifies through the real
  toolchain (the `sqli_safe` run certifies `L3`).

- **REQ-2 (v1 — the doors — the only mark-changing operations, each a contracted
  `#[boundary]`/`#[slag]`):** a mark changes ONLY through a declared door: the
  SANITIZERS (`parameterize`, `shell_escape`, `validate_path`, `html_escape`,
  `allowlist_host`, `sanitize_log` — `Tainted -> Clean`), the `declassify` door
  (`Secret -> Public`), and the `authorize` door (auth-check `-> Authorized`, the
  ONLY `Authorized` producer). Each door is a `#[boundary]`/`#[slag]` fn with a
  contract (`FnItem.boundary.is_some() || FnItem.slag.is_some()` in `ast.rs`, the
  SHIPPED form), L1-enforced at the crossing (`ffi-boundary.md` REQ-4). No
  mark-change exists outside a door — a value's mark is fixed at construction (the
  struct literal) and changeable only by passing a door's return type. Derived
  from §9 (the boundary contract is the interface) + §8 (the door is
  greppable/enumerable) + `ffi-boundary.md` REQ-2 (the SHIPPED boundary form) +
  `boundary-composition.md` (the door's contract composes). **GROUNDED**: the
  `#[boundary("ifc::parameterize")]` door type-changes `Tainted -> Sql` and is
  enumerated by `forge audit`.

### The sink catalog + the flow rules (governs `thermite-syntax/src/ast.rs`,
`thermite-spec/src/validator.rs`)

- **REQ-3 (v1 — the sink catalog — every sink's parameter type / `req` demands the
  CLEAN type):** each security sink is a `#[boundary]` whose PARAMETER TYPE (and,
  for the capability sink, its `req`) demands the SANITIZED/clean type, never the
  raw/tainted one: the SQL sink demands `Sql` (only `parameterize` produces it),
  the shell sink `Argv`, the path sink `SafePath`, the HTML sink `Html`, the net
  sink `Host`, the public-output sinks demand `Public` (not `Secret`, Axis 2). The
  protected-op sink (Axis 3) inverts it: its parameter type demands the PRESENCE of
  `Authorized` (only `authorize` produces it) + a `req cap.ok`. The sink demanding
  the clean type is just a boundary contract the caller verifies THROUGH
  (`boundary-composition.md` REQ-1). **GROUNDED end-to-end** (the full-path slice
  below): `query(s: Sql)` accepts only a `parameterize`-produced `Sql`; raw
  `Tainted` to `query` is `L0`/`FAILED` with `E0308`; the doored path `L3`. Derived
  from §4.1 (the effect `req`/`ens` row) + §9 + `03-effect-stdlib.md` (the sinks
  are boundary primitives) + `boundary-composition.md` REQ-1.

- **REQ-4 (v1.1 — the validator mark-PROPAGATION + REJECTION engine — the core new
  work, NOT v1):** the validator (`thermite-spec/src/validator.rs`) PROPAGATES
  each mark through dataflow and REJECTS the forbidden flows at compile time when
  the value reaching a sink is a DERIVED value rather than the syntactic source.
  Propagation rules: a value derived from a `Tainted` value is `Tainted` (through
  assignment, function call return, ADT construction/destructuring, arithmetic,
  field/index access); a value combining a `Secret` is `Secret`; the mark is
  cleared/changed ONLY by a door (REQ-2). Rejection rules (the forbidden flows): a
  *derived* `Tainted` value reaching a sink un-doored → a fresh
  `SpecError::TaintReachesSink { sink, span }`; a derived `Secret` reaching a
  public output un-`declassify`'d → `SpecError::SecretReachesPublic { sink, span }`;
  a protected op called without `Authorized` along a derived path →
  `SpecError::MissingCapability { op, span }`. This is the DATAFLOW /
  type-propagation engine (more validator than SMT) — the SHAPE of which the v1
  type slice proves, but whose mark-through-the-program tracking is NEW v1.1 work.
  Derived from §4.1 + §2.4 (crisp structured feedback) + `01-adts.md` REQ-5 (the
  validator's `SpecError` reject discipline) + the **#62**/**#76** IFC-dataflow
  decision. **v1.1, not v1** — the v1 direct-sink slice does NOT need this (a
  direct `query(input)` is already a type error, GROUNDED).

### Lowering + honesty (governs `thermite-lower/src/lower.rs`, `forge/src/audit.rs`
— via the SHIPPED #15 path)

- **REQ-5 (v1 — marks lower to Stage-1 wrapper types; doors lower to
  `external_body`):** a marked type lowers to its Stage-1 Verus wrapper
  (`01-adts.md` REQ-8/REQ-9 — a `struct`/`enum`, SHIPPED); the sink's clean-type
  parameter and the door's `ens` lower to the existing Verus param-type +
  `ensures` (`verus-lowering.md`), and the door (a `#[boundary]`) lowers to a
  `#[verifier::external_body]` signature woven into the caller's sub-program
  (`boundary-composition.md` REQ-1, `lower_external_body_fn in lower.rs`) — so the
  caller proves THROUGH the door's contract and the door's trusted body is never
  proved. **GROUNDED**: the typed-sink + door + caller pattern lowers and certifies
  `L3` / to-boundary against `verus 0.2026.05.24`; the careless caller lowers to a
  `Tainted` argument at a `Sql` parameter, which verus rejects `E0308`. Derived
  from §3 (transpile to Verus) + `01-adts.md` REQ-8/REQ-9 (SHIPPED) +
  `boundary-composition.md` REQ-1 + the GROUNDED slice.

- **REQ-6 (v1 — the doors are the security TCB — enumerated in the audit
  manifest):** every door a program reaches is enumerated in the
  `AuditManifest.tcb` `boundary_contracts`/`slag_blocks` section (`audit-manifest.md`
  REQ-3, `Tcb::from_certificates in forge/src/audit.rs`) — name + contract +
  foreign target + effect. `declassify` especially is audited: every secret-release
  is a named, contracted, enumerated door. A program routing through doors is
  verified-to-the-boundary listing exactly the doors (`e2e-vs-boundary.md` #17,
  `05-composition.md` REQ-7). The manifest NEVER claims "no leak, period" — it
  claims "every flow passes THESE enumerated doors" (R-DEFER-9). **GROUNDED**:
  `forge audit /tmp/.../sqli_safe.th --json` emits `boundary_contracts` =
  `[{name: parameterize, target: ifc::parameterize, req: true, ens: [result.q ==
  t.raw], fx: [pure]}, {query, ...}]`; the secret program lists `[declassify,
  emit]`. `grep declassify` = the manifest's declassify list. Derived from §1 (the
  auditable residue) + §9 (the enumerable TCB) + §8 + `audit-manifest.md` REQ-3 +
  `05-composition.md` REQ-3/REQ-7.

- **REQ-7 (v1 — marks compose through the call graph — the Stage-5 hook):** a mark
  propagates through a multi-step call graph exactly as a contract composes
  (`05-composition.md` REQ-1/REQ-4): a caller `g` calling a sink `f` discharges
  `f`'s clean-type parameter from its own (doored) value's type, and a value's mark
  flows through the transitive closure the #52 weave already computes
  (`reachable_fn_deps in check.rs`). The whole-program honest-assurance statement
  (`05-composition.md`) holds: the verified pure core orchestrates the IFC doors
  (the world-interaction + trust-change surface), and the manifest aggregates the
  door TCB across the deep graph (`05-composition.md` REQ-7). **GROUNDED**:
  `safe_path` calls `parameterize` then `query` across the graph and certifies
  `L3`/to-boundary (the #52 weave already composes the doors). Derived from §9 +
  `05-composition.md` REQ-1/REQ-4/REQ-7 (SHIPPED) + the **#62** Stage-5 weaving.

## Acceptance criteria

ACs tie to a NEW `conformance/provenance/` oracle the ORCHESTRATOR authors (a
hand-derived cases file, the `conformance/composition/cases.json` /
`conformance/effect-stdlib/cases.json` precedents — R-CHAR-3, expected values
hand-derived from the flow rules + verus/type semantics, NEVER copied from
toolchain output). The CENTERPIECE is the un-typeable demo: a program that passes
user input to a SQL sink **fails to certify**, and the same program routed through
`parameterize` certifies. The EXACT corpus + expected full-path output:

- **AC-1 (v1 — the SQLi program does NOT compile — the centerpiece):** a corpus
  program `conformance/provenance/sqli.th` (`struct Tainted`, `struct Sql`,
  `#[boundary] parameterize(Tainted) -> Sql`, `#[boundary] query(Sql) -> u64`,
  `fn careless_path(input: Tainted) { query(input) }`) is REJECTED — the careless
  fn lowers and verus reports `error[E0308]: expected Sql, found Tainted`, the cert
  is `Level::L0` and the project assurance is `FAILED` (exit 1). The SAME program
  with `fn safe_path(input: Tainted) { query(parameterize(input)) }`
  (`conformance/provenance/sqli_safe.th`) VALIDATES, lowers, and certifies — the
  doored fn is `Level::L3` / to-boundary (via `query`), the doors `parameterize`/
  `query` are `L1` boundaries, project assurance `L1` (min over functions), exit 0.
  **GROUNDED end-to-end** (`forge check`, `verus 0.2026.05.24`): careless = `L0`/
  `FAILED`/`E0308`; safe = `L3`/`L1`/exit 0 (output pasted in Architecture).
  (REQ-1, REQ-3, REQ-5, REQ-7.)

- **AC-2 (v1 — a `Secret` reaching `emit` does NOT compile; declassified does +
  shows in the manifest):** `conformance/provenance/secret_leak.th`
  (`fn leak(s: Secret) { emit(s) }` where `emit(p: Public)`) is REJECTED
  (`L0`/`FAILED`, `E0308: expected Public, found Secret`); `secret_safe.th`
  (`fn safe_emit(s: Secret) { emit(declassify(s)) }`) certifies (`L3`/to-boundary);
  and `forge audit secret_safe.th` enumerates `declassify` (and `emit`) in the
  `tcb` `boundary_contracts` (REQ-6 — every secret-release is in the manifest).
  **GROUNDED**: leak = `L0`/`E0308`; safe = `L3`; audit lists `[declassify, emit]`.
  (REQ-1, REQ-3, REQ-6.)

- **AC-3 (v1 — a protected op called without `Authorized` does NOT compile):**
  `conformance/provenance/cap_missing.th` (`fn unauth_delete(u: User) { delete(u) }`
  where `delete(c: Authorized) req c.ok`) is REJECTED (`L0`/`FAILED`, `E0308:
  expected Authorized, found User`); `cap_safe.th`
  (`fn safe_delete(u: User) { delete(authorize(u)) }`) certifies — the op's `req
  c.ok` discharges from `authorize`'s `ens result.ok`. **GROUNDED**: missing =
  `L0`/`E0308`; safe = `L3`. (REQ-1, REQ-3.)

- **AC-4 (v1.1 — mark propagation through a derived value rejects):** a tainted
  value flowed into a DERIVED value (`let y = passthru(x); query(y)` where `x:
  Tainted` and `passthru` returns the derived value still tainted) is REJECTED —
  the v1.1 validator-dataflow pass propagates the taint to `y` (REQ-4 propagation
  rule) and emits `SpecError::TaintReachesSink` at the sink, even though `y` is not
  syntactically the tainted source. A `Secret` combined into a derived value stays
  secret and rejects at a public output. Hand-derived expectations (R-CHAR-3). This
  is the v1.1 validator-dataflow engine's load-bearing behavior (NOT the v1 type
  slice — at v1 a function whose intermediate erases the marked type back to the
  clean type would type-check; only the dataflow pass catches the propagated mark).
  **NOT GROUNDED at v1** (the engine is unbuilt). (REQ-4.)

- **AC-5 (v1 — the doors are enumerated as the security TCB):** `forge audit` of
  the doored programs (`sqli_safe`, `secret_safe`, `cap_safe`) emits an
  `AuditManifest` whose `tcb` `boundary_contracts` enumerates `parameterize` /
  `declassify` / `authorize` (name + target + req + ens + fx); the pure caller
  appears as `L3` + to-boundary; nothing fiat-trusted is omitted (R-DEFER-9). `grep
  declassify`/`grep parameterize` over the corpus = the manifest's door list.
  **GROUNDED**: the `--json` audit lists exactly the reached doors (output pasted).
  (REQ-2, REQ-6.)

- **AC-6 (v1 — the existing corpus is unaffected — no regression):** the existing
  pure corpus (`sum`, `binary_search`, `shape`, `bank_account`) and the prior
  stages' corpora certify IDENTICAL certs before and after Stage 6 — no marked type
  appears, no IFC flow is checked, byte-stable goldens. The v1 IFC additions are
  PURELY ADDITIVE corpus + skill grammar (no new toolchain code at all); the v1.1
  additions (a new `SpecError` variant + a new validator pass) must be a no-op on
  mark-free programs. (All REQs; the security layer must not regress the kernel.)

## Architecture

Stage 6's **v1 owns NO new toolchain mechanism** — it instantiates SHIPPED
machinery (the Stage-1 wrapper types, the `#[boundary]` doors/sinks, the #52
compose-through, the #15 door enumeration) as a corpus + skill grammar. Stage 6's
**v1.1 owns ONE new mechanism** — the validator mark-propagation/rejection engine
(REQ-4). The component spans three crates, additively:

- **`thermite-syntax/src/ast.rs`** — the three marked types are Stage-1 concrete
  `struct` wrappers (`StructItem`, SHIPPED, `01-adts.md` REQ-1); the doors are the
  SHIPPED `#[boundary]`/`#[slag]` form (`FnItem.boundary` / `FnItem.slag`, with
  `struct BoundaryAttr`/`struct SlagAttr` ALREADY in `ast.rs`). No new node SHAPE
  is required at v1 — the marked types reuse the struct surface, the doors reuse
  the boundary surface. NO user generics (`StructItem` has no type params — PINNED).
- **`thermite-spec/src/validator.rs`** — UNCHANGED at v1 (a marked-type program
  validates clean; the type rejection happens at lowering/verus). The v1.1
  mark-propagation/rejection pass (`pub fn validate` extended): collect the
  marked-type set, propagate the mark through the dataflow of each `fn` body
  (assignment / call / ADT construct-destruct / arithmetic / field-index), and
  reject the forbidden DERIVED flows with the new `SpecError` variants
  (`TaintReachesSink` / `SecretReachesPublic` / `MissingCapability`). This is the
  CORE v1.1 work — a dataflow pass, NOT a solver query. The caged-flat walk
  (`spectherm-combinators.md` REQ-6) is UNCHANGED.
- **`thermite-lower/src/lower.rs`** — the marked types lower to their Stage-1
  wrappers via the SHIPPED `lower_struct` (`01-adts.md` REQ-8); the doors lower to
  `#[verifier::external_body]` signatures via the SHIPPED `lower_external_body_fn`
  (`boundary-composition.md` REQ-1). No new emission SHAPE — the type mismatch
  (`Tainted` arg at a `Sql` param) is rejected by verus's own type-check on the
  emitted source. UNCHANGED at v1; UNCHANGED at v1.1 (the dataflow reject is a
  validator concern, fired before lowering).
- **`forge/src/audit.rs`** — the doors are enumerated by the SHIPPED
  `Tcb::from_certificates` (`audit-manifest.md` REQ-3) — UNCHANGED, the doors are
  boundaries the existing TCB enumeration already lists (GROUNDED).

Symbol anchors: `struct StructItem` / `struct BoundaryAttr` / `struct SlagAttr` /
`enum Effect` in `ast.rs` (the SHIPPED marked-type + door substrate); `pub fn
validate` in `validator.rs` (the v1.1 mark-propagation pass extends it);
`lower_external_body_fn` / `lower_struct` in `lower.rs`; `Tcb::from_certificates`
in `audit.rs`.

### The full-path type-level slice (GROUNDED — real `forge` + `verus 0.2026.05.24`)

The v1 type-level enforcement of all three axes was run END-TO-END through the
real toolchain during authoring (`forge check` / `forge audit`, the real
`verus 0.2026.05.24.ecee80a` binary on PATH; scratch removed — `forge`'s
`ScratchDir` Drop guard cleans each run, `/tmp` scratch deleted). This is the seed
for the golden lowering; it proves the type-level slice GROUNDS at the toolchain
level (NOT merely a hand-run verus snippet).

**TAINT axis — the safe doored path (`sqli_safe.th`) — `forge check`:**

```
item: parameterize
level: L1
boundary: true   boundary_target: ifc::parameterize
  [ok] contract enforced at L1 (boundary); foreign body trusted by fiat
item: query
level: L1
boundary: true   boundary_target: ifc::query
  [ok] contract enforced at L1 (boundary); foreign body trusted by fiat
item: safe_path
level: L3
assurance_scope: to-the-boundary (via query)
  [ok] 1 obligations discharged
---
project assurance: L1 (min over functions)        (exit 0)
```

**TAINT axis — the SQLi careless path (`sqli.th`, `query(input)`) — does NOT
certify (the un-typeable demo at the TOOLCHAIN level):**

```
item: careless_path
level: L0
assurance_scope: to-the-boundary (via query)
  [FAIL] verus reported obligation failure
         error[E0308]: mismatched types
  --> .../Tainted_check.rs:27:11
   |
27 |     query(input)
   |     ----- ^^^^^ expected `Sql`, found `Tainted`
   |     |
   |     arguments to this function are incorrect
---
project assurance: FAILED (a function did not certify)        (exit 1)
```

**RECORDED FINDING.** The careless SQLi path PASSES parse + validate +
effect-check (the validator has no marked-type knowledge and accepts it) and is
rejected at the LOWERING/verus TYPE-CHECK — `Tainted` is not `Sql`, and only
`parameterize` produces `Sql`. **No new validator code is involved.** This is the
un-typeable demo at v1: the sink's parameter type IS the flow rule, enforced by
the SHIPPED full-path type-check. The doored path certifies `L3`/to-boundary. What
this slice does NOT show — the v1.1 work — is that a value DERIVED from tainted
input carries the taint through the program; that mark-PROPAGATION (REQ-4) is the
v1.1 dataflow pass, not this type-level slice.

**SECRET axis (`secret_safe.th` / `secret_leak.th`):** `safe_emit(s) {
emit(declassify(s)) }` certifies `L3`; `leak(s) { emit(s) }` is `L0`/`FAILED` with
`error[E0308]: expected Public, found Secret`. `forge audit secret_safe.th --json`
lists the doors `[declassify, emit]` in the `tcb` `boundary_contracts`.

**CAPABILITY axis (`cap_safe.th` / `cap_missing.th`):** `safe_delete(u) {
delete(authorize(u)) }` certifies `L3` (the op's `req c.ok` discharges from
`authorize`'s `ens result.ok`); `unauth_delete(u) { delete(u) }` is `L0`/`FAILED`
with `error[E0308]: expected Authorized, found User`.

**The audit TCB enumeration (`forge audit sqli_safe.th --json`) — the doors are
the grep-complete security TCB:**

```json
"boundary_contracts": [
  { "name": "parameterize", "target": "ifc::parameterize",
    "req": "true", "ens": ["result.q == t.raw"], "fx": ["pure"] },
  { "name": "query", "target": "ifc::query",
    "req": "true", "ens": ["result == s.q"], "fx": ["pure"] }
]
```

The three axes are ONE mechanism — a marked type, a door (a `#[boundary]` that
type-changes the mark), a sink whose parameter type encodes the flow rule —
confirmed end to end through the real `forge`/`verus` binaries.

## Dependency hooks (the Stage 1 / 3 / 5 wiring)

- **Stage 1 (marked types — `01-adts.md`):** `Tainted` / `Secret` / `Authorized`
  ARE Stage-1 wrapper ADTs (REQ-1/REQ-8 SHIPPED — a `struct`/newtype over the
  carried value; the mark a TYPE property). The marked type lowers via the SHIPPED
  Stage-1 `lower_struct` (REQ-8). Stage 6 v1 rides Stage 1 verbatim (GROUNDED).
- **Stage 3 (the sinks — `03-effect-stdlib.md`):** the SINKS are Stage-3
  effect-primitive `#[boundary]`s (the SQL/shell/file/net/log primitives); each
  sink's parameter type demands the CLEAN type. A `Secret` reaching a
  `Write`/`Net`/`print` boundary is the confidentiality flow to forbid. The doors
  are ALSO `#[boundary]`s. Stage 6 reuses the boundary form verbatim (REQ-2/REQ-3,
  GROUNDED).
- **Stage 5 (marks compose — `05-composition.md`):** a mark composes through the
  call graph exactly as a contract composes (REQ-1/REQ-4 — the #52 weave, SHIPPED);
  the door TCB aggregates across a deep graph (REQ-7). Stage 6's REQ-7 IS the IFC
  instantiation of the Stage-5 composition law (GROUNDED: `safe_path` composes
  `parameterize` + `query` and certifies `L3`).

## Verification

- **Mandatory full-path grounding (DONE during authoring — real `forge` + `verus
  0.2026.05.24.ecee80a`, scratch removed).** The v1 type-level slice — all three
  axes — was run END-TO-END through `forge check` / `forge audit` against the real
  binaries: the doored paths certify (`L3`/`L1`/to-boundary, exit 0), the careless
  paths are `L0`/`FAILED`/`E0308` (exit 1), and `forge audit` enumerates the doors
  in the TCB. This proves the v1 enforcement is EMERGENT from SHIPPED machinery (no
  new toolchain code). The v1.1 mark-PROPAGATION engine (REQ-4) is the NOT-STARTED
  validator-dataflow work the grounding does NOT cover (it is not an SMT property
  and the engine is unbuilt).
- **AC-1/AC-2/AC-3/AC-5 (v1):** `cargo test -p forge` over a new
  `conformance/provenance/` corpus, shelling the real `verus` binary on the emitted
  lowering of the doored programs (assert exit 0 + `L3`/to-boundary, R-CODE-4) and
  asserting the careless programs are `L0`/`FAILED` with an `E0308`-class verus type
  error, plus `forge audit` enumerating the doors in the TCB.
- **AC-4 (v1.1):** validator-dataflow reject fixtures (hand-derived expectations,
  R-CHAR-3) exercising mark propagation through a derived value — gated on the
  REQ-4 engine being built.
- **AC-6 (v1):** the existing `conformance/{sum,binary_search,shape,bank_account}`
  certs stay byte-stable (v1 adds no toolchain code; v1.1's validator pass is a
  no-op on mark-free programs).
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

The `validator.rs` route is needed for v1.1 (REQ-4); the `ast.rs`/`lower.rs`
routes cover the v1 corpus + golden. The orchestrator authors
`conformance/provenance/cases.json` (the oracle this doc's ACs cite), the
`conformance/provenance/{sqli,sqli_safe,secret_leak,secret_safe,cap_missing,cap_safe}.th`
programs, their `.cert.json` goldens, and the `tests/golden/lower/sqli_safe.verus.rs`
golden (hand-authored from the GROUNDED slice, confirmed to pass `verus`), BEFORE
the builder runs (R-CHAR-3). This doc does NOT author the oracle, the goldens, or
the routes (R-DOC-1).

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (v1 — the three marked types — `Tainted`/`Secret`/`Authorized`) | NOT-STARTED | epic **#62** / issue **#76** Stage 6. No `Tainted`/`Secret`/`Authorized` type in the tree or corpus (`grep -r "Tainted\|Secret\|Authorized"` over `.rs`/`conformance` returns NONE). The SUBSTRATE is SHIPPED (`StructItem` newtype, `01-adts.md` REQ-1/REQ-8 SHIPPED, no user generics — PINNED) and GROUNDED through the full path (`struct Tainted { raw: u64 }` certifies `L3` in `sqli_safe`); the v1 deliverable (the corpus declarations + skill vocabulary) is not authored. |
| REQ-2 (v1 — the doors — only mark-changing ops, contracted `#[boundary]`/`#[slag]`) | NOT-STARTED | epic **#62** / issue **#76** Stage 6. No `parameterize`/`declassify`/`authorize` door exists in the corpus (`grep -r "declassify\|sanitize"` returns NONE). The SHIPPED door substrate (`struct BoundaryAttr`/`struct SlagAttr` in `ast.rs`, `FnItem.boundary`/`.slag`, `ffi-boundary.md` REQ-2 SHIPPED) is the form, GROUNDED (the `#[boundary] parameterize(Tainted) -> Sql` door type-changes the mark and is audit-enumerated), but no IFC door is declared. |
| REQ-3 (v1 — the sink catalog — every sink's param type / `req` demands the CLEAN type) | NOT-STARTED | epic **#62** / issue **#76** Stage 6. No security sink exists in the corpus. The sink-demands-clean-type mechanism is SHIPPED + GROUNDED end-to-end: `query(s: Sql)` rejects raw `Tainted` (`L0`/`FAILED`/`E0308`) and accepts a `parameterize`-produced `Sql` (`L3`) through the real `forge`/`verus` — EMERGENT from the type system, no new validator code. The corpus is not authored. |
| REQ-4 (v1.1 — validator mark-PROPAGATION + REJECTION engine — the core new work) | NOT-STARTED | epic **#62** / issue **#76** Stage 6, **v1.1** (NOT v1). `thermite-spec/src/validator.rs` has no taint/secret/capability propagation pass and no `TaintReachesSink`/`SecretReachesPublic`/`MissingCapability` `SpecError` variant (`enum SpecError` lists no IFC variant). This is the NEW dataflow engine (NOT SMT) — the v1 type slice proves only the DIRECT-sink contract (a derived-value mark needs this pass). Compile-time tooth of handled-or-loud for derived flows. |
| REQ-5 (v1 — marks lower to Stage-1 wrappers; doors lower to `external_body`) | NOT-STARTED | epic **#62** / issue **#76** Stage 6. No marked type or door in the corpus. The mechanism is SHIPPED + GROUNDED (`lower_struct` for the wrapper, `01-adts.md` REQ-8 SHIPPED; `lower_external_body_fn` for the door, `boundary-composition.md` REQ-1; the careless path's `Tainted`-arg-at-`Sql`-param is rejected by verus `E0308` on the emitted source). The corpus/golden is not authored. |
| REQ-6 (v1 — the doors are the security TCB — enumerated in the manifest) | NOT-STARTED | epic **#62** / issue **#76** Stage 6. The SHIPPED `Tcb::from_certificates in forge/src/audit.rs` (`audit-manifest.md` REQ-3) enumerates boundary contracts as the TCB — GROUNDED for IFC: `forge audit sqli_safe.th --json` lists `[parameterize, query]`, `secret_safe.th` lists `[declassify, emit]` (name + target + req + ens + fx). No IFC corpus exists to audit yet; `grep declassify` = the door list once the corpus lands. |
| REQ-7 (v1 — marks compose through the call graph — the Stage-5 hook) | NOT-STARTED | epic **#62** / issue **#76** Stage 6. The SHIPPED #52 compose-through (`reachable_fn_deps in check.rs`, `05-composition.md` REQ-1) + the #15 deep-graph TCB aggregation (`05-composition.md` REQ-7) are the mechanism — GROUNDED: `safe_path` composes `parameterize` + `query` across the graph and certifies `L3`/to-boundary. No IFC corpus program composes a mark yet. |

## Open questions (for the orchestrator before the builder runs)

- **OQ-1 (RESOLVED — marked type as a concrete newtype, NOT a generic).** Thermite
  has NO user generics (`StructItem` carries no type params — verified). The marked
  types are concrete per-axis Stage-1 newtype `struct`s (`struct Tainted { raw }`,
  etc.), the SHIPPED ADT form, GROUNDED. A `Marked<Tag, T>` phantom-generic is
  un-expressible. If a `Tainted<String>` vs `Tainted<u64>` distinction is ever
  needed it is per-payload concrete wrappers; the corpus uses `u64` payloads as the
  grounding exemplar (the mechanism is payload-agnostic). The §10 6k-token skill
  must hold the IFC grammar (the marks + the door verbs + the sink catalog) — a
  real budget check at Stage 6's skill regeneration (#7); the surface is small
  (three types + a handful of door verbs) and expected to fit. Not a blocker.

- **OQ-2 (least-confident: the v1.1 mark-propagation engine's REACH — implicit
  flows, marks through arithmetic/ADTs).** REQ-4 (v1.1) is the highest-judgment,
  least-confident part. The v1 EXPLICIT-direct slice is GROUNDED (a tainted value
  passed DIRECTLY to a sink is a type error). The v1.1 open reach: (a) IMPLICIT
  flows — a `Secret` that influences a CONTROL path (`if secret > 0 { print("hi")
  }` leaks one bit) — v1.1 LEANS to tracking EXPLICIT data-flow only (the value
  reaching the sink), NOT implicit/control-flow leaks (a much harder
  non-interference property, future work like constant-time below); (b) marks
  through ARITHMETIC and ADT construct/destructure — `Tainted(a) + b` is tainted,
  `match t { ... }` on a tainted scrutinee taints the bindings — these are
  tractable explicit-flow rules the v1.1 engine must pin precisely; (c) the v1.1
  LINE — explicit data-flow propagation through assignment/call/arith/ADT,
  rejecting at sinks, is v1.1; implicit/control-flow and full lattice IFC are OUT.
  The builder must pin the propagation rules mechanically (a fixture per rule,
  AC-4). This is the REQ I am LEAST confident is fully specified.

- **OQ-3 (the v1 fixed-axis line vs full lattice IFC — and the OUT-of-scope future
  axes).** v1 is the THREE FIXED axes (tainted/clean, secret/public, the
  capability set) — NOT a full lattice IFC with arbitrary user-defined security
  levels (OUT, harder and unneeded for the CVE catalog). Explicitly noted as OUT,
  do-not-build: (a) **constant-time crypto / side-channels** — a harder RELATIONAL
  property (non-interference over timing), a FUTURE axis, not v1/v1.1; (b)
  **TOCTOU / concurrency** — out; (c) **full lattice IFC** — out. These are named
  so the builder does not over-reach; they are future work, not Stage-6 gaps. Not a
  blocker.

- **OQ-4 (the door's L1 enforcement vs the type-level guarantee — the honesty
  ceiling).** The language proves the data CAN'T reach the sink un-doored (a TYPE
  property, GROUNDED); it TRUSTS the door does its job (the escaper actually
  escapes). The door's contract is L1-enforced at the crossing (`ffi-boundary.md`
  REQ-4), but that L1 check verifies the door's STATED contract, which for a
  sanitizer is itself a trust statement (you cannot prove `html_escape` escapes all
  XSS vectors — that is the fiat the manifest enumerates, REQ-6, GROUNDED). The
  open question: how strong is a sanitizer's STATED contract (a shape claim, like
  Stage-3's syscall contracts, or a stronger property)? LEANING: a shape claim
  ("the result is the `Sql`/`Html` clean type") + the door enumerated in the TCB —
  the same honest ceiling as Stage 3 (`03-effect-stdlib.md` REQ-3,
  outcome-coverage). The honesty is "every flow passes a NAMED door," not "the door
  is provably correct." Not a blocker; the builder pins the door-contract strength
  against the `conformance/provenance` oracle.
