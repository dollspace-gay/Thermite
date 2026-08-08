# Correspondence-backed L3 build artifacts — compile the verified body

<!--
tier: 3-component
status: shipped
audited-content-sha256: 066f9070f1a30126f189e55537053987ef3df450074d7dd901329e10a6c83743 (re-pinned 2026-08-08 after the closed result-enum public ABI landed at the L3 export admission site)
decision: Option A — compile the canonical Verus executable body that was verified
issue: github:dollspace-gay/Thermite#101, github:dollspace-gay/Thermite#103, github:dollspace-gay/Thermite#104, github:dollspace-gay/Thermite#108, github:dollspace-gay/Thermite#111
governs:
  - forge/src/verified_build.rs (new)
  - forge/src/build.rs
  - forge/src/cli.rs
  - forge/src/closure.rs
  - forge/src/manifest.rs
  - thermite-lower/src/lower.rs
  - thermite-lower/tests/l3_library.rs
thesis-refs:
  - thermite-design.md §3
  - thermite-design.md §5.3
  - thermite-design.md §6
  - thermite-design.md §9
-->

## Summary

This design closes Forge's proof-to-executable gap by making the L3 Verus
emission the single source of truth for both proof and code generation. A
successful L3 build emits one canonical Verus library crate, runs every
correspondence and closure gate against that crate's frozen input plan, and
invokes Verus with `--no-cheating --compile`. The body accepted by Verus is
therefore the body Verus hands to rustc/LLVM. Forge never proves
`thermite_lower::lower(program)` and then ships the independently generated
`thermite_lower::lower_l1(program)` body.

The result is an atomic verified-build bundle containing:

- a compiled library artifact;
- the exact Verus source compiled;
- the canonical artifact plan and reachable closure;
- the per-item proof and translation-validation evidence;
- a versioned, cryptographically bound `VerifiedBuildReceiptV1`.

An L3 artifact exists only when the whole exported executable closure achieves
the L3 claim. Downgrades, skipped or unverifiable translation validation, open
holes, unresolved calls, `#[slag]`, `#[boundary]`, proof cheating, and
post-proof mutation are hard build failures. There is no "L3 build with
caveats" mode.

The existing `forge build` behavior remains the explicit L1 path. It continues
to call `lower_l1` and bake in always-active runtime checks. This design does
not prove refinement between the L3 and L1 lowerers and does not use an L1
artifact to satisfy an L3 request.

Issue #104 extends this pipeline additively for exact-source rich-state
composition. A composition build has the distinct plan schema
`thermite.combined-artifact-plan.v1` and receipt schema
`thermite.verified-composition-receipt.v1`; it binds canonical Thermite lowering
and exact direct-Verus shell bytes into the final caller proof-and-compile
input. Registry-v2 safe sequential primitives may be proven and emitted first
as separate crates; their authored/generated sources, interface, rlib, and
object members are bound before the caller imports them. Ordinary L3 builds
omit the optional composition fields and retain this document's original
schemas and semantics. The composition-specific policy, visibility, inventory,
and acceptance contract live in
`.design/build/l3-rich-composition.md`.

Issue #108 extends only the kernel composition toolchain boundary. Kernel
builds still use `--no-vstd --no-cheating`, but explicitly import the pinned
vstd VIR proof model and pair it with a deterministic erased `no_std` metadata
rlib. This makes native `&[u8]` length and indexing specifications available
without adding hosted runtime code. The model/source/metadata binding and byte
slice acceptance matrix live in `.design/build/kernel-byte-slice.md`.

## Decision

Issue #101 considered two sound ways to close the gap:

1. compile the verified executable body; or
2. prove a refinement theorem from the independently compiled L1 body to the
   L3 body.

This document chooses **Option A**.

The current L3 lowering already emits executable Verus `exec` functions, and
Verus can both verify and compile a library crate. Option A removes the
cross-lowering proof obligation entirely. Option B would require a maintained
semantic relation across two large, independently evolving lowerers before
Forge could make the same artifact claim. Existing divergence pins such as
`thermite-lower/tests/divergence_bytes_eq_l1_empty_window.rs` and
`thermite-lower/tests/divergence_numfmt_zero.rs` demonstrate why equivalence
must not be assumed.

### Non-goals

- Proving `lower_l1` refines the L3 lowering.
- Relabeling the current L1 artifact or `BuildManifest` as L3.
- Compiling a second Rust reconstruction after Verus succeeds.
- Supporting cross-file or package dependency closure in v1.
- Claiming a compiler-independent Rust ABI.
- Embedding a signature or organizational identity in the receipt. Hash
  binding is required; authenticity signing is a separate future layer.
- Shipping partial support by silently omitting an unsupported verifier or TV
  phase.

## Assurance statement and trust boundary

A successful bundle may state:

> Every executable function and generated wrapper reachable from the declared
> exports was translation-validated, verified without Thermite escape hatches,
> and compiled from the exact canonical Verus source recorded in this bundle.
> The artifact correspondence level is L3 and its scope is end-to-end within
> the recorded closure and pinned toolchain.

The claim is deliberately capped at L3. A contract clause may carry additional
L4 reconstruction evidence, but executable-body correspondence is established
by the Verus proof-and-compile path. L4 clause evidence does not turn the
compiled artifact into an "L4 artifact."

The trusted computing base recorded in the receipt includes Forge's parser,
validators, closure planner and emitter, Verus and its erasure/codegen bridge,
rustc/LLVM, Z3, the selected target libraries, and any pinned vstd components.
Thermite `#[boundary]` and `#[slag]` bodies are not added to that trust base for
this mode: reaching either is a build rejection.

The host Rust selection and the artifact code generator are separate trust
domains. The former is the `rustc` selected by the invoking shell and is
recorded only as diagnostic provenance. The latter is the rustup toolchain
named by the pinned Verus distribution's authoritative `Toolchain:` line; its
compiler, sysroot, target libraries, driver and LLVM closure are assurance
inputs. Host provenance must never stand in for artifact-codegen evidence.

## User surface

The additive CLI is:

```text
forge build <file.th> --level l3 \
  --export <fn> [--export <fn> ...] \
  [--crate-name <name>] \
  [--target std|kernel] \
  [--out <bundle-dir>] \
  [--json]
```

Examples:

```text
forge build math.th --level l3 --export checked_add --out dist/math.verified
forge build page.th --level l3 --export render --export size --target kernel \
  --out dist/page-kernel.verified
```

Rules:

- Omitting `--level l3` preserves today's L1 build behavior. The existing
  command is reported unambiguously as L1.
- At least one `--export` is required for an L3 build. Repeated flags define the
  closure roots in command-line order; the plan stores a canonical sorted form.
- `--entry`, `--sandbox`, `--no-sandbox`, and `--sandbox-self-test` are rejected
  with `--level l3` in v1. The verified deliverable is a library, not a
  generated sample runner.
- `--out` names a bundle directory, not a bare artifact, for L3. Without it,
  Forge uses the existing stable per-run output location and reports it.
