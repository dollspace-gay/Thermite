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
value), **SEALED clean types** (the clean/capability type is ABSTRACT — only its
door mints it, see the abstraction barrier below), **typed SINKS** (each demanding
the clean/safe type by its parameter type), and **DOORS** (the only mark-changing
operations — the audited, greppable security TCB). A tainted value reaching a SINK
without a sanitizer door, a `Secret` reaching a public output without an audited
`declassify`, or a protected op called without its `Authorized` capability is a
**compile-time SCREAM** — the loudest tooth of the toolchain's handled-or-loud
law. The SQL-injection program does not compile.

**v1 scope is PINNED (the buildable slice — GROUNDED, with the corrected claim
below):**

- **v1 = TYPE-LEVEL enforcement of the three axes at a DIRECT sink call, made REAL
  by the `#[sealed]` abstraction barrier.** The marked types are DISTINCT types
  from the clean types; a sink's parameter type demands the clean type; the only
  door from marked→clean is the declared door fn; so passing a raw marked value to
  a sink is a **TYPE MISMATCH** that the full toolchain path (parse → validate →
  effect-check → lower → verus) REJECTS. **BUT the type mismatch ALONE is not
  enough** — see the corrected claim. A clean type that is an ordinary struct with
  accessible fields can be MINTED directly via a `StructLit` (`Sql { stmt:
  input.raw }`) from a marked value, bypassing the door (**critic finding #77**).
  v1 therefore REQUIRES the **`#[sealed]` abstraction barrier** (REQ-8 below, a NEW
  validator rule + a struct attribute): a `#[sealed]` clean type CANNOT be
  constructed by a `StructLit` anywhere in Thermite code, so the ONLY way to obtain
  one is through its `#[boundary]` door. With the barrier, `query(Sql { stmt:
  input.raw })` is REJECTED at validation (`SpecError::SealedConstruction`) and the
  door is the only launder point. REQ-1/REQ-2/REQ-3/REQ-5/REQ-6/REQ-7/**REQ-8** are
  the v1 slice.
- **v1.1 = the DATAFLOW-PROPAGATION engine (REQ-4).** That a mark flows through
  INTERMEDIATE values (`let y = f(x)` where `x` is tainted makes `y` tainted),
  the dual secret-propagation (a value combining a `Secret` stays secret), and
  the reject at the point a *derived* marked value reaches a sink — this is the
  harder NEW validator-dataflow pass in `thermite-spec/src/validator.rs`. It is
  explicitly **v1.1**, NOT v1; the v1 type-level slice (with the sealed barrier)
  rejects the direct laundering forms without it.

This doc is **FORWARD-LOOKING for the IFC vocabulary** (`grep -r
"Tainted\|Secret\|Authorized\|declassify\|sanitize\|sealed"` over the `.rs` tree
returns NONE — no IFC type, door, or `#[sealed]` attribute is declared in the
toolchain or corpus today). **Every REQ below is NOT-STARTED.** REQ-1–REQ-7 are
tracked under epic **#62** / issue **#76** (no separate blocker — **#76** owns the
stage); **REQ-8 (the `#[sealed]` barrier) is owned by blocker #77** (the
abstraction-barrier fix; no redundant blocker is filed). Stage 6 BUILDS ON four
SHIPPED substrates: the Stage-1 marked wrapper TYPES (`01-adts.md` REQ-1/REQ-8 — a
newtype struct carrying its value), the SHIPPED `#[boundary]` SINK/door form
(`ffi-boundary.md` REQ-2; the Stage-3 effect-primitive sinks, `03-effect-stdlib.md`),
the Stage-5 composition law (`05-composition.md`), and the audit-manifest door
enumeration (`audit-manifest.md` #15). The `#[sealed]` validator rule (REQ-8) is the
ONE genuinely NEW v1 toolchain mechanism this stage adds.

## The model — IFC, one mechanism, three axes

Most of the security-CVE catalog reduces to ONE mechanism — a marked type, a
SEALED clean (or capability) type the sink's parameter type demands, and a small
set of doors (the only operations that mint a clean type / change a mark), which
are the audited security TCB. Three axes instantiate it:

### Axis 1 — Integrity / TAINT (`Tainted`)

Data from an untrusted source (user input, network, a file read) carries a
**taint mark**. The mark is a TYPE property: `Tainted` is a Stage-1 wrapper over
the carried value (`01-adts.md` REQ-1 — a `struct`/newtype). A tainted value
**cannot reach a SINK** without first passing a declared **sanitizer door**. The
sink catalog (each sink's parameter type / `req` demands a SANITIZED/clean type,
never the raw/tainted one):

| Sink (`#[boundary]` primitive) | Bug class killed | Sanitizer door (the clean-type producer) | Clean type the sink demands (`#[sealed]`) |
|---|---|---|---|
| SQL `query` | SQL injection (SQLi) | `parameterize(Tainted) -> Sql` | `Sql` (parameterized statement) |
| shell / `exec` | command injection | `shell_escape` / structured-args `-> Argv` | `Argv` (structured argument vector) |
| file `open` / path | path traversal | `validate_path(Tainted) -> SafePath` | `SafePath` (canonicalized, allow-rooted) |
| HTML / template output | XSS | `html_escape(Tainted) -> Html` | `Html` (entity-escaped) |
| net target / `connect` | SSRF | `allowlist_host(Tainted) -> Host` | `Host` (allow-listed target) |
| log / `print` / output | log/header injection, also the SECRET sink (Axis 2) | `html_escape` / `sanitize_log -> Clean` | the clean type (and no `Secret`, Axis 2) |

Each clean type in the catalog is `#[sealed]` (REQ-8): it can be obtained ONLY by
calling its door, never by a `StructLit`. Also killed by the same mechanism: LDAP
injection, HTTP-header injection, unvalidated-deserialization (the deserialized
value is `Tainted` until validated). A tainted value reaching any of these sinks
un-sanitized — whether passed directly (`query(input)`) OR laundered through a
clean-type `StructLit` (`query(Sql { stmt: input.raw })`) — is a **compile-time
reject** (the un-typeable demo, GROUNDED below: the direct form is `L0`/`E0308` at
verus; the `StructLit` launder is `SpecError::SealedConstruction` at the
validator, REQ-8).

### Axis 2 — Confidentiality / SECRET (`Secret`, the dual)

A secret (password, key, token) carries a **secret mark** (`Secret`, the Stage-1
dual wrapper). A secret **cannot reach a PUBLIC output** (a log, an error message,
a network response, stdout — the Stage-3 `Write`/`Net`/`print` boundaries) without
an explicit, **AUDITED `declassify` door** (`declassify(Secret) -> Public`). The
public clean type `Public` is `#[sealed]` (REQ-8): the ONLY way to obtain a
`Public` is `declassify`, so `emit(Public { val: s.val })` (laundering a `Secret`
field into a `Public` struct, #77) is REJECTED — the door is the only release
point. A `Secret` reaching an `emit`/`print` boundary is the confidentiality flow
to forbid (`03-effect-stdlib.md` — the public-output sinks). Kills: logged
passwords, keys in responses, secrets in stack traces. (At v1.1, the mark
propagates the dual way: a `Secret` combined with ANYTHING stays secret — REQ-4.
At v1, both a DIRECT `emit(secret)` (type mismatch) and a `Public { … }` launder
(sealed-construction) are rejected.)

### Axis 3 — CAPABILITIES (`Authorized`)

A protected operation's parameter type demands a **proof-carrying capability
token** (`Authorized`) that ONLY the auth check produces — the op is un-callable
without it. `Authorized` is `#[sealed]` (REQ-8): only the `authorize` door mints
it, so `delete(Authorized { id: u.id })` (forging a capability via a `StructLit`,
#77) is REJECTED. Kills: missing authorization, IDOR
(insecure-direct-object-reference). The capability is the dual of a sink: where a
sink's parameter type demands the *absence* of a mark (the clean type, not the
tainted one), a protected op's parameter type demands the *presence* of a mark
(`Authorized`, only the `authorize` door produces it). The op's `req` (e.g. `req
c.ok`) discharges from the door's `ens` (e.g. `ens result.ok`).

### The unifying law — handled-or-loud, the COMPILE-TIME tooth (the loudest)

This stage instantiates, in SECURITY, the toolchain's unifying law (the **#62**
design-refinement principle, stated in `01-adts.md` and `03-effect-stdlib.md`):
**for every outcome a program models it either HANDLES it (a proven/checked path)
or SCREAMS (an explicit, typed, greppable refusal); silently doing the wrong
thing is structurally impossible.** A forbidden flow is HANDLED (routed through a
door — `parameterize`/`declassify`/`authorize`) or it is a **compile-time
SCREAM** (the program does not type-check / does not validate / does not certify —
`L0`/`FAILED`). The critic finding #77 caught a HOLE in this law: a `StructLit`
launder (`query(Sql { stmt: input.raw })`) was neither handled (no door) nor loud
(it certified `L3`) — a silent R-DEFER-9 launder. The `#[sealed]` barrier (REQ-8)
CLOSES that hole: minting a sealed clean type outside its door is now a loud
`SpecError::SealedConstruction`. This is the LOUDEST tooth (the same rung
`01-adts.md` REQ-5/REQ-12 owns for exhaustive `match`): the dangerous flow is
caught *before the program ships*. The SQLi program does not compile. The fiat
line is a KNOB: whatever flow you NAME (mark a source `Tainted`, a value `Secret`,
an op capability-gated, a clean type `#[sealed]`) the toolchain forces
handled-or-loud; the doors you trust are NAMED in the manifest (the §9 TCB). `grep
declassify` = every secret-release; `grep parameterize`/`grep sanitize` = every
taint-clearing; `grep #[sealed]` = every door-only clean type — the security TCB
is grep-complete (§8, GROUNDED via `forge audit`).

## The abstraction barrier — `#[sealed]` clean types (REQ-8, the corrected v1 centerpiece)

**The corrected claim (R-SPEC-4, critic finding #77).** The prior draft asserted
the v1 type-level enforcement was PURELY EMERGENT from SHIPPED machinery, needing
"NO new validator code" because "the type system is the flow rule." **That is
FALSE for the full guarantee.** The type system rejects the NAIVE direct form
(`query(input)` where `input: Tainted` → `E0308: expected Sql, found Tainted`), and
THAT rejection is emergent. But the clean types (`Sql`/`Public`/`Authorized`) as
ordinary Stage-1 newtype structs have ACCESSIBLE fields, so a `StructLit` mints a
clean type DIRECTLY from a marked value's field:

```thermite
fn launder(input: Tainted) -> u64 { query(Sql { stmt: input.raw }) }   // #77: certified L3, MUST be rejected
```

This bypasses the door entirely — `Sql { … }` is an `Expr::StructLit` (`struct
StructLit { path, fields }` in `ast.rs`), not a call to `parameterize`. The
emitted Verus type-checks (`Sql { stmt: u64 }` is a valid struct literal), so the
careless flow certifies `L3` while the marked value reaches the sink un-doored. The
critic (#77) reproduced this live on all three axes (`Sql`/`Public`/`Authorized`
StructLit bypasses each certify `L3`, must be `L0`). **The "un-typeable, emergent,
no new code" claim held only for the naive `query(input)` form; the full
door-is-the-only-launder-point guarantee (REQ-2) needs the barrier.**

**The fix — the standard capability/IFC abstraction barrier.** A clean (or
capability) type must be ABSTRACT: only the trusted door mints it. Thermite has no
module-privacy/visibility system to hide a struct's fields, so the barrier is a
DIRECT validator rule keyed off a new struct attribute:

- **`#[sealed]` struct attribute.** A `struct` may carry a `#[sealed]` attribute.
  AST: a new boolean flag `StructItem.sealed: bool` (`struct StructItem` in
  `ast.rs` currently carries `name` + `fields` + `inv` + `span` — verified — and
  gains `sealed`). Parser: `#[sealed]` on a `struct` sets the flag, mirroring the
  `#[slag]`/`#[boundary]` attribute precedent (`ffi-boundary.md` "Exact ast.rs /
  parser.rs additions" — `parse_attribute` generalized from `parse_slag`).
- **The validator rule.** A new `SpecError::SealedConstruction { name, span }`. The
  validator (`pub fn validate` in `validator.rs`) collects the set of `#[sealed]`
  struct names in its pre-pass (alongside the existing `struct_fields` collection,
  REQ-6), then in its `Expr::StructLit` walk arm (the validator already visits
  `Expr::StructLit` — `walk_expr_inner` / the contract and body walks, verified)
  REJECTS any `StructLit` whose `path` resolves to a `#[sealed]` struct, emitting
  `SealedConstruction { name, span }`. The rejection applies ANYWHERE in Thermite
  code — there is no "outside the door" carve-out needed, because a door is a
  `#[boundary]` fn with NO Thermite body (`body: None`, `external_body`); the door
  never contains a Thermite `StructLit` in-language. So the safe path
  (`query(parameterize(input))`) has no sealed-`StructLit` and is NOT rejected,
  while EVERY in-language attempt to mint a sealed clean type screams.
- **The corpus marking.** `Sql`/`Public`/`Authorized` (and the rest of the clean
  catalog) become `#[sealed]` structs. The `#[ignore]`d failing tests #77 pinned
  (the 3 `StructLit` bypasses, `forge/tests/divergence_provenance.rs`) must then be
  un-ignored and PASS (each launder → `SpecError::SealedConstruction` →
  `L0`/`FAILED`, never `L3`; R-DEFER-3).

**Why this is the type-level mechanism that makes REQ-2 TRUE.** REQ-2 states "no
mark-change exists outside a door — a value's mark is fixed at construction (the
struct literal) and changeable only by passing a door's return type." That is a
LIE unless the struct literal CANNOT mint a clean type. The `#[sealed]` rule makes
it true: a sealed type is obtainable ONLY as a `#[boundary]` door's return value,
so the door IS the only launder point.

**Re-drawing the v1 / v1.1 line.** The `#[sealed]` barrier is REQUIRED for the v1
centerpiece (not a v1.1 nicety) — without it the centerpiece "the SQLi program
doesn't compile" is false (it catches only the naive form). It is moved INTO v1
scope as REQ-8. The **dataflow-propagation engine** (taint through arbitrary
derived values — `let y = f(x); query(y)`) stays v1.1 (REQ-4): it is a harder
distinct capability (tracking a mark through ARBITRARY derived values, not just
the direct launder), and the v1 sealed barrier does not need it (a direct
`StructLit` launder is rejected by the barrier; a multi-hop derived flow that
erases the marked type back to a clean type without a door is the v1.1 engine's
job). REQ-8 closes the DIRECT door-bypass; REQ-4 closes the DERIVED-value flow.

## The layer map (6a / 6b / 6c)

- **6a — the marked types + the SEALED clean types + the door/sink DECLARATIONS
  (the vocabulary + the barrier).** The three marked types are per-axis Stage-1
  newtype `struct`s; the clean/capability types (`Sql`/`Public`/`Authorized`/…) are
  `#[sealed]` Stage-1 `struct`s; the doors and sinks are `#[boundary]` fns. NEW
  toolchain code: the `#[sealed]` attribute (`StructItem.sealed`) + parser support
  (REQ-8). Deliverable: the corpus `.th` declarations (clean types marked
  `#[sealed]`) + the skill grammar (§10, #7) that teaches the IFC vocabulary (the
  marks, the door verbs, the sink catalog, the `#[sealed]` clean types). REQ-1,
  REQ-2, REQ-3, REQ-8.
- **6b — the TYPE + SEAL ENFORCEMENT (reject raw-marked → sink AND sealed launder
  → door-bypass).** The lower→verus type-check rejects a raw marked value at a sink
  (`E0308`, emergent); the validator rejects a `StructLit` of a `#[sealed]` clean
  type (`SealedConstruction`, REQ-8, NEW); the doored value certifies. v1
  deliverable: the conformance corpus asserting the careless path FAILS
  (`L0`/`E0308`), the launder path FAILS (`L0`/`SealedConstruction`), and the
  doored path certifies (`L3`/to-boundary). The v1.1 dataflow-propagation pass
  (REQ-4) lands here too as additional NEW validator code. REQ-3 + REQ-8 (v1),
  REQ-4 (v1.1), REQ-5.
- **6c — the AUDIT TCB enumeration of doors + the centerpiece demo.** `forge audit`
  enumerates every reached door in the manifest `boundary_contracts` (name +
  target + req + ens + fx) — the grep-complete security TCB. SHIPPED
  (`Tcb::from_certificates`, GROUNDED below). REQ-6.

## The door-as-audited-TCB honesty (the honest ceiling)

A door is a **trusted point**. You trust `html_escape` to actually escape, you
trust `validate_path` to actually canonicalize-and-root, you trust `declassify`
to be an intentional release. The language proves the data CAN'T reach the sink
un-doored (a TYPE property at the sink PLUS the `#[sealed]` barrier at the clean
type, so the door is the ONLY mint point — GROUNDED + REQ-8); it TRUSTS the door
does its job. That trust is made honest exactly the way Stage 3 makes a syscall
honest (`03-effect-stdlib.md` "the door-as-TCB" = "the boundary-as-TCB"):

- **A door is a `#[boundary]`/`#[slag]` with a contract.** A sanitizer
  (`parameterize`, `html_escape`, `validate_path`, `allowlist_host`) is a
  `#[boundary]` fn whose contract STATES what it guarantees (e.g. `parameterize`'s
  `ens result.q == t.raw`), whose RETURN TYPE is the `#[sealed]` clean type (so the
  door is the only mint), and whose body is the trusted escaper. `declassify` and
  `authorize` likewise. The door's contract is L1-ENFORCED at the crossing
  (`ffi-boundary.md` REQ-4, the `lower_boundary_fn_l1` wrapper) — a door that
  violates its stated contract is caught at the boundary, not a free pass. This is
  the SAME legitimate-`external_body` distinction Stage 3 pins
  (`03-effect-stdlib.md` REQ-7, `boundary-composition.md` HONESTY ARGUMENT): the
  door is a declared trust boundary, NOT a `--no-cheating` core-logic cheat
  (R-DEFER-9). Because the door body is foreign (`external_body`, no Thermite
  `StructLit`), the door itself is the legitimate way the sealed type is minted —
  the `#[sealed]` rule does NOT block it.
- **Every door is enumerated in the audit manifest.** The doors are the security
  TCB — exactly where you trusted a sanitizer or released a secret. The
  `AuditManifest.tcb` `boundary_contracts` section (`audit-manifest.md` REQ-3,
  `Tcb::from_certificates`) enumerates each reached door: name + contract +
  foreign target + effect. `declassify` ESPECIALLY is audited (GROUNDED: `forge
  audit` of the secret program lists `['declassify', 'emit']`). This is the honest
  ceiling: not "no secret ever leaks" (you cannot prove the escaper), but "every
  secret-release passes a NAMED, contracted, enumerated door, the clean type is
  door-only-mintable (`#[sealed]`), and there are exactly THESE doors" (§9, the
  enumerable TCB).

The triple that makes a one-line door an honest TCB member is the Stage-3 triple
specialized to IFC: the door's guarantee is **stated** (its contract), the flow
through it is **typed AND sealed** (the mark changes only at the door's return
type, and the clean type is `#[sealed]` so no `StructLit` mints it), and the door
is **enumerated** (the manifest names it). A mark-change OUTSIDE a declared door —
whether a direct type-cast (impossible, distinct types) or a `StructLit` launder
(rejected by REQ-8) — is closed; a *derived-value* flow is the gap the v1.1
dataflow engine (REQ-4) rejects.

## How the marked types + doors are REPRESENTED (PINNED)

**The marked types are per-axis concrete newtype `struct`s, NOT generics.**
Thermite has **no user generics** — `StructItem` in `ast.rs` carries `name` +
`fields` + `inv` + `span` and NO type parameters (verified). A user cannot write
`struct Tainted<T>`. So the marked types are concrete Stage-1 wrappers (`struct
Tainted { raw: u64 }`, `struct Secret { val: u64 }`), and the clean/capability
types are concrete `#[sealed]` wrappers (`#[sealed] struct Sql { stmt: u64 }`,
`#[sealed] struct Public { val: u64 }`, `#[sealed] struct Authorized { ok: bool }`),
exactly the SHIPPED ADT form (`01-adts.md` REQ-1/REQ-8) plus the new `sealed` flag
(REQ-8). The `Type::Generic` node exists (`Option<usize>`) but is for the built-in
`Option`; the marked/clean types do not use it. This is the v1 line (OQ-1): per-T
concrete wrappers, the fixed three axes — NOT a `Marked<Tag, T>` phantom-generic
(un-expressible) and NOT a full lattice (OQ-3). The corpus uses `u64` payloads as
the grounding exemplar (the mechanism is identical for any payload type).

**The doors are `#[boundary]` fns (the SHIPPED FFI-boundary form), audited, and
the ONLY mint of a sealed clean type.** A door is a `#[boundary("ifc::parameterize")]
fn parameterize(t: Tainted) -> Sql ens result.q == t.raw fx pure ;` — a boundary
fn (`body: None`, `boundary: Some`, `FnItem.boundary` in `ast.rs`) whose foreign
body is the trusted escaper, whose return type is the `#[sealed]` clean type
(`Sql`), whose contract is L1-enforced, and which `forge audit` enumerates in the
TCB `boundary_contracts`. Because the door's body is foreign (`external_body`), it
contains no in-language `StructLit`, so it is the one legitimate way to obtain the
sealed type — the `#[sealed]` rule (REQ-8) rejects every OTHER (in-language
`StructLit`) attempt. `#[slag]` is the alternative for an in-language door with a
review reason. NO new fn node shape — the door reuses `struct BoundaryAttr` /
`struct SlagAttr` (SHIPPED); the only new node shape is `StructItem.sealed` (REQ-8).

**The sink enforcement is TYPE-LEVEL — the parameter type — PLUS the seal at the
clean type.** A sink demands the clean type by its PARAMETER TYPE (`fn query(s:
Sql)`): a raw `Tainted` argument is a type mismatch the lower→verus type-check
rejects (emergent); a `StructLit`-minted `Sql` is rejected by the `#[sealed]` rule
(REQ-8) so the only `Sql` the sink can be handed is a door-produced one. The
capability sink ALSO uses its `req` (`fn delete(c: Authorized) req c.ok`) which
discharges from the door's `ens result.ok`. The mechanism is the existing
type-checking + the NEW `#[sealed]` validator rule at v1, extended by the
dataflow-propagation pass at v1.1.

## What v1 ships vs the v1.1 validator-dataflow engine

This is the load-bearing honesty of this stage — be explicit about the line:

- **The DIRECT-SINK guarantee is v1 (GROUNDED type slice + the NEW `#[sealed]`
  rule).** Two rejections compose to make the centerpiece REAL: (1) the EMERGENT
  type-mismatch — a direct `query(input)` (raw `Tainted`) is `L0`/`E0308` at verus
  (GROUNDED end-to-end against the real `verus` binary); (2) the NEW `#[sealed]`
  validator rule — a `StructLit` launder `query(Sql { stmt: input.raw })` is
  `L0`/`SealedConstruction` at the validator (REQ-8, the fix for #77). Together
  they make "the door is the only launder point" TRUE: the ONLY way to hand a `Sql`
  to `query` is `query(parameterize(input))`, which CERTIFIES (`L3`/to-boundary).
  **v1 is this slice + the `#[sealed]` rule + the corpus + the skill grammar.** The
  prior draft's "NO new toolchain pass" claim is CORRECTED: v1 needs the one
  `#[sealed]` validator rule (#77).
- **The MARK-PROPAGATION engine is v1.1, NEW validator-dataflow work, NOT SMT.**
  That a tainted value flowing into a *derived* value STAYS tainted (`let y =
  f(x)` where `x: Tainted` makes `y` tainted), that a `Secret` *combined* with
  anything stays secret, that the mark propagates through assignment / function
  calls / ADT construction & destructuring / arithmetic — this is a DATAFLOW /
  type-propagation pass in `thermite-spec/src/validator.rs`, not a solver query. It
  is the CORE NEW WORK of v1.1 (more validator than SMT). It is DISTINCT from REQ-8:
  REQ-8 rejects a clean-type `StructLit` outright (no propagation needed — the
  construction site IS the launder); REQ-4 tracks a mark through arbitrary derived
  values and rejects at the point a derived marked value reaches a sink.

The v1 sealed barrier closes the DIRECT door-bypass (the `StructLit` launder, #77);
the v1.1 engine closes the DERIVED-value flow (the mark through intermediate
values).

## Requirements

### The marked types + the SEALED clean types + the doors (governs `thermite-syntax/src/ast.rs`)

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
  SHIPPED form), L1-enforced at the crossing (`ffi-boundary.md` REQ-4). **No
  mark-change exists outside a door — and this is TRUE only because the clean types
  are `#[sealed]` (REQ-8):** a value's mark is fixed at construction and changeable
  ONLY by passing a door's return type; the clean type CANNOT be minted by a
  `StructLit` (the #77 launder is rejected). Derived from §9 (the boundary contract
  is the interface) + §8 (the door is greppable/enumerable) + `ffi-boundary.md`
  REQ-2 (the SHIPPED boundary form) + `boundary-composition.md` (the door's
  contract composes) + REQ-8 (the seal that makes "only the door" true).
  **GROUNDED**: the `#[boundary("ifc::parameterize")]` door type-changes `Tainted ->
  Sql` and is enumerated by `forge audit`. **PINNED CORRECTION (#77)**: without
  REQ-8 this REQ does NOT hold (a `StructLit` launders `Tainted -> Sql` outside the
  door).

- **REQ-8 (v1 — the `#[sealed]` abstraction barrier — the clean/capability type is
  door-only-mintable):** a `struct` may carry a `#[sealed]` attribute; AST gains a
  boolean flag `StructItem.sealed: bool` (`struct StructItem` in `ast.rs`, today
  `name`+`fields`+`inv`+`span` — verified — plus `sealed`); the parser sets it
  mirroring the `#[slag]`/`#[boundary]` attribute precedent (`ffi-boundary.md`
  `parse_attribute`). The validator (`pub fn validate` in `validator.rs`) collects
  the `#[sealed]` struct-name set in its pre-pass and REJECTS any `Expr::StructLit`
  whose `path` is a `#[sealed]` struct — emitting a structured `SpecError::
  SealedConstruction { name, span }` — ANYWHERE in Thermite code (no carve-out: a
  `#[boundary]` door's body is foreign/`external_body` and contains no in-language
  `StructLit`, so the safe path is unaffected). A `#[sealed]` type is therefore
  obtainable ONLY as a `#[boundary]` door's return value. The clean types in the
  corpus (`Sql`/`Public`/`Authorized` + the rest of the catalog) become `#[sealed]`.
  This is the type-level abstraction barrier that makes "the door is the ONLY
  launder point" (REQ-2) TRUE; it is REQUIRED for the v1 centerpiece (NOT v1.1) —
  the standard capability/IFC abstract-type pattern (only the trusted door mints
  the capability). Derived from REQ-2 + §9 (the door is the only trust-change) +
  R-DEFER-9 (no silent launder) + critic finding **#77**. **NOT GROUNDED at v1**
  (the `#[sealed]` rule is unbuilt). The #77 `#[ignore]`d failing tests (the 3
  `StructLit` bypasses, `forge/tests/divergence_provenance.rs`) MUST be un-ignored
  and PASS (each → `SealedConstruction`/`L0`, R-DEFER-3). Owned by blocker **#77**.

### The sink catalog + the flow rules (governs `thermite-syntax/src/ast.rs`,
`thermite-spec/src/validator.rs`)

- **REQ-3 (v1 — the sink catalog — every sink's parameter type / `req` demands the
  CLEAN type):** each security sink is a `#[boundary]` whose PARAMETER TYPE (and,
  for the capability sink, its `req`) demands the SANITIZED/clean type, never the
  raw/tainted one: the SQL sink demands `Sql` (only `parameterize` produces it, and
  `Sql` is `#[sealed]` so nothing else mints it), the shell sink `Argv`, the path
  sink `SafePath`, the HTML sink `Html`, the net sink `Host`, the public-output
  sinks demand `Public` (not `Secret`, Axis 2). The protected-op sink (Axis 3)
  inverts it: its parameter type demands the PRESENCE of `Authorized` (only
  `authorize` produces it) + a `req cap.ok`. The sink demanding the clean type is
  just a boundary contract the caller verifies THROUGH
  (`boundary-composition.md` REQ-1); the seal (REQ-8) ensures the clean value the
  sink receives can ONLY be a door's. **GROUNDED end-to-end** (the full-path slice
  below): `query(s: Sql)` accepts only a `parameterize`-produced `Sql`; raw
  `Tainted` to `query` is `L0`/`FAILED` with `E0308`; the doored path `L3`. (The
  `StructLit` launder rejection is REQ-8, not yet grounded.) Derived from §4.1 (the
  effect `req`/`ens` row) + §9 + `03-effect-stdlib.md` (the sinks are boundary
  primitives) + `boundary-composition.md` REQ-1.

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
  type-propagation engine (more validator than SMT) — DISTINCT from REQ-8 (which
  rejects a clean-type `StructLit` at the construction site, no propagation): REQ-4
  tracks the mark through ARBITRARY derived values and rejects at the sink. Derived
  from §4.1 + §2.4 (crisp structured feedback) + `01-adts.md` REQ-5 (the
  validator's `SpecError` reject discipline) + the **#62**/**#76** IFC-dataflow
  decision. **v1.1, not v1** — the v1 slice (the type mismatch + the `#[sealed]`
  rule) does NOT need this for the direct/launder forms.

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
  proved. The `#[sealed]` flag (REQ-8) is a VALIDATOR concern — it is checked
  BEFORE lowering; the lowering of a `#[sealed]` struct is identical to a plain
  struct (`lower_struct`), so REQ-5 is unchanged by the seal. **GROUNDED**: the
  typed-sink + door + caller pattern lowers and certifies `L3` / to-boundary
  against `verus 0.2026.05.24`; the careless caller lowers to a `Tainted` argument
  at a `Sql` parameter, which verus rejects `E0308`. Derived from §3 (transpile to
  Verus) + `01-adts.md` REQ-8/REQ-9 (SHIPPED) + `boundary-composition.md` REQ-1 +
  the GROUNDED slice.

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
user input to a SQL sink **fails to certify** — both the naive direct form AND the
`StructLit` launder — and the same program routed through `parameterize` certifies.
The EXACT corpus + expected full-path output:

- **AC-1 (v1 — the SQLi program does NOT compile — the centerpiece, BOTH forms):**
  a corpus program `conformance/provenance/sqli.th` (`struct Tainted`, `#[sealed]
  struct Sql`, `#[boundary] parameterize(Tainted) -> Sql`, `#[boundary] query(Sql)
  -> u64`, `fn careless_path(input: Tainted) { query(input) }`) is REJECTED — the
  careless fn lowers and verus reports `error[E0308]: expected Sql, found Tainted`,
  the cert is `Level::L0` and the project assurance is `FAILED` (exit 1). The
  `StructLit` LAUNDER form (`fn launder_path(input: Tainted) -> u64 { query(Sql {
  stmt: input.raw }) }`) is ALSO REJECTED — the validator emits `SpecError::
  SealedConstruction { name: "Sql", .. }` (because `Sql` is `#[sealed]`), so the fn
  is `Level::L0`/`FAILED` and never reaches `L3` (REQ-8, the #77 fix). The SAME
  program with `fn safe_path(input: Tainted) { query(parameterize(input)) }`
  (`conformance/provenance/sqli_safe.th`) VALIDATES, lowers, and certifies — the
  doored fn is `Level::L3` / to-boundary (via `query`), the doors `parameterize`/
  `query` are `L1` boundaries, project assurance `L1` (min over functions), exit 0.
  **GROUNDED end-to-end (direct + doored forms)**: careless = `L0`/`FAILED`/`E0308`;
  safe = `L3`/`L1`/exit 0 (output pasted in Architecture). The launder-form
  rejection is **NOT GROUNDED at v1** (REQ-8 unbuilt; pinned by the #77 failing
  test). (REQ-1, REQ-3, REQ-5, REQ-7, REQ-8.)

- **AC-2 (v1 — a `Secret` reaching `emit` does NOT compile, direct OR laundered;
  declassified does + shows in the manifest):** `conformance/provenance/secret_leak.th`
  (`fn leak(s: Secret) { emit(s) }` where `emit(p: Public)`) is REJECTED
  (`L0`/`FAILED`, `E0308: expected Public, found Secret`); the `StructLit` launder
  `fn launder_emit(s: Secret) { emit(Public { val: s.val }) }` is ALSO REJECTED
  (`SpecError::SealedConstruction { name: "Public", .. }` — `Public` is `#[sealed]`,
  REQ-8/#77 fix); `secret_safe.th` (`fn safe_emit(s: Secret) { emit(declassify(s))
  }`) certifies (`L3`/to-boundary); and `forge audit secret_safe.th` enumerates
  `declassify` (and `emit`) in the `tcb` `boundary_contracts` (REQ-6 — every
  secret-release is in the manifest). **GROUNDED (direct + doored)**: leak =
  `L0`/`E0308`; safe = `L3`; audit lists `[declassify, emit]`. Launder-rejection
  NOT GROUNDED (REQ-8 unbuilt). (REQ-1, REQ-3, REQ-6, REQ-8.)

- **AC-3 (v1 — a protected op called without `Authorized` does NOT compile, direct
  OR forged):** `conformance/provenance/cap_missing.th` (`fn unauth_delete(u: User)
  { delete(u) }` where `delete(c: Authorized) req c.ok`) is REJECTED (`L0`/`FAILED`,
  `E0308: expected Authorized, found User`); the `StructLit` forge `fn
  forge_cap(u: User) { delete(Authorized { id: u.id }) }` is ALSO REJECTED
  (`SpecError::SealedConstruction { name: "Authorized", .. }` — `Authorized` is
  `#[sealed]`, REQ-8/#77 fix); `cap_safe.th` (`fn safe_delete(u: User) {
  delete(authorize(u)) }`) certifies — the op's `req c.ok` discharges from
  `authorize`'s `ens result.ok`. **GROUNDED (direct + doored)**: missing =
  `L0`/`E0308`; safe = `L3`. Forge-rejection NOT GROUNDED (REQ-8 unbuilt). (REQ-1,
  REQ-3, REQ-8.)

- **AC-7 (v1 — the `#[sealed]` barrier rejects every in-language clean-type
  `StructLit`; the door path is unaffected — the #77 fix, un-ignored):** the three
  `#[ignore]`d failing tests #77 pinned (`forge/tests/divergence_provenance.rs`:
  `taint_structlit_bypass_must_not_certify_l3`,
  `secret_structlit_bypass_must_not_certify_l3`,
  `capability_structlit_bypass_must_not_certify_l3`) are UN-IGNORED and PASS: each
  `StructLit` launder of a `#[sealed]` clean type (`Sql`/`Public`/`Authorized`)
  yields `SpecError::SealedConstruction` and certifies `L0`/`FAILED` (never `L3`;
  R-DEFER-3). The safe doored paths (`sqli_safe`/`secret_safe`/`cap_safe`) still
  certify `L3`/to-boundary — the door (a `#[boundary]` with a foreign
  `external_body`, no in-language `StructLit`) is the one mint that the seal does
  NOT block. A plain (non-`#[sealed]`) struct's `StructLit` is unaffected (AC-6).
  Hand-derived expectations (R-CHAR-3). **NOT GROUNDED at v1** (REQ-8 unbuilt).
  (REQ-8.)

