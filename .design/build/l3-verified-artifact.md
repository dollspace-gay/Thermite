# Correspondence-backed L3 build artifacts — compile the verified body

<!--
tier: 3-component
status: shipped
audited-content-sha256: 8c6e9d22497dae609190eaaad5f5238ff94d071b87f850f588ba9a8d6b958427
decision: Option A — compile the canonical Verus executable body that was verified
issue: github:dollspace-gay/Thermite#101
governs:
  - forge/src/verified_build.rs (new)
  - forge/src/build.rs
  - forge/src/cli.rs
  - forge/src/closure.rs
  - forge/src/manifest.rs
  - thermite-lower/src/lower.rs
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

No independently reconstructed Rust source may appear between successful
verification and code generation. If a future Verus integration exposes
post-erasure Rust/MIR/LLVM material, Forge records its digest as additional
codegen evidence; it does not make a second compilation from that material.

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

If a precondition cannot be evaluated faithfully at runtime—for example, its
quantifier or ghost dependency has no admitted executable translation—the
function cannot be exported in v1. Forge reports the clause and missing
capability instead of exposing a partial API or inserting an unchecked call.

### Rust ABI scope

The v1 artifact is an `rlib` intended for downstream Rust or kernel Rust built
with the exact receipt-pinned compiler, target and dependency lock. Rust does
not promise a compiler-independent binary ABI, so Forge does not claim one.
The `abi_fingerprint` makes compatibility explicit: consumers must match the
recorded toolchain, target, crate name, type layouts and export signatures.

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
```

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
  rustc and LLVM identity
  Z3 identity
  vstd/dependency lock digest
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
6. confirms the recorded policy contains every mandatory strict gate.

`--replay` recreates the private compile input from the bound plan/source,
reruns the exact pinned Verus command, and compares the new artifact digest.
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

## Verification matrix

| Property | Primary test surface |
|---|---|
| CLI compatibility and flag conflicts | `forge/src/cli.rs` unit tests |
| Canonical plan and closure | `forge/src/verified_build.rs` unit tests and `forge/tests/verified_build.rs` |
| L3 emission and exact-source invariant | `thermite-lower` golden tests plus `forge/tests/verified_build.rs` |
| Strict proof invocation | real pinned Verus integration test |
| Contract/exec/body/loop TV completeness | phase-specific positive and refusal fixtures |
| Export wrapper proof and behavior | real Verus compile plus downstream Rust consumer |
| Receipt canonicalization/tampering | field-by-field mutation/property tests |
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
- **What does Forge publish on a failed build?** Nothing at the requested
  destination.

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
| REQ-L3BUILD-2 | shipped | `.design/build/l3-verified-artifact.md` | Frozen canonical artifact plan |  |
| REQ-L3BUILD-3 | shipped | `.design/build/l3-verified-artifact.md` | Strict end-to-end reachable closure |  |
| REQ-L3BUILD-4 | shipped | `.design/build/l3-verified-artifact.md` | Compile the exact verified source |  |
| REQ-L3BUILD-5 | shipped | `.design/build/l3-verified-artifact.md` | Complete translation-validation coverage |  |
| REQ-L3BUILD-6 | shipped | `.design/build/l3-verified-artifact.md` | Explicit verified exports and ABI fingerprints |  |
| REQ-L3BUILD-7 | shipped | `.design/build/l3-verified-artifact.md` | Total verified precondition boundary |  |
| REQ-L3BUILD-8 | shipped | `.design/build/l3-verified-artifact.md` | Cryptographically bound verified-build receipt |  |
| REQ-L3BUILD-9 | shipped | `.design/build/l3-verified-artifact.md` | Atomic verified-bundle publication |  |
<!-- /generated:reqs -->