- `--target kernel` selects the `no_std + alloc`, `rlib`, `panic=abort` profile.
- `--crate-name` defaults to the sanitized source stem. The chosen name is part
  of the ABI record and receipt.
- `--json` changes console rendering only. It does not suppress the receipt or
  evidence files.

A read-only validator is added:

```text
forge verify-build <bundle-dir> [--replay] [--json]
```

The default mode validates the canonical receipt and every recorded digest.
`--replay` additionally reruns the pinned verification/compilation pipeline and
requires the resulting artifact digest to match.

## Architecture

### Pipeline

```text
source bytes
   │
   ▼
parse + validate + effect-check
   │
   ▼
ArtifactPlanV1: exports + complete reachable closure + ABI/wrapper plan
   │                         │
   │                         └── fail closed on unresolved or excluded nodes
   ▼
strict proof and translation-validation gates
   │
   ▼
canonical Verus crate emission from the frozen plan
   │
   ├── compare emitted bytes with the frozen expected-source digest
   ▼
one Verus invocation: --no-cheating --compile
   │
   ├── proof failure ───────────────► publish nothing
   ├── compile failure ─────────────► publish nothing
   └── source rehash mismatch ──────► publish nothing
   ▼
hash artifact + evidence + toolchain
   │
   ▼
stage VerifiedBuildReceiptV1 and bundle
   │
   ▼
fsync + atomic rename into the final output directory
```

`forge/src/verified_build.rs` owns this orchestration. It may reuse parsing,
validation, effect checking, scratch-directory handling, certificate data and
rendering from existing modules, but it must not call `lower_l1`.

### `ArtifactPlanV1`

The plan is the frozen, canonical executable IR for a build. It is produced
once from the parsed program and is immutable after its digest is computed. It
contains at least:

- schema version and source digest;
- normalized parsed-program digest;
- crate name, target, crate type and panic strategy;
- declared exports and their source addresses;
- every reachable executable function, generated helper and wrapper;
- every required type, spec definition and proof dependency;
- a complete call/dependency edge list;
- per-node body, contract and effect-row digests;
- the export ABI signature and precondition-wrapper choice;
- explicit inclusion/exclusion records for source items;
- the strict gate policy and expected gate inventory.

Lists are deterministically ordered by semantic address unless source order is
part of the language meaning. Hash maps, filesystem iteration order, wall-clock
values and process IDs cannot affect the canonical bytes.

The plan is intentionally smaller than a new shared compiler IR. It controls
selection, closure, exports and generated wrappers while reusing the current L3
AST-to-Verus lowering for expression and statement emission.

### Complete reachable closure

The closure roots are the explicit exports. Closure construction includes:

- direct and transitive executable calls;
- generated executable helpers used by those calls;
- contract/spec definitions needed to verify them;
- type definitions and monomorphized support code required to compile them;
- generated total export wrappers;
- compiler-generated runtime support that can affect behavior.

The current intra-file closure behavior that ignores unresolved or cross-file
calls is not valid for an L3 artifact. In this mode, an unresolved callee,
ambiguous resolution, cross-file dependency, unsupported indirect call, or
missing generated helper is a named `IncompleteClosure` failure.

Unreachable functions are omitted from the emitted crate and cannot lower the
artifact headline. The receipt binds both the raw source and the exact included
closure, so omission is visible and cannot be confused with whole-file
certification.

## Strict gates

Every expected gate has one of two outcomes: `pass` or build failure. There is
no skipped-success state.

| Gate | Required result | Hard failures include |
|---|---|---|
| Parse/spec/effects | clean | recovery diagnostics, invalid contracts, effect errors |
| Closure | complete and end-to-end | unresolved/cross-file calls, indirect-call uncertainty |
| Source completeness | complete | body/proof holes, missing generated definitions |
| Escape-hatch scan | none reachable | `#[slag]`, `#[boundary]`, `external_body`, `assume`, `admit`, axiom injection |
| Termination | verified form | `fx diverge`, `decreases *`, no-decreases exemptions |
| Function certificates | L3 or stronger, not degraded | L0/L1/L2, timeout degradation, reject/counterexample |
| Contract TV | faithful for every reachable clause | divergent, unsupported, skipped, unverifiable |
| Exec-expression TV | faithful for every reachable expression class | divergent, unsupported, skipped, unverifiable |
| Body/statement TV | faithful for every reachable body | divergent, unsupported, skipped, unverifiable |
| Loop TV | faithful for every reachable loop obligation | divergent, unsupported, skipped, unverifiable |
| Export wrapper proof | verified and total | non-executable precondition, unproved wrapper branch |
| Whole-crate Verus | zero errors under strict flags | proof failure, cheating diagnostic, erasure failure |
| Code generation | artifact produced | Verus/rustc/LLVM failure |
| Binding | all hashes agree | source, plan, evidence, tool or artifact mismatch |

The escape-hatch rule applies to Thermite-originated and Forge-generated
source. Pinned Verus/vstd implementation internals remain part of the recorded
toolchain trust base; they do not authorize Forge to emit a Thermite
`external_body`, assumption or axiom.

The implementation calls the TV libraries directly and records structured
per-node verdicts. It does not infer success from the fact that standalone
commands were not run. If a current TV implementation cannot classify a
reachable construct, the initial L3 artifact language is narrower than the
general Thermite language and the build fails with
`UnsupportedForVerifiedBuild`.

Cached per-item proofs or TV results may be used only when their full
content-addressed identity matches the plan and strict policy. Regardless of
cache hits, the final whole-crate Verus proof-and-compile invocation always
runs. A cached `forge check` certificate can never authorize compilation of
different bytes.

## Exact-source proof and compilation

The correspondence invariant is:

```text
sha256(source verified by final Verus invocation)
  == sha256(source compiled by that invocation)
  == receipt.binding.verus_source_sha256
```

Forge writes the canonical source into a private scratch directory, hashes it,
checks that hash against a fresh emission from the frozen plan, and invokes
Verus once with the same path:

```text
verus --output-json --no-cheating --compile <canonical-source>
```

Pinned solver resource, seed, target and codegen flags are added to that
invocation and recorded exactly in the receipt. Forge keeps Verus's default
erasure checks enabled. It must not pass `--no-erasure-check` or an equivalent
escape hatch.

After Verus exits, Forge re-reads and hashes the source path before accepting
the artifact. A mismatch is a mutation failure even when the mutated program
would independently verify. This detects unauthorized changes rather than
merely contract-breaking changes.

The final whole-crate result is authoritative. Existing per-item certificates
remain useful evidence and diagnostics, but they do not replace proof of the
exact emitted crate, including generated wrappers and executable helpers.

Whole-crate error counts are structured evidence, not inferred presentation.
Forge records `verification-results.errors` when Verus supplies it and treats a
missing field as unknown. A frontend rejection therefore retains its diagnostic
without a fabricated numeric suffix, while strict success and receipt validation
still require an explicit zero. The detailed compatibility and negative matrix
are specified in `.design/build/verus-error-accounting.md`.