- **AC-4 (v1.1 — mark propagation through a derived value rejects):** a tainted
  value flowed into a DERIVED value (`let y = passthru(x); query(y)` where `x:
  Tainted` and `passthru` returns the derived value still tainted) is REJECTED —
  the v1.1 validator-dataflow pass propagates the taint to `y` (REQ-4 propagation
  rule) and emits `SpecError::TaintReachesSink` at the sink, even though `y` is not
  syntactically the tainted source. A `Secret` combined into a derived value stays
  secret and rejects at a public output. Hand-derived expectations (R-CHAR-3). This
  is the v1.1 validator-dataflow engine's load-bearing behavior (DISTINCT from REQ-8:
  REQ-8 rejects a clean-type `StructLit` outright; REQ-4 tracks a mark through
  arbitrary derived values). **NOT GROUNDED at v1** (the engine is unbuilt). (REQ-4.)

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
  appears, no `#[sealed]` struct appears, no IFC flow is checked, byte-stable
  goldens. The `#[sealed]` rule (REQ-8) is a no-op on a struct WITHOUT the attribute
  (a plain `StructLit` is accepted exactly as today, REQ-6); the v1.1 additions (a
  new `SpecError` variant + a new validator pass) must be a no-op on mark-free
  programs. (All REQs; the security layer must not regress the kernel.)