For a kernel target, the exact invocation additionally carries `--no-vstd`, an
explicit `--import vstd=<pinned-vstd.vir>`, and
`--extern vstd=<generated-no-std-rlib>`. The imported VIR is the semantic
authority. The erased rlib supplies only the matching Rust metadata needed by
code generation; it is built deterministically from a Forge-owned source whose
digest and normalized command are receipt-bound.

No independently reconstructed Rust source may appear between successful
verification and code generation. If a future Verus integration exposes
post-erasure Rust/MIR/LLVM material, Forge records its digest as additional
codegen evidence; it does not make a second compilation from that material.

### Authoritative codegen toolchain binding

Forge obtains the artifact compiler selection from the pinned Verus binary,
not from ambient `rustc`, `rust-toolchain.toml`, or rustup's current default.
`verus --version` must contain exactly one nonempty `Toolchain:` field. Forge
resolves that name with `rustup which --toolchain <name> rustc`, then runs all
compiler queries through `rustup run <name> rustc`. A missing, ambiguous or
unresolvable selection is a hard build failure.

`CodegenRustcEvidence` records and canonically binds:

- the Verus-selected rustup toolchain name;
- rustc's executable digest, full verbose version, release and commit;
- the sysroot and the digests of its rustc and rust-std component manifests;
- the rustc driver and LLVM library digests plus LLVM version;
- target triple, pointer width, endian, the canonical sorted
  `rustc --print target-features` inventory, linker identity, and a canonical
  digest of every file in the selected target library directory.

Install paths remain human-readable provenance but are excluded from the
path-independent codegen identity. File contents, version identities, target
facts and tree-relative target-library names are included. This permits replay
from an equivalent installation prefix without weakening compiler identity.
The closed Verus environment explicitly sets `RUSTUP_TOOLCHAIN` to this bound
selection, so both build and replay use the receipt-declared ABI domain.
Frozen primitive registries may request only names in that bound inventory;
their canonical `-C target-feature=...` argument is reconstructed on validation
and replay.

`HostRustcEvidence` separately records the ambient rustc path, digest and
version for diagnosis. It does not contribute to selection, compatibility or
replay equivalence. A downstream Rust consumer is compatible only when it uses
the artifact-codegen compiler recorded by the receipt; a different ambient
compiler is expected to reject the rlib metadata rather than being treated as
an interchangeable consumer.

## Exports and ABI

### Explicit exports

Only names supplied by `--export` become public. Reachable dependencies remain
private. The L3 emitter gains an export-aware signature path; it does not make
every Thermite function public.

Each receipt export row records:

- Thermite semantic address and source name;
- generated public Rust path;
- canonical parameter and return types;
- mutability/ownership mode;
- whether a total precondition wrapper was generated;
- target layout facts required by the signature;
- crate name, symbol/source name and an `abi_fingerprint`;
- the postcondition certificate identifiers associated with successful
  returns.

Duplicate exports, overloaded names without a unique semantic address, generic
exports without a closed monomorphization, or unsupported public types are
rejected.

The Rust export subset admits finite plain values: primitives, unit, tuples,
fixed arrays, and ordinary acyclic structs recursively composed from those
forms. It also admits a direct finite non-sealed named-record root by value or
shared/exclusive borrow. An opaque direct root may be returned, observed, or
mutated through generated public functions while its fields remain crate-
private; opaque records still fail closed when nested inside ambient arrays or
derived structural relations. Sealed, recursive, reference-bearing, and
heap-backed records fail closed. A returned `enum` is admitted at the direct
return root by `fn result_enum_admission in forge/src/verified_build.rs`; the
admission rule, its layout rule, and its fail-closed boundary are specified
under "Closed result enums at the public boundary" below and governed by
REQ-L3BUILD-15. Borrowed returns remain rejected because the
public lifetime relation is not yet represented in the receipt. Each parameter records `by_value`,
`shared_borrow`, or `exclusive_borrow`; those ownership modes participate in
the ABI fingerprint, which also binds record field order/types and the
opaque/sealed markers.

### Closed result enums at the public boundary

Each fallible transition and observer in `stdlib/kernel-primitives` returns a
closed result enum: `FixedRingPush64`, `FixedRingPop64`, `FixedVecPush64`,
`FixedSlabAllocate64`, `FixedOpenMapLookup64`, and the rest. A consumer may also
reach such a transition through a Thermite function that matches the result and
returns a finite plain value; `fn fixed_slab_allocate_get_probe in
stdlib/kernel-primitives/collections/slab.th` has that shape and builds,
replays, and executes as a strict freestanding L3 export.

The public boundary already carries one closed result enum. REQ-L3BUILD-7's
total wrapper returns `Result<ReturnType, ContractError>`;
`fn lower_with_profile in thermite-lower/src/lower.rs` emits
`pub enum ThermiteContractError { Precondition }` into the same crate,
`fn make_plan in forge/src/verified_build.rs` records it in the plan closure
under the semantic address `generated::ThermiteContractError`, and
`fn plan_exports in forge/src/verified_build.rs` writes the wrapped return as
`Result<...,ThermiteContractError>` into the export signature and its
`abi_fingerprint` preimage. A closed tagged choice over admitted payloads is an
admitted public shape in this design. REQ-L3BUILD-15 carries that shape from the
one generated enum to an authored one under the following admission rule.

#### Admissible shape

A `--export` return type `E` is admitted when all four hold.

1. `E` names an `enum` item declared in the bound program closure. Thermite has
   no open, extensible, or generic enums, so every variant of `E` is present in
   the frozen plan.
2. `E` occurs only as the direct return root. An enum reached through an array
   element, a tuple component, a record field, another enum's variant payload,
   a reference, or a parameter position stays refused.
3. Every variant payload type is an admitted finite plain value: primitives,
   unit, tuples, fixed arrays, and ordinary acyclic non-sealed non-opaque
   structs recursively composed from those forms. This is the alphabet
   `fn supported_public_value_type in forge/src/verified_build.rs` already
   admits. A unit variant carries no payload and is admitted. A payload naming
   a sealed or opaque record is refused, which keeps opaque roots direct-only.
4. No payload reaches `E` or any other enum, so the layout graph stays finite.
   The recursion guard in `fn abi_layout_type in forge/src/verified_build.rs`
   rejects a cycle through an enum name in the same way it rejects one through
   a record name.

Under this rule the shipped scalar-payload collection observers become
admissible by shape: `fixed_slab_get`, `fixed_slab_find_free`,
`fixed_open_map_lookup`, `fixed_open_map_find`, and `fixed_open_map_search`.
The rows that state a `spec fn` precondition are admissible by shape and stay
refused by REQ-L3BUILD-16: `fixed_ring_push`, `fixed_ring_pop`,
`fixed_vec_push`, `fixed_vec_pop`, and the three `fixed_direct_map_*` entries.
Rows whose payloads name an opaque record stay refused by rule 3;
`fixed_slab_allocate`, `fixed_slab_release`, `fixed_freelist_push`,
`fixed_open_map_insert`, and the intrusive family are in that class.

Shape admission is necessary and not sufficient for the five observers above.
Each of them sits on an `#[opaque]` state record and states an `ens` clause that
reads that record's fields, and Verus refuses a field expression on an opaque
datatype inside the postcondition of a public function: "this field expression
is disallowed because of datatype opaqueness … because this is a 'ensures'
clause of public function, this field expression must be well-formed everywhere".
`--export fixed_slab_get` therefore clears the ABI gate under REQ-L3BUILD-15 and
then fails the whole-crate Verus gate. This is the representation-barrier
consequence of the opaque direct root, not a REQ-L3BUILD-15 or REQ-L3BUILD-16
refusal: an opaque observer becomes publicly exportable when its postcondition
is stated through publicly visible specification functions rather than field
reads. `conformance/verified-build/closed_result_enum.th` carries the admitted
shape on a non-opaque state record and is the acceptance fixture.

#### Layout rule

The fingerprint preimage extends the record rule with source-order variant
tags. For `E` with variants in declaration order the preimage is

```text
enum:<E>{<entry>,<entry>,...}
```

where the entry for the variant at zero-based source index `i` is one of

```text
<i>:<Variant>:unit
<i>:<Variant>:tuple(<layout>,...)
<i>:<Variant>:struct{<field>:<layout>,...}
```

Tuple components and struct fields keep source order, `<layout>` is the
existing `fn abi_layout_type` output for that payload type, and a named array
capacity resolves to its integer value. Renaming a variant, reordering
variants, changing a payload type or field order, and changing a resolved
capacity each change the export fingerprint before the enclosing source digest
is considered. The enum name enters the same `visiting` set the record rule
uses, so the recursion diagnostic stays one mechanism.

#### Why the soundness argument survives

- The value alphabet is unchanged. Every admitted payload is already an
  admitted public value type, so the extension adds a closed tag over shapes
  the receipt binds and introduces no new representation at the boundary.
- The consumer's obligation is unchanged. Rust's exhaustiveness rule makes the
  match total, which is the obligation the generated
  `Result<T, ThermiteContractError>` already imposes on every guarded export.
- Erasure is unchanged. An `ens` clause is erased at the boundary for a record
  return in the same way as for an enum return, and the guard tier described
  below is untouched. The ABI states no linearity property today, and this
  extension states none.
- Determinism holds (thermite-design.md §5.3). The randomized `arrow_*`
  metadata hazard recorded in REQ-L3COMPOSE-11 is handled for every L3 library:
  `fn lower_with_profile in thermite-lower/src/lower.rs` sets
  `deterministic_library_enums` from `library.is_some()` and emits every
  library enum through the Forge-owned `__thermite_deterministic_enum!` frame,
  so an enum in a plain `--export` library reproduces byte-identically.
- The refusal set keeps its stated causes. Sealed types stay unforgeable,
  opaque records stay direct-root-only, recursive layouts stay unbounded,
  reference-bearing returns stay outside the receipt's lifetime relation, and
  heap-backed values stay outside the allocation-free kernel profile.

Shipped evidence: `fn result_enum_admission in forge/src/verified_build.rs`
classifies the return root, `fn named_abi_layout in forge/src/verified_build.rs`
emits the preimage above under the `visiting` guard `fn abi_layout_type` owns,
and `fn plan_exports in forge/src/verified_build.rs` consumes both. The
three-variant result enum in `conformance/verified-build/closed_result_enum.th`,
whose payloads are one finite plain record plus a `usize` and a `u64` alongside
a unit variant, builds to a published strict L3 `--target kernel` receipt with
every translation-validation row faithful and `errors: 0`, reproduces its
binding and rlib digests across three builds, replays, links into a freestanding
`no_std` consumer that matches every variant, and is executed through the
published rlib.

### Preconditions at an unverified caller boundary

A Verus `requires` clause is erased and cannot be treated as a runtime guard
for downstream unverified Rust. Therefore:

- an export whose effective precondition is `true` may expose the verified
  implementation directly; and
- an export with a nontrivial, executable precondition receives a generated
  total wrapper in the same Verus crate.

Conceptually, the wrapper has this contract:

```text
requires true
returns Result<ReturnType, ContractError>
Ok(value)  => the original function ran and its ensures clauses hold
Err(Precondition) => the implementation was not called
```

Verus proves that the successful guard establishes the implementation's
precondition. Contract TV covers the executable guard. The wrapper itself is
part of the closure, final whole-crate proof, compiled source and receipt.
Its successful-result pattern uses a compiler-reserved fresh identifier that is
checked against every source parameter before emission. Postconditions replace
the Thermite `result` binder with only that fresh identifier; a user parameter
named `value` or even the preferred internal spelling therefore cannot capture
the result or change the proved wrapper contract.

If a precondition cannot be evaluated faithfully at runtime—for example, its
quantifier or ghost dependency has no admitted executable translation—the
function cannot be exported in v1. Forge reports the clause and missing
capability instead of exposing a partial API or inserting an unchecked call.

#### Specification-function guards

`fn executable_precondition in forge/src/verified_build.rs` admits integer and
boolean literals, paths, binary and unary operators, casts, and `.len()`. It
refuses `Expr::Call`, so an export whose `req` calls a `spec fn` is rejected at
the `exports` stage with "has a non-executable precondition and cannot receive a
total wrapper". Collection transitions state such a precondition as a matter of
course; `fixed_ring_push` states `req fixed_ring_wf_spec(&ring)`. This gate is
independent of the ABI subset above and outlives REQ-L3BUILD-15.

Widening that predicate on its own does not produce a guard. `fn
lower_l3_export_wrapper in thermite-lower/src/lower.rs` emits the runtime test
with `lower_expr(&f.contract.req.expr, Ctx::exec(), 0, f.span)`, and a `spec fn`
has no executable translation, so the emitted crate is rejected by Verus with
`error: cannot call function fixed_ring_wf_spec with mode spec`. `fn
exec_tv_export_guard in forge/src/exec_tv.rs` derives the guard independently
for the mandatory `wrapper_guard` translation-validation row, so any executable
form must be reproducible there as well. Today's rejection at the `exports`
stage is the correct outcome, and this document already states the rule behind
it: an export whose precondition has no faithful runtime evaluation cannot be
exported in v1.

REQ-L3BUILD-16 names the whole job: an admitted rule for deriving an executable
form of a specification-function guard, its emission in
`fn lower_l3_export_wrapper`, and its independent reproduction in
`fn exec_tv_export_guard` so the `wrapper_guard` row stays faithful. A `spec fn`
whose body lies outside the executable fragment (a quantifier, a ghost
dependency, or recursion without an executable counterpart) keeps the existing
refusal stated above: the function cannot be exported, and Forge reports the
clause and the missing capability.