## Architecture

Stage 6's **v1 owns ONE new toolchain mechanism** — the `#[sealed]` abstraction
barrier (REQ-8: the `StructItem.sealed` flag + the parser support + the
`SpecError::SealedConstruction` validator rule), the fix for critic finding #77.
The rest of v1 instantiates SHIPPED machinery (the Stage-1 wrapper types, the
`#[boundary]` doors/sinks, the #52 compose-through, the #15 door enumeration) as a
corpus + skill grammar. Stage 6's **v1.1 owns a SECOND new mechanism** — the
validator mark-propagation/rejection engine (REQ-4). The component spans three
crates, additively:

- **`thermite-syntax/src/ast.rs`** — the three marked types are Stage-1 concrete
  `struct` wrappers (`StructItem`, SHIPPED, `01-adts.md` REQ-1); the clean types
  add the NEW `StructItem.sealed: bool` flag (REQ-8 — `struct StructItem` today
  carries `name`+`fields`+`inv`+`span`, verified, and gains `sealed`); the doors
  are the SHIPPED `#[boundary]`/`#[slag]` form (`FnItem.boundary` / `FnItem.slag`,
  with `struct BoundaryAttr`/`struct SlagAttr` ALREADY in `ast.rs`). The marked
  types reuse the struct surface, the doors reuse the boundary surface; the ONLY new
  node shape at v1 is `StructItem.sealed`. NO user generics (`StructItem` has no
  type params — PINNED).
- **`thermite-spec/src/validator.rs`** — gains the `#[sealed]` rule at v1 (REQ-8):
  `pub fn validate` collects the `#[sealed]` struct-name set in its pre-pass
  (alongside the existing `struct_fields` collection, REQ-6) and its existing
  `Expr::StructLit` walk arm (the validator already visits `Expr::StructLit` —
  `walk_expr_inner` and the body/contract walks, verified) REJECTS a `StructLit` of
  a `#[sealed]` struct with the NEW `SpecError::SealedConstruction { name, span }`
  variant (`enum SpecError` today lists `UnknownCombinator`/`WrongArity`/… and
  gains `SealedConstruction`). The v1.1 mark-propagation/rejection pass (`pub fn
  validate` further extended): collect the marked-type set, propagate the mark
  through the dataflow of each `fn` body, and reject the forbidden DERIVED flows
  with the `TaintReachesSink` / `SecretReachesPublic` / `MissingCapability`
  variants. The caged-flat walk (`spectherm-combinators.md` REQ-6) is UNCHANGED.
- **`thermite-lower/src/lower.rs`** — the marked AND clean types lower to their
  Stage-1 wrappers via the SHIPPED `lower_struct` (`01-adts.md` REQ-8; a `#[sealed]`
  struct lowers identically to a plain struct — the seal is a validator concern,
  fired before lowering); the doors lower to `#[verifier::external_body]` signatures
  via the SHIPPED `lower_external_body_fn` (`boundary-composition.md` REQ-1). No new
  emission SHAPE — the direct type mismatch (`Tainted` arg at a `Sql` param) is
  rejected by verus's own type-check on the emitted source; the `StructLit` launder
  never reaches lowering (the validator rejects it first). UNCHANGED at v1
  (the seal is a validator add, not a lowering add); UNCHANGED at v1.1.
- **`forge/src/audit.rs`** — the doors are enumerated by the SHIPPED
  `Tcb::from_certificates` (`audit-manifest.md` REQ-3) — UNCHANGED, the doors are
  boundaries the existing TCB enumeration already lists (GROUNDED).

Symbol anchors: `struct StructItem` (gains `sealed`) / `struct BoundaryAttr` /
`struct SlagAttr` / `enum SpecError` (gains `SealedConstruction`) / `Expr::StructLit`
in `ast.rs`; `pub fn validate` in `validator.rs` (the `#[sealed]` rule at v1 + the
mark-propagation pass at v1.1); `lower_external_body_fn` / `lower_struct` in
`lower.rs`; `Tcb::from_certificates` in `audit.rs`.

### The full-path type-level slice (GROUNDED — real `forge` + `verus 0.2026.05.24`)

The v1 type-level enforcement of all three axes' DIRECT form was run END-TO-END
through the real toolchain during authoring (`forge check` / `forge audit`, the
real `verus 0.2026.05.24.ecee80a` binary on PATH; scratch removed — `forge`'s
`ScratchDir` Drop guard cleans each run, `/tmp` scratch deleted). This is the seed
for the golden lowering; it proves the direct-form type-level slice GROUNDS at the
toolchain level. The `#[sealed]` `StructLit`-launder rejection (REQ-8) is NOT
grounded here — it is the unbuilt fix for #77, pinned by the `#[ignore]`d failing
tests.

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

**TAINT axis — the SQLi careless DIRECT path (`sqli.th`, `query(input)`) — does
NOT certify (the un-typeable demo at the TOOLCHAIN level):**

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

**RECORDED FINDING (direct form).** The careless DIRECT SQLi path PASSES parse +
validate + effect-check (the validator has no marked-type knowledge and accepts it)
and is rejected at the LOWERING/verus TYPE-CHECK — `Tainted` is not `Sql`, and only
`parameterize` produces `Sql`. This direct-form rejection needs NO new validator
code (emergent). The doored path certifies `L3`/to-boundary.

**RECORDED FINDING (the `StructLit` launder — #77, the corrected claim).** The
launder form `query(Sql { stmt: input.raw })` CERTIFIES `L3` on the toolchain as
built (the critic reproduced this live on all three axes), because `Sql` is an
ordinary struct with accessible fields and the emitted Verus `StructLit`
type-checks. This is a R-DEFER-9 silent launder — the marked value reaches the sink
un-doored, neither handled nor loud. The fix is the `#[sealed]` validator rule
(REQ-8): with `Sql` marked `#[sealed]`, the validator emits `SpecError::
SealedConstruction` and the launder is `L0`. This is why the prior "emergent, no new
validator code" claim is CORRECTED — the full guarantee needs the barrier (a NEW
validator rule). The DERIVED-value flow (a multi-hop tainted value reaching the
sink) remains the v1.1 dataflow pass (REQ-4).

**SECRET axis (`secret_safe.th` / `secret_leak.th`):** `safe_emit(s) {
emit(declassify(s)) }` certifies `L3`; `leak(s) { emit(s) }` is `L0`/`FAILED` with
`error[E0308]: expected Public, found Secret`. `forge audit secret_safe.th --json`
lists the doors `[declassify, emit]` in the `tcb` `boundary_contracts`. (The
`emit(Public { val: s.val })` launder certifies `L3` today — the #77 hole — and
becomes `L0`/`SealedConstruction` once `Public` is `#[sealed]`, REQ-8.)

**CAPABILITY axis (`cap_safe.th` / `cap_missing.th`):** `safe_delete(u) {
delete(authorize(u)) }` certifies `L3` (the op's `req c.ok` discharges from
`authorize`'s `ens result.ok`); `unauth_delete(u) { delete(u) }` is `L0`/`FAILED`
with `error[E0308]: expected Authorized, found User`. (The `delete(Authorized { id:
u.id })` forge certifies `L3` today — #77 — and becomes `L0`/`SealedConstruction`
once `Authorized` is `#[sealed]`, REQ-8.)

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

The three axes are ONE mechanism — a marked type, a `#[sealed]` clean type, a door
(a `#[boundary]` that is the only mint of the clean type), a sink whose parameter
type encodes the flow rule. The direct form is confirmed end to end through the
real `forge`/`verus` binaries; the `StructLit`-launder closure (REQ-8) is the
unbuilt #77 fix.

## Dependency hooks (the Stage 1 / 3 / 5 wiring)

- **Stage 1 (marked + clean types — `01-adts.md`):** `Tainted` / `Secret` /
  `Authorized` / `Sql` / `Public` / … ARE Stage-1 wrapper ADTs (REQ-1/REQ-8 SHIPPED
  — a `struct`/newtype over the carried value; the mark a TYPE property). The clean
  types add the NEW `StructItem.sealed` flag (REQ-8) — a small extension to the
  SHIPPED `StructItem`. The marked/clean types lower via the SHIPPED Stage-1
  `lower_struct` (REQ-8). Stage 6 v1 rides Stage 1 plus the sealed flag.
- **Stage 3 (the sinks — `03-effect-stdlib.md`):** the SINKS are Stage-3
  effect-primitive `#[boundary]`s (the SQL/shell/file/net/log primitives); each
  sink's parameter type demands the CLEAN (`#[sealed]`) type. A `Secret` reaching a
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
  0.2026.05.24.ecee80a`, scratch removed).** The v1 type-level slice's DIRECT form —
  all three axes — was run END-TO-END through `forge check` / `forge audit` against
  the real binaries: the doored paths certify (`L3`/`L1`/to-boundary, exit 0), the
  careless DIRECT paths are `L0`/`FAILED`/`E0308` (exit 1), and `forge audit`
  enumerates the doors in the TCB. The `StructLit`-launder rejection (REQ-8) is the
  unbuilt #77 fix — pinned by the `#[ignore]`d failing tests, NOT grounded.
- **AC-1/AC-2/AC-3/AC-5 (v1, direct + doored):** `cargo test -p forge` over a new
  `conformance/provenance/` corpus, shelling the real `verus` binary on the emitted
  lowering of the doored programs (assert exit 0 + `L3`/to-boundary, R-CODE-4) and
  asserting the careless DIRECT programs are `L0`/`FAILED` with an `E0308`-class
  verus type error, plus `forge audit` enumerating the doors in the TCB.
- **AC-7 (v1, the `#[sealed]` barrier / #77 fix):** the three `#[ignore]`d failing
  tests at `forge/tests/divergence_provenance.rs` are un-ignored and assert each
  `StructLit` launder of a `#[sealed]` clean type yields `SpecError::
  SealedConstruction` / `L0` (never `L3`); the doored paths stay `L3`. Plus a
  `thermite-spec` unit fixture: a `#[sealed]` struct's `StructLit` → one
  `SealedConstruction`; a plain struct's `StructLit` → accepted (no regression).
- **AC-4 (v1.1):** validator-dataflow reject fixtures (hand-derived expectations,
  R-CHAR-3) exercising mark propagation through a derived value — gated on the
  REQ-4 engine being built.
- **AC-6 (v1):** the existing `conformance/{sum,binary_search,shape,bank_account}`
  certs stay byte-stable (the `#[sealed]` rule is a no-op on a struct without the
  attribute; v1.1's validator pass is a no-op on mark-free programs).
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

The `validator.rs` route is needed for v1 REQ-8 (the `#[sealed]` rule) AND v1.1
(REQ-4); the `ast.rs` route covers the `#[sealed]` flag + the v1 corpus; the
`lower.rs` route covers the golden. The orchestrator authors
`conformance/provenance/cases.json` (the oracle this doc's ACs cite), the
`conformance/provenance/{sqli,sqli_safe,secret_leak,secret_safe,cap_missing,cap_safe}.th`
programs (clean types marked `#[sealed]`), their `.cert.json` goldens, and the
`tests/golden/lower/sqli_safe.verus.rs` golden (hand-authored from the GROUNDED
slice, confirmed to pass `verus`), BEFORE the builder runs (R-CHAR-3). This doc does
NOT author the oracle, the goldens, or the routes (R-DOC-1).

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (v1 — the three marked types — `Tainted`/`Secret`/`Authorized`) | NOT-STARTED | epic **#62** / issue **#76** Stage 6. No `Tainted`/`Secret`/`Authorized` type in the tree or corpus (`grep -r "Tainted\|Secret\|Authorized"` over `.rs`/`conformance` returns NONE). The SUBSTRATE is SHIPPED (`struct StructItem` newtype with `name`+`fields`+`inv`+`span`, `01-adts.md` REQ-1/REQ-8 SHIPPED, no user generics — PINNED) and GROUNDED through the full path (`struct Tainted { raw: u64 }` certifies `L3` in `sqli_safe`); the v1 deliverable (the corpus declarations + skill vocabulary) is not authored. |
| REQ-2 (v1 — the doors — only mark-changing ops, contracted `#[boundary]`/`#[slag]`) | NOT-STARTED | epic **#62** / issue **#76** Stage 6. No `parameterize`/`declassify`/`authorize` door exists in the corpus (`grep -r "declassify\|sanitize"` returns NONE). The SHIPPED door substrate (`struct BoundaryAttr`/`struct SlagAttr` in `ast.rs`, `FnItem.boundary`/`.slag`, `ffi-boundary.md` REQ-2 SHIPPED) is the form, GROUNDED (the `#[boundary] parameterize(Tainted) -> Sql` door type-changes the mark and is audit-enumerated). **CORRECTED (#77):** "only the door changes a mark" is TRUE only with the `#[sealed]` barrier (REQ-8) — without it a `StructLit` launders `Tainted -> Sql` outside the door (certified `L3` today, must be `L0`). Depends on REQ-8. |
| REQ-3 (v1 — the sink catalog — every sink's param type / `req` demands the CLEAN type) | NOT-STARTED | epic **#62** / issue **#76** Stage 6. No security sink exists in the corpus. The sink-demands-clean-type mechanism's DIRECT form is GROUNDED end-to-end: `query(s: Sql)` rejects raw `Tainted` (`L0`/`FAILED`/`E0308`) and accepts a `parameterize`-produced `Sql` (`L3`) through the real `forge`/`verus`. The `StructLit`-launder rejection at the sink's clean type needs REQ-8 (the `#[sealed]` rule). The corpus is not authored. |
| REQ-8 (v1 — the `#[sealed]` abstraction barrier — clean type is door-only-mintable) | NOT-STARTED | **blocker #77** (the abstraction-barrier fix). No `#[sealed]` attribute, no `StructItem.sealed` field (`struct StructItem` carries `name`+`fields`+`inv`+`span` only — verified), and no `SpecError::SealedConstruction` variant (`enum SpecError` lists `UnknownCombinator`/`WrongArity`/`WrongArgKind`/`ForbiddenCall`/`NestedCombinator`/`ExpressionTooDeep`/`UnsupportedAdt`/`UnknownField`/`UnknownVariant`/… — verified, no IFC/seal variant) exist. The validator already visits `Expr::StructLit` (`walk_expr_inner` arm + the body/contract walks, verified) so the seal check is a natural add. **REQUIRED for the v1 centerpiece** (NOT v1.1): without it the `StructLit` launder (#77, reproduced live: `query(Sql { stmt: input.raw })` certifies `L3` on all 3 axes, must be `L0`) defeats "un-typeable." The #77 `#[ignore]`d failing tests must be un-ignored + passing (R-DEFER-3). |
| REQ-4 (v1.1 — validator mark-PROPAGATION + REJECTION engine — the core new work) | NOT-STARTED | epic **#62** / issue **#76** Stage 6, **v1.1** (NOT v1). `thermite-spec/src/validator.rs` has no taint/secret/capability propagation pass and no `TaintReachesSink`/`SecretReachesPublic`/`MissingCapability` `SpecError` variant. This is the NEW dataflow engine (NOT SMT) — DISTINCT from REQ-8 (REQ-8 rejects a clean-type `StructLit` at the construction site, no propagation; REQ-4 tracks a mark through arbitrary derived values and rejects at the sink). Compile-time tooth of handled-or-loud for derived flows. |
| REQ-5 (v1 — marks lower to Stage-1 wrappers; doors lower to `external_body`) | NOT-STARTED | epic **#62** / issue **#76** Stage 6. No marked/clean type or door in the corpus. The mechanism is SHIPPED + GROUNDED (`lower_struct` for the wrapper, `01-adts.md` REQ-8 SHIPPED — a `#[sealed]` struct lowers identically, the seal is a validator concern; `lower_external_body_fn` for the door, `boundary-composition.md` REQ-1; the careless DIRECT path's `Tainted`-arg-at-`Sql`-param is rejected by verus `E0308` on the emitted source). The corpus/golden is not authored. |
| REQ-6 (v1 — the doors are the security TCB — enumerated in the manifest) | NOT-STARTED | epic **#62** / issue **#76** Stage 6. The SHIPPED `Tcb::from_certificates in forge/src/audit.rs` (`audit-manifest.md` REQ-3) enumerates boundary contracts as the TCB — GROUNDED for IFC: `forge audit sqli_safe.th --json` lists `[parameterize, query]`, `secret_safe.th` lists `[declassify, emit]` (name + target + req + ens + fx). No IFC corpus exists to audit yet; `grep declassify` = the door list once the corpus lands. |
| REQ-7 (v1 — marks compose through the call graph — the Stage-5 hook) | NOT-STARTED | epic **#62** / issue **#76** Stage 6. The SHIPPED #52 compose-through (`reachable_fn_deps in check.rs`, `05-composition.md` REQ-1) + the #15 deep-graph TCB aggregation (`05-composition.md` REQ-7) are the mechanism — GROUNDED: `safe_path` composes `parameterize` + `query` across the graph and certifies `L3`/to-boundary. No IFC corpus program composes a mark yet. |

## Open questions (for the orchestrator before the builder runs)

- **OQ-1 (RESOLVED — marked/clean type as a concrete newtype, NOT a generic; the
  seal is an attribute flag).** Thermite has NO user generics (`StructItem` carries
  no type params — verified). The marked types are concrete per-axis Stage-1 newtype
  `struct`s; the clean types are concrete `#[sealed]` `struct`s (`StructItem.sealed`,
  REQ-8). A `Marked<Tag, T>` phantom-generic is un-expressible. The §10 6k-token
  skill must hold the IFC grammar (the marks + the door verbs + the sink catalog +
  the `#[sealed]` clean types) — a real budget check at Stage 6's skill regeneration
  (#7); the surface is small (a handful of types + door verbs + one attribute) and
  expected to fit. Not a blocker.

- **OQ-2 (least-confident: the v1.1 mark-propagation engine's REACH — implicit
  flows, marks through arithmetic/ADTs).** REQ-4 (v1.1) is the highest-judgment,
  least-confident part. The v1 EXPLICIT-direct slice is GROUNDED (a tainted value
  passed DIRECTLY to a sink is a type error) and the v1 `StructLit`-launder is closed
  by REQ-8. The v1.1 open reach: (a) IMPLICIT flows — a `Secret` that influences a
  CONTROL path (`if secret > 0 { print("hi") }` leaks one bit) — v1.1 LEANS to
  tracking EXPLICIT data-flow only (the value reaching the sink), NOT
  implicit/control-flow leaks (a much harder non-interference property, future work
  like constant-time below); (b) marks through ARITHMETIC and ADT
  construct/destructure — `Tainted(a) + b` is tainted, `match t { ... }` on a tainted
  scrutinee taints the bindings — these are tractable explicit-flow rules the v1.1
  engine must pin precisely; (c) the v1.1 LINE — explicit data-flow propagation
  through assignment/call/arith/ADT, rejecting at sinks, is v1.1; implicit/control-flow
  and full lattice IFC are OUT. The builder must pin the propagation rules
  mechanically (a fixture per rule, AC-4). This is the REQ I am LEAST confident is
  fully specified.

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
  ceiling, now with the seal).** The language proves the data CAN'T reach the sink
  un-doored (a TYPE property at the sink PLUS the `#[sealed]` seal at the clean type
  — REQ-8 closes the `StructLit` launder, so the door is the ONLY mint); it TRUSTS
  the door does its job (the escaper actually escapes). The door's contract is
  L1-enforced at the crossing (`ffi-boundary.md` REQ-4), but that L1 check verifies
  the door's STATED contract, which for a sanitizer is itself a trust statement (you
  cannot prove `html_escape` escapes all XSS vectors — that is the fiat the manifest
  enumerates, REQ-6, GROUNDED). The open question: how strong is a sanitizer's STATED
  contract (a shape claim, like Stage-3's syscall contracts, or a stronger property)?
  LEANING: a shape claim ("the result is the `Sql`/`Html` clean type") + the door
  enumerated in the TCB — the same honest ceiling as Stage 3 (`03-effect-stdlib.md`
  REQ-3, outcome-coverage). The honesty is "every flow passes a NAMED door (and ONLY
  a door, by the seal)," not "the door is provably correct." Not a blocker; the
  builder pins the door-contract strength against the `conformance/provenance` oracle.