### Rust ABI scope

The v1 artifact is an `rlib` intended for downstream Rust or kernel Rust built
with the exact receipt-pinned compiler, target and dependency lock. Rust does
not promise a compiler-independent binary ABI, so Forge does not claim one.
The `abi_fingerprint` makes compatibility explicit: consumers must match the
recorded toolchain, target, crate name, type layouts and export signatures.
For finite plain records and arrays, the fingerprint preimage recursively
expands record names into ordered field names/types and resolves named array
capacities to their integer values. A field type/order or capacity change thus
changes the export fingerprint even before the enclosing receipt/source digest
is considered. Conformance also links a downstream consumer with the exact
receipt-pinned compiler, constructs exported finite records, and executes the
generated aggregate-array relation from the published rlib.

A future stable C ABI requires separately designed, verified `extern "C"`
wrappers and an ABI-safe type subset. Such wrappers must be emitted, verified
and compiled inside the same canonical source; an unverified post-build thunk
is forbidden.

## Runtime-check policy

The current L1 build is unchanged:

```text
Thermite AST ──lower_l1──► Rust with always-active thermite_check! ──rustc──► L1 artifact
```

The L3 build is:

```text
Thermite AST ──L3 Verus emission──► verified executable crate ──Verus compile──► L3 artifact
```

The L3 path does not splice in `lower_l1` bodies, `thermite_check!` macros, or
independently lowered postcondition checks. Preconditions at public boundaries
are handled by the verified total wrappers above. Postconditions, invariants
and internal call preconditions are proof obligations, not default runtime
checks.

Defense-in-depth runtime checks may be added later only if their guards,
failure behavior and helper code are generated inside the same Verus source
and pass the same proof and TV closure. Reusing the independent L1 lowerer
would reopen the gap and is expressly prohibited.

## Target profiles

### Hosted `std`

The hosted v1 output is a library rlib. It may use the target-pinned standard
library surface admitted by the L3 emitter and strict closure policy. Generated
sample `main` functions and seccomp injection remain features of the L1
`--entry` path, not L3.

### Kernel

`--target kernel` produces:

- `#![no_std]`;
- `extern crate alloc` when the reachable closure needs allocation;
- `--crate-type=rlib`;
- `panic=abort`;
- no `main`;
- no seccomp or hosted effect wrappers;
- no ambient `read`/`write`/`net`/`term`/`time`/`rand` effects;
- no `#[boundary]`, `#[slag]`, unresolved calls or diverging exemptions.

Kernel proof dependencies are explicit. Forge binds the pinned `vstd.vir`, the
complete pinned vstd source-tree digest, its deterministic erased `no_std`
metadata source and rlib, and the portable final argument shape. This
dependency contributes no allocator or hosted slice adapter; executable
`&[u8]` reads remain Rust core operations.

The kernel host supplies its panic handler and allocator at final link as
needed. A conformance harness links the produced rlib into a separate
`no_std` consumer crate, calls every declared export with an ABI-compatible
signature, and completes a real final link. Merely producing an rlib is not
enough acceptance evidence.

Export wrappers return `Result` for failed preconditions and do not panic. Any
reachable allocation is reflected in the effect and ABI records and requires a
host allocator at final link.

## `VerifiedBuildReceiptV1`

This receipt is distinct from both:

- `BuildManifest`, which describes today's L1 runtime-checked artifact; and
- `BurnReceipt`, which records proof-token/cited-lemma information and excludes
  oracle evidence.

Overloading either would blur different trust claims. The new receipt is
defined specifically for proof-to-artifact correspondence.

### Bundle layout

```text
<name>.verified/
  artifact/
    lib<crate>.rlib
    deps/                  # any non-system link-time dependencies
  receipt.json
  evidence/
    input.th
    artifact-plan.v1
    source.verus.rs
    certificates.json
    translation-validation.json
    verus-result.json
    toolchain.json
    kernel-vstd-link.rs   # kernel target only
```

Kernel bundles also place the generated metadata dependency at
`artifact/deps/libvstd.rlib`. Both conditional files are covered by the
ordinary receipt inventory and binding root.

The receipt uses bundle-relative paths only. Optional human-readable logs may
be included under `evidence/`, but no unbound field may contribute to an
assurance claim.

### Binding object

`receipt.json` contains a `binding` object and
`binding_sha256`. The digest is computed from a versioned canonical binary
encoding of `binding`, not from ordinary JSON serialization. The encoding uses
domain-separated, field-named, length-prefixed SHA-256 in the same style as
Forge's content-addressed proof cache. Map ordering and JSON whitespace are
therefore irrelevant.

The binding contains at least:

```text
schema/domain
source:
  raw source digest
  normalized parsed-program digest
plan:
  ArtifactPlanV1 digest
  exports, closure nodes and dependency edges
  strict policy digest
verification:
  exact Verus source digest
  per-item certificate-set digest
  contract/exec/body/loop TV evidence digest
  final whole-crate Verus result digest
  no-cheating and erasure-check policy
toolchain:
  Forge version and source identity
  Verus version/source identity
  authoritative Verus-selected rustup toolchain
  artifact rustc, sysroot, component manifests, rustc driver and LLVM identity
  target-library tree digest and linker identity
  ambient host rustc as non-authoritative diagnostic provenance
  Z3 identity
  vstd/dependency lock digest
  kernel-only pinned vstd VIR/source-tree and generated no_std metadata evidence
  target triple, data layout, crate options and ordered arguments
artifact:
  relative path, kind, length and SHA-256
  non-system link dependency paths and digests
  crate name and export ABI fingerprints
```

The receipt records environment variables that can affect proof or codegen and
rejects non-whitelisted variables. Paths, timestamps and process IDs are either
normalized out or identified as non-semantic display data.

`binding_sha256` is excluded from its own preimage to avoid a hash cycle. The
artifact digest is inside the binding; the receipt does not need to be embedded
in the artifact. A signature over `binding_sha256` may be added later without
changing the correspondence claim.

### Validation and replay

`forge verify-build`:

1. parses only the supported version;
2. recomputes the canonical binding digest;
3. rejects missing, extra-path, traversal or symlink escapes;
4. rehashes every referenced file and the artifact;
5. recomputes the ABI fingerprints and closure/plan consistency checks;
6. confirms the recorded policy contains every mandatory strict gate;
7. verifies the semantic relations inside the codegen record, including that
   Verus's `Toolchain:` field names the recorded artifact compiler closure.

`--replay` recreates the private compile input from the bound plan/source,
resolves the current pinned Verus binary's authoritative codegen selection,
requires its path-independent identity to match the receipt, explicitly selects
that toolchain, and, for a kernel receipt, re-hashes the pinned VIR and source
tree and independently rebuilds the erased metadata rlib. It then reruns the
exact pinned Verus command and compares the new artifact digest. The ambient
host compiler is neither selected nor compared.
If the pinned tools are unavailable, replay fails as unavailable; it never
reports structural hash validation as proof replay.

## Atomicity and failure behavior

The L3 operation is check-and-build atomic from the user's perspective:

- all work occurs in a private scratch tree;
- the destination bundle is absent until every gate, compile and hash succeeds;
- evidence and receipt are staged only after the artifact exists;
- Forge revalidates the complete staged bundle using the same library as
  `forge verify-build`;
- files and the parent directory are flushed before publication;
- the staged directory is atomically renamed into its final path on the same
  filesystem.

v1 refuses to overwrite an existing bundle. This avoids a non-atomic
multi-directory replacement and accidental loss of a prior verified artifact.
A future `--force` mode must specify an equally recoverable replacement
protocol before it ships.

Any error removes the scratch tree through a drop guard and leaves no final
artifact, no receipt, and no partially published directory. Console output
must not print a successful artifact path before the rename completes.

## Requirements

- **REQ-L3BUILD-1 (explicit L3 build mode).** `forge build --level l3`
  selects a new verified-build path and requires explicit exports. The existing
  no-level build remains L1 and is labeled L1.
- **REQ-L3BUILD-2 (frozen artifact plan).** One canonical
  `ArtifactPlanV1` binds source, target, exports, wrappers and the complete
  executable/proof closure before verification begins.
- **REQ-L3BUILD-3 (strict end-to-end closure).** Every reachable executable
  node must be fully resolved, escape-hatch-free and L3-certifying. A downgrade,
  boundary, slag item, hole, divergence exemption or unresolved dependency
  rejects the build.
- **REQ-L3BUILD-4 (compile the verified source).** The final whole-crate
  Verus invocation uses `--no-cheating --compile`; the exact canonical source
  accepted by that invocation is the only source from which the artifact may
  be generated.
- **REQ-L3BUILD-5 (complete translation validation).** Contract, executable
  expression, statement/body and loop TV cover every corresponding reachable
  node. Divergent, skipped, unsupported or unverifiable coverage rejects.
- **REQ-L3BUILD-6 (explicit verified exports).** Only declared exports are
  public; dependencies remain private. Every exported signature and layout is
  recorded in a receipt-bound ABI fingerprint.
- **REQ-L3BUILD-7 (total precondition boundary).** A nontrivial exported
  precondition is enforced by a TV-covered, Verus-verified total `Result`
  wrapper in the canonical source, or the export is rejected.
- **REQ-L3BUILD-8 (cryptographic receipt).** A separate
  `VerifiedBuildReceiptV1` binds source/IR, closure, certificates, TV evidence,
  toolchain, flags, exports and artifact through a canonical SHA-256 root.
- **REQ-L3BUILD-9 (atomic publication).** Forge publishes a complete,
  self-validating bundle with one atomic rename only after all proof, compile
  and binding steps pass; failure publishes nothing.
- **REQ-L3BUILD-10 (honest artifact assurance).** The artifact headline is
  the minimum over the complete reachable executable closure and generated
  wrappers, capped at L3. There is no downgraded L3 artifact.
- **REQ-L3BUILD-11 (kernel linkability).** The kernel profile emits a
  `no_std + alloc`, `panic=abort` rlib with no hosted boundary and passes a
  separate downstream `no_std` final-link test through its declared exports.
- **REQ-L3BUILD-12 (fault-injection rejection).** Mutation of the executable
  body, helper, wrapper, source file, plan, evidence, toolchain record or
  artifact after the plan freezes prevents publication.
- **REQ-L3BUILD-13 (L1 separation).** The current `lower_l1` build and its
  runtime checks remain available only as an honest L1 artifact path and cannot
  satisfy, seed or substitute for the final L3 proof-and-compile step.
- **REQ-L3BUILD-14 (authoritative codegen toolchain).** Forge binds the
  rustc/sysroot/target-library/rustc-driver/LLVM closure selected by the pinned
  Verus distribution, distinguishes it from ambient host rustc provenance, and
  explicitly selects the bound closure for build, replay and ABI consumers.
- **REQ-L3BUILD-15 (closed result-enum exports).** A `--export` return type
  may be a closed, non-recursive enum whose every variant payload is an
  already-admitted finite plain value, at the direct return root only. Its
  canonical layout preimage carries source-order variant tags and recursively
  expanded payloads. Nested, parameter-position, recursive, sealed-payload,
  opaque-payload, reference-bearing, and heap-backed enums fail closed.
- **REQ-L3BUILD-16 (specification-function export guards).** An export whose
  `req` calls a `spec fn` receives a total wrapper only when Forge derives an
  admitted executable form of that guard, `lower_l3_export_wrapper` emits it,
  and the independent `export_guard` derivation reproduces it as a faithful
  translation-validation row. A guard outside the executable fragment keeps
  the REQ-L3BUILD-7 refusal.

## Acceptance criteria

- **AC-1 (positive hosted build).** A supported pure corpus function builds as
  an rlib; the whole-crate Verus result reports zero errors, the source digest
  equals the receipt digest, and a downstream pinned-Rust crate calls the
  declared export successfully.
- **AC-2 (same-source structural pin).** Instrumentation records one canonical
  source path and digest for the final Verus proof/compile. Tests fail if a
  second executable emitter or `lower_l1` call is introduced into the L3 path.
- **AC-3 (known divergence regressions).** The scenarios pinned by
  `divergence_bytes_eq_l1_empty_window.rs` and
  `divergence_numfmt_zero.rs` cannot produce an L3 artifact from the divergent
  L1 body; the artifact behavior comes from the verified L3 emission.
- **AC-4 (bad body rejects).** An operator, branch, return value, loop update or
  call mutation that violates the contract yields a nonzero build, no final
  bundle and a source-located proof diagnostic.
- **AC-5 (proof-preserving unauthorized mutation rejects).** A test mutation
  after plan freeze that would still verify independently is rejected by the
  expected-source/plan digest mismatch.
- **AC-6 (no downgrade).** L1/L2 certificates, timeout degradation,
  counterexamples, rejected obligations and missing certificates each produce
  a named error and no artifact.
- **AC-7 (no skipped TV).** For each TV phase, a divergent, unsupported,
  skipped and unverifiable fixture independently blocks publication.
- **AC-8 (closure is fail-closed).** Direct and transitive `#[slag]`,
  `#[boundary]`, unresolved, cross-file and diverging dependencies each block;
  the diagnostic includes the export-to-offender path.
- **AC-9 (export visibility).** Only requested exports are link-visible through
  crate metadata; an unrequested reachable helper is private. Receipt ABI rows
  match an independently compiled consumer's view.
- **AC-10 (precondition wrapper).** A valid call returns `Ok` with the proved
  result; an invalid call returns `Err(Precondition)` without invoking the
  implementation. A non-executable precondition is rejected at build time.
- **AC-11 (receipt tampering matrix).** Mutating each bound component—source,
  plan, closure, certificate, TV result, tool identity, flag, ABI row or
  artifact—causes `forge verify-build` to fail.
- **AC-12 (atomic failure).** Injected failures before and after Verus,
  codegen, artifact hashing and receipt staging leave no destination bundle or
  partial sibling. Success publishes one self-validating directory.
- **AC-13 (kernel final link).** A pure or alloc-using supported function builds
  under `--target kernel`, and a separate `no_std` harness with a test allocator
  and panic handler links and calls it. Ambient effects and hosted boundaries
  reject before codegen.
- **AC-14 (L1 compatibility).** Existing build conformance and kernel-target
  L1 tests remain byte/behavior compatible when `--level l3` is absent, and
  every current L1 manifest still states L1.
- **AC-15 (reproducible replay).** Two builds under the same pinned inputs have
  identical canonical source, plan, receipt binding and artifact digests;
  `forge verify-build --replay` reproduces the artifact digest.
- **AC-16 (ambient mismatch isolation).** With an incompatible rustc selected
  in the shell, hosted and kernel builds still record and use Verus's pinned
  codegen rustc, replay succeeds, and repeat builds remain reproducible.
- **AC-17 (declared consumer ABI).** A separate consumer compiled with the
  receipt-declared codegen rustc links each hosted and kernel artifact, while
  the intentionally incompatible ambient compiler fails with an rlib metadata
  version error.
- **AC-18 (codegen closure tampering).** Mutating the recorded codegen
  selection or any compiler, sysroot, component-manifest, driver, LLVM, target
  or target-library identity causes structural validation or replay to fail.
- **AC-19 (closed enum export builds and replays).** `ring_offer` in
  `conformance/verified-build/closed_result_enum.th`, whose `req` is `true` and
  whose result enum has two record-payload variants and one unit variant, builds
  under `--target kernel`, publishes a strict receipt with every
  translation-validation row faithful and `errors: 0`, reproduces its binding and
  artifact digests across three builds, replays to the same artifact digest, and
  is called from a separate `no_std` consumer that matches every variant. This
  criterion previously named `fixed_slab_get`; that observer clears the ABI gate
  and then fails whole-crate Verus on the opaque-postcondition rule recorded
  under "Closed result enums at the public boundary", so the acceptance fixture
  carries the same admitted shape on a non-opaque state record.
- **AC-20 (enum fingerprint sensitivity).** Renaming a variant, reordering two
  variants, changing a payload field type or order, and changing a resolved
  payload capacity each change the export `abi_fingerprint` independently of
  the source digest.
- **AC-21 (enum fail-closed matrix).** An enum in parameter position, an enum
  inside an array element, tuple component, record field, or another variant
  payload, a variant payload naming a sealed or opaque record, a payload
  naming a reference or a heap-backed value, and a payload cycling back to the
  enum each reject at the `exports` stage with a named diagnostic and publish
  nothing.
- **AC-22 (guarded enum export stays refused until REQ-L3BUILD-16).**
  `fixed_ring_push` rejects with the non-executable-precondition diagnostic
  while its return type is admitted, so the two gates are separately
  observable.
- **AC-23 (specification-function guard is proved and validated).** An export
  whose `req` calls a `spec fn` with an executable form builds a total wrapper
  that returns `Err(Precondition)` without calling the implementation on a
  violating input, its `wrapper_guard` translation-validation row is faithful,
  and a `spec fn` outside the executable fragment rejects at build time.

## Verification matrix

| Property | Primary test surface |
|---|---|
| CLI compatibility and flag conflicts | `forge/src/cli.rs` unit tests |
| Canonical plan and closure | `forge/src/verified_build.rs` unit tests and `forge/tests/verified_build.rs` |
| L3 emission and exact-source invariant | `thermite-lower` golden tests plus `forge/tests/verified_build.rs` |
| Strict proof invocation | real pinned Verus integration test |
| Contract/exec/body/loop TV completeness | phase-specific positive and refusal fixtures |
| Export wrapper proof and behavior | real Verus compile plus downstream Rust consumer |
| Closed result-enum ABI and fail-closed matrix | `forge/tests/verified_build.rs` export planning plus a kernel consumer that matches every variant |
| Receipt canonicalization/tampering | field-by-field mutation/property tests |
| Verus codegen binding and Rust ABI | ambient-mismatch hosted/kernel builds, receipt-declared consumers and incompatible-consumer rejection |
| Atomic publication and cleanup | injected-stage failure integration tests |
| Kernel linkability | separate real `no_std` final-link harness |
| L1 non-regression | existing `build_conformance` and `kernel_target` suites |

The implementation gauntlet includes the existing formatter, clippy and crate
tests plus the real pinned Verus and rustc integration tests. Tests that cannot
find a required pinned tool fail the verified-build gate; they do not silently
skip.

## Implemented increments

The CLI was exposed only after all requirements needed for the L3 claim were
present. No intermediate increment emitted an artifact labeled L3.

1. **Plan and strict closure.** Add `ArtifactPlanV1`, export roots, complete
   dependency classification, deterministic encoding and all fail-closed
   preflight diagnostics. Add the governing routes for the new module and each
   existing change site.
2. **Export-aware L3 crate emission.** Teach the L3 lowerer to emit a library
   profile, selected public functions and verified total wrappers. Keep
   `lower_l1` untouched.
3. **Mandatory TV aggregation.** Expose library entry points for every required
   TV phase, produce a complete expected-vs-observed inventory from the plan,
   and reject missing coverage.
4. **Single-invocation proof/codegen.** Add the private compile input,
   strict Verus flag allowlist, source rehash checks and whole-crate
   `--no-cheating --compile` result parsing.
5. **Receipt and atomic bundle.** Add canonical binding, the evidence bundle,
   `forge verify-build`, replay, staging, fsync and atomic publication.
6. **Kernel and adversarial gate.** Add the final-link harness, mutation seams,
   tampering matrix, known L1/L3 divergence regressions and full non-regression
   gauntlet. Expose `--level l3` only when this gate is green.
7. **Codegen-toolchain closure.** Split ambient host provenance from the
   authoritative Verus-selected codegen closure, bind every compiler and target
   input, select it explicitly during replay, and exercise both matching and
   incompatible downstream consumers.

## Resolved design questions

- **Does L3 reuse `BuildManifest`?** No. It produces
  `VerifiedBuildReceiptV1`; the L1 manifest remains semantically stable.
- **Does L3 include current L1 runtime checks?** No. Public preconditions use
  verified total wrappers; other contracts are proved. Future
  defense-in-depth checks must live in the same verified source.
- **Can an L4 clause produce an L4 artifact?** No. Clause rung and executable
  correspondence are separate; the artifact claim is capped at L3.
- **Can a to-boundary proof build?** No. v1 verified artifacts require an
  end-to-end reachable closure.
- **Can a cache eliminate final verification?** No. The final exact-source
  Verus proof-and-compile always runs.
- **Is the Rust ABI globally stable?** No. v1 pins the exact Rust toolchain and
  records an ABI fingerprint. A portable C ABI is a separate verified-wrapper
  design.
- **Which rustc does the receipt pin?** The rustc selected by pinned Verus's
  authoritative `Toolchain:` field. Ambient rustc is diagnostic provenance
  only and cannot authorize build, replay or consumption.
- **May an authored enum cross the public ABI?** A closed, non-recursive one
  whose payloads are already-admitted finite plain values may, at the direct
  return root, under REQ-L3BUILD-15. The boundary already carries the
  generated `Result<T, ThermiteContractError>`, so the shape is not new; the
  admission rule keeps the payload alphabet and the layout preimage closed.
- **Does admitting that shape export `fixed_ring_push`?** No. Its `req` calls
  `fixed_ring_wf_spec`, which REQ-L3BUILD-16 governs. The ABI admission and
  the guard are separate requirements with separate blockers.
- **Does admitting that shape export an opaque-rooted observer?** Not on its
  own. `fixed_slab_get` and its siblings state postconditions that read the
  fields of an `#[opaque]` record, and Verus disallows such a field expression
  in the `ensures` clause of a public function. Restating those postconditions
  through publicly visible specification functions is the work that makes them
  exportable; it belongs to the owning primitive, not to REQ-L3BUILD-15.
- **What does Forge publish on a failed build?** Nothing at the requested
  destination.

## Shipped: closed result-enum public ABI admission

REQ-L3BUILD-15's admission rule is implemented at the single `exports`
admission site:

- `fn result_enum_admission in forge/src/verified_build.rs` classifies the
  direct return root, returning `NotAnEnum`, `Admitted`, or a `Refused` cause
  that names the variant and payload;
- `fn reachable_enum_name in forge/src/verified_build.rs` gives parameter
  positions and nested return positions their own diagnostic, and
  `fn supported_public_param_type` continues to refuse every enum;
- `fn named_abi_layout in forge/src/verified_build.rs` emits the source-order
  variant preimage under the `visiting` guard `fn abi_layout_type` owns, so the
  recursion diagnostic stays one mechanism for records and enums;
- `fn plan_exports in forge/src/verified_build.rs` consumes all three and its
  general refusal now names the admitted direct-return-root subset;
- the closure planner already binds every reachable `Item::Enum` declaration and
  its `item_sha256`; the fixture's `RingOffer` appears in the plan closure with
  no change to the walk.

Remaining in this area, unblocked by REQ-L3BUILD-15: pin the measured
`fixed_ring_empty` aggregate-rooted kernel receipt as a conformance fixture,
since no shipped test covers it today.

## Blocker: executable guard for a specification-function precondition

Filed by the orchestrator; referenced by REQ-L3BUILD-16. Scope:

- decide and document the admitted derivation for an executable form of a
  `spec fn` guard, covering which `spec fn` bodies qualify and how the
  executable form is bound to the specification it guards;
- emit that form from `fn lower_l3_export_wrapper in thermite-lower/src/lower.rs`
  in place of `lower_expr(..., Ctx::exec(), ...)` applied to the raw call, which
  today produces `error: cannot call function fixed_ring_wf_spec with mode spec`;
- reproduce the same derivation independently in `fn exec_tv_export_guard in
  forge/src/exec_tv.rs` so the mandatory `wrapper_guard` row stays faithful;
- widen `fn executable_precondition in forge/src/verified_build.rs` only
  together with the two items above. Widening it alone moves the failure from a
  named `exports`-stage rejection to a whole-crate Verus error, and any repair
  that emits a weaker guard or drops the `wrapper_guard` row silently weakens
  REQ-L3BUILD-7;
- add the AC-22 and AC-23 fixtures, including the refusal for a `spec fn` guard
  outside the executable fragment.

## REQ status

<!-- generated:reqs view=forge-l3-verified-build-status -->
Source: `.design/reqs/registry.toml`

| ID | Status | Owner | Title | Follow-up |
|---|---|---|---|---|
| REQ-L3BUILD-1 | shipped | `.design/build/l3-verified-artifact.md` | Explicit L3 build mode |  |
| REQ-L3BUILD-10 | shipped | `.design/build/l3-verified-artifact.md` | Honest artifact assurance aggregate |  |
| REQ-L3BUILD-11 | shipped | `.design/build/l3-verified-artifact.md` | Freestanding kernel linkability |  |
| REQ-L3BUILD-12 | shipped | `.design/build/l3-verified-artifact.md` | Post-freeze mutation and tampering rejection |  |
| REQ-L3BUILD-13 | shipped | `.design/build/l3-verified-artifact.md` | Strict separation from the L1 build |  |
| REQ-L3BUILD-14 | shipped | `.design/build/l3-verified-artifact.md` | Authoritative Verus codegen-toolchain binding |  |
| REQ-L3BUILD-15 | shipped | `.design/build/l3-verified-artifact.md` | Closed result-enum public exports |  |
| REQ-L3BUILD-16 | not_started | `.design/build/l3-verified-artifact.md` | Specification-function export guards | Open blocker: `.design/build/l3-verified-artifact.md` "Blocker: executable guard for a specification-function precondition". `fn executable_precondition` in forge/src/verified_build.rs refuses `Expr::Call`, so every collection transition with a `req <name>_wf_spec(...)` precondition is rejected at the exports stage. `fn lower_l3_export_wrapper` in thermite-lower/src/lower.rs lowers the guard with `lower_expr(..., Ctx::exec(), ...)`, and a spec fn has no executable translation, so widening the predicate alone yields `error: cannot call function fixed_ring_wf_spec with mode spec`. Define the admitted derivation, emit it from the wrapper, reproduce it in `fn exec_tv_export_guard` in forge/src/exec_tv.rs, and land AC-22 and AC-23. This requirement is independent of REQ-L3BUILD-15 and outlives it. |
| REQ-L3BUILD-2 | shipped | `.design/build/l3-verified-artifact.md` | Frozen canonical artifact plan |  |
| REQ-L3BUILD-3 | shipped | `.design/build/l3-verified-artifact.md` | Strict end-to-end reachable closure |  |
| REQ-L3BUILD-4 | shipped | `.design/build/l3-verified-artifact.md` | Compile the exact verified source |  |
| REQ-L3BUILD-5 | shipped | `.design/build/l3-verified-artifact.md` | Complete translation-validation coverage |  |
| REQ-L3BUILD-6 | shipped | `.design/build/l3-verified-artifact.md` | Explicit verified exports and ABI fingerprints |  |
| REQ-L3BUILD-7 | shipped | `.design/build/l3-verified-artifact.md` | Total verified precondition boundary |  |
| REQ-L3BUILD-8 | shipped | `.design/build/l3-verified-artifact.md` | Cryptographically bound verified-build receipt |  |
| REQ-L3BUILD-9 | shipped | `.design/build/l3-verified-artifact.md` | Atomic verified-bundle publication |  |
<!-- /generated:reqs -->
